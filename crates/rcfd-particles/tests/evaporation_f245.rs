//! F-245 (ADR 0224): droplet evaporation in the particle library. Registered checks A (EV1 to EV5, NE1, NE2), C (SP1 to
//! SP3) and RF1. The limits are those of ADR 0224, fixed before this code ran. Every reference value is computed here with
//! this file's own code (Clausius-Clapeyron, surface mass fraction, Spalding numbers, bisection), not with the library's.

use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_particles::*;
use std::f64::consts::PI;
use std::sync::Arc;

/// A change of a spec (the refusal lists).
type Mutator<T> = Box<dyn Fn(&mut T)>;

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}

/// `N_A k_B` (CODATA, exact).
const RU: f64 = 6.022_140_76e23 * 1.380_649e-23;

/// Data E of ADR 0224 (round test inputs of the order of n-heptane in hot air; not data).
const RHO_L: f64 = 700.0;
const C_L: f64 = 2200.0;
const LV: f64 = 3.2e5;
const TB: f64 = 371.5;
const PB: f64 = 101_325.0;
const P: f64 = 101_325.0;
const WF: f64 = 0.1;
const WB: f64 = 0.029;
const RHO: f64 = 0.5;
const MU: f64 = 3e-5;
const LAM: f64 = 0.05;
const CPM: f64 = 1100.0;
const CPV: f64 = 2500.0;
const DIFF: f64 = 3e-5;
const DMIN: f64 = 1e-7;
const D0: f64 = 5e-5;
const N0: f64 = 1000.0;

fn spec_e() -> EvaporationSpec {
    EvaporationSpec {
        latent_heat: LV,
        boiling_temperature: TB,
        boiling_pressure: PB,
        pressure: P,
        vapour_molar_mass: WF,
        gas_molar_mass: WB,
        diffusivity: DIFF,
        vapour_cp: CPV,
        min_diameter: DMIN,
        max_step_fraction: 0.1,
    }
}

// ---- this file's own model (ADR 0224 E2 to E4, DK)

fn x_of(t: f64) -> f64 {
    PB / P * (LV * WF / RU * (1.0 / TB - 1.0 / t)).exp()
}

fn y_i(t: f64) -> f64 {
    let x = x_of(t);
    x * WF / (x * WF + (1.0 - x) * WB)
}

fn b_m(t: f64, ys: f64) -> f64 {
    (y_i(t) - ys) / (1.0 - y_i(t))
}

fn nu_sh(re: f64) -> (f64, f64) {
    let pr = MU * CPM / LAM;
    let sc = MU / (RHO * DIFF);
    (2.0 + 0.6 * re.sqrt() * pr.cbrt(), 2.0 + 0.6 * re.sqrt() * sc.cbrt())
}

/// Wet bulb by bisection (200 halvings on `[250, T_b - 1e-9]`): with Bird's correction `c_p,vap (T_s - T) = B_T L_v`
/// (Table 4 row B); without it (`bird = false`) `lambda Nu0 (T_s - T) = rho D Sh0 ln(1 + B_M) L_v`.
fn wet_bulb(ts: f64, ys: f64, re: f64, bird: bool) -> f64 {
    let (nu, sh) = nu_sh(re);
    let phi = CPV * RHO * DIFF * sh / (LAM * nu);
    let g = |t: f64| {
        let l1 = (1.0 + b_m(t, ys)).ln();
        if bird {
            CPV * (ts - t) - ((phi * l1).exp() - 1.0) * LV
        } else {
            LAM * nu * (ts - t) - RHO * DIFF * sh * l1 * LV
        }
    };
    let (mut lo, mut hi) = (250.0, TB - 1e-9);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if g(mid) > 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// `K = 4 rho D Sh0 ln(1 + B_M(T))/rho_l` (DK with `Sh0 = 2`: `8 rho D ln(1 + B_M)/rho_l`).
fn k_of(t: f64, ys: f64, sh: f64) -> f64 {
    4.0 * RHO * DIFF * sh * (1.0 + b_m(t, ys)).ln() / RHO_L
}

// ---- set-ups

struct Single {
    tr: Tracker,
    u: Vec<[f64; 3]>,
}

/// One droplet of data E released at rest at the end of the first step (`rate`, `t_end` as given), injected at `t_inj`.
fn single(t_inj: f64, ts: f64, ys: f64, gas: [f64; 3], drag: DragLaw, rate: f64, t_end: f64) -> Single {
    let mesh = Arc::new(box_mesh([1, 1, 1], [0.01, 0.01, 0.01], [Grading::Uniform; 3]).unwrap());
    let opts = Options {
        drag,
        scheme: Scheme::Analytic,
        interp: Interp::CellCentre,
        gravity: [0.0; 3],
        ..Options::default()
    };
    let mut tr = Tracker::new(mesh, opts, &[]).unwrap();
    tr.add_injector(InjectorSpec {
        name: "d".into(),
        kind: InjKind::Point([0.005; 3]),
        rate,
        t_start: 0.0,
        t_end,
        velocity: VelMode::Given([0.0; 3]),
        dist: SizeDist::Constant(D0),
        density: RHO_L,
        parcel_count: N0,
        seed: 1,
    })
    .unwrap();
    tr.set_injector_ext(
        "d",
        InjectorExt {
            cone: None,
            mass_flow: None,
            temperature: Some(t_inj),
        },
    )
    .unwrap();
    tr.set_thermal(Thermal {
        cp_p: C_L,
        k_f: LAM,
        cp_f: CPM,
        t_fluid: vec![ts],
    })
    .unwrap();
    tr.set_evaporation(spec_e(), vec![ys]).unwrap();
    Single { tr, u: vec![gas] }
}

impl Single {
    fn step(&mut self, dt: f64) {
        let flow = Flow {
            u: &self.u,
            grad: None,
            rho: RHO,
            mu: MU,
        };
        self.tr.advance(&flow, dt).unwrap();
    }
}

fn total(v: &[f64]) -> f64 {
    v.iter().sum()
}

/// The EV1e check after a step: the vapour against the liquid lost; returns the relative errors (0 when nothing was lost).
fn mass_check(tr: &Tracker, what: &str) -> (f64, f64) {
    let ev = tr.evaporation().unwrap();
    let s = total(ev.source());
    let p = &tr.particles[0];
    let lost = N0 * (PI * RHO_L / 6.0) * (D0 * D0 * D0 - p.d * p.d * p.d);
    if lost == 0.0 {
        assert_eq!(s, 0.0, "{what}: vapour without a loss");
        assert_eq!(ev.evaporated(), 0.0, "{what}: evaporated without a loss");
        return (0.0, 0.0);
    }
    let e = rel(s, lost);
    let e2 = rel(ev.evaporated(), s);
    assert!(e <= 1e-10, "{what}: vapour {s:e} against lost {lost:e}: {e:e}");
    assert!(e2 <= 1e-12, "{what}: evaporated against the sources: {e2:e}");
    (e, e2)
}

/// Least-squares line of `y` against `t`: slope and the largest absolute residual.
fn fit(pts: &[(f64, f64)]) -> (f64, f64) {
    let n = pts.len() as f64;
    let mt = pts.iter().map(|p| p.0).sum::<f64>() / n;
    let my = pts.iter().map(|p| p.1).sum::<f64>() / n;
    let sxy: f64 = pts.iter().map(|p| (p.0 - mt) * (p.1 - my)).sum();
    let sxx: f64 = pts.iter().map(|p| (p.0 - mt) * (p.0 - mt)).sum();
    let slope = sxy / sxx;
    let res = pts
        .iter()
        .map(|p| (p.1 - (my + slope * (p.0 - mt))).abs())
        .fold(0.0, f64::max);
    (slope, res)
}

/// Runs the single droplet in steps of 1e-4 s until it has evaporated (at most 200 steps); the samples `(age, d^2, T_p)`
/// after the steps at which it is active; the mass check after every step.
fn run_single(t_inj: f64, what: &str) -> (Single, Vec<(f64, f64, f64)>, f64) {
    let mut s = single(t_inj, 800.0, 0.0, [0.0; 3], DragLaw::Stokes, 1e4, 1e-4);
    let mut samples = Vec::new();
    let mut worst = 0.0f64;
    for step in 1..=200 {
        s.step(1e-4);
        let (e, _) = mass_check(&s.tr, &format!("{what} step {step}"));
        worst = worst.max(e);
        let p = &s.tr.particles[0];
        if p.state == State::Active {
            samples.push((p.age, p.d * p.d, s.tr.temperature(0)));
        } else {
            break;
        }
    }
    (s, samples, worst)
}

/// EV1 (acceptance A1) with EV1b to EV1e, and the negative controls NE1, NE2.
#[test]
fn ev1_droplet_at_its_wet_bulb_follows_the_d2_law() {
    let twb = wet_bulb(800.0, 0.0, 0.0, true);
    let k = k_of(twb, 0.0, 2.0);
    let bm = b_m(twb, 0.0);
    let phi = CPV * RHO * DIFF / LAM;
    let bt = (1.0 + bm).powf(phi) - 1.0;
    let k_bird = 8.0 * LAM * bt.ln_1p() / (RHO_L * CPV);
    println!(
        "DK: T_wb {twb:.10} K, X {:.6}, Y_i {:.6}, B_M {bm:.8}, phi {phi:.6}, B_T {bt:.8}, K {k:.10e} (Bird form {k_bird:.10e})",
        x_of(twb),
        y_i(twb)
    );
    let (s, samples, worst) = run_single(twb, "EV1e");
    let p = &s.tr.particles[0];
    println!(
        "EV1: {} samples, final state {:?}, age at completion {:.10e}; mass worst {worst:.2e}",
        samples.len(),
        p.state,
        p.age
    );
    let d02 = D0 * D0;
    let pts: Vec<(f64, f64)> = samples
        .iter()
        .filter(|q| q.1 / d02 >= 0.05 && q.1 / d02 <= 0.95)
        .map(|q| (q.0, q.1))
        .collect();
    let (slope, res) = fit(&pts);
    let e1 = rel(-slope, k);
    let tmax = samples.iter().map(|q| (q.2 - twb).abs()).fold(0.0, f64::max);
    let life = (d02 - DMIN * DMIN) / k;
    let e_d = rel(p.age, life);
    println!(
        "EV1 slope {:.10e} over {} samples: rel {e1:.2e}; EV1b residual {:.2e} d0^2; EV1c max |T_p - T_wb| {tmax:.2e} K; EV1d age {:.10e} against {life:.10e}: {e_d:.2e}",
        -slope,
        pts.len(),
        res / d02,
        p.age
    );
    assert!(e1 <= 0.02, "EV1 slope: {e1:e}");
    assert!(res <= 1e-3 * d02, "EV1b residual {:e} d0^2", res / d02);
    assert!(tmax <= 1e-6, "EV1c temperature: {tmax:e} K");
    assert_eq!(p.state, State::Evaporated, "EV1d state");
    assert!(e_d <= 0.02, "EV1d completion age: {e_d:e}");
    let ev = s.tr.evaporation().unwrap();
    let e_end = rel(total(ev.source()), N0 * (PI * RHO_L / 6.0) * D0 * D0 * D0);
    println!(
        "EV1e at completion: vapour against the injected mass {e_end:.2e}; completed {}",
        ev.completed()
    );
    assert!(e_end <= 1e-10, "EV1e at completion: {e_end:e}");
    // NE1: the classical model (no Bird correction) at its own wet bulb
    let t_sd = wet_bulb(800.0, 0.0, 0.0, false);
    let k_sd = k_of(t_sd, 0.0, 2.0);
    let ne1 = rel(-slope, k_sd);
    // NE2: B_M in place of ln(1 + B_M)
    let k_lin = 8.0 * RHO * DIFF * bm / RHO_L;
    let ne2 = rel(-slope, k_lin);
    println!("NE1 classical T_wb {t_sd:.6} K, K {k_sd:.6e}: slope {ne1:.4} off; NE2 K {k_lin:.6e}: {ne2:.4} off");
    assert!(ne1 >= 0.5, "NE1 must be at least 50 percent off: {ne1}");
    assert!(ne2 >= 0.5, "NE2 must be at least 50 percent off: {ne2}");
}

/// EV2: a droplet injected at 300 K heats to its wet bulb, after which the slope is K.
#[test]
fn ev2_cold_droplet_reaches_the_d2_slope_after_heat_up() {
    let twb = wet_bulb(800.0, 0.0, 0.0, true);
    let k = k_of(twb, 0.0, 2.0);
    let (s, samples, worst) = run_single(300.0, "EV2");
    let d02 = D0 * D0;
    let pts: Vec<(f64, f64)> = samples
        .iter()
        .filter(|q| q.1 / d02 >= 0.05 && q.1 / d02 <= 0.25)
        .map(|q| (q.0, q.1))
        .collect();
    let (slope, _) = fit(&pts);
    let e = rel(-slope, k);
    let tmax = samples.iter().map(|q| q.2 - twb).fold(f64::NEG_INFINITY, f64::max);
    let last = samples.last().unwrap();
    let first_close = samples.iter().find(|q| twb - q.2 < 0.1).map(|q| q.1 / d02);
    println!(
        "EV2: {} samples, final state {:?}; slope {:.10e} over {} samples: rel {e:.2e}; max T_p - T_wb {tmax:.3e} K; last sample d^2/d0^2 {:.4e}, T_p - T_wb {:.3e} K; first within 0.1 K at d^2/d0^2 {first_close:?}; mass worst {worst:.2e}",
        samples.len(),
        s.tr.particles[0].state,
        -slope,
        pts.len(),
        last.1 / d02,
        last.2 - twb
    );
    assert!(e <= 0.02, "EV2 slope: {e:e}");
    assert!(tmax <= 1e-3, "EV2 overshoot: {tmax:e} K");
    assert!(
        (last.2 - twb).abs() <= 1e-4,
        "EV2 last temperature: {:e} K",
        last.2 - twb
    );
}

/// EV3: no driving force, no evaporation.
#[test]
fn ev3_saturated_gas_at_the_droplet_temperature() {
    let ys = y_i(330.0);
    let mut s = single(330.0, 330.0, ys, [0.0; 3], DragLaw::Stokes, 1e4, 1e-4);
    for _ in 0..100 {
        s.step(1e-4);
    }
    let p = &s.tr.particles[0];
    let m0 = (PI * RHO_L / 6.0) * D0 * D0 * D0;
    let src = total(s.tr.evaporation().unwrap().source());
    let e = rel(p.d, D0);
    println!(
        "EV3: Y_s {ys:.15}, d {:.17e} rel {e:.2e}, T_p {:.12}, vapour {src:e} = {:.2e} n m0",
        p.d,
        s.tr.temperature(0),
        src / (N0 * m0)
    );
    assert!(e <= 1e-9, "EV3 diameter: {e:e}");
    assert!(src.abs() <= 1e-9 * N0 * m0, "EV3 vapour: {src:e}");
}

/// EV4: the convective rates (Ranz-Marshall Nu0 and Sh0 at Re0 = 16.667).
#[test]
fn ev4_convective_rate_at_its_own_wet_bulb() {
    let u = 20.0;
    let re0 = RHO * D0 * u / MU;
    let (nu, sh) = nu_sh(re0);
    let twb = wet_bulb(800.0, 0.0, re0, true);
    let kloc = k_of(twb, 0.0, sh);
    let mut s = single(twb, 800.0, 0.0, [u, 0.0, 0.0], DragLaw::None, 1e5, 1e-5);
    s.step(1e-5);
    s.step(1e-5);
    let p = &s.tr.particles[0];
    let rate = (D0 * D0 - p.d * p.d) / p.age;
    let e = rel(rate, kloc);
    println!(
        "EV4: Re0 {re0:.6}, Nu0 {nu:.6}, Sh0 {sh:.6}, T_wb(Re0) {twb:.8} K, K_loc {kloc:.10e}; age {:.6e}, measured {rate:.10e}: rel {e:.2e}",
        p.age
    );
    assert!(e <= 0.02, "EV4: {e:e}");
}

/// EV5: the analytic Jacobian of the library's temperature rate against a central difference.
#[test]
fn ev5_jacobian_against_a_central_difference() {
    let ev = Evaporation::new(spec_e(), vec![0.0], 1).unwrap();
    let th = Thermal {
        cp_p: C_L,
        k_f: LAM,
        cp_f: CPM,
        t_fluid: vec![800.0],
    };
    let mut worst = 0.0f64;
    for re in [0.0, 16.666_666_666_666_668] {
        let speed = re * MU / (RHO * D0);
        for t in [300.0, 330.0, 356.0, 365.0] {
            let r = |tp: f64| ev.rates(&th, RHO, MU, 0, D0, RHO_L, tp, speed).unwrap();
            let fd = (r(t + 1e-3).f - r(t - 1e-3).f) / 2e-3;
            let j = r(t).jac;
            let e = rel(j, fd);
            println!("EV5 Re {re:.3} T {t}: jac {j:.10e} central {fd:.10e} rel {e:.2e}");
            worst = worst.max(e);
            assert!(e <= 1e-5, "EV5 Re {re} T {t}: {e:e}");
        }
    }
    println!("EV5 worst {worst:.2e}");
}

// ---- C. the spray

struct SpRun {
    tr: Tracker,
    worst_sp1: f64,
    worst_sp2: f64,
    checked: usize,
    is_a: Vec<bool>,
}

fn sp_injectors() -> [InjectorSpec; 2] {
    [
        InjectorSpec {
            name: "a".into(),
            kind: InjKind::Point([0.005, 0.012, 0.012]),
            rate: 2e4,
            t_start: 0.0,
            t_end: 0.004,
            velocity: VelMode::Given([0.0; 3]),
            dist: SizeDist::Constant(4e-4),
            density: RHO_L,
            parcel_count: 20.0,
            seed: 21,
        },
        InjectorSpec {
            name: "b".into(),
            kind: InjKind::Point([0.005, 0.008, 0.008]),
            rate: 2e4,
            t_start: 0.001,
            t_end: 0.005,
            velocity: VelMode::Given([0.0; 3]),
            dist: SizeDist::Constant(3e-5),
            density: RHO_L,
            parcel_count: 1000.0,
            seed: 22,
        },
    ]
}

/// Scenario SP of ADR 0224 C, with the per-step checks SP1 and SP2.
fn sp_run() -> SpRun {
    let mesh = Arc::new(box_mesh([8, 2, 2], [0.08, 0.02, 0.02], [Grading::Uniform; 3]).unwrap());
    let opts = Options {
        drag: DragLaw::SchillerNaumann,
        scheme: Scheme::Analytic,
        interp: Interp::CellCentre,
        gravity: [0.0, 0.0, -9.81],
        default_wall: WallAction::Reflect(0.8),
        ..Options::default()
    };
    let overrides = [
        ("xmax".to_string(), WallAction::Escape),
        ("zmin".to_string(), WallAction::Stick),
    ];
    let mut tr = Tracker::new(mesh.clone(), opts, &overrides).unwrap();
    let inj = sp_injectors();
    for (spec, t) in inj.iter().zip([300.0, 320.0]) {
        tr.add_injector(spec.clone()).unwrap();
        tr.set_injector_ext(
            &spec.name,
            InjectorExt {
                cone: None,
                mass_flow: None,
                temperature: Some(t),
            },
        )
        .unwrap();
    }
    let xc: Vec<f64> = mesh.cell_centres().iter().map(|c| c[0]).collect();
    tr.set_thermal(Thermal {
        cp_p: C_L,
        k_f: LAM,
        cp_f: CPM,
        t_fluid: xc.iter().map(|x| 700.0 + 1250.0 * x).collect(),
    })
    .unwrap();
    tr.set_evaporation(spec_e(), xc.iter().map(|x| 0.05 * x / 0.08).collect())
        .unwrap();
    tr.set_breakup(BreakupSpec::tab(0.01, 4e-4, 31)).unwrap();
    let u = vec![[30.0, 0.0, -2.0]; mesh.n_cells()];
    let flow = Flow {
        u: &u,
        grad: None,
        rho: RHO,
        mu: MU,
    };
    let m_a = 20.0 * (PI * RHO_L / 6.0) * 4e-4f64.powi(3);
    let m_b = 1000.0 * (PI * RHO_L / 6.0) * 3e-5f64.powi(3);
    let (mut worst_sp1, mut worst_sp2, mut checked) = (0.0f64, 0.0f64, 0);
    let mut is_a = Vec::new();
    let (mut na, mut nb) = (0u64, 0u64);
    for step in 1..=300 {
        tr.advance(&flow, 1e-4).unwrap();
        let t = step as f64 * 1e-4;
        let (na1, nb1) = (inj[0].emitted_by(t), inj[1].emitted_by(t));
        // the parcels of a step are spawned injector by injector: first those of a, then those of b
        is_a.resize(is_a.len() + (na1 - na) as usize, true);
        is_a.resize(is_a.len() + (nb1 - nb) as usize, false);
        (na, nb) = (na1, nb1);
        assert_eq!(tr.particles.len(), is_a.len(), "SP parcel count at step {step}");
        let m_inj = na as f64 * m_a + nb as f64 * m_b;
        let m_liq: f64 = tr
            .particles
            .iter()
            .map(|p| p.parcel * (PI * p.rho / 6.0) * p.d * p.d * p.d)
            .sum();
        let s = total(tr.evaporation().unwrap().source());
        let e2 = rel(m_liq + s, m_inj);
        worst_sp2 = worst_sp2.max(e2);
        assert!(e2 <= 1e-12, "SP2 step {step}: {e2:e}");
        if m_inj > m_liq {
            let e1 = rel(s, m_inj - m_liq);
            worst_sp1 = worst_sp1.max(e1);
            checked += 1;
            assert!(e1 <= 1e-10, "SP1 step {step}: {e1:e}");
        }
    }
    SpRun {
        tr,
        worst_sp1,
        worst_sp2,
        checked,
        is_a,
    }
}

/// SP1 (acceptance A1, mass, many cells), SP2 (acceptance A2, mass) and SP3 (coverage).
#[test]
fn sp_spray_with_evaporation_and_breakup_conserves_liquid_and_vapour() {
    let r = sp_run();
    let tr = &r.tr;
    let a_total = r.is_a.iter().filter(|x| **x).count();
    let a_broken = (0..tr.particles.len())
        .filter(|&i| r.is_a[i] && tr.breakups(i).0 >= 1)
        .count();
    let b_broken = (0..tr.particles.len())
        .filter(|&i| !r.is_a[i] && tr.breakups(i).0 >= 1)
        .count();
    let max_bu = (0..tr.particles.len()).map(|i| tr.breakups(i).0).max().unwrap_or(0);
    let counts = [
        State::Active,
        State::Deposited,
        State::Escaped,
        State::Stuck,
        State::Film,
        State::Evaporated,
    ]
    .map(|s| tr.count(s));
    let ev = tr.evaporation().unwrap();
    println!(
        "SP: {} parcels ({a_total} of a); SP1 {} steps checked, worst {:.2e}; SP2 worst {:.2e}; a broken {a_broken}, b broken {b_broken}, most breakups of a parcel {max_bu}, events {}; active/deposited/escaped/stuck/film/evaporated {counts:?}; vapour {:.6e} kg, completed {}",
        tr.particles.len(),
        r.checked,
        r.worst_sp1,
        r.worst_sp2,
        tr.breakup().unwrap().events(),
        ev.evaporated(),
        ev.completed()
    );
    assert_eq!(a_broken, a_total, "SP3 every parcel of a broke up");
    assert!(counts[5] >= 1, "SP3 one parcel evaporated: {counts:?}");
    assert!(counts[2] >= 1, "SP3 one parcel escaped: {counts:?}");
}

/// RF1: the refusals of evaporation and the controls.
#[test]
fn rf1_evaporation_refusals() {
    let mesh = Arc::new(box_mesh([1, 1, 1], [0.01, 0.01, 0.01], [Grading::Uniform; 3]).unwrap());
    let th = || Thermal {
        cp_p: C_L,
        k_f: LAM,
        cp_f: CPM,
        t_fluid: vec![800.0],
    };
    let fresh = || Tracker::new(mesh.clone(), Options::default(), &[]).unwrap();
    let with_th = || {
        let mut t = fresh();
        t.set_thermal(th()).unwrap();
        t
    };
    let mut results: Vec<(String, bool)> = Vec::new();
    let mut push = |what: &str, r: std::result::Result<(), ParticleError>| {
        println!("RF1 {what}: {r:?}");
        results.push((what.to_string(), r.is_err()));
    };
    push("without set_thermal", fresh().set_evaporation(spec_e(), vec![0.0]));
    {
        let mut t = fresh();
        let rx = ReactionSpec {
            solid_molar_mass: 0.012,
            solid_coefficient: 1.0,
            gas: vec![
                GasSpecies {
                    name: "O".into(),
                    molar_mass: 0.032,
                    coefficient: -1.0,
                },
                GasSpecies {
                    name: "P".into(),
                    molar_mass: 0.044,
                    coefficient: 1.0,
                },
            ],
            oxidiser: 0,
            rate: 0.01,
            reacting_fraction: 0.875,
        };
        t.set_reaction(rx.clone(), vec![0.2]).unwrap();
        push("with a reaction set", t.set_evaporation(spec_e(), vec![0.0]));
        let mut t2 = with_th();
        t2.set_evaporation(spec_e(), vec![0.0]).unwrap();
        push("second set_evaporation", t2.set_evaporation(spec_e(), vec![0.0]));
        push("set_reaction after set_evaporation", t2.set_reaction(rx, vec![0.2]));
    }
    let bad: Vec<(&str, Mutator<EvaporationSpec>)> = vec![
        ("L_v = 0", Box::new(|s| s.latent_heat = 0.0)),
        ("T_b = 0", Box::new(|s| s.boiling_temperature = 0.0)),
        ("p = 0", Box::new(|s| s.pressure = 0.0)),
        ("p_b < 0", Box::new(|s| s.boiling_pressure = -1.0)),
        ("W_f = 0", Box::new(|s| s.vapour_molar_mass = 0.0)),
        ("W_b = -0.029", Box::new(|s| s.gas_molar_mass = -0.029)),
        ("D_m = 0", Box::new(|s| s.diffusivity = 0.0)),
        ("c_p,vap = 0", Box::new(|s| s.vapour_cp = 0.0)),
        ("d_min = 0", Box::new(|s| s.min_diameter = 0.0)),
        ("f = 0", Box::new(|s| s.max_step_fraction = 0.0)),
        ("f = 1.5", Box::new(|s| s.max_step_fraction = 1.5)),
        ("NaN latent heat", Box::new(|s| s.latent_heat = f64::NAN)),
    ];
    for (what, f) in bad {
        let mut s = spec_e();
        f(&mut s);
        push(what, with_th().set_evaporation(s, vec![0.0]));
    }
    push("y_s one value short", with_th().set_evaporation(spec_e(), vec![]));
    push("y_s = 1", with_th().set_evaporation(spec_e(), vec![1.0]));
    push("y_s = -0.1", with_th().set_evaporation(spec_e(), vec![-0.1]));
    push(
        "set_evaporation_vapour without evaporation",
        with_th().set_evaporation_vapour(&[0.01]),
    );
    let u = vec![[0.0; 3]];
    let flow = Flow {
        u: &u,
        grad: None,
        rho: RHO,
        mu: MU,
    };
    {
        let mut s = single(300.0, 800.0, 0.0, [0.0; 3], DragLaw::Stokes, 1e4, 1e-4);
        s.tr.enable_coupling();
        push("advance with the exchange record", s.tr.advance(&flow, 2e-4));
        let mut s2 = single(380.0, 800.0, 0.0, [0.0; 3], DragLaw::Stokes, 1e4, 1e-4);
        push("advance of a droplet at 380 K", s2.tr.advance(&flow, 2e-4));
    }
    let failed: Vec<&String> = results.iter().filter(|r| !r.1).map(|r| &r.0).collect();
    assert!(failed.is_empty(), "RF1 accepted: {failed:?}");
    assert_eq!(results.len(), 22, "RF1 registered refusals");
    let mut c = with_th();
    let ok1 = c.set_evaporation(spec_e(), vec![0.0]);
    let ok2 = c.set_evaporation_vapour(&[0.01]);
    println!("RF1 controls: {ok1:?} {ok2:?}");
    assert!(ok1.is_ok() && ok2.is_ok(), "RF1 controls");
}
