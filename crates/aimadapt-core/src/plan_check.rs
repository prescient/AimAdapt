//! Planned against actual: for each kind of adaptive run (at your level, stretch, easy),
//! how often the player succeeded beside the chance the trainer planned. A kind whose
//! pooled gap is bigger than luck explains is marked `above` or `below`, so a plan that
//! keeps missing shows up. Calibration and standard runs have no plan and are left out.
//!
//! The tolerance copies `aimadapt_sim::adaptive::plan_tolerance`; a test in
//! aimadapt-engine, which has both crates, checks they agree.

use crate::db::{Db, DbError};

/// The kinds that carry a plan, in the order they are shown.
pub const KINDS: [&str; 3] = ["near", "stretch", "easy"];

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct KindStat {
    /// `near`, `stretch` or `easy`.
    pub kind: String,
    pub runs: usize,
    /// The runs' lengths added up, in target-equivalents.
    pub evidence: f64,
    /// Evidence-weighted mean of the planned chance. `None` with no runs.
    pub planned: Option<f64>,
    /// Evidence-weighted mean of the success rate.
    pub actual: Option<f64>,
    /// `actual - planned`.
    pub gap: Option<f64>,
    /// `as_planned`, `above` or `below`. `None` with no runs.
    pub verdict: Option<String>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct PlanCheck {
    /// Always three, in the order near, stretch, easy.
    pub kinds: Vec<KindStat>,
    /// Planned runs saved before their length was recorded.
    pub uncounted: usize,
}

/// One stored run, as far as the check needs it.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanRow {
    pub base_scenario: String,
    pub adaptive: bool,
    pub pick: Option<String>,
    pub planned: Option<f64>,
    pub actual: Option<f64>,
    pub evidence: Option<f64>,
}

/// The gap from plan that luck alone explains in a run of this length (in
/// target-equivalents): 1.5 binomial standard deviations, never under five points.
/// Must match `aimadapt_sim::adaptive::plan_tolerance`.
pub fn tolerance(planned: f64, evidence: f64) -> f64 {
    if evidence <= 0.0 {
        return 1.0;
    }
    (1.5 * (planned * (1.0 - planned) / evidence).sqrt()).max(0.05)
}

/// Planned against actual for the runs of one scenario, or of all of them with `None`.
pub fn by_kind(db: &Db, base_scenario: Option<&str>) -> Result<PlanCheck, DbError> {
    let mut stmt = db.conn().prepare(
        "SELECT base_scenario, adaptive, run_pick, planned_chance, success_rate, evidence
         FROM engine_runs WHERE adaptive = 1 AND run_pick IN ('near', 'stretch', 'easy')",
    )?;
    let rows: Vec<PlanRow> = stmt
        .query_map([], |r| {
            Ok(PlanRow {
                base_scenario: r.get(0)?,
                adaptive: r.get(1)?,
                pick: r.get(2)?,
                planned: r.get(3)?,
                actual: r.get(4)?,
                evidence: r.get(5)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(summarize(&rows, base_scenario))
}

/// The pure part of [`by_kind`].
pub fn summarize(rows: &[PlanRow], base_scenario: Option<&str>) -> PlanCheck {
    let mut uncounted = 0;
    let mut kinds: Vec<KindStat> = KINDS.iter().map(|k| empty(k)).collect();
    let mut sums = [(0.0, 0.0); 3];
    for row in rows.iter().filter(|r| base_scenario.is_none_or(|s| s == r.base_scenario)) {
        let Some(slot) = row.pick.as_deref().and_then(|p| KINDS.iter().position(|k| *k == p)) else {
            continue;
        };
        let (Some(planned), Some(actual)) = (row.planned, row.actual) else { continue };
        if !row.adaptive {
            continue;
        }
        match row.evidence {
            Some(w) if w > 0.0 => {
                let k = &mut kinds[slot];
                k.runs += 1;
                k.evidence += w;
                sums[slot].0 += w * planned;
                sums[slot].1 += w * actual;
            }
            _ => uncounted += 1,
        }
    }
    for (k, (planned, actual)) in kinds.iter_mut().zip(sums) {
        if k.runs == 0 {
            continue;
        }
        let (planned, actual) = (planned / k.evidence, actual / k.evidence);
        let gap = actual - planned;
        let verdict = if gap.abs() <= tolerance(planned, k.evidence) + 1e-6 {
            "as_planned"
        } else if gap > 0.0 {
            "above"
        } else {
            "below"
        };
        k.planned = Some(planned);
        k.actual = Some(actual);
        k.gap = Some(gap);
        k.verdict = Some(verdict.to_string());
    }
    PlanCheck { kinds, uncounted }
}

fn empty(kind: &str) -> KindStat {
    KindStat { kind: kind.into(), runs: 0, evidence: 0.0, planned: None, actual: None, gap: None, verdict: None }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::RunFile;
    use crate::telemetry::EngineRun;
    use chrono::NaiveDate;
    use std::collections::BTreeMap;

    fn row(pick: &str, planned: f64, actual: f64, evidence: Option<f64>) -> PlanRow {
        PlanRow {
            base_scenario: "Timed Flicks".into(),
            adaptive: true,
            pick: Some(pick.into()),
            planned: Some(planned),
            actual: Some(actual),
            evidence,
        }
    }

    fn kind<'a>(c: &'a PlanCheck, k: &str) -> &'a KindStat {
        c.kinds.iter().find(|s| s.kind == k).unwrap()
    }

    #[test]
    fn a_kind_with_no_runs_has_a_row_and_no_figures() {
        let c = summarize(&[], None);
        let names: Vec<&str> = c.kinds.iter().map(|k| k.kind.as_str()).collect();
        assert_eq!(names, ["near", "stretch", "easy"]);
        assert!(c.kinds.iter().all(|k| k.runs == 0 && k.planned.is_none() && k.verdict.is_none()));
        assert_eq!(c.uncounted, 0);
    }

    #[test]
    fn a_long_run_counts_for_more_than_a_short_one() {
        let rows = [row("near", 0.85, 1.0, Some(30.0)), row("near", 0.85, 0.5, Some(10.0))];
        let c = summarize(&rows, None);
        let near = kind(&c, "near");
        assert_eq!(near.runs, 2);
        assert_eq!(near.evidence, 40.0);
        assert!((near.actual.unwrap() - 0.875).abs() < 1e-9, "{:?}", near.actual);
        assert!((near.planned.unwrap() - 0.85).abs() < 1e-9);
    }

    #[test]
    fn a_gap_inside_the_noise_is_as_planned_and_one_outside_is_not() {
        // 100 targets at 0.55: the tolerance is about 0.075.
        let inside = summarize(&[row("stretch", 0.55, 0.60, Some(100.0))], None);
        assert_eq!(kind(&inside, "stretch").verdict.as_deref(), Some("as_planned"));
        let below = summarize(&[row("stretch", 0.55, 0.40, Some(100.0))], None);
        assert_eq!(kind(&below, "stretch").verdict.as_deref(), Some("below"));
        assert!((kind(&below, "stretch").gap.unwrap() + 0.15).abs() < 1e-9);
        let above = summarize(&[row("easy", 0.90, 1.0, Some(400.0))], None);
        assert_eq!(kind(&above, "easy").verdict.as_deref(), Some("above"));
        // A very long run still gets five points of slack.
        let slack = summarize(&[row("easy", 0.95, 0.91, Some(100_000.0))], None);
        assert_eq!(kind(&slack, "easy").verdict.as_deref(), Some("as_planned"));
    }

    #[test]
    fn a_run_without_its_length_is_counted_apart() {
        let rows =
            [row("near", 0.85, 0.8, None), row("near", 0.85, 0.8, Some(0.0)), row("near", 0.85, 0.8, Some(20.0))];
        let c = summarize(&rows, None);
        assert_eq!(c.uncounted, 2);
        assert_eq!(kind(&c, "near").runs, 1);
    }

    #[test]
    fn calibration_and_standard_runs_are_ignored() {
        let mut standard = row("near", 0.85, 0.2, Some(50.0));
        standard.adaptive = false;
        let rows = [row("calibrate", 0.5, 0.1, Some(50.0)), standard, row("near", 0.85, 0.8, None)];
        let c = summarize(&rows, None);
        assert!(c.kinds.iter().all(|k| k.runs == 0), "{c:?}");
        assert_eq!(c.uncounted, 1, "only the planned adaptive run missing its length");
    }

    #[test]
    fn one_scenario_can_be_picked() {
        let mut other = row("near", 0.85, 0.5, Some(50.0));
        other.base_scenario = "Other".into();
        let rows = [row("near", 0.85, 0.85, Some(50.0)), other];
        assert_eq!(kind(&summarize(&rows, None), "near").runs, 2);
        let one = summarize(&rows, Some("Timed Flicks"));
        assert_eq!(kind(&one, "near").runs, 1);
        assert!((kind(&one, "near").actual.unwrap() - 0.85).abs() < 1e-9);
        assert_eq!(kind(&summarize(&rows, Some("Nothing")), "near").runs, 0);
    }

    fn save(db: &mut Db, n: u32, scenario: &str, e: EngineRun) {
        let at = NaiveDate::from_ymd_opt(2026, 11, 4).unwrap().and_hms_opt(8, n, 0).unwrap();
        let name = format!("rustyaim/test/{n}");
        let run = RunFile {
            file_name: name.clone(),
            scenario: scenario.into(),
            mode: "RustyAim".into(),
            finished_at: at,
            fields: BTreeMap::from([("Source".to_string(), "rustyaim".to_string())]),
            kills: vec![],
            weapons: vec![],
        };
        db.insert_runs(&[run], at).unwrap();
        assert!(db.save_engine_run(&name, &EngineRun { base_scenario: scenario.into(), ..e }).unwrap());
    }

    fn planned(pick: &str, chance: f64, rate: f64, evidence: Option<f64>) -> EngineRun {
        EngineRun {
            category: "clicking".into(),
            adaptive: true,
            knob: Some("lifetime".into()),
            run_pick: Some(pick.into()),
            planned_chance: Some(chance),
            success_rate: Some(rate),
            evidence,
            ..Default::default()
        }
    }

    #[test]
    fn the_database_is_read_by_kind_and_scenario() {
        let mut db = Db::open_in_memory().unwrap();
        save(&mut db, 1, "A", planned("calibrate", 0.5, 0.2, Some(40.0)));
        save(&mut db, 2, "A", planned("near", 0.85, 0.80, Some(40.0)));
        save(&mut db, 3, "A", planned("stretch", 0.55, 0.30, Some(60.0)));
        save(&mut db, 4, "B", planned("near", 0.85, 0.90, Some(60.0)));
        save(&mut db, 5, "B", planned("easy", 0.95, 0.95, None));
        save(&mut db, 6, "B", EngineRun { adaptive: false, ..planned("near", 0.85, 0.1, Some(9.0)) });

        let all = by_kind(&db, None).unwrap();
        assert_eq!(kind(&all, "near").runs, 2);
        assert!((kind(&all, "near").actual.unwrap() - 0.86).abs() < 1e-9, "{:?}", kind(&all, "near"));
        assert_eq!(kind(&all, "stretch").verdict.as_deref(), Some("below"));
        assert_eq!(kind(&all, "easy").runs, 0);
        assert_eq!(all.uncounted, 1);

        let a = by_kind(&db, Some("A")).unwrap();
        assert_eq!(kind(&a, "near").runs, 1);
        assert_eq!(a.uncounted, 0);
    }
}
