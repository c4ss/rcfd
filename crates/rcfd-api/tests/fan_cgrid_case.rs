//! C-grid axial-fan passage through the case tree (`mesh.fan.topology = "c_grid"`): checks Q6(d), R1, S1' and S3' of
//! docs/adr/0068-wave33a-mesh-quality-foundation.md. The criteria were registered before this code ran; a miss becomes an
//! ignored `*_known_miss` test. The solves are smoke tests of the mesh and the boundary-condition plumbing, NOT a fan
//! performance validation.

use rcfd_api::case::Case;
use rcfd_api::rules::{check_case, has_errors, mesh_context, Severity};
use rcfd_api::sim::{build_mesh, Simulation};
use rcfd_fv::FvGeometry;
use rcfd_mesh::quality::check_mesh;
use std::sync::Arc;

const U: f64 = 1.0;

/// Mesh section of the registered small passage S on the C-grid topology.
fn mesh_section(include_blade: bool) -> String {
    format!(
        "[mesh]\ngenerator = \"axial_fan_sector\"\n[mesh.fan]\ntopology = \"c_grid\"\nn_up = 8\nn_chord = 16\nn_down = 8\nn_side = 10\nn_nose = 4\nn_core = 1\nn_span = 14\nn_gap = 7\nn_bl = 2\ngrowth = 1.2\nfirst_cell_height = 1e-3\ninclude_blade = {include_blade}\n"
    )
}

fn uniform_flow_case(include_blade: bool) -> String {
    format!(
        r#"{}
[physics]
nu = 1e-3
rho = 1.0
[initial]
velocity = [0.0, 0.0, {U:?}]
[boundaries.inlet]
velocity = "fixed"
velocity_value = [0.0, 0.0, {U:?}]
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.hub]
velocity = "fixed"
velocity_value = [0.0, 0.0, {U:?}]
[boundaries.shroud]
velocity = "fixed"
velocity_value = [0.0, 0.0, {U:?}]
{}
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 1500
tolerance_u = 1e-11
tolerance_p = 1e-11
"#,
        mesh_section(include_blade),
        if include_blade {
            "[boundaries.blade]\nvelocity = \"fixed\"\n"
        } else {
            ""
        }
    )
}

fn rotating_case(omega: f64) -> String {
    format!(
        r#"{}
[physics]
nu = 0.02
rho = 1.0
[boundaries.inlet]
velocity = "fixed"
velocity_value = [0.0, 0.0, {U:?}]
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.hub]
velocity = "fixed"
[boundaries.blade]
velocity = "fixed"
[boundaries.shroud]
velocity = "fixed"
velocity_expression_x = "{omega:?} [1/s] * y"
velocity_expression_y = "-{omega:?} [1/s] * x"
[sources.frame]
type = "rotating_frame"
rotation_axis = [0.0, 0.0, 1.0]
angular_velocity = {omega:?}
centrifugal = true
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 6000
tolerance_u = 1e-11
tolerance_p = 1e-11
[reports.m_in]
type = "mass_flow"
patch = "inlet"
[reports.m_out]
type = "mass_flow"
patch = "outlet"
[reports.m_shroud]
type = "mass_flow"
patch = "shroud"
[reports.m_hub]
type = "mass_flow"
patch = "hub"
[reports.m_blade]
type = "mass_flow"
patch = "blade"
[reports.m_lo]
type = "mass_flow"
patch = "seam_lo"
[reports.m_hi]
type = "mass_flow"
patch = "seam_hi"
"#,
        mesh_section(true)
    )
}

fn run(text: &str, iterations: usize) -> Simulation {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}\n{text}"));
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    let n = sim.run(iterations);
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    println!(
        "  {n} iterations, converged {}, last {:?}",
        sim.is_converged(),
        sim.last()
    );
    sim
}

#[test]
fn q6d_fv_geometry_accepts_the_c_grid_mesh() {
    let case = Case::parse_toml(&uniform_flow_case(true)).unwrap();
    let mesh = build_mesh(&case).unwrap();
    for name in ["inlet", "outlet", "hub", "shroud", "blade", "seam_lo", "seam_hi"] {
        assert!(mesh.patch(name).is_some(), "patch {name} missing");
    }
    let geom = FvGeometry::new(Arc::new(mesh)).expect("FvGeometry::new accepts the C-grid mesh");
    println!(
        "Q6d {} cells, {} faces accepted",
        geom.mesh.n_cells(),
        geom.mesh.n_faces()
    );
}

#[test]
fn r1_rules_raise_no_error_on_the_example_case() {
    let text = include_str!("../../../cases/axial_fan_cgrid/axial_fan_cgrid.toml");
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    let mesh = build_mesh(&case).unwrap_or_else(|e| panic!("{e:?}"));
    let q = check_mesh(&mesh);
    let rules = check_case(&case, Some(&mesh_context(&q)));
    println!(
        "R1 {} cells; mesh check ok {}; non-orthogonality max {:.2} deg; {} rule findings",
        q.n_cells,
        q.is_ok(),
        q.max_non_orthogonality,
        rules.len()
    );
    for r in &rules {
        println!("R1 finding {} {:?} {}: {}", r.id, r.severity, r.path, r.message);
    }
    assert!(!has_errors(&rules), "error-level finding");
    assert!(rules.iter().all(|r| r.severity != Severity::Error));
    assert!(q.is_ok(), "check_mesh rejects the example mesh");
}

#[test]
fn example_case_builds_and_runs_a_few_iterations() {
    let text = include_str!("../../../cases/axial_fan_cgrid/axial_fan_cgrid.toml");
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    println!("example case: {} cells", sim.geom.mesh.n_cells());
    sim.run(5);
    assert!(sim.failure().is_none());
}

#[test]
fn s1_prime_empty_passage_reproduces_uniform_flow() {
    // criterion (ADR 0068 S1'): Omega = 0, no blade, all walls moving at the inflow, outlet pressure: |u - (0,0,U)| <= 1e-8 U
    let sim = run(&uniform_flow_case(false), 400);
    let (ux, uy, uz) = (
        sim.field("ux").unwrap(),
        sim.field("uy").unwrap(),
        sim.field("uz").unwrap(),
    );
    let mut worst: f64 = 0.0;
    for c in 0..ux.len() {
        worst = worst.max(ux[c].abs()).max(uy[c].abs()).max((uz[c] - U).abs());
    }
    println!(
        "S1' worst |u - U| / U = {:.3e} (limit 1e-8), converged {}",
        worst / U,
        sim.is_converged()
    );
    assert!(worst <= 1e-8 * U);
}

fn s3_balances(omega: f64) -> (f64, f64, f64, f64, bool) {
    let sim = run(&rotating_case(omega), 6000);
    let m = |k: &str| sim.monitors.last(k).unwrap();
    let (m_in, m_out, m_lo, m_hi) = (m("m_in"), m("m_out"), m("m_lo"), m("m_hi"));
    println!(
        "S3' Omega {omega}: m_in {m_in:.6e} m_out {m_out:.6e} m_shroud {:.3e} m_hub {:.1e} m_blade {:.1e} m_lo {m_lo:.6e} m_hi {m_hi:.6e}",
        m("m_shroud"),
        m("m_hub"),
        m("m_blade")
    );
    println!(
        "S3' |m_in + m_out| / |m_in| = {:.3e}; seam pair |m_lo + m_hi| / |m_lo| = {:.3e}; all patches {:.3e}; converged {}",
        (m_in + m_out).abs() / m_in.abs(),
        (m_lo + m_hi).abs() / m_lo.abs(),
        (m_in + m_out + m_lo + m_hi + m("m_shroud") + m("m_hub") + m("m_blade")).abs() / m_in.abs(),
        sim.is_converged()
    );
    (m_in, m_out, m_lo, m_hi, sim.is_converged())
}

/// Criterion (ADR 0068 S3', registered): converges within the cap and |m_in + m_out| <= 1e-6 |m_in|. KNOWN MISS: the mass
/// flow through the pair is antisymmetric to round-off on this mesh (see the seam-flux diagnosis), but the faceted shroud wall
/// moves tangentially in the relative frame and its facets carry a small wall-normal velocity component, so the `shroud`
/// patch has a nonzero flux (m_shroud) that appears in m_in + m_out.
#[test]
#[ignore = "known miss: the faceted moving shroud carries a mass flux, see ADR 0068 addendum and the corrected test"]
fn s3_prime_rotating_passage_known_miss() {
    let (m_in, m_out, m_lo, m_hi, conv) = s3_balances(20.0);
    assert!(conv, "not converged");
    println!("S3' pair mismatch {:.3e}", (m_lo + m_hi).abs() / m_lo.abs());
    assert!((m_in + m_out).abs() <= 1e-6 * m_in.abs());
}

/// Corrected S3' (limit unchanged at 1e-6): the balance includes the flux of the moving faceted shroud wall; the converged pair
/// has m_lo + m_hi = 0 to round-off.
#[test]
fn s3_prime_rotating_passage_converges_and_balances_with_the_shroud() {
    let sim = run(&rotating_case(20.0), 6000);
    assert!(sim.is_converged(), "not converged");
    let m = |k: &str| sim.monitors.last(k).unwrap();
    let (m_in, m_out, m_lo, m_hi, m_sh) = (m("m_in"), m("m_out"), m("m_lo"), m("m_hi"), m("m_shroud"));
    println!(
        "S3' m_in {m_in:.6e} m_out {m_out:.6e} m_shroud {m_sh:.3e}; |m_in + m_out| / |m_in| = {:.3e}; with the shroud {:.3e}; pair {:.3e}",
        (m_in + m_out).abs() / m_in.abs(),
        (m_in + m_out + m_sh).abs() / m_in.abs(),
        (m_lo + m_hi).abs() / m_lo.abs()
    );
    assert!((m_in + m_out + m_sh).abs() <= 1e-6 * m_in.abs());
    assert!((m_lo + m_hi).abs() <= 1e-12 * m_lo.abs());
}
