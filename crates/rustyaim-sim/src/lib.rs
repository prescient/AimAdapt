//! RustyAim's aim trainer rules, kept apart from rendering so they can be tested
//! and replayed without a window.
//!
//! - [`scenario`]: the scenario file format (TOML) and its validation.
//! - [`spec`]: a description of every scenario field, for the editor and docs.
//! - [`sim`]: spawning, movement, hit detection and scoring.
//! - [`settings`]: the player's sensitivity, field of view and crosshair.
//! - [`library`]: built-in scenarios and the player's own scenario folder.

pub mod library;
pub mod scenario;
pub mod settings;
pub mod sim;
pub mod spec;

pub use glam::Vec3;
pub use scenario::Scenario;
pub use settings::PlayerSettings;
pub use sim::{Event, Input, Phase, RunResult, Sim};
