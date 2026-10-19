//! Shared set-ups of the wave 35B tests (ADR 0077): the FAN-01 attempt-2 case of ADR 0075 (H-slit, first cell 2e-4 m, design point) in its
//! laminar and SST forms with a chosen momentum convection scheme. The geometry and physics are copied from `fan01_validation.rs`.

#![allow(dead_code)]

use std::f64::consts::PI;

/// Cylindrical sections of the FAN-01 blade (ADR 0075; columns as in `fan01_validation.rs`).
const SECTIONS: [[f64; 8]; 16] = [
    [124.0, 98.92, 43.63, 0.0364, 0.53, 0.1086, -23.70, -15.93],
    [127.0, 96.38, 44.81, 0.0365, 0.53, 0.1088, -22.40, -16.34],
    [130.0, 93.84, 45.99, 0.0366, 0.53, 0.1090, -21.10, -16.75],
    [140.0, 85.99, 49.94, 0.0369, 0.52, 0.1094, -17.05, -18.05],
    [150.0, 79.23, 53.26, 0.0372, 0.52, 0.1097, -13.80, -19.26],
    [160.0, 73.68, 55.89, 0.0374, 0.52, 0.1100, -11.35, -20.33],
    [170.0, 69.41, 58.20, 0.0375, 0.52, 0.1102, -9.43, -21.21],
    [180.0, 65.89, 60.21, 0.0375, 0.52, 0.1104, -7.87, -21.95],
    [190.0, 62.88, 61.97, 0.0376, 0.52, 0.1106, -6.59, -22.60],
    [200.0, 60.51, 63.57, 0.0376, 0.52, 0.1107, -5.53, -23.14],
    [210.0, 58.57, 65.04, 0.0376, 0.52, 0.1109, -4.62, -23.60],
    [220.0, 57.11, 66.39, 0.0376, 0.52, 0.1110, -3.87, -23.98],
    [230.0, 56.19, 67.67, 0.0376, 0.52, 0.1111, -3.23, -24.29],
    [240.0, 55.49, 68.83, 0.0376, 0.515, 0.1111, -2.68, -24.55],
    [245.0, 55.08, 69.35, 0.0376, 0.515, 0.1111, -2.43, -24.69],
    [247.5, 54.87, 69.60, 0.0376, 0.515, 0.1112, -2.29, -24.76],
];
pub const NU: f64 = 1.5e-5;
pub const Q_DESIGN: f64 = 1.4032;

pub fn omega() -> f64 {
    1486.0 * 2.0 * PI / 60.0
}

pub fn annulus_area() -> f64 {
    PI * (0.250f64 * 0.250 - 0.124 * 0.124)
}

fn list(f: impl Fn(&[f64; 8]) -> f64) -> String {
    let v: Vec<String> = SECTIONS.iter().map(|s| format!("{:?}", f(s))).collect();
    format!("[{}]", v.join(", "))
}

/// The `[mesh]` section of the FAN-01 attempt-2 H-slit mesh with `n_span` spanwise cells (16 registered).
pub fn mesh(n_span: usize) -> String {
    let ref_speed = (Q_DESIGN / annulus_area()).hypot(omega() * 0.186);
    format!(
        "[mesh]\ngenerator = \"axial_fan_sector\"\n[mesh.fan]\nn_blades = 9\nr_hub = 0.124\nr_tip = 0.2475\ntip_clearance = 0.0025\naxial_extent = 0.4\nz_le_root = 0.1\nfamily = \"naca4\"\n\
station_radius = {}\nstation_chord = {}\nstation_stagger = {}\nstation_camber = {}\nstation_camber_pos = {}\nstation_thickness = {}\nstation_sweep = {}\nstation_lean = {}\n\
n_up = 8\nn_chord = 12\nn_down = 8\nn_side = 10\nn_mid = 1\nn_span = {n_span}\nn_gap = 7\nn_bl = 2\ngrowth = 1.2\nfirst_cell_height = 2e-4\ny_plus = 30.0\nref_speed = {ref_speed:?}\nnu = {NU:?}\n",
        list(|s| s[0] * 1e-3),
        list(|s| s[1] * 1e-3),
        list(|s| s[2]),
        list(|s| s[3]),
        list(|s| s[4]),
        list(|s| s[5]),
        list(|s| (s[6] - SECTIONS[0][6]) * 1e-3),
        list(|s| s[0] * 1e-3 * (s[7] - SECTIONS[0][7]).to_radians()),
    )
}

/// FAN-01 attempt-2 case at the design point, laminar, in the rotating frame, with momentum convection `conv`.
pub fn laminar(conv: &str) -> String {
    laminar_on(&mesh(16), conv)
}

/// [`laminar`] on another mesh section.
pub fn laminar_on(mesh: &str, conv: &str) -> String {
    let w = omega();
    let u_in = Q_DESIGN / annulus_area();
    format!(
        "{mesh}\n[physics]\nnu = {NU:?}\nrho = 1.2\n[initial]\nvelocity = [0.0, 0.0, {u_in:?}]\nvelocity_expression_x = \"{w:?} [1/s] * y\"\nvelocity_expression_y = \"-{w:?} [1/s] * x\"\n\
[boundaries.inlet]\nvelocity = \"fixed\"\nvelocity_value = [0.0, 0.0, {u_in:?}]\nvelocity_expression_x = \"{w:?} [1/s] * y\"\nvelocity_expression_y = \"-{w:?} [1/s] * x\"\n\
[boundaries.outlet]\nvelocity = \"zero_gradient\"\npressure = \"fixed\"\n[boundaries.hub]\nvelocity = \"fixed\"\n[boundaries.blade]\nvelocity = \"fixed\"\n\
[boundaries.shroud]\nvelocity = \"fixed\"\nvelocity_expression_x = \"{w:?} [1/s] * y\"\nvelocity_expression_y = \"-{w:?} [1/s] * x\"\n\
[sources.frame]\ntype = \"rotating_frame\"\nrotation_axis = [0.0, 0.0, 1.0]\nangular_velocity = {w:?}\ncentrifugal = true\n\
[solver]\nalgorithm = \"simplec\"\nconvection = \"{conv}\"\nmax_iterations = 8000\ntolerance_u = 1e-5\ntolerance_p = 1e-5\n"
    )
}

/// [`laminar`] at rest (ADR 0077 D6): no frame source, axial inflow only, the shroud a wall at rest.
pub fn laminar_at_rest(conv: &str) -> String {
    let t = laminar(conv);
    let (a, b) = (t.find("[sources.frame]").unwrap(), t.find("[solver]").unwrap());
    let t = format!("{}{}", &t[..a], &t[b..]);
    t.lines()
        .filter(|l| !l.starts_with("velocity_expression"))
        .map(|l| format!("{l}\n"))
        .collect()
}

/// FAN-01 attempt-2 case with k-omega SST (inlet k and omega of ADR 0075, the shroud a wall for the turbulence model).
pub fn sst(conv: &str) -> String {
    let u_in = Q_DESIGN / annulus_area();
    let k = 1.5 * (0.01 * u_in) * (0.01 * u_in);
    let om = k.sqrt() / (0.09f64.powf(0.25) * 0.0176);
    laminar(conv)
        .replacen("[initial]", "[turbulence]\nmodel = \"k_omega_sst\"\n[initial]", 1)
        .replacen(
            "[boundaries.outlet]",
            &format!(
                "turbulence = \"inlet\"\nturbulence_spec = \"values\"\nk_value = {k:?}\nomega_value = {om:?}\n[boundaries.outlet]"
            ),
            1,
        )
        .replacen("[sources.frame]", "turbulence = \"wall\"\n[sources.frame]", 1)
}
