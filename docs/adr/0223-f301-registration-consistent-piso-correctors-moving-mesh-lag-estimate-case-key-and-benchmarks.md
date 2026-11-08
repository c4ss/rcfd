# ADR 0223: F-301 registration: consistent PISO correctors: the pressure-lag estimate on a moving mesh, a case key and the benchmark set

Status: **REGISTERED 2026-11-08**, written before any code of F-301 and before any rcfd run of its checks; three amendments before
the runs they govern (corrected set-ups of the steady identities and of the examples' window); results in the addendum (every
deliverable met except the periodic pressure identity S1p3, a known miss; the benchmark set complete, two of its default-decision
criteria missed; F-301 partial by the decision rule). Not yet accepted by the maintainer.

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

## Amendment 1 (2026-11-08): the steady-identity set-ups of E2 and E3, and a seam check without the transient carry-over, written before the corrected runs

**What happened.** The N2 tests were printed on the library of 46a5f029 (commits 0bc419aa, 185ea729, test code only). Then E0 to E4
were written and the tests of `consistent_correctors_f301.rs` (MA, S, R2, MSG1, N1) were run once, in release, on that code; their
numbers go in the addendum and none is repeated. Two set-ups did not resolve their limits:

- **S0p / S1p.** S0p (velocity drift of branch A, 2.09e-12 U0) and SPs (0.157) held. S1p's velocity part was met (1.88e-12 U0), its
  pressure part missed (4.31e-10 of the range against 1e-10). The default's own branch A moved its pressure from `S0` by 5.78e-10 of
  the range in the same 50 steps (a common level drift of a problem without a pressure Dirichlet condition, at the solvers' floor),
  which S0p did not check: `S0` is not a fixed point of the default at the 1e-10 pressure level, so S1p's pressure limit measured the
  floor, not the identity. B against A after 50 steps: 6.0e-13 U0 and 3.6e-11 of the range.
- **S0B / SBs / S1B.** The flow of SBC is weaker than assumed (`U_s` 4.36e-4 m/s against SBs's 1e-3), and branch A's drift is 8.54e-10
  `U_s` (3.7e-13 m/s, the solvers' absolute floor; S0B's limit 2e-11 `U_s`). Both set-up checks failed, so S1B and S1Bh (7.31e-10 `U_s`,
  2.99e-10 of the range, below A's own drift) are not counted.
- **SPx** was met (consistent 6.08e-4 U0, default 6.06e-4 U0, limit ten times the default's), but the default's one-period and
  two-period runs differ by 6e-4 U0: the transient Rhie-Chow carry-over is added on internal faces only (ADR 0073 R3), so a seam face
  differs from the internal face of the two-period box, and that difference hides the effect SPx is meant to see.

**Corrected set-ups (registered now, before their runs).**

- **S-p2** (test `s_p2_steady_state_on_periodic_faces_against_the_default_branch`): SPC and `S0` as before (3000 default steps).
  Branches A (default) and B (`consistent_correctors`, `consistent_periodic`) run side by side from `S0` for 50 steps (two solvers
  restored from the same snapshot). **S0p2** (set-up): A's `max|u - u_S0|` over the 50 steps `<= 2e-11 U0`. SPs unchanged. **S1p2**:
  after every step `max|u_B - u_A| <= 1e-10 U0` and `max|p_B - p_A| <= 1e-10 (max p_S0 - min p_S0)` (S1p's limits, with the
  default's own trajectory as the reference, so that a common drift of the floor cancels). Printed: A's and B's deviations from `S0`.
- **S-B2** (test `s_b2_steady_state_with_a_body_force_against_the_default_branch`): SBC with `A = 10` (so that the flow is far above
  the floor: prediction `U_s` about 0.044 m/s by linear scaling of the measured 4.36e-4 m/s at `A = 0.1`; Reynolds number about 0.4),
  3000 default steps give `S0`; branches A (default), B (`consistent_correctors`, `consistent_body_force`, the static force) and B'
  (the same flags, the force from the hook) side by side from `S0`, 50 steps. **S0B2** (set-up): A's `max|u - u_S0| <= 2e-11 U_s`.
  **SBs2** (set-up): `U_s >= 1e-3` m/s. **S1B2** and **S1Bh2**: after every step, B and B' against A: `max|u - u_A| <= 1e-10 U_s`,
  `max|p - p_A| <= 1e-10` of the range of `p_S0`.
- **SPx2** (test `spx2_seam_cells_behave_as_interior_cells_without_the_carry_over`): SPX as registered with `ddt_correction = false`
  in both arms (no carry-over on any face, so the default's seam faces are the internal faces of the two-period box up to the
  solvers). Limits as SPx: C's matched velocity difference `<= max(1e-10 U0, 10 x D's)`, pressure `<= max(1e-10, 10 x D's)` of box 1's
  range. SPx stays registered and is reported as met.
- Mutations: MU2 must fail S1p2, MU3 must fail S1B2, MU4 must fail SPx2 (SPx reported with it).
- The registered S tests stay as the ignored records `s0p_sps_s1p_steady_state_on_periodic_faces_defective_setup` and
  `s0b_sbs_s1b_s1bh_steady_state_with_a_body_force_defective_setup`; their bodies are unchanged.

## Amendment 2 (2026-11-08): the pressure level in the periodic identity, written before the corrected run

**What happened.** The three corrected tests of amendment 1 were run once (commit after amendment 1). SPx2 was met (consistent
1.28e-12 U0 and 3.68e-11 of the range, default 8.18e-13 and 3.75e-11). S-B2: S0B2 (7.77e-12 `U_s`) and SBs2 (`U_s` 0.0436 m/s) held,
S1B2 and S1Bh2 were met (6.56e-12 `U_s`, 6.14e-12 of the range). S-p2: S0p2 (2.09e-12 U0) and SPs (0.157) held, S1p2's velocity part
was met (1.65e-12 U0) and its pressure part missed: `max|p_B - p_A|` over the steps 4.19e-10 of the range (1.1e-11 m^2/s^2) against
1e-10. SPC has no pressure Dirichlet condition: its level is fixed only by the reference-cell pin of the pressure solve, and the
level is the near-null mode the solver converges last, so each branch's level wanders at that size from step to step (A from `S0`:
5.78e-10 of the range), independently of the corrector algebra. Amendment 1's comparison kept the level; SPX's comparison already
removed it. The pressure level is not part of the steady state of such a problem (ADR 0214 M3 removed it the same way).

**Corrected set-up (registered now, before its run).** **S-p3** (test `s_p3_steady_state_on_periodic_faces_without_the_pressure_level`):
S-p2 exactly, with each branch's pressure taken with its volume mean removed in the pressure comparisons (B against A and both against
`S0`, whose mean is removed too). S0p3 = S0p2 and SPs (set-up), S1p3: after every step `max|u_B - u_A| <= 1e-10 U0` and
`max|p'_B - p'_A| <= 1e-10 (max p_S0 - min p_S0)`, `p'` the mean-removed pressure. Printed: the raw-pressure numbers too. MU2 must fail
S1p3. S-p2 stays as the ignored record `s_p2_..._defective_setup`; its body is unchanged.

## Amendment 3 (2026-11-08): the window of the Studio examples, written before the corrected run

**What happened.** The benchmark runs of `consistent_correctors_case_f301.rs` were started once (BK, BY, BX). BX's four runs reached
t = 80 unstopped, but `cylinder-re100.toml` has only 4 upward zero crossings of `cl` after t = 50 in both arms (D and C), so BXc's
"at least 5 crossings after t = 50" does not hold for either arm and St and the mean `cd` cannot be formed for Re 100; DC8 is then
not evaluable. At St about 0.16 the registered window (t 50 to 80) holds 4.8 periods, so whether it contains 5 upward crossings
depends on the phase: the window was too short for this example, in both arms alike (a defective set-up, not a property of the
option). `cylinder-re200.toml` had 5 crossings in both arms.

**Corrected set-up (registered now, before its run).** **BX2** (tests `bxc2_...` and `dc8b_...`): the same four runs, with the window
from t = 40 instead of 50 (the example's own comment: the start sheds within about 40 D/U): the upward zero crossings of `cl` minus its
mean over t >= 40, St from the last four whole periods (the last 5 crossings), the mean `cd` over them by the trapezoid rule.
**BXc2**: every run reaches t = 80 unstopped with at least 5 crossings after t = 40. **DC8b**: DC8's limits (1 % on St and on the mean
`cd`, both examples). The registered tests stay as the ignored records `bxc_..._defective_setup` and `dc8_..._defective_setup`.

## Addendum 1: what was built and the results (2026-11-08)

Measured against the limits registered above and in amendments 1 to 3; no limit was changed. Release builds on Windows, target
directory `E:/projects/cfd/w49i-work/target-f301` (the mutations in `target-f301-mut`). The registration is 32d6670d; the N2
fingerprints were printed on the library of 46a5f029 (0bc419aa, 185ea729: test code only); E0 to E4 and the MA, S, R2, MSG1, N1
tests are 8a45de9e (run 1); amendment 1 and its tests d3ae30c7, 54d612d2; amendment 2 and S-p3 4f33bad7, 513ff218; K0 ran on the
schema table alone (the tree of ac2f6f78 with `schema.rs` changed), then E5 and the benchmark tests 43384eea, a19acf2c (compile
fixes before any run), the benchmark runs on a19acf2c; amendment 3 and BX2 6f258aa0, 10ac24dc. Each registered run was made once;
the runs are deterministic and none was repeated to change a result.

**Built.** `crates/rcfd-fv/src/pimple.rs`: `PimpleSettings::moving_mesh_lag`, `consistent_periodic`, `consistent_body_force` (all
`false`, set by `piso` and so by `pimple`); the ALE exclusions of the lag check and of the adaptive count kept unless
`moving_mesh_lag` (E0); `PimpleSolver::lag_preflight_moving` (E1); the coupled-face compensation in `consistent_split` and the shared
helper `add_consistent_h1`, which counts the periodic link coefficients in `H1` in the step and in the lag map (E2); the body-force
face term with `interp(rAtU)` on fixed-velocity faces under the option (E3); the periodic and body-force refusals only without their
flags; the LagCheck documentation and the Raise message (E4). `crates/rcfd-api/src/schema.rs`: the optional table
`[solver.consistent_correctors]` with `enabled` (both `hash_omit_default`); `sim.rs`: the key sets the three library flags, and its
located errors (E5). Tests: `crates/rcfd-fv/tests/consistent_correctors_f301.rs` (MA, S, R2, MSG1, N1 and the ignored records),
`consistent_defaults_f301.rs` (N2), `consistent_benchmarks_f301.rs` (BO, BF, BS, BB, DC1 to DC5);
`crates/rcfd-api/tests/consistent_correctors_case_f301.rs` (C1 to C4, BK, BY, BX, DC6 to DC8), `consistent_defaults_case_f301.rs`
(N2). Documents: `docs/case-schema.md`, `docs/dynamic-mesh.md`, the F-301 entries of `docs/ROADMAP.md` and `docs/roadmap.json`. No
existing test file, golden, `baseline_hashes.rs` or `case_hashes_w48u.rs` was touched.

### Results: the deliverables

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| MA1 moving step that does not move | same estimate (1e-9), same cell, fields 1e-12 | rho 3.30206784 both, cell 5583 both, fields bit-identical | met |
| MA1n (negative control) | no estimate without the flag; fields bit-identical | none; bit-identical | held |
| MA2s (set-up) | 5 coupled base steps bounded | bounded, `max|u|` at most 1.293 `U_t` | held |
| MA2 TW-ALE, default | `|rho|` within 10 % of 3.15959, alternating; E-ALE within 10 % of it | 3.15736 (-0.07 %), sign -1.000; E-ALE 3.29454 (+4.3 %), cell 5583 | met |
| MA2c TW-ALE, consistent | both `< 1` | 0.91985 (sign +1); E-ALE 0.91699 | met |
| MA3 adaptive count on the moving mesh | step-1 count `>= 3`, `rho <= 0.5`, not cap-bound; 100 steps; A1 0.02; A3 1e-10 | count 8, rho 0.4572 (7 evaluations); 100 steps (one estimate, the count stayed 8); `U(10 tau)/U_t` 1.00547; continuity 5.7e-12 | met |
| MA3n (negative control) | record empty; the run fails within 100 steps | record empty; step 30: "mesh error: cell 0 has non-positive volume" | held |
| MA4 pre-run estimate of the moving step | the step's own, bit for bit; solver unchanged; within 10 % of 3.30207 | 3.30176712 at cell 5583, bit-identical; unchanged; -0.009 % | met |
| S0p, SPs, S1p as registered | | S0p 2.09e-12 U0, SPs 0.157; S1p 1.88e-12 U0 and 4.31e-10 of the range | defective set-up (amendment 1), ignored record |
| S0p2, SPs, S1p2 (amendment 1) | | S0p2 2.09e-12 U0; S1p2 velocity 1.65e-12 U0, pressure 4.19e-10 | defective set-up (amendment 2), ignored record |
| S0p3, SPs (set-up, amendment 2) | `2e-11 U0`; `>= 1e-2` | 2.09e-12 U0; 0.157 | held |
| S1p3 (amendment 2) | velocity `1e-10 U0`, mean-removed pressure `1e-10` of the range | velocity 1.65e-12 U0 (met); pressure 2.49e-10 (A's own mean-removed drift from `S0`: 3.32e-10) | **known miss** (`s_p3_..._known_miss`) |
| SPx | C `<= max(1e-10 U0, 10 x D)` | D 6.06e-4 U0 / 1.29e-2; C 6.08e-4 / 5.51e-3 | met |
| SPx2 (amendment 1) | as SPx | D 8.18e-13 U0 / 3.75e-11; C 1.28e-12 / 3.68e-11 | met |
| S0B, SBs, S1B, S1Bh as registered | | `U_s` 4.36e-4 m/s, A's drift 8.5e-10 `U_s` | defective set-up (amendment 1), ignored record |
| S0B2, SBs2 (set-up, amendment 1) | `2e-11 U_s`; `>= 1e-3` m/s | 7.77e-12 `U_s`; 0.0436 m/s | held |
| S1B2, S1Bh2 (amendment 1) | `1e-10 U_s`, `1e-10` of the range | 6.56e-12 `U_s`, 6.14e-12 (both) | met |
| R2 refusals and acceptances | (i) to (v) | all as registered | met |
| K0 (gate of E5) | the listed tests pass, no edit | all pass (the Studio's `correctors_x` after building `rcfd-runner` in this target directory, an environment step) | held |
| C1 the key is the library option | bit-identical to the replica with C; acts | `p`, `ux`, `uy` bit-identical; different from the run without the key | met |
| C2 located errors | six arms; each builds without the key | all six at `solver.consistent_correctors.enabled` | met |
| C3 the default of the key | equal hash; no key in the written text; `true` changes it | 6ac54a8827aedab0 both; absent; 2f289c1f9f0fa3ea | met |
| C4 dynamic mesh with the key | builds, 10 steps, acts | yes | met |
| MSG1 Raise message | the parts present, the old text absent | as registered (the text is in the test output) | met |
| BOc | every C run of BO completes | 18 rows, O9 and the 24 DC4 runs: none stopped | met |
| BSc, BSd | 232 runs, none refused; D's required verdicts those of ADR 0069 | 232, none refused; D unstable on exactly the four PIMPLE-2 Co_t 4 Re 100 runs, with ADR 0069's first violations to the printed digits (max|u| 2.138 at t 0.750, 1.536 at 2.125, max|p| 55.487 at 0.227, 91.630 at 0.094) | met, held |
| BBc | 20 B2 rows, 7 B3 runs | made, none stopped | met |
| BKc, BYc | the runs made | made | met |
| BXc as registered | | Re 100: 4 crossings after t = 50 in both arms | defective set-up (amendment 3), ignored record |
| BXc2 (amendment 3) | 4 runs to t = 80, `>= 5` crossings after t = 40 | Re 100: 6 crossings, Re 200: 7, all four runs | met |
| N1 | four flags `false` | `false` | met |
| N2 | the nine lines equal on 46a5f029 and on the final tree | equal (a 2437c2441ec5c618 ... i 6ac54a8827aedab0 a44cabd4832f4930) | met |
| N3 | the gate | see the gate section | |

The D rows of BO reproduce ADR 0069 and ADR 0073 to the printed digits (O1 1.624 and 2.290, O2 1.223 and 1.587, O3 1.031, O5 2.016
and 2.070, O6 1.939 and 1.605, 1.738 and 2.834, O7 1.832 and 1.902, O8 1.079, 1.040, 1.060, O9 3.814 and 3.254, TG-S 0.975 and
0.823), so the copied harness is ADR 0069's.

### Results: the benchmark set (the default-decision criteria, for Q1)

| Criterion | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| DC1 cavity required set with C | 88 of 88 stable | 88 of 88 (the default: 84 of 88); the scan with C: 56 of 56 stable, PISO to Co_t 8 and PIMPLE to Co_t 16 at Re 100 and 1000 on both meshes (the default is unstable on about 20 of these, ADR 0073 B4b) | met |
| DC2 B2 periodic Stokes box with C | every row `g < 1`, `rho < 1` | `g` 0.10 to 0.50, `rho` 0.18 to 0.72 (the default grows from D = 4, ADR 0073 B2) | met |
| DC3 B3 cavity runs with C | 7 of 7 stable | 7 of 7 (the default: 4 of 7 unstable, as ADR 0073 predicted) | met |
| DC4 no harm on the exact errors | 21 ratios `<= 1.05` | 20 met (velocity 0.990 to 1.000; TG, TG-S pressure 1.000 to 1.025); MMS pressure at dt 0.025: **1.119** (0.999 at 0.05, 1.043 at 0.0125) | **known miss** |
| DC5 no new order miss | | O1 coarse 2.305 with C (2.290 by default; band top 2.3); C enters the band on O6 MMS PISO coarse (1.605 to 1.919) and PIMPLE-C coarse (2.834 to 2.089) and halves the MMS PISO temporal errors (fine pair 2.92e-4 / 7.62e-5 to 1.43e-4 / 3.08e-5); the other rows within 0.01, except MMS PIMPLE-C fine (1.738 to 1.568, both outside the band) | **known miss** |
| DC6 ADR 0154 K1, K2 with the key | t = 3, `M_hf <= 0.0114` | K1 5.738e-3, K2 5.738e-3, 192 steps each, 21 s (the default: K1 diverges at step 30, K2 `M_hf` 282) | met |
| DC7 ADR 0069 BDF2 cylinder with C | K1b, K2b, K4 | St 0.16467, C_D 1.34578, 17 crossings; T1 met too (the default: 0.16480, 1.35247); 36 pressure iterations per step | met |
| DC8 as registered | | not evaluable for Re 100 | defective set-up (amendment 3) |
| DC8b (amendment 3) the Studio examples | St and mean `cd` within 1 % | Re 100: St 0.15169 to 0.15237 (0.44 %), cd 1.32349 to 1.32585 (0.18 %); Re 200: St 0.17514 to 0.17605 (0.52 %), cd 1.29133 to 1.29421 (0.22 %) | met |

Printed (BF): the TG BDF2 fine-pair orders with C equal the default's within 0.003 at N = 16, 32, 64 (PISO 1.036, 1.627, 2.359;
PIMPLE-4 1.005, 1.031, 2.010): the option does not touch ADR 0073's iterative-path floor. Cost: on the falling block the adaptive
count (MA3) needed 8 correctors and 8491 pressure iterations to 10 tau where the consistent correctors needed 2 and 2434 (ADR 0214
A1c).

**Reading.** With the option the step is stable on every set-up of the set, including the ones the default loses (the cavity at
Co_t 4 to 16, the B2 box from D = 4, the refined cylinder wall layer of K1 and K2, the falling block), and the answers of the
examples move by less than 0.6 %. The temporal accuracy is unchanged on TG and TG-S, better on MMS's velocity, and worse in one
place: MMS's pressure at dt 0.025 (+12 %). The DC5 miss is the coarse TG pair's order 2.305, 0.005 above the band (its errors change
by under 2 %).

### Mutations

Applied one at a time to b5c33ed4's code by `f301-scratch/mutate.py` (an exact text replacement that checks that the text occurs
once), the named tests run once in release in `target-f301-mut`, each file restored and compared byte for byte with its kept copy
(equal after each of the seven).

| Id | Must fail | Result |
| --- | --- | --- |
| MU1 ALE exclusions kept | MA1, MA3, MA4 | MA1: no estimate on the moving step; MA3: no estimate, the run fails at step 30 (mesh error); MA4: no preflight estimate |
| MU2 coupled-face compensation dropped | S1p3 | failed on its velocity part: 1.18e-3 U0 (limit 1e-10) |
| MU3 E3 with `interp(rAU)` | S1B2 | failed: 9.46e-2 `U_s` |
| MU4 periodic links not in `H1` | SPx2 | failed: 2.62e-3 U0 (D 8.2e-13); SPx (reported) passed (2.62e-3 against its limit 6.1e-3) |
| MU5 preflight without the flag | MA4 | failed: no preflight estimate |
| MU6 the key not passed | C1 | failed: not the replica, not different from the default |
| MU7 the old Raise message | MSG1 | failed: "short pressure waves" missing |

`scripts/limit_audit.py` (workspace scan) reports no hit in the new test files.

### Disclosures

- Order of the runs: registration; N2 on the base; E0 to E4; run 1 of the deliverable tests; amendment 1 and its three tests once;
  amendment 2 and S-p3 once; K0 on the schema table alone; E5; C1 to C4 once; the rcfd-fv benchmark set once (a first start stopped
  at compilation, before any run); the rcfd-api benchmark set once; amendment 3 and BX2 once; N2 on the final tree; the mutations;
  clippy-only edits of the test files afterwards (`allow` attributes, `is_ok_and`, a removed wrapper; no change of behaviour).
- S1p3 is a known miss of a deliverable: the periodic steady identity holds in velocity to 1.65e-12 U0 (about 60 times under its
  limit) and its pressure difference to the default (2.49e-10 of the range) lies below the default's own drift (3.32e-10), but the
  registered pressure limit is below the floor of this problem without a pressure Dirichlet condition; it was not re-registered a
  third time.
- SPx as registered was met, but the default's seam faces carry no transient carry-over (ADR 0073 R3) and dominate its comparison;
  MU4 shows that SPx alone would not see the periodic links (SPx2 does).
- K0's run of the Studio test `correctors_x` first failed on a missing `rcfd-runner.exe` in this target directory (an environment
  step of that test); after building it the test passed unchanged.
- The key is an optional table; no fenced TOML block was added to the documents (the H1 record and `rules_w19c` read those blocks).
- The machine was shared: other agents ran (CPU load 60 to 100 %, 7 to 9 GB of physical memory free during the runs); a duplicate
  acquire of the earlier f298 gate held the gate lock from 02:06 UTC and waited for 20 GB free memory.

Status by the decision rule: the deliverables are met except S1p3 (known miss), with every set-up check and negative control held
and every mutation failing its tests; the benchmark set is complete. F-301 is set **partial** (`[~]`): the case-level wiring of the
moving-mesh estimate (SV301 and the adaptive count for dynamic meshes, Q2) is not delivered, and S1p3 is a known miss. DC4 and DC5
are known misses of the option, for Q1.

### Open questions for the maintainer, with recommendations

- **Q1. The default.** The benchmark set favours the option on stability (DC1, DC2, DC3, DC6 met, the whole scan stable) at
  unchanged answers (DC7, DC8b) and accuracy, with two small misses (DC4: MMS pressure +12 % at one step size; DC5: an order 0.005
  above a band). A flip changes every transient result, so it needs the bit-identity lift and re-recorded hashes, and the default
  path would then refuse non-orthogonal correctors, the component diagonal and face jumps. Recommendation: keep `false` now; flip it
  in its own item once Q5's remainder is done (so that the flip refuses nothing a case uses), with the lift approved; meanwhile let
  SV301's remedy name the key as well (the Raise message already does).
- **Q2. Case-level wiring for moving meshes.** Recommendation: yes, as a follow-up: SV301 for `dynamic_mesh` cases from
  `lag_preflight_moving` with the dynamic mesh's next motion (on a copy), its remedy naming `[solver.consistent_correctors]` (cheaper
  than the adaptive count here: 2 against 8 correctors); lift ADR 0156's refusal of the adaptive count with a dynamic mesh only with
  the edits of its test A9b and of the Studio's scope test approved.
- **Q3. One flag.** Recommendation: fold `consistent_periodic` and `consistent_body_force` into `consistent_correctors` (the key
  already sets all three), with an amendment to ADR 0214 for its R1 record.
- **Q4. Status.** Recommendation: partial, as the decision rule says.
- **Q5. The remaining refused combinations.** Recommendation: non-orthogonal correctors next (the SIMPLEC non-orthogonal
  compensation of ADR 0050), then the component diagonal and face jumps, each with its steady identity check; the fractional step
  stays refused (it has no correctors).
- **Q6. S1p3.** Recommendation: accept the periodic identity on its velocity part and the floor comparison; a re-registration would
  need a periodic set-up with a pressure Dirichlet condition or tighter pressure tolerances.
