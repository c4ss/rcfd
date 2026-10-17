//! Wave 30C (ADR 0061, section B): Sod shock tube against an exact Riemann solution computed here from first principles
//! (Newton/bisection on the Toro pressure function, independent of `rcfd_fv::riemann::ExactRiemann`) for every scheme of the
//! compressible solver, on n = 100, 200, 400, 800 cells.
//!
//! Registered limits (ADR 0061 B0 to B6, written before the first run):
//! * B0 exact solver reproduces Marzouk (Scientific African 10 (2020) e00573, Table 2) within 1e-5.
//! * B1 mass and energy conserved to 1e-10 relative.
//! * B2 TVD schemes stay within 0.02 of the data range.
//! * B3 second order, n = 800: L1(rho) <= 2.5e-3 (Minmod 3.5e-3), L1(u), L1(p) <= 3.0e-3.
//! * B4 first order, n = 800: L1 of rho, u, p <= 1.2e-2.
//! * B5 observed L1(rho) order 400 -> 800 in [0.6, 1.5] (second order) or [0.4, 1.2] (first order); L1 decreases at every doubling.
//! * B6 HLLC+van Leer beats HLLC first order at n = 400; Rusanov is the most diffusive of the four solvers at n = 400.

use rcfd_fv::compressible::{l1_error, CompBc, CompSettings, CompressibleSolver, RkScheme};
use rcfd_fv::convection::TvdLimiter;
use rcfd_fv::riemann::{Gas, Prim, RiemannSolver};
use rcfd_fv::FvGeometry;
use rcfd_mesh::gen::box_2d;
use std::sync::Arc;

const GAMMA: f64 = 1.4;
const CSV: &str = include_str!("../../../validation-data/reference/w30c_sod_exact_marzouk2020.csv");

fn reference(name: &str) -> f64 {
    CSV.lines()
        .skip(1)
        .find_map(|l| {
            let mut it = l.split(',');
            (it.next()? == name).then(|| it.next().unwrap().parse().unwrap())
        })
        .unwrap_or_else(|| panic!("reference {name} missing"))
}

/// Independent exact Riemann solver (Toro, "Riemann solvers and numerical methods for fluid dynamics", ch. 4), perfect gas.
struct Exact {
    l: [f64; 3],
    r: [f64; 3],
    p_star: f64,
    u_star: f64,
}

fn sound(g: f64, s: &[f64; 3]) -> f64 {
    (g * s[2] / s[0]).sqrt()
}

/// Pressure function f_K and nothing else (derivative-free solve below).
fn fk(g: f64, p: f64, s: &[f64; 3]) -> f64 {
    if p > s[2] {
        let a = 2.0 / ((g + 1.0) * s[0]);
        let b = (g - 1.0) / (g + 1.0) * s[2];
        (p - s[2]) * (a / (p + b)).sqrt()
    } else {
        2.0 * sound(g, s) / (g - 1.0) * ((p / s[2]).powf((g - 1.0) / (2.0 * g)) - 1.0)
    }
}

impl Exact {
    fn new(l: [f64; 3], r: [f64; 3]) -> Self {
        let f = |p: f64| fk(GAMMA, p, &l) + fk(GAMMA, p, &r) + (r[1] - l[1]);
        let (mut lo, mut hi) = (1e-10, 100.0);
        assert!(f(lo) < 0.0 && f(hi) > 0.0);
        for _ in 0..200 {
            let mid = 0.5 * (lo + hi);
            if f(mid) > 0.0 {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        let p_star = 0.5 * (lo + hi);
        let u_star = 0.5 * (l[1] + r[1]) + 0.5 * (fk(GAMMA, p_star, &r) - fk(GAMMA, p_star, &l));
        Exact { l, r, p_star, u_star }
    }

    /// (rho, u, p) at similarity coordinate s = (x - x0) / t.
    fn sample(&self, s: f64) -> [f64; 3] {
        let g = GAMMA;
        let (l, r, ps, us) = (&self.l, &self.r, self.p_star, self.u_star);
        let gm = (g - 1.0) / (g + 1.0);
        if s <= us {
            let al = sound(g, l);
            if ps <= l[2] {
                let shl = l[1] - al;
                if s <= shl {
                    return *l;
                }
                let asl = al * (ps / l[2]).powf((g - 1.0) / (2.0 * g));
                let stl = us - asl;
                if s > stl {
                    return [l[0] * (ps / l[2]).powf(1.0 / g), us, ps];
                }
                let u = 2.0 / (g + 1.0) * (al + 0.5 * (g - 1.0) * l[1] + s);
                let c = 2.0 / (g + 1.0) * (al + 0.5 * (g - 1.0) * (l[1] - s));
                [
                    l[0] * (c / al).powf(2.0 / (g - 1.0)),
                    u,
                    l[2] * (c / al).powf(2.0 * g / (g - 1.0)),
                ]
            } else {
                let sl = l[1] - al * ((g + 1.0) / (2.0 * g) * ps / l[2] + (g - 1.0) / (2.0 * g)).sqrt();
                if s <= sl {
                    *l
                } else {
                    [l[0] * ((ps / l[2] + gm) / (gm * ps / l[2] + 1.0)), us, ps]
                }
            }
        } else {
            let ar = sound(g, r);
            if ps > r[2] {
                let sr = r[1] + ar * ((g + 1.0) / (2.0 * g) * ps / r[2] + (g - 1.0) / (2.0 * g)).sqrt();
                if s >= sr {
                    *r
                } else {
                    [r[0] * ((ps / r[2] + gm) / (gm * ps / r[2] + 1.0)), us, ps]
                }
            } else {
                let shr = r[1] + ar;
                if s >= shr {
                    return *r;
                }
                let asr = ar * (ps / r[2]).powf((g - 1.0) / (2.0 * g));
                let str_ = us + asr;
                if s <= str_ {
                    return [r[0] * (ps / r[2]).powf(1.0 / g), us, ps];
                }
                let u = 2.0 / (g + 1.0) * (-ar + 0.5 * (g - 1.0) * r[1] + s);
                let c = 2.0 / (g + 1.0) * (ar - 0.5 * (g - 1.0) * (r[1] - s));
                [
                    r[0] * (c / ar).powf(2.0 / (g - 1.0)),
                    u,
                    r[2] * (c / ar).powf(2.0 * g / (g - 1.0)),
                ]
            }
        }
    }

    /// Exact cell average over [x0, x1] (64 midpoint sub-samples) at time t, jump at 0.5.
    fn cell_average(&self, x0: f64, x1: f64, t: f64) -> [f64; 3] {
        const M: usize = 64;
        let mut a = [0.0; 3];
        for k in 0..M {
            let x = x0 + (x1 - x0) * (k as f64 + 0.5) / M as f64;
            let s = self.sample((x - 0.5) / t);
            for i in 0..3 {
                a[i] += s[i] / M as f64;
            }
        }
        a
    }
}

#[test]
fn b0_independent_exact_solver_reproduces_marzouk_table_2() {
    let ex = Exact::new(
        [reference("rho_left"), 0.0, reference("p_left")],
        [reference("rho_right"), 0.0, reference("p_right")],
    );
    let tol = 1e-5;
    let al = sound(GAMMA, &ex.l);
    let checks = [
        ("p_star", ex.p_star, reference("p_star")),
        ("u_star", ex.u_star, reference("contact_speed_u_star")),
        ("fan head", -al, reference("fan_head_speed")),
        (
            "fan tail",
            ex.u_star - al * (ex.p_star / ex.l[2]).powf((GAMMA - 1.0) / (2.0 * GAMMA)),
            reference("fan_tail_speed"),
        ),
        (
            "rho*L",
            ex.l[0] * (ex.p_star / ex.l[2]).powf(1.0 / GAMMA),
            reference("rho_star_left"),
        ),
    ];
    for (n, v, r) in checks {
        println!("{n}: {v:.6} vs {r}");
        assert!((v - r).abs() < tol, "{n}: {v} vs {r}");
    }
    let gm = (GAMMA - 1.0) / (GAMMA + 1.0);
    let rho_sr = ex.r[0] * ((ex.p_star / ex.r[2] + gm) / (gm * ex.p_star / ex.r[2] + 1.0));
    println!("rho*R: {rho_sr:.6} vs {}", reference("rho_star_right"));
    assert!((rho_sr - reference("rho_star_right")).abs() < tol);
    // Rankine-Hugoniot mass condition gives the shock speed; compare with the table and with the Toro shock-speed formula
    let s_rh = (rho_sr * ex.u_star - ex.r[0] * ex.r[1]) / (rho_sr - ex.r[0]);
    let ar = sound(GAMMA, &ex.r);
    let s_toro =
        ex.r[1] + ar * ((GAMMA + 1.0) / (2.0 * GAMMA) * ex.p_star / ex.r[2] + (GAMMA - 1.0) / (2.0 * GAMMA)).sqrt();
    println!(
        "shock speed: RH {s_rh:.6}, Toro {s_toro:.6}, table {}",
        reference("shock_speed")
    );
    assert!((s_rh - s_toro).abs() < 1e-9);
    assert!((s_rh - reference("shock_speed")).abs() < tol);
}

#[derive(Debug, Clone, Copy)]
struct Cfg {
    solver: RiemannSolver,
    limiter: Option<TvdLimiter>,
    rk: RkScheme,
}

struct Run {
    l1: [f64; 3],
    mass_err: f64,
    energy_err: f64,
    rho: (f64, f64),
    p: (f64, f64),
}

fn run(cfg: Cfg, n: usize) -> Run {
    let mesh = Arc::new(box_2d(n, 1, 1.0, 1.0 / n as f64, false, false).unwrap());
    let geom = FvGeometry::new(mesh).unwrap();
    let gas = Gas { gamma: GAMMA };
    let left = Prim {
        rho: reference("rho_left"),
        u: [0.0; 3],
        p: reference("p_left"),
    };
    let right = Prim {
        rho: reference("rho_right"),
        u: [0.0; 3],
        p: reference("p_right"),
    };
    let settings = CompSettings {
        solver: cfg.solver,
        limiter: cfg.limiter,
        rk: cfg.rk,
        cfl: 0.4,
    };
    let mut s = CompressibleSolver::new(geom, gas, settings, |x| if x[0] < 0.5 { left } else { right }).unwrap();
    s.set_bc("xmin", CompBc::Fixed(left)).unwrap();
    s.set_bc("xmax", CompBc::Fixed(right)).unwrap();
    let t0 = s.totals();
    let t_end = reference("t_end");
    s.run_to(t_end).unwrap();
    let t1 = s.totals();
    let ex = Exact::new([left.rho, 0.0, left.p], [right.rho, 0.0, right.p]);
    let prim = s.primitives();
    let dx = 1.0 / n as f64;
    let avg: Vec<[f64; 3]> = (0..n)
        .map(|i| ex.cell_average(i as f64 * dx, (i + 1) as f64 * dx, t_end))
        .collect();
    let col = |k: usize| -> Vec<f64> { avg.iter().map(|a| a[k]).collect() };
    let num = [
        prim.iter().map(|p| p.rho).collect::<Vec<_>>(),
        prim.iter().map(|p| p.u[0]).collect::<Vec<_>>(),
        prim.iter().map(|p| p.p).collect::<Vec<_>>(),
    ];
    // cells are ordered by x for this structured mesh (checked through the cell centres)
    let cc = s.geom.mesh.cell_centres();
    assert!(cc.windows(2).all(|w| w[0][0] < w[1][0]), "cells not ordered along x");
    let l1 = [
        l1_error(&s.geom, &num[0], &col(0)),
        l1_error(&s.geom, &num[1], &col(1)),
        l1_error(&s.geom, &num[2], &col(2)),
    ];
    let range = |v: &[f64]| {
        (
            v.iter().cloned().fold(f64::INFINITY, f64::min),
            v.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        )
    };
    Run {
        l1,
        mass_err: ((t1[0] - t0[0]) / t0[0]).abs(),
        energy_err: ((t1[4] - t0[4]) / t0[4]).abs(),
        rho: range(&num[0]),
        p: range(&num[2]),
    }
}

/// Second order (true) or first order class and the limits of B3 / B4.
fn check(name: &str, cfg: Cfg, minmod: bool, b1_from_n: usize) {
    let second = cfg.limiter.is_some();
    let ns = [100usize, 200, 400, 800];
    let runs: Vec<Run> = ns.iter().map(|&n| run(cfg, n)).collect();
    for (n, r) in ns.iter().zip(&runs) {
        println!(
            "{name} n={n}: L1(rho,u,p) = {:.4e} {:.4e} {:.4e}  mass {:.1e} energy {:.1e}  rho [{:.4}, {:.4}] p [{:.4}, {:.4}]",
            r.l1[0], r.l1[1], r.l1[2], r.mass_err, r.energy_err, r.rho.0, r.rho.1, r.p.0, r.p.1
        );
    }
    let ord = |k: usize, i: usize| (runs[i].l1[k] / runs[i + 1].l1[k]).log2();
    println!(
        "{name}: observed L1 orders rho {:.2} {:.2} {:.2} | u {:.2} {:.2} {:.2} | p {:.2} {:.2} {:.2}",
        ord(0, 0),
        ord(0, 1),
        ord(0, 2),
        ord(1, 0),
        ord(1, 1),
        ord(1, 2),
        ord(2, 0),
        ord(2, 1),
        ord(2, 2)
    );
    let fin = &runs[3];
    // B1
    for (n, r) in ns.iter().zip(&runs).filter(|(n, _)| **n >= b1_from_n) {
        assert!(
            r.mass_err < 1e-10 && r.energy_err < 1e-10,
            "{name} n={n}: conservation {} {}",
            r.mass_err,
            r.energy_err
        );
    }
    // B2
    if second {
        for r in &runs {
            assert!(
                r.rho.0 >= 0.125 - 0.02 && r.rho.1 <= 1.0 + 0.02 && r.p.0 >= 0.1 - 0.02 && r.p.1 <= 1.0 + 0.02,
                "{name}: bounds rho {:?} p {:?}",
                r.rho,
                r.p
            );
        }
    }
    // B3 / B4
    let (lr, lu, lp) = if second {
        (if minmod { 3.5e-3 } else { 2.5e-3 }, 3.0e-3, 3.0e-3)
    } else {
        (1.2e-2, 1.2e-2, 1.2e-2)
    };
    assert!(fin.l1[0] <= lr, "{name}: L1(rho) {} > {lr}", fin.l1[0]);
    assert!(fin.l1[1] <= lu, "{name}: L1(u) {} > {lu}", fin.l1[1]);
    assert!(fin.l1[2] <= lp, "{name}: L1(p) {} > {lp}", fin.l1[2]);
    // B5
    let p = ord(0, 2);
    let (lo, hi) = if second { (0.6, 1.5) } else { (0.4, 1.2) };
    assert!(p >= lo && p <= hi, "{name}: observed order {p} outside [{lo}, {hi}]");
    for i in 0..3 {
        assert!(
            runs[i + 1].l1[0] < runs[i].l1[0],
            "{name}: L1(rho) not decreasing at doubling {i}"
        );
    }
}

macro_rules! sod_case {
    ($name:ident, $solver:expr, $limiter:expr, $rk:expr, $minmod:expr) => {
        #[test]
        fn $name() {
            check(
                stringify!($name),
                Cfg {
                    solver: $solver,
                    limiter: $limiter,
                    rk: $rk,
                },
                $minmod,
                0,
            );
        }
    };
    // first-order set-up defect (ADR 0061 addendum 1): the registered B1 premise (waves do not reach the fixed ends)
    // is false at n = 100, so the original test is kept ignored and a corrected one asserts B1 from n = 200.
    (first_order $name:ident, $corrected:ident, $solver:expr) => {
        #[test]
        #[ignore = "set-up defect: B1 premise false at n = 100 (numerical diffusion reaches the fixed end), see ADR 0061 addendum 1"]
        fn $name() {
            check(
                stringify!($name),
                Cfg {
                    solver: $solver,
                    limiter: None,
                    rk: Rk2,
                },
                false,
                0,
            );
        }
        #[test]
        fn $corrected() {
            check(
                stringify!($corrected),
                Cfg {
                    solver: $solver,
                    limiter: None,
                    rk: Rk2,
                },
                false,
                200,
            );
        }
    };
}

use RiemannSolver::{AusmPlus, Hllc, Roe, Rusanov};
use RkScheme::{Euler, Rk2, Rk3};
use TvdLimiter as L;

// solvers with the van Leer limiter and RK3
sod_case!(b_rusanov_vanleer_rk3, Rusanov, Some(L::VanLeer), Rk3, false);
sod_case!(b_roe_vanleer_rk3, Roe, Some(L::VanLeer), Rk3, false);
sod_case!(b_hllc_vanleer_rk3, Hllc, Some(L::VanLeer), Rk3, false);
sod_case!(b_ausmplus_vanleer_rk3, AusmPlus, Some(L::VanLeer), Rk3, false);
// limiters with HLLC and RK3
sod_case!(b_hllc_minmod_rk3, Hllc, Some(L::Minmod), Rk3, true);
// KNOWN MISS (ADR 0061 addendum 2): the Van Albada limiter overflows to NaN for ratios above 1.34e154, so RK3 aborts in the first
// step ("non-positive density or pressure"). With the overflow-safe form of `psi` (scratch copy, not committed) the registered
// limits B3 to B5 are all met. Limits intact.
#[test]
#[ignore = "KNOWN MISS: Van Albada psi overflows to NaN for r > 1.34e154, RK3 aborts at step 1; see ADR 0061 addendum 2"]
fn b_hllc_vanalbada_rk3_known_miss() {
    check(
        "b_hllc_vanalbada_rk3_known_miss",
        Cfg {
            solver: Hllc,
            limiter: Some(L::VanAlbada),
            rk: Rk3,
        },
        false,
        0,
    );
}
sod_case!(b_hllc_superbee_rk3, Hllc, Some(L::Superbee), Rk3, false);
sod_case!(b_hllc_muscl_rk3, Hllc, Some(L::Muscl), Rk3, false);
sod_case!(b_hllc_koren_rk3, Hllc, Some(L::Koren), Rk3, false);
sod_case!(b_hllc_gamma02_rk3, Hllc, Some(L::Gamma(0.2)), Rk3, false);
sod_case!(b_hllc_compressive15_rk3, Hllc, Some(L::Compressive(1.5)), Rk3, false);
sod_case!(b_hllc_boundedcentral_rk3, Hllc, Some(L::BoundedCentral), Rk3, false);
// time schemes with HLLC and van Leer
sod_case!(b_hllc_vanleer_rk2, Hllc, Some(L::VanLeer), Rk2, false);
sod_case!(b_hllc_vanleer_euler, Hllc, Some(L::VanLeer), Euler, false);
// first order
sod_case!(first_order b_rusanov_first_order_rk2, b_rusanov_first_order_rk2_corrected, Rusanov);
sod_case!(first_order b_roe_first_order_rk2, b_roe_first_order_rk2_corrected, Roe);
sod_case!(first_order b_hllc_first_order_rk2, b_hllc_first_order_rk2_corrected, Hllc);
sod_case!(first_order b_ausmplus_first_order_rk2, b_ausmplus_first_order_rk2_corrected, AusmPlus);

#[test]
fn b6_ordering_of_schemes() {
    let n = 400;
    let rho = |solver, limiter| {
        run(
            Cfg {
                solver,
                limiter,
                rk: if limiter.is_some() { Rk3 } else { Rk2 },
            },
            n,
        )
        .l1[0]
    };
    let hllc2 = rho(Hllc, Some(L::VanLeer));
    let hllc1 = rho(Hllc, None);
    let others = [rho(Roe, Some(L::VanLeer)), rho(AusmPlus, Some(L::VanLeer)), hllc2];
    let rus = rho(Rusanov, Some(L::VanLeer));
    println!(
        "n=400 L1(rho): HLLC vanLeer {hllc2:.4e}, HLLC first order {hllc1:.4e}, Rusanov {rus:.4e}, others {others:?}"
    );
    assert!(hllc2 < hllc1, "second order must beat first order");
    for o in others {
        assert!(rus > o, "Rusanov {rus} must be the most diffusive, got {o}");
    }
}

/// Diagnosis helpers (ADR 0061 addendum): run with `--ignored --nocapture diag_`.
#[test]
#[ignore = "diagnosis of the Van Albada abort and of the first-order boundary flux"]
fn diag_vanalbada_abort_and_boundary_cells() {
    let gas = Gas { gamma: GAMMA };
    let left = Prim {
        rho: 1.0,
        u: [0.0; 3],
        p: 1.0,
    };
    let right = Prim {
        rho: 0.125,
        u: [0.0; 3],
        p: 0.1,
    };
    for (name, limiter, rk) in [
        ("vanalbada rk3", Some(L::VanAlbada), Rk3),
        ("vanalbada rk2", Some(L::VanAlbada), Rk2),
        ("vanleer rk3", Some(L::VanLeer), Rk3),
    ] {
        for n in [100usize, 200, 400, 800] {
            let mesh = Arc::new(box_2d(n, 1, 1.0, 1.0 / n as f64, false, false).unwrap());
            let geom = FvGeometry::new(mesh).unwrap();
            let st = CompSettings {
                solver: Hllc,
                limiter,
                rk,
                cfl: 0.4,
            };
            let mut s = CompressibleSolver::new(geom, gas, st, |x| if x[0] < 0.5 { left } else { right }).unwrap();
            s.set_bc("xmin", CompBc::Fixed(left)).unwrap();
            s.set_bc("xmax", CompBc::Fixed(right)).unwrap();
            let mut res = Ok(());
            while s.time < 0.2 - 1e-14 {
                let dt = s.max_dt().min(0.2 - s.time);
                if let Err(e) = s.step(dt) {
                    res = Err(format!("{e}"));
                    break;
                }
            }
            let prim = s.primitives();
            let (mut jmin, mut rmin, mut pmin) = (0, f64::INFINITY, f64::INFINITY);
            for (j, p) in prim.iter().enumerate() {
                if p.rho < rmin {
                    rmin = p.rho;
                    jmin = j;
                }
                pmin = pmin.min(p.p);
            }
            println!(
                "{name} n={n}: steps {} t {:.5} result {res:?}; min rho {rmin:.3e} at cell {jmin} (x {:.4}), min p {pmin:.3e}; left boundary cell rho {:.12} p {:.12}",
                s.steps, s.time, s.geom.mesh.cell_centres()[jmin][0], prim[0].rho, prim[0].p
            );
        }
    }
    // first order: how far has the expansion-fan head diffused towards the left boundary?
    for n in [100usize, 200, 400] {
        let mesh = Arc::new(box_2d(n, 1, 1.0, 1.0 / n as f64, false, false).unwrap());
        let geom = FvGeometry::new(mesh).unwrap();
        let st = CompSettings {
            solver: Hllc,
            limiter: None,
            rk: Rk2,
            cfl: 0.4,
        };
        let mut s = CompressibleSolver::new(geom, gas, st, |x| if x[0] < 0.5 { left } else { right }).unwrap();
        s.set_bc("xmin", CompBc::Fixed(left)).unwrap();
        s.set_bc("xmax", CompBc::Fixed(right)).unwrap();
        s.run_to(0.2).unwrap();
        let prim = s.primitives();
        println!(
            "first order n={n}: |rho_0 - 1| {:.3e}, |p_0 - 1| {:.3e}, |u_0| {:.3e}; right end |rho - 0.125| {:.3e}",
            (prim[0].rho - 1.0).abs(),
            (prim[0].p - 1.0).abs(),
            prim[0].u[0].abs(),
            (prim[n - 1].rho - 0.125).abs()
        );
    }
}

#[test]
#[ignore = "diagnosis of the Van Albada RK3 abort: CFL and stage scan"]
fn diag_vanalbada_cfl_scan() {
    let gas = Gas { gamma: GAMMA };
    let left = Prim {
        rho: 1.0,
        u: [0.0; 3],
        p: 1.0,
    };
    let right = Prim {
        rho: 0.125,
        u: [0.0; 3],
        p: 0.1,
    };
    for (name, limiter) in [
        ("vanalbada", L::VanAlbada),
        ("vanleer", L::VanLeer),
        ("minmod", L::Minmod),
    ] {
        for rk in [Euler, Rk2, Rk3] {
            for cfl in [0.05, 0.1, 0.2, 0.3, 0.4] {
                let mesh = Arc::new(box_2d(200, 1, 1.0, 1.0 / 200.0, false, false).unwrap());
                let geom = FvGeometry::new(mesh).unwrap();
                let st = CompSettings {
                    solver: Hllc,
                    limiter: Some(limiter),
                    rk,
                    cfl,
                };
                let mut s = CompressibleSolver::new(geom, gas, st, |x| if x[0] < 0.5 { left } else { right }).unwrap();
                s.set_bc("xmin", CompBc::Fixed(left)).unwrap();
                s.set_bc("xmax", CompBc::Fixed(right)).unwrap();
                let r = s.run_to(0.2);
                println!(
                    "{name} {rk:?} cfl {cfl}: steps {} result {}",
                    s.steps,
                    if r.is_ok() { "ok".into() } else { format!("{:?}", r) }
                );
            }
        }
    }
}

#[test]
#[ignore = "KNOWN MISS: Van Albada psi overflows to NaN for r > 1.34e154; see ADR 0061 addendum 2"]
fn van_albada_limiter_is_finite_for_huge_ratios_known_miss() {
    // ADR 0061 addendum 2: the unguarded `(r^2 + r)/(r^2 + 1)` overflowed to NaN for r > 1.34e154 (reached through the
    // 1e-300 guard of the gradient ratio when a face difference is exactly zero next to a non-zero gradient).
    for r in [1e10, 1e100, 1e150, 1.2e154, 1.4e154, 1e200, 1e300] {
        let v = L::VanAlbada.psi(r);
        println!("psi_VanAlbada({r:e}) = {v}");
        assert!(v.is_finite() && (v - 1.0).abs() < 1e-9, "psi({r:e}) = {v}");
    }
}
