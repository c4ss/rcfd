# F-868: the NACA 0012 lift curve on current main against Ladson's measured lift

Steady Spalart-Allmaras on the mesh M1w-far500 (256 x 128 O-grid, wall y+ about 40, far field at 500 chords), Re 6e6,
incompressible: the lift coefficient is within the registered band of Ladson's 80-grit lift at 0, 2, 4, 6, 8, 10, 12, 13,
14, 15 and 16 degrees, and misses at 17 degrees, where the steady iteration does not converge. The set-up, the band and
the limits are in [ADR 0760](../../adr/0760-f868-registration-naca0012-lift-curve-against-ladson.md); the set-up is the SA
arm of [ADR 0167](../../adr/0167-job49d-naca0012-aoa-sweep-with-the-sa-drag-fixes-at-500-chords.md), and main 13b0937d
reproduces that ADR's curve bit for bit at all twelve angles.

Band: `abs(cl - cl_L) <= max(0.05 abs(cl_L), 0.018)`, with `cl_L` the 80-grit zone of `CLCD_Ladson_expdata.dat`
(Ladson, NASA TM 4074, 1988, tripped, M 0.15, Re 6e6, from the NASA Turbulence Modeling Resource; recorded in
[validation-data.md](../../validation-data.md), section `naca0012-tmr`) interpolated linearly, 5 % from ADR 0167's limits
on cl, and 0.018 the largest spread of the file's three grit zones below the stall.

| alpha | cl | Ladson 80 grit | difference | band | flow value | iterations (exit code) | met |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | -0.0000 | -0.0072 | +0.0072 | 0.0180 | yes | 254 (0) | yes |
| 2 | 0.2202 | 0.2071 | +0.0130 | 0.0180 | yes | 299 (0) | yes |
| 4 | 0.4388 | 0.4272 | +0.0116 | 0.0214 | yes | 332 (0) | yes |
| 6 | 0.6540 | 0.6448 | +0.0092 | 0.0322 | yes | 380 (0) | yes |
| 8 | 0.8638 | 0.8557 | +0.0081 | 0.0428 | yes | 459 (0) | yes |
| 10 | 1.0656 | 1.0586 | +0.0070 | 0.0529 | yes | 533 (0) | yes |
| 12 | 1.2552 | 1.2493 | +0.0058 | 0.0625 | yes | 648 (0) | yes |
| 13 | 1.3423 | 1.3384 | +0.0039 | 0.0669 | yes | 777 (0) | yes |
| 14 | 1.4235 | 1.4189 | +0.0045 | 0.0709 | yes | 8000 (5) | yes |
| 15 | 1.4932 | 1.4938 | -0.0006 | 0.0747 | yes | 8000 (5) | yes |
| 16 | 1.5420 | 1.5563 | -0.0143 | 0.0778 | yes | 2008 (0) | yes |
| 17 | 1.0476 | 1.6057 | -0.5581 | 0.0803 | **no** | 8000 (5) | **no** |

- At 17 degrees cl swings between -0.07 and 1.95 over the last 1000 iterations; 1.0476 is their mean and not a flow
  value. The measured aerofoil is attached there (maximum lift 1.6116 at 17.13 degrees).
- The largest steady lift is 1.5420 at 16 degrees, 4.3 % under the measured maximum (limit 5 %).
- The slope over 0 to 8 degrees is 0.9855 x 2 pi; cl at 0 degrees is -1.4e-6.
- At 13 and 16 degrees the runs met the residual targets, but the window rule of ADR 0167 flags them, so the curve
  holds their window means (final iterates 1.3429 and 1.5436).
- Not compared and not run: 18, 20, 22 and 24 degrees. Not claimed: drag, moment, the stall angle, another mesh or model.

Files: `results.json` (the evaluation, every run, and the text of every case) and `cases/` (the twelve case files, ADR
0167's SA arm as its driver writes it). Binary: a release build of main 13b0937d, sha256 `00848da1...2ec8b081`; twelve
runs, one thread each, three at a time, 62 s to 41 minutes each. The runs are not stored in git; they are made again by

```
python scripts/w49d_aoa_sweep_fixed.py mesh <rcfd> <work>
python scripts/w49d_aoa_sweep_fixed.py cases <work>
python scripts/w49d_aoa_sweep_fixed.py run <rcfd> <work> SA-a00 SA-a02 SA-a04 SA-a06 SA-a08 SA-a10 SA-a12 SA-a13 SA-a14 SA-a15 SA-a16 SA-a17 --parallel 3
python scripts/f868_naca0012_lift_curve.py collect <work> docs/investigations/f868-naca0012-lift-curve --binary <rcfd> --commit <sha>
```

The gate holds the comparison by `crates/rcfd-api/tests/naca0012_lift_curve_f868.rs`: the band on this record, two
negative controls, and a fast solver run of the committed cases of 0 and 4 degrees on the coarse example mesh.
