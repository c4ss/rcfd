//! Three-node shell (Shell3): flat facet with an assumed transverse shear strain, for triangulated surfaces. Declared by ADR
//! 0440 (F-408, the scaffold of ADR 0311); filled by ADR 0454 (F-419, the package of ADR 0322).
//!
//! A Shell3 element joins three nodes with six unknowns each, `(u_x, u_y, u_z, theta_x, theta_y, theta_z)` in global axes, on
//! the conventions of Shell4 ([`crate::shell`], ADR 0432). Its facet ([`facet`], ADR 0454 arithmetic 1): `e_3` along `(x_1 -
//! x_0) x (x_2 - x_0)`, `e_1` along the first edge, `e_2 = e_3 x e_1`, local coordinates about the centroid; a triangle with an
//! angle below [`MIN_ANGLE_DEGREES`] is refused. In local axes: the constant-strain membrane, Reissner-Mindlin bending with
//! linear `w, theta_x, theta_y` and the transverse shear of MITC3 (the tangential shear of each edge tied at its mid-point, the
//! field in the lowest-order edge space; arithmetic 4) with the shear stabilisation `t^2 / (t^2 + alpha h^2)` of Lyly, Stenberg
//! and Vihinen ([`SHEAR_STABILISATION`], `h` the longest edge), and the drilling penalty of Shell4 tying `theta_z` to the
//! membrane rotation ([`crate::shell::DRILLING_FACTOR`]), all by the three mid-edge points (exact). The consistent mass
//! carries the rotary inertia `rho t^3 / 12` on the three rotations; the pressure, body and thermal loads and the resultants
//! at the centroid follow Shell4 (arithmetic 6 to 8). A triangle is flat, so the global matrices are `T^T A T` with the plain
//! rotation.
//!
//! The solid-element routines of the [`Element`] trait do not apply: [`element`] is a placeholder that refuses naming
//! [`SOLID_ROUTINES`]. The assembly, the mass, the loads, the thermal load and [`crate::shell::stresses`] call this module for
//! Shell3 (ADR 0454 D2).

use crate::element::{Element, ElementMatrix, Pending};
use crate::error::{FemError, Result};
use crate::mesh::{ElementKind, SolidMesh};
use crate::model::Material;
use crate::shell::{
    homogeneous_section, SectionStiffness, ShearStiffness, ShellStresses, TransverseShear, DRILLING_FACTOR,
};
use rcfd_core::vec3::{cross, dot, norm, Vec3};

/// The work package that fills this module; `None` once it has landed (ADR 0454).
pub const PENDING: Option<&'static str> = None;

/// The package named when a path that does not take shells is given a Shell3 model (ADR 0454 D3).
pub const FOLLOW_UP: &str = "a follow-up of ADR 0454";

/// The package named by the solid-element routines of Shell3.
pub const SOLID_ROUTINES: &str =
    "a follow-up of ADR 0454 (the solid-element routines of Shell3; its matrices are in rcfd_fem::shell3)";

/// A triangle whose smallest interior angle is below this many degrees is refused (ADR 0454 arithmetic 1: the sliver angle
/// of the surface check of `rcfd_mesh::surface`).
pub const MIN_ANGLE_DEGREES: f64 = 1.0;

/// The factor `alpha` of the shear stabilisation `t^2 / (t^2 + alpha h^2)`, `h` the longest edge (ADR 0454 arithmetic 4).
pub const SHEAR_STABILISATION: f64 = 0.1;

static ELEMENT: Pending = Pending::new(ElementKind::Shell3, SOLID_ROUTINES);

/// The element of the kind for the solid-element routines: a placeholder that refuses.
pub fn element() -> &'static dyn Element {
    &ELEMENT
}

/// Reference coordinates of the nodes (none: a shell has no isoparametric volume map).
pub fn reference_nodes() -> &'static [[f64; 3]] {
    &[]
}

/// The local faces (none).
pub fn faces() -> &'static [&'static [usize]] {
    &[]
}

/// Derivatives of the shape functions with respect to the reference coordinates (none).
pub fn shape_gradients(_xi: [f64; 3]) -> Vec<[f64; 3]> {
    Vec::new()
}

/// Options of the element matrices. The default is the element; the other values exist for the controls of ADR 0454 and
/// are not reachable from a model.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct Shell3Options {
    /// The transverse shear: the MITC3 edge tying (the element) or the displacement shear at the mid-edge points.
    pub shear: TransverseShear,
    /// The drilling factor `alpha_d`.
    pub drilling: f64,
    /// The shear stabilisation factor `alpha`.
    pub stabilisation: f64,
}

impl Default for Shell3Options {
    fn default() -> Shell3Options {
        Shell3Options {
            shear: TransverseShear::AssumedStrain,
            drilling: DRILLING_FACTOR,
            stabilisation: SHEAR_STABILISATION,
        }
    }
}

impl Shell3Options {
    /// The same options with a transverse shear treatment.
    pub fn shear(mut self, shear: TransverseShear) -> Shell3Options {
        self.shear = shear;
        self
    }
    /// The same options with a drilling factor.
    pub fn drilling(mut self, drilling: f64) -> Shell3Options {
        self.drilling = drilling;
        self
    }
    /// The same options with a shear stabilisation factor.
    pub fn stabilisation(mut self, stabilisation: f64) -> Shell3Options {
        self.stabilisation = stabilisation;
        self
    }
}

/// The facet of a Shell3 element (ADR 0454 arithmetic 1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Facet3 {
    /// `[e_1, e_2, e_3]`, the rows of the rotation to local axes.
    pub axes: [Vec3; 3],
    /// The centroid.
    pub centre: Vec3,
    /// Local coordinates `(X, Y)` of the nodes about the centroid.
    pub local: [[f64; 2]; 3],
    /// The area.
    pub area: f64,
    /// The smallest interior angle, degrees.
    pub min_angle: f64,
    /// The longest edge.
    pub longest_edge: f64,
}

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// The facet of three node coordinates (ADR 0454 arithmetic 1). Refuses other than three coordinates, a degenerate triangle
/// (`InvalidValue { key: "geometry" }`) and a smallest angle below [`MIN_ANGLE_DEGREES`] (`InvalidValue { key: "angle" }`; the
/// model checks name the element).
pub fn facet(coords: &[Vec3]) -> Result<Facet3> {
    if coords.len() != 3 {
        return Err(FemError::FieldLength {
            key: "coordinates",
            expected: 3,
            found: coords.len(),
        });
    }
    let d1 = sub(coords[1], coords[0]);
    let d2 = sub(coords[2], coords[0]);
    let n = cross(d1, d2);
    let nn = norm(n);
    let l1 = norm(d1);
    if !(nn > 0.0 && nn.is_finite() && l1 > 0.0) {
        return Err(FemError::InvalidValue {
            key: "geometry",
            reason: format!("the triangle {coords:?} is degenerate (|d_1 x d_2| = {nn:e})"),
        });
    }
    let e3 = [n[0] / nn, n[1] / nn, n[2] / nn];
    let e1 = [d1[0] / l1, d1[1] / l1, d1[2] / l1];
    let e2 = cross(e3, e1);
    let mut centre = [0.0; 3];
    for x in coords {
        for k in 0..3 {
            centre[k] += x[k] / 3.0;
        }
    }
    let mut local = [[0.0; 2]; 3];
    for i in 0..3 {
        let d = sub(coords[i], centre);
        local[i] = [dot(d, e1), dot(d, e2)];
    }
    let mut min_angle = f64::INFINITY;
    let mut longest_edge = 0.0f64;
    for i in 0..3 {
        let p = local[i];
        let a = [local[(i + 1) % 3][0] - p[0], local[(i + 1) % 3][1] - p[1]];
        let b = [local[(i + 2) % 3][0] - p[0], local[(i + 2) % 3][1] - p[1]];
        let angle = (a[0] * b[1] - a[1] * b[0]).abs().atan2(a[0] * b[0] + a[1] * b[1]).to_degrees();
        min_angle = min_angle.min(angle);
        longest_edge = longest_edge.max((a[0] * a[0] + a[1] * a[1]).sqrt());
    }
    if min_angle.is_nan() || min_angle < MIN_ANGLE_DEGREES {
        return Err(FemError::InvalidValue {
            key: "angle",
            reason: format!(
                "the smallest angle of the triangle is {min_angle:.4} deg, below the limit {MIN_ANGLE_DEGREES} deg (ADR 0454)"
            ),
        });
    }
    let area = 0.5
        * ((local[1][0] - local[0][0]) * (local[2][1] - local[0][1])
            - (local[2][0] - local[0][0]) * (local[1][1] - local[0][1]));
    Ok(Facet3 {
        axes: [e1, e2, e3],
        centre,
        local,
        area,
        min_angle,
        longest_edge,
    })
}

/// The Cartesian derivatives `(N_i,X, N_i,Y)` of the linear shape functions (ADR 0454 arithmetic 2).
fn derivatives(f: &Facet3) -> ([f64; 3], [f64; 3]) {
    let l = &f.local;
    let a2 = 2.0 * f.area;
    let mut nx = [0.0; 3];
    let mut ny = [0.0; 3];
    for i in 0..3 {
        let (j, k) = ((i + 1) % 3, (i + 2) % 3);
        nx[i] = (l[j][1] - l[k][1]) / a2;
        ny[i] = (l[k][0] - l[j][0]) / a2;
    }
    (nx, ny)
}

/// The shape values at the mid-point of edge `e` (from node `e` to node `e + 1`).
fn mid_edge_shapes(e: usize) -> [f64; 3] {
    let mut n = [0.0; 3];
    n[e] = 0.5;
    n[(e + 1) % 3] = 0.5;
    n
}

/// The local coordinates of the mid-point of edge `e`.
fn mid_edge_point(f: &Facet3, e: usize) -> [f64; 2] {
    let (p, q) = (f.local[e], f.local[(e + 1) % 3]);
    [0.5 * (p[0] + q[0]), 0.5 * (p[1] + q[1])]
}

fn membrane_rows(nx: &[f64; 3], ny: &[f64; 3]) -> [[f64; 18]; 3] {
    let mut b = [[0.0; 18]; 3];
    for i in 0..3 {
        b[0][6 * i] = nx[i];
        b[1][6 * i + 1] = ny[i];
        b[2][6 * i] = ny[i];
        b[2][6 * i + 1] = nx[i];
    }
    b
}

/// Curvature rows with `beta_x = theta_y`, `beta_y = -theta_x` (ADR 0432 arithmetic 3).
fn bending_rows(nx: &[f64; 3], ny: &[f64; 3]) -> [[f64; 18]; 3] {
    let mut b = [[0.0; 18]; 3];
    for i in 0..3 {
        b[0][6 * i + 4] = nx[i];
        b[1][6 * i + 3] = -ny[i];
        b[2][6 * i + 4] = ny[i];
        b[2][6 * i + 3] = -nx[i];
    }
    b
}

/// The coefficients `(a_1, a_2, b)` of the assumed shear `gamma = (a_1 - b Y, a_2 + b X)` as rows over the 18 local unknowns:
/// `gamma(m_e) . t_e = g_e`, `g_e = (w_j - w_i) / l_e + (beta_i + beta_j) / 2 . t_e` on each edge (ADR 0454 arithmetic 4).
fn tying_coefficients(f: &Facet3) -> [[f64; 18]; 3] {
    let mut g = [[0.0; 3]; 3];
    let mut rhs = [[0.0; 18]; 3];
    for e in 0..3 {
        let (i, j) = (e, (e + 1) % 3);
        let d = [f.local[j][0] - f.local[i][0], f.local[j][1] - f.local[i][1]];
        let l = (d[0] * d[0] + d[1] * d[1]).sqrt();
        let t = [d[0] / l, d[1] / l];
        let m = mid_edge_point(f, e);
        g[e] = [t[0], t[1], m[0] * t[1] - m[1] * t[0]];
        rhs[e][6 * j + 2] += 1.0 / l;
        rhs[e][6 * i + 2] -= 1.0 / l;
        for q in [i, j] {
            rhs[e][6 * q + 4] += 0.5 * t[0];
            rhs[e][6 * q + 3] -= 0.5 * t[1];
        }
    }
    let inv = invert3(&g);
    let mut c = [[0.0; 18]; 3];
    for r in 0..3 {
        for q in 0..18 {
            c[r][q] = inv[r][0] * rhs[0][q] + inv[r][1] * rhs[1][q] + inv[r][2] * rhs[2][q];
        }
    }
    c
}

fn invert3(a: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let det = a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1]) - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
        + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0]);
    let mut inv = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            let (r0, r1) = ((j + 1) % 3, (j + 2) % 3);
            let (c0, c1) = ((i + 1) % 3, (i + 2) % 3);
            inv[i][j] = (a[r0][c0] * a[r1][c1] - a[r0][c1] * a[r1][c0]) / det;
        }
    }
    inv
}

/// The transverse shear rows `(gamma_x, gamma_y)` at local point `p` (the assumed strain) or at the mid-point of edge `e`
/// (the displacement shear of the control).
fn shear_rows(
    p: [f64; 2],
    e: usize,
    shear: TransverseShear,
    coef: &[[f64; 18]; 3],
    nx: &[f64; 3],
    ny: &[f64; 3],
) -> [[f64; 18]; 2] {
    let mut out = [[0.0; 18]; 2];
    match shear {
        TransverseShear::AssumedStrain => {
            for q in 0..18 {
                out[0][q] = coef[0][q] - coef[2][q] * p[1];
                out[1][q] = coef[1][q] + coef[2][q] * p[0];
            }
        }
        TransverseShear::FullIntegration => {
            let n = mid_edge_shapes(e);
            for i in 0..3 {
                out[0][6 * i + 2] = nx[i];
                out[0][6 * i + 4] = n[i];
                out[1][6 * i + 2] = ny[i];
                out[1][6 * i + 3] = -n[i];
            }
        }
    }
    out
}

/// The stabilised shear stiffness `H t^2 / (t^2 + alpha h^2)` of the element (ADR 0454 arithmetic 4).
fn stabilised_shear(f: &Facet3, section: &SectionStiffness, alpha: f64) -> ShearStiffness {
    let t = section.drilling[1];
    let h = f.longest_edge;
    let s = t * t / (t * t + alpha * h * h);
    match section.shear {
        ShearStiffness::Isotropic(d) => ShearStiffness::Isotropic(d * s),
        ShearStiffness::Matrix(m) => ShearStiffness::Matrix([[m[0][0] * s, m[0][1] * s], [m[1][0] * s, m[1][1] * s]]),
    }
}

fn mul3(d: &[[f64; 3]; 3], v: [f64; 3]) -> [f64; 3] {
    [
        d[0][0] * v[0] + d[0][1] * v[1] + d[0][2] * v[2],
        d[1][0] * v[0] + d[1][1] * v[1] + d[1][2] * v[2],
        d[2][0] * v[0] + d[2][1] * v[1] + d[2][2] * v[2],
    ]
}

/// `K += w B1^T C B2` over 18 unknowns for 3-row `B1`, `B2`.
fn add_b1t_c_b2(k: &mut [f64], b1: &[[f64; 18]; 3], c: &[[f64; 3]; 3], b2: &[[f64; 18]; 3], w: f64) {
    let mut cb = [[0.0; 18]; 3];
    for r in 0..3 {
        for q in 0..18 {
            cb[r][q] = c[r][0] * b2[0][q] + c[r][1] * b2[1][q] + c[r][2] * b2[2][q];
        }
    }
    for i in 0..18 {
        for j in 0..18 {
            k[i * 18 + j] += w * (b1[0][i] * cb[0][j] + b1[1][i] * cb[1][j] + b1[2][i] * cb[2][j]);
        }
    }
}

fn transpose3(c: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    [[c[0][0], c[1][0], c[2][0]], [c[0][1], c[1][1], c[2][1]], [c[0][2], c[1][2], c[2][2]]]
}

/// The 18 x 18 local stiffness for a section (ADR 0454 arithmetic 2 to 5), unknowns `(u, v, w, theta_x, theta_y, theta_z)`
/// node by node.
pub fn local_stiffness_section(f: &Facet3, section: &SectionStiffness, options: &Shell3Options) -> ElementMatrix {
    let (nx, ny) = derivatives(f);
    let bm = membrane_rows(&nx, &ny);
    let bb = bending_rows(&nx, &ny);
    let a = f.area;
    let mut k = ElementMatrix::zeros(18);
    let v = k.values_mut();
    add_b1t_c_b2(v, &bm, &section.membrane, &bm, a);
    add_b1t_c_b2(v, &bb, &section.bending, &bb, a);
    if let Some(c) = &section.coupling {
        add_b1t_c_b2(v, &bm, c, &bb, a);
        add_b1t_c_b2(v, &bb, &transpose3(c), &bm, a);
    }
    let coef = tying_coefficients(f);
    let h = stabilised_shear(f, section, options.stabilisation);
    let dr = options.drilling * section.drilling[0] * section.drilling[1];
    let w = a / 3.0;
    for e in 0..3 {
        let p = mid_edge_point(f, e);
        let s = shear_rows(p, e, options.shear, &coef, &nx, &ny);
        let n = mid_edge_shapes(e);
        let mut d = [0.0; 18];
        for i in 0..3 {
            d[6 * i + 5] = n[i];
            d[6 * i + 1] -= 0.5 * nx[i];
            d[6 * i] += 0.5 * ny[i];
        }
        for i in 0..18 {
            for j in 0..18 {
                let q = match h {
                    ShearStiffness::Isotropic(hs) => hs * (s[0][i] * s[0][j] + s[1][i] * s[1][j]),
                    ShearStiffness::Matrix(hm) => {
                        hm[0][0] * s[0][i] * s[0][j]
                            + hm[0][1] * s[0][i] * s[1][j]
                            + hm[1][0] * s[1][i] * s[0][j]
                            + hm[1][1] * s[1][i] * s[1][j]
                    }
                };
                v[i * 18 + j] += w * (q + dr * d[i] * d[j]);
            }
        }
    }
    symmetrise(&mut k);
    k
}

/// The 18 x 18 consistent mass in local axes (ADR 0454 arithmetic 6).
pub fn local_mass(f: &Facet3, thickness: f64, density: f64) -> ElementMatrix {
    let t = thickness;
    let (mt, mr) = (density * t, density * t * t * t / 12.0);
    let mut m = ElementMatrix::zeros(18);
    let v = m.values_mut();
    for i in 0..3 {
        for j in 0..3 {
            let s = f.area * if i == j { 2.0 } else { 1.0 } / 12.0;
            for c in 0..3 {
                v[(6 * i + c) * 18 + 6 * j + c] = mt * s;
                v[(6 * i + 3 + c) * 18 + 6 * j + 3 + c] = mr * s;
            }
        }
    }
    m
}

fn symmetrise(a: &mut ElementMatrix) {
    let n = a.size();
    let v = a.values_mut();
    for i in 0..n {
        for j in 0..i {
            let s = 0.5 * (v[i * n + j] + v[j * n + i]);
            v[i * n + j] = s;
            v[j * n + i] = s;
        }
    }
}

/// The rotation `T` (18 x 18, row-major) from the global to the local unknowns: per node `diag(Lambda, Lambda)`.
pub fn transformation(f: &Facet3) -> Vec<f64> {
    let mut t = vec![0.0; 18 * 18];
    for i in 0..3 {
        for a in 0..3 {
            for b in 0..3 {
                t[(6 * i + a) * 18 + 6 * i + b] = f.axes[a][b];
                t[(6 * i + 3 + a) * 18 + 6 * i + 3 + b] = f.axes[a][b];
            }
        }
    }
    t
}

/// `T^T A T`, exactly symmetric.
fn to_global(a: &ElementMatrix, t: &[f64]) -> ElementMatrix {
    let av = a.values();
    let mut at = vec![0.0; 18 * 18];
    for i in 0..18 {
        for j in 0..18 {
            let mut s = 0.0;
            for k in 0..18 {
                s += av[i * 18 + k] * t[k * 18 + j];
            }
            at[i * 18 + j] = s;
        }
    }
    let mut out = ElementMatrix::zeros(18);
    let ov = out.values_mut();
    for i in 0..18 {
        for j in 0..18 {
            let mut s = 0.0;
            for k in 0..18 {
                s += t[k * 18 + i] * at[k * 18 + j];
            }
            ov[i * 18 + j] = s;
        }
    }
    symmetrise(&mut out);
    out
}

fn to_global_vector(f: &Facet3, fl: &[f64; 18]) -> [f64; 18] {
    let t = transformation(f);
    let mut out = [0.0; 18];
    for (j, o) in out.iter_mut().enumerate() {
        *o = (0..18).map(|i| t[i * 18 + j] * fl[i]).sum();
    }
    out
}

/// The global stiffness of a Shell3 element from its node coordinates, thickness and material (the element).
pub fn element_stiffness(coords: &[Vec3], thickness: f64, material: &Material) -> Result<ElementMatrix> {
    element_stiffness_with(coords, thickness, material, &Shell3Options::default())
}

/// The global stiffness with given options (the controls of ADR 0454).
pub fn element_stiffness_with(
    coords: &[Vec3],
    thickness: f64,
    material: &Material,
    options: &Shell3Options,
) -> Result<ElementMatrix> {
    element_stiffness_section(coords, &homogeneous_section(material, thickness), options)
}

/// The global stiffness for a given section (ADR 0440 D6).
pub fn element_stiffness_section(
    coords: &[Vec3],
    section: &SectionStiffness,
    options: &Shell3Options,
) -> Result<ElementMatrix> {
    let f = facet(coords)?;
    Ok(to_global(
        &local_stiffness_section(&f, section, options),
        &transformation(&f),
    ))
}

/// The global consistent mass of a Shell3 element.
pub fn element_mass(coords: &[Vec3], thickness: f64, material: &Material) -> Result<ElementMatrix> {
    let f = facet(coords)?;
    Ok(to_global(
        &local_mass(&f, thickness, material.density()),
        &transformation(&f),
    ))
}

/// The consistent nodal load (global axes, the element's unknown order) of a uniform pressure `p` along `e_3`: `p A / 3 e_3`
/// at each node (ADR 0454 arithmetic 7).
pub fn pressure_load_vector(coords: &[Vec3], p: f64) -> Result<[f64; 18]> {
    let f = facet(coords)?;
    let mut fl = [0.0; 18];
    for i in 0..3 {
        fl[6 * i + 2] = p * f.area / 3.0;
    }
    Ok(to_global_vector(&f, &fl))
}

/// The consistent nodal load of a body force per unit volume over the thickness, `F_i = t int N_i b dA` by the mid-edge rule
/// (exact for a `b` linear in `x`), the body force given by the shape values at the point and the point (ADR 0454
/// arithmetic 7). The moments are zero.
pub fn body_load_vector_at<B>(coords: &[Vec3], thickness: f64, b: B) -> Result<[f64; 18]>
where
    B: Fn(&[f64; 3], Vec3) -> Result<Vec3>,
{
    let f = facet(coords)?;
    let mut out = [0.0; 18];
    let w = thickness * f.area / 3.0;
    for e in 0..3 {
        let n = mid_edge_shapes(e);
        let mut x = [0.0; 3];
        for i in 0..3 {
            for k in 0..3 {
                x[k] += n[i] * coords[i][k];
            }
        }
        let bx = b(&n, x)?;
        for i in 0..3 {
            for k in 0..3 {
                out[6 * i + k] += w * n[i] * bx[k];
            }
        }
    }
    Ok(out)
}

/// The consistent nodal load of the membrane thermal strain `alpha dT (1, 1, 0)`, uniform through the thickness, of the
/// temperature changes of the three nodes: `A B_m^T A_m alpha dT_mean (1, 1, 0)` (ADR 0454 arithmetic 7). Refuses other than
/// three temperatures and a material without a finite expansion coefficient.
pub fn thermal_load_vector(
    coords: &[Vec3],
    thickness: f64,
    material: &Material,
    delta_t_nodal: &[f64],
) -> Result<[f64; 18]> {
    if delta_t_nodal.len() != 3 {
        return Err(FemError::FieldLength {
            key: "temperature",
            expected: 3,
            found: delta_t_nodal.len(),
        });
    }
    let alpha = material.expansion();
    if !alpha.is_finite() {
        return Err(FemError::InvalidValue {
            key: "expansion",
            reason: format!(
                "a {} material has no single expansion coefficient for a shell thermal load",
                material.name()
            ),
        });
    }
    let f = facet(coords)?;
    let a = homogeneous_section(material, thickness).membrane;
    let dt = (delta_t_nodal[0] + delta_t_nodal[1] + delta_t_nodal[2]) / 3.0;
    let e = alpha * dt;
    let s = mul3(&a, [e, e, 0.0]);
    let (nx, ny) = derivatives(&f);
    let bm = membrane_rows(&nx, &ny);
    let mut fl = [0.0; 18];
    for (c, fc) in fl.iter_mut().enumerate() {
        *fc = f.area * (bm[0][c] * s[0] + bm[1][c] * s[1] + bm[2][c] * s[2]);
    }
    Ok(to_global_vector(&f, &fl))
}

/// The facet of an element of a model, its refusals naming the element.
pub(crate) fn model_facet(mesh: &SolidMesh, e: usize) -> Result<Facet3> {
    let coords: Vec<Vec3> = mesh.element_nodes(e).iter().map(|&n| mesh.nodes()[n]).collect();
    facet(&coords).map_err(|source| FemError::Element {
        element: e,
        source: Box::new(source),
    })
}

/// The stiffness of a Shell3 element of a model (its block's section, its material).
pub(crate) fn model_element_stiffness(
    mesh: &SolidMesh,
    element: usize,
    coords: &[Vec3],
    material: &Material,
) -> Result<ElementMatrix> {
    let section = crate::shell::model_section(mesh, element, material)?;
    element_stiffness_section(coords, &section, &Shell3Options::default())
}

/// The consistent mass of a Shell3 element of a model.
pub(crate) fn model_element_mass(
    mesh: &SolidMesh,
    element: usize,
    coords: &[Vec3],
    material: &Material,
) -> Result<ElementMatrix> {
    let t = crate::shell::thickness_or_error(mesh, element)?;
    element_mass(coords, t, material)
}

/// The resultants and surface stresses of a Shell3 element at its centroid from its global unknowns (ADR 0454 arithmetic 8):
/// `thermal` is the membrane thermal strain `alpha dT_mean` subtracted from `eps_xx` and `eps_yy`.
pub(crate) fn element_stresses(
    f: &Facet3,
    ue: &[f64],
    section: &SectionStiffness,
    thickness: f64,
    thermal: f64,
) -> ShellStresses {
    let tr = transformation(f);
    let ul: Vec<f64> = (0..18).map(|i| (0..18).map(|j| tr[i * 18 + j] * ue[j]).sum()).collect();
    let (nx, ny) = derivatives(f);
    let strain = |b: &[[f64; 18]; 3]| -> [f64; 3] {
        let mut s = [0.0; 3];
        for (r, row) in b.iter().enumerate() {
            s[r] = row.iter().zip(&ul).map(|(a, x)| a * x).sum();
        }
        s
    };
    let mut eps = strain(&membrane_rows(&nx, &ny));
    eps[0] -= thermal;
    eps[1] -= thermal;
    let kappa = strain(&bending_rows(&nx, &ny));
    let mut membrane = mul3(&section.membrane, eps);
    let mut bending = mul3(&section.bending, kappa);
    if let Some(c) = &section.coupling {
        let ck = mul3(c, kappa);
        let cte = mul3(&transpose3(c), eps);
        for k in 0..3 {
            membrane[k] += ck[k];
            bending[k] += cte[k];
        }
    }
    let coef = tying_coefficients(f);
    let g0: f64 = coef[0].iter().zip(&ul).map(|(a, x)| a * x).sum();
    let g1: f64 = coef[1].iter().zip(&ul).map(|(a, x)| a * x).sum();
    let shear = match stabilised_shear(f, section, SHEAR_STABILISATION) {
        ShearStiffness::Isotropic(h) => [h * g0, h * g1],
        ShearStiffness::Matrix(h) => [h[0][0] * g0 + h[0][1] * g1, h[1][0] * g0 + h[1][1] * g1],
    };
    let t = thickness;
    let mut top = [0.0; 3];
    let mut bottom = [0.0; 3];
    for k in 0..3 {
        top[k] = membrane[k] / t + 6.0 * bending[k] / (t * t);
        bottom[k] = membrane[k] / t - 6.0 * bending[k] / (t * t);
    }
    ShellStresses {
        axes: f.axes,
        membrane,
        bending,
        shear,
        top,
        bottom,
    }
}
