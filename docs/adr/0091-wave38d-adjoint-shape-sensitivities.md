# ADR 0091: Wave 38D, adjoint shape sensitivities of a laminar 2D duct with FFD morphing (F-137)

Status: registered before any adjoint code was written. Limits are fixed; a miss becomes an `#[ignore]`d `*_known_miss` test with the
limit unchanged, a defective set-up gets a NEW corrected test and the old one stays ignored. Baseline hashes are never re-recorded;
everything here is opt-in (a new module, no existing code path changes). Clean-room: first principles only; no reference value from
literature enters a gate (the reference derivative is a finite difference of rcfd's own converged solutions).

Roadmap entry F-137 (deps F-028 done, F-095 partial). Acceptance text: "Drag sensitivity of a 2D duct with respect to a few shape
parameters matches finite differences within 5 percent". The existing design loop (`crates/rcfd-api/src/design`, ADR 0067) has
samplers, Nelder-Mead and differential evolution only; it has no gradient and no geometry parameters.

## What was run before this ADR was written (disclosed)

The set-up helpers (`design::adjoint::{Ffd, Duct, pressure_drop, wall_drag, solve_primal}`) and the ignored set-up smoke test
`duct_setup_smoke` (crates/rcfd-api/tests/adjoint_duct.rs) were written and run once, release build, HEAD bf2aa83 plus these files.
No adjoint code existed. Measured:

* Base shape `a0 = (0.10, 0.20, 0.05)`: cold start stagnates after 272 SIMPLEC iterations at `max(res_u, res_p)` 9.9e-15 (0.3 s);
  dp = 2.6838027830, wall drag Fx = 0.27085719594; smallest cell volume 8.9e-4 (no inversion).
* One SIMPLEC iteration at the converged state: 0.7 to 1.1 ms; `|G(X*) - X*|` max 1.6e-15 (u), 0 (p), 1.0e-17 (phi).
* Pressure inner iterations at `X* +- 1e-6 e_j` (cells 0, 100, 399): 30/30, 32/32, 32/32 (equal on both sides of each column).
* Internal faces with `|phi| < 1e-5 max|phi|`: 0 of 750 (no face near the upwind switch at the state step used).
* Noise floor: shape `a0 + 1e-3 e_2` solved cold (267 its, res 7.1e-15) and warm from the base solution (192 its, 6.8e-15):
  `|J_cold - J_warm|` = 4.1e-13 (dp), 2.1e-14 (Fx). At h = 1e-3 that is 4e-10 and 2e-11 in the derivative (3e-10 relative).
* Finite-difference step study, central differences of cold-start converged solutions:

| param | h | dD(dp)/da | dD(Fx)/da |
|---|---|---|---|
| a1 | 1e-3 | 1.2885669104 | 0.12925919544 |
| a1 | 2e-3 | 1.2885695896 | 0.12925948709 |
| a2 | 1e-3 | 1.3284876841 | 0.13320823825 |
| a2 | 2e-3 | 1.3284897415 | 0.13320844549 |
| a3 | 1e-3 | 1.1582573656 | 0.11629486646 |
| a3 | 2e-3 | 1.1582598160 | 0.11629510263 |

  The two steps agree to 2.1e-6 relative at worst (truncation, O(h^2)); the noise floor is 4 orders below that. So the reference
  derivative D(h = 1e-3) is good to about 1e-6 relative. All six reference values were therefore known when the limits below were
  written; all are of comparable size (no derivative is small next to the others), so no denominator floor is registered.

## Set-up (registered)

* Mesh: `rcfd_mesh::gen::channel` 2D, length 4, height H = 1, depth 0.1, 40 x 10 uniform cells; patches `inlet`, `outlet`,
  `walls` (bottom and top), `sides` (empty).
* Flow: parabolic inlet `u = 6 s (1 - s)` (mean 1), outlet zero pressure and zero-gradient velocity, no-slip walls, nu = 0.05
  (Re = U H / nu = 20). SIMPLEC with its defaults (alpha_u 0.9, alpha_p 1.0, upwind convection, no non-orthogonal correctors),
  inner linear solves to round-off (`tol_abs 1e-15, tol_rel 0`, caps 5000 / 20000) so that one outer iteration is a smooth map.
* Morph (free-form deformation): Bernstein lattice of degree (4, 1) over the box x 1..3, y 0..1. Parameters a1, a2, a3 are the
  y displacements of the bottom-row control points 1, 2, 3 (control points 0 and 4 stay, so the morph is continuous at the box
  edges and the inlet and outlet never move). Every mesh point inside the box moves by `sum_k a_k B_k^4(xi) (1 - eta)`, so the
  bottom wall gets the bump `sum_k a_k B_k^4(xi)` and the interior nodes follow linearly to the fixed top wall. Base shape a0 above.
* Objectives: J1 = dp = area-mean boundary pressure on `inlet` minus that on `outlet` (kinematic); J2 = Fx = x force of the
  fluid on `walls`, pressure plus viscous (`rcfd_post::integrals::wall_force_and_moment`, rho = 1). "Drag of a duct" is read as
  both: the pressure drop it costs and the wall force it carries.
* Primal convergence for every evaluation: `solve_primal(tol 1e-14, patience 400, cap 30000)` from rest (stagnation, not a residual
  target).

## Formulation comparison (registered decision)

| criterion | C1 continuous adjoint | C2 hand-transposed discrete adjoint of SIMPLE | C3 fixed-point adjoint, FD-assembled Jacobian of the iteration map |
|---|---|---|---|
| consistent with the discrete primal rcfd converges to | no: differs by O(h) on a 10-cell-high mesh, can exceed 5 percent | yes, if every term is transposed correctly | yes, by construction (it differentiates the solver's own iteration) |
| code size / risk | adjoint PDEs, adjoint BCs for dp and Fx, surface formula, its own solver | Rhie-Chow with relaxed a_p, SIMPLEC compensation, h_from_solution, lagged inflow, BC coefficients all by hand; redo per option | one map evaluation per state unknown, a dense LU, ~200 lines |
| cost | ~ one primal per objective, independent of the parameter count | same | 2N one-iteration evaluations + O(N^3) LU, independent of the parameter count; only small meshes (N up to a few thousand) |

Decision rule (registered): the gate is agreement with FD of the discrete model, so consistency is required; among consistent
options take the smallest code. Result: **C3**. Label everywhere: "fixed-point discrete adjoint with a finite-difference-assembled
dense Jacobian of the one-iteration SIMPLE/SIMPLEC map; small meshes only".

Why C3 is a discrete adjoint and not an approximation of one: the state is `X = (u active components, p, phi on non-empty faces)`,
which is everything `SimpleState` (snapshot/restore) carries, and one outer iteration is a map `X' = G(X; mesh(a))`. Any convergent
iteration can be written `G(X) = X - P(X) R(X)` with `R` the discrete residual and `P` the (relaxation-, preconditioner-dependent)
update operator. At an exact fixed point `R = 0`, so `I - G_X = P R_X` and `(I - G_X)^-1 G_a = -R_X^-1 R_a`: the sensitivity of the
discrete residual root, independent of the relaxation and of `P`. Hence `dJ/da = J_a + lambda^T G_a` with `(I - G_X)^T lambda = J_X^T`.
The equivalence needs a converged fixed point, so `max |G(X*) - X*|` is reported with every gradient.

Implementation (registered): `G_X` by central differences in each state unknown with step `1e-6 x (max |.| of its block u, p or
phi)`; `J_X` likewise; `G_a`, `J_a` by central differences of the mesh morph at the frozen converged state with step 1e-6; dense LU
with partial pivoting of `(I - G_X)^T`. Refused with an error (hidden state outside `SimpleState`, or not laminar): `nu_face` set
(turbulence: the frozen-turbulence assumption is NOT made; turbulent cases are refused), `body_force`, `driver`, `pseudo_control`,
`face_jump`; and a state larger than `max_state` (default 4000 unknowns; the dense matrix is `8 N^2` bytes).

What the gradient is accurate for: the derivative of the discrete objective of the converged discrete solution on the given mesh,
with the given settings, with respect to the FFD parameters including the interior-node motion of the morph. It is not the
derivative of the mesh-converged (continuous) quantity, which differs by the discretisation error of the primal. It assumes the
one-iteration map is differentiable at `X*`: inner solves converged to round-off, and no face flux near the upwind switch (or a
limiter switch) within the state step. Accuracy limit from the FD assembly: about 1e-6 relative (to be measured, A2).

## Registered checks

* **A0 (precondition of the reference, re-measured in the gate test)**: at a0 and at all 12 FD shapes the primal stagnates with
  `max(res_u, res_p) <= 1e-13`; step study `|D(1e-3) - D(2e-3)| <= 5e-3 |D(1e-3)|` for all six pairs; noise floor
  `|J_cold - J_warm| / 1e-3 <= 1e-3 |D(1e-3)|` at `a0 + 1e-3 e_2` for both objectives. If A0 fails, the set-up is defective, not
  the adjoint.
* **A1 (acceptance, F-137)**: for J in {dp, Fx} and k in {1, 2, 3}: `|g_adj,k - D_k(1e-3)| <= 0.05 |D_k(1e-3)|`. All six pairs
  must pass. Test `duct_adjoint_matches_fd` (not ignored).
* **A2 (smoothness diagnostic)**: the adjoint gradient with state step 1e-6 and with 1e-5 (x block max) agree within 1e-4 relative
  for all six pairs. Ignored test `duct_adjoint_state_step_diagnostic`, run once. A failure here with A1 passing is reported as
  non-smoothness, not hidden.
* **A3 (gradient option of the design loop)**: `optim::projected_gradient` (bound-constrained steepest descent with Armijo
  backtracking, new) on dp with the adjoint gradient, bounds `a_k in [-0.1, 0.25]`, from a0, initial step moving the largest
  parameter by 0.05, at most 3 iterations: every accepted iterate lowers dp, and the final dp `<= 0.95 x dp(a0)`. Test
  `duct_gradient_descent`; it is ignored as heavy if one adjoint gradient takes over 10 s in the gate build (decided from the
  A1 timing, recorded).
* **A4 (guards)**: `adjoint_gradient` returns an error for a solver with `nu_face` set and for a state above `max_state`. Unit tests.
* **A5 (defaults)**: no existing code path changes; `baseline_hashes` passes unchanged (gate).
* **Cost (recorded, not gated)**: wall time of one adjoint gradient (primal excluded) against 2 x 3 converged FD solves.

Status rule (registered): F-137 becomes `done` if A1 passes all six pairs with A0 holding in the gate; `partial` if a tested slice
lands but A1 misses; `todo` if nothing usable lands. The roadmap SCOPE extras (RBF morphing, more observables, topology and
turbulence-coefficient optimisers, multi-objective support, study / service / MCP exposure of gradients) are not part of the
acceptance text and are listed as not delivered.

## Addendum: measured values (release build, scratch tree on HEAD 37c2c8b plus the wave-38D files)

Delivered: `crates/rcfd-api/src/design/adjoint.rs` (FFD morph `Ffd`, the duct set-up `Duct`, objectives `pressure_drop` and
`wall_drag`, `solve_primal`, `adjoint_gradient`, `shape_gradient`, a dense LU), `optim::projected_gradient` with `PgOptions`,
tests `crates/rcfd-api/tests/adjoint_duct.rs` and unit tests in both modules, a section in docs/design-loop.md. No existing code
path changed.

| check | limit | measured | verdict |
|---|---|---|---|
| A0 primal residual at a0 and the 12 FD shapes | <= 1e-13 | asserted at every point; base 9.9e-15 (smoke 7.1e-15 at a0 + 1e-3 e2) | pass |
| A0 step study, worst of 6 pairs | <= 5e-3 | 2.26e-6 (Fx a1) | pass |
| A0 noise floor / D, worst of 6 pairs | <= 1e-3 | 3.5e-10 (dp a3) | pass |
| **A1 adjoint vs D(1e-3), worst of 6 pairs** | **<= 0.05** | **1.12e-6 (dp a1)** | **pass** |
| A2 state step 1e-6 vs 1e-5, worst of 6 pairs | <= 1e-4 | 2.4e-7 (dp a3) | pass |
| A3 final dp / dp(a0), monotone accepted iterates | <= 0.95 | 2.2935364 / 2.6838028 = 0.855 in 3 iterations, every iterate lower, no step rejected | pass |
| A4 guards | error | `nu_face` and `max_state` refused (unit test) | pass |
| A5 defaults | unchanged | no existing code path edited; gate below | see gate |

A1 per pair (adjoint / D(1e-3) / relative difference): dp a1 1.2885654712 / 1.2885669104 / 1.12e-6; dp a2 1.3284866030 /
1.3284876841 / 8.1e-7; dp a3 1.1582562470 / 1.1582573656 / 9.7e-7; Fx a1 0.12925909152 / 0.12925919544 / 8.0e-7; Fx a2
0.13320816779 / 0.13320823825 / 5.3e-7; Fx a3 0.11629478818 / 0.11629486646 / 6.7e-7. The residual difference is the same size
as the O(h^2) truncation of the reference itself (D(1e-3) and D(2e-3) differ by 1.6e-6 to 2.3e-6), so the 5 percent limit is met
with a margin of four orders; the test cannot tell the adjoint error from the reference's truncation below about 1e-6.

Fixed point: `max |G(X*) - X*|` = 1.55e-15. State N = 2050 (800 u, 400 p, 850 phi).

Cost (recorded): one adjoint gradient (both objectives, primal excluded) 6.1 s; one primal solve from rest 0.33 s; the 12 FD
solves of the step study 4.0 s. For these 3 parameters the adjoint costs about 3 times more than one-step central differences (6
solves); it breaks even at about 9 parameters on this mesh, because its cost does not grow with the parameter count. A3 used one
adjoint per iteration (6.1 s < 10 s, so the test is not ignored, as registered); its whole run took about 30 s.

Deviations from the registered plan: none in limits or set-ups. The test runs the noise-floor warm start from the base solution as
registered. A3 needed no step halving, so the backtracking branch is exercised only by the unit test of `projected_gradient`.

Not delivered (stated, not hidden): RBF morphing (FFD only); observables other than pressure drop and wall x-force (moments, centre
of pressure, cumulative forces); turbulent gradients (refused, no frozen-turbulence variant); transient, energy, body-force, driver
or pseudo-controller cases (refused); meshes above a few thousand state unknowns (dense Jacobian; a sparse or matrix-free
transposed solve is the upgrade path); gradients inside `design::study` (the `Method` / `Evaluator` / store have no gradient
slot), the `design.*` service methods, MCP tools and case files; topology, turbulence-coefficient and multi-objective gradient
optimisers. Only one configuration was verified (SIMPLEC, upwind, no non-orthogonal correctors, Re 20); other schemes and options
are accepted by the code but not tested, and a limited scheme with an active limiter switch would make the map non-smooth.

Status (by the registered rule): A1 passes all six pairs with A0 holding, so F-137 is set to `done`; the SCOPE extras above are
listed as not delivered in its note.
