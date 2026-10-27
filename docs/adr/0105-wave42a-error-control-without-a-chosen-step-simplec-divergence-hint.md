# ADR 0105: Wave 42A: error control without a chosen step (F-209), SIMPLEC divergence hint (F-248)

Status: registered before the registered tests were run. Limits are fixed; a miss becomes an `#[ignore]`d `*_known_miss` test
with the limit unchanged, a defective set-up gets a new corrected test and the old one stays ignored. Acceptance texts are not
edited. Defaults stay bit-identical and `crates/rcfd-api/tests/baseline_hashes.rs` is not touched. Clean-room: the reference of
E2 is rcfd's exact semi-discrete solution (ADR 0101); F-248 has no reference value.

## What was run before this ADR was written (disclosed)

Release build, HEAD 44d7ccc plus the two smoke tests below (both kept as ignored set-up smokes):

* `e2_default_controller_setup_smoke` (rcfd-fv tests/adaptive_step.rs): the E2 controller below at tol 1e-2 on the E1 shear
  wave, first 50 accepted steps: t = 2.2199e-4 reached, 18 rejected trial steps (the first proposal 1 halved down to about
  4e-6), next proposal 4.47e-6, field finite. No error at T was computed.
* `d1_vent_divergence_setup_smoke` (rcfd-api tests/heat_exchanger.rs): the default-code X7 vent at u = 2, K = 2.5, pseudo step
  0.02, no compensation key. The steady divergence guard (ADR 0077) trips at iteration 9, kind `continuity_growth` (continuity
  11.7 against the limit 3.13), cell 183 at x = 1.67; last good iteration 1. The message is the ADR 0077 text and does not name
  `solver.simplec_boundary_compensation`.

Known before writing: ADR 0101 measured E1 (S4 controller settings: first proposal 0.01, `min_dt` 1e-5, `max_dt` 1) at
E = 7.836e-3 for tol 1e-2 and 2.080e-3 for tol 2.5e-3 (E / (tol T) 0.784 and 0.832). Its Python model without the clamps
gave 2502 and 9989 controlled steps, which suggests E near 0.8 tol for E2 as well; a pass of E2 is therefore expected, but E2
itself was never run.

## E2 (F-209): error control reaches its requested error without a chosen step

Acceptance, first clause (user wording of 2026-10-22): "Error-controlled stepping (step doubling, `solver.error_tolerance`) on a
two-timescale linear decay (rates differing by at least 100, both modes of unit initial amplitude) reaches its requested error
without a user-chosen step: the final error E satisfies E / (tol T) within [0.3, 3] for two tolerances a factor 4 apart".

Set-up (`e2_error_control_reaches_the_requested_error_on_a_two_timescale_decay`, rcfd-fv tests/adaptive_step.rs, non-ignored):

* Flow: the E1 shear wave of ADR 0101 unchanged (`shear_wave(1.0)`: `u_x = sin y + sin 12y` on the periodic 4 x 64 box of
  height 2 pi, nu = 1, PISO, central, implicit Euler, solver tolerances 1e-16 abs / 1e-14 rel). Rates 0.9992 and 128.10, ratio
  128.2; both modes have unit initial amplitude. One PISO step equals the implicit-Euler factors to 1.7e-15 (E1 smoke).
* T = 1 (the slow mode decays by e^-1, as in E1 and S4).
* Tolerances 1e-2 and 2.5e-3 (factor 4; the ends of S4's range, as in E1).
* Controller, "no user-chosen step": `StepSettings::default()` with only `error_tol` set (no `min_dt`, no `max_dt`, safety 0.9,
  growth 1.2, shrink 0.5) and the first proposal equal to the run length T. The only inputs are the tolerance and T. The E1
  settings do not qualify: their first proposal 0.01 was chosen by hand and their `min_dt` 1e-5 floor bound 883 of the 1913
  accepted steps at tol 1e-2 (accepted above the tolerance), so the E1 values 0.784 / 0.832 come from a clamped run and are
  not evidence for this clause.
* Error norm: relative L2 over the cells of the velocity vector against the exact semi-discrete solution
  `exp(-nu lambda_k T) sin(k y_c)` (`e1_error` with `e1_exact`). The spatial error is excluded, so E is the time error the
  controller controls.
* Both tolerances are measured before asserting (the E1 deviation of ADR 0101 is avoided). Steps and rejections are reported;
  no step-count limit.

Limit: `0.3 <= E / (tol T) <= 3` for both tolerances.

Verdict: F-209 stays `partial` whatever E2 gives (the flat-plate clause is open). A miss becomes `e2_..._known_miss` with the
limit unchanged. E1 (`e1_..._known_miss`) stays as it is.

Registered limitation (not fixed here, a default change): through the case keys the run is not free of a chosen step.
`rcfd_api::timectl::from_case` takes the first proposal and `max_dt` from `solver.dt` and sets `min_dt = 1e-3 solver.dt` when
`solver.min_dt` is 0 or absent. E2 therefore shows the library controller (`rcfd_fv::stepctl`, which the case path drives), not
the case path with its defaults; on the case path with `solver.dt = T` the 1e-3 floor would bind through the fast transient.

## D1 / D2 (F-248): located hint for a diverging steady SIMPLEC run

F-248 is `done` (ADR 0102); the queued item is a located diagnostic: when a steady SIMPLEC run diverges and the case has a
fixed-pressure or periodic boundary and `solver.simplec_boundary_compensation` is false, the error names the key.

Design, fixed before coding:

* Divergence criterion: the existing steady divergence guard of ADR 0077, unchanged: K1 any non-finite residual, continuity,
  velocity, pressure or flux (always on); K2 continuity above `STEADY_GROWTH_FACTOR` = 1e6 times the smallest continuity of
  the last 50 iterations (floor 1e-6 U_ref); K3 speed above 1e6 U_ref; K4 pressure range above 1e12 P_ref (K2 to K4 off with
  `solver.steady_divergence_guard = "off"`). No new trip criterion is added: a new trip would stop runs that now keep
  iterating and so change their results.
* Location: only `Simulation::steady_divergence_message` (read-only, `&self`), which feeds the run-control reason
  (`runctl::drive`) and the service error. It appends one sentence after the ADR 0077 text (and after the ADR 0077 part B
  scheme note when that applies); `SteadyDivergence::message` and `divergence_json` are unchanged, so no other text changes.
* Condition: engine `Steady`, `settings.coupling == Simplec` (`solver.algorithm = "simplec"`), the compensation flag false,
  and at least one boundary face the compensation would act on: not an empty face, velocity not prescribed
  (`PressureBc::fixed_u` false), and either periodic-coupled or a Dirichlet pressure (`FaceCoeffs::c != 0`;
  `c = (a - 1)/dn`, so a fixed value has `c = -1/dn`). This is the face set of `simplec_boundary_flux_correction` restricted
  to fixed-pressure and periodic faces. The coupled solver, SIMPLE and the transient solvers get no hint.
* Hint text: names `solver.simplec_boundary_compensation = true` and says why (ADR 0102: without it the SIMPLEC term
  (rAtU - rAU) grad p_old is not compensated on fixed-pressure and periodic boundary faces).

Tests (rcfd-api tests/heat_exchanger.rs, non-ignored):

* D1 `d1_diverging_simplec_vent_names_the_compensation_key`: `runctl::drive` on the D1 smoke case (X7 vent, u = 2, K = 2.5,
  pseudo step 0.02, no key). Limits: state `diverged`, the reason contains `steady divergence guard` and
  `solver.simplec_boundary_compensation`, trip iteration <= 15 (smoke: 9).
* D1n negative control: the S2 cavity of ADR 0077 (w35b) with `alpha_u = alpha_p = 1.0` and `algorithm = "simple"`, known to
  diverge under the guard (ADR 0077 A5/A6/SV-1). Limits: state `diverged`, the reason does not contain
  `simplec_boundary_compensation`.
* D2 `d2_converging_simplec_run_is_bit_identical_with_the_hint_path`: the X7 vent at u = 0.5, pseudo step 0.02, no key
  (converges, ADR 0095: 2.85e-12), which meets the hint condition (SIMPLEC, periodic seams, vent pressure, key false). 600
  iterations twice: once calling `steady_divergence_message()` after every iteration, once never. Limits: ux, uy, p and the
  face flux bit-identical; no guard trip; the message is `None` throughout. Note: the hint code runs only after a trip
  (`steady_divergence_message` returns early otherwise, and `runctl::drive` already calls it every step), so D2 is a
  regression guard on that early return; bit identity of all default runs is also covered by the unchanged baseline hashes.

Also re-run after the `vent_text` extraction (a pure refactor of `vent_run`): the three X7 tests.

Verdict: F-248 stays `done`; its note is extended.

## Results

Addendum written after the runs: release build on top of b7ec907 (this ADR) plus the test files and the `sim.rs` hunk. Each
registered test was run once.

| Check | Measured | Limit | Result |
| --- | --- | --- | --- |
| E2 tol 1e-2 | 2502 steps (18 rejected), E 7.917e-3, E / (tol T) 0.792 | [0.3, 3] | met |
| E2 tol 2.5e-3 | 9989 steps (20 rejected), E 2.080e-3, E / (tol T) 0.832 | [0.3, 3] | met |
| D1 X7 vent u = 2, pseudo 0.02, no key | `diverged` at iteration 9 (`continuity_growth`); reason contains the key | key named, iteration <= 15 | met |
| D1n S2 cavity, SIMPLE, alpha 1 / 1 | `diverged` at iteration 6; no hint | no key in the reason | met |
| D2 X7 vent u = 0.5, 600 iterations, message queried every iteration vs never | ux, uy, p, flux bit-identical; no trip; message `None` | bit-identical | met |
| X7 tests after the `vent_text` refactor | `x7_vent_reproduces_its_quadratic_loss_law` 3.94e-13, `x7_vent_at_fixed_pseudo_dt` 2.93e-12 | 1e-6 (unchanged) | met |

E2 takes 61 s in release (both tolerances). The step counts equal the unclamped model of ADR 0101 (2502, 9989). The controller
resolves the fast transient (about lambda_f / (4 tol) steps, ADR 0101), so the step counts stay far above the uniform 63 and 240
of E1; this clause asks only that the requested error is reached, which it is.

No deviation from the registered plan. Statuses: F-209 `partial` (first clause met by e2, Taylor-Green clause by S4, flat-plate
clause open; the case-key path still takes its first, largest and smallest step from `solver.dt`). F-248 stays `done` with the
diagnostic added.

Final gate on a fresh `git archive` of 384805c (which also holds the wave 42B/42C commits made up to then): `cargo fmt --check` clean,
`cargo clippy --workspace --all-targets -- -D warnings` clean, `cargo test --workspace --release --no-fail-fast` 1965 passed /
0 failed / 580 ignored.
