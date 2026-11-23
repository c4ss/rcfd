---
type: entity
tags: [interface, service, protocol]
sources: [doc-service-protocol, doc-mcp-tools, doc-case-schema-cosim, doc-case-schema-schema-ui, doc-case-schema-turbulence, doc-run-control, adr-thread-studio-engine-entry-points, adr-thread-studio-electron, adr-thread-runner-records-and-run-sites]
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

### From the P3 ADR threads (2026-11-22)

- `rcfd-ui/1` (ADR 0730 Q2 (a)) is opt-in behind `rcfd serve --ui` and "the schema and protocol version stay at 1.1"; `service.info` gains a `ui` key and `UI_METHODS` at the end of the method list. `UI_METHODS` grew from `help.glossary`, `help.errors` to `mesh.preview` (ADR 0732) and the methods of ADR 0737 ([[adr-thread-studio-engine-entry-points]]).
- New service error codes: 1007 `CONFLICT` (ADR 0736) and 1009 `MESH_FILE` (ADR 0737); every code has a recovery line in the catalogue `rcfd_api::errors::CATALOGUE` (`help.errors`, `rcfd help errors`), and "A test reads the codes from the sources and fails on one that is not in the catalogue" ([[adr-thread-studio-engine-entry-points]]).
- `session.create` records `source.resolved` and `source.hash`; `journal.save` exists only when journal files are on, so `service.info` and `METHODS` (69) are otherwise unchanged (ADR 0735) ([[adr-thread-studio-engine-entry-points]]).
- `monitors.get {names, since}` builds only what is asked for (F-456): 4 series of 1e5 samples in a median of 82.6 ms (limit 100), an increment of 100 samples in 0.30 ms, with replies byte-equal to the old code ([[adr-thread-studio-electron]]).
- The `data.*` family (R1) was measured in the ADR 0350 spike: a 1e6-cell scalar in "p50 25.9 ms, p99 57.7 ms" against 552 ms for JSON (R0); the Studio uses R2 (its own reader of `store.rts`) for local projects, recommended by ADR 0350 QA2 (a) ([[adr-thread-studio-electron]]).
- `data.open` gains `time_unit: "s"` (ADR 0734) and `length_unit: "m"` (ADR 0738) ([[adr-thread-runner-records-and-run-sites]], [[adr-thread-studio-engine-entry-points]]).

## Related

- [[mcp]], [[rcfd-mcp]], [[project-journal]], [[located-error]], [[status-stream]], [[studio]]

## Open questions

- The page's title says "version 1.1" while it also defines a 1.2; see the contradiction on [[doc-service-protocol]].
