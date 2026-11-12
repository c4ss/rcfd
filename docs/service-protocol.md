# rcfd solver service protocol, version 1.1

This page specifies the solver service: the transport, the error codes, the session states and every method with its
parameters. It is for people who write a client for the service or check what a tool of the MCP server does underneath. If
you use an assistant rather than write a client, the [MCP tool list](mcp-tools.md) is the shorter page; for exit codes, the
status stream and checkpoints shared with the command line, see [run control](run-control.md).

Identifier: `rcfd-service/1` (returned by `service.info` as `protocol`). The identifier names the major version: a service
of any version 1.x answers every request of version 1.0 as before. `service.info` also returns `protocol_version` (`"1.1"`)
and `cosim` (`"rcfd-cosim/1"`, the identifier of the co-simulation calls).

Versions:

- 1.0: everything below except the `cosim.*` methods.
- 1.1 (ADR 0345): the `cosim.*` methods, the error codes 1005 and 1006, and the members `protocol_version` and `cosim` of
  the `service.info` reply. No earlier method changed.

Transport: JSON-RPC 2.0, one JSON object per line (newline-delimited), over TCP
(`rcfd serve --addr 127.0.0.1:7878`) or stdio (`rcfd serve --stdio`). Requests
without an `id` are notifications and get no response. Each TCP connection gets
its own thread, and connections share sessions.

## Limits and sandbox

`ServiceConfig` bounds cell count, steps per call, case runs per `design.*` call (`max_design_evaluations`) and event-log size.
`--root` confines every client-supplied file path (no `..`, no escape through symbolic links).

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
| 1005 | a hosted co-simulation participant returned an error; `error.message` is the participant's message |
| 1006 | no hosted co-simulation participant of that name |
| -32000 | internal error |

## Sessions and run states

`session.create` takes one of `{toml}`, `{settings}` (JSON object), `{path}`
(sandboxed) or nothing (defaults) and returns `{session}`. With `{toml}` or `{settings}` an optional
`base_dir` (sandboxed) is the folder that relative data paths of the case (mesh file, lookup tables) resolve
against, as a case file's own folder does for `{path}`. Without it they resolve against the sandbox root, else
the working folder (ADR 0144). The journal's create entry records `base_dir` when it was given.

States: `defined` -> `ready` -> `running` <-> `paused` -> `finished` | `stopped` | `failed`.

The service refuses a settings change (1002) while the session is running or paused. A change discards the
built simulation and returns the session to `defined`.

## Methods

All session methods take `session`.

- `service.info`, `settings.schema` (JSON Schema of the case and of the monitor format)
- `session.create | list | close`
- `settings.get {path?}`, `settings.set {path, value}`, `settings.remove {path}` (delete a named entry such as `boundaries.lid`; journaled), `settings.validate`
- `mesh.check`: quality report and patch list
- `run.initialize | start {max_steps?} | pause | resume | stop | step {count?} | status | wait {timeout_ms?}`
- `monitors.get {names?, since?}` (JSON format `rcfd-monitors` v1), `reports.get`
- `report.export {format: html|csv|json, path?}`
- `field.stats {name}`, `field.slice {name, max_points?}`, `field.export_vtk {dir?, name?}`
- `render.png {field?, width?, vectors?, path?}`: returns `data_base64`
- `render.glb {field?, patches?, min?, max?, path?, inline?}`: binary glTF 2.0 of boundary patches, flat coloured by the cell field (viridis, range from the data unless min and max are given). Returns size and face counts, `data_base64` only with `inline` (max 4 MiB), file only with a sandboxed `path`
- `log.tail {lines?}`
- `field.grid {name, cols?, rows?}`: field averaged onto a coarse grid (first row is the top), `null` where empty
- `panel.snapshot {since?, field?, cols?, rows?, log_lines?}`: status, new monitor samples, reports, log tail and optional grid in one call (used by the sidebar panel)
- `journal.get`: returns `{format: "rcfd-journal", version: 1, entries, case_hash}` (create + accepted settings changes)
- `journal.replay {journal}`: builds a new session and reports `matches` against the recorded hash
- `workflow.create {template?|workflow?, name?}`, `workflow.list`, `workflow.get`, `workflow.run {task?, with_prerequisites?, max_steps?}`, `workflow.insert {task, after?}`, `workflow.delete {id}`, `workflow.save {path}`, `workflow.load {path}`: task-based workflow over the session (F-120; see `docs/run-control.md`). `workflow.run` blocks until the planned tasks ran or one failed and returns `{ok, ran: [{id, ok, message, result}], workflow}`
- `run.check {session | path}`: check-only launch (`{ok, exit_code, event}`; exit code 0, 3 invalid, 4 bad mesh). `run.headless {max_steps?, max_elapsed_s?, stop_file?, continue_from?, interpolate_case?, interpolate_checkpoint?, interpolate_mode?, checkpoint?, checkpoint_every?, status_path?, status_every?, out_dir?}`: runs the session under the run-control driver and returns `{outcome, ok, events, status}`. `status.read {path}`: summarises an `rcfd-status` JSON-lines file (F-121). All paths are sandboxed; step and time limits of the service apply
- `events.poll {after, timeout_ms?, session?, limit?}`: long-poll; returns `{events, next}`.
  Event kinds: `created initialized started iteration paused resumed stopped
  chunk_done finished error setting_changed closed workflow_created workflow_task`. Pass `next` as the next `after`.

The `design.*` methods (ADR 0067, `docs/design-loop.md`, `docs/mcp-tools.md`) are listed below. Study state lives in the study
directory (`study.json`, `evaluations.jsonl`), so the methods are stateless. Every path is sandboxed (1004); the number of case runs
one call may start is bounded by `max_design_evaluations` (default 1000).

- `design.create_study {dir, base | session, parameters, conditions?, objectives, constraints?, method, workers?, chain?, max_steps?, overwrite?}`
- `design.create_variant {path, base | dir, params?, condition?, overrides?, overwrite?}`
- `design.run_map {out_dir, base | dir, params?, condition?, overrides?, axes | points, serpentine?, on_failure?, retry_cold?, cold?, max_steps?}`
- `design.run_study {dir, max_evaluations?}`
- `design.evaluate {dir, params}`
- `design.record {dir, params, metrics | conditions, ok?, note?}`
- `design.get_metrics {dir, index | params | map_dir}`
- `design.compare {dir, indices | candidates}`
- `design.propose {dir}`
- `design.list_study {dir}`

The design ledger (ADR 0121):

- `design.ledger_append {ledger, params, metrics, evaluator, evaluator_version, verdict, fidelity, study?, variant?, kind?, reason?, solver_version?, geometry_hash?, settings?, wall_s?, seed?, provenance?}`
- `design.ledger_query {ledger, kind?, verdict?, study?, evaluator?, fidelity?, offset?, limit?, pareto?, best?, history?}`
- `design.ledger_export {ledger, out, format?, filters?}`
- `design.ledger_rerun_check {ledger, index, metrics, verdict?, rtol?, atol?}`

## Co-simulation methods (version 1.1, `rcfd-cosim/1`)

The calls of the [co-simulation protocol](cosimulation-protocol.md) for a participant that the embedding program hosts in
the service (`Service::host_fluid`, `Service::host_structure`). `rcfd serve` hosts none, so there every one of them but
`cosim.list` answers 1006. The wire format, the lease and what a dropped connection means are specified in
[cosimulation-protocol.md](cosimulation-protocol.md#over-the-solver-service-rcfd-cosim1).

- `cosim.list`: `{protocol, participants: [{participant, kind, name}]}`
- `cosim.open {participant}`: `{protocol, lease, kind, name, time}`; supersedes every earlier lease of the participant
- `cosim.describe {participant, lease}`: a structure `{kind, name, nodes}`, a fluid `{kind, name, points, faces}`
- `cosim.time {participant, lease}`, `cosim.checkpoint {participant, lease}`, `cosim.restore {participant, lease}`: `{time}`
- `cosim.forces {participant, lease}` (a fluid): `{forces, time}`
- `cosim.advance {participant, lease, dt, forces}` (a structure): `{displacements, time}`;
  `cosim.advance {participant, lease, dt, displacements}` (a fluid): `{forces, time}`

A call with a lease that is not the current one, and `cosim.forces` to a structure, answer 1002.

## Where to go next

- [MCP tools](mcp-tools.md): which tool calls which method, and the access class of each.
- [Case schema](case-schema.md): the dotted settings paths that the settings methods take.
- [Run control](run-control.md): the headless run, exit codes and workflows behind the run and workflow methods.
- [Co-simulation protocol](cosimulation-protocol.md): the participant calls that the `cosim.*` methods carry.
