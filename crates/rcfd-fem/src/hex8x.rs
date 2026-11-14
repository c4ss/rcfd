//! Hex8 without locking: mean-dilatation (B-bar) and incompatible-mode formulations, selected per element block by
//! [`crate::mesh::SolidFormulation`]. Declared by ADR 0291; filled by ADR 0493 (F-390, the stub ADR 0293).
//!
//! * `BBar` (ADR 0493 arithmetic 1; the mean dilatation of Nagtegaal, Parks and Rice in Hughes's B-bar form): at each point of
//!   the 2 x 2 x 2 rule the dilatation rows of `B` are replaced by the element mean of the shape-function gradients,
//!   `Bbar[i][3a + c] = B[i][3a + c] + (bbar_a[c] - grad_c N_a) / 3` for the normal rows `i = 0, 1, 2`, with `bbar_a = sum_q w_q
//!   det J_q grad N_a(q) / V`; `K = sum_q w_q det J_q Bbar_q^T D Bbar_q`, the strain `Bbar_q u`.
//! * `Incompatible` (ADR 0493 arithmetic 2; Wilson, Taylor, Doherty and Ghaboussi's modes `1 - xi_k^2` with the correction of
//!   Taylor, Beresford and Wilson): nine internal unknowns, the mode gradients `G_k(q) = (det J_0 / det J_q) J_0^{-T}
//!   dP_k/dxi(q)` with the Jacobian at the centre, so that the enhanced strain integrates to zero over any element (the patch
//!   test holds on distorted elements); the modes are condensed, `K = K_uu - K_ua K_aa^{-1} K_au`, and recovered from the
//!   displacement, `alpha = -K_aa^{-1} K_au u`, the strain `B_q u + B_alpha,q alpha`.
//!
//! Only the linear-elastic paths take them (ADR 0493 D4); the paths of D5 refuse them naming [`FOLLOW_UP`].

use crate::element::{b_matrix, element_for, Element, ElementMatrix, Hex8, PointGeometry};
use crate::error::{FemError, Result};
use crate::material::Matrix6;
use crate::mesh::{ElementKind, SolidFormulation, SolidMesh};
use crate::model::{Load, SolidModel};
use crate::quadrature;
use rcfd_core::vec3::Vec3;

/// The work package that adds the formulations `BBar` and `Incompatible`; `None` once it has landed.
pub const PENDING: Option<&'static str> = None;

/// The package named by the refusals of the paths that do not take the formulations (ADR 0493 D5).
pub const FOLLOW_UP: &str = "a follow-up of ADR 0493";

const SIZE: usize = 24;
const MODES: usize = 9;

/// The formulation of a Hex8 element of a mesh when its block asks for one other than `Full`; `None` for every other element.
pub(crate) fn opted(mesh: &SolidMesh, element: usize) -> Option<SolidFormulation> {
    if mesh.element_kind(element) != ElementKind::Hex8 {
        return None;
    }
    match mesh.blocks()[mesh.block_of(element)].formulation() {
        SolidFormulation::Full => None,
        f => Some(f),
    }
}

/// The first block of a mesh that asks for a formulation other than `Full`.
fn first_opted(model: &SolidModel) -> Option<(usize, SolidFormulation)> {
    model
        .mesh()
        .blocks()
        .iter()
        .enumerate()
        .find(|(_, b)| b.formulation() != SolidFormulation::Full)
        .map(|(i, b)| (i, b.formulation()))
}

fn refusal(block: usize, formulation: SolidFormulation, what: String) -> FemError {
    FemError::NotYetSupported {
        item: "formulation",
        variant: formulation.name(),
        location: format!("block {block}, {what}"),
        package: FOLLOW_UP,
    }
}

/// The refusals of ADR 0493 D5 that every path makes: with a block of a formulation other than `Full`, a temperature load
/// and a per-point material (orthotropic or tabulated) on such a block. Returns at once when no block asks for one.
pub(crate) fn check_model(model: &SolidModel) -> Result<()> {
    let Some((block, f)) = first_opted(model) else {
        return Ok(());
    };
    if let Some(i) = model.loads().iter().position(|l| matches!(l, Load::Temperature { .. })) {
        return Err(refusal(block, f, format!("load {i} (a temperature load)")));
    }
    let mesh = model.mesh();
    let materials = model.material_indices()?;
    for (e, &m) in materials.iter().enumerate() {
        if let Some(f) = opted(mesh, e) {
            let material = &model.materials()[m].1;
            if crate::mat_ortho::is_point_material(material) {
                return Err(refusal(
                    mesh.block_of(e),
                    f,
                    format!("the per-point material {} of element {e}", material.name()),
                ));
            }
        }
    }
    Ok(())
}

/// Refuses a model with a block of a formulation other than `Full` on a path that does not take them (ADR 0493 D5).
pub(crate) fn refuse_formulations(model: &SolidModel, path: &str) -> Result<()> {
    match first_opted(model) {
        None => Ok(()),
        Some((block, f)) => Err(refusal(block, f, path.to_string())),
    }
}

fn check_coords(coords: &[Vec3]) -> Result<()> {
    if coords.len() != 8 {
        return Err(FemError::FieldLength {
            key: "coordinates",
            expected: 8,
            found: coords.len(),
        });
    }
    Ok(())
}

/// The points of the stiffness rule mapped to the element (geometry and weight `w det J`).
fn points(coords: &[Vec3]) -> Result<Vec<(PointGeometry, f64)>> {
    check_coords(coords)?;
    Ok(Hex8
        .integration_points(coords, quadrature::gauss_2())?
        .into_iter()
        .map(|ip| (ip.geometry, ip.weight))
        .collect())
}

/// `k += w B1^T D B2` for row-major `6 x n1` and `6 x n2` matrices into an `n1 x n2` row-major `k`, in the loop order of
/// `Element::stiffness_with_rule`.
fn add_btdb(k: &mut [f64], b1: &[f64], n1: usize, d: &Matrix6, b2: &[f64], n2: usize, w: f64) {
    let mut db = vec![0.0; 6 * n2];
    for r in 0..6 {
        for c in 0..n2 {
            let mut s = 0.0;
            for (m, drm) in d[r].iter().enumerate() {
                s += drm * b2[m * n2 + c];
            }
            db[r * n2 + c] = s;
        }
    }
    for i in 0..n1 {
        for j in 0..n2 {
            let mut s = 0.0;
            for m in 0..6 {
                s += b1[m * n1 + i] * db[m * n2 + j];
            }
            k[i * n2 + j] += w * s;
        }
    }
}

fn mul(b: &[f64], cols: usize, u: &[f64]) -> [f64; 6] {
    std::array::from_fn(|r| b[r * cols..(r + 1) * cols].iter().zip(u).map(|(a, v)| a * v).sum())
}

/// The B-bar matrices (6 x 24) at the points of the rule, with their weights.
fn bbar_matrices(coords: &[Vec3]) -> Result<Vec<(Vec<f64>, f64)>> {
    let pts = points(coords)?;
    let volume: f64 = pts.iter().map(|(_, w)| w).sum();
    let mut mean = [[0.0; 3]; 8];
    for (g, w) in &pts {
        for (m, gr) in mean.iter_mut().zip(&g.grad) {
            for c in 0..3 {
                m[c] += w * gr[c];
            }
        }
    }
    for m in mean.iter_mut() {
        for v in m.iter_mut() {
            *v /= volume;
        }
    }
    Ok(pts
        .into_iter()
        .map(|(g, w)| {
            let mut b = b_matrix(&g.grad);
            for (a, (m, gr)) in mean.iter().zip(&g.grad).enumerate() {
                for c in 0..3 {
                    let corr = (m[c] - gr[c]) / 3.0;
                    for i in 0..3 {
                        b[i * SIZE + 3 * a + c] += corr;
                    }
                }
            }
            (b, w)
        })
        .collect())
}

/// Inverse of a 3 x 3 matrix by the adjugate, given its determinant.
fn inverse3(j: &[[f64; 3]; 3], det: f64) -> [[f64; 3]; 3] {
    let mut inv = [[0.0; 3]; 3];
    for (r, row) in inv.iter_mut().enumerate() {
        for (c, v) in row.iter_mut().enumerate() {
            let (r1, r2) = ((c + 1) % 3, (c + 2) % 3);
            let (c1, c2) = ((r + 1) % 3, (r + 2) % 3);
            *v = (j[r1][c1] * j[r2][c2] - j[r1][c2] * j[r2][c1]) / det;
        }
    }
    inv
}

/// The parts of the incompatible element: `B_q` (6 x 24), `B_alpha,q` (6 x 9) and the weights at the points.
struct Modes {
    b: Vec<Vec<f64>>,
    ba: Vec<Vec<f64>>,
    w: Vec<f64>,
}

fn modes(coords: &[Vec3]) -> Result<Modes> {
    let pts = points(coords)?;
    let centre = Hex8.point(coords, [0.0; 3])?;
    let j0inv = inverse3(&centre.jacobian, centre.det_j);
    let mut out = Modes {
        b: Vec::with_capacity(8),
        ba: Vec::with_capacity(8),
        w: Vec::with_capacity(8),
    };
    for (g, w) in pts {
        let ratio = centre.det_j / g.det_j;
        // G_k[m] = (det J0 / det J) (J0^{-T} dP_k/dxi)_m = (det J0 / det J) (J0^{-1})_{k m} (-2 xi_k).
        let grads: Vec<[f64; 3]> = (0..3)
            .map(|k| std::array::from_fn(|m| ratio * j0inv[k][m] * (-2.0 * g.xi[k])))
            .collect();
        out.b.push(b_matrix(&g.grad));
        out.ba.push(b_matrix(&grads));
        out.w.push(w);
    }
    Ok(out)
}

/// `K_uu`, `K_ua` (24 x 9) and `K_aa` (9 x 9) of the incompatible element.
fn mode_matrices(m: &Modes, d: &Matrix6) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let mut kuu = vec![0.0; SIZE * SIZE];
    let mut kua = vec![0.0; SIZE * MODES];
    let mut kaa = vec![0.0; MODES * MODES];
    for q in 0..m.w.len() {
        add_btdb(&mut kuu, &m.b[q], SIZE, d, &m.b[q], SIZE, m.w[q]);
        add_btdb(&mut kua, &m.b[q], SIZE, d, &m.ba[q], MODES, m.w[q]);
        add_btdb(&mut kaa, &m.ba[q], MODES, d, &m.ba[q], MODES, m.w[q]);
    }
    (kuu, kua, kaa)
}

/// The Cholesky factor (lower, row-major) of the 9 x 9 `K_aa`; refuses a pivot that is not positive and finite.
fn cholesky(a: &[f64]) -> Result<Vec<f64>> {
    let n = MODES;
    let mut l = vec![0.0; n * n];
    for j in 0..n {
        let mut s = a[j * n + j];
        for k in 0..j {
            s -= l[j * n + k] * l[j * n + k];
        }
        if s.is_nan() || s <= 0.0 || s.is_infinite() {
            return Err(FemError::InvalidValue {
                key: "internal modes",
                reason: format!("the matrix of the incompatible modes has the pivot {s:e} at row {j}"),
            });
        }
        let ljj = s.sqrt();
        l[j * n + j] = ljj;
        for i in j + 1..n {
            let mut t = a[i * n + j];
            for k in 0..j {
                t -= l[i * n + k] * l[j * n + k];
            }
            l[i * n + j] = t / ljj;
        }
    }
    Ok(l)
}

/// Solves `L L^T x = b` in place.
fn cholesky_solve(l: &[f64], b: &mut [f64; MODES]) {
    let n = MODES;
    for i in 0..n {
        let mut s = b[i];
        for k in 0..i {
            s -= l[i * n + k] * b[k];
        }
        b[i] = s / l[i * n + i];
    }
    for i in (0..n).rev() {
        let mut s = b[i];
        for k in i + 1..n {
            s -= l[k * n + i] * b[k];
        }
        b[i] = s / l[i * n + i];
    }
}

/// `alpha = -K_aa^{-1} K_au u`.
fn alpha_of(kua: &[f64], l: &[f64], u: &[f64]) -> [f64; MODES] {
    let mut r = [0.0; MODES];
    for (k, rk) in r.iter_mut().enumerate() {
        let mut s = 0.0;
        for (i, ui) in u.iter().enumerate() {
            s += kua[i * MODES + k] * ui;
        }
        *rk = -s;
    }
    cholesky_solve(l, &mut r);
    r
}

fn check_u(u: &[f64]) -> Result<()> {
    if u.len() != SIZE {
        return Err(FemError::FieldLength {
            key: "displacement",
            expected: SIZE,
            found: u.len(),
        });
    }
    Ok(())
}

/// The element stiffness of a Hex8 with a formulation (ADR 0493 D1): `Full` is the element of the basic tier
/// (`element_for(Hex8).stiffness`, the same bits), `BBar` the mean-dilatation element, `Incompatible` the condensed element
/// with the nine modes, symmetric bit for bit. Refuses a coordinate list that is not of eight nodes and a point with `det J
/// <= 0`.
pub fn element_stiffness(formulation: SolidFormulation, coords: &[Vec3], d: &Matrix6) -> Result<ElementMatrix> {
    match formulation {
        SolidFormulation::Full => element_for(ElementKind::Hex8).stiffness(coords, d),
        SolidFormulation::BBar => {
            let mut k = ElementMatrix::zeros(SIZE);
            for (b, w) in bbar_matrices(coords)? {
                add_btdb(k.values_mut(), &b, SIZE, d, &b, SIZE, w);
            }
            Ok(k)
        }
        SolidFormulation::Incompatible => {
            let m = modes(coords)?;
            let (kuu, kua, kaa) = mode_matrices(&m, d);
            let l = cholesky(&kaa)?;
            // X = K_aa^{-1} K_au, column j of X from column j of K_au^T.
            let mut x = vec![[0.0; MODES]; SIZE];
            for (j, xj) in x.iter_mut().enumerate() {
                let mut col = [0.0; MODES];
                for (k, c) in col.iter_mut().enumerate() {
                    *c = kua[j * MODES + k];
                }
                cholesky_solve(&l, &mut col);
                *xj = col;
            }
            let s = |i: usize, j: usize| -> f64 { (0..MODES).map(|k| kua[i * MODES + k] * x[j][k]).sum() };
            let mut k = ElementMatrix::zeros(SIZE);
            let kv = k.values_mut();
            for i in 0..SIZE {
                for j in i..SIZE {
                    let sym = 0.5 * (s(i, j) + s(j, i));
                    kv[i * SIZE + j] = kuu[i * SIZE + j] - sym;
                    kv[j * SIZE + i] = kuu[j * SIZE + i] - sym;
                }
            }
            Ok(k)
        }
    }
}

/// The internal modes `alpha = -K_aa^{-1} K_au u_e` of an Incompatible Hex8 for its displacement (24 values, node-major),
/// ordered `3 k + c` (mode `1 - xi_k^2`, component `c`).
pub fn internal_modes(coords: &[Vec3], d: &Matrix6, u: &[f64]) -> Result<[f64; 9]> {
    check_u(u)?;
    let m = modes(coords)?;
    let (_, kua, kaa) = mode_matrices(&m, d);
    let l = cholesky(&kaa)?;
    Ok(alpha_of(&kua, &l, u))
}

/// The strain (Voigt, engineering shear) at the eight points of the stiffness rule, in the rule's order, of a Hex8 with a
/// formulation for its displacement: `Full` `Element::strain_at` (the recovery's call), `BBar` `Bbar_q u`, `Incompatible`
/// `B_q u + B_alpha,q alpha` with the recovered modes (ADR 0493 D1).
pub fn point_strains(formulation: SolidFormulation, coords: &[Vec3], d: &Matrix6, u: &[f64]) -> Result<Vec<[f64; 6]>> {
    check_u(u)?;
    match formulation {
        SolidFormulation::Full => {
            let el = element_for(ElementKind::Hex8);
            quadrature::gauss_2()
                .points()
                .iter()
                .map(|&xi| el.strain_at(coords, u, xi))
                .collect()
        }
        SolidFormulation::BBar => Ok(bbar_matrices(coords)?.iter().map(|(b, _)| mul(b, SIZE, u)).collect()),
        SolidFormulation::Incompatible => {
            let m = modes(coords)?;
            let (_, kua, kaa) = mode_matrices(&m, d);
            let l = cholesky(&kaa)?;
            let alpha = alpha_of(&kua, &l, u);
            Ok((0..m.w.len())
                .map(|q| {
                    let a = mul(&m.b[q], SIZE, u);
                    let b = mul(&m.ba[q], MODES, &alpha);
                    std::array::from_fn(|r| a[r] + b[r])
                })
                .collect())
        }
    }
}
