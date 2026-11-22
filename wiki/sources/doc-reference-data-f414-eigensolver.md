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

# Reference data: F-414 general real eigensolver, quadratic eigenproblem, damped modes (ADR 0450)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-414 general real eigensolver, quadratic eigenproblem, damped modes (ADR 0450)" (lines 1620-1637) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-414 general real eigensolver, quadratic eigenproblem, damped modes (ADR 0450).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-414; every method is cited from memory and checked by residuals and closed forms (the companion matrix of a polynomial with registered roots, the Jacobi values of F-382, Jordan blocks, the oscillator, the gyroscopic state solver of F-505 as an independent cross-check).

| Item | Status | Source |
|---|---|---|
| Balancing by powers of 2 (scaling part only, the 0.95 acceptance test) | from memory | Parlett and Reinsch, "Balancing a matrix for calculation of eigenvalues and eigenvectors", Numer. Math. 13 (1969) 293-304: NOT read; ADR 0450 D1.1 |
| Householder reduction to Hessenberg form with the orthogonal factor accumulated | from memory | Golub and Van Loan, Matrix Computations, section 7.4: NOT read; ADR 0450 D1.2 |
| Francis implicit double-shift QR, bulge chase by 3-element reflections, deflation test `|h_{k,k-1}| <= eps (|h_{k-1,k-1}| + |h_kk|)` | from memory | Francis, "The QR transformation", Computer J. 4 (1961-62); Golub and Van Loan section 7.5: NOT read; ADR 0450 D1.3 |
| Exceptional shifts at the 10th and 20th iteration (sum `1.5 s`, product `s^2`, `s = |h_{hi,hi-1}| + |h_{hi-1,hi-2}|`); cap 30 iterations per eigenvalue | from memory | Wilkinson and Reinsch, Handbook for Automatic Computation II (1971), procedure hqr: NOT read; ADR 0450 D1.3; the cyclic permutation of order 6 needs them (check G6, mutation MU1) |
| Eigenvectors by back-substitution on the quasi-triangular Schur factor, small divisors replaced by `eps ||T||_F` | from memory | Golub and Van Loan section 7.6; the device of hqr2 (Handbook II): NOT read; ADR 0450 D1.5 |
| Damped oscillator `lambda = -zeta w +/- i w sqrt(1 - zeta^2)`; proportional damping `zeta_j = (a/w_j + b w_j)/2`; skew-coupled pair `w^4 - (k1 + k2 + g^2) w^2 + k1 k2 = 0` | derived | closed forms, ADR 0450 |
| Bar with a tip dashpot: `tanh(lambda L/c0) = -A sqrt(E rho)/c`; for `c < Z` `lambda_j = (c0/L)(-atanh(c/Z) + i (2j - 1) pi/2)`, for `c > Z` `lambda_j = (c0/L)(-atanh(Z/c) + i j pi)`, `j = 0` real (the roadmap's draft said no oscillating root above `Z`: corrected) | derived | wave equation with `E A u_x = -c u_t`, ADR 0450; truncation to `N` fixed-free modes estimated from the partial fractions `tanh s = sum 2 s/(s^2 + ((2k-1) pi/2)^2)` |
| Constant loss factor in the eigenproblem: `lambda = +/- i w sqrt(1 + i eta)`, the root with `Im > 0` decaying | derived | complex stiffness `(1 + i eta) K` (as F-412), ADR 0450 |
| Eigenvalues of a complex matrix from the real embedding `[[A_r, -A_i], [A_i, A_r]]`, the half with `u + i v != 0` | derived | ADR 0450 D2 |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
