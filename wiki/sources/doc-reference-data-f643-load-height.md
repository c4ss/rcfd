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

# Reference data: F-643 load height and non-uniform moment for monosymmetric warping beams (ADR 0582)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-643 load height and non-uniform moment for monosymmetric warping beams (ADR 0582)" (lines 2233-2245) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-643 load height and non-uniform moment for warping beams (ADR 0582).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-643. The books are those of F-598 and F-627, cited from memory. Every reference used is derived in ADR 0582 (D1, D2, D4) and computed by a plain numpy script (`f643-scratch/arith.py`, `arith.log`), no rcfd code.

| Item | Status | Source |
|---|---|---|
| Load-height term of a dead force `F` at `p` on a section turning by `phi` about the shear centre `s`: `(1/2) (F . (p - s)) phi^2` (`-(1/2) P a phi^2` for a downward load `a` above the shear centre) | derived, cited | ADR 0582 D1; Timoshenko and Gere, Trahair, op. cit., ch. 6: NOT read, from memory |
| Second-order energy under a non-uniform moment in shear-centre unknowns: `int M phi v'' dx + (1/2) int beta_x M phi'^2 dx` | derived, cited | ADR 0582 D2; Timoshenko and Gere, Trahair, op. cit., ch. 6: NOT read, from memory |
| Rayleigh-Ritz references, forks, `L = 3`, central load `P_ref = 1000` N (`lambda`, sine series `N = 81`, converged to `<= 3.2e-6` from `N = 41`): doubly symmetric I at the shear centre `106.64131188`, top flange `76.653190682`, bottom flange `147.52392793`; ADR 0553 monosymmetric I at the shear centre `229.64455760` (larger flange compressed), `133.91738647` (in tension); at the centroid `285.61557187`, `106.14875777`; on flange 1 `210.84970872` | computed | ADR 0582 D4, arithmetic 3 |
| Doubly symmetric central-load coefficient `M_max/M_cr = 1.361` | computed; the textbook `1.35` to `1.36` cited from memory, not used | ADR 0582 D4 |
| Measured, 16 elements: `5.3e-4`, `3.5e-4`, `7.2e-4` (doubly symmetric at, above, below the shear centre); `1.4e-3`, `6.9e-4` (monosymmetric at the shear centre, both directions); all FE values below the Euler-Bernoulli reference (shear-flexible lateral plane) | computed | ADR 0582 addendum 1 |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
