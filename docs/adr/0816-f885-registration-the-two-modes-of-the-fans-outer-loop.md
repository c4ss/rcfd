# ADR 0816: F-885 registration: the two modes of the fan's outer loop, a constant change at c11837 and a slow contraction at c6245

Status: **REGISTERED 2026-11-26, written before any code of F-885 for this registration and before any run of its checks.**
Before this text no library code, test, harness or solver run of this work was written or made. What was done: reading
(below), `git apply --check` of the four committed F-885 mutation patches at the base (all four apply), and the arithmetic
of this page. Results go in addenda at the end; nothing above them is changed after a run.

It answers the brief [`docs/adr/briefs/f885-two-outer-modes.md`](briefs/f885-two-outer-modes.md) and follows
[`docs/process/agent-rules.md`](../process/agent-rules.md), version 5. A further registration of F-885 under rule 13 (a
diagnosis of the miss of check H in [ADR 0811](0811-f885-registration-a-face-flux-independent-of-alpha-u-then-check-h.md)),
taken as option (a) of question 1 of [ADR 0815](0815-f885-registration-the-fans-outer-stall-by-controls.md) by the lead's
decision of 2026-11-26. Branch `f885-two-outer-modes` from main d8b506d8; scratch `w49i-work/f885-two-outer-modes-scratch`.
Written by the model Claude Opus 5.5.

Size (agent): done; 1; 1; 1; 140000

## What is asked

ADR 0815 ruled out five hypotheses for the stall of the fan's relaxed outer loop (20 steps of H(dt), 20 outer iterations,
the switch of ADR 0811 on): every control stalls, `C` 0.017 to 0.033 against 1e-2. Its record shows two modes. With the
limiter (G0) the largest change sits at the interior cell c11837 (r 0.2492 m, z 0.2306 m) and is the same in each outer
iteration (step 20: `d_10` to `d_19` 5.56e-3 to 5.59e-3 m/s). Without it (GL) the loop contracts at c6245 (r 0.2346 m,
z 0.1485 m) by about 0.85 an iteration (step 20: `d_17` to `d_19` 5.87e-3, 4.98e-3, 4.22e-3 m/s). The brief asks, for each
mode, for hypotheses with the number that decides each, fixed now, from opt-in records, no long run, the cavity as a control.

## What was read

Whole: `AGENTS.md`; `docs/process/agent-rules.md` (version 5); the brief; ADR 0815 (registration, addendum 1, answers);
`crates/rcfd-api/tests/f885_outer_stall.rs`.

In part: ADR 0811 section 1 (lines 85 to 100: the fixed point of the relaxed iteration and its condition `d = a_c`);
`crates/rcfd-fv/src/pimple.rs` (base d8b506d8) lines 40 to 300 (settings, records, `max_velocity_change`), 1055 to 1147
(`piso`, `pimple` defaults), 1236 to 1340 (`OuterUpdate`, the hook, the solver's records), 1410 to 1465 (the two
constructors' record fields), 1783 to 2140 (`gamma_nu`, the outer loop, the hook block with the freezes of ADR 0815, the
momentum predictor: least-squares gradient, Barth-Jespersen factor, `convection`, `laplacian_opt`, `add_ddt`, the
relaxation), 2575 to 2595 (the record of `d_k`), 2655 to 2700 (the end of the step), 3100 to 3125 and 3255 to 3275 (the
relaxation with periodic and rotational links); `crates/rcfd-linalg/src/matrix.rs` lines 60 to 95 (`off_diag_abs_sum`,
`relax`: `d = max(diag, sum |a_nb|)`, the diagonal `d / alpha`); `crates/rcfd-fv/src/gradient.rs` lines 330 to 400
(`limiter`, Barth-Jespersen: per face `min(1, (max - u_c)/delta)` or `min(1, (min - u_c)/delta)`, the minimum over the
faces); `crates/rcfd-fv/src/ddt.rs` lines 24 to 90; `crates/rcfd-fv/src/convection.rs` lines 170 to 180;
`crates/rcfd-fv/src/diffusion.rs` lines 24, 83 to 95, 278; `docs/process/tolerances.md`, the table.

No published source outside the repository was read. The hypotheses and the predictions below come from the code and from
first principles (the algebra of the implicit relaxation, stated in section 2); nothing rests on a published result.

## 1. The relaxation at one cell (first principles, from `matrix.rs` 86 to 95)

For component `j` of cell `c` the predictor's row is `a_c u_c - sum a_nb u_nb = b`. The relaxation sets
`d = max(a_c, s)` with `s = sum |a_nb|` (with the link magnitudes where links exist), puts `d / alpha` on the diagonal and
adds `(d / alpha - a_c) u_c^old` to `b`. At a fixed point the added terms cancel, for any `d`. Away from it, with the
neighbours and `b` held, the cell's error is multiplied by

    lambda_cell = 1 - alpha a_c / d,

which is `1 - alpha` (0.1 at `alpha` 0.9) where the diagonal dominates (`d = a_c`) and `1 - alpha a_c / s` where it is lost
(`s > a_c`). A whole loop whose unrelaxed outer map has the eigenvalue `lambda_0` on a mode (where the diagonal dominates)
is, to first order, the blend `lambda(alpha) = 1 - alpha (1 - lambda_0)`.

## 2. The hypotheses and the numbers that decide them (fixed now)

### Mode A, the constant change at c11837 (run A0: G0 of ADR 0815 with the probe)

| Id | Hypothesis | The number that decides it, per classed step | Holds if |
| --- | --- | --- | --- |
| HA1 | a Barth-Jespersen factor switches between two values: of the cell or a face neighbour, in one component | over the entries `e` (the cell and its face neighbours, each active component), `delta_e,k = |psi_k - psi_(k-1)|` for `k = 3..19`; `e*` the entry of the largest median `delta`; the step switches if median `delta_e*` `>= 1e-9` and median over `k = 4..19` of `|psi_k - psi_(k-2)|` `<= 0.1` median `delta_e*` | at least 80 % of the classed steps switch |
| HA2 | the diagonal dominance is lost at the cell and its own factor keeps the change | per component `r_j = a_c / s` in each relaxed iteration `k = 2..19`; the step counts if `s > a_c` for some active component in every one of them and `1 - alpha_u min_j median_k r_j >= 0.99` | at least 80 % of the classed steps count |
| HA3 | a face flux of the cell changes sign between iterations (the upwind direction switches) | per face of the cell the number of sign changes of its outward convecting flux between iterations `k - 1` and `k`, `k = 4..19` (16 pairs); the step counts if some face has at least 8 | at least 80 % of the classed steps count |

The numbers. 1e-9: a floor above the rounding of a factor in `[0, 1]` (1e-16, times the conditioning of a ratio of
differences), so a factor that is pinned (at 1, or at a fixed face) does not count as switching. 0.1: a switch between two
values returns to the value it left (`psi_k = psi_(k-2)`), a monotone drift has `|psi_k - psi_(k-2)| = 2 delta`; 0.1 asks
for a return within a tenth of the change. 0.99: the record of G0 step 20 has `d_10` to `d_19` within 5.56e-3 to 5.59e-3,
a factor over nine iterations of at least `(5.56 / 5.59)^(1/9)` = 0.9994; a cell factor below 0.99 would shrink the change
by `0.99^9` = 0.91 over those iterations, which the record excludes. 8 of 16: a direction that switches in half the
iteration pairs; a single crossing would not repeat. 80 %: 8 of the 10 classed steps (of 3 in the cavity: all three).

Beside, without a limit: the shape of the mode at the cell, `q` = median over the classed steps of the median over
`k = 3..19` of `|u_k - u_(k-2)| / |u_k - u_(k-1)|` (near 0 a two-state cycle of the iterate, near 2 a drift), and the
recorded `a_c`, `s`, factors and fluxes.

**Result A.** `S_A` the set of HA1 to HA3 that hold. One member: **names** it. Empty: **rules all out**. Two or three:
**names none** (with the set). A0 not complete: **undecided**.

### Mode B, the slow contraction at c6245 (runs B1 to B4: GL of ADR 0815 with the probe)

| Run | `alpha_u` | correctors | otherwise |
| --- | --- | --- | --- |
| B1 | 0.9 | 2 | GL of ADR 0815 (no limiter), the probe |
| B2 | 0.7 | 2 | as B1 |
| B3 | 0.5 | 2 | as B1 |
| B4 | 0.9 | 4 | as B1 |

The factor of a run, `rho` = median over the classed steps of `(d_19 / d_16)^(1/3)`, from the step's largest changes
`d_k` (the measure of ADR 0815's observation). The dominance ratio of a run, `r` = median over the classed steps and
`k = 16..19` of `min_j a_c / s` at c6245. Estimate from ADR 0815 GL: `rho(B1)` 0.85; then `lambda_0 = 1 - (1 - 0.85)/0.9`
= 0.833.

| Id | Hypothesis | Predicted factor | Holds if |
| --- | --- | --- | --- |
| HB1 | the diagonal dominance is lost at c6245 and its own relaxation factor sets the contraction | `rho(alpha) = 1 - alpha r` (each run's own `r`) | `r(B1) < 1`, and `|rho - (1 - alpha r)| <= 0.015` for B1, B2 and B3 |
| HB2 | the pressure-velocity coupling within an outer iteration (the corrector count) sets it | `rho(B4) < rho(B1)` | `rho(B4) <= rho(B1) - 0.015` |
| HB3 | a slow eigenvalue `lambda_0` of the unrelaxed outer map, which the relaxation only blends | `rho(alpha) = 1 - alpha (1 - lambda_0)`, `lambda_0 = 1 - (1 - rho(B1)) / 0.9`: with 0.85 at B1, 0.883 at B2, 0.917 at B3; `rho(B4) = rho(B1)` | `|rho(B2) - pred| <= 0.015`, `|rho(B3) - pred| <= 0.015`, `|rho(B4) - rho(B1)| <= 0.015` |
| HB4 | a mode the relaxation does not set (the factor does not depend on `alpha_u`) | `rho(B2) = rho(B3) = rho(B1)` | `|rho(B2) - rho(B1)| <= 0.015` and `|rho(B3) - rho(B1)| <= 0.015` |

The band 0.015: less than half the smallest separation of two predictions of different hypotheses at the registered
`alpha_u` (HB3 against HB4 at B2: 0.883 - 0.85 = 0.033), so one measured factor cannot lie in the bands of both; the spread
of the factor in ADR 0815 (GL step 20: 0.848, 0.847) is a tenth of it.

**Result B.** `S_B` the set of HB1 to HB4 that hold. `{HB1, HB3}`: **names HB1** (the recorded dominance then explains the
blend). One member: **names** it. Empty: **rules all out**. Otherwise: **names none** (with the set). A run of B1 to B4 not
complete, or a factor NaN: **undecided**.

### The cavity (a control)

`box_2d(8, 8, 1, 1)`, lid `(1, 0, 0)`, `nu = 0.01`, `PimpleSettings::pimple(0.01, 20)`, BDF2, `dt = 0.05`, from rest, 3
steps, the record and the switch on, `convection = LinearUpwind`, `linear_upwind_limiter = BarthJespersen`, the probe at
cells 27 and 36 (interior). KL: `C` (median `d_19 / d_2` over steps 1 to 3) and the rule of mode A at cell 27, all three
steps classed.

## 3. The instrumentation (fixed now)

One setting and one record in `crates/rcfd-fv/src/pimple.rs` only (the limiter's file `gradient.rs` is not changed):

* `PimpleSettings::outer_cell_probe: Vec<usize>`, default empty. With cells in it, `step_with` keeps for each of them and
  each outer iteration: the momentum diagonal `a_c` of each component before the relaxation (after the time term), the sum
  `s` the relaxation takes (`off_diag_abs_sum` plus the magnitudes of the periodic links and, with
  `rotational_periodic_implicit`, of the rotational links, as in the three `relax` functions), the Barth-Jespersen factor of
  each component at the cell and at its face neighbours (NaN where no limiter is formed or the component is inactive; NaN
  for `a_c` and `s` of an inactive component), the convecting flux `phi_conv` on each face of the cell, signed outward,
  and the cell's velocity after the iteration's correctors.
* `PimpleSolver::outer_probe: Vec<OuterProbe>`: per completed step and probe cell, the step, the cell, its faces (in face
  order), its face neighbours (the other cell of each internal face, in that order) and the records of the iterations.

Read-only: the copies enter no equation; with the probe empty nothing is computed or kept (bit-identical;
`baseline_hashes` unchanged). Not read by the fractional step or the moving-mesh steps. No case key, schema, golden or hash
table. The runs set it on the built solver as ADR 0815 set its controls.

## 4. The runs (fixed now)

**Fan**: the harness of ADR 0815 (`h_text(4.605e-5)` copied unchanged, `alpha_free_face_flux = true`, `n_outer = 20`, the
record on, `outer_pressure_tolerance = Some(-1.0)`), the probe at c11837 and c6245, 20 steps from the uniform start; A0
(G0), then B1 to B4; one heavy test, each run once, the record of each run written as it ends. Time estimate: ADR 0815's
runs took 67 s to 99 s; B4 more (four correctors); about 8 min in one background process; no cap.

**Records**: `docs/investigations/f885-two-outer-modes/`: `fan_d.csv` (run, step, `k`, `d_k`, cell; all steps),
`probe.csv` (one row per run, classed step, probe cell and iteration: `a_c`, `s` and `u` per component, the factors and the
fluxes as `;`-joined lists), the run log and the mutation log. The outcomes are written into the test file after the
registered run (`FAN_RECORD`), as ADR 0815 did; the form and the tolerance are fixed here.

## Table of checks

There is no clause of the acceptance of F-885 that this work can meet (check H is not run); the brief's task is the
diagnosis and its decision rule.

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
| --- | --- | --- | --- | --- | --- | --- |
| B1 | every run of section 4 made as registered: A0, B1 to B4 complete 20 steps or stop with the stop recorded; KL completes 3 steps | as stated | the brief, decision rule | all run | n/a | solution verification |
| B2 | the result A and the result B each name a hypothesis or rule all out (section 2) | as stated | the brief, decision rule | A: HA1 or HA2; B: HB3 | n/a | solution verification |
| B3 | no solver default changes: `cargo test -p rcfd-api --release --test baseline_hashes` passes, `baseline_hashes.rs` unchanged | passes, file unchanged | the brief; rule 3 | passes | n/a | unchanged default output |
| R0 | A0 against G0 and B1 against GL of ADR 0815 (`docs/investigations/f885-outer-stall/fan_controls.csv`): every `d_k` of 20 steps | relative `<= 1e-6` | `docs/process/tolerances.md`, "Same discrete problem by two routes", widened to the 10 printed digits as ADR 0815 R0 | 0 | n/a | unchanged default output |
| R1 | the probe against the record: in A0's classed steps, where the record's cell of `d_k` is c11837 (`k >= 2`), `|u_k - u_(k-1)|` from the probe against `d_k` | relative `<= 1e-6` | "Same discrete problem by two routes", widened to the printed digits | 0 | n/a | code verification |
| T1 | two routes, the cavity (implicit Euler, linear upwind with Barth-Jespersen, `alpha_u` 0.8, 5 outer iterations, 3 steps, probe at cell 27): in the first outer iteration of each step, `a_c`, `s`, the factors of the cell and its neighbours and the outward fluxes against the same quantities assembled in the test from the state before the step with the crate's public functions (`least_squares`, `limiter`, `convection`, `laplacian_opt`, `V / dt`); the probe's velocity of the last iteration of each step against the solver's velocity; NaN for the inactive component | relative `<= 1e-12` each; NaN where stated | "Same discrete problem by two routes", `<= 1e-12` where only rounding separates | 0 | n/a | code verification |
| T2 | the probe read-only: the cavity of K1 of ADR 0815 (KC0) and the cavity KL, each with the probe at 27 and 36 against without: `u`, `p`, flux and the record of `d_k` | equal bits | "Bit-identity of default output" | equal | n/a | unchanged default output |
| T3 | the rules of section 2 on built records: a factor that alternates, one that drifts, one pinned; a lost and a kept dominance; a flux that switches and one that does not; built factors for each of HB1 to HB4 and their combinations; a run not complete | each as built | this registration, section 2 | as built | n/a | code verification |
| K2 | KL: `C` | `<= 1e-2` (CONVERGES) | ADR 0810 section 5 (ADR 0183's outer ratio, as ADR 0767 E-1 states it) | 1e-6 | 1e4 | code verification |
| K3 | KL: the rule of mode A at cell 27 | rules all out | this registration, section 2 (a loop that contracts carries no mode) | all ruled out | n/a | code verification |
| G2 | the outcomes from the committed records: A0's three hypotheses and `q`, B1 to B4's `rho` and `r`, the two results | equal; numbers within 1e-6 relative | "Same discrete problem by two routes", widened to the printed digits | 0 | n/a | unchanged default output |
| D1 | under `crates/*/src` only `crates/rcfd-fv/src/pimple.rs` changes; `docs/roadmap.json`, the ledger, every golden and every existing test file unchanged; the four committed F-885 mutation patches still apply (`git apply --check`), re-based in this branch with the mutation unchanged if not | as stated | the brief, owned files; rule 3 | as stated | n/a | unchanged default output |
| P | `cargo test -p rcfd-fv --release --lib`; the new test file; the existing tests of the changed step `f885_outer_stall`, `f885_alpha_free_face_flux`, `f885_outer_fixed_points`, `f885_check_h_outer_record`; `cargo fmt -p rcfd-fv -p rcfd-api --check`; `cargo clippy -p rcfd-fv -p rcfd-api --all-targets -- -D warnings`; `cargo test -p rcfd-cli --release --test docs_site`; `python scripts/roadmap_proposals.py check`; `python scripts/process_report.py --check` | clean; passes; no failure but a generated file not current | rule 7 | as stated | n/a | code verification |

The numbers of section 2 class a run; they are not pass or fail limits of a fan run. K2 and K3 are a control: a miss is
recorded and read in the addendum; it does not enter the decision rule. No row is of kind validation.

## Tests (fixed now)

All in `crates/rcfd-api/tests/f885_two_outer_modes.rs` (header: ADR 0816, F-885).

* Run by the gate: `f885_tm_probe_two_routes` (T1); `f885_tm_probe_read_only` (T2); `f885_tm_cavity_control` (K2, K3);
  `f885_tm_rules_on_built_records` (T3); `f885_tm_fan_from_the_committed_record` (G2, R0, R1 from the committed records).
* Heavy record (`#[ignore]`, run once by me): `f885_tm_fan_record`, A0 and B1 to B4, writing to the folder named by
  `F885_TM_OUT`; the fast tests on the same code path are T1 and T2 (the probe) and ADR 0811's
  `f885_af_short_run_on_the_check_h_path` (the fan harness).

## Negative controls and planned mutations

On a scratch copy of the branch (`git archive` into the scratch folder, its own target folder, every file touched after
extraction), each against the gate tests of the new file; the heavy record is not rerun for a mutation.

| Id | Mutation | Must be caught by |
| --- | --- | --- |
| N1 | solver: the probe keeps the diagonal after the relaxation (`d / alpha`) | `f885_tm_probe_two_routes` (T1) |
| N2 | solver: the probe's `s` without the upper coefficients (`lower` only) | `f885_tm_probe_two_routes` (T1) |
| N3 | solver: the probe keeps the factor of component 0 for every component | `f885_tm_probe_two_routes` (T1) |
| N4 | solver: the fluxes kept with the face's sign, not outward | `f885_tm_probe_two_routes` (T1) |
| N5 | solver: the probe's velocity taken before the correctors | `f885_tm_probe_two_routes` (T1) |
| N6 | solver: with the probe on, the momentum solve starts from zero | `f885_tm_probe_read_only` (T2) |
| N7 | test: the band of mode B 0.15 for 0.015 | `f885_tm_rules_on_built_records` (T3) |
| N8 | record: `probe.csv` cut to its first half | `f885_tm_fan_from_the_committed_record` (G2) |

The committed patch (`crates/rcfd-api/tests/f885_two_outer_modes.mutation.patch`) is N1, with the first line
`# expect-fail: cargo test -p rcfd-api --release --test f885_two_outer_modes f885_tm_probe_two_routes`.

## Decision rule

* **Done** for this registration if every run was made as registered (B1) and the result of each mode names a hypothesis
  or rules all out (B2), with B3, R0, R1, T1 to T3, G2 and D1 met.
* **Partial** if a run was not made as registered, or a result names none or is undecided, with the reason and what it
  would take.
* **Stopped and reported** if the probe cannot be made read-only with the defaults bit-identical, if A0 or B1 does not
  reproduce ADR 0815 (R0), or if anything else in the solver would need changing.

F-885 itself stays partial whatever this gives, until check H decides (the brief). A defective set-up found before a
registered run is corrected by an amendment committed before the corrected run.

## What is not covered

* Any remedy (a registration of its own); any change of a default; check H, H(dt/2), a long run.
* A hypothesis for either mode other than the seven above; the cells other than c11837 and c6245; the window state of
  check H.
* Mode A with the corrector or `alpha_u` grid (the grid is of mode B, as the brief asks).

## Open questions for the maintainer

To be completed in the addendum with options and a recommendation: the remedy, and any question of scope.

## Addendum 1: results

Written 2026-11-26, after the runs. Commits on the branch, from main d8b506d8: 3535252e (this registration, alone),
44a2e51b (the probe in `crates/rcfd-fv/src/pimple.rs` and the test file; every registered run was made from it), 9bbfee9e
(the fan record, the logs, the recorded outcomes `FAN_RECORD`), 0238503c (the committed mutation N1, the re-based patch
of `f885_check_h_outer_record`, rustfmt's layout of `FAN_RECORD`), and the commit of this addendum with the mutation log
and the proposal file. Records in `docs/investigations/f885-two-outer-modes/`: `fan_d.csv`, `probe.csv`, `fan.log`,
`cavity.log`, `mutations.log`.

### The sentence

**Mode A: the record names HA1.** The constant change at c11837 is a two-state cycle carried by the cell's own
Barth-Jespersen factor: in every outer iteration of all ten classed steps the factor of the x component switches between
two values (step 12: 1.0 and 0.60), and the iterate switches with it (`q` 0.005, `u_k` returns to `u_(k-2)`). The diagonal
dominates at the cell and no face flux changes sign. **Mode B: the record rules all four out.** The slow contraction
without the limiter (0.854 an iteration) is not a lost dominance, not the corrector count, not a blend of the relaxation
with a slow unrelaxed map, and not independent of `alpha_u`.

### Mode A (A0: G0 of ADR 0815 with the probe; 20 of 20 steps, 65.4 s)

| Hypothesis | Number per classed step (steps 11 to 20) | Steps that count | Holds |
| --- | --- | --- | --- |
| HA1 | the entry of the largest change is the cell's own factor, x component, in all ten; median change 0.219 to 0.562; median `abs(psi_k - psi_(k-2))` 3.0e-5 to 2.1e-2, below 0.1 of the change in each | 10 of 10 | yes |
| HA2 | `a_c / s` 1.527 to 1.528 (the diagonal dominates; `d = a_c`) | 0 of 10 | no |
| HA3 | largest number of sign changes of a face's outward flux: 0 | 0 of 10 | no |

`S_A = {HA1}`: **named HA1**. Beside: `q` 0.005037. The cell has six faces; two carry fluxes of 7.8e-4 m3/s (in and out),
four carry 1e-7 to 1.5e-6 m3/s, none of which changes sign. The factor of the z component is also below 1 (0.19 to 0.53)
but changes little within a step; the y component stays at 1.

### Mode B (B1 to B4: GL of ADR 0815 with the probe; each 20 of 20 steps)

| Run | `alpha_u` | correctors | `rho` | `r` at c6245 | s |
| --- | --- | --- | --- | --- | --- |
| B1 | 0.9 | 2 | 0.853789 | 1.38054 | 64.2 |
| B2 | 0.7 | 2 | 0.872992 | 1.38052 | 62.4 |
| B3 | 0.5 | 2 | 0.892044 | 1.38048 | 66.7 |
| B4 | 0.9 | 4 | 0.853740 | 1.38054 | 120.8 |

| Hypothesis | Prediction | Measured against it | Holds |
| --- | --- | --- | --- |
| HB1 | needs `r < 1` | `r` 1.38 (dominance kept) | no |
| HB2 | `rho(B4) <= rho(B1) - 0.015` | `rho(B4) - rho(B1)` = -0.00005 | no |
| HB3 | `lambda_0` 0.83754: 0.88628 (B2), 0.91877 (B3), `rho(B4) = rho(B1)` | B2 off by 0.0133 (in the band), B3 off by 0.0267 (outside), B4 off by 0.00005 | no |
| HB4 | `rho(B2) = rho(B3) = rho(B1)` | off by 0.0192 and 0.0383 | no |

`S_B` empty: **all ruled out**. Beside, without a limit: the factor depends on `alpha_u` with a slope of -0.0956 (B1 to
B3 lie on `rho = 0.9399 - 0.0956 alpha_u` within 1e-4), about 0.6 of the blend's slope -0.163, and it does not tend to 1
as `alpha_u` goes to 0, as a blend would. So a part of the slow mode is set by the relaxation and a part is not; the term
of ADR 0811, `(1 - alpha_u)(phi* - F(u*))`, which grows as `alpha_u` falls, is one candidate for the part that is not (not
tested here).

### Table of checks

| Id | Limit | Measured | Met |
| --- | --- | --- | --- |
| B1 | all runs as registered | A0, B1 to B4: 20 of 20 steps each, no stop; KL 3 of 3 steps | yes |
| B2 | each mode names one or rules all out | A: named HA1; B: all ruled out | yes |
| B3 | passes, file unchanged | `baseline_hashes`: 3 passed, 1 ignored; `baseline_hashes.rs` unchanged | yes |
| R0 | `<= 1e-6` | A0 against G0 4.50e-10, B1 against GL 4.65e-10 in the heavy run; 0 and 0 from the committed records | yes |
| R1 | `<= 1e-6` | 0 over 115 iterations in the heavy run; 4.50e-10 from the committed records (10 printed digits of `d_k`) | yes |
| T1 | `<= 1e-12`, NaN where stated | largest relative difference 0 (steps 1 to 3, both active components, the cell and its four neighbours, its faces, the velocity); the z component NaN | yes |
| T2 | equal bits | KC0 and KL: `u`, `p`, flux and record equal; 6 probe records with, 0 without | yes |
| T3 | as built | `f885_tm_rules_on_built_records` passed | yes |
| K2 | `C <= 1e-2` | KL `C` 1.112e-8 (CONVERGES) | yes |
| K3 | rules all out | KL at cell 27: HA1, HA2, HA3 do not hold (`q` 5.67): all ruled out | yes |
| G2 | equal, `1e-6` | `f885_tm_fan_from_the_committed_record` passed: the held sets, `q`, `rho`, `r` and both results as recorded | yes |
| D1 | as stated | under `crates/*/src` only `crates/rcfd-fv/src/pimple.rs` (branch lines 197 to 203, the setting; 292 to 323, the two record types; 1145, its default; 1374 to 1376, 1463 and 1504, the solver's record; 1894 to 1911, the probe cells' faces and neighbours; 2026 to 2045, the iteration record and the fluxes; 2130 to 2135, the factors; 2204 to 2217, the diagonal and the sum; 2693 to 2697, the velocity; 2806 to 2815, the push); no roadmap, ledger, golden or existing test changed; of the four committed F-885 patches three apply unchanged; `f885_check_h_outer_record.mutation.patch` no longer applied (its context is the line before the outer loop, where the probe's lines now stand) and was re-based in 0238503c with its two changed lines unchanged (`git apply --check` passes) | yes |
| P | see the checks of rule 7 | as listed | yes |

### Mutations

From `mutations.log` (git archive of 9bbfee9e in the scratch folder, its own target folder, every source file touched
before each build; the gate tests of the new file):

| Id | What happened |
| --- | --- |
| K0 | unmutated: the five gate tests pass |
| N1 | the relaxed diagonal kept: caught by `f885_tm_probe_two_routes` (T1). Committed as the mutation patch |
| N2 | `s` from the lower coefficients only: caught by `f885_tm_probe_two_routes` (T1) |
| N3 | the factor of component 0 for every component: **not caught**. In T1's first outer iterations the factor is 1 at cell 27 and its four neighbours in both active components, so the two components cannot be told apart there. The fan record itself is not affected (its components differ: x 0.57, z 0.29 at c11837), but no gate test holds the component order of the factors |
| N4 | the fluxes with the face's sign: caught by `f885_tm_probe_two_routes` (T1) |
| N5 | the velocity of the iteration's start: caught by `f885_tm_probe_two_routes` (T1) |
| N6 | with the probe, the momentum solve from zero: caught by `f885_tm_probe_read_only` (T2) |
| N7 | the band 0.15: caught by `f885_tm_rules_on_built_records` (T3) and `f885_tm_fan_from_the_committed_record` (G2) |
| N8 | `probe.csv` cut to its first half: caught by `f885_tm_fan_from_the_committed_record` (G2) |

Seven of eight caught.

### Deviations

1. The fan runs took 6.4 min together (estimate about 8 min).
2. The re-base of `f885_check_h_outer_record.mutation.patch` (D1, foreseen by the brief) was checked with
   `git apply --check`; I did not run its test with it applied (the lead's push script does).
3. Rule 7's run of the new test file was made test by test as each registered step came due (T1 to T3, K2, K3 before the
   fan run; G2 after the record), each once. The five gate tests ran again, unmutated, as K0 of the mutation copy.
4. rustfmt was applied to the two touched files before the code commit, and to the layout of `FAN_RECORD` after the record
   commit (layout only); the fmt check then ran clean.
5. The full-precision numbers of `FAN_RECORD` were taken from the committed records by a scratch script (the same
   medians), checked against the six printed digits of the heavy run's log, before G2 was run.

### What was not run

Any remedy; any change of a default; check H, H(dt/2), a long run; mode A under the `alpha_u` and corrector grid; mode B
with the term of ADR 0811 off; the whole workspace suite.

### The checks of rule 7

* `cargo test -p rcfd-api --release --test f885_two_outer_modes`: T1 to T3, K2 and K3 (4 passed), G2 (1 passed); the heavy
  record run once.
* `cargo test -p rcfd-fv --release --lib`: 115 passed, 6 ignored.
* `cargo test -p rcfd-api --release --test baseline_hashes`: 3 passed, 1 ignored.
* `--test f885_outer_stall`: 5 passed, 1 ignored; `--test f885_alpha_free_face_flux`: 5 passed, 3 ignored;
  `--test f885_outer_fixed_points`: 4 passed, 1 ignored; `--test f885_check_h_outer_record`: 4 passed, 2 ignored.
* `cargo fmt -p rcfd-fv --check`, `cargo fmt -p rcfd-api --check`: clean; `cargo clippy -p rcfd-fv -p rcfd-api
  --all-targets -- -D warnings`: clean.
* `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and
  `python scripts/process_report.py --check` were run at the commit of this addendum, as reported to the lead.

### Size

Size (agent): done; 1; 1; 2; 150000

### Decision by the registered rule

**Done for this registration**: every run was made as registered (B1), mode A names HA1 and mode B rules all out (B2),
with B3, R0, R1, T1 to T3, G2 and D1 met. One planned mutation (N3) was not caught; that is recorded, and it is not a row
of the rule. F-885 stays **partial**: check H has not decided.

## Open questions for the maintainer (addendum 1)

1. **The remedy of mode A (the switching limiter factor).** Options: (a) an opt-in that keeps the Barth-Jespersen factor
   of the step's first outer iteration for its later ones (a limiter lagged within the step; the factor still follows the
   solution from step to step), registered under F-885 with the fan's `C` as its check; (b) a smooth limiter for the fan
   case (Venkatakrishnan exists in `gradient.rs`), which changes the case's discretisation and its results; (c) more outer
   iterations or a different reading of ADR 0183's rule for this case. I recommend (a): it removes the switch the record
   names without a new case key and can be checked against the record of this ADR. It will not reach `C <= 1e-2` by
   itself while mode B remains (GL of ADR 0815 stalls at 0.017).
2. **Mode B.** Options: (a) a further diagnosis under F-885 (rule 13) of the slow mode: the `alpha_u` grid run with the
   term of ADR 0811 off and on, and a per-cell record of the mode's shape; (b) take the remedy of mode A first, then see
   what stalls. I recommend (a), registered together with question 1's control, since a remedy of mode A alone does not
   let check H decide.

## For the lead: roadmap entries

> Status proposed for F-885: partial.

> Note entry: ADR 0816, 2026-11-26, diagnosis done, item partial: an opt-in probe of the fan's outer iterations on 20 steps of H(dt) names the cause of the constant change at c11837: the cell's own Barth-Jespersen factor (x component) switches between two values in every outer iteration of all ten classed steps (change 0.22 to 0.56, return within 2e-2) and the iterate cycles with it (q 0.005), while the diagonal dominates (a_c/s 1.53) and no face flux changes sign; the slow contraction without the limiter (factor 0.854) is not the lost dominance (r 1.38), not the corrector count (0.854 with four), not a blend of the relaxation (0.892 at alpha_u 0.5 against 0.919 predicted) and not independent of alpha_u (0.873 at 0.7): all four ruled out; the cavity with the limiter contracts (C 1.1e-8); held by crates/rcfd-api/tests/f885_two_outer_modes.rs and docs/investigations/f885-two-outer-modes.

> Gap entry: Missing for done: a class by ADR 0183's rule. Check H does not decide because the fan's outer loop does not contract within a step. ADR 0816: with the limiter the Barth-Jespersen factor at c11837 switches between two values each outer iteration (a period-2 cycle); without it a slow mode (0.854 per iteration) remains whose cause is not the dominance, the correctors or the relaxation's blend. Needs a remedy of the limiter cycle and a diagnosis of the slow mode (ADR 0816 questions 1 and 2).

The same entries are in `docs/roadmap/proposals/0816-F-885.json` (form 1 of ADR 0755, base d8b506d8), checked with
`python scripts/roadmap_proposals.py check`. No follow-up item is proposed: a remedy or a further diagnosis adds no
acceptance clause and would be registered under F-885 (rule 13).

Generated pages that this work may leave not current, for the lead (rule 9): `docs/adr/index.md` (ADR 0816 is new).

## Amendment 1 (2026-11-26): the kind of row R1

`python scripts/process_report.py --check` at fa731013 reads the kind of row R1 of the table of checks as `0`: the row's
check text holds absolute-value bars inside a code span (the probe's velocity change, `u_k - u_(k-1)` between bars),
which split the table's cells. The row is meant as written: check R1, limit relative `<= 1e-6`, source "Same discrete
problem by two routes", widened to the printed digits, estimate 0, margin n/a, kind **code verification**. Its limit and
its result (met; addendum 1) are unchanged. This amendment changes no check and covers no run.

For the lead: the row needs the line
`0816-f885-registration-the-two-modes-of-the-fans-outer-loop.md: R1` in `docs/process/exceptions/registration-kind.txt`,
as for ADR 0767 T3 and ADR 0806 DG3; that file is added to by the lead only.

## Answers of the maintainer (2026-11-26), written by the lead

1. Option (a): an opt-in that keeps the limiter factor of the step's first outer iteration, registered under F-885 as ADR 0819; taken by the lead on the maintainer's standing instruction of 2026-11-26 (go with the recommended option always).
2. Option (a): a further diagnosis of mode B, the alpha_u grid with ADR 0811's term off and on, in ADR 0819; taken by the lead on the maintainer's standing instruction of 2026-11-26 (go with the recommended option always).
