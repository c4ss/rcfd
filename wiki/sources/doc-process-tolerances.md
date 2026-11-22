---
type: source
tags: [process, evidence, tolerances]
sources: []
origin: docs/process/tolerances.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: stable
---

# Tolerance table

**Origin:** [docs/process/tolerances.md](../../docs/process/tolerances.md) at `origin/main` 2e94e5ef
**Kind:** process rule table

## Summary

"Status: PROVISIONAL VERSION 1, approved by the maintainer on 2026-11-18." It binds registrations written under the agent rules from that date and no earlier one: a limit in such a registration cites a row here, a published reference with its place in it, or the acceptance of the parent item (rule 15). The starting values "are not new numbers": they are the limits found in the check tables of 18 registrations (ADR 0357, 0365, 0378, 0379, 0394, 0501, 0504, 0527, 0558, 0569, 0573, 0575, 0602, 0614, 0620, 0627, 0672, 0682), "a small sample, mostly of the structural solver". A registration may always set a tighter limit than its row. The rows are listed on [[tolerance-class]].

## Key points

- Ten rows (class of check, kind per rule 14, starting value): bit-identity of default output (exact); same discrete problem by two routes (`<= 1e-8`, `<= 1e-12` where only rounding separates them); agreement with a closed form (`<= 1e-6` to `<= 1e-9` exact for the discrete problem, `<= 1e-3` exact only for the continuum); observed order of accuracy (within 0.5 of formal, or at least formal less 0.1); grid convergence (ratio within 2.5 % of formal, 3.9 to 4.1 for order 2); linear and eigenvalue solver residual (`<= 1e-10`); agreement with a published value (`<= 5e-3` against four published digits); agreement with a result on a known front or optimum (`<= 2 %`); agreement with a measured case ("none seen in the sample; to be set by the maintainer per measured case"); run time or memory as a budget (stated budget with the estimate beside it; ratio `<= 0.5` for a speed-up claim).
- A negative control states its limit the other way round (the difference must be at least a stated size), taken from the row of the check it controls.
- Not covered: conservation of mass, momentum or energy in flow cases; turbulence-model comparisons with published profiles. Rows are added from the registrations that use them, each addition approved like the table.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[tolerance-class]]
- [[registered-limit]]
- [[registration]]
