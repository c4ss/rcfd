//! Wave 35C (ADR 0078, hypothesis RE-H and check X1): `radial_equilibrium_outlet` in a rotating frame. The ADR 0022 E2 case (swirling
//! annulus, forced vortex `c_theta = 2 r`, plug axial flow 1, walls moving with the fluid, exact static pressure
//! `p = 1.25 + 2 (r^2 - 0.625)`) solved in a frame rotating at `Omega_f` about z. The static pressure is frame independent, so the E2
//! limits apply unchanged: outlet and mid-length pressure error <= 3 % of the range 1.5, outlet absolute swirl error <= 3 % of 2.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;

fn x1_case(omega_f: f64, frame_rate: Option<f64>) -> String {
    let rel = 2.0 - omega_f;
    let key = frame_rate.map_or(String::new(), |r| format!("radial_frame_rate = {r:?}\n"));
    format!(
        r#"
[mesh]
generator = "annulus_3d"
nx = 48
ny = 12
nz = 24
r_in = 0.5
r_out = 1.0
lz = 4.0
[physics]
nu = 0.1
rho = 1.0
[boundaries.inner]
velocity = "moving_wall"
velocity_value = [0.0, 0.0, 1.0]
wall_rotation_rate = {rel:?}
[boundaries.outer]
velocity = "moving_wall"
velocity_value = [0.0, 0.0, 1.0]
wall_rotation_rate = {rel:?}
[boundaries.zmin]
bc_kind = "mass_flow"
mass_flow_value = 2.3494714599609283
swirl_rate = {rel:?}
[boundaries.zmax]
bc_kind = "radial_equilibrium_outlet"
radial_reference = "area_average"
bc_pressure = 1.25
{key}[sources.frame]
type = "rotating_frame"
rotation_axis = [0.0, 0.0, 1.0]
angular_velocity = {omega_f:?}
centrifugal = true
[solver]
algorithm = "simplec"
convection = "linear_upwind"
component_diagonal = "corrected"
max_iterations = 6000
tolerance_u = 1e-12
tolerance_p = 1e-12
"#
    )
}

/// (outlet pressure error, outlet absolute swirl error, mid-length pressure error, discrete balance with the absolute swirl, settled)
/// as fractions of the E2 normalisations (range 1.5, swirl 2, range 1.5, range 1.5).
fn x1(omega_f: f64, frame_rate: Option<f64>) -> (f64, f64, f64, f64, bool) {
    let case = Case::parse_toml(&x1_case(omega_f, frame_rate)).unwrap_or_else(|e| panic!("{e:?}"));
    let mut s = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    // the E2 settle rule: every cell value of u and p changes by at most 1e-10 for 20 iterations in a row, cap 6000
    let names = ["ux", "uy", "uz", "p"];
    let mut prev: Vec<Vec<f64>> = names.iter().map(|n| s.field(n).unwrap()).collect();
    let (mut quiet, mut settled, mut its) = (0, false, 0);
    for it in 0..6000 {
        s.step();
        its = it + 1;
        assert!(s.failure().is_none(), "{:?}", s.failure());
        let cur: Vec<Vec<f64>> = names.iter().map(|n| s.field(n).unwrap()).collect();
        let change = cur
            .iter()
            .zip(&prev)
            .flat_map(|(a, b)| a.iter().zip(b).map(|(x, y)| (x - y).abs()))
            .fold(0.0_f64, |m, v| if v.is_nan() { f64::INFINITY } else { m.max(v) });
        assert!(change.is_finite(), "not finite at iteration {its}");
        quiet = if change <= 1e-10 { quiet + 1 } else { 0 };
        prev = cur;
        if quiet >= 20 {
            settled = true;
            break;
        }
    }
    let exact = |r: f64| 1.25 + 2.0 * (r * r - 0.625);
    let range = 1.5;
    let pb = s.boundary_field("p").unwrap();
    let uf = s.velocity().boundary_values(&s.geom);
    let ni = s.geom.n_internal;
    let (mut ep, mut eu) = (0.0_f64, 0.0_f64);
    let mut rows: Vec<(f64, f64, f64)> = Vec::new();
    for f in s.geom.mesh.patch("zmax").unwrap().faces() {
        let x = s.geom.mesh.face_centres()[f];
        let r = x[0].hypot(x[1]);
        let v = uf[f - ni];
        let c_theta = (-x[1] * v[0] + x[0] * v[1]) / r + omega_f * r;
        let p = pb[f - ni];
        ep = ep.max((p - exact(r)).abs());
        eu = eu.max((c_theta - 2.0 * r).abs());
        rows.push((r, p, c_theta));
    }
    let p = s.field("p").unwrap();
    let mut em = 0.0_f64;
    for (c, x) in s.geom.mesh.cell_centres().iter().enumerate() {
        if x[2] >= 1.5 && x[2] <= 2.5 {
            em = em.max((p[c] - exact(x[0].hypot(x[1]))).abs());
        }
    }
    rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut ed = 0.0_f64;
    for i in 0..rows.len() - 1 {
        let g = |q: &(f64, f64, f64)| q.2 * q.2 / q.0;
        let dp = rows[i + 1].1 - rows[i].1;
        ed = ed.max((dp - 0.5 * (g(&rows[i]) + g(&rows[i + 1])) * (rows[i + 1].0 - rows[i].0)).abs());
    }
    let out = (ep / range, eu / 2.0, em / range, ed / range, settled);
    println!(
        "X1 Omega_f {omega_f}, radial_frame_rate {frame_rate:?}: {its} iterations (settled {settled}); outlet p error {:.3} %, outlet c_theta error {:.3} %, mid-length p error {:.3} %, absolute-swirl balance {:.2e} (of the range)",
        100.0 * out.0,
        100.0 * out.1,
        100.0 * out.2,
        out.3
    );
    out
}

/// Falsifier of RE-H, run with the code as it was (no `radial_frame_rate` key): RE-H predicts outlet pressure errors of about 50 % and
/// 37.5 % of the range; RE-H is falsified if both meet the 3 % limit.
#[test]
#[ignore = "ADR 0078 X1 falsifier of RE-H (run once before the fix; known defect of the default)"]
fn x1_radial_equilibrium_in_a_rotating_frame_without_the_frame_rate_known_miss() {
    let a = x1(2.0, None);
    let b = x1(1.0, None);
    assert!(
        a.0 <= 0.03 && b.0 <= 0.03,
        "RE-H holds: {:.3} / {:.3} of the range",
        a.0,
        b.0
    );
}

fn x1_accept(omega_f: f64) {
    let (ep, eu, em, ed, settled) = x1(omega_f, Some(omega_f));
    assert!(settled, "not settled");
    assert!(ep <= 0.03, "outlet pressure {ep}");
    assert!(eu <= 0.03, "outlet swirl {eu}");
    assert!(em <= 0.03, "mid-length pressure {em}");
    println!("X1 Omega_f {omega_f}: absolute-swirl balance {ed:.2e} (reported)");
}

#[test]
fn x1_radial_equilibrium_with_the_frame_rate_omega_2() {
    x1_accept(2.0);
}

#[test]
fn x1_radial_equilibrium_with_the_frame_rate_omega_1() {
    x1_accept(1.0);
}
