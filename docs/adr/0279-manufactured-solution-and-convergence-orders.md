# ADR 0279: Verification by a manufactured solution: observed order of displacement and stress for the three elements

Status: **REGISTERED 2026-11-12, written before any code of F-378 and before any run of its checks; planned by ADR 0270**.
Not yet accepted by the maintainer. Before this text nothing of F-378 was written, compiled or run: the code and documents
under "What was read" were read, and the arithmetic under "Arithmetic before this registration" was done by hand; no script,
no prototype and no library code was run. Results go in the addendum at the end; nothing above the addendum is changed after
a run.

Work package SB-9 of the structural solver, basic tier ([ADR 0270](0270-structural-solver-basic-tier-plan.md), section 3, the
entry for ADR 0279). Roadmap item F-378 (depends on F-375 and F-376). Size M. Number 0279, fixed by the plan; the file name of
the planned stub is kept and the stub's text is kept unchanged at the end ("The planned stub, as written"). Branch
`f378-manufactured-solution` from main 4b037ecb, worktree `w49i-work/f378`.

House rules (`w49i-work/AGENT_RULES.md`):

- limits are fixed here, before the code and the runs, and are not changed after a run; a missed limit becomes an ignored
  `*_known_miss` test with the limit intact and the number recorded, diagnosed, not tuned away; a defective set-up gets a
  corrected test by an amendment written and committed before the corrected run, the old test stays as an ignored record;
- each registered run is made once; nothing is re-run to pick a result;
- tests only: one new file `crates/rcfd-fem/tests/sb_mms.rs`; no library file, no shared solver file, no golden, no hash
  table is touched, so every default result keeps its bits by construction; `src/solve.rs` (being edited for F-381) is not
  touched and the iterative solver of ADR 0282 is not used;
- no new dependency, no unsafe code, the graph stays acyclic;
- clean-room: everything below is derived here from first principles (Navier's equations of linear elasticity, the
  isoparametric map, Gauss-Legendre quadrature computed by Newton's method on the Legendre recurrence); the method of
  manufactured solutions (Roache, 1998) and the a-priori estimates (Strang and Fix, 1973; Ciarlet, The Finite Element Method
  for Elliptic Problems, 1978, on the effect of quadrature) are cited from memory for orientation only.

## What F-378 asks

The acceptance of `docs/roadmap.json`, binding:

> Every limit not stated here is registered in the ADR before the run. Manufactured solution: a smooth trigonometric
> displacement field on the unit cube with nu = 0.3, the body force from the analytic divergence of the stress (checked
> against finite differences within 1e-6), displacements prescribed on all faces, and a second case with tractions on three
> faces. On at least three meshes, each halving h, the observed order of the L2 displacement error over the finest pair is
> >= 1.8 for Tet4 and Hex8 and >= 2.7 for Tet10, the observed order of the energy-norm error is >= 0.9 and >= 1.8, and both
> errors fall monotonically. Negative control: a body force with the sign of the lambda term reversed gives an order below
> 0.5.

Clauses: (c1) a smooth trigonometric field on the unit cube, nu = 0.3; (c2) the body force from the analytic divergence of
the stress, checked against finite differences within 1e-6; (c3) case D, displacements prescribed on all six faces; (c4) case
T, tractions on three faces; (c5) at least three meshes, each halving h; (c6) L2 order over the finest pair >= 1.8 (Tet4,
Hex8), >= 2.7 (Tet10); (c7) energy-norm order >= 0.9 (Tet4, Hex8), >= 1.8 (Tet10); (c8) both errors fall monotonically; (c9)
negative control, lambda term of the body force reversed: order below 0.5. The limits of (c6) to (c9) are applied to both
cases D and T (the acceptance does not restrict them to one), the negative control to case D.

The brief adds: check whether the load interfaces take a function of position; use per-node values through
`solve_static_with` where needed; keep the heavy end reasonable on a shared machine with the direct solver; put the largest
mesh in an ignored heavy record with a fast test covering the path.

## What was read (main 4b037ecb, read only)

1. `AGENT_RULES.md`; ADR 0270 (sections 1, 2, 3 entries 0271 to 0284, 4 wave notes, 5); this ADR's stub; ADR 0273's
   remaining question 2 (the mass rules, and the note to ADR 0279 on rule degree); ADR 0274's addendum (the measured
   factorisation times: Hex8-box sparsity of 27 783 unknowns in 34.9 s after RCM, 452 MB; 30^3 Laplacian 0.8 s); ADR 0276's
   roadmap texts for F-375 and the sentence for F-378; ADR 0277's limits and the sentence for F-378; ADR 0281 (head, design, the
   style followed here); ADR 0284 (its decision 1: the mass rules `tet_4`, `gauss_3`, `tet_14` are kept); the roadmap entry
   and note of F-378. ADRs 0271, 0275: their decisions as reflected in the code and the roadmap notes; their bodies were not
   read line by line. ADR 0282 is in progress elsewhere and is not used.
2. Code: `crates/rcfd-fem/src/{loads.rs, solve.rs, element.rs, quadrature.rs, material.rs, builders.rs, lib.rs}` in full; the
   public signatures of `model.rs`, `constraint.rs`, `mesh.rs`; the head of `tests/sb_loads.rs` (its helpers `Rng`, `Checks`
   are the pattern followed).
3. **Not read:** the books cited; any finite-element code outside this repository.

Findings that shape the design:

1. **Body force as a function: provided.** `loads::body_force_loads(model, |x| b(x))` returns the consistent load `int N_a
   b dV` with each element's mass rule (`tet_4` degree 2 for Tet4, `gauss_3` for Hex8, `tet_14` degree 5 for Tet10) as one
   `NodalForceAt` per node, for `solve_static_with`.
2. **Traction varying over a face: not provided.** `Load::Traction` is a uniform vector on a surface set. The test therefore
   builds the consistent load `f_a = int N_a t(x) dS` itself from the public `loads::face_shape_functions`, `loads::face_rule`
   and `SolidMesh::face_nodes`, the same functions and rule the library uses for a uniform traction (the face area element
   `|x_u x x_v|`), and passes it as `NodalForceAt` per node. Check X1 holds this helper to `Load::Traction` for a uniform
   traction. No library change is needed.
3. **Prescribed values per node: provided.** `constraint::Prescribed::new(node, component, value)` through
   `solve_static_with`; the exact field is evaluated at each node, including the mid-edge nodes of Tet10 (nodal
   interpolation). The box builders give node sets and surface sets `xmin .. zmax`; `upgrade_to_tet10` adds the mid-edge
   nodes to the node sets of their edges.
4. **Values away from the stiffness points: provided.** `Element::point(coords, xi)` (public provided method) gives `x`, `N`
   and the physical gradients at any reference point, so `u_h` and `grad u_h` are computed directly at the points of an error
   rule; `material::isotropic_elasticity(e, nu)` gives `D`.
5. **No library rule is of high enough degree for the error norms** (ADR 0273's note): the highest are `tet_14` (degree 5)
   and `gauss_3` (degree 5 per variable). The test builds its own (D3), checked by X2.

## Arithmetic before this registration (disclosed, by hand)

1. **Field (c1).** On the unit cube `[0, 1]^3`, `u_i(x) = U prod_{j=1..3} sin(k_ij x_j + phi_ij)`, `k_ij = pi K_ij`, with

   | component | K_i1, K_i2, K_i3 | phi_i1, phi_i2, phi_i3 |
   |---|---|---|
   | u_x | 1.0, 0.5, 0.75 | 0.3, 0.7, 0.5 |
   | u_y | 0.75, 1.0, 0.5 | 0.9, 0.2, 0.6 |
   | u_z | 0.5, 0.75, 1.0 | 0.4, 0.8, 0.1 |

   `U = 1`, `E = 1`, `nu = 0.3` (non-dimensional; the orders are scale-free), density 0, expansion 0. The phases keep every
   component non-zero on the faces (inhomogeneous boundary values) and the divergence of order `k U` (needed by the negative
   control, arithmetic 6). The largest wavenumber is `pi` (half a wave across the cube).
2. **Derivatives.** With `s_ij = sin(k_ij x_j + phi_ij)`, `c_ij = cos(k_ij x_j + phi_ij)`: `d_m u_i = U k_im c_im prod_{j !=
   m} s_ij`; `d_m d_n u_i = U k_im k_in c_im c_in prod_{j != m, n} s_ij` for `m != n`, `-U k_im^2 prod_j s_ij` for `m = n`.
3. **Body force (c2).** `sigma_ij = lambda delta_ij div u + mu (d_i u_j + d_j u_i)`, `lambda = E nu/((1 + nu)(1 - 2 nu)) =
   0.576 923 E`, `mu = E/(2 (1 + nu)) = 0.384 615 E`. Equilibrium `d_j sigma_ij + b_i = 0` gives `b_i = -[(lambda + mu) d_i
   (d_k u_k) + mu d_j d_j u_i]`. **Finite-difference check:** at 200 points uniform in the cube (registered seed), the
   divergence of the stress computed from the analytic gradient (arithmetic 2), by central differences with step `h = 1e-4`
   in each direction, `-sum_j (sigma_ij(x + h e_j) - sigma_ij(x - h e_j))/(2 h)`, against `b_i(x)`; error measure: the
   largest component difference over the points divided by the largest `|b_i|` over the points. Truncation `h^2/6 |d^3
   sigma|`, relative to `|b| ~ k^2 E U`: `h^2 k^2/6 = 1.6e-8`; rounding `eps |sigma|/h / |b| ~ 3e-13`. Estimate `<= 2e-8`,
   limit **1e-6** (acceptance). The same check on the gradient itself (extension F1): `d_m u_i` against central differences
   of `u_i`, error relative to the largest `|d_m u_i|`: estimate `h^2 k^2/6 = 1.6e-8`, limit 1e-6.
4. **Cases.** **D:** every node of the six face node sets prescribed, all three components, at `u(x_node)`; the body force
   `b` by `body_force_loads`. **T:** nodes of `xmin`, `ymin`, `zmin` prescribed as in D; on the faces `xmax`, `ymax`,
   `zmax` the traction `t = sigma(x) n` with the box normal `n = +e_x, +e_y, +e_z`, integrated per face by the helper of
   finding 2; plus the body force. Nodes on the edges shared by a prescribed and a loaded face are prescribed (their face
   loads go to the reactions). Case T has no free rigid motion (three faces fully held).
5. **Expected orders (c6, c7).** For complete polynomials of degree p (Tet4 p = 1, Tet10 p = 2; Hex8 contains the complete
   linears), with nodal interpolation of the boundary values and loads integrated by rules of sufficient degree, the
   a-priori estimates give `||u - u_h||_L2 = O(h^{p+1})`, `||u - u_h||_E = O(h^p)`. Quadrature of the loads (Ciarlet,
   from memory): energy-optimal if the rule is exact to degree `2p - 2`, L2-optimal if exact to `2p - 1`; the body rules
   have degree 2 (Tet4, needs 1), 5 per variable (Hex8) and 5 (Tet10, needs 3); the face rules (degree 4 on triangles, 5 per
   variable on quadrilaterals) satisfy the same. The stiffness is exact on these straight elements. **Pre-asymptotic
   size:** the relative correction to `C h^q` is of order `(k h)^2/12` to `(k h)^2/20`; on the finest pair `k h = pi/8 =
   0.39` (Tet4, Hex8, 8 -> 16 cells per edge) and `pi/4 = 0.79` (Tet10, 4 -> 8): corrections of about 1.3 percent and 3 to
   5 percent, which change the observed order by `log2(1 + delta)`, about 0.02 and 0.04 to 0.07. Estimates: L2 order 1.95
   to 2.0 (Tet4, Hex8) and 2.85 to 3.0 (Tet10); energy order 0.97 to 1.0 and 1.9 to 2.0. Limits as the acceptance: 1.8, 2.7,
   0.9, 1.8: margins 0.15 to 0.3 in order, i.e. a constant in the error ratio of about 10 to 20 percent. The Kuhn tetrahedra
   of `tet_box` have edge lengths `h, sqrt 2 h, sqrt 3 h` with `h = 1/n`; halving `n`'s spacing halves every edge, so the
   ratio 2 is exact.
6. **Negative control (c9).** `b_neg = -[(-lambda + mu) grad div u + mu Laplace u] = b + 2 lambda grad div u`. The discrete
   solution converges to `u + w`, `w` the solution with zero boundary values and load `2 lambda grad div u`, `|w| ~ 2 lambda
   k^2 U/(mu k^2) ~ 3 U` times a geometry factor (taken as `>= 0.05 U` in the L2 sense): the error tends to `||w|| > 0`, so
   `e(h)/e(h/2) = 1 + O(h^p)` and the observed order is `|log2(1 + O(h^p))|`, estimate below 0.1. Limit **< 0.5** for both
   norms (acceptance), case D, all three elements, the same meshes.
7. **Error norms.** `e_L2 = sqrt(sum_e sum_q w_q |u(x_q) - u_h(x_q)|^2)`, `e_E = sqrt(sum_e sum_q w_q d_q^T D d_q)`, `d_q =
   eps(x_q) - eps_h(x_q)` in Voigt order with engineering shear, `w_q` the rule weight times `det J`. Observed order of a pair
   `q = log2(e(h)/e(h/2))`. Relative values are reported against `||u||_L2`, `||u||_E` computed on the finest mesh with the
   same rule (not used in any limit).
8. **Error rules (D3).** The squared error of Tet10 is to leading order a polynomial of degree 6 on each element; Tet4 and
   Hex8 degree 4 (per variable 4 on the hexahedron). Rules: on the cube the tensor product of 5-point Gauss-Legendre (degree
   9 per variable); on the tetrahedron the collapsed product `x = u`, `y = (1 - u) v`, `z = (1 - u)(1 - v) w` on `[0, 1]^3`
   with Jacobian `(1 - u)^2 (1 - v)` and 5-point Gauss-Legendre in each of `u, v, w`: a monomial of total degree d becomes
   a polynomial of degree `<= d + 2` in u, `<= d + 1` in v, `<= d` in w, so the rule (degree 9 per variable) is exact for
   `d <= 7`. 125 points per element. The 5-point nodes are the roots of `P_5`, found by Newton's method from `cos(pi (i -
   1/4)/(n + 1/2))` with the three-term recurrence, weights `2/((1 - x^2) P_5'(x)^2)`; nothing is recalled. Check X2:
   monomials up to these degrees against `a! b! c!/(a + b + c + 3)!` (tetrahedron) and `prod 2/(a+1)` for even exponents,
   0 for odd (cube), each monomial within `1e-14` absolute on the reference domains (values `<= 8`; rounding of 125 terms
   `~ 1e-15`).
9. **Meshes and cost.** Fast (acceptance) sequences: Tet4 and Hex8 on `n = 4, 8, 16` cells per edge; Tet10 on `n = 2, 4,
   8` (quadratic node spacing `h/2`, so the same node grids `17^3` at the finest). Finest free unknowns: case D `15^3 x 3 =
   10 125` (Hex8, Tet4 at 16; Tet10 at 8), case T `16^3 x 3 = 12 288`. Cost from ADR 0274's measurement (27 783 unknowns
   of a Hex8-box graph in 34.9 s, growth like `N^(7/3)`): Hex8 and Tet4 at 1.0e4 to 1.2e4 unknowns about 3 to 5 s per solve;
   Tet10 couples nodes two spacings apart, about twice the bandwidth, four times the cost: about 13 to 20 s per solve.
   Envelope memory below 0.2 GB. Solves per element: 3 meshes x (D, T, negative control). Each element in its own test:
   Tet10 about 1 to 1.5 min, the others under 30 s in release. Error integration: at most 24 576 elements x 125 points, under
   a second. **Heavy record (H1, ignored):** a second family one level finer, case D only: Tet4 and Hex8 on `n = 6, 12, 24`,
   Tet10 on `n = 3, 6, 12` (each halving h), finest `23^3 x 3 = 36 501` free unknowns: Hex8 and Tet4 about 30 to 70 s each,
   Tet10 1 to 5 min, envelope `<= 1.5 GB` (24.3 GB free at the time of writing). One refinement more (`89 373` unknowns,
   Tet10 an estimated 35 min and 5 GB) is not reasonable on the shared machine and is not run.
10. **Exact reproduction (X3).** A field in the element's space is reproduced to rounding when its loads are integrated
    exactly: Tet4 and Hex8 a linear field (`b = 0`, constant traction), Tet10 a complete quadratic field (constant `b`,
    exact with `tet_14`; linear traction, times quadratic face functions degree 3, exact with the face rule). On `n = 2`,
    both cases: relative L2 and energy errors `~ 1e-14` (patch-test rounding of ADR 0275, 4e-16 to 6.4e-12); limit 1e-9.
    This checks the error code, the prescribed values, the traction helper and the body force path together.
11. **Traction helper (X1).** For a uniform traction `t0 = (0.3, -0.5, 0.7)` on `xmax`, `ymax`, `zmax`, the helper's nodal
    vector and `nodal_load_vector` of `Load::traction(set, t0)` sum the same products `w N_a t |x_u x x_v|` in the same face
    and point order: difference 0 to `~ 1e-16` of the largest entry; limit `1e-14` of the largest entry, n = 2, all three
    elements.

## Design

### D1. The field (test code)

A struct with the tables of arithmetic 1: `u(x)`, `grad(x)` (`[[d_m u_i]]`), `hessian(x)` (`d_m d_n u_i`), `strain(x)`
(Voigt, engineering shear), `stress(x)` (`D eps`), `body_force(x, lambda_sign)` with `lambda_sign = +1` the true force and
`-1` the negative control (arithmetic 3, 6). The polynomial fields of X3 implement the same small trait (`u`, `grad`, `b`).

### D2. Model, loads and solve

`builders::hex_box` or `tet_box` (then `upgrade_to_tet10`) on `[0, 1]^3`, `n` cells per edge; `Material::isotropic_elastic(1,
0.3, 0, 0)` on the element set `all`; no constraint or load in the model itself. Prescribed: the union of the face node sets
of the case, ascending, three components each, `Prescribed::new(node, c, u_c(x_node))`. Forces: `body_force_loads(model, b)`
plus, in case T, the traction helper (finding 2) on `xmax`, `ymax`, `zmax`, summed per node into one `NodalForceAt` list.
`solve_static_with(model, prescribed, forces, StaticOptions::default())` (the direct solver, the default).

### D3. Error norms

Per element, the error rule of arithmetic 8 (`Element::point` at each point; `u_h = sum N_a u_a`, `grad u_h = sum u_a (x)
grad N_a`), the sums of arithmetic 7 in element order then point order.

### D4. Tests in `crates/rcfd-fem/tests/sb_mms.rs`

| Test | Content | Ignored |
|---|---|---|
| `f1_f2_field_derivatives_and_body_force` | F1, F2 | no |
| `x1_traction_helper_against_uniform_traction` | X1 | no |
| `x2_error_rules_integrate_monomials` | X2 | no |
| `x3_polynomial_fields_reproduced` | X3 | no |
| `a_tet4_orders_cases_d_and_t_and_negative_control` | A1 to A3 Tet4 (D and T), N1 Tet4 | no |
| `a_hex8_orders_cases_d_and_t_and_negative_control` | the same for Hex8 | no |
| `a_tet10_orders_cases_d_and_t_and_negative_control` | the same for Tet10 | no |
| `h1_heavy_finer_family_case_d` | H1 | yes (heavy record) |

Every measured value is printed with its limit; a test fails at its end if any of its limits is missed.

## Checks

Seed `0xF378_0279` (SplitMix64, as in `sb_loads.rs`). K is Tet4, Hex8, Tet10; meshes as arithmetic 9.

| Check | Clause | Quantity | Limit | Estimate |
|---|---|---|---|---|
| F2 | c1, c2 | body force against central differences of the stress, 200 points, relative to max `|b|` | `<= 1e-6` | `<= 2e-8` |
| A1-D, A1-T | c3, c4, c5, c6 | L2 order, finest pair, per K and case | `>= 1.8` (Tet4, Hex8), `>= 2.7` (Tet10) | 1.95 to 2.0; 2.85 to 3.0 |
| A2-D, A2-T | c7 | energy-norm order, finest pair, per K and case | `>= 0.9`, `>= 1.8` | 0.97 to 1.0; 1.9 to 2.0 |
| A3-D, A3-T | c8 | `e_1 > e_2 > e_3`, both norms, per K and case | strictly decreasing | ratios about 4 / 8 (L2), 2 / 4 (energy) |
| N1 | c9 | case D with `b_neg`: L2 and energy orders, finest pair, per K | `< 0.5` each | `< 0.1` |
| F1 | extension | gradient against central differences of u, relative to max `|grad u|` | `<= 1e-6` | `<= 2e-8` |
| X1 | extension | traction helper minus `Load::traction` for uniform `t0`, relative to max entry, per K | `<= 1e-14` | 0 to 1e-16 |
| X2 | extension | error rules: monomials of total degree `<= 7` (tet), `<= 9` per variable (cube), absolute | `<= 1e-14` | `~ 1e-15` |
| X3 | extension | polynomial field in the space, n = 2, D and T, relative L2 and energy errors, per K | `<= 1e-9` | `~ 1e-14` |
| H1 | extension (heavy) | finer family (6, 12, 24 / 3, 6, 12), case D: A1, A2, A3 with the same limits; total wall time | as A1 to A3; time recorded (budget 1800 s) | as A; 2 to 7 min |

Wall times of the fast tests are recorded, not limited (rule 7 asks for `#[ignore]` above about two minutes; if a fast
test measures more than 120 s in release that is reported, and a follow-up moves it, without changing its limits).

### Negative controls

N1 above (from the acceptance). In addition, the mutations below are run once each on a scratch copy.

### Planned mutations (scratch copy, each run once)

| Id | Where | Change | Expected to catch it |
|---|---|---|---|
| M1 | test, body force | the `mu Laplace u` term with `2 mu` | F2 (and A1, A2 on every K) |
| M2 | test, traction | `t = -sigma n` | A1-T, A2-T (error floor) |
| M3 | test, prescribed values | Tet10 mid-edge boundary nodes take the mean of their two corner values instead of `u(x_mid)` | A1 Tet10 (boundary error `O(h^2)`: L2 order about 2), A2 Tet10 |
| M4 | library `loads.rs` | `body_rule` returns the stiffness rule (`tet_1`, `gauss_2`, `tet_4`) | **may survive**: by arithmetic 5 Tet4 (`tet_1`, degree 1 = `2p - 1`) and Tet10 (`tet_4`, degree 2 = `2p - 2`) stay energy-optimal; Tet10 L2 could fall below 3. Recorded either way |
| M5 | library `element.rs` | Hex8 stiffness rule `gauss_1` | A Hex8 (hourglass modes: a refused singular stiffness or an error floor) |
| M6 | test, body force | `lambda`, `mu` of the body force from `nu = 0.25` while the material keeps 0.3 | A1, A2 (error floor) on every K |

## Decision rule

**done**: F2, A1 to A3 for every element and both cases, and N1 met; every mutation except M4 caught (M4 recorded either
way). A miss among the extensions (F1, X1 to X3, H1) is a `*_known_miss` record with a follow-up item and does not alone block
done, unless its diagnosis shows the acceptance checks rest on a defect (then partial). **partial**: a miss or an unrun check
among F2, A1 to A3, N1, or an uncaught mutation other than M4 (after checking that the test covers it). **todo**: the test
file does not build.

## Limits (what is not covered)

- Straight-sided elements on a box only: no curved Tet10, no distorted Hex8 (where the full-integration Hex8 stiffness is not
  exact and its orders could differ); the orders are those of structured Kuhn and hexahedral meshes.
- One material, `nu = 0.3`; no near-incompressible case (Hex8 locking is not exercised).
- Errors against the exact field at the points of a high-degree rule; no recovered (nodal or patch) stress is checked, and the
  library's `recover` module is not used (its points are those of the stiffness rule).
- The direct solver only; no iterative solve (ADR 0282).
- The heavy record stops at 36 501 unknowns per solve; finer meshes need the iterative route.
- The traction helper is test code; a function-valued traction in the library is not added (a follow-up if the maintainer
  wants one for later verification work).

## The planned stub, as written

# ADR 0279: Verification by a manufactured solution: observed order of displacement and stress for the three elements

Status: **PLANNED 2026-11-12** (layout by the basic-tier plan, [ADR 0270](0270-structural-solver-basic-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are
the acceptance draft that the registration turns into checks.

Work package SB-9 of the structural solver, basic tier. Size M.

## Goal

A smooth displacement field on the unit cube, its body force from the analytic divergence of the stress, and the observed order
of convergence of the displacement error (L2) and of the stress error (energy norm) under uniform refinement, for each element.
Tests only: no library file is edited; a defect found in a library file is reported, not patched here.

## Dependencies

ADR 0276, ADR 0277; existing roadmap items: none.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. Manufactured solution: a smooth trigonometric displacement
field on the unit cube with nu = 0.3, the body force from the analytic divergence of the stress (checked against finite
differences within 1e-6), displacements prescribed on all faces, and a second case with tractions on three faces. On at least
three meshes, each halving h, the observed order of the L2 displacement error over the finest pair is >= 1.8 for Tet4 and Hex8
and >= 2.7 for Tet10, the observed order of the energy-norm error is >= 0.9 and >= 1.8, and both errors fall monotonically.
Negative control: a body force with the sign of the lambda term reversed gives an order below 0.5.

## Reference to check against

The method of manufactured solutions (Roache, Verification and Validation in Computational Science and Engineering, 1998; from
memory); a-priori estimates of the finite element method: order p + 1 in L2 and p in the energy norm for complete polynomials of
degree p (Strang and Fix, An Analysis of the Finite Element Method, 1973).

## Files

`crates/rcfd-fem/tests/sb_mms.rs` only.

## What could make it miss

Meshes small enough for the direct solver (about 1.5e4 unknowns per run) may still be pre-asymptotic for Tet10; the error norms
need a quadrature of higher degree than the stiffness, or the measured order is polluted.
