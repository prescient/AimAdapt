//! AimAdapt core: parsing KovaaK's stats files, storing them in SQLite, and the
//! stats built on top. No UI code lives here so everything can be tested directly.

pub mod db;
pub mod discover;
pub mod import;
pub mod insights;
pub mod parse;
pub mod programs;
pub mod progress;
pub mod replay;
pub mod summary;
pub mod telemetry;
pub mod trainer_runs;
pub mod training;

pub use db::Db;
pub use import::{ImportReport, import_dir};
pub use parse::{RunFile, parse_file};
