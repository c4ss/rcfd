# F-884: the steady iteration at 17 degrees, five candidate causes

Steady Spalart-Allmaras on the mesh M1w-far500 with the set-up of ADR 0760 does not converge at 17 degrees (window
range of cl_eff 2.02 over the last 1000 of 8000 iterations), while ADR 0764's time-accurate run of the same model settles
to an attached flow with cl 1.5516. Each run below is ADR 0760's committed case with one setting added; the
registration, the limits and the results are in
[ADR 0770](../../adr/0770-f884-registration-naca0012-17-degrees-steady-iteration-diagnosis.md).

Steady: ADR 0167's D5 (window range of cl_eff <= 0.005, of cd_eff <= 0.0005). Target: abs(cl - 1.5516) <= 0.018.
Control at 15 degrees: abs(cl - 1.4932034) <= 0.018.

| Run | Setting added | Iterations (exit) | Range cl_eff | cl | Difference | Steady | Target or control |
| --- | --- | --- | --- | --- | --- | --- | --- |
| K0-a17 | none (ADR 0760's run, reproduced bit for bit) | 8000 (5) | 2.020 | 1.0476 (mean) | -0.504 | no | no |
| KU-a17 | `alpha_u = 0.7` | 3517 (0) | 4.3e-4 | 1.5768 | +0.0252 | yes | **no** |
| KS-a17 | `initial.method = "potential"` | 3730 (0) | 0.023 | 1.5093 | -0.0423 | no | no |
| KT-a17 | `turbulence.relax_nut = 1.0` | 8000 (5) | 3.1e-6 | 1.5514 | -0.0002 | yes | **yes** |
| KC-a17 | `non_orthogonal_correctors = 1`, with the SIMPLEC compensation | 8000 (5) | 2.011 | 1.0260 (mean) | -0.526 | no | no |
| KG-a17 | `pseudo_mode = "physical"`, `pseudo_dt = 0.02` | 8000 (5) | 1.8e-4 | 1.5660 | +0.0144 | yes | **yes** |
| KT-a15 | as KT-a17 | 8000 (5) | 4.2e-6 | 1.4932 | -0.0000 | yes | **yes** |
| KG-a15 | as KG-a17 | 8000 (5) | 4.5e-4 | 1.4917 | -0.0015 | yes | **yes** |
| KU-a15 | as KU-a17 (reported, amendment 2) | 8000 (5) | 1e-10 | 1.5014 | +0.0082 | yes | reported |

Files: `results.json` (every run: window statistics, the D5 value and verdicts, cl_eff and cd_eff at every 50th
iteration, the case text) and `cases/` (the ten changed case files; K0 is
`docs/investigations/f868-naca0012-lift-curve/cases/SA-a17.toml`). Binary: a release build of main 21054b68, sha256
`087038c6...5462ec89`; one thread a run, three at a time. Made again by:

    python scripts/w49d_aoa_sweep_fixed.py mesh <rcfd> <work>
    python scripts/f884_naca0012_17_steady.py cases <work>/cases
    python scripts/f884_naca0012_17_steady.py run <rcfd> <work> K0-a17 KU-a17 KS-a17 KT-a17 KC-a17 KG-a17 --parallel 3
    python scripts/f884_naca0012_17_steady.py run <rcfd> <work> KT-a15 KG-a15 KU-a15 --parallel 3
    python scripts/f884_naca0012_17_steady.py collect <work> <this folder> --binary <rcfd> --commit <sha>

The gate test is `crates/rcfd-api/tests/naca0012_17_degrees_steady_f884.rs`.
