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
            // ADR 0496 D4: a Shell4 element takes its block's layup through the shell's model function.
            let fe: Vec<f64> = if mesh.element_kind(e) == ElementKind::Shell3 {
                crate::shell::thickness_or_error(mesh, e).and_then(|t| {
                    let membrane = crate::shell3::model_membrane(mesh, e);
                    crate::shell3::thermal_load_vector_with(&coords, t, material, &dte, membrane).map(|v| v.to_vec())
                })
            } else {
                crate::shell::model_thermal_load_vector(mesh, e, &coords, material, &dte)
            }
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
            // ADR 0525 D1: a Hex8 element of a BBar or Incompatible block takes the formulation's load.
            _ if crate::hex8x::opted(mesh, e).is_some() => {
                let f = crate::hex8x::opted(mesh, e).expect("checked");
                crate::hex8x::element_data(material, points.as_ref(), e, &coords, Some(&dte)).and_then(|(ds, eth)| {
                    let eth = eth.ok_or_else(|| FemError::InvalidValue {
                        key: "temperature",
                        reason: "a thermal strain at each point needs the model's temperature field".into(),
                    })?;
                    crate::hex8x::element_thermal_load(f, &coords, &ds, &eth)
                })
            }
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

/// A nodal temperature field against time (ADR 0534 D1): a reference temperature (the stress-free temperature, K) and points
/// `(t_k, field_k)`, each field one temperature in K per node of the mesh, the times strictly increasing. Every node is
/// interpolated in time by the rule of [`crate::transient::LoadTable::value`] (the point value at a point time, `v_i + s (v_j -
/// v_i)` between), so a uniform field has at every time the bits of the table of its values. Taken by the quasi-static driver
/// ([`crate::nonlinear::TimeSteppingOptions::temperature_field`]) and the nonlinear transient integrator
/// ([`crate::nl_transient::NlTransientOptions::temperature_field`]) at the end time of every attempt or sub-step, with the
/// multiplicative thermal stretch `F_th = (1 + alpha (T - T_ref)) I` of the material's expansion (ADR 0534 arithmetic 1).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct TemperatureFieldHistory {
    reference: f64,
    points: Vec<(f64, Vec<f64>)>,
}

fn field_error(reason: String) -> FemError {
    FemError::InvalidValue {
        key: "temperature",
        reason,
    }
}

impl TemperatureFieldHistory {
    /// A field history; refuses (key `"temperature"`) fewer than two points, times that are not finite and strictly
    /// increasing, fields of zero or unequal length, a value or a reference that is not finite.
    pub fn new(reference: f64, points: Vec<(f64, Vec<f64>)>) -> Result<TemperatureFieldHistory> {
        if !reference.is_finite() {
            return Err(field_error(format!(
                "the reference temperature {reference:e} is not finite"
            )));
        }
        if points.len() < 2 {
            return Err(field_error(format!(
                "a temperature field history of {} points (at least 2)",
                points.len()
            )));
        }
        let n = points[0].1.len();
        if n == 0 {
            return Err(field_error("a temperature field of no node".to_string()));
        }
        for (i, (t, field)) in points.iter().enumerate() {
            if !t.is_finite() {
                return Err(field_error(format!("time {t:e} of point {i} is not finite")));
            }
            if i > 0 && *t <= points[i - 1].0 {
                return Err(field_error(format!(
                    "time {t:e} of point {i} does not follow {:e}",
                    points[i - 1].0
                )));
            }
            if field.len() != n {
                return Err(field_error(format!(
                    "the field of point {i} has {} values, the first has {n}",
                    field.len()
                )));
            }
            if let Some(b) = field.iter().position(|v| !v.is_finite()) {
                return Err(field_error(format!(
                    "the value {:e} at node {b} of point {i} is not finite",
                    field[b]
                )));
            }
        }
        Ok(TemperatureFieldHistory { reference, points })
    }

    /// One field held over `[t_start, t_end]`.
    pub fn constant(reference: f64, field: Vec<f64>, t_start: f64, t_end: f64) -> Result<TemperatureFieldHistory> {
        TemperatureFieldHistory::new(reference, vec![(t_start, field.clone()), (t_end, field)])
    }

    /// The reference temperature, K.
    pub fn reference(&self) -> f64 {
        self.reference
    }

    /// The points `(time, field)`.
    pub fn points(&self) -> &[(f64, Vec<f64>)] {
        &self.points
    }

    /// The number of nodes of every field.
    pub fn n_nodes(&self) -> usize {
        self.points[0].1.len()
    }

    /// The first and last time.
    pub fn range(&self) -> (f64, f64) {
        (self.points[0].0, self.points[self.points.len() - 1].0)
    }

    /// Whether the history covers `[t_a, t_b]` (exact comparison).
    pub fn covers(&self, t_a: f64, t_b: f64) -> bool {
        let (lo, hi) = self.range();
        lo <= t_a && t_b <= hi
    }

    /// The nodal temperatures at `t`, `None` outside the history; each node by the rule of `LoadTable::value`.
    pub fn value(&self, t: f64) -> Option<Vec<f64>> {
        let (lo, hi) = self.range();
        if !(lo <= t && t <= hi) {
            return None;
        }
        let j = self.points.partition_point(|(ti, _)| *ti < t);
        let (tj, fj) = &self.points[j];
        if *tj == t || j == 0 {
            return Some(fj.clone());
        }
        let (ti, fi) = &self.points[j - 1];
        let s = (t - ti) / (tj - ti);
        Some(fi.iter().zip(fj).map(|(vi, vj)| vi + s * (vj - vi)).collect())
    }
}

/// The nodal temperatures of a field at the time of one attempt (ADR 0534 D2): the values, the reference and whether the
/// thermal strain acts (off for the damping tangent of the integrator, D5).
#[derive(Debug, Clone)]
pub(crate) struct FieldAt {
    pub nodal: Vec<f64>,
    pub reference: f64,
    pub expansion: bool,
}

impl FieldAt {
    /// The history's value at `t` (the history covers `t`).
    pub fn of(history: &TemperatureFieldHistory, t: f64, expansion: bool) -> FieldAt {
        FieldAt {
            nodal: history.value(t).expect("temperature field coverage checked"),
            reference: history.reference(),
            expansion,
        }
    }
}

/// The point temperature `T_q = sum_b N_b T_b` and change `dT_q = sum_b N_b (T_b - T_ref)` of an element (ADR 0534
/// arithmetic 2); an element whose nodal values have the same bits `T_0` gives `(T_0, T_0 - T_ref)`, the exact interpolant.
pub(crate) fn point_temperature(n: &[f64], nodal: &[f64], reference: f64) -> (f64, f64) {
    let t0 = nodal[0];
    if nodal.iter().all(|t| t.to_bits() == t0.to_bits()) {
        return (t0, t0 - reference);
    }
    let t = n.iter().zip(nodal).map(|(nb, tb)| nb * tb).sum();
    let dt = n.iter().zip(nodal).map(|(nb, tb)| nb * (tb - reference)).sum();
    (t, dt)
}

/// The models a temperature field is offered to (ADR 0534 D4): solids of the `Full` formulation with a material of one
/// finite expansion coefficient; a field of one value per node with `1 + alpha (T_b - T_ref) > 0` at every node of every
/// element at every time of the history. Refused: a field of another length (`FieldLength`, key `"temperature"`), a shell
/// block and a block of another formulation (key `"temperature"`), a material without a single finite coefficient (key
/// `"expansion"`), a non-positive thermal stretch at a node (key `"temperature"`).
pub(crate) fn check_field_model(model: &SolidModel, history: &TemperatureFieldHistory) -> Result<()> {
    let mesh = model.mesh();
    if history.n_nodes() != mesh.nodes().len() {
        return Err(FemError::FieldLength {
            key: "temperature",
            expected: mesh.nodes().len(),
            found: history.n_nodes(),
        });
    }
    for (b, block) in mesh.blocks().iter().enumerate() {
        if matches!(block.kind(), ElementKind::Shell4 | ElementKind::Shell3) {
            return Err(field_error(format!(
                "block {b} ({}): a temperature field is offered to solid elements only (ADR 0534 D4)",
                block.kind().name()
            )));
        }
        if block.formulation() != crate::mesh::SolidFormulation::Full {
            return Err(field_error(format!(
                "block {b} ({} {}): a temperature field is offered to the Full formulation only (ADR 0534 D4)",
                block.kind().name(),
                block.formulation().name()
            )));
        }
    }
    let materials = model.material_indices()?;
    for e in 0..mesh.n_elements() {
        let material = &model.materials()[materials[e]].1;
        let alpha = material.expansion();
        if !alpha.is_finite() {
            return Err(FemError::InvalidValue {
                key: "expansion",
                reason: format!(
                    "element {e}: the {} material has no single finite expansion coefficient (ADR 0534 D4)",
                    material.name()
                ),
            });
        }
        for (t, field) in history.points() {
            for &node in mesh.element_nodes(e) {
                let theta = 1.0 + alpha * (field[node] - history.reference());
                if theta.is_nan() || theta <= 0.0 {
                    return Err(field_error(format!(
                        "node {node} at {t:e} s: the thermal stretch 1 + alpha dT = {theta:e} is not positive (ADR 0534 D4)"
                    )));
                }
            }
        }
    }
    Ok(())
}
