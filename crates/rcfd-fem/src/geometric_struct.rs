//! Geometric stiffness of beams and shells. Declared by ADR 0440 (F-408, the scaffold of ADR 0311); filled by ADR 0473 (F-420,
//! the package of ADR 0323).
//!
//! The stress stiffening of Beam2, Shell4 and Shell3 elements on the paths of [`crate::geometric`] (pre-stressed modes, the
//! assembled geometric stiffness), [`crate::buckling`] and [`crate::rotating`] (ADR 0473 D6), with no parallel path:
//!
//! * The pre-stress of a structural element is one point of [`crate::geometric::Prestress`] holding a stress (global Voigt,
//!   tensor shear; D4): a beam's is `(N/A) e_1 e_1^T`, `N` the element-average axial force; a shell's is the membrane
//!   resultant over the thickness, `(1/t) sum N_{alpha beta} e_alpha e_beta^T`, from [`crate::shell::stresses`] at the centre
//!   ([`prestress_from_static`]). A uniform pre-stress ([`crate::geometric::Prestress::uniform`]) gives each structural
//!   element the stress as given.
//! * The element geometric stiffness ([`element_geometric_stiffness`]): the beam's of its axial force with the deflection
//!   functions of its interdependent element and Wagner's torsion term ([`crate::beam::local_geometric_stiffness`], D1); the
//!   shells' of their membrane resultants on the three translations ([`crate::shell::local_geometric_stiffness`],
//!   [`crate::shell3::local_geometric_stiffness`], D2).
//! * The spin-softening mass ([`element_spin_softening_mass`]): the kinetic energy of the motion perpendicular to the axis of
//!   the element's own kinematics, the section `u_c + theta x s` of a beam and the director `u + zeta theta x n` of a shell
//!   (D3).
//!
//! The centrifugal load on a beam is [`crate::beam::centrifugal_load_vector`] (D5). The trait method
//! [`crate::element::Element::geometric_stiffness`] of the placeholders of Beam2, Shell4 and Shell3 still refuses: the
//! assembly calls this module for them.

use crate::assemble::element_dofs;
use crate::element::ElementMatrix;
use crate::error::{FemError, Result};
use crate::geometric::Prestress;
use crate::mesh::{ElementKind, SolidMesh};
use crate::model::{Material, SolidModel, SolidResult};
use rcfd_core::vec3::Vec3;

/// The work package that fills this module; `None` once it has landed (ADR 0473).
pub const PENDING: Option<&'static str> = None;

/// The package named by the refusals of this module (ADR 0473 D4).
pub const FOLLOW_UP: &str = "a follow-up of ADR 0473";

/// Whether elements of `kind` take the structural geometric stiffness of this module (Beam2, Shell4, Shell3).
pub fn is_structural(kind: ElementKind) -> bool {
    matches!(kind, ElementKind::Beam2 | ElementKind::Shell4 | ElementKind::Shell3)
}

/// Whether `model` has a Beam2, Shell4 or Shell3 element.
pub fn has_structures(model: &SolidModel) -> bool {
    model
        .mesh()
        .blocks()
        .iter()
        .any(|b| is_structural(b.kind()) && !b.is_empty())
}

fn tensor(s: &[f64; 6]) -> [[f64; 3]; 3] {
    [[s[0], s[3], s[5]], [s[3], s[1], s[4]], [s[5], s[4], s[2]]]
}

/// The membrane resultant `t (Lambda S Lambda^T)_{alpha beta}`, `alpha, beta = 1, 2`, of the stress `stress` (global Voigt,
/// tensor shear) on a facet with the axes `axes` (rows `e_1, e_2, e_3`), N/m (ADR 0473 D2). A non-finite stress is refused (key
/// `"stress"`).
pub fn membrane_resultant(axes: &[Vec3; 3], thickness: f64, stress: &[f64; 6]) -> Result<[[f64; 2]; 2]> {
    if stress.iter().any(|v| !v.is_finite()) {
        return Err(FemError::InvalidValue {
            key: "stress",
            reason: format!("the stress {stress:?} of a shell element is not finite"),
        });
    }
    let t = tensor(stress);
    let mut n = [[0.0; 2]; 2];
    for (al, row) in n.iter_mut().enumerate() {
        for (be, v) in row.iter_mut().enumerate() {
            let mut s = 0.0;
            for i in 0..3 {
                for j in 0..3 {
                    s += axes[al][i] * t[i][j] * axes[be][j];
                }
            }
            *v = thickness * s;
        }
    }
    Ok(n)
}

/// The stress of a shell element from its membrane resultant `(N_xx, N_yy, N_xy)` in the facet axes `axes`: `(1/t) sum N_{alpha
/// beta} e_alpha e_beta^T` in global Voigt (ADR 0473 D4).
fn shell_stress(axes: &[Vec3; 3], thickness: f64, membrane: [f64; 3]) -> [f64; 6] {
    let n = [[membrane[0], membrane[2]], [membrane[2], membrane[1]]];
    let mut t = [[0.0; 3]; 3];
    for (i, row) in t.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            let mut s = 0.0;
            for al in 0..2 {
                for be in 0..2 {
                    s += axes[al][i] * n[al][be] * axes[be][j];
                }
            }
            *v = s / thickness;
        }
    }
    [t[0][0], t[1][1], t[2][2], t[0][1], t[1][2], t[2][0]]
}

fn beam_property_or_error(mesh: &SolidMesh, element: usize) -> Result<(crate::mesh::BeamSection, Vec3)> {
    crate::beam::beam_property(mesh, element).ok_or_else(|| FemError::InvalidValue {
        key: "property",
        reason: format!(
            "block {} (Beam2) has no Beam property: a beam needs its section and orientation",
            mesh.block_of(element)
        ),
    })
}

/// The pre-stress of a static result of a model of beams and shells (ADR 0473 D4): per Beam2 element the stress `(N/A) e_1
/// e_1^T` of its average axial force ([`crate::beam::axial_force`]), per Shell4 or Shell3 element `(1/t) sum N_{alpha beta}
/// e_alpha e_beta^T` of the membrane resultants of [`crate::shell::stresses`] at its centre. A model that also has solid
/// elements is refused ([`FemError::NotYetSupported`], item `"pre-stress"`, package [`FOLLOW_UP`]).
pub fn prestress_from_static(model: &SolidModel, result: &SolidResult) -> Result<Prestress> {
    prestress_from_displacement(model, result.displacement_values())
}

/// [`prestress_from_static`] for a displacement vector (one value per unknown of `model.dof_map()`).
pub fn prestress_from_displacement(model: &SolidModel, displacement: &[f64]) -> Result<Prestress> {
    let mesh = model.mesh();
    let dofs = model.dof_map();
    if displacement.len() != dofs.n_equations() {
        return Err(FemError::FieldLength {
            key: "displacement",
            expected: dofs.n_equations(),
            found: displacement.len(),
        });
    }
    if let Some((b, block)) = mesh
        .blocks()
        .iter()
        .enumerate()
        .find(|(_, b)| b.kind().is_solid() && !b.is_empty())
    {
        return Err(FemError::NotYetSupported {
            item: "pre-stress",
            variant: "solid and structural elements",
            location: format!(
                "block {b} ({}) of a model with beams or shells: the pre-stress of a static result",
                block.kind().name()
            ),
            package: FOLLOW_UP,
        });
    }
    let materials = model.material_indices()?;
    let ne = mesh.n_elements();
    let mut shells: Vec<Option<crate::shell::ShellStresses>> = vec![None; ne];
    if mesh
        .blocks()
        .iter()
        .any(|b| matches!(b.kind(), ElementKind::Shell4 | ElementKind::Shell3))
    {
        for (e, s) in crate::shell::stresses(model, displacement)? {
            shells[e] = Some(s);
        }
    }
    let mut offsets = Vec::with_capacity(ne + 1);
    offsets.push(0);
    let mut stress = Vec::with_capacity(ne);
    for e in 0..ne {
        let located = |source: FemError| FemError::Element {
            element: e,
            source: Box::new(source),
        };
        let nodes = mesh.element_nodes(e);
        let s = match mesh.element_kind(e) {
            ElementKind::Beam2 => {
                let (section, v) = beam_property_or_error(mesh, e).map_err(located)?;
                let coords: Vec<Vec3> = nodes.iter().map(|&n| mesh.nodes()[n]).collect();
                let ue: Vec<f64> = element_dofs(&dofs, nodes).iter().map(|&q| displacement[q]).collect();
                let material = &model.materials()[materials[e]].1;
                let n = crate::beam::axial_force(&coords, &section, v, material, &ue).map_err(located)?;
                let f = crate::beam::frame(coords[0], coords[1], v).map_err(located)?;
                let e1 = f.axes[0];
                let sx = n / section.area();
                [
                    sx * e1[0] * e1[0],
                    sx * e1[1] * e1[1],
                    sx * e1[2] * e1[2],
                    sx * e1[0] * e1[1],
                    sx * e1[1] * e1[2],
                    sx * e1[2] * e1[0],
                ]
            }
            ElementKind::Shell4 | ElementKind::Shell3 => {
                let t = crate::shell::thickness_or_error(mesh, e).map_err(located)?;
                let st = shells[e].as_ref().expect("a shell element has its stresses");
                shell_stress(&st.axes, t, st.membrane)
            }
            _ => unreachable!("solid blocks are refused above"),
        };
        stress.push(s);
        offsets.push(e + 1);
    }
    Ok(Prestress::from_parts(offsets, stress))
}

fn one_point(stress: &[[f64; 6]]) -> Result<&[f64; 6]> {
    if stress.len() != 1 {
        return Err(FemError::FieldLength {
            key: "stress",
            expected: 1,
            found: stress.len(),
        });
    }
    Ok(&stress[0])
}

/// The geometric stiffness of a Beam2, Shell4 or Shell3 element `element` of `mesh` for the stress of its one pre-stress point
/// (ADR 0473 D1, D2): [`crate::beam::element_geometric_stiffness`] with the block's section and orientation,
/// [`crate::shell::element_geometric_stiffness`] or [`crate::shell3::element_geometric_stiffness`] with the block's thickness.
/// Another kind is refused (`InvalidValue`, key `"element kind"`).
pub fn element_geometric_stiffness(
    mesh: &SolidMesh,
    element: usize,
    coords: &[Vec3],
    material: &Material,
    stress: &[[f64; 6]],
) -> Result<ElementMatrix> {
    let s = one_point(stress)?;
    match mesh.element_kind(element) {
        ElementKind::Beam2 => {
            let (section, v) = beam_property_or_error(mesh, element)?;
            crate::beam::element_geometric_stiffness(coords, &section, v, material, s)
        }
        ElementKind::Shell4 => {
            let t = crate::shell::thickness_or_error(mesh, element)?;
            crate::shell::element_geometric_stiffness(coords, t, s)
        }
        ElementKind::Shell3 => {
            let t = crate::shell::thickness_or_error(mesh, element)?;
            crate::shell3::element_geometric_stiffness(coords, t, s)
        }
        other => Err(FemError::InvalidValue {
            key: "element kind",
            reason: format!("{} is not a structural element (Beam2, Shell4, Shell3)", other.name()),
        }),
    }
}

/// The spin-softening mass of a Beam2, Shell4 or Shell3 element for a rotation about the unit `axis` (ADR 0473 D3). Another
/// kind is refused (`InvalidValue`, key `"element kind"`).
pub fn element_spin_softening_mass(
    mesh: &SolidMesh,
    element: usize,
    coords: &[Vec3],
    material: &Material,
    axis: Vec3,
) -> Result<ElementMatrix> {
    match mesh.element_kind(element) {
        ElementKind::Beam2 => {
            let (section, v) = beam_property_or_error(mesh, element)?;
            crate::beam::element_spin_softening_mass(coords, &section, v, material, axis)
        }
        ElementKind::Shell4 => {
            let t = crate::shell::thickness_or_error(mesh, element)?;
            crate::shell::element_spin_softening_mass(coords, t, material.density(), axis)
        }
        ElementKind::Shell3 => {
            let t = crate::shell::thickness_or_error(mesh, element)?;
            crate::shell3::element_spin_softening_mass(coords, t, material.density(), axis)
        }
        other => Err(FemError::InvalidValue {
            key: "element kind",
            reason: format!("{} is not a structural element (Beam2, Shell4, Shell3)", other.name()),
        }),
    }
}
