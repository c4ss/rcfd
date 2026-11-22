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

# Case schema: viscoelastic models and structural load export

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "Wave 27C: viscoelastic models and structural load export (F-135, F-237, ADR 0051)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section (short)

## Summary

"No case-tree keys were added." `rcfd_fv::viscoelastic`, `rcfd_post::structural`, `rcfd_io::nastran` and `rcfd_io::registry` are library-level APIs only; their errors are located by the API argument. "A case-tree section is future work."

## Key points

- The load-deck format is described in [[doc-interchange-formats]].

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[interchange-formats]]
