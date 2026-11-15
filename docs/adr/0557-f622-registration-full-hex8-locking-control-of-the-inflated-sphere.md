# ADR 0557: F-622 registration: the Full Hex8 locking control of the inflated hyperelastic sphere with a converging set-up

Status: **REGISTERED 2026-11-15, written before any code of F-622 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-622 was written, compiled or run in the repository: the files listed under "What was
read" were read, and the arithmetic below was done by hand and with one plain numpy prototype in the scratch folder
(`w49i-work/f622-scratch/probe_lock1d.py`, log `probe_lock1d.log`: the spherically symmetric one-dimensional model of ADR 0541's
`probe_sphere1d.py`, copied and extended to other inner stretches and to pressure control; no rcfd code; disclosed, its numbers
quoted below). Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-622 (dep F-425), structural solver, advanced tier, the follow-up of ADR 0541 (its remaining question 5 and its
"For the lead" follow-up). Number 0557 given by the lead. Branch `f622-work` at 15f1fee9, worktree `w49i-work/f622`, scratch
`w49i-work/f622-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test
edited; no new dependency, no unsafe code, the crate graph unchanged; clean-room. Every choice below that the maintainer would
normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except the
open questions at the end.

## What F-622 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. The Full Hex8 locking control of the inflated sphere of ADR 0541 with a set-up that converges (a smaller inner stretch with its own registered limit, or pressure control with the arc-length step).

The roadmap note adds that amending F-425's small-strain clause to option (b) of ADR 0541 open question 1 is the maintainer's
call: it is **not** taken here (rule 6: it would change an acceptance text, not a default; it is the maintainer's), and nothing of
F-622 depends on it.

Clauses: **f0** every limit registered here; **f1** the Full Hex8 locking control of the sphere of ADR 0541 D6 (the same mesh,
material and boundary values) with a set-up that converges, by one of the two named options. Registered reading: option (a),
**a smaller inner stretch with its own registered limit**, is the acceptance's set-up (check A1). Option (b) is run as a
companion (P1 to P3) with pressure control by the load-stepping Newton driver, **not** with the arc-length step, for the reason
of arithmetic 4; the acceptance is an "or", so f1 is decided by A1 alone, and P1 to P3 count in the decision rule as companions.

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0541 (whole: D6 the sphere, arithmetic 5 the closed form and the 1D probe, amendment 2 the
  failed Full run and the diagnostic SPLd, addendum 1 and its known misses: SS1 the small-strain clause, SPL not measured);
  ADR 0544 (the F-611 registration, as the newest registration of the same material, for the style and for what it touches:
  `nl_transient` only, nothing of the static driver or of the sphere).
- `crates/rcfd-fem/tests/sa_hyper.rs` (the sphere helpers `l_b`, `p_closed`, `gauss_legendre`, `sphere_model`,
  `sphere_pressure`, `sphere_pressure_to`, the records `sp0_sp1_spl_the_inflated_sphere_known_miss`, `sp0_sp1_met_rows`,
  `spld_locking_diagnostic`, the `Checks` harness).
- `src/nonlinear.rs` (`NewtonOptions`, `solve`, `solve_with`, `DriverExtensions`, `controlled_attempt`, the pivot tests of the
  envelope factor: the static driver refuses a pivot `<= PIVOT_TOLERANCE` times the diagonal, so an indefinite tangent fails
  the attempt; the dispatch of `Full` and `BBar` at lines 614-615, `fbar_kinematics`), `src/follower.rs` (`FollowerPressure`:
  `new`, `force`, `face_force`, `positions`, the `Contribution` impl with the symmetric part of the load stiffness;
  `tangent_modes`), `src/arclength.rs` (whole: `trace`, its refusal of a zero dead load and its load direction from the dead
  loads only), `src/mesh.rs` (`FaceRef`, `HEX_FACES`, `add_surface_set`), `src/modal.rs` (`ModalOptions`, `ModalResult`).
- `docs/roadmap.json` (F-425, F-622).

## Arithmetic done before the code (disclosed)

1. **Why the Full run of ADR 0541 stopped.** The registered Full run (ADR 0541 amendment 2) failed in load step 3 (from `l_a =
   1.10`) at the load factor `0.2656` (`l_a = 1.133`) after five halvings, with a negative pivot `-2.79e7` against the diagonal
   `4.52e8`. The static driver factors the reduced tangent as positive definite (the envelope `L D L^T` with the pivot test of
   the static solve, `nonlinear.rs`), so any state whose tangent has a negative eigenvalue fails, whatever the increment. Five
   halvings brought the increment down to `0.05 / 32 = 1.6e-3` in stretch and the attempt still failed: the expected cause is
   that the discrete Full model itself loses positive definiteness near `l_a = 1.133` (a spurious instability of the locked
   element: the Gauss-point pressures `kappa (J - 1)` of the over-constrained element are large, of order `5 mu` and with both
   signs, and their geometric stiffness can overcome the deviatoric stiffness `~ mu` of the soft modes). It is not a property
   of the continuum (whose pressure rises monotonically to its maximum at `l_a = 1.83`, ADR 0541 arithmetic 5) nor of the BBar
   model (which reached `1.5`). Consequence for the design: the Full control can only be measured below `1.133`, by any control
   of the static driver (displacement, pressure or arc-length, which all pass through the same states); the diagnostic DX below
   tests the explanation by the lowest eigenvalue of the tangent along the path.
2. **The 1D model reproduces the 3D Full model** (disclosed: found from numbers known before this registration). ADR 0541's
   diagnostic SPLd at `l_a = 1.125` measured the 3D Full error `14.95` (`p_FE / mu = 5.466`) and BBar `3.93e-3`; the 1D model of
   ADR 0541 (8 geometric layers, two-point Gauss, `kappa/mu = 1e4`, run now in `probe_lock1d.log`) gives at `1.125` Full
   `p / mu = 5.464833` (error `14.95`) and mean dilatation `1.83e-3`. The 3D Full pressure is the 1D one within `2.2e-4`: the
   locking of this mesh is radial (the element has two Gauss points through the thickness of a layer, as the 1D element), and
   the angular discretisation adds little to it. The 3D BBar error is about `2.1` times the 1D one (the angular error, ADR 0541
   arithmetic 5).
3. **The smaller inner stretch, option (a).** Registered: `l_a = 1.10`, displacement control as D6 of ADR 0541 with the
   prescribed values `(l_a - 1) X`, 10 equal load steps (increments `0.01` in stretch, smaller than the `0.0125` of SPLd, which
   converged to `1.125`), so the path stays `0.033` below the failure. `1.10` is a fresh run (not SPLd's `1.125`, whose result is
   known). 1D model (`probe_lock1d.log`): closed form `p_c(1.10) = 0.2871553 mu` (`l_b = 1.0136`), Full `4.769507 mu` (error
   `15.61`), mean dilatation `0.287714 mu` (`1.95e-3`); the Full error is nearly constant along the path (`18.4` at `1.01`,
   `17.1` at `1.05`, `15.6` at `1.10`, `14.95` at `1.125`, `8.4` at `1.5`): the locking factor of this mesh is about 16 at
   small and moderate stretch. Estimates for 3D: Full error `15.6` (arithmetic 2: the 1D value within `2e-4`), BBar `4e-3`
   (`2.1 x 1.95e-3`). The control's limit is that of ADR 0541's SPL, `>= 0.1` (a locked element misses the closed form by more
   than ten percent); the companion A3 checks the size of the locking against the 1D model.
4. **Pressure control, option (b), and the arc-length step.** `arclength::trace` takes its load direction from the dead loads
   only ("a contribution's dependence on the load factor is in the residual, not in `du_f`") and refuses a load that is zero on
   the free equations. A follower pressure is a contribution (`FollowerPressure`), so the arc-length step cannot follow it
   without a library change of its load direction (an opt-in change of `arclength.rs`, outside F-622; open question 2). A dead
   nodal load in its place is not the inflation problem (it does not follow the area `l_a^2 A^2` of the cavity). It is not
   needed here: the closed-form pressure rises monotonically up to `l_a = 1.83`, so at `p_c(1.5)` load control by the Newton
   driver with the follower pressure is well-posed for the continuum and the BBar model; and the locked Full model reaches only a
   small stretch at that pressure. 1D model at `p = p_c(1.5) = 0.7554331 mu` (bisection in `l_a`): Full `l_a = 1.011413`
   (extension ratio `e = (l_a - 1)/(1.5 - 1) = 2.28e-2`), mean dilatation `l_a = 1.498856` (`(l_a - 1.5)/0.5 = -2.29e-3`).
   Estimates for 3D: Full `e = 2.3e-2` (far below `1.133`, so the Full tangent stays positive definite), BBar `|l_a - 1.5|/0.5
   = 5e-3` (`2.1` times the 1D value, arithmetic 2; the slope `dp_c/dl_a = 0.448 mu` at `1.5` turns the 3D pressure error
   `2.6e-3` of SP1 into `2.6e-3 x 0.755 / 0.448 / 0.5 = 8.8e-3`, the larger estimate, taken).
5. **The follower pressure on the inner surface.** The inner faces are local face 0 (`[0, 3, 2, 1]`, counter-clockwise seen from
   outside the element) of every element of layer 0 (its nodes 0 to 3 are at `R = A`, D6 of ADR 0541); its outward normal points
   into the cavity, so a positive pressure presses on the solid and pushes it outward (`FollowerPressure`: positive when it
   presses on the solid). The edges of the inner octant surface lie on the symmetry planes; there the driver's symmetric part of
   the load stiffness is not the exact tangent (ADR 0480 D1: "elsewhere the iteration converges at a reduced rate to the same
   solution"), so the Newton cap is raised to `40` iterations for P1 to P3 (registered; the tolerance unchanged). Inner
   stretch measured as the mean of `|x_a|` over the 127 inner nodes (`|X_a| = 1` exactly by the construction); the spread
   `max |x_a| - min |x_a|` and the cavity-volume stretch `(V_cav(x) / V_cav(X))^(1/3)` are reported.
6. **The diagnostic DX.** `follower::tangent_modes` at a converged state gives the lowest `omega^2` of `K_T phi = omega^2 M phi`
   with the prescribed equations held (density 1000 of D6). For the Full model: at `u = 0` (rest), at the state of A1 (`l_a =
   1.10`) and at the state of a second Full solve to `l_a = 1.125` (10 steps, the options of A1). Expected if arithmetic 1
   holds: `omega_1^2` decreasing along the path, and the straight line through the values at `1.10` and `1.125` reaching zero
   between `1.125` and `1.20` (the failure at `1.133`). Reported only (a diagnostic of the cause, no limit, outside the decision
   rule); if it does not behave so, the addendum says the explanation of arithmetic 1 is not supported.
7. **Tolerances.** Residual tolerance `1e-8` for every run (ADR 0541 arithmetic 7, the rounding floor at `kappa/mu = 1e4`),
   energy tolerance the default `1e-10`, 5 halvings, `threads(4)` (the result does not depend on it).

## Design

No library code. F-622 is a test file, `crates/rcfd-fem/tests/sa_hyper_lock.rs`, with copies of the sphere helpers of
`sa_hyper.rs` (that file is not edited; the copies are byte-for-byte the same functions, the pressure evaluation generalised to an
inner stretch as in `sphere_pressure_to` and split from the solve so that the state can be kept for DX). The mesh, the material
(`neo_hookean(mu = 1e6, kappa = 1e10)`, density 1000), the symmetry conditions and the pressure measure are those of ADR 0541
D6: octant `n = 6`, `m = 8`, 864 Hex8. Since no library line changes, the default bits are unchanged by construction (no B0
record needed; S1 and S2 are still run).

- **D1 (A1 to A3).** Displacement control to `l_a = 1.10`, Full and BBar, `NewtonOptions::new(10).threads(4)`, residual `1e-8`.
- **D2 (P1 to P3).** The inner surface set `"inner"` (face 0 of the 288 layer-0 elements), `FollowerPressure::new(model,
  "inner", p_c(1.5) mu)` as the one contribution of `solve_with`, scaled by the load factor; the symmetry conditions as the
  only prescribed values (zero); `NewtonOptions::new(10).threads(4)`, residual `1e-8`, `max_iterations = 40`. Full and BBar.
- **D3 (DX).** `tangent_modes(model, prescribed, u, states, 1.0, &[], ModalOptions::new(1).threads(4))` at the three states of
  arithmetic 6 (the default eigen options).

## Checks

Every measure is required finite before it is compared (a non-finite value misses); a failed solve misses the checks of its
run.

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| A1 | f1 | D1 Full: every load step converges, and `|p_FE - p_c(1.10)| / p_c(1.10)` | solve converges; `>= 0.1` | `15.6` |
| A2 | f1 (companion) | D1 BBar: `|p_FE - p_c(1.10)| / p_c(1.10)` | `<= 1e-2` | `4e-3` |
| A3 | f1 (companion) | D1 Full: `|p_FE / mu - 4.769507| / 4.769507` (the 1D full-integration model, arithmetic 3) | `<= 5e-2` | `2e-4` |
| P1 | f1 (option b, companion) | D2 Full: every load step converges, and the extension ratio `e_F = (l_mean - 1) / 0.5` | converges; `0 < e_F <= 0.1` | `2.3e-2` |
| P2 | f1 (option b, companion) | D2 Full: `|e_F / 2.2826e-2 - 1|` (the 1D model) | `<= 0.1` | `1e-2` |
| P3 | f1 (option b, companion) | D2 BBar: converges, and `|l_mean - 1.5| / 0.5` | converges; `<= 2e-2` | `9e-3` |
| P0 | set-up | D2: the spread `(max |x_a| - min |x_a|) / (l_mean - 1)`, the cavity-volume stretch, the iterations per step | reported | |
| DX | diagnostic | D3: `omega_1^2` at rest, `1.10`, `1.125`; the zero crossing of the line through the last two | reported; expectation of arithmetic 6 | in `[1.125, 1.20]` |
| S1 | | `cargo test -p rcfd-fem --release` | 0 failed | |
| S2 | | `cargo test -p rcfd-api --release --test baseline_hashes`; `cargo test -p rcfd-cli --release --test docs_site` | pass | |
| S3 | | `cargo fmt -p rcfd-fem --check`, `cargo clippy -p rcfd-fem --release --all-targets -- -D warnings` | clean | |

### Negative controls

A1 is itself the negative control of ADR 0541 (the standard Hex8 on the mesh of SP1). A2 and P3 are its positive companions (the
mean-dilatation element on the same mesh and set-up). No further control.

### Planned mutations (on a scratch copy, each run once against `sa_hyper_lock.rs`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | `nonlinear.rs` dispatch: `BBar` evaluated by the Full element (line 615 calls `evaluate(&Hex8, ...)`) | A2, P3 |
| MU2 | `nonlinear.rs` dispatch: `Full` evaluated by the F-bar element (line 614 calls `evaluate_fbar`) | A1, A3, P1, P2 |
| MU3 | `follower.rs` `face_force`: the traction sign flipped (`t = +p c`) | P1 (`e_F < 0`), P2, P3 |
| MU4 | `follower.rs` `positions`: the displacement ignored (a dead pressure on the reference faces) | P3 |
| MU5 | `mat_hyper.rs`: the volumetric part of the stress halved (tangent unchanged) | A3 (or the solves of A1 and P1 do not converge) |

A mutation is caught if any registered check of `sa_hyper_lock.rs` misses (a solve that fails misses its checks).

## Decision rule

- **done**: A1 met in its single registered run (f1 by option (a)), A2, A3, P1 to P3 met, S1 to S3 met, every planned
  mutation caught by a registered check.
- **partial**: A1 met, and a companion (A2, A3, P1 to P3) missed (a `*_known_miss` test with the limit intact, diagnosed), or a
  mutation not caught.
- **todo**: A1 missed (the Full solve to `1.10` fails or its error is below `0.1`), or S1 or S2 fails.

DX does not enter the rule.

## Limits (not covered)

- The locking control is measured at `l_a = 1.10` (displacement) and at the pressure `p_c(1.5)` (where the Full model reaches
  `l_a ~ 1.01`), not at the acceptance stretch `1.5` of F-425: by arithmetic 1, the discrete Full model on this mesh is not
  expected to have a stable state beyond about `1.133`, whatever the control.
- The arc-length step is not used (arithmetic 4); tracing the follower-pressure path through the pressure maximum near `l_a =
  1.83` (BBar) needs the follower load direction in the arc-length step (open question 2).
- One mesh; no mesh refinement of the locking.
- F-425's small-strain clause (SS1) is not touched.

## Open questions (with the recommendation)

1. **F-425's status.** With A1 met, the locking control of F-425 (SPL, not measured in ADR 0541) is measured at a converging
   set-up; F-425 stays partial on SS1 (the small-strain clause) unless the maintainer amends it to option (b) of ADR 0541 open
   question 1. Recommend the lead adds "locking control measured by F-622 (ADR 0557)" to the F-425 note; the amendment is the
   maintainer's call (not taken).
2. **The arc-length step with a follower load direction.** Options: (a) leave it (F-622 does not need it); (b) an opt-in
   `Contribution::load_derivative` (the derivative of the contribution's force with respect to the load factor) taken into
   `du_f` by `arclength::trace`, defaults bit-identical, checked on the sphere through its pressure maximum at `l_a = 1.83`.
   Recommend (b) as a follow-up.
