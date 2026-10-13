//! Convergence and interrupt conditions evaluated on monitor series (F-109).
//!
//! A [`Condition`] looks at the tail of one monitored series (a residual or a
//! report such as a drag coefficient) and decides whether it is met. The run
//! controller stops when any (or all) conditions are met and records the
//! human-readable reason returned by [`Condition::check`].

use rcfd_core::{Error, Result};

/// How a series is judged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionKind {
    /// `max - min` over the last `window` samples is at most the tolerance.
    WindowRange,
    /// `|y_n - y_(n-1)|` is at most the tolerance.
    Change,
    /// The last value is at most (below) the tolerance, e.g. a residual target.
    Below,
    /// The last value is at least (above) the tolerance, e.g. a divergence guard.
    Above,
}

impl ConditionKind {
    /// Parse the settings-tree spelling (`window_range`, `change`, `below`, `above`).
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "window_range" => Self::WindowRange,
            "change" => Self::Change,
            "below" => Self::Below,
            "above" => Self::Above,
            _ => return None,
        })
    }
}

/// One convergence / interrupt condition on a named monitor series.
#[derive(Debug, Clone, PartialEq)]
pub struct Condition {
    /// Condition name (settings-tree key).
    pub name: String,
    /// Monitored series name.
    pub monitor: String,
    /// Kind of test.
    pub kind: ConditionKind,
    /// Tolerance or threshold.
    pub tolerance: f64,
    /// Window length in samples (window range; at least 2).
    pub window: usize,
    /// Interpret range / change relative to the magnitude of the monitored value.
    pub relative: bool,
}

impl Condition {
    /// Validate the numbers of a condition.
    pub fn validate(&self) -> Result<()> {
        if !self.tolerance.is_finite() {
            return Err(Error::InvalidValue {
                path: format!("convergence.{}.tolerance", self.name),
                reason: "tolerance must be finite".into(),
            });
        }
        if self.kind == ConditionKind::WindowRange && self.window < 2 {
            return Err(Error::InvalidValue {
                path: format!("convergence.{}.window", self.name),
                reason: "a window range needs at least 2 samples".into(),
            });
        }
        Ok(())
    }

    /// Test the condition against the series values `y`; `Some(reason)` when met.
    pub fn check(&self, y: &[f64]) -> Option<String> {
        let last = *y.last()?;
        if !last.is_finite() {
            return None;
        }
        match self.kind {
            ConditionKind::Below => (last <= self.tolerance).then(|| {
                format!(
                    "convergence condition '{}' met: {} = {:.3e} <= {:.3e}",
                    self.name, self.monitor, last, self.tolerance
                )
            }),
            ConditionKind::Above => (last >= self.tolerance).then(|| {
                format!(
                    "interrupt condition '{}' met: {} = {:.3e} >= {:.3e}",
                    self.name, self.monitor, last, self.tolerance
                )
            }),
            ConditionKind::Change => {
                if y.len() < 2 {
                    return None;
                }
                let prev = y[y.len() - 2];
                let mut d = (last - prev).abs();
                if self.relative {
                    d /= prev.abs().max(f64::MIN_POSITIVE);
                }
                (d.is_finite() && d <= self.tolerance).then(|| {
                    format!(
                        "convergence condition '{}' met: change of {} per sample {:.3e} <= {:.3e}",
                        self.name, self.monitor, d, self.tolerance
                    )
                })
            }
            ConditionKind::WindowRange => {
                if y.len() < self.window {
                    return None;
                }
                let w = &y[y.len() - self.window..];
                if w.iter().any(|v| !v.is_finite()) {
                    return None;
                }
                let (lo, hi) = w
                    .iter()
                    .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), &v| (l.min(v), h.max(v)));
                let mut r = hi - lo;
                if self.relative {
                    r /= hi.abs().max(lo.abs()).max(f64::MIN_POSITIVE);
                }
                (r <= self.tolerance).then(|| {
                    format!(
                        "convergence condition '{}' met: range of {} over the last {} samples {:.3e} <= {:.3e}",
                        self.name, self.monitor, self.window, r, self.tolerance
                    )
                })
            }
        }
    }
}

/// How several conditions combine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Combine {
    /// Stop when any condition is met.
    Any,
    /// Stop when all conditions are met.
    All,
}

/// Evaluate `conditions` on the series returned by `series(monitor)`. Returns the
/// stop reason when the combined criterion is met. With no conditions nothing stops.
pub fn evaluate<'a>(
    conditions: &[Condition],
    combine: Combine,
    series: impl Fn(&str) -> Option<&'a [f64]>,
) -> Option<String> {
    if conditions.is_empty() {
        return None;
    }
    let results: Vec<Option<String>> = conditions
        .iter()
        .map(|c| series(&c.monitor).and_then(|y| c.check(y)))
        .collect();
    match combine {
        Combine::Any => results.into_iter().flatten().next(),
        Combine::All => {
            if results.iter().all(Option::is_some) {
                Some(results.into_iter().flatten().collect::<Vec<_>>().join("; "))
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cond(kind: ConditionKind, tol: f64, window: usize, relative: bool) -> Condition {
        Condition {
            name: "c".into(),
            monitor: "cd".into(),
            kind,
            tolerance: tol,
            window,
            relative,
        }
    }

    #[test]
    fn window_range_needs_a_full_flat_window() {
        let c = cond(ConditionKind::WindowRange, 1e-3, 4, false);
        assert!(c.check(&[1.0, 1.0, 1.0]).is_none(), "too short");
        assert!(
            c.check(&[5.0, 1.0, 1.0004, 0.9998, 1.0]).is_some(),
            "old transient is outside the window"
        );
        assert!(c.check(&[1.0, 1.01, 1.0, 1.0]).is_none());
        let r = cond(ConditionKind::WindowRange, 1e-3, 3, true);
        assert!(r.check(&[100.0, 100.05, 100.02]).is_some());
        assert!(r.check(&[1.0, 1.05, 1.02]).is_none());
    }

    #[test]
    fn change_below_above_and_nonfinite() {
        let ch = cond(ConditionKind::Change, 1e-4, 2, false);
        assert!(ch.check(&[1.0]).is_none());
        assert!(ch.check(&[1.0, 1.00005]).is_some());
        assert!(ch.check(&[1.0, 1.01]).is_none());
        let b = cond(ConditionKind::Below, 1e-6, 2, false);
        assert!(b.check(&[1.0, 5e-7]).unwrap().contains("<="));
        assert!(b.check(&[1.0, 2e-6]).is_none());
        let a = cond(ConditionKind::Above, 1e3, 2, false);
        assert!(a.check(&[1.0, 2e3]).unwrap().contains("interrupt"));
        assert!(a.check(&[1.0, f64::NAN]).is_none());
        assert!(cond(ConditionKind::WindowRange, 1.0, 1, false).validate().is_err());
    }

    #[test]
    fn any_and_all_combination() {
        let conds = vec![
            Condition {
                name: "a".into(),
                monitor: "x".into(),
                ..cond(ConditionKind::Below, 1.0, 2, false)
            },
            Condition {
                name: "b".into(),
                monitor: "y".into(),
                ..cond(ConditionKind::Below, 1.0, 2, false)
            },
        ];
        let x = [0.5];
        let y = [2.0];
        let get = |n: &str| match n {
            "x" => Some(&x[..]),
            "y" => Some(&y[..]),
            _ => None,
        };
        assert!(evaluate(&conds, Combine::Any, get).unwrap().contains("'a'"));
        assert!(evaluate(&conds, Combine::All, get).is_none());
        assert!(evaluate(&[], Combine::Any, get).is_none());
        let missing = vec![Condition {
            monitor: "nope".into(),
            ..conds[0].clone()
        }];
        assert!(evaluate(&missing, Combine::Any, get).is_none());
    }
}
