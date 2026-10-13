# rcfd solver service protocol, version 1

Identifier: `rcfd-service/1` (returned by `service.info`).

Transport: JSON-RPC 2.0, one JSON object per line (newline-delimited), over TCP
(`rcfd serve --addr 127.0.0.1:7878`) or stdio (`rcfd serve --stdio`). Requests
without an `id` are notifications and get no response. Each TCP connection is
served by its own thread; sessions are shared between connections.

## Errors

| code | meaning |
|------|---------|
| -32700 | invalid JSON |
| -32600 | not a JSON-RPC 2.0 request |
| -32601 | unknown method |
| -32602 | missing or invalid parameters |
| 1001 | unknown session |
| 1002 | operation not allowed in the current run state |
| 1003 | settings failed validation; `error.data.diagnostics` is a list of `{path, line, message}` |
| 1004 | limit exceeded or path outside the sandbox root |
| -32000 | internal error |

## Sessions and run states

`session.create` takes one of `{toml}`, `{settings}` (JSON object), `{path}`
(sandboxed) or nothing (defaults) and returns `{session}`.
States: `defined` -> `ready` -> `running` <-> `paused` -> `finished` | `stopped` | `failed`.
Changing settings is refused (1002) while running or paused; a change discards the
built simulation and returns the session to `defined`.

## Methods

All session methods take `session`.

- `service.info`, `settings.schema` (JSON Schema of the case and of the monitor format)
- `session.create | list | close`
- `settings.get {path?}`, `settings.set {path, value}`, `settings.remove {path}` (delete a named entry such as `boundaries.lid`; journaled), `settings.validate`
- `mesh.check` quality report and patch list
- `run.initialize | start {max_steps?} | pause | resume | stop | step {count?} | status | wait {timeout_ms?}`
- `monitors.get {names?, since?}` (JSON format `rcfd-monitors` v1), `reports.get`
- `report.export {format: html|csv|json, path?}`
- `field.stats {name}`, `field.slice {name, max_points?}`, `field.export_vtk {dir?, name?}`
- `render.png {field?, width?, vectors?, path?}` returns `data_base64`
- `log.tail {lines?}`
- `field.grid {name, cols?, rows?}` field averaged onto a coarse grid (first row is the top), `null` where empty
- `panel.snapshot {since?, field?, cols?, rows?, log_lines?}` status, new monitor samples, reports, log tail and optional grid in one call (used by the sidebar panel)
- `journal.get` returns `{format: "rcfd-journal", version: 1, entries, case_hash}` (create + accepted settings changes); `journal.replay {journal}` builds a new session and reports `matches` against the recorded hash
- `events.poll {after, timeout_ms?, session?, limit?}` long-poll; returns `{events, next}`.
  Event kinds: `created initialized started iteration paused resumed stopped
  chunk_done finished error setting_changed closed`. Pass `next` as the next `after`.

## Limits and sandbox

`ServiceConfig` bounds cell count, steps per call and event-log size, and
`--root` confines every client-supplied file path (no `..`, no escape through
symbolic links).
