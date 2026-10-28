# ADR 0106: Wave 42B, height-function curvature for the VOF solver and the Hysing refinement re-run (F-055)

Status: registered before any code and any run. Limits are fixed here; a miss becomes an ignored `*_known_miss` test with the
limit intact; a defective set-up gets an amendment written before the corrected run. Results go into the addendum. Written at
HEAD 6a11732 (scratch copy `<scratch dir>`, target `<target dir>`). Clean-room: first principles and
the Hysing et al. (2009) case-1 values already read in ADR 0096 (V_c,max 0.2417 at t 0.9213, y_c(3) 1.0813). Context: ADR 0100
(refinement miss), ADR 0103 (diagnosis: the averaged-diagonal defect at `Symmetry` walls, fixed by the opt-in
`VofSettings::component_diagonal`; residual drift with errors growing from 40 x 80 to 80 x 160; suspect #1 the CSF curvature,
whose initial-circle rms error grows 0.13 -> 0.77 from 40 x 80 to 320 x 640).

## Choice of estimator

Height functions on the VOF fraction, Cartesian grids only. Reason (first principles): on a uniform grid the sum of the exact
volume fractions over a column that crosses the interface once, times the cell size along the column, is the column average of
the interface height `H(x)` over the column width. Central differences of column averages approximate `H'` and `H''` at the
column centre with O(h^2) error (the averaging adds `h^2 H''/24`-type terms that are smooth in x), so the curvature
`H'' / (1 + H'^2)^1.5` (2-D) is second order for exact fractions. The CSF estimator (divergence of the normalised gradient of a
smoothed fraction) has no such cancellation; ADR 0103 D1b shows it does not converge. The method is commonly attributed to
Cummins, Francois and Kothe (2005) and Popinet (2009) (UNVERIFIED: recalled, not read; nothing below depends on them).

Known limit, stated before coding: fraction errors `eps` per cell (for example the 10 x 10 point indicator used by every bubble
set-up, eps about 1e-2) give a height error of about `eps h` and a curvature error of about `eps / h`, which grows under
refinement for any curvature estimator. The convergence tests below therefore use exact fractions; the b1 set-up is not changed
(see the smoke rule S2).

## Code (opt-in, defaults bit-identical)

`VofSettings::height_function: bool` (default `false`; must be set before `VofSolver::new`). With `false` the code path is the
existing one, byte for byte (the old `curvature()` body is unchanged; the option returns early into the new code).

* Mesh check in `VofSolver::new`: the cell centres must form a complete lattice with constant spacing along each axis and all
  cell volumes equal (relative 1e-9). Otherwise `new` returns `InvalidValue` at `settings.height_function`. Axes with one cell
  (the empty direction of a 2-D mesh) are inactive; 2-D (one inactive axis) and 3-D are both implemented.
* Active cells: cells owning an internal face with `|alpha_N - alpha_P| > 1e-6`. Every face that carries a surface-tension force
  above that level has both cells active. Inactive cells get `kappa = 0` (their faces carry `|d alpha| <= 1e-6`).
* Per active cell: directions are the active axes in order of decreasing `|d alpha / dx_d|` (Green-Gauss gradient of the
  unsmoothed fraction). For a direction d with `s = sign(d alpha / dx_d)` and K = 3, 4, 5 in turn: columns along d at the
  transverse offsets {-1, 0, 1} (2-D: 3 columns; 3-D: 3 x 3), cells -K .. K from the cell's row. A column brackets the interface
  if all its cells exist, the cell at the phase-1 end has `alpha >= 1 - 1e-3` and the cell at the other end `alpha <= 1e-3` (the
  phase-1 end is the -d end if s < 0, the +d end if s > 0). The first (d, K) whose columns all bracket gives heights
  `H = h_d sum alpha` (s < 0) or `h_d sum (1 - alpha)` (s > 0) and
  * 2-D: `kappa = s H_tt / (1 + H_t^2)^1.5`,
  * 3-D: `kappa = s (H_xx (1 + H_y^2) + H_yy (1 + H_x^2) - 2 H_xy H_x H_y) / (1 + H_x^2 + H_y^2)^1.5`,
  with central differences. Sign convention as the existing estimator, `kappa = -div(grad alpha / |grad alpha|)` (negative on
  the Hysing bubble, which has alpha = 1 outside).
* Fallback near under-resolved interfaces (documented in the code): an active cell with no bracketing column takes the mean of
  its face neighbours that already have a height-function value, repeated up to 3 passes (fallback 1); a cell still without a
  value takes the existing CSF value with `n_smooth` passes (fallback 2). `VofSolver::curvature_fallbacks()` returns the two
  counts so the tests can check them.

## Registered tests (`crates/rcfd-fv/tests/vof_height_function_w42b.rs` unless stated)

Exact fractions: 2-D, cell area of `disc intersect rectangle` from the antiderivative of `sqrt(R^2 - x^2)` split at the
breakpoints (exact up to round-off); 3-D, the exact 2-D area integrated over z with 5-point Gauss-Legendre on 16 sub-intervals
between the breakpoints. Each set-up asserts the phase volume against `pi R^2` (relative 1e-10) or `4/3 pi R^3` (relative 1e-8).
Errors are relative errors of `-kappa` against `1/R` (2-D) or `2/R` (3-D), over cells with `0.05 < alpha < 0.95` (the ADR 0103
D1b selection).

* H1 circle, construction only, not ignored. Unit square, n x n, n = 40, 80, 160, 320, R = 0.25 at (0.51, 0.503), alpha = 1
  outside. Limits: H1a rms error decreases strictly from each grid to the next; H1b rms(320) <= rms(80) / 8 (observed order at
  least 1.5 over two refinements; the analysis gives 2); H1c max(320) <= max(80) / 4 (order at least 1 for the maximum); H1d no
  fallback-2 cell on any grid. Information: the fallback-1 count, and the CSF estimator (n_smooth 2) on the same exact
  fractions (this tells whether D1b's growth came from the indicator fractions or from the estimator).
* H2 ball, construction only, not ignored. Unit cube, n^3, n = 16, 32, 64, R = 0.25 at (0.51, 0.503, 0.497), alpha = 1 outside.
  Limits: H2a rms decreases strictly 16 -> 32 -> 64; H2b rms(64) <= rms(16) / 4 (order at least 1; the 16 grid has R/h = 4 and
  is pre-asymptotic, so the margin is wider than H1b); H2c no fallback-2 cell on 32 and 64.
* H3 static bubble (Laplace jump), the ADR 0103 D1 set-up with the height-function option and exact initial fractions: Hysing
  fluids (rho 1000 / 100, mu 10 / 1, sigma 24.5), g = 0, R = 0.25 centred in the unit square, no-slip walls, n x n with
  n = 40, 80, 160, dt = 0.1/n, AMG pressure, t = 1. Measured as D1: e = (dp - sigma/R) / (sigma/R), inside cells alpha < 0.01
  and r < R - 3h, outside alpha > 0.99 and r > R + 4h. Noise floor: |e| <= 0.002 counts as converged (the pressure noise of the
  D1 parasitic currents, rho U^2 with U about 5e-3, is 2.6e-4 of sigma/R; 0.002 is about 8 times that). Limits: H3a for each
  refinement step, |e| falls strictly or the finer |e| <= 0.002; H3b |e(160)| <= 0.5 |e(40)| or |e(160)| <= 0.002.
  Information: max|U| at t = 1 and the fallback counts at t = 1; the CSF estimator on the same exact fractions on 40 and 80.
  The test is ignored as heavy (run once) unless all of H3 takes under 120 s in release.
* H4 verdict (`b1h_hysing_bubble_error_decreases_under_refinement_height_function` in
  `crates/rcfd-fv/tests/vof_bubble_refinement_w40d.rs`): the ADR 0100 B1-B4 series, limits unchanged (B1 monotone decrease of
  e_y and e_v over 40, 80, 160; B2 e(160) <= 0.5 e(40) for both; B3 e_t(160) < e_t(40); B4 drift <= 1e-12, bound violation
  <= 1e-9 on every grid), set-up of ADR 0100 unchanged (10-point indicator fractions, dt = 0.1/n, AMG pressure) with
  `component_diagonal = true` and `height_function = true`. This variant is the verdict. The single 160 x 320 run of this wave
  goes to it, whatever the 40 and 80 points show (the helper runs all three grids before asserting). If H4 passes all of
  B1-B4 it is kept not ignored (about 30 min more in the release gate). If it misses it becomes
  `b1h_..._known_miss`, ignored, limits intact.
* H5 second registered variant (information, cannot change the verdict): `component_diagonal + height_function +
  full_viscous_stress` on 40 x 80 and 80 x 160 only (no 160 x 320: the single run goes to H4). Printed errors and whether
  e(80) < e(40) for both metrics. An ignored diagnostic; it cannot be un-ignored in this wave.

F-055 is set done only if H4 passes and is not ignored, and the dam break and 40 x 80 bubble tests of its acceptance still
pass not ignored. In every outcome no option becomes a default (the user decides after this re-run). The option is kept in every
outcome (registered now under wave-35 rule b): it is the curvature estimator H1 to H3 verify, and the H4/H5 records need it.

## Smoke (rule d), run before the H runs; rules fixed now

* S1: construction of every set-up on its smallest grid (H1 on 20 x 20 only, a grid not used in H1's limits; H2 on 8^3; H3 on
  40 x 40 to t = 0.05; H4 and H5 on 40 x 80 to t = 0.05; H4 on 160 x 320 to t = 0.1). No reference value is compared except
  the phase-volume checks. If a set-up fails to construct or is not finite, it is a defect: amendment before the corrected run.
* S2: the height-function curvature of the b1 initial field (10-point indicator) on 40 x 80, 80 x 160, 160 x 320, construction
  only, printed (rms, max, fallback counts). Rule fixed now: H4 keeps the indicator field (the ADR 0100 set-up), whatever S2
  shows, unless the 160 x 320 smoke of S1 is non-finite or its max|U| at t = 0.1 exceeds 1.0 (four times V_ref, a blow-up
  sign). Then the indicator field is a defective set-up for this estimator: an amendment moves H4 to exact initial fractions
  before any H4 run, and the indicator version is kept as an ignored known miss with the same limits.

## Defaults question (registered now, report only)

In the scratch tree only, never committed: set `component_diagonal` and `height_function` to `true` in `VofSettings::new` and
run every VOF-related test file (rcfd-fv: vof_*.rs, mixture_drift.rs, cavitating_venturi.rs; rcfd-api: vof_tree.rs,
vof_wiring_tree.rs, mixture_cavitation_tree.rs, baseline_hashes.rs) in release. Report per file pass / fail. Non-Cartesian VOF
meshes would be refused by the height-function check; that is reported as a blocker of a default switch, not worked around.

## Smoke results (S1, S2; written before the H3, H4 and H5 runs)

* S1 construction: circle 20 x 20 rms 0.0229, max 0.0471, fallbacks [11, 0]; ball 8^3 (R/h = 2) rms 0.109, max 0.193,
  fallbacks [0, 172] (the CSF fallback acts on this under-resolved ball, as designed); static bubble 40 x 40 to t = 0.05: Laplace
  error +0.0050, max|U| 1.8e-4, no fallback, finite; H4 on 40 x 80 to t = 0.05: V_c 0.0257, finite, drift 4.6e-16, bound 2.2e-15;
  H5 on 40 x 80 to t = 0.05: V_c 0.0256, finite; H4 on 160 x 320 to t = 0.1 (160 steps, 44 s): finite, max|U| 0.068, drift 4.9e-15,
  bound 1.4e-13. Every set-up constructs and runs; all phase-volume checks pass.
* S2 height-function curvature of the 10-point indicator field (construction only): rms 0.209 / 0.126 / 0.740, max 0.42 / 0.28 /
  2.98 on 40 x 80 / 80 x 160 / 160 x 320, no fallbacks. The indicator noise dominates on 160 x 320 (the eps / h growth stated
  above). By the S2 rule (160 x 320 smoke finite, max|U| 0.068 <= 1.0) H4 keeps the indicator field; no amendment.

## Addendum: measured values (after the runs)

All runs once, release build, scratch tree (archive of 6a11732 plus this wave's files). Errors are relative errors of `-kappa`
(H1, H2) or of the Laplace jump (H3); bubble errors are to 1.0813 / 0.2417 / 0.9213.

### H1 circle (exact fractions), not ignored

| n | rms | max | fallbacks [neighbour, CSF] | CSF estimator rms / max (information) |
|---|-----|-----|----------------------------|----------------------------------------|
| 40 | 5.18e-3 | 8.25e-3 | [0, 0] | 0.165 / 0.324 |
| 80 | 1.22e-3 | 1.95e-3 | [0, 0] | 0.225 / 0.444 |
| 160 | 3.12e-4 | 4.65e-4 | [0, 0] | 0.395 / 0.894 |
| 320 | 7.65e-5 | 1.17e-4 | [0, 0] | not run |

H1a strictly decreasing: met. H1b rms(320) = 7.65e-5 <= rms(80)/8 = 1.53e-4: met (observed order 2.0). H1c max(320) = 1.17e-4 <=
max(80)/4 = 4.87e-4: met. H1d no CSF fallback: met. The CSF estimator also diverges on exact fractions, so the ADR 0103 D1b growth
is a property of the CSF estimator, not of the indicator fractions.

### H2 ball (exact fractions), not ignored

| n | rms | max | fallbacks [neighbour, CSF] |
|---|-----|-----|----------------------------|
| 16 | 2.83e-2 | 2.87e-2 | [450, 3] |
| 32 | 7.64e-3 | 3.26e-2 | [386, 0] |
| 64 | 2.75e-3 | 2.51e-2 | [347, 0] |

H2a decreasing: met. H2b rms(64) = 2.75e-3 <= rms(16)/4 = 7.08e-3: met. H2c no CSF fallback on 32 and 64: met. Not registered,
reported: the 3-D maximum error does not converge (0.029, 0.033, 0.025), and a few hundred cells per grid need the neighbour
fallback; the rms converges at about order 1.5 to 1.9. Not investigated (the benchmark is 2-D); a 3-D user should know it.

### H3 static bubble Laplace jump (exact initial fractions, t = 1), ignored (215 s > 120 s)

| n | height function: e, max|U| at t = 1, fallbacks | CSF (information): e, max|U| |
|---|------------------------------------------------|------------------------------|
| 40 | +0.00511, 2.7e-5, [12, 0] | -0.0479, 5.1e-3 |
| 80 | +0.00128, 4.8e-6, [0, 0] | -0.0513, 3.7e-3 |
| 160 | +0.00030, 1.2e-6, [0, 0] | not run |

H3a: met (strict decrease on both steps). H3b |e(160)| = 0.00030 <= 0.5 x 0.00511: met. The Laplace error converges at about
order 2 and the parasitic currents fall by about 200 to 1000 times against CSF. The CSF numbers with exact fractions equal the
ADR 0103 D1 numbers with indicator fractions to 3 digits.

### H5 (second variant, information): component diagonal + height function + transposed viscous stress

| Grid | y_c(3) (err) | V_c,max (err) | t of V_c,max (err) | drift | bound |
|------|--------------|---------------|--------------------|-------|-------|
| 40 x 80 | 1.0794 (0.0019) | 0.2402 (0.0015) | 0.9400 (0.0187) | 5.2e-15 | 8.5e-14 |
| 80 x 160 | 1.0800 (0.0013) | 0.2409 (0.0008) | 0.9225 (0.0012) | 5.4e-14 | 7.8e-14 |

Both errors fall from 40 to 80. By the registered plan H5 gets no 160 x 320 run, so it cannot be judged on B1-B4 in this wave.

### H4 verdict: component diagonal + height function, ADR 0100 B1-B4 (the single 160 x 320 run)

| Grid | dt / dt_cap | y_c(3) (err) | V_c,max (err) | t of V_c,max (err) | drift | bound | run time |
|------|-------------|--------------|---------------|--------------------|-------|-------|----------|
| 40 x 80 | 0.473 | 1.0815 (0.0002) | 0.2486 (0.0069) | 0.8975 (0.0238) | 4.6e-15 | 8.0e-14 | 18 s |
| 80 x 160 | 0.669 | 1.0778 (0.0035) | 0.2492 (0.0075) | 0.8437 (0.0776) | 5.3e-14 | 7.2e-14 | 145 s |
| 160 x 320 | 0.946 | 1.0737 (0.0076) | 0.2486 (0.0069) | 0.8381 (0.0832) | 1.6e-13 | 1.7e-13 | 1234 s |

| Check | Measured | Limit | Result |
|-------|----------|-------|--------|
| B1 y_c(3) | 0.0002, 0.0035, 0.0076 (growing) | decreasing | **miss** |
| B1 V_c,max | 0.0069, 0.0075, 0.0069 | decreasing | **miss** |
| B2 | e(160) / e(40) = 45 (y_c), 1.00 (V_c,max) | <= 0.5 | **miss** (printed) |
| B3 | 0.0832 vs 0.0238 | e_t(160) < e_t(40) | **miss** (printed) |
| B4 | drift <= 1.6e-13, bound <= 1.7e-13 | 1e-12, 1e-9 | met (printed) |

H4 misses and becomes `b1h_hysing_bubble_error_decreases_under_refinement_height_function_known_miss`, ignored, limits unchanged.
F-055 stays partial. Reading: with a convergent curvature the bubble is now about 3 % too fast on every grid (V_c,max 0.249 against
0.2417, the peak 0.08 early), and y_c(3) moves down by about 0.004 per refinement without a sign of settling (differences
0.0037, 0.0041). The height-function curvature itself converges (H1 to H3), so the curvature is no longer a suspect for the
drift. The physically complete viscous stress (H5) lowers V_c,max to 0.2409 and moves the peak to 0.9225 on 80 x 160 with both
errors falling from 40 to 80, but by the plan registered before the runs it had no 160 x 320 run, so it is not a verdict.
Suggested next step (not done): the ADR 0100 series with all three options (component diagonal, height function, transposed
stress) on 40, 80 and 160, registered as its own test.

### Defaults question (scratch tree, both options forced to true in `VofSettings::new`, not committed)

| Test file | Result |
|-----------|--------|
| rcfd-fv mixture_drift, vof_advection, vof_bubble, vof_bubble_diagnosis_w41c, vof_bubble_refinement_w40d (non-ignored part), vof_dambreak, vof_dambreak_amg, vof_droplet, vof_height_function_w42b, vof_masstransfer, vof_static, vof_waves | all pass |
| rcfd-fv cavitating_venturi | both non-ignored tests fail at `VofSolver::new`: the warped mesh is refused by the height-function lattice check (the case has no surface tension) |
| rcfd-api vof_tree, vof_wiring_tree, mixture_cavitation_tree, baseline_hashes | all pass (baseline_hashes has no VOF case) |

So the defaults cannot be switched as they stand without breaking an existing test: the height-function check would have to fall
back to CSF (or be skipped) on non-Cartesian meshes and when sigma = 0. Every other existing test passes with both options on;
their printed values move (the options change the results), which is not a test change.

### Deviations

* None from the registered plan. H3 took 215 s and stays ignored by its registered rule. The H2 3-D maximum error and the 3-D
  neighbour-fallback counts were not registered limits; they are reported above.
* The 10-point indicator initial field gives a height-function curvature error that grows on 160 x 320 (S2). H4 kept it as
  registered; its effect on H4 was not isolated.

### Final gate

Fresh `git archive` of 9127771: `cargo fmt --check` clean, `cargo clippy --workspace --all-targets -- -D warnings` clean,
`cargo test --workspace --release --no-fail-fast` 1967 passed / 0 failed / 585 ignored. This wave adds H1 and H2 (not ignored) and
six ignored tests (H3, the b1h known miss, H5, two smokes).
