//! Wave 31A (ADR 0062, item 2): diagnosis of the Ra = 1e6 differentially heated cavity at 32 x 32 (symmetric grading ratio 8),
//! where the steady SIMPLEC iteration of `validation_natural_convection.rs` stalls at residual 1.3e-2 after 40000 iterations.
//!
//! Prints only (no assertion on a result, no tuning): one factor at a time is changed on the baseline case and the residual
//! history, the final Nu_avg / v_max and, over the last 2000 iterations, their relative peak-to-peak amplitude are printed.
//! Ignored (diagnosis); run with `cargo test --release -p rcfd-fv --test diag_cavity_ra1e6 -- --ignored --nocapture <name>`.

use rcfd_fv::convection::{ConvScheme, TvdLimiter};
use rcfd_fv::energy::{BuoyantPimple, BuoyantSimple, EnergySolver};
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::simple::{Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField, ZeroGradient};
use rcfd_mesh::gen::{box_2d_graded, Grading};
use std::sync::Arc;

const PR: f64 = 0.71;
const RA: f64 = 1e6;
const CAP: usize = 40000;

#[derive(Clone, Copy)]
struct Opt {
    n: usize,
    ratio: f64,
    alpha_u: f64,
    alpha_p: Option<f64>,
    relax_t: f64,
    coupling: Coupling,
    mom: ConvScheme,
    en: ConvScheme,
}

impl Opt {
    fn base() -> Opt {
        Opt {
            n: 32,
            ratio: 8.0,
            alpha_u: 0.8,
            alpha_p: None,
            relax_t: 0.9,
            coupling: Coupling::Simplec,
            mom: ConvScheme::Tvd(TvdLimiter::VanLeer),
            en: ConvScheme::Tvd(TvdLimiter::VanLeer),
        }
    }
}

fn name(s: ConvScheme) -> String {
    match s {
        ConvScheme::Tvd(l) => format!("{l:?}"),
        o => format!("{o:?}"),
    }
}

fn describe(o: &Opt) -> String {
    format!(
        "n {} ratio {} au {} ap {:?} rt {} {:?} mom {} en {}",
        o.n,
        o.ratio,
        o.alpha_u,
        o.alpha_p,
        o.relax_t,
        o.coupling,
        name(o.mom),
        name(o.en)
    )
}

fn grading(ratio: f64) -> Grading {
    if ratio > 1.0 {
        Grading::Symmetric(ratio)
    } else {
        Grading::Uniform
    }
}

fn bcs(geom: &Arc<FvGeometry>) -> (VectorField, ScalarField, ScalarField) {
    let mut u = VectorField::uniform("U", geom, [0.0; 3]);
    for p in ["xmin", "xmax", "ymin", "ymax"] {
        u.set_bc(geom, p, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p = ScalarField::uniform("p", geom, 0.0);
    let mut t = ScalarField::uniform("T", geom, 0.5);
    t.set_bc(geom, "xmin", Box::new(FixedValue::scalar(1.0))).unwrap();
    t.set_bc(geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    t.set_bc(geom, "ymin", Box::new(ZeroGradient)).unwrap();
    t.set_bc(geom, "ymax", Box::new(ZeroGradient)).unwrap();
    (u, p, t)
}

fn make_geom(n: usize, ratio: f64) -> Arc<FvGeometry> {
    let g = grading(ratio);
    let mesh = Arc::new(box_2d_graded(n, n, 1.0, 1.0, [g, g], false, false).unwrap());
    Arc::new(FvGeometry::new(mesh).unwrap())
}

fn steady(o: &Opt, init: Init) -> BuoyantSimple {
    let geom = make_geom(o.n, o.ratio);
    let (u, p, mut t) = bcs(&geom);
    if matches!(init, Init::Conduction) {
        let cc = geom.mesh.cell_centres();
        for c in 0..cc.len() {
            t.values[c] = 1.0 - cc[c][0];
        }
    }
    let mut s = SimpleSettings::new(PR, o.coupling);
    s.convection = o.mom;
    s.alpha_u = o.alpha_u;
    if let Some(a) = o.alpha_p {
        s.alpha_p = a;
    }
    let flow = SimpleSolver::new(geom.clone(), u, p, s);
    let mut energy = EnergySolver::new(geom.clone(), t, 1.0);
    energy.convection = o.en;
    energy.relax = o.relax_t;
    BuoyantSimple {
        flow,
        energy,
        beta: 1.0,
        gravity: [0.0, -RA * PR, 0.0],
        t_ref: 0.5,
    }
}

#[derive(Clone, Copy)]
enum Init {
    Rest,
    Conduction,
}

fn cell_width(geom: &FvGeometry, c: usize, axis: usize) -> f64 {
    let mesh = &geom.mesh;
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for &f in mesh.cell_faces(c) {
        let x = mesh.face_centres()[f][axis];
        lo = lo.min(x);
        hi = hi.max(x);
    }
    hi - lo
}

/// Maximum over the vertical mid-plane of v (cell columns next to x = 0.5 averaged), by parabola as in the validation test.
fn v_max_of(geom: &FvGeometry, vel: &VectorField) -> f64 {
    let cc = geom.mesh.cell_centres();
    let mut v: Vec<(f64, f64)> = (0..cc.len())
        .filter(|&c| (cc[c][1] - 0.5).abs() <= 0.5 * cell_width(geom, c, 1) + 1e-12)
        .map(|c| (cc[c][0], vel.values[c][1]))
        .collect();
    v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    // even n: two cell rows straddle y = 0.5, average them as the validation test does; odd n: one row
    if v.len() % 2 == 0 && v.len() > 1 && (v[1].0 - v[0].0).abs() < 1e-12 {
        v = v.chunks(2).map(|w| (w[0].0, 0.5 * (w[0].1 + w[1].1))).collect();
    }
    let i = (0..v.len())
        .max_by(|&a, &b| v[a].1.partial_cmp(&v[b].1).unwrap())
        .unwrap();
    if i == 0 || i + 1 == v.len() {
        return v[i].1;
    }
    let ((x0, y0), (x1, y1), (x2, y2)) = (v[i - 1], v[i], v[i + 1]);
    let d0 = (x0 - x1) * (x0 - x2);
    let d1 = (x1 - x0) * (x1 - x2);
    let d2 = (x2 - x0) * (x2 - x1);
    let a = y0 / d0 + y1 / d1 + y2 / d2;
    let b = -y0 * (x1 + x2) / d0 - y1 * (x0 + x2) / d1 - y2 * (x0 + x1) / d2;
    let c = y0 * x1 * x2 / d0 + y1 * x0 * x2 / d1 + y2 * x0 * x1 / d2;
    if a >= 0.0 {
        return y1;
    }
    let xv = -b / (2.0 * a);
    if xv < x0 || xv > x2 {
        return y1;
    }
    a * xv * xv + b * xv + c
}

fn nu_avg_of(geom: &FvGeometry, energy: &EnergySolver) -> f64 {
    let gr = energy.wall_gradient("xmin");
    let pt = geom.mesh.patch("xmin").unwrap();
    let area: f64 = pt.faces().map(|f| geom.mesh.face_area(f)).sum();
    pt.faces()
        .zip(&gr)
        .map(|(f, g)| g * geom.mesh.face_area(f))
        .sum::<f64>()
        / area
}

struct Trace {
    iterations: usize,
    last: f64,
    min: f64,
    /// residual range over the last 2000 iterations
    res_lo: f64,
    res_hi: f64,
    nu: f64,
    vmax: f64,
    /// relative peak-to-peak of Nu_avg and v_max over the last 2000 iterations
    nu_amp: f64,
    v_amp: f64,
}

fn iterate(sim: &mut BuoyantSimple, max_iter: usize) -> Trace {
    let geom = sim.flow.geom.clone();
    let (mut last, mut min, mut it) = (f64::INFINITY, f64::INFINITY, 0usize);
    let (mut res_lo, mut res_hi) = (f64::INFINITY, 0.0f64);
    let (mut nlo, mut nhi, mut vlo, mut vhi) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
    while it < max_iter {
        let info = sim.iterate();
        it += 1;
        last = info.flow.res_u.max(info.res_t);
        min = min.min(last);
        if last < 1e-9 {
            break;
        }
        if it + 2000 >= max_iter {
            res_lo = res_lo.min(last);
            res_hi = res_hi.max(last);
            let (nu, v) = (nu_avg_of(&geom, &sim.energy), v_max_of(&geom, &sim.flow.u));
            nlo = nlo.min(nu);
            nhi = nhi.max(nu);
            vlo = vlo.min(v);
            vhi = vhi.max(v);
        }
    }
    let nu = nu_avg_of(&geom, &sim.energy);
    let vmax = v_max_of(&geom, &sim.flow.u);
    let amp = |lo: f64, hi: f64, ref_: f64| if hi >= lo { (hi - lo) / ref_.abs() } else { 0.0 };
    Trace {
        iterations: it,
        last,
        min,
        res_lo,
        res_hi,
        nu,
        vmax,
        nu_amp: amp(nlo, nhi, nu),
        v_amp: amp(vlo, vhi, vmax),
    }
}

fn report(label: &str, o: &Opt, t: &Trace) {
    println!(
        "{label}: [{}] -> {} its, res {:.2e} (min {:.2e}, last-2000 range {:.1e}..{:.1e}), Nu {:.5}, v_max {:.4}, amp(Nu) {:.2e}, amp(v_max) {:.2e}{}",
        describe(o),
        t.iterations,
        t.last,
        t.min,
        t.res_lo,
        t.res_hi,
        t.nu,
        t.vmax,
        t.nu_amp,
        t.v_amp,
        if t.last < 1e-9 { "  CONVERGED" } else { "" }
    );
}

fn go(label: &str, o: Opt) -> Trace {
    let mut sim = steady(&o, Init::Rest);
    let t0 = std::time::Instant::now();
    let tr = iterate(&mut sim, CAP);
    report(label, &o, &tr);
    println!("   ({:.0} s)", t0.elapsed().as_secs_f64());
    tr
}

/// D0 baseline, D1 under-relaxation.
#[test]
#[ignore = "diagnosis, prints only"]
fn d0_d1_baseline_and_relaxation() {
    go("D0", Opt::base());
    for au in [0.5, 0.3] {
        go(
            "D1",
            Opt {
                alpha_u: au,
                ..Opt::base()
            },
        );
    }
    for rt in [0.7, 0.5] {
        go(
            "D1",
            Opt {
                relax_t: rt,
                ..Opt::base()
            },
        );
    }
    go(
        "D1",
        Opt {
            alpha_u: 0.5,
            relax_t: 0.5,
            ..Opt::base()
        },
    );
}

/// D2 convection scheme (together, then each alone).
#[test]
#[ignore = "diagnosis, prints only"]
fn d2_convection_schemes() {
    let tvd = |l| ConvScheme::Tvd(l);
    let schemes = [
        ConvScheme::Upwind,
        ConvScheme::LinearUpwind,
        tvd(TvdLimiter::Minmod),
        tvd(TvdLimiter::Koren),
        tvd(TvdLimiter::VanAlbada),
        tvd(TvdLimiter::Muscl),
    ];
    for s in schemes {
        go(
            "D2 both",
            Opt {
                mom: s,
                en: s,
                ..Opt::base()
            },
        );
    }
    for s in schemes {
        go("D2 momentum only", Opt { mom: s, ..Opt::base() });
        go("D2 energy only", Opt { en: s, ..Opt::base() });
    }
}

/// D3 coupling variant: classic SIMPLE.
#[test]
#[ignore = "diagnosis, prints only"]
fn d3_coupling() {
    go(
        "D3",
        Opt {
            coupling: Coupling::Simple,
            alpha_u: 0.7,
            alpha_p: Some(0.3),
            ..Opt::base()
        },
    );
    go(
        "D3",
        Opt {
            coupling: Coupling::Simple,
            alpha_u: 0.5,
            alpha_p: Some(0.2),
            ..Opt::base()
        },
    );
}

/// D5 initial condition and restart: conduction profile, and 2 x 40000 iterations of the baseline (restart of the stalled state).
#[test]
#[ignore = "diagnosis, prints only"]
fn d5_initial_condition_and_restart() {
    let o = Opt::base();
    let mut sim = steady(&o, Init::Conduction);
    let tr = iterate(&mut sim, CAP);
    report("D5 conduction start", &o, &tr);
    let mut sim = steady(&o, Init::Rest);
    let tr = iterate(&mut sim, CAP);
    report("D5 rest start, first 40000", &o, &tr);
    let tr = iterate(&mut sim, CAP);
    report("D5 restart of the stalled state, second 40000", &o, &tr);
}

/// D6 grading and mesh size.
#[test]
#[ignore = "diagnosis, prints only"]
fn d6_mesh_grading_and_size() {
    for ratio in [2.0, 4.0, 6.0, 8.0, 10.0] {
        go("D6 grading", Opt { ratio, ..Opt::base() });
    }
    for n in [30, 31, 33, 36, 40] {
        go("D6 size", Opt { n, ..Opt::base() });
    }
}

struct Transient {
    sim: BuoyantPimple,
    geom: Arc<FvGeometry>,
}

fn transient(o: &Opt) -> Transient {
    let geom = make_geom(o.n, o.ratio);
    let (u, p, t) = bcs(&geom);
    let mut s = PimpleSettings::piso(PR);
    s.convection = o.mom;
    s.n_correctors = 2;
    s.amg_pressure = true;
    let flow = PimpleSolver::new(geom.clone(), u, p, s);
    let mut energy = EnergySolver::new(geom.clone(), t, 1.0);
    energy.convection = o.en;
    energy.solver.tol_rel = 1e-10;
    energy.solver.tol_abs = 1e-300;
    energy.solver.max_iter = 2000;
    Transient {
        sim: BuoyantPimple {
            flow,
            energy,
            beta: 1.0,
            gravity: [0.0, -RA * PR, 0.0],
            t_ref: 0.5,
        },
        geom,
    }
}

/// D4 steady versus transient: PISO from rest to t_end at Courant numbers 0.5 and 2 (on the scale v_max = 220 and the smallest cell).
/// Prints the rate of change (max over cells of |dT|/dt and |du|/(dt v_max)) averaged as a maximum over each 0.05 window, v_max and Nu at the end,
/// and then the steady iteration started from the final transient state (D5).
#[test]
#[ignore = "diagnosis, prints only"]
fn d4_transient_march_and_steady_restart() {
    for courant in [0.5, 2.0] {
        let o = Opt::base();
        let mut tr = transient(&o);
        let cc = tr.geom.mesh.cell_centres();
        let hmin = (0..cc.len())
            .map(|c| cell_width(&tr.geom, c, 0).min(cell_width(&tr.geom, c, 1)))
            .fold(f64::INFINITY, f64::min);
        let dt = courant * hmin / 220.0;
        let t_end = 0.5;
        println!("D4 courant {courant}: hmin {hmin:.4e}, dt {dt:.3e}, t_end {t_end}");
        let (mut t_win, mut win_max_t, mut win_max_u, mut t_now) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
        let t0 = std::time::Instant::now();
        let mut next_print = 0.05;
        while t_now < t_end - 1e-12 {
            let told = tr.sim.energy.t.values.clone();
            let uold = tr.sim.flow.u.values.clone();
            tr.sim.step(dt);
            t_now += dt;
            t_win += dt;
            let dtm = told
                .iter()
                .zip(&tr.sim.energy.t.values)
                .map(|(a, b)| (a - b).abs())
                .fold(0.0, f64::max)
                / dt;
            let dum = uold
                .iter()
                .zip(&tr.sim.flow.u.values)
                .map(|(a, b)| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt())
                .fold(0.0, f64::max)
                / dt
                / 220.0;
            win_max_t = win_max_t.max(dtm);
            win_max_u = win_max_u.max(dum);
            if t_now >= next_print - 1e-12 {
                println!(
                    "   t {:.3}: max|dT/dt| {:.2e}, max|du/dt|/vmax {:.2e}, v_max {:.4}, Nu {:.5} ({:.0} s)",
                    t_now,
                    win_max_t,
                    win_max_u,
                    v_max_of(&tr.geom, &tr.sim.flow.u),
                    nu_avg_of(&tr.geom, &tr.sim.energy),
                    t0.elapsed().as_secs_f64()
                );
                next_print += 0.05;
                t_win = 0.0;
                win_max_t = 0.0;
                win_max_u = 0.0;
            }
        }
        let _ = t_win;
        // D5: the steady iteration from the transient state
        let mut sim = steady(&o, Init::Rest);
        sim.flow.u.values.clone_from(&tr.sim.flow.u.values);
        sim.flow.p.values.clone_from(&tr.sim.flow.p.values);
        sim.flow.phi.clone_from(&tr.sim.flow.phi);
        sim.energy.t.values.clone_from(&tr.sim.energy.t.values);
        let r = iterate(&mut sim, CAP);
        report(
            &format!("D5 steady iteration from the transient state (courant {courant})"),
            &o,
            &r,
        );
    }
}

/// The mid-plane profile of v along x (y = 0.5), rows averaged for even n.
fn v_profile(geom: &FvGeometry, vel: &VectorField) -> Vec<(f64, f64)> {
    let cc = geom.mesh.cell_centres();
    let mut v: Vec<(f64, f64)> = (0..cc.len())
        .filter(|&c| (cc[c][1] - 0.5).abs() <= 0.5 * cell_width(geom, c, 1) + 1e-12)
        .map(|c| (cc[c][0], vel.values[c][1]))
        .collect();
    v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    if v.len() % 2 == 0 && v.len() > 1 && (v[1].0 - v[0].0).abs() < 1e-12 {
        v = v.chunks(2).map(|w| (w[0].0, 0.5 * (w[0].1 + w[1].1))).collect();
    }
    v
}

/// Maximum and its location of the Lagrange cubic through the four points around the largest value.
fn cubic_max(p: &[(f64, f64)]) -> (f64, f64) {
    let i = (0..p.len())
        .max_by(|&a, &b| p[a].1.partial_cmp(&p[b].1).unwrap())
        .unwrap();
    let lo = if i >= 2 && (i + 1 >= p.len() || p[i - 2].1 > p[i + 1].1) {
        i - 2
    } else {
        i - 1
    };
    let q = &p[lo..lo + 4];
    let eval = |x: f64| -> f64 {
        (0..4)
            .map(|a| {
                let mut w = q[a].1;
                for b in 0..4 {
                    if b != a {
                        w *= (x - q[b].0) / (q[a].0 - q[b].0);
                    }
                }
                w
            })
            .sum()
    };
    let (x0, x1) = (q[0].0, q[3].0);
    let (mut bx, mut bv) = (p[i].0, p[i].1);
    for k in 0..=20000 {
        let x = x0 + (x1 - x0) * k as f64 / 20000.0;
        let v = eval(x);
        if v > bv {
            bx = x;
            bv = v;
        }
    }
    (bx, bv)
}

/// D7 (ADR 0062 addendum 1): converged sequence at alpha_u = 0.5, v_max by three extractions.
#[test]
#[ignore = "diagnosis, prints only"]
fn d7_vmax_sequence_and_extraction() {
    for n in [32usize, 48, 64, 96, 128] {
        let o = Opt {
            n,
            alpha_u: 0.5,
            ..Opt::base()
        };
        let mut sim = steady(&o, Init::Rest);
        let t0 = std::time::Instant::now();
        let tr = iterate(&mut sim, CAP);
        let prof = v_profile(&sim.flow.geom, &sim.flow.u);
        let raw = prof.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
        let (xc, vc) = cubic_max(&prof);
        println!(
            "D7 n {n}: {} its res {:.2e} | Nu {:.5} | v_max parabola {:.4}, raw cell max {:.4}, cubic {:.4} at x {:.4} ({:.0} s)",
            tr.iterations,
            tr.last,
            tr.nu,
            tr.vmax,
            raw,
            vc,
            xc,
            t0.elapsed().as_secs_f64()
        );
    }
}
