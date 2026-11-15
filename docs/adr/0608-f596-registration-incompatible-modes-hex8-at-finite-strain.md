# ADR 0608: F-596 registration: the incompatible-modes Hex8 at finite strain (an enhanced deformation gradient, the internal modes condensed in the Newton loop)

Status: **REGISTERED 2026-11-15, written before any code of F-596 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-596 was written, compiled or run in the repository: the files listed
under "What was read" were read, and the arithmetic below was done by hand and evaluated with plain numpy prototypes in the
scratch folder (`w49i-work/f596-scratch`: `probe_eas.py` (one distorted element: the small-strain limit, the modes against the
linear modes, central differences of the condensed and of the extended element, the tangent and local-iteration mutations; log
`probe_eas.log`), `probe_cant.py` (the elastica cantilever on Hex8 meshes, the element below or plain Hex8; logs
`probe_cant_50.log`, `probe_cant_more.log`) and `probe_block.py` (the Newton order on a sheared block, with the tangent mutation;
log `probe_block.log`); no rcfd code; disclosed, their numbers quoted below). The bit record B0 of the default paths is a test
file written and run on the base after this commit and before any library code (D7). Results go in the addenda at the end;
nothing above the addenda is changed after a run.

Roadmap item F-596 (deps F-587, F-390), the follow-up of [ADR 0506](0506-f587-registration-b-bar-and-f-bar-hex8-in-the-nonlinear-driver.md)
(F-587, D2: the nonlinear driver refuses the `Incompatible` formulation naming `hex8x::NONLINEAR_FOLLOW_UP`). The element is the
incompatible-modes Hex8 of [ADR 0493](0493-f390-registration-hex8-b-bar-and-incompatible-modes.md) (F-390, arithmetic 2). Number
0608 given by the lead. Branch `f596-work` from the local main tip c64046db, worktree `w49i-work/f596`, scratch
`w49i-work/f596-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited
except the records-and-copies scheme of ADR 0291 used by ADR 0493 and ADR 0506 (D6); no new dependency, no unsafe code, the crate
graph unchanged; clean-room. Every choice below that the maintainer would normally be asked about is taken on the maintainer's
standing instruction of 2026-11-11 (take the recommendation), except the open questions at the end.

## What F-596 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. The incompatible-modes Hex8 of F-390 at finite strain, as an enhanced assumed strain element with the internal modes condensed in the Newton loop; the nonlinear driver no longer refuses it. Further limits are registered by the implementing ADR (follow-up text of ADR 0506 gives no numbers).

The brief adds suggested checks (their numbers registered here): (b1) the small-strain limit equals the linear incompatible
Hex8 to rounding; (b2) a patch test at finite strain (homogeneous deformation reproduced, internal modes zero); (b3) a
bending-dominated cantilever at large deflection against a fine Hex20 or reference solution, with fewer elements than plain Hex8
needs; (b4) Newton quadratic convergence (the reading Q of ADR 0515/0519 where it applies); (b5) the condensation consistent (a
central-difference tangent check); (b6) the refusal removed only for this element; (b7) defaults bit-identical, the plain Hex8
and the small-strain paths untouched; (b8) mutations (dropping the condensation correction to the residual, dropping the
internal-mode update between iterations) shown caught. The formulation is to be the published enhanced-deformation-gradient
form (Simo and Armero 1992, Simo, Armero and Taylor 1993), cited from the papers' content, clean-room.

Clauses: **c0** every limit registered here; **c1** the Incompatible Hex8 at finite strain as an enhanced assumed strain
element; **c2** the internal modes condensed in the Newton loop; **c3** the nonlinear driver no longer refuses it; and from the
brief **c4** small-strain limit (b1), **c5** patch test (b2), **c6** large-deflection cantilever (b3), **c7** Newton order (b4),
**c8** consistent condensation (b5), **c9** refusal removed only for this element (b6), **c10** defaults bit-identical (b7),
**c11** mutations (b8).

### Readings, registered

- **c1: which enhancement.** J. C. Simo and F. Armero, "Geometrically non-linear enhanced strain mixed methods and the method of
  incompatible modes", Int. J. Numer. Meth. Engng 33 (1992) 1413-1449, and J. C. Simo, F. Armero and R. L. Taylor, "Improved
  versions of assumed enhanced strain tri-linear elements for 3D finite deformation problems", Comput. Methods Appl. Mech.
  Engrg. 110 (1993) 359-386 (both cited from memory, NOT re-read): the deformation gradient is enhanced additively in the
  reference configuration, `F = I + Grad u + F_enh`, with `F_enh = sum_k alpha_k (x) G_k`, the modes `G_k` chosen so that the
  enhancement is orthogonal to constant stresses (`int_V G_k dV = 0`); with the gradients of the incompatible displacement modes
  of Wilson et al. as `G_k` (Taylor's correction with the centre Jacobian making them integrate to zero), the element is the
  method of incompatible modes at finite strain, which Simo and Armero show is a special case of the enhanced strain method.
  Reading registered: the F-390 element of ADR 0493 arithmetic 2 (nine unknowns, the modes `P_k = 1 - xi_k^2` per displacement
  component, the mode gradients `G_k(q) = (det J_0 / det J_q) J_0^{-T} dP_k/dxi (q)`) with `F = I + sum_a u_a (x) g_a + sum_k
  alpha_k (x) G_k`, the material at the Green-Lagrange strain of that `F` (D1). The improved 1993 versions (other mode sets, 21
  or 30 parameters, and the stabilisations proposed later against the hourglass instability in compression) are not built: the
  item names the F-390 element (Limits).
- **c2: condensed in the Newton loop.** Simo and Armero's algorithm keeps `alpha` per element across the global iterations and
  updates it with the condensed increment, `alpha_{k+1} = alpha_k - K_aa^{-1} (R_a + K_au du)`. This driver commits element
  states only per converged attempt and evaluates every element from the committed state (ADR 0475 D3), and the arc-length,
  follower, transient and harmonic paths call the same assembly. Reading registered: at every element evaluation of every
  global Newton iteration, `alpha` is found by a local Newton iteration on `R_a(u, alpha) = 0` from `alpha = 0`, and the element
  returns the condensed force `f_u - K_ua K_aa^{-1} R_a` and tangent `K_uu - K_ua K_aa^{-1} K_au` at the converged `alpha` (D2).
  The element is then a function of `u` and the committed material state only, its tangent is the exact derivative of its force
  (implicit function theorem), and no state is added (the `ElementState` layout is that of the Full Hex8). Not stored between
  evaluations (open question 1, the warm start).
- **c3, c9: what "no longer refuses" covers.** The refusal of ADR 0506 D2 (`check_model`, `hex8_force_and_tangent_at`,
  `hex8_internal_force_at`, `hex8_second_piola_kirchhoff`) is removed for the Incompatible formulation; every path that takes a
  BBar block through the assembly takes an Incompatible block the same way (`solve`, `solve_with`, `solve_from*`,
  `solve_in_time*`, arc length, follower, `nl_transient`, the harmonic paths about a nonlinear state). Refusals that stay, with
  their keys: the energy-momentum step of `nl_transient` (Full only, ADR 0514, key `"nl_transient"`), a temperature field (Full
  only, ADR 0534, key `"temperature"`), `hex8_strains(Incompatible, ..)` (it has no material; the strain of the element comes
  from `nl_incompatible::modes_and_strains`, key `"formulation"`, D4), and every linear-path refusal of ADR 0525 (unchanged).
- **c7: the order reading.** The elastica cantilever is not a reading of order: the prototype (`probe_cant_50.log`) shows its
  relative residual falling `0.394, 5.9e-5, 3.8e-6, 2.2e-12` in step 1 (a pre-asymptotic pair `r_{k+1}/r_k^2 = 1.1e3`, the
  slender beam's axial-bending coupling) and a floor near `1e-11` from step 4 on, so that Q(10, 1e-13) is violated 9 and 21 times
  by the consistent element; the plain Hex8 and the Hex20 of ADR 0404 have the same character. Reading registered: Q(C, F) of
  ADR 0515 (every pair `k >= 1` with `r_{k+1} <= max(C r_k^2, F)`, `C = 10`, `F = 1e-13`; no active set, the material is
  elastic) on a compact block in large shear (D5, Q1), with at least one informative pair (`r_{k+1} > F`) in every increment.
  The cantilever reports its histories only (N4).

## What was read

- `AGENT_RULES.md`; the brief; ADR 0506 (whole: arithmetic 1 to 8, D1 to D8, the checks, mutations, addendum, the follow-up
  text); ADR 0493 (arithmetic 2, D1, the checks, the follow-ups); ADR 0404 (D1, D2, D5, the A6 elastica set-up and its
  addendum: Hex20 50 x 1 x 1 within `2.4e-4`, `4.8e-4` (alpha 1) and `3.0e-4`, `5.8e-4` (alpha 2)); ADR 0515 (the reading Q);
  ADR 0519 (its use); ADR 0603 and ADR 0572 for the form.
- `crates/rcfd-fem/src/hex8x.rs` (whole), `nonlinear.rs` (head to `check_driver_model`, `internal_force_field`,
  `force_and_tangent`, `classify`, `Problem::attempt_with`), `nl_transient.rs` (`Integrator::new` checks, `strain_energy`),
  `nl_transient/emc.rs` (`check_model`), `element.rs` (`PointGeometry`, `point`, the stiffness rules), `builders.rs` (`hex_box`),
  `model.rs` (the material constructors), `nonlinear_md.rs`, `lib.rs`.
- Tests: `si_fbar.rs` (the patch, `FIELD_A`, `distorted`, RF1 and the copy `rf1_rf2_rf3_refusals_after_f587`),
  `si_hex8x_linear.rs` (`rf1_rf2_rf3_refusals_after_f588_amended`, `rf_rows`), `si_hex8x.rs` (the cantilever builders),
  `si_nonlinear.rs` (the elastica cantilever, `elastica`, `rk4`, `order`, A6), `si_fbar_default_bits.rs` (the digest scheme).

## Arithmetic done before the code (disclosed)

1. **Kinematics** (c1). At the points `q` of the 2 x 2 x 2 rule (the Hex8 stiffness rule, the order of the states), the
   reference gradients `g_a = dN_a/dX` (`a = 0..7`), the weights `w_q = w det J_q`, and the mode gradients `G_k` (`k = 0, 1, 2`)
   of ADR 0493 arithmetic 2 (`G_k[m] = (det J_0 / det J_q) (J_0^{-1})_{k m} (-2 xi_k)`, `hex8x::mode_gradients`). With the
   extended list of eleven gradients `(g_0 .. g_7, G_0, G_1, G_2)` and the 33 unknowns `v = (u, alpha)` (node-major, `alpha` in
   the order `3 k + c` of `hex8x::internal_modes`), `H = sum_n v_n (x) grad_n`, `F = I + H`, `E = (H + H^T + H^T H)/2`
   (`green_lagrange`, ADR 0404 amendment 1). The extended element is the total Lagrangian element of ADR 0404 D1, D2 over eleven
   "nodes": `f = sum_q w_q B_NL^T S` (33), `K = sum_q w_q (B_NL^T C_t B_NL + K_sigma)`, `B_NL` the matrix of ADR 0404 D1 over
   the eleven gradients (6 x 33), `K_sigma[(m,i),(n,j)] = delta_ij grad_m . S grad_n` (the geometric stiffness of ADR 0390 D1,
   the mode rows and columns included). For a hyperelastic material `f` is the gradient of `Pi(u, alpha) = sum_q w_q W(E)` and
   `K` its Hessian (symmetric for a symmetric `C_t`).
2. **Condensation** (c2). `f = (f_u, R_a)`, `K = [[K_uu, K_ua], [K_au, K_aa]]`. For fixed `u` the local Newton iteration
   `alpha <- alpha - K_aa^{-1} R_a` from `alpha = 0` to `|R_a| <= tol r_a` (D2: `r_a = max_k sum_q w_q sum_m |B_NL[m][24 + k]|
   |S_m|`, the size of the terms that cancel in `R_a`; `tol = 1e-12`), at most 25 iterations, then `f_c = f_u - K_ua K_aa^{-1}
   R_a`, `K_c = K_uu - K_ua K_aa^{-1} K_au` at that `alpha`. With `R_a(u, alpha(u)) = 0`, `d alpha/du = -K_aa^{-1} K_au` and
   `d f_u(u, alpha(u))/du = K_c`: the condensed tangent is the exact derivative of the condensed force. The term `-K_ua
   K_aa^{-1} R_a` is Simo and Armero's condensed residual; at the converged `alpha` it is below `tol` relative and changes no
   result above rounding (MU4 is registered as equivalent). `K_aa` is factored by Gaussian elimination with partial pivoting (it
   need not be positive at finite strain); a zero or non-finite pivot and a local iteration that does not converge are refused
   with the key `"internal modes"` (located by the assembly; the driver halves the step as for any element failure).
3. **Small-strain limit** (c4). At `u = 0` the first local residual is `R_a = 0` (`S = 0`), so `alpha = 0`, `F = I`,
   `B_NL = [B, B_alpha]` (the linear matrices of `hex8x`), `K_sigma = 0`, and `K_c(0) = K_uu - K_ua K_aa^{-1} K_au` with `D`: the
   F-390 condensed stiffness `hex8x::element_stiffness(Incompatible)` up to the rounding of the two eliminations (Cholesky with
   a symmetrised product there, Gauss here). Prototype (`probe_eas.log`, the T1 element, `E = 2e11`, `nu = 0.3`):
   `|K_c(0) - K_lin| / |K_lin| = 1.6e-16`. Modes at `u = eps w`: `alpha - alpha_lin = O(eps^2)` (the quadratic part of `E`),
   relative `O(eps)`: `1.9e-6` at `eps = 1e-6`, `2.6e-4` at `1e-4`, `1.8e-3` at `1e-3` (three random directions; the registered
   check uses one direction for both amplitudes, the ratio then about 100).
4. **Patch test** (c5). For a homogeneous `F_c = I + A` (the nodal field `u = A X + c`), `alpha = 0` gives `F = F_c`, `S`
   constant, and the mode rows of `B_NL` are linear in `G_k`: `R_a = sum_q w_q B_alpha(G_k(q), F)^T S = B_alpha(sum_q w_q
   G_k(q), F)^T S` and `sum_q w_q G_k(q) = det J_0 J_0^{-T} (-2) sum_q w xi_k(q) e_k = 0` exactly on any element (the 2-point
   rule is symmetric). So `alpha = 0` is the local solution (taken at the first test, `R_a` at rounding), `f_c = f_u` is the Full
   element force, the interior nodes are in equilibrium and the driver reproduces the field. Without Taylor's correction
   (`G_k = J_q^{-T} dP_k/dxi`) `sum_q w_q G_k != 0` on a distorted element and the patch fails (ADR 0493's control N1 at small
   strain `3.76e-3`; MU5 here).
5. **Central differences** (c8). The prototype element (T1's distortion, `0.1 (2U - 1)` corners) with SVK: `|K_c - K_fd| /
   |K_c|` `4.4e-11` (`u ~ 1e-3`, `h = 1e-8`), `3.1e-8` (rotation by 90 degrees about `z` plus `5e-2`, `h = 1e-8`; `2.0e-9` at
   `h = 1e-7`); asymmetry `<= 1.6e-16`; the extended 33 x 33 tangent at a random `alpha` (`1e-2`) `1.8e-9`; `R_a` at the
   returned `alpha` relative to `|f_u|` `4.1e-16` and `2.3e-14`; 3 local iterations. Mutations on the prototype: the tangent
   `K_uu` alone `0.147`, `0.134`; one local step `8.1e-8`, `3.6e-4`; the geometric part without the mode rows `1.7e-4`, `5.8e-3`.
   The truncation and rounding estimate of ADR 0506 arithmetic 6 applies (`h = 1e-8`: truncation `~1e-10` at `|u| ~ 1e-3`,
   rounding `~1e-8` at `|u| ~ 1`). Limit `1e-6`, the limit of ADR 0506 T1. J2, creep and the neo-Hookean material are not
   prototyped; their return maps add the estimate of ADR 0506 (measured there `<= 1e-8`).
6. **The elastica cantilever** (c6). The set-up of `si_nonlinear.rs` A6 (ADR 0404): `L = 1`, `h = b = 0.02`, `E = 2e11`,
   `nu = 0`, `xmin` clamped in the three components, the dead tip traction `alpha E I / L^2` in `y` on `xmax`, `5 alpha` load
   steps; the elastica `v/L = 0.30172077`, `d/L = 0.05643324` (alpha 1) and `0.49345748`, `0.16064172` (alpha 2), registered there.
   Tip reading for Hex8: the mean of `u_y` and of `-u_x` over the four nodes of the tip face (the neutral axis by symmetry). Probe
   (`probe_cant_50.log`, `probe_cant_more.log`; consistent bilinear face loads, Newton to the relative residual `1e-10` of the
   driver's reference):

   | mesh | element | alpha 1: `|v/v_ref - 1|`, `|d/d_ref - 1|` | alpha 2 |
   |---|---|---|---|
   | 50 x 1 x 1 | this element | `1.53e-4`, `3.44e-4` | `2.26e-4`, `4.64e-4` |
   | 25 x 1 x 1 (aspect 2) | this element | `1.73e-4`, `2.03e-4` | `1.38e-4`, `1.46e-4` |
   | 50 x 1 x 1 | plain Hex8 | `0.298`, `0.515` | `0.234`, `0.438` |
   | 200 x 1 x 1 | plain Hex8 | `2.49e-2`, `5.09e-2` | `1.75e-2`, `3.84e-2` |
   | 400 x 1 x 1 | plain Hex8 | `6.15e-3`, `1.27e-2` | `4.17e-3`, `9.31e-3` |

   ADR 0404's Hex20 50 x 1 x 1: `2.4e-4`, `4.8e-4` and `3.0e-4`, `5.8e-4`. The element on 25 elements is within `2.1e-4`; the
   plain Hex8 on 400 elements (16 times) is still off by `4.2e-3` to `1.3e-2`. Limits: CB1, CB2 `2e-3` (margin 4.3 over the
   worst probe `4.64e-4`); CB3 (against the Hex20 run of the same test) `2e-3` relative to the elastica (the worst sign
   combination of the two probes and ADR 0404's numbers `1.04e-3`); the controls CL `>= 0.1` (probe `0.234`) and CN (the plain
   Hex8 on 200 x 1 x 1, eight times CB2's count) `>= 5e-3` on `v` and on `d` (probe `1.75e-2`, margin 3.5).
7. **The Newton order on a block** (c7). Unit cube, 3 x 3 x 3 Hex8, interior nodes moved by `0.1 h (2U - 1)`, SVK `E = 2e11`,
   `nu = 0.3`, `zmin` fixed, `zmax` prescribed `(0.3, 0, -0.1)` (a shear of 30 per cent with a compression of 10 per cent of the
   height), 4 load steps (`probe_block.log`): this element `0.113, 5.4e-4, 2.3e-8, 3.0e-16` and the like in every step, the
   largest `r_{k+1}/r_k^2` above `1e-13` `0.246`, no violation of Q(10, 1e-13) in 12 pairs; plain Hex8 `0.132`; the tangent
   `K_uu` alone (MU1): 17 to 19 iterations per step, 63 violations of 67 pairs, `r_{k+1}/r_k^2` up to `2.7e9`. Limits: Q1 no
   violation; an informative pair in every step.
8. **Small load** (c4, SS3). The one-layer cantilever of `si_hex8x.rs` (`L = 2`, `h = 0.1`, 20 x 1 x 1, `nu = 0.3`, `xmin`
   with `u_x = u_z = 0` and the root corner `u_y = 0`, the tip shear traction for `P = 1000`) at the load scale `s = 1e-3`:
   tip deflection `1.6e-6 L`, so the geometric terms relative `~(1.5 delta/L)^2 ~ 1e-12`; the driver stops at a relative
   residual `1e-10` (one correction from `u = 0` gives the linear solution, whose residual is of the order of the geometric
   terms). `max |u_nl/s - u_lin| / max |u_lin|`: estimate `<= 1e-9`, limit `1e-6`.
9. **Default bits** (c10). The Full and BBar arms keep their operations: the element loops of `evaluate`, `evaluate_fbar` and
   the linear paths are not changed; the assembly takes a new arm only for an element whose block is Incompatible
   (`hex8x::opted`); the only changed condition of `check_model` is on an Incompatible block; `nl_transient::strain_energy`
   gains an arm for an Incompatible element only; `hex8x.rs` gains a function and keeps every existing one.

## Design

### D1. The extended element (`crates/rcfd-fem/src/nl_incompatible.rs`, new module)

`hex8x` gains `pub(crate) fn mode_points(coords) -> Result<Vec<ModePoint>>` (per point of the stiffness rule its geometry
`grad`, the three mode gradients by the existing `mode_gradients` with the centre Jacobian, and the weight; the existing
functions untouched). The new module evaluates arithmetic 1 at `(u, alpha)`: per point the eleven gradients, `H`, `F`, `det F`
(a point with `det F <= 0` or not finite refused with `nonlinear::INVERSION_KEY`, as in `evaluate`), `E = green_lagrange(H)`,
the material by the driver's `point_update` (J2, viscoelastic with states, creep, the others by `Material::stress`) from the
committed state of the point, `B_NL` over the eleven gradients, the force, the material and geometric tangent. Public, for
checks: `enhanced_force_and_tangent(coords, material, displacement, modes, state, context) -> (Vec<f64> /* 33 */,
ElementMatrix /* 33 */)` (no condensation).

### D2. The condensed element (`nl_incompatible.rs`)

`evaluate(coords, material, displacement, state, want_tangent, context)` (crate) runs the local iteration of arithmetic 2 and
returns the driver's `Evaluation`: the condensed force `f_u - K_ua K_aa^{-1} R_a`, the condensed tangent `K_uu - K_ua K_aa^{-1}
K_au` (the product symmetrised `(s_ij + s_ji)/2` as `hex8x`), the stress at the points and the trial state of the last
evaluation (at the returned `alpha`). Constants `pub const LOCAL_TOLERANCE: f64 = 1e-12`, `pub const LOCAL_ITERATIONS: usize =
25`, `pub const ITERATION_KEY: &str = "internal modes"`. Public: `modes_and_strains(coords, material, displacement, state,
context) -> ([f64; 9], Vec<[f64; 6]>)` (the converged modes and the Green-Lagrange strain at the points, Voigt, engineering
shear).

### D3. Dispatch (`nonlinear.rs`, shared solver file)

`internal_force_field` and `force_and_tangent` evaluate an element whose block is Incompatible (`hex8x::opted(mesh, e) ==
Some(Incompatible)`) by D2, after the BBar arm, every other element as now. `check_model`: a solid block of the Incompatible
formulation is taken (`BBar | Incompatible => continue`). `hex8_evaluate` (behind `hex8_force_and_tangent_at`,
`hex8_internal_force_at`, `hex8_second_piola_kirchhoff`) takes `Incompatible` by D2. `hex8_strains(Incompatible, ..)` is
refused with `FemError::InvalidValue { key: "formulation" }` naming `nl_incompatible::modes_and_strains` and ADR 0608. The
function `incompatible_refusal` is removed (no caller is left); `hex8x::NONLINEAR_FOLLOW_UP` is kept (public, named by the
records) with its documentation saying that ADR 0608 retired the refusal. Module documentation of `nonlinear.rs` and `hex8x.rs`
updated.

### D4. `nl_transient.rs`

`Integrator::strain_energy` takes the strain of an Incompatible element from `nl_incompatible::modes_and_strains` at the
context of the energy (empty state: the energy is refused for a material with history), every other element as now. The mass is
the consistent mass, the damping tangent `K_0` `nonlinear::tangent` at `u = 0` (for an Incompatible block the F-390 condensed
stiffness, arithmetic 3).

### D5. The test files

`crates/rcfd-fem/tests/si_eas.rs` (every check below but B0), `crates/rcfd-fem/tests/si_eas_default_bits.rs` (B0). The block of
Q1: `hex_box([0, 0, 0], [1, 1, 1], [3, 3, 3])`, the interior nodes moved by `0.1 h (2U - 1)` with the SplitMix64 of
`si_fbar.rs` seeded `0xF596_0608`, `zmin` fixed (three components), `zmax` `u = (0.3, 0, -0.1)` by `Constraint::displacement`,
`NewtonOptions::new(4).threads(4)`. The cantilevers of CB: `hex_box([0, 0, 0], [1, 0.02, 0.02], [n, 1, 1])` with the block's
formulation, the material `isotropic_elastic(2e11, 0, 7850, 0)`, `xmin` fixed, `Load::traction("xmax", [0, P/h^2, 0])`,
`NewtonOptions::new(5 alpha).threads(4)`; the Hex20 run of CB3 is `si_nonlinear.rs`'s cantilever (50 x 1 x 1 upgraded, mid-edge
tip nodes) built in the test; the elastica by the test's copy of `si_nonlinear.rs`'s shooting (`rk4`, 4000 steps) and checked
against the registered values to `1e-6` (as C8 of ADR 0404).

### D6. Records and copies (ADR 0291's scheme, as ADR 0493 D7 and ADR 0506 D7)

Two existing tests assert the refusal and fail by design after D3: `si_fbar.rs::rf1_incompatible_refused_by_the_nonlinear_driver`
and `si_hex8x_linear.rs::rf1_rf2_rf3_refusals_after_f588_amended` (its rows `RF3 Incompatible nonlinear::internal_force /
tangent refused`). Each gets `#[ignore = "record of ...; the Incompatible nonlinear rows retired by ADR 0608; copy in si_eas.rs"]`
and nothing else; `si_eas.rs` carries `rf1_incompatible_taken_after_f596` (the six rows of the first, each asserting `Ok`) and
`rf1_rf2_rf3_refusals_after_f596` (a copy of `rf_rows(true)` whose two Incompatible nonlinear rows assert `Ok`, every other row
as in the original). No other existing test is edited.

### D7. The bit record B0 (`crates/rcfd-fem/tests/si_eas_default_bits.rs`)

Written after this commit and run on the base c64046db before any library code; FNV-1a 64 digests of the bits, the expected
values printed by that run and written into the file then (committed with the test, before any library code). Content (API of the
base only): `nonlinear::solve` of a distorted Hex8 box stretched and sheared, Full and BBar (SVK); `nonlinear::solve` of a J2
Hex8 bar, Full and BBar; `solve_in_time` of a creep Hex8 bar, BBar; `nl_transient` of a Hex8 bar, Full and BBar, with the strain
energy; `internal_force` and `tangent` of a distorted J2 model, Full and BBar; `hex8_force_and_tangent_at` Full and BBar on a
distorted element; `solve_static` and `assemble::stiffness` of the Incompatible cantilever (the linear path); the refusal texts of
the Full-only paths are not digested.

## Checks

Every measure is required finite before it is compared (a non-finite value misses). "Relative" is to the largest entry of the
reference.

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| SS1 | c4 | `hex8_force_and_tangent_at(Incompatible)` at `u = 0`, SVK `nu = 0.3`, against `hex8x::element_stiffness(Incompatible)`: the T1 element and every element of the PT1 patch; `nonlinear::tangent(model, 0)` against `assemble::stiffness` on the PT1 patch model and on the SS3 cantilever | `<= 1e-13` each | `1.6e-16` |
| SS2 | c4 | T1 element, `u = eps w` (`w` from the RNG, amplitude 1): `max |alpha - alpha_lin| / max |alpha_lin|`, `alpha_lin = hex8x::internal_modes(coords, D, u)`, at `eps = 1e-6` and `1e-4`; the ratio of the two | `<= 1e-5`, `<= 1e-3`; ratio in `[30, 300]` | `1.9e-6`, `2.6e-4`, about 100 |
| SS3 | c4 | the SS3 cantilever at `s = 1e-3`: `max |u_nl/s - u_lin| / max |u_lin|` (`nonlinear::solve` one step, `solve_static`) | `<= 1e-6` | `<= 1e-9` |
| PT1 | c5 | the PT2 patch of ADR 0506 (0.25 h, tag 1001), Incompatible, `F = I + FIELD_A`, one step: interior error; `nu = 0.3` and `0.4999` | `<= 1e-10` each | rounding |
| PT2 | c5 | the same, `100 FIELD_A` (strains 0.1), four steps | `<= 1e-10` | rounding |
| PT3 | c5 | the same, `F = R (I + 100 FIELD_A)`, `R` the rotation by 60 degrees about `(1, 1, 1)/sqrt 3`, eight steps (`u = (F - I) X + c` on the boundary) | `<= 1e-10` | rounding |
| PT4 | c5 | the same, J2 (`sigma_y = 1e8`, `2e9`, `1e9`), `FIELD_A`, four steps; set-up every point yielded | `<= 1e-10` | rounding |
| PM | c5 | PT1 to PT4: `max_e max |alpha_e|` from `modes_and_strains` at the converged state, over `max |F - I| * max element edge` | `<= 1e-10` | rounding (arithmetic 4) |
| PS | c5 | PT2: `hex8_second_piola_kirchhoff(Incompatible)` at every point against `D E(F)`, relative | `<= 1e-10` | rounding |
| T1 | c8 | the distorted element of `si_fbar.rs` (`distorted()`, seed `0xF587_0506`): `max |K - K_fd| / max |K|` of the condensed element, `h = 1e-8`: (a) SVK `u` amplitude `1e-3`; (b) SVK, rotation by 90 degrees about `z` plus `u` amplitude `5e-2`; (c) J2 (`2e8`, `2e9`, `1e9`) from a committed state built as in `si_fbar.rs` T1 (c) (plastic strains `2e-4`, deviatoric back stresses `1e7`, `alpha = 3e-4`, at random), `u` amplitude `3e-3`, set-up every point plastic; (d) creep (base `2e9`, `0.3`; `1e-25`, `4`, `-0.5`) from a committed state as `si_fbar.rs` T1 (d) (creep strains `2e-4`, `eps_c,eq = 3e-4`), `u` amplitude `1e-3`, context `(t, dt) = (2, 0.5)`, set-up every point creeping; (e) neo-Hookean (`mu = 8e10`, `kappa = 1.6e11`), the nodes moved by `0.3 B X` with the fixed `B = [[0.5, 0.2, -0.1], [0.1, -0.3, 0.2], [0.3, 0.1, 0.4]]` plus `u` amplitude `5e-2`. The test's RNG is the SplitMix64 of `si_fbar.rs` seeded `0xF596_0608 + 1`, drawn in the order (a) to (e) | `<= 1e-6` each | `<= 3.1e-8` (a, b) |
| T2 | c8 | T1 (a) to (e): `max |K - K^T| / max |K|` | `<= 1e-12` | `1.6e-16` |
| T3 | c8 | T1 (b) state with `alpha` random of amplitude `1e-2`: the extended 33 x 33 tangent of `enhanced_force_and_tangent` against central differences of its force, `h = 1e-8` | `<= 1e-6` | `1.8e-9` |
| T4 | c2 | T1 (a), (b), (e): `max |R_a| / max |f_u|` of `enhanced_force_and_tangent` at the modes returned by `modes_and_strains` | `<= 1e-10` | `2.3e-14` |
| X1 | c1 | T1 (b): the strain of `modes_and_strains` against the test's own `E(I + sum u_a (x) g_a + sum alpha_k (x) G_k)` with `G_k` built in the test from `Element::point` (arithmetic 1), relative to the largest | `<= 1e-12` | rounding |
| Q1 | c7 | the block of D5: Q(10, 1e-13) on every pair `k >= 1` of every step; an informative pair (`r_{k+1} > 1e-13`) in every step | 0 violations; every step | `0.246` largest ratio |
| CB1 | c6 | the elastica cantilever, Incompatible 50 x 1 x 1, alpha 1 and 2: `|v/v_ref - 1|`, `|d/d_ref - 1|` | `<= 2e-3` each | `<= 4.64e-4` |
| CB2 | c6 | the same on 25 x 1 x 1 | `<= 2e-3` each | `<= 2.03e-4` |
| CB3 | c6 | CB1 against the Hex20 50 x 1 x 1 run: `|v - v_H20| / v_ref`, `|d - d_H20| / d_ref` | `<= 2e-3` each | `<= 1.04e-3` |
| CE | D5 | the test's elastica against the registered values | `<= 1e-6` | as ADR 0404 C8 |
| NT1 | D4 | an Incompatible Hex8 bar (4 x 1 x 1, `xmin` clamped) under `nl_transient` (average acceleration, `dt = 1e-5`, a tip load history, 3 steps, as ADR 0506 NT1): `strain_energy` against `sum_q w_q S . E / 2` with `E` from `modes_and_strains` and the material in the test | `<= 1e-12` relative | rounding |
| RF1 | c3, c9 | `rf1_incompatible_taken_after_f596`: `solve`, `solve_in_time`, `internal_force`, `tangent`, `nl_transient::Integrator::new`, `hex8_force_and_tangent_at` of the RF1 model of ADR 0506 return `Ok`; refused with their keys: the energy-momentum step (`"nl_transient"`), `internal_force_with_field` (`"temperature"`), `hex8_strains(Incompatible)` (`"formulation"`, naming `modes_and_strains`) | exact | D3 |
| RF2 | c9 | `rf1_rf2_rf3_refusals_after_f596` (the copy of D6) | passes | D6 |
| B0 | c10 | `si_eas_default_bits.rs`: every digest equal to the base's | bit-equal | arithmetic 9 |
| S1 | c10 | `cargo test -p rcfd-fem --release` | 0 failed | |
| S2 | c10 | `cargo test -p rcfd-api --release --test baseline_hashes`; `cargo test -p rcfd-cli --release --test docs_site` | pass | |
| S3 | | `cargo fmt -p rcfd-fem --check`, `cargo clippy -p rcfd-fem --all-targets -- -D warnings` | clean | |

### Negative controls

| Id | Control | Expected |
|---|---|---|
| CL | CB1's measure of `v` with the plain Hex8 (Full) on 50 x 1 x 1 | `>= 0.1` at alpha 1 and 2 (probe `0.298`, `0.234`) |
| CN | the plain Hex8 on 200 x 1 x 1 (eight times CB2's elements): `|v/v_ref - 1|`, `|d/d_ref - 1|` | `>= 5e-3` each (probe `>= 1.75e-2`) |
| N3 | SS1 with the Full block (`nonlinear::tangent` at `u = 0` of the Full patch against the condensed stiffness) | `>= 1e-3` (the measure sees the formulation) |
| N4 | the plain Hex8 on 400 x 1 x 1 and the residual histories of CB1 | report only |

### Planned mutations (on a scratch copy, each run once against `si_eas.rs`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the condensation dropped from the tangent (`K_c = K_uu`) | T1, Q1 |
| MU2 | the internal-mode update dropped (`alpha` stays 0: no local iteration; the brief's "drop the internal-mode update between iterations") | T1, T4, CB1 |
| MU3 | one local Newton step only | T1 (b), T4 |
| MU4 | the condensation correction to the residual dropped (`f_c = f_u`), the local iteration intact | **equivalent at the local tolerance: expected not caught** (registered so; the brief's other mutation) |
| MU5 | Taylor's correction dropped in `mode_points` (`J_q^{-T}` in place of `(det J_0 / det J_q) J_0^{-T}`) | PT1, PM, SS1 |
| MU6 | the geometric stiffness without the mode rows and columns | T1 (a, b), T3 |
| MU7 | `check_model` keeps refusing Incompatible | RF1 |
| MU8 | `strain_energy` with the compatible strain for an Incompatible element | NT1 |
| MU9 | the dispatch of D3 dropped in the assembly (an Incompatible element evaluated as Full) | SS1, CB1 |
| MU10 | MU4 together with MU3 (one local step, no correction) | T1 (b) |

## Decision rule

- **done**: every check above met in its single registered run, the negative controls as stated, every planned mutation but
  MU4 caught by a registered check (MU4 registered as equivalent), `baseline_hashes` unchanged.
- **partial**: the code lands with B0, S1 and every check of c1 to c5, c8, c9 met, but a check of c6 or c7 is missed (recorded
  as a `*_known_miss` test with the limit intact, diagnosed), or a control does not behave as stated, or a mutation other than MU4
  is not caught.
- **todo**: B0 or S1 fails and cannot be fixed without changing a default result, or a check of c1 to c5, c8 or c9 is missed.

## Limits (not covered)

- The F-390 mode set only (nine modes); the improved enhanced elements of Simo, Armero and Taylor 1993 and later stabilised
  variants are not built.
- The hourglass instability of enhanced and incompatible-mode elements under large homogeneous compression (known in the
  literature since the early 1990s; cited from memory) is not checked and not stabilised; the cantilever's compressive strains
  stay below about 2 per cent, the block's compression is 10 per cent. A follow-up is proposed.
- No warm start: `alpha` is found from 0 at every element evaluation (open question 1); the cost of the local iteration (about 3
  extended evaluations per element evaluation in the probes) is not measured.
- Materials: those of the driver's `point_update`; J2 and creep through the driver are checked on the patch only (PT4) and by
  central differences (T1), the viscoelastic material by none (it goes through the same `point_update`).
- A temperature field and the energy-momentum step stay refused for the formulation; the linear paths are unchanged.
- No case-file key (the block property follow-up of ADR 0506 covers it).
- The cantilever reference is the elastica and the Hex20 at one mesh; no mesh convergence study of this element.

## Open questions (taken or open)

1. **Warm start of the modes.** Options: (a) `alpha` from 0 at every evaluation (no state, the element a function of `u` and
   the committed material state); (b) the modes stored in the element state and updated per iteration as Simo and Armero do.
   Taken (a) on the maintainer's standing instruction of 2026-11-11 (take the recommendation): (b) changes the state layout of
   every path (`StateLayout`, the commit and rollback, the active-set record) and makes the element depend on the iteration
   history; (a) costs only local iterations.
2. **Local tolerance.** `1e-12` of the cancelling terms (arithmetic 2): taken on the same instruction (the rounding of `R_a` is
   about `1e-15` of them; the probes converge in 3 iterations).
