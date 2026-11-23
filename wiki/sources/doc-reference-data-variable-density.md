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

# Reference data: Variable-density validation (wave 10, ADR 0005)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Variable-density validation (wave 10, ADR 0005)" (lines 379-406) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Variable-density validation of wave 10 (ADR 0005) against Le Quere et al. (2005) values.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


Verified against the primary source:

- Le Quere, Weisman, Paillere, Vierendeels, Dick, Becker, Braack and Locke, "Modelling of natural convection flows with large temperature differences: a benchmark problem for low Mach number solvers. Part 1. Reference solutions", ESAIM: M2AN 39(3) 609-616 (2005), DOI 10.1051/m2an:2005027; full text read from the author's postprint https://wrap.warwick.ac.uk/id/eprint/3824/1/WRAP_Locke_modelling_natural_convection.pdf. Its Table 1, Sutherland law and thermodynamic pressure follow this list.
- Quasi-1-D isentropic relations and the area-Mach relation (A/A*)(M) = (1/M) [2/(gamma+1) (1 + (gamma-1)/2 M^2)]^((gamma+1)/(2(gamma-1))), choked mass flow A* p0 sqrt(gamma/(R T0)) (2/(gamma+1))^((gamma+1)/(2(gamma-1))): standard textbook formulae (for example Anderson, Modern Compressible Flow), not looked up in this session; the code of the checks uses them as written here.
- Hydrostatic ideal-gas columns: analytic atmospheres dp/dy = -rho g with p = rho R T (isothermal exponential; linear T polytropic with exponent g/(R a)), derived in the test headers.

Le Quere et al. Table 1 (square cavity, adiabatic horizontal walls, no slip, P0 = 101325 Pa, T0 = 600 K, R = 287, Pr = 0.71, gamma = 1.4, g = 9.81, epsilon = (Th-Tc)/(Th+Tc) = 0.6, Ra = Pr g rho0^2 (Th-Tc) L^3 / (T0 mu0^2) with a single g):

| Case | Ra | Properties | Nu | P/P0 |
|---|---|---|---|---|
| T1 | 1e6 | constant properties | 8.85978 | 0.856338 |
| T2 | 1e6 | Sutherland | 8.6866 | 0.924487 |
| T3 (not used) | 1e7 | Sutherland | 16.2410 | 0.92263 |

Their Sutherland law: mu = mu* (T/T*)^1.5 (T*+S)/(T+S), T* = 273 K, S = 110.5 K, mu* = 1.68e-5 Pa s, k = mu gamma R / ((gamma-1) Pr). Thermodynamic pressure P(t) = P0 integral(1/T0 dv) / integral(1/T dv).


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
- [[differentially-heated-cavity]]
