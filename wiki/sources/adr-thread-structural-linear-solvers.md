---
type: source
tags: [adr-thread, structural, fem, linear-solvers, multigrid, direct-solver]
sources: []
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: structural solver, linear solvers (preconditioned CG, elasticity multigrid, sparse direct)

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations with result addenda), 7 ADRs in this half (2026-11-12 to 2026-11-14); later ADRs of the same subject (coordinate nested dissection, supernodal factor in contact and Newton drivers, six-unknown smoothed aggregation, thin-plate smoothers) are in the other half of the category and may be added here.

The linear-algebra route of [[rcfd-fem]]: the incomplete Cholesky preconditioner of the CG route (F-381 follow-ups), the elasticity multigrid on quadratic elements, the elasticity solve on the ranks of [[rcfd-dist]] and on the CUDA device, and the sparse direct $L D L^T$ (minimum degree, then nested dissection and supernodes) used by the static, modal, buckling and harmonic solves.

## Current state

- **Iterative route.** IC with one level of fill after a node-wise reverse Cuthill-McKee ordering (F-460, ADR 0357) is done: 58 iterations against the limit 78 and meets F-381's 0.5 iteration ratio at 0.372; IC(0) itself is unchanged, so F-381 stays partial. Elasticity multigrid on Tet10: the aggregation rule alone (F-470, ADR 0370) needs 31 CG iterations against 30 (partial); with two block Gauss-Seidel sweeps per side, `quadratic_smoothed` (F-477, ADR 0375), 23 iterations (done).
- **Parallel and GPU.** F-433 (ADR 0431) is done: `rcfd_api::solid_parallel` splits the reduced stiffness by nodes (recursive coordinate bisection) and solves on in-process ranks by the distributed CG of rcfd-dist, and on the CUDA device; library only.
- **Sparse direct.** F-431 (ADR 0447, AMD ordering, inertia) is **partial**: A4, A5 met only after amendment 1, the heavy record H1 not run (nnz(L) would be 13.6 GB), three mutations survived. F-529 (ADR 0503, nested dissection and supernodal $L D L^T$) met every check including H1 (3e5 unknowns) but one equivalent mutation survived: partial pending the maintainer. F-594 (ADR
  0509) puts the supernodal factor into the modal, buckling and harmonic solves and on the transformation path: partial only on how mutation M5 was caught.

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0357](../../docs/adr/0357-f460-registration-incomplete-cholesky-fill-and-ordering.md) | 2026-11-12 | registration | F-460 | IC with one level of fill after node-wise RCM | done; 58 iterations against 78, residual 5.4e-11 | |
| [0370](../../docs/adr/0370-f470-registration-tet10-aggregation-like-hex8.md) | 2026-11-13 | registration | F-470 | Opt-in geometric aggregation coarsening Tet10 like Hex8 | partial; H1b 31 against 30, M5 not caught | 0375 |
| [0375](../../docs/adr/0375-f477-registration-tet10-smoother.md) | 2026-11-13 | registration | F-477 | Two block Gauss-Seidel sweeps per side on quadratic elements, graded Tet10 box | done; H1a 23 iterations against 30, OC 1.4762 | |
| [0431](../../docs/adr/0431-f433-registration-elasticity-on-ranks-and-device.md) | 2026-11-14 | registration | F-433 | Elasticity solves on the ranks of rcfd-dist and on the CUDA device | done; every check met on its single run | |
| [0447](../../docs/adr/0447-f431-registration-sparse-ldlt-minimum-degree.md) | 2026-11-14 | registration | F-431 | Sparse $L D L^T$ after AMD, with the inertia, in static, modal, buckling solves | partial; H1 not run, MU6, MU7, MU9 survived | 0503 |
| [0503](../../docs/adr/0503-f529-registration-nested-dissection-and-supernodal-ldlt.md) | 2026-11-14 | registration | F-529 | Nested dissection and supernodal $L D L^T$, heavy record of 3e5 unknowns | partial; every check met, M11 equivalent survived | |
| [0509](../../docs/adr/0509-f594-registration-supernodal-ldlt-in-modal-buckling-harmonic-and-transformation.md) | 2026-11-14 | registration | F-594 | Supernodal factor in modal, buckling, harmonic solves and on the transformation | partial; M5 caught only by L4 (b) | |

## Key numbers and limits

- F-381 gap: "incomplete Cholesky with zero fill needs 0.545 of block Jacobi's iterations against the limit 0.5 (0.535 heavy, 0.705 at L/h 10.7); IC(1) meets it at 0.372 (F-460) but IC(0) is unchanged" ([[roadmap-structural-solver]]).
- F-470: "CG with one V-cycle needs 31 iterations against the limit 30 (default 79)"; first coarse level 1/10.14 of the fine one (window 1/15 to 1/6), operator complexity 1.476 (ADR 0370).
- F-477: "H1a 23 iterations against 30; H1b OC 1.4762" (ADR 0375).
- F-433 (from the user page): Jacobi CG on 304 842 unknowns in 0.60 s on the device, 4.50 s on 16 threads ([[doc-structural-solver]]).
- F-431 H1: "nnz(L) would be 13.6 GB, not solved for memory" (roadmap gap); recommendation (b), a follow-up with nested dissection and supernodes, H1 as its heavy record (ADR 0447), taken as F-529.

## Open questions

- F-470, F-529, F-594: done is "the maintainer's call" (accept the equivalent mutation, or L4 (b) as the check of the $\tau$ term) (ADR 0503 Q1, ADR 0509 Q1).
- F-431: "Done is the maintainer's call" (roadmap gap).

## Second half of the category (ADRs 0531 to 0577, job p3-structural-b)

### Current state (second half)

- **Coordinate nested dissection in the eigensolvers** (F-595, ADR 0531, done): `SubspaceOptions::points`, `BucklingEigenOptions::points` and `ModalOptions::coordinate_order` feed node coordinates to the supernodal factor's dissection; on a clamped cube of 104 544 unknowns `nnz(L)` falls to `0.838` of the level-structure order (limit `0.85`), frequencies within `1.4e-14`, 98 s against 127 s per modal analysis.
- **Supernodal factor in the contact and Newton drivers.** `ContactOptions::factor` with `SupernodalNestedDissection` in the linear contact driver: the hub fit of ADR 0517 (96 046 free unknowns) in 82.0 s against 1456.6 s, ratio `5.6e-2` (limit 1/3) (F-615, ADR 0533, done). `NewtonOptions::factor` for the total Lagrangian contact body (F-618, ADR 0542, partial: T1 `0.355` against `< 1/3`, M3 survived); caching the dissection order and symbolic analysis by pattern (`SupernodalAnalysis`, `FactorReuse`) brings T1 to `0.254` (F-624, ADR 0566, partial: the timed run Q had other builds on the machine).
- **Smoothed aggregation for structural elements.** `CgPreconditioner::SmoothedAggregationStructural` (solver name `cg-amg-rbm-structural`) with six rigid fields so rotation unknowns of Beam2, Shell4 and Shell3 nodes enter the near-null space (F-629, ADR 0552, done); the thin Shell4 plate's count is caused by the smoother, cured by a vertex-patch smoother (F-641, ADR 0572, done), robust over 34 thin-plate cases from $t/a = 0.1$ to `0.001` (F-659, ADR 0577, done).
- Defaults are unchanged throughout (the default factor stays `SparseMinimumDegree` / `Envelope`); the case-file key and default factor of ADR 0542 open questions 1 to 3 are not taken (ADR 0566 Q3).

### ADRs (second half)

| ADR | Date | Kind | Item | Question | Outcome | Superseded by / followed by |
|---|---|---|---|---|---|---|
| [ADR 0531](../../docs/adr/0531-f595-registration-coordinate-nested-dissection-in-the-eigensolvers.md) | 2026-11-14 | registration | F-595 (done) | Coordinate nested dissection in the eigensolvers | done | — |
| [ADR 0533](../../docs/adr/0533-f615-registration-supernodal-factorisation-in-the-linear-contact-driver.md) | 2026-11-14 | registration | F-615 (done) | Supernodal factorisation in the linear contact driver | done | — |
| [ADR 0542](../../docs/adr/0542-f618-registration-supernodal-factor-in-the-newton-driver-for-the-total-lagrangian-contact-body.md) | 2026-11-15 | registration | F-618 (partial) | Supernodal factor in the Newton driver for the TL contact body | partial: T1 0.355 vs < 1/3, M3 survived | ADR 0566 |
| [ADR 0552](../../docs/adr/0552-f629-registration-six-unknown-nodes-in-the-smoothed-aggregation-cg.md) | 2026-11-15 | registration | F-629 (done) | Six-unknown nodes in smoothed-aggregation CG | done | ADR 0572 |
| [ADR 0566](../../docs/adr/0566-f624-registration-cached-dissection-order-and-symbolic-analysis-of-the-newton-supernodal-factor.md) | 2026-11-15 | registration | F-624 (partial) | Cached dissection order and symbolic analysis of the Newton supernodal factor | partial: T1 0.254 met, Q (a4) not met under load | — |
| [ADR 0572](../../docs/adr/0572-f641-registration-smoothed-aggregation-for-thin-plates-and-shells.md) | 2026-11-15 | registration | F-641 (done) | Smoothed aggregation for thin plates and shells: cause and vertex-patch smoother | done: cause is the smoother | ADR 0577 |
| [ADR 0577](../../docs/adr/0577-f659-registration-vertex-patch-smoother-on-a-mesh-sequence-of-thin-plates.md) | 2026-11-15 | registration | F-659 (done) | Vertex-patch smoother on a thin-plate mesh sequence | done (34 cases) | — |

### Open questions (second half)

- Rerun the heavy timing record of F-624 when no agent is building (ADR 0566 Q1): recommendation (a), then done if T1 holds.
- Case-file key, default factor, `SparseMinimumDegree` (ADR 0542 Q3, ADR 0566 Q3): open.
- A convex switch for ADR 0517's interference with the total Lagrangian body (ADR 0542 Q4): needs the maintainer.

## Related

- [[rcfd-fem]], [[rcfd-dist]], [[rcfd-gpu]], [[rcfd-linalg]], [[structural-solver]], [[roadmap-structural-solver]]
- [[adr-thread-structural-case-file-and-fan-blade]] (the solid case file maps its `[solver]` keys onto these preconditioners)
