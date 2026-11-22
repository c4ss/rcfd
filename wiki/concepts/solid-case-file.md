---
type: concept
tags: [interface, structural, case]
sources: [doc-solid-case-file, doc-case-schema-reactor-fwh-files]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Solid case file

A TOML file for a linear solid model of [[rcfd-fem]], separate from the flow [[case-file]]: the flow reader refuses it and the solid reader refuses a flow case (F-386, ADR 0438) ([[doc-solid-case-file]]).

## How rcfd uses it

- Run by the library function `rcfd_api::solid_case::run_solid_case_file(path, out_dir)`; "There is no CLI subcommand or MCP tool for it yet" ([[doc-solid-case-file]]).
- Static or modal analysis; direct or CG solver; outputs a `.vtu` and a `rcfd-solid-report/1` JSON report ([[doc-solid-case-file]]).
- One of several input formats besides the flow case: reactor cases and FW-H surface files also have their own JSON documents ([[doc-case-schema-reactor-fwh-files]]).

## Related

- [[rcfd-fem]], [[case-file]], [[located-error]]
