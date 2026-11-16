//! F-720 (ADR 0640): periodic sliding at coinciding nodes. Checks A1, A1c, X1 (a uniform axial flow on ADR 0629's 3-D
//! sector pair S3 with a step that lands on a node coincidence every 8th step, through two re-imagings), A2, X2 (a uniform
//! flow on the planar pair PP likewise), S1 (the coincidence steps' residuals), and the negative controls N1 (a flow without
//! the part symmetry leaves the part means in the remapped old flux) and N2 (A1 sees the coincidence steps). The limits are
//! those registered in ADR 0640 before this code ran; each test prints every measured number and then asserts the
//! registered checks it carries.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::{FixedValue, ZeroGradient};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::sliding_periodic::{periodic_sliding_step, PeriodicSlidingStep};
use rcfd_fv::{FvGeometry, Profile, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{annulus_3d_sector, annulus_sector, rectilinear_2d, Grading};
use rcfd_mesh::sliding_periodic::{PeriodicSlidingInterface, PeriodicSlidingMesh};
use rcfd_mesh::{Mesh, PatchKind};
use std::f64::consts::{FRAC_PI_3, PI};
use std::sync::Arc;

const L: f64 = 0.1;
const Z: Vec3 = [0.0, 0.0, 1.0];
const TIGHT: SolveControl = SolveControl {
    tol_abs: 1e-16,
    tol_rel: 1e-13,
    max_iter: 5000,
};

/// CS: the rotor pitch of S3 over 8 (a coincidence every 8th step), 200 steps.
const CS_DT: f64 = PI / 288.0;
const CS_STEPS: usize = 200;
/// CP: the coincidence spacing of PP over 8, 650 steps.
const CP_DT: f64 = 1.0 / 640.0;
const CP_STEPS: usize = 650;
/// CN: the rotor pitch of SS over 8, 40 steps.
const CN_DT: f64 = PI / 576.0;
const CN_STEPS: usize = 40;

/// PS6 of ADR 0629.
fn settings(nu: f64) -> PimpleSettings {
    let mut s = PimpleSettings::piso(nu);
    s.convection = ConvScheme::Central;
    s.non_orth_correctors = 6;
    s.momentum_solver = TIGHT;
    s.pressure_solver = TIGHT;
    s
}

fn uniform(n: usize, a: f64, b: f64) -> Vec<f64> {
    (0..=n).map(|i| a + (b - a) * i as f64 / n as f64).collect()
}

fn periodic_x(mut m: Mesh) -> Mesh {
    m.set_periodic("xmin", "xmax").unwrap();
    m.set_periodic("xmax", "xmin").unwrap();
    m
}

fn pair_pp() -> PeriodicSlidingInterface {
    let a = periodic_x(rectilinear_2d(&uniform(20, 0.0, 1.0), &uniform(8, 0.5, 1.0), L).unwrap());
    let b = periodic_x(rectilinear_2d(&uniform(16, 0.0, 1.0), &uniform(8, 0.0, 0.5), L).unwrap());
    PeriodicSlidingInterface::planar(a, "ymin", b, "ymax", [1.0, 0.0, 0.0], 1e-8).unwrap()
}

fn pair_ss() -> PeriodicSlidingInterface {
    let a = annulus_sector(24, 8, 1.0, 1.5, FRAC_PI_3, Grading::Uniform, L).unwrap();
    let b = annulus_sector(20, 8, 1.5, 2.0, FRAC_PI_3, Grading::Uniform, L).unwrap();
    PeriodicSlidingInterface::sector(a, "outer", b, "inner", vec3::ZERO, Z, FRAC_PI_3, 1e-8).unwrap()
}

fn pair_s3() -> PeriodicSlidingInterface {
    let a = annulus_3d_sector(12, 4, 2, 1.0, 1.5, 0.4, FRAC_PI_3, Grading::Uniform).unwrap();
    let b = annulus_3d_sector(10, 4, 3, 1.5, 2.0, 0.4, FRAC_PI_3, Grading::Uniform).unwrap();
    PeriodicSlidingInterface::sector(a, "outer", b, "inner", vec3::ZERO, Z, FRAC_PI_3, 1e-8).unwrap()
}

fn geom_of(m: &Mesh) -> Arc<FvGeometry> {
    Arc::new(FvGeometry::new(Arc::new(m.clone())).unwrap())
}

fn root(p: &mut [usize], mut i: usize) -> usize {
    while p[i] != i {
        p[i] = p[p[i]];
        i = p[i];
    }
    i
}

/// Connected parts of the interface graph of `m` (edges: direct faces, then wrap pairs; nodes: the cells they touch).
fn parts(m: &PeriodicSlidingMesh) -> usize {
    let mesh = &m.mesh;
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let mut p: Vec<usize> = (0..mesh.n_cells()).collect();
    let mut touched = vec![false; mesh.n_cells()];
    let mut edges: Vec<(usize, usize)> = m
        .report
        .join
        .interface_faces
        .iter()
        .map(|&f| (own[f], nei[f]))
        .collect();
    edges.extend(m.report.wrap_pairs.iter().map(|&(a, b)| (own[a], own[b])));
    for &(a, b) in &edges {
        touched[a] = true;
        touched[b] = true;
        let (ra, rb) = (root(&mut p, a), root(&mut p, b));
        if ra != rb {
            p[ra] = rb;
        }
    }
    let mut roots: Vec<usize> = (0..mesh.n_cells())
        .filter(|&c| touched[c])
        .map(|c| root(&mut p, c))
        .collect();
    roots.sort_unstable();
    roots.dedup();
    roots.len()
}

fn xhat(x: Vec3) -> Vec3 {
    vec3::unit([x[0], x[1], 0.0])
}

fn per_face(g: &FvGeometry, patch: &str, f: impl Fn(Vec3) -> Vec3) -> Box<FixedValue> {
    let cf = g.mesh.face_centres();
    Box::new(FixedValue(Profile::PerFace(
        g.mesh.patch(patch).unwrap().faces().map(|k| f(cf[k])).collect(),
    )))
}

/// What one step of a run records.
#[derive(Debug, Clone, Copy)]
struct StepRecord {
    n: usize,
    parts: usize,
    dropped: usize,
    reimaged: bool,
    flux_residual: f64,
    gcl_residual: f64,
    eu: f64,
    ep: f64,
}

fn finite_or_inf(x: f64) -> f64 {
    if x.is_nan() {
        f64::INFINITY
    } else {
        x
    }
}

/// ADR 0634's turned loop with a uniform flow `u0` fixed on every non-empty non-cyclic patch; `alter` may change the step
/// data (with whether the step lands on a coincidence) before the solver takes it. Stops at the first refused step.
fn uniform_run(
    sc: &PeriodicSlidingInterface,
    u0: Vec3,
    dt: f64,
    steps: usize,
    alter: &mut dyn FnMut(&mut PeriodicSlidingStep, bool),
) -> (Vec<StepRecord>, Option<String>) {
    let names: Vec<String> = sc
        .mesh_at(0.0)
        .unwrap()
        .mesh
        .patches()
        .iter()
        .filter(|p| p.kind != PatchKind::Empty && p.kind != PatchKind::Cyclic)
        .map(|p| p.name.clone())
        .collect();
    let mut cur = sc.mesh_turned(0.0).unwrap();
    let g = geom_of(&cur.mesh);
    let mut u = VectorField::uniform("U", &g, u0);
    for n in &names {
        u.set_bc(&g, n, Box::new(FixedValue::vector(u0))).unwrap();
    }
    let p = ScalarField::uniform("p", &g, 0.0);
    let mut s = PimpleSolver::new(g, u, p, settings(0.1));
    let un = vec3::norm(u0);
    let mut out = Vec::with_capacity(steps);
    for n in 1..=steps {
        let new = match sc.mesh_turned(n as f64 * dt) {
            Ok(m) => m,
            Err(e) => return (out, Some(e.to_string())),
        };
        let mut st = match periodic_sliding_step(&s.geom, &cur, &new, &s.phi, &s.u.values, dt) {
            Ok(st) => st,
            Err(e) => return (out, Some(e)),
        };
        let dropped = new.report.join.n_dropped;
        let rec_parts = parts(&new);
        let (flux_residual, gcl_residual) = (st.flux_residual, st.gcl_residual);
        alter(&mut st, dropped > 0);
        let reimaged = new.image != cur.image;
        if let Err(e) = s.step_sliding_periodic(dt, st, Vec::new()) {
            return (out, Some(e));
        }
        cur = new;
        let eu =
            s.u.values
                .iter()
                .fold(0.0f64, |a, v| a.max(vec3::norm(vec3::sub(*v, u0))))
                / un;
        let p0 = s.p.values[0];
        let ep = s.p.values.iter().fold(0.0f64, |a, p| a.max((p - p0).abs())) / (un * un);
        out.push(StepRecord {
            n,
            parts: rec_parts,
            dropped,
            reimaged,
            flux_residual,
            gcl_residual,
            eu: finite_or_inf(eu),
            ep: finite_or_inf(ep),
        });
    }
    (out, None)
}

fn max_of(r: &[StepRecord], sel: impl Fn(&StepRecord) -> bool, val: impl Fn(&StepRecord) -> f64) -> f64 {
    r.iter().filter(|x| sel(x)).map(val).fold(0.0, f64::max)
}

type Fails = Vec<(&'static str, String)>;

fn check(fails: &mut Fails, id: &'static str, ok: bool, what: String) {
    println!("{id}: {what} -> {}", if ok { "met" } else { "MISSED" });
    if !ok {
        fails.push((id, what));
    }
}

/// The set-up check X1 / X2: `parts` parts and dropped overlaps on exactly the steps `n % 8 == 0`, one part and none
/// dropped on every other step.
fn coincidence_pattern(r: &[StepRecord], parts_at: usize) -> (bool, usize) {
    let mut n_coinc = 0;
    let ok = r.iter().all(|x| {
        if x.n % 8 == 0 {
            n_coinc += 1;
            x.parts == parts_at && x.dropped > 0
        } else {
            x.parts == 1 && x.dropped == 0
        }
    });
    (ok, n_coinc)
}

/// A1, A1c, X1 and S1 (CS) of ADR 0640.
#[test]
fn cs_uniform_axial_flow_through_coincidences_on_the_3d_sector() {
    let (r, ended) = uniform_run(&pair_s3(), Z, CS_DT, CS_STEPS, &mut |_, _| {});
    assert!(ended.is_none(), "{ended:?}");
    let coinc = |x: &StepRecord| x.n % 8 == 0;
    let after = |x: &StepRecord| x.n % 8 == 1 && x.n > 1;
    let reim = |x: &StepRecord| x.reimaged;
    let all = |_: &StepRecord| true;
    let (eu, ep) = (max_of(&r, all, |x| x.eu), max_of(&r, all, |x| x.ep));
    let (eu_c, ep_c) = (max_of(&r, coinc, |x| x.eu), max_of(&r, coinc, |x| x.ep));
    let (eu_a, ep_a) = (max_of(&r, after, |x| x.eu), max_of(&r, after, |x| x.ep));
    let (eu_r, ep_r) = (max_of(&r, reim, |x| x.eu), max_of(&r, reim, |x| x.ep));
    let reimaged: Vec<usize> = r.iter().filter(|x| x.reimaged).map(|x| x.n).collect();
    let n_after = r.iter().filter(|x| after(x)).count();
    let (pattern, n_coinc) = coincidence_pattern(&r, 2);
    let (fr, gr) = (
        max_of(&r, coinc, |x| x.flux_residual),
        max_of(&r, coinc, |x| x.gcl_residual),
    );
    let mut fails = Fails::new();
    check(
        &mut fails,
        "A1",
        eu <= 1e-10 && ep <= 1e-10,
        format!(
            "CS, {} steps: velocity {eu:.2e} |U|, pressure {ep:.2e} |U|^2 (1e-10 each)",
            r.len()
        ),
    );
    check(
        &mut fails,
        "A1c",
        [eu_c, ep_c, eu_a, ep_a, eu_r, ep_r].iter().all(|x| *x <= 1e-10),
        format!(
            "CS: {n_coinc} coincidence steps {eu_c:.2e}, {ep_c:.2e}; {n_after} steps after them {eu_a:.2e}, {ep_a:.2e}; re-imaging steps {eu_r:.2e}, {ep_r:.2e} (1e-10 each)"
        ),
    );
    check(
        &mut fails,
        "X1",
        pattern && n_coinc == 25 && reimaged == [97, 193],
        format!("CS: 2 parts exactly on the {n_coinc} steps n % 8 == 0, 1 part elsewhere: {pattern}; re-imaging steps {reimaged:?} ([97, 193])"),
    );
    check(
        &mut fails,
        "S1",
        fr <= 1e-12 && gr <= 1e-12,
        format!("CS coincidence steps: flux_residual {fr:.2e}, gcl_residual {gr:.2e} (1e-12 each)"),
    );
    assert!(fails.is_empty(), "{fails:?}");
}

/// A2, X2 and S1 (CP) of ADR 0640.
#[test]
fn cp_uniform_flow_through_coincidences_on_the_planar_slide() {
    let u0 = [1.0, 0.5, 0.0];
    let (r, ended) = uniform_run(&pair_pp(), u0, CP_DT, CP_STEPS, &mut |_, _| {});
    assert!(ended.is_none(), "{ended:?}");
    let coinc = |x: &StepRecord| x.n % 8 == 0;
    let all = |_: &StepRecord| true;
    let (eu, ep) = (max_of(&r, all, |x| x.eu), max_of(&r, all, |x| x.ep));
    let (eu_c, ep_c) = (max_of(&r, coinc, |x| x.eu), max_of(&r, coinc, |x| x.ep));
    let reimaged: Vec<usize> = r.iter().filter(|x| x.reimaged).map(|x| x.n).collect();
    let (pattern, n_coinc) = coincidence_pattern(&r, 4);
    let (fr, gr) = (
        max_of(&r, coinc, |x| x.flux_residual),
        max_of(&r, coinc, |x| x.gcl_residual),
    );
    let mut fails = Fails::new();
    check(
        &mut fails,
        "A2",
        [eu, ep, eu_c, ep_c].iter().all(|x| *x <= 1e-10),
        format!(
            "CP, {} steps: velocity {eu:.2e} |U|, pressure {ep:.2e} |U|^2; {n_coinc} coincidence steps {eu_c:.2e}, {ep_c:.2e} (1e-10 each)",
            r.len()
        ),
    );
    check(
        &mut fails,
        "X2",
        pattern && n_coinc == 81 && reimaged == [640],
        format!("CP: 4 parts exactly on the {n_coinc} steps n % 8 == 0, 1 part elsewhere: {pattern}; re-imaging steps {reimaged:?} ([640])"),
    );
    check(
        &mut fails,
        "S1",
        fr <= 1e-12 && gr <= 1e-12,
        format!("CP coincidence steps: flux_residual {fr:.2e}, gcl_residual {gr:.2e} (1e-12 each)"),
    );
    assert!(fails.is_empty(), "{fails:?}");
}

/// N1 of ADR 0640: SS with a source flow periodic in the pitch but not in the part spacing `pi/6`; the base step leaves the
/// part means in the remapped old flux on the coincidence steps (the latent defect, outside the acceptance).
#[test]
fn n1_non_symmetric_source_flow_leaves_the_part_means_at_coincidences() {
    let sc = pair_ss();
    let inlet = |x: Vec3| {
        let th = x[1].atan2(x[0]);
        vec3::scale(xhat(x), 1.0 + 0.5 * (6.0 * th).sin())
    };
    let mut cur = sc.mesh_turned(0.0).unwrap();
    let g = geom_of(&cur.mesh);
    let mut u = VectorField::from_fn("U", &g, |c| vec3::scale(xhat(c), 1.0 / c[0].hypot(c[1])));
    u.set_bc(&g, "inner", per_face(&g, "inner", inlet)).unwrap();
    u.set_bc(&g, "outer", Box::new(ZeroGradient)).unwrap();
    let mut p = ScalarField::uniform("p", &g, 0.0);
    p.set_bc(&g, "outer", Box::new(FixedValue::scalar(0.0))).unwrap();
    let mut s = PimpleSolver::new(g, u, p, settings(0.1));
    let mut coinc = Vec::new();
    let mut other: f64 = 0.0;
    for n in 1..=CN_STEPS {
        let new = sc.mesh_turned(n as f64 * CN_DT).unwrap();
        let st = periodic_sliding_step(&s.geom, &cur, &new, &s.phi, &s.u.values, CN_DT).unwrap();
        if n % 8 == 0 {
            coinc.push((n, parts(&new), st.flux_residual));
        } else if n >= 2 {
            other = other.max(finite_or_inf(st.flux_residual));
        }
        let g2 = st.geom.clone();
        s.u.set_bc(&g2, "inner", per_face(&g2, "inner", inlet)).unwrap();
        s.step_sliding_periodic(CN_DT, st, Vec::new()).unwrap();
        cur = new;
    }
    println!("N1: coincidence steps (step, parts, flux_residual): {coinc:?}");
    let mut fails = Fails::new();
    check(
        &mut fails,
        "N1",
        coinc.len() == 5 && coinc.iter().all(|x| x.2 >= 1e-6) && other <= 1e-12,
        format!(
            "CN: flux_residual on the {} coincidence steps, smallest {:.2e} (>= 1e-6 each); other steps from step 2 {other:.2e} (<= 1e-12)",
            coinc.len(),
            coinc.iter().map(|x| x.2).fold(f64::INFINITY, f64::min)
        ),
    );
    assert!(fails.is_empty(), "{fails:?}");
}

/// N2 of ADR 0640: CS with the mesh flux of the direct interface faces and of both wrap copies zeroed on the coincidence
/// steps only.
#[test]
fn n2_uniform_axial_flow_negative_control_sliver_gcl_open_on_coincidence_steps() {
    let mut zero = |st: &mut PeriodicSlidingStep, coinc: bool| {
        if coinc {
            for &f in &st.interface_faces {
                st.mesh_flux[f] = 0.0;
            }
            for &(a, b) in &st.wrap_pairs {
                st.mesh_flux[a] = 0.0;
                st.mesh_flux[b] = 0.0;
            }
        }
    };
    let (r, ended) = uniform_run(&pair_s3(), Z, CS_DT, CS_STEPS, &mut zero);
    let eu = max_of(&r, |_| true, |x| x.eu);
    let held = eu >= 1e-6;
    let mut fails = Fails::new();
    check(
        &mut fails,
        "N2",
        held,
        format!("CS, GCL open on the coincidence steps: largest velocity {eu:.2e} |U| (>= 1e-6) over {} steps, ended early: {ended:?}", r.len()),
    );
    assert!(fails.is_empty(), "{fails:?}");
}
