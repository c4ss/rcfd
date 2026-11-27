# Brief: F-997, A runner started without the caller's handles, and its exit when its project folder is gone

Branch: f997-runner-no-inherited-handles
Items: F-997
ADR: 0829
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 1; 0; 0; 64653

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f997-runner-no-inherited-handles`, on the branch `f997-runner-no-inherited-handles`, which the lead made at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f997-runner-no-inherited-handles-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief docs/adr/briefs/f997-runner-no-inherited-handles.md (F-997, ADR 0829) is carried out on the branch f997-runner-no-inherited-handles, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f997-runner-no-inherited-handles, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of ADR 0829 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of fmt and clippy of the crates touched, their `--lib` tests, the named test files and `baseline_hashes`, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- crates/rcfd-runner/src/sys.rs 22157
- crates/rcfd-runner/src/client.rs 27807

Also in the repository, as needed: ADR 0146 (the runner's start and lock), ADR 0123 (the runner and its config), ADR 0128
(finding 17, console programs with no window; `tests/no_window_scan.rs`), guardrail G7 and ADR 0801's addendum "Runner closes
with the Studio", `crates/rcfd-runner/src/runner.rs` (the service loop, its lock and `runner.json`), `crates/rcfd-runner/tests/`
(`started_by_qrl2.rs` and the start tests), `scripts/push_main.py` (`sh`, the hang it now reports).

## What the lead decided (2026-11-27)

The maintainer asked (2026-11-27) for the root cause of the runners left behind to be fixed in the engine too. A runner is
started by `rcfd-runner ensure` (and by rcfd-mcp's run tools through the client) detached, with `DETACHED_PROCESS |
CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW` on Windows (`crates/rcfd-runner/src/sys.rs`), its output to `.rcfd/runner.log`
by default. Two defects follow. (1) On Windows a process std's `Command` starts inherits every inheritable handle of its
parent, not only its standard streams: a runner started from a test whose output was a pipe kept that pipe open after the
test ended, and a push waited nine hours for it (2026-11-27; push_main now reports such a process, 52ea1458). (2) The
runner has no exit when its project is gone: the e2e tests remove their temporary project folders and the runner went on
serving a deleted folder (17 such runners were ended by hand). The tests' teardowns are another agent's (ADR 0828); this
work is the engine's side. Both changes are to shipped behaviour and are registered before any code; solver defaults and
`baseline_hashes` are untouched.

## Task

One registration, `docs/adr/0829-f997-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any code**: (a) how a detached runner is started with only its own standard streams and no other
   inherited handle on Windows (for example an explicit handle list in the start's attributes, from the operating system's
   published process-creation interface) and what holds on other platforms (close-on-exec), with the check: a parent whose
   standard output is a pipe starts a runner and exits, and the pipe reaches its end within a registered time while the
   runner lives; its negative control on the code before the change (the pipe stays open); (b) when the runner exits by
   itself because its project folder is gone (the folder, or its `.rcfd` lock, absent at a registered look interval), with
   no run queued, running or paused, cleanly as `rcfd-runner shutdown` does, and what it does with active runs; the check
   with a test that removes a project folder under a live runner and sees it exit within a registered time, and a control
   that a runner with its folder present keeps serving; (c) that every existing start path keeps its behaviour (the start
   tests, `started_by_qrl2.rs`, `no_window_scan.rs`); mutations (rule 16); the numbers fixed in advance (rule 15).
2. **Build** the two changes in `crates/rcfd-runner` only.
3. Tests in new files; one mutation committed as a patch, its expect-fail command with `--target-dir target/mutation`.

Decision rule to register: done if (a) and (b) hold with their controls, every existing runner test passes and each
mutation is caught; partial otherwise.

Open questions for the maintainer, with options and a recommendation: the look interval, active runs at a folder's removal,
and any question of scope.

Files you own: `crates/rcfd-runner/src/sys.rs`, the start in `client.rs`, the service loop's look in `runner.rs` (named in
the registration), new tests, ADR 0829, `docs/roadmap/proposals/0829-F-997.json`. No Studio files, no solver files.

## Common to this work

The acceptance of F-997 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
decision rule is registered before the run and is not changed after it; a result that misses is recorded as a miss with
its number; a run that does not decide is recorded as not deciding. No test is ignored, skipped, removed or quarantined
to make a run pass; a registered miss is held as the repository holds known misses (rule 2), with its limit intact.

Clean-room: reference values only from published sources that were read and that a page of the repository records with
their citation; nothing from memory (rule 4); vendor documentation is never a source.

Use your own target folder under the scratch folder for every cargo command (`CARGO_TARGET_DIR`). Other agents run on
this machine at the same time and own other files; a test that fails on a file refused by the operating system under
that load is reported, not retried until it passes. Delete nothing outside your worktree and your scratch folder.

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers. Rule 9: you do not edit `docs/roadmap.json`, `docs/milestones.json` or a generated page; you write your
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0829-F-997.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
