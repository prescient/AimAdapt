//! Command-line front end for RustyAim: import KovaaK's stats and print summaries.
//! The desktop UI will sit on the same `rustyaim-core` library.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use chrono::Local;
use clap::{Parser, Subcommand};
use notify::{RecursiveMode, Watcher};
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
    /// List files that couldn't be imported
    Errors,
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
