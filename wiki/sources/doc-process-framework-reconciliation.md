---
type: source
tags: [process, framework]
sources: []
origin: docs/process/framework-reconciliation.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: stable
---

# The working framework of 2026-11-17 against the process in force

**Origin:** [docs/process/framework-reconciliation.md](../../docs/process/framework-reconciliation.md) at `origin/main` 2e94e5ef
**Kind:** reconciliation note (lead, 2026-11-21, on the maintainer's instruction)

## Summary

Two descriptions of how rcfd works exist; this page says which governs. **In force:** `agent-rules.md` (version 4) for agents and `lead-process.md` for the lead, with the priority statement at the head of the roadmap, adopted by the maintainer on 2026-11-18 from the documentation process review ([[doc-process-documentation-review]]). "Where a text in the folder `framework-2026-11-17/` says something else, the rule set in force applies." The four documents of 2026-11-17 and `scripts/roadmap_shards.py` lay uncommitted in the maintainer's checkout (178 commits behind main on 2026-11-21) and are landed as written, as a proposal ([[doc-process-framework-2026-11-17]]).

## Key points

- Their evidence is of 2026-11-17 (754 items, two F-847 rows, 190 items in the overview). On 2026-11-21 main has 768 items, no duplicate id and no missing dependency, and the item list and overview numbers are generated. Their citations of published authors were not re-read by the lead.
- Not landed: a line in `CONTRIBUTING.md` and in `docs/index.md` naming a "process pilot", and three PR-template headings. "No pilot has been agreed."
- **Stage by stage (migration plan stages 0-6):** 0 cutover checkpoint met in substance (no written snapshot; 477 worktrees, nobody has listed which are finished); 1 control code under version control done by other means (`push_main.py`, `render_roadmap.py`, `process_report.py`; the lead still edits `roadmap.json` with one-off scripts kept outside the repo); 2 contracts and results in shadow mode partly; 3 pilot not started; 4 durable execution state not started and not due before a pilot shows the need; 5 admission and integration not started; 6 scale selectively not started. The plan's own success test ("two independent, verified item changes reach one tested candidate without manual re-entry of status...") "is not met: the lead re-enters every status".
- **Adopted:** (1) this page, and the documents as a proposal; (2) a root `AGENTS.md` pointing to the rule set in force; (3) the prototype's roadmap validation (ids, statuses, dependencies, cycles, a lossless split and join) as a CI test; (4) measures and stop conditions before any further worker (F-863); (5) the item-owned roadmap in shadow mode, as the pilot (F-864), with `docs/roadmap.json` the one source until a cutover the maintainer decides.
- **Not adopted:** the task contract and closeout form as a second form beside the brief and the registration ("One rule set", the maintainer, 2026-11-18); a transactional store with leases and attempts (stage 4); a second publisher ("The lead is the one writer of main").
- Housekeeping: `git worktree prune` removes nothing (none of 477 prunable); `bash.exe.stackdump` lies untracked in the maintainer's checkout root; `target/` folder sizes to be reported with the worktree list.

## Contradictions with the wiki

- The review of 2026-11-18 ([[doc-process-documentation-review]], I3) said the rules should "Not [be] a root CLAUDE.md or AGENTS.md, which every Claude Code session in a checkout would load"; this page (2026-11-21) adopts a root `AGENTS.md` that points to the rules without restating them. A later decision within the repo docs; the newer one stands.

## Pages this source informs

- [[doc-process-framework-2026-11-17]]
- [[lead]]
- [[work-streams]]
- [[roadmap]]
