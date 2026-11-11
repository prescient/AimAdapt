//! The parts of the results screen that are worked out, not drawn: where the chart's
//! dots go, its labels, the key hints, and the line that tells the app what to open.

use aimadapt_core::results::CHART_RUNS;

/// Width and height of the chart's plot, in pixels.
pub const PLOT_W: f32 = 300.0;
pub const PLOT_H: f32 = 64.0;
/// Width of an earlier run's dot and of this run's, in pixels.
const DOT: f32 = 8.0;
const THIS_DOT: f32 = 12.0;

const OPEN_PREFIX: &str = "aimadapt-open: ";

/// What the app should open when the trainer closes, asked for with a key on the
/// results screen. The trainer prints it on stderr as one line and the app reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Open {
    /// The replay of the run with this id.
    Replay(i64),
    /// The page of the scenario with this name.
    Scenario(String),
}

impl Open {
    /// The line the trainer prints: `aimadapt-open: replay 123`.
    pub fn line(&self) -> String {
        match self {
            Open::Replay(id) => format!("{OPEN_PREFIX}replay {id}"),
            Open::Scenario(name) => format!("{OPEN_PREFIX}scenario {name}"),
        }
    }

    /// The reverse of [`Open::line`]. `None` for any other line.
    pub fn parse(line: &str) -> Option<Open> {
        let rest = line.trim().strip_prefix(OPEN_PREFIX)?;
        if let Some(id) = rest.strip_prefix("replay ") {
            return id.trim().parse().ok().map(Open::Replay);
        }
        let name = rest.strip_prefix("scenario ")?.trim();
        (!name.is_empty()).then(|| Open::Scenario(name.to_string()))
    }
}

/// One dot of the chart. `x` and `y` run from 0 to 1, with `y` at 1 for the best value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dot {
    pub x: f32,
    pub y: f32,
    /// The run just played.
    pub this: bool,
}

/// One dot per value, in order. The newest is at the right edge and a short history
/// leaves the left empty. Up is always better.
pub fn dots(values: &[f64], higher_is_better: bool) -> Vec<Dot> {
    let n = values.len();
    let lo = values.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    values
        .iter()
        .enumerate()
        .map(|(i, &v)| {
            let y = if hi - lo < 1e-12 {
                0.5
            } else if higher_is_better {
                (v - lo) / (hi - lo)
            } else {
                (hi - v) / (hi - lo)
            };
            let x = 1.0 - (n - 1 - i) as f32 / (CHART_RUNS - 1) as f32;
            Dot { x: x.max(0.0), y: y as f32, this: i + 1 == n }
        })
        .collect()
}

/// Where a dot sits in the plot: left, top and size in pixels. A dot's centre stays
/// half its size inside the edges.
pub fn dot_box(dot: &Dot) -> (f32, f32, f32) {
    let size = if dot.this { THIS_DOT } else { DOT };
    let x = THIS_DOT / 2.0 + dot.x * (PLOT_W - THIS_DOT);
    let y = THIS_DOT / 2.0 + (1.0 - dot.y) * (PLOT_H - THIS_DOT);
    (x - size / 2.0, y - size / 2.0, size)
}

/// The text for the top and the bottom of the chart, the best value first. The
/// second is empty when both read the same.
pub fn axis_labels(values: &[f64], higher_is_better: bool, fmt: impl Fn(f64) -> String) -> (String, String) {
    let lo = values.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let (best, worst) = if higher_is_better { (hi, lo) } else { (lo, hi) };
    let (top, bottom) = (fmt(best), fmt(worst));
    if top == bottom { (top, String::new()) } else { (top, bottom) }
}

/// The line under the chart.
pub fn caption(points: usize, adaptive: bool) -> String {
    let what = if adaptive { "ratings" } else { "scores" };
    format!("Your last {points} {what}, this run marked. Up is better.")
}

/// Makes text safe for the game font, which has only plain ASCII.
pub fn game_text(s: &str) -> String {
    let s = s.replace(" \u{b0}", " deg").replace('\u{b0}', " deg");
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\u{2013}' | '\u{2014}' | '\u{2212}' => out.push('-'),
            '\u{d7}' => out.push('x'),
            '\u{2026}' => out.push_str("..."),
            '\u{b1}' => out.push_str("+/-"),
            '\u{2018}' | '\u{2019}' => out.push('\''),
            '\u{201c}' | '\u{201d}' => out.push('"'),
            c if c == '\n' || (' '..='~').contains(&c) => out.push(c),
            _ => out.push('?'),
        }
    }
    out
}

/// What the key hints depend on.
#[derive(Debug, Clone, Copy)]
pub struct Keys {
    /// Runs in this trainer session (1 for a single scenario).
    pub steps: usize,
    /// This is the last run of the session.
    pub last: bool,
    /// The app started the trainer and is waiting, so it can be sent to a page.
    pub handoff: bool,
    /// This run has a replay to watch.
    pub replay: bool,
}

/// The key hints under the results: one line, and a second one for the keys that
/// open a page in the app.
pub fn keys_text(k: Keys) -> String {
    let more = k.steps > 1 && !k.last;
    let mut text = if k.steps <= 1 {
        "Space: play again    R: restart    Esc: quit"
    } else if k.last {
        "Space or Esc: finish    R: play it again"
    } else {
        "Space: continue    R: play it again    Esc: end workout"
    }
    .to_string();
    if k.handoff {
        text.push('\n');
        text.push_str(if k.replay { "V: watch the replay    I: details in the app" } else { "I: details in the app" });
        if more {
            text.push_str(if k.replay { "    (both end the workout)" } else { "    (ends the workout)" });
        }
    }
    text
}

/// How a finished run of a workout went, for the workout's summary.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// An adaptive run: the rating after it and, unless it was a calibration run, the
    /// rating before it, both as text.
    Rated { after: String, before: Option<String> },
    /// A standard run's score.
    Scored(f64),
}

/// One finished run of a workout.
#[derive(Debug, Clone, PartialEq)]
pub struct StepDone {
    pub name: String,
    /// The run in one line, shown when its scenario was played once.
    pub line: String,
    pub outcome: Outcome,
}

/// Lines in the workout summary. Scenarios past that are counted, not listed.
pub const SUMMARY_LINES: usize = 12;

/// The workout summary: one line for each scenario, in the order they were first
/// played. A scenario played once keeps its run's line. One played several times
/// gets its run count with the rating it ended on, or its best score. A line for
/// every run of a long workout would push the key hints off the screen.
pub fn workout_summary(done: &[StepDone]) -> Vec<String> {
    let rated = |d: &StepDone| matches!(d.outcome, Outcome::Rated { .. });
    let mut groups: Vec<Vec<&StepDone>> = Vec::new();
    for d in done {
        match groups.iter_mut().find(|g| g[0].name == d.name && rated(g[0]) == rated(d)) {
            Some(group) => group.push(d),
            None => groups.push(vec![d]),
        }
    }
    let mut lines: Vec<String> = groups.iter().map(|g| scenario_line(g)).collect();
    if lines.len() > SUMMARY_LINES {
        let more = lines.len() - (SUMMARY_LINES - 1);
        lines.truncate(SUMMARY_LINES - 1);
        lines.push(format!("and {more} more scenarios"));
    }
    lines
}

/// The summary line for one scenario's runs, which are never empty.
fn scenario_line(runs: &[&StepDone]) -> String {
    let (first, last) = (runs[0], runs[runs.len() - 1]);
    if runs.len() == 1 {
        return format!("{}: {}", first.name, first.line);
    }
    let what = match &last.outcome {
        Outcome::Rated { after, .. } => match &first.outcome {
            Outcome::Rated { before: Some(before), .. } => format!("rating {after} (was {before})"),
            _ => format!("rating {after}"),
        },
        Outcome::Scored(_) => {
            let scores = runs.iter().filter_map(|d| match d.outcome {
                Outcome::Scored(score) => Some(score),
                Outcome::Rated { .. } => None,
            });
            format!("best score {:.0}", scores.fold(f64::NEG_INFINITY, f64::max))
        }
    };
    format!("{}: {} runs, {what}", first.name, runs.len())
}

/// Height of one line of the results text, in pixels.
const LINE_H: f32 = 26.0;
/// Height of what surrounds the text: the panel's padding, the gap above the key
/// hints and a margin to the window's edges.
const AROUND_H: f32 = 100.0;
/// Height the chart takes, and the note at its longest.
const CHART_H: f32 = 110.0;
const NOTE_H: f32 = 130.0;

/// Whether the chart and the note fit on the results screen: `(chart, note)`.
/// `text_lines` counts the lines of the results and of the key hints, and
/// `window_height` is the window's height in the same pixels the text is laid out in.
/// The chart goes in first. Whatever does not fit is left out, so the key hints
/// always stay on the screen.
pub fn room(window_height: f32, text_lines: usize, has_chart: bool, has_note: bool) -> (bool, bool) {
    let mut used = AROUND_H + text_lines as f32 * LINE_H;
    let chart = has_chart && used + CHART_H <= window_height;
    if chart {
        used += CHART_H;
    }
    let note = has_note && used + NOTE_H <= window_height;
    (chart, note)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rated_run(name: &str, after: &str, before: Option<&str>) -> StepDone {
        StepDone {
            name: name.into(),
            line: format!("At your level: rating {after}"),
            outcome: Outcome::Rated { after: after.into(), before: before.map(String::from) },
        }
    }

    fn scored_run(name: &str, score: f64) -> StepDone {
        StepDone { name: name.into(), line: format!("Score {score:.0}, accuracy 80%"), outcome: Outcome::Scored(score) }
    }

    /// The last screen of a Core Week day listed all 23 runs, one to a line, which ran
    /// off the top and the bottom of the window and took the key hints with it.
    #[test]
    fn a_workout_of_many_runs_is_summed_up_by_scenario() {
        let mut done = vec![scored_run("Static Grid", 640.0), scored_run("Static Grid", 760.0)];
        for (name, runs) in [("Timed Flicks", 5), ("Timed Grid", 5), ("Timed Wide Flicks", 4), ("Shrink Reflex", 4)] {
            done.push(rated_run(name, "0.54 s", None));
            for n in 1..runs {
                done.push(rated_run(name, &format!("0.5{n} s"), Some(&format!("0.6{n} s"))));
            }
        }
        for n in 0..3 {
            done.push(rated_run("Strafe Track", &format!("13.{n} deg/s"), Some(&format!("14.{n} deg/s"))));
        }
        assert_eq!(done.len(), 23);
        assert_eq!(
            workout_summary(&done),
            [
                "Static Grid: 2 runs, best score 760",
                "Timed Flicks: 5 runs, rating 0.54 s",
                "Timed Grid: 5 runs, rating 0.54 s",
                "Timed Wide Flicks: 4 runs, rating 0.53 s",
                "Shrink Reflex: 4 runs, rating 0.53 s",
                "Strafe Track: 3 runs, rating 13.2 deg/s (was 14.0 deg/s)",
            ]
        );
    }

    #[test]
    fn a_scenario_played_once_keeps_its_runs_line() {
        let done = [scored_run("Static Grid", 900.0), rated_run("Timed Flicks", "0.50 s", Some("0.52 s"))];
        assert_eq!(
            workout_summary(&done),
            ["Static Grid: Score 900, accuracy 80%", "Timed Flicks: At your level: rating 0.50 s"]
        );
        assert!(workout_summary(&[]).is_empty());
    }

    #[test]
    fn standard_and_adaptive_runs_of_one_scenario_get_a_line_each() {
        let done = [
            scored_run("Static Grid", 900.0),
            rated_run("Static Grid", "0.50 s", Some("0.52 s")),
            scored_run("Static Grid", 950.0),
            rated_run("Static Grid", "0.48 s", Some("0.50 s")),
        ];
        assert_eq!(
            workout_summary(&done),
            ["Static Grid: 2 runs, best score 950", "Static Grid: 2 runs, rating 0.48 s (was 0.52 s)"]
        );
    }

    #[test]
    fn a_summary_of_many_scenarios_counts_the_rest() {
        let done: Vec<StepDone> = (0..20).map(|n| scored_run(&format!("Scenario {n}"), 100.0)).collect();
        let lines = workout_summary(&done);
        assert_eq!(lines.len(), SUMMARY_LINES);
        assert_eq!(lines[SUMMARY_LINES - 2], "Scenario 10: Score 100, accuracy 80%");
        assert_eq!(lines[SUMMARY_LINES - 1], "and 9 more scenarios");
        let just_fits: Vec<StepDone> = (0..SUMMARY_LINES).map(|n| scored_run(&format!("Scenario {n}"), 1.0)).collect();
        assert_eq!(workout_summary(&just_fits).len(), SUMMARY_LINES);
        assert!(!workout_summary(&just_fits).last().unwrap().starts_with("and "));
    }

    /// With the chart and the note added, the results could be taller than the window,
    /// and what overflows is cut off at both ends: the scenario's name and the keys.
    #[test]
    fn the_chart_and_the_note_give_way_when_the_window_is_short() {
        // One run: nine lines of results and two of keys.
        assert_eq!(room(900.0, 11, true, true), (true, true));
        assert_eq!(room(720.0, 11, true, true), (true, true));
        // The longest workout summary: the results, "Workout complete", twelve lines, a gap and the keys.
        let longest = 9 + 1 + SUMMARY_LINES + 1 + 2;
        assert_eq!(room(900.0, longest, true, true), (true, false));
        assert_eq!(room(720.0, longest, true, true), (false, false));
        // A Core Week day: six scenarios.
        assert_eq!(room(900.0, 19, true, true), (true, true));
        assert_eq!(room(720.0, 19, true, true), (true, false));
        // Nothing to show is never shown, and the note does not wait for a chart.
        assert_eq!(room(900.0, 11, false, false), (false, false));
        assert_eq!(room(900.0, 11, false, true), (false, true));
        assert_eq!(room(400.0, 11, true, true), (false, false));
    }

    #[test]
    fn the_open_line_reads_back() {
        for open in [Open::Replay(123), Open::Scenario("Timed Flicks".into()), Open::Scenario("A: b  c".into())] {
            assert_eq!(Open::parse(&open.line()), Some(open));
        }
        assert_eq!(Open::Replay(123).line(), "aimadapt-open: replay 123");
        assert_eq!(Open::Scenario("Timed Flicks".into()).line(), "aimadapt-open: scenario Timed Flicks");
        assert_eq!(Open::parse("  aimadapt-open: replay 7\r\n"), Some(Open::Replay(7)));
        for bad in [
            "",
            "replay 5",
            "aimadapt: replay 5",
            "aimadapt-open: replay",
            "aimadapt-open: replay x",
            "aimadapt-open: replay 1.5",
            "aimadapt-open: scenario ",
            "aimadapt-open: scenario",
            "aimadapt-open: page 3",
        ] {
            assert_eq!(Open::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn the_newest_run_is_at_the_right_and_the_best_at_the_top() {
        let d = dots(&[1.5, 1.2, 1.4], false);
        assert_eq!(d.len(), 3);
        assert!(d[2].this && !d[0].this && !d[1].this);
        assert_eq!(d[2].x, 1.0);
        assert_eq!(d[1].y, 1.0, "the smallest lifetime is the best");
        assert_eq!(d[0].y, 0.0);
        assert!((d[0].x - (1.0 - 2.0 / 9.0)).abs() < 1e-6);
    }

    #[test]
    fn a_full_chart_spans_the_width() {
        let values: Vec<f64> = (0..CHART_RUNS).map(|i| i as f64).collect();
        let d = dots(&values, true);
        assert_eq!(d[0].x, 0.0);
        assert_eq!(d[CHART_RUNS - 1].x, 1.0);
    }

    #[test]
    fn equal_values_sit_in_the_middle() {
        assert!(dots(&[2.0, 2.0, 2.0], true).iter().all(|d| d.y == 0.5));
    }

    #[test]
    fn scores_and_speeds_put_the_highest_on_top() {
        let d = dots(&[10.0, 30.0, 20.0], true);
        assert_eq!((d[0].y, d[1].y, d[2].y), (0.0, 1.0, 0.5));
    }

    #[test]
    fn this_runs_dot_at_the_top_right_stays_inside_the_plot() {
        let (left, top, size) = dot_box(&Dot { x: 1.0, y: 1.0, this: true });
        assert!(left >= 0.0 && top >= 0.0 && left + size <= PLOT_W && top + size <= PLOT_H);
        let (left, top, size) = dot_box(&Dot { x: 0.0, y: 0.0, this: false });
        assert!(left >= 0.0 && top >= 0.0 && left + size <= PLOT_W && top + size <= PLOT_H);
    }

    #[test]
    fn the_labels_give_the_best_value_first_and_one_label_when_equal() {
        let f = |v: f64| format!("{v:.1}");
        assert_eq!(axis_labels(&[1.5, 1.2, 1.4], false, f), ("1.2".into(), "1.5".into()));
        assert_eq!(axis_labels(&[10.0, 30.0], true, f), ("30.0".into(), "10.0".into()));
        assert_eq!(axis_labels(&[2.0, 2.0], true, f), ("2.0".into(), String::new()));
        assert_eq!(axis_labels(&[2.01, 2.02], true, f), ("2.0".into(), String::new()), "same text, one label");
    }

    #[test]
    fn the_caption_counts_the_runs_and_names_what_they_are() {
        assert_eq!(caption(8, true), "Your last 8 ratings, this run marked. Up is better.");
        assert_eq!(caption(3, false), "Your last 3 scores, this run marked. Up is better.");
    }

    /// The game font has only ASCII, and notes are written with degree signs and dashes.
    #[test]
    fn game_text_is_always_ascii() {
        assert_eq!(game_text("sits 0.40\u{b0} behind"), "sits 0.40 deg behind");
        assert_eq!(game_text("12.0 \u{b0}/s"), "12.0 deg/s");
        let all = game_text(
            "a\u{2013}b\u{2014}c\u{2212}d \u{d7} e\u{2026} \u{b1}f \u{2018}g\u{2019} \u{201c}h\u{201d} \u{e9}\nz",
        );
        assert_eq!(all, "a-b-c-d x e... +/-f 'g' \"h\" ?\nz");
        assert!(all.is_ascii());
    }

    #[test]
    fn the_keys_offer_the_replay_and_the_app_only_when_the_app_is_waiting() {
        let k = |steps, last, handoff, replay| keys_text(Keys { steps, last, handoff, replay });
        assert_eq!(k(1, true, false, true), "Space: play again    R: restart    Esc: quit");
        assert_eq!(
            k(1, true, true, true),
            "Space: play again    R: restart    Esc: quit\nV: watch the replay    I: details in the app"
        );
        assert_eq!(k(1, true, true, false), "Space: play again    R: restart    Esc: quit\nI: details in the app");
        assert_eq!(k(5, true, false, true), "Space or Esc: finish    R: play it again");
        assert_eq!(
            k(5, true, true, true),
            "Space or Esc: finish    R: play it again\nV: watch the replay    I: details in the app"
        );
        assert_eq!(k(5, false, false, true), "Space: continue    R: play it again    Esc: end workout");
        assert_eq!(
            k(5, false, true, true),
            "Space: continue    R: play it again    Esc: end workout\nV: watch the replay    I: details in the app    (both end the workout)"
        );
        assert_eq!(
            k(5, false, true, false),
            "Space: continue    R: play it again    Esc: end workout\nI: details in the app    (ends the workout)"
        );
    }
}
