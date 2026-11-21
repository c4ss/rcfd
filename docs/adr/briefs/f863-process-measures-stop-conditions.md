# Brief: F-863, process measures and stop conditions before more workers

Branch: f863-process-measures-stop-conditions
Items: F-863
ADR: 0754
Rules version: 4
Model: Claude Fable 5.2
Issued: 2026-11-21
Size (lead): done; 0; 0; 8; 128261

The rules are `docs/process/agent-rules.md`, version 4, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f863-process-measures-stop-conditions`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f863-process-measures-stop-conditions-scratch`.

## Reading list

- docs/process/agent-rules.md 12458
- docs/process/lead-process.md 12474
- scripts/push_main.py 48755
- scripts/process_report.py 48832
- docs/process/framework-reconciliation.md 5742

Also in the repository, as needed: the item F-863 in `docs/roadmap.json` (its acceptance), `docs/process/ledger.tsv`,
`scripts/test_push_main.py`, `scripts/test_process_report.py`, the section "Budgets, queues and stopping" and the last
paragraphs of "Refactor sequence" of `docs/process/framework-2026-11-17/working-framework.md` (a proposal, not a rule).

## Why

The ledger records, per merge, the minutes of the push run and the tests that were flaky. It does not record how long an
agent's report waited for the lead, how long the lead took to check it, whether the lead wrote the status the agent
proposed, or which merge brought a failure of the full gate in. Without these the question "would one more agent help or
hurt" has no number behind it, and the cap of four agents at once is inherited, not chosen.

## Task

One registration, `docs/adr/0754-f863-registration-<slug>.md`, the first commit of this work, alone. The work is in
`scripts/` and `docs/process/lead-process.md`; no crate changes.

- **Recording.** Decide and register where each of the four measures of the acceptance is written and by what command of
  the lead: options on `push_main.py --run` (for example `--reported <time of the agent's report>`,
  `--review-minutes N`, `--proposed <status>` and `--written <status>`) that put `key=value` entries into the detail of
  the merge line, and an option of `--gate-line` (for example `--escaped <merge commit>=<binary::test>`) for a red gate.
  The names are yours to register. A value the lead does not give is absent, not zero. No column of the ledger and no
  existing line changes; a merge line written without the new options is what it is today.
- **Reading.** `process_report.py --weekly` prints the four measures over the last four weeks under "Process measures":
  the median and the largest queue age and review minutes, the number of merges whose written status differed from the
  proposed one out of those that state both, the number of escaped defects out of the merges, and for each the number of
  lines that state it. Lines without the entries are counted as not stated, never as zero.
- **Stop conditions.** Add a section to `lead-process.md` that states when no further agent is started and how many may
  work at once. The numbers are not yours to fix: register a proposal of each with its reason (from the ledger of
  November 2026 where it can be read there, and say where it cannot), put them under "Open questions for the
  maintainer" with options and a recommendation, and write the section so that it names the numbers as "proposed, not in
  force until the maintainer approves them". The conditions to cover: reports not yet read by the lead, the rate of
  escaped defects, a red full gate, and the largest number of agents at work at once (four today).
- A unit test for each measure and for each new option, failing before the change. Mutations as rule 16 asks, at least
  one committed patch for the recording and one for the reading.

Do not change: `scripts/roadmap_shards.py`, `scripts/render_roadmap.py` and the roadmap files (another agent works on
them); the meaning of any existing option; what a run without the new options writes. Run nothing with `--push` and write
no line into main's ledger: tests use a temporary repository or the functions directly.

Decision rule to register: done only if every measure has its recording, its line in the weekly list and a test that
fails before the change, every existing script test passes unedited or its edit is named with its reason, every mutation
is caught, and the stop conditions are written as a proposal with their open question; partial if a measure is not
recorded or rests on a manual count; stopped and reported if a measure cannot be recorded without a new column of the
ledger or a change of the gate workflow.

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers. Run in the background anything that takes longer than a few minutes.

## Report (under 400 words)

The registration's commit, any amendment and the code commits; for each measure, where it is written, how it is read and
its test; the proposed numbers of the stop conditions with their reasons; mutations; deviations, each with the commit that
precedes its corrected run; what was not run; open questions; the result of `python scripts/process_report.py --check`;
your verdict by the registered rule.
