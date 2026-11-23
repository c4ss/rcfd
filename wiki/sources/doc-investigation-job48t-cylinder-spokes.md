---
type: source
tags: [investigation, cylinder, piso, transient, studio]
sources: []
origin: docs/investigations/job48t-cylinder-spokes/README.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Investigation job 48T: pressure spokes on the refined cylinder O-grid

**Origin:** [docs/investigations/job48t-cylinder-spokes/README.md](../../docs/investigations/job48t-cylinder-spokes/README.md) at origin/main 2e94e5ef (172 lines).
**Kind:** investigation findings (README only). Registration, amendments, limits and full results: [ADR 0154](../../docs/adr/0154-job48t-cylinder-ogrid-pressure-spokes.md).

## Summary

A user's 256 x 256 cylinder run (Re 100) in the Studio showed pressure "spokes" at the wall. Answer **(b)**: "a numerical artefact of the solver at high wall resolution. The default transient algorithm of the case (PISO: one outer iteration, two pressure correctors) loses its pressure-velocity coupling in the thin wall cells of the graded O-grid." Changing one setting gives a clean run to t = 10; `time_advance = "fractional_step"` is the registered recommendation. K1 and K2 are known misses, their tests ignored.

## Key points

- Mechanism: with `grading = 100` and `ny = 256` the first radial cell is 0.00262 D, five times thinner than on the committed 64 x 48 mesh (0.01366 D). The wall diffusion number nu dt / dr1^2 is 3.35 on 64 x 48, 6.5 on 128 x 96, 22.7 on the user's 256 x 256 at dt 0.0156. "Above an onset between 6.5 (clean) and 11.4 (spoky)" the correctors' leftover error is amplified (measured pressure-lag amplification 2.42 per step on the user's set-up, 0.36 on 64 x 48).
- The mode lives in the first 0.2 D at the wall, grid-scale (dominant wavelength 4.5 to 6 cells); it saturates at about 50 free-stream dynamic pressures (p -231 .. 294 against a physical -0.74 .. 0.58) and makes cd jump by 10 to 25 per step.
- Ruled out: the Studio's save of the case (byte-identical runs), the display (every pixel equals the stored cell value), the Courant number, the cell Peclet number, the transient Rhie-Chow term (`ddt_correction = false` still spoky), the convection scheme, non-orthogonal correction (mesh orthogonal to 1e-11), linear-solver tolerances, the wall pressure condition, the start.
- Workarounds (each clean at t = 1, 3, 10; `M_hf` the registered spoke measure, control 0.0057 at t = 3, user's run 52.7): `time_advance = "fractional_step"` (recommended, "the cheapest accepted", 29 s per simulated second); `n_correctors = 3` at dt 1/64 or the user's dt 0.0156 (49 s); `dt = 0.00390625` (106 s, two threads). Also clean: `n_correctors = 4`; `mesh.grading` about 11 at ny = 256. Do not use: PIMPLE `n_outer = 2` or `4` with alpha 0.9 (diverged); `ddt_correction = false`, `convection = "upwind"`, `time_scheme = "bdf2"` (still spoky, M_hf 15).
- With `fractional_step` the Studio's p residual is "1.0 at every step, by design" (pressure increment solved from zero each step).
- "Every workaround was run to t = 10 only"; the case's end time is 80 and shedding starts at about 40 D/U, so "clean to t = 10 does not by itself prove the shedding phase".
- Rule of thumb on this O-grid with the default PISO: "clean up to nu dt / dr1^2 = 6.5, spoky from 11.4"; on the stretched 64 x 256 wall layer even three correctors stay spoky at 22.8.
- Classification rule L3: clean if M_hf <= max(2 x control, 0.01) at every checked time; the README tabulates about 30 arms (R64, R128, R256, RU, B1..B12, W1..W6).
- Mechanism diagnostic DG1 (ADR 0073 pressure-lag estimate): every failing arm has rho >= 1.68, every clean arm rho <= 1.17; the threshold rho = 1 is crossed by one clean arm (G256s, 1.07 to 1.17), "so the registered reading is 'rejected' in its strict form, while rho is a strong separator".
- Tools: `scripts/w48t_run_matrix.py`, `scripts/w48t_spoke_metrics.py`, `crates/rcfd-api/tests/w48t_cylinder_spokes.rs` (E1, DG1, K1, K2, W3, W5; all ignored), and `crates/rcfd-studio/tests/w48t_cylinder_spokes.rs` (display check D1).

## Folder contents

The folder holds only the README. Runs, dumps, metrics and figures are in the work folder `E:\projects\cfd\w48t-work` outside the repository (figures under `report\figures`, Studio-path renders under `report\studio-renders`).

## Contradictions with the wiki

> [!warning] Contradiction
> The README's Tools list names `crates/rcfd-studio/tests/w48t_cylinder_spokes.rs` for the display check D1. [[doc-architecture]] records `rcfd-studio` as removed (F-449), and the crate is absent from the 2e94e5ef snapshot. The README predates the removal; within the repo docs.

## Pages this source informs

- [[cylinder-re100]], [[piso]], [[divergence-guard]], [[known-miss]], [[studio]]
- [[adr-thread-piso-stability-consistent-correctors]] (ADR 0223's DC6 reuses ADR 0154's K1, K2)
- [[adr-thread-naca0012-sa-drag]] (the ADR thread of the NACA 0012 SA chain)
