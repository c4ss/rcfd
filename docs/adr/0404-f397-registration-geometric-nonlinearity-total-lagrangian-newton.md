# ADR 0404: F-397 registration: geometric nonlinearity, total Lagrangian solid elements, Newton iteration with load steps

Status: **REGISTERED 2026-11-13, written before any code of F-397 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-397 was written, compiled or run: the files listed under "What was read" were read and
the arithmetic below was done by hand and with two short Python scripts of plain arithmetic (an ODE integration of the elastica
and the scalar Newton iteration of a uniform bar; disclosed in "Arithmetic"). Results go in the addendum at the end; nothing
above the addendum is changed after a run.

Parent: [ADR 0300](0300-geometric-nonlinearity-total-lagrangian-newton.md) (the stub of SI-10), plan [ADR
0290](0290-structural-solver-intermediate-tier-plan.md) (sections 2.3, 2.5 item 1, the SI-10 entry). Building blocks: the
solid elements of ADR 0273 (Tet4, Hex8, Tet10) and ADR 0292 (Hex20), the static solve of ADR 0275 (elimination of prescribed
unknowns, `constraint::reduce`, the pivot test `solve::PIVOT_TOLERANCE`), the geometric stiffness of ADR 0390 D1
(`geometric::element_geometric_stiffness`), the constitutive call `Material::stress(strain, point, state)` of ADR 0273 D5.
Roadmap item F-397 (deps F-395, F-374, F-375). Size M. Number 0404 given by the lead. Branch `f397-geometric-nonlinearity`
from main bf03ed6d, worktree `w49i-work/f397`, scratch `w49i-work/f397-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs and not changed after a run (a miss
becomes an ignored `*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the hash tables or of a golden file; no new dependency, no unsafe code; clean-room.

## What F-397 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. The tangent agrees with central differences of the
> internal force within 1e-6 relative on distorted elements at displacements of 20 percent of the element size (registered
> seed). A rigid rotation by 90 degrees with a translation gives an internal force below 1e-10 E L^2. Uniform stretch lambda =
> 1.2 of a bar with nu = 0: nominal stress E lambda (lambda^2 - 1)/2 within 1e-9; under load control the last three residuals of
> each step show an order >= 1.8. A load factor of 1e-6 reproduces the linear solution within 1e-5 relative. Elastica: a
> cantilever of slenderness 50, nu = 0, with tip force P L^2/(E I) = 1 and 2 has the tip deflection and shortening within 2
> percent of the elliptic-integral solution (recomputed in the registration by integrating the elastica equation; recalled
> 0.302 L, 0.056 L and 0.494 L, 0.161 L). An element inverted during the iteration and a step that does not converge after the
> registered halvings are errors with the element or the step.

Clauses: (a1) tangent against central differences; (a2) rigid rotation with translation; (a3) the nominal stress of the uniform
stretch; (a4) the order of the last three residuals under load control; (a5) the linear limit; (a6) the elastica, two loads, two
quantities; (a7) inversion and non-convergence are located errors. ADR 0300 adds: St. Venant-Kirchhoff with the linear material,
Tet4, Tet10, Hex8 (full integration) and Hex20, the tangent as material part plus the geometric part of ADR 0298 (= 0390), a
residual and an energy criterion, step halving, the material called through `stress(strain, state)` with commit and roll-back of
the state per step, dead loads only.

Readings registered (each taken on the maintainer's standing instruction of 2026-11-11, take the recommendation):

- **(a1)** "relative": `max_ij |K_fd - K_T|_ij / max_ij |K_T|_ij`, element by element. "Distorted elements": one element of each
  kind, its reference nodes moved by the seed (D8). "Displacements of 20 percent of the element size": the field of D8, whose
  homogeneous part has gradient entries up to 0.2 and a translation up to `0.2 l`, plus a random nodal part `0.02 l`, once
  without and once composed with a rotation by 90 degrees. `l` is the element size of D8.
- **(a2)** "below 1e-10 E L^2": `max_i |f_int,i| <= 1e-10 E l^2`, `l` the element size, for one distorted element of each kind
  and for an assembled box; rotation by 90 degrees about a skew axis, translation of the order of `l`.
- **(a3)** "within 1e-9": `|P_measured / P_exact - 1| <= 1e-9`, `P_measured` the sum of the reactions in `x` on the end face
  divided by the reference area, under displacement control, for each of the four kinds. Under load control (the traction
  `P_exact` on the end face), `|lambda_measured / 1.2 - 1| <= 1e-9`.
- **(a4)** "the last three residuals of each step show an order >= 1.8": for each load step of the load-controlled stretch, the
  relative residuals `r_k` (D5) of that step; the residuals below the rounding floor `r <= 1e-12` are left out (a residual at
  rounding carries no order); of the rest, the last three `r_a, r_b, r_c` give `p = ln(r_c / r_b) / ln(r_b / r_a)`, and `p >= 1.8`
  for every step; a step with fewer than three residuals above the floor is a miss of the check.
- **(a5)** "within 1e-5 relative": `max_i |u_nl,i / s - u_lin,i| / max_i |u_lin,i| <= 1e-5`, `s = 1e-6` the load factor (the
  model's loads times `s`, one load step), `u_lin` the linear static solve of the full load.
- **(a6)** "slenderness 50": length over depth, `L / h = 50` (ADR 0300 states 1 percent strain "at this slenderness": the peak
  bending strain is `(P L^2 / E I) h / (2 L) = 1 / 100` at load 1 for `L / h = 50`; with `L / r = 50`, `r` the radius of
  gyration, it would be 3.5 percent). "Within 2 percent": `|v / v_ref - 1| <= 0.02` and `|d / d_ref - 1| <= 0.02`, `v` the tip
  deflection and `d` the tip shortening of the neutral axis at the tip (D8).
- **(a7)** "an error with the element": `Err(FemError::Element { element, source })` with `element` the inverted element and
  `source` an `InvalidValue` with key `"deformation gradient"`; "with the step": `Err(FemError::Solver { solver: "newton", reason
  })` whose reason names the load step (`"load step s of n"`) and the halvings. The inversion error also names the step in its
  reason. A failed attempt (inversion, non-convergence, a singular tangent, a non-finite residual) halves the increment (D6), so
  both are errors only after the registered halvings.

## What was read (no run)

- `AGENT_RULES.md`; the brief; `docs/roadmap.json` entry F-397; ADR 0300 (whole); ADR 0290 (sections 2.3 to 2.5, the SI-10
  entry); ADR 0401 (for the form); ADR 0390 D1 (the geometric stiffness as the second variation of the Green-Lagrange strain
  energy).
- Code: `crates/rcfd-fem/src/element.rs` (whole: the trait, `ElementState`, the refusing defaults of `internal_force` and
  `tangent`, `b_matrix`, `stiffness_with_rule`), `geometric.rs` (whole), `material.rs` (whole), `solve.rs` (whole), `assemble.rs`
  (whole), `constraint.rs` (`Prescribed`, `prescribed_equations`, `check_rigid_body_motions`, `reduce`, `ReducedSystem`,
  `reactions`), `loads.rs` (`nodal_load_vector`, `NodalForceAt`), `model.rs` (signatures, `check_elements_supported`,
  `check_supported`), `mesh.rs` (`SolidFormulation`), `hex20.rs` (the element, `upgrade_to_hex20`), `builders.rs`, `error.rs`
  (variants), `mpc.rs` (`uses_transformation`), `lib.rs`, the stub `nonlinear.rs`.
- Tests: `si_scaffold.rs` (X2 and its `_after_geometric_landed` copy, which assert that `internal_force` and `tangent` refuse
  naming ADR 0300), `si_hex20.rs` (the SplitMix64 source and the `Checks` helper, copied).
- Searched: no crate calls `Element::internal_force` or `Element::tangent` except `si_scaffold.rs`.
- **Not read:** any finite-element code or manual outside this repository. The methods are cited from memory and derived here.

## Design

### D1. Kinematics and stress (total Lagrangian; St. Venant-Kirchhoff)

Sources, from memory: Bathe, *Finite Element Procedures* (1996), section 6.2 (total Lagrangian formulation); Bonet and Wood,
*Nonlinear Continuum Mechanics for Finite Element Analysis* (2nd ed., 2008), chapters 3, 4 and 8; Belytschko, Liu and Moran,
*Nonlinear Finite Elements for Continua and Structures* (2000), chapter 4. Derived here from first principles; nothing is taken
as a number from them.

Reference coordinates `X` (the mesh), displacement `u = sum_a N_a(xi) u_a`, reference gradients `g_a = dN_a/dX` (the
`grad` of `Element::point` evaluated on the reference coordinates). Deformation gradient `F_iJ = delta_iJ + sum_a u_ai g_aJ`.
Green-Lagrange strain `E = (F^T F - I) / 2`, in Voigt with engineering shear `[E11, E22, E33, 2 E12, 2 E23, 2 E31]`. Second
Piola-Kirchhoff stress and material tangent from the constitutive call: `(S, D) = material.stress(E, point, state)`; for the
linear elastic material `S = D E` (St. Venant-Kirchhoff). Rotation invariance: `F = R (I + H)` gives `F^T F = (I + H)^T (I +
H)`, so a rigid motion (`H = 0`) gives `E = 0` and `S = 0` exactly in exact arithmetic.

Variation: `delta E_IJ = (F_kI delta F_kJ + F_kJ delta F_kI) / 2`, `delta F_kJ = sum_b delta u_bk g_bJ`, so `delta E = B_NL
delta u` with the 6 x 3n matrix, column `(b, k)`:

- normal row `II`: `F_kI g_bI`;
- shear row `IJ` (engineering, `I != J`): `F_kI g_bJ + F_kJ g_bI`.

At `F = I` this is the linear `B` of `element::b_matrix` entry by entry.

### D2. Internal force and tangent of an element

Virtual work in the reference configuration: `delta W_int = int_V0 S : delta E dV0`, so `f_int = sum_q w_q det J_q B_NL,q^T S_q`
at the points of the element's stiffness rule (`Element::stiffness_rule`, the same rule as the linear stiffness: Tet4 1 point,
Hex8 2 x 2 x 2, Tet10 4 points, Hex20 3 x 3 x 3; ADR 0300 "Hex8 full integration").

Linearisation: `d f_int = sum_q w_q det J_q (B_NL^T D B_NL du + d(B_NL)^T S)`. The second term is `sum_q w_q det J_q (g_a . S g_b)
delta_ij` (from `d(delta E_IJ) = (delta F_kI dF_kJ + dF_kI delta F_kJ) / 2`, contracted with the symmetric `S`), which is ADR 0390
D1's `K_sigma` evaluated with `S` at the points of the stiffness rule on the reference coordinates. So `K_T = K_mat(F) + K_sigma(S)`,
`K_mat = sum_q w_q det J_q B_NL^T D B_NL`, and `K_sigma` is computed by the existing `geometric::element_geometric_stiffness(element,
coords, S)` (not edited). `K_T` is symmetric because `D` is (hyperelastic). At `u = 0`: `S = 0`, `K_T = K` (the linear stiffness).

Inversion: at each point, `det F <= 0` or not finite is refused, `FemError::InvalidValue { key: "deformation gradient", reason:
"det F = ... at point q of a <kind> element" }`; the assembly locates it, `FemError::Element { element, source }`.

State: an `ElementState` holds `n_points x material.state_size()` values, point-major; each point's slice is passed to
`material.stress`. A state of the wrong length is refused (`FieldLength`, key `"state"`). For the linear elastic material the size
is 0.

### D3. Element entry points (shared file `element.rs`, two default bodies)

The trait defaults of `Element::internal_force` and `Element::tangent`, which refuse today naming ADR 0300, call D2 instead
(`crate::nonlinear::element_internal_force(self, ...)`, `element_tangent(self, ...)`). Every element with shape functions (Tet4,
Hex8, Tet10, Hex20) then answers; the placeholders (Beam2, Shell4) still refuse, through `Pending::point`, naming their package.
Nothing else in `element.rs` changes. No existing path calls these two methods, so no default result changes. The only existing
test that asserts the old refusal, `si_scaffold::x2_entry_points_refuse_by_default_after_geometric_landed`, has both of its rows
retired: by the maintainer's standing decision of 2026-11-13 it is marked `#[ignore]` with a reason naming ADR 0404 (its body
untouched); no row stays valid, so no `_after_landed` copy is added.

### D4. Assembly

`nonlinear::internal_force(model, u, states)`: the element vectors in element order, scattered sequentially (independent of
threads). `nonlinear::tangent(model, u, states, threads)`: `assemble::assemble_with` with D2's tangent (bit-identical for any
thread count, ADR 0275 D1). The driver computes both in one pass per iteration (`element_force_and_tangent`), scattered in element
order.

### D5. Newton driver (`nonlinear::solve`)

Load control with prescribed displacements scaled by the same factor (dead loads only):

1. Refusals before any work: `model.validate()`, `model.check_supported()`; a block formulation other than `Full`; a model on the
   transformation path (`mpc::uses_transformation`: linear constraints, rigid sets, cyclic constraints, elastic supports) and a
   temperature load, `FemError::InvalidValue { key: "nonlinear", reason }` naming what is refused; `steps == 0`, non-positive or
   non-finite tolerances (`InvalidValue`).
2. `f = loads::nodal_load_vector(model, forces)` (every dead load of the model, assembled once in the reference configuration),
   `g = constraint::prescribed_equations(model, prescribed)`; `check_rigid_body_motions(model, g)`.
3. User steps `n = 1..N`, targets `lambda_n = n / N`. Within a step the increment `dl` starts at `1 / N`; an attempt goes from the
   committed state `(u_c, lambda_c, states_c)` to `lambda_t = min(lambda_c + dl, lambda_n)` (the last attempt of a step lands on
   `lambda_n` exactly).
4. An attempt: iteration `k = 0, 1, ...`: assemble `f_int(u)` and `K_T(u)`; residual on the free equations `R_F = lambda_t f_F -
   f_int,F`; relative residual `r_k = |R_F| / max(|lambda_t f_F|, |f_int|)` (the second norm over every equation, so that the
   reactions count; `r_k = |R_F|` if both are zero). Convergence at `k >= 1`: `r_k <= residual_tolerance` **and** `e_k <=
   energy_tolerance e_1`, `e_k = |du_k . b_{k-1}|` the energy of the correction `du_k` against the right-hand side `b_{k-1}` it
   was solved from (Bathe section 8.4.3, from memory). Otherwise, if `k = max_iterations`, the attempt fails. Otherwise solve
   `K_T,FF du = b_k`, `b_k = R_F - K_T,FP dg` with `dg = (lambda_t - lambda_c) g` at `k = 0` and `dg = 0` after (the symmetric
   elimination `constraint::reduce` with the prescribed increment), by the envelope `L D L^T` of ADR 0274 with the pivot test of
   ADR 0275 (`PIVOT_TOLERANCE`); `u_F += du`, `u_P = lambda_t g` after `k = 0`.
5. A failed attempt (not converged in `max_iterations`, an inverted element, a refused pivot, a non-finite residual or
   correction) rolls back to the committed state and halves `dl`; the halvings of a user step are counted, and when the count
   would exceed `max_halvings` the solve returns the error of (a7): the inversion error of the element (`FemError::Element`) if the
   last attempt failed by inversion, otherwise `FemError::Solver { solver: "newton", reason }` naming the step, its load factors,
   the halvings and the cause of the last failure. A converged attempt commits `(u, lambda_t, states)` and records a `LoadStep`
   (the user step, the load factor, the residuals `r_0..r_k`, the energies, the halvings so far). `dl` is not increased again
   within the step.
6. The result: `SolidResult` with the displacement, the reactions `f_int - f` at the prescribed equations (at `lambda = 1`) and
   `SolverStats { solver: "newton-tl", unknowns, iterations (sum), relative_residual (last) }`, and the list of `LoadStep`.

Defaults (`NewtonOptions::new(steps)`): `max_iterations = 20`, `residual_tolerance = 1e-10`, `energy_tolerance = 1e-10`,
`max_halvings = 5`, `threads = None`. The energy tolerance is relative to the first correction of the attempt; with quadratic
convergence `e_k / e_1 ~ (r_{k-1} / r_0)^2`, so `1e-10` asks `r_{k-1} <= ~1e-5 r_0` and is met together with the residual test in
the cases below (arithmetic 2), without adding an iteration.

State with commit and roll-back: the driver keeps the committed states of every element; every iteration evaluates from the
committed states (as a return mapping from the start of the step does); a converged attempt commits the trial states; a failed
one discards them. `Material::stress` returns no updated state today (the linear material has none), so the trial state equals the
committed one; a material with history adds its update without changing the driver.

### D6. API (all in `crates/rcfd-fem/src/nonlinear.rs`)

`element_second_piola_kirchhoff`, `element_internal_force`, `element_tangent`, `element_force_and_tangent` (generic over
`E: Element + ?Sized`); `initial_states(model)`; `internal_force`, `tangent`; `NewtonOptions`, `LoadStep`, `NonlinearResult`,
`solve(model, prescribed, forces, options)`. `lib.rs` is not edited (the module is declared).

### D7. Files

`crates/rcfd-fem/src/nonlinear.rs` (the stub, filled), `crates/rcfd-fem/src/element.rs` (D3: the two default bodies),
`crates/rcfd-fem/tests/si_scaffold.rs` (D3: one `#[ignore]` attribute), new `crates/rcfd-fem/tests/si_nonlinear.rs`. Not edited:
`geometric.rs`, `solve.rs`, `buckling.rs`, `modal.rs`, `hex20.rs` (the parallel packages F-396 and F-496 work there).

### D8. The set-ups

Material: `E = 2e11 Pa`, `rho = 7850`, expansion 0; `nu = 0.3` for (a1), (a2); `nu = 0` for (a3) to (a7).

- **Distorted elements (a1, a2).** Seed `SEED = 0xF397_0404`, SplitMix64 as `si_hex20.rs`, one stream per kind (tag = kind
  index). Reference nodes of the kind scaled to the element size `l = 1 m` (Tet4, Tet10: the unit reference tetrahedron, the
  mid-edge nodes at the midpoints; Hex8, Hex20: the reference cube `[-1, 1]^3` times 0.5), each node coordinate moved by
  `sym(0.05 l)`. Displacement (a1): `u_a = (Q (I + G) - I)(X_a - X_c) + t + rho_a`, `X_c` the mean of the nodes, `G_ij =
  sym(0.2)`, `t_i = sym(0.2 l)`, `rho_ai = sym(0.02 l)`; `Q = I` (state 1) and `Q` the rotation by 90 degrees about the axis `(1,
  2, 3)/sqrt(14)` (state 2). `det(I + G) > 0` because every eigenvalue of `I + G` lies within `0.6` of 1 (Gershgorin). Central
  differences: `h = 1e-6 l`, column `j` of `K_fd` is `(f(u + h e_j) - f(u - h e_j)) / (2 h)`.
- **Rotation (a2).** The same distorted elements, `u_a = (Q - I) X_a + t`, `Q` as above, `t = (0.3, -0.2, 0.5) l`. Assembled:
  `hex_box([0,0,0], [2, 1, 1], [2, 1, 1])` Hex8 with the same `u` (`Q` about the same axis).
- **Bar (a3, a4, a7).** `L = 1 m`, section `a x a`, `a = 0.5 m`; Hex8 `hex_box([0,0,0], [L, a, a], [2, 1, 1])`, Tet4 `tet_box` of
  the same, Tet10 (`upgrade_to_tet10` of the Tet4 box), Hex20 (`upgrade_to_hex20` of the Hex8 box). Constraints: `u_x = 0` on
  `xmin`, `u_y = 0` on `ymin`, `u_z = 0` on `zmin`. Displacement control: `u_x = 0.2 L` on `xmax`, `NewtonOptions::new(4)`. Load
  control: `Load::traction("xmax", (P_exact, 0, 0))`, `P_exact = E 1.2 (1.44 - 1) / 2 = 0.264 E`, `NewtonOptions::new(2)`;
  `lambda_measured = 1 + mean u_x(xmax) / L`.
- **Cantilever (a5, a6).** `L = 1 m`, `h = b = 0.02 m` (`L / h = 50`), Hex20 `upgrade_to_hex20(hex_box([0,0,0], [L, h, b], [50,
  1, 1]))`; clamped (`u = 0`) on `xmin`; tip force `P = alpha E I / L^2`, `I = b h^3 / 12`, as `Load::traction("xmax", (0, P /
  (h b), 0))` (dead). Tip: the two mid-edge nodes of the tip face at `y = h / 2` (on the neutral axis, `z = 0` and `z = b`), `v`
  their mean `u_y`, `d` minus their mean `u_x`. `alpha = 1` in 5 load steps and `alpha = 2` in 10 (load increments of 0.2).
  (a5) also on a Tet10 block: `upgrade_to_tet10(tet_box([0,0,0], [1, 0.25, 0.25], [4, 1, 1]))`, clamped on `xmin`, traction `(0,
  1e7, 5e6)` Pa on `xmax`.
- **Inversion (a7).** Hex8 `hex_box([0,0,0], [1, 1, 2], [1, 1, 2])` (element 0 at `z < 1`, element 1 above), element sets
  `"lower"` = {0} with `E_0 = 1000 E` and `"upper"` = {1} with `E`; `u_x = u_y = 0` on every node (set `all`), `u_z = 0` on `zmin`,
  `u_z = -1.5 m` on `zmax` (the upper element passes `det F = 0` near load factor 2/3), `NewtonOptions::new(1)`. Only the middle
  nodes' `u_z` are free; their tangent `K_0 + K_1 (3 lambda_1^2 - 1) / 2` stays positive (`K_0 = 1000 K_1`), so the attempts fail by
  inversion and not by the compressive softening of St. Venant-Kirchhoff.
- **Halving (a7).** The Hex8 bar under load control in one step: with `max_iterations = 4` (positive: one halving, then Ok) and
  with `max_iterations = 1` (negative: refused after 5 halvings).

## Arithmetic before this registration (disclosed)

1. **Elastica reference** (Python, scratch `elastica.py`, RK4 with 4000 and 8000 steps and bisection on `theta'(0)`): nondimensional
   `theta'' = -alpha cos(theta)`, `theta(0) = 0`, `theta'(1) = 0`, `x' = cos(theta)`, `y' = sin(theta)` (`E I theta' = P (x(L) -
   x(s))`, the dead vertical tip force). `alpha = 1`: `v = 0.30172077 L`, `d = 0.05643324 L` (tip angle 0.46135); `alpha = 2`:
   `v = 0.49345748 L`, `d = 0.16064172 L` (tip angle 0.78175); 4000 and 8000 steps agree to 1e-14. They match the recalled values.
   The test integrates the same equation itself (RK4, 4000 steps, bisection) and checks its values against these to 1e-6.
2. **Scalar Newton of the uniform bar** (Python, scratch `stretch.py`): a uniform state is reproduced exactly by every kind, so the
   iteration is the scalar Newton on `P(lambda) = E lambda (lambda^2 - 1)/2`, `P' = E (3 lambda^2 - 1)/2`, with `r_k = |P_t -
   P_k| / max(P_t, sqrt(2) P_k)` (the reactions at `xmin` equal the end forces). Two steps: step 1 `r = 1, 1.2e-1, 3.3e-3, 1.9e-6,
   6.0e-13`, order of the last three above `1e-12`: 2.00; step 2 `r = 0.5, 4.1e-2, 4.5e-4, 4.9e-8, 8.9e-16`: 2.02. Converged at
   `k = 4` in both (energy ratios about `7e-12` and `1e-14`, below `1e-10`). One step: `r_4 = 1.5e-9 > 1e-10`, converged at `k =
   5`: with `max_iterations = 4` the single step fails (margin 15 in `r_4`) and its two halves are the two steps above (margin 170):
   one halving. With `max_iterations = 1`, an attempt of 1/32 still has `r_1 = 8.7e-3`: refused.
3. **Tangent error (a1).** `f_int` is a cubic polynomial in `u` for St. Venant-Kirchhoff, so the central difference has the
   truncation `h^2 f''' / 6`, relative about `(h / l)^2 = 1e-12`, and the rounding `eps |f| / (h |K|)`, `|f| ~ 0.3 l |K|`: about
   `3e-11`. Estimate `<= 1e-9` against `1e-6`.
4. **Rotation (a2).** `F^T F - I` at rounding, `|E| ~ 1e-15`, `|S| ~ 1e-15 E`, `|f| ~ |S| l^2 ~ 1e-15 E l^2`. Estimate
   `<= 1e-13 E l^2` against `1e-10`. The linear force `K u` of the same field is of order `E l^2` (negative control).
5. **Linear limit (a5).** The first correction from `u = 0` is the linear solution times `s` (`K_T(0) = K`, `f_int(0) = 0`); the
   nonlinear part is of the order of the rotations, `s alpha ~ 1e-6`: deviation about `3e-7` (cantilever), smaller on the Tet10
   block. Newton tolerance `1e-10`. Estimate `<= 1e-6` against `1e-5`.
6. **Elastica discretisation (a6).** Effects left out by the inextensible rod: axial strain `P / (E A) = alpha h^2 / (12 L^2) =
   6.7e-5` (relative to `d`: `4e-4`), shear `~ 2.4 alpha h^2 / (12 L^2)` (`3e-4` of `v`), the quadratic term of St. Venant-
   Kirchhoff (the moment changes at second order in the peak strain, 1.7 percent: `3e-4`), and the Hex20 interpolation of an arc
   of 0.034 rad per element (below `1e-3`). Estimate `<= 0.5 percent` for each of the four values against 2 percent. The linear tip
   deflection `alpha L / 3` is 10 and 35 percent off: the check discriminates.
7. **Run time.** The cantilever: 50 Hex20, 608 nodes, 1824 unknowns, 15 load steps of about 5 iterations, each a 60 x 60 tangent
   on 27 points per element and one envelope factorisation: about 1 s in release per solve. Everything else is smaller. The whole
   file: estimate under 30 s in release.

## Registered checks

All in `crates/rcfd-fem/tests/si_nonlinear.rs`, release profile, run once: `CARGO_BUILD_JOBS=4 cargo test -p rcfd-fem
--release --test si_nonlinear -- --nocapture --test-threads=1`.

### A. The acceptance

| Id | Clause | Measure | Limit | Estimate |
| --- | --- | --- | --- | --- |
| A1 | a1 | Tet4, Hex8, Tet10, Hex20 distorted, states 1 and 2: `max |K_fd - K_T| / max |K_T|` | `<= 1e-6` each | `<= 1e-9` |
| A2 | a2 | the four distorted elements and the Hex8 box: `max |f_int| / (E l^2)` | `<= 1e-10` each | `<= 1e-13` |
| A3 | a3 | the four bars, displacement control: `|P_measured / P_exact - 1|`; load control: `|lambda_measured / 1.2 - 1|` | `<= 1e-9` each | `<= 1e-12` |
| A4 | a4 | the four bars, load control in two steps: the order `p` of each step | `>= 1.8` each, three residuals above `1e-12` in each step | 2.00, 2.02 |
| A5 | a5 | cantilever (`alpha = 1`) and Tet10 block, `s = 1e-6`, one step | `<= 1e-5` each | `<= 1e-6` |
| A6 | a6 | `alpha = 1, 2`: `|v / v_ref - 1|`, `|d / d_ref - 1|` | `<= 0.02` each | `<= 0.005` |
| A7 | a7 | inversion: `Err(FemError::Element { element: 1, source })`, `source` an `InvalidValue` with key `"deformation gradient"` whose reason names `"load step 1 of 1"`; non-convergence (`max_iterations = 1`): `Err(FemError::Solver { solver: "newton", reason })` naming `"load step 1 of 1"` and `"5 halvings"` | as stated | as stated |

### C. Companions

| Id | Measure | Limit | Estimate |
| --- | --- | --- | --- |
| C1 | `K_T` of A1: `max |K_T - K_T^T| / max |K_T|` | `<= 1e-13` | rounding |
| C2 | the four distorted elements at `u = 0`: `max |K_T - K| / max |K|`, `K` the linear `Element::stiffness` | `<= 1e-14` | rounding or 0 |
| C3 | halving positive (`max_iterations = 4`, one step): Ok, halvings recorded `== 1`, `max |u - u_2steps| / max |u_2steps|` | `== 1`; `<= 1e-12` | 1; 0 |
| C4 | the trait entry points `element_for(kind).internal_force` / `.tangent` equal `nonlinear::element_internal_force` / `element_tangent` bit for bit (Tet4, Hex8, Tet10, Hex20); Beam2 still refuses naming ADR 0294 | as stated | as stated |
| C5 | refusals: a model with a `Linear` constraint and one with a temperature load (`InvalidValue`, key `"nonlinear"`); `steps = 0`; a state of the wrong length (`FieldLength`, key `"state"`) | as stated | as stated |
| C6 | the Tet10 block of A5 at full load (5 steps) solved with `threads = 1` and `threads = 4`: displacements bit-equal; `nonlinear::tangent` bit-equal for 1 and 4 threads | bit-equal | bit-equal |
| C7 | the four bars, displacement control: `max |u_y|, |u_z|` over the nodes `/ L` (`nu = 0`: no lateral strain) | `<= 1e-12` | rounding |
| C8 | the cantilever: the elastica in the test reproduces arithmetic 1 | `<= 1e-6` | `1e-14` |

### N. Negative controls

| Id | Control | Expected |
| --- | --- | --- |
| N1 | A2's fields with the linear force `K u` | `max |K u| / (E l^2) >= 1e-2` for each element |
| N2 | A1 state 2 with `K_T - K_sigma(S)` (the material part only) against `K_fd` | relative difference `>= 1e-3` for each element |
| N3 | A6 with the linear solve: `|v_lin / v_ref - 1|` | `>= 0.05` for both loads |

### M. Planned mutations (on a scratch copy, after the registered run, each alone)

| Id | Mutation | Expected to be caught by |
| --- | --- | --- |
| M1 | the tangent without `K_sigma` | A1, A4 |
| M2 | `B_NL` with `F = I` in the internal force | A1, A2 |
| M3 | `E = (F + F^T)/2 - I` (the linear strain) | A2, A3 |
| M4 | no halving (a failed attempt is an error at once) | C3 |
| M5 | no inversion test | A7 (inversion) |
| M6 | the prescribed increment `dg` dropped from the first iteration | A3 (displacement control) |
| M7 | a failed attempt not rolled back (the next attempt starts from its last iterate) | C3 |

### S. Existing tests

| Id | Measure | Limit |
| --- | --- | --- |
| S1 | `CARGO_BUILD_JOBS=4 cargo test -p rcfd-fem --release --no-fail-fast` | 0 failed (the ignored record of D3 excepted) |
| S2 | `cargo check -p rcfd-post --tests` | builds |
| S3 | `cargo fmt -p rcfd-fem --check`, `cargo clippy -p rcfd-fem --all-targets -- -D warnings` | clean |

`rcfd-api`'s `baseline_hashes` is not run (brief: tests of `rcfd-fem` only): the only change to an existing file of the solver is
the body of the two entry points that refused, which no path of any crate calls (searched). Recorded.

## Decision rule

- **done**: A1 to A7 met, C1 to C8 met, N1 to N3 as expected, S1 to S3 met, and M1 to M7 each caught by at least one check.
- **partial**: any A check missed (kept as a `*_known_miss` record with a diagnosed cause) or not run, or a mutation not caught;
  the rest met.
- **todo**: the code does not build, or more than two A checks are missed.

## Limits (what is not covered)

- Dead loads only: no follower pressure, no arc-length control, no nonlinear dynamics, no snap-through (ADR 0290: advanced tier).
- The linear elastic material only (St. Venant-Kirchhoff); it is unsuitable for large compressive strain (it softens below
  `lambda = 1/sqrt(3)` in uniaxial stress). No neo-Hookean material (not asked by ADR 0300). The state slot is plumbed but no
  material with history exists, so commit and roll-back are exercised with empty states only.
- Prescribed displacements only; the transformation path (linear constraints, rigid sets, cyclic constraints, elastic supports)
  and temperature loads are refused. Beam2, Shell4 and the Hex8 formulations of ADR 0293 are not offered.
- No line search; the increment is halved, never grown again within a step.
- Tet4 locks in bending and is checked on uniform states only (ADR 0300). The elastica is checked on Hex20 only.
- The result carries no stress field (the second Piola-Kirchhoff stress is available per element through
  `element_second_piola_kirchhoff`); output is not extended.

## Amendment 1 (2026-11-13, after the registered run of `si_nonlinear.rs` and before the corrected run of A5)

**What happened.** The registered run (commit 3a86b510, `cargo test -p rcfd-fem --release --test si_nonlinear -- --nocapture
--test-threads=1`, run once) met every check it reached except **A5**: on the cantilever at the load factor `s = 1e-6` the solve
returned `Err(FemError::Solver { solver: "newton", reason: "load step 1 of 1 (load factor 0e0 to 3.125e-2) after 5 halvings: not
converged in 20 iterations (relative residual 4.203887132749448e-5, energy ratio 6.05848702994758e-12)" })`. The energy criterion
was met; the residual stalled far above `1e-10`. The test panicked at that `unwrap`, so the Tet10 half of A5 was not reached.
A5 is a miss of the registered run; the test is kept as the ignored record `a5_linear_limit_known_miss` (body unchanged).

**Diagnosis** (on a scratch copy, nothing tuned): with the residual tolerance set out of reach and no halving, the residual of
the same cantilever stalls at `1.4e-6` (`s = 1e-6`), `1.4e-8` (`s = 1e-4`) and `1.2e-10` (`s = 1e-2`): a floor of about
`1.4e-12 / s`, inversely proportional to the strain. It is the cancellation in D1's `E = (F^T F - I) / 2`: `F^T F` is about `I`
plus the strain, so the strain is computed with an absolute error of about `eps` instead of a relative one; at the strain of
`1e-8` of this load the stress is wrong in its eighth digit, and the internal forces, which cancel at the interior nodes to about
`1/50` of their size, leave a residual of order `50 eps / 1e-8`. The registered limit is not changed.

**Correction (D1').** The Green-Lagrange strain is computed from the displacement gradient `H = F - I = sum_a u_a (x) g_a`
without forming `F^T F - I`: `E = (H + H^T + H^T H) / 2` (the same quantity in exact arithmetic; its rounding is relative to `H`).
`B_NL` still uses `F = I + H`. Nothing else changes. Expected: the residual floor becomes independent of `s` (about `1e-14` at
every load), A5 is then limited by the nonlinearity (`3e-7`, arithmetic 5). At `u = 0` the strain is exactly zero both ways, so
C2 keeps its bits; the other checks change only in their rounding.

**Corrected run.** `a5_linear_limit_corrected`, the body of A5 unchanged, the limits of A5 (`<= 1e-5` for the cantilever and the
Tet10 block), run once after the code of D1' by the same command as the registered run (the whole file, so the other checks run
again on the corrected code: their registered results stay those of the first run, the values of the second run are reported
beside them). By the decision rule A5 counts as missed in the registered run.

## Addendum 1: results (2026-11-13)

Runs: the registered run of `si_nonlinear.rs` on commit 3a86b510 (run 1, once); after amendment 1 (commit 00fcbdfa) and its code
(the strain from `H`, commit 12e0ed0c), the corrected run of the whole file (run 2, once), `CARGO_BUILD_JOBS=4 cargo test -p
rcfd-fem --release --test si_nonlinear -- --nocapture --test-threads=1`, 2.4 s and 1.2 s. The registered result of every check
except A5 is that of run 1; run 2's value is given beside it where it differs in a printed digit.

### Results

| Id | Measure | Limit | Run 1 | Run 2 | Met |
| --- | --- | --- | --- | --- | --- |
| A1 | `max |K_fd - K_T| / max |K_T|`, Tet4 / Hex8 / Tet10 / Hex20, state 1 | `<= 1e-6` | 5.3e-11 / 6.9e-11 / 4.4e-11 / 2.7e-11 | 3.8e-11 / 5.1e-11 / 3.1e-11 / 2.0e-11 | yes |
| A1 | same, state 2 (with the 90 degree rotation) | `<= 1e-6` | 6.3e-11 / 6.1e-11 / 3.4e-11 / 4.5e-11 | 7.4e-11 / 7.6e-11 / 3.3e-11 / 4.8e-11 | yes |
| A2 | `max |f_int| / (E l^2)`, Tet4 / Hex8 / Tet10 / Hex20 / Hex8 box | `<= 1e-10` | 8.3e-17 / 7.5e-17 / 7.1e-17 / 2.1e-16 / 1.8e-16 | 6.7e-17 / 7.4e-17 / 9.4e-17 / 2.0e-16 / 1.6e-16 | yes |
| A3 | displacement control `|P / P_exact - 1|`, Tet4 / Hex8 / Tet10 / Hex20 | `<= 1e-9` | 1.1e-16 / 6.7e-16 / 8.9e-16 / 1.1e-16 | 2.2e-16 / 1.1e-16 / 3.3e-16 / 2.2e-16 | yes |
| A3 | load control `|lambda / 1.2 - 1|` | `<= 1e-9` | 2.2e-16 / 2.2e-16 / 2.2e-16 / 0 | 2.2e-16 (each) | yes |
| A4 | order, step 1 / step 2 (every kind) | `>= 1.8` | 2.078 / 2.018 | 2.078 / 2.018 | yes |
| A5 | cantilever `max |u_nl / s - u_lin| / max |u_lin|` | `<= 1e-5` | not converged (Err, residual 4.2e-5) | 2.0e-7 | **no** (run 1); yes after amendment 1 |
| A5 | Tet10 block | `<= 1e-5` | not reached | 2.5e-9 | not run (run 1); yes after amendment 1 |
| A6 | `alpha = 1`: `|v / v_ref - 1|`, `|d / d_ref - 1|` | `<= 0.02` | 2.4e-4, 4.8e-4 | same | yes |
| A6 | `alpha = 2`: same | `<= 0.02` | 3.0e-4, 5.8e-4 | same | yes |
| A7 | inversion: `Element { element: 1, InvalidValue "deformation gradient" }`, "load step 1 of 1" | as registered | as registered (`det F = -3.1e-2` at load factor 0.6875, after 5 halvings) | same | yes |
| A7 | non-convergence: `Solver { "newton" }`, "load step 1 of 1", "5 halvings" | as registered | as registered | same | yes |
| C1 | `max |K_T - K_T^T| / max |K_T|` | `<= 1e-13` | <= 2.9e-16 | <= 1.9e-16 | yes |
| C2 | `max |K_T(0) - K| / max |K|` | `<= 1e-14` | 0 (bit-equal, every kind) | same | yes |
| C3 | halvings; `max |u - u_2steps| / max |u_2steps|` | `== 1`; `<= 1e-12` | 1; 0 (bit-equal) | same | yes |
| C4 | trait entry points = module functions; Beam2 refuses naming ADR 0294 | bit-equal; as stated | met | met | yes |
| C5 | refusals (linear constraint, temperature, steps 0, state length) | as stated | met | met | yes |
| C6 | threads 1 and 4: solve and tangent | bit-equal | met | met | yes |
| C7 | `max |u_y|, |u_z| / L` | `<= 1e-12` | <= 3.6e-17 | <= 2.3e-17 | yes |
| C8 | elastica in the test against arithmetic 1 | `<= 1e-6` | 3.8e-9, 8.3e-10 | same | yes |
| N1 | `max |K u| / (E l^2)` | `>= 1e-2` | 0.33 / 0.56 / 0.27 / 0.73 | same | as expected |
| N2 | material part only | `>= 1e-3` | 0.26 / 0.17 / 0.27 / 0.14 | same | as expected |
| N3 | linear `|v_lin / v_ref - 1|` | `>= 0.05` | 0.105, 0.351 | same | as expected |

Iterations recorded: the displacement-controlled bars 2 per step (the predictor with the prescribed increment is already the
uniform solution); the load-controlled bars 4 per step, the residuals as arithmetic 2 predicted to three digits (step 1: `1, 0.121,
3.31e-3, 1.87e-6, 5.9e-13`); the elastica 5 to 6 per step, no halving at load increments of 0.2.

### Mutations (scratch copy, each alone, the whole file run once per mutation)

| Id | Mutation | Expected | Caught by |
| --- | --- | --- | --- |
| M1 | tangent without `K_sigma` | A1, A4 | A1 (0.12 to 0.33), A4 (order 1.00), A6 and C3 (the solves fail) |
| M2 | `B_NL` with `F = I` | A1, A2 | A1 (up to 1.3), A3 (0.17 and 3.0e-2), A4 (order 1.00), A6, A7; A2 not (the force of a rigid motion stays zero: `S = 0`) |
| M3 | linear strain | A2, A3 | A1, A2 (0.30 to 1.33 E l^2), A3 (9.1e-2), A4 |
| M4 | no halving | C3 | C3 (the one-step solve fails), A7 non-convergence (refused after 0 halvings) |
| M5 | no inversion test | A7 inversion | A7 inversion (Ok, the upper element passed through `det F < 0`) |
| M6 | `dg` dropped from the first iteration | A3 | A3 (the displacement-controlled solve fails: the energy criterion has `e_1 = 0`), A7 inversion (refused as a step) |
| M7 | a failed attempt not rolled back | C3 | **not caught**: C3's deviation `2.8e-16` is below its limit `1e-12`; the printed bit-equality is `false` (it is not a registered limit) |

Diagnosis of M7: the material is hyperelastic, so the converged state does not depend on the path; a half step that starts from
the last iterate of the failed full step (close to the solution at load factor 1) converges to the same solution at load factor
0.5 within the residual tolerance, and the end state differs only in rounding. The limit `1e-12` cannot see it; bit-equality,
which a correct roll-back gives exactly (both paths start each attempt from the same committed bits), can. A path-dependent
material would see it in its result.

### Existing tests, format, lint

- S1: `CARGO_BUILD_JOBS=4 cargo test -p rcfd-fem --release --no-fail-fast`: 262 passed, 0 failed, 57 ignored (the ignored records
  of the crate, with `si_scaffold::x2_entry_points_refuse_by_default_after_geometric_landed` and
  `si_nonlinear::a5_linear_limit_known_miss` among them). Met.
- S2: `cargo check -p rcfd-post --tests`: builds. Met.
- S3: `cargo fmt -p rcfd-fem --check` clean (after a layout-only rustfmt commit, 7c298a5e); `cargo clippy -p rcfd-fem --all-targets
  --release -- -D warnings` clean. Met.
- Not run: `rcfd-api`'s `baseline_hashes` (as registered: the brief limits the tests to `rcfd-fem`; the shared-file change is the
  body of two entry points that refused and that no path of any crate calls). The whole workspace suite (the lead runs the gate).

### Deviations

- A5 missed in the registered run (amendment 1): a defect of D1's strain evaluation, corrected by computing the same quantity from
  `H`; the limit unchanged; the corrected run met it (`2.0e-7`, `2.5e-9`).
- The order measure of A4 left out no residual in practice except the last ones at `6e-13` to `3e-15` (below the floor `1e-12`).
- Shared files: `crates/rcfd-fem/src/element.rs` lines 305 to 327 (the doc lines and bodies of the default `internal_force` and
  `tangent`; nothing else); `crates/rcfd-fem/tests/si_scaffold.rs` line 903 (one `#[ignore]` attribute, the body untouched).
  `geometric.rs`, `solve.rs`, `buckling.rs`, `modal.rs` and `hex20.rs` are not edited.

### Decision by the registered rule

**partial.** A1 to A4, A6, A7 met in the registered run; **A5 missed** in the registered run (the Newton driver did not converge at
the load factor `1e-6`, diagnosed as the cancellation in `F^T F - I`) and met after amendment 1; C1 to C8 met; N1 to N3 as expected;
S1 to S3 met; M1 to M6 caught, **M7 not caught**. Two conditions of "done" fail (an A check missed in its registered run, a mutation
not caught), none of "todo".

### Remaining questions

1. **A5 after amendment 1.** Options: (a) accept the corrected run (same limit, the formula identical in exact arithmetic) and count
   A5 met; (b) keep A5 missed until a new registration re-runs it. Recommendation: (a); the correction is a numerical defect found by
   the check, not a tuned parameter, and the corrected value has a margin of 50.
2. **M7 (roll-back not observable).** Options: (a) a follow-up that registers C3's bit-equality (one halving against the two-step
   solve) as a limit; (b) wait for a material with history (J2 plasticity, advanced tier), whose result would show a missing
   roll-back. Recommendation: (a), size S, now: it costs one assertion and closes the gap for every later material.
3. **Defaults of the driver** (20 iterations, `1e-10`, `1e-10`, 5 halvings): taken on the maintainer's standing instruction of
   2026-11-11 (take the recommendation). They met every check here; a larger model may want a looser residual (the floor of the
   residual grows with the condition of the tangent). No change proposed.

## For the lead: roadmap entries

F-397, proposed status **partial**, note:

> Structural solver, intermediate tier (plan ADR 0290, 2026-11-12). Size M. ADR 0404, 2026-11-13 (registration of stub ADR 0300): nonlinear::solve, total Lagrangian Tet4, Hex8, Tet10 and Hex20 with St. Venant-Kirchhoff, tangent K_mat(F) + K_sigma(S) (the geometric stiffness of ADR 0390), Newton with load steps, residual and energy criteria, step halving, inversion and non-convergence refused naming the element or the step; Element::internal_force and tangent filled. Tangent against central differences 2e-11 to 8e-11 (limit 1e-6), rigid rotation 2e-16 E l^2, uniform stretch nominal stress 1e-16, order 2.02 to 2.08, elastica within 2.4e-4 to 5.8e-4 of the ODE (limit 2e-2). Partial: the linear limit (load factor 1e-6) missed in its registered run (cancellation in F^T F - I; met after amendment 1 at 2e-7), and a missing roll-back of a failed attempt is not caught by a registered check (follow-up F-NEW).

Follow-up item (number to be given by the lead), title "Nonlinear driver: roll-back of a failed attempt as a registered check", deps F-397, acceptance:

> Every limit not stated here is registered in the ADR before the run. The one-step solve of ADR 0404's load-controlled Hex8 bar with four iterations allowed (one halving) equals the two-step solve bit for bit, and the mutation of ADR 0404 M7 (a failed attempt not rolled back) is caught by that check; every check of ADR 0404 keeps its value.
