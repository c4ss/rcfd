//! ADR 0765 (F-878): the FAN-01 rotor alone against the measured LDA profiles of the published record, the Euler work from them, and
//! the division of the missing pressure rise of F-870 (ADR 0762) into the rotor's and the installation's share. The measured profiles
//! are those of `lda_data.zip` of the Zenodo record doi:10.5281/zenodo.23943494 (v6, CC BY 4.0), transcribed in
//! `docs/investigations/f878-fan01-lda-profiles/lda_profiles.csv` and recorded in `docs/reference-data.d/0765-F-878.part`. The planes, the
//! circumferential average, the band at each radius, the Euler work integral and the rule of division were registered in ADR 0765 before
//! any run. The case text is that of `f870_fan01_design_point.rs` (ADR 0762), copied without a change of value. The result is in
//! ADR 0765, addendum 1.

#![allow(clippy::neg_cmp_op_on_partial_ord)] // a NaN must fail the checks

use rcfd_api::case::Case;
use rcfd_api::sim::{build_mesh, Simulation};
use rcfd_mesh::Mesh;
use std::f64::consts::PI;
use std::sync::OnceLock;
use std::time::Instant;

const N_BLADES: f64 = 9.0;
const R_HUB: f64 = 0.124;
const R_DUCT: f64 = 0.250;
const RPM: f64 = 1486.0;
const RHO: f64 = 1.2; // ASSUMED (ADR 0075, ADR 0762): the density of the measurement is in no source that was read (F-877)
const Q_DESIGN: f64 = 1.4032;
/// Measured design point (`docs/validation-data.md`, 1. FAN-01): total-to-static pressure rise [Pa] and shaft power [W].
const DP_TS_MEAS: f64 = 126.46;
const POWER_MEAS: f64 = 333.30;
/// The recorded state of ADR 0762 at 8000 iterations: (torque [N m], pressure rise [Pa]) of B1, B2, B3.
const STATE_0762: [(f64, f64); 3] = [(2.098164, 73.9881), (2.028692, 64.2329), (1.918189, 46.7776)];
/// Iteration cap of the registered run (ADR 0086, ADR 0762) and the iterations at which the profiles are taken (ADR 0765).
const CAP: usize = 8000;
const SAMPLE_AT: [usize; 5] = [7000, 7250, 7500, 7750, 8000];
/// Iterations of the fast test on the same path.
const SHORT: usize = 300;
/// Tolerance of the state, relative (ADR 0762), and its floor for a profile value [m/s], the precision of the record (ADR 0765).
const STATE_TOL: f64 = 0.01;
const PROFILE_FLOOR: f64 = 0.01;
/// Factor of safety of the mesh-convergence estimate (ADR 0086, ADR 0762) and the refinement ratio B2 to B1.
const FS: f64 = 1.25;
const CELLS_B1: f64 = 38184.0;
const CELLS_B2: f64 = 13627.0;

/// Counts (n_up, n_chord, n_down, n_side, n_nose, n_span, n_gap) of the ADR 0086 family.
type Counts = (usize, usize, usize, usize, usize, usize, usize);
const B1: Counts = (12, 24, 12, 15, 6, 21, 10);
const B2: Counts = (8, 17, 8, 11, 4, 15, 7);
const B3: Counts = (6, 12, 6, 8, 3, 10, 5);

// ---------------------------------------------------------------- the sampling registered in ADR 0765

/// Points per pitch on a ring, the pitch of the nine-bladed rotor, and the largest distance from a cell's face planes at which a
/// point still counts as in the fluid [m].
const N_THETA: usize = 360;
const PITCH: f64 = 2.0 * PI / 9.0;
const FLUID_TOL: f64 = 1.0e-4;
/// `z_mesh = 0.1 m + (x - 21.45 mm)`: the frame of the record (x from the upstream hub face) in the mesh (ADR 0765: offset 45.15 mm
/// between the frame of the CAD sections and the record, the root leading edge at `le_x = -23.7 mm` and z = 0.1 m).
const X_ROOT_LE_MM: f64 = 21.45;
/// The measured planes [mm], and the plane behind the rotor moved by the registered +-1.3 mm.
const X_UP: f64 = 20.0;
const X_DOWN: f64 = 90.0;
const X_DOWN_LO: f64 = 88.7;
const X_DOWN_HI: f64 = 91.3;

// ---------------------------------------------------------------- the measured profiles (Zenodo doi:10.5281/zenodo.23943494, lda_data.zip)

/// The transcription the constants below are checked against (test `f878_lda_reference_and_measured_integrals`).
const LDA_CSV: &str = include_str!("../../../docs/investigations/f878-fan01-lda-profiles/lda_profiles.csv");

// `radius_norm` = r / 250 mm; `UX`, `UPHI`, `UR`: the time-averaged files as stored [m/s]; `ENS_*`: derived from the ensemble-averaged
// files, the bin mean of ux [m/s], of ux uphi [m^2/s^2] and of ux (ux^2 + uphi^2 + ur^2) / 2 [m^3/s^3] over the 180 bins.
const RADIUS_NORM: [f64; 26] = [
    0.532, 0.552, 0.572, 0.592, 0.612, 0.632, 0.652, 0.672, 0.692, 0.712, 0.732, 0.752, 0.772, 0.792, 0.812, 0.832,
    0.852, 0.872, 0.892, 0.912, 0.932, 0.952, 0.960, 0.968, 0.976, 0.984,
];
const LDA20_UX: [f64; 26] = [
    11.95, 11.62, 11.30, 11.11, 10.90, 10.76, 10.64, 10.52, 10.46, 10.39, 10.31, 10.25, 10.16, 10.09, 9.97, 9.82, 9.66,
    9.44, 9.19, 8.85, 8.50, 8.16, 7.96, 7.67, 7.25, 6.53,
];
const LDA20_UPHI: [f64; 26] = [
    0.39, 0.36, 0.28, 0.23, 0.14, 0.10, 0.04, -0.08, -0.18, -0.29, -0.36, -0.44, -0.52, -0.64, -0.76, -0.87, -1.03,
    -1.14, -1.33, -1.50, -1.71, -1.87, -1.99, -2.02, -2.05, -2.04,
];
const LDA20_UR: [f64; 26] = [
    -0.23, 0.06, 0.69, 0.85, 0.81, 0.77, 0.69, 0.60, 0.49, 0.37, 0.24, 0.11, -0.02, -0.21, -0.35, -0.50, -0.65, -0.79,
    -0.92, -1.04, -1.11, -1.05, -0.95, -0.86, -0.76, -0.59,
];
const LDA20_ENS_UX: [f64; 26] = [
    11.624722, 11.351222, 11.081778, 10.889000, 10.708167, 10.571500, 10.448278, 10.336111, 10.277167, 10.218833,
    10.140889, 10.081000, 9.994833, 9.925778, 9.808944, 9.671667, 9.506167, 9.300944, 9.054167, 8.729333, 8.393222,
    8.072056, 7.877500, 7.585111, 7.162778, 6.440611,
];
const LDA20_ENS_UX_UPHI: [f64; 26] = [
    5.233053, 4.681119, 3.662512, 2.986147, 1.980390, 1.431365, 0.801871, -0.409302, -1.388689, -2.473078, -3.227176,
    -3.942399, -4.673169, -5.863503, -6.979829, -7.894618, -9.249345, -10.150370, -11.589849, -12.684496, -14.010647,
    -14.743842, -15.320929, -15.018382, -14.366494, -12.852019,
];
const LDA20_ENS_KE_FLUX: [f64; 26] = [
    893.025230, 820.072004, 756.009359, 712.878154, 675.073822, 648.642344, 623.428546, 601.852117, 591.041246,
    580.202212, 566.631130, 556.502426, 541.880460, 530.554120, 513.431678, 493.740943, 472.267890, 444.895396,
    414.056975, 375.205255, 335.404271, 299.779321, 281.763240, 253.374325, 215.107321, 160.669957,
];
const LDA90_UX: [f64; 26] = [
    10.11, 10.66, 10.88, 10.91, 10.98, 11.13, 11.23, 11.25, 11.36, 11.47, 11.56, 11.62, 11.72, 11.83, 11.96, 12.02,
    11.94, 11.54, 10.37, 8.42, 5.97, 4.43, 4.06, 3.88, 3.87, 3.57,
];
const LDA90_UPHI: [f64; 26] = [
    9.77, 8.91, 8.56, 8.25, 7.94, 7.60, 7.31, 7.13, 6.76, 6.48, 6.21, 5.94, 5.64, 5.31, 4.99, 4.70, 4.55, 4.52, 5.02,
    5.93, 6.96, 7.99, 8.27, 8.64, 8.78, 8.43,
];
const LDA90_UR: [f64; 26] = [
    -0.47, -0.54, 0.22, 0.20, 0.21, 0.24, 0.35, 0.35, 0.37, 0.42, 0.43, 0.44, 0.43, 0.41, 0.45, 0.47, 0.49, 0.62, 0.92,
    1.26, 1.58, 1.56, 1.41, 1.28, 1.07, 0.82,
];
const LDA90_ENS_UX: [f64; 26] = [
    10.090778, 10.668333, 10.883611, 10.919444, 10.991389, 11.120222, 11.232889, 11.269778, 11.363333, 11.485278,
    11.569833, 11.634167, 11.733778, 11.845278, 11.976556, 12.037889, 11.960667, 11.561833, 10.361444, 8.390889,
    5.943000, 4.407389, 4.037389, 3.868056, 3.861167, 3.571944,
];
const LDA90_ENS_UX_UPHI: [f64; 26] = [
    96.983844, 93.668734, 92.118429, 89.090648, 86.360687, 84.027458, 81.367271, 79.454391, 76.383599, 73.817592,
    71.334370, 68.720016, 65.695104, 62.625122, 59.575517, 56.768301, 54.931358, 53.183778, 52.535174, 49.284873,
    40.547028, 34.736037, 33.072778, 33.240724, 33.780688, 30.091648,
];
const LDA90_ENS_KE_FLUX: [f64; 26] = [
    1128.796962,
    1179.837600,
    1048.581865,
    1028.809160,
    1018.749895,
    1020.017310,
    1018.200813,
    1010.509688,
    1004.295468,
    1008.660175,
    1007.804729,
    1003.601621,
    1004.689382,
    1008.959789,
    1019.060804,
    1017.703643,
    994.167610,
    910.399760,
    710.265917,
    465.605131,
    264.296117,
    192.208647,
    178.056780,
    179.470677,
    183.040241,
    155.150068,
];

fn omega() -> f64 {
    RPM * 2.0 * PI / 60.0
}

fn annulus_area() -> f64 {
    PI * (R_DUCT * R_DUCT - R_HUB * R_HUB)
}

/// The mesh coordinate z [m] of the plane at `x_mm` of the record.
fn z_of(x_mm: f64) -> f64 {
    0.1 + (x_mm - X_ROOT_LE_MM) * 1e-3
}

/// The 26 measured radii [m].
fn radii() -> Vec<f64> {
    RADIUS_NORM.iter().map(|v| v * R_DUCT).collect()
}

/// The 126 rings of 1 mm of the whole annulus [m] (midpoints).
fn annulus_radii() -> Vec<f64> {
    (0..126).map(|i| R_HUB + 1e-3 * (i as f64 + 0.5)).collect()
}

/// The 16 registered FAN-01 sections (ADR 0075 amendment 1).
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

/// The mesh section of `fan01_gci_w37c.rs` (ADR 0086), bladed passage, construction `camber_seam_h`; no value changed.
fn mesh_toml(c: Counts) -> String {
    let rows = sections();
    let (x0, t0) = (rows[0][6], rows[0][7]);
    let ref_speed = (Q_DESIGN / annulus_area()).hypot(omega() * 0.186);
    let (n_up, n_chord, n_down, n_side, n_nose, n_span, n_gap) = c;
    format!(
        "[mesh]\ngenerator = \"axial_fan_sector\"\n[mesh.fan]\nn_blades = 9\nr_hub = 0.124\nr_tip = 0.2475\ntip_clearance = 0.0025\naxial_extent = 0.4\nz_le_root = 0.1\nfamily = \"naca4\"\n\
station_radius = {}\nstation_chord = {}\nstation_stagger = {}\nstation_camber = {}\nstation_camber_pos = {}\nstation_thickness = {}\nstation_sweep = {}\nstation_lean = {}\n\
topology = \"c_grid\"\nc_grid_construction = \"camber_seam_h\"\nn_up = {n_up}\nn_chord = {n_chord}\nn_down = {n_down}\nn_side = {n_side}\nn_nose = {n_nose}\nn_core = 1\nfront_gap = 0.3\nn_span = {n_span}\nn_gap = {n_gap}\nn_bl = 2\ngrowth = 1.2\n\
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

/// The bladed case of `fan01_gci_w37c.rs` (ADR 0086): design point, `rotating_frame`, k-omega SST, SIMPLEC, `linear_upwind` with the
/// Barth-Jespersen limiter, tolerances 1e-5. `cap` is `solver.max_iterations`.
fn case_toml(c: Counts, cap: usize) -> String {
    let w = omega();
    let u_in = Q_DESIGN / annulus_area();
    let k = 1.5 * (0.01 * u_in) * (0.01 * u_in);
    let om = k.sqrt() / (0.09f64.powf(0.25) * 0.0176);
    let casing = format!(
        "velocity = \"moving_wall\"\nwall_rotation_rate = {:?}\nwall_velocity_projection = \"tangential\"\nturbulence = \"wall\"\n",
        -w
    );
    let rel = format!("velocity_expression_x = \"{w:?} [1/s] * y\"\nvelocity_expression_y = \"-{w:?} [1/s] * x\"\n");
    format!(
        r#"{}
[physics]
nu = 1.5e-5
rho = {RHO:?}
[turbulence]
model = "k_omega_sst"
[initial]
velocity = [0.0, 0.0, {u_in:?}]
{rel}[boundaries.inlet]
velocity = "fixed"
velocity_value = [0.0, 0.0, {u_in:?}]
{rel}turbulence = "inlet"
turbulence_spec = "values"
k_value = {k:?}
omega_value = {om:?}
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
[boundaries.hub]
velocity = "fixed"
[boundaries.shroud]
{casing}[sources.frame]
type = "rotating_frame"
rotation_axis = [0.0, 0.0, 1.0]
angular_velocity = {w:?}
centrifugal = true
[boundaries.blade]
velocity = "fixed"
[reports.m_blade]
type = "moment"
patch = "blade"
direction = [0.0, 0.0, 1.0]
[solver]
algorithm = "simplec"
convection = "linear_upwind"
linear_upwind_limiter = "barth_jespersen"
max_iterations = {cap}
tolerance_u = 1e-5
tolerance_p = 1e-5
[reports.m_in]
type = "mass_flow"
patch = "inlet"
[reports.m_out]
type = "mass_flow"
patch = "outlet"
[reports.p_in]
type = "area_average"
patch = "inlet"
field = "p"
[reports.p_out]
type = "area_average"
patch = "outlet"
field = "p"
[reports.m_hub]
type = "moment"
patch = "hub"
direction = [0.0, 0.0, 1.0]
"#,
        mesh_toml(c)
    )
}

/// Torque and pressure rise with the ADR 0075 definitions (as `f870_fan01_design_point.rs`).
fn functionals(sim: &Simulation) -> (f64, f64) {
    let u_in = Q_DESIGN / annulus_area();
    let mon = |k: &str| sim.monitors.last(k).unwrap_or(f64::NAN);
    (
        -N_BLADES * (mon("m_blade") + mon("m_hub")),
        RHO * (mon("p_out") - (mon("p_in") + 0.5 * u_in * u_in)),
    )
}

/// Per cell: its faces as (centre, outward unit normal), and bounds of the radius and of z over the convex hull of its points. The
/// lower bound of the radius is the least radius of a point times the cosine of half the angle the points span about the axis: a chord
/// between two points of one radius comes nearer to the axis than its ends.
struct CellGeo {
    faces: Vec<([f64; 3], [f64; 3])>,
    r: (f64, f64),
    z: (f64, f64),
}

/// Locates points of a ring in the sector mesh (ADR 0765, "The circumferential average of the solve").
struct Locator {
    cells: Vec<CellGeo>,
    centres: Vec<[f64; 3]>,
}

impl Locator {
    fn new(mesh: &Mesh) -> Self {
        let (cf, sf, own, pts) = (
            mesh.face_centres(),
            mesh.face_area_vectors(),
            mesh.owner(),
            mesh.points(),
        );
        let cells = (0..mesh.n_cells())
            .map(|c| {
                let mut g = CellGeo {
                    faces: Vec::new(),
                    r: (f64::INFINITY, f64::NEG_INFINITY),
                    z: (f64::INFINITY, f64::NEG_INFINITY),
                };
                let xc = mesh.cell_centres()[c];
                let mut half_angle = 0.0f64;
                for &f in mesh.cell_faces(c) {
                    let s = sf[f];
                    let a = (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt();
                    let sign = if own[f] == c { 1.0 } else { -1.0 };
                    g.faces
                        .push((cf[f], [sign * s[0] / a, sign * s[1] / a, sign * s[2] / a]));
                    for &p in mesh.face_points(f) {
                        let x = pts[p];
                        let r = x[0].hypot(x[1]);
                        g.r = (g.r.0.min(r), g.r.1.max(r));
                        g.z = (g.z.0.min(x[2]), g.z.1.max(x[2]));
                        // the angle about the axis between the point and the cell centre
                        let (cross, dot) = (xc[0] * x[1] - xc[1] * x[0], xc[0] * x[0] + xc[1] * x[1]);
                        half_angle = half_angle.max(cross.atan2(dot).abs());
                    }
                }
                g.r.0 *= half_angle.cos();
                g
            })
            .collect();
        Locator {
            cells,
            centres: mesh.cell_centres().to_vec(),
        }
    }

    /// For each of the `N_THETA` points of the ring (r, z): the cell that holds it, or `None` if the point is not in the fluid. A point
    /// is turned by each of the nine multiples of the pitch; the cell is the one with the smallest largest signed distance of the point
    /// from the planes of its faces, and the point is in the fluid iff that distance is at most `FLUID_TOL`.
    fn ring(&self, r: f64, z: f64) -> Vec<Option<usize>> {
        // a cell whose points do not reach the ring within 1 mm in radius and in z cannot hold a point of it
        let m = 1.0e-3;
        let cand: Vec<usize> = (0..self.cells.len())
            .filter(|&c| {
                let g = &self.cells[c];
                r >= g.r.0 - m && r <= g.r.1 + m && z >= g.z.0 - m && z <= g.z.1 + m
            })
            .collect();
        (0..N_THETA)
            .map(|k| {
                let theta = (k as f64 + 0.5) * PITCH / N_THETA as f64;
                let mut best = (f64::INFINITY, usize::MAX);
                for j in 0..9 {
                    let t = theta + j as f64 * PITCH;
                    let p = [r * t.cos(), r * t.sin(), z];
                    for &c in &cand {
                        let d = self.cells[c]
                            .faces
                            .iter()
                            .map(|(x, n)| (p[0] - x[0]) * n[0] + (p[1] - x[1]) * n[1] + (p[2] - x[2]) * n[2])
                            .fold(f64::NEG_INFINITY, f64::max);
                        if d < best.0 {
                            best = (d, c);
                        }
                    }
                }
                (best.0 <= FLUID_TOL).then_some(best.1)
            })
            .collect()
    }

    /// The rings of the radii `r` on the plane at `x_mm`.
    fn plane(&self, x_mm: f64, r: &[f64]) -> Vec<Vec<Option<usize>>> {
        r.iter().map(|&ri| self.ring(ri, z_of(x_mm))).collect()
    }
}

/// Circumferential means on one ring, in the frame at rest: the velocity components [m/s], the mean of `c_x c_t`, of
/// `c_x (c_x^2 + c_t^2 + c_r^2) / 2` and of `p c_x` (kinematic pressure), and the share of the points that are in the fluid.
#[derive(Clone, Copy, Debug, Default)]
struct Ring {
    r: f64,
    fluid: f64,
    cx: f64,
    ct: f64,
    cr: f64,
    cxct: f64,
    ke: f64,
    pcx: f64,
}

/// The circumferential mean over the points of a ring that are in the fluid. `w` is the relative velocity of the solve; the value at a
/// point is the absolute velocity of its cell, `w + omega e_z x x_c`, in cylindrical components formed at the cell centre (ADR 0765,
/// amendment 1).
fn ring_mean(loc: &Locator, cells: &[Option<usize>], r: f64, w: &[[f64; 3]], p: &[f64]) -> Ring {
    let mut m = Ring { r, ..Ring::default() };
    let mut n = 0.0;
    for &c in cells.iter().flatten() {
        let x = loc.centres[c];
        let rc = x[0].hypot(x[1]);
        let (wr, wt) = (
            (x[0] * w[c][0] + x[1] * w[c][1]) / rc,
            (-x[1] * w[c][0] + x[0] * w[c][1]) / rc,
        );
        let (cx, ct, cr) = (w[c][2], wt + omega() * rc, wr);
        n += 1.0;
        m.cx += cx;
        m.ct += ct;
        m.cr += cr;
        m.cxct += cx * ct;
        m.ke += 0.5 * cx * (cx * cx + ct * ct + cr * cr);
        m.pcx += p[c] * cx;
    }
    if n > 0.0 {
        for v in [&mut m.cx, &mut m.ct, &mut m.cr, &mut m.cxct, &mut m.ke, &mut m.pcx] {
            *v /= n;
        }
    }
    m.fluid = n / cells.len() as f64;
    m
}

fn plane_means(loc: &Locator, plane: &[Vec<Option<usize>>], r: &[f64], sim: &Simulation) -> Vec<Ring> {
    let (w, p) = (&sim.velocity().values, &sim.pressure().values);
    plane
        .iter()
        .zip(r)
        .map(|(cells, &ri)| ring_mean(loc, cells, ri, w, p))
        .collect()
}

/// `int f 2 pi r dr` by the trapezoid rule on the radii `r`.
fn trapezoid(r: &[f64], f: &[f64]) -> f64 {
    (0..r.len() - 1)
        .map(|i| 0.5 * (2.0 * PI * r[i] * f[i] + 2.0 * PI * r[i + 1] * f[i + 1]) * (r[i + 1] - r[i]))
        .sum()
}

/// `int f 2 pi r dr` by the midpoint rule on rings of 1 mm.
fn midpoint(r: &[f64], f: &[f64]) -> f64 {
    r.iter().zip(f).map(|(ri, fi)| 2.0 * PI * ri * fi * 1e-3).sum()
}

/// The integrals of ADR 0765 over a span: the volume flow `Q_c` [m^3/s], the Euler power `P_E` [W] and head `e` [Pa], and the
/// kinetic-energy flux per volume flow `q` [Pa].
#[derive(Clone, Copy, Debug)]
struct Head {
    flow: f64,
    power: f64,
    e: f64,
    q: f64,
}

/// From per-radius means of `c_x`, of `c_x c_t` and of `c_x |c|^2 / 2`, with the rule `int` for `int f 2 pi r dr`.
fn head(r: &[f64], cx: &[f64], cxct: &[f64], ke: &[f64], int: fn(&[f64], &[f64]) -> f64) -> Head {
    let flow = int(r, cx);
    let arm: Vec<f64> = r.iter().zip(cxct).map(|(ri, f)| ri * f).collect();
    let power = RHO * omega() * int(r, &arm);
    Head {
        flow,
        power,
        e: power / flow,
        q: RHO * int(r, ke) / flow,
    }
}

/// The two readings of ADR 0765 for the measurement at the plane `x`: (products of the time means, bin means of the products).
fn measured(x: f64) -> (Head, Head) {
    let r = radii();
    let (ux, ut, ur, eux, euxut, eke) = if x == X_DOWN {
        (
            &LDA90_UX,
            &LDA90_UPHI,
            &LDA90_UR,
            &LDA90_ENS_UX,
            &LDA90_ENS_UX_UPHI,
            &LDA90_ENS_KE_FLUX,
        )
    } else {
        (
            &LDA20_UX,
            &LDA20_UPHI,
            &LDA20_UR,
            &LDA20_ENS_UX,
            &LDA20_ENS_UX_UPHI,
            &LDA20_ENS_KE_FLUX,
        )
    };
    let prod: Vec<f64> = (0..26).map(|i| ux[i] * ut[i]).collect();
    let ke: Vec<f64> = (0..26)
        .map(|i| 0.5 * ux[i] * (ux[i] * ux[i] + ut[i] * ut[i] + ur[i] * ur[i]))
        .collect();
    (
        head(&r, ux, &prod, &ke, trapezoid),
        head(&r, eux, euxut, eke, trapezoid),
    )
}

/// The same two readings for rings of the solve (profile, pointwise), and `Pi_s` [Pa]: the flux-weighted static pressure of the span
/// against the inlet total pressure of ADR 0762's pressure rise. `weighted` (the whole annulus): a ring counts with its share of points in
/// the fluid, so points in the blade carry zero flux.
fn solved(rings: &[Ring], p_in: f64, int: fn(&[f64], &[f64]) -> f64, weighted: bool) -> (Head, Head, f64) {
    let r: Vec<f64> = rings.iter().map(|m| m.r).collect();
    let col = |f: fn(&Ring) -> f64| -> Vec<f64> {
        rings
            .iter()
            .map(|m| if weighted { m.fluid * f(m) } else { f(m) })
            .collect()
    };
    let cx = col(|m| m.cx);
    let prof = head(
        &r,
        &cx,
        &col(|m| m.cx * m.ct),
        &col(|m| 0.5 * m.cx * (m.cx * m.cx + m.ct * m.ct + m.cr * m.cr)),
        int,
    );
    let point = head(&r, &cx, &col(|m| m.cxct), &col(|m| m.ke), int);
    let u_in = Q_DESIGN / annulus_area();
    let pi = RHO * (int(&r, &col(|m| m.pcx)) / point.flow - p_in - 0.5 * u_in * u_in);
    (prof, point, pi)
}

/// `1.25 abs(f_B2 - f_B1) / (r_21 - 1)`: the mesh-convergence estimate of ADR 0762 with the order at its cap of 1 (ADR 0765).
fn u_num(f_b1: f64, f_b2: f64) -> f64 {
    FS * (f_b2 - f_b1).abs() / ((CELLS_B1 / CELLS_B2).cbrt() - 1.0)
}

/// The verdict of the rule of ADR 0765.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Verdict {
    Installation,
    Rotor,
    CannotBeTold,
}

/// The division of the missing pressure rise `D` (ADR 0765). All in Pa.
#[derive(Clone, Copy, Debug)]
struct Division {
    d: f64,
    /// bounds of the rotor's share, for a measured rotor that loses all it can and one that loses nothing, and their bands
    r_lo: f64,
    r_hi: f64,
    u_lo: f64,
    u_hi: f64,
    verdict: Verdict,
    /// the conditional division: the rotor's share if the measured rotor loses what the simulated one does, and its band
    r_eq: f64,
    u_eq: f64,
}

/// `pi_s` and `eq_s = e_s - q_s` of B1 at 8000 iterations with the sum of their registered band terms of the solve (`U_num + S + T +
/// u_cov`); the terms of the measurement are formed here from the two readings.
fn division(pi_s: f64, u_pi: f64, eq_s: f64, u_eq_s: f64) -> Division {
    let (prof, point) = measured(X_DOWN);
    let d = DP_TS_MEAS - STATE_0762[0].1;
    let (r_lo, r_hi) = (DP_TS_MEAS - point.q - pi_s, point.e - point.q - pi_s);
    let u_data_lo = (prof.q - point.q).abs();
    let u_data_hi = ((prof.e - prof.q) - (point.e - point.q)).abs();
    let (u_lo, u_hi) = (u_pi + u_data_lo, u_pi + u_data_hi);
    let verdict = if r_hi + u_hi < 0.5 * d {
        Verdict::Installation
    } else if r_lo - u_lo > 0.5 * d {
        Verdict::Rotor
    } else {
        Verdict::CannotBeTold
    };
    Division {
        d,
        r_lo,
        r_hi,
        u_lo,
        u_hi,
        verdict,
        r_eq: (point.e - point.q) - eq_s,
        u_eq: u_eq_s + u_data_hi,
    }
}

/// The profiles of one iteration: the rings of the measured radii on the four planes (x = 20, 88.7, 90, 91.3 mm), the inlet pressure
/// monitor and the functionals.
#[derive(Clone, Debug)]
struct Sample {
    iteration: usize,
    planes: Vec<Vec<Ring>>,
    p_in: f64,
    torque: f64,
    dp_ts: f64,
}

const PLANES: [f64; 4] = [X_UP, X_DOWN_LO, X_DOWN, X_DOWN_HI];

impl Sample {
    fn plane(&self, x: f64) -> &[Ring] {
        &self.planes[PLANES.iter().position(|&p| p == x).unwrap()]
    }

    /// (`e_s`, `q_s`, `Pi_s`, `Q_c`) of the pointwise reading over the measured span at the plane `x`.
    fn scalars(&self, x: f64) -> [f64; 4] {
        let (_, point, pi) = solved(self.plane(x), self.p_in, trapezoid, false);
        [point.e, point.q, pi, point.flow]
    }
}

#[derive(Clone, Debug)]
struct Outcome {
    cells: usize,
    iterations: usize,
    minutes: f64,
    torque: f64,
    dp_ts: f64,
    res_u: f64,
    res_p: f64,
    stop: Option<String>,
    samples: Vec<Sample>,
    /// the rings of the whole annulus at x = 20 and x = 90 mm at the last iteration
    annulus: [Vec<Ring>; 2],
    p_in: f64,
}

/// The one path of every solving test of this file: the design point of ADR 0762 on the mesh `c` for at most `cap` iterations, the
/// profiles taken at the iterations `sample_at`. No wall-clock stop; the stops are those of ADR 0762.
fn run_design_point(label: &str, c: Counts, cap: usize, sample_at: &[usize]) -> Outcome {
    let case = Case::parse_toml(&case_toml(c, cap)).unwrap_or_else(|e| panic!("{label}: {e:?}"));
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{label}: {e:?}"));
    let cells = sim.geom.mesh.n_cells();
    let loc = Locator::new(&sim.geom.mesh);
    let (r, ra) = (radii(), annulus_radii());
    let planes: Vec<Vec<Vec<Option<usize>>>> = PLANES.iter().map(|&x| loc.plane(x, &r)).collect();
    for (x, plane) in PLANES.iter().zip(&planes) {
        let out: Vec<usize> = plane
            .iter()
            .map(|ring| ring.iter().filter(|c| c.is_none()).count())
            .collect();
        println!("F878 {label} points not in the fluid at x = {x} mm, per measured radius: {out:?}");
    }
    let t0 = Instant::now();
    let (mut n, mut stop) = (0usize, None);
    let (mut res_u, mut res_p) = (f64::NAN, f64::NAN);
    let mut samples = Vec::new();
    while n < cap && !sim.is_done() {
        let st = sim.step();
        n += 1;
        (res_u, res_p) = (st.res_u, st.res_p);
        if n % 500 == 0 || n <= 3 || n == 10 || n == 30 || n == 100 {
            let (a, b) = functionals(&sim);
            println!(
                "F878 {label} it {n}: continuity {:.3e} res_u {:.3e} res_p {:.3e} torque {a:.6} dp_ts {b:.4} ({:.1} min)",
                st.continuity,
                st.res_u,
                st.res_p,
                t0.elapsed().as_secs_f64() / 60.0
            );
        }
        if let Some(d) = sim.steady_divergence() {
            stop = Some(format!("steady guard trip at {n}: {} {}", d.kind, d.quantity));
            break;
        }
        if !(st.continuity.is_finite() && st.res_u.is_finite() && st.res_p.is_finite()) || st.continuity > 1e6 {
            stop = Some(format!("diverged: continuity {:.3e} at iteration {n}", st.continuity));
            break;
        }
        if sample_at.contains(&n) {
            let (torque, dp_ts) = functionals(&sim);
            samples.push(Sample {
                iteration: n,
                planes: planes.iter().map(|pl| plane_means(&loc, pl, &r, &sim)).collect(),
                p_in: sim.monitors.last("p_in").unwrap_or(f64::NAN),
                torque,
                dp_ts,
            });
        }
    }
    if stop.is_none() {
        stop = sim.failure().map(|f| format!("failure after {n} iterations: {f}"));
    }
    let (torque, dp_ts) = functionals(&sim);
    let annulus = [X_UP, X_DOWN].map(|x| plane_means(&loc, &loc.plane(x, &ra), &ra, &sim));
    let o = Outcome {
        cells,
        iterations: n,
        minutes: t0.elapsed().as_secs_f64() / 60.0,
        torque,
        dp_ts,
        res_u,
        res_p,
        stop,
        samples,
        annulus,
        p_in: sim.monitors.last("p_in").unwrap_or(f64::NAN),
    };
    println!(
        "F878 {label} RESULT: {cells} cells, {n} iterations, {:.1} min; torque {torque:.6} N m, shaft power {:.3} W, dp_ts {dp_ts:.4} Pa; \
res_u {:.3e} res_p {:.3e}; is_converged {}; stop {:?}",
        o.minutes,
        torque * omega(),
        o.res_u,
        o.res_p,
        o.stop.is_none() && sim.is_converged(),
        o.stop
    );
    o
}

/// Prints every sampled ring and scalar of a run, one record per line, for `analyse.py` of the investigation folder.
fn print_profiles(label: &str, o: &Outcome) {
    for s in &o.samples {
        println!(
            "F878S {label} {} p_in {:.9e} torque {:.9e} dp_ts {:.9e}",
            s.iteration, s.p_in, s.torque, s.dp_ts
        );
        for (x, rings) in PLANES.iter().zip(&s.planes) {
            for m in rings {
                println!(
                    "F878P {label} {} {x} {:.6} {:.6} {:.9e} {:.9e} {:.9e} {:.9e} {:.9e} {:.9e}",
                    s.iteration, m.r, m.fluid, m.cx, m.ct, m.cr, m.cxct, m.ke, m.pcx
                );
            }
            let (prof, point, pi) = solved(rings, s.p_in, trapezoid, false);
            println!(
                "F878H {label} {} {x} span flow {:.9e} power_profile {:.9e} power_pointwise {:.9e} e_profile {:.9e} e_pointwise {:.9e} \
q_profile {:.9e} q_pointwise {:.9e} pi {:.9e}",
                s.iteration, point.flow, prof.power, point.power, prof.e, point.e, prof.q, point.q, pi
            );
        }
    }
    for (x, rings) in [X_UP, X_DOWN].iter().zip(&o.annulus) {
        for m in rings {
            println!(
                "F878A {label} {} {x} {:.6} {:.6} {:.9e} {:.9e} {:.9e} {:.9e} {:.9e} {:.9e}",
                o.iterations, m.r, m.fluid, m.cx, m.ct, m.cr, m.cxct, m.ke, m.pcx
            );
        }
        let (prof, point, pi) = solved(rings, o.p_in, midpoint, true);
        println!(
            "F878H {label} {} {x} annulus flow {:.9e} power_profile {:.9e} power_pointwise {:.9e} e_profile {:.9e} e_pointwise {:.9e} \
q_profile {:.9e} q_pointwise {:.9e} pi {:.9e}",
            o.iterations, point.flow, prof.power, point.power, prof.e, point.e, prof.q, point.q, pi
        );
    }
}

/// `a` within the registered tolerance of the state of the recorded value `b`.
fn holds(a: f64, b: f64) -> bool {
    (a - b).abs() <= STATE_TOL * b.abs()
}

/// The same for a profile value [m/s], with the floor of the record's precision.
fn holds_profile(a: f64, b: f64) -> bool {
    (a - b).abs() <= (STATE_TOL * b.abs()).max(PROFILE_FLOOR)
}

// ---------------------------------------------------------------- the recorded state (ADR 0765, addendum 1)

/// B3 at iteration 300, x = 90 mm: (`e_s` [Pa], `q_s` [Pa], `Pi_s` [Pa], `Q_c` [m^3/s]) of the first run (S1).
const B3_SHORT_RECORD: [f64; 4] = [240.0486, 96.4801, 39.3569, 1.339620];
/// B1 and B2 at 8000 iterations, x = 90 mm, the measured span, pointwise reading: (`e_s`, `q_s`, `Pi_s`, `Q_c`).
const B1_SCALARS: [f64; 4] = [226.861468, 102.279724, 61.709396, 1.375399];
const B2_SCALARS: [f64; 4] = [231.526440, 98.038657, 54.732823, 1.331399];
/// The band terms of the solve for `Pi_s` and for `e_s - q_s` [Pa]: the spread over the cycle `S`, the place of the plane `T`, the
/// coverage `u_cov` (`U_num` is formed from the two lines above).
const S_T_COV_PI: [f64; 3] = [0.000804, 0.008265, 0.393920];
const S_T_COV_EQ: [f64; 3] = [0.000497, 0.333469, 0.333564];
/// Circumferential means at x = 90 mm at the 26 measured radii [m/s]: B1 and B2 at 8000 iterations, and for B1 the spread over the
/// cycle `S` and over the place of the plane `T`.
const B1_CX: [f64; 26] = [
    11.268921, 11.344717, 11.445796, 11.407395, 11.231501, 11.245872, 10.885398, 10.923642, 10.558101, 10.588224,
    10.534551, 10.550688, 10.743585, 10.754598, 11.178851, 11.159286, 11.607318, 11.582900, 11.579082, 11.203062,
    11.192518, 6.818270, 6.813046, 1.659118, -0.253715, -0.974646,
];
const B1_CT: [f64; 26] = [
    8.156290, 7.809535, 7.669032, 7.656802, 7.433324, 7.409061, 7.268605, 7.224084, 7.124392, 7.073785, 6.660781,
    6.643203, 6.048415, 6.028036, 5.432015, 5.429048, 5.109094, 5.110004, 5.099351, 6.157801, 6.205307, 9.801844,
    9.810213, 10.546999, 10.133139, 9.796893,
];
const B2_CX: [f64; 26] = [
    10.981366, 11.064599, 11.212121, 10.985807, 11.037878, 11.082727, 10.534027, 10.548728, 10.545481, 10.480534,
    10.480756, 10.487291, 10.487554, 11.013072, 11.013243, 11.008779, 11.257023, 11.248147, 11.257063, 8.542032,
    8.522548, 8.501312, 8.489585, 0.874061, 0.794386, -0.659122,
];
const B2_CT: [f64; 26] = [
    8.225801, 8.023227, 7.824771, 7.445350, 7.387242, 7.378331, 7.235809, 7.243009, 7.236244, 6.597825, 6.590446,
    6.580419, 6.568562, 5.603203, 5.585844, 5.598429, 5.201541, 5.223102, 5.235716, 7.664718, 7.675708, 7.689141,
    7.701790, 9.190983, 9.177147, 8.563246,
];
const S_CX: [f64; 26] = [
    0.000260, 0.000441, 0.000447, 0.000301, 0.000349, 0.000328, 0.000084, 0.000081, 0.000101, 0.000099, 0.000109,
    0.000107, 0.000066, 0.000066, 0.000035, 0.000035, 0.000024, 0.000024, 0.000025, 0.000020, 0.000021, 0.000017,
    0.000017, 0.000013, 0.000010, 0.000010,
];
const S_CT: [f64; 26] = [
    0.000632, 0.000311, 0.000240, 0.000119, 0.000122, 0.000122, 0.000038, 0.000033, 0.000032, 0.000019, 0.000020,
    0.000025, 0.000024, 0.000025, 0.000011, 0.000011, 0.000008, 0.000008, 0.000007, 0.000009, 0.000010, 0.000011,
    0.000011, 0.000010, 0.000006, 0.000003,
];
const T_CX: [f64; 26] = [
    0.012319, 0.068944, 0.004574, 0.043061, 0.013354, 0.010657, 0.003460, 0.006947, 0.023922, 0.011055, 0.005921,
    0.004477, 0.010273, 0.009798, 0.017845, 0.010722, 0.033866, 0.022769, 0.016185, 0.039948, 0.031800, 0.053509,
    0.052819, 0.010349, 0.034829, 0.040874,
];
const T_CT: [f64; 26] = [
    0.038076, 0.084563, 0.002270, 0.026815, 0.011328, 0.025445, 0.022151, 0.018605, 0.021014, 0.015559, 0.023281,
    0.008007, 0.007412, 0.011639, 0.003317, 0.007399, 0.016849, 0.005743, 0.001260, 0.024639, 0.018848, 0.030435,
    0.025353, 0.054723, 0.066271, 0.058132,
];
/// The recorded verdicts: per radius within the band, and the division (`R_lo`, `R_hi`, `U_lo`, `U_hi`, `R_eq`, `U_eq` in Pa).
const WITHIN_CX_RECORD: [bool; 26] = [
    false, true, true, true, true, true, true, true, false, false, false, false, false, false, false, false, true,
    true, false, true, true, true, true, true, false, false,
];
const WITHIN_CT_RECORD: [bool; 26] = [
    false, false, false, true, false, false, true, false, false, true, false, false, true, true, true, false, false,
    false, true, true, true, true, true, true, true, true,
];
const VERDICT_RECORD: Verdict = Verdict::CannotBeTold;
const DIVISION_RECORD: [f64; 6] = [-34.45, 59.11, 23.75, 24.95, -3.76, 31.10];

/// The band of ADR 0765 at each measured radius, from the recorded profiles: `U_num + S + T` (`u_meas` is zero: not stated in a source).
fn bands() -> ([f64; 26], [f64; 26]) {
    let band = |b1: &[f64; 26], b2: &[f64; 26], s: &[f64; 26], t: &[f64; 26]| -> [f64; 26] {
        std::array::from_fn(|i| u_num(b1[i], b2[i]) + s[i] + t[i])
    };
    (band(&B1_CX, &B2_CX, &S_CX, &T_CX), band(&B1_CT, &B2_CT, &S_CT, &T_CT))
}

/// Per radius: the simulated value is within its band of the measured one, for `c_x` and `c_t`.
fn within(cx: &[f64; 26], ct: &[f64; 26]) -> ([bool; 26], [bool; 26]) {
    let (bx, bt) = bands();
    (
        std::array::from_fn(|i| (cx[i] - LDA90_UX[i]).abs() <= bx[i]),
        std::array::from_fn(|i| (ct[i] - LDA90_UPHI[i]).abs() <= bt[i]),
    )
}

/// The division from scalars of B1 (`e_s`, `q_s`, `Pi_s`) and the recorded band terms of the solve.
fn division_of(e_s: f64, q_s: f64, pi_s: f64) -> Division {
    let u_pi = u_num(B1_SCALARS[2], B2_SCALARS[2]) + S_T_COV_PI.iter().sum::<f64>();
    let u_eq = u_num(B1_SCALARS[0] - B1_SCALARS[1], B2_SCALARS[0] - B2_SCALARS[1]) + S_T_COV_EQ.iter().sum::<f64>();
    division(pi_s, u_pi, e_s - q_s, u_eq)
}

// ---------------------------------------------------------------- run by the gate

/// V1 (ADR 0765): the constants of the measured profiles equal the transcription of the record, and the integrals registered from the
/// measured data alone are reproduced from them.
#[test]
fn f878_lda_reference_and_measured_integrals() {
    let rows: Vec<Vec<f64>> = LDA_CSV
        .lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("x_mm"))
        .map(|l| l.split(',').map(|v| v.trim().parse::<f64>().unwrap()).collect())
        .collect();
    assert_eq!(rows.len(), 52);
    for (x, at) in [(X_UP, 0), (X_DOWN, 26)] {
        let c: [&[f64; 26]; 6] = if x == X_UP {
            [
                &LDA20_UX,
                &LDA20_UPHI,
                &LDA20_UR,
                &LDA20_ENS_UX,
                &LDA20_ENS_UX_UPHI,
                &LDA20_ENS_KE_FLUX,
            ]
        } else {
            [
                &LDA90_UX,
                &LDA90_UPHI,
                &LDA90_UR,
                &LDA90_ENS_UX,
                &LDA90_ENS_UX_UPHI,
                &LDA90_ENS_KE_FLUX,
            ]
        };
        for i in 0..26 {
            let row = &rows[at + i];
            assert_eq!(row[0], x);
            assert_eq!(row[1], RADIUS_NORM[i]);
            for (k, col) in [2, 3, 4, 8, 11, 12].into_iter().enumerate() {
                assert_eq!(row[col], c[k][i], "x {x} radius {} column {col}", RADIUS_NORM[i]);
            }
        }
    }
    let ((p20, e20), (p90, e90)) = (measured(X_UP), measured(X_DOWN));
    println!("V1 x = 20 mm: profile {p20:?}; ensemble {e20:?}");
    println!("V1 x = 90 mm: profile {p90:?}; ensemble {e90:?}");
    println!(
        "V1 measured shaft power {POWER_MEAS} W = {:.2} Pa per design volume flow",
        POWER_MEAS / Q_DESIGN
    );
    let near = |a: f64, b: f64, tol: f64| (a - b).abs() <= tol;
    assert!(
        near(p90.power, 299.20, 5e-3) && near(e90.power, 297.66, 5e-3),
        "{p90:?} {e90:?}"
    );
    assert!(
        near(p90.e, 221.22, 5e-3) && near(e90.e, 220.02, 5e-3),
        "{p90:?} {e90:?}"
    );
    assert!(near(p90.q, 97.13, 5e-3) && near(e90.q, 99.20, 5e-3), "{p90:?} {e90:?}");
    assert!(
        near(p20.power, -36.22, 5e-3) && near(e20.power, -33.55, 5e-3),
        "{p20:?} {e20:?}"
    );
    assert!(
        near(p90.flow, 1.3525, 5e-5) && near(e90.flow, 1.3529, 5e-5),
        "{p90:?} {e90:?}"
    );
    assert!(
        near(p20.flow, 1.3221, 5e-5) && near(e20.flow, 1.3005, 5e-5),
        "{p20:?} {e20:?}"
    );
}

/// V2 (as amendment 1 has it), V3 (on B3) and V4 (ADR 0765): the circumferential mean returns a field of constant cylindrical components to rounding at every
/// measured radius of both planes; no point of the plane ahead of the blade is outside the fluid; the trapezoid rule on the measured
/// radii returns the closed form of a free vortex.
#[test]
fn f878_sampler_closed_forms() {
    let case = Case::parse_toml(&case_toml(B3, CAP)).unwrap_or_else(|e| panic!("{e:?}"));
    let mesh = build_mesh(&case).unwrap_or_else(|e| panic!("{e:?}"));
    let loc = Locator::new(&mesh);
    let r = radii();
    // constant cylindrical components of the absolute velocity (axial, circumferential, radial); the field holds the relative velocity
    let (a, b, c) = (1.7, -2.3, 0.4);
    let w: Vec<[f64; 3]> = loc
        .centres
        .iter()
        .map(|x| {
            let rc = x[0].hypot(x[1]);
            let bw = b - omega() * rc;
            [(c * x[0] - bw * x[1]) / rc, (c * x[1] + bw * x[0]) / rc, a]
        })
        .collect();
    let p = vec![0.0; w.len()];
    for x in [X_UP, X_DOWN] {
        let plane = loc.plane(x, &r);
        let out: Vec<usize> = plane
            .iter()
            .map(|ring| ring.iter().filter(|c| c.is_none()).count())
            .collect();
        println!("V3 B3 points not in the fluid at x = {x} mm: {out:?}");
        if x == X_UP {
            assert!(
                out.iter().all(|&n| n == 0),
                "points outside the fluid ahead of the blade: {out:?}"
            );
        }
        let mut worst = 0.0f64;
        for (cells, &ri) in plane.iter().zip(&r) {
            let m = ring_mean(&loc, cells, ri, &w, &p);
            assert!(m.fluid > 0.5, "radius {ri}: {m:?}");
            for (got, want) in [(m.cx, a), (m.ct, b), (m.cr, c)] {
                worst = worst.max((got / want - 1.0).abs());
            }
        }
        println!("V2 x = {x} mm: largest relative error {worst:.3e}");
        assert!(worst <= 1e-9, "{worst}");
    }
    // V4: c_x = a, c_t = G / r gives P_E = rho omega a G pi (r_26^2 - r_1^2); the integrand is linear in r
    let (a, g) = (10.0, 1.3);
    let cx = vec![a; 26];
    let cxct: Vec<f64> = r.iter().map(|ri| a * g / ri).collect();
    let h = head(&r, &cx, &cxct, &[0.0; 26], trapezoid);
    let exact = RHO * omega() * a * g * PI * (r[25] * r[25] - r[0] * r[0]);
    println!("V4 Euler power {} W, closed form {exact} W", h.power);
    assert!((h.power / exact - 1.0).abs() <= 1e-9);
}

/// S2 (ADR 0765): the recorded verdicts follow from the recorded profiles, the recorded band terms and the measured constants.
#[test]
fn f878_recorded_verdicts_follow_from_the_recorded_profiles() {
    let (bx, bt) = bands();
    let (wx, wt) = within(&B1_CX, &B1_CT);
    for i in 0..26 {
        println!(
            "S2 r/R {:.3}: c_x {:.3} against {:.2}, band {:.3}, {}; c_t {:.3} against {:.2}, band {:.3}, {}",
            RADIUS_NORM[i],
            B1_CX[i],
            LDA90_UX[i],
            bx[i],
            if wx[i] { "within" } else { "MISS" },
            B1_CT[i],
            LDA90_UPHI[i],
            bt[i],
            if wt[i] { "within" } else { "MISS" },
        );
    }
    assert_eq!(wx, WITHIN_CX_RECORD);
    assert_eq!(wt, WITHIN_CT_RECORD);
    let d = division_of(B1_SCALARS[0], B1_SCALARS[1], B1_SCALARS[2]);
    println!("S2 division {d:?}");
    assert_eq!(d.verdict, VERDICT_RECORD);
    let near = |a: f64, b: f64| (a - b).abs() <= 5e-3;
    assert!(near(d.d, 52.47), "{d:?}");
    assert!(
        near(d.r_lo, DIVISION_RECORD[0])
            && near(d.r_hi, DIVISION_RECORD[1])
            && near(d.u_lo, DIVISION_RECORD[2])
            && near(d.u_hi, DIVISION_RECORD[3])
            && near(d.r_eq, DIVISION_RECORD[4])
            && near(d.u_eq, DIVISION_RECORD[5]),
        "{d:?}"
    );
}

/// S1 (ADR 0765): the fast test on the path of the heavy record. B3 for 300 iterations; `e_s`, `q_s`, `Pi_s` and `Q_c` at x = 90 mm
/// within 1 % of the values of the first run.
#[test]
fn f878_b3_short_run_holds_the_profiles() {
    let o = run_design_point("B3 short", B3, SHORT, &[SHORT]);
    assert_eq!(o.cells, 4920);
    assert_eq!(o.iterations, SHORT, "{:?}", o.stop);
    assert!(o.stop.is_none(), "{:?}", o.stop);
    let s = o.samples[0].scalars(X_DOWN);
    println!(
        "S1 B3 at {SHORT}: e_s {:.4} Pa, q_s {:.4} Pa, Pi_s {:.4} Pa, Q_c {:.6} m^3/s",
        s[0], s[1], s[2], s[3]
    );
    assert!(
        (0..4).all(|i| holds(s[i], B3_SHORT_RECORD[i])),
        "the profiles moved: {s:?} against {B3_SHORT_RECORD:?}"
    );
}

// ---------------------------------------------------------------- heavy records

static B1_OUTCOME: OnceLock<Outcome> = OnceLock::new();

/// The one B1 solve that the two heavy tests share when they run in one process.
fn b1_outcome() -> &'static Outcome {
    B1_OUTCOME.get_or_init(|| {
        let o = run_design_point("B1", B1, CAP, &SAMPLE_AT);
        print_profiles("B1", &o);
        o
    })
}

/// The profiles of B1 at 8000 iterations at x = 90 mm: (`c_x`, `c_t`) at the measured radii.
fn b1_profiles(o: &Outcome) -> ([f64; 26], [f64; 26]) {
    let rings = o.samples.last().unwrap().plane(X_DOWN);
    (
        std::array::from_fn(|i| rings[i].cx),
        std::array::from_fn(|i| rings[i].ct),
    )
}

/// The heavy record of the state (ADR 0765, task 5): B1 to 8000 iterations. Fails if the solve is not the recorded state of ADR 0762, if
/// a scalar or a profile value moves by more than the tolerance of the state either way, or if a recorded verdict changes.
#[test]
#[ignore = "heavy record, ADR 0765: the B1 design-point solve of ADR 0762 with the profiles (38 184 cells, 8000 iterations, about 1 h alone): cargo test --release -p rcfd-api --test f878_fan01_lda_profiles f878_b1 -- --ignored --nocapture --test-threads 1"]
fn f878_b1_profiles_state_record() {
    let o = b1_outcome();
    assert_eq!(o.cells, 38184);
    assert_eq!(o.iterations, CAP, "{:?}", o.stop);
    assert!(
        holds(o.torque, STATE_0762[0].0) && holds(o.dp_ts, STATE_0762[0].1),
        "not the recorded state of ADR 0762: torque {} dp_ts {}",
        o.torque,
        o.dp_ts
    );
    let last = o.samples.last().unwrap();
    assert_eq!(last.iteration, CAP);
    let s = last.scalars(X_DOWN);
    assert!(
        (0..4).all(|i| holds(s[i], B1_SCALARS[i])),
        "the scalars moved: {s:?} against {B1_SCALARS:?}"
    );
    let (cx, ct) = b1_profiles(o);
    for i in 0..26 {
        assert!(
            holds_profile(cx[i], B1_CX[i]) && holds_profile(ct[i], B1_CT[i]),
            "the profile moved at r/R {}: c_x {} against {}, c_t {} against {}",
            RADIUS_NORM[i],
            cx[i],
            B1_CX[i],
            ct[i],
            B1_CT[i]
        );
    }
    let (wx, wt) = within(&cx, &ct);
    assert_eq!(wx, WITHIN_CX_RECORD, "a verdict of c_x changed");
    assert_eq!(wt, WITHIN_CT_RECORD, "a verdict of c_t changed");
    assert_eq!(
        division_of(s[0], s[1], s[2]).verdict,
        VERDICT_RECORD,
        "the verdict of the division changed"
    );
}

/// The acceptance of F-878 as ADR 0765 registered it: every measured radius within its band for `c_x` and for `c_t`, and the division
/// of the missing pressure rise given by the registered rule.
#[test]
#[ignore = "heavy, ADR 0765: the acceptance of F-878 on the B1 solve; shares the solve of the state record"]
fn f878_b1_profiles_acceptance() {
    let o = b1_outcome();
    let s = o.samples.last().unwrap().scalars(X_DOWN);
    let (cx, ct) = b1_profiles(o);
    let (wx, wt) = within(&cx, &ct);
    let d = division_of(s[0], s[1], s[2]);
    let miss = |w: &[bool; 26]| -> Vec<f64> { (0..26).filter(|&i| !w[i]).map(|i| RADIUS_NORM[i]).collect() };
    assert!(
        wx.iter().all(|&b| b) && wt.iter().all(|&b| b) && d.verdict != Verdict::CannotBeTold,
        "F-878 MISSED: c_x outside its band at r/R {:?}; c_t outside its band at r/R {:?}; division {d:?}",
        miss(&wx),
        miss(&wt)
    );
}

/// One registered run of the family, selected by `F878_RUN` = `B1`, `B2` or `B3` (ADR 0765): prints the profiles at the five
/// iterations for `docs/investigations/f878-fan01-lda-profiles/analyse.py`.
#[test]
#[ignore = "heavy, ADR 0765: F878_RUN=B2 cargo test --release -p rcfd-api --test f878_fan01_lda_profiles f878_family_run -- --ignored --nocapture"]
fn f878_family_run() {
    let sel = std::env::var("F878_RUN").expect("set F878_RUN");
    let (c, state) = match sel.as_str() {
        "B1" => (B1, STATE_0762[0]),
        "B2" => (B2, STATE_0762[1]),
        "B3" => (B3, STATE_0762[2]),
        other => panic!("F878_RUN = {other}"),
    };
    let o = run_design_point(&sel, c, CAP, &SAMPLE_AT);
    print_profiles(&sel, &o);
    // R1: the solve is the recorded state of ADR 0762
    assert!(
        holds(o.torque, state.0) && holds(o.dp_ts, state.1),
        "{sel} is not the recorded state of ADR 0762: torque {} dp_ts {}",
        o.torque,
        o.dp_ts
    );
}
