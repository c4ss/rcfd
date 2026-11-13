# ADR 0414: F-504 registration: an opt-in projection that evaluates the bilinear shape functions on quadrilateral interface elements

Status: **REGISTERED 2026-11-13, written before any code of F-504 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-504 was written, compiled or run in Rust; the documents and code under "What was read"
were read, and the arithmetic under "Arithmetic before this registration" was done with one Python script (numpy). Results go
in the addendum at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0412](0412-f502-registration-nonmatching-coupled-frequency.md) (F-502: the coupled frequency change on the 3:2
wall predicted from the static compliance; D6, the projection's asymmetry is the triangle split of each quadrilateral along its
`(0, 2)` diagonal; the test-side bilinear evaluation `Q`; question 1, recommendation (a), from which this item comes),
[ADR 0263](0263-f362-registration-interface-mapping.md) (F-362: `interface_rows`, the projection D4/D5 with the offset
correction, the walls BP, BU, BC, the rigid-motion and balance checks) and
[ADR 0410](0410-f400-registration-dynamic-solid-participant.md) (F-400: `DynamicSolid`, the bar and the piston mock). Roadmap
item F-504 (deps F-362, F-502). Number 0414, branch `f504-bilinear-quad-projection` (from main `5638ad5a`), worktree
`w49i-work/f504`, scratch `w49i-work/f504-scratch`, given by the lead.

House rules (`w49i-work/AGENT_RULES.md`, the campaign rules of 2026-11-12, applied on 2026-11-13): limits fixed here and not
changed after a run; a missed limit becomes an ignored `*_known_miss` test with the limit intact; a defective set-up is
corrected only by an amendment committed before the corrected run; each registered run is made once; existing tests and records
are not edited; defaults bit-identical; `baseline_hashes.rs`, the hash tables and the goldens are not touched; no new crate, no
unsafe code; unsigned commits without trailers; no push. Standing decision of this tier: a scaffold test (`si_*`) retired by a
landed package becomes an ignored record citing its ADR with an `_after_landed` copy; F-504 retires none (no scaffold test is
superseded: the new map is opt-in and the F-502 tests keep testing the default projection).

Clean-room. Nothing outside this repository was opened. The bilinear shape functions of a four-node quadrilateral and Newton's
method for a stationary point of a smooth function of two variables are textbook material, derived below; nothing is taken from
a code or a paper.

## What F-504 asks

The acceptance of the roadmap entry, binding: "Every limit not stated here is registered in the ADR before the run. An opt-in
variant of the projection of F-362 evaluates the bilinear shape functions on quadrilateral structural elements (the nearest
point on the bilinear patch); the default projection keeps its weights bit for bit (the F-362, F-400 and F-502 tests pass
unedited). On the 3:2 walls of F-502 its weights are invariant under the eight symmetries of the square within 1e-13 and equal
the test-side bilinear evaluation of ADR 0412 within 1e-13; rigid translation and rotation are reproduced and resultant, moment
and work conserved within 1e-12 on a planar and on a cylindrical wall; the coupled frequency change of F-502 equals its
static-compliance prediction within 10 percent for faces of 2 x 2, 4 x 4 and 8 x 8 elements."

Clauses: (c0) every other limit registered here; (c1) an opt-in variant that evaluates the bilinear shape functions at the
nearest point of the bilinear patch of each quadrilateral; (c2) the default projection's weights bit for bit, the F-362, F-400
and F-502 tests unedited and passing; (c3) on the 3:2 walls of F-502 (`n = 2, 4, 8`) the weights invariant under the eight
symmetries within `1e-13`; (c4) equal to ADR 0412's test-side `Q` within `1e-13`; (c5) rigid translation and rotation
reproduced within `1e-12` on a planar and a cylindrical wall; (c6) resultant, moment and work conserved within `1e-12` there;
(c7) the coupled frequency change equals the static-compliance prediction within 10 % for `n = 2, 4, 8`.

## What was read (no Rust run)

ADR 0412 (whole), ADR 0263 (D1 to D8, the checks, the addendum: N1 measured `4.98e-3` rotation, `1.04e-4` moment), ADR 0410
(D4 to D7 and amendment 1 through ADR 0412's summary); `crates/rcfd-fv/src/cosim/mapping.rs` (whole), `crates/rcfd-fv/src/
cosim.rs` (`InterfaceMap::build`, the re-exports), `crates/rcfd-fv/tests/cosim_f362.rs` (the walls BP, BU, BC, `rigid`,
`balance`, R1, R2, C1 to C3, U1 to U3, N1), `crates/rcfd-post/tests/si_solid_dynamic_nonmatching.rs` (whole), `crates/rcfd-post/
src/solid_coupling.rs` (`SolidInterface::build_map` passes the settings through), `docs/cosimulation-protocol.md`. A grep shows
`MapMethod` matched only in `mapping.rs` and constructed in tests (`cosim_f362.rs` with `Projection { correct_offset: false }`
and `RadialBasis`); no case-file schema or golden names it.

## Arithmetic before this registration (disclosed)

One script, `w49i-work/f504-scratch/prearith.py` (numpy), log `prearith.log`. It implements the search of D2 below in Python and
runs it on the warped patch of check W1/W2, and computes the axial weights `w = B^T (A_f/A)` on the 3:2 walls with the triangle
split and the bilinear evaluation (the estimate of N1). Printed:

* W1 (18 lifted points): weights within `1.1e-16` of `N(s0, t0)`, distance within `7.6e-17` of `0.05`; 1 to 5 Newton steps
  (the step that meets the stopping test counted).
* W2 (3 points beyond the edge `t = 0`): weights and distance exact (`0`).
* MU2 (one Newton step only): the W1 weight error `6.4e-2`.
* `max |w_P - w_Q| / max w_P` on the 3:2 walls: `0.111` (`n = 2`), `0.0741` (`n = 4, 8`); both weight sets sum to 1 to 15 digits.

Nothing else was computed. The estimates of the frequency checks are ADR 0412's measured `Q` numbers (the new map's rows are
`Q`'s to rounding on these flat square faces, D4).

## Design

### D1. The opt-in

A new variant `MapMethod::BilinearProjection { correct_offset: bool }` and a constructor `MapSettings::bilinear_projection
(max_distance)` (offset correction on, `Unmapped::Refuse`). Everything else of `interface_rows` is shared with the projection:
the nearest element (the first at the smallest distance, strict comparison), the distance, the unmapped policy (an outside
point gets the plain row of its foot), the offset correction on the ring (ADR 0263 D5) and the report. Only the nearest point
on an element of **four** nodes and its weights differ; elements of 1, 2 or 3 nodes use the existing `nearest_on_element`.
`MapMethod::Projection` and `RadialBasis` call exactly the code they call now (D5).

### D2. The nearest point on the bilinear patch

Corners `x_0..x_3` in the element's order, `(s, t)` in `[0, 1]^2`:

    X(s, t) = (1 - s)(1 - t) x_0 + s (1 - t) x_1 + s t x_2 + (1 - s) t x_3,     N = ((1 - s)(1 - t), s (1 - t), s t, (1 - s) t).

`sum N = 1` identically, so a row of `N` sums to 1 (translation, resultant); `sum N_i x_i = X(s, t)`, so the row reproduces its
foot exactly and the precision argument of ADR 0263 D1/D5 is unchanged. The edges `s = 0, 1`, `t = 0, 1` of the patch are the
straight segments between consecutive corners, and `N` restricted to an edge is the linear interpolation between its two corners.

The nearest point minimises `f(s, t) = |X(s, t) - x|^2 / 2` over the closed square. A continuous function on a compact set has
its minimum either at an interior stationary point or on the boundary; both are computed and the nearer taken:

* **Interior:** Newton's method on `grad f = (r . X_s, r . X_t)`, `r = X - x`, with `X_s = (1 - t)(x_1 - x_0) + t (x_2 - x_3)`,
  `X_t = (1 - s)(x_3 - x_0) + s (x_2 - x_1)` and the exact Hessian `H = [[X_s . X_s, X_s . X_t + r . X_st], [., X_t . X_t]]`,
  `X_st = x_0 - x_1 + x_2 - x_3` (`X_ss = X_tt = 0`). Start `(0.5, 0.5)`; at most 50 steps. A step is taken only when `H` is
  positive definite by the test `H_00 > 0` and `det H > 1e-12 H_00 H_11` (a degenerate element or a non-minimum region gives no
  interior point); the iteration stops without an interior point when `|s - 0.5|` or `|t - 0.5|` exceeds `1.5` (it has left the
  patch) and with one when the step's largest component is `<= 1e-14`, if then `0 <= s, t <= 1`. On a parallelogram `X_st = 0`
  and `X` is affine: the first step lands on the foot to rounding, the second is below `1e-14`. The tolerance `1e-14` in `(s, t)`
  is `1e-14 h` in position, below every `1e-13` limit here; 50 steps are far beyond the quadratic convergence seen in the
  arithmetic (5 steps on the warped patch).
* **Boundary:** the four edges `(0, 1), (1, 2), (2, 3), (3, 0)` with the existing `on_segment`, the first at the strictly
  smallest distance; weights `1 - u`, `u` on the edge's corners.
* The interior point is taken when it exists and its distance is `<=` the boundary's.

On a strongly warped element `f` can have more than one interior local minimum; the Newton start at the centre then finds one
of them (stated as a limit: interface quadrilaterals of a structural mesh are planar or nearly so; the walls checked here are
planar elements, plus the moderately warped W1).

### D3. Why the bilinear rows are symmetric and equal `Q` on the 3:2 walls

On a flat axis-aligned square element, `X` is affine and its inverse gives `s = (y - y_0)/h`, `t = (z - z_0)/h` in the
element's corner order (`HEX_FACES[3] = [1, 2, 6, 5]`: `s` along `y`, `t` along `z`). `Q` (ADR 0412 D6) computes the same `s, t`
by dot products and the same `N`; so the rows agree to rounding (a few `1e-16`), and where a wall point lies on an element edge
(a choice of element left to rounding) the two adjacent elements give the same `N` values on the shared edge (continuity of the
bilinear interpolant). The bilinear interpolant on a square grid commutes with the eight symmetries of the square (each maps the
grid to itself and the tensor-product functions to tensor-product functions), so the weights are invariant (ADR 0412 S4:
`8e-16`). The planar offset correction adds a rounding-size row change only (the defect `x - X` is `1e-16`).

### D4. The walls

* **3:2 walls of F-502** (clauses c3, c4, c7): the bar `hex_box([0,0,0], [4, 1, 1], [20, n, n])`, `n = 2, 4, 8`, interface
  `xmax`, the `m x m` wall, `m = 3n/2`, in the plain order; everything of ADR 0412 D4/D5 unchanged, with
  `MapSettings::bilinear_projection(1e-9)` as the map. Its 1D prediction, piston mock, runs (320 steps, Aitken from 0.5,
  tolerance `1e-8`, caps 4 and 20) and balances are copied from `si_solid_dynamic_nonmatching.rs` into a new test file (test
  helpers are not shared). The test-side `Q` and its run are rebuilt in the same file (E1).
* **Planar wall BP and cylindrical wall BC of F-362** (clauses c5, c6): copied from `cosim_f362.rs` (the planar 6 x 4 node grid
  of quadrilaterals in the tilted plane against the 14 x 6 fluid points; the 16 x 5 node cylinder of radius 0.05 against 27 x 9
  fluid points), with the same rigid motions (`rigid`: translation, rotation by 0.3 rad about a skew axis, both) and the same
  `balance` (random face forces and displacements of seeds 1, 2, 3; resultant, moment about `CENTRE`, work, each relative), and
  BU (the first fluid column `0.3 h` beyond the structure) for the unmapped policy. The cylinder's quadrilaterals are planar
  chord rectangles, so the bilinear foot equals the triangle-split foot (the nearest point of a plane rectangle) and the offset
  correction (which depends only on the foot and the ring) is the projection's: the rigid errors and balances should be those of
  F-362 (rounding, `1e-15`).
* **Warped patch W** (clause c1): corners `(0,0,0), (1,0,0), (1,1,0.4), (0,1,0)`, so `X(s, t) = (s, t, 0.4 s t)`. W1: the 18
  points `X(s0, t0) +- 0.05 n_hat(s0, t0)`, `s0, t0` in `{0.15, 0.5, 0.85}`, `n_hat` the unit normal `(-0.4 t0, -0.4 s0, 1)/|.|`;
  the foot is `X(s0, t0)` (the lift `0.05` is far below the smallest radius of curvature of the patch, `>= 2.5`), the distance
  `0.05`, the weights `N(s0, t0)`. W2: the 3 points `(s0, -0.2, 0)`, nearest point on the edge `t = 0` at `(s0, 0, 0)` (for
  `t >= 0`, `|x - X|^2 = (s - s0)^2 + (t + 0.2)^2 + 0.16 s^2 t^2` grows with `t`), distance `0.2`, weights `(1 - s0, s0, 0, 0)`.
  Both with `max_distance = 1` and the offset correction off (a single element: the ring is the element, flat in no direction,
  and the correction would move the row off the closed form; the plain rows are the clause's object).

### D5. Default bit-identity

The library change is additive: a new enum variant, a new constructor, a new function for the nearest point on the patch, and
in `interface_rows`/`nearest` the dispatch to it only for the new variant. No existing arithmetic expression on the path of
`Projection` or `RadialBasis` changes. Shown by: (D-a) a digest of the bits of every row (node index and weight) and of the
report (distances, outside list, precision defect, condition) of `Projection { true }`, `Projection { false }` and
`RadialBasis` on BP, BC, BU (`Nearest`) and three flat `n x n` Hex8-ordered faces with their 3:2 walls (`n = 2, 4, 8`), computed
by one scratch test on a `git archive` of the parent `5638ad5a` and on one of the branch's library commit: equal; (D-b) the
F-362, F-400, F-502 tests unedited; (D-c) the baseline hashes (`mapping.rs` is in `rcfd-fv`).

## Registered checks

Every value is printed; a test fails at the end if any of its checks missed (the `Checks` helper of ADR 0412).

### A. The acceptance

| Id | Clause | Quantity | Limit | Estimate |
|---|---|---|---|---|
| A0 | c0 | every limit of this table registered before the run | holds | holds |
| W1 | c1 | warped patch, lifted points: `max |H_pi - N_i(s0, t0)|`; `max |distance - 0.05|` | `<= 1e-12`; `<= 1e-13` | `1.1e-16`; `7.6e-17` |
| W2 | c1 | warped patch, beyond the edge: `max |H_pi - (1 - s0, s0, 0, 0)_i|`; `max |distance - 0.2|` | `<= 1e-12`; `<= 1e-13` | `0`; `0` |
| D-a | c2 | digest of the default rows and reports, parent against branch, every wall and method of D5 | equal bit for bit | equal |
| D-b | c2 | `cosim_f362` (rcfd-fv), `si_solid_dynamic` (F-400) and `si_solid_dynamic_nonmatching` (F-502) (rcfd-post), release, unedited | all pass, ignored as before | pass |
| D-c | c2 | `cargo test -p rcfd-api --release --test baseline_hashes` | passes | passes |
| S-B-n | c3 | 3:2 wall, `n = 2, 4, 8`: `max |w(g x) - w(x)| / max w` for each of the seven non-trivial symmetries (21 checks) | `<= 1e-13` each | `8e-16` |
| Q-B-n | c4 | 3:2 wall: `max |w - w_Q| / max w` (axial weights); companion: the rows, `max |H_pi - H^Q_pi|` | `<= 1e-13`; `<= 1e-13` | `3e-16`; `3e-16` |
| R-B | c5 | BP and BC: rigid error (as F-362 R1, relative to `max |u|`) for translation, rotation, both | `<= 1e-12` each | `1e-15` |
| C-B | c6 | BP and BC, seeds 1, 2, 3: resultant, moment, work (as F-362 C1 to C3) | `<= 1e-12` each | `1e-16` |
| C-B-n | c7 | `abs(Delta_m / Delta_pred - 1)` with the bilinear projection, `n = 2, 4, 8` (prediction computed before the coupled runs) | `<= 0.1` each | `6.6e-4`, `1.1e-4`, `6.1e-4` |

### C. Companions

| Id | Quantity | Limit | Estimate |
|---|---|---|---|
| R2-B | BP, BC: row sums `max |sum_i H_pi - 1|`; coordinates `max |sum_i H_pi x_i - x_p| / size`; report defect / size | `<= 1e-13` each | `1e-16` |
| G-B | BC and BP: the bilinear distances equal the projection's | `<= 1e-15` m | `0` to `1e-17` |
| U1-B | BU, `Refuse`: the message names wall point 0, its distance `0.024` and `max_distance` `0.008` (as F-362 U1) | holds | holds |
| U2-B | BU, `Nearest`: outside list `0..6`; rigid error against the feet `<= 1e-12` for the three motions; translation at the points themselves `<= 1e-12` | limits | `1e-15` |
| E1 | 3:2 runs: `abs(Delta_m(bilinear) / Delta_m(Q) - 1)`, `n = 2, 4, 8` (the two maps give the same frequency) | `<= 1e-6` | `1e-12` |
| PD | `precision_defect` of the bilinear map on the 3:2 walls | `<= 1e-14` | `1e-16` |
| K0, FM, I, MB | as ADR 0412 (matching compliance `<= 1e-9`, matching frequency `<= 2e-5`, every step converged within the caps 4 and 20, resultant, moment, work of the last step of each 3:2 run `<= 1e-12`) | limits | as ADR 0412 |
| V | `MapSettings::bilinear_projection(d).validate()` passes for `d >= 0` and fails for `d < 0` and `NaN`, with the message of the projection | holds | holds |

### N. Negative controls

* N1: the default projection on the same 3:2 walls: `max |w_P - w_Q| / max w_P >= 0.05` and its axis-mirror defect `>= 0.05`
  for each `n` (estimates `0.111`, `0.074`, `0.074`; `0.17`, `0.11`, `0.11`): S-B-n and Q-B-n can fail.
* N2: `BilinearProjection { correct_offset: false }` on BC: rotation error `>= 1e-4`, moment defect `>= 1e-5` (estimates those
  of ADR 0263 N1, `4.98e-3` and `1.04e-4`: same feet, D4): R-B and C-B on BC depend on the offset correction.

### M. Planned mutations (each alone, on a `git archive` copy of the test commit in the scratch folder, after the registered run)

* MU1: the new variant calls the triangle split (`nearest_on_element`) for quadrilaterals: expected caught by S-B-n, Q-B-n.
* MU2: Newton limited to one step: expected caught by W1 (`6.4e-2`); the flat walls are unaffected (affine).
* MU3: `N_2` and `N_3` exchanged (`s t` and `(1 - s) t`): expected caught by W1, Q-B-n, S-B-n, R-B (rotation), C-B (moment).
* MU4: the offset correction skipped for the new variant: expected caught by R-B and C-B on BC.
* MU5: `MapMethod::Projection` dispatched to the bilinear search: expected caught by the F-502 test (S1, S3) and D-a.
* MU6: the interior point accepted without the test `0 <= s, t <= 1`: expected caught by U1-B, U2-B (BU's outside column then
  has distance zero to the extended patch) and W2.

## Decision rule

* **done**: every check of A met, the companions of C met, N1 and N2 hold, every planned mutation caught.
* **partial**: any check of A missed (recorded as `*_known_miss`, limit intact) or not run; or a companion missed with a
  diagnosed cause; or a negative control fails to hold; or a mutation not caught.
* **todo**: no bilinear map built.

## Limits (what is not covered)

* The interior search finds one local minimum on a strongly warped quadrilateral (D2); interface quadrilaterals of the meshes
  in this repository are planar (Hex8 faces of boxes, chord rectangles of cylinders).
* Triangles, segments and points are mapped as by the projection; Tet10/Hex20 faces (higher-order interfaces) are not covered:
  `SolidInterface` lists corner nodes only.
* Not exposed in the case file (no case-file schema names the map method; adding one would change the schema golden).
* One aspect ratio of the bar, mass ratio 4, the 3:2 ratio only, flat square and cylindrical walls; no flow solver.
* The cost: per point and element one Newton iteration of up to 50 steps and four segment tests, `O(points x elements)` as
  the projection.

## Open points taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. The shape of the opt-in: a new `MapMethod` variant (recommended) rather than a field in `Projection` (which would break the
   existing struct literals of the F-362 tests, which may not be edited).
2. The offset correction applies to the new variant as to the projection (on in the constructor), so that the cylinder's rigid
   rotation is reproduced (clause c5).
