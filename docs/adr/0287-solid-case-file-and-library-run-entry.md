# ADR 0287: A solid case file of its own and the library entry that runs it

Status: **PLANNED 2026-11-12** (layout by the basic-tier plan, [ADR 0270](0270-structural-solver-basic-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are
the acceptance draft that the registration turns into checks.

Work package SB-17 of the structural solver, basic tier. Size M.

## Goal

A minimal entry point that needs no golden edit: a TOML file of its own for a solid model (mesh from a generator or a Gmsh file,
material, constraints and loads on named sets, static or modal analysis, solver, output), parsed in a new module of `rcfd-api`,
and a library function that runs it and writes the `.vtu` and a JSON report. No key is added to the flow case schema, no CLI
subcommand and no MCP tool (each of those touches a golden and is left as the maintainer's step, decision M3).

## Dependencies

ADR 0272, ADR 0278, ADR 0282, ADR 0284; existing roadmap items: F-013, F-080.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. A solid case file (TOML of its own: mesh from a generator
or a Gmsh file, material, constraints and loads on named sets, static or modal analysis, solver choice, output) is read and run
by a library function of rcfd-api that writes the .vtu and a JSON report (displacement and stress extremes, reactions,
frequencies, solver statistics): an unknown key, a missing set, a wrong unit or type each give an error with the key and the
line; a file run gives the bits of the same model built by library calls (limit 1e-12, expected identical); the cantilever and
the bar-frequency checks pass when run from files, and a Gmsh tetrahedral mesh of a box read through rcfd-io gives the uniaxial
closed form within 1e-10; no flow case key, CLI help text, MCP tool list, hash table or golden changes (the existing schema, CLI
and tool-list tests pass unedited).

## Reference to check against

The closed forms of ADRs 0275, 0280 and 0284, re-run through the file. The way ADR 0257 established that a schema addition needs
no golden edit (its check K0) is the model for this ADR's own check that nothing existing changes.

## Files

`crates/rcfd-api/Cargo.toml`, `crates/rcfd-api/src/lib.rs` (one `pub mod` line), `crates/rcfd-api/src/ solid_case.rs`,
`crates/rcfd-api/tests/sb_solid_case.rs`, `docs/structural-solver.md` (the file format section).

## What could make it miss

Using the settings tree of `rcfd-core` (schema, located errors) against plain serde is a design choice that changes the size; if
the tree is used, the solid schema must stay a separate schema object so that the schema golden does not change.
