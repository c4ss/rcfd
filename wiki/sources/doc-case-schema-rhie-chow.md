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

# Case schema: step-consistent face flux

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "Step-consistent face flux (`[solver.rhie_chow]`, ADR 0245, ADR 0352)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

Optional table with one key, `step_consistent` (default `false`). The default transient face flux depends on the step size where the momentum diagonal differs across a face (`3.7e-4` of the lid velocity in ADR 0245's cavity between `dt = 0.04` and `0.01`); `true` makes a steady solution the same for every step size.

## Key points

- Harmonic face value of `rAU = V/a_P` plus the commutator of the lagged pressure gradient on internal faces; periodic faces unchanged.
- Works with the consistent correctors, body forces and a dynamic mesh (ADR 0352).
- Located errors with steady, compressible, variable density, VOF, fractional step, lumped components.
- Absent or `false`: bit for bit; not in the case hash or written text.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[bit-identical]]
