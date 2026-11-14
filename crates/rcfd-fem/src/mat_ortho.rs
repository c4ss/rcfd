//! Orthotropic elasticity with material axes and orthotropic expansion. Declared by ADR 0440 (F-408, the scaffold of ADR 0311); filled by ADR 0312 (F-409).
//!
//! ADR 0452 (F-409): the compliance of nine engineering constants in material axes (F1), its positive definiteness (F2), the
//! stiffness as its inverse (F3), the rotation of the stiffness and of the thermal strain between material and global axes
//! (F4, F6), cylindrical axes, and the per-point element paths of the static solve (stiffness, thermal load; D4, D5). Voigt
//! order xx, yy, zz, xy, yz, zx (material 11, 22, 33, 12, 23, 31) with engineering shear strains.

use crate::assemble::ElementContext;
use crate::element::{b_matrix, element_for, ElementMatrix};
use crate::error::{FemError, Result};
use crate::material::{apply, Axes, MaterialAxes, Matrix6, PointContext};
use crate::mesh::ElementKind;
use crate::model::{Material, SolidModel};
use rcfd_core::vec3::Vec3;

/// The work package that fills this module; `None` once it has landed.
pub const PENDING: Option<&'static str> = None;

/// The package named when a path that does not take the orthotropic or tabulated material is given one (ADR 0452 D6).
pub const FOLLOW_UP: &str = "a follow-up of ADR 0452";

/// The Voigt pairs of indices, in Voigt order.
const PAIRS: [(usize, usize); 6] = [(0, 0), (1, 1), (2, 2), (0, 1), (1, 2), (2, 0)];

/// The compliance in material axes (ADR 0452 F1): `e = (E_1, E_2, E_3)`, `nu = (nu_12, nu_13, nu_23)`, `g = (G_12, G_13,
/// G_23)`; symmetric by construction.
pub fn compliance(e: [f64; 3], nu: [f64; 3], g: [f64; 3]) -> Matrix6 {
    let mut s = [[0.0; 6]; 6];
    s[0][0] = 1.0 / e[0];
    s[1][1] = 1.0 / e[1];
    s[2][2] = 1.0 / e[2];
    s[0][1] = -nu[0] / e[0];
    s[1][0] = s[0][1];
    s[0][2] = -nu[1] / e[0];
    s[2][0] = s[0][2];
    s[1][2] = -nu[2] / e[1];
    s[2][1] = s[1][2];
    s[3][3] = 1.0 / g[0];
    s[4][4] = 1.0 / g[2];
    s[5][5] = 1.0 / g[1];
    s
}

/// Refuses constants whose compliance is not positive definite (ADR 0452 F2), naming the ratio of the first failing 2 x 2
/// minor (`nu_12`, `nu_13`, `nu_23`) or, for the determinant, all three. The moduli must be positive (checked first).
pub fn check_positive_definite(e: [f64; 3], nu: [f64; 3], g: [f64; 3]) -> Result<()> {
    for (key, v) in [
        ("e_1", e[0]),
        ("e_2", e[1]),
        ("e_3", e[2]),
        ("g_12", g[0]),
        ("g_13", g[1]),
        ("g_23", g[2]),
    ] {
        if !(v > 0.0 && v.is_finite()) {
            return Err(FemError::InvalidMaterial {
                key,
                value: v,
                reason: "must be positive and finite",
            });
        }
    }
    let minors = [
        ("nu_12", nu[0], e[0] / e[1]),
        ("nu_13", nu[1], e[0] / e[2]),
        ("nu_23", nu[2], e[1] / e[2]),
    ];
    for (key, v, bound) in minors {
        if v.is_nan() || v * v >= bound {
            return Err(FemError::InvalidMaterial {
                key,
                value: v,
                reason: "the compliance is not positive definite: nu_ij^2 must be below E_i/E_j",
            });
        }
    }
    let nu21 = nu[0] * e[1] / e[0];
    let nu31 = nu[1] * e[2] / e[0];
    let nu32 = nu[2] * e[2] / e[1];
    let delta = 1.0 - nu[0] * nu21 - nu[1] * nu31 - nu[2] * nu32 - 2.0 * nu21 * nu32 * nu[1];
    if delta.is_nan() || delta <= 0.0 {
        return Err(FemError::InvalidMaterial {
            key: "nu_12, nu_13, nu_23",
            value: delta,
            reason: "the compliance is not positive definite: 1 - nu12 nu21 - nu13 nu31 - nu23 nu32 - 2 nu21 nu32 nu13 must be positive",
        });
    }
    Ok(())
}

/// The stiffness in material axes, the inverse of [`compliance`] (ADR 0452 F3): the normal block by cofactors over the
/// determinant, the shear diagonal `G_12, G_23, G_13`.
pub fn material_elasticity(e: [f64; 3], nu: [f64; 3], g: [f64; 3]) -> Matrix6 {
    let s = compliance(e, nu, g);
    let a = |i: usize, j: usize| s[i][j];
    let c00 = a(1, 1) * a(2, 2) - a(1, 2) * a(2, 1);
    let c01 = a(1, 2) * a(2, 0) - a(1, 0) * a(2, 2);
    let c02 = a(1, 0) * a(2, 1) - a(1, 1) * a(2, 0);
    let c11 = a(0, 0) * a(2, 2) - a(0, 2) * a(2, 0);
    let c12 = a(0, 1) * a(2, 0) - a(0, 0) * a(2, 1);
    let c22 = a(0, 0) * a(1, 1) - a(0, 1) * a(1, 0);
    let det = a(0, 0) * c00 + a(0, 1) * c01 + a(0, 2) * c02;
    let mut c = [[0.0; 6]; 6];
    c[0][0] = c00 / det;
    c[1][1] = c11 / det;
    c[2][2] = c22 / det;
    c[0][1] = c01 / det;
    c[1][0] = c[0][1];
    c[0][2] = c02 / det;
    c[2][0] = c[0][2];
    c[1][2] = c12 / det;
    c[2][1] = c[1][2];
    c[3][3] = g[0];
    c[4][4] = g[2];
    c[5][5] = g[1];
    c
}

/// The strain rotation `T` with `eps' = T eps` (engineering Voigt) for material axes `axes` (rows in global components;
/// ADR 0452 F4).
pub fn strain_rotation(axes: &Axes) -> Matrix6 {
    let r = axes;
    let mut t = [[0.0; 6]; 6];
    for (row, &(a, b)) in PAIRS.iter().enumerate() {
        let f = if a == b { 1.0 } else { 2.0 };
        for (col, &(i, j)) in PAIRS.iter().enumerate() {
            t[row][col] = if i == j {
                f * (r[a][i] * r[b][i])
            } else {
                f * ((r[a][i] * r[b][j] + r[a][j] * r[b][i]) / 2.0)
            };
        }
    }
    t
}

/// The global elasticity `T^T C T` of a material-axes elasticity `c` (ADR 0452 F4), computed for `j >= i` and mirrored.
pub fn rotate_elasticity(c: &Matrix6, axes: &Axes) -> Matrix6 {
    let t = strain_rotation(axes);
    let mut ct = [[0.0; 6]; 6];
    for (k, row) in ct.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = (0..6).map(|m| c[k][m] * t[m][j]).sum();
        }
    }
    let mut d = [[0.0; 6]; 6];
    for i in 0..6 {
        for j in i..6 {
            let v: f64 = (0..6).map(|k| t[k][i] * ct[k][j]).sum();
            d[i][j] = v;
            d[j][i] = v;
        }
    }
    d
}

/// The global engineering thermal strain of the material-axes normal strains `m` (ADR 0452 F6):
/// `eps_ij = sum_a R_ai R_aj m_a`, shears doubled.
pub fn global_thermal_strain(axes: &Axes, m: [f64; 3]) -> [f64; 6] {
    let mut out = [0.0; 6];
    for (k, &(i, j)) in PAIRS.iter().enumerate() {
        let v: f64 = (0..3).map(|a| axes[a][i] * axes[a][j] * m[a]).sum();
        out[k] = if i == j { v } else { 2.0 * v };
    }
    out
}

/// Cylindrical material axes at `x` about the line through `origin` along `axis`: rows radial, tangential, axial. Refuses a
/// point on the axis (key `"axes"`).
pub fn cylindrical_axes(origin: Vec3, axis: Vec3, x: Vec3) -> Result<Axes> {
    let len = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
    let e3 = [axis[0] / len, axis[1] / len, axis[2] / len];
    let d = [x[0] - origin[0], x[1] - origin[1], x[2] - origin[2]];
    let along = d[0] * e3[0] + d[1] * e3[1] + d[2] * e3[2];
    let r = [d[0] - along * e3[0], d[1] - along * e3[1], d[2] - along * e3[2]];
    let rn = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();
    let scale = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1.0);
    if rn.is_nan() || rn <= 1e-12 * scale {
        return Err(FemError::InvalidValue {
            key: "axes",
            reason: format!("the point {x:?} is on the axis of the cylindrical material axes"),
        });
    }
    let e1 = [r[0] / rn, r[1] / rn, r[2] / rn];
    let e2 = [
        e3[1] * e1[2] - e3[2] * e1[1],
        e3[2] * e1[0] - e3[0] * e1[2],
        e3[0] * e1[1] - e3[1] * e1[0],
    ];
    Ok([e1, e2, e3])
}

/// Whether a material is one of the per-point variants of ADR 0452 (`Orthotropic`, `Tabulated`).
pub fn is_point_material(m: &Material) -> bool {
    matches!(m, Material::Orthotropic { .. } | Material::Tabulated { .. })
}

/// Whether a model has a material of the per-point variants.
pub fn uses_point_materials(model: &SolidModel) -> bool {
    model.materials().iter().any(|(_, m)| is_point_material(m))
}

/// The axes rule of a per-point material (the base's for a table), `None` for an isotropic one.
fn axes_rule(m: &Material) -> Option<&MaterialAxes> {
    match m {
        Material::Orthotropic { axes, .. } => Some(axes),
        Material::Tabulated { base, .. } => axes_rule(base),
        _ => None,
    }
}

/// The global elasticity of an orthotropic material at a point (ADR 0452 D3): the point's axes, else the `Fixed` triad.
pub(crate) fn orthotropic_elasticity(
    e: [f64; 3],
    nu: [f64; 3],
    g: [f64; 3],
    axes: &MaterialAxes,
    point: &PointContext,
) -> Result<Matrix6> {
    let r = resolve_axes(axes, point)?;
    Ok(rotate_elasticity(&material_elasticity(e, nu, g), &r))
}

fn resolve_axes(axes: &MaterialAxes, point: &PointContext) -> Result<Axes> {
    if let Some(r) = point.axes {
        return Ok(r);
    }
    match axes {
        MaterialAxes::Fixed(r) => Ok(*r),
        _ => Err(FemError::InvalidValue {
            key: "axes",
            reason: "cylindrical or per-element material axes need the axes of the point".to_string(),
        }),
    }
}

/// The global thermal strain of a material at a point for the temperature change `delta_t` from `reference` (ADR 0452 D5):
/// isotropic `alpha dT (1, 1, 1, 0, 0, 0)`; orthotropic [`global_thermal_strain`] of `alpha_a dT`; tabulated the strains of
/// [`crate::mat_table::expansion_strain`] (isotropic base: the same in every direction).
pub fn thermal_strain(material: &Material, point: &PointContext, reference: f64, delta_t: f64) -> Result<[f64; 6]> {
    match material {
        Material::IsotropicElastic { .. } => Ok(crate::thermal::thermal_strain(material, delta_t)),
        Material::Orthotropic { expansion, axes, .. } => {
            let r = resolve_axes(axes, point)?;
            let m = [expansion[0] * delta_t, expansion[1] * delta_t, expansion[2] * delta_t];
            Ok(global_thermal_strain(&r, m))
        }
        Material::Tabulated { base, .. } => {
            let t = point.temperature.ok_or_else(needs_temperature)?;
            let m = crate::mat_table::expansion_strain(material, reference, t, delta_t)?;
            match base.as_ref() {
                Material::Orthotropic { axes, .. } => {
                    let r = resolve_axes(axes, point)?;
                    Ok(global_thermal_strain(&r, m))
                }
                _ => Ok([m[0], m[0], m[0], 0.0, 0.0, 0.0]),
            }
        }
        _ => Err(FemError::NotYetSupported {
            item: "material",
            variant: material.name(),
            location: "the thermal strain".to_string(),
            package: FOLLOW_UP,
        }),
    }
}

pub(crate) fn needs_temperature() -> FemError {
    FemError::InvalidValue {
        key: "temperature",
        reason: "a Tabulated material needs the temperature at the point".to_string(),
    }
}

/// What the per-point paths need of a model (ADR 0452 D4): its temperature field and each element's index in its
/// material's element set.
#[derive(Debug, Clone)]
pub struct MaterialPoints<'a> {
    model: &'a SolidModel,
    local: Vec<usize>,
    temperature: Option<(&'a [f64], f64)>,
}

impl<'a> MaterialPoints<'a> {
    /// The data of a model. Refuses a second temperature field and an element without material.
    pub fn new(model: &'a SolidModel) -> Result<MaterialPoints<'a>> {
        let materials = model.material_indices()?;
        let mut local = vec![0; materials.len()];
        for (set, _) in model.materials() {
            if let Some(elements) = model.mesh().element_set(set) {
                for (k, &e) in elements.iter().enumerate() {
                    local[e] = k;
                }
            }
        }
        Ok(MaterialPoints {
            model,
            local,
            temperature: crate::thermal::temperature_field(model)?,
        })
    }

    /// The reference temperature of the model's temperature field, if it has one.
    pub fn reference(&self) -> Option<f64> {
        self.temperature.map(|(_, r)| r)
    }

    /// The point context of element `e` at the point `x` with shape values `n` (element node order), and the temperature
    /// change from the reference there (0 without a temperature field).
    pub fn context(&self, e: usize, material: &Material, x: Vec3, n: &[f64]) -> Result<(PointContext, f64)> {
        let mut point = PointContext::default();
        if let Some(rule) = axes_rule(material) {
            let r = match rule {
                MaterialAxes::Fixed(r) => *r,
                MaterialAxes::Cylindrical { origin, axis } => cylindrical_axes(*origin, *axis, x)?,
                MaterialAxes::PerElement(list) => *list.get(self.local[e]).ok_or_else(|| FemError::InvalidValue {
                    key: "axes",
                    reason: format!(
                        "no per-element axes for the element at index {} of its set",
                        self.local[e]
                    ),
                })?,
            };
            point.axes = Some(r);
        }
        let mut dt = 0.0;
        if let Some((nodal, reference)) = self.temperature {
            let nodes = self.model.mesh().element_nodes(e);
            let dte: Vec<f64> = nodes.iter().map(|&k| nodal[k] - reference).collect();
            dt = crate::thermal::point_temperature_change(n, &dte);
            point = point.with_temperature(reference + dt)?;
        }
        Ok((point, dt))
    }
}

/// The element stiffness with the elasticity evaluated at each point of the kind's stiffness rule (ADR 0452 D5): the loops
/// of [`crate::element::Element::stiffness_with_rule`] with `D_q`.
pub fn element_stiffness(points: &MaterialPoints, ctx: &ElementContext) -> Result<ElementMatrix> {
    let element = element_for(ctx.kind);
    let size = 3 * element.n_nodes();
    let mut k = ElementMatrix::zeros(size);
    let mut db = vec![0.0; 6 * size];
    for ip in element.integration_points(ctx.coords, element.stiffness_rule())? {
        let (point, _) = points.context(ctx.element, ctx.material, ip.geometry.x, &ip.geometry.n)?;
        let d = ctx.material.elasticity_at(&point)?;
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

/// The element thermal load `sum_q w_q B_q^T D_q eps_th,q` with the elasticity and the thermal strain at each point (ADR
/// 0452 D5): the loops of [`crate::thermal::element_thermal_load`].
pub fn element_thermal_load(
    points: &MaterialPoints,
    e: usize,
    kind: ElementKind,
    coords: &[Vec3],
    material: &Material,
) -> Result<Vec<f64>> {
    let reference = points.reference().ok_or_else(|| FemError::InvalidValue {
        key: "temperature",
        reason: "a thermal load needs the model's temperature field".to_string(),
    })?;
    let element = element_for(kind);
    let size = 3 * element.n_nodes();
    let mut fe = vec![0.0; size];
    for ip in element.integration_points(coords, element.stiffness_rule())? {
        let (point, dt) = points.context(e, material, ip.geometry.x, &ip.geometry.n)?;
        let d = material.elasticity_at(&point)?;
        let s = apply(&d, &thermal_strain(material, &point, reference, dt)?);
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

/// The model checks of the per-point variants on the static path (ADR 0452 D6): a `PerElement` list of the wrong length,
/// the table columns, a table without the model's temperature field, the reference temperature outside a table with an
/// expansion column, and a beam or shell element of either variant.
pub fn check_model(model: &SolidModel) -> Result<()> {
    if !uses_point_materials(model) {
        return Ok(());
    }
    let temperature = crate::thermal::temperature_field(model)?;
    for (i, (set, m)) in model.materials().iter().enumerate() {
        if !is_point_material(m) {
            continue;
        }
        let elements = model.mesh().element_set(set).unwrap_or(&[]);
        if let Some(MaterialAxes::PerElement(list)) = axes_rule(m) {
            if list.len() != elements.len() {
                return Err(FemError::InvalidValue {
                    key: "axes",
                    reason: format!(
                        "material {i}, element set '{set}': {} per-element axes for {} elements",
                        list.len(),
                        elements.len()
                    ),
                });
            }
        }
        if let Material::Tabulated { .. } = m {
            crate::mat_table::check_columns(m)?;
            let Some((_, reference)) = temperature else {
                return Err(FemError::InvalidValue {
                    key: "temperature",
                    reason: format!(
                        "material {i}, element set '{set}': a Tabulated material needs the model's temperature field"
                    ),
                });
            };
            crate::mat_table::check_reference(m, reference)?;
        }
        for &e in elements {
            let kind = model.mesh().element_kind(e);
            if !kind.is_solid() {
                return Err(FemError::NotYetSupported {
                    item: "material",
                    variant: m.name(),
                    location: format!("element {e} ({})", kind.name()),
                    package: FOLLOW_UP,
                });
            }
        }
    }
    Ok(())
}

/// Refuses a model with a cyclic constraint and a material of either per-point variant (ADR 0457 D3, ADR 0459 D3: the cyclic
/// rows rotate translations; a material with its own axes is not opened with them, also with cylindrical axes about the
/// constraint's axis), naming the variant, its set and the constraint. A model without either is accepted.
pub(crate) fn refuse_with_cyclic(model: &SolidModel) -> Result<()> {
    let Some(j) = model
        .constraints()
        .iter()
        .position(|c| matches!(c, crate::model::Constraint::Cyclic { .. }))
    else {
        return Ok(());
    };
    match model
        .materials()
        .iter()
        .enumerate()
        .find(|(_, (_, m))| is_point_material(m))
    {
        Some((i, (set, m))) => Err(FemError::NotYetSupported {
            item: "material",
            variant: m.name(),
            location: format!("material {i}, element set '{set}', with the cyclic constraint {j}"),
            package: FOLLOW_UP,
        }),
        None => Ok(()),
    }
}

/// The material with its thermal expansion set to zero (ADR 0459 D2): an isotropic or orthotropic material with the expansion
/// 0, a table with its base so and its `expansion*` columns removed; any other material unchanged.
pub(crate) fn without_expansion(material: &Material) -> Material {
    let mut out = material.clone();
    match &mut out {
        Material::IsotropicElastic { expansion, .. } => *expansion = 0.0,
        Material::Orthotropic { expansion, .. } => *expansion = [0.0; 3],
        Material::Tabulated { base, columns, .. } => {
            **base = without_expansion(base);
            columns.retain(|(name, _)| !name.starts_with("expansion"));
        }
        _ => {}
    }
    out
}
