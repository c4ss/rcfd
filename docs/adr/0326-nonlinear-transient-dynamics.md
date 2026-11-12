# ADR 0326: Nonlinear transient dynamics: Newton iteration inside the Newmark and generalised-alpha step

Status: **PLANNED 2026-11-12** (layout by the advanced-tier plan, [ADR 0310](0310-structural-solver-advanced-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are the
acceptance draft that the registration turns into checks.

Work package SA-16 of the structural solver, advanced tier. Size M.

## Goal

Time integration of M a + C v + f_int(u) = f(t) for the total Lagrangian solids and the materials with history: the schemes and
the state of ADR 0301 with a Newton iteration per step on the driver of ADR 0300, step halving on divergence, energy and momentum
reports, and save and restore bit for bit so that a nonlinear solid can stand behind the participant interface.

## Dependencies

ADR 0311; roadmap items (SA ids are this tier's packages, ADR 0310 section 7): SA-1, F-397, F-398.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. At a load factor of 1e-6 the history equals the linear one of
F-398 within 1e-9 relative. A stiff slender bar pinned at one end and released from the horizontal under gravity swings with the
period 2 pi sqrt(2 L/(3 g)) (2/pi) K(sin 45 degrees) (factor 1.1803, recomputed in the registration) within 1 percent, the total
energy constant within 1e-3 of its swing over five periods at the registered step. A free spinning elastic bar keeps its linear
momentum within 1e-10 and its angular momentum within 1e-6 relative over ten turns. Large axial oscillation of a bar element with
nu = 0 from the stretch 1.2: period equal to the quadrature of the energy integral (computed in the registration) within 0.5
percent. With the spectral radius 1 the generalised-alpha result equals Newmark within 1e-10. A run split by save and restore has
the bits of the unsplit run, with a material state included. A step that does not converge after the registered halvings is an
error with the time.

## Reference to check against

Closed forms: the compound pendulum at large amplitude (complete elliptic integral); conservation of momentum of a free body; the
one-unknown oscillator with the St. Venant-Kirchhoff bar force (energy integral). Methods as ADR 0300 and ADR 0301.

## Files

`crates/rcfd-solid/src/nl_transient.rs`, `crates/rcfd-solid/tests/sa_nl_transient.rs`.

## What could make it miss

The trapezoidal rule does not conserve energy in nonlinear problems and can blow up at large steps: the 1e-3 is a draft tied to
the registered step. A pin on a solid mesh is a line of constrained nodes. No energy-momentum scheme, no contact in dynamics. The
public beam tests of the flag benchmark are in ADR 0337, not here.
