# ADR 0490: F-578 registration: geometrically nonlinear shells in the Newton driver (corotational facets)

Status: **REGISTERED 2026-11-14, written before any code of F-578 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-578 was written in the repository. One probe was run with the existing linear code
only (P1, disclosed below, from a scratch crate outside the repository). The bit record N1 of the default paths is a test file
written and run on the base after this commit and before any library code (D6). Results go in the addenda at the end; nothing
above the addenda is changed after a run.

Follow-up of F-573 ([ADR 0487](0487-f573-registration-follower-pressure-on-shells.md)), proposed in its addendum. Roadmap item
F-578 (deps F-573). Number 0490 given by the lead. Branch `f578-work` at e90c2350, worktree `w49i-work/f578`, scratch
`w49i-work/f578-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code, the crate graph
unchanged; clean-room.

## What F-578 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. Shell4 and Shell3 gain an internal force and a consistent tangent at finite displacements and rotations (a corotational facet formulation, its rotation update registered), taken by nonlinear::solve and solve_with with every default bit-identical; the tangent against central differences of the internal force within 1e-6; a cantilever strip of shells under a tip moment rolls up to a full circle within 1 percent of the closed-form tip position; a thin cylinder of shells under the follower internal pressure of ADR 0487 follows p = E t (lambda^2 - 1)/(2 R) within 1 percent to the stretch 1.5 (the clause of F-573).

Clauses: (a0) every limit registered here; (a1) internal force and consistent tangent of Shell4 and Shell3 at finite
displacements and rotations, a corotational facet formulation with its rotation update registered; (a2) taken by
`nonlinear::solve` and `solve_with`, every default bit-identical; (a3) the tangent against central differences of the internal
force within `1e-6`; (a4) the tip-moment roll-up of a cantilever strip to a full circle within 1 percent of the closed-form tip
position; (a5) the thin cylinder under the follower internal pressure of ADR 0487 within 1 percent of `p = E t (lambda^2 -
1)/(2 R)` to the stretch 1.5.

The brief adds: (b1) extend the element dispatch of the existing driver, no second Newton loop; (b2) register the update of
the rotations (additive pseudo-vector or multiplicative) with its reason; (b3) reuse the linear Shell4 and Shell3 element
matrices (ANDES options included) as the corotated local response; (b4) a dead or a follower moment for the roll-up,
registered, with the closed form derived; (b5) defaults and every solid-only nonlinear result keep their bits,
`baseline_hashes` unchanged, a default-bits test at the base before any library change; (b6) if the scope is too large, stop at
force, tangent and roll-up and register the cylinder as a predicted miss, without shrinking limits; (b7) probe before a
`1e-12`-class limit, `1e-11` for sums of about `1e3` terms, every aggregated error finite (a NaN must fail, not vanish in an
`f64::max`); (b8) another item (F-411) edits `nonlinear.rs` and `material.rs`: the edits of `nonlinear.rs` stay in the element
dispatch and the model check.

## What was read, and the probe (disclosed)

- `AGENT_RULES.md`; the brief; ADR 0487 (whole); the F-578 entry of `docs/roadmap.json`.
- `crates/rcfd-fem/src/nonlinear.rs` (whole); `shell.rs` (module text, `Shell4Membrane`, `ShellOptions`, `Facet`, `facet`,
  `point`, `local_stiffness_section`, `SectionStiffness`, `homogeneous_section`, `section_of`, `transformation`, `to_global`,
  `model_section`, `model_element_stiffness`, `model_membrane`, `check_shell_model`); `shell3.rs` (`Shell3Membrane`,
  `Shell3Options`, `facet`, `local_stiffness_section`, `stabilised_shear`, `transformation`, `model_element_stiffness`,
  `model_membrane`); `assemble.rs` (`ElementContext`, `element_dofs`, `assemble_with`, `point_stiffness`); `model.rs`
  (`check_elements`, `check_elements_supported_nonlinear`, `check_supported_nonlinear`, `Material`, `Load::NodalMoment`);
  `element.rs` (`Pending`); `arclength.rs` (`trace`, its checks); `nl_transient.rs` (`Integrator::new`, its checks);
  `follower.rs` (`on_shells`).
- Tests: `sa_shell_follower.rs` (the strip and C1, C1r of ADR 0487), `sa_shell_follower_default_bits.rs` (the form of a bit
  record), `si_nonlinear.rs` (the solid elastica).
- **Probe P1** (scratch crate `f578-scratch/probe`, the existing linear `solve_static`, before this text): the cantilever strip
  of D5 (`L = 12`, `b = 1`, `t = 0.1`, `E = 1.2e6`, `nu = 0`, 16 x 1 Shell4, and the same cut into 32 Shell3) clamped at
  `x = 0` under a small tip moment about `-y` (tip rotation `1e-3`): the tip rotation is `M L/(E I)` within `8.9e-11` (Shell4)
  and `8.5e-12` (Shell3), the tip deflection `M L^2/(2 E I)` within `1.3e-10` and `1.3e-11`; `u_y, theta_x, theta_z` at the tip
  below `7e-15`. Both linear elements bend exactly at constant curvature (`nu = 0`); this is the local response the corotated
  element will carry (arithmetic 5).

## D0. What exists, and the two findings that shape the design

1. The driver of ADR 0404 refuses shells in `check_model` (`check_elements(shells = false)`) and in `solve_with`
   (`check_supported_nonlinear`); `initial_states` refuses them too. The driver updates every unknown additively, `u[q] +=
   du` (ADR 0404 D5 step 4), and factors the reduced tangent by the symmetric envelope `L D L^T`.
2. **The strain measure of the cylinder clause.** The law `p = E t (lambda^2 - 1)/(2 R)` is the thin ring of a St.
   Venant-Kirchhoff membrane (Green strain, `nu = 0`; ADR 0487 arithmetic 6: the hoop tension `E t lambda (lambda^2 - 1)/2`
   balancing `p lambda R`). A corotational element whose local response is the linear membrane gives the hoop force `E t
   (lambda - 1)` (the stretch of the chord, a Biot strain), so `p = E t (lambda - 1)/(lambda R)`: the ratio to the law is
   `2/(lambda (lambda + 1))`, `0.533` at `lambda = 1.5` and already `0.985` at `lambda = 1.01`. A linear local membrane cannot
   meet (a5). So the membrane of the default membranes (the bilinear membrane of Shell4, the constant-strain membrane of
   Shell3) is taken as the total Lagrangian Green-strain membrane of the same interpolation (D2); the bending, transverse
   shear and drilling parts are the corotated linear element (b3). The two coincide to first order, so the element reduces to
   the linear one at zero displacement (L1).

## Design

### D1. Unknowns and the rotation update (b2)

The six unknowns of a shell node are `(u_x, u_y, u_z, theta_x, theta_y, theta_z)`; at finite rotation the last three are the
**total rotation pseudo-vector** `theta` of the node, the nodal triad being `R(theta) = exp([theta]_x)` times its reference
(Rodrigues: `R = I + a [theta]_x + b [theta]_x^2`, `a = sin t/t`, `b = (1 - cos t)/t^2`, `t = |theta|`). The update is
**additive**: the driver's `u += du` is the update of `theta` as of every other unknown.

Reasons, taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation): (i) the driver keeps its one
Newton loop and its one update (b1, b8): a multiplicative update `R <- exp([dw]_x) R` would need a second update path in
`Problem::attempt_with`; (ii) with additive unknowns the internal force is the gradient of the strain energy `Pi(q)` of the
unknowns themselves and the tangent is its Hessian, **exactly symmetric at every state** (a multiplicative spin tangent is not
symmetric away from equilibrium), which the symmetric `L D L^T` of the driver requires; (iii) the tangent is then the
derivative of the force in the stored unknowns, so (a3) compares like with like. The cost: the map `theta -> R` is singular
at `|theta| = 2 pi k`, `k >= 1` (the differential `T(theta)` has the determinant `2 (1 - cos t)/t^2`): there the two directions
of `theta` normal to `theta` produce no rotation and the tangent loses rank. Registered consequences: the roll-up holds the
out-of-plane unknowns `u_y, theta_x, theta_z` (the planar problem, D5), where only the axial direction of `theta` (in which `T
= I`) is free; a companion without the holds stops at `0.9` of the full circle (RU4f). Rotations of fan blades are far below
`2 pi`. A dead nodal moment `M` on the `theta` unknowns is the generalised force conjugate to `theta`, which is the spatial
moment `T^{-T} M`; for a rotation about a fixed axis `e` with `M` along `e` (the roll-up), `T e = e` and the two coincide
(D5).

### D2. The corotated element (b3; after Rankin and Nour-Omid, Felippa and Haugen, Battini and Pacoste, cited from memory)

For an element of `n` nodes (`n = 4` Shell4, `3` Shell3), reference coordinates `X_a`, unknowns `q = (u_a, theta_a)`:

1. **Reference facet** (the linear element's): `shell::facet` / `shell3::facet` give the axes `Lambda_0` (rows `e_1, e_2,
   e_3`), the projected local coordinates `Y_a = (local_a, 0)` and the offsets `h_a` (Shell4; zero for Shell3). The linear
   Shell4 joins a node to its projected point by the rigid link `r_a = -h_a e_3` (ADR 0432 arithmetic 7); at finite rotation
   the link turns with the node: **`p_a = X_a + u_a + R(theta_a) r_a`**, the current projected point (`P_a = X_a + r_a` at
   rest).
2. **Current frame** from the projected points, by the rule of the facet: `e_3 = n/|n|`, `n = (p_2 - p_0) x (p_3 - p_1)`
   (Shell4) or `(p_1 - p_0) x (p_2 - p_0)` (Shell3); `e_1` the part of `p_1 - p_0` normal to `e_3`, normalised; `e_2 = e_3 x
   e_1`. `Lambda` has the rows `e_k`; `c` the mean of the `p_a`. At rest it is `Lambda_0` up to rounding.
3. **Local deformational unknowns**: translations `d_a = Lambda (p_a - c) - Y_a`; rotations **`phi_a = log(Lambda R(theta_a)
   Lambda_0^T)`**, the rotation of the nodal triad relative to the element frame (zero at rest, invariant under a rigid
   motion: `Lambda -> Lambda Q^T`, `R -> Q R`). `log` by `phi = (t/sin t) v`, `v` the axial vector of the skew part, `cos t`
   from the trace, refused (key `"rotation"`) when `cos t <= 0` (a deformational rotation of 90 degrees or more: the mesh is
   too coarse for the rotation, the driver halves the step).
4. **Strain energy** `Pi(q) = (1/2) d^T K_r d + Pi_m(p)`, `d` the `6 n` local unknowns node by node `(d_a, phi_a)`:
   - `K_r` the linear local stiffness of the element (`local_stiffness_section` of `shell.rs` / `shell3.rs`, the model's
     section and membrane option of `model_section`, `model_membrane`, with the default options otherwise) **without its
     membrane**: for the default membranes (`Shell4Membrane::Bilinear`, `Shell3Membrane::ConstantStrain`) the section's
     membrane matrix `A` is set to zero (the bending, the transverse shear with its stabilisation and the drilling penalty
     `alpha G t` do not read `A`), and `Pi_m` is the Green-strain membrane (below); for the drilling membranes (Shell4
     `AndesOpt`, Shell3 `Allman` and `AndesOpt`), whose membrane fields carry `theta_z`, `K_r` is the whole linear local
     stiffness and `Pi_m = 0` (the corotated linear membrane: exact rigid rotations, the Biot hoop response of D0 at large
     membrane strain; registered as a limit).
   - `Pi_m = (1/2) sum_g w_g eps_g^T A eps_g`, the total Lagrangian membrane of the linear element's interpolation and rule:
     Shell4 the bilinear functions on `Y` and the 2 x 2 Gauss rule with the weights `det J` of `shell.rs`; Shell3 the linear
     functions and the area. `g_X = sum_a N_a,X p_a`, `g_Y = sum_a N_a,Y p_a` (current tangent vectors, global axes),
     `eps = ((g_X.g_X - 1)/2, (g_Y.g_Y - 1)/2, g_X.g_Y)` (Green strain, engineering shear; the reference metric is the
     identity in the orthonormal local axes of the flat projected facet). To first order `eps = (u_X, v_Y, u_Y + v_X)`, the
     linear membrane rows: `Pi` reduces to the linear element at zero displacement (L1, arithmetic 2). `Pi_m` is objective by
     itself (no frame).
5. **Force and tangent**: `f = dPi/dq`, `K = d^2 Pi/dq^2`, computed exactly by forward differentiation of second order
   (first principles, the chain rule; hyper-dual numbers after Fike and Alonso, cited from memory): every quantity of steps
   1 to 4 carries its value, gradient and Hessian with respect to the `6 n` element unknowns. With `J = dd/dq` and `s = K_r d`:
   `f = J^T s + grad Pi_m`, `K = J^T K_r J + sum_i s_i Hess d_i + Hess Pi_m`. The Hessian is formed on its upper triangle and
   mirrored: `K` is symmetric bit for bit. The functions of `t^2` (`a`, `b`, and `t/sin t` as a function of `sin^2 t`) take
   their Taylor series below `t^2 = 1e-2` (resp. `sin^2 t = 1e-2`; truncation below `1e-17`, arithmetic 1), so that the
   derivatives exist at zero rotation.
6. Refusals of the element: a degenerate current frame (key `"geometry"`), `cos t <= 0` (key `"rotation"`), a section with
   membrane-bending coupling (a layup; key `"nonlinear"`), a non-finite unknown (key `"displacement"`).

New module `crates/rcfd-fem/src/shell_corot.rs` (the element, its derivative arithmetic, `element_force_and_tangent` and
`element_internal_force` for a shell element of a model, public for the tests). No edit of `shell.rs` or `shell3.rs` (their
public and crate functions are called).

### D3. The driver (b1, b8: the dispatch and the model check only)

`nonlinear.rs`:
- `check_model`: the element check takes shells (`check_elements(beams = false, shells = true, point = false, history =
  true)`, a new crate method of `model.rs` next to `check_elements_supported_nonlinear`); the block loop lets a Shell4 or
  Shell3 block through, and for a model with shells calls `shell::check_shell_model` and refuses (key `"nonlinear"`) a shell
  block whose material is not `IsotropicElastic`. A solid block is checked as before.
- `internal_force` and `force_and_tangent`: an element of kind Shell4 or Shell3 is evaluated by `shell_corot` (no state, no
  stress points); every other element by `evaluate` as before.
- `initial_states`: the element check of `check_model` (a shell gets the empty state: one point of the placeholder rule times
  `state_size() = 0`).
- `solve_with`: `check_supported_nonlinear` is replaced by its shell-taking twin (the same supports check).
- Not changed: `check_driver_model` and the entries that call `check_supported_nonlinear` themselves (`arclength::trace`,
  `follower::buckling_factors`, `nl_transient::Integrator::new`, `solve_from`, `solve_from_at`) keep refusing shells with the
  same error (R1). `solve` is `solve_with` with no extensions.

For a model without shells every changed line executes the operations it executed before (a branch on the element kind), so
the solid results keep their bits (N1).

### D4. The tangent check (a3; T1, S1)

Configurations: the warped Shell4 element and the Shell3 element of ADR 0487 D2 (`(0, 0, 0), (1, 0, 0.06), (1.1, 0.9, 0),
(-0.1, 1, 0.08)`; `(0, 0, 0), (1, 0.1, 0.05), (0.2, 0.9, -0.03)`), the Shell4 2 x 2 and Shell3 2 x 2 x 2 patches of ADR 0487 on
the cylinder `r = 2`, and the two single elements with the ANDES membrane (Shell4 `AndesOpt`, Shell3 `AndesOpt`); `t = 0.01`,
`E = 2e11`, `nu = 0.3`, `rho = 7850`. Two states per configuration (SplitMix64, seed `0xF578_0490`, configurations in that
order, state (a) before (b)): **(a)** a rigid rotation by the Rodrigues vector `w = (0.6, -0.9, 0.4)` (`|w| = 1.153`) about
the centroid of the nodes plus a perturbation uniform in `[-0.02, 0.02]` on every unknown (`theta_a = w + delta`); **(b)** the
rigid rotation `0.05 w/|w|` plus the perturbation uniform in `[-0.2, 0.2]` on the rotations and `[-0.02, 0.02]` on the
translations. (a) puts the nodal rotations on the closed-form branch and the deformational ones on the series branch; (b) the
reverse. T1: `max |K - K_cd| / max |K|` over the whole matrix, `K_cd` column by column `(f(q + h e_j) - f(q - h e_j))/(2 h)`,
`h = 1e-6`; T1r (record): the same over the rotational rows and columns relative to their own largest entry. S1: `K` equals
`K^T` bit for bit.

### D5. The roll-up (a4; RU4, RU4b, RU3, RU4a, RU4f)

Strip `L = 12`, `b = 1`, `t = 0.1`, `E = 1.2e6`, `nu = 0` (the parameters of the end-moment cantilever popular in the shell
literature, Sze, Liu and Lo 2004, cited from memory; nothing is taken from it but the parameters), along `x` in `[0, L]`, `y`
in `[0, b]`, 16 x 1 Shell4 (RU4) or the same rectangles each cut into two Shell3 along the diagonal from the first to the third
corner (RU3). Clamped at `x = 0` (all six unknowns of both nodes). **Dead moment** (b4): `Load::NodalMoment` on the two tip
nodes, `(0, -M/2, 0)` each, `M = 2 pi E I/L`, `E I = E b t^3/12 = 100`, `M = 50 pi/3`; with `nu = 0` there is no anticlastic
coupling. Held on every node: `u_y, theta_x, theta_z` (the planar problem; the exact solution has them zero by the symmetry of
the strip and `nu = 0`, and they are the directions of the singularity of D1 at the full circle). `nonlinear::solve`,
`NewtonOptions::new(40)` (a tip rotation of 9 degrees per step).

**Closed form** (derived): the bending moment is `M` along the whole strip (a free body cut anywhere carries the tip moment;
no force), so the curvature is `kappa = M/(E I)` everywhere (Euler-Bernoulli, inextensible, `nu = 0`): the strip is a circular
arc of radius `rho = E I/M` and angle `phi = M L/(E I) = 2 pi lambda` at the load factor `lambda`. With the moment about `-y`
the strip curls towards `+z`: the tip at `x = rho sin phi`, `z = rho (1 - cos phi)`, the tip rotation `theta_y = -phi`; at
`lambda = 1` the tip is back at the root. RU4: at every converged step, `e = max over the two tip nodes |(x, z) - (x, z)_cf| /
L <= 1e-2` and the last load factor 1. RU4b: `|theta_y,tip/(-2 pi lambda) - 1| <= 1e-6` at every step.

Estimate (arithmetic 5): each element bends at constant curvature, the chord keeps its length, the nodes lie on a circle of
radius `rho_h = h/(2 sin(alpha/2))` (`h = L/16`, `alpha = phi/16`) at the exact angles, so `e = (alpha^2/24) 2 sin(phi/2)/phi`
to leading order, largest `1.18e-3` at `phi = 4.06`, zero at the full circle; the tip rotation is exact up to the Newton
tolerance (P1: the linear bending is exact).

RU4a: RU4 with the Shell4 `AndesOpt` membrane (the corotated linear membrane, no membrane strain in pure bending): same limits.
RU4f: RU4 without the holds of `u_y, theta_x, theta_z` (only the root clamped), the moment `0.9 M` in 36 steps (the same
increment, `phi` to `1.8 pi`, short of the singular `2 pi`): `e <= 1e-2` at every step (with `phi = 1.8 pi lambda`), and the
out-of-plane `max |u_y|/L, |theta_x|, |theta_z| <= 1e-6` (symmetry; rounding expected).

### D6. The cylinder (a5; CY4, CY3, CY4a, X1)

The quarter-ring strip of ADR 0487 D3 (`R = 1`, `t = 0.01`, width `0.05`, 24 elements over the quarter, 1 across; `E = 2e11`,
`nu = 0`; held `u_z, theta_x, theta_y` on every node, `u_y, theta_z` on `theta = 0`, `u_x, theta_z` on `theta = pi/2`), its
function `strip` copied: `FollowerPressure::on_shells(model, "all", p_ref)`, `p_ref = E t (1.51^2 - 1)/(2 R)`,
`nonlinear::solve_with(model, [], [], NewtonOptions::new(20), contributions = [follower])`, a recorder of the contribution's
calls giving the displacement of each converged step (as ADR 0487 C1). At every converged step `|p/p_thin(lambda) - 1| <=
1e-2`, `lambda` the mean of `|x_perp|/R` over the nodes, `p` the load factor times `p_ref`; the last `lambda >= 1.5`. CY4
Shell4, CY3 the strip cut into Shell3. Estimate: the Green membrane and the follower force of the facet polygon give `p =
E t (lambda^2 - 1)/(2 R cos(pi/96))` (ADR 0487 arithmetic 6), `+5.4e-4` at every stretch; bending is not excited (the facets
rotate rigidly with the uniform expansion). CY3: `<= 2e-3` expected (the end-ring tangential forces of the triangulation,
ADR 0487 amendment 1, are balanced by `theta_z` held only on the symmetry planes; small).

CY4a (record): CY4 with the Shell4 `AndesOpt` membrane (the corotated linear membrane), the last `p/p_thin(lambda)` recorded
(predicted `2/(lambda (lambda + 1))/cos(pi/96) = 0.53` at `lambda = 1.5`, D0; if the step sequence fails, the error recorded).
X1: ADR 0487's ignored record `c1_cylinder_follower_internal_pressure_known_miss` (the CY4 computation) run once, unchanged;
expected to pass now (recorded).

**X2, a registered consequence.** ADR 0487's regular test `c1r_the_newton_driver_refuses_shells` asserts that `solve_with`
refuses the shell strip. F-578 removes that refusal by its acceptance, so the test fails by design. Its body is not changed;
it receives the attribute `#[ignore = "superseded by ADR 0490 (F-578): the Newton driver takes shells; ADR 0487 D0 record"]`
and stays as a record of the D0 finding. This is the one edit of an existing test file and is reported to the lead (open
question 2). Every other existing test is unchanged.

### D7. The bit record (N1, b5)

`crates/rcfd-fem/tests/sa_corot_shell_default_bits.rs`, written after this commit and run on the unchanged base e90c2350
before any library code; its digests (FNV-1a 64 over the bits) written into the file then. Content, every value in a fixed
order: `nonlinear::solve` of the Hex8 elastica cantilever of `si_nonlinear.rs` (displacement, reactions, the residuals of
every step); `internal_force` and `tangent` (values of the CSR) of a Tet10 and a Hex20 block at a deformed state;
`solve_with` of a Hex8 column with the solid `FollowerPressure` contribution; a J2 bar by `nonlinear::solve` (displacement and
states); `arclength::trace` of the Hex8 column (the first 3 points); `solve_static` of a Shell4 plate and of a Shell3 plate
under `ShellPressure` and a `NodalMoment`. Only API that exists at e90c2350.

## Arithmetic done before the code (disclosed; by hand)

1. **Series.** `a(s) = sum_k (-s)^k/(2k+1)!`, `b(s) = sum_k (-s)^k/(2k+2)!`, 6 terms (`k <= 5`) for `s < 1e-2`: the first
   omitted term `s^6/13! < 1.7e-22`. `t/sin t` as a function of `sigma = sin^2 t` (valid for `cos t > 0`): `asin(x)/x = sum_k
   c_k x^(2k)`, `c_k = (2k)!/(4^k (k!)^2 (2k+1))`, 10 terms for `sigma < 1e-2`: the first omitted `c_10 1e-20 < 1e-21`. Above
   the thresholds the closed forms (`t = sqrt(s)`, `sin`, `cos`; `t = atan(sqrt(sigma)/cos t)`). The branches agree to rounding;
   a central difference that straddles a branch sees `~eps/h = 2e-10` relative, far below `1e-6`.
2. **Linear limit (L1).** At `q = 0`: `d = 0` up to rounding, so `K(0) = J_0^T K_r J_0 + Hess Pi_m(0)`. `J_0` maps the global
   unknowns to the local ones at the projected points by `Lambda_0` and the link `delta p = delta u - r x delta theta`
   (`T` of `shell::transformation`), minus the rigid part of the frame (the projector of Rankin and Nour-Omid); `K_r` and the
   linear membrane annihilate the rigid modes of the flat projected facet (the drilling penalty ties `theta_z` to the
   membrane rotation `(v_X - u_Y)/2`; the shear rows vanish for a rigid rotation about an in-plane axis), so the projector
   drops out: `K(0) = T^T (K_r + K_m) T`, `K_m` the linear membrane of the same rule, the linear element (`to_global` of the
   whole local matrix). Rounding of sums of about `24 x 24` products: estimate `1e-14` of `max |K|`; limit `1e-10`. `f(0)`:
   `|f| ~ |K| eps L`; limit `max |f(0)| <= 1e-11 max |K_lin| L`, `L` the largest node distance from the centroid.
3. **Rigid motion (L2).** A rigid rotation `Q` (Rodrigues vector `(0.6, -1.0, 1.4)`, `|w| = 1.82`, closed-form branch; and
   `0.01 w/|w|`, series branch) about the centroid plus a translation `(0.2, -0.1, 0.3)`, every `theta_a = w`: `d` and `phi`
   vanish analytically; rounding `~eps L`: `max |f| <= 1e-11 max |K_lin| L` on the six single elements of L1.
4. **Central differences (T1).** `f` is smooth (analytic between the branch points). Rounding `eps |f|/h` with `|f| <= |K|
   |d|`, `|d| <= 0.2`: `2.2e-16 x 0.2/1e-6 = 4.4e-11` relative; truncation `h^2 |f'''|/6` with `|f'''| ~ |K|/l^2` (`l ~ 1`):
   `2e-13`. Estimate `<= 1e-9`; limit `1e-6` (the acceptance).
5. **Roll-up estimate** (RU4, D5). In the equilibrium every element is the same flat rectangle rotated, its nodal rotations
   `+-alpha/2` about the local `y` relative to the chord, `w = 0` in the local frame: the MITC4 shear at the tying points
   `w_x + beta = 0`, the bending moment `E I alpha/h` (P1: exact), the membrane strain zero with the chord of length `h`; the
   nodal balance is the tip moment `M` in every element: `alpha = M h/(E I)`, `16 alpha = phi`. The nodes on a circle of radius
   `rho_h = h/(2 sin(alpha/2))` at the exact angles: `|x - x_cf| = (rho_h - rho) 2 sin(phi/2)`, `rho_h/rho - 1 = alpha^2/24 +
   O(alpha^4)`: `e = phi sin(phi/2)/3072`, largest `1.18e-3` at `phi = 4.06` (`tan(phi/2) = -phi/2`). Shell3: the same
   (P1: exact constant-curvature bending; the two triangles of a rectangle share its chord plane).
6. **Cylinder estimate** (CY4): the uniform expansion `x_perp -> lambda x_perp` makes every facet a flat rectangle `lambda`
   times wider; Green hoop strain `(lambda^2 - 1)/2` exact for the bilinear interpolation of a rectangle, `nu = 0`: the
   nominal hoop tension `E t lambda (lambda^2 - 1)/2`; the nodal balance with the follower force of the polygon (ADR 0487
   arithmetic 6): `p = E t (lambda^2 - 1)/(2 R cos(pi/N))`, `N = 96` for the full ring: `+5.4e-4`. The rotations are those of
   the facets (`theta_z` free between the symmetry planes); the local deformational rotations vanish by symmetry; no bending.
7. **Cost** (record P2): an element evaluation has about `1e3` operations on values with a gradient of `6 n` and an upper
   Hessian of `6 n (6 n + 1)/2` entries: about `2e6` flops for a Shell4. The roll-up (`16` elements, `40` steps, about `5`
   iterations) and the cylinder (`24` elements, `20` steps) take seconds in release.

## Checks (limits fixed now)

| Id | Clause | What | Limit | Estimate |
|---|---|---|---|---|
| T1 | a3 | `max |K - K_cd| / max |K|`, 6 configurations x 2 states (D4) | `<= 1e-6` each | `<= 1e-9` (arithmetic 4) |
| T1r | a3 (record) | the rotational block of T1, relative to its own largest entry | recorded | |
| S1 | a1 | `K == K^T` bit for bit at the 12 states of T1 | holds | mirrored upper triangle (D2 step 5) |
| L1 | a1, b3 | `max |K(0) - K_lin| / max |K_lin|` on six single elements: the warped Shell4 and the Shell3 of D4, the flat Shell4 `(0, 0, 0), (1.2, 0.1, 0), (1.0, 0.9, 0), (-0.1, 1.1, 0)`, the Shell4 `AndesOpt`, the Shell3 `AndesOpt` and the Shell3 `Allman` (on the D4 element); `K_lin` the linear global element stiffness of the model path (the assembled `assemble::stiffness` of the one-element model) | `<= 1e-10` each | `1e-14` (arithmetic 2) |
| L1f | a1 | `max |f(0)| / (max |K_lin| L)`, the six elements of L1 | `<= 1e-11` each | `1e-16` |
| L2 | a1 | rigid motion, two rotations (arithmetic 3), the six elements of L1: `max |f| / (max |K_lin| L)` | `<= 1e-11` each | `1e-15` |
| RU4 | a4 | Shell4 strip, dead tip moment to the full circle, 40 steps: tip error `e` (D5) at every step; last load factor | `<= 1e-2` each; `= 1` | `<= 1.2e-3` (arithmetic 5) |
| RU4b | a4 | the same: `|theta_y,tip/(-2 pi lambda) - 1|` every step | `<= 1e-6` | Newton tolerance |
| RU3 | a4 | RU4 and RU4b on the Shell3 strip | `<= 1e-2`; `<= 1e-6` | as RU4 |
| RU4a | a4 (companion) | RU4 and RU4b with the Shell4 ANDES membrane | `<= 1e-2`; `<= 1e-6` | as RU4 |
| RU4f | a4 (companion) | RU4 without the planar holds, `0.9 M` in 36 steps: `e` every step; out-of-plane `max |u_y|/L, |theta_x|, |theta_z|` | `<= 1e-2`; `<= 1e-6` | as RU4; rounding |
| CY4 | a5 | quarter ring of Shell4, follower internal pressure by `solve_with`, 20 steps: `|p/p_thin - 1|` every step; last `lambda` | `<= 1e-2` each; `>= 1.5` | `+5.4e-4` (arithmetic 6) |
| CY3 | a5 | CY4 on the Shell3 strip | `<= 1e-2` each; `>= 1.5` | `<= 2e-3` |
| CY4a | record | CY4 with the ANDES Shell4 membrane: last `p/p_thin` | recorded | `0.53` (D0) |
| X1 | a5 (record) | ADR 0487's ignored `c1_cylinder_follower_internal_pressure_known_miss`, run once unchanged | recorded | passes (= CY4) |
| R1 | a2 | refusals kept: `arclength::trace`, `nl_transient::Integrator::new` and `solve_from` refuse the Shell4 strip with `NotYetSupported { item: "element kind", variant: "Shell4" }`; `solve` refuses a Shell4 block with a `J2` material (key `"nonlinear"`) | holds each | D3 |
| N1 | a2, b5 | the digests of D7 at the base | exact | the solid and linear shell paths execute the same operations |
| N2 | a2, b5 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | no shared output path changed |
| P2 | record | the wall time of the RU4 and CY4 runs; element evaluations per second | recorded | seconds |

Every aggregated error is formed with a NaN-propagating maximum and checked finite (b7): a NaN fails its check. Files:
`crates/rcfd-fem/tests/sa_corot_shell.rs` (T1 to CY4a, R1, P2), `sa_corot_shell_default_bits.rs` (N1). The ignored record of
X1 is in `sa_shell_follower.rs` (ADR 0487). A test that takes more than about two minutes in release becomes an ignored heavy
record with a fast test on the same path (house rule 7); none is expected.

## Planned mutations (on a scratch copy, one at a time)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the tangent without the term `sum_i s_i Hess d_i` | T1 |
| MU2 | the default membranes take the corotated linear membrane (`Pi_m = 0`, `K_r` with `A`) | CY4, CY3 |
| MU3 | `phi = v` (the axial vector without `t/sin t`) | RU4b, RU3 |
| MU4 | `phi_a = log(Lambda R(theta_a))` (the reference frame `Lambda_0` dropped) | L1, L2, RU4 |
| MU5 | the Rodrigues `b` term with the wrong sign | L2, RU4 |
| MU6 | the rigid link ignored (`p_a = X_a + u_a`) | L1 (warped Shell4), L2 |
| MU7 | the Taylor branch used at every rotation | L2 (large rotation), RU4b |
| MU8 | the Hessian mirrored from the lower triangle (left empty) | T1, S1 |
| MU9 | the dispatch gives shells the linear response (`f = K_lin u`, `K = K_lin`) | RU4, CY4 |

## Decision rule

- **done**: every check of the table met, every planned mutation caught by at least one check, fmt and clippy clean, N1 and
  N2 unchanged, X2 as registered.
- **partial**: N1 and N2 unchanged, and T1, S1, L1, L2 and RU4 met, with any other miss diagnosed and recorded as a
  `*_known_miss` test (in particular the cylinder, if it misses).
- **todo**: otherwise (in particular any change of the default bits).

## Limits (not covered)

- Rotations: the additive pseudo-vector is singular at `|theta| = 2 pi k` (D1); a node rotating through a full turn about an
  axis that is not held makes the tangent singular there. No multiplicative update.
- Small strain bending, transverse shear and drilling (the corotated linear element); large membrane strain only in the
  default membranes (Green strain, St. Venant-Kirchhoff, constant thickness). The drilling membranes (ANDES, Allman) are
  corotated linear (D2). No thickness change, no shell plasticity or other material: shells take `IsotropicElastic` only.
- Layups with membrane-bending coupling are refused; a symmetric layup takes the corotated path.
- The arc-length continuation, the nonlinear transient integrator and `solve_from` still refuse shells (R1); a follow-up.
- A dead moment is conjugate to the rotation vector, which is the spatial moment only for a rotation about a fixed axis along
  the moment (D1); no follower moment.
- Cost: dense second-order forward differentiation per element (arithmetic 7); no sparsity exploited.
- No case-file key and no change of the API or CLI crates.

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. **Rotation update.** Options: (a) additive total rotation vector; (b) multiplicative (incremental spin with the triads
   stored). Taken (a), reasons in D1.
2. **ADR 0487's C1r** (X2). Options: (a) the attribute `#[ignore]` with the reason, body unchanged; (b) leave it failing; (c)
   rewrite it. Taken (a): the test states the D0 finding of ADR 0487 that F-578 removes by its acceptance; it stays as a
   record. Reported to the lead for confirmation.
3. **The membrane strain** (D0 point 2). Options: (a) Green-strain membrane for the default membranes, corotated linear for
   the drilling membranes; (b) corotated linear everywhere (the cylinder a predicted miss by `2/(lambda(lambda+1))`). Taken
   (a): the acceptance's law is the SVK ring; (b) would not meet (a5) for any stretch beyond about 1.01.
4. **Dead or follower moment** (b4). Taken the dead `NodalMoment` about the fixed axis, equal to the follower moment for the
   planar roll-up (D1, D5), with the closed form derived in D5.
