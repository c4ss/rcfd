//! F-445 (ADR 0347): the prescribed flux on the faces between calculated and fringe cells of the overset step
//! (`OversetSolver::set_fringe_flux`). The set-ups RE and RL, the runs `base` and `Qb`, the response `R_b` and the
//! following coefficient `a_c` are those of ADR 0340 (`overset_f349.rs`, helpers copied), the uniform flow U that of ADR
//! 0219 (`overset_f134.rs`); UA is a uniform flow with a constant acceleration.
//!
//! Release only (the flow runs take minutes): `cargo test --release -p rcfd-fv --test overset_f445`.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
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
const G: FringeFlux = FringeFlux::PrescribedOwnGradient;
const P: FringeFlux = FringeFlux::Prescribed;
const DEFAULT: FringeFlux = FringeFlux::PressureEquation;

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
    /// Distance `max(|x - x_c|, |y - y_c|)` of a point from the component centre.
    fn rho(&self, x: Vec3) -> f64 {
        let c = self.centre * H1;
        (x[0] - c).abs().max((x[1] - c).abs())
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

// ---- flow runs -------------------------------------------------------------------------------------------------------------

/// The state of a run at one time.
struct Snap {
    t: f64,
    u: Vec<Vec3>,
    p: Vec<f64>,
    phi: Vec<f64>,
}

struct Run {
    s: OversetSolver,
    /// Largest continuity over the steps (C1).
    cont: f64,
    /// Largest `abs(hole_flux)` and largest net flux through the domain boundary, over the largest face area (C2).
    mass: (f64, f64),
    /// Unconverged linear solves over the steps (C3).
    unconverged: usize,
    /// QS of the first step: component fringe stencils changed, background fringe stencils that are not the quadratic ones.
    qs: (usize, usize),
    /// Fallbacks of the quadratic connectivity among the background fringe cells.
    fallbacks: usize,
    snaps: Vec<Snap>,
    /// The step at which the run ended early and why (ADR 0347 NP).
    failed: Option<(usize, String)>,
    /// Largest velocity error against the exact solution over the calculated cells, after every step.
    err_hist: Vec<f64>,
    secs: f64,
}

/// Times at which a run to `t = 0.5` keeps its state; every one is a whole number of steps at every level.
const TIMES: [f64; 5] = [0.0025, 0.025, 0.125, 0.25, 0.5];

/// A Taylor-Green solver on the set-up `case` at `t = 0` with the fringe mode `mode`.
fn tg_solver(case: &Case, mode: FringeFlux) -> OversetSolver {
    let ov = case.overset(DonorInterpolation::Linear);
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
    if mode != DEFAULT {
        s.set_fringe_flux(mode);
    }
    s
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

/// A Taylor-Green run on the set-up `case` with the fringe mode `mode`; with `qb` the background fringe takes the
/// quadratic stencils of the library. A step that fails, or a velocity above 10, ends the run early (`failed`).
fn run(case: &Case, qb: bool, mode: FringeFlux, times: &[f64]) -> Run {
    let t0 = Instant::now();
    let dt = case.end / case.steps as f64;
    let mut s = tg_solver(case, mode);
    let pose = case.pose();
    let lin = s.conn.clone();
    let nb = lin.n_background_cells;
    let quad = qb.then(|| {
        case.overset(DonorInterpolation::Quadratic)
            .connectivity(&s.geom.mesh, &pose)
            .unwrap()
    });
    let fallbacks = quad
        .as_ref()
        .map_or(0, |q| q.fallbacks.iter().filter(|&&c| c < nb).count());
    let amax = area_max(&s.geom);
    let (mut cont, mut unconverged, mut mass) = (0.0f64, 0usize, (0.0f64, 0.0f64));
    let mut qs_first = (0, 0);
    let mut snaps = Vec::new();
    let mut failed = None;
    let mut err_hist = Vec::new();
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
        let info = match s.advance(dt, step) {
            Ok(i) => i,
            Err(e) => {
                failed = Some((k, e));
                break;
            }
        };
        cont = cont.max(info.continuity);
        unconverged += info.unconverged;
        let boundary: f64 = SIDES.iter().map(|n| patch_flux(&s.geom, &s.phi, n)).sum();
        mass = (
            mass.0.max(info.hole_flux.abs() / amax),
            mass.1.max(boundary.abs() / amax),
        );
        err_hist.push(velocity_error(&s, t).0);
        let umax = s.u.values.iter().map(|v| vec3::norm(*v)).fold(0.0, f64::max);
        if !(umax <= 10.0) {
            failed = Some((k, format!("max abs(u) = {umax:e}")));
            break;
        }
        if times.iter().any(|ts| (ts - t).abs() < 0.25 * dt) {
            snaps.push(Snap {
                t,
                u: s.u.values.clone(),
                p: s.p.values.clone(),
                phi: s.phi.clone(),
            });
        }
    }
    assert!(
        failed.is_some() || snaps.len() == times.len(),
        "a snapshot time is not a whole number of steps"
    );
    Run {
        s,
        cont,
        mass,
        unconverged,
        qs: qs_first,
        fallbacks,
        snaps,
        failed,
        err_hist,
        secs: t0.elapsed().as_secs_f64(),
    }
}

/// Volume-weighted mean square.
#[derive(Default, Clone, Copy)]
struct Acc {
    w: f64,
    sum: f64,
    sq: f64,
}

impl Acc {
    fn add(&mut self, weight: f64, v: f64) {
        self.w += weight;
        self.sum += weight * v;
        self.sq += weight * v * v;
    }
    fn mean(&self) -> f64 {
        self.sum / self.w.max(1e-300)
    }
    fn rms(&self) -> f64 {
        (self.sq / self.w.max(1e-300)).sqrt()
    }
}

/// `R_b` at the snapshot `k` (ADR 0340): the root mean square over the calculated component cells of the difference of
/// the velocities of the two runs, weighted with the cell volumes, over the decay factor of that time.
fn r_b(base: &Run, var: &Run, k: usize) -> f64 {
    let s = &base.s;
    let nb = s.conn.n_background_cells;
    let vol = s.geom.mesh.cell_volumes();
    let decay = (-2.0 * NU_TG * base.snaps[k].t).exp();
    let mut acc = Acc::default();
    for c in nb..s.geom.n_cells() {
        if s.conn.kind[c] == CellKind::Calculated {
            let d = vec3::norm(vec3::sub(base.snaps[k].u[c], var.snaps[k].u[c]));
            acc.add(vol[c], d);
        }
    }
    acc.rms() / decay
}

/// The root mean square of the velocity error of the snapshot `k` over the calculated cells of both parts (over the
/// decay factor), and of the pressure error with its mean over the calculated cells of each part removed.
fn exact_errors(run: &Run, k: usize) -> (f64, f64) {
    let s = &run.s;
    let nb = s.conn.n_background_cells;
    let vol = s.geom.mesh.cell_volumes();
    let centres = s.geom.mesh.cell_centres();
    let snap = &run.snaps[k];
    let decay = (-2.0 * NU_TG * snap.t).exp();
    let mut eu = Acc::default();
    let mut ep = [Acc::default(); 2];
    for c in 0..s.geom.n_cells() {
        if s.conn.kind[c] == CellKind::Calculated {
            eu.add(vol[c], vec3::norm(vec3::sub(snap.u[c], tg_u(centres[c], snap.t))));
            ep[usize::from(c >= nb)].add(vol[c], snap.p[c] - tg_p(centres[c], snap.t));
        }
    }
    let var = |a: &Acc| (a.sq / a.w - a.mean() * a.mean()).max(0.0) * a.w;
    (
        eu.rms() / decay,
        ((var(&ep[0]) + var(&ep[1])) / (ep[0].w + ep[1].w)).sqrt(),
    )
}

/// The following coefficients of ADR 0340 on the faces between a calculated and a fringe cell of one part.
#[derive(Clone, Copy, Default)]
struct Follow {
    a_c: f64,
    a_m: f64,
}

/// The printed numbers of ADR 0340 for the difference `d` of two runs at the snapshot `k`; returns the following
/// coefficients on `Gamma_B` and on `Gamma_C`.
fn diagnostics(tag: &str, case: &Case, base: &Run, var: &Run, k: usize) -> [Follow; 2] {
    let s = &base.s;
    let g = &s.geom;
    let n = g.n_cells();
    let nb = s.conn.n_background_cells;
    let kind = &s.conn.kind;
    let vol = g.mesh.cell_volumes();
    let centres = g.mesh.cell_centres();
    let (a, b) = (&base.snaps[k], &var.snaps[k]);
    let t = a.t;
    let decay = (-2.0 * NU_TG * t).exp();
    let d: Vec<Vec3> = (0..n)
        .map(|c| vec3::scale(vec3::sub(a.u[c], b.u[c]), 1.0 / decay))
        .collect();
    // classes: CF, CC, BF and the calculated background cells within and beyond 7.5 H of the component centre
    let mut cls = [Acc::default(); 5];
    for c in 0..n {
        let i = match (kind[c], c < nb) {
            (CellKind::Hole, _) => continue,
            (CellKind::Fringe, false) => 0,
            (CellKind::Calculated, false) => 1,
            (CellKind::Fringe, true) => 2,
            (CellKind::Calculated, true) => 3 + usize::from(case.rho(centres[c]) >= HALF),
        };
        cls[i].add(vol[c], vec3::norm(d[c]));
    }
    println!(
        "{tag} t = {t}: rms of d: CC {:.4e}, CF {:.4e}, BF {:.4e}, BC rho < 7.5 H {:.4e}, BC beyond {:.4e}",
        cls[1].rms(),
        cls[0].rms(),
        cls[2].rms(),
        cls[3].rms(),
        cls[4].rms()
    );
    // the faces between a calculated and a fringe cell of each part
    let sf = g.mesh.face_area_vectors();
    let (own, nei) = (g.mesh.owner(), g.mesh.neighbour());
    let mut out = [Follow::default(); 2];
    for (i, (part, name)) in [(true, "Gamma_B"), (false, "Gamma_C")].into_iter().enumerate() {
        let (mut tt, mut cc, mut mm, mut ct, mut mt) = (0.0, 0.0, 0.0, 0.0, 0.0);
        let mut jump = Acc::default();
        for f in 0..g.n_internal {
            let (o, m) = (own[f], nei[f]);
            if (o < nb) != part {
                continue;
            }
            let (calc, fringe, sign) = match (kind[o], kind[m]) {
                (CellKind::Calculated, CellKind::Fringe) => (o, m, 1.0),
                (CellKind::Fringe, CellKind::Calculated) => (m, o, -1.0),
                _ => continue,
            };
            let area = vec3::norm(sf[f]);
            if area == 0.0 {
                continue;
            }
            let nrm = vec3::scale(sf[f], sign / area);
            let tf = vec3::dot(d[fringe], nrm);
            let cf = vec3::dot(d[calc], nrm);
            let mf = sign * (a.phi[f] - b.phi[f]) / area / decay;
            tt += tf * tf;
            cc += cf * cf;
            mm += mf * mf;
            ct += cf * tf;
            mt += mf * tf;
            jump.add(1.0, (a.p[fringe] - b.p[fringe]) - (a.p[calc] - b.p[calc]));
        }
        let cnt = jump.w.max(1.0);
        out[i] = Follow {
            a_c: ct / tt.max(1e-300),
            a_m: mt / tt.max(1e-300),
        };
        println!(
            "{tag} t = {t}: {name} ({} faces): rms of t {:.4e}, of c {:.4e}, of m {:.4e}; a_c {:.4}, a_m {:.4}; change of the pressure jump rms {:.4e} (mean removed)",
            jump.w,
            (tt / cnt).sqrt(),
            (cc / cnt).sqrt(),
            (mm / cnt).sqrt(),
            out[i].a_c,
            out[i].a_m,
            (jump.sq / cnt - jump.mean() * jump.mean()).max(0.0).sqrt()
        );
    }
    out
}

/// What a pair of runs (base, Qb) gives.
struct Pair {
    /// `R_b` at the times of [`TIMES`].
    r: Vec<f64>,
    /// The following coefficients on `Gamma_B` and `Gamma_C` at `t = 0.025` and at `t = 0.5`.
    early: [Follow; 2],
    late: [Follow; 2],
    /// Root mean square of the velocity error of the base run at `t = 0.5` over the calculated cells.
    err: f64,
}

/// C1, C2, C3 of a run under a prescribed mode.
fn conservation(tag: &str, run: &Run, c: &mut Checks) {
    c.holds(
        &format!("{tag}: the run reaches the end"),
        run.failed.is_none(),
        &format!("{:?}", run.failed),
    );
    c.at_most(&format!("C1 {tag} continuity"), run.cont, 1e-9);
    c.at_most(&format!("C2 {tag} flux through Gamma_B"), run.mass.0, 1e-9);
    c.at_most(&format!("C2 {tag} flux through the domain boundary"), run.mass.1, 1e-9);
    c.holds(
        &format!("C3 {tag}: no unconverged linear solve"),
        run.unconverged == 0,
        &format!("{} unconverged", run.unconverged),
    );
}

/// The base run and the Qb run of `case` under `mode` with C1 to C3 and QS, the diagnostics, and `R_b`.
fn pair(tag: &str, case: &Case, mode: FringeFlux, c: &mut Checks) -> Pair {
    let base = run(case, false, mode, &TIMES);
    let var = run(case, true, mode, &TIMES);
    println!(
        "{tag}: {} cells, {} steps to t = {}; base {:.1} s, Qb {:.1} s",
        base.s.geom.n_cells(),
        case.steps,
        case.end,
        base.secs,
        var.secs
    );
    conservation(&format!("{tag} base"), &base, c);
    conservation(&format!("{tag} Qb"), &var, c);
    c.holds(
        &format!("QS {tag}: the base run keeps every stencil"),
        base.qs.0 == 0 && base.qs.1 == var.s.conn.n_fringe_background(),
        &format!(
            "{} component stencils changed, {} linear background stencils",
            base.qs.0, base.qs.1
        ),
    );
    c.holds(
        &format!("QS {tag}: Qb changes the background fringe only"),
        var.qs == (0, 0) && var.fallbacks == 0,
        &format!(
            "{} component stencils changed, {} background stencils without nine calculated cells and the donor first, {} fallbacks",
            var.qs.0, var.qs.1, var.fallbacks
        ),
    );
    if base.failed.is_some() || var.failed.is_some() {
        return Pair {
            r: vec![f64::NAN; TIMES.len()],
            early: [Follow::default(); 2],
            late: [Follow::default(); 2],
            err: f64::NAN,
        };
    }
    let r: Vec<f64> = (0..TIMES.len()).map(|k| r_b(&base, &var, k)).collect();
    let mut follow = [[Follow::default(); 2]; 2];
    for (k, t) in TIMES.iter().enumerate() {
        println!("RESULT {tag} t = {t}: R_b {:.6e}", r[k]);
        if *t == 0.025 {
            follow[0] = diagnostics(tag, case, &base, &var, k);
        }
        if *t == 0.5 {
            follow[1] = diagnostics(tag, case, &base, &var, k);
        }
    }
    let (eu, ep) = exact_errors(&base, TIMES.len() - 1);
    println!(
        "{tag} base at t = 0.5: rms velocity error {eu:.6e}, rms pressure error (mean of each part removed) {ep:.6e}"
    );
    Pair {
        r,
        early: follow[0],
        late: follow[1],
        err: eu,
    }
}

const FRINGES: [&str; 2] = ["Gamma_B", "Gamma_C"];

/// S1 and S2 of ADR 0347 for one pair.
fn s1_s2(tag: &str, pr: &Pair, c: &mut Checks) {
    for (i, name) in FRINGES.iter().enumerate() {
        for (when, f) in [("t = 0.025", pr.early[i]), ("t = 0.5", pr.late[i])] {
            c.at_most(
                &format!("S1 {tag} {name} {when}: abs(a_m - (1 + a_c) / 2)"),
                (f.a_m - 0.5 * (1.0 + f.a_c)).abs(),
                0.02,
            );
        }
        c.at_least(
            &format!("S2 {tag} {name}: a_c(0.025) / a_c(0.5)"),
            pr.early[i].a_c / pr.late[i].a_c,
            0.9,
        );
    }
}

/// A1 of ADR 0347 for one pair.
fn a1(tag: &str, pr: &Pair, c: &mut Checks) {
    for (i, name) in FRINGES.iter().enumerate() {
        c.at_least(&format!("A1 {tag} {name}: a_c at t = 0.025"), pr.early[i].a_c, 0.9);
    }
}

// ---- B1: without the opt-in ------------------------------------------------------------------------------------------------

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
    let ov = box_overset(bg, 0.6, 16, 0.25, DonorInterpolation::Linear);
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

/// The two runs of B1: 40 steps of RE at level 1 and 20 steps of U; `explicit` sets the default mode by a call.
fn b1_hashes(explicit: bool) -> (u64, u64) {
    let case = re(1);
    let mut s = tg_solver(&case, DEFAULT);
    let mut v = u_new();
    if explicit {
        s.set_fringe_flux(FringeFlux::PressureEquation);
        v.set_fringe_flux(FringeFlux::PressureEquation);
    }
    let dt = 0.0025;
    for k in 1..=40 {
        let g = s.geom.clone();
        tg_conditions(&g, &mut s.u, k as f64 * dt);
        s.step(dt, case.pose(), &[]).unwrap();
    }
    for k in 1..=20 {
        v.step(DT_U, u_pose(k as f64 * DT_U), &[]).unwrap();
    }
    (state_hash(&s), state_hash(&v))
}

/// ADR 0347 B1: without the opt-in the overset step gives the bytes the library gave before F-445. The two hashes were
/// printed by these runs on the library of main 8165a0b3 (the worktree at the registration commit, whose `crates` are
/// those of 8165a0b3), before `overset.rs` was edited.
#[test]
fn b1_without_the_opt_in_every_bit_is_kept() {
    const RE_8165A0B3: u64 = 0x65f3bb871ad94a6d;
    const U_8165A0B3: u64 = 0x7c424f2255adfe35;
    for explicit in [false, true] {
        let (re_hash, u_hash) = b1_hashes(explicit);
        println!("B1 (mode set by a call: {explicit}): RE {re_hash:016x}, U {u_hash:016x}");
        assert_eq!(re_hash, RE_8165A0B3, "RE at level 1, 40 steps");
        assert_eq!(u_hash, U_8165A0B3, "U, 20 steps");
    }
}

// ---- U1: the uniform flow of ADR 0219 --------------------------------------------------------------------------------------

/// The run U under `mode`: the largest velocity and pressure deviations, continuity, `abs(hole_flux)` over the largest
/// face area, outflow error, the number of kind changes and of unconverged solves.
fn u_run(mode: FringeFlux) -> (f64, f64, f64, f64, f64, usize, usize) {
    let mut s = u_new();
    s.set_fringe_flux(mode);
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
    (eu, ep, cont, hole, q, changes, unconverged)
}

/// ADR 0347 U1 (with C1, C2, C3 and the set-up check U4): the uniform flow through a moving, turning component under
/// the two prescribed modes.
#[test]
fn u1_uniform_flow_through_a_moving_rotating_component() {
    let mut c = Checks::new();
    for (name, mode) in [("G", G), ("P", P)] {
        let (eu, ep, cont, hole, q, changes, unconverged) = u_run(mode);
        c.at_most(&format!("U1 {name} velocity"), eu, 1e-10);
        c.at_most(&format!("U1 {name} pressure"), ep, 1e-10);
        c.at_most(&format!("C1 U {name} continuity"), cont, 1e-9);
        c.at_most(&format!("C2 U {name} flux through Gamma_B"), hole, 1e-9);
        c.at_most(&format!("C2 U {name} outflow"), q, 1e-10);
        c.at_least(&format!("U4 (set-up) {name} kind changes"), changes as f64, 20.0);
        c.holds(
            &format!("C3 U {name}: no unconverged linear solve"),
            unconverged == 0,
            &format!("{unconverged} unconverged"),
        );
    }
    c.finish();
}

// ---- UA: a uniform flow with a constant acceleration -----------------------------------------------------------------------

const UA_ACC: Vec3 = [2.0, 1.0, 0.0];

fn ua_u(t: f64) -> Vec3 {
    [1.0 + 2.0 * t, 0.5 + t, 0.0]
}

/// UA of ADR 0347 (amendment 1: the pressure gradient `-a . n` on the background sides) under `mode`, with `offset`
/// added to the starting pressure of the calculated component cells: the largest relative velocity error over the
/// steps, the largest pressure error over the steps and over the steps after the first, the largest continuity.
fn ua_run(mode: FringeFlux, offset: f64) -> (f64, f64, f64, f64) {
    let case = re(1);
    let ov = case.overset(DonorInterpolation::Linear);
    let pose = case.pose();
    let geom = geometry_at(&ov, &pose);
    let conn = ov.connectivity(&geom.mesh, &pose).unwrap();
    let nb = conn.n_background_cells;
    let n = geom.n_cells();
    let x0 = geom.mesh.cell_centres()[0];
    let exact_p = |x: Vec3| -vec3::dot(UA_ACC, vec3::sub(x, x0));
    let mut u = VectorField::uniform("U", &geom, ua_u(0.0));
    let mut p = ScalarField::from_fn("p", &geom, exact_p);
    for c in nb..n {
        if conn.kind[c] == CellKind::Calculated {
            p.values[c] += offset;
        }
    }
    for name in SIDES {
        u.set_bc(&geom, name, Box::new(FixedValue::vector(ua_u(0.0)))).unwrap();
    }
    let mut s = OversetSolver::new(
        geom,
        u,
        p,
        settings(NU_TG),
        MassMode::Conservative,
        Some(OversetCoupling { overset: ov, pose }),
    )
    .unwrap();
    if mode != DEFAULT {
        s.set_fringe_flux(mode);
    }
    // dp/dn = -a . n on the four background sides
    let n_int = s.geom.n_internal;
    let mut dpdn = vec![f64::NAN; s.geom.mesh.n_boundary_faces()];
    for name in SIDES {
        for f in s.geom.mesh.patch(name).unwrap().faces() {
            dpdn[f - n_int] = -vec3::dot(UA_ACC, s.geom.normal(f));
        }
    }
    let dt = 0.0025;
    let (mut eu, mut ep, mut ep_later, mut cont) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for k in 1..=40 {
        let t = k as f64 * dt;
        let g = s.geom.clone();
        for name in SIDES {
            s.u.set_bc(&g, name, Box::new(FixedValue::vector(ua_u(t)))).unwrap();
        }
        let mut step = s.prepare(dt, pose, &[]).unwrap();
        step.wall_dpdn = dpdn.clone();
        let info = s.advance(dt, step).unwrap();
        cont = cont.max(info.continuity);
        let centres = s.geom.mesh.cell_centres();
        let mut e = 0.0f64;
        for c in 0..n {
            if s.conn.kind[c] != CellKind::Hole {
                eu = eu.max(vec3::norm(vec3::sub(s.u.values[c], ua_u(t))) / vec3::norm(ua_u(t)));
                e = e.max((s.p.values[c] - exact_p(centres[c])).abs());
            }
        }
        ep = ep.max(e);
        if k > 1 {
            ep_later = ep_later.max(e);
        }
    }
    (eu, ep, ep_later, cont)
}

/// ADR 0347 UA: the uniformly accelerating flow is reproduced by the default coupling and by both prescribed modes.
#[test]
fn ua_uniform_flow_with_a_constant_acceleration() {
    let mut c = Checks::new();
    for (name, mode) in [("default", DEFAULT), ("P", P), ("G", G)] {
        let (eu, ep, _, cont) = ua_run(mode, 0.0);
        c.at_most(&format!("UA {name} velocity"), eu, 1e-10);
        c.at_most(&format!("UA {name} pressure"), ep, 1e-9);
        c.at_most(&format!("C1 UA {name} continuity"), cont, 1e-9);
    }
    c.finish();
}

/// ADR 0347 UA2: under G a wrong level of the starting pressure of the component does not reach the velocity, and the
/// level is set by the first step.
#[test]
fn ua2_pressure_level_of_the_component() {
    let mut c = Checks::new();
    let (eu, ep, ep_later, cont) = ua_run(G, 0.7);
    println!("UA2: largest pressure error including the first step {ep:.4e}");
    c.at_most("UA2 velocity", eu, 1e-10);
    c.at_most("UA2 pressure after the first step", ep_later, 1e-9);
    c.at_most("C1 UA2 continuity", cont, 1e-9);
    c.finish();
}

// ---- the fast covering test at level 1 -------------------------------------------------------------------------------------

/// ADR 0347 at level 1 of RE under G (the fast test that covers the code of the heavy records): C1, C2, C3, QS, S1, S2;
/// `a_c` and `R_b` are printed (A1 and A2 are asserted in the record of levels 1 to 3).
#[test]
fn c_re_level_1_under_g() {
    let mut c = Checks::new();
    let pr = pair("G RE level 1", &re(1), G, &mut c);
    s1_s2("RE level 1", &pr, &mut c);
    c.finish();
}

// ---- A1, A2, S1, S2, S3 on RE ------------------------------------------------------------------------------------------------

/// ADR 0347 A1, A2, S1, S2, S3 on RE at levels 1 to 3 under G (and the base runs of the default coupling for S3).
#[test]
#[ignore = "slow (RE at levels 1 to 3 under G, base and Qb, and the default base runs, ADR 0347, about half an hour): run once"]
fn a_re_levels_1_to_3() {
    let mut c = Checks::new();
    let mut r = Vec::new();
    for l in 1..=3usize {
        let tag = format!("G RE level {l}");
        let pr = pair(&tag, &re(l), G, &mut c);
        a1(&format!("RE level {l}"), &pr, &mut c);
        s1_s2(&format!("RE level {l}"), &pr, &mut c);
        let def = run(&re(l), false, DEFAULT, &TIMES);
        let (eu, ep) = exact_errors(&def, TIMES.len() - 1);
        println!(
            "default RE level {l} base at t = 0.5 ({:.1} s): rms velocity error {eu:.6e}, rms pressure error {ep:.6e}",
            def.secs
        );
        c.at_most(
            &format!("S3 RE level {l}: rms velocity error, G over the default coupling"),
            pr.err / eu,
            1.5,
        );
        r.push(pr.r[4]);
    }
    c.at_least(
        "A2 order of R_b at t = 0.5 on RE, levels 1 to 2",
        order(r[0], r[1], 2.0),
        1.7,
    );
    c.at_least(
        "A2 order of R_b at t = 0.5 on RE, levels 2 to 3",
        order(r[1], r[2], 1.5),
        1.7,
    );
    c.finish();
}

// ---- A3 on RL ----------------------------------------------------------------------------------------------------------------

/// ADR 0347 A3 on RL at levels 1 to 3 under G: the order of `R_b` between levels 1 and 3 (the order between levels 2
/// and 4 is formed from the printed numbers of this test and of the next).
#[test]
#[ignore = "slow (RL at levels 1 to 3 under G, base and Qb, ADR 0347, about twenty minutes): run once"]
fn a_rl_levels_1_to_3() {
    let mut c = Checks::new();
    let r: Vec<f64> = (1..=3usize)
        .map(|l| pair(&format!("G RL level {l}"), &rl(l), G, &mut c).r[4])
        .collect();
    c.at_least(
        "A3 order of R_b at t = 0.5 on RL, levels 1 to 3",
        order(r[0], r[2], 3.0),
        1.7,
    );
    c.finish();
}

/// ADR 0347 A3 on RL at level 4 under G.
#[test]
#[ignore = "slow (RL at level 4 under G, base and Qb, ADR 0347, about forty minutes): run once"]
fn a_rl_level_4() {
    let mut c = Checks::new();
    pair("G RL level 4", &rl(4), G, &mut c);
    c.finish();
}

// ---- NP: the variant P -------------------------------------------------------------------------------------------------------

/// NP of ADR 0347 on one level of RE: the base and the Qb run under P; prints whether P is admissible and, if a run
/// ends early, the step and the growth per step of the largest velocity error over the last ten steps.
fn np(level: usize) -> bool {
    let case = re(level);
    let mut admissible = true;
    for qb in [false, true] {
        let r = run(&case, qb, P, &TIMES);
        let name = if qb { "Qb" } else { "base" };
        let h = &r.err_hist;
        let growth = if h.len() > 10 {
            (h[h.len() - 1] / h[h.len() - 11]).powf(0.1)
        } else {
            f64::NAN
        };
        match &r.failed {
            Some((k, why)) => {
                admissible = false;
                println!(
                    "NP RE level {level} {name}: NOT ADMISSIBLE, the run ends at step {k} of {} ({why}); growth of the largest velocity error per step over the last ten steps {growth:.4}; continuity {:.3e}; {:.1} s",
                    case.steps, r.cont, r.secs
                );
            }
            None => {
                let ok = r.cont <= 1e-9;
                admissible &= ok;
                println!(
                    "NP RE level {level} {name}: the run reaches t = 0.5; continuity {:.3e} ({}); largest velocity error at the end {:.4e}; {:.1} s",
                    r.cont,
                    if ok { "C1 met" } else { "C1 violated: NOT ADMISSIBLE" },
                    h[h.len() - 1],
                    r.secs
                );
            }
        }
    }
    if admissible {
        let mut c = Checks::new();
        let pr = pair(&format!("P RE level {level}"), &case, P, &mut c);
        let mut judged = Checks::new();
        a1(&format!("P RE level {level}"), &pr, &mut judged);
        s1_s2(&format!("P RE level {level}"), &pr, &mut judged);
        println!(
            "NP RE level {level}: P runs through; R_b(0.5) {:.6e}; missed limits of A1, S1, S2: {:?}; of C1 to C3, QS: {:?}",
            pr.r[4], judged.0, c.0
        );
    }
    admissible
}

/// ADR 0347 NP at level 1 (a classification, printed; the record of the outcome is in the addendum).
#[test]
fn np_variant_p_re_level_1() {
    let admissible = np(1);
    println!("NP RE level 1: P admissible: {admissible}");
}

/// ADR 0347 NP at level 2.
#[test]
#[ignore = "slow (RE at level 2 under P, ADR 0347 NP, up to ten minutes): run once"]
fn np_variant_p_re_level_2() {
    let admissible = np(2);
    println!("NP RE level 2: P admissible: {admissible}");
}

// ---- X: the other opt-ins together with G ------------------------------------------------------------------------------------

/// 40 steps of RE at level 1 under G after `alter`: continuity, unconverged solves, root mean square velocity error.
fn x_run(alter: impl Fn(&mut OversetSolver)) -> (f64, usize, f64) {
    let case = re(1);
    let mut s = tg_solver(&case, G);
    alter(&mut s);
    let dt = 0.0025;
    let (mut cont, mut unconverged) = (0.0f64, 0);
    for k in 1..=40 {
        let g = s.geom.clone();
        tg_conditions(&g, &mut s.u, k as f64 * dt);
        let info = s.step(dt, case.pose(), &[]).unwrap();
        cont = cont.max(info.continuity);
        unconverged += info.unconverged;
    }
    (cont, unconverged, velocity_error(&s, 40.0 * dt).1)
}

/// ADR 0347 X (smoke): G together with `rhie_chow_step_consistent`, with BDF2 and with two outer iterations.
#[test]
fn x_g_with_the_other_opt_ins() {
    let mut c = Checks::new();
    let (cont, unconverged, alone) = x_run(|_| {});
    println!("X G alone: continuity {cont:.3e}, {unconverged} unconverged, rms velocity error {alone:.6e}");
    let variants: [(&str, Box<dyn Fn(&mut OversetSolver)>); 3] = [
        (
            "rhie_chow_step_consistent",
            Box::new(|s| s.settings.rhie_chow_step_consistent = true),
        ),
        ("BDF2", Box::new(|s| s.set_time_scheme(TimeScheme::Bdf2).unwrap())),
        (
            "two outer iterations",
            Box::new(|s| s.set_outer_iterations(2, 1.0, 1.0).unwrap()),
        ),
    ];
    for (name, alter) in variants {
        let (cont, unconverged, err) = x_run(alter);
        c.at_most(&format!("X G with {name}: continuity"), cont, 1e-9);
        c.holds(
            &format!("X G with {name}: no unconverged linear solve"),
            unconverged == 0,
            &format!("{unconverged} unconverged"),
        );
        c.at_most(
            &format!("X G with {name}: rms velocity error over that of G alone"),
            err / alone,
            2.0,
        );
    }
    c.finish();
}
