//! The Bevy side: window, mouse look, drawing the room and targets, the HUD and
//! the results screen. Game rules live in `aimadapt_sim`; this module only feeds it
//! input and draws what it reports.
//!
//! A session plays one or more steps: a single scenario, or each run of a workout
//! in turn. The room is rebuilt whenever the scenario changes.

use std::collections::HashMap;
use std::path::PathBuf;

use aimadapt_sim::adaptive::{Knob, Pick, PlanVerdict, judge_against_plan};
use aimadapt_sim::scenario::{Shape, parse_color};
use aimadapt_sim::settings::DisplayMode;
use aimadapt_sim::sim::from_angles;
use aimadapt_sim::{Event, Input, Phase, PlayerSettings, RunResult, Scenario, Sim};
use bevy::asset::RenderAssetUsages;
use bevy::camera::{ClearColorConfig, Viewport};
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::math::Affine2;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::text::FontSize;
use bevy::window::{
    CursorGrabMode, CursorOptions, Monitor, MonitorSelection, PresentMode, PrimaryMonitor, VideoMode,
    VideoModeSelection, WindowMode, WindowPosition,
};
use chrono::{Local, NaiveDateTime};

use crate::record::{self, RunInfo};
use crate::results::{self, Keys, Open, Outcome, StepDone};

mod results_panel;

/// One run to play.
#[derive(Clone)]
pub struct PlayStep {
    pub scenario: Scenario,
    pub adaptive: bool,
    /// Shown with the run, e.g. "2 of 3".
    pub label: Option<String>,
    /// Rep number within its workout item, the item's rep count, and its block.
    pub rep: u32,
    pub reps: u32,
    pub block: aimadapt_sim::workout::Block,
}

pub struct PlayOptions {
    pub steps: Vec<PlayStep>,
    /// Set when the steps come from a workout; saved with each run.
    pub workout: Option<String>,
    /// Program name and weekday id (`mon`…) when playing a program day.
    pub program: Option<(String, String)>,
    pub settings: PlayerSettings,
    /// Where finished runs are saved. `None` plays without saving.
    pub db_path: Option<PathBuf>,
    pub seed: u64,
    /// Smoke test: a bot plays, screenshots go in this folder, then the app exits.
    pub smoke: Option<PathBuf>,
    /// The app window's outer position and width (physical pixels), to open over.
    pub anchor: Option<[i32; 3]>,
    /// The app started the trainer and waits for it: the results screen offers keys
    /// that close the trainer and open a page in the app.
    pub handoff: bool,
}

/// Where the trainer window opens over the app window: centered across it, with
/// the tops level. `anchor` is the app window's outer x, y and width.
pub fn window_position(anchor: [i32; 3], window_width: u32) -> [i32; 2] {
    let [x, y, w] = anchor;
    [x + (w - window_width as i32) / 2, y]
}

pub fn run(opts: PlayOptions) -> AppExit {
    let d = &opts.settings.display;
    let title = match &opts.workout {
        Some(w) => format!("AimAdapt — {w}"),
        None => format!("AimAdapt — {}", opts.steps.first().map_or("", |s| s.scenario.name.as_str())),
    };
    let size = d.resolution.unwrap_or([1600, 900]);
    let window = Window {
        title,
        present_mode: PresentMode::AutoNoVsync,
        // Exclusive full screen is switched to once the monitor's video modes are known.
        mode: match d.mode {
            DisplayMode::Windowed => WindowMode::Windowed,
            _ => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
        },
        resolution: (size[0], size[1]).into(),
        // Over the app window, so it opens where you keep it (and full screen goes
        // on that monitor).
        position: match opts.anchor {
            Some(a) => {
                let [x, y] = window_position(a, size[0]);
                WindowPosition::At(IVec2::new(x, y))
            }
            None => WindowPosition::Automatic,
        },
        ..default()
    };
    let mut game = Game {
        session: Session {
            steps: opts.steps,
            index: 0,
            seed: opts.seed,
            workout: opts.workout,
            program: opts.program,
            settings: opts.settings,
            db_path: opts.db_path,
            handoff: opts.handoff,
            done: Vec::new(),
        },
        sim: Sim::new(Scenario::default(), opts.seed),
        look: Look::default(),
        run: Run::default(),
        fps: 0.0,
        world_dirty: true,
        display_note: None,
        video_mode_set: false,
        smoke: opts.smoke.map(|dir| Smoke { dir, taken: 0, after_finish: 0.0 }),
        bot: crate::bot::Bot::new(opts.seed),
    };
    game.begin_step(0);
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin { primary_window: Some(window), ..default() }))
        .insert_resource(ClearColor(Color::srgb(0.05, 0.06, 0.08)))
        .insert_resource(GlobalAmbientLight { color: Color::WHITE, brightness: 600.0, ..default() })
        .insert_resource(game)
        .init_resource::<CursorLock>()
        .add_systems(Startup, (setup_cameras, setup_hud))
        .add_systems(
            Update,
            (
                (exclusive_video_mode, play_area, rebuild_world),
                (
                    keys,
                    cursor,
                    look,
                    step,
                    sync_targets,
                    sync_bars,
                    update_hud,
                    update_overlay,
                    results_panel::update,
                    smoke_capture,
                ),
            )
                .chain(),
        )
        .run()
}

#[derive(Default, Clone, Copy)]
struct Look {
    /// Degrees right of straight ahead. Kept in f64 so tiny movements add up exactly.
    yaw: f64,
    /// Degrees above level.
    pitch: f64,
}

impl Look {
    fn aim(&self) -> Vec3 {
        from_angles(self.yaw as f32, self.pitch as f32)
    }
}

/// What this trainer window plays: the steps (one scenario, or a workout's runs),
/// with the settings and database they use.
struct Session {
    steps: Vec<PlayStep>,
    index: usize,
    seed: u64,
    workout: Option<String>,
    program: Option<(String, String)>,
    settings: PlayerSettings,
    db_path: Option<PathBuf>,
    /// The app is waiting for the trainer (see [`PlayOptions::handoff`]).
    handoff: bool,
    /// Each finished step, for the workout summary.
    done: Vec<StepDone>,
}

/// Everything about one run that starts over with the next.
#[derive(Default)]
struct Run {
    /// Wall-clock start of the timed part of the run.
    started_at: Option<NaiveDateTime>,
    frames: u64,
    run_time: f32,
    /// Best earlier score (standard) or level (adaptive) for this scenario.
    best: Option<f64>,
    /// What happened when the finished run was saved: its id, or why not.
    saved: Option<Result<i64, String>>,
    /// The chart and note for the results screen, read once after the run is saved.
    extras: Option<aimadapt_core::results::RunExtras>,
    /// The finished run's result, built once: the results screen reads it every
    /// frame, and building it copies the run's whole telemetry.
    result: Option<RunResult>,
    /// Seconds of hit flash left, by target id.
    flashes: HashMap<u64, f32>,
    /// A message shown mid-screen for a moment (breakthroughs), and seconds left.
    banner: Option<(String, f32)>,
}

#[derive(Resource)]
struct Game {
    session: Session,
    sim: Sim,
    look: Look,
    /// The run being played. Replaced whole when the next step begins.
    run: Run,
    /// Smoothed frames per second.
    fps: f32,
    world_dirty: bool,
    display_note: Option<String>,
    video_mode_set: bool,
    smoke: Option<Smoke>,
    bot: crate::bot::Bot,
}

struct Smoke {
    dir: PathBuf,
    /// Screenshots taken in the current step.
    taken: u8,
    /// Seconds since the run finished.
    after_finish: f32,
}

impl Game {
    fn step_info(&self) -> &PlayStep {
        &self.session.steps[self.session.index]
    }

    fn is_last(&self) -> bool {
        self.session.index + 1 >= self.session.steps.len()
    }

    /// Starts step `i` (or restarts the current one), reading the last adaptive
    /// level and the best so far from the database.
    fn begin_step(&mut self, i: usize) {
        let changed = *self.sim.scenario() != self.session.steps[i].scenario;
        self.session.index = i;
        self.session.seed = self.session.seed.wrapping_add(1);
        let step = self.session.steps[i].clone();
        let db = self.session.db_path.as_deref().and_then(|p| aimadapt_core::Db::open(p).ok());
        let s = step.scenario.clone();
        self.run = Run::default();
        self.sim = if step.adaptive {
            let now = Local::now().naive_local();
            let prior = db.as_ref().and_then(|db| record::prior(db, &s, now));
            self.run.best = db.as_ref().and_then(|db| record::best_rating(db, &s));
            Sim::adaptive_in_block(s, self.session.seed, prior, self.session.settings.challenge, step.rep, step.reps)
        } else {
            self.run.best = db.as_ref().and_then(|db| record::best_score(db, &s));
            Sim::new(s, self.session.seed)
        };
        self.look = Look::default();
        if changed {
            self.world_dirty = true;
        }
        if let Some(s) = &mut self.smoke {
            s.taken = 0;
            s.after_finish = 0.0;
        }
    }

    fn record_done(&mut self) {
        let r = self.sim.result();
        let line = match &r.adaptive {
            Some(a) => {
                let hit = a.success_rate.unwrap_or(0.0) * 100.0;
                match a.run_value {
                    None => {
                        let after = a.break_days.map_or(String::new(), |d| format!(" after {} away", days_text(d)));
                        format!(
                            "Calibration{after}: {:.0}% at {}, hit {hit:.0}%",
                            a.goal * 100.0,
                            fmt_value(a.knob, a.value)
                        )
                    }
                    Some(v) => format!(
                        "{} at {}: hit {hit:.0}%, rating {} (was {})",
                        a.pick.run_label(),
                        fmt_value(a.knob, v),
                        fmt_value(a.knob, a.value),
                        fmt_value(a.knob, a.start_value)
                    ),
                }
            }
            None => format!(
                "Score {:.0}, accuracy {}",
                r.score,
                r.stats.accuracy().map_or("-".into(), |a| format!("{:.0}%", a * 100.0))
            ),
        };
        let outcome = match &r.adaptive {
            Some(a) => Outcome::Rated {
                after: fmt_value(a.knob, a.value),
                before: a.run_value.map(|_| fmt_value(a.knob, a.start_value)),
            },
            None => Outcome::Scored(f64::from(r.score)),
        };
        self.session.done.push(StepDone { name: self.sim.scenario().name.clone(), line, outcome });
    }
}

#[derive(Component)]
struct PlayerCamera;

/// Everything that belongs to the current scenario's room.
#[derive(Component)]
struct WorldItem;

#[derive(Component)]
struct TargetView {
    id: u64,
    material: Handle<StandardMaterial>,
}

#[derive(Resource)]
struct TargetAssets {
    mesh: Handle<Mesh>,
    base: Color,
    /// An imported scenario's kinds of bot: each one's mesh, the radius it was made
    /// at, and its color.
    kinds: Vec<(Handle<Mesh>, f32, Color)>,
    /// The second color for timed targets, when colors alternate.
    alt: Option<Color>,
    /// Targets per wave: the scenario's target count.
    wave: u32,
    radius: f32,
    /// A 1 m square facing +Z, and the bar materials: background, health, time left.
    bar: Handle<Mesh>,
    bar_back: Handle<StandardMaterial>,
    bar_health: Handle<StandardMaterial>,
    bar_time: Handle<StandardMaterial>,
}

/// One part of a bar above a target: its background or its fill.
#[derive(Component)]
struct BarView {
    id: u64,
    kind: Bar,
    fill: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Bar {
    Health,
    TimeLeft,
}

#[derive(Component)]
struct TimerText;

#[derive(Component)]
struct ScoreText;

#[derive(Component)]
struct StatsText;

#[derive(Component)]
struct CenterText;

#[derive(Component)]
struct FpsText;

#[derive(Component)]
struct StepText;

#[derive(Component)]
struct Overlay;

#[derive(Component)]
struct OverlayText;

/// The workout lines and key hints at the bottom of the results.
#[derive(Component)]
struct KeysText;

fn color(hex: &str) -> Color {
    let [r, g, b] = parse_color(hex).unwrap_or([1.0, 0.3, 0.3]);
    Color::srgb(r, g, b)
}

/// An imported map's triangles with flat normals, and texture coordinates from
/// the two axes each face is most square to, so the 2 m grid lies flat on it.
fn world_mesh(m: &aimadapt_sim::world::WorldMesh) -> Mesh {
    let mut positions = Vec::with_capacity(m.triangles.len() * 3);
    let mut normals = Vec::with_capacity(m.triangles.len() * 3);
    let mut uvs = Vec::with_capacity(m.triangles.len() * 3);
    for tri in &m.triangles {
        let [a, b, c] = tri.map(|i| Vec3::from_array(m.positions[i as usize]));
        let n = (b - a).cross(c - a).normalize_or_zero();
        let abs = n.abs();
        for p in [a, b, c] {
            let uv = if abs.x >= abs.y && abs.x >= abs.z {
                [p.z, p.y]
            } else if abs.y >= abs.z {
                [p.x, p.z]
            } else {
                [p.x, p.y]
            };
            positions.push(p.to_array());
            normals.push(n.to_array());
            uvs.push([uv[0] / 2.0, uv[1] / 2.0]);
        }
    }
    Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
}

/// A light grid on a transparent background, tiled over the walls so movement is
/// easy to judge.
fn grid_texture() -> Image {
    const N: u32 = 128;
    let mut data = Vec::with_capacity((N * N * 4) as usize);
    for y in 0..N {
        for x in 0..N {
            let line = x < 2 || y < 2;
            let v = if line { 255 } else { 0 };
            data.extend_from_slice(&[255, 255, 255, v]);
        }
    }
    let mut image = Image::new(
        Extent3d { width: N, height: N, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::linear()
    });
    image
}

fn setup_cameras(mut commands: Commands, game: Res<Game>) {
    // Clears the whole window to black, so a smaller play area has a black border.
    commands.spawn((Camera2d, Camera { order: -1, clear_color: ClearColorConfig::Custom(Color::BLACK), ..default() }));
    commands.spawn((
        PlayerCamera,
        Camera3d::default(),
        Camera { order: 0, ..default() },
        IsDefaultUiCamera,
        Projection::from(PerspectiveProjection { fov: game.session.settings.vertical_fov(), near: 0.05, ..default() }),
        Transform::IDENTITY,
    ));
}

/// Entities that belong to the current scenario and go when it changes.
type RoomEntities<'w, 's> = Query<'w, 's, Entity, Or<(With<WorldItem>, With<TargetView>, With<BarView>)>>;

/// Builds the room and target mesh for the current scenario when it changes.
fn rebuild_world(
    mut commands: Commands,
    mut game: ResMut<Game>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    old: RoomEntities,
) {
    if !game.world_dirty {
        return;
    }
    game.world_dirty = false;
    for e in &old {
        commands.entity(e).despawn();
    }
    let s = game.sim.scenario();
    let [w, h, d] = s.arena.size;
    let eye = s.arena.eye_height;
    let grid = images.add(grid_texture());
    let wall = color(&s.arena.wall_color);
    let floor = color(&s.arena.floor_color);
    let t = 0.1;

    // Imported scenarios draw their own map instead of the room.
    if let Some(world) = &s.world {
        for m in world.meshes.iter().filter(|m| !m.clip) {
            let base = color(&m.color);
            let mesh = meshes.add(world_mesh(m));
            let solid = materials.add(StandardMaterial {
                base_color: base,
                perceptual_roughness: 1.0,
                reflectance: 0.0,
                double_sided: true,
                cull_mode: None,
                ..default()
            });
            let lines = materials.add(StandardMaterial {
                base_color: base.lighter(0.08),
                base_color_texture: Some(grid.clone()),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                double_sided: true,
                cull_mode: None,
                depth_bias: 50.0,
                ..default()
            });
            commands.spawn((WorldItem, Mesh3d(mesh.clone()), MeshMaterial3d(solid), Transform::IDENTITY));
            commands.spawn((WorldItem, Mesh3d(mesh), MeshMaterial3d(lines), Transform::IDENTITY));
        }
    }
    // Each panel: size, center, color, and how many 2 m grid cells it spans.
    let floor_y = -eye - t / 2.0;
    let panels = [
        (Vec3::new(w, t, d), Vec3::new(0.0, floor_y, 0.0), floor, Vec2::new(w, d)),
        (Vec3::new(w, t, d), Vec3::new(0.0, h - eye + t / 2.0, 0.0), wall, Vec2::new(w, d)),
        (Vec3::new(w, h, t), Vec3::new(0.0, h / 2.0 - eye, -d / 2.0 - t / 2.0), wall, Vec2::new(w, h)),
        (Vec3::new(w, h, t), Vec3::new(0.0, h / 2.0 - eye, d / 2.0 + t / 2.0), wall, Vec2::new(w, h)),
        (Vec3::new(t, h, d), Vec3::new(-w / 2.0 - t / 2.0, h / 2.0 - eye, 0.0), wall, Vec2::new(d, h)),
        (Vec3::new(t, h, d), Vec3::new(w / 2.0 + t / 2.0, h / 2.0 - eye, 0.0), wall, Vec2::new(d, h)),
    ];
    for (size, center, base, cells) in if s.world.is_some() { Vec::new() } else { panels.to_vec() } {
        let mat = materials.add(StandardMaterial {
            base_color: base,
            perceptual_roughness: 1.0,
            reflectance: 0.0,
            ..default()
        });
        commands.spawn((
            WorldItem,
            Mesh3d(meshes.add(Cuboid::from_size(size))),
            MeshMaterial3d(mat),
            Transform::from_translation(center),
        ));
        // Grid lines on a slightly inset copy, so the panel color shows through.
        let lines = materials.add(StandardMaterial {
            base_color: base.lighter(0.08),
            base_color_texture: Some(grid.clone()),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            uv_transform: Affine2::from_scale(cells / 2.0),
            ..default()
        });
        let inset = -center.normalize_or_zero() * 0.01;
        commands.spawn((
            WorldItem,
            Mesh3d(meshes.add(Cuboid::from_size(size * 0.999))),
            MeshMaterial3d(lines),
            Transform::from_translation(center + inset),
        ));
    }
    commands.spawn((
        WorldItem,
        DirectionalLight { illuminance: 6000.0, shadow_maps_enabled: false, ..default() },
        Transform::from_xyz(4.0, 10.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    let tc = &s.targets;
    let mut body = |shape: Shape, radius: f32, height: f32| match shape {
        Shape::Sphere => meshes.add(Sphere::new(radius).mesh().uv(48, 24)),
        Shape::Capsule => meshes.add(Capsule3d::new(radius, (height - 2.0 * radius).max(0.0))),
        Shape::Box => meshes.add(Cuboid::new(2.0 * radius, height, 2.0 * radius)),
    };
    let mesh = body(tc.shape, tc.radius, tc.height);
    let kinds = s
        .world
        .iter()
        .flat_map(|w| &w.kinds)
        .map(|k| (body(k.shape, k.radius, k.height), k.radius, color(&k.color)))
        .collect();
    let flat = |c: Color| StandardMaterial { base_color: c, unlit: true, ..default() };
    commands.insert_resource(TargetAssets {
        mesh,
        base: color(&tc.color),
        kinds,
        alt: tc.alternate_colors.then(|| color(&tc.alt_color)),
        wave: tc.count.max(1),
        radius: tc.radius,
        bar: meshes.add(Rectangle::new(1.0, 1.0)),
        bar_back: materials.add(flat(Color::srgba(0.0, 0.0, 0.0, 1.0))),
        bar_health: materials.add(flat(Color::srgb(0.35, 0.85, 0.45))),
        bar_time: materials.add(flat(Color::srgb(0.95, 0.95, 0.95))),
    });
}

/// Switches to exclusive full screen at the chosen resolution and refresh rate once
/// the monitor's video modes are known.
fn exclusive_video_mode(
    mut game: ResMut<Game>,
    mut window: Single<&mut Window>,
    monitors: Query<&Monitor, With<PrimaryMonitor>>,
) {
    if game.video_mode_set || game.session.settings.display.mode != DisplayMode::Exclusive {
        return;
    }
    let Some(monitor) = monitors.iter().next() else { return };
    game.video_mode_set = true;
    let d = &game.session.settings.display;
    let want = d.resolution.unwrap_or([monitor.physical_width, monitor.physical_height]);
    let fits: Vec<&VideoMode> =
        monitor.video_modes.iter().filter(|m| m.physical_size.x == want[0] && m.physical_size.y == want[1]).collect();
    let pick = match d.refresh_hz {
        Some(hz) => fits.iter().min_by_key(|m| (i64::from(m.refresh_rate_millihertz) - i64::from(hz) * 1000).abs()),
        None => fits.iter().max_by_key(|m| (m.refresh_rate_millihertz, m.bit_depth)),
    };
    match pick {
        Some(m) => {
            window.mode = WindowMode::Fullscreen(MonitorSelection::Current, VideoModeSelection::Specific(**m));
        }
        None => {
            game.display_note =
                Some(format!("Your monitor doesn't offer {}x{}; using borderless full screen.", want[0], want[1]));
        }
    }
}

/// Keeps the 3D view (and the HUD with it) in the centered play area, if one is set.
fn play_area(game: Res<Game>, window: Single<&Window>, mut camera: Single<&mut Camera, With<PlayerCamera>>) {
    let size = [window.physical_width(), window.physical_height()];
    let viewport = game.session.settings.display.play_area_in(size).map(|(pos, area)| Viewport {
        physical_position: UVec2::new(pos[0], pos[1]),
        physical_size: UVec2::new(area[0], area[1]),
        ..default()
    });
    let key = |v: &Option<Viewport>| v.as_ref().map(|v| (v.physical_position, v.physical_size));
    if key(&camera.viewport) != key(&viewport) {
        camera.viewport = viewport;
    }
}

fn setup_hud(mut commands: Commands, game: Res<Game>) {
    let text = |size: f32| TextFont { font_size: FontSize::Px(size), ..default() };
    let shadow = TextShadow { offset: Vec2::splat(1.5), color: Color::srgba(0.0, 0.0, 0.0, 0.8) };

    // Timer, top center.
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            top: px(16),
            width: percent(100),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_child((TimerText, Text::new(""), text(34.0), shadow));
    // Score or level, top left.
    commands.spawn((
        ScoreText,
        Text::new(""),
        text(26.0),
        shadow,
        Node { position_type: PositionType::Absolute, top: px(18), left: px(24), ..default() },
    ));
    // Accuracy and kills, top right.
    commands.spawn((
        StatsText,
        Text::new(""),
        text(20.0),
        TextLayout::justify(Justify::Right),
        shadow,
        Node { position_type: PositionType::Absolute, top: px(18), right: px(24), ..default() },
    ));
    // Workout progress, bottom left.
    commands.spawn((
        StepText,
        Text::new(""),
        text(15.0),
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.75)),
        shadow,
        Node { position_type: PositionType::Absolute, bottom: px(10), left: px(14), ..default() },
    ));
    commands.spawn((
        FpsText,
        Text::new(""),
        text(14.0),
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.6)),
        Node { position_type: PositionType::Absolute, bottom: px(10), right: px(14), ..default() },
    ));
    // Countdown and prompts, middle of the screen.
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            top: percent(30),
            width: percent(100),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_child((CenterText, Text::new(""), text(64.0), TextLayout::justify(Justify::Center), shadow));

    spawn_crosshair(&mut commands, &game.session.settings);

    // Results, hidden until the run ends.
    commands
        .spawn((
            Overlay,
            Visibility::Hidden,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.03, 0.05, 0.72)),
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        padding: UiRect::all(px(32)),
                        min_width: px(460),
                        border_radius: BorderRadius::all(px(14)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.09, 0.1, 0.13)),
                ))
                .with_children(|panel| {
                    panel.spawn((OverlayText, Text::new(""), text(21.0), TextLayout::justify(Justify::Center)));
                    results_panel::spawn(panel);
                    panel.spawn((
                        KeysText,
                        Text::new(""),
                        text(21.0),
                        TextLayout::justify(Justify::Center),
                        Node { margin: UiRect::top(px(18)), ..default() },
                    ));
                });
        });
}

fn spawn_crosshair(commands: &mut Commands, settings: &PlayerSettings) {
    let c = &settings.crosshair;
    let col = color(&c.color);
    let (len, th, gap) = (c.size, c.thickness.max(1.0), c.gap);
    let mut bar = |x: f32, y: f32, w: f32, h: f32| {
        commands.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: percent(50),
                top: percent(50),
                width: px(w),
                height: px(h),
                margin: UiRect { left: px(x - w / 2.0), top: px(y - h / 2.0), ..default() },
                ..default()
            },
            BackgroundColor(col),
            Outline { width: px(1), offset: px(0), color: Color::srgba(0.0, 0.0, 0.0, 0.6) },
        ));
    };
    if c.dot {
        bar(0.0, 0.0, th, th);
    }
    if len > 0.0 {
        let off = gap + len / 2.0 + th / 2.0;
        bar(-off, 0.0, len, th);
        bar(off, 0.0, len, th);
        bar(0.0, -off, th, len);
        bar(0.0, off, th, len);
    }
}

fn keys(keys: Res<ButtonInput<KeyCode>>, mut game: ResMut<Game>, mut exit: MessageWriter<AppExit>) {
    if keys.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
    let finished = game.sim.phase() == Phase::Finished;
    let i = game.session.index;
    if keys.just_pressed(KeyCode::KeyR) {
        game.begin_step(i);
    } else if finished && keys.just_pressed(KeyCode::Space) {
        if game.is_last() {
            if game.session.steps.len() == 1 {
                game.begin_step(i);
            } else {
                exit.write(AppExit::Success);
            }
        } else {
            game.begin_step(i + 1);
        }
    }
    if finished && game.session.handoff {
        let open = if keys.just_pressed(KeyCode::KeyV) {
            game.run.extras.as_ref().filter(|e| e.replay).map(|e| Open::Replay(e.run_id))
        } else if keys.just_pressed(KeyCode::KeyI) {
            Some(Open::Scenario(game.sim.scenario().name.clone()))
        } else {
            None
        };
        if let Some(open) = open {
            eprintln!("{}", open.line());
            exit.write(AppExit::Success);
        }
    }
    if keys.just_pressed(KeyCode::F2) {
        game.session.settings.show_fps = !game.session.settings.show_fps;
    }
}

/// How often the lock is applied again while the window has focus.
const RELOCK_S: f32 = 0.5;

/// What the cursor system knows between frames.
#[derive(Resource, Default)]
struct CursorLock {
    focused: bool,
    since_lock: f32,
}

#[derive(Debug, PartialEq, Eq)]
enum Grab {
    Lock,
    Release,
    Keep,
}

/// Whether to lock the cursor this frame. On Windows a lock confines the cursor to
/// the window, and Windows drops that when focus moves away. After a quick alt-tab
/// the game may never see the window lose focus, so its own state still says
/// "locked" while the cursor is free. So the lock is applied again whenever focus
/// comes back, on every click, and every `RELOCK_S` while focused, whatever the
/// state says.
fn grab_action(lock: &mut CursorLock, focused: bool, clicked: bool, locked: bool, dt: f32) -> Grab {
    let regained = focused && !lock.focused;
    lock.focused = focused;
    if !focused {
        return if locked { Grab::Release } else { Grab::Keep };
    }
    lock.since_lock += dt;
    if regained || clicked || !locked || lock.since_lock >= RELOCK_S {
        lock.since_lock = 0.0;
        return Grab::Lock;
    }
    Grab::Keep
}

/// Locks the cursor while the window has focus (see `grab_action`).
fn cursor(
    window: Single<&Window>,
    mut cursor: Single<&mut CursorOptions>,
    mouse: Res<ButtonInput<MouseButton>>,
    time: Res<Time>,
    mut lock: ResMut<CursorLock>,
    game: Res<Game>,
) {
    if game.smoke.is_some() {
        return;
    }
    let locked = cursor.grab_mode == CursorGrabMode::Locked;
    match grab_action(&mut lock, window.focused, mouse.just_pressed(MouseButton::Left), locked, time.delta_secs()) {
        // Assigning marks the options changed, so Bevy applies the lock again even
        // when the value is the same.
        Grab::Lock => {
            cursor.grab_mode = CursorGrabMode::Locked;
            cursor.visible = false;
        }
        Grab::Release => {
            cursor.grab_mode = CursorGrabMode::None;
            cursor.visible = true;
        }
        Grab::Keep => {}
    }
}

fn look(
    motion: Res<AccumulatedMouseMotion>,
    cursor: Single<&CursorOptions>,
    mut game: ResMut<Game>,
    mut camera: Single<&mut Transform, With<PlayerCamera>>,
) {
    if cursor.grab_mode == CursorGrabMode::Locked && game.sim.phase() != Phase::Finished {
        let k = f64::from(game.session.settings.degrees_per_count());
        let d = motion.delta;
        game.look.yaw = (game.look.yaw + f64::from(d.x) * k).rem_euclid(360.0);
        if game.look.yaw > 180.0 {
            game.look.yaw -= 360.0;
        }
        game.look.pitch = (game.look.pitch - f64::from(d.y) * k).clamp(-89.0, 89.0);
    }
    let l = game.look;
    camera.rotation = Quat::from_euler(EulerRot::YXZ, -(l.yaw as f32).to_radians(), (l.pitch as f32).to_radians(), 0.0);
}

fn step(
    time: Res<Time>,
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Single<&CursorOptions>,
    camera: Single<&Camera, With<PlayerCamera>>,
    window: Single<&Window>,
    mut game: ResMut<Game>,
) {
    let dt = time.delta_secs();
    if dt > 0.0 {
        game.fps = if game.fps == 0.0 { 1.0 / dt } else { game.fps * 0.95 + 0.05 / dt };
    }
    // No shooting while the cursor is free (window not focused).
    let locked = cursor.grab_mode == CursorGrabMode::Locked;
    let mut input = Input {
        aim: game.look.aim(),
        fire_pressed: locked && mouse.just_pressed(MouseButton::Left),
        fire_held: locked && mouse.pressed(MouseButton::Left),
    };
    if game.smoke.is_some() {
        let Game { bot, sim, look, .. } = &mut *game;
        input = bot.input(sim, &mut look.yaw, &mut look.pitch, dt);
    }
    let was = game.sim.phase();
    let events = game.sim.step(dt, input);
    let now = game.sim.phase();
    if was == Phase::Countdown && now != Phase::Countdown {
        game.run.started_at = Some(Local::now().naive_local());
    }
    if now == Phase::Running {
        game.run.frames += 1;
        game.run.run_time += dt;
    }
    for e in events {
        match e {
            Event::Hit { id } => {
                game.run.flashes.insert(id, 0.06);
            }
            Event::Breakthrough { value } => {
                if let Some((knob, _, _)) = game.sim.rating() {
                    game.run.banner = Some((format!("Breakthrough: held it at {}!", fmt_value(knob, value)), 3.0));
                }
            }
            Event::Finished => {
                let size = camera
                    .physical_viewport_size()
                    .unwrap_or(UVec2::new(window.physical_width(), window.physical_height()));
                game.run.result = Some(game.sim.result());
                save_run(&mut game, format!("{}x{}", size.x, size.y));
                game.record_done();
            }
            _ => {}
        }
    }
    for f in game.run.flashes.values_mut() {
        *f -= dt;
    }
    if let Some((_, left)) = &mut game.run.banner {
        *left -= dt;
        if *left <= 0.0 {
            game.run.banner = None;
        }
    }
    game.run.flashes.retain(|_, f| *f > 0.0);
}

fn save_run(game: &mut Game, resolution: String) {
    let Some(path) = game.session.db_path.clone() else {
        game.run.saved = Some(Err("not saved (no database)".into()));
        return;
    };
    let finished_at = Local::now().naive_local();
    let result = game.run.result.clone().unwrap_or_else(|| game.sim.result());
    let avg_fps = (game.run.run_time > 0.0).then(|| game.run.frames as f32 / game.run.run_time);
    let info = RunInfo {
        scenario: game.sim.scenario(),
        settings: &game.session.settings,
        result: &result,
        started_at: game.run.started_at.unwrap_or(finished_at),
        finished_at,
        avg_fps,
        resolution: Some(resolution),
        workout: game.session.workout.clone(),
        program: game.session.program.clone(),
        rep: Some(game.step_info().rep),
        reps: Some(game.step_info().reps),
        block: Some(game.step_info().block),
    };
    let saved = record::save(&path, &info);
    if let Ok(id) = &saved {
        game.run.extras = record::extras(&path, game.sim.scenario(), *id);
    }
    game.run.saved = Some(saved);
}

fn sync_targets(
    mut commands: Commands,
    game: Res<Game>,
    assets: Option<Res<TargetAssets>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut views: Query<(Entity, &TargetView, &mut Transform)>,
) {
    let Some(assets) = assets else { return };
    if game.world_dirty {
        return;
    }
    let live: HashMap<u64, &aimadapt_sim::sim::Target> = game.sim.targets().iter().map(|t| (t.id, t)).collect();
    let mut shown = Vec::new();
    for (entity, view, mut tf) in &mut views {
        let Some(t) = live.get(&view.id) else {
            commands.entity(entity).despawn();
            continue;
        };
        shown.push(view.id);
        tf.translation = t.pos;
        tf.scale = Vec3::splat(t.radius / body_of(&assets, t).1);
        if let Some(mut m) = materials.get_mut(&view.material) {
            m.base_color =
                target_color(base_color(&assets, t), t.health_fraction(), game.run.flashes.contains_key(&t.id));
        }
    }
    for t in game.sim.targets() {
        if shown.contains(&t.id) {
            continue;
        }
        let material = materials.add(StandardMaterial {
            base_color: base_color(&assets, t),
            perceptual_roughness: 0.5,
            ..default()
        });
        let (mesh, radius) = body_of(&assets, t);
        commands.spawn((
            TargetView { id: t.id, material: material.clone() },
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(t.pos).with_scale(Vec3::splat(t.radius / radius)),
        ));
    }
}

/// A target's mesh and the radius it was made at: its kind's, or the scenario's.
fn body_of<'a>(assets: &'a TargetAssets, t: &aimadapt_sim::sim::Target) -> (&'a Handle<Mesh>, f32) {
    match t.kind().and_then(|k| assets.kinds.get(k as usize)) {
        Some((mesh, radius, _)) => (mesh, *radius),
        None => (&assets.mesh, assets.radius),
    }
}

/// A target's own color. When the scenario alternates colors, timed targets
/// change color by wave: with three targets up, the first three are one color, the
/// next three the other, and so on. A wave's color says how old its targets are.
fn base_color(assets: &TargetAssets, t: &aimadapt_sim::sim::Target) -> Color {
    if let Some((_, _, c)) = t.kind().and_then(|k| assets.kinds.get(k as usize)) {
        return *c;
    }
    let wave = (t.no().max(1) - 1) / assets.wave;
    match assets.alt {
        Some(alt) if t.time_left().is_some() && wave % 2 == 1 => alt,
        _ => assets.base,
    }
}

/// How far above a target's center its bars start: the top of the sphere, capsule or box.
fn bar_base(_s: &Scenario, t: &aimadapt_sim::sim::Target) -> f32 {
    match t.shape() {
        Shape::Capsule | Shape::Box => (t.height() / 2.0).max(t.radius),
        Shape::Sphere => t.radius,
    }
}

/// The bars a target shows, top to bottom order from the target up, with how
/// full each is.
fn bars_for(s: &Scenario, t: &aimadapt_sim::sim::Target) -> Vec<(Bar, f32)> {
    let mut out = Vec::new();
    if s.targets.health_bar {
        out.push((Bar::Health, t.health_fraction()));
    }
    if s.targets.timer_bar
        && let Some(f) = t.time_left_fraction()
    {
        out.push((Bar::TimeLeft, f));
    }
    out
}

/// Draws health and time-left bars just above each target, level with the screen
/// and the same size on screen at any distance.
fn sync_bars(
    mut commands: Commands,
    game: Res<Game>,
    assets: Option<Res<TargetAssets>>,
    camera: Single<&Transform, With<PlayerCamera>>,
    mut views: Query<(Entity, &BarView, &mut Transform), Without<PlayerCamera>>,
) {
    let Some(assets) = assets else { return };
    if game.world_dirty {
        return;
    }
    let s = game.sim.scenario();
    let mut want: HashMap<(u64, u8, bool), Transform> = HashMap::new();
    // Parallel to the screen, so bars read level wherever the target is.
    let rotation = camera.rotation;
    let (right, up, back_dir) = (rotation * Vec3::X, rotation * Vec3::Y, rotation * Vec3::Z);
    for t in game.sim.targets() {
        // Depth along the view, so the bar's size on screen doesn't change with distance.
        let d = (t.pos - camera.translation).dot(-back_dir).max(1.0);
        let (w, h) = (d * 0.06, d * 0.06 * 0.16);
        for (k, (bar, frac)) in bars_for(s, t).into_iter().enumerate() {
            let center = t.pos + up * (bar_base(s, t) + h * (1.0 + 1.5 * k as f32));
            let back = Transform { translation: center, rotation, scale: Vec3::new(w, h, 1.0) };
            let inner = w - h * 0.4;
            let fill_w = inner * frac;
            let fill = Transform {
                translation: center - right * (inner - fill_w) / 2.0 + back_dir * 0.02,
                rotation,
                scale: Vec3::new(fill_w.max(1e-4), h * 0.6, 1.0),
            };
            want.insert((t.id, bar as u8, false), back);
            want.insert((t.id, bar as u8, true), fill);
        }
    }
    for (entity, view, mut tf) in &mut views {
        match want.remove(&(view.id, view.kind as u8, view.fill)) {
            Some(next) => *tf = next,
            None => commands.entity(entity).despawn(),
        }
    }
    for ((id, kind, fill), tf) in want {
        let kind = if kind == Bar::Health as u8 { Bar::Health } else { Bar::TimeLeft };
        let material = match (fill, kind) {
            (false, _) => assets.bar_back.clone(),
            (true, Bar::Health) => assets.bar_health.clone(),
            (true, Bar::TimeLeft) => assets.bar_time.clone(),
        };
        commands.spawn((BarView { id, kind, fill }, Mesh3d(assets.bar.clone()), MeshMaterial3d(material), tf));
    }
}

/// Targets fade toward white as they lose health and brighten while being hit
/// (for a beam that's every tick on target, so it doubles as on-target feedback).
fn target_color(base: Color, health: f32, flash: bool) -> Color {
    let faded = base.mix(&Color::WHITE, (1.0 - health) * 0.6);
    if flash { faded.mix(&Color::WHITE, 0.55) } else { faded }
}

fn mode_name(adaptive: bool) -> &'static str {
    if adaptive { "adaptive" } else { "standard" }
}

#[allow(clippy::type_complexity)]
fn update_hud(
    game: Res<Game>,
    mut texts: ParamSet<(
        Single<&mut Text, With<TimerText>>,
        Single<&mut Text, With<ScoreText>>,
        Single<&mut Text, With<StatsText>>,
        Single<&mut Text, With<CenterText>>,
        Single<&mut Text, With<FpsText>>,
        Single<&mut Text, With<StepText>>,
    )>,
) {
    let sim = &game.sim;
    let st = sim.stats();
    texts.p0().0 = format!("{:.1}", sim.time_left().max(0.0));
    let acc = st.accuracy().map_or("-".into(), |a| format!("{:.1}%", a * 100.0));
    match sim.rating() {
        Some((knob, value, rate)) => {
            let goal = sim.scenario().adaptive.goal * 100.0;
            let plan = sim.run_plan();
            texts.p1().0 = match plan {
                Some((v, chance, pick)) => run_intro(pick, &fmt_value(knob, v), chance, sim.scenario().adaptive.goal),
                None => {
                    let note = sim.break_days().map_or(String::new(), |d| format!("\n{}", break_note(d)));
                    format!("Calibration run{note}\n{goal:.0}%: {}", fmt_value(knob, value))
                }
            };
            let rate = rate.map_or("-".into(), |r| format!("{:.0}%", r * 100.0));
            let target = rate_target(plan.map(|(_, chance, pick)| (chance, pick)), sim.scenario().adaptive.goal);
            let mut stats =
                format!("{} {rate} ({target})\n{}", rate_word(knob, sim.rated_per_target()), kills_line(sim));
            if let Some((_, left)) = sim.pace() {
                stats += &if left >= 0.0 { format!("\nNext kill within {left:.1} s") } else { "\nOff pace".into() };
            }
            texts.p2().0 = stats;
        }
        None => {
            texts.p1().0 = format!("{:.0}", sim.score());
            texts.p2().0 = format!("Accuracy {acc}\n{}", kills_line(sim));
        }
    }
    texts.p3().0 = match sim.phase() {
        Phase::Countdown => format!("{}", sim.countdown_left().ceil() as u32),
        _ => game.run.banner.as_ref().map(|b| b.0.clone()).unwrap_or_default(),
    };
    texts.p4().0 = if game.session.settings.show_fps { format!("{:.0} fps", game.fps) } else { String::new() };
    let step = game.step_info();
    let mut line = format!("{} | {}", sim.scenario().name, mode_name(step.adaptive));
    if let Some(l) = &step.label {
        line = format!("{line} | {l}");
    }
    if let Some(w) = &game.session.workout {
        line = format!("{w} | {} of {} | {line}", game.session.index + 1, game.session.steps.len());
    }
    texts.p5().0 = line;
}

/// Whole days, rounded to the nearest: "1 day", "9 days".
fn days_text(days: f32) -> String {
    let n = days.round().max(1.0) as u32;
    format!("{n} {}", if n == 1 { "day" } else { "days" })
}

/// Why a calibration run follows a long break, for the HUD.
fn break_note(days: f32) -> String {
    format!("{} since your last run, finding your level again", days_text(days))
}

/// The first words of a calibration run's results.
fn calibration_title(break_days: Option<f32>) -> String {
    break_days.map_or("Calibration run".into(), |d| format!("Calibration run after {} away", days_text(d)))
}

/// The closing line of a calibration run's results.
fn calibration_close(break_days: Option<f32>) -> &'static str {
    if break_days.is_some() {
        "Runs go back to one difficulty from here."
    } else {
        "From here on, each run plays at one difficulty."
    }
}

/// What to expect from a run planned at this chance of success: "about 6 in 10", or
/// out of 20 for the likelier runs.
fn expected_text(chance: f32) -> String {
    let out_of = if chance >= 0.9 { 20.0 } else { 10.0 };
    format!("about {} in {}", (chance * out_of).round() as u32, out_of as u32)
}

/// The HUD text before and during a fixed-difficulty run: its kind, its difficulty and
/// what to expect. `goal` and `chance` are fractions.
fn run_intro(pick: Pick, value_text: &str, chance: f32, goal: f32) -> String {
    let label = pick.run_label();
    match pick {
        Pick::Stretch | Pick::Easy => {
            let expected = expected_text(chance).replacen("about", "About", 1);
            format!("{label}: {value_text}\n{expected} expected")
        }
        _ => format!("{label}: {value_text}\nGoal {:.0}%", goal * 100.0),
    }
}

/// What the live success rate is measured against, for the HUD: the plan on a stretch
/// or easy run, the goal on any other. `plan` is the run's planned chance and kind.
fn rate_target(plan: Option<(f32, Pick)>, goal: f32) -> String {
    match plan {
        Some((chance, Pick::Stretch | Pick::Easy)) => format!("{} expected", expected_text(chance)),
        _ => format!("goal {:.0}%", goal * 100.0),
    }
}

/// What the results need to say how a fixed-difficulty run went. `goal`, `rate` and
/// `planned` are fractions (0.85); `evidence` is in target-equivalents.
#[derive(Clone, Copy)]
struct RunJudgment {
    pick: Pick,
    goal: f32,
    rate: Option<f32>,
    planned: Option<f32>,
    evidence: f32,
    held: bool,
    breakthrough: bool,
}

/// The verdict line after a fixed run. A run at your level is judged against the goal
/// (it can be at the goal rate and still not hold it, when it was too short to count).
/// A stretch or easy run is judged against the chance it was planned at.
fn run_verdict(j: RunJudgment) -> String {
    let pct = j.goal * 100.0;
    if j.breakthrough {
        return format!("Breakthrough: you held {pct:.0}% faster than ever before");
    }
    let kind = match j.pick {
        Pick::Stretch => "a stretch run",
        Pick::Easy => "an easy run",
        _ => {
            return if j.held {
                format!("You held the {pct:.0}% goal")
            } else if j.rate.is_some_and(|r| r >= j.goal) {
                format!("At the {pct:.0}% goal, but too few targets to count as held")
            } else {
                format!("Below the {pct:.0}% goal this time")
            };
        }
    };
    if j.pick == Pick::Stretch && j.held {
        return format!("Above plan: you held the {pct:.0}% goal on a stretch run");
    }
    let (Some(rate), Some(planned)) = (j.rate, j.planned) else {
        return "Too few targets to judge against the plan".into();
    };
    let expected = expected_text(planned);
    match judge_against_plan(planned, rate, j.evidence) {
        PlanVerdict::Below => format!("Below plan for {kind}: {expected} expected"),
        PlanVerdict::AsPlanned => format!("As planned for {kind}: {expected} expected"),
        PlanVerdict::Above => format!("Above plan for {kind}: {expected} expected"),
    }
}

fn update_overlay(
    game: Res<Game>,
    mut overlay: Single<&mut Visibility, With<Overlay>>,
    mut text: Single<&mut Text, (With<OverlayText>, Without<KeysText>)>,
    mut keys_text: Single<&mut Text, (With<KeysText>, Without<OverlayText>)>,
) {
    let finished = game.sim.phase() == Phase::Finished;
    **overlay = if finished { Visibility::Visible } else { Visibility::Hidden };
    let Some(r) = game.run.result.as_ref().filter(|_| finished) else {
        return;
    };
    let st = &r.stats;
    let acc = st.accuracy().map_or("-".into(), |a| format!("{:.1}%", a * 100.0));
    let ttk = r.avg_ttk_s().map_or("-".into(), |t| format!("{:.0} ms", t * 1000.0));
    let headline = match &r.adaptive {
        Some(a) => {
            let k = a.knob;
            let better = |x: f32, y: f64| if k.higher_is_harder() { f64::from(x) > y } else { f64::from(x) < y };
            let best = match game.run.best {
                None => "First adaptive run on this scenario".to_string(),
                Some(b) if better(a.value, b) => format!("Your best yet! (was {})", fmt_value(k, b as f32)),
                Some(b) => format!("Best {}", fmt_value(k, b as f32)),
            };
            let (lo, hi) = (a.range.0.min(a.range.1), a.range.0.max(a.range.1));
            let goal = a.goal * 100.0;
            let hit = a.success_rate.unwrap_or(0.0) * 100.0;
            match a.run_value {
                None => format!(
                    "{}: {goal:.0}% at {}   (likely {} to {})\n{} {hit:.0}% | {best}\n{}",
                    calibration_title(a.break_days),
                    fmt_value(k, a.value),
                    fmt_value(k, lo),
                    fmt_value(k, hi),
                    rate_word(k, game.sim.rated_per_target()),
                    calibration_close(a.break_days),
                ),
                Some(v) => {
                    let verdict = run_verdict(RunJudgment {
                        pick: a.pick,
                        goal: a.goal,
                        rate: a.success_rate,
                        planned: a.planned_chance,
                        evidence: a.evidence,
                        held: a.fastest.is_some(),
                        breakthrough: a.breakthrough,
                    });
                    format!(
                        "{}: {}   {} {hit:.0}%\n{verdict}\nRating {} (was {}) | {best}",
                        a.pick.run_label(),
                        fmt_value(k, v),
                        rate_word(k, game.sim.rated_per_target()),
                        fmt_value(k, a.value),
                        fmt_value(k, a.start_value),
                    )
                }
            }
        }
        None => {
            let score = f64::from(r.score);
            let best = match game.run.best {
                None => "First run of this scenario".to_string(),
                Some(b) if score > b => format!("New best! (was {b:.0})"),
                Some(b) => format!("Best {b:.0}"),
            };
            format!("Score {:.0}\n{best}", r.score)
        }
    };
    let saved = match &game.run.saved {
        Some(Ok(_)) => "Saved to your history".to_string(),
        Some(Err(e)) => format!("Couldn't save: {e}"),
        None => String::new(),
    };
    let note = game.display_note.as_deref().map(|n| format!("\n{n}")).unwrap_or_default();
    text.0 = format!(
        "{}\n\n{headline}\n\nAccuracy {acc}   {}   Timeouts {}   Avg TTK {ttk}\n\n{saved}{note}",
        game.sim.scenario().name,
        kills_line(&game.sim),
        st.timeouts,
    );
    keys_text.0 = keys_block(&game);
}

/// The workout lines and the key hints under the results.
fn keys_block(game: &Game) -> String {
    let hints = results::keys_text(Keys {
        steps: game.session.steps.len(),
        last: game.is_last(),
        handoff: game.session.handoff,
        replay: game.run.extras.as_ref().is_some_and(|e| e.replay),
    });
    if game.session.steps.len() == 1 {
        hints
    } else if game.is_last() {
        let summary = results::workout_summary(&game.session.done);
        format!("Workout complete\n{}\n\n{hints}", summary.join("\n"))
    } else {
        let next = &game.session.steps[game.session.index + 1];
        format!(
            "Next ({} of {}): {} | {}\n\n{hints}",
            game.session.index + 2,
            game.session.steps.len(),
            next.scenario.name,
            mode_name(next.adaptive)
        )
    }
}

/// Smoke test: screenshot mid-run and on the results screen, move to the next
/// step, and quit after the last.
fn smoke_capture(mut commands: Commands, time: Res<Time>, mut game: ResMut<Game>, mut exit: MessageWriter<AppExit>) {
    use bevy::render::view::screenshot::{Screenshot, save_to_disk};
    let elapsed = game.sim.elapsed();
    let finished = game.sim.phase() == Phase::Finished;
    let (i, last) = (game.session.index, game.is_last());
    let Some(smoke) = game.smoke.as_mut() else { return };
    let suffix = if i == 0 { String::new() } else { format!("-{}", i + 1) };
    let mut shot = |name: &str, smoke: &mut Smoke| {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(smoke.dir.join(format!("{name}{suffix}.png"))));
        smoke.taken += 1;
    };
    if smoke.taken == 0 && elapsed >= 1.5 {
        shot("play", smoke);
    }
    if finished {
        smoke.after_finish += time.delta_secs();
        if smoke.taken == 1 && smoke.after_finish >= 0.5 {
            shot("results", smoke);
        }
        if smoke.after_finish >= 1.5 {
            if last {
                exit.write(AppExit::Success);
            } else {
                game.begin_step(i + 1);
            }
        }
    }
}

/// A rating in the knob's units. The game font has no degree sign, so speeds read
/// "deg/s" in game.
fn fmt_value(knob: Knob, v: f32) -> String {
    knob.format(v).replace("°/s", "deg/s")
}

fn kills_line(sim: &Sim) -> String {
    let kills = sim.stats().kills;
    match sim.kills_per_min() {
        Some(k) => format!("Kills {kills} ({k:.0}/min)"),
        None => format!("Kills {kills}"),
    }
}

fn rate_word(knob: Knob, per_target: bool) -> &'static str {
    match knob {
        Knob::TargetSpeed if per_target => "Finished in time",
        Knob::TargetSpeed => "On target",
        // Precise Track: size rated per tracking segment.
        Knob::Radius if !per_target => "On target",
        Knob::KillPace => "On pace",
        _ => "Hit rate",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn judgment(pick: Pick, rate: Option<f32>, planned: f32) -> RunJudgment {
        RunJudgment {
            pick,
            goal: 0.85,
            rate,
            planned: Some(planned),
            evidence: 100.0,
            held: false,
            breakthrough: false,
        }
    }

    /// Bug: a run at 90% but too short to hold the goal said "Below the 85% goal".
    #[test]
    fn the_run_verdict_names_all_four_outcomes() {
        let near = |rate, held, breakthrough| RunJudgment { held, breakthrough, ..judgment(Pick::Near, rate, 0.85) };
        assert_eq!(run_verdict(near(Some(0.95), true, true)), "Breakthrough: you held 85% faster than ever before");
        assert_eq!(run_verdict(near(Some(0.95), true, false)), "You held the 85% goal");
        assert_eq!(run_verdict(near(Some(0.9), false, false)), "At the 85% goal, but too few targets to count as held");
        assert_eq!(run_verdict(near(Some(0.7), false, false)), "Below the 85% goal this time");
        assert_eq!(run_verdict(near(None, false, false)), "Below the 85% goal this time");
    }

    /// Bug: a stretch run planned at 60% that came out at 58% read "Below the 85% goal
    /// this time", as if it had failed.
    #[test]
    fn a_stretch_run_near_its_plan_is_not_called_below_the_goal() {
        let verdict = run_verdict(judgment(Pick::Stretch, Some(0.58), 0.6));
        assert_eq!(verdict, "As planned for a stretch run: about 6 in 10 expected");
        assert!(!verdict.contains("Below the"));
    }

    #[test]
    fn stretch_and_easy_runs_are_judged_against_their_plan() {
        let v = |pick, rate, planned| run_verdict(judgment(pick, Some(rate), planned));
        assert_eq!(v(Pick::Stretch, 0.6, 0.6), "As planned for a stretch run: about 6 in 10 expected");
        assert_eq!(v(Pick::Stretch, 0.8, 0.6), "Above plan for a stretch run: about 6 in 10 expected");
        assert_eq!(v(Pick::Stretch, 0.4, 0.6), "Below plan for a stretch run: about 6 in 10 expected");
        assert_eq!(v(Pick::Easy, 0.95, 0.95), "As planned for an easy run: about 19 in 20 expected");
        assert_eq!(v(Pick::Easy, 0.97, 0.93), "As planned for an easy run: about 19 in 20 expected");
        assert_eq!(v(Pick::Easy, 0.8, 0.95), "Below plan for an easy run: about 19 in 20 expected");
        assert_eq!(v(Pick::Easy, 1.0, 0.8), "Above plan for an easy run: about 8 in 10 expected");
    }

    #[test]
    fn a_held_stretch_run_is_above_plan_and_a_breakthrough_still_leads() {
        let held = RunJudgment { held: true, ..judgment(Pick::Stretch, Some(0.9), 0.6) };
        assert_eq!(run_verdict(held), "Above plan: you held the 85% goal on a stretch run");
        let broke = RunJudgment { breakthrough: true, ..held };
        assert_eq!(run_verdict(broke), "Breakthrough: you held 85% faster than ever before");
        let easy_broke = RunJudgment { pick: Pick::Easy, ..broke };
        assert_eq!(run_verdict(easy_broke), "Breakthrough: you held 85% faster than ever before");
    }

    #[test]
    fn a_stretch_run_with_no_rate_or_no_plan_says_so() {
        let none = "Too few targets to judge against the plan";
        assert_eq!(run_verdict(judgment(Pick::Stretch, None, 0.6)), none);
        assert_eq!(run_verdict(RunJudgment { planned: None, ..judgment(Pick::Easy, Some(0.9), 0.95) }), none);
    }

    #[test]
    fn a_short_stretch_run_gets_room_for_luck() {
        let short = RunJudgment { evidence: 10.0, ..judgment(Pick::Stretch, Some(0.4), 0.6) };
        assert_eq!(run_verdict(short), "As planned for a stretch run: about 6 in 10 expected");
    }

    #[test]
    fn expected_text_rounds_to_tenths_or_twentieths() {
        assert_eq!(expected_text(0.55), "about 6 in 10");
        assert_eq!(expected_text(0.64), "about 6 in 10");
        assert_eq!(expected_text(0.72), "about 7 in 10");
        assert_eq!(expected_text(0.93), "about 19 in 20");
        assert_eq!(expected_text(0.97), "about 19 in 20");
        assert_eq!(expected_text(0.89), "about 9 in 10");
        assert_eq!(expected_text(0.9), "about 18 in 20");
    }

    #[test]
    fn the_hud_names_the_kind_of_run_and_what_to_expect() {
        assert_eq!(run_intro(Pick::Stretch, "0.52 s", 0.6, 0.85), "Stretch run: 0.52 s\nAbout 6 in 10 expected");
        assert_eq!(run_intro(Pick::Easy, "0.71 s", 0.95, 0.85), "Easy run: 0.71 s\nAbout 19 in 20 expected");
        assert_eq!(run_intro(Pick::Near, "0.60 s", 0.85, 0.85), "At your level: 0.60 s\nGoal 85%");
    }

    /// The live rate on a stretch run sat beside "(goal 85%)", so a run going to plan
    /// looked like one going badly. It sits beside the plan now.
    #[test]
    fn the_live_rate_is_shown_against_the_plan_on_stretch_and_easy_runs() {
        assert_eq!(rate_target(Some((0.6, Pick::Stretch)), 0.85), "about 6 in 10 expected");
        assert_eq!(rate_target(Some((0.95, Pick::Easy)), 0.85), "about 19 in 20 expected");
        assert_eq!(rate_target(Some((0.85, Pick::Near)), 0.85), "goal 85%");
        assert_eq!(rate_target(None, 0.85), "goal 85%", "a calibration run");
    }

    /// The calibration run after a break says why, in whole days.
    #[test]
    fn the_break_note_gives_whole_days() {
        assert_eq!(break_note(9.4), "9 days since your last run, finding your level again");
        assert_eq!(break_note(9.6), "10 days since your last run, finding your level again");
        assert!(break_note(3.0).starts_with("3 days since"));
        assert_eq!(calibration_title(Some(9.0)), "Calibration run after 9 days away");
        assert_eq!(calibration_title(None), "Calibration run");
        assert_eq!(calibration_close(Some(9.0)), "Runs go back to one difficulty from here.");
        assert_eq!(calibration_close(None), "From here on, each run plays at one difficulty.");
    }

    fn builtin(id: &str) -> Scenario {
        let e = aimadapt_sim::library::builtins().into_iter().find(|e| e.id == id).unwrap();
        let mut s = e.scenario.unwrap();
        s.timing.countdown_s = 0.0;
        s
    }

    fn assets(s: &Scenario) -> TargetAssets {
        TargetAssets {
            mesh: Handle::default(),
            base: color(&s.targets.color),
            kinds: Vec::new(),
            alt: s.targets.alternate_colors.then(|| color(&s.targets.alt_color)),
            wave: s.targets.count.max(1),
            radius: s.targets.radius,
            bar: Handle::default(),
            bar_back: Handle::default(),
            bar_health: Handle::default(),
            bar_time: Handle::default(),
        }
    }

    /// Playtest: with several timed targets up at once nothing showed which one
    /// was about to expire. Timed targets now show a time-left bar that empties.
    #[test]
    fn timed_targets_show_time_left() {
        let s = builtin("builtin:timed-grid");
        let mut sim = Sim::new(s.clone(), 1);
        let t0 = bars_for(&s, &sim.targets()[0]);
        assert_eq!(t0.len(), 1);
        assert_eq!(t0[0].0, Bar::TimeLeft);
        assert!(t0[0].1 > 0.99);
        for _ in 0..120 {
            sim.step(1.0 / 240.0, Input { aim: Vec3::Y, fire_pressed: false, fire_held: false });
        }
        let id = sim.targets()[0].id;
        let later = bars_for(&s, sim.targets().iter().find(|t| t.id == id).unwrap());
        assert!(later[0].1 < 0.8, "half a second of a 2 s timer gone: {}", later[0].1);
        // Untimed targets have no timer bar.
        let track = builtin("builtin:strafe-track");
        let sim = Sim::new(track.clone(), 1);
        assert!(bars_for(&track, &sim.targets()[0]).is_empty());
    }

    /// Playtest: alternating every target made Timed Grid open with mixed colors
    /// (red, blue, red), as confusing as one color. Colors go by wave instead: the
    /// first three are one color, the next three the other, and so on.
    #[test]
    fn timed_targets_change_color_by_wave() {
        let s = builtin("builtin:timed-grid");
        assert_eq!(s.targets.count, 3);
        let a = assets(&s);
        let mut sim = Sim::new(s.clone(), 1);
        let mut seen: std::collections::BTreeMap<u32, Color> = Default::default();
        for _ in 0..(240 * 20) {
            for t in sim.targets() {
                seen.insert(t.no(), base_color(&a, t));
            }
            let aim = sim.targets()[0].pos.normalize();
            sim.step(1.0 / 240.0, Input { aim, fire_pressed: true, fire_held: true });
            sim.step(1.0 / 240.0, Input { aim: Vec3::Y, fire_pressed: false, fire_held: false });
        }
        assert!(seen.len() >= 9, "{} targets", seen.len());
        for (&no, &c) in &seen {
            let wave_color = if ((no - 1) / 3) % 2 == 0 { a.base } else { a.alt.unwrap() };
            assert_eq!(c, wave_color, "target {no}");
        }
        // One target at a time: every new target switches color.
        let one = builtin("builtin:timed-flicks");
        let a = assets(&one);
        let sim = Sim::new(one, 1);
        assert_eq!(base_color(&a, &sim.targets()[0]), a.base);
        // Untimed targets keep one color.
        let track = builtin("builtin:switch-track");
        let a = assets(&track);
        let sim = Sim::new(track, 1);
        assert!(sim.targets().iter().all(|t| base_color(&a, t) == a.base));
    }

    /// Bot check: on capsules (bodies) the health bar sat one radius above the
    /// center, inside the body, so Unbroken Strafes showed no bar. Bars go above the
    /// top of the target.
    #[test]
    fn bars_sit_above_the_top_of_the_target() {
        for id in ["builtin:unbroken-strafes", "builtin:stability-switch"] {
            let mut s = builtin(id);
            // Also try each of them as a box: the bars start at half its height.
            if id == "builtin:stability-switch" {
                s.targets.shape = Shape::Box;
                s.targets.height = 2.0 * s.targets.radius + 0.4;
            }
            let sim = Sim::new(s.clone(), 1);
            let t = &sim.targets()[0];
            let top = match s.targets.shape {
                Shape::Capsule | Shape::Box => s.targets.height / 2.0,
                Shape::Sphere => s.targets.radius,
            };
            assert!(
                bar_base(&s, t) >= top - 1e-4,
                "{id}: bars start {} above the center, top is {top}",
                bar_base(&s, t)
            );
        }
    }

    #[test]
    fn switching_targets_show_health() {
        let s = builtin("builtin:stability-switch");
        let sim = Sim::new(s.clone(), 1);
        let bars = bars_for(&s, &sim.targets()[0]);
        assert_eq!(bars, vec![(Bar::Health, 1.0)]);
    }

    /// Precise Track is rated on size per tracking segment: the HUD shows time on
    /// target, not a hit rate. Micro Flicks (size, clicking) still shows hit rate.
    #[test]
    fn size_ratings_name_what_they_count() {
        assert_eq!(rate_word(Knob::Radius, false), "On target");
        assert_eq!(rate_word(Knob::Radius, true), "Hit rate");
        assert_eq!(rate_word(Knob::KillPace, false), "On pace");
    }

    /// Playtest: after alt-tabbing out and back, the mouse was hard to lock to the
    /// game again. Windows drops the lock when focus moves away; after a quick
    /// alt-tab the game still thought it was locked, so it never locked it again,
    /// and clicking didn't help. Now it locks again on focus, on clicks and every
    /// half second, even when it thinks it's already locked.
    #[test]
    fn the_cursor_locks_again_after_alt_tab() {
        let mut l = CursorLock::default();
        let dt = 1.0 / 240.0;
        assert_eq!(grab_action(&mut l, true, false, false, dt), Grab::Lock, "locks on start");
        assert_eq!(grab_action(&mut l, true, false, true, dt), Grab::Keep);
        // Focus moves away: release it.
        assert_eq!(grab_action(&mut l, false, false, true, dt), Grab::Release);
        assert_eq!(grab_action(&mut l, false, false, false, dt), Grab::Keep);
        // Focus comes back: lock again.
        assert_eq!(grab_action(&mut l, true, false, false, dt), Grab::Lock);
        // The stale case: the game still thinks it's locked, but Windows dropped it.
        // A click locks it again, and so does the next periodic lock.
        assert_eq!(grab_action(&mut l, true, true, true, dt), Grab::Lock, "a click always locks");
        let mut relocked = false;
        for _ in 0..(RELOCK_S / dt) as usize + 2 {
            relocked |= grab_action(&mut l, true, false, true, dt) == Grab::Lock;
        }
        assert!(relocked, "locks again within {RELOCK_S} s without a click");
        // And a focus change the game sees as regaining focus locks straight away.
        l.focused = false;
        assert_eq!(grab_action(&mut l, true, false, true, dt), Grab::Lock);
    }
}
