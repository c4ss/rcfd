//! ADR 0073 (wave 34B) Part A: the Taylor-Green BDF2 order floor, registered decomposition A0/A1 (default code). The opt-in fixes of ADR 0073
//! (F-G `rhie_chow_consistent`) and ADR 0076 (F-S, F-R, F-G on rotational seams) were measured, not accepted and removed under the registered rule
//! (ADR 0076; the measured code and its tests are in the wave-35A record commit named there). Set-ups, tolerances, pairs and the error measure are those
//! of ADR 0069 (`w33b_transient_order.rs`). Heavy tests are ignored: `cargo test --release -p rcfd-fv --test w34b_floor -- --ignored --nocapture`.

use rcfd_core::vec3::Vec3;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::pimple::fractional::FractionalSettings;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::box_2d;
use std::f64::consts::PI;
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Path {
    Piso,
    Pimple4,
    Frac,
}

#[derive(Clone, Copy, Debug)]
struct Cfg {
    path: Path,
    scheme: TimeScheme,
}

impl Cfg {
    fn new(path: Path) -> Self {
        Cfg {
            path,
            scheme: TimeScheme::Bdf2,
        }
    }
}

const NU: f64 = 0.1;

fn exact_u(x: Vec3, t: f64) -> Vec3 {
    let d = (-2.0 * NU * t).exp();
    [x[0].sin() * x[1].cos() * d, -x[0].cos() * x[1].sin() * d, 0.0]
}

fn exact_p(x: Vec3, t: f64) -> f64 {
    0.25 * ((2.0 * x[0]).cos() + (2.0 * x[1]).cos()) * (-4.0 * NU * t).exp()
}

fn settings(cfg: &Cfg) -> PimpleSettings {
    let mut s = PimpleSettings::piso(NU);
    s.convection = ConvScheme::Central;
    s.time_scheme = cfg.scheme;
    match cfg.path {
        Path::Piso => {}
        Path::Pimple4 => s.n_outer = 4,
        Path::Frac => s.fractional = Some(FractionalSettings::default()),
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
    let l = 2.0 * PI;
    let mesh = Arc::new(box_2d(n, n, l, l, true, true).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let u = VectorField::from_fn("U", &geom, |c| exact_u(c, 0.0));
    let p = ScalarField::from_fn("p", &geom, |c| exact_p(c, 0.0));
    PimpleSolver::new(geom, u, p, st)
}

fn run_seq(cfg: &Cfg, n: usize, dts: &[f64]) -> PimpleSolver {
    let mut s = make(cfg, n);
    for &dt in dts {
        s.step(dt);
        assert!(s.diverged.is_none(), "{:?}: diverged", cfg);
    }
    s
}

fn const_seq(dt: f64) -> Vec<f64> {
    vec![dt; (1.0 / dt).round() as usize]
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

fn exact_error(s: &PimpleSolver) -> f64 {
    let ex: Vec<Vec3> = s.geom.mesh.cell_centres().iter().map(|&c| exact_u(c, s.time)).collect();
    rel_l2(&s.u.values, &ex)
}

/// Relative L2 of the cell pressure against the exact one, volume means removed from both.
fn pressure_error(s: &PimpleSolver) -> f64 {
    let vol = s.geom.mesh.cell_volumes();
    let tv: f64 = vol.iter().sum();
    let ex: Vec<f64> = s.geom.mesh.cell_centres().iter().map(|&c| exact_p(c, s.time)).collect();
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
        let es = exact_error(&refr);
        spatial.push((h, es));
        let mut per = Vec::new();
        for &dt in &DTS {
            let a = run_seq(cfg, n, &const_seq(dt));
            let e = rel_l2(&a.u.values, &refr.u.values);
            println!(
                "DECOMP {label} N={n} dt={dt} e {e:.4e} (vs exact {:.4e}, p err {:.4e})",
                exact_error(&a),
                pressure_error(&a)
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
        let cfg = Cfg::new(path);
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
    decomposition("A1-PISO", &Cfg::new(Path::Piso), &[16, 24, 32, 48, 64]);
}

#[test]
#[ignore = "ADR 0073 A1 (heavy, prints)"]
fn a1_decomposition_pimple4() {
    decomposition("A1-PIMPLE4", &Cfg::new(Path::Pimple4), &[16, 24, 32, 48, 64]);
}

#[test]
#[ignore = "ADR 0073 A1 (heavy, prints)"]
fn a1_decomposition_frac() {
    decomposition("A1-FRAC", &Cfg::new(Path::Frac), &[16, 24, 32, 48, 64]);
}
