---
type: entity
tags: [interface, service, protocol]
sources: [doc-service-protocol, doc-mcp-tools, doc-case-schema-cosim, doc-case-schema-schema-ui, doc-case-schema-turbulence, doc-run-control]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# rcfd-service/1 (the solver service protocol)

**What it is:** The JSON-RPC 2.0 protocol of the solver service `rcfd serve`, identifier `rcfd-service/1`, current `protocol_version` `"1.1"` ([[doc-service-protocol]]).
**Where:** [docs/service-protocol.md](../../docs/service-protocol.md); served by the `rcfd` binary ([[rcfd-cli]]).

## Details

- Compatibility rule: "a service of any version 1.x answers every request of version 1.0 as before" ([[doc-service-protocol]]).
- Versions: 1.0 base; 1.1 (ADR 0345) adds `cosim.*` (`rcfd-cosim/1`) and errors 1005/1006; 1.2 (ADR 0350) adds opt-in bulk data `data.*` (`rcfd-data/1`), reported only when the embedding program switches it on ([[doc-service-protocol]]).
- Transport: newline-delimited JSON over TCP (`--addr`, default `127.0.0.1:7878`) or stdio; "The service has no authentication", so a non-loopback address is refused with exit 2 unless `--allow-unauthenticated-remote` (ADR 0739 Q9) ([[doc-service-protocol]]).
- Sandbox `--root`: no `..`, no escape through symbolic links; `ServiceConfig` bounds cells, steps per call, design evaluations (default 1000) and event-log size ([[doc-service-protocol]]).
- Session states `defined` -> `ready` -> `running` <-> `paused` -> `finished` | `stopped` | `failed`; settings changes refused (1002) while running or paused ([[doc-service-protocol]]). Live turbulence switching in `ready`/`paused` rebuilds only the turbulence state ([[doc-case-schema-turbulence]]).
- Compare-and-set: `settings.set` / `settings.remove` with `base_value` or `base_hash`, else 1007 (ADR 0736) ([[doc-service-protocol]]).
- Opt-in UI layer `rcfd-ui/1` (`rcfd serve --ui`, ADR 0730): schema labels and help, `help.glossary`, `help.errors`, `mesh.preview`, `case.facts`, `mesh.refine`, `case.save` ([[doc-service-protocol]], [[doc-case-schema-schema-ui]]).
- `rcfd serve --cosim <case>` hosts a fluid participant from the case's `[cosim]` table ([[doc-case-schema-cosim]]).
- The MCP server exposes the service as tools; each tool names its service method ([[doc-mcp-tools]]). `run.headless` runs the same drivers as `rcfd run` ([[doc-run-control]]).

## Related

- [[mcp]], [[rcfd-mcp]], [[project-journal]], [[located-error]], [[status-stream]], [[studio]]

## Open questions

- The page's title says "version 1.1" while it also defines a 1.2; see the contradiction on [[doc-service-protocol]].
