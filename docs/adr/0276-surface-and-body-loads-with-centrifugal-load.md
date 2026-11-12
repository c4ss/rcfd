# ADR 0276: Surface and body loads: traction, pressure, gravity, centrifugal load of a rotating part, body force field

Status: **REGISTERED 2026-11-12, written before any code of F-375 and before any run of its checks; planned by ADR 0270**.
Not yet accepted by the maintainer. Before this text nothing of F-375 was written, compiled or run: the code and documents
under "What was read" were read, and the arithmetic under "Arithmetic before this registration" was done by hand; no script
and no library code was run. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Work package SB-6 of the structural solver, basic tier ([ADR 0270](0270-structural-solver-basic-tier-plan.md), section 3, the
entry for ADR 0276). Roadmap item F-375 (depends on F-374, done). Size M. Number 0276, fixed by the plan; the file name of
the planned stub is kept and the stub's text is kept unchanged at the end ("The planned stub, as written"). Branch
`f375-surface-body-loads` from main 15ef52dd, worktree `w49i-work/f375`.

House rules (`w49i-work/AGENT_RULES.md`):

- limits are fixed here, before the code and the runs, and are not changed after a run; a missed limit becomes an ignored
  `*_known_miss` test with the limit intact and the number recorded, diagnosed, not tuned away; a defective set-up gets a
  corrected test by an amendment written and committed before the corrected run, the old test stays as an ignored record;
- each registered run is made once; nothing is re-run to pick a result;
- defaults bit-identical: only `crates/rcfd-fem/src/loads.rs` is edited (its refusals replaced, helpers added) and one test
  file `crates/rcfd-fem/tests/sb_loads.rs` is added; no file outside `rcfd-fem`, no shared solver file, no golden, no hash
  table is touched; a model with nodal forces only gives the bits it gave before (the nodal-force arm is not changed);
- no new dependency, no unsafe code, the graph stays acyclic;
- clean-room: everything below is derived here from first principles (the principle of virtual work, the isoparametric map,
  Gauss-Legendre quadrature on an interval, one-dimensional equilibrium, the equilibrium of a rotating annulus); books are
  cited from memory for orientation only.

## What F-375 asks

The acceptance of `docs/roadmap.json`, binding:

> Every limit not stated here is registered in the ADR before the run. Consistent nodal loads for surface traction, pressure
> (positive pressing on the solid), gravity, the centrifugal load rho omega^2 r_perp about a given axis, and a body force field,
> for Tet4, Hex8 and Tet10. Resultants on a box: a uniform pressure on the closed surface gives zero force and moment within
> 1e-12 p A L; the traction resultant is t A, gravity rho V g and the centrifugal resultant rho omega^2 V r_c (r_c the
> centroid's distance from the axis), each within 1e-12. Closed forms with nu = 0, displacements only: hanging bar, u(x) = rho g
> (L x - x^2/2)/E: Tet10 nodal error <= 1e-9 relative, Hex8 and Tet4 tip displacement within 1 percent at 8 elements along;
> rotating radial bar clamped on the axis: tip displacement rho omega^2 L^3/(3 E) within 1 percent (Tet10 at 8, Hex8 at 16
> elements along); thin rotating ring (thickness/radius 0.02): radial displacement rho omega^2 r^3/E within 2 percent.

Clauses: (c1) consistent nodal loads for traction, (c2) pressure positive pressing on the solid, (c3) gravity, (c4) the
centrifugal load about a given axis, (c5) a body force field, each for Tet4, Hex8 and Tet10; (c6) a uniform pressure on a
closed box surface: zero force within 1e-12 p A and zero moment within 1e-12 p A L; (c7) traction resultant t A, (c8)
gravity resultant rho V g, (c9) centrifugal resultant rho omega^2 V r_c, each within 1e-12; (c10) hanging bar: Tet10 nodal
error <= 1e-9 relative, (c11) Hex8 and Tet4 tip within 1 percent at 8 elements along; (c12) rotating radial bar: tip within 1
percent, Tet10 at 8 and Hex8 at 16 elements along; (c13) thin rotating ring: radial displacement within 2 percent.

The brief adds: leave the `Load::Temperature` arm to ADR 0281; derive each closed form checked (hanging bar, rotating bar,
thin ring) from first principles and say why the stated element counts should meet the limits; say which mesh builder the
ring uses. The plan (ADR 0270 section 2.4) adds: a function-valued body force exists only as a library helper (for the
manufactured solution of ADR 0279); loads stay plain data.

## What was read (main 15ef52dd, read only)

1. `AGENT_RULES.md`; ADR 0270 sections 2.2 (conventions: pressure positive pressing on the solid, surface-set normals out of
   the solid, determinism by element index, located refusals), 2.3 (`loads` is 0275's and 0276's), 2.4 (`Load` plain data;
   the function-valued body force a library helper), the entries 0276, 0279, 0281, 0288 of section 3, the wave table (0281
   edits `loads.rs` after this package) and the decision record; this ADR's stub; ADR 0271 (local faces counter-clockwise
   seen from outside, the Tet10 face order corners then mid-edge nodes, the builders and their surface sets); ADR 0273 (the
   rules: Tet4 stiffness `tet_1`, mass `tet_4`; Hex8 `gauss_2`, `gauss_3`; Tet10 `tet_4`, `tet_14` of degree 5; its note
   that body loads are sums over `Element::integration_points` and that the face functions of Tri3, Quad4, Tri6 belong to
   `loads.rs`); ADR 0275 in full with its amendment and addendum (D4: `nodal_load_vector(model, extra)` is the one function
   the solve calls; its note for F-375).
2. Code: `crates/rcfd-fem/src/{loads.rs, model.rs, mesh.rs, element.rs, quadrature.rs, builders.rs, solve.rs, lib.rs}`, the
   public signatures of `constraint.rs`, `assemble.rs`, `error.rs`; `crates/rcfd-fem/tests/sb_static.rs` (helpers `Rng`,
   `rebuild`, `box_of`; the test `e1_to_e4_free_node_conflicts_loads_element_errors`), the load checks of `sb_model.rs`;
   `rcfd_core::vec3`.
3. **Not read:** the bodies of the books cited; any finite-element code outside this repository.

Findings that shape the design:

1. **A conflict with an existing test.** `sb_static.rs`, test `e1_to_e4_free_node_conflicts_loads_element_errors`, check E3
   of ADR 0275, asserts that a model with `Load::gravity` is refused with `UnsupportedLoad { load: "Gravity" }`. It pinned the
   refusal that this package replaces, so it fails by construction once gravity is supported, and the check E4 after it in
   the same function is then not reached. Existing tests are not edited (house rule 2; the brief). The test is therefore left
   as it is, and the one-line replacement that keeps E3's purpose (a load that is still unsupported, `Load::temperature`,
   refused naming "Temperature" and ADR 0281) is proposed to the lead as the remaining step needing approval; it is run once
   on a scratch copy and the result recorded (check X4). No other existing test asserts a refusal of a load this package adds
   (`sb_model` only adds loads to a model; it does not solve).
2. `SolidMesh::face_nodes` gives a face's global nodes in the local face order (corners counter-clockwise from outside, then
   for Tri6 the mid-edge nodes of the edges (c0 c1), (c1 c2), (c2 c0)); `face_area_vector` integrates `x_u x x_v`, so the
   right-hand normal of that order points out of the element. The surface sets of the builders are boundary faces whose
   normals point out of the solid.
3. Materials are per element set (`SolidModel::material_indices` refuses an element without material). Body loads need the
   density; surface loads do not. `nodal_load_vector` today does not require materials (nodal forces); that stays so for a
   model without body loads.
4. `Element::mass_rule` is `tet_4` (degree 2) for Tet4, `gauss_3` (degree 5 per variable) for Hex8 and `tet_14` (degree 5)
   for Tet10; these are the rules of the body loads (arithmetic 2).

## Arithmetic before this registration (disclosed, by hand)

1. **Consistent loads (virtual work).** With `u_h = sum_a N_a u_a`, the virtual work of a surface traction `t` and a body
   force `b` is `sum_a delta u_a . (int_S N_a t dS + int_V N_a b dV)`, so the nodal load of node a is `f_a = int_S N_a t dS +
   int_V N_a b dV`. A pressure p (positive pressing on the solid) is the traction `t = -p n`, n the outward unit normal; with a
   face map `x(u, v)`, `n dS = (x_u x x_v) du dv`, so `f_a = -p int N_a (x_u x x_v) du dv`: a polynomial integrand on every
   face of the three kinds (no square root), integrated exactly by a rule of high enough degree. A traction uses `dS = |x_u x
   x_v| du dv`, polynomial on flat faces (constant `|x_u x x_v|` for Tri3, for Tri6 with straight edges and for a parallelogram
   Quad4).
2. **Degrees and rules.** Face functions: Tri3 `(1 - u - v, u, v)`; Quad4 on `[-1, 1]^2` `(1 + u u_a)(1 + v v_a)/4` with
   nodes `(-,-), (+,-), (+,+), (-,+)`; Tri6 `L_i (2 L_i - 1)` at the corners and `4 L_i L_j` at the mid-edge nodes (as
   `mesh.rs`). Pressure integrand degree: Tri3 1; Quad4 `N_a` degree 1 per variable and `x_u x x_v` degree 1 per variable
   (`x_u` depends on v only, `x_v` on u only), total 2 per variable; Tri6 with curved edges `N_a` 2, `x_u x x_v` 2, total 4.
   **Face rules:** quadrilateral 3 x 3 Gauss-Legendre (exact to degree 5 per variable); triangle the collapsed product
   (Duffy) rule `u = s (1 - r)`, `v = s r`, `du dv = s ds dr`, with 3-point Gauss-Legendre on `[0, 1]` in s and in r: a
   polynomial of total degree d in (u, v) becomes degree `<= d + 1` in s and `<= d` in r, so the 9-point rule is exact for `d
   <= 4` (3 points are exact to degree 5); weights sum to `int s ds dr = 1/2`, the area of the reference triangle. Both are
   exact for every pressure integrand above and for tractions on flat faces. The 3-point rule on `[-1, 1]`: points `0, +-
   sqrt(3/5)`, weights `8/9, 5/9` (the roots of the Legendre polynomial `P3 = (5 x^3 - 3 x)/2`; the weights from `int 1 = 2`
   and `int x^2 = 2/3`); mapped to `[0, 1]` by `s = (1 + x)/2`, weights halved. **Body rule:** the element's mass rule. Gravity
   `b = rho g` (constant): integrand `N_a det J`: Tet4 1, straight Tet10 2, Hex8 (`det J` degree <= 2 per variable on a
   trilinear map) 3 per variable: exact. Centrifugal `b = rho omega^2 P (x - o)`, `P = I - a a^T`, a the unit axis: linear in
   `x = sum N_b x_b`, integrand `N_a N_b det J`: Tet4 2 (`tet_4` exact), straight Tet10 4 (`tet_14` exact), Hex8 4 per
   variable (`gauss_3` exact). A nodal body force field interpolated with the element's own functions, `b = sum_b N_b b_b`,
   has the same integrand (the mass-matrix integrand): exact on the same elements. On a curved Tet10 `det J` has degree 3 and
   none of these is exact (listed under Limits).
3. **Resultants.** `sum_a N_a = 1` on every element and face, so with exact quadrature `sum_a f_a = int_S t dS + int_V b dV`
   and, the map being isoparametric (`x = sum_a N_a x_a`), `sum_a x_a x f_a = int x x t dS + int x x b dV`. Closed surface
   under uniform pressure: `int n dS = 0` and `int x x n dS = 0` (divergence theorem: `int_V grad 1 = 0` and `int_V curl x =
   0`), so the resultant force and moment vanish to rounding. Traction on one box face: `t A_f`. Gravity: `rho V g`.
   Centrifugal: `rho omega^2 P (int x dV - o V) = rho omega^2 V P (x_c - o)`, the vector from the axis to the centroid, of
   length r_c. A linear nodal field `b = b0 + G (x - x_c)` (x_c the centroid): `V b0`. The boxes keep flat faces when their
   interior nodes are moved, so V, A_f and x_c are those of the box; the sums are of `~ 1e3` terms of one sign or of mixed
   sign of size `|f|`: rounding `~ 1e-15` to `1e-14` of the scale, margin `>= 100` to 1e-12.
4. **Consistent loads of flat faces, closed forms (for the distribution checks).** Tri3: `int L_i dA = A/3`. Quad4 on a
   parallelogram: `int N_a dA = A/4`. Tri6 with straight edges: corners `int L_i (2 L_i - 1) dA = 2 (2 A/12) - A/3 = 0`
   (`int L_i^2 dA = 2 A/4! x 2! = A/6`), mid-edge nodes `int 4 L_i L_j dA = 4 A/12 = A/3`. So a uniform traction t on a face
   of area A gives `t A/3`, `t A/4`, and `0` (corners) with `t A/3` (mid-edge nodes); a pressure gives the same with `t = -p
   n`. **Centrifugal load of a Tet4:** `int L_a L_b dV = V (1 + delta_ab)/20`, so `int N_a x dV = V (sum_b x_b + x_a)/20` and
   `f_a = rho omega^2 P (V (sum_b x_b + x_a)/20 - V o/4)`. A one-point rule would give `V P (x_c - o)/4` instead (detected).
5. **Hanging bar.** Bar `0 <= x <= L`, cross-section A, gravity `g e_x` (hanging from x = 0 in the direction of +x), clamped
   at x = 0. One-dimensional equilibrium `d sigma/dx + rho g = 0`, free end `sigma(L) = 0`: `sigma = rho g (L - x)`; Hooke
   `sigma = E du/dx`, `u(0) = 0`: `u(x) = rho g (L x - x^2/2)/E`, tip `rho g L^2/(2 E)`. **Three-dimensional exactness with nu
   = 0:** `u = (u(x), 0, 0)` has the only strain `eps_xx = rho g (L - x)/E`; with `nu = 0`, `lambda = 0`, `mu = E/2`, the only
   stress is `sigma_xx = E eps_xx`; `div sigma + b = 0` holds, the lateral faces and the end x = L are free of traction, and
   the clamp `u = 0` at x = 0 agrees with it. So it is the exact solution of the 3-D problem with a fully clamped root.
   **Tet10:** the field is quadratic, so it lies in the Tet10 space of a straight-sided mesh; the stiffness (degree 2, `tet_4`)
   and the load (arithmetic 2) are integrated exactly, so the Galerkin solution is the exact one, to rounding. The direct
   solve of `~ 2e3` unknowns of condition `~ 1e5` gives nodal errors `~ 1e-12` relative: margin `~ 1e3` to 1e-9. **Hex8:** the
   1-D field `U = (u1(x), 0, 0)`, u1 the piecewise linear 1-D finite-element solution on the 8 element planes, lies in the
   Hex8 space of the box. For any test function v of the space, `a(U, v) = int E u1' dv_x/dx dV = E A int u1' wbar' dx` with
   `wbar(x)` the mean of `v_x` over the cross-section (trilinear v: wbar piecewise linear in x), and `l(v) = rho g int v_x dV
   = rho g A int wbar dx` (exact quadrature). That is the 1-D Galerkin equation, which u1 satisfies, so U is the Hex8 solution.
   The 1-D linear elements with exact consistent loads are nodally exact (the Green's function of `-E A u'' = delta(x - x_j)`
   is piecewise linear with a kink at x_j, so it lies in the space), so the Hex8 tip displacement is exact to rounding
   (estimate `<= 1e-12`), limit 1 percent. **Tet4 (Kuhn split):** the same `U` lies in the Tet4 space (u1 is linear in x within
   each slab between grid planes). For a node p on an interior grid plane, `int phi_p dV` over the two slabs it touches equals
   `h int_plane phi_p dA` (counting the tetrahedra: within a cell, a corner with no, one, two or three raised coordinates lies in
   6, 2, 2, 6 of the six Kuhn tetrahedra; for p as corner (0, a, b) of the cell above and (1, a, b) of the cell below the
   counts add to 8 for (a, b) = (0, 0) or (1, 1) and to 4 otherwise, and on the plane p is in 2 or 1 of the two triangles of
   that column's square, so both sides are `V_c/3` or `V_c/6` per column); with `a(U, phi_p) = E (u1'_{j-1} - u1'_j)
   int_plane phi_p dA` (divergence theorem in each slab) the rows of the interior planes are satisfied exactly. Only the
   rows of the tip plane carry a residual, `rho g ((h/2) int_plane phi_p dA - int_slab phi_p dV)`, which sums to zero over the
   plane (self-equilibrated) and is `~ rho g V_c/12` per node. Its effect on the mean tip displacement is `<~ rho g h_y h_z/(12
   E)`, relative to `rho g L^2/(2 E)`: `h_y h_z/(6 L^2) ~ 4e-4` for the registered bar: limit 1 percent, margin `>~ 20`.
6. **Rotating radial bar.** Bar along x from the axis (x = 0) to L, rotating about the z axis through the origin at omega;
   body force `rho omega^2 (x, y, 0)`. Axial part: `E u1'' + rho omega^2 x = 0`, `u1(0) = 0`, `u1'(L) = 0`: `E u1' = rho
   omega^2 (L^2 - x^2)/2`, `u1 = rho omega^2 (L^2 x/2 - x^3/6)/E`, tip `rho omega^2 L^3/(3 E)`. Lateral part: on a section
   `|y| <= b/2` free at its sides, `sigma_yy = rho omega^2 (b^2/4 - y^2)/2`, `u_y = rho omega^2 (b^2 y/4 - y^3/3)/(2 E)` with
   `nu = 0`; `(u1(x), u_y(y), 0)` has `gamma_xy = 0` and is the exact 3-D solution for a root that holds `u_x` and lets `u_y`
   slide. The acceptance's full clamp also holds `u_y` at the root, a constraint of size `rho omega^2 b^3/(24 E)` against the
   tip `rho omega^2 L^3/(3 E)`: ratio `(b/L)^3/8 ~ 1.3e-4` for `b/L = 0.1`; by Saint-Venant its effect on the tip is smaller
   still. **Hex8 at 16:** the axial load `rho omega^2 x` depends on x only, so the argument of arithmetic 5 makes the Hex8
   response to it the 1-D linear solution, nodally exact; the lateral load and the root clamp add `<~ 1e-4`: estimate `<~ 1e-4`,
   limit 1 percent. **Tet10 at 8:** u1 is cubic and not in the space; the 1-D quadratic elements are nodally exact (same Green's
   function argument), and the 3-D deviation is the cross-section redistribution near the ends as in arithmetic 5 (`<~ 1e-3`)
   plus the clamp effect: estimate `<~ 2e-3`, limit 1 percent.
7. **Thin rotating ring.** Annulus `a <= r <= b` rotating about its axis z, `nu = 0`. With `nu = 0` the in-plane stresses do
   not depend on `eps_zz`, and `u_z = 0` makes `sigma_zz = 0` on the free top and bottom: the plane solution is the 3-D one for
   any height. Radial equilibrium `d sigma_r/dr + (sigma_r - sigma_t)/r + rho omega^2 r = 0` with `sigma_r = E u'`, `sigma_t
   = E u/r`: `u'' + u'/r - u/r^2 = -rho omega^2 r/E`; particular `u = -rho omega^2 r^3/(8 E)` (`c r^3` gives `(6 + 3 - 1) c r
   = 8 c r`); `u = A r + B/r - rho omega^2 r^3/(8 E)`; `sigma_r = 0` at a and b means `u' = 0` there: with `k = 3 rho omega^2/
   (8 E)`, `B = k a^2 b^2`, `A = k (a^2 + b^2)`. At the mean radius R with `a, b = R (1 -+ tau/2)`: `u(R) = k R^3 (a^2 + b^2 +
   a^2 b^2 - 1/3)` (in units of R) `= k R^3 (8/3 + tau^4/16) = rho omega^2 R^3/E (1 + 3 tau^4/128)`: for `tau = 0.02` the thin
   formula is exact at r = R to 4e-9. Away from R the thin formula `rho omega^2 r^3/E` and the exact u differ (u is nearly
   constant across the thickness, `u' = 0` at both faces, while `r^3` changes by `3 tau = 6` percent), so the check is made
   at the nodes of the mid-surface r = R. **Mesh:** a quarter ring `0 <= theta <= pi/2` with symmetry rollers (no seam to
   merge): the builders `hex_box` and `tet_box` on the parameter box `(rho, theta, z)` in `[R - t/2, R + t/2] x [0, pi/2] x
   [0, H]` and the map `(rho cos theta, rho sin theta, z)`, whose Jacobian `(e_r, rho e_t, e_z)` has determinant `rho > 0` and
   keeps the orientation; for Tet10 the parameter Tet4 box is upgraded first and then mapped, so the mid-edge nodes lie on the
   circles (curved edges). Rollers: `u_y = 0` on `theta = 0` (the plane y = 0), `u_x = 0` on `theta = pi/2` (the plane x = 0;
   the end coordinates snapped to `cos = 0, sin = 1` exactly), `u_z = 0` on `z = 0`: all agree with the exact field (radial,
   `u_z = 0`), and they fix the six rigid motions (`u_x = 0` on a plane of varying y and z removes `a_x`, `w_y`, `w_z`; `u_y =
   0` on a plane of varying x removes `a_y` and `w_z`; `u_z = 0` on a plane of varying x and y removes `a_z`, `w_x`, `w_y`).
   **Discretisation error, straight edges (Hex8, Tet4):** the mid-surface is a polygon with `Delta theta = (pi/2)/16`. A
   polygon of chords loaded by the centrifugal force: each chord carries `rho omega^2 A_s R^2 sin(Delta theta)` toward its
   mid-angle (its length `2 R sin(Delta theta/2)` times its mid-radius `R cos(Delta theta/2)`), half to each node, radial
   component `cos(Delta theta/2)`; the node's equilibrium `2 T sin(Delta theta/2) = F` gives the chord force `T = rho omega^2
   A_s R^2 cos^2(Delta theta/2)` and the radial node displacement `R T/(E A_s) = rho omega^2 R^3/E cos^2(Delta theta/2)`:
   `-0.24` percent for `Delta theta = pi/32` (it would be `-0.96` percent at 8 per quarter). Estimate `|error| <~ 0.5`
   percent, limit 2. **Tet10 (curved edges):** the quadratic arcs approximate the circle to `O(Delta theta^4)`, estimate `<~
   0.1` percent. The section `t x H = 0.02 R x 0.02 R` with 2 elements through the thickness (so that the mid-surface has
   nodes) and 1 in height; aspect of a cell `~ 0.1/0.01 = 10` in a membrane state, which the three kinds represent.
8. **Sizes.** Hanging bar Tet10 `[8, 2, 2]`: `9 x 3 x 3 = 81` corners plus mid-edge nodes, `~ 600` nodes, `~ 1.8e3` unknowns;
   ring Tet10 `[2, 16, 1]`: `~ 600` nodes. All solves are below a second in release.

## Design

### D1. The load vector (`loads.rs`)

`nodal_load_vector(model, extra)` keeps its signature and its nodal-force arm unchanged (bits of nodal-force-only models
unchanged). The loads are processed in the model's order, and within a load the faces or elements in their set or element
order, node by node and component by component, sequentially: the vector is a fixed sum and does not depend on threads.

* `Load::Traction { set, traction }`: for each face of the surface set, `f_a += sum_q w_q N_a(q) |x_u x x_v|(q) t`.
* `Load::Pressure { set, pressure }`: `f_a += sum_q w_q N_a(q) (-p) (x_u x x_v)(q)` (positive pressing on the solid).
* `Load::Gravity { acceleration }`: `f_a += sum_q w_q det J_q N_a(q) rho_e g` over every element, with the element's mass
  rule and the density of its material.
* `Load::Centrifugal { axis, origin, omega }`: the same with `b = rho_e omega^2 P (x_q - o)`, `P = I - a a^T`, `a = axis /
  |axis|`.
* `Load::BodyForceNodal { values }`: the same with `b = sum_b N_b(q) values[node b]` (per unit volume, N/m^3, no density).
* `Load::Temperature`: refused as before, `UnsupportedLoad { load: "Temperature", package: "ADR 0281" }`.

Materials are looked up (through `SolidModel::material_indices`, which refuses an element without material) only when a
gravity or centrifugal load is present. An element error of the isoparametric map (`det J <= 0`) is returned as
`FemError::Element { element, source }` as the assembly does.

### D2. Public helpers (`loads.rs`)

* `face_shape_functions(n_nodes, [u, v]) -> Result<(Vec<f64>, Vec<[f64; 2]>)>`: values and `(d/du, d/dv)` for Tri3 (3),
  Quad4 (4) and Tri6 (6) in the face orders of ADR 0271; another count is refused (`InvalidValue { key: "face nodes" }`).
* `face_rule(n_nodes) -> Result<Vec<([f64; 2], f64)>>`: the face rules of arithmetic 2 (9 points each).
* `body_force_loads(model, f: impl Fn(Vec3) -> Vec3) -> Result<Vec<NodalForceAt>>`: the consistent nodal load of a body
  force per unit volume given by a function of position (the helper of ADR 0270 section 2.4, for ADR 0279), one entry per
  node in node order, with the mass rule; a non-finite value of `f` is refused naming the element (`InvalidValue { key:
  "body force" }` inside `Element`). It is passed to `solve_static_with` as the per-node forces.

Nothing else is public; `element.rs`, `mesh.rs`, `model.rs` and `solve.rs` are not edited.

### D3. Test set-up (`tests/sb_loads.rs`)

* Generator: SplitMix64 as in ADR 0273 D6 and 0275 D8, **seed `0xF375_0276`**, stream `SEED + 10 + k` for the perturbation
  of the resultant box of kind k (Tet4 0, Hex8 1, Tet10 2).
* **Resultant box:** origin `(0.1, -0.2, 0.3)`, lengths `(1.2, 1.0, 0.8)` (V = 0.96, total area 5.44, diagonal D = sqrt(3.08)),
  divisions `[4, 3, 3]`, interior grid nodes moved by `0.1 h_k (2U - 1)` per coordinate (ADR 0275 arithmetic 6: no
  inversion); Tet10 by `upgrade_to_tet10` of the moved Tet4 box (straight edges). Material `E = 2e11 Pa`, `nu = 0.3`, `rho =
  7850 kg/m^3`. Loads: pressure `p = 2.5e5 Pa`; traction `t = (1e5, -2e5, 3e5) Pa`; gravity `g = (0.3, -9.81, 1.2) m/s^2`;
  centrifugal axis `(1, 2, 2)` (length 3, not a unit vector), origin `(-0.5, 0.3, 0.1)`, `omega = 300 rad/s`; linear field
  `b = b0 + G (x - x_c)`, `b0 = (1e3, -2e3, 5e2) N/m^3`, `G = 1e3 [[1, 0.5, -0.2], [0.3, -1, 0.4], [0.2, 0.1, 0.8]] N/m^4`.
  Each load alone in its own model, through `Load` and `nodal_load_vector(model, &[])`.
* **Closed forms:** `E = 2e11 Pa`, `nu = 0`, `rho = 7850 kg/m^3`. Bars: `L = 1 m`, section `[-0.05, 0.05]^2` (b = 0.1),
  divisions `[n, 2, 2]`, clamped on `xmin` by three `Constraint::displacement`. Hanging: `g = (9.81, 0, 0)`, n = 8 for the
  three kinds (Tet10 from the Tet4 box); exact tip `rho g L^2/(2 E) = 1.9252e-7 m`. Rotating: axis `(0, 0, 1)` through the origin, `omega =
  300 rad/s`, Tet10 n = 8, Hex8 n = 16 (and Tet4 n = 16, extension); exact tip `rho omega^2 L^3/(3 E) = 1.1775e-3 m`. Tip
  displacement: the mean of `u_x` over the nodes of the node set `xmax`. Ring: `R = 1 m`, `t = H = 0.02 m`, parameter
  divisions `[2, 16, 1]`, three kinds, `omega = 300 rad/s`; rollers by `Constraint::displacement` on the node sets `ymin`
  (`u_y`), `ymax` (`u_x`), `zmin` (`u_z`) of the parameter box; exact `rho omega^2 R^3/E = 3.5325e-3 m`; the radial
  displacement `u . (x, y)/|(x, y)|` at every node whose parameter radius is R.
* Runs: `cargo test -p rcfd-fem --release --test sb_loads -- --nocapture --test-threads=1`, once.

## Registered checks

All in `crates/rcfd-fem/tests/sb_loads.rs`, release profile, one run; every number printed. "Three kinds" means Tet4, Hex8
and Tet10, each checked against the limit.

| Id | Clause | Check | Limit | Estimate behind it |
| --- | --- | --- | --- | --- |
| R1 | c2, c6 | resultant box, three kinds, pressure p on all six surface sets: `|sum f| / (p A_tot)` and `|sum (x_a - x_c) x f_a| / (p A_tot D)`, x_c the box centre | `<= 1e-12` each | arithmetic 3: `~ 1e-15` |
| R2 | c2 | pressure p on `xmax` only: `|sum f - (-p A_xmax e_x)| / (p A_xmax)` (pressing on the solid) | `<= 1e-12` | arithmetic 3 |
| R3 | c1, c7 | traction t on `ymax`: `|sum f - t A_ymax| / (|t| A_ymax)` | `<= 1e-12` | arithmetic 3 |
| R4 | c3, c8 | gravity: `|sum f - rho V g| / (rho V |g|)` | `<= 1e-12` | arithmetic 3 |
| R5 | c4, c9 | centrifugal: `|sum f - rho omega^2 V P (x_c - o)| / (rho omega^2 V r_c)`, and `| |sum f| - rho omega^2 V r_c | / (rho omega^2 V r_c)` | `<= 1e-12` each | arithmetic 3 |
| R6 | c5 | nodal linear body force field: `|sum f - V b0| / (V |b0|)` | `<= 1e-12` | arithmetic 3 |
| D1 | c1 | traction on `ymax`: the library vector against the vector built in the test from `face_nodes`, `face_area` and the closed forms of arithmetic 4: `max |f_lib - f_test| / max |f_test|` | `<= 1e-13` | the same sums, other order: `~ 1e-16` |
| D2 | c2 | pressure on all six sets, likewise with `-p face_area_vector / k` and the Tri6 corner zeros | `<= 1e-13` | as D1 |
| D3 | c4 | Tet4 centrifugal load against the closed form of arithmetic 4, per node | `<= 1e-13` | as D1 |
| C1 | c3, c10 | hanging bar Tet10 `[8, 2, 2]`: `max_nodes |u_h - (u(x), 0, 0)| / u(L)` | `<= 1e-9` | arithmetic 5: `~ 1e-12` |
| C2 | c3, c11 | hanging bar Hex8 and Tet4 `[8, 2, 2]`: `|tip - u(L)| / u(L)` | `<= 1e-2` each | arithmetic 5: Hex8 `~ 1e-12`, Tet4 `<~ 4e-4` |
| C3 | c4, c12 | rotating bar, Tet10 `[8, 2, 2]` and Hex8 `[16, 2, 2]`: `|tip - rho omega^2 L^3/(3 E)| / (rho omega^2 L^3/(3 E))` | `<= 1e-2` each | arithmetic 6: Tet10 `<~ 2e-3`, Hex8 `<~ 1e-4` |
| C4 | c4, c13 | ring, three kinds: `max_mid-surface |u_r - rho omega^2 R^3/E| / (rho omega^2 R^3/E)` | `<= 2e-2` each | arithmetic 7: Hex8, Tet4 `~ 2.4e-3`, Tet10 `<~ 1e-3` |
| B1 | c5 | `Load::body_force_nodal` with `rho g` at every node against `Load::gravity` (resultant box, three kinds): `max |diff| / max |f|` | `<= 1e-14` | the same products, `sum N_b = 1` to rounding |
| B2 | c5 | `body_force_loads(model, x -> rho omega^2 P (x - o))` against `Load::centrifugal` (resultant box, three kinds) | `<= 1e-14` | same points and arithmetic up to the order of a product |
| B3 | c5 | `body_force_loads` of the linear field against `Load::body_force_nodal` of its nodal values (three kinds) | `<= 1e-13` | isoparametric interpolation of a linear field exact |
| E1 | extension | rotating bar Tet4 `[16, 2, 2]`: tip as C3 | `<= 1e-2` | as arithmetic 5, 6: `<~ 1e-3` |
| E2 | extension | ring: `max |u_theta| / (rho omega^2 R^3/E)` and `max |u_z| / (rho omega^2 R^3/E)` over all nodes (record) | `<= 1e-2` each | exact field radial, `u_z = 0` |
| X1 | D1 | a model with `Load::temperature`: refused `UnsupportedLoad { load: "Temperature", package: "ADR 0281" }` | as stated | D1 |
| X2 | D1 | gravity on a model without material: refused `MaterialUnassigned` | as stated | D1 |
| X3 | D2 | `body_force_loads` with a function returning NaN: refused, the message names "body force"; `face_shape_functions(5, ..)` refused | as stated | D2 |
| X4 | finding 1 | the proposed replacement of ADR 0275's E3 (`Load::temperature` instead of `Load::gravity`, asserting `UnsupportedLoad { load: "Temperature", .. }`), run once on a scratch copy of `sb_static.rs` with the rest of that test unchanged: the test passes | passes | D1 |

**Negative controls** (each must hold, else the check is not sensitive):

| Id | Control | Expected |
| --- | --- | --- |
| N1 | C1's measure with a lumped gravity load (each element's `rho V_e g / 10` on each of its ten nodes) passed as per-node forces | `> 1e-4` |
| N2 | R5's measure against the closed form with the distance from the origin point o to the centroid (`x_c - o`, no projection) | `> 1e-3` |
| N3 | C4's measure at the inner-surface nodes (r = R - t/2) against `rho omega^2 r^3/E` of their own radius, Hex8 | `> 1e-2` (arithmetic 7: about 3 percent) |

**Mutations** (on a `git archive` copy of the final tree in `w49i-work/f375-scratch`, own target directory; each applied
alone by exact text replacement, `sb_loads` run once, the file restored and compared byte for byte):

| Id | Mutation | Must fail |
| --- | --- | --- |
| M1 | pressure sign: `+p` instead of `-p` | R2, D2 |
| M2 | Tri6 corner functions `L_i` instead of `L_i (2 L_i - 1)` | D1 or D2 or R1/R3 (Tet10) |
| M3 | centrifugal without the axial projection (`b = rho omega^2 (x - o)`) | R5 |
| M4 | centrifugal projection with the axis not normalised | R5 |
| M5 | body loads with the stiffness rule instead of the mass rule | D3 |
| M6 | gravity without the density | R4 |
| M7 | nodal body force: node a's own value instead of the interpolated field | B3 |
| M8 | triangle face rule without the collapse Jacobian s | R3 or D1 (tetrahedra) |
| M9 | `Load::Temperature` skipped instead of refused | X1 |

**Decision rule.** F-375 is proposed **done** (pending the maintainer's acceptance) if R1 to R6, D1 to D3, C1 to C4 and B1 to
B3 are met for every kind they name (every acceptance clause), N1 to N3 hold and every mutation fails a test. E1, E2 and X1 to
X4 are registered extensions: a miss among them is a `*_known_miss` record and a follow-up and does not alone block done. A
miss or an unrun check among the acceptance checks, a failed negative control or an uncaught mutation (after checking that a
test covers it): **partial**, with the miss named. No crate that builds: **todo**. The failure of ADR 0275's E3 (finding 1) is
by construction and is reported to the lead with its proposed one-line replacement (X4); it does not change the decision, but
the gate fails on it until the lead applies or rejects the replacement.

## Limits (what is not covered)

- Loads are integrated exactly on straight-sided Tet10, on any trilinear Hex8 and on Tet4 (body loads), and for pressure on
  every face; tractions are exact on flat faces only, and body loads on curved Tet10 are not exact (`det J` of degree 3).
- Uniform traction and pressure per surface set only; a pressure varying over a surface is the business of ADR 0286 (face
  forces per node of a fluid surface) and can be given now as per-node forces.
- The centrifugal load is that of the undeformed geometry at constant speed: no spin softening, no stress stiffening, no
  Coriolis or angular-acceleration term (intermediate tier).
- Density per material (per element set); no density field.
- The function body force is a helper returning per-node forces; it is not part of the plain-data model.
- The ring is checked on a quarter with symmetry rollers; the closed forms need `nu = 0`, so the checks do not see the Poisson
  coupling of the loads (the patch test of ADR 0275 and the benchmarks of ADR 0280 do).

## Open points taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. Body loads use the element's mass rule (exact for the loads of the basic tier on straight elements, arithmetic 2), not a
   rule of their own.
2. The face rules are the 3 x 3 Gauss rule on quadrilaterals and the 9-point collapsed Gauss rule on triangles (arithmetic 2),
   derived here, rather than a published symmetric triangle rule recalled from memory.
3. The function-valued body force is `body_force_loads` returning `Vec<NodalForceAt>` for `solve_static_with` (no closure in
   the model, ADR 0270 section 2.4).
4. ADR 0275's E3 is not edited; its replacement is proposed (finding 1). It is a test edit, not a default, golden or hash
   table, but the house rules forbid editing an existing test, so it is the lead's step.

## Addendum 1: results (2026-11-12)

Code at commit f2b29d0c (after the registration 56780530): `crates/rcfd-fem/src/loads.rs` (the refusals of traction,
pressure, gravity, centrifugal and nodal body force replaced; `face_shape_functions`, `face_rule`, `body_force_loads` added;
the nodal-force arm unchanged in its arithmetic, moved from a closure into the private `add_force`) and
`crates/rcfd-fem/tests/sb_loads.rs`. No other file; no shared solver file, golden or hash table. The registered run was made
once, in release, with `cargo test -p rcfd-fem --release --test sb_loads -- --nocapture --test-threads=1` (7 passed, 0.18 s;
log `w49i-work/f375-scratch/run1.log`).

| Id | Limit | Measured (Tet4; Hex8; Tet10) | Met |
| --- | --- | --- | --- |
| R1 closed-surface pressure, force / (p A); moment / (p A L) | `<= 1e-12` each | 1.5e-17 / 3.2e-18; 1.6e-17 / 7.0e-18; 4.4e-17 / 1.3e-17 | yes |
| R2 pressure on `xmax` against `-p A e_x` | `<= 1e-12` | 2.9e-16; 1.7e-16; 5.2e-17 | yes |
| R3 traction against `t A` | `<= 1e-12` | 4.2e-16; 3.7e-16; 4.2e-16 | yes |
| R4 gravity against `rho V g` | `<= 1e-12` | 5.9e-16; 5.9e-16; 9.8e-16 | yes |
| R5 centrifugal against `rho omega^2 V r_c`, vector; magnitude | `<= 1e-12` each | 1.6e-16 / 1.6e-16; 1.6e-16 / 0; 7.0e-16 / 8.2e-16 | yes |
| R6 nodal linear field against `V b0` | `<= 1e-12` | 2.1e-16; 3.7e-16; 6.3e-16 | yes |
| D1 traction per node against the closed forms | `<= 1e-13` | 3.0e-16; 3.0e-16; 9.1e-16 | yes |
| D2 pressure per node against the closed forms | `<= 1e-13` | 4.4e-16; 4.4e-16; 8.7e-16 | yes |
| D3 Tet4 centrifugal per node against the closed form | `<= 1e-13` | 8.1e-16 | yes |
| C1 hanging bar Tet10 `[8, 2, 2]`, max nodal error / u(L) | `<= 1e-9` | 8.7e-13 (tip 2.2e-14) | yes |
| C2 hanging bar tip `[8, 2, 2]`, Tet4; Hex8 | `<= 1e-2` | 2.0e-5; 2.1e-15 | yes |
| C3 rotating bar tip, Tet10 `[8, 2, 2]`; Hex8 `[16, 2, 2]` | `<= 1e-2` | 3.0e-6; 1.1e-15 | yes |
| C4 ring mid-surface `u_r` against `rho omega^2 R^3/E` | `<= 2e-2` | 3.3e-3; 2.0e-3; 8.5e-6 | yes |
| B1 nodal `rho g` against gravity | `<= 1e-14` | 4.2e-16; 2.1e-16; 2.3e-16 | yes |
| B2 function helper against centrifugal | `<= 1e-14` | 0; 0; 0 | yes |
| B3 function helper against nodal linear field | `<= 1e-13` | 3.3e-16; 1.8e-16; 2.5e-16 | yes |
| E1 rotating bar Tet4 `[16, 2, 2]` | `<= 1e-2` | 5.0e-6 | yes |
| E2 ring `max |u_theta|`; `max |u_z|` / exact | `<= 1e-2` each | 3.1e-4 / 3.0e-5; 2.0e-11 / 1.2e-15; 3.6e-6 / 3.8e-8 | yes |
| X1 temperature refused | `UnsupportedLoad { "Temperature", "ADR 0281" }` | "load Temperature is not supported by the static solve yet (it is added by ADR 0281)" | yes |
| X2 gravity without material | `MaterialUnassigned` | "element 0: has no material"; a pressure without material accepted | yes |
| X3 NaN body force; face of 5 nodes | refused | "element 0: key 'body force': [NaN, 0.0, 0.0] at [...] is not finite"; "key 'face nodes': a face of 5 nodes ..." | yes |
| X4 ADR 0275's E3 replaced (scratch copy) | passes | `sb_static` 9 passed (`mutations.log`) | yes |
| N1 lumped gravity, Tet10 | `> 1e-4` | 0.45 | yes |
| N2 centrifugal against the unprojected distance | `> 1e-3` | 0.74 (all three) | yes |
| N3 ring inner surface against its own radius, Hex8 | `> 1e-2` | 2.9e-2 | yes |

Against the estimates: the exactness arguments held (Hex8 hanging and rotating bar, Tet10 hanging bar to rounding). The
Tet4 hanging tip (2.0e-5) and Tet10 rotating tip (3.0e-6) are one to three decades below the estimates `4e-4` and `2e-3`,
which were bounds. The ring matched the chord estimate: Hex8 `-0.20` percent (estimated `-0.24`), Tet4 `-0.17` to `-0.33`
percent, Tet10 with curved edges `8.5e-6`. Ring unknowns 243, 243, 1290; bar unknowns 216 to 1200; relative residuals of
the direct solves 3.6e-14 to 1.8e-11.

**Existing tests** (release): `sb_model` 11 passed, `sb_elements` 8 passed, the doc tests pass, `sb_static` 8 of 9 passed:
`e1_to_e4_free_node_conflicts_loads_element_errors` **fails** at line 707 (`unwrap_err` on a solve that now succeeds), as
registered in finding 1: its E3 asserts that gravity is refused. The test is not edited. With the replacement of X4 applied
on the scratch copy the whole of `sb_static` passes (E1, E2, E3 with the temperature load, E4).

**Mutations** (scratch copy by `git archive` of f2b29d0c, own target directory, each alone by exact text replacement,
`sb_loads` run once, the file restored and compared by SHA-256; `f375-scratch/mutate.py`, `mutations.log`):

| Id | Caught by |
| --- | --- |
| M1 pressure sign | R2 (2.0), D2 (2.0), three kinds |
| M2 Tri6 corner functions `L_i` | D1, D2 (3.0), R2, R3 (1.0), Tet10 |
| M3 no axial projection | R5, D3, B2 (and N2 then fails, as it should) |
| M4 axis not normalised | R5 (5.9), D3, B2 |
| M5 stiffness rule for body loads | D3 (7.7e-3) |
| M6 no density | R4, B1, C1, C2 |
| M7 nodal value instead of the interpolated field | B3, R6 |
| M8 no collapse Jacobian | D1, D2, R1 moment, R2, R3 (tetrahedra) |
| M9 temperature skipped | X1 |

`cargo fmt -p rcfd-fem --check`: clean. `cargo clippy -p rcfd-fem --all-targets -- -D warnings`: clean.

**Deviations.**

1. ADR 0275's E3 fails by construction (finding 1); not edited. The replacement is the lead's step.
2. The nodal-force arm of `nodal_load_vector` moved from a closure into the function `add_force`; the additions are the
   same in the same order, so a nodal-force-only model gives the same bits (`sb_static`'s U and R checks pass, and its
   R3 compares the load vector with one built in the test: 0).
3. Not run: the whole workspace (the lead runs the gate) and the baseline hashes (no file outside `rcfd-fem` changed and no
   crate depends on `rcfd-fem` yet).

**Decision by the registered rule: done** (pending the maintainer's acceptance). R1 to R6, D1 to D3, C1 to C4 and B1 to B3
are met for every kind named, N1 to N3 hold, every mutation fails a registered check. The gate fails on ADR 0275's E3 until
the lead applies (or rejects) the replacement below.

**Remaining questions.**

1. ADR 0275's E3. Options: (a) the lead applies the two-line replacement checked as X4 (`Load::temperature(vec![350.0;
   m3.mesh().nodes().len()], 293.15)` and `load: "Temperature"`); ADR 0281 must then replace it again, with whatever load is
   still unsupported, or drop it; (b) mark the test `#[ignore]` as a record of ADR 0275; (c) delete E3. Recommendation: (a),
   which keeps E4 running and E3's purpose (nothing is ignored) until ADR 0281.
2. Pressure varying over a surface (fluid loads): options (a) per-node forces from ADR 0286 as now; (b) a `Load` variant
   with face or nodal pressures. Recommendation: (a) for the basic tier; `face_shape_functions` and `face_rule` are public
   for it.

## For the lead: roadmap entries

F-375: proposed status **done**. Note:

> Structural solver, basic tier (plan ADR 0270). ADR 0276, 2026-11-12: rcfd_fem::loads::nodal_load_vector now adds the consistent nodal loads of a uniform traction and a uniform pressure (positive pressing on the solid) on surface sets, by the face functions of Tri3, Quad4 and Tri6 (face_shape_functions) and a 3 x 3 Gauss rule on quadrilaterals and a 9-point collapsed Gauss rule on triangles (face_rule; pressure exact on every face, traction on flat faces), and of gravity, the centrifugal load rho omega^2 r_perp about an axis through a point and a nodal body force field, by each element's mass rule (exact on Tet4, trilinear Hex8 and straight Tet10); body_force_loads gives the load of a body force given by a function as per-node forces; Load::Temperature stays refused (ADR 0281). Resultants on a box with moved interior nodes, Tet4, Hex8, Tet10: closed-surface pressure force and moment within 4.4e-17 and 1.3e-17 (limit 1e-12 p A, p A L), traction t A, gravity rho V g, centrifugal rho omega^2 V r_c about an oblique axis and a linear body force field within 9.8e-16 (limit 1e-12); per-node loads against the closed forms within 9.1e-16. Closed forms with nu = 0: hanging bar Tet10 nodal error 8.7e-13 (limit 1e-9), tip Hex8 2.1e-15 and Tet4 2.0e-5 at 8 elements (limit 1e-2); rotating bar tip Tet10 3.0e-6 at 8 and Hex8 1.1e-15 at 16 elements (limit 1e-2); thin rotating ring (t/R = 0.02, quarter ring by a mapped box with symmetry rollers) mid-surface radial displacement within 3.3e-3, 2.0e-3, 8.5e-6 (limit 2e-2). Nine mutations caught. ADR 0275's test E3 (gravity refused) fails by construction and needs the lead's two-line replacement (ADR 0276 remaining question 1).

F-380 (ADR 0281), sentence to append to its note:

> ADR 0276 (2026-11-12): in loads.rs only the Load::Temperature arm (other => unsupported) remains for ADR 0281; body loads are sums over Element::integration_points with the mass rule (private helpers add_body_load and element_body_load can carry the thermal load integrand); if the lead applied ADR 0276's replacement of ADR 0275's E3, that check asserts the temperature refusal and must be replaced or dropped by ADR 0281.

F-378 (ADR 0279), sentence to append to its note:

> ADR 0276 (2026-11-12): rcfd_fem::loads::body_force_loads(model, |x| b(x)) returns the consistent load of a body force given by a function as one NodalForceAt per node, for solve_static_with; tractions on box faces via Load::traction (exact on flat faces).

F-385 (ADR 0286), sentence to append to its note:

> ADR 0276 (2026-11-12): loads::face_shape_functions and loads::face_rule (Tri3, Quad4, Tri6 in the local face orders of ADR 0271) are public for nodal loads of face forces or pressures given per face or node.

## The planned stub, as written

Status: **PLANNED 2026-11-12** (layout by the basic-tier plan, [ADR 0270](0270-structural-solver-basic-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are
the acceptance draft that the registration turns into checks.

Work package SB-6 of the structural solver, basic tier. Size M.

### Goal

Consistent nodal loads `f = int N^T t dS` and `f = int N^T b dV` for the three elements: a traction vector and a pressure on a
surface set, gravity, the centrifugal body force `rho omega^2 r_perp` of a part rotating about a given axis at constant speed
(the dominant blade load of a fan), and a body force given at the nodes or by a function (used by the manufactured solution of
ADR 0279). Pressure is positive when it presses on the solid.

### Dependencies

ADR 0275; existing roadmap items: none.

### Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. Consistent nodal loads for surface traction, pressure
(positive pressing on the solid), gravity, the centrifugal load rho omega^2 r_perp about a given axis, and a body force field,
for Tet4, Hex8 and Tet10. Resultants on a box: a uniform pressure on the closed surface gives zero force and moment within 1e-12
p A L; the traction resultant is t A, gravity rho V g and the centrifugal resultant rho omega^2 V r_c (r_c the centroid's
distance from the axis), each within 1e-12. Closed forms with nu = 0, displacements only: hanging bar, u(x) = rho g (L x -
x^2/2)/E: Tet10 nodal error <= 1e-9 relative, Hex8 and Tet4 tip displacement within 1 percent at 8 elements along; rotating
radial bar clamped on the axis: tip displacement rho omega^2 L^3/(3 E) within 1 percent (Tet10 at 8, Hex8 at 16 elements along);
thin rotating ring (thickness/radius 0.02): radial displacement rho omega^2 r^3/E within 2 percent.

### Reference to check against

Closed forms, all derivable in a few lines: 1-D equilibrium `d sigma/dx + b = 0` with `b = rho g` and `b = rho omega^2 r`; the
thin-ring hoop stress `rho omega^2 r^2` (membrane equilibrium).

### Files

`crates/rcfd-fem/src/loads.rs`, `crates/rcfd-fem/tests/sb_loads.rs`.

### What could make it miss

The consistent load of a uniform pressure on a quadratic triangle is zero at the corners and one third at each mid-edge node:
correct, but a resultant-only check would not catch a wrong distribution, hence the displacement checks. The ring needs curved
geometry (ADR 0272 is not a dependency: the test builds its ring with the hexahedron builder and a coordinate map). Spin
softening and stress stiffening are NOT included (intermediate tier).
