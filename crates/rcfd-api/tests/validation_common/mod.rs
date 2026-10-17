//! Shared helpers of the wave 30B validation tests (ADR 0060): reference-table reading and the
//! grid-convergence arithmetic (observed order and Roache GCI) applied to a functional of the solution.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// Directory of the small reference tables added in wave 30B.
pub fn reference_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../validation-data/reference")
}

/// Directory of the earlier in-repo cavity tables.
pub fn cavity_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../cases/cavity/reference")
}

/// Rows of a CSV file below its header, as raw string fields.
pub fn read_rows(path: &Path) -> Vec<Vec<String>> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    text.lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.split(',').map(|v| v.trim().to_string()).collect())
        .collect()
}

/// Numeric CSV (every field parses as f64).
pub fn read_numeric(path: &Path) -> Vec<Vec<f64>> {
    read_rows(path)
        .into_iter()
        .map(|r| r.iter().map(|v| v.parse().unwrap()).collect())
        .collect()
}

/// Observed order of convergence from three solutions of a functional on meshes with constant refinement
/// ratio `r` (`f1` coarse, `f3` fine): `p = ln|(f1 - f2)/(f2 - f3)| / ln r`.
pub fn observed_order(f1: f64, f2: f64, f3: f64, r: f64) -> f64 {
    ((f1 - f2) / (f2 - f3)).abs().ln() / r.ln()
}

/// Roache grid-convergence index of the fine solution (safety factor 1.25), as a fraction of `|f3|`, with the
/// order limited to [1, 2].
pub fn gci_fine(f2: f64, f3: f64, p_obs: f64, r: f64) -> f64 {
    let p = if p_obs.is_finite() { p_obs.clamp(1.0, 2.0) } else { 1.0 };
    1.25 * (f3 - f2).abs() / (f3.abs() * (r.powf(p) - 1.0))
}

/// Relative deviation in percent.
pub fn pct(x: f64, reference: f64) -> f64 {
    100.0 * (x / reference - 1.0)
}

/// Collects the verdict of every registered check so that all measured values are printed before the test fails.
#[derive(Default)]
pub struct Checks {
    failures: Vec<String>,
}

impl Checks {
    /// Record `value <= limit` (a deviation, always non-negative) under `name`.
    pub fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value <= limit;
        println!(
            "CHECK {name}: measured {value:.5} limit {limit:.5} -> {}",
            if ok { "pass" } else { "MISS" }
        );
        if !ok {
            self.failures.push(format!("{name}: measured {value} > limit {limit}"));
        }
    }

    /// Record `value >= limit`.
    pub fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value >= limit;
        println!(
            "CHECK {name}: measured {value:.5} limit >= {limit:.5} -> {}",
            if ok { "pass" } else { "MISS" }
        );
        if !ok {
            self.failures.push(format!("{name}: measured {value} < limit {limit}"));
        }
    }

    /// Record a boolean condition.
    pub fn is_true(&mut self, name: &str, ok: bool, detail: &str) {
        println!("CHECK {name}: {detail} -> {}", if ok { "pass" } else { "MISS" });
        if !ok {
            self.failures.push(format!("{name}: {detail}"));
        }
    }

    /// Fail the test with every recorded miss.
    pub fn finish(self) {
        assert!(
            self.failures.is_empty(),
            "registered checks missed: {:#?}",
            self.failures
        );
    }
}
