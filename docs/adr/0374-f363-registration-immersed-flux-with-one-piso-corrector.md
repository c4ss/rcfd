# ADR 0374: F-363 registration: the second-order immersed face flux with a single PISO corrector

Status: **REGISTERED 2026-11-12, written before any code of F-363 and before any run of its checks**. Not yet accepted by the
maintainer. Before this text nothing of F-363 was written, compiled or run: the documents and code under "What was read" were
read, and the arithmetic under "Arithmetic before this registration" was done by hand. Results go in an addendum; a defective
set-up is corrected by an amendment written and committed before the corrected run; nothing above an amendment or the
addendum is changed after a run.

Parents: [ADR 0254](0254-f355-registration-immersed-second-order-forcing-in-transient-piso.md) (F-355: the bounded form
`build_second_order_bounded`, the refusal `flux_growth`, the record `sb_bounded_with_one_corrector_known_miss`, the
measurement D11 and question Q3, from which F-363 was made),
[ADR 0234](0234-f309-registration-immersed-face-flux-at-the-surface-crossing.md) (F-309: the face flux at the crossing, D1 to
D3), [ADR 0268](0268-f354-registration-dfg-2d2-force-extrema-mesh-and-time-step-convergence.md) and
[ADR 0363](0363-f466-registration-bounded-forcing-stop-on-t.md) (F-354, F-466: the bounded form on T; the stop there was the
AMG-preconditioned pressure CG, `pressure_ic_fallback`), ADR 0073 B4 and ADR 0156 (the pressure-lag map and its estimate).
Roadmap item F-363 (deps F-355).

Number: the lead gave 0374. Branch `f363-immersed-single-piso` from main 18c92d08, worktree `w49i-work/f363`, scratch
`w49i-work/f363-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run; a missed limit becomes an ignored
`*_known_miss` test with the limit intact and the number recorded; each registered run is made once; defaults stay
bit-identical; no hash table, golden or existing test file is edited (the helpers of `immersed_piso_stop_f355.rs` are copied,
that file is not edited); no new third-party crate, no unsafe code; unsigned commits without trailers; no push.

Clean-room: the algorithms are rcfd's own. PISO is that of Issa, J. Comput. Phys. 62 (1986) 40-65, and the remark that a
single corrector leaves the pressure of the predictor in the corrected velocity (the pressure lag) is the argument of ADR 0073
B4, both as written in `crates/rcfd-fv/src/pimple.rs` and its ADRs (cited from memory for the paper; nothing of it is used
beyond what ADR 0073 derived). The face flux and the image form are those of ADR 0234 and ADR 0254 (themselves after the
ghost-cell literature cited there from memory). No vendor code, text, manual or binary is used.

## What F-363 asks

Roadmap entry: "Second-order immersed forcing: the single PISO corrector"; acceptance "The stop of DFG 2D-2 on M2 with one PISO
corrector and a second-order immersed flux face of weight above 1 (ADR 0254 D11: step 34 with the bounded form, step 126 with
weights up to 2) is explained by a registered measurement, and the setting either runs 200 steps or is refused with a located
message; two correctors are unaffected." The acceptance is taken as binding, clause by clause:

1. *explained by a registered measurement* (checks R0, L1, L2, L3 below);
2. *runs 200 steps or is refused with a located message* (the second branch is taken: check RF);
3. *two correctors are unaffected* (checks N0 and RF2, and the existing tests that run two correctors).

## What was read (disclosed)

ADR 0254 in full (registration, amendments 1 and 2, addendum: D11, SB, Q3); the head and the results of ADR 0363; the roadmap
entries F-363, F-355, F-354, F-466; `crates/rcfd-fv/src/immersed.rs` lines 270 to 714 (`FluxFace`, the constructors,
`reconstruct_face_flux`, `flux_growth`); `crates/rcfd-fv/src/pimple.rs`: the settings (`n_correctors`, `pimple()`), the solver
struct, `new` and `scratch`, `lag_preflight`, `lag_trial`, the momentum predictor and the immersed forcing (1600 to 1749), the
corrector loop (1769 to 2054), `Divergence::message` (820 to 865), `lag_map` (2124 to 2236), `lag_amplification_n` and
`pick_correctors`; `pressure.rs` `solve_pressure_src`; `crates/rcfd-fv/tests/immersed_piso_stop_f355.rs` (the DFG helpers,
`Variant`, `run_variant`, `r_solver`, D11, SB, RF); a grep of the immersed test files for `n_correctors`, `pimple(` and the lag
check (every existing immersed run with a second-order forcing uses two or more correctors, except the diagnosis runs of
ADR 0254 that switch the check off, and D11 (c) and the ignored record of SB, which use the bounded form with one corrector).

Facts read from the code (no run):

- `HbyA = H(u)/a_P` is built from the matrix without the pressure gradient. The predictor solves `a_P u* - H(u*) = b - V grad
  p_old`, so in the first corrector `HbyA_c = u*_c + rAU_c (grad p_old)_c` up to the momentum tolerance: `HbyA` carries the
  pressure gradient that drove the predictor.
- The flux of a flux face is overwritten by `reconstruct_face_flux`: `(1 - w) HbyA_c . S` (ADR 0234 D1, `w` the weight) or,
  on an image face, `(2 u_G - (1 - a) HbyA_c - a HbyA_d) . S` (ADR 0254 E1). The solid cells take the net outflow of these
  fluxes as their volume source, so the pressure gradient flux out of a solid cell sums to the (negligible) flux of its
  solid-solid faces: the solid pressure follows the fluid pressure and a flux face carries essentially the reconstructed flux,
  not a pressure correction.
- The pressure-lag map of ADR 0073 B4 (`lag_map`) does not reconstruct the flux faces: with an immersed forcing it is the map
  of the first-order forcing (the forcing is in the frozen matrices, the face flux is the interpolated one). It is reached only
  through `pressure_lag_check`, `corrector_control`, `lag_preflight` and `lag_trial`.
- The refusal of ADR 0254 E2 (`flux_growth`) looks only at faces without an image of weight above 5; with weights of at most 2
  or with the bounded form it never fires, so the stops of D11 are not refused today.

## Arithmetic before this registration (disclosed): the hypothesis HL

On a regular face the corrected flux of the first corrector is `interp(HbyA) . S - gamma snGrad p_new`; with `HbyA = u* +
rAU grad p_old` the pressure parts are `rAU (grad p_old - grad p_new)`: the lagged pressure enters with the sign that the new
pressure undoes, and a pressure error `e` left at the end of a step returns in the next one multiplied by about `-D`, `D` the
off-diagonal share of the momentum diagonal (ADR 0073 B4; one corrector is stable while that product stays below one).

On a flux face of weight `w` the reconstructed flux is `(1 - w) HbyA_c . S`, and it is not corrected by the pressure (above).
For `w <= 1` the lagged part enters with the factor `(1 - w) >= 0`, the same sign as on a regular face and smaller. For `w > 1`
it enters reversed, `-(w - 1) rAU_c grad p_old . S`, and on an image face reversed at full size (coefficients `-(1 - a)` and
`-a`, magnitudes summing to 1). A crude one-dimensional balance of the first fluid cell (its wall face imposed, its opposite
face regular) gives the local lag gain about `w D_c` for the extension and about `2 D_c` for an image face, against `D_c` for a
regular cell, so the reversal can lift the gain of the cells at the body above one while the rest of the domain stays below.
A second corrector rebuilds `HbyA` from the corrected velocity, in which the pressure enters only as `grad p_old - grad p_1`
through the neighbours, so the reversed part is no longer the full lagged gradient. I do not trust the one-dimensional numbers
(the pressure coupling of the first fluid cell, the corner cells and the cell `d` of an image face are left out, and `D_c`
near the wall depends on the local convection), so **no number is predicted**; only the signs and the dependence on the time
step, below.

**HL.** The stop of D11 is a pressure-lag instability of the single corrector: the reconstructed flux of a face of weight above
1 (or of an image face) returns the lagged pressure gradient of the predictor with the reversed sign, and the linearised lag map
of the step that includes the reconstruction has a spectral radius above one with one corrector and below one with two.

Consequences used as tests: (i) the lag map with the reconstruction, `rho_1`, is above one for the three settings that stop
and below one for the two that run; (ii) without the reconstruction (the map as it is now) `rho_1` is below one for the bounded
form, the forcing being otherwise the same; (iii) `rho_2 < 1` for all five; (iv) the mode sits at the body; (v) a pressure-lag
gain grows with the off-diagonal share, which falls with the time step (at `dt = 2.5e-4` the time term of BDF2 is `6000` 1/s
against about `640` viscous and `600` convective per unit volume on M2, against `1500` at `dt = 1e-3`), so with a step four
times smaller the bounded form with one corrector must run, and `rho_1` there must be below one; ADR 0254 H7 showed that the
large-weight stop of F-355 does not depend on the step, so this run separates the two mechanisms. A run that grows over
physical time would also stop within 200 steps of `2.5e-4` (`t = 0.05`, beyond the stops at `t = 0.034` and `0.046`).

## Design (D1 to D3)

**D1. The lag map with the reconstruction (diagnostic, opt-in).** A public field `PimpleSolver::immersed_lag: bool` (`false`
from `new`; copied by `scratch`). When `true` and the forcing has flux faces, `lag_amplification_n` builds once a homogeneous
copy of the forcing (every `FluxFace::wall` and `rigid` set to zero: the map is linear and homogeneous) and `lag_map` calls its
`reconstruct_face_flux` on `phi_h` right after `face_flux` (before the consistent split, the order of the step) and solves the
pressure with `solve_pressure_src` and the returned source. With `false`, or without flux faces, `lag_map` is the present one,
operation for operation. The estimate is read with the existing `lag_trial(dt, iterations, n_correctors)`.

**D2. The refusal (the second branch of the acceptance).** In `PimpleSolver::step_with`, after the corrector count of the outer
iteration is known and before its correctors: when that count is 1, `n_outer == 1`, and the forcing reports a flux face that
returns the fluid velocity reversed, the step is stopped with a located record (`Divergence`, stage `pressure correctors`,
quantity `immersed flux corrector`, the fluid cell, the value = the largest such weight) whose message names ADR 0374, says
that the face returns the velocity of its fluid cell reversed and that with one corrector the lagged pressure is returned
reversed, and asks for `solver.n_correctors >= 2`. The report is a new method `ImmersedForcing::single_corrector_face(&self)
-> Option<(usize, f64)>`: `None` when `flux_weight_check` is `false` (the switch of ADR 0254 E2, which then means "the solver
runs the flux faces wherever they lead", as its doc says) or when no flux face has an image or a weight above `1 + 1e-9` (the
tolerance covers the round-off of `t_f/theta_e` on a face whose weight is 1 by construction, as with `theta_min = 0.5` on the
uniform core); otherwise the fluid cell and the weight of the face with the largest weight. A constant
`SINGLE_CORRECTOR_WEIGHT_LIMIT = 1.0` holds the limit. `n_outer > 1` (PIMPLE with outer iterations) is not refused: not measured
(limits). The adaptive corrector count is covered (the count of the outer iteration is what is checked).

**D3. Defaults.** Without an immersed forcing, with `build` (no flux faces), with two or more correctors, with
`flux_weight_check = false`, and with `immersed_lag = false`, every operation of a step and of the lag map is the present one.
Hunks: `immersed.rs` (the constant and the method, added); `pimple.rs` (the field and its two initialisers, the refusal
statement, one branch of `Divergence::message`, the homogeneous copy in `lag_amplification_n`, one argument and one call in
`lag_map`). `simple.rs`, `sim.rs`, the case schema and the API are not touched.

Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation): the second branch (a located refusal)
rather than a change of the corrector algorithm (the first branch would change what an opt-in builds or how PISO corrects, and
the two-corrector runs already give the supported path); the refusal static (from the face weights, not from a run-time lag
estimate, which would cost about 40 step-equivalents at the start and depends on the state); `n_outer > 1` left alone.

## The runs

Set-up R of ADR 0254 (copied): DFG 2D-2 on M2 (`dfg_family(2)`, 176 x 104 cells), `U_m = 1.5`, `nu = 1e-3`, `K = 1e7`, the
start field of ADR 0249, `PimpleSettings::piso` with BDF2, `LinearUpwind`, AMG pressure, `tol_rel` 1e-6, `dt = 1e-3`, here
with `n_correctors = 1`. Five forcings, each with `flux_weight_check = false` for the runs R0 and L (so that the stops are
reproduced, not refused):

| Id | Forcing | Largest weight / face form |
| --- | --- | --- |
| F | `build`, `theta_min = 0.01` | no flux faces |
| Sa | `build_second_order`, `theta_min = 0.5` | at most 1 |
| Sb | `build_second_order`, `theta_min = 0.25` | up to 2, no image |
| B1 | bounded, `theta_min = 0.1` | image faces with `a <= 0.3` |
| B | bounded, `theta_min = 0.01` | image faces (the SB record) |

Verdicts as in ADR 0254 (at most 200 steps; "stops": the guard trips; "runs": no stop and the largest `abs(u)` at step 200
`<= 10`).

## Registered checks

New file `crates/rcfd-fv/tests/immersed_single_corrector_f363.rs`. The heavy records carry `#[ignore]` and are run once by me
in release; the fast checks run in release in the gate (and in debug where noted).

| Id | Check | Limit |
| --- | --- | --- |
| N0 (defaults; measured before any library code by the same test, committed before the library) | fingerprints, FNV-1a over the bits: FP1 F with one corrector, 20 steps, `u` then `p`; FP2 B with two correctors, 20 steps; FP3 `lag_trial(1e-3, 20, 1)` of F at step 0 and `lag_trial(1e-3, 20, 2)` of B at step 0, the bits of `rho` and the cell; FP4 Sa with one corrector, 20 steps (check on) | bit-equal to the values measured on the unchanged tree 18c92d08 |
| R0 (reproduction; heavy record) | the five one-corrector runs, check off | F runs, Sa runs; Sb stops at step 126, B1 at step 46, B at step 34, each by the Courant guard (the numbers of ADR 0254 D11 and SB; a different step is a defective copy of the set-up) |
| L1 (the measurement of HL; heavy record) | `immersed_lag = true`, `lag_trial(1e-3, 40, n)` for `n = 1, 2` on the state of each one-corrector run at step 0 (estimate at step 2) and at step 10 (estimate at step 11) | (a) `max(rho_1(2), rho_1(11)) > 1` for Sb, B1 and B; (b) `rho_1 < 1` at both instants for F and Sa; (c) `rho_2 < 1` at both instants for all five |
| L1n (negative control, the same record) | B, `immersed_lag = false` (the map as it is now), `n = 1`, both instants | `rho_1 < 1` at both |
| L2 (location, the same record) | the cell of the largest entry of the mode of `rho_1` of B at step 2 | its centre within `3 h = 0.0075` of the cylinder surface (`abs(abs(x - x_0) - D/2) <= 0.0075`, `h = 0.0025` the core spacing of M2) |
| L3 (the time step, the same record) | B with one corrector at `dt = 2.5e-4`, check off, 200 steps; its control F at the same step; `rho_1` of B at step 2 with `immersed_lag = true` | B runs, F runs, `rho_1 < 1` |
| RF1 (the refusal) | B with one corrector, check on, one step; Sb with one corrector, check on, one step | `diverged` at step 1, stage `pressure correctors`, quantity `immersed flux corrector`, value `> 1`, the cell a fluid cell of a flux face with an image or a weight above 1, the message contains "ADR 0374", "n_correctors" and "cell <c>" |
| RF2 (not refused) | B with two correctors, 5 steps; Sa with one corrector, 5 steps; F with one corrector, 5 steps; B with `PimpleSettings::pimple(nu, 2)` (one corrector, two outer iterations), 1 step | none stops |
| U1 (fast; also in debug) | `single_corrector_face` on circular Couette at N 24 (the set-up of ADR 0234, `theta_min = 1e-2`): `build` gives `None`; `build_second_order` with `theta_min = 0.5` gives `None`; the bounded form gives `Some((c, w))` with `w` the largest weight of its flux faces and `c` its fluid cell; the bounded form with `flux_weight_check = false` gives `None` | as stated |
| U2 (fast) | the lag estimate on circular Couette at N 24 (PISO, Euler, central, `dt = 2.5e-3`, the run V3 of ADR 0234): with `build`, `immersed_lag` `true` and `false` give bit-equal `rho` and cell; with `build_second_order` (`theta_min = 1e-2`) they differ in `rho` | as stated |
| N1 | the existing immersed test files (`immersed_solid_f219`, `immersed_second_order_f309`, `immersed_force_order_f337`, `immersed_unsteady_force_f346`, `immersed_piso_stop_f355`, `immersed_dfg_convergence_f354`, `immersed_bounded_stop_f466`) unedited, release, the tests that are not ignored; `cargo test -p rcfd-fv --release --lib`; `cargo test -p rcfd-api --release --test baseline_hashes` | every test passes |

Printed, no limit: every `rho` with its cell; for the runs that stop, the growth per step of the largest `abs(u)` over the last
three completed steps, next to `rho_1`; `rho_1` and `rho_2` of B at `dt = 2.5e-4`; the weight and form of the flux faces of
the cell of L2.

**Negative controls.** F and Sa in L1 (b), L1n, the control F of L3, RF2, FP1 and FP4.

### Mutations (scratch copy of the final tree, not committed)

Each applied alone by an exact text replacement checked to occur once, the named test run once in release, the file restored
and compared byte for byte with a kept copy.

| Id | Mutation | Must fail |
| --- | --- | --- |
| M1 | the refusal never fires (`n_corr == 1` made `n_corr == 0` in the refusal) | RF1 |
| M2 | the refusal fires with any corrector count (`n_corr == 1` made `n_corr >= 1`) | RF2 (B with two correctors refused) and FP2 |
| M3 | the refusal limit 1 made 2 (faces of weight up to 2 no longer refused) | RF1 (Sb not refused) |
| M4 | the lag map without the reconstruction (the homogeneous copy not built when `immersed_lag` is set) | U2 |
| M5 | `single_corrector_face` ignores `flux_weight_check` | U1 |
| M6 | the refusal reports the solid cell of the face instead of its fluid cell | RF1 |

## Decision rule

- **done**: N0, R0, L1 (a, b, c), L1n, L2, L3, RF1, RF2, U1, U2 and N1 met, and every mutation caught by the check named for it.
- **partial**: N0, RF1, RF2, U1 and N1 met (the refusal delivered, defaults and two correctors unaffected), and any of R0, L1,
  L1n, L2, L3 missed (the stop not explained in the registered form), or a mutation not caught by its named check, or caught
  only after an amendment written after a run (Rule I of the brief).
- **todo**: N0, RF1, RF2, U1 or N1 missed and not repaired; the library part is then not proposed.

A set-up check that does not hold (R0, a control run, the fingerprint measurement) is a defective set-up: an amendment before
any corrected run, never a pass.

## Limits (not covered)

One flow (DFG 2D-2) on one mesh (M2), BDF2, one convection scheme, one penalty; two dimensions, orthogonal meshes, a body at
rest. The lag map is a linearisation with the matrices frozen and no velocity error at the start of the step (ADR 0073); it is
an explanation of the instability, not a proof. The refusal is static: it also refuses settings that would run (the bounded
form with one corrector at a small enough step, L3), and the message says what to change. PIMPLE with outer iterations and one
corrector, the consistent correctors, SIMPLE and the fractional step are not examined (SIMPLE and the fractional step have
their own paths; the fractional step refuses the immersed forcing already). The refusal does not apply to `build_second_order`
faces with a weight of at most 1.

## Open questions

- **Q1. A run-time refusal instead of the static one.** Options: (a) static, from the face weights (as registered); (b) the lag
  estimate with the reconstruction at the first BDF2 step, refusing only when `rho_1 >= 1` (precise, costs about 40
  step-equivalents once, depends on the state); (c) both: static by default, the estimate available through
  `immersed_lag` with the existing `pressure_lag_check` (`LagAction::Stop`). Taken on the maintainer's standing instruction of
  2026-11-11 (take the recommendation): (a), with (c) available as a consequence of D1.
- **Q2. PIMPLE with outer iterations and one corrector.** Not measured; not refused. Recommendation: a follow-up only if a
  case uses it.
</content>
</invoke>
