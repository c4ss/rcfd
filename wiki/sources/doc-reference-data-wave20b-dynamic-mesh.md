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

# Reference data: Wave 20B dynamic mesh and six-degree-of-freedom references (F-063, F-229, ADR 0033)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 20B dynamic mesh and six-degree-of-freedom references (F-063, F-229, ADR 0033)" (lines 846-860) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 20B dynamic mesh and six-degree-of-freedom references (ADR 0033), unverified.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No external source was consulted in this wave; every entry below is from memory and UNVERIFIED unless it says derived.

| Quantity | Source | Status |
| --- | --- | --- |
| Geometric conservation law: cell volume change equals the sum of the face swept volumes | Thomas and Lombard (1979); Demirdzic and Peric (1988) | UNVERIFIED citation; the identity is checked to round-off by G1 to G3 |
| Time accuracy and the GCL of implicit schemes on moving meshes | Farhat, Geuzaine and Grandmont (2001) | UNVERIFIED citation, not used quantitatively |
| Variable-diffusivity Laplacian mesh motion (inverse distance) | Jasak and Tukovic (2006/2007) | UNVERIFIED citation; the diffusivity `1/l^k` is implemented as described in the ADR |
| Stokes second problem `u = U0 exp(-eta) cos(omega t - eta)`, `eta = y sqrt(omega/(2 nu))` | Schlichting, Boundary-Layer Theory | UNVERIFIED citation; the profile was checked by substitution into the diffusion equation |
| Newmark average acceleration (beta 1/4, gamma 1/2) | Newmark (1959) | UNVERIFIED citation; second-order convergence measured (ratio 3.99) |
| Euler's equations, axisymmetric torque-free precession `e^{+i Omega t}` with `Omega = (I3 - I1) w3 / I1` | rigid-body dynamics | derived here; the registered sign (`e^{-i Omega t}`) of the ADR was wrong |
| Piston pushing a fluid column, `v^2/2 = -(F/(rho A)) ln(1 - rho A x/m0)` | unsteady Bernoulli for plug flow | derived here (ADR 0033), no external source |
| Added-mass instability of explicit partitioned coupling | Causin, Gerbeau and Nobile (2005) | UNVERIFIED citation; not reproduced, the checks use a fluid-to-body mass ratio of 0.2 |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
