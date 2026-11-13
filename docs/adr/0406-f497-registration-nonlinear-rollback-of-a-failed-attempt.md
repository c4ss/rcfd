# ADR 0406: F-497 registration: roll-back of a failed attempt of the nonlinear driver as a registered check (bit-equality of the one-halving solve with the two-step solve)

Status: **REGISTERED 2026-11-13, written before any code of F-497 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-497 was written, compiled or run: the files listed under "What was read" were read, no
arithmetic beyond the counting below was done. Results go in the addendum at the end; nothing above the addendum is changed after a
run.

Parent: [ADR 0404](0404-f397-registration-geometric-nonlinearity-total-lagrangian-newton.md) (F-397), mutation M7 ("a failed
attempt not rolled back") and its remaining question 2, option (a), taken on the maintainer's standing instruction of 2026-11-11
(take the recommendation). Roadmap item F-497 (deps F-397). Size S. Number 0406 given by the lead. Branch
`f497-nonlinear-rollback-check`, worktree `w49i-work/f497`, scratch `w49i-work/f497-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs; each registered run made once; defaults
bit-identical (this item is test code only, no library line is changed); no edit of `baseline_hashes.rs`, of the hash tables or of
a golden file; no new dependency, no unsafe code; clean-room.

## What F-497 asks

The acceptance of the roadmap entry, binding: "Every limit not stated here is registered in the ADR before the run. The one-step
solve of ADR 0404's load-controlled Hex8 bar with four iterations allowed (one halving) equals the two-step solve bit for bit, and
the mutation of ADR 0404 M7 (a failed attempt not rolled back) is caught by that check; every check of ADR 0404 keeps its value."

Clauses: (f1) bit equality of the one-halving solve with the two-step solve; (f2) M7 caught by that check; (f3) every check of ADR
0404 keeps its value (the existing tests are run unchanged).

## What was read (no run)

ADR 0404 (D5 step 5 and the state paragraph; C3; M7 and its diagnosis; the addendum, question 2);
`crates/rcfd-fem/src/nonlinear.rs` (`Problem::attempt` starts every attempt from `u_c` and `states_c`, the committed values, and
returns the iterate only on success; `solve` replaces `u`, `states`, `lambda` only in the `Ok` arm and halves `dl` in the `Err`
arm; `NewtonOptions::max_halvings` is a public field, `max_iterations` has a builder); `crates/rcfd-fem/tests/si_nonlinear.rs`
(the bar set-up `bar_load_control`, the C3 code, `Checks`).

## Design

### D1. Why bit equality is the observable

The St. Venant-Kirchhoff material is hyperelastic: the converged state at a load factor does not depend on the path. A driver that
starts the half step from the last iterate of the failed full step still converges to the same state within the residual
tolerance, so a limit on the displacement deviation (C3: `1e-12`, measured `2.8e-16` under M7) cannot see the defect. What the
roll-back guarantees exactly is that the second attempt of the one-step solve (load factor 0 to 0.5, after one halving) starts
from the same bits as the first step of the two-step solve (0 to 0.5), and the third (0.5 to 1) from the same bits as its second
step. Same inputs and the same code give the same bits (the assembly is deterministic, C6 of ADR 0404). Hence every recorded
number of the two solves is bit-equal: the displacement, the reactions, the residual and energy histories of both steps, the
iteration count. Under M7 the half step starts from a different iterate, so its residual history (`r_0`, `r_1`, ...) differs in
value and in length, whatever the material. The residual history is therefore the path-sensitive signature, and the check
compares it too.

The load factors: the one-step solve has `dl = 1`, fails, `dl = 0.5`, `lt = 0 + 0.5 = 0.5`, then `lt = 0.5 + 0.5` snaps to the
target 1; the two-step solve has `dl = 0.5`, targets 0.5 and 1: the same factors, the same arithmetic.

### D2. Where it goes

A new test file `crates/rcfd-fem/tests/si_nonlinear_rollback.rs` with its own copy of the Hex8 bar set-up of ADR 0404 (bar 1 x
0.5 x 0.5, 2 x 1 x 1 Hex8, `E = 2e11`, `nu = 0`, fixings at the minimum faces, traction `p = E 1.2 (1.2^2 - 1) / 2` on `xmax`).
`si_nonlinear.rs` and the library are not edited. No library code changes, so no baseline hash run is required.

## Registered checks

| Id | Clause | Measure | Limit | Estimate |
| --- | --- | --- | --- | --- |
| R1 | f1 | one step with `max_iterations = 4` against two steps with the default 20: the number of displacement entries (all dofs) whose bit pattern differs | `== 0` | 0 (ADR 0404 C3 printed bit-equality, deviation 0) |
| R2 | f1 | the number of reaction entries whose bit pattern differs | `== 0` | 0 |
| R3 | f1 | the records: the one-step solve has 2 records, both with `halvings == 1`, the load factors equal bit for bit to the two-step ones (`0.5`, `1.0`); the residual and the energy histories of each record equal in length and bit for bit | all as stated | as stated; ADR 0404 gives 4 iterations per step for the load-controlled bar, `r_4` of the full attempt fails with margin 15, the half steps pass with margin 170 |
| R4 | f1 | `SolverStats` iterations of the two solves equal; the final `states` equal (`==`) | equal | equal |
| N1 | control | the one-step solve with `max_iterations = 4` and `max_halvings = 0` is `Err(FemError::Solver { solver: "newton", .. })` naming `"load step 1 of 1"` and `"0 halvings"` (the first attempt really fails, so R1 to R3 exercise a roll-back) | as stated | as stated (ADR 0404 arithmetic: `r_4` above tolerance) |
| N2 | control | the one-step solve with the default 20 iterations (no failed attempt) has 1 record, the two-step solve 2; the records of the first have `halvings == 0` | `1 != 2`, `0` | as stated |
| S1 | f3 | `CARGO_BUILD_JOBS=4 cargo test -p rcfd-fem --release --test si_nonlinear` (the existing tests of ADR 0404, unchanged) and the new file | all pass as before; ignored records stay ignored | as in ADR 0404 |

Time: the bar is 2 Hex8 elements; each solve takes milliseconds, the file runs in seconds in release.

### M. Planned mutations (on a scratch copy of the code commit, after the registered run, each alone)

| Id | Mutation | Expected to be caught by |
| --- | --- | --- |
| M7 | ADR 0404 M7: a failed attempt is not rolled back: the iterate of a failed attempt is kept (a side cell in `Problem`) and the next attempt starts from it instead of from `u_c` | R1 or R3 (the half step starts from another iterate: its residual history differs); ADR 0404 C3 does not see it (2.8e-16 against 1e-12) |
| M7b | the iterate of a failed attempt is committed into `u` in the `Err` arm of `solve` while `lambda` stays | R1, R3 |
| M4 | no halving (ADR 0404 M4): a failed attempt is an error at once | N1 stays met; R1 to R4 (the solve returns an error, the test fails at the unwrap) |

M7 must be caught by the new check: that is clause f2. If the registered check does not catch M7 the result is a miss (decision
rule).

## Decision rule

- **done**: R1 to R4, N1, N2 and S1 met, and each planned mutation caught by at least one of R1 to R4.
- **partial**: R1 to R4 and S1 met but M7 or another planned mutation not caught (diagnosed in the addendum).
- **todo**: R1 to R4 not met without a diagnosed cause.

## Limits (what is not covered)

- One material (St. Venant-Kirchhoff, no history) and one element kind (Hex8). The roll-back of a state with history
  (`ElementState` updated by `Material::stress`, which no material of the crate does today) is not exercised: the trial states
  equal the committed ones. A path-dependent material would need its own check when it exists.
- Roll-back after an inversion failure and after a refused pivot is the same code path (the `Err` arm of `solve`) but is not run
  with a bit comparison here.
- Only one halving is exercised (the acceptance); several halvings are covered by A7 of ADR 0404 (5 halvings, then the error).

## Open points taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. A new test file instead of an added assertion in `si_nonlinear.rs`: the rules forbid editing existing tests, and C3 keeps its
   registered limit.
2. The residual and energy histories are compared as well as the displacement: they are what makes the check sensitive for a
   hyperelastic material (D1).

## Amendment 1 (2026-11-13, written after the first registered run and before the corrected run)

The first registered run (test `r1_to_r4_n1_n2_rollback_of_a_failed_attempt`) met R1, R2, R4, N1 and N2 and missed R3. Cause: a
defect of the test, not of the driver: R3 compared the user-step number of each record of the one-step solve (always 1) with the
user-step number of the record of the two-step solve at the same position (1, 2), so `b.step == a.step` is false for the second
record by construction. Printed in that run: 2 and 2 records, `halvings [1, 1]`, load factors 0.5 and 1, residual histories of 5
entries and energy histories of 4 entries in both solves. The limits are not changed: R3 is as registered ("the load factors equal
bit for bit, the residual and energy histories equal in length and bit for bit"); only the clause on the step number is stated
correctly: the one-step records have `step == 1` and the two-step record number `k` has `step == k`. The old test stays as an
ignored record; the corrected test is `r1_to_r4_n1_n2_corrected`, run once. Its R1 to R4, N1, N2 are the registered ones.

## Addendum 1: results (2026-11-13)

Code commit: the test file `crates/rcfd-fem/tests/si_nonlinear_rollback.rs` (test code only; no library line, `si_nonlinear.rs`,
hash table or golden file changed). Release profile, `CARGO_BUILD_JOBS=4`.

### Checks

First registered run (`r1_to_r4_n1_n2_rollback_of_a_failed_attempt`, now an ignored record): R1, R2, R4, N1, N2 met, R3 missed by
the defect of the test described in amendment 1. The corrected test `r1_to_r4_n1_n2_corrected` was run once for the table.

| Id | Limit | Measured | Met |
| --- | --- | --- | --- |
| R1 displacement entries differing in bits | `== 0` | 0 | yes |
| R2 reaction entries differing in bits | `== 0` | 0 | yes |
| R3 records | 2 records, `halvings == 1`, load factors and residual and energy histories bit-equal | 2 and 2 records, halvings `[1, 1]`, load factors 0.5 and 1, residual histories 5 entries, energy histories 4 entries, all bit-equal | yes (first run: no, defect of the test) |
| R4 iterations, states | equal | 8 and 8, states equal | yes |
| N1 no halving | `Err(Solver "newton")`, "load step 1 of 1", "0 halvings" | as stated (`r` 1.47e-9 after 4 iterations, energy ratio 7.26e-9) | as expected |
| N2 20 iterations | 1 record against 2, `halvings == 0` | 1 against 2 | as expected |
| S1 existing tests | `si_nonlinear`: all pass | 7 passed, 0 failed, 1 ignored (as in ADR 0404); `si_nonlinear_rollback`: 1 passed, 1 ignored | yes |

Time: 0.01 s per test in release.

### Mutations (copy `f497-scratch/copy` from `git archive` of the code commit, own target directory, script `mutate.py`, log `mutations.log`)

| Id | Result in `r1_to_r4_n1_n2_corrected` | Result in ADR 0404 `a7_c3` |
| --- | --- | --- |
| M7 (iterate of a failed attempt carried into the next attempt) | caught: R1 19 displacement entries differ, R2 16 reaction entries differ, R3 (the residual histories differ in value) | not caught: C3 deviation 2.8e-16, printed bit-equal false (as in ADR 0404) |
| M7b (iterate of a failed attempt committed in the `Err` arm, `lambda` kept) | caught: R1 19, R2 16, R3 | not caught: C3 2.8e-16, bit-equal false |
| M4 (no halving) | caught: the solve returns the error, the test fails at the unwrap (R1 to R4 not reached) | caught: A7 non-convergence ("after 0 halvings" instead of 5) |

M7 is caught by the new check: clause f2 met.

### Deviations

- The first run missed R3 by a defect of the test (amendment 1); no limit changed; the old test is an ignored record.
- `cargo fmt` reformatted the test file after the mutation runs (layout only, no logic change); the check was re-run: 1 passed.

### What was not run

Anything beyond the registered scope (see "Limits"): materials with history, other element kinds, roll-back after inversion or
pivot refusal. The whole-workspace tests and `baseline_hashes` were not run (no shared solver file changed).

### Decision by the registered rule

**done**: R1 to R4, N1, N2, S1 met, and M7, M7b, M4 each caught by at least one check. The decision is formal: the first run of R3
missed through a defect of the test, which the rule of the house treats as an amendment, not a miss of the driver.

### Remaining questions

1. A material with history (plasticity) will make the roll-back observable in the result and not only in the bits. Options: (a)
   register a roll-back check with it when the first such material exists; (b) nothing now. Recommendation: (a), with that item.
2. Roll-back after an inversion failure and after a refused pivot (the same `Err` arm) is not compared bit for bit. Options: (a)
   a follow-up check with a bar that inverts once and recovers after a halving; (b) none, the arm is one code path. Recommendation:
   (b); the `Err` arm does not distinguish the cause.

## For the lead: roadmap entries

Proposed status for F-497: done.

> Nonlinear driver roll-back as a registered check. ADR 0406, 2026-11-13: new test file si_nonlinear_rollback.rs (test code only); the one-step solve of the load-controlled Hex8 bar of ADR 0404 with four iterations allowed (one halving) equals the two-step solve bit for bit (displacements, reactions, load factors, residual and energy histories, iteration count, states: 0 differing entries); ADR 0404 mutation M7 (a failed attempt not rolled back), and a second variant, are caught (19 displacement and 16 reaction entries differ), where C3 of ADR 0404 stays below its limit at 2.8e-16; the existing checks of ADR 0404 keep their values. Limits: one material without history, Hex8, one halving.
