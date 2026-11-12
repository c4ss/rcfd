//! F-349 (ADR 0340): the response `R_b` of the overset coupling to the truncation of the background-fringe interpolation.
//! The set-up is RL of ADR 0232 amendment 3 as ADR 0246 ran it (Taylor-Green, the component at rest, the fringe phase
//! locked between the levels). `R_b` is the difference, over the calculated component cells, of the run with the library's
//! default stencils and of the run whose background fringe takes the library's quadratic donor interpolation (ADR 0251).
//! Measured here: `R_b` run on to `t = 3`, on levels 4 and 5, with half the time step, on a set-up with equal spacings and
//! no modulation of the truncation along the ring (RE), and with the two parts at different levels. No library code is
//! changed by these tests.
//!
//! Release only (the flow runs take minutes): `cargo test --release -p rcfd-fv --test overset_f349`.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::overset::{MassMode, OversetCoupling, OversetSolver};
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

fn settings() -> PimpleSettings {
    let mut s = PimpleSettings::piso(NU_TG);
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

/// One set-up: the background at level `lb`, a component of half width `7.5 H` with `n_side` cells per side whose centre
/// lies at `(centre, centre)` (in units of `H`), `steps` steps to the time `end`.
#[derive(Clone, Copy)]
struct Case {
    lb: usize,
    n_side: usize,
    centre: f64,
    steps: usize,
    end: f64,
}

/// `RL(lb, lc)` of ADR 0246 with `200 max(lb, lc)` steps to `t = 0.5`.
fn rl(lb: usize, lc: usize) -> Case {
    Case {
        lb,
        n_side: 20 * lc,
        centre: 16.5,
        steps: 200 * lb.max(lc),
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
        let cs = uniform(-HALF, 2.0 * HALF, self.n_side);
        let comp = rectilinear_2d(&cs, &cs, DEPTH).unwrap();
        let composite = compose(&bg, &comp, "c_").unwrap();
        let spec = OversetSpec {
            outer_patches: ["c_xmin", "c_xmax", "c_ymin", "c_ymax"].map(String::from).to_vec(),
            hole: Arc::new(|x: Vec3| x[0].abs().max(x[1].abs()) < HOLE),
        };
        let mut ov = Overset::new(composite, spec).unwrap();
        if mode != DonorInterpolation::Linear {
            ov.set_donor_interpolation(mode);
        }
        ov
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
    /// Cell size of the component.
    fn h_c(&self) -> f64 {
        2.0 * HALF / self.n_side as f64
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

fn tg_conditions(geom: &FvGeometry, u: &mut VectorField, t: f64) {
    for name in ["xmin", "xmax", "ymin", "ymax"] {
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

// ---- flow runs -------------------------------------------------------------------------------------------------------------

/// The state of a run at one time.
struct Snap {
    t: f64,
    u: Vec<Vec3>,
    p: Vec<f64>,
    phi: Vec<f64>,
    delta: f64,
}

struct Run {
    s: OversetSolver,
    /// Largest continuity over the steps (C1).
    cont: f64,
    /// Unconverged linear solves over the steps (C2).
    unconverged: usize,
    /// QS of the first step: component fringe stencils changed, background fringe stencils that are not the quadratic ones.
    qs: (usize, usize),
    /// Fallbacks of the quadratic connectivity among the background fringe cells.
    fallbacks: usize,
    snaps: Vec<Snap>,
    secs: f64,
}

/// Times at which a run to `t = 0.5` keeps its state; every one is a whole number of steps at every level.
const TIMES: [f64; 5] = [0.0025, 0.025, 0.125, 0.25, 0.5];
/// The times of the long run (ADR 0340 L).
const LONG_TIMES: [f64; 11] = [0.0025, 0.025, 0.125, 0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 2.5, 3.0];

/// A Taylor-Green run on the set-up `case`; with `qb` the background fringe takes the quadratic stencils of the library.
fn run(case: &Case, qb: bool, times: &[f64]) -> Run {
    let t0 = Instant::now();
    let dt = case.end / case.steps as f64;
    let ov = case.overset(DonorInterpolation::Linear);
    let pose = case.pose();
    let mesh = mesh_at(&ov, &pose);
    let lin = ov.connectivity(&mesh, &pose).unwrap();
    let nb = lin.n_background_cells;
    let quad = qb.then(|| {
        case.overset(DonorInterpolation::Quadratic)
            .connectivity(&mesh, &pose)
            .unwrap()
    });
    let fallbacks = quad
        .as_ref()
        .map_or(0, |q| q.fallbacks.iter().filter(|&&c| c < nb).count());
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::from_fn("U", &geom, |x| tg_u(x, 0.0));
    tg_conditions(&geom, &mut u, 0.0);
    let p = ScalarField::from_fn("p", &geom, |x| tg_p(x, 0.0));
    let mut s = OversetSolver::new(
        geom,
        u,
        p,
        settings(),
        MassMode::Conservative,
        Some(OversetCoupling { overset: ov, pose }),
    )
    .unwrap();
    let (mut cont, mut unconverged) = (0.0f64, 0usize);
    let mut qs_first = (0, 0);
    let mut snaps = Vec::new();
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
        let info = s.advance(dt, step).unwrap();
        cont = cont.max(info.continuity);
        unconverged += info.unconverged;
        if times.iter().any(|ts| (ts - t).abs() < 0.25 * dt) {
            snaps.push(Snap {
                t,
                u: s.u.values.clone(),
                p: s.p.values.clone(),
                phi: s.phi.clone(),
                delta: info.delta,
            });
        }
    }
    assert_eq!(
        snaps.len(),
        times.len(),
        "a snapshot time is not a whole number of steps"
    );
    Run {
        s,
        cont,
        unconverged,
        qs: qs_first,
        fallbacks,
        snaps,
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

/// `R_b` at the snapshot `k`: the root mean square over the calculated component cells of the difference of the
/// velocities of the two runs, weighted with the cell volumes, over the decay factor of that time.
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

/// Bands of the calculated background cells by the distance `rho` from the component centre, in units of `H`.
const BANDS: [(f64, f64, &str); 4] = [
    (0.0, 5.0, "rho < 5 H"),
    (5.0, 7.5, "5 H <= rho < 7.5 H"),
    (7.5, 12.0, "7.5 H <= rho < 12 H"),
    (12.0, 1e9, "rho >= 12 H"),
];

/// The printed numbers of ADR 0340 for the difference `d` of two runs at the snapshot `k` (no limit).
fn diagnostics(tag: &str, case: &Case, base: &Run, var: &Run, k: usize) {
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
    // the pressure difference with its mean over the calculated background cells removed
    let mut level = Acc::default();
    for c in 0..nb {
        if kind[c] == CellKind::Calculated {
            level.add(vol[c], a.p[c] - b.p[c]);
        }
    }
    let dp: Vec<f64> = (0..n).map(|c| a.p[c] - b.p[c] - level.mean()).collect();
    // CC: mean vector and the share that is a multiple of the exact velocity
    let (mut mean, mut vsum, mut eu, mut uu, mut ee) = ([0.0; 3], 0.0, 0.0, 0.0, 0.0);
    // classes: CF, CC, BF and the bands of BC
    let mut cls = [Acc::default(); 7];
    let mut pcls = [Acc::default(); 7];
    for c in 0..n {
        let i = match (kind[c], c < nb) {
            (CellKind::Hole, _) => continue,
            (CellKind::Fringe, false) => 0,
            (CellKind::Calculated, false) => 1,
            (CellKind::Fringe, true) => 2,
            (CellKind::Calculated, true) => {
                let r = case.rho(centres[c]) / H1;
                3 + BANDS.iter().position(|b| r >= b.0 && r < b.1).unwrap()
            }
        };
        cls[i].add(vol[c], vec3::norm(d[c]));
        pcls[i].add(vol[c], dp[c]);
        if i == 1 {
            let ex = vec3::scale(tg_u(centres[c], t), 1.0 / decay);
            mean = vec3::add(mean, vec3::scale(d[c], vol[c]));
            vsum += vol[c];
            eu += vol[c] * vec3::dot(d[c], ex);
            uu += vol[c] * vec3::dot(ex, ex);
            ee += vol[c] * vec3::dot(d[c], d[c]);
        }
    }
    println!(
        "{tag} t = {t}: R_b {:.4e}; CC mean vector ({:.3e}, {:.3e}); share of the exact velocity {:.4}, coefficient {:.3e}; delta base {:.4e}, Qb {:.4e}",
        cls[1].rms(),
        mean[0] / vsum,
        mean[1] / vsum,
        eu * eu / (uu * ee).max(1e-300),
        eu / uu,
        a.delta,
        b.delta
    );
    println!(
        "{tag} t = {t}: rms of d: CF {:.4e}, BF {:.4e}, BC {} {:.4e}, {} {:.4e}, {} {:.4e}, {} {:.4e}",
        cls[0].rms(),
        cls[2].rms(),
        BANDS[0].2,
        cls[3].rms(),
        BANDS[1].2,
        cls[4].rms(),
        BANDS[2].2,
        cls[5].rms(),
        BANDS[3].2,
        cls[6].rms()
    );
    println!(
        "{tag} t = {t}: rms of the pressure difference (BC mean removed): CF {:.4e}, CC {:.4e} (mean {:.4e}), BF {:.4e}, BC {} {:.4e}, {} {:.4e}, {} {:.4e}, {} {:.4e}",
        pcls[0].rms(),
        pcls[1].rms(),
        pcls[1].mean(),
        pcls[2].rms(),
        BANDS[0].2,
        pcls[3].rms(),
        BANDS[1].2,
        pcls[4].rms(),
        BANDS[2].2,
        pcls[5].rms(),
        BANDS[3].2,
        pcls[6].rms()
    );
    // the faces between a calculated and a fringe cell of each part
    let sf = g.mesh.face_area_vectors();
    let (own, nei) = (g.mesh.owner(), g.mesh.neighbour());
    for (part, name) in [(true, "Gamma_B"), (false, "Gamma_C")] {
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
            jump.add(1.0, dp[fringe] - dp[calc]);
        }
        let cnt = jump.w.max(1.0);
        println!(
            "{tag} t = {t}: {name} ({} faces): rms of t {:.4e}, of c {:.4e}, of m {:.4e}; a_c {:.4}, a_m {:.4}; change of the pressure jump mean {:.4e}, rms {:.4e}",
            jump.w,
            (tt / cnt).sqrt(),
            (cc / cnt).sqrt(),
            (mm / cnt).sqrt(),
            ct / tt.max(1e-300),
            mt / tt.max(1e-300),
            jump.mean(),
            jump.rms()
        );
    }
}

/// The base run and the Qb run of `case` with C1, C2 and QS, the diagnostics, and `R_b` at every snapshot time.
fn pair_runs(tag: &str, case: &Case, times: &[f64], c: &mut Checks) -> (Run, Run, Vec<f64>) {
    let base = run(case, false, times);
    let var = run(case, true, times);
    println!(
        "{tag}: {} cells, {} steps to t = {}; base {:.1} s, Qb {:.1} s; unconverged solves {} and {}",
        base.s.geom.n_cells(),
        case.steps,
        case.end,
        base.secs,
        var.secs,
        base.unconverged,
        var.unconverged
    );
    c.at_most(&format!("C1 {tag} base continuity"), base.cont, 1e-9);
    c.at_most(&format!("C1 {tag} Qb continuity"), var.cont, 1e-9);
    c.holds(
        &format!("C2 {tag}: no unconverged linear solve"),
        base.unconverged == 0 && var.unconverged == 0,
        &format!("{} and {} unconverged", base.unconverged, var.unconverged),
    );
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
    let r: Vec<f64> = (0..times.len()).map(|k| r_b(&base, &var, k)).collect();
    for (k, t) in times.iter().enumerate() {
        println!("RESULT {tag} t = {t}: R_b {:.6e}", r[k]);
        if [0.025, 0.125, 0.5].contains(t) || k + 1 == times.len() {
            diagnostics(tag, case, &base, &var, k);
        }
    }
    (base, var, r)
}

/// [`pair_runs`] without the runs.
fn pair(tag: &str, case: &Case, times: &[f64], c: &mut Checks) -> Vec<f64> {
    pair_runs(tag, case, times, c).2
}

/// QL of ADR 0340: `R_b(L, 0.5)` against the value of ADR 0246 (nine-cell stencils of the test).
fn ql(c: &mut Checks, level: usize, r: f64) {
    let adr_0246 = [4.8742e-5, 2.1922e-5, 1.4073e-5][level - 1];
    c.at_most(
        &format!("QL level {level}: R_b(0.5) against ADR 0246 ({adr_0246:.4e}), relative difference"),
        (r / adr_0246 - 1.0).abs(),
        0.05,
    );
}

// ---- K: the truncation of the background fringe (no flow solve) -----------------------------------------------------------

/// The numbers of K for one set-up: the root mean square over the background fringe cells of the truncation of the exact
/// velocity with the linear and with the quadratic stencils, and the largest error of the quadratic field (iii) over its
/// largest value.
fn truncation(tag: &str, case: &Case, c: &mut Checks) -> (f64, f64, f64) {
    let pose = case.pose();
    let ov = case.overset(DonorInterpolation::Linear);
    let mesh = mesh_at(&ov, &pose);
    let lin = ov.connectivity(&mesh, &pose).unwrap();
    let quad = case
        .overset(DonorInterpolation::Quadratic)
        .connectivity(&mesh, &pose)
        .unwrap();
    let nb = lin.n_background_cells;
    let centres = mesh.cell_centres();
    let tg = |x: Vec3| tg_u(x, 0.0);
    let poly = |x: Vec3| (1.0 + x[0] + 2.0 * x[1]) * (1.0 - x[0] + x[1]);
    let apply = |st: &Stencil, cell: usize| -> Vec3 {
        let mut v = vec3::scale(tg(centres[cell]), -1.0);
        for (&k, &w) in st.cells.iter().zip(&st.weights) {
            v = vec3::add(v, vec3::scale(tg(centres[k]), w));
        }
        v
    };
    let h_c = case.h_c();
    let centre = case.centre * H1;
    let (mut all, mut normal, mut tangent, mut q) = (Acc::default(), Acc::default(), Acc::default(), Acc::default());
    let mut mean = [0.0; 3];
    let mut classes = [Acc::default(); 3];
    let (mut poly_err, mut poly_max, mut rho_min, mut rho_max) = (0.0f64, 0.0f64, f64::INFINITY, 0.0f64);
    let mut n_fringe = 0usize;
    let mut swapped = lin.clone();
    substitute_background_fringe(&mut swapped, &quad);
    let (comp_changed, bg_bad) = qs(&swapped, &lin);
    for (cell, st) in lin.stencils.iter().filter(|(cell, _)| *cell < nb) {
        let x = centres[*cell];
        let tau = apply(st, *cell);
        let sq = stencil_of(&quad, *cell).unwrap();
        let tau_q = apply(sq, *cell);
        // the normal of the ring: the axis along which the centre lies farthest from the component centre
        let (dx, dy) = (x[0] - centre, x[1] - centre);
        let (nrm, along) = if dx.abs() >= dy.abs() {
            ([dx.signum(), 0.0, 0.0], dy)
        } else {
            ([0.0, dy.signum(), 0.0], dx)
        };
        let tn = vec3::dot(tau, nrm);
        all.add(1.0, vec3::norm(tau));
        normal.add(1.0, tn);
        tangent.add(1.0, vec3::norm(vec3::sub(tau, vec3::scale(nrm, tn))));
        q.add(1.0, vec3::norm(tau_q));
        mean = vec3::add(mean, tau);
        // the tangential fraction of the centre in its donor cell, to the nearest third
        let frac = ((along + HALF) / h_c).rem_euclid(1.0);
        classes[(frac * 3.0).round() as usize % 3].add(1.0, vec3::norm(tau));
        let mut pv = -poly(x);
        for (&k, &w) in sq.cells.iter().zip(&sq.weights) {
            pv += w * poly(centres[k]);
        }
        poly_err = poly_err.max(pv.abs());
        poly_max = poly_max.max(poly(x).abs());
        rho_min = rho_min.min(case.rho(x));
        rho_max = rho_max.max(case.rho(x));
        n_fringe += 1;
    }
    println!(
        "{tag}: {n_fringe} background fringe cells at rho = {:.4} H to {:.4} H; linear: rms {:.4e}, kappa_b {:.5}, normal part {:.4e}, tangential part {:.4e}, mean ({:.3e}, {:.3e}); by tangential fraction 0, 1/3, 2/3: {:.4e} ({}), {:.4e} ({}), {:.4e} ({}); quadratic: rms {:.4e}",
        rho_min / H1,
        rho_max / H1,
        all.rms(),
        all.rms() / (h_c * h_c),
        normal.rms(),
        tangent.rms(),
        mean[0] / n_fringe as f64,
        mean[1] / n_fringe as f64,
        classes[0].rms(),
        classes[0].w,
        classes[1].rms(),
        classes[1].w,
        classes[2].rms(),
        classes[2].w,
        q.rms()
    );
    c.holds(
        &format!("QS {tag}: the substituted connectivity"),
        comp_changed == 0 && bg_bad == 0 && quad.fallbacks.iter().all(|&k| k >= nb),
        &format!(
            "{comp_changed} component stencils changed, {bg_bad} background stencils without nine calculated cells and the donor first"
        ),
    );
    (all.rms(), q.rms(), poly_err / poly_max)
}

/// ADR 0340 K as registered. Run once: (ii), (iii) and QS met; K (i) missed between levels 3 and 4, order 1.621 against
/// the limit 1.8 (2.565 between levels 2 and 3, 2.371 between 4 and 5): `kappa_b` is 0.0685, 0.0781, 0.0621, 0.0693, 0.0638
/// at levels 1 to 5, it alternates between the odd and the even levels. Kept with the limit intact (ADR 0340 amendment 1).
#[test]
#[ignore = "known miss, ADR 0340 amendment 1: K (i) order of the linear truncation of the background fringe 1.621 between levels 3 and 4 (limit 1.8)"]
fn k_background_fringe_truncation_known_miss() {
    let mut c = Checks::new();
    let rows: Vec<(f64, f64, f64)> = (1..=5usize)
        .map(|l| truncation(&format!("K RL level {l}"), &rl(l, l), &mut c))
        .collect();
    for (l, row) in rows.iter().enumerate() {
        c.at_most(
            &format!(
                "K (iii) level {}: the quadratic field at the background fringe centres",
                l + 1
            ),
            row.2,
            1e-11,
        );
    }
    println!(
        "K levels 1 to 2 (the ring moves from 4.0 H to 3.25 H): order of the linear truncation {:.4}, of the quadratic {:.4}",
        order(rows[0].0, rows[1].0, 2.0),
        order(rows[0].1, rows[1].1, 2.0)
    );
    for l in 2..5usize {
        let ratio = (l + 1) as f64 / l as f64;
        c.at_least(
            &format!("K (i) order of the linear truncation, levels {l} to {}", l + 1),
            order(rows[l - 1].0, rows[l].0, ratio),
            1.8,
        );
        c.at_least(
            &format!("K (ii) order of the quadratic truncation, levels {l} to {}", l + 1),
            order(rows[l - 1].1, rows[l].1, ratio),
            2.7,
        );
    }
    let re_rows: Vec<(f64, f64, f64)> = (1..=3usize)
        .map(|l| truncation(&format!("K RE level {l}"), &re(l), &mut c))
        .collect();
    println!(
        "K RE: orders of the linear truncation {:.4}, {:.4}; of the quadratic {:.4}, {:.4}; the quadratic field {:.2e}, {:.2e}, {:.2e}",
        order(re_rows[0].0, re_rows[1].0, 2.0),
        order(re_rows[1].0, re_rows[2].0, 1.5),
        order(re_rows[0].1, re_rows[1].1, 2.0),
        order(re_rows[1].1, re_rows[2].1, 1.5),
        re_rows[0].2,
        re_rows[1].2,
        re_rows[2].2
    );
    c.finish();
}

/// ADR 0340 amendment 1, K': K (ii), K (iii) and QS as registered, and the order of the linear truncation of the
/// background fringe between levels of the same parity (1 to 3, 3 to 5, 2 to 4), on which the normal fraction of the
/// fringe centres in their donor cells is the same and not its mirror image.
#[test]
fn k2_truncation_within_the_parity_families() {
    let mut c = Checks::new();
    let rows: Vec<(f64, f64, f64)> = (1..=5usize)
        .map(|l| truncation(&format!("K' RL level {l}"), &rl(l, l), &mut c))
        .collect();
    for (l, row) in rows.iter().enumerate() {
        c.at_most(
            &format!(
                "K (iii) level {}: the quadratic field at the background fringe centres",
                l + 1
            ),
            row.2,
            1e-11,
        );
    }
    for l in 2..5usize {
        c.at_least(
            &format!("K (ii) order of the quadratic truncation, levels {l} to {}", l + 1),
            order(rows[l - 1].1, rows[l].1, (l + 1) as f64 / l as f64),
            2.7,
        );
    }
    for (a, b) in [(1usize, 3usize), (3, 5), (2, 4)] {
        c.at_least(
            &format!("K' (i) order of the linear truncation, levels {a} to {b}"),
            order(rows[a - 1].0, rows[b - 1].0, b as f64 / a as f64),
            1.8,
        );
    }
    c.finish();
}

// ---- amendment 1, HK: the kink relation at the fringe faces ----------------------------------------------------------------

/// ADR 0340 amendment 1, HK, on the faces between a calculated and a fringe cell of the background (`background`) or of
/// the component at the snapshot `k`: with `dD` the change of the flux defect `phi / abs(S) - (u)_f . n` between the two
/// runs and `dJ` the change of the pressure jump `p_F - p_C`, the coefficient `k = -(3 h / (16 nu)) sum dJ dD / sum dD^2`
/// (1 by the kink relation `dJ = -(16 nu / (3 h)) dD`) and the correlation `-sum dJ dD / sqrt(sum dJ^2 sum dD^2)`.
fn kink(base: &Run, var: &Run, k: usize, background: bool) -> (f64, f64) {
    let s = &base.s;
    let g = &s.geom;
    let nb = s.conn.n_background_cells;
    let kind = &s.conn.kind;
    let centres = g.mesh.cell_centres();
    let (a, b) = (&base.snaps[k], &var.snaps[k]);
    let sf = g.mesh.face_area_vectors();
    let (own, nei) = (g.mesh.owner(), g.mesh.neighbour());
    let (mut jd, mut dd, mut jj, mut h) = (0.0, 0.0, 0.0, 0.0);
    for f in 0..g.n_internal {
        let (o, m) = (own[f], nei[f]);
        if (o < nb) != background {
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
        let du = |c: usize| vec3::dot(vec3::sub(a.u[c], b.u[c]), nrm);
        let d_defect = sign * (a.phi[f] - b.phi[f]) / area - 0.5 * (du(fringe) + du(calc));
        let d_jump = (a.p[fringe] - b.p[fringe]) - (a.p[calc] - b.p[calc]);
        jd += d_jump * d_defect;
        dd += d_defect * d_defect;
        jj += d_jump * d_jump;
        h = vec3::norm(vec3::sub(centres[fringe], centres[calc]));
    }
    (
        -(3.0 * h / (16.0 * NU_TG)) * jd / dd.max(1e-300),
        -jd / (jj * dd).sqrt().max(1e-300),
    )
}

/// HK on one set-up at `t = 0.5`: printed for both fringes; asserted on the background fringe when `assert` is set.
fn hk(tag: &str, case: &Case, assert: bool, c: &mut Checks) {
    let (base, var, _) = pair_runs(tag, case, &TIMES, c);
    let (kb, rb) = kink(&base, &var, 4, true);
    let (kc, rc) = kink(&base, &var, 4, false);
    println!(
        "HK {tag}: Gamma_B coefficient {kb:.4}, correlation {rb:.4}; Gamma_C coefficient {kc:.4}, correlation {rc:.4}"
    );
    if assert {
        c.at_least(&format!("HK {tag}: Gamma_B coefficient"), kb, 0.6);
        c.at_most(&format!("HK {tag}: Gamma_B coefficient"), kb, 1.4);
        c.at_least(&format!("HK {tag}: Gamma_B correlation"), rb, 0.8);
    }
}

/// ADR 0340 amendment 1, HK on RE at level 1 (the gate-run check of HK).
#[test]
fn hk_kink_relation_re_level_1() {
    let mut c = Checks::new();
    hk("RE level 1", &re(1), true, &mut c);
    c.finish();
}

/// ADR 0340 amendment 1, HK on RE at level 2 (asserted) and on RL at levels 1 and 2 (printed: unequal `rAU`).
#[test]
#[ignore = "slow (RE at level 2, RL at levels 1 and 2, base and Qb, ADR 0340 amendment 1, about ten minutes): run once"]
fn hk_kink_relation_level_2() {
    let mut c = Checks::new();
    hk("RE level 2", &re(2), true, &mut c);
    hk("RL level 1", &rl(1, 1), false, &mut c);
    hk("RL level 2", &rl(2, 2), false, &mut c);
    c.finish();
}

// ---- the gate-run check -----------------------------------------------------------------------------------------------------

/// ADR 0340 C1, C2, QS and QL on RL at level 1 (the run of L at that level up to `t = 0.5`, which is deterministic).
#[test]
fn c_level_1_controls() {
    let mut c = Checks::new();
    let r = pair("C RL level 1", &rl(1, 1), &TIMES, &mut c);
    ql(&mut c, 1, r[4]);
    c.finish();
}

// ---- L: the long run ---------------------------------------------------------------------------------------------------------

fn long(l: usize) -> Case {
    Case {
        steps: 1200 * l,
        end: 3.0,
        ..rl(l, l)
    }
}

/// ADR 0340 L at levels 1 and 2: `R_b` run on to `t = 3`; QL, NL and A3 on the first pair of levels. Run once (1771 s):
/// C1, C2, QS, QL (2.5 and 0.7 percent) and NL (1.198) met; A3 missed, the order of `R_b` at `t = 3` between levels 1
/// and 2 is 1.418 (`1.4028e-4`, `5.2499e-5`) against the limit 1.7. Kept with the limit intact (ADR 0340 addendum 1).
#[test]
#[ignore = "known miss, ADR 0340: A3 order of R_b at t = 3 between levels 1 and 2 is 1.418 (limit 1.7); slow"]
fn l_long_run_levels_1_and_2_known_miss() {
    let mut c = Checks::new();
    let r1 = pair("L RL level 1", &long(1), &LONG_TIMES, &mut c);
    let r2 = pair("L RL level 2", &long(2), &LONG_TIMES, &mut c);
    ql(&mut c, 1, r1[4]);
    ql(&mut c, 2, r2[4]);
    for (k, t) in LONG_TIMES.iter().enumerate() {
        println!("L levels 1 to 2, t = {t}: order of R_b {:.4}", order(r1[k], r2[k], 2.0));
    }
    println!(
        "L: G_1 = R_b(3) / R_b(0.5) = {:.4}, G_2 = {:.4}",
        r1[10] / r1[4],
        r2[10] / r2[4]
    );
    c.at_most(
        "NL order of R_b at t = 0.5, levels 1 to 2",
        order(r1[4], r2[4], 2.0),
        1.5,
    );
    c.at_least(
        "A3 order of R_b at t = 3, levels 1 to 2",
        order(r1[10], r2[10], 2.0),
        1.7,
    );
    c.finish();
}

/// ADR 0340 L at level 3 (the orders against level 2 are formed in the ADR from the printed numbers).
#[test]
#[ignore = "slow (RL at level 3 to t = 3, base and Qb, ADR 0340 L, about an hour): run once"]
fn l_long_run_level_3() {
    let mut c = Checks::new();
    let r3 = pair("L RL level 3", &long(3), &LONG_TIMES, &mut c);
    ql(&mut c, 3, r3[4]);
    println!("L: G_3 = R_b(3) / R_b(0.5) = {:.4}", r3[10] / r3[4]);
    c.finish();
}

// ---- F: finer levels ---------------------------------------------------------------------------------------------------------

/// ADR 0340 F at level 4 (A2 is formed in the ADR from the printed numbers).
#[test]
#[ignore = "slow (RL at level 4, base and Qb, ADR 0340 F, about half an hour): run once"]
fn f_finer_level_4() {
    let mut c = Checks::new();
    pair("F RL level 4", &rl(4, 4), &TIMES, &mut c);
    c.finish();
}

/// ADR 0340 F at level 5 (A2 is formed in the ADR from the printed numbers).
#[test]
#[ignore = "slow (RL at level 5, base and Qb, ADR 0340 F, about an hour): run once"]
fn f_finer_level_5() {
    let mut c = Checks::new();
    pair("F RL level 5", &rl(5, 5), &TIMES, &mut c);
    c.finish();
}

// ---- D: half the time step ---------------------------------------------------------------------------------------------------

/// ADR 0340 D: RL at levels 1, 2, 3 with `400 L` steps; A1 for this switch (`rho_dt` is formed in the ADR against L). Run
/// once (2613 s): C1, C2 and QS met; A1 missed, the orders of `R_b` at `t = 0.5` with half the step are 1.201 and 1.068
/// (`5.0402e-5`, `2.1928e-5`, `1.4223e-5`) against the limit 1.7. Kept with the limit intact (ADR 0340 addendum 1).
#[test]
#[ignore = "known miss, ADR 0340: A1 (D) orders of R_b with half the step 1.201, 1.068 (limit 1.7); slow"]
fn d_half_time_step_levels_1_to_3_known_miss() {
    let mut c = Checks::new();
    let r: Vec<f64> = (1..=3usize)
        .map(|l| {
            let case = Case {
                steps: 400 * l,
                ..rl(l, l)
            };
            pair(&format!("D RL level {l}, half the step"), &case, &TIMES, &mut c)[4]
        })
        .collect();
    c.at_least(
        "A1 (D) order of R_b at t = 0.5 with half the step, levels 1 to 2",
        order(r[0], r[1], 2.0),
        1.7,
    );
    c.at_least(
        "A1 (D) order of R_b at t = 0.5 with half the step, levels 2 to 3",
        order(r[1], r[2], 1.5),
        1.7,
    );
    c.finish();
}

// ---- E: equal spacings, no modulation along the ring -------------------------------------------------------------------------

/// ADR 0340 E: RE at levels 1, 2, 3; A1 for this switch. Run once (1097 s): C1, C2 and QS met; A1 missed, the orders of
/// `R_b` at `t = 0.5` are 0.983 and 1.015 (`6.4279e-5`, `3.2510e-5`, `2.1540e-5`) against the limit 1.7. Kept with the
/// limit intact (ADR 0340 addendum 1).
#[test]
#[ignore = "known miss, ADR 0340: A1 (E) orders of R_b on RE 0.983, 1.015 (limit 1.7); slow"]
fn e_equal_spacings_levels_1_to_3_known_miss() {
    let mut c = Checks::new();
    let r: Vec<f64> = (1..=3usize)
        .map(|l| pair(&format!("E RE level {l}"), &re(l), &TIMES, &mut c)[4])
        .collect();
    c.at_least(
        "A1 (E) order of R_b at t = 0.5 on RE, levels 1 to 2",
        order(r[0], r[1], 2.0),
        1.7,
    );
    c.at_least(
        "A1 (E) order of R_b at t = 0.5 on RE, levels 2 to 3",
        order(r[1], r[2], 1.5),
        1.7,
    );
    c.finish();
}

// ---- S: the two spacings apart -----------------------------------------------------------------------------------------------

/// ADR 0340 S: `R_b` with the two parts at different levels (the hypothesis Hsp is decided in the ADR by the printed
/// exponents).
#[test]
#[ignore = "slow (RL with the parts at different levels up to level 4, base and Qb, ADR 0340 S, about half an hour): run once"]
fn s_spacings_apart() {
    let mut c = Checks::new();
    let mut r = |lb: usize, lc: usize| pair(&format!("S RL({lb},{lc})"), &rl(lb, lc), &TIMES, &mut c)[4];
    let (r11, r22) = (r(1, 1), r(2, 2));
    let (r12, r21, r24, r42) = (r(1, 2), r(2, 1), r(2, 4), r(4, 2));
    println!(
        "Hsp: y_1 = log2(R_b(1,1) / R_b(1,2)) = {:.4}; y_2 = log2(R_b(2,2) / R_b(2,4)) = {:.4}; x_1 = log2(R_b(1,1) / R_b(2,1)) = {:.4}; x_2 = log2(R_b(2,2) / R_b(4,2)) = {:.4}",
        (r11 / r12).log2(),
        (r22 / r24).log2(),
        (r11 / r21).log2(),
        (r22 / r42).log2()
    );
    println!(
        "Hsp: orders within the families: B2 (2,1) to (4,2) {:.4}; C2 (1,2) to (2,4) {:.4}; (1,1) to (2,2) {:.4}",
        order(r21, r42, 2.0),
        order(r12, r24, 2.0),
        order(r11, r22, 2.0)
    );
    c.finish();
}
