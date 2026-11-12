# ADR 0337: Public coupled benchmark (the elastic flag behind a cylinder): structural tests CSM1 to CSM3 and the steady coupled case FSI1 (needs the maintainer: reference values)

Status: **PLANNED 2026-11-12** (layout by the advanced-tier plan, [ADR 0310](0310-structural-solver-advanced-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are the
acceptance draft that the registration turns into checks. **Blocked until the maintainer approves recording the published
reference values in `docs/reference-data.md` (decision A2 of ADR 0310).**

Work package SA-27 of the structural solver, advanced tier. Size M.

## Goal

The one public benchmark that the earlier tiers named (Turek and Hron, 2006). Its structural part: the flag alone under gravity,
St. Venant-Kirchhoff, plane strain, static (CSM1, CSM2) and dynamic (CSM3). Its steady coupled part: the flag in channel flow at
Reynolds number 20 (FSI1), run to a steady state with the static participant, after the flow-only case with a rigid flag (CFD1) as
a gate. The published values must first be recorded in `docs/reference-data.md` from the public paper: house rule 6 does not let
an implementation agent take a data set the repository lacks, so this needs the maintainer (decision A2).

## Dependencies

ADR 0326; roadmap items (SA ids are this tier's packages, ADR 0310 section 7): SA-16, F-385, F-397, F-401.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. NEEDS THE MAINTAINER'S APPROVAL to record the published
values in docs/reference-data.md before it starts; no limit below uses a value recalled from memory. Structure alone (plane
strain, one element thick): the tip displacement of CSM1 and CSM2 within 1 percent of the published values, both components; CSM3
(released under gravity, undamped): mean, amplitude and frequency of both tip components within 2 percent, on two meshes with the
change recorded. Flow alone with a rigid flag (CFD1): drag within 2 percent and lift within 10 percent of the published values
(gate; if it misses, the coupled case is not run and the item is partial). Coupled steady case FSI1: drag within 2 percent, tip
displacement in x within 5 percent and in y within 10 percent, lift within 10 percent, with the coupling converged to 1e-6 in the
final state and the iteration counts recorded. The unsteady cases FSI2 and FSI3 are not run.

## Reference to check against

Turek and Hron, Proposal for numerical benchmarking of fluid-structure interaction between an elastic object and laminar
incompressible flow, in Fluid-Structure Interaction, Lecture Notes in Computational Science and Engineering 53, Springer (2006)
371; public as a report of TU Dortmund. Not read for this plan; the values are not quoted here on purpose.

## Files

`docs/reference-data.md` (rows, after approval), `crates/rcfd-fem/tests/sa_flag_csm.rs`,
`crates/rcfd-post/tests/sa_flag_fsi1.rs`, a case folder under `cases/` for the channel mesh.

## What could make it miss

The riskiest package of the tier. A two-dimensional body-fitted mesh of the cylinder with the flag from the repository's
generators is not known to exist. The lift of FSI1 is a small number that the paper's own participants scatter on. The flow side's
accuracy on CFD1 is untested here. FSI2 and FSI3 need large mesh motion and a nonlinear participant over thousands of coupled
steps: not planned.
