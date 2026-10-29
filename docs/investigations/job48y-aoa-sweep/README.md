# NACA 0012 angle-of-attack sweep through stall (job 48Y, ADR 0159)

The set-up, the limits, the two amendments and the evaluation are in
[ADR 0159](../../adr/0159-job48y-naca0012-angle-of-attack-sweep-through-stall.md). This folder holds the results only.
No run, mesh or field store is kept in git. To build the runs again, use `scripts/w48y_aoa_sweep.py` in an empty work folder
outside the repository:

```
cargo build --release --locked -p rcfd-cli
python scripts/w48y_aoa_sweep.py mesh target/release/rcfd.exe <work> M1w M2w
python scripts/w48y_aoa_sweep.py cases <work>
python scripts/w48y_aoa_sweep.py run target/release/rcfd.exe <work> SA-a00 SA-a02 ... --parallel 4
python scripts/w48y_aoa_sweep.py plan <work>        # which DDES and extension runs the rules ask for
python scripts/w48y_aoa_sweep.py collect <work> docs/investigations/job48y-aoa-sweep --plots
```

## Files

| File | Contents |
|---|---|
| `results.json` | Every counted run and smoke run: exit code, state, wall time, warnings and window statistics. Also the evaluation: the sweep curve, L1-L4, the DDES plan, the SST curve, the grid check and the Ladson comparison |
| `results.csv` | One row per run (window mean, min and max of cl and cd; steady or reached; correctors; wall time) |
| `histories.csv` | cl and cd of every run, at most 101 samples per run |
| `cl_alpha.png`, `polar.png`, `histories.png` | cl(alpha) and the drag polar against Ladson (1988, 80 grit); the SA-DDES lift histories and the SST iteration histories in their windows |

## Set-up in brief

- **Flow and geometry.** NACA 0012 at Re 6e6 (U = c = 1, nu = 1/6e6). The angle of attack is set by
  `[mesh.transform] rotate_z_deg = -alpha` about the quarter chord, with the flow along +x.
- **Mesh M1w.** A 256 x 128 O-grid, 32 768 cells. It comes from the `cylinder_ogrid` generator, warped onto the aerofoil
  with grid lines that leave the wall along its normal. The far field is a circle of 50 chords. The first cell is 3.5e-4
  chord, a wall y+ of about 40.
- **Arms.**
  - SA: steady Spalart-Allmaras, the primary arm. 8000 iterations; the value is the mean over the last 1000.
  - SST: steady k-omega SST, report only, at 8 angles. 4000 iterations.
  - SA-DDES: transient (PISO with adaptive correctors), dt 0.01 to t = 40. The value is the time mean over [20, 40].
  - SA2: SA on the 384 x 192 grid M2w at 6 and 14 degrees.

## Results (SA unless stated; Ladson interpolated to the same angle)

| alpha | cl | cd | Ladson cl | Ladson cd | SST cl | SST cd |
|---|---|---|---|---|---|---|
| 0 | 0.0000 | 0.0037 | -0.007 | 0.0081 | 0.0000 | 0.0036 |
| 2 | 0.2138 | 0.0076 | 0.207 | 0.0082 | | |
| 4 | 0.4203 | 0.0118 | 0.427 | 0.0082 | 0.4322 | 0.0047 |
| 6 | 0.6185 | 0.0182 | 0.645 | 0.0088 | | |
| 8 | 0.8062 | 0.0274 | 0.856 | 0.0103 | 0.8506 | 0.0083 |
| 10 | 0.9817 | 0.0396 | 1.059 | 0.0119 | | |
| 12 | 1.1431 | 0.0549 | 1.249 | 0.0132 | 1.2346 | 0.0162 |
| 13 | 1.2175 | 0.0637 | 1.338 | 0.0149 | | |
| 14 | 1.2872 | 0.0734 | 1.419 | 0.0160 | **1.3973** (SST peak) | 0.0232 |
| 15 | 1.3516 | 0.0839 | 1.494 | 0.0183 | | |
| 16 | 1.4095 | 0.0953 | 1.556 | 0.0213 | 0.886 (not steady, 0.16 to 1.47) | 0.251 |
| 17 | 1.4594 | 0.1078 | 1.606 | 0.0251 | | |
| 18 | 1.4988 | 0.1214 | (stalled, 1.01 at 18.02) | | 0.968 (not steady, -0.37 to 1.82) | 0.300 |
| 20 | **1.5278** (SA peak) | 0.1560 | | | 0.946 (not steady, -0.42 to 1.87) | 0.339 |
| 22 | 1.3951 | 0.2175 | | | | |
| 24 | 1.2039 | 0.3068 | | | | |

- **SA-DDES.**
  - At 18 degrees: cl 1.343 (1.333 to 1.380), cd 0.0666. That is 0.156 below steady SA, which is attached there.
  - At 20 degrees: cl 0.864 (0.144 to 1.673), cd 0.390. The run sheds at a chord Strouhal number of 0.24 (4 periods in the
    window). Its half-window drift is -0.21 in cl, so 20 time units do not give a settled mean.
  - Correctors: 2 to 3 (mean 2.75 and 2.25), never at the cap.
- **Limits.** All are met:
  - L1: 28 of 28 counted runs complete.
  - L2: the slope is 0.1009 per degree, 0.920 x 2 pi (band 0.90 to 1.05; Ladson 0.985).
  - L3: stall is found only after the registered extension. Steady SA peaks at **20 degrees** (cl_max 1.528), 22 degrees is
    0.913 of the peak and 24 degrees is 0.788.
  - L4: cl(0) is -2e-11.
- **Against Ladson** (cl_max 1.612 at 17.1 degrees, then 1.00 at 18.0). Steady SA on this mesh stalls about 3 degrees
  late, with cl_max 5 % low, and its lift falls gradually after the peak (0.91 of it at 22 degrees, 0.79 at 24) where
  the experiment drops abruptly. Its drag is 1.4 to 4.6 times the
  measured drag from 4 degrees up, and it grows steadily with alpha.
- **SST (report only).** Slope 0.970 x 2 pi over 0, 4 and 8 degrees. It peaks at **14 degrees** (cl 1.397), about
  3 degrees early and 13 % low, and is not steady from 16 degrees on. Its drag up to 14 degrees is close to Ladson's:
  0.0083 against 0.0103 at 8 degrees, 0.023 against 0.016 at 14.
- **Grid check (M2w against M1w).** At 6 degrees cl is +0.8 % and cd -3.5 %. At 14 degrees cl is +0.3 % and cd -3.6 %.
  M2w at 14 degrees is not steady by D4: the cd range is 8.8e-4, above the 5e-4 limit.

## What this does and does not show

At the measured Reynolds number, the two RANS models on this mesh bracket the measured stall angle: SST stalls about
3 degrees early and SA about 3 degrees late. The lift slope is within 8 % of thin-aerofoil theory for SA and 3 % for SST.
SA's drag is far too high: from 4 degrees up the SA arm is 1.4 to 4.6 times Ladson's drag, while SST is close. This needs
its own investigation (ADR 0159, question 4). A transient SA-DDES run at 20 degrees separates even though steady SA is
attached there. A 2D DDES of a stalled aerofoil is not a validated stall model, so its mean is reported, not adopted.
The wall cell is at y+ of about 40, because finer wall cells made the trailing-edge cells of this O-grid diverge
(amendment 1).
