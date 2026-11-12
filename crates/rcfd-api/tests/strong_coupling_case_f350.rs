//! F-350 (ADR 0266): the strong body-flow coupling of `rcfd_fv::fsi::strong_step` selected from the case tree by the
//! optional table `[dynamic_mesh.six_dof.coupling]`, on the spring-mounted piston of ADR 0250 (SPm, the mesh translated
//! by `X` so that the case's mesh is the mesh at the body's initial position).
//!
//! The limits, set-ups and mutations were registered in ADR 0266 before this file existed. The crossings and the
//! turning points are those of `rcfd-fv/tests/strong_fsi_f251.rs` (copied, that file is not edited).

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::fsi::{relaxation_from_added_mass, strong_step, BodyCoupling, Relaxation, StrongCoupling};
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
const COUPLING: &str = "dynamic_mesh.six_dof.coupling";

fn mass(mu: f64) -> f64 {
    M_A / mu
}

/// Spring of period 1 at the mass ratio `mu`.
fn stiffness(mu: f64) -> f64 {
    TAU * TAU * (mass(mu) + M_A)
}

/// The SPm case (ADR 0266 arithmetic): `box_3d` of `lx = L0 - X` with the body at `X`, so the column is `L0 - x`.
fn piston_with(mu: f64, dt: f64, dm_extra: &str, six_extra: &str, table: &str) -> String {
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
{dm_extra}
[dynamic_mesh.six_dof]
mass = {m:?}
inertia = [1.0, 1.0, 1.0]
position = [{X:?}, 0.0, 0.0]
translation_locked = [false, true, true]
stiffness = [{k:?}, 0.0, 0.0]
{six_extra}
{table}
"#
    )
}

fn piston(mu: f64, dt: f64, table: &str) -> String {
    piston_with(mu, dt, "", "", table)
}

/// The registered strong settings of ADR 0250 as keys: Aitken from `2M/(2M + rho A L0)`, `1e-8`, `k X`, cap 20.
fn registered_table(mu: f64) -> String {
    format!(
        "[dynamic_mesh.six_dof.coupling]\nscheme = \"strong\"\nrelaxation = \"aitken\"\nadded_mass = {M_A:?}\ntolerance = 1e-8\nforce_scale = {:?}\nmax_iterations = 20\n",
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

/// E2's keys away from their defaults (ADR 0266 arithmetic).
fn e2_table(mu: f64) -> String {
    format!(
        "[dynamic_mesh.six_dof.coupling]\nscheme = \"strong\"\nrelaxation = \"fixed\"\nadded_mass = 0.032\ntolerance = 1e-10\nforce_scale = {:?}\nmoment_scale = 1e-3\nmax_iterations = 30\n",
        0.5 * stiffness(mu) * X
    )
}

fn e2(mu: f64) -> StrongCoupling {
    StrongCoupling {
        relaxation: Relaxation::Fixed(relaxation_from_added_mass(mass(mu), 0.032)),
        tolerance: 1e-10,
        force_scale: 0.5 * stiffness(mu) * X,
        moment_scale: 1e-3,
        max_iterations: 30,
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

fn bits_of(flow: &rcfd_fv::pimple::PimpleSolver, body: &Body) -> Bits {
    Bits {
        u: flow.u.values.iter().flatten().map(|a| a.to_bits()).collect(),
        p: flow.p.values.iter().map(|a| a.to_bits()).collect(),
        phi: flow.phi.iter().map(|a| a.to_bits()).collect(),
        x: body.state.x.map(f64::to_bits),
        v: body.state.v.map(f64::to_bits),
    }
}

fn tree_bits(s: &Simulation) -> Bits {
    bits_of(s.pimple_solver().unwrap(), s.dynamic_body().unwrap())
}

/// The tree's steps: the bits after every step and the iteration counts.
fn tree_run(case: &Case, steps: usize) -> (Vec<Bits>, Vec<usize>, bool) {
    let mut s = Simulation::new(case).unwrap();
    let mut out = Vec::new();
    for n in 1..=steps {
        s.step();
        assert!(s.failure().is_none(), "step {n}: {:?}", s.failure());
        out.push(tree_bits(&s));
    }
    let r = s.coupling_records();
    (
        out,
        r.iter().map(|r| r.iterations).collect(),
        r.iter().all(|r| r.converged),
    )
}

/// The library run on the flow solver of a second `Simulation` of the same case: the body, the points, the coupling
/// and the settings built here from the case's numbers (ADR 0266 E1, E2).
fn library_run(case: &Case, mu: f64, settings: &StrongCoupling, steps: usize) -> (Vec<Bits>, Vec<usize>, bool) {
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
    let mut out = Vec::new();
    let mut iters = Vec::new();
    let mut conv = true;
    for n in 1..=steps {
        let st =
            strong_step(flow, &mut body, dt, &cpl, settings, &mut points).unwrap_or_else(|e| panic!("step {n}: {e}"));
        iters.push(st.iterations);
        conv &= st.converged;
        out.push(bits_of(flow, &body));
    }
    (out, iters, conv)
}

fn first_difference(a: &[Bits], b: &[Bits]) -> Option<usize> {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).position(|(x, y)| x != y).map(|i| i + 1)
}

// ---- the measurements of ADR 0250 (copied from strong_fsi_f251.rs) ----

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

fn worst_growth(x: &[f64]) -> (f64, usize, usize) {
    let (mx, mn) = turning_points(x);
    let mut worst: f64 = 0.0;
    for w in mx.windows(2) {
        worst = worst.max(w[1] / w[0]);
    }
    for w in mn.windows(2) {
        worst = worst.max(w[1].abs() / w[0].abs());
    }
    (worst, mx.len(), mn.len())
}

// ---- T0, Z ----

/// ADR 0266 T0: without the table the texts hold no `coupling` and the key reads its default; with it they do.
#[test]
fn t0_a_case_without_the_table_keeps_its_text() {
    let without = parse(&piston(4.0, 0.01, ""));
    let with = parse(&piston(4.0, 0.01, &registered_table(4.0)));
    let (tw, jw) = (without.to_toml(), without.to_json_string());
    println!(
        "T0: settings hash without {:016x}, with {:016x}",
        without.tree.content_hash(),
        with.tree.content_hash()
    );
    assert!(!tw.contains("coupling") && !jw.contains("coupling"));
    assert!(without
        .tree
        .to_json()
        .pointer("/dynamic_mesh/six_dof/coupling")
        .is_none());
    assert_eq!(without.get(&format!("{COUPLING}.scheme")).unwrap(), "loose");
    assert!(with.to_toml().contains("[dynamic_mesh.six_dof.coupling]"));
    assert!(with.to_json_string().contains("\"coupling\""));
    assert_ne!(without.tree.content_hash(), with.tree.content_hash());
}

/// ADR 0266 Z: `scheme = "loose"` with every other key away from its default equals no table, bit for bit.
#[test]
fn z_loose_table_equals_no_table_bit_for_bit() {
    let table = "[dynamic_mesh.six_dof.coupling]\nscheme = \"loose\"\nrelaxation = \"fixed\"\nadded_mass = 0.5\ntolerance = 1e-6\nforce_scale = 1.0\nmoment_scale = 1.0\nmax_iterations = 3\nat_cap = \"continue\"\n";
    let (a, ra, _) = tree_run(&parse(&piston(0.2, 0.01, "")), 100);
    let (b, rb, _) = tree_run(&parse(&piston(0.2, 0.01, table)), 100);
    println!("Z: first differing step {:?} of 100", first_difference(&a, &b));
    assert_eq!(first_difference(&a, &b), None);
    assert!(ra.is_empty() && rb.is_empty());
}

// ---- E1, E2 ----

/// ADR 0266 E1: the tree's strong path with ADR 0250's settings equals `strong_step` called directly, bit for bit.
#[test]
fn e1_tree_equals_library_aitken() {
    let mu = 4.0;
    let case = parse(&piston(mu, 0.01, &registered_table(mu)));
    let (t, ti, tc) = tree_run(&case, 60);
    let (l, li, lc) = library_run(&case, mu, &registered(mu), 60);
    println!(
        "E1: first differing step {:?} of 60; iterations tree {:?}, library {:?}; converged {tc} {lc}",
        first_difference(&t, &l),
        ti,
        li
    );
    assert_eq!(first_difference(&t, &l), None);
    assert_eq!(ti, li);
    assert!(tc && lc);
}

/// ADR 0266 E2: the same with a fixed factor and every other key away from its default.
#[test]
fn e2_tree_equals_library_fixed_and_non_default_keys() {
    let mu = 4.0;
    let case = parse(&piston(mu, 0.01, &e2_table(mu)));
    let (t, ti, tc) = tree_run(&case, 40);
    let (l, li, lc) = library_run(&case, mu, &e2(mu), 40);
    println!(
        "E2: first differing step {:?} of 40; iterations tree {:?}, library {:?}; converged {tc} {lc}",
        first_difference(&t, &l),
        ti,
        li
    );
    assert_eq!(first_difference(&t, &l), None);
    assert_eq!(ti, li);
    assert!(tc && lc);
    assert!(ti.iter().all(|&i| i <= 20), "{ti:?}");
}

// ---- S1 to S4, R1, R2 ----

struct Run {
    t: Vec<f64>,
    x: Vec<f64>,
    iterations: Vec<usize>,
    converged: bool,
}

fn strong_case_run(mu: f64, per_period: usize) -> Run {
    let dt = 1.0 / per_period as f64;
    let case = parse(&piston(mu, dt, &registered_table(mu)));
    let mut s = Simulation::new(&case).unwrap();
    let steps = (11.5 * per_period as f64).round() as usize;
    let mut run = Run {
        t: vec![0.0],
        x: vec![X],
        iterations: Vec::new(),
        converged: true,
    };
    for n in 1..=steps {
        s.step();
        assert!(s.failure().is_none(), "step {n}: {:?}", s.failure());
        run.t.push(n as f64 * dt);
        run.x.push(s.dynamic_body().unwrap().state.x[0]);
    }
    for r in s.coupling_records() {
        run.iterations.push(r.iterations);
        run.converged &= r.converged;
    }
    assert_eq!(run.iterations.len(), steps);
    run
}

/// ADR 0266 S1 to S4, R1, R2 (a2): the piston at a mass ratio of 4 through the case driver, `T/100` and `T/200`.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn s1_strong_piston_at_mass_ratio_4_through_the_case_driver() {
    let mu = 4.0;
    let r1 = strong_case_run(mu, 100);
    let r2 = strong_case_run(mu, 200);
    let (w1, n1) = measured_omega(&r1.t, &r1.x);
    let (w2, n2) = measured_omega(&r2.t, &r2.x);
    let (g1, mx1, mn1) = worst_growth(&r1.x);
    let (g2, mx2, mn2) = worst_growth(&r2.x);
    let omega = TAU;
    let (a1, a2) = (w1 / omega, w2 / omega);
    let halving = (w2 - w1).abs() / w1;
    let stats = |r: &Run| {
        let max = r.iterations.iter().copied().max().unwrap();
        let mean = r.iterations.iter().sum::<usize>() as f64 / r.iterations.len() as f64;
        let mut h = vec![0; max + 1];
        for &i in &r.iterations {
            h[i] += 1;
        }
        (max, mean, h)
    };
    let (m1, e1, h1) = stats(&r1);
    let (m2, e2, h2) = stats(&r2);
    println!(
        "S1: omega_m/omega T/100 {a1:.6} ({n1} crossings), T/200 {a2:.6} ({n2}); S2 halving {halving:.3e}; S3 worst turning ratio \
         {g1:.5} ({mx1} maxima, {mn1} minima), {g2:.5} ({mx2}, {mn2}); S4 iterations largest {m1}, {m2}, mean {e1:.3}, {e2:.3}, \
         histograms {h1:?} {h2:?}, all converged {} {}; R1 against 1.005081, 1.005063: {:.2e}, {:.2e}; R2 against 0.92452, \
         0.96141: {:.2e}, {:.2e}",
        r1.converged,
        r2.converged,
        (a1 - 1.005081).abs(),
        (a2 - 1.005063).abs(),
        (g1 - 0.92452).abs(),
        (g2 - 0.96141).abs()
    );
    let (mxs, mns) = turning_points(&r1.x);
    println!("S3 T/100 maxima {mxs:?}\nminima {mns:?}");
    assert!((a1 - 1.0).abs() <= 0.02, "S1 {a1}");
    assert!(halving <= 0.005, "S2 {halving}");
    assert!(g1 <= 1.0 + 1e-3 && g2 <= 1.0 + 1e-3, "S3 {g1} {g2}");
    assert!(n1 >= 11 && n2 >= 11, "S3 crossings {n1} {n2}");
    assert!(r1.converged && r2.converged, "S4 converged");
    assert!(m1 <= 8 && m2 <= 8 && e1 <= 5.0 && e2 <= 5.0, "S4 {m1} {m2} {e1} {e2}");
    assert!(
        (a1 - 1.005081).abs() <= 1e-5 && (a2 - 1.005063).abs() <= 1e-5,
        "R1 {a1} {a2}"
    );
    assert!(
        (g1 - 0.92452).abs() <= 1e-4 && (g2 - 0.96141).abs() <= 1e-4,
        "R2 {g1} {g2}"
    );
}

// ---- L ----

/// The loose run: the first step with `abs(x) >= 10 X`, and how and when the run stopped.
fn loose_case_run(text: &str, per_period: usize) -> (Option<usize>, Option<(usize, String, bool)>) {
    let case = parse(text);
    let mut s = Simulation::new(&case).unwrap();
    let mut big = None;
    for n in 1..=(11.5 * per_period as f64).round() as usize {
        s.step();
        if let Some(f) = s.failure() {
            return (big, Some((n, f.to_string(), s.diverged())));
        }
        if s.diverged() {
            return (big, Some((n, "divergence guard".into(), true)));
        }
        let x = s.dynamic_body().unwrap().state.x[0];
        if big.is_none() && (x.abs() >= 10.0 * X || !x.is_finite()) {
            big = Some(n);
        }
        if big.is_some_and(|b| n > b + 200) {
            break;
        }
    }
    (big, None)
}

/// ADR 0266 L (a3): the same case with the loose coupling grows past `10 X` or stops, located, within 60 steps.
#[test]
fn l_loose_coupling_grows_or_stops_located() {
    let mu = 4.0;
    let loose = "[dynamic_mesh.six_dof.coupling]\nscheme = \"loose\"\n";
    for (name, text, pp) in [
        ("table, T/100", piston(mu, 0.01, loose), 100),
        ("table, T/200", piston(mu, 0.005, loose), 200),
        ("no table, T/100", piston(mu, 0.01, ""), 100),
    ] {
        let (big, stop) = loose_case_run(&text, pp);
        println!("L {name}: first step with |x| >= 10 X {big:?}; stopped {stop:?}");
        let first = match (big, &stop) {
            (Some(b), Some((n, ..))) => b.min(*n),
            (Some(b), None) => b,
            (None, Some((n, ..))) => *n,
            (None, None) => panic!("L {name}: neither grew nor stopped"),
        };
        assert!(first <= 60, "L {name}: {first}");
        if let Some((_, msg, diverged)) = &stop {
            assert!(msg.starts_with("dynamic_mesh: step") || *diverged, "L {name}: {msg}");
        }
    }
}

// ---- C1, C2 ----

/// ADR 0266 C1 (a4): a step at the cap stops the run with a located message.
#[test]
fn c1_a_step_at_the_cap_stops_the_run_located() {
    let table = "[dynamic_mesh.six_dof.coupling]\nscheme = \"strong\"\nmax_iterations = 1\n";
    let mut s = Simulation::new(&parse(&piston(4.0, 0.01, table))).unwrap();
    s.step();
    let f = s.failure().expect("the run must stop").to_string();
    println!("C1: {f}");
    assert!(f.starts_with("dynamic_mesh: step 1,"), "{f}");
    assert!(f.contains(&format!("{COUPLING}.max_iterations")), "{f}");
    let r = s.coupling_records();
    assert_eq!(r.len(), 1);
    assert!(!r[0].converged && r[0].iterations == 1);
}

/// ADR 0266 C2 (a4): with `at_cap = "continue"` the run goes on; with the cap 1 it is the loose step, bit for bit.
#[test]
fn c2_continue_at_the_cap_of_one_is_the_loose_step() {
    let table = "[dynamic_mesh.six_dof.coupling]\nscheme = \"strong\"\nmax_iterations = 1\nat_cap = \"continue\"\n";
    let (a, ..) = tree_run(&parse(&piston(0.2, 0.01, "")), 100);
    let mut s = Simulation::new(&parse(&piston(0.2, 0.01, table))).unwrap();
    let mut b = Vec::new();
    let mut warnings = Vec::new();
    for n in 1..=100 {
        s.step();
        assert!(s.failure().is_none(), "step {n}: {:?}", s.failure());
        b.push(tree_bits(&s));
        warnings.extend(s.take_runtime_warnings());
    }
    let r = s.coupling_records();
    let caps: Vec<_> = warnings.iter().filter(|w| w["kind"] == "coupling_cap").collect();
    println!(
        "C2: first differing step {:?} of 100; {} records, {} converged; warnings {warnings:?}",
        first_difference(&a, &b),
        r.len(),
        r.iter().filter(|r| r.converged).count()
    );
    assert_eq!(first_difference(&a, &b), None);
    assert_eq!(r.len(), 100);
    assert!(r.iter().all(|r| !r.converged));
    assert_eq!(caps.len(), 1);
}

// ---- N ----

/// ADR 0266 N (a5): every static check of the new keys is located at its key.
#[test]
fn n_static_checks_are_located() {
    let mu = 4.0;
    let strong = |extra: &str| format!("{}{extra}\n", registered_table(mu));
    let valid = piston(mu, 0.01, &strong(""));
    assert!(paths(&valid).is_empty(), "{:?}", paths(&valid));
    let key = |k: &str| format!("{COUPLING}.{k}");
    let cases: Vec<(String, String)> = vec![
        (
            valid.replace("motion = \"six_dof\"", "motion = \"prescribed\""),
            key("scheme"),
        ),
        (
            piston_with(mu, 0.01, "rest_patches = [\"ymin\"]", "", &strong("")),
            key("scheme"),
        ),
        (
            piston_with(mu, 0.01, "", "integrator = \"newmark\"", &strong("")),
            key("scheme"),
        ),
        (valid.replace("tolerance = 1e-8", "tolerance = 0.0"), key("tolerance")),
        (valid.replace("tolerance = 1e-8", "tolerance = -1.0"), key("tolerance")),
        (piston(mu, 0.01, &strong("moment_scale = -1.0")), key("moment_scale")),
        (piston(mu, 0.01, &strong("at_cap = \"warn\"")), key("at_cap")),
        (
            valid.replace("added_mass = 0.04", "added_mass = -1.0"),
            key("added_mass"),
        ),
        (
            valid.replace("max_iterations = 20", "max_iterations = 0"),
            key("max_iterations"),
        ),
        (
            valid.replace("scheme = \"strong\"", "scheme = \"implicit\""),
            key("scheme"),
        ),
        (
            valid.replace("relaxation = \"aitken\"", "relaxation = \"secant\""),
            key("relaxation"),
        ),
        (piston(mu, 0.01, &strong("factor = 0.5")), COUPLING.to_string()),
    ];
    let fs = format!("force_scale = {:?}", stiffness(mu) * X);
    let mut cases = cases;
    cases.push((valid.replace(&fs, "force_scale = -1.0"), key("force_scale")));
    for (text, path) in &cases {
        assert_ne!(text, &valid, "the variant for {path} did not change the text");
        let p = paths(text);
        println!("N {path}: {p:?}");
        assert!(p.iter().any(|x| x == path), "{path}: got {p:?}");
    }
    // the section disabled: the table is inert
    let off = valid
        .replace("enabled = true", "enabled = false")
        .replace("tolerance = 1e-8", "tolerance = 0.0");
    assert!(paths(&off).is_empty(), "{:?}", paths(&off));
}
