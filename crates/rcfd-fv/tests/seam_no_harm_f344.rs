//! ADR 0245 amendment 3 (F-344): ADR 0076's periodic-seam no-harm comparisons with the opt-in
//! `PimpleSettings::rhie_chow_step_consistent` ("C") against the default: the TG-S rows of N1 (ADR 0073's TG-S, copied from
//! `consistent_benchmarks_f301.rs`), R4 on SWIRL and SECT-M and R2b on SECT-M (the sector set-ups copied from `w35a_seam.rs`),
//! and the PIMPLE-4 rows of ADR 0080 B-R4. The limits are ADR 0076's (1.05 for the no-harm ratios, [1.9, 2.3] for R2b), fixed
//! in amendment 3 before any run. `cargo test --release -p rcfd-fv --test seam_no_harm_f344 -- --nocapture --test-threads 1`.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{annulus_sector, box_2d, Grading};
use std::f64::consts::PI;
use std::sync::{Arc, OnceLock};

// ---- SWIRL and SECT-M (copied from w35a_seam.rs) ----------------------------------------------------------------------------------
const RA: f64 = 0.5;
const RB: f64 = 1.0;
const NU: f64 = 0.02;

/// Radial eigenfunction U'' + U'/r - U/r^2 + k^2 U = 0, U(RA) = U(RB) = 0, by RK4 shooting; tabulated (r, U, U', P) with P' = U^2/r.
struct Eigen {
    k: f64,
    m: usize,
    h: f64,
    tab: Vec<[f64; 3]>, // U, U', P
    scale: f64,
}

fn rhs(r: f64, y: [f64; 3], k: f64) -> [f64; 3] {
    [y[1], -y[1] / r + y[0] / (r * r) - k * k * y[0], y[0] * y[0] / r]
}

fn integrate(k: f64, m: usize) -> Vec<[f64; 3]> {
    let h = (RB - RA) / m as f64;
    let mut y = [0.0, 1.0, 0.0];
    let mut out = Vec::with_capacity(m + 1);
    out.push(y);
    for i in 0..m {
        let r = RA + h * i as f64;
        let k1 = rhs(r, y, k);
        let a = |s: f64, kk: [f64; 3]| [y[0] + s * kk[0], y[1] + s * kk[1], y[2] + s * kk[2]];
        let k2 = rhs(r + 0.5 * h, a(0.5 * h, k1), k);
        let k3 = rhs(r + 0.5 * h, a(0.5 * h, k2), k);
        let k4 = rhs(r + h, a(h, k3), k);
        for j in 0..3 {
            y[j] += h / 6.0 * (k1[j] + 2.0 * k2[j] + 2.0 * k3[j] + k4[j]);
        }
        out.push(y);
    }
    out
}

impl Eigen {
    fn new(m: usize) -> Self {
        let end = |k: f64| integrate(k, m)[m][0];
        // first root of U(RB; k) above k = 1 by bisection
        let (mut lo, mut hi) = (1.0, 1.0);
        let f_lo = end(lo);
        loop {
            hi += 0.5;
            if end(hi).signum() != f_lo.signum() {
                break;
            }
            lo = hi;
        }
        for _ in 0..200 {
            let mid = 0.5 * (lo + hi);
            if end(mid).signum() == end(lo).signum() {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let k = 0.5 * (lo + hi);
        let tab = integrate(k, m);
        let scale = 1.0 / tab.iter().map(|y| y[0].abs()).fold(0.0, f64::max);
        Eigen {
            k,
            m,
            h: (RB - RA) / m as f64,
            tab,
            scale,
        }
    }
    /// (U, P) at r by cubic Hermite interpolation (U with U', P with U^2/r), scaled to max |U| = 1.
    fn at(&self, r: f64) -> (f64, f64) {
        let x = ((r - RA) / self.h).clamp(0.0, self.m as f64);
        let i = (x.floor() as usize).min(self.m - 1);
        let t = x - i as f64;
        let (y0, y1) = (self.tab[i], self.tab[i + 1]);
        let (r0, r1) = (RA + self.h * i as f64, RA + self.h * (i + 1) as f64);
        let herm = |p0: f64, d0: f64, p1: f64, d1: f64| {
            let (t2, t3) = (t * t, t * t * t);
            (2.0 * t3 - 3.0 * t2 + 1.0) * p0
                + (t3 - 2.0 * t2 + t) * self.h * d0
                + (-2.0 * t3 + 3.0 * t2) * p1
                + (t3 - t2) * self.h * d1
        };
        let u = herm(y0[0], y0[1], y1[0], y1[1]);
        let p = herm(y0[2], y0[0] * y0[0] / r0, y1[2], y1[0] * y1[0] / r1);
        (u * self.scale, p * self.scale * self.scale)
    }
}

fn exact_u(e: &Eigen, x: Vec3, t: f64) -> Vec3 {
    let r = x[0].hypot(x[1]);
    let (u, _) = e.at(r);
    let d = (-NU * e.k * e.k * t).exp();
    [-u * x[1] / r * d, u * x[0] / r * d, 0.0]
}

fn exact_p(e: &Eigen, x: Vec3, t: f64) -> f64 {
    let r = x[0].hypot(x[1]);
    e.at(r).1 * (-2.0 * NU * e.k * e.k * t).exp()
}

fn rel_l2(a: &[Vec3], b: &[Vec3]) -> f64 {
    let (mut n, mut d) = (0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        n += (x[0] - y[0]).powi(2) + (x[1] - y[1]).powi(2);
        d += y[0] * y[0] + y[1] * y[1];
    }
    (n / d).sqrt()
}

// ---- manufactured non-axisymmetric flow on the sector: psi = g(t) F(r) cos(6 theta), no-slip walls -----------------------------
fn gm(t: f64) -> f64 {
    1.0 + 0.5 * (2.0 * t + 0.3).sin()
}
fn gmd(t: f64) -> f64 {
    (2.0 * t + 0.3).cos()
}
const MM: f64 = 6.0;
fn mms_u(x: Vec3, t: f64) -> Vec3 {
    let r = x[0].hypot(x[1]);
    let th = x[1].atan2(x[0]);
    let (a, b) = (r - RA, RB - r);
    let f = 32.0 * a * a * b * b;
    let fp = 32.0 * (2.0 * a * b * b - 2.0 * a * a * b);
    let g = gm(t);
    let ur = -g * f * MM * (MM * th).sin() / r;
    let ut = -g * fp * (MM * th).cos();
    let (c, s) = (th.cos(), th.sin());
    [ur * c - ut * s, ur * s + ut * c, 0.0]
}
fn mms_p(x: Vec3, t: f64) -> f64 {
    let th = x[1].atan2(x[0]);
    0.25 * gm(t).powi(2) * (MM * th).cos()
}
fn d1(f: &dyn Fn(Vec3) -> f64, x: Vec3, k: usize, h: f64) -> f64 {
    let at = |s: f64| {
        let mut y = x;
        y[k] += s * h;
        f(y)
    };
    (8.0 * (at(1.0) - at(-1.0)) - (at(2.0) - at(-2.0))) / (12.0 * h)
}
fn d2(f: &dyn Fn(Vec3) -> f64, x: Vec3, k: usize, h: f64) -> f64 {
    let at = |s: f64| {
        let mut y = x;
        y[k] += s * h;
        f(y)
    };
    (-at(2.0) + 16.0 * at(1.0) - 30.0 * at(0.0) + 16.0 * at(-1.0) - at(-2.0)) / (12.0 * h * h)
}
fn mms_grad_p(x: Vec3, t: f64) -> Vec3 {
    let p = |y: Vec3| mms_p(y, t);
    [d1(&p, x, 0, 1e-4), d1(&p, x, 1, 1e-4), 0.0]
}
fn mms_source(x: Vec3, t: f64) -> Vec3 {
    let u = mms_u(x, t);
    let gp = mms_grad_p(x, t);
    let rate = gmd(t) / gm(t);
    [0, 1]
        .map(|i| {
            let ui = |y: Vec3| mms_u(y, t)[i];
            let conv = u[0] * d1(&ui, x, 0, 1e-4) + u[1] * d1(&ui, x, 1, 1e-4);
            let lap = d2(&ui, x, 0, 1e-3) + d2(&ui, x, 1, 1e-3);
            rate * u[i] + conv + gp[i] - NU * lap
        })
        .into_iter()
        .chain(std::iter::once(0.0))
        .collect::<Vec<_>>()
        .try_into()
        .unwrap()
}
fn mms_force(geom: &FvGeometry, t: f64) -> rcfd_fv::simple::BodyForce {
    let mesh = &geom.mesh;
    let (cc, fc, sf) = (mesh.cell_centres(), mesh.face_centres(), mesh.face_area_vectors());
    let cell: Vec<Vec3> = cc.iter().map(|&c| mms_source(c, t)).collect();
    let face: Vec<f64> = (0..mesh.n_faces())
        .map(|f| {
            if geom.empty[f] {
                0.0
            } else if f < geom.n_internal || geom.coupled_face[f - geom.n_internal] {
                vec3::dot(mms_source(fc[f], t), sf[f])
            } else {
                vec3::dot(mms_grad_p(fc[f], t), sf[f])
            }
        })
        .collect();
    rcfd_fv::simple::BodyForce { cell, face }
}

// ---- the sector harness (copied from w35a_seam.rs; the opt-in `c` added) ---------------------------------------------------------
#[derive(Clone, Copy, PartialEq, Debug)]
enum Prob {
    Swirl,
    SectM,
}

fn eigen() -> &'static Eigen {
    static E: OnceLock<Eigen> = OnceLock::new();
    E.get_or_init(|| Eigen::new(40000))
}

fn u_ex(prob: Prob, x: Vec3, t: f64) -> Vec3 {
    match prob {
        Prob::Swirl => exact_u(eigen(), x, t),
        Prob::SectM => mms_u(x, t),
    }
}

fn p_ex(prob: Prob, x: Vec3, t: f64) -> f64 {
    match prob {
        Prob::Swirl => exact_p(eigen(), x, t),
        Prob::SectM => mms_p(x, t),
    }
}

/// `outer` = 1 (PISO) or 4 (PIMPLE-4, alpha 1); `c` = `rhie_chow_step_consistent`.
fn make(prob: Prob, n: usize, outer: usize, c: bool) -> PimpleSolver {
    let mesh = annulus_sector(n, n, RA, RB, std::f64::consts::FRAC_PI_3, Grading::Uniform, 0.1).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::from_fn("U", &geom, |c| u_ex(prob, c, 0.0));
    u.set_bc(&geom, "inner", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    u.set_bc(&geom, "outer", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    let p = ScalarField::from_fn("p", &geom, |c| p_ex(prob, c, 0.0));
    let mut s = PimpleSettings::piso(NU);
    s.convection = ConvScheme::Central;
    s.time_scheme = TimeScheme::Bdf2;
    s.n_outer = outer;
    let ctl = SolveControl {
        tol_abs: 1e-14,
        tol_rel: 1e-12,
        max_iter: 5000,
    };
    s.momentum_solver = ctl;
    s.pressure_solver = ctl;
    s.rhie_chow_step_consistent = c;
    PimpleSolver::new(geom, u, p, s)
}

/// `bp` switches on `coupled_body_pair_mean` (ADR 0080).
fn run(prob: Prob, n: usize, dt: f64, outer: usize, c: bool, bp: bool) -> PimpleSolver {
    let mut s = make(prob, n, outer, c);
    s.settings.coupled_body_pair_mean = bp;
    for _ in 0..(1.0 / dt).round() as usize {
        if prob == Prob::SectM {
            let bf = mms_force(&s.geom, s.time + dt);
            let mut hook = move |_u: &VectorField, _phi: &[f64]| rcfd_fv::pimple::OuterUpdate {
                source_force: Some(bf.clone()),
                ..Default::default()
            };
            s.step_with(dt, Some(&mut hook as &mut rcfd_fv::pimple::OuterHook));
        } else {
            s.step(dt);
        }
        assert!(s.diverged.is_none(), "{prob:?} dt {dt} c {c}: diverged");
    }
    s
}

fn exact_err(prob: Prob, s: &PimpleSolver) -> f64 {
    let ex: Vec<Vec3> = s
        .geom
        .mesh
        .cell_centres()
        .iter()
        .map(|&c| u_ex(prob, c, s.time))
        .collect();
    rel_l2(&s.u.values, &ex)
}

/// Relative L2 of the cell pressure against the exact one, volume means removed from both.
fn pressure_err(prob: Prob, s: &PimpleSolver) -> f64 {
    let vol = s.geom.mesh.cell_volumes();
    let tv: f64 = vol.iter().sum();
    let ex: Vec<f64> = s
        .geom
        .mesh
        .cell_centres()
        .iter()
        .map(|&c| p_ex(prob, c, s.time))
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

const DT_REF: f64 = 1.0 / 1280.0;

// ---- TG-S (copied from consistent_benchmarks_f301.rs: ADR 0073's TG-S, PISO, N = 32) ----------------------------------------------
const NU_TG: f64 = 0.1;
const SHIFT: f64 = 0.6;

fn tgs_u(x: Vec3, t: f64) -> Vec3 {
    let d = (-2.0 * NU_TG * t).exp();
    let (a, b) = (x[0] + SHIFT, x[1] + SHIFT);
    [a.sin() * b.cos() * d, -a.cos() * b.sin() * d, 0.0]
}

fn tgs_p(x: Vec3, t: f64) -> f64 {
    0.25 * ((2.0 * (x[0] + SHIFT)).cos() + (2.0 * (x[1] + SHIFT)).cos()) * (-4.0 * NU_TG * t).exp()
}

fn tgs_run(dt: f64, c: bool) -> PimpleSolver {
    let mut st = PimpleSettings::piso(NU_TG);
    st.convection = ConvScheme::Central;
    st.time_scheme = TimeScheme::Bdf2;
    let ctl = SolveControl {
        tol_abs: 1e-14,
        tol_rel: 1e-12,
        max_iter: 5000,
    };
    st.momentum_solver = ctl;
    st.pressure_solver = ctl;
    st.rhie_chow_step_consistent = c;
    let l = 2.0 * PI;
    let mesh = Arc::new(box_2d(32, 32, l, l, true, true).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let u = VectorField::from_fn("U", &geom, |x| tgs_u(x, 0.0));
    let p = ScalarField::from_fn("p", &geom, |x| tgs_p(x, 0.0));
    let mut s = PimpleSolver::new(geom, u, p, st);
    for _ in 0..(1.0 / dt).round() as usize {
        s.step(dt);
        assert!(s.diverged.is_none(), "TG-S dt {dt} c {c}: diverged");
    }
    s
}

fn tgs_exact_error(s: &PimpleSolver) -> f64 {
    let ex: Vec<Vec3> = s.geom.mesh.cell_centres().iter().map(|&x| tgs_u(x, s.time)).collect();
    rel_l2(&s.u.values, &ex)
}

/// ADR 0073 N1's pressure error: relative L2 of the cell pressure against the exact one, both with their volume means removed.
fn tgs_pressure_error(s: &PimpleSolver) -> f64 {
    let vol = s.geom.mesh.cell_volumes();
    let tv: f64 = vol.iter().sum();
    let ex: Vec<f64> = s.geom.mesh.cell_centres().iter().map(|&x| tgs_p(x, s.time)).collect();
    let mean = |v: &[f64]| v.iter().zip(vol).map(|(a, w)| a * w).sum::<f64>() / tv;
    let (mp, me) = (mean(&s.p.values), mean(&ex));
    let (mut num, mut den) = (0.0, 0.0);
    for ((p, e), w) in s.p.values.iter().zip(&ex).zip(vol) {
        num += w * ((p - mp) - (e - me)).powi(2);
        den += w * (e - me).powi(2);
    }
    (num / den).sqrt()
}

// ---- the comparisons of amendment 3 --------------------------------------------------------------------------------------------

/// The largest difference of the cell velocities of two runs on the same mesh, over the cells next to a coupled face and over
/// the others (printed, no limit).
fn seam_split(a: &PimpleSolver, b: &PimpleSolver) -> (f64, f64) {
    let g = &a.geom;
    let mut near = vec![false; g.n_cells()];
    for (i, &c) in g.coupled_face.iter().enumerate() {
        if c {
            near[g.mesh.owner()[g.n_internal + i]] = true;
        }
    }
    let (mut dn, mut di) = (0.0f64, 0.0f64);
    for (c, (x, y)) in a.u.values.iter().zip(&b.u.values).enumerate() {
        let d = (x[0] - y[0]).abs().max((x[1] - y[1]).abs());
        if near[c] {
            dn = dn.max(d);
        } else {
            di = di.max(d);
        }
    }
    (dn, di)
}

/// One no-harm row: (label, dt, [velocity default, C], [pressure default, C]).
type Row = (String, f64, [f64; 2], [f64; 2]);

#[allow(clippy::neg_cmp_op_on_partial_ord)]
fn ratio_misses(rows: &[Row]) -> Vec<String> {
    let mut bad = Vec::new();
    for (label, dt, v, p) in rows {
        let (rv, rp) = (v[1] / v[0], p[1] / p[0]);
        if !(rv <= 1.05) {
            bad.push(format!("{label} dt={dt} velocity {rv:.4}"));
        }
        if !(rp <= 1.05) {
            bad.push(format!("{label} dt={dt} pressure {rp:.4}"));
        }
    }
    bad
}

fn print_row(id: &str, r: &Row, split: (f64, f64)) {
    let (label, dt, v, p) = r;
    println!(
        "RESULT {id} {label} dt={dt}: velocity default {:.4e} C {:.4e} (ratio {:.4}); pressure default {:.4e} C {:.4e} (ratio {:.4}); \
         largest change of u by C: next to a seam {:.3e}, elsewhere {:.3e}",
        v[0],
        v[1],
        v[1] / v[0],
        p[0],
        p[1],
        p[1] / p[0],
        split.0,
        split.1
    );
}

/// S3 and N1s: TG-S PISO at dt 0.05, 0.025, 0.0125, default and C.
fn tgs_rows() -> &'static Vec<Row> {
    static R: OnceLock<Vec<Row>> = OnceLock::new();
    R.get_or_init(|| {
        [0.05, 0.025, 0.0125]
            .into_iter()
            .map(|dt| {
                let (d, c) = (tgs_run(dt, false), tgs_run(dt, true));
                let r = (
                    "TG-S PISO".to_string(),
                    dt,
                    [tgs_exact_error(&d), tgs_exact_error(&c)],
                    [tgs_pressure_error(&d), tgs_pressure_error(&c)],
                );
                print_row("N1s", &r, seam_split(&d, &c));
                r
            })
            .collect()
    })
}

/// ADR 0245 amendment 3 S3 (set-up): the default TG-S errors at dt 0.05 are ADR 0076's.
#[test]
fn s3_tg_s_default_is_that_of_adr_0076() {
    let (_, _, v, p) = &tgs_rows()[0];
    println!(
        "S3: velocity {:.4e} (limit [2.09e-3, 2.11e-3]), pressure {:.4e} (limit [8.20e-3, 8.23e-3])",
        v[0], p[0]
    );
    assert!((2.09e-3..=2.11e-3).contains(&v[0]), "S3 velocity {:e}", v[0]);
    assert!((8.20e-3..=8.23e-3).contains(&p[0]), "S3 pressure {:e}", p[0]);
}

/// ADR 0245 amendment 3 N1s (ADR 0076 N1, the TG-S rows): with the opt-in the exact velocity and pressure errors are at most
/// 1.05 times the default's.
#[test]
fn n1s_tg_s_no_harm_with_the_opt_in() {
    let bad = ratio_misses(tgs_rows());
    println!("N1s: 6 comparisons, above 1.05: {bad:?}");
    assert!(bad.is_empty(), "N1s {bad:#?}");
}

/// R4s rows (SWIRL and SECT-M, PISO) and R4b rows (SECT-M, PIMPLE-4) at dt 0.05, 0.025, 0.0125.
fn sector_rows(outer: usize) -> Vec<Row> {
    let probs: &[Prob] = if outer == 1 {
        &[Prob::Swirl, Prob::SectM]
    } else {
        &[Prob::SectM]
    };
    let mut rows = Vec::new();
    for &prob in probs {
        for dt in [0.05, 0.025, 0.0125] {
            let (d, c) = (
                run(prob, 16, dt, outer, false, false),
                run(prob, 16, dt, outer, true, false),
            );
            let r = (
                format!("{prob:?} n_outer {outer}"),
                dt,
                [exact_err(prob, &d), exact_err(prob, &c)],
                [pressure_err(prob, &d), pressure_err(prob, &c)],
            );
            print_row(if outer == 1 { "R4s" } else { "R4b" }, &r, seam_split(&d, &c));
            rows.push(r);
        }
    }
    rows
}

/// ADR 0245 amendment 3 R4s (ADR 0076 R4): no harm on the sector, SWIRL and SECT-M, PISO.
#[test]
fn r4s_sector_piso_no_harm_with_the_opt_in() {
    let bad = ratio_misses(&sector_rows(1));
    println!("R4s: 12 comparisons, above 1.05: {bad:?}");
    assert!(bad.is_empty(), "R4s {bad:#?}");
}

/// ADR 0245 amendment 3 R4b (ADR 0080 B-R4, its PIMPLE-4 rows): no harm on SECT-M, PIMPLE-4.
#[test]
fn r4b_sector_pimple4_no_harm_with_the_opt_in() {
    let bad = ratio_misses(&sector_rows(4));
    println!("R4b: 6 comparisons, above 1.05: {bad:?}");
    assert!(bad.is_empty(), "R4b {bad:#?}");
}

/// The SECT-M PIMPLE-4 fine pair (0.025, 0.0125) against the same-arm dt = 1/1280 reference: (errors, order).
fn fine_pair(c: bool, bp: bool) -> ([f64; 2], f64) {
    let refr = run(Prob::SectM, 16, DT_REF, 4, c, bp);
    let (a, b) = (
        run(Prob::SectM, 16, 0.025, 4, c, bp),
        run(Prob::SectM, 16, 0.0125, 4, c, bp),
    );
    let e = [rel_l2(&a.u.values, &refr.u.values), rel_l2(&b.u.values, &refr.u.values)];
    let o = (e[0] / e[1]).log2();
    println!(
        "RESULT SECT-M PIMPLE-4 fine pair C {c} pair-mean {bp}: e {:.4e} {:.4e} (vs exact {:.4e} {:.4e}; reference vs exact {:.4e}) order {o:.3}",
        e[0],
        e[1],
        exact_err(Prob::SectM, &a),
        exact_err(Prob::SectM, &b),
        exact_err(Prob::SectM, &refr)
    );
    (e, o)
}

/// (default, C) fine pairs of SECT-M PIMPLE-4, and C with the pair mean (printed).
fn r2b_pairs() -> &'static [([f64; 2], f64); 2] {
    static R: OnceLock<[([f64; 2], f64); 2]> = OnceLock::new();
    R.get_or_init(|| {
        let out = [fine_pair(false, false), fine_pair(true, false)];
        let (_, o) = fine_pair(true, true);
        println!("printed: SECT-M PIMPLE-4 with C and coupled_body_pair_mean: order {o:.3}");
        out
    })
}

/// ADR 0245 amendment 3 S4 (set-up): the default SECT-M PIMPLE-4 fine-pair order is ADR 0076's 0.538.
#[test]
fn s4_sectm_pimple4_default_order_is_that_of_adr_0076() {
    let o = r2b_pairs()[0].1;
    println!("S4: default order {o:.3} (limit [0.52, 0.56])");
    assert!((0.52..=0.56).contains(&o), "S4 {o}");
}

/// ADR 0245 amendment 3 R2b (ADR 0076 R2b): the SECT-M PIMPLE-4 fine-pair order with the opt-in in [1.9, 2.3].
#[test]
fn r2b_sectm_pimple4_order_with_the_opt_in() {
    let o = r2b_pairs()[1].1;
    println!("R2b: order with C {o:.3} (band [1.9, 2.3])");
    assert!((1.9..=2.3).contains(&o), "R2b {o}");
}

/// ADR 0245 amendment 3 R2bn: the two fine-pair errors with the opt-in at most 1.05 times the default's.
#[test]
fn r2bn_sectm_pimple4_fine_pair_no_harm_with_the_opt_in() {
    let [(d, _), (c, _)] = r2b_pairs();
    let r = [c[0] / d[0], c[1] / d[1]];
    println!("R2bn: ratios {:.4} {:.4} (limit 1.05)", r[0], r[1]);
    assert!(r[0] <= 1.05 && r[1] <= 1.05, "R2bn {r:?}");
}
