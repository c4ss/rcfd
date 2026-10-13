//! Run-time statistics of transient runs (F-116): time-weighted mean, root-mean-square,
//! standard deviation, minimum and maximum of selected cell fields.
//!
//! Definitions, for steps `n` ending at time `t_n` with width `w_n` (the part of the step after
//! `statistics.start_time`) and end-of-step values `x_n`, which is the rectangle rule consistent with
//! the implicit time stepping:
//!
//! * `mean = sum(w_n x_n) / sum(w_n)`
//! * `rms = sqrt(sum(w_n x_n^2) / sum(w_n))` (root mean square of the value)
//! * `std = sqrt(sum(w_n (x_n - mean)^2) / sum(w_n))` (root mean square of the fluctuation, computed
//!   with a weighted Welford update, so no cancellation)
//! * `min`, `max` over the end-of-step values
//!
//! A step contributes if it ends after the start time.

use crate::case::{Case, Diagnostic};

/// Fields that can be accumulated.
pub const FIELDS: [&str; 5] = ["ux", "uy", "uz", "speed", "p"];
/// Suffixes of the derived fields.
pub const SUFFIXES: [&str; 5] = ["mean", "rms", "std", "min", "max"];

/// Accumulated statistics of a set of cell fields.
#[derive(Debug, Clone)]
pub struct Statistics {
    names: Vec<String>,
    start: f64,
    prev_t: f64,
    total: f64,
    mean: Vec<Vec<f64>>,
    mean_sq: Vec<Vec<f64>>,
    m2: Vec<Vec<f64>>,
    min: Vec<Vec<f64>>,
    max: Vec<Vec<f64>>,
}

fn gs(c: &Case, p: &str) -> String {
    c.tree
        .get(p)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

/// Names selected by `statistics.fields` (default `ux, uy, uz, p`).
pub fn selected(case: &Case) -> Vec<String> {
    let v: Vec<String> = case
        .tree
        .get("statistics.fields")
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|x| x.as_str().map(String::from))
        .collect();
    if v.is_empty() {
        ["ux", "uy", "uz", "p"].map(String::from).to_vec()
    } else {
        v
    }
}

/// Is statistics collection requested?
pub fn enabled(case: &Case) -> bool {
    case.tree
        .get("statistics.enabled")
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// Settings checks (names, algorithm).
pub fn check(case: &Case) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if !enabled(case) {
        return out;
    }
    for n in selected(case) {
        if !FIELDS.contains(&n.as_str()) {
            out.push(Diagnostic {
                path: "statistics.fields".into(),
                line: None,
                message: format!("cannot collect statistics of '{n}' (available: {})", FIELDS.join(", ")),
            });
        }
    }
    if matches!(
        gs(case, "solver.algorithm").as_str(),
        "simple" | "simplec" | "compressible"
    ) {
        out.push(Diagnostic {
            path: "statistics.enabled".into(),
            line: None,
            message: "statistics are collected by the transient incompressible algorithms (piso, pimple)".into(),
        });
    }
    out
}

/// Names that `output.fields` may list besides the statistics fields.
pub const OUTPUT_FIELDS: [&str; 12] = [
    "U",
    "p",
    "speed",
    "T",
    "k",
    "epsilon",
    "omega",
    "nut",
    "wall_distance",
    "y_plus",
    "rho",
    "mach",
];

/// Variables selected by `output.fields` (empty = all).
pub fn output_selection(case: &Case) -> Vec<String> {
    case.tree
        .get("output.fields")
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|x| x.as_str().map(String::from))
        .collect()
}

/// Does the selection keep the result variable `name`? (`statistics` selects all statistics fields.)
pub fn keeps(selection: &[String], name: &str, is_statistic: bool) -> bool {
    selection.is_empty()
        || selection.iter().any(|s| s == name)
        || (is_statistic && selection.iter().any(|s| s == "statistics"))
}

/// Check `output.fields`.
pub fn check_output(case: &Case) -> Vec<Diagnostic> {
    let stat_names: Vec<String> = selected(case)
        .iter()
        .flat_map(|n| SUFFIXES.iter().map(move |s| format!("{n}_{s}")))
        .collect();
    output_selection(case)
        .into_iter()
        .filter(|n| {
            !(OUTPUT_FIELDS.contains(&n.as_str())
                || n == "statistics"
                || (enabled(case) && stat_names.contains(n)))
        })
        .map(|n| Diagnostic {
            path: "output.fields".into(),
            line: None,
            message: format!(
                "unknown result variable '{n}' (available: {}, statistics, and <field>_mean/rms/std/min/max with statistics enabled)",
                OUTPUT_FIELDS.join(", ")
            ),
        })
        .collect()
}

impl Statistics {
    /// Build from the `statistics` settings; `None` when disabled.
    pub fn from_case(case: &Case, n_cells: usize) -> Option<Statistics> {
        if !enabled(case) {
            return None;
        }
        let names = selected(case);
        let k = names.len();
        let zero = || vec![vec![0.0; n_cells]; k];
        Some(Statistics {
            start: case
                .tree
                .get("statistics.start_time")
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            prev_t: 0.0,
            total: 0.0,
            mean: zero(),
            mean_sq: zero(),
            m2: zero(),
            min: vec![vec![f64::INFINITY; n_cells]; k],
            max: vec![vec![f64::NEG_INFINITY; n_cells]; k],
            names,
        })
    }

    /// Accumulated averaging time (s).
    pub fn time(&self) -> f64 {
        self.total
    }

    /// Names of the accumulated fields.
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// Add the step that ended at `t_end` with the end-of-step `values` (one vector per name).
    pub fn accumulate(&mut self, t_end: f64, values: &[Vec<f64>]) {
        let t0 = self.prev_t;
        self.prev_t = t_end;
        let w = t_end - t0.max(self.start);
        if w.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
            return;
        }
        let total = self.total + w;
        for (k, x) in values.iter().enumerate() {
            for (i, &v) in x.iter().enumerate() {
                let d = v - self.mean[k][i];
                self.mean[k][i] += d * w / total;
                self.m2[k][i] += w * d * (v - self.mean[k][i]);
                self.mean_sq[k][i] += (v * v - self.mean_sq[k][i]) * w / total;
                self.min[k][i] = self.min[k][i].min(v);
                self.max[k][i] = self.max[k][i].max(v);
            }
        }
        self.total = total;
    }

    /// One derived field, e.g. `ux_mean`; `None` for an unknown name or before any sample.
    pub fn get(&self, name: &str) -> Option<Vec<f64>> {
        if self.total <= 0.0 {
            return None;
        }
        let (base, suffix) = name.rsplit_once('_')?;
        let k = self.names.iter().position(|n| n == base)?;
        Some(match suffix {
            "mean" => self.mean[k].clone(),
            "rms" => self.mean_sq[k].iter().map(|v| v.max(0.0).sqrt()).collect(),
            "std" => self.m2[k].iter().map(|v| (v / self.total).max(0.0).sqrt()).collect(),
            "min" => self.min[k].clone(),
            "max" => self.max[k].clone(),
            _ => return None,
        })
    }

    /// All derived fields in a stable order (empty before the first sample).
    pub fn fields(&self) -> Vec<(String, Vec<f64>)> {
        let mut out = Vec::new();
        for n in &self.names {
            for s in SUFFIXES {
                let name = format!("{n}_{s}");
                if let Some(v) = self.get(&name) {
                    out.push((name, v));
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stats(start: f64) -> Statistics {
        Statistics {
            names: vec!["p".into()],
            start,
            prev_t: 0.0,
            total: 0.0,
            mean: vec![vec![0.0]],
            mean_sq: vec![vec![0.0]],
            m2: vec![vec![0.0]],
            min: vec![vec![f64::INFINITY]],
            max: vec![vec![f64::NEG_INFINITY]],
        }
    }

    #[test]
    fn weighted_moments_match_the_direct_sums() {
        let mut s = stats(0.0);
        let steps = [(0.1, 2.0), (0.3, -1.0), (0.35, 4.0), (1.0, 0.5)];
        for (t, v) in steps {
            s.accumulate(t, &[vec![v]]);
        }
        let ws = [0.1, 0.2, 0.05, 0.65];
        let xs = [2.0, -1.0, 4.0, 0.5];
        let tot: f64 = ws.iter().sum();
        let mean: f64 = ws.iter().zip(xs).map(|(w, x)| w * x).sum::<f64>() / tot;
        let msq: f64 = ws.iter().zip(xs).map(|(w, x)| w * x * x).sum::<f64>() / tot;
        let var: f64 = ws.iter().zip(xs).map(|(w, x)| w * (x - mean).powi(2)).sum::<f64>() / tot;
        assert!((s.get("p_mean").unwrap()[0] - mean).abs() < 1e-14);
        assert!((s.get("p_rms").unwrap()[0] - msq.sqrt()).abs() < 1e-14);
        assert!((s.get("p_std").unwrap()[0] - var.sqrt()).abs() < 1e-14);
        assert_eq!(s.get("p_min").unwrap()[0], -1.0);
        assert_eq!(s.get("p_max").unwrap()[0], 4.0);
        assert!((s.time() - 1.0).abs() < 1e-15);
    }

    #[test]
    fn start_time_cuts_the_first_contributing_step() {
        let mut s = stats(0.25);
        s.accumulate(0.2, &[vec![100.0]]); // ends before the start: ignored
        assert!(s.get("p_mean").is_none());
        s.accumulate(0.5, &[vec![2.0]]); // contributes the part after 0.25
        s.accumulate(1.0, &[vec![4.0]]);
        assert!((s.time() - 0.75).abs() < 1e-15);
        assert!((s.get("p_mean").unwrap()[0] - (0.25 * 2.0 + 0.5 * 4.0) / 0.75).abs() < 1e-14);
        assert_eq!(s.get("p_min").unwrap()[0], 2.0);
        assert!(s.get("p_nope").is_none() && s.get("q_mean").is_none());
    }
}
