# ADR 0281: Thermal strain: nodal temperature field, expansion coefficient, thermal load and stress

Status: **REGISTERED 2026-11-12, written before any code of F-380 and before any run of its checks; planned by ADR 0270**.
Not yet accepted by the maintainer. Before this text nothing of F-380 was written, compiled or run: the code and documents
under "What was read" were read, and the arithmetic under "Arithmetic before this registration" was done by hand; no script
and no library code was run. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Work package SB-11 of the structural solver, basic tier ([ADR 0270](0270-structural-solver-basic-tier-plan.md), section 3, the
entry for ADR 0281). Roadmap item F-380 (depends on F-375, done pending acceptance, and F-376, partial). Size S. Number 0281,
fixed by the plan; the file name of the planned stub is kept and the stub's text is kept unchanged at the end ("The planned
stub, as written"). Branch `f380-thermal-strain` from main d4bde1e0, worktree `w49i-work/f380`.

House rules (`w49i-work/AGENT_RULES.md`):

- limits are fixed here, before the code and the runs, and are not changed after a run; a missed limit becomes an ignored
  `*_known_miss` test with the limit intact and the number recorded, diagnosed, not tuned away; a defective set-up gets a
  corrected test by an amendment written and committed before the corrected run, the old test stays as an ignored record;
- each registered run is made once; nothing is re-run to pick a result;
- defaults bit-identical: only files of `rcfd-fem` are edited (`src/thermal.rs` filled, one arm of `src/loads.rs`, the
  stress and energy of `src/recover.rs`), two test files are added; no file outside `rcfd-fem`, no shared solver file, no
  golden, no hash table is touched; a model without a temperature load takes the code paths it took before (check B1);
- no new dependency, no unsafe code, the graph stays acyclic;
- clean-room: everything below is derived here from first principles (Hooke's law with an isotropic thermal strain, the
  principle of virtual work, compatibility of a linear strain field); Timoshenko and Goodier, Theory of Elasticity, the chapter
  on thermal stress, is cited from memory for orientation only.

## What F-380 asks

The acceptance of `docs/roadmap.json`, binding:

> Every limit not stated here is registered in the ADR before the run. With a nodal temperature field, an expansion
> coefficient and a reference temperature: a uniformly heated, statically determinately held box expands by alpha dT x at
> every node within 1e-10 with stresses below 1e-10 E alpha dT; a box held on all faces has the hydrostatic stress -E alpha
> dT/(1 - 2 nu) within 1e-10; a bar held at both ends with free sides has the axial stress -E alpha dT within 1e-10 (nu =
> 0.3); a linear temperature through the thickness of a free beam gives the curvature alpha dT/h within 1 percent and
> stresses below 1 percent of E alpha dT (Tet10); a model without temperature is bit-identical to the same model before this
> change.

Clauses: (c1) free expansion of a statically determinately held box, `u = alpha dT x` at every node within 1e-10, (c2) its
stresses below 1e-10 E alpha dT; (c3) a box held on all faces, hydrostatic stress `-E alpha dT/(1 - 2 nu)` within 1e-10;
(c4) a bar held at both ends with free sides, axial stress `-E alpha dT` within 1e-10, nu = 0.3; (c5) a free beam with a
linear temperature through its thickness, curvature `alpha dT/h` within 1 percent, (c6) its stresses below 1 percent of E
alpha dT, Tet10; (c7) a model without temperature bit-identical to the same model before this change.

The brief adds: replace the `Load::Temperature` arm of `loads.rs` (refused today with `UnsupportedLoad` naming ADR 0281);
the recovered stress subtracts the thermal strain, `sigma = D (eps - eps_th)` (ADR 0277); define the bit-identity comparison
here; ADR 0275's check E3 (`sb_static.rs`) asserts the thermal refusal and will fail: do not edit it, propose a replacement
(or the removal of the assertion if no unsupported load remains), run it once on a scratch copy as a registered extension
check and report it, as ADR 0276 did with its X4.

## What was read (main d4bde1e0, read only)

1. `AGENT_RULES.md`; ADR 0270 (sections 1, 2.2 to 2.5, the entries 0276, 0277, 0281 of section 3); this ADR's stub; ADR
   0276 in full (the pattern of its finding 1 and X4 followed here); the roadmap entries and notes of F-375, F-376, F-377,
   F-378, F-380 (the notes left for F-380 by ADRs 0275, 0276, 0277). ADRs 0271, 0273, 0275, 0277, 0284: their decisions as
   reflected in the code and in their roadmap notes, and the measured patch-test numbers of ADR 0275's addendum (P1 4e-16 to
   1.2e-15, U2 up to 6.4e-12) used for the estimates below; their bodies were not read line by line.
2. Code: `crates/rcfd-fem/src/{loads.rs, recover.rs, material.rs, model.rs, element.rs, assemble.rs, solve.rs, thermal.rs,
   lib.rs}` in full; the public signatures of `builders.rs`, `constraint.rs`, `mesh.rs`, `error.rs`; the tests
   `sb_static.rs` (helpers, check E3), `sb_loads.rs` (check X1).
3. **Not read:** the bodies of the books cited; any finite-element code outside this repository.

Findings that shape the design:

1. **Two existing tests assert the thermal refusal, not one.** (a) `sb_static.rs`, test
   `e1_to_e4_free_node_conflicts_loads_element_errors`, check E3 (edited by the lead on 2026-11-12 after ADR 0276) asserts
   that `solve_static` of a model with `Load::temperature` fails with `UnsupportedLoad { load: "Temperature", .. }`; E4 after
   it in the same function is then not reached. (b) `sb_loads.rs`, test `x1_to_x3_refusals`, check X1 of ADR 0276, asserts
   that `nodal_load_vector` of such a model fails with `UnsupportedLoad { load: "Temperature", package: "ADR 0281" }`; X2 and
   X3 after it are then not reached. Both fail by construction once the thermal load is supported. Existing tests are not
   edited (house rule 2). After this package **every variant of `Load` is supported**, so no unsupported load remains to put
   in their place: the proposal is to delete the two assertion blocks (E3: the lines from the comment `// E3:` to the end of
   its `assert!`; X1: the lines from `// X1:` to the end of its `assert!`), keeping E1, E2, E4 and X2, X3. The proposal is run
   once on a scratch copy (checks X3, X4). `FemError::UnsupportedLoad` stays in `error.rs` (public, for later load kinds)
   but has no producer any more; the private `unsupported` of `loads.rs` and the wildcard arm are removed (they would be
   dead code and an unreachable pattern, which clippy with `-D warnings` refuses).
2. `nodal_load_vector` processes the loads in the model's order, element by element through `add_force`; materials are
   looked up only when a load needs them. `Load::Temperature { nodal, reference }` is validated by `SolidModel::add_load`
   (one finite value per node, finite reference). `Material::expansion()` exists (finite, any sign).
3. `IntegrationPointField::compute` takes the strain `B u` at the points of each element's stiffness rule (from
   `Element::integration_points`, which also gives the shape function values `ip.geometry.n`) and calls
   `Material::stress(eps, &PointContext::default(), &[])`, which returns `D eps`. `element_strain_energy` uses `eps . sigma /
   2`, which is not the elastic energy once a thermal strain is present (the note of ADR 0277).
4. `solve_static_with` computes the reactions `K u - f` with `f` from `nodal_load_vector`: with the thermal load in `f` these
   are the support forces on the body, nothing in `solve.rs` needs to change. Modal analysis (`mass.rs`, `modal.rs`) does not
   read the loads.

## Arithmetic before this registration (disclosed, by hand)

1. **Constitutive law.** Small strain, isotropic: `eps = eps_el + eps_th`, `eps_th = alpha (T - T_ref) (1, 1, 1, 0, 0, 0)` in
   Voigt order with engineering shear (no shear part), `sigma = D eps_el = D (eps - eps_th)`. With `lambda = E nu/((1 +
   nu)(1 - 2 nu))`, `mu = E/(2 (1 + nu))`: `D eps_th = (3 lambda + 2 mu) alpha dT (1, 1, 1, 0, 0, 0)` and `3 lambda + 2 mu =
   E/(1 - 2 nu)` (three times the bulk modulus).
2. **Weak form and the thermal load.** The potential `Pi(u) = 1/2 int (eps - eps_th) . D (eps - eps_th) dV - (work of the
   mechanical loads)`; its stationarity in `u_h = sum N_a u_a` gives `K u = f_mech + f_th` with `f_th = int B^T D eps_th dV`.
   The temperature is interpolated with the element's own functions, `T(x) - T_ref = sum_b N_b (T_b - T_ref)` (the nodal
   differences formed first, so a uniform field gives `dT sum N_b`, `sum N_b = 1` to rounding). **Rule: the element's
   stiffness rule**, the same points at which the recovery evaluates the stress. Then for any temperature field the discrete
   equations are exactly `sum_e sum_q w_q B_q^T sigma_q = f_mech + R` with `sigma_q = D (B_q u - eps_th,q)` the recovered
   stress and R the reactions: the recovered stress is in discrete equilibrium with the loads (check Q1). Exactness of the
   rule: for a uniform dT the integrand is `B` (times a constant): on Tet4 constant, on straight Tet10 degree 1, on a
   trilinear Hex8 `grad N_a det J` polynomial of degree `<= 3` per variable (columns of J bilinear, cofactors degree `<= 2`
   per variable, `dN/dxi` degree `<= 1`): `tet_1`, `tet_4`, `gauss_2` are exact. More useful than exactness: for any rule,
   `f_th = sum_q w_q B_q^T D eps_th` and `K u_lin = sum_q w_q B_q^T D B_q u_lin` coincide when `B_q u_lin = eps_th` at every
   point, so a field whose thermal strain is reproduced pointwise is solved exactly whatever the rule (arithmetic 3, 6). A
   linear temperature on straight Tet10 gives an integrand of degree 2, exact by `tet_4`; on Hex8 a trilinear T against
   `gauss_2` is not exact on a distorted element (degree up to 4 per variable), whereas the mass rule `gauss_3` would be:
   the two rules differ there, which check Q1 (random nodal temperatures) sees.
3. **(c1, c2) Free expansion.** Uniform dT, any `nu`: `u = alpha dT (x - x_A)` has `eps = eps_th` everywhere, `sigma = 0`,
   no traction on any face, and it satisfies any support that holds only rigid motions. Statically determinate support (3-2-1)
   on a box `[0, a] x [0, b] x [0, c]`: corner A = (0, 0, 0) all three components, corner B = (a, 0, 0) the components y and
   z, corner C = (0, b, 0) the component z, all zero: `u(B) = (alpha dT a, 0, 0)` and `u_z(C) = 0` agree. This linear field
   lies in every element space and `B_q u = eps_th` at every point (isoparametric elements reproduce linear fields), so by
   arithmetic 2 the discrete solution is exact on distorted elements too. **Rounding:** ADR 0275's patch test on the same kind
   of distorted box gave interior errors 4e-16 to 1.2e-15 of the field; here the right-hand side is a sum of element vectors
   of size `E alpha dT h^2/(1 - 2 nu)` that cancel at interior nodes, so `~ 1e-14` relative is expected. Stress: `D (B u -
   eps_th)` with `|B u - eps_th| ~ 1e-14 alpha dT L/h`, times `E/(1 - 2 nu) = 2.5 E`: `~ 1e-13 E alpha dT`. Limits 1e-10
   (acceptance): margin `>= 1e2`.
4. **(c3) Box held on all faces.** Rollers on the six faces (the normal component zero on `xmin`, `xmax` for x, `ymin`,
   `ymax` for y, `zmin`, `zmax` for z). `u = 0` gives `eps = 0`, `sigma = -D eps_th = -E alpha dT/(1 - 2 nu) (1, 1, 1, 0, 0,
   0)`, uniform, hence in equilibrium with no body force; the rollers carry the normal traction and there is no tangential
   traction (the stress is isotropic), so `u = 0` is the exact solution; a full clamp would give the same. Discretely, at a
   free unknown (an interior node, or a tangential component on a face) `f_th,a = int grad N_a dV . (D eps_th)`-like sums
   over the patch of the node equal the boundary integral `int N_a sigma_th n dS` over the patch boundary, where `N_a = 0`
   (interior node) or `sigma_th n` is normal to the face (tangential component): zero, with the rule exact (arithmetic 2), so
   `u = 0` to rounding and the stress is `-D eps_th` to `~ 1e-14`. Limit 1e-10 relative to `E alpha dT/(1 - 2 nu)`.
5. **(c4) Bar held at both ends, free sides.** Bar `[0, L] x [-b/2, b/2]^2`, `u_x = 0` on both end faces (rollers), the
   sides free. Uniaxial stress `sigma_xx = s`, others zero: `eps_xx = s/E + alpha dT = 0` (u_x = 0 everywhere) gives `s = -E
   alpha dT` for any nu; `eps_yy = eps_zz = -nu s/E + alpha dT = (1 + nu) alpha dT`; `u = (0, (1 + nu) alpha dT y, (1 + nu)
   alpha dT z)`, linear, with no shear. The lateral rigid motions are held without contradicting it: node A = (0, 0, 0) the
   components y and z, node C = (0, b/2, 0) the component z (C has z = 0, so u_z(C) = 0). The rollers on both ends hold the
   x translation and the rotations about y and z; A holds the y and z translations, C the rotation about x. Linear field:
   exact by arithmetic 2, rounding as arithmetic 3. Reactions: the traction on the end x = L is `sigma n = -E alpha dT e_x`,
   so the reactions on the nodes of `xmax` sum to `-E alpha dT A e_x` (extension E2). Elastic energy `s^2 V/(2 E) = E (alpha
   dT)^2 V/2` (extension S1).
6. **(c5, c6) Free beam, linear temperature through the thickness.** Beam `[0, L] x [-b/2, b/2] x [-h/2, h/2]`, `T - T_ref =
   T0 + dT z/h` (so `dT` is the difference between the faces z = h/2 and z = -h/2). The thermal strain `alpha (T0 + dT z/h)
   I` is linear in x, hence compatible (all second derivatives vanish), so a free body has `sigma = 0` and `eps = eps_th`:
   `u*_x = alpha (T0 x + dT x z/h)`, `u*_y = alpha (T0 + dT z/h) y`, `u*_z = alpha (T0 z + dT z^2/(2 h)) - alpha dT (x^2 +
   y^2)/(2 h)`. Check: `eps_xx = eps_yy = alpha (T0 + dT z/h)`, `eps_zz = alpha (T0 + dT z/h)`; `gamma_xz = alpha dT x/h -
   alpha dT x/h = 0`, `gamma_yz = alpha dT y/h - alpha dT y/h = 0`, `gamma_xy = 0`. The axis bends to `u_z = -alpha dT
   x^2/(2 h)`: curvature `kappa = alpha dT/h` (hot face convex). **Support (3-2-1):** A = (0, 0, 0) all three, B = (L, 0, 0)
   y and z, C = (0, b/2, 0) z, all zero. The solution is `U = u* + w x x` with the small rotation `w = (alpha dT b/(4 h),
   -alpha dT L/(2 h), 0)`: `u*(A) = 0`; at B `U_y = w_z L = 0`, `U_z = -alpha dT L^2/(2 h) - w_y L = 0`; at C `U_z = -alpha dT
   b^2/(8 h) + w_x b/2 = 0`. The rotation is linear in x, so the second difference along the axis is unchanged: `kappa_h =
   -(U_z(0) - 2 U_z(L/2) + U_z(L))/(L/2)^2` at the axis nodes (y = z = 0) is `alpha dT/h` for the exact field. **Tet10 on
   straight-sided elements:** `U` is quadratic, so it lies in the space; the linear T is interpolated exactly by the quadratic
   functions, so `B_q U = eps_th,q` at every point and by arithmetic 2 the discrete solution is `U` to rounding: curvature and
   stress errors `~ 1e-12` (displacements `~ alpha dT L^2/h ~ 6e-3 m`, a second difference of numbers of `~ 1.5e-3` with
   absolute errors `~ 1e-17`), limits 1e-2: margin `~ 1e9`. Tet4 and Hex8 cannot represent `x^2` and are not checked here.
7. **Strain energy with a thermal strain.** The elastic strain energy `U_el = 1/2 int (eps - eps_th) . sigma dV`, at the
   stiffness rule's points `sum_q w_q (eps_q - eps_th,q) . sigma_q / 2`. Box held on all faces: `eps - eps_th = -alpha dT (1,
   1, 1, 0, 0, 0)`, `sigma = -s_h (1, 1, 1, 0, 0, 0)`, `s_h = E alpha dT/(1 - 2 nu)`: `U_el = 3 E (alpha dT)^2 V/(2 (1 - 2
   nu))`. Bar: `E (alpha dT)^2 V/2`. Free expansion: `0` (to the square of the stress rounding, `~ 1e-26` of `E (alpha dT)^2
   V`). The old formula `eps . sigma/2` would give `0` for the box on all faces (`eps = 0`) and `0` for the bar (`eps` has no
   xx part, `sigma` only an xx part): the check separates them.
8. **Discrete equilibrium (Q1).** With the thermal load and the recovery on the same points (arithmetic 2), `r_int = sum_e
   sum_q w_q B_q^T sigma_q` assembled from the recovered stresses equals `K u - f_th` exactly in exact arithmetic, and the
   reactions are `R = K u - f` with `f = f_th` (no mechanical load): `r_int - R` is rounding of two orderings of the same sums
   (constrained unknowns) and the residual of the direct solve (free unknowns; ADR 0276 measured relative residuals 3.6e-14
   to 1.8e-11 on slender bars, `~ 1e-14` on compact boxes). Estimate `<~ 1e-12` of `max |f_th|`, limit 1e-9. With the load
   on the mass rule and the stress on the stiffness rule (mutation M6) the difference is the quadrature error of a trilinear
   T on a distorted Hex8 or a quadratic T on Tet10, `~ 1e-3` relative for random nodal temperatures: caught.
9. **Signed zeros (extension B2).** A model with `alpha = 0`, or with `T_b = T_ref` at every node, adds element thermal loads
   whose entries are `+-0.0`; `x + (+-0.0) = x` for `x != 0` and `0.0 + (-0.0) = 0.0`, so the load vector, the displacement and
   the reactions keep their bits. In the recovery the diagonal of `eps_th` may be `-0.0` (a negative `N_b` times `0.0`), and
   `(-0.0) - (-0.0) = +0.0` flips the sign of a zero strain component before `D` is applied; so the stress is registered as
   equal by value (`==`, which takes `-0.0 == 0.0`), not by bits.
10. **Values.** `E = 2e11 Pa`, `nu = 0.3`, `rho = 7850 kg/m^3`, `alpha = 1.2e-5 1/K`, `T_ref = 293.15 K`, `T = 343.15 K` (dT =
    50 K): `alpha dT = 6e-4`, `E alpha dT = 1.2e8 Pa`, `E alpha dT/(1 - 2 nu) = 3e8 Pa`. Beam `L = 1 m`, `b = h = 0.1 m`,
    `T0 = 20 K`, `dT = 50 K`: `kappa = 6e-3 1/m`. Sizes: boxes `[3, 3, 2]` (Tet10 245 nodes), bar and beam `[8, 2, 2]` and
    `[10, 2, 2]` (Tet10 `~ 700` nodes): all solves well below a second in release.

## Design

### D1. `thermal.rs` (public)

* `temperature_field(model) -> Result<Option<(&[f64], f64)>>`: the nodal temperatures and the reference of the model's
  temperature load, `None` without one; **a second `Load::Temperature` is refused** (`InvalidValue { key: "temperature" }`,
  "a model has one temperature field"): a temperature is a state, not an additive load, and two would be a user error (two
  temperature sets of a deck); nothing is summed silently.
* `thermal_strain(material, delta_t) -> Result<[f64; 6]>`: `alpha dT (1, 1, 1, 0, 0, 0)` with `alpha = material.expansion()`.
* `point_temperature_change(n, delta_t_nodal) -> f64`: `sum_b N_b dT_b` in node order.
* `element_thermal_load(kind, coords, material, delta_t_nodal) -> Result<Vec<f64>>`: `f_e = sum_q w_q B_q^T D eps_th,q`
  over the points of the kind's stiffness rule, `3 n` entries node-major; `D` from `material.elasticity_matrix()`.
* `add_thermal_load(f, dofs, model, nodal, reference)` (crate-private): `dT_b = T_b - T_ref` per node, then element by
  element in element order, the element vector added node by node through the `add_force` of `loads.rs` (made
  `pub(crate)`); materials through `SolidModel::material_indices` (refuses an element without material); an element error
  is `FemError::Element { element, source }`.

### D2. `loads.rs`

The `Load::Temperature { nodal, reference }` arm calls `thermal::temperature_field(model)?` (the refusal of a second field)
and then `thermal::add_thermal_load`. The wildcard arm and the private `unsupported` are removed (finding 1); `add_force`
becomes `pub(crate)` with its body unchanged. No other arm changes; the module documentation is updated.

### D3. `recover.rs`

`IntegrationPointField::compute` calls `thermal::temperature_field(model)?` once. Without a temperature field the loop is
the old one, line for line (`sigma = Material::stress(eps)`), and the new private field `thermal` is `None`. With one, at
each point `dT_q = point_temperature_change(ip.geometry.n, dT_e)`, `eps_th,q = thermal_strain(material, dT_q)`, and
`sigma_q = Material::stress(eps_q - eps_th,q)`; the stored strain stays the total strain `B u` and the thermal strain is
kept per point (`IntegrationPointField::thermal_strain() -> Option<&[[f64; 6]]>`). `element_strain_energy` (and so
`strain_energy`) is the elastic energy `sum w (eps - eps_th) . sigma / 2` when `thermal` is `Some`, the old expression when
it is `None`. `attach`, the means, the extrapolation and the invariants are unchanged (they act on whatever stress the
field holds).

### D4. Tests

* `tests/sb_thermal.rs`: checks T1 to T4, Q1, S1, E1, E2, B2, X1, X2 and the negative controls. Generator SplitMix64 as in
  ADR 0275 D8, **seed `0xF380_0281`**, stream `SEED + 10 + k` for the perturbed box of kind k (Tet4 0, Hex8 1, Tet10 2),
  `SEED + 20 + k` for the bar, `SEED + 30 + k` for the random temperatures of Q1. Perturbation: interior grid nodes moved by
  `0.1 h_k (2U - 1)` per coordinate before the Tet10 upgrade (ADR 0275 arithmetic 6). Box `[0, 1.2] x [0, 1.0] x [0, 0.8]`,
  divisions `[3, 3, 2]`, perturbed. Bar `[0, 1] x [-0.05, 0.05]^2`, `[8, 2, 2]`, perturbed. Beam `[0, 1] x [-0.05, 0.05]^2`,
  `[10, 2, 2]`, Tet10 from the unperturbed Tet4 box. Supports as in arithmetic 3, 5, 6 (rollers by `Constraint::displacement`
  on the face node sets, single components by `Prescribed`). Q1: the T1 box and support, `T_b = T_ref + 50 + 20 (2U - 1)` K
  per node.
* `tests/sb_thermal_bits.rs`: check B1, using only API that existed at d4bde1e0. **Comparison defined:** for each model of
  the list below and each kind (Tet4, Hex8, Tet10), an FNV-1a 64-bit digest of the bits (`f64::to_bits`, little-endian
  bytes) of, in this order: the load vector of `nodal_load_vector(model, extra)`; the displacement, the reactions and the
  relative residual of `solve_static_with`; the positions, weights, strains and stresses of
  `IntegrationPointField::from_result`; its element means of the stress and the strain; `strain_energy()`; the nodal
  averages of the stress over all elements (`None` as the bits of NaN). Models (no temperature load, `alpha` = 1.2e-5 so
  that an expansion coefficient alone changes nothing): (a) uniaxial: the perturbed box, `xmin` clamped, a nodal force on
  `xmax`; (b) all mechanical loads at once on the perturbed box: traction on `ymax`, pressure on `xmax`, gravity,
  centrifugal, a nodal body force field, and the per-node forces of `body_force_loads` of a linear field as `extra`, held by
  the clamp of `xmin`; (c) the bar of the thermal tests with a pressure on `xmax` and its supports. **Procedure:** the test
  file is written before any library code and run once on a `git archive` copy of d4bde1e0 (scratch, own target directory);
  the digests it prints there are written into the file as its expected constants (a commit of the test file alone, still
  before any library code); after the library code the test asserts equality. B1 passes if every digest is equal.

## Registered checks

All in `crates/rcfd-fem/tests/sb_thermal.rs` and `sb_thermal_bits.rs`, release profile, one run each; every number printed.
"Three kinds" means Tet4, Hex8 and Tet10, each checked against the limit. `s_u = E alpha dT`, `s_h = E alpha dT/(1 - 2 nu)`.

| Id | Clause | Check | Limit | Estimate behind it |
| --- | --- | --- | --- | --- |
| T1a | c1 | free expansion, box, three kinds: `max_nodes max_k |u_k - alpha dT x_k| / (alpha dT max_nodes max_k |x_k|)` | `<= 1e-10` | arithmetic 3: `~ 1e-14` |
| T1b | c2 | the same: `max_points max_k |sigma_k| / s_u` | `<= 1e-10` | arithmetic 3: `~ 1e-13` |
| T2 | c3 | box with rollers on all faces, three kinds: `max_points (max_{k<3} |sigma_k + s_h|, max_{k>=3} |sigma_k|) / s_h` | `<= 1e-10` | arithmetic 4: `~ 1e-14` |
| T3 | c4 | bar, three kinds, nu = 0.3: `max_points |sigma_xx + s_u| / s_u` and `max_points max_{k != xx} |sigma_k| / s_u` | `<= 1e-10` each | arithmetic 5: `~ 1e-13` |
| T4a | c5 | beam Tet10: `|kappa_h - alpha dT/h| / (alpha dT/h)` | `<= 1e-2` | arithmetic 6: `~ 1e-12` |
| T4b | c6 | beam Tet10: `max_points max_k |sigma_k| / s_u` | `<= 1e-2` | arithmetic 6: `~ 1e-12` |
| B1 | c7 | digests of sb_thermal_bits (D4) on the final tree against those of d4bde1e0, 3 models x 3 kinds | all equal | D2, D3: the paths without a temperature load are unchanged |
| Q1 | arithmetic 2, 8 | box, random nodal temperatures, three kinds: `max |r_int - R| / max |f_th|` | `<= 1e-9` | arithmetic 8: `<~ 1e-12` |
| S1 | D3 | elastic strain energy: box on all faces against `3 E (alpha dT)^2 V/(2 (1 - 2 nu))`, bar against `E (alpha dT)^2 V/2` (relative), free box `U_el / (E (alpha dT)^2 V)`, three kinds | `<= 1e-10`, `<= 1e-10`, `<= 1e-18` | arithmetic 7 |
| E1 | extension | beam Tet10: `max_nodes |u_h - U| / max_nodes |U|` (U of arithmetic 6) | `<= 1e-9` | arithmetic 6 |
| E2 | extension | bar, three kinds: `max_nodes |u_h - (0, (1 + nu) alpha dT y, (1 + nu) alpha dT z)| / ((1 + nu) alpha dT b/2)`, and `|sum_{xmax} R_x + s_u A| / (s_u A)` | `<= 1e-10` each | arithmetic 5 |
| B2 | extension | the T1 box with (a) `alpha = 0` and (b) `T_b = T_ref` at every node, against the same model without a temperature load and with a nodal force on `xmax`: load vector, displacement and reactions bit-equal; stresses equal by value | as stated | arithmetic 9 |
| X1 | D1 | two temperature loads: `nodal_load_vector` and `IntegrationPointField::compute` refuse with `InvalidValue { key: "temperature" }` | as stated | D1 |
| X2 | D1 | a temperature load on a model without material: `nodal_load_vector` refuses with `MaterialUnassigned` | as stated | D1 |
| X3 | finding 1 | the proposed removal of ADR 0275's E3 block from `sb_static.rs`, run once on a scratch copy with the rest unchanged: `sb_static` passes | passes | finding 1 |
| X4 | finding 1 | the proposed removal of ADR 0276's X1 block from `sb_loads.rs`, run once on the same scratch copy: `sb_loads` passes | passes | finding 1 |

**Negative controls** (each must hold, else the check is not sensitive):

| Id | Control | Expected |
| --- | --- | --- |
| N1 | T1b's measure with `D eps` (total strain, the thermal strain not subtracted) from the field's strains | `> 1e-1` (it is `s_h/s_u = 2.5`) |
| N2 | T2's measure against the uniaxial value `-s_u` instead of `-s_h` | `> 1e-1` (it is `1.5`) |
| N3 | T4a's measure for the beam with the uniform temperature `T_ref + T0` (no gradient) | `> 0.5` (curvature 0, measure 1) |

**Mutations** (on a `git archive` copy of the final tree in `w49i-work/f380-scratch`, own target directory; each applied
alone by exact text replacement, `sb_thermal` run once, the file restored and compared by SHA-256):

| Id | Mutation | Must fail |
| --- | --- | --- |
| M1 | thermal load with the sign reversed (`-` before the element vector) | T1a |
| M2 | recovery without the thermal strain (`sigma = D eps`) | T1b, T2, T3, Q1 |
| M3 | thermal strain with shear parts (`alpha dT` in all six entries) | T1a |
| M4 | temperature difference without the reference (`dT_b = T_b`) | T1a |
| M5 | point temperature as the mean of the element's nodal values instead of the interpolation | T4b |
| M6 | thermal load on the mass rule instead of the stiffness rule | Q1 |
| M7 | thermal load with `E eps_th` instead of `D eps_th` (Poisson coupling lost) | T1a |
| M8 | a second temperature load accepted (the refusal skipped) | X1 |
| M9 | elastic energy with the total strain (`eps . sigma / 2`) | S1 |

**Decision rule.** F-380 is proposed **done** (pending the maintainer's acceptance) if T1a to T4b, B1, Q1 and S1 are met for
every kind they name (every acceptance clause, c7 by B1), N1 to N3 hold and every mutation fails a test. E1, E2, B2 and X1 to
X4 are registered extensions: a miss among them is a `*_known_miss` record and a follow-up and does not alone block done. A
miss or an unrun check among T1a to T4b, B1, Q1, S1, a failed negative control or an uncaught mutation (after checking that a
test covers it): **partial**, with the miss named. No crate that builds: **todo**. The failure of ADR 0275's E3 and ADR
0276's X1 (finding 1) is by construction and is reported to the lead with the proposed removals (X3, X4); it does not change
the decision, but the gate fails on them until the lead applies or rejects the proposal.

## Limits (what is not covered)

- Isotropic thermal strain only; `E`, `nu` and `alpha` do not depend on temperature (intermediate tier); one temperature field
  per model, given at the nodes; no temperature in `PointContext` yet (ADR 0311's).
- The thermal load is integrated with the stiffness rule: exact for a temperature linear in the reference coordinates on Tet4
  and straight Tet10 and for a uniform one on any element; otherwise it carries the quadrature error of that rule, consistent
  with the recovered stress (Q1) but not exact against the continuous integral.
- No check of Tet4 or Hex8 in thermal bending (they cannot represent the quadratic field; their curvature is not registered).
- `SolidResult` stores the total strain and the stress; the thermal strain per point is available from the
  `IntegrationPointField` only.
- Modal analysis ignores the temperature (no thermal pre-stress, intermediate tier).

## Open points taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. The thermal load uses the stiffness rule (arithmetic 2: discrete equilibrium of the recovered stress), not the mass rule
   of the body loads.
2. A second temperature load is refused rather than summed (D1).
3. The strain energy of a model with a temperature field is the elastic energy `1/2 int (eps - eps_th) . sigma` (D3).
4. ADR 0275's E3 and ADR 0276's X1 are not edited; their removal is proposed (finding 1). It is a test edit, not a default,
   golden or hash table, but the house rules forbid editing an existing test, so it is the lead's step.

## The planned stub, as written

Status: **PLANNED 2026-11-12** (layout by the basic-tier plan, [ADR 0270](0270-structural-solver-basic-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are
the acceptance draft that the registration turns into checks.

Work package SB-11 of the structural solver, basic tier. Size S.

### Goal

The load export of F-237 carries nodal temperatures (`TEMP`). Add the isotropic thermal strain `eps_th = alpha (T - T_ref) I`:
the equivalent nodal load `int B^T D eps_th dV` and the stress `sigma = D (eps - eps_th)` in the recovery. Opt-in: a model
without a temperature field gives the bits it gave before.

### Dependencies

ADR 0276, ADR 0277; existing roadmap items: none.

### Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. With a nodal temperature field, an expansion coefficient
and a reference temperature: a uniformly heated, statically determinately held box expands by alpha dT x at every node within
1e-10 with stresses below 1e-10 E alpha dT; a box held on all faces has the hydrostatic stress -E alpha dT/(1 - 2 nu) within
1e-10; a bar held at both ends with free sides has the axial stress -E alpha dT within 1e-10 (nu = 0.3); a linear temperature
through the thickness of a free beam gives the curvature alpha dT/h within 1 percent and stresses below 1 percent of E alpha dT
(Tet10); a model without temperature is bit-identical to the same model before this change.

### Reference to check against

Closed forms from Hooke's law with thermal strain (Timoshenko and Goodier, Theory of Elasticity, the chapter on thermal stress;
each derivable in a few lines).

### Files

`crates/rcfd-fem/src/thermal.rs`, small hooks in `loads.rs` and `recover.rs`, `crates/rcfd-fem/tests/sb_thermal.rs`.

### What could make it miss

It edits `loads.rs` and `recover.rs`, so it cannot run beside ADR 0276 or 0277. Temperature-dependent `E` and `alpha` are not
included (intermediate).
