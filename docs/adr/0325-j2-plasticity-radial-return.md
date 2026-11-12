# ADR 0325: J2 plasticity with isotropic and kinematic hardening: radial return and consistent tangent

Status: **PLANNED 2026-11-12** (layout by the advanced-tier plan, [ADR 0310](0310-structural-solver-advanced-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are the
acceptance draft that the registration turns into checks.

Work package SA-15 of the structural solver, advanced tier. Size M.

## Goal

The first material with history, moved here by decision N1 of ADR 0290. Small-strain von Mises plasticity with linear isotropic
and linear kinematic hardening (and a tabulated isotropic curve), integrated by radial return with the consistent tangent, on the
Newton driver and the state slot of ADR 0300; plastic strain and back stress as results.

## Dependencies

ADR 0311; roadmap items (SA ids are this tier's packages, ADR 0310 section 7): SA-1, F-389, F-397.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. The tangent agrees with central differences within 1e-6 in
the elastic and in the plastic branch. Uniaxial bar, bilinear: the stress follows sigma_y + (E H/(E + H)) (eps - sigma_y/E) within
1e-10 in one step and in 100; on reversal the bar yields again at -sigma_max with isotropic and at sigma_max - 2 sigma_y with
kinematic hardening within 1e-10. Thick sphere b/a = 2, perfectly plastic, internal pressure: the pressure at plastic radius c
equals 2 sigma_y ln(c/a) + (2 sigma_y/3)(1 - c^3/b^3) within 1 percent at c = 1.25 a and 1.5 a, the solve converges at 0.98 of the
limit pressure 2 sigma_y ln(b/a), and the residual hoop stress at the bore after unloading equals the closed form within 2
percent. Beam of rectangular section in pure bending: M/M_y = 1.5 - 0.5 (kappa_y/kappa)^2 within 1 percent up to kappa = 5
kappa_y. The last three residuals of each step show an order >= 1.8; a halved step restores the state bit for bit; a non-positive
yield stress and a softening curve are refused with the key.

## Reference to check against

Closed forms: the bilinear bar; the partly plastic sphere, where Tresca and von Mises coincide (Hill, The Mathematical Theory of
Plasticity, 1950, chapter 5; from memory, re-derived from equilibrium and the yield condition); the elastic-plastic moment of a
rectangle. Method: Simo and Taylor, Comput. Meth. Appl. Mech. Engng 48 (1985) 101; Simo and Hughes, Computational Inelasticity
(from memory; the tangent is checked by differences).

## Files

`crates/rcfd-solid/src/mat_j2.rs`, `crates/rcfd-solid/tests/sa_j2.rs`.

## What could make it miss

Isochoric plastic flow locks fully integrated elements: the checks use Hex20, and whether the B-bar element of ADR 0293 is offered
to the nonlinear driver is not known. Small strains only (the measure of the total Lagrangian driver, valid for large rotation
with small strain). Yield data of polymers are rate and temperature dependent: not modelled.
