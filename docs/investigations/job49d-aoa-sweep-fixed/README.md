# Job 49D: NACA 0012 angle-of-attack sweep with the SA drag fixes, 500-chord far field

The set-up, limits and reading rules are in [ADR 0167](../../adr/0167-job49d-naca0012-aoa-sweep-with-the-sa-drag-fixes-at-500-chords.md).

This job repeats the sweep of [ADR 0159](../../adr/0159-job48y-naca0012-angle-of-attack-sweep-through-stall.md) with
the two opt-in fixes of [ADR 0164](../../adr/0164-job49a-why-the-sa-drag-of-the-naca0012-is-wrong.md):
- the effective force, `reports.<n>.component = "total_effective"`;
- the far-field turbulence role, `boundaries.farfield.turbulence = "freestream"`.

The far field is at 500 chords (mesh M1w-far500, which is ADR 0164's F500w). The case files spell out every key, so
they mean the same thing if the defaults change later.

Runs are on demand. They are not stored in git.

```
python scripts/w48y_aoa_sweep.py mesh <rcfd> <work> M1w
python scripts/w48y_aoa_sweep.py mesh <rcfd> <work> M1w --far 500
python scripts/w49d_aoa_sweep_fixed.py cases <work>
python scripts/w49d_aoa_sweep_fixed.py run <rcfd> <work> SA-a00 ... --parallel 2
python scripts/w49d_aoa_sweep_fixed.py collect <work> <outdir> [--c10 <w49a-work>] --plots
```

Files in this folder:
- `results.json`: every run and the evaluation;
- `results.csv`: one row per run;
- `histories.csv`: cl_eff and cd_eff, at most 61 samples per run;
- `cl_alpha.png`, `polar.png`, `histories.png`.

Binary: a release build of f9a520a, sha256 `056669b7...c74d869`. There were 28 runs, one thread each, at most two at
a time.

## Lift curve and drag (SA, both fixes)

The curve C uses `cl_eff` and `cd_eff`. "Steady" is D5's window rule. A run that stopped at the residual targets
(exit 0) takes its final iterate. A run at the 8000-iteration cap (exit 5) takes its window mean. An angle that is not
steady takes its window mean, flagged.

| alpha | cl | cd | Ladson cl | Ladson cd | ADR 0159 cl | ADR 0159 cd | steady | cl range in window | iterations (exit) | D [%] |
|---|---|---|---|---|---|---|---|---|---|---|
| 0 | 0.0000 | 0.00840 | -0.007 | 0.0081 | 0.0000 | 0.0037 | yes | | 254 (0) | +0.00 |
| 2 | 0.2202 | 0.00854 | 0.207 | 0.0082 | 0.2138 | 0.0076 | yes | | 299 (0) | -0.06 |
| 4 | 0.4388 | 0.00901 | 0.427 | 0.0082 | 0.4203 | 0.0118 | yes | | 332 (0) | -0.14 |
| 6 | 0.6540 | 0.00988 | 0.645 | 0.0088 | 0.6185 | 0.0182 | yes | | 380 (0) | -0.21 |
| 8 | 0.8638 | 0.01130 | 0.856 | 0.0103 | 0.8062 | 0.0274 | yes | | 459 (0) | -0.24 |
| 10 | 1.0656 | 0.01352 | 1.059 | 0.0119 | 0.9817 | 0.0396 | yes | | 533 (0) | -0.45 |
| 12 | 1.2552 | 0.01695 | 1.249 | 0.0132 | 1.1431 | 0.0549 | yes | | 648 (0) | -0.58 |
| 13 | 1.3423 | 0.01943 | 1.338 | 0.0149 | 1.2175 | 0.0637 | no (window rule) | 1.338-1.343 | 777 (0) | -0.52 |
| 14 | 1.4235 | 0.02253 | 1.419 | 0.0160 | 1.2872 | 0.0734 | yes | | 8000 (5) | -0.00 |
| 15 | 1.4932 | 0.02676 | 1.494 | 0.0183 | 1.3516 | 0.0839 | yes | | 8000 (5) | -0.00 |
| 16 | **1.5420** | 0.03302 | 1.556 | 0.0213 | 1.4095 | 0.0953 | no (window rule) | 1.524-1.544 | 2008 (0) | -0.70 |
| 17 | 1.0476 | 0.2997 | 1.606 | 0.0251 | 1.4594 | 0.1078 | **no** | -0.07 to 1.95 | 8000 (5) | +0.78 |
| 18 | 0.9981 | 0.3208 | 1.011 | 0.1842 | 1.4988 | 0.1214 | **no** | -0.20 to 1.77 | 8000 (5) | -0.10 |
| 20 | 1.0846 | 0.3869 | | | 1.5278 | 0.1560 | **no** | -0.10 to 1.76 | 8000 (5) | +0.04 |
| 22 | 1.2343 | 0.4748 | | | 1.3951 | 0.2175 | **no** | 0.10 to 2.00 | 8000 (5) | -0.08 |
| 24 | 1.2748 | 0.5451 | | | 1.2039 | 0.3068 | **no** | 0.18 to 2.15 | 8000 (5) | +1.40 |

Notes on the table:
- **Ladson** is the 80-grit data, interpolated to each angle.
- **ADR 0159** is the 50-chord sweep without either fix.
- **D** is D6's drift metric on cd_eff.
- **"no (window rule)"** at 13 and 16 degrees: both runs met the residual targets (1e-6), at iterations 777 and 2008.
  The rule's window is the last half of the run, and it still holds the start-up tail. Their final iterates are
  1.3429 / 0.01938 and 1.5436 / 0.03291.
- **"no"** from 17 degrees up: the steady iteration swings through a cycle of cl from about 0 to 2 and never settles.
  The value shown is an iteration mean, not a flow value. No DDES was run in this job.

### Summary against Ladson

- **Lift slope** over 0-8 degrees: 0.1081 per degree, which is **0.986 x 2 pi**. Ladson's is 0.985. ADR 0159 gave 0.920.
- **Stall.**
  - SA: cl_max **1.542 at 16 degrees**. At 17 degrees and above, steady SA no longer converges; its iteration mean
    is 1.05, a 35 % drop, so the first angle below 0.9 cl_max is 17.
  - Ladson: cl_max 1.612 at 17.13 degrees, then 1.00 at 18.02.
  - ADR 0159: the peak was at 20 degrees (1.528), and the first angle below 0.9 cl_max was 24.
- **Lift in the attached range:** within 0.013 of Ladson's from 2 to 15 degrees, and within 0.006 from 12 to 15 (at
  15: 1.4932 against 1.4938).
- **Drag:** 0.0084 at 0 degrees, against 0.0081 measured. From 6 degrees up it is above the measurement, increasingly
  so (+14 % at 10, +46 % at 15, +55 % at 16). Ladson's tripped tunnel boundary layer is not the TMR fully-turbulent
  case, so TMR (below) is the cleaner reference for drag.

### Summary against the TMR SA values (CFL3D)

| alpha | cl | TMR cl | | cd | TMR cd | | limit |
|---|---|---|---|---|---|---|---|
| 0 | 0.0000 | 0.0000 | | 0.00840 | 0.00819 | +2.5 % | LT0 (12 %) met |
| 10 | 1.0656 | 1.0909 | -2.3 % | 0.01352 | 0.01231 | +9.8 % | LT10 (15 %, 5 %) met |
| 15 | 1.4932 | 1.5461 | -3.4 % | 0.02676 | 0.02124 | **+26.0 %** | LT15 (25 %, 5 %) **missed** |

SA-a10 is ADR 0164's C10 re-run on today's main. All 1599 values of cl, cd and cd_eff over its 533 iterations are
equal to C10's, bit for bit.

## The effect of each fix (four-way comparison, 500 chords)

There are two runs per angle:
- **N** keeps ADR 0159's far field, written out as `turbulence = "zero_gradient"`;
- **SA** uses the `freestream` role.

Each run reports both the molecular force and the effective force. The table gives cl / cd.

| alpha | old set-up | effective force only | freestream role only | both fixes | ADR 0159 (50 c, old) | TMR |
|---|---|---|---|---|---|---|
| 0 | 0.0000 / 0.00365 | 0.0000 / 0.00842 | 0.0000 / 0.00364 | 0.0000 / 0.00840 | 0.0000 / 0.00368 | 0.0000 / 0.00819 |
| 10 | 0.9878 / 0.03849 | 0.9879 / 0.04344 | 1.0654 / 0.00949 | 1.0656 / 0.01352 | 0.9817 / 0.03956 | 1.0909 / 0.01231 |
| 15 | 1.3563 / 0.08234 | 1.3567 / 0.08640 | 1.4926 / 0.02381 | 1.4932 / 0.02676 | 1.3516 / 0.08392 | 1.5461 / 0.02124 |
| 18 | 1.5016 / 0.11982 | 1.5022 / 0.12334 | 0.9982 / 0.31946 | 0.9981 / 0.32078 | 1.4988 / 0.12136 | |

Outer nuTilda/nu (the median over cells 5 to 40 chords upstream) and the drift D of the N runs:

| | N runs | SA runs |
|---|---|---|
| 0 degrees | 7.3 | 3.0 |
| 10 degrees | 5.9e4 | 3.0 |
| 15 degrees | 8.1e4 | 3.0 |
| 18 degrees | 8.9e4 | 3.0 |
| D | +1.33 %, +1.36 % and +1.16 % at 10, 15 and 18 degrees (over LD's 0.5 %) | |

What each fix does:
- **The effective force** adds the friction that the molecular report misses: +0.003 to +0.005 in cd where the flow is
  attached, +0.001 in SA's separated flow at 18 degrees. It does not change cl or the solution.
- **The freestream role** changes the solution. It removes the outer eddy viscosity of 6e4 to 9e4 nu. Then:
  - the pressure drag falls, from 0.0363 to 0.0075 at 10 degrees;
  - cl rises by 0.08 at 10 degrees and by 0.14 at 15 degrees;
  - the flow separates from 17 degrees up, where the old set-up stayed attached to beyond 20 degrees.
- **Either fix alone** is wrong at lift. The effective force alone makes cd worse at 10 and 15 degrees, because it adds
  friction on top of the far-field pressure drag. The freestream role alone gives cd 23 % below TMR at 10
  degrees. **Both together** give +9.8 % at 10 degrees and +26 % at 15.
- **50 against 500 chords:** with the old set-up, cl changes by up to 0.006 and cd by 1-3 %. With both fixes, the
  50-chord F10 of ADR 0164 gave cd +26 %; 500 chords gives +9.8 %.

## SST (report only, both fixes)

The far field holds the initial turbulence: intensity 0.05 and viscosity ratio 10.

| alpha | 0 | 4 | 8 | 12 | 14 | 16 | 18 | 20 |
|---|---|---|---|---|---|---|---|---|
| cl | 0.0000 | 0.4368 | 0.8596 | 1.2457 | 1.4063 | 1.5089 | 1.0385 | 1.0597 |
| cd | 0.00832 | 0.00893 | 0.01125 | 0.01723 | 0.02342 | 0.03553 | 0.3174 | 0.3666 |
| steady | yes | yes | yes | yes | yes | no (window rule; exit 0 at 1826) | no | no |

- Slope 0.980 x 2 pi, over 0, 4 and 8 degrees.
- Peak 1.509 at 16 degrees; first angle below 0.9 cl_max at 18.
- In ADR 0159 (no fixes) SST peaked at 14 degrees with 1.397, and its drag was half the measured value.

## Limits (ADR 0167)

| Id | Outcome |
|---|---|
| L1 | met: 28 runs, every one exit 0 or 5. Two N runs were cut by a PC restart and run again from scratch (ADR 0167 deviations) |
| L2 | met: 0.986 |
| L3 | met: peak 16 degrees, first below 0.9 cl_max at 17 |
| L4 | met: cl(0) = -1.4e-6 |
| LT0 | met: +2.5 % |
| LT10 | met: cd +9.8 %, cl -2.3 % |
| LT15 | **known miss**: cd +26.0 % (band 25 %); cl -3.4 % |
| LD | **known miss**: over 0.5 % at 12, 13, 16 (start-up tail of runs that met the targets early) and 17, 24 (not steady) |
| P2 | met: both mesh hashes |
| B1 | met (test `w49d_b1_storage_budget`) |

The limit tests are in `crates/rcfd-api/tests/aoa_sweep_w49d.rs`.
