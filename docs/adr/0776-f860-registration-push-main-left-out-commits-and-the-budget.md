# ADR 0776: F-860 registration: push_main.py, the left-out run names its commits, the ADR's tests count in the budget, changed-file binaries first

Status: **REGISTERED 2026-11-23, written before any code of F-860 and before any run of its checks.** Before this text no
script or test code of F-860 was written and no file of the repository was changed. What was done: reading (below), two
dry runs of the unchanged `scripts/push_main.py` (`--merge 1b939c19`, the newest merge on main, and `--merge 9a0afcec`,
the merge of ADR 0748; both exit 0; a dry run runs no check), and one counting script over the index and the recorded
times (`arith.py` in the scratch folder; its numbers are below). Results go in addenda at the end; nothing above them is
changed after a run.

It answers the brief `docs/adr/briefs/f860-push-main-left-out-and-budget.md` and follows `docs/process/agent-rules.md`,
version 5. Branch `f860-push-main-left-out-and-budget` from main a3d29aff, scratch
`w49i-work/f860-push-main-left-out-and-budget-scratch`.

## What is asked

Roadmap item F-860, the binding acceptance: `scripts/push_main.py`, with a unit test for each point that fails before the
change: (1) a left-out line records the commit of the checkout at the start and at the end of the left-out run, and says
when they differ; (2) the tests the index maps to the merge's ADRs are charged to the budget and never cut, and the dry
run prints their recorded minutes; (3) inside a touched crate, the test binaries named after a changed source file are
selected first, by a registered rule of naming. The ledger's columns, every existing line and what a run that passes at
the first attempt writes are unchanged. The three points are the lead's answers 2 to 4 to ADR 0752 (findings 10, 11 and
16 of F-851): option (b) for finding 10, option (b) for finding 11, option (a) for finding 16.

## What was read

`docs/process/agent-rules.md` (version 5, whole); the brief; `scripts/push_main.py` and `scripts/test_push_main.py`
(each whole); `docs/test-process.md` section 7.4 and the start of 7.5; the item F-860 of `docs/roadmap.json` (read with a
script: acceptance and note); ADR 0752 (registration, open questions, the lead's and the maintainer's answers);
`scripts/adr_test_index.py` (`adr_commands`); `docs/process/ledger.tsv` (header, every `leftout` line, the last four
lines, by a script); `scripts/process_report.py` (where it reads `leftout` and `left_out`); the form of a proposal file
(`docs/roadmap/proposals/0770-F-884.json`, `scripts/roadmap_proposals.py --help`).

## Readings (before any code)

* **The left-out run.** `do_left_out(root, path)` reads the job file `target/push_main/leftout-<commit>.json`, runs every
  command with `sh` in `root` (the lead's checkout) and appends one `leftout` line: commit (the merge commit, from the
  job), branch, items, ADRs, result, minutes, and the detail `ran=N;failed=N`, then, when some failed,
  `;fails=<binary>::*=none/none,...;cmds=...`. Nothing reads the checkout's commit. The 13 `leftout` lines of the ledger
  have no commit of the checkout; they ran 52.5 to 103.7 minutes, long enough for the lead's next merge to land in the
  same checkout.
* **Who reads a left-out line.** `recent_failed_binaries` reads its `fails=` field (regular expression `fails=([^;]*)`);
  `process_report.py` reads only the `left_out` column of merge lines. A new `key=value` field that is not named
  `fails` changes neither.
* **The budget.** `make_plan` puts the ADR commands (`adr_test_index.adr_commands`, one per binary, or one per test with
  `-- --exact` when the ADR holds only some tests of a binary) into the always group, outside the budget; `cut` drops a
  diff-selected unit whose command is in the always group, without cost. The budget (`--budget-minutes`, 30) goes to
  the diff-selected binaries alone.
* **The order.** `order_group` sorts a group by: not recently failed, cost, crate, target. Nothing reads which source
  files changed.
* **Names.** The integration tests of `rcfd-fv` for the overset solver of `crates/rcfd-fv/src/overset.rs` are named
  `overset_<item>` or `overset_<item>_records` and `overset_fingerprints_adr0748`, `overset_motion_f442`.

## Arithmetic (before any code)

From `arith.py` (the index of the tree at a3d29aff, the recorded times the newest gate line names,
`w49i-work/gate-c76b2d81`; 1.8 s of build per command, 60 s for a command with no recorded test):

* Merge 1b939c19 (ADR 0768): 56 ADR commands, 213.7 s = 3.56 minutes, one with no record. The unchanged dry run selects
  208 binaries (29.4 minutes) and leaves out 132 (197.2 minutes).
* Merge 9a0afcec (ADR 0748): 52 ADR commands, 1600.2 s = 26.67 minutes. The unchanged dry run selects 154 binaries (29.3
  minutes) and leaves out 458 (673.8 minutes). `rcfd-fv` has 22 test binaries named `overset` or `overset_*`; 10 of them
  hold tests of ADR 0748 (always group), 5 more are diff-selected, so 7 are left out.
* Estimates for the changed script: on 1b939c19 about 3.6 minutes fewer for the diff-selected group (some 20 binaries
  fewer selected); on 9a0afcec about 3.3 minutes left for the diff-selected group, of which the overset binaries come
  first. These are estimates of scale, not limits.

## Design (fixed now)

All from first principles; no published method is used. Every change is in `scripts/push_main.py`; the tests are added
to `scripts/test_push_main.py`, and no existing test is edited.

**Point 1, the commits of the left-out run.** `do_left_out(root, path, run=sh, head=None)`: `head()` gives the commit of
the checkout (default: `git rev-parse HEAD` in `root`; `unknown` when git cannot say). It is read once before the first
command and once after the last. The detail becomes
`ran=N;failed=N;checkout_start=<10 characters>;checkout_end=<10 characters>`, and when the two differ `;checkout_moved=yes`
follows; `fails=` and `cmds=` keep their place after these fields. When they differ the run also prints
`push_main: the checkout moved during the left-out run, from <start> to <end>; a binary built after the move was built from <end>`.
The result column stays as it is (green when no command failed): the move is recorded, not judged. The commit column
stays the merge commit of the job. The `run` parameter lets the tests give a recorder in place of `sh`, as `do_run` does.

**Point 2, the ADR's tests in the budget.** For each ADR command the plan computes a cost by one registered rule, the
rule `binary_units` already uses for a binary: the command's tests are the non-ignored tests of the index with its crate,
its kind (`--lib`, `--test T`, `--bin T`, with the target) and, with `-- --exact NAME`, that name; their recorded seconds
are summed over those with a record; a command none of whose tests has a record counts `--default-seconds`; the build
figure (`--build-seconds`, 1.8) is added once per command. The sum is charged first: the diff-selected groups are cut
against the budget less that sum (none fits when the sum is the budget or more). The ADR commands stay in the always
group and are never cut. The plan carries `adr_units` (command, seconds, cost) and `adr_minutes`; the print, dry run
included, has one line after the always group when the merge has ADR commands:
`ADR tests (charged to the budget first, never cut): N commands, X.X recorded minutes, with 1.8 s of build counted per command; Y.Y of the Z.Z budget minutes left for the diff-selected binaries`.
The other commands of the always group (fmt, clippy, `--lib`, `baseline_hashes`, `docs_site`, the process check, the
script tests) stay outside the budget, as before; the acceptance names the ADR's tests only.

**Point 3, the rule of naming.** A changed source file is a file of the merge's diff that ends in `.rs` and lies under
`<crate dir>/src/` of a touched crate (the crate by the longest directory prefix, as `touched_crates`). Its stem is its
file name without `.rs`; for `mod.rs` the name of its folder; `lib.rs` and `main.rs` give no stem. A test binary of the
first group (the touched crates) is named after the changed file when it belongs to the same crate and its target is the
stem or begins with the stem followed by `_` (`overset.rs` names `overset_f134` and `overset_fingerprints_adr0748`, not
`oversets_x` nor a binary `overset_f134` of another crate). Inside the first group the named binaries come first; among
them, and among the rest, the order is as before (recently failed first, then by cost, then by name). The cut is
unchanged (it stops at the first binary that does not fit). The second group (the dependent crates) is not reordered.
The print, dry run included, has one line when some binary is named:
`named after a changed source file: N binaries, first in their crate's group; M of them left out by the budget`.

**What does not change.** `COLUMNS`; the ledger file (not touched by this work); the merge line `do_run` writes for a
given plan (the new plan keys add no field to it); the gate, mutants and ci lines; for a merge with no ADR command and no
named binary, the plan (selection, left-out list, minutes) is what it was. The selection of a merge with ADR commands or a
named binary changes by design; the line's form does not.

## Table of checks

Each row is a unit test (or a set of them) in `scripts/test_push_main.py`, except D, L and P. "Fails before" is shown once
per row by running the new test file against `scripts/push_main.py` of the base commit a3d29aff on a scratch copy. The
limits are pass or fail, so the margin is n/a.

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| T1 | point 1: in a temporary repository, `do_left_out` with a recorder that commits during the second of two commands writes a left-out line with `checkout_start` the commit before and `checkout_end` the commit after, `checkout_moved=yes`, and prints the move; a run during which nothing moves writes both fields equal and no `checkout_moved`; a run with a failed command keeps `fails=` and `cmds=` after the new fields and `recent_failed_binaries` still reads the binary | fails before, passes after | acceptance of F-860, point 1 | passes | n/a | code verification |
| T2 | point 2: the cost of an ADR command is the recorded seconds of its tests (or the default) plus the build figure, for a whole binary and for one `--exact` test; `make_plan` with an ADR command of 61.8 s and a budget of 120 s selects one binary where the unchanged plan selected two; an ADR cost above the budget leaves every diff-selected binary out and keeps the ADR command in the always group; the print of the plan has the line of point 2 with the recorded minutes | fails before, passes after | acceptance of F-860, point 2 | passes | n/a | code verification |
| T3 | point 3: the rule of naming (stem, `mod.rs`, `lib.rs`, `main.rs`, the `_` boundary, same crate only, files under `tests/` not source files); `make_plan` for a change of `crates/a/src/slow.rs` with a budget of 575 s selects `slow` first and leaves `norec` out, where the unchanged plan selected `fast` and `norec`; the print has the line of point 3 with the count left out | fails before, passes after | acceptance of F-860, point 3; the rule registered above | passes | n/a | code verification |
| U | unchanged output: `COLUMNS` is the 11 names; the diff of this work does not touch `docs/process/ledger.tsv`; `do_run` writes the same merge line for a plan with the new keys as for the same plan without them; `make_plan` for a merge with no ADR command and no named binary gives the selection, left-out list and minutes of the unchanged script (checked by the existing tests `Build`, `Selection` and `FirstAttempt`, unedited) | holds | acceptance of F-860 ("the ledger's columns, every existing line and what a run that passes at the first attempt writes are unchanged") | holds | n/a | unchanged default output |
| E | every existing test of `scripts/` passes, unedited: `python -m unittest discover -s scripts -p "test_*.py"` | all pass; no existing test edited | the brief, task 2 | all pass | n/a | unchanged default output |
| D | `python scripts/push_main.py --merge 1b939c19` and `--merge 9a0afcec`, dry runs on the changed script: exit 0, the line of point 2 printed with its minutes, for 9a0afcec the line of point 3 | exit 0; lines present | the brief, task 2 | holds | n/a | code verification |
| L | `cargo test -p rcfd-cli --release --test docs_site` (the links of this ADR) | passes | the brief | passes | n/a | code verification |
| P | `python scripts/process_report.py --check` | 0 failures, except a generated file that is only not current (rule 7) | agent rules, rule 7 | 0 such failures | n/a | code verification |
| M | the mutations below | each caught by the named row | agent rules, rule 16 | each caught | n/a | code verification |

## Negative controls and mutations

Negative controls: the "fails before" of rows T1, T2 and T3, on a scratch copy with the base commit's script.

Mutations, each on a scratch copy of the committed tree, each expected to fail `python scripts/test_push_main.py`. One is
committed beside the test file as `scripts/test_push_main.leftout.mutation.patch` (named as ADR 0752 named its four, since
`test_push_main.mutation.patch` alone would hold one of several).

| Id | Mutation | Must be caught by | Committed |
|---|---|---|---|
| M1 | point 1: the end commit is the one read at the start | T1 | `test_push_main.leftout.mutation.patch` |
| M1b | point 1: `checkout_moved` is never written | T1 | no |
| M2 | point 2: the ADR cost is not taken from the budget | T2 | no |
| M2b | point 2: the recorded seconds of an ADR command are ignored (every command counts the default) | T2 | no |
| M3 | point 3: the named binaries are not put first | T3 | no |
| M3b | point 3: the stem matches any prefix, without the `_` boundary | T3 | no |
| M3c | point 3: a binary of another crate with the stem's name is taken | T3 | no |

## Size

Size (agent): done; 0; 0; 0; 200000

Outcome done; no further registration; no heavy run (one `docs_site` build and run in the release profile, which the
brief asks for); no question for the maintainer; about 200 000 bytes read.

## Decision rule

**Done** if T1, T2 and T3 fail before the change and pass after it, and row U holds (with E). **Partial** otherwise.
Rows D, L, P and M are reported; a failure of one of them is corrected before the report or named in it.

## What is not covered

* No real `--run`, `--push`, `--run-left-out` or `--gate-line`: the brief forbids them. `do_left_out` and `do_run` are
  tested in temporary repositories with the commands replaced by a recorder.
* Point 1 sees a commit, not the working tree: an uncommitted edit in the checkout during a left-out run is not seen.
  It also does not say which binaries were built before and which after a move; option (a) of ADR 0752 (a worktree of
  its own for the left-out run) is the lead's to propose if the field shows moves in practice.
* The recorded seconds of an ADR command come from the gate's times, as for every binary; an `--exact` command runs one
  test but builds and starts the binary, which the 1.8 s figure covers only on average.
* The rule of naming is by file name. A source file whose tests are named otherwise (by item only) gains nothing; option
  (b) of ADR 0752 (the index of ADRs whose tests live in the changed file) is not done.
* `docs/test-process.md` section 7.4 describes the plan; it is not a file of this work. Where it now disagrees, the report
  says so for the lead.
