---
type: source
tags: [adr-thread, meshing, tetrahedral, robustness, located-error]
sources: [doc-reference-data-f306-tet-mesher]
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: the tetrahedral mesher's fold panic and its output check

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (one fix, two output-check registrations), 3 ADRs, 2026-11-08 to 2026-11-12.

`tetrahedralize` (`crates/rcfd-mesh/src/tetmesh.rs`) panicked with "neighbour link" on a thin box. F-306 (ADR 0228) traced it to a folded edge removal in `try_edge_removal` and fixed it. F-324 (ADR 0231) added an output validity check inside `tetrahedralize`: every cell positive, and the cell volume equal to the enclosed volume of the output surface; an overlapping output becomes a located error. F-327 (ADR 0236) runs the same check on the outputs of the F-247 remesher, through `TetMesh::check_output`. All three are done. See [[mesh-generators]], [[located-error]].

## Current state

- **F-306, F-324, F-327 done** ([[roadmap-meshing-and-mesh-formats]]), each "with the maintainer's acceptance" pending in the ADR.
- **Unchanged outputs:** with the fix, all 111 fold-free inputs and all 7 fold-free STL inputs give the recorded fingerprint or error text (ADR 0228 N1, N1s); the F-306 sweep inputs give identical meshes under the check (ADR 0231 C1, C6); the `remesh_f247` record and every P-T number are unchanged (ADR 0236 C1).
- **Invariant checks kept:** `commit` and `replace` keep their `expect("neighbour link")`; "with the fix no known input reaches them" (ADR 0228 Q2 recommendation (a)). Under ADR 0231's MU1, "six inputs still panic before the check can run" (ADR 0231 Q3).
- **Remesher:** the check runs at remeshing events only (ADR 0236 Q2 (a)); callers that move planes can call the public `TetRemesher::check_output()`.

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0228](../../docs/adr/0228-f306-registration-tetmesh-neighbour-link-panic-on-a-thin-box.md) | 2026-11-08 | diagnosis + fix | F-306 | The neighbour-link panic on a thin box | done (fix `501904db` in `try_edge_removal`); every check met, every mutation caught | 0231 |
| [0231](../../docs/adr/0231-f324-registration-tet-output-check.md) | 2026-11-12 | registration (two amendments) | F-324 | Output validity check inside `tetrahedralize` | done; C3a, C3c met by their amendment 2 corrections | 0236 |
| [0236](../../docs/adr/0236-f327-registration-remesher-output-check.md) | 2026-11-12 | registration | F-327 | The check on the F-247 remesher's outputs | done; NC1 to NC4 refuse with a located error, state left as it was | |

## Key numbers and limits

- ADR 0231 Q3: the check compares with the output surface (rigorous), not with `diagnose(input).enclosed_volume`.
- ADR 0231 Q2: face pairing on every run would catch overlaps "below B, which no input of the sweep has"; its cost was not measured (recommendation leave now).

## Open questions

- ADR 0228 Q2 / ADR 0231 Q3: turn the invariant panics into located errors (only if an unknown mechanism shows up).
- ADR 0231 Q2 (b): face pairing on every output, only if a sub-B overlap is ever seen.
- ADR 0236 Q6: a unit test that makes the operators' V2 or V3 decisive (recommendation leave).

## Related

- [[mesh-generators]], [[mesh-quality]], [[located-error]], [[rcfd-mesh]], [[doc-meshing]], [[doc-reference-data-f306-tet-mesher]]
- [[adr-thread-layering-remeshing-ale]], [[roadmap-meshing-and-mesh-formats]]
