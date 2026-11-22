---
type: source
tags: [reference-data, wave-section]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: Wave 32A: fan geometry and sector mesh (ADR 0065)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 32A: fan geometry and sector mesh (ADR 0065)" (lines 1373-1383) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 32A: fan geometry and sector mesh references (ADR 0065).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


Nothing in this wave is a validation against measured data.

| Item | What was read (source) | Result |
|---|---|---|
| NACA 4-digit thickness and camber formulas | the Wikipedia page "NACA airfoil" fetched 2026-10-18, which cites Moran, "Fundamentals of Aerodynamics" (2003) p. 7; the originals (Jacobs, Ward, Pinkerton, NACA TR-460, 1933) NOT read | coefficients 0.2969, -0.1260, -0.3516, 0.2843 and the closed-trailing-edge -0.1036 as on that page; the code uses -0.1036; the NACA 00xx closed-form area matches the quadrature to 1e-15 |
| Double circular arc section | constructed here from elementary geometry; no source | formulas stated in docs/meshing.md and checked by quadrature |
| Flat-plate friction `Cf = 0.0576 Re^(-1/5)` for the first-cell estimate | the power-law form used earlier in this repository (wave 7); not re-read | used only to size the first cell; the y+ of a converged solution was not measured |
| Published fan or blade data | none read | no comparison with a real fan was made; geometry and mesh are checked only against analytic formulas and conservation identities |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
- [[fan-01]]
