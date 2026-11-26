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
