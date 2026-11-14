//! Direct harmonic solve: loss factors per material, Rayleigh and discrete viscous dampers, base motion (F-412, ADR 0448;
//! declared by ADR 0440, design stub ADR 0315).
//!
//! The steady response `x(t) = Re(X e^{i omega t})` (the convention of [`crate::harmonic`], ADR 0391) of the model reduced
//! by its zero prescribed displacements, from one complex factorisation per frequency (ADR 0448 D2):
//!
//! `A(omega) = sum_m s_m (1 + i eta_m) K_m - omega^2 M + i omega (alpha M + beta K + C_d)`,
//! `A_FF X_F = F_F - A_FP G_P`,
//!
//! with `K_m` the stiffness of the elements of material `m` (`K = sum_m K_m`), `M` the consistent mass, `eta_m` and `s_m` from
//! the material's [`LossFactor`] (a constant loss factor, `s = 1`, or a [`ModulusTable`] of storage modulus and loss factor
//! against frequency, `s = E'(f)/E`), Rayleigh damping `alpha M + beta K` (the meaning of
//! [`crate::harmonic::ModalDamping::Rayleigh`]), `C_d` the [`Dashpot`]s, and `G_P` the [`BaseMotion`] at prescribed equations
//! (zero elsewhere). `A_FF` is factored by [`ComplexSkylineLdlt`] in reverse Cuthill-McKee order. When `A_FF` is real (no
//! damping of any kind) a relative pivot `<= RESONANCE_PIVOT` is refused as a resonance without damping, with the frequency.
//! The assembly takes the thread count; the factor and the solve are sequential, so the result is bit-identical for any.
//!
//! ADR 0491 (F-577): a [`Material::Viscoelastic`] material is taken directly, by its complex moduli at each frequency
//! ([`crate::mat_visco::complex_moduli`]). With equal shear and bulk weights in every term it is one part, the base's
//! stiffness times `E*(omega)/E`; otherwise two parts, the stiffness of `2 G_0 I_dev` times `G*(omega)/G_0` and that of
//! `K_0 1 (x) 1` times `K*(omega)/K_0` (the stiffness is linear in the elasticity matrix, so this is exact). A shifted
//! material takes [`DirectHarmonicOptions::temperature`]. The Rayleigh term `beta K` takes its instantaneous base
//! stiffness; a loss factor on its set and the harmonic stress are refused. A model without the material takes the code of
//! ADR 0448 as it was.

use crate::assemble::{assemble_with, stiffness_with};
use crate::constraint::prescribed_equations;
use crate::element::element_for;
use crate::error::{FemError, Result};
use crate::harmonic::{HarmonicLoad, HarmonicResponse, HarmonicStress};
use crate::mass::consistent_with;
use crate::material::{isotropic_elasticity, Matrix6};
use crate::model::{DofMap, Material, SolidModel};
use crate::recover::IntegrationPointField;
use rcfd_linalg::complex::C64;
use rcfd_linalg::cskyline::{ComplexCsr, ComplexSkylineError, ComplexSkylineLdlt, ComplexSkylineOptions};
use rcfd_linalg::CsrMatrix;
use std::collections::BTreeMap;
use std::f64::consts::PI;

/// The work package that fills this module; `None` once it has landed.
pub const PENDING: Option<&'static str> = None;

/// Without damping, a pivot with `|d| <= RESONANCE_PIVOT s` (`s` the scale of the pivot test of
/// [`rcfd_linalg::cskyline`]) is refused as a resonance (ADR 0448 D2, estimate from the 1-D chain).
pub const RESONANCE_PIVOT: f64 = 1e-8;

fn invalid(key: &'static str, reason: String) -> FemError {
    FemError::InvalidValue { key, reason }
}

// ------------------------------------------------------------------------------------------------------ damping data

/// Storage modulus `E'(f)` and loss factor `eta(f)` against frequency `f` in Hz, linear between points, not extrapolated.
#[derive(Debug, Clone, PartialEq)]
pub struct ModulusTable {
    frequencies: Vec<f64>,
    storage: Vec<f64>,
    loss: Vec<f64>,
}

impl ModulusTable {
    /// A table from its columns: at least two points, frequencies finite, `>= 0` and strictly ascending, storage moduli
    /// finite and positive, loss factors finite and `>= 0`.
    pub fn new(frequencies: Vec<f64>, storage: Vec<f64>, loss: Vec<f64>) -> Result<ModulusTable> {
        let bad = |reason: String| Err(invalid("modulus table", reason));
        if frequencies.len() < 2 || storage.len() != frequencies.len() || loss.len() != frequencies.len() {
            return bad(format!(
                "{} frequencies, {} storage moduli, {} loss factors (at least two points, equal lengths)",
                frequencies.len(),
                storage.len(),
                loss.len()
            ));
        }
        for (k, &f) in frequencies.iter().enumerate() {
            if !(f.is_finite() && f >= 0.0) {
                return bad(format!("frequency {f:e} Hz at point {k} (must be finite and >= 0)"));
            }
            if k > 0 && f <= frequencies[k - 1] {
                return bad(format!(
                    "frequency {f:e} Hz at point {k} is not above {:e} Hz (strictly ascending needed)",
                    frequencies[k - 1]
                ));
            }
        }
        if let Some(k) = storage.iter().position(|&e| !(e.is_finite() && e > 0.0)) {
            return bad(format!(
                "storage modulus {:e} at point {k} (must be finite and > 0)",
                storage[k]
            ));
        }
        if let Some(k) = loss.iter().position(|&e| !(e.is_finite() && e >= 0.0)) {
            return bad(format!(
                "loss factor {:e} at point {k} (must be finite and >= 0)",
                loss[k]
            ));
        }
        Ok(ModulusTable {
            frequencies,
            storage,
            loss,
        })
    }

    /// `(E'(f), eta(f))` by linear interpolation; a frequency outside the table is refused.
    pub fn at(&self, f: f64) -> Result<(f64, f64)> {
        let n = self.frequencies.len();
        let (lo, hi) = (self.frequencies[0], self.frequencies[n - 1]);
        if !(f >= lo && f <= hi) {
            return Err(invalid(
                "modulus table",
                format!("frequency {f:e} Hz outside the table [{lo:e}, {hi:e}] Hz (no extrapolation)"),
            ));
        }
        let k = (0..n - 1)
            .find(|&k| f <= self.frequencies[k + 1])
            .expect("inside the table");
        let t = (f - self.frequencies[k]) / (self.frequencies[k + 1] - self.frequencies[k]);
        let lerp = |v: &[f64]| v[k] + t * (v[k + 1] - v[k]);
        Ok((lerp(&self.storage), lerp(&self.loss)))
    }
}

/// The structural (hysteretic) damping of one material: its stiffness enters as `s (1 + i eta) K_m`.
#[derive(Debug, Clone, PartialEq)]
pub enum LossFactor {
    /// A constant loss factor `eta` (`s = 1`); any material whose stiffness assembles.
    Constant(f64),
    /// Storage modulus and loss factor against frequency, `s = E'(f)/E`; an isotropic elastic material only.
    Table(ModulusTable),
}

/// A viscous dashpot along one component: to the ground, or between two nodes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dashpot {
    node: usize,
    other: Option<usize>,
    component: usize,
    c: f64,
}

impl Dashpot {
    /// A dashpot `c` (N s/m) from `node`, `component`, to the ground: `c` on the diagonal.
    pub fn ground(node: usize, component: usize, c: f64) -> Dashpot {
        Dashpot {
            node,
            other: None,
            component,
            c,
        }
    }

    /// A dashpot `c` between `a` and `b` along `component`: `c [1 -1; -1 1]`.
    pub fn between(a: usize, b: usize, component: usize, c: f64) -> Dashpot {
        Dashpot {
            node: a,
            other: Some(b),
            component,
            c,
        }
    }

    /// `(node, other, component, c)`, for the damped modes of ADR 0450 (F-414).
    pub(crate) fn parts(&self) -> (usize, Option<usize>, usize, f64) {
        (self.node, self.other, self.component, self.c)
    }
}

/// Harmonic displacements `G` of prescribed equations (the base motion), by node and component.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BaseMotion {
    entries: Vec<(usize, usize, C64)>,
}

impl BaseMotion {
    /// No base motion.
    pub fn new() -> BaseMotion {
        BaseMotion::default()
    }

    /// The amplitude of `node`, `component`.
    pub fn add(&mut self, node: usize, component: usize, amplitude: C64) {
        self.entries.push((node, component, amplitude));
    }

    /// The same amplitude on every node of a node set of the model's mesh.
    pub fn add_set(&mut self, model: &SolidModel, set: &str, component: usize, amplitude: C64) -> Result<()> {
        let nodes = model
            .mesh()
            .node_set(set)
            .ok_or_else(|| invalid("base motion", format!("no node set '{set}'")))?;
        for &a in nodes {
            self.add(a, component, amplitude);
        }
        Ok(())
    }

    /// The entries `(node, component, amplitude)` in the order added.
    pub fn entries(&self) -> &[(usize, usize, C64)] {
        &self.entries
    }
}

/// Options of a [`DirectHarmonic`] solver.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct DirectHarmonicOptions {
    /// Loss factors by the element set of a material of the model (materials not listed: none).
    pub loss_factors: Vec<(String, LossFactor)>,
    /// Rayleigh damping `(alpha, beta)`: `C = alpha M + beta K` (default none).
    pub rayleigh: Option<(f64, f64)>,
    /// Discrete dashpots.
    pub dashpots: Vec<Dashpot>,
    /// Worker threads of the assembly (`None`: the available parallelism); the result does not depend on it.
    pub threads: Option<usize>,
    /// The relative pivot below which an undamped solve is refused as a resonance (default [`RESONANCE_PIVOT`]).
    pub resonance_pivot: f64,
    /// The temperature, K, of a viscoelastic material with a time-temperature shift (ADR 0491; default none). Read only
    /// by a viscoelastic material.
    pub temperature: Option<f64>,
}

impl Default for DirectHarmonicOptions {
    fn default() -> Self {
        DirectHarmonicOptions {
            loss_factors: Vec::new(),
            rayleigh: None,
            dashpots: Vec::new(),
            threads: None,
            resonance_pivot: RESONANCE_PIVOT,
            temperature: None,
        }
    }
}

impl DirectHarmonicOptions {
    /// No damping, default threads.
    pub fn new() -> DirectHarmonicOptions {
        DirectHarmonicOptions::default()
    }

    /// The loss factor of the material on element set `set`.
    pub fn loss_factor(mut self, set: &str, loss: LossFactor) -> DirectHarmonicOptions {
        self.loss_factors.push((set.to_string(), loss));
        self
    }

    /// Rayleigh damping `alpha M + beta K`.
    pub fn rayleigh(mut self, alpha: f64, beta: f64) -> DirectHarmonicOptions {
        self.rayleigh = Some((alpha, beta));
        self
    }

    /// A dashpot.
    pub fn dashpot(mut self, d: Dashpot) -> DirectHarmonicOptions {
        self.dashpots.push(d);
        self
    }

    /// The assembly threads.
    pub fn threads(mut self, threads: usize) -> DirectHarmonicOptions {
        self.threads = Some(threads);
        self
    }

    /// The resonance threshold of an undamped solve.
    pub fn resonance_pivot(mut self, tau: f64) -> DirectHarmonicOptions {
        self.resonance_pivot = tau;
        self
    }

    /// The temperature of a shifted viscoelastic material (ADR 0491).
    pub fn temperature(mut self, temperature: f64) -> DirectHarmonicOptions {
        self.temperature = Some(temperature);
        self
    }
}

// ----------------------------------------------------------------------------------------------------------- solver

/// One stored entry of a free row: the full column, its position in the mesh pattern (if there) and the dashpot sum.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Entry {
    col: usize,
    p: Option<usize>,
    cd: f64,
}

/// The direct harmonic solver of a model: the matrices assembled once, one complex factorisation per frequency.
#[derive(Debug, Clone)]
pub struct DirectHarmonic {
    dofs: DofMap,
    n: usize,
    /// Per material: the values of `K_m` on the mesh pattern and its damping.
    k_parts: Vec<(Vec<f64>, Damping)>,
    k_total: Vec<f64>,
    mass: Vec<f64>,
    rayleigh: (f64, f64),
    free: Vec<usize>,
    reduced: Vec<usize>,
    is_fixed: Vec<bool>,
    /// For each free row: the entries at free columns and at prescribed columns.
    rows_free: Vec<Vec<Entry>>,
    rows_fixed: Vec<Vec<Entry>>,
    resonance_pivot: f64,
}

#[derive(Debug, Clone, PartialEq)]
enum Damping {
    None,
    Constant(f64),
    Table(ModulusTable, f64),
    /// A part of a viscoelastic material (ADR 0491 D1): the coefficient is its complex modulus over `modulus`.
    Viscoelastic {
        material: Box<Material>,
        part: ViscoPart,
        temperature: Option<f64>,
        modulus: f64,
    },
}

/// The part of a viscoelastic material's stiffness (ADR 0491 D1).
#[derive(Debug, Clone, Copy, PartialEq)]
enum ViscoPart {
    /// Equal weights: the whole base stiffness, `E*/E`.
    Whole,
    /// The stiffness of `2 G_0 I_dev`, `G*/G_0`.
    Shear,
    /// The stiffness of `K_0 1 (x) 1`, `K*/K_0`.
    Bulk,
}

/// `2 G_0 I_dev` in Voigt form with engineering shear strain.
fn shear_matrix(g0: f64) -> Matrix6 {
    let mut d = [[0.0; 6]; 6];
    for (i, row) in d.iter_mut().enumerate().take(3) {
        for (j, v) in row.iter_mut().enumerate().take(3) {
            *v = 2.0 * g0 * (if i == j { 1.0 } else { 0.0 } - 1.0 / 3.0);
        }
    }
    for (k, row) in d.iter_mut().enumerate().skip(3) {
        row[k] = g0;
    }
    d
}

/// `K_0 1 (x) 1` in Voigt form.
fn bulk_matrix(k0: f64) -> Matrix6 {
    let mut d = [[0.0; 6]; 6];
    for row in d.iter_mut().take(3) {
        for v in row.iter_mut().take(3) {
            *v = k0;
        }
    }
    d
}

fn is_viscoelastic(m: &Material) -> bool {
    matches!(m, Material::Viscoelastic { .. })
}

/// The model check of the viscoelastic route (ADR 0491 D1): the element checks with the history flag (which lets the
/// viscoelastic material through) and the supports, then J2 refused as [`SolidModel::check_supported`] refuses it.
fn check_viscoelastic_model(model: &SolidModel) -> Result<()> {
    model.check_supported_nonlinear()?;
    for (i, (set, m)) in model.materials().iter().enumerate() {
        if matches!(m, Material::J2 { .. }) {
            return Err(FemError::NotYetSupported {
                item: "material",
                variant: m.name(),
                location: format!("material {i}, element set '{set}'"),
                package: crate::mat_j2::FOLLOW_UP,
            });
        }
    }
    Ok(())
}

/// The values of each stiffness part on the mesh pattern and its damping.
type StiffnessParts = Vec<(Vec<f64>, Damping)>;

/// The stiffness parts of a model with a viscoelastic material (ADR 0491 D1): per material one part (two for a
/// viscoelastic material with unequal weights), each assembled over the elements of its material, and the sum of the parts
/// (the total stiffness on the mesh pattern).
fn viscoelastic_parts(
    model: &SolidModel,
    damping: &[Damping],
    options: &DirectHarmonicOptions,
) -> Result<(CsrMatrix, StiffnessParts)> {
    let materials = model.materials();
    let which = model.material_indices()?;
    let dofs = model.dof_map();
    let mut parts: StiffnessParts = Vec::new();
    let mut total: Option<CsrMatrix> = None;
    for (m, (_, material)) in materials.iter().enumerate() {
        let pieces: Vec<(Option<Matrix6>, Damping)> = if is_viscoelastic(material) {
            let (e, nu, g0, k0, equal) = crate::mat_visco::harmonic_base(material)?;
            // a missing or bad temperature is refused here, not at the first frequency
            crate::mat_visco::complex_moduli(material, 0.0, options.temperature)?;
            let part = |part: ViscoPart, modulus: f64| Damping::Viscoelastic {
                material: Box::new(material.clone()),
                part,
                temperature: options.temperature,
                modulus,
            };
            if equal {
                vec![(Some(isotropic_elasticity(e, nu)), part(ViscoPart::Whole, e))]
            } else {
                vec![
                    (Some(shear_matrix(g0)), part(ViscoPart::Shear, g0)),
                    (Some(bulk_matrix(k0)), part(ViscoPart::Bulk, k0)),
                ]
            }
        } else {
            vec![(None, damping[m].clone())]
        };
        for (d, dm) in pieces {
            let km = assemble_with(model, options.threads, |ctx| {
                if which[ctx.element] != m {
                    let size: usize = ctx.nodes.iter().map(|&a| dofs.components(a)).sum();
                    return Ok(crate::element::ElementMatrix::zeros(size));
                }
                let d = match d {
                    Some(d) => d,
                    None => ctx.material.elasticity_matrix()?,
                };
                element_for(ctx.kind).stiffness(ctx.coords, &d)
            })?;
            total = match total {
                None => Some(km.clone()),
                Some(mut t) => {
                    if t.col != km.col || t.row_ptr != km.row_ptr {
                        return Err(invalid("pattern", "stiffness parts on different patterns".to_string()));
                    }
                    for (a, b) in t.val.iter_mut().zip(&km.val) {
                        *a += b;
                    }
                    Some(t)
                }
            };
            parts.push((km.val, dm));
        }
    }
    let total = total.ok_or_else(|| invalid("material", "the model has no material".to_string()))?;
    Ok((total, parts))
}

fn finite_nonneg(key: &'static str, what: &str, v: f64) -> Result<()> {
    if !(v.is_finite() && v >= 0.0) {
        return Err(invalid(key, format!("{what} {v:e} (must be finite and >= 0)")));
    }
    Ok(())
}

impl DirectHarmonic {
    /// Assembles the stiffness per material, the consistent mass and the dashpots of a model (solid elements, zero
    /// prescribed displacements, not on the transformation path) and checks the options.
    pub fn new(model: &SolidModel, options: &DirectHarmonicOptions) -> Result<DirectHarmonic> {
        model.validate()?;
        let visco = model.materials().iter().any(|(_, m)| is_viscoelastic(m));
        if visco {
            check_viscoelastic_model(model)?;
        } else {
            model.check_supported()?;
        }
        if crate::mpc::uses_transformation(model) {
            return Err(invalid(
                "constraint",
                "linear constraints, rigid sets, cyclic constraints and elastic supports are not taken by the direct \
                 harmonic solve (ADR 0448: the transformation path is not included)"
                    .to_string(),
            ));
        }
        let fixed = prescribed_equations(model, &[])?;
        if let Some(&(eq, v)) = fixed.iter().find(|&&(_, v)| v != 0.0) {
            let (node, component) = model.dof_map().node_component(eq).expect("existing equation");
            return Err(invalid(
                "constraint",
                format!(
                    "prescribed value {v:e} at node {node} component {component}: the direct harmonic solve takes zero \
                     values only (a harmonic base motion is given by BaseMotion)"
                ),
            ));
        }
        finite_nonneg("damping", "resonance pivot threshold", options.resonance_pivot)?;
        let rayleigh = options.rayleigh.unwrap_or((0.0, 0.0));
        finite_nonneg("damping", "Rayleigh mass coefficient", rayleigh.0)?;
        finite_nonneg("damping", "Rayleigh stiffness coefficient", rayleigh.1)?;

        // damping per material
        let materials = model.materials();
        let mut damping = vec![Damping::None; materials.len()];
        for (set, loss) in &options.loss_factors {
            let m = materials.iter().position(|(s, _)| s == set).ok_or_else(|| {
                invalid(
                    "loss factor",
                    format!("element set '{set}' carries no material of the model"),
                )
            })?;
            if damping[m] != Damping::None {
                return Err(invalid("loss factor", format!("element set '{set}' given twice")));
            }
            if is_viscoelastic(&materials[m].1) {
                return Err(invalid(
                    "loss factor",
                    format!(
                        "element set '{set}' carries a viscoelastic material, whose damping is its complex modulus (ADR \
                         0491): no loss factor is taken"
                    ),
                ));
            }
            damping[m] = match loss {
                LossFactor::Constant(eta) => {
                    finite_nonneg("loss factor", &format!("loss factor of set '{set}'"), *eta)?;
                    Damping::Constant(*eta)
                }
                LossFactor::Table(t) => match &materials[m].1 {
                    Material::IsotropicElastic { e, .. } => Damping::Table(t.clone(), *e),
                    _ => {
                        return Err(invalid(
                            "loss factor",
                            format!(
                                "a modulus table on set '{set}' needs an isotropic elastic material (the stiffness is \
                                 scaled by E'/E)"
                            ),
                        ))
                    }
                },
            };
        }

        // matrices on the mesh pattern
        let (k_total_m, mass_m, k_parts, k_total) = if visco {
            let (k_total_m, k_parts) = viscoelastic_parts(model, &damping, options)?;
            let mass_m = consistent_with(model, options.threads)?;
            let k_total = k_total_m.val.clone();
            (k_total_m, mass_m, k_parts, k_total)
        } else {
            let k_total_m = stiffness_with(model, options.threads)?;
            let mass_m = consistent_with(model, options.threads)?;
            let k_parts: Vec<(Vec<f64>, Damping)> = if materials.len() == 1 {
                vec![(k_total_m.val.clone(), damping[0].clone())]
            } else {
                let which = model.material_indices()?;
                let dofs = model.dof_map();
                let mut parts = Vec::with_capacity(materials.len());
                for (m, dm) in damping.iter().enumerate() {
                    let km = assemble_with(model, options.threads, |ctx| {
                        if which[ctx.element] != m {
                            let size: usize = ctx.nodes.iter().map(|&a| dofs.components(a)).sum();
                            return Ok(crate::element::ElementMatrix::zeros(size));
                        }
                        let d = ctx.material.elasticity_matrix()?;
                        element_for(ctx.kind).stiffness(ctx.coords, &d)
                    })?;
                    parts.push((km.val, dm.clone()));
                }
                parts
            };
            let k_total = if materials.len() == 1 {
                k_total_m.val.clone()
            } else {
                let mut s = vec![0.0; k_total_m.val.len()];
                for (v, _) in &k_parts {
                    for (a, b) in s.iter_mut().zip(v) {
                        *a += b;
                    }
                }
                s
            };
            (k_total_m, mass_m, k_parts, k_total)
        };
        if mass_m.col != k_total_m.col || mass_m.row_ptr != k_total_m.row_ptr {
            return Err(invalid(
                "pattern",
                "stiffness and mass on different patterns".to_string(),
            ));
        }

        // dashpots
        let dofs = model.dof_map();
        let n = dofs.n_equations();
        let n_nodes = model.mesh().nodes().len();
        let mut cd: BTreeMap<(usize, usize), f64> = BTreeMap::new();
        for (i, d) in options.dashpots.iter().enumerate() {
            finite_nonneg("dashpot", &format!("dashpot {i}: coefficient"), d.c)?;
            let eq = |node: usize| -> Result<usize> {
                if node >= n_nodes {
                    return Err(invalid(
                        "dashpot",
                        format!("dashpot {i}: node {node} does not exist ({n_nodes} nodes)"),
                    ));
                }
                dofs.equation(node, d.component).ok_or_else(|| {
                    invalid(
                        "dashpot",
                        format!("dashpot {i}: node {node} has no component {}", d.component),
                    )
                })
            };
            let a = eq(d.node)?;
            *cd.entry((a, a)).or_insert(0.0) += d.c;
            if let Some(other) = d.other {
                let b = eq(other)?;
                if b == a {
                    return Err(invalid("dashpot", format!("dashpot {i}: both ends at node {other}")));
                }
                *cd.entry((b, b)).or_insert(0.0) += d.c;
                *cd.entry((a, b)).or_insert(0.0) -= d.c;
                *cd.entry((b, a)).or_insert(0.0) -= d.c;
            }
        }

        let mut is_fixed = vec![false; n];
        for &(e, _) in &fixed {
            is_fixed[e] = true;
        }
        let free: Vec<usize> = (0..n).filter(|&e| !is_fixed[e]).collect();
        let mut reduced = vec![usize::MAX; n];
        for (r, &e) in free.iter().enumerate() {
            reduced[e] = r;
        }
        let mut rows_free = Vec::with_capacity(free.len());
        let mut rows_fixed = Vec::with_capacity(free.len());
        for &i in &free {
            let pat = k_total_m.row_ptr[i]..k_total_m.row_ptr[i + 1];
            let extra: Vec<(usize, f64)> = cd.range((i, 0)..(i + 1, 0)).map(|(&(_, j), &c)| (j, c)).collect();
            let mut merged: Vec<Entry> = Vec::with_capacity(pat.len() + extra.len());
            let (mut p, mut q) = (pat.start, 0usize);
            while p < pat.end || q < extra.len() {
                let cp = if p < pat.end { k_total_m.col[p] } else { usize::MAX };
                let cq = if q < extra.len() { extra[q].0 } else { usize::MAX };
                if cp == cq {
                    merged.push(Entry {
                        col: cp,
                        p: Some(p),
                        cd: extra[q].1,
                    });
                    p += 1;
                    q += 1;
                } else if cp < cq {
                    merged.push(Entry {
                        col: cp,
                        p: Some(p),
                        cd: 0.0,
                    });
                    p += 1;
                } else {
                    merged.push(Entry {
                        col: cq,
                        p: None,
                        cd: extra[q].1,
                    });
                    q += 1;
                }
            }
            let (f_part, x_part): (Vec<Entry>, Vec<Entry>) = merged.into_iter().partition(|e| !is_fixed[e.col]);
            rows_free.push(f_part);
            rows_fixed.push(x_part);
        }
        Ok(DirectHarmonic {
            dofs,
            n,
            k_parts,
            k_total,
            mass: mass_m.val,
            rayleigh,
            free,
            reduced,
            is_fixed,
            rows_free,
            rows_fixed,
            resonance_pivot: options.resonance_pivot,
        })
    }

    /// The free equations, ascending (reduced index `r` is full equation `free()[r]`).
    pub fn free(&self) -> &[usize] {
        &self.free
    }

    /// The coefficients `s_m (1 + i eta_m)` of the materials at `omega`.
    fn coefficients(&self, omega: f64) -> Result<Vec<C64>> {
        let f = omega / (2.0 * PI);
        self.k_parts
            .iter()
            .map(|(_, d)| match d {
                Damping::None => Ok(C64::ONE),
                Damping::Constant(eta) => Ok(C64::new(1.0, *eta)),
                Damping::Table(t, e) => {
                    let (storage, eta) = t.at(f)?;
                    let s = storage / e;
                    Ok(C64::new(s, s * eta))
                }
                Damping::Viscoelastic {
                    material,
                    part,
                    temperature,
                    modulus,
                } => {
                    let c = crate::mat_visco::complex_moduli(material, omega, *temperature)?;
                    let z = match part {
                        ViscoPart::Whole => c.young,
                        ViscoPart::Shear => c.shear,
                        ViscoPart::Bulk => c.bulk,
                    };
                    Ok(C64::new(z.re / modulus, z.im / modulus))
                }
            })
            .collect()
    }

    fn value(&self, e: &Entry, coef: &[C64], omega: f64) -> C64 {
        let (alpha, beta) = self.rayleigh;
        let w2 = omega * omega;
        let mut v = C64::ZERO;
        if let Some(p) = e.p {
            for ((k, _), c) in self.k_parts.iter().zip(coef) {
                v += c.scale(k[p]);
            }
            v += C64::from_real(-w2 * self.mass[p]);
            v += C64::new(0.0, omega * (alpha * self.mass[p] + beta * self.k_total[p]));
        }
        if e.cd != 0.0 {
            v += C64::new(0.0, omega * e.cd);
        }
        v
    }

    /// The steady response at `omega` (finite, `>= 0`) to `load`, without base motion.
    pub fn response(&self, load: &HarmonicLoad, omega: f64) -> Result<HarmonicResponse> {
        self.solve(load, &BaseMotion::new(), omega)
    }

    /// The steady response at `omega` to `load` and the base motion `base` (absolute displacements; the base values at
    /// their equations).
    pub fn response_with_base(&self, load: &HarmonicLoad, base: &BaseMotion, omega: f64) -> Result<HarmonicResponse> {
        self.solve(load, base, omega)
    }

    /// The responses at several frequencies, each bit-equal to [`DirectHarmonic::response`] at its frequency.
    pub fn sweep(&self, load: &HarmonicLoad, omegas: &[f64]) -> Result<Vec<HarmonicResponse>> {
        omegas.iter().map(|&w| self.response(load, w)).collect()
    }

    /// The stress of a response of this solver at the integration points: the elastic stress of
    /// [`HarmonicResponse::stress`] times the coefficient `s_m (1 + i eta_m)` of each element's material at the response's
    /// frequency, so that a loss factor gives the stress `E* eps` of the complex modulus (ADR 0448 amendment 2).
    pub fn stress(&self, model: &SolidModel, response: &HarmonicResponse) -> Result<HarmonicStress> {
        // ADR 0491 D2: the recovery evaluates the material at the points, which the viscoelastic material refuses.
        if self
            .k_parts
            .iter()
            .any(|(_, d)| matches!(d, Damping::Viscoelastic { .. }))
        {
            return Err(FemError::NotYetSupported {
                item: "material",
                variant: "Viscoelastic",
                location: "the harmonic stress".to_string(),
                package: crate::mat_visco::FOLLOW_UP,
            });
        }
        let mut st = response.stress(model)?;
        let coef = self.coefficients(response.omega)?;
        let which: Vec<usize> = if self.k_parts.len() == 1 {
            vec![0; model.mesh().n_elements()]
        } else {
            model.material_indices()?
        };
        let field = IntegrationPointField::compute(model, &vec![0.0; self.n])?;
        for (e, &m) in which.iter().enumerate() {
            let k = coef[m];
            for q in field.points_of(e) {
                for comp in 0..6 {
                    let z = C64::new(st.re[q][comp], st.im[q][comp]) * k;
                    st.re[q][comp] = z.re;
                    st.im[q][comp] = z.im;
                }
            }
        }
        Ok(st)
    }

    fn solve(&self, load: &HarmonicLoad, base: &BaseMotion, omega: f64) -> Result<HarmonicResponse> {
        if !(omega.is_finite() && omega >= 0.0) {
            return Err(invalid("frequency", format!("{omega:e} (must be finite and >= 0)")));
        }
        if load.re().len() != self.n {
            return Err(FemError::FieldLength {
                key: "harmonic load",
                expected: self.n,
                found: load.re().len(),
            });
        }
        let mut g = vec![C64::ZERO; self.n];
        let mut given = vec![false; self.n];
        for &(node, component, amp) in base.entries() {
            let eq = self.dofs.equation(node, component).ok_or_else(|| {
                invalid(
                    "base motion",
                    format!("node {node} component {component} is not an unknown of the model"),
                )
            })?;
            if !self.is_fixed[eq] {
                return Err(invalid(
                    "base motion",
                    format!("node {node} component {component} is free: base motion is given at prescribed equations"),
                ));
            }
            if given[eq] {
                return Err(invalid(
                    "base motion",
                    format!("node {node} component {component} given twice"),
                ));
            }
            if !amp.is_finite() {
                return Err(invalid(
                    "base motion",
                    format!("amplitude {amp} at node {node} component {component} is not finite"),
                ));
            }
            given[eq] = true;
            g[eq] = amp;
        }
        let coef = self.coefficients(omega)?;
        let nf = self.free.len();
        let mut row_ptr = Vec::with_capacity(nf + 1);
        row_ptr.push(0usize);
        let (mut col, mut val) = (Vec::new(), Vec::new());
        let mut rhs = Vec::with_capacity(nf);
        let mut damped = false;
        for (r, &i) in self.free.iter().enumerate() {
            for e in &self.rows_free[r] {
                let v = self.value(e, &coef, omega);
                damped |= v.im != 0.0;
                col.push(self.reduced[e.col]);
                val.push(v);
            }
            row_ptr.push(col.len());
            let mut b = C64::new(load.re()[i], load.im()[i]);
            for e in &self.rows_fixed[r] {
                if given[e.col] {
                    b -= self.value(e, &coef, omega) * g[e.col];
                }
            }
            rhs.push(b);
        }
        let a = ComplexCsr::new(nf, row_ptr, col, val).map_err(|e| invalid("pattern", e.to_string()))?;
        let opts = ComplexSkylineOptions {
            min_relative_pivot: if damped { 0.0 } else { self.resonance_pivot },
            ..ComplexSkylineOptions::default()
        };
        let ldlt = ComplexSkylineLdlt::factor_with(&a, &opts).map_err(|e| match e {
            ComplexSkylineError::Pivot {
                row,
                pivot,
                threshold,
                scale,
                ..
            } => {
                let at = match self.dofs.node_component(self.free[row]) {
                    Some((node, component)) => format!("reduced equation {row} (node {node} component {component})"),
                    None => format!("reduced equation {row}"),
                };
                let what = if damped {
                    "the damped dynamic stiffness is numerically singular without pivoting"
                } else {
                    "the undamped dynamic stiffness is singular: a resonance without damping, the steady response does \
                     not exist"
                };
                invalid(
                    "frequency",
                    format!(
                        "omega {omega:e} rad/s ({:e} Hz): {what}; pivot modulus {:e} at {at} is at most {threshold:e} \
                         (scale {scale:e})",
                        omega / (2.0 * PI),
                        pivot.abs()
                    ),
                )
            }
            other => invalid("frequency", format!("omega {omega:e} rad/s: {other}")),
        })?;
        let x = ldlt.solve(&rhs);
        let mut re: Vec<f64> = g.iter().map(|z| z.re).collect();
        let mut im: Vec<f64> = g.iter().map(|z| z.im).collect();
        for (&e, z) in self.free.iter().zip(&x) {
            re[e] = z.re;
            im[e] = z.im;
        }
        Ok(HarmonicResponse { omega, re, im })
    }
}
