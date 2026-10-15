//! Reactor networks and parameter sensitivity (ADR 0027 D4, roadmap F-225): brute-force logarithmic sensitivity of a scalar
//! outcome to the pre-exponential factors, and a perfectly stirred reactor feeding a plug-flow reactor.
//! Shock tubes and premixed flame-speed libraries are not delivered.

use crate::mech::Mechanism;
use crate::reactors::{psr_steady, run_plug_flow, Energy, Psr, ReactorError, Tol, Trajectory};

/// Logarithmic sensitivity `d ln(outcome) / d ln(A_r)` of reaction `reaction`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sensitivity {
    /// Reaction index.
    pub reaction: usize,
    /// Normalised sensitivity coefficient.
    pub coefficient: f64,
}

/// Multiply the pre-exponential factor of reaction `r` by `factor` (the low-pressure limit of a falloff reaction and an
/// explicit reverse expression are scaled as well, so the whole rate scales).
pub fn scale_reaction(mech: &mut Mechanism, r: usize, factor: f64) {
    let rx = &mut mech.reactions[r];
    rx.arr[0] *= factor;
    if let Some(l) = rx.low.as_mut() {
        l[0] *= factor;
    }
    if let Some(v) = rx.rev.as_mut() {
        v[0] *= factor;
    }
}

/// Central-difference logarithmic sensitivities of `outcome` (a positive scalar such as an ignition delay) to every reaction's
/// pre-exponential factor, with relative step `rel_step` (`A (1 +- rel_step)`), sorted by decreasing magnitude.
pub fn sensitivities(
    mech: &Mechanism,
    rel_step: f64,
    outcome: &dyn Fn(&Mechanism) -> Result<f64, ReactorError>,
) -> Result<Vec<Sensitivity>, ReactorError> {
    let mut out = Vec::with_capacity(mech.reactions.len());
    for r in 0..mech.reactions.len() {
        let mut up = mech.clone();
        scale_reaction(&mut up, r, 1.0 + rel_step);
        let mut dn = mech.clone();
        scale_reaction(&mut dn, r, 1.0 - rel_step);
        let (fu, fd) = (outcome(&up)?, outcome(&dn)?);
        if !(fu > 0.0 && fd > 0.0) {
            return Err(ReactorError::Input("sensitivity outcome must be positive".into()));
        }
        let s = (fu.ln() - fd.ln()) / ((1.0 + rel_step).ln() - (1.0 - rel_step).ln());
        out.push(Sensitivity {
            reaction: r,
            coefficient: s,
        });
    }
    out.sort_by(|a, b| b.coefficient.abs().total_cmp(&a.coefficient.abs()));
    Ok(out)
}

/// Result of a PSR followed by a plug-flow reactor.
#[derive(Debug, Clone)]
pub struct PsrPlug {
    /// PSR outlet state `[T, Y...]`.
    pub psr: Vec<f64>,
    /// Plug-flow trajectory started from the PSR outlet; the last state entry is the residence time.
    pub plug: Trajectory,
}

/// A PSR (steady state found by pseudo-time integration from `guess`) whose outlet feeds a constant-pressure plug flow of
/// `velocity` and `length`. Errors if the PSR does not converge.
pub fn psr_then_plug(
    mech: &Mechanism,
    psr: &Psr,
    guess: &[f64],
    velocity: f64,
    length: f64,
    tol: Tol,
) -> Result<PsrPlug, ReactorError> {
    let (state, converged) = psr_steady(mech, psr, guess, tol)?;
    if !converged {
        return Err(ReactorError::Input("the PSR did not reach a steady state".into()));
    }
    let energy: Energy = psr.energy;
    let plug = run_plug_flow(mech, energy, psr.p, &state, velocity, length, tol)?;
    Ok(PsrPlug { psr: state, plug })
}
