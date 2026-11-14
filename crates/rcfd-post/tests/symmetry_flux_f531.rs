//! F-531 (ADR 0455): the viscous flux of the `Symmetry` condition for flow tangential to the plane. The condition's own
//! coefficients give the tangential momentum exactly zero flux through an axis-aligned plane; the extra dissipation found by
//! ADR 0446 comes from the averaged component diagonal of the PISO correctors (`HbyA_k = H_k/a_P`, `a_P` the mean of the
//! component diagonals; the plane loads only the normal component's diagonal), a sink `(a_P - a_k) u_k` next to the plane.
//! The opt-in corrected split (`PimpleSettings::component_diagonal`, ADR 0010 / ADR 0069 F3) removes it exactly.
//!
//! Checked here: Stokes' second problem in a layer bounded by a `Symmetry` plane against the closed form `rho sqrt(i w nu)
//! tanh(k H)` and against exact linear models of the step (corrected: implicit Euler; default: the averaged correctors); the
//! oscillating channel of ADR 0446 between two `Symmetry` planes of a one-cell reduction with the corrected diagonal, and the
//! default's excess against the sink `lambda = 4 nu/(3 h^2)` of ADR 0455 D1.
//!
//! The limits, set-ups and mutations were registered in ADR 0455 before this file existed. The channel, its formula and its
//! run are those of `sa_aerodamp.rs` (ADR 0446; copied, that file is not edited).

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::{FixedValue, Symmetry, ZeroGradient};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::cosim::{FlowWall, Fluid, InterfaceMap};
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::sixdof::patch_face_forces;
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{box_2d, box_mesh, Grading};
use rcfd_mesh::motion::moved_mesh;
use rcfd_mesh::Mesh;
use rcfd_post::solid_aerodamp::{drive_mode, periodic_response, ModeHistory, ModeMotion, PeriodicResponse};
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
    fn arg(self) -> f64 {
        self.1.atan2(self.0)
    }
    /// `tanh(z) = (1 - e^{-2z})/(1 + e^{-2z})` (`Re z > 0`).
    fn tanh(self) -> Cx {
        let e = self.scale(-2.0).exp();
        Cx::ONE.sub(e).div(Cx::ONE.add(e))
    }
    /// The principal square root.
    fn sqrt(self) -> Cx {
        let (r, t) = (self.abs().sqrt(), 0.5 * self.arg());
        Cx(r * t.cos(), r * t.sin())
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

/// `2 z_fine - z_coarse` (first order in the step).
fn extrapolate(coarse: Cx, fine: Cx) -> Cx {
    fine.scale(2.0).sub(coarse)
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

// ---- Stokes' second problem in a layer bounded by a Symmetry plane (ADR 0455 D2, D3) -----------------------------------

const S_H: f64 = 1.0;
const S_RHO: f64 = 1000.0;
const S_Q0: f64 = 1e-3;
const S_CYCLES: usize = 4;

/// `nu = w H^2/2`: the Stokes depth `sqrt(2 nu/w)` equals `H`.
fn s_nu() -> f64 {
    OMEGA * S_H * S_H / 2.0
}

/// The closed form per area `rho nu k tanh(k H)`, `k = sqrt(i w/nu)` (ADR 0455 D2).
fn stokes_closed_form() -> Cx {
    let k = Cx(0.0, OMEGA / s_nu()).sqrt();
    k.mul(k.scale(S_H).tanh()).scale(S_RHO * s_nu())
}

/// The step of the column as `PimpleSolver::step` takes it (ADR 0455 D3): implicit predictor, then `n_corr` correctors with the
/// averaged diagonal (`corrected = false`) or the corrected split. Per volume; returns the new cell values.
fn column_step(ny: usize, n: usize, corrected: bool, n_corr: usize, u_old: &[f64], v: f64) -> Vec<f64> {
    let dt = 1.0 / n as f64;
    let kap = s_nu() / (S_H / ny as f64).powi(2);
    let ax: Vec<f64> = (0..ny)
        .map(|i| {
            let l = if i == 0 { 2.0 } else { 1.0 };
            let r = if i == ny - 1 { 0.0 } else { 1.0 };
            1.0 / dt + kap * (l + r)
        })
        .collect();
    let mut ay = ax.clone();
    ay[ny - 1] += 2.0 * kap;
    let ap: Vec<f64> = ax.iter().zip(&ay).map(|(x, y)| 0.5 * (x + y)).collect();
    let b: Vec<f64> = (0..ny)
        .map(|i| u_old[i] / dt + if i == 0 { 2.0 * kap * v } else { 0.0 })
        .collect();
    // predictor: the tridiagonal system (Thomas)
    let mut c = vec![0.0; ny];
    let mut d = vec![0.0; ny];
    for i in 0..ny {
        let (den, rhs) = if i == 0 {
            (ax[0], b[0])
        } else {
            (ax[i] + kap * c[i - 1], b[i] + kap * d[i - 1])
        };
        c[i] = -kap / den;
        d[i] = rhs / den;
    }
    let mut u = vec![0.0; ny];
    u[ny - 1] = d[ny - 1];
    for i in (0..ny - 1).rev() {
        u[i] = d[i] - c[i] * u[i + 1];
    }
    for _ in 0..n_corr {
        let h: Vec<f64> = (0..ny)
            .map(|i| {
                let lo = if i > 0 { u[i - 1] } else { 0.0 };
                let hi = if i + 1 < ny { u[i + 1] } else { 0.0 };
                b[i] + kap * (lo + hi)
            })
            .collect();
        u = (0..ny)
            .map(|i| {
                let extra = if corrected { (ap[i] - ax[i]) * u[i] } else { 0.0 };
                (h[i] + extra) / ap[i]
            })
            .collect();
    }
    u
}

/// The periodic state of the linear step `u_n = T u_{n-1} + g v_n` and its impedance per area `-(F/v) s/(i w)` (ADR 0455 D3).
fn column_model(ny: usize, n: usize, corrected: bool, n_corr: usize) -> Cx {
    let dt = 1.0 / n as f64;
    let zero = vec![0.0; ny];
    let g = column_step(ny, n, corrected, n_corr, &zero, 1.0);
    let z = Cx(0.0, -OMEGA * dt).exp();
    let mut a = vec![Cx::ZERO; ny * ny];
    for j in 0..ny {
        let mut e = zero.clone();
        e[j] = 1.0;
        let col = column_step(ny, n, corrected, n_corr, &e, 0.0);
        for i in 0..ny {
            a[i * ny + j] = z.scale(-col[i]);
        }
    }
    for i in 0..ny {
        a[i * ny + i] = a[i * ny + i].add(Cx::ONE);
    }
    let u = solve_complex(a, g.iter().map(|&x| Cx(x, 0.0)).collect());
    let dy = S_H / ny as f64;
    let f_per_v = u[0].sub(Cx::ONE).scale(S_RHO * s_nu() / (0.5 * dy));
    let s = Cx::ONE.sub(z).scale(1.0 / dt);
    f_per_v.mul(s).div(Cx(0.0, OMEGA)).scale(-1.0)
}

fn s_settings(corrected: bool) -> PimpleSettings {
    let mut s = PimpleSettings::piso(s_nu());
    s.convection = ConvScheme::Central;
    for c in [&mut s.momentum_solver, &mut s.pressure_solver] {
        c.tol_abs = 1e-16;
        c.tol_rel = 1e-13;
        c.max_iter = 5000;
    }
    s.component_diagonal = corrected;
    s
}

/// The measured impedance per area of the column of `ny` cells at `n` steps per period (ADR 0455 D2).
fn stokes_run(ny: usize, n: usize, corrected: bool) -> Result<Cx, String> {
    let t0 = std::time::Instant::now();
    let dy = S_H / ny as f64;
    let mesh = box_2d(1, ny, dy, S_H, false, false).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
    u.set_bc(&geom, "ymax", Box::new(Symmetry)).unwrap();
    for name in ["xmin", "xmax"] {
        u.set_bc(&geom, name, Box::new(ZeroGradient)).unwrap();
    }
    u.set_bc(&geom, "ymin", Box::new(FixedValue::vector(vec3::ZERO)))
        .unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    let mut fl = PimpleSolver::new(geom.clone(), u, p, s_settings(corrected));
    let wall = vec!["ymin".to_string()];
    let patch = geom.mesh.patch("ymin").unwrap();
    let area: f64 = patch
        .faces()
        .map(|f| vec3::norm(geom.mesh.face_area_vectors()[f]))
        .sum();
    let dt = 1.0 / n as f64;
    let q = |m: usize| S_Q0 * (OMEGA * m as f64 * dt).sin();
    let mut f = Vec::with_capacity(S_CYCLES * n);
    for k in 1..=S_CYCLES * n {
        let v = (q(k) - q(k - 1)) / dt;
        fl.u.set_bc(&geom, "ymin", Box::new(FixedValue::vector([v, 0.0, 0.0])))
            .unwrap();
        fl.step(dt);
        if let Some(d) = &fl.diverged {
            return Err(format!("step {k}: {}", d.message(None)));
        }
        let loads = patch_face_forces(&fl.geom, &fl.u, &fl.p, S_RHO, s_nu(), &wall, None)?;
        f.push(loads.iter().map(|x| x[0]).sum::<f64>());
    }
    let r = periodic_response(&f, n, S_Q0, OMEGA, TOL_PERIODIC)?;
    let (re, im) = r.impedance();
    let max_uy = fl.u.values.iter().fold(0.0f64, |m, v| m.max(v[1].abs()));
    println!(
        "Stokes n_y {ny} N {n} corrected {corrected}: Z/A {:?}, cycle change {:e}, max |u_y| {max_uy:e} ({:.1} s)",
        Cx(re / area, im / area),
        r.cycle_change,
        t0.elapsed().as_secs_f64()
    );
    Ok(Cx(re / area, im / area))
}

/// Runs `stokes_run` and records a refusal as a miss.
fn stokes_level(c: &mut Checks, id: &str, ny: usize, n: usize, corrected: bool) -> Option<Cx> {
    match stokes_run(ny, n, corrected) {
        Ok(z) => Some(z),
        Err(m) => {
            c.holds(id, false, &m);
            None
        }
    }
}

#[test]
#[cfg_attr(debug_assertions, ignore = "run in release")]
fn s1_stokes_layer_corrected_against_the_closed_form() {
    let mut c = Checks::new();
    let zc = stokes_closed_form();
    println!("closed form Z/A {zc:?}");
    // S1a, S1b: n_y = 20
    let mut z20 = Vec::new();
    for n in [1000, 2000] {
        let m = column_model(20, n, true, 2);
        if let Some(z) = stokes_level(&mut c, &format!("S1a n_y 20 N {n}"), 20, n, true) {
            println!("S1a n_y 20 N {n}: model IE {m:?}");
            c.at_most(&format!("S1a n_y 20 N {n} against model IE"), rel_mod(z, m), 1e-6);
            z20.push(z);
        }
    }
    if let [z1, z2] = z20[..] {
        let ext = extrapolate(z1, z2);
        let (re, im) = rel_parts(ext, zc);
        println!("S1b Z_ext {ext:?}: {re:e}, {im:e}");
        c.at_most("S1b n_y 20 Z_ext real", re.abs(), 3e-3);
        c.at_most("S1b n_y 20 Z_ext imaginary", im.abs(), 3e-3);
    }
    // S1c, S1d: n_y = 40
    let mut z40 = Vec::new();
    for n in [4000, 8000] {
        let m = column_model(40, n, true, 2);
        if let Some(z) = stokes_level(&mut c, &format!("S1c n_y 40 N {n}"), 40, n, true) {
            println!("S1c n_y 40 N {n}: model IE {m:?}");
            c.at_most(&format!("S1c n_y 40 N {n} against model IE"), rel_mod(z, m), 1e-6);
            z40.push(z);
        }
    }
    if let [z1, z2] = z40[..] {
        let ext = extrapolate(z1, z2);
        let (re, im) = rel_parts(ext, zc);
        println!("S1c Z_ext {ext:?}: {re:e}, {im:e}");
        c.at_most("S1c n_y 40 Z_ext real", re.abs(), 1e-3);
        c.at_most("S1c n_y 40 Z_ext imaginary", im.abs(), 1e-3);
        // S1d: the wall shear's amplitude per rho sqrt(w nu) V and its phase
        let tanh = Cx(0.0, OMEGA / s_nu()).sqrt().scale(S_H).tanh();
        let amp = ext.abs() / (S_RHO * (OMEGA * s_nu()).sqrt());
        println!("S1d amplitude {amp:e} against |tanh(kH)| {:e}", tanh.abs());
        c.at_most("S1d wall shear amplitude", (amp / tanh.abs() - 1.0).abs(), 1e-3);
        c.at_most("S1d wall shear phase [rad]", (ext.arg() - zc.arg()).abs(), 1e-3);
    }
    c.finish();
}

#[test]
#[cfg_attr(debug_assertions, ignore = "run in release")]
fn s0_stokes_layer_default_against_the_averaged_step() {
    let mut c = Checks::new();
    let zc = stokes_closed_form();
    let mut z20 = Vec::new();
    for n in [1000, 2000] {
        let m = column_model(20, n, false, 2);
        if let Some(z) = stokes_level(&mut c, &format!("S0a n_y 20 N {n}"), 20, n, false) {
            println!("S0a n_y 20 N {n}: model AV {m:?}");
            c.at_most(&format!("S0a n_y 20 N {n} against model AV"), rel_mod(z, m), 1e-6);
            z20.push(z);
        }
    }
    if let [z1, z2] = z20[..] {
        let ext = extrapolate(z1, z2);
        let (re, im) = rel_parts(ext, zc);
        println!("S0b Z_ext {ext:?}: {re:e}, {im:e}");
        c.at_least("S0b (NC1) default real error", re, 0.1);
        c.at_least("S0b (NC1) default minus imaginary error", -im, 0.1);
    }
    let m = column_model(40, 4000, false, 2);
    if let Some(z) = stokes_level(&mut c, "S0c n_y 40 N 4000", 40, 4000, false) {
        println!("S0c n_y 40 N 4000: model AV {m:?}");
        c.at_most("S0c n_y 40 N 4000 against model AV", rel_mod(z, m), 1e-6);
        c.at_least("S0c n_y 40 N 4000 real error", rel_parts(z, zc).0, 0.25);
    }
    // the converged-corrector limit of D3 (printed only)
    for ny in [20, 40] {
        let n = 1000 * (ny / 20) * (ny / 20);
        println!(
            "model AV with 200 correctors, n_y {ny} N {n}: real error {:e}",
            rel_parts(column_model(ny, n, false, 200), zc).0
        );
    }
    c.finish();
}

// ---- the plane channel between two Symmetry planes (ADR 0455 D4; the code of sa_aerodamp.rs) ---------------------------

const H: f64 = 0.02;
const RHO: f64 = 1000.0;
const WALL: &str = "xmin";

/// `n_outer > 1`: `PimpleSettings::pimple(nu, n_outer)` (relaxed, one corrector; the registered Q4), or with `piso_outer`
/// the `piso` settings with `n_outer` outer iterations (ADR 0455 amendment 1).
fn settings(nu: f64, corrected: bool, n_outer: usize, piso_outer: bool) -> PimpleSettings {
    let mut s = if n_outer > 1 && !piso_outer {
        PimpleSettings::pimple(nu, n_outer)
    } else {
        PimpleSettings::piso(nu)
    };
    s.n_outer = n_outer;
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
struct ChannelCase {
    nx: usize,
    ny: usize,
    lf: f64,
    expansion: f64,
    wo: f64,
    cycles: usize,
    /// `Symmetry` on the `z` planes; `false`: the default zero gradient.
    z_sym: bool,
    /// `PimpleSettings::component_diagonal`.
    corrected: bool,
    n_outer: usize,
    /// The outer iterations on the `piso` settings (ADR 0455 amendment 1) instead of `PimpleSettings::pimple`.
    piso_outer: bool,
}

impl ChannelCase {
    /// QF of ADR 0446 (`Wo = 1`, 8 cells across, `20 h`), `Symmetry` planes, the default diagonal.
    fn qf() -> Self {
        ChannelCase {
            nx: 30,
            ny: 8,
            lf: 20.0 * H,
            expansion: 5.5,
            wo: 1.0,
            cycles: 3,
            z_sym: true,
            corrected: false,
            n_outer: 1,
            piso_outer: false,
        }
    }
    fn nu(&self) -> f64 {
        OMEGA * (H / 2.0).powi(2) / (self.wo * self.wo)
    }
}

fn channel_run(case: &ChannelCase, n: usize) -> ModeHistory {
    let mesh0 = box_mesh(
        [case.nx, case.ny, 1],
        [case.lf, H, H],
        [Grading::Expansion(case.expansion), Grading::Uniform, Grading::Uniform],
    )
    .unwrap();
    let nu = case.nu();
    let motion = ChannelMotion::new(&mesh0, WALL, 0, case.lf, 1e-9).unwrap();
    let mut fluid = FlowWall::new(
        flow(
            &mesh0,
            mesh0.points().to_vec(),
            settings(nu, case.corrected, case.n_outer, case.piso_outer),
            &["ymin", "ymax"],
            if case.z_sym { &["zmin", "zmax"] } else { &[] },
        ),
        WALL,
        &mesh0,
        RHO,
        nu,
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
        steps_per_cycle: n,
        cycles: case.cycles,
    };
    drive_mode(&mut fluid, &map, &motion).unwrap()
}

/// `A i w rho L_f/(1 - tanh(kappa)/kappa)`, `kappa = (h/2) sqrt(s/nu)`, `s = i w + lambda` (ADR 0446 D3; ADR 0455 D1 with the
/// sink `lambda`, 0 for the formula itself).
fn impedance_formula(case: &ChannelCase, lambda: f64) -> Cx {
    let s = Cx(lambda, OMEGA);
    let kappa = s.scale(1.0 / case.nu()).sqrt().scale(H / 2.0);
    let t = kappa.tanh().div(kappa);
    s.scale(H * H * RHO * case.lf).div(Cx::ONE.sub(t))
}

/// The discrete impedance `A rho L_f G(s + lambda) s/(i w)` of implicit Euler on the cell-centred profile across the gap
/// (ADR 0451 D2; ADR 0455 D1 with the sink `lambda`).
fn model_zdt(case: &ChannelCase, n: usize, lambda: f64) -> Cx {
    let ny = case.ny;
    let dt = 2.0 * PI / (OMEGA * n as f64);
    let s = Cx::ONE.sub(Cx(0.0, -OMEGA * dt).exp()).scale(1.0 / dt);
    let k = case.nu() / (H / ny as f64).powi(2);
    let diag = |i: usize| {
        let left = if i == 0 { 2.0 } else { 1.0 };
        let right = if i == ny - 1 { 2.0 } else { 1.0 };
        s.add(Cx(lambda + k * (left + right), 0.0))
    };
    let off = Cx(-k, 0.0);
    let mut c = vec![Cx::ZERO; ny];
    let mut d = vec![Cx::ZERO; ny];
    for i in 0..ny {
        let (b, r) = if i == 0 {
            (diag(0), Cx::ONE)
        } else {
            (diag(i).sub(off.mul(c[i - 1])), Cx::ONE.sub(off.mul(d[i - 1])))
        };
        c[i] = off.div(b);
        d[i] = r.div(b);
    }
    let mut x = vec![Cx::ZERO; ny];
    x[ny - 1] = d[ny - 1];
    for i in (0..ny - 1).rev() {
        x[i] = d[i].sub(c[i].mul(x[i + 1]));
    }
    let mean = x.iter().fold(Cx::ZERO, |a, &v| a.add(v)).scale(1.0 / ny as f64);
    Cx::ONE
        .div(mean)
        .mul(s)
        .div(Cx(0.0, OMEGA))
        .scale(H * H * RHO * case.lf)
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

fn channel_z(c: &mut Checks, id: &str, case: &ChannelCase, n: usize) -> Option<Cx> {
    let t0 = std::time::Instant::now();
    let h = channel_run(case, n);
    let z = response_of(c, id, &h).map(|r| {
        let (re, im) = r.impedance();
        Cx(re, im)
    });
    println!("{id}: Z {z:?} ({:.1} s)", t0.elapsed().as_secs_f64());
    z
}

/// The sink of the one-cell-deep reduction (ADR 0455 D1): `4 nu/(3 h^2)`.
fn lambda_one_cell(case: &ChannelCase) -> f64 {
    4.0 * case.nu() / (3.0 * H * H)
}

#[test]
#[cfg_attr(debug_assertions, ignore = "run in release")]
fn q_channel_between_symmetry_planes_corrected_and_the_default_excess() {
    let mut c = Checks::new();
    let qd = ChannelCase::qf();
    let qs = ChannelCase { corrected: true, ..qd };
    let qz = ChannelCase { z_sym: false, ..qd };
    let zf = impedance_formula(&qd, 0.0);
    println!("formula {zf:?}");
    let mut zs = Vec::new();
    let mut z100 = None;
    for n in [50, 100] {
        let s = channel_z(&mut c, &format!("QS N {n}"), &qs, n);
        let d = channel_z(&mut c, &format!("QD N {n}"), &qd, n);
        let z = channel_z(&mut c, &format!("QZ N {n}"), &qz, n);
        if let (Some(s), Some(z)) = (s, z) {
            let (re, im) = rel_parts(s, z);
            c.at_most(&format!("Q1 QS/QZ N {n} real"), re.abs(), 1e-3);
            c.at_most(&format!("Q1 QS/QZ N {n} imaginary"), im.abs(), 1e-3);
            zs.push(s);
        }
        if n == 100 {
            z100 = d.zip(z);
        }
    }
    if let [z1, z2] = zs[..] {
        let ext = extrapolate(z1, z2);
        let (re, im) = rel_parts(ext, zf);
        println!("Q2 QS Z_ext {ext:?}: {re:e}, {im:e}");
        c.at_most("Q2 QS Z_ext real against the formula", re.abs(), 0.2);
        c.at_most("Q2 QS Z_ext imaginary against the formula", im.abs(), 0.2);
    }
    let dm = model_zdt(&qd, 100, lambda_one_cell(&qd)).sub(model_zdt(&qd, 100, 0.0));
    println!("Delta Z_model {dm:?}");
    if let Some((d, z)) = z100 {
        let dz = d.sub(z);
        println!("Q3 Delta Z {dz:?}");
        c.at_most("Q3 Re Delta Z/Re Delta Z_model - 1", (dz.0 / dm.0 - 1.0).abs(), 0.15);
    }
    // NC2: the corrected diagonal on zero-gradient planes changes nothing
    let qzc = ChannelCase { corrected: true, ..qz };
    let a = channel_z(&mut c, "NC2 QZ corrected N 100", &qzc, 100);
    if let (Some(a), Some((_, z))) = (a, z100) {
        c.at_most("NC2 QZ corrected against QZ default", rel_mod(a, z), 1e-12);
    }
    c.finish();
}

#[test]
#[ignore = "registered record with a defective set-up (ADR 0455 amendment 1): the relaxed pimple constructor; Q4c is the corrected run"]
fn q4_default_excess_with_ten_outer_iterations() {
    let mut c = Checks::new();
    let qd = ChannelCase {
        n_outer: 10,
        ..ChannelCase::qf()
    };
    let qz = ChannelCase { z_sym: false, ..qd };
    let d = channel_z(&mut c, "QD10 N 100", &qd, 100);
    let z = channel_z(&mut c, "QZ10 N 100", &qz, 100);
    let dm = model_zdt(&qd, 100, lambda_one_cell(&qd)).sub(model_zdt(&qd, 100, 0.0));
    if let (Some(d), Some(z)) = (d, z) {
        let dz = d.sub(z);
        println!("Q4 Delta Z {dz:?}, model {dm:?}");
        c.at_most("Q4 Re Delta Z/Re Delta Z_model - 1", (dz.0 / dm.0 - 1.0).abs(), 5e-2);
    }
    c.finish();
}

#[test]
#[cfg_attr(debug_assertions, ignore = "run in release")]
fn q4c_default_excess_with_ten_piso_outer_iterations() {
    let mut c = Checks::new();
    let qd = ChannelCase {
        n_outer: 10,
        piso_outer: true,
        ..ChannelCase::qf()
    };
    let qz = ChannelCase { z_sym: false, ..qd };
    let d = channel_z(&mut c, "QD10c N 100", &qd, 100);
    let z = channel_z(&mut c, "QZ10c N 100", &qz, 100);
    let dm = model_zdt(&qd, 100, lambda_one_cell(&qd)).sub(model_zdt(&qd, 100, 0.0));
    if let (Some(d), Some(z)) = (d, z) {
        let dz = d.sub(z);
        println!("Q4c Delta Z {dz:?}, model {dm:?}");
        c.at_most("Q4c Re Delta Z/Re Delta Z_model - 1", (dz.0 / dm.0 - 1.0).abs(), 5e-2);
    }
    c.finish();
}

#[test]
#[ignore = "heavy record (ADR 0455 WS, about 200 s): run once with --ignored"]
fn ws_womersley_5_between_symmetry_planes_corrected_record() {
    let mut c = Checks::new();
    let case = ChannelCase {
        nx: 120,
        ny: 40,
        lf: 100.0 * H,
        expansion: 21.0,
        wo: 5.0,
        cycles: 6,
        z_sym: true,
        corrected: true,
        n_outer: 1,
        piso_outer: false,
    };
    let zf = impedance_formula(&case, 0.0);
    println!("WS formula {zf:?}");
    let z: Vec<Option<Cx>> = [100, 200, 400]
        .into_iter()
        .map(|n| channel_z(&mut c, &format!("WS N {n}"), &case, n))
        .collect();
    if let [Some(_), Some(z2), Some(z4)] = z[..] {
        let ext = extrapolate(z2, z4);
        let (re, im) = rel_parts(ext, zf);
        println!("WS Z_ext {ext:?}: {re:e}, {im:e}");
        c.at_most("W1 WS real", re.abs(), 3e-2);
        c.at_most("W1 WS imaginary", im.abs(), 3e-2);
        c.at_most("W2 WS real minus W5c's", (re.abs() - 8.29e-3).abs(), 2e-3);
        c.at_most("W2 WS imaginary minus W5c's", (im.abs() - 1.58e-3).abs(), 2e-3);
    } else {
        c.holds("WS", false, "a run was refused");
    }
    c.finish();
}
