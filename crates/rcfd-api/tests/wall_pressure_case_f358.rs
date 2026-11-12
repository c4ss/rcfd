//! F-358 (ADR 0269): the wall pressure of the body force selected from the case tree by the key `wall_pressure` of the
//! optional table `[dynamic_mesh.six_dof.coupling]`; the spring-mounted piston of ADR 0211 at a mass ratio of 0.2 through
//! the case driver; the other callers that evaluate a wall force after a step with a prescribed wall gradient (a wall at
//! rest, the rotor wall of the sliding step); the corrected evaluation on wall cells whose centre is off the face normal;
//! the listing of the cases whose results move under the other default (an ignored record).
//!
//! The limits, set-ups and mutations were registered in ADR 0269 before this file existed. The piston text, the crossings
//! and the turning points are those of `strong_coupling_case_f350.rs` (copied, that file is not edited); the sliding
//! set-up is ADR 0216's rigid rotation RR (copied from `rcfd-fv/tests/sliding_interface_f250.rs`).

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::{FixedValue, Profile};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::fsi::{
    relaxation_from_added_mass, strong_step_with, BodyCoupling, Relaxation, StrongCoupling, WallPressure,
};
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::sixdof::{
    patch_face_forces, patch_force_moment, patch_force_moment_wall_gradient, Body, BodySpec, BodyState, Integrator,
};
use rcfd_fv::sliding::{rotating_wall_conditions, sliding_step};
use rcfd_fv::smoothing::{smooth_displacement, PatchClass, SmoothingMethod};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{box_mesh, cylinder_ogrid, Grading};
use rcfd_mesh::motion::{moved_mesh, patch_points};
use rcfd_mesh::sliding::SlidingInterface;
use rcfd_mesh::Mesh;
use std::f64::consts::TAU;
use std::sync::Arc;

/// Added mass of the column, `rho A L0`.
const M_A: f64 = 1.0 * 0.04 * 1.0;
/// Amplitude (initial displacement) of the piston.
const X: f64 = 0.01;
const KEY: &str = "dynamic_mesh.six_dof.coupling.wall_pressure";
const PRESCRIBED: &str = "[dynamic_mesh.six_dof.coupling]\nwall_pressure = \"prescribed_gradient\"\n";

fn mass(mu: f64) -> f64 {
    M_A / mu
}

/// Spring of period 1 at the mass ratio `mu`.
fn stiffness(mu: f64) -> f64 {
    TAU * TAU * (mass(mu) + M_A)
}

/// ADR 0266's piston text: `box_3d` of `lx = L0 - X` with the body at `X`, so the column is `L0 - x`.
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

/// ADR 0250's registered strong settings as keys (ADR 0266), with `extra` lines in the table.
fn strong_table(mu: f64, extra: &str) -> String {
    format!(
        "[dynamic_mesh.six_dof.coupling]\nscheme = \"strong\"\nrelaxation = \"aitken\"\nadded_mass = {M_A:?}\ntolerance = 1e-8\nforce_scale = {:?}\nmax_iterations = 20\n{extra}",
        stiffness(mu) * X
    )
}

fn registered(mu: f64) -> StrongCoupling {
    StrongCoupling {
        relaxation: Relaxation::Aitken {
            initial: relaxation_from_added_mass(mass(mu), M_A),
            max: 2.0,
        },
        tolerance: 1e-8,
        force_scale: stiffness(mu) * X,
        moment_scale: 0.0,
        max_iterations: 20,
    }
}

/// The loose step as `strong_step_with` at one iteration (ADR 0269 arithmetic).
fn one_iteration() -> StrongCoupling {
    StrongCoupling {
        relaxation: Relaxation::Fixed(1.0),
        tolerance: 1e-8,
        force_scale: 0.0,
        moment_scale: 0.0,
        max_iterations: 1,
    }
}

fn parse(text: &str) -> Case {
    let c = Case::parse_toml(text).unwrap_or_else(|d| panic!("{d:?}"));
    assert!(c.semantic_errors().is_empty(), "{:?}", c.semantic_errors());
    c
}

/// Diagnostic paths of a text: parse errors, then the static checks, then `Simulation::new`.
fn paths(text: &str) -> Vec<String> {
    match Case::parse_toml(text) {
        Err(d) => d.iter().map(|x| x.path.clone()).collect(),
        Ok(c) => {
            let mut d: Vec<String> = c.semantic_errors().iter().map(|x| x.path.clone()).collect();
            if d.is_empty() {
                if let Err(e) = Simulation::new(&c) {
                    d = e.iter().map(|x| x.path.clone()).collect();
                }
            }
            d
        }
    }
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

fn bits_of(flow: &PimpleSolver, body: &Body) -> Bits {
    Bits {
        u: flow.u.values.iter().flatten().map(|a| a.to_bits()).collect(),
        p: flow.p.values.iter().map(|a| a.to_bits()).collect(),
        phi: flow.phi.iter().map(|a| a.to_bits()).collect(),
        x: body.state.x.map(f64::to_bits),
        v: body.state.v.map(f64::to_bits),
    }
}

fn vec_bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|a| a.to_bits()).collect()
}

struct TreeRun {
    bits: Vec<Bits>,
    vectors: Vec<Vec<f64>>,
    x: Vec<f64>,
    iterations: Vec<usize>,
    converged: bool,
}

/// The tree's steps: the bits, the kept vector and the body position after every step, and the iteration counts.
fn tree_run(case: &Case, steps: usize) -> TreeRun {
    let mut s = Simulation::new(case).unwrap();
    let mut out = TreeRun {
        bits: Vec::new(),
        vectors: Vec::new(),
        x: Vec::new(),
        iterations: Vec::new(),
        converged: true,
    };
    for n in 1..=steps {
        s.step();
        assert!(s.failure().is_none(), "step {n}: {:?}", s.failure());
        out.bits
            .push(bits_of(s.pimple_solver().unwrap(), s.dynamic_body().unwrap()));
        out.vectors.push(s.dynamic_wall_gradients().to_vec());
        out.x.push(s.dynamic_body().unwrap().state.x[0]);
    }
    for r in s.coupling_records() {
        out.iterations.push(r.iterations);
        out.converged &= r.converged;
    }
    out
}

/// `strong_step_with` on the flow solver of a second `Simulation` of the same case, the body, the points and the
/// coupling built here from the case's numbers, the vector carried here (ADR 0269 E1, E2).
fn library_run(case: &Case, mu: f64, settings: &StrongCoupling, wp: WallPressure, steps: usize) -> TreeRun {
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
    let mut spec = BodySpec::new(mass(mu), [1.0, 1.0, 1.0]);
    spec.translation_locked = [false, true, true];
    spec.stiffness = [stiffness(mu), 0.0, 0.0];
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
    let mut carried: Vec<f64> = Vec::new();
    let mut out = TreeRun {
        bits: Vec::new(),
        vectors: Vec::new(),
        x: Vec::new(),
        iterations: Vec::new(),
        converged: true,
    };
    for n in 1..=steps {
        let st = strong_step_with(flow, &mut body, dt, &cpl, settings, wp, &mut carried, &mut points)
            .unwrap_or_else(|e| panic!("step {n}: {e}"));
        out.iterations.push(st.iterations);
        out.converged &= st.converged;
        out.bits.push(bits_of(flow, &body));
        out.vectors.push(carried.clone());
        out.x.push(body.state.x[0]);
    }
    out
}

fn first_difference(a: &[Bits], b: &[Bits]) -> Option<usize> {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).position(|(x, y)| x != y).map(|i| i + 1)
}

fn first_vector_difference(a: &[Vec<f64>], b: &[Vec<f64>]) -> Option<usize> {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .position(|(x, y)| vec_bits(x) != vec_bits(y))
        .map(|i| i + 1)
}

// ---- the measurements of ADR 0250 (copied from strong_coupling_case_f350.rs) ----

fn upward_crossings(t: &[f64], x: &[f64]) -> Vec<f64> {
    (1..x.len())
        .filter(|&i| x[i - 1] < 0.0 && x[i] >= 0.0)
        .map(|i| t[i - 1] + (t[i] - t[i - 1]) * (-x[i - 1]) / (x[i] - x[i - 1]))
        .collect()
}

fn measured_omega(t: &[f64], x: &[f64]) -> (f64, usize) {
    let c = upward_crossings(t, x);
    assert!(c.len() >= 2, "fewer than two upward crossings");
    (TAU * (c.len() - 1) as f64 / (c[c.len() - 1] - c[0]), c.len())
}

fn turning_points(x: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let mut maxima = vec![x[0]];
    let mut minima = Vec::new();
    for i in 1..x.len() - 1 {
        let (a, b, c) = (x[i - 1], x[i], x[i + 1]);
        let curv = a - 2.0 * b + c;
        if b > a && b >= c && curv != 0.0 {
            maxima.push(b - (c - a).powi(2) / (8.0 * curv));
        } else if b < a && b <= c && curv != 0.0 {
            minima.push(b - (c - a).powi(2) / (8.0 * curv));
        }
    }
    (maxima, minima)
}

fn worst_growth(x: &[f64]) -> f64 {
    let (mx, mn) = turning_points(x);
    let mut worst: f64 = 0.0;
    for w in mx.windows(2) {
        worst = worst.max(w[1] / w[0]);
    }
    for w in mn.windows(2) {
        worst = worst.max(w[1].abs() / w[0].abs());
    }
    worst
}

/// `omega_m/omega` (the period is 1) and the crossings of a run of `steps` steps of `dt`, the body starting at `X`.
fn omega_ratio(dt: f64, x: &[f64]) -> (f64, usize) {
    let t: Vec<f64> = (0..=x.len()).map(|n| n as f64 * dt).collect();
    let mut xs = vec![X];
    xs.extend_from_slice(x);
    let (w, n) = measured_omega(&t, &xs);
    (w / TAU, n)
}

// ---- T0, Z0, Z1 ----

/// ADR 0269 T0: without the table the texts hold no `wall_pressure` and the key reads its default (`prescribed_gradient`
/// since ADR 0346, `extrapolated` before); with it they do.
#[test]
fn t0_a_case_without_the_table_keeps_its_text() {
    let without = parse(&piston(0.2, 0.01, ""));
    let with = parse(&piston(0.2, 0.01, PRESCRIBED));
    let (tw, jw) = (without.to_toml(), without.to_json_string());
    println!(
        "T0: settings hash without {:016x}, with {:016x}",
        without.tree.content_hash(),
        with.tree.content_hash()
    );
    assert!(!tw.contains("wall_pressure") && !jw.contains("wall_pressure"));
    assert!(without
        .tree
        .to_json()
        .pointer("/dynamic_mesh/six_dof/coupling")
        .is_none());
    assert_eq!(without.get(KEY).unwrap(), "prescribed_gradient");
    assert_eq!(with.get(KEY).unwrap(), "prescribed_gradient");
    assert!(with.to_toml().contains("wall_pressure = \"prescribed_gradient\""));
    assert!(with.to_json_string().contains("\"wall_pressure\""));
    assert_ne!(without.tree.content_hash(), with.tree.content_hash());
}

/// ADR 0269 Z0: a table with only the default value of `wall_pressure` equals no table, bit for bit (loose). ADR 0346:
/// the default is `prescribed_gradient` (the old statement, `extrapolated`, is ADR 0346 X1 against the old tree).
#[test]
fn z0_extrapolated_key_equals_no_table_loose() {
    let table = PRESCRIBED;
    let a = tree_run(&parse(&piston(0.2, 0.01, "")), 100);
    let b = tree_run(&parse(&piston(0.2, 0.01, table)), 100);
    println!(
        "Z0: first differing step {:?} of 100",
        first_difference(&a.bits, &b.bits)
    );
    assert_eq!(first_difference(&a.bits, &b.bits), None);
}

/// ADR 0269 Z1: ADR 0266's strong table with the default value of `wall_pressure` equals the table without it (ADR 0346:
/// `prescribed_gradient`; `extrapolated` before).
#[test]
fn z1_extrapolated_key_equals_the_strong_table_without_it() {
    let mu = 4.0;
    let a = tree_run(&parse(&piston(mu, 0.01, &strong_table(mu, ""))), 40);
    let b = tree_run(
        &parse(&piston(
            mu,
            0.01,
            &strong_table(mu, "wall_pressure = \"prescribed_gradient\"\n"),
        )),
        40,
    );
    println!(
        "Z1: first differing step {:?} of 40; iterations {:?}",
        first_difference(&a.bits, &b.bits),
        a.iterations
    );
    assert_eq!(first_difference(&a.bits, &b.bits), None);
    assert_eq!(a.iterations, b.iterations);
}

// ---- E1, W1, E2 ----

/// ADR 0269 E1 and W1: the loose loop with the key equals `strong_step_with(PrescribedGradient)` at one iteration, bit for
/// bit, with the kept vector; the kept vector is the fixed-flux gradient of the piston's acceleration.
#[test]
fn e1_w1_loose_tree_equals_library_and_keeps_the_vector() {
    let (mu, dt) = (0.2, 0.01);
    let case = parse(&piston(mu, dt, PRESCRIBED));
    let t = tree_run(&case, 100);
    let l = library_run(&case, mu, &one_iteration(), WallPressure::PrescribedGradient, 100);
    let (db, dv) = (
        first_difference(&t.bits, &l.bits),
        first_vector_difference(&t.vectors, &l.vectors),
    );
    println!("E1: first differing step {db:?} of 100 (state), {dv:?} (kept vector)");
    // W1
    let g = Simulation::new(&case).unwrap().geom.clone();
    let (ni, nb) = (g.n_internal, g.mesh.n_boundary_faces());
    let piston_faces: Vec<usize> = g.mesh.patch("xmin").unwrap().faces().collect();
    let mut xs = vec![X];
    xs.extend_from_slice(&t.x);
    let w = |n: usize| if n == 0 { 0.0 } else { (xs[n] - xs[n - 1]) / dt };
    let mut worst: f64 = 0.0;
    let mut layout = true;
    for n in 2..=5 {
        let v = &t.vectors[n - 1];
        layout &= v.len() == nb;
        let expect = (w(n) - w(n - 1)) / dt;
        for (i, e) in v.iter().enumerate() {
            if piston_faces.contains(&(i + ni)) {
                worst = worst.max((e - expect).abs() / expect.abs());
            } else {
                layout &= e.is_nan();
            }
        }
    }
    println!(
        "W1: one entry per boundary face and NaN off the piston: {layout}; worst relative error {worst:.3e} (1e-9)"
    );
    assert_eq!(db, None);
    assert_eq!(dv, None);
    assert!(layout);
    assert!(worst <= 1e-9, "{worst}");
}

/// ADR 0269 E2: the strong path with the key equals `strong_step_with(PrescribedGradient)` with ADR 0250's settings.
#[test]
fn e2_strong_tree_equals_library() {
    let mu = 4.0;
    let case = parse(&piston(
        mu,
        0.01,
        &strong_table(mu, "wall_pressure = \"prescribed_gradient\"\n"),
    ));
    let t = tree_run(&case, 60);
    let l = library_run(&case, mu, &registered(mu), WallPressure::PrescribedGradient, 60);
    println!(
        "E2: first differing step {:?} of 60 (state), {:?} (vector); iterations tree {:?}, library {:?}; converged {} {}",
        first_difference(&t.bits, &l.bits),
        first_vector_difference(&t.vectors, &l.vectors),
        t.iterations,
        l.iterations,
        t.converged,
        l.converged
    );
    assert_eq!(first_difference(&t.bits, &l.bits), None);
    assert_eq!(first_vector_difference(&t.vectors, &l.vectors), None);
    assert_eq!(t.iterations, l.iterations);
    assert!(t.converged && l.converged);
}

// ---- A1 to A4 ----

/// SP of ADR 0211 through the tree: `omega_m/omega` and the crossings after 650 steps of `T/100`.
fn sp_run(table: &str) -> (f64, usize, f64) {
    let dt = 0.01;
    let r = tree_run(&parse(&piston(0.2, dt, table)), 650);
    let (w, n) = omega_ratio(dt, &r.x);
    let mut xs = vec![X];
    xs.extend_from_slice(&r.x);
    (w, n, worst_growth(&xs))
}

/// ADR 0269 A1 to A3 (acceptance a3): the spring-mounted piston of ADR 0211 (mass ratio 0.2) through the case driver.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn a1_a3_spring_mounted_piston_at_mass_ratio_0_2() {
    let (w1, n1, g1) = sp_run(PRESCRIBED);
    // ADR 0346: the control is the old wall pressure, written (it was the default, no table, before ADR 0346)
    let (w2, n2, g2) = sp_run("[dynamic_mesh.six_dof.coupling]\nwall_pressure = \"extrapolated\"\n");
    println!(
        "A1: with the key omega_m/omega = {w1:.6} ({n1} crossings, largest turning ratio {g1:.5}); limit |. - 1| <= 5e-4"
    );
    println!(
        "A2: without the key {w2:.6} ({n2} crossings, largest turning ratio {g2:.5}); limits |. - 1.001307| <= 1e-5 and . - 1 > 5e-4"
    );
    assert!(n1 >= 6 && n2 >= 6, "{n1} {n2}");
    assert!((w1 - 1.0).abs() <= 5e-4, "{w1}");
    assert!((w2 - 1.001307).abs() <= 1e-5, "{w2}");
    assert!(w2 - 1.0 > 5e-4, "{w2}");
}

/// ADR 0269 A4: the strong path with the key at a mass ratio of 4, `T/100`, 11.5 periods.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn a4_strong_piston_at_mass_ratio_4_with_the_key() {
    let (mu, dt) = (4.0, 0.01);
    let r = tree_run(
        &parse(&piston(
            mu,
            dt,
            &strong_table(mu, "wall_pressure = \"prescribed_gradient\"\n"),
        )),
        1150,
    );
    let (w, n) = omega_ratio(dt, &r.x);
    let largest = r.iterations.iter().copied().max().unwrap_or(0);
    let mean = r.iterations.iter().sum::<usize>() as f64 / r.iterations.len() as f64;
    println!(
        "A4: omega_m/omega = {w:.6} ({n} crossings); converged {}; iterations largest {largest}, mean {mean:.3}",
        r.converged
    );
    assert!((w - 1.0).abs() <= 1e-3, "{w}");
    assert!((w - 1.000042).abs() <= 1e-5, "{w}");
    assert!(r.converged);
    assert!(largest <= 8, "{largest}");
}

// ---- N ----

/// ADR 0269 N (D3): the static checks of the key are located at it.
#[test]
fn n_static_checks_are_located() {
    let bogus = piston(
        0.2,
        0.01,
        "[dynamic_mesh.six_dof.coupling]\nwall_pressure = \"bogus\"\n",
    );
    let pb = paths(&bogus);
    println!("N: unknown value: {pb:?}");
    assert!(pb.iter().any(|p| p == KEY), "{pb:?}");
    let six = piston(0.2, 0.01, PRESCRIBED);
    let start = six.find("[dynamic_mesh.six_dof]").unwrap();
    let end = six.find("[dynamic_mesh.six_dof.coupling]").unwrap();
    let prescribed = format!("{}{}", &six[..start], &six[end..]).replace(
        "motion = \"six_dof\"",
        "motion = \"prescribed\"\ndisplacement_expression_x = \"0.01 [m] * sin(t / 1 [s])\"",
    );
    let pp = paths(&prescribed);
    println!("N: prescribed motion: {pp:?}");
    assert!(pp.iter().any(|p| p == KEY), "{pp:?}");
    let ok = paths(&six);
    println!("N: the valid case: {ok:?}");
    assert!(ok.is_empty(), "{ok:?}");
    let off = paths(&prescribed.replace("enabled = true", "enabled = false"));
    println!("N: section disabled: {off:?}");
    assert!(off.is_empty(), "{off:?}");
}

// ---- SK0 to SK3: the corrected evaluation on wall cells off the face normal ----

/// A box `[n, 2, 2]` of `[1, 0.2, 0.2]` with every point `y' = y + gamma x` and the points of the plane `x = 1/n` moved
/// by `kappa / n` in y (ADR 0269 meshes S1 and S3).
fn skewed_box(n: usize, gamma: f64, kappa: f64) -> Arc<FvGeometry> {
    let m = box_mesh([n, 2, 2], [1.0, 0.2, 0.2], [Grading::Uniform; 3]).unwrap();
    let dx = 1.0 / n as f64;
    let pts: Vec<Vec3> = m
        .points()
        .iter()
        .map(|&p| {
            let mut q = p;
            q[1] += gamma * p[0];
            if (p[0] - dx).abs() <= 1e-12 {
                q[1] += kappa * dx;
            }
            q
        })
        .collect();
    let mesh: Mesh = moved_mesh(&m, pts).unwrap();
    Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap())
}

/// Relative x-force error `F/F_exact - 1` on `xmin` of the corrected and the default evaluation for the field `p`, the
/// wall's stored condition the default zero gradient, exact values on `ymin`, `ymax`, `xmax`, `g = 3` on `xmin`.
fn sk_errors(g: &FvGeometry, p: &dyn Fn(Vec3) -> f64) -> (f64, f64) {
    let mut pf = ScalarField::from_fn("p", g, p);
    let cf = g.mesh.face_centres();
    for name in ["ymin", "ymax", "xmax"] {
        let v: Vec<Vec3> = g
            .mesh
            .patch(name)
            .unwrap()
            .faces()
            .map(|f| [p(cf[f]), 0.0, 0.0])
            .collect();
        pf.set_bc(g, name, Box::new(FixedValue(Profile::PerFace(v)))).unwrap();
    }
    let u = VectorField::uniform("U", g, vec3::ZERO);
    let (ni, nb) = (g.n_internal, g.mesh.n_boundary_faces());
    let mut dpdn = vec![f64::NAN; nb];
    let mut exact = 0.0;
    for f in g.mesh.patch("xmin").unwrap().faces() {
        dpdn[f - ni] = 3.0;
        exact += p(cf[f]) * g.mesh.face_area_vectors()[f][0];
    }
    let wall = vec!["xmin".to_string()];
    let (fc, _) = patch_force_moment_wall_gradient(g, &u, &pf, 1.0, 0.0, &wall, vec3::ZERO, &dpdn).unwrap();
    let (fd, _) = patch_force_moment(g, &u, &pf, 1.0, 0.0, &wall, vec3::ZERO).unwrap();
    (fc[0] / exact - 1.0, fd[0] / exact - 1.0)
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}

/// ADR 0269 SK0 to SK3 (acceptance a6).
#[test]
fn sk_corrected_wall_pressure_on_skewed_wall_cells() {
    let lin = |x: Vec3| 3.0 * (1.0 - x[0]) + 0.5 * x[1];
    let lin_x = |x: Vec3| 3.0 * (1.0 - x[0]);
    let smooth = |x: Vec3| (2.0 * x[0]).cos() + 3.0 * (1.0 - x[0]) + 0.5 * x[1];
    let n = 20.0;
    // SK0: the unsheared box
    let (c0, d0) = sk_errors(&skewed_box(20, 0.0, 0.0), &lin_x);
    println!(
        "SK0: corrected {c0:.3e} (1e-12); default {d0:.6e} against -1/(4N) = {:.6e}",
        -1.0 / (4.0 * n)
    );
    // SK1: uniform shear, linear field
    let (c1, d1) = sk_errors(&skewed_box(20, 0.5, 0.0), &lin);
    let p1 = -0.6875 / (3.05 * n);
    println!(
        "SK1: corrected {c1:.3e} (1e-12); default {d1:.6e} against {p1:.6e} (relative {:.2e})",
        rel(d1, p1)
    );
    // SK2: uniform shear, smooth field, orders
    let e: Vec<(f64, f64)> = [20, 40, 80]
        .iter()
        .map(|&k| sk_errors(&skewed_box(k, 0.5, 0.0), &smooth))
        .collect();
    let order = |a: f64, b: f64| (a.abs() / b.abs()).log2();
    let oc = [order(e[0].0, e[1].0), order(e[1].0, e[2].0)];
    let od = [order(e[0].1, e[1].1), order(e[1].1, e[2].1)];
    println!(
        "SK2: corrected {:.4e} {:.4e} {:.4e}, orders {:.4} {:.4} ([1.8, 2.2]); default {:.4e} {:.4e} {:.4e}, orders {:.4} {:.4} ([0.8, 1.2]); corrected predicted -0.5/(4.05 N^2) = {:.4e} on 20 cells",
        e[0].0, e[1].0, e[2].0, oc[0], oc[1], e[0].1, e[1].1, e[2].1, od[0], od[1], -0.5 / (4.05 * n * n)
    );
    // SK3: the first interior plane shifted, linear field
    let (c3, d3) = sk_errors(&skewed_box(20, 0.0, 0.5), &lin_x);
    let (pc3, pd3) = (-1.0 / (38.0 * n), -9.0 / (38.0 * n));
    println!(
        "SK3: corrected {c3:.6e} against {pc3:.6e} (relative {:.2e}); default {d3:.6e} against {pd3:.6e} (relative {:.2e})",
        rel(c3, pc3),
        rel(d3, pd3)
    );
    assert!(c0.abs() <= 1e-12, "{c0}");
    assert!(rel(d0, -1.0 / (4.0 * n)) <= 1e-9, "{d0}");
    assert!(c1.abs() <= 1e-12, "{c1}");
    assert!(rel(d1, p1) <= 1e-9, "{d1}");
    assert!(oc.iter().all(|o| (1.8..=2.2).contains(o)), "{oc:?}");
    assert!(od.iter().all(|o| (0.8..=1.2).contains(o)), "{od:?}");
    assert!(rel(c3, pc3) <= 1e-9, "{c3}");
    assert!(rel(d3, pd3) <= 1e-9, "{d3}");
}

// ---- AU1: a wall at rest on a rigidly moving column ----

fn rest_column() -> String {
    r#"
[mesh]
generator = "box_3d"
nx = 20
ny = 2
nz = 2
lx = 1.0
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
dt = 0.01
end_time = 1.0
momentum_rel_tol = 1e-13
momentum_max_iter = 5000
pressure_rel_tol = 1e-13
pressure_abs_tol = 1e-16
pressure_max_iter = 5000
[dynamic_mesh]
enabled = true
deformation = "rigid"
rest_patches = ["xmin"]
displacement_expression_x = "0.01 [m] * cos(2 * pi * t / 1 [s])"
"#
    .to_string()
}

/// ADR 0269 AU1 (a5, `ale::set_rest_conditions`): the force on a wall at rest, default, corrected and the tree's report.
/// Registered form, missed (amendment 1): worst default `4.083e-2`, corrected `2.617e-2`, report `4.963e-2` (step 3, a
/// start-up part), against `1.25e-2` (1 %), `1e-4` and `2.5e-2` (1 %); past step 6 corrected about `2e-4`.
#[test]
#[ignore = "known miss: AU1 default 4.083e-2, corrected 2.617e-2, report 4.963e-2 at step 3 (ADR 0269 amendment 1)"]
fn au1_wall_at_rest_on_a_rigid_column_known_miss() {
    let case = parse(&rest_column());
    let mut s = Simulation::new(&case).unwrap();
    let wall = vec!["xmin".to_string()];
    let (mut wd, mut wc, mut wr): (f64, f64, f64) = (1.0 / 80.0, 0.0, 1.0 / 40.0);
    for n in 1..=12 {
        s.step();
        assert!(s.failure().is_none(), "step {n}: {:?}", s.failure());
        if n < 3 {
            continue;
        }
        let dpdn = s.dynamic_wall_gradients().to_vec();
        let pfv = s.pressure_face_values();
        let fl = s.pimple_solver().unwrap();
        let g = fl.geom.clone();
        let ni = g.n_internal;
        let (mut area, mut gsum, mut k) = (0.0, 0.0, 0.0);
        for f in g.mesh.patch("xmin").unwrap().faces() {
            area += vec3::norm(g.mesh.face_area_vectors()[f]);
            gsum += dpdn[f - ni];
            k += 1.0;
        }
        let gm = gsum / k;
        let exact = -gm * 1.0 * area;
        let (fd, _) = patch_force_moment(&g, &fl.u, &fl.p, 1.0, 0.01, &wall, vec3::ZERO).unwrap();
        let (fc, _) = patch_force_moment_wall_gradient(&g, &fl.u, &fl.p, 1.0, 0.01, &wall, vec3::ZERO, &dpdn).unwrap();
        let (fr, _) =
            rcfd_post::integrals::wall_force_and_moment(&g, "xmin", &fl.u, &pfv, 0.01, 1.0, vec3::ZERO).unwrap();
        let (dd, dc, dr) = (1.0 - fd[0] / exact, 1.0 - fc[0] / exact, 1.0 - fr.pressure[0] / exact);
        println!("AU1 step {n}: g = {gm:.6e}; deficit default {dd:.7e}, corrected {dc:.3e}, report {dr:.7e}");
        if (dd - 1.0 / 80.0).abs() > (wd - 1.0 / 80.0).abs() {
            wd = dd;
        }
        if dc.abs() > wc.abs() {
            wc = dc;
        }
        if (dr - 1.0 / 40.0).abs() > (wr - 1.0 / 40.0).abs() {
            wr = dr;
        }
    }
    println!(
        "AU1: worst default {wd:.7e} (within 1 % of 1.25e-2), corrected {wc:.3e} (1e-4), report {wr:.7e} (within 1 % of 2.5e-2)"
    );
    assert!(rel(wd, 1.0 / 80.0) <= 0.01, "{wd}");
    assert!(wc.abs() <= 1e-4, "{wc}");
    assert!(rel(wr, 1.0 / 40.0) <= 0.01, "{wr}");
}

/// ADR 0269 amendment 1, X1: prints only, no limit (the rest column per step, registered settings and a converged step).
#[test]
#[ignore = "exploration X1 of ADR 0269 amendment 1: prints only, run once by name"]
fn x1_rest_column_exploration_prints_only() {
    let base = rest_column();
    let conv = base.replace(
        "algorithm = \"piso\"",
        "algorithm = \"pimple\"
n_outer = 4",
    );
    let wall = vec!["xmin".to_string()];
    let xb = |t: f64| 0.01 * (TAU * t).cos();
    for (name, text) in [("piso", base), ("pimple n_outer = 4", conv)] {
        let mut s = Simulation::new(&parse(&text)).unwrap();
        let dt = 0.01;
        let wmax = 0.01 * TAU;
        for n in 1..=20 {
            s.step();
            assert!(s.failure().is_none(), "step {n}: {:?}", s.failure());
            let w = (xb(n as f64 * dt) - xb((n - 1) as f64 * dt)) / dt;
            let dpdn = s.dynamic_wall_gradients().to_vec();
            let pfv = s.pressure_face_values();
            let fl = s.pimple_solver().unwrap();
            let g = fl.geom.clone();
            let ni = g.n_internal;
            let faces: Vec<usize> = g.mesh.patch("xmin").unwrap().faces().collect();
            let wall_cells: Vec<usize> = faces.iter().map(|&f| g.mesh.owner()[f]).collect();
            let area: f64 = faces.iter().map(|&f| vec3::norm(g.mesh.face_area_vectors()[f])).sum();
            let gm = faces.iter().map(|&f| dpdn[f - ni]).sum::<f64>() / faces.len() as f64;
            let exact = -gm * area;
            let du = fl.u.values.iter().map(|v| (v[0] - w).abs()).fold(0.0, f64::max) / wmax;
            let duw = wall_cells
                .iter()
                .map(|&c| (fl.u.values[c][0] - w).abs())
                .fold(0.0, f64::max)
                / wmax;
            let (fd0, _) = patch_force_moment(&g, &fl.u, &fl.p, 1.0, 0.0, &wall, vec3::ZERO).unwrap();
            let (fd, _) = patch_force_moment(&g, &fl.u, &fl.p, 1.0, 0.01, &wall, vec3::ZERO).unwrap();
            let (fc0, _) =
                patch_force_moment_wall_gradient(&g, &fl.u, &fl.p, 1.0, 0.0, &wall, vec3::ZERO, &dpdn).unwrap();
            let (fr, _) =
                rcfd_post::integrals::wall_force_and_moment(&g, "xmin", &fl.u, &pfv, 0.01, 1.0, vec3::ZERO).unwrap();
            // the first row: the cells with centre y, z below 0.1, ordered by x
            let cc = g.mesh.cell_centres();
            let mut row: Vec<usize> = (0..cc.len()).filter(|&c| cc[c][1] < 0.1 && cc[c][2] < 0.1).collect();
            row.sort_by(|a, b| cc[*a][0].partial_cmp(&cc[*b][0]).unwrap());
            let dx = cc[row[1]][0] - cc[row[0]][0];
            let sq = (fl.p.values[row[0]] - fl.p.values[row[1]]) / (gm * dx);
            println!(
                "X1 {name} step {n}: w {w:+.6e}, g {gm:+.6e}; max|u_x - w| {du:.3e} (wall cells {duw:.3e}) of 2 pi X; pressure-only deficit default {:.6e}, corrected {:.3e}, report {:.6e}; viscous part {:.3e}; s - 1 {:+.3e}",
                1.0 - fd0[0] / exact,
                1.0 - fc0[0] / exact,
                1.0 - fr.pressure[0] / exact,
                (fd[0] - fd0[0]) / fd[0],
                sq - 1.0
            );
        }
    }
}

/// ADR 0269 amendment 2, AU1b: AU1 with the box starting at rest at the case's mesh (the registered expression jumped by
/// `X` in the first step).
#[test]
fn au1b_wall_at_rest_on_a_rigid_column_starting_at_rest() {
    let text = rest_column().replace(
        "displacement_expression_x = \"0.01 [m] * cos(2 * pi * t / 1 [s])\"",
        "displacement_expression_x = \"0.01 [m] * (cos(2 * pi * t / 1 [s]) - 1)\"",
    );
    assert!(text.contains("- 1)"));
    let case = parse(&text);
    let mut s = Simulation::new(&case).unwrap();
    let wall = vec!["xmin".to_string()];
    let (mut wd, mut wc, mut wr): (f64, f64, f64) = (1.0 / 80.0, 0.0, 1.0 / 40.0);
    for n in 1..=12 {
        s.step();
        assert!(s.failure().is_none(), "step {n}: {:?}", s.failure());
        if n < 3 {
            continue;
        }
        let dpdn = s.dynamic_wall_gradients().to_vec();
        let pfv = s.pressure_face_values();
        let fl = s.pimple_solver().unwrap();
        let g = fl.geom.clone();
        let ni = g.n_internal;
        let (mut area, mut gsum, mut k) = (0.0, 0.0, 0.0);
        for f in g.mesh.patch("xmin").unwrap().faces() {
            area += vec3::norm(g.mesh.face_area_vectors()[f]);
            gsum += dpdn[f - ni];
            k += 1.0;
        }
        let gm = gsum / k;
        let exact = -gm * 1.0 * area;
        let (fd, _) = patch_force_moment(&g, &fl.u, &fl.p, 1.0, 0.01, &wall, vec3::ZERO).unwrap();
        let (fc, _) = patch_force_moment_wall_gradient(&g, &fl.u, &fl.p, 1.0, 0.01, &wall, vec3::ZERO, &dpdn).unwrap();
        let (fr, _) =
            rcfd_post::integrals::wall_force_and_moment(&g, "xmin", &fl.u, &pfv, 0.01, 1.0, vec3::ZERO).unwrap();
        let (dd, dc, dr) = (1.0 - fd[0] / exact, 1.0 - fc[0] / exact, 1.0 - fr.pressure[0] / exact);
        println!("AU1b step {n}: g = {gm:.6e}; deficit default {dd:.7e}, corrected {dc:.3e}, report {dr:.7e}");
        if (dd - 1.0 / 80.0).abs() > (wd - 1.0 / 80.0).abs() {
            wd = dd;
        }
        if dc.abs() > wc.abs() {
            wc = dc;
        }
        if (dr - 1.0 / 40.0).abs() > (wr - 1.0 / 40.0).abs() {
            wr = dr;
        }
    }
    println!(
        "AU1b: worst default {wd:.7e} (within 1 % of 1.25e-2), corrected {wc:.3e} (1e-4), report {wr:.7e} (within 1 % of 2.5e-2)"
    );
    assert!(rel(wd, 1.0 / 80.0) <= 0.01, "{wd}");
    assert!(wc.abs() <= 1e-4, "{wc}");
    assert!(rel(wr, 1.0 / 40.0) <= 0.01, "{wr}");
}

// ---- AU2: the rotor wall of the sliding step (ADR 0216 RR) ----

const RI: f64 = 1.0;
const RM: f64 = 1.5;
const RO: f64 = 2.0;
const Z: Vec3 = [0.0, 0.0, 1.0];

fn rigid(omega: f64, x: Vec3) -> Vec3 {
    vec3::scale(vec3::cross(Z, x), omega)
}

/// ADR 0269 AU2 (a5, the sliding step): the wall pressure on the rotor wall in rigid rotation.
#[test]
fn au2_rotor_wall_of_the_sliding_step() {
    let (omega, nu, dt, steps) = (1.0, 1.0, 0.0005, 20);
    let a = cylinder_ogrid(144, 8, RI, RM, Grading::Uniform, 0.1).unwrap();
    let b = cylinder_ogrid(120, 8, RM, RO, Grading::Uniform, 0.1).unwrap();
    let sc = SlidingInterface::cylindrical(a, "farfield", b, "cylinder", vec3::ZERO, Z, 1e-8).unwrap();
    let mut cur = sc.mesh_at(0.0).unwrap();
    let g = Arc::new(FvGeometry::new(Arc::new(cur.mesh.clone())).unwrap());
    let mut u = VectorField::from_fn("U", &g, |c| rigid(omega, c));
    let cf = g.mesh.face_centres();
    for name in ["cylinder", "farfield"] {
        let v: Vec<Vec3> = g
            .mesh
            .patch(name)
            .unwrap()
            .faces()
            .map(|f| rigid(omega, cf[f]))
            .collect();
        u.set_bc(&g, name, Box::new(FixedValue(Profile::PerFace(v)))).unwrap();
    }
    let p = ScalarField::from_fn("p", &g, |c| 0.5 * omega * omega * (c[0] * c[0] + c[1] * c[1]));
    let mut set = PimpleSettings::piso(nu);
    set.convection = ConvScheme::Central;
    set.non_orth_correctors = 2;
    let tight = SolveControl {
        tol_abs: 1e-16,
        tol_rel: 1e-13,
        max_iter: 5000,
    };
    set.momentum_solver = tight;
    set.pressure_solver = tight;
    let mut s = PimpleSolver::new(g, u, p, set);
    let mut dpdn = Vec::new();
    for n in 1..=steps {
        let new = sc.mesh_at(omega * n as f64 * dt).unwrap();
        let st = sliding_step(&s.geom, &cur, &new, &s.phi, &s.u.values, dt).unwrap();
        let old = s.geom.clone();
        dpdn = rotating_wall_conditions(&mut s.u, &st, &old, &["cylinder".to_string()], omega, dt).unwrap();
        s.step_sliding(dt, st, dpdn.clone()).unwrap();
        cur = new;
    }
    let g = s.geom.clone();
    let (cc, vol) = (g.mesh.cell_centres(), g.mesh.cell_volumes());
    let r2 = |x: Vec3| x[0] * x[0] + x[1] * x[1];
    let c = (0..cc.len())
        .map(|i| vol[i] * (s.p.values[i] - 0.5 * omega * omega * r2(cc[i])))
        .sum::<f64>()
        / vol.iter().sum::<f64>();
    let wall = vec!["cylinder".to_string()];
    let ld = patch_face_forces(&g, &s.u, &s.p, 1.0, 0.0, &wall, None).unwrap();
    let lc = patch_face_forces(&g, &s.u, &s.p, 1.0, 0.0, &wall, Some(&dpdn)).unwrap();
    let (ni, cf, sf) = (g.n_internal, g.mesh.face_centres(), g.mesh.face_area_vectors());
    let (mut ed, mut ec, mut k) = (0.0, 0.0, 0.0);
    let (mut gm, mut dnm) = (0.0, 0.0);
    for (j, f) in g.mesh.patch("cylinder").unwrap().faces().enumerate() {
        let s2 = vec3::dot(sf[f], sf[f]);
        let exact = 0.5 * omega * omega * r2(cf[f]) + c;
        let gdn = dpdn[f - ni] * g.dn[f - ni];
        ed += (vec3::dot(ld[j], sf[f]) / s2 - exact) / gdn;
        ec += (vec3::dot(lc[j], sf[f]) / s2 - exact) / gdn;
        gm += dpdn[f - ni];
        dnm += g.dn[f - ni];
        k += 1.0;
    }
    let (ed, ec) = (ed / k, ec / k);
    println!(
        "AU2: {k} rotor wall faces, mean g = {:.6}, mean dn = {:.6}; mean e default {ed:.5} ([-0.6, -0.4]), corrected {ec:.5} (|.| <= 0.1)",
        gm / k,
        dnm / k
    );
    assert!((-0.6..=-0.4).contains(&ed), "{ed}");
    assert!(ec.abs() <= 0.1, "{ec}");
}

// ---- L: the listing for the maintainer's decision on the default (ADR 0269 a4) ----

/// `rcfd-api/tests/dynamic_mesh.rs` T4 (copied): the constant-force piston of mass 0.2 through the tree, with `table`.
fn t4_text(table: &str) -> String {
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = 20
ny = 2
lx = 1.0
ly = 0.2
[physics]
nu = 0.01
rho = 1.0
gravity = [5.0, 0.0, 0.0]
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
[solver]
algorithm = "piso"
convection = "central"
dt = 0.005
end_time = 2.0

[dynamic_mesh]
enabled = true
motion = "six_dof"
moving_patches = ["xmin"]
fixed_patches = ["xmax"]
deformation = "smoothing"
[dynamic_mesh.six_dof]
mass = 0.2
inertia = [1.0, 1.0, 1.0]
translation_locked = [false, true, true]
gravity = true
{table}
"#
    )
}

fn t4_run(table: &str) -> (usize, f64, f64, f64, Vec<f64>, Vec<f64>) {
    let c = parse(&t4_text(table));
    let mut s = Simulation::new(&c).unwrap();
    let m = &s.geom.mesh;
    let area: f64 = m
        .patch("xmin")
        .unwrap()
        .faces()
        .map(|f| vec3::norm(m.face_area_vectors()[f]))
        .sum();
    let (mut xs, mut vs) = (Vec::new(), Vec::new());
    let mut steps = 0;
    while s.dynamic_body().unwrap().state.x[0] < 0.5 && steps < 400 {
        s.step();
        assert!(s.failure().is_none(), "{:?}", s.failure());
        steps += 1;
        let b = s.dynamic_body().unwrap();
        xs.push(b.state.x[0]);
        vs.push(b.state.v[0]);
    }
    let (x, v) = (xs[steps - 1], vs[steps - 1]);
    let m0 = 0.2 + area;
    let v2 = -(2.0 * 1.0 / area) * (1.0 - area * x / m0).ln();
    (steps, x, v, v * v / v2, xs, vs)
}

/// The loose piston at mass ratio 4 (ADR 0266 L): the step of `10 X` and the step at which the run stops.
fn loose_unstable(table: &str) -> (Option<usize>, Option<usize>) {
    let mut s = Simulation::new(&parse(&piston(4.0, 0.01, table))).unwrap();
    let mut big = None;
    for n in 1..=1150 {
        s.step();
        if s.failure().is_some() || s.stop_reason().is_some() {
            return (big, Some(n));
        }
        if big.is_none() && s.dynamic_body().unwrap().state.x[0].abs() >= 10.0 * X {
            big = Some(n);
        }
    }
    (big, None)
}

/// ADR 0269 L: each six-DOF case of the case driver both ways (the key absent, the key `prescribed_gradient`); printed.
#[test]
#[ignore = "record of ADR 0269 L (the listing for the maintainer's decision on the default): run once in release"]
fn l_listing_of_the_cases_that_move_under_the_other_default() {
    // T4 of dynamic_mesh.rs
    let (sa, xa, va, ra, xsa, vsa) = t4_run("");
    let (sb, xb, vb, rb, xsb, vsb) = t4_run(PRESCRIBED);
    let common = sa.min(sb);
    println!(
        "L T4: default {sa} steps, x = {xa:.6}, v = {va:.6}, v^2/v_exact^2 = {ra:.6}; key {sb} steps, x = {xb:.6}, v = {vb:.6}, ratio {rb:.6}; at step {common}: x {:+.3e}, v {:+.3e} relative",
        xsb[common - 1] / xsa[common - 1] - 1.0,
        vsb[common - 1] / vsa[common - 1] - 1.0
    );
    // ADR 0266 S1 (strong, mass ratio 4, T/100, 1150 steps)
    for (name, extra) in [("default", ""), ("key", "wall_pressure = \"prescribed_gradient\"\n")] {
        let r = tree_run(&parse(&piston(4.0, 0.01, &strong_table(4.0, extra))), 1150);
        let (w, n) = omega_ratio(0.01, &r.x);
        let mut xs = vec![X];
        xs.extend_from_slice(&r.x);
        let mean = r.iterations.iter().sum::<usize>() as f64 / r.iterations.len() as f64;
        println!(
            "L S1 ({name}): omega_m/omega = {w:.6} ({n} crossings), largest turning ratio {:.5}, mean iterations {mean:.3}",
            worst_growth(&xs)
        );
    }
    // ADR 0266 L (loose, mass ratio 4, T/100)
    for (name, table) in [("default", ""), ("key", PRESCRIBED)] {
        let (big, stop) = loose_unstable(table);
        println!("L loose mass ratio 4 ({name}): 10 X at step {big:?}, stopped at step {stop:?}");
    }
    // ADR 0266 Z and C2 (mass ratio 0.2, 100 steps)
    let a = tree_run(&parse(&piston(0.2, 0.01, "")), 100);
    let b = tree_run(&parse(&piston(0.2, 0.01, PRESCRIBED)), 100);
    let pmax = |r: &TreeRun| {
        r.bits[99]
            .p
            .iter()
            .map(|x| f64::from_bits(*x).abs())
            .fold(0.0, f64::max)
    };
    println!(
        "L Z (mass ratio 0.2, 100 steps): x default {:.9e}, key {:.9e} (difference {:.3e} X); max |p| {:.6e}, {:.6e}",
        a.x[99],
        b.x[99],
        (b.x[99] - a.x[99]) / X,
        pmax(&a),
        pmax(&b)
    );
    // SP of ADR 0211 (A1, A2)
    let (w1, _, _) = sp_run(PRESCRIBED);
    let (w2, _, _) = sp_run("");
    println!("L SP (mass ratio 0.2, 650 steps): omega_m/omega default {w2:.6}, key {w1:.6}");
}
