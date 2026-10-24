# ADR 0114: Wave 44D, the VOF defaults flip together: component diagonal, full viscous stress, height-function curvature with CSF fallback (F-055)

Status: registered before any code of this wave was written. Limits and decision rules below are not changed after a run; a missed
limit becomes an ignored `*_known_miss` test with the limit intact; a defective set-up gets a new corrected test registered here as an
amendment before it runs. Written at HEAD e868916, target `<target dir>`. Clean-room: the only literature values are
the Hysing et al. (2009) case-1 values read in ADR 0096 and the code-to-code range of the reworded F-055 acceptance (user, 2026-10-24,
Hysing et al. 2009 Table XII, read by the user).

## Decision (the user's, 2026-10-24)

The user lifted the bit-identical-default rule for this one change and decided (F-055 note, "USER DECISION 2026-10-24 ... Decided: VOF
defaults flip together (wave 44D)") that the three VOF opt-ins of ADRs 0103, 0106 and 0108 become the defaults together, with the
ADR 0108 CSF fallback built in.

* Library, `VofSettings::new`: `component_diagonal = true`, `full_viscous_stress = true`, `height_function = true`,
  `height_function_csf_fallback = true`. So on a uniform axis-aligned Cartesian grid with `sigma > 0` the curvature is the height
  function; on any other mesh, and whenever `sigma <= 0`, it is the CSF estimator exactly as before (ADR 0108 F1-F4). The old
  behaviour is reached by setting the fields to `false`; `height_function = true` with `height_function_csf_fallback = false` is the
  strict ADR 0106 mode (refusal of other meshes). `harmonic_viscosity` stays `false`.
* Case tree, new keys under `multiphase` (inert while `multiphase.enabled = false`: a non-default value is then a located error, as
  for the other `multiphase.*` keys):
  * `multiphase.component_diagonal`: `"corrected"` (default, `component_diagonal = true`) or `"averaged"` (opt-out);
  * `multiphase.viscous_stress`: `"full"` (default, `full_viscous_stress = true`) or `"laplacian"` (opt-out);
  * `multiphase.curvature`: `"height_function"` (default, `height_function` and `height_function_csf_fallback` both true) or
    `"csf"` (opt-out, both false).
  The spellings are those of `solver.component_diagonal` and `solver.viscous_stress`. Those two global keys are NOT flipped and NOT
  reused: they act on the single-phase kernels, and a flip of `solver.component_diagonal` moves the `packed_bed` baseline hash
  (docs/component-diagonal-default.md). `solver.viscous_stress = "full"` stays a located error with multiphase (same path); its
  message now names `multiphase.viscous_stress`. The tree already took every unset VOF flag from `VofSettings::new`, so without the
  new keys the tree follows the library flip.
* `crates/rcfd-api/tests/baseline_hashes.rs` has no VOF case and is never re-recorded. If any hash moved, this wave stops and reports.

## (1) Existing tests whose printed or asserted value changes under the new defaults

Method (done before this ADR): two fresh archives of e868916; in the second the four `VofSettings::new` defaults were set to `true`
(nothing else changed, never committed). In each, every VOF-related test file was run in release with `--nocapture`: rcfd-fv
cavitating_venturi, mixture_drift, vof_advection, vof_bubble, vof_bubble_diagnosis_w41c, vof_bubble_refinement_w40d, vof_dambreak,
vof_dambreak_amg, vof_droplet, vof_height_function_fallback_w43a, vof_height_function_w42b, vof_masstransfer, vof_static, vof_waves;
rcfd-api vof_tree, vof_wiring_tree, mixture_cavitation_tree, baseline_hashes (the files with `VofSettings`, `VofSolver` or
`multiphase` in `crates/*/tests`; no `src` unit test builds a VOF solver). The printed lines (timings removed) were compared.

Result: with the new defaults every non-ignored test of these files passes (same pass / ignore counts as with the old defaults). No
asserted limit fails, so rule (2) below re-pins nothing. Tests whose printed value changes (old -> new):

| File / test | Printed value, old -> new defaults | Asserted limit |
|-------------|-------------------------------------|----------------|
| vof_bubble `e_rising_bubble_free_slip_sides` | y_c(3) 1.0282 -> 1.0794, V_c,max 0.2204 -> 0.2402, peak t 0.850 -> 0.940, dV 4.8e-15 -> 6.0e-15, bound 7.4e-14 -> 3.5e-13 | +-0.12, 25 percent, 1e-12, 1e-9: met |
| vof_bubble `e_rising_bubble_no_slip_sides_labelled` | y_c(3) 1.0161 -> 1.0124, v_max 0.2148 -> 0.2074, peak t 0.852 -> 0.915 | drift / bound only: met |
| vof_droplet `c_static_droplet` | Laplace error 4.810e-2 -> 3.665e-3, Ca_max 1.642e-3 -> 7.793e-4, centroid drift 1.5e-12 -> 3.3e-8 dx | 0.10, 1e-2, 0.01 dx: met |
| vof_static `b1_interface_on_a_face` | max abs U 1.711e-12 -> 1.841e-12, pressure error 9.7e-12 -> 1.67e-11, alpha change 2.6e-14 -> 1.49e-13 | 1e-10, 1e-10, 1e-12: met |
| vof_static `b2_interface_inside_a_cell` | max abs U (sigma 0) 5.86e-13 -> 5.30e-13, (sigma 0.07) 4.43e-13 -> 5.30e-13 | 1e-10: met |
| vof_dambreak `d_dam_break_front` | D1 1.504, D2 0.161 / 0.110 unchanged at the printed digits; D3 dV 4.54e-15 -> 4.90e-15, alpha min -4.8e-18 -> -1.0e-17 | met |
| vof_dambreak_amg `a4_dam_break_with_amg` | dV 5.63e-15 -> 5.26e-15, front(AMG) - front(CG) 3.7e-15 -> 1.7e-14 m; D1, D2, iterations unchanged | met |
| vof_bubble_diagnosis_w41c `c1_uniform_channel_symmetry_walls` | `component_diagonal = false` branch: wall-column error 1.680e-1 -> 1.735e-1 (transposed stress now on); `true` branch unchanged 2.5e-16 | the `true` branch limit: met |
| mixture_drift `d1_conservation_closed_column_corrected` | mass 2.4e-16 -> 0, alpha max 0.989407182068 -> 0.989407182070, max abs u 5.81e-12 -> 5.69e-12 | 1e-12, 1e-9: met |
| mixture_drift `m4b_mixture_mass_balance_per_cell` | 7.89e-15 -> 8.31e-15 | 1e-12: met |
| vof_masstransfer `m2_phase_mass_conservation`, `m4_superheated_layer_continuity` | identity residual 9.2e-16 -> 5.0e-16, mass 1.2e-15 -> 6.1e-16; flux error 2.07e-13 -> 1.94e-13 | 1e-12, 1e-6: met |
| cavitating_venturi `n1_single_phase_bernoulli_and_inception_pressure` | p_in 9863.9 -> 9864.0, p_throat 99.3 -> 99.4 | met |
| cavitating_venturi `n2_to_n6_cavitating_venturi_corrected` | N2 alpha_v max 0.999925166 -> 0.999926631; N6 cavitating throat minimum 50456.4 -> 2851.4 Pa (limits: above the non-cavitating -1349.9 and >= -786.0); printed known-miss list N4 31763 -> 32833, N4' 16 -> 19, N5 1.958e-5 -> 4.888e-6 (still above 1e-6); max flow Courant 0.512 -> 0.517 | asserted N2, N3 (x < 0.05), N6: met |
| vof_waves `w1_w3_generation_and_beach`, `w2b_beach_long_window`, `w4b_still_water_with_zone_four_correctors` | G2 period 0.9951 -> 0.9952 s, volume identities 5.8e-14 / 6.2e-14 -> 5.6e-14 / 5.8e-14; W4b max abs U 8.3e-12 -> 1.0e-11, alpha change 2.1e-13 -> 2.3e-13 | met |
| rcfd-api vof_tree `f2_f3_hydrostatic_pool_through_the_tree` | max abs U 1.711e-12 -> 1.848e-12, tree vs library p 3.9e-12 -> 5.4e-12 | 1e-10, 1e-9: met |
| rcfd-api vof_tree `f2_dam_break_through_the_tree` | tree vs library front 6.3e-14 -> 2.1e-13, alpha 2.3e-12 -> 2.6e-12, U 1.1e-12 -> 3.5e-12; D1 1.504 unchanged | 1e-9, D1 band: met |
| rcfd-api vof_wiring_tree `t2_...`, `t3_...` | tree vs library p 2.05e-11 -> 1.85e-11, 5.9e-15 -> 2.8e-15; max ux 5.832e-2 -> 5.831e-2 | 1e-9: met |
| rcfd-api mixture_cavitation_tree W2, W3 rows | tree vs library alpha 2.2e-19 -> 3.3e-19, 6.6e-13 -> 9.9e-13 | met |

Unchanged to every printed digit: vof_advection (no `VofSolver`), vof_bubble_diagnosis_w41c `c1` with `component_diagonal = true`,
vof_height_function_fallback_w43a F1-F4 (equality tests, they set both height-function flags explicitly and compare solvers built
from the same other defaults), vof_height_function_w42b H1 and H2 (curvature of the initial field, both flags explicit),
mixture_drift D2-D6, M1-M3, vof_masstransfer M1, M3, M5, vof_waves W5 and the W2b control, the located-error rows of vof_tree, vof_wiring_tree and mixture_cavitation_tree, and rcfd-api baseline_hashes (3 passed, 1 ignored as before: no hash moved).

Ignored tests were not run (the heavy series cost hours). By reading the code, these ignored tests construct `VofSettings::new`
without setting all four flags, so they now run under the new defaults and would not reproduce the numbers their ADRs recorded with
the old defaults: cavitating_venturi `n2_to_n6_cavitating_venturi`, `n3_n4_n5_cavitating_venturi_known_miss`; mixture_drift `d1`,
`d4`, `d5`, `d6` (the registered unstable set-ups); all 20 ignored diagnostics of vof_bubble_diagnosis_w41c (ADR 0103; their `edit`
closures set single flags on top of the defaults, so "baseline" now means all options on); vof_height_function_w42b `h3`, `s1`
(height function explicit; component diagonal and transposed stress now on); vof_waves `w2_..._known_miss`, `w4_..._known_miss`.
They are not edited (rule (2) concerns asserted limits that fail in the gate; these are not run there). To reproduce their recorded
numbers, set the four flags to `false`. vof_bubble_refinement_w40d sets `full_viscous_stress`, `component_diagonal` and
`height_function` explicitly in every test; the fallback flag acts only with `height_function = true`, and on its uniform grid with
`sigma > 0` it changes nothing (ADR 0108 F3), so all its tests are unaffected.

## (2) Rule for asserted limits

Any ASSERTED limit of an existing test that fails under the new defaults is either (a) re-pinned by an explicit opt-out in that test
(the set-up is otherwise unchanged, the limits stay as written, never loosened, and the reason is written in this ADR), when the
test's purpose is a property of the old configuration; or (b) reported as a known miss (ignored `*_known_miss`, limit intact) when the
test checks a property the default solver should have. Per (1) no test needs either; if the gate shows a failure that the scan did not,
this rule applies and the case is written in the addendum.

## (3) The Hysing series with DEFAULT settings reproduces wave 43A

Set-up: the ADR 0108 X set-up (`vof_bubble_refinement_w40d.rs`: n x 2n, n = 40, 80, 160, dt = 0.1/n, AMG pressure, free-slip
sides, no-slip top and bottom, 3 PISO correctors, exact initial fractions) with NO option flag assigned (the library defaults).

Reference: the full-precision errors printed by the ADR 0108 X run (`B errors` line of its log, `<scratch dir>\x.log`):
y_c(3) errors to 1.0813: 0.002053058745789871, 0.0012840785484355965, 0.0018457966779672486 (y_c(3) = 1.0792, 1.0800, 1.0795);
V_c,max errors to 0.2417: 0.0015273890756644926, 0.0007790117810689556, 8.49155348201236e-5 (V_c,max = 0.2402, 0.2409, 0.2416).
Both lie below the reference on every grid. No VOF-path source changed since that run (`git log 66f5922..e868916` touches no file of
`rcfd-fv/src` used by the VOF solver; `simple.rs`, `sources.rs` and the mesh joins changed, which the VOF path does not call).

Limit: on each grid |e - e_43A| <= 1e-9 for the y_c(3) error and the V_c,max error. A mismatch is a defect to diagnose (the defaults
are not the 43A options), never a reason to loosen the limit.

Tests (`vof_bubble_refinement_w40d.rs`):

* `x0_default_settings_reproduce_adr0108_x_40` (not ignored, about 20 s): the 40 x 80 point.
* `x0_default_settings_reproduce_adr0108_x_series` (ignored, heavy, about 25 min, run once in this wave): 40, 80, 160, also prints
  B1-B4 (not asserted: the ADR 0108 known miss stands as recorded).

And `d_dam_break_front` (`vof_dambreak.rs`, not ignored, ADR 0049 D1-D3 limits unchanged) passes with the new defaults (scan: D1
1.504, D2 0.161 / 0.110, as before).

Smoke (rule d): the 40 x 80 point with no flag assigned, run in the scratch archive with the four defaults flipped, gives errors 0.002053058745789871 (y_c), 0.0015273890756644926 (V_c,max) and 0.016199999999991 (peak time), equal to the 43A values in every printed digit; 17 s.

## (4) The reworded F-055 bubble clause, default settings, finest grid

Acceptance (user text, not edited): on the finest of three grids y_c(3) in [1.0799, 1.0817] and V_c,max in [0.2417, 0.2421], phase
volume conserved to round-off; time of V_c,max reported against [0.9213, 0.9313], not a limit.

Test `f055_hysing_bubble_finest_grid_in_published_range_known_miss` (`vof_bubble_refinement_w40d.rs`): the (3) set-up (default
settings, exact initial fractions) on 160 x 320 to t = 3. Asserted: 1.0799 <= y_c(3) <= 1.0817; 0.2417 <= V_c,max <= 0.2421; phase
volume drift <= 1e-12 relative (the ADR 0100 B4 round-off limit) and bound violation <= 1e-9. Printed: time of V_c,max and whether it
lies in [0.9213, 0.9313]. Expected from (3) (the same computation as the 160 point): y_c(3) 1.0795 and V_c,max 0.2416, both below
the bands, so it is registered ignored with the `_known_miss` suffix from the start. It is run once in this wave (in parallel with
the (3) series), limits intact, nothing tuned. F-055 stays partial unless this test passes; its acceptance text is not edited, only
its note is extended.

## (5) Tree keys

Test `f5_vof_option_keys_reach_the_library` (`crates/rcfd-api/tests/vof_tree.rs`, not ignored): the F2 dam break (120 x 40) with
`multiphase.surface_tension = 0.07`, 20 steps of 1.5e-3, through the tree and through the library:

* tree without the new keys vs library `VofSettings::new` defaults: max alpha and U difference <= 1e-9 (the F2 limit);
* tree with `component_diagonal = "averaged"`, `viscous_stress = "laplacian"`, `curvature = "csf"` vs library with the four flags
  `false`: <= 1e-9;
* the two library runs differ by more than 1e-9 in alpha or U (the keys act in this set-up).

Plus three rows in `f1_located_errors`: each new key set to its opt-out value while `multiphase.enabled = false` is a located error at
its own path. Smoke: the library part of the set-up (default vs all flags off, 20 steps) gives TREE_the 40 x 80 point with no flag assigned, run in the scratch archive with the four defaults flipped, gives errors 0.002053058745789871 (y_c), 0.0015273890756644926 (V_c,max) and 0.016199999999991 (peak time), equal to the 43A values in every printed digit; 17 s., so the third
check can be met; if the gate showed otherwise the set-up is defective (amendment before a new run).

## Documentation

docs/case-schema.md (multiphase table and the located-error list), the `rcfd_fv::vof` module documentation and the `VofSettings`
field documentation, the schema doc strings of `multiphase.surface_tension` and `multiphase.curvature_smoothing`, and the VOF row of
docs/component-diagonal-default.md.
