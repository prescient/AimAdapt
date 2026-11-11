//! What the trainer's results screen shows beyond the run's own figures: a chart of
//! the scenario's recent ratings and the most useful note. Built on [`insights`].

use crate::db::{Db, DbError};
use crate::insights::{self, Note, NoteKind, RunPoint};
use crate::knob::Knob;

/// Runs on the results chart, counting the one just played.
pub const CHART_RUNS: usize = 10;

#[derive(Debug, Clone, PartialEq)]
pub struct ChartPoint {
    pub run_id: i64,
    pub value: f64,
}

/// The scenario's latest ratings (adaptive) or scores (standard), oldest first. The
/// last point is the run just played.
#[derive(Debug, Clone, PartialEq)]
pub struct RunChart {
    /// The knob the ratings are in, as stored. `None` for standard runs, whose
    /// values are scores.
    pub knob: Option<String>,
    pub points: Vec<ChartPoint>,
}

impl RunChart {
    /// Whether a bigger value is a better one. A score always is. A rating is when
    /// its knob gets harder as it rises.
    pub fn higher_is_better(&self) -> bool {
        self.knob.as_deref().is_none_or(|k| Knob::from_id(k).higher_is_harder())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RunExtras {
    pub run_id: i64,
    pub chart: Option<RunChart>,
    pub note: Option<Note>,
    /// The run recorded replay frames, so it can be watched again.
    pub replay: bool,
}

/// The chart for the run with `run_id`, from the scenario's runs (oldest first). `None`
/// when the run is not there, has no rating or score, or is the only point.
pub fn chart(runs: &[RunPoint], run_id: i64) -> Option<RunChart> {
    let at = runs.iter().position(|r| r.run_id == run_id)?;
    let this = &runs[at];
    let knob = if this.adaptive { this.knob.clone() } else { None };
    let value = |r: &RunPoint| -> Option<f64> {
        if this.adaptive {
            (r.adaptive && r.current && r.knob == this.knob).then_some(r.rating).flatten()
        } else {
            (!r.adaptive && r.current).then_some(r.score).flatten()
        }
    };
    let mut points: Vec<ChartPoint> =
        runs[..=at].iter().filter_map(|r| value(r).map(|value| ChartPoint { run_id: r.run_id, value })).collect();
    if points.len() > CHART_RUNS {
        points.drain(..points.len() - CHART_RUNS);
    }
    let last_is_this = points.last().is_some_and(|p| p.run_id == run_id);
    (last_is_this && points.len() >= 2).then_some(RunChart { knob, points })
}

/// The kinds of note in the order they are chosen from: what a player can change on
/// the next run comes ahead of what only describes them.
const NOTE_ORDER: [NoteKind; 9] = [
    NoteKind::Shots,
    NoteKind::Entries,
    NoteKind::Tracking,
    NoteKind::Direction,
    NoteKind::Segments,
    NoteKind::Curve,
    NoteKind::Switching,
    NoteKind::Reaction,
    NoteKind::Fitts,
];

/// The note most worth showing after a run. Never a rating note: the screen already
/// shows the rating.
pub fn top_note(notes: &[Note]) -> Option<&Note> {
    NOTE_ORDER.iter().find_map(|kind| notes.iter().find(|n| n.kind == *kind))
}

/// The chart, note and replay flag for the results screen of the run `run_id` of
/// `base_scenario`. `None` when the scenario has no insights or the run is not in them.
pub fn for_run(db: &Db, base_scenario: &str, run_id: i64) -> Result<Option<RunExtras>, DbError> {
    let Some(i) = insights::scenario(db, base_scenario)? else {
        return Ok(None);
    };
    let Some(this) = i.runs.iter().find(|r| r.run_id == run_id) else {
        return Ok(None);
    };
    Ok(Some(RunExtras {
        run_id,
        chart: chart(&i.runs, run_id),
        note: top_note(&i.notes).cloned(),
        replay: this.replay,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::insights::tests::{clicking_run, save};

    fn point(id: i64, adaptive: bool, knob: Option<&str>, value: Option<f64>, current: bool) -> RunPoint {
        RunPoint {
            run_id: id,
            finished_at: format!("2026-11-04 08:{id:02}:00"),
            adaptive,
            knob: knob.map(String::from),
            rating: if adaptive { value } else { None },
            rating_low: None,
            rating_high: None,
            fastest: None,
            success_rate: None,
            score: if adaptive { None } else { value },
            accuracy: None,
            throughput: None,
            reaction_ms: None,
            on_target: None,
            workout: None,
            pick: None,
            breakthrough: false,
            replay: true,
            current,
        }
    }

    fn rated(n: i64) -> Vec<RunPoint> {
        (1..=n).map(|i| point(i, true, Some("lifetime"), Some(1.0 + i as f64 / 10.0), true)).collect()
    }

    fn note(kind: NoteKind, title: &str) -> Note {
        Note { kind, title: title.into(), body: String::new() }
    }

    #[test]
    fn the_chart_is_the_last_ten_ratings_ending_with_this_run() {
        let c = chart(&rated(12), 12).unwrap();
        assert_eq!(c.points.len(), CHART_RUNS);
        assert_eq!(c.points.last().unwrap().run_id, 12);
        assert_eq!(c.points[0].run_id, 3);
        let ids: Vec<i64> = c.points.iter().map(|p| p.run_id).collect();
        assert!(ids.windows(2).all(|w| w[0] < w[1]), "oldest first: {ids:?}");
    }

    #[test]
    fn the_chart_stops_at_this_run() {
        let c = chart(&rated(8), 5).unwrap();
        assert_eq!(c.points.len(), 5);
        assert_eq!(c.points.last().unwrap().run_id, 5);
    }

    #[test]
    fn an_adaptive_run_charts_ratings_and_a_standard_run_scores() {
        let runs = vec![
            point(1, true, Some("lifetime"), Some(1.5), true),
            point(2, false, None, Some(900.0), true),
            point(3, true, Some("lifetime"), Some(1.4), true),
            point(4, false, None, Some(950.0), true),
        ];
        let a = chart(&runs, 3).unwrap();
        assert_eq!(a.knob.as_deref(), Some("lifetime"));
        assert_eq!(a.points.iter().map(|p| p.value).collect::<Vec<_>>(), [1.5, 1.4]);
        let s = chart(&runs, 4).unwrap();
        assert_eq!(s.knob, None);
        assert_eq!(s.points.iter().map(|p| p.value).collect::<Vec<_>>(), [900.0, 950.0]);
    }

    #[test]
    fn other_versions_and_other_knobs_stay_off_the_chart() {
        let runs = vec![
            point(1, true, Some("lifetime"), Some(1.5), false),
            point(2, true, Some("radius"), Some(2.0), true),
            point(3, true, Some("lifetime"), Some(1.4), true),
            point(4, true, Some("lifetime"), Some(1.3), true),
        ];
        let c = chart(&runs, 4).unwrap();
        assert_eq!(c.points.iter().map(|p| p.run_id).collect::<Vec<_>>(), [3, 4]);
    }

    #[test]
    fn one_run_is_not_a_chart() {
        assert_eq!(chart(&rated(1), 1), None);
        assert_eq!(chart(&rated(3), 99), None);
        let mut runs = rated(3);
        runs[2].rating = None;
        assert_eq!(chart(&runs, 3), None, "a run without a rating is not the last point");
    }

    #[test]
    fn a_lower_lifetime_is_better_and_a_higher_speed_or_score_is() {
        let c = |knob: Option<&str>| RunChart { knob: knob.map(String::from), points: vec![] };
        assert!(!c(Some("lifetime")).higher_is_better());
        assert!(c(Some("target_speed")).higher_is_better());
        assert!(c(None).higher_is_better());
    }

    #[test]
    fn the_top_note_is_never_the_rating() {
        assert_eq!(top_note(&[note(NoteKind::Rating, "a"), note(NoteKind::Rating, "b")]), None);
        assert_eq!(top_note(&[]), None);
    }

    #[test]
    fn the_top_note_puts_what_you_can_change_first() {
        let all = [
            note(NoteKind::Rating, "rating"),
            note(NoteKind::Curve, "curve"),
            note(NoteKind::Direction, "direction"),
            note(NoteKind::Shots, "shots"),
            note(NoteKind::Reaction, "reaction"),
        ];
        assert_eq!(top_note(&all).unwrap().title, "shots");
        let some = [note(NoteKind::Curve, "curve"), note(NoteKind::Reaction, "reaction")];
        assert_eq!(top_note(&some).unwrap().title, "curve");
        let two = [note(NoteKind::Shots, "first"), note(NoteKind::Shots, "second")];
        assert_eq!(top_note(&two).unwrap().title, "first");
    }

    #[test]
    fn extras_are_read_for_a_saved_run() {
        let mut db = Db::open_in_memory().unwrap();
        for (n, rating) in [(1, 1.2), (2, 1.1), (3, 1.0)] {
            save(&mut db, n, &clicking_run(rating));
        }
        let ids: Vec<i64> =
            insights::scenario(&db, "Timed Flicks").unwrap().unwrap().runs.iter().map(|r| r.run_id).collect();
        let last = *ids.last().unwrap();
        let extras = for_run(&db, "Timed Flicks", last).unwrap().unwrap();
        assert_eq!(extras.run_id, last);
        let chart = extras.chart.unwrap();
        assert_eq!(chart.points.len(), 3);
        assert_eq!(chart.points.last().unwrap().run_id, last);
        assert_eq!(for_run(&db, "Timed Flicks", last + 100).unwrap(), None);
        assert_eq!(for_run(&db, "No Such Scenario", last).unwrap(), None);
    }
}
