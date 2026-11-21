# ADR 0754: F-863 registration: process measures and stop conditions before more workers

Status: **REGISTERED 2026-11-21, written before any code of F-863 and before any run of its checks.** Before this text no
script or test code of F-863 was written. What was done: reading (below); one run of `python scripts/test_push_main.py`
(62 tests pass, 26 s), of `python scripts/test_process_report.py` (30 tests pass, 3 s), of
`python scripts/process_report.py --check` (0 failures) and of `python scripts/process_report.py --weekly`, all on the
unchanged tree (log: `baseline.log` in the scratch folder); and the arithmetic on the ledger below. Results go in addenda
at the end; nothing above them is changed after a run.

It answers the brief `docs/adr/briefs/f863-process-measures-stop-conditions.md` and follows
`docs/process/agent-rules.md`, version 4. Branch `f863-process-measures-stop-conditions` from main 47961cd5, scratch
`w49i-work/f863-process-measures-stop-conditions-scratch`.

## What is asked

Roadmap item F-863. Its acceptance: the ledger or a file beside it records, for each merge from the registered date on,
(1) the minutes between the agent's report and the merge (queue age), (2) the lead's review minutes as the lead states
them, (3) whether the item's status as the agent proposed it was the status the lead wrote, and (4) for each full gate
that is red on a test, the merge that brought the failure in when it can be named (an escaped defect);
`scripts/process_report.py --weekly` prints the four measures over the last four weeks, each shown by a unit test;
`docs/process/lead-process.md` states, as numbers registered here and approved by the maintainer before they apply, the
conditions under which no further agent is started (a queue of reports not yet read, a rate of escaped defects, a red
gate) and the largest number of agents at work at once; no existing ledger line and no column changes. The brief adds: a
value the lead does not give is absent, not zero; a merge line written without the new options is what it is today; a
unit test for each measure and each new option, failing before the change; one committed mutation patch for the
recording and one for the reading; the numbers of the stop conditions are proposed with reasons and are not put in force.

The registered date from which the measures are recorded is the date of the merge of this work into main; no earlier
line is changed or completed.

## What was read

`docs/process/agent-rules.md` (version 4); the brief; `docs/process/lead-process.md`, `scripts/push_main.py`,
`scripts/process_report.py`, `docs/process/framework-reconciliation.md` (each whole); the item F-863 of
`docs/roadmap.json`; `docs/process/ledger.tsv` (all 30 lines, the first 330 characters of each detail);
`scripts/test_push_main.py` (its harness `Runner`, `TempRepo`, and the classes `FirstAttempt`, `Times`, `LeadProcess`);
`scripts/test_process_report.py` (its harness `Repo` and the class `State`); ADR 0752 (its form and its table of checks);
the section "Budgets, queues and stopping" and the end of `docs/process/framework-2026-11-17/working-framework.md` (a
proposal, not a rule); the notes of F-471, F-848, F-849, F-854 and F-855 in `docs/roadmap.json` where they name a red
gate. About 190000 bytes.

## Readings (before any code)

* **The detail of a merge line** is `exit=0;mutations=N`, then the optional `studio_skipped=`, `flaky=`, `resumed=` and
  `joint=`, then `cmds=` and the commands joined by ` | `. `cmds=` is last and may hold any character but a tab. The
  detail of a gate line is `passed=N;failed=N;ignored=N`, then the optional `skipped=`, `fails=`, `times=` and a free
  note. Both are `key=value` entries separated by `;`.
* **What the tests fix today.** `FirstAttempt.test_the_ledger_line_of_a_run_with_no_failure` compares the whole line of a
  run without options. `Times.test_the_gate_line_names_the_times_path` compares the whole detail of a gate line. Both call
  `do_run` and `do_gate_line` with a namespace that has none of the new options, so the new code reads each option with a
  default of "not given".
* **Who writes the status.** The push script writes no roadmap status; the lead writes it after the script's commit
  (`lead-process.md`, "Merging and pushing"). At the time of `--run` the lead has read the report and decided the status
  by the registered rule, so the status the lead is about to write can be given to the run.
* **The weekly list** already ends with a heading "Process measures" and the lines of section m (`sec_m`).
* **The ledger of November 2026** (30 lines, 2026-11-12 to 2026-11-21): 16 merge lines on three days (8 on 2026-11-19, 5
  on 2026-11-20, 3 on 2026-11-21), 12 gate lines, 2 left-out lines. Push minutes of the 16: sum 381.4, mean 23.8, median
  9.7 (the mean of the 8th and 9th, 6.5 and 12.9), largest 132.1; per day 291.9, 39.7 and 49.8. Five gate lines are red
  (205889bfb1, 252c9279eb, 46cea4f857, 24a262232e, e9378ecb28); four of them follow one another on 2026-11-20 and
  2026-11-21 with 10, 2, 1 and 1 failed tests, and three item merges and two dependency merges went in between them.
* **What the ledger does not hold.** No line holds the time of an agent's report, the lead's minutes on it, a proposed
  status, the number of agents at work, or the merge that brought a failure in; only the first red gate line has a
  `fails=` entry. The cause of each red gate is in the roadmap notes, not in the ledger: 252c9279eb failed ten tests that
  the push run of the merge 075d8fc8f4 (F-471) had left out, nine of them attributed to that merge alone (note of F-471);
  46cea4f857 and 24a262232e were red on wall-clock limits that pass on a re-run (F-854); e9378ecb28 on the edit lock's F2,
  which passed on the gate's re-run (F-855); 205889bfb1 on a test seen failing once and not reproduced in 180 local runs
  (F-848). By those notes one merge of 16 brought a failure in.
* **Briefs issued and neither merged nor closed** are counted by section h of the process report: 3 today, of which one
  (`claude/documentation-process-framework-3f9271`, issued 2026-11-18) is not an agent at work.

## Design (fixed now)

All from first principles; no published method is used. The changes are in `scripts/push_main.py`,
`scripts/process_report.py`, their two test files and `docs/process/lead-process.md`. No column is added to the ledger,
no existing line is touched, and no file beside the ledger is needed.

### Recording: `scripts/push_main.py`

Four options of `--run`. Each is optional and independent; one that is not given writes no entry. The entries go into the
detail of the merge line after `joint=` (or where it would be) and before `cmds=`, in this order:
`reported=`, `queue_minutes=`, `review_minutes=`, `proposed=`, `written=`.

* `--reported TIME`: the time of the agent's report, in ISO form `YYYY-MM-DDTHH:MM`, with optional seconds and an
  optional offset (`+01:00`); a time without an offset is the local time of the machine. It writes two entries:
  `reported=` with the time as `YYYY-MM-DDTHH:MM` and its offset, and `queue_minutes=N`, the whole minutes (rounded to
  the nearest) from that time to the committer time of the merge commit at HEAD (`git log -1 --format=%ct`). "The merge"
  of the acceptance is that commit: the time of the ledger line would add the minutes of the push run, which the line
  already holds. A time that cannot be read, or one after the merge, stops the run before any check.
* `--review-minutes N`: the minutes the lead spent on checking the report, as the lead states them; a number that is not
  negative. It writes `review_minutes=N` (`N` printed with `%g`). Anything else stops the run before any check.
* `--proposed STATUS` and `--written STATUS`: the status the agent proposed and the status the lead writes. STATUS is
  `done`, `partial` or `todo`; for a line of several items it is a list `F-863=done,F-864=partial`. They write
  `proposed=STATUS` and `written=STATUS`. Each may be given alone. Another word, or one option as a word and the other as
  a list, or two lists of different items, stops the run before any check.
* In a joint run of several merges the line is one, so each option is given once for it: the time of the oldest report,
  the review minutes together, and the statuses as a list.

One option of `--gate-line`, which may be given several times:

* `--escaped COMMIT=BINARY::TEST`: the commit that brought the failure of that test in. COMMIT is resolved with
  `git rev-parse` and written with ten characters, as the ledger writes commits. The entry
  `escaped=<commit>=<binary::test>,<commit>=<binary::test>` goes into the detail of the gate line after `times=` (or
  where it would be) and before the note. `--escaped none` writes `escaped=none`: the lead has looked and no commit
  brought a failure of this gate in (a limit that depends on the load, for example). Without the option the line has no
  entry: nothing is known. The option on a gate that is green, `none` beside another value, a commit that does not
  resolve, or a value without `=` and `::` or with a `;`, a `,` or white space stops with nothing written.
  The commit need not be a merge of a branch: a commit the lead made on main can bring a failure in as well.

The pure parts are functions (`measure_entries`, `queue_minutes`, `escaped_entry`), called by `do_run` before the tree
check and by `do_gate_line` before the line is appended.

### Reading: `scripts/process_report.py`

A function `ledger_measures(rows, today, days=28)` returns the lines; `sec_m` appends them to the lines it gives today, so
`--weekly` prints them under its heading "Process measures" (and section m prints them too). The window is the 28 days
that end today: a line counts when `today - 27 days <= date <= today`. N is the number of merge lines in the window, G the
number of red gate lines in it. The detail of a merge line is read up to `;cmds=`. The lines, in this order:

1. `queue age, minutes from the agent's report to the merge: median M, largest L; stated by K of N merge lines` from the
   `queue_minutes=` entries.
2. `review minutes of the lead: median M, largest L; stated by K of N merge lines` from `review_minutes=`.
3. `status written differed from the status proposed: D of K merges that state both; stated by K of N merge lines` from
   `proposed=` and `written=`; a line that has only one of the two does not state the measure.
4. `escaped defects: E (a commit and a test), brought in by C commits; merges in the window: N; stated by S of G red gate
   lines` from the `escaped=` entries of the red gate lines in the window. E counts distinct pairs of commit and test (a
   failure that stays red over several gates is one defect), C the distinct commits. A line with `escaped=none` states
   the measure and adds nothing to E.

A measure that no line states prints `not stated by any of N merge lines` (for the fourth: `of G red gate lines`) in
place of its numbers: a line without the entry is counted as not stated, never as zero, and an entry whose value is not a
number is not stated either. The median is the middle value of the sorted values, or the mean of the two middle values
(`statistics.median`); the numbers are printed with `%g`.

### Stop conditions: `docs/process/lead-process.md`

A new section "When no further agent is started" before "Changes", headed by the sentence that its numbers are "proposed,
not in force until the maintainer approves them", with one dated line under "Changes". The version number of the page
stays: no step of the lead's process changes until the maintainer approves a number. The section says how the lead gives
the new options and what each condition counts. The proposed numbers, each with its reason:

* **C1, reports not yet read: no further agent is started while 3 or more reports wait.** A report waits from the
  agent's final message until the lead has checked it against its branch. Reason: pushes run one after another on the
  one machine, so a queue of q reports waits at least q pushes: by the ledger's mean of 23.8 minutes, 48 minutes for 2,
  71 for 3 and 95 for 4, before the lead's own minutes, which no line records. At 4, the cap of C4, every agent at work
  could have a report waiting, and a further agent only lengthens the queue; at 2 the lead would stop whenever two agents
  finish together, which the 8 merges of 2026-11-19 show to be common. The length of the queue in November cannot be read
  from the ledger; the queue age recorded from now on is what a later review of this number has.
* **C2, escaped defects: no further agent is started when, over the last 28 days, 2 or more commits brought a failure
  into a full gate and they are more than 1 in 10 of the merges.** It ends when each fix is issued and the lead has
  written why the push run left the failing tests out. Reason: by the roadmap notes (not the ledger) 1 merge of 16
  brought a failure in, 6 %; the proposed rate is the next round figure above what was seen, and the floor of 2 keeps one
  event in a week of few merges from stopping the work. Where it cannot be read: the ledger names no such merge for any
  gate of November.
* **C3, a red full gate: while the newest gate line is red, no agent is started except on the fix of a test that failed
  in it.** Reason: main already runs ahead of the newest green run; on 2026-11-20 and 2026-11-21 four gates in a row were
  red while five merges went in, and each further merge on a red main makes the next failure harder to attribute. The
  three item merges of those two days were fixes of failed tests and would have been started under this condition. Today
  the page stops only "new work on that line".
* **C4, the largest number of agents at work at once: 4**, counted as the briefs issued and neither merged nor closed
  (section h of the process report), less those that are not an agent's work. Reason: it is the number in use, and no
  line of the ledger supports another: the ledger does not hold how many agents worked at once, nor the lead's minutes
  per report, which is what a larger number would spend. What it holds, 16 merges in three days and 381 minutes of
  pushes, was reached under 4. The proposal is to choose 4 with a date of review: after four weeks of the measures
  above, the number is put to the maintainer again with them.

These four are put to the maintainer under "Open questions for the maintainer" in the addendum, each with options and a
recommendation; rule 6 does not let me take them (they are the scope of the item, and the acceptance asks for the
maintainer's approval).

## Table of checks

"Fails before" means: the test is written first and run on the tree without the change, where it must fail; then it must
pass with the change. No limit is a number that could be tuned.

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| A1 | acceptance, queue age recorded: `do_run` in a temporary repository with `--reported` 90 minutes before the merge commit's time writes `reported=` and `queue_minutes=90`; a time without an offset is local time; a time after the merge or an unreadable one stops before any check and writes no line | fails before, passes after | acceptance of F-863 (queue age); design above | passes | n/a | code verification |
| A2 | acceptance, review minutes recorded: `--review-minutes 25` writes `review_minutes=25`; a negative value or a word stops before any check | fails before, passes after | acceptance of F-863 (review minutes) | passes | n/a | code verification |
| A3 | acceptance, statuses recorded: `--proposed done --written partial` writes both entries; one alone writes one; a list of items is written as given; another word, mixed forms or different items stop | fails before, passes after | acceptance of F-863 (proposed and written status) | passes | n/a | code verification |
| A4 | acceptance, escaped defect recorded: `do_gate_line` on a red gate with two `--escaped` values writes `escaped=<ten characters>=<binary::test>,...` before the note; `none` writes `escaped=none`; a green gate, `none` beside a value, an unknown commit and a malformed value stop and write no line | fails before, passes after | acceptance of F-863 (escaped defect) | passes | n/a | code verification |
| A5 | the entries stand after `joint=` and before `cmds=`, in the registered order, in a line that has all of them | fails before, passes after | the brief ("`key=value` entries into the detail of the merge line"); design above | passes | n/a | code verification |
| R1 | acceptance, weekly, queue age: `ledger_measures` gives the median and the largest of the stated values and "stated by K of N"; lines without the entry do not lower the median | fails before, passes after | acceptance of F-863 ("prints the four measures"); the brief ("never as zero") | passes | n/a | code verification |
| R2 | acceptance, weekly, review minutes: the same for `review_minutes=` | fails before, passes after | as R1 | passes | n/a | code verification |
| R3 | acceptance, weekly, status: D of K over the lines that state both; a line with one of the two is not counted | fails before, passes after | as R1 | passes | n/a | code verification |
| R4 | acceptance, weekly, escaped defects: distinct pairs and distinct commits over the red gate lines of the window, the merges of the window, "stated by S of G"; `escaped=none` is stated and adds none | fails before, passes after | as R1 | passes | n/a | code verification |
| R5 | a measure no line states prints "not stated", not a zero; a line dated 28 days before today or earlier is outside the window | fails before, passes after | the brief ("counted as not stated, never as zero"; "the last four weeks") | passes | n/a | code verification |
| R6 | `process_report.py --weekly` on a temporary repository prints the four lines after "Process measures"; a line written by the functions of `push_main.py` is read back by `ledger_measures` with the same values | fails before, passes after | acceptance of F-863; the brief ("under \"Process measures\"") | passes | n/a | code verification |
| S | `lead-process.md` has the section, the words "proposed, not in force until the maintainer approves them", the four conditions and the options' names; the ADR lists the four numbers under "Open questions for the maintainer" | fails before, passes after | acceptance of F-863 (stop conditions); the brief | passes | n/a | code verification |
| E | every existing test of `scripts/test_push_main.py` (62) and `scripts/test_process_report.py` (30) and every other `scripts/test_*.py` passes, unedited | all pass; none edited | the brief ("every existing script test passes unedited or its edit is named with its reason") | all pass, none edited | n/a | unchanged default output |
| U | a run without the new options writes the line it wrote before, and a gate line without `--escaped` the detail it had: the existing tests `FirstAttempt` and `Times.test_the_gate_line_names_the_times_path`, unedited; `docs/process/ledger.tsv` is not in the diff of the branch | holds | the brief ("what a run without the new options writes"; "no existing line changes") | holds | n/a | unchanged default output |
| P | `python scripts/process_report.py --check`; `cargo test -p rcfd-cli --release --test docs_site` | 0 failures other than a generated file that is not current; passes | agent rules, rule 7 | as the limit | n/a | code verification |
| M | the mutations below | each caught by the named check | agent rules, rule 16; the brief (one committed patch for the recording, one for the reading) | each caught | n/a | code verification |

## Negative controls and mutations

Negative controls are in the rows themselves: every stop case of A1 to A4 asserts that no ledger line was written, and
R5 asserts the absence of a number. Mutations, each on a scratch copy except the two committed ones:

| Id | Mutation | Must be caught by | Committed |
|---|---|---|---|
| M1 | recording: the queue age is not divided by 60 (seconds written as minutes) | A1 | `scripts/test_push_main.measures.mutation.patch` |
| M2 | recording: review minutes not given are written as `review_minutes=0` | U, A5 | no |
| M3 | recording: `--proposed` is written under `written=` and the reverse | A3 | no |
| M4 | recording: `--escaped` is accepted on a green gate | A4 | no |
| M5 | recording: a report time after the merge does not stop the run | A1 | no |
| M6 | reading: a merge line without `queue_minutes=` counts as 0 in the median | R1 | `scripts/test_process_report.measures.mutation.patch` |
| M7 | reading: the window is not applied | R5 | no |
| M8 | reading: a line with only `proposed=` counts as stating both | R3 | no |
| M9 | reading: an escaped pair named on two gate lines counts twice | R4 | no |
| M10 | reading: the largest value is replaced by the smallest | R1, R2 | no |

## Size

Size (agent): done; 0; 0; 4; 190000

Expected outcome done; no further registration; no heavy run (script tests and one `docs_site`); four questions for the
maintainer, the four numbers; about 190000 bytes read.

## Decision rule

As the brief fixes it. **Done** only if every one of the four measures has its recording (A1 to A4), its line in the
weekly list (R1 to R4, R6) and a test that fails before the change; every existing script test passes unedited, or its
edit is named with its reason; every mutation M1 to M10 is caught; and the stop conditions are written as a proposal with
their open questions (S). **Partial** if a measure is not recorded or rests on a manual count. **Stopped and reported**
if a measure cannot be recorded without a new column of the ledger or a change of the gate workflow.

## What is not covered

* No line of the ledger before the merge of this work gets the entries; the measures of November stay unknown.
* The values are the lead's statements: the script cannot check a report time, a count of minutes or a proposed status
  against anything. An entry cannot be added to a line later; a value the lead learns afterwards is not recorded.
* The number of reports that wait (C1) and of agents at work (C4) are not written into the ledger by this work. C4 can be
  read from section h of the process report; C1 is the lead's count until a later item records the report's time when it
  arrives and not only at the merge.
* An escaped defect is named by the lead; finding the commit (a bisection) is not automated.
* Nothing is run with `--push`, no line is written into main's ledger, and no cargo test other than `docs_site` is run
  (a timing measurement of another job runs on the machine).
* The stop conditions are not enforced by any script: they are sentences of the lead's process, and not in force.

## Amendment 1 (2026-11-21, before any code and before any run)

Written after the registration's commit and before any script or test code of F-863. It adds two things the design left
open and changes no check, limit or number above.

* **An option without its mode stops.** `--reported`, `--review-minutes`, `--proposed` or `--written` without `--run`,
  and `--escaped` without `--gate-line`, stop `push_main.py` with a message and write nothing. The design above would
  have passed over them in a dry run, and the lead would have believed a value recorded that was not.
* **A heading line of the four measures.** `ledger_measures` gives a first line before the four,
  `F-863, the 28 days to <today>: N merge lines, G red gate lines`, so that the reader of the weekly list sees the window
  the numbers are of. The four lines are as registered.

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| A6 | `push_main.py` with one of the four options and no `--run`, or with `--escaped` and no `--gate-line`, exits with a message and writes no ledger line | fails before, passes after | this amendment; the brief ("a unit test for each new option") | passes | n/a | code verification |
| R7 | the first line of `ledger_measures` names the window's last day and the numbers N and G | fails before, passes after | this amendment; acceptance of F-863 ("over the last four weeks") | passes | n/a | code verification |

Mutation M11: the options without `--run` are passed over. Must be caught by A6. Not committed.
