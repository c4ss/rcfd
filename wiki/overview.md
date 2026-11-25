---
type: overview
tags: [overview]
sources: [doc-readme, doc-product, doc-index, doc-roadmap-overview, doc-architecture, doc-glossary, doc-contributing, doc-process-agent-rules, doc-roadmap-json, doc-dynamic-mesh, doc-structural-solver, doc-validation-data, doc-reference-data, doc-milestone-fan, doc-fan-project-breakdown, doc-design-loop, doc-mcp-tools, doc-run-control, doc-service-protocol, doc-studio-guardrails, doc-process-lead-process, adr-thread-fan01-rotating-solve, adr-thread-sliding-mesh-rotor-stator, adr-thread-structural-plan, adr-thread-studio-electron, adr-thread-workspace-layer, adr-thread-simulation-milestones-validation, adr-thread-founding-decisions]
created: 2026-11-22
updated: 2026-11-24
status: draft
---

# rcfd — overview

The synthesis after the full initial ingest (P0-P5): all repo docs, the roadmap and all 760 ADRs, at snapshot `2e94e5ef` (2026-11-22), caught up to `origin/main` `5b96a1f8` the same day (ADRs 0762-0766, six changed docs; [[stale-report]] empty). Start here, then follow the links. [[index]] lists every page. The map of the repo's own documentation is [[doc-index]]. What the repo changed since a page was ingested is listed in [[stale-report]]; known gaps in [[open-gaps]].

## What rcfd is

A clean-room, open-source (AGPL-3.0 plus a commercial licence) finite-volume CFD suite in Rust. It has a command-line solver, a solver service, an [[mcp]] server that lets Claude Code drive the solver, and a desktop [[studio]] ([[doc-readme]]). Every algorithm comes from published literature or first principles ([[clean-room]], [[provenance]]). The long-term goal is an open toolset that covers what commercial packages do, starting with CFD ([[doc-roadmap-overview]]). It is early-stage and **not validated against measurement for engineering use**: nothing in 0.1.0 is validated against a measured case ([[verification-vs-validation]]).

## How it is built

- **Crates:** 19 Rust crates in a strict layered [[crate-graph]] ([[doc-architecture]]). The founding conventions still shape everything: a validated settings tree with a content hash, flat SoA fields, and bit-reproducible chunked reductions (ADRs 0001-0003). They are the root of the "defaults stay [[bit-identical]]" rule ([[adr-thread-founding-decisions]]).
- **One run contract** is shared by the CLI, the service, MCP and Python: exit codes 0-8, the status stream, checkpoints, and stop/pause files instead of signals ([[exit-codes]], [[status-stream]], [[checkpoint]], [[stop-file]]). The service [[rcfd-service-protocol]] is loopback-only with no authentication; opt-in layers leave default replies byte-identical.
- **Project folders:** [[rcfd-project]] (L1), [[rcfd-runner]] (L2), [[rcfd-tsstore]] (L3), tied together by the workspace layer [[rcfd-workspace]] (ADR 0173). The workspace layer is the shared core of the MCP server and the Studio: one hash-checked write rule and a [[project-journal]] ([[adr-thread-workspace-layer]]).
- **The Studio** is an Electron/TypeScript app (ADR 0800, 0801). It replaced the egui Studio on 2026-11-16 ([[adr-thread-wave48-egui-studio]]). Claude Code is no longer embedded: since ADR 0801 Addendum 23 the Studio registers the project server with Claude Code and follows its changes through the journal feed. The rule is "engine first": every Studio action has a CLI/MCP twin. 22 guardrails govern the Studio, and it reaches 78.5 % of 1363 engine capabilities ([[studio-guardrails]], [[adr-thread-studio-electron]]).
- **Much of the capability is library-only** and has no case key or CLI/MCP route:
  - overset ([[overset-mesh]]), immersed solids ([[immersed-solid]]) and the planar slider;
  - the acoustics stack (impedance, sponges, wave solver) ([[aeroacoustics]]);
  - AMR/directional refinement and the tet mesher;
  - the co-simulation coupler;
  - the structural solver ([[structural-solver]]).

  Users reach rcfd through the [[case-file]] (49 schema sections, [[doc-case-schema]]), [[run]]s and [[field-store]] playback.

## How work is done

The process is evidence-first and agent-driven, with written roles ([[lead]], [[implementation-agent]], [[maintainer]]) and state kept outside any session: briefs, the [[process-ledger]] and the [[process-report]] ([[doc-process-agent-rules]], [[doc-process-lead-process]]).

- **Roadmap:** work is tracked as [[f-item]]s. `docs/roadmap.json` is the source of truth, and there are 15 lines of work, one [[roadmap]] area page each ([[doc-roadmap-json]]).
- **Registration first:** each item starts with a [[registration]] [[adr]] that fixes its checks and [[registered-limit]]s before any code. Each limit cites a [[tolerance-class]], a reference or the parent's acceptance.
- **Misses:** a miss stays a [[known-miss]] and is never tuned away. "Done" follows the registered rule ([[roadmap-status]]). The maintainer approves lines of work, not single results.
- **Partial by rule:** many items are partial only by that rule (a check met only after an amendment, an equivalent mutant, or the maintainer not yet closing it) rather than by a missed acceptance. That makes a large maintainer queue, mostly in the structural line.
- **Gate:** the full [[gate]] runs daily in CI and attributes failures to ADRs ([[test-to-adr-index]]). Per-merge checks are reduced and recorded in the ledger.
- **Stop conditions on new agents (since 2026-11-21):** three unread reports, the escaped-defect rate, a red gate, or more than 4 agents ([[adr-thread-process-measures-and-roadmap-form]]).
- **Defaults on hold:** the maintainer's answers of 2026-11-18 hold every result-moving default change in the numerics and moving-mesh areas "until the fan path needs it". New keys are approved as additive golden edits.

## Where it stands (origin/main 82985b47, 2026-11-25)

- **Roadmap:** ~~795~~ ~~798~~ ~~799~~ ~~800~~ 801 items (F-886 on main with the wiki from 8732f2ae; F-890 to F-892 entered 2026-11-23; F-893 entered 2026-11-24; F-894 and F-895 entered 2026-11-25), with ~~404~~ ~~409~~ ~~410~~ ~~412~~ ~~413~~ 419 done, ~~299~~ ~~300~~ ~~301~~ ~~300~~ 294 partial and ~~92~~ ~~89~~ 88 not started (419 done at 82985b47; ~70 % at 21e04d2e, F-895 done; ~70 % at d2cc5641, F-894 and F-339 done; ~70 % at 2514444c, F-893 done; ~70 % at 975beadb, F-339 partial; ~70 % at 709e302e; F-861, F-852, F-451, F-891, F-892 done, F-890 partial, the Studio's end-to-end tests no longer failing at random on the lead's machine after F-890 to F-892; ~~791, 391/297/103~~ on 2026-11-22; at the snapshot ~~786, 389/297/100~~) ([[doc-roadmap-json]], [[doc-changelog]]). That counts landed work, not accuracy. The structural line alone has 309 items ([[roadmap-structural-solver]]).
- **Validation milestones:** ten since F-871 (ADR 0775, six added) and F-875 (ADR 0779, fan-structure) ([[simulation-milestones]], [[adr-thread-simulation-milestones-validation]]):
  - **NACA 0012 lift** is met from 0 to 16 degrees and at maximum lift. ~~17 degrees is a known miss with steady SA.~~ F-868 is done: at 17 degrees a time-accurate run (ADR 0764, answering ADR 0760 Q1 option (a)) settles to the attached state with $c_l$ 1.5516 inside the band; the steady route at 17 degrees is F-884, done (ADR 0770): attached with `relax_nut = 1.0` or a global pseudo step; F-887 (ADR 0772) ran the whole lift curve steady with `relax_nut = 1.0`, all twelve angles within the band, and no default changed. The milestone headline is still not met because F-043 (SST) is partial (87 %). The path ran through five investigations that found report and boundary-condition defects, not model defects ([[naca0012]], [[adr-thread-naca0012-sa-drag]]).
  - **Cylinder at Re 100: the first milestone headline met** (vortex shedding, ~~88 %~~ 92 %). The drag is in the band; the observed order 1.387 misses the registered [1.5, 2.5] and is kept as a known miss; ADR 0763 answer (c) made F-869 done, and F-881 (ADR 0771, done) found the order held down by the first-order wall terms of the force integral (1.387 to 1.777) ([[cylinder-re100]], [[adr-thread-simulation-milestones-validation]]).
  - **FAN-01 design point:** the pressure rise is 41.5 % low, while shaft power is within 2.0 %. The band can't be completed until a person reads the measurement's uncertainty and density (F-877). The LDA comparison (F-878, ADR 0765, partial) shows the rotor does the measured work (Euler head 226.86 against 220.02 Pa) and leaves the same kinetic energy, so the missing 52.47 Pa is loss or recovery, which the profiles cannot divide (they miss at 12 of 26 radii). Check H (F-879, ADR 0766) did not decide; the oscillation decays. Since: F-883 done (ADR 0769: casing pressure behind the rotor 113.90 Pa, the rotor's share 32.26 Pa), F-882 partial (ADR 0768: the loss budget closes on four meshes, the extrapolation not valid) and F-885 partial (ADR 0767: check H not evaluable); which side of the measured plane holds the missing rise is still undecided, and whether to issue F-880 (outlet as on the rig) is a question for the maintainer; F-870 now also compares the Euler head, the outflow kinetic energy and the $c_x$, $c_t$ profiles ([[adr-thread-fan01-design-point]], [[doc-reference-data-f878-fan01-lda-profiles]]).
- **FAN-01 rotating solve** has been the central open problem since ADR 0075 ([[fan-01]], [[f-126]], [[adr-thread-fan01-rotating-solve]]). The deficit is attributed to numerical swirl from momentum convection, and the steady bladed run limit-cycles. MRF is blocked on the bladed passage, and wave 49 located the drivers, but every fix was "not accepted". EXP-ROT and M-FAN-B are proposed next steps ([[doc-milestone-fan]]).
- **Computer fan (120 mm):** there is no open reference. It needs a fan measured under [[doc-fan-measurement-protocol]]. Until then the [[design-loop]] (optimisers, [[design-ledger]], [[gaussian-process-surrogate]], [[fidelity-management]], [[hypothesis-record]]) ranks variants of the reduced-order model, not real fans ([[computer-fan-target]]).
- **Sliding mesh** runs from a case file for the full annulus (F-303, ADR 0637) and the sector (F-711, ADR 0646). It is limited to laminar constant-property PISO/PIMPLE with Euler and no restart ([[sliding-mesh]], [[adr-thread-sliding-mesh-rotor-stator]]).
- **Structural solver:** a broad, verified-only library ([[rcfd-fem]], `rcfd_api::solid_case`): static, modal, buckling, dynamics, rotordynamics, nonlinear, contact, fracture and fatigue. It was planned as 66 packages on 2026-11-12 and mostly built within a week ([[adr-thread-structural-plan]]). It has one published-reference comparison (the hinged roof, ADR 0716) and no measured one. The CLI, MCP, flow case and Studio have no route to it yet (F-404, F-405; approved but unbuilt keys F-593, F-605, F-621, F-625, F-842; ~~F-616~~ done, ADR 0785: superelements and interface enrichment from a case file). The milestone fan-structure (ADR 0779) maps 39 items and reads 72 %, not met.
- **Numerics:** November's work was diagnosis plus opt-ins. The only default change was the coarsest dense LU order of the scalar [[amg]] (F-471), approved hash by hash. Transient RANS on PIMPLE exists for [[k-omega-sst]], [[k-epsilon]] and [[spalart-allmaras]]; k-epsilon's BDF2 order miss has no known cause ([[adr-thread-piso-stability-consistent-correctors]], [[adr-thread-transient-rans]]).

## Main threads

1. **Workspace layer** for Claude Code and the Studio (ADR 0173, F-270..F-275), the top priority since 2026-11-04 ([[adr-thread-workspace-layer]]).
2. **Fan programme:** the FAN-01 diagnosis, the design point and LDA profiles, the computer-fan target, and the design loop ([[adr-thread-fan01-rotating-solve]], [[adr-thread-fan01-design-point]], [[adr-thread-design-optimisers-and-ledger]]).
3. **Studio:** the Electron rebuild and its engine entry points ([[adr-thread-studio-electron]], [[adr-thread-studio-engine-entry-points]], [[adr-thread-runner-records-and-run-sites]]).
4. **Structural solver:** user entry points and fan demonstrations ([[roadmap-structural-solver]]).
5. **Validation milestones:** cylinder, NACA 0012, FAN-01 ([[simulation-milestones]]).
6. **Process and tests:** the daily gate, test-to-ADR attribution, the edit lock and stop conditions ([[adr-thread-daily-gate-and-edit-lock]], [[adr-thread-test-to-adr-attribution]]).

## Contradictions

> [!success] Settled — sliding mesh
> [[doc-readme]] and [[doc-dynamic-mesh]] say transient sliding-mesh solves run from a case file. The ADRs confirm it: ADR 0637 c1 met, bit for bit against the in-process run; ADR 0646 done. The "not available" line in [[doc-roadmap-overview]], reviewed 2026-11-18 after both ADRs, is stale ([[sliding-mesh]]).

> [!success] Settled — structural solver
> [[doc-product]] and the Studio plan say a structural solver "does not exist". The library does exist (see above). What is true is that the Studio, CLI and MCP have no route to it and the Studio has no stress/strain colouring. The documentation review of 2026-11-18 (G12) already calls the wording stale ([[structural-solver]], [[roadmap-structural-solver]]).

> [!warning] Stale roadmap entries and docs (within the repo)
> The ingest found many places where an ADR's outcome or the maintainer's answer is newer than a roadmap gap, a status or a doc sentence. Examples:
> - F-474 gap vs ADR 0743 (AMG default switched).
> - F-666 partial vs ADR 0746 done.
> - F-625 "NEEDS THE USER" though approved on 2026-11-18.
> - F-848 todo though phase 1 is built.
> - ~~The F-868 gap and the overview still call ADR 0760 Q1 open.~~ Resolved on `main`: F-868 done (ADR 0764).
> - Guardrail G11 still describes Claude Code inside the Studio.
> - The MCP tool counts (25/42 vs a 36-row table) ([[rcfd-mcp]]).
>
> Each has a callout on its page, and the per-job lists are in the log ([[log]]).

## Open questions for the maintainer (as recorded)

- ADR 0645 Q1: reclassify F-716 from todo to partial.
- ~~ADR 0762 Q4: the compared quantity and convergence rule for FAN-01, deferred until F-878 and F-879 report.~~ Answered 2026-11-22: compared quantities by ADR 0765 Q1 (b); the convergence rule after F-885 reports (ADR 0766 Q2) ([[adr-thread-fan01-design-point]]).
- F-877: a person must read the FAN-01 measurement's uncertainty and air density.
- The default for `solver.component_diagonal` (`averaged` vs `corrected`) ([[component-diagonal]]).
- The structural defaults pending approval (warm-start local update, contact line search, stagnation held) and the entry points F-404/F-405 ([[roadmap-structural-solver]]).
- Close the FAN-01 diagnosis or choose another reference case for the rotating-machinery path ([[fan-01]]).

## Next for the wiki

- ~~P6, Foundations~~ done: 152 foundation pages under the rcfd concepts, from cited open references ([[ingest-plan]]); ~~P7~~ equations in LaTeX done.
- ~~Ingest the backlog after the snapshot~~ done (caught up to `5b96a1f8`). Next: work through [[related-candidates]] (lint judgement pass), the page-structure review, the `raw/` size decision, then merge and switch to the steady-state rule in `CLAUDE.md` with `/wiki-ingest` per loop merge.
