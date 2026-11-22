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

# Reference data: Wave 29A rotor interfaces and guarded secant (F-126, F-200, ADR 0055)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 29A rotor interfaces and guarded secant (F-126, F-200, ADR 0055)" (lines 1201-1218) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 29A rotor interfaces and guarded secant references (ADR 0055).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read in this wave. Every reference below is recalled from memory and is unverified. No vendor code, binary or install was used. No published turbomachine (NASA Rotor 37, Hannover compressor, any pump or fan) was run or compared: NOT ATTEMPTED.

| Item | Status | Note |
|------|--------|------|
| Frame velocity `Omega x r`, relative/absolute velocity `w = u - Omega x r` | derived | elementary kinematics; the transformation checks are exact vector identities (R1) |
| Frozen-rotor (fixed relative position) interface as the steady limit of a rotor-stator interface | unverified | the usual textbook description recalled from memory; the solver form is the absolute-velocity MRF of ADR 0008 |
| Mixing-plane / stage interface by circumferential averaging in radial bands, conserving mass, momentum and energy flux | unverified | the idea is recalled from the turbomachinery CFD literature (for example Denton's work on mixing); the formulas used here (u_n = Q/A, u_theta = Gamma/r, mixed-out pressure from normal momentum, flux-weighted scalar) are DERIVED in ADR 0055 D3 and checked against closed forms, not taken from a paper |
| Euler turbine equation (torque from the change of angular momentum flux) | unverified | recalled; used only to justify conserving angular momentum flux |
| Mixing loss `(A/2)(U sigma^2 + m3) >= 0` for uniform pressure and non-negative normal velocity | derived | from `<d^2 (d + U)> >= 0`; the two-jet closed form `(A/2) a(1-a)(u1-u2)^2 (U + (1-2a)(u1-u2))` was checked (3.3e-15) |
| 3-point Gauss-Legendre nodes and weights (exact to degree 5) | unverified | standard values recalled from memory; verified indirectly by the exact band integrals of X1/X4 (1e-15) |
| Free-vortex swirl `u_theta = Gamma_0/r` and solid-body swirl `Omega r` | derived | closed-form angular momentum fluxes integrated here |
| Taylor-Couette profile `A r + B/r` | derived | as in ADR 0008 |
| Guarded secant rule thresholds (`|dm| <= 0.5 |e|`, slope within 0.1 to 10 of the dynamic-head slope) | derived | chosen from the diagnosed cause of the plain-secant miss (spin-up contaminated slope), not tuned to the result; one case (F1) exercises it |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
