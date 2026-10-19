//! Seam-flux diagnosis of ADR 0068 (0065 S3: the mass flows of `seam_lo` and `seam_hi` did not cancel, 3.5e-6 to 2.7e-5 relative).
//! The per-face pair metric e_pair = max over matched pairs (f, g) of |phi_f + phi_g| / sum |phi_seam| is measured on the converged
//! face flux. D1 (translational Couette control), D2 (circular Couette in the empty C-grid sector), D3 (D2 with the rotating-frame
//! source), D4 (the registered S3' fan passage) are the registered tests; the decision rule is in the ADR ("clean" is e_pair <= 1e-12).
//! D5 (the 0065 H-slit passage) is NOT registered: a control run added to attribute the 0065 value, reported as an observation.

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use std::f64::consts::PI;

const CLEAN: f64 = 1e-12;

#[derive(Clone, Copy)]
enum Map {
    /// lo centre + shift = hi centre
    Shift([f64; 3]),
    /// lo centre rotated by +angle about z = hi centre
    Rot(f64),
}

impl Map {
    fn apply(self, c: [f64; 3]) -> [f64; 3] {
        match self {
            Map::Shift(s) => [c[0] + s[0], c[1] + s[1], c[2] + s[2]],
            Map::Rot(a) => [a.cos() * c[0] - a.sin() * c[1], a.sin() * c[0] + a.cos() * c[1], c[2]],
        }
    }
}

/// (e_pair, patch-level (m_lo + m_hi)/|m_lo|, number of pairs) of the converged flux on the pair `lo` / `hi`.
fn pair_metric(sim: &Simulation, lo: &str, hi: &str, map: Map) -> (f64, f64, usize) {
    let mesh = &sim.geom.mesh;
    let phi = sim.flux();
    let cf = mesh.face_centres();
    let (plo, phi_p) = (mesh.patch(lo).unwrap(), mesh.patch(hi).unwrap());
    assert_eq!(plo.size, phi_p.size);
    let (mut sum_abs, mut worst, mut m_lo, mut m_hi) = (0.0, 0.0f64, 0.0, 0.0);
    let mut used = vec![false; phi_p.size];
    for f in plo.faces() {
        let t = map.apply(cf[f]);
        let (mut best, mut bd) = (phi_p.start, f64::INFINITY);
        for g in phi_p.faces() {
            let d = (0..3).map(|i| (cf[g][i] - t[i]).powi(2)).sum::<f64>().sqrt();
            if d < bd {
                bd = d;
                best = g;
            }
        }
        assert!(bd < 1e-9, "unmatched seam face {f}: {bd}");
        assert!(!used[best - phi_p.start]);
        used[best - phi_p.start] = true;
        worst = worst.max((phi[f] + phi[best]).abs());
        sum_abs += phi[f].abs() + phi[best].abs();
        m_lo += phi[f];
        m_hi += phi[best];
    }
    (worst / sum_abs, (m_lo + m_hi).abs() / m_lo.abs(), plo.size)
}

fn run(text: &str, iterations: usize) -> Simulation {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}\n{text}"));
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    let n = sim.run(iterations);
    assert!(sim.failure().is_none(), "{:?}", sim.failure());
    println!("  {n} iterations, converged {}, last {:?}", sim.is_converged(), sim.last().map(|s| (s.res_u, s.res_p)));
    sim
}

/// D1 control: plane Couette flow in a box with the translational pair xmin / xmax.
#[test]
fn d1_translational_pair_couette() {
    let text = r#"
[mesh]
generator = "box_2d"
nx = 8
ny = 10
lx = 1.0
ly = 1.0
periodic_x = true
[physics]
nu = 0.1
rho = 1.0
[boundaries.ymin]
velocity = "fixed"
[boundaries.ymax]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 3000
tolerance_u = 1e-13
tolerance_p = 1e-13
"#;
    let sim = run(text, 3000);
    let (e, e_patch, n) = pair_metric(&sim, "xmin", "xmax", Map::Shift([1.0, 0.0, 0.0]));
    println!("D1 {n} pairs: e_pair {e:.3e}, patch-level {e_patch:.3e}, converged {}", sim.is_converged());
    assert!(e <= CLEAN, "D1 e_pair {e}");
}

/// Circular Couette: u_theta = A r + B / r between the hub (omega1) and the shroud (omega2).
fn couette_ab(r1: f64, r2: f64, w1: f64, w2: f64) -> (f64, f64) {
    let a = (w2 * r2 * r2 - w1 * r1 * r1) / (r2 * r2 - r1 * r1);
    let b = (w1 - w2) * r1 * r1 * r2 * r2 / (r2 * r2 - r1 * r1);
    (a, b)
}

/// Empty rotational sector on the C-grid mesh with circular Couette walls; `frame` is the angular velocity of the
/// rotating frame (0 = stationary frame, no source).
fn couette_case(frame: f64) -> String {
    let (r1, r2, w1, w2) = (0.10, 0.31, 10.0, 20.0);
    let (a, b) = couette_ab(r1, r2, w1, w2);
    // relative-frame velocity: u_theta - frame r, i.e. omega_rel(r) = A - frame + B / r^2
    let (ar, br) = (a - frame, b);
    let (hub, shroud) = (w1 - frame, w2 - frame);
    let source = if frame != 0.0 {
        format!("[sources.frame]\ntype = \"rotating_frame\"\nrotation_axis = [0.0, 0.0, 1.0]\nangular_velocity = {frame:?}\ncentrifugal = true\n")
    } else {
        String::new()
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
velocity_expression_x = "-({ar:?} [1/s] + {br:?} [m^2/s] / (x*x + y*y)) * y"
velocity_expression_y = "({ar:?} [1/s] + {br:?} [m^2/s] / (x*x + y*y)) * x"
velocity_expression_z = "0.0 [m/s]"
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.hub]
velocity = "fixed"
velocity_expression_x = "-{hub:?} [1/s] * y"
velocity_expression_y = "{hub:?} [1/s] * x"
[boundaries.shroud]
velocity = "fixed"
velocity_expression_x = "-{shroud:?} [1/s] * y"
velocity_expression_y = "{shroud:?} [1/s] * x"
{source}
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 4000
tolerance_u = 1e-12
tolerance_p = 1e-12
"#
    )
}

fn d2_d3(frame: f64, tag: &str) -> f64 {
    let sim = run(&couette_case(frame), 4000);
    let (e, e_patch, n) = pair_metric(&sim, "seam_lo", "seam_hi", Map::Rot(2.0 * PI / 5.0));
    println!("{tag} {n} pairs: e_pair {e:.3e}, patch-level {e_patch:.3e}, converged {}", sim.is_converged());
    e
}

#[test]
fn d2_rotational_pair_circular_couette() {
    let e = d2_d3(0.0, "D2");
    assert!(e <= CLEAN, "D2 e_pair {e}");
}

#[test]
fn d3_rotational_pair_circular_couette_in_the_rotating_frame() {
    let e = d2_d3(20.0, "D3");
    assert!(e <= CLEAN, "D3 e_pair {e}");
}

/// D4: the registered S3' fan passage (small C-grid mesh S) with the default settings. The solver default is already no
/// non-orthogonal corrector (`non_orthogonal_correctors = 0`), which is the registered "correction replaced by none" variant.
#[test]
fn d4_fan_passage_c_grid() {
    let text = include_str!("../../../cases/axial_fan_cgrid/axial_fan_cgrid.toml");
    let sim = run(text, 6000);
    let (e, e_patch, n) = pair_metric(&sim, "seam_lo", "seam_hi", Map::Rot(2.0 * PI / 5.0));
    println!("D4 {n} pairs: e_pair {e:.3e}, patch-level {e_patch:.3e}, converged {}", sim.is_converged());
    assert!(e <= CLEAN, "D4 e_pair {e}");
}

/// D5 (not registered): the 0065 H-slit passage of the example case, the mesh on which the mismatch was first seen.
#[test]
fn d5_h_slit_fan_passage_observation() {
    let text = include_str!("../../../cases/axial_fan_sector/axial_fan_sector.toml");
    let sim = run(text, 6000);
    let (e, e_patch, n) = pair_metric(&sim, "seam_lo", "seam_hi", Map::Rot(2.0 * PI / 5.0));
    println!("D5 (observation) {n} pairs: e_pair {e:.3e}, patch-level {e_patch:.3e}, converged {}", sim.is_converged());
}
