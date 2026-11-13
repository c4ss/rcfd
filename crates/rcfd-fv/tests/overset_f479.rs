//! F-479 (ADR 0409): the second-order own-part pressure gradient of the ring cells under the prescribed fringe flux of ADR
//! 0347 (`OversetSolver::set_fringe_flux(PrescribedOwnGradient)` with `set_second_order_ring_gradient(true)`), on the
//! Taylor-Green set-up TG_L of ADR 0219 with the moving, turning component and the quadratic donor interpolation, against
//! the default coupling with the same stencils (the runs of ADR 0376, helpers of `overset_f450.rs` copied), and the checks
//! UA, UA2 and U1 of ADR 0347 (helpers of `overset_f445.rs` copied).
//!
//! Release only (the flow runs take minutes): `cargo test --release -p rcfd-fv --test overset_f479`.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::overset::{FringeFlux, MassMode, OversetCoupling, OversetSolver};
use rcfd_fv::pimple::PimpleSettings;
use rcfd_fv::{FvGeometry, Profile, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::rectilinear_2d;
use rcfd_mesh::motion::{moved_mesh, Pose};
use rcfd_mesh::overset::{compose, CellKind, DonorInterpolation, Overset, OversetSpec};
use rcfd_mesh::Mesh;
use std::f64::consts::PI;
use std::sync::Arc;
use std::time::Instant;

const DEPTH: f64 = 0.1;
const NU_TG: f64 = 0.1;
const T_END: f64 = 0.5;
/// ADR 0219 PS.
const SOLVE: SolveControl = SolveControl {
    tol_abs: 1e-15,
    tol_rel: 1e-12,
    max_iter: 3000,
};
const QD: DonorInterpolation = DonorInterpolation::Quadratic;
const LD: DonorInterpolation = DonorInterpolation::Linear;
const SIDES: [&str; 4] = ["xmin", "xmax", "ymin", "ymax"];

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

// ---- TG_L of ADR 0219 (helpers of overset_f450.rs) -------------------------------------------------------------------------

fn uniform(x0: f64, len: f64, n: usize) -> Vec<f64> {
    (0..=n).map(|i| x0 + len * i as f64 / n as f64).collect()
}

fn tg_overset(level: usize, donors: DonorInterpolation) -> Overset {
    let bs = uniform(0.0, PI, 40 * level);
    let bg: Mesh = rectilinear_2d(&bs, &bs, DEPTH).unwrap();
    let cs = uniform(-0.6, 1.2, 20 * level);
    let comp = rectilinear_2d(&cs, &cs, DEPTH).unwrap();
    let composite = compose(&bg, &comp, "c_").unwrap();
    let spec = OversetSpec {
        outer_patches: ["c_xmin", "c_xmax", "c_ymin", "c_ymax"].map(String::from).to_vec(),
        hole: Arc::new(|x: Vec3| x[0].abs().max(x[1].abs()) < 0.25),
    };
    let mut ov = Overset::new(composite, spec).unwrap();
    if donors != DonorInterpolation::Linear {
        ov.set_donor_interpolation(donors);
    }
    ov
}

fn tg_pose(t: f64) -> Pose {
    Pose {
        origin: [0.0; 3],
        axis: [0.0, 0.0, 1.0],
        angle: t,
        translation: [PI / 2.0 - 0.3 + 0.5 * t, PI / 2.0 - 0.2 + 0.3 * t, 0.0],
    }
}

fn tg_u(x: Vec3, t: f64) -> Vec3 {
    let e = (-2.0 * NU_TG * t).exp();
    [-x[0].cos() * x[1].sin() * e, x[0].sin() * x[1].cos() * e, 0.0]
}

fn tg_p(x: Vec3, t: f64) -> f64 {
    -((2.0 * x[0]).cos() + (2.0 * x[1]).cos()) * (-4.0 * NU_TG * t).exp() / 4.0
}

fn geometry_at(ov: &Overset, pose: &Pose) -> Arc<FvGeometry> {
    let mesh = moved_mesh(&ov.composite.mesh, ov.composite.points_at(pose)).unwrap();
    Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap())
}

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

fn area_max(geom: &FvGeometry) -> f64 {
    geom.mesh
        .face_area_vectors()
        .iter()
        .map(|s| vec3::norm(*s))
        .fold(0.0, f64::max)
}

fn patch_flux(geom: &FvGeometry, phi: &[f64], patch: &str) -> f64 {
    geom.mesh.patch(patch).unwrap().faces().map(|f| phi[f]).sum()
}

/// `state_hash` of `overset_f445.rs`.
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

fn tg_solver(level: usize, mode: Mode, donors: DonorInterpolation) -> OversetSolver {
    let ov = tg_overset(level, donors);
    let pose = tg_pose(0.0);
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

struct Run {
    s: OversetSolver,
    cont: f64,
    hole: f64,
    boundary: f64,
    unconverged: usize,
    fallbacks: usize,
    changes: usize,
    steps: usize,
    failed: Option<(usize, String)>,
    secs: f64,
}

/// TG_L moving with `200 L^2` steps to `t = 0.5` (ADR 0219 amendment 2), the quadratic stencils. A refused step ends the run.
fn tg_run(level: usize, mode: Mode) -> Run {
    let t0 = Instant::now();
    let steps = 200 * level * level;
    let dt = T_END / steps as f64;
    let mut s = tg_solver(level, mode, QD);
    let amax = area_max(&s.geom);
    let (mut cont, mut hole, mut boundary) = (0.0f64, 0.0f64, 0.0f64);
    let (mut unconverged, mut fallbacks, mut changes) = (0usize, s.conn.n_fallback(), 0usize);
    let (mut taken, mut failed) = (0usize, None);
    for k in 1..=steps {
        let t = k as f64 * dt;
        let g = s.geom.clone();
        tg_conditions(&g, &mut s.u, t);
        let info = match s.step(dt, tg_pose(t), &[]) {
            Ok(i) => i,
            Err(e) => {
                failed = Some((k, e));
                break;
            }
        };
        taken = k;
        cont = cont.max(info.continuity);
        hole = hole.max(info.hole_flux.abs() / amax);
        let b: f64 = SIDES.iter().map(|n| patch_flux(&s.geom, &s.phi, n)).sum();
        boundary = boundary.max(b.abs() / amax);
        unconverged += info.unconverged;
        fallbacks = fallbacks.max(info.n_fallback);
        changes += info.n_changed;
    }
    Run {
        s,
        cont,
        hole,
        boundary,
        unconverged,
        fallbacks,
        changes,
        steps: taken,
        failed,
        secs: t0.elapsed().as_secs_f64(),
    }
}

const CLASSES: [&str; 4] = ["CF", "CC", "BF", "BC"];

fn class_of(kind: CellKind, background: bool) -> Option<usize> {
    match (kind, background) {
        (CellKind::Hole, _) => None,
        (CellKind::Fringe, false) => Some(0),
        (CellKind::Calculated, false) => Some(1),
        (CellKind::Fringe, true) => Some(2),
        (CellKind::Calculated, true) => Some(3),
    }
}

struct Errors {
    class_max: [f64; 4],
    class_rms: [f64; 4],
    max: f64,
    max_at: (usize, usize, Vec3),
}

/// The errors at `t = 0.5`, `abs(u_c - u_ex) / e^{-2 nu t}`, over the calculated and fringe cells.
fn errors(s: &OversetSolver) -> Errors {
    let nb = s.conn.n_background_cells;
    let vol = s.geom.mesh.cell_volumes();
    let centres = s.geom.mesh.cell_centres();
    let scale = (-2.0 * NU_TG * T_END).exp();
    let (mut sum, mut vsum) = ([0.0; 4], [0.0; 4]);
    let mut e = Errors {
        class_max: [0.0; 4],
        class_rms: [0.0; 4],
        max: 0.0,
        max_at: (0, 0, [0.0; 3]),
    };
    for c in 0..s.geom.n_cells() {
        let Some(k) = class_of(s.conn.kind[c], c < nb) else {
            continue;
        };
        let eps = vec3::norm(vec3::sub(s.u.values[c], tg_u(centres[c], T_END))) / scale;
        e.class_max[k] = e.class_max[k].max(eps);
        sum[k] += vol[c] * eps * eps;
        vsum[k] += vol[c];
        if eps > e.max {
            e.max = eps;
            e.max_at = (c, k, centres[c]);
        }
    }
    for k in 0..4 {
        e.class_rms[k] = (sum[k] / vsum[k]).sqrt();
    }
    e
}

/// `band_errors` of `overset_f450.rs` (ADR 0376 amendment 1): the largest error of the calculated cells of a part within
/// two face steps of a fringe cell of that part (`band`), and of the other calculated cells of that part (`out`).
fn band_errors(s: &OversetSolver, part_background: bool) -> (f64, f64) {
    let nb = s.conn.n_background_cells;
    let n = s.geom.n_cells();
    let in_part = |c: usize| (c < nb) == part_background;
    let (own, nei) = (s.geom.mesh.owner(), s.geom.mesh.neighbour());
    let mut dist = vec![usize::MAX; n];
    for c in 0..n {
        if in_part(c) && s.conn.kind[c] == CellKind::Fringe {
            dist[c] = 0;
        }
    }
    for step in 1..=2 {
        let mut next = dist.clone();
        for f in 0..s.geom.n_internal {
            for (x, y) in [(own[f], nei[f]), (nei[f], own[f])] {
                if dist[x] == step - 1 && s.conn.kind[y] == CellKind::Calculated && dist[y] == usize::MAX {
                    next[y] = step;
                }
            }
        }
        dist = next;
    }
    let centres = s.geom.mesh.cell_centres();
    let scale = (-2.0 * NU_TG * T_END).exp();
    let (mut band, mut out) = (0.0f64, 0.0f64);
    for c in 0..n {
        if !(in_part(c) && s.conn.kind[c] == CellKind::Calculated) {
            continue;
        }
        let eps = vec3::norm(vec3::sub(s.u.values[c], tg_u(centres[c], T_END))) / scale;
        if dist[c] <= 2 {
            band = band.max(eps);
        } else {
            out = out.max(eps);
        }
    }
    (band, out)
}

fn print_run(name: &str, r: &Run, e: &Errors) {
    println!(
        "{name}: {:.1} s; {} steps; refused: {:?}; continuity {:.2e}; hole flux {:.2e}; boundary flux {:.2e}; \
         unconverged {}; fallbacks {}; kind changes {}",
        r.secs, r.steps, r.failed, r.cont, r.hole, r.boundary, r.unconverged, r.fallbacks, r.changes
    );
    let per_class: Vec<String> = (0..4)
        .map(|k| format!("{} {:.4e} / {:.4e}", CLASSES[k], e.class_max[k], e.class_rms[k]))
        .collect();
    println!(
        "{name}: largest / root-mean-square error by class: {}; largest {:.4e} in cell {} ({}) at ({:.4}, {:.4})",
        per_class.join(", "),
        e.max,
        e.max_at.0,
        CLASSES[e.max_at.1],
        e.max_at.2[0],
        e.max_at.2[1]
    );
}

/// D5 (G) in the band `B`, moving, from ADR 0376's addendum (printed for comparison, not re-run).
const E_B_G_ADR0376: [f64; 2] = [9.7081e-4, 1.7016e-4];

/// The two runs of a level and the checks A1 to A4, A7, C3 (and K at level 1) of ADR 0409.
fn level(c: &mut Checks, level: usize) {
    let qm = tg_run(level, Mode::Q);
    let dm = tg_run(level, Mode::Default);
    let (eq, ed) = (errors(&qm.s), errors(&dm.s));
    let tag = format!("TG_{level}");
    print_run(&format!("{tag} Q moving"), &qm, &eq);
    print_run(&format!("{tag} default moving"), &dm, &ed);
    let (bq, bd) = (band_errors(&qm.s, true), band_errors(&dm.s, true));
    let (cq, cd) = (band_errors(&qm.s, false), band_errors(&dm.s, false));
    println!(
        "{tag} (printed): background band Q {:.4e}, default {:.4e}, D5 of ADR 0376 {:.4e} ({:.4} times the default); \
         other background calculated Q {:.4e}, default {:.4e} (ratio {:.4}); component band Q {:.4e}, default {:.4e} \
         (ratio {:.4}); other component calculated ratio {:.4}",
        bq.0,
        bd.0,
        E_B_G_ADR0376[level - 1],
        E_B_G_ADR0376[level - 1] / bd.0,
        bq.1,
        bd.1,
        bq.1 / bd.1,
        cq.0,
        cd.0,
        cq.0 / cd.0,
        cq.1 / cd.1
    );
    c.at_most(&format!("A1 {tag} e_B(Q) / e_B(default)"), bq.0 / bd.0, 1.2);
    c.at_most(&format!("A2 {tag} e_max(Q) / e_max(default)"), eq.max / ed.max, 1.2);
    c.at_most(&format!("A3 {tag} Q continuity"), qm.cont, 1e-9);
    c.at_most(&format!("A4 {tag} Q flux into the hole region"), qm.hole, 1e-9);
    c.at_most(
        &format!("A4 {tag} Q flux through the domain boundary"),
        qm.boundary,
        1e-9,
    );
    c.holds(
        &format!("A7 {tag} Q: no refused step"),
        qm.failed.is_none() && qm.steps == 200 * level * level,
        &format!("{} steps, refused: {:?}", qm.steps, qm.failed),
    );
    for (name, r) in [("Q", &qm), ("default", &dm)] {
        c.holds(
            &format!("C3 {tag} {name}: no unconverged solve, no fallback"),
            r.unconverged == 0 && r.fallbacks == 0,
            &format!("{} unconverged, {} fallbacks", r.unconverged, r.fallbacks),
        );
    }
    if level == 1 {
        c.at_least("K (set-up) TG_1 Q kind changes", qm.changes as f64, 20.0);
    }
}

/// ADR 0409 at level 1 (A1 to A4, A7, C3, K): the fast test.
#[test]
fn a_tg_level_1_second_order_ring_gradient() {
    let mut c = Checks::new();
    level(&mut c, 1);
    c.finish();
}

/// ADR 0409 at level 2 (A1 to A4, A7, C3): the heavy record.
#[test]
#[ignore = "slow (TG_2, two runs of 800 steps, ADR 0409, about six minutes): run once"]
fn a_tg_level_2_second_order_ring_gradient() {
    let mut c = Checks::new();
    level(&mut c, 2);
    c.finish();
}

// ---- A5: UA, UA2 and U1 of ADR 0347 (helpers of overset_f445.rs) ------------------------------------------------------------

const H1: f64 = PI / 40.0;
const HALF: f64 = 7.5 * H1;
const HOLE: f64 = 3.1 * H1;

fn box_overset(bg: Mesh, half: f64, n: usize, hole: f64) -> Overset {
    let cs = uniform(-half, 2.0 * half, n);
    let comp = rectilinear_2d(&cs, &cs, DEPTH).unwrap();
    let composite = compose(&bg, &comp, "c_").unwrap();
    let spec = OversetSpec {
        outer_patches: ["c_xmin", "c_xmax", "c_ymin", "c_ymax"].map(String::from).to_vec(),
        hole: Arc::new(move |x: Vec3| x[0].abs().max(x[1].abs()) < hole),
    };
    Overset::new(composite, spec).unwrap()
}

/// RE of ADR 0340 at level 1 (as `re(1)` of `overset_f445.rs`): the overset and the pose.
fn re1() -> (Overset, Pose) {
    let bs = uniform(0.0, PI, 40);
    let bg = rectilinear_2d(&bs, &bs, DEPTH).unwrap();
    let d = (16.5 + 0.25) * H1;
    let pose = Pose {
        origin: [0.0; 3],
        axis: [0.0, 0.0, 1.0],
        angle: 0.0,
        translation: [d, d, 0.0],
    };
    (box_overset(bg, HALF, 15, HOLE), pose)
}

const UA_ACC: Vec3 = [2.0, 1.0, 0.0];

fn ua_u(t: f64) -> Vec3 {
    [1.0 + 2.0 * t, 0.5 + t, 0.0]
}

/// `ua_run` of `overset_f445.rs` under `mode`, with `offset` added to the starting pressure of the calculated component
/// cells: the largest relative velocity error, the largest pressure error over the steps and over the steps after the
/// first, the largest continuity.
fn ua_run(mode: Mode, offset: f64) -> (f64, f64, f64, f64) {
    let (ov, pose) = re1();
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
    apply(&mut s, mode);
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

/// The set-up U of ADR 0219 at `t = 0` (`u_new` of `overset_f445.rs`).
fn u_new() -> OversetSolver {
    let bs = uniform(-2.5, 5.0, 50);
    let bg = rectilinear_2d(&bs, &bs, DEPTH).unwrap();
    let ov = box_overset(bg, 0.6, 16, 0.25);
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

/// `u_run` of `overset_f445.rs` under `mode`.
fn u_run(mode: Mode) -> (f64, f64, f64, f64, f64, usize, usize) {
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
    (eu, ep, cont, hole, q, changes, unconverged)
}

/// ADR 0409 A5: UA, UA2 and U1 of ADR 0347 under G with the second-order ring gradient.
#[test]
fn a5_ua_ua2_u1_under_the_second_order_ring_gradient() {
    let mut c = Checks::new();
    let (eu, ep, _, cont) = ua_run(Mode::Q, 0.0);
    c.at_most("A5 UA velocity", eu, 1e-10);
    c.at_most("A5 UA pressure", ep, 1e-9);
    c.at_most("A5 UA continuity", cont, 1e-9);
    let (eu, ep, ep_later, cont) = ua_run(Mode::Q, 0.7);
    println!("A5 UA2: largest pressure error including the first step {ep:.4e}");
    c.at_most("A5 UA2 velocity", eu, 1e-10);
    c.at_most("A5 UA2 pressure after the first step", ep_later, 1e-9);
    c.at_most("A5 UA2 continuity", cont, 1e-9);
    let (eu, ep, cont, hole, q, changes, unconverged) = u_run(Mode::Q);
    c.at_most("A5 U1 velocity", eu, 1e-10);
    c.at_most("A5 U1 pressure", ep, 1e-10);
    c.at_most("A5 U1 continuity", cont, 1e-9);
    c.at_most("A5 U1 flux through Gamma_B", hole, 1e-9);
    c.at_most("A5 U1 outflow", q, 1e-10);
    c.at_least("A5 U1 (set-up) kind changes", changes as f64, 20.0);
    c.holds(
        "A5 U1: no unconverged linear solve",
        unconverged == 0,
        &format!("{unconverged} unconverged"),
    );
    c.finish();
}

// ---- A6: bit-identity without the opt-in -----------------------------------------------------------------------------------

/// 40 steps of TG_1 moving (`b1_hash` of `overset_f450.rs`) under `mode`; `explicit_off` switches the opt-in off by a call.
fn hash_40(mode: Mode, donors: DonorInterpolation, explicit_off: bool) -> u64 {
    let mut s = tg_solver(1, mode, donors);
    if explicit_off {
        s.set_second_order_ring_gradient(false);
    }
    let dt = 0.0025;
    for k in 1..=40 {
        let t = k as f64 * dt;
        let g = s.geom.clone();
        tg_conditions(&g, &mut s.u, t);
        s.step(dt, tg_pose(t), &[]).unwrap();
    }
    state_hash(&s)
}

/// The hashes of ADR 0376 B1 (default coupling; library of main 56a14aa5).
const B1_QUADRATIC: u64 = 0xe43a026a778e83b3;
const B1_LINEAR: u64 = 0x9c5ab1e5b21d5368;
/// G without the opt-in, quadratic stencils: measured once on the registration commit eb13160d (unedited library) before
/// any other run (ADR 0409 A6; log `w49i-work/f479-scratch/b-g-base.log`).
const G_EB13160D: u64 = 0xb2b43812eb166b04;

/// ADR 0409 A6: without the opt-in the default coupling and G give the bytes of the library before F-479.
#[test]
fn a6_without_the_opt_in_every_bit_is_kept() {
    let (q, l) = (hash_40(Mode::Default, QD, false), hash_40(Mode::Default, LD, false));
    let (g, g_off) = (hash_40(Mode::G, QD, false), hash_40(Mode::G, QD, true));
    println!("A6: default quadratic {q:016x}, default linear {l:016x}, G {g:016x}, G with the opt-in set off {g_off:016x}");
    assert_eq!(q, B1_QUADRATIC, "default, quadratic stencils");
    assert_eq!(l, B1_LINEAR, "default, linear stencils");
    assert_eq!(g, G_EB13160D, "G");
    assert_eq!(g_off, G_EB13160D, "G, opt-in set off by a call");
}

// ---- R: refusal outside G ----------------------------------------------------------------------------------------------------

/// ADR 0409 R: a step with the opt-in on under the default coupling or under `Prescribed` is refused.
#[test]
fn r_second_order_ring_gradient_only_under_g() {
    for mode in [FringeFlux::PressureEquation, FringeFlux::Prescribed] {
        let mut s = tg_solver(1, Mode::Default, LD);
        s.set_fringe_flux(mode);
        assert!(!s.second_order_ring_gradient());
        s.set_second_order_ring_gradient(true);
        assert!(s.second_order_ring_gradient());
        let g = s.geom.clone();
        tg_conditions(&g, &mut s.u, 0.0025);
        let err = s.step(0.0025, tg_pose(0.0025), &[]).err().expect("refused");
        println!("R ({mode:?}): {err}");
        assert!(err.contains("ADR 0409"), "{err}");
    }
}
