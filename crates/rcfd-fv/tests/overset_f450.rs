//! F-450 (ADR 0376): the prescribed fringe flux of ADR 0347 (`OversetSolver::set_fringe_flux(PrescribedOwnGradient)`,
//! variant G) with the quadratic donor interpolation of ADR 0251, on the Taylor-Green set-up TG_L of ADR 0219 with the
//! moving, turning component and the time step of its amendment 2 (`dt` proportional to `h^2`), against the default
//! coupling with the same stencils. The set-up helpers are those of `overset_f336.rs` (copied), the conservation measures
//! and the state hash those of `overset_f445.rs`.
//!
//! Release only (the flow runs take minutes): `cargo test --release -p rcfd-fv --test overset_f450`.

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
const G: FringeFlux = FringeFlux::PrescribedOwnGradient;
const DEFAULT: FringeFlux = FringeFlux::PressureEquation;
const Q: DonorInterpolation = DonorInterpolation::Quadratic;
const L: DonorInterpolation = DonorInterpolation::Linear;
const SIDES: [&str; 4] = ["xmin", "xmax", "ymin", "ymax"];

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

// ---- set-up (ADR 0219 TG_L, helpers of overset_f336.rs) --------------------------------------------------------------------

fn uniform(x0: f64, len: f64, n: usize) -> Vec<f64> {
    (0..=n).map(|i| x0 + len * i as f64 / n as f64).collect()
}

/// TG_L of ADR 0219 with the donor interpolation `mode`.
fn tg_overset(level: usize, mode: DonorInterpolation) -> Overset {
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
    if mode != DonorInterpolation::Linear {
        ov.set_donor_interpolation(mode);
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

/// Set the exact velocity at time `t` on the four background sides.
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

fn order(coarse: f64, fine: f64, ratio: f64) -> f64 {
    (coarse / fine).ln() / ratio.ln()
}

/// A TG_L solver at `t = 0` with the fringe mode `mode` and the donor interpolation `donors`.
fn tg_solver(level: usize, mode: FringeFlux, donors: DonorInterpolation) -> OversetSolver {
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
    if mode != DEFAULT {
        s.set_fringe_flux(mode);
    }
    s
}

// ---- runs ------------------------------------------------------------------------------------------------------------------

struct Run {
    s: OversetSolver,
    /// Largest continuity over the steps (A2).
    cont: f64,
    /// Largest `abs(hole_flux)` and largest net flux through the four sides, over the largest face area (A3).
    hole: f64,
    boundary: f64,
    /// Unconverged linear solves and the largest `n_fallback` over the steps (C3).
    unconverged: usize,
    fallbacks: usize,
    /// Kind changes summed over the steps (K).
    changes: usize,
    /// Steps taken, and the step that was refused with the message (A4).
    steps: usize,
    failed: Option<(usize, String)>,
    secs: f64,
}

/// TG_L with `200 L^2` steps to `t = 0.5` (ADR 0219 amendment 2), the quadratic stencils, the fringe mode `mode`; the
/// component moving (`tg_pose(t)`) or held at `tg_pose(0)`. A refused step ends the run.
fn tg_run(level: usize, mode: FringeFlux, moving: bool) -> Run {
    let t0 = Instant::now();
    let steps = 200 * level * level;
    let dt = T_END / steps as f64;
    let mut s = tg_solver(level, mode, Q);
    let amax = area_max(&s.geom);
    let (mut cont, mut hole, mut boundary) = (0.0f64, 0.0f64, 0.0f64);
    let (mut unconverged, mut fallbacks, mut changes) = (0usize, s.conn.n_fallback(), 0usize);
    let (mut taken, mut failed) = (0usize, None);
    for k in 1..=steps {
        let t = k as f64 * dt;
        let g = s.geom.clone();
        tg_conditions(&g, &mut s.u, t);
        let pose = if moving { tg_pose(t) } else { tg_pose(0.0) };
        let info = match s.step(dt, pose, &[]) {
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

/// Cell classes: component fringe, component calculated, background fringe, background calculated.
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

/// The errors at `t = 0.5`: `eps_c = |u_c - u_ex| / e^{-2 nu t}`.
struct Errors {
    class_max: [f64; 4],
    class_rms: [f64; 4],
    /// Largest over the calculated and fringe cells (A1), its cell, class and place.
    max: f64,
    max_at: (usize, usize, Vec3),
    /// Largest over the overlap classes CF, CC, BF (printed).
    overlap: f64,
}

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
        overlap: 0.0,
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
        if k != 3 {
            e.overlap = e.overlap.max(eps);
        }
    }
    for k in 0..4 {
        e.class_rms[k] = (sum[k] / vsum[k]).sqrt();
    }
    e
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
        "{name}: largest / root-mean-square error by class: {}; largest {:.4e} in cell {} ({}) at ({:.4}, {:.4}); \
         overlap classes {:.4e}",
        per_class.join(", "),
        e.max,
        e.max_at.0,
        CLASSES[e.max_at.1],
        e.max_at.2[0],
        e.max_at.2[1],
        e.overlap
    );
}

/// The four runs of a level and the checks of ADR 0376 on them; returns `e_max` of G moving (for S1).
fn level(c: &mut Checks, level: usize) -> f64 {
    let gm = tg_run(level, G, true);
    let dm = tg_run(level, DEFAULT, true);
    let gr = tg_run(level, G, false);
    let dr = tg_run(level, DEFAULT, false);
    let (egm, edm, egr, edr) = (errors(&gm.s), errors(&dm.s), errors(&gr.s), errors(&dr.s));
    for (name, r, e) in [
        ("G moving", &gm, &egm),
        ("default moving", &dm, &edm),
        ("G at rest", &gr, &egr),
        ("default at rest", &dr, &edr),
    ] {
        print_run(&format!("TG_{level} {name}"), r, e);
    }
    let tag = format!("TG_{level}");
    c.at_most(&format!("A1 {tag} e_max(G) / e_max(default)"), egm.max / edm.max, 1.2);
    c.at_most(&format!("A2 {tag} G continuity"), gm.cont, 1e-9);
    c.at_most(&format!("A3 {tag} G flux into the hole region"), gm.hole, 1e-9);
    c.at_most(&format!("A3 {tag} G flux through the domain boundary"), gm.boundary, 1e-9);
    c.holds(
        &format!("A4 {tag} G: no refused step"),
        gm.failed.is_none() && gm.steps == 200 * level * level,
        &format!("{} steps, refused: {:?}", gm.steps, gm.failed),
    );
    for (name, r) in [("G moving", &gm), ("default moving", &dm), ("G at rest", &gr), ("default at rest", &dr)] {
        c.holds(
            &format!("C3 {tag} {name}: no unconverged solve, no fallback"),
            r.unconverged == 0 && r.fallbacks == 0,
            &format!("{} unconverged, {} fallbacks", r.unconverged, r.fallbacks),
        );
    }
    if level == 1 {
        c.at_least("K (set-up) TG_1 G kind changes", gm.changes as f64, 20.0);
    }
    let (mg, md) = (egm.overlap / egr.overlap, edm.overlap / edr.overlap);
    println!(
        "{tag} (printed): overlap e_ov G / default moving {:.4}, at rest {:.4}; D: m_G {:.4}, m_default {:.4}, \
         m_G / m_default {:.4}; e_max G / default at rest {:.4}; e_max(G moving) {:.4e}",
        egm.overlap / edm.overlap,
        egr.overlap / edr.overlap,
        mg,
        md,
        mg / md,
        egr.max / edr.max,
        egm.max
    );
    egm.max
}

/// ADR 0376 at level 1 (A1 to A4, C3, K, the split D printed).
#[test]
fn a_tg_level_1_moving_component_under_g() {
    let mut c = Checks::new();
    level(&mut c, 1);
    c.finish();
}

/// ADR 0376 at level 2 (A1 to A4, C3, the split D printed); the heavy record.
#[test]
#[ignore = "slow (TG_2, four runs of 800 steps, ADR 0376, about twelve minutes): run once"]
fn a_tg_level_2_moving_component_under_g() {
    let mut c = Checks::new();
    level(&mut c, 2);
    c.finish();
}

// ---- B1: defaults ----------------------------------------------------------------------------------------------------------

/// 40 steps of TG_1 moving with the default coupling and the donor interpolation `donors`.
fn b1_hash(donors: DonorInterpolation) -> u64 {
    let mut s = tg_solver(1, DEFAULT, donors);
    let dt = 0.0025;
    for k in 1..=40 {
        let t = k as f64 * dt;
        let g = s.geom.clone();
        tg_conditions(&g, &mut s.u, t);
        s.step(dt, tg_pose(t), &[]).unwrap();
    }
    state_hash(&s)
}

/// ADR 0376 B1: the default coupling gives the bytes of the library before F-450, with the quadratic and with the
/// default stencils.
#[test]
fn b1_defaults_bit_identical() {
    let (q, l) = (b1_hash(Q), b1_hash(L));
    println!("B1: TG_1 moving, 40 steps: quadratic {q:016x}, linear {l:016x}");
    assert_eq!(q, B1_QUADRATIC, "quadratic stencils");
    assert_eq!(l, B1_LINEAR, "linear stencils");
}

/// Measured once on the registration commit 4407610a (the library of main 56a14aa5) before any other run.
const B1_QUADRATIC: u64 = 0xe43a026a778e83b3;
const B1_LINEAR: u64 = 0x9c5ab1e5b21d5368;
