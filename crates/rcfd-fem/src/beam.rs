//! Two-node shear-flexible beam (Beam2): sections, stiffness, mass and section forces. Declared by ADR 0291; filled by
//! ADR 0417 (F-391, the package of ADR 0294).
//!
//! A Beam2 element joins two nodes with six unknowns each, `(u_x, u_y, u_z, theta_x, theta_y, theta_z)` in global axes
//! (small rotations). Its local frame ([`frame`]): `e_1` along the axis from node 0 to node 1, `e_3 = e_1 x v / |e_1 x v|`
//! with the block's orientation vector `v`, `e_2 = e_3 x e_1`; the section axes y and z are `e_2` and `e_3`. In local axes
//! the element is the exact prismatic Timoshenko beam (ADR 0417 arithmetic 2 and 3): axial `E A / L`, torsion `G J / L`, and in
//! each bending plane the stiffness of the cantilever flexibility with shear, `Phi = 12 E I / (kappa G A L^2)`, which is also
//! the energy of the "interdependent" shape functions (cubic deflection, quadratic section rotation, constant shear strain,
//! [`plane_shape_coefficients`]). The consistent mass is the kinetic energy of the same interpolation with rotary inertia
//! `rho I` in bending and `rho (I_y + I_z)` in torsion. The global matrices are `T^T K_l T`, `T` four copies of the rotation
//! with rows `e_1, e_2, e_3`.
//!
//! The solid-element routines of the [`Element`] trait (an isoparametric volume map, a 6 x 6 elasticity matrix) do not apply:
//! [`element`] is a placeholder that refuses naming [`SOLID_ROUTINES`], so the solid body loads, the thermal load and stress
//! recovery refuse a beam. The assembly calls [`element_stiffness`] and [`element_mass`] for Beam2 (ADR 0417 D6). Uniform
//! line loads and gravity on beams have the consistent load [`line_load_vector`] (ADR 0422), the centrifugal load that of
//! [`centrifugal_load_vector`] (ADR 0473); a nodal body force and the thermal load still refuse.
//!
//! ADR 0473 (F-420): the geometric stiffness of the axial force ([`local_geometric_stiffness`], with the deflection functions
//! of the interdependent element and Wagner's torsion term), the element-average axial force ([`axial_force`]) and the
//! spin-softening mass of the section's kinematics ([`local_spin_softening_mass`]).

use crate::assemble::element_dofs;
use crate::element::{Element, ElementMatrix, Pending};
use crate::error::{FemError, Result};
use crate::mesh::{BeamSection, BlockProperty, ElementKind, SolidMesh};
use crate::model::{Load, Material, SolidModel};
use rcfd_core::vec3::{cross, norm, Vec3};

/// The work package that adds Beam2; `None` once it has landed (ADR 0417).
pub const PENDING: Option<&'static str> = None;

/// The package named when a path that does not take beams is given a beam model (ADR 0417 D4).
pub const FOLLOW_UP: &str = "a follow-up of ADR 0417";

/// The package named by the solid-element routines of Beam2 (ADR 0417 D7).
pub const SOLID_ROUTINES: &str =
    "a follow-up of ADR 0417 (the solid-element routines of Beam2; its matrices are in rcfd_fem::beam)";

/// An orientation vector within this angle (rad) of the beam axis is refused (ADR 0417 arithmetic 1).
pub const PARALLEL_TOLERANCE: f64 = 1e-6;

static ELEMENT: Pending = Pending::new(ElementKind::Beam2, SOLID_ROUTINES);

/// The element of the kind for the solid-element routines: a placeholder that refuses (ADR 0417 D7).
pub fn element() -> &'static dyn Element {
    &ELEMENT
}

/// Reference coordinates of the nodes (none: a beam has no isoparametric volume map).
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

/// A rectangular section of width `b` along the section axis y and height `h` along z: `A = b h`, `I_y = b h^3 / 12`,
/// `I_z = h b^3 / 12`; the torsion constant (a series for a rectangle) and the shear factor (both directions) are inputs.
pub fn rectangle_section(b: f64, h: f64, torsion: f64, shear: f64) -> Result<BeamSection> {
    BeamSection::new(b * h, b * h * h * h / 12.0, h * b * b * b / 12.0, torsion, shear, shear)
}

/// A solid circular section of radius `r`: `A = pi r^2`, `I_y = I_z = pi r^4 / 4`, `J = pi r^4 / 2`; the shear factor is an
/// input.
pub fn circle_section(r: f64, shear: f64) -> Result<BeamSection> {
    let pi = std::f64::consts::PI;
    let r2 = r * r;
    BeamSection::new(
        pi * r2,
        pi * r2 * r2 / 4.0,
        pi * r2 * r2 / 4.0,
        pi * r2 * r2 / 2.0,
        shear,
        shear,
    )
}

/// The local frame of a beam element: its length and the axes `e_1, e_2, e_3` (rows of the rotation to local axes).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BeamFrame {
    /// Length, m.
    pub length: f64,
    /// `[e_1, e_2, e_3]`.
    pub axes: [Vec3; 3],
}

/// The frame of the element from node `x0` to node `x1` with the orientation vector `v` (ADR 0417 arithmetic 1). Refuses a
/// zero length and an orientation within [`PARALLEL_TOLERANCE`] of the axis.
pub fn frame(x0: Vec3, x1: Vec3, v: Vec3) -> Result<BeamFrame> {
    let d = [x1[0] - x0[0], x1[1] - x0[1], x1[2] - x0[2]];
    let length = norm(d);
    if !(length > 0.0 && length.is_finite()) {
        return Err(FemError::InvalidValue {
            key: "length",
            reason: format!("the beam from {x0:?} to {x1:?} has length {length:e}"),
        });
    }
    let e1 = [d[0] / length, d[1] / length, d[2] / length];
    let c = cross(e1, v);
    let s = norm(c);
    let nv = norm(v);
    if s.is_nan() || s <= PARALLEL_TOLERANCE * nv {
        return Err(FemError::InvalidValue {
            key: "orientation",
            reason: format!(
                "the orientation {v:?} is parallel to the beam axis {e1:?} (|e_1 x v| / |v| = {:e})",
                s / nv
            ),
        });
    }
    let e3 = [c[0] / s, c[1] / s, c[2] / s];
    let e2 = cross(e3, e1);
    Ok(BeamFrame {
        length,
        axes: [e1, e2, e3],
    })
}

/// The coefficients `C` (4 x 4, row-major by coefficient) of the interdependent shape functions of one bending plane with
/// the shear parameter `phi`: the deflection is `d(xi) = sum_j (sum_i C[j][i] q_i) xi^j` and the section rotation
/// `psi(xi) = (b_1 + 2 b_2 xi + b_3 (3 xi^2 + phi / 2)) / L` for the end values `q = (d_0, L psi_0, d_1, L psi_1)`, `xi = x /
/// L` (ADR 0417 arithmetic 2: `C = A^{-1}`).
pub fn plane_shape_coefficients(phi: f64) -> [[f64; 4]; 4] {
    let mut a = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, phi / 2.0],
        [1.0, 1.0, 1.0, 1.0],
        [0.0, 1.0, 2.0, 3.0 + phi / 2.0],
    ];
    let mut inv = [[0.0; 4]; 4];
    for (i, row) in inv.iter_mut().enumerate() {
        row[i] = 1.0;
    }
    // Gauss-Jordan elimination with partial pivoting.
    for col in 0..4 {
        let p = (col..4)
            .max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs()))
            .expect("non-empty");
        a.swap(col, p);
        inv.swap(col, p);
        let d = a[col][col];
        for k in 0..4 {
            a[col][k] /= d;
            inv[col][k] /= d;
        }
        for r in 0..4 {
            if r != col {
                let f = a[r][col];
                if f != 0.0 {
                    for k in 0..4 {
                        a[r][k] -= f * a[col][k];
                        inv[r][k] -= f * inv[col][k];
                    }
                }
            }
        }
    }
    inv
}

/// The bending stiffness of one plane in `(d_0, psi_0, d_1, psi_1)` (ADR 0417 arithmetic 2).
fn plane_stiffness(ei: f64, ks: f64, l: f64) -> [[f64; 4]; 4] {
    let phi = 12.0 * ei / (ks * l * l);
    let c = ei / ((1.0 + phi) * l * l * l);
    let (a, b) = ((4.0 + phi) * l * l, (2.0 - phi) * l * l);
    let s = 6.0 * l;
    [
        [12.0 * c, s * c, -12.0 * c, s * c],
        [s * c, a * c, -s * c, b * c],
        [-12.0 * c, -s * c, 12.0 * c, -s * c],
        [s * c, b * c, -s * c, a * c],
    ]
}

// Four-point Gauss-Legendre rule on [-1, 1].
const GAUSS4_X: [f64; 4] = [
    -0.861_136_311_594_052_6,
    -0.339_981_043_584_856_3,
    0.339_981_043_584_856_3,
    0.861_136_311_594_052_6,
];
const GAUSS4_W: [f64; 4] = [
    0.347_854_845_137_453_9,
    0.652_145_154_862_546_1,
    0.652_145_154_862_546_1,
    0.347_854_845_137_453_9,
];

/// The values at `xi` of the deflection and section-rotation functions of the plane, for the end unknowns `(d_0, psi_0, d_1,
/// psi_1)`.
fn plane_functions(c: &[[f64; 4]; 4], phi: f64, l: f64, xi: f64) -> ([f64; 4], [f64; 4]) {
    let pd = [1.0, xi, xi * xi, xi * xi * xi];
    let pp = [0.0, 1.0 / l, 2.0 * xi / l, (3.0 * xi * xi + phi / 2.0) / l];
    let scale = [1.0, l, 1.0, l];
    let mut nd = [0.0; 4];
    let mut np = [0.0; 4];
    for q in 0..4 {
        for j in 0..4 {
            nd[q] += pd[j] * c[j][q] * scale[q];
            np[q] += pp[j] * c[j][q] * scale[q];
        }
    }
    (nd, np)
}

/// The consistent mass of one bending plane in `(d_0, psi_0, d_1, psi_1)`: `int (m_a N_d^T N_d + m_i N_psi^T N_psi) dx` by
/// four-point Gauss (exact, degree 6).
fn plane_mass(m_a: f64, m_i: f64, ei: f64, ks: f64, l: f64) -> [[f64; 4]; 4] {
    let phi = 12.0 * ei / (ks * l * l);
    let c = plane_shape_coefficients(phi);
    let mut m = [[0.0; 4]; 4];
    for (x, w) in GAUSS4_X.iter().zip(GAUSS4_W) {
        let xi = 0.5 * (x + 1.0);
        let wl = 0.5 * w * l;
        let (nd, np) = plane_functions(&c, phi, l, xi);
        for i in 0..4 {
            for j in i..4 {
                m[i][j] += wl * (m_a * nd[i] * nd[j] + m_i * np[i] * np[j]);
            }
        }
    }
    for i in 0..4 {
        for j in 0..i {
            m[i][j] = m[j][i];
        }
    }
    m
}

/// `G = E / (2 (1 + nu))` and `E` of the material.
fn moduli(material: &Material) -> (f64, f64) {
    let e = material.youngs_modulus();
    (e, e / (2.0 * (1.0 + material.poisson_ratio())))
}

/// Places a plane matrix into the 12 x 12 local matrix at the unknowns `(d, psi)` of each node, with the sign `s` of `psi`.
fn place_plane(k: &mut ElementMatrix, p: &[[f64; 4]; 4], d: usize, r: usize, s: f64) {
    let idx = [d, r, 6 + d, 6 + r];
    let sg = [1.0, s, 1.0, s];
    let v = k.values_mut();
    for i in 0..4 {
        for j in 0..4 {
            v[idx[i] * 12 + idx[j]] += sg[i] * sg[j] * p[i][j];
        }
    }
}

fn place_pair(k: &mut ElementMatrix, a: usize, m: [[f64; 2]; 2]) {
    let idx = [a, 6 + a];
    let v = k.values_mut();
    for i in 0..2 {
        for j in 0..2 {
            v[idx[i] * 12 + idx[j]] += m[i][j];
        }
    }
}

/// The 12 x 12 stiffness in local axes, unknowns `(u, v, w, theta_x, theta_y, theta_z)` of node 0 then node 1 (ADR 0417
/// arithmetic 2, 3).
pub fn local_stiffness(length: f64, section: &BeamSection, material: &Material) -> ElementMatrix {
    let (e, g) = moduli(material);
    let l = length;
    let a = section.area();
    let mut k = ElementMatrix::zeros(12);
    let ea = e * a / l;
    place_pair(&mut k, 0, [[ea, -ea], [-ea, ea]]);
    let gj = g * section.torsion() / l;
    place_pair(&mut k, 3, [[gj, -gj], [-gj, gj]]);
    // Plane x-y: v with theta_z, I_z, shear along y.
    let pxy = plane_stiffness(e * section.i_z(), section.shear_y() * g * a, l);
    place_plane(&mut k, &pxy, 1, 5, 1.0);
    // Plane x-z: w with psi = -theta_y, I_y, shear along z.
    let pxz = plane_stiffness(e * section.i_y(), section.shear_z() * g * a, l);
    place_plane(&mut k, &pxz, 2, 4, -1.0);
    k
}

/// The 12 x 12 consistent mass in local axes (ADR 0417 arithmetic 3).
pub fn local_mass(length: f64, section: &BeamSection, material: &Material) -> ElementMatrix {
    let (e, g) = moduli(material);
    let rho = material.density();
    let l = length;
    let a = section.area();
    let mut m = ElementMatrix::zeros(12);
    let ma = rho * a * l / 6.0;
    place_pair(&mut m, 0, [[2.0 * ma, ma], [ma, 2.0 * ma]]);
    let mt = rho * (section.i_y() + section.i_z()) * l / 6.0;
    place_pair(&mut m, 3, [[2.0 * mt, mt], [mt, 2.0 * mt]]);
    let pxy = plane_mass(
        rho * a,
        rho * section.i_z(),
        e * section.i_z(),
        section.shear_y() * g * a,
        l,
    );
    place_plane(&mut m, &pxy, 1, 5, 1.0);
    let pxz = plane_mass(
        rho * a,
        rho * section.i_y(),
        e * section.i_y(),
        section.shear_z() * g * a,
        l,
    );
    place_plane(&mut m, &pxz, 2, 4, -1.0);
    m
}

/// `T^T A T` with `T` four copies of the rotation `axes` (rows `e_1, e_2, e_3`).
fn to_global(a: &ElementMatrix, axes: &[Vec3; 3]) -> ElementMatrix {
    // B = A T, then T^T B, block by block.
    let mut b = [[0.0; 12]; 12];
    let av = a.values();
    for i in 0..12 {
        for bj in 0..4 {
            for c in 0..3 {
                let mut s = 0.0;
                for k in 0..3 {
                    s += av[i * 12 + 3 * bj + k] * axes[k][c];
                }
                b[i][3 * bj + c] = s;
            }
        }
    }
    let mut out = ElementMatrix::zeros(12);
    let ov = out.values_mut();
    for bi in 0..4 {
        for r in 0..3 {
            for j in 0..12 {
                let mut s = 0.0;
                for k in 0..3 {
                    s += axes[k][r] * b[3 * bi + k][j];
                }
                ov[(3 * bi + r) * 12 + j] = s;
            }
        }
    }
    // Exact symmetry.
    for i in 0..12 {
        for j in 0..i {
            let s = 0.5 * (ov[i * 12 + j] + ov[j * 12 + i]);
            ov[i * 12 + j] = s;
            ov[j * 12 + i] = s;
        }
    }
    out
}

/// The global stiffness of a beam element from its node coordinates, section, orientation vector and material.
pub fn element_stiffness(
    coords: &[Vec3],
    section: &BeamSection,
    orientation: Vec3,
    material: &Material,
) -> Result<ElementMatrix> {
    let f = frame_of(coords, orientation)?;
    Ok(to_global(&local_stiffness(f.length, section, material), &f.axes))
}

/// The global consistent mass of a beam element.
pub fn element_mass(
    coords: &[Vec3],
    section: &BeamSection,
    orientation: Vec3,
    material: &Material,
) -> Result<ElementMatrix> {
    let f = frame_of(coords, orientation)?;
    Ok(to_global(&local_mass(f.length, section, material), &f.axes))
}

fn frame_of(coords: &[Vec3], orientation: Vec3) -> Result<BeamFrame> {
    if coords.len() != 2 {
        return Err(FemError::FieldLength {
            key: "coordinates",
            expected: 2,
            found: coords.len(),
        });
    }
    frame(coords[0], coords[1], orientation)
}

// ------------------------------------------------------------------------------------- stress stiffening (ADR 0473)

/// The slopes `d'(xi)` of the deflection functions of a plane at `xi`, for the end unknowns `(d_0, psi_0, d_1, psi_1)`.
fn plane_slopes(c: &[[f64; 4]; 4], l: f64, xi: f64) -> [f64; 4] {
    let dd = [0.0, 1.0 / l, 2.0 * xi / l, 3.0 * xi * xi / l];
    let scale = [1.0, l, 1.0, l];
    let mut s = [0.0; 4];
    for q in 0..4 {
        for j in 0..4 {
            s[q] += dd[j] * c[j][q] * scale[q];
        }
    }
    s
}

/// The geometric stiffness of one bending plane, `N int d'^T d' dx` with the deflection functions of the plane (ADR 0473 D1),
/// by four-point Gauss (exact, degree 4).
fn plane_geometric(n: f64, ei: f64, ks: f64, l: f64) -> [[f64; 4]; 4] {
    let phi = 12.0 * ei / (ks * l * l);
    let c = plane_shape_coefficients(phi);
    let mut m = [[0.0; 4]; 4];
    for (x, w) in GAUSS4_X.iter().zip(GAUSS4_W) {
        let xi = 0.5 * (x + 1.0);
        let wl = 0.5 * w * l;
        let s = plane_slopes(&c, l, xi);
        for i in 0..4 {
            for j in i..4 {
                m[i][j] += wl * n * s[i] * s[j];
            }
        }
    }
    for i in 0..4 {
        for j in 0..i {
            m[i][j] = m[j][i];
        }
    }
    m
}

/// The 12 x 12 geometric stiffness in local axes of the axial force `axial_force` (N, positive in tension; ADR 0473 D1 and
/// arithmetic 1): `N int (u'^2 + v'^2 + w'^2) dx + N (I_y + I_z)/A int theta_x'^2 dx`, the lateral terms with the deflection
/// functions of each bending plane (interdependent, with its shear parameter), the axial and torsion terms with the linear
/// functions. The bending rotations take no term.
pub fn local_geometric_stiffness(
    length: f64,
    section: &BeamSection,
    material: &Material,
    axial_force: f64,
) -> ElementMatrix {
    let (e, g) = moduli(material);
    let l = length;
    let a = section.area();
    let mut k = ElementMatrix::zeros(12);
    let na = axial_force / l;
    place_pair(&mut k, 0, [[na, -na], [-na, na]]);
    let nt = axial_force * (section.i_y() + section.i_z()) / (a * l);
    place_pair(&mut k, 3, [[nt, -nt], [-nt, nt]]);
    let pxy = plane_geometric(axial_force, e * section.i_z(), section.shear_y() * g * a, l);
    place_plane(&mut k, &pxy, 1, 5, 1.0);
    let pxz = plane_geometric(axial_force, e * section.i_y(), section.shear_z() * g * a, l);
    place_plane(&mut k, &pxz, 2, 4, -1.0);
    k
}

fn check_stress(stress: &[f64; 6]) -> Result<()> {
    if stress.iter().any(|v| !v.is_finite()) {
        return Err(FemError::InvalidValue {
            key: "stress",
            reason: format!("the stress {stress:?} of a Beam2 element is not finite"),
        });
    }
    Ok(())
}

/// `e . S e` for the Voigt stress `s` (tensor shear).
fn normal_stress(s: &[f64; 6], e: Vec3) -> f64 {
    let t = [[s[0], s[3], s[5]], [s[3], s[1], s[4]], [s[5], s[4], s[2]]];
    let mut v = 0.0;
    for i in 0..3 {
        for j in 0..3 {
            v += e[i] * t[i][j] * e[j];
        }
    }
    v
}

/// The global geometric stiffness of a beam element for the stress `stress` (global Voigt, tensor shear) of its one pre-stress
/// point: the axial force `N = A e_1 . S e_1` (ADR 0473 D1, D4); the other components are not read. A non-finite stress is
/// refused (key `"stress"`).
pub fn element_geometric_stiffness(
    coords: &[Vec3],
    section: &BeamSection,
    orientation: Vec3,
    material: &Material,
    stress: &[f64; 6],
) -> Result<ElementMatrix> {
    check_stress(stress)?;
    let f = frame_of(coords, orientation)?;
    let n = section.area() * normal_stress(stress, f.axes[0]);
    Ok(to_global(
        &local_geometric_stiffness(f.length, section, material, n),
        &f.axes,
    ))
}

/// The element-average axial force `E A e_1 . (u_1 - u_0)/L` of a beam element for its twelve global unknowns `ue` (ADR 0473
/// D4; exact for a bar under any axial load with its consistent nodal load).
pub fn axial_force(
    coords: &[Vec3],
    section: &BeamSection,
    orientation: Vec3,
    material: &Material,
    ue: &[f64],
) -> Result<f64> {
    if ue.len() != 12 {
        return Err(FemError::FieldLength {
            key: "element unknowns",
            expected: 12,
            found: ue.len(),
        });
    }
    let f = frame_of(coords, orientation)?;
    let e1 = f.axes[0];
    let du: f64 = (0..3).map(|k| e1[k] * (ue[6 + k] - ue[k])).sum();
    Ok(material.youngs_modulus() * section.area() * du / f.length)
}

/// The rows over the twelve local unknowns of the fields `(u_1, u_2, u_3, theta_1, theta_2, theta_3)` at `xi` (ADR 0473 D3): the
/// axial and torsion fields linear, the lateral fields and the bending rotations the functions of each plane with the signs of
/// [`local_stiffness`].
fn field_rows(length: f64, section: &BeamSection, material: &Material, xi: f64) -> [[f64; 12]; 6] {
    let (e, g) = moduli(material);
    let a = section.area();
    let l = length;
    let phi_xy = 12.0 * e * section.i_z() / (section.shear_y() * g * a * l * l);
    let phi_xz = 12.0 * e * section.i_y() / (section.shear_z() * g * a * l * l);
    let (nd_xy, np_xy) = plane_functions(&plane_shape_coefficients(phi_xy), phi_xy, l, xi);
    let (nd_xz, np_xz) = plane_functions(&plane_shape_coefficients(phi_xz), phi_xz, l, xi);
    let mut r = [[0.0; 12]; 6];
    r[0][0] = 1.0 - xi;
    r[0][6] = xi;
    r[3][3] = 1.0 - xi;
    r[3][9] = xi;
    let (ixy, sxy) = ([1, 5, 7, 11], [1.0, 1.0, 1.0, 1.0]);
    let (ixz, sxz) = ([2, 4, 8, 10], [1.0, -1.0, 1.0, -1.0]);
    for q in 0..4 {
        r[1][ixy[q]] = sxy[q] * nd_xy[q];
        r[5][ixy[q]] = sxy[q] * np_xy[q];
        r[2][ixz[q]] = sxz[q] * nd_xz[q];
        // theta_y = -psi of the x-z plane.
        r[4][ixz[q]] = -sxz[q] * np_xz[q];
    }
    r
}

/// The 12 x 12 spin-softening mass in local axes for the unit axis `a` in local components (ADR 0473 D3, arithmetic 4): the
/// consistent mass minus `int (rho A (a . u_c)^2 + rho I_z (theta . (e_2 x a))^2 + rho I_y (theta . (e_3 x a))^2) dx`, the kinetic
/// energy of the motion along the axis of the section `u_c + theta x s`.
pub fn local_spin_softening_mass(length: f64, section: &BeamSection, material: &Material, a: Vec3) -> ElementMatrix {
    let mut m = local_mass(length, section, material);
    let rho = material.density();
    let (ma, mz, my) = (rho * section.area(), rho * section.i_z(), rho * section.i_y());
    let v = m.values_mut();
    for (x, w) in GAUSS4_X.iter().zip(GAUSS4_W) {
        let xi = 0.5 * (x + 1.0);
        let wl = 0.5 * w * length;
        let r = field_rows(length, section, material, xi);
        let mut ra = [0.0; 12];
        let mut rb = [0.0; 12];
        let mut rc = [0.0; 12];
        for q in 0..12 {
            ra[q] = a[0] * r[0][q] + a[1] * r[1][q] + a[2] * r[2][q];
            rb[q] = a[2] * r[3][q] - a[0] * r[5][q];
            rc[q] = a[0] * r[4][q] - a[1] * r[3][q];
        }
        for i in 0..12 {
            for j in 0..12 {
                v[i * 12 + j] -= wl * (ma * ra[i] * ra[j] + mz * rb[i] * rb[j] + my * rc[i] * rc[j]);
            }
        }
    }
    m
}

/// The global spin-softening mass of a beam element for a rotation about `axis` (any non-zero length; ADR 0473 D3).
pub fn element_spin_softening_mass(
    coords: &[Vec3],
    section: &BeamSection,
    orientation: Vec3,
    material: &Material,
    axis: Vec3,
) -> Result<ElementMatrix> {
    let f = frame_of(coords, orientation)?;
    let a = unit_or_error(axis)?;
    let al = [
        (0..3).map(|k| f.axes[0][k] * a[k]).sum(),
        (0..3).map(|k| f.axes[1][k] * a[k]).sum(),
        (0..3).map(|k| f.axes[2][k] * a[k]).sum(),
    ];
    Ok(to_global(
        &local_spin_softening_mass(f.length, section, material, al),
        &f.axes,
    ))
}

fn unit_or_error(axis: Vec3) -> Result<Vec3> {
    let n = norm(axis);
    if !(n.is_finite() && n > 0.0) {
        return Err(FemError::InvalidValue {
            key: "axis",
            reason: format!("{axis:?} is not a finite non-zero direction"),
        });
    }
    Ok([axis[0] / n, axis[1] / n, axis[2] / n])
}

/// The consistent nodal load (forces and moments, global axes, the element's unknown order) of the centrifugal load of a
/// rotation about `axis` through `origin` at `omega2 = Omega^2` on a beam element (ADR 0473 D5, arithmetic 5): the force per
/// unit length `rho A Omega^2 P (x - p)`, linear along the element, with the deflection functions of each plane and the linear
/// axial functions, and the section's distributed moment `-rho Omega^2 (a_2 I_z (e_2 x a) + a_3 I_y (e_3 x a))` with the
/// rotation functions; four-point Gauss (exact).
pub fn centrifugal_load_vector(
    coords: &[Vec3],
    section: &BeamSection,
    orientation: Vec3,
    material: &Material,
    axis: Vec3,
    origin: Vec3,
    omega2: f64,
) -> Result<[f64; 12]> {
    let f = frame_of(coords, orientation)?;
    let a = unit_or_error(axis)?;
    let rho = material.density();
    let ax = f.axes;
    let loc = |g: Vec3| -> Vec3 {
        [
            (0..3).map(|k| ax[0][k] * g[k]).sum(),
            (0..3).map(|k| ax[1][k] * g[k]).sum(),
            (0..3).map(|k| ax[2][k] * g[k]).sum(),
        ]
    };
    let al = loc(a);
    // The section's moment per unit length, local components: e_2 x a = (a_3, 0, -a_1), e_3 x a = (-a_2, a_1, 0).
    let (iz, iy) = (section.i_z(), section.i_y());
    let ml = [
        -rho * omega2 * (al[1] * iz * al[2] - al[2] * iy * al[1]),
        -rho * omega2 * (al[2] * iy * al[0]),
        -rho * omega2 * (-al[1] * iz * al[0]),
    ];
    let mut fl = [0.0; 12];
    for (x, w) in GAUSS4_X.iter().zip(GAUSS4_W) {
        let xi = 0.5 * (x + 1.0);
        let wl = 0.5 * w * f.length;
        let mut p = [0.0; 3];
        for k in 0..3 {
            p[k] = (1.0 - xi) * coords[0][k] + xi * coords[1][k] - origin[k];
        }
        let s = p[0] * a[0] + p[1] * a[1] + p[2] * a[2];
        let b = [
            rho * section.area() * omega2 * (p[0] - s * a[0]),
            rho * section.area() * omega2 * (p[1] - s * a[1]),
            rho * section.area() * omega2 * (p[2] - s * a[2]),
        ];
        let ql = loc(b);
        let r = field_rows(f.length, section, material, xi);
        for (q, fq) in fl.iter_mut().enumerate() {
            *fq += wl
                * (r[0][q] * ql[0]
                    + r[1][q] * ql[1]
                    + r[2][q] * ql[2]
                    + r[3][q] * ml[0]
                    + r[4][q] * ml[1]
                    + r[5][q] * ml[2]);
        }
    }
    let mut out = [0.0; 12];
    for blk in 0..4 {
        for c in 0..3 {
            out[3 * blk + c] = (0..3).map(|k| ax[k][c] * fl[3 * blk + k]).sum();
        }
    }
    Ok(out)
}

/// The consistent nodal load of a uniform force per unit length `q` (global axes) on a beam element from `coords[0]` to
/// `coords[1]`, in the element's unknown order (node 0: force, moment; node 1: force, moment). ADR 0422 arithmetic 1: in each
/// bending plane the interdependent shape functions integrate to `q L (1/2, L/12, 1/2, -L/12)` whatever the shear parameter,
/// so node 0 gets `q L / 2` and `(L^2 / 12) e_1 x q`, node 1 gets `q L / 2` and `-(L^2 / 12) e_1 x q`. Refuses other than two
/// coordinates and a zero or non-finite length.
pub fn line_load_vector(coords: &[Vec3], q: Vec3) -> Result<[f64; 12]> {
    if coords.len() != 2 {
        return Err(FemError::FieldLength {
            key: "coordinates",
            expected: 2,
            found: coords.len(),
        });
    }
    let d = [
        coords[1][0] - coords[0][0],
        coords[1][1] - coords[0][1],
        coords[1][2] - coords[0][2],
    ];
    let l = norm(d);
    if !(l > 0.0 && l.is_finite()) {
        return Err(FemError::InvalidValue {
            key: "length",
            reason: format!("the beam from {:?} to {:?} has length {l:e}", coords[0], coords[1]),
        });
    }
    let e1 = [d[0] / l, d[1] / l, d[2] / l];
    let m = cross(e1, q);
    let (h, c) = (0.5 * l, l * l / 12.0);
    let mut f = [0.0; 12];
    for k in 0..3 {
        f[k] = h * q[k];
        f[3 + k] = c * m[k];
        f[6 + k] = h * q[k];
        f[9 + k] = -c * m[k];
    }
    Ok(f)
}

/// The uniform force per unit length on a beam element from the model's loads (ADR 0422 D4): the sum of every line load
/// whose set holds the element and of gravity (`rho A g`); `None` when there is none.
fn element_line_load(model: &SolidModel, element: usize, section: &BeamSection, density: f64) -> Option<Vec3> {
    let mut q = [0.0; 3];
    let mut any = false;
    for load in model.loads() {
        match load {
            Load::LineLoad { set, force } => {
                if model.mesh().element_set(set).is_some_and(|s| s.contains(&element)) {
                    any = true;
                    for k in 0..3 {
                        q[k] += force[k];
                    }
                }
            }
            Load::Gravity { acceleration } => {
                any = true;
                for k in 0..3 {
                    q[k] += density * section.area() * acceleration[k];
                }
            }
            _ => {}
        }
    }
    any.then_some(q)
}

/// The section and orientation of the block of a beam element, if it is a Beam2 element of a block with a Beam property.
pub fn beam_property(mesh: &SolidMesh, element: usize) -> Option<(BeamSection, Vec3)> {
    if mesh.element_kind(element) != ElementKind::Beam2 {
        return None;
    }
    match mesh.blocks()[mesh.block_of(element)].property() {
        Some(BlockProperty::Beam { section, orientation }) => Some((*section, *orientation)),
        _ => None,
    }
}

fn property_or_error(mesh: &SolidMesh, element: usize) -> Result<(BeamSection, Vec3)> {
    beam_property(mesh, element).ok_or_else(|| FemError::InvalidValue {
        key: "property",
        reason: format!(
            "block {} (Beam2) has no Beam property: a beam needs its section and orientation",
            mesh.block_of(element)
        ),
    })
}

/// The checks of a model with beams (ADR 0417 D5): every Beam2 block has a Beam property, no node of a beam element is a
/// node of a solid element (refused with [`FemError::BeamSolidNode`], the first in element and node order), every beam
/// element has a frame.
pub fn check_beam_model(model: &SolidModel) -> Result<()> {
    let mesh = model.mesh();
    let mut any = false;
    for (b, block) in mesh.blocks().iter().enumerate() {
        if block.kind() == ElementKind::Beam2 {
            any = true;
            if !matches!(block.property(), Some(BlockProperty::Beam { .. })) {
                return Err(FemError::InvalidValue {
                    key: "property",
                    reason: format!("block {b} (Beam2) has no Beam property: a beam needs its section and orientation"),
                });
            }
        }
    }
    if !any {
        return Ok(());
    }
    let mut solid_of: Vec<Option<usize>> = vec![None; mesh.nodes().len()];
    for e in 0..mesh.n_elements() {
        if mesh.element_kind(e).is_solid() {
            for &n in mesh.element_nodes(e) {
                if solid_of[n].is_none() {
                    solid_of[n] = Some(e);
                }
            }
        }
    }
    for e in 0..mesh.n_elements() {
        if mesh.element_kind(e) != ElementKind::Beam2 {
            continue;
        }
        for &n in mesh.element_nodes(e) {
            if let Some(solid) = solid_of[n] {
                return Err(FemError::BeamSolidNode {
                    node: n,
                    beam: e,
                    solid,
                });
            }
        }
        let (_, v) = property_or_error(mesh, e)?;
        let nodes = mesh.element_nodes(e);
        frame(mesh.nodes()[nodes[0]], mesh.nodes()[nodes[1]], v).map_err(|source| FemError::Element {
            element: e,
            source: Box::new(source),
        })?;
    }
    Ok(())
}

/// The stiffness of a beam element of a model (its block's section and orientation, its material).
pub(crate) fn model_element_stiffness(
    mesh: &SolidMesh,
    element: usize,
    coords: &[Vec3],
    material: &Material,
) -> Result<ElementMatrix> {
    let (s, v) = property_or_error(mesh, element)?;
    element_stiffness(coords, &s, v, material)
}

/// The consistent mass of a beam element of a model.
pub(crate) fn model_element_mass(
    mesh: &SolidMesh,
    element: usize,
    coords: &[Vec3],
    material: &Material,
) -> Result<ElementMatrix> {
    let (s, v) = property_or_error(mesh, element)?;
    element_mass(coords, &s, v, material)
}

/// The section forces at the two ends of a beam element, in its local axes: `(N, V_y, V_z, T, M_y, M_z)`, the force and
/// moment that the part on the side of larger local x exerts on the other part (ADR 0417 reading (a4)).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SectionForces {
    /// At the section just after node 0.
    pub start: [f64; 6],
    /// At the section just before node 1.
    pub end: [f64; 6],
}

/// The section forces of every beam element of a model for a displacement vector (one value per unknown of
/// `model.dof_map()`), as (element, forces) in element order: `f = K_l T u_e - T f_e`, `start = -f_0`, `end = f_1`, with
/// `f_e` the element's consistent load of the model's line loads and gravity (ADR 0422 D4; none for an element without them).
pub fn section_forces(model: &SolidModel, displacement: &[f64]) -> Result<Vec<(usize, SectionForces)>> {
    let dofs = model.dof_map();
    if displacement.len() != dofs.n_equations() {
        return Err(FemError::FieldLength {
            key: "displacement",
            expected: dofs.n_equations(),
            found: displacement.len(),
        });
    }
    let mesh = model.mesh();
    let materials = model.material_indices()?;
    let mut out = Vec::new();
    for e in 0..mesh.n_elements() {
        if mesh.element_kind(e) != ElementKind::Beam2 {
            continue;
        }
        let located = |source: FemError| FemError::Element {
            element: e,
            source: Box::new(source),
        };
        let nodes = mesh.element_nodes(e);
        let (s, v) = property_or_error(mesh, e).map_err(located)?;
        let f = frame(mesh.nodes()[nodes[0]], mesh.nodes()[nodes[1]], v).map_err(located)?;
        let ue: Vec<f64> = element_dofs(&dofs, nodes).iter().map(|&q| displacement[q]).collect();
        if ue.len() != 12 {
            return Err(located(FemError::FieldLength {
                key: "element unknowns",
                expected: 12,
                found: ue.len(),
            }));
        }
        let mut ul = [0.0; 12];
        for b in 0..4 {
            for r in 0..3 {
                ul[3 * b + r] = (0..3).map(|k| f.axes[r][k] * ue[3 * b + k]).sum();
            }
        }
        let material = &model.materials()[materials[e]].1;
        let kl = local_stiffness(f.length, &s, material);
        let mut fl = kl.matvec(&ul);
        // ADR 0422 D4: the end forces are K_l u_l minus the element's consistent load.
        if let Some(q) = element_line_load(model, e, &s, material.density()) {
            let coords = [mesh.nodes()[nodes[0]], mesh.nodes()[nodes[1]]];
            let fg = line_load_vector(&coords, q).map_err(located)?;
            for b in 0..4 {
                for r in 0..3 {
                    fl[3 * b + r] -= (0..3).map(|k| f.axes[r][k] * fg[3 * b + k]).sum::<f64>();
                }
            }
        }
        let mut start = [0.0; 6];
        let mut end = [0.0; 6];
        for i in 0..6 {
            start[i] = -fl[i];
            end[i] = fl[6 + i];
        }
        out.push((e, SectionForces { start, end }));
    }
    Ok(out)
}
