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

# Reference data: F-390 Hex8 without locking: B-bar and incompatible modes (ADR 0493)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-390 Hex8 without locking: B-bar and incompatible modes (ADR 0493)" (lines 2072-2087) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-390 Hex8 without locking (ADR 0493).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-390; the papers below are cited from memory for the methods (volumes, pages and years as recalled, not checked). Every formula used is derived in ADR 0493 (arithmetic 1 to 4) and checked by a plain numpy prototype (`f390-scratch/proto.py`, `est1.py` to `est3.py`, `mut_proto.py` and their logs), no rcfd code. The references of the checks are closed forms already in this file (the Timoshenko cantilever and Lame's cylinder of F-379, ADR 0433) and the exact linear field of the patch test.

| Item | Status | Source |
|---|---|---|
| Mean-dilatation B-bar Hex8: `Bbar = B + (1/3) m (bbar - grad N)^T`, `bbar` the volume mean of the shape-function gradients, `K = sum w det J Bbar^T D Bbar`; passes the patch test on any distortion | derived, cited | ADR 0493 arithmetic 1; T. J. R. Hughes, Int. J. Numer. Meth. Engng 15 (1980) 1413-1418; J. C. Nagtegaal, D. M. Parks and J. R. Rice, Comput. Methods Appl. Mech. Engng 4 (1974) 153-177: NOT read, from memory |
| Incompatible modes `1 - xi_k^2` (nine per element), condensed; the corrected gradients `(det J_0 / det J) J_0^{-T} dP/dxi` with the Jacobian at the centre, the enhanced strain orthogonal to constant stress, so the patch test holds on distorted elements (uncorrected: `3.7e-3` on the F-374 patch) | derived, cited | ADR 0493 arithmetic 2; E. L. Wilson, R. L. Taylor, W. P. Doherty and J. Ghaboussi, in Numerical and Computer Methods in Structural Mechanics, Academic Press, 1973, 43-57; R. L. Taylor, P. J. Beresford and E. L. Wilson, Int. J. Numer. Meth. Engng 10 (1976) 1211-1219; J. C. Simo and M. S. Rifai, Int. J. Numer. Meth. Engng 29 (1990) 1595-1638: NOT read, from memory |
| Pure bending of a rectangular brick with incompatible modes exact for any `nu` (`u_x = -kappa x z'`, `u_y = -nu kappa y' z'`, `u_z = (kappa/2)(x^2 + nu (z'^2 - y'^2))` in the element space) | derived | ADR 0493 arithmetic 3; measured `1.7e-11` (stress), `2.9e-12` (tip) on a one-layer strip |
| Parasitic shear of the fully integrated trilinear element in bending, energy ratio `(l/h)^2 / (2 (1 + nu))`, deflection about `2/3` at `l = h`, `nu = 0` | derived | ADR 0493 arithmetic 4; measured `0.6665` (20 x 1 x 1, L/h = 20) |
| One-layer cantilever L/h = 20 with incompatible modes `0.99913` (`nu = 0`), `0.99894` (`nu = 0.3`) of `P L^3/(3EI)(1 + 3EI/(kappa G A L^2))`; L/h = 100 at aspect ratio 5 `0.99927`; nearly incompressible Lame cylinder (`nu = 0.4999`, 8 x 16) B-bar bore `u_r` within `1.7e-3`, full integration `0.225` | computed | ADR 0493 addendum 1 (C1 to C4, T1, T2), equal to the prototype to the printed digits |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
