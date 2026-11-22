---
type: source
tags: [interface, case]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: reactor cases and FW-H surface files

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "Reactor cases and FW-H surface files (F-112, F-136)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section (a two-line pointer)

## Summary

`rcfd reactor <case.json>` and `rcfd acoustics <surface.json>` take their own JSON documents, described in `docs/reactor-and-acoustics.md`; "they are not sections of the CFD case file".

## Key points

- Two more input formats besides the [[case-file]] and the [[solid-case-file]]: reactor cases (F-112) and FW-H surface files (F-136).

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[rcfd-reactor]], [[rcfd-cli]]
