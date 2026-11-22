---
type: source
tags: [reference-data, wave-section]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: Wave 29D distributed-memory layer (F-100, F-016, ADR 0058)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 29D distributed-memory layer (F-100, F-016, ADR 0058)" (lines 1169-1182) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 29D distributed-memory layer references (ADR 0058).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read in this wave. Every reference below is recalled from memory and is unverified. No MPI library, no vendor code or binary was used or installed.

| Item | Status | Note |
|------|--------|------|
| Saad, "Iterative Methods for Sparse Linear Systems": preconditioned CG and BiCGStab, block-Jacobi and additive-Schwarz preconditioning | unverified | the algorithms are the ones already in `rcfd-linalg`, copied with global inner products; the book was not read in this wave |
| Higham, "Accuracy and Stability of Numerical Algorithms": `gamma_k = k u / (1 - k u)` bounds for dot products and matrix-vector products | unverified | the bound used in B4/B5 is DERIVED from the standard model of floating-point arithmetic; the text was not read |
| Condition number of the 2-D Dirichlet five-point Laplacian, `kappa = cot^2(pi / (2 (n+1)))` | derived | from the known eigenvalues `4 - 2 cos` sums; used to state the solution-difference limit 2e-7 (kappa 973 for 48 x 48) |
| Rank-ordered sequential reduction (deterministic, O(P) messages) | derived | a design choice, not taken from an MPI implementation; MPI collectives do not guarantee a fixed order, which is why the order is fixed here |
| Green-Gauss gradient with linear face interpolation, upwind convection with deferred linear-upwind correction | unverified | standard finite-volume practice recalled from memory; verified here only by exactness on linear fields and by distributed-versus-serial agreement |
| Strong-scaling timings of in-process threads | measured | one 16-logical-core host, shared memory bandwidth and rayon pool; NOT a cluster or network measurement and not compared with any published scaling |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
