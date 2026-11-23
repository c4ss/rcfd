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

# Reference data: F-431 sparse L D L^T with a fill-reducing ordering (ADR 0447)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-431 sparse L D L^T with a fill-reducing ordering (ADR 0447)" (lines 1605-1619) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-431 sparse LDL^T with a fill-reducing ordering (ADR 0447).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-431; every method is cited from memory and checked by the residual, the fill count, the closed-form spectrum of the tridiagonal (2, -1) and the envelope solver of ADR 0274.

| Item | Status | Source |
|---|---|---|
| Approximate minimum degree on the quotient graph: elements and variables, element absorption (and aggressive absorption, $\lvert L_e \setminus L_p \rvert = 0$), the bound $\lvert A_i \rvert + \lvert L_p \setminus i \rvert + \sum_e \lvert L_e \setminus L_p \rvert$ of the external degree, supervariables found by a hash of $A_i$ and $E_i$ | from memory | Amestoy, Davis and Duff, "An approximate minimum degree ordering algorithm", SIAM J. Matrix Anal. Appl. 17 (1996) 886-905: NOT read; derived in ADR 0447 D1 (the old-degree bound and the dense-row rule left out) |
| Minimum degree, mass elimination and indistinguishable nodes | from memory | George and Liu, "The evolution of the minimum degree ordering algorithm", SIAM Review 31 (1989) 1-19; Liu, "Modification of the minimum-degree algorithm by multiple elimination", ACM Trans. Math. Softw. 11 (1985) 141-153: NOT read |
| Elimination tree (`parent(j)` = least $k > j$ with $l_{kj} \ne 0$) and the row structure of $L$ as the row subtree | from memory | Liu, ACM Trans. Math. Softw. 12 (1986) 127-148; Liu, SIAM J. Matrix Anal. Appl. 11 (1990) 134-172: NOT read; ADR 0447 D2 |
| Up-looking $L D L^T$ by rows with the tree reach as the pattern of row $k$ | from memory | Davis, "Algorithm 849: a concise sparse Cholesky factorization package", ACM Trans. Math. Softw. 31 (2005) 587-591: NOT read; derived in ADR 0447 D3 |
| Nested dissection of a grid (used only for an order-of-magnitude fill estimate before the registration) | from memory | George, SIAM J. Numer. Anal. 10 (1973) 345-363: NOT read; `w49i-work/f431-scratch/nd_estimate.py` |
| Eigenvalues of the tridiagonal (2, -1) of order n: $4 \sin^2(j \pi/(2 (n + 1)))$; $\lambda_j + \lambda_{n+1-j} = 4$ | derived | closed form (as F-382, ADR 0283); the symmetry made the registered middle shift of ADR 0447 A5 exactly 2 (amendment 1) |
| Sylvester's law of inertia: the negative pivots of $C = L D L^T$ count the negative eigenvalues of $C$ | derived | congruence (as ADR 0283 D8) |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
