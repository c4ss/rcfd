//! F-357 (ADR 0255): the uniform-flow check U1 of ADR 0219 on the warped three-dimensional overset set-up B3d of ADR 0251
//! with the component moving (turning and translating), with the default and with the quadratic donor interpolation (A1,
//! A2, with continuity, the GCL, the outflow and the set-up checks); the separating checks on the single warped mesh at
//! rest and moving and on the planar B3 (S0, S1, SB, SP); the negative controls N1 (weights) and N2 (mesh flux).
//!
//! Release: `cargo test --release -p rcfd-fv --test overset_f357`.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::ale::move_mesh;
use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::overset::{MassMode, OversetCoupling, OversetSolver};
use rcfd_fv::pimple::PimpleSettings;
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{box_mesh, warp, Grading};
use rcfd_mesh::motion::{moved_mesh, Pose};
use rcfd_mesh::overset::{compose, CellKind, DonorInterpolation, Overset, OversetSpec};
use rcfd_mesh::Mesh;
use std::sync::Arc;
use std::time::Instant;

/// ADR 0219 PS.
const SOLVE: SolveControl = SolveControl {
    tol_abs: 1e-15,
    tol_rel: 1e-12,
    max_iter: 3000,
};
const NU: f64 = 0.01;
const U_FLOW: Vec3 = [1.0, 0.5, 0.25];
const DT: f64 = 0.005;
const STEPS: usize = 40;
/// Area of the outflow patch `xmax` of the background.
const A_XMAX: f64 = 2.4 * 2.4;

fn settings() -> PimpleSettings {
    let mut s = PimpleSettings::piso(NU);
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
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

// ---- the set-ups of ADR 0251 (the same code as crates/rcfd-mesh/tests/overset_f352.rs) ------------------------------------

/// A fixed pseudo-random number in [-1, 1] of a position.
fn jitter(x: Vec3, seed: f64) -> f64 {
    let v = (x[0] * 127.1 + x[1] * 311.7 + x[2] * 74.7 + seed).sin() * 43758.5453;
    2.0 * (v - v.floor()) - 1.0
}

fn distorted(mesh: &Mesh, lo: f64, hi: f64, dirs: &[usize], amp: f64, planar: bool) -> Mesh {
    let dirs = dirs.to_vec();
    warp(mesh, move |p| {
        let tol = 1e-9 * (hi - lo);
        if dirs.iter().any(|&d| p[d] - lo < tol || hi - p[d] < tol) {
            return p;
        }
        let key = if planar { [p[0], p[1], 0.0] } else { p };
        let mut q = p;
        for &d in &dirs {
            q[d] += amp * jitter(key, 1.0 + d as f64);
        }
        q
    })
    .unwrap()
}

/// The background and the component of B3 (`distort`: B3d) of ADR 0251.
fn b3_parts(distort: bool) -> (Mesh, Mesh) {
    let mut bg = box_mesh([24; 3], [2.4; 3], [Grading::Uniform; 3]).unwrap();
    let mut comp = box_mesh([14; 3], [1.4; 3], [Grading::Uniform; 3]).unwrap();
    if distort {
        bg = distorted(&bg, 0.0, 2.4, &[0, 1, 2], 0.015, false);
        comp = distorted(&comp, 0.0, 1.4, &[0, 1, 2], 0.015, false);
    }
    (bg, comp)
}

/// B3 of ADR 0251 (`distort`: B3d) with the donor interpolation `mode`.
fn b3(distort: bool, mode: DonorInterpolation) -> Overset {
    let (bg, comp) = b3_parts(distort);
    let spec = OversetSpec {
        outer_patches: ["c_xmin", "c_xmax", "c_ymin", "c_ymax", "c_zmin", "c_zmax"]
            .map(String::from)
            .to_vec(),
        hole: Arc::new(|x: Vec3| (0..3).map(|d| (x[d] - 0.7).abs()).fold(0.0, f64::max) < 0.15),
    };
    let mut ov = Overset::new(compose(&bg, &comp, "c_").unwrap(), spec).unwrap();
    if mode != DonorInterpolation::Linear {
        ov.set_donor_interpolation(mode);
    }
    ov
}

/// The motion of ADR 0255: `P(0)` is the pose of ADR 0251.
fn pose(t: f64) -> Pose {
    Pose {
        origin: [0.7; 3],
        axis: vec3::unit([1.0, 2.0, 3.0]),
        angle: 0.3 + 0.5 * t,
        translation: [0.53 - 0.25 * t, 0.52 + 0.25 * t, 0.49 + 0.25 * t],
    }
}

/// The velocity patches of the background (every side but the outflow `xmax`).
const INFLOW: [&str; 5] = ["xmin", "ymin", "ymax", "zmin", "zmax"];

fn fields(geom: &Arc<FvGeometry>) -> (VectorField, ScalarField) {
    let mut u = VectorField::uniform("U", geom, U_FLOW);
    for name in INFLOW {
        u.set_bc(geom, name, Box::new(FixedValue::vector(U_FLOW))).unwrap();
    }
    let mut p = ScalarField::uniform("p", geom, 0.0);
    p.set_bc(geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    (u, p)
}

fn patch_flux(geom: &FvGeometry, phi: &[f64], patch: &str) -> f64 {
    geom.mesh.patch(patch).unwrap().faces().map(|f| phi[f]).sum()
}

/// The deviations of a uniform state over the cells `keep`.
fn deviations(u: &VectorField, p: &ScalarField, keep: impl Fn(usize) -> bool) -> (f64, f64) {
    let un = vec3::norm(U_FLOW);
    let (mut eu, mut ep) = (0.0f64, 0.0f64);
    for c in 0..u.values.len() {
        if keep(c) {
            eu = eu.max(vec3::norm(vec3::sub(u.values[c], U_FLOW)) / un);
            ep = ep.max(p.values[c].abs() / (un * un));
        }
    }
    (eu, ep)
}

/// The GCL residual `max_c abs(V1 - V0 - dt sum_f sigma_cf m_f) / mean(V)` over the cells `cells`.
fn gcl_residual(old: &Mesh, new: &Mesh, mesh_flux: &[f64], cells: std::ops::Range<usize>) -> f64 {
    let n = old.n_cells();
    let mut sum = vec![0.0; n];
    for f in 0..new.n_faces() {
        sum[new.owner()[f]] += mesh_flux[f];
        if f < new.n_internal_faces() {
            sum[new.neighbour()[f]] -= mesh_flux[f];
        }
    }
    let (v0, v1) = (old.cell_volumes(), new.cell_volumes());
    let mean_v = cells.clone().map(|c| v0[c]).sum::<f64>() / cells.len() as f64;
    cells
        .map(|c| (v1[c] - v0[c] - DT * sum[c]).abs() / mean_v)
        .fold(0.0, f64::max)
}

/// What a test may do to a prepared step (the negative controls).
#[derive(Clone, Copy, PartialEq)]
enum Control {
    None,
    /// N1: every stencil weight multiplied by 1.001.
    Weights,
    /// N2: the mesh flux of every internal face of the component perturbed by `1e-6 |S_f| j_f`.
    MeshFlux,
}

/// The largest values of a run over its steps.
#[derive(Default, Debug)]
struct Run {
    eu: f64,
    ep: f64,
    cont: f64,
    gcl: f64,
    outflow: f64,
    changes: usize,
    unconverged: usize,
    min_share: f64,
    max_fallback: usize,
    stencils: usize,
    seconds: f64,
}

/// W3 (`distort`), P3 (planar) with the donor interpolation `mode`, 40 steps of the motion `P`.
fn overset_run(distort: bool, mode: DonorInterpolation, control: Control) -> Run {
    let t0 = Instant::now();
    let ov = b3(distort, mode);
    let p0 = pose(0.0);
    let mesh = moved_mesh(&ov.composite.mesh, ov.composite.points_at(&p0)).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let (u, p) = fields(&geom);
    let mut s = OversetSolver::new(
        geom,
        u,
        p,
        settings(),
        MassMode::Conservative,
        Some(OversetCoupling { overset: ov, pose: p0 }),
    )
    .unwrap();
    let nb = s.conn.n_background_cells;
    let n = s.geom.n_cells();
    let mut r = Run {
        min_share: 1.0,
        ..Run::default()
    };
    for k in 1..=STEPS {
        let mut st = s.prepare(DT, pose(k as f64 * DT), &[]).unwrap();
        match control {
            Control::None => {}
            Control::Weights => {
                for (_, stn) in st.conn.stencils.iter_mut() {
                    stn.weights.iter_mut().for_each(|w| *w *= 1.001);
                }
            }
            Control::MeshFlux => {
                let m = &st.geom.mesh;
                for f in 0..m.n_internal_faces() {
                    if m.owner()[f] >= nb {
                        let j = jitter(m.face_centres()[f], 9.0);
                        st.mesh_flux[f] += 1e-6 * vec3::norm(m.face_area_vectors()[f]) * j;
                    }
                }
            }
        }
        r.gcl = r
            .gcl
            .max(gcl_residual(&s.geom.mesh, &st.geom.mesh, &st.mesh_flux, nb..n));
        let info = s.advance(DT, st).unwrap();
        r.cont = r.cont.max(info.continuity);
        r.changes += info.n_changed;
        r.unconverged += info.unconverged;
        r.max_fallback = r.max_fallback.max(info.n_fallback);
        let ns = s.conn.stencils.len();
        r.stencils += ns;
        if mode == DonorInterpolation::Quadratic {
            assert_eq!(s.conn.interpolation, DonorInterpolation::Quadratic);
            r.min_share = r.min_share.min(1.0 - info.n_fallback as f64 / ns as f64);
        }
        let (eu, ep) = deviations(&s.u, &s.p, |c| s.conn.kind[c] != CellKind::Hole);
        r.eu = r.eu.max(eu);
        r.ep = r.ep.max(ep);
        let q = patch_flux(&s.geom, &s.phi, "xmax");
        r.outflow = r.outflow.max((q - U_FLOW[0] * A_XMAX).abs() / (U_FLOW[0] * A_XMAX));
    }
    r.seconds = t0.elapsed().as_secs_f64();
    println!("{r:?}");
    r
}

/// M0, M1, MB: one mesh alone (every cell calculated), placed by `place(t, x_ref)` at every step (`moving`) or left at
/// its first placement.
fn single_run(reference: &Mesh, moving: bool, place: impl Fn(f64, Vec3) -> Vec3) -> Run {
    let t0 = Instant::now();
    let at = |t: f64| -> Vec<Vec3> { reference.points().iter().map(|&x| place(t, x)).collect() };
    let mesh = moved_mesh(reference, at(0.0)).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let (u, p) = fields(&geom);
    let mut s = OversetSolver::new(geom, u, p, settings(), MassMode::Conservative, None).unwrap();
    let n = s.geom.n_cells();
    let mut r = Run::default();
    for k in 1..=STEPS {
        let pts = if moving {
            at(k as f64 * DT)
        } else {
            s.geom.mesh.points().to_vec()
        };
        let ms = move_mesh(&s.geom, pts, DT).unwrap();
        r.gcl = r
            .gcl
            .max(gcl_residual(&s.geom.mesh, &ms.geom.mesh, &ms.mesh_flux, 0..n));
        let info = s.advance_mesh(DT, ms, Vec::new()).unwrap();
        r.cont = r.cont.max(info.continuity);
        r.unconverged += info.unconverged;
        let (eu, ep) = deviations(&s.u, &s.p, |_| true);
        r.eu = r.eu.max(eu);
        r.ep = r.ep.max(ep);
    }
    r.seconds = t0.elapsed().as_secs_f64();
    println!("{r:?}");
    r
}

/// The limits of A1 and A1c (and U2 where it applies) on a separating run.
fn uniform_limits(c: &mut Checks, id: &str, r: &Run, gcl: bool) {
    c.at_most(&format!("{id} velocity"), r.eu, 1e-10);
    c.at_most(&format!("{id} pressure"), r.ep, 1e-10);
    c.at_most(&format!("{id} continuity"), r.cont, 1e-9);
    if gcl {
        c.at_most(&format!("{id} GCL"), r.gcl, 1e-12);
    }
}

// ---- A: the acceptance ---------------------------------------------------------------------------------------------------

/// ADR 0255 A1, A1c, U2 to U5 on W3 (B3d moving, the default donor interpolation).
#[test]
fn a1_w3_uniform_flow_default_interpolation() {
    let r = overset_run(true, DonorInterpolation::Linear, Control::None);
    println!(
        "W3: {:.1} s, {} stencils over {STEPS} steps, {} kind changes, unconverged solves (U5) {}",
        r.seconds, r.stencils, r.changes, r.unconverged
    );
    let mut c = Checks::new();
    c.at_most("A1 velocity", r.eu, 1e-10);
    c.at_most("A1 pressure", r.ep, 1e-10);
    c.at_most("A1c continuity", r.cont, 1e-9);
    c.at_most("U2 GCL", r.gcl, 1e-12);
    c.at_most("U3 outflow", r.outflow, 1e-10);
    c.at_least("U4 (set-up) kind changes", r.changes as f64, 20.0);
    c.finish();
}

/// ADR 0255 A2, A2c, A2s, U2, U3 on W3q (B3d moving, the quadratic donor interpolation).
#[test]
fn a2_w3q_uniform_flow_quadratic_interpolation() {
    let r = overset_run(true, DonorInterpolation::Quadratic, Control::None);
    println!(
        "W3q: {:.1} s, {} stencils over {STEPS} steps, largest fallback count of a step {}, {} kind changes, \
         unconverged solves (U5) {}",
        r.seconds, r.stencils, r.max_fallback, r.changes, r.unconverged
    );
    let mut c = Checks::new();
    c.at_most("A2 velocity", r.eu, 1e-10);
    c.at_most("A2 pressure", r.ep, 1e-10);
    c.at_most("A2c continuity", r.cont, 1e-9);
    c.at_least("A2s (set-up) share of quadratic stencils", r.min_share, 0.95);
    c.at_most("U2 GCL (W3q)", r.gcl, 1e-12);
    c.at_most("U3 outflow (W3q)", r.outflow, 1e-10);
    c.finish();
}

// ---- S: separating checks ------------------------------------------------------------------------------------------------

/// ADR 0255 S0, S1, SB: the single warped meshes of B3d without overset, the component at rest and moving, the
/// background at rest.
#[test]
fn s_single_warped_mesh_at_rest_and_moving() {
    let (bg, comp) = b3_parts(true);
    let p0 = pose(0.0);
    let s0 = single_run(&comp, false, |_, x| p0.apply(x));
    let s1 = single_run(&comp, true, |t, x| pose(t).apply(x));
    let sb = single_run(&bg, false, |_, x| x);
    println!(
        "S0 {:.1} s, S1 {:.1} s, SB {:.1} s; unconverged solves {} {} {}",
        s0.seconds, s1.seconds, sb.seconds, s0.unconverged, s1.unconverged, sb.unconverged
    );
    let mut c = Checks::new();
    uniform_limits(&mut c, "S0", &s0, false);
    uniform_limits(&mut c, "S1", &s1, true);
    uniform_limits(&mut c, "SB", &sb, false);
    println!("printed: GCL residual S0 {:.3e}, SB {:.3e}", s0.gcl, sb.gcl);
    c.finish();
}

/// ADR 0255 SP: the planar B3 with the same motion (default interpolation).
#[test]
fn sp_planar_b3_uniform_flow() {
    let r = overset_run(false, DonorInterpolation::Linear, Control::None);
    println!(
        "P3: {:.1} s, {} kind changes, GCL {:.3e}, outflow {:.3e}",
        r.seconds, r.changes, r.gcl, r.outflow
    );
    let mut c = Checks::new();
    uniform_limits(&mut c, "SP", &r, true);
    c.finish();
}

// ---- N: negative controls ------------------------------------------------------------------------------------------------

/// ADR 0255 N1 (weights times 1.001) and N2 (the mesh flux of the component perturbed) on W3.
#[test]
fn n_uniform_flow_negative_controls() {
    let n1 = overset_run(true, DonorInterpolation::Linear, Control::Weights);
    let n2 = overset_run(true, DonorInterpolation::Linear, Control::MeshFlux);
    println!("N2: GCL residual {:.3e}", n2.gcl);
    let mut c = Checks::new();
    c.at_least("N1 velocity deviation", n1.eu, 1e-6);
    c.at_least("N2 velocity deviation", n2.eu, 1e-8);
    c.finish();
}
