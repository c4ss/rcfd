# Brief: F-864, the roadmap as one file per item, in shadow mode

Branch: f864-item-owned-roadmap-shadow
Items: F-864
ADR: 0755
Rules version: 4
Model: Claude Fable 5.2
Issued: 2026-11-21
Size (lead): done; 0; 0; 10; 65078

The rules are `docs/process/agent-rules.md`, version 4, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f864-item-owned-roadmap-shadow`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f864-item-owned-roadmap-shadow-scratch`.

## Reading list

- docs/process/agent-rules.md 12458
- scripts/roadmap_shards.py 5428
- scripts/test_roadmap_shards.py 3422
- docs/process/framework-reconciliation.md 5742
- docs/process/framework-2026-11-17/loop-migration-plan.md 27166 (a proposal, not a rule; "Make independent work compose" and "Prove the new path on a bounded slice")
- scripts/render_roadmap.py 10862

Also in the repository, as needed: the item F-864 in `docs/roadmap.json` (its acceptance); rule 9 of the agent rules (what
an agent proposes for the roadmap, and in what form); the addenda of ADRs 0739 to 0753, each of which ends with "For the
lead: roadmap entries"; `git log -p -- docs/roadmap.json` for what the lead then wrote. The lead's one-off scripts that applied those
entries are not in the repository; the history of `docs/roadmap.json` holds the same edits.

## Why

An agent ends its addendum with roadmap entries as quoted paragraphs. The lead reads them and retypes them into
`docs/roadmap.json` with a script written for that one merge. That is manual re-entry: it is slow, it is where the lead's
wording replaces the agent's without a record of the difference, and two agents' entries cannot be applied without the
lead. The proposal of 2026-11-17 answers it with one record per item, owned by the item's branch. This item tries that
without changing what governs: `docs/roadmap.json` stays the one source.

## Task

One registration, `docs/adr/0755-f864-registration-<slug>.md`, the first commit of this work, alone. The work is in
`scripts/`; no crate changes, and `docs/roadmap.json` and every page generated from it are not changed by you (rule 9).

- **The form of a proposal.** Register a file form for an agent's roadmap proposal: for one item its proposed status,
  its new note entry (one line), its gap when partial, the base commit it was written against; and any follow-up items
  with title, deps and acceptance, without an id (the lead gives ids). Say where on a branch such a file lies.
- **The script.** A script (a new one, or commands added to `roadmap_shards.py`) that validates a set of proposals
  against a roadmap and applies them: the note entry appended to the item's note, the status and gap set, follow-ups
  appended with ids from a start number the lead gives. It writes the result to a path given to it or to standard
  output; when a proposal fails validation nothing is written. It never writes `docs/roadmap.json` by default.
- **Replay.** Choose, by a registered rule and before looking at the results, at least six merged ADRs of November 2026
  whose addendum has roadmap entries. Write each one's proposal file from the addendum alone, apply it to the roadmap of
  the commit before the lead's edit, and compare with what the lead wrote. List every difference and explain each: the
  lead's own additions (a gate, a decision), a changed wording, an entry the lead did not take, an error of either side.
  The count of differences is a result, not a limit.
- **Composition.** Unit tests, each failing before the code exists and each with a named failure that leaves the roadmap
  as it was: two proposals for different items from one base apply in either order with the same result; two proposals
  for the same item; a duplicate id among follow-ups; a missing dependency; a dependency cycle; a base that is not an
  ancestor of the roadmap's commit, or an item whose note changed since the base; a malformed file.
- **Open questions for the maintainer**, with options and a recommendation, not decided by you: the change of rule 9 that
  would let an agent commit such a file; and whether and when `docs/roadmap/items/` becomes the source and
  `docs/roadmap.json` a generated file (the cutover), with what it would cost the existing readers of `roadmap.json`
  (`render_roadmap.py`, `roadmap_progress.py`, `process_report.py`, `adr_test_index.py`, the Studio capability tooling).
- Mutations as rule 16 asks: at least one committed patch for the validation and one for the application.

Do not change: `scripts/push_main.py`, `scripts/process_report.py` and `docs/process/lead-process.md` (another agent
works on them); the existing behaviour of `roadmap_shards.py split` and `join` and its tests.

Decision rule to register: done only if the six replays are made and every difference is listed and explained, every
composition case has its test, every mutation is caught, and no file under `docs/` other than your ADR changes; partial if
a replay could not be made from the addendum alone or a difference is unexplained; stopped and reported if the form
cannot hold what a real addendum of the six proposed.

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers. Run in the background anything that takes longer than a few minutes.

## Report (under 400 words)

The registration's commit, any amendment and the code commits; the form and where it lies; the six ADRs and the rule that
chose them; the differences found, by kind, with counts; the composition tests; mutations; deviations, each with the
commit that precedes its corrected run; what was not run; the open questions with your recommendations; the result of
`python scripts/process_report.py --check`; your verdict by the registered rule.
