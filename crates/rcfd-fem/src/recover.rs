//! Strain and stress recovery, invariants, strain energy (ADR 0277). Declared by ADR 0271; filled by ADR 0277.
//!
//! * [`IntegrationPointField`]: strain `B u` and stress `D eps` at every point of each element's stiffness rule, with the
//!   point's position and weight `w det J`, element by element in element order (D1); element means and the strain energy
//!   `sum w det J eps . sigma / 2` (D2); [`attach`] stores strain and stress in a [`SolidResult`]. With a temperature field
//!   (ADR 0281) the stress is `D (eps - eps_th)`, the thermal strain is kept per point and the energy is the elastic one.
//! * Nodal values (D3): per element by extrapolation from the points with a polynomial fitted to them
//!   ([`extrapolation_matrix`], [`element_nodal_values`]), then averaged over the elements of a region weighted by element
//!   volume ([`nodal_average`]); averaging across a material interface is refused.
//! * Invariants of a Voigt stress (D4): [`tensor_norm`], [`von_mises`], [`principal`] (values descending with orthonormal
//!   directions) and the bare closed trigonometric form [`principal_values_trigonometric`].
//!
//! Voigt order xx, yy, zz, xy, yz, zx; strains carry engineering shear, stresses tensor shear (ADR 0270 section 2.2). Every
//! loop is sequential in a fixed order: the results do not depend on threads.

use crate::assemble::element_dofs;
use crate::element::element_for;
use crate::error::{FemError, Result, SetKind};
use crate::material::PointContext;
use crate::mesh::ElementKind;
use crate::model::{SolidModel, SolidResult};
use crate::thermal;
use rcfd_core::vec3::{cross, dot, norm, Vec3};
use std::ops::Range;
use std::sync::OnceLock;

/// Strain and stress at the integration points of every element (the points of each element's stiffness rule).
#[derive(Debug, Clone, PartialEq)]
pub struct IntegrationPointField {
    offsets: Vec<usize>,
    positions: Vec<Vec3>,
    weights: Vec<f64>,
    strain: Vec<[f64; 6]>,
    stress: Vec<[f64; 6]>,
    thermal: Option<Vec<[f64; 6]>>,
}

impl IntegrationPointField {
    /// The field of a displacement vector (one value per unknown of `model.dof_map()`). Refuses a displacement of the
    /// wrong length and an element without material; an element error is returned as [`FemError::Element`].
    pub fn compute(model: &SolidModel, displacement: &[f64]) -> Result<IntegrationPointField> {
        let dofs = model.dof_map();
        if displacement.len() != dofs.n_equations() {
            return Err(FemError::FieldLength {
                key: "displacement",
                expected: dofs.n_equations(),
                found: displacement.len(),
            });
        }
        let materials = model.material_indices()?;
        let mesh = model.mesh();
        let ne = mesh.n_elements();
        let mut field = IntegrationPointField {
            offsets: Vec::with_capacity(ne + 1),
            positions: Vec::new(),
            weights: Vec::new(),
            strain: Vec::new(),
            stress: Vec::new(),
            thermal: None,
        };
        field.offsets.push(0);
        let context = PointContext::default();
        let temperature = thermal::temperature_field(model)?;
        let delta_t: Option<Vec<f64>> =
            temperature.map(|(nodal, reference)| nodal.iter().map(|t| t - reference).collect());
        let mut thermal_strains: Vec<[f64; 6]> = Vec::new();
        // ADR 0452 D5: the per-point data, only for a model with an orthotropic or tabulated material.
        let points = if crate::mat_ortho::uses_point_materials(model) {
            Some(crate::mat_ortho::MaterialPoints::new(model)?)
        } else {
            None
        };
        for e in 0..ne {
            let located = |source: FemError| FemError::Element {
                element: e,
                source: Box::new(source),
            };
            let element = element_for(mesh.element_kind(e));
            let nodes = mesh.element_nodes(e);
            let coords: Vec<Vec3> = nodes.iter().map(|&n| mesh.nodes()[n]).collect();
            let ue: Vec<f64> = element_dofs(&dofs, nodes).iter().map(|&q| displacement[q]).collect();
            let material = &model.materials()[materials[e]].1;
            let rule = element.stiffness_rule();
            let dte: Option<Vec<f64>> = delta_t.as_ref().map(|d| nodes.iter().map(|&n| d[n]).collect());
            let point_material = points
                .as_ref()
                .filter(|_| crate::mat_ortho::is_point_material(material));
            for ip in element.integration_points(&coords, rule).map_err(located)? {
                let eps = element.strain_at(&coords, &ue, ip.geometry.xi).map_err(located)?;
                if let Some(points) = point_material {
                    // ADR 0452 D5: the context and the thermal strain of the point.
                    let (point, dt) = points
                        .context(e, material, ip.geometry.x, &ip.geometry.n)
                        .map_err(located)?;
                    let sigma = match (&dte, points.reference()) {
                        (Some(_), Some(reference)) => {
                            let eth = crate::mat_ortho::thermal_strain(material, &point, reference, dt).map_err(located)?;
                            let elastic: [f64; 6] = std::array::from_fn(|k| eps[k] - eth[k]);
                            thermal_strains.push(eth);
                            material.stress(&elastic, &point, &[]).map_err(located)?.0
                        }
                        _ => material.stress(&eps, &point, &[]).map_err(located)?.0,
                    };
                    field.positions.push(ip.geometry.x);
                    field.weights.push(ip.weight);
                    field.strain.push(eps);
                    field.stress.push(sigma);
                    continue;
                }
                let (sigma, _) = match &dte {
                    None => material.stress(&eps, &context, &[]).map_err(located)?,
                    Some(dte) => {
                        let dt = thermal::point_temperature_change(&ip.geometry.n, dte);
                        let eth = thermal::thermal_strain(material, dt);
                        let elastic: [f64; 6] = std::array::from_fn(|k| eps[k] - eth[k]);
                        thermal_strains.push(eth);
                        material.stress(&elastic, &context, &[]).map_err(located)?
                    }
                };
                field.positions.push(ip.geometry.x);
                field.weights.push(ip.weight);
                field.strain.push(eps);
                field.stress.push(sigma);
            }
            field.offsets.push(field.weights.len());
        }
        if delta_t.is_some() {
            field.thermal = Some(thermal_strains);
        }
        Ok(field)
    }

    /// The field of a result's displacement.
    pub fn from_result(model: &SolidModel, result: &SolidResult) -> Result<IntegrationPointField> {
        IntegrationPointField::compute(model, result.displacement_values())
    }

    /// `offsets[e]..offsets[e + 1]` are the points of element `e`.
    pub fn offsets(&self) -> &[usize] {
        &self.offsets
    }
    /// Physical position of every point.
    pub fn positions(&self) -> &[Vec3] {
        &self.positions
    }
    /// Weight `w det J` of every point (physical volume).
    pub fn weights(&self) -> &[f64] {
        &self.weights
    }
    /// Strain at every point (engineering shear).
    pub fn strain(&self) -> &[[f64; 6]] {
        &self.strain
    }
    /// Stress at every point (`D (eps - eps_th)` when the model has a temperature field).
    pub fn stress(&self) -> &[[f64; 6]] {
        &self.stress
    }
    /// Thermal strain at every point (ADR 0281), `None` when the model has no temperature field.
    pub fn thermal_strain(&self) -> Option<&[[f64; 6]]> {
        self.thermal.as_deref()
    }
    /// Number of points.
    pub fn n_points(&self) -> usize {
        self.weights.len()
    }
    /// Number of elements.
    pub fn n_elements(&self) -> usize {
        self.offsets.len() - 1
    }
    /// The points of an element.
    pub fn points_of(&self, element: usize) -> Range<usize> {
        self.offsets[element]..self.offsets[element + 1]
    }
    /// Volume of an element by its rule, `sum w det J`.
    pub fn element_volume(&self, element: usize) -> f64 {
        self.points_of(element).map(|q| self.weights[q]).sum()
    }

    fn check_values(&self, values: &[[f64; 6]]) -> Result<()> {
        if values.len() != self.n_points() {
            return Err(FemError::FieldLength {
                key: "values",
                expected: self.n_points(),
                found: values.len(),
            });
        }
        Ok(())
    }

    /// The volume mean `sum w det J v / sum w det J` of per-point values over each element.
    pub fn element_means(&self, values: &[[f64; 6]]) -> Result<Vec<[f64; 6]>> {
        self.check_values(values)?;
        Ok((0..self.n_elements())
            .map(|e| {
                let mut s = [0.0; 6];
                let mut v = 0.0;
                for q in self.points_of(e) {
                    let w = self.weights[q];
                    v += w;
                    for (sc, x) in s.iter_mut().zip(&values[q]) {
                        *sc += w * x;
                    }
                }
                s.map(|x| x / v)
            })
            .collect())
    }

    /// Strain energy of each element, `sum w det J eps . sigma / 2`; with a temperature field the elastic energy `sum w det
    /// J (eps - eps_th) . sigma / 2` (ADR 0281 D3).
    pub fn element_strain_energy(&self) -> Vec<f64> {
        match &self.thermal {
            None => (0..self.n_elements())
                .map(|e| {
                    self.points_of(e)
                        .map(|q| self.weights[q] * energy_density(&self.strain[q], &self.stress[q]))
                        .sum()
                })
                .collect(),
            Some(th) => (0..self.n_elements())
                .map(|e| {
                    self.points_of(e)
                        .map(|q| {
                            let elastic: [f64; 6] = std::array::from_fn(|k| self.strain[q][k] - th[q][k]);
                            self.weights[q] * energy_density(&elastic, &self.stress[q])
                        })
                        .sum()
                })
                .collect(),
        }
    }

    /// Strain energy of the model, the sum of the element energies in element order.
    pub fn strain_energy(&self) -> f64 {
        self.element_strain_energy().iter().sum()
    }
}

/// Strain energy density `eps . sigma / 2` of Voigt vectors with engineering shear strain.
fn energy_density(eps: &[f64; 6], sigma: &[f64; 6]) -> f64 {
    let normal = eps[0] * sigma[0] + eps[1] * sigma[1] + eps[2] * sigma[2];
    let shear = eps[3] * sigma[3] + eps[4] * sigma[4] + eps[5] * sigma[5];
    0.5 * (normal + shear)
}

/// The result with the field's strain and stress at the integration points (bit for bit).
pub fn attach(result: SolidResult, field: &IntegrationPointField) -> Result<SolidResult> {
    result.with_integration_points(field.offsets.clone(), field.strain.clone(), field.stress.clone())
}

// ------------------------------------------------------------------------------------------------------------ nodal values

/// The fitted polynomial space of a kind, as many functions as its stiffness rule has points (ADR 0277 arithmetic 10).
fn fit_basis(kind: ElementKind, xi: [f64; 3]) -> Vec<f64> {
    let [x, y, z] = xi;
    match kind {
        ElementKind::Tet4 => vec![1.0],
        ElementKind::Tet10 => vec![1.0, x, y, z],
        ElementKind::Hex8 => vec![1.0, x, y, z, x * y, y * z, z * x, x * y * z],
        ElementKind::Hex20 => crate::hex20::fit_basis(xi),
        ElementKind::Beam2 | ElementKind::Shell4 | ElementKind::Shell3 => Vec::new(),
    }
}

/// Inverse of a small dense matrix by Gauss-Jordan elimination with partial pivoting.
fn invert(mut a: Vec<Vec<f64>>) -> Vec<Vec<f64>> {
    let n = a.len();
    let mut inv: Vec<Vec<f64>> = (0..n)
        .map(|i| (0..n).map(|j| f64::from(u8::from(i == j))).collect())
        .collect();
    for col in 0..n {
        let mut piv = col;
        for r in col + 1..n {
            if a[r][col].abs() > a[piv][col].abs() {
                piv = r;
            }
        }
        assert!(a[piv][col] != 0.0, "fit basis not unisolvent on the rule's points");
        a.swap(col, piv);
        inv.swap(col, piv);
        let d = a[col][col];
        for j in 0..n {
            a[col][j] /= d;
            inv[col][j] /= d;
        }
        for r in 0..n {
            if r != col {
                let f = a[r][col];
                if f != 0.0 {
                    for j in 0..n {
                        a[r][j] -= f * a[col][j];
                        inv[r][j] -= f * inv[col][j];
                    }
                }
            }
        }
    }
    inv
}

fn build_extrapolation(kind: ElementKind) -> Vec<Vec<f64>> {
    let points = element_for(kind).stiffness_rule().points();
    let v: Vec<Vec<f64>> = points.iter().map(|&p| fit_basis(kind, p)).collect();
    let vinv = invert(v);
    let n = points.len();
    kind.reference_nodes()
        .iter()
        .map(|&node| {
            let p = fit_basis(kind, node);
            (0..n).map(|q| (0..n).map(|j| p[j] * vinv[j][q]).sum()).collect()
        })
        .collect()
}

/// The matrix `E = P(nodes) V^{-1}` that takes the values at the points of a kind's stiffness rule to its nodes: row `a`
/// gives node `a` (the kind's node order), column `q` point `q` (the rule's order).
pub fn extrapolation_matrix(kind: ElementKind) -> &'static [Vec<f64>] {
    static TET4: OnceLock<Vec<Vec<f64>>> = OnceLock::new();
    static HEX8: OnceLock<Vec<Vec<f64>>> = OnceLock::new();
    static TET10: OnceLock<Vec<Vec<f64>>> = OnceLock::new();
    static HEX20: OnceLock<Vec<Vec<f64>>> = OnceLock::new();
    // A kind whose package has not landed has no rule to extrapolate from; a structural kind has no solid stress (ADR 0291).
    if kind.pending().is_some() {
        return &[];
    }
    let cell = match kind {
        ElementKind::Tet4 => &TET4,
        ElementKind::Hex8 => &HEX8,
        ElementKind::Tet10 => &TET10,
        ElementKind::Hex20 => &HEX20,
        ElementKind::Beam2 | ElementKind::Shell4 | ElementKind::Shell3 => return &[],
    };
    cell.get_or_init(|| build_extrapolation(kind))
}

fn check_field(model: &SolidModel, field: &IntegrationPointField, values: &[[f64; 6]]) -> Result<()> {
    let ne = model.mesh().n_elements();
    if field.n_elements() != ne {
        return Err(FemError::FieldLength {
            key: "field elements",
            expected: ne,
            found: field.n_elements(),
        });
    }
    field.check_values(values)?;
    for e in 0..ne {
        let kind = model.mesh().element_kind(e);
        let expected = element_for(kind).stiffness_rule().len();
        let found = field.points_of(e).len();
        if found != expected {
            return Err(FemError::Element {
                element: e,
                source: Box::new(FemError::FieldLength {
                    key: "integration points",
                    expected,
                    found,
                }),
            });
        }
    }
    Ok(())
}

fn extrapolate(kind: ElementKind, values: &[[f64; 6]]) -> Vec<[f64; 6]> {
    extrapolation_matrix(kind)
        .iter()
        .map(|row| {
            let mut s = [0.0; 6];
            for (w, v) in row.iter().zip(values) {
                for (sc, x) in s.iter_mut().zip(v) {
                    *sc += w * x;
                }
            }
            s
        })
        .collect()
}

/// Per-point values extrapolated to the nodes of each element: `out[e][a]` is the value at local node `a` of element `e`.
pub fn element_nodal_values(
    model: &SolidModel,
    field: &IntegrationPointField,
    values: &[[f64; 6]],
) -> Result<Vec<Vec<[f64; 6]>>> {
    check_field(model, field, values)?;
    Ok((0..field.n_elements())
        .map(|e| extrapolate(model.mesh().element_kind(e), &values[field.points_of(e)]))
        .collect())
}

/// Nodal averages of per-point values over the elements of `region` (an element set; `None`: every element), each
/// element's extrapolated value weighted by its volume. A node of no element of the region has `None`. Refuses an unknown
/// region and a node shared by elements of the region whose materials differ (a material interface: average over one
/// element set at a time).
pub fn nodal_average(
    model: &SolidModel,
    field: &IntegrationPointField,
    values: &[[f64; 6]],
    region: Option<&str>,
) -> Result<Vec<Option<[f64; 6]>>> {
    check_field(model, field, values)?;
    let mesh = model.mesh();
    let elements: Vec<usize> = match region {
        None => (0..mesh.n_elements()).collect(),
        Some(name) => {
            let mut v = mesh
                .element_set(name)
                .ok_or_else(|| FemError::UnknownSet {
                    kind: SetKind::Element,
                    name: name.to_string(),
                })?
                .to_vec();
            v.sort_unstable();
            v
        }
    };
    let materials = model.material_indices()?;
    let n = mesh.nodes().len();
    let mut first: Vec<Option<usize>> = vec![None; n];
    let mut sum = vec![[0.0; 6]; n];
    let mut weight = vec![0.0; n];
    for &e in &elements {
        let m = materials[e];
        for &node in mesh.element_nodes(e) {
            match first[node] {
                None => first[node] = Some(m),
                Some(f) if model.materials()[f].1 != model.materials()[m].1 => {
                    return Err(FemError::InvalidValue {
                        key: "nodal average",
                        reason: format!(
                            "node {node} joins elements of the material sets '{}' and '{}' with different materials; \
                             average over one element set at a time",
                            model.materials()[f].0,
                            model.materials()[m].0
                        ),
                    });
                }
                Some(_) => {}
            }
        }
        let w = field.element_volume(e);
        let local = extrapolate(mesh.element_kind(e), &values[field.points_of(e)]);
        for (&node, v) in mesh.element_nodes(e).iter().zip(&local) {
            weight[node] += w;
            for (sc, x) in sum[node].iter_mut().zip(v) {
                *sc += w * x;
            }
        }
    }
    Ok(sum
        .into_iter()
        .zip(weight)
        .map(|(s, w)| (w > 0.0).then(|| s.map(|x| x / w)))
        .collect())
}

// -------------------------------------------------------------------------------------------------------------- invariants

/// The Frobenius norm of the tensor of a Voigt stress, `sqrt(s_xx^2 + s_yy^2 + s_zz^2 + 2 (s_xy^2 + s_yz^2 + s_zx^2))`.
pub fn tensor_norm(s: &[f64; 6]) -> f64 {
    (s[0] * s[0] + s[1] * s[1] + s[2] * s[2] + 2.0 * (s[3] * s[3] + s[4] * s[4] + s[5] * s[5])).sqrt()
}

/// The von Mises stress `sqrt(((s_xx - s_yy)^2 + (s_yy - s_zz)^2 + (s_zz - s_xx)^2) / 2 + 3 (s_xy^2 + s_yz^2 + s_zx^2))`.
pub fn von_mises(s: &[f64; 6]) -> f64 {
    let (a, b, c) = (s[0] - s[1], s[1] - s[2], s[2] - s[0]);
    (0.5 * (a * a + b * b + c * c) + 3.0 * (s[3] * s[3] + s[4] * s[4] + s[5] * s[5])).sqrt()
}

/// Principal values, descending (`values[0]` the largest), with their unit directions: `directions[k]` belongs to
/// `values[k]`; the directions are orthonormal, each with its entry of largest modulus positive.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Principal {
    /// Principal values, descending.
    pub values: [f64; 3],
    /// Principal directions.
    pub directions: [Vec3; 3],
}

const AXES: [Vec3; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// The scaled, shifted form of a Voigt tensor: `m` (largest modulus), `q` (mean of the scaled diagonal), `p` and the
/// trace-free `C = (S/m - q I) / p` as a 3 x 3 matrix (ADR 0277 arithmetic 1). `None` for the zero tensor or `p = 0`.
struct Shifted {
    m: f64,
    q: f64,
    p: f64,
    c: [[f64; 3]; 3],
}

enum Form {
    Zero,
    NonFinite,
    Isotropic(f64),
    General(Shifted),
}

fn shifted(s: &[f64; 6]) -> Form {
    if s.iter().any(|v| !v.is_finite()) {
        return Form::NonFinite;
    }
    let m = s.iter().fold(0.0f64, |a, v| a.max(v.abs()));
    if m == 0.0 {
        return Form::Zero;
    }
    let a = s.map(|v| v / m);
    let q = (a[0] + a[1] + a[2]) / 3.0;
    let b = [[a[0] - q, a[3], a[5]], [a[3], a[1] - q, a[4]], [a[5], a[4], a[2] - q]];
    let p2 = (b[0][0] * b[0][0]
        + b[1][1] * b[1][1]
        + b[2][2] * b[2][2]
        + 2.0 * (b[0][1] * b[0][1] + b[1][2] * b[1][2] + b[0][2] * b[0][2]))
        / 6.0;
    let p = p2.sqrt();
    if p == 0.0 {
        return Form::Isotropic(m * q);
    }
    let c = b.map(|row| row.map(|v| v / p));
    Form::General(Shifted { m, q, p, c })
}

fn det3(c: &[[f64; 3]; 3]) -> f64 {
    c[0][0] * (c[1][1] * c[2][2] - c[1][2] * c[2][1]) - c[0][1] * (c[1][0] * c[2][2] - c[1][2] * c[2][0])
        + c[0][2] * (c[1][0] * c[2][1] - c[1][1] * c[2][0])
}

/// The roots `b1 >= b2 >= b3` of `l^3 - 3 l - det C` by the triple-angle identity (ADR 0277 arithmetic 1).
fn trigonometric_roots(c: &[[f64; 3]; 3]) -> [f64; 3] {
    let r = (det3(c) / 2.0).clamp(-1.0, 1.0);
    let phi = r.acos() / 3.0;
    let b1 = 2.0 * phi.cos();
    let b3 = 2.0 * (phi + 2.0 * std::f64::consts::FRAC_PI_3).cos();
    [b1, -b1 - b3, b3]
}

/// Principal values by the bare closed trigonometric form (Smith 1961), descending. Loses accuracy near a repeated root
/// (ADR 0277 arithmetic 2); [`principal`] is the accurate routine.
pub fn principal_values_trigonometric(s: &[f64; 6]) -> [f64; 3] {
    match shifted(s) {
        Form::NonFinite => [f64::NAN; 3],
        Form::Zero => [0.0; 3],
        Form::Isotropic(v) => [v; 3],
        Form::General(t) => trigonometric_roots(&t.c).map(|b| t.m * (t.q + t.p * b)),
    }
}

fn matvec(c: &[[f64; 3]; 3], v: Vec3) -> Vec3 {
    c.map(|row| dot(row, v))
}

fn unit(v: Vec3) -> Vec3 {
    let n = norm(v);
    v.map(|x| x / n)
}

fn sign_normalised(v: Vec3) -> Vec3 {
    let mut imax = 0;
    for k in 1..3 {
        if v[k].abs() > v[imax].abs() {
            imax = k;
        }
    }
    if v[imax] < 0.0 {
        v.map(|x| -x)
    } else {
        v
    }
}

/// Principal values (descending) and orthonormal directions of a Voigt stress (ADR 0277 D4): the closed trigonometric
/// form gives the root farthest from the middle one, its direction comes from the largest cross product of two rows of
/// `C - b I`, and the other two roots and directions from one exact Jacobi rotation of the 2 x 2 restriction to the plane
/// orthogonal to it, so the result is accurate and orthonormal also for repeated or close roots. The zero tensor and an
/// isotropic one give the coordinate axes; a non-finite entry gives NaN values and directions.
pub fn principal(s: &[f64; 6]) -> Principal {
    let t = match shifted(s) {
        Form::NonFinite => {
            return Principal {
                values: [f64::NAN; 3],
                directions: [[f64::NAN; 3]; 3],
            }
        }
        Form::Zero => {
            return Principal {
                values: [0.0; 3],
                directions: AXES,
            }
        }
        Form::Isotropic(v) => {
            return Principal {
                values: [v; 3],
                directions: AXES,
            }
        }
        Form::General(t) => t,
    };
    let c = &t.c;
    let [b1, b2, b3] = trigonometric_roots(c);
    let far = if b1 - b2 >= b2 - b3 { b1 } else { b3 };
    let rows = [
        [c[0][0] - far, c[0][1], c[0][2]],
        [c[1][0], c[1][1] - far, c[1][2]],
        [c[2][0], c[2][1], c[2][2] - far],
    ];
    let candidates = [
        cross(rows[0], rows[1]),
        cross(rows[0], rows[2]),
        cross(rows[1], rows[2]),
    ];
    let mut best = candidates[0];
    let mut best_norm = norm(best);
    for cand in &candidates[1..] {
        let nc = norm(*cand);
        if nc > best_norm {
            best = *cand;
            best_norm = nc;
        }
    }
    let e = if best_norm > 0.0 {
        best.map(|x| x / best_norm)
    } else {
        AXES[0]
    };
    // The coordinate axis of smallest |e_k| (first such) for the frame of the orthogonal plane.
    let mut k = 0;
    for j in 1..3 {
        if e[j].abs() < e[k].abs() {
            k = j;
        }
    }
    let u = unit(cross(e, AXES[k]));
    let v = cross(e, u);
    let (cu, cv) = (matvec(c, u), matvec(c, v));
    let m11 = dot(u, cu);
    let m22 = dot(v, cv);
    let m12 = 0.5 * (dot(u, cv) + dot(v, cu));
    let (cs, sn, tt) = if m12 == 0.0 {
        (1.0, 0.0, 0.0)
    } else {
        let tau = (m22 - m11) / (2.0 * m12);
        let tt = if tau == 0.0 {
            1.0
        } else {
            tau.signum() / (tau.abs() + (1.0 + tau * tau).sqrt())
        };
        let cs = 1.0 / (1.0 + tt * tt).sqrt();
        (cs, tt * cs, tt)
    };
    let wp: Vec3 = [0, 1, 2].map(|i| cs * u[i] - sn * v[i]);
    let wq: Vec3 = [0, 1, 2].map(|i| sn * u[i] + cs * v[i]);
    let mut pairs = [(dot(e, matvec(c, e)), e), (m11 - tt * m12, wp), (m22 + tt * m12, wq)];
    pairs.sort_by(|a, b| b.0.total_cmp(&a.0));
    Principal {
        values: pairs.map(|(mu, _)| t.m * (t.q + t.p * mu)),
        directions: pairs.map(|(_, d)| sign_normalised(d)),
    }
}
