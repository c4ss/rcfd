//! Element library: shape functions, strain-displacement matrices and element matrices of Tet4, Hex8, Tet10 (ADR 0273
//! D1, D2, D4). Declared by ADR 0271; filled by ADR 0273.
//!
//! Reference coordinates and node orders are those of [`crate::mesh`] (ADR 0271 D2). Element unknowns are node-major,
//! `(u_x, u_y, u_z)` of node 0, then node 1, and so on; strains are in the Voigt order xx, yy, zz, xy, yz, zx with
//! engineering shear (ADR 0270 section 2.2).
//!
//! The trait leaves room for the later packages without an edit of this file: the mass matrix (ADR 0284) and body loads
//! (ADR 0276) are sums over [`Element::integration_points`] with [`Element::mass_rule`] or the stiffness rule; stresses at
//! the integration points (ADR 0277) come from [`Element::strain_at`] at the rule's points. The intermediate tier adds
//! provided methods that refuse by default (ADR 0291).

use crate::error::{FemError, Result};
use crate::material::Matrix6;
use crate::mesh::{ElementKind, TET_EDGES};
use crate::quadrature::{self, QuadratureRule};
use rcfd_core::vec3::Vec3;

/// The isoparametric map of an element at one point.
#[derive(Debug, Clone, PartialEq)]
pub struct PointGeometry {
    /// Reference coordinates of the point.
    pub xi: [f64; 3],
    /// Physical position of the point.
    pub x: Vec3,
    /// Shape function values, in the element's node order.
    pub n: Vec<f64>,
    /// Physical gradients of the shape functions, `dN_a/dx_k`.
    pub grad: Vec<[f64; 3]>,
    /// Jacobian `J[i][j] = dx_i/dxi_j`.
    pub jacobian: [[f64; 3]; 3],
    /// `det J`, positive.
    pub det_j: f64,
}

/// A point of a quadrature rule mapped to an element: its geometry and its weight `w det J` in physical volume.
#[derive(Debug, Clone, PartialEq)]
pub struct IntegrationPoint {
    /// Geometry at the point.
    pub geometry: PointGeometry,
    /// `w_q det J_q`.
    pub weight: f64,
}

/// A dense square element matrix, row-major.
#[derive(Debug, Clone, PartialEq)]
pub struct ElementMatrix {
    size: usize,
    values: Vec<f64>,
}

impl ElementMatrix {
    /// A zero matrix of the given size.
    pub fn zeros(size: usize) -> ElementMatrix {
        ElementMatrix {
            size,
            values: vec![0.0; size * size],
        }
    }
    /// Number of rows (= columns).
    pub fn size(&self) -> usize {
        self.size
    }
    /// Entry `(i, j)`.
    pub fn get(&self, i: usize, j: usize) -> f64 {
        self.values[i * self.size + j]
    }
    /// Row-major values.
    pub fn values(&self) -> &[f64] {
        &self.values
    }
    /// Mutable row-major values.
    pub fn values_mut(&mut self) -> &mut [f64] {
        &mut self.values
    }
    /// `K v`.
    pub fn matvec(&self, v: &[f64]) -> Vec<f64> {
        self.values
            .chunks(self.size)
            .map(|row| row.iter().zip(v).map(|(a, b)| a * b).sum())
            .collect()
    }
}

fn det3(j: &[[f64; 3]; 3]) -> f64 {
    j[0][0] * (j[1][1] * j[2][2] - j[1][2] * j[2][1]) - j[0][1] * (j[1][0] * j[2][2] - j[1][2] * j[2][0])
        + j[0][2] * (j[1][0] * j[2][1] - j[1][1] * j[2][0])
}

/// Inverse of a 3 x 3 matrix by the adjugate, given its determinant.
fn inverse3(j: &[[f64; 3]; 3], det: f64) -> [[f64; 3]; 3] {
    let mut inv = [[0.0; 3]; 3];
    for (r, row) in inv.iter_mut().enumerate() {
        for (c, v) in row.iter_mut().enumerate() {
            // (J^{-1})_{rc} = cofactor_{cr} / det
            let (r1, r2) = ((c + 1) % 3, (c + 2) % 3);
            let (c1, c2) = ((r + 1) % 3, (r + 2) % 3);
            *v = (j[r1][c1] * j[r2][c2] - j[r1][c2] * j[r2][c1]) / det;
        }
    }
    inv
}

/// The strain-displacement matrix (6 x 3n, row-major) from physical gradients (ADR 0273 D2).
pub fn b_matrix(grad: &[[f64; 3]]) -> Vec<f64> {
    let cols = 3 * grad.len();
    let mut b = vec![0.0; 6 * cols];
    for (a, g) in grad.iter().enumerate() {
        let c = 3 * a;
        b[c] = g[0];
        b[cols + c + 1] = g[1];
        b[2 * cols + c + 2] = g[2];
        b[3 * cols + c] = g[1];
        b[3 * cols + c + 1] = g[0];
        b[4 * cols + c + 1] = g[2];
        b[4 * cols + c + 2] = g[1];
        b[5 * cols + c] = g[2];
        b[5 * cols + c + 2] = g[0];
    }
    b
}

/// A finite element: shape functions, rules, and the element matrices built from them.
pub trait Element: Sync {
    /// The element kind.
    fn kind(&self) -> ElementKind;
    /// Shape function values at `xi`, in the node order of the kind.
    fn shape_functions(&self, xi: [f64; 3]) -> Vec<f64>;
    /// Derivatives of the shape functions with respect to the reference coordinates at `xi`.
    fn shape_derivatives(&self, xi: [f64; 3]) -> Vec<[f64; 3]>;
    /// The rule of the stiffness matrix.
    fn stiffness_rule(&self) -> &'static QuadratureRule;
    /// The rule proposed for the consistent mass matrix (ADR 0284 confirms or replaces it).
    fn mass_rule(&self) -> &'static QuadratureRule;

    /// Number of nodes.
    fn n_nodes(&self) -> usize {
        self.kind().nodes_per_element()
    }

    /// The isoparametric map at `xi` for the given node coordinates. Refuses a coordinate list of the wrong length and a
    /// point with `det J <= 0` or not finite.
    fn point(&self, coords: &[Vec3], xi: [f64; 3]) -> Result<PointGeometry> {
        let n_nodes = self.n_nodes();
        if coords.len() != n_nodes {
            return Err(FemError::FieldLength {
                key: "coordinates",
                expected: n_nodes,
                found: coords.len(),
            });
        }
        let n = self.shape_functions(xi);
        let dn = self.shape_derivatives(xi);
        let mut jac = [[0.0; 3]; 3];
        let mut x = [0.0; 3];
        for (a, p) in coords.iter().enumerate() {
            for i in 0..3 {
                x[i] += n[a] * p[i];
                for j in 0..3 {
                    jac[i][j] += p[i] * dn[a][j];
                }
            }
        }
        let det_j = det3(&jac);
        if det_j.is_nan() || det_j <= 0.0 || det_j.is_infinite() {
            return Err(FemError::InvalidValue {
                key: "jacobian",
                reason: format!(
                    "det J = {det_j:e} at the point ({}, {}, {}) of a {} element",
                    xi[0],
                    xi[1],
                    xi[2],
                    self.kind().name()
                ),
            });
        }
        let inv = inverse3(&jac, det_j);
        // dN/dx_k = sum_j (J^{-1})_{jk} dN/dxi_j, i.e. grad N = J^{-T} dN/dxi.
        let grad = dn
            .iter()
            .map(|d| {
                let mut g = [0.0; 3];
                for (k, gk) in g.iter_mut().enumerate() {
                    *gk = inv[0][k] * d[0] + inv[1][k] * d[1] + inv[2][k] * d[2];
                }
                g
            })
            .collect();
        Ok(PointGeometry {
            xi,
            x,
            n,
            grad,
            jacobian: jac,
            det_j,
        })
    }

    /// The points of `rule` mapped to the element, in the rule's order, each with its weight `w det J`.
    fn integration_points(&self, coords: &[Vec3], rule: &QuadratureRule) -> Result<Vec<IntegrationPoint>> {
        rule.points()
            .iter()
            .zip(rule.weights())
            .map(|(&xi, &w)| {
                let geometry = self.point(coords, xi)?;
                let weight = w * geometry.det_j;
                Ok(IntegrationPoint { geometry, weight })
            })
            .collect()
    }

    /// The element stiffness `sum_q w_q det J_q B_q^T D B_q` with the element's stiffness rule.
    fn stiffness(&self, coords: &[Vec3], d: &Matrix6) -> Result<ElementMatrix> {
        self.stiffness_with_rule(coords, d, self.stiffness_rule())
    }

    /// The element stiffness with a given rule (reduced integration and checks). Both triangles are computed.
    fn stiffness_with_rule(&self, coords: &[Vec3], d: &Matrix6, rule: &QuadratureRule) -> Result<ElementMatrix> {
        let size = 3 * self.n_nodes();
        let mut k = ElementMatrix::zeros(size);
        let mut db = vec![0.0; 6 * size];
        for ip in self.integration_points(coords, rule)? {
            let b = b_matrix(&ip.geometry.grad);
            for r in 0..6 {
                for c in 0..size {
                    let mut s = 0.0;
                    for (m, drm) in d[r].iter().enumerate() {
                        s += drm * b[m * size + c];
                    }
                    db[r * size + c] = s;
                }
            }
            let kv = k.values_mut();
            for i in 0..size {
                for j in 0..size {
                    let mut s = 0.0;
                    for m in 0..6 {
                        s += b[m * size + i] * db[m * size + j];
                    }
                    kv[i * size + j] += ip.weight * s;
                }
            }
        }
        Ok(k)
    }

    /// The strain `B u` at `xi` (Voigt, engineering shear) for element unknowns `u` (node-major, three per node).
    fn strain_at(&self, coords: &[Vec3], u: &[f64], xi: [f64; 3]) -> Result<[f64; 6]> {
        let size = 3 * self.n_nodes();
        if u.len() != size {
            return Err(FemError::FieldLength {
                key: "displacement",
                expected: size,
                found: u.len(),
            });
        }
        let p = self.point(coords, xi)?;
        let b = b_matrix(&p.grad);
        let mut eps = [0.0; 6];
        for (r, e) in eps.iter_mut().enumerate() {
            *e = b[r * size..(r + 1) * size].iter().zip(u).map(|(a, v)| a * v).sum();
        }
        Ok(eps)
    }
}

/// The linear tetrahedron.
#[derive(Debug, Clone, Copy, Default)]
pub struct Tet4;
/// The trilinear hexahedron, fully integrated (2 x 2 x 2 Gauss).
#[derive(Debug, Clone, Copy, Default)]
pub struct Hex8;
/// The quadratic tetrahedron.
#[derive(Debug, Clone, Copy, Default)]
pub struct Tet10;

impl Element for Tet4 {
    fn kind(&self) -> ElementKind {
        ElementKind::Tet4
    }
    fn shape_functions(&self, xi: [f64; 3]) -> Vec<f64> {
        vec![1.0 - xi[0] - xi[1] - xi[2], xi[0], xi[1], xi[2]]
    }
    fn shape_derivatives(&self, xi: [f64; 3]) -> Vec<[f64; 3]> {
        ElementKind::Tet4.shape_gradients(xi)
    }
    fn stiffness_rule(&self) -> &'static QuadratureRule {
        quadrature::tet_1()
    }
    fn mass_rule(&self) -> &'static QuadratureRule {
        quadrature::tet_4()
    }
}

impl Element for Hex8 {
    fn kind(&self) -> ElementKind {
        ElementKind::Hex8
    }
    fn shape_functions(&self, xi: [f64; 3]) -> Vec<f64> {
        ElementKind::Hex8
            .reference_nodes()
            .iter()
            .map(|a| (1.0 + xi[0] * a[0]) * (1.0 + xi[1] * a[1]) * (1.0 + xi[2] * a[2]) / 8.0)
            .collect()
    }
    fn shape_derivatives(&self, xi: [f64; 3]) -> Vec<[f64; 3]> {
        ElementKind::Hex8.shape_gradients(xi)
    }
    fn stiffness_rule(&self) -> &'static QuadratureRule {
        quadrature::gauss_2()
    }
    fn mass_rule(&self) -> &'static QuadratureRule {
        quadrature::gauss_3()
    }
}

impl Element for Tet10 {
    fn kind(&self) -> ElementKind {
        ElementKind::Tet10
    }
    fn shape_functions(&self, xi: [f64; 3]) -> Vec<f64> {
        let l = [1.0 - xi[0] - xi[1] - xi[2], xi[0], xi[1], xi[2]];
        let mut n = Vec::with_capacity(10);
        for li in l {
            n.push(li * (2.0 * li - 1.0));
        }
        for [i, j] in TET_EDGES {
            n.push(4.0 * l[i] * l[j]);
        }
        n
    }
    fn shape_derivatives(&self, xi: [f64; 3]) -> Vec<[f64; 3]> {
        ElementKind::Tet10.shape_gradients(xi)
    }
    fn stiffness_rule(&self) -> &'static QuadratureRule {
        quadrature::tet_4()
    }
    fn mass_rule(&self) -> &'static QuadratureRule {
        quadrature::tet_14()
    }
}

/// The element of a kind.
pub fn element_for(kind: ElementKind) -> &'static dyn Element {
    match kind {
        ElementKind::Tet4 => &Tet4,
        ElementKind::Hex8 => &Hex8,
        ElementKind::Tet10 => &Tet10,
    }
}
