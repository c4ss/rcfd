---
type: source
tags: [reference-data, structural]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: F-552 residual vectors of the interior load and kept modes by a cutoff (ADR 0474)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-552 residual vectors of the interior load and kept modes by a cutoff (ADR 0474)" (lines 1877-1889) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-552 residual vectors of the interior load (ADR 0474).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-552; the texts below are cited from memory for the names and the published form only. Every formula used is derived in ADR 0474 (the modal expansion of `K_ii^{-1} f_i` and the Rayleigh-Ritz method); the reference of every check is the full model computed by the existing solver paths (`solve_static`, `modal_analysis`, `DirectHarmonic`).

| Item | Status | Source |
|---|---|---|
| Residual flexibility of the interior load `r = K_ii^{-1} f_i - Phi Phi^T M_ii K_ii^{-1} f_i = sum_{k > m} phi_k phi_k^T f_i / omega_k^2`, M- and K-orthogonal to the kept modes; with `[Psi, Phi, R]` the static solution of an interior load lies in the trial space (exact statics) | derived | ADR 0474 design; R. R. Craig and A. J. Kurdila, Fundamentals of Structural Dynamics (2006); S. Rubin, AIAA J. 13 (1975) 995; R. H. MacNeal, Computers and Structures 1 (1971) 581: NOT read, from memory |
| M-orthonormalisation by classical Gram-Schmidt made twice, drop below `1e-8` of the M-norm | shared | ADR 0464 D1 (`damped::m_orthonormalise`); B. N. Parlett, The Symmetric Eigenvalue Problem ("twice is enough"): NOT read, from memory |
| Keeping the fixed-interface modes up to about 1.5 to 2 times the top frequency of interest (rule of thumb; ADR 0474 takes 4 times) | cited | Craig and Kurdila (2006): NOT read, from memory; no number taken |
| Response error at a resonance `2 abs(delta) / eta` for a relative frequency error `delta` and a loss factor `eta` | derived | `1 / (omega^2 (1 + 2 delta)(1 + i eta) - omega^2)` against `1 / (i eta omega^2)` (ADR 0474 summary); measured 1.77e-2 at 2921 rad/s for `delta` 1.77e-4 |
| Probe numbers of the bar (cutoff 32964 rad/s, A 5 and B 9 modes, the vector of B): H1 largest 0.494 at 15107 rad/s; to 2000 rad/s 5.4e-4; statics 7.5e-12 | computed | `f552-scratch/probe` on the API of 4abff5fe (ADR 0474 arithmetic 2); measured 0.494, 5.39e-4, 1.5e-12 |

## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
