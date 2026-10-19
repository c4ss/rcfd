//! ADR 0076 (wave 35A): transient set-ups on a rotationally periodic annular sector (60 degrees, walls at r = 0.5 and 1, `theta_min`/`theta_max` a
//! rotational pair). SWIRL: the exact unforced decay `u = U(r) e^{-nu k^2 t} e_theta` (U by RK4 shooting, an independent solution). SECT-M:
//! manufactured non-axisymmetric flow `psi = g(t) F(r) cos(6 theta)` with no-slip walls. The candidate fixes measured on them (ADR 0076 K) were not
//! accepted and were removed; this file keeps the set-ups and the default-code smoke (fine-pair orders 1.135 / 2.052 SWIRL, 1.041 / 0.538 SECT-M for
//! PISO / PIMPLE-4). `cargo test --release -p rcfd-fv --test w35a_seam -- --ignored --nocapture`.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::field::rotate;
use rcfd_fv::gradient::green_gauss;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::pressure::{boundary_values_pbc, pressure_bc};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{annulus_sector, Grading};
use std::sync::Arc;

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

/// rms of the face defect phi - interp(u).S and of R(p) on internal faces and on the seam faces.
fn seam_norms(s: &PimpleSolver) -> [f64; 4] {
    let g = &s.geom;
    let m = &g.mesh;
    let (own, nei, sf) = (m.owner(), m.neighbour(), m.face_area_vectors());
    let pbc = pressure_bc(g, &s.u, &s.p);
    let bv = boundary_values_pbc(g, &pbc, &s.p.values);
    let gp = green_gauss(g, &s.p.values, &bv, 0);
    let p = &s.p.values;
    let (mut di, mut ri, mut ni) = (0.0, 0.0, 0usize);
    for f in 0..g.n_internal {
        if g.empty[f] {
            continue;
        }
        let w = g.weights[f];
        let (o, q) = (own[f], nei[f]);
        let uf = [0, 1, 2].map(|k| w * s.u.values[o][k] + (1.0 - w) * s.u.values[q][k]);
        let gf = [0, 1, 2].map(|k| w * gp[o][k] + (1.0 - w) * gp[q][k]);
        di += (s.phi[f] - vec3::dot(uf, sf[f])).powi(2);
        ri += (vec3::dot(gf, sf[f]) - g.lap_coeff[f] * (p[q] - p[o])).powi(2);
        ni += 1;
    }
    let (mut ds, mut rs, mut ns) = (0.0, 0.0, 0usize);
    for (pi, patch) in m.patches().iter().enumerate() {
        let Some(map) = &g.periodic_partner[pi] else { continue };
        let rot = g.periodic_rot[pi];
        for (local, f) in patch.faces().enumerate() {
            let gf_ = map[local];
            let (dp, dq) = (g.dn[f - g.n_internal], g.dn[gf_ - g.n_internal]);
            let w = dq / (dp + dq);
            let (o, q) = (own[f], own[gf_]);
            let r = |v: Vec3| rot.as_ref().map_or(v, |r| rotate(r, v));
            let (uq, gq) = (r(s.u.values[q]), r(gp[q]));
            let uf = [0, 1, 2].map(|k| w * s.u.values[o][k] + (1.0 - w) * uq[k]);
            let gf = [0, 1, 2].map(|k| w * gp[o][k] + (1.0 - w) * gq[k]);
            ds += (s.phi[f] - vec3::dot(uf, sf[f])).powi(2);
            rs += (vec3::dot(gf, sf[f]) - vec3::norm(sf[f]) * (p[q] - p[o]) / (dp + dq)).powi(2);
            ns += 1;
        }
    }
    [
        (di / ni as f64).sqrt(),
        (ri / ni as f64).sqrt(),
        (ds / ns.max(1) as f64).sqrt(),
        (rs / ns.max(1) as f64).sqrt(),
    ]
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

// ---- harness ------------------------------------------------------------------------------------------------------------------------
#[derive(Clone, Copy, PartialEq, Debug)]
enum Prob {
    Swirl,
    SectM,
}

fn eigen() -> &'static Eigen {
    static E: std::sync::OnceLock<Eigen> = std::sync::OnceLock::new();
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

/// `outer` = 1 (PISO) or 4 (PIMPLE-4, alpha 1).
fn make(prob: Prob, n: usize, outer: usize) -> PimpleSolver {
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
    PimpleSolver::new(geom, u, p, s)
}

fn run(prob: Prob, n: usize, dt: f64, outer: usize) -> PimpleSolver {
    let mut s = make(prob, n, outer);
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
        assert!(s.diverged.is_none(), "{prob:?} n {n} dt {dt}: diverged");
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

const DT_REF: f64 = 1.0 / 1280.0;

/// Fine-pair order (0.025, 0.0125) on 16 x 16 against the same-scheme dt = 1/1280 reference; prints the row.
fn fine_order(id: &str, prob: Prob, outer: usize) -> f64 {
    let refr = run(prob, 16, DT_REF, outer);
    let (a, b) = (run(prob, 16, 0.025, outer), run(prob, 16, 0.0125, outer));
    let e = [rel_l2(&a.u.values, &refr.u.values), rel_l2(&b.u.values, &refr.u.values)];
    let o = (e[0] / e[1]).log2();
    println!(
        "RESULT {id} {prob:?} n_outer {outer}: e {:.4e} {:.4e} (vs exact {:.4e} {:.4e}; reference vs exact {:.4e}; seam norms {:?}) order {o:.3} {}",
        e[0],
        e[1],
        exact_err(prob, &a),
        exact_err(prob, &b),
        exact_err(prob, &refr),
        seam_norms(&b),
        if (1.9..=2.3).contains(&o) { "MET" } else { "MISS" }
    );
    o
}

/// Smoke of the set-ups before registration (ADR 0076 rule d): eigenvalue accuracy, periodicity of the manufactured fields, default orders.
#[test]
#[ignore = "ADR 0076 smoke (prints)"]
fn smoke_sector_setups() {
    let e2 = Eigen::new(80000);
    let mut dmax: f64 = 0.0;
    for i in 0..=1000 {
        let r = RA + (RB - RA) * i as f64 / 1000.0;
        dmax = dmax
            .max((eigen().at(r).0 - e2.at(r).0).abs())
            .max((eigen().at(r).1 - e2.at(r).1).abs());
    }
    println!(
        "SMOKE k {:.12} (80000 steps {:.12}), table difference {dmax:.3e}",
        eigen().k,
        e2.k
    );
    let x = [0.7, 0.0, 0.0];
    let a = std::f64::consts::FRAC_PI_3;
    let y = [0.7 * a.cos(), 0.7 * a.sin(), 0.0];
    let rot = |v: Vec3| [v[0] * a.cos() - v[1] * a.sin(), v[0] * a.sin() + v[1] * a.cos(), 0.0];
    println!(
        "SMOKE periodicity u {:?} vs {:?}; source {:?} vs {:?}",
        rot(mms_u(x, 0.3)),
        mms_u(y, 0.3),
        rot(mms_source(x, 0.3)),
        mms_source(y, 0.3)
    );
    for prob in [Prob::Swirl, Prob::SectM] {
        for outer in [1usize, 4] {
            fine_order("SMOKE", prob, outer);
        }
    }
}
