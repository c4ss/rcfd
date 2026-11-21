# Documentation process review and unified management framework

> Status (2026-11-21): a proposal of 2026-11-17, landed as it was written; not the rule set in force. What governs, and
> what of this text is adopted, is in [the reconciliation](../framework-reconciliation.md). Only the links to pages
> outside this folder were changed so that they resolve from here.

Review date: 2026-11-17. Evidence baseline: **`65c78c47`** in `E:/projects/cfd/rcfd`.
Status: historical assessment and original proposal. The [working framework](working-framework.md) unifies and replaces
the operating proposal below; the baseline evidence and grade remain historical.

## Assessment

**The process is strong at recording engineering intent and experimental limits, but weak at maintaining a coherent current view and demonstrating user outcomes.** It has the foundations of an evidence-driven management system. Its main improvement is to connect those foundations and close their feedback loops.

Against the framework below, the evidenced process scores **54/100, D (developing)**. This is a purpose-built maturity assessment, not an industry benchmark, a software quality score, or a judgement of the contributors. The strongest dimension is technical evidence; the weakest is current authority and consistency. More documentation alone would not raise the score.

## Scope and method

The workspace root contains several checkouts, scratch directories and external reference material. This review treats `rcfd/` as the active repository. It uses tracked project documents, scripts, workflow definitions, selected tests, decision records and commit history. It does not treat scratch copies as current authority and did not inspect proprietary reference material.

The checkout changed during the investigation. Counts and contradictions below were therefore checked using `git show 65c78c47:<path>` and `git ls-tree -r --name-only 65c78c47`. References link to working files for convenience; their contents may have advanced beyond the baseline.

This is a structural audit with targeted content sampling, not a reading of every record. It examines the founding documentation decision (ADR 0089), gate changes (ADR 0653), the test-to-ADR index, the Studio planning authority (ADR 0800), the accumulated Studio implementation record (ADR 0801), and a recent phased proposal (ADR 0739). It also examines contribution and PR instructions, product and design briefs, user documentation, roadmap representations and release guidance.

The checks performed here were read-only inventory, ID/status comparisons and inspection of automation. Solver, tutorial and CI suites were not executed. Workflow files establish intended enforcement, not proof of a successful hosted run. Missing evidence means “not demonstrated in the reviewed records”; it does not prove an activity never happens elsewhere.

### Baseline inventory

| Measure | Observed |
|---|---:|
| Tracked Markdown files across the repository | 843 |
| Files under `docs/adr/` | 736 |
| Items in `docs/roadmap.json` | 754 |
| JSON item statuses | 354 done; 319 partial; 81 todo |
| Item rows in `docs/ROADMAP.md` | 755, including duplicate F-847 |
| Lines in `docs/ROADMAP.md` | 17,119 |
| Lines in ADR 0801 | 3,483 |

These are measures of information volume and consistency, not productivity. A partial item has no defensible universal fractional value, and these counts do not measure scientific validity or useful product completion.

## How documentation currently governs work

1. **Direction and constraints.** `PRODUCT.md`, `DESIGN.md` and [Studio guardrails](../../studio-guardrails.md) establish audiences, experience, technical constraints and maintainer decisions. [ADR 0800](../../adr/0800-electron-studio-roadmap.md) explicitly maps them into roadmap items and acceptance clauses. This is a valuable, relatively recent improvement over a feature list alone.
2. **Planning.** `docs/roadmap.json` holds IDs, dependencies, acceptance criteria, statuses and narrative notes. [The detailed roadmap](../../ROADMAP.md) presents similar information for readers. The [overview](../../roadmap-overview.md), progress graphics and milestone plans provide other views. JSON is declared authoritative for automated status.
3. **Registration and decisions.** Records under `docs/adr/` describe context, public sources, proposed or accepted choices, numeric limits, tests, exclusions and open questions. Results and later changes often accumulate in addenda. Some records combine a proposal, experiment registration, implementation log and decision history.
4. **Implementation and evidence.** Tests reference ADRs and feature IDs. `scripts/adr_test_index.py` reconstructs relationships using comments, filenames, citations, roadmap notes and Git history. This supports targeted verification and failure attribution, but a recovered relationship is not itself proof of prospective registration or complete acceptance coverage.
5. **Integration.** [Test process section 7.4](../../test-process.md) describes per-merge formatting, linting and the ADR's registered tests, with remaining coverage deferred to the daily full gate. `.github/workflows/ci.yml` runs faster PR checks and Studio tests; `gate.yml` defines the daily/on-demand full Rust gate. [Release guidance](../../releasing.md) explains what these checks do and do not prove.
6. **User guidance.** [The documentation index](../../index.md) already separates tutorials, tasks, reference and explanation. [ADR 0089](../../adr/0089-wave38b-docs-site-tutorials-gallery.md) established executable tutorial blocks and a static site built by `crates/rcfd-cli/tests/docs_site.rs`. The site checks relative page destinations; tutorial runners execute tagged CLI/MCP steps. GUI instructions and other unexecuted steps are disclosed.

The loop is therefore: direction → item → registered work → implementation → tests/results → integration → updated guidance. The last step, and the subsequent decision about value, are less systematically enforced than the engineering middle.

## Philosophical foundations

The framework is **a synthesis for this project**, not a claim that these authors prescribe one shared method. Books identify the underlying bodies of work; linked author or official sources support the particular concepts used here.

| Author and published approach | Principle retained | Consequence for documentation |
|---|---|---|
| Marty Cagan, *Inspired* | Distinguish value, usability, feasibility and business viability risks; address significant risks early. [Four Big Risks](https://www.svpg.com/four-big-risks/) | State which risk a document or test reduces. Engineering success alone cannot establish customer demand or usability. |
| Teresa Torres, *Continuous Discovery Habits* | Connect outcomes to observed opportunities, alternative solutions and assumption tests. [Opportunity Solution Trees](https://www.producttalk.org/opportunity-solution-trees/) | Give active work an outcome and evidence of the problem; preserve alternatives and why one was selected. |
| Eric Ries, *The Lean Startup* | Use experiments and measured learning to decide whether to persevere or change direction. [Methodology](https://theleanstartup.com/principles) | Record the assumption, observation, interpretation and resulting decision. Shipping is an input to learning. |
| Ryan Singer, *Shape Up* | Shape bounded work with an appetite and explicit exclusions; reconsider unsuccessful bets. [Set Boundaries](https://basecamp.com/shapeup/1.2-chapter-03), [Betting Table](https://basecamp.com/shapeup/2.2-chapter-08) | State the investment limit, what may be cut and when work must return for a decision. |
| Donald Reinertsen, *The Principles of Product Development Flow* | Manage queues, batch sizes and feedback; delegate through explicit trade-off rules. His [2011 paper](https://proceedings.jacow.org/ipac2011/papers/weib02.pdf) explains these principles. | Measure waiting for review, testing and decisions; make authority and economic trade-offs explicit. |
| W. Edwards Deming, *The New Economics* | Plan, do, study and act as a cycle of prediction and learning. [Deming Institute on PDSA](https://deming.org/explore/pdsa/) | Preserve what was predicted, study discrepancies and change the process using evidence. |

Two documentation-specific complements make this practical: Daniele Procida's [Diátaxis](https://diataxis.fr/) distinguishes four reader needs; Michael Nygard's [architecture decision records](https://cognitect.com/blog/2011/11/15/documenting-architecture-decisions) preserve concise context, decisions and consequences, including supersession.

### Resolving tensions between the approaches

Discovery stays continuous, while delivery commitments are bounded. An opportunity or hypothesis can remain open without an implementation branch remaining active indefinitely. We adopt Singer's appetite, not a mandatory six-week cadence; cycle length should fit this project's feedback costs.

Scope may shrink; scientific standards may not silently weaken. In CFD, a small useful delivery can be a narrower validated capability or a documented disconfirmation. Registered accuracy limits remain intact; a changed scientific question needs a new, linked registration.

Retain the full capability inventory, but distinguish it from selected commitments. This preserves the Studio's engine-coverage requirement without treating every inventory item as equally urgent. A coverage obligation and the order in which users need it are separate decisions.

Keep experimental history immutable while maintaining editable current guidance. Append-only evidence is useful; append-only policy forces readers to reconstruct the present from history. Supersession links connect both needs.

Delegation should follow explicit decision boundaries. Maintainer accountability can coexist with routine decisions made within approved constraints. The framework does not assume a particular team size or replace existing authority.

## Unified framework: an outcome-to-evidence loop

The unit of management is a **bounded decision with an observable consequence**. Documentation should let a reader answer: why this work, why now, what was chosen, what would count as success, what happened, and what is true today?

| Stage | Minimum durable record | Exit condition |
|---|---|---|
| Frame | Audience, problem evidence, desired outcome, baseline and accountable owner | Someone can explain why solving this matters. Unverified assumptions are labelled. |
| Discover | Significant risks, plausible alternatives, cheapest informative test | Evidence supports a bounded next step, or work is explicitly exploratory. |
| Commit | Scope, exclusions, dependencies, appetite, decision authority, stop/reconsider rule | Work is executable and its investment has a limit. |
| Register and decide | Decision rationale; acceptance clauses and test mapping; prospective scientific limits where applicable | Proposed, registered and accepted states are distinguishable. |
| Deliver and verify | Revision, environment, commands, results, skips, misses and review | Each acceptance clause has evidence or an explicit unresolved state. |
| Publish current truth | Updated status and user guidance; superseded decisions; reproducible examples | Current entry points agree about the delivered capability and its limits. |
| Study and act | Outcome observation, discrepancy, resulting continue/change/stop decision | Learning changes a decision, or a dated follow-up is assigned. |

These are information requirements, not seven new documents or mandatory approval meetings. A small fix may fit in the PR and existing item. A significant architectural change merits an ADR. A numerical claim needs its registration and reproducible evidence. Cross-link rather than repeat.

### Authority and information design

Use one authoritative home **per kind of fact**:

- Product brief: intended audience, outcomes and product principles; design brief: interaction and visual rules; guardrails: constraints and decision rights.
- Roadmap data: item identity, dependencies and delivery status. Store structured links to outcomes, decisions and evidence rather than using narrative notes as the only join.
- ADR: a consequential decision and its rationale, with explicit proposed/accepted/rejected/superseded lifecycle. Keep implementation progress and experiment outcomes distinguishable from decision status.
- Experiment record: hypothesis, prospective protocol and limits, followed by immutable observations. A new protocol supersedes the old protocol without rewriting its results.
- User documentation: the current supported workflow, reference and explanation. Evidence and historical ADRs explain its basis.

Generate the detailed roadmap, numeric summaries and graphics from the same data. Maintain qualitative product explanations editorially, with source links and review triggers. A generated number cannot establish that an explanatory paragraph remains true.

Minimal links for active work: item ID, outcome/opportunity, accountable role, decision/protocol IDs, acceptance-clause-to-check mapping, evidence revision, and affected current pages. Add review date or trigger where the claim is volatile. Avoid backfilling hundreds of dormant items before this is useful.

## Grading

Common maturity anchors: **0** absent or contradicted; **1** isolated/informal; **2** repeatable but incomplete; **3** systematic with evidence and bounded gaps; **4** measured, maintained and improved through demonstrated outcomes.

Weights reflect this project's need for trustworthy scientific evidence and trustworthy current claims. They are judgement calls, published here so they can be challenged. Formula: sum of `weight × score / 4`. Grade bands: A 85–100, B 70–<85, C 60–<70, D 50–<60, E <50. The bands describe this rubric only.

| Dimension and weight | Score / 4 | Evidence supporting the score and limiting it |
|---|---:|---|
| Outcomes and direction — 15 | 2 | `PRODUCT.md`, the fan milestone and ADR 0800 connect work to audiences and purpose. The roadmap schema has no explicit outcome link; delivery counts dominate the overview. |
| Discovery and risk — 15 | 2 | Numerical experiments and feasibility investigations are substantial. `PRODUCT.md` explicitly says there are no customers or usability studies. Value, usability and viability evidence is not systematically represented. |
| Bounded scope and commitments — 10 | 3 | Dependencies, acceptance clauses, exclusions and phased work are well developed; ADR 0739 is a concrete example. Appetite, investment trade-offs and stop rules are not consistent roadmap fields. |
| Technical evidence and traceability — 20 | 3 | Prospective limits, known misses, executable tutorials, ADR-linked tests and gate attribution provide strong controls. Index inference, skips and unexecuted steps prevent treating coverage as complete; this audit did not replay results. |
| Current authority and consistency — 15 | 1 | JSON authority is declared, but duplicated and contradictory current views persist. Acceptance and supersession often require reading long narratives and addenda. |
| Reader usefulness — 10 | 2 | The index follows reader needs and tutorials execute. Its task section admits most task steps still live in reference pages; current summaries conflict, and discovery of present decisions requires substantial reading. |
| Flow and decision economics — 10 | 2 | Gate timings and per-ADR checks show deliberate reduction of feedback costs. No systematic review-age, decision-wait or documentation-rework measures were found in the sampled process. |
| Learning and process adaptation — 5 | 2 | Audits, gate redesign and recorded misses show adaptation. Product outcome follow-ups and documented effects of process changes are less complete than technical results. |
| **Weighted total** | **53.75 → 54/100** | **D: developing.** |

Confidence is high for the inventory and reproduced contradictions, medium for repo-wide maturity inferences, and low for actual user success or review lead times because those were not measured. Raising both outcome and discovery scores by one would add 7.5 points; the exact total should not obscure the dimension-level diagnosis.

### Concrete findings

| Finding at the baseline | Evidence | Consequence |
|---|---|---|
| Overview reports 190 items, 95 done, 63 partial, 32 not started; JSON contains 754/354/319/81. | [Overview](../../roadmap-overview.md), `docs/roadmap.json` | The main status entry point cannot reliably orient a reader. |
| F-847 appears twice in detailed Markdown, once partial and once todo; JSON says partial. | [Detailed roadmap](../../ROADMAP.md), item F-847 | Declaring a source of truth has not prevented divergent presentations. |
| Product and overview say no structural solver exists; the structural page describes `rcfd-fem` and its supported tier. | `PRODUCT.md` → Capabilities; [overview](../../roadmap-overview.md) → What does not work; [structural solver](../../structural-solver.md) → Scope | Library existence, interface availability and physical validation need separate claims. This does not establish that the solver is validated. |
| F-001/F-002 retain CI caveats dated 2026-10-14. ADR 0653 documents later hosted runs of the separate full gate. | `docs/roadmap.json`, [ADR 0653](../../adr/0653-f723-registration-daily-full-gate-with-failures-attributed-to-adrs.md) → What was read | Refresh the notes from evidence for the named workflow, platforms and checks. A recorded `gate.yml` run does not establish execution of `ci.yml` or satisfy both items' acceptance. |
| “Full gate before each merge” persists in the overview, while current test-process section 7.4 describes targeted merge checks and a daily full gate. | [Overview](../../roadmap-overview.md) → What works; [test process](../../test-process.md) → 7.4 | Contributors may act on different integration rules. |
| ADR 0801 contains 3,483 lines spanning dependencies, implementation, questions, results and changed behaviour. | [ADR 0801](../../adr/0801-studio-web-dependencies-and-layout.md) | History is rich, but current authority is expensive to reconstruct. Length is a symptom, not a defect on its own. |

The site checker excludes external links and same-page anchors from destination checks and strips fragments when resolving page links. It builds from `docs/`, which leaves root briefs outside its scope. These checks cover link destinations. `scripts/roadmap_progress.py` generates the two graphics. The later [loop inspection](loop-migration-plan.md) identified a scratchpad script that patches individual Markdown rows alongside JSON. The overview's generation claim should distinguish that incremental update from a reproducible full regeneration.

## Improvement sequence

Suggested roles below describe responsibilities, not additional headcount. The maintainer may hold several roles. Proposed targets are pilot acceptance criteria, not measured baselines.

| Order | Change | Accountable role | Evidence that it worked |
|---|---|---|---|
| 1: Restore current truth | Reconcile the six findings above against actual evidence. Preserve historical registrations; update their current status summaries. Give each authority a current entry point and supersession links. | Maintainer + documentation steward | The overview, JSON and detailed roadmap agree; F-847 is unique; capability and gate claims resolve to dated evidence. |
| 2: Prevent mechanical drift | Add schema/unique-ID/dependency checks and deterministic roadmap generation with a check mode. Generate counts and graphics together. Check relevant docs on PRs without waiting for the full numerical gate. | Tooling owner | A deliberately duplicated item, status mismatch or stale generated file fails a focused check; normal changes regenerate cleanly. |
| 3: Make current decisions cheap to retrieve | Create a small accepted-decision index by subject. Split future consequential decisions from ADR 0801; link its historical addenda rather than rewrite their evidence. Assign owners and change triggers to volatile current pages. | Documentation steward | A fresh reader can identify current runner lifecycle, engine location and platform claims, and cite their authority without reconstructing the entire ADR. Pilot target: within five minutes per question. |
| 4: Connect work to value | Pilot one learner outcome and one engineering outcome. Record observed problems, alternatives, major risks, appetite and stop rules for their next items. Keep library capability, UI availability and validation status separate. | Product decision owner | Each selected item has an outcome link and a planned observation. Genuine user evidence is labelled separately from maintainer experience and hypotheses. |
| 5: Close delivery and learning | Require a short closeout: acceptance coverage, evidence revision, skips/misses, affected docs and outcome follow-up date. Add doc/evidence impact to the PR template. Measure review waits and reader task success. | Implementer + reviewer | A completed pilot traces from problem to decision, tests, current guidance and follow-up; resulting learning changes or explicitly confirms the next decision. |

Start with active work. Do not halt development for an archive migration. Preserve the existing prospective scientific limits and executable tutorials throughout.

Useful measures: duplicate IDs and generated drift (target zero); unresolved conflicting current claims; age of awaiting-review decisions; time from evidence to accepted decision; acceptance clauses with explicit evidence or unresolved status; time and errors for a newcomer completing a task; and pilot outcomes observed after delivery. Track skipped and unverified checks separately. Commit count, ADR count and a global percentage of equally weighted features are insufficient substitutes.

A proportionate closeout can be only a few lines: **decision/item → acceptance clauses and checks → revision/environment/results → misses and skips → current pages updated → outcome observation and next decision**. Use the same links in the existing PR and records, rather than creating another narrative copy.

## Example pilot and boundary for the next discussion

For the learner's first path, frame the problem as an assumption until observed: a learner may struggle to reach a first result and understand its limits. Compare an in-app starter, an improved executable tutorial and a guided case setup before committing. Choose a small delivery with an explicit appetite. Keep solver verification separate from whether the learner understands the result. Observe real participants where available; record maintainer-only trials honestly. Set the success threshold prospectively after defining the task and pilot sample, then publish what happened and decide the next step.

This framework deliberately stops before designing AI swarm orchestration. Its later adaptation should preserve the distinctions between **authority, a claim, independent evidence, delivery and user value**. The first-principles questions for that phase are:

1. Which scarce resource dominates when drafting and coding become cheaper: review attention, independent experiments, compute, integration or real user access?
2. What decision rights and shared state let parallel workers make aligned choices without serializing every action through the maintainer?
3. When do multiple agents provide independent evidence, and when do shared models, sources and tests create correlated agreement?
4. Which historical assumptions about team size, cycle time and communication costs change, while the need for trustworthy measurement remains?
5. How will increased throughput affect queues, document consistency, duplicated work and the cost of accepting a false claim?

The next phase can derive an orchestration model against this baseline, rather than assuming more agents automatically improve the management loop.
