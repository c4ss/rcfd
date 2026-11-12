# ADR 0306: Solid case file: the analyses and model data of the intermediate tier

Status: **PLANNED 2026-11-12** (layout by the intermediate-tier plan, [ADR
0290](0290-structural-solver-intermediate-tier-plan.md)); the registration is written by the implementation agent before any code.
Nothing here is a registered limit: the numbers below are the acceptance draft that the registration turns into checks.

Work package SI-16 of the structural solver, intermediate tier. Size M.

## Goal

Make the intermediate tier usable without writing Rust: the solid case file of ADR 0287 gains the element properties (formulation,
section, thickness), linear constraints, rigid sets, cyclic pairs and supports, and the analysis kinds pre-stressed modal,
rotating modal with a speed list (Campbell table), buckling, cyclic modal with a list of harmonic indices, transient and harmonic;
the JSON report and the files written gain their results. Every key is optional: a file of the basic tier runs as before. No flow
case key, no CLI subcommand, no MCP tool.

## Dependencies

ADR 0296, 0297, 0298, 0299, 0301, 0302; roadmap items (SI ids are this tier's packages, ADR 0290 section 7): SI-6, SI-7, SI-8,
SI-9, SI-11, SI-12, F-386.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. Each new analysis run from a file gives the bits of the same
model run by library calls (limit 1e-12, expected identical), and the closed-form checks of the library pass from files: the Euler
column (ADR 0298), the rotating radial bar and a Campbell table with its crossings (ADR 0299), the ring at harmonic index 2 (ADR
0297), the resonance peak (ADR 0302), the free vibration of the bar (ADR 0301), the skew roller (ADR 0296). An unknown key, an
analysis that lacks a required table, a harmonic index above N/2, a speed list that is not increasing and a property that names a
missing set are each refused with the key and the line. Every solid case file of the basic tier's tests gives the same report and
the same .vtu bytes as before. No flow case key, CLI help text, MCP tool list, hash table or golden changes (the existing schema,
CLI and tool-list tests pass unedited).

## Reference to check against

The closed forms of the library packages, re-run through the file; the bit comparison with library calls. ADR 0287 is the model
for the file and for the check that nothing existing changes.

## Files

`crates/rcfd-api/src/solid_case.rs`, `crates/rcfd-api/tests/si_solid_case.rs`, `docs/structural-solver.md` (the file format
section).

## What could make it miss

Six dependencies: if one of them ends partial its analysis is left out of the file and this package is partial with it. Nonlinear
statics (ADR 0300), beams and shells are parsed as data but checked only if their packages are merged when this one starts. Time
histories of many steps need an output cadence and a size limit that the basic file did not need.
