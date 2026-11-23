---
type: concept
tags: [interface, validation]
sources: [doc-case-schema, doc-tutorial-quick-start-cli, doc-mcp-tools, doc-service-protocol, doc-interchange-formats, doc-solid-case-file, doc-udf-abi, doc-case-schema-registers]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Located error

An error message that names where the problem is: the dotted setting path and, for TOML, the line; or the parameter, file, byte offset or JSON pointer. The convention runs through every rcfd interface ([[doc-case-schema]], [[doc-mcp-tools]]).

## How rcfd uses it

- Case files: `rcfd validate` and `Case::parse_toml` name the key and line, e.g. `case.toml: compressible.rk_order (line 22): ...` ([[doc-case-schema]]); `line 8: reports.lid_drag.patches: unknown setting 'patches'`, exit 3 ([[doc-tutorial-quick-start-cli]]).
- Combinations that are not supported are refused "with a located diagnostic instead of being ignored" ([[doc-case-schema]]).
- Errors list alternatives: `sources.bed.register: unknown register 'x' (registers: core)` ([[doc-case-schema-registers]]).
- Service: error 1003 carries `error.data.diagnostics`, a list of `{path, line, message}` ([[doc-service-protocol]]).
- MCP: "Messages are located: they name the parameter, setting path or file" ([[doc-mcp-tools]]).
- Parsers: `line N` for text, `byte offset N` / `record N` for binary, JSON pointers for GLB ([[doc-interchange-formats]]).
- Solid case file: every problem with its dotted key and line, all problems of a file together ([[doc-solid-case-file]]).
- User functions: loading problems are case diagnostics under `udf.<name>.*` ([[doc-udf-abi]]).

## Related

- [[setup-rules]], [[exit-codes]], [[case-file]]
