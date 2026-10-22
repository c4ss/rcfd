# ADR 0108: Wave 43A, Hysing refinement with all three VOF options and exact initial fractions; CSF fallback of the height function (F-055)

Status: registered before any code and any run. Limits are fixed here; a miss becomes an ignored `*_known_miss` test with the limit
intact; a defective set-up gets an amendment written before the corrected run. Results go into the addendum. Written at HEAD 80c26e9
(scratch copy `<scratch dir>`, target `<target dir>`). Clean-room: first principles and the Hysing et al.
(2009) case-1 values read in ADR 0096 (V_c,max 0.2417 at t 0.9213, y_c(3) 1.0813). Context: ADR 0100 (refinement check B1-B4),
ADR 0103 (component-diagonal wall defect), ADR 0106 (height-function curvature; H4 verdict missed; H5 with all three options on 40
and 80 only).

## What I already know when writing this (rule c)

* `crates/rcfd-fv/src/vof.rs` has not changed since ADR 0106's runs (the only commit touching it after 893d87c is d48ad57, the
  height-function commit itself), so the control series below runs the same code path as ADR 0106 H5. Its 40 x 80 and 80 x 160
  points are therefore already published: y_c(3) 1.0794 / 1.0800, V_c,max 0.2402 / 0.2409, peak 0.9400 / 0.9225 (both errors
  falling from 40 to 80). No 160 x 320 run of any all-three-options variant exists.
* ADR 0106 S2: the height-function curvature of the 10-point indicator initial field has rms error 0.209 / 0.126 / 0.740 on
  40 x 80 / 80 x 160 / 160 x 320 (the `eps / h` growth of indicator fraction errors). Its effect on the bubble was not isolated.
* No t = 3 value of any exact-fraction rising-bubble run exists.

## Part 1: the Hysing series with all three options (test file `crates/rcfd-fv/tests/vof_bubble_refinement_w40d.rs`)

Set-up: the ADR 0100 set-up unchanged (n x 2n, n = 40, 80, 160, dt = 0.1/n, AMG pressure, free-slip sides, no-slip top and
bottom, 3 PISO correctors) with `component_diagonal = true`, `height_function = true`, `full_viscous_stress = true`. Two initial
fields, the only difference between the two series:

* X (exact fractions): alpha = 1 - A/h^2, with A the exact area of the disc (R = 0.25 at (0.5, 0.5)) inside the cell, from the
  antiderivative of `sqrt(R^2 - x^2)` split at the breakpoints (the `disc_rect` function of `vof_height_function_w42b.rs`, verified
  there by H1 to round-off; copied, not made a library API). The set-up asserts the phase-2 volume sum A = pi R^2 to relative 1e-10.
* XC (control): the 10-point indicator field of ADR 0100 (`indicator_fraction(&geom, 10, ..)`).

Limits: ADR 0100 B1-B4 unchanged (B1 e_y and e_v decrease strictly 40 -> 80 -> 160; B2 e(160) <= 0.5 e(40) for both; B3
e_t(160) < e_t(40); B4 drift <= 1e-12 and bound violation <= 1e-9 on every grid), asserted in that order after all three grids are
printed (the existing `b1` helper).

Decision rules (fixed now):

* X is the verdict: `b1x_hysing_bubble_error_decreases_under_refinement_all_options_exact_fraction`. If it passes all of B1-B4 it is
  kept not ignored (about 30 min more in the release gate, the ADR 0106 precedent). If it misses it becomes `..._known_miss`,
  ignored, limits intact.
* XC is a control and cannot change the verdict or F-055's status, whatever it shows (even if it passes and X misses). It runs the
  same B1-B4 asserts; it is ignored in every outcome (a passing control would double the gate cost for no new claim); a miss gets the
  `_known_miss` suffix. Reading registered now: per grid, X minus XC in y_c(3), V_c,max and peak time is the effect of the initial
  field; "the initial field explains the ADR 0106 drift" is said only if X meets B1 and XC does not.
* Order: both series run in full once (40, 80, 160 each), X first, one cargo process at a time. Each series is expected to take
  25 to 35 min (ADR 0106: H4 needed 1234 s on 160 x 320; the transposed stress adds a gradient pass per outer iteration). A series
  that exceeds 60 min or is not finite on 160 x 320 is a defect: the ADR 0100 fallback series (n = 40, 56, 80) is registered for
  that case, with an amendment before it runs.
* F-055 is set done only if X passes all of B1-B4 and stays not ignored, and `d_dam_break_front` (vof_dambreak.rs) and
  `e_rising_bubble_free_slip_sides` (vof_bubble.rs, the 40 x 80 check) still pass not ignored. The note then says that the
  refinement passes with the three opt-ins only and that the defaults still miss (b1 known miss). No option becomes a default in any
  outcome (the user decides). Otherwise F-055 stays partial and its note is extended.

### Smoke (rule d), run before any t = 3 run; rules fixed now

* S1 construction of X on 40 x 80, 80 x 160, 160 x 320 with the phase-volume assert; printed: the height-function curvature error
  of the initial field (rms, max of `-kappa/4 - 1` over 0.05 < alpha < 0.95) and the fallback counts, next to ADR 0106 S2.
* S2 both series on 160 x 320 to t = 0.1 (160 steps), and X on 40 x 80 to t = 0.05: finite, max|U|, drift, bound. No y_c(3),
  V_c,max or reference value is looked at.
* Rule: if a set-up fails to construct, fails the phase-volume assert, is not finite, or has max|U| > 1.0 at t = 0.1 (four times
  V_ref, a blow-up sign), it is a defect: amendment before any t = 3 run of that series.

## Part 2: CSF fallback of the height function (code for a later default switch; no default changes)

ADR 0106's defaults question found the blocker of a default switch: with `height_function` forced on, `cavitating_venturi.rs`
fails at `VofSolver::new` because its warped mesh is refused by the lattice check, although that case has no surface tension.

Code: new `VofSettings::height_function_csf_fallback: bool` (default `false`; set before `VofSolver::new`). It acts only together
with `height_function = true`: then `VofSolver::new` does not build the lattice when `sigma <= 0` (the curvature is never used
without surface tension) and, when the mesh is not a uniform axis-aligned grid, keeps the CSF estimator instead of returning
`InvalidValue`. `curvature()` already returns the CSF value when no lattice exists. With the option off, nothing changes (the strict
refusal stays, defaults bit-identical, baseline hashes untouched). A separate bool, not an enum, so every existing
`height_function = true` assignment keeps its meaning.

Registered tests (`crates/rcfd-fv/tests/vof_height_function_fallback_w43a.rs`, not ignored, exact equality, no tolerance):

* F1 warped mesh (the `cavitating_venturi.rs` style `warp` of a `box_2d`), sigma > 0, a circle interface: `height_function` alone is
  refused by `new` (unchanged); with the fallback `new` succeeds, `curvature()` equals the `height_function = false` solver's bit for
  bit, `curvature_fallbacks()` is `[0, 0]`, and the fields after 3 steps equal the plain solver's bit for bit.
* F2 the same warped mesh with sigma = 0: with the fallback `new` succeeds and the fields after 3 steps equal the plain solver's bit
  for bit.
* F3 uniform Cartesian grid, sigma > 0: with the fallback, `curvature()` and `curvature_fallbacks()` equal the plain
  `height_function = true` solver's bit for bit (the fallback does not change the Cartesian path).
* F4 Cartesian grid, sigma = 0: with the fallback the fields after 3 steps equal the plain solver's bit for bit.

Report only (scratch tree, never committed): set `component_diagonal`, `height_function`, `height_function_csf_fallback` and
`full_viscous_stress` to `true` in `VofSettings::new` and run the ADR 0106 file list in release (rcfd-fv: vof_*.rs,
mixture_drift.rs, cavitating_venturi.rs; rcfd-api: vof_tree.rs, vof_wiring_tree.rs, mixture_cavitation_tree.rs,
baseline_hashes.rs); per file pass / fail. Target: cavitating_venturi passes.

## Part 3: the dam-break clause of the acceptance ("Dam break front position vs experiment")

Settled now, before the runs, from `docs/reference-data.md` (section Wave 27A) and ADR 0049 part D: the only published dam-break
data in the reference file are the Martin and Moyce non-dimensional average front velocities `v / sqrt(g H)` = 1.48 (H = 57 mm) and
1.69 (H = 114 mm), averaging window t* > 1, from Table 1 of a 2014 secondary source (arXiv:1308.0115, page read); the primary
Z-T table was not obtained and the original time normalisation was not resolved. The test geometry is the H = 114 mm column
(width a = 57.15 mm, H = 2a), so ADR 0049 maps it to 1.69.

* Feasible with these data, and already in place and not ignored: a front test against the average front velocity. ADR 0049 D1
  (front speed between t* = 1 and 3 within 15 percent of 1.69 and below Ritter's 2; measured 1.504) and D2 (front position
  `x_f(t*) - x_f(1)` within 20 percent of the 1.69 line at t* = 2 and 3; measured 0.161 and 0.110), `d_dam_break_front` in
  `crates/rcfd-fv/tests/vof_dambreak.rs`.
* Not feasible without new data: a pointwise front-position-versus-time comparison with the experiment. No Z-T table is in the
  reference file, and none is invented here.

This clause was judged met by the ADR 0096 status decision (the user set F-055 back to partial for the bubble refinement only). This
wave does not change it; it checks that `d_dam_break_front` still passes.

## Smoke results (S1, S2; written before any t = 3 run)

* S1 construction of X: the phase-volume assert passes on 40 x 80, 80 x 160, 160 x 320. Height-function curvature error of the
  exact initial field: rms 5.01e-3 / 1.22e-3 / 3.19e-4, max 6.85e-3 / 1.75e-3 / 4.78e-4, no fallback on any grid (order 2; the
  indicator field of ADR 0106 S2 gives rms 0.209 / 0.126 / 0.740).
* S2 X on 40 x 80 to t = 0.05: finite, max|U| 0.038, drift 3.1e-16, bound 4.9e-15. X on 160 x 320 to t = 0.1 (35 s): finite,
  max|U| 0.063, drift 4.9e-15, bound 1.5e-13. XC on 160 x 320 to t = 0.1 (41 s): finite, max|U| 0.063, drift 4.5e-15, bound
  1.4e-13. Every set-up constructs and runs; by the smoke rule no amendment. Measured rate about 0.22 to 0.26 s per step on
  160 x 320, so a 160 x 320 run to t = 3 is about 20 min.
* Part 2 tests F1-F4 were written and pass (4 of 4, 0.04 s); they are equality tests, so no limit was set after a run.

## Addendum: measured values (after the runs)

All runs once, release build, scratch tree (archive of 80c26e9 plus this wave's files). Errors are to Hysing 1.0813 / 0.2417 /
0.9213.

### Part 1, X (verdict): all three options, exact initial fractions

| Grid | dt / dt_cap | y_c(3) (err) | V_c,max (err) | t of V_c,max (err) | drift | bound | run time |
|------|-------------|--------------|---------------|--------------------|-------|-------|----------|
| 40 x 80 | 0.473 | 1.0792 (0.0021) | 0.2402 (0.0015) | 0.9375 (0.0162) | 5.9e-15 | 9.3e-14 | 20 s |
| 80 x 160 | 0.669 | 1.0800 (0.0013) | 0.2409 (0.0008) | 0.9225 (0.0012) | 5.1e-14 | 8.0e-14 | 139 s |
| 160 x 320 | 0.946 | 1.0795 (0.0018) | 0.2416 (0.0001) | 0.9175 (0.0038) | 1.6e-13 | 1.7e-13 | 1194 s |

| Check | Measured | Limit | Result |
|-------|----------|-------|--------|
| B1 y_c(3) | 0.0021, 0.0013, 0.0018 | strictly decreasing | **miss** (160 above 80) |
| B1 V_c,max | 0.0015, 0.0008, 0.00008 | strictly decreasing | met |
| B2 y_c(3) | e(160) / e(40) = 0.90 | <= 0.5 | **miss** (printed) |
| B2 V_c,max | e(160) / e(40) = 0.056 | <= 0.5 | met (printed) |
| B3 | 0.0038 vs 0.0162 | e_t(160) < e_t(40) | met (printed) |
| B4 | drift <= 1.6e-13, bound <= 1.7e-13 | 1e-12, 1e-9 | met (printed) |

X misses B1 and B2 on y_c(3). It becomes `b1x_..._all_options_exact_fraction_known_miss`, ignored, limits unchanged. F-055 stays
partial.

### Part 1, XC (control): all three options, 10-point indicator field

| Grid | y_c(3) (err) | V_c,max (err) | t of V_c,max (err) | drift | bound | run time |
|------|--------------|---------------|--------------------|-------|-------|----------|
| 40 x 80 | 1.0794 (0.0019) | 0.2402 (0.0015) | 0.9400 (0.0187) | 5.2e-15 | 8.5e-14 | 38 s |
| 80 x 160 | 1.0800 (0.0013) | 0.2409 (0.0008) | 0.9225 (0.0012) | 5.4e-14 | 7.8e-14 | 163 s |
| 160 x 320 | 1.0794 (0.0019) | 0.2416 (0.0001) | 0.9175 (0.0038) | 1.5e-13 | 1.7e-13 | 1212 s |

XC misses B1 on y_c(3) the same way (0.0019, 0.0013, 0.0019); it is `b1xc_..._control_known_miss`, ignored as registered. Its 40
and 80 points equal ADR 0106 H5 to the printed digits (reproducibility of the code path).

### Reading

* Effect of the initial field (X minus XC): y_c(3) -0.0002, 0.0000, +0.0001 and V_c,max below 1e-4 on 40, 80, 160; peak time
  -0.0025 on 40 and 0 on 80 and 160. The initial field has no measurable effect on the bubble with these options, although it
  changes the initial curvature error by a factor 40 to 2300 (S1 against ADR 0106 S2). By the registered rule the initial field
  does not explain anything (X does not meet B1).
* Errors of X per metric: y_c(3) 0.19, 0.12, 0.17 percent; V_c,max 0.62, 0.32, 0.04 percent; peak time 1.8, 0.13, 0.41 percent
  (40, 80, 160). V_c,max falls on every refinement. y_c(3) varies by at most 0.0008 across the three grids (differences +0.0008,
  -0.0005, which change sign, so no order is read from them) and sits 0.0013 to 0.0021 below Hysing's TP2D 1.0813, which is 3 to
  5 times the 0.0004 difference between the TP2D and MooNMD (1.0817) values read in ADR 0096. B1 and B2 on y_c(3) miss; the
  limits stay as written. Whether this result is acceptable is the user's decision, not taken here.
* Against ADR 0106 H4 (component diagonal and height function without the transposed stress, same indicator field as XC): on
  160 x 320 the transposed viscous stress moves V_c,max from 0.2486 to 0.2416 (the 3 percent over-speed is gone) and y_c(3) from
  1.0737 to 1.0794. So the offset of the defaults (ADR 0100) is accounted for, to the size above, by three opt-ins: the wall defect
  (ADR 0103), the non-convergent CSF curvature (ADR 0106) and the missing transposed stress.

### Part 2, fallback

F1-F4 pass (not ignored). Report-only run in a scratch copy with `component_diagonal`, `height_function`,
`height_function_csf_fallback` and `full_viscous_stress` all forced to `true` in `VofSettings::new` (not committed), release:

| Test file | Result |
|-----------|--------|
| rcfd-fv cavitating_venturi | 2 passed, 2 ignored (was: both non-ignored tests fail at `VofSolver::new` in ADR 0106) |
| rcfd-fv mixture_drift, vof_advection, vof_bubble, vof_bubble_diagnosis_w41c, vof_dambreak, vof_dambreak_amg, vof_droplet, vof_height_function_fallback_w43a, vof_height_function_w42b, vof_masstransfer, vof_static, vof_waves | all pass (vof_waves took 642 s under parallel load; its time with the defaults was not measured in this run) |
| rcfd-api vof_tree, vof_wiring_tree, mixture_cavitation_tree, baseline_hashes | all pass (baseline_hashes has no VOF case) |

So with the fallback, the four options can be switched on together without breaking an existing test. No default was changed.

### Part 3

As registered: the dam-break clause rests on the Martin-Moyce average front velocity (1.69, secondary source), the only published
dam-break data in `docs/reference-data.md`; `d_dam_break_front` passes not ignored in the final gate. A pointwise front-versus-time
test is not feasible with the data in the file.

### Deviations

* None from the registered plan. The first launch of X used `--exact` with a partial test name and ran no test (0 tests, no
  output); it was relaunched with the full name. No series was run twice.
* `vof_bubble.rs` `e_rising_bubble_free_slip_sides` and `vof_dambreak.rs` `d_dam_break_front` were not re-run separately: they are
  unchanged (defaults bit-identical) and are part of the final gate.

### Final gate

Fresh `git archive` of 66f5922 (which includes the wave 43B / 43C commits before it): `cargo fmt --check` clean, `cargo clippy
--workspace --all-targets -- -D warnings` clean, `cargo test --workspace --release --no-fail-fast` 1983 passed / 0 failed / 619
ignored. This wave adds F1-F4 (not ignored) and three ignored tests (b1x and b1xc known misses, the smoke); the rest of the change
from 1967 / 585 belongs to the other wave-43 agents. `d_dam_break_front` and `e_rising_bubble_free_slip_sides` pass. The wording
corrections of the Reading section and of the F-055 note were made after the gate (documentation only).
