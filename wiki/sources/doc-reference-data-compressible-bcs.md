---
type: source
tags: [reference-data, model-constants]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: Compressible boundary conditions (F-062, ADR 0006)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Compressible boundary conditions (F-062, ADR 0006)" (lines 453-464) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Compressible boundary conditions (F-062, ADR 0006): citations confirmed, formulas unverified.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


| Value | Source | Result |
|---|---|---|
| Characteristic (LODI) boundary treatment, pressure-relaxation outlet $K\,(p - p_{ref})$ | Thompson, J. Comput. Phys. 68 (1987) 1-24; Poinsot and Lele, J. Comput. Phys. 101 (1992) 104-129 | bibliographic details confirmed via Crossref (wave 13); formulas re-derived in ADR 0006, not checked against the papers, unverified |
| Riemann-invariant far field, total-pressure inlet | Blazek, *Computational Fluid Dynamics: Principles and Applications*, ch. 8 | textbook formulas re-derived; book not accessed here, unverified |
| Isentropic and area-Mach relations (nozzle check) | Anderson, *Modern Compressible Flow* | standard textbook formulae, same as F-061 |
| Vassberg and Jameson, J. Aircraft 47(4) (2010) 1152-1166 (Euler NACA 0012 grid convergence) | listed in the reference list of arXiv:2407.03746 (fetched) | citation verified; its Cl, Cd and Cp values were NOT accessed |
| AGARD AR-211 NACA 0012 transonic Cp scatter | not accessed | no numbers available; the F-062 airfoil criterion is therefore not attempted (wave 31C: not attempted again; the incompressible NASA TMR experiment subsets were read instead, see the wave 31C section) |
| NACA 0012 M = 0.8, alpha = 1.25 degrees upper/lower shock positions (about x/c 0.62 / 0.34 in Euler solutions) | recollection only | UNVERIFIED, not used |



## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
