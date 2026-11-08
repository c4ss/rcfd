# ADR 0223: F-301 registration: consistent PISO correctors: the pressure-lag estimate on a moving mesh, a case key and the benchmark set

Status: **REGISTERED 2026-11-08**, written before any code of F-301 and before any rcfd run of its checks; results in the addendum.
Not yet accepted by the maintainer.

Parents: [ADR 0214](0214-f299-registration-piso-instability-at-large-cell-diffusion-number-across-a-thin-gap.md) (F-299: the
mechanism H-RC, the opt-in `PimpleSettings::consistent_correctors`, its refusals and its open questions 1 and 3 to 7, which define
this item), [ADR 0073](0073-wave34b-transient-floor-pimple-lag.md) (the pressure-lag model R5 and its run-time estimate B4, the
TG-S set-up and the B2 to B4 harness), [ADR 0069](0069-wave33b-transient-accuracy-coupling.md) (the temporal-order and cavity
stability harnesses and the BDF2 cylinder run), [ADR 0154](0154-job48t-cylinder-ogrid-pressure-spokes.md) (the cylinder K1 and
K2), [ADR 0156](0156-job48v-adaptive-pressure-correctors.md) and [ADR 0162](0162-job48ab-adaptive-correctors-follow-the-time-step.md)
(the adaptive corrector count and the pre-run rule SV301), [ADR 0211](0211-f244-registration-flow-coupled-six-dof-validation.md)
(the falling block FB), [ADR 0102](0102-wave41b-simplec-boundary-compensation-forward-vent.md) and [ADR 0113](0113-wave44c-mrf-cyclic-seams.md) F1 (the steady SIMPLEC compensation on
periodic faces and the periodic partner coefficient in `h1`). Roadmap item F-301 (depends on F-299, merged).

Number: the lead gave 0223 to F-301. Main 46a5f029 holds 0221 as its highest; 0222 is on `f260-cmaes`. No branch holds a 0223.

What was run before this text (disclosed): no rcfd test and no rcfd run; a release build of the unchanged main (46a5f029) in the
target directory `E:/projects/cfd/w49i-work/target-f301` (to warm it). The code was read (main 46a5f029): `crates/rcfd-fv/src/pimple.rs`,
`pimple/moving.rs`, `pressure.rs`, `simple.rs` (the SIMPLEC compensation), `bc.rs`; `crates/rcfd-api/src/sim.rs`, `schema.rs`,
`dynmesh.rs`, `rules.rs`; `crates/rcfd-core/src/settings.rs`; the harnesses `w33b_transient_order.rs`, `w33b_stability.rs`,
`w34b_lag.rs`, `piso_thin_gap_f299.rs`, `w48t_cylinder_spokes.rs`, `validation_cylinder_re100_w33b.rs`, `w48v_adaptive_correctors.rs`,
`case_hashes_w48u.rs`; the Studio tests `correctors_x.rs`, `editor_b5.rs`. Every number quoted below from an earlier ADR is quoted,
not measured here.

House rules (ADR 0207's, ADR 0211's and ADR 0214's):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the measured number recorded, and the cause is
  diagnosed, not tuned away;
- a defective set-up gets a new corrected test by an amendment written before the corrected run, and the old one stays as an
  ignored record;
- every new code path is opt-in (four new `PimpleSettings` fields, all `false` by default, and an optional case table): a run
  without them runs as before, bit for bit (N2);
- `crates/rcfd-api/tests/baseline_hashes.rs`, `case_hashes_w48u.rs` and the goldens are not edited or re-recorded, and no
  existing test file is edited (the set-ups of earlier harnesses are copied into the new test files). If adding the case key
  changes a schema golden or any existing test (K0), the key is not added: the golden edit becomes an open question for the
  maintainer and the work stays at library level;
- each registered run is made once, and nothing is re-run to pick a result (N2 is a registered re-run for identity only);
- a change that the default path would need is not made: it is an open question with a recommendation;
- no new third-party crate, no commit trailers, unsigned commits.

Clean-room: everything below is derived from the discrete equations of the solver as read in the repository, ADR 0214's algebra
(the SIMPLEC split of Van Doormaal and Raithby, 1984, as rcfd's steady solver already applies it), ADR 0102's and ADR 0113's
periodic-face treatment and ADR 0073's pressure-lag map. No source outside the repository was read for this work, and no vendor
code, text or install was used.

## What F-301 asks

The roadmap entry (acceptance, quoted in full in `docs/roadmap.json`): (a) the lag estimate of ADR 0073 defined on a moving mesh per
step, with the pre-run warning and adaptive correctors connected to it (ADR 0214 Q3); (b) a case key (for example
`solver.consistent_correctors`) with the schema golden edit registered, and the LagCheck documentation and Raise message
corrected (Q4, Q7); (c) a registered benchmark set (the ADR 0069 and 0073 harnesses, the ADR 0154 cylinder K1 and K2, the Studio
examples) run with the option, and only then a decision on the default (Q1: the default stays false until then); defaults
bit-identical. The note adds ADR 0214 Q5: extend the refused combinations one at a time, each with a steady-state identity check.

**Reading of the acceptance (registered here).**

- (a) "Defined on a moving mesh per step" is read as: on a moving-mesh step the estimate is made on that step's own frozen
  matrices (the moved geometry, the relative flux, the step's wall conditions made homogeneous), so it differs from step to step
  as the mesh moves, and it predicts that step's own growth (MA2). "Connected" is read at two levels. At library level: the
  run-time check, the adaptive count and a pre-run estimate of the first moving step use it (MA1 to MA4). At case level (the
  rule SV301 and `solver.corrector_control = "adaptive"` for `dynamic_mesh` cases): this registration does **not** deliver it,
  because the case path refuses the adaptive count with a dynamic mesh and that refusal is asserted by ADR 0156's registered test
  `w48v_a9b_adaptive_is_refused_where_the_kernel_has_no_piso_correctors` (and the Studio's scope rule by `correctors_x.rs`); lifting
  it means editing those registered tests, which is the maintainer's call (Q2). SV301's remedy is the adaptive count, so SV301 for
  moving meshes waits for the same decision.
- (b) The key is an optional table (K below), which keeps every earlier case's written text and hash; whether that changes any
  golden is checked first (K0). The LagCheck documentation and the Raise message are corrected (E4).
- (c) The benchmark set is run with the option and reported; the decision on the default is the maintainer's (Q1). The set's
  outcomes are scored against registered **default-decision criteria** DC1 to DC8. These criteria inform Q1; they are not checks
  of F-301's deliverables, and a miss of one is a finding about the option (kept as an ignored known-miss test with its limit
  intact), not a defect of this item. Whether the benchmark set ran completely is a deliverable check (BOc to BXc).
- Q5: periodic faces and body forces are the two refused combinations the benchmark set needs (Taylor-Green and the B2 box are
  periodic, MMS has a body force). They are extended one at a time (E2, E3), each with its steady-state identity check. Because
  ADR 0214's registered test `r1_refusals_of_the_consistent_correctors` asserts that `consistent_correctors` alone refuses them, each
  extension is behind its own flag (default `false`); merging them into the main flag needs that record changed (Q3). The case key
  sets all three.

## What the code read shows (main 46a5f029, read only)

1. **The moving-mesh step** (`pimple/moving.rs`): `step_ale_with_walls` and `step_sliding` set `self.ale` and call `step`; in
   `step_with` the pressure-lag check is due only when `self.ale.is_none()` and the adaptive count is used only when
   `self.ale.is_none()` (`lag_due`, `cc`). The lag map (`lag_map`, `lag_amplification_n`) works on `self.geom` and the frozen matrices
   of the final outer iteration, with homogeneous pressure conditions (`b`, `d` zeroed on non-periodic faces), no carry-over and no
   body force. During an ALE step `self.geom` is the moved geometry, the momentum matrices carry the relative flux and the
   transient term's new volume (the old volume enters the source only), the wall gradients of `apply_wall_gradients` are boundary
   data (zeroed by the lag map) and `add_wall_flux` adds a source. So the existing lag map, given the ALE step's frozen matrices, is
   the linearised pressure-lag map of that step; only the two exclusions keep it from being made.
2. **The case path** (`sim.rs`): `pressure_lag_check` is set for `n_correctors == 1` without the adaptive count, whatever the
   mesh, so moving-mesh cases with one corrector carry the check today and the ALE exclusion keeps it from acting; removing the
   exclusion unconditionally would change what those runs report (and stop them with `raise`). Hence the library flag E0.
   `corrector_control = "adaptive"` with `dynamic_mesh` is a located error; SV301 (`lag_watch_solver`) excludes dynamic meshes.
3. **The consistent split** (`consistent_split`): the compensation on internal faces and on boundary faces whose flux the
   pressure sets, skipping periodic faces; `consistent_conflict` refuses periodic faces, body forces, face jumps, non-orthogonal
   correctors, the component diagonal and the fractional step. `h1` sums `off_diag_abs_sum()` of the momentum matrices, which does
   not include the implicit periodic links (`PeriodicLinks::t`).
4. **Periodic faces in the pressure equation** (`pressure.rs`): `solve_pressure` and `corrected_flux` use `coupled_gamma` (the
   distance-weighted pair mean of the face coefficient) and the partner-cell difference over the summed distances; the steady
   SIMPLEC's boundary compensation (`simplec_boundary_flux_correction`, ADR 0102) uses `coupled_gamma(interp(rAtU - rAU))` with the
   same difference.
5. **Body forces** (`step_with`): the balanced-force term `gamma_p b_f` is added to `phiHbyA` on every non-empty face; on
   fixed-velocity faces the pressure condition becomes the gradient `b_f/|S|`, whose Neumann term in the pressure equation carries
   the equation's own face coefficient and cancels the added term only if both coefficients are equal; the cell velocity gets
   `rAU b`.
6. **The schema** (`rcfd-core` `Schema::optional`): only an object can be optional; an absent optional object stays out of the
   data, the written text and the hash, and `get` of a path into it gives the defaults. A scalar key, even with
   `hash_omit_default`, is written into every case's text (`case_hashes_w48u.rs` normalises a fixed list of such keys and cannot
   be edited here).

## Built (specified now; every item opt-in, default off)

**E0. `PimpleSettings::moving_mesh_lag: bool`** (default `false`, set by `piso` and therefore by `pimple`). With it a moving-mesh
step (`step_ale`, `step_ale_with_walls`, `step_sliding`) makes the ADR 0073 estimate of `pressure_lag_check` and the ADR 0156
adaptive count of `corrector_control` exactly as a fixed-mesh step does, on its own frozen matrices (item 1). Without it a moving
step makes neither (as before).

**E1. `PimpleSolver::lag_preflight_moving(&self, dt, iterations, new_geom, mesh_flux, wall_dpdn) -> Option<LagWarning>`**: the
pre-run estimate of the next step taken on a moving mesh: a copy of the solver (the solver is not changed) with `moving_mesh_lag`
on, the adaptive count off and the check (`Warn`) at the next step takes `step_ale_with_walls` with the given data and returns that
step's estimate; `None` for the fractional step, a stopped run, or a step that fails. The caller sets the moving walls' velocity
conditions (`set_follow_conditions`) on the solver first, as before a real step.

**E2. `PimpleSettings::consistent_periodic: bool`** (default `false`). With `consistent_correctors` and it, periodic (coupled)
pressure faces are accepted: on each coupled face the compensation `coupled_gamma(interp(rAtU - rAU))_f |S| (p_prev[own q] -
p_prev[own f]) / (dn_f + dn_q)` (ADR 0102's form), and the magnitudes of the implicit periodic link coefficients counted in `h1` of
their row (ADR 0113 F1's rule: a seam cell gets the `rAtU` of the same cell of the unsplit mesh), in the step and in the lag map by
one shared helper. At a converged state the face flux is the default's on every coupled face (`coupled_gamma` and `interp` are
linear). Without it: refused as in ADR 0214.

**E3. `PimpleSettings::consistent_body_force: bool`** (default `false`). With `consistent_correctors` and it, body forces
(`body_force`, and the hook's `source_force` in every outer iteration) are accepted: the body-force term of `phiHbyA` keeps
`interp(rAU)` on every face except the fixed-velocity, non-periodic boundary faces, where it takes `interp(rAtU)`, the coefficient
of the pressure equation's Neumann term there (item 5; as `add_wall_flux` of a moving wall under the option, ADR 0214 F0). The cell
velocity keeps `rAU b`. At a converged state every face flux, the pressure equation and the cell velocity are the default's.
Without it: refused as in ADR 0214. Face jumps stay refused.

**E4. The LagCheck documentation and the Raise message (Q7).** The documentation of `LagCheck` no longer says that a second corrector
removes the lag: it says that it removes the lag of smooth pressure errors (the ADR 0073 model) but not of short pressure waves
across thin cells on a collocated mesh (ADR 0214 M2), that the check is made with the corrector count the step uses, and that the
consistent correctors remove that instability. The Raise message drops "with one pressure corrector" and reads, after the located
part, `the run will diverge: use more pressure correctors (solver.n_correctors >= 2 removes the lag of smooth pressure errors;
across thin cells short pressure waves need more, ADR 0214), the consistent correctors (<key>, ADR 0214) or a smaller solver.dt`,
with `<key>` = `solver.consistent_correctors.enabled` if the key is added (K0), else `PimpleSettings::consistent_correctors`. The
existing assertions on it (`w34b_lag` B4d: "pressure-lag check", "n_correctors >= 2", "cell"; `w35b_lag_check`) keep holding. The
`rcfd-api` warning text of a one-corrector run is not changed.

**E5. The case key** (only if K0 holds): an optional table `[solver.consistent_correctors]` with one field `enabled` (boolean,
default `false`, `hash_omit_default`; the table `hash_omit_default` too). `enabled = true` sets `consistent_correctors`,
`consistent_periodic` and `consistent_body_force` on the transient solver; the moving-mesh step takes it. Located errors (path
`solver.consistent_correctors.enabled`) with `enabled = true` for: an engine other than the incompressible transient
`piso`/`pimple` one (steady algorithms, compressible, variable density, VOF), `time_advance = "fractional_step"`,
`non_orthogonal_correctors > 0`, `component_diagonal = "corrected"`, and lumped components (their face jumps). Allowed: dynamic
meshes, periodic patches, body forces (`physics.body_force`, buoyancy, sources), turbulence on `pimple`, the adaptive count
(whose estimate then has the option's algebra, ADR 0214 F3). Documented in `docs/case-schema.md`.

## Set-ups

**FB, FX, TW** are those of ADR 0214 (the falling-block mesh of ADR 0211, `fb_settings`, the fixed-mesh reproduction FX, the
twin-run power iteration TW), copied into the new test file `crates/rcfd-fv/tests/consistent_correctors_f301.rs`; the copy is
asserted to give ADR 0211's 12 320 cells and grid. **FB coupled** is ADR 0211's coupled loop (RK4 body, Laplacian smoothing of the
unit displacement, follow conditions, `step_ale_with_walls`), with the settings as a parameter and a variant that records a failure
(a mesh error, a non-finite velocity, a guard trip) instead of panicking. `dt = tau/10` everywhere below unless stated.

**TW-ALE.** TW of ADR 0214 on a moving step: the base state `S` after `n_b = 5` FB-coupled steps; the motion data of step 6 (the
body advanced once with `S`'s force, the moved mesh, the mesh flux, the follow conditions and the wall gradients) computed once
from `S` and used for every iteration; each iteration restores `S` (its fields and histories, and the solver's geometry set back to
`S`'s), perturbs it as TW does, and takes the ALE step with the data; the base step from `S` is subtracted. Norm, start direction,
`K = 60`, the estimate `|rho|` and its sign as in TW. The estimate **E-ALE** is made on the base step: `moving_mesh_lag` on and
`LagCheck { every: 0, start: 6, iterations: 20, action: Warn }`, then the check is turned off for the iterations.

**SPC (steady periodic channel).** `box_2d_graded(20, 12, 0.3, 0.1, [Uniform, Symmetric(3.0)], true, false)` (periodic in x);
`ymin` fixed velocity zero; `ymax` fixed velocity per face `(U0, V0 sin(2 pi x_f / 0.3), 0)`, `U0 = 0.1`, `V0 = 0.02` (zero net
flux); `nu = 0.01`, `fb_settings`; `dt = 0.002`; from rest; no pressure Dirichlet condition.

**SPX (one and two periods).** SPC (box 1) and the same channel of two periods, `box_2d_graded(40, 12, 0.6, 0.1, ...)` (box 2),
with the same wall profile function; from rest, 20 steps. Every cell of box 2 is matched with the cell of box 1 at the same `y`
and `x mod 0.3` (centres within 1e-12); pressures are compared with each box's volume mean removed.

**SBC (steady body-force box).** `box_2d_graded(20, 12, 0.3, 0.1, [Symmetric(3.0), Symmetric(3.0)], false, false)`, all walls
fixed velocity zero, the body force `f(x) = (A sin(pi y / 0.1), -A, 0)`, `A = 0.1` (cell values at the centres, face values
`f(x_f) . S_f` on every non-empty face, boundary faces included), `nu = 0.01`, `fb_settings`, `dt = 0.002`, from rest.

**The option C** in the benchmark set: `consistent_correctors`, `consistent_periodic` and `consistent_body_force` all `true` (what
the case key sets). **D** is the default.

## Registered checks (deliverables)

Files: `crates/rcfd-fv/tests/consistent_correctors_f301.rs` (MA, S, R2, MSG1, N1), `consistent_defaults_f301.rs` (N2, the
existing API only), `consistent_benchmarks_f301.rs` (BO, BF, BS, BB); `crates/rcfd-api/tests/consistent_correctors_case_f301.rs` (C,
BK, BY, BX), `consistent_defaults_case_f301.rs` (N2). Release; the heavy ones carry `#[cfg_attr(debug_assertions, ignore = ...)]`
(gated in release) or, above the 5 minute guideline, `#[ignore]` (run once here and reported).

### MA, the estimate on a moving mesh (a)

| Id | Check | Limit |
| --- | --- | --- |
| MA1 | FX, step 1, default correctors, check `Warn` at step 1, `moving_mesh_lag` on: the step taken as `step_ale_with_walls` with the same geometry, zero mesh flux and no wall gradients, against the same step taken by `step` | both estimate at step 1; `abs(rho_ALE - rho) <= 1e-9 rho`, the same cell; fields after the step within `1e-12 U_t` (velocity) and `1e-12 max|p|` |
| MA1n (negative control) | the ALE step of MA1 without `moving_mesh_lag` | no estimate (as before); its fields bit-identical to MA1's ALE step (the estimate does not change the solution) |
| MA2s (set-up) | the 5 FB-coupled base steps of TW-ALE, default | bounded (finite, no guard trip, `max|u| <= 10 U_t`) |
| MA2 | TW-ALE with the default algebra; E-ALE | `|rho_TW|` within 10 % of ADR 0214 D1's 3.15959 (the moving mesh plays no part) and alternating (`s <= -0.5`); `abs(E-ALE - |rho_TW|) <= 0.10 |rho_TW|` |
| MA2c | TW-ALE and E-ALE with `consistent_correctors` (base run with it too) | `|rho_TW| < 1`; `E-ALE < 1` |
| MA3 | FB coupled, 100 steps, default algebra, `corrector_control = Some(CorrectorControl::registered())`, `moving_mesh_lag` on | (i) an estimate at step 1, not cap-bound, count `>= 3`, its `rho <= 0.5`; (ii) 100 steps without a failure; (iii) ADR 0211 A1 `abs(U(10 tau)/U_t - 1) <= 0.02` and A3 continuity `<= 1e-10` at every step |
| MA3n (negative control) | MA3 without `moving_mesh_lag` | the corrector record stays empty (no estimate, no step counted) and the run fails within 100 steps (the known miss of F-244) |
| MA4 | FB coupled from rest, default: `lag_preflight_moving` with step 1's motion data, then the real step 1 with `moving_mesh_lag` and the check `Warn` at step 1 | the preflight returns step 1; its `rho` and cell equal the real step's bit for bit; the solver's `u`, `p`, `phi`, time and step count bit-identical before and after the preflight; `rho` within 10 % of ADR 0214 L1's 3.30207 |

Printed, no limit: E-ALE and TW-ALE against ADR 0214's fixed-mesh numbers; MA3's record (counts, `rho`, triggers, mean count) and
pressure iterations against ADR 0214 A1c's 2434; an estimate of MA3 later in the run, if the monitor makes one.

### S and R2, the extensions (Q5)

| Id | Check | Limit |
| --- | --- | --- |
| S0p (set-up) | SPC, 3000 default steps give `S0`; branch A: 50 more default steps | A's `max|u - u_S0| <= 2e-11 U0` |
| SPs (set-up) | `S0`: the largest pressure difference across a periodic pair | `>= 1e-2 (max p_S0 - min p_S0)` |
| S1p | branch B: 50 steps from `S0` with `consistent_correctors` and `consistent_periodic` | after every step `max|u - u_S0| <= 1e-10 U0`; `max|p - p_S0| <= 1e-10 (max p_S0 - min p_S0)` |
| SPx | SPX with C and with D, 20 steps each | C's largest matched velocity difference `<= max(1e-10 U0, 10 x D's)`, pressure `<= max(1e-10, 10 x D's)` of box 1's pressure range |
| S0B (set-up) | SBC, 3000 default steps give `S0`; branch A: 50 default steps; `U_s = max|u_S0|` | A's `max|u - u_S0| <= 2e-11 U_s` |
| SBs (set-up) | `S0` | `U_s >= 1e-3` m/s |
| S1B | branch B: 50 steps from `S0` with `consistent_correctors` and `consistent_body_force` (the static `body_force`) | after every step `max|u - u_S0| <= 1e-10 U_s`; `max|p - p_S0| <= 1e-10` of the range |
| S1Bh | branch B' : the same with the force supplied by the hook (`source_force`) and `body_force = None` | S1B's limits |
| R2 | ADR 0214's 2-D box (`box_solver`), one step each: (i) `consistent_correctors` alone: periodic faces and body forces (static and hook) refused as in ADR 0214 R1; (ii) with `consistent_periodic`: the periodic box accepted; (iii) with `consistent_body_force`: the static and the hook body force accepted; (iv) with both flags: the fractional step, non-orthogonal correctors, the component diagonal and a face jump refused with ADR 0214's stages; (v) the two flags without `consistent_correctors`: the periodic box with a body force, 3 steps, bit-identical to the default | (i), (iv) stopped at step 1, stage and quantity as in ADR 0214, state bit-identical, message names ADR 0214; (ii), (iii) not stopped; (v) bit-identical |

Printed: S1p and S1B branch B against branch A after 50 steps; SPx's differences of D and C.

### K and C, the case key (b)

| Id | Check | Limit |
| --- | --- | --- |
| K0 (gate of E5) | with the schema table added and nothing else of E5: `rcfd-api` tests `case_hashes_w48u`, `baseline_hashes`, `w48v_adaptive_correctors` (non-ignored), `rules_w19c`; `rcfd-cli` `cli`; `rcfd-mcp` `default_f1`, `tool_list_f275`, `project_list_f270`; `rcfd-studio` `editor_b5`, `correctors_x`; `rcfd-gui` `logic` | all pass, no file edited. If any fails because of the table: the table is removed, the golden edit becomes an open question (Q-K), C1 to C4 are not run, the Raise message names the library field, and BK and BX run with C through the library (the ADR 0154 harness replica) |
| C1 | the cylinder example (64 x 48, dt 1/16) with `solver.consistent_correctors.enabled = true` through `Simulation`, 16 steps, against ADR 0154's harness replica with C, 16 steps | `p`, `ux`, `uy` bit-identical; and different from the run without the key |
| C2 | `enabled = true` with: the quick-start cavity (simplec), the cylinder with `time_advance = "fractional_step"`, with `non_orthogonal_correctors = 1`, with `component_diagonal = "corrected"`, ADR 0156's low-Mach case (variable density), a transient case with lumped components | each a diagnostic at `solver.consistent_correctors.enabled`; each text without the key builds |
| C3 | the cylinder example: no table, and `[solver.consistent_correctors]` with `enabled = false` | equal settings content hash; the written TOML and JSON of the case without the table contain no `consistent_correctors`; `enabled = true` changes the hash |
| C4 | ADR 0156's piston case (moving `xmin`, piso) without the adaptive key, with `enabled = true` | builds; 10 steps without a guard trip; fields different from the run without the key |
| MSG1 | FX with the check in `Raise` (ADR 0214 L1's set-up, two correctors) | the message contains "pressure-lag check", "n_correctors >= 2", "cell", "short pressure waves", "ADR 0214" and `<key>`; it does not contain "with one pressure corrector" |

### B, the benchmark set with the option (c): completeness (deliverables)

| Id | Benchmark | Deliverable check |
| --- | --- | --- |
| BOc | **BO**, ADR 0069's order harness (copied, a `consistent` field in its configuration), arms D and C, each against its own reference (`dt_ref = 1/1280`): O1 (TG PISO BDF2, fine and coarse), O2 (TG PIMPLE-C), O3 (TG PIMPLE-4, fine), O5 (MODE PISO), O6 (MMS PISO and PIMPLE-C), O7 (TG and MMS PISO, variable step, fine), O8 (TG, MODE, MMS PISO, Euler, fine), O9 (TG PISO joint refinement against the exact solution), and ADR 0073's TG-S PISO and PIMPLE-4 (fine); the FRAC rows are not applicable (refused) | every C run reaches `t = 1` without a refusal or a guard trip |
| BF | prints: TG BDF2 fine-pair orders at N = 16, 32, 64 for PISO and PIMPLE-4, arms D and C | none (prints) |
| BSc | **BS**, ADR 0069's cavity stability harness (copied): the required set (PISO Co_t 0.25, 0.5, 1; PIMPLE-2 and PIMPLE-4 0.5, 1, 2, 4; both meshes, both Re, both schemes: 88 runs) in arms D and C, the scan (PISO 2, 4, 8; PIMPLE 8, 16) in arm C | every run made; set-up check **BSd**: D's verdicts equal ADR 0069's (PISO 24 of 24 stable, PIMPLE-2 28 of 32 with exactly the four Co_t 4 Re 100 runs unstable, PIMPLE-4 32 of 32) |
| BBc | **BB**, ADR 0073 Part B (copied): the B2 periodic Stokes box (Euler and BDF2, n_outer 2 and 4, D 1, 2, 3, 4, 6; estimate at step 6) and the seven B3 cavity runs, arm C | every run made (the B2 runs are periodic: E2) |
| BKc | **BK**, ADR 0154's K1 and K2 (64 x 256, dt 1/64, two and three correctors, to t = 3) with C (through the key, or the harness if K0 failed) | both runs made; M_hf computed at t = 3 or the stop recorded |
| BYc | **BY**, ADR 0069's BDF2 cylinder (128 x 100, dt 0.05, PIMPLE n_outer 2, alpha 0.9, to t = 160) with C, one run | the run made; its St, C_D and crossings recorded or its stop |
| BXc | **BX**, the Studio examples that run transient (`cylinder-re100.toml`, `cylinder-re200.toml`, 64 x 48, PISO, dt 1/16) through `Simulation` to t = 80, arms D and C | every run reaches t = 80 with at least 5 upward zero crossings of `cl` after t = 50 (St from the last four whole periods, mean `cd` over them by the trapezoid rule) |

The other Studio examples are steady (`simplec`): not applicable (the key refuses them, C2).

### DC, the default-decision criteria (scored for Q1; not deliverables)

| Id | Criterion | Limit |
| --- | --- | --- |
| DC1 | BS required set, arm C | 88 of 88 stable (ADR 0069's criteria) |
| DC2 | BB B2 box, arm C | every row: measured `g < 1` and the check's `rho < 1` |
| DC3 | BB B3, arm C | all seven runs stable |
| DC4 | BO no-harm (ADR 0073 N1's measure): exact velocity error, and pressure error after removal of the volume means, at N = 32, dt 0.05, 0.025, 0.0125 for TG, TG-S, MMS, MODE PISO BDF2 (MODE: velocity only) | every C error `<= 1.05 x` D's (21 comparisons) |
| DC5 | BO orders | every row whose D order lies in its ADR 0069 band has its C order in the band |
| DC6 | BK | K1 and K2 with C: not stopped, `t = 3` reached, `M_hf <= 0.0114` (ADR 0154's limit) |
| DC7 | BY | ADR 0069's K1b (St in [0.16005, 0.16995]), K2b (C_D in [1.2883, 1.3957]) and K4 (more than 9 crossings) with C; T1 printed |
| DC8 | BX | for each example: `abs(St_C - St_D) <= 0.01 St_D` and `abs(cd_C - cd_D) <= 0.01 cd_D` |

Printed with them: pressure iterations per step (C against D) in BX, BK, BY; the scan of BS; BF.

### N, the defaults

| Id | Check | Limit |
| --- | --- | --- |
| N1 | `PimpleSettings::piso(0.01)` and `pimple(0.01, 3)` | `consistent_correctors`, `consistent_periodic`, `consistent_body_force`, `moving_mesh_lag` all `false` |
| N2 | fingerprints (FNV-1a of the bits) printed by `consistent_defaults_f301.rs` and `consistent_defaults_case_f301.rs`, which use only the API of 46a5f029: (a) FX default at `tau/40`, 20 steps; (b) FB coupled default, 5 steps, with the check `Warn` at step 1 and `corrector_control = registered()` (the state, the number of estimates and of corrector-record steps); (c) TG PISO BDF2 N = 16, dt 0.05, 20 steps; (d) the B2 box PIMPLE-2 D = 4 Euler, 8 steps, the check every 6 (the state and each estimate); (e) the uniform cavity Re 100 PIMPLE-2 Euler Co_t 4 with the check in `Raise` (the stop's step, quantity, value bits and cell, and the state; not the message text, which E4 changes); (f) a periodic box with a body force, PISO, 5 steps; (g) the cylinder example through `Simulation`, 16 steps; (h) ADR 0156's piston case through `Simulation` with `n_correctors = 1` (the check set by the case path), 10 steps, and the number of run-time warnings; (i) the cylinder example's settings content hash and the FNV of its written TOML | the lines printed on the library of 46a5f029 (these two files committed before any library edit) equal those printed on the final tree |
| N3 | the gate: the existing suites unedited (among them `piso_thin_gap_f299` with its R1, `sixdof_flow_f244`, `ale_flows`, `w34b_lag`, `w33b_*`, the `pimple` unit tests, `rcfd-api` `w48t_cylinder_spokes`, `w48v_adaptive_correctors`, `w35b_lag_check`, `baseline_hashes`, `case_hashes_w48u`) | pass |

### Mutations (scratch, not committed)

Each is applied alone to the final tree by an exact text replacement (a script that checks that the replaced text occurs once),
the named tests are run once in release in a separate target directory and must fail, and the file is restored and compared byte
for byte with a kept copy. `scripts/limit_audit.py` is run on the new test files and every hit is read.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | `moving_mesh_lag` ignored in `step_with` (both ALE exclusions kept) | MA1, MA3, MA4 |
| MU2 | E2's coupled-face compensation dropped | S1p |
| MU3 | E3's fixed-velocity faces with `interp(rAU)` | S1B |
| MU4 | E2's periodic links not counted in `h1` (the shared helper) | SPx |
| MU5 | `lag_preflight_moving` without `moving_mesh_lag` on its copy | MA4 |
| MU6 | the case key read but not passed to the solver (if E5 exists) | C1 |
| MU7 | E4's Raise message reverted to the old text | MSG1 |

Expected: MU1 gives no estimate on a moving step; MU2 and MU3 move the fixed point by about `(rAtU - rAU)` times the seam pressure
difference or the wall body force (of order `1e-3` to `1e-2` of the velocity scale); MU4 gives the seam cells another `rAtU` than
the cells of box 2 at `x = 0.3`, which a transient step from rest shows; MU5 returns `None`.

### The gate

`cargo fmt` per package, `cargo clippy --workspace --all-targets -- -D warnings`, and every test in release on a `git archive` copy of
the gated commit, one process per test, three jobs (`scripts/gate_test.py`), in the target directory
`E:/projects/cfd/w49i-work/target-f301-gate`, through the machine's gate lock (one acquire process, the 5 s poll of
`gate-f291/outer2.sh`), started with at least 19 GB of physical memory free (disclosed with the result). A failure of a known flaky
test (earlier gate lines: `runner_w47b`, `journal_f273` c3s, `preview` g8/g10, `rcfd-mcp` `adopt_f288` "Access is denied") is run
again alone on the same build and reported with its name.

### Classes

A set-up check (MA2s, S0p, SPs, S0B, SBs, BSd) or a negative control (MA1n, MA3n) that does not hold, or a mutation that does not
make its tests fail, means a check is not shown to be able to fail: reported as a defective set-up, corrected by an amendment
before any re-run, and never counted as a pass. A miss of MA1 to MA4, S1p, SPx, S1B, S1Bh, R2, C1 to C4 or MSG1 is a miss of a
deliverable: an ignored known miss with the limit intact. A miss of a DC criterion is a finding about the option: an ignored known
miss of the option with the limit intact, reported to the maintainer for Q1.

**Decision rule.** Every deliverable check met (with every set-up check and negative control held, and every mutation failing its
tests), N1 to N3 met, and the benchmark set complete (BOc to BXc): (a) is delivered at library level, (b) as the case key (or, if K0
failed, at library level with Q-K open), (c) as the run set; F-301 is still set **partial** (`[~]`), the remaining part named: the
case-level wiring of the moving-mesh estimate (SV301 and the adaptive count for `dynamic_mesh`, Q2) and, if K0 failed, the key. A
deliverable miss: partial with the miss named (todo stays if neither (a) nor (c) is delivered). The default is the maintainer's
decision after the benchmarks (Q1); done (`[x]`) is the maintainer's call.

## Not delivered

The case-level SV301 and adaptive count on dynamic meshes (Q2); the default of the option (Q1); the option with face jumps,
non-orthogonal correctors, the component diagonal or the fractional step (still refused, Q5's remainder); BDF2 on a moving mesh
(the ALE step is implicit Euler only); a Studio control beyond the schema-driven editor; the `rcfd-api` warning text of one-corrector
runs.

## Open questions and choices for the maintainer

Registered now; their recommendations are given in the addendum with the results.

- **Q1. The default** of the option, after the benchmark set (DC1 to DC8).
- **Q2. The case-level wiring for moving meshes**: lift ADR 0156's refusal of `corrector_control = "adaptive"` with `dynamic_mesh`
  (its test A9b and the Studio's scope rule change) and give SV301 a moving-mesh preflight (E1 with the dynamic mesh's next motion).
- **Q3. One flag**: fold `consistent_periodic` and `consistent_body_force` into `consistent_correctors` (ADR 0214's R1 record
  changes).
- **Q4. Status** of F-301.
- **Q5. The remaining refused combinations** (face jumps, non-orthogonal correctors, the component diagonal, the fractional step).
- **Q-K** (only if K0 fails): the golden edit the key needs.
