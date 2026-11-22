---
type: concept
tags: [interface, run]
sources: [doc-run-control]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Stop and pause files

File-based run control of `rcfd run`, the substitute for a signal handler or control socket ([[doc-run-control]]).

## How rcfd uses it

- `--stop-file f`: "the run stops cleanly before the next step when this file exists": checkpoint and results written, exit 7, state `interrupted` ([[doc-run-control]]).
- `--pause-file f`: hold between two steps while `f` exists, writing `f.ack` (the iteration count); the stop file ends the hold; paused time counts towards `--max-elapsed`. Plain runs only (not chains). Used by the run service (ADR 0123) ([[doc-run-control]]).
- "No signal handler: interrupt a CLI run with `--stop-file` ... killing the process loses the unwritten results." Remote control of a running job is the service (`run.pause/resume/stop/status`) ([[doc-run-control]]).

## Related

- [[checkpoint]], [[exit-codes]], [[rcfd-runner]], [[run]]
