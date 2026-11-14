//! Gyroscopic matrix in the rotating frame and the split Campbell table. Declared by ADR 0440 (F-408, the scaffold of ADR 0311);
//! filled by ADR 0472 (F-415, design stub ADR 0318).
//!
//! In the frame rotating at `Omega` about the axis `a` the free vibration about the pre-stressed state is `M u_tt + G u_t +
//! K_eff(Omega) u = 0` (ADR 0472 D1), `K_eff` the operator of F-396 ([`RotatingPart::operator`]) and `G = 2 Omega C`
//! ([`gyroscopic_matrix`], `C` the Coriolis matrix of ADR 0411, skew bit for bit). The roots of `(lambda^2 M + lambda G + K_eff) x
//! = 0` are found on a modal subspace (D2): the F-396 modes at the speed when `K_eff` is positive definite (the **speed basis**),
//! the F-396 modes at rest at a speed of loss of stiffness (the **rest basis**); `M_r`, `K_r` symmetrised, `C_r` skew-symmetrised;
//! the `2 m` roots by the general quadratic eigensolver of ADR 0450 ([`rcfd_linalg::eigen_general::quadratic_eigen_with`]) in the
//! scaled time `lambda = tau mu`, `tau = sqrt(max |K_r,ii|/M_r,ii) + 2 Omega` (D3). Each root with `Im lambda > REAL_TOLERANCE
//! |lambda|` and `Im lambda > 0` is a split mode (the rotating-frame frequency `Im lambda`, the growth `Re lambda`, the whirl number
//! and direction of ADR 0411 D3); the speed is unstable when `max Re lambda > STABILITY_TOLERANCE max |lambda|` (D4, the flag of
//! ADR 0416). [`RotatingPart::split_campbell`] tracks the modes over the speeds by the complex-line tracker of ADR 0411 D4 (D5).
//! Opt-in by [`crate::rotating::RotatingOptions::coriolis`]: without it the split table is the F-396 table itself. A model with a
//! cyclic constraint is refused by the whole-part functions (D6; the whole-sector problem is
//! [`crate::rotating::RotatingSector::whirl_modes`]).
//!
//! ADR 0481 (F-555): the same problem on a cyclic sector, [`RotatingSector::split_modes`] and [`RotatingSector::split_campbell`]:
//! at each harmonic index the basis is the F-396 harmonic modes of the index ([`RotatingSector::modes`], expanded to the cosine
//! and sine parts for a pair), and `K_eff`, `M` and the Coriolis matrix are doubled block-diagonally for a pair (ADR 0411 D5), so
//! the cosine/sine coupling enters through the basis. A sector exists only for materials whose axes pass the check of ADR 0462;
//! without the option the results are the F-396 sector results themselves.
//!
//! ADR 0482 (F-569): with [`crate::rotating::RotatingOptions::split_correction`] the basis of an index gains the static
//! correction vectors `K_b^-1 C_r q_i` of its harmonic modes (reduced coordinates, `M_r`-orthonormalised by the rule of ADR 0464,
//! dependent ones dropped), and every split mode reports the first-order Ritz error `eta` of ADR 0481 D5, computed from the
//! split mode alone by the projected correction equation on the complement of the basis (MINRES, preconditioned by `K_b^-1`).
//!
//! ADR 0492 (F-575): beside it, [`RotatingSector::split_modes`] reports `schur_eta`, the Ritz error of the Schur-complemented
//! projected problem `Q_VV(l) - Q_VW Q_WW(theta)^-1 Q_WV` frozen at the Ritz value (the coupling of the complement with every
//! basis vector: one complement solve per basis vector, then one dense quadratic solve per split mode).

use crate::cyclic::{doubled_matrix, HarmonicModes};
use crate::damped::{m_orthonormalise, REAL_TOLERANCE};
use crate::error::{FemError, Result};
use crate::model::{Constraint, SolidModel};
use crate::rotating::{
    check_harmonics, check_speeds, coriolis_matrix, track_whirl, Entry, HarmonicSpeedModes, LossOfStiffness,
    RotatingPart, RotatingSector, SpeedModes, Stability, Whirl, WhirlCampbell, WhirlMode, WhirlModes,
    STABILITY_TOLERANCE,
};
use rcfd_core::vec3::Vec3;
use rcfd_linalg::complex::C64;
use rcfd_linalg::eigen_general::{quadratic_eigen_with, GeneralEigenError, GeneralEigenOptions};
use rcfd_linalg::sparse_chol::SparseLdlt;
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;
use std::fmt::Write as _;

/// The work package that fills this module; `None` once it has landed.
pub const PENDING: Option<&'static str> = None;

fn invalid(key: &'static str, reason: String) -> FemError {
    FemError::InvalidValue { key, reason }
}

fn check_speed(omega: f64) -> Result<()> {
    if omega.is_finite() && omega >= 0.0 {
        Ok(())
    } else {
        Err(invalid("speed", format!("{omega} rad/s is not a finite speed >= 0")))
    }
}

/// The gyroscopic matrix `G = 2 omega C` of a rotation at `omega` rad/s about `axis` (ADR 0472 D1), `C` the Coriolis matrix of
/// [`coriolis_matrix`], on the stiffness pattern; skew bit for bit. Refuses a negative or non-finite speed (`"speed"`) and a
/// zero or non-finite axis (`"axis"`).
pub fn gyroscopic_matrix(model: &SolidModel, axis: Vec3, omega: f64, threads: Option<usize>) -> Result<CsrMatrix> {
    check_speed(omega)?;
    let mut g = coriolis_matrix(model, axis, threads)?;
    let s = 2.0 * omega;
    for v in &mut g.val {
        *v *= s;
    }
    Ok(g)
}

/// One split mode of a dense problem (ADR 0472 D4): `x e^{lambda t}`, `lambda = growth + i omega`.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct DenseSplitMode {
    /// `Im lambda`, rad/s, positive.
    pub omega: f64,
    /// `omega^2`.
    pub eigenvalue: f64,
    /// `Re lambda`, 1/s.
    pub growth: f64,
    /// The vector, `x^H M x = 1`.
    pub q: Vec<C64>,
    /// The whirl number `2 Re(x)^T C Im(x) / x^H M x`.
    pub whirl: f64,
    /// Its direction (ADR 0411 D3).
    pub direction: Whirl,
    /// The relative residual of the root (the solver's, on the scaled problem).
    pub residual: f64,
}

/// The roots of a dense gyroscopic problem (ADR 0472 D3, D4).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct DenseSplit {
    /// The split modes (oscillating roots with `Im lambda > 0`), by `omega` ascending.
    pub modes: Vec<DenseSplitMode>,
    /// All `2 n` roots `lambda`, 1/s, in the solver's order.
    pub roots: Vec<C64>,
    /// The real parts of the non-oscillating roots (`|Im lambda| <= REAL_TOLERANCE |lambda|`).
    pub real_roots: Vec<f64>,
    /// The flag of D4.
    pub stability: Stability,
    /// `max Re lambda / max |lambda|` (0 if every root is 0).
    pub real_defect: f64,
}

fn check_dense(a: &[f64], n: usize, what: &str) -> Result<()> {
    if a.len() != n * n {
        return Err(invalid(
            "matrix",
            format!("{what} has {} entries, {n} x {n} = {} expected", a.len(), n * n),
        ));
    }
    if let Some(i) = a.iter().position(|v| !v.is_finite()) {
        return Err(invalid("matrix", format!("{what} entry {i} is not finite")));
    }
    Ok(())
}

/// The split modes of `(lambda^2 M + lambda G + K) x = 0`, `G = 2 omega C`, for dense row-major `n x n` matrices (ADR 0472 D3,
/// D4): `M`, `K` symmetrised, `C` skew-symmetrised, solved by the quadratic eigensolver of ADR 0450 in the scaled time. Refuses
/// `n = 0`, a wrong length or a non-finite entry (`"matrix"`), a negative or non-finite speed (`"speed"`) and a mass that is not
/// positive definite (`"mass"`).
pub fn split_modes_dense(m: &[f64], c: &[f64], omega: f64, k: &[f64], n: usize) -> Result<DenseSplit> {
    if n == 0 {
        return Err(invalid("matrix", "the problem has no unknowns".to_string()));
    }
    check_dense(m, n, "M")?;
    check_dense(c, n, "C")?;
    check_dense(k, n, "K")?;
    check_speed(omega)?;
    let mut ms = vec![0.0; n * n];
    let mut ks = vec![0.0; n * n];
    let mut cs = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            ms[i * n + j] = 0.5 * (m[i * n + j] + m[j * n + i]);
            ks[i * n + j] = 0.5 * (k[i * n + j] + k[j * n + i]);
            cs[i * n + j] = 0.5 * (c[i * n + j] - c[j * n + i]);
        }
    }
    let mut kmax: f64 = 0.0;
    for i in 0..n {
        let d = ms[i * n + i];
        if d.is_nan() || d <= 0.0 {
            return Err(invalid(
                "mass",
                format!("diagonal {i} of M is {d:e}: not positive definite"),
            ));
        }
        kmax = kmax.max(ks[i * n + i].abs() / d);
    }
    let mut tau = kmax.sqrt() + 2.0 * omega;
    if !(tau > 0.0 && tau.is_finite()) {
        tau = 1.0;
    }
    let gs: Vec<f64> = cs.iter().map(|v| 2.0 * omega * v / tau).collect();
    let kt: Vec<f64> = ks.iter().map(|v| v / (tau * tau)).collect();
    let q = quadratic_eigen_with(&ms, &gs, &kt, None, n, &GeneralEigenOptions::default()).map_err(|e| match e {
        GeneralEigenError::MassNotPositiveDefinite { .. } => invalid("mass", e.to_string()),
        other => FemError::Solver {
            solver: "quadratic_eigen",
            reason: other.to_string(),
        },
    })?;
    let roots: Vec<C64> = q.values.iter().map(|mu| mu.scale(tau)).collect();
    let mut modes = Vec::new();
    let mut real_roots = Vec::new();
    for (idx, lam) in roots.iter().enumerate() {
        if lam.im.abs() <= REAL_TOLERANCE * lam.abs() {
            real_roots.push(lam.re);
            continue;
        }
        if lam.im <= 0.0 {
            continue;
        }
        let x = &q.vectors[idx];
        let mut num = 0.0;
        let mut den = 0.0;
        for i in 0..n {
            for j in 0..n {
                num += x[i].re * cs[i * n + j] * x[j].im;
                den += ms[i * n + j] * (x[i].re * x[j].re + x[i].im * x[j].im);
            }
        }
        let kappa = 2.0 * num / den;
        modes.push(DenseSplitMode {
            omega: lam.im,
            eigenvalue: lam.im * lam.im,
            growth: lam.re,
            q: x.clone(),
            whirl: kappa,
            direction: Whirl::of(kappa),
            residual: q.residuals[idx],
        });
    }
    modes.sort_by(|a, b| a.omega.total_cmp(&b.omega));
    let max_re = roots.iter().map(|l| l.re).fold(f64::NEG_INFINITY, f64::max);
    let max_abs = roots.iter().map(|l| l.abs()).fold(0.0, f64::max);
    let real_defect = if max_abs > 0.0 { max_re / max_abs } else { 0.0 };
    let stability = if max_abs > 0.0 && max_re > STABILITY_TOLERANCE * max_abs {
        Stability::Unstable { growth: max_re }
    } else {
        Stability::Stable
    };
    Ok(DenseSplit {
        modes,
        roots,
        real_roots,
        stability,
        real_defect,
    })
}

/// The basis of the projection at one speed (ADR 0472 D2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Basis {
    /// The F-396 modes at the speed (`K_eff` positive definite).
    Speed,
    /// The F-396 modes at rest (a speed of loss of stiffness).
    Rest,
}

/// One split mode of a part, `u = Re((re + i im) e^{lambda t})`, `lambda = growth + i omega` (ADR 0472 D4).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct SplitMode {
    /// The rotating-frame frequency `Im lambda`, rad/s.
    pub omega: f64,
    /// `omega^2`.
    pub eigenvalue: f64,
    /// `omega / (2 pi)`, Hz.
    pub frequency: f64,
    /// `Re lambda`, 1/s.
    pub growth: f64,
    /// The real part of the shape (the model's equations), `phi^H M phi = 1`.
    pub re: Vec<f64>,
    /// The imaginary part.
    pub im: Vec<f64>,
    /// The whirl number `kappa` (ADR 0411 D3; 0 without the option).
    pub whirl: f64,
    /// Its direction.
    pub direction: Whirl,
    /// The residual of the reduced root (the modal residual without the option).
    pub residual: f64,
}

/// The split modes of a part at one speed (ADR 0472 D4, D5).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct SplitModes {
    /// The speed, rad/s.
    pub speed: f64,
    /// The split modes, by `omega` ascending.
    pub modes: Vec<SplitMode>,
    /// All `2 m` roots of the reduced problem (empty without the option).
    pub roots: Vec<C64>,
    /// The real parts of the non-oscillating roots.
    pub real_roots: Vec<f64>,
    /// The flag of D4 (`None` without the option).
    pub stability: Option<Stability>,
    /// The basis used (`None` without the option).
    pub basis: Option<Basis>,
    /// The loss of stiffness of F-396 at this speed, if any.
    pub loss: Option<LossOfStiffness>,
    /// `max |a_ij - a_ji| / max |a|` of the projected `K_eff` and `M` before the symmetrisation (the larger).
    pub asymmetry: f64,
    /// `max |c_ij + c_ji| / max |c|` of the projected `C` before the skew-symmetrisation.
    pub skew_defect: f64,
    /// The number of basis vectors `m`.
    pub basis_size: usize,
}

/// One speed of a [`SplitCampbell`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct SplitSpeed {
    /// The speed, rad/s.
    pub speed: f64,
    /// The flag of D4 (`None` without the option).
    pub stability: Option<Stability>,
    /// The basis (`None` without the option).
    pub basis: Option<Basis>,
    /// The loss of stiffness of F-396, if any.
    pub loss: Option<LossOfStiffness>,
}

/// The split Campbell table (ADR 0472 D5): the tracked split modes over the speeds with their whirl numbers and directions, the
/// growth rate per track and speed, the stability per speed. Without the option `campbell.table` is the table of F-396.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct SplitCampbell {
    /// Whether the Coriolis term was included.
    pub coriolis: bool,
    /// The tracks (the table of F-396 and the whirl numbers and directions of ADR 0411 D4).
    pub campbell: WhirlCampbell,
    /// Per track and speed, the growth rate `Re lambda`, 1/s (`None` without the option or where the track has no mode).
    pub growth: Vec<Vec<Option<f64>>>,
    /// Per speed, the stability, the basis and the loss of stiffness.
    pub speeds: Vec<SplitSpeed>,
}

impl SplitCampbell {
    /// `(speed, growth)` of every unstable speed (ADR 0472 D4, clause c11).
    pub fn unstable(&self) -> Vec<(f64, f64)> {
        self.speeds
            .iter()
            .filter_map(|s| match s.stability {
                Some(Stability::Unstable { growth }) => Some((s.speed, growth)),
                _ => None,
            })
            .collect()
    }

    /// The CSV of [`WhirlCampbell::to_csv`] with the columns `stability` (`stable`, `unstable`, empty without the option)
    /// and `growth_1_s` (0 for a stable speed) appended.
    pub fn to_csv(&self) -> String {
        let base = self.campbell.to_csv();
        let mut s = String::new();
        for (j, line) in base.lines().enumerate() {
            s.push_str(line);
            if j == 0 {
                s.push_str(",stability,growth_1_s");
            } else {
                match self.speeds.get(j - 1).and_then(|p| p.stability) {
                    Some(Stability::Stable) => s.push_str(",stable,0"),
                    Some(Stability::Unstable { growth }) => {
                        let _ = write!(s, ",unstable,{growth}");
                    }
                    None => s.push_str(",,"),
                }
            }
            s.push('\n');
        }
        s
    }
}

fn check_not_cyclic(model: &SolidModel) -> Result<()> {
    if model
        .constraints()
        .iter()
        .any(|c| matches!(c, Constraint::Cyclic { .. }))
    {
        return Err(invalid(
            "cyclic",
            "the split modes on the modal subspace take no cyclic constraint (ADR 0472 D6); a sector's whirl modes with the \
             Coriolis term are RotatingSector::whirl_modes (ADR 0411) and its split modes on the modal subspace \
             RotatingSector::split_modes (ADR 0481)"
                .to_string(),
        ));
    }
    Ok(())
}

/// One F-396 rest mode against the Rayleigh quotient of its own vector (ADR 0484, F-554).
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct RestQuotient {
    /// The position in F-396's rest modes (ascending).
    pub index: usize,
    /// F-396's reported rest eigenvalue `lambda_i`, rad^2/s^2.
    pub eigenvalue: f64,
    /// `phi_i^T K0 phi_i / phi_i^T M phi_i`, `K0` the operator at rest.
    pub quotient: f64,
    /// `quotient / eigenvalue - 1`, signed.
    pub relative_difference: f64,
}

impl<'a> RotatingPart<'a> {
    /// The Rayleigh quotient of every F-396 rest mode against its reported eigenvalue (ADR 0484 D2): at zero speed the split
    /// frequencies of [`RotatingPart::split_modes`] equal the square roots of these quotients, not of the reported eigenvalues,
    /// whose accuracy is that of the eigensolver's stopping test. Read-only; available with or without the Coriolis option.
    /// Refuses a model with a cyclic constraint (`"cyclic"`).
    pub fn rest_quotients(&self) -> Result<Vec<RestQuotient>> {
        check_not_cyclic(self.model())?;
        let r = match self.modes(0.0)? {
            SpeedModes::Modes(r) => r,
            SpeedModes::LossOfStiffness(l) => {
                return Err(FemError::Solver {
                    solver: "rest_quotients",
                    reason: format!("no rest modes: {l:?}"),
                })
            }
        };
        let k = self.operator(0.0);
        let m = self.mass();
        let mut out = Vec::with_capacity(r.modes.len());
        for (index, phi) in r.modes.iter().enumerate() {
            let (mut kp, mut mp) = (vec![0.0; k.n], vec![0.0; m.n]);
            k.matvec(phi, &mut kp);
            m.matvec(phi, &mut mp);
            let num: f64 = phi.iter().zip(&kp).map(|(a, b)| a * b).sum();
            let den: f64 = phi.iter().zip(&mp).map(|(a, b)| a * b).sum();
            let quotient = num / den;
            let eigenvalue = r.eigenvalues[index];
            out.push(RestQuotient {
                index,
                eigenvalue,
                quotient,
                relative_difference: quotient / eigenvalue - 1.0,
            });
        }
        Ok(out)
    }
}

/// `Phi^T A Phi`, row-major `m x m`, not symmetrised.
fn project(a: &CsrMatrix, phi: &[Vec<f64>]) -> Vec<f64> {
    let m = phi.len();
    let mut out = vec![0.0; m * m];
    let mut y = vec![0.0; a.n];
    for j in 0..m {
        a.matvec(&phi[j], &mut y);
        for i in 0..m {
            out[i * m + j] = phi[i].iter().zip(&y).map(|(p, q)| p * q).sum();
        }
    }
    out
}

/// `max |a_ij - s a_ji| / max |a|` (0 for a zero matrix).
fn defect(a: &[f64], m: usize, s: f64) -> f64 {
    let big = a.iter().fold(0.0_f64, |x, v| x.max(v.abs()));
    if big == 0.0 {
        return 0.0;
    }
    let mut d: f64 = 0.0;
    for i in 0..m {
        for j in 0..m {
            d = d.max((a[i * m + j] - s * a[j * m + i]).abs());
        }
    }
    d / big
}

/// The projection of D2 on the basis `phi` (`K_r`, `M_r` symmetrised, `C_r` skew-symmetrised, the defects before that) and the
/// solve of D3 (ADR 0472; ADR 0481 D1 on the expanded harmonic modes with the doubled matrices).
fn project_and_solve(
    phi: &[Vec<f64>],
    keff: &CsrMatrix,
    mass: &CsrMatrix,
    c: &CsrMatrix,
    omega: f64,
) -> Result<(DenseSplit, f64, f64)> {
    let (d, asymmetry, skew_defect, _) = project_and_solve_with(phi, keff, mass, c, omega)?;
    Ok((d, asymmetry, skew_defect))
}

/// The symmetrised projected matrices `K_VV`, `M_VV`, `C_VV` (row-major) of [`project_and_solve_with`] (ADR 0492 D2).
struct Projected {
    k: Vec<f64>,
    m: Vec<f64>,
    c: Vec<f64>,
}

/// [`project_and_solve`] that also returns the symmetrised projected matrices (the same operations in the same order).
fn project_and_solve_with(
    phi: &[Vec<f64>],
    keff: &CsrMatrix,
    mass: &CsrMatrix,
    c: &CsrMatrix,
    omega: f64,
) -> Result<(DenseSplit, f64, f64, Projected)> {
    let m = phi.len();
    let kr = project(keff, phi);
    let mr = project(mass, phi);
    let cr = project(c, phi);
    let asymmetry = defect(&kr, m, 1.0).max(defect(&mr, m, 1.0));
    let skew_defect = defect(&cr, m, -1.0);
    let mut ks = vec![0.0; m * m];
    let mut ms = vec![0.0; m * m];
    let mut cs = vec![0.0; m * m];
    for i in 0..m {
        for j in 0..m {
            ks[i * m + j] = 0.5 * (kr[i * m + j] + kr[j * m + i]);
            ms[i * m + j] = 0.5 * (mr[i * m + j] + mr[j * m + i]);
            cs[i * m + j] = 0.5 * (cr[i * m + j] - cr[j * m + i]);
        }
    }
    let d = split_modes_dense(&ms, &cs, omega, &ks, m)?;
    Ok((d, asymmetry, skew_defect, Projected { k: ks, m: ms, c: cs }))
}

/// The split mode of a reduced root on the basis `phi`: the shape `Phi q` (ADR 0472 D4).
fn split_mode_of(phi: &[Vec<f64>], dm: &DenseSplitMode) -> SplitMode {
    let n = phi.first().map_or(0, Vec::len);
    let mut re = vec![0.0; n];
    let mut im = vec![0.0; n];
    for (p, x) in phi.iter().zip(&dm.q) {
        for e in 0..n {
            re[e] += p[e] * x.re;
            im[e] += p[e] * x.im;
        }
    }
    SplitMode {
        omega: dm.omega,
        eigenvalue: dm.eigenvalue,
        frequency: dm.omega / (2.0 * PI),
        growth: dm.growth,
        re,
        im,
        whirl: dm.whirl,
        direction: dm.direction,
        residual: dm.residual,
    }
}

impl<'a> RotatingPart<'a> {
    /// The split modes at the speed `omega`, rad/s (ADR 0472 D2 to D5). With `options.coriolis` the roots of the gyroscopic
    /// problem on the modal subspace (the speed basis, or the rest basis at a loss of stiffness) with the stability flag;
    /// without it the F-396 modes at the speed as real modes. Refuses a negative or non-finite speed (`"speed"`) and a model
    /// with a cyclic constraint (`"cyclic"`).
    pub fn split_modes(&self, omega: f64) -> Result<SplitModes> {
        check_speed(omega)?;
        check_not_cyclic(self.model())?;
        if !self.options().coriolis {
            return self.split_without(omega);
        }
        let c = self.coriolis()?;
        self.split_at(omega, &c)
    }

    fn split_without(&self, omega: f64) -> Result<SplitModes> {
        let (modes, loss, unknowns) = match self.modes(omega)? {
            SpeedModes::Modes(r) => {
                let modes = (0..r.eigenvalues.len())
                    .map(|i| {
                        let l = r.eigenvalues[i];
                        SplitMode {
                            omega: l.signum() * l.abs().sqrt(),
                            eigenvalue: l,
                            frequency: r.frequencies[i],
                            growth: 0.0,
                            re: r.modes[i].clone(),
                            im: vec![0.0; r.modes[i].len()],
                            whirl: 0.0,
                            direction: Whirl::Neutral,
                            residual: r.residuals[i],
                        }
                    })
                    .collect();
                (modes, None, r.eigenvalues.len())
            }
            SpeedModes::LossOfStiffness(l) => (Vec::new(), Some(l), 0),
        };
        Ok(SplitModes {
            speed: omega,
            modes,
            roots: Vec::new(),
            real_roots: Vec::new(),
            stability: None,
            basis: None,
            loss,
            asymmetry: 0.0,
            skew_defect: 0.0,
            basis_size: unknowns,
        })
    }

    fn split_at(&self, omega: f64, c: &CsrMatrix) -> Result<SplitModes> {
        let (basis, loss, r) = match self.modes(omega)? {
            SpeedModes::Modes(r) => (Basis::Speed, None, r),
            SpeedModes::LossOfStiffness(l) => match self.modes(0.0)? {
                SpeedModes::Modes(r) => (Basis::Rest, Some(l), r),
                SpeedModes::LossOfStiffness(l0) => {
                    return Err(FemError::Solver {
                        solver: "split_modes",
                        reason: format!("no rest basis at {} rad/s: {l0:?}", l.speed),
                    })
                }
            },
        };
        let phi = &r.modes;
        let keff = self.operator(omega);
        let (d, asymmetry, skew_defect) = project_and_solve(phi, &keff, self.mass(), c, omega)?;
        let m = phi.len();
        let modes = d.modes.iter().map(|dm| split_mode_of(phi, dm)).collect();
        Ok(SplitModes {
            speed: omega,
            modes,
            roots: d.roots,
            real_roots: d.real_roots,
            stability: Some(d.stability),
            basis: Some(basis),
            loss,
            asymmetry,
            skew_defect,
            basis_size: m,
        })
    }

    /// The split Campbell table over a list of speeds, rad/s (finite, `>= 0`, strictly increasing; key `"speeds"`) (ADR 0472
    /// D5). Without `options.coriolis` the table of [`RotatingPart::campbell`] itself; with it the split modes of every speed
    /// tracked by the complex-line tracker of ADR 0411 D4, with the growth per track and the stability per speed. Refuses a
    /// model with a cyclic constraint (`"cyclic"`).
    pub fn split_campbell(&self, speeds: &[f64]) -> Result<SplitCampbell> {
        check_speeds(speeds)?;
        check_not_cyclic(self.model())?;
        let ns = speeds.len();
        if !self.options().coriolis {
            let table = self.campbell(speeds)?;
            let nt = table.tracks.len();
            let per = SplitSpeedList::without(speeds, &table.losses);
            return Ok(SplitCampbell {
                coriolis: false,
                campbell: WhirlCampbell {
                    table,
                    whirl: vec![vec![None; ns]; nt],
                    directions: vec![vec![None; ns]; nt],
                    waves: vec![vec![None; ns]; nt],
                },
                growth: vec![vec![None; ns]; nt],
                speeds: per,
            });
        }
        let c = self.coriolis()?;
        let mut results = Vec::with_capacity(ns);
        let mut entries = Vec::with_capacity(ns);
        for &w in speeds {
            let s = self.split_at(w, &c)?;
            let modes: Vec<WhirlMode> = s
                .modes
                .iter()
                .map(|sm| WhirlMode {
                    omega: sm.omega,
                    eigenvalue: sm.eigenvalue,
                    frequency: sm.frequency,
                    re: sm.re.clone(),
                    im: sm.im.clone(),
                    whirl: sm.whirl,
                    direction: sm.direction,
                    wave: None,
                    residual: sm.residual,
                })
                .collect();
            entries.push(Entry::Modes(WhirlModes {
                frequencies_all: modes.iter().map(|m| m.omega).collect(),
                modes,
                unknowns: s.basis_size,
            }));
            results.push(s);
        }
        let campbell = track_whirl(speeds, &entries, self.mass());
        let n = self.mass().n;
        let growth = campbell
            .table
            .tracks
            .iter()
            .map(|tr| {
                (0..ns)
                    .map(|j| {
                        let line = tr.modes[j].as_ref()?;
                        results[j]
                            .modes
                            .iter()
                            .find(|sm| line[..n] == sm.re[..] && line[n..] == sm.im[..])
                            .map(|sm| sm.growth)
                    })
                    .collect()
            })
            .collect();
        let per = results
            .iter()
            .map(|s| SplitSpeed {
                speed: s.speed,
                stability: s.stability,
                basis: s.basis,
                loss: s.loss,
            })
            .collect();
        Ok(SplitCampbell {
            coriolis: true,
            campbell,
            growth,
            speeds: per,
        })
    }
}

struct SplitSpeedList;

impl SplitSpeedList {
    fn without(speeds: &[f64], losses: &[LossOfStiffness]) -> Vec<SplitSpeed> {
        speeds
            .iter()
            .map(|&w| SplitSpeed {
                speed: w,
                stability: None,
                basis: None,
                loss: losses.iter().find(|l| l.speed.to_bits() == w.to_bits()).copied(),
            })
            .collect()
    }
}

// ------------------------------------------------------------------------------------------------------------------------
// ADR 0481 (F-555): the split modes of a cyclic sector on the harmonic modes of each index.

/// The split modes of one harmonic index of a sector at one speed (ADR 0481 D1, D4).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct HarmonicSplitModes {
    /// The harmonic index `k`.
    pub harmonic: usize,
    /// 1 for `k = 0` and `2 k = N`, 2 otherwise.
    pub multiplicity: usize,
    /// The split modes of the index; for a pair each shape is the cosine part then the sine part (length `2 n`).
    pub modes: SplitModes,
    /// Per split mode, the wave sense `tau` of ADR 0411 D5 for a pair (`None` for a single index).
    pub waves: Vec<Option<f64>>,
    /// The static correction vectors kept in the basis (ADR 0482 D1; 0 without the option).
    pub corrections: usize,
    /// The correction vectors dropped as dependent (ADR 0482 D1 step 4; 0 without the option).
    pub dropped_corrections: usize,
    /// Per split mode, the first-order Ritz error `eta` (ADR 0482 D2): `None` without the option, at an unstable speed or when
    /// its solve did not converge.
    pub eta: Vec<Option<f64>>,
    /// Per split mode, the Ritz error of the Schur-complemented projected problem `|theta/theta' - 1|` (ADR 0492 D2): `Some`
    /// where `eta` is computed by [`RotatingSector::split_modes`] (not by `split_campbell`), `None` otherwise or when one of
    /// its solves failed; `Some(0)` when the basis spans the index.
    pub schur_eta: Vec<Option<f64>>,
}

/// The split Campbell table of a sector (ADR 0481 D2): one [`SplitCampbell`] per harmonic index.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct SectorSplitCampbell {
    /// The number of sectors `N`.
    pub sectors: usize,
    /// The harmonic indices, increasing.
    pub harmonics: Vec<usize>,
    /// The multiplicity of each index.
    pub multiplicities: Vec<usize>,
    /// Whether the Coriolis term was included.
    pub coriolis: bool,
    /// The families, one per index; without the option each table is the family table of [`RotatingSector::campbell`].
    pub families: Vec<SplitCampbell>,
}

impl SectorSplitCampbell {
    /// `(harmonic, speed, growth)` of every unstable index and speed.
    pub fn unstable(&self) -> Vec<(usize, f64, f64)> {
        self.harmonics
            .iter()
            .zip(&self.families)
            .flat_map(|(&k, f)| f.unstable().into_iter().map(move |(s, g)| (k, s, g)))
            .collect()
    }
}

fn harmonic_refused(harmonic: usize, sectors: usize) -> FemError {
    invalid(
        "harmonic",
        format!(
            "harmonic index {harmonic} is above N/2 for a rotor of {sectors} sectors (the largest is {})",
            sectors / 2
        ),
    )
}

/// The wave sense `tau = 2 (a_r^T M b_i - a_i^T M b_r) / (|a|_M^2 + |b|_M^2)` of a pair mode (ADR 0411 D5).
fn wave_sense(m: &CsrMatrix, re: &[f64], im: &[f64]) -> f64 {
    let n = m.n;
    let bil = |x: &[f64], y: &[f64]| -> f64 {
        let mut t = vec![0.0; n];
        m.matvec(y, &mut t);
        x.iter().zip(&t).map(|(p, q)| p * q).sum::<f64>()
    };
    let (ar, br) = (&re[..n], &re[n..]);
    let (ai, bi) = (&im[..n], &im[n..]);
    let num = 2.0 * (bil(ar, bi) - bil(ai, br));
    let den = bil(ar, ar) + bil(ai, ai) + bil(br, br) + bil(bi, bi);
    num / den
}

/// The expanded basis of an index: `(cos, sin)` for a pair, `cos` for a single index.
fn sector_basis(h: &HarmonicModes) -> Vec<Vec<f64>> {
    h.modes
        .iter()
        .map(|m| {
            if h.multiplicity == 2 {
                m.cos.iter().chain(&m.sin).copied().collect()
            } else {
                m.cos.clone()
            }
        })
        .collect()
}

impl<'a> RotatingSector<'a> {
    /// The split modes of harmonic index `harmonic` at the speed `omega`, rad/s (ADR 0481 D1, D2). With
    /// `options.coriolis` the gyroscopic problem of ADR 0472 on the F-396 harmonic modes of the index (the speed basis
    /// [`RotatingSector::modes`], or the rest basis at a loss of stiffness of the index), with `K_eff`, `M` and the Coriolis
    /// matrix doubled for a pair; without it the F-396 modes of the index as real split modes. A sector exists only for
    /// materials whose axes pass the check of ADR 0462 (D3). Refuses a harmonic above `N / 2` (`"harmonic"`) and a negative
    /// or non-finite speed (`"speed"`).
    pub fn split_modes(&self, harmonic: usize, omega: f64) -> Result<HarmonicSplitModes> {
        check_speed(omega)?;
        if 2 * harmonic > self.sectors() {
            return Err(harmonic_refused(harmonic, self.sectors()));
        }
        if !self.part().options().coriolis {
            return self.sector_split_without(harmonic, omega);
        }
        let c = self.part().coriolis()?;
        self.sector_split_at(harmonic, omega, &c, true)
    }

    fn sector_split_without(&self, harmonic: usize, omega: f64) -> Result<HarmonicSplitModes> {
        let multiplicity = if harmonic == 0 || 2 * harmonic == self.sectors() {
            1
        } else {
            2
        };
        let (modes, loss) = match self.modes(harmonic, omega)? {
            HarmonicSpeedModes::Modes(h) => {
                let basis = sector_basis(&h);
                let modes: Vec<SplitMode> = basis
                    .into_iter()
                    .enumerate()
                    .map(|(i, re)| {
                        let l = h.eigenvalues[i];
                        SplitMode {
                            omega: l.signum() * l.abs().sqrt(),
                            eigenvalue: l,
                            frequency: crate::modal::frequency(l),
                            growth: 0.0,
                            im: vec![0.0; re.len()],
                            re,
                            whirl: 0.0,
                            direction: Whirl::Neutral,
                            residual: h.residuals[i],
                        }
                    })
                    .collect();
                (modes, None)
            }
            HarmonicSpeedModes::LossOfStiffness(l) => (Vec::new(), Some(l.loss)),
        };
        let waves = vec![if multiplicity == 2 { Some(0.0) } else { None }; modes.len()];
        let count = modes.len();
        Ok(HarmonicSplitModes {
            harmonic,
            multiplicity,
            modes: SplitModes {
                speed: omega,
                modes,
                roots: Vec::new(),
                real_roots: Vec::new(),
                stability: None,
                basis: None,
                loss,
                asymmetry: 0.0,
                skew_defect: 0.0,
                basis_size: count,
            },
            waves,
            corrections: 0,
            dropped_corrections: 0,
            eta: vec![None; count],
            schur_eta: vec![None; count],
        })
    }

    /// `schur`: whether the Ritz error of the Schur-complemented problem is computed (ADR 0492 D2; not in `split_campbell`).
    fn sector_split_at(&self, harmonic: usize, omega: f64, c: &CsrMatrix, schur: bool) -> Result<HarmonicSplitModes> {
        let (basis, loss, h) = match self.modes(harmonic, omega)? {
            HarmonicSpeedModes::Modes(h) => (Basis::Speed, None, h),
            HarmonicSpeedModes::LossOfStiffness(l) => match self.modes(harmonic, 0.0)? {
                HarmonicSpeedModes::Modes(h) => (Basis::Rest, Some(l.loss), h),
                HarmonicSpeedModes::LossOfStiffness(l0) => {
                    return Err(FemError::Solver {
                        solver: "split_modes",
                        reason: format!("no rest basis of harmonic {harmonic} at {} rad/s: {l0:?}", l.loss.speed),
                    })
                }
            },
        };
        let multiplicity = h.multiplicity;
        let mut phi = sector_basis(&h);
        let part = self.part();
        let correction = if part.options().split_correction {
            let cb = correction_basis(&h, &phi, c)?;
            phi.extend(cb.expanded.iter().cloned());
            Some(cb)
        } else {
            None
        };
        let keff = part.operator(omega);
        let (d, asymmetry, skew_defect, projected) = if multiplicity == 2 {
            project_and_solve_with(
                &phi,
                &doubled_matrix(&keff),
                &doubled_matrix(part.mass()),
                &doubled_matrix(c),
                omega,
            )?
        } else {
            project_and_solve_with(&phi, &keff, part.mass(), c, omega)?
        };
        let modes: Vec<SplitMode> = d.modes.iter().map(|dm| split_mode_of(&phi, dm)).collect();
        let waves = modes
            .iter()
            .map(|sm| (multiplicity == 2).then(|| wave_sense(part.mass(), &sm.re, &sm.im)))
            .collect();
        let (corrections, dropped_corrections, eta, schur_eta) = match &correction {
            None => (0, 0, vec![None; modes.len()], vec![None; modes.len()]),
            Some(cb) => {
                let (eta, schur_eta) = if d.stability == Stability::Stable {
                    let rest = match basis {
                        Basis::Speed => None,
                        Basis::Rest => {
                            let k2 = if multiplicity == 2 {
                                doubled_matrix(&keff)
                            } else {
                                keff.clone()
                            };
                            let zero = vec![0.0; h.transformation.n_unknowns()];
                            Some(h.transformation.reduce(&k2, &zero)?.0)
                        }
                    };
                    let kq = rest.as_ref().unwrap_or(&h.reduced_stiffness);
                    let eta: Vec<Option<f64>> = d
                        .modes
                        .iter()
                        .map(|dm| ritz_error(cb, kq, &h.reduced_mass, omega, dm))
                        .collect();
                    let schur_eta = if schur {
                        d.modes
                            .iter()
                            .map(|dm| schur_error(cb, kq, &h.reduced_mass, omega, dm, &projected))
                            .collect()
                    } else {
                        vec![None; modes.len()]
                    };
                    (eta, schur_eta)
                } else {
                    (vec![None; modes.len()], vec![None; modes.len()])
                };
                (cb.kept(), cb.dropped, eta, schur_eta)
            }
        };
        Ok(HarmonicSplitModes {
            harmonic,
            multiplicity,
            modes: SplitModes {
                speed: omega,
                modes,
                roots: d.roots,
                real_roots: d.real_roots,
                stability: Some(d.stability),
                basis: Some(basis),
                loss,
                asymmetry,
                skew_defect,
                basis_size: phi.len(),
            },
            waves,
            corrections,
            dropped_corrections,
            eta,
            schur_eta,
        })
    }

    /// The split Campbell table over the harmonic indices `harmonics` (as [`RotatingSector::campbell`]; key `"harmonics"`)
    /// and the speeds `speeds` (key `"speeds"`) (ADR 0481 D2): without `options.coriolis` each family's table is the table of
    /// [`RotatingSector::campbell`] itself; with it the split modes of every index and speed ([`RotatingSector::split_modes`])
    /// tracked per index by the complex-line tracker of ADR 0411 D4, with the growth per track and the stability per speed.
    pub fn split_campbell(&self, harmonics: &[usize], speeds: &[f64]) -> Result<SectorSplitCampbell> {
        check_harmonics(harmonics, self.sectors())?;
        check_speeds(speeds)?;
        let sectors = self.sectors();
        let multiplicities: Vec<usize> = harmonics
            .iter()
            .map(|&k| if k == 0 || 2 * k == sectors { 1 } else { 2 })
            .collect();
        let ns = speeds.len();
        if !self.part().options().coriolis {
            let cc = self.campbell(harmonics, speeds)?;
            let families = cc
                .families
                .into_iter()
                .map(|f| {
                    let table = f.table;
                    let nt = table.tracks.len();
                    let per = SplitSpeedList::without(speeds, &table.losses);
                    SplitCampbell {
                        coriolis: false,
                        campbell: WhirlCampbell {
                            table,
                            whirl: vec![vec![None; ns]; nt],
                            directions: vec![vec![None; ns]; nt],
                            waves: vec![vec![None; ns]; nt],
                        },
                        growth: vec![vec![None; ns]; nt],
                        speeds: per,
                    }
                })
                .collect();
            return Ok(SectorSplitCampbell {
                sectors,
                harmonics: harmonics.to_vec(),
                multiplicities,
                coriolis: false,
                families,
            });
        }
        let c = self.part().coriolis()?;
        let mut families = Vec::with_capacity(harmonics.len());
        for &k in harmonics {
            let mut results = Vec::with_capacity(ns);
            let mut entries = Vec::with_capacity(ns);
            for &w in speeds {
                let s = self.sector_split_at(k, w, &c, false)?;
                let modes: Vec<WhirlMode> = s
                    .modes
                    .modes
                    .iter()
                    .zip(&s.waves)
                    .map(|(sm, wave)| WhirlMode {
                        omega: sm.omega,
                        eigenvalue: sm.eigenvalue,
                        frequency: sm.frequency,
                        re: sm.re.clone(),
                        im: sm.im.clone(),
                        whirl: sm.whirl,
                        direction: sm.direction,
                        wave: *wave,
                        residual: sm.residual,
                    })
                    .collect();
                entries.push(Entry::Modes(WhirlModes {
                    frequencies_all: modes.iter().map(|m| m.omega).collect(),
                    modes,
                    unknowns: s.modes.basis_size,
                }));
                results.push(s.modes);
            }
            let campbell = track_whirl(speeds, &entries, self.part().mass());
            let growth = campbell
                .table
                .tracks
                .iter()
                .map(|tr| {
                    (0..ns)
                        .map(|j| {
                            let line = tr.modes[j].as_ref()?;
                            results[j]
                                .modes
                                .iter()
                                .find(|sm| {
                                    let w = sm.re.len();
                                    line.len() == 2 * w && line[..w] == sm.re[..] && line[w..] == sm.im[..]
                                })
                                .map(|sm| sm.growth)
                        })
                        .collect()
                })
                .collect();
            let per = results
                .iter()
                .map(|s| SplitSpeed {
                    speed: s.speed,
                    stability: s.stability,
                    basis: s.basis,
                    loss: s.loss,
                })
                .collect();
            families.push(SplitCampbell {
                coriolis: true,
                campbell,
                growth,
                speeds: per,
            });
        }
        Ok(SectorSplitCampbell {
            sectors,
            harmonics: harmonics.to_vec(),
            multiplicities,
            coriolis: true,
            families,
        })
    }
}

// ------------------------------------------------------------------------------------------------------------------------
// ADR 0482 (F-569): static correction vectors of a sector index and the Ritz error of every split mode.

/// The relative tolerance of the MINRES solve of the Ritz error (ADR 0482 D2): the preconditioned residual estimate at most
/// this times its start value.
pub const RITZ_TOLERANCE: f64 = 1e-10;

/// The cap of the MINRES iterations (ADR 0482 D2; the count is `min(4 n_q + 20, RITZ_ITERATIONS)`).
const RITZ_ITERATIONS: usize = 2000;

/// The enlarged basis of an index in reduced coordinates (ADR 0482 D1).
struct CorrectionBasis {
    /// The modes then the kept correction vectors, `M_r`-orthonormal (reduced coordinates).
    v: Vec<Vec<f64>>,
    /// `M_r v_j`.
    mv: Vec<Vec<f64>>,
    /// The number of modes `m`.
    modes: usize,
    /// The kept correction vectors expanded by `T_k` (the length of the expanded basis vectors).
    expanded: Vec<Vec<f64>>,
    /// The vectors dropped by the rule of ADR 0464.
    dropped: usize,
    /// `T_k^T C_2 T_k`.
    cr: CsrMatrix,
    /// The factor of the basis stiffness `K_b` (the preconditioner of D2).
    ldlt: SparseLdlt,
}

fn dot(x: &[f64], y: &[f64]) -> f64 {
    x.iter().zip(y).map(|(a, b)| a * b).sum()
}

impl CorrectionBasis {
    fn kept(&self) -> usize {
        self.v.len() - self.modes
    }

    /// `(I - P)^T x = x - M V V^T x` on one half.
    fn project_dual(&self, x: &mut [f64]) {
        for (vj, mj) in self.v.iter().zip(&self.mv) {
            let a = dot(vj, x);
            for (xi, mi) in x.iter_mut().zip(mj) {
                *xi -= a * mi;
            }
        }
    }

    /// `(I - P) x = x - V V^T M x` on one half.
    fn project(&self, x: &mut [f64]) {
        for (vj, mj) in self.v.iter().zip(&self.mv) {
            let a = dot(mj, x);
            for (xi, vi) in x.iter_mut().zip(vj) {
                *xi -= a * vi;
            }
        }
    }
}

/// D1: the static correction vectors `K_b^-1 C_r q_a` of the modes of `h` (expanded basis `phi`), `M_r`-orthonormalised against
/// the modes and the vectors kept before them by [`m_orthonormalise`] (dropped by its rule).
fn correction_basis(h: &HarmonicModes, phi: &[Vec<f64>], c: &CsrMatrix) -> Result<CorrectionBasis> {
    let tr = &h.transformation;
    if tr.n_unknowns() != tr.n_mesh() {
        return Err(invalid(
            "constraint",
            "the static correction vectors of the split modes take no rigid set in the sector (the expanded basis does not \
             carry the reference unknowns; ADR 0482 D1)"
                .to_string(),
        ));
    }
    let ind = tr.independent();
    let mut v: Vec<Vec<f64>> = phi.iter().map(|x| ind.iter().map(|&i| x[i]).collect()).collect();
    let modes = v.len();
    let c2 = if h.multiplicity == 2 {
        doubled_matrix(c)
    } else {
        c.clone()
    };
    let zero = vec![0.0; tr.n_unknowns()];
    let (cr, _) = tr.reduce(&c2, &zero)?;
    let ldlt = SparseLdlt::factor(&h.reduced_stiffness).map_err(|e| FemError::Solver {
        solver: "sparse_ldlt",
        reason: e.to_string(),
    })?;
    let nq = cr.n;
    let mut f = vec![0.0; nq];
    let mut y = vec![0.0; nq];
    let mut expanded = Vec::new();
    let mut dropped = 0;
    for a in 0..modes {
        cr.matvec(&v[a], &mut f);
        let mut z = ldlt.solve(&f);
        let keep = {
            let basis: Vec<&[f64]> = v.iter().map(Vec::as_slice).collect();
            m_orthonormalise(&h.reduced_mass, &mut z, &basis, &mut y)
        };
        if keep {
            expanded.push(tr.expand(&z));
            v.push(z);
        } else {
            dropped += 1;
        }
    }
    let mv = v
        .iter()
        .map(|x| {
            let mut t = vec![0.0; nq];
            h.reduced_mass.matvec(x, &mut t);
            t
        })
        .collect();
    Ok(CorrectionBasis {
        v,
        mv,
        modes,
        expanded,
        dropped,
        cr,
        ldlt,
    })
}

/// D2: the Ritz error `eta` of the split mode `dm` (coordinates on the basis `cb.v`) at the speed `speed`, `kq` the reduced
/// `K_eff` at the speed, `mr` the reduced mass. `Some(0)` when the basis spans the index; `None` when the MINRES solve did not
/// converge.
fn ritz_error(cb: &CorrectionBasis, kq: &CsrMatrix, mr: &CsrMatrix, speed: f64, dm: &DenseSplitMode) -> Option<f64> {
    let nq = kq.n;
    if cb.v.len() >= nq {
        return Some(0.0);
    }
    let theta = dm.omega;
    let g = 2.0 * speed;
    let mut pr = vec![0.0; nq];
    let mut pi = vec![0.0; nq];
    for (vj, x) in cb.v.iter().zip(&dm.q) {
        for e in 0..nq {
            pr[e] += vj[e] * x.re;
            pi[e] += vj[e] * x.im;
        }
    }
    let mut tk = vec![0.0; nq];
    let mut tm = vec![0.0; nq];
    let mut tc = vec![0.0; nq];
    // The real form of Q(theta): (A u_r - theta g C u_i, A u_i + theta g C u_r), A = K - theta^2 M.
    let mut qapply = |u: &[f64], out: &mut [f64]| {
        let (ur, ui) = u.split_at(nq);
        let (or, oi) = out.split_at_mut(nq);
        kq.matvec(ur, &mut tk);
        mr.matvec(ur, &mut tm);
        cb.cr.matvec(ui, &mut tc);
        for e in 0..nq {
            or[e] = tk[e] - theta * theta * tm[e] - theta * g * tc[e];
        }
        kq.matvec(ui, &mut tk);
        mr.matvec(ui, &mut tm);
        cb.cr.matvec(ur, &mut tc);
        for e in 0..nq {
            oi[e] = tk[e] - theta * theta * tm[e] + theta * g * tc[e];
        }
    };
    let psi: Vec<f64> = pr.iter().chain(&pi).copied().collect();
    let mut r = vec![0.0; 2 * nq];
    qapply(&psi, &mut r);
    let mut b: Vec<f64> = r.iter().map(|x| -x).collect();
    cb.project_dual(&mut b[..nq]);
    cb.project_dual(&mut b[nq..]);
    let mut work = vec![0.0; 2 * nq];
    let op = |u: &[f64], out: &mut [f64]| {
        work.copy_from_slice(u);
        cb.project(&mut work[..nq]);
        cb.project(&mut work[nq..]);
        qapply(&work, out);
        cb.project_dual(&mut out[..nq]);
        cb.project_dual(&mut out[nq..]);
    };
    let prec = |x: &[f64], out: &mut [f64]| {
        for half in 0..2 {
            let mut h = x[half * nq..(half + 1) * nq].to_vec();
            cb.project_dual(&mut h);
            let mut z = cb.ldlt.solve(&h);
            cb.project(&mut z);
            out[half * nq..(half + 1) * nq].copy_from_slice(&z);
        }
    };
    let maxit = (4 * nq + 20).min(RITZ_ITERATIONS);
    let t = minres(op, prec, &b, RITZ_TOLERANCE, maxit)?;
    let num = dot(&t, &b).abs();
    let mut mp = vec![0.0; nq];
    mr.matvec(&pr, &mut mp);
    let mut quad_m = dot(&pr, &mp);
    mr.matvec(&pi, &mut mp);
    quad_m += dot(&pi, &mp);
    cb.cr.matvec(&pi, &mut mp);
    let gyro = g * dot(&pr, &mp);
    let den = theta * (2.0 * theta * quad_m + 2.0 * gyro).abs();
    let eta = num / den;
    eta.is_finite().then_some(eta)
}

/// MINRES with a positive semi-definite preconditioner (Paige and Saunders 1975, from memory; ADR 0482 D2) for the symmetric
/// `op`, from `x = 0`; `None` when the preconditioned residual estimate has not fallen to `tol` times its start in `maxit`
/// iterations or the preconditioned inner product is negative or not finite.
fn minres(
    mut op: impl FnMut(&[f64], &mut [f64]),
    mut prec: impl FnMut(&[f64], &mut [f64]),
    b: &[f64],
    tol: f64,
    maxit: usize,
) -> Option<Vec<f64>> {
    let n = b.len();
    let mut x = vec![0.0; n];
    let mut r1 = b.to_vec();
    let mut y = vec![0.0; n];
    prec(&r1, &mut y);
    let beta1_sq = dot(&r1, &y);
    if !(beta1_sq >= 0.0 && beta1_sq.is_finite()) {
        return None;
    }
    if beta1_sq == 0.0 {
        return Some(x);
    }
    let beta1 = beta1_sq.sqrt();
    let mut r2 = r1.clone();
    let mut oldb = 0.0;
    let mut beta = beta1;
    let mut dbar = 0.0;
    let mut epsln = 0.0;
    let mut phibar = beta1;
    let mut cs = -1.0;
    let mut sn = 0.0;
    let mut w = vec![0.0; n];
    let mut w2 = vec![0.0; n];
    let mut v = vec![0.0; n];
    for itn in 1..=maxit {
        let s = 1.0 / beta;
        for (vi, yi) in v.iter_mut().zip(&y) {
            *vi = s * yi;
        }
        op(&v, &mut y);
        if itn >= 2 {
            let f = beta / oldb;
            for (yi, ri) in y.iter_mut().zip(&r1) {
                *yi -= f * ri;
            }
        }
        let alfa = dot(&v, &y);
        let f = alfa / beta;
        for (yi, ri) in y.iter_mut().zip(&r2) {
            *yi -= f * ri;
        }
        std::mem::swap(&mut r1, &mut r2);
        r2.copy_from_slice(&y);
        prec(&r2, &mut y);
        oldb = beta;
        let beta_sq = dot(&r2, &y);
        if !(beta_sq >= 0.0 && beta_sq.is_finite()) {
            return None;
        }
        beta = beta_sq.sqrt();
        let oldeps = epsln;
        let delta = cs * dbar + sn * alfa;
        let gbar = sn * dbar - cs * alfa;
        epsln = sn * beta;
        dbar = -cs * beta;
        let gamma = gbar.hypot(beta).max(f64::EPSILON);
        cs = gbar / gamma;
        sn = beta / gamma;
        let phi = cs * phibar;
        phibar *= sn;
        let denom = 1.0 / gamma;
        for i in 0..n {
            let w1 = w2[i];
            w2[i] = w[i];
            w[i] = (v[i] - oldeps * w1 - delta * w2[i]) * denom;
            x[i] += phi * w[i];
        }
        if phibar <= tol * beta1 {
            return Some(x);
        }
        if beta == 0.0 {
            return None;
        }
    }
    None
}

// ------------------------------------------------------------------------------------------------------------------------
// ADR 0492 (F-575): the Ritz error of the Schur-complemented projected problem.

/// The real form of `Q(theta) u` (ADR 0482 D2): `(A u_r - theta g C u_i, A u_i + theta g C u_r)`, `A = K - theta^2 M`; `t` three
/// scratch vectors of the half length.
#[allow(clippy::too_many_arguments)]
fn shifted_apply(
    k: &CsrMatrix,
    m: &CsrMatrix,
    c: &CsrMatrix,
    theta: f64,
    g: f64,
    u: &[f64],
    out: &mut [f64],
    t: &mut [Vec<f64>; 3],
) {
    let nq = k.n;
    let (ur, ui) = u.split_at(nq);
    let (or, oi) = out.split_at_mut(nq);
    let [tk, tm, tc] = t;
    k.matvec(ur, tk);
    m.matvec(ur, tm);
    c.matvec(ui, tc);
    for e in 0..nq {
        or[e] = tk[e] - theta * theta * tm[e] - theta * g * tc[e];
    }
    k.matvec(ui, tk);
    m.matvec(ui, tm);
    c.matvec(ur, tc);
    for e in 0..nq {
        oi[e] = tk[e] - theta * theta * tm[e] + theta * g * tc[e];
    }
}

/// ADR 0492 D2: the Ritz error `|theta/theta' - 1|` of the split mode `dm` (coordinates on `cb.v`), `theta'` the root near
/// `theta` of the projected problem condensed by the complement and frozen at `theta`: for each basis vector `v_j` the
/// complement solve `(I - P)^T Q(theta) (I - P) x_j = -(I - P)^T Q(theta) v_j` (the MINRES of ADR 0482 D2), `E_ij = -v_i^T
/// Q(theta) x_j`, then the dense quadratic problem with `K_VV - sym(Re E)` and `C_VV - skew(Im E)/(2 speed theta)` on the
/// projected matrices `p`; `theta'` the root of largest overlap with `dm`. `Some(0)` when the basis spans the index; `None` when a
/// solve failed or the value is not finite.
fn schur_error(
    cb: &CorrectionBasis,
    kq: &CsrMatrix,
    mr: &CsrMatrix,
    speed: f64,
    dm: &DenseSplitMode,
    p: &Projected,
) -> Option<f64> {
    let nq = kq.n;
    let nb = cb.v.len();
    if nb >= nq {
        return Some(0.0);
    }
    let theta = dm.omega;
    let g = 2.0 * speed;
    let mut scratch = [vec![0.0; nq], vec![0.0; nq], vec![0.0; nq]];
    let mut op_scratch = [vec![0.0; nq], vec![0.0; nq], vec![0.0; nq]];
    let mut work = vec![0.0; 2 * nq];
    let mut op = |u: &[f64], out: &mut [f64]| {
        work.copy_from_slice(u);
        cb.project(&mut work[..nq]);
        cb.project(&mut work[nq..]);
        shifted_apply(kq, mr, &cb.cr, theta, g, &work, out, &mut op_scratch);
        cb.project_dual(&mut out[..nq]);
        cb.project_dual(&mut out[nq..]);
    };
    let mut prec = |x: &[f64], out: &mut [f64]| {
        for half in 0..2 {
            let mut h = x[half * nq..(half + 1) * nq].to_vec();
            cb.project_dual(&mut h);
            let mut z = cb.ldlt.solve(&h);
            cb.project(&mut z);
            out[half * nq..(half + 1) * nq].copy_from_slice(&z);
        }
    };
    let maxit = (4 * nq + 20).min(RITZ_ITERATIONS);
    let mut er = vec![0.0; nb * nb];
    let mut ei = vec![0.0; nb * nb];
    let mut u = vec![0.0; 2 * nq];
    let mut qx = vec![0.0; 2 * nq];
    for j in 0..nb {
        u[..nq].copy_from_slice(&cb.v[j]);
        u[nq..].fill(0.0);
        shifted_apply(kq, mr, &cb.cr, theta, g, &u, &mut qx, &mut scratch);
        let mut b: Vec<f64> = qx.iter().map(|x| -x).collect();
        cb.project_dual(&mut b[..nq]);
        cb.project_dual(&mut b[nq..]);
        let mut x = minres(&mut op, &mut prec, &b, RITZ_TOLERANCE, maxit)?;
        cb.project(&mut x[..nq]);
        cb.project(&mut x[nq..]);
        shifted_apply(kq, mr, &cb.cr, theta, g, &x, &mut qx, &mut scratch);
        for (i, vi) in cb.v.iter().enumerate() {
            er[i * nb + j] = -dot(vi, &qx[..nq]);
            ei[i * nb + j] = -dot(vi, &qx[nq..]);
        }
    }
    let mut k2 = p.k.clone();
    let mut c2 = p.c.clone();
    for i in 0..nb {
        for j in 0..nb {
            k2[i * nb + j] -= 0.5 * (er[i * nb + j] + er[j * nb + i]);
            if speed > 0.0 {
                c2[i * nb + j] -= 0.5 * (ei[i * nb + j] - ei[j * nb + i]) / (g * theta);
            }
        }
    }
    let corrected = split_modes_dense(&p.m, &c2, speed, &k2, nb).ok()?;
    let mut best: Option<(f64, f64)> = None;
    for cm in &corrected.modes {
        // y^H M y' with y = dm.q, y' = cm.q.
        let mut re = 0.0;
        let mut im = 0.0;
        for i in 0..nb {
            let (yr, yi) = (dm.q[i].re, dm.q[i].im);
            for j in 0..nb {
                let mij = p.m[i * nb + j];
                re += mij * (yr * cm.q[j].re + yi * cm.q[j].im);
                im += mij * (yr * cm.q[j].im - yi * cm.q[j].re);
            }
        }
        let overlap = re * re + im * im;
        let better = match best {
            None => true,
            Some((_, o)) => overlap > o,
        };
        if better {
            best = Some((cm.omega, overlap));
        }
    }
    let (theta_c, _) = best?;
    let v = (theta / theta_c - 1.0).abs();
    v.is_finite().then_some(v)
}
