---
type: source
tags: [adr-thread, meshing, amr, adaptation, anisotropic, coarsening, field-store]
sources: []
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: adaptive mesh refinement in the solver loop

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations with result addenda), 7 ADRs, 2026-11-12 to 2026-11-13.

The lattice `rcfd_mesh::adapt` (`AmrMesh`, ADR 0054) had refinement and criteria but no solver loop. F-102 (ADR 0242) added a solver-loop adaptation hook and the adapted lid-driven cavity; F-343 (ADR 0247) added target-count, edge-length-weighted and register-based criteria and a suction-layer case where adaptation pays; F-342 (ADR 0262) gave the loop the case key `[mesh.adapt]`; F-348 (ADR 0267) a second lattice type with per-axis levels for directional refinement; F-439 (ADR 0341) a box merge for directional coarsening in 3-D; F-441 (ADR 0343) coarsening inside the directional loop; F-476 (ADR 0373) made the field store of an adaptive run write the adapted mesh. See [[mesh-adaptation]].

## Current state

- **Done:** F-102, F-342, F-439, F-441, F-476. **Partial:** F-343 ("Merge-based polyhedral conversion, multigrid precoarsening and parallel adaptation are not built and stay listed here until split"), F-348 ("the item stays partial because two registered supporting checks missed and were corrected by amendment"; open: the case key F-438) ([[roadmap-meshing-and-mesh-formats]], [[roadmap-post-processing-and-output]]).
- **Case key:** `[mesh.adapt]` beside `[mesh.box_3d]` and `[mesh.transform]` (ADR 0262 Q1 (a)); an empty table is ADR 0242's run A; `converged` is the last solve's. Graded and inflated box meshes are refused (Q4). The directional lattice has no case key yet (F-438).
- **Acceptance numbers:** adapted cavity 2799 cells against 9216, centreline difference `0.0023` of the lid speed (ADR 0242); suction layer 3841 cells, `0.23` of the uniform 128 x 128, with `0.26` of its error (ADR 0247); directional run reaches `E(A)` with 1547 of 3841 cells (ADR 0267, limit 1920); coarsening run `0.0029776` (limit `0.00298`) with 1292 cells and 413 x-split leaves (ADR 0343).
- **Defaults bit for bit:** each criterion, lattice type and coarsening rule is opt-in (`AmrMesh` untouched by the per-axis type; `Coarsening` enum with `adapt_with`, `adapt` unchanged).

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0242](../../docs/adr/0242-f102-registration-adapted-cavity-and-solver-loop-hook.md) | 2026-11-12 | registration | F-102 | Adapted cavity matches the fine uniform solution with fewer cells; a loop hook | done; 2799 cells against 9216 | 0247, 0262 |
| [0247](../../docs/adr/0247-f343-registration-adaptation-criteria-and-suction-layer.md) | 2026-11-12 | registration | F-343 | Target-count, edge-length, register-based criteria; a case where adaptation pays | partial; all checks and the layer acceptance met; anisotropic criterion not built (D5) | 0267 |
| [0262](../../docs/adr/0262-f342-registration-case-key-for-the-adaptive-steady-loop.md) | 2026-11-12 | registration (+ gate 4 addendum 2026-11-13) | F-342 | The case key `[mesh.adapt]` | done; gate 4 found the store gap (became F-476) | 0373 |
| [0267](../../docs/adr/0267-f348-registration-anisotropic-refinement-per-axis-levels.md) | 2026-11-12 | registration | F-348 | Directional refinement on a lattice with per-axis levels | partial; every acceptance clause met; G6, C5 missed (corrected as G6b, G6c, C5b) | 0341 |
| [0341](../../docs/adr/0341-f439-registration-box-merge-for-directional-coarsening.md) | 2026-11-12 | registration | F-439 | Coarsening on the directional lattice by a box merge | done; every check met at the first run | 0343 |
| [0343](../../docs/adr/0343-f441-registration-coarsening-in-the-directional-loop.md) | 2026-11-12 | registration | F-441 | Box coarsening inside the directional loop | done; margin to the limit `2.4e-6` | |
| [0373](../../docs/adr/0373-f476-registration-field-store-of-an-adaptive-run.md) | 2026-11-13 | registration | F-476 | The field store of a `[mesh.adapt]` run writes the adapted mesh | proposed done | |

## Key numbers and limits

- ADR 0242 R2: "Adaptation pays little on Re 100. The uniform 48 x 48 is nearly as close per cell."
- ADR 0247: the uniform 90 x 90 with 8100 cells misses the layer acceptance "by a factor 1.9"; exponents `r = 1` and `r = 2` of the edge-length weighting gave the same layer error (R2).
- ADR 0267 R2: in 3-D "pair merges can leave a five-box tiling that they cannot undo" (settled by the box merge of ADR 0341).
- ADR 0343 Q2: `E(DC) <= 0.00298` read as the quoted number, not `E(D)` of the same run (`0.0029752`); R1: the ratio rule cannot see 251 of the 413 far-field x splits.

## Open questions

- ADR 0242 R1 (case key, delivered by F-342), R2 (a case with a localised feature, Re 1000 corner eddies or a step).
- ADR 0262 Q4 (graded and inflated box meshes), Q5 (one monitor row per solve).
- ADR 0267 R1 / ADR 0341 R1: F-348's status (partial by its rule; done is the maintainer's to give).
- ADR 0343 R1 (an absolute far-field coarsening rule), R2 (a fast loop test that merges inside the loop).

## Related

- Predecessor (added by p3-records-a): the library adaptation criteria, hanging-node refinement and polyhedral dual of F-102 come from ADR 0054 (wave 28C), with "Nothing ... added to the case-tree schema" there; see [[adr-thread-early-meshing]].
- [[mesh-adaptation]], [[lid-driven-cavity]], [[field-store]], [[rcfd-mesh]], [[doc-meshing]]
- [[roadmap-meshing-and-mesh-formats]], [[roadmap-post-processing-and-output]]
