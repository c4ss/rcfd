//! Modes of a rotating part: spin softening, modes over speed, mode tracking, Campbell table. Declared by ADR 0291; filled
//! by ADR 0403 (the registration of ADR 0299, F-396).
//!
//! A part rotating at the constant speed `Omega` about the unit axis `a` is analysed in the frame that rotates with it
//! (ADR 0403 D1). The kinetic energy `1/2 int rho |u_t + Omega a x (X - p + u)|^2` gives, beside the mass and the centrifugal
//! body force, the term `1/2 Omega^2 int rho |a x u|^2`, a negative stiffness, the spin softening `-Omega^2 M_perp` with
//! `M_perp[(a, i), (b, j)] = m_ab (delta_ij - a_i a_j)` ([`spin_softening_mass`]). The centrifugal pre-stress is linear in
//! `Omega^2`: one static solve at 1 rad/s gives `K_sigma,1` ([`crate::geometric`]), and
//!
//! `K_eff(Omega) phi = omega^2 M phi`, `K_eff = K + Omega^2 (K_sigma,1 - M_perp)`.
//!
//! The Coriolis (gyroscopic) term is neglected (ADR 0403 D6). At `Omega = 0` the result is that of
//! [`crate::modal::modal_analysis`] itself. At `Omega > 0` the reduced `K_eff` is tested by its inertia first: a speed at
//! which it is not positive definite is reported as [`SpeedModes::LossOfStiffness`] with the speed (D3), never as a
//! frequency. [`RotatingPart::campbell`] solves a list of speeds, tracks the modes by the mass-weighted modal assurance (D4)
//! and finds the crossings with the engine orders by interpolation linear in `Omega^2` ([`engine_order_crossings`]); the
//! table is written as CSV and JSON.

use crate::assemble::{assemble_with, stiffness_with};
use crate::constraint::{check_rigid_body_motions, prescribed_equations, reduce};
use crate::element::{element_for, ElementMatrix};
use crate::error::{FemError, Result};
use crate::geometric::{geometric_stiffness, Prestress};
use crate::mass::{consistent_with, element_mass};
use crate::modal::{modal_analysis, ModalOptions, ModalResult};
use crate::model::{Load, SolidModel};
use crate::mpc::Transformation;
use crate::solve::{solve_static, StaticOptions};
use rcfd_core::vec3::{norm, Vec3};
use rcfd_linalg::eigen::{count_below, lowest_eigenpairs, EigenError, Shift, SubspaceOptions};
use rcfd_linalg::skyline::SkylineOrdering;
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;
use std::fmt::Write as _;

fn unit_axis(axis: Vec3) -> Result<Vec3> {
    let len = norm(axis);
    if !(len.is_finite() && len > 0.0) || axis.iter().any(|c| !c.is_finite()) {
        return Err(FemError::InvalidValue {
            key: "axis",
            reason: format!("{axis:?} is not a finite non-zero direction"),
        });
    }
    Ok([axis[0] / len, axis[1] / len, axis[2] / len])
}

/// The spin-softening mass `M_perp[(a, i), (b, j)] = m_ab (delta_ij - a_i a_j)` of a rotation about `axis` (ADR 0403 D1),
/// `m_ab = int rho N_a N_b` with each element's mass rule (the scalar of [`crate::mass::consistent`]), on the stiffness
/// pattern, with `threads` workers (`None`: the available parallelism); bit-identical for any value. A zero or non-finite
/// axis is refused (key `"axis"`).
pub fn spin_softening_mass(model: &SolidModel, axis: Vec3, threads: Option<usize>) -> Result<CsrMatrix> {
    let a = unit_axis(axis)?;
    let mut p = [[0.0; 3]; 3];
    for (i, row) in p.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = if i == j { 1.0 } else { 0.0 } - a[i] * a[j];
        }
    }
    assemble_with(model, threads, |ctx| {
        let m = element_mass(element_for(ctx.kind), ctx.coords, ctx.material.density())?;
        let size = m.size();
        let n = size / 3;
        let mut out = ElementMatrix::zeros(size);
        let src = m.values();
        let v = out.values_mut();
        for ia in 0..n {
            for ib in 0..n {
                let mab = src[(3 * ia) * size + 3 * ib];
                for i in 0..3 {
                    for j in 0..3 {
                        v[(3 * ia + i) * size + 3 * ib + j] = mab * p[i][j];
                    }
                }
            }
        }
        Ok(out)
    })
}

/// Options of a rotating modal analysis.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct RotatingOptions {
    /// The options of each modal analysis (modes, tolerance, residual test, shift, threads). The default shift at a speed
    /// above zero is `Shift::Value(0.0)`, after the inertia test has found `K_eff` positive definite.
    pub modal: ModalOptions,
    /// Whether the geometric stiffness of the centrifugal pre-stress is included (default true).
    pub stress_stiffening: bool,
    /// Whether the spin softening `-Omega^2 M_perp` is included (default true).
    pub spin_softening: bool,
    /// The options of the static solve of the pre-stress at 1 rad/s.
    pub static_options: StaticOptions,
}

impl RotatingOptions {
    /// The lowest `modes` modes, both effects, the default modal and static options.
    pub fn new(modes: usize) -> RotatingOptions {
        RotatingOptions {
            modal: ModalOptions::new(modes),
            stress_stiffening: true,
            spin_softening: true,
            static_options: StaticOptions::default(),
        }
    }
    /// The same options with other modal options.
    pub fn modal(mut self, modal: ModalOptions) -> RotatingOptions {
        self.modal = modal;
        self
    }
    /// The same options with the stress stiffening switched on or off.
    pub fn stress_stiffening(mut self, on: bool) -> RotatingOptions {
        self.stress_stiffening = on;
        self
    }
    /// The same options with the spin softening switched on or off.
    pub fn spin_softening(mut self, on: bool) -> RotatingOptions {
        self.spin_softening = on;
        self
    }
}

/// A speed at which the reduced `K_eff` is not positive definite (ADR 0403 D3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LossOfStiffness {
    /// The speed, rad/s.
    pub speed: f64,
    /// The number of negative eigenvalues of `K_eff` on the admissible motions (the negative pivots of its factor);
    /// `None` when the factorisation met a zero pivot (a speed at the loss itself).
    pub negative: Option<usize>,
}

/// The result of the modal analysis at one speed.
#[derive(Debug, Clone, PartialEq)]
pub enum SpeedModes {
    /// The modes of `K_eff phi = omega^2 M phi`.
    Modes(ModalResult),
    /// `K_eff` is not positive definite at this speed: no frequency is computed.
    LossOfStiffness(LossOfStiffness),
}

impl SpeedModes {
    /// The modes, if the speed has them.
    pub fn modes(&self) -> Option<&ModalResult> {
        match self {
            SpeedModes::Modes(r) => Some(r),
            SpeedModes::LossOfStiffness(_) => None,
        }
    }
    /// The loss of stiffness, if the speed has one.
    pub fn loss(&self) -> Option<&LossOfStiffness> {
        match self {
            SpeedModes::Modes(_) => None,
            SpeedModes::LossOfStiffness(l) => Some(l),
        }
    }
}

enum Path {
    Elimination { fixed: Vec<(usize, f64)> },
    Transformation(Box<Transformation>),
}

/// A model prepared for modal analyses at any speed about one axis: the pre-stress at 1 rad/s and the matrices `K` (with
/// the supports on the transformation path), `K_sigma,1`, `M` and `M_perp`, assembled once (ADR 0403 D1, D5).
pub struct RotatingPart<'a> {
    model: &'a SolidModel,
    options: RotatingOptions,
    prestress: Prestress,
    k: CsrMatrix,
    kg1: CsrMatrix,
    m: CsrMatrix,
    m_perp: CsrMatrix,
    path: Path,
}

/// A copy of `model` whose only load is the centrifugal load at 1 rad/s.
fn unit_centrifugal_model(model: &SolidModel, axis: Vec3, origin: Vec3) -> Result<SolidModel> {
    let mut copy = SolidModel::new(model.mesh().clone());
    for (set, material) in model.materials() {
        copy.add_material(set, material.clone())?;
    }
    for c in model.constraints() {
        copy.add_constraint(c.clone())?;
    }
    for s in model.supports() {
        copy.add_support(s.clone())?;
    }
    copy.add_load(Load::centrifugal(axis, origin, 1.0))?;
    Ok(copy)
}

impl<'a> RotatingPart<'a> {
    /// Prepares `model` rotating about `axis` through `origin`: one static solve of the centrifugal load at 1 rad/s with the
    /// model's constraints and supports (its own loads are not part of the pre-stress), then the four matrices. A zero or
    /// non-finite axis (`"axis"`), a non-finite origin (`"origin"`) and a non-zero constraint value are refused.
    pub fn new(model: &'a SolidModel, axis: Vec3, origin: Vec3, options: &RotatingOptions) -> Result<RotatingPart<'a>> {
        check_frame(axis, origin)?;
        check_model(model)?;
        let prestress = if options.stress_stiffening {
            let copy = unit_centrifugal_model(model, axis, origin)?;
            let r = solve_static(&copy, &options.static_options)?;
            Prestress::from_static(&copy, &r)?
        } else {
            Prestress::uniform(model, [0.0; 6])?
        };
        RotatingPart::with_unit_prestress(model, axis, origin, prestress, options)
    }

    /// As [`RotatingPart::new`] with a pre-stress the caller computed at 1 rad/s (for example of a sector), which must fit
    /// the model (`"prestress"`).
    pub fn with_unit_prestress(
        model: &'a SolidModel,
        axis: Vec3,
        origin: Vec3,
        prestress: Prestress,
        options: &RotatingOptions,
    ) -> Result<RotatingPart<'a>> {
        check_frame(axis, origin)?;
        check_model(model)?;
        prestress.check_fits(model)?;
        let threads = options.modal.threads;
        let mut k = stiffness_with(model, threads)?;
        let path = if crate::mpc::uses_transformation(model) {
            crate::mpc::add_supports(&mut k, model)?;
            Path::Transformation(Box::new(crate::mpc::transformation(model, &[])?))
        } else {
            Path::Elimination {
                fixed: prescribed_equations(model, &[])?,
            }
        };
        let kg1 = geometric_stiffness(model, &prestress, threads)?;
        let m = consistent_with(model, threads)?;
        let m_perp = spin_softening_mass(model, axis, threads)?;
        Ok(RotatingPart {
            model,
            options: options.clone(),
            prestress,
            k,
            kg1,
            m,
            m_perp,
            path,
        })
    }

    /// The pre-stress at 1 rad/s.
    pub fn unit_prestress(&self) -> &Prestress {
        &self.prestress
    }
    /// The consistent mass `M` (the model's equations).
    pub fn mass(&self) -> &CsrMatrix {
        &self.m
    }
    /// The spin-softening mass `M_perp`.
    pub fn spin_softening(&self) -> &CsrMatrix {
        &self.m_perp
    }
    /// The geometric stiffness of the pre-stress at 1 rad/s, `K_sigma,1`.
    pub fn unit_geometric_stiffness(&self) -> &CsrMatrix {
        &self.kg1
    }

    /// `K + Omega^2 (s K_sigma,1 - c M_perp)` on the stiffness pattern, `s` and `c` 1 or 0 by the options; on the
    /// transformation path `K` includes the supports.
    pub fn operator(&self, omega: f64) -> CsrMatrix {
        let w2 = omega * omega;
        let s = if self.options.stress_stiffening { 1.0 } else { 0.0 };
        let c = if self.options.spin_softening { 1.0 } else { 0.0 };
        let mut out = self.k.clone();
        for ((v, g), mp) in out.val.iter_mut().zip(&self.kg1.val).zip(&self.m_perp.val) {
            *v += w2 * (s * g - c * mp);
        }
        out
    }

    /// `phi^T K_sigma,1 phi / phi^T M phi`: the Rayleigh quotient of the centrifugal tension at 1 rad/s.
    pub fn stiffening_quotient(&self, phi: &[f64]) -> f64 {
        quad(&self.kg1, phi) / quad(&self.m, phi)
    }

    /// `phi^T (s K_sigma,1 - c M_perp) phi / phi^T M phi`: at a mode at rest, the first-order slope `d omega^2 / d Omega^2`.
    pub fn slope(&self, phi: &[f64]) -> f64 {
        let s = if self.options.stress_stiffening { 1.0 } else { 0.0 };
        let c = if self.options.spin_softening { 1.0 } else { 0.0 };
        (s * quad(&self.kg1, phi) - c * quad(&self.m_perp, phi)) / quad(&self.m, phi)
    }

    /// The modes at the speed `omega`, rad/s (ADR 0403 D1 to D3). At `omega = 0` the result of
    /// [`crate::modal::modal_analysis`] with the modal options. A negative or non-finite speed is refused (`"speed"`).
    pub fn modes(&self, omega: f64) -> Result<SpeedModes> {
        if !(omega.is_finite() && omega >= 0.0) {
            return Err(FemError::InvalidValue {
                key: "speed",
                reason: format!("{omega} rad/s is not a finite speed >= 0"),
            });
        }
        if omega == 0.0 {
            return modal_analysis(self.model, &self.options.modal).map(SpeedModes::Modes);
        }
        let opts = &self.options.modal;
        let dofs = self.model.dof_map();
        let keff = self.operator(omega);
        let n = self.m.n;
        let (kr, mr, free): (CsrMatrix, CsrMatrix, Option<Vec<usize>>) = match &self.path {
            Path::Elimination { fixed } => {
                let zero = vec![0.0; n];
                let kr = reduce(&keff, &zero, fixed)?;
                let mr = reduce(&self.m, &zero, fixed)?;
                let free = kr.free().to_vec();
                (kr.matrix, mr.matrix, Some(free))
            }
            Path::Transformation(tr) => {
                let zero = vec![0.0; tr.n_unknowns()];
                let (kr, _) = tr.reduce(&keff, &zero)?;
                let (mr, _) = tr.reduce(&self.m, &zero)?;
                (kr, mr, None)
            }
        };
        let nf = kr.n;
        if opts.modes == 0 || opts.modes >= nf {
            return Err(FemError::InvalidValue {
                key: "modes",
                reason: format!(
                    "{} modes requested of a model with {nf} free unknowns (at least 1 and fewer than the unknowns)",
                    opts.modes
                ),
            });
        }
        for (i, d) in mr.diag().iter().enumerate() {
            if d.is_nan() || *d <= 0.0 {
                let what = match (&self.path, &free) {
                    (Path::Elimination { .. }, Some(free)) => {
                        let (node, component) = dofs.node_component(free[i]).expect("free equation exists");
                        format!("node {node} component {component}")
                    }
                    (Path::Transformation(tr), _) => crate::mpc::unknown_name(tr, self.model, tr.independent()[i]),
                    _ => unreachable!("the elimination path has its free list"),
                };
                return Err(FemError::InvalidValue {
                    key: "density",
                    reason: format!("{what} has no mass (diagonal {d:e})"),
                });
            }
        }
        // D3: the inertia of the reduced K_eff.
        match count_below(&kr, &mr, 0.0, &SkylineOrdering::Rcm) {
            Ok(0) => {}
            Ok(negative) => {
                return Ok(SpeedModes::LossOfStiffness(LossOfStiffness {
                    speed: omega,
                    negative: Some(negative),
                }))
            }
            Err(EigenError::CountFailed { .. }) => {
                return Ok(SpeedModes::LossOfStiffness(LossOfStiffness {
                    speed: omega,
                    negative: None,
                }))
            }
            Err(e) => {
                return Err(FemError::Solver {
                    solver: "count_below",
                    reason: e.to_string(),
                })
            }
        }
        let sub = SubspaceOptions {
            shift: opts.shift.unwrap_or(Shift::Value(0.0)),
            tolerance: opts.tolerance,
            residual_test: opts.residual_test,
            max_iterations: opts.max_iterations,
            subspace_size: opts.subspace_size,
            ..SubspaceOptions::default()
        };
        let eig = lowest_eigenpairs(&kr, &mr, opts.modes, &sub).map_err(|e| FemError::Solver {
            solver: "lowest_eigenpairs",
            reason: e.to_string(),
        })?;
        let modes = match (&self.path, &free) {
            (Path::Elimination { .. }, Some(free)) => eig
                .vectors
                .iter()
                .map(|v| {
                    let mut full = vec![0.0; n];
                    for (&eq, &x) in free.iter().zip(v) {
                        full[eq] = x;
                    }
                    full
                })
                .collect(),
            (Path::Transformation(tr), _) => eig.vectors.iter().map(|q| tr.expand(q)[..n].to_vec()).collect(),
            _ => unreachable!("the elimination path has its free list"),
        };
        Ok(SpeedModes::Modes(ModalResult::from_parts(eig, modes, nf, false, dofs)))
    }

    /// The modes over a list of speeds, rad/s (finite, `>= 0`, strictly increasing; refused otherwise, key `"speeds"`),
    /// tracked by the mass-weighted modal assurance (ADR 0403 D4).
    pub fn campbell(&self, speeds: &[f64]) -> Result<CampbellTable> {
        check_speeds(speeds)?;
        let mut results = Vec::with_capacity(speeds.len());
        for &w in speeds {
            results.push(self.modes(w)?);
        }
        Ok(CampbellTable::track(speeds, &results, &self.m))
    }
}

fn check_frame(axis: Vec3, origin: Vec3) -> Result<()> {
    unit_axis(axis)?;
    if origin.iter().any(|c| !c.is_finite()) {
        return Err(FemError::InvalidValue {
            key: "origin",
            reason: format!("{origin:?} is not finite"),
        });
    }
    Ok(())
}

fn check_model(model: &SolidModel) -> Result<()> {
    model.validate()?;
    model.check_supported()?;
    if crate::mpc::uses_transformation(model) {
        return crate::mpc::refuse_nonzero_values(model, "a rotating modal analysis");
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
    // A free body is allowed: at a speed above zero it is reported as a loss of stiffness.
    match check_rigid_body_motions(model, &fixed) {
        Ok(()) | Err(FemError::RigidBodyMotionFree { .. }) => Ok(()),
        Err(e) => Err(e),
    }
}

fn check_speeds(speeds: &[f64]) -> Result<()> {
    if speeds.is_empty() {
        return Err(FemError::InvalidValue {
            key: "speeds",
            reason: "the list of speeds is empty".to_string(),
        });
    }
    for (i, &w) in speeds.iter().enumerate() {
        if !(w.is_finite() && w >= 0.0) {
            return Err(FemError::InvalidValue {
                key: "speeds",
                reason: format!("speed {i} ({w} rad/s) is not a finite speed >= 0"),
            });
        }
        if i > 0 && w <= speeds[i - 1] {
            return Err(FemError::InvalidValue {
                key: "speeds",
                reason: format!(
                    "speed {i} ({w} rad/s) does not exceed speed {} ({} rad/s): the list must increase",
                    i - 1,
                    speeds[i - 1]
                ),
            });
        }
    }
    Ok(())
}

fn quad(a: &CsrMatrix, x: &[f64]) -> f64 {
    let mut y = vec![0.0; a.n];
    a.matvec(x, &mut y);
    x.iter().zip(&y).map(|(p, q)| p * q).sum()
}

fn bilinear(a: &CsrMatrix, x: &[f64], z: &[f64]) -> f64 {
    let mut y = vec![0.0; a.n];
    a.matvec(z, &mut y);
    x.iter().zip(&y).map(|(p, q)| p * q).sum()
}

/// The mass-weighted modal assurance `(phi^T M psi)^2 / ((phi^T M phi)(psi^T M psi))` (ADR 0403 D4).
pub fn mac(m: &CsrMatrix, phi: &[f64], psi: &[f64]) -> f64 {
    let c = bilinear(m, phi, psi);
    c * c / (quad(m, phi) * quad(m, psi))
}

/// The speeds, rad/s, at which a branch `lambda(Omega)` given at increasing speeds (`None` where it has no value) crosses the
/// engine-order line `omega = order Omega`: on each interval with values at both ends, `lambda` linear in `Omega^2`, the root
/// of `lambda - order^2 Omega^2` (ADR 0403 D4). Exact for `lambda = omega_0^2 + S Omega^2`.
pub fn engine_order_crossings(speeds: &[f64], eigenvalues: &[Option<f64>], order: u32) -> Vec<f64> {
    let k2 = f64::from(order) * f64::from(order);
    let g = |j: usize| eigenvalues[j].map(|l| (speeds[j] * speeds[j], l - k2 * speeds[j] * speeds[j]));
    let mut out = Vec::new();
    let n = speeds.len().min(eigenvalues.len());
    for j in 0..n {
        if let Some((_, gj)) = g(j) {
            if gj == 0.0 {
                out.push(speeds[j]);
            }
        }
        if j + 1 < n {
            if let (Some((sa, ga)), Some((sb, gb))) = (g(j), g(j + 1)) {
                if (ga < 0.0 && gb > 0.0) || (ga > 0.0 && gb < 0.0) {
                    let s = sa - ga * (sb - sa) / (gb - ga);
                    out.push(s.sqrt());
                }
            }
        }
    }
    out
}

/// One tracked mode over the speeds of a [`CampbellTable`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ModeTrack {
    /// `omega^2` at each speed (`None` at a loss of stiffness or before the track starts).
    pub eigenvalues: Vec<Option<f64>>,
    /// Frequency, Hz, at each speed.
    pub frequencies: Vec<Option<f64>>,
    /// The modal assurance with the track's previous mode (`None` at its first speed and where it has no mode).
    pub mac: Vec<Option<f64>>,
    /// The mode shape at each speed (the model's equations).
    pub modes: Vec<Option<Vec<f64>>>,
}

/// A crossing of a tracked mode with an engine order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Crossing {
    /// The track.
    pub track: usize,
    /// The engine order `k` of the line `f = k Omega/(2 pi)`.
    pub order: u32,
    /// The speed, rad/s.
    pub speed: f64,
    /// The frequency, Hz, `order speed/(2 pi)`.
    pub frequency: f64,
}

/// The table of a Campbell diagram: the tracked modes over increasing speeds (ADR 0403 D4).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct CampbellTable {
    /// The speeds, rad/s.
    pub speeds: Vec<f64>,
    /// The tracked modes.
    pub tracks: Vec<ModeTrack>,
    /// The speeds at which `K_eff` is not positive definite.
    pub losses: Vec<LossOfStiffness>,
}

impl CampbellTable {
    fn track(speeds: &[f64], results: &[SpeedModes], m: &CsrMatrix) -> CampbellTable {
        let ns = speeds.len();
        let mut tracks: Vec<ModeTrack> = Vec::new();
        let mut losses = Vec::new();
        for (j, r) in results.iter().enumerate() {
            match r {
                SpeedModes::LossOfStiffness(l) => losses.push(*l),
                SpeedModes::Modes(res) => {
                    if tracks.is_empty() {
                        for (i, phi) in res.modes.iter().enumerate() {
                            let mut t = ModeTrack {
                                eigenvalues: vec![None; ns],
                                frequencies: vec![None; ns],
                                mac: vec![None; ns],
                                modes: vec![None; ns],
                            };
                            t.eigenvalues[j] = Some(res.eigenvalues[i]);
                            t.frequencies[j] = Some(res.frequencies[i]);
                            t.modes[j] = Some(phi.clone());
                            tracks.push(t);
                        }
                        continue;
                    }
                    let last: Vec<&Vec<f64>> = tracks
                        .iter()
                        .map(|t| {
                            t.modes[..j]
                                .iter()
                                .rev()
                                .find_map(|m| m.as_ref())
                                .expect("a track has a mode at its start")
                        })
                        .collect();
                    let mut pairs = Vec::with_capacity(last.len() * res.modes.len());
                    for (t, phi) in last.iter().enumerate() {
                        for (i, psi) in res.modes.iter().enumerate() {
                            pairs.push((mac(m, phi, psi), t, i));
                        }
                    }
                    // Decreasing MAC; ties by the lower track, then the lower mode.
                    pairs.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
                    let mut track_used = vec![false; tracks.len()];
                    let mut mode_used = vec![false; res.modes.len()];
                    for (value, t, i) in pairs {
                        if track_used[t] || mode_used[i] {
                            continue;
                        }
                        track_used[t] = true;
                        mode_used[i] = true;
                        let tr = &mut tracks[t];
                        tr.eigenvalues[j] = Some(res.eigenvalues[i]);
                        tr.frequencies[j] = Some(res.frequencies[i]);
                        tr.mac[j] = Some(value);
                        tr.modes[j] = Some(res.modes[i].clone());
                    }
                }
            }
        }
        CampbellTable {
            speeds: speeds.to_vec(),
            tracks,
            losses,
        }
    }

    /// The crossings of every track with the engine order `order` (by [`engine_order_crossings`]), track by track in
    /// increasing speed.
    pub fn crossings(&self, order: u32) -> Vec<Crossing> {
        let mut out = Vec::new();
        for (t, tr) in self.tracks.iter().enumerate() {
            for speed in engine_order_crossings(&self.speeds, &tr.eigenvalues, order) {
                out.push(Crossing {
                    track: t,
                    order,
                    speed,
                    frequency: f64::from(order) * speed / (2.0 * PI),
                });
            }
        }
        out
    }

    /// The table as CSV: `speed_rad_s,speed_rpm,track_0_hz,...,loss_negative`, one row per speed; a frequency is empty
    /// where the track has none, `loss_negative` is the negative count at a loss of stiffness (`singular` for a zero
    /// pivot) and empty otherwise. Numbers in Rust's shortest round-trip form.
    pub fn to_csv(&self) -> String {
        let mut s = String::from("speed_rad_s,speed_rpm");
        for t in 0..self.tracks.len() {
            let _ = write!(s, ",track_{t}_hz");
        }
        s.push_str(",loss_negative\n");
        for (j, &w) in self.speeds.iter().enumerate() {
            let _ = write!(s, "{w},{}", w * 60.0 / (2.0 * PI));
            for t in &self.tracks {
                match t.frequencies[j] {
                    Some(f) => {
                        let _ = write!(s, ",{f}");
                    }
                    None => s.push(','),
                }
            }
            match self.losses.iter().find(|l| l.speed.to_bits() == w.to_bits()) {
                Some(LossOfStiffness { negative: Some(n), .. }) => {
                    let _ = write!(s, ",{n}");
                }
                Some(LossOfStiffness { negative: None, .. }) => s.push_str(",singular"),
                None => s.push(','),
            }
            s.push('\n');
        }
        s
    }

    /// The table as JSON: the speeds, each track's frequencies (Hz) and modal assurance (`null` where it has none), the
    /// losses of stiffness and the crossings with the given engine orders.
    pub fn to_json(&self, orders: &[u32]) -> String {
        fn opt(v: Option<f64>) -> String {
            v.map_or_else(|| "null".to_string(), |x| format!("{x}"))
        }
        fn list(v: impl Iterator<Item = String>) -> String {
            format!("[{}]", v.collect::<Vec<_>>().join(", "))
        }
        let mut s = String::from("{\n");
        let _ = writeln!(
            s,
            "  \"speeds_rad_s\": {},",
            list(self.speeds.iter().map(|w| format!("{w}")))
        );
        s.push_str("  \"tracks\": [\n");
        for (t, tr) in self.tracks.iter().enumerate() {
            let _ = write!(
                s,
                "    {{\"track\": {t}, \"frequencies_hz\": {}, \"mac\": {}}}",
                list(tr.frequencies.iter().map(|&f| opt(f))),
                list(tr.mac.iter().map(|&m| opt(m)))
            );
            s.push_str(if t + 1 < self.tracks.len() { ",\n" } else { "\n" });
        }
        s.push_str("  ],\n");
        let _ = writeln!(
            s,
            "  \"losses_of_stiffness\": {},",
            list(self.losses.iter().map(|l| format!(
                "{{\"speed_rad_s\": {}, \"negative_eigenvalues\": {}}}",
                l.speed,
                l.negative.map_or_else(|| "null".to_string(), |n| n.to_string())
            )))
        );
        let crossings: Vec<Crossing> = orders.iter().flat_map(|&k| self.crossings(k)).collect();
        let _ = writeln!(
            s,
            "  \"crossings\": {}",
            list(crossings.iter().map(|c| format!(
                "{{\"track\": {}, \"order\": {}, \"speed_rad_s\": {}, \"frequency_hz\": {}}}",
                c.track, c.order, c.speed, c.frequency
            )))
        );
        s.push('}');
        s.push('\n');
        s
    }
}
