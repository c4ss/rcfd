//! Monitor history store with incremental merge (no UI code).

use serde_json::Value;
use std::collections::BTreeMap;

/// One monitor series.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Series {
    /// Iteration or time.
    pub x: Vec<f64>,
    /// Value.
    pub y: Vec<f64>,
}

/// All series of the current run.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MonitorStore {
    /// Series by monitor name.
    pub series: BTreeMap<String, Series>,
}

impl MonitorStore {
    /// Largest x seen in any series (the `since` value for the next poll).
    pub fn since(&self) -> Option<f64> {
        self.series
            .values()
            .filter_map(|s| s.x.last().copied())
            .fold(None, |a, x| Some(a.map_or(x, |m: f64| m.max(x))))
    }

    /// Forget everything (new run or new case).
    pub fn clear(&mut self) {
        self.series.clear();
    }

    /// Merge an `rcfd-monitors` document (full or `since`-filtered). Samples at or
    /// before a series' last x are ignored, so merging the same document twice is a no-op.
    /// Returns the number of samples added.
    pub fn merge(&mut self, doc: &Value) -> usize {
        let mut added = 0;
        for (name, s) in doc["series"].as_object().into_iter().flatten() {
            let (Some(x), Some(y)) = (s["x"].as_array(), s["y"].as_array()) else {
                continue;
            };
            let dst = self.series.entry(name.clone()).or_default();
            for (xv, yv) in x.iter().zip(y) {
                let Some(xf) = xv.as_f64() else { continue };
                if dst.x.last().is_some_and(|l| xf <= *l) {
                    continue;
                }
                dst.x.push(xf);
                // Non-finite residuals arrive as null; keep them as NaN so plots show the gap.
                dst.y.push(yv.as_f64().unwrap_or(f64::NAN));
                added += 1;
            }
        }
        added
    }

    /// Points for plotting `name`; with `log` the y values are log10 and non-positive or
    /// non-finite samples are dropped.
    pub fn plot_points(&self, name: &str, log: bool) -> Vec<[f64; 2]> {
        let Some(s) = self.series.get(name) else {
            return vec![];
        };
        s.x.iter()
            .zip(&s.y)
            .filter_map(|(&x, &y)| {
                if log {
                    (y.is_finite() && y > 0.0).then(|| [x, y.log10()])
                } else {
                    y.is_finite().then_some([x, y])
                }
            })
            .collect()
    }

    /// Like [`MonitorStore::plot_points`] but reduced to at most `max_points` by [`decimate_minmax`].
    pub fn plot_points_decimated(&self, name: &str, log: bool, max_points: usize) -> Vec<[f64; 2]> {
        decimate_minmax(&self.plot_points(name, log), max_points)
    }

    /// Names of series that are residual-like (used for the residual plot).
    pub fn residual_names(&self) -> Vec<String> {
        self.series
            .keys()
            .filter(|k| k.starts_with("res_") || *k == "continuity")
            .cloned()
            .collect()
    }

    /// Names of the other series (reports, forces).
    pub fn report_names(&self) -> Vec<String> {
        self.series
            .keys()
            .filter(|k| !(k.starts_with("res_") || *k == "continuity"))
            .cloned()
            .collect()
    }
}

/// Points handed to a plot per series (see [`decimate_minmax`]).
pub const MAX_PLOT_POINTS: usize = 4000;

/// Reduce `points` (x increasing) to at most `max_points` while keeping the first and last point and, for every
/// bucket of consecutive samples, the sample with the smallest and the one with the largest y (in original order).
/// The result is a subsequence of the input; short inputs are returned unchanged.
pub fn decimate_minmax(points: &[[f64; 2]], max_points: usize) -> Vec<[f64; 2]> {
    let n = points.len();
    if n <= max_points {
        return points.to_vec();
    }
    if max_points < 4 {
        return vec![points[0], points[n - 1]];
    }
    // Interior samples 1..n-1 are split into `nb` buckets; each keeps at most two.
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
