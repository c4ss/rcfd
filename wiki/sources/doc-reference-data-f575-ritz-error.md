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

# Reference data: F-575 the second-order Ritz error of the corrected sector split modes (ADR 0492)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-575 the second-order Ritz error of the corrected sector split modes (ADR 0492)" (lines 1827-1838) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-575 second-order Ritz error of corrected sector split modes (ADR 0492).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-575; every item is cited from memory and derived in ADR 0492 (D1 to D3). The references of the checks are the whole-sector whirl solve of ADR 0411 (`RotatingSector::whirl_modes`) and a test-side dense computation of the same condensed problem (bordered Gaussian elimination instead of MINRES).

| Item | Status | Source |
|---|---|---|
| Exact condensation of the projected problem by the complement: `S(w) = Q_VV(w) - Q_VW(w) Q_WW(w)^-1 Q_WV(w)`, `S_ij = v_i^T Q (v_j + x_j)` with the complement solve `(I - P)^T Q (I - P) x_j = -(I - P)^T Q v_j`; an exact mode satisfies `S(omega) y = 0` | derived | block elimination (Schur complement), textbook linear algebra, e.g. G. H. Golub, C. F. Van Loan, Matrix Computations: NOT read, from memory (ADR 0492 D1) |
| `S` frozen at the Ritz value `theta`, the skew part of `E` folded into the gyroscopic matrix at `l = theta`: the estimate `|theta/theta' - 1|` differs from the measured error by a relative amount of the order of the error times `2 theta^2/(w_c^2 - theta^2) + O(1)` | derived | ADR 0492 D3; eigenvalue-dependent condensation in dynamic substructuring, R. J. Guyan, AIAA J. 3 (1965) 380 for the static limit: NOT read, from memory |
| On DS B4 (modes plus static correction vectors) at 0.4 and 0.8 `omega_ref` the estimate matches the measured error of the 34 matched modes above 1e-9 within 3.4e-4 (ADR 0482's first-order `eta` within a factor 2.62) | measured | ADR 0492 addendum 1 (`f575-scratch/run1.log`) |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
