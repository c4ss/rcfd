# ADR 0273: Element library: linear tetrahedron, trilinear hexahedron, quadratic tetrahedron; isotropic elasticity matrix

Status: **REGISTERED 2026-11-12, written before any code of F-372 and before any run of its checks; planned by ADR 0270**.
Not yet accepted by the maintainer. Before this text nothing of F-372 was written, compiled or run: the code and documents
under "What was read" were read, and the arithmetic under "Arithmetic before this registration" was done by hand; no script
and no library code was run. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Work package SB-3 of the structural solver, basic tier ([ADR 0270](0270-structural-solver-basic-tier-plan.md), section 3, the
entry for ADR 0273). Roadmap item F-372 (depends on F-370, done). Size M. Number 0273, fixed by the plan; the file name of the
planned stub is kept and the stub's text is kept unchanged at the end ("The planned stub, as written"). Branch
`f372-element-library` from main 73c5dfa0, worktree `w49i-work/f372`.

House rules (`w49i-work/AGENT_RULES.md`):

- limits are fixed here, before the code and the runs, and are not changed after a run; a missed limit becomes an ignored
  `*_known_miss` test with the limit intact and the number recorded, diagnosed, not tuned away; a defective set-up gets a
  corrected test by an amendment written and committed before the corrected run, the old test stays as an ignored record;
- each registered run is made once; nothing is re-run to pick a result;
- defaults bit-identical: only the three one-line stubs `element.rs`, `quadrature.rs`, `material.rs` of `rcfd-fem` are
  filled and one test file is added; no other existing file, no shared solver file, no golden, no hash table is touched;
- no new dependency (the crate keeps `rcfd-core`, `rcfd-mesh`, `rcfd-linalg`), no unsafe code, the graph stays acyclic;
- clean-room: everything below is derived here from first principles (isoparametric interpolation, Gauss quadrature, moment
  equations of symmetric tetrahedral rules, Hooke's law); books are cited from memory for orientation only.

## What F-372 asks

The acceptance of `docs/roadmap.json`, binding:

> Every limit not stated here is registered in the ADR before the run. For Tet4, Hex8 (2 x 2 x 2 Gauss) and Tet10, on the
> reference element and on randomly distorted ones (registered seed): the shape functions sum to 1 and their derivatives to 0
> within 1e-14 at 100 random points; the analytic derivatives agree with central differences within 1e-7; each quadrature rule
> integrates every monomial up to its stated degree within 1e-13; the element stiffness is symmetric within 1e-13 of its norm,
> gives no force for the six rigid-body motions within 1e-11 relative and has rank n - 6; each of the six unit strain states
> gives the strain energy V/2 eps:D:eps within 1e-12 relative; D gives stress E eps in uniaxial stress and G gamma in shear
> within 1e-14.

Clauses, for Tet4, Hex8 (stiffness by 2 x 2 x 2 Gauss) and Tet10, each on the reference element and on randomly distorted
ones: (a1) the shape functions sum to 1 within 1e-14 at 100 random points; (a2) their derivatives sum to 0 within 1e-14 there;
(a3) the analytic derivatives agree with central differences within 1e-7; (a4) each quadrature rule integrates every monomial
up to its stated degree within 1e-13; (a5) the element stiffness is symmetric within 1e-13 of its norm; (a6) it gives no force
for the six rigid-body motions within 1e-11 relative; (a7) it has rank n - 6; (a8) each of the six unit strain states gives
the strain energy `V/2 eps:D:eps` within 1e-12 relative; (a9) D gives the stress `E eps` in uniaxial stress and (a10) `G
gamma` in shear, within 1e-14.

The brief adds: an `Element` trait with room for the mass matrix (ADR 0284), the loads (0275, 0276), stress recovery (0277),
the manufactured solution (0279), and the intermediate tier's defaulted entry points (ADR 0290 section 2.3, ADR 0291) and the
advanced tier's point context of the constitutive call (ADR 0311); the rank by `rcfd_linalg::eigen` with a tolerance
registered here; random points and distortions from a small deterministic generator in the test, seed registered.

## What was read (main 73c5dfa0, read only)

1. `AGENT_RULES.md`; ADR 0270 in full (2.2 conventions: Voigt order xx, yy, zz, xy, yz, zx with engineering shear strains;
   2.3 module table: `element`, `quadrature`, `material` are 0273's; 2.4 the `Element` trait with `stiffness`, `mass`,
   `strain_at`, `face_load`, `body_load`, the constitutive call `stress(strain, state) -> (stress, tangent)`; the entries of
   0275 to 0279 and 0284 in section 3; the decision record); ADR 0271 in full with its addendum (D2 node orders and reference
   coordinates; `TET_EDGES`; the answer to its question 1: the element library takes the order of D2 as fixed); this ADR's
   stub; ADR 0290 sections 2.2 to 2.4 (the trait gains `geometric_stiffness`, `internal_force`, `tangent` with refusing
   defaults; a generic assembly hook); ADR 0291, 0292 (Hex20 needs a 3 x 3 x 3 rule), 0293 (B-bar and incompatible modes
   need the reduced and full rules and `B` at a point); ADR 0311 (the constitutive call gains a point context and a state
   size; "if the constitutive call of ADR 0273 has a fixed argument list ... a second entry point").
2. Code: `crates/rcfd-fem/src/lib.rs`, `mesh.rs` (`ElementKind`, `reference_nodes`, `shape_gradients` for the three kinds,
   `TET_EDGES`, the private degree-3 rule), `model.rs` (`Material::IsotropicElastic` and its accessors), `error.rs`
   (`FemError` variants `FieldLength`, `InvalidValue`), the stubs; `crates/rcfd-linalg/src/eigen.rs` (`symmetric_eigen(a,
   n)`: values ascending, refuses an asymmetry above `1e-12 max|a|`, then symmetrises).
3. **Not read:** the bodies of the books cited; any finite-element code outside this repository.

## Arithmetic before this registration (disclosed, by hand)

1. **Monomial integrals.** Cube `[-1, 1]^3`: `int x^a y^b z^c = I(a) I(b) I(c)`, `I(k) = 2/(k+1)` for even `k`, 0 for odd.
   Reference tetrahedron `{x, y, z >= 0, x + y + z <= 1}` (volume 1/6): `int x^a y^b z^c = a! b! c! / (a + b + c + 3)!` (the
   Dirichlet integral; checked for `a = b = c = 0`: `1/3! = 1/6`, and `a = 1`: `1/4! = 1/24` = volume times the centroid
   coordinate 1/4). Number of monomials of total degree `<= d`: `C(d + 3, 3)`: 4, 10, 56 for d = 1, 2, 5; of per-variable
   degree `<= p`: `(p + 1)^3`: 8, 64, 216 for p = 1, 3, 5.
2. **Gauss-Legendre on [-1, 1]** (exact to degree `2n - 1`): n = 1: point 0, weight 2; n = 2: `+-1/sqrt(3)`, weights 1 (check
   `x^2`: `2/3`); n = 3: 0 with weight 8/9 and `+-sqrt(3/5)` with 5/9 (checks: `8/9 + 10/9 = 2`; `x^2`: `10/9 x 3/5 = 2/3`;
   `x^4`: `10/9 x 9/25 = 2/5`). The cube rules are the tensor products.
3. **Four-point tetrahedral rule, degree 2.** Points: the four permutations of the barycentric coordinates `(b, a, a, a)`,
   `b = 1 - 3a`, weight `V/4` each. Barycentric moments over a tetrahedron, as a fraction of its volume: `L1^2: 3! 2!/5! =
   1/10`, `L1 L2: 1/20`. Condition `(b^2 + 3 a^2)/4 = 1/10` with `b = 1 - 3a`: `12 a^2 - 6 a + 3/5 = 0`, `a = (6 - sqrt(7.2))/24
   = (5 - sqrt 5)/20 = 0.1381966` (the root with `a < 1/4`, so the points are inside), `b = (5 + 3 sqrt 5)/20 = 0.5854102`. Then
   `L1 L2`: `(2 a b + 2 a^2)/4 = a (1 - 2a)/2 = (a - 2a^2)/2`; from the quadratic `2 a^2 = a - 1/10`, so it is `1/20`, exact.
   Degree 0 and 1 hold by the weights and the symmetry.
4. **Fourteen-point tetrahedral rule, degree 5** (the mass rule of Tet10, D3). Orbits: two of four points `(1 - 3 a_k, a_k,
   a_k, a_k)` with weights `w_k`, one of six points `(b, b, 1/2 - b, 1/2 - b)` with weight `w_3`: six unknowns. A rule
   invariant under the 24 permutations of the barycentric coordinates is exact to degree 5 iff it integrates the invariant
   polynomials of degree `<= 5` modulo `L0 + L1 + L2 + L3 = 1`: one each for degree 0, 2 (`e2`), 3 (`e3`), 5 (`e2 e3`) and two
   for degree 4 (`e2^2`, `e4`) (partitions of d into parts 2, 3, 4): six equations, six unknowns. The constants are recalled
   from memory as a published positive-weight rule (Walkington's or Keast's tables; UNVERIFIED): `a_1 ~ 0.0927352503`, `w_1
   ~ 0.0122488405 (x 4)`, `a_2 ~ 0.3108859193`, `w_2 ~ 0.0187813210 (x 4)`, `b ~ 0.0455037041`, `w_3 ~ 0.0070910035 (x 6)`
   (weights absolute on the reference tetrahedron); only check by hand: `4 (0.0122488 + 0.0187813) + 6 x 0.0070910 =
   0.1666664`, about 1/6. They are used only as the start of Newton's method on the six equations (D3), never as given.
5. **Hooke's law** in Voigt form with engineering shear: `lambda = E nu / ((1 + nu)(1 - 2 nu))`, `G = mu = E / (2 (1 + nu))`;
   `D = lambda (1 1^T on the normal block) + 2 mu I_normal + mu I_shear`. Uniaxial stress along x: `eps = s (1, -nu, -nu, 0, 0,
   0)` gives `sigma_xx = (lambda + 2 mu) s - 2 lambda nu s = s (lambda (1 - 2 nu) + 2 mu) = E s` (since `lambda (1 - 2 nu) = E
   nu/(1 + nu)` and `2 mu = E/(1 + nu)`), `sigma_yy = lambda s + (lambda + 2 mu)(-nu s) + lambda (-nu s) = s (lambda (1 - 2
   nu) - 2 mu nu) = 0`; shear `tau_xy = mu gamma_xy`. Rounding: for `nu = 0.45`, `lambda = 3.10 E`, `lambda + 2 mu = 3.79 E`:
   `sigma_xx` is a difference of terms of size `3.8 E s`, so about `4 x 2 eps x 3.8 = 7e-15` relative to `E s` in the worst
   case and typically `1e-15`; the limit 1e-14 (fixed by the acceptance) has a margin of about 1.5 on the worst case at `nu =
   0.45` and 7 at `nu = 0.3`. Larger `nu` would not meet it in floating point (the cancellation grows like `1/(1 - 2 nu)`), so
   the registered `nu` stop at 0.45 and the limit for `nu -> 0.5` is listed under Limits.
6. **Rounding estimates of the element checks** (n the number of unknowns, at most 30; elements of size about 1).
   Partition of unity: at most 10 terms of size `<= 1`: `<= 1e-15`. Sum of reference derivatives: 10 terms of size `<= 4`:
   `<= 10 x 4 x 1.1e-16 = 4.4e-15`; Hex8 cancels exactly in mirror pairs. Physical derivatives `J^{-T} dN/dxi`: each a 3-term
   dot, so `|sum_a grad N_a| <= ~ 3 n_a eps max_a |grad N_a| + cond(J) 4.4e-15`; with `cond(J) <= ~ 2` on the distortions of D5
   that is `<= ~ 1.2e-14` worst, `~ 2e-15` typical: the limit 1e-14 (fixed by the acceptance) is met typically with a margin
   of 5 and is tight in the worst case; a miss there would be rounding and recorded as such. Central differences with step `h
   = 1e-5`: the shape functions are of degree at most 1 (Hex8, along each reference axis) or 2 (Tet10) along any line, so the
   truncation error is 0 and the rounding error is `~ eps |N| / h = 1.1e-11`: margin 1e4 to 1e-7. Symmetry: the two triangles
   of `K` are summed in different orders: `~ n_q x 6 x eps` relative, `~ 1e-14` worst against 1e-13. Rigid-body force: `B r`
   vanishes up to rounding `~ 10 eps |grad N| |x|`: `|K r| / (||K||_F |r|) ~ 1e-15`, margin 1e4 to 1e-11. Strain energy: `u^T
   K u` with `u` linear, entries of size `eps_unit |x| |f|` without cancellation for elements centred near the origin: `~ n
   eps x 10 = 3e-14`, margin 30 to 1e-12.
7. **Rank tolerance.** The eigenvalues of the six rigid-body modes of a computed `K` are `~ eps ||K|| ~ 1e-15 lambda_max` (Jacobi
   is accurate to `eps ||K||` absolutely); the smallest deformation eigenvalue of an element of aspect ratio about 1 is
   estimated at `1e-3` to `1e-1` of the largest (Tet4 and Hex8 about `1e-2` to `1e-1`, Tet10 about `1e-3` to `1e-2`), at most
   a factor 10 lower on the distortions of D5. A threshold `tau = 1e-9 lambda_max` therefore lies at least 4 decades from
   both groups. Rank = the number of eigenvalues `> tau lambda_max`.

## Design

### D1. Shape functions (`element.rs`)

Reference coordinates and node orders are those of ADR 0271 D2 (`rcfd_fem::mesh`), taken as fixed.

* **Tet4**: `xi = (xi, eta, zeta) = (L1, L2, L3)`, `L0 = 1 - xi - eta - zeta`; `N_a = L_a`.
* **Hex8**: on `[-1, 1]^3`, `N_a = (1 + xi xi_a)(1 + eta eta_a)(1 + zeta zeta_a)/8` with `(xi_a, eta_a, zeta_a)` the
  reference node `a` of `ElementKind::reference_nodes`.
* **Tet10**: corners `N_i = L_i (2 L_i - 1)`, mid-edge node `4 + k` on edge `TET_EDGES[k] = (i, j)`: `N = 4 L_i L_j`.

The derivatives with respect to the reference coordinates are `ElementKind::shape_gradients` of `mesh.rs` (ADR 0271): one
implementation for the validation of the mesh and for the elements, so the two cannot disagree on a node order. The
functions themselves are new here; check A3 (central differences) ties them to those derivatives, and check E1 (Kronecker
property at the reference nodes) ties them to `reference_nodes`.

### D2. Isoparametric map and the strain-displacement matrix

`J_ij = dx_i/dxi_j = sum_a x_a,i dN_a/dxi_j`; `det J` by the triple product; `J^{-1}` by the adjugate over `det J`; physical
gradients `grad N_a = J^{-T} dN_a/dxi` (componentwise `dN_a/dx_k = sum_j (J^{-1})_jk dN_a/dxi_j`). A point with `det J <= 0` or
not finite is refused (`FemError::InvalidValue { key: "jacobian", .. }`, naming the integration point and the value); a
coordinate list of the wrong length is refused (`FieldLength { key: "coordinates" }`), a displacement of the wrong length
(`FieldLength { key: "displacement" }`). Unknowns of an element node-major, `(u_x, u_y, u_z)` of node 0, then node 1, ...;
the `DofMap` of ADR 0271 maps them to equations (ADR 0275). Rows of `B` (6 x 3 n) in the order xx, yy, zz, xy, yz, zx with
engineering shear (ADR 0270 2.2): node `a` contributes `[g_x 0 0; 0 g_y 0; 0 0 g_z; g_y g_x 0; 0 g_z g_y; g_z 0 g_x]` with `g =
grad N_a`.

### D3. Quadrature rules (`quadrature.rs`)

`QuadratureRule` (private fields: domain, points in reference coordinates, absolute weights on the reference domain, stated
degree; accessors). Rules, each built once (`std::sync::OnceLock`):

| Rule | Domain | Points | Stated degree | Use |
| --- | --- | --- | --- | --- |
| `tet_1` | tetrahedron | centroid, weight 1/6 | total 1 | Tet4 stiffness |
| `tet_4` | tetrahedron | arithmetic 3, weights 1/24 | total 2 | Tet10 stiffness, Tet4 mass |
| `tet_14` | tetrahedron | arithmetic 4, constants by Newton | total 5 | Tet10 mass |
| `gauss_1`, `gauss_2`, `gauss_3` | cube | tensor Gauss-Legendre (arithmetic 2) | per variable 1, 3, 5 | Hex8 stiffness `gauss_2`; Hex8 mass `gauss_3`; `gauss_1` for reduced integration (ADR 0293) |

The constants of `tet_14` are solved, after this registration is committed and before the library code, by Newton's method on
the six invariant moment equations of arithmetic 4 in a scratch script (`f372-scratch/tet14.py`, double precision, the start
values of arithmetic 4), iterated until the residual stops falling; the constants are written into `quadrature.rs` with 17
significant digits and the script's final residual is recorded in the addendum. The script checks only the six invariant
equations; check A4 checks all 56 monomials with the constants as written into the library. Fallback, if Newton does not
converge to a rule with all points inside and positive weights: the collapsed (Duffy) product of 4-point Gauss-Legendre rules,
`x = u, y = (1 - u) v, z = (1 - u)(1 - v) w` on `[0, 1]^3` with Jacobian `(1 - u)^2 (1 - v)`, 64 points, exact to total degree 5
(a monomial of degree `<= 5` becomes of degree `<= 7, 6, 5` in `u, v, w`; Gauss with 4 points is exact to 7). Which one was
used is recorded in the addendum.

Mass rules: Tet4 `tet_4` and Tet10 `tet_14` integrate `N_a N_b det J` exactly on straight-sided elements (degree 2 and 4);
Hex8 `gauss_3` integrates `N_a N_b det J` exactly on every trilinear element (degree `<= 2 + 2 = 4` per variable). ADR 0284
confirms or replaces them; here they are only checked by A4.

### D4. Element trait, element stiffness, strain at a point (`element.rs`)

```text
pub trait Element: Sync {
    fn kind(&self) -> ElementKind;                                  // required
    fn shape_functions(&self, xi: [f64; 3]) -> Vec<f64>;            // required (D1)
    fn shape_derivatives(&self, xi: [f64; 3]) -> Vec<[f64; 3]>;     // required (D1, from ElementKind::shape_gradients)
    fn stiffness_rule(&self) -> &'static QuadratureRule;            // required (D3)
    fn mass_rule(&self) -> &'static QuadratureRule;                 // required (D3)
    fn n_nodes(&self) -> usize;                                     // provided
    fn point(&self, coords: &[Vec3], xi: [f64; 3]) -> Result<PointGeometry>;          // provided: N, grad N, J, det J
    fn integration_points(&self, coords, rule) -> Result<Vec<IntegrationPoint>>;      // provided: PointGeometry and w det J
    fn stiffness(&self, coords: &[Vec3], d: &Matrix6) -> Result<ElementMatrix>;       // provided: stiffness_rule
    fn stiffness_with_rule(&self, coords, d, rule) -> Result<ElementMatrix>;          // provided
    fn strain_at(&self, coords: &[Vec3], u: &[f64], xi: [f64; 3]) -> Result<[f64; 6]>; // provided: B u
}
```

`Tet4`, `Hex8`, `Tet10` are unit structs implementing it; `element_for(kind) -> &'static dyn Element`. `K_e = sum_q w_q det J_q
B_q^T D B_q`, computed in full (both triangles, `K_ij = sum_k B_ki (D B)_kj`, no mirroring, so that A5 tests something),
in the order of the rule's points. `ElementMatrix` is a dense row-major `n x n` with `size()`, `get(i, j)`, `values()`. `D` is
constant over the element in this tier; a point-dependent material is the intermediate tier's (`stiffness_with_rule` is the
place a later `stiffness_with_material` sits beside).

Room for the later packages, without editing this file: the mass matrix (ADR 0284) is `sum_q rho N^T N w det J` from
`integration_points(coords, mass_rule())`; the body load (0276) `sum_q N^T b w det J` from the same; the face loads (0276) need
face functions of Tri3, Quad4 and Tri6, which belong to `loads.rs` with the local faces of ADR 0271; strain and stress at the
integration points (0277) are `strain_at` at `stiffness_rule().points()`; the manufactured solution (0279) uses
`integration_points` with a rule of higher degree. The intermediate tier's `geometric_stiffness`, `internal_force` and
`tangent` (ADR 0291) are added to this trait as provided methods that refuse, which breaks no implementer; Hex20 (0292)
implements the five required methods with `gauss_3`; B-bar and incompatible modes (0293) use `point` and `gauss_1`.

### D5. Constitutive matrix and call (`material.rs`)

`pub type Matrix6 = [[f64; 6]; 6]`; `isotropic_elasticity(e, nu) -> Matrix6` (arithmetic 5, no validation: the values come
from `Material::isotropic_elastic`, which refused bad ones); `Material::elasticity_matrix(&self) -> Result<Matrix6>`;
`PointContext` (`#[non_exhaustive]`, `Default`; no field yet: ADR 0311 adds temperature, time increment and material axes,
with the default meaning "none, global axes"); `Material::state_size(&self) -> usize` (0 for elasticity); the constitutive call
`Material::stress(&self, strain: &[f64; 6], point: &PointContext, state: &[f64]) -> Result<([f64; 6], Matrix6)>` returning
stress and tangent (`D eps` and `D`; a state of the wrong length is refused, `FieldLength { key: "state" }`). The context and
the state slice are in the argument list now, so ADR 0311 needs no second entry point. Inherent `impl Material` in
`material.rs` (the type stays in `model.rs`, unedited).

### D6. The test set-up (`tests/sb_elements.rs`)

* **Generator:** SplitMix64 (from memory: `z = (s += 0x9E3779B97F4A7C15)`, then `z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9`, `z
  = (z ^ (z >> 27)) * 0x94D049BB133111EB`, `z ^ (z >> 31)`, wrapping), uniform `(z >> 11) 2^-53` in `[0, 1)`. Used as a
  deterministic source only, no statistical claim. **Seed `0xF372_0273`**; each use gets its own stream seeded `SEED +
  tag` with a fixed tag per use (distortions of kind k: `10 + k`; points: `20 + k`; Tet4 k = 0, Hex8 1, Tet10 2), so that the
  checks draw the same elements and points independently of their order.
* **Elements per kind:** the reference element, and 10 distorted ones; for Tet10 10 curved and 5 straight-sided ones.
  Distortion: reference node coordinates (tetrahedron edge 1, cube edge 2) plus a uniform perturbation per coordinate in
  `[-r, r]`, `r = 0.1` (tetrahedra, corners) and `r = 0.2` (Hex8), then a random rotation (unit quaternion from four draws in
  `[-1, 1]`, normalised; a norm below 0.1 is redrawn) and no translation. Tet10 straight-sided: the mid-edge nodes at the mean
  of the perturbed corners; curved: plus a perturbation in `[-0.03, 0.03]` per coordinate (arithmetic: the corner tangent of a
  quadratic edge changes by `4 x 0.052 = 0.21` against an edge of at least ~0.65, so no inversion). Each element is passed
  through `SolidMesh::new` and `add_block` (ADR 0271 validation, det J > 0 at every node) and the test asserts it is accepted.
* **Points:** 100 per element: uniform in `[-1, 1]^3` (Hex8), uniform in the reference tetrahedron by rejection from `[0,
  1)^3` (`x + y + z <= 1`) for the tetrahedra; a fresh set per element.
* **Material:** `E = 2.0e11`, `nu = 0.3` for the element checks (relative limits make `E` irrelevant); for A9, A10 `E = 2.0e11`
  and `nu` = 0, 0.3, 0.45, -0.5, each through `Material::isotropic_elastic`.

## Registered checks

All in `crates/rcfd-fem/tests/sb_elements.rs`, release profile, one run (`cargo test -p rcfd-fem --release --test sb_elements
-- --nocapture --test-threads=1`). "All elements" means, per kind, the reference element and the distorted ones of D6 (Tet10:
both families). Every printed number is the maximum over all elements and points of the kind.

| Id | Clause | Check | Limit | Estimate behind it |
| --- | --- | --- | --- | --- |
| A1 | a1 | all elements, 100 points each: `|sum_a N_a - 1|` | `<= 1e-14` | arithmetic 6: `<= 1e-15` |
| A2 | a2 | reference elements: `|sum_a dN_a/dxi_j|` (absolute); distorted: `|sum_a dN_a/dx_k| / max_a |grad N_a|`, from `Element::point` | `<= 1e-14` | arithmetic 6: `~ 2e-15` typical, tight worst case |
| A3 | a3 | all elements, 100 points, `h = 1e-5`: reference derivatives against `(N_a(xi + h e_j) - N_a(xi - h e_j))/(2h)`, absolute; distorted: physical gradients along the central-difference tangents `T_j = (x(xi + h e_j) - x(xi - h e_j))/(2h)`: `|grad N_a . T_j - (N_a(xi + h e_j) - N_a(xi - h e_j))/(2h)|` (the chain rule), absolute | `<= 1e-7` | arithmetic 6: truncation 0, rounding `~ 1e-11` |
| A4 | a4 | every rule of D3: every monomial of total degree `<= d` (tetrahedron) or of degree `<= p` in each variable (cube) against arithmetic 1, absolute | `<= 1e-13` | rounding of `<= 64` products of size `<= 8`: `~ 1e-15` |
| A5 | a5 | all elements: `max_ij |K_ij - K_ji| / max_ij |K_ij|` | `<= 1e-13` | arithmetic 6 |
| A6 | a6 | all elements, the three translations `e_k` at every node and the three rotations `r_a = e_k x x_a`: `|K r|_2 / (||K||_F |r|_2)` | `<= 1e-11` | arithmetic 6: `~ 1e-15` |
| A7 | a7 | all elements: eigenvalues of `K` by `rcfd_linalg::eigen::symmetric_eigen`; rank = number `> 1e-9 lambda_max` | `= n - 6` (6, 18, 24) | arithmetic 7; the ratios `lambda_6 / lambda_max` and `lambda_7 / lambda_max` (1-based ascending) are printed |
| A8 | a8 | all elements, the six unit strains `eps = e_k` (engineering shear), nodal displacement `u_a = E x_a` with `E` the symmetric tensor of `eps`: `|u^T K u / 2 - V/2 eps^T D eps| / (V/2 eps^T D eps)`, `V = sum_q w_q det J_q` of the stiffness rule | `<= 1e-12` | arithmetic 6 |
| A9 | a9 | `nu` = 0, 0.3, 0.45, -0.5; uniaxial stress along x, y and z, `s = 1e-3`: `max_k |sigma_k - E s delta_k,axis| / (E s)`, both for `D eps` from `elasticity_matrix` and for `Material::stress` | `<= 1e-14` | arithmetic 5 |
| A10 | a10 | the same `nu`; `gamma = 1e-3` in each of the three shear slots: `max_k |sigma_k - G gamma delta_k,slot| / (G gamma)`, `G = E/(2 (1 + nu))` | `<= 1e-14` | arithmetic 5: one rounding |
| E1 | D1 (extension) | every kind: `N_a(x_b) = delta_ab` at the reference nodes of `ElementKind::reference_nodes` | `<= 1e-15` absolute | exact or one rounding |
| E2 | D2 (extension) | distorted elements: linear reproduction `sum_a x_a,i dN_a/dx_k = delta_ik` at the 100 points | `<= 1e-13` | as A2 with coordinates of size 1.3 |
| E3 | A8 (extension) | `strain_at` of the six unit-strain fields at the 100 points equals `eps` | `<= 1e-12 |eps|` absolute | as A8 |
| E4 | D5 (extension) | `D S = I` with the closed-form compliance `S` (`1/E`, `-nu/E`, `1/G`), the four `nu` of A9: `max |D S - I|` | `<= 1e-13` | arithmetic 5 with `lambda <= 3.1 E` |
| E5 | D2 (extension) | refusals: coordinates of 3 nodes for a Tet4 (`coordinates`), a displacement of wrong length (`displacement`), an inverted Tet4 (`jacobian`, message names the point), a state of length 1 (`state`) | as stated | D2, D5 |
| E6 | A8 (record only) | curved Tet10: `|V_rule - V_exact| / V_exact` with `V_exact = SolidMesh::element_volume` (the degree-3 rule of ADR 0271); printed, no limit | none | 4-point rule not exact for the cubic `det J` |

**Negative controls** inside the checks (each must hold, else the check is not sensitive):

| Id | Control | Expected |
| --- | --- | --- |
| N1 | for every rule of D3, the largest monomial error at degree `d + 1` (total) or `p + 1` (per variable) | `> 1e-6` (the stated degree is the true one) |
| N2 | rank (A7's threshold) of Hex8 and Tet10 with `stiffness_with_rule(gauss_1)` and `tet_1` on the reference elements | `6` (below `n - 6`: the threshold sees a deficiency) |
| N3 | A6's measure for the unit strain field `eps = e_0` instead of a rigid motion, all reference elements | `> 1e-3` |

**Mutations** (on a `git archive` copy of the final tree in `w49i-work/f372-scratch`, own target directory; each applied alone
by exact text replacement, `sb_elements` run once, the file restored and compared byte for byte):

| Id | Mutation | Must fail |
| --- | --- | --- |
| M1 | `tet_4`: `a` replaced by `0.14` | A4 |
| M2 | `tet_14`: one weight changed in its fourth significant digit | A4 |
| M3 | `gauss_3`: `sqrt(3/5)` replaced by `0.7746` | A4 |
| M4 | `B`: the yz row takes `g_x` instead of `g_z` at the y column | A6 or A8 |
| M5 | `D`: shear diagonal `2 mu` (tensor shear) instead of `mu` | A10 |
| M6 | `lambda` with `(1 - nu)` instead of `(1 - 2 nu)` | A9 |
| M7 | physical gradients with `J^{-1}` instead of `J^{-T}` | A2 or A3 (distorted) |
| M8 | Tet10 shape functions: edges 1 and 2 of the mid-edge loop swapped | E1 and A3 |
| M9 | Tet10 stiffness rule `tet_1` | A7 |
| M10 | Hex8 stiffness rule `gauss_1` | A7 |
| M11 | Hex8 shape functions divided by 4 instead of 8 | A1 |
| M12 | stiffness without the factor `det J` | A8 (distorted) |

**Decision rule.** F-372 is proposed **done** (pending the maintainer's acceptance) if A1 to A10 are met for every element and
every rule (every acceptance clause), N1 to N3 hold and every mutation M1 to M12 fails its test. E1 to E5 are registered
extensions: a miss among them is a `*_known_miss` record and a follow-up item and does not alone block done, except that M8 is
then caught only by A3 (acceptable). A miss or an unrun check among A1 to A10, a failed negative control, or an uncaught
mutation (after checking that the test covers it): **partial**, with the miss named. No crate that builds: **todo**.

## Limits (what is not covered)

- One constant `D` per element; no point-dependent material, no thermal strain (ADR 0281), no near-incompressible material:
  the stiffness is computed for any `nu < 0.5`, but A9 is checked only to `nu = 0.45`, and for `nu -> 0.5` the uniaxial
  identity loses digits like `1/(1 - 2 nu)` (arithmetic 5) and Hex8 and Tet4 lock (ADR 0293).
- No mass matrix, no loads, no assembly: only the rules and the trait's room for them (D3, D4).
- On curved Tet10 the 4-point rule does not integrate `B^T D B det J` exactly: the stiffness is the usual approximate one and
  A8 holds with the rule's volume (E6 records the difference to the exact volume).
- Element quality: no shape measure; `det J > 0` is checked at the integration points used, not everywhere inside.
- The constants of `tet_14` come from a recalled rule refined by Newton; their correctness rests on A4 only.

## Open points taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. The derivatives of the shape functions come from `ElementKind::shape_gradients` of ADR 0271 (one implementation), not a
   second copy in `element.rs` (D1).
2. The mass rules of D3 (`tet_4`, `gauss_3`, `tet_14`) are chosen here for ADR 0284, which may replace them.
3. The constitutive call carries a point context and a state slice now (D5), so that ADR 0311 needs no second entry point.
4. Errors use the existing variants `FieldLength` and `InvalidValue` of `error.rs`, so `error.rs` is not edited; the element
   index is added by the caller (ADR 0275) that knows it.
5. The rank threshold `1e-9 lambda_max` (arithmetic 7) and the distortion amplitudes of D6.

## The planned stub, as written

Status: **PLANNED 2026-11-12** (layout by the basic-tier plan, [ADR 0270](0270-structural-solver-basic-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are
the acceptance draft that the registration turns into checks.

Work package SB-3 of the structural solver, basic tier. Size M.

### Goal

Shape functions and their derivatives, quadrature rules, the strain-displacement matrix and the element stiffness `K_e = sum_q
w_q B^T D B det J` for Tet4 (one point), Hex8 (2 x 2 x 2 Gauss, full integration) and Tet10 (a rule exact to degree 2 for
stiffness; a degree-4 rule is added here for the mass matrix of ADR 0284), and the isotropic matrix `D` from `E` and `nu`.
Behind an `Element` trait with the entry points later tiers need (section 2.4 of ADR 0270).

### Dependencies

ADR 0271; existing roadmap items: none.

### Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. For Tet4, Hex8 (2 x 2 x 2 Gauss) and Tet10, on the
reference element and on randomly distorted ones (registered seed): the shape functions sum to 1 and their derivatives to 0
within 1e-14 at 100 random points; the analytic derivatives agree with central differences within 1e-7; each quadrature rule
integrates every monomial up to its stated degree within 1e-13; the element stiffness is symmetric within 1e-13 of its norm,
gives no force for the six rigid-body motions within 1e-11 relative and has rank n - 6; each of the six unit strain states gives
the strain energy V/2 eps:D:eps within 1e-12 relative; D gives stress E eps in uniaxial stress and G gamma in shear within
1e-14.

### Reference to check against

First principles: Hooke's law; exact integrals of monomials over the reference tetrahedron, `a! b! c! / (a + b + c + 3)!`, and
over the cube. Method texts (cited from memory, to be confirmed by the implementer): Hughes, The Finite Element Method (1987);
Zienkiewicz and Taylor, The Finite Element Method, vol. 1. Quadrature points and weights are derived or checked by the monomial
test, never taken on trust.

### Files

`crates/rcfd-fem/src/{element.rs, quadrature.rs, material.rs}`, `crates/rcfd-fem/tests/sb_elements.rs`.

### What could make it miss

A tetrahedral rule recalled with wrong weights (the monomial check catches it); the rank check on a badly shaped element;
node-order convention of Tet10 differing from the mid-edge upgrade of ADR 0271.

## Addendum 1: results (2026-11-12)

Code at commit 4c806df0 (after the registration cd1cdc8e): `crates/rcfd-fem/src/element.rs`, `quadrature.rs`, `material.rs`
(the three stubs filled) and `crates/rcfd-fem/tests/sb_elements.rs`; no other file. The constants of `tet_14` were solved first
by `f372-scratch/tet14.py` (Newton on the six invariant equations from the recalled start values; converged in one step to a
residual of 2.8e-17; all points inside, all weights positive; constants agree with the recalled ones to the eight to ten digits
recalled), so the Newton rule is used, not the fallback. The registered run was made once, in release, with `cargo test -p
rcfd-fem --release --test sb_elements -- --nocapture --test-threads=1` (8 passed). Log: `w49i-work/f372-scratch/run1.log`.
Numbers are maxima over all elements (reference, 10 distorted; Tet10 also 5 straight-sided) and their 100 points.

| Id | Limit | Measured (Tet4; Hex8; Tet10) | Met |
| --- | --- | --- | --- |
| A1 `|sum N - 1|` | `<= 1e-14` | 0; 2.2e-16; 2.2e-16 | yes |
| A2 sum of derivatives, reference (absolute) / distorted (relative) | `<= 1e-14` | 0 / 9.0e-17; 1.1e-16 / 4.6e-16; 0 / 7.0e-16 | yes |
| A3 central differences, reference / distorted | `<= 1e-7` | 6.6e-12 / 6.8e-12; 1.1e-11 / 4.9e-12; 2.2e-11 / 7.1e-11 | yes |
| A4 monomials (rule: points, degree, count) | `<= 1e-13` | tet_1 (1, 1, 4) 0; tet_4 (4, 2, 10) 6.9e-18; tet_14 (14, 5, 56) 8.3e-17; gauss_1 (1, 1, 8) 0; gauss_2 (8, 3, 64) 8.9e-16; gauss_3 (27, 5, 216) 1.8e-15 | yes |
| A5 symmetry | `<= 1e-13` | 1.5e-16; 1.3e-16; 1.9e-16 | yes |
| A6 rigid-body force | `<= 1e-11` | 6.2e-17; 4.1e-17; 3.6e-17 | yes |
| A7 rank (threshold `1e-9 lambda_max`) | `n - 6` | 6 on all 11; 18 on all 11; 24 on all 16; max `lambda_6/lambda_max` 1.3e-16, 1.2e-16, 1.5e-16; min `lambda_7/lambda_max` 9.1e-2, 4.3e-2, 1.4e-3 | yes |
| A8 strain energy of the six unit strains | `<= 1e-12` | 7.4e-16; 4.5e-16; 5.8e-16 | yes |
| A9 uniaxial stress, `nu` = 0, 0.3, 0.45, -0.5 | `<= 1e-14` | 0, 7.5e-17, 6.0e-16, 0 | yes |
| A10 shear | `<= 1e-14` | 0 for all four `nu` | yes |
| E1 Kronecker property | `<= 1e-15` | 0; 0; 0 | yes |
| E2 linear reproduction | `<= 1e-13` | 3.3e-16; 6.7e-16; 9.2e-16 | yes |
| E3 `strain_at` of the unit strains | `<= 1e-12` | 3.3e-16; 6.7e-16; 9.2e-16 | yes |
| E4 `D S = I` | `<= 1e-13` | 1.1e-16, 2.2e-16, 4.4e-16, 1.1e-16 | yes |
| E5 refusals | as stated | `coordinates`, `displacement`, `jacobian` ("det J = -1e0 at the point (0.25, 0.25, 0.25) of a Tet4 element"), `state` | yes |
| E6 curved Tet10, `|V_rule - V_exact| / V_exact` | record | 1.2e-5 | record |
| N1 error at degree `d + 1` | `> 1e-6` | tet_1 6.3e-3; tet_4 3.6e-4; tet_14 1.4e-5; gauss_1 2.7; gauss_2 0.71; gauss_3 0.18 | yes |
| N2 rank with reduced rules | 6 | Hex8 with gauss_1: 6; Tet10 with tet_1: 6 | yes |
| N3 A6 measure of a unit strain | `> 1e-3` | 0.33; 0.46; 0.12 | yes |

The rounding estimates of arithmetic 5 and 6 were pessimistic by one to two decades (for example A2 distorted 7.0e-16 against
the estimated worst 1.2e-14; A9 at `nu = 0.45` 6.0e-16 against 7e-15). The rank gap is at least 13 decades below and 6 above
the threshold.

**Mutations** (scratch copy by `git archive` of 4c806df0, own target directory, each alone by exact text replacement, `sb_elements`
run once, file restored and compared by SHA-256; `f372-scratch/mutate.py`, `mutations.log`):

| Id | Caught by |
| --- | --- |
| M1 `tet_4` with `a = 0.14` | A4 (tet_4) |
| M2 a `tet_14` weight 7.091e-3 -> 7.092e-3 | A4 (tet_14) |
| M3 `sqrt(3/5)` -> 0.7746 | A4 (gauss_3) |
| M4 yz row with `g_x` at the y column | A6 (all three kinds), A7 (Tet10, rank 23), A8 and E3 |
| M5 shear diagonal `2 mu` | A10, E4 |
| M6 `lambda` with `(1 - nu)` | A9 (all `nu` but 0), E4 |
| M7 `J^{-1}` instead of `J^{-T}` | A3 (distorted), A6, A7 (Hex8 rank 21), A8, E3; A2 is blind to it (the sum of the wrong gradients is still zero), as the registration allowed ("A2 or A3") |
| M8 Tet10 mid-edge loop with edges 1 and 2 swapped | A3 (Tet10), E1 (Tet10) |
| M9 Tet10 stiffness by `tet_1` | A7 (rank 6) |
| M10 Hex8 stiffness by `gauss_1` | A7 (rank 6) |
| M11 Hex8 shape functions over 4 | A1, A3, E1 (Hex8) |
| M12 stiffness without `det J` | A8 (all three kinds) |

`cargo fmt -p rcfd-fem --check`: clean. `cargo clippy -p rcfd-fem --all-targets -- -D warnings` (debug and release): clean. The
existing `sb_model` (ADR 0271) passes unedited (11 passed, release).

**Deviations.**

1. M12 was applied in the stiffness sum (`ip.weight / ip.geometry.det_j`) and not in `integration_points`: there it would also
   change the volume `V` of A8, which is taken from the same points, and the mutation would cancel. This is the registered
   mutation ("stiffness without the factor det J") at the only place where it is a stiffness defect.
2. The 14-point rule's start values were recalled correctly; the registration's check by hand (sum of weights 0.1666664) is
   consistent with the solved weights (sum 1/6 to rounding).
3. Not run: the whole workspace (the lead runs the gate), the baseline hashes (no file outside `rcfd-fem` changed and no
   shared solver file was touched).

**Decision by the registered rule: done** (pending the maintainer's acceptance). A1 to A10 are met on every element and every
rule, N1 to N3 hold, every mutation M1 to M12 fails a registered check; the extensions E1 to E5 are met and E6 is recorded.

**Remaining questions.**

1. Where the intermediate tier's `geometric_stiffness`, `internal_force` and `tangent` go. Options: (a) provided methods of
   `Element` that refuse by default, added by ADR 0291 (as planned); (b) a second trait `NonlinearElement: Element`. Recommendation:
   (a); adding provided methods breaks no implementer.
2. The mass rules (D3: `tet_4`, `gauss_3`, `tet_14`). Options: (a) ADR 0284 takes them (exact for straight-sided tetrahedra
   and every trilinear hexahedron); (b) ADR 0284 chooses again. Recommendation: (a); on curved Tet10 no fixed rule is exact
   (the integrand has degree 7), and E6 shows the 4-point stiffness rule's volume error there is 1.2e-5 at mid-edge offsets of
   0.03, which ADR 0279 should keep in mind for its error norms.
3. Errors of the element routines carry no element index (D4, open point 4). Options: (a) the assembly of ADR 0275 maps them
   to the element; (b) add an element index to the trait's arguments. Recommendation: (a).

## For the lead: roadmap entries

F-372: proposed status **done**. Note:

> Structural solver, basic tier (plan ADR 0270). ADR 0273, 2026-11-12: rcfd_fem::element with the Element trait (shape functions, derivatives from ElementKind::shape_gradients of ADR 0271, the isoparametric point with J, det J and physical gradients, integration points with w det J, stiffness with its own or a given rule computed in both triangles, strain at a point; refusals by key: coordinates, displacement, jacobian) for Tet4 (1 point), Hex8 (2 x 2 x 2 Gauss) and Tet10 (4 points), element_for(kind); rcfd_fem::quadrature with tet_1, tet_4, tet_14 (degree 5, positive weights, constants by Newton on the invariant moment equations) and Gauss 1 to 3 on the cube; rcfd_fem::material with the isotropic D and the constitutive call stress(strain, point context, state) returning stress and tangent. On the reference elements and 10 randomly distorted ones of each kind (Tet10: 10 curved and 5 straight-sided; seed 0xF372_0273): partition of unity within 2.2e-16 and derivative sums within 7.0e-16 (limit 1e-14), central differences within 7.1e-11 (limit 1e-7), every monomial up to each rule's degree within 1.8e-15 (limit 1e-13, and each rule misses degree d + 1), stiffness symmetric within 1.9e-16 (limit 1e-13), rigid-body force within 6.2e-17 relative (limit 1e-11), rank n - 6 by rcfd_linalg::eigen with the threshold 1e-9 lambda_max (rigid modes at 1.5e-16, the seventh at 1.4e-3 or more), strain energy of the six unit strains within 7.4e-16 (limit 1e-12); D gives E eps in uniaxial stress within 6.0e-16 and G gamma in shear exactly (nu = 0, 0.3, 0.45, -0.5; limit 1e-14). Twelve mutations caught.

F-374 (ADR 0275), sentence to append to its note:

> ADR 0273 (2026-11-12): element_for(kind).stiffness(coords, &material.elasticity_matrix()?) gives the dense element stiffness (node-major unknowns, three per node); its errors name the key but not the element, which the assembly adds.

F-375 (ADR 0276), sentence to append to its note:

> ADR 0273 (2026-11-12): body loads are sums over Element::integration_points (N and w det J at each point); the face functions of Tri3, Quad4 and Tri6 are not in the element library.

F-376 (ADR 0277), sentence to append to its note:

> ADR 0273 (2026-11-12): Element::strain_at(coords, u, xi) gives B u at a point; the integration points are stiffness_rule().points(); Material::stress(strain, &PointContext::default(), &[]) gives stress and tangent.

F-383 (ADR 0284), sentence to append to its note:

> ADR 0273 (2026-11-12): Element::mass_rule() proposes tet_4 (Tet4), gauss_3 (Hex8) and tet_14 (Tet10, degree 5), exact for straight-sided tetrahedra and every trilinear hexahedron; the consistent mass is the sum of rho N^T N w det J over Element::integration_points.

No follow-up items.