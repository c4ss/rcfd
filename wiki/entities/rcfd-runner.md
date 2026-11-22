---
type: entity
tags: [crate]
sources: [doc-architecture, doc-ui-restart-plan, doc-studio-guardrails, doc-security, doc-studio-coverage, doc-run-control, doc-mcp-tools, doc-tutorial-desktop-assistant]
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

## Open questions

_Filled in by P1-P3 ingests._
