# ADR 0562: F-634 registration: shell documentation note on pressure-loaded thin curved shells on unstructured meshes

Status: **REGISTERED 2026-11-15, written before any code of F-634 and before any run of its checks.** Not yet accepted by the
maintainer. Docs-only item: no library line, no test file. Results go in the addendum at the end; nothing above it is changed
after a run.

Roadmap item F-634 (dep F-546), the documentation follow-up named by remaining question 3 of
[ADR 0550](0550-f546-registration-angle-weighted-shell-pressure-on-an-unstructured-sphere-octant.md) (option (a), recommended
there). Number 0562 given by the lead. Branch `f634-work`, scratch `w49i-work/f634-scratch`. House rules
(`w49i-work/AGENT_RULES.md`) apply; the open question below is taken on the maintainer's standing instruction of 2026-11-11 (take
the recommendation).

## What F-634 asks

> Every limit not stated here is registered in the ADR before the run. The shell topic documentation states the `h^2/(R t)` mesh
> requirement measured in ADR 0550 and when to choose `Load::shell_pressure_angle_weighted`; docs_site test passes.

## What was read

The addendum and the "For the lead" section of ADR 0550, and the diagnosis of [ADR 0461](0461-f540-registration-angle-weighted-shell-pressure.md)
(for the cylinder factor 300 quoted by ADR 0550). `docs/structural-solver.md` is the only topic page that names shells (one
line, in "Scope", pointing to the roadmap); there is no separate shell page, so the note goes there as a new section. The
docs_site test (`crates/rcfd-cli/tests/docs_site.rs`) builds every Markdown file under docs/ and fails on a relative link that
does not resolve.

## Design: what the note may say

Only what ADR 0550 measured, with its numbers and the reference. Content fixed now:

1. A section "Shells under pressure on curved meshes" in `docs/structural-solver.md`, after "Scope".
2. The requirement: on an unstructured thin curved shell the error of a pressure load, whichever lumping, is set by the shell's
   bending response to the nodal unevenness of the load, `~ c h^2 / (R t)` (second order in `h`, growing as `1 / t`), not by
   the stiffness (the area-gradient load gives the uniform expansion to `6e-14`). Measured: unstructured octant of a sphere,
   `R = 1`, `t = 0.01`, internal pressure; 2 percent of `p R^2 (1 - nu) / (2 E t)` at every node needs n = 128 (edges about
   `0.012 R`); errors `1.40e-1, 3.33e-2, 7.99e-3` at n = 32, 64, 128 (order 2.07, 2.06); default load `1.01e-2` at n = 128.
3. The choice rule: the angle split is opt-in (`Load::shell_pressure_angle_weighted`); the default consistent split stays the
   default. Measured advantage of the angle split over the default: factor 0.62 to 0.79 on the unstructured octant, 16 on the
   structured geodesic grid, 300 on the one-diagonal cylinder of ADR 0461. Rule stated: choose it on structured or
   one-diagonal-type curved meshes where the default load is uneven; on unstructured meshes the gain is small and refinement,
   not the split, is what meets the percent level. Not stated: any threshold for `h^2 / (R t)` beyond the measured n = 128 point
   (ADR 0550 gives only "well below one"), any other geometry, thickness or element.
4. Link to ADR 0550 and ADR 0461; the limits of the note (below).

## Checks (limits fixed now)

| id | check | limit |
| --- | --- | --- |
| C1 | the page contains `h^2 / (R t)`, `Load::shell_pressure_angle_weighted` and a link to ADR 0550 | all three present |
| C2 | the choice rule states the three factors 0.62 to 0.79, 16 and 300 | all present |
| C3 | every number quoted equals ADR 0550's: `1.40e-1`, `3.33e-2`, `7.99e-3`, `1.01e-2`, `6e-14`, `2.07`, `2.06`, `0.012`, `128` | each string found in ADR 0550 (script `grep -F`) |
| C4 | `cargo test -p rcfd-cli --release --test docs_site` | passes |
| C5 | files changed: this ADR and `docs/structural-solver.md` only; no `crates/` line | `git diff --stat` shows no other path |

Negative control: C3 run on a temporary copy of the page with one number altered (`7.99e-3` to `7.98e-3`) must fail. Planned
mutation: delete the ADR 0550 link from a scratch copy; the docs_site test must still pass (it checks links that exist, not
their presence), so C1 is the check that catches it; recorded as such.

## Decision rule

All of C1 to C5 met: done. Any miss: partial, with the miss recorded.

## Limits

Documentation of one measured case. No new measurement, no guidance on other geometries, thicknesses or meshes, no change to the
default load (ADR 0550 question 2: keep the consistent split as the default, taken on the standing instruction).

## Addendum 1 (2026-11-15): results

Made once; no amendment, no deviation.

| id | limit | measured | met |
| --- | --- | --- | --- |
| C1 | three items present | `h^2 / (R t)` twice, `Load::shell_pressure_angle_weighted` once, link to ADR 0550 present | yes |
| C2 | 0.62 to 0.79, 16, 300 | all three in the choice rule | yes |
| C3 | every number found in ADR 0550 | `1.40e-1 3.33e-2 7.99e-3 1.01e-2 6e-14 2.07 2.06 0.012 128` all found | yes |
| C4 | docs_site passes | 2 passed, 0 failed (release) | yes |
| C5 | only the ADR and structural-solver.md | no `crates/` path changed | yes |

Negative control: `7.98e-3` is not found in ADR 0550, so C3 fails on an altered number. The planned link-deletion mutation was
not run as a separate step (it is reasoned in the registration: docs_site would not catch it, C1 does).

Page choice: there was no separate shell topic page; the note is a new section of `docs/structural-solver.md`. The page's
choice rule goes one step past ADR 0550's wording only in naming "structured or one-diagonal meshes" (the two mesh families where
ADR 0550 reports factors 16 and 300).

**Decision: done**, by the rule.

## For the lead: roadmap entries

Proposed status of F-634: **done**.

> F-634 (ADR 0562, 2026-11-15): `docs/structural-solver.md` has a new section "Shells under pressure on curved meshes" stating what ADR 0550 measured: the error of a shell pressure load on an unstructured thin curved mesh is `~ c h^2/(R t)` (bending response to the nodal unevenness of the load, not the stiffness), 2 % needs n = 128 at t/R = 0.01 on the sphere octant (errors 1.40e-1, 3.33e-2, 7.99e-3 at n = 32, 64, 128; default 1.01e-2 at n = 128), and `Load::shell_pressure_angle_weighted` is opt-in with measured gains of 0.62 to 0.79 (unstructured octant), 16 (structured geodesic grid) and 300 (one-diagonal cylinder of ADR 0461). Every quoted number checked against ADR 0550; docs_site passes; no code change.
