---
type: source
tags: [adr-thread, meshing, mesh-quality, formats, partitioning, inflation, tet-mesher, hex-meshing, adaptation, quality-sweeps]
sources: [doc-mesh-quality, doc-meshing, doc-interchange-formats, doc-reference-data-wave14-mesh-quality, doc-reference-data-wave19b-meshing, doc-reference-data-f306-tet-mesher]
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: meshing and mesh formats in the early waves (formats, quality measures, partitioning, tet and hex meshers, adaptation, the tet quality pass)

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations with results, partials close-outs, one default evaluation), 10 ADRs, ADR 0011 to wave 42C.

Library meshing grew in vertical slices, each with closed-form expected numbers: interchange formats and cell registers (ADR 0011), six quality measures and repair (ADR 0018), partitioning, reordering, inflation and derived fields (ADR 0020), surface repair, remeshing and a Delaunay tetrahedral mesher (ADR 0030), sweep / multizone hex meshing and a Cartesian-core fill (ADR 0047), adaptation and turbomachinery post (ADR 0054). Waves 39 to 42 closed partials: the tet mesher's sliver problem was traced to the remesher's dependence on vertex numbering and an opt-in quality pass (`quality_sweeps`) was added (ADR 0104); making it the default failed the registered rule (ADR 0107). The fan-passage meshes are on [[adr-thread-fan01-rotating-solve]] and [[adr-thread-rom-reference-fan-and-c-grid]]. See [[mesh-quality]], [[mesh-generators]], [[interchange-formats]].

## Current state

- **Tet quality pass is opt-in, default `quality_sweeps = 0`.** "F-131 is done when the quality pass is on; the default options still miss and a user must set `quality_sweeps`"; the pass "has no termination or quality guarantee; it is a bounded heuristic (10 sweeps)" (ADR 0104). The default flip was evaluated under the user decision of 2026-10-22 ("only if every existing baseline hash stays identical") and "do[es] NOT meet the registered rule": B4 (`t5_torus_known_miss` would change from 0.634 to 1.000) and B5 (cost) block (ADR 0107). F-132 stays partial: "no test covers boundary layers on a tetrahedral mesh of STL geometry".
- **Quality measures and warnings:** six measures and a poor-mesh warning list separate from `warnings` and `errors`, "so the pass/fail behaviour cannot change"; `angle_between` uses $\operatorname{atan2}(\lvert \mathbf{a}\times\mathbf{b}\rvert,\, \mathbf{a}\cdot\mathbf{b})$ after an `acos` conditioning miss (ADR 0018). F-233 closed with closed-form assertions on reference cells (ADR 0093).
- **Formats:** STL, Tecplot ASCII, Plot3D, XDMF, glTF 2.0 "written from their public specifications"; HDF5 not in scope, "so F-119 stays `partial` whatever else passes" (ADR 0011); EnSight Gold later (ADR 0048, on [[adr-thread-early-workflow-and-partials]]).
- **Partitioning and inflation:** recursive coordinate and inertial bisection, KL / FM refinement, Cuthill-McKee; "every registered check passed at its first run"; inflation layers go "on the outward side of a boundary patch, not carved into a fluid domain", and Addendum 1 found the layer order reversed (the wall cell was the thickest), corrected with limits unchanged (ADR 0020).
- **Hex meshing:** sweep, thin-volume and multizone (F-235) and a Cartesian core with prism layers (F-234, partial); the Poiseuille point "is reached on the nz = 2 mesh, NOT on the registered nz = 8 mesh" (ADR 0047).
- **Adaptation:** criteria, hanging-node refinement and polyhedral dual conversion (F-102); "Nothing is added to the case-tree schema" (ADR 0054). See [[mesh-adaptation]].

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0011](../../docs/adr/0011-formats-and-registers.md) | | registration + results | F-117, F-119, F-124 | Interchange formats, cell registers, GLB validation | R5 known miss (1600 against the mis-derived 1728); no viewer check; HDF5 out of scope | 0048 |
| [0018](../../docs/adr/0018-mesh-quality.md) | | registration + results | F-231, F-233 | Quality measure catalogue; mesh editing and repair | warp miss fixed by `atan2`; E1 invariance known miss (1.48e-6 deg); both partial | 0093 |
| [0020](../../docs/adr/0020-wave15b-mesh-post.md) | | registration + results | F-072, F-108, F-125, F-236 | Partitioning and reordering, inflation, derived fields | all met at first run; layer-order defect found in review and corrected | 0021 (wiring) |
| [0030](../../docs/adr/0030-wave19b-meshing.md) | | registration + results | F-131, F-132, F-232 | Surface diagnostics and repair, remeshing and sizing, Delaunay tet meshing | T5 radius-edge 0.634 against >= 0.99 and R5 volume 4.18 % against 3 % on the torus; slivers remain | 0104 |
| [0047](../../docs/adr/0047-wave26b-hex-meshing.md) | | registration + results | F-234, F-235 | Sweep / thin-volume / multizone hex; Cartesian-core fill | S8, S10, S10b, O5 missed; S10c corrected met | |
| [0054](../../docs/adr/0054-wave28c-adaptation-turbo-post.md) | | registration + results | F-102, F-123, F-132, F-348 | Adaptation, hanging nodes, polyhedral dual; turbomachinery post | A4 (3.71 against 3) and T2 (2.85e-3 against 1e-11) known misses | |
| [0065](../../docs/adr/0065-wave32a-fan-geometry-mesh.md) | | registration + results | F-133 | Parametric axial-fan geometry, H-type periodic passage | M4 quality missed (on [[adr-thread-fan01-rotating-solve]]) | 0068 |
| [0093](../../docs/adr/0093-wave39b-partials-closeout-mesh-and-schemes.md) | | partials close-out | F-131, F-211, F-233 | Closed-form cell checks; compressive order; STL cylinder-in-box | F-233, F-211 met; F-131 S2 (1.84 deg against 3.0) and S3 (0.936 against 0.99) missed | 0104 |
| [0104](../../docs/adr/0104-wave41d-tet-sliver-removal.md) | | diagnosis + opt-in | F-131, F-132 | Numbering dependence and an opt-in quality pass | H1 holds (remesher), H2 falsified (`tetrahedralize`); Q1-Q7 met with `quality_sweeps` 10; F-131 done with the pass on | 0107 |
| [0107](../../docs/adr/0107-wave42c-tet-quality-pass-default-evaluation.md) | | default evaluation | F-131, F-132, F-306 | The quality pass as a default? | rule NOT met (B4, B5); default stays `quality_sweeps = 0` | |

## Key numbers and limits

- ADR 0018: first warp run "1.2e-6 degrees on planar faces against 1e-12"; E1 "worst change 1.48e-6 degrees", because `compute_metrics` "uses `acos(cos)` and was not edited".
- ADR 0020 Addendum 1: with the wrong layer order "the y+ 0.8 case would have given about 2.0. None of the registered checks I1, I3, I4 and I6 could see it."
- ADR 0047: S8 annulus skewness 2.1e-2 against 1e-6 ("a derivation error in the ADR"); S10 centreline error 3.33 percent (limit 1) because `non_orthogonal_correctors = 0` leaves the 35 degree junction faces uncorrected; O5 non-orthogonality 77.4 degrees against 70.
- ADR 0104: with the pass "17.66 deg and fraction 1.0000 on the STL path and 16.33 deg and 0.9974 on the in-code path"; without it 1.84 deg (STL) and 3.34 deg (in-code surface) against 3.0.
- ADR 0107 B5: "L1 is 3.051 on M1 (a marginal miss) and 4.810 on M10 against 3.0, and L2 is 2.787 against 2.0, with other agents' builds on the machine in both runs".

## Open questions

- ADR 0107: switching the default "would need a user decision on `t5_torus_known_miss`, a cheaper pass or a user decision accepting the cost, and then the pins, docs and listed ADR addenda".- ADR 0011: "a viewer with correct colours" is NOT demonstrated.

## Related

- [[mesh-quality]], [[mesh-generators]], [[mesh-adaptation]], [[interchange-formats]], [[rcfd-mesh]], [[fan-passage-mesh]]
- [[adr-thread-fan01-rotating-solve]], [[adr-thread-rom-reference-fan-and-c-grid]], [[adr-thread-early-workflow-and-partials]]
- [[roadmap-meshing-and-mesh-formats]]
