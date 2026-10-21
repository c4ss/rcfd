# ADR 0096: Wave 39E, partials close-out for F-143, F-052 and F-055

Status: registered before the registered tests were run. Limits are fixed; a miss becomes an `#[ignore]`d `*_known_miss` test
with the limit unchanged. No library code changes: every addition is test code (and one corrected print string), so defaults
and the baseline hashes are untouched. Clean-room: the only literature values used are the Ghia et al. (1982) Re 100 u
centreline (already verified, docs/reference-data.md) and the Hysing et al. (2009) case-1 values (read in this wave, below).
The acceptance text of every item is unchanged; rewordings are proposals only.

The residual gaps are those of [partials group 6](../audit/partials-group-6.md) (F-143, F-052) and
[partials group 8](../audit/partials-group-8.md) (F-055). I re-checked each claim before writing this ADR: `v10` in
crates/rcfd-gui/tests/viewport_raster.rs asserts only the sign of the return flow and the row of the speed maximum; tests/radiation.rs
in rcfd-api has no check below tau = 0.1; the Hysing values in crates/rcfd-fv/tests/vof_bubble.rs were marked recalled from memory.

## F-143: measurable cavity-vortex check on the raster (test file crates/rcfd-gui/tests/viewport_raster.rs)

Acceptance (unchanged): "Cavity speed field shows the lid-driven vortex; colour map and range logic unit-tested; viewport renders
headless without panic". Clauses 2 and 3 are met (ADR 0025). Clause 1 is visual. This wave adds a measurable check on the data the
viewport paints; it does not claim that a person has looked at it, so the item stays partial.

Set-up (test `v11_cavity_raster_centreline_matches_ghia_re100_and_has_one_vortex_minimum`): the GUI controller opens the built-in
lid-driven cavity template (Re 100, nu 0.01, unit lid) and, through `edit_text`, switches it to the F-028 validation set-up of
crates/rcfd-fv/tests/cavity.rs (`cavity_re100_simplec` passes the 2 % limit with it): `mesh.nx = mesh.ny = 40`,
`solver.convection = linear_upwind`, simplec (template), `solver.max_iterations = 3000`, `solver.tolerance_u = tolerance_p = 1e-7`.
The template defaults are not changed. The viewport raster is fetched with `view.cols = 40`; `field.grid` bins cell centres over
their extent, and with cols = nx each pixel holds exactly one cell (bin index floor(40 i / 39) is a bijection since 39 and 40 are
coprime). The run must report state `finished` or `converged` before sampling.

* C1 (centreline): raster ux on x = 0.5 is the mean of pixel columns 19 and 20 (cell centres 0.4875 and 0.5125, linear
  interpolation), row r at y = (39 - r + 0.5)/40, anchored by u(0) = 0 and u(1) = 1 and interpolated with the 4-point Lagrange
  rule of cavity.rs to the 17 y values of column `u_Re100` of validation-data/reference/w31c_ghia1982_u_centerline_read.csv.
  Limit: max |u - u_Ghia| <= 0.02 (2 % of lid speed, the F-028 project tolerance; the same set-up already meets it, so no new
  mesh study is needed).
* C2 (vortex): on the `speed` raster (same 40 x 40 pixels), search the pixels whose centres lie in the upper half (y >= 0.5) and at
  least 0.15 from the side walls and the lid. Limit: exactly one strict 3 x 3 local minimum there (lower than all 8 neighbours),
  and its centre y lies inside the bracket where the Ghia Re 100 centreline u changes sign (y 0.6172 to 0.7344, both rows of the
  committed CSV), widened by one pixel (0.025) on each side. The lower half is excluded because the bottom-corner eddies are
  slower than the primary vortex core. No recalled vortex-centre coordinate is used.
* The test also writes the speed raster as a PNG (scale 8) to `CARGO_TARGET_TMPDIR/viewport/f143_cavity_speed.png` for a
  human look. It asserts nothing about the image beyond C1 and C2.

Smoke (rule d, 20 iterations, before registering): all six `edit_text` calls accepted, state `finished`, raster 40 x 40 with
0 undefined pixels, row 0 mean ux 0.789 and last row mean -0.003, so row 0 is the lid and columns run along x (no transposition
from the equal x/y extents).

Proposed rewording (for the user, not applied): "Cavity raster at Re 100 (40 x 40, linear upwind) matches Ghia u centreline within
2 % of lid speed and shows one primary-vortex speed minimum in the upper half; colour map and range logic unit-tested; viewport
renders headless without panic". With that wording and C1/C2 passing, the item could close.

## F-052: optically thin limit of the P1 slab (test file crates/rcfd-api/tests/radiation.rs)

Acceptance (unchanged): "Gray enclosure benchmark matches analytic solution". S2S and discrete ordinates stay out of scope
(F-130). The thin limit is a check of the exact transport solution at tau -> 0: two infinite, diffuse, grey, isothermal plates with
a non-participating medium exchange q = sigma (T1^4 - T2^4) / (1/eps1 + 1/eps2 - 1) (radiosity balance of two surfaces with view
factor 1, derived from first principles). The medium changes the exchange by O(tau): the hemispherical absorptance of a thin slab
is about 2 tau, and the P1 solution carries 0.75 tau in the denominator. At tau = 1e-4 both P1 and exact transport sit within
about 2e-4 of the tau = 0 value.

* T1 (test `r6_p1_optically_thin_limit_matches_two_plate_exchange`): slab of R1 (L = 1, nx = 100, k = 1e-9, T1 1000 K,
  T2 500 K), absorption coefficient 1e-4 (tau = 1e-4; a = 0 is rejected by R5), no scattering, eps 1/1 and 0.8/0.5, run with
  `run_converged`. Limit: |q/q_thin - 1| <= 1e-3 for both the hot and cold wall fluxes.
* What it shows: the Marshak boundary conditions and the emissivity treatment reproduce an exact RTE limit. It does not show P1
  accuracy at intermediate tau, which only R1 (P1 against its own analytic solution) and R2 (Milne, tau = 20) touch.

Smoke (rule d, 400 iterations, before registering): both emissivity pairs construct and run without failure; wall flux finite.
No flux was compared with the reference before this ADR.

Proposed rewording (the audit's, for the user, not applied): "P1 grey slab matches the analytic P1 solution (1e-3) and the exact
transport limits: optically thin two-plate exchange and the thick Milne asymptote (1 %)". With that wording and T1 passing, the item
could close; the status stays partial under the current text because no grey enclosure with view factors exists.

## F-055: Hysing case-1 reference values (test file crates/rcfd-fv/tests/vof_bubble.rs)

Source read in this wave: S. Hysing, S. Turek, D. Kuzmin, N. Parolini, E. Burman, S. Ganesan, L. Tobiska, "Quantitative benchmark
computations of two-dimensional bubble dynamics", Int. J. Numer. Meth. Fluids 60 (2009) 1259-1288, DOI 10.1002/fld.1934 (journal
PDF, copy at karlin.mff.cuni.cz/~hron/NMMO403/1934_ftp.pdf; the MOX preprint 23/2008 has the same tables).

* Table V (group 1, TP2D, 1/h = 320): V_c,max = 0.2417 at t = 0.9213; y_c(t = 3) = 1.0813; minimum circularity 0.9013 at
  t = 1.9041. Table XI (group 3, MooNMD, finest grid): V_c,max 0.2417 at t = 0.9239, y_c(3) 1.0817. The text gives 1.081 +- 0.001
  for y_c(3) and 0.2419 +- 0.0002 for the maximum rise velocity over all groups (time 0.921 to 0.932). Both asserted numbers,
  1.0813 and 0.2417, are verified.
* Correction: the time "0.9317" printed by vof_bubble.rs and written in ADR 0049 is not in the paper; group 1 gives 0.9213
  (groups 2 and 3: 0.9313, 0.9239). The time is not asserted. The print string and docs/reference-data.md are corrected; ADR
  0049 is left as written.
* Set-up check against Table I and section 3 of the paper: rho 1000 / 100, mu 10 / 1, g 0.98, sigma 24.5, bubble radius 0.25
  centred at (0.5, 0.5), domain 1 x 2, no-slip top and bottom, free-slip sides. `bubble(true)` in vof_bubble.rs matches all of
  them. The test's rise velocity, the (1 - alpha)-weighted mean of u_y over the bubble, is the paper's U_c definition.
* Registered check: re-run `vof_bubble` and `vof_dambreak` at HEAD with the unchanged ADR 0049 limits (y_c(3) within 0.12 of
  1.0813, V_c,max within 25 % of 0.2417; dam break front speed band 1.44 to 1.94). If both pass, F-055 is set to done: its
  acceptance ("Dam break front position vs experiment; rising bubble benchmark") is then met by passing non-ignored tests with
  numeric limits against references that are now read (Hysing) or secondary-source (Martin-Moyce 1.69, still marked so).

## Addendum: measured values (after the runs)

| Check | Measured | Limit | Result |
|-------|----------|-------|--------|
| F-143 C1, raster centreline vs Ghia Re 100 | max \|u - u_Ghia\| 0.00277 at y = 0.8516; run stopped at iteration 203 of the 3000 cap, state `finished` | 0.02 | pass |
| F-143 C2, one strict speed minimum in the upper-half window, y in [0.5922, 0.7594] | two minima: (0.6125, 0.7375) speed 0.0048 and (0.3375, 0.8375) speed 0.178 | exactly one | **miss** |
| F-052 T1, tau = 1e-4, eps 1/1 | 7.50e-5 (hot and cold wall; 800 iterations) | 1e-3 | pass |
| F-052 T1, tau = 1e-4, eps 0.8/0.5 | 3.33e-5 (1400 iterations) | 1e-3 | pass |
| F-055 bubble, free-slip sides | y_c(3) 1.0282 (ref 1.0813), V_c,max 0.2204 at t 0.850 (ref 0.2417) | +-0.12, 25 % | pass |
| F-055 dam break | v/sqrt(gH) 1.504 | 1.44 to 1.94 | pass |

F-052: the deviation equals the P1 term 0.75 tau / (1/eps1 + 1/eps2 - 1) to the printed digits (7.5e-5 and 3.33e-5), so the
discretisation adds nothing visible at this tau.

F-143 C2 miss: the primary-vortex minimum is where C2 expects it (inside the Ghia u sign-change bracket, speed 0.0048, the lowest
pixel in the window). The window also contains a second strict 3 x 3 minimum at (0.3375, 0.8375), speed 0.178, in the shear layer
below the lid on the upstream side. I did not anticipate a local speed minimum outside the vortex core when I chose the window.
Whether it is a property of the exact flow or of the 40 x 40 discretisation was not investigated. Narrowing the window now would
be tuning, so there is no corrected C2.

Deviation from the registered plan: C1 and C2 were registered as one test. After the run I split them into
`v11_cavity_raster_centreline_matches_ghia_re100` (passes) and the ignored `v11_cavity_raster_has_one_vortex_speed_minimum_known_miss`,
which share one run helper. Neither limit changed.

Look at the picture: the test writes `f143_cavity_speed.png` (320 x 320) under the test target's `CARGO_TARGET_TMPDIR/viewport`.
The agent opened it. It shows the lid band at the top, a dark core at about (0.6, 0.74), the faster down-flow along the right
wall, and dark bottom corners. This is an agent's look, not the human look that clause 1 asks for.

Status: F-055 done (acceptance met; the reference values are now read). F-143 and F-052 stay partial under their current
acceptance text; the rewordings proposed above are the user's decision.
