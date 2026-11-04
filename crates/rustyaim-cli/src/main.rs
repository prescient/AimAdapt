//! Command-line front end for RustyAim: import KovaaK's stats and print summaries.
//! The desktop UI will sit on the same `rustyaim-core` library.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use chrono::Local;
use clap::{Parser, Subcommand};
use notify::{RecursiveMode, Watcher};
use rustyaim_core::progress::{self, Progress, ProgressSettings};
use rustyaim_core::training::{self, TimeSettings, TrainingReport};
use rustyaim_core::{Db, ImportReport, discover, import, summary};

#[derive(Parser)]
#[command(name = "rustyaim", about = "Track your KovaaK's aim training")]
struct Cli {
    /// Database file. Defaults to %APPDATA%\RustyAim\rustyaim.db
    #[arg(long, global = true)]
    db: Option<PathBuf>,

    /// KovaaK's stats folder. Found automatically in your Steam libraries if not given.
    #[arg(long, global = true)]
    stats: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Import new stats files into the database
    Import,
    /// Keep running and import new runs as KovaaK's writes them
    Watch,
    /// Print totals, time per day and most-played scenarios
    Summary {
        /// How many scenarios to list
        #[arg(long, default_value_t = 10)]
        top: usize,
    },
    /// Training time: sessions, days, streak, weekdays and hours
    Time {
        /// Break length (minutes) that starts a new session
        #[arg(long, default_value_t = 15)]
        gap: i64,
        /// Hour a training day starts (so late-night sessions count toward the previous day)
        #[arg(long, default_value_t = 4)]
        day_start: u32,
        /// Daily goal in minutes of active time
        #[arg(long, default_value_t = 30.0)]
        goal: f64,
        /// How many recent sessions to list
        #[arg(long, default_value_t = 10)]
        sessions: usize,
    },
    /// Progress per scenario, or one scenario's full history
    Progress {
        /// Scenario name, or part of it. Omit to list all scenarios.
        scenario: Option<String>,
        /// Sort order for the list
        #[arg(long, value_enum, default_value_t = SortBy::Played)]
        sort: SortBy,
        /// How many scenarios to list
        #[arg(long, default_value_t = 20)]
        top: usize,
    },
    /// Sensitivity and FOV changes over time
    Sens,
    /// List files that couldn't be imported
    Errors,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum SortBy {
    Played,
    Improved,
    Recent,
    Name,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let db_path = match cli.db {
        Some(p) => p,
        None => default_db_path()?,
    };
    let mut db = Db::open(&db_path).with_context(|| format!("opening database {}", db_path.display()))?;

    match cli.command {
        Command::Import => {
            let dir = stats_dir(cli.stats)?;
            println!("Importing from {}", dir.display());
            print_report(&import::import_dir(&mut db, &dir)?);
        }
        Command::Watch => watch(&mut db, &stats_dir(cli.stats)?)?,
        Command::Summary { top } => print_summary(&db, top)?,
        Command::Time { gap, day_start, goal, sessions } => {
            if day_start > 23 {
                bail!("--day-start must be an hour from 0 to 23");
            }
            let settings = TimeSettings { session_gap_min: gap, day_start_hour: day_start, daily_goal_min: goal, ..Default::default() };
            print_time(&training::report(&db, &settings, Local::now().naive_local())?, &settings, sessions);
        }
        Command::Progress { scenario, sort, top } => {
            let p = progress::load(&db, &TimeSettings::default())?;
            let settings = ProgressSettings::default();
            match scenario {
                Some(query) => print_scenario(&p, &find_scenario(&p, &query)?, &settings),
                None => print_progress_list(&p, &settings, sort, top),
            }
        }
        Command::Sens => print_sens(&progress::load(&db, &TimeSettings::default())?),
        Command::Errors => {
            let errors = db.import_errors()?;
            if errors.is_empty() {
                println!("No import errors.");
            }
            for (file, error) in errors {
                println!("{file}\n  {error}");
            }
        }
    }
    Ok(())
}

fn default_db_path() -> Result<PathBuf> {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).context("APPDATA isn't set; pass --db")?;
    let dir = base.join("RustyAim");
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    Ok(dir.join("rustyaim.db"))
}

fn stats_dir(given: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(dir) = given {
        if !dir.is_dir() {
            bail!("stats folder not found: {}", dir.display());
        }
        return Ok(dir);
    }
    discover::find_stats_dir().context("couldn't find the KovaaK's stats folder in your Steam libraries; pass --stats")
}

fn print_report(r: &ImportReport) {
    println!("Found {} stats files: {} imported, {} already in database, {} errors", r.found, r.imported, r.skipped, r.errors.len());
    for (file, error) in &r.errors {
        println!("  ! {file}: {error}");
    }
}

fn watch(db: &mut Db, dir: &Path) -> Result<()> {
    print_report(&import::import_dir(db, dir)?);
    let (tx, rx) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(tx)?;
    watcher.watch(dir, RecursiveMode::NonRecursive)?;
    println!("Watching {} for new runs (Ctrl+C to stop)", dir.display());

    loop {
        let event = rx.recv()?;
        if !touches_stats_file(&event) {
            continue;
        }
        // KovaaK's may still be writing; wait until events stop for a moment.
        while rx.recv_timeout(Duration::from_millis(750)).is_ok() {}
        let report = import::import_dir(db, dir)?;
        if report.imported > 0 || !report.errors.is_empty() {
            print_report(&report);
        }
    }
}

fn touches_stats_file(event: &notify::Result<notify::Event>) -> bool {
    match event {
        Ok(e) => e.paths.iter().any(|p| import::is_stats_file(p)),
        Err(_) => false,
    }
}

fn print_summary(db: &Db, top: usize) -> Result<()> {
    let t = summary::totals(db)?;
    if t.runs == 0 {
        println!("No runs imported yet. Run `rustyaim import` first.");
        return Ok(());
    }
    let day = |s: &Option<String>| s.as_deref().map(|s| s[..10].to_string()).unwrap_or_default();
    println!("{} runs across {} scenarios, {} to {}", t.runs, t.scenarios, day(&t.first), day(&t.last));
    println!("Time in runs: {}", hm(t.run_seconds));

    println!("\nBy day");
    for d in summary::by_day(db)? {
        println!("  {}  {:>4} runs  {:>7}", d.day, d.runs, hm(d.run_seconds));
    }

    println!("\nMost played");
    for s in summary::top_scenarios(db, top)? {
        let change = match (s.first_score, s.best) {
            (Some(first), Some(best)) if first > 0.0 => format!("{:+.0}%", (best / first - 1.0) * 100.0),
            _ => String::from("–"),
        };
        println!("  {:>3}x  best {:>10}  {:>6}  {}", s.runs, s.best.map(num).unwrap_or_default(), change, s.scenario);
    }
    Ok(())
}

fn print_time(r: &TrainingReport, settings: &TimeSettings, recent: usize) {
    let Some(first) = r.days.first() else {
        println!("No runs imported yet. Run `rustyaim import` first.");
        return;
    };
    let active = r.active_s();
    let session = r.session_s();
    let calendar_days = (r.today - first.day).num_days() + 1;
    let restarted = r.runs.iter().filter(|x| x.restarted).count();

    println!("Since {} ({} days)", first.day.format("%a %b %-d"), calendar_days);
    println!("  Active time    {}  (inside runs)", hm(active));
    println!("  Session time   {}  (first run to last, incl. menus and breaks)", hm(session));
    if session > 0.0 {
        println!("  Active share   {:.0}%", active / session * 100.0);
    }
    println!("  Days trained   {} of {}", r.days.len(), calendar_days);
    println!("  Per day        {} active", hm(active / r.days.len() as f64));
    println!("  Sessions       {}, {} active on average", r.sessions.len(), hm(active / r.sessions.len() as f64));
    println!("  Restarted runs {restarted} of {}", r.runs.len());

    let s = r.streak;
    let today_note = if s.trained_today { "trained today" } else { "not trained yet today" };
    println!("\nStreak: {} days (best {}), {today_note}", s.current, s.best);
    if let Some(w) = r.weeks.last() {
        println!(
            "This week (from {}): {} active over {} days, goal {}",
            w.week_start.format("%a %b %-d"),
            hm(w.active_s),
            w.days_trained,
            hm(settings.daily_goal_min * 60.0 * 7.0)
        );
    }

    println!("\nDays                runs  sessions   active  session  goal ({:.0}m)", settings.daily_goal_min);
    for d in &r.days {
        let goal = if d.goal_met { "met" } else { "-" };
        println!(
            "  {}  {:>4}  {:>8}  {:>7}  {:>7}  {goal}",
            d.day.format("%a %Y-%m-%d"),
            d.runs,
            d.sessions,
            hm(d.active_s),
            hm(d.session_s)
        );
    }

    println!("\nRecent sessions");
    for s in r.sessions.iter().rev().take(recent) {
        let restarts = if s.restarted > 0 { format!(", {} restarted", s.restarted) } else { String::new() };
        println!(
            "  {} {}-{}  {:>3} runs  {} active of {}{restarts}",
            s.start.format("%a %b %-d"),
            s.start.format("%H:%M"),
            s.end.format("%H:%M"),
            s.runs,
            hm(s.active_s),
            hm(s.length_s())
        );
    }

    println!("\nAverage active minutes by weekday (missed days count as 0)");
    for (i, avg) in r.weekday_avg_min.iter().enumerate() {
        let name = chrono::Weekday::try_from(i as u8).map(|w| format!("{w}")).unwrap_or_default();
        match avg {
            Some(m) => println!("  {name}  {m:>5.0}m  {}", bar(*m, 90.0, 30)),
            None => println!("  {name}      -  (hasn't come up yet)"),
        }
    }

    println!("\nRuns by hour started");
    let max = r.runs_by_hour.iter().copied().max().unwrap_or(1).max(1) as f64;
    for (hour, n) in r.runs_by_hour.iter().enumerate().filter(|(_, n)| **n > 0) {
        println!("  {hour:02}:00  {n:>4}  {}", bar(*n as f64, max, 30));
    }
}

fn find_scenario(p: &Progress, query: &str) -> Result<String> {
    let names = p.scenarios();
    if let Some(exact) = names.iter().find(|n| n.eq_ignore_ascii_case(query)) {
        return Ok(exact.clone());
    }
    let q = query.to_lowercase();
    let matches: Vec<&String> = names.iter().filter(|n| n.to_lowercase().contains(&q)).collect();
    match matches.as_slice() {
        [] => bail!("no scenario matches \"{query}\""),
        [one] => Ok((*one).clone()),
        many => {
            let list: Vec<String> = many.iter().map(|n| format!("  {n}")).collect();
            bail!("\"{query}\" matches {} scenarios, be more specific:\n{}", many.len(), list.join("\n"))
        }
    }
}

fn opt_num(v: Option<f64>) -> String {
    v.map(num).unwrap_or_else(|| "-".into())
}

fn opt_pct(v: Option<f64>) -> String {
    v.map(|p| format!("{p:+.0}%")).unwrap_or_else(|| "-".into())
}

fn print_progress_list(p: &Progress, settings: &ProgressSettings, sort: SortBy, top: usize) {
    let mut stats = p.stats(settings);
    if stats.is_empty() {
        println!("No runs imported yet. Run `rustyaim import` first.");
        return;
    }
    match sort {
        SortBy::Played => stats.sort_by(|a, b| b.runs.cmp(&a.runs).then(b.last_played.cmp(&a.last_played))),
        SortBy::Improved => stats.sort_by(|a, b| {
            b.improvement_pct.unwrap_or(f64::NEG_INFINITY).total_cmp(&a.improvement_pct.unwrap_or(f64::NEG_INFINITY))
        }),
        SortBy::Recent => stats.sort_by_key(|s| std::cmp::Reverse(s.last_played)),
        SortBy::Name => stats.sort_by_key(|s| s.scenario.to_lowercase()),
    }

    println!("{} scenarios. Change = first run to best. Trend = last {} runs vs the {} before.", stats.len(), settings.trend_window, settings.trend_window);
    println!("\n  runs        best   change   trend   since PB  scenario");
    for s in stats.iter().take(top) {
        let flag = if s.plateau { "  [plateau]" } else { "" };
        println!(
            "  {:>4}  {:>10}  {:>7}  {:>6}  {:>9}  {}{flag}",
            s.runs,
            opt_num(s.best_score),
            opt_pct(s.improvement_pct),
            opt_pct(s.trend_pct),
            s.runs_since_pb,
            s.scenario
        );
    }
    if stats.len() > top {
        println!("  … {} more (use --top)", stats.len() - top);
    }

    let cold = p.cold_starts();
    if !cold.is_empty() {
        // Median, because one noisy scenario can swing an average a long way.
        let mut changes: Vec<f64> = cold.iter().map(|c| c.vs_previous_best_pct).collect();
        changes.sort_by(f64::total_cmp);
        let median = changes[changes.len() / 2];
        let below = changes.iter().filter(|c| **c < 0.0).count();
        println!(
            "\nCold starts: the first run of the day was below your previous best on {below} of {} return visits (median {median:+.0}%).",
            cold.len()
        );
        let mut vs_rest: Vec<f64> = cold.iter().filter_map(|c| c.vs_rest_of_day_pct).collect();
        if !vs_rest.is_empty() {
            vs_rest.sort_by(f64::total_cmp);
            let below = vs_rest.iter().filter(|c| **c < 0.0).count();
            println!(
                "Warm-up: it was below that day's later runs on {below} of {} (median {:+.0}%).",
                vs_rest.len(),
                vs_rest[vs_rest.len() / 2]
            );
        }
    }

    let fatigue = p.fatigue(settings);
    if !fatigue.is_empty() {
        println!("\nScore vs. minutes into session (100% = that scenario's average)");
        for b in &fatigue {
            println!("  {:>3}-{:<3} min  {:>4} runs  {:>4.0}%  {}", b.from_min, b.to_min, b.runs, b.relative_score_pct, bar(b.relative_score_pct, 150.0, 30));
        }
        println!("  (Mixes in which scenarios you play when, so treat it as a rough signal.)");
    }
}

fn print_scenario(p: &Progress, scenario: &str, settings: &ProgressSettings) {
    let Some(s) = p.stats_for(scenario, settings) else { return };
    println!("{scenario}");
    println!("  Runs           {} ({} counted)", s.runs, s.counted_runs);
    println!("  Played         {} to {}", s.first_played.format("%b %-d"), s.last_played.format("%b %-d"));
    let best_at = s.best_at.map(|t| format!(" on {}", t.format("%b %-d %H:%M"))).unwrap_or_default();
    println!("  Best           {}{best_at}", opt_num(s.best_score));
    println!("  First / last   {} / {}", opt_num(s.first_score), opt_num(s.last_score));
    println!("  Average        {}", opt_num(s.average.map(|a| (a * 10.0).round() / 10.0)));
    println!("  Change         {} (first run to best)", opt_pct(s.improvement_pct));
    println!("  Trend          {}", opt_pct(s.trend_pct));
    println!("  Variation      {}", s.variation_pct.map(|v| format!("{v:.0}% (lower is steadier)")).unwrap_or_else(|| "-".into()));
    println!("  Accuracy       {}", s.recent_accuracy.map(|a| format!("{:.1}% (last {} runs)", a * 100.0, settings.trend_window)).unwrap_or_else(|| "-".into()));
    println!("  Since PB       {} runs{}", s.runs_since_pb, if s.plateau { " (plateau)" } else { "" });

    println!("\n     #  when             score    acc   ttk");
    let mut period = None;
    for (i, r) in p.history(scenario).iter().enumerate() {
        if period.is_some_and(|prev| prev != r.sens_period) {
            let sens = &p.sens_periods[r.sens_period].settings;
            println!("        --- settings changed: {} ---", describe_sens(sens));
        }
        period = Some(r.sens_period);
        let mark = if r.restarted { "restarted" } else if r.is_pb { "PB" } else { "" };
        println!(
            "  {:>4}  {}  {:>8}  {:>5}  {:>4}  {mark}",
            i + 1,
            r.finished_at.format("%a %b %-d %H:%M"),
            opt_num(r.score),
            r.accuracy.map(|a| format!("{:.0}%", a * 100.0)).unwrap_or_else(|| "-".into()),
            r.avg_ttk_s.filter(|t| *t > 0.0).map(|t| format!("{t:.2}")).unwrap_or_else(|| "-".into()),
        );
    }

    let cold: Vec<_> = p.cold_starts().into_iter().filter(|c| c.scenario == scenario).collect();
    if !cold.is_empty() {
        println!("\nFirst run of the day vs. earlier best");
        for c in cold {
            let rest = c.vs_rest_of_day_pct.map(|v| format!(", {v:+.0}% vs. the rest of that day")).unwrap_or_default();
            println!("  {}  {} vs {} ({:+.0}%{rest})", c.day.format("%b %-d"), num(c.first_score), num(c.previous_best), c.vs_previous_best_pct);
        }
    }
}

fn describe_sens(s: &progress::SensSettings) -> String {
    let sens = match (&s.sens_scale, s.horiz_sens) {
        (Some(scale), Some(v)) => format!("{v} {scale}"),
        (None, Some(v)) => format!("{v}"),
        _ => "sens ?".into(),
    };
    let dpi = s.dpi.map(|d| format!("{d} DPI")).unwrap_or_else(|| "DPI ?".into());
    let fov = s.fov.map(|f| format!("FOV {f}")).unwrap_or_else(|| "FOV ?".into());
    let cm = s.cm_per_360().map(|c| format!(", {c:.1} cm/360")).unwrap_or_default();
    format!("{sens}, {dpi}, {fov}{cm}")
}

fn print_sens(p: &Progress) {
    if p.sens_periods.is_empty() {
        println!("No runs imported yet. Run `rustyaim import` first.");
        return;
    }
    println!("Settings periods, oldest first");
    for period in &p.sens_periods {
        println!(
            "  {} to {}  {:>4} runs  {}",
            period.from.format("%b %-d %H:%M"),
            period.to.format("%b %-d %H:%M"),
            period.runs,
            describe_sens(&period.settings)
        );
    }
}

fn bar(value: f64, max: f64, width: usize) -> String {
    "#".repeat(((value / max) * width as f64).round().clamp(0.0, width as f64) as usize)
}

fn hm(seconds: f64) -> String {
    let minutes = (seconds / 60.0).round() as i64;
    format!("{}h {:02}m", minutes / 60, minutes % 60)
}

fn num(v: f64) -> String {
    if v.fract() == 0.0 { format!("{v:.0}") } else { format!("{v:.1}") }
}
