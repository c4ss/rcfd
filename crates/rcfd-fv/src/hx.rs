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
    /// Crossflow, both streams unmixed (the exact Poisson-tail series solution, ADR 0044 D1).
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
        Arrangement::CrossBothUnmixed => cross_both_unmixed(n, c),
        Arrangement::ShellAndTube => {
            let w = (1.0 + c * c).sqrt();
            let e = (-n * w).exp();
            2.0 / (1.0 + c + w * (1.0 + e) / (1.0 - e))
        }
    }
}

/// `ln k!`: exact sum for small `k`, Stirling series (error below 1e-15) from `k = 20`.
fn ln_factorial(k: u64) -> f64 {
    if k < 20 {
        return (2..=k).map(|j| (j as f64).ln()).sum();
    }
    let x = k as f64;
    let (x2, x3) = (x * x, x * x * x);
    x * x.ln() - x + 0.5 * (2.0 * std::f64::consts::PI * x).ln() + 1.0 / (12.0 * x) - 1.0 / (360.0 * x3)
        + 1.0 / (1260.0 * x3 * x2)
        - 1.0 / (1680.0 * x3 * x3 * x)
}

/// Poisson probability `e^-x x^k / k!` evaluated in log space.
fn poisson_pmf(x: f64, k: u64) -> f64 {
    (k as f64 * x.ln() - x - ln_factorial(k)).exp()
}

/// Index below which `P(n, x) = Pr(Poisson(x) >= n)` equals 1 to better than `e^-70`.
fn poisson_lower_start(x: f64) -> u64 {
    let s = (x - 12.0 * x.sqrt() - 20.0).floor();
    if s < 0.0 {
        0
    } else {
        s as u64
    }
}

/// Index above which `P(n, x)` is below `e^-70` (the upper tail is negligible).
fn poisson_upper_top(x: f64) -> u64 {
    (x + 12.0 * x.sqrt() + 40.0).ceil() as u64
}

/// `P(n, x) = Pr(Poisson(x) >= n)` for `n = n_lo..=n_hi` (`n_lo >= 1`). Only sums of positive terms are used: for `n <= x` the
/// complement `1 - sum_{k<n} pmf` (where `P >= 1/2`), for `n > x` the upper tail summed downwards from where it is negligible, so
/// no small tail is ever obtained by subtracting from 1.
fn poisson_tail_series(x: f64, n_lo: u64, n_hi: u64) -> Vec<f64> {
    let len = (n_hi - n_lo + 1) as usize;
    let mut out = vec![0.0; len];
    let s = poisson_lower_start(x);
    let fx = x.floor() as u64;
    // n <= s: exactly 1
    for n in n_lo..=n_hi.min(s) {
        out[(n - n_lo) as usize] = 1.0;
    }
    // s < n <= floor(x): 1 - sum_{k=s}^{n-1} pmf(k)
    let mut lower = 0.0;
    for n in (s + 1)..=fx.min(n_hi) {
        lower += poisson_pmf(x, n - 1);
        if n >= n_lo {
            out[(n - n_lo) as usize] = 1.0 - lower;
        }
    }
    // n > floor(x): upper tail summed downwards
    let top = poisson_upper_top(x);
    let mut tail = 0.0;
    let mut n = top;
    while n > fx {
        tail += poisson_pmf(x, n);
        if n >= n_lo && n <= n_hi {
            out[(n - n_lo) as usize] = tail;
        }
        n -= 1;
    }
    out
}

/// Exact effectiveness of the crossflow exchanger with both streams unmixed (ADR 0044 D1):
/// `eps = (1/M) sum_{n>=1} P(n, N) P(n, M)`, `M = N Cr`, `P(n, y) = Pr(Poisson(y) >= n)`. Derived from the double Laplace transform
/// of the two first-order balances; `0 < cr <= 1`, `n > 0`.
fn cross_both_unmixed(n: f64, cr: f64) -> f64 {
    let m = n * cr;
    if m < 1e-300 {
        return -(-n).exp_m1();
    }
    if n > 1e9 {
        // asymptotics: exponentially close to 1 for Cr < 1; 1 - 1/sqrt(pi N) at Cr = 1
        return if cr < 1.0 {
            1.0
        } else {
            1.0 - 1.0 / (std::f64::consts::PI * n).sqrt()
        };
    }
    let n0 = poisson_lower_start(n).min(poisson_lower_start(m)).max(1);
    let n_end = poisson_upper_top(n).min(poisson_upper_top(m));
    let pn = poisson_tail_series(n, n0, n_end);
    let pm = poisson_tail_series(m, n0, n_end);
    let sum: f64 = pn.iter().zip(&pm).map(|(a, b)| a * b).sum();
    (((n0 - 1) as f64 + sum) / m).min(1.0)
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
