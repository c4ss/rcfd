//! Porous media and rotating-frame sources (F-031) through the settings tree.
//!
//! Checks fixed before running:
//! * packed bed (superficial velocity 0.15 m/s through a 1 m bed, e = 0.4, d = 5 mm, nu = 1.5e-5,
//!   channel with slip walls, 40 x 8 cells): the kinematic pressure gradient between the columns at
//!   30 % and 70 % of the length matches the Ergun equation (150 / 1.75 constants) within 1 %, the
//!   viscous and the inertial term each contribute at least 40 % of the total, the direct
//!   Darcy-Forchheimer input with the equivalent K and C2 gives the same field (difference < 1e-9),
//!   and a porous zone in the middle half of the bed only changes the gradient there (outside
//!   the zone the gradient is below 1 % of the Ergun value);
//! * spanwise-rotating laminar plane channel (Poiseuille flow in a periodic channel, Omega = 2 rad/s,
//!   Coriolis force only): the velocity stays the parabola u = g/(2 nu) y (H - y) within 1 % of the
//!   centre-line value and the transverse pressure difference between the first and last cell matches
//!   dp/dy = -2 Omega u(y), integrated analytically between the cell centres, within 1 %;
//! * fluid at rest in a rotating frame (closed box, co-rotating walls, Coriolis and centrifugal): the
//!   spurious speed stays below 1e-3 of Omega times the largest radius and the pressure follows
//!   Omega^2 r^2 / 2 within 1 % of its range;
//! * invalid source settings and unsupported algorithms are diagnosed.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

const L: f64 = 1.0;
const E: f64 = 0.4;
const D: f64 = 0.005;
const NU: f64 = 1.5e-5;
const UIN: f64 = 0.15;

fn bed(sources: &str) -> String {
    format!(
        r#"
[mesh]
generator = "channel_2d"
nx = 40
ny = 8
lx = {L}
ly = 0.2
depth = 0.05
[physics]
nu = {NU}
rho = 1.0
[initial]
velocity = [{UIN}, 0.0, 0.0]
[boundaries.inlet]
velocity = "fixed"
velocity_value = [{UIN}, 0.0, 0.0]
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.walls]
velocity = "symmetry"
[solver]
algorithm = "simplec"
convection = "upwind"
h_operator = "from_solution"
momentum_rel_tol = 1e-8
pressure_rel_tol = 1e-8
pressure_max_iter = 1000
max_iterations = 3000
tolerance_u = 1e-10
tolerance_p = 1e-10
{sources}
"#
    )
}

/// Run `n` iterations, then 200 more, and require the fields to be stationary (change of ux, uy and p below 1e-8 of the
/// maximum speed, respectively pressure). The normalised residual is not used as the stop criterion: for
/// the uniform or linear solutions of these tests its normalisation constant vanishes.
fn run(text: &str) -> Simulation {
    run_with_floor(text, 1e-12)
}

/// [`run`] with a floor for the velocity scale of the stationarity check (a fluid at rest has speeds at round-off level,
/// for which a relative change is meaningless).
fn run_with_floor(text: &str, speed_floor: f64) -> Simulation {
    let case = Case::parse_toml(text).unwrap();
    let mut sim = Simulation::new(&case).unwrap();
    let n = 1500;
    for _ in 0..n {
        sim.step();
    }
    let before: Vec<Vec<f64>> = ["ux", "uy", "p"].iter().map(|f| sim.field(f).unwrap()).collect();
    for _ in 0..200 {
        sim.step();
    }
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    for (f, old) in ["ux", "uy", "p"].iter().zip(&before) {
        let new = sim.field(f).unwrap();
        let reference = if *f == "p" {
            new.clone()
        } else {
            sim.field("speed").unwrap()
        };
        let scale = reference
            .iter()
            .map(|v| v.abs())
            .fold(if *f == "p" { 1e-12 } else { speed_floor }, f64::max);
        let change = new.iter().zip(old).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max) / scale;
        assert!(
            change < 1e-8,
            "{f} is not stationary after {n} iterations: change {change:.3e}"
        );
    }
    sim
}

fn column_pressure(sim: &Simulation, x_target: f64) -> (f64, f64) {
    let c = sim.geom.mesh.cell_centres();
    let p = sim.field("p").unwrap();
    let x = c
        .iter()
        .map(|v| v[0])
        .min_by(|a, b| (a - x_target).abs().partial_cmp(&(b - x_target).abs()).unwrap())
        .unwrap();
    let idx: Vec<usize> = (0..c.len()).filter(|&i| (c[i][0] - x).abs() < 1e-9).collect();
    (x, idx.iter().map(|&i| p[i]).sum::<f64>() / idx.len() as f64)
}

fn gradient(sim: &Simulation, a: f64, b: f64) -> f64 {
    let (x1, p1) = column_pressure(sim, a * L);
    let (x2, p2) = column_pressure(sim, b * L);
    (p1 - p2) / (x2 - x1)
}

fn ergun_kinematic(u: f64) -> (f64, f64) {
    let visc = NU * 150.0 * (1.0 - E).powi(2) * u / (E.powi(3) * D * D);
    let inert = 1.75 * (1.0 - E) * u * u / (E.powi(3) * D);
    (visc, inert)
}

#[test]
fn packed_bed_pressure_drop_matches_the_ergun_equation() {
    let (visc, inert) = ergun_kinematic(UIN);
    let exact = visc + inert;
    assert!(visc > 0.4 * exact && inert > 0.4 * exact, "{visc} {inert}");
    let sim = run(&bed(&format!(
        "[sources.bed]\ntype = \"porous\"\nmodel = \"ergun\"\nporosity = {E}\nparticle_diameter = {D}\n"
    )));
    let g = gradient(&sim, 0.3, 0.7);
    eprintln!(
        "Ergun: iterations {}, dp/dx {g:.4} vs {exact:.4} ({:+.3}%)",
        sim.last().unwrap().index,
        100.0 * (g / exact - 1.0)
    );
    assert!((g / exact - 1.0).abs() < 0.01, "dp/dx {g} vs Ergun {exact}");

    // the same bed through the direct Darcy-Forchheimer inputs
    let inv_k = 150.0 * (1.0 - E).powi(2) / (E.powi(3) * D * D);
    let c2 = 3.5 * (1.0 - E) / (E.powi(3) * D);
    let sim2 = run(&bed(&format!(
        "[sources.bed]\ntype = \"porous\"\nmodel = \"darcy_forchheimer\"\npermeability = {}\ninertial_resistance = {c2}\n",
        1.0 / inv_k
    )));
    let (a, b) = (sim.field("p").unwrap(), sim2.field("p").unwrap());
    let diff = a.iter().zip(&b).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max);
    assert!(diff < 1e-9 * exact.max(1.0), "pressure difference {diff}");
}

#[test]
fn porous_zone_acts_only_inside_its_box() {
    let (visc, inert) = ergun_kinematic(UIN);
    let exact = visc + inert;
    let sim = run(&bed(&format!(
        "[sources.bed]\ntype = \"porous\"\nzone = \"box\"\nbox_min = [0.25, -1.0, -1.0]\nbox_max = [0.75, 1.0, 1.0]\nporosity = {E}\nparticle_diameter = {D}\n"
    )));
    let inside = gradient(&sim, 0.4, 0.6);
    let outside = gradient(&sim, 0.8, 0.95);
    eprintln!("zone: inside {inside:.4} (Ergun {exact:.4}), outside {outside:.5}");
    assert!((inside / exact - 1.0).abs() < 0.01, "inside {inside} vs {exact}");
    assert!(outside.abs() < 0.01 * exact, "outside gradient {outside}");
}

const ROT_CHANNEL: &str = r#"
[mesh]
generator = "box_2d"
nx = 4
ny = 40
lx = 0.4
ly = 1.0
periodic_x = true
[physics]
nu = 0.1
rho = 1.0
body_force = [1.0, 0.0, 0.0]
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[sources.frame]
type = "rotating_frame"
rotation_axis = [0.0, 0.0, 1.0]
angular_velocity = 2.0
centrifugal = false
[solver]
algorithm = "simplec"
convection = "upwind"
h_operator = "from_solution"
momentum_rel_tol = 1e-9
pressure_rel_tol = 1e-9
pressure_max_iter = 1000
max_iterations = 3000
tolerance_u = 1e-10
tolerance_p = 1e-10
"#;

#[test]
fn rotating_channel_has_a_parabolic_profile_and_geostrophic_pressure() {
    let sim = run(ROT_CHANNEL);
    let (nu, g, omega, h) = (0.1, 1.0, 2.0, 1.0);
    let u_of = |y: f64| g / (2.0 * nu) * y * (h - y);
    let c = sim.geom.mesh.cell_centres();
    let ux = sim.field("ux").unwrap();
    let p = sim.field("p").unwrap();
    let uc = u_of(0.5);
    let perr = (0..c.len())
        .map(|i| (ux[i] - u_of(c[i][1])).abs() / uc)
        .fold(0.0, f64::max);
    // pressure: column at x = first cell; first and last cell in y
    let x0 = c.iter().map(|v| v[0]).fold(f64::INFINITY, f64::min);
    let mut col: Vec<usize> = (0..c.len()).filter(|&i| (c[i][0] - x0).abs() < 1e-9).collect();
    col.sort_by(|&a, &b| c[a][1].partial_cmp(&c[b][1]).unwrap());
    let (y0, y1) = (c[col[0]][1], c[*col.last().unwrap()][1]);
    let antider = |y: f64| g / (2.0 * nu) * (h * y * y / 2.0 - y * y * y / 3.0);
    let dp_exact = -2.0 * omega * (antider(y1) - antider(y0));
    let dp = p[*col.last().unwrap()] - p[col[0]];
    eprintln!(
        "rotating channel: profile error {:.3}% of centre-line, dp {dp:.5} vs {dp_exact:.5} ({:+.3}%)",
        100.0 * perr,
        100.0 * (dp / dp_exact - 1.0)
    );
    assert!(perr < 0.01, "profile error {perr}");
    assert!((dp / dp_exact - 1.0).abs() < 0.01, "dp {dp} vs {dp_exact}");
    // no cross-stream velocity
    let uy = sim.field("uy").unwrap();
    eprintln!(
        "rotating channel: max abs uy {:.3e} ({:.3e} of centre-line)",
        uy.iter().map(|v| v.abs()).fold(0.0, f64::max),
        uy.iter().map(|v| v.abs()).fold(0.0, f64::max) / uc
    );
    // sanity bound only (set after the measurement: the cross-stream velocity is the O(h) wall-pressure
    // inconsistency discussed in the roadmap note of F-031)
    assert!(uy.iter().map(|v| v.abs()).fold(0.0, f64::max) < 1e-3 * uc);
}

fn rest_text(n: usize) -> String {
    format!(
        r#"
[mesh]
generator = "box_2d"
nx = {n}
ny = {n}
lx = 1.0
ly = 1.0
[physics]
nu = 0.05
rho = 1.0
[boundaries.xmin]
velocity = "fixed"
[boundaries.xmax]
velocity = "fixed"
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
[sources.frame]
type = "rotating_frame"
rotation_origin = [0.5, 0.5, 0.0]
angular_velocity = 3.0
[solver]
algorithm = "simplec"
convection = "upwind"
h_operator = "from_solution"
momentum_rel_tol = 1e-9
pressure_rel_tol = 1e-9
pressure_max_iter = 2000
max_iterations = 3000
tolerance_u = 1e-10
tolerance_p = 1e-10
"#
    )
}

/// (max speed / (Omega R_max), pressure error as a fraction of the pressure range) of the fluid at rest.
fn rest_errors(n: usize) -> (f64, f64) {
    let sim = run_with_floor(&rest_text(n), 1e-3);
    let (omega, rmax) = (3.0, 0.5 * 2f64.sqrt());
    let c = sim.geom.mesh.cell_centres();
    let umax = sim.field("speed").unwrap().iter().copied().fold(0.0, f64::max);
    let p = sim.field("p").unwrap();
    let r2: Vec<f64> = c.iter().map(|v| (v[0] - 0.5).powi(2) + (v[1] - 0.5).powi(2)).collect();
    let head = |i: usize, j: usize| 0.5 * omega * omega * (r2[i] - r2[j]);
    let i_min = (0..r2.len())
        .min_by(|&a, &b| r2[a].partial_cmp(&r2[b]).unwrap())
        .unwrap();
    let i_max = (0..r2.len())
        .max_by(|&a, &b| r2[a].partial_cmp(&r2[b]).unwrap())
        .unwrap();
    let range = head(i_max, i_min);
    let perr = (0..p.len())
        .map(|i| ((p[i] - p[i_min]) - head(i, i_min)).abs())
        .fold(0.0, f64::max)
        / range;
    (umax / (omega * rmax), perr)
}

/// Fluid at rest in a rotating frame, with the criterion fixed beforehand (wave 9): spurious speed below 1e-3 of
/// Omega R_max and pressure error below 1 % of the range, on the 24 x 24 mesh of the original test. Wave 9 measured
/// 5.2e-3 and 1.62 % there (criterion missed). Wave 10 changes the wall pressure that enters the cell gradient next
/// to walls (SimpleSolver::wall_pressure_trapezoid, rotating-frame sources only) so that the wall-normal gradient of
/// the wall cell equals the cell-centre body force, and applies the SIMPLEC (rAU - rAtU) correction to the wall
/// pressure gradient; the 24 x 24 case then reaches round-off. The 48 x 48 mesh is printed for information.
#[test]
fn fluid_at_rest_in_a_rotating_frame_meets_the_original_criterion() {
    let (u24, p24) = rest_errors(24);
    let (u48, p48) = rest_errors(48);
    eprintln!(
        "rotating at rest: speed/(Omega R) {u24:.3e} (24x24), {u48:.3e} (48x48); pressure error {:.3e}% (24x24), {:.3e}% (48x48)",
        100.0 * p24,
        100.0 * p48
    );
    assert!(u24 < 1e-3, "speed/(Omega R) {u24}");
    assert!(p24 < 0.01, "pressure error {p24}");
}

#[test]
fn invalid_sources_are_diagnosed() {
    let bad = |extra: &str| {
        let errs = Case::parse_toml(&bed(extra))
            .err()
            .map(|e| e.iter().map(|d| d.path.clone()).collect::<Vec<_>>());
        errs.unwrap_or_default()
    };
    assert!(
        bad("[sources.a]\ntype = \"porous\"\nmodel = \"ergun\"\nporosity = 1.2\nparticle_diameter = 0.01\n")
            .contains(&"sources.a.porosity".to_string())
    );
    assert!(
        bad("[sources.a]\ntype = \"porous\"\nmodel = \"ergun\"\n").contains(&"sources.a.particle_diameter".to_string())
    );
    assert!(
        bad("[sources.a]\ntype = \"rotating_frame\"\nrotation_axis = [0.0, 0.0, 0.0]\nangular_velocity = 1.0\n")
            .contains(&"sources.a.rotation_axis".to_string())
    );
    assert!(bad("[sources.a]\ntype = \"porous\"\nzone = \"box\"\nbox_min = [0.0, 0.0, 0.0]\nporosity = 0.4\nparticle_diameter = 0.01\n")
        .contains(&"sources.a.box_min".to_string()));
    // unknown cell zone and empty box are reported when the simulation is built
    let case = Case::parse_toml(&bed(
        "[sources.a]\ntype = \"porous\"\nzone = \"cell_zone\"\ncell_zone = \"nothing\"\nporosity = 0.4\nparticle_diameter = 0.01\n",
    ))
    .unwrap();
    assert!(Simulation::new(&case).is_err());
    // an algorithm without a momentum-source hook is rejected (ADR 0045: piso and pimple have one since wave 25C; coupled does not)
    let text = bed("[sources.a]\ntype = \"porous\"\nporosity = 0.4\nparticle_diameter = 0.01\n").replace(
        "algorithm = \"simplec\"",
        "algorithm = \"coupled\"\ndt = 0.01\nend_time = 0.1",
    );
    let errs = match Case::parse_toml(&text) {
        Err(d) => d,
        Ok(c) => Simulation::new(&c).err().unwrap_or_default(),
    };
    assert!(errs.iter().any(|d| d.path == "sources.a.type"), "{errs:?}");
}
