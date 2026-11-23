---
type: source
tags: [process, lead]
sources: []
origin: docs/process/lead-process.md
origin_commit: 8732f2ae
created: 2026-11-22
updated: 2026-11-23
status: stable
---

# The lead's process

**Origin:** [docs/process/lead-process.md](../../docs/process/lead-process.md) at `origin/main` ~~2e94e5ef~~ 8732f2ae (changed at 18475a78)
**Kind:** process rules (version 1, 2026-11-18, with dated changes to ~~2026-11-21~~ 2026-11-23)

## Summary

What the [[lead]] agent does between the [[maintainer]] and the implementation agents ([[implementation-agent]]); the agents' side is [[doc-process-agent-rules]]. "The lead's state is not kept in its session: what is issued is the brief files, what is merged is the ledger, what is asked is the open-questions list of the process report. Another session can read all three."

## Key points

- **The constraint**, in order: the maintainer's attention (a question reaches the maintainer only when it is about scope or priority, as options with a recommendation); the one shared machine (no step may need more than the push budget); whole-system verification ("main runs ahead of the newest green full run, and the ledger shows by how much").
- **Maintainer acceptance** is "the maintainer's go-ahead for a line of work to be scoped or prioritised", written in the dated priority statement at the head of `docs/ROADMAP.md` with each line's weekly budget in registrations. "The maintainer does not accept results one by one: an item is done when its registered decision rule says so and the push checks pass."
- **Issuing** (6 steps): choose from the priority lines in order, not on a line whose weekly budget is spent (`process_report.py --weekly`); new item vs further registration (rule 13); the lead alone gives ADR numbers; write the [[brief]] as `docs/adr/briefs/<branch>.md` and commit it on main before starting the agent; send back a registration whose limit has no source (rule 15); an abandoned dispatch gets "Closed without merge (<date>): <why>".
- **Merging and pushing:** merge locally, then `python scripts/push_main.py` ([[push-main]]), the one list of per-merge checks (test-process.md 7.4). A failing check is run two further times; if it then passes the ledger names it flaky (`flaky=`); "No test is marked ignored, skipped or removed to make a run pass." "Merging never stops for the full run." The lead writes the item's status in `docs/roadmap.json` in a commit of its own after the script's commit. Agents' roadmap proposals (`docs/roadmap/proposals/<ADR>-<item>.json`, form 1 of ADR 0755) are applied with `scripts/roadmap_proposals.py apply`; the lead does not reword the agent's entry (the maintainer, 2026-11-21).
- **The wiki after each merge** (added 2026-11-23, the maintainer's answer of that day, option (a)): after the roadmap commit of a merge, the lead runs `/wiki-ingest` for what the merge added or changed (its ADR on the thread page, the item's row, the changed docs, the pages whose claims changed), as the wiki schema's "Steady-state ingest" says, and commits the wiki's changes with a log entry `## [date] ingest | <ADR/F/doc id> (loop merge)`. "Agents do not edit the wiki; their ADRs are its sources."
- **Shared files and their one writer** (table): roadmap.json, the ROADMAP.md head and the overview by the lead on main only, both streams; `docs/process/ledger.tsv` by the push script and the lead; briefs by the lead; "Answers of the maintainer" by the lead; an ADR above its answers by its agent; agent-rules.md, lead-process.md, tolerances.md by the lead on the maintainer's word; `docs/reference-data.md` by nobody (generated from per-item fragments). Generated, never hand-edited: ROADMAP.md item list, `docs/adr/index.md`, the roadmap SVGs, `docs/studio-coverage.md`, the capabilities inventory, generated sections of overview and area pages.
- **Questions and answers:** see [[maintainer-questions]].
- **The full run:** CI on main runs every test; the lead adds a ledger line (commit, date, passed, failed, ignored, each failed test's primary ADR and item via `scripts/gate_attribute.py`); on green, move tag `verified`; on a failure, issue the fix against the failing test's ADR before new work on that line ([[gate]]).
- **Weekly:** print `process_report.py --weekly`; one audit item (seeded sample of the lead's status decisions and briefs); one citation item (five citations by seed; the from-memory count may not rise); citation batches only for an ADR whose numbers the code uses and that marks a value UNVERIFIED (2026-11-21); a reader only for an item that changes a default result or a golden (2026-11-21); `push_main.py --mutants --run` on one merge a week; a distillation item when an area's missing-done list passes the maintainer's size.
- **"Sources read" addendum:** see [[sources-read-addendum]]; `process_report.py --sections e` counts ADRs citing from memory without one, a count that "may fall and never rise".
- **Stop conditions** ("When no further agent is started", in force from 2026-11-21, ADR 0754, F-863): no further agent while 3 or more reports wait unread; none when over 28 days 2 or more commits brought a failure into a full gate and they are more than 1 in 10 of the merges; while the newest gate line is red, no agent except on the fix; at most 4 agents at work at once. Recording options: `--reported`, `--review-minutes`, `--proposed`, `--written`, `--escaped`.
- **Golden files:** an agent never edits one; for an "additive golden edit" the lead checks the diff only adds, re-records on main and records it in the ledger line; every change to `baseline_hashes.rs` needs the maintainer's word.
- **Status by the registered rule:** an item whose check passed only after an amendment is done (the maintainer, 2026-11-18) when rule 2 was followed (no numeric limit changed, amendment committed before the corrected run, failed test kept, no clause unmet); the 2026-11-13 practice of recording such items partial ends.
- **Questions asked before 2026-11-11** are closed on their written recommendation, except ones changing a default, a golden, a hash table, or needing a missing source. **Amending an acceptance:** "Amended <date>: ..." appended, old words stay, the weekly list names each amendment.

## Contradictions with the wiki

_None found at ingest._ ([[gate]] already says the gate runs on main, not before each merge; this page agrees.) At 18475a78: the wiki schema's "Steady-state ingest" (2026-11-22) lets subagents in a loop draft wiki edits for their own item; the lead process (2026-11-23) says agents do not edit the wiki. The newer rule is the maintainer's answer of 2026-11-23 and governs the repository's process; the schema's sentence is left for the maintainer to align (the schema is co-edited with the maintainer).

## Pages this source informs

- [[lead]]
- [[maintainer]]
- [[implementation-agent]]
- [[process-ledger]]
- [[process-report]]
- [[push-main]]
- [[brief]]
- [[maintainer-questions]]
- [[sources-read-addendum]]
- [[gate]]
- [[roadmap]]
- [[roadmap-status]]
- [[registration]]
