---
type: source
tags: [interface, case, cosim]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: co-simulation fluid participant

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "Co-simulation fluid participant (`[cosim]`, F-444, ADR 0380)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

The optional table `[cosim]` makes the case the fluid participant of a co-simulation with a structural solver: `rcfd serve --cosim <case>` hosts it under protocol `rcfd-cosim/1` and a coupler drives it; `rcfd run` refuses the case.

## Key points

- Keys: `patch` (required), `participant` (default `"fluid"`), `fixed_patches`, `smoothing_method`, `smoothing_exponent` (1), `smoothing_tolerance` (`1e-12`, absolute floor `1e-16`, at most 5000 iterations), `initial_displacement`.
- Must be incompressible `piso`/`pimple` with nothing else to advance; adaptive correctors, `dynamic_mesh`, `mesh.adapt` are located errors. "The coupler gives the step; `solver.dt` and `solver.end_time` are not used."
- Absent: every earlier result bit for bit; not in the case hash or written text.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[rcfd-service-protocol]]
