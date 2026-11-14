//! Geometric (initial-stress) stiffness and pre-stressed modal analysis. Declared by ADR 0291; filled by ADR 0390 (the
//! registration of ADR 0298, F-395).
//!
//! * [`element_geometric_stiffness`]: `K_sigma[(a, i), (b, j)] = delta_ij sum_q w_q det J_q (g_a . S_q g_b)`, the second
//!   variation of the Green-Lagrange strain energy at the fixed stress `S` (ADR 0390 D1), with the stress at the points of
//!   the element's stiffness rule; the elements of [`crate::element`] and [`crate::hex20`] call it from
//!   [`crate::element::Element::geometric_stiffness`].
//! * [`Prestress`]: the stress per element and rule point, from a static result ([`Prestress::from_static`], the stress of
//!   [`crate::recover::IntegrationPointField`]) or uniform (D2).
//! * [`geometric_stiffness`]: the assembled matrix on the pattern of [`crate::assemble::stiffness_with`] (D3), bit-identical
//!   for any thread count.
//! * [`prestressed_modes`]: `(K + K_sigma) phi = omega^2 M phi` by the steps of [`crate::modal::modal_analysis_with`] (D4).
//!
//! Linear buckling, `(K + lambda K_sigma) x = 0`, is [`crate::buckling::factors`].

use crate::assemble::{assemble_with, point_stiffness};
use crate::constraint::{check_rigid_body_motions, prescribed_equations, reduce};
use crate::element::{element_for, Element, ElementMatrix};
use crate::error::{FemError, Result};
use crate::mass::consistent_with;
use crate::modal::{ModalOptions, ModalResult};
use crate::model::{SolidModel, SolidResult};
use crate::recover::IntegrationPointField;
use rcfd_core::vec3::Vec3;
use rcfd_linalg::eigen::{lowest_eigenpairs, Shift, SubspaceOptions};
use rcfd_linalg::CsrMatrix;

/// The geometric stiffness of one element for the Voigt stresses `[xx, yy, zz, xy, yz, zx]` (tensor shear) at the points of
/// its stiffness rule, in the rule's order (ADR 0390 D1). Both triangles are computed. A stress list of the wrong length
/// (`FieldLength`, key `"stress"`) and a non-finite stress (`InvalidValue`, key `"stress"`) are refused.
pub fn element_geometric_stiffness<E: Element + ?Sized>(
    element: &E,
    coords: &[Vec3],
    stress: &[[f64; 6]],
) -> Result<ElementMatrix> {
    let rule = element.stiffness_rule();
    if stress.len() != rule.len() {
        return Err(FemError::FieldLength {
            key: "stress",
            expected: rule.len(),
            found: stress.len(),
        });
    }
    if let Some(q) = stress.iter().position(|s| s.iter().any(|v| !v.is_finite())) {
        return Err(FemError::InvalidValue {
            key: "stress",
            reason: format!(
                "the stress at point {q} of a {} element is not finite",
                element.kind().name()
            ),
        });
    }
    let nn = element.n_nodes();
    let size = 3 * nn;
    let mut k = ElementMatrix::zeros(size);
    for (ip, s) in element.integration_points(coords, rule)?.iter().zip(stress) {
        let t = [[s[0], s[3], s[5]], [s[3], s[1], s[4]], [s[5], s[4], s[2]]];
        let g = &ip.geometry.grad;
        // h_b = S g_b
        let h: Vec<[f64; 3]> = g
            .iter()
            .map(|gb| {
                let mut v = [0.0; 3];
                for (k, vk) in v.iter_mut().enumerate() {
                    *vk = t[k][0] * gb[0] + t[k][1] * gb[1] + t[k][2] * gb[2];
                }
                v
            })
            .collect();
        let kv = k.values_mut();
        for (a, ga) in g.iter().enumerate() {
            for (b, hb) in h.iter().enumerate() {
                let v = ip.weight * (ga[0] * hb[0] + ga[1] * hb[1] + ga[2] * hb[2]);
                for i in 0..3 {
                    kv[(3 * a + i) * size + 3 * b + i] += v;
                }
            }
        }
    }
    Ok(k)
}

/// The pre-stress of a model: one Voigt stress (tensor shear) per point of each element's stiffness rule, element by
/// element in element order (ADR 0390 D2).
#[derive(Debug, Clone, PartialEq)]
pub struct Prestress {
    offsets: Vec<usize>,
    stress: Vec<[f64; 6]>,
}

impl Prestress {
    /// The stress of an integration-point field.
    pub fn from_field(field: &IntegrationPointField) -> Prestress {
        Prestress {
            offsets: field.offsets().to_vec(),
            stress: field.stress().to_vec(),
        }
    }

    /// The stress of a static result of `model` (`D (eps - eps_th)` at the points of the stiffness rules). A model with a
    /// Beam2, Shell4 or Shell3 element takes [`crate::geometric_struct::prestress_from_static`] (ADR 0473 D4), also with solid
    /// elements beside them (ADR 0486 D1).
    pub fn from_static(model: &SolidModel, result: &SolidResult) -> Result<Prestress> {
        if crate::geometric_struct::has_structures(model) {
            return crate::geometric_struct::prestress_from_static(model, result);
        }
        Ok(Prestress::from_field(&IntegrationPointField::from_result(
            model, result,
        )?))
    }

    /// The same stress at every point of every element of `model` (a Beam2, Shell4 or Shell3 element has one point; ADR 0473
    /// D4).
    pub fn uniform(model: &SolidModel, stress: [f64; 6]) -> Result<Prestress> {
        // ADR 0457 D2: also a model of the orthotropic or the tabulated material; ADR 0473 D4: also beams and shells.
        model.check_elements_supported_with_materials()?;
        let mesh = model.mesh();
        let mut offsets = Vec::with_capacity(mesh.n_elements() + 1);
        offsets.push(0);
        for e in 0..mesh.n_elements() {
            let n = element_for(mesh.element_kind(e)).stiffness_rule().len();
            offsets.push(offsets[e] + n);
        }
        let total = *offsets.last().expect("one offset at least");
        Ok(Prestress {
            offsets,
            stress: vec![stress; total],
        })
    }

    /// A pre-stress of the given point offsets and stresses (ADR 0473 D4), checked against a model by
    /// [`Prestress::check_fits`] where it is used.
    pub(crate) fn from_parts(offsets: Vec<usize>, stress: Vec<[f64; 6]>) -> Prestress {
        Prestress { offsets, stress }
    }

    /// The pre-stress times `factor`.
    pub fn scaled(&self, factor: f64) -> Prestress {
        Prestress {
            offsets: self.offsets.clone(),
            stress: self.stress.iter().map(|s| s.map(|v| factor * v)).collect(),
        }
    }

    /// The same pre-stress with the stresses of every element `e` for which `keep(e)` is false set to zero (ADR 0486 D2): the
    /// stress of one part of a model alone.
    pub fn masked<F: Fn(usize) -> bool>(&self, keep: F) -> Prestress {
        let mut stress = self.stress.clone();
        for e in 0..self.offsets.len().saturating_sub(1) {
            if !keep(e) {
                for s in &mut stress[self.offsets[e]..self.offsets[e + 1]] {
                    *s = [0.0; 6];
                }
            }
        }
        Prestress {
            offsets: self.offsets.clone(),
            stress,
        }
    }

    /// `offsets()[e]..offsets()[e + 1]` are the points of element `e`.
    pub fn offsets(&self) -> &[usize] {
        &self.offsets
    }

    /// The stresses of every point.
    pub fn stress(&self) -> &[[f64; 6]] {
        &self.stress
    }

    /// The stresses at the points of element `e`.
    pub fn of_element(&self, e: usize) -> &[[f64; 6]] {
        &self.stress[self.offsets[e]..self.offsets[e + 1]]
    }

    /// Refuses a pre-stress whose element or point counts do not fit `model` (key `"prestress"`).
    pub fn check_fits(&self, model: &SolidModel) -> Result<()> {
        let mesh = model.mesh();
        let ne = mesh.n_elements();
        if self.offsets.len() != ne + 1 {
            return Err(FemError::FieldLength {
                key: "prestress",
                expected: ne + 1,
                found: self.offsets.len(),
            });
        }
        for e in 0..ne {
            let n = element_for(mesh.element_kind(e)).stiffness_rule().len();
            let found = self.offsets[e + 1].saturating_sub(self.offsets[e]);
            if found != n {
                return Err(FemError::Element {
                    element: e,
                    source: Box::new(FemError::FieldLength {
                        key: "prestress",
                        expected: n,
                        found,
                    }),
                });
            }
        }
        if *self.offsets.last().expect("one offset at least") != self.stress.len() {
            return Err(FemError::FieldLength {
                key: "prestress",
                expected: *self.offsets.last().expect("one offset at least"),
                found: self.stress.len(),
            });
        }
        Ok(())
    }
}

/// The assembled geometric stiffness of `model` for a pre-stress, on the pattern of the stiffness matrix (ADR 0390 D3),
/// with `threads` workers (`None`: the available parallelism); bit-identical for any value.
pub fn geometric_stiffness(model: &SolidModel, prestress: &Prestress, threads: Option<usize>) -> Result<CsrMatrix> {
    // ADR 0473 D6: also beams and shells.
    model.check_elements_supported_with_structures()?;
    assemble_geometric(model, prestress, threads)
}

/// The assembly of [`geometric_stiffness`] after its entry check (ADR 0457 D2: the pre-stressed modes make their own; ADR
/// 0459 D1: the rotating part and the buckling factors too).
pub(crate) fn assemble_geometric(
    model: &SolidModel,
    prestress: &Prestress,
    threads: Option<usize>,
) -> Result<CsrMatrix> {
    prestress.check_fits(model)?;
    assemble_with(model, threads, |ctx| {
        // ADR 0473 D6: a Beam2, Shell4 or Shell3 element takes the structural geometric stiffness.
        if crate::geometric_struct::is_structural(ctx.kind) {
            return crate::geometric_struct::element_geometric_stiffness(
                model.mesh(),
                ctx.element,
                ctx.coords,
                ctx.material,
                prestress.of_element(ctx.element),
            );
        }
        element_for(ctx.kind).geometric_stiffness(ctx.coords, prestress.of_element(ctx.element))
    })
}

/// `K + K_sigma` on their common pattern (value by value).
pub(crate) fn add_on_pattern(k: &CsrMatrix, kg: &CsrMatrix) -> CsrMatrix {
    debug_assert!(k.row_ptr == kg.row_ptr && k.col == kg.col);
    let mut out = k.clone();
    for (v, g) in out.val.iter_mut().zip(&kg.val) {
        *v += g;
    }
    out
}

/// The modes of the pre-stressed model, `(K + K_sigma) phi = omega^2 M phi` (ADR 0390 D4): the steps of
/// [`crate::modal::modal_analysis_with`] with `K + K_sigma` in place of `K`. With a zero pre-stress the result has the bits
/// of [`crate::modal::modal_analysis`]. Beyond the buckling load the factorisation refuses (`FemError::Solver`).
///
/// A model with linear constraints, rigid sets or elastic supports is solved on the transformation `x = T q` (ADR 0398):
/// `T^T (K_ext + K_sigma) T q = omega^2 T^T M T q`, `K_ext` with the supports, zero constraint values only, the default
/// shift `Shift::Auto` (ADR 0384), each mode expanded, `x = T q`; a negative eigenvalue (beyond the buckling load) is
/// refused (`FemError::Solver`). A cyclic constraint with an orthotropic or tabulated material is taken only when every
/// material with its own axes has cylindrical axes about the cyclic axis and origin; other axes are refused by name
/// ([`crate::mat_ortho::check_cyclic_axes`], ADR 0465).
pub fn prestressed_modes(model: &SolidModel, prestress: &Prestress, options: &ModalOptions) -> Result<ModalResult> {
    model.validate()?;
    // ADR 0457 D2: solid elements, also of the orthotropic and the tabulated material (the per-point stiffness); ADR 0473 D6:
    // also beams and shells.
    model.check_supported_with_materials()?;
    if crate::mpc::uses_transformation(model) {
        return prestressed_modes_on_transformation(model, prestress, options);
    }
    let dofs = model.dof_map();
    let fixed = prescribed_equations(model, &[])?;
    if let Some(&(eq, v)) = fixed.iter().find(|&&(_, v)| v != 0.0) {
        let (node, component) = dofs.node_component(eq).expect("existing equation");
        return Err(FemError::InvalidValue {
            key: "constraint",
            reason: format!(
                "prescribed value {v:e} at node {node} component {component}: a modal analysis takes zero values only"
            ),
        });
    }
    let rigid_free = match check_rigid_body_motions(model, &fixed) {
        Ok(()) => false,
        Err(FemError::RigidBodyMotionFree { .. }) => true,
        Err(e) => return Err(e),
    };
    let kg = assemble_geometric(model, prestress, options.threads)?;
    let k = add_on_pattern(&point_stiffness(model, options.threads)?, &kg);
    let m = consistent_with(model, options.threads)?;
    let zero = vec![0.0; k.n];
    let kr = reduce(&k, &zero, &fixed)?;
    let mr = reduce(&m, &zero, &fixed)?;
    let free = kr.free().to_vec();
    let nf = free.len();
    if options.modes == 0 || options.modes >= nf {
        return Err(FemError::InvalidValue {
            key: "modes",
            reason: format!(
                "{} modes requested of a model with {nf} free unknowns (at least 1 and fewer than the unknowns)",
                options.modes
            ),
        });
    }
    for (i, d) in mr.matrix.diag().iter().enumerate() {
        if d.is_nan() || *d <= 0.0 {
            let (node, component) = dofs.node_component(free[i]).expect("free equation exists");
            return Err(FemError::InvalidValue {
                key: "density",
                reason: format!("node {node} component {component} has no mass (diagonal {d:e})"),
            });
        }
    }
    let shift = options
        .shift
        .unwrap_or(if rigid_free { Shift::Auto } else { Shift::Value(0.0) });
    let opts = SubspaceOptions {
        shift,
        tolerance: options.tolerance,
        residual_test: options.residual_test,
        max_iterations: options.max_iterations,
        subspace_size: options.subspace_size,
        factorisation: options.factorisation,
        ..SubspaceOptions::default()
    };
    let eig = lowest_eigenpairs(&kr.matrix, &mr.matrix, options.modes, &opts).map_err(|e| FemError::Solver {
        solver: "lowest_eigenpairs",
        reason: e.to_string(),
    })?;
    let modes = eig
        .vectors
        .iter()
        .map(|v| {
            let mut full = vec![0.0; k.n];
            for (&eq, &x) in free.iter().zip(v) {
                full[eq] = x;
            }
            full
        })
        .collect();
    Ok(ModalResult::from_parts(eig, modes, nf, rigid_free, dofs))
}

/// Lowest eigenvalue below `-NEGATIVE_EIGENVALUE_RATIO |sigma|` (`sigma` the shift used): `K + K_sigma` is not positive
/// semidefinite on the admissible motions (ADR 0398 D2 step 5).
const NEGATIVE_EIGENVALUE_RATIO: f64 = 1e-8;

/// [`prestressed_modes`] on the transformation (ADR 0398 D2): the steps of [`crate::mpc::modal_analysis_mpc`] with `K_ext +
/// K_sigma` in place of `K_ext`.
fn prestressed_modes_on_transformation(
    model: &SolidModel,
    prestress: &Prestress,
    options: &ModalOptions,
) -> Result<ModalResult> {
    // ADR 0465 D1: a material with its own axes is taken with a cyclic constraint only with cylindrical axes about the
    // constraint's axis and origin (ADR 0462 D1), as on the other paths of the transformation.
    crate::mat_ortho::check_cyclic_axes(model)?;
    crate::mpc::refuse_nonzero_values(model, "a pre-stressed modal analysis")?;
    let dofs = model.dof_map();
    let tr = crate::mpc::transformation(model, &[])?;
    let n = tr.n_mesh();
    let mut k = point_stiffness(model, options.threads)?;
    crate::mpc::add_supports(&mut k, model)?;
    let kg = assemble_geometric(model, prestress, options.threads)?;
    let a = add_on_pattern(&k, &kg);
    let m = consistent_with(model, options.threads)?;
    let zero = vec![0.0; tr.n_unknowns()];
    let (ar, _) = tr.reduce(&a, &zero)?;
    let (mr, _) = tr.reduce(&m, &zero)?;
    let nq = ar.n;
    if options.modes == 0 || options.modes >= nq {
        return Err(FemError::InvalidValue {
            key: "modes",
            reason: format!(
                "{} modes requested of a model with {nq} free unknowns (at least 1 and fewer than the unknowns)",
                options.modes
            ),
        });
    }
    for (i, d) in mr.diag().iter().enumerate() {
        if d.is_nan() || *d <= 0.0 {
            return Err(FemError::InvalidValue {
                key: "density",
                reason: format!(
                    "{} has no mass (diagonal {d:e})",
                    crate::mpc::unknown_name(&tr, model, tr.independent()[i])
                ),
            });
        }
    }
    let opts = SubspaceOptions {
        shift: options.shift.unwrap_or(Shift::Auto),
        tolerance: options.tolerance,
        residual_test: options.residual_test,
        max_iterations: options.max_iterations,
        subspace_size: options.subspace_size,
        factorisation: options.factorisation,
        ..SubspaceOptions::default()
    };
    let eig = lowest_eigenpairs(&ar, &mr, options.modes, &opts).map_err(|e| FemError::Solver {
        solver: "lowest_eigenpairs",
        reason: e.to_string(),
    })?;
    if let Some(&lowest) = eig.values.first() {
        if lowest < -NEGATIVE_EIGENVALUE_RATIO * eig.shift.abs() {
            return Err(FemError::Solver {
                solver: "lowest_eigenpairs",
                reason: format!(
                    "eigenvalue {lowest:e} at shift {:e}: K + K_sigma is not positive semidefinite on the admissible \
                     motions (beyond the buckling load)",
                    eig.shift
                ),
            });
        }
    }
    let rigid_free = match options.shift {
        None => eig.shift != 0.0,
        Some(_) => crate::mpc::zero_shift_refused(&ar),
    };
    let modes = eig.vectors.iter().map(|q| tr.expand(q)[..n].to_vec()).collect();
    Ok(ModalResult::from_parts(eig, modes, nq, rigid_free, dofs))
}
