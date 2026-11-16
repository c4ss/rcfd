# ADR 0642: F-717 registration: the fidelity driver's uncertainty guard isolated, and P4 searched with the file path removed

Status: **REGISTERED 2026-11-16, written before any code of F-717 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-717 was written. One smoke run was made before this text on seeds
that no check below uses (disclosed below); it ran the unchanged driver of ADR 0622 from a scratch test file that was moved out
of the worktree after the run and never committed (`w49i-work/f717-scratch/zz_f717_smoke.rs`, log `smoke.log`). Results go in
the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-717 (deps F-699), proposed by [ADR 0622](0622-f699-fidelity-management-screen-confirm-correct.md) (F-699,
"Remaining questions" 1 and 3, and its follow-up entry). Number 0642 given by the lead. Branch `f717-work` from d40cd665 (main
with F-699 merged), worktree `w49i-work/f717`, scratch `w49i-work/f717-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number); each registered run made once; existing tests not edited
(the one exception, P4's set-up, is registered below as the brief directs); defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code; clean-room. The
roadmap files are not edited. Every choice below that the maintainer would normally be asked about is taken on the maintainer's
standing instruction of 2026-11-11 (take the recommendation) and is marked "(standing instruction)".

The binding acceptance:

> A registered check on a synthetic noise source on which the one-standard-error rule alone would trust a correction shows the correction untrusted only because the discrepancy is not above twice the stated uncertainty (and trusted with that guard off), the mutation "stated uncertainty ignored" fails it, P4 searches its messages with the file path removed, and every ADR 0622 check keeps its result. Every limit not stated here is registered in the ADR before the run.

## What was read

`AGENT_RULES.md`; the brief; ADR 0622 in full (stub, maintainer decisions, amendments 1 and 2, addendum 1 with the M5 diagnosis
and remaining questions 1 and 3); `crates/rcfd-api/src/design/fidelity.rs` (`TrustRule`, `fit_metric`, `rung_residuals`,
`MeasuredTable::read` and its messages); `crates/rcfd-api/tests/f699_fidelity.rs` (helpers `tmp`, `s1_low`, `fnv_bits`,
`Noisy`, `f_only_spec`, `same_screening`, `low_lines`, `s7_table`, and the tests `t1_nc2_noise_is_not_corrected` and
`p4_table_errors_name_row_and_column_never_a_value`); ADR 0572 and 0638 for the form.

## The two defects

**D1. The uncertainty guard is not isolated (M5 of ADR 0622 survived).** The trust rule of ADR 0622 (amendment 1, "Trust")
applies a fitted rung only when (t1) it is not R0, (t2) its leave-one-out RMSE is at most `f_trust = 0.8` times the uncorrected
RMSE `rmse0 = sqrt(mean((yH - yL)^2))`, and (t3) `rmse0 > k_u u_rms` with `k_u = 2`, `u_rms` the rms of the high source's
stated standard uncertainty. On S4 (high = low + 0.05 z, stated 0.05) the one-standard-error rule already chooses R0 in every
round, so t3 never decides and `u_rms = 0` (M5) changes no tested outcome. A check that isolates t3 needs a source on which t1
and t2 hold and t3 alone fails.

**D2. P4 depends on the process id.** P4 searches each error message of `MeasuredTable::read` for every cell text of at least 3
characters of the table. Every message begins `measured table <path>:` with the path of the test's temporary directory
`<temp>/rcfd_f699_p4_<pid>/...`. When the process id contains a cell text (for example a `point` cell `168` in `41688`), P4
reports a leak that is not one (ADR 0622 addendum 1 under M7, M9, M10; ADR 0624's R2 run).

## Design (test only; no library change)

No library file changes: the guard exists and is correct by inspection (`fit_metric`, the `ok_trust` closure); what is missing is
a check that it decides. `crates/rcfd-api/src/` stays byte-identical to d40cd665, so every default and every ADR 0622 result keeps
its bits by construction; the R checks below confirm it by running them.

### S8, the offset-noise source (for D1)

Derivation. Let the high source be `yH = yL + b + s z`, `z` standard normal, independent of `x`, with stated standard uncertainty
`u`. For n pairs, `rmse0^2 = mean((b + s z)^2) -> b^2 + s^2`; the additive rung R1add has leave-one-out residuals with
`mean(e^2) -> s^2 n / (n - 1)`; R0's leave-one-out residuals are `b + s z` themselves. So:

* t1 (one-standard-error rule picks a rung other than R0): R1add's MSE `~ s^2` is far below R0's `~ b^2 + s^2` when `b >> s`,
  further than one standard error of the squared residuals (`~ sqrt(2) s^2 / sqrt(n)` for normal residuals).
* t2 holds when `s sqrt(n / (n - 1)) <= 0.8 sqrt(b^2 + s^2)`.
* t3 fails when `sqrt(b^2 + s^2) <= 2 u`; with `sqrt(b^2 + s^2) > u` the factor 2 itself decides (a guard with factor 1 would
  trust).

Chosen (from this derivation, before the smoke): `u = 0.05` (as S4), `b = 0.075 = 1.5 u`, `s = 0.01 = u / 5`. Then
`rmse0 -> 0.0757 = 1.51 u` (inside `(u, 2u]` with margins 0.51 u and 0.49 u), and R1add's leave-one-out RMSE over rmse0 `->
0.13` at n = 6 (limit 0.8). This is a source whose actual scatter is smaller than its stated uncertainty and whose offset lies
within twice that uncertainty: the case the guard exists for (a stated rig uncertainty that covers the disagreement), and
"structure-free" apart from the offset. Wording: the acceptance says "noise source"; S8 is noise plus a constant offset below
twice the stated uncertainty, as ADR 0622's remaining question 1 proposed (standing instruction).

S8 in the test: low S1's model (`analytic.s1`), metric `f` only (`f_only_spec`), objective `f` Min, candidates `Lhs { 300, seed
717_001 }`, 4 rounds of 6 (2 explore), default trust rule, workers 1. High `synthetic.s8` version `1`, tier analytic, level
`offset-noisy`: `f_H = f + 0.075 + 0.01 z`, `z` by Box-Muller from `Rng::new(717_002 ^ fnv_bits(x))` exactly as S4's source does;
stated uncertainty of `f` 0.05.

### P4c, P4 with the file path removed (for D2)

The old test `p4_table_errors_name_row_and_column_never_a_value` stays in the file with its body unchanged and gets the attribute
`#[ignore = "superseded set-up record (ADR 0642): ..."]`; this is the one edit of an existing test, registered here as the change
of P4's set-up, as the brief directs. Its old form: the three refusals (malformed cell `12.5x` in row 4, column `dp_static_Pa`;
header without `eta_s`; header without `variant`), each message must contain the wanted texts, and no cell text of at least 3
characters and not `12.5x` may occur anywhere in the message, path included.

The new test `p4c_table_errors_with_the_path_removed` (same file, so it shares S7's table writer) keeps every part of the old
check and changes only what is searched:

1. Same table, same three refusals, same wanted texts.
2. New, stronger: each message must contain the full path text `p.display()` of the table it refused (the error is located).
3. The search for cell texts (at least 3 characters) and for `12.5x` runs on the message with every occurrence of that path text
   removed.
4. The temporary directory is chosen so that the defect of the old form occurs on every run, not only for some process ids: its
   tag contains the cell text `158` (the `point` cell of row `v158`): `tmp("p4c_158")`, so the directory is
   `<temp>/rcfd_f699_p4c_158_<pid>`. Control PC: at least one of the three unstripped messages contains a cell text (the old form
   would fail on this set-up), while the stripped messages contain none.

Why not a path without digits: the system temporary directory is outside the test's control; removing the known path text is
exact and independent of it (standing instruction, the brief's first option).

## Checks (limits fixed now)

New tests appended to `crates/rcfd-api/tests/f699_fidelity.rs`, release profile, fixed clock as the file's other tests.
`g_uncertainty_guard_decides` holds G1 to G3.

| Id | Clause | What | Limit |
|---|---|---|---|
| G1 | 1 | S8 with the default rule, every one of the 4 rounds: (a) chosen rung not R0; (b) `sqrt(MSE_chosen) <= 0.8 rmse0` and `sqrt(MSE_R1add) <= 0.8 rmse0` (t2 holds, for the rung and for the constant); (c) `rmse0 <= 2 u_rms` (t3 fails); (d) `trusted` false and `constant_trusted` false; (e) `why` begins `the discrepancy` and contains `is not above 2 times the stated uncertainty`; (f) picks, prediction bits and the 300 low entry lines equal to the run with `correct = false` | all exact, every round |
| G2 | 1 | S8 with `k_u = 0`, every other rule setting default: every round the same rung, the same `mse` array and `rmse0` (bits) as G1's, `trusted` true and `constant_trusted` true | exact, every round |
| G3 | 1 | S8: in every round of G1, `rmse0 > u_rms` (so a guard of factor 1 would trust); the run with `k_u = 1`: `trusted` true in every round | `1 < rmse0 / u_rms <= 2` every round; exact |
| P4c | 3 | as designed above: three refusals; wanted texts; each message contains its path text; stripped messages contain no cell text of >= 3 characters and not `12.5x` | exact; 0 found |
| PC | 3 | the unstripped messages of P4c | at least 1 cell text found (control: the old form fails on this set-up) |
| R1 | 4 | `cargo test -p rcfd-api --release --test f699_fidelity` (the fast tests: ADR 0622's 16 run tests with P4 now ignored, plus the new tests) | every test passes; ADR 0622's 15 remaining fast tests pass |
| R2 | 4 | ADR 0622's heavy record once: `--ignored --exact b_reduced_order_fan_heavy` | passes; the printed B1 ratios, B2 gap, NC1 gap, B4 ratio and B5 counts equal ADR 0622 addendum 1's to the digits printed there (0.359, 0.115, 0.0000, 0.0579, 0.063; 2000, 24, 22, 22, 4) |
| R3 | 4 | `git diff d40cd665 -- crates/rcfd-api/src crates/rcfd-mcp` | empty |
| R4 | 4 | `git diff d40cd665 -- crates/rcfd-api/tests/f699_fidelity.rs`: the only change above the appended tests is the one `#[ignore = ...]` line on the old P4 | exact |
| R5 | — | `cargo test -p rcfd-api --release --lib design::fidelity`; `cargo fmt -p rcfd-api --check`; `cargo clippy -p rcfd-api --all-targets -- -D warnings` | pass; clean; clean |

The library is unchanged, so `baseline_hashes.rs` is not run for this item (no shared solver file is touched; R3 shows it).

## Smoke run (disclosed, before this text)

`zz_f717_smoke.rs`: S8 as above with 20 seed pairs (candidates `Lhs { 300, seed 717_100 + k }`, noise seed `717_200 + k`, k =
0..19, none used by a check), `k_u` 2, 1 and 0, 4 rounds each (80 rounds per `k_u`). Measured: rung R1add in 79 rounds and R2 in
1; with `k_u = 2` untrusted in all 80 (rung and constant), with `k_u` 1 and 0 trusted in all 80; `rmse0 / u_rms` between 1.332
and 1.687; leave-one-out RMSE of the chosen rung over rmse0 at most 0.219 (limit 0.8). So the registered limits have margins of
at least 0.33 u_rms on each side of `(u, 2u]` and a factor 3.6 on t2 at the smoke's worst seed.

## Negative controls and mutations (registered before the run)

Controls: G2 (guard off, trusted) is the control of G1; G3's `k_u = 1` run is the control of the factor 2; PC is the control of
P4c.

Mutations, each on a scratch copy (`git archive` of the code commit into `f717-scratch/mcopy`, own target directory), one at a
time, running `cargo test -p rcfd-api --release --test f699_fidelity` (fast tests):

| Id | Mutation (library, `design/fidelity.rs`) | Expected to fail |
|---|---|---|
| M5 | stated uncertainty ignored: `u_rms` = 0 in `fit_metric` | G1 |
| M5b | condition t3 removed from `ok_trust` | G1 |
| M13 | factor dropped: t3 compares `rmse0 <= u_rms` | G1 (G3's bound `rmse0 > u_rms`) |
| M14 | t3 inverted: refuse when `rmse0 > k_u u_rms` | G1 or G2, and A1 |
| MP4a | the not-a-number message names the cell: `... not a number ('<cell text>')` | P4c (`12.5x`) |
| MP4b | the not-a-number message names a measured value of the same row: `... (row value <cell of the last mapped column>)` | P4c (the stripped search) |
| MP4c | the message without the path (`at` = `measured table`) | P4c (the located assertion) |

The unmutated copy is run first as the control of the mutation runs.

## Decision rule

* **done:** G1 to G3, P4c, PC and R1 to R5 met, and every mutation fails a check.
* **partial:** G and P4c met, R met, and a mutation survives; or a G limit missed (an ignored `*_known_miss` record).
* **todo** (the lead does not take the change): an R check missed, or P4c missed.

## Limits (not covered)

* S8 isolates t3 on one synthetic source; it does not tell whether `k_u = 2` is the right factor for a real rig or a CFD tier
  (a question of the source's stated uncertainty, which the driver takes on trust, ADR 0622 risk 10).
* S8's offset is constant; a structured discrepancy below twice the stated uncertainty (for example a slope) is guarded by the
  same comparison of `rmse0` but is not checked separately.
* P4c checks the three messages that P4 checked; the other `MeasuredTable::read` messages (field count, empty variant, not
  finite, unreadable file) are not searched, as before.
* The Forrester sign question and the public low entries (ADR 0622 remaining questions 2 and 4) are not part of this item.

## Addenda

Results go here; nothing above is changed after a run.
