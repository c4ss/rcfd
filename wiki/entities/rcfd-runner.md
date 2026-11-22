---
type: entity
tags: [crate]
sources: [doc-architecture, doc-ui-restart-plan, doc-studio-guardrails, doc-security, doc-studio-coverage, doc-run-control, doc-mcp-tools, doc-tutorial-desktop-assistant, adr-thread-runner-records-and-run-sites, adr-thread-workspace-layer, adr-thread-studio-foundation-crates]
created: 2026-11-22
updated: 2026-11-22
status: stub
---

# rcfd-runner

**What it is:** Studio layer L2, the run service: runs the `rcfd` binary as child processes and owns every run of a project. `rcfd-runner ensure` starts it. ([[doc-architecture]])
**Where:** `crates/rcfd-runner/` — layer 2 (gpu / dist / runner / fem) of the [[crate-graph]].

## Dependencies

- Depends on: [[rcfd-project]]
- Used by: [[rcfd-api]], [[rcfd-workspace]], [[rcfd-mcp]]

## Details

- Decision records: ADR 0123, ADR 0130 ([[doc-architecture]]).
- The Studio, the `rcfd-mcp` project tools (`start_run`) and `rcfd-runner start` / `ensure` queue runs; when no runner holds the project's lock one detached `rcfd-runner serve` starts and runs each queued run as an `rcfd run` child process (ADR 0123, ADR 0143) ([[doc-run-control]]).
- It passes `--case-dir`, `--store-dir runs/<id>/fields`, `--status-monitors`, `--pause-file`, `--max-steps` and `--continue-from` (ADR 0130, ADR 0200) to the child ([[doc-run-control]]).
- Output of a started runner: `.rcfd/runner.log` by default, `--log-file`, `--stdout`/`--foreground`, `--quiet`; a runner that exits before taking the lock is reported at once with its exit code (ADR 0146) ([[doc-run-control]]).
- Windows: started processes inherit every inheritable handle of the starter (no unsafe code allowed), so a capturing caller waits until the runner exits; on Linux it returns at once ([[doc-run-control]]).
- Budgets in `.rcfd/runner-config.json` (`max_runs`, `cpu_threads`, `mem_budget_mb`, ...), read at runner start; `rcfd-runner config` and the MCP tool `set_runner_config` write them (ADR 0733) ([[doc-mcp-tools]]). Other commands: `rerun <id>`, `start ... --continue-from <id> --max-steps <n>` ([[doc-mcp-tools]]).
- "The run is the runner's, not the window's" ([[doc-tutorial-desktop-assistant]]).
- Planned as wave 47B, layer L2: "A detached runner process that owns runs: queue, N concurrent runs under a CPU and memory budget, pause, resume, cancel, crash recovery, progress through append-only status files", tested by process and kill tests ([[doc-ui-restart-plan]]).
- Guardrail G7: runs outlive the Studio window. Amended 2026-11-17: the runner records who started it as `started_by` ("studio", "cli" or "mcp") in `.rcfd/runner.json`, with `studio_pid` for the Studio (ADR 0801 Q-RL-2); the Studio shuts down only a runner it owns, quietly when idle (`rcfd-runner shutdown`) and after asking when runs are active ([[doc-studio-guardrails]], [[studio-guardrails]]).
- Starts the solver binary given by `RCFD_BIN`, or the `rcfd` binary next to it; reads per-project settings from `<project>/.rcfd/runner-config.json` ([[doc-security]]).
- CLI surface seen by the capability inventory: `rcfd-runner ensure`, `start` (with `--site` for a remote run site, F-848, ADR 0739), `serve`, `shutdown`, `wait`, `rerun`, `site` ([[doc-studio-coverage]]).

### From the P3 ADR threads (2026-11-22)

- Launch records (ADR 0734): `runs/<id>/launch.json`, then `launch-<n>.json`, immutable, written before `spawn` ("the solver was not started" if it cannot be written), with `argv`, `cwd` and the solver's `build {exe_hash, git_rev}`; `rerun_of` in `run.json` only on a rerun; `mesh` and `mesh_hash` for mesh-file cases ([[adr-thread-runner-records-and-run-sites]]).
- Budgets (ADR 0733): `Config::set` checks a change with the same checks as `Config::load` and writes it with `replace_if_hash_with`, journalled; `rcfd-runner config set` exits 4 on a conflict ([[adr-thread-runner-records-and-run-sites]]).
- Remote runs (ADR 0739): a run whose `run.json` has `site` is handed to `site::remote::RemoteRuns`, one worker thread per site; "Local runs never use the seam" ([[adr-thread-runner-records-and-run-sites]], [[run-site]]).
- The order of the final writes (ADR 0742, 2026-11-19): a new `set_terminal` writes the state event, removes the request files, writes the status, removes the request files again; "the terminal status is now the last write of a run" ([[adr-thread-runner-records-and-run-sites]]).
- Runs continued from another (`continue_from`, `max_steps`, ADR 0200) and runs of design studies tagged `study`, `variant`, `condition` (ADR 0189) are run settings and record fields approved on 2026-11-18 and 2026-11-06 ([[adr-thread-workspace-layer]]).
- **Created by ADR 0123 (wave 47B, 2026-10-26, added by p3-records-a):** one `rcfd-runner serve` per project, each run an `rcfd run` child, plain files in the project folder "as queue and transport, with no socket and no network port", pause through `rcfd run --pause-file f`, adoption of running solvers without relaunch. S5 stays an ignored known miss ("on Windows the runner inherits the client's inheritable handles"); the PowerShell start route was called "a defect of the product" and later replaced by ADR 0143 ([[adr-thread-studio-foundation-crates]]).
- **How clients start it, and how it knows what is alive** (ADRs 0143, 0146, 0229, [[adr-thread-runner-records-and-run-sites]]): started directly from every client since 48I (the PowerShell launcher removed; N1 0.389 s against 1.5 s); its output goes to `<project>/.rcfd/runner.log` and an early exit is reported at once (0.042 s on Windows, 0.021 s on Linux, against 2.0 s); on Windows a std-spawned runner still holds a capturing caller's pipes (known misses C1, C2). ADR 0229 (accepted 2026-11-11) identifies an adopted solver by a held file, retries failed status and stream writes, and closes the shutdown-start race.

## Open questions

_Filled in by P1-P3 ingests._
