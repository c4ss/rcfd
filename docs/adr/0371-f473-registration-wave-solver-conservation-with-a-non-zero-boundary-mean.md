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

## Addendum 1: results (2026-11-12; the registered text above is unchanged)

Registration 515283c3; test 99fb5671 (the registered run: `cargo test --release -p rcfd-post --test wave_conservation_f473`,
once); 44bc0f5c replaced `!(e > 1e-3)` and `!(d < 0.0)` by the equivalent `partial_cmp` forms for clippy (no limit and no
number changed; the mutations were run on this tip). The new file is `crates/rcfd-post/tests/wave_conservation_f473.rs`; no
library line changed. A disclosed repetition: the test was also run once with `--nocapture` for the numbers; the same
numbers, digit for digit, come from the mutation control MU0 on the archive copy.

### Checks

| id | quantity | limit | measured | met |
|---|---|---|---|---|
| F0 | patch means of `p0` (wall, inlet, outlet); minimum `p0` | each `>= 0.5`; `> 0` | `1.2500`, `1.0417`, `1.4583`; `0.5738` | yes |
| C1 | all walls, 100 steps: relative change of `sum p V` | `<= 1e-12` | `3.72e-15` (absolute `-5.2e-18`) | yes |
| O1 | inlet open: relative change | `> 1e-3` | `7.61e-1` | yes |
| O2 | outlet open | `> 1e-3` | `7.18e-1` | yes |
| O3 | wall open | `> 1e-3` | `9.78e-1` | yes |
| O4 | sign of the change in O1 to O3 | decrease | `-1.06e-3`, `-1.00e-3`, `-1.37e-3` Pa m3 | yes |

My estimate of the open-patch change (`0.05` to `0.3`) was too low by a factor of 3 to 10: the change is `0.72` to `0.98` of
`sum |p0| V`. The estimate took the drained layer to be one cell thick; the reconstruction's gradient and the acoustic waves
leaving the open patch over `100 dt` involve more than the first layer, and 100 steps at `cfl 0.48` are about 0.7 transit
times of the 0.1 m duct. The limit is met by almost three orders; the estimate is recorded as wrong, not adjusted.

### Mutations

Once each, on a `git archive` copy of 44bc0f5c (`f473-scratch/tree`, own target directory, logs `MU*.log`), running all four
tests with `--include-ignored`. MU0 (no mutation): F0, C1, O1 to O4 pass; the ignored control `mu_f_u8_field_fails_f0` fails
by design (it asserts what it records is false: the U8 field's means are `0.966` (inlet), `-0.966` (outlet) and about 0
(wall)).

| id | mutation | registered check | result |
|---|---|---|---|
| MU3 | `Wall` arm given the `Open` values (ADR 0366 MU3) | C1 | **caught by C1**: relative change `9.55e-1` against the limit `1e-12` (U8 measured `3.3e-16` for the same mutation). O1 to O3 pass |
| MU-L | `Wall` normal velocity `1e-9 pl/z` instead of `0` | C1 | **caught by C1**: `7.84e-9` against `1e-12` (estimate `~1e-9`) |
| MU-O | `Open` normal velocity `0` instead of `0.5 unl + 0.5 pl/z` | O1, O2, O3 | **caught by O1, O2 and O3** (relative change `2.0e-15`, `2.9e-15`, `2.9e-15` against `1e-3`). C1 passes |
| MU-F | test field replaced by the U8 field, with MU3 applied | F0 | **caught by F0** (the field is not positive: the wall and outlet means fail). C1 passes (`3.4e-16`): with U8's field MU3 is again invisible to the conservation check, which reproduces the blind spot. O4 and O3 also fail |

### Existing tests, fmt, clippy

`cargo test --release -p rcfd-api --test baseline_hashes`: 3 passed, 1 ignored, as before (no shared file changed). The other
tests of `rcfd-post` were not re-run: no line of any library or existing test file changed. `cargo fmt -p rcfd-post --check`
clean. `cargo clippy --release -p rcfd-post --all-targets -- -D warnings` clean (first run on 99fb5671 failed with
`neg_cmp_op_on_partial_ord`, fixed in 44bc0f5c).

### Deviations and disclosures

- The estimate of O1 to O3 was off (above). The control `mu_f_u8_field_fails_f0` is an ignored test that fails when run; this
  is its purpose (a record), and it is not part of the decision.
- Not run: the other tests of `rcfd-post`, the whole workspace; longer runs, other meshes, two patches open, a driven patch.

### Decision by the registered rule (Rule I)

**done.** Every registered check meets its limit at its first run, no post-run amendment was written, every registered
mutation is caught by its registered check (MU3 by C1, MU-L by C1, MU-O by O1 to O3, MU-F by F0), baseline hashes pass, fmt
and clippy are clean. The acceptance's two clauses are met.

### Remaining questions (options and recommendation)

1. **F-453's partial status.** ADR 0366 is partial for three reasons: SP10, SP10c (diagnosed start-up transient) and the MU3
   miss. This item answers the MU3 reason. Options: (a) leave F-453 partial, since the SP misses remain; (b) record in the
   F-453 note that U8's blind spot is closed by F-473. Recommendation: (b), the status unchanged.
2. **A conservation check for a driven patch** (the sum of the flux in, against the work done by the driver). Options: (a) a
   follow-up; (b) none. Recommendation: (b) now; open it if the wave solver gets a case key.

## For the lead: roadmap entries

**F-473, proposed status: `done`.** Proposed note text:

> DONE, ADR 0371, 2026-11-12. Tests only: crates/rcfd-post/tests/wave_conservation_f473.rs; no library change. On the closed circular O-grid duct (circular_duct_ogrid(6, 3, 6), 648 cells) the initial pressure p = 1 + 0.4 x/R + 0.3 y/R + 0.5 z/L has a mean of 1.25 on the wall, 1.04 on the inlet and 1.46 on the outlet. With every patch a wall, 100 steps of cfl 0.48 change sum p V by 3.7e-15 relative (limit 1e-12). With one patch open the integral decreases by 0.76 (inlet), 0.72 (outlet), 0.98 (wall) of sum |p0| V (limit 1e-3; the registered estimate of 0.05 to 0.3 was too low). Mutations: the wall given the open flux (MU3 of ADR 0366, which U8 missed with its zero-mean field) changes the integral by 0.955 and is caught by the closed check; a wall leak of 1e-9 is caught (7.8e-9 against 1e-12); an open face that lets no mass out is caught by all three open checks; the old U8 field is rejected by the boundary-mean check, and with it the MU3 mutation is again invisible to the conservation check. Limits: one mesh, one field, 100 steps, one patch open at a time; no driven patch.

**F-453, note addition** (status unchanged, `partial`): append to its note:

> The blind spot of U8 (mutation MU3 escaping with a zero-boundary-mean field) is closed by F-473 (ADR 0371, 2026-11-12), whose closed-duct check with a non-zero boundary mean catches it.
