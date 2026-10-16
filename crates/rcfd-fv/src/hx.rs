//! Heat-exchanger macro models (F-217, ADR 0039 D2): effectiveness-NTU closed forms for the standard flow arrangements,
//! the two-stream exchanger solution (heat duty, outlet temperatures) and the pressure-loss closures of an exchanger core.
//!
//! `NTU = UA / C_min` and `Cr = C_min / C_max` with the capacity rates `C = m cp` (W/K). Heat to the primary stream is
//! `Q = eps C_min (T_secondary,in - T_primary,in)`.

/// Flow arrangement of the two streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arrangement {
    /// Streams in opposite directions.
    Counterflow,
    /// Streams in the same direction.
    Parallel,
    /// Crossflow, both streams unmixed (closed approximation of the series solution).
    CrossBothUnmixed,
    /// Crossflow, the stream of larger capacity rate mixed, the other unmixed.
    CrossCmaxMixed,
    /// Crossflow, the stream of smaller capacity rate mixed, the other unmixed.
    CrossCminMixed,
    /// One shell pass, two (or any even number of) tube passes.
    ShellAndTube,
}

impl Arrangement {
    /// Setting names (`components.<n>.arrangement`).
    pub const NAMES: [&'static str; 6] = [
        "counterflow",
        "parallel",
        "crossflow_unmixed",
        "crossflow_cmax_mixed",
        "crossflow_cmin_mixed",
        "shell_and_tube",
    ];

    /// Parse a setting name.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "counterflow" => Arrangement::Counterflow,
            "parallel" => Arrangement::Parallel,
            "crossflow_unmixed" => Arrangement::CrossBothUnmixed,
            "crossflow_cmax_mixed" => Arrangement::CrossCmaxMixed,
            "crossflow_cmin_mixed" => Arrangement::CrossCminMixed,
            "shell_and_tube" => Arrangement::ShellAndTube,
            _ => return None,
        })
    }
}

/// Effectiveness `eps(NTU, Cr)` with `ntu >= 0` and `cr` clamped to `[0, 1]`.
pub fn effectiveness(arr: Arrangement, ntu: f64, cr: f64) -> f64 {
    let n = ntu.max(0.0);
    let c = cr.clamp(0.0, 1.0);
    if n == 0.0 {
        return 0.0;
    }
    if c == 0.0 {
        return 1.0 - (-n).exp();
    }
    match arr {
        Arrangement::Counterflow => {
            if (1.0 - c).abs() < 1e-9 {
                n / (1.0 + n)
            } else {
                let e = (-n * (1.0 - c)).exp();
                (1.0 - e) / (1.0 - c * e)
            }
        }
        Arrangement::Parallel => (1.0 - (-n * (1.0 + c)).exp()) / (1.0 + c),
        Arrangement::CrossCmaxMixed => (1.0 - (-c * (1.0 - (-n).exp())).exp()) / c,
        Arrangement::CrossCminMixed => 1.0 - (-(1.0 - (-n * c).exp()) / c).exp(),
        Arrangement::CrossBothUnmixed => 1.0 - (n.powf(0.22) / c * ((-c * n.powf(0.78)).exp() - 1.0)).exp(),
        Arrangement::ShellAndTube => {
            let w = (1.0 + c * c).sqrt();
            let e = (-n * w).exp();
            2.0 / (1.0 + c + w * (1.0 + e) / (1.0 - e))
        }
    }
}

/// Outcome of the two-stream exchanger solution.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExchangerResult {
    /// Heat to the primary stream (W), positive when the secondary is hotter.
    pub q: f64,
    /// Primary outlet temperature.
    pub t_primary_out: f64,
    /// Secondary outlet temperature.
    pub t_secondary_out: f64,
    /// Effectiveness used.
    pub eps: f64,
    /// `UA / C_min` (infinite when `C_min` is zero).
    pub ntu: f64,
    /// `C_min / C_max` (0 for an infinite secondary).
    pub cr: f64,
}

/// Two-stream exchanger: capacity rates `c_primary` and `c_secondary` (W/K; `c_secondary = 0` means an infinite secondary
/// stream whose temperature does not change), conductance `ua` (W/K), inlet temperatures. A non-positive primary capacity
/// rate (no flow, or a reversed flow) transfers no heat.
pub fn solve_exchanger(
    arr: Arrangement,
    c_primary: f64,
    c_secondary: f64,
    ua: f64,
    t_primary_in: f64,
    t_secondary_in: f64,
) -> ExchangerResult {
    if c_primary.is_nan() || c_primary <= 0.0 || ua.is_nan() || ua <= 0.0 {
        return ExchangerResult {
            q: 0.0,
            t_primary_out: t_primary_in,
            t_secondary_out: t_secondary_in,
            eps: 0.0,
            ntu: if c_primary > 0.0 { 0.0 } else { f64::INFINITY },
            cr: 0.0,
        };
    }
    let (c_min, cr) = if c_secondary > 0.0 {
        let (lo, hi) = (c_primary.min(c_secondary), c_primary.max(c_secondary));
        (lo, lo / hi)
    } else {
        (c_primary, 0.0)
    };
    let ntu = ua / c_min;
    let eps = effectiveness(arr, ntu, cr);
    let q = eps * c_min * (t_secondary_in - t_primary_in);
    ExchangerResult {
        q,
        t_primary_out: t_primary_in + q / c_primary,
        t_secondary_out: if c_secondary > 0.0 {
            t_secondary_in - q / c_secondary
        } else {
            t_secondary_in
        },
        eps,
        ntu,
        cr,
    }
}

/// `NTU` that gives the effectiveness `eps` (`0 <= eps < eps_max(Cr)`), by bisection (closed form for counterflow). `None`
/// when `eps` is not reachable.
pub fn ntu_for_effectiveness(arr: Arrangement, eps: f64, cr: f64) -> Option<f64> {
    let c = cr.clamp(0.0, 1.0);
    if !(0.0..1.0).contains(&eps) {
        return None;
    }
    if eps == 0.0 {
        return Some(0.0);
    }
    if arr == Arrangement::Counterflow && c < 1.0 - 1e-9 && c > 0.0 {
        return Some(((1.0 - eps * c) / (1.0 - eps)).ln() / (1.0 - c));
    }
    if arr == Arrangement::Counterflow && c == 0.0 {
        return Some(-(1.0 - eps).ln());
    }
    let (mut lo, mut hi) = (0.0, 1.0);
    while effectiveness(arr, hi, c) < eps {
        hi *= 2.0;
        if hi > 1e8 {
            return None;
        }
    }
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if effectiveness(arr, mid, c) < eps {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Some(0.5 * (lo + hi))
}

/// Pressure drop (Pa) of an exchanger core, `r1 v + k rho v |v| / 2`: a linear (viscous-type) part with the coefficient `r1`
/// [Pa s/m] and a loss-coefficient part with `k` (dimensionless), `v` the face-normal superficial velocity.
pub fn core_dp(r1: f64, k: f64, rho: f64, v: f64) -> f64 {
    r1 * v + k * rho * v * v.abs() / 2.0
}

/// Pressure drop (Pa) of a vent with the loss coefficient `k`, `k rho v |v| / 2`.
pub fn vent_dp(k: f64, rho: f64, v: f64) -> f64 {
    core_dp(0.0, k, rho, v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counterflow_special_cases() {
        assert!((effectiveness(Arrangement::Counterflow, 1.0, 1.0) - 0.5).abs() < 1e-15);
        assert!((effectiveness(Arrangement::Counterflow, 2.0, 0.0) - (1.0 - (-2.0_f64).exp())).abs() < 1e-15);
        assert_eq!(effectiveness(Arrangement::Parallel, 0.0, 0.5), 0.0);
    }

    #[test]
    fn exchanger_balance_and_no_flow() {
        let r = solve_exchanger(Arrangement::Counterflow, 40.0, 20.0, 30.0, 300.0, 350.0);
        assert!((40.0 * (r.t_primary_out - 300.0) - 20.0 * (350.0 - r.t_secondary_out)).abs() < 1e-9);
        let z = solve_exchanger(Arrangement::Counterflow, -1.0, 20.0, 30.0, 300.0, 350.0);
        assert_eq!(z.q, 0.0);
    }
}
