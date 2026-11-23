---
type: source
tags: [investigation, naca0012, spalart-allmaras, k-omega-sst, stall]
sources: []
origin: docs/investigations/job49d-aoa-sweep-fixed/README.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Investigation job 49D: NACA 0012 sweep with the SA drag fixes, 500-chord far field

**Origin:** [docs/investigations/job49d-aoa-sweep-fixed/README.md](../../docs/investigations/job49d-aoa-sweep-fixed/README.md) at origin/main 2e94e5ef (164 lines).
**Kind:** investigation results folder. Set-up, limits and reading rules: [ADR 0167](../../docs/adr/0167-job49d-naca0012-aoa-sweep-with-the-sa-drag-fixes-at-500-chords.md).

## Summary

Repeats ADR 0159's sweep ([[doc-investigation-job48y-aoa-sweep]]) with ADR 0164's two opt-in fixes ([[doc-investigation-job49a-sa-drag]]): `reports.<n>.component = "total_effective"` and `boundaries.farfield.turbulence = "freestream"`, on mesh M1w-far500 (ADR 0164's F500w). "With both fixes, the lift slope over 0-8 degrees is 0.986 x 2 pi (Ladson 0.985) and SA's cl_max is 1.542 at 16 degrees (Ladson 1.612 at 17.13 degrees). Two limits are known misses; the others are met." At 17 degrees and above steady SA no longer converges, so the values there "are iteration means, not flow values". F-868 ([[doc-investigation-f868-naca0012-lift-curve]]) reproduces this curve bit for bit.

## Key points

- The case files spell out every key, "so they mean the same thing if the defaults change later". Binary: release build of f9a520a, sha256 `056669b7...c74d869`; 28 runs, one thread each, at most two at a time.
- Reading rule: "Steady" is D5's window rule; exit 0 takes the final iterate, exit 5 (8000-iteration cap) the window mean; an angle that is not steady takes its window mean, flagged.
- SA curve (cl / cd): 0: 0.0000 / 0.00840; 2: 0.2202 / 0.00854; 4: 0.4388 / 0.00901; 6: 0.6540 / 0.00988; 8: 0.8638 / 0.01130; 10: 1.0656 / 0.01352; 12: 1.2552 / 0.01695; 13: 1.3423 / 0.01943 (window rule); 14: 1.4235 / 0.02253; 15: 1.4932 / 0.02676; 16: **1.5420** / 0.03302 (window rule); 17: 1.0476 / 0.2997 (not steady, -0.07 to 1.95); 18: 0.9981; 20: 1.0846; 22: 1.2343; 24: 1.2748 (all not steady).
- Against Ladson: slope 0.1081 per degree = 0.986 x 2 pi (ADR 0159 gave 0.920); lift within 0.013 of Ladson's from 2 to 15 degrees and within 0.006 from 12 to 15; drag 0.0084 at 0 degrees against 0.0081, then increasingly above (+14 % at 10, +46 % at 15, +55 % at 16). "Ladson's tripped tunnel boundary layer is not the TMR fully-turbulent case, so TMR ... is the cleaner reference for drag."
- Against TMR SA (CFL3D): 0 degrees cd +2.5 % (LT0, 12 %, met); 10 degrees cd +9.8 %, cl -2.3 % (LT10, met); 15 degrees cd **+26.0 %** (band 25 %), cl -3.4 % (LT15, **known miss**). SA-a10 equals ADR 0164's C10 bit for bit (1599 values).
- Effect of each fix (500 chords): the effective force adds +0.003 to +0.005 in attached cd and does not change cl or the solution; the freestream role changes the solution (removes the outer eddy viscosity of 6e4 to 9e4 nu, pressure drag 0.0363 to 0.0075 at 10 degrees, cl +0.08 at 10 and +0.14 at 15, separation from 17 degrees); "either fix alone is wrong".
- SST (report only, both fixes; far field intensity 0.05, viscosity ratio 10): slope 0.980 x 2 pi, peak 1.509 at 16 degrees, first below 0.9 cl_max at 18.
- Limits: L1 met (28 runs, two N runs re-run after a PC restart); L2 met (0.986); L3 met (peak 16, first below 0.9 cl_max at 17); L4 met (cl(0) = -1.4e-6); LT0, LT10 met; LT15 **known miss**; LD **known miss** (drift over 0.5 % at 12, 13, 16, 17, 24); P2, B1 met. Tests in `crates/rcfd-api/tests/aoa_sweep_w49d.rs`.
- Rebuild: `scripts/w48y_aoa_sweep.py mesh ... M1w --far 500`, then `scripts/w49d_aoa_sweep_fixed.py` (`cases`, `run`, `collect --plots`).

## Folder contents (not ingested)

- `results.json`: every run and the evaluation; `results.csv`: one row per run; `histories.csv`: cl_eff and cd_eff, at most 61 samples per run.
- `cl_alpha.png`, `polar.png`, `histories.png`.

## Contradictions with the wiki

None. Agrees with the F-868 rows on [[naca0012]].

## Pages this source informs

- [[naca0012]], [[spalart-allmaras]], [[k-omega-sst]], [[known-miss]]
- [[adr-thread-simulation-milestones-validation]] (ADR 0760 takes its SA arm and its 5 % cl band from ADR 0167)
- [[adr-thread-naca0012-sa-drag]] (the ADR thread of the NACA 0012 SA chain)
