---
type: source
tags: [investigation, naca0012, convection-scheme, limiter, divergence]
sources: []
origin: docs/investigations/job49h-te-divergence/README.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Investigation job 49H: NACA 0012 trailing-edge divergence at fine wall spacing

**Origin:** [docs/investigations/job49h-te-divergence/README.md](../../docs/investigations/job49h-te-divergence/README.md) at origin/main 2e94e5ef (19 lines).
**Kind:** investigation results folder. Registration, amendments 1 to 3 and the addendum with results: [ADR 0171](../../docs/adr/0171-job49h-the-naca0012-trailing-edge-divergence-at-fine-wall-spacing.md).

## Summary

"The unlimited `linear_upwind` extrapolation diverges in the trailing-edge tip cells of the y+ 2 O-grid at 6 degrees (iteration 15). Pressure-velocity coupling, relaxation, turbulence model and mesh transform do not change it. The limiter setting `solver.linear_upwind_limiter = "barth_jespersen"` removes it in every run tried and costs 3 % of `cd_eff` at 0 degrees." Missed limits are ignored `*_known_miss` tests. ADR 0171's title names it as ADR 0159 question 5 and ADR 0164 queue item A1; job 48Y kept its wall cell at y+ about 40 "because finer wall cells made the trailing-edge cells of this O-grid diverge" ([[doc-investigation-job48y-aoa-sweep]]).

## Key points

- The LA0 and LB6 rows come from `monitors.csv` because "the status stream was lost with a scratch folder" (ADR 0171 deviations).
- Tests: `crates/rcfd-api/tests/te_divergence_w49h.rs`.
- Rebuild: `python scripts/w49h_te_divergence.py collect <work-folder> docs/investigations/job49h-te-divergence` (the work folder holds `runs/<id>`); the same script's `cases`, `run <id>` and `list`.
- ADR 0174 (job 49K) later applied this job's wall-cell metric to the FAN-01 meshes (folder `job49k-fan01-wall-metric`, no README).

## Folder contents (not ingested)

- `results.json`: every run's end state (clean at the iteration cap, or guard stop with iteration, cell and position), the full-length window means, and the evaluation of every registered limit (`evaluation.<id>.met`).
- `results.csv`: one row per short run.

## Contradictions with the wiki

None.

## Pages this source informs

- [[naca0012]], [[divergence-guard]], [[known-miss]], [[doc-case-schema-linear-upwind-limiter]]
- [[adr-thread-naca0012-sa-drag]] (the ADR thread of the NACA 0012 SA chain)
