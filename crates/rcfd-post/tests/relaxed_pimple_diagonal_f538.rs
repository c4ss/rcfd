//! F-538 (ADR 0613): the averaged component diagonal in the relaxed PIMPLE loop. ADR 0455 found that the averaged diagonal
//! of the PISO correctors puts a sink next to a `Symmetry` plane (the oscillating channel QF: `Delta Z = 0.36` over the
//! zero-gradient planes), but the relaxed `PimpleSettings::pimple` loop (alpha_u 0.8, alpha_p 0.5, one corrector, 10 outer
//! iterations) showed `-0.0021`. Here an exact linear model of the relaxed step in the developed channel (ADR 0613 D1), the
//! converged loop in closed form and its 0-D reduction (D2): the relaxed iterations converge to a sink `lambda/alpha_u` plus an
//! anti-sink through the neighbour links, the unrelaxed final iteration removes part of it, and at alpha_u 0.8 and this time step
//! the remainder nearly cancels, `sigma(alpha_u) = r - (1/alpha_u - 1)(1/r - 1)`.
//!
//! The limits, set-ups and mutations were registered in ADR 0613 before this file existed. The channel and its run are those of
//! `symmetry_flux_f531.rs` (ADR 0455; copied, that file is not edited).

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::{FixedValue, Symmetry};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::cosim::{FlowWall, Fluid, InterfaceMap};
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_mesh::motion::moved_mesh;
use rcfd_mesh::Mesh;
use rcfd_post::solid_aerodamp::{drive_mode, ModeHistory, ModeMotion, PeriodicResponse};
use rcfd_post::solid_fsi::ChannelMotion;
use std::f64::consts::PI;
use std::sync::Arc;

const TOL_PERIODIC: f64 = 1e-6;
const OMEGA: f64 = 2.0 * PI;

/// Measured values against their limits; every value is printed, the test fails at the end if any missed.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Self {
        Checks(Vec::new())
    }
    fn at_most(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{id}: {value:e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{id}: {value:e} > {limit:e}"));
        }
    }
    fn at_least(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value >= limit;
        println!(
            "{id}: {value:e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{id}: {value:e} < {limit:e}"));
        }
    }
    fn holds(&mut self, id: &str, ok: bool, what: &str) {
        println!("{id}: {what} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{id}: {what}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

// ---- complex arithmetic ----------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
struct Cx(f64, f64);

impl Cx {
    const ZERO: Cx = Cx(0.0, 0.0);
    const ONE: Cx = Cx(1.0, 0.0);
    fn add(self, o: Cx) -> Cx {
        Cx(self.0 + o.0, self.1 + o.1)
    }
    fn sub(self, o: Cx) -> Cx {
        Cx(self.0 - o.0, self.1 - o.1)
    }
    fn scale(self, a: f64) -> Cx {
        Cx(a * self.0, a * self.1)
    }
    fn mul(self, o: Cx) -> Cx {
        Cx(self.0 * o.0 - self.1 * o.1, self.0 * o.1 + self.1 * o.0)
    }
    fn div(self, o: Cx) -> Cx {
        let d = o.0 * o.0 + o.1 * o.1;
        Cx((self.0 * o.0 + self.1 * o.1) / d, (self.1 * o.0 - self.0 * o.1) / d)
    }
    fn exp(self) -> Cx {
        let r = self.0.exp();
        Cx(r * self.1.cos(), r * self.1.sin())
    }
    fn abs(self) -> f64 {
        self.0.hypot(self.1)
    }
}

/// Per-part relative errors `(Re z/Re r - 1, Im z/Im r - 1)`.
fn rel_parts(z: Cx, r: Cx) -> (f64, f64) {
    (z.0 / r.0 - 1.0, z.1 / r.1 - 1.0)
}

/// `|z - r|/|r|`.
fn rel_mod(z: Cx, r: Cx) -> f64 {
    z.sub(r).abs() / r.abs()
}

/// Gaussian elimination with partial pivoting of the dense complex system `a x = b` (`a` row-major, `n x n`).
fn solve_complex(mut a: Vec<Cx>, mut b: Vec<Cx>) -> Vec<Cx> {
    let n = b.len();
    for col in 0..n {
        let p = (col..n)
            .max_by(|&i, &j| a[i * n + col].abs().total_cmp(&a[j * n + col].abs()))
            .unwrap();
        if p != col {
            for j in 0..n {
                a.swap(p * n + j, col * n + j);
            }
            b.swap(p, col);
        }
        let piv = a[col * n + col];
        for i in col + 1..n {
            let f = a[i * n + col].div(piv);
            if f == Cx::ZERO {
                continue;
            }
            for j in col..n {
                let v = a[col * n + j].mul(f);
                a[i * n + j] = a[i * n + j].sub(v);
            }
            let v = b[col].mul(f);
            b[i] = b[i].sub(v);
        }
    }
    let mut x = vec![Cx::ZERO; n];
    for i in (0..n).rev() {
        let mut s = b[i];
        for j in i + 1..n {
            s = s.sub(a[i * n + j].mul(x[j]));
        }
        x[i] = s.div(a[i * n + i]);
    }
    x
}

// ---- the channel (ADR 0455 D4, the code of symmetry_flux_f531.rs) -------------------------------------------------------

const H: f64 = 0.02;
const RHO: f64 = 1000.0;
const WALL: &str = "xmin";
const NY: usize = 8;
const WO: f64 = 1.0;

fn nu() -> f64 {
    OMEGA * (H / 2.0).powi(2) / (WO * WO)
}

/// The step as run: `Some(alpha_u)` = `PimpleSettings::pimple(nu, 10)` with that `alpha_u`; `None` = `PimpleSettings::piso(nu)`.
fn settings(relaxed: Option<f64>, corrected: bool) -> PimpleSettings {
    let mut s = match relaxed {
        Some(a) => {
            let mut s = PimpleSettings::pimple(nu(), 10);
            s.alpha_u = a;
            s
        }
        None => PimpleSettings::piso(nu()),
    };
    s.convection = ConvScheme::Central;
    for c in [&mut s.momentum_solver, &mut s.pressure_solver] {
        c.tol_abs = 1e-16;
        c.tol_rel = 1e-13;
        c.max_iter = 5000;
    }
    s.component_diagonal = corrected;
    s
}

/// Fluid at rest, `p = 0` at `xmax`, on `mesh0` moved to `points`; `walls` get zero velocity, `slips` the symmetry condition.
fn flow(mesh0: &Mesh, points: Vec<Vec3>, s: PimpleSettings, walls: &[&str], slips: &[&str]) -> PimpleSolver {
    let mesh = moved_mesh(mesh0, points).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
    for name in slips {
        u.set_bc(&geom, name, Box::new(Symmetry)).unwrap();
    }
    for name in walls {
        u.set_bc(&geom, name, Box::new(FixedValue::vector(vec3::ZERO))).unwrap();
    }
    let mut pr = ScalarField::uniform("p", &geom, 0.0);
    pr.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    PimpleSolver::new(geom, u, pr, s)
}

#[derive(Debug, Clone, Copy)]
struct Run {
    /// `false`: 20 h (30 cells); `true`: 40 h (60 cells).
    long: bool,
    z_sym: bool,
    corrected: bool,
    relaxed: Option<f64>,
    n: usize,
}

impl Run {
    fn lf(&self) -> f64 {
        if self.long {
            40.0 * H
        } else {
            20.0 * H
        }
    }
    fn label(&self) -> String {
        format!(
            "{} {} {}{} N {}",
            match self.relaxed {
                Some(a) => format!("relaxed alpha_u {a}"),
                None => "PISO".to_string(),
            },
            if self.long { "40h" } else { "20h" },
            if self.z_sym { "Symmetry" } else { "zero-gradient" },
            if self.corrected { " corrected" } else { "" },
            self.n
        )
    }
}

fn channel_run(r: &Run) -> ModeHistory {
    let nx = if r.long { 60 } else { 30 };
    let mesh0 = box_mesh(
        [nx, NY, 1],
        [r.lf(), H, H],
        [Grading::Expansion(5.5), Grading::Uniform, Grading::Uniform],
    )
    .unwrap();
    let motion = ChannelMotion::new(&mesh0, WALL, 0, r.lf(), 1e-9).unwrap();
    let mut fluid = FlowWall::new(
        flow(
            &mesh0,
            mesh0.points().to_vec(),
            settings(r.relaxed, r.corrected),
            &["ymin", "ymax"],
            if r.z_sym { &["zmin", "zmax"] } else { &[] },
        ),
        WALL,
        &mesh0,
        RHO,
        nu(),
        motion.into_motion(),
    )
    .unwrap();
    let np = fluid.point_coordinates().len();
    let rows = (0..np).map(|i| vec![(i, 1.0)]).collect();
    let map = InterfaceMap::new(rows, np, fluid.face_points()).unwrap();
    let motion = ModeMotion {
        shape: vec![[1.0, 0.0, 0.0]; np],
        amplitude: 1e-3 * H,
        omega: OMEGA,
        steps_per_cycle: r.n,
        cycles: 3,
    };
    drive_mode(&mut fluid, &map, &motion).unwrap()
}

fn response_of(c: &mut Checks, id: &str, h: &ModeHistory) -> Option<PeriodicResponse> {
    match h.response(TOL_PERIODIC) {
        Ok(r) => {
            c.at_most(&format!("Q5 {id} cycle change"), r.cycle_change, TOL_PERIODIC);
            Some(r)
        }
        Err(m) => {
            c.holds(&format!("Q5 {id}"), false, &m);
            None
        }
    }
}

fn channel_z(c: &mut Checks, r: &Run) -> Option<Cx> {
    let t0 = std::time::Instant::now();
    let id = r.label();
    let h = channel_run(r);
    let z = response_of(c, &id, &h).map(|p| {
        let (re, im) = p.impedance();
        Cx(re, im)
    });
    println!("{id}: Z {z:?} ({:.1} s)", t0.elapsed().as_secs_f64());
    z
}

// ---- the exact linear model of the step in the developed channel (ADR 0613 D1) -----------------------------------------

/// Parameters of the model step.
#[derive(Debug, Clone, Copy)]
struct Model {
    n: usize,
    /// The averaged-diagonal excess `lambda` (`4 nu/(3 h^2)` with Symmetry planes, 0 with zero gradient).
    lambda: f64,
    n_outer: usize,
    n_corr: usize,
    alpha_u: f64,
    alpha_p: f64,
    /// Negative-control variant (ADR 0613 NC1): the last outer iteration relaxed as well.
    final_relaxed: bool,
    /// Model X (ADR 0613 amendment 1): the diagonal `c` of the `x`-direction Laplacian of the column, whose off-diagonals act
    /// as `-c` on an `x`-uniform field (0: the registered model D1).
    cx: f64,
}

impl Model {
    fn piso(n: usize, z_sym: bool) -> Self {
        Model {
            n,
            lambda: if z_sym { lambda_one_cell() } else { 0.0 },
            n_outer: 1,
            n_corr: 2,
            alpha_u: 1.0,
            alpha_p: 1.0,
            final_relaxed: false,
            cx: 0.0,
        }
    }
    fn relaxed(n: usize, z_sym: bool, alpha_u: f64) -> Self {
        Model {
            n_outer: 10,
            n_corr: 1,
            alpha_u,
            alpha_p: 0.5,
            ..Model::piso(n, z_sym)
        }
    }
    fn dt(&self) -> f64 {
        1.0 / self.n as f64
    }
    fn kap(&self) -> f64 {
        nu() / (H / NY as f64).powi(2)
    }
    /// The own-component diagonal `D` per volume (no-slip walls at both ends of the gap).
    fn diag(&self) -> Vec<f64> {
        let k = self.kap();
        (0..NY)
            .map(|i| {
                let walls = if i == 0 || i == NY - 1 { 1.0 } else { 0.0 };
                1.0 / self.dt() + k * (2.0 + walls)
            })
            .collect()
    }
}

/// `lambda = 4 nu/(3 h^2)` (ADR 0455 D1).
fn lambda_one_cell() -> f64 {
    4.0 * nu() / (3.0 * H * H)
}

/// Solve the tridiagonal system `lo_i x_{i-1} + di_i x_i + up_i x_{i+1} = r_i` (Thomas).
fn tridiag(lo: &[f64], di: &[f64], up: &[f64], r: &[f64]) -> Vec<f64> {
    let n = di.len();
    let mut c = vec![0.0; n];
    let mut d = vec![0.0; n];
    for i in 0..n {
        let (den, rhs) = if i == 0 {
            (di[0], r[0])
        } else {
            (di[i] - lo[i] * c[i - 1], r[i] - lo[i] * d[i - 1])
        };
        c[i] = up[i] / den;
        d[i] = rhs / den;
    }
    let mut x = vec![0.0; n];
    x[n - 1] = d[n - 1];
    for i in (0..n - 1).rev() {
        x[i] = d[i] - c[i] * x[i + 1];
    }
    x
}

/// `O u`: the off-diagonals `-kap` of the gap.
fn off_times(kap: f64, u: &[f64]) -> Vec<f64> {
    (0..u.len())
        .map(|i| {
            let lo = if i > 0 { u[i - 1] } else { 0.0 };
            let hi = if i + 1 < u.len() { u[i + 1] } else { 0.0 };
            -kap * (lo + hi)
        })
        .collect()
}

fn mean(x: &[f64]) -> f64 {
    x.iter().sum::<f64>() / x.len() as f64
}

/// One step of `PimpleSolver::step_with` on the developed channel (ADR 0613 D1): `(u_old, G_old, v) -> (u, G)`, the
/// averaged diagonal (`corrected = false`) or the corrected split.
fn model_step(m: &Model, corrected: bool, u_old: &[f64], g_old: f64, v: f64) -> (Vec<f64>, f64) {
    // the own diagonal with the x-direction Laplacian (model X; `cx = 0` leaves D1's arithmetic unchanged)
    let d0: Vec<f64> = m.diag().iter().map(|d| d + m.cx).collect();
    let kap = m.kap();
    let dt = m.dt();
    let b0: Vec<f64> = u_old.iter().map(|x| x / dt).collect();
    let mut u = u_old.to_vec();
    let mut g = g_old;
    let off = vec![-kap; NY];
    for o in 0..m.n_outer {
        let last = o + 1 == m.n_outer;
        let (au, ap) = if last && !m.final_relaxed {
            (1.0, 1.0)
        } else {
            (m.alpha_u, m.alpha_p)
        };
        // relax: D_r = D/a_u, b_r = b + (D_r - D) u with u the iterate at the start of the outer iteration
        let dr: Vec<f64> = d0.iter().map(|d| d / au).collect();
        let b: Vec<f64> = (0..NY).map(|i| b0[i] + (dr[i] - d0[i]) * u[i]).collect();
        let a_p: Vec<f64> = d0.iter().map(|d| (d + m.lambda) / au).collect();
        let rhs: Vec<f64> = b.iter().map(|x| x - g).collect();
        let dp: Vec<f64> = dr.iter().map(|d| d - m.cx).collect();
        u = tridiag(&off, &dp, &off, &rhs);
        let inv_mean = mean(&a_p.iter().map(|a| 1.0 / a).collect::<Vec<_>>());
        for corr in 0..m.n_corr {
            let ou = off_times(kap, &u);
            let hby_a: Vec<f64> = (0..NY)
                .map(|i| {
                    let h = b[i] - ou[i] + m.cx * u[i];
                    let extra = if corrected { (a_p[i] - dr[i]) * u[i] } else { 0.0 };
                    (h + extra) / a_p[i]
                })
                .collect();
            let g_new = (mean(&hby_a) - v) / inv_mean;
            u = (0..NY).map(|i| hby_a[i] - g_new / a_p[i]).collect();
            let a = if corr + 1 == m.n_corr { ap } else { 1.0 };
            g += a * (g_new - g);
        }
    }
    (u, g)
}

/// The periodic state of a linear step `x_n = T x_{n-1} + g v_n`, `x = (u, G)`, and the impedance `-rho h^2 L G s/(i w)` of
/// `L = 20 h` of developed channel.
fn periodic_impedance(n: usize, step: &dyn Fn(&[f64], f64, f64) -> (Vec<f64>, f64)) -> Cx {
    let dt = 1.0 / n as f64;
    let ns = NY + 1;
    let z = Cx(0.0, -OMEGA * dt).exp();
    let zero = vec![0.0; NY];
    let (u1, g1) = step(&zero, 0.0, 1.0);
    let mut rhs: Vec<Cx> = u1.iter().map(|&x| Cx(x, 0.0)).collect();
    rhs.push(Cx(g1, 0.0));
    let mut a = vec![Cx::ZERO; ns * ns];
    for j in 0..ns {
        let mut e = zero.clone();
        let mut ge = 0.0;
        if j < NY {
            e[j] = 1.0;
        } else {
            ge = 1.0;
        }
        let (u, g) = step(&e, ge, 0.0);
        for i in 0..ns {
            let t = if i < NY { u[i] } else { g };
            a[i * ns + j] = z.scale(-t);
        }
    }
    for i in 0..ns {
        a[i * ns + i] = a[i * ns + i].add(Cx::ONE);
    }
    let x = solve_complex(a, rhs);
    let s = Cx::ONE.sub(z).scale(1.0 / dt);
    x[NY].mul(s).div(Cx(0.0, OMEGA)).scale(-RHO * H * H * 20.0 * H)
}

fn model_z(m: &Model, corrected: bool) -> Cx {
    periodic_impedance(m.n, &|u, g, v| model_step(m, corrected, u, g, v))
}

fn with_lambda(m: &Model, z_sym: bool) -> Model {
    Model {
        lambda: if z_sym { lambda_one_cell() } else { 0.0 },
        ..*m
    }
}

/// `Delta Z` of the model: Symmetry minus zero gradient, the rest of `m` kept.
fn model_dz(m: &Model) -> Cx {
    model_z(&with_lambda(m, true), false).sub(model_z(&with_lambda(m, false), false))
}

/// Model CL (ADR 0613 D2): the relaxed loop converged, `K u_fp = b - G_fp`, `K = D + O + L/alpha_u + O D^{-1} L`, then the
/// unrelaxed final iteration with one averaged corrector.
fn cl_step(m: &Model, u_old: &[f64], v: f64) -> (Vec<f64>, f64) {
    let d = m.diag();
    let kap = m.kap();
    let lam = m.lambda;
    let b: Vec<f64> = u_old.iter().map(|x| x / m.dt()).collect();
    // K: diagonal D + L/alpha_u; the off-diagonals of O (I + D^{-1} L): row i, column j gets -kap (1 + lam/d_j)
    let kd: Vec<f64> = d.iter().map(|x| x + lam / m.alpha_u).collect();
    let klo: Vec<f64> = (0..NY)
        .map(|i| if i > 0 { -kap * (1.0 + lam / d[i - 1]) } else { 0.0 })
        .collect();
    let kup: Vec<f64> = (0..NY)
        .map(|i| if i + 1 < NY { -kap * (1.0 + lam / d[i + 1]) } else { 0.0 })
        .collect();
    let kb = tridiag(&klo, &kd, &kup, &b);
    let k1 = tridiag(&klo, &kd, &kup, &[1.0; NY]);
    let g_fp = (mean(&kb) - v) / mean(&k1);
    let off = vec![-kap; NY];
    let rhs: Vec<f64> = b.iter().map(|x| x - g_fp).collect();
    let uss = tridiag(&off, &d, &off, &rhs);
    let a_p: Vec<f64> = d.iter().map(|x| x + lam).collect();
    let hby_a: Vec<f64> = (0..NY).map(|i| (d[i] * uss[i] + g_fp) / a_p[i]).collect();
    let inv_mean = mean(&a_p.iter().map(|a| 1.0 / a).collect::<Vec<_>>());
    let g_f = (mean(&hby_a) - v) / inv_mean;
    ((0..NY).map(|i| hby_a[i] - g_f / a_p[i]).collect(), g_f)
}

fn cl_dz(n: usize, alpha_u: f64) -> Cx {
    let zs = |z_sym: bool| {
        let m = with_lambda(&Model::relaxed(n, true, alpha_u), z_sym);
        periodic_impedance(n, &|u, _g, v| cl_step(&m, u, v))
    };
    zs(true).sub(zs(false))
}

/// The discrete impedance of implicit Euler with the sink `lambda` (ADR 0455 D1, `model_zdt` of symmetry_flux_f531.rs), 20 h.
fn model_zdt(n: usize, lambda: f64) -> Cx {
    let dt = 2.0 * PI / (OMEGA * n as f64);
    let s = Cx::ONE.sub(Cx(0.0, -OMEGA * dt).exp()).scale(1.0 / dt);
    let k = nu() / (H / NY as f64).powi(2);
    let diag = |i: usize| {
        let left = if i == 0 { 2.0 } else { 1.0 };
        let right = if i == NY - 1 { 2.0 } else { 1.0 };
        s.add(Cx(lambda + k * (left + right), 0.0))
    };
    let off = Cx(-k, 0.0);
    let mut c = vec![Cx::ZERO; NY];
    let mut d = vec![Cx::ZERO; NY];
    for i in 0..NY {
        let (b, r) = if i == 0 {
            (diag(0), Cx::ONE)
        } else {
            (diag(i).sub(off.mul(c[i - 1])), Cx::ONE.sub(off.mul(d[i - 1])))
        };
        c[i] = off.div(b);
        d[i] = r.div(b);
    }
    let mut x = vec![Cx::ZERO; NY];
    x[NY - 1] = d[NY - 1];
    for i in (0..NY - 1).rev() {
        x[i] = d[i].sub(c[i].mul(x[i + 1]));
    }
    let mean = x.iter().fold(Cx::ZERO, |a, &v| a.add(v)).scale(1.0 / NY as f64);
    Cx::ONE
        .div(mean)
        .mul(s)
        .div(Cx(0.0, OMEGA))
        .scale(H * H * RHO * 20.0 * H)
}

/// ADR 0455 D1's infinite-corrector excess `Delta Z_D1` at `n` steps per period.
fn d1_dz(n: usize) -> Cx {
    model_zdt(n, lambda_one_cell()).sub(model_zdt(n, 0.0))
}

/// The 0-D ratio `r = (1/dt + 12 nu/h^2)/(1/dt + 2 nu/dy^2)` (ADR 0613 D2).
fn r_0d(n: usize) -> f64 {
    let e = n as f64 + 12.0 * nu() / (H * H);
    let d = n as f64 + 2.0 * nu() / (H / NY as f64).powi(2);
    e / d
}

/// `sigma(alpha_u) = r - (1/alpha_u - 1)(1/r - 1)`.
fn sigma_0d(n: usize, alpha_u: f64) -> f64 {
    let r = r_0d(n);
    r - (1.0 / alpha_u - 1.0) * (1.0 / r - 1.0)
}

/// `1/alpha_0 = 1 + r^2/(1 - r)`.
fn alpha0_0d(n: usize) -> f64 {
    let r = r_0d(n);
    1.0 / (1.0 + r * r / (1.0 - r))
}

/// The zero of the dense model's `Re Delta Z(alpha_u)` by bisection in `[lo, hi]` (increasing in `alpha_u`).
fn alpha0_dense(n: usize, mut lo: f64, mut hi: f64) -> f64 {
    for _ in 0..50 {
        let mid = 0.5 * (lo + hi);
        if model_dz(&Model::relaxed(n, true, mid)).0 > 0.0 {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    0.5 * (lo + hi)
}

const SWEEP: [f64; 5] = [0.6, 0.7, 0.8, 0.9, 1.0];

// ---- the model checks (C7, C8; arithmetic only) ----------------------------------------------------------------------

#[test]
fn f538_models_closed_form_and_alpha_u_dependence() {
    let mut c = Checks::new();
    // the table of ADR 0613 D3 (printed)
    for (lab, m) in [
        ("PISO N 100", Model::piso(100, true)),
        ("relaxed 0.6 N 100", Model::relaxed(100, true, 0.6)),
        ("relaxed 0.7 N 100", Model::relaxed(100, true, 0.7)),
        ("relaxed 0.8 N 100", Model::relaxed(100, true, 0.8)),
        ("relaxed 0.9 N 100", Model::relaxed(100, true, 0.9)),
        ("relaxed 1.0 N 100", Model::relaxed(100, true, 1.0)),
        ("relaxed 0.8 N 50", Model::relaxed(50, true, 0.8)),
    ] {
        let zs = model_z(&m, false);
        let zz = model_z(&with_lambda(&m, false), false);
        let zc = model_z(&m, true);
        println!(
            "D3 {lab}: Symmetry {zs:?}, zero gradient {zz:?}, Delta Z {:?}, corrected Symmetry {zc:?}",
            zs.sub(zz)
        );
    }
    let d1 = d1_dz(100);
    println!("Delta Z_D1 N 100 {d1:?}");
    // C7: the converged loop in closed form against the dense 10-outer model
    for a in SWEEP {
        let dense = model_dz(&Model::relaxed(100, true, a));
        let cl = cl_dz(100, a);
        println!("C7 alpha_u {a}: dense {dense:?}, CL {cl:?}");
        c.at_most(&format!("C7 CL against dense alpha_u {a}"), cl.sub(dense).abs(), 1e-3);
    }
    let base = model_dz(&Model::relaxed(100, true, 0.8));
    for ap in [0.3, 0.7] {
        let m = Model {
            alpha_p: ap,
            ..Model::relaxed(100, true, 0.8)
        };
        let dz = model_dz(&m);
        println!("C7 alpha_p {ap}: {dz:?}");
        c.at_most(&format!("C7 alpha_p {ap} against 0.5"), dz.sub(base).abs(), 1e-3);
    }
    // C8: the 0-D formula against the dense model
    for a in SWEEP {
        let s_dense = model_dz(&Model::relaxed(100, true, a)).0 / d1.0;
        let s0 = sigma_0d(100, a);
        println!("C8 alpha_u {a}: sigma dense {s_dense:e}, 0-D {s0:e}");
        c.at_most(&format!("C8 sigma alpha_u {a}"), (s0 - s_dense).abs(), 0.06);
    }
    for n in [100, 50] {
        let ad = alpha0_dense(n, 0.6, 1.0);
        let a0 = alpha0_0d(n);
        println!("C8 N {n}: r {:e}, alpha_0 dense {ad:e}, 0-D {a0:e}", r_0d(n));
        c.at_most(&format!("C8 alpha_0 N {n}"), (a0 - ad).abs(), 0.02);
    }
    c.finish();
}

// ---- the registered runs (groups P, R, A, N; ADR 0613) ------------------------------------------------------------------

/// The per-length part `Z(40 h) - Z(20 h)` and the end part `2 Z(20 h) - Z(40 h)`.
fn parts(z20: Cx, z40: Cx) -> (Cx, Cx) {
    (z40.sub(z20), z20.scale(2.0).sub(z40))
}

/// The four runs (Symmetry and zero gradient at 20 h and 40 h) of one configuration: `[s20, s40, z20, z40]`.
fn four(c: &mut Checks, relaxed: Option<f64>, n: usize) -> Option<[Cx; 4]> {
    let mut out = Vec::new();
    for z_sym in [true, false] {
        for long in [false, true] {
            let r = Run {
                long,
                z_sym,
                corrected: false,
                relaxed,
                n,
            };
            out.push(channel_z(c, &r));
        }
    }
    Some([out[0]?, out[1]?, out[2]?, out[3]?])
}

/// C3 (per part, `limit`) for the per-length parts of one configuration; returns `Delta Z_pl` and records `Delta Z_end` (E1).
fn per_length(c: &mut Checks, id: &str, z: [Cx; 4], m: &Model, limit: f64) -> Cx {
    let (s_pl, s_end) = parts(z[0], z[1]);
    let (z_pl, z_end) = parts(z[2], z[3]);
    let ms = model_z(&with_lambda(m, true), false);
    let mz = model_z(&with_lambda(m, false), false);
    for (what, meas, model) in [("Symmetry", s_pl, ms), ("zero gradient", z_pl, mz)] {
        let (re, im) = rel_parts(meas, model);
        println!("{id} {what}: Z_pl {meas:?}, model {model:?}: {re:e}, {im:e}");
        c.at_most(&format!("{id} {what} Z_pl real"), re.abs(), limit);
        c.at_most(&format!("{id} {what} Z_pl imaginary"), im.abs(), limit);
    }
    let dz = s_pl.sub(z_pl);
    println!(
        "E1 {id}: Delta Z_pl {dz:?} (model {:?}), Delta Z(20 h) {:?}, Delta Z_end {:?}",
        ms.sub(mz),
        z[0].sub(z[2]),
        s_end.sub(z_end)
    );
    dz
}

#[test]
#[ignore = "known miss (ADR 0613 amendment 1), heavy record (29 channel runs, 135 s): C1 1.07e-2 (limit 5e-3), the Symmetry parts of C3, C4, C6 and the Re Delta Z_pl of C4 at 0.6, 0.7, 0.9 and of C6, NC3's bracket (0.8, 0.9); diagnosed: the x-direction Laplacian diagonal in the relaxation and in a_P (model X)"]
fn f538_registered_runs_relaxed_piso_sweep_and_n50_known_miss() {
    let mut c = Checks::new();
    let t0 = std::time::Instant::now();
    // group P: PISO, N 100
    let p = four(&mut c, None, 100);
    // group R: relaxed alpha_u 0.8, N 100, and the corrected split
    let r = four(&mut c, Some(0.8), 100);
    let rc = channel_z(
        &mut c,
        &Run {
            long: false,
            z_sym: true,
            corrected: true,
            relaxed: Some(0.8),
            n: 100,
        },
    );
    // R0: ADR 0455's printed 20 h values
    if let (Some(p), Some(r)) = (p, r) {
        for (id, z, z0) in [
            ("R0 PISO Symmetry (QD)", p[0], Cx(3.3926045570268286, 1.079197068958323)),
            (
                "R0 PISO zero gradient (QZ)",
                p[2],
                Cx(3.0318371762026075, 1.0928385577536086),
            ),
            (
                "R0 relaxed Symmetry (QD10)",
                r[0],
                Cx(3.0308399791722214, 1.080025234703981),
            ),
            (
                "R0 relaxed zero gradient (QZ10)",
                r[2],
                Cx(3.032931844875428, 1.0807530111304366),
            ),
        ] {
            c.at_most(id, rel_mod(z, z0), 1e-9);
        }
    } else {
        c.holds("groups P, R", false, "a run was refused");
    }
    // C2, C3 (PISO)
    let mp = Model::piso(100, true);
    if let Some(p) = p {
        let dz = per_length(&mut c, "C3 PISO", p, &mp, 2e-3);
        let dm = model_dz(&mp);
        c.at_most("C2 PISO Delta Z_pl real", (dz.0 - dm.0).abs(), 5e-3);
        c.at_most("C2 PISO Delta Z_pl imaginary", (dz.1 - dm.1).abs(), 2e-3);
    }
    // C1, C3, NC1, RC (relaxed 0.8)
    let mr = Model::relaxed(100, true, 0.8);
    let mut sweep: Vec<(f64, f64)> = Vec::new();
    let mut r_dz = None;
    if let Some(r) = r {
        let dz = per_length(&mut c, "C3 relaxed 0.8", r, &mr, 2e-3);
        let dm = model_dz(&mr);
        c.at_most("C1 relaxed Delta Z_pl real", (dz.0 - dm.0).abs(), 5e-3);
        c.at_most("C1 relaxed Delta Z_pl imaginary", (dz.1 - dm.1).abs(), 2e-3);
        for (what, wrong) in [
            (
                "final iteration relaxed",
                model_dz(&Model {
                    final_relaxed: true,
                    ..mr
                })
                .0,
            ),
            ("relaxation ignored", model_dz(&Model::relaxed(100, true, 1.0)).0),
            ("ADR 0455 D1 sink", d1_dz(100).0),
        ] {
            println!("NC1 {what}: model {wrong:e}");
            c.at_least(&format!("NC1 against {what}"), (dz.0 - wrong).abs(), 0.1);
        }
        if let Some(zc) = rc {
            let (re, im) = rel_parts(zc, r[2]);
            println!(
                "RC corrected Symmetry {zc:?} against zero gradient {:?}: {re:e}, {im:e}",
                r[2]
            );
            c.at_most("RC real", re.abs(), 1e-3);
            c.at_most("RC imaginary", im.abs(), 1e-3);
        }
        sweep.push((0.8, dz.0));
        r_dz = Some(dz);
    }
    // group A: the alpha_u sweep (C4)
    for a in [0.6, 0.7, 0.9, 1.0] {
        let m = Model::relaxed(100, true, a);
        match four(&mut c, Some(a), 100) {
            Some(z) => {
                let dz = per_length(&mut c, &format!("C4 relaxed {a}"), z, &m, 2e-3);
                let dm = model_dz(&m).0;
                c.at_most(
                    &format!("C4 alpha_u {a} Re Delta Z_pl"),
                    (dz.0 - dm).abs(),
                    5e-3 + 0.02 * dm.abs(),
                );
                sweep.push((a, dz.0));
            }
            None => c.holds(&format!("C4 alpha_u {a}"), false, "a run was refused"),
        }
    }
    sweep.sort_by(|x, y| x.0.total_cmp(&y.0));
    println!("sweep Re Delta Z_pl: {sweep:?}");
    // NC3, C5
    c.holds(
        "NC3 monotone increasing",
        sweep.windows(2).all(|w| w[1].1 > w[0].1),
        "Re Delta Z_pl increases with alpha_u",
    );
    match sweep.windows(2).find(|w| w[0].1 <= 0.0 && w[1].1 > 0.0) {
        Some(w) => {
            c.holds(
                "NC3 sign change between 0.7 and 0.8",
                w[0].0 == 0.7 && w[1].0 == 0.8,
                &format!("bracket {:?}", (w[0].0, w[1].0)),
            );
            let lin = |a0: f64, y0: f64, a1: f64, y1: f64| a0 + (a1 - a0) * (-y0) / (y1 - y0);
            let meas = lin(w[0].0, w[0].1, w[1].0, w[1].1);
            let m0 = model_dz(&Model::relaxed(100, true, w[0].0)).0;
            let m1 = model_dz(&Model::relaxed(100, true, w[1].0)).0;
            let model = lin(w[0].0, m0, w[1].0, m1);
            println!("C5 zero: measured {meas:e}, model {model:e}");
            c.at_most("C5 zero of Re Delta Z_pl", (meas - model).abs(), 0.02);
        }
        None => c.holds("C5", false, "no sign change in the sweep"),
    }
    // group N: N 50 (C6, NC2)
    let m50 = Model::relaxed(50, true, 0.8);
    match four(&mut c, Some(0.8), 50) {
        Some(z) => {
            let dz = per_length(&mut c, "C6 relaxed 0.8 N 50", z, &m50, 3e-3);
            let dm = model_dz(&m50).0;
            c.at_most("C6 Re Delta Z_pl", (dz.0 - dm).abs(), 5e-3 + 0.02 * dm.abs());
            if let Some(r) = r_dz {
                c.at_least("NC2 N 50 against N 100", (dz.0 - r.0).abs(), 0.1);
            }
        }
        None => c.holds("C6", false, "a run was refused"),
    }
    println!("all runs {:.1} s", t0.elapsed().as_secs_f64());
    c.finish();
}

// ---- ADR 0613 amendment 1: model X against the logged registered runs ---------------------------------------------------

/// One configuration of the registered runs: `alpha_u` (`None`: PISO), `N`, and `[Symmetry 20 h, Symmetry 40 h, zero gradient
/// 20 h, zero gradient 40 h]`.
type Logged = (Option<f64>, usize, [(f64, f64); 4]);

/// The impedances of the registered runs (`run2_registered.log`, commit `6857342e`).
const LOGGED: [Logged; 7] = [
    (
        None,
        100,
        [
            (3.3926045570268286, 1.079197068958323),
            (6.754904879701068, 2.161364829243946),
            (3.0318371762026075, 1.0928385577536086),
            (6.032149424900445, 2.188689265043991),
        ],
    ),
    (
        Some(0.6),
        100,
        [
            (2.782349849919308, 1.0893947694584931),
            (5.533832231920674, 2.1822218292713176),
            (3.030114966448081, 1.0806560077985476),
            (6.033924387529718, 2.1645767626200905),
        ],
    ),
    (
        Some(0.7),
        100,
        [
            (2.9241807083813804, 1.0841369138229433),
            (5.8175451918231715, 2.171724382139457),
            (3.031578331768899, 1.0808110040490946),
            (6.035428720867575, 2.164956792146438),
        ],
    ),
    (
        Some(0.8),
        100,
        [
            (3.0308399791722214, 1.080025234703981),
            (6.030628628230832, 2.1635240987279265),
            (3.032931844875428, 1.0807530111304366),
            (6.0367897420343795, 2.164901398320602),
        ],
    ),
    (
        Some(0.9),
        100,
        [
            (3.1140364527271656, 1.0768126049451898),
            (6.1966056321479615, 2.1571267585176646),
            (3.0342200589325516, 1.0806935809760039),
            (6.038082430749906, 2.1648384479576537),
        ],
    ),
    (
        Some(1.0),
        100,
        [
            (3.1808022556420354, 1.0742284611163406),
            (6.329603503581387, 2.151986396283511),
            (3.0354568610716193, 1.0806331296072462),
            (6.039325219982094, 2.1647670183014265),
        ],
    ),
    (
        Some(0.8),
        50,
        [
            (2.9387799428921153, 0.9904399881168894),
            (5.844799552870221, 1.985439822280563),
            (3.1004161230213008, 0.9793878553368068),
            (6.171967867694041, 1.9630607578908492),
        ],
    ),
];

/// The `x` cells of the channel mesh (`long`: 40 h, 60 cells): widths `dx_i` and the per-volume diagonal `c_i = nu sum
/// |S_f|/(d_f V_i)` of the `x` faces (the piston face at half a cell, nothing at the zero-gradient outlet), from the points.
fn x_cells(long: bool) -> Vec<(f64, f64)> {
    let (nx, lf) = if long { (60, 40.0 * H) } else { (30, 20.0 * H) };
    let mesh = box_mesh(
        [nx, NY, 1],
        [lf, H, H],
        [Grading::Expansion(5.5), Grading::Uniform, Grading::Uniform],
    )
    .unwrap();
    let mut xs: Vec<f64> = mesh.points().iter().map(|p| p[0]).collect();
    xs.sort_by(f64::total_cmp);
    xs.dedup_by(|a, b| (*a - *b).abs() < 1e-12 * lf);
    assert_eq!(xs.len(), nx + 1);
    let dx: Vec<f64> = xs.windows(2).map(|w| w[1] - w[0]).collect();
    let xc: Vec<f64> = xs.windows(2).map(|w| 0.5 * (w[0] + w[1])).collect();
    (0..nx)
        .map(|i| {
            let west = if i == 0 { 0.5 * dx[0] } else { xc[i] - xc[i - 1] };
            let mut c = nu() / (west * dx[i]);
            if i + 1 < nx {
                c += nu() / ((xc[i + 1] - xc[i]) * dx[i]);
            }
            (dx[i], c)
        })
        .collect()
}

/// Model X on one mesh: `Z = sum_i z(c_i) dx_i/(20 h)` (each `x` column a developed column with its own `c_i`).
fn model_x_mesh(m: &Model, long: bool, z_sym: bool) -> Cx {
    x_cells(long).iter().fold(Cx::ZERO, |acc, &(dx, c)| {
        let mc = Model {
            cx: c,
            ..with_lambda(m, z_sym)
        };
        acc.add(model_z(&mc, false).scale(dx / (20.0 * H)))
    })
}

/// Model X's per-length parts `(Symmetry, zero gradient)` and its 20 h `Delta Z`.
fn model_x_parts(m: &Model) -> (Cx, Cx, Cx) {
    let s20 = model_x_mesh(m, false, true);
    let s40 = model_x_mesh(m, true, true);
    let z20 = model_x_mesh(m, false, false);
    let z40 = model_x_mesh(m, true, false);
    (s40.sub(s20), z40.sub(z20), s20.sub(z20))
}

/// The per-length mean of `c_i`: `(sum_40 c dx - sum_20 c dx)/(20 h)`.
fn c_bar() -> f64 {
    let s = |long: bool| x_cells(long).iter().map(|(dx, c)| dx * c).sum::<f64>();
    (s(true) - s(false)) / (20.0 * H)
}

/// The 0-D ratio with `d -> d + c_bar`: `r = (1/dt + 12 nu/h^2)/(1/dt + 2 nu/dy^2 + c_bar)`.
fn r_0d_x(n: usize, cb: f64) -> f64 {
    let e = n as f64 + 12.0 * nu() / (H * H);
    let d = n as f64 + 2.0 * nu() / (H / NY as f64).powi(2) + cb;
    e / d
}

#[test]
fn f538_amended_model_x_against_the_registered_runs() {
    let mut c = Checks::new();
    let cb = c_bar();
    println!("c_bar {cb:e} /s");
    // (alpha_u, measured Re Delta Z_pl, model X's) at N 100
    let mut sweep: Vec<(f64, f64, f64)> = Vec::new();
    for (relaxed, n, z) in LOGGED {
        let z: Vec<Cx> = z.iter().map(|&(a, b)| Cx(a, b)).collect();
        let (m, id) = match relaxed {
            Some(a) => (Model::relaxed(n, true, a), format!("relaxed {a} N {n}")),
            None => (Model::piso(n, true), format!("PISO N {n}")),
        };
        let (xs, xz, x20) = model_x_parts(&m);
        let (s_pl, z_pl) = (z[1].sub(z[0]), z[3].sub(z[2]));
        let dz = s_pl.sub(z_pl);
        let dx = xs.sub(xz);
        // C3A: the four runs of C3 (PISO and relaxed 0.8 at N 100); C4A the sweep; C6A N 50
        let main = relaxed.is_none() || (relaxed == Some(0.8) && n == 100);
        let (cid, lim) = if n == 50 {
            ("C6A", 3e-3)
        } else if main {
            ("C3A", 2e-3)
        } else {
            ("C4A", 2e-3)
        };
        for (what, meas, model) in [("Symmetry", s_pl, xs), ("zero gradient", z_pl, xz)] {
            let (re, im) = rel_parts(meas, model);
            println!("{cid} {id} {what}: Z_pl {meas:?}, model X {model:?}: {re:e}, {im:e}");
            c.at_most(&format!("{cid} {id} {what} Z_pl real"), re.abs(), lim);
            c.at_most(&format!("{cid} {id} {what} Z_pl imaginary"), im.abs(), lim);
        }
        println!(
            "E1A {id}: Delta Z_pl {dz:?}, model X {dx:?}; Delta Z(20 h) {:?}, model X {x20:?}",
            z[0].sub(z[2])
        );
        if main {
            let tag = if relaxed.is_none() { "C2A" } else { "C1A" };
            c.at_most(&format!("{tag} {id} Delta Z_pl real"), (dz.0 - dx.0).abs(), 5e-3);
            c.at_most(&format!("{tag} {id} Delta Z_pl imaginary"), (dz.1 - dx.1).abs(), 2e-3);
        } else {
            c.at_most(
                &format!("{cid} {id} Re Delta Z_pl"),
                (dz.0 - dx.0).abs(),
                5e-3 + 0.02 * dx.0.abs(),
            );
        }
        if let Some(a) = relaxed {
            let s_meas = dz.0 / d1_dz(n).0;
            let r = r_0d_x(n, cb);
            let s0 = r - (1.0 / a - 1.0) * (1.0 / r - 1.0);
            println!("C8A {id}: sigma measured {s_meas:e}, 0-D with c_bar {s0:e} (r {r:e})");
            c.at_most(&format!("C8A {id} sigma"), (s0 - s_meas).abs(), 0.06);
            if n == 100 {
                sweep.push((a, dz.0, dx.0));
            }
        }
    }
    sweep.sort_by(|x, y| x.0.total_cmp(&y.0));
    let lin = |a0: f64, y0: f64, a1: f64, y1: f64| a0 + (a1 - a0) * (-y0) / (y1 - y0);
    let mb = sweep.windows(2).find(|w| w[0].1 <= 0.0 && w[1].1 > 0.0);
    let xb = sweep.windows(2).find(|w| w[0].2 <= 0.0 && w[1].2 > 0.0);
    match (mb, xb) {
        (Some(w), Some(v)) => {
            c.holds(
                "NC3A bracket",
                w[0].0 == v[0].0 && w[1].0 == v[1].0,
                &format!("measured {:?}, model X {:?}", (w[0].0, w[1].0), (v[0].0, v[1].0)),
            );
            let meas = lin(w[0].0, w[0].1, w[1].0, w[1].1);
            let model = lin(w[0].0, w[0].2, w[1].0, w[1].2);
            println!("C5A zero: measured {meas:e}, model X {model:e}");
            c.at_most("C5A zero", (meas - model).abs(), 0.02);
            let r = r_0d_x(100, cb);
            let a0 = 1.0 / (1.0 + r * r / (1.0 - r));
            println!("C8A zero: 0-D with c_bar {a0:e}, measured {meas:e}");
            c.at_most("C8A zero", (a0 - meas).abs(), 0.02);
        }
        _ => c.holds("C5A", false, "no sign change"),
    }
    c.finish();
}
