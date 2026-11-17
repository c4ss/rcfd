# ADR 0709: F-769 registration: the mesh generator parameters and the checks of their test file

Status: **REGISTERED 2026-11-17, written before any run of the checks of [ADR 0705](0705-f769-registration-adrs-for-the-five-feature-groups-that-no-adr-describes.md)
(F-769, group 4 of 5).** Not yet accepted by the maintainer. Results are in the addendum of ADR 0705.

**These tests predate this ADR.** `crates/rcfd-api/tests/mesh_params.rs` was committed on 2026-10-14 (`0704ccaf`, "Mesh: cavity_2d honours
ny/ly; reject silently ignored generator options") as the regression test of a wave-11 bug; no ADR describes the generator parameters.
The documented meaning is in `docs/case-schema.md` (the passage on `nx`/`ny` and `lx`/`ly` of `cavity_2d`, which says that before wave 11
the generator silently built an `nx` x `nx` cavity of side `lx`). This ADR records what the file checks and which limits it carries,
read from the code and from that passage. It is not a decision taken before the tests. No Rust file is edited and no test is run for it
(ADR 0705, Limits).

## The feature

`rcfd_api::sim::build_mesh` builds the mesh of a case from the `[mesh]` section. The rule the file checks has two halves:

1. A parameter a generator documents is honoured: for `cavity_2d`, `nx` and `ny` are the cells in x and y, `lx` and `ly` the lengths, so
   a cavity is an `nx` x `ny` grid on `lx` x `ly`; with `nx = ny` and `lx = ly` the result is the old square cavity (unchanged).
2. A parameter a generator cannot honour is rejected with a located diagnostic (`Diagnostic.path` is the settings path, for example
   `mesh.wall_layers`) and is not ignored.

## Limits carried by the file

| id | test | kind | limit | source |
|---|---|---|---|---|
| M1 | `cavity_2d_honours_ny` | exact | `cavity_2d` with `nx = 20`, `ny = 2` has **40** cells | the assert |
| M2 | `cavity_2d_honours_lx_ly` | numeric and exact | `cavity_2d` with `nx = 8`, `ny = 4`, `lx = 2.0`, `ly = 0.5`: the bounding box of the points is **2.0** by **0.5** to within **1e-12** in x and y, and the mesh has **32** cells | the asserts |
| M3 | `square_cavity_unchanged` | exact | `cavity_2d` with `nx = 12`, `ny = 12` has **144** cells | the assert |
| M4 | `unsupported_mesh_options_are_rejected_not_ignored` | exact | `build_mesh` returns an error whose diagnostics contain a diagnostic with the given `path` for each of seven inputs: `wedge_2d` with `wall_layers = 4` and `first_layer_height = 0.01` gives `mesh.wall_layers`; `nozzle_2d` the same gives `mesh.wall_layers`; `channel_2d` with `periodic_x = true` gives `mesh.periodic_x`; `cavity_2d` with `periodic_y = true` gives `mesh.periodic_y`; `grading = 3.0` on `cavity_2d`, on `wedge_2d` and on `nozzle_2d` gives `mesh.grading`. The comments of the test state the rule: wall layers exist for `box_2d` and `channel_2d` only, periodic flags for `box_2d` only, and these generators do not apply grading | the asserts |

## What the file does not check

* The other generators and their parameters (`box_2d`, `channel_2d` with layers, `nz`, `lz`, 3D generators, imported meshes); `channel_2d`
  with `wall_layers` and `box_2d` with periodic flags are accepted by design but are not asserted here.
* The text, the line number and the number of diagnostics: only that one of them has the path. A test that passes with an extra
  diagnostic passes here.
* Cell geometry beyond the cell count and the bounding box (no orthogonality, no volume, no boundary patch names).
* That every documented parameter of every generator is honoured: the file covers `nx`, `ny`, `lx`, `ly` of `cavity_2d` and the seven
  rejected combinations above. A parameter silently ignored by another generator is not found by this file.
