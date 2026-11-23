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

# Reference data: Known misses (assertions added after the 2026-10-14 audit)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Known misses (assertions added after the 2026-10-14 audit)" (lines 429-452) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Table of registered criteria the code does not meet, with their ignored known-miss tests, after the 2026-10-14 audit.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


Each row is a registered criterion that the code does not meet. The threshold in each test is the registered one, not widened. Each test is `#[ignore]`d and its name ends in `known_miss`, so it fails by design when run with `--ignored` and will start passing if the solver improves. The values were measured in release with `CARGO_TARGET_DIR=target-fix2`; no solver parameter changed.

| Item | Registered criterion | Measured | Test |
|------|----------------------|----------|------|
| F-042 k-epsilon BFS, Realizable | x_r/h in 6.26 +- 0.10 (Driver and Seegmiller) | 6.402 (0.042 outside) | `rcfd-turb/tests/step_kepsilon.rs::step_reattachment_realizable_known_miss` |
| F-042 k-epsilon BFS, RNG | same | 6.373 (0.013 outside) | `step_reattachment_rng_known_miss` |
| F-042 k-epsilon BFS, Standard | same | 5.359 (0.80 below) | `step_reattachment_standard_known_miss` |
| F-061 nozzle case file | tolerance_u = tolerance_p = 1e-8 within 3000 iterations | res_u 2.62e-5, res_p 6.07e-13, not converged | `rcfd-api/tests/variable_density.rs::subsonic_nozzle_residual_target_1e8_known_miss` |
| F-043 SST flat plate vs White | Cf(0.97) within 5 % of 0.455/ln^2(0.06 Re_x) | 0.002663 vs 0.002875, -7.36 % (-6.6 to -8.5 % along the plate) | `rcfd-turb/tests/flat_plate_sst.rs::sst_flat_plate_cf_within_5pct_of_white_known_miss` |
| F-111 Sutherland air viscosity | within 1 % of Huber table, 300-500 K | -0.22, -1.08, -1.46 % | `rcfd-core/src/fluids.rs::sutherland_air_viscosity_within_1pct_300_to_500_k_known_miss` |

Met and now asserted, in `sst_flat_plate_cf` (ignored, heavy, about 6 minutes): F-043 Cf(0.97) within 5 % of the TMR page value 0.00268299 (-0.7 %) and of the TMR file value 0.0026909 (-1.0 %).

The air test that asserted 1.5 % for Sutherland viscosity was renamed `air_cp_conductivity_fit_laws_within_1pct_and_sutherland_viscosity_within_1p5pct_300_to_500_k`. The 1.5 % is a regression guard; the 1 % fit-law check is in-sample.



## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
- [[backward-facing-step]]
- [[flat-plate]]
