# ADR 0415: F-508 registration: curved quadratic hexahedra on the quarter annulus (Hex20 from rcfd_fem::convert with the wall mid-edge nodes projected onto the cylinders; volume and wall areas under refinement)

Status: **REGISTERED 2026-11-13, written before any code of F-508 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-508 was written, compiled or run: the files listed under "What was read" were read and
the arithmetic below was done by hand and evaluated with a short script of the closed forms derived here
(`w49i-work/f508-scratch/estimate.py`, plain floating point, no rcfd code; disclosed). Results go in the addendum at the end;
nothing above the addendum is changed after a run.

Parent: [ADR 0413](0413-f371-registration-solid-mesh-from-existing-meshes.md) (F-371, `rcfd_fem::convert`; this item is its
limit L1 and its remaining question 3, option (a)), [ADR 0292](0292-quadratic-hexahedron-hex20.md) (F-389, Hex20,
`upgrade_to_hex20_mapped`), [ADR 0383](0383-f484-registration-hex20-face-loads-through-the-model-path.md) (the 8-node face area
of `SolidMesh::face_area`). Roadmap item F-508 (deps F-371, F-389). Size S. Number 0415 given by the lead. Branch
`f508-curved-hex20-annulus`, worktree `w49i-work/f508`, scratch `w49i-work/f508-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of `baseline_hashes.rs`,
of the hash tables or of a golden file; no new dependency, no unsafe code; clean-room. **No library code is planned**: the
functions needed exist (`from_mesh`, `upgrade_to_hex20`, `project_mid_edge_nodes`, `onto_cylinder`, `element_volume`,
`face_area`); F-508 adds one test file in `rcfd-fem`. If a library change turns out to be needed it is made by an amendment
before the run. `rcfd-mesh`, `rcfd-fv` and `rcfd-api` are not touched, so `baseline_hashes` cannot move and is not run.

## What F-508 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. The quarter annulus of ADR 0413 converted by
> rcfd_fem::convert::from_mesh and upgraded to Hex20 with the wall mid-edge nodes placed on the cylinders (by
> upgrade_to_hex20_mapped or project_mid_edge_nodes) has its projected nodes on their radius within 1e-14 and its volume error
> falling at an observed order of at least 3 over four levels; the areas of its inner and outer surface sets converge to
> pi a h / 2 and pi b h / 2 at an observed order of at least 3.

Clauses: (a1) the quarter annulus of ADR 0413 (its set-up CQ: `annulus_3d_sector(4k, k, k, a = 1, b = 2, h = 1, pi / 2,
Uniform)`, levels `k = 1, 2, 4, 8`), converted by `from_mesh`, upgraded to Hex20, wall mid-edge nodes placed on the cylinders;
(a2) projected nodes on their radius within `1e-14`; (a3) volume error falling at an observed order of at least 3 over the four
levels; (a4) areas of the surface sets `inner` and `outer` converging to `pi a h / 2` and `pi b h / 2` at an observed order of at
least 3.

Reading of the clauses, registered:

- **(a1)** route: `upgrade_to_hex20` (the identity map of `upgrade_to_hex20_mapped`) of `from_mesh(annulus)`, then
  `project_mid_edge_nodes` moving every mid-edge node that is a member of the node set `inner` (after the upgrade) by
  `onto_cylinder(., 0, z, a)` and of `outer` by `onto_cylinder(., 0, z, b)`. The upgrade adds to a node set the mid-edge node of
  every edge whose two ends it contains (ADR 0292 D6), so these are exactly the mid-edge nodes of the wall edges (circumferential
  chords and axial edges); radial edges have one end off the wall. The acceptance allows either function; the projection is the
  one ADR 0413 used for Tet10, and a map for `upgrade_to_hex20_mapped` would have to recognise wall chords by their radius, which
  is not a property of the mesh (L1).
- **(a2)** `| hypot(q_x, q_y) - r | <= 1e-14` absolute for every projected node at every level (`r = 1` or `2`).
- **(a3)** relative volume error `e_k = |V_k - V| / V`, `V = pi (b^2 - a^2) h / 4 = 3 pi / 4`, `V_k = sum element_volume`;
  observed order `log2(e_k / e_2k)` for each of the three pairs (1,2), (2,4), (4,8); each `>= 3`.
- **(a4)** `A_k(set) = sum of SolidMesh::face_area over the set's faces`; relative error against `pi r h / 2`; observed order
  for each of the three pairs, for `inner` and for `outer` separately; each `>= 3`.

## What was read (no run)

- `AGENT_RULES.md`; the brief; `docs/roadmap.json` entries F-508 and F-389; ADR 0413 (whole, including addendum 1).
- `crates/rcfd-fem/src/convert.rs` (whole: `from_mesh`, `project_mid_edge_nodes` rebuilding through `SolidMesh::new` and
  `add_block`, `onto_cylinder`); `crates/rcfd-fem/src/hex20.rs` (module doc, `volume` with the 3 x 3 x 3 rule,
  `upgrade_to_hex20_mapped` and its node-set extension, the face node order); `crates/rcfd-fem/src/mesh.rs`
  (`element_volume`, `face_tangents` for 8-node faces with `hex20::face_rule`, `face_area`); `crates/rcfd-mesh/src/gen.rs`
  (`annulus_3d_sector`: points `[r cos th, r sin th, z]` with `th = angle * it / n_theta`, patches `inner, outer, theta_min,
  theta_max, zmin, zmax`); `crates/rcfd-fem/tests/sb_convert.rs` (the Tet10 version, `curved_level`).
- Scaffold rows: no `si_*` test refuses curved Hex20 or refers to this item (grep for `F-508`, `0415`, `curved` in
  `crates/rcfd-fem/tests`); nothing is retired, so no `_after_landed` copy is needed.
- **Not read:** any finite-element code or manual outside this repository.

## Arithmetic done before the code (disclosed)

Notation: `n = 4k` circumferential divisions, angle per division `delta = pi / (2n)`, half angle `phi = delta / 2 = pi / (16 k)`;
`s = sin phi`, `c = cos phi`.

1. **A Hex20 whose nodes are extruded from a plane is the extrusion of its Quad8 trace.** On CQ every node of an element has
   `(x, y)` depending on its `(xi, eta)` position only and `z` on `zeta` only: the generator's points are `(r_i, th_j)` times
   `z_l`, chord midpoints keep this structure, and the radial projection keeps `z`. Let `g(xi, eta)` be the 8-node serendipity
   interpolant of the `(x, y)` data of the bottom face; `g` lies in `span{1, xi, eta, xi^2, xi eta, eta^2, xi^2 eta, xi eta^2}`,
   which is inside the 20-monomial Hex20 space (ADR 0292 D2: `1`, the 3 linear, 6 quadratic, `xi eta zeta` and the 6 `a^2 b`,
   and the 3 `a^2 b c`), and it matches the data at all 20 nodes (the four vertical mid-edge nodes carry their corners' `(x, y)`).
   By unisolvence the element's `(x, y)` is `g(xi, eta)`, and `z` is linear in `zeta` by the same argument. So
   `det J = (dz / 2) det J_2D(xi, eta)` and the volume of the mesh is `h` times the area of the plane region bounded by the
   traces. The same argument on an 8-node wall face whose node data depend on one face coordinate only shows that a wall edge is
   the **quadratic interpolant, in the uniform parameter, of the circle through its three nodes**, and a wall face is that arc
   extruded linearly in z.
2. **The volume is computed exactly for the curved geometry.** `hex20::volume` is the 3 x 3 x 3 Gauss sum of `det J`, which has
   degree at most 5 per direction for any Hex20 (ADR 0292 D3), so `V_k` equals the volume enclosed by the boundary surface up to
   rounding. The elements conform (a shared face has the same eight nodes), interior circumferential edges stay chords; the
   enclosed region is the prism of height `h` over the plane region bounded by the two piecewise-quadratic wall arcs and two
   radial segments.
3. **Closed form of the volume (registered as C5).** With the area formula `(1/2) closed integral (x dy - y dx)` (the radial
   segments contribute 0), one wall segment of radius `r` symmetric about the x axis, `x(u) = r (1 - (1 - c) u^2)`, `y(u) = r s u`,
   `u in [-1, 1]` (the quadratic through the points at angles `-phi, 0, phi`; at `u = 0` the projected chord midpoint is the
   point at angle 0 exactly, since radial projection of a chord midpoint lands on the bisector), has the sector area
   `(1/2) int (x y' - y x') du = (r^2 s / 2) int (1 + (1 - c) u^2) du = r^2 s (4 - c) / 3`. Summing, the volume of level `k` is
   `V_p(k) = h (b^2 - a^2) n s (4 - c) / 3`. The exact sector of a segment is `r^2 phi`, so the relative volume error is
   `e_V(phi) = 1 - s (4 - c) / (3 phi)`. Series (by hand): `s (4 - c) / 3 = phi - phi^5 / 30 + phi^7 / 252 + O(phi^9)`, so
   `e_V = phi^4 / 30 - phi^6 / 252 + ...`: **order 4**, not assumed but from the series, the `phi^3` terms cancelling (the parabola
   through three points of the circle matches its curvature at the middle). Values (script): `e_V = 4.9318e-5, 3.0930e-6,
   1.9348e-7, 1.2095e-8` for `k = 1, 2, 4, 8`; orders `3.9950, 3.9988, 3.9997`. These equal the measured Tet10 errors of ADR 0413
   (`4.93e-5 ... 1.21e-8`), as they must: both meshes have the same boundary (arithmetic 1 here, arithmetic 3 there).
4. **Closed form of the wall areas (registered as C6).** By arithmetic 1 a wall face is `X(u)` extruded in `z`: `x_u = (c'(u), 0)`,
   `x_v = (0, 0, dz / 2)`, `|x_u x x_v| = (dz / 2) |c'(u)|`, `|c'(u)| = r sqrt(s^2 + 4 (1 - c)^2 u^2)`. `face_area` uses the 3 x 3
   Gauss rule (`hex20::face_rule`, points `0, +-sqrt(3/5)`, weights `8/9, 5/9`); in `v` the integrand is constant, so the face
   area is `dz r L_G` with `L_G = (8/9) s + (10/9) sqrt(s^2 + (12/5) (1 - c)^2)`, and the set area is
   `A_p(r, k) = h n r L_G(phi)`. This is not the exact arc length of the parabola (the integrand is not a polynomial); both errors
   are in the estimate. Series (by hand): `s^2 + (12/5)(1 - c)^2 = phi^2 + (4/15) phi^4 - phi^6 / 18 + ...`, its root
   `phi (1 + (2/15) phi^2 - (11/300) phi^4)`, and `L_G = 2 phi - phi^5 / 30 + O(phi^7)` (the `phi^3` terms cancel: `-4/27 + 4/27`).
   Relative error against the circle arc `2 r phi`: `e_A = phi^4 / 60 + O(phi^6)`, the same for `inner` and `outer` (independent
   of `r`) and of the axial division. **Order 4.** Values (script): `e_A = 2.4555e-5, 1.5449e-6, 9.6714e-8, 6.0471e-9`; orders
   `3.9905, 3.9976, 3.9994`. For disclosure, the exact arc length of the parabola (closed form `sqrt(A + B) + A / sqrt(B)
   asinh(sqrt(B / A))`, `A = s^2`, `B = 4 (1 - c)^2`) gives `2.4476e-5 ... 6.0468e-9`: the Gauss rule's own error is about 0.3
   percent of the geometric one at `k = 1`.
5. **Margins.** The order limit 3 has a margin of about 1 on both (a3) and (a4) from the coarsest pair on (pre-asymptotic
   correction `(30 / 252) phi^2 = 0.0046` at `k = 1`). Rounding: `e_A(8) = 6.0e-9` and `e_V(8) = 1.2e-8` against sums of a few
   thousand terms with relative rounding near `1e-15`; the observed order at the finest pair moves by about `1e-6`.
6. **Radius after projection (a2).** As ADR 0413 arithmetic 5: `| |q_xy| - r | <= ~4 r eps = 1.8e-15` for `r = 2`; the limit
   `1e-14` has a margin above 5. The corners are the generator's `(r cos th, r sin th)`: radius within a few `eps r` (C7).
7. **Negative controls (no projection).** The wall faces are flat chord rectangles, the solid the chord prism of ADR 0413
   arithmetic 1: `e_V = 1 - sin(2 phi) / (2 phi) = 2.5505e-2, 6.4131e-3, 1.6056e-3, 4.0155e-4`, orders `1.9917, 1.9979,
   1.9995`; wall area `h n 2 r s`, `e_A = 1 - s / phi = 6.4131e-3, 1.6056e-3, 4.0155e-4, 1.0040e-4`, orders `1.9979, 1.9995,
   1.9999` (a flat parallelogram face with mid-edge nodes at the midpoints is integrated exactly, ADR 0383).
8. **Counts.** Level `k`: points `(k + 1)(4k + 1)(k + 1)`; edges `4k (k + 1)^2` circumferential, `(4k + 1) k (k + 1)` radial,
   `(4k + 1)(k + 1) k` axial; Hex20 nodes = points + edges (`k = 1`: 20 + 36 = 56); elements `4 k^3` (2048 at `k = 8`); projected
   nodes per wall `4k (k + 1) + (4k + 1) k` (13, 42, 148, 552), both walls 26, 84, 296, 1104; faces per wall set `4 k^2`. Only
   volumes and areas, no solve: well under a second per level.

## Design

### D1. The test

One new test file `crates/rcfd-fem/tests/sb_convert_hex20.rs`. Per level `k` and flag `project`:
`fv = annulus_3d_sector(4k, k, k, 1.0, 2.0, 1.0, pi / 2, Uniform)`, `h8 = from_mesh(&fv)`, `(q, mids) = upgrade_to_hex20(&h8)`,
`m = project_mid_edge_nodes(&q, &mids, f)` with `f` returning `onto_cylinder(x, [0; 3], [0, 0, 1], r)` for a node in node set
`inner` (`r = 1`) or `outer` (`r = 2`) of `q` when `project`, else `None`. Measured: the projected nodes and their radius error,
`V_k`, `A_k(inner)`, `A_k(outer)`, counts. No library code changes (see the house rules paragraph).

## Checks (limits fixed now)

| id | clause | check | limit | estimate behind it |
| --- | --- | --- | --- | --- |
| C1 | a1 | every level: conversion, upgrade and projection accepted (no element refused as inverted); one block of Hex20; elements `4 k^3`; nodes points + edges; projected nodes per wall as arithmetic 8 | exact | arithmetic 8 |
| C2 | a2 | every level: every projected node `| hypot(x, y) - r |` | `1e-14` | arithmetic 6: `~2e-15` |
| C3 | a3 | observed order of `e_V` against `3 pi / 4`, pairs (1,2), (2,4), (4,8) | each `>= 3` | arithmetic 3: `3.995, 3.999, 4.000` |
| C4 | a4 | observed order of the area error of `inner` against `pi a h / 2` and of `outer` against `pi b h / 2`, the three pairs each | each `>= 3` | arithmetic 4: `3.990, 3.998, 3.999` |
| C5 | (stronger, a3) | every level: `|V_k - V_p(k)| / V_p(k)` | `1e-12` | arithmetic 2, 3: rounding |
| C6 | (stronger, a4) | every level, both sets: `|A_k - A_p(r, k)| / A_p(r, k)`; faces per set `4 k^2` | `1e-12`; exact | arithmetic 4: rounding |
| C7 | (rest) | every level: every corner node of the walls `| hypot(x, y) - r |` | `1e-14` | arithmetic 6 |
| C8 | (rest) | every level: node set `inner` (`outer`) of the upgraded mesh equals the nodes of the surface set `inner` (`outer`) | exact | ADR 0292 D6 |

Negative controls:

| id | control | expected | limit |
| --- | --- | --- | --- |
| N1 | no projection: volume order, each pair | `1.992, 1.998, 2.000` (arithmetic 7) | each in `[1.9, 2.1]`, so C3 discriminates |
| N2 | no projection: wall area order, each pair, both sets | `1.998, 2.000, 2.000` | each in `[1.9, 2.1]`, so C4 discriminates |
| N3 | no projection: volume against the chord form `(h / 2)(b^2 - a^2) n sin(2 phi)` and areas against `h n 2 r sin(phi)` | rounding | `1e-12` relative |

## Planned mutations (on a scratch copy, one at a time)

| id | mutation | expected to be caught by |
| --- | --- | --- |
| M1 | `project_mid_edge_nodes` ignores the returned position (node left) | C2, C3, C4 |
| M2 | `upgrade_to_hex20_mapped` does not extend node sets with mid-edge nodes | C1 (projected count), C8 |
| M3 | `onto_cylinder` scales by `radius / norm(d)` (whole offset) instead of `radius / norm(radial)` | C2 (nodes with `z != 0`) |
| M4 | `hex20::volume` with the 2 x 2 x 2 Gauss rule instead of 3 x 3 x 3 | C5 (and possibly C3) |
| M5 | the 8-node face area by the centre point of the face rule only (weight 4) | C6, C4 (order 2) |
| M6 | `hex20::face_rule` with the 2 x 2 Gauss rule | C6 |

## Decision rule

- **done:** C1 to C4 (every clause of the acceptance) met, C5 to C8 and N1 to N3 as stated, every mutation caught by a registered
  check.
- **partial:** a clause of the acceptance missed (recorded as a `*_known_miss`) or not run, a mutation not caught, or a check met
  only after an amendment written after its run.
- **todo:** the route does not build.

## Limits (not covered)

- L1. The route through `upgrade_to_hex20_mapped` with a curved map is not checked: on a mesh already in physical coordinates a
  map would have to recognise wall chords by their radius. The mapped route belongs to meshes built in a parameter space (the
  bore of ADR 0385).
- L2. Only volume, wall area and radius are checked; stresses on curved Hex20 are later packages' (ADR 0280). Interior
  circumferential edges stay chords; whether projecting them as well (an isoparametric polar map) helps the stress is not asked.
- L3. Uniform radial division only; one angle (`pi / 2`) and one ratio `b / a = 2`.
