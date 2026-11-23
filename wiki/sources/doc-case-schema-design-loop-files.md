---
type: source
tags: [interface, case, design]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: design-loop files (not part of the case schema)

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "Design-loop files (not part of the case schema)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section (a pointer)

## Summary

Operating maps (`rcfd map`), design studies and the `design_*` tools add no section to the case: a map or a study is a driver that overrides settings of a base case (`rcfd map <case> --axis path=v1,v2,...`; `study.json` next to `evaluations.jsonl`). Formats are in `docs/design-loop.md`. The `study` section (ADR 0034) is unchanged.

## Key points

- Two distinct study mechanisms: the case `study` section ([[parametric-study]]) and the design-loop study directory driven by the MCP `design_*` tools ([[doc-mcp-tools]]).

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[parametric-study]], [[mcp]]
