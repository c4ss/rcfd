# ADR 0307: A [solid] table in the flow case and the library entry that runs the coupled case

Status: **PLANNED 2026-11-12** (layout by the intermediate-tier plan, [ADR
0290](0290-structural-solver-intermediate-tier-plan.md)); the registration is written by the implementation agent before any code.
Nothing here is a registered limit: the numbers below are the acceptance draft that the registration turns into checks.

Work package SI-17 of the structural solver, intermediate tier. Size M.

## Goal

A flow case can name a solid: an optional table `[solid]` (the solid case file, the fluid patch and the solid surface set of the
interface, the mapping method, the coupling tolerance, iteration cap, update and initial relaxation), added by the route of ADR
0257 so that no hash table or golden changes, and a library function of `rcfd-api` that builds both participants and runs
`coupled_step` over the time steps. The ordinary run path refuses a case that has the table instead of ignoring it. Library only:
the CLI and MCP exposure is ADR 0308.

## Dependencies

ADR 0304, 0306; roadmap items (SI ids are this tier's packages, ADR 0290 section 7): SI-14, SI-16, F-386.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. Gate (as check K0 of ADR 0257): with the optional table in
the schema the tests case_hashes_w48u, baseline_hashes, w48v_adaptive_correctors, rules_w19c, mesh_transform_w48u and
physics_schema of rcfd-api, cli of rcfd-cli, and default_f1, project_list_f270 and tool_list_f275 of rcfd-mcp pass with no file
edited. The piston of ADR 0304 written as a flow case with a [solid] table and a solid case file and run by the library entry
gives the interface displacement history of the library set-up within 1e-12 (expected identical) and so its frequency; the report
carries the iterations and residuals of every step. A case with the table run through the ordinary path is refused with the key; a
missing patch, a missing surface set, a solid file without a transient analysis and a time step that differs between the two files
are each refused with the key and the line. A flow case without the table gives the bytes it gave before.

## Reference to check against

ADR 0257 (reading 3 and check K0: an optional table needs no golden edit) and ADR 0155; the closed form is that of ADR 0304,
reached through the files.

## Files

`crates/rcfd-api/src/schema.rs` (the optional table), `crates/rcfd-api/src/solid_coupled.rs` (new), `crates/rcfd-api/src/lib.rs`
(one `pub mod` line), the one refusal in the ordinary case-building path (a shared file: baseline hashes are run),
`crates/rcfd-api/tests/si_solid_coupled.rs`, `docs/cosimulation-protocol.md`, `docs/structural-solver.md`.

## What could make it miss

I did not read how a moving wall and its mesh smoothing are described in a flow case (`dynmesh.rs`): if `FlowWall` cannot be built
from existing keys, the table needs more keys than listed or the package is partial. The refusal in the ordinary path touches a
shared file (decision N3). If the gate fails the table is removed and the library entry takes the two files as arguments.
