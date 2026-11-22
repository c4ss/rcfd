# F-868: the NACA 0012 at 17 degrees by an unsteady arm (ADR 0764)

The record of [ADR 0764](../../adr/0764-f868-registration-naca0012-17-degrees-by-an-unsteady-arm.md): two time-accurate
runs of Spalart-Allmaras (without DES) on the mesh M1w-far500 of
[ADR 0760](../../adr/0760-f868-registration-naca0012-lift-curve-against-ladson.md), at 17 degrees and, as the control, at
15 degrees. `pimple`, `bdf2`, three outer iterations, adaptive correctors, dt 0.02 c/U from the free stream to t = 200
(10 000 steps); the time mean of `cl_eff` over [100, 200] from the samples at every fifth step. Made on 2026-11-22 on
Windows with a release build of main 2bab1863 (binary sha256
`eeb751d35817a5a92af79012ac45164b5751a27a066fe37b38b84f07a18565ce`), both runs side by side, one thread each, 11 064 s
and 11 139 s.

| | 15 degrees (control) | 17 degrees |
| --- | --- | --- |
| Time mean of cl over [100, 200] | 1.49167 | 1.55158 |
| Compared with | 1.49320, the steady value of ADR 0760 | 1.6057, Ladson's 80-grit lift |
| Difference (limit) | -0.0015 (0.018) | -0.0541 (0.0803) |
| Half means by the registered rule, and their difference (limit 0.018) | 1.49167, 1.49217; +0.0005 | 1.55158, 1.55130; -0.0003 |
| Plain half means over [100, 150] and [150, 200] | 1.49117, 1.49217; +0.0010 | 1.55187, 1.55130; -0.0006 |
| Range of cl over the window | 0.0021 (1.49035 to 1.49250, rising) | 0.0012 (1.55234 to 1.55110, falling) |
| Oscillation | none: steady in time | none: steady in time |
| Time mean of cd (no claim) | 0.02727 (0.02699 to 0.02773) | 0.04369 (0.04353 to 0.04395) |
| Correctors per outer iteration | 2 at every step | 2 at every step |
| End | t = 200, exit code 0 | t = 200, exit code 0 |

At 17 degrees the time-accurate run does not shed: after the start the lift passes 1.571 at t = 11 and then falls slowly
towards 1.551, still by 6e-6 per time unit at the end. The value is within the band of ADR 0760. The steady SIMPLEC
iteration of ADR 0760 did not reach this state (it swung between -0.07 and 1.95); why was not examined.

Files: `results.json` (per run the samples of the window in full precision, the samples at every whole time unit before
it, the judgement and the case text) and `cases/U-a15.toml`, `cases/U-a17.toml`. The gate holds the judgement in
`crates/rcfd-api/tests/naca0012_17_degrees_unsteady_f868.rs`, which computes it again from the samples.

To make the record again (a work folder outside the repository; about three hours):

    python scripts/w49d_aoa_sweep_fixed.py mesh <rcfd> <work>
    python scripts/f868_naca0012_lift_curve.py urun <rcfd> <work>
    python scripts/f868_naca0012_lift_curve.py ucollect <work> docs/investigations/f868-naca0012-17-degrees-unsteady --binary <rcfd> --commit <sha>
