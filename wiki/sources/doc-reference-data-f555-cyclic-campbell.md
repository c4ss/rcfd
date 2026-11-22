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

# Reference data: F-555 the split Campbell table of a cyclic sector on the modal subspace (ADR 0481)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-555 the split Campbell table of a cyclic sector on the modal subspace (ADR 0481)" (lines 1802-1814) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-555 split Campbell table of a cyclic sector (ADR 0481).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-555; every item is cited from memory and derived in ADR 0481 (D1, D5). The references of the checks are the whole-sector whirl solve of ADR 0411 (`RotatingSector::whirl_modes`) and the F-396 sector paths (`RotatingSector::modes`, `campbell`).

| Item | Status | Source |
|---|---|---|
| A full-rotor matrix commuting with the sector shift enters index `k` as `T_k^T blockdiag(A_s, A_s) T_k`; for the Coriolis matrix the doubled `blockdiag(C_s, C_s)` | derived | ADR 0407 D1, ADR 0411 D5 |
| Projection on expanded harmonic modes with doubled matrices equals the reduced projection: `x^T A_2 y = q^T T_k^T A_2 T_k q'` | derived | ADR 0481 D1 |
| Ritz error of a gyroscopic mode on a subspace, first order: `eta = |e^H Q(omega) e| / (omega |s^H (2 omega M - i G) s|)`, `s` the M-projection of the exact mode, `e = phi - s`, `s^H Q s = e^H Q e` exactly | derived | the Rayleigh functional of the Hermitian family `Q(w) = K - w^2 M + i w G`; P. Lancaster, Lambda-Matrices and Vibrating Systems (Pergamon 1966), K. P. Hadeler, Arch. Rational Mech. Anal. 27 (1968) 306, B. N. Parlett, The Symmetric Eigenvalue Problem (Prentice-Hall 1980): NOT read, from memory (ADR 0481 D5) |
| Leading order of `eta` in modal coordinates: `sum_out 2 Omega^2 |C_ja|^2/(w_j^2 - omega^2)` | derived | ADR 0481 D5, ADR 0472 arithmetic 4 |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
