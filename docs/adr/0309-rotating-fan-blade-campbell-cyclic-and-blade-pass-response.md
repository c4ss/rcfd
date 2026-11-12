# ADR 0309: Demonstration on the fan rotor: Campbell diagram of the blade, the bladed disc by cyclic symmetry, response to blade-passing forcing

Status: **PLANNED 2026-11-12** (layout by the intermediate-tier plan, [ADR
0290](0290-structural-solver-intermediate-tier-plan.md)); the registration is written by the implementation agent before any code.
Nothing here is a registered limit: the numbers below are the acceptance draft that the registration turns into checks.

Work package SI-19 of the structural solver, intermediate tier. Size M.

## Goal

Bring the tier to the target of fan breakdown item E5 for a rotating 120 mm fan: the blade of ADR 0288 at speed (Campbell table of
its lowest modes with the engine orders of the blade and strut counts), one sector of the bladed hub by cyclic symmetry, and the
harmonic response of the blade to a pressure harmonic at the blade-passing frequency with an assumed damping. There is no measured
blade, so the checks are consistency, bounds and mesh change; the result is a demonstration, not a validation. Extends the user
page.

## Dependencies

ADR 0297, 0299, 0302, 0306; roadmap items (SI ids are this tier's packages, ADR 0290 section 7): SI-7, SI-9, SI-12, SI-16, F-387.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. Blade of the parametric fan geometry in quadratic tetrahedra,
clamped at the hub, at registered speeds from rest to the registered maximum: the Campbell table of the four lowest modes is
written from a solid case file, with every crossing of the engine orders 1 to the blade count listed; at rest the frequencies have
the bits of F-387; for each mode the slope of omega^2 against Omega^2 at the lowest speed equals the Rayleigh quotient phi^T
(K_sigma,1 - M_perp) phi of the mode at rest within 1 percent and lies between -1 and the registered flap bound; between two
meshes (the second with at least twice the elements) the first frequency at the maximum speed changes by less than 5 percent, both
recorded. Bladed hub with Z blades: the sector frequencies for harmonic indices 0 to Z/2 equal those of the full rotor on the
coarse mesh within 1e-6 (heavy record). Blade-passing response: at a registered Campbell crossing the tip amplitude under a unit
pressure harmonic equals the modal estimate (participation over 2 zeta omega^2) within 1 percent and falls to the static value
within 1 percent at one tenth of the frequency. The page docs/structural-solver.md states the scope, that the Coriolis term is
neglected, that the damping is assumed and that nothing is validated against a measured blade.

## Reference to check against

Consistency only: the first-order perturbation of an eigenvalue, the full rotor as the reference of the sector, the single-mode
resonance estimate. No reference value exists for the blade (docs/fan-project-breakdown.md, part G).

## Files

`crates/rcfd-api/tests/si_fan_rotor.rs`, `docs/structural-solver.md`, the E5 status text of `docs/fan-project-breakdown.md`.

## What could make it miss

It inherits the risk of ADR 0288 (whether the tetrahedral mesher gives a usable blade); if the blade cannot be meshed the flat
twisted plate is used and the item is partial. The hub-and-blade sector needs matching cyclic boundaries from a mesher that does
not promise them. A coupled (flutter) run on the rotating fan passage is NOT part of this tier (ADR 0290 section 1.3).
