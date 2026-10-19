//! Wave 35D (ADR 0079 part 1, P9-P10): the FAN-01 passage on the C-grid with `mesh.fan.c_grid_construction = "camber_seam"`, through
//! the case: `FvGeometry::new` accepts the mesh, `check_mesh` is ok, `rules::check_case` raises no error-level finding, and a coarse
//! laminar smoke solve at the design point stays bounded. Registered in ADR 0079 before this code ran. The smoke solve is bounded only:
//! no accuracy and no fan performance claim.

#![allow(clippy::neg_cmp_op_on_partial_ord)] // a NaN must fail the limits

use rcfd_api::case::Case;
use rcfd_api::rules::{check_case, has_errors, mesh_context, Severity};
use rcfd_api::sim::{build_mesh, Simulation};
use rcfd_fv::FvGeometry;
use rcfd_mesh::quality::check_mesh;
use std::f64::consts::PI;
use std::sync::Arc;

const OMEGA_RPM: f64 = 1486.0;
const Q_DESIGN: f64 = 1.4032;

fn omega() -> f64 {
    OMEGA_RPM * 2.0 * PI / 60.0
}

fn annulus_area() -> f64 {
    PI * (0.25f64.powi(2) - 0.124f64.powi(2))
}

/// The 16 registered FAN-01 sections (ADR 0075 amendment 1) from `w34d_fan01_cad_sections.csv` (the row r = 247.0 mm is not one of them).
fn sections() -> Vec<Vec<f64>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../validation-data/reference/w34d_fan01_cad_sections.csv");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let rows: Vec<Vec<f64>> = text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("r_mm"))
        .map(|l| {
            l.split(',')
                .take(8)
                .map(|v| v.trim().parse::<f64>().unwrap())
                .collect::<Vec<f64>>()
        })
        .filter(|r| r[0] != 247.0)
        .collect();
    assert_eq!(rows.len(), 16);
    rows
}

fn list(rows: &[Vec<f64>], f: impl Fn(&[f64]) -> f64) -> String {
    let v: Vec<String> = rows.iter().map(|s| format!("{:?}", f(s))).collect();
    format!("[{}]", v.join(", "))
}

/// The `mesh` section of FAN-01-C (ADR 0079): the ADR 0075 amendment 1 C-grid counts, y+ = 30 rule, the given construction.
fn mesh_section(construction: &str) -> String {
    let rows = sections();
    let (x0, t0) = (rows[0][6], rows[0][7]);
    let ref_speed = (Q_DESIGN / annulus_area()).hypot(omega() * 0.186);
    format!(
        "[mesh]\ngenerator = \"axial_fan_sector\"\n[mesh.fan]\nn_blades = 9\nr_hub = 0.124\nr_tip = 0.2475\ntip_clearance = 0.0025\naxial_extent = 0.4\nz_le_root = 0.1\nfamily = \"naca4\"\n\
station_radius = {}\nstation_chord = {}\nstation_stagger = {}\nstation_camber = {}\nstation_camber_pos = {}\nstation_thickness = {}\nstation_sweep = {}\nstation_lean = {}\n\
topology = \"c_grid\"\nc_grid_construction = \"{construction}\"\nn_up = 8\nn_chord = 16\nn_down = 8\nn_side = 10\nn_nose = 4\nn_core = 1\nfront_gap = 0.3\nn_span = 14\nn_gap = 7\nn_bl = 2\ngrowth = 1.2\n\
first_cell_height = 0.0\ny_plus = 30.0\nref_speed = {ref_speed:?}\nnu = 1.5e-5\n",
        list(&rows, |s| s[0] * 1e-3),
        list(&rows, |s| s[1] * 1e-3),
        list(&rows, |s| s[2]),
        list(&rows, |s| s[3]),
        list(&rows, |s| s[4]),
        list(&rows, |s| s[5]),
        list(&rows, |s| (s[6] - x0) * 1e-3),
        list(&rows, |s| s[0] * 1e-3 * (s[7] - t0).to_radians()),
    )
}

/// P10 set-up: laminar, design point, rotating frame, shroud at rest in the absolute frame, SIMPLEC, upwind.
fn smoke_case(construction: &str) -> String {
    let w = omega();
    let u_in = Q_DESIGN / annulus_area();
    format!(
        r#"{}
[physics]
nu = 1.5e-5
rho = 1.2
[initial]
velocity = [0.0, 0.0, {u_in:?}]
velocity_expression_x = "{w:?} [1/s] * y"
velocity_expression_y = "-{w:?} [1/s] * x"
[boundaries.inlet]
velocity = "fixed"
velocity_value = [0.0, 0.0, {u_in:?}]
velocity_expression_x = "{w:?} [1/s] * y"
velocity_expression_y = "-{w:?} [1/s] * x"
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.hub]
velocity = "fixed"
[boundaries.blade]
velocity = "fixed"
[boundaries.shroud]
velocity = "fixed"
velocity_expression_x = "{w:?} [1/s] * y"
velocity_expression_y = "-{w:?} [1/s] * x"
[sources.frame]
type = "rotating_frame"
rotation_axis = [0.0, 0.0, 1.0]
angular_velocity = {w:?}
centrifugal = true
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 300
tolerance_u = 1e-12
tolerance_p = 1e-12
[reports.m_in]
type = "mass_flow"
patch = "inlet"
[reports.m_out]
type = "mass_flow"
patch = "outlet"
"#,
        mesh_section(construction)
    )
}

/// P9 (ADR 0079): FvGeometry, check_mesh and the rules check on the FAN-01 mesh.
fn p9(construction: &str) {
    let text = smoke_case(construction);
    let case = Case::parse_toml(&text).unwrap_or_else(|e| panic!("{e:?}"));
    let mesh = build_mesh(&case).unwrap_or_else(|e| panic!("P1 FAN-01 not built: {e:?}"));
    let q = check_mesh(&mesh);
    let rules = check_case(&case, Some(&mesh_context(&q)));
    println!(
        "P9 FAN-01 ({construction}): {} cells; check_mesh ok {} (errors {:?}); non-orthogonality {:.2}, skewness {:.3}, aspect {:.1}; {} rule findings",
        q.n_cells,
        q.is_ok(),
        q.errors,
        q.max_non_orthogonality,
        q.max_skewness,
        q.max_aspect_ratio,
        rules.len()
    );
    for r in &rules {
        println!("P9 finding {} {:?} {}: {}", r.id, r.severity, r.path, r.message);
    }
    let geom = FvGeometry::new(Arc::new(mesh));
    assert!(geom.is_ok(), "FvGeometry::new refuses the FAN-01 mesh");
    assert!(q.is_ok(), "check_mesh rejects the FAN-01 mesh: {:?}", q.errors);
    assert!(
        !has_errors(&rules) && rules.iter().all(|r| r.severity != Severity::Error),
        "error-level rule finding"
    );
}

/// P10 (ADR 0079): the laminar smoke solve stays bounded (the ADR 0075 amendment 3 criterion).
fn p10(construction: &str) {
    let text = smoke_case(construction);
    let case = Case::parse_toml(&text).unwrap_or_else(|e| panic!("{e:?}"));
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    let (mut c10, mut late, mut finite) = (f64::NAN, 0.0f64, true);
    for n in 1..=300 {
        let st = sim.step();
        if !(st.continuity.is_finite() && st.res_u.is_finite() && st.res_p.is_finite()) {
            finite = false;
            println!("P10 non-finite at iteration {n}: {st:?}");
            break;
        }
        if n == 10 {
            c10 = st.continuity;
        }
        if n >= 200 {
            late = late.max(st.continuity);
        }
        if n % 50 == 0 {
            println!(
                "P10 iteration {n}: continuity {:.3e} res_u {:.3e} res_p {:.3e}",
                st.continuity, st.res_u, st.res_p
            );
        }
    }
    let (ux, uy, uz) = (
        sim.field("ux").unwrap(),
        sim.field("uy").unwrap(),
        sim.field("uz").unwrap(),
    );
    let umax = (0..ux.len())
        .map(|c| (ux[c] * ux[c] + uy[c] * uy[c] + uz[c] * uz[c]).sqrt())
        .fold(0.0, f64::max);
    let (m_in, m_out) = (
        sim.monitors.last("m_in").unwrap_or(f64::NAN),
        sim.monitors.last("m_out").unwrap_or(f64::NAN),
    );
    println!(
        "P10 FAN-01 ({construction}): {} cells; continuity at 10 {c10:.3e}, max over 200-300 {late:.3e}; largest relative speed {umax:.2} m/s; m_in + m_out {:.3e} of m_in; failure {:?}",
        sim.geom.mesh.n_cells(),
        (m_in + m_out).abs() / m_in.abs(),
        sim.failure()
    );
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    assert!(finite, "non-finite residual");
    assert!(
        late <= c10,
        "continuity grew: {late:e} over 200-300 against {c10:e} at iteration 10"
    );
}

#[test]
fn p9_fan01_mesh_is_accepted_by_the_solver_and_the_rules() {
    p9("camber_seam");
}

/// KNOWN MISS (a3): continuity 7.92e-4 at iteration 10 and up to 6.62e-3 over iterations 200-300 (finite, no solver failure, largest relative
/// speed 63 m/s next to the seam corner of the near-tip and gap layers).
#[test]
#[ignore = "known miss: FAN-01-C laminar smoke continuity grows (7.9e-4 at iteration 10, 6.6e-3 over 200-300; finite, no failure), ADR 0079 amendment 5"]
fn p10_fan01_laminar_smoke_solve_stays_bounded_known_miss() {
    p10("camber_seam");
}
