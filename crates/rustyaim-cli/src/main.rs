//! Command-line front end for RustyAim: import KovaaK's stats and print summaries.
//! The desktop UI will sit on the same `rustyaim-core` library.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use notify::{RecursiveMode, Watcher};
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

fn hm(seconds: f64) -> String {
    let minutes = (seconds / 60.0).round() as i64;
    format!("{}h {:02}m", minutes / 60, minutes % 60)
}

fn num(v: f64) -> String {
    if v.fract() == 0.0 { format!("{v:.0}") } else { format!("{v:.1}") }
}
