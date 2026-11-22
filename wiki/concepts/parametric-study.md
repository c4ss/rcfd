---
type: concept
tags: [interface, study]
sources: [doc-studies, doc-case-schema-study, doc-run-control, doc-python, doc-case-schema-design-loop-files, doc-mcp-tools]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Parametric study (study section and result table)

A sweep of one case over settings values, defined in the case's `study` section and run by `rcfd study <case>`, one `rcfd run` per design point, into a result table ([[doc-studies]], [[doc-case-schema-study]]).

## How rcfd uses it

- Parameters override any `--set` path; kinds `list`, `linspace`, `logspace`; `combine` `grid` (default) or `zip` ([[doc-case-schema-study]]).
- A point is bit-identical to `rcfd run --set ...` with the same values (tested) ([[doc-studies]]).
- Result table `table.csv` / `table.json`: `point`, parameters, `status`, `exit_code`, `state`, `iterations`, reports, `message`; JSON rows add a FNV-1a `fingerprint`. Byte-identical for any `--parallel`; a resumed study ends byte-identical to a fresh one ([[doc-studies]]).
- Failure isolation: a failing point gives a failed row; the study exits with the lowest-index failed point's code ([[doc-studies]]).
- Driven from Python by `rcfd.study(...)` -> `StudyResult` ([[doc-python]]).
- Not the same as the design-loop studies (`study.json` + `evaluations.jsonl`, the MCP `design_*` tools), which are drivers outside the case schema ([[doc-case-schema-design-loop-files]], [[doc-mcp-tools]]).
- Not delivered: service and MCP tools for `study` sections, chained studies, adaptive exploration; F-095 stays `partial` ([[doc-studies]]).

## Related

- [[exit-codes]], [[rcfd-python]], [[report]], [[case-file]]
