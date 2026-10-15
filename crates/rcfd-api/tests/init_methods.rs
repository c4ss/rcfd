//! Initialisation methods through the settings tree (F-205, ADR 0019 checks I1, I3, I4, I5, I6). The limits are those of
//! the ADR, written before this file was run.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use serde_json::{json, Value};

const CYLINDER: &str = r#"
[mesh]
generator = "cylinder_ogrid"
nx = 128
ny = 80
r_in = 1.0
r_out = 10.0
depth = 1.0
grading = 10.0
[physics]
nu = 0.01
rho = 1.0
[initial]
method = "potential"
[boundaries.cylinder]
velocity = "fixed"
[boundaries.farfield]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[solver]
algorithm = "simplec"
"#;

fn case(base: &str, edits: &[(&str, Value)]) -> Case {
    let mut c = Case::parse_toml(base).unwrap_or_else(|e| panic!("{e:?}"));
    for (p, v) in edits {
        c.set(p, v.clone()).unwrap_or_else(|e| panic!("{p}: {e}"));
    }
    c
}

fn diags(c: &Case) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = c.semantic_errors().into_iter().map(|d| (d.path, d.message)).collect();
    if out.is_empty() {
        if let Err(e) = Simulation::new(c) {
            out.extend(e.into_iter().map(|d| (d.path, d.message)));
        }
    }
    out
}

/// Errors of the potential-flow velocity against the closed form for a cylinder of radius a in a circle of radius R.
struct CylinderErrors {
    first_ring_max: f64,
    surface_max: f64,
    rms: f64,
    peak: f64,
}

fn cylinder_errors(nx: usize, ny: usize) -> CylinderErrors {
    let c = case(CYLINDER, &[("mesh.nx", json!(nx)), ("mesh.ny", json!(ny))]);
    let s = Simulation::new(&c).unwrap_or_else(|e| panic!("{e:?}"));
    let (a, big_r, uinf) = (1.0_f64, 10.0_f64, 1.0_f64);
    let amp = uinf * big_r * big_r / (big_r * big_r - a * a);
    let (ux, uy) = (s.field("ux").unwrap(), s.field("uy").unwrap());
    let cc = s.geom.mesh.cell_centres();
    let vol = s.geom.mesh.cell_volumes();
    let comps = |i: usize| {
        let (x, y) = (cc[i][0], cc[i][1]);
        let (r, th) = ((x * x + y * y).sqrt(), y.atan2(x));
        let ur = ux[i] * th.cos() + uy[i] * th.sin();
        let ut = -ux[i] * th.sin() + uy[i] * th.cos();
        (r, th, ur, ut)
    };
    let exact = |r: f64, th: f64| {
        (
            amp * (1.0 - a * a / (r * r)) * th.cos(),
            -amp * (1.0 + a * a / (r * r)) * th.sin(),
        )
    };
    let peak = 2.0 * amp;
    // cell id = it + nx * ir
    let mut first_ring_max: f64 = 0.0;
    let mut surface_max: f64 = 0.0;
    for it in 0..nx {
        let (r1, th, _, ut1) = comps(it);
        let (r2, _, _, ut2) = comps(it + nx);
        first_ring_max = first_ring_max.max((ut1 - exact(r1, th).1).abs() / peak);
        let surf = ut1 - (ut2 - ut1) / (r2 - r1) * (r1 - a);
        surface_max = surface_max.max((surf - (-2.0 * amp * th.sin())).abs() / peak);
    }
    let (mut num, mut den) = (0.0, 0.0);
    for i in 0..cc.len() {
        let (r, th, ur, ut) = comps(i);
        let (er, et) = exact(r, th);
        num += vol[i] * ((ur - er).powi(2) + (ut - et).powi(2));
        den += vol[i];
    }
    CylinderErrors {
        first_ring_max,
        surface_max,
        rms: (num / den).sqrt() / peak,
        peak,
    }
}

#[test]
fn i1_potential_flow_around_a_cylinder_matches_the_closed_form() {
    let m1 = cylinder_errors(64, 40);
    let m2 = cylinder_errors(128, 80);
    let order_rms = (m1.rms / m2.rms).log2();
    let order_ring = (m1.first_ring_max / m2.first_ring_max).log2();
    println!(
        "I1 peak {:.5}: M1 ring {:.4e} surface {:.4e} rms {:.4e}; M2 ring {:.4e} surface {:.4e} rms {:.4e}; orders rms {:.3} ring {:.3}",
        m2.peak, m1.first_ring_max, m1.surface_max, m1.rms, m2.first_ring_max, m2.surface_max, m2.rms, order_rms, order_ring
    );
    assert!(m2.first_ring_max <= 0.03, "I1a first ring {}", m2.first_ring_max);
    assert!(m2.surface_max <= 0.03, "I1b extrapolated surface {}", m2.surface_max);
    assert!(order_rms >= 1.4, "I1c rms order {order_rms}");
    assert!(order_ring >= 0.9, "I1d ring order {order_ring}");
}

const CAVITY: &str = r#"
[mesh]
generator = "cavity_2d"
nx = 40
ny = 40
[physics]
nu = 0.01
[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
max_iterations = 3000
tolerance_u = 1e-6
tolerance_p = 1e-6
"#;

fn iterations_to_converge(c: &Case) -> (usize, bool) {
    let mut s = Simulation::new(c).unwrap_or_else(|e| panic!("{e:?}"));
    while !s.is_done() {
        s.step();
    }
    (s.iteration(), s.is_converged())
}

#[test]
fn i3_coarse_presolve_reduces_the_fine_iterations() {
    let (uni, uni_ok) = iterations_to_converge(&case(CAVITY, &[]));
    let c = case(
        CAVITY,
        &[
            ("initial.method", json!("coarse")),
            ("initial.coarse_factor", json!(2)),
            ("initial.coarse_iterations", json!(300)),
        ],
    );
    let (coarse, coarse_ok) = iterations_to_converge(&c);
    println!(
        "I3 fine iterations: uniform {uni} ({uni_ok}), coarse-initialised {coarse} ({coarse_ok}); equivalent work with a coarse iteration at 1/4: {:.0} against {uni}",
        coarse as f64 + 300.0 / 4.0
    );
    assert!(uni_ok && coarse_ok);
    assert!(coarse < uni, "coarse {coarse} vs uniform {uni}");
}

const CHANNEL: &str = r#"
[mesh]
generator = "box_2d"
nx = 4
ny = 14
lx = 1.0
ly = 2.0
periodic_x = true
[physics]
nu = 0.002531645569620253
body_force = [1.0, 0.0, 0.0]
[initial]
velocity = [17.0, 0.0, 0.0]
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "upwind"
alpha_u = 0.9
max_iterations = 100
[turbulence]
model = "k_epsilon"
k_initial = 0.01
epsilon_initial = 0.05
[registers.core]
type = "box"
box_min = [0.0, 0.5, -1.0]
box_max = [1.0, 1.0, 1.0]
[initial.patches.a]
register = "core"
variable = "k"
value = [0.5]
[initial.patches.b]
register = "core"
variable = "epsilon"
value = [2.0]
"#;

#[test]
#[ignore = "record: set-up defect, the model name for no turbulence model is \"laminar\", not \"none\"; replaced by i4b"]
fn i4_localised_turbulence_initialisation() {
    let c = case(CHANNEL, &[]);
    let s = Simulation::new(&c).unwrap_or_else(|e| panic!("{e:?}"));
    let (k, eps) = (s.field("k").unwrap(), s.field("epsilon").unwrap());
    let cc = s.geom.mesh.cell_centres();
    let (mut n_in, mut worst): (usize, f64) = (0, 0.0);
    for i in 0..cc.len() {
        let inside = cc[i][1] >= 0.5 && cc[i][1] <= 1.0;
        let (ek, ee) = if inside { (0.5, 2.0) } else { (0.01, 0.05) };
        n_in += usize::from(inside);
        worst = worst.max((k[i] - ek).abs()).max((eps[i] - ee).abs());
    }
    println!(
        "I4 cells in the register {n_in} of {}, largest difference {worst:e}",
        cc.len()
    );
    assert!(n_in > 0 && n_in < cc.len());
    assert_eq!(worst, 0.0);
    // diagnostics
    let bad = case(
        CHANNEL,
        &[
            ("turbulence.model", json!("k_omega_sst")),
            ("turbulence.k_initial", json!(0.0)),
            ("turbulence.epsilon_initial", json!(0.0)),
        ],
    );
    let d = diags(&bad);
    assert!(
        d.iter()
            .any(|(p, m)| p == "initial.patches.b.variable" && m.contains("no field 'epsilon'")),
        "{d:?}"
    );
    let none = case(
        CHANNEL,
        &[
            ("turbulence.model", json!("none")),
            ("turbulence.k_initial", json!(0.0)),
            ("turbulence.epsilon_initial", json!(0.0)),
        ],
    );
    let d = diags(&none);
    assert!(
        d.iter()
            .any(|(p, m)| p == "initial.patches.a.variable" && m.contains("needs a turbulence model")),
        "{d:?}"
    );
}

#[test]
fn i4b_localised_turbulence_initialisation() {
    let c = case(CHANNEL, &[]);
    let s = Simulation::new(&c).unwrap_or_else(|e| panic!("{e:?}"));
    let (k, eps) = (s.field("k").unwrap(), s.field("epsilon").unwrap());
    let cc = s.geom.mesh.cell_centres();
    let (mut n_in, mut worst): (usize, f64) = (0, 0.0);
    for i in 0..cc.len() {
        let inside = cc[i][1] >= 0.5 && cc[i][1] <= 1.0;
        let (ek, ee) = if inside { (0.5, 2.0) } else { (0.01, 0.05) };
        n_in += usize::from(inside);
        worst = worst.max((k[i] - ek).abs()).max((eps[i] - ee).abs());
    }
    println!(
        "I4 cells in the register {n_in} of {}, largest difference {worst:e}",
        cc.len()
    );
    assert!(n_in > 0 && n_in < cc.len());
    assert_eq!(worst, 0.0);
    // diagnostics
    let bad = case(
        CHANNEL,
        &[
            ("turbulence.model", json!("k_omega_sst")),
            ("turbulence.k_initial", json!(0.0)),
            ("turbulence.epsilon_initial", json!(0.0)),
        ],
    );
    let d = diags(&bad);
    assert!(
        d.iter()
            .any(|(p, m)| p == "initial.patches.b.variable" && m.contains("no field 'epsilon'")),
        "{d:?}"
    );
    let none = case(
        CHANNEL,
        &[
            ("turbulence.model", json!("laminar")),
            ("turbulence.k_initial", json!(0.0)),
            ("turbulence.epsilon_initial", json!(0.0)),
        ],
    );
    let d = diags(&none);
    assert!(
        d.iter()
            .any(|(p, m)| p == "initial.patches.a.variable" && m.contains("needs a turbulence model")),
        "{d:?}"
    );
}

#[test]
fn i5_uniform_method_is_the_default() {
    let run = |extra: &[(&str, Value)]| {
        let mut s = Simulation::new(&case(CAVITY, extra)).unwrap();
        for _ in 0..20 {
            s.step();
        }
        [s.field("ux").unwrap(), s.field("uy").unwrap(), s.field("p").unwrap()]
    };
    let a = run(&[]);
    let b = run(&[("initial.method", json!("uniform"))]);
    assert_eq!(a, b);
}

#[test]
fn i6_initialisation_diagnostics_are_located() {
    // potential with a transient algorithm
    let d = diags(&case(
        CYLINDER,
        &[
            ("solver.algorithm", json!("piso")),
            ("solver.dt", json!(0.01)),
            ("solver.end_time", json!(0.1)),
        ],
    ));
    assert!(d.iter().any(|(p, _)| p == "initial.method"), "{d:?}");
    // zero-gradient velocity without a fixed pressure
    let d = diags(&case(
        CYLINDER,
        &[("boundaries.farfield.velocity", json!("zero_gradient"))],
    ));
    assert!(d.iter().any(|(p, _)| p == "boundaries.farfield.velocity"), "{d:?}");
    // a closed domain with a net inflow
    let d = diags(&case(
        CAVITY,
        &[
            ("initial.method", json!("potential")),
            ("boundaries.lid.velocity_value", json!([0.0, -1.0, 0.0])),
        ],
    ));
    assert!(
        d.iter()
            .any(|(p, m)| p == "initial.method" && m.contains("do not balance")),
        "{d:?}"
    );
    // the coarse factor does not divide the cell counts
    let d = diags(&case(
        CAVITY,
        &[("initial.method", json!("coarse")), ("initial.coarse_factor", json!(3))],
    ));
    assert!(d.iter().any(|(p, _)| p == "initial.coarse_factor"), "{d:?}");
}
