# ADR 0453: F-534 registration: rainflow residue guard and the Gerber compressive-mean message

Status: **REGISTERED 2026-11-14, written before any code of F-534 and before any run of its checks**. Not yet accepted by the
maintainer. Nothing was compiled or run for this item; the files under "What was read" were read. No arithmetic was done
beforehand. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Roadmap item F-534 (deps F-413), size S, follow-up of [ADR 0449](0449-f413-registration-stress-life-rainflow-miner.md), its
addendum questions 2 and 3. Worktree `w49i-work/f534` (branch `f534-work` from main 0ca10e83), scratch `w49i-work/f534-scratch`.
House rules: `w49i-work/AGENT_RULES.md`. Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation):
ADR 0449 addendum question 2 option (a) and question 3 option (a).

## What was read

ADR 0449 (closed forms (i), checks C3, Q1, Q3, mutation M5, addendum 1); `crates/rcfd-fem/src/fatigue.rs`
(`equivalent_amplitude`, `four_point`, `rainflow_periodic`); `crates/rcfd-fem/tests/sa_fatigue.rs` (C3 and Q1 and the helper
`refused_with`, which needs the key and a substring of the message); `crates/rcfd-fem/src/error.rs` (there is no internal-error
variant of `FemError`).

## Design

**Residue guard.** ADR 0449 closed forms (i): the sequence of `rainflow_periodic` is rotated to start at a turning point of
largest modulus `M` and closed by repeating it, so after the four-point pass the residue is `[M, v, M]` (three points). After the
pass, `rainflow_periodic` returns `Err(FemError::InvalidValue { key: "rainflow", reason: "internal error: ..." })` when the residue
has more than three points, naming its length. A shorter residue (fewer than three) is left as it is. No new variant is added to
the `#[non_exhaustive]` `FemError`. The reason: with a longer residue the current pairing counts every second window of three
as a cycle, right only by accident (ADR 0449 addendum, M5).

**Gerber message.** Only the refusal of a negative mean (Gerber with compressive credit, `sigma_m <= -sigma_u`) changes, to
`mean stress {sm:e} at or above the strength {strength:e} in modulus ({name}, with compressive credit): no equivalent amplitude`.
It still contains the text "at or above the strength" that C3 checks, so C3 passes unedited. The text of every refusal of a
positive mean is byte-identical to the present text. Numbers returned are unchanged.

## Checks

New tests in the new file `crates/rcfd-fem/tests/sa_fatigue_f534.rs`; no existing test is edited.

| Id | Clause | Check | Limit | Estimate |
|---|---|---|---|---|
| P1 | guard not triggered | `rainflow_periodic` of `[S, -S] x n`, `n` = 1, 7, 1000, `S = 120e6`: Ok, `n` cycles, every range `== 2S`, mean `== 0`, count 1, residue empty (the Q1 check, restated so that an error fails it) | exact | exact |
| P2 | negative control | 200 random periodic histories (12 to 60 points, seed `0xF534_0453`, values in +-100 MPa) and the 16-sample sine of 50 periods: all Ok with an empty residue | as stated | as stated (closed forms (i); R1 of ADR 0449 ran 200 such) |
| P3 | all of ADR 0449 | the existing tests of `sa_fatigue.rs` pass unedited (`cargo test -p rcfd-fem --release --test sa_fatigue`, F and K included) | as stated | as stated |
| G1 | Gerber message | Gerber with credit at `-sigma_u` and at `-1.1 sigma_u`: refused, key `"mean stress"`, message contains "in modulus" and "at or above the strength" | as stated | as stated |
| G2 | positive means unchanged | Goodman at `sigma_u`, Gerber at `sigma_u`, Soderberg at `sigma_y`: refused with the exact present text (formatted in the test from the same pattern) and no "in modulus" | `==` | as stated |
| G3 | negative control | Goodman and Soderberg with credit at `-sigma_u` and `-sigma_y`: accepted (they are not refused for a negative mean); Gerber with credit at `-0.999 sigma_u` accepted | as stated | as stated |

The guard itself cannot be reached with the registered rule (that is why it is an internal error), so it has no direct
test; it is checked through the mutations below.

## Negative controls and planned mutations (scratch copy)

| Id | Where | Change | Registered catchers |
|---|---|---|---|
| M5b | rainflow closing | `<` instead of `<=` in `four_point` (M5 of ADR 0449) | P1 (error for `n` = 7, 1000), P2 |
| M13 | guard | threshold `>= 3` instead of `> 3` | P1, P2 |
| M14 | guard | removed | not caught by design (unreachable); with M5b together: P1 again as `n` cycles come out by accident, so this pair is recorded as the surviving mutant that the guard exists to expose: expected caught by none |
| M15 | Gerber message | "in modulus" dropped | G1 |
| M16 | Gerber message | "in modulus" also for positive means | G2 |

M14 alone is recorded as an equivalent mutant (unreachable code), and M5b with the guard removed shows what the guard fixed.

## Decision rule

**done** if every check P1 to P3 and G1 to G3 is met in its single run without an amendment and M5b, M13, M15 and M16 are each
caught by at least one registered catcher (M14 as stated). **partial** if a check is missed, a mutation other than M14 is not
caught, or a check is met only after an amendment made after its run. **todo** if the code does not build.

## Limits

No wiring into a case, CLI or API. The internal error uses `InvalidValue` with key `"rainflow"` because `FemError` has no
internal-error variant (adding one is out of scope). The guard is untested by a direct input because none exists.

## Addendum 1: results (2026-11-14)

All checks were run once, after the code commit, release profile (`cargo test -p rcfd-fem --release --test sa_fatigue_f534`
and `--test sa_fatigue`). Code: `crates/rcfd-fem/src/fatigue.rs` (guard in `rainflow_periodic`, message in
`equivalent_amplitude`), tests in the new file `crates/rcfd-fem/tests/sa_fatigue_f534.rs`.

| Check | Limit | Measured | Met |
|---|---|---|---|
| P1 `[S, -S] x n`, n = 1, 7, 1000 | exact | Ok, n cycles of 2S, mean 0, residue empty | yes |
| P2 sine of 50 periods and 200 random periodic histories | all Ok, empty residue | as stated | yes |
| P3 existing `sa_fatigue` (9 tests, unedited) | pass | 9 passed | yes |
| G1 Gerber with credit at `-sigma_u`, `-1.1 sigma_u` | key, "in modulus", "at or above the strength" | message `mean stress -6e8 at or above the strength 6e8 in modulus (Gerber, with compressive credit): no equivalent amplitude` | yes |
| G2 positive means unchanged | exact present text, no "in modulus" | as stated for Goodman, Gerber, Soderberg | yes |
| G3 negative control | accepted | Goodman, Soderberg with credit at `-sigma_u`, `-sigma_y`; Gerber at `-0.999 sigma_u` | yes |

### Mutations

Run in the worktree on `fatigue.rs` and restored with `git checkout` after each (not on a separate copy; the log is
`w49i-work/f534-scratch/mut.log`).

| Id | Caught by |
|---|---|
| M5b (`<` in `four_point`) | P1 (error with a residue of 15 points) and the existing Q1 of ADR 0449, which now fails where in ADR 0449 it survived M5 |
| M13 (guard `>= 3`) | P1 (rerun on the F-534 test alone to see it), existing Q1 and B2 |
| M14 (guard removed) | nothing, as registered (unreachable code; equivalent mutant) |
| M15 ("in modulus" dropped) | G1 |
| M16 ("in modulus" for positive means) | G2 |

### Deviations and what was not run

- No registered set-up was changed. The registered catcher P2 of M5b and M13 was not individually observed (P1 failed first in
  the same test function, which stops at the first assertion).
- The mutations were run in the worktree, not on a copy.
- Not run: the whole workspace suite (the lead's gate); `baseline_hashes` (no shared solver file touched). `cargo fmt -p rcfd-fem
  --check` clean; `cargo clippy -p rcfd-fem --all-targets -- -D warnings` clean.
- Wording: the registration says the new message contains "at or above the strength in modulus"; the text is "at or above the
  strength S in modulus", which keeps the C3 substring "at or above the strength".

### Decision by the registered rule

**done**: every check was met in its single run without an amendment, and M5b, M13, M15 and M16 were each caught (M14 not caught,
as registered). The clauses of the acceptance (guard, M5-type mutation caught by the constant-amplitude check, Gerber message in
modulus, ADR 0449 checks unedited) are all met.

### Remaining questions

1. `FemError` has no internal-error variant, so the guard uses `InvalidValue` with key `"rainflow"`. Options: (a) keep; (b) add a
   variant. Recommendation (a): adding a variant touches a shared `#[non_exhaustive]` enum for one unreachable guard.

### For the lead: roadmap entries

Proposed status for F-534: done.

> ADR 0453, 2026-11-14: rainflow_periodic returns an internal error (InvalidValue, key "rainflow") when the residue of the closed sequence has more than three points, and the mutation that closes loops with a strict inequality is now caught by the constant-amplitude check (error for n = 7 and 1000; it escaped that check in ADR 0449); the Gerber refusal of a compressive mean at or beyond the strength says "in modulus" while every positive-mean message is byte-identical; all 9 tests of ADR 0449 pass unedited; mutations of the guard threshold and of both message forms caught; the guard itself is unreachable by design (equivalent mutant recorded).
