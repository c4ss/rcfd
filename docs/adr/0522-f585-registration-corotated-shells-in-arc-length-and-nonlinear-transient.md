# ADR 0522: F-585 registration: corotated shells in the arc-length continuation and the nonlinear transient

Status: **REGISTERED 2026-11-14, written before any code of F-585 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-585 was written in the repository and nothing was run (no probe). The bit record N1
of the default paths is a test file written and run on the base after this commit and before any library code (D7). Results
go in the addenda at the end; nothing above the addenda is changed after a run.

Follow-up of F-578 ([ADR 0490](0490-f578-registration-corotational-shells-newton-driver.md)), proposed in its addendum.
Roadmap item F-585 (deps F-578). Number 0522 given by the lead. Branch `f585-work` at b6d3fb1c, worktree `w49i-work/f585`,
scratch `w49i-work/f585-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code, the crate graph
unchanged; clean-room.

## What F-585 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. arclength::trace, solve_from and the nonlinear transient integrator take Shell4 and Shell3 with the corotated element of ADR 0490 (the transient with the consistent shell mass and the rotary inertia of the rotation vector, in a registered form), every default bit-identical; a shallow cylindrical shell panel under a point load followed through its limit point by arc length within 2 percent of a registered reference; a cantilever strip of shells in free vibration of small amplitude gives the linear first frequency within 1 percent and conserves energy within a registered limit over 100 periods.

Clauses: (a0) every limit registered here; (a1) `arclength::trace`, `solve_from` and `nl_transient::Integrator` take Shell4
and Shell3 with the corotated element of ADR 0490; (a2) the transient with the consistent shell mass and the rotary inertia of
the rotation vector in a registered form; (a3) every default bit-identical; (a4) the shallow cylindrical panel under a point
load through its limit point by arc length, within 2 percent of a registered reference; (a5) the free vibration of a cantilever
strip of shells at small amplitude: the linear first frequency within 1 percent; (a6) energy conserved within a registered
limit over 100 periods.

The brief adds: (b1) follow how ADR 0490 wired shells into the Newton driver; (b2) state how rotation increments are applied and
the form of the rotary inertia; (b3) a registered integrator choice without numerical damping for the energy clause; (b4) the
hinged cylindrical roof (R = 2540, L = 508, theta = 0.1 rad, t = 12.7 or 6.35) with a cited published limit load, or, if no
number can be cited with confidence, an independent registered reference and an open question; (b5) shells opt-in where they
were refused, `baseline_hashes` unchanged, no case-file key (an open question instead); (b6) every aggregated error finite; (b7)
do not edit the files of F-604, F-586 (`damped.rs`), F-606.

## What was read (disclosed)

- `AGENT_RULES.md`; the brief; ADR 0490 (whole, with its amendments and addendum); the module text of `arclength.rs` (ADR 0480)
  and `nl_transient.rs` (ADR 0479, ADR 0514); `nonlinear.rs` (`check_model`, `check_driver_model`, `initial_states`,
  `internal_force_at`, `force_and_tangent`, `solve_with`, `solve_from`, `solve_from_at`, `Problem::attempt_with`,
  `controlled_correction`, `controlled_attempt`); `shell_corot.rs` (whole); `shell.rs` (`local_mass`, `element_mass`,
  `model_element_mass`); `mass.rs` (`consistent_with`); `model.rs` (`check_supported_nonlinear`,
  `check_supported_nonlinear_shells`); `nl_transient/emc.rs` (`check_model`); `modal.rs` (`ModalOptions`, `ModalResult`).
- Tests: `sa_corot_shell.rs` (its `Checks`, `roll_strip`, `grid_conn`, `r1_refusals_kept`), `sa_corot_shell_default_bits.rs`
  (the form of a bit record), `sa_j2_f602.rs` (a mapped Hex20 mesh by `upgrade_to_hex20_mapped`).
- A web search (2026-11-14) for the published limit load of the hinged roof: it found the source of the benchmark (Sze, Liu and
  Lo, Finite Elem. Anal. Des. 40 (2004) 1551, "Popular benchmark problems for geometric nonlinear analysis of shells") and
  open implementations that reproduce its curves, but not the tabulated numbers; the paper itself was not read. Vendor
  documentation that came up in the search was not opened. No number from it is used (D4).

## D0. What exists

1. `arclength::trace`, `nonlinear::solve_from` / `solve_from_at` and `nl_transient::Integrator::new` call
   `model.check_supported_nonlinear()`, which refuses Shell4 and Shell3 (`NotYetSupported { item: "element kind" }`), and then
   `check_driver_model`, which since ADR 0490 lets shells through. Below the refusal every path already runs the corotated
   element: `trace` iterates by `controlled_attempt`, i.e. `Problem::attempt_with` with `force_and_tangent`, which dispatches a
   Shell4 or Shell3 element to `shell_corot`; `solve_from_at` is `Problem::attempt`; `initial_states` gives a shell the empty
   state. So (a1) is, for the statics, the replacement of the refusal by its shell-taking twin
   `check_supported_nonlinear_shells` (the one ADR 0490 used in `solve_with`), and no second loop.
2. The integrator forms the consistent mass by `mass::consistent_with`, which already gives a Shell4 element
   `shell::model_element_mass` and a Shell3 element `shell3::model_element_mass`: `rho t` on the translations and `rho t^3/12`
   on each of the three rotations, the same scalar on the three rotations of a node (`shell::local_mass`), so the rotary block
   of a node pair is `rho t^3/12 int N_a N_b dA` times the identity in any axes.
3. `Integrator::strain_energy` integrates `S . E/2` at the points of `element_for(kind)` with the solid displacement gradient;
   for a shell that is not the energy of the corotated element. `Integrator::momentum` reads the three translations of every
   node; for a shell node it leaves out the rotary part of the angular momentum.
4. The energy-momentum step (ADR 0514) refuses every block that is not a solid (`emc::check_model`); unchanged.

## Design

### D1. Rotation increments (b2)

As ADR 0490 D1: the rotation unknowns of a shell node are the total rotation pseudo-vector, updated additively by the driver
(`u += du`), in the arc-length correction (`du_r + d lambda du_f`) and in the Newmark update of the integrator (`u_{n+1} =
u~ + beta h^2 a_{n+1}`, and the velocity and acceleration of the rotation unknowns are the rates of the rotation vector).
Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation): (i) no second update path in the
driver, the arc length or the integrator; (ii) the internal force stays the gradient of the energy of the stored unknowns and
its tangent symmetric (ADR 0490 D1), which the indefinite `L D L^T` of the arc length and the symmetric factor of the transient
need. Scope: rotations below `2 pi` (the singularity of ADR 0490 D1); the panel turns by about `0.1` rad, the vibrating strip by
about `1.5e-3` rad.

### D2. The inertia of the shell in the transient (a2; registered form)

The kinetic energy of the shell is taken as **`T = (1/2) q_dot^T M q_dot`, `M` the constant consistent shell mass of D0.2** on
the additive unknowns. For the translations of the mid-surface this is exact at any motion (the mass of the mid-surface
interpolation in total Lagrangian form does not depend on the displacement). For the rotations it is the **rotary inertia of
the rotation vector**: the true rotary kinetic energy is `(1/2) rho t^3/12 int |omega|^2 dA` with the angular velocity `omega =
T(theta) theta_dot`, `T(theta) = I + (1 - cos t)/t^2 [theta]_x + (t - sin t)/t^3 [theta]_x^2` (`t = |theta|`), and the
registered form replaces `omega` by `theta_dot` (because the rotary block is isotropic, D0.2, no frame enters). The relative
error of the rotary energy is `O(|theta|)` (the first term `[theta]_x theta_dot/2`), and the rotary energy is a fraction
`O((t/l)^2)` of the kinetic energy, `l` the wavelength (for the strip of D5, `(0.1/12)^2 x 12 = 8e-4` of the rotary share at
most, times `|theta| <= 1.5e-3`: below `1e-6`). With constant `M` the equation of motion is the Lagrange equation `M q_ddot +
dPi/dq = f` of the energy `Pi` of the corotated element, without gyroscopic terms, so `H = T + Pi` is the conserved energy of
the semi-discrete system: the energy of (a6). No change of `mass.rs` or `shell.rs`.

### D3. The integrator choice for the energy clause (b3)

The registered runs use `NlTransientOptions::new(h)`, the default method `Method::average_acceleration()` (Newmark `beta =
1/4`, `gamma = 1/2`, `alpha_m = alpha_f = 0`: the trapezoidal rule, no numerical damping), undamped, the default Newton options
(20 iterations, residual and energy `1e-10`, 5 halvings). For a linear system it conserves `H` exactly and elongates the period
by `omega_h h = 2 atan(omega h/2)` (closed form, arithmetic 3). For the nonlinear energy the per-step error is the error of the
trapezoidal quadrature of `int f_int . du`, `(1/12) Pi'''(du, du, du)`, of relative order `(omega h)^2 (a/l)^2` at the
amplitude `a` (arithmetic 4).

### D4. The code (b1, b5, b7)

- `arclength::trace`, `nonlinear::solve_from_at` (so `solve_from`) and `nl_transient::Integrator::new`: `check_supported_nonlinear`
  replaced by `check_supported_nonlinear_shells` (the same supports check; the element check takes Shell4 and Shell3). For a
  model without shells both functions execute the same checks and return `Ok` on the same models (the element check differs in
  the shell flag only), so a solid model runs the same operations (N1).
- `nl_transient::Integrator::strain_energy`: for a Shell4 or Shell3 element the energy of the corotated element
  (`shell_corot::model_strain_energy`, below); a solid element as before.
- `shell_corot.rs`: `model_strain_energy(mesh, element, coords, material, ue) -> f64`, the energy `Pi = (1/2) d^T K_r d + Pi_m`
  of ADR 0490 D2 step 4 at `ue`, by the same function `evaluate` run with values only (the derivative dimension zero: every
  intermediate value is the same arithmetic as the value part of the force path). The force and tangent path keeps its
  arithmetic (the derivative dimension `6 n`), so the F-578 results keep their bits (N1 holds digests of them).
- `Integrator::momentum` unchanged (the translational momenta; the shell's rotary part is not added, registered as a limit).
- The energy-momentum step keeps refusing shells (R2). `nonlinear::solve_in_time` and `follower::buckling_factors` keep
  refusing shells (R2; not in the acceptance).
- No edit of `mass.rs`, `shell.rs`, `shell3.rs`, `model.rs`, `damped.rs`, the contact or J2 files, the API or CLI crates. No
  case-file key (open question 2).

### D5. The arc length through the limit point of the hinged roof (a4; A1, A2, H0, records)

**Panel** (the hinged cylindrical roof; its geometry the one of the brief, b4): radius `R = 2540`, straight edges of length
`508` along the axis, half-angle `theta_0 = 0.1` rad, thickness `t = 12.7`, `E = 3102.75`, `nu = 0.3` (units mm, N, MPa;
density 1, not used). Straight edges hinged (translations held, rotations free), curved edges free, a point load `P` towards
the axis at the apex. By the double symmetry of the geometry and the load, a **quarter**: angle `s` in `[0, theta_0]`, axial
coordinate `y` in `[0, 254]`, mid-surface `x = R sin s`, `z = R cos s - R` (apex at the origin). Held: on `s = 0` (the plane
`x = 0`) `u_x, theta_y, theta_z`; on `y = 0` `u_y, theta_x, theta_z` (the mirror images of a pseudo-vector); on the hinge
`s = theta_0` `u_x, u_y, u_z`. Load `Load::nodal_force` `(0, 0, -250)` on the apex node (a quarter of the reference load `P_0 =
1000` N; the load factor is `P/P_0`). **Shell4 `n x n`, `n = 16`** (A1), node `(i, j)` at `s = theta_0 i/n`, `y = 254 j/n`.

**Arc length**: `trace` with the load scale `c = |u_lin|^2` (`u_lin` the `solve_static` displacement of the same model, all
equations; equal to the default `|K_0^-1 f|^2` up to the factorisation's rounding since the tangent at rest is the linear
stiffness, ADR 0490 L1) given explicitly, the length `Ds = 0.03 sqrt(2 c)` (in the linear range a point is a load-factor
increment of `0.03`), at most 300 points, 5 halvings, `NewtonOptions::new(1)`. Stop: the first point whose load factor is at most
`0.8 lambda_max`, `lambda_max > 0` the largest load factor of an earlier point (the path has passed its limit point and fallen
by a fifth). **The limit load** `P_lim = P_0 lambda_v`: `lambda_v` the vertex of the parabola through `(w, lambda)` of the point
of the largest load factor and its two neighbours, `w = -u_z` of the apex; if the parabola is not concave or the point is the
first or the last, the largest load factor (recorded).

**The reference (b4): an independent in-house solution, not a published number.** The limit load of this roof is tabulated
in Sze, Liu and Lo (2004) and reproduced in many papers; the number could not be cited with confidence here (memory only, the
paper not read), so it is not used, and the reference is registered as **the limit load of the same quarter panel by Hex20
solids of the total Lagrangian St. Venant-Kirchhoff driver** (ADR 0404; a formulation independent of the shell: 3D, no
rotation unknowns, no corotation), by the same trace, stop and vertex rule. Mesh: the Hex8 grid `n x n x 2` on the parameters
`(s, y, zeta)`, `zeta` in `[-t/2, t/2]` (two layers through the thickness, so that the mid-surface carries a quadratic line of
nodes), upgraded by `upgrade_to_hex20_mapped` with the map `(s, y, zeta) -> ((R + zeta) sin s, y, (R + zeta) cos s - R)`.
Held: `u_x` on `x = 0`, `u_y` on `y = 0`, `u_x, u_y, u_z` of the nodes of the hinge line (`s = theta_0`, `zeta = 0`: the
mid-thickness line, a hinge about the edge); the load `(0, 0, -250)` on the apex node of the mid-surface. **`P_ref` = the Hex20
`16 x 16 x 2` value**; H0 compares it with `8 x 8 x 2`. The mesh rule `Ds = 0.03 sqrt(2 c)` with `c` from the solid's own linear
solve. A Hex20 run over two minutes is a heavy ignored record run once (house rule 7), with the fast test of D5 covering the
same path on `4 x 4 x 2` (no limit, recorded).

Checks: **A1** `|P_lim(Shell4, 16) / P_ref - 1| <= 2e-2`. **A2** the trace stopped by the stop rule (not by the cap), and a
point after the point of `lambda_max` has at least one negative pivot (the tangent past the limit point is indefinite). **H0**
(the reference converged) `|P_lim(Hex20, 8) / P_ref - 1| <= 2e-2`. Records: `P_lim` of Shell4 `8 x 8` (A1c) and of Shell3
`16 x 16` (A3; each rectangle cut along the diagonal from its first to its third corner), the apex deflection at the limit, the
points, halvings and negative pivots, the wall times (P1).

Estimates (arithmetic 5): the shell and the solid differ by the transverse shear and the thickness effects, `O((t/l)^2)` with
`l ~ 250`: `3e-3`; the discretisation error of `P_lim` of each at `n = 16`: a few `1e-3` (second-order elements in the solid,
MITC4 in the shell); so `|A1| <= 1e-2` expected. H0: if the solid converges as `h^2`, the `8` to `16` difference is four
thirds of the error of the `8` mesh; expected `<= 1e-2`. If H0 misses, A1 is not met whatever its value (the reference is then
not shown converged).

### D6. The free vibration of the strip (a5, a6; F1, F1b, F1c, E1, EZ, companions)

Strip of ADR 0490 D5: `L = 12`, `b = 1`, `t = 0.1`, `E = 1.2e6`, `nu = 0`, density `rho = 1`; 16 x 1 Shell4 along `x`, clamped
at `x = 0` (all six unknowns of the two root nodes), nothing else held. `E I = E b t^3/12 = 100`, `rho A = 0.1`.

1. The **linear first frequency** `f_1`: `modal::modal_analysis(model, ModalOptions::new(4))`, the lowest pair (the bending
   about `y`: its stiffness `E I` against `E t b^3/12 = 1e4` in the plane). `omega_1 = 2 pi f_1`, `T_1 = 1/f_1`.
2. The start: `u_0 = a phi_1/phi_1[tip u_z]`, `phi_1` the first mode, `a = 1e-3 L = 0.012` at the tip node `(L, 0, 0)`; `v_0 = 0`.
3. `Integrator::new(model, [], NlTransientOptions::new(h))`, `h = T_1/40`, `start(0, u_0, v_0)`, `run(4000)` (100 periods),
   after each step the tip `u_z` and `H = kinetic_energy + strain_energy`.
4. **The measured frequency**: the zero crossings of the tip `u_z` by linear interpolation between steps, `K` crossings from
   `t_first` to `t_last`, `f_m = (K - 1)/(2 (t_last - t_first))`.

Checks (the 100-period run): **F1** `|f_m / f_1 - 1| <= 1e-2` (the acceptance). **F1b** `|f_m / (f_1 r) - 1| <= 1e-3`, `r = 2
atan(omega_1 h/2)/(omega_1 h)` the trapezoidal period factor. **F1c** (the linear frequency of the mesh against
Euler-Bernoulli) `|f_1 / f_EB - 1| <= 1e-2`, `f_EB = beta_1^2/(2 pi L^2) sqrt(E I/(rho A))`, `beta_1 = 1.875104068711961`.
**E1** `max over the steps |H_n - H_0| / H_0 <= 1e-5`. **EZ** `|strain_energy(u_0) / ((1/2) u_0^T K u_0) - 1| <= 1e-4`, `K` the
linear stiffness `assemble::stiffness` (the energy function against the linear limit). Every value finite.

Companions (fast, regular tests): **F1/E1-4** the same strip and start, 4 periods (160 steps): F1, F1b, E1 with the same limits.
**F1/E1-3** the strip cut into Shell3 (32 triangles, as ADR 0490 RU3), its own `f_1`, 4 periods: F1, F1b, E1 with the same limits.
If the 100-period run takes more than about two minutes in release it is an ignored heavy record, run once (house rule 7);
the companions cover its path.

Estimates (arithmetic 3, 4, 6): F1 `-2.05e-3` (the period factor `r = 0.997945`); F1b `<= 1e-5` (the amplitude shift of the
frequency `O((a/L)^2) = 1e-6`, the interpolation of the crossings `O(h^3)` at an inflection, the Newton tolerance); F1c `<= 4e-3`
(the linear Mindlin element of 16 divisions: `O((beta_1 h/L)^2/4)`; shear and rotary inertia `O((t/L)^2) = 7e-5`); E1 `<= 1e-7`;
EZ `O((a/L)^2) = 1e-6`.

### D7. Other checks

- **SF1** (`solve_from` on shells): the strip of D6 under the tip force `Load::nodal_force` `(0, 0, -0.1042)` on each of the two
  tip nodes (the linear tip deflection `P L^3/(3 E I) = 1.2 = 0.1 L`); `solve_from(model, [], f, 0, initial_states, NewtonOptions::new(1), [])`
  with `f = nodal_load_vector(model, [])` against `nonlinear::solve(model, [], [], NewtonOptions::new(1))`: the displacements
  equal bit for bit (the same attempt from zero to the load factor 1, ADR 0479 D2), the record of `solve` without a halving.
- **R2** (refusals kept): the energy-momentum step (`NlTransientOptions::energy_momentum`) refuses the strip (key
  `"nl_transient"`); `nonlinear::solve_in_time` refuses it (`NotYetSupported`, `Shell4`).
- **N1** (b5): `crates/rcfd-fem/tests/sa_corot_shell_dyn_default_bits.rs`, written after this commit and run on the unchanged
  base b6d3fb1c before any library code; its digests (FNV-1a 64 over the bits) written into the file then. Content, in a fixed
  order: `arclength::trace` of a Hex8 column (3 points: load factors, displacements, residuals, negative pivots);
  `solve_from` of a Hex8 cantilever (displacement, internal force, residuals); the nonlinear transient of a Hex8 cantilever
  under a step load, 10 steps by the default method and 10 by the generalised-alpha `rho_inf = 0.8` (the state bytes, the
  kinetic and strain energies and the momenta after each step); `shell_corot::element_force_and_tangent` of a warped Shell4 and
  a Shell3 at a deformed state (the F-578 element). Only API that exists at b6d3fb1c.
- **N2**: `cargo test -p rcfd-api --release --test baseline_hashes` passes.
- **X1, a registered consequence.** ADR 0490's regular test `sa_corot_shell::r1_refusals_kept` asserts that `trace`,
  `Integrator::new` and `solve_from` refuse the Shell4 strip; F-585 removes those refusals by its acceptance. As ADR 0490
  amendments 3 and 4 (the ADR 0291 pattern): the test receives `#[ignore = "superseded by ADR 0522 (F-585) ..."]`, body
  unchanged, and a copy `r1_refusals_kept_after_f585` in the same file keeps the fourth row (J2 on a Shell4 refused by `solve`)
  and expects the three others not to be `NotYetSupported`. Any other existing test that fails in the crate run because it
  asserts one of these three refusals for a shell model is treated the same way by an amendment written before its corrected
  run. Every other existing test is unchanged.

## Arithmetic done before the code (disclosed; by hand)

1. **Bits (N1).** `check_supported_nonlinear` and `check_supported_nonlinear_shells` call `check_elements(false, s, false, true)`
   with `s = false` / `true`, then `check_supports`; on a model without shell blocks the shell flag is not read on an accepting
   path, so both return `Ok` and nothing downstream changes. `strain_energy` gains a branch on the element kind before the
   solid loop body. `evaluate` with the derivative dimension `m`: every operation as before.
2. **Energy-only evaluation.** With the derivative dimension zero every `Hd` has empty gradient and Hessian; the value parts of
   `add`, `mul`, `chain`, `axpy` are the same expressions as with derivatives (the value is never formed from the
   derivatives), so the energy is the energy of the state the force path differentiates. `Pi = (1/2) sum_i s_i d_i + Pi_m`,
   `s = K_r d`, `Pi_m = sum_g (1/2) w_g eps_g . A eps_g`.
3. **Trapezoidal period.** For `x'' + omega^2 x = 0` the average-acceleration step has the amplification of modulus 1 and the
   phase `2 atan(omega h/2)` per step, so `f_num/f = 2 atan(omega h/2)/(omega h)`; at `omega h = 2 pi/40 = 0.15708`:
   `atan(0.07854) = 0.078379`, `r = 0.997945`, `-2.055e-3`.
4. **Energy error.** Per step `Delta H = du . (f_n + f_{n+1})/2 - Delta Pi = -(1/12) Pi'''(du, du, du) + O(du^5)`; the
   geometric nonlinearity of the cantilever is of relative order `(a/L)^2 = 1e-6` (the shortening of the axis), `du ~ omega h a`:
   relative per step `(omega h)^2/12 x 1e-6 = 2e-9`, oscillating with the motion (the rule is symmetric in time). The Newton
   residual `1e-10` of the reference force `~ K a` gives `1e-10 x omega h ~ 2e-11` of `H` per step; over 4000 steps, even
   systematic, `1e-7`. Rounding of `H` (positions relative to the element, ADR 0490 amendment 2): `~eps L/a = 2e-13`. E1
   estimate `<= 1e-7`; limit `1e-5`.
5. **Roof estimates.** As in D5. The rise of the panel is `R (1 - cos 0.1) = 12.69`; the limit point is reached at an apex
   deflection of the order of the rise; the rotations stay of the order `0.1`, the strains small: the corotated element is in its
   scope (ADR 0490 limits).
6. **Euler-Bernoulli** (F1c): `sqrt(E I/(rho A L^4)) = sqrt(100/(0.1 x 20736)) = 0.21960`, `omega_EB = 1.875104^2 x 0.21960 =
   0.77212` rad/s, `f_EB = 0.122886` Hz, `T = 8.138` s; `h = 0.2034` s. The linear Timoshenko element with assumed (MITC)
   shear and consistent mass errs by `O((beta_1 h_e/L)^2)` with `beta_1 h_e/L = 0.117`: `(0.117)^2/4 = 3.4e-3` at most.
7. **Cost** (record P1): ADR 0490 P2: an assembly of the 16-element strip with its solve about `6` ms; 4000 steps x 2 to 3
   iterations plus one energy evaluation per step: about `60` to `100` s for the 100-period run. The Hex20 `16 x 16 x 2` quarter
   (512 elements, about `9e3` unknowns) at about 80 points of 4 to 5 iterations: several minutes, a heavy record.

## Checks (limits fixed now)

| Id | Clause | What | Limit | Estimate |
|---|---|---|---|---|
| A1 | a4 | `|P_lim(Shell4 16 x 16)/P_ref - 1|`, `P_ref` the Hex20 `16 x 16 x 2` limit load (D5) | `<= 2e-2` | `<= 1e-2` |
| A2 | a4 | the roof trace stopped by its rule past the limit point; a negative pivot at a point after `lambda_max` | holds | |
| H0 | a4 | the reference converged: `|P_lim(Hex20 8 x 8 x 2)/P_ref - 1|` | `<= 2e-2` | `<= 1e-2` |
| A1c, A3 | a4 (record) | `P_lim` of Shell4 `8 x 8` and of Shell3 `16 x 16`, with their ratios to `P_ref` | recorded | |
| F1 | a5 | 100 periods: `|f_m/f_1 - 1|` | `<= 1e-2` | `2.05e-3` |
| F1b | a5 | 100 periods: `|f_m/(f_1 r) - 1|` | `<= 1e-3` | `<= 1e-5` |
| F1c | a5 | `|f_1/f_EB - 1|` | `<= 1e-2` | `<= 4e-3` |
| E1 | a6 | 100 periods: `max |H_n - H_0|/H_0` | `<= 1e-5` | `<= 1e-7` |
| EZ | a2 | `|Pi(u_0)/((1/2) u_0^T K u_0) - 1|` | `<= 1e-4` | `1e-6` |
| F1/E1-4 | a5, a6 (companion) | Shell4, 4 periods: F1, F1b, E1 | as F1, F1b, E1 | as above |
| F1/E1-3 | a1, a5, a6 (companion) | Shell3 strip, 4 periods: F1, F1b, E1 | as F1, F1b, E1 | as above |
| SF1 | a1 | `solve_from` against `solve` on the strip under a tip force: displacements equal bit for bit | holds | same attempt |
| R2 | a1 | the energy-momentum step and `solve_in_time` still refuse the strip | holds each | D4 |
| N1 | a3 | the digests of D7 at the base | exact | arithmetic 1 |
| N2 | a3 | `baseline_hashes` of `rcfd-api` | pass | |
| X1 | a1 | `r1_refusals_kept` superseded as registered; `r1_refusals_kept_after_f585` | holds | |
| P1 | record | wall times of the runs | recorded | arithmetic 7 |

Every aggregated error is formed with a NaN-propagating maximum and checked finite (b6): a NaN fails its check. Files:
`crates/rcfd-fem/tests/sa_corot_shell_dyn.rs` (A1 to R2, P1), `sa_corot_shell_dyn_default_bits.rs` (N1). The fast tests run on
every crate run; the 100-period run and the Hex20 reference are heavy ignored records if they take more than about two minutes,
each run once by this work, as the registered runs.

## Planned mutations (on a scratch copy, one at a time)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | `model_strain_energy` with the factor `1` instead of `1/2` on `d^T K_r d` | EZ, E1 |
| MU2 | `strain_energy` of a shell evaluated at zero displacement | EZ, E1 |
| MU3 | `Integrator::new` keeps `check_supported_nonlinear` | F1/E1 companions, X1 copy (refusal) |
| MU4 | `trace` keeps `check_supported_nonlinear` | A1, A2 (fast roof test), X1 copy |
| MU5 | the integrator's mass scaled by `1.01` (`Integrator::new`) | F1b (`-5e-3`) |
| MU6 | the arc-length root choice reversed (the root of the smaller score) | A2 (fast roof test) |
| MU7 | the energy-only path takes the membrane energy out | EZ? (the membrane share at small amplitude is `O((a/L)^2)`: planned as a probe of sensitivity, not expected to be caught) |

## Decision rule

- **done**: every check of the table met (A1, A2, H0, F1, F1b, F1c, E1, EZ, the companions, SF1, R2, X1), N1 and N2 unchanged,
  every planned mutation caught except MU7 (registered as not expected), fmt and clippy clean.
- **partial**: N1 and N2 unchanged and SF1 met, and either the roof (A1, A2, H0) or the vibration (F1, E1) met, with every miss
  diagnosed and recorded as a `*_known_miss` test; also if the reference fails to converge (H0 missed), whatever A1 gives.
- **todo**: otherwise (in particular any change of the default bits).

## Limits (not covered)

- The registered reference of the roof is in-house (an independent solid formulation of the same repository), not the
  published number (open question 1).
- Rotations: additive pseudo-vector (ADR 0490 D1); the rotary inertia replaces the angular velocity by the rate of the rotation
  vector (D2): exact at small rotation, no gyroscopic terms; valid for rotations of shells that stay small (fan blades vibrate
  at small rotation). `Integrator::momentum` gives the translational momenta only (the shell's rotary angular momentum is not
  added).
- The energy-momentum step, `solve_in_time` and `follower::buckling_factors` still refuse shells.
- No case-file key, no change of the API or CLI crates.

## Open questions

1. **The published limit load** (b4). Options: (a) keep the in-house Hex20 reference and add the published value as a second
   check once the maintainer supplies the table of Sze, Liu and Lo (2004) for `t = 12.7`; (b) take a number from memory now.
   Recommendation (a): a number from memory is not a registered reference. Left open (needs a paper the repository does not
   have).
2. **A case-file key** for shells in the arc length and the transient. Options: (a) none now, the library only; (b) a key in a
   follow-up with the CLI. Recommendation (b) as a follow-up; left open (it would change a golden of the schema).
3. **The rotary inertia at large rotation**. Options: (a) the registered constant-mass form (D2); (b) a follow-up with `omega =
   T(theta) theta_dot` and its gyroscopic terms. Taken (a) on the maintainer's standing instruction of 2026-11-11 (take the
   recommendation): the acceptance asks for a registered form and a small-amplitude vibration.

## Amendment 1 (2026-11-14): the test layout; written after the code compiled, before any run of a registered check

No limit and no parameter changes.

1. The registered roof run (`a1_a2_h0_hinged_roof_through_the_limit_point`: Shell4 `16 x 16`, Hex20 `16 x 16 x 2` and
   `8 x 8 x 2`, the records Shell4 `8 x 8` and Shell3 `16 x 16`) and the registered 100-period run
   (`f1_e1_strip_free_vibration_100_periods`) are ignored heavy records from the start (their cost estimates, arithmetic 7, are
   at or beyond two minutes on the shared machine), each run once by this work with `--ignored`.
2. The fast roof test named in the mutation table (`a2_fast_roof_shell4_and_hex20`) is the trace of D5 on the Shell4 `8 x 8`
   quarter (the A1c mesh) and the Hex20 `4 x 4 x 2` quarter with the check A2 on each (the same rule and limits), their limit
   loads recorded. A2 is applied in the registered run to every one of its five traces.
3. The companion F1/E1-4 also checks F1c and EZ on its strip (the same limits); F1/E1-3 checks EZ on the Shell3 strip.
4. In the roof run every trace prints its points `(w, P)`, so the path through the limit point is on record.

## Amendment 2 (2026-11-14): three set-up defects found by run 1 of the fast tests; written before the corrected runs

Run 1 (the regular tests of `sa_corot_shell_dyn` once at 0273f25f, `--release -- --nocapture --test-threads 1`,
`f585-scratch/run1-fast.log`): A2 met on the fast roof (Shell4 `8 x 8` and Hex20 `4 x 4 x 2`) and R2 met; on the Shell3
strip (F1/E1-3) F1 `2.05e-3` and F1b `9.9e-7` met, **E1 `4.71e-4` and EZ `2.52e-2` missed**; F1/E1-4 did not reach a
measurement (the modal solve: the subspace iteration not converged in 200 iterations, residual `1.06e-9` of the first pair
against the tolerance `1e-10`); SF1 did not reach a measurement (`solve_from`: a negative pivot, `-3.4` against the diagonal
`6.4e3` at node 21 component 4, in its single attempt). The 100-period run and the roof run had not been started.

**Diagnoses.**

1. *EZ and E1, the start state (a defect of the set-up and of arithmetic 4).* The start `u_0 = a phi_1` is the linear mode,
   whose axial displacement is zero, so the Green membrane strain `w_x^2/2` of the deflected strip is not relaxed: the start
   carries a membrane energy of relative order `(a/t)^2` (here `0.12^2` times a shape factor), not `(a/L)^2` as arithmetic 4
   assumed, which left out the factor `(L/t)^2` of a thin strip (the parameter of the membrane-bending coupling is the
   amplitude over the thickness). EZ measures exactly this energy (`2.5e-2`). Released at `t = 0`, it drives the stiff axial
   modes (`omega h ~ 30`), whose nonlinear coupling with the bending the trapezoidal rule integrates with an energy error of
   `4.7e-4` (E1). The energy function itself is the potential of the element force: a probe after run 1 (a scratch test, not
   kept) gave `|dPi/dq - f| <= 8.5e-10` of `max |f| = 72` by central differences on a Shell4 and a Shell3 element.
2. *F1/E1-4, the modal solve (a defect of the set-up).* `ModalOptions::new(4)` with its default cap of 200 iterations did not
   converge the four pairs on the Shell4 strip; F1 needs the first pair only.
3. *SF1, the load (a defect of the set-up).* The tip load of the linear deflection `0.1 L` is too large for one Newton attempt
   from zero: the first iterate (the linear solution, with the unrelaxed membrane strain of point 1) has an indefinite tangent.
   SF1 compares the attempt of `solve` with that of `solve_from`; it needs a load that one attempt reaches.

**Corrections** (no limit changes):

1. The strip starts from **`u_0 = 0`, `v_0 = omega_1 a phi_1/phi_1[tip u_z]`** (the same mode and amplitude, the tip at `a
   sin(omega_1 t)`): no strain energy at the start, and the axial unknowns follow the slow bending quasi-statically from rest
   (the shortening is of second order in the deflection, its rate zero at `t = 0`). The crossings are counted as registered
   (the start, a zero, counts as the first crossing). Estimate of E1 as registered (`<= 1e-7`; the membrane strain relaxed,
   the remaining nonlinearity `(a/L)^2`).
2. **EZ** is evaluated at an equilibrium, where the membrane strain is relaxed: the strip under the tip force `(0, 0,
   -1.042e-3)` on each tip node (the linear tip deflection `1e-3 L`) solved by `nonlinear::solve` (one step), `u_s` its
   displacement, `Pi(u_s)` by `Integrator::strain_energy` after `start(0, u_s, 0)`; `|Pi(u_s)/((1/2) u_s^T K u_s) - 1| <=
   1e-4` (limit unchanged; estimate `(a/L)^2 = 1e-6`). For Shell4 in F1/E1-4 and the 100-period run, Shell3 in F1/E1-3.
3. The modal solve takes `ModalOptions::new(1)` with `max_iterations = 2000`.
4. **SF1** takes the tip force `(0, 0, -1.042e-3)` on each tip node (the load of point 2).

The three tests of run 1 become ignored records with their bodies unchanged and the suffix `_known_miss`
(`f1_e1_3_strip_shell3_four_periods_known_miss`, `f1_e1_4_strip_shell4_four_periods_known_miss`,
`sf1_solve_from_equals_solve_on_shells_known_miss`); the 100-period test, not run, is ignored as superseded before its run
(`f1_e1_strip_free_vibration_100_periods`, body unchanged). The corrected tests `f1_e1_3_corrected`, `f1_e1_4_corrected`,
`sf1_corrected` and the heavy `f1_e1_100_periods_corrected` are run once (run 2; the heavy one with `--ignored`). Mutations
MU1, MU2 and MU5 are aimed at the corrected tests.

## Amendment 3 (2026-11-14): the solid reference under a point load does not converge; written before the corrected run

The registered roof run (`a1_a2_h0_hinged_roof_through_the_limit_point`, once at 7884d878, `f585-scratch/run1-roof.log`): A2
met on all five traces; H0 met (`7.85e-3`); **A1 missed: `|P_lim(Shell4 16)/P_ref - 1| = 2.53e-2`** (limit `2e-2`), with
`P_ref = 2167.5` N (Hex20 `16 x 16 x 2`), Shell4 `16 x 16` `2222.4` N, Shell4 `8 x 8` `2223.6` N, Shell3 `16 x 16`
`2221.3` N; the fast test of run 1 gave Hex20 `4 x 4 x 2` `2213.9` N. The test stays as it is, an ignored record; it is
renamed `a1_a2_h0_hinged_roof_through_the_limit_point_known_miss` with its body unchanged.

**Diagnosis (a defect of the registered reference, not of the element).** The Hex20 limit load falls with every refinement,
`2213.9, 2184.5, 2167.5` N at `n = 4, 8, 16`: differences `29.4` and `17.0`, a ratio `1.7`, not the `4` of a second-order
convergence that D5 assumed. A force at one node of a three-dimensional continuum has no mesh limit (the displacement under a
point force on an elastic solid is unbounded, the Boussinesq singularity), so the solid's local indentation under the load grows
as the mesh is refined, and with it the softening of the panel; the shell has a finite deflection under a point load (its
singularity is only in the moments), and its limit load is converged: Shell4 `8` to `16` `5.4e-4`, Shell4 and Shell3 at `16`
within `5.0e-4`. H0 compared two meshes only and could not see the divergence. The point-load solid is therefore not a
reference of the shell point-load limit; the registered A1 stays a miss.

**Corrected comparison (A1p, H0p; no limit changes).** Both models carry the **same distributed load**, for which the solid
has a mesh limit: a uniform dead pressure over the patch `s in [0, theta_0/8]`, `y in [0, 254/8]` of the quarter (the element
of the `8 x 8` mesh at the apex; a union of elements for `n = 8, 16`), on the shell elements of the patch (`Load::shell_pressure`)
and on the outer faces of the solid elements of the patch (`Load::pressure` on the faces of the `zmax` set of the parameter
box whose element centroids lie in the patch), the pressure scaled so that the consistent nodal forces of the patch sum to
`250` N towards the axis (the value of the load factor 1 as in D5). The same trace, stop and vertex rule as D5. **A1p**:
`|P_lim(Shell4 16, patch)/P_ref,p - 1| <= 2e-2`, `P_ref,p` the Hex20 `16 x 16 x 2` patch value; **H0p**: `|P_lim(Hex20 8,
patch)/P_ref,p - 1| <= 2e-2`; A2 on every trace; records: the Shell4 `8 x 8` patch value (the patch is a union of elements for `n` a multiple of 8 only, so the
coarsest solid is `8`). Estimate: the patch (`31.75 x 31.75` of the quarter, `2.5 t` across) removes the singularity; the solid then converges
at second order, its residual difference from the shell `O((t/l)^2)` with `l = 63.5` the full patch width: `0.04` at most if taken
literally, about `1e-2` expected as the transverse shear and the thickness stresses are confined to a region of size `t` at the
patch edge. Heavy (`a1p_h0p_roof_patch_load`, ignored), run once.

This does not replace the acceptance clause, which asks for a point load: under the point load the shell limit load is recorded
(and converged in the shell sequence), and A1 is a miss against the registered reference; the decision follows the rule.

## Amendment 4 (2026-11-14): two more set-up defects found by run 2 of the strip; written before the corrected runs

Run 2 (the corrected regular tests once at 1f5943e9, `f585-scratch/run2-fast.log`): SF1 met on Shell4 and Shell3 (bit equal,
3 iterations, tip `-1.1993e-2` and `-1.2000e-2`); on the Shell3 strip F1 `2.051e-3`, F1b `2.53e-6` and **E1 `2.77e-8` met**
(the corrected start removed the energy error of run 1, as diagnosed), **EZ `2.87e-2` missed again**; the Shell4 strip did
not reach a measurement: the modal solve with 2000 iterations stalled at the residual `8.4e-10` of the first pair (tolerance
`1e-10`).

**Diagnoses.** (1) *EZ, the comparator.* At the static equilibrium the membrane strain is relaxed by the axial shortening
`u_x ~ -int w_x^2/2`, which the linear quadratic form `(1/2) u^T K u` reads as an axial strain `u_x,x = -w_x^2/2`: it adds
the membrane energy `(1/2) E t int (w_x^2/2)^2`, of relative order `(a/t)^2`, that the nonlinear energy does not have
(`Pi = 1.2505e-5` against `1.2875e-5`). The quadratic form is not the linear limit at this amplitude, for the same reason as
amendment 2 point 1. (2) *The modal solve of the Shell4 strip* has a residual floor near `8e-10`, above the default tolerance;
a residual `r` of the pair changes the eigenvalue by `O(r^2)` relative, far below every frequency limit here.

**Corrections** (no limit changes): (1) **EZ** compares the energy at the equilibrium with Clapeyron's work of the dead tip
force, `|Pi(u_s)/((1/2) f . u_s) - 1| <= 1e-4` (for a linear system equal to 1; for the strip the deviation is of the order
of the geometric nonlinearity of the deflection `(a/L)^2 = 1e-6` and of the membrane share at equilibrium, relaxed). (2) The
modal solve takes `ModalOptions::new(1)` with `max_iterations = 2000` and `tolerance = 1e-8`, on Shell4 and Shell3.

The tests of run 2 that missed or did not reach a measurement become ignored records, bodies unchanged:
`f1_e1_3_corrected_known_miss`, `f1_e1_4_corrected_known_miss`; the 100-period test of amendment 2, not run, is ignored as
superseded before its run. The tests `f1_e1_3_corrected2`, `f1_e1_4_corrected2` and the heavy `f1_e1_100_periods_corrected2`
are run once (run 3). SF1 (`sf1_corrected`) stands as run in run 2.

## Addendum 1 (2026-11-14): results

Runs (release, `--nocapture --test-threads 1`, logs in `f585-scratch`): N1 recorded at the base b6d3fb1c (`n1-base.log`)
before any library code and passing at every later commit (`x1-n1.log`, the crate run); run 1 of the fast tests at 0273f25f
(`run1-fast.log`); the registered roof run at 7884d878 (`run1-roof.log`); after amendment 2, run 2 at 1f5943e9
(`run2-fast.log`); after amendment 4, run 3 of the companions at d401b3a8 (`run3-fast.log`); the patch-load roof run of
amendment 3 and the 100-period run of amendment 4, once each, at d401b3a8 (`run3-heavy.log`); X1 (`x1-n1.log`); the whole
`rcfd-fem` suite with `--no-fail-fast` (`fem-suite.log`: 863 passed, 0 failed, 247 ignored); `baseline_hashes` of `rcfd-api` and `docs_site` of `rcfd-cli`
(`api-cli.log`). No limit was changed after a run; four amendments corrected set-ups before their corrected runs, each
superseded test kept as an ignored record with its body unchanged.

| Id | Limit | Measured | Met |
|---|---|---|---|
| A1 roof, point load, Shell4 `16` against Hex20 `16 x 16 x 2` | `<= 2e-2` | `2.53e-2` (`2222.4` N against `P_ref = 2167.5` N) | **missed**; record `a1_a2_h0_..._known_miss` |
| A2 roof, 5 traces of the registered run, 4 of the patch run, 2 of the fast test | holds | holds on all: stopped by the rule after 205 to 211 points, no halving, a negative pivot after the limit on each | yes |
| H0 Hex20 `8` against `16` | `<= 2e-2` | `7.85e-3` | yes (but see the diagnosis) |
| A1c Shell4 `8`, A3 Shell3 `16` (records) | recorded | `2223.6` N (`1.0259 P_ref`), `2221.3` N (`1.0248 P_ref`) | recorded |
| A1p roof, patch load (amendment 3) | `<= 2e-2` | `2.61e-2` (`2243.5` N against `2186.4` N) | **missed**; record `a1p_h0p_roof_patch_load_known_miss` |
| H0p Hex20 `8` against `16`, patch | `<= 2e-2` | `7.89e-3` | yes |
| fast roof (records) | recorded | Shell4 `8` `2223.6` N, Hex20 `4 x 4 x 2` `2213.9` N | recorded |
| F1 100 periods (amendment 4) | `<= 1e-2` | `2.049e-3` (`f_1 = 0.122940` Hz, `f_m = 0.122689` Hz, 200 crossings) | yes |
| F1b 100 periods | `<= 1e-3` | `7.97e-8` | yes |
| F1c `f_1/f_EB` | `<= 1e-2` | `4.29e-4` | yes |
| E1 100 periods | `<= 1e-5` | `2.82e-8` | yes |
| EZ (amendment 4, Clapeyron) | `<= 1e-4` | `5.13e-7` | yes |
| F1/E1-4 (amendment 4) | as F1, F1b, E1; F1c, EZ | `2.05e-3`, `2.53e-6`, `2.77e-8`; `4.29e-4`, `5.13e-7` | yes |
| F1/E1-3 Shell3 (amendment 4) | as F1, F1b, E1; EZ | `2.05e-3`, `2.53e-6`, `2.77e-8`; `5.14e-7` | yes |
| run 1: F1/E1-3 E1, EZ | `<= 1e-5`, `<= 1e-4` | `4.71e-4`, `2.52e-2` | missed, set-up (amendment 2; record) |
| run 2: F1/E1-3 EZ | `<= 1e-4` | `2.87e-2` | missed, set-up (amendment 4; record) |
| run 1 and 2: F1/E1-4; run 1: SF1 | | no measurement (modal floor; one Newton attempt) | records |
| SF1 (amendment 2) | holds | Shell4 and Shell3 bit equal, one step, no halving, 3 iterations | yes |
| R2 | holds | the energy-momentum step (key `"nl_transient"`) and `solve_in_time` (`NotYetSupported` Shell4) refuse | yes |
| X1 | holds | `r1_refusals_kept` ignored, body unchanged; `r1_refusals_kept_after_f585` passes (three paths take the strip, J2 refused) | yes |
| N1 | exact | the 6 digests equal | yes |
| N2 `baseline_hashes` | pass | 3 passed, 1 ignored (`docs_site` of `rcfd-cli`: 2 passed) | yes |
| P1 wall times | recorded | Shell4 `16` roof `19` s, Hex20 `16` `232` s (`312` s patch), Hex20 `8` `26` s, Shell3 `16` `13` s; 100 periods `63` s (11988 iterations, 3 per step) | recorded |

### Diagnosis of the roof miss

The shell is converged: Shell4 `8` and `16` within `5.4e-4`, Shell3 `16` within `5.0e-4` of Shell4 `16` (point load), and
the corotated element meets every other check. The registered reference is not: the Hex20 limit load falls with each
refinement, `2213.9, 2184.5, 2167.5` N at `n = 4, 8, 16` (point load), differences `29.4` and `17.0`, a ratio `1.7` where a
converging second-order solution gives `4`; the patch load of amendment 3 shows the same fall (`2203.6, 2186.4`, difference
`17.3`), so the point-load singularity that amendment 3 named is **not** the cause, and that diagnosis was wrong. The remaining
singular feature of the solid model, common to both loads, is the hinge: the mid-thickness line of nodes pinned in a
three-dimensional continuum is a line support, under which the displacement of an elastic solid is unbounded (logarithmic), so
the solid's local compliance at the hinge grows with every refinement and the panel softens with it. H0 compared two meshes
and could not detect a slowly divergent sequence; the registration's assumption of second-order convergence (D5) was wrong.
With the observed ratio the solid at `n = infinity` would lie further below the shell. The roof clause therefore lacks a valid
reference: it is not shown that the shell is wrong, nor that it is within 2 percent of the physical limit load. A valid
reference needs the published value (open question 1) or a solid hinge without a singularity (for instance a rigid hinge
plate, which the nonlinear driver does not offer: the transformation path is refused). Not verified here: the hinge hypothesis
itself (no run was made to test it).

### Mutations (scratch copy of d401b3a8 by `git archive`, one at a time, `f585-scratch/mutate.py`, `mutations.log`, `mut-MU*.log`)

| Id | Caught by |
|---|---|
| MU1 bending energy with the factor 1 | E1 (`1.0`), EZ (`1.0`) on Shell4 and Shell3 |
| MU2 shell energy at zero displacement | E1, EZ (`1.0`) |
| MU3 `Integrator::new` refuses shells | X1 copy (the run stops at the first failing binary) |
| MU4 `trace` refuses shells | X1 copy (as MU3) |
| MU5 mass times `1.01` | F1b `4.94e-3` (F1 not, as planned) |
| MU6 arc-length root reversed | the fast roof test (the trace fails) |
| MU7 membrane energy left out | not caught (as registered: not expected) |

Six of the six mutations expected to be caught were caught; MU7 was not, as registered.

### Deviations

- Four amendments corrected set-ups before their corrected runs: the start of the strip from the unrelaxed linear mode
  (amendment 2; the arithmetic missed the factor `(L/t)^2`), the modal options (amendments 2 and 4), the SF1 load (amendment 2),
  the EZ comparator (amendment 4) and the roof load (amendment 3, whose diagnosis turned out wrong). Every superseded test is an
  ignored record with its body unchanged.
- One existing test was superseded by design (X1, `sa_corot_shell::r1_refusals_kept`), with an `_after_f585` copy; it is an
  edit of an existing test file and needs the lead's confirmation.
- After the runs, two edits without effect on any result: a doc paragraph of `arclength.rs` (a clippy `doc_lazy_continuation`)
  and `!(a < 0.0)` written as `a.is_nan() || a >= 0.0` in the vertex rule of the test (clippy); the regular tests and N1 rerun
  as a regression (`regression.log`, all pass). fmt and clippy (`-D warnings`, all targets) clean.
- `rcfd-api` and `rcfd-cli` were not edited. The roadmap files are updated in the last commit as the brief asks.

### Decision

**partial** by the registered rule: N1 and N2 unchanged, SF1 met, the vibration met (F1 `2.05e-3`, E1 `2.8e-8` over 100
periods); the roof missed A1 (`2.53e-2`) and, after amendment 3, A1p (`2.61e-2`), recorded as known misses with the diagnosis
above. Clauses: (a1) met (the three paths take Shell4 and Shell3: SF1, X1, the transient on both elements, the arc length on
both); (a2) met (the constant consistent shell mass with the rotary inertia of the rotation-vector rate, D2); (a3) met (N1,
N2); (a4) not met (the path is followed through the limit point, A2, but not shown within 2 percent of a valid reference);
(a5) met; (a6) met.

### Remaining questions (recommendations for the lead)

- **Q1** (the roof reference): options (a) obtain the tabulated limit load of Sze, Liu and Lo (2004) for `t = 12.7` and
  register it in a follow-up; (b) a solid reference with a non-singular hinge (needs constraints in the nonlinear driver);
  (c) accept the converged shell sequence as the reference. Recommendation (a): it is the independent reference the acceptance
  intends; (c) is not independent.
- **Q2** (X1): accept the ignored record and its copy (the ADR 0291 pattern). Recommendation: accept.
- **Q3** (a case-file key): a follow-up with the CLI; left open (schema golden).
- **Q4** (the rotary inertia at large rotation): keep the registered form; a follow-up with `omega = T(theta) theta_dot` only
  if large-rotation dynamics of shells are needed.

## For the lead: roadmap entries

Proposed status of F-585: **partial**. Note:

> Structural solver, advanced tier. Follow-up of F-578 (ADR 0490). PARTIAL. ADR 0522, 2026-11-14: arclength::trace, solve_from and the nonlinear transient integrator take Shell4 and Shell3 with the corotated element of ADR 0490 (the refusal replaced by the shell-taking check; additive rotation-vector increments; the constant consistent shell mass with the rotary inertia rho t^3/12 of the rotation-vector rate, isotropic, exact at small rotation; the shell strain energy from the corotated potential); the energy-momentum step, solve_in_time and buckling_factors still refuse shells. Free vibration of a 16-element Shell4 cantilever strip at a tip amplitude 1e-3 L over 100 periods by the trapezoidal rule: frequency within 2.05e-3 of the linear modal frequency (limit 1 percent; the trapezoidal period factor accounts for it within 8e-8), energy conserved within 2.8e-8 (limit 1e-5); the same on Shell3 over 4 periods; solve_from equals solve bit for bit on shells. Hinged cylindrical roof (R 2540, edges 508, 0.1 rad, t 12.7) under a central point load traced through its limit point by arc length (Shell4 16 x 16 quarter, limit load 2222 N, converged to 5e-4 against 8 x 8 and Shell3), but 2.5 percent above the registered Hex20 solid reference (limit 2 percent; 2.6 percent under a patch load): the solid sequence does not converge (probably its mid-line hinge, a singular line support), so the roof clause lacks a valid reference; recorded as known misses. Default bits unchanged (6 digests), baseline hashes unchanged; six of six expected mutations caught. Four set-up amendments, records kept.

Follow-up (title, deps, acceptance), for the lead to number or drop:

> (new item, number for the lead) Hinged cylindrical roof limit load against a published reference (deps F-585). Every limit not stated here is registered in the ADR before the run. The tabulated limit load of the hinged cylindrical roof of Sze, Liu and Lo (2004) for t = 12.7 recorded in docs/reference-data.md from the paper; the Shell4 and Shell3 quarter models of ADR 0522 traced through the limit point by arc length within 2 percent of it; the Hex20 sequence of ADR 0522 kept as a record of the singular hinge.
