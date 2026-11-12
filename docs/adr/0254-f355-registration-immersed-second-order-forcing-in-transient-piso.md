# ADR 0254: F-355 registration: second-order immersed forcing, the stop in transient PISO at Re 100

Status: **REGISTERED 2026-11-12**, written before any code of F-355 and before any run of its checks. Not yet accepted by the
maintainer. The measured cause and the corrective design (or the refusal) go in an amendment written and committed before the
corrected code and its runs; results go in an addendum; nothing above an amendment or the addendum is changed after a run.

Parents: [ADR 0234](0234-f309-registration-immersed-face-flux-at-the-surface-crossing.md) (F-309: `build_second_order`,
`reconstruct_face_flux`, its D1 to D3, its check V3 and questions Q3, Q4, Q6),
[ADR 0249](0249-f346-registration-immersed-force-in-unsteady-flow.md) (F-346: the printed 2D-2 run that stopped; its Q6, from
which F-355 was made), [ADR 0218](0218-f219-registration-immersed-solid-with-modified-forcing.md) (F-219: the forcing, the DFG
and CC set-ups, the mesh family M1 to M3). Roadmap item F-355 (depends on F-309).

Number: the lead gave 0254. Branch `f355-immersed-piso-stop` from main 9387a743, worktree `E:/projects/cfd/w49i-work/f355`.

House rules (those of ADR 0218, 0234, 0239, 0249): limits are fixed here and not changed after a run; a missed limit becomes an
ignored `*_known_miss` test with the limit intact and the number recorded, and is diagnosed, not tuned away; a defective set-up
is corrected by an amendment before the corrected run, and the old test stays as an ignored record; each registered run is made
once; without an immersed solid and with the forcing of `build` every result is bit-identical; no case-schema key, hash table,
golden or existing test file is edited; no new third-party crate; no unsafe code; no commit trailers.

Clean-room: the forcing is that of ADR 0234 (the ghost-cell view of a Dirichlet boundary on a Cartesian mesh: Fedkiw, Aslam,
Merriman and Osher, J. Comput. Phys. 152 (1999) 457-492; Gibou, Fedkiw, Cheng and Kang, J. Comput. Phys. 176 (2002) 205-227;
Tseng and Ferziger, J. Comput. Phys. 192 (2003) 593-623; Mittal et al., J. Comput. Phys. 227 (2008) 4825-4852; the mass source
of Kim, Kim and Choi, J. Comput. Phys. 171 (2001) 132-150). That a linear extrapolation through a boundary point that lies
close to a grid node has large weights of alternating sign and can make an explicit or segregated update unstable, and that the
remedy is a stencil with bounded weights (an image point in the fluid), is the remark of Tseng and Ferziger (their section on
the ghost-cell reconstruction) and the construction of Mittal et al. These are cited from memory of the published papers; no
text of them was read for this work and no formula is copied: every formula used is derived here on the discrete equations of
`crates/rcfd-fv/src/pimple.rs` and `immersed.rs`. PISO is that of Issa, J. Comput. Phys. 62 (1986) 40-65 (from memory). No
vendor code, text, manual or binary was used.

## What F-355 asks

The roadmap entry: "Second-order immersed forcing: transient PISO at Re 100"; acceptance "DFG 2D-2 on M2 with
`build_second_order` runs to t = 8 in PISO with BDF2 and gives a Strouhal number within 1 percent of the first-order forcing on
the same mesh, or the cause of the stop at step 6 is diagnosed by a registered measurement and the configuration is refused with
a located message." The acceptance is taken as binding. It has two branches; which one is reached is said in the addendum.

## What was read and run before this text (disclosed)

Read: ADR 0234 in full; of ADR 0249 the head, the set-up of the 2D-2 runs, the results table, "the printed run that stopped"
and Q6; of ADR 0218 the mesh family and the 2D-2 set-up; `crates/rcfd-fv/src/immersed.rs` in full; of
`crates/rcfd-fv/src/pimple.rs` the `ddt` flux defect (lines 1019 to 1042), the momentum assembly with the forcing (1230 to
1237), the correctors (1316 to 1508, with the call of `reconstruct_face_flux` at 1346 to 1350 and the pressure solve with the
source at 1417 to 1423) and the end of the step (1552 to 1575); `immersed_second_order_f309.rs` (the CC helpers, A1, V3, U1 to
U3); of `immersed_unsteady_force_f346.rs` the DFG helpers and `run_2d2`; the roadmap entries F-355, F-309, F-354.

Run: **the stop was reproduced before this registration**, with the existing test, on the unchanged tree 9387a743, release:

    cargo test -p rcfd-fv --release --test immersed_unsteady_force_f346 printed_two_d_two_on_m2_second_order -- --ignored --nocapture

Output: "2D-2 M2 Second: 18304 cells, 1264 solid cells, 116 first fluid cells"; "stopped at step 6: Divergence { step: 6,
stage: "end of step", quantity: "courant", value: 11094166.6, limit: 1000000, ... continuity: NaN, res_u: 0.799, res_p: 0.110,
p_iters: 18 }". The number of ADR 0249 (Courant 1.1e7 at step 6). No other run was made and no code of F-355 exists.

Facts read from the code (no run):

- `reconstruct_face_flux` is called in every corrector of every outer iteration, after the `ddt` flux correction, and
  overwrites the flux of the flux faces: the correction is not present on those faces in any corrector. The run has
  `non_orth_correctors = 0`.
- The old and old-old fluxes enter only through the `ddt` flux defect, which is overwritten at the flux faces. They also are
  the convecting flux of the predictor (`phi` of the previous step).
- `corrector_control` is `None` in the run (`PimpleSettings::piso`), so `pick_correctors` is not called; the continuity number
  of `StepInfo` is a report (line 1562) and feeds nothing. The stop is the Courant guard at the end of the step: the flux
  itself is of the order of 1e7 cell volumes per step.

Arithmetic done beforehand (by hand), disclosed because it adds a hypothesis (H8) and the numbers of its prediction. Let `c`
be a first fluid cell with one solid neighbour across the face `f`, on a uniform mesh of spacing `h` in two dimensions,
`theta_e` the bounded crossing fraction, `w = t_f/theta_e = 1/(2 theta_e)` the weight of ADR 0234 D1, `u_G = 0`. The flux
velocity of `f` is `(1 - w) HbyA_c`. The diagonal of the momentum equation of `c` (per unit volume, convection left out) is
`c0/dt + (nu/h^2)(3 + 1/theta_e)` (`c0 = 1` for implicit Euler and for the first step of BDF2, `3/2` for BDF2 with a constant
step; three regular faces and the modified face of ADR 0218 D2). A perturbation `delta` of the old wall-normal velocity of `c`
changes `HbyA_c` by `beta delta` times a factor of order one (`c1 = 2`, `c2 = 1/2` over `c0 = 3/2` for BDF2; 1 for Euler), with

    beta = (c0/dt) / (c0/dt + (nu/h^2)(3 + 1/theta_e)),

the share of the time term in the diagonal, and the flux velocity of `f` by `(1 - w) beta delta`. The pressure equation makes
the fluxes of `c` sum to zero, and the corrected cell velocity is close to the mean of the fluxes of opposite faces, so the new
wall-normal velocity of `c` is between `1/2` and `1` times the flux velocity of `f` (the flux leaves through the opposite face,
factor 1, or sideways, factor down to 1/2). Per time step the perturbation is multiplied by about

    g = kappa G,   G = abs(w - 1) beta,   kappa between 1/2 and 1.

With `r = c0 h^2/(nu dt)`: `G(theta_e) = abs(1/(2 theta_e) - 1) r/(r + 3 + 1/theta_e)`, which tends to `r/2` for `theta_e -> 0`.
Numbers at `theta_e = theta_min = 0.01` (`w = 50`):

| Configuration | `r` | `G` |
| --- | --- | --- |
| 2D-2 on M2 (`h = 0.0025`, `nu = 1e-3`, `dt = 1e-3`), BDF2 | 9.375 | 4.09 |
| the same, implicit Euler | 6.25 | 2.80 |
| the same with `theta_min` 0.1, 0.25, 0.5 (BDF2) | 9.375 | 1.68, 0.57, at most 0.35 |
| the same with `dt` 5e-4, 2e-3, 4e-3, 8e-3 | 18.75, 4.69, 2.34, 1.17 | 7.55, 2.13, 1.09, 0.55 |
| the same with `nu` 3e-3, 1e-2 | 3.125, 0.9375 | 1.44, 0.44 |
| 2D-2 on M1 (`h = 0.005`) | 37.5 | 13.1 |
| CC at N 48, the PISO run V3 of ADR 0234 (`h = 0.05`, `nu = 1`, `dt = 2.5e-3`, Euler) | 1 | 0.47 |
| CC at N 48 with `dt = 2.5e-5` | 100 | 24.1 |

So the estimate says the check V3 of ADR 0234 was in the stable range (the viscous term dominates the diagonal, `nu dt/h^2 = 1`)
and 2D-2 is not (`nu dt/h^2 = 0.16`), and that a **smaller** time step makes it worse. The numbers of a run use the actual
faces of its mesh (the smallest `theta` present need not reach `theta_min`); the test computes `G` from them (below).

## Set-ups

**R (the reproduction).** The 2D-2 set-up of ADR 0249's `run_2d2` on M2 (`dfg_family(2)`, 176 x 104 cells): `U_m = 1.5`,
`nu = 1e-3`, `K = 1e7`, `theta_min = 1e-2`, the start field (the inflow profile everywhere plus the transverse blob),
`PimpleSettings::piso` with BDF2, `LinearUpwind`, `amg_pressure`, two correctors, `tol_rel` 1e-6 for momentum and pressure,
`dt = 1e-3`; `build_second_order`. The helpers are copied into the new test file (the old files are not edited).

**A variant** is R with one named change, run for at most 200 steps. After each step the test records `m_n`, the largest
`abs(u)` over the first fluid cells (the fluid cells of the flux faces), and the largest `abs(u)` over all cells. Verdicts:

- **stops**: `diverged` is set within the 200 steps (the step is recorded);
- **runs**: no stop and the largest `abs(u)` over all cells at step 200 is `<= 10` (the inflow maximum is 1.5);
- **grows**: no stop but that number is above 10 (counted with "stops").

The measured growth factor `g` of a variant that stops is the geometric mean of `m_n/m_(n-1)` over the last three completed
steps before the stop (printed; with fewer than four completed steps, over those available).

**The control of a variant** is the same run with `build` (the first-order forcing of ADR 0218). It must "run"; a variant whose
control does not is void (the change itself breaks the solver) and is reported as such, not as evidence.

**The estimate `G` of a run** (computed by the test from the forcing, before the run, and printed): over the flux faces `f`
with fluid cell `c`, `G = max_f abs(weight_f - 1) beta_c`, `beta_c = (c0 V_c/dt)/(c0 V_c/dt + sum over the internal non-empty
faces of c of k_face m_face)`, `k_face = nu lap_coeff`, `m_face = 1/theta_e` for a fluid-solid face of `c` (from the
modification term) and 1 otherwise; `c0 = 3/2` for BDF2, 1 for Euler. Convection is left out of the diagonal.

**CC** (ADR 0218 and 0234, unchanged): for H8 the PISO run of V3 (N 48, Euler, central, from rest) with `dt = 2.5e-3` (400
steps, the run of V3) and with `dt = 2.5e-5` (at most 400 steps), `build_second_order`; controls with `build`.

## Hypotheses, measurements and the numbers that decide

Each is tested on R by the variant named; "necessary" means the stop needs it, "not the cause" that the stop does not.

| Id | Hypothesis | Measurement | Decides |
| --- | --- | --- | --- |
| H1 (a) | the `ddt` flux correction on top of the reconstructed flux | code (above: the reconstruction overwrites it in every corrector) and the variant `ddt_correction = false` | the variant stops: not the cause. It runs: necessary |
| H2 (b) | the reconstruction is missing in later correctors | code (above) and the variants `n_correctors` 1, 3, 4 | every one stops: the corrector count is not the cause. One runs: that count matters (reported with its `g`) |
| H3 (c) | the BDF2 old and old-old fluxes at the flux faces | the variant implicit Euler | it stops: BDF2 is not necessary. It runs: necessary |
| H4 (d) | the solid-cell source in the continuity error or in the corrector control | code (above: no corrector control, the continuity number is a report) and, on R, the largest `abs(sum of phi)` over the **fluid** cells divided by the largest `abs(phi)` at the last completed step before the stop | `<= 1e-4`: the pressure equation closes the fluid cells and the large flux is a real one, not a continuity error: not the cause. Above: the pressure solve with the source does not close continuity (cause to follow up) |
| H5 (e) | a large extrapolation weight (crossing close to the fluid cell centre) | the largest weight of the forcing and the weights of the faces of the cell that holds the largest `abs(u)` at the stop of R; the variants `theta_min` 0.1, 0.25, 0.5 (largest weights 5, 2, 1) | `theta_min` 0.5 and 0.25 run and the worst cell of R has a face with weight `>= 5`: confirmed (the weight is necessary). `theta_min = 0.5` stops: not the cause |
| H6 (f) | linear upwind reading solid-cell values | the variants `Upwind` and `Central` | both stop: not the cause. `Upwind` runs: necessary |
| H7 (g) | the time step and the impulsive start | the variants `dt` 5e-4, 2e-3, 4e-3, 8e-3; the variant "late": 300 steps with `build` from the start field, then the forcing replaced by `build_second_order` (the solver keeps its fields and history) and at most 200 more steps | "late" stops: the start is not the cause. `dt = 5e-4` runs and a larger step stops: a Courant-type limit. The reverse: see H8 |
| H8 (added here) | the product of the extrapolation weight and the share of the time term in the diagonal of the first fluid cell: the wall-normal velocity of that cell is multiplied by about `kappa G` per step, `kappa` in `[1/2, 1]` (the arithmetic above) | `G` of every run of this table against its verdict; in addition the variants `nu` 3e-3 and 1e-2, R on M1, and the two CC runs | **confirmed** if every run with `G <= 1` runs, every run with `G >= 2` stops (or grows), the CC run with `dt = 2.5e-5` stops and the CC run with `dt = 2.5e-3` runs. **Refuted** by one run with `G <= 1` that stops or one with `G >= 2` that runs (its control running). Runs with `1 < G < 2` are printed and decide nothing |

The measured `g` against `G` is printed for every stopping run (expected between `G/2` and `G`; no limit: the estimate leaves
out convection, the tangential component and the second corrector).

If H8 is confirmed, H5 and H7 are parts of it (the weight and the time step enter as the product `G`), and H1, H3, H6 are
expected "not the cause". If H8 is refuted, the verdicts of H1 to H7 stand on their own numbers and the diagnosis continues by
an amendment (new measurements registered before they are run).

## Registered checks

All in the new file `crates/rcfd-fv/tests/immersed_piso_stop_f355.rs`, release (the heavy ones carry
`#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]`).

| Id | Check | Limit |
| --- | --- | --- |
| D0 | R stops | at step 6, by the Courant guard, Courant between 1.0e7 and 1.2e7 (the reproduction; a different number is a defective copy of the set-up) |
| D1 to D8 | the measurements of H1 to H8 | the deciding numbers of the table; the test asserts only that every control runs and that the verdict of H8 is one of "confirmed" or "refuted" as defined; the verdicts are printed and reported |
| A (acceptance, first branch; only if a corrective is registered by amendment) | 2D-2 on M2 with the corrected second-order forcing, the settings of R, 8000 steps to `t = 8`; the same run with `build` in the same test. Set-up S2 for both (the window of ADR 0218: from `t = 5.5`, at least 6 upward crossings of `C_L` about its mean, every period within 1 percent of their mean, the maximum of `C_L` of every period within 2 percent of their mean, `max C_L - min C_L >= 1.0`; `C_L` from `force_moment`) | no stop; `abs(St_second/St_first - 1) <= 1e-2` |
| CC-A1 (only with a corrective) | the check A1 of ADR 0234 with the corrected forcing: CC SIMPLEC at N 24, 48, 96, S1 (`max_du <= 1e-10` within 20 000 iterations) | `e(96) <= 1e-2`; `log2(e(48)/e(96)) >= 1.5` |
| CC-V3 (only with a corrective) | the check V3 of ADR 0234 with the corrected forcing (N 48, PISO, Euler, central, `dt = 2.5e-3`, 400 steps; set-up: last step change `<= 1e-9`) | `e <= 1.2 e_SIMPLEC`; `eps <= 1.5 eps_SIMPLEC` (SIMPLEC with the same corrected forcing) |
| RF (second branch, and with a corrective for whatever stays unsupported) | the refused configuration: R, one step | the run is stopped at step 1 with a message that names ADR 0254, the quantity that decided and a cell; the CC run V3 and a SIMPLE iteration of CC are not refused |
| N1 (defaults) | `immersed_solid_f219.rs`, `immersed_second_order_f309.rs`, `immersed_force_order_f337.rs`, `immersed_unsteady_force_f346.rs`, unedited, the tests that are not ignored, once on the final tree in release; `cargo test -p rcfd-api --release --test baseline_hashes` if `simple.rs` or `pimple.rs` is changed | every test passes (they hold the state hashes of `build`, the numbers of ADR 0234 and the digits of ADR 0239's 2D-1 run with `build_second_order`) |

The further checks of a corrective (unit checks of the new formula, the re-measurement of the numbers of ADR 0234 and 0239 if
`build_second_order` itself changes, its negative controls) are registered in the amendment that registers the corrective, with
their limits, before its code.

**Negative controls.** The control run of every variant (`build`); C-R: R with `build` runs its 200 steps (the stop belongs to
the second-order forcing); the CC run at `dt = 2.5e-3` (H8 must not predict a stop where ADR 0234 V3 ran).

### Mutations (scratch, not committed)

Each applied alone to the final tree by an exact text replacement (checked to occur once), the named tests run once in release,
the file restored and compared byte for byte with a kept copy. Planned now, for whatever library code F-355 adds; the list is
completed in the amendment:

| Id | Mutation | Must fail |
| --- | --- | --- |
| M1 | the refusal never fires (its comparison made false) | RF |
| M2 | the refusal always fires with flux faces | RF (the CC runs refused), CC-V3 |
| M3 (with a corrective) | the corrective switched off (the formula of ADR 0234 D1 on every face) | A or the unit check of the corrective |
| M4 (with a corrective) | one deliberate error in the corrected weight (named in the amendment) | CC-A1 or the unit check |

## Decision rule

- **done**: D0 held; one hypothesis is confirmed by its registered number (for H8: "confirmed" as defined) and
  either (first branch) a corrective registered by amendment meets A with S2 held, CC-A1 and CC-V3, with N1 met and RF met for
  whatever configuration stays unsupported, or (second branch) the configuration is refused and RF and N1 are met; every
  mutation failed a test it must fail.
- **partial**: the cause is confirmed but neither branch is complete (a corrective that misses A, CC-A1 or CC-V3, kept as an
  ignored known miss with the configuration refused; a mutation not caught; RF missed); or the stop is narrowed (some
  hypotheses ruled out by their numbers) without a confirmed cause, with the configuration refused by a located message.
- **todo** (unchanged): D0 does not hold and is not repaired, or N1 is missed; the library part is then not proposed.

A set-up check or control that does not hold (D0, a control run, S2, S1, the set-up of CC-V3) is a defective set-up: an
amendment before any corrected run, never counted as a pass.

## Limits (not covered)

One flow (2D-2) on one mesh family and CC; two-dimensional, orthogonal meshes; stationary bodies. The estimate `G` is a local
one-dimensional argument, not a stability proof. PIMPLE with outer iterations, the consistent correctors, the adaptive corrector
count (`pick_correctors`, ADR 0234 Q6), the fractional step and moving meshes are not run. No case-schema key and no Studio
control. The force extrema of 2D-2 with the second-order forcing are printed only (F-354 is the item for them). M3 and T are
not run.

## Open questions

- **Q1. Where a corrective goes.** `build_second_order` is an opt-in whose 2D-1 numbers are asserted to nine digits by an
  existing test (`immersed_unsteady_force_f346.rs`, A3a). Options: (a) a corrective changes `build_second_order` itself (that
  test then fails and cannot be edited); (b) a corrective is a further opt-in and `build_second_order` stays bit-identical and
  is refused where it is unstable. Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation): (b).
  Whether the corrected form should replace `build_second_order` is left open (it changes recorded numbers).
- **Q2. The form of the refusal.** ADR 0234 Q3 recommended the divergence guard over a panic. Taken on the same instruction:
  the located stop of the solver (`diverged`, the mechanism of the refusals of ADR 0218 D4), not a panic, with the smallest
  possible hunk in `pimple.rs`.

## Amendment 1 (2026-11-12): the measured cause, the corrective and the refusal

Written after the diagnosis runs D0 to D8 (tests committed in 475e79d1 before their first run; one invocation, release, one
test thread, `f355-scratch/diag1.log`) and before any library code of F-355 and any run of the checks registered here. Nothing
above is changed.

### What D0 to D8 measured

D0 held: R stops at step 6 by the Courant guard, Courant 1.1094e7; the control with `build` runs (largest `abs(u)` 2.21).
`m_n` of R, steps 1 to 5: 2.63, 11.9, 732, 4.0e4, 2.3e5 (ratios 4.5, 62, 55, 5.8: faster than any linear estimate, a nonlinear
growth). Every control of every variant ran.

| Id | Variant of R | Verdict (stop step) | `G` | Hypothesis verdict by the registered number |
| --- | --- | --- | --- | --- |
| H1 | `ddt_correction = false` | stops (6) | 3.32 | **not the cause** |
| H2 | `n_correctors` 1, 3, 4 | stops (11), (6), (4) | 3.32 | **the corrector count is not the cause** (more correctors stop earlier) |
| H3 | implicit Euler | stops (8) | 2.28 | **BDF2 is not necessary** |
| H4 | R, fluid-cell continuity at the last completed step | 7.3e-7 (limit 1e-4) | | **not the cause**: the pressure equation closes the fluid cells, the large flux is real |
| H5 | `theta_min` 0.1, 0.25, 0.5 (largest weight 5, 2, 1) | runs, runs, runs | 1.20, 0.49, 0.34 | **the weight is necessary** (the variant part of the registered number). The located part did not hold as written: at the stop the largest velocity is in cell 9926, which is not a first fluid cell (the disturbance has spread); D9 below repairs the measurement |
| H6 | `Upwind`, `Central` | stops (7), (6) | 3.32 | **not the cause** |
| H7 | `dt` 5e-4, 2e-3, 4e-3, 8e-3 | stops (6), (6), (6), (5) | 6.11, 1.74, 0.89, 0.45 | **the time step is not the cause**, in either direction |
| H7 | "late" (300 steps with `build`, then `build_second_order`) | **runs** 200 steps, `m` 0.67 to 0.69 | 3.32 | **the start is necessary**: on a developed field the forcing of ADR 0234 runs |
| H8 | `nu` 3e-3, 1e-2; M1; CC `dt` 2.5e-3, 2.5e-5 | runs, runs; runs (largest weight 3.77); runs, grows (`max abs(u)` 281 against 0.50) | 1.18, 0.36; 2.09; 0.30, 8.30 | **refuted** by the registered rule: `dt` 4e-3 and 8e-3 stop with `G <= 1`; "late" and M1 run with `G >= 2` |

H8, the hypothesis added in the registration, is refuted as a criterion: the stop is not a linear amplification governed by
the share of the time term (the measured growth is 5 to 60 per step against `G` between 0.45 and 6, and it does not depend on
the time step). What the numbers leave: the stop needs a weight above 5 (H5) **and** the state of the impulsive start (H7), and
nothing else of the list.

### The cause (derived from these numbers; D9 is its registered measurement)

ADR 0234 D1 takes the flux velocity of a fluid-solid face as the linear extension of the fluid profile from the fluid centre
`x_c` through the no-slip value at the crossing, `U_f = HbyA_c + w (u_G - HbyA_c)`, `w = t_f/theta_e`. Relative to the wall
this is `-(w - 1)(HbyA_c - u_G)`: the velocity of the first fluid cell relative to the wall, turned round and multiplied by
`w - 1`, which is `(t_f - theta_e)/theta_e` and reaches 38 on M2 (49 at `theta_min`). The formula is right when the velocity
of the first fluid cell relative to the wall is of order `theta h du/dn`, that is when the profile between the wall and the
cell centre is resolved; the product is then of order `h du/dn`. It is wrong by the factor `w - 1` when the cell holds a
velocity that is not small with `theta`: at the impulsive start of 2D-2 the fluid slips past the cylinder (the layer after one
step has the thickness `sqrt(nu dt) = 1e-3` against `h = 2.5e-3`), the first fluid cells hold a velocity of 2.6 whatever
their `theta`, and the faces with a small `theta` get a flux velocity of up to tens of times the inflow velocity, with the
sign reversed. Continuity then carries that flux into the fluid, the next predictor convects with it, and the run is lost in
five steps. On the developed field ("late") the first fluid cells have the velocity of a resolved layer (the layer of 2D-2 is
about `D/sqrt(Re) = 4 h` thick) and the same weights do no harm; with weights of at most 5 (`theta_min = 0.1`, M1) the start
is survived. In circular Couette flow (ADR 0234 V3) the cell Reynolds number is 0.025 and the wall term dominates the
diagonal of the first fluid cell, so the cell follows the wall within a step; with a step 100 times smaller it does not, and
the same forcing grows there too (`max abs(u)` 281). This is the large-weight failure of the ghost-cell extrapolation near a
node that Tseng and Ferziger describe (cited from memory); ADR 0234 checked it only on a flow in which the first fluid cell is
slaved to the wall.

**D9 (registered now; run with the library of this amendment, the refusal switched off).** R with `build_second_order`,
after its first step (which completes): over the flux faces, `v_f = phi_f/abs(S_f)`; the face with the largest `abs(v_f)`,
its weight, and `abs(u)` of its fluid cell. The same for "late" after its last step. **The cause is confirmed** if in R the
face with the largest `abs(v_f)` has a weight `>= 5` and `abs(v_f) >= 4.5` (three times the inflow maximum 1.5), and in
"late" the largest `abs(v_f)` over the flux faces is `<= 1.5`. Otherwise it is not confirmed and the item is at most partial.
Printed: the largest `abs(v_f - ((1 - w) u_c + w u_G) . n_f)` (the flux against the extension of the cell velocity).

### The corrective (derived): an image point with bounded weights, a further opt-in

Along the line from `x_c` to the solid centre `x_s` (length `L`) let `xi` be the distance from the crossing, positive into
the solid. The smooth extension of the fluid profile is `u(xi) = u_G + xi u' + xi^2 u''/2 + ...`, so

    u(xi) = 2 u_G - u(-xi) + xi^2 u'' + O(xi^3):

the value at a point behind the surface is the wall value reflected through the value at the image point in the fluid, to
second order (the reflection of Mittal et al., cited from memory). The face lies at `xi_f = (t_f - theta_e) L`; its image
lies at the distance `s = (2 theta_e - t_f) L` from `x_c` towards the surface.

- `2 theta_e >= t_f` (`w <= 2`): the image lies between `x_c` and the crossing. Interpolating there linearly between
  `HbyA_c` and `u_G` gives exactly the formula of ADR 0234 D1 (`2 u_G - HbyA_c - (2 - t_f/theta_e)(u_G - HbyA_c)`), with
  `abs(1 - w) <= 1`. These faces keep D1, the same expression.
- `2 theta_e < t_f` (`w > 2`): the image lies behind `x_c`, at the distance `(t_f - 2 theta_e) L` on the far side. With `d`
  the fluid cell behind `c` on the same line (the neighbour of `c` across an internal non-empty face whose centre direction
  from `x_c` is opposite to `x_s - x_c`: cosine `<= -0.99`, the most opposite one) and
  `a = min(1, (t_f - 2 theta_e) L / abs(x_d - x_c))`:

      U_f = 2 u_G - ((1 - a) HbyA_c + a HbyA_d).

  The two forms agree at `2 theta_e = t_f` (`a = 0`, `w = 2`). The weights of `HbyA` are `-(1 - a)` and `-a`: their
  magnitudes sum to 1 for every `theta`, against `w - 1` up to 49. If `c` has no such fluid neighbour (a domain boundary or a
  solid cell behind it), `a = 0` and `d = c`: `U_f = 2 u_G - HbyA_c`, the image taken at the cell centre, first order at that
  face (counted and printed).

Error of the face velocity: the reflection `xi_f^2 abs(u'')`, at most `(t_f h)^2 abs(u'')`, plus the linear interpolation, at
most `h^2 abs(u'')/8`: second order, with a constant up to `0.25 + 0.125` on a uniform mesh against `0.125` of D1. D2 (the
source of the solid cells) and D3 (the net flux of a body) are unchanged and apply to the new flux.

**E1. Opt-in (Q1 of the registration).** A new constructor `ImmersedForcing::build_second_order_bounded(geom, bodies,
settings, gamma)`: `build_second_order` with, on the faces with `2 theta_e < t_f`, the image form. `FluxFace` gets the field
`image: Option<(usize, f64)>` (the cell `d` and the fraction `a`; `None` on every face of `build_second_order` and on the
faces with `w <= 2`). `reconstruct_face_flux` evaluates the image form where `image` is set and the unchanged expression of
D1 elsewhere. `build` and `build_second_order` produce what they produced (the three hashes below).

**E2. The refusal of the unbounded form in the transient solver (Q2 of the registration).** No criterion separates every
stop from every run without the state of the flow (H8 was refuted). The refusal is a sufficient condition, taken from the
two measured necessities and from the runs that must not be refused (ADR 0234 V3, largest weight 12.3, which runs because
the wall term dominates the diagonal): `PimpleSolver` stops the step, before the correctors, when a flux face without an
image has

    weight > 5   and   (weight - 1) c0 V_c / (dt a_P,c) > 1,

`a_P,c` the momentum diagonal of its fluid cell as assembled in that step (with convection and the forcing), `c0 = c1 - c2`
of the time scheme (1 for Euler, 3/2 for BDF2 with a constant step). The second number is the largest velocity, in units of
the velocity the time term carries over from the old level, that the extension can put on the face. Measured before (the
estimate without convection): R 3.32 (2.28 at its first step), `dt` 5e-4 6.11, CC with `dt` 2.5e-5 8.30, against CC V3 0.30
and the F-346 runs with `build_second_order` (CC N 48 and 96 at `dt` 2.5e-3 with BDF2, at most `r/2 = 0.75`). The limits 5
and 1: weights up to 5 were measured to run from the impulsive start; 1 is the value below which the extension cannot return
more than it is given. Known and accepted: the runs with `dt` 4e-3 and 8e-3 (`G` 0.89, 0.45) are **not** refused and stop by
the Courant guard as before; "late" and any restart from a developed field **are** refused although they would run (the
bounded form is the supported path). The stop is a located record (`Divergence`, quantity `immersed flux weight`, the cell,
the value), with a message naming ADR 0254 and `build_second_order_bounded`. `ImmersedForcing` gets the public field
`flux_weight_check: bool` (`true` from every constructor); set to `false` the solver does not make the check (used by the
diagnosis tests so that D0 to D8 stay reproducible, and by D9). SIMPLE has no time term and no check.

Hunks outside `immersed.rs`: `pimple.rs`, one statement after the diagonal is formed and one branch of `Divergence::message`.
`simple.rs` is not touched.

Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation): E1 as a further opt-in rather than a
change of `build_second_order`; E2 as a located stop rather than a panic; the limits 5 and 1.

### Probes of the existing code (disclosed; a scratch test, deleted, not committed; release, tree 475e79d1, library unchanged)

CC at N 24 with `build_second_order` (`K = 1e8`, `nu = 1`, `theta_min = 1e-2`), FNV-1a over the bits:

- P1 `reconstruct_face_flux` on a pseudo-random `hby_a` (LCG of `immersed_second_order_f309.rs`, seed `0x0355`, values in
  `[-1, 1]`, two components per cell), hash over `phi` then the source: `1d4b8502a0aeb637`;
- P2 PISO (`piso(1.0)`, central, Euler), 20 steps of `dt = 0.0025` from rest, hash over `u` then `p`: `36d0549bb0533aa0`;
- P3 SIMPLEC (`SimpleSettings::new(1.0, Simplec)`, central), 40 iterations from rest: `6d18eb02d60d3a7a`.

### Checks registered now (limits fixed; with those of the registration: A, CC-A1, CC-V3, RF, N1)

"Bounded" is `build_second_order_bounded`. The diagnosis tests D0 to D8 get one line (`flux_weight_check = false` on the
second-order forcing) and nothing else; they are not run again to obtain a result, except D0 as the check D0' below.

| Id | Check | Limit |
| --- | --- | --- |
| D9 | the located cause (above) | as defined above |
| D0' | D0 on the final tree with the check switched off | stop at step 6, the Courant guard, Courant 1.1094e7 to the five digits printed |
| P | P1 to P3 on the final tree | the three hashes |
| UB1 | U1 of ADR 0234 with bounded (`theta_min = 1e-9`, the exact Couette field): `abs(phiHbyA_f/abs(S_f) - u_ext(x_f) . n_f)` over the flux faces | maximum at N 96 `<= 3e-3` (`(0.25 + 0.125) h^2 abs(u'')` with `abs(u'') = 9`: 2.1e-3); rms(48)/rms(96) `>= 3` |
| UB2 | U2 of ADR 0234 with bounded (CC N 24, pseudo-random `hby_a`, seed `0x0309`): net flux per body; sum of the sources; sources of the fluid cells; other faces | `<= 1e-12 A_b`; `<= 1e-12 sum A_b`; exactly 0; bit-identical |
| UB3 | structure at CC N 24 and on M2: `coeff`, `source`, `bodies` of bounded equal to `build`; the faces with `weight <= 2` have no image and the weight and wall value of `build_second_order`; every face with `weight > 2` has an image with `0 <= a <= 1`; `build` and `build_second_order` have none | as stated; at least 10 image faces on each mesh; the count of fall-back faces (`d = c`) printed |
| SB | bounded on R and on every variant that stopped (H1, H2 with 1 and 4 correctors, H3, H6 both, H7 with the four time steps), 200 steps each; CC N 48 PISO with `dt` 2.5e-5, 400 steps | every one "runs" (largest `abs(u) <= 10`; CC: `<= 1`, the first-order control gave 0.50) |
| A | as registered, the corrected forcing being bounded | no stop; S2 for both runs; `abs(St_bounded/St_first - 1) <= 1e-2` |
| CC-A1, CC-V3 | as registered, with bounded | as registered |
| RF | as registered: R with `build_second_order`, one step; in addition bounded on R, one step, and ADR 0234 V3's run (20 steps), F-346's annulus set-up (N 48, BDF2, central, `dt` 2.5e-3, 5 steps) and CC SIMPLEC (5 iterations) with `build_second_order` | R: `diverged` at step 1 with quantity `immersed flux weight`, a cell, value `> 1`, and a message containing "ADR 0254" and "build_second_order_bounded"; none of the others stops |

Printed, no limit: for A the extrema and mean of `C_D`, `C_L` of both runs against FeatFlow, and the Strouhal numbers against
0.30188; for CC-A1 `e` and `eps` on the three meshes next to the numbers of ADR 0234 (6.121e-3, 1.611e-3, 4.243e-4; order
1.925), the inner torque; the number of image faces.

**Mutations (the list of the registration, completed).**

| Id | Mutation | Must fail |
| --- | --- | --- |
| M1 | the refusal never fires (the comparison `> 1.0` of the growth number made false) | RF |
| M2 | the refusal fires for every face without an image (both limits removed) | RF (the runs that must not stop) |
| M3 | bounded builds no image (D1 on every face) | SB |
| M4 | the fraction `a` from `t_f - theta_e` instead of `t_f - 2 theta_e` | UB1 |
| M5 | `2 u_G` replaced by `u_G` in the image form | UB1 |
| M6 | `pimple.rs`: the statement of the refusal removed | RF |
| M7 | the image form without the far cell (`a = 0` on every image face) | UB1 |

### Decision rule (unchanged), read with this amendment

"One hypothesis confirmed by its registered number" is: H5 and H7 by the variants above, and D9. The first branch needs A, S2,
CC-A1, CC-V3 with bounded, and RF, P, N1, D0', UB1 to UB3, SB and the mutations; SB or a unit check missed is a known miss
and the item is partial. The acceptance names `build_second_order`; whether the first branch is reached by a further opt-in
is a reading, stated in the addendum with the second branch (the diagnosis and the refusal), which is then reached in any case.

### Limits added

The refusal is a sufficient condition, not a stability criterion (two measured stops are outside it). The time-scheme factor
`c0` is that of Euler for Crank-Nicolson and the bounded BDF2. The image form needs a fluid cell behind the first fluid cell
on the same line; a fluid gap one cell wide falls back to first order at that face. Non-orthogonal meshes are not checked.
SIMPLE with under-relaxation below 1 and a large weight is not examined (the checks of ADR 0234 ran SIMPLEC with 1.0).

## Amendment 2 (2026-11-12): two missed limits, their records and two further measurements

Written after the first run of D9, P, UB1 to UB3, RF, CC-A1, CC-V3, D0' and SB on the library of 68392e71 (one invocation
each, `f355-scratch/run_b.log`, `run_sb.log`), while the run A was in progress, and before the runs registered here. No limit
is changed and no library code is changed by this amendment.

**Missed: D9, the size.** After the first step of R the face with the largest flux velocity has the weight 38.99 (limit
`>= 5`: held) and `abs(v_f) = 2.49` (limit `>= 4.5`: **missed**); "late" after 200 steps: 0.187 (limit `<= 1.5`: held; 0.182
after its first step). The fluid cell of that face holds `abs(u) = 0.095`: after one step the cells with a small `theta` are
already slow, and the flux velocity on the worst face is 1.7 times the inflow maximum, 13 times that of the developed field,
not the "tens of times" the amendment 1 text claims for the first step. The largest difference between the flux velocity and
the extension of the cell velocity is 6.1 (0.029 in "late"): the flux is the extension of `HbyA_c`, which at the start differs
from the cell velocity by the pressure-gradient part, and that difference is what the weight multiplies. The growth to the
stop happens over steps 2 to 5 (`m_n` 11.9, 732, 4.0e4, 2.3e5). The test is split: `d9_located_cause` keeps the two limits that
held, `d9_flux_velocity_after_the_first_step_known_miss` (ignored) keeps the limit 4.5 with the measured 2.49. By the rule of
amendment 1 the cause is therefore **not confirmed in the registered form**; D10 below measures where the growth sits, and
does not replace D9 in the decision rule.

**Missed: SB, one of its twelve runs.** Bounded runs R and nine of the ten variants (largest `abs(u)` 2.09 to 2.26, as the
first-order controls) and CC with `dt = 2.5e-5` (0.496); with **one corrector** (`n_correctors = 1`) it stops at step 34 by
the Courant guard (2.4e7; the first-order control ran; `build_second_order` stopped at step 11). The test is split:
`sb_bounded_runs_the_variants_that_stopped` keeps the eleven runs that held, `sb_bounded_with_one_corrector_known_miss`
(ignored) keeps the limit with the measured step 34. D11 below is its diagnosis.

### Measurements registered now

| Id | Measurement | Decides |
| --- | --- | --- |
| D10 | R with `build_second_order` (the check off), after each of the steps 1 to 5: the flux face with the largest `abs(v_f)`, its weight. Bounded on R, the same after each of the steps 1 to 5 and after step 200 | **the growth sits on the large-weight faces** if the weight is `>= 5` at each of the five steps and `abs(v_f) >= 45` (30 times the inflow maximum) at step 3; **the bounded form removes it** if its largest `abs(v_f)` is `<= 3` (twice the inflow maximum) at each of the six instants. Known beforehand: `m_3 = 732` (a cell velocity, not a flux velocity) |
| D11 | R with one corrector, at most 200 steps, three runs: (a) `build_second_order` with `theta_min = 0.5` (every weight at most 1, the flux velocity an interpolation); (b) `build_second_order` with `theta_min = 0.25` (weights up to 2: the flux velocity up to the reflection `-HbyA_c`, no image face); (c) bounded with `theta_min = 0.1` (image faces with `a <= 0.3`) | (a) runs and (b) stops: a reflection of magnitude up to 1 on the fluid-solid faces is not survived by PISO with a single corrector, whatever the stencil (the bounded form then needs two correctors, as does D1 with `w = 2`). (a) stops: the reconstructed flux with the source needs a second corrector at Re 100 as such. (a) and (b) run: the image stencil is the cause. (c) is printed |

Hand estimate behind D11 (disclosed): with one corrector the cell velocity is corrected with the gradient of a pressure that
was solved for the fluxes of the predictor's `HbyA`, and the flux of the next step's flux faces is built from the `HbyA` of a
velocity that was never made consistent with them; a second corrector repeats the reconstruction on the corrected velocity.
The reflection returns the full wall-normal part of `HbyA_c`, so an error of the single corrector is returned at full size.
No number is predicted.

The decision rule is unchanged: with D9 and one run of SB missed the item is at most **partial** by the rule, whatever A gives.

## Addendum 1: results (2026-11-12)

Measured against the limits registered above. No limit was changed. Release builds on Windows, the machine shared with other
jobs. Commits: the registration 7c767055; the diagnosis tests 475e79d1 (before their run); amendment 1 9ec118e1 (before any
library code); the library and its checks 68392e71 (before their run); amendment 2 7bbaf5f3 and the split tests 1c72ecbe (before
the runs of D10 and D11). The library was not changed after 68392e71. Every registered run was made once (D0 a second time on
the final library, as the registered check D0'); after the mutations the whole test file except the run A was run once more on
the restored tree, with the same verdicts.

**Which branch of the acceptance was reached.** The **second**, in full: the cause of the stop is diagnosed by registered
measurements (below), and the configuration, 2D-2 on M2 with `build_second_order` in PISO, is refused at step 1 with a located
message. The **first** is reached in substance but not in its letter: 2D-2 on M2 runs to `t = 8` in PISO with BDF2 and gives a
Strouhal number within 0.24 percent of the first-order forcing, with the second-order forcing in its bounded form
`build_second_order_bounded`, a further opt-in; `build_second_order` itself is unchanged, bit for bit, and still cannot make
that run. Whether the bounded form should become what `build_second_order` builds is Q1.

**Built.** `crates/rcfd-fv/src/immersed.rs`: the field `FluxFace::image`, the constant `FLUX_WEIGHT_LIMIT`, the field
`ImmersedForcing::flux_weight_check`, `ImmersedForcing::build_second_order_bounded`, the image branch of
`reconstruct_face_flux`, `ImmersedForcing::flux_growth`; `build` and `build_second_order` build what they built.
`crates/rcfd-fv/src/pimple.rs`, two hunks: lines 482 to 488 (a branch of `Divergence::message` for the quantity
`immersed flux weight`, seven lines added after the `immersed solid` branch) and lines 1296 to 1312 (the refusal, seventeen
lines added after the normalisation of `h1` and before `if lag_due && last`); nothing else of that file is touched.
`simple.rs`, `sim.rs`, `schema.rs` are not touched. `crates/rcfd-fv/tests/immersed_piso_stop_f355.rs` (new).
`docs/reference-data.md` (a section for F-355). No case-schema key, golden, hash table or existing test file was touched.

### Hypotheses

| Id | Hypothesis | Measurement (R = 2D-2 on M2, the run of ADR 0249; variants 200 steps; every first-order control ran) | Verdict |
| --- | --- | --- | --- |
| H1 (a) | the `ddt` flux correction at the flux faces | `ddt_correction = false`: stops at step 6 (Courant 1.08e7 against 1.11e7) | **ruled out** |
| H2 (b) | the reconstruction missing in later correctors | the code calls it in every corrector; 1, 3, 4 correctors stop at steps 11, 6, 4 | **ruled out** (more correctors stop sooner) |
| H3 (c) | the BDF2 old and old-old fluxes | implicit Euler stops at step 8 | **ruled out** |
| H4 (d) | the solid-cell source in the continuity error or the corrector control | no corrector control in the run; fluid-cell continuity over the largest flux 7.3e-7 at the last completed step (limit 1e-4) | **ruled out**: the large flux is a real flux |
| H5 (e) | a large extrapolation weight | `theta_min` 0.1, 0.25, 0.5 (largest weight 5, 2, 1) all run; M1 (largest weight 3.77) runs; D10: at each of the steps 1 to 5 of R the largest flux velocity is on the face of weight 38.99 (2.49, 23.1, 1426, 9.2e4, 6.9e5) | **necessary, and the place of the growth** (the registered location test at the stop did not hold: by then the largest velocity is elsewhere; D10 is the repaired measurement) |
| H6 (f) | linear upwind reading solid-cell values | `Upwind` stops at step 7, `Central` at step 6 | **ruled out** |
| H7 (g) | the time step | `dt` 5e-4, 2e-3, 4e-3, 8e-3 stop at steps 6, 6, 6, 5 | **ruled out**, in both directions |
| H7 (g) | the impulsive start | "late" (300 steps with `build`, then `build_second_order`) runs 200 steps, first fluid cells 0.67 to 0.69, largest flux velocity 0.19 | **necessary**: on a developed field the forcing of ADR 0234 runs |
| H8 (added) | weight times the share of the time term in the diagonal as a growth criterion | `dt` 4e-3 and 8e-3 stop with `G` 0.89 and 0.45; "late" and M1 run with `G` 3.32 and 2.09; CC with `dt` 2.5e-5 grows (`max abs(u)` 281), with 2.5e-3 runs | **refuted** as a criterion (my estimate; measured growth 5 to 60 per step, independent of the step) |

**The cause, as far as the numbers carry it.** The stop needs two things together and nothing else of the list: a fluid-solid
face whose crossing lies close to the fluid centre, so that the linear extension of ADR 0234 D1 has a weight above 5 (it
multiplies the velocity of the fluid cell relative to the wall by `w - 1`, 38 on M2), and a state in which what is extended is
not small with `theta`, here the impulsive start. The flux is the extension of `HbyA_c`, not of the cell velocity: after the
first step the two differ at the worst face by 6.1 (0.029 on the developed field), the weight times the pressure-gradient part
of the cell velocity. On the face of weight 38.99 the flux velocity is 2.49 after step 1 (13 times the developed value) and
grows by 9, 62, 64 and 7.5 per step. With weights of at most 5, or on the developed field, or with the bounded form (largest
flux velocity 0.87 after step 1, 0.19 after 200 steps) there is no growth. **Not confirmed in the registered form:** D9 asked
for a flux velocity of at least 4.5 after the first step and measured 2.49; my statement in amendment 1 that the first step
already puts tens of times the inflow velocity on those faces was wrong, the growth builds over steps 2 to 5. Why exactly the
developed field is immune (the size of the pressure-gradient part, the resolved layer, or both) is not separated by a
measurement.

### Checks

| Check | Limit | Measured | Met |
| --- | --- | --- | --- |
| D0 the reproduction | stop at step 6, Courant guard, 1.0e7 to 1.2e7 | step 6, 1.1094e7 | yes |
| C-R first-order forcing on R | runs | runs, `max abs(u)` 2.21 | yes |
| D1 to D8 controls | every control runs | 18 of 18, and the CC controls | yes |
| D8 verdict | "confirmed" or "refuted" as defined | refuted | (a verdict) |
| D9 worst face of R after step 1: weight | `>= 5` | 38.99 | yes |
| D9 worst face of R after step 1: flux velocity | `>= 4.5` | **2.49** | **no** (ignored known miss, limit intact) |
| D9 "late" after 200 steps: largest flux velocity | `<= 1.5` | 0.187 | yes |
| D10 R: weight of the worst face, steps 1 to 5; flux velocity at step 3 | `>= 5`; `>= 45` | 38.99 at each; 1426 | yes |
| D10 bounded: largest flux velocity, steps 1 to 5 and 200 | `<= 3` | 0.87, 0.70, 0.40, 0.32, 0.30, 0.19 | yes |
| D11 one corrector: (a) weights at most 1; (b) weights up to 2, no image; (c) bounded, `theta_min` 0.1 | a reading | (a) runs; (b) stops at step 126; (c) stops at step 46 | reading: a reflection of magnitude up to 1 at the fluid-solid faces is not survived by PISO with one corrector, whatever the stencil |
| D0' `build_second_order` on the final library, check off | step 6, 1.1094e7 | step 6, 1.1094e7 (`m_n` to all digits as before) | yes |
| P hashes of `build_second_order` (reconstruction, PISO, SIMPLEC) | `1d4b8502a0aeb637`, `36d0549bb0533aa0`, `6d18eb02d60d3a7a` | the same three | yes |
| UB1 bounded reconstruction of the exact Couette field | max at N 96 `<= 3e-3`; rms(48)/rms(96) `>= 3` | 7.99e-4 (2.56e-3 at N 48); 3.858 (ADR 0234 U1: 3.24e-4; 3.889) | yes |
| UB2 net flux per body; sum of the sources; fluid cells; other faces | `<= 1e-12 A_b`; `<= 1e-12 sum A_b`; 0; bit-identical | 4.3e-19 and 5.2e-18; -1.8e-17; 0; bit-identical | yes |
| UB3 structure, CC N 24 and M2 | as registered; at least 10 image faces | CC: 120 flux faces, 32 with an image; M2: 160, 32; no fall-back face | yes |
| SB bounded on R and nine variants; CC with `dt` 2.5e-5 | every one runs (`<= 10`; CC `<= 1`) | all run, `max abs(u)` 2.09 to 2.26; CC 0.496 | yes |
| SB bounded with one corrector | runs | **stops at step 34** (Courant 2.4e7) | **no** (ignored known miss, limit intact) |
| A (acceptance, first branch, with bounded) 2D-2 on M2 to `t = 8` | no stop; `abs(St_bounded/St_first - 1) <= 1e-2` | 8000 steps; `St` 0.30112 against 0.30041: **+0.237 percent** | **yes** |
| S2 of A, both runs | at least 6 crossings; periods within 1 percent; maxima within 2 percent; amplitude at least 1 | bounded 7, 8.0e-6, 3.1e-4, 1.987; first 7, 8.5e-6, 3.1e-4, 2.004 | held |
| CC-A1 bounded: `e(96)`; order 48 to 96; S1 | `<= 1e-2`; `>= 1.5`; converged | 4.232e-4; **1.947**; 102, 103, 340 iterations | yes |
| CC-V3 bounded: steady; `e`; `eps` against SIMPLEC | `<= 1e-9`; `<= 1.2`; `<= 1.5` times | 1.3e-12; 1.62978e-3 against 1.63149e-3; 1.10522e-3 against 1.11240e-3 | yes |
| RF R with `build_second_order`, one step | stopped at step 1, quantity, cell, value `> 1`, message | step 1, `immersed flux weight`, cell 5679 at (0.18875, 0.15125), value 2.206, message names ADR 0254 and `build_second_order_bounded` | yes |
| RF bounded on R; ADR 0234 V3; ADR 0249's annulus; CC SIMPLEC, with `build_second_order` | none stops | none stops | yes |
| N1 the four existing immersed test files, unedited | every test that is not ignored passes | `immersed_solid_f219` 11 passed, 2 ignored; `immersed_second_order_f309` 9 passed, 1 ignored; `immersed_force_order_f337` 5 passed, 2 ignored; `immersed_unsteady_force_f346` 9 passed, 5 ignored (the ignored ones are those files' own records and printed runs) | yes |
| N1 `cargo test -p rcfd-api --release --test baseline_hashes` | passes | 3 passed, 1 ignored | yes |

Printed, no limit. 2D-2 on M2, window from `t = 5.5`, against FeatFlow level 6: bounded `St` 0.30112 (-0.250 percent), `C_D`
max 3.20930 (-0.332), min 3.14799 (-0.282), mean 3.17889 (-0.298), `C_L` max 0.97593 (-1.011), min -1.01116 (-0.925); first
order `St` 0.30041 (-0.486), `C_D` max 3.22841 (+0.261), `C_L` max 0.98432 (-0.161), the numbers of ADR 0249. Circular Couette
with bounded next to ADR 0234 (`build_second_order`): `e` 5.912e-3, 1.631e-3, 4.232e-4 against 6.121e-3, 1.611e-3, 4.243e-4
(orders 1.857, 1.947 against 1.926, 1.925); `eps` 3.923e-3, 1.112e-3, 3.078e-4 against 1.903e-3, 4.737e-4, 1.612e-4 (the face
flux error is about twice as large, as the larger constant of the reflection says; the velocity error is the same); inner
torque -4.04401, -4.12191, -4.15778 against -4.04376, -4.12171, -4.15767; image faces 32 of 120, 80 of 240, 88 of 480. The
value of the refusal on R at step 1 is 2.206 (the estimate without convection gave 2.28).

### The two known misses

- **D9, the size of the flux velocity after the first step** (2.49 against 4.5). Diagnosed above: the prediction was wrong,
  not the set-up. `d9_flux_velocity_after_the_first_step_known_miss`.
- **SB, the bounded form with one corrector** (stops at step 34; `build_second_order` at step 11; the first-order forcing
  runs). D11: with one corrector the unbounded form with weights of at most 1 runs and with weights up to 2 (no image face)
  stops at step 126; the bounded form with `theta_min = 0.1` stops at step 46. So the stop does not come from the image stencil
  but from any flux face that returns the wall-normal part of `HbyA_c` at full size, with a single corrector; with two or more
  correctors the same faces run (H5 `theta_min` 0.25, SB). Not measured: the mechanism (the hand argument of amendment 2 is a
  hypothesis), and whether more than 200 steps with two correctors and weights between 1 and 2 stay bounded beyond what A
  shows (8000 steps with the bounded form). `sb_bounded_with_one_corrector_known_miss`. The solver does not refuse this
  setting; Q3.

### Mutations

Applied one at a time to the final tree by `w49i-work/f355-scratch/mutate.py` (an exact text replacement checked to occur
once), the named test run once in release, the file restored and compared byte for byte with the kept copy (equal after each).

| Id | Mutation | Must fail | Result |
| --- | --- | --- | --- |
| M1 | the refusal never fires | RF | failed: "R with build_second_order is not refused" |
| M2 | the refusal fires for every face without an image | RF | failed: the bounded forcing on R refused at step 1 |
| M3 | bounded builds no image | SB | failed: every run stopped at step 1 by the refusal (value 2.206) |
| M4 | `a` from `t_f - theta_e` | UB1 | failed: max 5.90e-3, rms ratio 2.898 |
| M5 | `2 u_G -> u_G` in the image form | UB1 | failed: max 0.487, rms ratio 1.07 |
| M6 | `pimple.rs`: the refusal statement disabled | RF | failed: not refused |
| M7 | the image form without the far cell | UB1 | failed: max 7.34e-3, rms ratio 2.27 |

M3 was caught through the refusal, not through a stop of the unbounded faces: with the mutation the bounded constructor builds
the forcing of `build_second_order` with the check on. That the forcing without images stops on R is D0.

### Tools

`cargo fmt -p rcfd-fv --check`: clean. `cargo clippy -p rcfd-fv --all-targets -- -D warnings`: clean. `cargo test -p rcfd-fv
--release --test immersed_piso_stop_f355`: 15 passed, 2 ignored (the two known misses), over the invocations named above; in
the debug profile the heavy tests are ignored. The whole workspace suite and the gate were not run.

### Deviations and disclosures

- The stop was reproduced with the existing test before the registration (disclosed there).
- H8, the hypothesis I added with a hand estimate, was refuted by its own registered rule. The refusal E2 nevertheless uses
  the same number with the solver's diagonal, as a sufficient condition chosen so that the runs of ADR 0234 and 0249 are not
  refused; it is not a stability criterion, it lets two measured stops through (`dt` 4e-3 and 8e-3, which stop by the Courant
  guard as before) and it refuses "late", which runs. Amendment 1 says so before the code.
- The text of amendment 1 on the first step ("tens of times the inflow velocity") is wrong; amendment 2 and this addendum
  correct it, the text stays.
- The diagnosis tests got the line that switches the refusal off (`flux_weight_check = false` in the helper that builds the
  unbounded forcing), as amendment 1 registered. Two tests were split into their met parts and their known misses
  (amendment 2). Clippy asked for the negated comparisons of the new tests to go through a helper (`le`) and for `map_or`
  in place of `is_none_or` (the crate's minimum Rust version) before the commit 68392e71 and its runs.
- `G` printed by the tests is the estimate without convection, from the forcing of `build_second_order`, also on the lines of
  the bounded and first-order runs (there it is only the label of the variant).
- Not run: the workspace suite and the gate; M3 and T with the bounded form; PIMPLE with outer iterations, the consistent
  correctors, the adaptive corrector count; SIMPLE with under-relaxation; DFG 2D-1 with the bounded form (the steady numbers
  of ADR 0234 V4 and ADR 0239 are those of `build_second_order`, which is unchanged); three dimensions; a mesh with a fall-back
  face; `Crank-Nicolson` and the bounded BDF2.

### Decision by the registered rule

D0 held; H5 and H7 are confirmed by their variants and D10 locates the growth, but D9 missed one of its three limits, so the
cause is not confirmed in the registered form; A, S2, CC-A1, CC-V3, RF, P, D0', UB1 to UB3 and N1 are met; SB missed one of
its twelve runs; the seven mutations were caught. **F-355 is partial by the rule** (two known misses: D9's size, SB with one
corrector). In terms of the acceptance text: its second branch is met, and its first branch is met with the bounded opt-in
and not with `build_second_order` as named.

### Remaining questions

- **Q1. Should `build_second_order` build the bounded form?** Options: (a) keep both: `build_second_order` unchanged and
  refused where E2 applies, `build_second_order_bounded` for transient runs (as built); (b) make the bounded form what
  `build_second_order` builds: one constructor, no refusal needed, but the recorded numbers of ADR 0234 and 0239 change (CC
  `e(96)` 4.243e-4 to 4.232e-4, `eps(96)` 1.612e-4 to 3.078e-4, the 2D-1 values asserted to nine digits by
  `immersed_unsteady_force_f346.rs` A3a, which would have to be re-recorded by its owner). Recommendation: (b), with the
  re-measurement registered; until then (a). Not taken: it changes recorded numbers of a merged opt-in.
- **Q2. The refusal E2.** Options: (a) keep it as a sufficient condition (as built); (b) refuse every flux face without an
  image whose weight exceeds 5 in the transient solver (simpler, no hole, but it refuses ADR 0234 V3 and ADR 0249's runs,
  which are existing tests); (c) drop it with Q1 (b). Recommendation: (c) if Q1 (b) is taken, else (a).
- **Q3. One corrector.** The second-order flux (either form, weights above 1) stops with a single PISO corrector on 2D-2.
  Options: (a) a follow-up item that measures the mechanism and refuses the setting with a located message; (b) refuse now
  `n_correctors = 1` with flux faces without a measurement of the mechanism; (c) document it. Recommendation: (a).
- **Q4. The pressure-gradient part.** The flux extends `HbyA_c`, not the cell velocity; the difference is what the weight
  multiplies at the start. Options: (a) leave it (bounded weights make it harmless in every run made); (b) a follow-up that
  extends the corrected velocity (the pressure part of the face flux written at the crossing), which would also say why the
  developed field is immune. Recommendation: (a) now; (b) only if a run with the bounded form shows a wall-normal error.
- **Q5. The lift of 2D-2 with the bounded form** is -1.01 percent of FeatFlow on M2 against -0.16 percent of the first-order
  forcing (the drag -0.33 against +0.26). One mesh; F-354 is the item that measures the convergence, and should run both
  forcings.

### For the lead: roadmap entries

**F-355: proposed status `partial`.** Proposed note (replacing the present one):

> PARTIAL (ADR 0254, 2026-11-12), in the library. From ADR 0249 question 6. The stop was reproduced (step 6, Courant 1.1094e7) and diagnosed by registered variant runs, each with its first-order control: it needs a fluid-solid face whose surface crossing lies close to the fluid cell centre, so that the linear extension of ADR 0234 D1 has a weight above 5 (it multiplies the velocity of the first fluid cell relative to the wall by up to 38 on M2), together with the impulsive start; runs with weights of at most 5 (theta_min 0.1, 0.25, 0.5; mesh M1) and a run started from a developed field do not stop, and at each of the five steps the largest flux velocity sits on the face of weight 38.99 (2.49, 23, 1426, 9.2e4, 6.9e5). Ruled out, each by its number: the ddt flux correction, the corrector count, BDF2, the convection scheme, the time step (5e-4 to 8e-3 all stop) and the continuity of the fluid cells (7e-7). A hand estimate of a linear growth criterion (weight times the share of the time term in the diagonal) was refuted by its registered rule. Delivered: rcfd-fv immersed.rs, `ImmersedForcing::build_second_order_bounded` (opt-in): on the faces with a weight above 2 the flux is that of the wall value reflected through the image point of the face in the fluid, interpolated between the first fluid cell and the cell behind it, weights of magnitude at most 1, second order; `build` and `build_second_order` are bit-identical (three hashes, the existing immersed tests, baseline_hashes). With it DFG 2D-2 on M2 runs to t = 8 in PISO with BDF2: Strouhal number 0.30112 against 0.30041 of the first-order forcing (+0.24 percent, limit 1), C_D max -0.33 percent and C_L max -1.01 percent of FeatFlow; circular Couette error 5.91e-3, 1.63e-3, 4.23e-4, order 1.947 (limit 1.5), PISO equal to SIMPLEC to 0.1 percent. `build_second_order` itself is refused by PimpleSolver with a located message where a flux face has a weight above 5 and can return more than the time term carries (`flux_growth`; 2D-2 on M2 at step 1, value 2.2; the runs of ADR 0234 and 0249 are not refused); the refusal is a sufficient condition, not a stability criterion (time steps of 4e-3 and 8e-3 still stop by the Courant guard). Seven mutations caught. Partial by the registered rule because of two known misses: the flux velocity after the first step was registered as at least 4.5 and is 2.49 (the growth builds over steps 2 to 5), and the bounded form with a single PISO corrector stops at step 34 (any flux face that returns the wall-normal velocity at full size does, with one corrector; with two or more it runs). Acceptance: the second branch (diagnosis and located refusal) is met; the first is met with the bounded opt-in, not with `build_second_order` as named. Open (ADR 0254 questions 1 to 3): whether `build_second_order` should build the bounded form (changes recorded numbers of ADR 0234 and 0239), the form of the refusal, the single corrector. Not run: M3, T, three dimensions, PIMPLE outer iterations, SIMPLE with under-relaxation.

**F-309: status unchanged (`done`, pending acceptance).** Proposed change of its note: the last three sentences ("FOUND LATER (ADR 0249, 2026-11-12): ... Not diagnosed: F-355.") are replaced by:

> FOUND LATER (ADR 0249, 2026-11-12): the second-order forcing stopped at step 6 of DFG 2D-2 on M2 in PISO with BDF2 (divergence guard, Courant 1.1e7); it had been run in PISO only on circular Couette flow. DIAGNOSED (ADR 0254, 2026-11-12, F-355): the linear extension of D1 has a weight of up to 38 where the surface crossing lies close to the fluid cell centre, and at an impulsive start, or with a time step small against h^2/nu (circular Couette with dt 2.5e-5 grows to 281), what it multiplies is not small; `build_second_order` is unchanged and is refused by PimpleSolver with a located message in that range; transient runs use `build_second_order_bounded` (ADR 0254), which keeps the order (1.947 on circular Couette). Open: whether `build_second_order` should build the bounded form (ADR 0254 question 1).

**F-354:** no change of status; proposed addition to its note:

> ADR 0254 (2026-11-12) measured 2D-2 on M2 with `build_second_order_bounded`: max C_D 3.20930 (-0.33 percent), max C_L 0.97593 (-1.01 percent), St 0.30112 (-0.25 percent); the convergence runs should be made with both forcings.

**Proposed follow-up items:**

1. *Second-order immersed forcing: the single PISO corrector.* Deps: F-355. Acceptance: "The stop of DFG 2D-2 on M2 with one PISO corrector and a second-order immersed flux face of weight above 1 (ADR 0254 D11: step 34 with the bounded form, step 126 with weights up to 2) is explained by a registered measurement, and the setting either runs 200 steps or is refused with a located message; two correctors are unaffected." Note:

> From ADR 0254 question 3, recommendation taken on the maintainer's standing instruction of 2026-11-11. ADR 0254 measured: with one corrector the first-order forcing and the second-order forcing with weights of at most 1 run, weights up to 2 stop at step 126, the bounded form at step 34 (theta_min 0.1: step 46); with two or more correctors all of these run. The mechanism is not measured.

2. *Second-order immersed forcing: one constructor with bounded weights.* Deps: F-355. Acceptance: "`build_second_order` builds the bounded form of ADR 0254, the refusal of the unbounded form is removed, and the checks of ADR 0234 (A1, V1 to V4) and ADR 0239 are measured again and reported next to the old numbers." Note:

> From ADR 0254 questions 1 and 2. Needs the maintainer's decision: it changes the recorded numbers of a merged opt-in (circular Couette e(96) 4.243e-4 to 4.232e-4, face-flux error 1.612e-4 to 3.078e-4; the DFG 2D-1 values asserted to nine digits in immersed_unsteady_force_f346.rs).
