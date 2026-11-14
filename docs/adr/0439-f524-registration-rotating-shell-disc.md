# ADR 0439: F-524 registration: the rotating plane-stress disc of Shell4 facets against the closed form

Status: **REGISTERED 2026-11-14, written before any code of F-524 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-524 was written, compiled or run in rcfd: the files listed under "What was read" were
read, the closed form below was derived by hand, and the discretisation estimate was computed by an independent prototype
(`w49i-work/f524-scratch/estimate.py`, logs `estimate.log` and `precise.log`; plain Python with numpy, no rcfd code, written from
the formulas of ADR 0432 and ADR 0435; disclosed, its numbers quoted below). Results go in the addendum at the end; nothing above
the addendum is changed after a run.

Parents: [ADR 0435](0435-f522-registration-shells-on-the-transformation-and-body-loads.md) (F-522: the centrifugal load on Shell4,
`loads::nodal_load_vector` through `shell::body_load_vector`, `t int N_i rho omega^2 r_perp dA` by 2 x 2 Gauss at the projected
facet; its limit L5 and remaining question 1 option (a) name this item),
[ADR 0432](0432-f392-registration-flat-facet-shell.md) (F-392: Shell4, the bilinear membrane by 2 x 2 Gauss, the drilling penalty
`alpha G t int (theta_z - omega)^2 dA` with `alpha = shell::DRILLING_FACTOR = 1e-3`, `shell::stresses` at the element centre),
[ADR 0433](0433-f379-registration-closed-form-cantilever-thick-cylinder.md) (F-379: Lame's cylinder on a polar map, the measures
`u_r = (u_x x + u_y y)/r` at nodes and the hoop stress, the observed order between levels). Roadmap item F-524 (depends on
F-522). Size S. Branch `f524-rotating-shell-disc` from main d3d05478, worktree `w49i-work/f524`, scratch `w49i-work/f524-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact); each registered run made once; defaults bit-identical (this item adds one test file and no library
code); no edit of `baseline_hashes.rs`, of the hash tables or of a golden file; no new dependency, no unsafe code; clean-room
(the closed form derived below from first principles; the textbook form is recalled from memory only as a cross-check).

## What F-524 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. A flat annular disc of Shell4 elements spinning about its
> axis has the radial displacement and the hoop and radial membrane stresses of the plane-stress rotating disc within limits
> registered from a discretisation estimate; the error decreases at the registered order under two mesh refinements.

Clauses: (a1) every limit not stated is registered here; (a2) a flat annular disc of Shell4 elements spinning about its axis
(free inner and outer edges, plane stress); (a3) the radial displacement within a limit registered from a discretisation
estimate; (a4) the hoop membrane stress, the same; (a5) the radial membrane stress, the same; (a6) under two mesh refinements
the error decreases at the registered order. The brief adds: (b1) supports only what removes the rigid motions without
restraining the radial expansion (registered); (b2) the geometric error of straight-edged facets on a circle derived, bounded
or controlled; (b3) no library change expected, defaults bit-identical.

Readings, registered:

- **(a3) "the radial displacement":** `u_r,h = (u_x x + u_y y) / r` at every node (every node lies on its circle, D2), against
  the closed form `u(r)` at the node's radius; the measure `e_u = max_k |u_r,h(x_k) - u(r_k)| / max_[a,b] u`.
- **(a4), (a5) "the hoop and radial membrane stresses":** the membrane forces `N` of `shell::stresses` (element centre, local
  axes `e_1, e_2`) divided by `t`, turned to the polar directions at the element centre `x_c` (the vertex mean, the point
  `shell::stresses` samples): `sigma_r = e_r^T S e_r`, `sigma_theta = e_theta^T S e_theta`, `e_r = x_c / |x_c|`; against the
  closed form at `r_c = |x_c|`. Measures `e_sr = max_e |sigma_r,h - sigma_r(r_c)| / max_[a,b] sigma_r` and `e_st = max_e
  |sigma_theta,h - sigma_theta(r_c)| / sigma_theta(a)` (each normalised by the peak of its own exact field; `sigma_r` vanishes at
  both edges, so a pointwise relative error is not used).
- **(a6) "the registered order" and "two mesh refinements":** three levels, each doubling the radial and the hoop divisions
  together (D2); registered order 2 (D4); observed order `log2(e_k / e_{k+1})` for each of the three measures and each of the
  two refinements, limit `>= 1.8`.
- **(b1)**: D3.

## What was read (no run)

- ADR 0435 (whole), ADR 0432 (arithmetic 1 to 9, D3, D6, D7), ADR 0433 (D6 to D8, the addendum: the measures and the order).
- `crates/rcfd-fem/src/shell.rs` (`facet`, `local_stiffness`: the membrane rows, `plane_matrix`, the drilling row `d`, 2 x 2
  Gauss; `body_load_vector`; `stresses`: the membrane at the centre `point(&f.local, 0, 0)`, `N = D_m eps`),
  `crates/rcfd-fem/src/loads.rs` (`Load::Centrifugal`: the shell closure `rho omega^2 r_perp`), `crates/rcfd-fem/src/model.rs`
  (`Constraint::displacement`, `SolidResult::{displacement, displacement_values, reactions}`), `crates/rcfd-fem/src/free_motions.rs`
  (`free_rigid_motions(model, &[]).count`), `crates/rcfd-fem/tests/si_shell_transformation.rs` (the `Checks` helper, the shell
  mesh helpers, C1 of ADR 0435).
- Scaffold rows: none for F-524 (the item was proposed by ADR 0435; `grep "F-524"` over `crates` finds nothing). Nothing to
  retire.

## Design

No library code. One new test file `crates/rcfd-fem/tests/si_shell_rotating_disc.rs`. Material `E = 2e11 Pa`, `nu = 0.3`,
`rho = 7850 kg/m^3`, no expansion. Disc `a = 0.1 m`, `b = 0.3 m`, `t = 0.005 m`, `omega = 300 rad/s` about `z` through the
origin (`Load::centrifugal([0, 0, 1], [0; 3], 300)`). The default static solve (`StaticOptions::default()`, direct).

### D1. The closed form (derived)

Plane stress, axisymmetric, `u = u(r) e_r`: `eps_r = u'`, `eps_theta = u / r`, `sigma_r = E/(1 - nu^2) (u' + nu u/r)`,
`sigma_theta = E/(1 - nu^2) (u/r + nu u')`. Radial equilibrium of a ring element with the body force `rho omega^2 r`:
`d sigma_r/dr + (sigma_r - sigma_theta)/r + rho omega^2 r = 0`. Substituting, `E/(1 - nu^2) (u'' + u'/r - u/r^2) = -rho omega^2
r`, i.e. `d/dr (1/r d(r u)/dr) = -(1 - nu^2) rho omega^2 r / E`, whose solution is `u = C1 r + C2 / r - (1 - nu^2) rho omega^2
r^3 / (8 E)` (the particular part: `k r^3` gives `8 k r`). Then `sigma_r = E/(1 - nu^2)((1 + nu) C1 - (1 - nu) C2 / r^2) - (3 +
nu) rho omega^2 r^2 / 8`; the free edges `sigma_r(a) = sigma_r(b) = 0` fix `C1, C2`, and with `K = (3 + nu) rho omega^2 / 8`:

`sigma_r = K (a^2 + b^2 - a^2 b^2 / r^2 - r^2)`,
`sigma_theta = K (a^2 + b^2 + a^2 b^2 / r^2 - (1 + 3 nu)/(3 + nu) r^2)`,
`u = r (sigma_theta - nu sigma_r) / E` (Hooke, `eps_theta = u / r`).

(This is the textbook form of the rotating annular disc, recalled from memory as a cross-check only.) The prototype checks it
numerically: equilibrium residual `5.0e-10` of `rho omega^2 b` and Hooke's `u' = (sigma_r - nu sigma_theta)/E` residual `7.8e-11`
(central differences, step `1e-6 m`), `sigma_r(a) = 2e-9 Pa`, `sigma_r(b) = 0`. Values: `K = 2.9142e8 Pa/m^2`; `u(a) =
2.684700e-5 m` (the maximum of `u` over `[a, b]`), `u(b) = 2.543400e-5 m`; `sigma_theta(a) = 5.369400e7 Pa` (the peak),
`sigma_theta(b) = 1.695600e7 Pa`; `max sigma_r = K (b - a)^2 = 1.165725e7 Pa` at `r = sqrt(a b) = 0.17321 m`.

### D2. The mesh (clause a2)

A full annulus (no symmetry cut, so no symmetry condition on the drilling rotation is needed). Levels `k = 1, 2, 3`: `n_r = 4,
8, 16` uniform radial divisions, `n_theta = 24, 48, 96` (`Delta theta = 15, 7.5, 3.75` degrees). Node `(i, j)`, `i = 0..n_r`,
`j = 0..n_theta - 1`, at `r_i = a + (b - a) i / n_r`, `theta_j = 2 pi j / n_theta`, `(r_i cos theta_j, r_i sin theta_j, 0)`,
index `j (n_r + 1) + i`: **every node on its circle** (the straight edges are chords). Element `(i, j)`: nodes `(i, j), (i + 1,
j), (i + 1, j + 1), (i, j + 1)` (`j + 1` modulo `n_theta`), normal `+z` (`(d_1 x d_2)_z = (r_{i+1} - r_i) (r_i + r_{i+1}) sin
Delta theta > 0`). One Shell4 block with `BlockProperty::shell(t)`. Element shapes: isosceles trapezoids, aspect from about 2 at
the bore to 0.6 at the rim. Doubling `n_r` and `n_theta` together keeps the shapes and halves every size: a uniform `h`
refinement.

### D3. The supports (clause b1)

The exact solution has `u_theta = 0` and `w = 0` everywhere. Six single-unknown zeros, on three nodes of the bore (node sets of
one node each): `P_0 = (a, 0)`: `u_y = 0`; `P_90 = (0, a)`: `u_x = 0`; `P_180 = (-a, 0)`: `u_y = 0`; and `w = u_z = 0` at all
three. Each prescribed in-plane component is the tangential one at its node, which the radial expansion does not move
(`u_theta = 0`), so the supports do not restrain the expansion. They remove the six rigid motions and no more: in-plane, `t_x`
by `u_x(P_90)`, and `t_y`, `r_z` by `u_y` at the two nodes on the x axis (`r_z` moves `P_0` by `+a r_z`, `P_180` by `-a r_z`
along `y`); out of plane, `w` at three non-collinear nodes removes `t_z`, `r_x`, `r_y`. Statically determinate, so the reactions
are those of the load's resultant and moment, zero by the mesh's symmetry (the mesh is invariant under the reflections `y -> -y`
and `x -> -x` because `4 | n_theta`, and under rotation by `Delta theta`); check K4. The drilling unknowns `theta_z` are left
free; the drilling penalty has no rigid mode beyond `r_z` (its row ties `theta_z` to the membrane rotation), so the stiffness is
nonsingular (check K5: `free_rigid_motions` counts 0).

The flat facets have zero offsets, so the global matrices are `T^T A T` with `T` a pure rotation per node (ADR 0432 arithmetic
7): the bending part (`w, theta_x, theta_y`) is decoupled from the membrane and drilling part, and the centrifugal load of a flat
facet in its own plane has no out-of-plane force and no moment (ADR 0435 D2: the offset moments vanish). The bending part is
therefore unloaded and its solution zero up to rounding (check K2). The out-of-plane supports only make it nonsingular.

### D4. The discretisation estimate (clauses a3 to a6, b2)

Two error sources, each of second order in the size, derived first, then quantified by the prototype.

- **Radial discretisation.** Along a radial line the membrane field is linear per element; the axisymmetric equation has the
  variable coefficients of D1 (`1/r`, `1/r^2`), so the nodal values are not exact (as Lame's cylinder of ADR 0433 D8): nodal
  error `O(dr^2)`, of order `(dr/a)^2 / 12 = 2.1e-2` at `n_r = 4` (`dr = 0.05`). The centre strains of a bilinear element on a
  parallelogram are superconvergent (the midpoint derivative of a linear interpolant errs by `O(h^2)`); the trapezoids of D2
  depart from parallelograms by `dr Delta theta = O(h^2)` in absolute terms, which keeps the centre stresses second order.
- **Geometric (chords).** The facets fill the inscribed polygon. A thin ring of radius `r` made of chords of angle `Delta
  theta = 2 h` under its own centrifugal load (derived; first principles): a node receives from its two half-chords the
  consistent radial load `2 L (1/3 + cos(2h)/6) rho A omega^2 r` with `L = 2 r sin h`, and holds it by the two chord forces
  `2 T sin h`; so `T / T_exact = 2/3 + cos(2h)/3 = 1 - (2/3) sin^2 h = 1 - Delta theta^2 / 6 + O(Delta theta^4)`: `-1.14e-2`,
  `-2.85e-3`, `-7.14e-4` at `n_theta = 24, 48, 96` (the prototype's evaluation of the exact expression). The hoop stiffness of a
  uniform radial expansion is exact on chords (a chord's strain is `u/r`), so the hoop displacement error of the ring is the same
  `Delta theta^2 / 6`: a second-order geometric error, of the same order as the radial one. It is controlled by refining
  `n_theta` with `n_r` (D2); the nodes are placed on the circles so that no first-order radius error enters.
- **Drilling.** The penalty `alpha G t` with `alpha = 1e-3` adds the energy of the interpolant's rotation, which is antisymmetric
  about each element's mid-radial line; its effect in the prototype: `1e-7` relative to `e_u` at `16 x 96` (with and without the
  penalty: `1.87495e-3` and `1.87490e-3`).
- **The prototype** (`estimate.py`, disclosed): an independent implementation of the membrane and drilling part of the element
  as ADR 0432 describes it (bilinear plane stress, `D_m = E t/(1 - nu^2)[[1, nu, 0], [nu, 1, 0], [0, 0, (1 - nu)/2]]`, 2 x 2
  Gauss, the drilling row `theta_z - (v_x - u_y)/2`), the consistent centrifugal load by 2 x 2 Gauss at the bilinear map (ADR
  0435 D2, exact for the facet), the in-plane supports of D3, a dense solve, and the three measures exactly as read above. Its
  results (`estimate.log`):

| Level | `n_r x n_theta` | `e_u` | `e_sr` | `e_st` |
| --- | --- | --- | --- | --- |
| 1 | 4 x 24 | 2.7370e-2 | 4.3880e-2 | 1.4444e-2 |
| 2 | 8 x 48 | 7.3480e-3 | 1.1595e-2 | 3.7069e-3 |
| 3 | 16 x 96 | 1.8750e-3 | 2.9722e-3 | 9.3471e-4 |
| orders 1 -> 2 | | 1.897 | 1.920 | 1.962 |
| orders 2 -> 3 | | 1.971 | 1.964 | 1.988 |

  The separation of the two sources (prototype): `16 x 24`: `e_u 1.66e-2`, `e_st 2.10e-2` (chord-dominated, about the ring's
  `Delta theta^2 / 6 = 1.14e-2` on `u`); `4 x 96`: `e_u 1.29e-2` (radial-dominated, inside the `2.1e-2` order of magnitude
  above). Both halves scale with `h^2`, so the combined error does; the orders approach 2 from below as the `O(h^4)` remainder
  fades. Tangential displacement, `theta_z` and the reactions are rounding-limited in the prototype (`|u_theta| / max u <=
  3.8e-13`, `|theta_z| <= 1.4e-11 rad`, `|R| / max |f| <= 2.1e-12`).

**Limits** (fixed here from the estimate): each error at each level at most **1.5 times** the prototype's value rounded up to two
digits, which covers a modest difference between the prototype and the element as built (there should be none: D4's prototype
follows the same formulas, check P1); orders `>= 1.8` (registered order 2; the prototype's smallest is 1.897).

## Registered checks

All in `crates/rcfd-fem/tests/si_shell_rotating_disc.rs`, release profile, run once (`cargo test -p rcfd-fem --release --test
si_shell_rotating_disc -- --nocapture --test-threads=1`).

### A. The acceptance

| Id | Clause | Measure | Limit | Estimate |
| --- | --- | --- | --- | --- |
| U1 | a2, a3 | level 1 (4 x 24): `e_u` | <= 4.2e-2 | 2.737e-2 |
| U2 | a2, a3 | level 2 (8 x 48): `e_u` | <= 1.2e-2 | 7.348e-3 |
| U3 | a2, a3 | level 3 (16 x 96): `e_u` | <= 2.9e-3 | 1.875e-3 |
| SR1 | a5 | level 1: `e_sr` | <= 6.6e-2 | 4.388e-2 |
| SR2 | a5 | level 2: `e_sr` | <= 1.8e-2 | 1.160e-2 |
| SR3 | a5 | level 3: `e_sr` | <= 4.5e-3 | 2.972e-3 |
| ST1 | a4 | level 1: `e_st` | <= 2.2e-2 | 1.444e-2 |
| ST2 | a4 | level 2: `e_st` | <= 5.6e-3 | 3.707e-3 |
| ST3 | a4 | level 3: `e_st` | <= 1.5e-3 | 9.347e-4 |
| O1 | a6 | observed orders `log2(e_1 / e_2)` of `e_u`, `e_sr`, `e_st` | each >= 1.8 | 1.897, 1.920, 1.962 |
| O2 | a6 | observed orders `log2(e_2 / e_3)`, the same | each >= 1.8 | 1.971, 1.964, 1.988 |

### P. The estimate is of this element

| Id | Measure | Limit | Estimate |
| --- | --- | --- | --- |
| P1 | each of the nine errors of U, SR, ST against the prototype's ten-digit value (`precise.log`: `2.7369781392e-2, 4.3880312115e-2, 1.4444232806e-2; 7.3480083461e-3, 1.1595371467e-2, 3.7068596673e-3; 1.8749513223e-3, 2.9721749953e-3, 9.3471196775e-4`): `abs(e / e_proto - 1)` | each <= 1e-6 | rounding of two direct solves (the same formulas) |

### K. The set-up

| Id | Measure | Limit | Estimate |
| --- | --- | --- | --- |
| K1 | every level: `max |u_theta,h| / max u` over the nodes | <= 1e-9 | 4e-13 (prototype) |
| K2 | every level: `max(|u_z|, a |theta_x|, a |theta_y|) / max u` over the nodes | <= 1e-9 | zero up to rounding (D3) |
| K3 | every level: `a max |theta_z| / max u` | <= 1e-6 | 5e-8 (prototype) |
| K4 | every level: `max |R| / max |f|` over the six supported unknowns, `f = nodal_load_vector(model, &[])` | <= 1e-9 | 2e-12 (prototype) |
| K5 | every level: `free_rigid_motions(model, &[]).count` | `== 0` | D3 |
| K6 | every level: every node at its radius `r_i` within `1e-15 m`; node and element counts `(n_r + 1) n_theta`, `n_r n_theta` | as stated | D2 |

### N. Negative controls

| Id | Control | Expected |
| --- | --- | --- |
| N1 | level 3 with the bore held (`u_x = u_y = 0` at every bore node, instead of the in-plane supports of D3): `e_u` | `>= 0.5` (prototype 1.0: `u(a) = 0` where the closed form has its maximum) |
| N2 | the chord control: `16 x 24` (radial divisions of level 3, hoop divisions of level 1): `e_st >= 1e-2` and `e_st(16 x 24) / e_st(level 3)` in `[10, 40]` | prototype `2.0998239058e-2`, ratio 22.5 (the chord error does not fall with `n_r` alone) |

### S. Existing tests

| Id | Measure | Limit |
| --- | --- | --- |
| S1 | `git diff d3d05478 --stat -- crates` lists only the new test file | as stated (no library code; no baseline-hash run owed) |

### Mutations (on a scratch copy `w49i-work/f524-scratch/tree` from `git archive`, not committed)

| Id | Mutation | Expected to be caught by |
| --- | --- | --- |
| MU1 | `shell.rs` `plane_matrix`: the off-diagonal `nu c` set to 0 (stiffness and stresses) | U, SR, ST, P1 |
| MU2 | `loads.rs`: the shell closure of `Load::Centrifugal` with `rho omega` instead of `rho omega^2` | U, SR, ST, P1 |
| MU3 | `shell.rs` `stresses`: the membrane strain at the Gauss point `(-G, -G)` instead of the centre | SR, ST, P1 (first-order stress error; U unaffected) |
| MU4 | `shell.rs` `body_load_vector`: the body force at the vertex centroid for every Gauss point | P1 (possibly U, SR, ST) |
| MU5 | test: the closed-form `sigma_theta` with `(1 + 3 nu)/(3 + nu)` replaced by 1 | ST, P1 |
| MU6 | test: `sigma_theta,h` with `+ 2 c s S_xy` in place of `- 2 c s S_xy` (polar turn in global components) | ST, P1 |

## Decision rule

- **done**: every A, P and K check met, every negative control as expected, S1 as stated, every mutation caught by at least one
  check.
- **partial**: any A, P or K check missed (kept as a `*_known_miss` record with the limit intact and the measured number, the
  cause diagnosed), or a mutation not caught, or a check met only after an amendment made after its run.
- **todo**: the checks could not be run.

## Limits of this registration

- L1. One disc (`b/a = 3`, `nu = 0.3`), uniform radial divisions, one family of meshes; no grading study, no other `nu`.
- L2. The disc is flat and loaded in its plane; the bending part is unloaded (D3). A conical or warped disc (offset moments,
  membrane-bending coupling) is not covered; neither is the geometric (centrifugal) stiffening, which does not enter a linear
  in-plane problem.
- L3. Stresses at element centres only; no nodal recovery or extrapolation of shell stresses (none exists in rcfd).
- L4. The order is measured on three levels with max-norm measures; the coarsest level is not quite asymptotic (prototype order
  1.897 for `e_u`).
- L5. The prototype (D4) shares the published formulas with the element, so P1 checks that the element is what ADR 0432 says;
  the closed-form checks A are the independent ones.

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. A full annulus with six point zeros (D3) rather than a quarter with symmetry rollers, which on a shell would need `theta_z =
   0` on the cut lines as well. Recommended and taken.
2. The limits from an independent prototype of the element (D4) times 1.5, with the first-principles derivation of the two
   second-order sources, rather than from the derivation alone (which gives only the order and an order of magnitude).
   Recommended and taken.
