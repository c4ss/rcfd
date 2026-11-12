# ADR 0338: Solid case file: the materials, analyses and results of the advanced tier

Status: **PLANNED 2026-11-12** (layout by the advanced-tier plan, [ADR 0310](0310-structural-solver-advanced-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are the
acceptance draft that the registration turns into checks.

Work package SA-28 of the structural solver, advanced tier. Size M.

## Goal

Make the fan-relevant part of the tier usable without writing Rust, and through the tools of ADR 0308 without a new tool: the
solid case file gains the material variants (orthotropic with axes, tables against temperature, Prony series, creep), the analysis
kinds direct harmonic, rotating modal with the Coriolis term, creep over time, random response, and the result requests fatigue
(curve, correction, mean and alternating load cases) and radiated power. Every key is optional; files of the earlier tiers run as
before.

## Dependencies

ADR 0312, ADR 0313, ADR 0314, ADR 0315, ADR 0316, ADR 0318, ADR 0320, ADR 0321; roadmap items (SA ids are this tier's packages,
ADR 0310 section 7): SA-2, SA-3, SA-4, SA-5, SA-6, SA-8, SA-10, SA-11, F-403.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. Each new analysis run from a file gives the bits of the same
model run by library calls (limit 1e-12, expected identical), and one closed-form check of each library package passes from files:
the off-axis block (ADR 0312), the relaxation curve (ADR 0313), the rotating creeping bar (ADR 0314), the bar with a loss factor
(ADR 0315), the cantilever life (ADR 0316), the plane oscillator's split (ADR 0318), the single-mode mean square (ADR 0320), the
piston's equivalent radiated power (ADR 0321). An unknown key, a material that lacks a required constant, a fatigue request
without a mean or an alternating case, a table that is not increasing and an analysis that names a missing set are each refused
with the key and the line. Every solid case file of the tests of the earlier tiers gives the same report and the same .vtu bytes
as before. No flow case key, CLI help text, MCP tool list, hash table or golden changes (the existing schema, CLI and tool-list
tests pass unedited).

## Reference to check against

The closed forms of the library packages, re-run through the file, and the bit comparison with library calls. ADR 0287 and ADR
0306 are the models.

## Files

`crates/rcfd-api/src/solid_case.rs`, `crates/rcfd-api/tests/sa_solid_case.rs`, `docs/structural-solver.md` (the file format
section).

## What could make it miss

Eight dependencies: if one ends partial its keys are left out and this package is partial with it. The other packages of the tier
(plasticity, contact, laminates, fracture, substructures, rotor, nonlinear dynamics) are parsed and checked only if merged when
this one starts; what is left over is library-only and the page says so.
