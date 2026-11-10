//! What an adaptive rating measures, as stored in `engine_runs.knob`. The sim has
//! its own `Knob` for playing; this is the database's view of the same thing, and
//! a test in aimadapt-engine (which has both crates) checks they agree.

/// The knob of a stored rating.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Knob {
    /// Seconds a target lives: shorter is harder.
    Lifetime,
    /// Degrees per second a target moves across the view: faster is harder.
    TargetSpeed,
    /// Seconds per kill allowed: shorter is harder.
    KillPace,
    /// Meters of target radius: smaller is harder.
    Radius,
}

impl Knob {
    pub const ALL: [Knob; 4] = [Knob::Lifetime, Knob::TargetSpeed, Knob::KillPace, Knob::Radius];

    /// The knob stored as `id`. Anything else is a size, as it always was here.
    pub fn from_id(id: &str) -> Knob {
        match id {
            "lifetime" => Knob::Lifetime,
            "target_speed" => Knob::TargetSpeed,
            "kill_pace" => Knob::KillPace,
            _ => Knob::Radius,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Knob::Lifetime => "lifetime",
            Knob::TargetSpeed => "target_speed",
            Knob::KillPace => "kill_pace",
            Knob::Radius => "radius",
        }
    }

    /// Whether a bigger value is harder.
    pub fn higher_is_harder(self) -> bool {
        self == Knob::TargetSpeed
    }

    /// Log-scale difficulty of a value: higher is harder.
    pub fn h(self, v: f64) -> f64 {
        let l = v.max(1e-4).ln();
        if self.higher_is_harder() { l } else { -l }
    }

    /// The value at log-scale difficulty `h`.
    pub fn value(self, h: f64) -> f64 {
        if self.higher_is_harder() { h.exp() } else { (-h).exp() }
    }

    /// Whether `a` is a better (harder) rating than `b`.
    pub fn better(self, a: f64, b: f64) -> bool {
        if self.higher_is_harder() { a > b } else { a < b }
    }

    /// A value with its unit.
    pub fn format(self, v: f64) -> String {
        match self {
            Knob::Lifetime | Knob::KillPace => format!("{v:.2} s"),
            Knob::TargetSpeed => format!("{v:.1} °/s"),
            Knob::Radius => format!("{v:.3} m"),
        }
    }

    /// What the rating measures, in words ("85% tracking speed").
    pub fn word(self) -> &'static str {
        match self {
            Knob::Lifetime => "speed",
            Knob::TargetSpeed => "tracking speed",
            Knob::KillPace => "kill pace",
            Knob::Radius => "size",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip_and_unknown_ids_are_sizes() {
        for k in Knob::ALL {
            assert_eq!(Knob::from_id(k.id()), k);
        }
        assert_eq!(Knob::from_id("anything else"), Knob::Radius);
    }

    #[test]
    fn harder_is_higher_on_the_log_scale() {
        assert!(Knob::TargetSpeed.h(60.0) > Knob::TargetSpeed.h(30.0));
        assert!(Knob::Lifetime.h(0.5) > Knob::Lifetime.h(1.0));
        assert!((Knob::Radius.value(Knob::Radius.h(0.3)) - 0.3).abs() < 1e-12);
        assert!(Knob::KillPace.better(0.6, 0.8) && Knob::TargetSpeed.better(40.0, 30.0));
    }
}
