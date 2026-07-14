//! solid_base / avg aggregation (WolfBench-style floor).

use super::types::{CompareItem, CompareSummary, HarnessSummary};
use std::collections::BTreeMap;

/// Min score across runs (0 if empty).
pub fn solid_base(scores: &[f64]) -> f64 {
    if scores.is_empty() {
        return 0.0;
    }
    scores.iter().copied().fold(f64::INFINITY, f64::min)
}

pub fn summarize_items(items: &[CompareItem]) -> CompareSummary {
    let mut by_harness: BTreeMap<String, Vec<&CompareItem>> = BTreeMap::new();
    for it in items {
        by_harness.entry(it.harness.clone()).or_default().push(it);
    }

    let mut harnesses = BTreeMap::new();
    let mut total_scored = 0usize;
    let mut total_correct = 0usize;

    for (h, rows) in by_harness {
        let scored: Vec<&&CompareItem> = rows
            .iter()
            .filter(|r| r.status == "pass" || r.status == "fail")
            .collect();
        let n_skip = rows.iter().filter(|r| r.status == "skip").count();
        let n_error = rows.iter().filter(|r| r.status == "error").count();

        let mut by_task: BTreeMap<String, Vec<f64>> = BTreeMap::new();
        for r in &scored {
            by_task
                .entry(r.task_id.clone())
                .or_default()
                .push(r.score);
        }
        let mut task_avgs = Vec::new();
        let mut task_floors = Vec::new();
        let mut per_task_avg = BTreeMap::new();
        for (tid, scores) in &by_task {
            let avg = scores.iter().sum::<f64>() / scores.len() as f64;
            let floor = scores.iter().copied().fold(f64::INFINITY, f64::min);
            task_avgs.push(avg);
            task_floors.push(if floor.is_finite() { floor } else { 0.0 });
            per_task_avg.insert(tid.clone(), avg);
        }
        let avg_score = if task_avgs.is_empty() {
            0.0
        } else {
            task_avgs.iter().sum::<f64>() / task_avgs.len() as f64
        };
        let solid = if task_floors.is_empty() {
            0.0
        } else {
            task_floors.iter().sum::<f64>() / task_floors.len() as f64
        };
        let pass_rate = if scored.is_empty() {
            0.0
        } else {
            scored.iter().filter(|r| r.correct).count() as f64 / scored.len() as f64
        };
        let mean_t_ms = if scored.is_empty() {
            0
        } else {
            (scored.iter().map(|r| r.t_ms).sum::<u64>()) / scored.len() as u64
        };

        total_scored += scored.len();
        total_correct += scored.iter().filter(|r| r.correct).count();

        harnesses.insert(
            h,
            HarnessSummary {
                n_items: rows.len(),
                n_scored: scored.len(),
                n_skip,
                n_error,
                avg_score,
                solid_base: solid,
                pass_rate,
                per_task_avg,
                mean_t_ms,
            },
        );
    }

    let accuracy = if total_scored == 0 {
        0.0
    } else {
        total_correct as f64 / total_scored as f64
    };

    CompareSummary {
        harnesses,
        total: total_scored,
        correct: total_correct,
        accuracy,
    }
}

/// task_id → harness → avg score among scored runs.
pub fn task_matrix(items: &[CompareItem]) -> BTreeMap<String, BTreeMap<String, f64>> {
    let mut acc: BTreeMap<String, BTreeMap<String, Vec<f64>>> = BTreeMap::new();
    for it in items {
        if it.status != "pass" && it.status != "fail" {
            continue;
        }
        acc.entry(it.task_id.clone())
            .or_default()
            .entry(it.harness.clone())
            .or_default()
            .push(it.score);
    }
    let mut out = BTreeMap::new();
    for (task, by_h) in acc {
        let mut row = BTreeMap::new();
        for (h, scores) in by_h {
            row.insert(h, scores.iter().sum::<f64>() / scores.len() as f64);
        }
        out.insert(task, row);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(h: &str, task: &str, status: &str, score: f64, correct: bool) -> CompareItem {
        CompareItem {
            harness: h.into(),
            task_id: task.into(),
            track: "coding".into(),
            run_index: 1,
            status: status.into(),
            correct,
            score,
            t_ms: 10,
            detail: String::new(),
            metrics: BTreeMap::new(),
            workspace: String::new(),
            capabilities_missing: vec![],
        }
    }

    #[test]
    fn solid_base_is_min_and_leq_avg() {
        let items = vec![
            item("hermes", "t1", "pass", 1.0, true),
            item("hermes", "t1", "fail", 0.0, false),
            item("hermes", "t2", "pass", 1.0, true),
            item("jewell", "t1", "skip", 0.0, false),
        ];
        // fix run_index for multi-run
        let mut items = items;
        items[1].run_index = 2;

        let s = summarize_items(&items);
        let h = s.harnesses.get("hermes").unwrap();
        assert!(h.solid_base <= h.avg_score + 1e-9);
        assert_eq!(h.n_skip, 0);
        assert_eq!(h.n_scored, 3);
        let j = s.harnesses.get("jewell").unwrap();
        assert_eq!(j.n_skip, 1);
        assert_eq!(j.n_scored, 0);
        assert_eq!(j.avg_score, 0.0);
    }

    #[test]
    fn skip_score_stays_zero_in_pass_rate_denom() {
        let items = vec![
            item("jewell", "coding", "skip", 0.0, false),
            item("jewell", "host", "pass", 1.0, true),
        ];
        let s = summarize_items(&items);
        let j = s.harnesses.get("jewell").unwrap();
        assert_eq!(j.pass_rate, 1.0);
        assert_eq!(j.n_skip, 1);
    }
}
