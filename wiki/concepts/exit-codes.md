---
type: concept
tags: [interface, run, cli]
sources: [doc-run-control, doc-tutorial-quick-start-cli, doc-studies, doc-python, doc-service-protocol, doc-case-schema-decomposition-postprocessing, doc-udf-abi, doc-case-schema-transient-divergence-guard, doc-case-schema-steady-divergence-guard]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Exit codes of rcfd run

The numbered outcome of an `rcfd run` launch, paired with the `state` of the final status event; shared by the CLI, the service, the MCP tools, studies and the Python package ([[doc-run-control]]).

| code | meaning | `state` in the final event |
| --- | --- | --- |
| 0 | converged, or a transient run reached its end time (also `--check-only` success) | `converged`, `finished` |
| 1 | runtime or I/O failure (including an unusable checkpoint) | `failed` |
| 2 | usage error | |
| 3 | invalid case or settings | |
| 4 | the mesh failed its quality check | |
| 5 | steady run stopped at its iteration cap without meeting the tolerances | `not_converged` |
| 6 | residuals became non-finite, or a divergence guard stopped the run | `diverged` |
| 7 | interrupted through the stop file | `interrupted` |
| 8 | `--max-elapsed` reached | `time_limit` |

(Table from [[doc-run-control]].)

## How rcfd uses it

- `--max-steps n` stops with state `step_limit` and exit 0, checkpoint written ([[doc-run-control]]).
- `solver.min_iterations` "does not hold a run to a count": it only delays `convergence.<name>` conditions; to run a steady case for a fixed count, set the residual targets below what the run reaches and let `max_iterations` end it ([[doc-run-control]]).
- `rcfd check` exits 0 (also with warnings), 3 or 4 ([[doc-run-control]]).
- A study exits with the code of the lowest-index failed point (1, 3, 4, 5, 6), 0 when all ok, 3 for static sweep errors, 2 for usage ([[doc-studies]]); a workflow exits with the code of the failing task ([[doc-run-control]]).
- `rcfd inflate`: 1 failure, 2 usage, 4 quality errors ([[doc-case-schema-decomposition-postprocessing]]).
- A failing user-function module gives state `failed`, exit 1 ([[doc-udf-abi]]); divergence guards give exit 6 ([[doc-case-schema-transient-divergence-guard]], [[doc-case-schema-steady-divergence-guard]]).
- In Python, non-zero codes are outcomes in `RunResult.exit_code`, not exceptions ([[doc-python]]).
- `help.errors` (with `rcfd serve --ui`) lists every error and exit code with a recovery line, by surface `cli_exit`, `runner_exit`, `mcp_exit`, `mcp_hook_exit`, ... ([[doc-service-protocol]]).

## Related

- [[status-stream]], [[divergence-guard]], [[stop-file]], [[located-error]], [[setup-rules]]

## Open questions

- The quick start's list from `rcfd --help` omits code 1 ([[doc-tutorial-quick-start-cli]]); whether `--help` itself lists 1 is not stated.
