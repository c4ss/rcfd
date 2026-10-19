//! Wave 34A (ADR 0072, part 1): the wall flux of a moving wall that is a surface of revolution. F1 (falsifier of L-H1: the
//! shroud mass flow of the converged S3' passage is the geometric number sum u_w(c_f) . S_f), the characterisation by the
//! exact facet flux of the rigid rotation, T2 (empty C-grid sector Couette with and without the tangential projection),
//! S3'' (the S3' passage with the projected shroud) and T3 (the default still rejects a non-tangential moving wall).
//! The limits were registered before this code ran.

use rcfd_api::case::Case;
use rcfd_api::sim::{build_mesh, Simulation};
use rcfd_core::vec3::{self, Vec3};
use rcfd_mesh::Mesh;

const OMEGA: f64 = 20.0;

/// Mesh section of the registered small passage S on the C-grid topology (ADR 0068).
fn mesh_section(include_blade: bool) -> String {
    format!(
        "[mesh]\ngenerator = \"axial_fan_sector\"\n[mesh.fan]\ntopology = \"c_grid\"\nn_up = 8\nn_chord = 16\nn_down = 8\nn_side = 10\nn_nose = 4\nn_core = 1\nn_span = 14\nn_gap = 7\nn_bl = 2\ngrowth = 1.2\nfirst_cell_height = 1e-3\ninclude_blade = {include_blade}\n"
    )
}

/// The S3' set-up of ADR 0068 with the given shroud section.
fn rotating_case(shroud: &str) -> String {
    format!(
        r#"{}
[physics]
nu = 0.02
rho = 1.0
[boundaries.inlet]
velocity = "fixed"
velocity_value = [0.0, 0.0, 1.0]
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.hub]
velocity = "fixed"
[boundaries.blade]
velocity = "fixed"
[boundaries.shroud]
{shroud}
[sources.frame]
type = "rotating_frame"
rotation_axis = [0.0, 0.0, 1.0]
angular_velocity = {OMEGA:?}
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

/// The 33A shroud: stationary in the absolute frame, given as expressions evaluated at the face centres.
fn shroud_expressions() -> String {
    format!("velocity = \"fixed\"\nvelocity_expression_x = \"{OMEGA:?} [1/s] * y\"\nvelocity_expression_y = \"-{OMEGA:?} [1/s] * x\"\n")
}

fn run(text: &str, iterations: usize) -> Simulation {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}\n{text}"));
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    let n = sim.run(iterations);
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    println!("  {n} iterations, converged {}", sim.is_converged());
    sim
}

/// Rigid rotation `w e_z x x`.
fn rot(w: f64, p: Vec3) -> Vec3 {
    [-w * p[1], w * p[0], 0.0]
}

/// Exact flux of the rigid rotation `w e_z x x` through the facet bounded by the straight edges of face `f` (in the face's point
/// order, the circulation that defines its area vector): `-(w/2) oint r^2 dz`, `int r^2 dz = dz (|P|^2 + P.Q + |Q|^2)/3` per edge.
fn exact_facet_flux(mesh: &Mesh, f: usize, w: f64) -> f64 {
    let ids = mesh.face_points(f);
    let pts = mesh.points();
    let mut s = 0.0;
    for m in 0..ids.len() {
        let (p, q) = (pts[ids[m]], pts[ids[(m + 1) % ids.len()]]);
        let pq = p[0] * q[0] + p[1] * q[1];
        s += (q[2] - p[2]) * (p[0] * p[0] + p[1] * p[1] + pq + q[0] * q[0] + q[1] * q[1]) / 3.0;
    }
    -0.5 * w * s
}

/// Geometric wall flux of a patch for the wall rotation rate `w`: (sum u_w(c_f) . S_f, sum of the exact facet fluxes, sum of the
/// magnitudes of the exact facet fluxes, max |u_w . n| / |u_w|, max |u_w(c_f) . S_f - exact| / (|u_w| |S_f|)).
fn geometric_flux(mesh: &Mesh, patch: &str, w: f64) -> (f64, f64, f64, f64, f64) {
    let (cf, sf) = (mesh.face_centres(), mesh.face_area_vectors());
    let (mut g, mut ex, mut ex_abs, mut tilt, mut quad): (f64, f64, f64, f64, f64) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for f in mesh.patch(patch).unwrap().faces() {
        let u = rot(w, cf[f]);
        let un = vec3::dot(u, sf[f]);
        let e = exact_facet_flux(mesh, f, w);
        g += un;
        ex += e;
        ex_abs += e.abs();
        let scale = vec3::norm(u) * vec3::norm(sf[f]);
        tilt = tilt.max(un.abs() / scale);
        quad = quad.max((un - e).abs() / scale);
    }
    (g, ex, ex_abs, tilt, quad)
}

#[test]
fn exact_facet_flux_formula_matches_the_centre_rule_on_planar_facets() {
    // the centre rule is exact for a linear field on a planar polygon: a tilted planar quad off the axis
    let pts = vec![
        [0.30, 0.02, 0.10],
        [0.31, 0.06, 0.12],
        [0.29, 0.09, 0.20],
        [0.27, 0.03, 0.17],
    ];
    // make it planar: project the last point onto the plane of the first three
    let n = vec3::unit(vec3::cross(vec3::sub(pts[1], pts[0]), vec3::sub(pts[2], pts[0])));
    let mut p = pts.clone();
    let d = vec3::dot(vec3::sub(p[3], p[0]), n);
    p[3] = vec3::sub(p[3], vec3::scale(n, d));
    let (c, s) = rcfd_mesh::mesh::polygon_geometry(&p, &[0, 1, 2, 3]);
    let centre_rule = vec3::dot(rot(-7.0, c), s);
    let mut e = 0.0;
    for m in 0..4 {
        let (a, b) = (p[m], p[(m + 1) % 4]);
        e += (b[2] - a[2]) * (a[0] * a[0] + a[1] * a[1] + a[0] * b[0] + a[1] * b[1] + b[0] * b[0] + b[1] * b[1]) / 3.0;
    }
    let exact = 3.5 * e;
    println!("planar facet: centre rule {centre_rule:.16e}, edge formula {exact:.16e}");
    assert!((centre_rule - exact).abs() <= 1e-14 * centre_rule.abs().max(1e-30));
}

/// F1 (falsifier of L-H1) with the characterisation. Decision rule of ADR 0072: |m_shroud - G| <= 1e-12 |m_in| confirms L-H1,
/// > 1e-9 |m_in| falsifies it.
#[test]
fn f1_shroud_mass_flow_is_the_geometric_wall_flux() {
    let sim = run(&rotating_case(&shroud_expressions()), 6000);
    let mesh = &sim.geom.mesh;
    let m = |k: &str| sim.monitors.last(k).unwrap();
    let (m_in, m_sh) = (m("m_in"), m("m_shroud"));
    // the shroud velocity (Omega y, -Omega x, 0) is the rigid rotation at rate -Omega
    let (g, ex, ex_abs, tilt, quad) = geometric_flux(mesh, "shroud", -OMEGA);
    println!(
        "F1: converged {}; m_in {m_in:.10e}; m_shroud {m_sh:.10e}; G = sum u_w(c_f).S_f {g:.10e}",
        sim.is_converged()
    );
    println!(
        "F1: |m_shroud - G| / |m_in| = {:.3e}; G / m_in = {:.3e}",
        (m_sh - g).abs() / m_in.abs(),
        g / m_in
    );
    println!(
        "F1 characterisation: sum of exact facet fluxes {ex:.3e} (sum of magnitudes {ex_abs:.3e}, ratio {:.3e}); max |u_w.n|/|u_w| {tilt:.3e}; max |centre rule - exact| / (|u_w||S|) {quad:.3e}",
        ex.abs() / ex_abs.max(1e-300)
    );
    // the 0065 H-slit example mesh (no solve), reported only
    let text = include_str!("../../../cases/axial_fan_sector/axial_fan_sector.toml");
    let hs = build_mesh(&Case::parse_toml(text).unwrap()).unwrap();
    let (gh, exh, exh_abs, tilth, _) = geometric_flux(&hs, "shroud", -OMEGA);
    println!(
        "F1 H-slit (0065 example mesh): G {gh:.3e} ({:.3e} of m_in of the C-grid run); exact facet sum {exh:.3e} of magnitudes {exh_abs:.3e}; max |u_w.n|/|u_w| {tilth:.3e}",
        gh / m_in
    );
    assert!(sim.is_converged(), "S3' not converged");
    assert!(
        (m_sh - g).abs() <= 1e-9 * m_in.abs(),
        "L-H1 falsified: the shroud flow is not the geometric wall flux"
    );
}

/// The projected shroud of the rotating-frame passage: stationary in the absolute frame, i.e. rotating at -Omega in the frame.
fn shroud_projected() -> String {
    format!(
        "velocity = \"moving_wall\"\nwall_rotation_rate = {:?}\nwall_velocity_projection = \"tangential\"\n",
        -OMEGA
    )
}

/// S3'' (ADR 0072): the S3' passage with the projected shroud. Registered limit of S3' unchanged (1e-6).
#[test]
fn s3_second_rotating_passage_with_the_projected_shroud_balances() {
    let sim = run(&rotating_case(&shroud_projected()), 6000);
    let m = |k: &str| sim.monitors.last(k).unwrap();
    let (m_in, m_out, m_sh, m_lo, m_hi) = (m("m_in"), m("m_out"), m("m_shroud"), m("m_lo"), m("m_hi"));
    println!(
        "S3'': converged {}; m_in {m_in:.10e} m_out {m_out:.10e} m_shroud {m_sh:.3e}; |m_in + m_out| / |m_in| = {:.3e}; |m_shroud| / |m_in| = {:.3e}; pair {:.3e}",
        sim.is_converged(),
        (m_in + m_out).abs() / m_in.abs(),
        m_sh.abs() / m_in.abs(),
        (m_lo + m_hi).abs() / m_lo.abs()
    );
    assert!(sim.is_converged(), "not converged");
    assert!((m_in + m_out).abs() <= 1e-6 * m_in.abs());
    assert!(m_sh.abs() <= 1e-14 * m_in.abs());
    assert!((m_lo + m_hi).abs() <= 1e-12 * m_lo.abs());
}

/// T3: without the projection key a moving wall that is not tangential to its facets is still rejected (33A behaviour).
#[test]
fn t3_default_still_rejects_a_non_tangential_moving_wall() {
    let text = rotating_case(&format!(
        "velocity = \"moving_wall\"\nwall_rotation_rate = {:?}\n",
        -OMEGA
    ));
    let case = Case::parse_toml(&text).unwrap();
    let e = match Simulation::new(&case) {
        Ok(_) => panic!("the non-tangential moving wall was accepted"),
        Err(e) => e,
    };
    println!("T3: {e:?}");
    assert!(format!("{e:?}").contains("the wall velocity must be tangential to the wall"));
    // the projection on another velocity kind is a located error
    let text = rotating_case(&format!(
        "velocity = \"partial_slip\"\nslip_length = 0.01\nwall_rotation_rate = {:?}\nwall_velocity_projection = \"tangential\"\n",
        -OMEGA
    ));
    let e = match Simulation::new(&Case::parse_toml(&text).unwrap()) {
        Ok(_) => panic!("projection accepted on partial_slip"),
        Err(e) => e,
    };
    assert!(format!("{e:?}").contains("wall_velocity_projection"), "{e:?}");
}

/// Circular Couette `u_theta = A r + B/r` between the hub (w1) and the shroud (w2).
fn couette_ab(r1: f64, r2: f64, w1: f64, w2: f64) -> (f64, f64) {
    let a = (w2 * r2 * r2 - w1 * r1 * r1) / (r2 * r2 - r1 * r1);
    let b = (w1 - w2) * r1 * r1 * r2 * r2 / (r2 * r2 - r1 * r1);
    (a, b)
}

const R1: f64 = 0.10;
const R2: f64 = 0.31;
const W1: f64 = 10.0;
const W2: f64 = 20.0;

/// The D2 set-up of ADR 0068 (empty C-grid sector, stationary frame) with the given hub and shroud sections.
fn couette_case(projected: bool) -> String {
    let (a, b) = couette_ab(R1, R2, W1, W2);
    let wall = |w: f64| {
        if projected {
            format!(
                "velocity = \"moving_wall\"\nwall_rotation_rate = {w:?}\nwall_velocity_projection = \"tangential\"\n"
            )
        } else {
            format!("velocity = \"fixed\"\nvelocity_expression_x = \"-{w:?} [1/s] * y\"\nvelocity_expression_y = \"{w:?} [1/s] * x\"\n")
        }
    };
    format!(
        r#"[mesh]
generator = "axial_fan_sector"
[mesh.fan]
topology = "c_grid"
include_blade = false
n_up = 6
n_chord = 12
n_down = 6
n_side = 8
n_nose = 3
n_core = 1
n_span = 8
n_gap = 6
n_bl = 2
growth = 1.2
first_cell_height = 1e-3
[physics]
nu = 0.02
rho = 1.0
[boundaries.inlet]
velocity = "fixed"
velocity_expression_x = "-({a:?} [1/s] + {b:?} [m^2/s] / (x*x + y*y)) * y"
velocity_expression_y = "({a:?} [1/s] + {b:?} [m^2/s] / (x*x + y*y)) * x"
velocity_expression_z = "0.0 [m/s]"
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.hub]
{}
[boundaries.shroud]
{}
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 4000
tolerance_u = 1e-12
tolerance_p = 1e-12
"#,
        wall(W1),
        wall(W2)
    )
}

/// (error / (W2 R2), max per-face wall leak of hub and shroud, converged)
fn couette_run(projected: bool) -> (f64, f64, bool) {
    let sim = run(&couette_case(projected), 4000);
    let (a, b) = couette_ab(R1, R2, W1, W2);
    let mesh = &sim.geom.mesh;
    let cc = mesh.cell_centres();
    let (ux, uy) = (sim.field("ux").unwrap(), sim.field("uy").unwrap());
    let mut e: f64 = 0.0;
    for c in 0..mesh.n_cells() {
        let (x, y) = (cc[c][0], cc[c][1]);
        let r = x.hypot(y);
        let ut = (-y * ux[c] + x * uy[c]) / r;
        e = e.max((ut - (a * r + b / r)).abs() / (W2 * R2));
    }
    let phi = sim.flux();
    let (cf, sf) = (mesh.face_centres(), mesh.face_area_vectors());
    let mut leak: f64 = 0.0;
    for (patch, w) in [("hub", W1), ("shroud", W2)] {
        for f in mesh.patch(patch).unwrap().faces() {
            leak = leak.max(phi[f].abs() / (vec3::norm(rot(w, cf[f])) * vec3::norm(sf[f])));
        }
    }
    println!(
        "T2 {}: converged {}; error {e:.4e} of W2 R2; wall leak max {leak:.3e}",
        if projected { "projected" } else { "unprojected" },
        sim.is_converged()
    );
    (e, leak, sim.is_converged())
}

/// T2 (ADR 0072): the empty C-grid sector Couette with and without the projection.
#[test]
fn t2_empty_cgrid_sector_couette_projection() {
    let (e0, l0, c0) = couette_run(false);
    let (e1, l1, c1) = couette_run(true);
    println!("T2: unprojected error {e0:.4e}, leak {l0:.3e}; projected error {e1:.4e}, leak {l1:.3e}");
    assert!(c0 && c1, "not converged");
    assert!(l1 <= 1e-13, "projected leak {l1}");
    assert!(e1 <= e0, "the projected error {e1} is above the unprojected {e0}");
}
