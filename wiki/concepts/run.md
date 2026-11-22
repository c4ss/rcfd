---
type: concept
tags: [run, studio]
sources: [doc-glossary, doc-product]
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

## Related

- [[rcfd-runner]]
- [[field-store]]
- [[report]]
- [[studio]]
