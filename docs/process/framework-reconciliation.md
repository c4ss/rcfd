# The working framework of 2026-11-17 against the process in force

Written by the lead on 2026-11-21, on the maintainer's instruction of that day. Two descriptions of how rcfd works exist.
This page says which one governs and what of the other is adopted.

## What governs

The rule set in force is [agent-rules.md](agent-rules.md) (version 4) for agents and [lead-process.md](lead-process.md)
for the lead, with the priority statement at the head of [the roadmap](../ROADMAP.md). They were adopted by the maintainer
on 2026-11-18 from [the process review](documentation-process-review.md) and are what every brief cites. Where a text in
the folder `framework-2026-11-17/` says something else, the rule set in force applies.

## What the other description is

Four documents and one script were written on 2026-11-17, one day before that adoption, and lay uncommitted in the
maintainer's checkout, which was 178 commits behind main on 2026-11-21. They are landed as they were written, as a
proposal:

- [working-framework.md](framework-2026-11-17/working-framework.md): principles, one work cycle, a task contract and a
  closeout form, six stages of a refactor.
- [loop-migration-plan.md](framework-2026-11-17/loop-migration-plan.md): how the roadmap loop would move to it; an
  item-owned roadmap; seven stages numbered 0 to 6.
- [ai-swarm-orchestration-framework.md](framework-2026-11-17/ai-swarm-orchestration-framework.md): the research behind it.
- [documentation-process-review.md](framework-2026-11-17/documentation-process-review.md): an earlier review, read at
  `65c78c47`. It is not the review of the same name in this folder, which the adoption of 2026-11-18 rests on.
- `scripts/roadmap_shards.py`: a prototype that splits `docs/roadmap.json` into one file per item and joins them again.

Their evidence is of 2026-11-17: 754 items, two F-847 rows, 190 items in the overview, helper scripts in a scratch folder.
On 2026-11-21 main has 768 items, no duplicate id and no missing dependency, and the item list and the overview numbers
are generated. The citations of published authors in these documents were not re-read by the lead; they are the
documents' own.

Not landed: the three edits of tracked files that lay with them (a line in `CONTRIBUTING.md` and a line in
`docs/index.md` that name a "process pilot", and three headings in the pull request template). No pilot has been agreed,
and the agent rules already ask for what the template headings ask.

## Stage by stage

The stages are those of the migration plan.

| Stage | State on main, 2026-11-21 | What is open |
|---|---|---|
| 0. Cutover checkpoint | Met in substance: one lead, briefs and the ledger in the repository, each open decision in an ADR or a roadmap note. | No written snapshot. 477 worktrees exist and nobody has listed which are finished. |
| 1. Control code under version control | Done by other means: `push_main.py`, `render_roadmap.py`, `process_report.py`, each with tests. The scratch helpers the plan names (`roadmap_apply.py`, `lead_patch.py`, the gate wrappers) are not used. | The lead edits `roadmap.json` with one-off scripts kept outside the repository. A check for a dependency cycle ran nowhere; from this commit the prototype's validation runs in CI on the live roadmap. |
| 2. Contracts and results in shadow mode | Partly: a brief has a fixed form and a registration a size line and a decision rule. | An agent's proposed roadmap entry is prose that the lead retypes. No shadow comparison of a validator with the lead's decisions. |
| 3. Pilot of outcome-based work | Not started. | No pilot sample, no success or stop condition, and none of: review minutes, escaped defects, queue age. |
| 4. Durable execution state | Not started, and by the plan's own words not due before a pilot shows the need. | |
| 5. Admission and integration from that state | Not started. The push script binds its checks to the merge commit and pushes without force. | |
| 6. Scale selectively | Not started. | |

The plan's own test of success is that two independent, verified item changes reach one tested candidate without manual
re-entry of status, hand-read test attribution or a merge of generated views. That is not met: the lead re-enters every
status.

## What is adopted

1. This page, and the documents as a proposal.
2. A root `AGENTS.md` that points to the rule set in force, so that an agent entering the repository finds the contract.
3. The prototype's validation of the roadmap (ids, statuses, dependencies, cycles, a lossless split and join) as a test
   that CI runs.
4. Measures and stop conditions before any further worker is added: roadmap item F-863.
5. The item-owned roadmap in shadow mode, as the pilot: roadmap item F-864. `docs/roadmap.json` stays the one source
   until a cutover that the maintainer decides.

## What is not adopted

- The task contract and the closeout form as a second form beside the brief and the registration. One rule set (the
  maintainer, 2026-11-18). Fields the brief lacks are proposed as a change of the agent rules, by F-864.
- A transactional store with leases and attempts (stage 4). It is built only if the pilot shows that unattended dispatch
  is the constraint.
- A second publisher. The lead is the one writer of main.

## Housekeeping the plan names

- `git worktree prune` removes nothing: none of the 477 worktrees is prunable. Removing a worktree deletes a folder and
  can strand a branch that is not merged, so the lead lists them with their state first and the maintainer sees the list.
- `bash.exe.stackdump` lies untracked in the root of the maintainer's checkout, not in a scratch folder.
- The `target/` folders are per worktree; their size is to be reported with the list.
