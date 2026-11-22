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

# Reference data: F-569 static correction vectors for the split modes of a sector index (ADR 0482)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-569 static correction vectors for the split modes of a sector index (ADR 0482)" (lines 1815-1826) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-569 static correction vectors for sector split modes (ADR 0482).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-569; every item is cited from memory and derived in ADR 0482 (D1, D2). The references of the checks are the whole-sector whirl solve of ADR 0411 (`RotatingSector::whirl_modes`) and the F-555 sector split path without the option.

| Item | Status | Source |
|---|---|---|
| Static correction vector $K^{-1} G \phi_a$ of a kept mode carries the leading (static) part of every out-of-basis error component $b_j = -i \omega G_{ja}/(w_j^2 - \omega^2)$; the rest is smaller by $\omega^2/w_j^2$ | derived | modal expansion of $K^{-1}$ (ADR 0482 D1); J. M. Dickens, J. M. Nakagawa, M. J. Wittbrodt, Computers and Structures 62 (1997) 985 (modal truncation augmentation): NOT read, from memory |
| Ritz error from the split mode alone: $\eta = \lvert t \cdot b \rvert / (\theta \lvert 2 \theta \psi^H M \psi + 2 \psi_r^T G \psi_i \rvert)$, $t$ the solution of the complement-projected correction equation $(I - P)^T Q(\theta) (I - P) t = -(I - P)^T Q(\theta) \psi$ | derived | Schur complement of the projected problem, first order (ADR 0482 D2, addendum); G. L. G. Sleijpen, H. A. van der Vorst, SIAM J. Matrix Anal. Appl. 17 (1996) 401 (correction equation): NOT read, from memory |
| MINRES with a positive definite preconditioner | from memory | C. C. Paige, M. A. Saunders, SIAM J. Numer. Anal. 12 (1975) 617: NOT read; the recurrence written out in `gyro.rs` |
| On a basis that is not invariant under $K^{-1} M$ (modes plus correction vectors) the first-order estimate from the Ritz vector falls below the measured error, and ADR 0481's $\eta$ from the exact mode above it, by up to a factor 2.6 each (DS B4); the ratio of the squared error to their product lies in `[0.898, 1.001]` over the 34 matched modes with $\eta > 10^{-9}$ (an observation, not derived) | measured | ADR 0482 addendum 1 (diagnostic `f569-scratch/diag1.log`) |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
