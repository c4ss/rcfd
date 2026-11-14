# ADR 0478: F-432 registration: stress recovery by element patches (superconvergent patch recovery) and an energy-norm error estimate with element indicators

Status: **REGISTERED 2026-11-14, written before any code of F-432 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-432 was written, compiled or run in the repository: the files listed under "What was
read" were read, and the arithmetic below was done by hand and with two plain numpy scripts in the scratch folder
(`w49i-work/f432-scratch`: `probe_1d.py`, `probe_1d_quad.py`; one-dimensional models written in numpy, no rcfd code; disclosed,
their numbers quoted below). The bit record D1 of the default paths is a test file written and run on the base after this commit
and before any library code (D8). Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-432 (deps F-408, F-376, F-378), the package SA-25 of the advanced tier (plan
[ADR 0310](0310-structural-solver-advanced-tier-plan.md), stub [ADR 0335](0335-stress-recovery-by-patches-and-error-estimate.md)).
Number 0478 given by the lead. Branch `f432-work` from local main 57808e24, worktree `w49i-work/f432`, scratch
`w49i-work/f432-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no
unsafe code, the crate graph unchanged; existing tests not edited, except that the X1 row this work retires is kept as an
ignored record citing ADR 0291 and continued in an `_after_f432` copy (the pattern of `sa_scaffold.rs`, named in the brief);
clean-room.

## What F-432 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. A linear displacement field gives the exact stress and a zero estimate within 1e-10. On the manufactured solution of F-378 for Hex8, Tet10 and Hex20: the effectivity index (estimate over true error in the energy norm) lies in [0.8, 1.2] on the three finest meshes and its distance from 1 does not grow under refinement; the recovered stress of Hex8 converges in L2 at an observed order >= 1.7 where the raw stress shows >= 0.9. Plate with a small circular hole under tension: the recovered peak stress is within 2 percent of 3 sigma on the mesh on which the averaged nodal stress of F-376 is recorded, and the tenth of the elements with the largest indicator contains the elements at the hole edge. Boundary and corner nodes take the patch of the nearest interior node by the registered rule; the result is bit-identical for 1 and 8 threads; a patch with too few points is reported, not solved.

The brief adds: (b1) reuse the meshes and helpers of F-376 (ADR 0277) and F-378 (ADR 0279); (b2) existing results keep their
bits and `baseline_hashes` stays unchanged, with a default-bits test recorded at the base before any library change; (b3) a
case-file key or an output-file field is the maintainer's call (recommended, not added); (b4) probe a quantity before a
`1e-12`-class limit, `1e-11` for sums of about `1e3` terms; (b5) the X1 row of `sa_scaffold.rs` (latest copy
`x1_modules_and_pending_after_f415_f420`) retired by an ignored record and an `_after_f432` copy, the edit kept minimal (F-416
and F-417 edit the same file); (b6) sources: Zienkiewicz and Zhu 1987 and 1992, Ainsworth and Oden, recorded in
`docs/reference-data.md`.

Clauses: (a0) every limit registered here; (a1) a linear displacement field: exact recovered stress and zero estimate within
`1e-10`; (a2) the effectivity index on the manufactured solution in `[0.8, 1.2]` on the three finest meshes, Hex8, Tet10 and
Hex20; (a3) its distance from 1 not growing under refinement; (a4) Hex8 recovered stress L2 order `>= 1.7`; (a5) Hex8 raw stress
L2 order `>= 0.9`; (a6) plate with a hole: recovered peak stress within 2 percent of `3 sigma_0`; (a7) the tenth of the elements
with the largest indicator contains the hole-edge elements; (a8) boundary and corner nodes by the registered rule (D3); (a9)
bit-identical for 1 and 8 threads; (a10) a patch with too few points reported, not solved.

Readings, registered:

- **(a1) "within 1e-10":** relative: `max_n |sigma*_n - sigma|/|sigma|` over every node (tensor norm of `recover::tensor_norm`)
  and `eta/||u||_E` with `||u||_E^2 = int sigma . D^{-1} sigma dV` of the exact field.
- **(a2) "the three finest meshes":** each element runs three meshes, each halving `h`, so the three finest are the three run:
  Hex8 `n = 4, 8, 16` cells per edge (F-378's sequence), Tet10 and Hex20 `n = 4, 8, 16` (one level finer than F-378's 2, 4, 8 for
  Tet10 and ADR 0292's 2, 4, 8 for Hex20; the reason is arithmetic 2: the one-dimensional quadratic probe has `|theta - 1|`
  growing from `n = 2` to `n = 4`, a pre-asymptotic effect of a one-patch mesh, which is not what (a3) asks about). Case D of
  F-378 (every face prescribed with the nodal values of the exact field), the body force of F-378 (`body_force_loads`), `nu = 0.3`,
  `E = 1`.
- **(a3) "does not grow":** `|theta_{k+1} - 1| <= |theta_k - 1|` for both successive pairs, with no tolerance.
- **(a4, a5) "converges in L2 at an observed order":** `log2(e_n/e_2n)` over the finest pair (8 to 16), `e = (int |sigma - s|^2
  dV)^{1/2}` with `|.|` the tensor norm, `s` the recovered field `sigma* = sum_a N_a sigma*_a` (a4) or the raw `D B u_h` (a5),
  integrated with the 125-point rules of F-378.
- **(a6) "the mesh on which the averaged nodal stress of F-376 is recorded":** the plate of ADR 0449 (F-413: quarter plate, `r =
  0.005`, full width `0.1`, `r/W = 0.05`, Hex20, the graded map `plate_map`, symmetry on the cut faces, `sigma_0 = 50` MPa on
  `x = 0.1`) with two layers of elements through the thickness instead of one (24 x 24 x 2). The single-layer mesh of ADR 0449
  has no interior corner node (every node lies on `z = 0` or `z = t`), so by the rule of (a8) it has no patch at all; it is the
  negative control S3. On the two-layer mesh both `recover::nodal_average` (F-376, recorded in P1) and the recovered stress are
  computed in the same test: the clause compares them on one mesh. "Recovered peak stress": the largest recovered `sigma_xx`
  over all nodes (Kirsch: the hoop stress at `(0, r)`, which is `sigma_xx` there), against `3 sigma_0`, relative. A static solve
  (`StaticSolver::SparseDirect`); ADR 0449 used the harmonic solve at 100 rad/s, far below the first frequency (about `7.9e4`
  rad/s).
- **(a7) "the tenth of the elements with the largest indicator":** the `ceil(N/10)` elements of largest `eta_e` (D4), ties to
  the lower element index; "the elements at the hole edge": every element with a face whose nodes all lie on the hole (`|sqrt(x^2
  + y^2) - r| <= 1e-9` m). On the two-layer plate `N = 1152`, the tenth 116 elements, the hole-edge elements 48.
- **(a9) "bit-identical for 1 and 8 threads":** `PatchOptions::threads(Some(1))` against `Some(8)`: every nodal value, every
  element indicator and the global estimate equal to the bit.
- **(a10) "too few points":** fewer sampling points than terms of the basis, or a rank-deficient fit (D2); "reported": listed
  in `Recovery::short_patches` with the node, the number of points and terms and the reason; "not solved": no fit is made and the
  node is not a source for any other node; when no patch at all can be solved the call is refused with the list in the message.

## What was read (no run)

- `AGENT_RULES.md`; the brief; `docs/roadmap.json` and `docs/ROADMAP.md` (F-432, F-376, F-378, F-408); ADR 0310 (SA-25, the
  file layout, `estimate::energy_error`), ADR 0335 (the stub).
- `crates/rcfd-fem/src`: `recover.rs` (the field, the extrapolation, `nodal_average`, `tensor_norm`), `estimate.rs` (the stub),
  `element.rs` (the trait, the rules of the kinds: Tet4 `tet_1`, Hex8 `gauss_2`, Tet10 `tet_4`, Hex20 `gauss_3`), `quadrature.rs`
  (`tet_1`, `tet_4`, `tet_14`, `gauss_1` to `gauss_3`), `material.rs` (`elasticity_matrix`), `mesh.rs` (`face_incidence`,
  `faces`, `corners`), `assemble.rs` (the thread pattern), `builders.rs` (`tet_box`, the Kuhn split), `hex20.rs` (the upgrade),
  `solve.rs` (`StaticSolver::SparseDirect`), `thermal.rs` (`temperature_field`).
- `crates/rcfd-fem/tests`: `sb_mms.rs` (the field of F-378, the error rules, case D), `si_hex20.rs` (the Hex20 manufactured
  solution, `hex20_box`, `gauss_125`), `sa_fatigue.rs` (the plate with a hole of ADR 0449: `plate_map`, `plate`, K1 with the
  nodal average at `+1.23` percent), `sa_cms_default_bits.rs` (the digest pattern), `sa_scaffold.rs` (X1).
- ADR 0449 (the plate, its estimate of the finite-width effect `+0.8` percent), `docs/reference-data.md` (the Kirsch row).

## Arithmetic done before the code (disclosed)

1. **One-dimensional probe, linear elements** (`probe_1d.py`): `u = sin(pi x + 0.3)` on `[0, 1]`, linear elements (nodally
   exact in 1-D), the raw stress the element mean, sampled at the centroid, a linear fit over the two elements of each interior
   node, end nodes from the nearest interior patch (the rule of D3). Effectivity `theta` at `n = 2, 4, 8, 16, 32`: `1.141, 1.080,
   1.025, 1.007, 1.002`; recovered L2 error `3.96e-1, 2.17e-1, 7.17e-2, 1.99e-2, 5.20e-3` (order 1.85 over 8 to 16), raw
   `4.99e-1, 2.51e-1, 1.26e-1` at 4, 8, 16 (order 1.00).
2. **One-dimensional probe, quadratic elements** (`probe_1d_quad.py`): the same field, quadratic elements (nodally exact), the
   raw stress sampled at the two Gauss points, a quadratic fit over the four points of each interior vertex's two elements, mid
   nodes from both end vertices (tie), end nodes from the nearest interior vertex. `theta` at `n = 2, 3, 4, 6, 8, 12, 16, 24`:
   `0.909, 0.771, 0.854, 0.941, 0.971, 0.990, 0.995, 0.998`. At `n = 2` the single patch spans the domain; from `n = 3` on
   `|theta - 1|` falls monotonically. Hence `n = 4, 8, 16` for Tet10 and Hex20 (reading a2), where the probe gives `0.854,
   0.971, 0.995`.
3. **Why the superconvergent points (D1).** For trilinear elements on a uniform mesh the gradient error at a point of an element
   is `-(x - x_c) u'' + O(h^2)` per direction (1-D: the derivative of the interpolant is exact to `O(h^2)` at the midpoint only;
   Barlow 1976, cited from memory, NOT read). At the four points of the 2-point Gauss rule of a two-element patch this sawtooth
   has a non-zero least-squares slope, `-u''/4` (by hand: points `+-h/2 +- h/(2 sqrt 3)`), so a fit to the stiffness-rule points
   is unbiased at the patch centre but has an `O(h)` error at a distance `h` from it: boundary nodes (D3) would carry `O(h)`
   errors on a layer of width `h`, an `O(h^1.5)` contribution to the L2 error, below the 1.7 of (a4). At the centroid the data
   are `O(h^2)` and the fit is `O(h^2)` over the whole patch. For the quadratic hexahedron the 2 x 2 x 2 Gauss points are the
   corresponding points (Barlow, from memory); for tetrahedra no such points are known to me (Zienkiewicz and Zhu 1992, from
   memory: superconvergence is not guaranteed on simplices), so Tet10 samples at the points of its stiffness rule.
4. **Counts.** Hex8 `n^3` elements, Hex20 `n^3`, Tet10 `6 n^3` (Kuhn split); interior corner nodes `(n - 1)^3` for all three (27,
   343, 3375 at 4, 8, 16). Free unknowns (case D): Hex8 `3 (n - 1)^3` (81, 1029, 10125); Tet10 `3 (2n - 1)^3` (1029, 10125,
   89373); Hex20 `3 ((n - 1)^3 + 3 n (n - 1)^2)` (405, 4557, 42525). Patch points: Hex8 8 (one per element), Hex20 64, Tet10 96
   (24 tetrahedra around an interior Kuhn vertex, four points each); terms 4, 10, 10. Two-layer plate: 1152 elements, `25 x 25 x
   3` corner nodes plus `3 x 2 x 24 x 25 + 2 x 25 x 25` mid-edge nodes, 6725 nodes; interior corner nodes `23 x 23 x 1 = 529`
   (the mid-plane, off the hole, the outer edges and both symmetry planes).
5. **Plate (P1).** ADR 0449 measured the nodal average at `(0, r)` at `+1.23` percent of `3 sigma_0` on the single layer, of
   which about `+0.8` percent is the finite width (Howland's factor, from memory, ADR 0449). With `nu = 0` the solution does not
   vary through the thickness, so the second layer changes nothing but the patches. The hole-edge node at `(0, r)` lies on the
   hole and on the symmetry plane `x = 0`; its nearest interior corner nodes are at the first ring, first angular step (distance
   about `0.072 r`). A quadratic fit there misses by the third derivative: radially `d^3 sigma_xx/drho^3 = -192 sigma_0/r^3` at
   the hole (Kirsch, by hand), over an extrapolation of `0.03 r`, about `0.2` percent of `3 sigma_0`. Estimate: `+0.8` to `+1.5`
   percent; limit (acceptance) 2 percent.
6. **Plate indicator (P2), a crude estimate that says this may miss.** The mesh is graded: radial size `0.0305 r` at the hole,
   growing by `e^{1/6} = 1.18` per ring, angular step `0.0654 rho`. With the Hex20 stress error `~ h^2 sigma''` and Kirsch's
   second derivatives (radial `~ 30 sigma_0 r^4/rho^6`, angular `~ 8 sigma_0/rho^2` at the hole), the element indicator `~ |K|^{1/2}
   h^2 sigma''` of the fifth ring comes out about 1.9 times that of the first: the grading that makes the hole-edge elements small
   also makes their indicator small. The estimate is too crude to decide; the check is run as registered and may miss.
7. **Tie and rank tolerances.** Distances of equidistant nodes computed in floating point differ by a few ulp: ties are distances
   within `1e-9` relative of the smallest (far below the spacing of distinct distances on any mesh here). The rank test of D2:
   `|R_kk| < 1e-10 max_j |R_jj|` in the scaled coordinates (columns of order 1); the single-layer plate's quadratic patch has
   two levels in `z`, so its `z^2` column is a combination of `1` and `z` to rounding (`|R_kk| ~ 1e-16`).
8. **(a1) at rounding.** A linear field gives an exactly constant raw stress per element up to rounding of `B u` (`1e-16`
   relative); the fit of constant data reproduces them to the conditioning of the scaled fit (`< 1e3` on these patches), so
   `1e-13` is expected against the stated `1e-10`.

## Design

### D1. Sampling points and bases (`estimate.rs`)

The raw stress `sigma_h = D B u_h` is sampled, per element, at the points of a sampling rule of its kind: Hex8 `gauss_1` (the
centroid), Hex20 `gauss_2` (2 x 2 x 2), Tet4 `tet_1` (the centroid), Tet10 `tet_4` (arithmetic 3). `D` is the element's
`Material::elasticity_matrix()` (isotropic, or orthotropic with fixed axes; other materials are refused through it). The basis of
a patch (Zienkiewicz and Zhu 1992, Part 1, the superconvergent patch recovery, cited from memory, NOT read; derived here as a
discrete least-squares fit): complete linear `P1 = {1, x, y, z}` for the linear kinds and complete quadratic `P2` (10 terms) for
the quadratic kinds, in the scaled coordinates `(x - x_c)/l` with `x_c` the centre node and `l` the largest distance of a patch
point from it. A patch with elements of both orders takes `P2`. `PatchOptions::basis` can set `P1` or `P2` for every patch
(used by S1, S2). The choice of `P1` for Hex8 instead of the trilinear polynomial of its shape functions is taken for the
stability of the fit on eight points (`P1` is the complete polynomial the element reproduces).

### D2. The patch and the fit

The centres are the **interior corner nodes**: nodes that are a corner of a solid element and lie on no boundary face (a face of
`SolidMesh::face_incidence` with one element; all nodes of such a face are boundary nodes). The patch of a centre is the set of
elements containing it, ascending, with their sampling points in rule order. The fit is a least-squares problem `P a = s` with
one row per point and the six stress components as six right-hand sides, solved by Householder QR with no pivoting (columns in
the fixed basis order). A patch is **short** when it has fewer points than terms ("too few points") or a diagonal entry of `R`
below `1e-10` of the largest ("rank deficient"); it is listed in `Recovery::short_patches` and not solved. A patch joining
elements of different materials is refused naming the node and the two material sets (a material interface: the stress is not
continuous there).

### D3. Nodal values: the registered rule for boundary and corner nodes

- A solved centre takes the value of its own fit at itself (the constant coefficient).
- Every other node of a solid element (a boundary node, a corner of the domain, a mid-edge node, a centre whose patch is short)
  takes the **mean of the fits of the nearest solved centres**, each evaluated at the node: nearest in Euclidean distance, every
  solved centre within `1e-9` relative of the smallest distance counted (ties), summed in ascending centre order and divided by
  their number. (Zienkiewicz and Zhu 1992, from memory: boundary values from the polynomial of an interior patch; this rule
  makes "the nearest interior node" exact.) The nearest centres are found by a uniform grid over the centres searched ring by
  ring until no cell can hold a nearer one: the result is that of a search over all centres (checked against it in B1).
- No solved centre at all: the call is refused (`InvalidValue`, key `"patch recovery"`), the message naming the number of
  interior corner nodes and each short patch.

The recovered field in an element is `sigma*(xi) = sum_a N_a(xi) sigma*_{node a}`.

### D4. The estimate (Zienkiewicz and Zhu 1987)

The element indicator `eta_e = (int_e (sigma* - sigma_h) . D^{-1} (sigma* - sigma_h) dV)^{1/2}`, the estimate `eta = (sum_e
eta_e^2)^{1/2}` (element order), the norm of the solution `||u_h||_E = (int sigma_h . D^{-1} sigma_h dV)^{1/2}` and the relative
estimate `eta/(||u_h||_E^2 + eta^2)^{1/2}` (Zienkiewicz and Zhu 1987, cited from memory, NOT read). `D^{-1}` by Gauss-Jordan
inversion of the element's `D` (the 6 x 6 of `recover`'s pattern). Integration: hexahedra `gauss_3`, tetrahedra `tet_14`
(degree 5 per variable and in total; the integrand is of degree at most 4 on affine elements). The effectivity index of the
checks is `eta/||u - u_h||_E` with the true error of F-378 (`errors` of `sb_mms.rs`).

### D5. Threads

`PatchOptions::threads` (`None`: the available parallelism) splits the patch fits and the element integrals into contiguous
ranges computed by `std::thread::scope` workers and concatenated in order; every value is computed by one worker from the same
inputs in the same order, so the result does not depend on the count (the pattern of `assemble.rs`). The nodal assignment of D3
is sequential.

### D6. The interface

`rcfd_fem::estimate`: `PENDING = None`; `PatchBasis { Linear, Quadratic }`; `PatchOptions { basis, threads }` with `new`,
`basis`, `threads`; `sampling_rule(kind)`, `estimate_rule(kind)`; `PatchFit` (`centre`, `elements`, `n_points`, `basis`,
`evaluate(x)`); `ShortPatch { node, points, terms, reason }`; `Recovery` (`nodal`, `sources(node)`, `patch(centre)`, `patches`,
`short_patches`, `stress_at(model, element, xi)`); `recover(model, displacement, options)`; `EnergyError` (`element_errors`,
`estimate`, `solution_norm`, `relative`, `ranked`); `energy_error(model, displacement, options) -> (Recovery, EnergyError)`.
Refused: a displacement of the wrong length (`FieldLength`), an element of a structural kind, a model with a temperature field,
a material without a constant elasticity matrix, a patch across a material interface. Nothing outside `estimate.rs` changes
(the module comment of `lib.rs` already names it).

### D7. Records retired

`sa_scaffold.rs`: `x1_modules_and_pending_after_f415_f420` becomes an ignored record citing ADR 0291 and ADR 0478 and is
continued in `x1_modules_and_pending_after_f432` with the `estimate` row `"landed"`. No other existing test is edited.

### D8. The bit record of the default (D1 check)

`crates/rcfd-fem/tests/sa_estimate_default_bits.rs`, written and run on the base (after this commit, before any library code):
FNV digests of the static displacement, the integration-point stress (`IntegrationPointField`) and `nodal_average` of the
two-layer plate and of the Hex8 `n = 4` manufactured solution. After the library change the digests must be equal.

## Checks (limits fixed now)

Tests in `crates/rcfd-fem/tests/sa_estimate.rs` (fast) with the heavy records `#[ignore]`d there (Tet10 and Hex20 at `n = 16`,
run once). Every value is printed. Linear solves: `StaticSolver::Direct` for the fast tests, `SparseDirect` for the heavy
records and the plate. Seed `0xF432_0478`.

| Id | Clause | Quantity | Limit | Estimate |
|---|---|---|---|---|
| L1 | a1 | 3 x 3 x 3 boxes of Hex8, Tet4, Tet10 (upgraded after the move) and Hex20 (upgraded after the move) on the unit cube, the 8 interior grid nodes moved by uniform `+-0.15 h` per coordinate; `u = a + G x` at every node (`Poly::linear` of `sb_mms.rs`): `max_n |sigma*_n - sigma|/|sigma|` and `eta/||u||_E` | `<= 1e-10` each | `1e-13` |
| L2 | supp. | quadratic field (`Poly::quadratic`) on Tet10 (the moved box of L1, straight edges) and Hex20 (the undistorted 3 x 3 x 3 box): the same two quantities, `|sigma|` the largest nodal tensor norm | `<= 1e-10` each | `1e-13` |
| M1 | a2 | case D, Hex8 `n = 4, 8, 16`, Tet10 `n = 4, 8, 16`, Hex20 `n = 4, 8, 16`: `theta = eta/||u - u_h||_E` | in `[0.8, 1.2]`, all nine | Hex8 `1.08, 1.03, 1.01`; Hex20 `0.85, 0.97, 0.99` (1-D); Tet10 none |
| M2 | a3 | the same: `|theta_{k+1} - 1| <= |theta_k - 1|`, both pairs, each element | holds (six) | holds (1-D) |
| M3 | a4 | Hex8 recovered `||sigma - sigma*||_L2`, order 8 to 16 | `>= 1.7` | 1.85 (1-D) |
| M4 | a5 | Hex8 raw `||sigma - sigma_h||_L2`, order 8 to 16 | `>= 0.9` | 1.00 |
| M5 | supp. | Tet10 and Hex20 recovered and raw L2 orders 8 to 16 | recorded, no limit | |
| F1 | supp. (fast path of M1 for Tet10, Hex20) | `theta` at `n = 2, 4` | in `(0.5, 2.0)` | `0.9, 0.85` (1-D) |
| P1 | a6 | two-layer plate: largest recovered `sigma_xx` over all nodes against `3 sigma_0`, relative; recorded beside it the largest `nodal_average` `sigma_xx` of F-376 on the same mesh and the node of each | `<= 2e-2` | `+0.8` to `+1.5` percent |
| P2 | a7 | the same plate: the 116 elements of largest `eta_e` (ties to the lower index) contain the 48 hole-edge elements | holds | may miss (arithmetic 6) |
| B1 | a8 | Tet10 `n = 4`, Hex20 `n = 4` (case D solutions) and the plate: for every node, its sources equal the nearest solved centres by a search over all centres (tolerance of D3), and its value equals, bit for bit, the mean of their `PatchFit::evaluate` in ascending order (a solved centre: its own fit at itself) | holds | holds |
| T1 | a9 | the three models of B1: `threads(Some(1))` against `threads(Some(8))`, every nodal value, every `eta_e` and `eta` | bit-equal | equal |
| S1 | a10 | `tet_box` 2 x 2 x 2 Tet4 with element 0 split into four by its centroid node `p` (the mesh stays conforming), `PatchBasis::Quadratic`: `p` is listed short ("too few points", 4 points, 10 terms), has no fit, and takes the fit of the nearest solved centre; the box centre (26 elements, 26 points) is solved | holds | holds |
| S2 | a10 | Hex8 3 x 3 x 3, `PatchBasis::Quadratic` (8 points for 10 terms at each of the 8 centres): refused, `InvalidValue` key `"patch recovery"`, the message naming the 8 short patches | holds | holds |
| S3 | a10 (neg.) | the single-layer plate of ADR 0449: refused (no interior corner node), key `"patch recovery"` | holds | holds |
| R1 | supp. | refusals: displacement of wrong length (`FieldLength`); a model with a Beam2 block; a model with a temperature field; two materials meeting in a patch (key `"patch recovery"`, the node and both set names in the message) | each | each |
| N1 | neg. | case D Hex8 `n = 16` with the lambda term of the body force reversed (F-378 N1): `theta` | `< 0.5` | the true error is `O(1)`, the estimate `O(h)` |
| D1 | b2 | every digest of `sa_estimate_default_bits.rs` equal to the base's | equal | equal |
| X1 | D7 | `estimate::PENDING` is `None` (the `_after_f432` copy) | holds | holds |

Negative controls are N1 and S3 (above).

## Planned mutations (on a scratch copy, one at a time)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | boundary nodes take their own patch when it can be solved (the rule of D3 replaced) | B1 |
| MU2 | Hex8 sampled at the points of its stiffness rule (`gauss_2`) | M3 (arithmetic 3; may survive if the boundary layer is weaker in 3-D) |
| MU3 | the indicator with `D` in place of `D^{-1}` | M1, L1 survives (zero) |
| MU4 | ties broken by the first centre instead of the mean | B1 |
| MU5 | the rank test removed (a rank-deficient patch solved) | expected to survive: no registered model has a patch with enough points that is rank deficient (S3 has no centre at all, S1 and S2 are short by count) |
| MU6 | the point count test removed (a patch with fewer points than terms passed to QR) | S1, S2 |
| MU7 | the worker ranges concatenated in reverse order | T1, L1 |
| MU8 | the estimate integrated with the sampling rule (`gauss_1` for Hex8) | M1 or M2 (may survive) |

## Decision rule

- **done:** every check above met (L1, L2, M1 to M4, F1, P1, P2, B1, T1, S1 to S3, R1, N1, D1, X1), the heavy records run once,
  the existing tests of `rcfd-fem` passing (with the record of D7), `baseline_hashes` passing, fmt and clippy clean.
- **partial:** a clause of the acceptance missed or not run, with the miss recorded as a `*_known_miss` test and diagnosed; or a
  supplementary check missed.
- **todo:** no recovery or no estimate produced on the manufactured solution.

## Limits (not covered)

- Solid elements only: beams and shells have no patch recovery (refused).
- No temperature field (the thermal strain would have to be subtracted at the sampling points); materials with a constant
  elasticity matrix only; no recovery across a material interface (refused, not split per material).
- The superconvergent points are those of uniform meshes; on distorted meshes and on tetrahedra the effectivity is measured, not
  guaranteed. No equilibrium terms in the fit (the plain recovery of Zienkiewicz and Zhu), no adaptation (ADR 0310).
- The nearest-centre rule is geometric: a thin wall with centres on the far side of a slot would take them (not met on the meshes
  here).
- No case-file key and no output field (Q1).

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

- **Q1. A case-file key and an output field.** Options: (a) none now; (b) a solid-case block `[estimate]` writing the recovered
  nodal stress and the element indicator. Recommendation (b) with the fields `recovered_stress` (nodal, Voigt) and
  `error_indicator` (per element) and the relative estimate in the summary; not added (the maintainer's call).
- **Q2. The plate mesh of (a6).** Options: (a) ADR 0449's single layer (no interior corner node: nothing to recover by the rule
  of (a8)); (b) the same plate with two layers, both stresses compared on it. Taken (b); (a) is the negative control S3.
- **Q3. The Tet10 and Hex20 levels.** Options: (a) F-378's 2, 4, 8; (b) 4, 8, 16 (heavy records). Taken (b) (arithmetic 2).

## Amendment 1 (2026-11-14, written after the registered runs, before any run of the copies)

The registered runs (commit 674356db: the fast suite once, log `f432-scratch/run_fast.log`; the two heavy records once, log
`f432-scratch/run_heavy.log`) missed four checks. No limit is changed; each missed test is kept as an ignored `*_known_miss`
record with its limit and the measured numbers, and the checks of the same test that were met are continued in copies:

- **F1** (supp.): `theta` at `n = 2` is `2.0405` (Tet10) and `2.7943` (Hex20) against `(0.5, 2.0)`; at `n = 4` `1.0909` and
  `1.3369` (met). `f1_tet10_hex20_fast` becomes `f1_tet10_hex20_fast_known_miss`; `f1_tet10_hex20_fast_n4` continues `n = 4`.
- **P2** (a7): 30 of the 48 hole-edge elements lie outside the 116 of largest indicator (their ranks 38 to 937; the largest
  indicator, `1.21e-5`, is element 551 of the outer ring at the free edge `y = B`; the largest at the hole `4.68e-6`, rank 116
  `3.41e-6`). As arithmetic 6 feared: the indicator is absolute, and the graded mesh makes the hole-edge elements small.
  `p1_p2_plate_with_a_hole` becomes `p1_p2_plate_with_a_hole_known_miss`; `p1_plate_with_a_hole` continues P1.
- **M1 Hex20** (a2): `theta = 1.3369` at `n = 4` against `[0.8, 1.2]`; `1.0278` and `0.9939` at 8 and 16 (met); M2 met. At `n = 4`
  the recovered L2 error of Hex20 (`1.05e-1`) is above the raw one (`9.80e-2`): each P2 patch spans half the cube, a pre-asymptotic
  mesh for a field with wavenumbers up to `pi` (the 1-D probe put the pre-asymptotic range below `n = 3`; in 3-D it reaches `n =
  4`, where boundary nodes are 26 of every 27 extrapolated). `m1_m2_m5_hex20_heavy` becomes `m1_m2_m5_hex20_heavy_known_miss`.
- **M2 Tet10** (a3): `|theta - 1|` is `0.0909, 0.00514, 0.00832` at 4, 8, 16: `theta = 1.0051` then `0.9917` crosses 1, so its
  distance from 1 grows by `3.2e-3` after falling by a factor 18. M1 met on all three meshes. `m1_m2_m5_tet10_heavy` becomes
  `m1_m2_m5_tet10_heavy_known_miss`.

`cargo fmt -p rcfd-fem` was applied after the runs (layout only).

## Addendum 1 (2026-11-14): results

Runs: the fast suite `cargo test -p rcfd-fem --release --test sa_estimate -- --nocapture --test-threads=1` once at commit
674356db (`f432-scratch/run_fast.log`, 10 s); the two heavy records once (`--ignored heavy`, `run_heavy.log`, 756 s: Hex20 233 s,
Tet10 at n = 16 519 s); the copies of amendment 1 and the whole `rcfd-fem` suite after it. D1 was recorded at the base (commit
1fed9f7e) before any library code.

| Id | Limit | Measured | Met |
|---|---|---|---|
| L1 Hex8, Tet4, Tet10, Hex20: nodal stress | `<= 1e-10` | `9.7e-16, 1.5e-15, 1.0e-14, 4.0e-15` | yes |
| L1 the same: estimate | `<= 1e-10` | `5.5e-16, 6.0e-16, 2.1e-15, 2.2e-15` | yes |
| L2 Tet10, Hex20 quadratic field: nodal stress; estimate | `<= 1e-10` | `3.4e-15, 1.2e-15`; `2.0e-15, 2.0e-15` | yes |
| M1 Hex8 `theta` at 4, 8, 16 | `[0.8, 1.2]` | `1.1804, 1.0725, 1.0246` | yes |
| M1 Tet10 at 4, 8, 16 | `[0.8, 1.2]` | `1.0909, 1.0051, 0.9917` | yes |
| M1 Hex20 at 4, 8, 16 | `[0.8, 1.2]` | `1.3369, 1.0278, 0.9939` | **no** (n = 4) |
| M2 Hex8 `abs(theta - 1)` | non-increasing | `0.180, 0.072, 0.025` | yes |
| M2 Tet10 | non-increasing | `0.0909, 0.00514, 0.00832` | **no** (8 to 16) |
| M2 Hex20 | non-increasing | `0.337, 0.0278, 0.0061` | yes |
| M3 Hex8 recovered L2 order 8 to 16 | `>= 1.7` | `1.8085` | yes |
| M4 Hex8 raw L2 order 8 to 16 | `>= 0.9` | `0.9986` | yes |
| M5 (record) recovered and raw L2 orders 8 to 16 | none | Tet10 `3.26` and `1.99`, Hex20 `3.52` and `2.00` | recorded |
| F1 `theta` at n = 2, 4 | `(0.5, 2.0)` | Tet10 `2.0405, 1.0909`; Hex20 `2.7943, 1.3369` | **no** (n = 2) |
| P1 recovered peak `sigma_xx` against `3 sigma_0` | `<= 2e-2` | `+1.4085e-2` at `(0, r, t)`; nodal average of F-376 on the same mesh `+1.2340e-2` | yes |
| P2 hole-edge elements in the tenth | all 48 | 18 of 48 (ranks 38 to 937) | **no** |
| B1 rule, bit for bit (Tet10 n = 4, Hex20 n = 4, plate) | holds | 0 wrong sources, 0 wrong values (386, 150, 0 tied nodes) | yes |
| T1 1 and 8 threads (same three) | bit-equal | equal | yes |
| S1 split node short, centre solved (26 points), source the centre | holds | holds | yes |
| S2 Hex8 quadratic basis refused naming 8 short patches | holds | holds | yes |
| S3 single-layer plate refused | holds | holds (0 interior corner nodes) | yes |
| R1 four refusals | each | each | yes |
| N1 lambda reversed, Hex8 n = 16: `theta` | `< 0.5` | `0.0771` | yes |
| D1 six digests | equal | equal | yes |
| X1 `estimate::PENDING` None | holds | holds | yes |

Remarks. The Hex8 numbers follow the one-dimensional probe closely (`1.08, 1.025, 1.007` there); the recovered stress of the
quadratic kinds converges at order 3.3 to 3.5 and the raw stress at 2.0, so the estimate is asymptotically exact on all three
kinds and `theta` is within 0.9 percent of 1 at `n = 16`. The two M misses sit at the ends of the sequence: Hex20 on its coarsest
mesh (each patch spans half the cube) and Tet10 where `theta` passes through 1 between 8 and 16, so "does not grow" compares
two distances of `5e-3` and `8e-3` on opposite sides of 1. P1: the recovered peak is 0.17 percent above the nodal average, and
about 0.8 percent of either is the finite width (ADR 0449); the hole-edge patch extrapolates from the first ring. P2 is the
graded mesh (arithmetic 6): the hole-edge elements near 90 degrees rank 38 to 65, those near 45 and 0 degrees below 800, and the
largest indicators sit on the coarse outer ring. An indicator per unit volume (`eta_e/|K|^{1/2}`) would weigh the small
hole-edge elements up; it is a different indicator from the registered one and is proposed as a follow-up, not substituted.

Mutations (scratch copy `f432-scratch/mut` at 674356db, the fast suite per mutation, `mutate.py`, logs `mut_MU*.log`; each
compared against the baseline misses F1 at n = 2 and P2):

| Id | Caught by |
|---|---|
| MU1 boundary corner nodes as centres | P1 (`1.5e5`), R1 (another interface node is named), S1, S2; not B1, which checks the rule against the centres returned and so moves with them (the registration expected B1) |
| MU2 Hex8 sampled at `gauss_2` | S2 (8 points per element: no longer short) and the rule table; M3 survives (`1.8387`): in 3-D the boundary layer does not lower the order on these meshes, so arithmetic 3 overstated it |
| MU3 `D` for `D^{-1}` | M1 Hex8 (`2.25, 2.06, 1.97`), F1 |
| MU4 ties to the first centre | B1 (386 and 150 wrong sources) |
| MU5 rank test removed | survived, as registered |
| MU6 point count test removed | S1, S2 (reported "rank deficient" instead of "too few points") |
| MU7 workers joined in reverse | T1, F1, M1 |
| MU8 estimate integrated at `gauss_1` (Hex8) | M1 (`0.58, 0.45, 0.27`), M2 |

After amendment 1: the copies `f1_tet10_hex20_fast_n4` and `p1_plate_with_a_hole` and the rest of `sa_estimate`, the whole
`rcfd-fem` suite, `baseline_hashes` and the docs site test were run; fmt and clippy (`-D warnings`, all targets) on `rcfd-fem`
are clean. Not run: the whole workspace (the lead's gate). No shared solver file was changed (only `estimate.rs`, the stub of
ADR 0440).

**Decision by the rule: partial.** Clauses a2 (Hex20 at n = 4), a3 (Tet10 from 8 to 16) and a7 missed, each diagnosed and kept
as a known-miss record; a1, a4, a5, a6, a8, a9 and a10 met; the supplementary F1 missed at n = 2.

Remaining questions (open):

- **Q4. The indicator of a7.** Options: (a) keep the absolute `eta_e` of Zienkiewicz and Zhu (the clause stays missed on a
  graded mesh); (b) add the density `eta_e/|K|^{1/2}` as a second ranking and register a7 on it in a follow-up; (c) reword a7
  for a uniform mesh. Recommendation (b).
- **Q5. M2 near `theta = 1`.** Options: (a) keep "does not grow" literally (it fails whenever `theta` crosses 1); (b) read it
  as `abs(theta_next - 1) <= max(abs(theta_prev - 1), 1e-2)` in a follow-up. Recommendation (b), for the maintainer.
- **Q1** (case-file block and output fields): as registered, recommended and not added.

## For the lead: roadmap entries

Proposed status of F-432: **partial**. Note:

> Structural solver, advanced tier (plan ADR 0310, 2026-11-12). Size S. ADR 0478, 2026-11-14: rcfd_fem::estimate (stub of ADR 0440 filled, PENDING None): superconvergent patch recovery (sampling at the centroid of Hex8 and Tet4, the 2 x 2 x 2 points of Hex20 and the four points of Tet10; a complete linear or quadratic fit around every interior corner node by Householder QR; every other node takes the mean of the fits of the nearest solved centres, ties within 1e-9), the energy-norm estimate of Zienkiewicz and Zhu with element indicators, a ranking and the relative estimate; short patches (too few points or rank deficient) reported, not solved; structural kinds, a temperature field and a material interface in a patch refused; bit-identical for 1 and 8 threads. A linear field is exact within 1e-14 (limit 1e-10). Manufactured solution of F-378, case D, n = 4, 8, 16: effectivity Hex8 1.180, 1.073, 1.025; Tet10 1.091, 1.005, 0.992; Hex20 1.337 (MISSED at n = 4), 1.028, 0.994; distance from 1 non-increasing except Tet10 from 8 to 16 (0.0051 to 0.0083 as theta crosses 1: MISSED); Hex8 recovered L2 order 1.81 (limit 1.7), raw 1.00 (limit 0.9). Plate with a hole (the ADR 0449 plate with two layers): recovered peak +1.41 percent of 3 sigma (limit 2), nodal average of F-376 +1.23 percent on the same mesh; the tenth of largest indicator holds 18 of the 48 hole-edge elements (MISSED: the graded mesh makes them small). Seven of eight mutations caught (removing the rank test survives, as registered). Partial by the registered rule; the misses are kept as known-miss records with their limits.

Follow-up (title, deps, acceptance):

> (new item, number for the lead) Error indicator per unit volume and the effectivity criterion near one (deps F-432). Every limit not stated here is registered in the ADR before the run. EnergyError gives the density eta_e/|K|^{1/2} beside eta_e, the absolute values keeping their bits; on the two-layer plate with a hole of ADR 0478 the tenth of the elements with the largest density contains every element with a face on the hole; on the manufactured solution of F-378 (Hex20 and Tet10 at n = 8, 16 and a finer heavy record where feasible) abs(theta - 1) on each mesh is at most max(abs(theta - 1) on the previous mesh, 1e-2); the results of F-432 keep their bits.
