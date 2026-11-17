# ADR 0705: F-769 registration: ADRs for the five feature groups that no ADR describes, and the index of them

Status: **REGISTERED 2026-11-17, written before the five group ADRs (0706 to 0710) and before any run of its checks.** Not yet accepted
by the maintainer. Roadmap item F-769 (deps F-760, [ADR 0685](0685-f760-registration-adr-text-for-the-25-unmapped-integration-test-binaries.md)
addendum 1, open question 1, option (b)). Number 0705 given by the lead for this registration and the index; 0706 to 0710 for the five
groups. Branch `f769-work` from 410900e2, worktree `w49i-work/f769`, scratch `w49i-work/f769-scratch`. No Rust file, golden, hash table or
script is edited. Results go in the addenda at the end; nothing above them is changed after a run.

These ADRs are **retrospective**. Every test file they describe was committed on 2026-10-13 or 2026-10-14 (or the F-105 commit of
2026-10-13), before this ADR and before most ADRs existed, and none had an ADR of its own. Each group ADR records what its files check and
which limits they carry, read from the code and from the roadmap notes of the items that delivered them. It is **not** a decision taken
before the tests; where a limit came from a measurement the roadmap note records, the ADR says so, and the ADR does not claim a derivation
the files do not contain. The choices in this item (one ADR per group, what counts as a limit) are taken on the maintainer's standing
instruction of 2026-11-11 (take the recommendation).

## What F-769 asks

> Every limit not stated here is registered in the ADR before the run. A short registration ADR per group states what the files
> `rcfd-assistant/tests/physics_workflow.rs`, `rcfd-assistant/tests/workflow_engine.rs`, `rcfd-turb/tests/channel_wall_function.rs`,
> `rcfd-cli/tests/physics_cases.rs`, `rcfd-api/tests/mesh_params.rs` and `rcfd-core/tests/table_expr.rs` check and which limits they carry,
> read from the code and the roadmap notes, with no Rust file edited; the index maps these files by a reading of that ADR (evidence E2,
> not only a word); a registered sample is read.

## What was read (before any edit)

* ADR 0685 (the plan, the five binaries it leaves to this item, its sample and its open question 1), ADR 0701 (rules H0 to H3 of
  `scripts/adr_test_index.py`, `--primary git`), ADR 0704 (second reader; it recommends that the notes of ADR 0067 and 0123 on the two
  `workflow.rs` files move into the ADRs this item writes).
* The six files of the acceptance in full, and the two `workflow.rs` files (`rcfd-assistant/tests/workflow.rs`, F-148, 9 tests;
  `rcfd-api/tests/workflow.rs`, F-120 and F-121, 5 tests) for their test names, headers and the asserts that carry a limit.
* The notes in `docs/roadmap.json` of F-040, F-042, F-043, F-060, F-090, F-105, F-120, F-121, F-147 and F-148; `docs/case-schema.md` at
  the passage on `nx`/`ny` and `lx`/`ly` of `cavity_2d` (wave 11); `git log --diff-filter=A` for the six files (adding commits `f71f6e2c`
  2026-10-13 for four, `40473ad5` 2026-10-13 for `table_expr`, `0704ccaf` 2026-10-14 for `mesh_params`).
* One run of the unchanged index (`python scripts/adr_test_index.py --primary git`, 46.6 s) saved as `f769-scratch/before.json`. It shows,
  for the files of this item: `table_expr.rs` (2 tests), `channel_wall_function.rs` (2), `physics_cases.rs` (7), `mesh_params.rs` (4) and
  `physics_workflow.rs` (5) have primary ADR **0685** with evidence E2, by rule H2 (ADR 0685 says no ADR describes them);
  `workflow_engine.rs` (1) and `rcfd-api/tests/workflow.rs` (5) have primary 0123 and `rcfd-assistant/tests/workflow.rs` (9) has 0067, all by
  rule H1 (the "Test files" notes of ADR 0685). **The estimates below were made after seeing this**, and the rule of the script (the lowest
  number wins inside a tier) was read before them.
* No test of the eight files was run for this item. The limits are read from the code, so the ADRs register what a passing run must show;
  they do not report a measurement of it (see Limits).

## Design

**Five group ADRs, one per feature group of ADR 0685's open question, each short.** Each has: a status line (registered 2026-11-17,
retrospective), what the files check, a table of limit rows with a stable id (`P1`, `W2`, ...), the test function and the source of the
limit (the assert in the file, or the roadmap note that records where the number came from), what the files do not check, and the
citation of the files as `crates/<crate>/tests/<file>.rs` (the form the index matches for evidence E2). A limit row has one of three
kinds: **numeric** (a tolerance or threshold in an assert), **exact** (an equality on a count, a name, a state, an exit code, a list) or
**structural** (a field or message must be present). Every `#[test]` function of the files is named in a row.

| ADR | group | files (crate-relative) | tests |
|---|---|---|---|
| 0706 | assistant setup helpers and workflow engine | `rcfd-assistant/tests/physics_workflow.rs` (5), `rcfd-assistant/tests/workflow_engine.rs` (1); and, as ADR 0704 recommends, `rcfd-assistant/tests/workflow.rs` (9, F-148) and `rcfd-api/tests/workflow.rs` (5, F-120, F-121) | 20 |
| 0707 | the F-040 wall-function channel | `rcfd-turb/tests/channel_wall_function.rs` | 2 |
| 0708 | the shipped turbulence and compressible examples through the binary | `rcfd-cli/tests/physics_cases.rs` | 7 |
| 0709 | the mesh generator parameters | `rcfd-api/tests/mesh_params.rs` | 4 |
| 0710 | tables called from expressions | `rcfd-core/tests/table_expr.rs` | 2 |

The two `workflow.rs` files are included in 0706 because their checks are the same feature (the assistant tools over the workflow engine,
the engine and the headless methods under it): `rcfd-assistant/tests/workflow.rs` checks the setup helpers that `physics_workflow.rs`
extends, and `rcfd-api/tests/workflow.rs` checks the engine that `workflow_engine.rs` drives through the assistant. The existing notes of
ADR 0067 and ADR 0123 are not edited (the text above their "Test files" sections stays, and the sections are ADR 0685's); 0706 says what
they do not describe.

**Limits fixed here, before the group ADRs are written** (the group ADRs register what each file carries; these are the checks of this
item on them):

| id | limit | estimate behind it |
|---|---|---|
| B1 | each of the 8 files is named by path (`crates/<crate>/tests/<file>.rs`) in its group ADR in the table above, with a table of limit rows | 8 files, 5 ADRs |
| B2 | each of the 35 `#[test]` functions of the 8 files (5 + 1 + 9 + 5 in 0706 as 20, 2, 7, 4, 2) is named in a row of its group ADR: **35 of 35** | counted with `grep -c '#\[test\]'`: 5, 1, 9, 5, 2, 7, 4, 2 = 35 |
| B3 | every numeric literal that stands in an `assert*` line of the six files of the acceptance and has a decimal point, an exponent, or at least two digits (a tolerance or threshold, not a 0 or 1) appears in the group ADR: **at least 95 %** of the distinct (file, literal) pairs, by a script run once after the ADRs are written | the ADRs are written from the code; the 5 % allowance is for literals that belong to a test input and not to a limit, which the script cannot tell apart |
| B4 | each group ADR says in its first section that the tests predate it and that it records what they check, not a decision taken before them (the word `predate`): **5 of 5** | the text of this ADR |
| B5 | index after the edit, `--primary git`: every test of the 8 files has its group ADR in `adrs` **with an E2 evidence entry** (not only E7): **35 of 35** tests; tests that lose an ADR: **0**; tests outside the 8 files whose `adrs` change: **at most 10** (a stem shared with another crate, or an ADR text that names a word of the index) | the group ADRs cite the files by `crates/.../tests/<file>.rs`, the form E2 matches; `workflow` is a stem of two crates, so the path form is needed |
| B6 | the `primary_adr` of the tests of the 8 files is **reported** next to B5 (a number, not a limit) | the estimate is that it does not change: 0685 (5 files), 0123 (2), 0067 (1) win inside their tiers by the lowest number, and a group ADR has a higher number. If it does not change, that is a finding about rule H of `adr_test_index.py` (F-785 changes H3 on another branch), not a miss of this item |
| B7 | a registered sample of the limit rows is read (below): **at least 9 of 10 agree** with the code and the note they cite | 5 ADRs x 2 rows |
| B8 | `git diff --stat 410900e2` shows files only in `docs/adr/` | the acceptance: no Rust file edited |
| C1 | `python -m unittest discover -s scripts` from the worktree: all pass | 93 run, 4 skipped at ADR 0685 |
| C2 | `cargo test -q --release -p rcfd-cli --test docs_site` passes (files under `docs/` changed), `CARGO_TARGET_DIR=w49i-work/f769-scratch/target` | passed at ADR 0685 |

**Registered sample (B7), before it is read.** A row is a numbered limit row of a group ADR (`ids` as written in the table of that ADR).
After the five ADRs are committed, the rows of each ADR are listed in the order they appear in its tables, and
`random.Random(769).sample(rows_of_that_ADR, 2)` is drawn for each of 0706, 0707, 0708, 0709, 0710 (the pool of 0706 includes the rows of
the two `workflow.rs` files; an ADR with fewer than two rows gives all of them). The 10 rows are read **against the file**: the test
function named in the row is opened, the assert or the roadmap-note sentence the row cites is found and compared with the row's number,
kind and source; the row **agrees** if the number (or the exact value or the structural field) equals the one in the code, the test
function is the one named, and the stated source says what the row says it says. It **disagrees** if any of these fails, and the
disagreement is reported with the line. I read my own text (there is no second reader), so a second reading such as ADR 0704's is left
open (Limits). The sample is read once, no row is replaced.

Decision rule: **done** if B1 to B5, B7, B8, C1 and C2 are met (B6 is reported); **partial** if B3 or B5 or B7 is missed, or a clause of
the acceptance is not met (the clause "the index maps these files by a reading of that ADR (evidence E2, not only a word)" is read as B5;
if B6 shows that the group ADR never becomes the primary, the item still says so and proposes the follow-up below).

## Negative controls and planned mutations

There is no code. N1: the index is run on the same script and the same inputs before (`before.json`) and after; the only inputs that
differ are the six new ADR files. N2: B3 is run once on the ADR text with one group ADR's literal list deleted in a scratch copy, to show
that the script reports misses (a control of the check, not of the tests). N3: the 10 sample rows are chosen by a fixed seed and the draw
code is saved in the scratch folder, so the draw can be repeated.

## Limits (what is not covered)

* The eight files are **not run** by this item: the ADRs register what a passing run shows. Whether each limit holds today is the daily
  gate's information (ADR 0653), not this item's. No limit is "met" or "missed" here in the sense of F-xxx registrations; B-rows measure the
  text and the index.
* Limits that came from a roadmap note (the 2.1 %, 2.3 %, 2.4 % deviations of F-040, 18.17 against 17.4 of F-043) are recorded with that
  source; the ADR does not re-derive them.
* The sample is 10 of the rows, read by the author of the rows. It is not a second reading.
* The primary ADR of a test is decided by the script on this base. It may still be 0685, 0123 or 0067 after this item (B6).
* The notes in ADR 0067 and ADR 0123 on the two `workflow.rs` files are not moved or edited.
