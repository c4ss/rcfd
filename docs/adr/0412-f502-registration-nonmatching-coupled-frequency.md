# ADR 0412: F-502 registration: the coupled frequency of the dynamic solid on non-matching interfaces, against face refinement and map, predicted from the static compliance of the mapped weights

Status: **REGISTERED 2026-11-13, written before any code of F-502 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-502 was written, compiled or run in Rust; the documents and code under "What was read"
were read, and the arithmetic under "Arithmetic before this registration" was done with one Python script (numpy). Results go
in the addendum at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0410](0410-f400-registration-dynamic-solid-participant.md) (F-400: `DynamicSolid`, the bar, the piston mock of its
amendment 1, the miss M2c: the first coupled frequency changes by `6.279e-3` between the matching and the 3:2 map at mass ratio
4; its questions 1 and 2, recommendation (a) each, from which this item comes) and
[ADR 0263](0263-f362-registration-interface-mapping.md) (F-362: `interface_rows`, the projection with its offset correction and
the radial basis of support `3 h`). Roadmap item F-502 (deps F-400, F-362), size S. Number 0412, branch
`f502-cosim-nonmatching-frequency`, worktree `w49i-work/f502`, scratch `w49i-work/f502-scratch`, given by the lead.

House rules (`w49i-work/AGENT_RULES.md`, the campaign rules of 2026-11-12, applied on 2026-11-13): limits fixed here and not
changed after a run; a missed limit becomes an ignored `*_known_miss` test with the limit intact; a defective set-up is
corrected only by an amendment committed before the corrected run; each registered run is made once; existing tests and records
are not edited; defaults bit-identical; `baseline_hashes.rs`, the hash tables and the goldens are not touched; no new crate, no
unsafe code; unsigned commits without trailers; no push.

Clean-room. Nothing outside this repository was opened. Everything used is derived below from the discrete equations (the
rank-1 added mass, the decomposition of the weights, the series compliance) or is the Newmark recurrence already implemented and
cited in ADR 0389/0410.

## What F-502 asks

The acceptance of the roadmap entry, binding: "Every limit not stated here is registered in the ADR before the run. On the bar of
F-400 (ADR 0410) with the piston mock at mass ratio 4, the change of the first coupled frequency between the matching and the
3:2 interface is measured for solid end faces of 2 x 2, 4 x 4 and 8 x 8 elements with the projection map and with the
radial-basis map; each change equals the one predicted from the static compliance w^T K^{-1} w of the mapped weights (computed
before the run) within 10 percent, and the face refinement or the map for which the change is below 1e-3 is reported; the
projection's weights on the 3:2 wall are checked for the square's mirror symmetries (ADR 0410 Q2) and an asymmetry is explained
or removed with defaults bit-identical."

Clauses: (c1) the change measured for `n = 2, 4, 8` with the projection and the radial basis (six changes); (c2) each equals
the prediction from the static compliance of the mapped weights within 10 %, the prediction computed before the coupled run;
(c3) the refinements or maps with a change below `1e-3` are reported; (c4) the projection's weights on the 3:2 wall checked for
the eight symmetries of the square; (c5) an asymmetry explained or removed, defaults bit-identical.

## What was read (no Rust run)

ADR 0410 (whole, with its amendments and addendum); ADR 0263 (the design of the projection and of the radial basis, the support
`3 h`); `crates/rcfd-fv/src/cosim/mapping.rs` (whole); `crates/rcfd-fv/src/cosim.rs` (`InterfaceMap`); `crates/rcfd-post/src/
solid_dynamic.rs` and `solid_coupling.rs` (whole); `crates/rcfd-post/tests/si_solid_dynamic.rs` (the set-ups, the piston mock,
the 1D chain, the run and crossing code); `rcfd_fem::mesh` (`HEX_FACES`, `face_corners`); `rcfd_fem::solve::StaticOptions`.

## Arithmetic before this registration (disclosed)

One script, `w49i-work/f502-scratch/prearith.py` (numpy, dense), which rebuilds the bar of ADR 0410 D4 (Hex8, `20 x n x n`,
`nu = 0`, clamped on `x = 0`) for `n = 2, 4, 8`, the weights `w` of three maps on the 3:2 wall in Python (the projection as the
library does it on a flat wall: the barycentric coordinates on the triangles `(0, 1, 2)`, `(0, 2, 3)` of each solid face in the
corner order of `HEX_FACES[3] = [1, 2, 6, 5]`; the bilinear shape functions of the face; the radial basis of support `3 h` with a
linear polynomial), their static compliance, the prediction of D3 and two checks of the prediction: the first eigenvalue of the
semi-discrete pencil `(K, M + m_a w w^T)` by shift-inverted iteration, and Newmark's method on that pencil run as the coupled run
will be (320 steps, `dt = T/100`, the same initial state and crossing count). Printed (ratio 4, `k L = 0.480094436957`):

| n | map | `Dc E A/L` | predicted change | semi-discrete | simulated run | sim./pred. - 1 |
|---|---|---|---|---|---|---|
| 2 | projection | `1.4891e-2` | `-6.2644e-3` | `-6.2691e-3` | `-6.2792e-3` | `2.4e-3` |
| 2 | bilinear | `5.1222e-3` | `-2.1667e-3` | `-2.1681e-3` | `-2.1681e-3` | `6.6e-4` |
| 2 | radial basis | `4.5056e-3` | `-1.9065e-3` | `-1.9077e-3` | `-1.9061e-3` | `2.2e-4` |
| 4 | projection | `4.2334e-3` | `-1.7916e-3` | `-1.7920e-3` | `-1.7912e-3` | `2.3e-4` |
| 4 | bilinear | `1.9306e-3` | `-8.1811e-4` | `-8.1826e-4` | `-8.1801e-4` | `1.1e-4` |
| 4 | radial basis | `5.9389e-4` | `-2.5185e-4` | `-2.5192e-4` | `-2.5219e-4` | `1.4e-3` |
| 8 | projection | `1.2637e-3` | `-5.3567e-4` | `-5.3571e-4` | `-5.3593e-4` | `4.8e-4` |
| 8 | bilinear | `6.6553e-4` | `-2.8222e-4` | `-2.8223e-4` | `-2.8239e-4` | `6.1e-4` |
| 8 | radial basis | `6.7088e-5` | `-2.8458e-5` | `-2.8460e-5` | `-2.8438e-5` | `7.0e-4` |

The consistent (matching) weights give `w0^T K^{-1} w0 = L/(E A)` to 12 digits for every `n`; the matching simulated run is
`1.46e-7` below `Omega_pred` for every `n`. The simulated projection run at `n = 2` reproduces the library's measured M2c of ADR
0410 (`6.279e-3`) to the printed digits, so the Python projection is the library's on this wall. Projection weights at `n = 2`:
corners `0.10185` at `(y, z) = (0, 0)` and `(W, W)`, `0.06481` at the other two; the bilinear weights `0.07716` at all four.
Symmetry defects of the projection weights (largest `|w(g x) - w(x)|/max w`): `0` to `6e-16` under the mirrors `y <-> z`, `(y, z)
-> (W - z, W - y)` and (hence) the half turn; `0.17` (`n = 2`), `0.11` (`n = 4, 8`) under the mirrors `y -> W - y`, `z -> W - z`
(and hence the quarter turns). Bilinear and radial basis: below `1e-14` under all. Nothing else was computed. The tests compute
their own predictions; no number above is pasted into a test.

## Design

### D1. The coupled system is Newmark's method on a rank-1 added mass

ADR 0410 amendment 1 and its addendum: with the piston mock the converged coupled step is Newmark's step of
`(M + m_a w w^T) u'' + K u = 0`, `w = B^T (A_f/A)_f e_x` the transpose of the map applied to the area fractions of the wall faces
(axial components only; `B` maps nodal displacements to the face means of the mapped points). Its first frequency
`omega_w` solves, with `phi = lambda m_a (w^T phi) (K - lambda M)^{-1} w`,

`1 = lambda m_a G_w(lambda)`, `G_w(lambda) = w^T (K - lambda M)^{-1} w`, `lambda = omega_w^2`,

and the measured discrete frequency is `Omega = (2/dt) atan(omega_w dt/2)` (average acceleration, undamped; ADR 0410 D6).

### D2. Splitting the weights: the cross term vanishes exactly

Write `w = w0 + d`, `w0` the consistent axial pattern of the face (`int N_i dA / A`: what the matching map gives, ADR 0410 D5).
Both sum to 1 (each map conserves the resultant), so `sum_i d_i = 0`. By ADR 0410 D5 the uniform axial field is invariant under
`K` and `M` (`nu = 0`, consistent mass without component coupling) and `w0` is its load, so `(K - lambda M)^{-1} w0` is a uniform
axial field: its axial value at the face nodes is one number `g(lambda)`. Hence `w0^T (K - lambda M)^{-1} d = g(lambda) sum_i d_i
= 0` for every `lambda`, and

`G_w(lambda) = G_0(lambda) + D(lambda)`, `D(lambda) = d^T (K - lambda M)^{-1} d`,

with `G_0` the tip compliance of the 1D chain of ADR 0410 D5 (exact). At `lambda = 0` this gives the static compliance
`w^T K^{-1} w = L/(E A) + D(0)`, so `Dc := w^T K^{-1} w - w0^T K^{-1} w0 = D(0)` exactly.

### D3. The prediction from the static compliance

`d` is self-equilibrated in the axial resultant (and, with the symmetries of D5, in the moments); its field decays within about
one face width `W` from the face (Saint-Venant), so at the first coupled frequency (`k W = 0.12`) it answers quasi-statically:
`D(lambda) = D(0) + lambda d^T K^{-1} M K^{-1} d + ...`, the relative correction of order `(k W)^2`, about `1e-2` of `Dc` (the
arithmetic gives the prediction within `5e-3` of the semi-discrete eigenvalue's change). The prediction replaces `D(lambda)` by
`Dc`: `omega_pred,w` solves `1 = lambda m_a (G_0(lambda) + Dc)`, i.e. is the first frequency of the 1D chain of ADR 0410 D7 whose
tip mass `m_a` hangs on a spring of compliance `Dc` in series (one more unknown: the mass, joined to the tip by `1/Dc`).
Predicted change: `Delta_pred = Omega(omega_pred,w)/Omega(omega_pred,0) - 1`, `Omega(x) = (2/dt) atan(x dt/2)`, `omega_pred,0`
the matching prediction of ADR 0410 D7 (the tip mass on the tip). Measured change: `Delta_m = Omega_m,3:2/Omega_m,matching - 1`
(signed). The test computes `Dc` by two static solves (D5) before the coupled runs and prints the prediction first.

### D4. The set-ups (those of ADR 0410 D4 and amendment 1; only `n` and the map vary)

Bar `hex_box([0,0,0], [L, W, W], [20, n, n])`, `L = 4`, `W = 1`, `E = 2e11`, `nu = 0`, `rho = 7850`, clamped on `xmin`, interface
`xmax`, `n = 2, 4, 8` (`h = W/n`). Ratio 4: `m_a = 4 rho A L`, `k L` the first root of `k L tan(k L) = 1/4`, `dt = T/100`, 320
coupled steps, `sub_steps = 1`, average-acceleration Newmark, the initial state of ADR 0410 D4 (`v0 = V sin(k x) e_x`, the mock
at the mapped velocity). The piston mock of ADR 0410 amendment 1 (copied into the new test file; test helpers are not shared
between test crates). Settings `CouplingSettings::aitken(0.5)`, tolerance `1e-8`, caps 4 (matching) and 20 (3:2), `AtCap::Stop`.
Walls: matching `n x n` faces with its points listed in the permuted order of ADR 0410's `wall(n, true)`; 3:2 `m x m`, `m = 3n/2`
(3, 6, 12) faces in the plain order. Maps (F-362, unchanged): projection `MapSettings::projection(1e-9)`; radial basis
`MapSettings::radial_basis(3 h, 1e-9)` (the support of ADR 0263, `3 h` with `h` the solid face's element size). A third map, a
companion and not a library map, is the bilinear evaluation `Q` of D6, built in the test with `InterfaceMap::new`.
Frequency: as ADR 0410 D4 (the zero crossings of the mean axial displacement of the interface nodes).

### D5. The static compliance

For each map: face forces `(A_f/A) e_x` (unit total), nodal forces `f = map.forces_to_nodes(...)` (so `f = w` in axial
components), `rcfd_post::solid_coupling::solve_face_forces` with the direct solver (default `StaticOptions`), and `c_w = sum_i
f_i . u_i` over the interface nodes. `Dc = c_w - c_w0`, `c_w0` from the matching map on the same `n`.

### D6. The symmetry of the projection's weights, and its cause

The eight symmetries of the square act on the face nodes; the bar, the clamp, both walls and the radial basis are invariant under
all eight, so a map that only depends on the geometry gives weights invariant under all eight. The projection is not only
geometric: `nearest_on_element` splits each quadrilateral interface element into the triangles `(0, 1, 2)` and `(0, 2, 3)` of its
corner order and takes barycentric coordinates there (ADR 0263, "quadrilaterals split into two triangles"). On a Hex8 `xmax` face
the corners are `HEX_FACES[3] = [1, 2, 6, 5]`, i.e. `(y, z)` low-low, high-low, high-high, low-high: every element of the face is
split along its diagonal from `(low, low)` to `(high, high)`, all diagonals parallel. The piecewise-linear interpolant on that
triangulation is invariant only under the symmetries that map the set of diagonals to itself: the identity, the mirror `y <-> z`,
the mirror `(y, z) -> (W - z, W - y)` and the half turn (a subgroup of order 4). Under the axis mirrors and the quarter turns the
diagonals go to the other diagonals, and the weights change (the hat function of a corner on a diagonal covers both triangles of
its element, `A_e/3`, a corner off the diagonal one triangle, `A_e/6`, against `A_e/4` for the bilinear function). On a flat wall
the offset correction adds nothing (the barycentric image is the point to rounding). Explanation registered as checks S1 to S3:
the library's projection weights equal the test's own triangle-split interpolation of the elements' corner order (`S1`), are
invariant under the subgroup (`S2`) and not under the other four (`S3`), and the bilinear evaluation `Q` (the shape functions of
the face, `N = (1 - s)(1 - t), s (1 - t), s t, (1 - s) t` on each element, located by the element's own corner coordinates) is
invariant under all eight (`S4`). The asymmetry is a property of the split, not an error of the code; removing it would change the
projection's weights, so it is not removed here (defaults bit-identical: nothing in the library changes) and its removal as an
opt-in is a question below.

### D7. What changes in the code

Nothing in a library. One new test file `crates/rcfd-post/tests/si_solid_dynamic_nonmatching.rs`. No shared solver file is
changed, so the baseline hashes are not affected (not run: no line they cover changes; stated in the addendum).

## Registered checks

Every value is printed; a test fails at the end if any of its checks missed. `n` runs over 2, 4, 8.

### A. The acceptance

| Id | Clause | Quantity | Limit | Estimate |
|---|---|---|---|---|
| C-P-n | c1, c2 | projection: `abs(Delta_m/Delta_pred - 1)`, `n = 2, 4, 8` (three checks) | `<= 0.1` each | `2.4e-3`, `2.3e-4`, `4.8e-4` |
| C-R-n | c1, c2 | radial basis, the same (three checks) | `<= 0.1` each | `2.2e-4`, `1.4e-3`, `7.0e-4` |
| C0 | c2 | `Dc` and `Delta_pred` of every map and `n` computed and printed before the first coupled run of that `n` (by the order of the test, stated in the addendum) | holds | holds |
| B1 | c3 | the list of `(n, map)` with `abs(Delta_m) < 1e-3`, printed, for the six of A and the three of `Q` | printed | projection 8; radial basis 4, 8; `Q` 4, 8 |
| B2 | c3 | the list of B1 equals the list with `abs(Delta_pred) < 1e-3` (the prediction tells which refinement or map reaches `1e-3`) | equal | equal (nearest margin: projection 4, `1.79e-3`) |
| S1 | c4, c5 | projection weights `w` on the 3:2 wall equal the test's triangle-split weights (D6): `max abs(w - w_tri)/max w` | `<= 1e-13` each `n` | `1e-16` |
| S2 | c4, c5 | projection: symmetry defect `max abs(w(g x) - w(x))/max w` under the mirror `y <-> z`, the mirror `(y, z) -> (W - z, W - y)` and the half turn | `<= 1e-13` each | `6e-16` |
| S3 | c4, c5 | projection: the same under the mirrors `y -> W - y`, `z -> W - z` and the two quarter turns | `>= 0.05` each (the asymmetry is present) | `0.17`, `0.11`, `0.11` |
| S4 | c5 | bilinear `Q`: defect under all seven non-trivial symmetries | `<= 1e-13` each | `5e-16` |
| S5 | c4 | radial basis: the same | `<= 1e-12` each | `1e-14` |

### C. Companions

| Id | Quantity | Limit | Estimate |
|---|---|---|---|
| C-Q-n | bilinear `Q`: `abs(Delta_m/Delta_pred - 1)` | `<= 0.1` each | `6.6e-4`, `1.1e-4`, `6.1e-4` |
| FM | matching runs: `abs(Omega_m/Omega_pred,0 - 1)` each `n` | `<= 2e-5` | `1.5e-7` |
| FU | matching runs: `abs(Omega_m(n)/Omega_m(2) - 1)`, `n = 4, 8` (the uniform field is exact for every `n`) | `<= 1e-9` | rounding, `1e-13` |
| K0 | `abs(c_w0 E A/L - 1)` each `n` (the static solve and D2's `G_0(0)`) | `<= 1e-9` | `1e-13` |
| I | every one of the 320 steps of every run converged to `1e-8`; iterations at most the cap (4 matching, 20 for 3:2) | caps | 3 |
| MB | resultant, moment, work of the last step of every 3:2 run (ADR 0410's `balance`), relative | `<= 1e-12` each | `1e-16` |
| PD | map reports: `precision_defect` of the projection and `Q` (computed in the test) `<= 1e-14`, the radial basis `<= 1e-12` | limits | `1e-16` |

### N. Negative control

* N1: the cruder mass-on-spring estimate `Delta_crude = -Dc E A/(2 L)` (the change of `1/sqrt(m_a (L/(E A) + Dc))`, the bar's
  mass and the dynamic part of `G_0` ignored) must miss the 10 % limit for every one of the nine runs: `abs(Delta_m/Delta_crude -
  1) >= 0.1`. Estimate `0.16` each (the factor between the two predictions is set by the bar, not by `d`). It shows that the 10 %
  limit can tell a right prediction from a nearly right one.

### S. Existing tests (unedited, release)

* S6: `cargo test -p rcfd-post --release --test si_solid_dynamic` (the neighbour; no line of it changes).
* `cargo test -p rcfd-fv --release --test cosim_f362` is not needed (no line of `rcfd-fv` changes) and is not run.

### M. Planned mutations (each alone, on a `git archive` copy of the test commit in the scratch folder, after the registered run)

* MU1: `nearest_on_element` splits quadrilaterals along `(1, 3)` (triangles `(0, 1, 3)`, `(1, 2, 3)`): expected caught by S1
  (S2 and S3 keep their values: the subgroup is the same).
* MU2: `InterfaceMap::forces_to_nodes` returns `1.01` times the forces: expected caught by MB (the resultant).
* MU3: the test's prediction with the series spring to ground instead of in series (the extra unknown dropped, `1/Dc` added to
  the tip's diagonal): expected caught by C-P-n, C-R-n.
* MU4: the test's `Dc` computed with `w0` in place of `w` (so `Dc = 0`): expected caught by C-P-n, C-R-n (a division by a zero
  prediction gives an infinite ratio).
* MU5: the test's triangle split of S1 taken along `(1, 3)`: expected caught by S1.

## Decision rule

* **done**: every check of A met, the companions of C met, N1 misses for all nine, S6 passes.
* **partial**: any check of A missed (recorded as `*_known_miss`, limit intact) or not run; or a companion missed with a
  diagnosed cause; or N1 meets the limit for some run (the 10 % limit cannot then tell the predictions apart).
* **todo**: no coupled run possible.

## Limits (what is not covered)

* Mass ratio 4 only (the acceptance's); one aspect ratio of the bar; flat square faces; the 3:2 ratio only.
* The piston mock (rank-1 added mass); a distributed added mass (ADR 0410 question 3) is not covered.
* The asymmetry is explained, not removed: no library map changes. The bilinear evaluation `Q` exists only in the test.
* The radial basis only at the support `3 h`.
* No flow solver.

## Open points taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. Explain or remove the asymmetry (c5): explained (D6), with the bilinear evaluation `Q` measured as a companion; a library
   opt-in (a bilinear projection on quadrilaterals) is left as a question for the addendum, because the acceptance allows either
   and the explanation changes no library code.
2. The update of the coupled iteration: Aitken only (the acceptance names none; ADR 0410 showed Aitken and least squares give the
   same frequencies to the printed digits).
