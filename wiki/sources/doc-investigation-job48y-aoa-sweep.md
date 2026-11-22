---
type: source
tags: [investigation, naca0012, spalart-allmaras, k-omega-sst, stall]
sources: []
origin: docs/investigations/job48y-aoa-sweep/README.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Investigation job 48Y: NACA 0012 angle-of-attack sweep through stall

**Origin:** [docs/investigations/job48y-aoa-sweep/README.md](../../docs/investigations/job48y-aoa-sweep/README.md) at origin/main 2e94e5ef (94 lines).
**Kind:** investigation results folder. Set-up, limits, two amendments and evaluation: [ADR 0159](../../docs/adr/0159-job48y-naca0012-angle-of-attack-sweep-through-stall.md).

## Summary

"At the measured Reynolds number, the two RANS models on mesh M1w bracket the measured stall angle: SST stalls about 3 degrees early and SA about 3 degrees late. All four limits (L1 to L4) are met, and L3 finds stall only after the registered extension. SA's drag is far too high: from 4 degrees up the SA arm is 1.4 to 4.6 times Ladson's drag." The SA-DDES mean "is reported, not adopted". Later diagnosed by job 49A ([[doc-investigation-job49a-sa-drag]]) and repeated with the fixes by job 49D ([[doc-investigation-job49d-aoa-sweep-fixed]]).

## Key points

- Flow: NACA 0012 at Re 6e6 (U = c = 1, nu = 1/6e6); angle set by `[mesh.transform] rotate_z_deg = -alpha` about the quarter chord.
- Mesh M1w: 256 x 128 O-grid, 32 768 cells, from the `cylinder_ogrid` generator warped onto the aerofoil; far field a circle of 50 chords; first cell 3.5e-4 chord, wall y+ about 40. Finer wall cells made the trailing-edge cells diverge (amendment 1).
- Arms: SA steady (primary, 8000 iterations, mean of the last 1000); SST steady (report only, 8 angles, 4000 iterations); SA-DDES transient (PISO with adaptive correctors, dt 0.01 to t = 40, mean over [20, 40]); SA2 on the 384 x 192 grid M2w at 6 and 14 degrees.
- SA results: slope 0.1009 per degree, 0.920 x 2 pi (band 0.90 to 1.05; Ladson 0.985); peak cl 1.5278 at 20 degrees; 22 degrees is 0.913 of the peak, 24 degrees 0.788; cl(0) -2e-11. Against Ladson (cl_max 1.612 at 17.1 degrees, then 1.00 at 18.0) SA stalls about 3 degrees late with cl_max 5 % low.
- SST: slope 0.970 x 2 pi; peak 1.3973 at 14 degrees, about 3 degrees early and 13 % low, not steady from 16 degrees on; drag close to Ladson's (0.0083 against 0.0103 at 8 degrees).
- SA-DDES: at 18 degrees cl 1.343, cd 0.0666; at 20 degrees cl 0.864, sheds at a chord Strouhal number of 0.24, half-window drift -0.21 so "20 time units do not give a settled mean".
- Grid check M2w against M1w: at 6 degrees cl +0.8 %, cd -3.5 %; at 14 degrees cl +0.3 %, cd -3.6 %; M2w at 14 degrees is not steady by D4 (cd range 8.8e-4 above 5e-4).
- Limits: L1 28 of 28 counted runs complete; L2 met; L3 met after the registered extension; L4 met.
- Rebuild: `scripts/w48y_aoa_sweep.py` (`mesh`, `cases`, `run`, `plan`, `collect --plots`) in an empty work folder outside the repository.

## Folder contents (not ingested)

- `results.json`: every counted and smoke run (exit code, state, wall time, warnings, window statistics) and the evaluation (sweep curve, L1-L4, DDES plan, SST curve, grid check, Ladson comparison).
- `results.csv`: one row per run; `histories.csv`: cl and cd of every run, at most 101 samples each.
- `cl_alpha.png`, `polar.png`, `histories.png`: cl(alpha) and the drag polar against Ladson 80 grit; SA-DDES lift and SST iteration histories.

## Contradictions with the wiki

None with the wiki. The SA drag and the late stall are superseded: [[doc-investigation-job49a-sa-drag]] attributes the drag to the force report and the far-field nuTilda, and [[doc-investigation-job49d-aoa-sweep-fixed]] gives slope 0.986 x 2 pi and cl_max at 16 degrees with both fixes.

## Pages this source informs

- [[naca0012]], [[spalart-allmaras]], [[k-omega-sst]]
