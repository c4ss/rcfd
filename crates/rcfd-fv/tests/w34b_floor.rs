//! ADR 0073 (wave 34B) Part A: the Taylor-Green BDF2 order floor. Registered decomposition A0/A1 and the acceptance checks of the opt-in consistent
//! momentum interpolation F-G (`PimpleSettings::rhie_chow_consistent`, amendment 1). The falsifier Y1/Y2 and the rejected fixes F-C and F-K were run in
//! the scratch tree only (their code is not in the repository; results in the ADR). Set-ups, tolerances, pairs and the error measure are those of ADR 0069 (`w33b_transient_order.rs`); TG-S is the phase-shifted vortex.
//! Heavy tests are ignored: `cargo test --release -p rcfd-fv --test w34b_floor -- --ignored --nocapture`.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::pimple::fractional::FractionalSettings;
use rcfd_fv::pimple::{OuterHook, OuterUpdate, PimpleSettings, PimpleSolver};
use rcfd_fv::simple::BodyForce;
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField, ZeroGradient};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::box_2d;
use std::f64::consts::PI;
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Prob {
    Tg,
    TgS,
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
    /// F-G `rhie_chow_consistent`
    fg: bool,
}

impl Cfg {
    fn new(prob: Prob, path: Path) -> Self {
        Cfg {
            prob,
            path,
            scheme: TimeScheme::Bdf2,
            fg: false,
        }
    }
}

/// Phase shift of TG-S.
const SHIFT: f64 = 0.6;

fn nu_of(p: Prob) -> f64 {
    match p {
        Prob::Tg | Prob::TgS | Prob::Mode => 0.1,
        Prob::Mms => 0.05,
    }
}

// ---- manufactured wall-bounded solution (ADR 0069 MMS) ------------------------------------------------------------------------
fn g_of(t: f64) -> f64 {
    80.0 * (1.0 + 0.5 * (2.0 * t + 0.3).sin())
}
fn gdot_of(t: f64) -> f64 {
    80.0 * (2.0 * t + 0.3).cos()
}
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
fn mms_grad_p(x: Vec3, t: f64) -> Vec3 {
    let a = p_amp(t);
    [
        -a * PI * (PI * x[0]).sin() * (PI * x[1]).cos(),
        -a * PI * (PI * x[0]).cos() * (PI * x[1]).sin(),
        0.0,
    ]
}
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
                vec3::dot(mms_grad_p(fc[f], t), sf[f])
            }
        })
        .collect();
    BodyForce { cell, face }
}

fn exact_u(p: Prob, x: Vec3, t: f64) -> Vec3 {
    match p {
        Prob::Tg | Prob::TgS => {
            let s = if p == Prob::TgS { SHIFT } else { 0.0 };
            let d = (-2.0 * nu_of(p) * t).exp();
            let (a, b) = (x[0] + s, x[1] + s);
            [a.sin() * b.cos() * d, -a.cos() * b.sin() * d, 0.0]
        }
        Prob::Mode => [0.0, (PI * x[0]).sin() * (-nu_of(p) * PI * PI * t).exp(), 0.0],
        Prob::Mms => {
            let (a, b) = (xs(x[0]), xs(x[1]));
            let g = g_of(t);
            [g * a[0] * b[1], -g * a[1] * b[0], 0.0]
        }
    }
}

fn exact_p(p: Prob, x: Vec3, t: f64) -> f64 {
    match p {
        Prob::Tg | Prob::TgS => {
            let s = if p == Prob::TgS { SHIFT } else { 0.0 };
            0.25 * ((2.0 * (x[0] + s)).cos() + (2.0 * (x[1] + s)).cos()) * (-4.0 * nu_of(p) * t).exp()
        }
        Prob::Mode => 0.0,
        Prob::Mms => p_amp(t) * (PI * x[0]).cos() * (PI * x[1]).cos(),
    }
}

fn settings(cfg: &Cfg) -> PimpleSettings {
    let mut s = PimpleSettings::piso(nu_of(cfg.prob));
    s.convection = ConvScheme::Central;
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
    s.rhie_chow_consistent = cfg.fg;
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
        Prob::Tg | Prob::TgS => {
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

fn step_one(s: &mut PimpleSolver, cfg: &Cfg, dt: f64) {
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

fn run_seq(cfg: &Cfg, n: usize, dts: &[f64]) -> PimpleSolver {
    let mut s = make(cfg, n);
    for &dt in dts {
        step_one(&mut s, cfg, dt);
    }
    s
}

fn const_seq(dt: f64) -> Vec<f64> {
    vec![dt; (1.0 / dt).round() as usize]
}

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

/// Relative L2 of the cell pressure against the exact one, volume means removed from both.
fn pressure_error(s: &PimpleSolver, prob: Prob) -> f64 {
    let vol = s.geom.mesh.cell_volumes();
    let tv: f64 = vol.iter().sum();
    let ex: Vec<f64> = s
        .geom
        .mesh
        .cell_centres()
        .iter()
        .map(|&c| exact_p(prob, c, s.time))
        .collect();
    let mp = s.p.values.iter().zip(vol).map(|(a, v)| a * v).sum::<f64>() / tv;
    let me = ex.iter().zip(vol).map(|(a, v)| a * v).sum::<f64>() / tv;
    let (mut num, mut den) = (0.0, 0.0);
    for ((a, b), v) in s.p.values.iter().zip(&ex).zip(vol) {
        num += v * ((a - mp) - (b - me)).powi(2);
        den += v * (b - me).powi(2);
    }
    (num / den.max(1e-300)).sqrt()
}

fn reference(cfg: &Cfg, n: usize, dt_ref: f64) -> PimpleSolver {
    run_seq(cfg, n, &const_seq(dt_ref))
}

// ---- fits ---------------------------------------------------------------------------------------------------------------------------
/// Fit M: e = a dt^p + c h^s dt^r (least squares on log e over a grid of (p, s, r), a and c >= 0 by inner weighted least squares).
fn fit_m(rows: &[(f64, f64, f64)]) -> (f64, f64, f64, f64, f64, f64) {
    let ps: Vec<f64> = (0..=100).map(|i| 1.0 + 0.02 * i as f64).collect();
    let ss: Vec<f64> = (0..=80).map(|i| 0.05 * i as f64).collect();
    let rs: Vec<f64> = (0..=100).map(|i| 0.5 + 0.02 * i as f64).collect();
    let tp: Vec<Vec<f64>> = ps.iter().map(|&p| rows.iter().map(|r| r.1.powf(p)).collect()).collect();
    let th: Vec<Vec<f64>> = ss.iter().map(|&s| rows.iter().map(|r| r.0.powf(s)).collect()).collect();
    let tr: Vec<Vec<f64>> = rs.iter().map(|&q| rows.iter().map(|r| r.1.powf(q)).collect()).collect();
    let mut best = (f64::INFINITY, 0.0, 0.0, 0.0, 0.0, 0.0);
    let m = rows.len();
    let mut x2 = vec![0.0; m];
    for (ip, x1) in tp.iter().enumerate() {
        for (is, hs) in th.iter().enumerate() {
            for (ir, dr) in tr.iter().enumerate() {
                for k in 0..m {
                    x2[k] = hs[k] * dr[k];
                }
                // weighted normal equations, w = 1/e
                let (mut s11, mut s12, mut s22, mut r1, mut r2) = (0.0, 0.0, 0.0, 0.0, 0.0);
                for k in 0..m {
                    let w2 = 1.0 / (rows[k].2 * rows[k].2);
                    s11 += w2 * x1[k] * x1[k];
                    s12 += w2 * x1[k] * x2[k];
                    s22 += w2 * x2[k] * x2[k];
                    r1 += w2 * x1[k] * rows[k].2;
                    r2 += w2 * x2[k] * rows[k].2;
                }
                let det = s11 * s22 - s12 * s12;
                let (mut a, mut c) = if det.abs() > 1e-300 {
                    ((r1 * s22 - r2 * s12) / det, (s11 * r2 - s12 * r1) / det)
                } else {
                    (-1.0, -1.0)
                };
                if a < 0.0 || c < 0.0 {
                    // best single-term fit
                    let (a1, c1) = (r1 / s11, r2 / s22);
                    let l = |a: f64, c: f64| -> f64 {
                        (0..m)
                            .map(|k| {
                                let v = a * x1[k] + c * x2[k];
                                if v <= 0.0 {
                                    1e3
                                } else {
                                    (v / rows[k].2).ln().powi(2)
                                }
                            })
                            .sum()
                    };
                    if l(a1.max(0.0), 0.0) <= l(0.0, c1.max(0.0)) {
                        a = a1.max(0.0);
                        c = 0.0;
                    } else {
                        a = 0.0;
                        c = c1.max(0.0);
                    }
                }
                let mut loss = 0.0;
                for k in 0..m {
                    let v = a * x1[k] + c * x2[k];
                    loss += if v <= 0.0 { 1e3 } else { (v / rows[k].2).ln().powi(2) };
                }
                if loss < best.0 {
                    best = (loss, a, ps[ip], c, ss[is], rs[ir]);
                }
            }
        }
    }
    let rms = (best.0 / m as f64).sqrt();
    (best.1, best.2, best.3, best.4, best.5, rms)
}

/// Two-term fit e = A dt^2 + B dt (weights 1/e).
fn fit_two(rows: &[(f64, f64)]) -> (f64, f64) {
    let (mut s11, mut s12, mut s22, mut r1, mut r2) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for &(dt, e) in rows {
        let w2 = 1.0 / (e * e);
        let (x1, x2) = (dt * dt, dt);
        s11 += w2 * x1 * x1;
        s12 += w2 * x1 * x2;
        s22 += w2 * x2 * x2;
        r1 += w2 * x1 * e;
        r2 += w2 * x2 * e;
    }
    let det = s11 * s22 - s12 * s12;
    ((r1 * s22 - r2 * s12) / det, (s11 * r2 - s12 * r1) / det)
}

const DTS: [f64; 4] = [0.05, 0.025, 0.0125, 0.00625];

/// A1 decomposition on the given meshes; prints every error and the fits.
fn decomposition(label: &str, cfg: &Cfg, ns: &[usize]) {
    let mut rows = Vec::new();
    let mut spatial = Vec::new();
    for &n in ns {
        let refr = reference(cfg, n, DT_REF);
        let h = 2.0 * PI / n as f64;
        let es = exact_error(&refr, cfg.prob);
        spatial.push((h, es));
        let mut per = Vec::new();
        for &dt in &DTS {
            let a = run_seq(cfg, n, &const_seq(dt));
            let e = rel_l2(&a.u.values, &refr.u.values);
            println!(
                "DECOMP {label} N={n} dt={dt} e {e:.4e} (vs exact {:.4e}, p err {:.4e})",
                exact_error(&a, cfg.prob),
                pressure_error(&a, cfg.prob)
            );
            rows.push((h, dt, e));
            per.push((dt, e));
        }
        let (a2, b1) = fit_two(&per);
        let dt = 0.0125;
        println!(
            "DECOMP {label} N={n}: fine-pair order {:.3}; two-term fit A {a2:.4e} B {b1:.4e}, B dt share at dt=0.0125 {:.1} %; ref vs exact {es:.4e}",
            (per[1].1 / per[2].1).log2(),
            100.0 * b1 * dt / (a2 * dt * dt + b1 * dt)
        );
    }
    if ns.len() >= 3 {
        let (a, p, c, s, r, rms) = fit_m(&rows);
        let h32 = 2.0 * PI / 32.0;
        let (t1, t2) = (a * 0.0125f64.powf(p), c * h32.powf(s) * 0.0125f64.powf(r));
        let share = 100.0 * t2 / (t1 + t2);
        let present = share >= 30.0 && r <= 1.5;
        let ident = present && (s - 2.0).abs() <= 0.5 && (r - 1.0).abs() <= 0.35;
        println!(
            "DECOMP {label} FIT M: a {a:.4e} p {p:.2} c {c:.4e} s {s:.2} r {r:.2}; rms log misfit {rms:.3}; c-term share at (32, 0.0125) {share:.1} % -> floor {} {}",
            if present { "PRESENT" } else { "not present" },
            if ident { "(identified as O(dt h^2))" } else { "" }
        );
        // spatial error exponent
        let k = spatial.len() as f64;
        let (mx, my) = (
            spatial.iter().map(|x| x.0.ln()).sum::<f64>() / k,
            spatial.iter().map(|x| x.1.ln()).sum::<f64>() / k,
        );
        let q = spatial.iter().map(|x| (x.0.ln() - mx) * (x.1.ln() - my)).sum::<f64>()
            / spatial.iter().map(|x| (x.0.ln() - mx).powi(2)).sum::<f64>();
        println!("DECOMP {label} spatial error of the references: exponent q {q:.3}");
    }
}

// ---- A0 / A1 -------------------------------------------------------------------------------------------------------------------------
#[test]
#[ignore = "ADR 0073 A0 (prints)"]
fn a0_reference_adequacy() {
    for path in [Path::Piso, Path::Pimple4] {
        let cfg = Cfg::new(Prob::Tg, path);
        for n in [16usize, 32] {
            let r1 = reference(&cfg, n, DT_REF);
            let r2 = reference(&cfg, n, 1.0 / 2560.0);
            let a = run_seq(&cfg, n, &const_seq(0.0125));
            let (e1, e2) = (rel_l2(&a.u.values, &r1.u.values), rel_l2(&a.u.values, &r2.u.values));
            println!(
                "A0 {path:?} N={n}: e(0.0125) vs 1/1280 {e1:.4e}, vs 1/2560 {e2:.4e}, change {:.2} % ({}); refs differ by {:.3e}",
                100.0 * (e2 / e1 - 1.0),
                if (e2 / e1 - 1.0).abs() <= 0.05 { "adequate" } else { "NOT adequate" },
                rel_l2(&r1.u.values, &r2.u.values)
            );
        }
    }
}

#[test]
#[ignore = "ADR 0073 A1 (heavy, prints)"]
fn a1_decomposition_piso() {
    decomposition("A1-PISO", &Cfg::new(Prob::Tg, Path::Piso), &[16, 24, 32, 48, 64]);
}

#[test]
#[ignore = "ADR 0073 A1 (heavy, prints)"]
fn a1_decomposition_pimple4() {
    decomposition("A1-PIMPLE4", &Cfg::new(Prob::Tg, Path::Pimple4), &[16, 24, 32, 48, 64]);
}

#[test]
#[ignore = "ADR 0073 A1 (heavy, prints)"]
fn a1_decomposition_frac() {
    decomposition("A1-FRAC", &Cfg::new(Prob::Tg, Path::Frac), &[16, 24, 32, 48, 64]);
}

// ---- A3: F-G ------------------------------------------------------------------------------------------------------------------------
fn fg(prob: Prob, path: Path) -> Cfg {
    let mut c = Cfg::new(prob, path);
    c.fg = true;
    c
}

/// Orders of the pairs (dt, dt/2) for each dt in `pairs` against the same-scheme reference; prints them.
fn orders(id: &str, cfg: &Cfg, n: usize, pairs: &[f64], variable: bool) -> Vec<f64> {
    let refr = reference(cfg, n, DT_REF);
    let seq = |d: f64| if variable { var_seq(d) } else { const_seq(d) };
    pairs
        .iter()
        .map(|&dt| {
            let a = run_seq(cfg, n, &seq(dt));
            let b = run_seq(cfg, n, &seq(0.5 * dt));
            let e = [rel_l2(&a.u.values, &refr.u.values), rel_l2(&b.u.values, &refr.u.values)];
            let o = (e[0] / e[1]).log2();
            println!(
                "RESULT {id} {:?}/{:?} fg={} N={n} dt={dt}{}: e {:.4e} {:.4e} (vs exact {:.4e} {:.4e}) order {o:.3} {}",
                cfg.prob,
                cfg.path,
                cfg.fg,
                if variable { " variable" } else { "" },
                e[0],
                e[1],
                exact_error(&a, cfg.prob),
                exact_error(&b, cfg.prob),
                if (1.9..=2.3).contains(&o) { "MET" } else { "MISS" }
            );
            o
        })
        .collect()
}

fn band(id: &str, o: &[f64], bad: &mut Vec<String>) {
    for &x in o {
        if !(1.9..=2.3).contains(&x) {
            bad.push(format!("{id}: order {x:.3} outside [1.9, 2.3]"));
        }
    }
}

const FINE: f64 = 0.025;
const COARSE: f64 = 0.1;

/// Registered A3 acceptance of F-G (amendment 1), whole set. KNOWN MISS: C-O2 (PIMPLE-C, 1.237) and C-S1 (TG-S PISO, 0.995); limits unchanged.
#[test]
#[ignore = "known miss of ADR 0073 A3 with F-G (C-O2 1.237, C-S1 0.995; limits unchanged)"]
fn c_fg_orders_known_miss() {
    let mut bad = Vec::new();
    band(
        "C-O1",
        &orders("C-O1", &fg(Prob::Tg, Path::Piso), 32, &[FINE, COARSE], false),
        &mut bad,
    );
    band(
        "C-O2",
        &orders("C-O2", &fg(Prob::Tg, Path::PimpleC), 32, &[FINE], false),
        &mut bad,
    );
    band(
        "C-O3",
        &orders("C-O3", &fg(Prob::Tg, Path::Pimple4), 32, &[FINE], false),
        &mut bad,
    );
    band(
        "C-O7",
        &orders("C-O7", &fg(Prob::Tg, Path::Piso), 32, &[FINE], true),
        &mut bad,
    );
    band(
        "C-S1",
        &orders("C-S1", &fg(Prob::TgS, Path::Piso), 32, &[FINE], false),
        &mut bad,
    );
    band(
        "C-S2",
        &orders("C-S2", &fg(Prob::TgS, Path::Pimple4), 32, &[FINE], false),
        &mut bad,
    );
    // reported only: the default on TG-S, the coarse pair of PIMPLE-C and the MMS rows (C-O6r)
    orders("TG-S default", &Cfg::new(Prob::TgS, Path::Piso), 32, &[FINE], false);
    orders("TG-S default", &Cfg::new(Prob::TgS, Path::Pimple4), 32, &[FINE], false);
    orders("C-O2r", &fg(Prob::Tg, Path::PimpleC), 32, &[COARSE], false);
    for path in [Path::Piso, Path::PimpleC] {
        orders("C-O6r", &fg(Prob::Mms, path), 32, &[FINE, COARSE], false);
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn c_o9_fg_joint_refinement() {
    let cfg = fg(Prob::Tg, Path::Piso);
    let errs: Vec<f64> = [(16usize, 0.1), (32, 0.05), (64, 0.025)]
        .iter()
        .map(|&(n, dt)| exact_error(&run_seq(&cfg, n, &const_seq(dt)), Prob::Tg))
        .collect();
    let o = [(errs[0] / errs[1]).log2(), (errs[1] / errs[2]).log2()];
    println!(
        "RESULT C-O9 errors {:.4e} {:.4e} {:.4e} orders {:.3} {:.3} (limit >= 1.9)",
        errs[0], errs[1], errs[2], o[0], o[1]
    );
    assert!(o[0] >= 1.9 && o[1] >= 1.9, "{o:?}");
}

/// No-harm comparison against the default: velocity and pressure errors against the exact solution, factor limit 1.05.
fn no_harm(id: &str, on: &Cfg, off: &Cfg, bad: &mut Vec<String>) {
    for dt in [0.05, 0.025, 0.0125] {
        let a = run_seq(on, 32, &const_seq(dt));
        let b = run_seq(off, 32, &const_seq(dt));
        let (ua, ub) = (exact_error(&a, on.prob), exact_error(&b, on.prob));
        let line = format!(
            "{id} {:?}/{:?} dt={dt}: velocity error {ua:.4e} vs default {ub:.4e} (ratio {:.4})",
            on.prob,
            on.path,
            ua / ub
        );
        if ua > 1.05 * ub || ua.is_nan() {
            bad.push(line.clone());
        }
        if on.prob == Prob::Mode {
            println!("RESULT {line}");
            continue;
        }
        let (pa, pb) = (pressure_error(&a, on.prob), pressure_error(&b, on.prob));
        let pl = format!("; pressure error {pa:.4e} vs {pb:.4e} (ratio {:.4})", pa / pb);
        if pa > 1.05 * pb || pa.is_nan() {
            bad.push(format!("{line}{pl}"));
        }
        println!("RESULT {line}{pl}");
    }
}

/// KNOWN MISS of N1 with F-G: TG-S velocity error at dt 0.0125 is 1.0504 x the default (limit 1.05); the other 23 comparisons are met.
#[test]
#[ignore = "known miss of ADR 0073 N1 with F-G (TG-S dt 0.0125 ratio 1.0504 > 1.05; limit unchanged)"]
fn n1_fg_no_harm_known_miss() {
    let mut bad = Vec::new();
    for prob in [Prob::Tg, Prob::TgS, Prob::Mms, Prob::Mode] {
        no_harm("N1", &fg(prob, Path::Piso), &Cfg::new(prob, Path::Piso), &mut bad);
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
#[ignore = "ADR 0073 A3 N3 (heavy, prints)"]
fn n3_fg_decomposition() {
    for path in [Path::Piso, Path::Pimple4] {
        decomposition(&format!("N3-FG-{path:?}"), &fg(Prob::Tg, path), &[16, 32, 64]);
    }
}

/// The rows of the A3 set that F-G meets (C-O1 fine and coarse, C-O3, C-O7, C-S2), asserted separately (ADR 0073 addendum).
#[test]
#[ignore = "ADR 0073 A3 met rows of F-G (heavy)"]
fn c_fg_met_rows() {
    let mut bad = Vec::new();
    band(
        "C-O1",
        &orders("C-O1", &fg(Prob::Tg, Path::Piso), 32, &[FINE, COARSE], false),
        &mut bad,
    );
    band(
        "C-O3",
        &orders("C-O3", &fg(Prob::Tg, Path::Pimple4), 32, &[FINE], false),
        &mut bad,
    );
    band(
        "C-O7",
        &orders("C-O7", &fg(Prob::Tg, Path::Piso), 32, &[FINE], true),
        &mut bad,
    );
    band(
        "C-S2",
        &orders("C-S2", &fg(Prob::TgS, Path::Pimple4), 32, &[FINE], false),
        &mut bad,
    );
    assert!(bad.is_empty(), "{bad:#?}");
}
