//! F-503 (ADR 0418): the second-order ring gradient of ADR 0409 (`OversetSolver::set_second_order_ring_gradient(true)`
//! under `FringeFlux::PrescribedOwnGradient`) on the set-ups RE and RL of ADR 0347 with the quadratic donor interpolation,
//! against the default coupling with the same stencils (the error runs of ADR 0347 amendment 2, HT); the orders of `R_b`
//! on RE and U1, C1, C2 of ADR 0347 under the opt-in. Helpers of `overset_f445.rs` copied.
//!
//! Release only (the flow runs take minutes): `cargo test --release -p rcfd-fv --test overset_f503`.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::overset::{FringeFlux, MassMode, OversetCoupling, OversetSolver};
use rcfd_fv::pimple::PimpleSettings;
use rcfd_fv::{FvGeometry, Profile, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::rectilinear_2d;
use rcfd_mesh::motion::{moved_mesh, Pose};
use rcfd_mesh::overset::{compose, CellKind, Connectivity, DonorInterpolation, Overset, OversetSpec, Stencil};
use rcfd_mesh::Mesh;
use std::f64::consts::PI;
use std::sync::Arc;
use std::time::Instant;

const DEPTH: f64 = 0.1;
const NU_TG: f64 = 0.1;
/// Background cell size of level 1.
const H1: f64 = PI / 40.0;
/// Half width of the component and of the hole (ADR 0232 RL).
const HALF: f64 = 7.5 * H1;
const HOLE: f64 = 3.1 * H1;
/// ADR 0219 PS.
const SOLVE: SolveControl = SolveControl {
    tol_abs: 1e-15,
    tol_rel: 1e-12,
    max_iter: 3000,
};
const QD: DonorInterpolation = DonorInterpolation::Quadratic;
const LD: DonorInterpolation = DonorInterpolation::Linear;

/// The couplings compared: the default, G of ADR 0347 (D5) and G with the second-order ring gradient of ADR 0409 (D6).
#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode {
    Default,
    G,
    Q,
}

fn apply(s: &mut OversetSolver, mode: Mode) {
    match mode {
        Mode::Default => {}
        Mode::G => s.set_fringe_flux(FringeFlux::PrescribedOwnGradient),
        Mode::Q => {
            s.set_fringe_flux(FringeFlux::PrescribedOwnGradient);
            s.set_second_order_ring_gradient(true);
        }
    }
}

fn settings(nu: f64) -> PimpleSettings {
    let mut s = PimpleSettings::piso(nu);
    s.convection = ConvScheme::Central;
    s.momentum_solver = SOLVE;
    s.pressure_solver = SOLVE;
    s
}

/// Records the failed checks and prints every check.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Self {
        Checks(Vec::new())
    }
    fn at_most(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{id}: {value:.4e} (limit <= {limit:.3e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{id}: {value:e} > {limit:e}"));
        }
    }
    fn at_least(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value >= limit;
        println!(
            "{id}: {value:.4e} (limit >= {limit:.3e}) {}",
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
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

// ---- set-ups ---------------------------------------------------------------------------------------------------------------

fn uniform(x0: f64, len: f64, n: usize) -> Vec<f64> {
    (0..=n).map(|i| x0 + len * i as f64 / n as f64).collect()
}

fn box_overset(bg: Mesh, half: f64, n: usize, hole: f64, mode: DonorInterpolation) -> Overset {
    let cs = uniform(-half, 2.0 * half, n);
    let comp = rectilinear_2d(&cs, &cs, DEPTH).unwrap();
    let composite = compose(&bg, &comp, "c_").unwrap();
    let spec = OversetSpec {
        outer_patches: ["c_xmin", "c_xmax", "c_ymin", "c_ymax"].map(String::from).to_vec(),
        hole: Arc::new(move |x: Vec3| x[0].abs().max(x[1].abs()) < hole),
    };
    let mut ov = Overset::new(composite, spec).unwrap();
    if mode != DonorInterpolation::Linear {
        ov.set_donor_interpolation(mode);
    }
    ov
}

/// One set-up of ADR 0340: the background at level `lb`, a component of half width `7.5 H` with `n_side` cells per side
/// whose centre lies at `(centre, centre)` (in units of `H`), `steps` steps to the time `end`.
#[derive(Clone, Copy)]
struct Case {
    lb: usize,
    n_side: usize,
    centre: f64,
    steps: usize,
    end: f64,
}

/// `RL(l, l)` of ADR 0246 with `200 l` steps to `t = 0.5`.
fn rl(l: usize) -> Case {
    Case {
        lb: l,
        n_side: 20 * l,
        centre: 16.5,
        steps: 200 * l,
        end: 0.5,
    }
}

/// RE of ADR 0340: equal spacings, both fringes at the fraction 0.25 or 0.75 of a donor cell along the whole ring.
fn re(l: usize) -> Case {
    Case {
        lb: l,
        n_side: 15 * l,
        centre: 16.5 + 0.25 / l as f64,
        steps: 200 * l,
        end: 0.5,
    }
}

impl Case {
    fn overset(&self, mode: DonorInterpolation) -> Overset {
        let bs = uniform(0.0, PI, 40 * self.lb);
        let bg = rectilinear_2d(&bs, &bs, DEPTH).unwrap();
        box_overset(bg, HALF, self.n_side, HOLE, mode)
    }
    fn pose(&self) -> Pose {
        let d = self.centre * H1;
        Pose {
            origin: [0.0; 3],
            axis: [0.0, 0.0, 1.0],
            angle: 0.0,
            translation: [d, d, 0.0],
        }
    }
}

fn tg_u(x: Vec3, t: f64) -> Vec3 {
    let e = (-2.0 * NU_TG * t).exp();
    [-x[0].cos() * x[1].sin() * e, x[0].sin() * x[1].cos() * e, 0.0]
}

fn tg_p(x: Vec3, t: f64) -> f64 {
    -((2.0 * x[0]).cos() + (2.0 * x[1]).cos()) * (-4.0 * NU_TG * t).exp() / 4.0
}

fn mesh_at(ov: &Overset, pose: &Pose) -> Mesh {
    moved_mesh(&ov.composite.mesh, ov.composite.points_at(pose)).unwrap()
}

fn geometry_at(ov: &Overset, pose: &Pose) -> Arc<FvGeometry> {
    Arc::new(FvGeometry::new(Arc::new(mesh_at(ov, pose))).unwrap())
}

const SIDES: [&str; 4] = ["xmin", "xmax", "ymin", "ymax"];

fn tg_conditions(geom: &FvGeometry, u: &mut VectorField, t: f64) {
    for name in SIDES {
        let v: Vec<Vec3> = geom
            .mesh
            .patch(name)
            .unwrap()
            .faces()
            .map(|f| tg_u(geom.mesh.face_centres()[f], t))
            .collect();
        u.set_bc(geom, name, Box::new(FixedValue(Profile::PerFace(v)))).unwrap();
    }
}

/// Observed order between two values on meshes whose spacings have the ratio `ratio` (coarse over fine).
fn order(coarse: f64, fine: f64, ratio: f64) -> f64 {
    (coarse / fine).ln() / ratio.ln()
}

/// The stencil of the cell `c` in `conn`.
fn stencil_of(conn: &Connectivity, c: usize) -> Option<&Stencil> {
    conn.stencils.iter().find(|(k, _)| *k == c).map(|(_, s)| s)
}

fn same_stencil(a: &Stencil, b: &Stencil) -> bool {
    a.cells == b.cells
        && a.weights.len() == b.weights.len()
        && a.weights
            .iter()
            .zip(&b.weights)
            .all(|(x, y)| x.to_bits() == y.to_bits())
}

/// The background fringe stencils of `conn` replaced by those of `quad` (ADR 0340 Qb).
fn substitute_background_fringe(conn: &mut Connectivity, quad: &Connectivity) {
    let nb = conn.n_background_cells;
    for (c, stn) in conn.stencils.iter_mut() {
        if *c < nb {
            *stn = stencil_of(quad, *c)
                .expect("a fringe cell of both connectivities")
                .clone();
        }
    }
}

/// ADR 0340 QS on a substituted connectivity `conn` against the linear one `lin`: the number of component fringe stencils
/// that differ from the linear ones, and the number of background fringe stencils that do not have nine calculated
/// component cells with the linear donor first.
fn qs(conn: &Connectivity, lin: &Connectivity) -> (usize, usize) {
    let nb = conn.n_background_cells;
    let (mut comp_changed, mut bg_bad) = (0, 0);
    for (c, stn) in &conn.stencils {
        let l = stencil_of(lin, *c).expect("the same fringe cells");
        if *c >= nb {
            comp_changed += usize::from(!same_stencil(stn, l));
        } else {
            let ok = stn.cells.len() == 9
                && stn.cells[0] == l.cells[0]
                && stn
                    .cells
                    .iter()
                    .all(|&k| k >= nb && conn.kind[k] == CellKind::Calculated);
            bg_bad += usize::from(!ok);
        }
    }
    (comp_changed, bg_bad)
}

fn area_max(geom: &FvGeometry) -> f64 {
    geom.mesh
        .face_area_vectors()
        .iter()
        .map(|s| vec3::norm(*s))
        .fold(0.0, f64::max)
}

/// Net flux through a patch.
fn patch_flux(geom: &FvGeometry, phi: &[f64], patch: &str) -> f64 {
    geom.mesh.patch(patch).unwrap().faces().map(|f| phi[f]).sum()
}

fn state_hash(s: &OversetSolver) -> u64 {
    let mut bytes = Vec::new();
    for v in &s.u.values {
        for x in v {
            bytes.extend_from_slice(&x.to_bits().to_le_bytes());
        }
    }
    for x in s.p.values.iter().chain(&s.phi) {
        bytes.extend_from_slice(&x.to_bits().to_le_bytes());
    }
    bytes.extend_from_slice(&s.time.to_bits().to_le_bytes());
    rcfd_core::hash::fnv64(&bytes)
}

// ---- runs ------------------------------------------------------------------------------------------------------------------

/// A Taylor-Green solver on the set-up `case` at `t = 0` under `mode` with the donor interpolation `donors` on both
/// fringes.
fn tg_solver(case: &Case, mode: Mode, donors: DonorInterpolation) -> OversetSolver {
    let ov = case.overset(donors);
    let pose = case.pose();
    let geom = geometry_at(&ov, &pose);
    let mut u = VectorField::from_fn("U", &geom, |x| tg_u(x, 0.0));
    tg_conditions(&geom, &mut u, 0.0);
    let p = ScalarField::from_fn("p", &geom, |x| tg_p(x, 0.0));
    let mut s = OversetSolver::new(
        geom,
        u,
        p,
        settings(NU_TG),
        MassMode::Conservative,
        Some(OversetCoupling { overset: ov, pose }),
    )
    .unwrap();
    apply(&mut s, mode);
    s
}

/// Volume-weighted mean square.
#[derive(Default, Clone, Copy)]
struct Acc {
    w: f64,
    sq: f64,
}

impl Acc {
    fn add(&mut self, weight: f64, v: f64) {
        self.w += weight;
        self.sq += weight * v * v;
    }
    fn rms(&self) -> f64 {
        (self.sq / self.w.max(1e-300)).sqrt()
    }
}

/// The largest velocity error over the calculated cells and its volume-weighted root mean square, at the time `t`.
fn velocity_error(s: &OversetSolver, t: f64) -> (f64, f64) {
    let centres = s.geom.mesh.cell_centres();
    let vol = s.geom.mesh.cell_volumes();
    let (mut max, mut acc) = (0.0f64, Acc::default());
    for c in 0..s.geom.n_cells() {
        if s.conn.kind[c] == CellKind::Calculated {
            let e = vec3::norm(vec3::sub(s.u.values[c], tg_u(centres[c], t)));
            max = max.max(e);
            acc.add(vol[c], e);
        }
    }
    (max, acc.rms())
}

/// What a run gives: C1, C2, C3, the fallbacks, whether it took every step (F).
struct Tally {
    cont: f64,
    /// Largest `abs(hole_flux)` and largest net flux through the domain boundary, over the largest face area.
    mass: (f64, f64),
    unconverged: usize,
    /// Largest `n_fallback` over the steps.
    fallback: usize,
    failed: Option<(usize, String)>,
    secs: f64,
}

impl Tally {
    fn new() -> Self {
        Tally {
            cont: 0.0,
            mass: (0.0, 0.0),
            unconverged: 0,
            fallback: 0,
            failed: None,
            secs: 0.0,
        }
    }
    /// Records one step; `false` when the velocity passed 10 (the run ends).
    fn step(&mut self, k: usize, s: &OversetSolver, info: &rcfd_fv::overset::OversetInfo, amax: f64) -> bool {
        self.cont = self.cont.max(info.continuity);
        self.unconverged += info.unconverged;
        self.fallback = self.fallback.max(info.n_fallback);
        let boundary: f64 = SIDES.iter().map(|n| patch_flux(&s.geom, &s.phi, n)).sum();
        self.mass = (
            self.mass.0.max(info.hole_flux.abs() / amax),
            self.mass.1.max(boundary.abs() / amax),
        );
        let umax = s.u.values.iter().map(|v| vec3::norm(*v)).fold(0.0, f64::max);
        if umax.is_nan() || umax > 10.0 {
            self.failed = Some((k, format!("max abs(u) = {umax:e}")));
            return false;
        }
        true
    }
}

/// The error run of ADR 0347 HT (`error_run` of `overset_f445.rs`): `case` under `mode` with `donors` on both fringes,
/// `steps` calls of `OversetSolver::step` to `t = 0.5`; a step that returns `Err`, or a velocity above 10, ends it.
/// Returns the root mean square of the velocity error over the calculated cells at the end (over the decay factor),
/// the state hash and the tally.
fn error_run(case: &Case, mode: Mode, donors: DonorInterpolation) -> (f64, u64, Tally) {
    let t0 = Instant::now();
    let dt = case.end / case.steps as f64;
    let mut s = tg_solver(case, mode, donors);
    let amax = area_max(&s.geom);
    let mut y = Tally::new();
    for k in 1..=case.steps {
        let g = s.geom.clone();
        tg_conditions(&g, &mut s.u, k as f64 * dt);
        match s.step(dt, case.pose(), &[]) {
            Ok(info) => {
                if !y.step(k, &s, &info, amax) {
                    break;
                }
            }
            Err(e) => {
                y.failed = Some((k, e));
                break;
            }
        }
    }
    y.secs = t0.elapsed().as_secs_f64();
    let decay = (-2.0 * NU_TG * case.end).exp();
    let e = if y.failed.is_some() {
        f64::NAN
    } else {
        velocity_error(&s, case.end).1 / decay
    };
    (e, state_hash(&s), y)
}

/// A run of ADR 0347 (`run` of `overset_f445.rs`) with the linear stencils under `mode`; with `qb` the background fringe
/// takes the quadratic stencils. Returns the solver at `t = 0.5`, QS of the first step, the fallbacks of the quadratic
/// connectivity among the background fringe cells, and the tally.
fn r_run(case: &Case, qb: bool, mode: Mode) -> (OversetSolver, (usize, usize), usize, Tally) {
    let t0 = Instant::now();
    let dt = case.end / case.steps as f64;
    let mut s = tg_solver(case, mode, LD);
    let pose = case.pose();
    let lin = s.conn.clone();
    let nb = lin.n_background_cells;
    let quad = qb.then(|| case.overset(QD).connectivity(&s.geom.mesh, &pose).unwrap());
    let fallbacks = quad
        .as_ref()
        .map_or(0, |q| q.fallbacks.iter().filter(|&&c| c < nb).count());
    let amax = area_max(&s.geom);
    let mut y = Tally::new();
    let mut qs_first = (0, 0);
    for k in 1..=case.steps {
        let t = k as f64 * dt;
        let g = s.geom.clone();
        tg_conditions(&g, &mut s.u, t);
        let mut step = s.prepare(dt, pose, &[]).unwrap();
        if let Some(q) = &quad {
            substitute_background_fringe(&mut step.conn, q);
        }
        if k == 1 {
            qs_first = qs(&step.conn, &lin);
        }
        match s.advance(dt, step) {
            Ok(info) => {
                if !y.step(k, &s, &info, amax) {
                    break;
                }
            }
            Err(e) => {
                y.failed = Some((k, e));
                break;
            }
        }
    }
    y.secs = t0.elapsed().as_secs_f64();
    (s, qs_first, fallbacks, y)
}

/// `R_b` at the time `t` (ADR 0340): the root mean square over the calculated component cells of the difference of the
/// velocities of the two runs, weighted with the cell volumes, over the decay factor.
fn r_b(base: &OversetSolver, var: &OversetSolver, t: f64) -> f64 {
    let nb = base.conn.n_background_cells;
    let vol = base.geom.mesh.cell_volumes();
    let decay = (-2.0 * NU_TG * t).exp();
    let mut acc = Acc::default();
    for c in nb..base.geom.n_cells() {
        if base.conn.kind[c] == CellKind::Calculated {
            acc.add(vol[c], vec3::norm(vec3::sub(base.u.values[c], var.u.values[c])));
        }
    }
    acc.rms() / decay
}

/// F, C1, C2, C3 of a run; C1 and C2 are checked under a prescribed mode (ADR 0347 registers them for P and G) and
/// printed for the default coupling.
fn tally_checks(tag: &str, mode: Mode, y: &Tally, c: &mut Checks) {
    c.holds(
        &format!("F {tag}: every step taken"),
        y.failed.is_none(),
        &format!("{:?}", y.failed),
    );
    if mode == Mode::Default {
        println!(
            "{tag} (printed): continuity {:.3e}, flux through Gamma_B {:.3e}, through the domain boundary {:.3e}",
            y.cont, y.mass.0, y.mass.1
        );
    } else {
        c.at_most(&format!("C1 {tag} continuity"), y.cont, 1e-9);
        c.at_most(&format!("C2 {tag} flux through Gamma_B"), y.mass.0, 1e-9);
        c.at_most(&format!("C2 {tag} flux through the domain boundary"), y.mass.1, 1e-9);
    }
    c.holds(
        &format!("C3 {tag}: no unconverged linear solve"),
        y.unconverged == 0,
        &format!("{} unconverged", y.unconverged),
    );
}

/// The error runs of one level, all with the quadratic stencils.
struct Level {
    q: f64,
    default: f64,
    /// `NaN` when G is not run.
    g: f64,
    hash_q: u64,
    hash_g: u64,
}

/// The error runs of one level: Q, the default coupling and, with `with_g`, G, with F, C1 to C3 and QD.
fn error_level(set: &str, case: &Case, l: usize, with_g: bool, c: &mut Checks) -> Level {
    let mut out = Level {
        q: f64::NAN,
        default: f64::NAN,
        g: f64::NAN,
        hash_q: 0,
        hash_g: 0,
    };
    let modes: &[Mode] = if with_g {
        &[Mode::Q, Mode::Default, Mode::G]
    } else {
        &[Mode::Q, Mode::Default]
    };
    for &mode in modes {
        let tag = format!("{set} level {l} {mode:?} quadratic");
        let (e, hash, y) = error_run(case, mode, QD);
        println!(
            "RESULT {tag}: rms velocity error at t = 0.5 {e:.6e} ({:.1} s, largest n_fallback {}, hash {hash:016x})",
            y.secs, y.fallback
        );
        tally_checks(&tag, mode, &y, c);
        c.holds(
            &format!("QD {tag}: no fallback"),
            y.fallback == 0,
            &format!("largest n_fallback {}", y.fallback),
        );
        match mode {
            Mode::Q => (out.q, out.hash_q) = (e, hash),
            Mode::Default => out.default = e,
            Mode::G => (out.g, out.hash_g) = (e, hash),
        }
    }
    println!(
        "RESULT {set} level {l}: e(Q) / e(default) {:.6}, e(Q) / e(G) {:.6}, excess e(Q) - e(default) {:.6e}",
        out.q / out.default,
        out.q / out.g,
        out.q - out.default
    );
    out
}

/// The Q pair of one level of RE (linear base, Qb) with F, C1 to C3 and QS; returns `R_b(0.5)`.
fn r_level(case: &Case, l: usize, c: &mut Checks) -> f64 {
    let (base, qs_b, _, yb) = r_run(case, false, Mode::Q);
    let (var, qs_v, fb, yv) = r_run(case, true, Mode::Q);
    let tag = format!("RE level {l} Q pair");
    println!("{tag}: base {:.1} s, Qb {:.1} s", yb.secs, yv.secs);
    tally_checks(&format!("{tag} base"), Mode::Q, &yb, c);
    tally_checks(&format!("{tag} Qb"), Mode::Q, &yv, c);
    c.holds(
        &format!("QS {tag}: the base run keeps every stencil"),
        qs_b.0 == 0 && qs_b.1 == var.conn.n_fringe_background(),
        &format!(
            "{} component stencils changed, {} linear background stencils",
            qs_b.0, qs_b.1
        ),
    );
    c.holds(
        &format!("QS {tag}: Qb changes the background fringe only"),
        qs_v == (0, 0) && fb == 0,
        &format!(
            "{} component stencils changed, {} background stencils without nine calculated cells and the donor first, {fb} fallbacks",
            qs_v.0, qs_v.1
        ),
    );
    let r = if yb.failed.is_some() || yv.failed.is_some() {
        f64::NAN
    } else {
        r_b(&base, &var, case.end)
    };
    println!("RESULT {tag}: R_b(0.5) {r:.6e}");
    r
}

/// ADR 0418 DR: the HT numbers of ADR 0347 (`w49i-work/f445-scratch/f445-ht.log`), `e(default, quadratic)` and
/// `e(G, quadratic)` on RE at levels 1 to 3.
const HT_DEFAULT: [f64; 3] = [5.234643e-4, 1.303540e-4, 5.802028e-5];
const HT_G: [f64; 3] = [5.991924e-4, 1.376739e-4, 6.007299e-5];

fn dr(l: usize, lv: &Level, c: &mut Checks) {
    c.at_most(
        &format!("DR RE level {l}: e(default, quadratic) against HT, relative"),
        (lv.default / HT_DEFAULT[l - 1] - 1.0).abs(),
        1e-6,
    );
    c.at_most(
        &format!("DR RE level {l}: e(G, quadratic) against HT, relative"),
        (lv.g / HT_G[l - 1] - 1.0).abs(),
        1e-6,
    );
}

fn qg(lv: &Level, c: &mut Checks) {
    c.holds(
        "QG RE level 1: the opt-in changes the state",
        lv.hash_q != lv.hash_g,
        &format!("Q {:016x}, G {:016x}", lv.hash_q, lv.hash_g),
    );
}

/// HX of ADR 0418 on one set-up (printed; the decision is taken in the ADR by the registered rule): the excess `x_L =
/// e(Q) - e(default)` at levels 1 to 3, its orders, and the level at which the ratio would reach 1.05 if the excess and
/// the error of the default coupling kept their orders between levels 2 and 3.
fn hx(set: &str, q: &[f64; 3], d: &[f64; 3]) {
    let x: Vec<f64> = (0..3).map(|i| q[i] - d[i]).collect();
    let positive = x.iter().all(|&v| v > 0.0);
    let (o1, o2) = (order(x[0], x[1], 2.0), order(x[1], x[2], 1.5));
    let met = positive && o1 >= 2.7 && o2 >= 2.7;
    let oe = order(d[1], d[2], 1.5);
    // the ratio at the level n > 3: 1 + (x_3 / d_3) (3 / n)^(o2 - oe)
    let l_star = if positive && o2 > oe {
        3.0 * (x[2] / d[2] / 0.05).powf(1.0 / (o2 - oe))
    } else {
        f64::NAN
    };
    println!(
        "HX {set}: excess {:.4e}, {:.4e}, {:.4e}; orders {o1:.4}, {o2:.4} (limit >= 2.7, every excess positive): {}; default error order 2 to 3 {oe:.4}; ratio 1.05 extrapolated at level {l_star:.3}",
        x[0],
        x[1],
        x[2],
        if met { "HX met" } else { "HX NOT met" }
    );
}

// ---- U1 under the opt-in ---------------------------------------------------------------------------------------------------

const U_FLOW: Vec3 = [1.0, 0.5, 0.0];
const DT_U: f64 = 0.01;

fn u_pose(t: f64) -> Pose {
    Pose {
        origin: [0.0; 3],
        axis: [0.0, 0.0, 1.0],
        angle: t,
        translation: [t, 0.5 * t, 0.0],
    }
}

/// The set-up U of ADR 0219 at `t = 0`.
fn u_new() -> OversetSolver {
    let bs = uniform(-2.5, 5.0, 50);
    let bg = rectilinear_2d(&bs, &bs, DEPTH).unwrap();
    let ov = box_overset(bg, 0.6, 16, 0.25, LD);
    let pose = u_pose(0.0);
    let geom = geometry_at(&ov, &pose);
    let mut u = VectorField::uniform("U", &geom, U_FLOW);
    for name in ["xmin", "ymin", "ymax"] {
        u.set_bc(&geom, name, Box::new(FixedValue::vector(U_FLOW))).unwrap();
    }
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    OversetSolver::new(
        geom,
        u,
        p,
        settings(0.01),
        MassMode::Conservative,
        Some(OversetCoupling { overset: ov, pose }),
    )
    .unwrap()
}

/// U1 of ADR 0347 under `mode`, with C1, C2, C3 and U4 (`u_run` of `overset_f445.rs`).
fn u1(mode: Mode, c: &mut Checks) {
    let mut s = u_new();
    apply(&mut s, mode);
    let n = s.geom.n_cells();
    let un = vec3::norm(U_FLOW);
    let (mut eu, mut ep, mut cont, mut hole, mut q) = (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let (mut changes, mut unconverged) = (0, 0);
    for k in 1..=80 {
        let info = s.step(DT_U, u_pose(k as f64 * DT_U), &[]).unwrap();
        cont = cont.max(info.continuity);
        hole = hole.max(info.hole_flux.abs() / area_max(&s.geom));
        changes += info.n_changed;
        unconverged += info.unconverged;
        for c in 0..n {
            if s.conn.kind[c] != CellKind::Hole {
                eu = eu.max(vec3::norm(vec3::sub(s.u.values[c], U_FLOW)) / un);
                ep = ep.max(s.p.values[c].abs() / (un * un));
            }
        }
        let qx = patch_flux(&s.geom, &s.phi, "xmax");
        q = q.max((qx - 0.5).abs() / 0.5);
    }
    c.at_most("U1 U under Q: velocity", eu, 1e-10);
    c.at_most("U1 U under Q: pressure", ep, 1e-10);
    c.at_most("C1 U under Q: continuity", cont, 1e-9);
    c.at_most("C2 U under Q: flux through Gamma_B", hole, 1e-9);
    c.at_most("C2 U under Q: outflow", q, 1e-10);
    c.at_least("U4 (set-up) U under Q: kind changes", changes as f64, 20.0);
    c.holds(
        "C3 U under Q: no unconverged linear solve",
        unconverged == 0,
        &format!("{unconverged} unconverged"),
    );
}

// ---- the tests ---------------------------------------------------------------------------------------------------------------

/// ADR 0418, the fast test at level 1 (the level-1 runs of both heavy records, repeated; control DT): U1, C1, C2, C3, F,
/// QD, QS, QG and DR at level 1 asserted; E1, E2 at level 1 and `R_b` printed (asserted by the records).
#[test]
fn fast_level_1_re_rl_u1_under_the_opt_in() {
    let mut c = Checks::new();
    u1(Mode::Q, &mut c);
    let lv = error_level("RE", &re(1), 1, true, &mut c);
    println!(
        "E1 RE level 1 (printed here, asserted by the record): {:.6}",
        lv.q / lv.default
    );
    dr(1, &lv, &mut c);
    qg(&lv, &mut c);
    r_level(&re(1), 1, &mut c);
    let lv = error_level("RL", &rl(1), 1, false, &mut c);
    println!(
        "E2 RL level 1 (printed here, asserted by the record): {:.6}",
        lv.q / lv.default
    );
    c.finish();
}

/// ADR 0418 E1, A2 and the controls on RE at levels 1 to 3 (heavy record, run once, 517 s): all met. `e(Q) / e(default)`
/// 1.002457, 1.000666, 1.000379; orders of `R_b(0.5)` 1.964 and 1.789 (`5.753316e-4`, `1.474341e-4`, `7.137494e-5`).
#[test]
#[ignore = "slow (RE at levels 1 to 3: Q, default and G with the quadratic stencils, and the Q pair, ADR 0418, about twenty minutes): run once"]
fn re_levels_1_to_3() {
    let mut c = Checks::new();
    let (mut q, mut d, mut r) = ([0.0; 3], [0.0; 3], [0.0; 3]);
    for l in 1..=3usize {
        let lv = error_level("RE", &re(l), l, true, &mut c);
        dr(l, &lv, &mut c);
        if l == 1 {
            qg(&lv, &mut c);
        }
        c.at_most(
            &format!("E1 RE level {l}: e(Q, quadratic) / e(default, quadratic)"),
            lv.q / lv.default,
            1.05,
        );
        (q[l - 1], d[l - 1]) = (lv.q, lv.default);
        r[l - 1] = r_level(&re(l), l, &mut c);
    }
    c.at_least(
        "A2 order of R_b at t = 0.5 on RE under Q, levels 1 to 2",
        order(r[0], r[1], 2.0),
        1.7,
    );
    c.at_least(
        "A2 order of R_b at t = 0.5 on RE under Q, levels 2 to 3",
        order(r[1], r[2], 1.5),
        1.7,
    );
    hx("RE", &q, &d);
    c.finish();
}

/// ADR 0418 E2 and the controls on RL at levels 1 to 3 (heavy record, run once, 238 s): all met. `e(Q) / e(default)`
/// 1.001343, 0.999425, 0.999352.
#[test]
#[ignore = "slow (RL at levels 1 to 3: Q and default with the quadratic stencils, ADR 0418, about fifteen minutes): run once"]
fn rl_levels_1_to_3() {
    let mut c = Checks::new();
    let (mut q, mut d) = ([0.0; 3], [0.0; 3]);
    for l in 1..=3usize {
        let lv = error_level("RL", &rl(l), l, false, &mut c);
        c.at_most(
            &format!("E2 RL level {l}: e(Q, quadratic) / e(default, quadratic)"),
            lv.q / lv.default,
            1.05,
        );
        (q[l - 1], d[l - 1]) = (lv.q, lv.default);
    }
    hx("RL", &q, &d);
    c.finish();
}
