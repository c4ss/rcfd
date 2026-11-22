---
type: plan
tags: [meta, ingest]
created: 2026-11-22
updated: 2026-11-22
status: stable
---

# Ingestion plan — existing repo documentation

How to compile rcfd's existing docs into this wiki. Follows the ingest loop in `CLAUDE.md`; this page only decides **order, grouping and granularity**. Tick items off here and log each ingest in [[log]].

## Inventory (2026-11-22, `main` working tree)

> [!note] Correction after P0 (read at `origin/main` 2e94e5ef)
> The table below was counted on a local `main` 282 commits behind origin. Upstream differences that change the plan:
> - ~794 ADRs (not 736), and `docs/adr/index.md` is a **generated catalog** (one row per ADR, grouped by roadmap category, with item and item status). P3 uses it instead of writing `tools/adr_catalog.py`. Categories with counts: structural 266, moving meshes 77, quality/docs/validation 40, design studies 35, numerics 24, Studio 15, compressible 13, MCP 9, meshing 9, multiphase 8, turbulence 7, heat 2, post 1, no item in file name 259.
> - `docs/process/` is new: README, `agent-rules.md` (v5), `lead-process.md`, `tolerances.md`, `ledger.tsv`, `exceptions/`, `trials/`, `handover-2026-11-19.md`, `framework-reconciliation.md`; `working-framework.md`, `loop-migration-plan.md`, `ai-swarm-orchestration-framework.md`, `documentation-process-review.md` moved under `docs/process/` (the first three into `framework-2026-11-17/`, superseded in part per the reconciliation page).
> - `docs/adr/briefs/` (agent briefs, ~29), `docs/roadmap/proposals/*.json`, `docs/milestones.json`, `docs/reference-data.d/*.part` (reference-data.md is generated from these).
> - `docs/roadmap.json` is the source of truth; `ROADMAP.md` is generated from it. P2 reads the JSON.

| Corpus | Files | Lines | Notes |
|---|---|---|---|
| `docs/adr/` | 736 | ~260k | ADR 0001-0739, 0800-0801. Status mostly `REGISTERED` (~490), `PLANNED` (~49), `ACCEPTED`, `PROPOSED`. Outcomes live in addenda. |
| `docs/ROADMAP.md` | 1 | 17.2k | 19 `##` sections (Phases 0-9, revision blocks, Electron Studio). ~758 distinct F-items. |
| `docs/*.md` (other) | 46 | ~19k | Reference + "how and why" pages. Big ones: `case-schema.md` 3.1k, `reference-data.md` 2.4k. |
| Repo root | 8 | ~1.4k | README, PRODUCT, DESIGN, CONTRIBUTING, CHANGELOG, SECURITY, licenses. |
| `docs/tutorials/` | 5 | ~1k | Executed by tests. |
| `docs/audit/` | 10 | ~1.9k | Partials audit groups 1-10. |
| `docs/designs/` | 2 | ~0.7k | `exp-rot`, `m-fan-b`. |
| `docs/investigations/` | 126 | — | Mostly json/csv/log/png run outputs; 6 README.md. |
| Misc | ~20 | small | `example-projects/*/notes`, `clients/claude-plugin/skills`, `.impeccable/surfaces`, `.github` templates. |

### Excluded

- `.claude/worktrees/**` (stale copies of `docs/`), `example-projects-linux/` (duplicate of `example-projects/`), `target/`, `apps/*/test-results/`, `scratch-*`.
- `THIRD_PARTY_LICENSES.md`, `COMMERCIAL-LICENSE.md`, `LICENSE`, `NOTICE`: link from the overview, don't ingest.
- `docs/investigations/**` data files: never ingested; linked as evidence from the page that cites them.
- `docs/audit-report.md` is ingested but marked `> [!caution] unreviewed model output` on every claim drawn from it.

## Granularity rules

The gist's "one source at a time, 10-15 pages touched" works for the ~60 prose docs. It does not scale to 736 ADRs and 758 F-items, so:

- **Docs pages:** one `sources/` page per doc. Docs over ~1k lines (`case-schema`, `reference-data`, `ROADMAP`) get one source page per top-level section.
- **ADRs:** one `sources/adr-thread-<topic>.md` page per **thread** (a chain of ADRs on one problem, e.g. the FAN-01 stall series), not per ADR. The thread page has a table: ADR, date, status, registered question, outcome, superseded-by. Individual ADRs get their own page only when they are hubs (cited by 5+ other ADRs or define a lasting convention, e.g. 0001-0005).
- **F-items:** no page per item. One `entities/roadmap-<area>.md` per roadmap section with a status table; a dedicated `entities/f-<nnn>.md` only for hub items (e.g. F-126 fan milestone, cited by 19 ADRs).
- **Current state first:** within a thread, read the newest ADR + addenda first and write the current state, then walk older ADRs for history. Readers mostly want "where are we now".
- **Never paraphrase a registered limit or decision rule loosely.** Quote numbers exactly and link the ADR.

## Phases

### P0 — Skeleton (supervised, 1 session)
Sets the taxonomy every later phase hangs on.
- [x] `README.md`, `PRODUCT.md`, `docs/index.md`, `docs/roadmap-overview.md` → write [[overview]].
- [x] `docs/architecture.md` → `entities/` page per crate (+ [[crate-graph]]).
- [x] `docs/glossary.md` → `concepts/` pages for project terms (20 concepts; [[fan-01]], [[studio]] as entities).
- [x] `CONTRIBUTING.md`, `docs/process/README.md`, `docs/process/agent-rules.md` (in place of `working-framework.md`, moved upstream) → [[registration]] and the evidence concepts. `lead-process.md` and the framework pages move to P1 'Process & meta'.
- [ ] Review the taxonomy with the maintainer before P1.

### P1 — Prose docs, by theme (supervised, one theme per session)
- [ ] **Numerics & physics:** `meshing`, `mesh-quality`, `mesh-import`, `dynamic-mesh`, `structural-solver`, `cosimulation-protocol`, `particles`, `acoustics`, `reactor-and-acoustics`, `gpu`, `component-diagonal-default`, `bfs-convergence-findings`.
- [ ] **Interfaces:** `case-schema` (by section), `solid-case-file`, `run-control`, `studies`, `mcp-tools`, `service-protocol`, `interchange-formats`, `python`, `udf-abi`, tutorials (5).
- [ ] **Evidence & validation:** `validation-data`, `reference-data` (by section), `verification-report`, `provenance`, `parity-matrix`, `test-process`, `releasing`, `CHANGELOG.md`.
- [ ] **Fan programme:** `milestone-fan`, `fan-project-breakdown`, `fan-measurement-protocol`, `design-loop`, `docs/designs/*`.
- [ ] **Studio:** `ui-restart-plan`, `studio-guardrails`, `studio-coverage`, `design-themes-proposal`, `DESIGN.md`, `.impeccable/surfaces`, `clients/claude-plugin/skills`.
- [ ] **Process & meta:** `docs/process/lead-process.md`, `tolerances.md`, `framework-reconciliation.md`, `handover-2026-11-19.md`, `framework-2026-11-17/*`, `documentation-process-review`, `docs/adr/briefs/README.md`, `audit-report`, `docs/audit/*`, `product-suites`, `SECURITY.md`.

### P2 — ROADMAP (batched, light supervision)
- [ ] One source page per `##` section (19), one `entities/roadmap-<area>` page each.
- [ ] Identify hub F-items (cited by many ADRs) → own pages.
- [ ] Cross-link F-items to the concept/entity pages from P0-P1.

### P3 — ADR threads (batched, subagents)
- [ ] Use the generated `docs/adr/index.md` as the catalog (category, item, item status per ADR). Threads follow the item in the file name (`NNNN-fMMM-...`); the 259 records with no item get clustered by title/citations.
- [ ] Cluster into threads using the citation graph + title keywords. Seed list (filename hits): mesh (~48), fan/FAN-01 (~34), studio/UI (~25), structural (~16), compressible (~13), turbulence (~12), cylinder (~10), gate/test process (~9+4), particles (~6), MCP (~6), loops (~6), heat (~5), NACA (~5), VOF, MRF, limiter, cavity, validation, docs. Expect ~25-35 threads.
- [ ] Ingest threads in priority order: FAN-01/fan milestone → validation (cylinder, NACA, cavity, BFS) → Studio → numerics threads → process threads → the rest.
- [ ] Parallel: up to 4 subagents, one thread each. Subagents write only their thread page + the concept/entity pages they own; the main session alone edits `index.md`, `log.md`, `overview.md` (avoids merge conflicts).

### P4 — Leftovers
- [ ] `docs/investigations/*/README.md` (6) → linked from the ADR thread pages that cite them.
- [ ] `example-projects/*/notes` → entity pages for the example projects.

### P5 — Consolidate
- [ ] Full lint (`python tools/lint.py` + judgement pass).
- [ ] Rewrite [[overview]] from the finished wiki.
- [ ] Record the steady-state rule (below) in `CLAUDE.md`.

## Steady state (after P5)

Every new ADR / F-item / doc change merged to `main` gets ingested into its thread or source page in the same session that merges it, and logged. Lint weekly.

## Decisions (maintainer, 2026-11-22)

1. **ADR granularity:** one page per thread.
2. **Supervision:** P0-P1 supervised, P2-P3 batched with up to 4 subagents.
3. **Committing:** the wiki is public. Commit `wiki/` to `main` at the end of each phase.
4. **Steady state:** every roadmap/studio loop merge also ingests its new or changed docs into the wiki (rule in `CLAUDE.md` > Steady-state ingest).

## Ingest backlog

Loop merges that land before their phase is ingested. One line each: date, id, phase.

_empty_
