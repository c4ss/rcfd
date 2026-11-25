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
//! stiffness; a loss factor on its set is refused. A model without the material takes the code of ADR 0448 as it was.
//! ADR 0494 (F-583): [`DirectHarmonic::stress`] of a viscoelastic material is `(G*/G_0) dev sigma_0 + (K*/K_0) vol
//! sigma_0` at each integration point, `sigma_0` the elastic stress of the base (`E*/E_0 sigma_0` for equal weights).
//!
//! ADR 0543 (F-528): a model with linear constraints, rigid sets or elastic supports is solved on the transformation `x = T q`
//! of ADR 0296: every real matrix of `A(omega)` (the stiffness parts, the supports `K_s`, the pre-stress `K_sigma`, the mass,
//! the Rayleigh stiffness `K_0 + K_s + K_sigma`, the dashpots) is reduced once, `T^T X T`, on one extended pattern; at each
//! frequency `A_r Q = T^T F` and the response is the mesh part of `T Q`. A cyclic constraint, a non-zero constraint value,
//! base motion and the supernodal factor are refused there. [`DirectHarmonicOptions::prestress`] adds `K_sigma` (ADR 0390)
//! on both paths, real and undamped, and in the Rayleigh stiffness. A model without these and without a pre-stress takes the
//! code of ADR 0448 as it was.
//!
//! ADR 0590 (F-669): [`DirectHarmonic::about_state`] takes the tangent of the nonlinear driver at a state in place of the
//! stiffness at rest (one undamped part; Rayleigh damping and dashpots as above; no loss factor, pre-stress or viscoelastic
//! material; the plain path). The code after the assembly is shared with [`DirectHarmonic::new`], moved unchanged.
//!
//! ADR 0596 (F-671): [`DirectHarmonic::about_state_with`] adds the tangents of the driver's contributions (a follower
//! pressure, by its symmetric part) at the state to `K_T`, in the operator and in the Rayleigh stiffness.
//!
//! ADR 0565 (F-619): [`DirectHarmonic::response_with_references`] takes a [`BaseMotion`] at the prescribed equations of a
//! model on the transformation path, `x = T q + G` with `G` the constant of the elimination for the complex base values
//! (`T` unchanged), `A_r Q = T^T (F - A_ext G)`, and returns the reference unknowns of the rigid sets with the response
//! ([`DirectHarmonicSolution`]). [`DirectHarmonic::response_with_base`] keeps refusing base motion on that path.
//!
//! ADR 0798 (F-630): with [`DirectHarmonicOptions::shells`] Shell4 and Shell3 on both paths (not with a viscoelastic
//! material); without it they are refused.

use crate::assemble::{assemble_with, stiffness_with};
use crate::constraint::{prescribed_equations, Prescribed};
use crate::element::ElementState;
use crate::error::{FemError, Result};
use crate::geometric::Prestress;
use crate::harmonic::{HarmonicLoad, HarmonicResponse, HarmonicStress};
use crate::mass::consistent_with;
use crate::material::{isotropic_elasticity, Matrix6};
use crate::model::Constraint;
use crate::model::{DofMap, Material, SolidModel};
use crate::recover::IntegrationPointField;
use rcfd_linalg::complex::C64;
use rcfd_linalg::cskyline::{ComplexCsr, ComplexSkylineError, ComplexSkylineLdlt, ComplexSkylineOptions};
// ADR 0509 (F-594): the complex supernodal factor after nested dissection by the node coordinates.
use crate::solve::DirectFactor;
use rcfd_linalg::sparse_chol::{
    nested_dissection_order_with_points, ComplexSupernodalLdlt, ComplexSupernodalOptions, SparseOrdering,
};
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

/// The harmonic motion of the reference point of one rigid set (ADR 0565 D3): the translation (x, y, z) then the small
/// rotation (x, y, z), real and imaginary parts.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct HarmonicReference {
    /// Index of the rigid set in [`SolidModel::constraints`].
    pub constraint: usize,
    /// The real parts of the six reference unknowns.
    pub re: [f64; 6],
    /// The imaginary parts.
    pub im: [f64; 6],
}

/// A response of [`DirectHarmonic::response_with_references`] (ADR 0565 D3): the mesh response and, on the transformation
/// path, the reference unknowns of every rigid set in the order of the constraints (none on the plain path).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct DirectHarmonicSolution {
    /// The mesh response (absolute displacements; the base values at their equations).
    pub response: HarmonicResponse,
    /// The reference unknowns of the rigid sets.
    pub references: Vec<HarmonicReference>,
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
    /// The factor of each frequency (ADR 0509 D4): [`DirectFactor::Envelope`] (the default, ADR 0448) or
    /// [`DirectFactor::SupernodalNestedDissection`] (the complex supernodal factor after nested dissection of the node
    /// coordinates, the order computed once); the minimum-degree factor is refused.
    pub factor: DirectFactor,
    /// A pre-stress whose geometric stiffness `K_sigma` (ADR 0390) is added, real and undamped, to the stiffness and to the
    /// Rayleigh stiffness (ADR 0543 D5; default none).
    pub prestress: Option<Prestress>,
    /// Take Shell4 and Shell3 blocks (ADR 0798 D1; default `false`: refused as before).
    pub shells: bool,
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
            factor: DirectFactor::Envelope,
            prestress: None,
            shells: false,
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

    /// The same options with Shell4 and Shell3 blocks taken (ADR 0798 D1).
    pub fn shells(mut self, on: bool) -> DirectHarmonicOptions {
        self.shells = on;
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

    /// The factor of each frequency (ADR 0509 D4).
    pub fn factor(mut self, factor: DirectFactor) -> DirectHarmonicOptions {
        self.factor = factor;
        self
    }

    /// The pre-stress of the geometric stiffness (ADR 0543 D5).
    pub fn prestress(mut self, prestress: Prestress) -> DirectHarmonicOptions {
        self.prestress = Some(prestress);
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
    /// The elimination order of the supernodal factor (ADR 0509 D4); `None`: the envelope.
    order: Option<Vec<usize>>,
    /// ADR 0543 D5: `K_sigma` on the mesh pattern of the plain path; `None` without a pre-stress.
    geometric: Option<Vec<f64>>,
    /// ADR 0543 D1: the reduced matrices of the transformation path; `None` on the plain path.
    transformed: Option<Box<Transformed>>,
    /// ADR 0590 D2: a solver of [`DirectHarmonic::about_state`] (its stress is refused).
    about_state: bool,
}

/// The real matrices of the transformation path reduced once, `T^T X T`, on one reduced pattern (ADR 0543 D1, D3).
#[derive(Debug, Clone)]
struct Transformed {
    tr: crate::mpc::Transformation,
    row_ptr: Vec<usize>,
    col: Vec<usize>,
    /// The reduced stiffness parts, in the order of `k_parts`.
    parts: Vec<Vec<f64>>,
    /// `T^T (K_s + K_sigma) T`, coefficient one.
    extra: Vec<f64>,
    /// `T^T (K_0 + K_s + K_sigma) T`, the Rayleigh stiffness.
    beta: Vec<f64>,
    mass: Vec<f64>,
    dashpots: Vec<f64>,
    /// ADR 0565 D1: the same real matrices on the extended mesh pattern (`n x n`), for `A_ext G`.
    ext: ExtendedMatrices,
    /// ADR 0565 D2: whether each mesh equation is prescribed.
    prescribed: Vec<bool>,
    /// ADR 0565 D1: the model, for the elimination with the base values.
    model: Box<SolidModel>,
}

/// The real matrices of the transformation path on the extended mesh pattern (ADR 0565 D1).
#[derive(Debug, Clone)]
struct ExtendedMatrices {
    row_ptr: Vec<usize>,
    col: Vec<usize>,
    parts: Vec<Vec<f64>>,
    extra: Vec<f64>,
    beta: Vec<f64>,
    mass: Vec<f64>,
    dashpots: Vec<f64>,
}

/// One entry of `A(omega)` from the real matrices of the transformation path at position `p` (ADR 0543 D1).
#[allow(clippy::too_many_arguments)]
fn dynamic_entry(
    parts: &[Vec<f64>],
    extra: &[f64],
    beta_k: &[f64],
    mass: &[f64],
    dashpots: &[f64],
    p: usize,
    coef: &[C64],
    rayleigh: (f64, f64),
    omega: f64,
) -> C64 {
    let (alpha, beta) = rayleigh;
    let w2 = omega * omega;
    let mut v = C64::ZERO;
    for (k, c) in parts.iter().zip(coef) {
        v += c.scale(k[p]);
    }
    v += C64::from_real(extra[p]);
    v += C64::from_real(-w2 * mass[p]);
    v += C64::new(0.0, omega * (alpha * mass[p] + beta * beta_k[p] + dashpots[p]));
    v
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
                    None => ctx.material.linear_elasticity_matrix()?,
                };
                crate::hex8x::element_stiffness_for(model.mesh(), ctx.element, ctx.kind, ctx.coords, &d)
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

/// Whether the element set holds an element of a Layup block (ADR 0798 D4).
fn has_layup(model: &SolidModel, set: &str) -> bool {
    let mesh = model.mesh();
    mesh.element_set(set).unwrap_or(&[]).iter().any(|&e| {
        matches!(
            mesh.blocks()[mesh.block_of(e)].property(),
            Some(crate::mesh::BlockProperty::Layup { .. })
        )
    })
}

impl DirectHarmonic {
    /// Assembles the stiffness per material, the consistent mass and the dashpots of a model (solid elements, zero
    /// prescribed displacements) and checks the options. A model with linear constraints, rigid sets or elastic supports is
    /// reduced on the transformation `x = T q` (ADR 0543 D1 to D4; zero values, no cyclic constraint, the envelope factor).
    pub fn new(model: &SolidModel, options: &DirectHarmonicOptions) -> Result<DirectHarmonic> {
        model.validate()?;
        // ADR 0525 D2: the direct solve takes the formulations; Incompatible with a viscoelastic material is refused.
        crate::hex8x::refuse_incompatible_viscoelastic(model, "the direct harmonic solve")?;
        let visco = model.materials().iter().any(|(_, m)| is_viscoelastic(m));
        if visco {
            // ADR 0798 D5: shells stay refused beside a viscoelastic material.
            check_viscoelastic_model(model)?;
        } else if options.shells {
            // ADR 0798 D1
            model.check_supported_shells()?;
        } else {
            model.check_supported()?;
        }
        // ADR 0543 D4: the transformation path takes linear constraints, rigid sets and elastic supports with zero values.
        let on_transformation = crate::mpc::uses_transformation(model);
        if on_transformation {
            if let Some(i) = model
                .constraints()
                .iter()
                .position(|c| matches!(c, Constraint::Cyclic { .. }))
            {
                return Err(invalid(
                    "constraint",
                    format!(
                        "constraint {i} (Cyclic): the direct harmonic solve does not take a cyclic constraint (its \
                         harmonic transformation belongs to a harmonic index; ADR 0543 D4)"
                    ),
                ));
            }
            crate::mpc::refuse_nonzero_values(model, "the direct harmonic solve on the transformation path")?;
            if options.factor != DirectFactor::Envelope {
                return Err(invalid(
                    "solver",
                    format!(
                        "factor {:?}: the direct harmonic solve on the transformation path takes the envelope factor \
                         only (ADR 0543 D4)",
                        options.factor
                    ),
                ));
            }
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
        // ADR 0509 D4: the envelope or the supernodal factor.
        if !matches!(
            options.factor,
            DirectFactor::Envelope | DirectFactor::SupernodalNestedDissection
        ) {
            return Err(invalid(
                "solver",
                format!(
                    "factor {:?}: the direct harmonic solve takes the envelope or the supernodal factor (ADR 0509)",
                    options.factor
                ),
            ));
        }
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
                // ADR 0798 D4: the moduli of a layup are those of its plies.
                LossFactor::Table(_) if has_layup(model, set) => return Err(invalid(
                    "loss factor",
                    format!(
                        "a modulus table on set '{set}', which holds an element of a Layup block: E'/E refers to the \
                             material of the set, not to those of the plies (ADR 0798 D4)"
                    ),
                )),
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
                        // ADR 0798 D2
                        crate::assemble::element_stiffness_of(model, ctx)
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
        // ADR 0543 D5: the geometric stiffness of the pre-stress on the same pattern.
        let geometric = match &options.prestress {
            None => None,
            Some(p) => {
                let kg = crate::geometric::assemble_geometric(model, p, options.threads)?;
                if kg.col != k_total_m.col || kg.row_ptr != k_total_m.row_ptr {
                    return Err(invalid(
                        "pattern",
                        "stiffness and geometric stiffness on different patterns".to_string(),
                    ));
                }
                Some(kg.val)
            }
        };

        Self::finish(
            model,
            options,
            rayleigh,
            &fixed,
            k_total_m,
            mass_m,
            k_parts,
            k_total,
            geometric,
            on_transformation,
            false,
        )
    }

    /// The direct harmonic solver about a nonlinear state (ADR 0590 D2): `A(omega) = K_T - omega^2 M + i omega (alpha M + beta
    /// K_T + C_d)`, `K_T` the tangent of the driver at the displacement `u` and the element `states`
    /// ([`crate::nonlinear::tangent`]), `M` the consistent mass, the Rayleigh damping and the dashpots of `options`. The model
    /// check is the driver's (the transformation path and temperature loads are refused); the prescribed equations of the
    /// model and of `prescribed` are held with zero increments (their values are not used; a [`BaseMotion`] gives a harmonic
    /// increment there). Refused: a loss factor (key `"loss factor"`), a pre-stress (key `"prestress"`: the state's stress is
    /// in `K_T`), a temperature (key `"temperature"`), a viscoelastic material (key `"material"`), a tangent off the mass
    /// pattern (key `"pattern"`). [`DirectHarmonic::stress`] of this solver is refused. At `u = 0` with the initial states the
    /// operator is that of [`DirectHarmonic::new`] without a pre-stress.
    pub fn about_state(
        model: &SolidModel,
        prescribed: &[Prescribed],
        u: &[f64],
        states: &[ElementState],
        options: &DirectHarmonicOptions,
    ) -> Result<DirectHarmonic> {
        Self::about_state_with(model, prescribed, u, states, 0.0, &[], options)
    }

    /// [`DirectHarmonic::about_state`] with the driver's contributions (ADR 0596 D2, F-671): `K = K_T + K_c(u, lambda)`, the
    /// contributions' tangents at the displacement `u` and `load_factor` (the triplets of
    /// [`crate::nonlinear::Contribution::add`], for a [`crate::follower::FollowerPressure`] the symmetric part of its load
    /// stiffness) added to the driver's tangent before the pattern test, in `A(omega)` and in the Rayleigh stiffness `beta
    /// K`. The factor is symmetric: where the antisymmetric part of a follower load couples free unknowns the response is
    /// that of the symmetric approximation. Contribution errors are keyed `"contribution"`. With no contribution it is
    /// [`DirectHarmonic::about_state`] (the same operations).
    pub fn about_state_with(
        model: &SolidModel,
        prescribed: &[Prescribed],
        u: &[f64],
        states: &[ElementState],
        load_factor: f64,
        contributions: &[&dyn crate::nonlinear::Contribution],
        options: &DirectHarmonicOptions,
    ) -> Result<DirectHarmonic> {
        model.validate()?;
        model.check_supported_nonlinear()?;
        crate::nonlinear::check_driver_model(model)?;
        if let Some((set, _)) = options.loss_factors.first() {
            return Err(invalid(
                "loss factor",
                format!(
                    "element set '{set}': the direct harmonic solve about a nonlinear state takes no loss factor \
                     (ADR 0590 D2)"
                ),
            ));
        }
        if options.prestress.is_some() {
            return Err(invalid(
                "prestress",
                "the direct harmonic solve about a nonlinear state takes the state's stress in its tangent; no \
                 pre-stress is added (ADR 0590 D2)"
                    .to_string(),
            ));
        }
        if let Some(t) = options.temperature {
            return Err(invalid(
                "temperature",
                format!(
                    "temperature {t:e}: not read by the direct harmonic solve about a nonlinear state (ADR 0590 D2)"
                ),
            ));
        }
        if let Some((set, m)) = model.materials().iter().find(|(_, m)| is_viscoelastic(m)) {
            return Err(invalid(
                "material",
                format!(
                    "element set '{set}' carries a {} material: the direct harmonic solve about a nonlinear state takes \
                     no complex modulus (ADR 0590 D2)",
                    m.name()
                ),
            ));
        }
        finite_nonneg("damping", "resonance pivot threshold", options.resonance_pivot)?;
        if !matches!(
            options.factor,
            DirectFactor::Envelope | DirectFactor::SupernodalNestedDissection
        ) {
            return Err(invalid(
                "solver",
                format!(
                    "factor {:?}: the direct harmonic solve takes the envelope or the supernodal factor (ADR 0509)",
                    options.factor
                ),
            ));
        }
        let rayleigh = options.rayleigh.unwrap_or((0.0, 0.0));
        finite_nonneg("damping", "Rayleigh mass coefficient", rayleigh.0)?;
        finite_nonneg("damping", "Rayleigh stiffness coefficient", rayleigh.1)?;
        let fixed: Vec<(usize, f64)> = prescribed_equations(model, prescribed)?
            .into_iter()
            .map(|(eq, _)| (eq, 0.0))
            .collect();
        let mut k_total_m = crate::nonlinear::tangent(model, u, states, options.threads)?;
        if !contributions.is_empty() {
            // ADR 0596 D2: the contributions' tangents at the state.
            let mut triplets = Vec::new();
            let mut scratch = vec![0.0; u.len()];
            for c in contributions {
                c.add(u, load_factor, &mut scratch, &mut triplets)
                    .map_err(|e| invalid("contribution", format!("contribution '{}': {e}", c.name())))?;
            }
            crate::nonlinear::add_triplets(&mut k_total_m, &triplets);
        }
        let mass_m = consistent_with(model, options.threads)?;
        if mass_m.col != k_total_m.col || mass_m.row_ptr != k_total_m.row_ptr {
            return Err(invalid(
                "pattern",
                "the tangent and the mass on different patterns".to_string(),
            ));
        }
        let k_total = k_total_m.val.clone();
        let k_parts: StiffnessParts = vec![(k_total_m.val.clone(), Damping::None)];
        Self::finish(
            model, options, rayleigh, &fixed, k_total_m, mass_m, k_parts, k_total, None, false, true,
        )
    }

    /// The dashpots, the transformation branch, the free rows and the nested-dissection order of a solver whose matrices are
    /// assembled (ADR 0590 D2: the code of [`DirectHarmonic::new`] after its assembly, moved unchanged; `about_state` marks a
    /// solver of [`DirectHarmonic::about_state`]).
    #[allow(clippy::too_many_arguments)]
    fn finish(
        model: &SolidModel,
        options: &DirectHarmonicOptions,
        rayleigh: (f64, f64),
        fixed: &[(usize, f64)],
        k_total_m: CsrMatrix,
        mass_m: CsrMatrix,
        k_parts: StiffnessParts,
        k_total: Vec<f64>,
        geometric: Option<Vec<f64>>,
        on_transformation: bool,
        about_state: bool,
    ) -> Result<DirectHarmonic> {
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

        if on_transformation {
            let mut t = transformed(
                model,
                &k_total_m,
                &k_parts,
                &k_total,
                &mass_m.val,
                geometric.as_deref(),
                &cd,
            )?;
            for &(e, _) in fixed {
                t.prescribed[e] = true;
            }
            return Ok(DirectHarmonic {
                dofs,
                n,
                k_parts,
                k_total,
                mass: mass_m.val,
                rayleigh,
                free: t.tr.independent().to_vec(),
                reduced: Vec::new(),
                is_fixed: vec![false; n],
                rows_free: Vec::new(),
                rows_fixed: Vec::new(),
                resonance_pivot: options.resonance_pivot,
                order: None,
                geometric: None,
                transformed: Some(Box::new(t)),
                about_state,
            });
        }
        let mut is_fixed = vec![false; n];
        for &(e, _) in fixed {
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
        // ADR 0509 D4: the nested dissection of the free-free pattern by the coordinates of each free equation's node
        let order = if options.factor == DirectFactor::SupernodalNestedDissection {
            let nf = free.len();
            let mut row_ptr = Vec::with_capacity(nf + 1);
            row_ptr.push(0usize);
            let mut col = Vec::new();
            for row in &rows_free {
                col.extend(row.iter().map(|e| reduced[e.col]));
                row_ptr.push(col.len());
            }
            let pattern = CsrMatrix {
                n: nf,
                ncols: nf,
                row_ptr,
                val: vec![1.0; col.len()],
                col,
            };
            let nodes = model.mesh().nodes();
            let points: Vec<[f64; 3]> = free
                .iter()
                .map(|&e| nodes[dofs.node_component(e).expect("free equation exists").0])
                .collect();
            Some(nested_dissection_order_with_points(&pattern, &points))
        } else {
            None
        };
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
            order,
            geometric,
            transformed: None,
            about_state,
        })
    }

    /// The free equations, ascending (reduced index `r` is full equation `free()[r]`). On the transformation path (ADR 0543
    /// D6) the independent unknowns of `T`, ascending (a reference unknown of a rigid set is `>=` the number of equations).
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
            // ADR 0543 D5: the pre-stress, real, and in the Rayleigh stiffness.
            let kb = match &self.geometric {
                Some(g) => {
                    v += C64::from_real(g[p]);
                    self.k_total[p] + g[p]
                }
                None => self.k_total[p],
            };
            v += C64::from_real(-w2 * self.mass[p]);
            v += C64::new(0.0, omega * (alpha * self.mass[p] + beta * kb));
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
        // ADR 0590 D2: the recovered stress is that of the material at rest, not the state's increment.
        if self.about_state {
            return Err(invalid(
                "stress",
                "the stress of a direct harmonic solve about a nonlinear state is not recovered (ADR 0590 D2)"
                    .to_string(),
            ));
        }
        // ADR 0494 D1: a viscoelastic part takes the split stress of the elastic base.
        if self
            .k_parts
            .iter()
            .any(|(_, d)| matches!(d, Damping::Viscoelastic { .. }))
        {
            return self.stress_viscoelastic(model, response);
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

    /// The stress of a response of a solver with a viscoelastic part (ADR 0494 D1): `sigma_0` of the model with every
    /// viscoelastic material replaced by its elastic base, then per element `(G*/G_0) dev sigma_0 + (K*/K_0) vol sigma_0`
    /// for unequal weights and `(E*/E_0) sigma_0` (as the other materials) otherwise.
    fn stress_viscoelastic(&self, model: &SolidModel, response: &HarmonicResponse) -> Result<HarmonicStress> {
        let coef = self.coefficients(response.omega)?;
        // the coefficient pair of each material, walking the parts in the order the matrices were built
        let mut pairs: Vec<(C64, C64, bool)> = Vec::new();
        let mut cursor = 0;
        for (i, (_, material)) in model.materials().iter().enumerate() {
            let Some((_, d)) = self.k_parts.get(cursor) else {
                return Err(invalid(
                    "model",
                    "the model has fewer materials than the solver".to_string(),
                ));
            };
            if is_viscoelastic(material) != matches!(d, Damping::Viscoelastic { .. }) {
                return Err(invalid(
                    "model",
                    format!("material {i} of the model is not the material of the solver's part"),
                ));
            }
            if let Damping::Viscoelastic {
                part: ViscoPart::Shear, ..
            } = d
            {
                if cursor + 1 >= self.k_parts.len() {
                    return Err(invalid("model", "the model does not match the solver".to_string()));
                }
                pairs.push((coef[cursor], coef[cursor + 1], true));
                cursor += 2;
            } else {
                pairs.push((coef[cursor], coef[cursor], false));
                cursor += 1;
            }
        }
        if cursor != self.k_parts.len() {
            return Err(invalid(
                "model",
                "the model has more materials than the solver's parts".to_string(),
            ));
        }
        let elastic = model.with_viscoelastic_bases();
        let mut st = response.stress(&elastic)?;
        let which = model.material_indices()?;
        let field = IntegrationPointField::compute(&elastic, &vec![0.0; self.n])?;
        for (e, &m) in which.iter().enumerate() {
            let (cs, ck, split) = pairs[m];
            for q in field.points_of(e) {
                if !split {
                    for comp in 0..6 {
                        let z = C64::new(st.re[q][comp], st.im[q][comp]) * cs;
                        st.re[q][comp] = z.re;
                        st.im[q][comp] = z.im;
                    }
                    continue;
                }
                let (re0, im0) = (st.re[q], st.im[q]);
                let (mr, mi) = ((re0[0] + re0[1] + re0[2]) / 3.0, (im0[0] + im0[1] + im0[2]) / 3.0);
                for comp in 0..6 {
                    let (vr, vi) = if comp < 3 { (mr, mi) } else { (0.0, 0.0) };
                    let dev = C64::new(re0[comp] - vr, im0[comp] - vi);
                    let z = dev * cs + C64::new(vr, vi) * ck;
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
        if let Some(t) = &self.transformed {
            return self.solve_transformed(t, load, base, omega);
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
        let refusal = |e: ComplexSkylineError| match e {
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
        };
        let x = match &self.order {
            None => ComplexSkylineLdlt::factor_with(&a, &opts).map_err(refusal)?.solve(&rhs),
            // ADR 0509 D4: the supernodal factor in the order of `new`, the same pivot rule
            Some(order) => {
                let sopts = ComplexSupernodalOptions {
                    ordering: SparseOrdering::Given(order.clone()),
                    symmetry_tolerance: opts.symmetry_tolerance,
                    min_relative_pivot: opts.min_relative_pivot,
                };
                ComplexSupernodalLdlt::factor_with(&a, &sopts)
                    .map_err(refusal)?
                    .solve(&rhs)
            }
        };
        let mut re: Vec<f64> = g.iter().map(|z| z.re).collect();
        let mut im: Vec<f64> = g.iter().map(|z| z.im).collect();
        for (&e, z) in self.free.iter().zip(&x) {
            re[e] = z.re;
            im[e] = z.im;
        }
        Ok(HarmonicResponse { omega, re, im })
    }
}

// ---------------------------------------------------------------------------------------- the transformation path

/// The reduced matrices of the transformation path (ADR 0543 D1 to D3): the extended pattern (the mesh pattern of
/// `k_mesh`, the dashpot entries `cd` and the support triplets), each real matrix put on it and reduced, `T^T X T`.
fn transformed(
    model: &SolidModel,
    k_mesh: &CsrMatrix,
    k_parts: &[(Vec<f64>, Damping)],
    k_total: &[f64],
    mass: &[f64],
    geometric: Option<&[f64]>,
    cd: &BTreeMap<(usize, usize), f64>,
) -> Result<Transformed> {
    let tr = crate::mpc::transformation(model, &[])?;
    let n = k_mesh.n;
    let mut supports: BTreeMap<(usize, usize), f64> = BTreeMap::new();
    for (i, j, v) in crate::mpc::support_triplets(model)? {
        *supports.entry((i, j)).or_insert(0.0) += v;
    }
    // the extended pattern, row by row
    let mut row_ptr = Vec::with_capacity(n + 1);
    row_ptr.push(0usize);
    let mut col: Vec<usize> = Vec::new();
    for i in 0..n {
        let mut cols: Vec<usize> = k_mesh.col[k_mesh.row_ptr[i]..k_mesh.row_ptr[i + 1]].to_vec();
        cols.extend(cd.range((i, 0)..(i + 1, 0)).map(|(&(_, j), _)| j));
        cols.extend(supports.range((i, 0)..(i + 1, 0)).map(|(&(_, j), _)| j));
        cols.sort_unstable();
        cols.dedup();
        col.extend(cols);
        row_ptr.push(col.len());
    }
    let position = |i: usize, j: usize| -> usize {
        let row = &col[row_ptr[i]..row_ptr[i + 1]];
        row_ptr[i] + row.binary_search(&j).expect("entry of the extended pattern")
    };
    // the mesh pattern's entries in the extended pattern
    let mut at = vec![0usize; k_mesh.col.len()];
    for i in 0..n {
        for p in k_mesh.row_ptr[i]..k_mesh.row_ptr[i + 1] {
            at[p] = position(i, k_mesh.col[p]);
        }
    }
    let nnz = col.len();
    let on_mesh = |v: &[f64]| -> Vec<f64> {
        let mut out = vec![0.0; nnz];
        for (p, &x) in v.iter().enumerate() {
            out[at[p]] += x;
        }
        out
    };
    let mut ks = vec![0.0; nnz];
    for (&(i, j), &v) in &supports {
        ks[position(i, j)] += v;
    }
    let mut c_d = vec![0.0; nnz];
    for (&(i, j), &v) in cd {
        c_d[position(i, j)] += v;
    }
    let mut extra = ks;
    if let Some(g) = geometric {
        for (e, x) in extra.iter_mut().zip(on_mesh(g)) {
            *e += x;
        }
    }
    let mut beta = on_mesh(k_total);
    for (b, x) in beta.iter_mut().zip(&extra) {
        *b += x;
    }
    let zero = vec![0.0; tr.n_unknowns()];
    let mut pattern: Option<(Vec<usize>, Vec<usize>)> = None;
    let mut reduce = |v: Vec<f64>| -> Result<Vec<f64>> {
        let m = CsrMatrix {
            n,
            ncols: n,
            row_ptr: row_ptr.clone(),
            col: col.clone(),
            val: v,
        };
        let (r, _) = tr.reduce(&m, &zero)?;
        match &pattern {
            None => pattern = Some((r.row_ptr, r.col)),
            Some((rp, c)) => {
                if *rp != r.row_ptr || *c != r.col {
                    return Err(invalid("pattern", "reduced matrices on different patterns".to_string()));
                }
            }
        }
        Ok(r.val)
    };
    let ext_parts: Vec<Vec<f64>> = k_parts.iter().map(|(v, _)| on_mesh(v)).collect();
    let ext_mass = on_mesh(mass);
    let mut parts = Vec::with_capacity(k_parts.len());
    for v in &ext_parts {
        parts.push(reduce(v.clone())?);
    }
    let ext_extra = extra.clone();
    let ext_beta = beta.clone();
    let ext_dashpots = c_d.clone();
    let extra = reduce(extra)?;
    let beta = reduce(beta)?;
    let mass = reduce(ext_mass.clone())?;
    let dashpots = reduce(c_d)?;
    let (r_ptr, r_col) = pattern.expect("one matrix reduced at least");
    Ok(Transformed {
        tr,
        row_ptr: r_ptr,
        col: r_col,
        parts,
        extra,
        beta,
        mass,
        dashpots,
        ext: ExtendedMatrices {
            row_ptr,
            col,
            parts: ext_parts,
            extra: ext_extra,
            beta: ext_beta,
            mass: ext_mass,
            dashpots: ext_dashpots,
        },
        prescribed: vec![false; n],
        model: Box::new(model.clone()),
    })
}

impl DirectHarmonic {
    /// A name of independent unknown `r` of the transformation path (the names of `crate::mpc::unknown_name`).
    fn unknown_name(&self, t: &Transformed, r: usize) -> String {
        let i = t.tr.independent()[r];
        if i < t.tr.n_mesh() {
            return match self.dofs.node_component(i) {
                Some((node, component)) => format!("node {node} component {component}"),
                None => format!("unknown {i}"),
            };
        }
        match t.tr.references().iter().rev().find(|&&(_, o)| o <= i) {
            Some(&(constraint, r0)) => {
                let what = [
                    "translation x",
                    "translation y",
                    "translation z",
                    "rotation x",
                    "rotation y",
                    "rotation z",
                ][i - r0];
                format!("the reference {what} of constraint {constraint} (RigidSet)")
            }
            None => format!("unknown {i}"),
        }
    }

    /// The solve on the transformation path (ADR 0543 D1): `A_r(omega) Q = T^T F`, the response the mesh part of `T Q`.
    fn solve_transformed(
        &self,
        t: &Transformed,
        load: &HarmonicLoad,
        base: &BaseMotion,
        omega: f64,
    ) -> Result<HarmonicResponse> {
        if let Some(&(node, component, _)) = base.entries().first() {
            return Err(invalid(
                "base motion",
                format!(
                    "node {node} component {component}: base motion is not taken on the transformation path (ADR 0543 \
                     D4)"
                ),
            ));
        }
        let (response, _, _) = self.solve_transformed_full(t, load, None, omega)?;
        Ok(response)
    }

    /// The solve on the transformation path with an optional base constant `G = (re, im)` over all `N` unknowns (ADR 0543
    /// D1, ADR 0565 D1): `A_r(omega) Q = T^T (F - A_ext G)` (`T^T F` without one), the solution `x = T Q + G` over all `N`
    /// unknowns; returns the mesh response and the real and imaginary parts of `x`.
    fn solve_transformed_full(
        &self,
        t: &Transformed,
        load: &HarmonicLoad,
        base: Option<&(Vec<f64>, Vec<f64>)>,
        omega: f64,
    ) -> Result<(HarmonicResponse, Vec<f64>, Vec<f64>)> {
        let coef = self.coefficients(omega)?;
        let nq = t.row_ptr.len() - 1;
        let mut val = Vec::with_capacity(t.col.len());
        let mut damped = false;
        for p in 0..t.col.len() {
            let v = dynamic_entry(
                &t.parts,
                &t.extra,
                &t.beta,
                &t.mass,
                &t.dashpots,
                p,
                &coef,
                self.rayleigh,
                omega,
            );
            damped |= v.im != 0.0;
            val.push(v);
        }
        let a = ComplexCsr::new(nq, t.row_ptr.clone(), t.col.clone(), val)
            .map_err(|e| invalid("pattern", e.to_string()))?;
        let nn = t.tr.n_unknowns();
        let extend = |v: &[f64]| {
            let mut x = v.to_vec();
            x.resize(nn, 0.0);
            x
        };
        let (b_re, b_im) = match base {
            None => (
                t.tr.transpose_times(&extend(load.re())),
                t.tr.transpose_times(&extend(load.im())),
            ),
            Some((g_re, g_im)) => {
                // ADR 0565 D1: F - A_ext G on the mesh rows (the reference rows of A_ext are zero)
                let e = &t.ext;
                let (mut y_re, mut y_im) = (extend(load.re()), extend(load.im()));
                for i in 0..self.n {
                    let mut s = C64::ZERO;
                    for p in e.row_ptr[i]..e.row_ptr[i + 1] {
                        let j = e.col[p];
                        let gj = C64::new(g_re[j], g_im[j]);
                        if gj == C64::ZERO {
                            continue;
                        }
                        let v = dynamic_entry(
                            &e.parts,
                            &e.extra,
                            &e.beta,
                            &e.mass,
                            &e.dashpots,
                            p,
                            &coef,
                            self.rayleigh,
                            omega,
                        );
                        s += v * gj;
                    }
                    y_re[i] -= s.re;
                    y_im[i] -= s.im;
                }
                (t.tr.transpose_times(&y_re), t.tr.transpose_times(&y_im))
            }
        };
        let rhs: Vec<C64> = b_re.iter().zip(&b_im).map(|(&r, &i)| C64::new(r, i)).collect();
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
                let what = if damped {
                    "the damped dynamic stiffness is numerically singular without pivoting"
                } else {
                    "the undamped dynamic stiffness is singular: a resonance without damping, the steady response does \
                     not exist"
                };
                invalid(
                    "frequency",
                    format!(
                        "omega {omega:e} rad/s ({:e} Hz): {what}; pivot modulus {:e} at reduced unknown {row} ({}) on the \
                         transformation path is at most {threshold:e} (scale {scale:e})",
                        omega / (2.0 * PI),
                        pivot.abs(),
                        self.unknown_name(t, row)
                    ),
                )
            }
            other => invalid("frequency", format!("omega {omega:e} rad/s: {other}")),
        })?;
        let q = ldlt.solve(&rhs);
        let q_re: Vec<f64> = q.iter().map(|z| z.re).collect();
        let q_im: Vec<f64> = q.iter().map(|z| z.im).collect();
        let mut x_re = t.tr.expand(&q_re);
        let mut x_im = t.tr.expand(&q_im);
        if let Some((g_re, g_im)) = base {
            for (x, g) in x_re.iter_mut().zip(g_re) {
                *x += g;
            }
            for (x, g) in x_im.iter_mut().zip(g_im) {
                *x += g;
            }
        }
        let re = x_re[..self.n].to_vec();
        let im = x_im[..self.n].to_vec();
        Ok((HarmonicResponse { omega, re, im }, x_re, x_im))
    }

    /// The constant `G` of `x = T q + G` for the base motion `base` at prescribed equations (ADR 0565 D1, D2): the
    /// elimination of the model's constraints with the real and with the imaginary base values.
    fn base_constant(&self, t: &Transformed, base: &BaseMotion) -> Result<(Vec<f64>, Vec<f64>)> {
        let (mut v_re, mut v_im) = (BTreeMap::new(), BTreeMap::new());
        for &(node, component, amp) in base.entries() {
            let eq = self.dofs.equation(node, component).ok_or_else(|| {
                invalid(
                    "base motion",
                    format!("node {node} component {component} is not an unknown of the model"),
                )
            })?;
            if !t.prescribed[eq] {
                return Err(invalid(
                    "base motion",
                    format!(
                        "node {node} component {component} is not a prescribed equation of the model: base motion is \
                         given at prescribed equations (ADR 0565 D2)"
                    ),
                ));
            }
            if v_re.contains_key(&eq) {
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
            v_re.insert(eq, amp.re);
            v_im.insert(eq, amp.im);
        }
        let mut parts = Vec::with_capacity(2);
        for values in [&v_re, &v_im] {
            let tv = crate::mpc::transformation_with_values(&t.model, values)?;
            if tv.t() != t.tr.t() || tv.independent() != t.tr.independent() {
                return Err(invalid(
                    "base motion",
                    "the elimination with the base values gives another transformation (internal; ADR 0565 D1)"
                        .to_string(),
                ));
            }
            parts.push(tv.g().to_vec());
        }
        let g_im = parts.pop().expect("two eliminations");
        let g_re = parts.pop().expect("two eliminations");
        Ok((g_re, g_im))
    }

    /// The steady response at `omega` to `load` and the base motion `base` with the reference unknowns of the rigid sets
    /// (ADR 0565). On the transformation path a base motion at the model's prescribed equations enters `x = T q + G`,
    /// `A_r Q = T^T (F - A_ext G)`; the response is the mesh part of `x`, the references its reference unknowns. On the plain
    /// path the response of [`DirectHarmonic::response_with_base`] and no references.
    pub fn response_with_references(
        &self,
        load: &HarmonicLoad,
        base: &BaseMotion,
        omega: f64,
    ) -> Result<DirectHarmonicSolution> {
        let Some(t) = &self.transformed else {
            let response = self.solve(load, base, omega)?;
            return Ok(DirectHarmonicSolution {
                response,
                references: Vec::new(),
            });
        };
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
        let g = if base.entries().is_empty() {
            None
        } else {
            Some(self.base_constant(t, base)?)
        };
        let (response, x_re, x_im) = self.solve_transformed_full(t, load, g.as_ref(), omega)?;
        let references =
            t.tr.references()
                .iter()
                .map(|&(constraint, r0)| {
                    let mut re = [0.0; 6];
                    let mut im = [0.0; 6];
                    re.copy_from_slice(&x_re[r0..r0 + 6]);
                    im.copy_from_slice(&x_im[r0..r0 + 6]);
                    HarmonicReference { constraint, re, im }
                })
                .collect();
        Ok(DirectHarmonicSolution { response, references })
    }
}
