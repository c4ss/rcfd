# How we work: outcomes, bounded work and trustworthy evidence

> Status (2026-11-21): a proposal of 2026-11-17, landed as it was written; not the rule set in force. What governs, and
> what of this text is adopted, is in [the reconciliation](../framework-reconciliation.md). Only the links to pages
> outside this folder were changed so that they resolve from here.

Version 1, 2026-11-17. The unified framework for refactoring how people and AI agents work in rcfd.
It replaces the operating proposals in the [documentation review](documentation-process-review.md) and
[orchestration research](ai-swarm-orchestration-framework.md). Those documents retain the baseline assessment,
source detail and limitations. Their historical grade is not a score for this new framework.

Start applying the framework to new or substantially reshaped work in the agreed pilot scope. Use the existing task and
PR system and follow the applicable clean-room requirements, product and design authority, Studio guardrails, scientific
registrations and required checks. The rollout below describes when to expand the pilot and introduce automation.

## Purpose and principles

Optimize useful outcomes supported by evidence within compute, money and human attention budgets. Judge the whole path
from problem to accepted result. Faster generation helps when verification, integration and documentation keep pace.

1. **Start with a reason.** Link active work to a user or engineering outcome, demonstrated defect, scientific question
   or maintenance obligation. Label assumptions. Infrastructure work need not invent a customer story.
2. **Use the smallest sufficient process.** Reuse existing items, PRs and records. Add a document, agent or check when
   it resolves a concrete coordination or evidence problem. Avoid repeated accounts of the same fact.
3. **Bound commitments, preserve standards.** Set scope, exclusions, investment limits and a reconsideration point.
   Reduce scope when needed; never silently relax scientific acceptance or rewrite a miss into a pass.
4. **Delegate within explicit authority.** Workers make routine choices inside task constraints. Bring consequential
   unresolved choices to the existing decision owner with evidence and a recommendation.
5. **Separate claims from observations.** Persuasive explanations, agent agreement, successful execution and physical
   validation are different things. State exactly which claim the evidence supports.
6. **Make the present easy to find.** Give each fact one authoritative home. Link current guidance to historical rationale
   and evidence; do not require reconstruction of policy from chronological addenda.
7. **Improve experimentally.** Include failures and process overhead when measuring benefit. More agents, documents or
   completed items are not success measures by themselves.

## One work cycle

| Step | Action | Exit condition |
|---|---|---|
| Frame | Identify beneficiary or obligation, problem evidence, outcome and accountable owner. | The reason is clear; assumptions are labelled. |
| Explore | Address significant product, technical or scientific risks; compare alternatives when the choice matters. | Justify the next bounded step or name the uncertainty it will investigate. |
| Commit | Specify deliverable, exclusions, dependencies, budget, decision rights and acceptance. Register scientific protocols before runs. | The worker can act and knows when to return for a decision. |
| Execute | Work within scope; preserve material decisions, attempts and artifacts. | A reviewable result, explicit blocker or negative finding exists. |
| Verify and integrate | Check acceptance against the actual revision, including misses/skips; apply existing integration checks. | Evidence supports acceptance and the integrated revision satisfies its required checks. |
| Publish | Update delivery status and affected current guidance; link superseded decisions and limitations. | Entry points accurately describe what exists and what was checked. |
| Observe and adapt | Compare intended outcomes with observations; continue, revise, expand or stop. | Learning informs a decision, or an owner and date/trigger exist for follow-up. |

These steps may overlap and do not require seven meetings or artifacts. A disconfirmed hypothesis can complete a useful
investigation. A feature can be integrated while its effect on users remains unobserved: record both, rather than holding
the patch open indefinitely or claiming value prematurely.

## Scale controls to the work

| Work | Minimum record | Additional controls when applicable |
|---|---|---|
| Small reversible correction within policy | PR/existing task: reason, result, proportionate verification and docs impact | No new ADR or agent team solely for a typo, mechanical edit or established fix. |
| Bounded feature, interface or cross-component change | Task contract, acceptance-to-evidence links and closeout | ADR for consequential choices; interface ownership and integration review. |
| Numerical method, benchmark claim or scientific investigation | Public sources, prospective question/protocol/limits, reproducible evidence and interpretation | Analytic checks, conservation, manufactured solutions, refinement or held-out data appropriate to the claim. |
| Product discovery or UX work | Problem evidence, assumptions, chosen alternative and planned observation | Genuine user/maintainer trials labelled by source; agent simulations remain hypotheses. |
| Policy, permission, scientific acceptance or orchestration change | Rationale, existing decision owner, effect on active work, evaluation and reversal plan | Respect standing authorization; register a new protocol before a changed scientific run. |

Categories may overlap. Apply relevant controls instead of every control to every task. Keep library capability,
CLI/MCP access, Studio availability and physical validation separate. Passing a test establishes only what that test checks.

## Authority and documentation

| Information | Authoritative home | Working rule |
|---|---|---|
| Audience, outcomes and product principles | `PRODUCT.md` and relevant product/milestone plan | Link active work to an outcome or obligation; distinguish observation from assumption. |
| Visual and interaction rules | `DESIGN.md` | Follow existing design authority. |
| Constraints and decision rights | Contribution policy, [Studio guardrails](../../studio-guardrails.md) and accepted decisions | A worker cannot override these through a task summary. |
| Item identity, dependencies and delivery status | `docs/roadmap.json` today, item-owned records after the proposed cutover | Generate the legacy JSON and roadmap views from the candidate records after cutover. Until then, check edited views together. |
| Consequential rationale and decision status | `docs/adr/` | Separate proposed/accepted/rejected/superseded decisions from implementation progress; index the current decision by subject. |
| Scientific protocols and observations | Linked registrations and evidence artifacts | Preserve the original protocol/results; corrections and new protocols are linked, dated additions. |
| Execution state and ownership | One task record in the existing work system | Do not create a competing roadmap status. |
| Checks and release procedure | [Test process](../../test-process.md), [release guidance](../../releasing.md) and their workflows | Link the procedure rather than copy another command list. Resolve conflicts explicitly with the relevant owner. |
| Supported user workflows | Tutorials, task guides, reference and explanation in `docs/` | Update affected current guidance with the change; historical evidence stays historical. |

Join records with links and IDs. Generate numbers and repeated item text; editorially review qualitative summaries when
supporting capabilities change. A generated count cannot validate a sentence about solver behaviour. New consequential
decisions get focused records. Reference old addenda without rewriting their evidence. Migrate active work first.

## Responsibility and autonomy

Roles are responsibilities, not required headcount or permanent agents. One person can hold several roles.

| Role | Owns | Boundary |
|---|---|---|
| Maintainer / relevant decision owner | Outcomes, major trade-offs, policy and exceptions | Routine actions within existing authorization do not need repeated approval. |
| Coordinator | Decomposition, dependencies, assignment, budgets and progress | Cannot silently expand scope or declare unsupported evidence sufficient. A lone worker may coordinate its own task. |
| Worker | Investigation/implementation and honest result package | Reports unexpected coupling, misses and changed assumptions. |
| Verifier / reviewer | Acceptance coverage, evidence quality and relevant failure cases | Model review supplements executable evidence; agreement is not scientific measurement. |
| Integrator | Conflicting writes, integrated revision, status and current docs | Owns the shared update path and required integration checks. |

Escalate when authority is missing, hard constraints would change, requirements conflict, important acceptance is ambiguous
or the agreed budget is exhausted. Return the concrete result, uncertainty, options and recommendation. Continue independent
authorized work where possible. If the owner is unavailable, record the blocker; never fabricate approval. Prefer a reusable
decision rule over repeated escalation of routine choices.

## AI orchestration within the same cycle

Start with one capable worker. Add workers for independent investigations or components with agreed interfaces, or for
verification with a distinct evidence task. Agent count and model diversity are choices to evaluate, not targets.

Parallelize independent cases, source checks and hypotheses; keep continuation chains ordered. Give tightly coupled solver
changes a coherent owner until boundaries are explicit. Agents may prepare discovery questions and prototypes but cannot
substitute generated personas for real user evidence.

Delegated work uses the contract below. Pin the base revision and relevant decisions. Supply compact current context with
authoritative links instead of entire historical transcripts. Store substantive results as artifacts; handoffs carry
references, provenance and open questions, avoiding repeated paraphrases.

Set the worker's file and tool access to match the contract. Give it the isolated checkout, approved clean-room sources
and task artifacts it needs. The existing contribution policy governs source selection. Reserve publication credentials
and protected decisions for the role authorized to use them.

Isolate concurrent edits. Serialize conflicting writes through integration, especially roadmap, shared schema, dependency
and policy updates. Independent work can continue in parallel. Recheck affected evidence against the integrated revision;
a branch check says nothing about a later incompatible merge.

Use existing worker/worktree facilities with explicit owner and attempt records initially. Before unattended rescheduling,
test how authority expires, where stale writes are rejected and how retries are bounded. A heartbeat indicates liveness.
Use a budgeted checkpoint to show progress. Before repeating a launch, merge or publication after interruption, reconcile
against the receiving system. Hold attempts whose effects cannot be determined.

Keep execution state, decision status, roadmap delivery status and observed outcome separate. Useful execution states are
ready, running, evidence-ready, verifying, integrated, blocked and cancelled. Record acceptance authority and rationale.
A verified negative experiment may complete its investigation while a related feature remains partial.

## Acceptance and trust

Trace accepted claims to artifacts, revisions, environments and checks/observations. Disclose failures, skips, unavailable
hardware and absent data. Label inferred historical test mappings; prefer explicit acceptance/check mappings for new work.
Retain the ADR index for navigation and historical recovery.

For important claims, scope verification separately from implementation. Seek different evidence paths: analytic results,
independently derived identities, published benchmarks or held-out cases. Different models can share the same mistaken
source. Tests derived from the same incorrect equation as the implementation can agree with it. Do not use votes as proof.

Preserve scientific limits and golden baselines. When automation is built, protect them through permissions or integration
checks; prompt wording alone is not enforcement. Legitimate test corrections follow the existing decision process with
rationale and before/after evidence. Preserve the old result so a test correction cannot masquerade as better solver accuracy.

Use the documented per-merge checks in section 7.4 and the separate Studio checks in section 8. The full Rust gate gives
evidence for its own scope. Failure attribution identifies candidates for diagnosis. Before asserting a cause, verify it
against the relevant revisions and conditions. Bind integration results to the exact candidate and capture them directly
from the trusted check runner. Record mismatches between written policy and campaign practice for the decision owner.

## Budgets, queues and stopping

Bound investment in the units that matter: elapsed time, compute/test slots, money/tokens and reviewer attention. Use defaults
agreed with the owner; do not invent a precise cost model for tiny fixes. An investment limit is not a duration estimate.

Admit work according to dependencies and downstream capacity. Evidence-ready patches awaiting review remain unfinished work.
When that queue grows, verify and integrate before launching more implementation. Bound builds and solves to available RAM,
CPU/GPU and test slots. Do not assume a fixed large swarm is optimal.

Checkpoint and reconsider at a budget limit, lack of progress, contradictory evidence or unexpected shared-interface change.
Choose smaller scope, a new hypothesis, a specific extension or a stop. Avoid unlimited repair loops and replacements repeating
failed approaches without new evidence. A negative result is useful when it answers the registered question.

## Reusable task and closeout

Keep these in the existing task, PR or registration. Fields may be links or short sentences. State inapplicability briefly;
do not duplicate an ADR or create a file merely to satisfy a template.

### Task contract

```text
Reason/outcome/obligation and supporting evidence:
Item / decision / protocol links:
Owner; worker; reviewer/integrator where needed:
Base revision; permitted changes; interfaces/invariants:
Deliverable; exclusions; dependencies and accepted inputs:
Acceptance clauses -> checks/observations; prospective limits:
Budget; checkpoint/stop rule; delegated choices and escalation boundary:
Affected current docs; artifact destination:
```

### Closeout

```text
Result and artifact/patch references:
Tested revision; environment; commands/observations:
Acceptance clauses -> evidence, misses, skips and unresolved states:
Material decisions, limitations and remaining work:
Integrated revision and required checks (or pending integration):
Current docs/status updates (or concrete remaining updates):
Outcome: observation OR follow-up owner and date/trigger:
```

## Refactor sequence and exit conditions

The [loop migration plan](loop-migration-plan.md) maps these stages onto the inspected lead/worker loop, its helpers,
state, checks and decision queue, with shadow operation, cutover and recovery conditions.

The framework is usable manually now. Implement support in bounded steps; do not block development on an archive migration,
new orchestration platform or perfect maturity score.

| Stage | Work | Exit condition |
|---|---|---|
| 1. Establish current truth | Recheck audit findings against current main: duplicate items/counts, capability claims, CI history and gate guidance. Preserve registrations. | Entry points agree with authoritative records and dated evidence; remaining uncertainty is explicit. |
| 2. Prevent drift | Add read-only schema, ID and dependency checks first. Generate mechanical views and counts from the source. Preserve Markdown-only history before replacing the detailed view. | Stale views, invalid dependencies and duplicate IDs are detected. Invalid updates leave source files intact. |
| 3. Pilot bounded work | Apply task/closeout formats to active work, including a learner workflow and an engineering or maintenance outcome. | Each pilot traces reason -> decision -> evidence -> current guidance -> observation or owned follow-up; overhead is recorded. |
| 4. Compare orchestration | Preregister single-worker versus small coordinated comparisons on comparable isolated tasks, including fresh evaluation cases. | Report correctness, cost, elapsed time, reviewer minutes, rework and failures; expand for a demonstrated useful trade-off. |
| 5. Automate justified mechanisms | Add state/ownership enforcement, resource admission, traces and evidence integration where pilot failures justify them. | Exercise stale ownership, duplicate retries, failed workers and changed bases before unattended operation. |
| 6. Expand selectively | Move to bounded interface work, then independent numerical investigations with prospective protocols. | Benefits persist across broader tasks without weaker scientific standards or growing review queues. |

For each stage, propose a bounded implementation item and owner through existing authorization. Do not mark planned tooling
complete. Until generators and ownership enforcement exist, use explicit manual checks and disclose that limitation.

Review process changes at pilot closeout and after material failures or model/tool changes. Start with a small dashboard:
accepted correctness and escaped errors; outcome observations; reviewer minutes/rework; queue age; documentation contradictions;
total execution cost. Include blocked, cancelled and failed attempts. Use measures to find bottlenecks, not rank workers by
volume. Do not award universal fractional credit to partial features.

Retain a control when its benefit justifies its cost. If a pilot adds insufficient value, reduce agents/artifacts, revise the
rule or restore the earlier workflow through a recorded decision. Preserve evidence, required checks and current guidance
during reversal. Scientific protections do not weaken to improve an orchestration benchmark.

## Foundations and limits

This is our synthesis, not a method collectively prescribed by these authors:

- Cagan's [product risks](https://www.svpg.com/four-big-risks/) and Torres's
  [opportunity solution trees](https://www.producttalk.org/opportunity-solution-trees/) inform outcomes and discovery.
- Singer's [bounded appetite](https://basecamp.com/shapeup/1.2-chapter-03), Reinertsen's
  [flow and delegation](https://proceedings.jacow.org/ipac2011/papers/weib02.pdf), Ries's
  [measured learning](https://theleanstartup.com/principles) and Deming's [PDSA](https://deming.org/explore/pdsa/)
  inform investment, queues and adaptation.
- Nygard's [decision records](https://cognitect.com/blog/2011/11/15/documenting-architecture-decisions) and
  Procida's [Diataxis](https://diataxis.fr/) inform rationale and documentation organized for readers.
- Wooldridge's [multiagent systems](https://www.cs.ox.ac.uk/people/michael.wooldridge/pubs/imas/resources.html),
  Simon's [decomposition](https://www.andrew.cmu.edu/course/15-440/assets/READINGS/simon-architecture-of-complexity-1962.pdf)
  and Lamport's [event ordering](https://lamport.azurewebsites.net/pubs/time-clocks.pdf) inform coordination.
- Cemri and colleagues' [MAST study](https://proceedings.neurips.cc/paper_files/paper/2025/hash/b1041e52d3be19f0a9bc491657488e4a-Abstract-Datasets_and_Benchmarks_Track.html),
  Kim and colleagues' [scaling study, v3](https://arxiv.org/abs/2512.08296v3) and Kapoor and colleagues'
  [evaluation analysis](https://arxiv.org/abs/2407.01502) inform failure-aware, cost-aware local comparisons.

The supporting research distinguishes textbooks, peer-reviewed work, preprints and vendor reports. Treat each as evidence for
specific claims with limits. Keep a small register for adopted mechanisms: source/version, local prediction, measured result
and reconsideration trigger. External results do not establish the optimal swarm for rcfd. Local outcomes must justify it.
