# Partials audit, group 8

Read-only audit of ten roadmap items with status `partial` (HEAD e20b7fc, 2026-10-21). Each item is graded against its
`acceptance` field in docs/roadmap.json only. Wider scope from the title, from a "SCOPE (merged review)" addition or from
"acceptance additions" in the note is listed separately with the prefix "scope-only". It does not lower the grade, but the
paragraph says how to record it.

Grades: DONE = the acceptance text, as written, is met by a passing non-ignored test with numeric limits; PARTIAL = a real
tested slice exists, with the unmet clause named; TODO = nothing usable; OVERSTATED = the roadmap note claims more than the
tests show.

How the runs were made: a clean `git archive e20b7fc` copy, Windows 11,
`cargo test --release -p <crate> --test <name> -- --include-ignored --nocapture`, with a private target directory, on a
machine shared with other agents. Every failing test in these runs is an `#[ignore]`d known-miss or superseded record. Each
one fails as its note says, and no default test failed. F-149 was also run on Linux (WSL2 Ubuntu 20.04, ext4 home
directory, rustc 1.96, not the 1.75 MSRV): `cargo test --release -p rcfd-assistant --test key_security --test providers`,
17 of 17 pass, including the `cfg(unix)` 0600 assertions. Not re-run: a full workspace test, macOS, hosted CI, and the
GUI key test on Linux.

## Summary table

| id | title (short) | grade | evidence run or read | residual gap (against acceptance) | size | blockers | one wave | user decision |
|---|---|---|---|---|---|---|---|---|
| F-041 | Spalart-Allmaras model | PARTIAL | ran rcfd-turb `flat_plate_sa` (6 pass, 196 s; ignored power-law known miss fails at +5.29 %); read ADR 0014 | ADR 0014 reads the acceptance as S1 and S2 and S3; S2 (power law) misses at x = 1.0 by +5.29 % against 5 % | S (rewording) | user decision: the acceptance names no correlation, and the two registered correlations differ by 8.5 % at Re_x 5e6 | yes | yes |
| F-055 | VOF free surface and surface tension | DONE | ran rcfd-fv `vof_dambreak`, `vof_bubble`, `vof_static`, `vof_droplet`, `vof_advection` (A3 known miss fails as recorded), rcfd-api `vof_tree` 5/5, `vof_wiring_tree` 4/4 | none against the text; the Hysing reference values were recalled from memory and are unverified. Scope-only: PLIC, contact angle, CSS tension, Marangoni, 3-D | S (verify Hysing) | none | yes | no |
| F-095 | Parametric studies and design-point runner | DONE | ran rcfd-cli `study_w20c` (10 pass; 3 known misses fail as recorded), `map_w32c` 2/2, rcfd-api `design_e2e_w32c` 1/1, `design_opmap_w32c` 1/1, rcfd-mcp lib `design_tests` (5 pass, T3 known miss) | none against the text. Scope-only: Poiseuille map within 1 % (S7 misses: 1.9 % at ny 10; 0.29 % at ny 20) | S (S7 scope) | none | yes | yes (record S7 as scope or re-register at a resolved mesh) |
| F-115 | Interpolate a solution onto a new mesh and restart | DONE (vacuous check) | ran rcfd-api `interpolate` 3/3, `restart_w19c` (7 pass; S5a known miss), rcfd-post `interpolate` 2/2, `remap_w19c` (10 pass; 4 known misses), rcfd-cli `interpolate` 1/1 | met as written, but the uniform start has residual 1.0 by construction, so the residual check is vacuous; S5b (10 vs 23 iterations to within 1e-2 of the converged speed) is the meaningful check and passes | S (rewording) | user decision on the wording | yes | yes |
| F-128 | Lagrangian extensions | DONE | ran rcfd-api `particles_two_way` (14 pass; 6 known misses fail as recorded), rcfd-particles `lagrangian_ext` (13 pass; 1 set-up-defect record) | none against the text. Scope-only: breakup, wall film, erosion, evaporation; note's acceptance additions (spray penetration, falling film); published jet | L (scope) | public spray and film data | no (scope) | yes (split the scope) |
| F-149 | Provider key and endpoint storage | DONE | ran rcfd-assistant `keychain` 7/7, `key_security` 4/4, rcfd-gui `key_security` 3/3 on Windows; `key_security` 4/4 and `providers` 13/13 on Linux (0600 asserted); read crates/rcfd-gui/Cargo.toml | none against the text. Scope-only: real Windows Credential Manager or macOS Keychain round trip, Windows ACLs | S (scope) | real keychain (manual or hosted run) | yes | no |
| F-208 | Numerics robustness options for poor meshes | DONE | ran rcfd-fv `mms_poor_meshes` (3 pass; superseded G2 precondition record), rcfd-api `limits_wave13` 2/2; read ADR 0016 | none against the text (finest-pair orders 1.744 to 2.04; options change good-mesh error by at most 4.6 %). Scope-only: pressure-interpolation variants, separate warped-face correction, case-tree wiring | M (scope) | none | yes | yes (split the scope) |
| F-218 | Actuator-disk momentum source | PARTIAL | ran rcfd-fv `actuator` 3/3, rcfd-api `actuator_disk` (2 pass; 2 known misses fail as recorded); read ADR 0036 | no CFD hover induced velocity against momentum theory (A5 not attempted; only the formulas are checked); no blade-element lookup, tip loss or model-rotor comparison within 10 % | L | public hover data set to be obtained and verified; blade-element tables | no (hover momentum clause alone: yes, M) | yes (which data set) |
| F-230 | Dynamic mesh layering and remeshing | DONE | ran rcfd-fv `layering_w38c` 5/5, `ale_gcl` 4/4, `ale_flows` (4 pass; 4 F-063 known misses fail as recorded); read ADR 0090 | none against the text (p V^gamma 1.53e-5 and 3.42e-5 against 2e-2; layer-event conservation 4.4e-16). Scope-only: local remeshing, unstructured layering, case-tree wiring, PIMPLE layering | L (scope) | none | no (scope) | yes (split the scope) |
| F-240 | Declarative setup-validation rule engine | PARTIAL | ran rcfd-api `rules_w19c` (5 pass; E2 zero-warning known miss fails on SV103); read crates/rcfd-api/rules/setup_rules.json, ADR 0031 | "published validation cases raise no error-level diagnostic" was never measured: the validation tests build their solvers through the library or from templates, which E2 skips; the five registry descriptors (case.json) have no loadable setup | S | none | yes | no |

Counts: DONE 7, PARTIAL 3 (F-041, F-218, F-240), TODO 0, OVERSTATED 0. Of the seven DONE items, only F-149 can flip now. The
other six are DONE pending a reference check (F-055, Hysing values) or a user decision on wording or scope split (F-095,
F-115, F-128, F-208, F-230).

## F-041 Spalart-Allmaras one-equation model (PARTIAL)

Re-run (wall-resolved plate, y+ 0.52): Cf(0.97) = 0.002786. Against the TMR CFL3D SA value this is +2.97 %. Against
White's correlation it is -4.1 / -3.6 / -3.1 / -3.0 % at x 0.4 / 0.6 / 0.97 / 1.0. Against the power law
0.0576 Re_x^-0.2 it is +0.8 / +2.7 / +5.29 % at x 0.4 / 0.6 / 1.0. All of these match the note. The wall-function mesh
gives +5.81 % against TMR SA (limit 10 %), and resolved mode is identical to auto mode.

The miss is S2 (power law at x = 1.0). ADR 0014 registered that the acceptance is met only when S1, S2 and S3 all pass.
That rule is the problem. At Re_x = 5e6 the power law gives about 2.64e-3 and White's correlation about 2.86e-3, which are
8.5 % apart. To sit within 5 % of both, Cf must fall in a band about 2 % wide (2.72e-3 to 2.77e-3), and the TMR SA reference itself sits
about +2.4 % above the power law at x = 0.97. The acceptance "within 5 percent of correlation" never names a correlation,
so it is ill-posed as written.

Proposed rewording, for the user to decide: "flat plate Cf within 5 % of the TMR SA reference and of White's correlation
for 0.4 <= x <= 1.0 (Re_x 2e6 to 5e6)". Any new reading must be registered in a new ADR before the item is re-graded, so
that the limit is not chosen to fit the result. If the user wants it, a mesh-convergence slice would make the
item stronger (size M).

Unverified: the grid behind the TMR CFL3D file value. The 0.0576 power law is verified only against a CFD-Online wiki
page; textbooks also use 0.0592 (docs/reference-data.md). Neither is used for the proposed wording beyond White.

## F-055 VOF free surface (DONE, reference check outstanding)

Re-run:
- Dam break: v/sqrt(gH) = 1.504 (Martin-Moyce 1.69, band 1.44 to 1.94), with deviation from the 1.69 line 0.161 / 0.110
  at t* 2 / 3 (limit 0.20) and volume drift 4.5e-15.
- Hysing bubble, free-slip sides: y_c(3) = 1.028 (reference 1.0813, limit +-0.12) and v_max = 0.220 (reference 0.2417,
  limit 25 %).

Both are default (non-ignored) tests, so the acceptance text is met.

Two caveats:
1. "Front position vs experiment" is tested as the mean front speed against one averaged value. That value comes from a
   secondary source (arXiv:1308.0115) and is not a pointwise front table.
2. The Hysing values are marked unverified (recalled from memory) in docs/reference-data.md. The bubble clause therefore
   rests on numbers nobody has read in the paper.

Closing step (S): verify the Hysing case-1 values against the published paper or the public benchmark data. Only then flip
the status.

Scope-only: geometric PLIC, explicit/implicit and sharp/dispersed taxonomy, contact angle, continuum-surface-stress
tension, Marangoni, adaptive capillary time step, 3-D validation. These should become a separate item.

## F-095 Parametric studies (DONE)

The literal acceptance is met:
- s2 shows tables byte-identical between 1 and 3 threads.
- s3 shows resume skipping exactly the completed points (11 run, 7 skipped).
- The study runner is exposed as the `design_run_study` MCP tool (T4 passes).

The specific "5 inlet velocities" study exists only in the ignored S7 test. That test runs `--parallel 2`, exits 0 and
returns 5 rows (table.json left by this run: 5 rows, all `ok`, u = 0.02 to 0.1). It fails only on the merged-review 1 % clause (u = 0.02: dp 0.9417 against 0.96, -1.9 %). The non-ignored
refinement diagnostic gives 1.90 % at ny 10 and 0.29 % at ny 20, and dp/U is constant to 1e-3.

No non-ignored test runs the five-velocity sweep. The DONE grade combines three passing tests: s2 (thread independence),
s3 (resume on the slab study), and the 3-velocity `--parallel 2` run inside `poiseuille_error_decreases_with_refinement`.
The ignored S7 run supplies the 5-row table.

Misquote in the note: it says "the acceptance text asks for" assistant/MCP exposure. Only the title does. The exposure now
exists in any case (ten `design_*` tools).

Options for the scope-only S7 clause:
- record it as scope, or
- pre-register a new S7 at a stated resolution (ny 20) in a new ADR, with the old test kept ignored.

Unverified: the Sobol direction numbers (not used by this acceptance).

## F-115 Interpolate and restart (DONE, but the check is vacuous)

Re-run: first-iteration res_u 0.166 and res_p 0.109 against 1.0 / 1.0 for the uniform start. Both modes are selectable
(conservative res_p 0.1086). Iterations to 1e-6: 103 interpolated against 89 cold, so S5a is a known miss and the
measured benefit is negative.

The 2026-10-14 audit remark stands. The residual is normalised by the run's own variation, so a uniform start always gives
1.0, and "lower initial residual" cannot fail.

The meaningful check is S5b, which already passes: 10 iterations against 23 cold to within 1e-2 of the converged speed,
for gradient, least_squares and conservative_overlap. Proposed rewording, for the user to decide: "a coarse cavity solution
interpolated to a fine mesh reaches within 1e-2 of the converged speed in fewer iterations than a uniform start".

Scope-only: unstructured conservative remap, divergence-free flux projection (imbalance 0.18), BDF2 history across meshes,
VTU read-back, service `restart.from`.

## F-128 Lagrangian extensions (DONE)

Re-run, particle-laden jet (two-way, wired into SIMPLEC): converged with coupling change 4.7e-9. The record matches the
particle sum to 5.7e-16. The x-momentum balance residual is 3.9e-11 (limit 2e-2), the cross-stream residual 2.1e-13, and
the force is linear in loading (D(0.1)/D(0.01) = 9.57).

Rosin-Rammler injection: KS 0.00487 (truncated 0.00531) against the bound 0.01379, mass flow to 3e-13.

The six plug-flow known misses (PL1b, PL2, PL3, PL4, PL7, PL7b) fail as recorded. None of them is an acceptance check.

Scope-only: breakup, wall film, erosion, evaporation, track files, a published particle-laden jet, and the note's
"acceptance additions" (spray penetration within 10 %, falling-film thickness within 5 % of Nusselt). These additions are
in the note, not in the acceptance field. If the user wants them to gate the item, they belong in new items, each needing
public data. The Rosin-Rammler form is verified from secondary sources only.

## F-149 Provider key storage (DONE)

The three acceptance clauses:
- Redaction: `secret_debug_is_redacted_and_config_has_no_key`, `http_errors_are_reported_without_the_key`, and the k2
  canary scan pass, and the k2 scanner self-test shows the scan is not vacuous.
- Not exposed to egui storage: the GUI flow test scans providers.json, the preferences file, status lines, the chat
  transcript and every request except the credential header, and finds 0 occurrences. Also, `eframe` is built without its
  `persistence` feature (crates/rcfd-gui/Cargo.toml), so no egui storage file exists.
- 0600 on unix: measured for the first time in this audit, on Linux. `k3_file_store_round_trip_and_contents` and
  `file_key_store_and_config_round_trip_in_a_temp_dir` assert `mode & 0o777 == 0o600` and pass.

The note's "real unix 0600 permissions unverified" is now resolved. A hosted Linux CI job would keep it that way.

Scope-only: a real Windows Credential Manager and macOS Keychain round trip, and Windows ACLs on keys.json.

## F-208 Poor-mesh numerics options (DONE)

Re-run, finest-pair (16 to 32) L2 orders:
- skewed K: 2.002 / 2.019 / 1.756 for C0 / C1 / C2
- warped W: 1.939 / 1.959 / 1.866
- stretched S: 1.744 for all three

All are within the registered limits (1.7, and 1.5 for the limiter). On the near-orthogonal meshes the options change the
error by a factor of 1.000 (G), 1.022 / 1.046 (G2) and 1.013 / 1.027 (G3), against limits of 1.10 / 1.25.

On the stretched mesh the coarse pair is 1.594. The registered check uses only the finest pair, so the asymptotic range is
barely reached.

The acceptance says "the options do not degrade it". The options named in the title but not implemented (pressure
interpolation variants, a separate warped-face correction) cannot be tested. This audit grades only the implemented
options and lists the rest as scope-only, to go into a new item along with case-tree wiring of `GradientOptions`.

## F-218 Actuator disk (PARTIAL)

Measured:
- The closed forms are checked to 1e-13, but these are formula checks, not CFD.
- Force and torque are exact (3.7 N, 0.7 N m to 1e-15).
- Duct kinematic dp is 50.0000, with velocity perturbation 2.9e-12.

Unmet:
1. "Axial hover thrust and induced velocity match momentum theory" in a flow solution. The registered A5 (hover in a box)
   was report-only and was never attempted.
2. "A public model-rotor hover data set gives thrust within 10 %". This needs blade-element lookup, tip loss and trim
   (none exist) plus a reference data set.

The first clause needs a pre-registered hover case with a finite-domain allowance and is closable in one wave (M). The
second is L: it is blocked on choosing, obtaining and verifying a public data set. A frequently cited candidate is the
Caradonna-Tung model rotor (NASA TM 81232, 1981), but that citation is recalled from memory here and is UNVERIFIED. Which
set to use is a user decision. The hover formulas are derived, not checked against a rotorcraft text.

## F-230 Dynamic mesh layering (DONE)

Re-run, piston in a closed cylinder:
- p_bar V^gamma error 1.528e-5 at full compression and 3.415e-5 after re-expansion (limit 2e-2)
- 24 removals and 24 additions
- volume, mass and energy change per layer event 4.4e-16 (limit 1e-13)
- mass drift 5.4e-13 (limit 1e-12)
- volume against A x_p 8.9e-16
- GCL uniform flow 4.5e-15
- inverse-volume stiffness strain ratio 0.179

The acceptance is met as written. Two remarks:
- The piston runs at peak Mach 0.05, so the isentrope check is close to quasi-static.
- The four `ale_flows` known misses belong to F-063, not this item.

Scope-only (the title): local remeshing, unstructured or prism layering, case-tree and CLI wiring, layering in the
incompressible PIMPLE path, characteristic boundaries on moving faces. Split these into a new item.

## F-240 Setup-validation rule engine (PARTIAL)

Met:
- 21 rules, 13 of them error-level dependency, required or count checks (count conditions in SV001, SV002, SV202), so at
  least 10.
- E1: each rule fires with its id, severity, category and path, and no other error rule fires.
- E3: output is deterministic.
- E4: the loader is strict.
- E2 as re-run: 84 distinct case texts evaluated (the note says 66; the corpus has grown) with 0 error-level findings.
  Warnings are SV103 and SV101 only.

Not measured: E2 skipped 40 templates, 97 texts that are not valid cases and 10 whose mesh did not build. Two clauses are
affected:
- "Accepts the benchmark registry cases": the five registry descriptors (cavity, backward_step, flat_plate,
  heated_cavity, naca0012) have only case.json and no loadable setup.
- "The published validation cases raise no error-level diagnostic": the `validation_*.rs` tests (Ghia cavity, BFS,
  cylinder, Blasius, channel Re 180, Graetz, Sod, natural convection, wall law) build solvers through the library or from
  `format!` templates. E2 never evaluates them.

Closing step (S): write case-tree TOML setups for the registry and validation cases (or instantiate the templates with
their registered parameters), and assert 0 error-level diagnostics on them.

Separately, the note honestly records that 12 of the 13 error rules duplicate loader checks, and that auto-fix is
suggestion-only. Neither is part of the acceptance.

## Unverified references this group depends on

- Hysing et al. (2009) case-1 values (F-055), recalled from memory.
- Martin-Moyce 1.69 (F-055), secondary source only.
- The grid behind the TMR CFL3D SA flat-plate value (F-041).
- The 0.0576 power-law coefficient (F-041), web wiki only.
- Rosin-Rammler form (F-128), secondary sources.
- Sobol direction numbers (F-095 scope only).
- The Caradonna-Tung data-set citation proposed for F-218, recalled here and not checked.
