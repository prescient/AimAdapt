//! The `[Map Data]` part of a `.sce` file, in either of its two formats:
//!
//! - **Reflex** (older): text in tab-indented blocks. `brush` blocks are convex solids
//!   given as vertices and faces; `entity` blocks are typed objects with properties
//!   such as `Vector3 position 1 2 3`. Y is up.
//! - **JSON** (newer, KovaaK's own editor): `objects` that are `brush`es (an editor
//!   primitive such as `Cube` placed by location, rotation and scale, or `procedural`
//!   triangles), `gameObject`s (spawn points, spawn volumes, waypoints, teleporters…)
//!   and decorative `prop`s. Z is up.
//!
//! Coordinates are kept exactly as written, in the file's own axes and units. The
//! scenario's `MapScale` isn't applied here.

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapFormat {
    Reflex,
    Json,
}

/// Where an object sits: location, rotation (degrees, as written) and scale.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Placement {
    pub location: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
}

/// Solid level geometry.
#[derive(Debug, Clone, PartialEq)]
pub enum Solid {
    /// A Reflex brush: a convex solid. Each face lists vertex indices in order.
    Polyhedron { vertices: Vec<[f32; 3]>, faces: Vec<Vec<usize>> },
    /// A JSON brush made from a named editor primitive (`Cube`, `Cylinder`, `Ramp`…).
    Primitive { mesh: String, place: Placement },
    /// A JSON brush with its own triangles, in the brush's local space.
    Mesh { place: Placement, vertices: Vec<[f32; 3]>, triangles: Vec<[u32; 3]> },
}

/// Which teams may use a spawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Teams {
    /// JSON maps: `TeamMask`, a number as written (not yet interpreted).
    Mask(u32),
    /// Reflex maps: `teamA` and `teamB` flags, both on unless the file turns one off.
    Flags { a: bool, b: bool },
}

/// A place where the player or a bot can appear: a point, or a volume to pick a
/// point in (JSON `SpawnVolume`, whose size is its scale).
#[derive(Debug, Clone, PartialEq)]
pub struct Spawn {
    pub name: String,
    pub position: [f32; 3],
    pub angles: Option<[f32; 3]>,
    pub teams: Teams,
    pub volume: Option<Placement>,
    pub weight: f32,
    /// Character profiles allowed to spawn here; empty means any.
    pub characters: Vec<String>,
    /// Reflex `initialSpawn`.
    pub initial: bool,
    /// Waypoint path a bot spawned here follows; empty for none.
    pub path: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Waypoint {
    pub name: String,
    pub position: [f32; 3],
    pub pause_s: (f32, f32),
}

/// Scenery from KovaaK's asset packs.
#[derive(Debug, Clone, PartialEq)]
pub struct Prop {
    pub name: String,
    pub place: Placement,
    pub collides: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Map {
    pub format: MapFormat,
    pub version: String,
    pub solids: Vec<Solid>,
    pub spawns: Vec<Spawn>,
    pub waypoints: Vec<Waypoint>,
    pub props: Vec<Prop>,
    /// Gameplay objects not modelled yet, by type (e.g. "Teleporter", "Hurt", "prefab").
    pub other: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MapError {
    /// Nothing after `[Map Data]`.
    Empty,
    Json(String),
    Reflex {
        line: usize,
        message: String,
    },
}

impl std::fmt::Display for MapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MapError::Empty => write!(f, "the file has no map"),
            MapError::Json(e) => write!(f, "the map isn't valid JSON: {e}"),
            MapError::Reflex { line, message } => write!(f, "map line {line}: {message}"),
        }
    }
}

impl std::error::Error for MapError {}

/// Reads map text in either format.
pub fn parse(text: &str) -> Result<Map, MapError> {
    let t = text.trim_start();
    if t.is_empty() {
        Err(MapError::Empty)
    } else if t.starts_with('{') {
        parse_json(t)
    } else {
        parse_reflex(text)
    }
}

// --- Reflex -------------------------------------------------------------------

/// Entity types that only affect looks or the end-of-game camera. A `Target` is a
/// named point; in these maps it's where `WorldSpawn`'s `targetGameOverCamera` looks.
/// (Teleporters that send you to one are flagged under their own type.)
const REFLEX_COSMETIC: &[&str] = &["WorldSpawn", "CameraPath", "Effect", "PointLight", "Decal", "Sky", "Target"];

enum Block {
    None,
    Entity { kind: String, props: Vec<(String, Vec<String>)> },
    Brush { vertices: Vec<[f32; 3]>, faces: Vec<Vec<usize>>, in_faces: bool },
}

fn parse_reflex(text: &str) -> Result<Map, MapError> {
    let mut map = Map {
        format: MapFormat::Reflex,
        version: String::new(),
        solids: Vec::new(),
        spawns: Vec::new(),
        waypoints: Vec::new(),
        props: Vec::new(),
        other: Vec::new(),
    };
    let mut block = Block::None;
    let mut in_prefab = false;
    let err = |line: usize, message: String| MapError::Reflex { line, message };
    for (i, raw) in text.lines().enumerate() {
        let line_no = i + 1;
        let line = raw.trim_end_matches('\r');
        if line.trim().is_empty() {
            continue;
        }
        let depth = line.len() - line.trim_start_matches('\t').len();
        let mut tokens = line.split_whitespace();
        let first = tokens.next().unwrap_or("");
        if depth == 0 {
            finish(&mut map, std::mem::replace(&mut block, Block::None), in_prefab);
            if first == "reflex" {
                map.version = line.trim().rsplit(' ').next().unwrap_or("").to_string();
            } else if first == "prefab" {
                in_prefab = true;
                map.other.push("prefab".into());
            } else {
                in_prefab = false;
            }
            continue;
        }
        if depth == 1 {
            finish(&mut map, std::mem::replace(&mut block, Block::None), in_prefab);
            block = match first {
                "entity" => Block::Entity { kind: String::new(), props: Vec::new() },
                "brush" => Block::Brush { vertices: Vec::new(), faces: Vec::new(), in_faces: false },
                other => return Err(err(line_no, format!("unexpected \"{other}\""))),
            };
            continue;
        }
        match &mut block {
            Block::None => return Err(err(line_no, "indented line outside a block".into())),
            Block::Entity { kind, props } => {
                if first == "type" {
                    *kind = tokens.collect::<Vec<_>>().join(" ");
                } else if let Some(key) = tokens.next() {
                    props.push((key.to_string(), tokens.map(String::from).collect()));
                }
            }
            Block::Brush { vertices, faces, in_faces } => match (depth, first) {
                (2, "vertices") => *in_faces = false,
                (2, "faces") => *in_faces = true,
                (2, other) => return Err(err(line_no, format!("unexpected \"{other}\" in a brush"))),
                _ if !*in_faces => {
                    let v: Vec<f32> = line.split_whitespace().filter_map(|t| t.parse().ok()).collect();
                    if v.len() != 3 {
                        return Err(err(line_no, "a vertex needs three numbers".into()));
                    }
                    vertices.push([v[0], v[1], v[2]]);
                }
                _ => {
                    // Five texture numbers (offset, scale, rotation), the vertex
                    // indices, then flags in hex and an optional material.
                    let idx: Vec<usize> = line
                        .split_whitespace()
                        .skip(5)
                        .take_while(|t| !t.starts_with("0x"))
                        .map(|t| t.parse::<usize>())
                        .collect::<Result<_, _>>()
                        .map_err(|_| err(line_no, "a face's vertex indices must be whole numbers".into()))?;
                    if idx.len() < 3 {
                        return Err(err(line_no, "a face needs at least three vertices".into()));
                    }
                    if let Some(bad) = idx.iter().find(|&&j| j >= vertices.len()) {
                        return Err(err(
                            line_no,
                            format!("a face uses vertex {bad}, but the brush has {}", vertices.len()),
                        ));
                    }
                    faces.push(idx);
                }
            },
        }
    }
    finish(&mut map, block, in_prefab);
    Ok(map)
}

fn finish(map: &mut Map, block: Block, in_prefab: bool) {
    if in_prefab {
        return; // Prefab definitions are templates; the map counts them as "prefab".
    }
    match block {
        Block::None => {}
        Block::Brush { vertices, faces, .. } => map.solids.push(Solid::Polyhedron { vertices, faces }),
        Block::Entity { kind, props } => {
            let get = |key: &str| props.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v);
            let vec3 = |key: &str| {
                get(key).and_then(|v| match v.iter().map(|t| t.parse::<f32>()).collect::<Result<Vec<_>, _>>() {
                    Ok(n) if n.len() == 3 => Some([n[0], n[1], n[2]]),
                    _ => None,
                })
            };
            let flag = |key: &str| get(key).and_then(|v| v.first()).map(|t| t != "0");
            match kind.as_str() {
                "PlayerSpawn" => map.spawns.push(Spawn {
                    name: String::new(),
                    position: vec3("position").unwrap_or_default(),
                    angles: vec3("angles"),
                    teams: Teams::Flags { a: flag("teamA").unwrap_or(true), b: flag("teamB").unwrap_or(true) },
                    volume: None,
                    weight: 1.0,
                    characters: Vec::new(),
                    initial: flag("initialSpawn").unwrap_or(false),
                    path: get("path").map(|v| v.join(" ")).unwrap_or_default(),
                }),
                k if REFLEX_COSMETIC.contains(&k) => {}
                k => map.other.push(k.to_string()),
            }
        }
    }
}

// --- JSON ---------------------------------------------------------------------

fn parse_json(text: &str) -> Result<Map, MapError> {
    // Some files have stray text after the map's closing brace; read the first value.
    let doc: Value = serde_json::Deserializer::from_str(text)
        .into_iter::<Value>()
        .next()
        .ok_or(MapError::Empty)?
        .map_err(|e| MapError::Json(e.to_string()))?;
    let mut map = Map {
        format: MapFormat::Json,
        version: doc.get("version").and_then(Value::as_str).unwrap_or("").to_string(),
        solids: Vec::new(),
        spawns: Vec::new(),
        waypoints: Vec::new(),
        props: Vec::new(),
        other: Vec::new(),
    };
    let objects =
        doc.get("objects").and_then(Value::as_array).ok_or_else(|| MapError::Json("no \"objects\" list".into()))?;
    for (i, o) in objects.iter().enumerate() {
        let bad = |m: &str| MapError::Json(format!("object {i}: {m}"));
        let place = placement(o).map_err(|m| bad(&m))?;
        let name = o.get("name").and_then(Value::as_str).unwrap_or("").to_string();
        match o.get("type").and_then(Value::as_str).unwrap_or("") {
            "brush" => {
                let mesh = o.get("mesh").and_then(Value::as_str).unwrap_or("").to_string();
                match o.get("procedural").and_then(Value::as_array) {
                    Some(parts) => {
                        let (vertices, triangles) = procedural(parts).map_err(|m| bad(&m))?;
                        map.solids.push(Solid::Mesh { place, vertices, triangles });
                    }
                    None => map.solids.push(Solid::Primitive { mesh, place }),
                }
            }
            "prop" => map.props.push(Prop { name, place, collides: prop_bool(o, "EnableCollision").unwrap_or(true) }),
            "gameObject" => match name.as_str() {
                "SpawnPoint" | "SpawnVolume" => map.spawns.push(Spawn {
                    name: prop_str(o, "Name").unwrap_or("").to_string(),
                    position: place.location,
                    angles: Some(place.rotation),
                    teams: Teams::Mask(prop_f64(o, "TeamMask").unwrap_or(0.0) as u32),
                    volume: (name == "SpawnVolume").then_some(place),
                    weight: prop_f64(o, "Weight").unwrap_or(1.0) as f32,
                    characters: prop_str(o, "PermittedCharacterProfiles")
                        .unwrap_or("")
                        .split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(String::from)
                        .collect(),
                    initial: false,
                    path: prop_str(o, "Path").unwrap_or("").to_string(),
                }),
                "Waypoint" => map.waypoints.push(Waypoint {
                    name: prop_str(o, "Name").unwrap_or("").to_string(),
                    position: place.location,
                    pause_s: (
                        prop_f64(o, "BotPauseTimeMin").unwrap_or(0.0) as f32,
                        prop_f64(o, "BotPauseTimeMax").unwrap_or(0.0) as f32,
                    ),
                }),
                other => map.other.push(other.to_string()),
            },
            other => map.other.push(if other.is_empty() { "untyped object".into() } else { other.to_string() }),
        }
    }
    Ok(map)
}

/// Parses "1.0, 2.0, 3.0".
fn triplet(s: &str) -> Option<[f32; 3]> {
    let v: Vec<f32> = s.split(',').map(|t| t.trim().parse().ok()).collect::<Option<_>>()?;
    (v.len() == 3).then(|| [v[0], v[1], v[2]])
}

fn placement(o: &Value) -> Result<Placement, String> {
    let field = |key: &str, default: [f32; 3]| match o.get(key) {
        None => Ok(default),
        Some(Value::String(s)) => triplet(s).ok_or_else(|| format!("bad {key} \"{s}\"")),
        Some(_) => Err(format!("{key} isn't text")),
    };
    Ok(Placement {
        location: field("location", [0.0; 3])?,
        rotation: field("rotation", [0.0; 3])?,
        scale: field("scale", [1.0; 3])?,
    })
}

/// Vertices and triangles (indices into the vertices).
type Triangles = (Vec<[f32; 3]>, Vec<[u32; 3]>);

/// Joins a brush's procedural parts into one vertex list and triangle list.
fn procedural(parts: &[Value]) -> Result<Triangles, String> {
    let mut vertices = Vec::new();
    let mut triangles = Vec::new();
    for part in parts {
        let base = vertices.len() as u32;
        let verts = part.get("vertices").and_then(Value::as_array).ok_or("a procedural part has no vertices")?;
        for v in verts {
            let loc = v.get("location").and_then(Value::as_str).and_then(triplet).ok_or("a vertex has no location")?;
            vertices.push(loc);
        }
        let idx: Vec<u32> = part
            .get("indices")
            .and_then(Value::as_array)
            .ok_or("a procedural part has no indices")?
            .iter()
            .map(|i| i.as_u64().map(|n| n as u32))
            .collect::<Option<_>>()
            .ok_or("indices must be whole numbers")?;
        if !idx.len().is_multiple_of(3) {
            return Err(format!("{} indices don't make whole triangles", idx.len()));
        }
        for t in idx.as_chunks::<3>().0 {
            if t.iter().any(|&i| i >= verts.len() as u32) {
                return Err("a triangle uses a vertex that isn't there".into());
            }
            triangles.push([base + t[0], base + t[1], base + t[2]]);
        }
    }
    Ok((vertices, triangles))
}

fn prop<'a>(o: &'a Value, name: &str) -> Option<&'a Value> {
    o.get("properties")?.as_array()?.iter().find(|p| p.get("name").and_then(Value::as_str) == Some(name))?.get("value")
}

fn prop_str<'a>(o: &'a Value, name: &str) -> Option<&'a str> {
    prop(o, name)?.as_str()
}

fn prop_f64(o: &Value, name: &str) -> Option<f64> {
    prop(o, name)?.as_f64()
}

fn prop_bool(o: &Value, name: &str) -> Option<bool> {
    prop(o, name)?.as_bool()
}

#[cfg(test)]
mod tests {
    use super::*;

    const REFLEX: &str = "reflex map version 8\r\nglobal\r\n\tentity\r\n\t\ttype WorldSpawn\r\n\t\tUInt8 playersMax 16\r\n\tbrush\r\n\t\tvertices\r\n\t\t\t0 0 0\r\n\t\t\t10 0 0\r\n\t\t\t10 0 10\r\n\t\t\t0 0 10\r\n\t\tfaces\r\n\t\t\t0.0 0.0 1.0 1.0 0.0 0 1 2 3 0x00000000 \r\n\tentity\r\n\t\ttype PlayerSpawn\r\n\t\tVector3 position 1.5 2 -3\r\n\t\tVector3 angles 90 0 0\r\n\t\tBool8 TeamA 0\r\n\tentity\r\n\t\ttype Teleporter\r\n";

    #[test]
    fn reads_reflex_brushes_and_spawns() {
        let m = parse(REFLEX).unwrap();
        assert_eq!(m.format, MapFormat::Reflex);
        assert_eq!(m.version, "8");
        assert_eq!(m.solids.len(), 1);
        let Solid::Polyhedron { vertices, faces } = &m.solids[0] else { panic!() };
        assert_eq!(vertices.len(), 4);
        assert_eq!(faces, &vec![vec![0, 1, 2, 3]]);
        assert_eq!(m.spawns.len(), 1);
        assert_eq!(m.spawns[0].position, [1.5, 2.0, -3.0]);
        assert_eq!(m.spawns[0].angles, Some([90.0, 0.0, 0.0]));
        // `TeamA` is spelled both ways in real files.
        assert_eq!(m.spawns[0].teams, Teams::Flags { a: false, b: true });
        assert_eq!(m.other, vec!["Teleporter".to_string()]);
    }

    /// Regression: Reflex `Target` entities (the end-of-game camera's look-at point)
    /// were reported as unmodelled gameplay objects in 44 of 225 local files.
    #[test]
    fn reflex_camera_targets_are_cosmetic() {
        let text = format!(
            "{REFLEX}	entity
		type Target
		Vector3 position 0 0 0
		String32 name end
"
        );
        assert_eq!(parse(&text).unwrap().other, vec!["Teleporter".to_string()]);
    }

    #[test]
    fn reflex_faces_must_use_real_vertices() {
        let bad = REFLEX.replace("0 1 2 3 0x", "0 1 2 9 0x");
        assert!(matches!(parse(&bad), Err(MapError::Reflex { line: 13, .. })));
    }

    const JSON: &str = r#"{
      "version": "1.0.0",
      "materialSets": [],
      "objects": [
        {"type": "brush", "mesh": "Cube", "name": "Default", "location": "0, 0, -16", "rotation": "0, 0, 90", "scale": "20, 20, 0.16"},
        {"type": "brush", "mesh": "Cube", "name": "Default", "location": "5, 5, 0",
         "procedural": [{"indices": [0, 1, 2], "vertices": [{"location": "0, 0, 0"}, {"location": "1, 0, 0"}, {"location": "0, 1, 0"}]},
                        {"indices": [0, 1, 2], "vertices": [{"location": "0, 0, 1"}, {"location": "1, 0, 1"}, {"location": "0, 1, 1"}]}]},
        {"type": "gameObject", "name": "SpawnPoint", "location": "240, 0, 0",
         "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 0.5}, {"name": "PermittedCharacterProfiles", "value": "ball, cube"}]},
        {"type": "gameObject", "name": "SpawnVolume", "location": "0, 0, 100", "scale": "4, 8, 2", "properties": [{"name": "TeamMask", "value": 2}]},
        {"type": "gameObject", "name": "Waypoint", "location": "1, 2, 3", "properties": [{"name": "Name", "value": "a"}, {"name": "BotPauseTimeMax", "value": 0.5}]},
        {"type": "gameObject", "name": "Teleporter", "location": "0, 0, 0"},
        {"type": "prop", "name": "Tiles", "properties": [{"name": "EnableCollision", "value": false}]}
      ]
    }trailing junk"#;

    #[test]
    fn reads_json_brushes_spawns_and_objects() {
        let m = parse(JSON).unwrap();
        assert_eq!(m.format, MapFormat::Json);
        assert_eq!(m.version, "1.0.0");
        assert_eq!(m.solids.len(), 2);
        let Solid::Primitive { mesh, place } = &m.solids[0] else { panic!() };
        assert_eq!(mesh, "Cube");
        assert_eq!(place.scale, [20.0, 20.0, 0.16]);
        let Solid::Mesh { vertices, triangles, .. } = &m.solids[1] else { panic!() };
        assert_eq!(vertices.len(), 6);
        assert_eq!(triangles, &vec![[0, 1, 2], [3, 4, 5]]);
        assert_eq!(m.spawns.len(), 2);
        assert_eq!(m.spawns[0].teams, Teams::Mask(2));
        assert_eq!(m.spawns[0].weight, 0.5);
        assert_eq!(m.spawns[0].characters, vec!["ball".to_string(), "cube".to_string()]);
        assert_eq!(m.spawns[1].volume.unwrap().scale, [4.0, 8.0, 2.0]);
        assert_eq!(m.waypoints[0].pause_s, (0.0, 0.5));
        assert!(!m.props[0].collides);
        assert_eq!(m.other, vec!["Teleporter".to_string()]);
    }

    #[test]
    fn bad_json_says_which_object() {
        let bad = JSON.replace("\"20, 20, 0.16\"", "\"20, 20\"");
        let Err(MapError::Json(msg)) = parse(&bad) else { panic!() };
        assert!(msg.starts_with("object 0:"), "{msg}");
    }

    #[test]
    fn an_empty_map_is_an_error() {
        assert_eq!(parse("  \r\n"), Err(MapError::Empty));
    }
}
