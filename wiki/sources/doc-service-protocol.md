---
type: source
tags: [interface, service, protocol]
sources: []
origin: docs/service-protocol.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# rcfd solver service protocol, version 1.1

**Origin:** [docs/service-protocol.md](../../docs/service-protocol.md) at `origin/main` 2e94e5ef
**Kind:** protocol specification

## Summary

The solver service `rcfd serve`: transport, error codes, session states and every method. Identifier `rcfd-service/1` (`service.info.protocol`); a service of any 1.x answers every 1.0 request as before. See [[rcfd-service-protocol]].

## Key points

- Versions: 1.0 everything except `cosim.*`; 1.1 (ADR 0345) adds `cosim.*`, errors 1005/1006, `protocol_version` and `cosim` in `service.info`; 1.2 (ADR 0350, "a measurement spike") adds opt-in bulk data `data.*` (`rcfd-data/1`), off unless the embedding program calls `Service::enable_data` ("`rcfd serve` does not").
- Transport: JSON-RPC 2.0, newline-delimited, over TCP (`--addr 127.0.0.1:7878`) or stdio; one thread per TCP connection; sessions shared across connections.
- No authentication: a non-loopback `--addr` is refused with exit 2 (ADR 0739 Q9) unless `--allow-unauthenticated-remote`.
- Errors: JSON-RPC -32700/-32600/-32601/-32602, 1001 unknown session, 1002 wrong run state, 1003 validation (with `{path, line, message}` diagnostics), 1004 limit or sandbox, 1005/1006 co-simulation, 1007 settings conflict (compare-and-set, ADR 0736), 1008 file changed (`case.save`), 1009 mesh is a file (`mesh.refine`), -32000 internal.
- Session states `defined` -> `ready` -> `running` <-> `paused` -> `finished` | `stopped` | `failed`; a settings change is refused (1002) while running or paused and returns the session to `defined`.
- Methods: `service.info`, `settings.schema`, `session.*`, `settings.get/set/remove/validate`, `mesh.check`, `run.*`, `monitors.get` (ADR 0353: last 100 samples of four series at 1e5 samples cost 0.3 ms instead of 21 ms), `reports.get`, `report.export`, `field.*`, `render.png`, `render.glb`, `log.tail`, `panel.snapshot`, `journal.get/replay/save`, `workflow.*`, `run.check`, `run.headless`, `status.read`, `events.poll`, `design.*`, `design.ledger_*`.
- Journals: `journal.get` returns `{format: "rcfd-journal", version: 1, ...}`; `rcfd serve --journal-dir` keeps one file per session (ADR 0735); `rcfd run --journal <file>` runs it.
- Bulk data: values over a second TCP connection as raw little-endian bytes under a one-use ticket; at most 8 tickets wait.
- UI metadata `rcfd-ui/1` (opt-in `--ui`, ADR 0730): `settings.schema {ui: true}`, `help.glossary`, `help.errors`, `mesh.preview` (ADR 0732; refuses over 2 000 000 cells without `force`), `case.facts`, `mesh.refine`, `case.save` (ADR 0737). Without `--ui` "every reply is as before, byte for byte".

## Contradictions with the wiki

> [!warning] Contradiction (within the repo doc)
> The page title says "version 1.1" and the UI section says `protocol_version` "stays 1.1", but the Versions list defines a 1.2 (bulk data, opt-in). Both hold only because 1.2 is reported solely by a service with `data.*` switched on; the title does not say so.

## Pages this source informs

- [[rcfd-service-protocol]], [[run]], [[located-error]], [[mcp]], [[rcfd-cli]]
