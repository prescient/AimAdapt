//! Parser for KovaaK's per-run stats files (`<Scenario> - <Mode> - <timestamp> Stats.csv`).
//!
//! A file has three kinds of content, separated by blank lines:
//! - a kill table (`Kill #,Timestamp,...`), empty for tracking scenarios
//! - a weapon table (`Weapon,Shots,Hits,...`)
//! - `Key:,value` lines: run summary first, then the settings used
//!
//! Parsing is deliberately tolerant: columns are looked up by header name,
//! missing values become `None`, and every `Key:,value` pair is kept in
//! [`RunFile::fields`] so nothing is lost when a game update adds fields.

use std::collections::BTreeMap;
use std::path::Path;

use chrono::{Duration, NaiveDateTime, NaiveTime};

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("file name doesn't match `<Scenario> - <Mode> - YYYY.MM.DD-HH.MM.SS Stats.csv`: {0}")]
    FileName(String),
    #[error("couldn't read file: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FileNameInfo {
    pub scenario: String,
    pub mode: String,
    /// Local time the run finished (KovaaK's writes the file at the end of the run).
    pub finished_at: NaiveDateTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Kill {
    pub kill_no: u32,
    pub timestamp: Option<String>,
    pub bot: Option<String>,
    pub weapon: Option<String>,
    pub ttk_s: Option<f64>,
    pub shots: Option<i64>,
    pub hits: Option<i64>,
    pub damage_done: Option<f64>,
    pub damage_possible: Option<f64>,
    pub cheated: Option<bool>,
    pub overshots: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WeaponTotal {
    pub weapon: String,
    pub shots: Option<i64>,
    pub hits: Option<i64>,
    pub damage_done: Option<f64>,
    pub damage_possible: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RunFile {
    pub file_name: String,
    pub scenario: String,
    pub mode: String,
    pub finished_at: NaiveDateTime,
    /// Every `Key:,value` line, keyed without the trailing colon.
    pub fields: BTreeMap<String, String>,
    pub kills: Vec<Kill>,
    pub weapons: Vec<WeaponTotal>,
}

impl RunFile {
    pub fn field(&self, key: &str) -> Option<&str> {
        self.fields.get(key).map(String::as_str).filter(|v| !v.is_empty())
    }

    pub fn field_f64(&self, key: &str) -> Option<f64> {
        self.field(key).and_then(|v| v.parse().ok())
    }

    pub fn field_i64(&self, key: &str) -> Option<i64> {
        self.field(key).and_then(|v| v.parse::<i64>().ok().or_else(|| v.parse::<f64>().ok().map(|f| f as i64)))
    }

    pub fn score(&self) -> Option<f64> {
        self.field_f64("Score")
    }

    /// Hit Count / (Hit Count + Miss Count). Works for tracking too, where hits are ticks.
    pub fn accuracy(&self) -> Option<f64> {
        let hits = self.field_f64("Hit Count")?;
        let misses = self.field_f64("Miss Count")?;
        let total = hits + misses;
        (total > 0.0).then(|| hits / total)
    }

    /// When the run started. `Challenge Start` is only a time of day, so a start
    /// later than the finish time means the run crossed midnight.
    pub fn started_at(&self) -> Option<NaiveDateTime> {
        let time = parse_time_of_day(self.field("Challenge Start")?)?;
        let start = self.finished_at.date().and_time(time);
        Some(if start > self.finished_at { start - Duration::days(1) } else { start })
    }

    /// Wall-clock length of the run in seconds (includes pauses).
    pub fn duration_s(&self) -> Option<f64> {
        let start = self.started_at()?;
        let ms = (self.finished_at - start).num_milliseconds();
        // The file name only has whole seconds, so a run can come out a hair negative.
        Some((ms as f64 / 1000.0).max(0.0))
    }
}

pub fn parse_file(path: &Path) -> Result<RunFile, ParseError> {
    let file_name =
        path.file_name().and_then(|n| n.to_str()).ok_or_else(|| ParseError::FileName(path.display().to_string()))?;
    let bytes = std::fs::read(path)?;
    parse_contents(file_name, &String::from_utf8_lossy(&bytes))
}

pub fn parse_contents(file_name: &str, contents: &str) -> Result<RunFile, ParseError> {
    let info = parse_file_name(file_name)?;
    let mut fields = BTreeMap::new();
    let mut kills = Vec::new();
    let mut weapons = Vec::new();

    // Which table the following rows belong to, with its header columns.
    let mut table: Option<(Table, Vec<String>)> = None;

    for line in contents.lines() {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() {
            table = None;
            continue;
        }
        if line.starts_with("Kill #,") {
            table = Some((Table::Kills, split(line)));
            continue;
        }
        if line.starts_with("Weapon,") {
            table = Some((Table::Weapons, split(line)));
            continue;
        }
        if let Some((key, value)) = key_value(line) {
            table = None;
            fields.insert(key.to_string(), value.trim().to_string());
            continue;
        }
        match &table {
            Some((Table::Kills, header)) => {
                if let Some(kill) = parse_kill(header, &split(line)) {
                    kills.push(kill);
                }
            }
            Some((Table::Weapons, header)) => {
                if let Some(weapon) = parse_weapon(header, &split(line)) {
                    weapons.push(weapon);
                }
            }
            None => {}
        }
    }

    Ok(RunFile {
        file_name: file_name.to_string(),
        scenario: info.scenario,
        mode: info.mode,
        finished_at: info.finished_at,
        fields,
        kills,
        weapons,
    })
}

/// Splits `<Scenario> - <Mode> - 2026.11.03-06.31.39 Stats.csv`. Scenario names can
/// themselves contain ` - `, so the mode and timestamp are taken from the right.
pub fn parse_file_name(file_name: &str) -> Result<FileNameInfo, ParseError> {
    let err = || ParseError::FileName(file_name.to_string());
    let stem = file_name.strip_suffix(" Stats.csv").ok_or_else(err)?;
    let mut parts = stem.rsplitn(3, " - ");
    let stamp = parts.next().ok_or_else(err)?;
    let mode = parts.next().ok_or_else(err)?;
    let scenario = parts.next().ok_or_else(err)?;
    if scenario.is_empty() {
        return Err(err());
    }
    let finished_at = NaiveDateTime::parse_from_str(stamp, "%Y.%m.%d-%H.%M.%S").map_err(|_| err())?;
    Ok(FileNameInfo { scenario: scenario.to_string(), mode: mode.to_string(), finished_at })
}

#[derive(Debug, Clone, Copy)]
enum Table {
    Kills,
    Weapons,
}

fn split(line: &str) -> Vec<String> {
    line.split(',').map(|s| s.trim().to_string()).collect()
}

/// `Key:,value` lines. Table rows never have a colon right before the first comma.
fn key_value(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once(":,")?;
    (!key.is_empty() && !key.contains(',')).then_some((key, value))
}

fn column<'a>(header: &[String], row: &'a [String], name: &str) -> Option<&'a str> {
    let idx = header.iter().position(|h| h == name)?;
    row.get(idx).map(String::as_str).filter(|v| !v.is_empty())
}

fn parse_kill(header: &[String], row: &[String]) -> Option<Kill> {
    let col = |name| column(header, row, name);
    Some(Kill {
        kill_no: col("Kill #")?.parse().ok()?,
        timestamp: col("Timestamp").map(str::to_string),
        bot: col("Bot").map(str::to_string),
        weapon: col("Weapon").map(str::to_string),
        ttk_s: col("TTK").and_then(|v| v.trim_end_matches('s').parse().ok()),
        shots: col("Shots").and_then(|v| v.parse().ok()),
        hits: col("Hits").and_then(|v| v.parse().ok()),
        damage_done: col("Damage Done").and_then(|v| v.parse().ok()),
        damage_possible: col("Damage Possible").and_then(|v| v.parse().ok()),
        cheated: col("Cheated").map(|v| v != "0" && !v.eq_ignore_ascii_case("false")),
        overshots: col("OverShots").and_then(|v| v.parse().ok()),
    })
}

fn parse_weapon(header: &[String], row: &[String]) -> Option<WeaponTotal> {
    let col = |name| column(header, row, name);
    Some(WeaponTotal {
        weapon: col("Weapon")?.to_string(),
        shots: col("Shots").and_then(|v| v.parse::<f64>().ok()).map(|v| v as i64),
        hits: col("Hits").and_then(|v| v.parse::<f64>().ok()).map(|v| v as i64),
        damage_done: col("Damage Done").and_then(|v| v.parse().ok()),
        damage_possible: col("Damage Possible").and_then(|v| v.parse().ok()),
    })
}

fn parse_time_of_day(value: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(value, "%H:%M:%S%.f").ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn file_name_with_dashes_in_scenario() {
        let info =
            parse_file_name("4BK - Horizontalish Pressure Shot 1 bullet - Challenge - 2026.11.03-06.29.11 Stats.csv")
                .unwrap();
        assert_eq!(info.scenario, "4BK - Horizontalish Pressure Shot 1 bullet");
        assert_eq!(info.mode, "Challenge");
        assert_eq!(info.finished_at, date(2026, 11, 3).and_hms_opt(6, 29, 11).unwrap());
    }

    #[test]
    fn rejects_other_files() {
        assert!(parse_file_name("notes.txt").is_err());
        assert!(parse_file_name("Challenge - 2026.11.03-06.29.11 Stats.csv").is_err());
        assert!(parse_file_name("X - Challenge - not a date Stats.csv").is_err());
    }

    #[test]
    fn run_crossing_midnight() {
        let csv = "Score:,10.0\nChallenge Start:,23:59:30.500\n";
        let run = parse_contents("X - Challenge - 2026.11.04-00.00.30 Stats.csv", csv).unwrap();
        assert_eq!(run.started_at().unwrap().date(), date(2026, 11, 3));
        assert!((run.duration_s().unwrap() - 59.5).abs() < 1e-9);
    }

    #[test]
    fn keeps_unknown_fields_and_ignores_bad_values() {
        let csv = "Score:,abc\nSome New Field:,42\n";
        let run = parse_contents("X - Challenge - 2026.11.04-00.00.30 Stats.csv", csv).unwrap();
        assert_eq!(run.score(), None);
        assert_eq!(run.field("Some New Field"), Some("42"));
        assert_eq!(run.accuracy(), None);
        assert_eq!(run.duration_s(), None);
    }
}
