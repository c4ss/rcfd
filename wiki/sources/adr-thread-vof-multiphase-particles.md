---
type: source
tags: [adr-thread, multiphase, vof, hysing, dam-break, cavitation, mixture, population-balance, lagrangian, particles]
sources: [doc-reference-data-wave27a-vof, doc-reference-data-wave28a-vof-wiring, doc-particles]
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: volume-of-fluid free surface, the Hysing bubble refinement, multiphase closures and Lagrangian particles (early waves)

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations, diagnoses, one default flip), 15 ADRs, wave 18C to wave 44D.

Two strands. **Particles:** one-way Lagrangian tracking in the new crate `rcfd-particles` (ADR 0028), wired into `rcfd run` (ADR 0037), extended with a two-way exchange library, discrete random walk dispersion and injection options (ADR 0057), and with the two-way coupling wired into the solver loops (ADR 0059). **Multiphase:** mass-transfer and wave closures as libraries before a host existed (ADR 0040), population balance (ADR 0041), the VOF host (ADR 0049), its AMG fix and wiring (ADR 0052), mixture and cavitation (ADR 0056), and then the F-055 refinement chain on the Hysing et al. (2009) rising bubble: the reference read (ADR 0096), the refinement miss (ADR 0100), its diagnosis (ADR 0103), height-function curvature (ADR 0106), all options together (ADR 0108), and the flip of three VOF opt-ins to library defaults with the user's lifting of the bit-identical rule (ADR 0114). See [[vof]], [[lagrangian-particles]].

## Current state

- **VOF defaults since ADR 0114:** "The three VOF opt-ins of ADRs 0103, 0106 and 0108 become the library defaults together: component diagonal, full viscous stress, and height-function curvature with the ADR 0108 CSF fallback built in (F-055). The user lifted the bit-identical-default rule for this one change." New `multiphase` keys give the opt-outs; the final gate gave "2010 passed / 0 failed / 633 ignored with no hash re-recorded".
- **F-055 stays partial** (ADR 0114): "The reworded F-055 bubble clause is a known miss: on 160 x 320, y_c(3) 1.079454 is 0.0004 below [1.0799, 1.0817] and V_c,max 0.241615 is 0.0001 below [0.2417, 0.2421], with phase volume met." The dam-break position clause "still lacks data". Its status moved during the chain: set done by ADR 0096 ("its reference values are now read and the bubble and dam break tests pass"), kept done "as the user decided" in ADR 0100 despite the refinement miss, and "stays partial" from ADR 0103 on.
- **What the refinement chain found:** the default algebraic VOF converged "to a bubble that rises about 10 % too slowly" (ADR 0100); the primary cause is the component-averaged diagonal, which makes a `Symmetry` plane act "mostly like a no-slip wall" (ADR 0103, C7); with a convergent height-function curvature the bubble is "about 3 % too fast on every grid" (ADR 0106); with all three options and exact initial fractions B1 and B2 still miss on y_c(3) (ADR 0108). See [[component-diagonal]].
- **AMG on the variable-density pressure matrix** works since ADR 0052: a coarse row with zero row sum and every coupling below the strength threshold got "a filtered diagonal of EXACTLY 0"; after the fix AMG "is enabled in the multiphase path above 2000 cells". See [[adr-thread-amg-coarse-lu]].
- **Particles:** F-128 "reached its roadmap acceptance on the first-principles jet (J3 residual 3.9e-11 against 2e-2, J2 5.65e-16), which is not a published jet" (ADR 0059); F-056 is partial "whatever the numbers" because the acceptance names settling and cyclone cases (ADR 0028). See [[rcfd-particles]].
- **Library-only closures:** wave generation and absorption (F-227) and mass transfer (F-226) wired into the VOF host by ADR 0052, but "No energy equation exists in the multiphase path, so the mass-transfer temperature is prescribed"; population balance (F-228) with no case-tree wiring (ADR 0041); mixture and Schnerr-Sauer cavitation without full Euler-Euler (F-127, ADR 0056).

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0028](../../docs/adr/0028-wave18c-particles.md) | | registration + results | F-056 | One-way Lagrangian tracking in `rcfd-particles` | four known misses (PT3 order 2.290, PT5, PT8 (a), PT14 hash); F-056 partial | 0037, 0057 |
| [0037](../../docs/adr/0037-wave21c-wiring-fractional.md) | | registration + results | F-054, F-056, F-210 | Particle and chemistry wiring into `rcfd run`; fractional step (on [[adr-thread-early-numerics-transient-accuracy]]) | W and C checks met; C2 printed limit a misquote | |
| [0040](../../docs/adr/0040-wave23b-masstransfer-waves.md) | | registration + results | F-226, F-227 | Mass-transfer and wave closures as a library | M1-M9 met; beach R = 2.6e-4; W7 (d) missed (R = 0.214 against >= 0.5) | 0044, 0052 |
| [0041](../../docs/adr/0041-wave23c-population-balance.md) | | registration + results | F-228 | Population balance (fixed pivot, QMOM) | every limit met, no known miss; no case-tree wiring | |
| [0049](../../docs/adr/0049-wave27a-vof-free-surface.md) | | registration + results | F-055 | VOF free surface with compression and surface tension | every check met except A3 drift (set-up defect, corrected A3b met); dam-break 1.504 inside [1.44, 1.94] | 0052 |
| [0052](../../docs/adr/0052-wave28a-vof-wiring.md) | | diagnosis + wiring | F-055, F-226, F-227, F-854 | AMG breakdown on the density jump; mass transfer and waves in the VOF host | AMG fixed (12.4 against 157.9 iterations per corrector); W2, W4 known misses, corrected tests met | |
| [0056](../../docs/adr/0056-wave29b-mixture-cavitation.md) | | registration + results | F-127 | Mixture (drift flux) and Schnerr-Sauer cavitation on the VOF host | inception met; N3, N4, N4', N5, W2, W3 known misses; venturi corrected to dt 5e-5 | |
| [0057](../../docs/adr/0057-wave29c-lagrangian-extensions.md) | | registration + results | F-056, F-128, F-255, F-323 | Two-way exchange library, random-walk dispersion, injection distributions | all met at first run except L1 (set-up defect); sources do not reach the flow solver | 0059 |
| [0059](../../docs/adr/0059-wave30a-wiring-particles-mixing-plane.md) | | registration + results | F-126, F-128 | Two-way particle coupling in the solver loops (mixing plane on [[adr-thread-fan01-rotating-solve]]) | F-128 acceptance met on a first-principles jet | |
| [0096](../../docs/adr/0096-wave39e-cavity-raster-p1-thin-limit-hysing-check.md) | | partials close-out | F-052, F-055, F-143 | Read the Hysing case-1 reference values | F-055 done; F-052 T1 met; F-143 C2 missed | 0100 |
| [0100](../../docs/adr/0100-wave40d-bubble-refinement-heat-exchanger-nan-check.md) | | registration + results | F-055 | Hysing bubble under refinement; NaN-rejecting stationarity check | B1-B3 missed (errors grow with refinement); B4, H1-H3 met; F-055 stays done by user decision | 0103 |
| [0103](../../docs/adr/0103-wave41c-hysing-bubble-refinement-diagnosis.md) | | diagnosis | F-055 | Causes C1 to C7 of the refinement miss | primary cause C7 (averaged diagonal at `Symmetry`), "contributing" by the letter; CSF smoothing the remaining suspect; F-055 partial | 0106 |
| [0106](../../docs/adr/0106-wave42b-height-function-curvature-hysing-rerun.md) | | registration + results | F-055 | Height-function curvature and the Hysing re-run | estimator converges (H1-H3); H4 misses B1-B3; bubble 3 % too fast | 0108 |
| [0108](../../docs/adr/0108-wave43a-hysing-all-options-exact-fractions-height-function-fallback.md) | | registration + results | F-055 | All three options with exact fractions; CSF fallback of the height function | series X misses B1, B2 on y_c(3) (0.0021, 0.0013, 0.0018); F1-F4 met | 0114 |
| [0114](../../docs/adr/0114-wave44d-vof-default-flip.md) | | default flip | F-055 | The three VOF opt-ins become the defaults together | reproduces wave 43A bit for bit; reworded bubble clause a known miss; F-055 partial | |

## Key numbers and limits

- ADR 0049: AMG-preconditioned CG "broke down on the density jump at step 120 of the dam break, so the multiphase path always uses plain CG" (until ADR 0052); dam-break front `v / sqrt(g H)` 1.504, "11 percent below the reference 1.69".
- ADR 0052 A4 on the 120 x 40 dam break: "a mean of 12.4 pressure iterations per corrector against 157.9 for plain CG and a wall time of 13.0 s against 23.5 s"; W2 control K_r = 0.061 against 0.5, corrected 20-period run K_r = 0.524 ("a narrow pass").
- ADR 0100: y_c(3) errors 0.0531, 0.0635, 0.0689; V_c,max errors 0.0213, 0.0230, 0.0237; "The 40 x 80 pass of ADR 0049 / 0096 is the coarse-grid point of that error, not a grid-converged agreement."
- ADR 0103 C1: "the wall column of an exact uniform flow runs 84 % slow"; the correction "removes about 80 % of the gap on 80 x 160".
- ADR 0106: "y_c(3) moves down by about 0.004 per refinement without a sign of settling"; the height-function lattice check refuses the warped mesh of `cavitating_venturi`.
- ADR 0108: series X `e(160) / e(40) = 0.90`; the dam-break clause "rests on the Martin-Moyce average front velocity (1.69, secondary source)".
- ADR 0056: "The registered dt = 2.5e-4 blows up the venturi at t = 0.082 s"; vapour pressure of water at 20 C (2339 Pa) "unverified, used from memory".
- ADR 0028: PT3 RK2 order "2.290 on its first pair against [1.9, 2.1]"; PT8 (a) "1522 mismatches of 99 510 in-domain points against 0".

## Open questions

- ADR 0108: "Whether this result is acceptable is the user's decision, not taken here."
- ADR 0114: ignored tests that construct `VofSettings::new` without setting all four flags "now run under the new defaults and would not reproduce the numbers their ADRs recorded with the old defaults".
- ADR 0106: the H5 variant (transposed viscous stress too) "had no 160 x 320 run, so it is not a verdict".
- ADR 0056: "a lateral mode at nx >= 4 was NOT root-caused".

## Related

- [[vof]], [[lagrangian-particles]], [[rcfd-particles]], [[component-diagonal]], [[known-miss]], [[doc-particles]]
- [[adr-thread-amg-coarse-lu]], [[adr-thread-early-physics-models]]
- [[roadmap-multiphase-and-particles]]
