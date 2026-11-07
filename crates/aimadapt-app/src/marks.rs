//! Favorites and hidden items on the Scenarios page, kept in `list.json` in the
//! data folder. Items are named by the list's keys: a trainer scenario's id
//! (`builtin:static-grid`), `workout:<id>`, or `kovaaks:<scenario name>`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Marks {
    /// In the order they were starred.
    pub favorites: Vec<String>,
    pub hidden: Vec<String>,
}

impl Marks {
    /// Drops blanks and repeats, keeping the first of each.
    fn tidy(mut self) -> Self {
        for list in [&mut self.favorites, &mut self.hidden] {
            let mut seen = std::collections::HashSet::new();
            list.retain(|k| !k.trim().is_empty() && seen.insert(k.clone()));
        }
        self
    }
}

pub fn path(data_dir: &Path) -> PathBuf {
    data_dir.join("list.json")
}

/// The saved marks; none if the file is missing or can't be read, so a damaged
/// file never stops the page from loading.
pub fn load(data_dir: &Path) -> Marks {
    std::fs::read_to_string(path(data_dir))
        .ok()
        .and_then(|t| serde_json::from_str::<Marks>(&t).ok())
        .unwrap_or_default()
        .tidy()
}

pub fn save(data_dir: &Path, marks: Marks) -> Result<Marks, String> {
    let marks = marks.tidy();
    std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(&marks).map_err(|e| e.to_string())?;
    std::fs::write(path(data_dir), json).map_err(|e| e.to_string())?;
    Ok(marks)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("aimadapt-marks-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn no_file_means_no_marks() {
        assert_eq!(load(&temp_dir("none")), Marks::default());
    }

    #[test]
    fn marks_round_trip_without_repeats_or_blanks() {
        let d = temp_dir("trip");
        let m = Marks {
            favorites: vec![
                "builtin:static-grid".into(),
                "workout:mine".into(),
                "builtin:static-grid".into(),
                " ".into(),
            ],
            hidden: vec!["kovaaks:Old One".into()],
        };
        let saved = save(&d, m).unwrap();
        assert_eq!(saved.favorites, vec!["builtin:static-grid".to_string(), "workout:mine".to_string()]);
        assert_eq!(load(&d), saved);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_damaged_or_partial_file_still_loads() {
        let d = temp_dir("bad");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(path(&d), "{not json").unwrap();
        assert_eq!(load(&d), Marks::default());
        // A file from before a list existed only lacks that list.
        std::fs::write(path(&d), r#"{"favorites":["builtin:wide-flicks"]}"#).unwrap();
        assert_eq!(load(&d), Marks { favorites: vec!["builtin:wide-flicks".into()], hidden: vec![] });
        let _ = std::fs::remove_dir_all(&d);
    }
}
