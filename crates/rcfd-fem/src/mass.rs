//! Mass matrices. Declared by ADR 0271; filled by ADR 0284.
//!
//! The consistent mass matrix is the kinetic energy of the interpolated velocity: `M_{(a,i),(b,j)} = delta_ij int rho N_a
//! N_b dV`, summed over the points of each element's mass rule (ADR 0273 D3: `tet_4` for Tet4, `gauss_3` for Hex8, `tet_14`
//! for Tet10; exact on straight tetrahedra and on every trilinear hexahedron). It is assembled with
//! [`crate::assemble::assemble_with`], on the stiffness pattern and bit-identical for any thread count.
//!
//! Rigid motions `t + w x (x - p)` are in every element space, so `R^T M S` of two of them is the exact integral of `rho r .
//! s`; [`mass_properties`] reads the mass, the centre of mass and the inertia tensor from it (ADR 0284 D1).

use crate::assemble::assemble_with;
use crate::element::{element_for, Element, ElementMatrix};
use crate::error::{FemError, Result};
use crate::model::SolidModel;
use crate::quadrature::QuadratureRule;
use rcfd_core::vec3::Vec3;
use rcfd_linalg::CsrMatrix;

/// The consistent element mass matrix with the element's mass rule.
pub fn element_mass(element: &dyn Element, coords: &[Vec3], density: f64) -> Result<ElementMatrix> {
    element_mass_with_rule(element, coords, density, element.mass_rule())
}

/// The consistent element mass matrix `rho sum_q w_q det J_q N_a N_b` (on each of the three components) with a given rule.
/// The scalar matrix is computed for `a <= b` and mirrored, so the result is exactly symmetric.
pub fn element_mass_with_rule(
    element: &dyn Element,
    coords: &[Vec3],
    density: f64,
    rule: &QuadratureRule,
) -> Result<ElementMatrix> {
    let n = element.n_nodes();
    let mut s = vec![0.0; n * n];
    for ip in element.integration_points(coords, rule)? {
        let w = ip.weight;
        let nv = &ip.geometry.n;
        for a in 0..n {
            let wa = w * nv[a];
            for b in a..n {
                s[a * n + b] += wa * nv[b];
            }
        }
    }
    let size = 3 * n;
    let mut m = ElementMatrix::zeros(size);
    let v = m.values_mut();
    for a in 0..n {
        for b in a..n {
            let mab = density * s[a * n + b];
            for i in 0..3 {
                v[(3 * a + i) * size + 3 * b + i] = mab;
                v[(3 * b + i) * size + 3 * a + i] = mab;
            }
        }
    }
    Ok(m)
}

/// The consistent mass matrix of a model, with the available parallelism.
pub fn consistent(model: &SolidModel) -> Result<CsrMatrix> {
    consistent_with(model, None)
}

/// The consistent mass matrix with `threads` workers (`None`: the available parallelism); bit-identical for any value.
pub fn consistent_with(model: &SolidModel, threads: Option<usize>) -> Result<CsrMatrix> {
    assemble_with(model, threads, |ctx| {
        element_mass(element_for(ctx.kind), ctx.coords, ctx.material.density())
    })
}

/// The six rigid motions of a model as nodal vectors (one value per equation of its `DofMap`): the unit translations along
/// x, y, z, then the unit rotations about the axes x, y, z through `about`, `e_k x (x - about)`.
pub fn rigid_motions(model: &SolidModel, about: Vec3) -> [Vec<f64>; 6] {
    let dofs = model.dof_map();
    let nodes = model.mesh().nodes();
    let n = dofs.n_equations();
    let mut r: [Vec<f64>; 6] = Default::default();
    for v in r.iter_mut() {
        *v = vec![0.0; n];
    }
    for (a, p) in nodes.iter().enumerate() {
        let y = [p[0] - about[0], p[1] - about[1], p[2] - about[2]];
        // e_x x y = (0, -y_z, y_y), e_y x y = (y_z, 0, -y_x), e_z x y = (-y_y, y_x, 0)
        let rot = [[0.0, -y[2], y[1]], [y[2], 0.0, -y[0]], [-y[1], y[0], 0.0]];
        for c in 0..dofs.components(a).min(3) {
            let eq = dofs.equation(a, c).expect("component of an existing node");
            r[c][eq] = 1.0;
            for k in 0..3 {
                r[3 + k][eq] = rot[k][c];
            }
        }
    }
    r
}

/// Mass, centre of mass and inertia tensor (about the centre of mass) of a model, from a mass matrix.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MassProperties {
    /// Total mass, kg.
    pub mass: f64,
    /// Centre of mass, m.
    pub centre: Vec3,
    /// Inertia tensor about the centre of mass, `int rho (|y|^2 I - y y^T)`, kg m^2.
    pub inertia: [[f64; 3]; 3],
}

fn m_dot(m: &CsrMatrix, a: &[f64], b: &[f64]) -> f64 {
    let mut mb = vec![0.0; m.n];
    m.matvec(b, &mut mb);
    a.iter().zip(&mb).map(|(x, y)| x * y).sum()
}

/// The mass properties `r^T M r` of the rigid motions (ADR 0284 D1): the mass from a translation, the centre of mass from
/// the translations against the rotations about the origin, and the inertia from the rotations about that centre.
pub fn mass_properties(model: &SolidModel, m: &CsrMatrix) -> Result<MassProperties> {
    let n = model.dof_map().n_equations();
    if m.n != n || m.ncols != n {
        return Err(FemError::FieldLength {
            key: "mass matrix",
            expected: n,
            found: m.n,
        });
    }
    let r0 = rigid_motions(model, [0.0; 3]);
    let mass = m_dot(m, &r0[0], &r0[0]);
    if !(mass > 0.0 && mass.is_finite()) {
        return Err(FemError::InvalidValue {
            key: "density",
            reason: format!("total mass {mass:e} is not positive"),
        });
    }
    // Translation against rotation about the origin: int rho e_i . (e_k x x) = m e_i . (e_k x x_c), and
    // e_y . (e_z x x) = x, e_z . (e_x x x) = y, e_x . (e_y x x) = z.
    let centre = [
        m_dot(m, &r0[1], &r0[5]) / mass,
        m_dot(m, &r0[2], &r0[3]) / mass,
        m_dot(m, &r0[0], &r0[4]) / mass,
    ];
    let rc = rigid_motions(model, centre);
    let mut inertia = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in i..3 {
            let v = m_dot(m, &rc[3 + i], &rc[3 + j]);
            inertia[i][j] = v;
            inertia[j][i] = v;
        }
    }
    Ok(MassProperties {
        mass,
        centre,
        inertia,
    })
}
