//! AimAdapt's aim trainer rules, kept apart from rendering so they can be tested
//! and replayed without a window.
//!
//! - [`scenario`]: the scenario file format (TOML) and its validation.
//! - [`spec`]: a description of every scenario field, for the editor and docs.
//! - [`sim`]: spawning, movement, hit detection and scoring.
//! - [`settings`]: the player's sensitivity, field of view and crosshair.
//! - [`library`]: built-in scenarios and the player's own scenario folder.
//! - [`adaptive`]: the difficulty level and the staircase that keeps success near 85%.
//! - [`telemetry`]: per-target, per-shot and tracking records for analytics.
//! - [`workout`]: lists of scenarios played back to back.
//! - [`program`]: weekly plans that map weekdays to workouts.
//! - [`world`]: the map, spawn points and scoring of a scenario imported from KovaaK's.
//! - [`collide`]: an imported map's walls as solid triangles, for shots and moving bots.
//! - [`motion`]: how an imported scenario's bots move: KovaaK's dodge profiles on
//!   Unreal-style character movement.

pub mod adaptive;
pub mod collide;
pub mod library;
pub mod motion;
pub mod profiles;
pub mod program;
pub mod scenario;
pub mod settings;
pub mod sim;
pub mod spec;
pub mod telemetry;
pub mod workout;
pub mod world;

pub use glam::Vec3;
pub use scenario::Scenario;
pub use settings::PlayerSettings;
pub use sim::{Event, Input, Phase, RunResult, Sim};
