# ADR 0824: F-885 registration: the lag of the kept convection gradient, its arms fixed in advance, a corrected variant, then check H

Status: **REGISTERED 2026-11-27, written before any code of F-885 for this registration and before any run of its checks.**
Before this text no test, harness, solver change or solver run of this work was written or made. What was done: reading
(below), one reading script over `docs/roadmap.json` (F-885's status, acceptance and gap), `git apply --check` of the nine
committed F-885 mutation patches at the base 689fab92 (all nine apply), and the arithmetic of this page. Results go in
addenda at the end; nothing above them is changed after a run.

It answers the brief [`docs/adr/briefs/f885-kept-gradient-lag.md`](briefs/f885-kept-gradient-lag.md) and follows
[`docs/process/agent-rules.md`](../process/agent-rules.md), version 5. A follow-up of F-885's miss under rule 13: answer 1
of [ADR 0822](0822-f885-registration-the-order-floor-of-the-pimple-step-a-finer-order-case-and-check-h.md) (a diagnosis of
the kept gradient's lag with its arms fixed in advance). Branch `f885-kept-gradient-lag` from 689fab92; scratch
`w49i-work/f885-kept-gradient-lag-scratch`. Written by the model Claude Opus 5.5.

Size (agent): done; 0; 3; 0; 160000

## What is asked

F-885's acceptance (read by script): "... Check H of ADR 0183 is then run as registered there and in ADR 0766, with the
outer-iteration rule of ADR 0183 as written (the velocity change of the 5th outer iteration at most 1e-2 of the 1st in at
least 99 % of the window's steps), and classes the hub pocket as steady, physical or numerical, or is recorded as not
deciding." The gap: on ADR 0822's corrected order case (N = 64) the remedy `outer_frozen_convection_gradient` gives `p_f`
1.822 against 1.9 (the control 2.035), `e_ON / e_OFF` rising 1.36, 1.43, 1.66 as `dt` falls.

## What was read

Whole: `AGENTS.md`; `docs/process/agent-rules.md` (version 5); the brief; ADR 0822 (registration, amendment 1, addendum 1,
answers); `crates/rcfd-api/tests/f885_order_case.rs`. In part: ADR 0821 section 3 (the steady check S) and its table of
checks; `crates/rcfd-api/tests/f885_frozen_convection_check_h.rs` lines 905 to 1060 (S, `cavity`, `steady_settings`,
`march`); `crates/rcfd-fv/src/pimple.rs` (base 689fab92) lines 200 to 280 (the F-885 settings,
`periodic_partner_extrapolated`), 1170 to 1200 (the constructor), 1940 to 1955 (the kept state of a step), 2030 to 2260
(the momentum predictor); `crates/rcfd-fv/src/gradient.rs` lines 101 to 172 (`least_squares`, `least_squares_partner`);
`crates/rcfd-fv/src/field.rs` lines 175 to 190 (`VectorField::boundary_values`); `docs/process/tolerances.md`, the table.

No published source outside the repository was read. The mechanism is read from the code; the arithmetic is from first
principles.

## 1. The code read, and the arithmetic

In the momentum predictor (`pimple.rs`, base 689fab92): line 2046 `ub = self.u.boundary_values(&g)`, the boundary values
of the current iterate (at the step's first outer iteration the iterate is `u^n`); line 2074 `extrap = Some(c1 - 1)` on the
first outer iteration of a BDF2 step; line 2191 `bv` the component of `ub`; lines 2192 to 2197 `conv_cells` the
extrapolated cells `u* = (1 + r) u^n - r u^(n-1)` on that iteration (the iterate otherwise); line 2199
`least_squares(&g, &conv_cells, &bv)`; lines 2205 to 2208 the setting keeps that gradient (`frozen_grad_conv`, line 1950)
for every later outer iteration. `least_squares_partner` (`gradient.rs` lines 163 to 169) takes on every boundary face that
is not empty the difference `bvals[f] - cells[own[f]]`; on a translational periodic face `bvals` is the coupled
coefficient's face value of `self.u` (`field.rs` line 186), so of `u^n`.

So the kept gradient mixes two time levels on every boundary face whose value depends on the cells (the periodic seam, a
zero-gradient face): the cells are `u*`, the face values are of `u^n`. Without the setting this mix lives only in the first
outer iteration and the later ones form the gradient anew from one level; with it, the mix is kept to the end of the step.

Arithmetic (uniform square cells of side `h`, a cell on the seam with one internal neighbour and the seam face along x):
the weights of `least_squares` give `M_xx = h^2 / h^2 + (h / 2)^2 / (h / 2)^2 = 2` and the error of the face's term in the
right-hand side `(1 / (h / 2)^2) (h / 2) delta = 2 delta / h`, with `delta = u^n_f - u*_f`; at a constant step BDF2's
`r = 1`, so `u* - u^n = u^n - u^(n-1) = dt u_t + O(dt^2)` and the gradient's error is `delta / h = -dt u_t / h`. Linear
upwind's deferred correction `(x_f - x_U) . grad` at a face of the cell is then wrong by `(h / 2) dt u_t / h = dt u_t / 2`:
an `O(dt)` error of the face value, independent of `h`, on the faces of the cells next to the seams, kept in the converged
step. It adds to BDF2's `O(dt^2)` a part of order 1 whose weight in the relative L2 falls as the share of those cells
(`2 / N` a direction) falls; so `e_ON / e_OFF` grows as `dt` falls and falls as `N` grows, as measured.

## 2. The hypotheses and their arms (fixed now)

Every arm is the order harness of ADR 0822 (`tg_run`, `order_arm`, copied unchanged): `box_2d(N, N, 2 pi, 2 pi, true,
true)`, the settings of ADR 0821's O (`o_settings`), `dt` 0.1, 0.05, 0.025 to `t = 1` against `dt_ref = 1/640` on the same
mesh, `e` the relative L2 of the in-plane velocity, `p_c = log2(e(0.1) / e(0.05))`, `p_f = log2(e(0.05) / e(0.025))`. Two
meshes: **N = 64** with the default step (ADR 0822's O'), and **N = 16 with `rhie_chow_step_consistent`** (ADR 0822's D-rc,
2.022 without the setting; answer 2 of ADR 0822). The arms with the setting on:

* **ON**: `outer_frozen_convection_gradient` as it is.
* **A, the kept gradient's boundary values** (`pimple.rs` lines 2046, 2191, 2199). Arm: the new opt-in
  `outer_frozen_convection_gradient_extrapolated_boundary` (section 3): on the first outer iteration of a BDF2 step the
  gradient's boundary values are those of the extrapolated field. Predicted: the lag removed, `p_f >= 1.9` on both meshes,
  `e_A / e_OFF` about constant in `dt`.
* **B, the periodic partner values of the convection** (lines 2160 to 2190). Arm: the existing
  `periodic_partner_extrapolated` with the setting. It changes the coupled faces' coefficients of the first iteration's
  convection only, which the later iterations assemble anew, and not `bv`: predicted `abs(p_f(B) - p_f(ON)) <= 0.05`.
* **C, the time level at which the gradient is kept** (lines 2205 to 2208). Arm: the new opt-in
  `outer_frozen_convection_gradient_second` (section 3): the first outer iteration forms its own gradient, the gradient of
  the second (cells and boundary values of the one iterate `u^(1)`) is kept for the third on. Predicted: `p_f >= 1.9` on
  both meshes (`u^(1)` differs from `u^(n+1)` by much less than `dt`).
* **D, another source found in the code: the first step.** On the first step of BDF2 (`c2 = 0`, implicit Euler) `extrap`
  is `None` and the kept gradient is of `u^n`, an `O(dt)` lag over one step, so `O(dt^2)` in the solution. Arm: the exact
  BDF2 start of ADR 0822's D-s (`exact_start`) with the setting. Predicted: `abs(p_f(D) - p_f(ON)) <= 0.1`.

**The reading (fixed now).** A stands if `p_f(A) >= 1.9` on both meshes and `p_f(A) - p_f(ON) >= 0.05` at N = 64. B is
ruled out if `abs(p_f(B) - p_f(ON)) <= 0.05` on both meshes. C stands if `p_f(C) >= 1.9` on both meshes. D is ruled out
if `abs(p_f(D) - p_f(ON)) <= 0.1` on both meshes. The thresholds are those of ADR 0822's H2 rule (0.05) and twice it for
D (the exact start changed `e` by a factor 2 there).

**R1 (set-up):** the ON arm at N = 64 reproduces ADR 0822's ON errors (3.7568e-4, 9.7105e-5, 2.7461e-5): each relative
difference `<= 1e-4` (the five printed digits, as ADR 0822 R0). It also shows the setting's bytes unchanged by this work.

## 3. The variants (the code, fixed now)

Two fields of `PimpleSettings` (`crates/rcfd-fv/src/pimple.rs`), both default `false`, both read only with
`outer_frozen_convection_gradient` and only where a convection gradient is formed:

* `outer_frozen_convection_gradient_extrapolated_boundary`: on the step's first outer iteration where the cells are
  extrapolated (`extrap` is `Some`), the boundary values given to the convection gradient (`bv` at line 2199) are
  `VectorField::boundary_values` of a copy of `self.u` whose cell values are the extrapolated cells (as `bc_extrapolated`
  builds it, lines 2160 to 2171). The convection's coefficients, the Laplacian's gradient and the limiter keep theirs.
* `outer_frozen_convection_gradient_second`: the gradient is kept from the step's second outer iteration; the first uses
  the gradient it forms. With one outer iteration nothing is kept.

**The corrected remedy** is A if its order checks (K64-A, K16-A and the controls K64-OFF, K16-OFF) are met; otherwise C if
K64-C and K16-C are met; otherwise none, and check H is not run. Check H is coded with A; if A is not the corrected remedy
and C is, an amendment committed before check H's run switches it to C.

Defaults are bit-identical: with both fields `false` no line runs that did not run before.

## 4. The steady check S (fixed now)

ADR 0821's S unchanged (the 8 x 8 cavity, `steady_settings`, `dt` 0.2, the step change `<= 1e-12` within 5 000 steps), the
ON arm with the variant in addition, against OFF; for A and for C. S1: both arms reach the criterion; S2: velocity and face
flux relative `<= 1e-8`; S3: after 5 steps `> 1e-6`. Source: `docs/process/tolerances.md`, "Same discrete problem by two
routes". Estimate as ADR 0821 (3.4e-13 measured there), margin 1e4. On the cavity (fixed walls) A's boundary values equal
the default's, so A's S reads ON's path with the variant's branch taken; at a fixed point `u^n = u^(n-1)` and the
extrapolation is the identity, so A and ON solve the same equations there by construction.

## 5. Check H (fixed now)

As ADR 0822 section 5 registered it (B2, H(dt) 4.605e-5 s 5 498 steps, H(dt/2) 2.3025e-5 s 10 996 steps, the window
above 0.1266 s, the six site cells, A_STEADY 0.193 m/s, the classes of ADR 0183 in their order, the verdict of F-879 with
persistence 0.75, no wall-clock cap, both records on, `n_outer` 5, the outer rule of E-1 of ADR 0767 as written, the
opt-ins `alpha_free_face_flux`, `outer_frozen_limiter` and `outer_frozen_convection_gradient`), with the corrected remedy's
field on in addition. The case, the measures, `class_0183`, `verdict`, `run_h`, the record form and C5 are copied from
`f885_order_case.rs` into the new file, `run_h` setting the variant; the records go to
`docs/investigations/f885-kept-gradient-lag/` (`H_dt.csv`, `H_dt2.csv`, their logs).

**Run only if the corrected remedy's order checks and S are met.** Both runs started together, in the background, no cap.
Estimate (ADR 0821 section 5): about 1.6 h and 3 h; `d_5 / d_1` about 3e-3, margin 3.3; STEADY. If check H is not run, SR
and G1 are marked `#[ignore = "not run: ..."]` in the commit of the results; that is fixed now, not an amendment.

## Table of checks

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
| --- | --- | --- | --- | --- | --- | --- |
| A3 | check H with the corrected remedy: H(dt) completes 5 498 steps and H(dt/2) 10 996, no failure, every field finite | both complete | acceptance of F-885; ADR 0183 D5; ADR 0767 A3 | completes | n/a | solution verification |
| A4 | each run TIME-ACCURATE by E-1 of ADR 0767 | at least 99 % of the window's steps with `d_5 <= 1e-2 d_1` | acceptance of F-885; ADR 0183, check H, the outer-iteration rule | `d_5 / d_1` about 3e-3 | 3.3 | solution verification |
| A5 | the class by ADR 0183's rule and the verdict of F-879 by ADR 0766's table | one of the four | acceptance of F-885; ADR 0183; ADR 0766 | STEADY | n/a | solution verification |
| A6 | a run that does not decide is recorded as not deciding | as stated | acceptance of F-885 (last clause) | as stated | n/a | solution verification |
| K64-OFF | N = 64, default step, setting off (control): `p_f` | `>= 1.9` | `docs/process/tolerances.md`, "Observed order of accuracy" | 2.03 | 0.13 in order | solution verification |
| K64-A | N = 64, the setting with A: `p_f` | `>= 1.9` | as K64-OFF | 2.0 | 0.1 in order | solution verification |
| K64-C | N = 64, the setting with C: `p_f` | `>= 1.9` | as K64-OFF | 2.0 | 0.1 in order | solution verification |
| K16-OFF | N = 16, `rhie_chow_step_consistent`, setting off (control): `p_f` | `>= 1.9` | as K64-OFF | 2.02 | 0.12 in order | solution verification |
| K16-A | N = 16, `rhie_chow_step_consistent`, the setting with A: `p_f` | `>= 1.9` | as K64-OFF | 2.0 | 0.1 in order | solution verification |
| K16-C | N = 16, `rhie_chow_step_consistent`, the setting with C: `p_f` | `>= 1.9` | as K64-OFF | 2.0 | 0.1 in order | solution verification |
| R1 | ON at N = 64 against ADR 0822's three ON errors | relative `<= 1e-4` each | "Same discrete problem by two routes", widened to the five printed digits (as ADR 0822 R0) | 0 | n/a | unchanged default output |
| RD | the arms ON, B, D on both meshes and the reading of section 2 | recorded; the reading by its rule | section 2 | A and C stand, B and D ruled out | n/a | solution verification |
| VF | N = 16, default step, `dt` 0.05: A against ON and C against ON | relative L2 `> 1e-9` each | ADR 0821 O3 (the setting must act on the case) | 1e-5 | 1e4 | code verification |
| S1 to S3 | section 4, for A and for C | as ADR 0821 S | "Same discrete problem by two routes"; ADR 0821 S | 3e-13 | 1e4 | code verification |
| SR | 20 steps of the check H path against the first 20 rows of `H_dt.csv` | relative `<= 1e-6`; finite; `d_1 > 0` | as ADR 0822 SR | 0 | n/a | code verification |
| C5 | the rule of E-1 on built records (ADR 0767 C5, copied) | each as built | ADR 0767 E-1 | as built | n/a | code verification |
| G1 | the class and verdict from the committed `H_dt.csv` and `H_dt2.csv` as recorded | counts exact; class and verdict equal; 1e-6 relative | as ADR 0822 G1 | 0 | n/a | unchanged default output |
| B3 | `cargo test -p rcfd-api --release --test baseline_hashes` passes, `baseline_hashes.rs` unchanged | passes, file unchanged | rule 3; the brief | passes | n/a | unchanged default output |
| D1 | under `crates/*/src` only `pimple.rs` changes, by the two fields and their reads; `docs/roadmap.json`, the ledger, every golden and every existing test file unchanged; the nine committed F-885 patches and the new one apply | as stated | the brief, owned files; rule 3 | as stated | n/a | unchanged default output |
| P | `cargo test -p rcfd-fv --release --lib` and `-p rcfd-api --release --lib`; the new file's gate tests; `--test f885_order_case`, `--test f885_frozen_convection_check_h` (the setting's tests); `--test baseline_hashes`; `rustfmt --check` of the touched files (for `cargo fmt -p`, which may refuse here with os error 206); `cargo clippy -p rcfd-fv -p rcfd-api --all-targets -- -D warnings`; `cargo test -p rcfd-cli --release --test docs_site`; `python scripts/roadmap_proposals.py check`; `python scripts/process_report.py --check` | clean; passes; no failure but a generated file not current | rule 7 | as stated | n/a | code verification |

No row is of kind validation: F-885 claims no agreement with a measured case. Margins of the order rows are thin; a miss
is recorded as a miss.

## Tests (fixed now)

All in `crates/rcfd-api/tests/f885_kept_gradient_lag.rs` (header: ADR 0824, F-885). Gate tests: `f885_kg_variants_act`
(VF), `f885_kg_order_16` (K16-OFF, K16-A; about a minute), `f885_kg_steady` (S for A and C),
`f885_kg_rules_on_built_records` (C5), `f885_kg_short_run_on_the_check_h_path` (SR),
`f885_kg_class_from_the_committed_records` (G1; `RECORD` written after the runs). Records, `#[ignore]`, run once by me:
`f885_kg_arms_16_record` (ON, B, C, D at N = 16 with `rhie_chow_step_consistent`; K16-C asserted, the reading printed),
`f885_kg_order_64_record` (OFF, ON, A, B, C, D at N = 64; R1, K64-OFF, K64-A, K64-C asserted after every arm is printed;
heavy, about 8 minutes), `f885_kg_h_dt_record`, `f885_kg_h_dt2_record` (check H, heavy, writing to `F885_KG_OUT`). The fast
tests on the heavy paths: `f885_kg_order_16` and VF for the order record, SR for check H.

## Negative controls and planned mutations

On a scratch copy of the full commit (`git archive` into the scratch folder, its own target folder), each against the
named test, which passes unmutated; the check H records are not rerun for a mutation.

| Id | Mutation | Must be caught by |
| --- | --- | --- |
| M1 | solver: `outer_frozen_convection_gradient_extrapolated_boundary` not read | `f885_kg_variants_act` (VF) |
| M2 | solver: `outer_frozen_convection_gradient_second` not read | `f885_kg_variants_act` (VF) |
| M3 | solver: A's extrapolated boundary values with half the extrapolation (`0.5 r` for `r`) | `f885_kg_order_16` (K16-A) |
| M4 | solver: with the setting, the kept gradient scaled by 0.99 where it is used | `f885_kg_steady` (S2) |
| M5 | test: the ratio of E-1 1e-1 for 1e-2 | `f885_kg_rules_on_built_records` (C5) |
| M6 | record: `H_dt.csv` cut to its first half | `f885_kg_class_from_the_committed_records` (G1); not run if check H is not run |

The committed patch (`crates/rcfd-api/tests/f885_kept_gradient_lag.mutation.patch`) is M1, first line
`# expect-fail: cargo test -p rcfd-api --release --test f885_kept_gradient_lag f885_kg_variants_act`. M3 is caught only if
half the lag is enough to bring K16-A below 1.9; if not, it is recorded as not caught.

## Decision rule

* **Done** for F-885 if the corrected remedy's order checks (K64-OFF, K16-OFF and K64 and K16 of the variant) and S1 to S3
  are met, VF, R1, A3 and A4 in both runs, A5 with the verdict steady, physically unsteady or numerical, SR, C5, G1, B3, D1
  and P are met, and each of M1 to M6 is caught.
* **Partial** otherwise, with the reason: no arm restores the order (then check H is not run); check H run and not
  deciding; a row or a mutation missed.
* **Stopped and reported** if check H cannot be run as registered on this base.

A defective set-up found before a registered run is corrected by an amendment committed before the corrected run.

## What is not covered

The variants as defaults or case keys; their order with the limiter, variable steps or a rotational seam (A reads the
translational and rotational coupled values alike through `boundary_values`, but only a translational box is checked); a
steady check that A changes (the cavity has fixed walls; section 4); arm 1, arm 2 and D9 of ADR 0183; any agreement with a
measurement.

## Open questions for the maintainer

To be completed in the addendum with options and a recommendation: any question of scope.

## Addendum 1: results

Written 2026-11-27, after the runs. Commits on the branch, from 689fab92: f8d9059d (this registration, alone), 4bb84d4f
(the two variants in `pimple.rs` and the test file; every registered run of the order, the steady check, C5 and check H was
made from its code), 0661b643 (the committed mutation M1, the two F-885 patches re-based, the order and mutation logs),
12f8c546 (the records of check H, `RECORD` written from them, the SR and G1 logs), and the commit of this addendum with the
rule 7 log and the proposal file. Records in `docs/investigations/f885-kept-gradient-lag/`: `order.log`, `mutations.log`,
`rebased_patches.log`, `H_dt.csv`, `H_dt.log`, `H_dt2.csv`, `H_dt2.log`, `sr.log`, `g1.log`, `rule7.log`.

### The sentence

**The remedy's lag was the kept gradient's boundary values: on the first outer iteration of a BDF2 step they are of `u^n`
while its cells are extrapolated, and the setting kept that mix to the end of the step. Formed from the extrapolated field
(variant A), the remedy has BDF2's order on both meshes, the steady answer is unchanged, and check H with it classes the hub
pocket STEADY in both runs.** `p_f` with A is 2.0293 at N = 64 (the setting as it was 1.8222, off 2.0345) and 2.0105 at
N = 16 with `rhie_chow_step_consistent` (as it was 1.1262, off 2.0219); `e_A / e_OFF` is constant in `dt` (1.336, 1.341,
1.346). Keeping the gradient from the second outer iteration (C) also restores the order. By the registered rule F-885 is
**done**.

### Table of checks

| Id | Limit | Measured | Met |
| --- | --- | --- | --- |
| K64-OFF | `p_f >= 1.9` | 2.0345 (`e` 2.7615e-4, 6.7885e-5, 1.6570e-5; `p_c` 2.0243) | yes |
| K64-A | `p_f >= 1.9` | 2.0293 (`e` 3.6899e-4, 9.1059e-5, 2.2307e-5; `p_c` 2.0187) | yes |
| K64-C | `p_f >= 1.9` | 2.0377 (`e` 2.7845e-4, 6.8127e-5, 1.6593e-5; `p_c` 2.0311) | yes |
| K16-OFF | `p_f >= 1.9` | 2.0219 (`e` 3.8292e-4, 9.3308e-5, 2.2976e-5; `p_c` 2.0370) | yes |
| K16-A | `p_f >= 1.9` | 2.0105 (`e` 7.4350e-4, 1.8368e-4, 4.5587e-5; `p_c` 2.0171) | yes |
| K16-C | `p_f >= 1.9` | 2.0282 (`e` 3.8988e-4, 9.4145e-5, 2.3081e-5; `p_c` 2.0501) | yes |
| R1 | relative `<= 1e-4` each | 1.16e-5, 1.04e-6, 1.42e-5 | yes |
| RD | recorded; the reading by its rule | A and C stand, B ruled out, D not ruled out (below) | recorded |
| VF | `> 1e-9` each | A against ON 4.0386e-4, C against ON 4.1045e-4 | yes |
| S1 to S3 (A) | as ADR 0821 S | OFF 211 steps, ON with A 210; velocity 3.3828e-13, flux 3.3426e-13; after 5 steps 8.3136e-3 | yes |
| S1 to S3 (C) | as ADR 0821 S | ON with C 213 steps; velocity 7.8456e-13, flux 7.6156e-13; after 5 steps 5.2638e-3 | yes |
| A3 | both complete | H(dt) 5 498 of 5 498 steps, H(dt/2) 10 996 of 10 996, no failure, fields finite, mass balance 8.9e-16 and 7.4e-16 | yes |
| A4 | at least 99 % with `d_5 <= 1e-2 d_1` | H(dt) 1.000000 (`d_5 / d_1` median 1.26e-3, max 3.01e-3); H(dt/2) 1.000000 (median 2.39e-4, max 2.30e-3) | yes |
| A5 | one of the four | class STEADY, verdict Steady: site amplitude 1.2557e-3 m/s (H(dt)) and 2.2775e-3 m/s (H(dt/2)), both `<= 0.193` | yes |
| A6 | as stated | both runs decide | yes |
| SR | relative `<= 1e-6`; finite; `d_1 > 0` | 4.603e-10 | yes |
| C5 | each as built | each of the seven as built | yes |
| G1 | as registered | passed: counts 5 498 and 10 996, windows 2 749 and 5 498, class and verdict Steady equal to `RECORD` | yes |
| B3 | passes, file unchanged | `baseline_hashes`: 3 passed, 1 ignored; `baseline_hashes.rs` unchanged since 689fab92 | yes |
| D1 | as stated | under `crates/*/src` only `pimple.rs` (the two fields, their defaults and reads); no roadmap, ledger, golden or existing test file; the patches of `f885_frozen_convection_check_h` and `f885_order_case` re-based (below); all ten F-885 patches apply | yes |
| P | rule 7 | see "The checks of rule 7" | yes |

Beside, without a limit: `e / e_OFF` at N = 64: ON 1.360, 1.430, 1.657; A 1.336, 1.341, 1.346; C 1.008, 1.004, 1.001. At
N = 16 with `rhie_chow_step_consistent`: ON 2.80, 4.69, 8.73; A 1.94, 1.97, 1.98; C 1.018, 1.009, 1.005. Check H, beside:
the largest seam-pair flux sum 2.3e-15 in both runs; shaft power 313.56 W and 313.96 W; the five cells of largest
amplitude over the window lie at the tip (r 0.234 to 0.247 m), 1.54 m/s at most in H(dt) and 0.126 m/s in H(dt/2). Wall
time: the N = 64 record 710 s, the N = 16 arms 99 s, H(dt) 106 min, H(dt/2) 165 min.

### The reading (section 2)

| Arm | `p_f` at N = 64 | `p_f` at N = 16, `rhie_chow_step_consistent` | Reading |
| --- | --- | --- | --- |
| ON | 1.8222 | 1.1262 | |
| A | 2.0293 (`p_f(A) - p_f(ON)` 0.2071) | 2.0105 | stands |
| B | 1.8222 (equal to ON to the printed digits) | 1.1262 (equal) | ruled out |
| C | 2.0377 | 2.0282 | stands |
| D | 1.0921 (`abs(p_f(D) - p_f(ON))` 0.7301) | 1.0398 (0.0863) | not ruled out |

A and C stand by their rules: the lag is the mix of time levels in the kept gradient, removed by forming the first
iteration's gradient wholly from the extrapolated field (A) or by keeping a gradient of one iterate (C). B is the
convection's partner value, which the later iterations assemble anew, as predicted. D is not ruled out by its rule, but it
does not bear on A: the exact start of ADR 0822 is itself first order without the setting (its D-s, 1.064 at N = 16), its
older face flux being the face value of the exact field rather than a flux of the solver's own step, so the arm adds a
start defect of its own; with A its order was not run. Not diagnosed further; the registered checks of A do not use it.

### Mutations

From `mutations.log` (`git archive` of 4bb84d4f in the scratch folder, its own target folder) and, for M6, an archive of
12f8c546:

| Id | What happened |
| --- | --- |
| M1 | A not read: caught by `f885_kg_variants_act` (A against ON 0). Committed as the mutation patch |
| M2 | C not read: caught by `f885_kg_variants_act` (C against ON 0) |
| M3 | A's extrapolation halved: caught by `f885_kg_order_16` (K16-A 1.3309) |
| M4 | the kept gradient scaled by 0.99: caught by `f885_kg_steady` (S2 for A 2.95e-3) |
| M5 | the ratio of E-1 1e-1: caught by `f885_kg_rules_on_built_records` ("d5/d1 = 0.05 everywhere" got Steady) |
| M6 | `H_dt.csv` cut to 2 749 rows: caught by `f885_kg_class_from_the_committed_records` (row count 2 749 against 5 498) |

Six of six caught. The two earlier F-885 patches whose lines this work moved were re-based with the mutation unchanged
(`f885_frozen_convection_check_h`: the kept gradient scaled by 0.99; `f885_order_case`: the setting not read); on a scratch
copy of 4bb84d4f each still fails its test (`f885_fc_steady`, `f885_oc_setting_acts`; `rebased_patches.log`).

### Deviations

1. Check H's two runs were not started together. The first launch started H(dt) at 02:53 UTC, but by a defect of the
   command line H(dt/2)'s output went to a path the system refused, so no process started; its second launch failed at the
   link (the test binary was in use by H(dt) and cargo rebuilt it after the docs commit 0661b643, which the crates read at
   compile time) before any step. H(dt/2) was then run once, from a copy of the same binary of 4bb84d4f, from 02:58:58 UTC.
   Each run of check H was made once; no step of either was repeated.
2. M6's first attempt read a stale archive of 4bb84d4f (no `H_dt.csv`), so the mutation was not applied; it is void, and
   M6 was then run once on the archive of 12f8c546.
3. G1's log keeps only the result line (the log filter of the run script left out its print line); the pass asserts the
   class and verdict equal to `RECORD`, which was written from the two runs' printed summaries.
4. The N = 16 arms record prints OFF and A beside the registered run of `f885_kg_order_16`, as registered; the same numbers
   to every printed digit.
5. The lib tests, the named test files and `baseline_hashes` ran at 0661b643 (solver and test code of 4bb84d4f); the later
   commits change the test file's `RECORD` only, which G1, fmt and clippy read.
6. `rustfmt --check` of the touched files failed at 12f8c546 on line wrapping (two calls in the test file, one condition
   in `pimple.rs`); `rustfmt` was applied, a layout change only (no token changed), and the committed M1 patch was made
   again on the new layout with the same mutation. `cargo fmt -p rcfd-fv -p rcfd-api --check` printed its help; run per
   crate it gave exit 0. No registered run was repeated for the layout change.

### What was not run

A steady check that A changes (registered as not covered); the order of A with the limiter, variable steps or a rotational
seam; the whole workspace suite.

### The checks of rule 7

See `rule7.log` (copied into this commit): the five test runs of `rule7a` at 0661b643 (`rcfd-fv --lib` 115 passed,
6 ignored; `rcfd-api --lib` 160 passed, 4 ignored; `--test f885_order_case` 3 passed, 6 ignored; `--test
f885_frozen_convection_check_h` 3 passed, 6 ignored; `--test baseline_hashes` 3 passed, 1 ignored); `cargo clippy -p
rcfd-fv -p rcfd-api --all-targets -- -D warnings` clean; after deviation 6 `rustfmt --check --edition 2021` of the touched
files exit 0 and `cargo fmt -p rcfd-fv --check`, `cargo fmt -p rcfd-api --check` exit 0; the ten F-885 patches apply.
`docs_site`, `roadmap_proposals.py check` and `process_report.py --check` run at the commit of this addendum, as reported
to the lead.

### Size

Size (agent): done; 0; 3; 0; 175000

### Decision by the registered rule

**Done.** The corrected remedy is A (section 3): K64-OFF, K16-OFF, K64-A and K16-A are met, and S1 to S3; VF, R1, A3 and
A4 in both runs, A5 with the verdict Steady, SR, C5, G1, B3, D1 and P are met, and each of M1 to M6 is caught. F-885's
acceptance: check H of ADR 0183 was run as registered there and in ADR 0766 with the outer-iteration rule of ADR 0183 as
written (100 % of the window's steps of both runs), and it classes the hub pocket as steady, with the opt-ins
`alpha_free_face_flux`, `outer_frozen_limiter`, `outer_frozen_convection_gradient` and
`outer_frozen_convection_gradient_extrapolated_boundary`.

## Open questions for the maintainer

1. **The variant A and the setting.** Options: (a) keep A a separate opt-in beside `outer_frozen_convection_gradient`, as
   now, the setting's bytes unchanged and every earlier record valid; (b) make A the behaviour of the setting itself, which
   changes the bytes of every run with the setting (ADRs 0820 to 0822's records) though no default. I recommend (a): it is
   the registered state, and a run that wants the order sets both.
2. **Scope.** The class STEADY holds with four opt-ins of the pimple step; whether the hub pocket is steady with the
   default step is not what F-885 asks. No new item proposed; a question of the maintainer's scope only.

## For the lead: roadmap entries

> Status proposed for F-885: done.

> Note entry: ADR 0824, 2026-11-27, done: the remedy's lag was the kept convection gradient's boundary values (of u^n while its cells are extrapolated on BDF2's first outer iteration); formed from the extrapolated field (outer_frozen_convection_gradient_extrapolated_boundary) the remedy has BDF2's order (p_f 2.029 at N = 64, 2.011 at N = 16 with rhie_chow_step_consistent; the setting alone 1.822 and 1.126), the steady answer is unchanged, and check H with it is TIME-ACCURATE in 100 % of both windows and classes the hub pocket STEADY; held by crates/rcfd-api/tests/f885_kept_gradient_lag.rs and docs/investigations/f885-kept-gradient-lag.

The same entries are in `docs/roadmap/proposals/0824-F-885.json` (form 1 of ADR 0755, base 689fab92), checked with
`python scripts/roadmap_proposals.py check`. No gap (done) and no follow-up item (question 2).

Generated pages that this work may leave not current, for the lead (rule 9): `docs/adr/index.md` (ADR 0824 is new).
