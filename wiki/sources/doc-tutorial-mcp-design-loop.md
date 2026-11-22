---
type: source
tags: [tutorial, mcp, design]
sources: []
origin: docs/tutorials/mcp-design-loop.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Tutorial: drive rcfd from an assistant, the MCP design loop

**Origin:** [docs/tutorials/mcp-design-loop.md](../../docs/tutorials/mcp-design-loop.md) at `origin/main` 2e94e5ef
**Kind:** tutorial (executed by tests, as JSON-RPC lines)

## Summary

Connect a client to `rcfd-mcp` on stdio, set up and run a contraction duct, run a small Nelder-Mead design study, write a variant and map its operating range, then check (only check) the axial fan passage mesh.

## Key points

- Client entry: `rcfd-mcp --root <dir>`; options `--read-only`, `--max-cells`, `--max-steps`, `--max-run-seconds`, `--max-design-evaluations`. `tools/list` marks state-changing tools (`readOnlyHint` false, `rcfd/needsApproval` true).
- Each `jsonl` block is one session; the test starts `rcfd-mcp --root .` and requires every response to be free of errors.
- Duct study: minimise `dp` subject to `ux_out >= 1.3`, parameters angle [0, 12] and start [0.5, 3], Nelder-Mead from (6, 0.5); `design_propose` has no side effects; a repeated `design_evaluate` returns `"cached":true`.
- For reference ADR 0067 E1 measured `dp` falling from 9.94 to 5.07 at angle 4.46 and start 1.77 after 77 evaluations; "This tutorial does not re-check those numbers."
- The fan design study "is not part of this tutorial on purpose": the steady rotating-frame solution "has not been shown to converge to mesh-independent torque, and the comparison with the measured FAN-01 fan missed ... A design iteration on the fan today would optimise numerical error."
- "The rcfd side is deterministic: the same study definition and seed give the same history."

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[executable-tutorials]], [[mcp]], [[rcfd-mcp]], [[mcp-access-class]], [[fan-01]]
