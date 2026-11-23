---
type: concept
tags: [run, studio]
sources: [doc-glossary, doc-product, doc-run-control, doc-service-protocol, doc-mcp-tools, doc-tutorial-desktop-assistant, doc-example-projects]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Run and its lifecycle

One solve of a case by the project's runner, kept in `runs/<run id>/` with settings, status, log and results; it goes on when the Studio is closed ([[doc-glossary]]).

## How rcfd uses it

- States: queued (accepted, waiting for a slot), running (its step is the iteration or time step reached), then finished/failed/paused per [[doc-product]].
- Iteration: one round of a steady solver, repeated until residuals fall below `solver.tolerance_u` / `solver.tolerance_p` or `solver.max_iterations` ([[doc-glossary]]).
- Converged = 'residual targets met': the calculation stopped changing, not that the answer is right ([[doc-glossary]]).
- Headless contract: `rcfd run` flags, [[exit-codes]] 0-8 with a final `state` (`converged`, `finished`, `failed`, `not_converged`, `diverged`, `interrupted`, `time_limit`, `step_limit`), the [[status-stream]], [[checkpoint]]s and [[stop-file]]s; implemented once in `rcfd-api` and shared by the CLI, the service and the MCP tools ([[doc-run-control]]).
- A service session has the states `defined` -> `ready` -> `running` <-> `paused` -> `finished` | `stopped` | `failed` ([[doc-service-protocol]]).
- A project run of the run service is an `rcfd run` child process on `runs/<id>/case.snapshot` with `--case-dir`, `--store-dir`, `--status-monitors` and a pause file ([[doc-run-control]]); run ids look like `r-<yyyymmddThhmmss>-<4 hex>` and `runs/<id>/` holds `run.json`, `launch.json`, `residuals.jsonl`, `output/`, `fields/` ([[doc-mcp-tools]]).
- "The run is the runner's, not the window's: closing the Studio leaves it going" ([[doc-tutorial-desktop-assistant]]).
- Committed example runs (made by the real solver through the run service with `scripts/make_example_projects.py`) live under `example-projects/*/runs/` and, built on Linux, `example-projects-linux/*/runs/` ([[doc-example-projects]]).

## Related

- [[rcfd-runner]]
- [[field-store]]
- [[report]]
- [[studio]]
- [[exit-codes]]
- [[status-stream]]
- [[checkpoint]]
- [[stop-file]]
- [[divergence-guard]]
- [[example-project-wind-tunnel]], [[example-project-quick-start]], [[example-project-axial-fan]]
