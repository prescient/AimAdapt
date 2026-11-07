//! The settings part of a `.sce` file: `key=value` lines, first for the scenario
//! itself, then in repeated sections such as `[Bot Profile]` or `[Character Profile]`,
//! each named by its `Name=` line. Everything after the `[Map Data]` line is the map.
//!
//! Keys are matched without regard to case, because KovaaK's versions spell some
//! differently (`FlickFOV` and `FlickFov`). When a key appears twice the last one wins.

/// One block of settings: the scenario's own (`kind` is empty) or a named profile.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Section {
    /// The header without brackets, e.g. "Bot Profile".
    pub kind: String,
    entries: Vec<(String, String)>,
}

impl Section {
    pub fn new(kind: &str) -> Self {
        Section { kind: kind.to_string(), entries: Vec::new() }
    }

    /// Adds a setting, as read from the file.
    pub fn push(&mut self, key: &str, value: &str) {
        self.entries.push((key.to_string(), value.to_string()));
    }

    /// The raw value of a key, if present.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.iter().rev().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v.as_str())
    }

    /// The value, or "" when absent.
    pub fn str(&self, key: &str) -> &str {
        self.get(key).unwrap_or("")
    }

    /// The `Name=` line.
    pub fn name(&self) -> &str {
        self.str("Name")
    }

    pub fn f32(&self, key: &str) -> Option<f32> {
        self.get(key).and_then(|v| v.trim().parse().ok())
    }

    pub fn f32_or(&self, key: &str, default: f32) -> f32 {
        self.f32(key).unwrap_or(default)
    }

    pub fn bool(&self, key: &str) -> Option<bool> {
        match self.get(key)?.trim().to_ascii_lowercase().as_str() {
            "true" | "1" => Some(true),
            "false" | "0" => Some(false),
            _ => None,
        }
    }

    /// True only when the key is present and true.
    pub fn is(&self, key: &str) -> bool {
        self.bool(key).unwrap_or(false)
    }

    /// A `;`-separated list, e.g. `BotTeams=2;2;2`. Empty entries are kept, because
    /// lists like `WeaponProfileNames=BB Gun;;;` are positional.
    pub fn list(&self, key: &str) -> Vec<&str> {
        match self.get(key) {
            None | Some("") => Vec::new(),
            Some(v) => v.split(';').map(str::trim).collect(),
        }
    }

    /// A vector written as `X=1.000 Y=2.000 Z=3.000` (also used for colors).
    pub fn vec3(&self, key: &str) -> Option<[f32; 3]> {
        parse_vec3(self.get(key)?)
    }

    /// Every key and value, in file order.
    pub fn entries(&self) -> impl Iterator<Item = (&str, &str)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

/// Parses `X=1.0 Y=2.0 Z=3.0`.
pub fn parse_vec3(s: &str) -> Option<[f32; 3]> {
    let mut out = [None; 3];
    for part in s.split_whitespace() {
        let (axis, v) = part.split_once('=')?;
        let i = match axis.trim().to_ascii_uppercase().as_str() {
            "X" => 0,
            "Y" => 1,
            "Z" => 2,
            _ => return None,
        };
        out[i] = Some(v.trim().parse().ok()?);
    }
    Some([out[0]?, out[1]?, out[2]?])
}

/// The settings of one file, and the map text that followed them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Settings {
    /// The scenario's own settings, before the first section.
    pub top: Section,
    pub sections: Vec<Section>,
}

/// Header that starts the map, which runs to the end of the file.
pub const MAP_HEADER: &str = "[Map Data]";

impl Settings {
    /// Splits a file into its settings and its map text (empty when there's none).
    /// Lines that aren't `key=value`, headers or comments are skipped.
    pub fn parse(text: &str) -> (Settings, &str) {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let mut settings = Settings::default();
        let mut rest = text;
        let mut current: Option<Section> = None;
        while !rest.is_empty() {
            let (line, next) = match rest.find('\n') {
                Some(i) => (&rest[..i], &rest[i + 1..]),
                None => (rest, ""),
            };
            let trimmed = line.trim();
            if trimmed.eq_ignore_ascii_case(MAP_HEADER) {
                if let Some(s) = current.take() {
                    settings.sections.push(s);
                }
                return (settings, next);
            }
            rest = next;
            if trimmed.is_empty() || trimmed.starts_with("//") {
                continue;
            }
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                if let Some(s) = current.take() {
                    settings.sections.push(s);
                }
                current = Some(Section::new(trimmed[1..trimmed.len() - 1].trim()));
                continue;
            }
            if let Some((k, v)) = trimmed.split_once('=') {
                current.as_mut().unwrap_or(&mut settings.top).push(k.trim(), v.trim());
            }
        }
        if let Some(s) = current.take() {
            settings.sections.push(s);
        }
        (settings, "")
    }

    /// Every section of one kind, e.g. all `[Dodge Profile]`s.
    pub fn of_kind<'a>(&'a self, kind: &'a str) -> impl Iterator<Item = &'a Section> + 'a {
        self.sections.iter().filter(move |s| s.kind.eq_ignore_ascii_case(kind))
    }

    /// The section of a kind with this name. An exact match wins; failing that the
    /// name is compared trimmed and without regard to case, since references and
    /// `Name=` lines don't always agree on either.
    pub fn find(&self, kind: &str, name: &str) -> Option<&Section> {
        let mut of_kind = self.sections.iter().filter(|s| s.kind.eq_ignore_ascii_case(kind));
        of_kind.clone().find(|s| s.name() == name).or_else(|| {
            let name = name.trim();
            of_kind.find(|s| s.name().trim().eq_ignore_ascii_case(name))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "\u{feff}Name=Test Scene\r\nTimelimit=60.0\r\nBotTeams=2;2\r\n\r\n[Bot Profile]\r\nName=Walker\r\n// a comment\r\nCameraOffset=X=0.000 Y=150.000 Z=-1.5\r\n\r\nUseWeapons=true\r\nStrayLineWithoutEquals\r\n\r\n[Character Profile]\r\nName=Ball\r\nFlickFOV=30\r\nflickfov=45\r\n\r\n[Map Data]\r\n{\"objects\": []}\r\n";

    #[test]
    fn splits_settings_sections_and_map() {
        let (s, map) = Settings::parse(FILE);
        assert_eq!(s.top.name(), "Test Scene");
        assert_eq!(s.top.f32("timelimit"), Some(60.0));
        assert_eq!(s.top.list("BotTeams"), vec!["2", "2"]);
        assert_eq!(s.sections.len(), 2);
        let bot = s.find("Bot Profile", "Walker").unwrap();
        assert!(bot.is("UseWeapons"));
        assert_eq!(bot.vec3("CameraOffset"), Some([0.0, 150.0, -1.5]));
        assert_eq!(bot.get("StrayLineWithoutEquals"), None);
        assert_eq!(map.trim(), "{\"objects\": []}");
    }

    #[test]
    fn keys_ignore_case_and_the_last_one_wins() {
        let (s, _) = Settings::parse(FILE);
        assert_eq!(s.find("character profile", "ball").unwrap().f32("FlickFov"), Some(45.0));
    }

    #[test]
    fn positional_lists_keep_empty_slots() {
        let mut sec = Section::new("Character Profile");
        sec.push("WeaponProfileNames", "BB Gun;;;");
        assert_eq!(sec.list("WeaponProfileNames"), vec!["BB Gun", "", "", ""]);
        assert!(sec.list("Missing").is_empty());
    }

    #[test]
    fn a_file_without_a_map_has_empty_map_text() {
        let (s, map) = Settings::parse("Name=x\n[Weapon Profile]\nName=y\n");
        assert_eq!(s.sections.len(), 1);
        assert_eq!(map, "");
    }

    #[test]
    fn vectors_need_all_three_axes() {
        assert_eq!(parse_vec3("X=1 Y=2 Z=3"), Some([1.0, 2.0, 3.0]));
        assert_eq!(parse_vec3("X=1 Y=2"), None);
        assert_eq!(parse_vec3("1, 2, 3"), None);
    }
}
