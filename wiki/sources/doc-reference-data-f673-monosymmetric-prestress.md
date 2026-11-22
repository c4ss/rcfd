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

# Reference data: F-673 load height on the pre-stressed modes of a monosymmetric warping beam (ADR 0597)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-673 load height on the pre-stressed modes of a monosymmetric warping beam (ADR 0597)" (lines 2271-2282) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-673 load height on pre-stressed modes of a monosymmetric beam (ADR 0597).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-673. Every reference is derived in ADR 0597 (D2, D3, D5) and computed by a plain numpy script (`f673-scratch/arith.py`, `arith.log`), no rcfd code.

| Item | Status | Source |
|---|---|---|
| Kinetic energy of a section with the shear centre at $(0, z_0)$ from the centroid, in shear-centre unknowns: $(1/2) \int (\rho A (v_s + z_0 \phi)_t^2 + \rho I_z v_{s,t}'^2 + \rho I_p \phi_t^2) dx$, $I_p = I_y + I_z$ about the centroid (the coupling $\rho A z_0 v_s \phi$), no warping inertia | derived | ADR 0597 D2 |
| Rayleigh-Ritz frequencies, forks, $L = 3$, the ADR 0553 monosymmetric I, downward UDL $0.5 \lambda_{\mathrm{ref}} q_{\mathrm{ref}}$ (sine series $N = 81$, converged to $\le 4.3 \times 10^{-9}$ from $N = 41$): $\omega = 166.799139498357$ (shear centre), `161.354294924654` (centroid), `168.883949617271` rad/s (flange 1); unloaded $\omega_0^2 = 2.338537032326 \times 10^{4}$; without the coupling the frequencies are about `20 %` lower | computed | ADR 0597 D3, arithmetic 3 |
| Coupled Timoshenko-Rayleigh pinned frequency, mode 1, $\kappa = 5/6$: $\omega_T = 152.755814868645$ rad/s | derived (3 x 3 frequency equation of $v_s = \sin$, $\theta_z = \cos$, $\phi = \sin$) | ADR 0597 D5 |
| Measured, 16 elements: `prestressed_modes` `2.26e-3` below the references; the unloaded $\omega_0$ `5.2e-4` above $\omega_T$ (the mass interpolates the centroid translation from $(v_c, \theta_z)$, an $O(l^2)$ error: `1.3e-4` at 32, `3.2e-5` at 64 elements) | computed | ADR 0597 addendum 1 |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
