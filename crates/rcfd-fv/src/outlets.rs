//! Pure helper functions of the boundary-condition catalogue (ADR 0022): outflow scaling with reverse-flow
//! prevention, area-mean pressure shift, radial-equilibrium integration and the target-mass-flow pressure updates.
//! No mesh is involved; the settings-tree layer (`rcfd-api::bc_cat`) feeds them with patch data.

/// Face normal speeds `q_f` of an `outflow` patch so that `sum q_f A_f = target` (volume flux).
///
/// `u_n[f]` is the owner-cell velocity component along the outward normal. The shape is `max(u_n, 0)` when
/// `prevent_reverse` else `u_n`; if its area integral is below `1e-6` of the integral of its magnitude the shape
/// falls back to 1 (uniform).
pub fn outflow_speeds(u_n: &[f64], area: &[f64], target: f64, prevent_reverse: bool) -> Vec<f64> {
    let shape: Vec<f64> = u_n
        .iter()
        .map(|&v| if prevent_reverse { v.max(0.0) } else { v })
        .collect();
    let net: f64 = shape.iter().zip(area).map(|(s, a)| s * a).sum();
    let mag: f64 = shape.iter().zip(area).map(|(s, a)| s.abs() * a).sum();
    let shape = if net.abs() <= 1e-6 * mag || mag == 0.0 {
        vec![1.0; u_n.len()]
    } else {
        shape
    };
    let net: f64 = shape.iter().zip(area).map(|(s, a)| s * a).sum();
    let scale = target / net;
    shape.iter().map(|s| s * scale).collect()
}

/// `p_set + (p_P - mean_A(p_P))` per face: the area mean of the result is `p_set`.
pub fn area_mean_shift(p_cell: &[f64], area: &[f64], p_set: f64) -> Vec<f64> {
    let total: f64 = area.iter().sum();
    let mean: f64 = p_cell.iter().zip(area).map(|(p, a)| p * a).sum::<f64>() / total;
    p_cell.iter().map(|p| p_set + (p - mean)).collect()
}

/// Reference of the radial-equilibrium pressure profile.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RadialReference {
    /// The face of smallest radius carries this (kinematic) pressure.
    Hub(f64),
    /// The area mean of the face pressures equals this value.
    AreaMean(f64),
}

/// Kinematic pressure of faces at radii `r` with tangential velocities `w` from `dp/dr = w^2/r` (trapezoid rule over
/// the faces sorted by radius). Returned in the input face order.
pub fn radial_equilibrium(r: &[f64], w: &[f64], area: &[f64], reference: RadialReference) -> Vec<f64> {
    let n = r.len();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| r[a].total_cmp(&r[b]));
    let g = |i: usize| w[i] * w[i] / r[i];
    let mut p = vec![0.0; n];
    for k in 1..n {
        let (a, b) = (order[k - 1], order[k]);
        p[b] = p[a] + 0.5 * (g(a) + g(b)) * (r[b] - r[a]);
    }
    let shift = match reference {
        RadialReference::Hub(v) => v - p[order[0]],
        RadialReference::AreaMean(v) => {
            let total: f64 = area.iter().sum();
            v - p.iter().zip(area).map(|(p, a)| p * a).sum::<f64>() / total
        }
    };
    p.iter().map(|x| x + shift).collect()
}

/// Update strategy of a target-mass-flow pressure outlet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressureUpdate {
    /// Dynamic-head sensitivity estimate.
    Bernoulli,
    /// Secant slope of the last two iterates (magnitude of `dp/dm`, which is negative), safeguarded.
    Secant,
    /// Secant slope accepted only when the last mass-flow change is not dominated by the spin-up transient
    /// (`|dm| <= 0.5 |e|`) and the slope lies within `[0.1, 10]` times the dynamic-head estimate; otherwise the dynamic-head
    /// slope (ADR 0055 D5).
    GuardedSecant,
}

/// State of the pressure update of one target-mass-flow outlet.
#[derive(Debug, Clone)]
pub struct PressureUpdater {
    /// Strategy.
    pub strategy: PressureUpdate,
    /// Relaxation `r` of the step.
    pub relax: f64,
    prev: Option<(f64, f64)>,
}

impl PressureUpdater {
    /// New updater without history.
    pub fn new(strategy: PressureUpdate, relax: f64) -> Self {
        PressureUpdater {
            strategy,
            relax,
            prev: None,
        }
    }

    /// Next kinematic pressure from the present pressure `p`, the outward mass flow `m`, the target, density and the
    /// patch area (ADR 0022 D6).
    pub fn step(&mut self, p: f64, m: f64, target: f64, rho: f64, area: f64) -> f64 {
        let e = m - target;
        let u_ref = m.abs().max(target.abs()) / (rho * area);
        let bernoulli = u_ref / (rho * area);
        let mut slope = bernoulli;
        if self.strategy == PressureUpdate::Secant {
            if let Some((p0, m0)) = self.prev {
                let dm = m - m0;
                if dm.abs() > 1e-9 * m.abs().max(target.abs()) {
                    let s = -(p - p0) / dm; // m falls as the back pressure rises
                    if s > 0.0 {
                        slope = s.clamp(0.01 * bernoulli, 1000.0 * bernoulli);
                    }
                }
            }
        }
        if self.strategy == PressureUpdate::GuardedSecant {
            if let Some((p0, m0)) = self.prev {
                let dm = m - m0;
                if dm.abs() > 1e-9 * m.abs().max(target.abs()) && dm.abs() <= 0.5 * e.abs() {
                    let s = -(p - p0) / dm;
                    if s >= 0.1 * bernoulli && s <= 10.0 * bernoulli {
                        slope = s;
                    }
                }
            }
        }
        self.prev = Some((p, m));
        p + self.relax * slope * e
    }
}
