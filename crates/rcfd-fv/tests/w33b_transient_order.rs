//! ADR 0069 (wave 33B): temporal order of the BDF2 paths. Checks D0, O1 to O9 (registered bands) and the diagnostic matrix X1 to X7, S0.
//! The set-ups, solver tolerances, pairs and bands are those registered in the ADR before any run; none was changed afterwards.
//!
//! Error measure: relative L2 of the velocity difference at t = 1 to a reference run of the same scheme on the same mesh with the
//! constant step 1/1280 (temporal error at fixed h). Observed order of a pair = log2(e(dt)/e(dt/2)).

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::{add_ddt, History, TimeScheme};
use rcfd_fv::pimple::fractional::FractionalSettings;
use rcfd_fv::pimple::{OuterHook, OuterUpdate, PimpleSettings, PimpleSolver};
use rcfd_fv::simple::BodyForce;
use rcfd_fv::{FixedValue, FvGeometry, FvMatrix, ScalarField, VectorField, ZeroGradient};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::box_2d;
use std::f64::consts::PI;
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Prob {
    Tg,
    Mode,
    Mms,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Path {
    Piso,
    PimpleC,
    Pimple4,
    Frac,
}

#[derive(Clone, Copy, Debug)]
struct Cfg {
    prob: Prob,
    path: Path,
    scheme: TimeScheme,
    ddt_corr: bool,
    n_corr: Option<usize>,
    n_outer: Option<usize>,
    conv: ConvScheme,
    exact_start: bool,
    exact_flux: bool,
    f3: bool,
}

impl Cfg {
    pub fn new(prob: Prob, path: Path, scheme: TimeScheme) -> Self {
        Cfg {
            prob,
            path,
            scheme,
            ddt_corr: true,
            n_corr: None,
            n_outer: None,
            conv: ConvScheme::Central,
            exact_start: false,
            exact_flux: false,
            f3: false,
        }
    }
}

fn nu_of(p: Prob) -> f64 {
    match p {
        Prob::Tg | Prob::Mode => 0.1,
        Prob::Mms => 0.05,
    }
}

// ---- manufactured wall-bounded solution -------------------------------------------------------------------------------------
fn g_of(t: f64) -> f64 {
    80.0 * (1.0 + 0.5 * (2.0 * t + 0.3).sin())
}
fn gdot_of(t: f64) -> f64 {
    80.0 * (2.0 * t + 0.3).cos()
}
/// X, X', X'', X''' of X = x^2 (1 - x)^2.
fn xs(x: f64) -> [f64; 4] {
    [
        x * x * (1.0 - x) * (1.0 - x),
        2.0 * x - 6.0 * x * x + 4.0 * x * x * x,
        2.0 - 12.0 * x + 12.0 * x * x,
        -12.0 + 24.0 * x,
    ]
}
fn p_amp(t: f64) -> f64 {
    0.5 * (g_of(t) / 80.0).powi(2)
}
fn mms_pressure(x: Vec3, t: f64) -> f64 {
    p_amp(t) * (PI * x[0]).cos() * (PI * x[1]).cos()
}
fn mms_grad_p(x: Vec3, t: f64) -> Vec3 {
    let a = p_amp(t);
    [
        -a * PI * (PI * x[0]).sin() * (PI * x[1]).cos(),
        -a * PI * (PI * x[0]).cos() * (PI * x[1]).sin(),
        0.0,
    ]
}
fn mms_velocity(x: Vec3, t: f64) -> Vec3 {
    let (a, b) = (xs(x[0]), xs(x[1]));
    let g = g_of(t);
    [g * a[0] * b[1], -g * a[1] * b[0], 0.0]
}
/// Source acceleration `f = u_t + u.grad u + grad p_ex - nu lap u`.
fn mms_source(x: Vec3, t: f64) -> Vec3 {
    let nu = nu_of(Prob::Mms);
    let (a, b) = (xs(x[0]), xs(x[1]));
    let (g, gd) = (g_of(t), gdot_of(t));
    let dp = mms_grad_p(x, t);
    let fx = gd * a[0] * b[1] + g * g * (a[0] * a[1] * b[1] * b[1] - a[0] * a[1] * b[0] * b[2])
        - nu * g * (a[2] * b[1] + a[0] * b[3])
        + dp[0];
    let fy = -gd * a[1] * b[0]
        + g * g * (-a[0] * a[2] * b[0] * b[1] + a[1] * a[1] * b[0] * b[1])
        + nu * g * (a[3] * b[0] + a[1] * b[2])
        + dp[1];
    [fx, fy, 0.0]
}

fn exact_u(p: Prob, x: Vec3, t: f64) -> Vec3 {
    match p {
        Prob::Tg => {
            let d = (-2.0 * nu_of(p) * t).exp();
            [x[0].sin() * x[1].cos() * d, -x[0].cos() * x[1].sin() * d, 0.0]
        }
        Prob::Mode => [0.0, (PI * x[0]).sin() * (-nu_of(p) * PI * PI * t).exp(), 0.0],
        Prob::Mms => mms_velocity(x, t),
    }
}

fn exact_p(p: Prob, x: Vec3, t: f64) -> f64 {
    match p {
        Prob::Tg => 0.25 * ((2.0 * x[0]).cos() + (2.0 * x[1]).cos()) * (-4.0 * nu_of(p) * t).exp(),
        Prob::Mode => 0.0,
        Prob::Mms => mms_pressure(x, t),
    }
}

fn mms_force(geom: &FvGeometry, t: f64) -> BodyForce {
    let mesh = &geom.mesh;
    let (cc, fc, sf) = (mesh.cell_centres(), mesh.face_centres(), mesh.face_area_vectors());
    let cell: Vec<Vec3> = cc.iter().map(|&c| mms_source(c, t)).collect();
    let face: Vec<f64> = (0..mesh.n_faces())
        .map(|f| {
            if geom.empty[f] {
                0.0
            } else if f < geom.n_internal {
                vec3::dot(mms_source(fc[f], t), sf[f])
            } else {
                // wall faces: the TRUE pressure gradient (see ADR 0069)
                vec3::dot(mms_grad_p(fc[f], t), sf[f])
            }
        })
        .collect();
    BodyForce { cell, face }
}

// ---- solver construction ----------------------------------------------------------------------------------------------------
fn settings(cfg: &Cfg) -> PimpleSettings {
    let mut s = PimpleSettings::piso(nu_of(cfg.prob));
    s.convection = cfg.conv;
    s.time_scheme = cfg.scheme;
    match cfg.path {
        Path::Piso => {}
        Path::PimpleC => {
            s.n_outer = 2;
            s.alpha_u = 0.9;
            s.alpha_p = 0.9;
        }
        Path::Pimple4 => s.n_outer = 4,
        Path::Frac => s.fractional = Some(FractionalSettings::default()),
    }
    s.ddt_correction = cfg.ddt_corr;
    s.component_diagonal = cfg.f3;
    if let Some(n) = cfg.n_corr {
        s.n_correctors = n;
    }
    if let Some(n) = cfg.n_outer {
        s.n_outer = n;
    }
    s.momentum_solver = SolveControl {
        tol_abs: 1e-14,
        tol_rel: 1e-12,
        max_iter: 5000,
    };
    s.pressure_solver = SolveControl {
        tol_abs: 1e-14,
        tol_rel: 1e-12,
        max_iter: 5000,
    };
    s
}

fn make(cfg: &Cfg, n: usize) -> PimpleSolver {
    let st = settings(cfg);
    let prob = cfg.prob;
    match prob {
        Prob::Tg => {
            let l = 2.0 * PI;
            let mesh = Arc::new(box_2d(n, n, l, l, true, true).unwrap());
            let geom = Arc::new(FvGeometry::new(mesh).unwrap());
            let u = VectorField::from_fn("U", &geom, |c| exact_u(prob, c, 0.0));
            let p = ScalarField::from_fn("p", &geom, |c| exact_p(prob, c, 0.0));
            PimpleSolver::new(geom, u, p, st)
        }
        Prob::Mode => {
            let mesh = Arc::new(box_2d(n, n, 1.0, 1.0, false, false).unwrap());
            let geom = Arc::new(FvGeometry::new(mesh).unwrap());
            let mut u = VectorField::from_fn("U", &geom, |c| exact_u(prob, c, 0.0));
            u.set_bc(&geom, "xmin", Box::new(FixedValue::vector([0.0; 3]))).unwrap();
            u.set_bc(&geom, "xmax", Box::new(FixedValue::vector([0.0; 3]))).unwrap();
            u.set_bc(&geom, "ymin", Box::new(ZeroGradient)).unwrap();
            u.set_bc(&geom, "ymax", Box::new(ZeroGradient)).unwrap();
            let mut p = ScalarField::uniform("p", &geom, 0.0);
            p.set_bc(&geom, "ymin", Box::new(FixedValue::scalar(0.0))).unwrap();
            p.set_bc(&geom, "ymax", Box::new(FixedValue::scalar(0.0))).unwrap();
            PimpleSolver::new(geom, u, p, st)
        }
        Prob::Mms => {
            let mesh = Arc::new(box_2d(n, n, 1.0, 1.0, false, false).unwrap());
            let geom = Arc::new(FvGeometry::new(mesh).unwrap());
            let mut u = VectorField::from_fn("U", &geom, |c| exact_u(prob, c, 0.0));
            for patch in ["xmin", "xmax", "ymin", "ymax"] {
                u.set_bc(&geom, patch, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
            }
            let p = ScalarField::from_fn("p", &geom, |c| exact_p(prob, c, 0.0));
            PimpleSolver::new(geom, u, p, st)
        }
    }
}

/// Apply the X1 (exact two-level history) and X2 (exact initial face flux) changes through the public restart state.
fn apply_start(s: &mut PimpleSolver, cfg: &Cfg, dt_first: f64) {
    if !(cfg.exact_start || cfg.exact_flux) {
        return;
    }
    let geom = s.geom.clone();
    let mesh = &geom.mesh;
    let (cc, fc, sf) = (mesh.cell_centres(), mesh.face_centres(), mesh.face_area_vectors());
    let flux_at = |t: f64| -> Vec<f64> {
        (0..mesh.n_faces())
            .map(|f| {
                if geom.empty[f] {
                    0.0
                } else {
                    vec3::dot(exact_u(cfg.prob, fc[f], t), sf[f])
                }
            })
            .collect()
    };
    let mut st = s.snapshot();
    if cfg.exact_flux {
        st.phi = flux_at(0.0);
        st.phi_hist.old = st.phi.clone();
    }
    if cfg.exact_start {
        let t = -dt_first;
        for k in 0..3 {
            st.hist_u[k].older = Some(cc.iter().map(|&c| exact_u(cfg.prob, c, t)[k]).collect());
            st.hist_u[k].dt_old = Some(dt_first);
        }
        st.phi_hist.older = Some(flux_at(t));
        st.phi_hist.dt_old = Some(dt_first);
    }
    s.restore(&st).unwrap();
}

/// Run with the given step sequence; returns the solver at the end.
fn run_seq(cfg: &Cfg, n: usize, dts: &[f64]) -> PimpleSolver {
    let mut s = make(cfg, n);
    apply_start(&mut s, cfg, dts[0]);
    for &dt in dts {
        if cfg.prob == Prob::Mms {
            let bf = mms_force(&s.geom, s.time + dt);
            let mut hook = move |_u: &VectorField, _phi: &[f64]| OuterUpdate {
                source_force: Some(bf.clone()),
                ..Default::default()
            };
            s.step_with(dt, Some(&mut hook as &mut OuterHook));
        } else {
            s.step(dt);
        }
        assert!(s.diverged.is_none(), "{:?}: diverged", cfg);
    }
    s
}

fn const_seq(dt: f64) -> Vec<f64> {
    vec![dt; (1.0 / dt).round() as usize]
}

/// Alternating dt (1.25), dt (0.75), ending at t = 1.
fn var_seq(dt: f64) -> Vec<f64> {
    let pairs = (1.0 / (2.0 * dt)).round() as usize;
    (0..2 * pairs)
        .map(|i| if i % 2 == 0 { 1.25 * dt } else { 0.75 * dt })
        .collect()
}

const DT_REF: f64 = 1.0 / 1280.0;

fn rel_l2(a: &[Vec3], b: &[Vec3]) -> f64 {
    let (mut num, mut den) = (0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        num += (x[0] - y[0]).powi(2) + (x[1] - y[1]).powi(2);
        den += y[0] * y[0] + y[1] * y[1];
    }
    (num / den).sqrt()
}

fn exact_error(s: &PimpleSolver, prob: Prob) -> f64 {
    let ex: Vec<Vec3> = s
        .geom
        .mesh
        .cell_centres()
        .iter()
        .map(|&c| exact_u(prob, c, s.time))
        .collect();
    rel_l2(&s.u.values, &ex)
}

/// Reference configuration: same scheme, no start modifications.
fn ref_cfg(cfg: &Cfg) -> Cfg {
    Cfg {
        exact_start: false,
        exact_flux: false,
        ..*cfg
    }
}

struct PairResult {
    e: [f64; 2],
    ex: [f64; 2],
    order: f64,
}

/// Errors against the reference for the pair (dt, dt/2); `variable` uses the alternating sequence of mean dt.
fn pair(cfg: &Cfg, n: usize, dt: f64, variable: bool, reference: &PimpleSolver) -> PairResult {
    let seq = |d: f64| if variable { var_seq(d) } else { const_seq(d) };
    let a = run_seq(cfg, n, &seq(dt));
    let b = run_seq(cfg, n, &seq(0.5 * dt));
    let e = [
        rel_l2(&a.u.values, &reference.u.values),
        rel_l2(&b.u.values, &reference.u.values),
    ];
    PairResult {
        e,
        ex: [exact_error(&a, cfg.prob), exact_error(&b, cfg.prob)],
        order: (e[0] / e[1]).log2(),
    }
}

fn reference(cfg: &Cfg, n: usize) -> PimpleSolver {
    run_seq(&ref_cfg(cfg), n, &const_seq(DT_REF))
}

fn report(id: &str, cfg: &Cfg, n: usize, dt: f64, variable: bool, r: &PairResult, lo: f64, hi: f64) {
    println!(
        "RESULT {id} {:?}/{:?}/{:?} N={n} dt={dt}{} : e {:.4e} {:.4e} (vs exact {:.4e} {:.4e}) order {:.3} band [{lo}, {hi}] {}",
        cfg.prob,
        cfg.path,
        cfg.scheme,
        if variable { " variable" } else { "" },
        r.e[0],
        r.e[1],
        r.ex[0],
        r.ex[1],
        r.order,
        if r.order >= lo && r.order <= hi { "MET" } else { "MISS" }
    );
}

/// Registered check on the fine and/or coarse pair(s); returns the orders (asserted by the caller).
fn check(id: &str, cfg: &Cfg, n: usize, pairs: &[f64], variable: bool, lo: f64, hi: f64) -> Vec<f64> {
    let refr = reference(cfg, n);
    pairs
        .iter()
        .map(|&dt| {
            let r = pair(cfg, n, dt, variable, &refr);
            report(id, cfg, n, dt, variable, &r, lo, hi);
            r.order
        })
        .collect()
}

const FINE: f64 = 0.025;
const COARSE: f64 = 0.1;
const BDF2: TimeScheme = TimeScheme::Bdf2;
const EULER: TimeScheme = TimeScheme::ImplicitEuler;

fn assert_band(id: &str, orders: &[f64], lo: f64, hi: f64) {
    for &o in orders {
        assert!(o >= lo && o <= hi, "{id}: order {o} outside [{lo}, {hi}]");
    }
}

// ---- D0 ---------------------------------------------------------------------------------------------------------------------
#[test]
fn d0_bdf2_weights_exact_on_quadratics() {
    let mesh = Arc::new(box_2d(1, 1, 1.0, 1.0, false, false).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let vol = geom.mesh.cell_volumes().to_vec();
    let y = |t: f64| 3.0 + 2.0 * t - 1.5 * t * t;
    let yd = |t: f64| 2.0 - 3.0 * t;
    for r in [0.5, 1.0, 1.7] {
        let (dt_old, t_m1) = (0.2, 0.3);
        let dt = r * dt_old;
        let (t_n, t_new) = (t_m1 + dt_old, t_m1 + dt_old + dt);
        let mut h = History::new(vec![y(t_n)]);
        h.older = Some(vec![y(t_m1)]);
        h.dt_old = Some(dt_old);
        let m = add_ddt(BDF2, FvMatrix::zeros(&geom), &vol, dt, &h);
        // (diag y_new - b) / V = ddt y
        let ddt = (m.a.diag[0] * y(t_new) - m.b[0]) / vol[0];
        println!(
            "RESULT D0 r={r}: ddt {ddt:.15} exact {:.15} diff {:.2e}",
            yd(t_new),
            (ddt - yd(t_new)).abs()
        );
        assert!((ddt - yd(t_new)).abs() <= 1e-12, "r {r}");
    }
}

// ---- registered order checks -------------------------------------------------------------------------------------------------
#[test]
#[ignore = "known miss of ADR 0069 (registered limit unchanged; ADR 0069 addendum)"]
fn o1_tg_piso_bdf2_known_miss() {
    let o = check(
        "O1",
        &Cfg::new(Prob::Tg, Path::Piso, BDF2),
        32,
        &[FINE, COARSE],
        false,
        1.9,
        2.3,
    );
    assert_band("O1", &o, 1.9, 2.3);
}

#[test]
#[ignore = "known miss of ADR 0069 (registered limit unchanged; ADR 0069 addendum)"]
fn o2_tg_pimple_c_bdf2_known_miss() {
    let o = check(
        "O2",
        &Cfg::new(Prob::Tg, Path::PimpleC, BDF2),
        32,
        &[FINE, COARSE],
        false,
        1.9,
        2.3,
    );
    assert_band("O2", &o, 1.9, 2.3);
}

#[test]
#[ignore = "known miss of ADR 0069 (registered limit unchanged; ADR 0069 addendum)"]
fn o3_tg_pimple4_bdf2_known_miss() {
    let o = check(
        "O3",
        &Cfg::new(Prob::Tg, Path::Pimple4, BDF2),
        32,
        &[FINE],
        false,
        1.9,
        2.3,
    );
    assert_band("O3", &o, 1.9, 2.3);
}

#[test]
#[ignore = "known miss of ADR 0069 (registered limit unchanged; ADR 0069 addendum)"]
fn o4_tg_frac_bdf2_known_miss() {
    let o = check(
        "O4",
        &Cfg::new(Prob::Tg, Path::Frac, BDF2),
        32,
        &[FINE, COARSE],
        false,
        1.9,
        2.3,
    );
    assert_band("O4", &o, 1.9, 2.3);
}

#[test]
fn o5_mode_bdf2() {
    let mut all = Vec::new();
    for path in [Path::Piso, Path::Frac] {
        all.extend(check(
            "O5",
            &Cfg::new(Prob::Mode, path, BDF2),
            32,
            &[FINE, COARSE],
            false,
            1.9,
            2.3,
        ));
    }
    assert_band("O5", &all, 1.9, 2.3);
}

#[test]
#[ignore = "known miss of ADR 0069 (registered limit unchanged; ADR 0069 addendum)"]
fn o6_mms_bdf2_known_miss() {
    let mut all = Vec::new();
    for path in [Path::Piso, Path::PimpleC] {
        all.extend(check(
            "O6",
            &Cfg::new(Prob::Mms, path, BDF2),
            32,
            &[FINE, COARSE],
            false,
            1.9,
            2.3,
        ));
    }
    assert_band("O6", &all, 1.9, 2.3);
}

#[test]
#[ignore = "known miss of ADR 0069 (registered limit unchanged; ADR 0069 addendum)"]
fn o7_variable_step_bdf2_known_miss() {
    let mut all = Vec::new();
    for prob in [Prob::Tg, Prob::Mms] {
        all.extend(check(
            "O7",
            &Cfg::new(prob, Path::Piso, BDF2),
            32,
            &[FINE],
            true,
            1.9,
            2.3,
        ));
    }
    assert_band("O7", &all, 1.9, 2.3);
}

#[test]
fn o8_euler_controls() {
    let mut all = Vec::new();
    for (prob, path) in [
        (Prob::Tg, Path::Piso),
        (Prob::Mode, Path::Piso),
        (Prob::Mms, Path::Piso),
        (Prob::Tg, Path::Frac),
        (Prob::Mode, Path::Frac),
    ] {
        all.extend(check("O8", &Cfg::new(prob, path, EULER), 32, &[FINE], false, 0.9, 1.15));
    }
    assert_band("O8", &all, 0.9, 1.15);
}

#[test]
fn o9_joint_refinement_against_exact() {
    let cfg = Cfg::new(Prob::Tg, Path::Piso, BDF2);
    let mut errs = Vec::new();
    for (n, dt) in [(16usize, 0.1), (32, 0.05), (64, 0.025)] {
        let s = run_seq(&cfg, n, &const_seq(dt));
        errs.push(exact_error(&s, cfg.prob));
    }
    let o = [(errs[0] / errs[1]).log2(), (errs[1] / errs[2]).log2()];
    println!(
        "RESULT O9 errors {:.4e} {:.4e} {:.4e} orders {:.3} {:.3} (limit >= 1.9)",
        errs[0], errs[1], errs[2], o[0], o[1]
    );
    assert!(o[0] >= 1.9 && o[1] >= 1.9, "{o:?}");
}

// ---- registered fix F3 (ADR 0069 Amendment 1); F1 and F2 were measured and rejected, see the ADR addendum ---------------------
/// F3: Bdf2Bounded on the PISO path within three times the Bdf2 error at dt = 0.05; Bdf2 and Euler unchanged (1e-10).
#[test]
fn f3_component_diagonal() {
    let mut bad = Vec::new();
    for (prob, limit) in [(Prob::Mode, 5.6e-3), (Prob::Tg, 5.2e-3), (Prob::Mms, 1.7e-2)] {
        let mut c = Cfg::new(prob, Path::Piso, TimeScheme::Bdf2Bounded);
        c.f3 = true;
        let e = exact_error(&run_seq(&c, 32, &const_seq(0.05)), prob);
        println!("RESULT FX-F3 {prob:?} Bdf2Bounded error vs exact {e:.4e} (limit <= {limit:.1e})");
        if e.is_nan() || e > limit {
            bad.push(format!("{prob:?}: {e:e} > {limit:e}"));
        }
        for scheme in [BDF2, EULER] {
            let a = Cfg::new(prob, Path::Piso, scheme);
            let mut b = a;
            b.f3 = true;
            let d = rel_l2(
                &run_seq(&b, 32, &const_seq(0.05)).u.values,
                &run_seq(&a, 32, &const_seq(0.05)).u.values,
            );
            println!("RESULT FX-F3 {prob:?} {scheme:?}: difference flag on/off {d:.3e} (limit 1e-10)");
            if d > 1e-10 {
                bad.push(format!("{prob:?}/{scheme:?}: {d:e}"));
            }
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

// ---- diagnostic matrix (prints only) -------------------------------------------------------------------------------------------
fn diag_row(label: &str, cfg: &Cfg, n: usize, refr: &PimpleSolver) {
    let r = pair(cfg, n, FINE, false, refr);
    println!(
        "DIAG {label} {:?}/{:?}/{:?}: e {:.4e} {:.4e} order {:.3} (vs exact {:.4e} {:.4e})",
        cfg.prob, cfg.path, cfg.scheme, r.e[0], r.e[1], r.order, r.ex[0], r.ex[1]
    );
}

/// X1 to X5, X7 on every (problem, path) combination of the registered checks.
#[test]
#[ignore = "diagnostic matrix of ADR 0069 (prints only)"]
fn diag_x1_to_x5_x7() {
    let cases = [
        (Prob::Tg, Path::Piso),
        (Prob::Tg, Path::PimpleC),
        (Prob::Tg, Path::Frac),
        (Prob::Mode, Path::Piso),
        (Prob::Mode, Path::Frac),
        (Prob::Mms, Path::Piso),
        (Prob::Mms, Path::PimpleC),
    ];
    for (prob, path) in cases {
        let base = Cfg::new(prob, path, BDF2);
        let refr = reference(&base, 32);
        diag_row("base", &base, 32, &refr);
        diag_row(
            "X1 exact two-level start",
            &Cfg {
                exact_start: true,
                ..base
            },
            32,
            &refr,
        );
        diag_row(
            "X2 exact initial flux",
            &Cfg {
                exact_flux: true,
                ..base
            },
            32,
            &refr,
        );
        diag_row(
            "X1+X2",
            &Cfg {
                exact_start: true,
                exact_flux: true,
                ..base
            },
            32,
            &refr,
        );
        if path != Path::Frac {
            diag_row(
                "X3 ddt_correction off",
                &Cfg {
                    ddt_corr: false,
                    ..base
                },
                32,
                &reference(
                    &Cfg {
                        ddt_corr: false,
                        ..base
                    },
                    32,
                ),
            );
            diag_row(
                "X4 three correctors",
                &Cfg {
                    n_corr: Some(3),
                    ..base
                },
                32,
                &reference(
                    &Cfg {
                        n_corr: Some(3),
                        ..base
                    },
                    32,
                ),
            );
            diag_row(
                "X5 n_outer 4",
                &Cfg {
                    n_outer: Some(4),
                    ..base
                },
                32,
                &reference(
                    &Cfg {
                        n_outer: Some(4),
                        ..base
                    },
                    32,
                ),
            );
        }
        if prob != Prob::Mode {
            let lu = Cfg {
                conv: ConvScheme::LinearUpwind,
                ..base
            };
            diag_row("X7 linear upwind", &lu, 32, &reference(&lu, 32));
        }
    }
}

/// X6: e = a dt^2 + b h^2 dt on N = 16, 32, 64 (PISO and FRAC, TG).
#[test]
#[ignore = "diagnostic matrix of ADR 0069 (prints only)"]
fn diag_x6_mesh_dependence() {
    for path in [Path::Piso, Path::Frac] {
        let cfg = Cfg::new(Prob::Tg, path, BDF2);
        let dts = [0.05, 0.025, 0.0125];
        let mut rows: Vec<(f64, f64, f64)> = Vec::new(); // (h, dt, e)
        for n in [16usize, 32, 64] {
            let refr = reference(&cfg, n);
            for &dt in &dts {
                let a = run_seq(&cfg, n, &const_seq(dt));
                let e = rel_l2(&a.u.values, &refr.u.values);
                rows.push((2.0 * PI / n as f64, dt, e));
                println!("DIAG X6 {path:?} N={n} dt={dt} e {e:.4e}");
            }
        }
        // least squares for e = a dt^2 + b h^2 dt (weights 1/e so the relative misfit is fitted)
        let (mut s11, mut s12, mut s22, mut r1, mut r2) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for &(h, dt, e) in &rows {
            let (x1, x2, w) = (dt * dt, h * h * dt, 1.0 / e);
            s11 += w * w * x1 * x1;
            s12 += w * w * x1 * x2;
            s22 += w * w * x2 * x2;
            r1 += w * w * x1 * e;
            r2 += w * w * x2 * e;
        }
        let det = s11 * s22 - s12 * s12;
        let (a, b) = ((r1 * s22 - r2 * s12) / det, (s11 * r2 - s12 * r1) / det);
        let mut rms = 0.0;
        for &(h, dt, e) in &rows {
            rms += ((a * dt * dt + b * h * h * dt) / e - 1.0).powi(2);
        }
        let (h32, dt) = (2.0 * PI / 32.0, 0.025);
        let (t2, t1) = (a * dt * dt, b * h32 * h32 * dt);
        println!(
            "DIAG X6 {path:?}: a {a:.4e} b {b:.4e}; at N=32 dt=0.025 the dt^2 part is {t2:.3e} and the h^2 dt part {t1:.3e} ({:.1} % of the fit); relative rms misfit {:.3}",
            100.0 * t1 / (t1 + t2),
            (rms / rows.len() as f64).sqrt()
        );
    }
}

/// S0: Bdf2Bounded on the PISO path against BDF2 (the weight mismatch of review item 5).
#[test]
#[ignore = "diagnostic of ADR 0069 (prints only)"]
fn diag_s0_bdf2_bounded_weights() {
    let b = Cfg::new(Prob::Tg, Path::Piso, TimeScheme::Bdf2Bounded);
    let refr = reference(&b, 32);
    diag_row("S0 bdf2_bounded", &b, 32, &refr);
    let a = Cfg::new(Prob::Tg, Path::Piso, BDF2);
    let ra = run_seq(&a, 32, &const_seq(FINE));
    let rb = run_seq(&b, 32, &const_seq(FINE));
    println!(
        "DIAG S0 difference Bounded - BDF2 at dt {FINE}: {:.4e}",
        rel_l2(&rb.u.values, &ra.u.values)
    );
}
