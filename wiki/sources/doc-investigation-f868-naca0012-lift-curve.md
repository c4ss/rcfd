---
type: source
tags: [investigation, naca0012, validation, spalart-allmaras, milestone]
sources: []
origin: docs/investigations/f868-naca0012-lift-curve/README.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Investigation F-868: the NACA 0012 lift curve against Ladson's measured lift

**Origin:** [docs/investigations/f868-naca0012-lift-curve/README.md](../../docs/investigations/f868-naca0012-lift-curve/README.md) at origin/main 2e94e5ef (50 lines).
**Kind:** investigation results folder (README + run output). Registration: [ADR 0760](../../docs/adr/0760-f868-registration-naca0012-lift-curve-against-ladson.md); set-up from the SA arm of [ADR 0167](../../docs/adr/0167-job49d-naca0012-aoa-sweep-with-the-sa-drag-fixes-at-500-chords.md).

## Summary

Steady Spalart-Allmaras on mesh M1w-far500 (256 x 128 O-grid, wall y+ about 40, far field at 500 chords), Re 6e6, incompressible: "the lift coefficient is within the registered band of Ladson's 80-grit lift at 0, 2, 4, 6, 8, 10, 12, 13, 14, 15 and 16 degrees, and misses at 17 degrees, where the steady iteration does not converge." Main 13b0937d reproduces ADR 0167's curve bit for bit at all twelve angles.

## Key points

- Band: `abs(cl - cl_L) <= max(0.05 abs(cl_L), 0.018)`, `cl_L` the 80-grit zone of `CLCD_Ladson_expdata.dat` (Ladson, NASA TM 4074, 1988, tripped, M 0.15, Re 6e6, from the NASA TMR; [validation-data.md](../../docs/validation-data.md) section `naca0012-tmr`) interpolated linearly; 5 % from ADR 0167's limits on cl, 0.018 "the largest spread of the file's three grit zones below the stall".
- Results (alpha: cl against Ladson, band, iterations (exit code)): 0: -0.0000 / -0.0072, 0.0180, 254 (0); 2: 0.2202 / 0.2071, 0.0180, 299 (0); 4: 0.4388 / 0.4272, 0.0214, 332 (0); 6: 0.6540 / 0.6448, 0.0322, 380 (0); 8: 0.8638 / 0.8557, 0.0428, 459 (0); 10: 1.0656 / 1.0586, 0.0529, 533 (0); 12: 1.2552 / 1.2493, 0.0625, 648 (0); 13: 1.3423 / 1.3384, 0.0669, 777 (0); 14: 1.4235 / 1.4189, 0.0709, 8000 (5); 15: 1.4932 / 1.4938, 0.0747, 8000 (5); 16: 1.5420 / 1.5563, 0.0778, 2008 (0); 17: 1.0476 / 1.6057, difference -0.5581, band 0.0803, 8000 (5), **not met**.
- At 17 degrees cl swings between -0.07 and 1.95 over the last 1000 iterations; 1.0476 is their mean "and not a flow value". The measured aerofoil is attached there (maximum lift 1.6116 at 17.13 degrees).
- Largest steady lift 1.5420 at 16 degrees, 4.3 % under the measured maximum (limit 5 %). Slope over 0 to 8 degrees 0.9855 x 2 pi; cl at 0 degrees -1.4e-6.
- At 13 and 16 degrees the runs met the residual targets but ADR 0167's window rule flags them, so the curve holds their window means (final iterates 1.3429 and 1.5436).
- Not compared and not run: 18, 20, 22 and 24 degrees. Not claimed: drag, moment, the stall angle, another mesh or model.
- Binary: a release build of main 13b0937d, sha256 `00848da1...2ec8b081`; twelve runs, one thread each, three at a time, 62 s to 41 minutes each. The runs are not in git; they are made again with `scripts/w49d_aoa_sweep_fixed.py` (`mesh`, `cases`, `run ... --parallel 3`) and `scripts/f868_naca0012_lift_curve.py collect`.
- Gate test: `crates/rcfd-api/tests/naca0012_lift_curve_f868.rs` (the band on this record, two negative controls, a fast solver run of the committed 0 and 4 degree cases on the coarse example mesh).

## Folder contents (not ingested)

- `results.json`: the evaluation, every run, and the text of every case.
- `cases/`: the twelve case files `SA-a00.toml` to `SA-a17.toml`, ADR 0167's SA arm as its driver writes it.

These evidence the result table above; the gate test reads this record.

## Contradictions with the wiki

None found. The numbers agree with [[adr-thread-simulation-milestones-validation]] and [[naca0012]].

## Pages this source informs

- [[naca0012]], [[spalart-allmaras]], [[simulation-milestones]], [[known-miss]]
- [[adr-thread-simulation-milestones-validation]], [[doc-reference-data-f868-ladson-naca0012]]
- [[doc-investigation-job49d-aoa-sweep-fixed]] (the sweep it reproduces)
- [[adr-thread-naca0012-sa-drag]] (the ADR thread of the NACA 0012 SA chain)
