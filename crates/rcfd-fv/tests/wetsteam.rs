//! F-221 (ADR 0217): non-equilibrium condensing steam. The registered checks of ADR 0217: fast (P1, P2, M1, L1, E1, C1, C1b,
//! S1, D1) and heavy, ignored (the Moses and Stein nozzle, case 203: S1n, S2, N1, Q1, V1, V2, G1g, NC1, NC2). The limits are
//! those of the ADR and are not changed after a run.
//!
//! Sources: Starzmann et al., ASME GT2016-57598 (the nozzle, its case 203 data, Eqs. 1-3, alpha = 11); Starzmann et al.,
//! GT2011-45672 (mean free path); Afzalifar et al., ISROMAC 2016 (critical radius, the mixture); Dykas et al., J. Power
//! Technologies 93 (2013) (Kantrowitz correction); IAPWS R1-76(2014) (surface tension). See ADR 0217 "Sources".

use rcfd_core::iapws::{self, R_WATER};
use rcfd_fv::compressible::{CompBc, CompSettings, CompressibleSolver, RkScheme};
use rcfd_fv::compressible_bc::TotalInlet;
use rcfd_fv::convection::TvdLimiter;
use rcfd_fv::geometry::FvGeometry;
use rcfd_fv::riemann::{ExactRiemann, Gas, Prim, RiemannSolver};
use rcfd_fv::wetsteam::{liquid_density, surface_tension, WetBc, WetPrim, WetSettings, WetSteamModel, WetSteamSolver};
use rcfd_mesh::gen::{box_2d, warp};
use std::f64::consts::PI;
use std::sync::{Arc, OnceLock};

// ---- case 203 (ADR 0217) -------------------------------------------------------------------------------------------------

const P0: f64 = 35.87e3;
const T0: f64 = 368.3;
const ALPHA: f64 = 11.0;
/// Throat half-height, the throat arc radius, the upstream arc radius and the junction (G1).
const H: f64 = 0.005;
const R1: f64 = 0.686;
const R2: f64 = 0.053;
const XJ: f64 = -0.020;
const X_IN: f64 = -0.050;
const LX: f64 = 0.115;

/// Moses and Stein case 203, [S1] Fig. 2: (x cm, p/p_t1), digitized from the vector paths (ADR 0217).
const DATA_P: [(f64, f64); 41] = [
    (1.5039, 0.43590),
    (1.6015, 0.42881),
    (1.7043, 0.42143),
    (1.8058, 0.41424),
    (1.9086, 0.40724),
    (2.0101, 0.40006),
    (2.1129, 0.39287),
    (2.2144, 0.38587),
    (2.3211, 0.37868),
    (2.4316, 0.37102),
    (2.5421, 0.36373),
    (2.6488, 0.35664),
    (2.7554, 0.34964),
    (2.8788, 0.34179),
    (2.9983, 0.33451),
    (3.1178, 0.32722),
    (3.2321, 0.32013),
    (3.3105, 0.31559),
    (3.3709, 0.31190),
    (3.4737, 0.31048),
    (3.6061, 0.31180),
    (3.6921, 0.31370),
    (3.8053, 0.31606),
    (3.9106, 0.31720),
    (4.0186, 0.31720),
    (4.1111, 0.31578),
    (4.1959, 0.31322),
    (4.2576, 0.31029),
    (4.3231, 0.30707),
    (4.4002, 0.30358),
    (4.4876, 0.29951),
    (4.6032, 0.29412),
    (4.7445, 0.28740),
    (4.8924, 0.28059),
    (5.0504, 0.27330),
    (5.2071, 0.26602),
    (5.3601, 0.25940),
    (5.5143, 0.25193),
    (5.6556, 0.24531),
    (5.7944, 0.23935),
    (5.9293, 0.23348),
];

/// Moses and Stein case 203, [S1] Fig. 2: (x cm, Sauter radius nm).
const DATA_R32: [(f64, f64); 17] = [
    (3.5239, 10.390),
    (3.6150, 11.600),
    (3.7205, 12.735),
    (3.8412, 13.682),
    (3.9980, 14.608),
    (4.1599, 15.460),
    (4.3205, 16.197),
    (4.4811, 16.784),
    (4.6418, 17.294),
    (4.8037, 17.748),
    (4.9643, 18.108),
    (5.1262, 18.449),
    (5.2868, 18.733),
    (5.4475, 18.978),
    (5.6132, 19.187),
    (5.7944, 19.376),
    (5.9666, 19.508),
];

fn model203() -> WetSteamModel {
    WetSteamModel::for_inlet(P0, T0, ALPHA).unwrap()
}

/// Wall half-height (m) of G1.
fn wall_h(x: f64) -> f64 {
    if x >= XJ {
        return H + R1 - (R1 * R1 - x * x).sqrt();
    }
    let s = -XJ / R1;
    let xc = -(R1 - R2) * s;
    let yc = H + R1 - (R1 - R2) * (1.0 - s * s).sqrt();
    yc - (R2 * R2 - (x - xc) * (x - xc)).sqrt()
}

fn wall_dhdx(x: f64) -> f64 {
    let e = 1e-7;
    (wall_h(x + e) - wall_h(x - e)) / (2.0 * e)
}

/// Mach number at area ratio `ar` on the subsonic or supersonic branch (bisection).
fn mach_of(g: f64, ar: f64, supersonic: bool) -> f64 {
    let f = |m: f64| {
        (1.0 / m) * ((2.0 / (g + 1.0)) * (1.0 + 0.5 * (g - 1.0) * m * m)).powf((g + 1.0) / (2.0 * (g - 1.0))) - ar
    };
    let (mut lo, mut hi) = if supersonic { (1.0, 5.0) } else { (1e-6, 1.0) };
    let flo = f(lo);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if (f(mid) > 0.0) == (flo > 0.0) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Dry isentropic quasi-1-D state `(rho, u, p)` at `x` (subsonic for `x < 0`, supersonic for `x > 0`).
fn isentropic(model: &WetSteamModel, x: f64) -> (f64, f64, f64) {
    let g = model.gamma_v();
    let m = mach_of(g, (wall_h(x) / H).max(1.0), x > 0.0);
    let t = T0 / (1.0 + 0.5 * (g - 1.0) * m * m);
    let p = P0 * (t / T0).powf(g / (g - 1.0));
    (p / (R_WATER * t), m * (g * R_WATER * t).sqrt(), p)
}

fn nozzle_mesh(nx: usize, ny: usize) -> FvGeometry {
    let base = box_2d(nx, ny, LX, 1.0, false, false).unwrap();
    let mesh = warp(&base, |p| {
        let x = X_IN + p[0];
        [x, p[1] * wall_h(x), p[2]]
    })
    .unwrap();
    FvGeometry::new(Arc::new(mesh)).unwrap()
}

fn total_inlet() -> TotalInlet {
    TotalInlet {
        p0: P0,
        t0: T0,
        r_gas: R_WATER,
        dir: [1.0, 0.0, 0.0],
    }
}

fn nozzle_solver(nx: usize, ny: usize, condensation: bool) -> WetSteamSolver {
    let model = model203();
    let settings = WetSettings {
        condensation,
        ..WetSettings::default()
    };
    let mut s = WetSteamSolver::new(nozzle_mesh(nx, ny), model, settings, |x| {
        let (rho, u, p) = isentropic(&model, x[0]);
        WetPrim {
            rho,
            u: [u, 0.0, 0.0],
            p,
            y: 0.0,
            n: 0.0,
        }
    })
    .unwrap();
    s.set_bc("xmin", WetBc::TotalInlet(total_inlet())).unwrap();
    s.set_bc("xmax", WetBc::Extrapolate).unwrap();
    s.set_bc("ymin", WetBc::Slip).unwrap();
    s.set_bc("ymax", WetBc::Slip).unwrap();
    s
}

// ---- P1, P2 --------------------------------------------------------------------------------------------------------------

/// P1: IAPWS R1-76(2014) Table 1 column 4 within 0.006 mN/m.
#[test]
fn p1_surface_tension_matches_the_iapws_table() {
    for (tc, sig) in [
        (0.01, 75.65),
        (25.0, 71.97),
        (50.0, 67.94),
        (75.0, 63.58),
        (100.0, 58.91),
    ] {
        let v = surface_tension(273.15 + tc) * 1e3;
        println!("P1 t = {tc} C: sigma {v:.5} mN/m, table {sig}, diff {:+.5}", v - sig);
        assert!((v - sig).abs() <= 0.006, "P1 at {tc} C: {v} vs {sig}");
    }
}

/// P2: the saturation temperature inverts the (extended) IF97 Eq. 30; the extension equals the IF97 function above 273.15 K.
#[test]
fn p2_saturation_temperature_round_trip() {
    for p in [1.0e3, 3.0e3, 10.0e3, 30.0e3, 100.0e3] {
        let t = iapws::saturation_temperature(p).unwrap();
        let back = iapws::saturation_pressure_extended(t).unwrap();
        println!("P2 p = {p}: T_s {t:.10} K, p_s(T_s)/p - 1 = {:.2e}", back / p - 1.0);
        assert!((back / p - 1.0).abs() <= 1e-12, "P2 p {p}");
    }
    for t in [250.0, 280.0, 300.0, 350.0, 450.0] {
        let p = iapws::saturation_pressure_extended(t).unwrap();
        let back = iapws::saturation_temperature(p).unwrap();
        println!("P2 T = {t}: T_s(p_s(T)) - T = {:.2e} K", back - t);
        assert!((back - t).abs() <= 1e-9, "P2 T {t}");
    }
    for t in [273.15, 300.0, 500.0] {
        let a = iapws::saturation_pressure(t).unwrap();
        let b = iapws::saturation_pressure_extended(t).unwrap();
        assert_eq!(a.to_bits(), b.to_bits(), "P2 bit identity at {t}");
    }
}

// ---- M1 ------------------------------------------------------------------------------------------------------------------

/// M1: the model's values against the prototype (an independent implementation written before the library) at three
/// states, relative 1e-6 (ADR 0217 table).
#[test]
fn m1_model_values_match_the_prototype() {
    let m = model203();
    #[allow(clippy::type_complexity)]
    let rows: [((f64, f64, f64, f64), [f64; 10]); 3] = [
        (
            (285.0, 12000.0, 0.0, 0.0),
            [
                1.3890056426e3,
                322.5697759943,
                7.3950695449e-2,
                9.9946874651e2,
                2.4707520092e6,
                2.3870115016e6,
                9.2066963264e1,
                5.2173394580e-10,
                2.0308632340e22,
                0.0,
            ],
        ),
        (
            (290.0, 10000.0, 0.02, 1.5e-8),
            [
                1.9199333106e3,
                318.9575482070,
                7.3210101179e-2,
                9.9875642965e2,
                2.4596073450e6,
                2.3950629147e6,
                8.8065863443e1,
                6.6372129998e-10,
                7.6313126716e16,
                5.7695442623e-4,
            ],
        ),
        (
            (300.0, 9000.0, 0.04, 2.0e-8),
            [
                3.5365894130e3,
                316.9118416104,
                7.1685962527e-2,
                9.9651426338e2,
                2.4373180166e6,
                2.3996226574e6,
                8.0708601334e1,
                1.1124681327e-9,
                3.4277088937e-8,
                2.8906688854e-4,
            ],
        ),
    ];
    let names = [
        "p_s", "T_s", "sigma", "rho_l", "L(T)", "L(T_s)", "phi", "r*", "J", "dr/dt",
    ];
    let mut worst: f64 = 0.0;
    for ((t, p, y, r), want) in rows {
        let rho = p / ((1.0 - y) * R_WATER * t);
        let ts = iapws::saturation_temperature(p).unwrap();
        let nuc = m.nucleation(t, p, y, rho).unwrap();
        let got = [
            iapws::saturation_pressure_extended(t).unwrap(),
            ts,
            surface_tension(t),
            liquid_density(t).unwrap(),
            m.latent(t),
            m.latent(ts),
            m.kantrowitz_phi(t),
            nuc.r_crit,
            nuc.rate,
            if r > 0.0 {
                m.growth_rate(t, p, y, rho, r).unwrap()
            } else {
                0.0
            },
        ];
        for k in 0..10 {
            let rel = if want[k] == 0.0 {
                got[k].abs()
            } else {
                (got[k] / want[k] - 1.0).abs()
            };
            worst = worst.max(rel);
            println!(
                "M1 T {t} p {p} y {y}: {:6} got {:.10e} want {:.10e} rel {rel:.2e}",
                names[k], got[k], want[k]
            );
            assert!(rel <= 1e-6, "M1 {} at T {t}: {} vs {}", names[k], got[k], want[k]);
        }
    }
    println!("M1 worst relative difference {worst:.2e}");
}

// ---- L1, E1: isochoric relaxation ------------------------------------------------------------------------------------------

const BOX_T: f64 = 285.0;
const BOX_P: f64 = 12.0e3;
const BOX_Y: f64 = 1e-4;
const BOX_N: f64 = 1e16;

fn box_solver(point_implicit: bool) -> WetSteamSolver {
    let model = model203();
    let geom = FvGeometry::new(Arc::new(box_2d(2, 2, 2e-3, 2e-3, false, false).unwrap())).unwrap();
    let settings = WetSettings {
        point_implicit,
        ..WetSettings::default()
    };
    let rho = model.density(BOX_P, BOX_T, BOX_Y);
    let mut s = WetSteamSolver::new(geom, model, settings, |_| WetPrim {
        rho,
        u: [0.0; 3],
        p: BOX_P,
        y: BOX_Y,
        n: BOX_N,
    })
    .unwrap();
    for p in ["xmin", "xmax", "ymin", "ymax"] {
        s.set_bc(p, WetBc::Slip).unwrap();
    }
    s
}

/// The isochoric ODE of the model (`rho`, `e` fixed), integrated by classic RK4: `(y, n, T)` at the requested times.
fn isochoric_reference(times: &[f64], dt: f64) -> Vec<(f64, f64, f64)> {
    let m = model203();
    let rho = m.density(BOX_P, BOX_T, BOX_Y);
    let e = m.internal_energy(BOX_T, BOX_Y);
    let f = |y: f64, n: f64| {
        let r = m.rates(rho, e, y, n).unwrap();
        (r.dy_dt, r.dn_dt)
    };
    let (mut y, mut n) = (BOX_Y, BOX_N);
    let mut out = vec![];
    let mut k = 0usize;
    for &target in times {
        let steps = (target / dt).round() as usize;
        while k < steps {
            let k1 = f(y, n);
            let k2 = f(y + 0.5 * dt * k1.0, n + 0.5 * dt * k1.1);
            let k3 = f(y + 0.5 * dt * k2.0, n + 0.5 * dt * k2.1);
            let k4 = f(y + dt * k3.0, n + dt * k3.1);
            y += dt / 6.0 * (k1.0 + 2.0 * k2.0 + 2.0 * k3.0 + k4.0);
            n += dt / 6.0 * (k1.1 + 2.0 * k2.1 + 2.0 * k3.1 + k4.1);
            k += 1;
        }
        out.push((y, n, m.temperature(e, y)));
    }
    out
}

fn box_state(s: &WetSteamSolver) -> (f64, f64, f64, f64) {
    let w = s.primitives().unwrap();
    let mut spread: f64 = 0.0;
    for k in [4usize, 5, 6] {
        let v: Vec<f64> = s.u.iter().map(|c| c[k]).collect();
        let (lo, hi) = v
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |a, &x| (a.0.min(x), a.1.max(x)));
        let scale = hi.abs().max(lo.abs()).max(1e-300);
        spread = spread.max((hi - lo) / scale);
    }
    (w[0].y, w[0].n, w[0].temperature(), spread)
}

fn l1_box_check() {
    let times = [1e-5, 2e-5, 4e-5, 1e-4];
    let reference = isochoric_reference(&times, 1e-10);
    let mut s = box_solver(true);
    let dt = 2e-8;
    let mut k = 0usize;
    for (i, &target) in times.iter().enumerate() {
        let steps = (target / dt).round() as usize;
        while k < steps {
            s.step(dt).unwrap();
            k += 1;
        }
        let (y, n, t, spread) = box_state(&s);
        let (yr, nr, tr) = reference[i];
        println!(
            "L1 t {target:.1e}: y {y:.8e} (ref {yr:.8e}, rel {:.2e}), n {n:.6e} (ref {nr:.6e}, rel {:.2e}), T {t:.5} (ref {tr:.5}, \
             diff {:.2e}), cell spread {spread:.1e}",
            (y / yr - 1.0).abs(),
            (n / nr - 1.0).abs(),
            t - tr
        );
        assert!(spread <= 1e-12, "L1 spread {spread}");
        assert!((y / yr - 1.0).abs() <= 1e-3, "L1 y at {target}");
        assert!((n / nr - 1.0).abs() <= 1e-3, "L1 n at {target}");
        assert!((t - tr).abs() <= 0.01, "L1 T at {target}");
    }
    assert_eq!(s.clips, 0);
}

/// L1: the solver's box against the independent RK4 integration (point-implicit on, dt 2e-8 s). KNOWN MISS (ADR 0217
/// amendment 1): with the registered `T_s` tolerance 1e-13 the cell spread at 4e-5 s was 8.91e-12 against 1e-12 (y, n, T
/// at 4e-5 s inside their limits; 1e-4 s not evaluated). Kept as the record; the code now runs the amended M3, so this test
/// is the same as L1b.
#[test]
#[ignore = "known miss (ADR 0217 amendment 1): cell spread 8.91e-12 > 1e-12 at 4e-5 s with the T_s tolerance 1e-13"]
fn l1_isochoric_relaxation_matches_an_independent_ode_known_miss() {
    l1_box_check();
}

/// L1b (ADR 0217 amendment 1): the L1 set-up and limits with `T_s` iterated to the last bits.
#[test]
fn l1b_isochoric_relaxation_matches_an_independent_ode() {
    l1_box_check();
}

/// The growth equilibrium `ln S = 2 sigma/(rho_l R T r)` at fixed `rho`, `e`, `n`, solved for `y` by bisection.
fn equilibrium_wetness(m: &WetSteamModel, rho: f64, e: f64, n: f64) -> f64 {
    let f = |y: f64| {
        let t = m.temperature(e, y);
        let p = (1.0 - y) * rho * R_WATER * t;
        let ps = iapws::saturation_pressure_extended(t).unwrap();
        let rl = liquid_density(t).unwrap();
        let r = (3.0 * y / (4.0 * PI * rl * n)).cbrt();
        (p / ps).ln() - 2.0 * surface_tension(t) / (rl * R_WATER * t * r)
    };
    let (mut lo, mut hi) = (1e-6, 0.2);
    assert!(
        f(lo) > 0.0 && f(hi) < 0.0,
        "E1 bracket: f(lo) {} f(hi) {}",
        f(lo),
        f(hi)
    );
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if f(mid) > 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// E1: the relaxed box ends at the model's growth equilibrium (dt 1e-7 s, 20000 steps).
#[test]
fn e1_relaxation_ends_at_the_growth_equilibrium() {
    let mut s = box_solver(true);
    for _ in 0..20000 {
        s.step(1e-7).unwrap();
    }
    let w = s.primitives().unwrap();
    let m = s.model;
    let c = &s.u[0];
    let rho = c[0];
    let e = c[4] / rho - 0.5 * (c[1] * c[1] + c[2] * c[2] + c[3] * c[3]) / (rho * rho);
    let y_eq = equilibrium_wetness(&m, rho, e, w[0].n);
    let rel = (w[0].y / y_eq - 1.0).abs();
    let ps = iapws::saturation_pressure_extended(w[0].temperature()).unwrap();
    println!(
        "E1 t {:.1e}: y {:.10e}, equilibrium {y_eq:.10e}, rel {rel:.2e}; n {:.4e}, T {:.4} K, S {:.5}, clips {}",
        s.time,
        w[0].y,
        w[0].n,
        w[0].temperature(),
        w[0].p / ps,
        s.clips
    );
    assert!(rel <= 1e-4, "E1 {rel}");
}

// ---- C1, C1b: conservation ------------------------------------------------------------------------------------------------

fn conservation_run(point_implicit: bool) -> (f64, f64, f64, f64, usize) {
    let model = model203();
    let (lx, ly) = (1e-3, 0.5e-3);
    let geom = FvGeometry::new(Arc::new(box_2d(12, 6, lx, ly, false, false).unwrap())).unwrap();
    let settings = WetSettings {
        point_implicit,
        ..WetSettings::default()
    };
    let mut s = WetSteamSolver::new(geom, model, settings, |x| {
        let t = 290.0 + 5.0 * (2.0 * PI * x[0] / lx).sin();
        let p = 11.0e3 * (1.0 + 0.05 * (2.0 * PI * x[1] / ly).cos());
        let y = 0.01;
        WetPrim {
            rho: model.density(p, t, y),
            u: [50.0 * (PI * x[1] / ly).sin(), 20.0 * (PI * x[0] / lx).sin(), 0.0],
            p,
            y,
            n: 1e17,
        }
    })
    .unwrap();
    for p in ["xmin", "xmax", "ymin", "ymax"] {
        s.set_bc(p, WetBc::Slip).unwrap();
    }
    let t0 = s.totals();
    let mut integral = [0.0; 2];
    for _ in 0..300 {
        let dt = s.max_dt().unwrap();
        let si = s.step(dt).unwrap();
        integral[0] += si[0];
        integral[1] += si[1];
    }
    let t1 = s.totals();
    let mass = (t1[0] / t0[0] - 1.0).abs();
    let energy = (t1[4] / t0[4] - 1.0).abs();
    let liq = (t1[5] - t0[5] - integral[0]).abs() / t1[5];
    let num = (t1[6] - t0[6] - integral[1]).abs() / t1[6];
    println!(
        "C1 (point-implicit {point_implicit}): t {:.3e} s, mass drift {mass:.2e}, energy drift {energy:.2e}, liquid {:.6e} -> \
         {:.6e} (source integral {:.6e}, bookkeeping {liq:.2e}), droplets bookkeeping {num:.2e}, clips {}",
        s.time, t0[5], t1[5], integral[0], s.clips
    );
    (mass, energy, liq, num, s.clips)
}

/// C1: a closed box conserves mixture mass and energy, and the liquid changes by the source integral only (point-implicit
/// off).
#[test]
fn c1_closed_box_conserves_mass_energy_and_the_liquid_bookkeeping() {
    let (mass, energy, liq, num, clips) = conservation_run(false);
    assert!(mass <= 1e-12, "C1 mass {mass}");
    assert!(energy <= 1e-12, "C1 energy {energy}");
    assert!(liq <= 1e-12, "C1 liquid bookkeeping {liq}");
    assert!(num <= 1e-12, "C1 droplet bookkeeping {num}");
    assert_eq!(clips, 0, "C1 clips");
}

/// C1b: with the point-implicit factor the mixture is still conserved.
#[test]
fn c1b_point_implicit_keeps_the_mixture_conserved() {
    let (mass, energy, _, _, _) = conservation_run(true);
    assert!(mass <= 1e-12, "C1b mass {mass}");
    assert!(energy <= 1e-12, "C1b energy {energy}");
}

// ---- S1, D1: shock tubes ---------------------------------------------------------------------------------------------------

const TUBE_N: usize = 200;

fn tube_states(model: &WetSteamModel, y: f64) -> (WetPrim, WetPrim) {
    let (tl, pl) = (300.0, 20.0e3);
    let rho_l = model.density(pl, tl, y);
    let l = WetPrim {
        rho: rho_l,
        u: [0.0; 3],
        p: pl,
        y,
        n: 0.0,
    };
    let r = WetPrim {
        rho: rho_l / 8.0,
        p: 2.0e3,
        ..l
    };
    (l, r)
}

fn tube_mesh() -> FvGeometry {
    FvGeometry::new(Arc::new(
        box_2d(TUBE_N, 1, 1.0, 1.0 / TUBE_N as f64, false, false).unwrap(),
    ))
    .unwrap()
}

fn wet_tube(y: f64) -> WetSteamSolver {
    let model = model203();
    let (l, r) = tube_states(&model, y);
    let settings = WetSettings {
        condensation: false,
        ..WetSettings::default()
    };
    let mut s = WetSteamSolver::new(tube_mesh(), model, settings, |x| if x[0] < 0.5 { l } else { r }).unwrap();
    s.set_bc("xmin", WetBc::Fixed(l)).unwrap();
    s.set_bc("xmax", WetBc::Fixed(r)).unwrap();
    s.set_bc("ymin", WetBc::Slip).unwrap();
    s.set_bc("ymax", WetBc::Slip).unwrap();
    s
}

fn tube_end_time(model: &WetSteamModel, y: f64) -> f64 {
    let (l, _) = tube_states(model, y);
    0.237 / (model.gamma_mix(y) * l.p / l.rho).sqrt()
}

/// S1: the frozen wet shock tube against the exact Riemann solution with `gamma = Gamma(0.05)`.
#[test]
fn s1_frozen_wet_shock_tube_matches_the_exact_solution() {
    let y = 0.05;
    let mut s = wet_tube(y);
    let model = s.model;
    let t_end = tube_end_time(&model, y);
    while s.time < t_end - 1e-14 * t_end {
        let dt = s.max_dt().unwrap().min(t_end - s.time);
        s.step(dt).unwrap();
    }
    let (l, r) = tube_states(&model, y);
    let g = Gas {
        gamma: model.gamma_mix(y),
    };
    let ex = ExactRiemann::new(g, [l.rho, 0.0, l.p], [r.rho, 0.0, r.p]).unwrap();
    let w = s.primitives().unwrap();
    let cc = s.geom.mesh.cell_centres();
    let vol = s.geom.mesh.cell_volumes();
    let (mut num, mut den, mut ydev) = (0.0, 0.0, 0.0f64);
    for c in 0..w.len() {
        let e = ex.sample((cc[c][0] - 0.5) / t_end);
        num += (w[c].rho - e[0]).abs() * vol[c];
        den += vol[c];
        ydev = ydev.max((w[c].y - y).abs());
    }
    let l1 = num / den / l.rho;
    println!(
        "S1 Gamma {:.6}, t {t_end:.4e} s, {} steps: density L1/rho_L {l1:.4e}, max |y - 0.05| {ydev:.2e}",
        g.gamma, s.steps
    );
    assert!(l1 <= 4.0e-3, "S1 L1 {l1}");
    assert!(ydev <= 1e-13, "S1 y {ydev}");
}

fn compare_flow(w: &[WetPrim], p: &[Prim]) -> f64 {
    let umax = w.iter().map(|q| q.u[0].hypot(q.u[1])).fold(1e-300, f64::max);
    let mut d: f64 = 0.0;
    for (a, b) in w.iter().zip(p) {
        d = d.max((a.rho / b.rho - 1.0).abs());
        d = d.max((a.p / b.p - 1.0).abs());
        for k in 0..3 {
            d = d.max((a.u[k] - b.u[k]).abs() / umax);
        }
    }
    d
}

fn comp_settings() -> CompSettings {
    CompSettings {
        solver: RiemannSolver::Hllc,
        limiter: Some(TvdLimiter::VanLeer),
        rk: RkScheme::Rk3,
        cfl: 0.4,
    }
}

/// D1 (a): the dry shock tube against `CompressibleSolver` with `gamma_v`.
#[test]
fn d1a_dry_limit_shock_tube_matches_the_compressible_solver() {
    let mut s = wet_tube(0.0);
    let model = s.model;
    let (l, r) = tube_states(&model, 0.0);
    let mut c = CompressibleSolver::new(tube_mesh(), model.gas(), comp_settings(), |x| {
        if x[0] < 0.5 {
            l.flow()
        } else {
            r.flow()
        }
    })
    .unwrap();
    c.set_bc("xmin", CompBc::Fixed(l.flow())).unwrap();
    c.set_bc("xmax", CompBc::Fixed(r.flow())).unwrap();
    c.set_bc("ymin", CompBc::Slip).unwrap();
    c.set_bc("ymax", CompBc::Slip).unwrap();
    let t_end = tube_end_time(&model, 0.0);
    let mut dtdiff: f64 = 0.0;
    while s.time < t_end - 1e-14 * t_end {
        let dt = s.max_dt().unwrap().min(t_end - s.time);
        dtdiff = dtdiff.max((c.max_dt() / s.max_dt().unwrap() - 1.0).abs());
        s.step(dt).unwrap();
        c.step(dt).unwrap();
    }
    let d = compare_flow(&s.primitives().unwrap(), &c.primitives());
    println!(
        "D1a {} steps: largest relative difference {d:.3e}; max_dt relative difference {dtdiff:.1e}",
        s.steps
    );
    assert!(d <= 1e-12, "D1a {d}");
}

/// D1 (b): the dry coarse nozzle (60 x 6, 200 steps) against `CompressibleSolver` with `gamma_v`.
#[test]
fn d1b_dry_limit_nozzle_matches_the_compressible_solver() {
    let mut s = nozzle_solver(60, 6, false);
    let model = s.model;
    let mut c = CompressibleSolver::new(nozzle_mesh(60, 6), model.gas(), comp_settings(), |x| {
        let (rho, u, p) = isentropic(&model, x[0]);
        Prim {
            rho,
            u: [u, 0.0, 0.0],
            p,
        }
    })
    .unwrap();
    c.set_bc("xmin", CompBc::TotalInlet(total_inlet())).unwrap();
    c.set_bc("xmax", CompBc::Extrapolate).unwrap();
    c.set_bc("ymin", CompBc::Slip).unwrap();
    c.set_bc("ymax", CompBc::Slip).unwrap();
    for _ in 0..200 {
        let dt = s.max_dt().unwrap();
        s.step(dt).unwrap();
        c.step(dt).unwrap();
    }
    let d = compare_flow(&s.primitives().unwrap(), &c.primitives());
    println!("D1b 200 steps, t {:.3e} s: largest relative difference {d:.3e}", s.time);
    assert!(d <= 1e-12, "D1b {d}");
}

// ---- heavy: the nozzle runs ------------------------------------------------------------------------------------------------

/// A finished nozzle run.
struct NozzleRun {
    nx: usize,
    steps: usize,
    change: f64,
    time: f64,
    clips: usize,
    seconds: f64,
    /// Per column: cell indices sorted by height.
    columns: Vec<Vec<usize>>,
    prim: Vec<rcfd_fv::wetsteam::WetPrim>,
    radii: Vec<f64>,
    vol: Vec<f64>,
    depth: f64,
}

impl NozzleRun {
    fn dx(&self) -> f64 {
        LX / self.nx as f64
    }
    fn x_col(&self, i: usize) -> f64 {
        X_IN + (i as f64 + 0.5) * self.dx()
    }
    /// Linear interpolation in x of a per-column value.
    fn at(&self, x: f64, f: &dyn Fn(&[usize]) -> f64) -> f64 {
        let s = ((x - X_IN) / self.dx() - 0.5).floor().clamp(0.0, (self.nx - 2) as f64) as usize;
        let t = (x - self.x_col(s)) / self.dx();
        let (a, b) = (f(&self.columns[s]), f(&self.columns[s + 1]));
        a + t * (b - a)
    }
    fn centre_p(&self, x: f64) -> f64 {
        self.at(x, &|c| self.prim[c[0]].p) / P0
    }
    fn centre_r(&self, x: f64) -> f64 {
        self.at(x, &|c| self.radii[c[0]])
    }
    fn mean_p(&self, x: f64) -> f64 {
        self.at(x, &|c| {
            let (mut a, mut v) = (0.0, 0.0);
            for &k in c {
                a += self.prim[k].p * self.vol[k];
                v += self.vol[k];
            }
            a / v
        }) / P0
    }
    /// Mass flow per unit depth through a column, and the mass-flow-weighted `y` and `n` there.
    fn column_flow(&self, i: usize) -> (f64, f64, f64) {
        let (mut m, mut my, mut mn) = (0.0, 0.0, 0.0);
        for &k in &self.columns[i] {
            let w = &self.prim[k];
            let q = w.rho * w.u[0] * self.vol[k] / (self.dx() * self.depth);
            m += q;
            my += q * w.y;
            mn += q * w.n;
        }
        (m, my / m, mn / m)
    }
    fn t_min(&self) -> f64 {
        self.prim.iter().map(|w| w.temperature()).fold(f64::INFINITY, f64::min)
    }
}

fn run_nozzle(nx: usize, ny: usize, condensation: bool, cap: usize) -> NozzleRun {
    let mut s = nozzle_solver(nx, ny, condensation);
    let start = std::time::Instant::now();
    let mut change = f64::INFINITY;
    let mut steps = cap;
    for i in 0..cap {
        let before: Vec<f64> = s.u.iter().map(|c| c[0]).collect();
        let dt = s.max_dt().unwrap();
        s.step(dt).unwrap();
        change =
            s.u.iter()
                .zip(&before)
                .map(|(c, b)| (c[0] - b).abs() / b)
                .fold(0.0, f64::max);
        if i % 2000 == 0 {
            eprintln!(
                "nozzle {nx}x{ny} condensation {condensation}: step {i}, t {:.4e}, dt {dt:.3e}, change {change:.3e}, {:.0} s",
                s.time,
                start.elapsed().as_secs_f64()
            );
        }
        if change < 1e-8 {
            steps = i + 1;
            break;
        }
    }
    let prim = s.primitives().unwrap();
    let radii = s.droplet_radii().unwrap();
    let cc = s.geom.mesh.cell_centres();
    let dx = LX / nx as f64;
    let mut columns = vec![vec![]; nx];
    for (c, x) in cc.iter().enumerate() {
        let i = (((x[0] - X_IN) / dx).floor() as usize).min(nx - 1);
        columns[i].push(c);
    }
    for col in columns.iter_mut() {
        col.sort_by(|a, b| cc[*a][1].partial_cmp(&cc[*b][1]).unwrap());
        assert_eq!(col.len(), ny);
    }
    NozzleRun {
        nx,
        steps,
        change,
        time: s.time,
        clips: s.clips,
        seconds: start.elapsed().as_secs_f64(),
        columns,
        prim,
        radii,
        vol: s.geom.mesh.cell_volumes().to_vec(),
        depth: dx,
    }
}

static FINE_WET: OnceLock<NozzleRun> = OnceLock::new();
static FINE_DRY: OnceLock<NozzleRun> = OnceLock::new();
static COARSE_WET: OnceLock<NozzleRun> = OnceLock::new();

fn fine_wet() -> &'static NozzleRun {
    FINE_WET.get_or_init(|| run_nozzle(460, 20, true, 100_000))
}
fn fine_dry() -> &'static NozzleRun {
    FINE_DRY.get_or_init(|| run_nozzle(460, 20, false, 100_000))
}
fn coarse_wet() -> &'static NozzleRun {
    COARSE_WET.get_or_init(|| run_nozzle(230, 10, true, 50_000))
}

/// The G5 onset vertex of a 41-value sequence at the data stations (m), or `None`.
fn onset_vertex(v: &[f64]) -> Option<f64> {
    let k = (1..v.len() - 1).find(|&i| v[i] <= v[i - 1] && v[i] <= v[i + 1])?;
    let x: Vec<f64> = DATA_P.iter().map(|d| d.0 * 1e-2).collect();
    let (x1, x2, x3) = (x[k - 1], x[k], x[k + 1]);
    let (y1, y2, y3) = (v[k - 1], v[k], v[k + 1]);
    let d = (x1 - x2) * (x1 - x3) * (x2 - x3);
    let a = (x3 * (y2 - y1) + x2 * (y1 - y3) + x1 * (y3 - y2)) / d;
    let b = (x3 * x3 * (y1 - y2) + x2 * x2 * (y3 - y1) + x1 * x1 * (y2 - y3)) / d;
    Some(-b / (2.0 * a))
}

fn stations() -> Vec<f64> {
    DATA_P.iter().map(|d| d.0 * 1e-2).collect()
}

/// R-Q1D: the steady quasi-1-D condensing nozzle of model M from the dry isentropic state at x = 2 mm, RK4 in x with
/// dx = 2e-7 m. Rows `(x, p/p0, y, n, r)`.
fn quasi_1d() -> Vec<[f64; 5]> {
    let m = model203();
    let g = m.gamma_v();
    let dc = m.cp_v - m.cp_l;
    let rho0 = P0 / (R_WATER * T0);
    let a0 = (g * R_WATER * T0).sqrt();
    let mdot = rho0 * (2.0 / (g + 1.0)).powf(1.0 / (g - 1.0)) * a0 * (2.0 / (g + 1.0)).sqrt() * H;
    let h0 = m.cp_v * T0;
    let rhs = |x: f64, s: &[f64; 3]| -> ([f64; 3], [f64; 5]) {
        let (u, y, n) = (s[0], s[1].max(0.0), s[2].max(0.0));
        let cpm = m.cp_v - y * dc;
        let t = (h0 - 0.5 * u * u + y * (m.latent_ref - dc * m.t_ref)) / cpm;
        let a = wall_h(x);
        let rho = mdot / (u * a);
        let p = (1.0 - y) * rho * R_WATER * t;
        let nuc = m.nucleation(t, p, y, rho).unwrap();
        let rl = liquid_density(t).unwrap();
        let mut ydot = 0.0;
        if nuc.rate > 0.0 {
            ydot += nuc.rate * 4.0 / 3.0 * PI * nuc.r_crit.powi(3) * rl;
        }
        let mut r = 0.0;
        if n > 0.0 && y > 0.0 {
            r = (3.0 * y / (4.0 * PI * rl * n)).cbrt();
            ydot += n * 4.0 * PI * r * r * rl * m.growth_rate(t, p, y, rho, r).unwrap();
        }
        let cf2 = (1.0 - y) * R_WATER * t * cpm / (cpm - (1.0 - y) * R_WATER);
        let dydx = ydot / u;
        let dudx = u * (-wall_dhdx(x) / a + dydx * (m.latent(t) / (cpm * t) - 1.0 / (1.0 - y))) / (1.0 - u * u / cf2);
        ([dudx, dydx, nuc.rate / u], [x, p / P0, y, n, r])
    };
    let xs = 2e-3;
    let mach = mach_of(g, wall_h(xs) / H, true);
    let t = T0 / (1.0 + 0.5 * (g - 1.0) * mach * mach);
    let mut st = [mach * (g * R_WATER * t).sqrt(), 0.0, 0.0];
    let dx = 2e-7;
    let nsteps = ((0.065 - xs) / dx).round() as usize;
    let mut out = Vec::with_capacity(nsteps / 10 + 2);
    for k in 0..nsteps {
        let x = xs + k as f64 * dx;
        let (k1, row) = rhs(x, &st);
        if k % 10 == 0 {
            out.push(row);
        }
        let s2 = std::array::from_fn(|i| st[i] + 0.5 * dx * k1[i]);
        let (k2, _) = rhs(x + 0.5 * dx, &s2);
        let s3 = std::array::from_fn(|i| st[i] + 0.5 * dx * k2[i]);
        let (k3, _) = rhs(x + 0.5 * dx, &s3);
        let s4 = std::array::from_fn(|i| st[i] + dx * k3[i]);
        let (k4, _) = rhs(x + dx, &s4);
        for i in 0..3 {
            st[i] += dx / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]);
        }
    }
    out.push(rhs(0.065, &st).1);
    out
}

fn q1d_at(rows: &[[f64; 5]], x: f64, k: usize) -> f64 {
    let i = rows.partition_point(|r| r[0] < x).clamp(1, rows.len() - 1);
    let (a, b) = (rows[i - 1], rows[i]);
    a[k] + (x - a[0]) / (b[0] - a[0]) * (b[k] - a[k])
}

fn report(name: &str, r: &NozzleRun) {
    println!(
        "{name}: {} x {} cells, {} steps, t {:.4e} s, last change {:.3e}, clips {}, T_min {:.2} K, {:.0} s",
        r.nx,
        r.columns[0].len(),
        r.steps,
        r.time,
        r.change,
        r.clips,
        r.t_min(),
        r.seconds
    );
}

/// S1n and S2: steadiness of the three runs and the final minimum temperature of the wet runs.
#[test]
#[ignore = "heavy: three nozzle runs (ADR 0217 S1n, S2), release, about an hour"]
fn s1n_s2_nozzle_runs_reach_a_steady_state_above_the_triple_point() {
    let (fw, fd, cw) = (fine_wet(), fine_dry(), coarse_wet());
    report("fine wet", fw);
    report("fine dry", fd);
    report("coarse wet", cw);
    assert!(fw.change < 1e-8 && fd.change < 1e-8 && cw.change < 1e-8, "S1n");
    assert!(fw.t_min() >= 273.15 && cw.t_min() >= 273.15, "S2");
}

/// N1: the dry fine nozzle against the isentropic quasi-1-D relation and the choked mass flow.
#[test]
#[ignore = "heavy: fine dry nozzle run (ADR 0217 N1), release"]
fn n1_dry_nozzle_matches_the_isentropic_relation() {
    let r = fine_dry();
    report("fine dry", r);
    let model = model203();
    let mut worst: f64 = 0.0;
    for x in stations() {
        let (_, _, p) = isentropic(&model, x);
        let d = r.mean_p(x) / (p / P0) - 1.0;
        worst = worst.max(d.abs());
        println!(
            "N1 x {:.4} cm: column mean p/p0 {:.5}, isentropic {:.5}, {:+.3} %",
            x * 100.0,
            r.mean_p(x),
            p / P0,
            100.0 * d
        );
    }
    let g = model.gamma_v();
    let rho0 = P0 / (R_WATER * T0);
    let choked =
        rho0 * (2.0 / (g + 1.0)).powf(1.0 / (g - 1.0)) * (g * R_WATER * T0).sqrt() * (2.0 / (g + 1.0)).sqrt() * H;
    let (m_in, _, _) = r.column_flow(0);
    let dm = m_in / choked - 1.0;
    println!(
        "N1 worst {:.3} %; inlet mass flow {m_in:.6e} kg/(m s), choked {choked:.6e}, {:+.4} %",
        100.0 * worst,
        100.0 * dm
    );
    assert!(worst <= 0.015, "N1 pressure {worst}");
    assert!(dm.abs() <= 0.005, "N1 mass flow {dm}");
}

/// Q1: the wet fine nozzle against the quasi-1-D reference R-Q1D.
#[test]
#[ignore = "heavy: fine wet nozzle run and the quasi-1-D reference (ADR 0217 Q1), release"]
fn q1_wet_nozzle_matches_the_quasi_1d_reference() {
    let r = fine_wet();
    report("fine wet", r);
    let q = quasi_1d();
    let mut worst: f64 = 0.0;
    let (mut vs, mut vq) = (vec![], vec![]);
    for x in stations() {
        let (ps, pq) = (r.mean_p(x), q1d_at(&q, x, 1));
        vs.push(ps);
        vq.push(pq);
        let d = ps / pq - 1.0;
        worst = worst.max(d.abs());
        println!(
            "Q1 x {:.4} cm: column mean p/p0 {ps:.5}, quasi-1-D {pq:.5}, {:+.3} %",
            x * 100.0,
            100.0 * d
        );
    }
    let (os, oq) = (onset_vertex(&vs), onset_vertex(&vq));
    let (_, ys, ns) = r.column_flow(r.nx - 1);
    let last = q.last().unwrap();
    let (yq, nq) = (last[2], last[3]);
    println!(
        "Q1 worst {:.3} %; onset vertex {:?} mm vs quasi-1-D {:?} mm; exit y {ys:.5} vs {yq:.5} ({:+.2} %), n {ns:.4e} vs \
         {nq:.4e} (ratio {:.3})",
        100.0 * worst,
        os.map(|v| v * 1e3),
        oq.map(|v| v * 1e3),
        100.0 * (ys / yq - 1.0),
        ns / nq
    );
    assert!(worst <= 0.02, "Q1 pressure {worst}");
    let (os, oq) = (os.expect("Q1 onset (rcfd)"), oq.expect("Q1 onset (quasi-1-D)"));
    assert!((os - oq).abs() <= 1.5e-3, "Q1 onset {os} vs {oq}");
    assert!((ys / yq - 1.0).abs() <= 0.05, "Q1 exit y");
    assert!(ns / nq <= 2.0 && nq / ns <= 2.0, "Q1 exit n");
}

fn centre_series(r: &NozzleRun) -> Vec<f64> {
    stations().iter().map(|&x| r.centre_p(x)).collect()
}

/// V1 (a): the centreline pressure within 5 percent of the 41 data points.
#[test]
#[ignore = "heavy: fine wet nozzle run (ADR 0217 V1a), release"]
fn v1a_centreline_pressure_within_five_percent_of_the_data() {
    let r = fine_wet();
    report("fine wet", r);
    let v = centre_series(r);
    let mut worst: f64 = 0.0;
    let mut misses = 0;
    for (k, d) in DATA_P.iter().enumerate() {
        let dev = v[k] / d.1 - 1.0;
        worst = worst.max(dev.abs());
        if dev.abs() > 0.05 {
            misses += 1;
        }
        println!(
            "V1a x {:.4} cm: data {:.5}, rcfd {:.5}, {:+.3} %",
            d.0,
            d.1,
            v[k],
            100.0 * dev
        );
    }
    println!("V1a worst {:.3} %, {misses} of 41 points beyond 5 %", 100.0 * worst);
    assert!(worst <= 0.05, "V1a {worst}");
}

/// V1 (b): the onset vertex within 5 percent of the data's 34.906 mm.
#[test]
#[ignore = "heavy: fine wet nozzle run (ADR 0217 V1b), release"]
fn v1b_onset_location_within_five_percent_of_the_data() {
    let r = fine_wet();
    let data: Vec<f64> = DATA_P.iter().map(|d| d.1).collect();
    let xd = onset_vertex(&data).unwrap();
    let xs = onset_vertex(&centre_series(r));
    println!(
        "V1b onset vertex: data {:.3} mm, rcfd {:?} mm",
        xd * 1e3,
        xs.map(|v| v * 1e3)
    );
    let xs = xs.expect("V1b: no onset in the rcfd centreline");
    println!("V1b relative {:+.3} %", 100.0 * (xs / xd - 1.0));
    assert!((xs / xd - 1.0).abs() <= 0.05, "V1b {xs} vs {xd}");
}

/// V2: the centreline droplet radius within 25 percent of r32 at the 12 stations x >= 4.1599 cm.
#[test]
#[ignore = "heavy: fine wet nozzle run (ADR 0217 V2), release"]
fn v2_droplet_radius_within_25_percent_of_r32() {
    let r = fine_wet();
    let mut worst: f64 = 0.0;
    for &(x, r32) in DATA_R32.iter().filter(|d| d.0 >= 4.1599 - 1e-9) {
        let rs = r.centre_r(x * 1e-2) * 1e9;
        let d = rs / r32 - 1.0;
        worst = worst.max(d.abs());
        println!(
            "V2 x {x:.4} cm: r32 {r32:.3} nm, rcfd r {rs:.3} nm, {:+.2} %",
            100.0 * d
        );
    }
    println!("V2 worst {:.2} %", 100.0 * worst);
    assert!(worst <= 0.25, "V2 {worst}");
}

/// G1g: the coarse grid against the fine grid.
#[test]
#[ignore = "heavy: fine and coarse wet nozzle runs (ADR 0217 G1g), release"]
fn g1g_coarse_grid_agrees_with_the_fine_grid() {
    let (f, c) = (fine_wet(), coarse_wet());
    report("coarse wet", c);
    let (vf, vc) = (centre_series(f), centre_series(c));
    let mut worst: f64 = 0.0;
    for k in 0..41 {
        worst = worst.max((vc[k] / vf[k] - 1.0).abs());
    }
    let (of, oc) = (onset_vertex(&vf), onset_vertex(&vc));
    println!(
        "G1g centreline worst {:.3} %; onset fine {:?} mm, coarse {:?} mm",
        100.0 * worst,
        of.map(|v| v * 1e3),
        oc.map(|v| v * 1e3)
    );
    assert!(worst <= 0.015, "G1g pressure {worst}");
    let (of, oc) = (of.expect("G1g onset fine"), oc.expect("G1g onset coarse"));
    assert!((of - oc).abs() <= 1.0e-3, "G1g onset");
}

/// NC1: the dry run's centreline misses the data by more than 5 percent (V1a sees condensation).
#[test]
#[ignore = "heavy: fine dry nozzle run (ADR 0217 NC1), release"]
fn nc1_dry_solution_misses_the_data() {
    let r = fine_dry();
    let v = centre_series(r);
    let worst = DATA_P
        .iter()
        .enumerate()
        .map(|(k, d)| (v[k] / d.1 - 1.0).abs())
        .fold(0.0, f64::max);
    println!(
        "NC1 dry centreline worst deviation from the data {:.2} %",
        100.0 * worst
    );
    assert!(worst > 0.05, "NC1 {worst}");
}

/// NC2: the wet column means differ from the dry isentropic relation by more than 2 percent somewhere (Q1 sees
/// condensation).
#[test]
#[ignore = "heavy: fine wet nozzle run (ADR 0217 NC2), release"]
fn nc2_wet_solution_differs_from_the_dry_relation() {
    let r = fine_wet();
    let model = model203();
    let worst = stations()
        .iter()
        .map(|&x| (r.mean_p(x) / (isentropic(&model, x).2 / P0) - 1.0).abs())
        .fold(0.0, f64::max);
    println!(
        "NC2 wet column means vs the dry isentropic relation: worst {:.2} %",
        100.0 * worst
    );
    assert!(worst > 0.02, "NC2 {worst}");
}
