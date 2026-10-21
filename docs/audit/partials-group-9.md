# Partials audit, group 9

Read-only audit of ten roadmap items with status `partial` (docs/roadmap.json at 01948a9). Grades follow the acceptance text as
written. Tests were re-run by the auditor (release build, own target directory, Windows, two test threads) unless marked "read
only". The earlier notes were not trusted. Where a note and a measurement disagree, the measurement is recorded. Every failure seen
under `--include-ignored` was checked to be an `#[ignore]`d known-miss or set-up-defect test. No non-ignored test failed.

Grades: DONE = acceptance met by a passing, non-ignored (or reproducibly run) test with numeric limits; PARTIAL = a tested slice
exists and a named clause is unmet; TODO = nothing usable; OVERSTATED = the note claims more than the tests show.

| id | title (short) | grade | evidence run or read | residual gap against the acceptance | size | blockers | one wave | user decision |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| F-042 | k-epsilon family, scalable wall functions | PARTIAL (note stale on scalable wiring) | `rcfd-turb --test channel_kepsilon` (3 pass), `--test step_kepsilon_corrected -- --ignored step_c_reattachment` (RNG pass, Standard and Realizable known misses, 101 s), `rcfd-api --test wall_options` (5 pass) | channel "profile" compared with DNS only at centre-line and bulk values. The profile is checked against Reichardt's law, not DNS. Step reattachment: only RNG is inside 6.26 +- 0.10 (6.334, marginal). Standard 5.350 is outside even the uncited 5.5-7 band | S (DNS profile points) / M (energy, moving-wall sub-criteria) | Re_tau 395 DNS profile data (not in validation-data); acceptance wording per variant (user) | yes for the channel clause, no for the rest | yes |
| F-056 | Lagrangian particle tracking | PARTIAL | `rcfd-particles --test closed_forms` (24 pass, 3 known misses ignored), `--test lagrangian_ext` (13 pass, 1 set-up defect ignored), `rcfd-api --test particle_run` (9 pass) | no cyclone separation case of any kind | M | none for a prescribed-vortex sanity case; a real 3-D cyclone needs a swirl-capable turbulence model and mesh | yes (prescribed-flow case) | yes (what "sanity" means) |
| F-096 | docs site, tutorials, gallery | PARTIAL | `rcfd-cli --test docs_site` (129 pages, 192 relative links, 0 broken), `rcfd-cli --test tutorials --include-ignored` (6 pass, 64 s), `rcfd-mcp --test tutorials --include-ignored` (3 pass, 14 s) | "executed in CI": no hosted CI run. The tutorials postdate the last remote commit. Windows only. GUI and sidebar steps are `not-executed` | S (once pushed) | push to the remote (user decision); deps F-094, F-150 partial | yes, if the user pushes | yes |
| F-117 | glTF/GLB export | PARTIAL | `rcfd-post --test gltf` (4 pass), `--test glb_check` (11 pass) | no viewer shown with correct colours. The Khronos validator result (0 errors) is recorded only: the validator is not installed here, so it was not re-run | S | a human with a viewer, or a scripted browser render with pixel sampling; npm `gltf-validator` (network) | yes with a browser, otherwise no | yes (whether a VTK load plus internal colour check counts) |
| F-129 | flamelet / progress-variable combustion | PARTIAL (library only; no acceptance clause has a measured slice) | `rcfd-reactor --test combustion` (16 pass, 2 ignored misses) | no comparison with any published flame, no lift-off trend, no flamelet-library file reader (tables are built in memory by `FlameletTable::from_rows`), so "reader is documented" cannot be met; no case wiring | L | reference data (TNF / Sandia archive, network); a reacting turbulent-jet solve; a detailed mechanism | no | yes (acceptance wording) |
| F-150 | offline and local-model path | PARTIAL | `rcfd-assistant --test local_endpoints` (5 pass, `l1` ignored set-up defect superseded by `l1b`), `--test workflow` (9 pass), `rcfd-gui --test smoke` (6 pass), `--test key_security` (3 pass) | "verified against a real local server by a human" not done | S | a human and a real Ollama / LM Studio / llama.cpp / vLLM install (none on this machine) | no (needs a human) | yes |
| F-209 | adaptive stepping, accelerators | PARTIAL (Taylor-Green clause OVERSTATED) | `rcfd-fv --test adaptive_step` (9 pass), `rcfd-fv --lib ddt --include-ignored` (2 known misses reproduced), `rcfd-api --test timectl_tree` (4 pass) | linear decay: miss reproduced (81 vs 76, 46 vs 36 steps, old `ErrorController`; `stepctl` never tried). Taylor-Green: no fixed-step run's error is computed. Flat-plate high-aspect-ratio clause not attempted | S (TG fixed-step comparison) / M (decay redo) / L (flat plate) | flat plate: local-timescale / residual-smoothing machinery not delivered | no (whole item) | yes (reword the decay clause) |
| F-220 | tabulated and piecewise properties | DONE | `rcfd-core --lib iapws` (5 pass), `rcfd-core --test tabular --include-ignored` (12 pass, T9 vapour known miss ignored), `rcfd-api --test tabulated_properties` (5 pass) | none against the acceptance text. Title scope missing: orthotropic conductivity, nine-coefficient cp as a case law, P-T tables and Tait not connected to solvers | - | - | - | yes (close, or move the title scope to new items) |
| F-231 | mesh editing and repair | DONE (note stale) | `rcfd-mesh --test edit` (8 pass, none ignored) | none against the acceptance: volumes 2.7e-15, area vectors 2.7e-15 (limit 1e-12); repaired meshes pass `check_mesh` and equal the originals. Title scope missing: slit/baffle, zone separation, CLI/tree entry | - | - | - | yes (close) |
| F-241 | chained run configurations | DONE (caveat) | `rcfd-api --test chain_w19c` (5 pass), `rcfd-cli --test chain_w19c` (2 pass) | none against the acceptance: chain equals manual run then restart bit for bit. Speed 2.08e-3 and pressure 8.35e-3 relative L2 against the single steady run (limit 2e-2) | - | - | - | yes (close) |

Counts: DONE 3 (F-220, F-231, F-241), PARTIAL 7 (F-042, F-056, F-096, F-117, F-129, F-150, F-209), TODO 0. OVERSTATED sub-claim:
F-209 (Taylor-Green "fewer steps to a target error"; the fixed-step error is never measured). Stale notes: F-042 says scalable wall
functions are not wired to the settings tree, but `turbulence.wall_function = "scalable"` exists and `wall_options` c5 passes. F-231
says the non-orthogonality invariance is an ignored known miss, but it now passes non-ignored (worst change 3.5e-13 degrees).

Not re-run: the BFS-C grid study and sensitivities (`step_c_grid_study`, `step_c_sensitivities`), `step_kepsilon.rs` (old set-up),
`step_realizable_a0.rs` heavy tests, the Khronos glTF validator and the VTK load of F-117, and anything needing a real local model
server (F-150).

## F-042 k-epsilon family

Measured (non-ignored): centre-line / bulk U+ Standard 20.742 / 18.441, RNG 21.193 / 18.702, Realizable 21.630 / 18.973. The test
divides by 20.1 / 17.4. The note's verified MKM values are 20.13 / 17.54, which would give Realizable bulk +8.2 instead of +9.0
percent. "Mean profile within 10 percent of DNS" is shown only at these two points. Between y+ 20 and 150 the profile is compared
with Reichardt's law, not DNS, and validation-data holds no Re_tau 395 profile (only `mkm_chan180_sampled.csv`).

Step (BFS-C medium mesh, re-run, all converged in 840-909 iterations): RNG 6.334 (window 6.16-6.36, pass by 0.026), Realizable 6.413
and Standard 5.350 (both miss). Standard is also below the 5.5-7 "literature scatter" band. That band is in
`cases/backward_step/case.json` as "roughly 5.5 to 7 step heights for RANS models" with no citation: UNVERIFIED. The note's ADR
0017 result (Realizable with A0 = 4.0 at 6.349) was not re-run.

Proposed acceptance: "channel U+ at the MKM profile points (y+ above the first cell) within 10 percent; step x_r/h within 6.26 +-
0.10 for RNG and Realizable, and Standard within a cited band of published standard k-epsilon results". That needs the Re_tau 395
DNS profile file and a primary source for the Standard band. Closing it means rewording the acceptance (user), a DNS profile check
(S), and the energy and moving-wall sub-criteria from the merged review (M).

## F-056 particle tracking

Settling is met: terminal velocity relative error 1.39e-11 (PT1), plus the exponential approach, bounce and deposition closed forms.
The run path equals the library hooks bit for bit. No cyclone case exists in any form, so one acceptance clause is entirely unmet.
A one-wave closure: a frozen prescribed swirling flow (for example a Rankine vortex in a cylinder with axial outflow), particles
injected over a size range, and the grade-efficiency cut size compared with a closed-form estimate (Barth or Lapple type, recalled
from memory, UNVERIFIED). A real cyclone flow solve is a different, much larger job. The acceptance word "sanity" is unmeasurable.
Proposed wording: "terminal velocity within 1e-6; in a prescribed cyclone-like vortex flow the separation efficiency rises
monotonically with diameter and d50 is within a registered factor of the cited estimate".

## F-096 docs and tutorials

Every registered check of ADR 0089 re-passes: the site now has 129 pages (other audits added files) with 0 broken relative links,
5 tutorials, and the slow blocks run. The unmet clause is "executed in CI". `ci.yml` runs `cargo test --workspace` on ubuntu and
windows but has not run on these commits. `origin/main` is at 38ccbaa (wave 32), so the remote has an older state, and the
tutorials (wave 38) were never on it. Whether any hosted run happened for 38ccbaa was not checked (no `gh` here). One push by the
user would also settle F-001 and F-002. The GUI and sidebar steps stay `not-executed` by design. Linux execution of the tutorials is
untested.

## F-117 glTF export

The writer's structure and colours are checked internally: the vertex colours equal the colour map of the stored value, the 12-rule
`glb_check` passes, and the size is linear. "Loads in a glTF validator" rests on the recorded Khronos run (0 errors, 0 warnings); it
was not re-run because `gltf-validator` is not installed. "A viewer with correct colours" is not demonstrated. A scripted
headless-browser render (three.js or model-viewer from a CDN) with pixel sampling at known faces would close it in under a day if
network and a browser are allowed. Otherwise it needs a human.

## F-129 combustion extensions

Only library building blocks are tested: the Burke-Schumann closed form, the beta-PDF, flamelet limits and the extinction trend, and
the Zeldovich front. Neither acceptance clause has a measured slice. There is no flame comparison. There is no flamelet library
reader, because tables are built in memory, so there is nothing to document. The acceptance pairs "Sandia flames" with a "lift-off
trend". As far as recalled, the Sandia piloted flames D-F are attached flames, and lift-off studies use other burners (for example
the Cabra vitiated-coflow flame). Recalled from memory, UNVERIFIED. Proposed wording: "flamelet table read from a documented file
format; a non-premixed jet flame (Sandia D or similar) mean temperature and mixture fraction on the axis within a registered band of
the TNF data". Size L. It needs network access for reference data and a reacting turbulent jet case.

## F-150 local-model path

Measured: all four presets send the documented request to a loopback mock over real sockets, streaming and failures behave, and the
quick actions run with no model and open 0 connections (`quick_actions_run_without_a_model`,
`quick_actions_open_no_connection_even_when_a_local_endpoint_is_configured`). The remaining clause is human verification against a
real server. No local model server is installed on this machine. This is a user task, not an agent task.

## F-209 adaptive stepping

Taylor-Green: the Courant controller takes 74 steps, with L2 1.69e-2 against the small-step reference. The test compares this with
`n_fixed = ceil(t_end / dt_aim) = 113`, but that fixed run is never executed and its error is never computed. So "reaches a target
error in fewer steps" is not shown. The fix is size S: search the smallest uniform step count that meets 0.02, as `e1_run` already
does for decay.

Linear decay: the known misses reproduce (relative scale 81 vs 76 steps, absolute scale 46 vs 36). They use the wave-17 `ErrorController`, and the
wave-27 `stepctl` step-doubling control was never applied to this clause. For a single exponential decay with a relative tolerance
the optimal step is constant, so the clause is ill-posed as written. Proposed: a two-timescale (stiff) linear decay against a
uniform step count at equal final error.

The flat-plate clause, residual smoothing, local timescale factors and stretched-mesh acceleration are not delivered. Aitken
relaxation is only a library check (S10).

## F-220, F-231, F-241 (done against the acceptance)

F-220: the IF97 verification values (R7-97 Tables 5, 15, 35) are reproduced to 1e-8 relative, and the transport tables to 1e-7.
That tolerance was widened after the run and disclosed in docs/reference-data.md. Only regions 1 and 2, the saturation line and B23
are covered; regions 3 and 5 are absent. The saturation round trip closes to 1e-12 (p) and 1e-10 K (T). The Tait law meets its
defining relation to 1e-13.

F-231: both clauses pass non-ignored. The note's known miss is fixed (atan2 metric).

F-241: the "single run" in the acceptance is read as the single steady run. Because the transient stage starts from the converged
steady state, the 2e-2 check is weak. A stronger reading would compare against one transient run from rest to the same time. That
is optional wording for the user.
