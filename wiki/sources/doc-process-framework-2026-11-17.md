---
type: source
tags: [process, framework, proposal]
sources: []
origin: docs/process/framework-2026-11-17/*.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Working framework of 2026-11-17 (proposal, 4 documents)

**Origin:** [docs/process/framework-2026-11-17/](../../docs/process/framework-2026-11-17/) at `origin/main` 2e94e5ef (`working-framework.md`, `loop-migration-plan.md`, `ai-swarm-orchestration-framework.md`, `documentation-process-review.md`)
**Kind:** proposal, landed as written; not the rule set in force

## Summary

Four documents written on 2026-11-17, one day before the maintainer adopted the rule set now in force, and landed on 2026-11-21 with a status banner on each: "a proposal of 2026-11-17, landed as it was written; not the rule set in force. What governs, and what of this text is adopted, is in [the reconciliation]". Per [[doc-process-framework-reconciliation]], the rule set in force is `agent-rules.md` (version 4) and `lead-process.md`; where these texts say something else, the rule set in force applies. Which parts are adopted and which are not is recorded below as the reconciliation states it.

## Key points

### working-framework.md ("How we work: outcomes, bounded work and trustworthy evidence", version 1)

- Seven principles: start with a reason; smallest sufficient process; bound commitments, preserve standards; delegate within explicit authority; separate claims from observations; make the present easy to find; improve experimentally.
- One work cycle: Frame, Explore, Commit, Execute, Verify and integrate, Publish, Observe and adapt.
- Roles as responsibilities: maintainer / decision owner, coordinator, worker, verifier / reviewer, integrator.
- Execution states: ready, running, evidence-ready, verifying, integrated, blocked, cancelled.
- A reusable **task contract** and **closeout** form; six refactor stages (establish current truth, prevent drift, pilot bounded work, compare orchestration, automate justified mechanisms, expand selectively).
- **Reconciliation:** the task contract and closeout form are **not adopted** as a second form beside the brief and the registration ("One rule set", the maintainer, 2026-11-18); fields the brief lacks are to be proposed as a change of the agent rules, by F-864.

### loop-migration-plan.md

- Target: an item-owned roadmap, one source record per item under `docs/roadmap/items/`, IDs reserved before dispatch, a deterministic generator building shared views; seven stages numbered 0 to 6; a first batch A-E (verified checkpoint, validated roadmap update, task/result schemas in shadow, manual outcome pilot, durable ownership and recovery).
- **Reconciliation:** stage table on [[doc-process-framework-reconciliation]] (stage 0 met in substance, stage 1 done by other means, stage 2 partly, stages 3-6 not started). **Adopted:** the prototype's roadmap validation as a CI test; measures and stop conditions (F-863); the item-owned roadmap **in shadow mode** as the pilot (F-864), `roadmap.json` staying the one source until a cutover the maintainer decides. **Not adopted:** a transactional store with leases and attempts (stage 4) unless the pilot shows the need; a second publisher. The plan's own success test is "not met".
- Comes with `scripts/roadmap_shards.py`, a prototype that splits `docs/roadmap.json` into one file per item and joins them again.

### ai-swarm-orchestration-framework.md (research, 2026-11-17)

- Recommendation: "bounded delegation, explicit state and evidence-based integration": a coordinator, a small set of workers, independently scoped verification, an integration queue; increase autonomy and concurrency only when project-specific measurements justify them. "A free-form group conversation is not a sufficient protocol for editing code and accepting scientific claims."
- **Reconciliation:** kept as "the research behind it"; its citations of published authors were not re-read by the lead.

### documentation-process-review.md (framework folder, baseline `65c78c47`)

- An earlier review that scored the evidenced process "54/100, D (developing)" on a weighted rubric (outcomes 15, discovery 15, scope 10, technical evidence 20, current authority 15, ...), with "Current authority and consistency" weakest (1 of 4).
- **Reconciliation:** "It is not the review of the same name in this folder [docs/process/], which the adoption of 2026-11-18 rests on." Historical only.

## Contradictions with the wiki

- Two different files are named `documentation-process-review.md` (`docs/process/` and `docs/process/framework-2026-11-17/`); the wiki keeps them apart: the governing one is [[doc-process-documentation-review]]. Not a contradiction, a naming trap.

## Pages this source informs

- [[doc-process-framework-reconciliation]]
- [[work-streams]]
- [[lead]]
