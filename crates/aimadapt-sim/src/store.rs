//! The player's own TOML files (scenarios, workouts): one folder each, addressed as
//! `user:<file stem>` next to the built-ins' `builtin:<name>`. Writes replace the
//! file in one step, so a crash or a full disk mid-write never leaves half a file.

use std::path::{Path, PathBuf};

/// Writes `text` to a temporary file beside `path`, then renames it over `path`.
pub fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = path.with_file_name(format!(".{name}.tmp"));
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// Every `.toml` file in `dir` as `(stem, path)`, sorted by file name.
pub fn user_files(dir: &Path) -> Vec<(String, PathBuf)> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("toml")))
        .collect();
    files.sort();
    files.into_iter().filter_map(|p| Some((p.file_stem()?.to_str()?.to_string(), p.clone()))).collect()
}

/// The file for `user:<stem>`, refusing stems that would leave the folder.
/// `what` names the kind of file in the error ("scenario", "workout").
pub fn user_path(dir: &Path, stem: &str, what: &str) -> Result<PathBuf, String> {
    if stem.is_empty() || stem.contains(['/', '\\', ':']) || stem.starts_with('.') {
        return Err(format!("bad {what} id {stem}"));
    }
    Ok(dir.join(format!("{stem}.toml")))
}

/// Reads a user file by id (`user:<stem>`) or by path.
pub fn read_user(dir: &Path, id: &str, what: &str) -> Result<(PathBuf, String), String> {
    let path = match id.strip_prefix("user:") {
        Some(stem) => user_path(dir, stem, what)?,
        None => PathBuf::from(id),
    };
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok((path, text))
}

/// Saves `text` and returns its id. `replace` is the id of the user file being
/// edited; otherwise a new file is named after `name` (with a number added if the
/// name is taken).
pub fn save_user(dir: &Path, name: &str, text: &str, replace: Option<&str>, what: &str) -> Result<String, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let stem = match replace.and_then(|id| id.strip_prefix("user:")) {
        Some(stem) => stem.to_string(),
        None => {
            let base = slug(name);
            let mut stem = base.clone();
            let mut n = 2;
            while dir.join(format!("{stem}.toml")).exists() {
                stem = format!("{base}-{n}");
                n += 1;
            }
            stem
        }
    };
    write_atomic(&user_path(dir, &stem, what)?, text).map_err(|e| e.to_string())?;
    Ok(format!("user:{stem}"))
}

pub fn delete_user(dir: &Path, id: &str, what: &str) -> Result<(), String> {
    let stem = id.strip_prefix("user:").ok_or(format!("only your own {what}s can be deleted"))?;
    std::fs::remove_file(user_path(dir, stem, what)?).map_err(|e| e.to_string())
}

/// Lowercase words joined by dashes, safe as a file name.
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() { "scenario".into() } else { out }
}

/// Checks a hand-written table of built-ins against its folder: every file listed
/// once, under its own name, with its own text.
#[cfg(test)]
pub(crate) fn check_builtins(folder: &str, builtin: &[(&str, &str)]) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(folder);
    let mut files: Vec<String> = user_files(&dir).into_iter().map(|(stem, _)| stem).collect();
    let mut listed: Vec<String> = builtin.iter().map(|(id, _)| id.to_string()).collect();
    files.sort();
    listed.sort();
    assert_eq!(listed, files, "the built-in table and {folder}/ list different files");
    for (id, text) in builtin {
        let on_disk = std::fs::read_to_string(dir.join(format!("{id}.toml"))).unwrap();
        assert_eq!(*text, on_disk, "{id} is listed with another file's text");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Files were written in place with `std::fs::write`, so a crash or a full disk
    /// mid-write left a cut-off file (and a profile file that fails to parse was
    /// replaced with defaults). That can't be staged in a test; this checks the
    /// replacement itself: the new text lands whole and no temporary file is left.
    #[test]
    fn writes_replace_the_file_and_leave_no_temporary() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.toml");
        write_atomic(&path, "one").unwrap();
        write_atomic(&path, "two").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "two");
        let names: Vec<_> = std::fs::read_dir(dir.path()).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(names, vec![std::ffi::OsString::from("a.toml")]);
    }

    #[test]
    fn ids_cant_leave_the_folder() {
        let dir = Path::new("x");
        for bad in ["", "../a", "a/b", "a\\b", "c:a", ".hidden"] {
            assert!(user_path(dir, bad, "scenario").is_err(), "{bad}");
        }
        assert_eq!(user_path(dir, "ok", "scenario").unwrap(), dir.join("ok.toml"));
    }
}
