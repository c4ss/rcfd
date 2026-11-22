---
type: concept
tags: [process, evidence, tolerances]
sources: [doc-process-tolerances, doc-process-documentation-review, doc-process-lead-process]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Tolerance class (tolerance table)

A class of check with an allowed limit set once by the maintainer, so that "the bar is then set by class of check by the maintainer, not by the agent it measures" ([[doc-process-documentation-review]], gap-closing change for clause 4f). The table is `docs/process/tolerances.md`, "PROVISIONAL VERSION 1, approved by the maintainer on 2026-11-18" ([[doc-process-tolerances]]).

## How rcfd uses it

- A limit in a registration written from 2026-11-18 cites a row of the table, a published reference with its place in it, or the acceptance of the parent item (rule 15); a registration may always set a tighter limit than its row ([[doc-process-tolerances]]). A limit with no such source is sent back by the lead before code ([[doc-process-lead-process]]).
- The rows (starting values as seen in 18 registrations, mostly structural):

| Class of check | Kind | Starting value |
|---|---|---|
| Bit-identity of default output | unchanged default output | exact: equal bytes or equal hash |
| Same discrete problem by two routes | code verification | relative difference `<= 1e-8`; `<= 1e-12` where only rounding separates the two |
| Agreement with a closed form | code verification | relative error `<= 1e-6` to `<= 1e-9` (exact for the discrete problem); `<= 1e-3` (exact only for the continuum) |
| Observed order of accuracy | solution verification | within 0.5 of the formal order, or at least the formal order less 0.1 |
| Grid convergence | solution verification | ratio of successive differences within 2.5 % of its formal value (3.9 to 4.1 for order 2) |
| Linear and eigenvalue solver residual | code verification | relative residual `<= 1e-10` |
| Agreement with a published value | solution verification | relative error `<= 5e-3` against a value published to four digits |
| Agreement with a result on a known front or optimum | solution verification | `<= 2 %` |
| Agreement with a measured case | validation | none seen; to be set by the maintainer per measured case |
| Run time or memory, as a budget | unchanged default output | stated budget with the estimate beside it; ratio `<= 0.5` for a speed-up |

  ([[doc-process-tolerances]])
- Negative controls state the limit the other way round, from the row of the check they control ([[doc-process-tolerances]]).
- The kinds are the four values of the "kind" column of a checks table (rule 14) ([[doc-process-documentation-review]]).

## Related

- [[registered-limit]]
- [[registration]]
- [[bit-identical]]
- [[fan-01]] (a measured case, for which no validation row exists yet)

## Open questions

- No rows yet for conservation of mass, momentum or energy in flow cases, or for turbulence-model comparisons with published profiles ([[doc-process-tolerances]]).
- The validation row has no value: "to be set by the maintainer per measured case" ([[doc-process-tolerances]]).
