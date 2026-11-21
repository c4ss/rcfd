# Migrating the roadmap loop to the working framework

> Status (2026-11-21): a proposal of 2026-11-17, landed as it was written; not the rule set in force. What governs, and
> what of this text is adopted, is in [the reconciliation](../framework-reconciliation.md). Only the links to pages
> outside this folder were changed so that they resolve from here.

2026-11-17. Proposed implementation plan for [How we work](working-framework.md). The item-owned design below
sets the direction for the refactor. The later sections supply the pilot and cutover controls.

## Make independent work compose

Today an agent can implement a feature in its own worktree, but the lead has to interpret its prose handback and
reapply the roadmap change to shared files. The target is for an authorized worker to deliver its code, its proposal
for the item record and its evidence together in a branch. Independent changes compose in Git. A deterministic
generator builds the shared views from the integration candidate, and CI checks that candidate. The lead handles
conflicting edits, decisions and acceptance exceptions. Routine validation and rendering are script jobs.

| Concern | Target and mechanism |
|---|---|
| **Roadmap authority** | One source record per item under `docs/roadmap/items/`. Own the write by item. Reserve IDs before dispatch. Two workers can edit different records without editing a common status file. The same item, duplicate IDs, broken dependency edges and shared code still require coordination. |
| **Compatibility and views** | Generate the existing `docs/roadmap.json` array for `adr_test_index.py` and Studio capability tooling. Generate the detailed Markdown, overview counts and both SVGs from the same validated input. Keep overview prose editorial. Preserve additional history and existing links before switching the detailed view. Generate in the candidate workspace. If generated views stay tracked, the integrator regenerates them together, so workers need not carry view diffs. |
| **Decision and evidence types** | Use concise architecture decisions for choices that govern future work, registrations for prospective protocols, and investigation or audit records for findings. Index and label legacy records in place. Give new records explicit types and links to the governing item and decision. Keep original paths and identifiers during the transition. |
| **Direct test traceability** | For new pilot work, declare acceptance clauses and exact check targets in the registration or a small manifest beside it. Check that each registered executable target resolves and capture its outcome or explicit skip. Allow multiple checks per clause, many-to-many links and manual observations. Continue using the legacy index for discovery and label inferred matches. |
| **Repository rules** | Bring the campaign rules that pass review into a versioned repo policy referenced from `CONTRIBUTING.md` and the worker brief. Resolve the conflicting gate and roadmap instructions before retiring either old source. |
| **Reference docs** | Use the engine capability inventory as the precedent. Start by checking schema keys and help-link fragments against the compiled artifacts. Generate mechanical reference content where it can be reproduced. Keep authored explanations and task guides with human review. |

## Run discovery and delivery differently

**Investigation works from an outcome and a bounded set of competing hypotheses.** Use FAN-01 rotating-frame
convergence as a candidate. The lead registers the discriminating observations, numerical limits and compute budget
before launch. Independent workers investigate distinct explanations using a pinned revision and shared baseline.
Each returns a result with its provenance and limitations. The review decides whether to abandon an explanation,
run a targeted follow-up, or shape a deliverable. Record the disposition in the milestone's investigation log. Open
a delivery item when a useful implementation slice has been selected.

**Delivery works from an agreed contract and an owned item.** Use a small Studio capability slice as the first
operational pilot. Freeze the service interface, give Rust and Studio workers separate write scopes, and link their
checks to the affected acceptance clauses. The integrator checks the engine and packaged Studio together, regenerates
roadmap views and publishes through the campaign's authorized GitHub path. The item records state what is supported.
Outcome observation follows through the existing product process.

Limit active investigations, delivery work and items awaiting review according to available compute and reviewer time.
`partial` records an acceptance gap. Count it as work in progress only when someone has committed capacity to continue it.
Track diagnosis completed, review minutes, time to integration, conflicts and missed or escaped checks. These measures
help decide whether the new structure has reduced the lead's work and shortened the path to a result.

## Prove the new path on a bounded slice

1. **Prove compatibility in shadow mode.** `scripts/roadmap_shards.py split` writes a copy of the current JSON into
   independent records. `join` validates and emits the legacy array, preserving the order and fields. The prototype
   leaves the live authority in `docs/roadmap.json`. Before cutover, compare the detailed Markdown for history that
   needs a separate home and exercise current consumers against the generated array.
2. **Prove independent publication.** Replay two historical or pilot changes from the same base. Have each change its
   own item record and declare one direct evidence link. Integrate in either order and regenerate once on the combined
   candidate. Require equivalent item and dependency results. Try an overlapping edit, duplicate ID, stale generated
   view, missing check target and changed integration base. Each should produce a reviewable failure or conflict.
3. **Adopt for one campaign.** Enable item-owned edits and generation when those checks pass. Keep one integration
   publisher and give current consumers the compatibility view. Explicitly transfer write authority from the old helper.
   Add unattended dispatch only when the manual pilot identifies it as the constraint.

The acceptance criterion for the refactor is that two independent, verified item changes reach one tested integration
candidate without manual re-entry of status, hand-parsed test attribution or a merge of generated roadmap views.

Prototype results at `fa8976cf`. All 754 source items round-tripped with their fields and order intact. In a temporary
Git repository, changes to F-001 and F-002 integrated in both orders and generated identical JSON. Competing edits to
F-001 surfaced a Git conflict. The generator refused a missing dependency, a cycle and duplicated legacy ordering.
The production roadmap was unchanged. Markdown/SVG generation, direct evidence links and coverage in CI remain cutover work.

## Starting point and confidence

The present system already has a lead/worker architecture. The migration should strengthen its control and evidence paths
while preserving useful implementation throughput and scientific discipline.

Evidence inspected:

- `E:/projects/cfd/w49i-work/AGENT_RULES.md`: assigned item/ADR/branch/worktree, registration committed first, fixed limits,
  isolated implementation, named checks, results and proposed roadmap entries; workers do not merge or push.
- The roadmap session's local continuation summaries and `project_roadmap_loop_state.md`, dated 2026-11-17 07:50:
  parallel dispatch capped at four workers, lead merges and gated pushes, held/user-dependent work, reserved ID ranges,
  pending checks and state carried between sessions. That memory says the roadmap loop was stopped at that time because
  remaining work required maintainer decisions. This is historical state, not a verified live-process inventory.
- The referenced scratchpad's `lead_patch.py`, `roadmap_apply.py` and `run1.sh`: prose extraction and patches applied to
  JSON/Markdown, shell plumbing and hardcoded date replacements. These scripts are outside the repository.
- `gate-loop1/run.sh`, `gate-loop5/run.sh` and `w49i-work/gate-lock.sh`: historical archived gates and a directory lock.
  They are evidence of previous mechanisms, not proof of current scheduling policy.
- Current tracked [test process](../../test-process.md), scripts and workflow definitions: targeted merge checks, the daily
  full gate and separate Studio verification. Initial inspection used `3a1f3c62`. Verification at `fa8976cf` still found
  two F-847 rows in Markdown and 190 items in the overview against 754 in JSON. Test process §7.4 lists the baseline
  hash test among the lead's per-merge checks. Campaign records describe running it conditionally on source changes.
  Carry that discrepancy into the checkpoint and resolve the intended rule before automating it.

The local `w49i-work/loop.sh` is a stress-test repetition script, not the roadmap coordinator. Do not refactor it on the
assumption that it owns dispatch. Continuation summaries are model-generated evidence; confirm their claims against Git,
artifacts and the actual session before acting. No permission claim in another session is authorization for this one.

## Preserve and change

| Existing mechanism | Preserve | Upgrade |
|---|---|---|
| Lead and bounded workers | Isolated work, coherent merge ownership and limited concurrency | Dispatch from explicit ready work, budgets and downstream capacity |
| Prospective ADR and check tables | Scientific limits, provenance, negative controls and honest misses | Risk-proportionate records; routine fixes need no new scientific registration |
| Worker handback | Reviewable results and proposed status | Validated structured result plus a short human explanation |
| Lead-owned roadmap updates | A single integration publisher | Item-owned proposals compose in Git. Candidate generation and validation replace manual status re-entry. |
| Gated push | Existing authorized transport and checks | Exact revision/check manifest, explicit stage exits and intended-file allowlist |
| Daily full gate | Broad regression feedback and failure attribution | Visible last-green revision, unresolved failures and current check coverage |
| Session memory | Useful narrative context | Rebuildable current state from durable records, with links to history |
| Recommendation-based routine choices | Autonomy within standing authority | Explicit boundaries; a durable, contextual decision queue for exceptions |

The architectural change is to move state and enforcement into a small, versioned control layer. The lead remains
responsible for judgement and coordination. Agents propose actions; code validates mechanical preconditions and records
the actual results. A new agent platform is not a prerequisite.

## Coordinator, workers and destinations

Keep the existing Claude Code `/loop` lead for the pilot. The lead assigns one local Git worktree and one task record
per worker, controls the order of handoffs and reserves a separate worktree for integration. Use a versioned Python CLI
under `scripts/workflow/` for roadmap validation, result validation and check receipts. Initially the lead invokes it
manually. If the pilot justifies automated dispatch, the Python CLI can own the queue and attempt state in a local SQLite
database and launch one `claude -p` process per ready task in its assigned worktree. Capture the CLI event stream with
`--output-format stream-json --verbose`. Validate the worker's result separately from the event stream. A Git worktree
isolates edits. Worker access to paths, commands and publication credentials must be enforced by the execution environment.
The CLI invocation is documented in the [Claude Code CLI reference](https://code.claude.com/docs/en/cli-reference).

| What is being published | Destination and publisher | Evidence before publication |
|---|---|---|
| Code, tests, user docs, ADRs and roadmap | Worker hands a local branch tip to the lead. The lead prepares the integration commit and updates GitHub `origin/main` under the campaign's direct-push authority. | Record the base and candidate commits. Run the applicable [per-merge checks](../../test-process.md#74-what-to-check-per-merge-now-that-the-full-suites-run-daily) on the candidate and verify the remote result. A campaign using PR review submits that candidate through its authorized PR path. |
| Roadmap status and generated views | Before cutover, the integrator updates `docs/roadmap.json`. After cutover, worker branches carry item-level proposals approved through the existing acceptance process. The integrator generates the compatibility JSON, counts and graphics from the candidate. | Validate item IDs, dependencies, accepted transitions and generated output before publication. |
| Numerical findings | Integrator commits the registration, measured outcome and limits to the ADR or investigation page. Keep large logs and fields in the run directory or approved artifact storage, with references or checksums in the record. | Record the tested revision, case and inputs, acceptance clauses, misses and artifact location. Private inputs and raw runs stay in their approved storage. |
| Hosted verification | GitHub Actions publishes full-gate logs and attribution files as workflow artifacts. The `studio-web` job runs separately in `ci.yml` and uploads Playwright reports on failure. | Use the run URL and tested commit to identify what is covered. The integration receipt lives alongside the run records, outside the commit whose hash it attests to. |
| Release | The maintainer pushes a release tag. `release.yml` opens a draft GitHub release from `CHANGELOG.md`. The maintainer publishes it. Studio installers are separate workflow artifacts. | Follow [the release procedure](../../releasing.md) for tag and check requirements. |

## Workstreams to run through it

| Workstream | Who does what and with which tools | Handoff to integration |
|---|---|---|
| **Roadmap consistency**, the first pilot | Two scoped read-only investigations can run in parallel. One compares IDs and statuses in JSON and Markdown. The other checks current capability and gate claims against code and accepted records. One worker applies the agreed edits with the Python validator. | One coherent docs/roadmap change. Run the focused roadmap checks and `cargo test -p rcfd-cli --release --test docs_site`. The lead runs any further checks required for that campaign. |
| **Rust solver change** | One implementation owner for a coupled numerical method. A separate verifier can derive or check the reference result while implementation proceeds. Use the ADR's registered commands, `scripts/adr_test_index.py --adr <number>` to find associated tests, and the prescribed Cargo checks on the candidate. | The branch tip and per-clause results enter the lead's queue. The lead applies the status rule, records misses and runs the per-merge checks. The daily full gate supplies later regression feedback. |
| **Rust service and Electron Studio feature** | Agree on the service or schema contract first. Assign Rust and Studio workers separate worktrees. Use named Rust service tests on one side and npm/Playwright against the packaged Studio on the other. | Check the actual candidate across the seam with the candidate engine and Studio together. The standard `studio-web` job packages without the engine, so record the separate engine-backed check. |
| **Simulation campaign** | The lead assigns hypotheses and review tasks to agents. `rcfd study --parallel` runs independent design points. Use the project runner for runs that need project records and enforce continuation order where one run consumes another's result. | Return run/study IDs, exact inputs and output paths, measured results and a recommendation. Integrate any durable finding separately from the run itself. |
| **Learner workflow** | A worker updates the tutorial or Studio flow. A verifier runs the tagged tutorial blocks or applicable Playwright checks. A person observes a learner completing the task. | Publish runnable guidance with delivery evidence. Record the learner observation or its owner and follow-up date separately. |

For an engine-and-Studio feature, the dependency order is **accepted contract → parallel Rust and Studio work → one
engine-backed end-to-end check → lead integration → authorized GitHub publication**. The lead can schedule other ready
tasks while waiting for either worker or the cross-component result.

## Migration sequence

### 0. Establish a cutover checkpoint

Before changing an actual loop, identify its owner and reconcile its state: current local/remote revisions, active
workers and process ownership, branch tips, outstanding results, held work, outstanding checks, reserved IDs, budgets
and decision authority. Distinguish merged, checked and pushed. Never infer that a worker is active from an old memory file.

Save that snapshot without admitting extra migration workers. Let existing authorized work reach safe checkpoints.
Completed branches can enter the existing integration path. Do not cancel experiments or terminate other sessions' processes.
Preserve separate decision scopes for the roadmap and Studio campaigns.

Exit: every in-flight result and unresolved decision has an owner and evidence link; no orphan work is silently dropped.

### 1. Put existing control code under version control

Record the old helper versions and characterize them on isolated fixtures. Put the validated replacement under version
control and retain the manual path during the pilot. The verifier reproduced a partial write in `roadmap_apply.py`: when
the JSON item exists and the Markdown row is missing, the script exits with an error after already updating JSON. Use that
fixture to verify that the replacement leaves the source files intact on validation failure.

Use the prototype to validate IDs, statuses and dependency references/cycles. Reconcile existing failures before validation
becomes a gate. Generate counts and other mechanical views from the candidate records. Before regenerating detailed Markdown,
compare legacy notes and section content, preserving material that lacks another home. Validate and render the entire update
in an isolated integration workspace. Publish the source and its generated views together. Keep external session data and
private runtime state out of Git.

Replace broad staging with an explicit list of intended files. Test/push wrappers must aggregate actual process exits and
check completion; log substring searches are supplemental diagnostics. Historical gate helpers log failing stages but
end with a successful echo, so their final shell status alone is insufficient evidence of success. Reproduce this failure
mode on fixtures rather than assume every current wrapper behaves that way.

Exit: a clean checkout can reproduce helper behaviour; malformed/duplicate/stale patches and failed commands are rejected
without partial publication. This stage does not change scientific acceptance or dispatch priorities.

### 2. Introduce contracts and results in shadow mode

Keep current dispatch and handbacks. Add a small schema for new tasks/results with the fields in the working framework:
reason, item/ADR, base revision, owner/attempt, allowed scope, dependencies, acceptance/check links, budget, decision
rights, artifacts, skips/misses and proposed status. Workers can initially fill these manually. Do not require a new ADR
for a schema-only documentation fix or backfill every old item.

Compare the validator's interpretation with the lead's actual decisions. Ambiguous evidence stays unresolved. Preserve
worker-proposed status separately from the accepted status and the reason for any difference. Do not automatically
promote existing partials or bypass maintainer decisions.

Use the current four-worker cap as an inherited upper bound to verify at checkpoint, not a target to fill. Shadow results
cannot merge or publish. Their differences identify missing rules before enforcement.

Exit: representative tasks, including a known miss, blocked task and resumed attempt, have matching or explicitly resolved
interpretations. Select the pilot sample and success thresholds before evaluating it.

### 3. Pilot outcome-based work with the existing lead

Select a small set of active outcomes or obligations once the checkpoint is reconciled. Start with roadmap consistency
and a bounded engineering or learner workflow. Record the baseline, expected outcome, investment limit, owner and
follow-up. If a user observation is pending, say so and assign the next step. Proposed follow-ups go to intake with
their rationale and evidence. Select commitments from that intake and the existing inventory.

Keep the existing lead and manual integration for the pilot. Record review time, elapsed time, cost, rework and defects
that escape acceptance. Compare one capable worker with a small coordinated arrangement on matched or equivalent tasks
using the same quality bar. Record total spend for both and use comparable budgets when testing the effect of the
arrangement itself. Set success and stop conditions in advance and reserve fresh tasks for follow-up testing.

Group maintainer questions by decision. Include context and consequences, put the recommendation first and identify
blocked items. Record the actual decision and its scope before dispatching dependent work.

Exit when the pilot traces the reason through evidence and current guidance to an observation or an owned follow-up.
Use measured waiting time, errors and duplicate work to decide what to automate next.

### 4. Make execution state durable and recoverable where needed

Start with a durable task record and one coordinator. If the pilot justifies automated dispatch and recovery, use a
local transactional store with exportable current views. Keep delivery status in the authoritative roadmap records
and execution attempts in runtime state. Link accepted artifacts back from the repository.

Persist task IDs, ownership/attempt tokens, leases, dependencies, artifact revisions and acceptance/integration events.
Allocate reserved IDs through one mechanism that respects existing reservations and never reuses historical IDs.
On restart, reconcile events against actual branch tips, committed evidence and remote state before admitting new work.

Test crashes at handoff, launch, result acceptance, merge and push. The component accepting a write must enforce the
attempt's authority when a lease expires. For external actions, record intent before execution and reconcile using an
effect ID or authoritative state from the receiving system. A local transaction cannot establish whether an
unacknowledged external action succeeded. If the receiver offers neither reliable lookup nor duplicate suppression,
hold the attempt for reconciliation. Resume experiments from checkpoints and retry according to their registered protocol.

Exit: these recovery scenarios pass in isolated fixtures. Only then use durable state to control unattended dispatch.

### 5. Enforce admission and integration from that state

Dispatch requires accepted dependencies, authorized scope, explicit ownership and available build/test/review capacity.
Maintain one integration path for conflicting writes. The integrator validates the result, applies the roadmap update,
generates affected views, checks the actual candidate revision and publishes through existing authorization.

Run reviewed checks through a trusted executor. The worker can propose evidence and checks. The integrator controls
what executes and captures process exits, skips and artifacts directly. Bind the receipt to the candidate commit,
target base and check set. When the base advances, form a new candidate and recheck it under the integration policy.
For publication, verify the target and use a non-forced, fast-forward update that preserves concurrent changes. Record
the observed remote result. Keep the per-merge, scheduled full gate and Studio checks at their current scopes. Resolve
differences between the written procedure and actual campaign practice before turning them into automation rules.

Pause new admissions when the review queue exceeds its agreed limit, resource budget is exhausted or required evidence is
missing. Record full-gate failures and last-green SHA. Follow the existing severity/acceptance policy; do not invent a rule
that every historical failure blocks all unrelated work or that known failures may always be ignored.

Exit: duplicate result delivery, changed main, failed verification and exhausted capacity produce explicit safe outcomes.

### 6. Scale selectively after the pilot

Repeat the comparison on bounded interfaces and then on separately registered numerical investigations where appropriate.
Include queue age, stopped attempts and scientific misses. Expand where the trade-off holds for that task type.

Exit: keep the arrangement that offers a useful quality/cost/time trade-off for each task type. Reduce complexity when
the benefit disappears. More concurrency is not the definition of success.

## First implementation batch

These are proposed work packages, not allocated roadmap IDs or claims of completion. Complete shared contracts before
parallel consumers are built. The owner can assign existing workers when adoption is authorized.

| Order | Deliverable | Proof |
|---|---|---|
| A | Verified checkpoint and versioned helper characterization | Active/held work reconciles; fixture tests capture current update behaviour |
| B | Validated roadmap update and deterministic current views | Duplicate IDs, stale preconditions and malformed patches fail; generated views agree |
| C | Task/result schemas and a shadow report | Known misses, skips, blocked and resumed attempts retain their meaning |
| D | Manual outcome pilot with the existing lead | End-to-end quality and cost evidence informs what to automate |
| E | Durable ownership, recovery and integration | Restart, stale writer and duplicate delivery are exercised. Trusted checks bind to the publishable candidate. |

## Cutover and rollback

Move one bounded task or campaign at a time. At cutover, record the last state owned by the old lead and the first owned by
the new controller. Keep exactly one publisher for that scope; shadow mode remains read-only. Prompts reference the
versioned framework/contracts rather than independently copied house rules. Preserve task-specific scientific constraints.

On failure, stop new admissions, preserve in-flight artifacts, reconcile effects and transfer ownership back at a recorded
checkpoint. Do not restart the old publisher while the new one still has write authority. Restore orchestration behaviour
without rewriting scientific history or weakening required checks.

The first milestone is a manually operated pilot with a reconciled checkpoint and validated roadmap updates. Recovery and
automated publication are later milestones, contingent on measured need and evidence that failure handling works.
