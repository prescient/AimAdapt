//! Command-line front end for AimAdapt: import KovaaK's stats and print summaries.
//! The desktop UI will sit on the same `aimadapt-core` library.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use aimadapt_core::plan_check::PlanCheck;
use aimadapt_core::progress::{self, Progress, ProgressSettings};
use aimadapt_core::training::{self, TimeSettings, TrainingReport};
use aimadapt_core::{Db, ImportReport, discover, import, summary};
use anyhow::{Context, Result, bail};
use chrono::Local;
use clap::{Parser, Subcommand};
use notify::{RecursiveMode, Watcher};

#[derive(Parser)]
#[command(name = "aimadapt", about = "Track your KovaaK's aim training")]
struct Cli {
    /// Database file. Defaults to %APPDATA%\AimAdapt\rustyaim.db
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
    /// Planned against actual success by kind of run, for all scenarios or one
    PlanCheck {
        /// Scenario name, or part of it. Omit for all scenarios.
        scenario: Option<String>,
    },
    /// Sensitivity and FOV changes over time
    Sens,
    /// List files that couldn't be imported
    Errors,
    /// Read KovaaK's scenario files (.sce) and list which features each uses that the
    /// trainer can't play yet. Only reads them; nothing is changed or stored.
    Sce {
        /// Only scenarios whose name contains this (any case), e.g. "Novice S5"
        filter: Option<String>,
        /// Folders or .sce files to read. Defaults to your KovaaK's scenario and
        /// Workshop folders in every Steam library.
        #[arg(long = "from")]
        from: Vec<PathBuf>,
        /// Only print the totals and the features most often missing
        #[arg(long)]
        summary: bool,
    },
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
    // Reading scenario files needs no database.
    if let Command::Sce { filter, from, summary } = &cli.command {
        return print_sce(filter.as_deref(), from, *summary);
    }
    let db_path = match cli.db {
        Some(p) => p,
        None => aimadapt_core::db::default_path().context("finding the default database location; pass --db")?,
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
            let settings = TimeSettings {
                session_gap_min: gap,
                day_start_hour: day_start,
                daily_goal_min: goal,
                ..Default::default()
            };
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
        Command::PlanCheck { scenario } => {
            let scenario = match scenario {
                Some(query) => Some(find_scenario(&progress::load(&db, &TimeSettings::default())?, &query)?),
                None => None,
            };
            print!("{}", plan_check_text(&aimadapt_core::plan_check::by_kind(&db, scenario.as_deref())?));
        }
        Command::Sens => print_sens(&progress::load(&db, &TimeSettings::default())?),
        // Handled before the database is opened.
        Command::Sce { .. } => {}
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

fn stats_dir(given: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(dir) = given {
        if !dir.is_dir() {
            bail!("stats folder not found: {}", dir.display());
        }
        return Ok(dir);
    }
    discover::find_stats_dir().context("couldn't find the KovaaK's stats folder in your Steam libraries; pass --stats")
}

/// Whether the trainer plays a scenario file.
enum Status {
    /// It plays, as this.
    Plays(Box<aimadapt_sce::Imported>),
    /// Covered by the plan, but the importer refuses it, for these reasons.
    Refused(Vec<aimadapt_sce::Gap>),
    /// It uses features outside the plan.
    Gaps(Vec<aimadapt_sce::Gap>),
}

fn status(path: &Path) -> Status {
    match aimadapt_sce::import(path) {
        Ok(i) => Status::Plays(Box::new(i)),
        Err(reasons) if reasons.iter().any(|g| g.beyond_plan) => {
            Status::Gaps(reasons.into_iter().filter(|g| g.beyond_plan).collect())
        }
        Err(reasons) => Status::Refused(reasons),
    }
}

fn print_sce(filter: Option<&str>, from: &[PathBuf], summary_only: bool) -> Result<()> {
    if let Some(missing) = from.iter().find(|p| !p.exists()) {
        bail!("{} doesn't exist", missing.display());
    }
    let dirs = if from.is_empty() { discover::find_scenario_dirs() } else { from.to_vec() };
    if dirs.is_empty() {
        bail!("couldn't find KovaaK's scenario folders in your Steam libraries; pass --from");
    }
    let files: Vec<PathBuf> = dirs
        .iter()
        .flat_map(|d| if d.is_file() { vec![d.clone()] } else { aimadapt_sce::find_files(std::slice::from_ref(d)) })
        .collect();
    let needle = filter.map(str::to_lowercase);
    let mut rows = Vec::new();
    let mut unreadable = Vec::new();
    for f in &files {
        match aimadapt_sce::Sce::read(f) {
            Ok(sce) => {
                let name = if sce.name().is_empty() {
                    f.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
                } else {
                    sce.name().to_string()
                };
                if needle.as_ref().is_some_and(|n| !name.to_lowercase().contains(n)) {
                    continue;
                }
                rows.push((name, status(f)));
            }
            Err(e) => unreadable.push(format!("{}: {e}", f.display())),
        }
    }
    rows.sort_by_key(|(n, _)| n.to_lowercase());
    if !summary_only {
        for (name, status) in &rows {
            match status {
                Status::Plays(i) => {
                    println!("  >>  {name}");
                    println!("        {}", describe_import(&i.scenario));
                    for n in &i.notes {
                        println!("        note: {n}");
                    }
                }
                Status::Refused(reasons) => {
                    println!("  !!  {name}");
                    for r in reasons {
                        println!("        {r}");
                    }
                }
                Status::Gaps(gaps) => {
                    println!("  --  {name}");
                    for g in gaps {
                        println!("        {}: {}", g.feature, g.detail);
                    }
                }
            }
        }
        println!();
    }
    let playable = rows.iter().filter(|(_, s)| matches!(s, Status::Plays(_))).count();
    let refused = rows.iter().filter(|(_, s)| matches!(s, Status::Refused(_))).count();
    let gapped = rows.len() - playable - refused;
    println!(
        "{} scenarios: {playable} play in the trainer now (>>), {refused} are covered by the plan but don't play yet (!!), {gapped} use features it won't have yet (--)",
        rows.len(),
    );
    // Each reason once per scenario; refusals (!!) and gaps (--) alike.
    let mut by_feature: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for (_, s) in &rows {
        let (Status::Gaps(reasons) | Status::Refused(reasons)) = s else { continue };
        let seen: std::collections::BTreeSet<&str> = reasons.iter().map(|g| g.feature).collect();
        for f in seen {
            *by_feature.entry(f).or_default() += 1;
        }
    }
    let mut by_feature: Vec<_> = by_feature.into_iter().collect();
    by_feature.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    for (f, n) in by_feature {
        println!("  {n:>4}  {f}");
    }
    for u in &unreadable {
        println!("  ! {u}");
    }
    Ok(())
}

fn print_report(r: &ImportReport) {
    println!(
        "Found {} stats files: {} imported, {} already in database, {} errors",
        r.found,
        r.imported,
        r.skipped,
        r.errors.len()
    );
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
        println!("No runs imported yet. Run `aimadapt import` first.");
        return Ok(());
    }
    let day = |s: &Option<String>| s.as_deref().map(|s| s.get(..10).unwrap_or(s).to_string()).unwrap_or_default();
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
        println!("No runs imported yet. Run `aimadapt import` first.");
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

/// The plan check as a table: Kind, Runs, Planned, Actual, Gap.
fn plan_check_text(plan: &PlanCheck) -> String {
    use std::fmt::Write;
    let mut out = String::from("Planned against actual success, by kind of run\n\n");
    if plan.kinds.iter().all(|k| k.runs == 0) {
        out.push_str(
            "No planned runs yet. A calibration run has no plan, so this starts with the next run after it.\n",
        );
    } else {
        let _ = writeln!(out, "{:<14} {:>5} {:>8} {:>7}  Gap", "Kind", "Runs", "Planned", "Actual");
        for k in &plan.kinds {
            let label = match k.kind.as_str() {
                "near" => "At your level",
                "stretch" => "Stretch",
                _ => "Easy",
            };
            let pct = |v: Option<f64>| v.map_or("-".to_string(), |v| format!("{:.0}%", v * 100.0));
            let gap = match (k.gap, k.verdict.as_deref()) {
                (Some(g), verdict) => {
                    let points = (g * 100.0).round();
                    let text = format!("{}{points:.0} pts", if points > 0.0 { "+" } else { "" });
                    match verdict {
                        Some("above") => format!("{text} above plan"),
                        Some("below") => format!("{text} below plan"),
                        _ => text,
                    }
                }
                _ => "-".to_string(),
            };
            let _ = writeln!(out, "{label:<14} {:>5} {:>8} {:>7}  {gap}", k.runs, pct(k.planned), pct(k.actual));
        }
        out.push_str(
            "\nA gap is marked when it is bigger than luck explains for that many targets, and never under five points.\n",
        );
    }
    if plan.uncounted > 0 {
        let n = plan.uncounted;
        let (count, verb) =
            if n == 1 { ("1 earlier run".to_string(), "is") } else { (format!("{n} earlier runs"), "are") };
        let _ = writeln!(out, "{count} {verb} left out because their length was not recorded.");
    }
    out
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
        println!("No runs imported yet. Run `aimadapt import` first.");
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

    println!(
        "{} scenarios. Change = first run to best. Trend = last {} runs vs the {} before.",
        stats.len(),
        settings.trend_window,
        settings.trend_window
    );
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
            println!(
                "  {:>3}-{:<3} min  {:>4} runs  {:>4.0}%  {}",
                b.from_min,
                b.to_min,
                b.runs,
                b.relative_score_pct,
                bar(b.relative_score_pct, 150.0, 30)
            );
        }
        println!("  (Mixes in which scenarios you play when, so treat it as a rough signal.)");
    }
}

/// One line on an imported scenario: its bots, where they appear, and how it scores.
fn describe_import(s: &aimadapt_sim::Scenario) -> String {
    let t = &s.targets;
    let Some(w) = &s.world else { return String::new() };
    use aimadapt_sim::Vec3;
    // A volume counts from its center.
    let center = |p: &aimadapt_sim::world::WorldSpawn| {
        Vec3::from_array(p.pos) + p.edges.iter().map(|e| Vec3::from_array(*e) / 2.0).sum::<Vec3>()
    };
    let dist: Vec<f32> = w.spawns.iter().map(|p| center(p).length()).collect();
    let (near, far) = dist.iter().fold((f32::MAX, 0.0f32), |(a, b), d| (a.min(*d), b.max(*d)));
    let size = 2.0 * (t.radius / near.max(0.01)).atan().to_degrees();
    let volumes = w.spawns.iter().filter(|p| p.edges.iter().flatten().any(|v| *v != 0.0)).count();
    let moves = match (&w.motion, w.kinds.len()) {
        (_, k) if k > 1 => {
            let moving = w.kinds.iter().filter(|k| k.motion.is_some()).count();
            format!("{k} kinds of bot ({moving} moving)")
        }
        (Some(m), _) => {
            format!("moving up to {:.1} m/s{}", m.max_speed, if m.flight.is_some() { ", flying" } else { "" })
        }
        (None, _) => "standing still".into(),
    };
    format!(
        "{} × {:?} r {:.2} m, {:.0} hp, {moves}, {} spawn points ({volumes} volumes) {:.1}–{:.1} m away ({:.2}° wide at the nearest), offset {:?}–{:?}, apart {:.2} m; {:.0} shots/s {:?}; score {}/kill {}/hit × {:?}",
        t.count,
        t.shape,
        t.radius,
        t.health,
        w.spawns.len(),
        near,
        far,
        size,
        w.offset_min,
        w.offset_max,
        w.blocked_radius,
        s.weapon.fire_rate,
        s.weapon.trigger,
        s.scoring.per_kill,
        s.scoring.per_hit,
        w.accuracy_mult
    )
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
    println!(
        "  Variation      {}",
        s.variation_pct.map(|v| format!("{v:.0}% (lower is steadier)")).unwrap_or_else(|| "-".into())
    );
    println!(
        "  Accuracy       {}",
        s.recent_accuracy
            .map(|a| format!("{:.1}% (last {} runs)", a * 100.0, settings.trend_window))
            .unwrap_or_else(|| "-".into())
    );
    println!("  Since PB       {} runs{}", s.runs_since_pb, if s.plateau { " (plateau)" } else { "" });

    println!("\n     #  when             score    acc   ttk");
    let mut period = None;
    for (i, r) in p.history(scenario).iter().enumerate() {
        if period.is_some_and(|prev| prev != r.sens_period) {
            let sens = &p.sens_periods[r.sens_period].settings;
            println!("        --- settings changed: {} ---", describe_sens(sens));
        }
        period = Some(r.sens_period);
        let mark = if r.restarted {
            "restarted"
        } else if r.is_pb {
            "PB"
        } else {
            ""
        };
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
            println!(
                "  {}  {} vs {} ({:+.0}%{rest})",
                c.day.format("%b %-d"),
                num(c.first_score),
                num(c.previous_best),
                c.vs_previous_best_pct
            );
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
        println!("No runs imported yet. Run `aimadapt import` first.");
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

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../aimadapt-sce/tests/fixtures/AimAdapt Test - {name}.sce"))
    }

    /// Bug: `aimadapt sce` threw away why the importer refused a file with no gaps,
    /// printed it as "ok" and counted it as covered.
    #[test]
    fn a_refused_file_says_why() {
        match status(&fixture("Drifting Pair")) {
            Status::Refused(reasons) => {
                assert!(reasons.iter().any(|r| r.feature == "bots that use abilities"), "{reasons:?}");
            }
            _ => panic!("expected a refusal with reasons"),
        }
        assert!(matches!(status(&fixture("Still Wall")), Status::Plays(_)));
        // Features beyond the plan come back as gaps, without the planned reasons.
        match status(&fixture("Old Room")) {
            Status::Gaps(gaps) => assert!(gaps.iter().all(|g| g.beyond_plan), "{gaps:?}"),
            _ => panic!("expected gaps"),
        }
    }

    fn kind(
        kind: &str,
        runs: usize,
        planned: Option<f64>,
        actual: Option<f64>,
        verdict: Option<&str>,
    ) -> aimadapt_core::plan_check::KindStat {
        aimadapt_core::plan_check::KindStat {
            kind: kind.into(),
            runs,
            evidence: runs as f64 * 40.0,
            planned,
            actual,
            gap: planned.zip(actual).map(|(p, a)| a - p),
            verdict: verdict.map(String::from),
        }
    }

    /// Plan A6: the plan-check table lists the three kinds in order, marks a gap that
    /// luck does not explain, and says how many runs were left out.
    #[test]
    fn the_plan_check_prints_a_row_per_kind_and_marks_big_gaps() {
        let plan = PlanCheck {
            kinds: vec![
                kind("near", 4, Some(0.85), Some(0.88), Some("as_planned")),
                kind("stretch", 3, Some(0.55), Some(0.43), Some("below")),
                kind("easy", 0, None, None, None),
            ],
            uncounted: 2,
        };
        let text = plan_check_text(&plan);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "Planned against actual success, by kind of run");
        assert!(lines[2].starts_with("Kind") && lines[2].ends_with("Gap"), "{text}");
        assert!(lines[3].starts_with("At your level") && lines[3].ends_with("+3 pts"), "{text}");
        assert!(lines[4].starts_with("Stretch") && lines[4].ends_with("-12 pts below plan"), "{text}");
        assert!(lines[5].starts_with("Easy") && lines[5].contains(" 0 ") && lines[5].ends_with('-'), "{text}");
        assert!(text.contains("2 earlier runs are left out because their length was not recorded."), "{text}");
        let one = plan_check_text(&PlanCheck { uncounted: 1, ..plan });
        assert!(one.contains("1 earlier run is left out"), "{one}");
    }

    #[test]
    fn the_plan_check_with_no_planned_runs_says_so() {
        let plan = PlanCheck { kinds: vec![kind("near", 0, None, None, None)], uncounted: 0 };
        assert!(plan_check_text(&plan).contains("No planned runs yet."));
    }

    /// Bug: `--from` a folder that doesn't exist found nothing and still succeeded.
    #[test]
    fn a_missing_from_folder_is_an_error() {
        let err = print_sce(None, &[PathBuf::from("no such folder")], true).unwrap_err();
        assert!(err.to_string().contains("doesn't exist"), "{err}");
    }
}
