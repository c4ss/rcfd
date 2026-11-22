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

# Reference data: Wave 21B tabulated properties, actuator disk and regions references (F-220, F-218, F-216, ADR 0036)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 21B tabulated properties, actuator disk and regions references (F-220, F-218, F-216, ADR 0036)" (lines 861-876) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 21B tabulated properties, actuator disk and region references (ADR 0036).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


Web pages were consulted for the entries marked "verified"; no vendor documentation was consulted.

| Quantity | Source | Status |
| --- | --- | --- |
| Tait equation of the compressible liquid, `rho(p) = rho_ref / (1 - C ln((B + p)/(B + p_ref)))` (equivalent to the inverse used in `rcfd_core::tabular::Tait`) | NIST Thermodynamics Data Engine, Tait equation page (https://trc.nist.gov/TDE/TDE_Help_4_0/Eqns-Pure-DensityL/TaitEquation.htm) and the Wikipedia article "Tait equation" | verified secondary: the form of the relation; the water coefficients used in the tests are test inputs, not data claims |
| Vogel-Fulcher-Tammann viscosity `mu = a exp(b/(T - c))` | Wikipedia "Vogel-Fulcher-Tammann equation" (https://en.wikipedia.org/wiki/Vogel%E2%80%93Fulcher%E2%80%93Tammann_equation) | verified secondary: the equation; the name "Andrade form" for `c = 0` is UNVERIFIED; coefficients in tests are inputs |
| Steam and water values for the P-T table (saturation line, phase selection) | the IAPWS-IF97 module of this repository (`rcfd_core::iapws`), verified in wave 7 | verified as before; no new table data was introduced |
| Betz limit 16/27 = 0.593 at induction factor 1/3 | https://www3.nd.edu/~tcorke/w.WindTurbineCourse/Aerodynamics_Presentation.pdf (lecture notes) | verified secondary |
| Axial momentum theory: `T = 2 rho A u_d (u_d - u0)`, hover `v_h = sqrt(T/(2 rho A))`, wake velocity `2 v_h`, propulsive efficiency `2 u0/(u0 + u_w)`, figure of merit `T^{3/2}/sqrt(2 rho A)/P` | actuator-disk / momentum theory | DERIVED here from mass, momentum and energy conservation across the disk; not checked against a rotorcraft text; UNVERIFIED citation |
| Pressure jump across a disk in a duct `Delta p = T/A` | one-dimensional momentum balance | derived here |
| Kirchhoff transform for temperature-dependent conductivity, `K(T) = int k dT` linear in x | heat-conduction theory | derived here (ADR 0036 P2); no external source |
| Per-region laminar assignment as a face-viscosity mask | design decision of ADR 0036 | no external source; the equality with a laminar run (R1) is an implementation check |



## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
