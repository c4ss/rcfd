//! Thermal strain load (ADR 0281). Declared by ADR 0271; filled by ADR 0281.
//!
//! The isotropic thermal strain `eps_th = alpha (T - T_ref) (1, 1, 1, 0, 0, 0)` (Voigt, engineering shear, no shear part)
//! of a nodal temperature field ([`crate::Load::Temperature`]), interpolated with each element's own functions from the
//! nodal differences `T_b - T_ref`. Its equivalent nodal load `f = int B^T D eps_th dV` is taken at the points of each
//! element's stiffness rule, the points at which `recover` evaluates the stress `sigma = D (eps - eps_th)`, so the recovered
//! stress is in discrete equilibrium with the loads (ADR 0281 arithmetic 2). A Shell4 element takes the membrane thermal
//! load of [`crate::shell::thermal_load_vector`], uniform through the thickness (ADR 0445 D2). A model has at most one
//! temperature field; a second one is refused. Every sum is sequential in a fixed order (elements, points, nodes): no dependence on threads.

use crate::element::{b_matrix, element_for};
use crate::error::{FemError, Result};
use crate::loads::{add_force, add_moment};
use crate::material::apply;
use crate::mesh::ElementKind;
use crate::model::{DofMap, Load, Material, SolidModel};
use rcfd_core::vec3::Vec3;

/// The nodal temperatures and the reference temperature of the model's temperature load, `None` without one. Refuses a
/// second temperature load (`InvalidValue { key: "temperature" }`).
pub fn temperature_field(model: &SolidModel) -> Result<Option<(&[f64], f64)>> {
    let mut found: Option<(&[f64], f64)> = None;
    for load in model.loads() {
        if let Load::Temperature { nodal, reference } = load {
            if found.is_some() {
                return Err(FemError::InvalidValue {
                    key: "temperature",
                    reason: "a model has one temperature field; a second Load::Temperature was given".into(),
                });
            }
            found = Some((nodal.as_slice(), *reference));
        }
    }
    Ok(found)
}

/// The thermal strain `alpha dT (1, 1, 1, 0, 0, 0)` of a material for a temperature change `dT` from the reference.
pub fn thermal_strain(material: &Material, delta_t: f64) -> [f64; 6] {
    let a = material.expansion() * delta_t;
    [a, a, a, 0.0, 0.0, 0.0]
}

/// The temperature change at a point, `sum_b N_b dT_b`, from the shape function values and the nodal changes of an element
/// (both in its node order).
pub fn point_temperature_change(n: &[f64], delta_t_nodal: &[f64]) -> f64 {
    n.iter().zip(delta_t_nodal).map(|(nb, tb)| nb * tb).sum()
}

/// The element thermal load `sum_q w_q B_q^T D eps_th,q` over the points of the kind's stiffness rule: `3 n` entries,
/// node-major. `delta_t_nodal` holds `T_b - T_ref` of the element's nodes in its node order.
pub fn element_thermal_load(
    kind: ElementKind,
    coords: &[Vec3],
    material: &Material,
    delta_t_nodal: &[f64],
) -> Result<Vec<f64>> {
    if crate::mat_ortho::is_point_material(material) {
        // ADR 0452 D5: the per-point materials need the element and the reference temperature (the model path).
        return Err(FemError::NotYetSupported {
            item: "material",
            variant: material.name(),
            location: "thermal::element_thermal_load".to_string(),
            package: crate::mat_ortho::FOLLOW_UP,
        });
    }
    let element = element_for(kind);
    if delta_t_nodal.len() != element.n_nodes() {
        return Err(FemError::FieldLength {
            key: "temperature",
            expected: element.n_nodes(),
            found: delta_t_nodal.len(),
        });
    }
    let d = material.elasticity_matrix()?;
    let size = 3 * element.n_nodes();
    let mut fe = vec![0.0; size];
    for ip in element.integration_points(coords, element.stiffness_rule())? {
        let dt = point_temperature_change(&ip.geometry.n, delta_t_nodal);
        let s = apply(&d, &thermal_strain(material, dt));
        let b = b_matrix(&ip.geometry.grad);
        for (c, fc) in fe.iter_mut().enumerate() {
            let mut v = 0.0;
            for (m, sm) in s.iter().enumerate() {
                v += b[m * size + c] * sm;
            }
            *fc += ip.weight * v;
        }
    }
    Ok(fe)
}

/// Adds the thermal load of the nodal field `nodal` (reference `reference`) to `f`, element by element in element order.
pub(crate) fn add_thermal_load(
    f: &mut [f64],
    dofs: &DofMap,
    model: &SolidModel,
    nodal: &[f64],
    reference: f64,
) -> Result<()> {
    let materials = model.material_indices()?;
    let mesh = model.mesh();
    let delta: Vec<f64> = nodal.iter().map(|t| t - reference).collect();
    // ADR 0452 D5: the per-point data, only for a model with an orthotropic or tabulated material.
    let points = if crate::mat_ortho::uses_point_materials(model) {
        Some(crate::mat_ortho::MaterialPoints::new(model)?)
    } else {
        None
    };
    for e in 0..mesh.n_elements() {
        let nodes = mesh.element_nodes(e);
        let coords: Vec<Vec3> = nodes.iter().map(|&n| mesh.nodes()[n]).collect();
        let dte: Vec<f64> = nodes.iter().map(|&n| delta[n]).collect();
        let material = &model.materials()[materials[e]].1;
        if matches!(mesh.element_kind(e), ElementKind::Shell4 | ElementKind::Shell3) {
            // ADR 0445 D2, ADR 0454 D2: the membrane thermal load of a shell, forces and moments at its nodes.
            let fe: Vec<f64> = crate::shell::thickness_or_error(mesh, e)
                .and_then(|t| {
                    if mesh.element_kind(e) == ElementKind::Shell3 {
                        let membrane = crate::shell3::model_membrane(mesh, e);
                        crate::shell3::thermal_load_vector_with(&coords, t, material, &dte, membrane)
                            .map(|v| v.to_vec())
                    } else {
                        let membrane = crate::shell::model_membrane(mesh, e);
                        crate::shell::thermal_load_vector_with(&coords, t, material, &dte, membrane).map(|v| v.to_vec())
                    }
                })
                .map_err(|source| FemError::Element {
                    element: e,
                    source: Box::new(source),
                })?;
            for (a, &node) in nodes.iter().enumerate() {
                add_force(f, dofs, node, &[fe[6 * a], fe[6 * a + 1], fe[6 * a + 2]])?;
                add_moment(f, dofs, node, &[fe[6 * a + 3], fe[6 * a + 4], fe[6 * a + 5]])?;
            }
            continue;
        }
        let fe = match &points {
            Some(points) if crate::mat_ortho::is_point_material(material) => {
                crate::mat_ortho::element_thermal_load(points, e, mesh.element_kind(e), &coords, material)
            }
            _ => element_thermal_load(mesh.element_kind(e), &coords, material, &dte),
        }
        .map_err(|source| FemError::Element {
            element: e,
            source: Box::new(source),
        })?;
        for (&node, fa) in nodes.iter().zip(fe.chunks(3)) {
            add_force(f, dofs, node, &[fa[0], fa[1], fa[2]])?;
        }
    }
    Ok(())
}
