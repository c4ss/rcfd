//! ADR 0831 (F-870): the FAN-01 design point as a time march to a settled state on B1 (38 184 cells) at two steps, the
//! convergence rule of option (c) of ADR 0827 (answer 1), and the compared quantities of F-870 read from the settled state:
//! pressure rise and shaft power (ADR 0762), Euler head, kinetic energy and the profiles of `c_x` and `c_t` (ADR 0765), the casing
//! pressure behind the rotor (ADR 0769). The case is check H's (`h_text`, `run_h` and the opt-ins of `f885_kept_gradient_lag.rs`,
//! ADR 0824) with the B1 counts; the sampler is that of `f878_fan01_lda_profiles.rs` (ADR 0765), copied without a change. The two
//! marches are heavy records that write the files of `docs/investigations/f870-time-march-b1/`; the gate reads those files.

#![allow(clippy::neg_cmp_op_on_partial_ord)] // a NaN must fail the checks

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_core::vec3::Vec3;
use rcfd_mesh::Mesh;
use std::f64::consts::PI;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::Instant;

const N_BLADES: f64 = 9.0;
const R_HUB: f64 = 0.124;
const R_DUCT: f64 = 0.250;
const RPM: f64 = 1486.0;
const RHO: f64 = 1.2; // ASSUMED (ADR 0075, ADR 0762): the density of the measurement is in no source that was read (F-877)
const Q_DESIGN: f64 = 1.4032;

/// The measured design point (ADR 0762) and casing pressure at x = 90 mm against the chamber (ADR 0769).
const DP_TS_MEAS: f64 = 126.46;
const POWER_MEAS: f64 = 333.30;
const CASING_MEAS: f64 = 113.90;
/// ADR 0762's band (`U_num`, `u_meas` = 0) of pressure rise [Pa] and shaft power [W], and `U_num` of the torque [N m].
const U_DP: f64 = 29.75;
const U_POWER: f64 = 32.98;
const U_TORQUE: f64 = 0.21190;
/// ADR 0762's B1 SIMPLEC sample at 8000 iterations: torque [N m], pressure rise [Pa].
const SIMPLEC_B1: (f64, f64) = (2.098164, 73.9881);
/// ADR 0769's terms of the reading of the casing pressure [Pa]: two standard errors, the place between the transducers.
const U_STAT: f64 = 0.5674;
const U_X: f64 = 0.4901;

/// The step of check H (ADR 0766: `T_c / 100`) and its steps for six flow-through times (ADR 0183 D5).
const H_DT: f64 = 4.605e-5;
const H_STEPS: usize = 5498;
const CELLS: usize = 38184;
/// The settled rule (ADR 0827 section 4, answer 1): half range over mean, and the two steps' means [-].
const SETTLE: f64 = 0.01;
const STEP_AGREE: f64 = 0.01;
/// The last flow-through time starts after this many flow-through times.
const LAST_FT_FROM: f64 = 5.0;
/// The steps of M(dt) at which the rings are sampled; M(dt/2) at twice these (the same times).
const SAMPLE_DT: [usize; 5] = [4582, 4811, 5040, 5269, 5498];
/// The casing ring [m]: in the wall-adjacent cell (ADR 0831 section 1).
const R_CASING_RING: f64 = 0.2495;

const N_THETA: usize = 360;
const PITCH: f64 = 2.0 * PI / 9.0;
const FLUID_TOL: f64 = 1.0e-4;
const X_ROOT_LE_MM: f64 = 21.45;
const X_DOWN: f64 = 90.0;
const X_DOWN_LO: f64 = 88.7;
const X_DOWN_HI: f64 = 91.3;
const PLANES: [f64; 3] = [X_DOWN_LO, X_DOWN, X_DOWN_HI];
const FS: f64 = 1.25;
const CELLS_B1: f64 = 38184.0;
const CELLS_B2: f64 = 13627.0;

/// The tolerance of a held value (H, SR) [-] and the steps of the short run.
const REC_TOL: f64 = 1e-6;
const SHORT: usize = 20;

fn omega() -> f64 {
    RPM * 2.0 * PI / 60.0
}

fn annulus_area() -> f64 {
    PI * (R_DUCT * R_DUCT - R_HUB * R_HUB)
}

fn t_ft() -> f64 {
    H_STEPS as f64 * H_DT / 6.0
}

// ---------------------------------------------------------------- the case (f885_kept_gradient_lag.rs, ADR 0824; B1 counts)

/// The 16 registered FAN-01 sections (ADR 0075 amendment 1).
fn sections() -> Vec<Vec<f64>> {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../validation-data/reference/w34d_fan01_cad_sections.csv");
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

/// The case text of the march (ADR 0831 section 1): check H's text, as `h_text` of `f879_fan01_check_h.rs` (ADR 0766: the ADR 0086 bladed B2 case with the solver of
/// ADR 0183 D5, the seam diffusivity `pair` of D-1 and the six reports of D-4).
fn m_text(dt: f64) -> String {
    let rows = sections();
    let (x0, t0) = (rows[0][6], rows[0][7]);
    let ref_speed = (Q_DESIGN / annulus_area()).hypot(omega() * 0.186);
    // B1 of ADR 0086 (ADR 0831: the only change against check H)
    let (n_up, n_chord, n_down, n_side, n_nose, n_span, n_gap) = (12, 24, 12, 15, 6, 21, 10);
    let mesh = format!(
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
    );
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
        r#"{mesh}
[physics]
nu = 1.5e-5
rho = {RHO:?}
[turbulence]
model = "k_omega_sst"
[turbulence.periodic_seam]
diffusivity = "pair"
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
[solver]
algorithm = "pimple"
n_outer = 5
time_scheme = "bdf2"
dt = {dt:?}
end_time = 0.2532
convection = "linear_upwind"
linear_upwind_limiter = "barth_jespersen"
[reports.m_blade]
type = "moment"
patch = "blade"
direction = [0.0, 0.0, 1.0]
[reports.m_hub]
type = "moment"
patch = "hub"
direction = [0.0, 0.0, 1.0]
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
"#
    )
}

fn sim_of(text: &str) -> Simulation {
    let case = Case::parse_toml(text).unwrap_or_else(|e| panic!("{e:?}"));
    let errs = case.semantic_errors();
    assert!(errs.is_empty(), "{errs:?}");
    Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"))
}

/// Route B: the largest Euclidean norm over the cells of `b - a`, formed in the test.
fn change(a: &[Vec3], b: &[Vec3]) -> f64 {
    a.iter().zip(b).fold(0.0f64, |m, (x, y)| {
        m.max(((y[0] - x[0]).powi(2) + (y[1] - x[1]).powi(2) + (y[2] - x[2]).powi(2)).sqrt())
    })
}

fn rel(a: f64, b: f64) -> f64 {
    if a == b {
        0.0
    } else {
        (a - b).abs() / a.abs().max(b.abs())
    }
}

// ---------------------------------------------------------------- the sampler and the recorded B1/B2 state (f878_fan01_lda_profiles.rs, ADR 0765; copied)

/// The mesh coordinate z [m] of the plane at `x_mm` of the record.
fn z_of(x_mm: f64) -> f64 {
    0.1 + (x_mm - X_ROOT_LE_MM) * 1e-3
}

/// The 26 measured radii [m].
fn radii() -> Vec<f64> {
    RADIUS_NORM.iter().map(|v| v * R_DUCT).collect()
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

const RADIUS_NORM: [f64; 26] = [
    0.532, 0.552, 0.572, 0.592, 0.612, 0.632, 0.652, 0.672, 0.692, 0.712, 0.732, 0.752, 0.772, 0.792, 0.812, 0.832,
    0.852, 0.872, 0.892, 0.912, 0.932, 0.952, 0.960, 0.968, 0.976, 0.984,
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

/// B1 and B2 at 8000 iterations, x = 90 mm, the measured span, pointwise reading: (`e_s`, `q_s`, `Pi_s`, `Q_c`).
const B1_SCALARS: [f64; 4] = [226.861468, 102.279724, 61.709396, 1.375399];

const B2_SCALARS: [f64; 4] = [231.526440, 98.038657, 54.732823, 1.331399];

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

// ---------------------------------------------------------------- the measures (ADR 0831 sections 2 and 3)

/// Mean and half range `(max - min) / 2`.
fn mean_half(x: &[f64]) -> (f64, f64) {
    assert!(!x.is_empty(), "an empty window");
    let m = x.iter().sum::<f64>() / x.len() as f64;
    let (lo, hi) = x
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &v| (a.min(v), b.max(v)));
    (m, 0.5 * (hi - lo))
}

/// The indices of the last flow-through time: `t > 5 T_ft`.
fn last_ft(t: &[f64]) -> Vec<usize> {
    (0..t.len()).filter(|&i| t[i] > LAST_FT_FROM * t_ft()).collect()
}

/// S1: settled iff the half range is at most 1 % of the mean.
fn settled(m: f64, a: f64) -> bool {
    a / m.abs() <= SETTLE
}

/// A band of ADR 0765's form: the terms added.
fn band(u_num: f64, s: f64, t: f64, u_meas: f64) -> f64 {
    u_num + s + t + u_meas
}

/// The measured Euler head and kinetic energy at x = 90 mm (ADR 0765), (profile reading, pointwise reading).
fn measured90() -> (Head, Head) {
    let r = radii();
    let prod: Vec<f64> = (0..26).map(|i| LDA90_UX[i] * LDA90_UPHI[i]).collect();
    let ke: Vec<f64> = (0..26)
        .map(|i| 0.5 * LDA90_UX[i] * (LDA90_UX[i].powi(2) + LDA90_UPHI[i].powi(2) + LDA90_UR[i].powi(2)))
        .collect();
    (
        head(&r, &LDA90_UX, &prod, &ke, trapezoid),
        head(&r, &LDA90_ENS_UX, &LDA90_ENS_UX_UPHI, &LDA90_ENS_KE_FLUX, trapezoid),
    )
}

/// The mean pressure (kinematic) over the points of a ring in the fluid.
fn ring_p(cells: &[Option<usize>], p: &[f64]) -> f64 {
    let v: Vec<f64> = cells.iter().flatten().map(|&c| p[c]).collect();
    v.iter().sum::<f64>() / v.len() as f64
}

// ---------------------------------------------------------------- the records

const STEP_HEADER: &str = "step,t,torque,dp_ts,res_u,continuity,d1,d2,d3,d4,d5,du_step";
const RING_HEADER: &str = "step,t,p_in,plane_mm,ring,r,fluid,cx,ct,cr,cxct,ke,pcx,p";

/// One step of a march.
#[derive(Debug, Clone, Copy)]
struct Row {
    step: usize,
    t: f64,
    torque: f64,
    dp: f64,
    res_u: f64,
    continuity: f64,
    dv: [f64; 5],
    du_step: f64,
}

/// One ring of one sample: rings 0 to 25 are the measured radii, ring 26 the casing ring.
#[derive(Debug, Clone, Copy)]
struct RingRow {
    step: usize,
    t: f64,
    p_in: f64,
    plane: f64,
    ring: usize,
    m: Ring,
    p: f64,
}

fn write_records(dir: &Path, label: &str, dt: f64, rows: &[Row], rings: &[RingRow], failed: &Option<String>) {
    std::fs::create_dir_all(dir).unwrap();
    let mut out = format!(
        "# ADR 0831 (F-870), the march {label} on B1: dt = {dt:?} s, {} steps registered, {} made, failed {failed:?}; one row per step\n{STEP_HEADER}\n",
        if dt == H_DT { H_STEPS } else { 2 * H_STEPS },
        rows.len()
    );
    for r in rows {
        write!(out, "{},{:.9e}", r.step, r.t).unwrap();
        for v in [r.torque, r.dp, r.res_u, r.continuity]
            .iter()
            .chain(r.dv.iter())
            .chain([&r.du_step])
        {
            write!(out, ",{v:.9e}").unwrap();
        }
        out.push('\n');
    }
    std::fs::write(dir.join(format!("{label}.csv")), out).unwrap();
    let mut out = format!(
        "# ADR 0831 (F-870), the march {label} on B1: circumferential means (ADR 0765) at the sampled steps; ring 26 is the casing ring\n{RING_HEADER}\n"
    );
    for g in rings {
        let m = &g.m;
        write!(
            out,
            "{},{:.9e},{:.9e},{},{},{:.9e},{:.9e},{:.9e},{:.9e},{:.9e},{:.9e},{:.9e},{:.9e},{:.9e}",
            g.step, g.t, g.p_in, g.plane, g.ring, m.r, m.fluid, m.cx, m.ct, m.cr, m.cxct, m.ke, m.pcx, g.p
        )
        .unwrap();
        out.push('\n');
    }
    std::fs::write(dir.join(format!("{label}_rings.csv")), out).unwrap();
}

fn records_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/investigations/f870-time-march-b1")
}

fn data_lines(path: &Path) -> Vec<Vec<f64>> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("step"))
        .map(|l| l.split(',').map(|c| c.trim().parse::<f64>().unwrap()).collect())
        .collect()
}

fn read_rows(path: &Path) -> Vec<Row> {
    data_lines(path)
        .iter()
        .map(|c| Row {
            step: c[0] as usize,
            t: c[1],
            torque: c[2],
            dp: c[3],
            res_u: c[4],
            continuity: c[5],
            dv: [c[6], c[7], c[8], c[9], c[10]],
            du_step: c[11],
        })
        .collect()
}

fn read_rings(path: &Path) -> Vec<RingRow> {
    data_lines(path)
        .iter()
        .map(|c| RingRow {
            step: c[0] as usize,
            t: c[1],
            p_in: c[2],
            plane: c[3],
            ring: c[4] as usize,
            m: Ring {
                r: c[5],
                fluid: c[6],
                cx: c[7],
                ct: c[8],
                cr: c[9],
                cxct: c[10],
                ke: c[11],
                pcx: c[12],
            },
            p: c[13],
        })
        .collect()
}

// ---------------------------------------------------------------- the march (ADR 0831 section 1; `run_h` of ADR 0824, copied)

/// The one path of every solving test of this file: `steps` steps of the march at `dt` with the four opt-ins of ADR 0824 and both
/// outer records; the rings sampled at `samples`. No wall-clock stop; a failure or a non-finite value ends the run.
fn run_march(label: &str, dt: f64, steps: usize, samples: &[usize]) -> (Vec<Row>, Vec<RingRow>, Option<String>) {
    let mut sim = sim_of(&m_text(dt));
    assert_eq!(sim.geom.mesh.n_cells(), CELLS);
    {
        let s = &mut sim.pimple_solver_mut().expect("pimple").settings;
        s.outer_pressure_tolerance = Some(-1.0);
        s.outer_velocity_record = true;
        s.alpha_free_face_flux = true;
        s.outer_frozen_limiter = true;
        s.outer_frozen_convection_gradient = true;
        s.outer_frozen_convection_gradient_extrapolated_boundary = true;
    }
    let mut r = radii();
    r.push(R_CASING_RING);
    let sampler = (!samples.is_empty()).then(|| {
        let loc = Locator::new(&sim.geom.mesh);
        let planes: Vec<Vec<Vec<Option<usize>>>> = PLANES.iter().map(|&x| loc.plane(x, &r)).collect();
        for (x, pl) in PLANES.iter().zip(&planes) {
            let out: Vec<usize> = pl.iter().map(|g| g.iter().filter(|c| c.is_none()).count()).collect();
            println!("F870TM {label} points not in the fluid at x = {x} mm, per ring: {out:?}");
        }
        (loc, planes)
    });
    let u_in = Q_DESIGN / annulus_area();
    let (mut rows, mut rings, mut failed) = (Vec::with_capacity(steps), Vec::new(), None);
    let mut prev: Vec<Vec3> = sim.velocity().values.clone();
    let t0 = Instant::now();
    for k in 1..=steps {
        let st = sim.step();
        let t = k as f64 * dt;
        let du_step = change(&prev, &sim.velocity().values);
        prev.clone_from(&sim.velocity().values);
        let mon = |name: &str| sim.monitors.last(name).unwrap_or(f64::NAN);
        let torque = -N_BLADES * (mon("m_blade") + mon("m_hub"));
        let dp = RHO * (mon("p_out") - (mon("p_in") + 0.5 * u_in * u_in));
        let ps = sim.pimple_solver();
        if let Some(rec) = ps.and_then(|s| s.outer_record.last()) {
            assert_eq!((rec.step, rec.used, rec.met), (k, 5, false), "outer record of step {k}");
        }
        let dv = match ps.and_then(|s| s.outer_velocity.last()) {
            Some(v) => {
                assert_eq!((v.step, v.change.len()), (k, 5), "velocity record of step {k}");
                [v.change[0], v.change[1], v.change[2], v.change[3], v.change[4]]
            }
            None => [f64::NAN; 5],
        };
        rows.push(Row {
            step: k,
            t,
            torque,
            dp,
            res_u: st.res_u,
            continuity: st.continuity,
            dv,
            du_step,
        });
        if let (Some((loc, planes)), true) = (&sampler, samples.contains(&k)) {
            let p_in = mon("p_in");
            for (x, pl) in PLANES.iter().zip(planes) {
                let means = plane_means(loc, pl, &r, &sim);
                for (i, m) in means.into_iter().enumerate() {
                    let p = ring_p(&pl[i], &sim.pressure().values);
                    rings.push(RingRow {
                        step: k,
                        t,
                        p_in,
                        plane: *x,
                        ring: i,
                        m,
                        p,
                    });
                }
            }
        }
        if k % 250 == 0 || k <= 2 || k == steps {
            println!(
                "F870TM {label} step {k}: t {t:.5e} s, res_u {:.3e}, continuity {:.3e}, d5/d1 {:.3e}, torque {torque:.6} N m, dp_ts {dp:.4} Pa, {:.3} s per step, {:.1} min",
                st.res_u,
                st.continuity,
                dv[4] / dv[0],
                t0.elapsed().as_secs_f64() / k as f64,
                t0.elapsed().as_secs_f64() / 60.0
            );
        }
        if let Some(f) = sim.failure() {
            failed = Some(format!("failure at step {k}: {f}"));
            break;
        }
        if !(torque.is_finite() && dp.is_finite() && st.res_u.is_finite()) {
            failed = Some(format!("non-finite value at step {k}"));
            break;
        }
    }
    println!(
        "F870TM {label} END: {} of {steps} steps, failed {failed:?}, {:.3} s per step ({:.1} min)",
        rows.len(),
        t0.elapsed().as_secs_f64() / rows.len().max(1) as f64,
        t0.elapsed().as_secs_f64() / 60.0
    );
    (rows, rings, failed)
}

fn heavy(label: &str, dt: f64, steps: usize, samples: &[usize]) {
    let dir = PathBuf::from(std::env::var("F870_TM_OUT").expect("F870_TM_OUT: the folder of the records"));
    let (rows, rings, failed) = run_march(label, dt, steps, samples);
    write_records(&dir, label, dt, &rows, &rings, &failed);
    assert!(failed.is_none(), "{failed:?}");
    assert_eq!((rows.len(), rings.len()), (steps, 5 * 3 * 27));
}

// ---------------------------------------------------------------- the rows (ADR 0831 table of checks), from the records

/// Every number of the rows by key, and every verdict by key.
struct Rows {
    keys: Vec<(String, f64)>,
    verdicts: Vec<(String, bool)>,
}

impl Rows {
    fn num(&mut self, k: &str, v: f64) {
        self.keys.push((k.to_string(), v));
    }
    fn verdict(&mut self, k: &str, v: bool) {
        self.verdicts.push((k.to_string(), v));
    }
    fn get(&self, k: &str) -> f64 {
        self.keys.iter().find(|(a, _)| a == k).unwrap().1
    }
    fn met(&self, k: &str) -> bool {
        self.verdicts.iter().find(|(a, _)| a == k).unwrap().1
    }
}

/// The sampled quantities of one run: per plane the mean over the samples, and at x = 90 mm the half range over the samples, of
/// `e`, `q`, `Pi_s`, the casing pressure and the 26 values of `c_x` and of `c_t`.
struct Sampled {
    mean: [Vec<f64>; 3],
    spread: Vec<f64>,
}

/// The vector of one sample at one plane: `e`, `q`, `Pi_s`, casing pressure [Pa], `c_x` (26), `c_t` (26).
fn sample_vector(rings: &[RingRow]) -> Vec<f64> {
    assert_eq!(rings.len(), 27);
    let ms: Vec<Ring> = rings[..26].iter().map(|g| g.m).collect();
    let (_, point, pi) = solved(&ms, rings[0].p_in, trapezoid, false);
    let u_in = Q_DESIGN / annulus_area();
    let casing = RHO * (rings[26].p - rings[0].p_in - 0.5 * u_in * u_in);
    let mut v = vec![point.e, point.q, pi, casing];
    v.extend(ms.iter().map(|m| m.cx));
    v.extend(ms.iter().map(|m| m.ct));
    v
}

fn sampled(rings: &[RingRow]) -> Sampled {
    let mut steps: Vec<usize> = rings.iter().map(|g| g.step).collect();
    steps.dedup();
    let per = |x: f64| -> Vec<Vec<f64>> {
        steps
            .iter()
            .map(|&s| {
                let mut g: Vec<RingRow> = rings.iter().filter(|g| g.step == s && g.plane == x).copied().collect();
                g.sort_by_key(|g| g.ring);
                sample_vector(&g)
            })
            .collect()
    };
    let mean_of = |vs: &[Vec<f64>]| -> Vec<f64> {
        (0..vs[0].len())
            .map(|j| mean_half(&vs.iter().map(|v| v[j]).collect::<Vec<_>>()).0)
            .collect()
    };
    let at90 = per(X_DOWN);
    let spread = (0..at90[0].len())
        .map(|j| mean_half(&at90.iter().map(|v| v[j]).collect::<Vec<_>>()).1)
        .collect();
    Sampled {
        mean: [mean_of(&per(X_DOWN_LO)), mean_of(&at90), mean_of(&per(X_DOWN_HI))],
        spread,
    }
}

/// The rows of ADR 0831 from the records of M(dt) and M(dt/2).
fn rows_of(a: &[Row], b: &[Row], ra: &[RingRow], rb: &[RingRow]) -> Rows {
    let mut o = Rows {
        keys: Vec::new(),
        verdicts: Vec::new(),
    };
    // R: complete, the sampled steps and the rings as registered
    let complete = |rows: &[Row], n: usize| rows.len() == n && rows.iter().enumerate().all(|(i, r)| r.step == i + 1);
    let rings_ok = |rings: &[RingRow], f: usize| {
        let mut want = radii();
        want.push(R_CASING_RING);
        rings.len() == 5 * 3 * 27
            && rings
                .iter()
                .all(|g| SAMPLE_DT.iter().any(|&s| s * f == g.step) && rel(g.m.r, want[g.ring]) <= 1e-12)
    };
    o.verdict(
        "R",
        complete(a, H_STEPS) && complete(b, 2 * H_STEPS) && rings_ok(ra, 1) && rings_ok(rb, 2),
    );
    o.num("R rows dt", a.len() as f64);
    o.num("R rows dt2", b.len() as f64);
    // S1, S2
    let mut means = [[0.0; 2]; 2];
    let mut s1 = true;
    for (j, (name, rows)) in [("dt", a), ("dt2", b)].iter().enumerate() {
        let t: Vec<f64> = rows.iter().map(|r| r.t).collect();
        let w = last_ft(&t);
        o.num(&format!("last ft steps {name}"), w.len() as f64);
        o.num(&format!("last ft first step {name}"), rows[w[0]].step as f64);
        for (i, sig) in ["torque", "dp_ts"].iter().enumerate() {
            let x: Vec<f64> = w
                .iter()
                .map(|&k| if i == 0 { rows[k].torque } else { rows[k].dp })
                .collect();
            let (m, h) = mean_half(&x);
            means[j][i] = m;
            s1 &= settled(m, h);
            o.num(&format!("{sig} mean {name}"), m);
            o.num(&format!("{sig} half range {name}"), h);
            o.num(&format!("{sig} share {name}"), h / m.abs());
            for f in 1..=6 {
                let xs: Vec<f64> = rows
                    .iter()
                    .filter(|r| r.t > (f - 1) as f64 * t_ft() && r.t <= f as f64 * t_ft())
                    .map(|r| if i == 0 { r.torque } else { r.dp })
                    .collect();
                if !xs.is_empty() {
                    o.num(&format!("{sig} half range ft{f} {name}"), mean_half(&xs).1);
                }
            }
        }
        let e1 = w.iter().filter(|&&k| rows[k].dv[4] <= 1e-2 * rows[k].dv[0]).count() as f64 / w.len() as f64;
        o.num(&format!("E-1 share last ft {name}"), e1);
    }
    o.verdict("S1", s1);
    let s2: Vec<f64> = (0..2).map(|i| (means[1][i] / means[0][i] - 1.0).abs()).collect();
    o.num("S2 torque", s2[0]);
    o.num("S2 dp_ts", s2[1]);
    o.verdict("S2", s2.iter().all(|&v| v <= STEP_AGREE));
    for (i, sig) in ["torque", "dp_ts"].iter().enumerate() {
        o.num(
            &format!("{sig} Richardson"),
            means[1][i] + (means[1][i] - means[0][i]) / 3.0,
        );
    }
    let (torque, dp) = (means[1][0], means[1][1]);
    // Q1, Q2, K
    o.num("Q1 distance", (dp - DP_TS_MEAS).abs());
    o.verdict("Q1", (dp - DP_TS_MEAS).abs() <= U_DP);
    let power = omega() * torque;
    o.num("Q2 power", power);
    o.num("Q2 distance", (power - POWER_MEAS).abs());
    o.verdict("Q2", (power - POWER_MEAS).abs() <= U_POWER);
    o.num("K torque", torque - SIMPLEC_B1.0);
    o.num("K dp_ts", dp - SIMPLEC_B1.1);
    o.verdict(
        "K",
        (torque - SIMPLEC_B1.0).abs() <= U_TORQUE && (dp - SIMPLEC_B1.1).abs() <= U_DP,
    );
    // Q3 to Q7 from the rings of M(dt/2); M(dt) reported
    let (prof, point) = measured90();
    o.num("measured Euler power, pointwise", point.power);
    let (sa, sb) = (sampled(ra), sampled(rb));
    let t_of = |s: &Sampled, j: usize| mean_half(&[s.mean[0][j], s.mean[1][j], s.mean[2][j]]).1;
    let scal = [
        (
            "Q3 e",
            point.e,
            u_num(B1_SCALARS[0], B2_SCALARS[0]),
            (prof.e - point.e).abs(),
        ),
        (
            "Q4 q",
            point.q,
            u_num(B1_SCALARS[1], B2_SCALARS[1]),
            (prof.q - point.q).abs(),
        ),
        ("Pi_s", f64::NAN, u_num(B1_SCALARS[2], B2_SCALARS[2]), 0.0),
        (
            "Q7 casing",
            CASING_MEAS,
            u_num(B1_SCALARS[2], B2_SCALARS[2]),
            U_STAT + U_X,
        ),
    ];
    for (j, (name, meas, un, um)) in scal.iter().enumerate() {
        let v = sb.mean[1][j];
        let bd = band(*un, sb.spread[j], t_of(&sb, j), *um);
        o.num(&format!("{name} value"), v);
        o.num(&format!("{name} value dt"), sa.mean[1][j]);
        o.num(&format!("{name} S"), sb.spread[j]);
        o.num(&format!("{name} T"), t_of(&sb, j));
        o.num(&format!("{name} band"), bd);
        if meas.is_finite() {
            o.num(&format!("{name} distance"), (v - meas).abs());
            o.verdict(name.split(' ').next().unwrap(), (v - meas).abs() <= bd);
        }
    }
    o.num("Pi_s against ADR 0765 B1", sb.mean[1][2] - B1_SCALARS[2]);
    o.num("e against ADR 0765 B1", sb.mean[1][0] - B1_SCALARS[0]);
    o.num("q against ADR 0765 B1", sb.mean[1][1] - B1_SCALARS[1]);
    for (q, off, lda, b1, b2) in [
        ("Q5", 4, &LDA90_UX, &B1_CX, &B2_CX),
        ("Q6", 30, &LDA90_UPHI, &B1_CT, &B2_CT),
    ] {
        let mut within = 0;
        for i in 0..26 {
            let j = off + i;
            let v = sb.mean[1][j];
            let bd = band(u_num(b1[i], b2[i]), sb.spread[j], t_of(&sb, j), 0.0);
            o.num(&format!("{q} r{i} value"), v);
            o.num(&format!("{q} r{i} band"), bd);
            o.num(&format!("{q} r{i} against ADR 0765 B1"), v - b1[i]);
            within += usize::from((v - lda[i]).abs() <= bd);
        }
        o.num(&format!("{q} radii within"), within as f64);
        o.verdict(q, within == 26);
    }
    o
}

fn committed_rows() -> Rows {
    let d = records_dir();
    rows_of(
        &read_rows(&d.join("M_dt.csv")),
        &read_rows(&d.join("M_dt2.csv")),
        &read_rings(&d.join("M_dt_rings.csv")),
        &read_rings(&d.join("M_dt2_rings.csv")),
    )
}

/// H: the rows as first computed (the registered run of the rows test, 2026-11-28, `rows.log`), and their verdicts.
const RECORDED: Option<&[(&str, f64)]> = Some(&[
    ("R rows dt", 5498.0),
    ("R rows dt2", 10996.0),
    ("last ft steps dt", 917.0),
    ("last ft first step dt", 4582.0),
    ("torque mean dt", 2.040217817),
    ("torque half range dt", 0.0004770005),
    ("torque share dt", 0.0002337988111),
    ("torque half range ft1 dt", 29.26274526),
    ("torque half range ft2 dt", 0.0077878905),
    ("torque half range ft3 dt", 0.00293907),
    ("torque half range ft4 dt", 0.0015665685),
    ("torque half range ft5 dt", 0.0008205785),
    ("torque half range ft6 dt", 0.0004770005),
    ("dp_ts mean dt", 80.25432601),
    ("dp_ts half range dt", 0.04289381),
    ("dp_ts share dt", 0.0005344734936),
    ("dp_ts half range ft1 dt", 775.2365015),
    ("dp_ts half range ft2 dt", 0.28358733),
    ("dp_ts half range ft3 dt", 0.320751075),
    ("dp_ts half range ft4 dt", 0.1762936),
    ("dp_ts half range ft5 dt", 0.08284272),
    ("dp_ts half range ft6 dt", 0.04289381),
    ("E-1 share last ft dt", 0.5583424209),
    ("last ft steps dt2", 1833.0),
    ("last ft first step dt2", 9164.0),
    ("torque mean dt2", 2.040512182),
    ("torque half range dt2", 0.0004744705),
    ("torque share dt2", 0.0002325251984),
    ("torque half range ft1 dt2", 58.26167718),
    ("torque half range ft2 dt2", 0.007225037),
    ("torque half range ft3 dt2", 0.0041109235),
    ("torque half range ft4 dt2", 0.0016420525),
    ("torque half range ft5 dt2", 0.0008058925),
    ("torque half range ft6 dt2", 0.0004744705),
    ("dp_ts mean dt2", 80.31403885),
    ("dp_ts half range dt2", 0.04457862),
    ("dp_ts share dt2", 0.0005550538939),
    ("dp_ts half range ft1 dt2", 1610.812745),
    ("dp_ts half range ft2 dt2", 0.38152529),
    ("dp_ts half range ft3 dt2", 0.423893575),
    ("dp_ts half range ft4 dt2", 0.18668981),
    ("dp_ts half range ft5 dt2", 0.085067635),
    ("dp_ts half range ft6 dt2", 0.04457862),
    ("E-1 share last ft dt2", 1.0),
    ("S2 torque", 0.0001442812694),
    ("S2 dp_ts", 0.0007440451564),
    ("torque Richardson", 2.040610304),
    ("dp_ts Richardson", 80.33394313),
    ("Q1 distance", 46.14596115),
    ("Q2 power", 317.5313569),
    ("Q2 distance", 15.76864307),
    ("K torque", -0.05765181796),
    ("K dp_ts", 6.325938851),
    ("measured Euler power, pointwise", 297.6573748),
    ("Q3 e value", 228.9863725),
    ("Q3 e value dt", 228.9025),
    ("Q3 e S", 0.008429814571),
    ("Q3 e T", 0.07089926657),
    ("Q3 e band", 15.51116728),
    ("Q3 e distance", 8.970955548),
    ("Q4 q value", 102.6617238),
    ("Q4 q value dt", 102.4766978),
    ("Q4 q S", 0.0384202267),
    ("Q4 q T", 0.1116142195),
    ("Q4 q band", 15.15354427),
    ("Q4 q distance", 3.465457289),
    ("Pi_s value", 65.91545415),
    ("Pi_s value dt", 65.97622847),
    ("Pi_s S", 0.06202756627),
    ("Pi_s T", 0.02447201056),
    ("Pi_s band", 21.36643405),
    ("Q7 casing value", 69.1137088),
    ("Q7 casing value dt", 69.03096573),
    ("Q7 casing S", 0.0472310064),
    ("Q7 casing T", 0.2684250034),
    ("Q7 casing band", 22.65309048),
    ("Q7 casing distance", 44.7862912),
    ("Pi_s against ADR 0765 B1", 4.206058149),
    ("e against ADR 0765 B1", 2.124904529),
    ("q against ADR 0765 B1", 0.3819998113),
    ("Q5 r0 value", 11.24327607),
    ("Q5 r0 band", 0.8909719811),
    ("Q5 r0 against ADR 0765 B1", -0.025644932),
    ("Q5 r1 value", 11.20129023),
    ("Q5 r1 band", 0.9266508927),
    ("Q5 r1 against ADR 0765 B1", -0.14342677),
    ("Q5 r2 value", 11.3201742),
    ("Q5 r2 band", 0.7238163526),
    ("Q5 r2 against ADR 0765 B1", -0.125621796),
    ("Q5 r3 value", 11.31124488),
    ("Q5 r3 band", 1.320751534),
    ("Q5 r3 against ADR 0765 B1", -0.09615012),
    ("Q5 r4 value", 11.18208704),
    ("Q5 r4 band", 0.600313037),
    ("Q5 r4 against ADR 0765 B1", -0.049413964),
    ("Q5 r5 value", 11.21060477),
    ("Q5 r5 band", 0.5120614945),
    ("Q5 r5 against ADR 0765 B1", -0.035267226),
    ("Q5 r6 value", 10.93865173),
    ("Q5 r6 band", 1.08572317),
    ("Q5 r6 against ADR 0765 B1", 0.05325373),
    ("Q5 r7 value", 10.9687284),
    ("Q5 r7 band", 1.154673057),
    ("Q5 r7 against ADR 0765 B1", 0.045086396),
    ("Q5 r8 value", 10.72780956),
    ("Q5 r8 band", 0.07855384264),
    ("Q5 r8 against ADR 0765 B1", 0.169708562),
    ("Q5 r9 value", 10.74093378),
    ("Q5 r9 band", 0.3505908369),
    ("Q5 r9 against ADR 0765 B1", 0.152709784),
    ("Q5 r10 value", 10.77425161),
    ("Q5 r10 band", 0.1799076785),
    ("Q5 r10 against ADR 0765 B1", 0.239700606),
    ("Q5 r11 value", 10.77864639),
    ("Q5 r11 band", 0.2023504213),
    ("Q5 r11 against ADR 0765 B1", 0.227958394),
    ("Q5 r12 value", 11.01624043),
    ("Q5 r12 band", 0.7874651576),
    ("Q5 r12 against ADR 0765 B1", 0.27265543),
    ("Q5 r13 value", 11.02199054),
    ("Q5 r13 band", 0.7965302073),
    ("Q5 r13 against ADR 0765 B1", 0.267392544),
    ("Q5 r14 value", 11.3738793),
    ("Q5 r14 band", 0.5173817581),
    ("Q5 r14 against ADR 0765 B1", 0.195028298),
    ("Q5 r15 value", 11.368727),
    ("Q5 r15 band", 0.4686306582),
    ("Q5 r15 against ADR 0765 B1", 0.209440996),
    ("Q5 r16 value", 11.64326641),
    ("Q5 r16 band", 1.096779038),
    ("Q5 r16 against ADR 0765 B1", 0.035948412),
    ("Q5 r17 value", 11.62521103),
    ("Q5 r17 band", 1.042751029),
    ("Q5 r17 against ADR 0765 B1", 0.042311026),
    ("Q5 r18 value", 11.62167477),
    ("Q5 r18 band", 0.9986756489),
    ("Q5 r18 against ADR 0765 B1", 0.042592774),
    ("Q5 r19 value", 10.86433655),
    ("Q5 r19 band", 8.150757052),
    ("Q5 r19 against ADR 0765 B1", -0.338725452),
    ("Q5 r20 value", 10.86535536),
    ("Q5 r20 band", 8.169040019),
    ("Q5 r20 against ADR 0765 B1", -0.32716264),
    ("Q5 r21 value", 6.022909378),
    ("Q5 r21 band", 5.20467937),
    ("Q5 r21 against ADR 0765 B1", -0.795360622),
    ("Q5 r22 value", 6.021757502),
    ("Q5 r22 band", 5.184660263),
    ("Q5 r22 against ADR 0765 B1", -0.7912884976),
    ("Q5 r23 value", 1.282054341),
    ("Q5 r23 band", 2.41689088),
    ("Q5 r23 against ADR 0765 B1", -0.377063659),
    ("Q5 r24 value", -0.3894262296),
    ("Q5 r24 band", 3.23014192),
    ("Q5 r24 against ADR 0765 B1", -0.1357112296),
    ("Q5 r25 value", -1.023025004),
    ("Q5 r25 band", 0.9989042844),
    ("Q5 r25 against ADR 0765 B1", -0.048379004),
    ("Q5 radii within", 14.0),
    ("Q6 r0 value", 8.262235662),
    ("Q6 r0 band", 0.2539177325),
    ("Q6 r0 against ADR 0765 B1", 0.1059456624),
    ("Q6 r1 value", 7.903925908),
    ("Q6 r1 band", 0.7331971437),
    ("Q6 r1 against ADR 0765 B1", 0.094390908),
    ("Q6 r2 value", 7.748122846),
    ("Q6 r2 band", 0.4860310741),
    ("Q6 r2 against ADR 0765 B1", 0.079090846),
    ("Q6 r3 value", 7.713819993),
    ("Q6 r3 band", 0.6703590814),
    ("Q6 r3 against ADR 0765 B1", 0.0570179928),
    ("Q6 r4 value", 7.484931867),
    ("Q6 r4 band", 0.1542854177),
    ("Q6 r4 against ADR 0765 B1", 0.0516078668),
    ("Q6 r5 value", 7.451589936),
    ("Q6 r5 band", 0.1247716675),
    ("Q6 r5 against ADR 0765 B1", 0.042528936),
    ("Q6 r6 value", 7.322716632),
    ("Q6 r6 band", 0.122279325),
    ("Q6 r6 against ADR 0765 B1", 0.0541116318),
    ("Q6 r7 value", 7.286931849),
    ("Q6 r7 band", 0.07958155987),
    ("Q6 r7 against ADR 0765 B1", 0.0628478492),
    ("Q6 r8 value", 7.138545011),
    ("Q6 r8 band", 0.3634075225),
    ("Q6 r8 against ADR 0765 B1", 0.0141530114),
    ("Q6 r9 value", 7.099182269),
    ("Q6 r9 band", 1.471446752),
    ("Q6 r9 against ADR 0765 B1", 0.0253972694),
    ("Q6 r10 value", 6.586987522),
    ("Q6 r10 band", 0.2492227788),
    ("Q6 r10 against ADR 0765 B1", -0.073793478),
    ("Q6 r11 value", 6.59379702),
    ("Q6 r11 band", 0.2052525804),
    ("Q6 r11 against ADR 0765 B1", -0.04940598),
    ("Q6 r12 value", 5.97917759),
    ("Q6 r12 band", 1.610848556),
    ("Q6 r12 against ADR 0765 B1", -0.0692374102),
    ("Q6 r13 value", 5.981500857),
    ("Q6 r13 band", 1.320014555),
    ("Q6 r13 against ADR 0765 B1", -0.0465351428),
    ("Q6 r14 value", 5.341910588),
    ("Q6 r14 band", 0.4848798649),
    ("Q6 r14 against ADR 0765 B1", -0.0901044118),
    ("Q6 r15 value", 5.33538312),
    ("Q6 r15 band", 0.5389706822),
    ("Q6 r15 against ADR 0765 B1", -0.0936648802),
    ("Q6 r16 value", 5.097062282),
    ("Q6 r16 band", 0.2867066235),
    ("Q6 r16 against ADR 0765 B1", -0.0120317178),
    ("Q6 r17 value", 5.08623496),
    ("Q6 r17 band", 0.3484125671),
    ("Q6 r17 against ADR 0765 B1", -0.0237690404),
    ("Q6 r18 value", 5.065044967),
    ("Q6 r18 band", 0.4247456797),
    ("Q6 r18 against ADR 0765 B1", -0.0343060326),
    ("Q6 r19 value", 6.551734757),
    ("Q6 r19 band", 4.627054183),
    ("Q6 r19 against ADR 0765 B1", 0.3939337574),
    ("Q6 r20 value", 6.597626319),
    ("Q6 r20 band", 4.515550241),
    ("Q6 r20 against ADR 0765 B1", 0.3923193186),
    ("Q6 r21 value", 10.51384502),
    ("Q6 r21 band", 6.50128523),
    ("Q6 r21 against ADR 0765 B1", 0.712001022),
    ("Q6 r22 value", 10.51532062),
    ("Q6 r22 band", 6.48641835),
    ("Q6 r22 against ADR 0765 B1", 0.705107616),
    ("Q6 r23 value", 11.01856761),
    ("Q6 r23 band", 4.222687195),
    ("Q6 r23 against ADR 0765 B1", 0.471568606),
    ("Q6 r24 value", 10.51217128),
    ("Q6 r24 band", 3.009582323),
    ("Q6 r24 against ADR 0765 B1", 0.379032278),
    ("Q6 r25 value", 10.13814542),
    ("Q6 r25 band", 3.844398708),
    ("Q6 r25 against ADR 0765 B1", 0.341252418),
    ("Q6 radii within", 14.0),
]);
const VERDICTS: Option<&[(&str, bool)]> = Some(&[
    ("R", true),
    ("S1", true),
    ("S2", true),
    ("Q1", false),
    ("Q2", true),
    ("K", true),
    ("Q3", true),
    ("Q4", true),
    ("Q7", false),
    ("Q5", false),
    ("Q6", false),
]);

// ---------------------------------------------------------------- tests

#[test]
fn f870_tm_measures_on_known_signals() {
    // a series of 5 498 steps of dt: its last flow-through time is 917 steps from 4 582; at dt/2, 1 833 from 9 164
    for (dt, n, len, first) in [(H_DT, H_STEPS, 917, 4582), (0.5 * H_DT, 2 * H_STEPS, 1833, 9164)] {
        let t: Vec<f64> = (1..=n).map(|k| k as f64 * dt).collect();
        let w = last_ft(&t);
        assert_eq!(
            (w.len(), w[0] + 1),
            (len, first),
            "the last flow-through time at dt {dt}"
        );
    }
    // mean and half range of 2 + 0.37 sin over whole periods
    let n = 917;
    let sig = |a: f64| -> Vec<f64> {
        (0..n)
            .map(|i| 2.0 + a * (2.0 * PI * 17.0 * i as f64 / n as f64).sin())
            .collect()
    };
    let (m, h) = mean_half(&sig(0.37));
    assert!((m - 2.0).abs() <= 1e-12 && (h / 0.37 - 1.0).abs() <= 1e-3, "{m} {h}");
    // S1: 0.05 % settled; 2.5 % not (N1)
    let (m, h) = mean_half(&sig(0.001));
    assert!(settled(m, h));
    let (m, h) = mean_half(&sig(0.05));
    assert!(!settled(m, h), "N1: 2.5 % called settled");
    // the band terms from ADR 0765's constants
    let un = [
        u_num(B1_SCALARS[0], B2_SCALARS[0]),
        u_num(B1_SCALARS[1], B2_SCALARS[1]),
        u_num(B1_SCALARS[2], B2_SCALARS[2]),
    ];
    for (u, want) in un.iter().zip([14.229, 12.936, 21.280]) {
        assert!((u - want).abs() <= 5e-4, "U_num {u} against {want}");
    }
    let bx = band(u_num(B1_CX[0], B2_CX[0]), S_CX[0], T_CX[0], 0.0);
    let bt = band(u_num(B1_CT[0], B2_CT[0]), S_CT[0], T_CT[0], 0.0);
    assert!(
        (bx - 0.890).abs() <= 5e-4 && (bt - 0.251).abs() <= 5e-4,
        "ADR 0765's band at 0.532: {bx} {bt}"
    );
    let (prof, point) = measured90();
    assert!(
        (point.e - 220.02).abs() <= 5e-3 && (point.q - 99.20).abs() <= 5e-3,
        "{point:?}"
    );
    assert!(((prof.e - point.e).abs() - 1.20).abs() <= 5e-3 && ((prof.q - point.q).abs() - 2.07).abs() <= 5e-3);
    println!(
        "F870TM C1: U_num {un:?}, bands at 0.532 {bx:.6} {bt:.6}, e {:.4} q {:.4}",
        point.e, point.q
    );
}

#[test]
fn f870_tm_rows_from_the_committed_records() {
    let o = committed_rows();
    for (k, v) in &o.keys {
        println!("F870TM ROW {k} = {v:.9e}");
    }
    for (k, v) in &o.verdicts {
        println!("F870TM VERDICT {k} = {v}");
    }
    assert!(o.met("R"), "R: the records are not complete as registered");
    if let Some(rec) = RECORDED {
        assert_eq!(rec.len(), o.keys.len());
        for (k, v) in rec {
            let got = o.get(k);
            assert!(rel(got, *v) <= REC_TOL, "H: {k} {got} against {v}");
        }
    }
    if let Some(ver) = VERDICTS {
        for (k, v) in ver {
            assert_eq!(o.met(k), *v, "H: verdict {k}");
        }
    }
}

#[test]
#[ignore = "known miss, ADR 0831 addendum 2 (B1 march, dt/2): settled (S1, S2 met), but Q1 pressure rise 80.31 Pa against 126.46 +- 29.75 (distance 46.15), Q5 c_x 14 of 26 radii within, Q6 c_t 14 of 26, Q7 casing pressure 69.11 Pa against 113.90 +- 22.65 (distance 44.79); Q2, Q3, Q4 met"]
fn f870_tm_acceptance_known_miss() {
    let o = committed_rows();
    let missed: Vec<&str> = ["S1", "S2", "Q1", "Q2", "Q3", "Q4", "Q5", "Q6", "Q7"]
        .into_iter()
        .filter(|k| !o.met(k))
        .collect();
    assert!(missed.is_empty(), "F-870 acceptance missed: {missed:?}");
}

#[test]
fn f870_tm_short_run_holds_the_record() {
    let (rows, _, failed) = run_march("SR", H_DT, SHORT, &[]);
    assert!(failed.is_none(), "{failed:?}");
    let rec = read_rows(&records_dir().join("M_dt.csv"));
    for (a, b) in rows.iter().zip(&rec[..SHORT]) {
        let pairs = [
            (a.torque, b.torque),
            (a.dp, b.dp),
            (a.res_u, b.res_u),
            (a.dv[0], b.dv[0]),
            (a.dv[4], b.dv[4]),
        ];
        for (x, y) in pairs {
            assert!(rel(x, y) <= REC_TOL, "SR step {}: {x} against {y}", a.step);
        }
    }
    println!(
        "F870TM SR: 20 steps equal to the record within {REC_TOL:e}; step 20 torque {:.9}",
        rows[SHORT - 1].torque
    );
}

#[test]
#[ignore = "heavy record, ADR 0831: M(dt) on B1, 5 498 steps (hours): F870_TM_OUT=<folder> cargo test --release -p rcfd-api --test f870_time_march_b1 f870_tm_march_dt_record -- --ignored --nocapture"]
fn f870_tm_march_dt_record() {
    heavy("M_dt", H_DT, H_STEPS, &SAMPLE_DT);
}

#[test]
#[ignore = "heavy record, ADR 0831: M(dt/2) on B1, 10 996 steps (hours): F870_TM_OUT=<folder> cargo test --release -p rcfd-api --test f870_time_march_b1 f870_tm_march_dt2_record -- --ignored --nocapture"]
fn f870_tm_march_dt2_record() {
    let s: Vec<usize> = SAMPLE_DT.iter().map(|k| 2 * k).collect();
    heavy("M_dt2", 0.5 * H_DT, 2 * H_STEPS, &s);
}
