# ADR 0819: F-885 registration: a limiter factor kept from the step's first outer iteration, and mode B with ADR 0811's term off and on

Status: **REGISTERED 2026-11-26, written before any code of F-885 for this registration and before any run of its checks.**
Before this text no library code, test, harness or solver run of this work was written or made. What was done: reading
(below), `git apply --check` of the five committed F-885 mutation patches at the base 61a5de03 (all five apply), and the
arithmetic of this page. Results go in addenda at the end; nothing above them is changed after a run.

It answers the brief [`docs/adr/briefs/f885-frozen-limiter.md`](briefs/f885-frozen-limiter.md) and follows
[`docs/process/agent-rules.md`](../process/agent-rules.md), version 5. A further registration of F-885 under rule 13 (a
remedy of mode A and a diagnosis of mode B of [ADR 0816](0816-f885-registration-the-two-modes-of-the-fans-outer-loop.md)),
taken as options (a) of ADR 0816's questions 1 and 2 by the lead's decision of 2026-11-26. Branch `f885-frozen-limiter`
from 61a5de03; scratch `w49i-work/f885-frozen-limiter-scratch`. Written by the model Claude Opus 5.5.

Size (agent): done; 1; 1; 2; 150000

## What is asked

ADR 0816 named mode A: at the interior cell c11837 the x-component Barth-Jespersen factor of the cell switches between two
values in every outer iteration (change 0.22 to 0.56), and the velocity cycles with it (`q` 0.005). Mode B (the limiter off,
a contraction of about 0.854 an iteration at c6245) kept its dominance and followed neither the corrector count nor the
blend of the relaxation; its factor falls with `alpha_u` at 0.6 of the blend's slope, and ADR 0816 named the term of
ADR 0811 as one candidate for the part the relaxation does not set. The brief asks for (1) an opt-in that keeps each cell's
limiter factor from the step's first outer iteration, with its rows on the fan, a steady check and the cavity; (2) mode B's
grid (`alpha_u` 0.9, 0.7, 0.5) with ADR 0811's term off and on, the limiter off, with the factor each hypothesis predicts.

## What was read

Whole: `AGENTS.md`; `docs/process/agent-rules.md` (version 5); the brief; ADR 0816 (registration, addendum 1, amendment 1,
answers); `crates/rcfd-api/tests/f885_two_outer_modes.rs`.

In part: ADR 0815 lines 115 to 135 (the measures and the class of `C`) and 240 to 250 (G0 and GL); the acceptance and the
gap of F-885 in `docs/roadmap.json` (read by a script); `crates/rcfd-fv/src/pimple.rs` (base 61a5de03) lines 160 to 330
(the settings of F-885 and the probe's records), 1094 to 1190 (`piso`, `pimple`: `alpha_u` 0.8 and one corrector in
`pimple`), 1880 to 1960 (the start of the outer loop, the freezes of ADR 0815), 2012 (`u_iter`, the iterate at the start of
the outer iteration), 2080 to 2180 (the momentum predictor: the time-extrapolated `conv_cells` of BDF2's first outer
iteration, the least-squares gradient, the call of `limiter`, the probe of the factor, `convection`, `laplacian_opt`);
`crates/rcfd-fv/src/gradient.rs` lines 344 to 416 (`limiter`, Barth-Jespersen); `docs/process/tolerances.md`, the table.
The only other call of `limiter` in the crate is the steady solver's (`simple.rs` 937), which this work does not touch.

No published source outside the repository was read. The design and the predictions come from the code and from first
principles (sections 1 and 3); nothing rests on a published result.

## 1. The opt-in (fixed now)

`PimpleSettings::outer_frozen_limiter: bool`, default `false`, in `crates/rcfd-fv/src/pimple.rs` only (the limiter's file
`gradient.rs` is not changed). Where the factor is kept and reset: a local of `step_with`, one optional vector of cell
factors per velocity component, created empty before the outer loop of each step (so it is reset at every step and never
outlives one). In the step's first outer iteration the factor `psi = limiter(conv_cells, ...)` of each active component
is formed as now and kept; in every later outer iteration of the step (the final unrelaxed one included) the kept vector
is used in its place. Only the factor is kept: the least-squares gradient is formed from the iteration's own values and
multiplied by the kept factor, as now by the formed one. With BDF2 the first outer iteration's `conv_cells` are the
time-extrapolated values (ADR 0183 P-A), so the kept factor is that of the extrapolated field; with implicit Euler it is
that of the velocity at the start of the step. The probe of ADR 0816 records the factor that was used.

Read only where the factor is formed now (`linear_upwind_limiter` set and `convection = LinearUpwind`); without the
limiter it does nothing. Not read by the fractional step or the moving-mesh steps (they form no factor in this file), nor by
the steady solver. Default `false`: the branch that keeps a factor is not taken and no copy is made, so every default is
bit-identical; `baseline_hashes` unchanged. No case key, schema, golden or hash table; the runs set it on the built solver
as ADR 0815 and ADR 0816 set their controls.

Why it removes mode A (first principles). Within a step, with the factor fixed, the convection operator of the predictor
no longer depends on the iterate through `psi`; the switch that ADR 0816 recorded (HA1) cannot occur, so the period-2
cycle it carried has no source. At a steady state the step starts from the converged field, the kept factor is the factor
of that field, and the fixed point of the outer iteration is the one without the opt-in: the converged answer is unchanged
(check S1).

## 2. The rows on the fan (fixed now)

Harness: that of ADR 0816 (`h_text(4.605e-5)` copied unchanged, `alpha_free_face_flux = true`, `n_outer = 20`, the record
on, `outer_pressure_tolerance = Some(-1.0)`), the probe at c11837 and c6245, 20 steps from the uniform start.

| Run | limiter | `outer_frozen_limiter` | ADR 0811's term | `alpha_u` | correctors | reproduces |
| --- | --- | --- | --- | --- | --- | --- |
| F | Barth-Jespersen | on | on | 0.9 | 2 | step 1, `d_1`: A0 of ADR 0816 |
| B1r | none | off | on | 0.9 | 2 | B1 of ADR 0816 (all `d_k`) |
| M1 | none | off | off | 0.9 | 2 | |
| M2 | none | off | off | 0.7 | 2 | |
| M3 | none | off | off | 0.5 | 2 | |

The term-on legs of mode B at `alpha_u` 0.7 and 0.5 are B2 and B3 of ADR 0816, taken from its committed record
(`FAN_RECORD` of `f885_two_outer_modes.rs`: `rho` 0.8729920722691961 and 0.8920440841467279; B1 0.853789228181186); B1r
shows that the code of this work reproduces that leg (R0). Each run once, in one heavy test, the record of each written as
it ends. Time estimate: ADR 0816's runs took 62 s to 67 s each (two correctors); about 5.5 min for the five in one
background process; no cap. No long run, no check H.

The measures (as ADR 0815 and ADR 0816, upper median): `C` = median of `d_19 / d_2` over steps 11 to 20, classed
**CONVERGES** if `C <= 1e-2`, **STALLS** if `C > 1e-2`, **UNDECIDED** if NaN or the run did not complete (ADR 0815, "The
measures"; ADR 0810 section 5). `rho` of a run = median over steps 11 to 20 of `(d_19 / d_16)^(1/3)` (ADR 0816 section 2).
At c11837 in F: the rule of mode A of ADR 0816 section 2 (HA1, HA2, HA3 with its numbers) and the shape `q` (median over
the classed steps of the median over `k = 3..19` of `abs(u_k - u_(k-2)) / abs(u_k - u_(k-1))`, the norms of the velocity
differences of the probe).

**The cycle gone (row FA2), the number.** For a mode of the iterate that changes by a factor `rho` an iteration
(`u_k - u_(k-1)` proportional to `rho^k`), `q = abs(1 + rho) / abs(rho)`: 0 for the two-state cycle (`rho = -1`), 0.11 at
`rho = -0.9`, at least 2 for a monotone contraction (`0 < rho < 1`). ADR 0816 recorded `q` 0.005 in A0. The cycle is gone if
`q >= 0.1` (an oscillation, if any is left, shrinks by at least `1/1.1` an iteration: `rho >= -0.909`), the return ratio of
ADR 0816 section 2 applied to the iterate, and the rule of mode A rules all out at c11837.

## 3. Mode B with ADR 0811's term off and on (fixed now)

`rho_on(alpha)`: B1, B2, B3 of ADR 0816 (above). `rho_off(alpha)`: M1, M2, M3. ADR 0816 recorded that `rho_on` lies on
`0.9399 - 0.0956 alpha_u`, about 0.6 of the slope of the blend `1 - alpha_u (1 - lambda_0)` (ADR 0816 section 1), and named
the term `(1 - alpha_u)(phi* - F(u*))` of ADR 0811 as one candidate for the part that the relaxation does not set (it grows
as `alpha_u` falls).

| Id | Hypothesis | Predicted factor | Holds if |
| --- | --- | --- | --- |
| HC1 | the term carries the part of the slow mode that the relaxation does not set: without it the loop is the blend of the relaxation with a slow unrelaxed map | `rho_off(alpha) = 1 - alpha (1 - lambda_off)`, `lambda_off = 1 - (1 - rho_off(0.9)) / 0.9` | `abs(rho_off(0.7) - pred) <= 0.015` and `abs(rho_off(0.5) - pred) <= 0.015` |
| HC2 | the term takes no part in the slow mode | `rho_off(alpha) = rho_on(alpha)` | `abs(rho_off(alpha) - rho_on(alpha)) <= 0.015` at 0.9, 0.7 and 0.5 |
| HC3 | the term slows the loop at every `alpha_u` | `rho_off(alpha) < rho_on(alpha)` | `rho_off(alpha) <= rho_on(alpha) - 0.015` at 0.9, 0.7 and 0.5 |

The band 0.015 is ADR 0816's (section 2: less than half the smallest separation of two of its predictions; the spread of
the factor in ADR 0815 is a tenth of it). HC2 and HC3 exclude each other. HC1 and HC2 can both hold only if `rho_off` lies
within 0.015 of both predictions at 0.7 and at 0.5; with `rho_off(0.9) = rho_on(0.9)` the two predictions are 0.886 and
0.873 at 0.7 and 0.919 and 0.892 at 0.5, so a narrow overlap exists; the rule names none then.

**Result C.** `S_C` the set of HC1 to HC3 that hold. `{HC1, HC3}`: **names HC1** (without the term the loop is a blend and
faster). One member: **names** it. Empty: **rules all out**. Otherwise: **names none** (with the set). A run of M1 to M3 not
complete or a factor NaN: **undecided**.

Estimate (not a limit): HC1, the candidate ADR 0816 named; I have no number for `rho_off` before the run.

Beside, without a limit: `C` and its class of B1r and M1 to M3; the dominance ratio `r` at c6245 (ADR 0816's measure).

## 4. The cavity rows (fixed now)

`box_2d(8, 8, 1, 1)`, lid `(1, 0, 0)`, from rest, `convection = LinearUpwind`, `linear_upwind_limiter = BarthJespersen`
unless stated.

* **T1, two routes**: `PimpleSettings::pimple(0.01, 5)` (implicit Euler, `alpha_u` 0.8), the opt-in on, `dt` 0.05, 3 steps,
  the probe at all 64 cells. In each step, the factor of each active component that the probe records at each cell in each
  of the five outer iterations (the final one included) against `limiter` of the state before the step, formed in the
  test with the crate's public functions (`least_squares`, `limiter`, as ADR 0816 T1), relative `<= 1e-12`; NaN for the
  inactive component. Non-vacuity: the same run with the opt-in off has, in step 2 or 3, some cell and component whose
  factor in iteration 5 differs from iteration 1 by more than 1e-9 (ADR 0816's floor of a switching factor); if not, T1
  does not decide.
* **T2, read only without the limiter**: KC0 of ADR 0815 (`pimple(0.01, 20)`, BDF2, `dt` 0.05, 3 steps, the record and the
  switch on, no limiter) with the opt-in on against off: `u`, `p`, flux and the record of `d_k`, equal bits.
* **K, the control**: KL of ADR 0816 (KC0 with the limiter) with the opt-in on: `C` (median `d_19 / d_2` over steps 1 to 3)
  `<= 1e-2`.
* **S1, the steady answer**: `pimple(0.1, 20)` (`nu` 0.1, Re 10 on the lid), implicit Euler, the switch on, `dt` 0.1, 400
  steps from rest, the opt-in off and on. Derivation of the length: the slowest viscous decay of the box is about
  `tau = 1 / (2 pi^2 nu)` = 0.51 s; implicit Euler damps it by `1 / (1 + dt / tau)` = 0.84 a step, so 400 steps leave
  `0.84^400` = 1e-30 of the start, below rounding; a slower mode of the flow (up to `tau` 2 s, factor 0.95) still leaves
  1e-9 by step 400 and the precondition below then fails rather than passing falsely. Preconditions, each run: steady,
  `max_c abs(u^400 - u^399) <= 1e-12` m/s (the lid speed 1); the limiter active, the factor of the final field (formed in
  the test with `limiter`) below `1 - 1e-9` at one cell and component at least. If a precondition fails, S1 does not
  decide. The check: `max_c abs(u_on - u_off) / max_c abs(u_off) <= 1e-8` and the same for `p - mean(p)`.

## Table of checks

There is no clause of the acceptance of F-885 that this work can meet (check H is not run); the brief's task is the opt-in,
its rows and the diagnosis.

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
| --- | --- | --- | --- | --- | --- | --- |
| FA1 | F: the factor used at c11837 and its face neighbours, each active component, in outer iterations 2 to 20 of each classed step against the one of iteration 1: the largest absolute change | `<= 1e-12` | `docs/process/tolerances.md`, "Same discrete problem by two routes", `<= 1e-12` where only rounding separates (a factor in `[0, 1]`) | 0 | n/a | code verification |
| FA2 | F: the cycle at c11837 gone: the rule of mode A of ADR 0816 rules all out at c11837, and `q >= 0.1` | as stated | ADR 0816 section 2 (the rule of mode A and its return ratio 0.1), this registration section 2 | all ruled out; `q` near 2 | n/a | solution verification |
| FC | F: `C` over steps 11 to 20, classed by ADR 0815's rule | classed: CONVERGES or STALLS, not UNDECIDED (the class is recorded, it is not a pass or fail limit) | ADR 0815, "The measures"; ADR 0810 section 5 | STALLS, `C` near 0.017 (GL of ADR 0815) | n/a | solution verification |
| R1 | F against A0 of ADR 0816 (`docs/investigations/f885-two-outer-modes/fan_d.csv`): `d_1` of step 1 (the first outer iteration, where the kept factor is the formed one) | relative `<= 1e-6` | "Same discrete problem by two routes", widened to the 10 printed digits as ADR 0816 R0 | 0 | n/a | code verification |
| R0 | B1r against B1 of ADR 0816: every `d_k` of 20 steps | relative `<= 1e-6` | "Same discrete problem by two routes", widened to the 10 printed digits as ADR 0816 R0 | 0 | n/a | unchanged default output |
| BC | M1 to M3 complete 20 steps or stop with the stop recorded; result C names a hypothesis or rules all out (section 3) | as stated | the brief, decision rule | named HC1 | n/a | solution verification |
| S1 | the steady cavity: `u` and `p - mean(p)` with the opt-in on against off, with the preconditions of section 4 | relative `<= 1e-8` each | "Same discrete problem by two routes", `<= 1e-8` | 1e-13 | 1e5 | code verification |
| T1 | two routes, the cavity (section 4), and its non-vacuity | relative `<= 1e-12`, NaN where stated | "Same discrete problem by two routes", `<= 1e-12` where only rounding separates | 0 | n/a | code verification |
| T2 | the opt-in read only with the limiter: KC0 on against off | equal bits | "Bit-identity of default output" | equal | n/a | unchanged default output |
| K | KL with the opt-in: `C` | `<= 1e-2` (CONVERGES) | ADR 0810 section 5 (ADR 0183's outer ratio, as ADR 0767 E-1 states it) | 1e-8 (KL of ADR 0816: 1.1e-8) | 1e6 | code verification |
| T3 | the rule of section 3 on built factors: each of HC1 to HC3 alone, HC1 with HC3, HC1 with HC2, none, NaN; the class of `C` and the rule of FA2 on built records | each as built | this registration, sections 2 and 3 | as built | n/a | code verification |
| G2 | the outcomes from the committed records: FA1, FA2 (the held set, `q`), FC (`C`, class), R0, R1, the factors of M1 to M3 and B1r, result C | equal; numbers within 1e-6 relative | "Same discrete problem by two routes", widened to the printed digits | 0 | n/a | unchanged default output |
| B3 | no solver default changes: `cargo test -p rcfd-api --release --test baseline_hashes` passes, `baseline_hashes.rs` unchanged | passes, file unchanged | the brief; rule 3 | passes | n/a | unchanged default output |
| D1 | under `crates/*/src` only `crates/rcfd-fv/src/pimple.rs` changes; `docs/roadmap.json`, the ledger, every golden and every existing test file unchanged; the five committed F-885 mutation patches still apply (`git apply --check`), re-based in this branch with the mutation unchanged if not | as stated | the brief, owned files; rule 3 | as stated | n/a | unchanged default output |
| P | `cargo test -p rcfd-fv --release --lib`; `cargo test -p rcfd-api --release --lib`; the new test file; the existing tests of the changed step `f885_two_outer_modes`, `f885_outer_stall`, `f885_alpha_free_face_flux`, `f885_outer_fixed_points`, `f885_check_h_outer_record`; `cargo fmt -p rcfd-fv -p rcfd-api --check`; `cargo clippy -p rcfd-fv -p rcfd-api --all-targets -- -D warnings`; `cargo test -p rcfd-cli --release --test docs_site`; `python scripts/roadmap_proposals.py check`; `python scripts/process_report.py --check` | clean; passes; no failure but a generated file not current | rule 7 | as stated | n/a | code verification |

The numbers of section 3 and the class of `C` class a run; they are not pass or fail limits of a fan run. No row is of kind
validation.

## Tests (fixed now)

All in `crates/rcfd-api/tests/f885_frozen_limiter.rs` (header: ADR 0819, F-885).

* Run by the gate: `f885_fl_two_routes` (T1); `f885_fl_read_only` (T2); `f885_fl_cavity_control` (K);
  `f885_fl_steady_same_answer` (S1); `f885_fl_rules_on_built_records` (T3); `f885_fl_fan_from_the_committed_record` (G2, and
  FA1, FA2, FC, R0, R1, BC from the committed records).
* Heavy record (`#[ignore]`, run once by me): `f885_fl_fan_record`, F, B1r, M1, M2, M3, writing to the folder named by
  `F885_FL_OUT`; the fast tests on the same code path are T1, T2, K and S1 (the opt-in) and ADR 0811's
  `f885_af_short_run_on_the_check_h_path` (the fan harness).

Records in `docs/investigations/f885-frozen-limiter/`: `fan_d.csv` (run, step, `k`, `d_k`, cell; all steps), `probe.csv`
(F's classed steps at c11837 and c6245, as ADR 0816's), the run log, the cavity log and the mutation log. The outcomes are
written into the test file after the registered run (`FAN_RECORD`), as ADR 0816 did; the form and the tolerance are fixed
here.

## Negative controls and planned mutations

On a scratch copy of the branch (`git archive` of the full commit into the scratch folder, its own target folder, every
source file touched after extraction), each against the gate tests of the new file; the heavy record is not rerun for a
mutation.

| Id | Mutation | Must be caught by |
| --- | --- | --- |
| N1 | solver: the kept factor is not reset at a new step (kept from the first step of the run) | `f885_fl_two_routes` (T1) |
| N2 | solver: the final (unrelaxed) outer iteration forms its factor anew | `f885_fl_two_routes` (T1) |
| N3 | solver: the kept factor of component 0 used for every component | `f885_fl_two_routes` (T1) |
| N4 | solver: the opt-in not read (the factor formed in every iteration) | `f885_fl_two_routes` (T1) |
| N5 | solver: with the opt-in on, the least-squares gradient of the step's first outer iteration is kept as well, with or without a limiter | `f885_fl_read_only` (T2) |
| N6 | test: the band of section 3 0.15 for 0.015 | `f885_fl_rules_on_built_records` (T3) |
| N7 | record: `fan_d.csv` cut to its first half | `f885_fl_fan_from_the_committed_record` (G2) |

The committed patch (`crates/rcfd-api/tests/f885_frozen_limiter.mutation.patch`) is N1, with the first line
`# expect-fail: cargo test -p rcfd-api --release --test f885_frozen_limiter f885_fl_two_routes`.

## Decision rule

* **Done** for this registration if every row holds as registered: FA1, FA2, R0, R1, S1, T1, T2, K, T3, G2, B3, D1 and P
  met, FC classed (not UNDECIDED) and BC met (result C names a hypothesis or rules all out), and each mutation of the
  table is caught.
* **Partial** if a row misses or does not decide, a run was not made as registered, or a mutation is not caught, with the
  reason and what it would take.
* **Stopped and reported** if the opt-in cannot be made with the defaults bit-identical, if B1r does not reproduce B1 of
  ADR 0816 (R0), or if anything else in the solver would need changing.

F-885 itself stays partial whatever this gives, until check H decides (the brief). A defective set-up found before a
registered run is corrected by an amendment committed before the corrected run.

## What is not covered

* Check H, H(dt/2), a long run; any change of a default; a smooth limiter; the opt-in in the steady solver.
* A remedy of mode B; a hypothesis for it other than HC1 to HC3; mode B under the corrector grid (done in ADR 0816).
* The opt-in with the limiter on and the term off; the frozen factor of a field other than the step's first iterate.

## Open questions for the maintainer

To be completed in the addendum with options and a recommendation: the next step for check H, and any question of scope.

## Addendum 1: results

Written 2026-11-26, after the runs. Commits on the branch, from 61a5de03: 0d8a5472 (this registration, alone), e06ad42a
(the opt-in in `crates/rcfd-fv/src/pimple.rs` and the test file; every registered run was made from it), 71cca463 (the fan
record, the logs, the recorded outcomes `FAN_RECORD`), e91d00a3 (rustfmt's layout of the test file), and the commit of
this addendum with the mutation log, the committed mutation patch and the proposal file. Records in
`docs/investigations/f885-frozen-limiter/`: `fan_d.csv`, `probe.csv`, `fan.log`, `cavity.log`, `mutations.log`.

### The sentence

**The opt-in removes mode A but the fan's loop still stalls; ADR 0811's term takes no part in mode B.** With the factor
kept from the step's first outer iteration, the factor at c11837 and its neighbours does not change within a step, the
cell's iterate no longer cycles (`q` 2.25 against ADR 0816's 0.005; mode A rules all out), and the steady cavity gives the
same answer to 1e-12. `C` falls from 0.0258 (G0 of ADR 0815) to 0.0143, still STALLS: the largest change of the late
iterations now sits at c13101, c6245 and others, and the run's factor is 0.834 an iteration. Without the limiter, turning
the term off leaves the slow factor where it was at every `alpha_u` (HC2).

### The fan (each 20 of 20 steps, no stop; 6.7 min in one process)

| Run | `rho` | `C` | Class | s |
| --- | --- | --- | --- | --- |
| F | 0.834438 | 0.014301 | STALLS | 79.4 |
| B1r | 0.853789 | 0.017267 | STALLS | 71.4 |
| M1 | 0.854110 | 0.017659 | STALLS | 86.6 |
| M2 | 0.871342 | 0.011825 | STALLS | 89.6 |
| M3 | 0.892664 | 0.009457 | CONVERGES | 68.2 |

Cell of `d_19` in F's classed steps: c13101 in 4, c6245 in 3, c9315 in 2, c5631 in 1 (A0 of ADR 0816: c11837 in all ten).

Mode B, the factor with the term off against on (B1 to B3 of ADR 0816), and HC1's blend from `rho_off(0.9)`
(`lambda_off` 0.837900):

| `alpha_u` | off | on | off minus on | HC1 predicts |
| --- | --- | --- | --- | --- |
| 0.9 | 0.854110 | 0.853789 | +0.00032 | (its base) |
| 0.7 | 0.871342 | 0.872992 | -0.00165 | 0.886530 (off by 0.0152, outside 0.015) |
| 0.5 | 0.892664 | 0.892044 | +0.00062 | 0.918950 (off by 0.0263) |

`S_C = {HC2}`: **named HC2**. HC1 misses at 0.7 by 0.0002 beyond the band and at 0.5 by 0.011 beyond it; HC3 holds at no
`alpha_u`. Beside, without a limit: `r` at c6245 1.376 to 1.380 (dominance kept, as in ADR 0816); M3 at `alpha_u` 0.5
classes CONVERGES (`C` 0.0095) while its factor is the slowest, because its `d_2` is larger; that is a property of the
ratio, not of the mode.

### Table of checks

| Id | Limit | Measured | Met |
| --- | --- | --- | --- |
| FA1 | `<= 1e-12` | 0 (10 classed steps, the cell and its six neighbours, three components, iterations 2 to 20) | yes |
| FA2 | mode A rules all out and `q >= 0.1` | HA1, HA2, HA3 not held (all ruled out); `q` 2.2505 | yes |
| FC | classed, not UNDECIDED | `C` 0.014301, STALLS | yes (classed) |
| R1 | `<= 1e-6` | 1.6e-11 in the heavy run; 0 from the committed records | yes |
| R0 | `<= 1e-6` | 4.65e-10 in the heavy run; 0 from the committed records | yes |
| BC | M1 to M3 complete; result C names or rules all out | 20 of 20 each; named HC2 | yes |
| S1 | `<= 1e-8`; preconditions | `u` 8.207e-13, `p` 1.337e-12; last-step change 2.8e-17 (off) and 2.9e-14 (on) against 1e-12; 42 limited cell components in each | yes |
| T1 | `<= 1e-12`, NaN where stated; non-vacuous | 2880 factors compared, largest relative difference 0; without the opt-in a factor changes by 0.8375 in steps 2 and 3 (floor 1e-9) | yes |
| T2 | equal bits | KC0 on against off: `u`, `p`, flux and record equal | yes |
| K | `C <= 1e-2` | 1.0597e-8 (CONVERGES) | yes |
| T3 | as built | `f885_fl_rules_on_built_records` passed | yes |
| G2 | equal; `1e-6` | `f885_fl_fan_from_the_committed_record` passed | yes |
| B3 | passes, file unchanged | in the checks of rule 7 (addendum 2) | see addendum 2 |
| D1 | as stated | under `crates/*/src` only `crates/rcfd-fv/src/pimple.rs` (branch lines 204 to 209, the setting; 1152, its default; 1901 to 1902, the kept factors; 2138 to 2148, their use and keeping); no roadmap, ledger, golden or existing test changed; the five committed F-885 mutation patches apply unchanged with `git apply --check` after the change, none re-based | yes |
| P | the checks of rule 7 | in addendum 2 | see addendum 2 |

### Mutations

From `mutations.log` (git archive of e91d00a3 in the scratch folder, its own target folder, every file touched after
extraction; each against the test the registration named):

| Id | What happened |
| --- | --- |
| K0 | unmutated: the four gate tests run pass |
| N1 | the kept factor not reset at a new step: caught by `f885_fl_two_routes` (T1). Committed as the mutation patch |
| N2 | the final iteration forms its factor anew: caught by `f885_fl_two_routes` (T1) |
| N3 | component 0's kept factor for every component: caught by `f885_fl_two_routes` (T1) |
| N4 | the opt-in not read: caught by `f885_fl_two_routes` (T1) |
| N5 | the least-squares gradient of the first iteration kept as well: **not caught** by `f885_fl_read_only` (T2). KC0 uses upwind convection without non-orthogonal correctors, so the step forms no gradient there and the mutation has nothing to act on. T2 checks what it was meant to (the opt-in does nothing without a limiter), but no gate test holds that the gradient is formed anew in each iteration when a limiter is set |
| N6 | the band 0.15: caught by `f885_fl_rules_on_built_records` (T3) |
| N7 | `fan_d.csv` cut to its first half: caught by `f885_fl_fan_from_the_committed_record` (G2) |

Six of seven caught.

### Deviations

1. The fan runs took 6.7 min together (estimate about 5.5 min).
2. `FAN_RECORD`'s full-precision numbers were taken from the committed records by a scratch script (the same medians),
   checked against the six printed digits of the heavy run's log, before G2 was run.
3. rustfmt was applied to the new test file after the record commit (layout only). `cargo fmt -p` refused on this machine
   ("The filename or extension is too long", os error 206), so `rustfmt` was run on the two touched files with the
   repository's `rustfmt.toml`; `pimple.rs` was already in its layout.
4. A first build of all of rcfd-api's test targets was stopped by me before it finished (a build, not a registered run)
   and replaced by the build of the new test file only, to keep the load on the shared machine down.
5. The gate tests of the new file ran once each as their registered steps came due (T1, T2, K, S1, T3 before the fan
   run; G2 after the record); four of them ran again, unmutated, as K0 of the mutation copy.

### What was not run

Check H, H(dt/2), a long run; the opt-in with the term off; the opt-in in the steady solver; the whole workspace suite.

### Size

Size (agent): partial; 1; 1; 2; 160000

### Decision by the registered rule

**Partial for this registration.** Every row holds as registered (FA1, FA2, R0, R1, S1, T1, T2, K, T3, G2 and D1 met, FC
classed STALLS, BC named HC2; B3 and P in addendum 2), but one mutation of the table (N5) was not caught, and the rule asks
that each be caught. What it would take: a gate test with a limiter and linear upwind in which the gradient changes
between outer iterations (for example T1's cavity, compared with the convection operator formed in the test), registered
by an amendment. F-885 stays **partial**: check H has not decided.

## Open questions for the maintainer (addendum 1)

1. **The next step for check H.** With the opt-in the fan's loop still stalls (`C` 0.0143 against 1e-2, factor 0.834), so
   check H run with the opt-in as it stands would record "does not decide". Options: (a) a further diagnosis under F-885
   (rule 13) of the slow mode that remains (0.83 to 0.85 an iteration, dominance kept, not the correctors, not the blend,
   not ADR 0811's term), for instance its spatial shape from the probe at the cells that now carry `d_19` (c13101, c6245,
   c9315) and the outer map with the pressure equation's relaxation `alpha_p`; (b) check H with the opt-in and more outer
   iterations per step (at a factor of 0.834, `d_k / d_2 = 0.834^(k - 2)` reaches 1e-2 at about `k` = 28, where 20 do not),
   which raises the case's cost and needs a reading of ADR 0183's rule for that count; (c) the maintainer's reading of
   ADR 0183's outer rule for a loop that contracts slowly but steadily. I recommend (a): it keeps the rule as written and
   goes after the one mode that remains; (b) is the fallback if (a) finds no cause.
2. **Scope: the missed mutation N5.** Options: (a) an amendment of this registration that adds the gate test named in the
   decision above and runs N5 against it; (b) leave it recorded, since the opt-in keeps only the factor and does not
   touch the gradient line. I recommend (a), registered together with question 1's next step, so the remedy's code is
   held by its tests.

## For the lead: roadmap entries

> Status proposed for F-885: partial.

> Note entry: ADR 0819, 2026-11-26, opt-in and diagnosis made, item partial: an opt-in that keeps each component's Barth-Jespersen factor of the step's first outer iteration (outer_frozen_limiter, default off, baseline hashes unchanged) removes the fan's period-2 cycle at c11837 on 20 steps of H(dt) (factor change 0, q 2.25 against 0.005, mode A all ruled out) and gives the same steady cavity answer (u 8e-13, p 1e-12 relative), but the loop still stalls (C 0.0143 against 1e-2) with the slow mode at c6245; without the limiter, turning ADR 0811's term off leaves the slow factor as it was at alpha_u 0.9, 0.7, 0.5 (0.854, 0.871, 0.893 against 0.854, 0.873, 0.892): named HC2, the term takes no part; six of seven mutations caught; held by crates/rcfd-api/tests/f885_frozen_limiter.rs and docs/investigations/f885-frozen-limiter.

> Gap entry: Missing for done: a class by ADR 0183's rule. Check H does not decide because the fan's outer loop does not contract within a step. ADR 0819: the opt-in frozen limiter removes the limiter's period-2 cycle at c11837, but the loop still stalls (C 0.0143) on a slow mode (about 0.83 to 0.85 per outer iteration) at c6245 that is not the dominance, the correctors, the relaxation's blend or ADR 0811's term. Needs a diagnosis of that slow mode, or the maintainer's reading of the outer rule.

The same entries are in `docs/roadmap/proposals/0819-F-885.json` (form 1 of ADR 0755, base 61a5de03), checked with
`python scripts/roadmap_proposals.py check`. No follow-up item is proposed: a further diagnosis adds no acceptance clause
and would be registered under F-885 (rule 13).

Generated pages that this work may leave not current, for the lead (rule 9): `docs/adr/index.md` (ADR 0819 is new).

## Addendum 2: the checks of rule 7

Written 2026-11-26, after addendum 1 (b53e5971), from the run at that commit (scratch `rule7.log`), each once:

* `cargo test --release -p rcfd-fv --lib`: 115 passed, 6 ignored.
* `cargo test --release -p rcfd-api --lib`: 160 passed, 4 ignored.
* `cargo test --release -p rcfd-api --test baseline_hashes`: 3 passed, 1 ignored; `baseline_hashes.rs` unchanged since
  61a5de03 (B3 met).
* `--test f885_two_outer_modes`: 5 passed, 1 ignored; `--test f885_outer_stall`: 5 passed, 1 ignored;
  `--test f885_alpha_free_face_flux`: 5 passed, 3 ignored; `--test f885_outer_fixed_points`: 4 passed, 1 ignored;
  `--test f885_check_h_outer_record`: 4 passed, 2 ignored.
* The new file `f885_frozen_limiter`: its gate tests as in addendum 1 (each once as its registered step came due).
* Format: `cargo fmt -p rcfd-fv -p rcfd-api --check` refuses on this machine ("The filename or extension is too long",
  os error 206) and checked nothing; `rustfmt --check` on the two touched files (`crates/rcfd-fv/src/pimple.rs`,
  `crates/rcfd-api/tests/f885_frozen_limiter.rs`) with the repository's `rustfmt.toml`: clean.
* `cargo clippy --release -p rcfd-fv -p rcfd-api --all-targets -- -D warnings`: clean (no warning).
* `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and
  `python scripts/process_report.py --check` are run at the commit of this addendum, as reported to the lead.

P is met for the cargo checks, with the fmt check made by `rustfmt` in place of `cargo fmt` (a deviation). The decision of
addendum 1 stands: partial for this registration (N5 not caught).

## Amendment 1 (2026-11-26): a gate test for N5, written before its code

The lead's answer of 2026-11-26 to question 2 of addendum 1 takes option (a), on the maintainer's standing instruction of
2026-11-26 (go with the recommended option always): a gate test with a limiter in which the gradient changes, so that N5
(the least-squares gradient of the step's first outer iteration kept as well) is caught. Nothing above is changed; T2 and
its result stand, and N5 against T2 stays recorded as not caught.

**T4, the opt-in with a limiter set and a scheme that forms a gradient but no factor.** The cavity KC0 of ADR 0815
(`box_2d(8, 8, 1, 1)`, lid `(1, 0, 0)`, `pimple(0.01, 20)`, BDF2, `dt` 0.05, 3 steps from rest, the record and the switch
on) with `convection = Quick` (it needs the cell gradient, `ConvScheme::needs_gradient`) and
`linear_upwind_limiter = BarthJespersen`. The step forms the least-squares gradient anew in every outer iteration and
forms no limiter factor (the factor is formed only with linear upwind, section 1), so the opt-in has nothing to keep. The
check: the opt-in on against off, `u`, `p`, flux and the record of `d_k`, **equal bits**. Source: `docs/process/tolerances.md`,
"Bit-identity of default output" (the opt-in must not act where no factor is formed, section 1). Non-vacuity: with the
opt-in off, `d_2 > 0` in each of the three steps (the iterate, and so the gradient, changes between outer iterations); if
not, T4 does not decide. Kind: unchanged default output. Estimate: equal. Test: `f885_fl_read_only_with_a_gradient` in
`crates/rcfd-api/tests/f885_frozen_limiter.rs`.

N5 is run again, unchanged, on a scratch copy of the full commit that adds T4, against T4; it must be caught by T4. The
decision rule of the registration stands, with N5's row read against T4: done for this registration if T4 is met and N5
is caught by it, the other rows and mutations as in addendum 1. The new test and the named F-885 test files
(`f885_two_outer_modes`, `f885_outer_stall`, `f885_alpha_free_face_flux`, `f885_outer_fixed_points`,
`f885_check_h_outer_record`) are run once after the code, with `cargo fmt` (or `rustfmt --check` where `cargo fmt -p`
refuses, addendum 2), `cargo clippy -p rcfd-api --all-targets -- -D warnings`, `docs_site`, the proposal check and the
process report. The results go in a further addendum (addendum 1 is not edited, rule 18).

## Addendum 3: results of amendment 1

Written 2026-11-26. Commits: 1c2d027c (amendment 1, alone), 54ecfd02 (the test T4; its run and N5's were made from it),
and the commit of this addendum with `docs/investigations/f885-frozen-limiter/mutations-amendment-1.log` and the updated
proposal file. Addenda 1 and 2 stay as written (rule 18); where they say partial, this addendum gives the decision of the
registered rule with amendment 1.

| Id | Limit | Measured | Met |
| --- | --- | --- | --- |
| T4 | equal bits; non-vacuous `d_2 > 0` each step | KC0 with QUICK and the limiter set, the opt-in on against off: `u`, `p`, flux and record equal; `d_2` without the opt-in 8.10e-3, 6.77e-3, 6.29e-3 m/s | yes |

**N5 against T4** (git archive of 54ecfd02 in the scratch folder, its own target folder, every file touched after
extraction; the mutation's edits as in addendum 1, unchanged): `f885_fl_read_only_with_a_gradient` fails, **caught**. With
it, seven of seven planned mutations are caught (N1 to N4 by T1, N5 by T4, N6 by T3, N7 by G2).

Checks made once after the code of amendment 1: the new test T4 (above); `--test f885_two_outer_modes` 5 passed, 1
ignored; `--test f885_outer_stall` 5 passed, 1 ignored; `--test f885_alpha_free_face_flux` 5 passed, 3 ignored;
`--test f885_outer_fixed_points` 4 passed, 1 ignored; `--test f885_check_h_outer_record` 4 passed, 2 ignored;
`rustfmt --check` of the two touched files: clean; `cargo clippy --release -p rcfd-api --all-targets -- -D warnings`:
clean. The library code is unchanged by amendment 1 (only the test file), so the `--lib` runs and `baseline_hashes` of
addendum 2 stand. `docs_site`, the proposal check and the process report are run at the commit of this addendum, as
reported to the lead.

Size (agent), after amendment 1: done; 1; 1; 2; 165000

**Decision by the registered rule, with amendment 1: done for this registration.** Every row holds as registered (FA1,
FA2, R0, R1, S1, T1, T2, T4, K, T3, G2, B3, D1 and P met, FC classed STALLS, BC named HC2) and each mutation of the table is
caught. F-885 stays **partial**: check H has not decided (question 1 of addendum 1 is answered by the lead in ADR 0820).

For the lead, the note entry replaces the one of addendum 1:

> Note entry: ADR 0819, 2026-11-26, opt-in and diagnosis done, item partial: an opt-in that keeps each component's Barth-Jespersen factor of the step's first outer iteration (outer_frozen_limiter, default off, baseline hashes unchanged) removes the fan's period-2 cycle at c11837 on 20 steps of H(dt) (factor change 0, q 2.25 against 0.005, mode A all ruled out) and gives the same steady cavity answer (u 8e-13, p 1e-12 relative), but the loop still stalls (C 0.0143 against 1e-2) with the slow mode at c6245; without the limiter, turning ADR 0811's term off leaves the slow factor as it was at alpha_u 0.9, 0.7, 0.5 (0.854, 0.871, 0.893 against 0.854, 0.873, 0.892): named HC2, the term takes no part; seven of seven mutations caught (N5 by the test of amendment 1); held by crates/rcfd-api/tests/f885_frozen_limiter.rs and docs/investigations/f885-frozen-limiter.

The status (partial) and the gap entry of addendum 1 are unchanged. The proposal file carries the new note.
