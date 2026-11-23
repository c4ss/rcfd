---
type: source
tags: [overview, docs]
sources: []
origin: docs/index.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: stable
---

# docs/index.md

**Origin:** [docs/index.md](../../docs/index.md) at `origin/main` 2e94e5ef
**Kind:** documentation map

## Summary

The map of the documentation, grouped by intent: tutorials, tasks, reference, how-and-why, status/plans/reports.

## Key points

- Tutorials (executed by tests): quick-start CLI, cavity vs published data, example gallery, MCP design loop, desktop Studio + Claude Code.
- Reference pages: glossary, case schema, run control, studies, MCP tools, service protocol, interchange formats, Python, UDF ABI (sandboxed WebAssembly), mesh import, parity matrix.
- How-and-why pages: architecture, meshing, mesh quality, dynamic meshes, structural solver, co-simulation, particles, acoustics, reactor, GPU, design loop, findings pages, and `docs/adr/`.
- Status pages: roadmap overview, detailed roadmap, fan milestone, fan project breakdown, Studio plan, validation data, provenance, reference data, verification report, product suites, audit report (model-run, unreviewed), test process, releasing.
- HTML site: `cargo test -p rcfd-cli --test docs_site -- --nocapture`.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[overview]]
