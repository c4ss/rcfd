# ADR 0277: Stress and strain recovery: integration-point, element and nodal values; von Mises and principal stresses

Status: **REGISTERED 2026-11-12, written before any code of F-376 and before any run of its checks; planned by ADR 0270**.
Not yet accepted by the maintainer. Before this text nothing of F-376 was written, compiled or run: the code and documents
under "What was read" were read, and the arithmetic under "Arithmetic before this registration" was done by hand; no script
and no library code was run. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Work package SB-7 of the structural solver, basic tier ([ADR 0270](0270-structural-solver-basic-tier-plan.md), section 3, the
entry for ADR 0277). Roadmap item F-376 (depends on F-374, done). Size S. Number 0277, fixed by the plan; the file name of the
planned stub is kept and the stub's text is kept unchanged at the end ("The planned stub, as written").
Branch `f376-stress-recovery` from main fd4f8e49, worktree `w49i-work/f376`.

House rules (`w49i-work/AGENT_RULES.md`):

- limits are fixed here, before the code and the runs, and are not changed after a run; a missed limit becomes an ignored
  `*_known_miss` test with the limit intact and the number recorded, diagnosed, not tuned away; a defective set-up gets a
  corrected test by an amendment written and committed before the corrected run, the old test stays as an ignored record;
- each registered run is made once; nothing is re-run to pick a result;
- defaults bit-identical: only the one-line stub `crates/rcfd-fem/src/recover.rs` is filled and one test file
  `crates/rcfd-fem/tests/sb_recover.rs` is added; no other file of `rcfd-fem` (in particular not `model.rs`, `element.rs`,
  `error.rs`), no file outside `rcfd-fem` except the topic page, no shared solver file, no golden, no hash table is touched;
- no new dependency (the crate keeps `rcfd-core`, `rcfd-mesh`, `rcfd-linalg`), no unsafe code, the graph stays acyclic;
- clean-room: everything below is derived here from first principles (the characteristic polynomial of a symmetric 3 x 3
  matrix, the triple-angle identity, Jacobi plane rotations, the principle of virtual work); papers are cited from memory for
  orientation only.

## What F-376 asks

The acceptance of `docs/roadmap.json`, binding:

> Every limit not stated here is registered in the ADR before the run. Strain and stress at integration points, element means
> and nodal averages; von Mises stress, principal stresses with directions, strain energy. For 1000 random symmetric tensors
> (registered seed) the principal values from the routine agree with an independent method (Jacobi rotations against the
> closed trigonometric form) within 1e-12 of the tensor norm, are ordered, have orthonormal directions within 1e-12 and are
> unchanged by a random rotation of the tensor within 1e-12; von Mises equals |s| in uniaxial stress, sqrt(3) tau in pure
> shear and 0 for a hydrostatic state within 1e-14 of the norm; a tensor with repeated roots gives finite values. On the patch
> test of the static solve every recovered stress equals D eps within 1e-10; the strain energy equals half the work of the
> loads within 1e-10 for a uniaxial and a bending load.

Clauses: (c1) strain and stress at the integration points; (c2) element means; (c3) nodal averages; (c4) von Mises stress;
(c5) principal stresses with directions; (c6) strain energy; (c7) 1000 random tensors: principal values agree with an
independent Jacobi method within 1e-12 of the tensor norm, the closed trigonometric form being compared against Jacobi as
well (both readings of the parenthesis are checked, P1 and P1t); (c8) ordered; (c9) orthonormal directions within 1e-12;
(c10) unchanged by a random rotation within 1e-12; (c11) von Mises `|s|` in uniaxial stress, (c12) `sqrt(3) tau` in pure
shear, (c13) 0 for a hydrostatic state, each within 1e-14 of the norm; (c14) a tensor with repeated roots gives finite values;
(c15) on the patch test every recovered stress equals `D eps` within 1e-10; (c16) the strain energy equals half the work of
the loads within 1e-10 for a uniaxial load and (c17) for a bending load.

The brief adds: `rcfd_linalg::eigen` (F-382) may serve as the independent method; state which tensors check "repeated roots
give finite values"; leave room for the VTK output of ADR 0278 (nodal von Mises and principal stresses, element stress
components) and for the patch recovery of ADR 0335 (integration-point values with their positions and weights).

## What was read (main fd4f8e49, read only)

1. `AGENT_RULES.md`; ADR 0270 sections 1, 2 (2.2 conventions: Voigt order xx, yy, zz, xy, yz, zx, engineering shear strain,
   determinism by element index, every refusal located; 2.3: `recover` is 0277's; 2.4: `SolidResult` carries integration-point
   values, derived fields come from `recover`), 3 (entries 0277 to 0281), 4, 5, 6, the lead's note and the decision record;
   this ADR's stub; ADR 0275 in full (patch test set-up D8, seed `0xF374_0275`, the note that the static solve returns no
   integration-point values); the stubs of ADR 0278 and ADR 0335; the roadmap notes of ADR 0273 and 0276 quoted in their
   addenda (`Element::strain_at`, `stiffness_rule().points()`, `Material::stress`; body loads by the mass rule).
2. Code: `crates/rcfd-fem/src/{lib.rs, model.rs (in full), element.rs (in full), material.rs, quadrature.rs, mesh.rs
   (ElementKind, reference nodes, public API), builders.rs (head), solve.rs, assemble.rs, loads.rs (head), constraint.rs
   (public API)}`; `crates/rcfd-fem/tests/sb_static.rs` (head: generator, patch helpers); `crates/rcfd-linalg/src/eigen.rs`
   (`symmetric_eigen`, the cyclic Jacobi with its stopping test `|a_pq| <= eps sqrt(|a_pp a_qq|)`, `DenseEigen`).
3. **Not read:** the bodies of the papers cited; any finite-element or eigensolver code outside this repository.

Findings that shape the design:

1. `SolidResult::with_integration_points(offsets, strain, stress)` exists (ADR 0271) and stores strain and stress only, no
   positions or weights. The recovery therefore has its own field type with positions and weights (ADR 0335 needs both) and
   a function that attaches strain and stress to a `SolidResult`; `model.rs` is not edited.
2. `Element::integration_points(coords, rule)` gives the physical position and `w det J` of each point;
   `Element::strain_at(coords, u, xi)` gives `B u`; `Material::stress(strain, &PointContext::default(), &[])` gives `D eps`
   (ADR 0281 adds the thermal strain there or in `recover.rs`).
3. `rcfd_linalg::eigen::symmetric_eigen(a, n)` is a cyclic Jacobi solver with values ascending and its own code path; it is
   independent of the closed form used here and is the reference of P1 and P1t.
4. `FemError` has `InvalidValue { key, reason }`, `FieldLength`, `UnknownSet` and `UnsupportedKind`; they suffice for every
   refusal of this package, so `error.rs` is not edited.

## Arithmetic before this registration (disclosed, by hand)

1. **The closed trigonometric form** (Smith, Comm. ACM 4 (1961) 168, from memory; derived here). For a symmetric `A` let `q =
   tr A / 3`, `B = A - q I`, `p = sqrt(tr(B^2) / 6)` and, for `p > 0`, `C = B / p`. `C` is trace-free with `tr C^2 = 6`, so
   its characteristic polynomial is `l^3 - (tr C^2 / 2) l - det C = l^3 - 3 l - det C`. With `l = 2 cos t` and `8 cos^3 t - 6
   cos t = 2 cos 3t` the roots satisfy `cos 3t = det C / 2 =: r`; real roots give `|r| <= 1` (clamped against rounding). With
   `phi = acos(r) / 3` in `[0, pi/3]`: `b1 = 2 cos phi` in `[1, 2]` (largest), `b3 = 2 cos(phi + 2 pi/3)` in `[-2, -1]`
   (smallest), `b2 = -b1 - b3` (trace zero); the eigenvalues of `A` are `q + p b_k`. With `sum b = 0`, `sum b^2 = 6` the spread
   `b1 - b3` lies in `[3, 2 sqrt 3]` (3 for `(1, 1, -2)`, `2 sqrt 3` for `(sqrt 3, 0, -sqrt 3)`).
2. **Its accuracy near a repeated root.** `d b3 / d r = -2 sin(phi + 2 pi/3) d phi / d r` with `d phi / d r = -1 / (3 sqrt(1 -
   r^2))`, unbounded as `r -> 1` (b2 = b3) or `r -> -1` (b1 = b2); `d b1 / d r` stays bounded near `r = 1`. With the gap `d =
   b2 - b3 ~ 2 sqrt 3 phi` and `sqrt(1 - r^2) ~ 3 phi` near `r = 1`, a rounding `dr` of `r` moves the close pair by `~ 0.67
   dr / d`. `r = det C / 2` is computed from entries of size 1 with `dr ~ 4 eps ~ 1e-15`. Error in units of `p`: `~ 6.7e-16 /
   d`; relative to the tensor norm (`p <= |A| / sqrt 6`): `~ 2.7e-16 / d`. Exactly repeated roots (`d ~ 0`): the error is
   `~ sqrt(dr) ~ 3e-8` (from `acos(1 - x) ~ sqrt(2 x)`). Hence the routine (D3) does not take the close pair from the closed
   form; it uses the closed form only for the separated root and its direction.
3. **Probability that the bare closed form misses 1e-12 on random tensors.** It needs `d < 2.7e-4` (in units of `p`). For 3
   x 3 random symmetric matrices the spacing distribution vanishes linearly at zero (eigenvalue repulsion; Wigner's surmise for
   the orthogonal ensemble, `P(s) = (pi/2) s exp(-pi s^2 / 4)`, from memory), so `P(s < x) ~ (pi/4) x^2`; with the mean gap
   `~ 1.6` in units of `p`, `x ~ 1.7e-4` and `P ~ 2.3e-8` per tensor, `~ 2e-5` for 1000. P1t (the bare closed form against
   Jacobi) is registered at 1e-12 on that estimate.
4. **Direction of the separated root.** The root farther from the middle one has gap `>= (b1 - b3)/2 >= 1.5` (arithmetic 1)
   and its closed-form value an error `~ eps` (bounded derivative). `C - b I` then has rank 2; the cross product of two of its
   rows is orthogonal to both and so parallel to the null vector. The three cross products of pairs of rows are the rows of
   the adjugate of `C - b I`, which for a symmetric matrix of eigenvalues `0, m_i, m_j` with null vector `e` is `m_i m_j e
   e^T`; so their squared norms sum to `(m_i m_j)^2` and the largest has norm `>= |m_i m_j| / sqrt 3`. Here `|m_i| >= 1.5`
   (the nearer root) and `|m_j| >= 3` (the spread): norm `>= 4.5 / sqrt 3 = 2.6`. Entries of `C - b I` are `<= 4`; rounding
   of the cross product `~ 2 x 16 eps`; direction error `~ 32 eps / 2.6 ~ 3e-15`.
5. **The other two in the plane orthogonal to it.** With `e` the unit direction, `u = e x a / |e x a|` (a the coordinate axis
   of smallest `|e_k|`, so `|e x a| >= sqrt(2/3)`) and `v = e x u` form an orthonormal frame to rounding. The 2 x 2 matrix
   `M = [u v]^T C [u v]` has the remaining two roots as eigenvalues up to the error of `e` (`O(eps)` in units of `p`), however
   close they are; one Jacobi rotation diagonalises it exactly in exact arithmetic: `tau = (m22 - m11) / (2 m12)`, `t =
   sign(tau) / (|tau| + sqrt(1 + tau^2))` (`t = 1` for `tau = 0`; no rotation for `m12 = 0`), `c = 1 / sqrt(1 + t^2)`, `s = t
   c`; values `m11 - t m12` and `m22 + t m12`, vectors `c u - s v` and `s u + c v` (Golub and Van Loan, Matrix Computations,
   the symmetric Schur decomposition, from memory; the same formulas as `rcfd_linalg::eigen`'s Jacobi, derived again here from
   `tan 2 theta = 2 m12 / (m11 - m22)`). The rotation is orthogonal to rounding, so the three directions are orthonormal within
   `~ 4 eps ~ 1e-15` (limit 1e-12) whatever the gaps. The separated value is taken as the Rayleigh quotient `e^T C e`. The
   construction is that of a published robust 3 x 3 eigensolver (D. Eberly, "A Robust Eigensolver for 3 x 3 Symmetric
   Matrices", Geometric Tools, 2014, from memory); nothing of it is copied, it is derived above.
6. **Error of the routine against Jacobi.** Routine: values to `~ 10 eps` in units of `p` plus the rounding of `A - q I`
   (`eps |q|`); scaled by the largest entry first, so no overflow. Jacobi of `rcfd_linalg`: values to `~ eps |A|` (its
   stopping test). Difference relative to the Frobenius norm `|A|`: `~ 1e-15`; limit 1e-12, margin `~ 1e3`. Under a rotation
   `R A R^T` computed in floating point, each entry carries `~ 9 eps |A|` of rounding; by Weyl's inequality the eigenvalues move
   by at most the norm of that perturbation, `~ 3e-15 |A|`; limit 1e-12.
7. **Residual.** `|A v - l v| / |A|` for each pair: the separated pair `~ 3e-15` (arithmetic 4), the plane pair backward
   stable, `~ 1e-15`. Registered extension at 1e-12.
8. **Tensor norm.** The Frobenius norm of the 3 x 3 tensor: `|s| = sqrt(s_xx^2 + s_yy^2 + s_zz^2 + 2 (s_xy^2 + s_yz^2 +
   s_zx^2))`. It is `|sigma|` for uniaxial stress, `sqrt 2 |tau|` for pure shear, `sqrt 3 |p|` for a hydrostatic state.
9. **von Mises.** `s_vm = sqrt(((s_xx - s_yy)^2 + (s_yy - s_zz)^2 + (s_zz - s_xx)^2) / 2 + 3 (s_xy^2 + s_yz^2 + s_zx^2))`
   (`sqrt(3 J2)`, `J2 = s':s' / 2` of the deviator; derivation: `s':s' = sum (s_ii - q)^2 + 2 sum s_ij^2` and `sum (s_ii -
   q)^2 = sum_{i<j} (s_ii - s_jj)^2 / 3`). Uniaxial `sigma` on an axis: `sqrt((sigma^2 + sigma^2)/2) = sqrt(sigma^2)`, one or
   two roundings: `<= 2.2e-16` relative. Pure shear: `sqrt(3 tau^2)` against `sqrt(3) |tau|`: `<= 4.4e-16` relative.
   Hydrostatic: every difference is exactly 0 and the shear terms are 0: exactly 0. Limit 1e-14 of the norm: margin `~ 20`
   to `~ 50`. A uniaxial stress along a rotated axis (extension V4) carries the rotation's rounding, `~ 9 eps` per entry;
   estimate `~ 3e-15`, limit 1e-13. Consistency `s_vm = sqrt(((l1 - l2)^2 + (l2 - l3)^2 + (l3 - l1)^2)/2)` with the principal
   values: `~ 1e-15` (V5, limit 1e-12).
10. **Extrapolation from the integration points to the nodes** (Hinton and Campbell, Int. J. Numer. Meth. Engng 8 (1974) 461,
    local smoothing, from memory). Per kind, a polynomial space with exactly as many functions as the stiffness rule has points,
    unisolvent on the points, is fitted to the point values and evaluated at the nodes' reference coordinates: `E = P(nodes)
    V^{-1}`, `V_qj = P_j(xi_q)`. Tet4 (`tet_1`, 1 point): `{1}`, every node takes the point value. Tet10 (`tet_4`, 4 points
    `(b, a, a, a)` and permutations, `a = (5 - sqrt 5)/20`, `b = 1 - 3a`): `{1, xi, eta, zeta}`. By hand: a linear `f = sum c_i
    L_i` has `f_i = (b - a) c_i + a S` at point i (`S = sum c`), summed `sum f = (b + 3a) S = S`, so the corner value `c_i =
    sqrt 5 f_i - ((sqrt 5 - 1)/4) sum_j f_j` (`b - a = 1/sqrt 5`), i.e. `1.927 f_i - 0.309` times each other point; row sum 1,
    absolute row sum 2.854; mid-edge node `(c_i + c_j)/2`. Hex8 (`gauss_2`, points `+-g`, `g = 1/sqrt 3`): `{1, xi, eta,
    zeta, xi eta, eta zeta, zeta xi, xi eta zeta}`; along one axis the value at `+-1` from the points `+-g` is `((1 + sqrt 3)/2)
    f_near + ((1 - sqrt 3)/2) f_far = 1.366 f_near - 0.366 f_far`; the 3-D row is the product, absolute row sum `(sqrt 3)^3 =
    5.2`, row sum 1. `V^{-1}` is computed once per kind by Gauss-Jordan elimination with partial pivoting (cubes and the tet_4
    simplex: `V` well conditioned). A constant field is reproduced to `~ 5.2 x few eps` (rounding), a field in the polynomial
    space exactly: a physical linear field is linear in the reference coordinates on straight-sided Tet10 and trilinear on any
    trilinear Hex8 (`x(xi)` is trilinear), so on those it is reproduced at every node (X1).
11. **Element means and nodal averages.** Element mean `sum_q w_q det J_q v_q / sum_q w_q det J_q` (the volume mean by the
    element's rule; for a field linear in `x` on a trilinear Hex8 the integrand `f det J` has degree `<= 3` per variable, so
    `gauss_2` gives `f(centroid)` exactly, X5). Nodal average: `sum_e V_e v_e(node) / sum_e V_e` over the elements of the
    averaging region containing the node, `v_e(node)` the extrapolated value of D3 and `V_e = sum_q w_q det J_q` the element
    volume by the rule (exact for Tet4, straight Tet10 and trilinear Hex8: `det J` of degree `<= 2` per variable). Weighted by
    volume as the plan says; a node with no element in the region has no value (`None`).
12. **Patch test** (ADR 0275 arithmetic 5): on its meshes the solution is the linear field to `~ 1e-15` (ADR 0275 P1, measured
    4.4e-16 to 1.2e-15 of the field maximum `~ 3e-3`). The strain `B u_e` sums terms `grad N_a u_a` of size `|u| / h ~ 3e-3 /
    0.27 ~ 1e-2` to a strain `~ 1e-3`: rounding `~ 10 eps` relative, plus the propagated displacement error `~ 1e-15 x 3e-3 /
    0.27 / 1e-3 ~ 1e-14` relative. Stress `D eps` adds `~ eps`; the extrapolation (absolute row sum 5.2) and averaging keep it
    `<~ 1e-13`. Limit 1e-10 (the acceptance's), relative to `max |D eps|` over the six components: margin `~ 1e3`.
13. **Strain energy and Clapeyron.** `U = sum_e sum_q w_q det J_q eps_q . sigma_q / 2` with Voigt vectors (engineering shear:
    `eps . sigma = eps_ij sigma_ij`). With the stiffness rule this is exactly `u^T K u / 2` in exact arithmetic (`K_e = sum_q
    w_q det J_q B^T D B`). `K u = f + r` with reactions `r` only at prescribed equations; all prescribed values here are 0, so
    `u^T K u = u^T f + u_F^T (K_FF u_F - f_F)`, and `|u_F^T res| <= |u| rho |f|` with `rho` the relative residual of the
    direct solve (ADR 0275 measured 2.4e-16 to 9.5e-14). Relative to `W = f . u` (`f . u / (|f| |u|) = O(1)` for a tip load
    and for end tractions): `<~ 1e-13`. Rounding of `B u` in bending: the rigid rotation part of `u` cancels in `B u`; the tip
    displacement `~ kappa L^2 / 2` against the strain `~ kappa h / 2` times `1 / h_e` gives a cancellation factor `L^2 / (h
    h_e) ~ 2^2 / (0.2 x 0.1) = 200`, so `~ 200 eps ~ 4e-14` relative. Limit 1e-10 (the acceptance's): margin `~ 1e3`. Closed
    form of the uniaxial bar (W2): `U = sigma^2 V / (2 E)`, the solution being the exact linear field (ADR 0275 U1 to `1e-13`):
    `~ 1e-13`, limit 1e-10. `U` against `u^T K u / 2` with the assembled `K` (W4): rounding of `K u` cancels by `~ |u|^T |K|
    |u| / u^T K u ~ 1e3` to `1e4` in bending: `~ 1e-12`, limit 1e-10.
14. **Numbers of the energy checks.** Uniaxial bar `[0, 2] x [0, 0.5] x [0, 0.4]` m (ADR 0275 arithmetic 10), divisions `[6, 2,
    2]`, `E = 2e11 Pa`, `nu = 0.3`, end tractions `+-5e6 Pa` along x on `xmax` / `xmin` (`Load::traction`, ADR 0276) and the
    3-2-1 support of ADR 0275 (P1 all three at `(0, 0, 0)`, P2 x and y at `(0, 0, 0.4)`, P3 x at `(0, 0.5, 0)`, all zero):
    `U = (5e6)^2 x 0.4 / (2 x 2e11) = 25 J`, `W = 2 U = 50 J`. Cantilever `[0, 2] x [0, 0.2] x [0, 0.2]` m, divisions `[10, 2,
    2]` (Tet10 from the Tet4 box), `xmin` clamped (`Constraint::displacement` on the three components), traction `(0, 0, -1e6)
    Pa` on `xmax` (resultant `-4e4 N`): `U ~ P^2 L^3 / (6 E I) = (4e4)^2 x 8 / (6 x 2e11 x 1.33e-4) ~ 80 J` (Tet4 locks and
    gives less; the check is Clapeyron's identity, not the beam value).

## Design

### D1. Integration-point field (`recover.rs`)

```rust
pub struct IntegrationPointField { offsets, positions: Vec<Vec3>, weights: Vec<f64>, strain: Vec<[f64; 6]>, stress: Vec<[f64; 6]> }
impl IntegrationPointField {
    pub fn compute(model: &SolidModel, displacement: &[f64]) -> Result<IntegrationPointField>;
    pub fn from_result(model: &SolidModel, result: &SolidResult) -> Result<IntegrationPointField>;
    pub fn offsets(&self) -> &[usize]; pub fn positions(&self) -> &[Vec3]; pub fn weights(&self) -> &[f64];
    pub fn strain(&self) -> &[[f64; 6]]; pub fn stress(&self) -> &[[f64; 6]];
    pub fn n_elements(&self) -> usize; pub fn points_of(&self, element: usize) -> Range<usize>;
    pub fn element_volume(&self, element: usize) -> f64;
    pub fn element_means(&self, values: &[[f64; 6]]) -> Result<Vec<[f64; 6]>>;
    pub fn element_strain_energy(&self) -> Vec<f64>; pub fn strain_energy(&self) -> f64;
}
pub fn attach(result: SolidResult, field: &IntegrationPointField) -> Result<SolidResult>;
```

For every element in element order and every point of `element_for(kind).stiffness_rule()` in the rule's order: position and
`w det J` from `Element::integration_points`, strain from `Element::strain_at(coords, u_e, xi)` with `u_e` gathered by
`assemble::element_dofs`, stress from `Material::stress(strain, &PointContext::default(), &[])` (the element's material by
`SolidModel::material_indices`). `offsets[e]..offsets[e + 1]` are the points of element `e`. A displacement of the wrong
length is refused (`FieldLength { key: "displacement" }`); an element error is returned as `FemError::Element`. Sequential,
fixed order: no dependence on threads. `attach` stores strain and stress in the result by
`SolidResult::with_integration_points` (bit for bit). Element means and nodal averages take any per-point value array of the
field's length (strain, stress, or a caller's field), so ADR 0335 and 0281 reuse them. `element_means` refuses a wrong
length (`FieldLength { key: "values" }`).

### D2. Strain energy

`element_strain_energy()[e] = sum_q w_q det J_q (eps_q . sigma_q) / 2` with the plain dot product of the Voigt vectors
(arithmetic 13); `strain_energy()` their sum in element order.

### D3. Nodal values

```rust
pub fn extrapolation_matrix(kind: ElementKind) -> Result<&'static [Vec<f64>]>;   // rows: nodes, columns: points
pub fn element_nodal_values(model, field, values) -> Result<Vec<Vec<[f64; 6]>>>;  // per element, per node in its order
pub fn nodal_average(model, field, values, region: Option<&str>) -> Result<Vec<Option<[f64; 6]>>>;
```

Extrapolation by arithmetic 10 (`OnceLock` per kind). `nodal_average`: over the elements of the element set `region` (all
elements for `None`; an unknown set is refused `UnknownSet`), volume-weighted (arithmetic 11), sequential in element order.
**Material interfaces are refused**: if a node is shared by elements of the region whose materials differ (compared by value,
`Material: PartialEq`), the call is refused, `InvalidValue { key: "nodal average", reason }` naming the node and the two
material sets, with the advice to average over one element set at a time. Elements of equal materials in different sets
average together.

### D4. Invariants

```rust
pub fn tensor_norm(s: &[f64; 6]) -> f64;                 // arithmetic 8
pub fn von_mises(s: &[f64; 6]) -> f64;                   // arithmetic 9, the formula as written
pub struct Principal { pub values: [f64; 3], pub directions: [Vec3; 3] }
pub fn principal(s: &[f64; 6]) -> Principal;
pub fn principal_values_trigonometric(s: &[f64; 6]) -> [f64; 3];
```

`principal`: scale by `m = max |s_ij|` (zero tensor: values 0, directions the axes); `q`, `B`, `p` (arithmetic 1; `p = 0`:
values `q`, directions the axes); `C = B / p`; closed form `b1, b2, b3` with `r` clamped to `[-1, 1]`; the separated root is
`b1` if `b1 - b2 >= b2 - b3`, else `b3`; its direction by the largest cross product of rows of `C - b I` (arithmetic 4; if
that cross product is exactly zero, which arithmetic 4 excludes for `p > 0`, the axes are used); the plane pair by the Jacobi
rotation of arithmetic 5; the separated value as `e^T C e`. Values `m (q + p mu)`, sorted **descending** (`values[0]` the
largest principal stress), directions with the values; each direction with its entry of largest modulus positive (first such
index). Directions are a frame, not necessarily right-handed. `principal_values_trigonometric`: the bare closed form
(arithmetic 1, descending), kept public for the record P1t and the repeated-root record R2.

The stress `s` is a Voigt stress (tensor shear components); for a strain, the caller halves the shear components first
(documented; strains are not passed to `principal` in this package).

### D5. Test set-up (`tests/sb_recover.rs`)

* Generator: SplitMix64 as ADR 0273 D6; **seed `0xF376_0277`**, streams `SEED + tag`: random tensors tag 1, rotations tag 2,
  von Mises samples tag 3, repeated-root rotations tag 4, extrapolation and averaging meshes tag `10 + k` (Tet4 0, Hex8 1,
  Tet10 2). The patch test uses the meshes of ADR 0275 exactly (its seed `0xF374_0275`, tags `10 + k`, origin `(0.1, -0.2,
  0.3)`, lengths `(1.2, 1.0, 0.8)`, divisions `[4, 3, 3]`, interior perturbation `0.1 h`, its linear field `u = c + A x` with
  the six strains `(1.0, -0.7, 0.9, 0.8, 1.0, -0.1) 1e-3`), solved by `solve_static_with` with the boundary values prescribed.
* Random tensors: six components uniform in `[-1, 1]` times `10^k`, `k` uniform in `[-6, 9]` per tensor (scale range of
  stresses in Pa and below). Random rotation: a unit quaternion from four components uniform in `[-1, 1]` accepted when the
  norm lies in `(0.1, 1]`, normalised; `R` from it; `R S R^T` in floating point.
* **Repeated-root tensors (clause c14)**, each with its exact roots: (a) the zero tensor (triple root 0); (b) hydrostatic `p
  I`, `p = 3.7e5` and `p = -2` (triple); (c) uniaxial `diag(sigma, 0, 0)`, `sigma = 2.5e8` and `-1` (double root 0); (d) `diag(a,
  a, b)` with `(a, b) = (1e8, -3e8)` and `(1e8, 2e8)`, each unrotated and under 10 random rotations (double root); (e) `p I`
  under 10 random rotations, `p = 3.7e5` (triple root; the rounding of the rotation leaves a deviator of `~ eps p`, the
  hardest input for the closed form); (f) near-repeated: `diag(a, a (1 + d), b)` with `d = 1e-9` and `d = 1e-15`, `(a, b) =
  (1e8, -3e8)`, under 10 random rotations each. 54 tensors in total.
* Material `E = 2.0e11 Pa`, `nu = 0.3`, density 7850, expansion 0.

## Registered checks

All in `crates/rcfd-fem/tests/sb_recover.rs`, release profile, one run (`cargo test -p rcfd-fem --release --test sb_recover
-- --nocapture --test-threads=1`). Every printed number is the maximum over the items concerned. "Norm" is the tensor norm of
arithmetic 8 of the tensor concerned.

| Id | Clause | Check | Limit | Estimate behind it |
| --- | --- | --- | --- | --- |
| P1 | c5, c7 | 1000 random tensors: `max_k |l_k - l_k^Jacobi| / norm`, `l^Jacobi` from `rcfd_linalg::eigen::symmetric_eigen` sorted descending | `<= 1e-12` | arithmetic 6: `~ 1e-15` |
| P1t | c7 | the same with the bare closed form `principal_values_trigonometric` in place of the routine | `<= 1e-12` | arithmetic 3: a miss has probability `~ 2e-5` |
| P2 | c8 | 1000 random tensors: `values[0] >= values[1] >= values[2]` | every tensor | D4 sort |
| P3 | c5, c9 | 1000 random tensors: `max |d_i . d_j - delta_ij|` | `<= 1e-12` | arithmetic 5: `~ 1e-15` |
| P4 | c10 | 1000 random tensors, each with its own random rotation: `max_k |l_k(R S R^T) - l_k(S)| / norm` | `<= 1e-12` | arithmetic 6: `~ 3e-15` |
| V1 | c4, c11 | 100 values `sigma` (uniform in `[-1, 1]` times `10^k`, `k` in `[-6, 9]`) on each of the three axes: `|s_vm - |sigma|| / norm` | `<= 1e-14` | arithmetic 9: `<= 2.2e-16` |
| V2 | c4, c12 | the same values as `tau` in each of xy, yz, zx: `|s_vm - sqrt(3) |tau|| / norm` | `<= 1e-14` | arithmetic 9: `<= 4.4e-16` |
| V3 | c4, c13 | the same values as `p` in `p I`: `s_vm / norm` | `<= 1e-14` | arithmetic 9: exactly 0 |
| R1 | c14 | the 54 repeated-root tensors of D5: every value and direction component finite; ordered; `max |d_i . d_j - delta_ij| <= 1e-12`; `max_k |l_k - l_k^exact| / norm <= 1e-12` (exact roots sorted descending; the zero tensor: values exactly 0) | as stated | arithmetic 5, 6 |
| S1 | c1, c15 | patch test, three kinds: at every integration point `max_c |sigma_c - (D eps)_c| / max_c |(D eps)_c|`, and the same for the strain against `eps` | `<= 1e-10` each | arithmetic 12: `<~ 1e-13` |
| S2 | c2, c15 | patch test: element means of stress and strain, same measure | `<= 1e-10` each | arithmetic 12 |
| S3 | c3, c15 | patch test: nodal averages (`region = None`) of stress and strain at every node, same measure; every node has a value | `<= 1e-10` each | arithmetic 10, 12 |
| W1 | c6, c16, c17 | uniaxial bar and cantilever of arithmetic 14, three kinds: `|U - W/2| / (W/2)`, `U = strain_energy()`, `W = f . u`, `f = nodal_load_vector` | `<= 1e-10` | arithmetic 13: `<~ 1e-13` |

**Registered extensions** (a miss is a `*_known_miss` record and a follow-up item; alone it does not block done):

| Id | Check | Limit | Estimate behind it |
| --- | --- | --- | --- |
| P5 | 1000 random tensors: `max_k |S d_k - l_k d_k| / norm` (each direction is an eigenvector of its value) | `<= 1e-12` | arithmetic 7 |
| R2 | the 54 repeated-root tensors with the bare closed form: max error against the exact roots / norm | record (estimate `~ 3e-8`) | arithmetic 2 |
| V4 | the V1 values along the axis `R e_x` of a random rotation (`R diag(sigma, 0, 0) R^T`): `|s_vm - |sigma|| / norm` | `<= 1e-13` | arithmetic 9: `~ 3e-15` |
| V5 | 1000 random tensors: `|s_vm - sqrt(((l1 - l2)^2 + (l2 - l3)^2 + (l3 - l1)^2)/2)| / norm` | `<= 1e-12` | arithmetic 9 |
| W2 | uniaxial bar, three kinds: `|U - sigma^2 V / (2 E)| / (sigma^2 V / (2 E))` | `<= 1e-10` | arithmetic 13 |
| W4 | uniaxial bar and cantilever, three kinds: `|U - u^T K u / 2| / (u^T K u / 2)` with `K = assemble::stiffness` | `<= 1e-10` | arithmetic 13: `~ 1e-12` |
| X1 | extrapolation exactness: on the perturbed patch meshes of tag `10 + k` (Hex8 perturbed, Tet10 straight-sided from the perturbed Tet4), the values of the linear field `g(x) = (x, y, z, x + y, y - z, 2 z - x) 1e3 + (1, 2, 3, 4, 5, 6)` given at the integration-point positions, extrapolated by `element_nodal_values`: `max |v_e(node) - g(x_node)| / max |g|` over every element and node (Hex8 and Tet10); Tet4 the same with a constant field | `<= 1e-13` | arithmetic 10: `~ 1e-15` |
| X2 | the same field through `nodal_average`: every node | `<= 1e-13` | arithmetic 10, 11 |
| X3 | volume weighting: per-point values equal to the element index `e` (all six components) on the perturbed Tet4 patch mesh; `nodal_average` against the test's own `sum V_e e / sum V_e` with `V_e = SolidMesh::element_volume` | `<= 1e-14` relative | exact up to rounding of the sums |
| X4 | material interface: a Hex8 box `[2, 1, 1]` with element sets `left` (element 0) and `right` (element 1) of different `E`: `nodal_average(None)` refused naming a shared node and both sets; with `Some("left")` accepted, nodes of the right face of element 1 `None`; with equal materials on both sets `nodal_average(None)` accepted | as stated | D3 |
| X5 | element means of the field `g` of X1 on the perturbed Hex8 patch: `max |mean_e - g(c_e)| / max |g|`, `c_e` the volume centroid computed in the test with `gauss_3` | `<= 1e-13` | arithmetic 11 |
| A1 | `attach`: the result's `strain()`, `stress()` and `integration_point_offsets()` equal the field's bit for bit (patch, three kinds) | equal | D1 |
| E1 | refusals: a displacement of the wrong length (`FieldLength`), values of the wrong length for `element_means` and `nodal_average` (`FieldLength`), an unknown region (`UnknownSet`) | as stated | D1, D3 |

**Negative controls** (each must hold, else the check is not sensitive):

| Id | Control | Expected |
| --- | --- | --- |
| N1 | S1's stress measure with the stress of `nu = 0.25` (`D'` of another material) in place of `D eps` | `> 1e-3` |
| N2 | W1's measure with `W` in place of `W/2` (the factor 1/2 of Clapeyron left out) | `> 0.4` |
| N3 | P1's measure with the Jacobi values sorted ascending against the routine's descending (ordering mismatch) | `> 1e-3` |

**Mutations** (on a `git archive` copy of the final tree in `w49i-work/f376-scratch`, own target directory; each applied alone
by exact text replacement, `sb_recover` run once, the file restored and compared byte for byte):

| Id | Mutation | Must fail |
| --- | --- | --- |
| M1 | strain energy density with the three shear products halved (tensor instead of engineering shear) | W1 |
| M2 | von Mises: the shear coefficient 3 replaced by 6 | V2 |
| M3 | `principal`: the plane pair's directions by cross products of rows (as for the separated root) instead of the 2 x 2 rotation | R1 (orthonormality) or P3 |
| M4 | `principal`: values sorted ascending | P1, P2 |
| M5 | `principal`: the bare closed-form values returned (no refinement) | R1 (values against the exact roots) |
| M6 | Hex8 extrapolation: the Gauss coordinate `1/sqrt 3` replaced by 1 in the fitted basis (point values copied to the nodes) | X1 |
| M7 | `nodal_average`: plain mean instead of volume weighting | X3 |
| M8 | `nodal_average`: the material-interface test removed | X4 |
| M9 | `element_means`: plain mean of the points instead of weighting by `w det J` | X5 |
| M10 | integration-point strain taken at the first point of the rule for every point | W1 (Hex8, Tet10) |
| M11 | `principal`: the clamp of `r` to `[-1, 1]` removed | R1 (a NaN), or recorded as surviving if no registered tensor gives `|r| > 1` by rounding |

**Decision rule.** F-376 is proposed **done** (pending the maintainer's acceptance) if P1, P1t, P2, P3, P4, V1, V2, V3, R1,
S1, S2, S3 and W1 are met for every kind and case (every acceptance clause), N1 to N3 hold, and every mutation fails its test
(M11 may survive in the way stated; that is recorded and is not a miss of the rule). A miss among the extensions is a
`*_known_miss` record with a follow-up item and does not alone block done. A miss or an unrun check among the acceptance
checks, a failed negative control, or an uncaught mutation other than M11 (after checking that the test covers it):
**partial**, with the miss named. No crate that builds: **todo**.

## Limits (what is not covered)

- Recovery uses the stiffness rule's points only; values at other points (for error norms of higher degree, ADR 0279) are the
  caller's, through `Element::strain_at`.
- Nodal values by local extrapolation and volume-weighted averaging only; no superconvergent patch recovery and no error
  estimate (ADR 0335). Extrapolation amplifies point-to-point variation (absolute row sums 2.9 for Tet10, 5.2 for Hex8), so
  nodal peaks of a steep field overshoot; that is the method, not a defect.
- Averaging across a material interface is refused, not handled; the caller averages per element set.
- No thermal strain (ADR 0281 adds `sigma = D (eps - eps_th)` here or in the constitutive call).
- `principal` takes a stress (tensor shear components). Principal strains need the caller to halve the engineering shear.
- Principal directions carry a sign convention (largest-modulus entry positive) and need not form a right-handed frame; for a
  repeated root any orthonormal basis of its eigenspace is returned.
- The 1000 tensors are random with uniform components; the accuracy near repeated roots is checked on the 54 constructed
  tensors of D5, not proved for every input.

## Open points taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. The integration-point field is a type of `recover.rs` with positions and weights, and `attach` puts strain and stress into
   `SolidResult` through its existing builder; `model.rs` is not edited.
2. Nodal values by extrapolation from the points with a fitted polynomial per kind, then volume-weighted averaging (the
   plan's wording), rather than copying element means to the nodes.
3. The material-interface refusal compares materials by value, so equal materials on two sets average together.
4. Principal values descending (`values[0]` the largest); direction sign by the largest-modulus entry, as `rcfd_linalg::eigen`.
5. The routine refines the closed form by the separated root's direction and a 2 x 2 rotation (arithmetic 2 to 5); the bare
   closed form stays public as `principal_values_trigonometric`.
6. Refusals use the existing `FemError` variants; `error.rs` is not edited.

## The planned stub, as written

Status: **PLANNED 2026-11-12** (layout by the basic-tier plan, [ADR 0270](0270-structural-solver-basic-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are
the acceptance draft that the registration turns into checks.

Work package SB-7 of the structural solver, basic tier. Size S.

### Goal

From a displacement field: strain and stress at the integration points, element means, nodal values by extrapolation and
volume-weighted averaging; von Mises stress, principal stresses with their directions, and the strain energy. The six components
are stored in the order xx, yy, zz, xy, yz, zx with engineering shear strains (fixed by ADR 0270 section 2.2).

### Dependencies

ADR 0275; existing roadmap items: none.

### Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. Strain and stress at integration points, element means and
nodal averages; von Mises stress, principal stresses with directions, strain energy. For 1000 random symmetric tensors
(registered seed) the principal values from the routine agree with an independent method (Jacobi rotations against the closed
trigonometric form) within 1e-12 of the tensor norm, are ordered, have orthonormal directions within 1e-12 and are unchanged by
a random rotation of the tensor within 1e-12; von Mises equals |s| in uniaxial stress, sqrt(3) tau in pure shear and 0 for a
hydrostatic state within 1e-14 of the norm; a tensor with repeated roots gives finite values. On the patch test of the static
solve every recovered stress equals D eps within 1e-10; the strain energy equals half the work of the loads within 1e-10 for a
uniaxial and a bending load.

### Reference to check against

Closed forms: invariants of a symmetric 3 x 3 tensor; Clapeyron's theorem (strain energy = half the work of the loads in linear
elasticity).

### Files

`crates/rcfd-fem/src/recover.rs`, `crates/rcfd-fem/tests/sb_recover.rs`.

### What could make it miss

Loss of accuracy of the trigonometric form near repeated roots; nodal averaging across a material interface (basic tier: one
material per averaging region, otherwise refused). Superconvergent patch recovery and error estimation are intermediate.
