//! Incremental import of a stats folder into the database.

use std::path::{Path, PathBuf};

use chrono::{Local, NaiveDateTime};

use crate::db::{Db, DbError};
use crate::parse::{self, RunFile};

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("couldn't read stats folder {path}: {source}")]
    ReadDir { path: PathBuf, source: std::io::Error },
    #[error(transparent)]
    Db(#[from] DbError),
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct ImportReport {
    /// Stats files found in the folder.
    pub found: usize,
    /// Newly added to the database.
    pub imported: usize,
    /// Already in the database.
    pub skipped: usize,
    /// Files that couldn't be parsed, with the reason.
    pub errors: Vec<(String, String)>,
}

pub fn is_stats_file(path: &Path) -> bool {
    path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.ends_with(" Stats.csv"))
}

/// Imports every stats file in `dir` that isn't in the database yet.
/// Safe to call repeatedly: already-imported runs are skipped by file name.
pub fn import_dir(db: &mut Db, dir: &Path) -> Result<ImportReport, ImportError> {
    import_dir_at(db, dir, Local::now().naive_local())
}

pub fn import_dir_at(db: &mut Db, dir: &Path, now: NaiveDateTime) -> Result<ImportReport, ImportError> {
    let read_err = |source| ImportError::ReadDir { path: dir.to_path_buf(), source };
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(read_err)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file() && is_stats_file(p))
        .collect();
    files.sort();

    let known = db.known_file_names()?;
    let mut report = ImportReport { found: files.len(), ..Default::default() };
    let mut parsed: Vec<RunFile> = Vec::new();

    for path in &files {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
        if known.contains(&name) {
            report.skipped += 1;
            continue;
        }
        match parse::parse_file(path) {
            Ok(run) => {
                db.clear_error(&name)?;
                parsed.push(run);
            }
            Err(e) => {
                db.record_error(&name, &e.to_string(), now)?;
                report.errors.push((name, e.to_string()));
            }
        }
    }

    report.imported = db.insert_runs(&parsed, now)?;
    report.skipped += parsed.len() - report.imported;
    Ok(report)
}
