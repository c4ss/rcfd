# ADR 0355: Manufactured solution on distorted meshes: orders of Tet4, Hex8, Tet10 with moved interior nodes, and the Hex8 stiffness rule

Status: **REGISTERED 2026-11-12, written before any code of F-459 and before any run of its checks**. Not yet accepted by the
maintainer. Before this text nothing of F-459 was written, compiled or run: the code and documents under "What was read" were
read, and the arithmetic under "Arithmetic before this registration" was done by hand; no script, no prototype and no library
code was run. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Roadmap item F-459 (depends on F-378), the follow-up 2 of [ADR 0279](0279-manufactured-solution-and-convergence-orders.md)
("The Hex8 stiffness rule is not seen by this test"). Branch `f459-distorted-mms` from main 910fbe22, worktree
`w49i-work/f459`, scratch `w49i-work/f459-scratch`.

House rules (`w49i-work/AGENT_RULES.md`):

- limits are fixed here, before the code and the runs, and are not changed after a run; a missed limit becomes an ignored
  `*_known_miss` test with the limit intact and the number recorded, diagnosed, not tuned away; a defective set-up gets a
  corrected test by an amendment written and committed before the corrected run;
- each registered run is made once; nothing is re-run to pick a result;
- tests only: one new file `crates/rcfd-fem/tests/sb_mms_distorted.rs`; `sb_mms.rs` is not edited (its field, body force,
  error rules and norms are copied); no library file, no shared solver file, no golden, no hash table is touched, so every
  default result keeps its bits by construction; a library defect found is reported, not patched;
- no new dependency, no unsafe code, the graph stays acyclic;
- clean-room: everything below is derived here from first principles (the isoparametric map, the degree of the stiffness
  integrand, Gershgorin's disc theorem and Ostrowski's lower bound for the determinant of a diagonally dominant matrix); the
  failure of the one-point hexahedron on non-parallelepiped elements (Flanagan and Belytschko, 1981) and the optimal orders of
  isoparametric bilinear and trilinear elements on shape-regular, not asymptotically affine meshes (Arnold, Boffi and Falk,
  2002, for quadrilaterals) are cited from memory for orientation only; what is used is derived below.

## What F-459 asks

The acceptance of `docs/roadmap.json`, binding:

> Every limit not stated here is registered in the ADR before the run. The manufactured solution of ADR 0279, case D, on
> boxes whose interior nodes are moved by a registered seed and amplitude (elements valid at every node): on at least three
> meshes, each halving h, the observed L2 order over the finest pair is >= 1.8 for Tet4 and Hex8 and >= 2.7 for Tet10, the
> energy-norm order >= 0.9 and >= 1.8, both errors fall monotonically; a mutation of the Hex8 stiffness rule to one point is
> caught by these checks.

Clauses: (c1) the field, body force and case D of ADR 0279; (c2) interior nodes moved by a registered seed and amplitude;
(c3) every element valid at every node; (c4) at least three meshes, each halving h; (c5) L2 order over the finest pair >= 1.8
(Tet4, Hex8), >= 2.7 (Tet10); (c6) energy-norm order >= 0.9 (Tet4, Hex8), >= 1.8 (Tet10); (c7) both errors fall
monotonically; (c8) the one-point Hex8 stiffness rule is caught by the checks of (c5) to (c7).

The brief adds: reuse the field, body force and error norms of `sb_mms.rs` by copying into the new file; register at least the
one-point Hex8 mutation before running; for the tetrahedra of a split hexahedron a moved interior node must keep every
element's Jacobian positive at every node; check and record the minimum.

## What was read (main 910fbe22, read only)

1. `AGENT_RULES.md`; ADR 0279 in full (registration, amendment 1, addendum 1, its remaining question 2, which is this item).
2. Code: `crates/rcfd-fem/tests/sb_mms.rs` in full; `crates/rcfd-fem/src/builders.rs` in full; `src/mesh.rs` lines 205 to
   525 (`SolidMesh::new`, `add_block` with its check `det J > 0` at every node, the set methods, `jacobians_at_nodes`);
   `src/element.rs` lines 125 to 351 (`Element::point`, the stiffness rules: Tet4 `tet_1`, Hex8 `gauss_2`, Tet10 `tet_4`;
   mass rules `tet_4`, `gauss_3`, `tet_14`); the rule list of `src/quadrature.rs` (names only).
3. **Not read:** the papers cited; any finite-element code outside this repository.

Findings that shape the design:

1. **A moved mesh can be built with the public API.** `SolidMesh::new(nodes)`, then `add_block(kind, connectivity)` with the
   builder's connectivity, then the builder's node, element and surface sets copied by name. `add_block` refuses an element
   with `det J <= 0` at any of its nodes, so (c3) is also enforced by the library; the test measures the minimum itself.
2. **Grid order.** `hex_box` and `tet_box` number the nodes `i + (n + 1)(j + (n + 1) k)`; the interior nodes are those in no
   face node set. The Kuhn split of `tet_box` gives each tetrahedron the vertices `v0, v0 + h e_p, v0 + h (e_p + e_q), v0 + h
   (e_p + e_q + e_r)` (a permutation `(p, q, r)`, two vertices swapped for odd ones), so `det J = h^3` (volume `h^3/6`) on the
   regular mesh.
3. **Tet10 from the moved Tet4.** `upgrade_to_tet10` places each mid-edge node at the middle of its two (moved) corners, so
   the quadratic tetrahedra stay straight-sided (affine). The moved mesh of Tet10 is therefore the moved Tet4 mesh upgraded;
   mid-edge nodes are not moved off their edges (curved elements are not in this item, Limits).
4. **The Hex8 rule is a one-line change** (`element.rs` line 310, `quadrature::gauss_2()`), the mutation M1 below.

## Arithmetic before this registration (disclosed, by hand)

1. **Field, body force, case D, error norms (c1).** Exactly those of ADR 0279 (arithmetic 1 to 4, 7, 8 there): `u_i = prod_j
   sin(pi K_ij x_j + phi_ij)` with its tables, `E = 1`, `nu = 0.3`, `b = -div sigma` from the analytic second derivatives,
   every node of the six face node sets prescribed at `u(x_node)`, the body load by `body_force_loads`, the errors `e_L2`,
   `e_E` with the 125-point rules (degree 7 on the tetrahedron, 9 per variable on the cube). The boundary nodes are not moved,
   so the domain stays the unit cube and case D is unchanged; only the interior of the mesh changes. The rules integrate on
   the reference element with the weight `det J` of the actual map; for the straight tetrahedra this is exact to the same
   degree as in ADR 0279; for the moved hexahedron the squared error times `det J` is no longer a polynomial (the inverse map
   enters the error of the gradient), and the 125-point rule is a quadrature of a smooth integrand with error far below the
   errors measured (its 5-point factors are of degree 9; the integrand varies on the element by the field, `k h <= pi/4`).
2. **Moved nodes (c2).** Seed `0xF459_0355` (SplitMix64, as in `sb_mms.rs`). For each mesh with `n` cells per edge, `h =
   1/n`, a generator `Rng::new(n)` (tag `n`, the same for Tet4, Hex8 and the Tet4 base of Tet10 at the same `n`); the nodes
   are visited in ascending index; for each interior node (in no face node set) three uniforms `r_c` in `[0, 1)` are drawn and
   the node is moved by `alpha h (2 r_c - 1)` in component `c`. Amplitudes: **`alpha = 0.15` for Hex8**, **`alpha = 0.075`
   for Tet4 and Tet10** (the base tetrahedra), each the largest round value below the guaranteed-validity bound of arithmetic
   3 with the margin there. The amplitude is a fixed fraction of `h`, so the relative distortion does not vanish under
   refinement: the meshes are not asymptotically parallelepiped (this is what can expose the one-point rule, arithmetic 5).
3. **Validity at every node (c3), derived.** Every edge vector `h a` of a regular element changes by the difference of two
   offsets, each component in `[-2 alpha h, 2 alpha h]`.
   - *Hex8.* At a corner the three edge vectors to its neighbours along the axes form `J = h (S + P)`, `S = diag(+-1)` (the
     orientation of that corner, `det` of the regular corner `> 0`), `|P_ij| <= 2 alpha`. `S^{-1} J / h = I + S P` has
     diagonal `>= 1 - 2 alpha` and off-diagonal `<= 2 alpha` (two per row): strictly diagonally dominant with positive
     diagonal when `1 - 2 alpha > 4 alpha`, i.e. `alpha < 1/6`. Then every Gershgorin disc lies in the right half-plane, every
     real eigenvalue is positive and complex ones come in conjugate pairs, so the determinant is positive; Ostrowski's bound
     gives `det >= prod_i (a_ii - r_i) >= (1 - 6 alpha)^3`. With `alpha = 0.15`: corner `det J` relative to the regular
     corner (`h^3/8` on the reference cube `[-1, 1]^3`) **`>= 0.1^3 = 1e-3`**.
   - *Tet4 (Kuhn).* `J = h (U + P)` with `U = [e_p, e_p + e_q, e_p + e_q + e_r]` (upper triangular ones in the permuted
     basis), `U^{-1}` with rows `(1, -1, 0)`, `(0, 1, -1)`, `(0, 0, 1)` (row sums of absolute values `<= 2`). `U^{-1} J / h =
     I + U^{-1} P`, entries of `U^{-1} P` at most `2 x 2 alpha = 4 alpha`: dominant when `1 - 4 alpha > 8 alpha`, i.e. `alpha
     < 1/12`; Ostrowski `det >= (1 - 12 alpha)^3`. With `alpha = 0.075`: `det J` relative to `h^3` **`>= 0.1^3 = 1e-3`**.
     A tetrahedron's `det J` is constant, so it holds at every node; Tet10 (straight) has the same `det J` at its ten nodes.
   - The actual minima over thousands of random elements are expected far above the bound (estimate 0.3 to 0.6 for both),
     recorded. Check **J1**: the minimum over all elements and all nodes of `det J / det J_regular` `>= 1e-3` on every mesh
     (guaranteed by the derivation; a miss would mean the code does not do what is registered).
4. **Expected orders of the unmutated elements (c5, c6).** *Tet4, Tet10:* the moved tetrahedra are affine, of bounded shape
   (arithmetic 3), so the a-priori estimates of ADR 0279 arithmetic 5 apply unchanged: L2 order `p + 1`, energy `p`; the
   stiffness rules are exact on affine elements (integrand of degree `2p - 2`: `tet_1` for p = 1, `tet_4` of degree 2 for
   p = 2), the body rules as in ADR 0279. *Hex8:* the trilinear isoparametric space contains the affine functions of the
   physical coordinates (the map is interpolated with the same functions), so the interpolation error is `O(h)` in energy and
   `O(h^2)` in L2 on shape-regular meshes without the asymptotically-affine condition; the full rule `gauss_2` passes the
   patch test exactly (arithmetic 5). Expected: L2 order near 2, energy near 1. *Pre-asymptotic and random effects:* the
   regular-mesh orders of ADR 0279 case D over the same pairs were 1.979, 1.999, 2.982 (L2) and 0.994, 0.999, 1.956 (energy).
   The distortion raises the error constants by an amount set by `alpha`, the same at every level because `alpha` is relative
   to `h`; the meshes of a pair are drawn independently, so the error ratio carries a random part: the error is a sum over
   `N` elements of contributions whose relative spread from the random geometry is of order 0.3, so the relative spread of
   the error is about `0.3/sqrt(N)` (`N = 512` hexahedra or 3 072 tetrahedra at `n = 8`, 384 tetrahedra at Tet10's `n = 4`):
   1 to 1.5 percent, an order spread of about 0.02 to 0.03 per mesh, 0.03 to 0.05 for a pair. Estimates: L2 1.9 to 2.0
   (Tet4, Hex8), 2.85 to 3.0 (Tet10); energy 0.95 to 1.0 and 1.85 to 2.0. The tightest margin is Tet10 energy (estimate 1.85
   to 2.0 against 1.8). Limits as the acceptance: 1.8, 2.7, 0.9, 1.8.
5. **Why the one-point Hex8 is expected to fail here (c8).** The internal force of a constant stress `sigma` at node `a` of
   an element is `F_a = int_e sigma grad N_a dV = sigma int cof(J) grad_xi N_a dxi`, with `cof(J)` the cofactor matrix
   (`det J J^{-T}`). With the trilinear map, `x_eta` is of degree (1, 0, 1) in `(xi, eta, zeta)`, `x_zeta` of (1, 1, 0), so
   the column `x_eta x x_zeta` of `cof(J)` is of degree (2, 1, 1); `d N_a/d xi` is of degree (0, 1, 1); their product is of
   degree at most 2 in each variable, and likewise for the other columns. The 2 x 2 x 2 rule (degree 3 per variable)
   integrates it exactly: the fully integrated element passes the patch test on any moved mesh. The one-point rule (degree 1
   per variable) does not: writing `x_zeta = A_zeta + c eta + ...` with `c` the coefficient of the `eta zeta` term of the map
   (of size `alpha h`, zero only for a parallelepiped), the product contains `(A_eta x c) eta * (d N_a/d xi)_eta-part`, an
   `eta^2` term whose integral is non-zero and whose one-point value is 0. So the one-point element's nodal force of a
   constant stress is wrong by `s_e ~ sigma h^2 alpha` per element, first order in `alpha`, random in sign from element to
   element; at an interior node the element errors do not cancel (they would only for parallelepipeds, i.e. on the regular
   mesh, where ADR 0279's M5 survived). **Effect on the errors:** the discrete equations `K e = r` with a residual of
   divergence form `r = D^T s` (`D` the discrete gradient, `K ~ E h D^T D`), white in `s`: the displacement error is `e ~
   s/(E h) ~ alpha |grad u| h` per node, an L2 term of order 1, and the energy error `e^T K e ~ sum_e s_e^2/(E h) ~ E |grad
   u|^2 alpha^2 (volume)`, a floor of relative size `c alpha` that does not fall with `h` (order 0). With `alpha = 0.15` and
   an unknown constant `c` of order 0.1 to 1: relative energy floor 0.015 to 0.15 against the fully integrated element's
   relative energy error 0.057 at `n = 16` (ADR 0279); an L2 term `~ c' alpha |grad u| h`, with `|grad u| ~ 1.5`, `h = 1/16`:
   `c' x 0.014`, against the converging part 3.8e-3. Over the pair 8 -> 16 the L2 order `log2((4 a + 2 b)/(a + b))` (`a` the
   `h^2` part, `b` the `h` part at `n = 16`) falls below 1.8 when `b > 0.35 a`, i.e. `c' > 0.1`. **Estimate:** the one-point
   Hex8 misses A1 (L2 order 1.0 to 1.7) and probably A2 (energy order 0 to 0.85), or the solve is refused as singular (the
   hourglass modes are no longer held by symmetry); **this is an estimate with an uncertain constant**, not a certainty: a
   constant `c'` below 0.1 and `c` below about 0.03 would let it survive. `alpha` is not raised to make the catch surer than
   the validity bound allows. Also the constant-strain reproduction X3 on the moved Hex8 mesh (below) must fail under the
   one-point rule by the same patch-test argument, with a relative error of order `alpha`: an independent check of this
   derivation.
6. **Exact reproduction on the moved meshes (X3).** A linear field (Tet4, Hex8, `b = 0`) and the quadratic field of ADR 0279
   (Tet10, constant `b`, integrated exactly by `tet_14` on affine elements) are reproduced to rounding on moved meshes if the
   stiffness passes the patch test (Tet4, Tet10 affine; Hex8 `gauss_2` by arithmetic 5): relative L2 and energy errors of
   rounding, estimate 1e-15 to 1e-13 (the distortion worsens the conditioning modestly), limit **1e-9**, on `n = 4` (Tet4,
   Hex8; 27 moved nodes) and `n = 2` and `n = 4` for Tet10 (Tet10 at `n = 2` has one moved vertex), case D.
7. **Meshes and cost.** The sequences of ADR 0279: Tet4 and Hex8 on `n = 4, 8, 16`, Tet10 on `n = 2, 4, 8` (h halves
   nominally; every edge of the moved meshes is within `+-2 alpha h` per component of its regular length, so the mesh size
   halves as well). Solve sizes as ADR 0279 case D (finest 10 125 free unknowns); measured there 1.3 to 5.5 s per solve. Per
   element test: 3 meshes x (case D, negative control) = 6 solves: under 30 s in release. **Heavy record H1 (ignored):** the
   finer family `n = 6, 12, 24` (Tet4, Hex8), `3, 6, 12` (Tet10), case D, moved by the same rule; ADR 0279 measured 108.5 s
   for the regular family.
8. **Negative control N1 (extension).** As ADR 0279 arithmetic 6: the body force with the lambda term reversed, case D on the
   same moved meshes: the error tends to `||w|| > 0`, observed orders below 0.1 (ADR 0279 measured -0.005 to 0.015); limit
   `< 0.5`, both norms, all three elements.

## Design

### D1. Copied code (from `sb_mms.rs`, unchanged in substance)

`Rng`, `Checks`, `lame`, `d_matrix`, the `Field` trait, `voigt_strain`, `stress_tensor`, `body_force`, `Trig`, `Poly`,
`gauss_legendre`, `error_rule`, the case-D model and solve (`model_of`, the prescribed values, `body_force_loads`,
`solve_static_with` with `StaticOptions::default()`), `errors`, `Level`, `order`, `orders`, `limits`, the A1 to A3 check.
The traction helper and case T are not copied (not in this item). The seed constant is this ADR's.

### D2. Moved meshes

`moved(mesh, alpha, n)`: the offsets of arithmetic 2 applied to the interior nodes of a builder mesh; a new `SolidMesh` from
the moved nodes, the same blocks (`add_block` checks `det J > 0` at every node), and every node, element and surface set
copied by name. Hex8: `moved(hex_box(n), 0.15, n)`. Tet4: `moved(tet_box(n), 0.075, n)`. Tet10: `upgrade_to_tet10(moved(tet_box(n),
0.075, n))`. `min_jacobian_ratio(mesh)`: the minimum over elements of `jacobians_at_nodes` divided by the regular value
(`h^3/8` for Hex8, `h^3` for the tetrahedra; Tet10's mid-edge nodes included). Recorded too: the largest offset relative to
`h` (must be `<= alpha`), and for Hex8 the minimum of `det J` over the 125 error points relative to `h^3/8` (J2).

### D3. Tests in `crates/rcfd-fem/tests/sb_mms_distorted.rs`

| Test | Content | Ignored |
|---|---|---|
| `j1_moved_meshes_valid_at_every_node` | J1 and J2 on every mesh of the fast and heavy sequences (meshes only, no solve) | no |
| `x3_polynomial_fields_reproduced_on_moved_meshes` | X3 | no |
| `a_tet4_orders_on_moved_meshes_and_negative_control` | A1 to A3 Tet4, N1 Tet4 | no |
| `a_hex8_orders_on_moved_meshes_and_negative_control` | the same for Hex8 | no |
| `a_tet10_orders_on_moved_meshes_and_negative_control` | the same for Tet10 | no |
| `h1_heavy_finer_family_on_moved_meshes` | H1 | yes (heavy record) |

Every measured value is printed with its limit; a test fails at its end if any of its limits is missed.

## Checks

| Check | Clause | Quantity | Limit | Estimate |
|---|---|---|---|---|
| J1 | c2, c3 | min over elements and nodes of `det J / det J_regular`, every mesh, per K; largest offset / `h` | `>= 1e-3`; offset `<= alpha` | 0.3 to 0.6; `alpha` |
| A1 | c1, c4, c5 | L2 order, finest pair, case D, per K | `>= 1.8` (Tet4, Hex8), `>= 2.7` (Tet10) | 1.9 to 2.0; 2.85 to 3.0 |
| A2 | c6 | energy-norm order, finest pair, per K | `>= 0.9`, `>= 1.8` | 0.95 to 1.0; 1.85 to 2.0 |
| A3 | c7 | `e_1 > e_2 > e_3`, both norms, per K | strictly decreasing | ratios about 4 / 8 (L2), 2 / 4 (energy) |
| M1 | c8 | the one-point Hex8 rule (mutation, below) fails the Hex8 test of A1 to A3 | caught by A1, A2 or A3 (or the solve refused) | L2 order 1.0 to 1.7, energy 0 to 0.85 (uncertain constant) |
| J2 | extension | Hex8: min `det J` over the 125 error points of every element / `h^3/8` | `> 0` | 0.2 to 0.6 |
| X3 | extension | linear (Tet4, Hex8, n = 4) and quadratic (Tet10, n = 2 and 4) fields on moved meshes, relative L2 and energy errors | `<= 1e-9` | 1e-15 to 1e-13 |
| N1 | extension | lambda term reversed, case D, L2 and energy orders, finest pair, per K | `< 0.5` | `< 0.1` |
| H1 | extension (heavy) | family 6, 12, 24 / 3, 6, 12 on moved meshes: A1, A2, A3 with the same limits; J1 on its meshes; wall time | as A1 to A3; `<= 1800 s` | as A; 2 to 4 min |

Wall times are recorded, not limited (except H1's budget); a fast test above 120 s in release is reported.

### Planned mutations (scratch copy `w49i-work/f459-scratch/mut`, each run once)

| Id | Where | Change | Expected to catch it |
|---|---|---|---|
| M1 | library `element.rs` line 310 | Hex8 stiffness rule `gauss_2` -> `gauss_1` | **A1/A2 Hex8** (c8; arithmetic 5, estimate with an uncertain constant), X3 Hex8 |
| M2 | library `element.rs` line 336 | Tet10 stiffness rule `tet_4` -> `tet_1` | A1/A2/A3 Tet10 or a refused singular solve (one point for 30 unknowns: rank at most 6 per element), X3 Tet10 |
| M3 | test, error norms | the exact field evaluated at the point of the unmoved element (the regular map) instead of the moved one | A1 every K (an error `|grad u| alpha h`, L2 order near 1), A2 Tet10 |
| M4 | test, moved mesh | Tet4 amplitude 0.075 -> 0.3 (four times the value, above the bound 1/12 of arithmetic 3) | J1 Tet4 (minimum ratio below 1e-3 or negative; `add_block` refuses an inverted element, which counts as caught by J1) |
| M5 | library `element.rs` line 310 | Hex8 stiffness rule `gauss_2` -> `gauss_3` (over-integration) | **expected to survive** (both rules pass the patch test and converge; a check that the order checks do not reject a sound rule), recorded |

The mutation runs use the fast tests only (not H1). M1 is the clause (c8); M2 to M5 test the tests. Moving the boundary
nodes as well is not a mutation: the field and its body force are defined everywhere, so the manufactured solution holds on
the moved domain too, with the prescribed values taken at the moved nodes; it is not run.

## Decision rule

**done**: J1, A1 to A3 for all three elements met, and M1 caught by the Hex8 test of A1 to A3 (a refused solve in that test
counts as caught and is reported as such). M2 to M4 caught and M5 surviving are expected; a deviation among them is recorded
and diagnosed and does not alone block done unless its diagnosis shows the acceptance checks rest on a defect. A miss among
the extensions (J2, X3, N1, H1) is a `*_known_miss` record with a follow-up and does not alone block done, unless its
diagnosis shows the acceptance checks rest on a defect (then partial). **partial** (rule I): a miss or an unrun check among
J1, A1 to A3; M1 not caught by these checks; or M1 caught only after a post-run amendment. **todo**: the test file does not
build.

## Limits (what is not covered)

- Case D only (all faces prescribed); the traction case T of ADR 0279 is not repeated on moved meshes.
- Straight tetrahedra: Tet10's mid-edge nodes stay on the straight edges; no curved Tet10.
- Random interior moves of bounded amplitude (shape-regular meshes); no systematic distortion (for example a trapezoidal or
  sheared grading), no element near degeneracy beyond what the random draw gives, no boundary-node moves.
- One material, `nu = 0.3`; no near-incompressible case.
- The direct solver only; the heavy record stops at the solve size of ADR 0279's H1.

## The roadmap entry, as it stood before this registration

F-459, title "Manufactured solution on distorted meshes: orders of Tet4, Hex8, Tet10 with moved interior nodes, and the Hex8
stiffness rule", deps F-378; the acceptance quoted under "What F-459 asks".

## Addendum 1: results (2026-11-12)

Runs, each once, release profile, on the shared machine, at commit 30e7b53c (the test file as committed before any run): the
fast file `cargo test -p rcfd-fem --release --test sb_mms_distorted -- --nocapture --test-threads=1` (7.4 s for the file, log
`w49i-work/f459-scratch/run1_fast.log`); the heavy record H1 (`-- --ignored h1`, 90.4 s, `run1_h1.log`); the mutations M1 to
M5 on the archive copy `w49i-work/f459-scratch/mut` of 30e7b53c by `mutate.py` (each applied, the fast file run once, the file
restored; log `mutations.log`). No amendment was needed; nothing above this addendum was changed after a run.

### Checks

| Check | Limit | Measured | Met |
|---|---|---|---|
| J1 min `det J / det J_regular` at every node, Tet4 (4, 8, 16; 6, 12, 24) | `>= 1e-3` | 0.795, 0.685, 0.654; 0.703, 0.698, 0.666 | yes |
| J1 Hex8 (same meshes) | `>= 1e-3` | 0.622, 0.432, 0.374; 0.483, 0.402, 0.369 | yes |
| J1 Tet10 (2, 4, 8; 3, 6, 12) | `>= 1e-3` | 0.903, 0.795, 0.685; 0.723, 0.703, 0.698 | yes |
| J1 largest offset / h | `<= alpha` | at most 0.14999 (Hex8, 0.15), 0.07499 (tetrahedra, 0.075) | yes |
| A1 L2 order, finest pair, Tet4 / Hex8 / Tet10 | `>= 1.8 / 1.8 / 2.7` | 1.9798 / 1.9933 / 2.9705 | yes |
| A2 energy order, finest pair | `>= 0.9 / 0.9 / 1.8` | 0.9945 / 0.9984 / 1.9511 | yes |
| A3 monotone fall, both norms | decreasing | L2 Tet4 8.78e-2, 2.32e-2, 5.87e-3; Hex8 6.04e-2, 1.59e-2, 3.99e-3; Tet10 4.00e-2, 5.19e-3, 6.62e-4 (energy likewise) | yes |
| M1 one-point Hex8 caught by A1 to A3 | caught | A1 Hex8 **1.7577**, A2 Hex8 **0.7292** (both missed under the mutation) | yes |
| J2 Hex8 min `det J` at the 125 error points / regular | `> 0` | 0.642, 0.483, 0.430; 0.524, 0.466, 0.421 | yes |
| X3 polynomial fields on moved meshes, relative L2 / energy | `<= 1e-9` | 1.8e-16 to 4.0e-16 / 1.1e-15 to 3.9e-15 | yes |
| N1 lambda term reversed, L2 / energy order, Tet4 / Hex8 / Tet10 | `< 0.5` | -0.0015 / 0.0001; 0.0145 / 0.0060; -0.0010 / -0.0052 | yes |
| H1 (6, 12, 24 / 3, 6, 12) L2 order | as A1 | 1.9895 / 1.9981 / 2.9812 | yes |
| H1 energy order | as A2 | 0.9973 / 0.9994 / 1.9738 | yes |
| H1 monotone fall | decreasing | yes for all three | yes |
| H1 total wall time | `<= 1800 s` | 90.4 s (Hex8 73.9 s, Tet4 9.2 s, Tet10 7.3 s; largest solve 36 501 unknowns) | yes |

Relative errors at the finest fast mesh: L2 6.95e-3 (Tet4), 4.72e-3 (Hex8), 7.84e-4 (Tet10); energy 7.80e-2, 5.79e-2,
1.17e-2, against ADR 0279's regular meshes 6.9e-3, 4.5e-3, 7.7e-4 and 7.7e-2, 5.7e-2, 1.2e-2: the moves raise the constants
by 0 to 5 percent. RMS node move (all nodes, boundary and mid-edge included) 0.03 to 0.13 h. The measured minima of J1 (0.37
to 0.90) are far above the guaranteed bound, as estimated (0.3 to 0.6; Tet10 at its coarse meshes higher, few moved nodes).
Wall time of the fast file 7.4 s (largest test, Tet4, 3.0 s).

### Mutations (scratch copy of 30e7b53c, each run once)

| Id | Change | Caught | By |
|---|---|---|---|
| M1 | Hex8 stiffness `gauss_2` -> `gauss_1` | **yes** | A1 Hex8 (L2 order 1.7577; pair 4 -> 8 1.8832), A2 Hex8 (energy order 0.7292; 0.9066), and X3 Hex8 (relative L2 1.9e-4, energy 1.7e-3) |
| M2 | Tet10 stiffness `tet_4` -> `tet_1` | yes | the solve refused: `SingularStiffness` (node 13, pivot 0) in the Tet10 order test and in X3 |
| M3 | exact field at the regular map's point | yes | A1 Tet4 (1.7282), A1 Hex8 (1.2982), A1 and A2 Tet10 (0.5487, 1.5529); X3 unaffected (a polynomial field's error is not evaluated this way in X3, which calls the unmutated path) |
| M4 | Tet4 amplitude 0.075 -> 0.3 | yes | `add_block` refuses an inverted element (element 6951 of Tet4 n = 16, `det J = -4.7e-6`) in J1 and in the Tet4 order test; the Tet10 meshes (n <= 8 at 0.3) stayed valid and met their limits |
| M5 | Hex8 stiffness `gauss_2` -> `gauss_3` | survived (registered as expected to survive) | every limit met; Hex8 orders 1.9931, 0.9984 |

**M1 against arithmetic 5.** The one-point Hex8 is caught by the acceptance checks, as estimated, and by the patch test X3 as
derived (constant-strain reproduction fails at a relative energy error 1.7e-3, against 1e-15 for `gauss_2`). The sizes: the
L2 miss is narrow (1.7577 against 1.8, within the estimated range 1.0 to 1.7 at its upper end) and the energy miss clear
(0.7292 against 0.9, in the range 0 to 0.85). The one-point errors are below the full rule's at n = 4 and 8 (L2 4.46e-2,
1.21e-2) and the floor shows at 16 (energy 2.41e-1 against 1.87e-1): the order falls from pair to pair (energy 0.907, then
0.729), so on finer meshes the miss widens; on the registered meshes the catch rests mainly on A2.

### Deviations and what was not run

- None of the registration was changed. M3's note: as registered it changes only where the exact field is evaluated in the
  order tests; X3 does not use that call.
- Not run: case T on moved meshes, curved Tet10, a near-incompressible material (Limits).
- No library file was edited; no golden or hash table touched; the files of the branch are the ADR and the new test file.

### Decision by the registered rule

**done**: J1 and A1 to A3 met for all three elements; M1 caught by the Hex8 test of A1 to A3 (A1 and A2 missed under the
mutation) without any amendment; M2 to M4 caught, M5 survived as expected; every extension (J2, X3, N1, H1) met.

### Remaining questions

1. **The margin of the M1 catch.** The L2 catch is narrow (1.7577 against 1.8); the energy catch (0.7292 against 0.9) carries
   it. Options: (a) leave it (the registered checks catch it, and the energy floor grows with refinement); (b) a follow-up that
   adds the moved-mesh patch test X3 for Hex8 to the element tests of ADR 0273 as the direct check of the rule (caught at
   1.7e-3 against 1e-9, a margin of six orders). Recommendation: (b), small; it makes the rule's check independent of a
   convergence study.
2. **ADR 0279's note on M5.** ADR 0279 recorded the one-point Hex8 as surviving its regular-mesh checks; this ADR closes that
   gap. Options: (a) mention it in the note of F-378; (b) leave F-378's note as is. Recommendation: (a), one sentence.

### For the lead: roadmap entries

F-459: proposed status **done**. Note:

> Structural solver, basic tier. ADR 0355, 2026-11-12: the manufactured solution of ADR 0279 (case D) on boxes with interior nodes moved by seed 0xF459_0355, uniform offsets of at most 0.15 h per component for Hex8 and 0.075 h for Tet4 and Tet10 (Tet10 from the moved Tet4, straight edges), validity guaranteed by diagonal dominance (det J >= 1e-3 of the regular value) and measured: minimum det J at the nodes 0.37 (Hex8) to 0.65 (tetrahedra) of the regular value; tests crates/rcfd-fem/tests/sb_mms_distorted.rs only. Tet4 and Hex8 on 4, 8, 16, Tet10 on 2, 4, 8 cells per edge: L2 orders over the finest pair 1.980, 1.993, 2.971 (limits 1.8, 1.8, 2.7), energy orders 0.995, 0.998, 1.951 (0.9, 0.9, 1.8), errors fall monotonically; polynomial fields reproduced on moved meshes to 4e-15; negative control orders below 0.015; heavy record on 6, 12, 24 / 3, 6, 12 (36 501 unknowns, 90 s): 1.990, 1.998, 2.981 and 0.997, 0.999, 1.974. The one-point Hex8 stiffness (which survived ADR 0279 on regular meshes) is caught: L2 order 1.758, energy order 0.729, and the moved-mesh patch test fails at 1.7e-3; four of the other mutations behave as registered (one-point Tet10 refused as singular, the exact field at the regular map caught, an amplitude above the validity bound refused as inverted, the over-integrated Hex8 survives).

F-378: suggested addition to its note (one sentence at the end):

> The one-point Hex8 stiffness that survived here is caught on moved meshes by ADR 0355 (F-459).

Follow-up item (id by the lead):

1. Title: "Element tests: the constant-strain patch test of Hex8 on a moved mesh". Deps: F-459.

> Every limit not stated here is registered in the ADR before the run. A linear displacement field on a box of Hex8 whose interior nodes are moved by a registered seed and an amplitude below the validity bound of ADR 0355, all boundary nodes prescribed, no body force: the relative L2 and energy errors are <= 1e-9, and the same test with the stiffness rule mutated to one point gives a relative energy error >= 1e-5.
