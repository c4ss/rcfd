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
