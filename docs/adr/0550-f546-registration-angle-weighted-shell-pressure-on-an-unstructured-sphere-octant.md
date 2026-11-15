# ADR 0550: F-546 registration: the angle-weighted shell pressure on an unstructured triangulation of a sphere octant (the error, its cause and its convergence on three meshes)

Status: **REGISTERED 2026-11-15, written before any code of F-546 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-546 was written, compiled or run in the repository. The files listed under "What was
read" were read, and the estimates below come from plain numpy scripts in the scratch folder (`w49i-work/f546-scratch`:
`proto.py`, a copy of `f540-scratch/proto.py`, the F-419 prototype of Shell3 that matched rcfd to `2.2e-9` on every check of
ADR 0454; `sphere.py`, the mesh generator and the octant model; `est64.py`, the same model at n = 64 assembled on the free
unknowns only; `diag_t.py`; logs `est1.log`, `est2.log`, `est3_n64.log`, `diag1.log`, `mesh.log`; dense matrices, no rcfd
code; disclosed, their numbers quoted below). Results go in the addenda at the end; nothing above the addenda is changed after
a run.

Roadmap item F-546 (deps F-540), structural solver, advanced tier, optional, low priority; the follow-up named by
[ADR 0461](0461-f540-registration-angle-weighted-shell-pressure.md) (its limit L2 and remaining question 2). Number 0550 given
by the lead. Branch `f546-work` from main e906faa8, worktree `w49i-work/f546`, scratch `w49i-work/f546-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited
(new tests in a new file); no new dependency, no unsafe code, the crate graph unchanged; clean-room. Every choice below that the
maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation), except the open questions at the end.

## What F-546 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. A sphere octant triangulated without structure under internal pressure with `Load::shell_pressure_angle_weighted`: the radial displacement within 2 % of `p R^2 (1 - nu)/(2 E t)` at every node, and the error measured on two refinements.

Clauses: (a0) every limit registered before the run (this document); (a1) a sphere octant, triangulated without structure,
internal pressure with the angle-weighted load, `max_nodes |u_r / ref - 1| <= 2e-2`, `ref = p R^2 (1 - nu) / (2 E t)`, `u_r`
the displacement along the radius through the node; (a2) the error measured on two refinements of the mesh. The acceptance
fixes neither the thickness nor the mesh size; both are fixed below from a derivation (design 4) before any run.

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0461 (whole); the roadmap entries F-540 and F-546.
- `crates/rcfd-fem/src/shell3.rs` (`facet`, `MIN_ANGLE_DEGREES = 1`, `pressure_load_vector_angle_weighted`), `model.rs` (the
  `Load` builders, `Load::shell_pressure_angle_weighted`), `loads.rs` (`NodalForceAt`), `solve.rs` (`solve_static_with`,
  `StaticOptions`); the test `crates/rcfd-fem/tests/sa_shell_pressure_angle.rs` (the model helper and the symmetry conditions
  of the cylinder quarter).
- `f540-scratch/proto.py`, `diag.py`.
- **Not read:** any finite-element code, manual or documentation outside this repository, and no paper. Everything used below
  is derived here. Recalled from memory and used only for the scaling of design 3, not for any number: the deflection under a
  point load `F` of a thin spherical shell, `w = sqrt(3 (1 - nu^2)) F R / (4 E t^2)` (E. Reissner's shallow-shell solution, as
  remembered); only its form `w ~ F R / (E t^2)` is used, and the scaling it gives is checked by the runs D4, D5.

## Design

### 1. The model

A sphere of radius `R = 1`, thickness `t = 0.01` (the `t / R` of the cylinder of ADR 0454 and 0461), `E = 2e11`, `nu = 0.3`,
internal pressure `p = 1e5`; the octant `x, y, z >= 0` with the symmetry conditions of its three planes: on `x = 0` the
components `(u_x, theta_y, theta_z)` = `(0, 4, 5)` zero, on `y = 0` `(1, 3, 5)`, on `z = 0` `(2, 3, 4)` (the union at a node on
two planes; the three corners on three). These remove every rigid motion. Shell3 elements with the default membrane, one
block, the load `Load::shell_pressure_angle_weighted("all", p)`.

Reference (closed form): the membrane state of a pressurised sphere, `N = p R / 2` in every direction; the isotropic strain
`N (1 - nu) / (E t)`, so `u_r = p R^2 (1 - nu) / (2 E t) = 1.75e-5`. The uniform expansion changes the curvature uniformly;
the moment it carries is uniform and self-equilibrated, and its effect on `u_r` is of relative order `t^2 / R^2 = 1e-4` (not
derived further; it is below every limit by two orders).

### 2. The unstructured mesh (fixed now; the generator of `sphere.py`, reproduced in the test)

1. **Geodesic octant grid of level n.** Nodes `(i, j, k)`, `i + j + k = n`, at `normalise(i e_x + j e_y + k e_z)`, numbered
   with `i` outer, `j` inner; triangles `[(i, j), (i + 1, j), (i, j + 1)]` for `i + j < n` and `[(i + 1, j), (i + 1, j + 1),
   (i, j + 1)]` for `i + j < n - 1`, in that order (outward normals: `(e_x - e_z) x (e_y - e_z) = (1, 1, 1)`).
2. **Jitter.** A 64-bit linear congruential generator (`s <- 6364136223846793005 s + 1442695040888963407 mod 2^64`, value
   `(s >> 11) / 2^53`), seed `546 + n`. Every node draws two numbers `a, b` in turn, scaled to `[-0.2, 0.2)` grid units. An
   interior node moves to `(i + a, j + b, k - a - b)`; a node of an edge of the octant moves along it (`k = 0`: `(i + a, j - a,
   0)`; `i = 0`: `(0, j + a, k - a)`; `j = 0`: `(i + a, 0, k - a)`), so it stays on its symmetry plane; the corners stay. Then
   it is normalised onto the sphere.
3. **Random flips.** One pass over the triangles in order and over each triangle's edges in order: an edge `a -> b` of triangle
   `t` whose twin `b -> a` belongs to a triangle `t2 > t`, with neither triangle flipped yet, draws a number `u`; if `u < 0.5`,
   both new triangles `(a, d, c)`, `(d, b, c)` (c, d the opposite nodes) face outward (`((x_1 - x_0) x (x_2 - x_0)) . centroid >
   0`) and their smallest angle (by `atan2(|u x v|, u . v)`) is at least 20 degrees, the edge is flipped and both triangles are
   marked. Boundary edges (on the symmetry planes) have no twin and are never flipped.

`mesh.log`: n = 16, 32, 64, 128 give 153, 561, 2145, 8385 nodes, 256, 1024, 4096, 16384 triangles, 72, 286, 1185, 4764 flips,
smallest angle 20.05, 20.07, 20.00, 20.00 degrees; the interior valences run from 3 to 10, 11, 12, 11 (at n = 64: 37 nodes of
valence 3, 236 of 4, 478 of 5, 532 of 6, 414 of 7, 169 of 8, 74 of 9, 13 of 10 to 12). No two levels are nested: each is its
own draw. The structured grid (no jitter, no flips) is kept as a comparison.

### 3. What error to expect, and why (arithmetic and runs before the registration, disclosed)

1. **The load of which the uniform expansion is the exact discrete solution.** Let `u = alpha x` (a uniform scaling about the
   centre). Each flat facet is scaled in its plane and translated along its normal: its membrane strain is the isotropic
   `alpha`, its nodal rotations are zero and its local `w = alpha (x . e_3)` is constant over the facet, so there is no bending,
   no transverse shear and no drilling strain. The CST nodal force of a triangle under the isotropic membrane force `N` is `N
   dA_t / dx_i` (the virtual work of an isotropic stress is `N dA`), `dA_t / dx_i = (1/2) e_3 x (x_k - x_j)` for `(i, j, k)` in
   cyclic order. With `N = E t alpha / (1 - nu)` and `alpha R = ref`, the loads `F_i = (p R / 2) sum_t dA_t / dx_i` (the
   *area-gradient load*) make `u = alpha x` an exact solution on any triangulation inscribed in the sphere; the symmetry
   conditions are consistent with it (`u_x = x alpha = 0` on `x = 0`, rotations zero) and take the reactions. **D1**
   (`est2.log`, `est3_n64.log`): with that load the prototype gives `|u_r / ref - 1| <= 1.5e-13, 1.0e-12, 2.7e-12, 1.4e-11` at
   n = 8, 16, 32, 64 on the unstructured meshes. So the stiffness of Shell3 is not the source of the error on this problem.
2. **What the pressure loads are.** The default `p A / 3` along each facet normal is the derivative of the enclosed volume of
   the polyhedron (`dV / dx_i = (1/6) sum x_j x x_k` over the fan of `i`, which differs from `sum A_t e_3 / 3` by `(1/6) x_i x
   sum (x_k - x_j) = 0` for a closed fan); the angle split of ADR 0461 is another lumping of the same facet forces. Neither is
   the area-gradient load: on an irregular mesh `dA / dx_i` has tangential parts (moving a node tangentially changes the area)
   and its normal part follows the mesh's local discrete curvature, while the pressure loads are normal to the facets and
   follow the facet areas. The difference is a nodal load `delta F_i ~ c p h^2` with `c` of order one on a mesh whose cells
   vary at random by O(1) from node to node (it does not fall with refinement, since the irregularity of design 2 is the same
   at every level).
3. **How the difference shows in `u_r`.** A thin sphere resists a local nodal load mainly by bending around the point: with the
   remembered form `w ~ F R / (E t^2)` (above), `delta w / ref ~ c p h^2 R / (E t^2) / (p R^2 / (E t)) = c h^2 / (R t)`. The
   error is predicted second order in the mesh size and inversely proportional to the thickness, for either pressure load,
   with different constants `c`.
4. **Runs D2 to D5 (prototype).** `max_nodes |u_r / ref - 1|`:

   | n | unstructured, angle | unstructured, default | structured, angle | structured, default |
   | --- | --- | --- | --- | --- |
   | 8 | `9.6665e-1` | `1.8242` | `3.2457e-2` | `4.7289e-1` |
   | 16 | `5.5648e-1` | `7.8775e-1` | `1.0872e-2` | `1.7532e-1` |
   | 32 | `1.4014e-1` | `2.2612e-1` | `2.9233e-3` | `4.8627e-2` |
   | 64 | `3.3342e-2` | `5.3407e-2` | not run | not run |

   Ranges of `u_r / ref` (the P checks below): unstructured angle n = 16 `[0.7975032605, 1.5564788140]`, n = 32
   `[0.8598630675, 1.1257115733]`, n = 64 `[0.9666577934, 1.0303006206]`; unstructured default n = 16 `[0.6349865117,
   1.7877475664]`, n = 32 `[0.7738809759, 1.2075570046]`, n = 64 `[0.9465929416, 1.0464244510]`; structured angle n = 16
   `[0.9891279349, 1.0061449536]`, n = 32 `[0.9970766504, 1.0016982460]`; structured default n = 16 `[0.8246763589,
   1.0033889813]`.

   (D4) the unstructured angle error falls by `1.74, 3.97, 4.20` per halving of `h` (second order from n = 16 on, as design 3
   predicts); the default by `2.32, 3.48, 4.23`. (D5, `diag1.log`) at n = 32 the angle error is `8.54e-2, 1.40e-1, 2.67e-1` for
   `t = 0.02, 0.01, 0.005` (ratios `1.64, 1.91` per halving of `t`, tending to the predicted `1 / t`); the default `1.25e-1,
   2.26e-1, 4.98e-1`. The largest error sits at a node of a symmetry plane at n = 16 and n = 32 (`diag1.log`).
5. **What this says.** On the unstructured octant the angle split is better than the default by a factor of about 0.62 at n =
   32 and 64, not by the factor of 300 of the one-diagonal cylinder of ADR 0461: there, a rectangle cell gets the exact `A / 4`
   per corner and the remaining error is the polygon's; here no lumping of facet forces is exact, and the thin shell turns the
   lumping's nodal unevenness into `c h^2 / (R t)`. On the structured geodesic grid the angle split is 16 times better than the
   default (n = 16 and 32). The acceptance's 2 percent is a matter of mesh size for either load.

### 4. The meshes of the checks (derived from design 3 and D4, fixed now)

Second order with the measured constant: `e(n) ~ 3.33e-2 (64 / n)^2`. `e <= 2e-2` needs `n >= 64 sqrt(3.33 / 2) = 83`; the
next level of the doubling sequence is n = 128 (edges about `pi / (2 x 128) = 0.012 R`), with the estimate `3.33e-2 / 4.2 =
7.9e-3` to `3.33e-2 / 4.0 = 8.3e-3`. The checks use the base level n = 32 and its two refinements n = 64 and n = 128 (clause
a2), the acceptance's 2 percent at the finest (clause a1), and n = 16 as a further comparison with the prototype. The default
at n = 128 is estimated at `5.34e-2 / 4.23 = 1.26e-2`: it would meet 2 percent there as well; this is recorded as observation
O1 (no limit) and named in the note.

The n = 128 model (50310 unknowns) is one `#[ignore]`d heavy test run once with `--ignored`; the fast test covers the same
code path on n = 16 to 64.

### 5. No library change

F-546 is a measurement of the load F-540 added: no line under `crates/*/src` changes, so defaults are bit-identical by
construction (B1 checks it). One new test file, `crates/rcfd-fem/tests/sa_shell_pressure_sphere.rs`, holds the mesh
generator of design 2, the model of design 1 and every check.

## Checks (limits fixed now)

`e(n)` = `max_nodes |u_r / ref - 1|` on the unstructured mesh of level n with the angle-weighted load unless stated.

| id | clause | check | limit | estimate behind it |
| --- | --- | --- | --- | --- |
| A1 | a1 | `e(128)` | `<= 2e-2` | design 4: `7.9e-3 .. 8.3e-3` |
| A2 | a2 | the error on the base and its two refinements, `e(32)`, `e(64)`, `e(128)`, recorded; observed order `log2(e(32) / e(64))` and `log2(e(64) / e(128))` | each `>= 1.5` | D4: `2.07`; `2.0` predicted |
| G1 | a1 | the generated meshes equal the prototype's: nodes, triangles, flips at n = 16, 32, 64, 128 (`153, 256, 72`; `561, 1024, 286`; `2145, 4096, 1185`; `8385, 16384, 4764`); smallest angle `>= 20` degrees; every node of a symmetry edge on its plane to `1e-15`; every triangle outward | exact; `>= 20`; `1e-15`; all | `mesh.log` |
| G2 | a1 | "without structure": interior valences at n = 64 include at least five distinct values; at least 25 % of the triangles flipped (`2 x flips / triangles`) | `>= 5`; `>= 0.25` | `mesh.log`: 10 values, `0.58` |
| P1 | a1 | against the prototype, the angle split: min and max of `u_r / ref` at n = 16, 32, 64 (D2 ranges) | `1e-5` relative each | the prototype matched rcfd to `2.2e-9` (ADR 0454) |
| P2 | design 3 | the default `Load::shell_pressure`: min and max at n = 16, 32, 64 (D2 ranges) | `1e-5` relative each | as P1 |
| P3 | design 3 | the structured grid, angle split: min and max at n = 16, 32 | `1e-5` relative each | as P1 |
| D1 | design 3.1 | the area-gradient nodal forces (`NodalForceAt`, no pressure load) on the unstructured n = 32 mesh: `max_nodes |u_r / ref - 1|` | `<= 1e-8` | `2.7e-12` |
| R1 | design 3.5 | `e(n) / e_default(n)` at n = 32, 64 (fast) and 128 (heavy) | `<= 0.8` each | `0.620, 0.624`; 128 estimated `0.66` |
| S1 | design 3.5 | the structured grid, angle split, n = 16: `max_nodes |u_r / ref - 1|` | `<= 2e-2` | `1.0872e-2` |
| NC1 | design 4 | the unstructured angle split at n = 64 misses the acceptance's limit: `e(64)` | `> 2e-2` | `3.33e-2` |
| NC2 | design 3.5 | the structured grid with the default, n = 16 | `> 5e-2` | `1.75e-1` |
| B1 | defaults | `git diff e906faa8 -- crates/*/src` empty (no library line changed) | empty | design 5 |
| B2 | defaults | `cargo test -p rcfd-fem --release` (every test binary of the crate, unedited, and the new one) | pass | design 5 |
| B3 | defaults | `cargo test -p rcfd-api --release --test baseline_hashes`; `cargo test -p rcfd-cli --release --test docs_site` | pass | design 5 |

Observation O1 (no limit, recorded): `e_default(128)` (estimated `1.26e-2`), from the heavy test.

## Planned mutations (on a scratch copy of the library, one at a time, own `CARGO_TARGET_DIR`; the fast test only)

| id | mutation | expected to be caught by |
| --- | --- | --- |
| MU1 | `pressure_load_vector_angle_weighted` weights `1 / 3` (angles ignored) | P1, R1 |
| MU2 | node `i` takes the angle of node `i + 1` | P1 |
| MU3 | the angle-weighted force along `-e_3` | P1, R1 |
| MU4 | the `ShellPressureAngleWeighted` arm of `nodal_load_vector` passes `angle_weighted = false` | P1, R1 |
| MU5 | the Shell3 membrane stiffness scaled by `1 + 1e-3` | D1, P1, P2, P3 |

## Decision rule

- **done:** A1, A2, G1, G2, P1 to P3, D1, R1, S1, NC1, NC2, B1 to B3 met, every mutation caught by a registered check, no
  amendment after a run.
- **partial:** a check missed (recorded as a `*_known_miss`) or not run, a mutation not caught, or a check met only after an
  amendment written after its run.
- **todo:** the test does not build.

## Limits (not covered)

- L1. The mesh is a jittered and randomly flipped geodesic grid (design 2), not the output of a Delaunay or advancing-front
  mesher; its irregularity (jitter 0.2 grid units, flips of 58 % of the triangles, smallest angle 20 degrees) is fixed and does
  not grade. A smoother unstructured mesh is expected to give a smaller constant `c` (design 3.2), not a different order.
- L2. One thickness (`t / R = 0.01`) is checked; the `1 / t` growth (D5) means a thinner shell needs a proportionally finer mesh
  for the same error, for either load.
- L3. The area-gradient load of design 3.1 needs the radius and is a diagnosis only, not a load of the library; a general
  pressure load with the uniform state exact on irregular curved meshes is not provided (open question 2).
- L4. Only closed-form membrane reference; no bending-dominated sphere problem, no Shell4 cells.

## Open questions

Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation):

1. The mesh family and its levels (design 2, 4): taken as above, rather than a mesher (none in the repository for curved
   surfaces, and writing one is outside the item).

Not taken:

2. Whether a pressure load whose nodal forces make the uniform membrane state exact on irregular curved triangulations should be
   designed (for example from a local fit of the curvature): options (a) leave as a limit; (b) a follow-up item that designs and
   registers it. Recommendation (b), low priority, only if the 2 percent at coarse unstructured meshes matters to a user.
3. The default pressure of Shell3 (ADR 0461 question 4): unchanged; recommendation still to keep the consistent split as the
   default.
