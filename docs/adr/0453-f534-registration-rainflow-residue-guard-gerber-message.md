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
