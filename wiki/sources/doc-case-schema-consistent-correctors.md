---
type: source
tags: [interface, case, numerics]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: consistent pressure correctors

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "Consistent pressure correctors (`[solver.consistent_correctors]`, ADR 0214, ADR 0223)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

Optional table with one key, `enabled` (default `false`): SIMPLEC-type pressure correctors for transient `piso`/`pimple` (`rAtU = V/(a_P - H1)`), which remove the instability of the default correctors across thin cells.

## Key points

- Applies on periodic patches, with body forces and on a dynamic mesh (ADR 0223 E2, E3).
- Located errors with steady algorithms, compressible, variable density, VOF, fractional step, `non_orthogonal_correctors > 0`, `component_diagonal = "corrected"`, lumped components.
- Absent or `false`: every earlier result bit for bit; enters neither the case hash nor the written case text.
- "The default stays `false` until the maintainer decides it on the benchmark set of ADR 0223."

## Contradictions with the wiki

_None found at ingest._

## Open questions

- Whether the default flips is a pending maintainer decision on ADR 0223's benchmark set.

## Pages this source informs

- [[bit-identical]]
