# ADR 0100: Wave 40D, Hysing bubble refinement check (F-055) and a NaN-rejecting stationarity check in the heat-exchanger tests

Status: registered before the registered runs. Limits are not changed after a run; a miss becomes an ignored `*_known_miss` test with the
limit intact. No library code changes: both parts are test code only, so defaults and the baseline hashes are untouched. Clean-room: the only
literature values are the Hysing et al. (2009) case-1 values read in wave 39E (ADR 0096). Written at HEAD a310dea (scratch copy
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
* H3: every non-ignored test of `heat_exchanger.rs` gives the same result as at a310dea (all pass), and every ignored known miss still
  fails. A test that passed before and fails now was passing on a non-finite field; that would be reported as a finding with its limit kept.

## Addendum: measured values (after the runs)

### Part B (one registered run, 1750 s in total; scratch tree: archive of a310dea plus the 40D test files; no library source changed from a310dea to ff02104)

| Grid | dt / dt_cap | y_c(3) (err) | V_c,max (err) | t of V_c,max (err) | drift | bound violation | run time |
|------|-------------|--------------|---------------|--------------------|-------|-----------------|----------|
| 40 x 80 | 0.473 | 1.0282 (0.0531) | 0.2204 (0.0213) | 0.8500 (0.0713) | 5.5e-15 | 9.8e-14 | 30 s |
| 80 x 160 | 0.669 | 1.0178 (0.0635) | 0.2187 (0.0230) | 0.8287 (0.0926) | 3.6e-14 | 1.3e-13 | 207 s |
| 160 x 320 | 0.946 | 1.0124 (0.0689) | 0.2180 (0.0237) | 0.8200 (0.1013) | 1.1e-13 | 6.4e-13 | 1513 s |

The 40 x 80 AMG values equal the CG values of `vof_bubble.rs` to the printed digits. The 160 x 320 grid ran (25 min); the fallback series was
not needed.

| Check | Measured | Limit | Result |
|-------|----------|-------|--------|
| B1 y_c(3) monotone | errors 0.0531, 0.0635, 0.0689 (growing) | decreasing | **miss** |
| B1 V_c,max monotone | errors 0.0213, 0.0230, 0.0237 (growing) | decreasing | **miss** |
| B2 order | e(160) / e(40) = 1.30 (y_c), 1.11 (V_c,max) | <= 0.5 | **miss** (printed values; the test stops at B1) |
| B3 peak time | 0.1013 vs 0.0713 | e_t(160) < e_t(40) | **miss** (printed values) |
| B4 Z1 on every grid | drift <= 1.1e-13, bound <= 6.4e-13 | 1e-12, 1e-9 | met (printed values) |

The test is kept, limits unchanged, as the ignored `b1_hysing_bubble_error_decreases_under_refinement_known_miss`. It is not a set-up
defect: the set-up is the registered one, all three grids are finite and bounded, and the solution converges, but not to the reference.
Successive differences of y_c(3) are 0.0105 and 0.0054 (observed order 0.96); a Richardson extrapolation from these three grids gives
y_c(3) about 1.007, against 1.0813. For V_c,max the differences are 0.0017 and 0.0007 (order 1.3) and the extrapolated value is about
0.2175, 10 % below 0.2417. The peak also comes earlier on finer grids (0.850, 0.829, 0.820 against 0.9213). So the algebraic VOF discretisation
converges to a bubble that rises about 10 % too slowly. The 40 x 80 pass of ADR 0049 / 0096 is the coarse-grid point of that error, not a
grid-converged agreement. The cause is not investigated in this wave. Candidates for a later wave, not tested here: the linear-in-alpha
mixture viscosity and density at the interface, the CSF curvature from the smoothed alpha, the interface-compression smearing, and the
missing transposed viscous stress at the viscosity jump (listed as not delivered in the F-055 note). F-055 stays done, as the user decided.

### Part H

| Check | Measured | Result |
|-------|----------|--------|
| H1 NaN field | panics "T is not finite after 10 iterations" | pass |
| H2 stationary +inf | panics "p is not finite after 10 iterations" | pass |
| H3 non-ignored tests | the 7 tests x1-x7 pass, as at a310dea; with H1 and H2, 9 of 9 | pass |
| H3 known misses still fail | x1 registered stationarity: p change 0.423 (as before); x4 registered lossless: ux change 4.06e-8 > 1e-8 with finite fields; x7 fixed pseudo-dt: now stops at the new finiteness assert in `run` (it stopped at the finiteness assert of `x7_sweep` before) | pass |

No test that passed before fails now, so no existing pass came from a non-finite field. One stale comment found: the ignore reason and the doc
comment of `x4_registered_lossless_setup_known_miss` say "flow NaN", but at this HEAD the registered lossless X4 duct ends with finite
fields and fails on the ux change (4.06e-8). The text is left as written (not this item); the finiteness check would have caught a NaN.
