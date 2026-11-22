---
type: source
tags: [interface, run, cli]
sources: []
origin: docs/run-control.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Headless launch, run control and workflows

**Origin:** [docs/run-control.md](../../docs/run-control.md) at `origin/main` 2e94e5ef (F-120, F-121)
**Kind:** reference page; "the contract for scripts, CI jobs, the service, the MCP server and the assistant"

## Summary

The reference for launching and controlling runs without the desktop app: `rcfd run` flags, exit codes, the status stream, checkpoints and restarts, chains, the setup check, output policy and the field store, workflows, studies, and the run service `rcfd-runner`. `rcfd-api` (`runctl`, `workflow`) implements everything; the CLI, the service and the MCP tools share it, "so all of them behave the same way".

## Key points

- `rcfd run` flags include `--check-only`, `--status-json`/`--status-file`/`--status-every` (default 10), `--continue-from`, `--checkpoint`/`--checkpoint-every`, `--stop-file`, `--pause-file` (ADR 0123), `--case-dir`, `--store-dir` and `--status-monitors` (ADR 0130), `--max-elapsed`, `--max-steps` (ADR 0200), `--threads`, `--precision double` (only value), `--journal` (+ `--journal-ignore-hash`, ADR 0735), `--set path=json`, `--print-settings`, `--restart-from`, `--no-chain`. `--hostfile` is exit 2.
- Exit codes 0-8 ([[exit-codes]]); `solver.min_iterations` "does not hold a run to a count".
- Status stream `format: "rcfd-status"`, `version: 1`; events `start`, `iteration`, `checkpoint`, `snapshot`, `warning`, `check`, `finish`, plus chain events ([[status-stream]]).
- Checkpoints: a checksummed `rcfd-io` container tied to the mesh by content hash; since ADR 0031 it embeds the mesh. Available for steady laminar SIMPLE/SIMPLEC and transient laminar constant-density PISO/PIMPLE only ([[checkpoint]]).
- Measured: a 12x12 cavity stopped at 30/40 iterations and continued stops at the same 111 iterations with byte-identical solution files; `--threads 1` and `--threads 4` give byte-identical files.
- Field store (ADR 0124): off by default; `store.rts` keeps the mesh once and snapshots in checksummed blocks; `f32` opt-in (ADR 0133, error at most $\lvert x\rvert\, 2^{-24} + 2^{-150}$). The workspace start plan stores fields unless the case sets its own store (ADR 0152, 0186); `start_run` applies the plan by default since ADR 0741 ([[field-store]]).
- Workflows: task kinds `check_mesh`, `validate`, `set`, `initialize`, `solve`, `report`, `export_vtk`; states `not_run`, `done`, `failed`, `stale`; documents `{format: "rcfd-workflow", version: 1, ...}`.
- Run service (ADR 0123, 0143, 0146): `rcfd-runner serve` runs each queued run as an `rcfd run` child; output options (`.rcfd/runner.log` default, `--log-file`, `--stdout`, `--quiet`). On Windows a capturing caller waits until the runner exits (inherited handles; the workspace allows no unsafe code).
- Not implemented: control socket, signal handler (use `--stop-file`), MPI/`--hostfile`, single precision; F-121 stays partial.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[exit-codes]], [[status-stream]], [[checkpoint]], [[stop-file]], [[field-store]], [[run]], [[rcfd-runner]], [[rcfd-cli]], [[rcfd-api]], [[setup-rules]], [[parametric-study]]
