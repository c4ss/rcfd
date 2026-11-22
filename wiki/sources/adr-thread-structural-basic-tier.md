---
type: source
tags: [adr-thread, structural, fem, rcfd-fem, rcfd-linalg, basic-tier, elements, solvers, modal]
sources: [adr-thread-structural-plan, doc-structural-solver]
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: the structural solver's basic tier as built, and the first intermediate registrations (ADRs 0271 to 0286, 0291, 0292, 0296, 0305)

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread: 17 registrations written into the numbers that the plans ADR 0270 and ADR 0290 reserved ("the file name of the planned stub is kept and the stub's text is kept unchanged at the end"), all on 2026-11-12 and 2026-11-13. Each says "Not yet accepted by the maintainer" and proposes a status for its item; the lead's status is the roadmap's.

Work packages SB-1 to SB-16 of the basic tier and SI-1, SI-2, SI-6, SI-15 of the intermediate tier ([[adr-thread-structural-plan]]). In order: the crate `rcfd-fem` and its data model (0271), the element library Tet4 / Hex8 / Tet10 (0273), the envelope `L D L^T` direct solver in `rcfd-linalg` (0274), assembly, constraints and the static solve (0275), surface and body loads with the centrifugal load (0276), stress recovery (0277), `.vtu` output (0278), the manufactured-solution orders (0279), thermal strain (0281), preconditioned CG (0282), the generalised eigensolver (0283), mass and modal analysis (0284), the static co-simulation participant (0286), the intermediate scaffold (0291), Hex20 (0292), multi-point constraints (0296) and smoothed-aggregation multigrid for elasticity (0305). Each was written "before any code" with arithmetic done by hand or by a short closed-form script, and each ends with a roadmap note for the lead. The follow-up registrations of the same items (orders on distorted meshes, IC(1), Hex20 follow-ups, AMG on Tet10) are on [[adr-thread-structural-verification-elements]] and [[adr-thread-structural-linear-solvers]].

## Current state

- **Built and in the roadmap as done:** F-370 (0271), F-372 (0273), F-373 (0274), F-374 (0275), F-375 (0276), F-376 (0277), F-377 (0278), F-380 (0281), F-382 (0283), F-385 (0286), F-388 (0291), F-393 (0296) ([[roadmap-structural-solver]]).
- **Partial:** F-378 (0279): "The Tet4 case T L2 order is 1.7925 on the pair 8 to 16 against 1.8 (pre-asymptotic; 1.9176 one level finer by F-458, limit unchanged)", two mutations survive; F-381 (0282): "incomplete Cholesky with zero fill needs 0.545 of block Jacobi's iterations against the limit 0.5 (0.535 heavy, 0.705 at L/h 10.7); IC(1) meets it at 0.372 (F-460) but IC(0) is unchanged"; F-383 (0284): "All numbers are met; the item stays partial until the maintainer accepts amendment 2 (check C5 of the element mass matrices, added after mutation M8 survived the registered checks)"; F-389 (0292): C1 1.805e-2 and T1b 1.168e-2 against 1e-2, "both diagnosed", met under the follow-ups of F-485; F-402 (0305): on the slenderness-20 Tet10 cantilever (121 680 unknowns) "AMG needs 79 iterations against the limit of 30".
- **"Done is the maintainer's call"** is the roadmap's wording for F-378 and F-381.
- **The interfaces the later tiers use** were fixed here: `DofMap` as the only place that knows the unknowns per node (0271), the `Element` trait (0273), `assemble_with` as the generic hook (0275), `IntegrationPointField` (0277), `StaticSolid` behind `cosim::Structure` (0286), and the scaffold's extension points (0291).

> [!warning] Contradiction (within the repo docs)
> ADR 0286's "Decision by the registered rule" reads "**partial.** ... the registered check F2 missed on its registered run (a set-up defect, corrected by amendment 1 after the run), and Rule I makes a missed registered check partial." The roadmap lists F-385 as **done** with no gap ([[roadmap-structural-solver]]). No text in this job's reading says who moved it to done; the roadmap is the source of truth for the status.

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0271](../../docs/adr/0271-rcfd-fem-crate-and-solid-data-model.md) | 2026-11-12 | registration (SB-1) | F-370 | Crate `rcfd-fem` in layer 2 and the solid data model, structured builders | proposed done; done | |
| [0273](../../docs/adr/0273-element-library-tet4-hex8-tet10.md) | 2026-11-12 | registration (SB-3) | F-372 | Tet4 (1 point), Hex8 (2 x 2 x 2 Gauss), Tet10 (4 points); isotropic elasticity | proposed done; done | |
| [0274](../../docs/adr/0274-sparse-direct-solver-for-spd-matrices.md) | 2026-11-12 | registration (SB-4) | F-373 | Envelope `L D L^T` after reverse Cuthill-McKee in `rcfd_linalg::skyline` | proposed done; done | 0334 / F-431 (fill-reducing, opt-in) |
| [0275](../../docs/adr/0275-assembly-displacement-constraints-and-static-solve.md) | 2026-11-12 | registration (SB-5) | F-374 | Assembly, constraints, nodal forces, linear static solve, patch test | proposed done; done | |
| [0276](../../docs/adr/0276-surface-and-body-loads-with-centrifugal-load.md) | 2026-11-12 | registration (SB-6) | F-375 | Traction, pressure, gravity, centrifugal load, body force field | proposed done; done | |
| [0277](../../docs/adr/0277-stress-and-strain-recovery-von-mises-principal.md) | 2026-11-12 | registration (SB-7) | F-376 | Integration-point, element and nodal stress and strain; von Mises, principal | proposed done; done | 0335 / F-432 (patch recovery) |
| [0278](../../docs/adr/0278-vtk-output-of-solid-results.md) | 2026-11-12 | registration (SB-8) | F-377 | `.vtu` of finite-element cells with point and cell data | proposed done; done | |
| [0279](../../docs/adr/0279-manufactured-solution-and-convergence-orders.md) | 2026-11-12 | registration (SB-9) | F-378 | Observed orders of displacement and stress for the three elements | partial: A1-T 1.7925 against 1.8; mutation M5 not caught | F-458, F-459 ([[adr-thread-structural-verification-elements]]) |
| [0281](../../docs/adr/0281-thermal-strain-load.md) | 2026-11-12 | registration (SB-11) | F-380 | Nodal temperature field, expansion, thermal load and stress | proposed done; done | |
| [0282](../../docs/adr/0282-preconditioned-cg-for-elasticity.md) | 2026-11-12 | registration (SB-12) | F-381 | CG with block Jacobi and incomplete Cholesky on the stiffness | partial: A3 missed (IC(0) 0.545 of block Jacobi's iterations, limit 0.5) | F-460 ([[adr-thread-structural-linear-solvers]]) |
| [0283](../../docs/adr/0283-generalised-symmetric-eigensolver.md) | 2026-11-12 | registration (SB-13) | F-382 | `K x = lambda M x` by dense Jacobi and subspace iteration on the factor, with a Sturm count | proposed done; done | |
| [0284](../../docs/adr/0284-mass-matrix-and-modal-analysis.md) | 2026-11-12 | registration (SB-14) | F-383 | Consistent mass and linear modal analysis | proposed partial (amendment 2, C5 added after mutation M8 survived); partial | |
| [0286](../../docs/adr/0286-in-process-loads-and-static-structural-participant.md) | 2026-11-12 | registration (SB-16) | F-385 | Fluid face forces in process; a static solid as a co-simulation participant | partial by Rule I (F2 set-up defect); roadmap done | F-400 (dynamic participant) |
| [0291](../../docs/adr/0291-intermediate-scaffold-modules-and-model-extensions.md) | 2026-11-13 | registration (SI-1) | F-388 | Module layout, model extension data, element-trait extension points | proposed done; done | |
| [0292](../../docs/adr/0292-quadratic-hexahedron-hex20.md) | 2026-11-13 | registration (SI-2) | F-389 | Hex20 with face loads, mass and the quadratic upgrade of a hexahedron box | partial: C1 1.805e-2, T1b 1.168e-2 against 1e-2 | F-484, F-485 ([[adr-thread-structural-verification-elements]]) |
| [0296](../../docs/adr/0296-linear-multi-point-constraints-rigid-sets-elastic-supports.md) | 2026-11-13 | registration (SI-6) | F-393 | Linear MPCs by a transformation, rigid sets, elastic supports | proposed done; done (four `si_scaffold` assertions need the maintainer's edit) | [[adr-thread-structural-modal-buckling-cyclic]] |
| [0305](../../docs/adr/0305-aggregation-amg-with-rigid-body-modes.md) | 2026-11-12 | registration (SI-15) | F-402 | Smoothed aggregation with rigid-body modes as a CG preconditioner | partial: H2, H3 missed in their registered runs | F-470, F-477 ([[adr-thread-structural-linear-solvers]]) |

## Key numbers and limits

- **ADR 0271:** `SolidMesh` with Tet4, Hex8 and Tet10 blocks "in the VTK node order", refusals for "node out of range, node repeated, det J <= 0 at any node, empty or duplicate sets"; `DofMap` is "the only place that knows the unknowns per node".
- **ADR 0273:** quadrature `tet_1`, `tet_4`, `tet_14` ("degree 5, positive weights, constants by Newton on the invariant moment equations") and Gauss 1 to 3 on the cube.
- **ADR 0274:** 7-point Laplacian 12^3, a permuted copy and its Kronecker product with a 3 x 3 SPD block: "relative residual 7.4e-16 to 1.3e-15 (limit 1e-12), agreement with cg 8.3e-14 to 1.1e-13 (limit 1e-9)"; orderings `Rcm` (default), `Natural`, `Given`, `SmallerEnvelope`; the existing `rcm_order` unchanged.
- **ADR 0275:** element matrices "by std threads in batches and a sequential scatter in element order, so bit-identical for any thread count"; free rigid-body motions refused per connected part "from the 6 x 6 Gram matrix of the rigid modes at the prescribed components, threshold 1e-10".
- **ADR 0276:** pressure "positive pressing on the solid"; face rules "pressure exact on every face, traction on flat faces"; body loads "exact on Tet4, trilinear Hex8 and straight Tet10".
- **ADR 0277:** extrapolation from the rule's points "constant for Tet4, linear for Tet10, trilinear for Hex8"; "a node joining two different materials is refused naming the node".
- **ADR 0278:** `fe_vtu_string` / `write_fe_vtu` with VTK_TETRA 10, VTK_HEXAHEDRON 12, VTK_QUADRATIC_TETRA 24; "the existing writers are unchanged (211 lines inserted before the test module, none changed)".
- **ADR 0282:** incomplete Cholesky restarts on a small pivot with "A + alpha diag(A), alpha <- max(2 alpha, 1e-3), which terminates by diagonal dominance"; every breakdown recorded.
- **ADR 0283:** subspace iteration with "q = max(2m, m+8) start vectors, a residual test per pair (1e-10 on the shifted operator)" and a Sturm count "that verifies no eigenvalue was missed".
- **ADR 0284:** consistent mass "exactly symmetric, bit-identical for 1 and 8 threads"; mass, centre and inertia from rigid motions "within 7.3e-16" on a box with moved interior nodes.
- **ADR 0305:** "smoothed aggregation on the node-block graph (theta 0), tentative prolongator by a local QR of the six rigid-body modes per aggregate with a registered drop rule (1e-10), prolongator smoothed by damped block Jacobi (4/3 over the spectral radius) ... V(1,1) cycle with forward / backward block Gauss-Seidel and a dense LU on the coarsest level". Roadmap gap: "The first coarse level is 5 times coarser than on Hex8. The residual comparison was moved to 1e-8 because 1e-10 is below the rounding floor."

## Open questions

- F-378, F-381, F-383: done "is the maintainer's call" or waits on the maintainer's acceptance of an amendment (roadmap gaps).
- ADR 0296: the four `si_scaffold` assertions "need the maintainer's" edit before F-393 is closed by its rule.
- ADR 0292: "the only failing existing tests are the ADR 0291 refusals of Hex20"; the old test `si_hex20::l1_to_l3_face_loads_and_uniaxial_tension` is left to the maintainer (ADR 0383, [[adr-thread-structural-verification-elements]]).
- F-385: which status holds (see the callout above).

## Related

- [[rcfd-fem]], [[rcfd-linalg]], [[structural-solver]], [[amg]], [[cosimulation-protocol]], [[mutation-testing]], [[registration]], [[bit-identical]]
- Threads: [[adr-thread-structural-plan]], [[adr-thread-structural-verification-elements]], [[adr-thread-structural-linear-solvers]], [[adr-thread-structural-modal-buckling-cyclic]], [[adr-thread-structural-fsi-coupling]]
- Areas: [[roadmap-structural-solver]]
