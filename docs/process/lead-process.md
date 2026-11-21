# The lead's process

Version 1 (2026-11-18). What the lead agent does between the maintainer and the implementation agents. The agents' side is
[agent-rules.md](agent-rules.md). The lead's state is not kept in its session: what is issued is the brief files, what is
merged is the ledger, what is asked is the open-questions list of the process report. Another session can read all three.

## The constraint

The maintainer's attention comes first: a question reaches the maintainer only when it is about scope or priority, as
options with a recommendation. The one shared machine is second: no step here may need more of it than the push budget
below. Whole-system verification is third: main runs ahead of the newest green full run, and the ledger shows by how much.

## Maintainer acceptance

Acceptance is the maintainer's go-ahead for a line of work to be scoped or prioritised. It is written in the dated priority
statement at the head of `docs/ROADMAP.md`, which is restated whenever the maintainer gives a new instruction and names the
lines of work in order, each with its weekly budget in registrations where one is set. The maintainer does not accept
results one by one: an item is done when its registered decision rule says so and the push checks pass.

## Issuing

1. Choose the item from the lines the priority statement names, in order. Do not issue on a line whose weekly budget is
   spent (`python scripts/process_report.py --weekly` shows it). A blocked priority item is not skipped silently: its
   question goes to the head of the weekly list.
2. Decide whether a proposed follow-up is a new item or a further registration of its parent (rule 13). Enter a new item
   only if its acceptance names a number or a named reference, or hands its limits to the registration.
3. Give the ADR number: the next free number on main. Numbers are given by the lead only.
4. Write the brief as a file, `docs/adr/briefs/<branch>.md`, in the form of [the briefs folder](../adr/briefs/README.md):
   items, ADR number, rules version, model, the lead's size (five values, rule 17), the reading list with its bytes, the
   task. Commit it on main (several briefs may share a commit), then start the agent and give it the path. If the brief's
   reading exceeds the reading limit, split it or point it at an area page's reading list.
5. After the registration is committed and before the code: if the report lists one of its limits as having no source
   (rule 15), send it back for a source.
6. A dispatch that is abandoned gets a dated line appended to its brief: "Closed without merge (<date>): <why>".

## Merging and pushing

Merge the branch into main locally, then run `python scripts/push_main.py`. The script is the one list of per-merge checks
([test-process.md](../test-process.md) section 7.4). It first regenerates the generated files, so that the checks see the
pages the push will carry. It then runs the script tests when `scripts/` is touched, fmt and clippy of the touched
packages, the ADR's registered tests, `baseline_hashes`, `docs_site` when `docs/` is touched, the process report's
checks, the tests the diff selects, up to 30 minutes of recorded run time with a recorded build figure per binary, the
committed mutation patches (expecting a failure), and `npm test` in `apps/studio-web` when it is touched. It commits the
generated files with the ledger line, pushes, and starts the selected tests that did not fit in the background.

- The script writes no roadmap status. The lead writes the item's status in `docs/roadmap.json` on main, in a commit of
  its own after the script's commit: after the checks, not before.
- A check that fails is run two further times. If it then passes, the run goes on and the ledger line names the test as
  flaky (`flaky=` in its detail). A check that fails every time stops the run, and nothing is pushed. No test is left
  out of a further run.
- A stopped run is started again with the same command line: what passed on the same merge commit is not repeated
  (`--fresh` repeats everything).
- The tree must be clean. Two things do not count: lines appended to the ledger (the line of a left-out run, which the
  next run commits) and the generated files.
- The item field of the ledger line comes from the branch's brief, or from the head of the branch name.
- The script names an ADR of the merge that has registered tests and no committed mutation patch; it does not stop for it.
- Recorded test times: `--gate-line --gate-times <file or folder>` writes the path into the gate line, and the newest
  such path that exists is the default of `--times`.
- A conflict in a generated file is not resolved by hand: take either side and let the script regenerate it.
- Merging never stops for the full run. No test is marked ignored, skipped or removed to make a run pass.
- If pushes queue up, merge several branches and run one selection over their joint diff:
  `python scripts/push_main.py --run --base <the commit before the first merge>` (an octopus merge needs no `--base`).
  One ledger line names every branch.

## Shared files and their one writer

| File | Writer | Where |
| --- | --- | --- |
| `docs/roadmap.json`, the head of `docs/ROADMAP.md` (above its marker line), `docs/roadmap-overview.md` | the lead | main only, both streams |
| `scripts/roadmap_progress.py` (categories) | the lead | main only |
| `docs/process/ledger.tsv` | the push script and the lead | main only |
| `docs/adr/briefs/*.md` | the lead | main only |
| "Answers of the maintainer" in an ADR | the lead | main only |
| An ADR above its answers | the agent it was issued to | its branch |
| `docs/process/agent-rules.md`, this file, `tolerances.md` | the lead, on the maintainer's word | main only |
| `docs/reference-data.md` | nobody: generated from `docs/reference-data.d/*.part`, one fragment per item, written by that item's agent | each branch adds its own fragment |
| A topic page under `docs/` | the agent whose subject it is | its branch |

Generated, never edited by hand: the item list of `docs/ROADMAP.md` (from `docs/roadmap.json`), `docs/adr/index.md`, the two roadmap SVGs, `docs/studio-coverage.md`, the capabilities
inventory, and the generated sections of the overview and the area pages.

## Questions and answers

An agent's open questions sit in its ADR under "Open questions for the maintainer". The process report lists every such
heading that has no "Answers of the maintainer" heading after it. When the maintainer answers, write the answer with its
date under that heading in the same ADR, on main. Put a question to the maintainer only if it is about scope or priority, or
if rule 6 forbids taking it; decide the rest and record the decision in the ADR.

## The full run

CI on main runs every test. When a run finishes, add its ledger line: commit, date, passed, failed, ignored, and for each
failed test its primary ADR and item (`scripts/gate_attribute.py`). On a green run, move the tag `verified` to that commit.
On a failure, issue the fix as an item against the failing test's ADR before any new work on that line; its result is a
further addendum of that ADR. Main may be red on a test that was left out of a push budget until that fix lands.

## Weekly

- Print `python scripts/process_report.py --weekly` for the maintainer: blocked priority items, lines over budget,
  unanswered questions, and the process measures.
- Issue one audit item: a sample, drawn by a registered seed, of the lead's status decisions checked against the decision
  rule each registration fixed, and of the briefs against the rules. Findings of scope go on the weekly list.
- Issue one citation item: five citations drawn by seed, re-read against their sources. The count of citations from memory
  may not rise.
- A batch of citations beyond the weekly item is issued only for an ADR whose numbers the code uses and that marks a value
  UNVERIFIED (the maintainer, 2026-11-21). The other citations from memory are not a backlog.
- A reader, an agent that reads an item's work afresh, is issued for an item that changes a default result or a golden,
  and not as a rule for every item (the maintainer, 2026-11-21).
- Issue a distillation item for an area whose list of done items missing from its page has passed the size the maintainer
  chose.

## Citations from memory: the "Sources read" addendum

An ADR is append-only, so a citation from memory is not edited away. It is closed by an addendum at the end of the ADR
with the heading `## Sources read (YYYY-MM-DD)` and one row per citation the ADR makes from memory:

| column | content |
|---|---|
| citation | the source as the ADR names it, and what the ADR takes from it (a constant, an equation, a table value) |
| status | `read`, `not obtainable` or `not a source` |
| where | for `read`: the place in the source (equation, table, page, section) and where it was read (DOI or address) |
| finding | `agrees`, or what differs, with both values |

- `read` means the agent opened the source itself in this work and found the place. A second-hand statement of the
  value (a review, a textbook that quotes it) is `read` of that second source, named as such, not of the original.
- `not obtainable` says what was tried (the addresses) and why it failed (behind a paywall, out of print, not found).
  The citation then stays from memory, and the ADR's own derivation or test is what supports the value.
- `not a source` is for a line that names no source, for example a number recalled as a check on arithmetic.
- Vendor documentation, vendor code and course material derived from them are not sources (agent rules, rule 4).
- A finding that differs is not fixed in the addendum. It names the item and goes to the lead as a defect report.

`python scripts/process_report.py --sections e` counts the ADRs that cite from memory and have no such addendum; that
count may fall and never rise. An addendum whose rows are all `not obtainable` still closes the ADR for the count: the
count measures citations looked at, and the rows say what was found.

## When no further agent is started

The numbers of this section are proposed, not in force until the maintainer approves them (ADR 0754, F-863, "Open
questions for the maintainer"). Until then the lead works as before and records the measures.

What the lead records, from the merge of F-863 on (a value that is not given is absent from the line, not zero):

- With the push of a merge: `python scripts/push_main.py --run --reported <time of the agent's report> --review-minutes
  <the lead's minutes on the report> --proposed <the status the agent proposed> --written <the status the lead writes>`.
  The time is `2026-11-21T14:05`, local time unless it carries an offset; the line gets the minutes from the report to
  the merge commit. A status is `done`, `partial` or `todo`; a line of several items takes `F-863=done,F-864=partial`,
  the time of the oldest report and the minutes together.
- With the line of a red full gate: `--gate-line ... --escaped <commit>=<binary::test>` for each failed test whose commit
  can be named, the commit that brought the failure in; `--escaped none` when the lead has looked and no commit did (a
  limit that depends on the load). Without the option nothing is known of that gate.
- `python scripts/process_report.py --weekly` prints the four measures over the last 28 days under "Process measures",
  each with the number of lines that state it.

The conditions:

| Condition | Proposed number | What it counts |
| --- | --- | --- |
| Reports not yet read | no further agent while 3 or more wait | reports from the agent's final message until the lead has checked them against the branch; the lead's count |
| Escaped defects | no further agent when, over the last 28 days, 2 or more commits brought a failure into a full gate and they are more than 1 in 10 of the merges | the line "escaped defects" of the weekly list; it ends when each fix is issued and the lead has written why the push run left the failing tests out |
| A red full gate | while the newest gate line is red, no agent except on the fix of a test that failed in it | the newest line of kind `gate` in the ledger |
| Agents at work at once | at most 4 | the briefs issued and neither merged nor closed (section h of the process report), less those that are not an agent's work |

The reasons, and what of them can be read from the ledger of November 2026 and what cannot, are in ADR 0754. The number
of agents is put to the maintainer again after four weeks of the measures.

## Changes

- Version 1, 2026-11-18: first written, from the documentation process review and the maintainer's decisions of that day.
- 2026-11-21 (F-863, ADR 0754): "When no further agent is started" added as a proposal: the options that record the
  process measures, and four stop conditions whose numbers are not in force until the maintainer approves them. The
  version number is unchanged for that reason.
- 2026-11-21 (F-851, ADR 0752): "Merging and pushing" says what `scripts/push_main.py` does after its first real runs; it
  writes no roadmap status. The version number is unchanged: no step of the lead's process changed, only its description.
- 2026-11-21: "Weekly" carries the maintainer's two answers of that day on citation batches and on readers (handover note of
  2026-11-19, "Answers of the maintainer").

## Golden files

An agent never edits a golden file or a hash table (agent rules, rule 3). Where an item's addendum reports an "additive
golden edit", the lead checks that the diff adds entries and removes or changes none, re-records the file on main and writes
the diff's size and the file in the ledger line of the merge. Any other golden change, and every change to
`baseline_hashes.rs`, needs the maintainer's word for that item.

## Status by the registered rule

"Done" is the lead's decision by the rule the registration fixed. An item whose check, negative control or mutation passed
only after an amendment is done all the same (the maintainer, 2026-11-18) when rule 2 was followed: no numeric limit changed,
the amendment only corrected a defective set-up and was committed before the corrected run, the failed or stopped test is
kept as a record, and no clause of the acceptance is unmet or not run. The earlier practice of recording such an item as
partial (2026-11-13) ends. The lead confirms these points against the ADR before changing the status.

## Questions asked before 2026-11-11

The standing instruction of 2026-11-11 (take the recommendation) also applies to questions that agents listed before that
date (the maintainer, 2026-11-18): the lead closes them on the written recommendation, except a question that would change
a default result, a golden or a hash table, or that needs a source the repository lacks.

## Amending an acceptance

Where a registration's result shows that a clause of an acceptance cannot be met as written or does not measure what was
meant, the lead may amend the acceptance (the maintainer, 2026-11-18): the new words are appended to the acceptance as
"Amended <date>: ..." with the reason and the ADR, the old words stay, and the weekly list names every amended acceptance
so that a lowered bar is seen.
