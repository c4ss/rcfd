---
type: source
tags: [reference-data, f-item-section]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: F-346 immersed solid: force and moment in unsteady flow (ADR 0249)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-346 immersed solid: force and moment in unsteady flow (ADR 0249)" (lines 1468-1479) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-346 immersed solid force and moment in unsteady flow (ADR 0249).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


Opt-in (`ImmersedForcing::force_moment_unsteady`, `immersed::velocity_rate`); `force_moment` and `force_moment_second_order` are unchanged.

| Item | Status | What was read (source) |
|---|---|---|
| The force on a body from the momentum balance of a fixed control volume (Batchelor, An Introduction to Fluid Dynamics, 1967, section 3.2), and the hydrodynamic force on a body represented by a volume forcing as the reaction of the forcing plus the rate of change of the momentum of the fictitious fluid inside it (Uhlmann, J. Comput. Phys. 209 (2005) 448-476; Lai and Peskin, J. Comput. Phys. 160 (2000) 705-719; Balaras, Comput. Fluids 33 (2004) 375-404) | from memory | NOT read; ADR 0249 D1 derives on rcfd's discrete momentum equation that the storage to add back is that of the solid cells and that the first fluid cells do not enter |
| Inner cylinder accelerated with $\Omega = \alpha t$ in a fixed outer one: $u_\theta = \alpha t (A r + B/r) + w(r)$, $w = \frac{\alpha}{\nu}\left(\frac{A r^3}{8} + \frac{B}{2} r \ln r\right) + C r + D/r$, moment on the inner cylinder per unit depth $-4\pi\mu B \alpha t + 2\pi R_1^2 \mu \left[\frac{\alpha}{\nu}\left(\frac{A R_1^2}{4} + \frac{B}{2}\right) - \frac{2D}{R_1^2}\right]$ ($-4.188790\, t - 0.123934$ for $R_1 = 0.5$, $R_2 = 1$, $\alpha = \nu = \rho = 1$) | derived | ADR 0249 amendment 1 (X1b); measured error 0.167 and 0.078 percent at N 48 and 96, the reaction alone 4.3 and 4.4 percent |
| Plate accelerated with $U = a t$ under a wall at rest at distance $d$: $u = a t (1 - \eta/d) + \frac{a}{\nu}\left(\frac{\eta^2}{2} - \frac{\eta^3}{6d} - \frac{d\eta}{3}\right)$, force on the plate per area $-(\mu a t/d + \rho a d/3)$ | derived | ADR 0249 (X2, X2b); measured error 0.056 and 0.014 percent at N 16 and 32, the reaction alone 42.8 percent |
| DFG 2D-2 level-6 values | read | as the F-219 row above (max $C_D$ 3.2200, max $C_L$ 0.9859); measured values in ADR 0249 addendum 1 |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
- [[dfg-cylinder-benchmark]]
