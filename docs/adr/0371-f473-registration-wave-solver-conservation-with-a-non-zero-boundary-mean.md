# ADR 0371: F-473 registration: conservation of the integral of p in the wave solver with an initial field of non-zero boundary mean

Status: **REGISTERED 2026-11-12**, written before any code of F-473 and before any run of its checks. Not yet accepted by the
maintainer. Results go in an addendum; nothing above the addendum is changed after a run.

Parent: [ADR 0366](0366-f453-registration-circular-and-three-dimensional-ducts-through-the-wave-solver.md) (F-453: the check
U8 and its question 2, from which this item comes, recommendation taken on the maintainer's standing instruction of
2026-11-11 (take the recommendation)); [ADR 0351](0351-f333-registration-acoustic-wave-solver-and-modal-export.md) (F-333, the
solver and its check U1). Roadmap item F-473 (depends on F-453). Number 0371, branch `f473-wave-conservation` from main
7a533a53, worktree `w49i-work/f473`, given by the lead.

House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here and not changed after a run; a miss becomes an ignored
`*_known_miss` test with the limit intact; each registered run is made once; defaults bit-identical; no hash table or golden
touched; no new crate, no unsafe code; unsigned commits without trailers; no push.

## What F-473 asks

Acceptance (`docs/roadmap.json`, binding in full):

> On the closed circular O-grid duct, an initial pressure with a non-zero mean on every patch keeps sum p V to 1e-12 over 100
> steps with every patch a wall, and changes it by more than 1e-3 when one patch is open.

Clauses: (A) closed duct, every patch a wall, the initial pressure with a non-zero mean on every patch, 100 steps,
`sum p V` kept to `1e-12`; (B) one patch open, `sum p V` changes by more than `1e-3`. "Relative" is read as in ADR 0366 U8:
the change divided by `sum |p0| V`.

## What was read and run before this text (disclosed)

- **Read:** the F-473 entry of `docs/roadmap.json`; ADR 0366 in full (U8, its mutation MU3 and diagnosis, question 2);
  `crates/rcfd-fv/src/acoustic_wave.rs` (the face fluxes, lines 454 to 481: `Wall` gives `(ps, uns) = (pl + z unl, 0)`, `Open`
  gives `(0.5 pl + 0.5 z unl, 0.5 unl + 0.5 pl/z)`; `pressure_integral`, `max_dt`); the U8 test in
  `crates/rcfd-post/tests/circular_duct_f453.rs` lines 226 to 252 (read only; that file is not edited).
- **Run:** nothing. No code of F-473 exists, nothing was compiled or run.
- **Arithmetic by hand** (below, estimates only).

## Design

**Why U8 missed MU3 (from ADR 0366).** The semi-discrete update of cell `c` is `dp_c/dt = -(rho0 c^2 / V_c) sum_f u*_f A_f`
(the interface normal velocity `u*` of the exact Riemann flux), so `d(sum p V)/dt = -rho0 c^2 sum_{boundary f} u*_f A_f`: only
boundary faces change the integral (the interior fluxes cancel in pairs). On a `Wall` face `u* = 0`. On an `Open` face `u* =
0.5 u_n + 0.5 p/z` with `z = rho0 c`; at rest `u* = p/(2z)`, linear in the boundary value `p` of the reconstruction. U8's
field `(1 + x/R + 0.5 y/R) cos(pi z/L)` has a zero mean over the wall (the `cos` averages to zero along the duct, `x`, `y`
average to zero by the 90-degree symmetry) and equal and opposite values on the two ends, so a wrongly open wall gives a sum
of boundary fluxes that cancels. The new field must have a non-zero mean on each patch, so that the same sum does not cancel
for any single patch.

**Field.** On `circular_duct_ogrid(6, 3, 6, R = 0.06, core 0.5, length 0.1)` (the mesh of U8; 6 x 6 core cells, 3 ring cells,
6 layers: (36 + 72) x 6 = 648 cells), the cell value at the cell centre `x_c`:

    p0(x_c) = 1 + 0.4 x/R + 0.3 y/R + 0.5 z/L     [Pa],  u0 = 0.

It is positive everywhere (the minimum is at least `1 - sqrt(0.4^2 + 0.3^2) + 0.5 z_c/L > 0.5`, so `sum |p0| V = sum p0
V`). Patch means: the area-weighted mean of the owner cells' `p0` over the faces of the patch. By the 90-degree symmetry of
the section the `x`, `y` terms average to zero over the wall and over each end, so the means are: `inlet` `1 + 0.5 (z_c/L) =
1.042` (the first layer's centre is at `L/12`), `outlet` `1.458` (`11/12`), `wall` `1.25`. All three are far from zero. This is
the field with a non-zero boundary mean that ADR 0366 asked for; the linear part also makes the initial gradient non-zero, so
the run is not trivial.

**Steps.** `dt = max_dt(0.48)` once at the start, 100 steps with `step(dt)`, as U8. The same `dt` for the closed and the open
runs. Open runs set exactly one patch to `WaveBc::Open` and the other two to `WaveBc::Wall`; no `Driven`.

**Estimate of the change with one patch open** (rough). At rest, `d(sum p V)/dt = -rho0 c^2 sum u*_f A_f = -0.5 c sum p_f
A_f`, with `p_f` the boundary value of the reconstruction. Inlet (area `A ~ pi R^2 ~ 0.0111 m^2`, `p_f ~ 1.0`):
`-0.5 * 340 * 1.0 * 0.0111 = -1.9 Pa m^3/s`. The time of 100 steps: the smallest cells (core `h = R/6 = 0.01 m`, layer
`dz = L/6 = 0.0167 m`) give `dt` of a few `1e-6 s` (`dt = 0.48 * 2V/(c sum A)`, about `5e-6` s), so 100 steps are about `5e-4
s`. The open face drains the first layer at the rate `~0.5 c/dz = 1e4 /s`, so within `5e-4 s` that layer has largely
lost its pressure, and the loss is bounded by the layer's share of `sum p V`, `(1/6)(1.04/1.25) ~ 0.14`. Hence the estimated
relative change is `0.05` to `0.2` for the inlet, `0.1` to `0.3` for the outlet, and for the open wall (a ring layer of
thickness `~0.01 m` against the radius) `0.05` to `0.3`. The limit `1e-3` is a factor 50 or more below. These estimates are
rough and may be off by a factor of a few; they are not tuned.

## Registered checks

All limits fixed now. Every check decides the status (Rule I).

| id | clause | quantity | limit | estimate |
|---|---|---|---|---|
| F0 | A (set-up) | area-weighted mean of `p0` over each of `wall`, `inlet`, `outlet` | each `>= 0.5` Pa, and `p0 > 0` in every cell | `1.25`, `1.04`, `1.46` |
| C1 | A | all three patches `Wall`, 100 steps: `abs(sum p V - sum p0 V) / sum abs(p0) V` | `<= 1e-12` | round-off, `<= 1e-14` (ADR 0366 U8: `2.2e-16`; ADR 0351 U1: `6.5e-15`) |
| O1 | B | `inlet` `Open`, others `Wall`: the same relative change | `> 1e-3` | `0.05` to `0.2` |
| O2 | B | `outlet` `Open`, others `Wall` | `> 1e-3` | `0.1` to `0.3` |
| O3 | B | `wall` `Open`, others `Wall` | `> 1e-3` | `0.05` to `0.3` |
| O4 | B (sign) | in O1 to O3 the integral decreases (`sum p V` after `< ` before): the open boundary lets out a positive excess | holds in each | physical: `p0 > 0` leaves through an open face |

O4 is a sanity check of the direction of the change; a change of the wrong sign would be a defect of the set-up.

## Negative controls and planned mutations (scratch copy, once each, not committed)

The tests of the new file run on every mutated copy. A mutation not caught by its registered check is a miss by Rule I,
reported and diagnosed, whatever else catches it.

| id | mutation | registered check expected to catch it |
|---|---|---|
| MU3 | `acoustic_wave.rs`: the `Wall` arm given the `Open` arm's values, `(0.5 pl + 0.5 z unl, 0.5 unl + 0.5 pl/z)` (this is MU3 of ADR 0366, which U8 missed) | C1 |
| MU-L | `acoustic_wave.rs`: the `Wall` arm's normal velocity `0.0` replaced by `1e-9 * pl / z` (a leak of `2e-9` of an open wall) | C1 (estimate of the relative change: `1e-9 * 0.5 c ...` about `1e-9`, three orders above the limit) |
| MU-O | `acoustic_wave.rs`: the `Open` arm's normal velocity set to `0.0` (an open face that does not let the mass out) | O1, O2 and O3 (all three) |
| MU-F | the test's field replaced by U8's `(1 + x/R + 0.5 y/R) cos(pi z/L)`, with MU3 applied | F0 (the wall mean is zero by symmetry and the outlet mean is negative); C1 passes under MU3 with that field, which reproduces the blind spot, and is recorded as such |

MU3 and MU-L are the point of the item; MU-F shows that F0 is the guard against the field that hid MU3.

## Decision rule (registered, Rule I)

- **done** when F0, C1, O1, O2, O3 and O4 meet their limits at their first registered run with no post-run amendment, the
  existing tests of `rcfd-post` and `baseline_hashes` pass unedited, fmt and clippy of `rcfd-post` are clean, and every
  registered mutation is caught by its registered check.
- **partial** when a registered check misses (kept as `*_known_miss`, diagnosed), or a registered mutation is not caught by
  its registered check, or a check is met only after a post-run amendment.
- **todo** does not apply once code exists.

## Limits (what is not covered)

- One O-grid (n = 6), one field, one step size (`cfl 0.48`), 100 steps; no longer run, no finer grid.
- A single patch open at a time; a driven patch, the sponge layer and the moving medium are not part of this item.
- The conservation of energy is not checked (the solver's `energy` is not conserved by the exact-Riemann flux).
- Tests only: no change to `rcfd-fv`, `rcfd-mesh` or `rcfd-post/src`, no case key, CLI or MCP tool.

## Sources

- ADR 0351 (the scheme and its fluxes), ADR 0366 (U8 and MU3). The conservation argument above is the divergence theorem
  applied to the semi-discrete update; no external source is used. No vendor code or documentation was used.
