//! ADR 0768 (F-882): the loss of total pressure of the simulated FAN-01 rotor between the inlet and the measured plane of ADR 0765
//! (x = 90 mm), divided by a rule registered before the run into the parts that arise at the blade surfaces, at hub and casing, in the
//! tip gap and in the passage interior; the same on a mesh B0 finer than B1 in every count and at the wall; the extrapolation of the
//! loss over B2, B1, B0 by the rule of ADR 0763 and what the identity of ADR 0765 then leaves for the installation. The loss is the
//! drop of the flux of rothalpy `I = p + |w|^2 / 2 - omega^2 r^2 / 2` (ADR 0768, "The design"). The case text and the sampling of the
//! loss are those of `f878_fan01_lda_profiles.rs` (ADR 0765), copied without a change of value.

#![allow(clippy::neg_cmp_op_on_partial_ord)] // a NaN must fail the checks
#![allow(clippy::needless_range_loop, clippy::too_many_arguments)]

use rcfd_api::case::Case;
use rcfd_api::sim::Simulation;
use rcfd_fv::geometry::FvGeometry;
use rcfd_mesh::quality::check_mesh;
use rcfd_mesh::Mesh;
use std::collections::HashMap;
use std::f64::consts::PI;
use std::sync::OnceLock;
use std::time::Instant;

type Vec3 = [f64; 3];

const N_BLADES: f64 = 9.0;
const R_HUB: f64 = 0.124;
const R_DUCT: f64 = 0.250;
const R_TIP: f64 = 0.2475;
const RPM: f64 = 1486.0;
const RHO: f64 = 1.2; // ASSUMED (ADR 0075, ADR 0762): the density of the measurement is in no source that was read (F-877)
const Q_DESIGN: f64 = 1.4032;
/// The recorded state of ADR 0762 at 8000 iterations: (torque [N m], pressure rise [Pa]) of B1, B2, B3.
const STATE_0762: [(f64, f64); 3] = [(2.098164, 73.9881), (2.028692, 64.2329), (1.918189, 46.7776)];
/// The loss over the measured span at x = 90 mm of ADR 0765 (its logs, `e - q - Pi`, pointwise) [Pa]: B1, B2, B3.
const L_S_0765: [f64; 3] = [
    226.8614676 - 102.2797242 - 61.70939556,
    231.5264400 - 98.03865670 - 54.73282339,
    240.2413110 - 96.54755323 - 39.35132598,
];
/// The terms of the identity of ADR 0765 (ADR 0768, "Arithmetic done beforehand") [Pa].
const DP_TS_MEAS: f64 = 126.46;
const E_M: f64 = 220.015;
const Q_M: f64 = 99.196;
const U_DATA: f64 = 3.270;
const PI_S_B1: f64 = 61.709396;
const STC_PI_B1: f64 = 0.000804 + 0.008265 + 0.393920;
const CAP: usize = 8000;
const SAMPLE_AT: [usize; 5] = [7000, 7250, 7500, 7750, 8000];
const SHORT: usize = 300;
const STATE_TOL: f64 = 0.01;
const CELLS_B1: f64 = 38184.0;
const CELLS_B2: f64 = 13627.0;
/// Factor of safety and the range of the observed order that makes the extrapolation valid (ADR 0768).
const FS: f64 = 1.25;
const P_RANGE: (f64, f64) = (1.5, 2.5);
/// Region thickness [m] and the two reported variants.
const DELTAS: [f64; 3] = [2.5e-3, 1.25e-3, 5.0e-3];
/// Mesh gates of ADR 0762.
const GATES: [f64; 3] = [70.0, 2.0, 300.0];

/// Counts (n_up, n_chord, n_down, n_side, n_nose, n_span, n_gap) and the wall layers (n_bl, growth, y+).
#[derive(Clone, Copy, Debug)]
struct Spec {
    c: (usize, usize, usize, usize, usize, usize, usize),
    n_bl: usize,
    growth: f64,
    y_plus: f64,
}
const B1: Spec = Spec {
    c: (12, 24, 12, 15, 6, 21, 10),
    n_bl: 2,
    growth: 1.2,
    y_plus: 30.0,
};
const B2: Spec = Spec {
    c: (8, 17, 8, 11, 4, 15, 7),
    ..B1
};
const B3: Spec = Spec {
    c: (6, 12, 6, 8, 3, 10, 5),
    ..B1
};
/// B0 (ADR 0768): B1 refined by 1.41 in every count and at the wall.
fn b0() -> Spec {
    Spec {
        c: (17, 34, 17, 21, 8, 30, 14),
        n_bl: 3,
        growth: 1.2f64.powf(1.0 / 1.41),
        y_plus: 30.0 / 1.41,
    }
}
/// The corrected B0 (ADR 0768, amendment 1): the counts and wall layers of B0 with the first cell of the family, `y+ = 30`.
fn b0c() -> Spec {
    Spec { y_plus: 30.0, ..b0() }
}

// ---------------------------------------------------------------- the case text of ADR 0762 / ADR 0765

const N_THETA: usize = 360;
const PITCH: f64 = 2.0 * PI / 9.0;
const FLUID_TOL: f64 = 1.0e-4;
const X_ROOT_LE_MM: f64 = 21.45;
const PLANES: [f64; 3] = [88.7, 90.0, 91.3];
const RADIUS_NORM: [f64; 26] = [
    0.532, 0.552, 0.572, 0.592, 0.612, 0.632, 0.652, 0.672, 0.692, 0.712, 0.732, 0.752, 0.772, 0.792, 0.812, 0.832,
    0.852, 0.872, 0.892, 0.912, 0.932, 0.952, 0.960, 0.968, 0.976, 0.984,
];

fn omega() -> f64 {
    RPM * 2.0 * PI / 60.0
}

fn annulus_area() -> f64 {
    PI * (R_DUCT * R_DUCT - R_HUB * R_HUB)
}

fn z_of(x_mm: f64) -> f64 {
    0.1 + (x_mm - X_ROOT_LE_MM) * 1e-3
}

fn radii() -> Vec<f64> {
    RADIUS_NORM.iter().map(|v| v * R_DUCT).collect()
}

fn annulus_radii() -> Vec<f64> {
    (0..126).map(|i| R_HUB + 1e-3 * (i as f64 + 0.5)).collect()
}

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

/// The mesh section of `f878_fan01_lda_profiles.rs`; for B1, B2, B3 the same bytes (n_bl 2, growth 1.2, y+ 30).
fn mesh_toml(s: Spec) -> String {
    let rows = sections();
    let (x0, t0) = (rows[0][6], rows[0][7]);
    let ref_speed = (Q_DESIGN / annulus_area()).hypot(omega() * 0.186);
    let (n_up, n_chord, n_down, n_side, n_nose, n_span, n_gap) = s.c;
    let (n_bl, growth, y_plus) = (s.n_bl, s.growth, s.y_plus);
    format!(
        "[mesh]\ngenerator = \"axial_fan_sector\"\n[mesh.fan]\nn_blades = 9\nr_hub = 0.124\nr_tip = 0.2475\ntip_clearance = 0.0025\naxial_extent = 0.4\nz_le_root = 0.1\nfamily = \"naca4\"\n\
station_radius = {}\nstation_chord = {}\nstation_stagger = {}\nstation_camber = {}\nstation_camber_pos = {}\nstation_thickness = {}\nstation_sweep = {}\nstation_lean = {}\n\
topology = \"c_grid\"\nc_grid_construction = \"camber_seam_h\"\nn_up = {n_up}\nn_chord = {n_chord}\nn_down = {n_down}\nn_side = {n_side}\nn_nose = {n_nose}\nn_core = 1\nfront_gap = 0.3\nn_span = {n_span}\nn_gap = {n_gap}\nn_bl = {n_bl}\ngrowth = {growth:?}\n\
first_cell_height = 0.0\ny_plus = {y_plus:?}\nref_speed = {ref_speed:?}\nnu = 1.5e-5\n",
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

fn case_toml(s: Spec, cap: usize) -> String {
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
        mesh_toml(s)
    )
}

fn functionals(sim: &Simulation) -> (f64, f64) {
    let u_in = Q_DESIGN / annulus_area();
    let mon = |k: &str| sim.monitors.last(k).unwrap_or(f64::NAN);
    (
        -N_BLADES * (mon("m_blade") + mon("m_hub")),
        RHO * (mon("p_out") - (mon("p_in") + 0.5 * u_in * u_in)),
    )
}

// ---------------------------------------------------------------- the sampling of ADR 0765 (copied from f878_fan01_lda_profiles.rs)

struct CellGeo {
    faces: Vec<(Vec3, Vec3)>,
    r: (f64, f64),
    z: (f64, f64),
}

struct Locator {
    cells: Vec<CellGeo>,
    centres: Vec<Vec3>,
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

    /// The largest signed distance of `p` from the face planes of cell `c` (at most `FLUID_TOL`: the point is in the cell).
    fn depth(&self, c: usize, p: Vec3) -> f64 {
        self.cells[c]
            .faces
            .iter()
            .map(|(x, n)| (p[0] - x[0]) * n[0] + (p[1] - x[1]) * n[1] + (p[2] - x[2]) * n[2])
            .fold(f64::NEG_INFINITY, f64::max)
    }

    fn ring(&self, r: f64, z: f64) -> Vec<Option<usize>> {
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
                        let d = self.depth(c, p);
                        if d < best.0 {
                            best = (d, c);
                        }
                    }
                }
                (best.0 <= FLUID_TOL).then_some(best.1)
            })
            .collect()
    }

    fn plane(&self, x_mm: f64, r: &[f64]) -> Vec<Vec<Option<usize>>> {
        r.iter().map(|&ri| self.ring(ri, z_of(x_mm))).collect()
    }
}

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

fn ring_mean(loc: &Locator, cells: &[Option<usize>], r: f64, w: &[Vec3], p: &[f64]) -> Ring {
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

fn trapezoid(r: &[f64], f: &[f64]) -> f64 {
    (0..r.len() - 1)
        .map(|i| 0.5 * (2.0 * PI * r[i] * f[i] + 2.0 * PI * r[i + 1] * f[i + 1]) * (r[i + 1] - r[i]))
        .sum()
}

fn midpoint(r: &[f64], f: &[f64]) -> f64 {
    r.iter().zip(f).map(|(ri, fi)| 2.0 * PI * ri * fi * 1e-3).sum()
}

/// `L = e - q - Pi` of ADR 0765 (pointwise reading) from the rings of a plane; `weighted` for the whole annulus.
fn sampled_loss(rings: &[Ring], p_in: f64, int: fn(&[f64], &[f64]) -> f64, weighted: bool) -> f64 {
    let r: Vec<f64> = rings.iter().map(|m| m.r).collect();
    let col = |f: fn(&Ring) -> f64| -> Vec<f64> {
        rings
            .iter()
            .map(|m| if weighted { m.fluid * f(m) } else { f(m) })
            .collect()
    };
    let flow = int(&r, &col(|m| m.cx));
    let arm: Vec<f64> = rings
        .iter()
        .map(|m| m.r * if weighted { m.fluid * m.cxct } else { m.cxct })
        .collect();
    let e = RHO * omega() * int(&r, &arm) / flow;
    let q = RHO * int(&r, &col(|m| m.ke)) / flow;
    let u_in = Q_DESIGN / annulus_area();
    let pi = RHO * (int(&r, &col(|m| m.pcx)) / flow - p_in - 0.5 * u_in * u_in);
    e - q - pi
}

// ---------------------------------------------------------------- the budget of ADR 0768

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: Vec3, b: Vec3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn norm(a: Vec3) -> f64 {
    dot(a, a).sqrt()
}
fn rot_z(p: Vec3, t: f64) -> Vec3 {
    let (s, c) = t.sin_cos();
    [c * p[0] - s * p[1], s * p[0] + c * p[1], p[2]]
}
fn mat(m: &[[f64; 3]; 3], v: Vec3) -> Vec3 {
    [dot(m[0], v), dot(m[1], v), dot(m[2], v)]
}

/// Distance from `p` to the triangle (a, b, c): the closest point by the region of the triangle's plane it falls in (vertex, edge or
/// face), from the barycentric coordinates of the projection.
fn point_triangle_distance(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> f64 {
    let (ab, ac, ap) = (sub(b, a), sub(c, a), sub(p, a));
    let (d1, d2) = (dot(ab, ap), dot(ac, ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return norm(ap);
    }
    let bp = sub(p, b);
    let (d3, d4) = (dot(ab, bp), dot(ac, bp));
    if d3 >= 0.0 && d4 <= d3 {
        return norm(bp);
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return norm(sub(ap, [v * ab[0], v * ab[1], v * ab[2]]));
    }
    let cp = sub(p, c);
    let (d5, d6) = (dot(ab, cp), dot(ac, cp));
    if d6 >= 0.0 && d5 <= d6 {
        return norm(cp);
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return norm(sub(ap, [w * ac[0], w * ac[1], w * ac[2]]));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        let bc = sub(c, b);
        return norm(sub(bp, [w * bc[0], w * bc[1], w * bc[2]]));
    }
    let den = 1.0 / (va + vb + vc);
    let (v, w) = (vb * den, vc * den);
    let q = [
        a[0] + ab[0] * v + ac[0] * w,
        a[1] + ab[1] * v + ac[1] * w,
        a[2] + ab[2] * v + ac[2] * w,
    ];
    norm(sub(p, q))
}

/// Distance of every cell centre from the faceted blade (the sector's blade and its images at plus and minus one pitch), exact where
/// it is at most `cut`, infinite otherwise. Triangles are binned on a grid of `cut`.
fn blade_distance(mesh: &Mesh, cut: f64) -> Vec<f64> {
    let blade = mesh.patch("blade").expect("patch blade");
    let (pts, cf) = (mesh.points(), mesh.face_centres());
    let mut tris: Vec<[Vec3; 3]> = Vec::new();
    for f in blade.faces() {
        let ids = mesh.face_points(f);
        for k in 0..ids.len() {
            let (a, b) = (pts[ids[k]], pts[ids[(k + 1) % ids.len()]]);
            for t in [0.0, PITCH, -PITCH] {
                tris.push([rot_z(cf[f], t), rot_z(a, t), rot_z(b, t)]);
            }
        }
    }
    let key = |x: f64| (x / cut).floor() as i64;
    let mut grid: HashMap<(i64, i64, i64), Vec<usize>> = HashMap::new();
    for (i, t) in tris.iter().enumerate() {
        let lo = [0, 1, 2].map(|k| key(t.iter().map(|p| p[k]).fold(f64::INFINITY, f64::min) - cut));
        let hi = [0, 1, 2].map(|k| key(t.iter().map(|p| p[k]).fold(f64::NEG_INFINITY, f64::max) + cut));
        for a in lo[0]..=hi[0] {
            for b in lo[1]..=hi[1] {
                for c in lo[2]..=hi[2] {
                    grid.entry((a, b, c)).or_default().push(i);
                }
            }
        }
    }
    mesh.cell_centres()
        .iter()
        .map(|&p| {
            let d = grid
                .get(&(key(p[0]), key(p[1]), key(p[2])))
                .map_or(f64::INFINITY, |list| {
                    list.iter()
                        .map(|&i| point_triangle_distance(p, tris[i][0], tris[i][1], tris[i][2]))
                        .fold(f64::INFINITY, f64::min)
                });
            if d <= cut {
                d
            } else {
                f64::INFINITY
            }
        })
        .collect()
}

/// The fixed geometry of the budget on one mesh: the cells of `V` for the three planes, the regions (blade surfaces, hub and casing,
/// tip gap, passage interior) for the three thicknesses at x = 90 mm and for 2.5 mm at the other two planes, and the seam pairs.
struct BudgetGeo {
    in_v: [Vec<bool>; 3],
    /// regions[plane][delta] = four lists of cells; at planes 0 and 2 only delta index 0
    regions: Vec<Vec<[Vec<usize>; 4]>>,
    /// (seam_lo face, partner face, rotation of the partner's vectors into the seam_lo frame)
    seams: Vec<(usize, usize, [[f64; 3]; 3])>,
    seam_hi: Vec<bool>,
    gap_cells: usize,
}

fn tip_gap_cells(mesh: &Mesh, loc: &Locator) -> Vec<bool> {
    let rq = R_TIP - 1.0e-3;
    let layer: Vec<usize> = (0..mesh.n_cells())
        .filter(|&c| {
            let g = &loc.cells[c];
            rq >= g.r.0 - 1e-3 && rq <= g.r.1 + 1e-3
        })
        .collect();
    mesh.cell_centres()
        .iter()
        .map(|&x| {
            if x[0].hypot(x[1]) <= R_TIP {
                return false;
            }
            let t = x[1].atan2(x[0]);
            let mut best = f64::INFINITY;
            for &c in &layer {
                let g = &loc.cells[c];
                if x[2] < g.z.0 - 1e-3 || x[2] > g.z.1 + 1e-3 {
                    continue;
                }
                for j in [0.0, PITCH, -PITCH] {
                    let p = [rq * (t + j).cos(), rq * (t + j).sin(), x[2]];
                    best = best.min(loc.depth(c, p));
                }
            }
            best > FLUID_TOL
        })
        .collect()
}

/// The region rule of ADR 0768 for the cells of `in_v`.
fn regions_of(mesh: &Mesh, in_v: &[bool], gap: &[bool], d_b: &[f64], delta: f64) -> [Vec<usize>; 4] {
    let mut out: [Vec<usize>; 4] = Default::default();
    for (c, x) in mesh.cell_centres().iter().enumerate() {
        if !in_v[c] {
            continue;
        }
        let r = x[0].hypot(x[1]);
        let d_hc = (r - R_HUB).min(R_DUCT - r);
        let k = if gap[c] {
            2
        } else if d_b[c] <= delta && d_b[c] <= d_hc {
            0
        } else if d_hc <= delta {
            1
        } else {
            3
        };
        out[k].push(c);
    }
    out
}

impl BudgetGeo {
    fn new(g: &FvGeometry, loc: &Locator) -> Self {
        let mesh = &g.mesh;
        let gap = tip_gap_cells(mesh, loc);
        let d_b = blade_distance(mesh, 5.0e-3);
        let in_v = PLANES.map(|x| {
            mesh.cell_centres()
                .iter()
                .map(|c| c[2] < z_of(x))
                .collect::<Vec<bool>>()
        });
        let regions = (0..3)
            .map(|i| {
                let ds: &[f64] = if i == 1 { &DELTAS } else { &DELTAS[..1] };
                ds.iter().map(|&d| regions_of(mesh, &in_v[i], &gap, &d_b, d)).collect()
            })
            .collect();
        let n_int = g.n_internal;
        let mut seams = Vec::new();
        let mut seam_hi = vec![false; mesh.n_faces()];
        let lo = mesh
            .patches()
            .iter()
            .position(|p| p.name == "seam_lo")
            .expect("seam_lo");
        let map = g.periodic_partner[lo].as_ref().expect("periodic seam_lo");
        let rot = g.periodic_rot[lo].unwrap_or([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
        for (i, f) in mesh.patches()[lo].faces().enumerate() {
            seams.push((f, map[i], rot));
            seam_hi[map[i]] = true;
        }
        assert!(seams.iter().all(|&(f, p, _)| f >= n_int && p >= n_int));
        BudgetGeo {
            in_v,
            regions,
            seams,
            seam_hi,
            gap_cells: gap.iter().filter(|&&b| b).count(),
        }
    }
}

/// The state the budget reads: relative velocity and kinematic pressure of the cells and the boundary faces (index `f - n_internal`),
/// the face flux, the face viscosity and the inlet total pressure (kinematic).
struct State<'a> {
    w: &'a [Vec3],
    p: &'a [f64],
    bw: &'a [Vec3],
    bp: &'a [f64],
    phi: &'a [f64],
    nu: &'a [f64],
    p0: f64,
}

/// The parts in Pa (blade surfaces, hub and casing, tip gap, passage interior), their convective shares, the sub-parts (hub, casing,
/// interior upstream of z = 0.1 m), the whole by route A (sum over the regions) and route B (sum over the boundary of `V`), the inlet
/// volume flow and the largest seam flux defect.
#[derive(Clone, Debug, Default)]
struct Budget {
    parts: [f64; 4],
    conv: [f64; 4],
    hub: f64,
    casing: f64,
    upstream: f64,
    whole_a: f64,
    whole_b: f64,
    q_in: f64,
    seam_defect: f64,
}

fn rothalpy(p: f64, w: Vec3, x: Vec3, p0: f64) -> f64 {
    let r2 = x[0] * x[0] + x[1] * x[1];
    p + 0.5 * dot(w, w) - 0.5 * omega() * omega() * r2 - p0
}

fn budget(g: &FvGeometry, bg: &BudgetGeo, in_v: &[bool], regions: &[Vec<usize>; 4], s: &State) -> Budget {
    let mesh = &g.mesh;
    let (own, nei, cc, cf) = (mesh.owner(), mesh.neighbour(), mesh.cell_centres(), mesh.face_centres());
    let n_int = g.n_internal;
    let ic: Vec<f64> = (0..mesh.n_cells())
        .map(|c| rothalpy(s.p[c], s.w[c], cc[c], s.p0))
        .collect();
    let (mut conv, mut work) = (vec![0.0; mesh.n_cells()], vec![0.0; mesh.n_cells()]);
    let mut route_b = 0.0;
    for f in 0..n_int {
        let (o, n) = (own[f], nei[f]);
        if !in_v[o] && !in_v[n] {
            continue;
        }
        let l = g.weights[f];
        let i_f = l * ic[o] + (1.0 - l) * ic[n];
        let t = -s.phi[f] * i_f;
        match (in_v[o], in_v[n]) {
            (true, true) => {
                let wf = [0, 1, 2].map(|k| l * s.w[o][k] + (1.0 - l) * s.w[n][k]);
                let wk = s.nu[f] * g.lap_coeff[f] * dot(sub(s.w[n], s.w[o]), wf);
                conv[o] += t;
                conv[n] -= t;
                work[o] += wk;
                work[n] -= wk;
            }
            (true, false) => {
                conv[o] += t;
                route_b += t;
            }
            _ => {
                conv[n] -= t;
                route_b -= t;
            }
        }
    }
    let mut seam_defect = 0.0f64;
    let mut is_seam = vec![false; mesh.n_faces()];
    for &(f, fp, rot) in &bg.seams {
        is_seam[f] = true;
        let (o, op) = (own[f], own[fp]);
        seam_defect = seam_defect.max((s.phi[f] + s.phi[fp]).abs());
        if !in_v[o] && !in_v[op] {
            continue;
        }
        let (dn_o, dn_p) = (g.dn[f - n_int], g.dn[fp - n_int]);
        let l = dn_p / (dn_o + dn_p);
        let i_f = l * ic[o] + (1.0 - l) * ic[op];
        let t = -s.phi[f] * i_f;
        match (in_v[o], in_v[op]) {
            (true, true) => {
                let wp = mat(&rot, s.w[op]);
                let wf = [0, 1, 2].map(|k| l * s.w[o][k] + (1.0 - l) * wp[k]);
                let a = mesh.face_area(f) / (dn_o + dn_p);
                let wk = s.nu[f] * a * dot(sub(wp, s.w[o]), wf);
                conv[o] += t;
                conv[op] -= t;
                work[o] += wk;
                work[op] -= wk;
            }
            (true, false) => {
                conv[o] += t;
                route_b += t;
            }
            _ => {
                conv[op] -= t;
                route_b -= t;
            }
        }
    }
    let inlet = mesh.patch("inlet").expect("inlet").faces();
    let mut q_in = 0.0;
    for f in n_int..mesh.n_faces() {
        if is_seam[f] || bg.seam_hi[f] || !in_v[own[f]] {
            continue;
        }
        let i_b = rothalpy(s.bp[f - n_int], s.bw[f - n_int], cf[f], s.p0);
        let t = -s.phi[f] * i_b;
        conv[own[f]] += t;
        route_b += t;
        if inlet.contains(&f) {
            q_in -= s.phi[f];
        }
    }
    let mut b = Budget {
        q_in,
        seam_defect,
        ..Budget::default()
    };
    let per = RHO / q_in;
    for k in 0..4 {
        for &c in &regions[k] {
            b.parts[k] += per * (conv[c] + work[c]);
            b.conv[k] += per * conv[c];
            if k == 1 {
                let r = cc[c][0].hypot(cc[c][1]);
                if r - R_HUB < R_DUCT - r {
                    b.hub += per * (conv[c] + work[c]);
                } else {
                    b.casing += per * (conv[c] + work[c]);
                }
            }
            if k == 3 && cc[c][2] < 0.1 {
                b.upstream += per * (conv[c] + work[c]);
            }
        }
    }
    b.whole_a = b.parts.iter().sum();
    b.whole_b = per * route_b;
    b
}

fn closes(b: &Budget) -> bool {
    (b.whole_a - b.whole_b).abs() <= 1e-8 * b.whole_b.abs()
}

// ---------------------------------------------------------------- the extrapolation of ADR 0768

/// The observed order of `f1` (coarse), `f2`, `f3` (fine) on the ratios `r21`, `r32` (ADR 0763): the root of
/// `r32^p (r21^p - 1) / (r32^p - 1) = (f1 - f2) / (f2 - f3)` by bisection on [0.01, 20], if the sequence is monotone and it has one.
fn observed_order(f1: f64, f2: f64, f3: f64, r21: f64, r32: f64) -> Option<f64> {
    let t = (f1 - f2) / (f2 - f3);
    let g = |p: f64| r32.powf(p) * (r21.powf(p) - 1.0) / (r32.powf(p) - 1.0);
    let (mut lo, mut hi) = (0.01, 20.0);
    if !((f1 - f2) * (f2 - f3) > 0.0 && g(lo) < t && t < g(hi)) {
        return None;
    }
    for _ in 0..200 {
        let m = 0.5 * (lo + hi);
        if g(m) < t {
            lo = m
        } else {
            hi = m
        }
    }
    Some(0.5 * (lo + hi))
}

/// The extrapolation of ADR 0768 with its band and the share of the installation.
#[derive(Clone, Copy, Debug)]
struct Extrapolation {
    p: f64,
    valid: bool,
    l0: f64,
    l_order1: f64,
    l_order2: f64,
    u_l: f64,
    rec_m: f64,
    i0: f64,
    u_i: f64,
}

fn extrapolate(f1: f64, f2: f64, f3: f64, r21: f64, r32: f64, s: f64, t: f64) -> Extrapolation {
    let p = observed_order(f1, f2, f3, r21, r32);
    let pv = p.unwrap_or(f64::NAN);
    let valid = p.is_some_and(|p| p >= P_RANGE.0 && p <= P_RANGE.1);
    let l0 = f3 + (f3 - f2) / (r32.powf(pv) - 1.0);
    let pb = if pv.is_finite() { pv.clamp(1.0, 2.0) } else { 1.0 };
    let u_l = FS * (f3 - f2).abs() / (r32.powf(pb) - 1.0) + s + t;
    let d = DP_TS_MEAS - STATE_0762[0].1;
    let r0 = (E_M - Q_M) - l0 - PI_S_B1;
    Extrapolation {
        p: pv,
        valid,
        l0,
        l_order1: f3 + (f3 - f2) / (r32 - 1.0),
        l_order2: f3 + (f3 - f2) / (r32 * r32 - 1.0),
        u_l,
        rec_m: DP_TS_MEAS - ((E_M - Q_M) - l0),
        i0: d - r0,
        u_i: u_l + U_DATA + STC_PI_B1,
    }
}

// ---------------------------------------------------------------- the run

#[derive(Clone, Debug)]
struct Sample {
    iteration: usize,
    torque: f64,
    dp_ts: f64,
    /// `L_s` at x = 88.7, 90, 91.3 mm and `L_ann` at x = 90 mm
    l_s: [f64; 3],
    l_ann: f64,
    /// at x = 90 mm for the three thicknesses, then at x = 88.7 and 91.3 mm for 2.5 mm
    budgets: Vec<Budget>,
}

#[derive(Clone, Debug)]
struct Outcome {
    cells: usize,
    iterations: usize,
    torque: f64,
    dp_ts: f64,
    stop: Option<String>,
    samples: Vec<Sample>,
}

fn take_sample(
    sim: &Simulation,
    n: usize,
    loc: &Locator,
    bg: &BudgetGeo,
    planes: &[Vec<Vec<Option<usize>>>],
    ann: &[Vec<Option<usize>>],
) -> Sample {
    let g = &sim.geom;
    let (r, ra) = (radii(), annulus_radii());
    let (w, p) = (&sim.velocity().values, &sim.pressure().values);
    let p_in = sim.monitors.last("p_in").unwrap_or(f64::NAN);
    let rings = |pl: &[Vec<Option<usize>>], rr: &[f64]| -> Vec<Ring> {
        pl.iter().zip(rr).map(|(c, &ri)| ring_mean(loc, c, ri, w, p)).collect()
    };
    let l_s = [0, 1, 2].map(|i| sampled_loss(&rings(&planes[i], &r), p_in, trapezoid, false));
    let l_ann = sampled_loss(&rings(ann, &ra), p_in, midpoint, true);
    let (bw, bp) = (sim.velocity().boundary_values(g), sim.pressure_face_values());
    let nu = sim
        .momentum_face_viscosity()
        .unwrap_or_else(|| vec![1.5e-5; g.mesh.n_faces()]);
    let u_in = Q_DESIGN / annulus_area();
    let st = State {
        w,
        p,
        bw: &bw,
        bp: &bp,
        phi: sim.flux(),
        nu: &nu,
        p0: p_in + 0.5 * u_in * u_in,
    };
    let mut budgets: Vec<Budget> = bg.regions[1]
        .iter()
        .map(|reg| budget(g, bg, &bg.in_v[1], reg, &st))
        .collect();
    for i in [0, 2] {
        budgets.push(budget(g, bg, &bg.in_v[i], &bg.regions[i][0], &st));
    }
    let (torque, dp_ts) = functionals(sim);
    Sample {
        iteration: n,
        torque,
        dp_ts,
        l_s,
        l_ann,
        budgets,
    }
}

fn print_sample(label: &str, s: &Sample) {
    println!(
        "F882S {label} {} torque {:.9e} dp_ts {:.9e} L_s88.7 {:.9e} L_s90 {:.9e} L_s91.3 {:.9e} L_ann90 {:.9e}",
        s.iteration, s.torque, s.dp_ts, s.l_s[0], s.l_s[1], s.l_s[2], s.l_ann
    );
    let tags = ["90 2.5", "90 1.25", "90 5", "88.7 2.5", "91.3 2.5"];
    for (t, b) in tags.iter().zip(&s.budgets) {
        println!(
            "F882B {label} {} {t} whole_a {:.9e} whole_b {:.9e} blade {:.9e} hubcasing {:.9e} gap {:.9e} interior {:.9e} \
conv {:.9e} {:.9e} {:.9e} {:.9e} hub {:.9e} casing {:.9e} upstream {:.9e} q_in {:.9e} seam {:.3e} closes {}",
            s.iteration,
            b.whole_a,
            b.whole_b,
            b.parts[0],
            b.parts[1],
            b.parts[2],
            b.parts[3],
            b.conv[0],
            b.conv[1],
            b.conv[2],
            b.conv[3],
            b.hub,
            b.casing,
            b.upstream,
            b.q_in * N_BLADES,
            b.seam_defect,
            closes(b)
        );
    }
}

/// The one path of every solving test: the design point on the mesh `s` for at most `cap` iterations, the loss sampled and divided
/// at the iterations `sample_at`. No wall-clock stop; the stops are those of ADR 0762.
fn run_design_point(label: &str, s: Spec, cap: usize, sample_at: &[usize]) -> Outcome {
    let case = Case::parse_toml(&case_toml(s, cap)).unwrap_or_else(|e| panic!("{label}: {e:?}"));
    let mut sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{label}: {e:?}"));
    let cells = sim.geom.mesh.n_cells();
    let t0 = Instant::now();
    let loc = Locator::new(&sim.geom.mesh);
    let bg = BudgetGeo::new(&sim.geom, &loc);
    let planes: Vec<Vec<Vec<Option<usize>>>> = PLANES.iter().map(|&x| loc.plane(x, &radii())).collect();
    let ann = loc.plane(90.0, &annulus_radii());
    let counts: Vec<Vec<usize>> = bg.regions[1]
        .iter()
        .map(|r| r.iter().map(|v| v.len()).collect())
        .collect();
    println!(
        "F882 {label}: {cells} cells; cells of V {}; tip-gap cells {}; region cells (blade, hub and casing, gap, interior) at delta 2.5, 1.25, 5 mm {counts:?}; set-up {:.1} min",
        bg.in_v[1].iter().filter(|&&b| b).count(),
        bg.gap_cells,
        t0.elapsed().as_secs_f64() / 60.0
    );
    // where the staircase of the plane faces of V lies (x = 90 mm): the least, the largest and the flux-weighted z of their centres
    {
        let (own, nei, cf) = (
            sim.geom.mesh.owner(),
            sim.geom.mesh.neighbour(),
            sim.geom.mesh.face_centres(),
        );
        let v = &bg.in_v[1];
        let zs: Vec<f64> = (0..sim.geom.n_internal)
            .filter(|&f| v[own[f]] != v[nei[f]])
            .map(|f| cf[f][2])
            .collect();
        println!(
            "F882 {label}: {} plane faces at z from {:.5} to {:.5} m (the plane z = {:.5} m)",
            zs.len(),
            zs.iter().cloned().fold(f64::INFINITY, f64::min),
            zs.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            z_of(90.0)
        );
    }
    let (mut n, mut stop) = (0usize, None);
    let mut samples = Vec::new();
    while n < cap && !sim.is_done() {
        let st = sim.step();
        n += 1;
        if n % 500 == 0 || n <= 3 || n == 100 {
            let (a, b) = functionals(&sim);
            println!(
                "F882 {label} it {n}: continuity {:.3e} res_u {:.3e} res_p {:.3e} torque {a:.6} dp_ts {b:.4} ({:.1} min)",
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
            let s = take_sample(&sim, n, &loc, &bg, &planes, &ann);
            print_sample(label, &s);
            samples.push(s);
        }
    }
    if stop.is_none() {
        stop = sim.failure().map(|f| format!("failure after {n} iterations: {f}"));
    }
    let (torque, dp_ts) = functionals(&sim);
    println!(
        "F882 {label} RESULT: {cells} cells, {n} iterations, {:.1} min; torque {torque:.6} N m, dp_ts {dp_ts:.4} Pa; is_converged {}; stop {stop:?}",
        t0.elapsed().as_secs_f64() / 60.0,
        stop.is_none() && sim.is_converged(),
    );
    Outcome {
        cells,
        iterations: n,
        torque,
        dp_ts,
        stop,
        samples,
    }
}

fn holds(a: f64, b: f64) -> bool {
    (a - b).abs() <= STATE_TOL * b.abs()
}

/// `S` and `T` of `L_s` at x = 90 mm over the samples of a run (ADR 0765): half the range over the iterations, half over the planes.
fn spread(o: &Outcome) -> (f64, f64) {
    let v: Vec<f64> = o.samples.iter().map(|s| s.l_s[1]).collect();
    let s =
        0.5 * (v.iter().cloned().fold(f64::NEG_INFINITY, f64::max) - v.iter().cloned().fold(f64::INFINITY, f64::min));
    let last = o.samples.last().unwrap().l_s;
    let t = 0.5
        * (last.iter().cloned().fold(f64::NEG_INFINITY, f64::max) - last.iter().cloned().fold(f64::INFINITY, f64::min));
    (s, t)
}

// ---------------------------------------------------------------- the recorded values (ADR 0768, addendum 1)

/// B3 at iteration 300: `L_s` at x = 90 mm, the whole of the budget and the four parts at delta 2.5 mm [Pa] (S1).
const B3_SHORT_RECORD: [f64; 6] = [104.211633, 86.591950, 20.095895, 2.812707, 1.916770, 61.766578];
/// B0: cell count, and non-orthogonality [deg], skewness, aspect ratio (S3).
const B0_CELLS: usize = 106692;
const B0_QUALITY_RECORD: [f64; 3] = [88.074, 14.5937, 323.08];
/// The corrected B0 (amendment 1): non-orthogonality [deg], skewness, aspect ratio; its cell count is `B0_CELLS`.
const B0C_QUALITY_RECORD: [f64; 3] = [83.687, 2.5641, 428.08];
/// The corrected B0 at 8000 iterations: torque [N m], pressure rise, `L_s`, the whole and the four parts [Pa]; and `S`, `T` of `L_s`.
const B0_STATE: [f64; 8] = [
    2.145170, 82.655991, 56.780989, 52.360688, 16.877113, 3.460321, 1.615084, 30.408170,
];
const B0_S_T: [f64; 2] = [0.007815, 0.755584];
/// The recorded extrapolation: validity, and p, L_0, U_L, I_0, U_I.
const EXTRAPOLATION_VALID_RECORD: bool = false;
const EXTRAPOLATION_RECORD: [f64; 5] = [2.785548, 52.965004, 8.502893, 46.327300, 12.175882];

fn extrapolation_of(l_b0: f64, cells_b0: f64, s: f64, t: f64) -> Extrapolation {
    extrapolate(
        L_S_0765[1],
        L_S_0765[0],
        l_b0,
        (CELLS_B1 / CELLS_B2).cbrt(),
        (cells_b0 / CELLS_B1).cbrt(),
        s,
        t,
    )
}

// ---------------------------------------------------------------- run by the gate

/// V1 to V4 (ADR 0768): the distance to a triangle, the partition of `V` into the regions, a uniform state and a random state of the
/// budget on the B3 mesh, and the observed order of an exact power law on unequal ratios.
#[test]
fn f882_rules_closed_forms() {
    // V1: the triangle (0,0,0), (1,0,0), (0,1,0)
    let (a, b, c) = ([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    let cases = [
        ([0.2, 0.3, 0.7], 0.7),
        ([0.5, -2.0, 1.0], 5.0f64.sqrt()),
        ([-3.0, -4.0, 0.0], 5.0),
        ([1.0, 1.0, 0.0], 0.5f64.sqrt()),
    ];
    for (p, want) in cases {
        let d = point_triangle_distance(p, a, b, c);
        assert!((d - want).abs() <= 1e-12, "V1 {p:?}: {d} against {want}");
    }
    // V4: f = 2 + 3 h^1.7 on h3 = 1, h2 = 1.41, h1 = 1.41 * 1.40981
    let (r21, r32) = (1.40981, 1.41);
    let f = |h: f64| 2.0 + 3.0 * h.powf(1.7);
    let p = observed_order(f(r32 * r21), f(r32), f(1.0), r21, r32).expect("a root");
    let l0 = f(1.0) + (f(1.0) - f(r32)) / (r32.powf(p) - 1.0);
    println!("V4 p {p}, Richardson value {l0}");
    assert!((p / 1.7 - 1.0).abs() <= 1e-9 && (l0 / 2.0 - 1.0).abs() <= 1e-9);
    // V2, V3 on the B3 mesh
    let case = Case::parse_toml(&case_toml(B3, CAP)).unwrap_or_else(|e| panic!("{e:?}"));
    let sim = Simulation::new(&case).unwrap_or_else(|e| panic!("{e:?}"));
    let g = &sim.geom;
    let mesh = &g.mesh;
    let loc = Locator::new(mesh);
    let bg = BudgetGeo::new(g, &loc);
    let cc = mesh.cell_centres();
    for (i, per_delta) in bg.regions.iter().enumerate() {
        for reg in per_delta {
            let mut count = vec![0usize; mesh.n_cells()];
            for list in reg {
                for &c in list {
                    count[c] += 1;
                }
            }
            for c in 0..mesh.n_cells() {
                assert_eq!(
                    count[c], bg.in_v[i][c] as usize,
                    "V2 cell {c} at plane {i}: {} regions",
                    count[c]
                );
            }
            assert!(!reg[2].is_empty(), "V2: no tip-gap cell");
            assert!(reg[2].iter().all(|&c| cc[c][0].hypot(cc[c][1]) > R_TIP));
        }
    }
    println!(
        "V2 B3: tip-gap cells {}; regions at 2.5 mm {:?}",
        bg.gap_cells,
        bg.regions[1][0].iter().map(|v| v.len()).collect::<Vec<_>>()
    );
    // V3: a uniform state, I = 3 everywhere, w = (0, 0, 10), phi = w . S
    let n_int = g.n_internal;
    let (sf, cf) = (mesh.face_area_vectors(), mesh.face_centres());
    let u = [0.0, 0.0, 10.0];
    let p0 = 7.0;
    let pc = |x: Vec3| 3.0 + p0 - 0.5 * dot(u, u) + 0.5 * omega() * omega() * (x[0] * x[0] + x[1] * x[1]);
    let w = vec![u; mesh.n_cells()];
    let p: Vec<f64> = cc.iter().map(|&x| pc(x)).collect();
    let bw = vec![u; mesh.n_faces() - n_int];
    let bp: Vec<f64> = (n_int..mesh.n_faces()).map(|f| pc(cf[f])).collect();
    let phi: Vec<f64> = sf.iter().map(|s| dot(u, *s)).collect();
    let nu = vec![1.5e-5; mesh.n_faces()];
    let st = State {
        w: &w,
        p: &p,
        bw: &bw,
        bp: &bp,
        phi: &phi,
        nu: &nu,
        p0,
    };
    let b = budget(g, &bg, &bg.in_v[1], &bg.regions[1][0], &st);
    println!("V3 uniform state: {b:?}");
    let scale = RHO * 3.0; // rho |I|: the parts are per volume flow
    assert!(b.q_in > 0.0);
    assert!(b.parts.iter().all(|v| v.abs() <= 1e-9 * scale), "V3 {b:?}");
    // two routes on a random state
    let mut seed = 12345u64;
    let mut rnd = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 11) as f64 / (1u64 << 53) as f64 - 0.5
    };
    let w: Vec<Vec3> = (0..mesh.n_cells()).map(|_| [rnd(), rnd(), 10.0 + rnd()]).collect();
    let p: Vec<f64> = (0..mesh.n_cells()).map(|_| 50.0 * rnd()).collect();
    let bw: Vec<Vec3> = (n_int..mesh.n_faces()).map(|_| [rnd(), rnd(), 10.0 + rnd()]).collect();
    let bp: Vec<f64> = (n_int..mesh.n_faces()).map(|_| 50.0 * rnd()).collect();
    let phi: Vec<f64> = sf.iter().map(|s| dot(u, *s) * (1.0 + 0.1 * rnd())).collect();
    let nu: Vec<f64> = (0..mesh.n_faces()).map(|_| 1e-3 * (1.0 + rnd())).collect();
    let st = State {
        w: &w,
        p: &p,
        bw: &bw,
        bp: &bp,
        phi: &phi,
        nu: &nu,
        p0,
    };
    for (i, per_delta) in bg.regions.iter().enumerate() {
        for reg in per_delta {
            let b = budget(g, &bg, &bg.in_v[i], reg, &st);
            println!(
                "B1 random state, plane {i}: route A {} route B {}",
                b.whole_a, b.whole_b
            );
            assert!(closes(&b), "B1 {b:?}");
        }
    }
}

/// S3 (ADR 0768): the B0 mesh has the recorded cell count and quality, and each gate of ADR 0762 its recorded verdict.
#[test]
fn f882_b0_mesh_gates_as_recorded() {
    mesh_gates_as_recorded("B0", b0(), &B0_QUALITY_RECORD);
}

/// S3 for the corrected B0 (ADR 0768, amendment 1).
#[test]
fn f882_b0c_mesh_gates_as_recorded() {
    mesh_gates_as_recorded("B0c", b0c(), &B0C_QUALITY_RECORD);
}

fn mesh_gates_as_recorded(name: &str, spec: Spec, record: &[f64; 3]) {
    let case = Case::parse_toml(&case_toml(spec, CAP)).unwrap_or_else(|e| panic!("{e:?}"));
    let m = rcfd_api::sim::build_mesh(&case).unwrap_or_else(|e| panic!("{e:?}"));
    let q = check_mesh(&m);
    let v = [q.max_non_orthogonality, q.max_skewness, q.max_aspect_ratio];
    println!(
        "S3 {name} mesh: {} cells, non-orthogonality {:.3} deg, skewness {:.4}, aspect {:.2}; gates {GATES:?}; met {:?}",
        m.n_cells(),
        v[0],
        v[1],
        v[2],
        [0, 1, 2].map(|i| v[i] <= GATES[i])
    );
    assert_eq!(m.n_cells(), B0_CELLS);
    for i in 0..3 {
        assert!(
            (v[i] / record[i] - 1.0).abs() <= 5e-3,
            "quality {i}: {} against {}",
            v[i],
            record[i]
        );
        assert_eq!(v[i] <= GATES[i], record[i] <= GATES[i], "gate {i}");
    }
}

/// S1 (ADR 0768): B3 for 300 iterations on the path of the heavy runs; the budget closes by the two routes, and `L_s`, the whole and
/// the four parts are within 1 % of the recorded whole of the values of the first run.
#[test]
fn f882_b3_short_run_holds_the_budget() {
    let o = run_design_point("B3 short", B3, SHORT, &[SHORT]);
    assert_eq!(o.cells, 4920);
    assert_eq!(o.iterations, SHORT, "{:?}", o.stop);
    let s = &o.samples[0];
    assert!(
        s.budgets.iter().all(closes),
        "the budget does not close: {:?}",
        s.budgets
    );
    let b = &s.budgets[0];
    let v = [s.l_s[1], b.whole_b, b.parts[0], b.parts[1], b.parts[2], b.parts[3]];
    println!("S1 B3 at {SHORT}: {v:?}");
    let tol = STATE_TOL * B3_SHORT_RECORD[1].abs();
    assert!(
        holds(v[0], B3_SHORT_RECORD[0]) && (0..5).all(|i| (v[1 + i] - B3_SHORT_RECORD[1 + i]).abs() <= tol),
        "the budget moved: {v:?} against {B3_SHORT_RECORD:?}"
    );
}

/// S2 (ADR 0768): the recorded extrapolation follows from the recorded values.
#[test]
fn f882_recorded_family_gives_the_recorded_verdicts() {
    let x = extrapolation_of(B0_STATE[2], B0_CELLS as f64, B0_S_T[0], B0_S_T[1]);
    println!("S2 {x:?}");
    assert_eq!(x.valid, EXTRAPOLATION_VALID_RECORD);
    let got = [x.p, x.l0, x.u_l, x.i0, x.u_i];
    assert!(
        (0..5).all(|i| (got[i] - EXTRAPOLATION_RECORD[i]).abs() <= 5e-3),
        "{got:?} against {EXTRAPOLATION_RECORD:?}"
    );
}

// ---------------------------------------------------------------- heavy records

static B0_OUTCOME: OnceLock<Outcome> = OnceLock::new();

fn b0_outcome() -> &'static Outcome {
    B0_OUTCOME.get_or_init(|| run_design_point("B0", b0(), CAP, &SAMPLE_AT))
}

/// The heavy record of B0 (ADR 0768): torque, pressure rise, `L_s`, the whole and the four parts at 8000 iterations within the
/// tolerance of the state of the recorded values.
#[test]
#[ignore = "heavy record, ADR 0768: the B0 design-point solve (about 107 000 cells, 8000 iterations, hours): cargo test --release -p rcfd-api --test f882_fan01_rotor_loss -- --ignored --nocapture --test-threads 1 f882_b0_loss f882_acceptance"]
fn f882_b0_loss_state_record() {
    let o = b0_outcome();
    assert_eq!(o.iterations, CAP, "{:?}", o.stop);
    let s = o.samples.last().unwrap();
    let b = &s.budgets[0];
    let v = [
        o.torque, o.dp_ts, s.l_s[1], b.whole_b, b.parts[0], b.parts[1], b.parts[2], b.parts[3],
    ];
    println!("F882 B0 state {v:?}; S, T {:?}", spread(o));
    assert!(s.budgets.iter().all(closes));
    let tol = STATE_TOL * B0_STATE[3].abs();
    assert!(
        (0..4).all(|i| holds(v[i], B0_STATE[i])) && (4..8).all(|i| (v[i] - B0_STATE[i]).abs() <= tol),
        "the B0 state moved: {v:?} against {B0_STATE:?}"
    );
}

/// The acceptance of F-882 as ADR 0768 registered its decision rule: the budget closes on B0 at every sampled iteration and the
/// extrapolation of `L_s` over B2, B1, B0 is valid.
#[test]
#[ignore = "heavy, ADR 0768: shares the B0 solve of the state record"]
fn f882_acceptance() {
    let o = b0_outcome();
    assert_eq!(o.iterations, CAP, "{:?}", o.stop);
    let (s, t) = spread(o);
    let x = extrapolation_of(o.samples.last().unwrap().l_s[1], o.cells as f64, s, t);
    println!("F882 B0 extrapolation {x:?}");
    println!(
        "F882 B0 L_0 {:.4} Pa (valid {}), at order 1 {:.4}, at order 2 {:.4}; band {:.4}; Rec_m {:.4}; I_0 {:.4} +- {:.4} Pa",
        x.l0, x.valid, x.l_order1, x.l_order2, x.u_l, x.rec_m, x.i0, x.u_i
    );
    assert!(
        o.samples.iter().all(|s| s.budgets.iter().all(closes)) && x.valid,
        "F-882 MISSED: extrapolation {x:?}"
    );
}

static B0C_OUTCOME: OnceLock<Outcome> = OnceLock::new();

fn b0c_outcome() -> &'static Outcome {
    B0C_OUTCOME.get_or_init(|| run_design_point("B0c", b0c(), CAP, &SAMPLE_AT))
}

/// The heavy record of the corrected B0 (ADR 0768, amendment 1), in the form of `f882_b0_loss_state_record`.
#[test]
#[ignore = "heavy record, ADR 0768 amendment 1: the corrected B0 solve (106 692 cells, 8000 iterations, hours): cargo test --release -p rcfd-api --test f882_fan01_rotor_loss -- --ignored --nocapture --test-threads 1 f882_b0c_"]
fn f882_b0c_loss_state_record() {
    let o = b0c_outcome();
    assert_eq!(o.iterations, CAP, "{:?}", o.stop);
    let s = o.samples.last().unwrap();
    let b = &s.budgets[0];
    let v = [
        o.torque, o.dp_ts, s.l_s[1], b.whole_b, b.parts[0], b.parts[1], b.parts[2], b.parts[3],
    ];
    println!("F882 B0c state {v:?}; S, T {:?}", spread(o));
    assert!(s.budgets.iter().all(closes));
    let tol = STATE_TOL * B0_STATE[3].abs();
    assert!(
        (0..4).all(|i| holds(v[i], B0_STATE[i])) && (4..8).all(|i| (v[i] - B0_STATE[i]).abs() <= tol),
        "the B0c state moved: {v:?} against {B0_STATE:?}"
    );
}

/// The acceptance of F-882 on the corrected B0 (ADR 0768, amendment 1), in the form of `f882_acceptance`.
#[test]
#[ignore = "KNOWN MISS, ADR 0768 addendum 1: the extrapolation of L_s over B2, B1, B0 is not valid, the observed order 2.786 is outside [1.5, 2.5] (L_0 52.965 Pa not shown); heavy, shares the corrected B0 solve of the state record"]
fn f882_b0c_acceptance_known_miss() {
    let o = b0c_outcome();
    assert_eq!(o.iterations, CAP, "{:?}", o.stop);
    let (s, t) = spread(o);
    let x = extrapolation_of(o.samples.last().unwrap().l_s[1], o.cells as f64, s, t);
    println!("F882 B0c extrapolation {x:?}");
    println!(
        "F882 B0c L_0 {:.4} Pa (valid {}), at order 1 {:.4}, at order 2 {:.4}; band {:.4}; Rec_m {:.4}; I_0 {:.4} +- {:.4} Pa",
        x.l0, x.valid, x.l_order1, x.l_order2, x.u_l, x.rec_m, x.i0, x.u_i
    );
    assert!(
        o.samples.iter().all(|s| s.budgets.iter().all(closes)) && x.valid,
        "F-882 MISSED: extrapolation {x:?}"
    );
}

/// One registered run of B1, B2 or B3 by `F882_RUN` (ADR 0768), with R1: the recorded states of ADR 0762 and ADR 0765.
#[test]
#[ignore = "heavy, ADR 0768: F882_RUN=B2 cargo test --release -p rcfd-api --test f882_fan01_rotor_loss f882_family_run -- --ignored --nocapture"]
fn f882_family_run() {
    let sel = std::env::var("F882_RUN").expect("set F882_RUN");
    let (s, i) = match sel.as_str() {
        "B1" => (B1, 0),
        "B2" => (B2, 1),
        "B3" => (B3, 2),
        other => panic!("F882_RUN = {other}"),
    };
    let o = run_design_point(&sel, s, CAP, &SAMPLE_AT);
    println!("F882 {sel} S, T {:?}", spread(&o));
    let last = o.samples.last().unwrap();
    assert!(
        o.samples.iter().all(|s| s.budgets.iter().all(closes)),
        "the budget does not close"
    );
    assert!(
        holds(o.torque, STATE_0762[i].0) && holds(o.dp_ts, STATE_0762[i].1) && holds(last.l_s[1], L_S_0765[i]),
        "{sel} is not the recorded state: torque {} dp_ts {} L_s {}",
        o.torque,
        o.dp_ts,
        last.l_s[1]
    );
}
