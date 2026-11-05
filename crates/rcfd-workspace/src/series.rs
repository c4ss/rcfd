//! Residual and monitor series of a run (`runs/<id>/residuals.jsonl`), and their decimation for results under the size
//! budget.

use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Min/max decimation to at most `max_points`: first and last kept, the interior split into buckets that each keep
/// their minimum and maximum. Copied from `rcfd-gui/src/monitors.rs` (`decimate_minmax`, ADR 0025 M3; the frozen GUI
/// is not a dependency of this crate).
pub fn decimate_minmax(points: &[[f64; 2]], max_points: usize) -> Vec<[f64; 2]> {
    let n = points.len();
    if n <= max_points {
        return points.to_vec();
    }
    if max_points < 4 {
        return vec![points[0], points[n - 1]];
    }
    let nb = (max_points - 2) / 2;
    let inner = n - 2;
    let mut keep: Vec<usize> = Vec::with_capacity(2 * nb + 2);
    keep.push(0);
    for b in 0..nb {
        let (s, e) = (1 + b * inner / nb, 1 + (b + 1) * inner / nb);
        if s >= e {
            continue;
        }
        let (mut imin, mut imax) = (s, s);
        for i in s..e {
            if points[i][1] < points[imin][1] {
                imin = i;
            }
            if points[i][1] > points[imax][1] {
                imax = i;
            }
        }
        if imin == imax {
            keep.push(imin);
        } else {
            keep.push(imin.min(imax));
            keep.push(imin.max(imax));
        }
    }
    keep.push(n - 1);
    keep.into_iter().map(|i| points[i]).collect()
}

/// Series of `residuals.jsonl` bytes: `residuals.<eq>`, `monitors.<name>` and (adaptive correctors only) `correctors` and
/// `corrector_cap` against the step, decimated to
/// `max_points` each (`None`: no decimation). Returns (series, complete lines read, lines that did not parse).
pub fn residual_series(
    bytes: &[u8],
    since_step: u64,
    max_points: Option<usize>,
) -> (BTreeMap<String, Vec<[f64; 2]>>, usize, usize) {
    let committed = bytes.iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
    let mut series: BTreeMap<String, Vec<[f64; 2]>> = BTreeMap::new();
    let (mut lines, mut bad) = (0, 0);
    for line in bytes[..committed].split(|&b| b == b'\n') {
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let Ok(v) = serde_json::from_slice::<Value>(line) else {
            bad += 1;
            continue;
        };
        let Some(step) = v.get("step").and_then(Value::as_u64) else {
            bad += 1;
            continue;
        };
        lines += 1;
        if step < since_step {
            continue;
        }
        for group in ["residuals", "monitors"] {
            for (k, x) in v.get(group).and_then(Value::as_object).into_iter().flatten() {
                series
                    .entry(format!("{group}.{k}"))
                    .or_default()
                    .push([step as f64, x.as_f64().unwrap_or(f64::NAN)]);
            }
        }
        // ADR 0172 D5: the adaptive corrector count and the estimates that ended at the cap (ADR 0160 D4), only in the lines
        // of runs with solver.corrector_control = "adaptive"
        for k in ["correctors", "corrector_cap"] {
            if let Some(x) = v.get(k).and_then(Value::as_f64) {
                series.entry(k.to_string()).or_default().push([step as f64, x]);
            }
        }
    }
    if let Some(m) = max_points {
        for s in series.values_mut() {
            *s = decimate_minmax(s, m);
        }
    }
    (series, lines, bad)
}

pub(crate) fn series_json(series: &BTreeMap<String, Vec<[f64; 2]>>) -> Value {
    Value::Object(
        series
            .iter()
            .map(|(k, s)| {
                (
                    k.clone(),
                    json!({"x": s.iter().map(|p| p[0]).collect::<Vec<_>>(), "y": s.iter().map(|p| p[1]).collect::<Vec<_>>()}),
                )
            })
            .collect(),
    )
}
