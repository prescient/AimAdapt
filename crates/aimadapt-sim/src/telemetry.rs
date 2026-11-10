//! What the simulation records about how a run was played, for analytics. Because
//! AimAdapt runs the game itself, it can measure things a stats file never shows:
//! reaction time, how long each flick took for its distance, overshoot, where shots
//! land relative to the target, and how closely tracking follows the target.
//!
//! Angles are in degrees. Screen offsets use the player's view: +x is right, +y is up.

use glam::Vec3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Killed,
    TimedOut,
    /// Still alive when the run ended.
    Open,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Outcome::Killed => "killed",
            Outcome::TimedOut => "timed_out",
            Outcome::Open => "open",
        }
    }
}

/// One target, from when the player could start going for it to when it ended.
#[derive(Debug, Clone, PartialEq)]
pub struct TargetRecord {
    pub no: u32,
    pub spawned_at: f32,
    /// When the flick started: the later of the spawn and the previous kill, since
    /// with several targets up the player only goes for the next one after a kill.
    pub start_at: f32,
    pub ended_at: f32,
    pub outcome: Outcome,
    /// Angle from the crosshair to the target when the flick started.
    pub distance_deg: f32,
    /// Direction of that flick on screen: 0° = right, 90° = up, 180° = left, -90° = down.
    pub direction_deg: f32,
    /// Angular radius of the target at the start.
    pub radius_deg: f32,
    pub lifetime_s: f32,
    /// Seconds from the start until the crosshair had covered 10% of the distance.
    pub reaction_s: Option<f32>,
    /// Seconds from the start until the crosshair was first on the target.
    pub reach_s: Option<f32>,
    /// Furthest the crosshair went past the target's edge along the flick direction.
    pub overshoot_deg: f32,
    /// Times the crosshair moved onto the target (1 = straight on, more = corrections).
    pub entries: u32,
    pub shots: u32,
    pub hits: u32,
    /// Adaptive mode: the knob value this target (or tracking segment) was given,
    /// its planned chance of success, and whether it was a near, stretch, easy or
    /// calibration pick.
    pub value: Option<f32>,
    pub chance: Option<f32>,
    pub pick: Option<&'static str>,
}

/// One hitscan shot.
#[derive(Debug, Clone, PartialEq)]
pub struct ShotRecord {
    pub t: f32,
    pub hit: bool,
    /// The target hit, or the one nearest the crosshair for a miss.
    pub target_no: Option<u32>,
    /// Crosshair minus target center, in degrees on screen.
    pub dx_deg: f32,
    pub dy_deg: f32,
    /// The same offset in target radii, turned so +along points the way the flick
    /// went (positive = past the target, negative = short of it).
    pub along: f32,
    pub across: f32,
}

/// Tracking state, sampled 30 times a second for beam scenarios.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackSample {
    pub t: f32,
    pub on_target: bool,
    /// Angle from the crosshair to the target center.
    pub error_deg: f32,
    /// How fast the target moves across the view, degrees per second.
    pub target_speed_dps: f32,
    /// Crosshair offset along the target's motion: positive = ahead, negative = behind.
    pub lead_deg: f32,
    /// Seconds since the target last changed direction.
    pub since_change_s: f32,
    /// Some target is damaged and not yet healed (switching: you're on a kill).
    pub engaged: bool,
}

/// Where one target was at a moment, for replays.
#[derive(Debug, Clone, PartialEq)]
pub struct TargetPose {
    pub no: u32,
    /// Direction of its center from the player.
    pub yaw_deg: f32,
    pub pitch_deg: f32,
    /// Angular radius, and for capsules the angular height (0 for spheres).
    pub radius_deg: f32,
    pub height_deg: f32,
    /// Health left as a share of full health.
    pub health: f32,
}

/// Every target alive at one moment.
#[derive(Debug, Clone, PartialEq)]
pub struct TargetFrame {
    pub t: f32,
    pub targets: Vec<TargetPose>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Telemetry {
    pub targets: Vec<TargetRecord>,
    pub shots: Vec<ShotRecord>,
    pub samples: Vec<TrackSample>,
    /// (seconds, yaw°, pitch°) of the crosshair at up to 240 Hz, for replays.
    pub trace: Vec<[f32; 3]>,
    /// The targets at up to 60 Hz, for replays.
    pub frames: Vec<TargetFrame>,
}

/// Right and up unit vectors of a view looking along `d`.
pub fn basis(d: Vec3) -> (Vec3, Vec3) {
    let right = d.cross(Vec3::Y).normalize_or(Vec3::X);
    let up = right.cross(d).normalize_or(Vec3::Y);
    (right, up)
}

/// Where `p` appears relative to the center of a view along `view`, in degrees
/// (right, up).
pub fn screen_offset(view: Vec3, p: Vec3) -> (f32, f32) {
    let (r, u) = basis(view);
    let fwd = p.dot(view);
    (p.dot(r).atan2(fwd).to_degrees(), p.dot(u).atan2(fwd).to_degrees())
}

/// Angle in degrees between two directions.
pub fn angle_deg(a: Vec3, b: Vec3) -> f32 {
    a.normalize_or(Vec3::NEG_Z).angle_between(b.normalize_or(Vec3::NEG_Z)).to_degrees()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::from_angles;

    #[test]
    fn screen_offsets_have_the_right_signs() {
        let ahead = Vec3::NEG_Z;
        let (x, y) = screen_offset(ahead, from_angles(10.0, 0.0));
        assert!((x - 10.0).abs() < 1e-3 && y.abs() < 1e-3, "{x},{y}");
        let (x, y) = screen_offset(ahead, from_angles(0.0, -5.0));
        assert!(x.abs() < 1e-3 && (y + 5.0).abs() < 1e-3, "{x},{y}");
        // Looking up and to the left, a point further left is still at -x.
        let view = from_angles(-30.0, 20.0);
        let (x, _) = screen_offset(view, from_angles(-40.0, 20.0));
        assert!(x < -8.0, "{x}");
    }
}
