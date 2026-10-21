# ADR 0100: Wave 40D, Hysing bubble refinement check (F-055) and a NaN-rejecting stationarity check in the heat-exchanger tests

Status: registered before the registered runs. Limits are not changed after a run; a miss becomes an ignored `*_known_miss` test with the
limit intact. No library code changes: both parts are test code only, so defaults and the baseline hashes are untouched. Clean-room: the only
literature values are the Hysing et al. (2009) case-1 values read in wave 39E (ADR 0096). Written at HEAD f0168d0 (scratch copy
`<scratch dir>`, target `<target dir>`).

## Part B: F-055 rising-bubble refinement (test file `crates/rcfd-fv/tests/vof_bubble_refinement_w40d.rs`)

F-055 is done (ADR 0096: 40 x 80 meets the ADR 0049 limits). The user accepted that status and asked for a refinement check. The status
stays done whatever this check measures; the outcome goes into the F-055 note.

Reference (Hysing et al. 2009, Table V, TP2D, 1/h = 320, read in ADR 0096): V_c,max = 0.2417 at t = 0.9213, y_c(3) = 1.0813.

Set-up: the `bubble(true)` set-up of `vof_bubble.rs` (Hysing Table I properties, free-slip sides, no-slip top and bottom, 3 PISO
correctors, alpha from a 10-point indicator) on n x 2n cells, n = 40, 80, 160, with two changes, the same on all three grids:

* `dt = 0.1 / n` (2.5e-3 on 40 x 80, as `vof_bubble.rs`; constant flow Courant number). The Brackbill capillary limit scales as h^1.5, so
  dt / dt_cap grows with refinement: 0.47 (40 x 80), 0.67 (80 x 160), 0.95 (160 x 320). The test asserts dt <= dt_cap on every grid.
* `amg_pressure = true` (the case tree already turns AMG on above 2000 cells, ADR 0052). Reason: the timing smoke below puts a CG run of
  160 x 320 at about 2.1 h, over the 60-minute run bound; AMG puts it at about 33 min. The 40 x 80 point of this series therefore is not
  bit-equal to the CG run of `vof_bubble.rs` (y_c(3) 1.0282, V_c,max 0.2204 at t 0.850, ADR 0096); the trend uses only the AMG series and
  the CG numbers are context.

Metrics per grid: e_y(n) = |y_c(3) - 1.0813|, e_v(n) = |V_c,max - 0.2417|, e_t(n) = |t(V_c,max) - 0.9213| (V_c,max sampled every step).

Registered checks (one test, `b1_hysing_bubble_error_decreases_under_refinement`, ignored as heavy and run once for the record; it prints
all three grids before asserting anything, then asserts in this order):

* B1 (monotone): e_y(80) < e_y(40) and e_y(160) < e_y(80); e_v(80) < e_v(40) and e_v(160) < e_v(80).
* B2 (order): e_y(160) <= 0.5 e_y(40) and e_v(160) <= 0.5 e_v(40), i.e. a mean observed order of at least 0.5 over the two refinements
  (a first-order method gives 0.25). This is an order limit (wave-35 rule a), not a limit from the 40 x 80 numbers.
* B3 (peak time, weak, separate): e_t(160) < e_t(40). The peak of V_c is flat (on 40 x 80 it falls at 0.850, 0.071 early), so the time is
  not part of the monotone set B1.
* B4: the ADR 0049 Z1 limits on every grid (phase-volume drift <= 1e-12 relative, bound violation <= 1e-9). They were registered for
  40 x 80 only; the smoke shows the bound violation growing about 10x per refinement, so B4 is asserted last and a B4 miss cannot hide
  B1-B3.

Fallback (registered now): if the 160 x 320 run exceeds 60 minutes or is not finite, the series becomes n = 40, 56, 80 (ratio about
sqrt 2) with the same checks, and I say so. No other change.

Smoke (rule d, before registering; no reference value was compared):
* Timing, t = 0.05 on each grid: CG 0.3 s / 8.2 s / 127.7 s (40 / 80 / 160), AMG 0.4 s / 2.5 s / 32.7 s. Extrapolated full runs to t = 3:
  CG about 2.1 h on 160 x 320, AMG about 33 min. Phase-volume drift <= 4.2e-15 and bound violation <= 1.3e-12 on all grids.
* 160 x 320 with AMG to t = 0.4 (640 steps, 198 s, dt / dt_cap 0.946): all fields finite, max|U| 0.28, drift 9.7e-15, bound violation
  6.4e-13. So the finest grid is stable at 95 % of the capillary limit over the first 0.4 time units. Measured rate 0.31 s per step:
  the full run is about 25 min.

## Part H: heat-exchanger `run()` rejects non-finite fields (test file `crates/rcfd-api/tests/heat_exchanger.rs`)

Defect (same class as ADR 0081 part N, where `SimpleSolver::run` reported a NaN state as converged): `run_floor` checks stationarity with
`fold(.., f64::max)`, which drops NaN. A field with a NaN anywhere reads as unchanged (change 0), and an infinite value equal before and
after gives inf - inf = NaN, dropped too (the scale becomes inf and the change 0). So a NaN or infinite field passes the 1e-8 check.

Change: the per-field comparison moves into `assert_stationary`, which first asserts that every value of the new field is finite
("<field> is not finite after <n> iterations"), then runs the unchanged scale, `p_floor` and 1e-8 change test with the unchanged message.
`run_floor` calls it for the same fields. No limit, iteration count or set-up changes.

Registered checks:

* H1 `h1_stationarity_rejects_nan_field`: `assert_stationary` on a field with a NaN (equal before and after) panics with "is not finite".
* H2 `h2_stationarity_rejects_stationary_infinity`: same with +inf in the same cell before and after.
* H3: every non-ignored test of `heat_exchanger.rs` gives the same result as at f0168d0 (all pass), and every ignored known miss still
  fails. A test that passed before and fails now was passing on a non-finite field; that would be reported as a finding with its limit kept.
