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

# Reference data: MRF zones (F-031, F-114, ADR 0008, `crates/rcfd-api/tests/mrf.rs`)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "MRF zones (F-031, F-114, ADR 0008, `crates/rcfd-api/tests/mrf.rs`)" (lines 496-523) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

MRF zones (F-031, F-114, ADR 0008): analytic Taylor-Couette checks M1 to M6 and their misses.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


All reference values are analytic (solid-body rotation, Taylor-Couette flow `u_theta = A r + B/r`, torque `4 pi rho nu W_i r_i^2 r_o^2/(r_o^2 - r_i^2)` per unit height) and derived in the ADR. No literature data and no pump or turbine benchmark are used or claimed. The set-up is r_i 0.5, r_o 1, nu 0.1, 48 cells around. Limits were registered before the runs; misses are ignored `*_known_miss` tests.

| Check | Measured | Limit | Result |
|---|---|---|---|
| M1 solid body, velocity | 7.84e-3 W r_o | 2e-2 | pass |
| M1 pressure | 7.55e-2 of range | 5e-2 | MISS |
| M2 single MRF zone, velocity (symmetry ends) | 6.76e-3 V_i | 1.5e-2 | pass |
| M2 torque of the `moment` report vs full-stress value | -37.8 % | 3 % | MISS (report omits the transpose term) |
| M2b open ends, velocity / torque vs report value | 6.61e-3 V_i / -0.70 % | 1.5e-2 / 3 % | pass |
| M3 frames 0, 0.5, 1, 2 W_i, velocity; difference to stationary | 6.76e-3 V_i; 4.3e-4, 8.8e-4, 1.7e-3 V_i | 1.5e-2; 1e-2 | pass |
| M3(a) registered stationary frame with no source: velocity / spurious / torque / difference to MRF frames | 3.06e-2 V_i / 1.39e-3 V_i / -27.3 % / 2.57e-2 V_i | 1.5e-2 / 1e-3 / 3 % / 1e-2 | MISS (kernel without H correction) |
| M3 spurious speed | 1.41e-3 (Omega 0), 9.7e-4, 5.3e-4, 3.2e-4 V_i | 1e-3 | MISS at Omega 0 only |
| M3b open ends: velocity, torque, difference; spurious | 6.6e-3, -0.70 %, <= 1.8e-3 V_i; 8.2e-3 to 8.6e-3 V_i | 1.5e-2, 3 %, 1e-2; 1e-3 | pass; spurious MISS |
| M4 across the non-conformal interface, velocity / torque | 7.92e-3 V_i / -37.8 % | 2e-2 / 3 % | pass / MISS (report) |
| M4b open ends, velocity / torque | 7.59e-3 V_i / -0.87 % | 2e-2 / 3 % | pass |
| M5 continuity / net flux / sum abs flux (symmetry ends) | 5.2e-12 / 3.3e-14 / 8.9e-4 | 1e-8 / 1e-8 / 1e-3 | pass |
| M5 in M4b (open ends) net flux / sum abs flux | 3.2e-5 / 1.0003e-3 | 1e-8 / 1e-3 | MISS |
| M6 through-flow: in/out, interface flux, swirl, axial deviation | exact to 1e-10; 2.0e-4; 9.8e-3 | 1e-9; 2e-2; 2e-2 | pass |

The first runs (before the H-operator component-diagonal correction) missed M1-M5 by large margins; see the ADR disclosures. The correction (`SimpleSolver::component_diag_correction`) is enabled only for cases with an MRF source.



## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
