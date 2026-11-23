# F-887: the NACA 0012 lift curve with the eddy viscosity not relaxed

ADR 0760's twelve steady Spalart-Allmaras cases (mesh M1w-far500, 0 to 17 degrees) with the one line
`turbulence.relax_nut = 1.0` added, the setting with which ADR 0770 reached the attached state at 17 degrees. No default
is changed. The registration, the limits and the results are in
[ADR 0772](../../adr/0772-f887-registration-naca0012-lift-curve-without-eddy-viscosity-relaxation.md).

Steady: ADR 0167's D5 (window range of cl_eff <= 0.005, of cd_eff <= 0.0005). Row A (the acceptance): steady and
abs(cl - reference) <= 0.018, the reference ADR 0760's steady value (0 to 16 degrees) or ADR 0764's 1.5516 (17). Row V:
ADR 0760's band max(0.05 abs(cl_L), 0.018) about Ladson's 80-grit lift cl_L; row VS: 0.018 alone.

| alpha | iterations (exit) | range cl_eff | cl | A: difference | V: cl - cl_L | VS | default (ADR 0760): iterations (exit), cl |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | 244 (0) | 2.3e-4 | -0.000001 | -0.000000 | +0.0072 | met | 254 (0), -0.000001 |
| 2 | 298 (0) | 7.3e-4 | 0.220164 | -0.000000 | +0.0130 | met | 299 (0), 0.220164 |
| 4 | 331 (0) | 1.0e-3 | 0.438753 | -0.000000 | +0.0116 | met | 332 (0), 0.438753 |
| 6 | 378 (0) | 1.2e-3 | 0.653988 | +0.000000 | +0.0092 | met | 380 (0), 0.653988 |
| 8 | 452 (0) | 1.3e-3 | 0.863827 | -0.000000 | +0.0081 | met | 459 (0), 0.863827 |
| 10 | 536 (0) | 2.5e-3 | 1.065594 | +0.000000 | +0.0070 | met | 533 (0), 1.065594 |
| 12 | 645 (0) | 4.1e-3 | 1.255160 | -0.000000 | +0.0058 | met | 648 (0), 1.255161 |
| 13 | 8000 (5) | 9.1e-7 | 1.342886 | +0.000576 | +0.0045 | met | 777 (0), 1.342311 (window mean, not steady) |
| 14 | 8000 (5) | 3.0e-6 | 1.423469 | -0.000001 | +0.0045 | met | 8000 (5), 1.423470 |
| 15 | 8000 (5) | 4.2e-6 | 1.493201 | -0.000002 | -0.0006 | met | 8000 (5), 1.493203 |
| 16 | 8000 (5) | 4.5e-6 | 1.543568 | +0.001528 | -0.0127 | met | 2008 (0), 1.542041 (window mean, not steady) |
| 17 | 8000 (5) | 3.1e-6 | 1.551398 | -0.000202 | -0.0543 | **miss** (0.018) | 8000 (5), 1.047622 (iteration mean, not steady) |

Every angle is steady by D5 and meets A and V. At 13 and 16 degrees the default met the residual targets and this
setting did not (the residuals stall; the run goes to its cap, steady). RN-a15 and RN-a17 equal ADR 0770's KT-a15 and
KT-a17 bit for bit.

Files: `results.json` (every run: window statistics, the D5 and D7 values, the verdicts of A, V and VS, cl_eff and cd_eff
at every 50th iteration, the case text; the evaluation VM, L2, L4, R1, D15, D17) and `cases/` (the twelve case files).
Binary: a release build of main d6e44b27, sha256 `6be42efd...f20a15bb`; one thread a run, three at a time. Made again by:

    python scripts/w49d_aoa_sweep_fixed.py mesh <rcfd> <work>
    python scripts/f887_naca0012_lift_curve_relax_nut.py cases <work>/cases
    python scripts/f887_naca0012_lift_curve_relax_nut.py run <rcfd> <work> RN-a17 RN-a15 RN-a14 RN-a16 RN-a13 RN-a12 RN-a10 RN-a08 RN-a06 RN-a04 RN-a02 RN-a00 --parallel 3
    python scripts/f887_naca0012_lift_curve_relax_nut.py collect <work> <this folder> --binary <rcfd> --commit <sha>

The gate test is `crates/rcfd-api/tests/naca0012_lift_curve_relax_nut_f887.rs`.
