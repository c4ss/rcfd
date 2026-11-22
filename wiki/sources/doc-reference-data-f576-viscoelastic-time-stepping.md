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

# Reference data: F-576 quasi-static viscoelastic time stepping with histories, a temperature in the nonlinear transient integrator (ADR 0499)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-576 quasi-static viscoelastic time stepping with histories, a temperature in the nonlinear transient integrator (ADR 0499)" (lines 2088-2100) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-576 quasi-static viscoelastic time stepping (ADR 0499).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-576. The closed forms below are derived in ADR 0499 (arithmetic 3 to 6) from the hereditary law of ADR 0485 and checked by a plain python prototype of the uniaxial law (`f576-scratch/probe.py`, `probe_h1.py`), no rcfd code. Every modulus, relaxation time, load and shift constant is an input of a check.

| Item | Status | Source |
|---|---|---|
| Stress of a strain ramp `a = r t` on the total Lagrangian cube, `E_xx = a + a^2/2`: `S_xx = E_0 (r G_1 + r^2 G_2)`, `G_1 = w_inf t + sum w_i tau_i (1 - e^{-t/tau_i})`, `G_2 = w_inf t^2/2 + sum w_i tau_i (t - tau_i (1 - e^{-t/tau_i}))`; the error of the update at most `0.6 E_0 r^2 dt^2/8` | derived | ADR 0499 arithmetic 3 (i) |
| Creep of the standard solid under a stress ramp `sigma = s' t`: `eps = s' (t/E_inf - (1/E_inf - 1/E_0) tau_c (1 - e^{-t/tau_c}))`, `tau_c = tau E_0/E_inf` | derived | ADR 0499 arithmetic 3 (ii) |
| Relative residual of one Newton correction of the homogeneous stretch of one Hex8 under a load ramp (the halving set-up): `2.130e-2` for the step `[0, 1] s`, `1.000e-2` and `6.318e-3` for its halves | computed | ADR 0499 arithmetic 4 (`probe_h1.py`) |
| A uniform temperature `T` with a shift `a_T` equals the reference temperature with every relaxation time `a_T tau_i` (the reduced time `dxi = dt/a_T`) | derived | ADR 0499 arithmetic 5; ADR 0485 arithmetic 4 |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
