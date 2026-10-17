//! Frozen-rotor interface checks R4 and R5 of docs/adr/0055-wave29a-rotor-interfaces-bcs.md (F-126). The limits were
//! registered in the ADR before any run and are repeated next to each assertion. A criterion that misses moves
//! unchanged into an ignored `*_known_miss` test.
//!
//! Set-up as in tests/mrf.rs: annulus r_i = 0.5, r_o = 1 about z, nu = 0.1, 48 cells around, SIMPLEC, linear upwind, one
//! non-orthogonal corrector; lower block (z < 0.2) in an MRF zone, upper block stationary, joined by the planar
//! non-conformal interface of F-114.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_fv::rotor_interface::{FrozenRotorInterface, RotorFrame};

const RI: f64 = 0.5;
const RO: f64 = 1.0;
const V_I: f64 = 0.5;

fn run(text: &str, iterations: usize) -> Simulation {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}\n{text}"));
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}\n{text}"));
    let n = sim.run(iterations);
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    println!(
        "  {n} iterations, converged {}, last {:?}",
        sim.is_converged(),
        sim.last()
    );
    sim
}

fn interface(sim: &Simulation, omega_a: f64, z_if: f64) -> FrozenRotorInterface {
    let cc = sim.geom.mesh.cell_centres();
    let a: Vec<bool> = cc.iter().map(|c| c[2] < z_if).collect();
    let b: Vec<bool> = a.iter().map(|x| !x).collect();
    let z = [0.0, 0.0, 1.0];
    FrozenRotorInterface::new(
        &sim.geom,
        &a,
        &b,
        RotorFrame::new([0.0; 3], z, omega_a).unwrap(),
        RotorFrame::new([0.0; 3], z, 0.0).unwrap(),
    )
    .unwrap()
}

fn lower_mrf(omega: f64, z_max: f64) -> String {
    format!(
        "[sources.rotor]\ntype = \"mrf\"\nrotation_axis = [0.0, 0.0, 1.0]\nangular_velocity = {omega:?}\nzone = \"box\"\nbox_min = [-2.0, -2.0, -1.0]\nbox_max = [2.0, 2.0, {z_max:?}]\n"
    )
}

fn through_text(omega: f64) -> String {
    format!(
        r#"
[mesh]
generator = "annulus_3d_interface"
nx = 48
ny = 8
nz = 3
ny_right = 11
nz_right = 4
r_in = {RI}
r_out = {RO}
lz = 0.8
[physics]
nu = 0.1
rho = 1.0
[boundaries.inner]
velocity = "fixed"
velocity_value = [0.0, 0.0, 1.0]
[boundaries.outer]
velocity = "fixed"
velocity_value = [0.0, 0.0, 1.0]
[boundaries.zmin]
velocity = "fixed"
velocity_value = [0.0, 0.0, 1.0]
[boundaries.zmax]
velocity = "zero_gradient"
pressure = "fixed"
[solver]
algorithm = "simplec"
convection = "linear_upwind"
non_orthogonal_correctors = 1
max_iterations = 4000
tolerance_u = 1e-10
tolerance_p = 1e-10
[reports.m_in]
type = "mass_flow"
patch = "zmin"
[reports.m_out]
type = "mass_flow"
patch = "zmax"
{}
"#,
        lower_mrf(omega, 0.4)
    )
}

#[test]
fn r4_uniform_axial_flow_across_a_stationary_rotating_pair() {
    let sim = run(&through_text(5.0), 4000);
    let (ux, uy, uz) = (
        sim.field("ux").unwrap(),
        sim.field("uy").unwrap(),
        sim.field("uz").unwrap(),
    );
    let dev = (0..ux.len())
        .map(|c| ux[c].abs().max(uy[c].abs()).max((uz[c] - 1.0).abs()))
        .fold(0.0, f64::max);
    let p = sim.field("p").unwrap();
    let range = p.iter().cloned().fold(f64::MIN, f64::max) - p.iter().cloned().fold(f64::MAX, f64::min);
    let (m_in, m_out) = (sim.monitors.last("m_in").unwrap(), sim.monitors.last("m_out").unwrap());
    let fri = interface(&sim, 5.0, 0.4);
    let phi = sim.flux();
    let abs = fri.closure(phi);
    let rel_a = fri.closure_relative_a(&sim.geom, phi);
    let flux_err = ((m_in + m_out).abs() / m_in.abs()).max((abs.net - m_in.abs()).abs() / m_in.abs());
    let frame_err = (rel_a.net - abs.net).abs() / abs.net.abs();
    println!(
        "R4: max |u - (0,0,1)| = {dev:.3e} (1e-6), pressure range {range:.3e} (1e-6), in/out + interface flux error {flux_err:.3e} (1e-9), relative-frame flux difference {frame_err:.3e} (1e-12), {} interface faces",
        fri.faces.len()
    );
    assert!(fri.faces.len() > 48 * 11);
    assert!(dev <= 1e-6, "velocity deviation {dev}");
    assert!(range <= 1e-6, "pressure range {range}");
    assert!(flux_err <= 1e-9, "flux {flux_err}");
    assert!(frame_err <= 1e-12, "relative flux {frame_err}");
}

fn tc_text(omega: f64) -> String {
    format!(
        r#"
[mesh]
generator = "annulus_3d_interface"
nx = 48
ny = 16
nz = 2
ny_right = 11
nz_right = 3
r_in = {RI}
r_out = {RO}
lz = 0.4
[physics]
nu = 0.1
rho = 1.0
[boundaries.inner]
velocity = "fixed"
velocity_expression_x = "-1.0 [m/s] * y / 1 [m]"
velocity_expression_y = "1.0 [m/s] * x / 1 [m]"
[boundaries.outer]
velocity = "fixed"
velocity_value = [0.0, 0.0, 0.0]
[boundaries.zmin]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.zmax]
velocity = "zero_gradient"
pressure = "fixed"
[solver]
algorithm = "simplec"
convection = "linear_upwind"
non_orthogonal_correctors = 1
max_iterations = 4000
tolerance_u = 1e-10
tolerance_p = 1e-10
{}
"#,
        lower_mrf(omega, 0.2)
    )
}

fn polar(sim: &Simulation) -> Vec<(f64, f64, f64, f64)> {
    let cc = sim.geom.mesh.cell_centres();
    let (ux, uy, uz) = (
        sim.field("ux").unwrap(),
        sim.field("uy").unwrap(),
        sim.field("uz").unwrap(),
    );
    (0..cc.len())
        .map(|c| {
            let (x, y) = (cc[c][0], cc[c][1]);
            let r = x.hypot(y);
            ((-y * ux[c] + x * uy[c]) / r, (x * ux[c] + y * uy[c]) / r, uz[c], r)
        })
        .collect()
}

fn tc_analytic(r: f64) -> f64 {
    let d = RO * RO - RI * RI;
    -RI * RI / d * r + RI * RI * RO * RO / d / r
}

struct R5 {
    omega: f64,
    err: f64,
    net: f64,
    abs: f64,
    field: Vec<(f64, f64, f64, f64)>,
}

fn r5_runs() -> Vec<R5> {
    let mut out = Vec::new();
    for omega in [0.0, 0.5, 1.0] {
        println!("R5: lower-zone frame Omega = {omega} W_i");
        let sim = run(&tc_text(omega), 4000);
        let field = polar(&sim);
        let err = field
            .iter()
            .map(|&(ut, _, _, r)| (ut - tc_analytic(r)).abs())
            .fold(0.0, f64::max);
        let fri = interface(&sim, omega, 0.2);
        let c = fri.closure(sim.flux());
        let (net, abs) = (c.net.abs() / (V_I * c.area), c.abs_sum / (V_I * c.area));
        println!(
            "  max |u_theta - analytic| = {:.3e} V_i (2e-2); interface net flux {net:.2e} (1e-8), sum |flux| {abs:.3e} (1e-3) in V_i A_if",
            err / V_I
        );
        out.push(R5 {
            omega,
            err,
            net,
            abs,
            field,
        });
    }
    out
}

fn max_diff(a: &[(f64, f64, f64, f64)], b: &[(f64, f64, f64, f64)]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(a, b)| (a.0 - b.0).abs().max((a.1 - b.1).abs()).max((a.2 - b.2).abs()))
        .fold(0.0, f64::max)
}

/// R5 velocity limits: each run within 2e-2 V_i of the analytic solution, the three runs within 1e-2 V_i of each other.
#[test]
fn r5_frame_invariance_across_the_interface_velocity() {
    let runs = r5_runs();
    for r in &runs {
        assert!(r.err <= 2e-2 * V_I, "Omega {}: analytic error {}", r.omega, r.err / V_I);
    }
    let (d01, d02, d12) = (
        max_diff(&runs[1].field, &runs[0].field),
        max_diff(&runs[2].field, &runs[0].field),
        max_diff(&runs[2].field, &runs[1].field),
    );
    println!(
        "R5: pairwise differences (0.5|0, 1|0, 1|0.5): {:.3e} {:.3e} {:.3e} V_i (limit 1e-2)",
        d01 / V_I,
        d02 / V_I,
        d12 / V_I
    );
    assert!(d01.max(d02).max(d12) <= 1e-2 * V_I);
}

/// R5 interface-flux limits as registered (net 1e-8, sum of absolute 1e-3, in V_i A_if). KNOWN MISS: the open ends
/// (zero-gradient velocity, fixed pressure) let a small axial flow through, as already recorded for M4b in ADR 0008
/// (3.2e-5); the registered limits copied the symmetry-end values of M5. Run with `--ignored`.
#[test]
#[ignore]
fn r5_interface_flux_known_miss() {
    for r in r5_runs() {
        assert!(r.net <= 1e-8, "Omega {}: net flux {}", r.omega, r.net);
        assert!(r.abs <= 1e-3, "Omega {}: sum |flux| {}", r.omega, r.abs);
    }
}
