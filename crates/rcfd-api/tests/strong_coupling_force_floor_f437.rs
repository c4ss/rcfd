//! F-437 (ADR 0349): a tree-against-library equality test of the strong coupling whose force floor decides the
//! convergence test of every step, on the spring-mounted piston of ADR 0266 (SPm translated by `X`).
//!
//! The limits, settings and mutations were registered in ADR 0349 before this file existed. The case text, the tree run
//! and the library run are those of `strong_coupling_case_f350.rs` (copied, that file is not edited), extended to return
//! the test quantity of every step (the comparison ADR 0266 E1 lacked) and the library's per-iteration records.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::fsi::{
    relaxation_from_added_mass, strong_step_with, BodyCoupling, CouplingStep, Relaxation, StrongCoupling, WallPressure,
};
use rcfd_fv::sixdof::{Body, BodySpec, BodyState, Integrator};
use rcfd_fv::smoothing::{smooth_displacement, PatchClass, SmoothingMethod};
use rcfd_fv::FvGeometry;
use rcfd_linalg::SolveControl;
use rcfd_mesh::motion::patch_points;
use std::f64::consts::TAU;
use std::sync::Arc;

/// Added mass of the column, `rho A L0`.
const M_A: f64 = 1.0 * 0.04 * 1.0;
/// Amplitude (initial displacement) of the piston.
const X: f64 = 0.01;
const MU: f64 = 4.0;
const STEPS: usize = 60;

fn mass(mu: f64) -> f64 {
    M_A / mu
}

/// Spring of period 1 at the mass ratio `mu`.
fn stiffness(mu: f64) -> f64 {
    TAU * TAU * (mass(mu) + M_A)
}

/// `k X`, the spring force at the amplitude.
fn kx() -> f64 {
    stiffness(MU) * X
}

/// The SPm case of ADR 0266 (its arithmetic): `box_3d` of `lx = L0 - X` with the body at `X`.
fn piston(mu: f64, dt: f64, table: &str) -> String {
    let (m, k) = (mass(mu), stiffness(mu));
    format!(
        r#"
[mesh]
generator = "box_3d"
nx = 20
ny = 2
nz = 2
lx = 0.99
ly = 0.2
lz = 0.2
[physics]
nu = 0.01
rho = 1.0
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "zero_gradient"
pressure = "fixed"
pressure_value = 0.0
[boundaries.ymin]
velocity = "zero_gradient"
[boundaries.ymax]
velocity = "zero_gradient"
[boundaries.zmin]
velocity = "zero_gradient"
[boundaries.zmax]
velocity = "zero_gradient"
[solver]
algorithm = "piso"
convection = "central"
dt = {dt:?}
end_time = 11.5
momentum_rel_tol = 1e-13
momentum_max_iter = 5000
pressure_rel_tol = 1e-13
pressure_abs_tol = 1e-16
pressure_max_iter = 5000
[dynamic_mesh]
enabled = true
motion = "six_dof"
moving_patches = ["xmin"]
fixed_patches = ["xmax"]
deformation = "smoothing"
[dynamic_mesh.six_dof]
mass = {m:?}
inertia = [1.0, 1.0, 1.0]
position = [{X:?}, 0.0, 0.0]
translation_locked = [false, true, true]
stiffness = [{k:?}, 0.0, 0.0]
{table}
"#
    )
}

/// ADR 0266's registered strong settings with the floors of ADR 0349 (`wall_pressure` not written: the default).
fn table(force_scale: f64, moment_scale: f64) -> String {
    format!(
        "[dynamic_mesh.six_dof.coupling]\nscheme = \"strong\"\nrelaxation = \"aitken\"\nadded_mass = {M_A:?}\ntolerance = 1e-8\nforce_scale = {force_scale:?}\nmoment_scale = {moment_scale:?}\nmax_iterations = 20\n"
    )
}

fn settings(force_scale: f64, moment_scale: f64) -> StrongCoupling {
    StrongCoupling {
        relaxation: Relaxation::Aitken {
            initial: relaxation_from_added_mass(mass(MU), M_A),
            max: 2.0,
        },
        tolerance: 1e-8,
        force_scale,
        moment_scale,
        max_iterations: 20,
    }
}

fn parse(text: &str) -> Case {
    let c = Case::parse_toml(text).unwrap_or_else(|d| panic!("{d:?}"));
    assert!(c.semantic_errors().is_empty(), "{:?}", c.semantic_errors());
    c
}

/// Bits of the flow and the body after a step.
#[derive(Debug, PartialEq, Eq)]
struct Bits {
    u: Vec<u64>,
    p: Vec<u64>,
    phi: Vec<u64>,
    x: [u64; 3],
    v: [u64; 3],
}

fn bits_of(flow: &rcfd_fv::pimple::PimpleSolver, body: &Body) -> Bits {
    Bits {
        u: flow.u.values.iter().flatten().map(|a| a.to_bits()).collect(),
        p: flow.p.values.iter().map(|a| a.to_bits()).collect(),
        phi: flow.phi.iter().map(|a| a.to_bits()).collect(),
        x: body.state.x.map(f64::to_bits),
        v: body.state.v.map(f64::to_bits),
    }
}

/// One run: bits after every step, iteration counts, the test quantity of the last iteration (bits), all converged.
struct Run {
    bits: Vec<Bits>,
    iterations: Vec<usize>,
    residual: Vec<u64>,
    converged: bool,
}

fn tree_run(case: &Case, steps: usize) -> Run {
    let mut s = Simulation::new(case).unwrap();
    let mut bits = Vec::new();
    for n in 1..=steps {
        s.step();
        assert!(s.failure().is_none(), "step {n}: {:?}", s.failure());
        bits.push(bits_of(s.pimple_solver().unwrap(), s.dynamic_body().unwrap()));
    }
    let r = s.coupling_records();
    assert_eq!(r.len(), steps);
    Run {
        bits,
        iterations: r.iter().map(|r| r.iterations).collect(),
        residual: r.iter().map(|r| r.residual.to_bits()).collect(),
        converged: r.iter().all(|r| r.converged),
    }
}

/// The library run of ADR 0266 E1 (since ADR 0346 `strong_step_with(PrescribedGradient)` with a carried vector), also
/// returning every step's record.
fn library_run(case: &Case, settings: &StrongCoupling, steps: usize) -> (Run, Vec<CouplingStep>) {
    let mut sim = Simulation::new(case).unwrap();
    let dt = case.get("solver.dt").unwrap().as_f64().unwrap();
    let mesh0 = (*sim.geom.mesh).clone();
    let geom0 = FvGeometry::new(Arc::new(mesh0.clone())).unwrap();
    let classes: Vec<PatchClass> = mesh0
        .patches()
        .iter()
        .map(|p| match p.name.as_str() {
            "xmin" => PatchClass::Prescribed,
            "xmax" => PatchClass::Fixed,
            _ => PatchClass::Slip,
        })
        .collect();
    let control = SolveControl {
        tol_abs: 1e-16,
        tol_rel: 1e-12,
        max_iter: 5000,
    };
    let piston = patch_points(&mesh0, "xmin").unwrap();
    let mut spec = BodySpec::new(mass(MU), [1.0, 1.0, 1.0]);
    spec.translation_locked = [false, true, true];
    spec.stiffness = [stiffness(MU), 0.0, 0.0];
    let mut body = Body::new(spec, BodyState::at([X, 0.0, 0.0])).unwrap();
    let cpl = BodyCoupling {
        rho: 1.0,
        nu: 0.01,
        force_patches: vec!["xmin".into()],
        follow_patches: vec!["xmin".into()],
        integrator: Integrator::Rk4,
    };
    let mut points = |b: &Body| -> Result<Vec<Vec3>, String> {
        let p0 = mesh0.points();
        let mut presc = vec![vec3::ZERO; p0.len()];
        for &q in &piston {
            presc[q] = vec3::sub(b.map_point(p0[q]), p0[q]);
        }
        let sm = smooth_displacement(&geom0, &classes, &presc, SmoothingMethod::Laplacian, &control)?;
        Ok(p0
            .iter()
            .zip(&sm.displacement)
            .map(|(p, d)| vec3::add(*p, *d))
            .collect())
    };
    let flow = sim.pimple_solver_mut().unwrap();
    let mut run = Run {
        bits: Vec::new(),
        iterations: Vec::new(),
        residual: Vec::new(),
        converged: true,
    };
    let mut records = Vec::new();
    let mut carried: Vec<f64> = Vec::new();
    for n in 1..=steps {
        let st = strong_step_with(
            flow,
            &mut body,
            dt,
            &cpl,
            settings,
            WallPressure::PrescribedGradient,
            &mut carried,
            &mut points,
        )
        .unwrap_or_else(|e| panic!("step {n}: {e}"));
        run.iterations.push(st.iterations);
        run.residual.push(st.residuals.last().unwrap().to_bits());
        run.converged &= st.converged;
        run.bits.push(bits_of(flow, &body));
        records.push(st);
    }
    (run, records)
}

fn first_difference<T: PartialEq>(a: &[T], b: &[T]) -> Option<usize> {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).position(|(x, y)| x != y).map(|i| i + 1)
}

/// The tree against the library: bits, iteration counts, test quantities, converged, cap (ADR 0349 F1 and G).
fn compare(name: &str, tree: &Run, lib: &Run) {
    println!(
        "{name}: first differing step: bits {:?}, iterations {:?}, test quantity {:?} of {STEPS}; iterations tree {:?}, \
         library {:?}; converged {} {}",
        first_difference(&tree.bits, &lib.bits),
        first_difference(&tree.iterations, &lib.iterations),
        first_difference(&tree.residual, &lib.residual),
        tree.iterations,
        lib.iterations,
        tree.converged,
        lib.converged
    );
    assert_eq!(first_difference(&tree.bits, &lib.bits), None, "{name} bits");
    assert_eq!(tree.iterations, lib.iterations, "{name} iterations");
    assert_eq!(tree.residual, lib.residual, "{name} test quantities");
    assert!(tree.converged && lib.converged, "{name} converged");
    assert!(tree.iterations.iter().all(|&i| i <= 20), "{name} cap");
}

/// The force of the flow's last answer of a step, `F + r_F` of the last iterate.
fn last_force(st: &CouplingStep) -> f64 {
    vec3::norm(vec3::add(st.force, *st.force_residuals.last().unwrap()))
}

/// ADR 0349 F1, B1, B2: `force_scale = moment_scale = 100 k X`; the floor sets every test quantity.
#[test]
fn f1_tree_equals_library_with_a_binding_force_floor() {
    let s = 100.0 * kx();
    let case = parse(&piston(MU, 0.01, &table(s, s)));
    let tree = tree_run(&case, STEPS);
    let (lib, records) = library_run(&case, &settings(s, s), STEPS);
    // B1: every iteration's test quantity is |r_F|/s_F, bit for bit
    let mut b1_misses = Vec::new();
    for (n, st) in records.iter().enumerate() {
        assert_eq!(st.residuals.len(), st.force_residuals.len());
        for (k, (e, rf)) in st.residuals.iter().zip(&st.force_residuals).enumerate() {
            if e.to_bits() != (vec3::norm(*rf) / s).to_bits() {
                b1_misses.push((n + 1, k + 1, *e, vec3::norm(*rf) / s));
            }
        }
    }
    let (b2_step, b2) = records
        .iter()
        .enumerate()
        .map(|(n, st)| (n + 1, last_force(st) / s))
        .fold((0, 0.0_f64), |a, b| if b.1 > a.1 { b } else { a });
    let rf1 = vec3::norm(*records[0].force_residuals.last().unwrap());
    let n_iter: usize = records.iter().map(|st| st.iterations).sum();
    println!(
        "B1: {} of {n_iter} iterations where the test quantity is not |r_F|/s_F: {b1_misses:?}\nB2: largest |F + r_F|/s_F \
         at the last iterate {b2:.4e} (step {b2_step}); |r_F| at the last iterate of step 1 {rf1:.3e}; test quantities \
         (library) {:?}",
        b1_misses.len(),
        records.iter().map(|st| *st.residuals.last().unwrap()).collect::<Vec<_>>()
    );
    compare("F1", &tree, &lib);
    assert!(b1_misses.is_empty(), "B1 {b1_misses:?}");
    assert!(b2 <= 0.1, "B2 {b2}");
    assert!(rf1 > 0.0, "B2 r_F of step 1");
}

/// ADR 0349 G, GB (negative control): `force_scale = 1e-9 k X`, where the floor does not bind; equal with and without the
/// mutation "the driver ignores force_scale".
#[test]
fn g_tree_equals_library_with_a_force_floor_that_does_not_bind() {
    let (sf, sm) = (1e-9 * kx(), 100.0 * kx());
    let case = parse(&piston(MU, 0.01, &table(sf, sm)));
    let tree = tree_run(&case, STEPS);
    let (lib, records) = library_run(&case, &settings(sf, sm), STEPS);
    let (gb_step, gb) = records
        .iter()
        .enumerate()
        .map(|(n, st)| (n + 1, last_force(st) / sf))
        .fold((0, f64::INFINITY), |a, b| if b.1 < a.1 { b } else { a });
    println!("GB: smallest |F + r_F|/s_F' at the last iterate {gb:.4e} (step {gb_step})");
    compare("G", &tree, &lib);
    assert!(gb >= 1e3, "GB {gb}");
}
