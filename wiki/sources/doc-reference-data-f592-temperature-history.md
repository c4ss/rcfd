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

# Reference data: F-592 a temperature history on the quasi-static driver and the nonlinear transient integrator (ADR 0526)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-592 a temperature history on the quasi-static driver and the nonlinear transient integrator (ADR 0526)" (lines 2101-2113) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-592 temperature history on the quasi-static and transient drivers (ADR 0526).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-592. The closed forms below are derived in ADR 0526 (arithmetic 2 to 5) from the reduced time of ADR 0485 and checked by a plain python prototype of the uniaxial law (`f592-scratch/probe.py`), no rcfd code. Every modulus, relaxation time, temperature and shift constant is an input of a check.

| Item | Status | Source |
|---|---|---|
| Reduced time of the step-end rule: `xi_n = sum_k (t_k - t_{k-1})/a_T(T(t_k))`, the right-endpoint rule of `int ds/a_T(T(s))`; exact under a held strain when the temperature steps on step ends, first order under a ramp | derived | ADR 0526 arithmetic 2 |
| Relaxation under a temperature step `T_1 -> T_2` at `t_s`: `S_xx = E_xx E_0 (w_inf + sum w_i exp(-xi/tau_i))`, `xi = min(t, t_s)/a_1 + max(t - t_s, 0)/a_2` | derived | ADR 0526 arithmetic 3 |
| WLF (`c1 = 17.44`, `c2 = 51.6 K`, `T_ref = 373 K`): `a_T(383) = 1.475132967e-3`, `a_T(393) = 1.344285828e-5` | computed | ADR 0526 arithmetic 3 (`probe.py`) |
| Ramp `383 -> 393 K` to `xi = 2` (`t_end = 1.167445e-4 s`), three branches: right-endpoint error of `g(xi)` `6.930e-3`, `3.488e-3`, `1.748e-3` at 50, 100, 200 steps | computed | ADR 0526 arithmetic 5 (`probe.py`) |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
