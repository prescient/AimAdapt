//! One `.sce` file: its settings and map, and the profiles they point at.
//!
//! References chain like this:
//! - `PlayerProfile` names the player's `[Character Profile]`, whose
//!   `WeaponProfileNames` name `[Weapon Profile]`s.
//! - `AddedBots` lists one entry per bot: `Name.bot` is a `[Bot Profile]`, and
//!   `Name.rot` a `[Bot Rotation Profile]` that picks a bot profile by weight.
//! - A bot profile names its `CharacterProfile` and its `DodgeProfileNames` (with
//!   weights); a character's `AbilityProfileNames` name ability profiles.

use std::path::Path;

use crate::map::{self, Map, MapError};
use crate::settings::{Section, Settings};

pub const ABILITY_KINDS: &[&str] = &["Weapon Ability Profile", "Melee Ability Profile", "Movement Ability Profile"];

#[derive(Debug, Clone)]
pub struct Sce {
    pub settings: Settings,
    pub map: Result<Map, MapError>,
}

/// A bot that may fill a slot, and how likely it is.
#[derive(Debug, Clone)]
pub struct BotChoice<'a> {
    pub weight: f32,
    pub bot: &'a Section,
    pub character: Option<&'a Section>,
    pub dodges: Vec<(&'a Section, f32)>,
    pub abilities: Vec<&'a Section>,
}

/// One entry of `AddedBots`.
#[derive(Debug, Clone)]
pub struct BotSlot<'a> {
    pub reference: &'a str,
    pub team: Option<&'a str>,
    pub choices: Vec<BotChoice<'a>>,
    /// A rotation with `Randomized=false`: its bots come in turn, in the order
    /// listed, rather than by weight.
    pub in_order: bool,
}

/// Everything the settings point at, and the names that point at nothing.
#[derive(Debug, Clone, Default)]
pub struct Resolved<'a> {
    pub player: Option<&'a Section>,
    pub weapon: Option<&'a Section>,
    pub player_abilities: Vec<&'a Section>,
    pub bots: Vec<BotSlot<'a>>,
    /// e.g. `Bot Profile "Walker"`.
    pub missing: Vec<String>,
}

impl Sce {
    pub fn parse(text: &str) -> Sce {
        let (settings, map_text) = Settings::parse(text);
        Sce { settings, map: map::parse(map_text) }
    }

    /// Reads a file. KovaaK's writes UTF-8 (sometimes with a byte-order mark); UTF-16
    /// is accepted too, as KovaaK's uses it for playlists.
    pub fn read(path: &Path) -> std::io::Result<Sce> {
        Ok(Sce::parse(&decode(&std::fs::read(path)?)))
    }

    pub fn name(&self) -> &str {
        self.settings.top.name()
    }

    pub fn resolve(&self) -> Resolved<'_> {
        let s = &self.settings;
        let top = &s.top;
        let mut r = Resolved::default();
        let mut missing = |kind: &str, name: &str| {
            let m = format!("{kind} \"{name}\"");
            if !r.missing.contains(&m) {
                r.missing.push(m);
            }
        };

        let player_name = top
            .get("PlayerProfile")
            .filter(|v| !v.is_empty())
            .or_else(|| top.list("PlayerCharacters").first().copied())
            .unwrap_or("");
        let player_name = strip_ext(player_name);
        r.player = s.find("Character Profile", player_name);
        if r.player.is_none() {
            missing("Character Profile", player_name);
        }
        if let Some(p) = r.player {
            if let Some(w) = p.list("WeaponProfileNames").into_iter().find(|w| !w.is_empty()) {
                r.weapon = s.find("Weapon Profile", w);
                if r.weapon.is_none() {
                    missing("Weapon Profile", w);
                }
            }
            r.player_abilities = abilities(s, p, &mut missing);
        }

        let teams = top.list("BotTeams");
        let mut refs = top.list("AddedBots");
        if refs.iter().all(|x| x.is_empty()) {
            refs = top.list("BotCharacters");
        }
        for (i, reference) in refs.into_iter().filter(|x| !x.is_empty()).enumerate() {
            let mut slot = BotSlot { reference, team: teams.get(i).copied(), choices: Vec::new(), in_order: false };
            let base = strip_ext(reference);
            let rotation = if reference.to_ascii_lowercase().ends_with(".bot") {
                None
            } else {
                s.find("Bot Rotation Profile", base)
            };
            slot.in_order = rotation.is_some_and(|rot| rot.str("Randomized").trim().eq_ignore_ascii_case("false"));
            let picks: Vec<(&str, f32)> = match rotation {
                Some(rot) => {
                    let weights = rot.list("ProfileWeights");
                    rot.list("ProfileNames")
                        .into_iter()
                        .enumerate()
                        .filter(|(_, n)| !n.is_empty())
                        .map(|(j, n)| (n, weights.get(j).and_then(|w| w.parse().ok()).unwrap_or(1.0)))
                        .collect()
                }
                None => vec![(base, 1.0)],
            };
            for (name, weight) in picks {
                let Some(bot) = s.find("Bot Profile", name) else {
                    let kind = if rotation.is_some() || reference.to_ascii_lowercase().ends_with(".bot") {
                        "Bot Profile"
                    } else {
                        "Bot Profile or Bot Rotation Profile"
                    };
                    missing(kind, name);
                    continue;
                };
                let char_name = bot.str("CharacterProfile");
                let character = s.find("Character Profile", char_name);
                if character.is_none() {
                    missing("Character Profile", char_name);
                }
                let weights = bot.list("DodgeProfileWeights");
                let mut dodges = Vec::new();
                for (j, d) in bot.list("DodgeProfileNames").into_iter().enumerate().filter(|(_, d)| !d.is_empty()) {
                    match s.find("Dodge Profile", d) {
                        Some(sec) => dodges.push((sec, weights.get(j).and_then(|w| w.parse().ok()).unwrap_or(1.0))),
                        None => missing("Dodge Profile", d),
                    }
                }
                let abilities = character.map(|c| abilities(s, c, &mut missing)).unwrap_or_default();
                slot.choices.push(BotChoice { weight, bot, character, dodges, abilities });
            }
            r.bots.push(slot);
        }
        r
    }
}

fn abilities<'a>(s: &'a Settings, character: &Section, missing: &mut impl FnMut(&str, &str)) -> Vec<&'a Section> {
    let mut out = Vec::new();
    for reference in character.list("AbilityProfileNames").into_iter().filter(|n| !n.is_empty()) {
        // References say which kind they are by extension, e.g. "Approach.abilmov".
        let name = strip_ext(reference);
        let kinds: &[&str] = match ext(reference).as_deref() {
            Some("abilwep") => &ABILITY_KINDS[0..1],
            Some("abilmelee") => &ABILITY_KINDS[1..2],
            Some("abilmov") => &ABILITY_KINDS[2..3],
            _ => ABILITY_KINDS,
        };
        match kinds.iter().find_map(|k| s.find(k, name)) {
            Some(sec) => out.push(sec),
            None => missing(if kinds.len() == 1 { kinds[0] } else { "Ability Profile" }, name),
        }
    }
    out
}

/// Extensions KovaaK's puts on profile references.
const EXTENSIONS: &[&str] = &["bot", "rot", "abilwep", "abilmelee", "abilmov"];

/// The reference's extension, lowercased, if it's one of [`EXTENSIONS`].
fn ext(name: &str) -> Option<String> {
    let (_, e) = name.rsplit_once('.')?;
    let e = e.to_ascii_lowercase();
    EXTENSIONS.contains(&e.as_str()).then_some(e)
}

/// "Fly Rot.rot" → "Fly Rot", "Approach.abilmov" → "Approach".
fn strip_ext(name: &str) -> &str {
    match ext(name) {
        Some(e) => &name[..name.len() - e.len() - 1],
        None => name,
    }
}

/// Some Workshop files wrap the text in a small container: this magic number, a
/// version (1), the text's length in bytes (all little-endian u32), the text, then a
/// binary trailer.
const CONTAINER_MAGIC: [u8; 4] = [0xE6, 0xAC, 0x5B, 0xF5];

/// Bytes to text: unwraps the container if there is one, then reads UTF-16 with a
/// byte-order mark, otherwise UTF-8 (invalid bytes replaced).
pub fn decode(bytes: &[u8]) -> String {
    let bytes = match bytes {
        [m0, m1, m2, m3, _, _, _, _, l0, l1, l2, l3, rest @ ..] if [*m0, *m1, *m2, *m3] == CONTAINER_MAGIC => {
            let len = u32::from_le_bytes([*l0, *l1, *l2, *l3]) as usize;
            &rest[..len.min(rest.len())]
        }
        _ => bytes,
    };
    let utf16 = |le: bool| {
        let units: Vec<u16> = bytes[2..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&c| if le { u16::from_le_bytes(c) } else { u16::from_be_bytes(c) })
            .collect();
        String::from_utf16_lossy(&units)
    };
    match bytes {
        [0xFF, 0xFE, ..] => utf16(true),
        [0xFE, 0xFF, ..] => utf16(false),
        _ => String::from_utf8_lossy(bytes).into_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCENE: &str = "Name=Two Kinds\nPlayerProfile=Hero\nAddedBots=Mix.rot;Lone.bot;Ghost.bot\nBotTeams=2;2;2\n\
        [Character Profile]\nName=Hero\nWeaponProfileNames=;Pea Shooter;;\nAbilityProfileNames=;;;\n\
        [Weapon Profile]\nName=Pea Shooter\nType=Hitscan\n\
        [Bot Rotation Profile]\nName=Mix\nProfileNames=Lone;Fast\nProfileWeights=3.0;1.0\n\
        [Bot Profile]\nName=Lone\nCharacterProfile=Ball\nDodgeProfileNames=Slow;Quick\nDodgeProfileWeights=1.0;2.0\n\
        [Bot Profile]\nName=Fast\nCharacterProfile=Ball\nDodgeProfileNames=Nowhere\n\
        [Character Profile]\nName=Ball\nAbilityProfileNames=Hop;;\n\
        [Movement Ability Profile]\nName=Hop\n\
        [Dodge Profile]\nName=Slow\n[Dodge Profile]\nName=Quick\n\
        [Map Data]\n{\"objects\": []}";

    #[test]
    fn follows_bots_rotations_characters_and_dodges() {
        let sce = Sce::parse(SCENE);
        assert_eq!(sce.name(), "Two Kinds");
        let r = sce.resolve();
        assert_eq!(r.player.unwrap().name(), "Hero");
        // The first non-empty weapon slot is the player's weapon.
        assert_eq!(r.weapon.unwrap().name(), "Pea Shooter");
        assert_eq!(r.bots.len(), 3);
        let mix = &r.bots[0];
        assert_eq!(mix.reference, "Mix.rot");
        assert_eq!(mix.team, Some("2"));
        assert_eq!(
            mix.choices.iter().map(|c| (c.bot.name(), c.weight)).collect::<Vec<_>>(),
            vec![("Lone", 3.0), ("Fast", 1.0)]
        );
        let lone = &r.bots[1].choices[0];
        assert_eq!(lone.character.unwrap().name(), "Ball");
        assert_eq!(
            lone.dodges.iter().map(|(d, w)| (d.name(), *w)).collect::<Vec<_>>(),
            vec![("Slow", 1.0), ("Quick", 2.0)]
        );
        assert_eq!(lone.abilities[0].kind, "Movement Ability Profile");
        assert!(sce.map.is_ok());
    }

    /// Regression: real files write ability references with an extension that names
    /// the kind ("Approach.abilmov"), and the reader looked for a profile called
    /// "Approach.abilmov", so 50 of 225 local files reported a missing profile.
    #[test]
    fn ability_references_with_extensions_resolve_to_their_kind() {
        let text = SCENE
            .replace("AbilityProfileNames=Hop;;", "AbilityProfileNames=Hop.abilmov;Zap.abilwep;Hop.abilmelee")
            .replace(
                "[Dodge Profile]\nName=Slow",
                "[Weapon Ability Profile]\nName=Zap\n[Melee Ability Profile]\nName=Hop\n[Dodge Profile]\nName=Slow",
            );
        let sce = Sce::parse(&text);
        let r = sce.resolve();
        let kinds: Vec<&str> = r.bots[1].choices[0].abilities.iter().map(|a| a.kind.as_str()).collect();
        assert_eq!(kinds, vec!["Movement Ability Profile", "Weapon Ability Profile", "Melee Ability Profile"]);
        assert!(!r.missing.iter().any(|m| m.contains("Hop") || m.contains("Zap")), "{:?}", r.missing);
        assert_eq!(strip_ext("Fly Rot.rot"), "Fly Rot");
        assert_eq!(strip_ext("v1.5 Ball"), "v1.5 Ball");
    }

    #[test]
    fn names_what_is_missing_once() {
        let sce = Sce::parse(SCENE);
        let r = sce.resolve();
        assert_eq!(r.bots[2].choices.len(), 0);
        assert_eq!(r.missing, vec!["Dodge Profile \"Nowhere\"".to_string(), "Bot Profile \"Ghost\"".to_string()]);
    }

    #[test]
    fn bot_characters_stand_in_when_added_bots_is_empty() {
        let text = SCENE.replace("AddedBots=Mix.rot;Lone.bot;Ghost.bot", "AddedBots=\nBotCharacters=Lone.bot");
        let sce = Sce::parse(&text);
        let r = sce.resolve();
        assert_eq!(r.bots.len(), 1);
        assert_eq!(r.bots[0].choices[0].bot.name(), "Lone");
    }

    /// Regression: two Workshop files start with a 12-byte binary header and end with
    /// a binary trailer, so their first key read as garbage + "Name" (no scenario
    /// name) and one JSON map had "extra data" after it.
    #[test]
    fn unwraps_the_workshop_container() {
        let text = "Name=Boxed
[Map Data]
{\"objects\": []}
";
        let mut bytes = CONTAINER_MAGIC.to_vec();
        bytes.extend(1u32.to_le_bytes());
        bytes.extend((text.len() as u32).to_le_bytes());
        bytes.extend(text.as_bytes());
        bytes.extend([0, 0, 2, 0, b'x', 0x9c, 0x63, 0x60]);
        let sce = Sce::parse(&decode(&bytes));
        assert_eq!(sce.name(), "Boxed");
        assert!(sce.map.is_ok());
    }

    #[test]
    fn decodes_utf8_with_bom_and_utf16() {
        assert_eq!(Sce::parse(&decode(b"\xef\xbb\xbfName=A\n")).name(), "A");
        let mut le = vec![0xFF, 0xFE];
        le.extend("Name=B\n".encode_utf16().flat_map(u16::to_le_bytes));
        assert_eq!(Sce::parse(&decode(&le)).name(), "B");
    }
}
