# ADR 0752: F-851 registration: scripts/push_main.py after its first real runs

Status: **REGISTERED 2026-11-21, written before any code of F-851 and before any run of its checks.** Before this text no
script or test code of F-851 was written. What was done: reading (below), one run of each existing script test of
`scripts/` on the unchanged tree (all 18 files exit 0; 75 s together), one run of `python scripts/process_report.py
--check` on the unchanged tree (0 failures), and two counting scripts over the lead's push logs (`build_stats.py` and
`build_stats2.py` in the scratch folder; their numbers are below). Results go in addenda at the end; nothing above them
is changed after a run.

It answers the brief `docs/adr/briefs/f851-push-main-after-first-runs.md` and follows `docs/process/agent-rules.md`,
version 3. Branch `f851-push-main-after-first-runs` from main e097520e, scratch
`w49i-work/f851-push-main-after-first-runs-scratch`.

## What is asked

Roadmap item F-851, the binding acceptance, eight points on `scripts/push_main.py`, each with a unit test: (1) a failed
check is run a registered number of further times, a test that failed and then passed is recorded as flaky with its name,
a test that fails every time still stops the push; (2) a stopped run is resumed without repeating what passed on the same
merge commit; (3) the budget counts build time from a recorded figure; (4) several merges are one joint diff; (5)
`--times` has a default, the newest gate times file the ledger names; (6) a left-out line in the working tree does not
stop the next run; (7) a merge whose ADR has registered tests and no mutation patch is named in the output; (8)
`lead-process.md` says that the script writes no roadmap status. The brief adds findings 9, 13, 14 and 18 of the item's
note, asks for findings 10, 11 and 16 to be reported with options and not changed, and keeps: the ledger's columns and
every existing line; the meaning of `--run`, `--push`, `--gate-line` and `--times` when given; what a run that passes at
the first attempt writes.

## What was read

`docs/process/agent-rules.md` (version 3); the brief; `scripts/push_main.py`, `scripts/test_push_main.py`,
`scripts/fmt_check.py`, `docs/process/lead-process.md` (each whole); the item F-851 of `docs/roadmap.json` (acceptance
and note, findings 9 to 18); `docs/process/ledger.tsv` (every line, the first 400 characters of each);
`scripts/process_report.py` (`main`, `generated`, `load_adrs`, `load_ledger`, `load_briefs`, `sec_b`, `sec_e`, `sec_h`,
`sec_k`, `sec_r`, `checks_table`); `scripts/adr_test_index.py` (`adr_commands`); `docs/adr/briefs/README.md`;
`docs/test-process.md` section 7.4; ADR 0751 (its form); outside the repository, in `E:/projects/cfd/w49i-work`: the
eleven `push-*.log` files (the lines `push_main:`, `Compiling` and `Finished`, the tail of `push-f855.log`, every
`push_main: FAILED` with the three lines before it) and the layout of the folders `gate-<commit>` (one folder per shard,
each with a `times.tsv`; shard 1 also has `times_hooks.tsv`).

## Readings (before any code)

* **How a failed command looks.** Cargo prints `test <name> ... FAILED` for each failed test and, at the end, a block
  `failures:` with one name per line indented by four spaces (`push-f855.log`, `push-f471.log`). A command that fails
  before any test (a compile error, the process check, fmt) prints neither.
* **Build seconds in the lead's runs.** Cargo prints one line "Finished `release` profile ... in X s" per invocation,
  after compiling and linking and before the tests run. Over the eleven logs, the `cargo test` commands that compiled
  something (a `Compiling` line between the command and its `Finished` line): 891 commands, 1592.1 s together, mean
  1.79 s, median 1.08 s, 95 % at most 2.68 s, the largest 45.0 s (the first command after a change of `rcfd-fv`). The
  commands that compiled nothing: 370, 52.8 s together, mean 0.14 s. The mean of the first class holds the long first
  compile of each run, spread over that run's commands.
* **What these logs do not hold.** The first run of the merge of F-848 (the item's note: about 50 minutes for 70
  commands, "most of it compiling", in a worktree with an empty target folder) has no log in that folder. A cold build
  of the whole workspace is therefore not in the figure below and is stated as not covered.
* **The item field of 26856a9027.** `merge_info` joins the `F-NNN` of the subject with `adrs_in_files`, which takes
  every `-f<digits>-` of an added ADR file name: `0751-f855-registration-the-edit-locks-f2-lockbusy-...` gave F-002.
  The branch name has the same `-f2-`. The note names the branch; the code read the ADR file name.
* **The process check.** `process_report.py --check` fails with "generated file is not current" for every file
  `--write` would change; `do_run` runs `--write` only after every check has passed (finding 9).
* **The dirty-tree stop.** `do_run` stops when `git status --porcelain --untracked-files=no` prints anything;
  `do_left_out` appends its line to the working-tree ledger and commits nothing (point 6).
* **Who reads the ledger's branch field.** `process_report.py` `sec_h` takes a brief as merged when its branch equals
  the branch field of a merge line.
* **The times files.** The lead keeps the times of a gate as several files, one per shard, under one folder per gate.
  No ledger line names a times file today; a gate line's detail is `key=value` fields and a free note, separated by `;`.

## Arithmetic (before any code)

* **Further runs of a failed check (point 1).** Take a test that fails a run with probability p, runs independent. With
  R further runs the push stops on it with probability p^(1+R). The two flaky tests the note names: `runner_w47b` s7,
  3 of 20, p = 0.15; `adopt_f288`, 1 of 7, p = 0.143. R = 1: 0.0225 and 0.0204. R = 2: 0.0034 and 0.0029. R = 3:
  0.0005 and 0.0004. A test that fails every time costs (1 + R) times its command before the push stops; the longest
  class of command in the ledger is the tests the index maps to an ADR (89.8 minutes for five, finding 11). **R = 2 is
  registered:** it takes a 15 % flake below one stopped push in 290, and a true failure costs three runs of one
  command, not of the plan. The number is a constant of the script (`RETRIES`), not an option.
* **The build figure (point 3).** 1592.1 s / 891 = 1.787 s. **Registered: 1.8 s of build per integration-test binary**,
  added to the cost of every binary in the diff-selected groups (to the recorded seconds, and to `--default-seconds`
  for a binary with no record). It is a constant of the script (`BUILD_SECONDS`), with the option `--build-seconds` to
  give another figure. Effect on the merge of F-855, as a check of scale: 235 selected binaries carry 235 x 1.8 s =
  7.05 minutes of build beside their 29.4 recorded minutes, so about a quarter fewer binaries fit in 30 minutes.

## Design (fixed now)

All from first principles; no published method is used. Every change is in `scripts/push_main.py`, with a small one in
`scripts/fmt_check.py` (so that it can be tested without rustfmt), one in `scripts/process_report.py` (below, point 4)
and the text of `docs/process/lead-process.md`.

**Point 1, further runs and flaky tests.** Every command of the always group and of the diff-selected group, and
`npm test`, goes through one function `run_with_retries(cmd, cwd, run, retries)`. It runs the command; on a non-zero exit
it runs it again, up to `RETRIES` = 2 further times, and stops at the first run that passes. Its result: passed or not,
the number of runs, the names that failed in the failed runs, the minutes of all runs. The command's output is read line
by line, printed as it comes and kept, so that the failed names can be read from it: every `test <name> ... FAILED`
line and every line of the last `failures:` block. A name is written `<binary>::<test>`, the binary being the target of
`--test`, or the crate of `-p` with `-` as `_` for `--lib`; a failed command with no test name (a build error, a script)
is named by its binary with `*`, or by the command itself. Passed after a failure: the names are flaky; the run goes on;
each is printed (`push_main: FLAKY: <name> failed in run 1 of 2 of <command>`) and again in one line before the ledger
is written, and the ledger line's detail carries `flaky=<name>,<name>` between `mutations=N` and `cmds=`. Failed in all
1 + R runs: the push stops exactly as today (the `FAILED` line, nothing written to the ledger, exit 1). No command is
removed from the plan, and no test is filtered out of a command, in a further run: a further run is the same command.
A mutation patch is not run again: its command is expected to fail. A run with no failure writes no `flaky` field.

**Point 2, resuming.** After each command that passes, `do_run` writes `target/push_main/run-<commit, 8 characters>.json`
(under `/target`, which git ignores): the merge commit, the base of the diff, and for each passed command its text, its
minutes and its flaky names; a verified mutation patch is recorded as `mutation <path>`, `npm test` with its skipped
count. A later `--run` that finds this file with the same commit and the same base does not repeat those commands; it
prints each as `passed earlier on this merge commit, not repeated`, counts their minutes and flaky names in the ledger
line, lists them in `cmds=` (the line says what was checked on that commit) and adds `resumed=N` to the detail. A file
of another commit or base is not used and is said so. `--fresh` ignores the file. The file is removed when the ledger
line is written. The premise that makes skipping sound: the same commit with an unchanged tree gives the same inputs to
every command; the tree check (point 6) holds on every run.

**Point 3, build time.** `binary_units` gains a parameter `build_s` (0 where not given, so the existing tests of the
function are unchanged); `make_plan` passes `BUILD_SECONDS` or `--build-seconds`. The cost of a unit is its recorded
seconds (or the default) plus `build_s`; selection, `selected_minutes` and `left_minutes` use the cost. The plan's
print says that the minutes include the build figure. The always group stays outside the budget, as before.

**Point 4, several merges as one diff.** `merge_info(root, rev, base=None)`:
* without `base`: the commit must have at least two parents (one parent stops as today). The diff is first parent to
  the commit, for any number of parents; the branches are every quoted name of the subject after `Merge branch` or
  `Merge branches`.
* with `--base REV`: `REV` must be an ancestor of the commit; the merges are the commits of
  `rev-list --first-parent REV..commit` with more than one parent, at least one; the diff is `REV` to the commit, once.
  Branches, items and ADRs are the union over those merges; added ADR files are read from the joint diff.

One plan and one ledger line follow: commit = the last merge, branch = the branches joined by `,`, items and ADRs the
unions, and `joint=<the other merge commits>` in the detail when there is more than one merge. So that a brief of a
jointly merged branch still counts as merged, `process_report.py` `sec_h` splits the branch field at `,` (a branch name
cannot hold a comma in the lead's use; git allows it, which is noted under "not covered").

**Finding 18, the item field.** The items of a merge are, for each of its branches: the `Items:` line of its brief
`docs/adr/briefs/<branch, / as ->.md`, read from the merge commit's tree, when that file exists (`none` gives no item);
otherwise the head of the branch name, the leading run of `f<digits>-` segments (`f855-edit-lock-f2-lockbusy` gives
F-855; `f828-f814-x` gives both). Nothing else gives an item: not the `F-NNN` of the subject (a subject names other
items in passing, as "F-431 note" in the existing test's example) and not an ADR file name. `parse_subject` and
`adrs_in_files` keep their results (their tests are unchanged); `merge_info` no longer uses their items. The ADR numbers
come from the subject and the added files as before. The line of 26856a9027 is not touched.

**Point 5, the default of `--times`.** `--gate-line` gains `--gate-times PATH`: the detail of the gate line then holds
`times=PATH` (forward slashes; a `;` or a tab in the path is refused), after the counts and `fails=` and before the
note. Without `--times`, the plan reads the ledger from the newest gate line back and takes the first `times=` whose
path exists (relative paths from the repository root); with none, it works as today (every binary counts the default)
and says so. The path may be a folder: every file `times*.tsv` under it is read, and where two files have the same
(binary, test) the larger seconds are kept (the hooks run of a shard repeats binaries of the default run). `--times`
given keeps its meaning and wins; a folder is accepted there too. The plan prints which times it used. `--gate-times`
is a new option because `--times` with `--gate-line` has no meaning today and the brief keeps the meaning of both.

**Point 6 and finding 9, the tree check.** The run stops for a changed tracked file, except:
* the ledger, when the committed ledger is a prefix of the working-tree ledger (lines were appended, none changed);
  the appended lines are committed with the run's own line, as `do_left_out` already says they will be;
* the generated files (`GENERATED` and what `process_report.py --list-generated` names), which the run rewrites.

The second exception is needed by the fix of finding 9: `do_run` regenerates **before** the checks
(`process_report.py --write`, then `roadmap_progress.py`), so the process check and `docs_site` see the index the push
will carry, and a stopped run leaves those files changed for the resumed run. The plan and its command texts do not
change; the commit at the end is the same commit as today. A change in any other tracked file stops the run as today.

**Point 7, no mutation patch.** For each ADR of the merge with at least one test in the index: it is covered when a
patch of the plan lies beside one of its test files, or when the merge adds or changes a patch that lies beside no ADR's
test file (a patch of a script test, say). Each ADR not covered is printed by the plan, dry run included:
`no mutation patch: ADR NNNN has N registered tests and no committed mutation patch (agent rules, rule 16)`. It blocks
nothing and the ledger line is unchanged (`mutations=N` already counts).

**Point 8.** The paragraph "Merging and pushing" of `lead-process.md` says: the script writes no roadmap status (the
lead writes it on main after the script's commit); and, briefly, what points 1, 2, 4, 5, 6 and finding 9 changed. A
line under "Changes" dates it. The version number of that file is the lead's.

**Finding 13.** When the merge changes a file under `scripts/`, the always group begins with `python scripts/<file>`
for every `scripts/test_*.py` of the tree, in name order (18 files, 75 s on this machine, measured above).

**Finding 14.** `fmt_check.py`: the split into batches becomes a function `batches(files, n)`, and `main` takes the
root and the function that runs a command as optional arguments; behaviour and output are unchanged. A new file
`scripts/test_fmt_check.py` tests: 0, 1, 40, 41 and 85 files give 0, 1, 1, 2 and 3 batches that hold every file once
in order; a crate one of whose batches fails gives exit 1 and still runs the later batches; no argument and a crate
with no target file give exit 2 and run nothing.

## Table of checks

Each row is a unit test (or a set of them) in `scripts/test_push_main.py`, except F14 (`scripts/test_fmt_check.py`) and
T4s (`scripts/test_process_report.py`). "Fails before" is shown once per row by running the new test against
`scripts/push_main.py` (or `fmt_check.py`, `process_report.py`, `lead-process.md`) of the base commit e097520e on a
scratch copy. The estimate is what I expect of the finished code; the limits are pass or fail, so the margin is n/a.

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| T1 | point 1: a command that fails once and then passes is run twice, returns passed with the failed test's name; one that fails in all 3 runs is run 3 times and returns failed; `do_run` in a temporary repository writes `flaky=<name>` in the ledger line and prints the name; with a command that fails every time it writes no ledger line and returns 1; the failed names are read from cargo's output | fails before, passes after; 2 further runs | acceptance of F-851, point 1; the brief; R = 2 from the arithmetic above | passes | n/a | code verification |
| T2 | point 2: a run stopped at its third command leaves the state file; the next run on the same commit runs only what had not passed, and the ledger line has every command and `resumed=2`; a state file of another commit is not used; `--fresh` repeats everything; the file is gone after the ledger line | fails before, passes after | acceptance of F-851, point 2 | passes | n/a | code verification |
| T3 | point 3: with a build figure of 1.8 s the cost of a recorded binary is its seconds + 1.8 and of an unrecorded one 61.8; a budget that held three binaries without the figure holds fewer with it; `BUILD_SECONDS` is 1.8 and `make_plan` uses it | fails before, passes after; 1.8 s | acceptance of F-851, point 3; the arithmetic above | passes | n/a | code verification |
| T4 | point 4: in a temporary repository, two first-parent merges with a commit of main between them: `merge_info(HEAD, base)` gives the files of both, both branches, the union of items and ADRs; an octopus merge of two branches the same without a base; a base that is not an ancestor and a range without a merge stop; `do_run` writes one line with `joint=` | fails before, passes after | acceptance of F-851, point 4 | passes | n/a | code verification |
| T4s | point 4: `process_report.py` `sec_h` counts a brief as merged when its branch is one of several in a merge line | fails before, passes after | design above (a consequence of point 4) | passes | n/a | code verification |
| T5 | point 5: the newest gate line with a `times=` path that exists is the default; a newer line whose path does not exist is passed over; no such line gives none; `gate_detail` writes `times=`; a folder of two shard files is read with the larger seconds kept; `--times` given wins | fails before, passes after | acceptance of F-851, point 5 | passes | n/a | code verification |
| T6 | point 6: a tree whose only change is a ledger with an appended left-out line does not stop `do_run`, and the line is in the run's commit; a ledger with a changed existing line stops it; another changed tracked file stops it; a changed generated file does not | fails before, passes after | acceptance of F-851, point 6; finding 9 for the generated files | passes | n/a | code verification |
| T7 | point 7: an ADR with registered tests and no patch is named by the plan's print; one with a patch beside a test file is not; one in a merge that adds a patch beside no ADR's test is not; an ADR with no test is not | fails before, passes after | acceptance of F-851, point 7 | passes | n/a | code verification |
| T8 | point 8: `lead-process.md` says the script writes no roadmap status and no longer says it commits the generated files "with the item's roadmap status" | fails before, passes after | acceptance of F-851, point 8 | passes | n/a | code verification |
| F9 | finding 9: in `do_run` the command `process_report.py --write` runs before `process_report.py --check` and before `docs_site` | fails before, passes after | the brief, finding 9 | passes | n/a | code verification |
| F13 | finding 13: a merge with a file under `scripts/` has `python scripts/test_*.py` for every such file at the head of the always group; a merge without one has none | fails before, passes after | the brief, finding 13 | passes | n/a | code verification |
| F14 | finding 14: the three cases of `scripts/test_fmt_check.py` described above | fails before, passes after | the brief, finding 14 | passes | n/a | code verification |
| F18 | finding 18: `f855-edit-lock-f2-lockbusy` gives F-855 alone; `f828-f814-x` gives both; `citations-2026-w47` gives none; a brief's `Items:` line wins over the name and `none` gives none; in a temporary repository the merge of a branch `f855-a-f2-b` that adds `docs/adr/0901-f855-registration-the-f2-x.md`, with a subject that names F-431, gives the items F-855 alone | fails before, passes after | the brief, finding 18 | passes | n/a | code verification |
| E | every existing test of `scripts/test_push_main.py` (19 tests) and of the 17 other script test files passes, unedited | all pass; no existing test edited | the brief ("every existing test of the script passes unedited or its edit is named with its reason") | all pass, none edited | n/a | unchanged default output |
| U | a run that passes at the first attempt writes the same ledger line as before: in a temporary repository, the detail of the line `do_run` writes is `exit=0;mutations=N;cmds=` and the commands joined by a bar between two spaces, with no `flaky`, `resumed` or `joint` field | holds | the brief ("what a run that passes at the first attempt writes") | holds | n/a | unchanged default output |
| P | `python scripts/process_report.py --check`; `cargo test -p rcfd-cli --release --test docs_site` | 0 failures; passes | the brief ("Check before the report") | 0 failures; passes | n/a | code verification |
| M | the mutations below | each caught by the named test | rule 16; the brief (one committed patch for each of points 1, 2, 4 and finding 18) | each caught | n/a | code verification |

## Negative controls and mutations

Negative controls: the "fails before" of every row T1 to F18, on a scratch copy with the base commit's file.

Mutations, each on a scratch copy of the committed tree, each expected to fail `python scripts/test_push_main.py`
(M14: `python scripts/test_fmt_check.py`). Four are committed as patches beside the test file. Rule 16 names a patch
`<test file stem>.mutation.patch`; four patches beside one test file cannot share that name, so they are named
`test_push_main.<what>.mutation.patch`, which the push script finds by the ending `.mutation.patch`.

| Id | Mutation | Must be caught by | Committed |
|---|---|---|---|
| M1 | point 1: a command that failed in every run is reported as passed | T1 (fails every time still stops) | `test_push_main.retries.mutation.patch` |
| M1b | point 1: no further run is made (`RETRIES` = 0) | T1 (the flaky case) | no |
| M2 | point 2: the state file of another merge commit is used | T2 | `test_push_main.resume.mutation.patch` |
| M2b | point 2: the state file is never read | T2 | no |
| M3 | point 3: the build figure is not added to the cost | T3 | no |
| M4 | point 4: the joint diff starts at the last merge's first parent, not at the base | T4 | `test_push_main.joint.mutation.patch` |
| M5 | point 5: the oldest gate line with a times file is taken, not the newest | T5 | no |
| M6 | point 6: a ledger with a changed existing line counts as appended | T6 | no |
| M7 | point 7: no ADR is ever named | T7 | no |
| M9 | finding 9: `--write` runs after the checks | F9 | no |
| M13 | finding 13: the script tests are never added | F13 | no |
| M14 | finding 14: every file goes into one batch | F14 | no |
| M18 | finding 18: `f<digits>-` is taken anywhere in the branch name | F18 | `test_push_main.items.mutation.patch` |

## Size

Size (agent): done; 0; 0; 0; 125000

Outcome done; no further registration; no heavy run (one `docs_site` build and run in the release profile, which the
brief asks for); no question for the maintainer (three for the lead, which the brief asks for); about 125 000 bytes read.

## Decision rule

**Done** only if every point in scope (points 1 to 8, findings 9, 13, 14, 18) has a test that fails before its change
and passes after, every existing test of the script passes unedited or its edit is named with its reason, and every
mutation of the table is caught. **Partial** if a point is not done or rests on a manual run. **Stopped and reported**
if a point cannot be done without changing the ledger's format, the gate workflow or a crate: that is a question for the
lead, written under "Open questions". Rows E, U and P must hold for done.

Whether `times=` and `flaky=`, `resumed=`, `joint=` in the detail column are a change of the ledger's format: I take
them not to be. The columns stay; the detail is already a list of `key=value` fields that differ by kind; no existing
line changes; a first-attempt run of one merge writes none of them. Taken on the maintainer's standing instruction of
2026-11-11 (take the recommendation); the lead may read it otherwise, so it is repeated in the report.

## What is not covered

* No real `--run --push`: the brief forbids it. `do_run` is tested in temporary repositories with the commands replaced
  by a recorded function; cargo, rustfmt and npm are not started by the tests.
* The build figure is a mean over warm runs of the lead's checkout. A cold build (a new worktree, a change of
  `rcfd-linalg` that rebuilds every crate) costs far more and is not in the figure; `--build-seconds` is the lead's
  handle until a run records one.
* The further runs repeat a whole command. A binary with two tests that fail in turn, each in one run of three, stops
  the push although no test failed every time; this errs to the safe side.
* A resumed run trusts that the toolchain and the target folder are what they were; `--fresh` is the way out.
* A branch name with a comma would be split by `sec_h`.
* `docs/test-process.md` section 7.4 describes the script too; it is outside the brief's scope ("the work is in
  `scripts/` and `docs/process/lead-process.md`") and is reported if it now disagrees.
* Findings 10, 11 and 16 are not changed; options and a recommendation go in the addendum. Findings 12, 15 and 17 are
  checked against point 1 and the named commits, and reported only if something more is needed.
