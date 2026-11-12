# ADR 0313: Linear viscoelasticity: Prony series in time, complex modulus in frequency, time-temperature shift

Status: **PLANNED 2026-11-12** (layout by the advanced-tier plan, [ADR 0310](0310-structural-solver-advanced-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are the
acceptance draft that the registration turns into checks.

Work package SA-3 of the structural solver, advanced tier. Size M.

## Goal

Polymers relax and damp. Add the generalised Maxwell solid (Prony series on the shear and, optionally, the bulk modulus) with a
recursive update of one internal stress per branch and integration point, exact for a strain that is linear in the step; the
time-temperature shift (WLF or Arrhenius constants as inputs); and the storage and loss moduli as functions of frequency, which is
the material damping that the direct harmonic solve (ADR 0315) and the damped modes (ADR 0317) take.

## Dependencies

ADR 0311; roadmap items (SA ids are this tier's packages, ADR 0310 section 7): SA-1, F-397, F-398.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. Relaxation of one element after a strain step: sigma(t) =
eps0 (E_inf + sum E_i exp(-t/tau_i)) within 1e-10 at every step end for three branches. Creep of the standard solid under constant
stress: eps(t) = sigma0 (1/E_inf - (1/E_inf - 1/E_0) exp(-t/tau_c)), tau_c = tau E_0/E_inf, within 0.1 percent at 20 steps per tau
with observed order >= 1.9. The complex modulus function returns E' = E_inf + sum E_i w^2 tau_i^2/(1 + w^2 tau_i^2) and E'' = sum
E_i w tau_i/(1 + w^2 tau_i^2) within 1e-13, and a strain cycled in time dissipates pi eps0^2 E'' per cycle and volume within 1
percent at 200 steps per cycle. Shift: the relaxation curve at a temperature with shift factor a_T equals the reference curve at
t/a_T within 1e-10. Free vibration of a bar of the standard solid with a loss factor near 0.05 at its first frequency: logarithmic
decrement pi E''/E' within 3 percent. A material without branches gives the bits of the elastic one; the internal stresses are
saved and restored bit for bit with the integrator state; a non-positive relaxation time or modulus is refused with the key.

## Reference to check against

Closed forms of the Maxwell and standard linear solid (one ordinary differential equation each); the recursive update is the exact
integral of the hereditary law for piecewise-linear strain (Taylor, Pister and Goudreau, Int. J. Numer. Meth. Engng 2 (1970) 45;
Simo and Hughes, Computational Inelasticity, chapter 10; from memory, derived in the registration).

## Files

`crates/rcfd-solid/src/mat_visco.rs`, `crates/rcfd-solid/tests/sa_visco.rs`.

## What could make it miss

With a fixed step the effective stiffness is constant, so the linear integrator of ADR 0301 can carry the history as a load; if
its interface cannot take a history force the dynamic clause runs through the nonlinear driver quasi-statically and the vibration
clause is not run (partial). No Prony series of any polymer is in the repository (decision A1).
