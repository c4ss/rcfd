---
type: entity
tags: [crate]
sources: [doc-architecture, doc-ui-restart-plan, doc-studio-guardrails, adr-thread-studio-engine-entry-points, adr-thread-workspace-layer, doc-example-projects, adr-thread-studio-foundation-crates]
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

### From the P3 ADR threads (2026-11-22)

- `Project::create` is reached through `rcfd project init <dir> [--name <name>]` and MCP `create_project` (ADR 0731); no journal line, "the manifest's `created` is the record" (Q5 (a)) ([[adr-thread-studio-engine-entry-points]]).
- The project trash (ADR 0736): `rcfd_project::trash`, one `.rcfd/trash/<token>/` per item with `item.json` written first and the item moved into `data/` by a rename under the edit lock; no automatic emptying ([[adr-thread-studio-engine-entry-points]]).
- `rcfd_project::journal` (ADR 0188): the journal lock, `seq` and the numbered append; `atomic::replace_if_hash_with` carries the caller's fields ([[adr-thread-workspace-layer]]).
- `rcfd_project::filehash` (ADR 0734): `fnv64:` of a file or of a folder's sorted file hashes, capped at 4 GiB and 100 000 files ([[adr-thread-studio-engine-entry-points]]).
- The committed example projects are instances of the contract, with `check.json`, `fields/` (`index.json`, `store.rts`) and `output/` (`monitors.csv`, `monitors.json`, `report.html`) in each run folder; quick-start and axial-fan keep an empty `meshes/` with a README because the contract "and `rcfd-project`'s validation expect it to exist" ([[doc-example-projects]]). See [[example-project-wind-tunnel]], [[example-project-quick-start]], [[example-project-axial-fan]].
- **Created by ADR 0122 (wave 47A, 2026-10-26, added by p3-records-a):** typed records, atomic replace, JSONL with torn-tail handling, path safety, a poll-based watcher, a migration hook and a query API, "built first, headless, with no UI and no solver dependency"; recorded defect: NaN in `residuals.jsonl` or `status.json` "would make the whole residual file unreadable and drop the run into `skipped`"; "The kill checks test atomicity, not durability across power loss." Audit fixes in ADR 0128 ([[adr-thread-studio-foundation-crates]]).

## Related

- [[rcfd-cli]] — `rcfd project init <dir> [--name <name>]` reaches `Project::create` (ADR 0731).

## Open questions

_Filled in by P1-P3 ingests._
