---
type: entity
tags: [crate]
sources: [doc-architecture, doc-ui-restart-plan, doc-studio-guardrails]
created: 2026-11-22
updated: 2026-11-22
status: stub
---

# rcfd-project

**What it is:** Studio layer L1, the project folder: run records, atomic files, path safety, poll watcher. ([[doc-architecture]])
**Where:** `crates/rcfd-project/` — layer 1 (mesh / linalg / project) of the [[crate-graph]].

## Dependencies

- Depends on: [[rcfd-core]]
- Used by: [[rcfd-runner]], [[rcfd-api]], [[rcfd-workspace]], [[rcfd-mcp]]

## Details

- Decision records: ADR 0122 ([[doc-architecture]]).
- Planned as wave 47A, layer "L1 Project format: folder layout and manifest; run records with a case snapshot, mesh reference and hash, solver version, settings, status; atomic writes; schema version and migration", tested by files and unit tests ([[doc-ui-restart-plan]]).
- **The project folder contract** (plan §8, 2026-10-26; "later changes are additive and go through the lead"): `rcfd-project.json` (schema_version, name, created, solver_version_created), `cases/`, `meshes/`, `runs/<run-id>/` with run-id `r-<UTC yyyymmddThhmmss>-<4 hex>` holding `run.json` (immutable), `case.snapshot`, `status.json` (queued, running, paused, finished, failed, cancelled; replaced atomically), `events.jsonl` (append-only), `log.txt`, `residuals.jsonl`, `fields/`, `metrics.json`; plus `designs/` (ADR 0121), `notes/`, `.rcfd/` (runner lock and socket, caches; not versioned) ([[doc-ui-restart-plan]]).
- Guardrail G12 binds the Studio to this contract and ADR 0173; edits made elsewhere show up live ([[doc-studio-guardrails]], [[studio-guardrails]]).

## Open questions

_Filled in by P1-P3 ingests._
