# ADR 0333: Substructuring: static condensation and fixed-interface component mode synthesis (Craig-Bampton)

Status: **PLANNED 2026-11-12** (layout by the advanced-tier plan, [ADR 0310](0310-structural-solver-advanced-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are the
acceptance draft that the registration turns into checks.

Work package SA-23 of the structural solver, advanced tier. Size M.

## Goal

Reduce a part to its interface: static condensation (Guyan) and the Craig-Bampton reduction (constraint modes plus a number of
fixed-interface modes) written as a transformation on the machinery of ADR 0296; reduced stiffness and mass as plain data that can
be saved and assembled with other parts and with ordinary elements; recovery of the interior displacements and stresses. A housing
with a rotor, or a blade row from one reduced blade, is the use.

## Dependencies

ADR 0311; roadmap items (SA ids are this tier's packages, ADR 0310 section 7): SA-1, F-382, F-383, F-393.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. Statics: the condensed model gives the retained displacements
and, after recovery, the interior ones and the stresses of the full model within 1e-10. Keeping every fixed-interface mode
reproduces all frequencies of the full model within 1e-8 (small model). A bar of two substructures with five modes each: the first
three frequencies of the assembly within 0.1 percent of the full model and not below it (1e-10), the error falling monotonically
with the number of modes. The reduced matrices are symmetric within 1e-13 and the reduced mass is positive definite; a free
substructure keeps six zero frequencies below 1e-6 of the seventh. Harmonic response of the assembly equals the full model within
1 percent up to half the lowest omitted fixed-interface frequency. A reduced part written to its file and read back gives the same
bits. An interface set with no nodes and a mode count above the interior size are refused.

## Reference to check against

The full model is the reference. The reduction is a Rayleigh-Ritz transformation, so the frequencies are upper bounds (first
principles). Guyan, AIAA J. 3 (1965) 380; Craig and Bampton, AIAA J. 6 (1968) 1313 (from memory).

## Files

`crates/rcfd-solid/src/cms.rs`, `crates/rcfd-solid/tests/sa_cms.rs`.

## What could make it miss

The reduced matrices are dense in the interface unknowns: a large interface defeats the purpose (reported). Free-interface
methods, reduction with damping or gyroscopic terms and mistuning studies on reduced blades are not included.
