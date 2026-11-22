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

# Reference data: F-556 the Shell3 rest stiffness of the twisted strip against Hex20 (ADR 0483)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-556 the Shell3 rest stiffness of the twisted strip against Hex20 (ADR 0483)" (lines 1963-1977) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-556 Shell3 rest stiffness of the twisted strip against Hex20 (ADR 0483).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-556; the papers below are cited from memory for the names. The torsion of a pretwisted strip is derived in ADR 0483 (arithmetic 2); the reference of every comparison is the Hex20 strip of ADR 0473 run by the existing solver, and the base numbers come from the F-420 run log.

| Item | Status | Source |
|---|---|---|
| Torsional stiffness of a pretwisted thin strip `G c t^3/3 + E t c^5 theta'^2/180` (the fibre strain `theta' phi' s^2`, free mean) | derived | ADR 0483 arithmetic 2; A. Rosen, "Structural and dynamic behavior of pretwisted rods and beams", Appl. Mech. Rev. 44 (1991) 483: NOT read, from memory |
| The base rest frequencies of the third track: Hex20 442.797470 Hz, Shell4 445.161813 Hz, Shell3 460.730207 Hz | computed | ADR 0473 E1/E2 run log (`f420-scratch/run_e1.txt`); reproduced to 8.3e-10 (C1) |
| Constant-strain triangle: parasitic in-plane shear when the strain varies across the element (in-plane bending) | cited | C. A. Felippa, "A study of optimal membrane triangles with drilling freedoms", Comput. Methods Appl. Mech. Engrg. 192 (2003) 2125: NOT read, from memory (as F-539) |
| ANDES optimal membrane triangle, Allman's triangle | cited | Felippa 2003, op. cit.; D. J. Allman, Comput. Struct. 19 (1984) 1: NOT read, from memory (implemented by ADR 0466) |
| MITC3 transverse shear, the stabilisation `t^2/(t^2 + alpha h^2)` | cited | P. S. Lee and K. J. Bathe, Comput. Struct. 82 (2004) 945; M. Lyly, R. Stenberg and T. Vihinen, Comput. Mech. 13 (1993) 92: NOT read, from memory (implemented by ADR 0454) |
| Measured: the CST membrane-energy excess `e_m = 9.16e-2` at 12 x 36 on the coupled torsion field, `O(h^2)` (ratios 0.257, 0.264 per halving); the ANDES Shell3 within 3.3e-4 of Hex20 at rest and 2.98e-3 over the Campbell table | computed | ADR 0483 addenda 1 and 2 |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
