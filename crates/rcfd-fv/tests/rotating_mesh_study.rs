//! Wave 34D (ADR 0075, part C): registered mesh study of the fv-level rotating-frame checks of ADR 0071. R2 (Taylor-Couette in the
//! rotating frame on a periodic 60-degree sector) and R3 (Ekman layer, against the exact finite-height solution) on four meshes h ... h/8,
//! Richardson extrapolation of a functional on the triple (h/2, h/4, h/8), blind prediction of the h/8 max-norm error. The limits are those
//! registered in ADR 0075 before this code ran; the ADR 0071 finest-mesh tests stay ignored and unchanged.

use rcfd_fv::bc::FixedValue;
use rcfd_fv::simple::{BodyForce, Coupling, IterationInfo, SimpleSettings, SimpleSolver};
use rcfd_fv::sources::RotatingFrame;
use rcfd_fv::{FvGeometry, Profile, ScalarField, VectorField};
use rcfd_mesh::gen::{annulus_sector, box_mesh, Grading};
use std::sync::Arc;

// ---------------------------------------------------------------- Richardson analysis (ADR 0075 L1-L7)

/// Result of the registered analysis of one functional on the triple (h/2, h/4, h/8).
#[derive(Debug)]
struct Richardson {
    rho: f64,
    p: f64,
    f_re: f64,
    err_fine: f64,
    err_re: f64,
    gci: f64,
}

fn richardson(f1: f64, f2: f64, f3: f64, f_ex: f64) -> Richardson {
    let rho = (f2 - f3) / (f1 - f2);
    let p = (1.0 / rho).log2();
    let f_re = f3 + (f3 - f2) / (2f64.powf(p) - 1.0);
    let gci = 1.25 * (f3 - f2).abs() / ((2f64.powf(p) - 1.0) * f3.abs());
    Richardson {
        rho,
        p,
        f_re,
        err_fine: (f3 - f_ex).abs(),
        err_re: (f_re - f_ex).abs(),
        gci,
    }
}

/// Failed limits L1-L4 of one functional.
fn functional_misses(name: &str, r: &Richardson, f_ex: f64) -> Vec<String> {
    let mut bad = Vec::new();
    if !(r.rho > 0.0 && r.rho < 1.0) {
        bad.push(format!("{name}: L1 non-monotone, rho = {:.4}", r.rho));
        return bad;
    }
    if !(1.7..=2.3).contains(&r.p) {
        bad.push(format!("{name}: L2 order {:.3} outside [1.7, 2.3]", r.p));
    }
    if r.err_re > 0.2 * r.err_fine {
        bad.push(format!(
            "{name}: L3 extrapolated error {:.3e} > 0.2 x finest error {:.3e}",
            r.err_re, r.err_fine
        ));
    }
    if r.err_fine / f_ex.abs() > r.gci {
        bad.push(format!(
            "{name}: L4 true relative error {:.3e} above GCI {:.3e}",
            r.err_fine / f_ex.abs(),
            r.gci
        ));
    }
    bad
}

/// Failed limits L5-L7 of a max-norm error sequence on h, h/2, h/4, h/8.
fn max_norm_misses(name: &str, e: &[f64; 4], old_limit: f64) -> Vec<String> {
    let mut bad = Vec::new();
    let pred = e[2] * e[2] / e[1];
    if (e[3] / pred - 1.0).abs() > 0.2 {
        bad.push(format!("{name}: L5 h/8 error {:.4e} vs prediction {pred:.4e}", e[3]));
    }
    let o: Vec<f64> = e.windows(2).map(|w| (w[0] / w[1]).log2()).collect();
    if !(1.7..=2.3).contains(&o[2]) {
        bad.push(format!("{name}: L6 finest-pair order {:.3}", o[2]));
    }
    if o[0] < 1.4 || o[1] < 1.4 {
        bad.push(format!("{name}: L6 earlier orders {:.3}, {:.3}", o[0], o[1]));
    }
    if e[3] > old_limit {
        bad.push(format!("{name}: L7 h/8 error {:.4e} > {old_limit:e}", e[3]));
    }
    bad
}

fn print_study(name: &str, e: &[f64; 4], f: &[f64; 4], f_ex: f64, r: &Richardson) {
    let o: Vec<f64> = e.windows(2).map(|w| (w[0] / w[1]).log2()).collect();
    println!(
        "{name}: max-norm errors {:.4e} {:.4e} {:.4e} {:.4e}, orders {:.3} {:.3} {:.3}, L5 prediction {:.4e} (ratio {:.4})",
        e[0],
        e[1],
        e[2],
        e[3],
        o[0],
        o[1],
        o[2],
        e[2] * e[2] / e[1],
        e[3] / (e[2] * e[2] / e[1])
    );
    println!(
        "{name}: functional {:.10e} {:.10e} {:.10e} {:.10e}, exact {f_ex:.10e}; errors {:.3e} {:.3e} {:.3e} {:.3e}",
        f[0],
        f[1],
        f[2],
        f[3],
        (f[0] - f_ex).abs(),
        (f[1] - f_ex).abs(),
        (f[2] - f_ex).abs(),
        (f[3] - f_ex).abs()
    );
    println!(
        "{name}: rho {:.4}, p {:.4}, f_RE {:.10e}, |f_RE - f_ex| {:.3e} = {:.4} x finest error {:.3e}, GCI {:.3e} vs true relative {:.3e}",
        r.rho,
        r.p,
        r.f_re,
        r.err_re,
        r.err_re / r.err_fine,
        r.err_fine,
        r.gci,
        r.err_fine / f_ex.abs()
    );
}

// ---------------------------------------------------------------- R2 Taylor-Couette sector

const RI: f64 = 0.5;
const RO: f64 = 1.0;
const NU: f64 = 0.1;
const W_I: f64 = 1.0;
const V_I: f64 = 0.5;
const DEPTH: f64 = 0.1;

fn tc(r: f64) -> f64 {
    let d = RO * RO - RI * RI;
    -W_I * RI * RI / d * r + W_I * RI * RI * RO * RO / d / r
}

struct Sector {
    solver: SimpleSolver,
    frame: RotatingFrame,
    zone: Vec<bool>,
}

impl Sector {
    fn new(n_theta: usize, n_r: usize) -> Self {
        let mesh = annulus_sector(
            n_theta,
            n_r,
            RI,
            RO,
            std::f64::consts::FRAC_PI_3,
            Grading::Uniform,
            DEPTH,
        )
        .unwrap();
        let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
        let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
        let cf = geom.mesh.face_centres();
        let wall = |name: &str, w_abs: f64| {
            let w = w_abs - W_I;
            geom.mesh
                .patch(name)
                .unwrap()
                .faces()
                .map(|f| [-w * cf[f][1], w * cf[f][0], 0.0])
                .collect::<Vec<_>>()
        };
        u.set_bc(
            &geom,
            "inner",
            Box::new(FixedValue(Profile::PerFace(wall("inner", W_I)))),
        )
        .unwrap();
        u.set_bc(
            &geom,
            "outer",
            Box::new(FixedValue(Profile::PerFace(wall("outer", 0.0)))),
        )
        .unwrap();
        let p = ScalarField::uniform("p", &geom, 0.0);
        let mut s = SimpleSettings::new(NU, Coupling::Simplec);
        s.alpha_u = 0.8;
        s.alpha_p = 1.0;
        s.convection = rcfd_fv::convection::ConvScheme::Central;
        let solver = SimpleSolver::new(geom.clone(), u, p, s);
        let zone = vec![true; geom.n_cells()];
        Sector {
            solver,
            frame: RotatingFrame {
                origin: [0.0; 3],
                axis: [0.0, 0.0, 1.0],
                omega: W_I,
                centrifugal: true,
            },
            zone,
        }
    }

    fn step(&mut self) -> IterationInfo {
        let geom = self.solver.geom.clone();
        let ub = self.solver.u.boundary_values(&geom);
        let mut f = BodyForce::uniform(&geom, [0.0; 3]);
        self.frame
            .add_force(&geom, &self.solver.u.values, &ub, &self.zone, &mut f);
        self.solver.body_force = Some(f);
        self.solver.wall_pressure_trapezoid = true;
        self.solver.iterate()
    }

    /// Runs to the registered criterion (res_u < 1e-10, continuity < 1e-9) within `cap` iterations.
    fn converge(&mut self, cap: usize) -> (bool, IterationInfo, usize) {
        let mut last = self.step();
        for k in 0..cap {
            if last.res_u < 1e-10 && last.continuity < 1e-9 {
                return (true, last, k + 1);
            }
            last = self.step();
        }
        (false, last, cap)
    }

    /// max |u_abs,theta - analytic| / V_i.
    fn error(&self) -> f64 {
        let cc = self.solver.geom.mesh.cell_centres();
        self.solver
            .u
            .values
            .iter()
            .enumerate()
            .map(|(c, v)| {
                let (x, y) = (cc[c][0], cc[c][1]);
                let r = x.hypot(y);
                let ut = (-y * v[0] + x * v[1]) / r + W_I * r;
                (ut - tc(r)).abs() / V_I
            })
            .fold(0.0, f64::max)
    }

    /// |seam volume flux through theta_min| (the C5a quantity).
    fn seam(&self) -> f64 {
        let s = &self.solver;
        s.geom
            .mesh
            .patch("theta_min")
            .unwrap()
            .faces()
            .map(|f| s.phi[f])
            .sum::<f64>()
            .abs()
    }
}

fn seam_exact() -> f64 {
    let (a, b) = (
        -W_I * RI * RI / (RO * RO - RI * RI),
        W_I * RI * RI * RO * RO / (RO * RO - RI * RI),
    );
    (DEPTH * (0.5 * a * (RO * RO - RI * RI) + b * (RO / RI).ln() - 0.5 * W_I * (RO * RO - RI * RI))).abs()
}

/// (max-norm errors, functionals, failures of convergence / I1).
fn r2_study() -> ([f64; 4], [f64; 4], Vec<String>) {
    let mut e = [0.0; 4];
    let mut f = [0.0; 4];
    let mut bad = Vec::new();
    let mut i1 = Vec::new();
    for (k, (nt, nr)) in [(2, 4), (4, 8), (8, 16), (16, 32)].into_iter().enumerate() {
        let mut s = Sector::new(nt, nr);
        let (conv, last, it) = s.converge(20000);
        e[k] = s.error();
        f[k] = s.seam();
        println!(
            "R2 sector {nt}x{nr}: converged {conv} in {it} iterations, error {:.4e} V_i, seam flux {:.10e}, last {last:?}",
            e[k], f[k]
        );
        if !conv {
            bad.push(format!("R2 {nt}x{nr} not converged: {last:?}"));
        }
        if k >= 2 {
            for _ in 0..500 {
                s.step();
            }
            i1.push((k, (s.seam() - f[k]).abs(), (s.error() - e[k]).abs()));
        }
    }
    for (k, df, de) in i1 {
        println!(
            "R2 I1 mesh {k}: functional change {df:.3e} (limit {:.3e}), max-norm change {de:.3e} (limit {:.3e})",
            1e-2 * (f[2] - f[3]).abs(),
            1e-2 * e[3]
        );
        if df > 1e-2 * (f[2] - f[3]).abs() || de > 1e-2 * e[3] {
            bad.push(format!("R2 I1 mesh {k}: changes {df:.3e}, {de:.3e}"));
        }
    }
    (e, f, bad)
}

fn r2_misses() -> Vec<String> {
    let (e, f, mut bad) = r2_study();
    let f_ex = seam_exact();
    let r = richardson(f[1], f[2], f[3], f_ex);
    print_study("R2", &e, &f, f_ex, &r);
    bad.extend(functional_misses("R2 seam flux", &r, f_ex));
    bad.extend(max_norm_misses("R2", &e, 5e-3));
    bad
}

#[test]
fn r2_taylor_couette_sector_registered_mesh_study() {
    let bad = r2_misses();
    assert!(bad.is_empty(), "{bad:#?}");
}

// ---------------------------------------------------------------- R3 Ekman layer, exact finite-height solution

#[derive(Clone, Copy, Debug)]
struct C(f64, f64);

impl C {
    fn mul(self, o: C) -> C {
        C(self.0 * o.0 - self.1 * o.1, self.0 * o.1 + self.1 * o.0)
    }
    fn div(self, o: C) -> C {
        let d = o.0 * o.0 + o.1 * o.1;
        C((self.0 * o.0 + self.1 * o.1) / d, (self.1 * o.0 - self.0 * o.1) / d)
    }
    fn scale(self, s: f64) -> C {
        C(self.0 * s, self.1 * s)
    }
    fn exp(self) -> C {
        let m = self.0.exp();
        C(m * self.1.cos(), m * self.1.sin())
    }
    fn sinh(self) -> C {
        let (a, b) = (self.exp(), self.scale(-1.0).exp());
        C(0.5 * (a.0 - b.0), 0.5 * (a.1 - b.1))
    }
    fn cosh(self) -> C {
        let (a, b) = (self.exp(), self.scale(-1.0).exp());
        C(0.5 * (a.0 + b.0), 0.5 * (a.1 + b.1))
    }
}

const EK_NU: f64 = 0.01;
const EK_OMEGA: f64 = 1.0;
const EK_UG: f64 = 1.0;

fn ek_delta() -> f64 {
    (EK_NU / EK_OMEGA).sqrt()
}

fn ek_h() -> f64 {
    8.0 * ek_delta()
}

fn ek_lambda() -> C {
    C(1.0 / ek_delta(), -1.0 / ek_delta())
}

/// Exact V(y) = (u - U_g) + i w = -U_g sinh(lambda (H - y)) / sinh(lambda H).
fn ek_v(y: f64) -> C {
    let l = ek_lambda();
    l.scale(ek_h() - y).sinh().div(l.scale(ek_h()).sinh()).scale(-EK_UG)
}

/// Exact transport int_0^H V dy = -U_g (cosh(lambda H) - 1) / (lambda sinh(lambda H)).
fn ek_transport() -> C {
    let l = ek_lambda();
    let lh = l.scale(ek_h());
    let ch = lh.cosh();
    C(ch.0 - 1.0, ch.1).div(l.mul(lh.sinh())).scale(-EK_UG)
}

struct Ekman {
    solver: SimpleSolver,
    frame: RotatingFrame,
    zone: Vec<bool>,
}

impl Ekman {
    fn new(n: usize) -> Self {
        let mut mesh = box_mesh([2, n, 2], [0.1, ek_h(), 0.1], [Grading::Uniform; 3]).unwrap();
        for (a, b) in [("xmin", "xmax"), ("zmin", "zmax")] {
            mesh.set_periodic(a, b).unwrap();
            mesh.set_periodic(b, a).unwrap();
        }
        let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
        let mut u = VectorField::uniform("U", &geom, [EK_UG, 0.0, 0.0]);
        u.set_bc(&geom, "ymin", Box::new(FixedValue::vector([0.0; 3]))).unwrap();
        u.set_bc(&geom, "ymax", Box::new(FixedValue::vector([EK_UG, 0.0, 0.0])))
            .unwrap();
        let p = ScalarField::uniform("p", &geom, 0.0);
        let mut s = SimpleSettings::new(EK_NU, Coupling::Simplec);
        s.alpha_u = 0.8;
        s.alpha_p = 1.0;
        s.convection = rcfd_fv::convection::ConvScheme::Central;
        let solver = SimpleSolver::new(geom.clone(), u, p, s);
        let zone = vec![true; geom.n_cells()];
        Ekman {
            solver,
            frame: RotatingFrame {
                origin: [0.0; 3],
                axis: [0.0, 1.0, 0.0],
                omega: EK_OMEGA,
                centrifugal: false,
            },
            zone,
        }
    }

    fn step(&mut self) -> IterationInfo {
        let geom = self.solver.geom.clone();
        let ub = self.solver.u.boundary_values(&geom);
        let mut f = BodyForce::uniform(&geom, [0.0, 0.0, -2.0 * EK_OMEGA * EK_UG]);
        self.frame
            .add_force(&geom, &self.solver.u.values, &ub, &self.zone, &mut f);
        self.solver.body_force = Some(f);
        self.solver.wall_pressure_trapezoid = true;
        self.solver.iterate()
    }

    /// The C3b criterion: max_du < 1e-11 and continuity < 1e-9.
    fn converge(&mut self, cap: usize) -> (bool, IterationInfo, usize) {
        let mut last = self.step();
        for k in 0..cap {
            if last.max_du < 1e-11 && last.continuity < 1e-9 {
                return (true, last, k + 1);
            }
            last = self.step();
        }
        (false, last, cap)
    }

    /// (max error / U_g against the finite-height solution, max |w| / U_g, transports (T_u, T_w)).
    fn measure(&self) -> (f64, f64, f64, f64) {
        let geom = &self.solver.geom;
        let cc = geom.mesh.cell_centres();
        let vol = geom.mesh.cell_volumes();
        let area = 0.1 * 0.1;
        let (mut err, mut wmax, mut tu, mut tw) = (0.0f64, 0.0f64, 0.0, 0.0);
        for (c, v) in self.solver.u.values.iter().enumerate() {
            let ve = ek_v(cc[c][1]);
            err = err.max((v[0] - (EK_UG + ve.0)).abs().max((v[2] - ve.1).abs()) / EK_UG);
            wmax = wmax.max(v[2].abs() / EK_UG);
            tu += (v[0] - EK_UG) * vol[c] / area;
            tw += v[2] * vol[c] / area;
        }
        (err, wmax, tu, tw)
    }
}

/// (max-norm errors, T_u, T_w, max |w| on h/8, failures of convergence / I1).
fn r3_study() -> ([f64; 4], [f64; 4], [f64; 4], f64, Vec<String>) {
    let (mut e, mut tu, mut tw) = ([0.0; 4], [0.0; 4], [0.0; 4]);
    let mut wmax = 0.0;
    let mut bad = Vec::new();
    let mut i1 = Vec::new();
    for (k, n) in [16, 32, 64, 128].into_iter().enumerate() {
        let mut s = Ekman::new(n);
        let (conv, last, it) = s.converge(400000);
        let (err, w, a, b) = s.measure();
        e[k] = err;
        tu[k] = a;
        tw[k] = b;
        wmax = w;
        println!(
            "R3 Ekman N={n}: converged {conv} in {it} iterations, error {err:.4e} U_g, max|w| {w:.5}, T_u {a:.10e}, T_w {b:.10e}, last {last:?}"
        );
        if !conv {
            bad.push(format!("R3 N={n} not converged: {last:?}"));
        }
        if k >= 2 {
            for _ in 0..500 {
                s.step();
            }
            let (err2, _, a2, b2) = s.measure();
            i1.push((k, (a2 - a).abs(), (b2 - b).abs(), (err2 - err).abs()));
        }
    }
    for (k, da, db, de) in i1 {
        println!("R3 I1 mesh {k}: T_u change {da:.3e}, T_w change {db:.3e}, max-norm change {de:.3e}");
        if da > 1e-2 * (tu[2] - tu[3]).abs() || db > 1e-2 * (tw[2] - tw[3]).abs() || de > 1e-2 * e[3] {
            bad.push(format!("R3 I1 mesh {k}: changes {da:.3e}, {db:.3e}, {de:.3e}"));
        }
    }
    (e, tu, tw, wmax, bad)
}

fn r3_misses() -> Vec<String> {
    let (e, tu, tw, wmax, mut bad) = r3_study();
    let t = ek_transport();
    let ru = richardson(tu[1], tu[2], tu[3], t.0);
    let rw = richardson(tw[1], tw[2], tw[3], t.1);
    print_study("R3 T_u", &e, &tu, t.0, &ru);
    print_study("R3 T_w", &e, &tw, t.1, &rw);
    bad.extend(functional_misses("R3 T_u", &ru, t.0));
    bad.extend(functional_misses("R3 T_w", &rw, t.1));
    bad.extend(max_norm_misses("R3", &e, 2e-3));
    // spiral amplitude of the finite-height solution (sampled finely)
    let exact_w = (0..=80000)
        .map(|i| ek_v(ek_h() * i as f64 / 80000.0).1.abs())
        .fold(0.0, f64::max);
    println!("R3 max|w| on h/8 {wmax:.5} vs finite-height analytic {exact_w:.5}");
    if (wmax / exact_w - 1.0).abs() > 0.05 {
        bad.push(format!("R3 spiral amplitude {wmax} vs {exact_w}"));
    }
    bad
}

#[test]
fn r3_finite_height_solution_satisfies_its_conditions() {
    // derivation check of the reference (no CFD): V(0) = -U_g, V(H) = 0, and the ODE V'' = lambda^2 V by central differences
    let v0 = ek_v(0.0);
    let vh = ek_v(ek_h());
    assert!((v0.0 + EK_UG).abs() < 1e-12 && v0.1.abs() < 1e-12, "{v0:?}");
    assert!(vh.0.abs() < 1e-12 && vh.1.abs() < 1e-12, "{vh:?}");
    let l2 = ek_lambda().mul(ek_lambda());
    let d = 1e-5;
    for y in [0.05, 0.2, 0.5] {
        let (a, b, c) = (ek_v(y - d), ek_v(y), ek_v(y + d));
        let lap = C((a.0 - 2.0 * b.0 + c.0) / (d * d), (a.1 - 2.0 * b.1 + c.1) / (d * d));
        let rhs = l2.mul(b);
        assert!(
            (lap.0 - rhs.0).abs() + (lap.1 - rhs.1).abs() < 1e-3 * (rhs.0.abs() + rhs.1.abs()),
            "y {y}"
        );
    }
    // transport by quadrature
    let n = 200000;
    let h = ek_h() / n as f64;
    let (mut su, mut sw) = (0.0, 0.0);
    for i in 0..n {
        let v = ek_v((i as f64 + 0.5) * h);
        su += v.0 * h;
        sw += v.1 * h;
    }
    let t = ek_transport();
    println!("R3 exact transport {t:?}, quadrature ({su:.12e}, {sw:.12e})");
    assert!((su - t.0).abs() < 1e-9 && (sw - t.1).abs() < 1e-9);
}

#[test]
fn r3_ekman_registered_mesh_study() {
    let bad = r3_misses();
    assert!(bad.is_empty(), "{bad:#?}");
}
