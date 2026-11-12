# ADR 0281: Thermal strain: nodal temperature field, expansion coefficient, thermal load and stress

Status: **PLANNED 2026-11-12** (layout by the basic-tier plan, [ADR 0270](0270-structural-solver-basic-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are
the acceptance draft that the registration turns into checks.

Work package SB-11 of the structural solver, basic tier. Size S.

## Goal

The load export of F-237 carries nodal temperatures (`TEMP`). Add the isotropic thermal strain `eps_th = alpha (T - T_ref) I`:
the equivalent nodal load `int B^T D eps_th dV` and the stress `sigma = D (eps - eps_th)` in the recovery. Opt-in: a model
without a temperature field gives the bits it gave before.

## Dependencies

ADR 0276, ADR 0277; existing roadmap items: none.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. With a nodal temperature field, an expansion coefficient
and a reference temperature: a uniformly heated, statically determinately held box expands by alpha dT x at every node within
1e-10 with stresses below 1e-10 E alpha dT; a box held on all faces has the hydrostatic stress -E alpha dT/(1 - 2 nu) within
1e-10; a bar held at both ends with free sides has the axial stress -E alpha dT within 1e-10 (nu = 0.3); a linear temperature
through the thickness of a free beam gives the curvature alpha dT/h within 1 percent and stresses below 1 percent of E alpha dT
(Tet10); a model without temperature is bit-identical to the same model before this change.

## Reference to check against

Closed forms from Hooke's law with thermal strain (Timoshenko and Goodier, Theory of Elasticity, the chapter on thermal stress;
each derivable in a few lines).

## Files

`crates/rcfd-solid/src/thermal.rs`, small hooks in `loads.rs` and `recover.rs`, `crates/rcfd-solid/tests/sb_thermal.rs`.

## What could make it miss

It edits `loads.rs` and `recover.rs`, so it cannot run beside ADR 0276 or 0277. Temperature-dependent `E` and `alpha` are not
included (intermediate).
