# Partials audit, group 6

Read-only audit of ten roadmap items with status `partial` (launch HEAD 0d7b48a, 2026-10-21). Each item is graded against its
`acceptance` field in docs/roadmap.json only. Wider scope from the title or a "SCOPE (merged review)" addition is listed as
"scope beyond acceptance". It does not lower the grade.

Grades: DONE = the acceptance text, as written, is met by a passing non-ignored test (or a reproducible script) with numeric
limits; PARTIAL = a real tested slice exists, with the unmet clause named; TODO = nothing usable; OVERSTATED = the roadmap note
claims more than the tests show.

How the runs were made: Windows 11, `cargo test --release -p <crate> --test <name> -- --include-ignored` with a private target
directory, on a machine shared with other agents. "Ignored records FAIL" means the `#[ignore]`d known-miss or superseded
set-up-defect tests fail, as their notes say. Runs made:

* rcfd-api: sources 5/5, mrf 13 pass + 8 ignored records FAIL (3 exploration tests pass), radiation 5/5, regions 5/5,
  mixing_plane_run 8 pass + mp4_pressure_known_miss FAIL (6.85e-2 P, limit 5e-2), rotor_interface 2 pass + r5 known miss FAIL,
  mixing_length_tree 4/4, w26c_reports 2/2, component_diagonal 3/3.
* rcfd-fv: popbal_bins 9 pass + 1 superseded set-up record FAIL, popbal_qmom 8/8, popbal_transport 3/3.
* rcfd-post: w26c_catalogue 17 pass + 1 superseded set-up record FAIL.
* rcfd-turb: mixing_length 2/2. rcfd-gui: viewport_raster 13 pass + 3 ignored records FAIL (v1 known miss, v3 and v4 set-up
  defects).
* rcfd-reactor: reactor 10 pass + r3_gri_nasa_continuity_known_miss FAIL (h 5.12e-3, limit 1e-3). The GRI-Mech 3.0 files were
  present in validation-data/data/gri-mech-30, so the heavy R7 tests ran (no "skipped" line).
* F-093: a Python driver of `rcfd serve --stdio` (release build) timed 30 `panel.snapshot` calls on a running 40 x 40 cavity.

Not re-run: the FAN-01 sector runs of rcfd-api tests/fan01_validation.rs and the later fan studies (16 to 60 minutes each; the
ADR 0075, 0078 and 0086 tables are cited instead); the TypeScript test clients/claude-panel/panel.test.ts (it needs the Claude
Code mod test runner); any GUI run with a window; any Linux run.

## Summary table

| id | title (short) | grade | evidence run or read | residual gap (against acceptance) | size | blockers | one wave | user decision |
|---|---|---|---|---|---|---|---|---|
| F-031 | Porous media, momentum sources, MRF | DONE | ran rcfd-api `sources` 5/5 (packed bed, porous box, rotating channel, rest state), `mrf` 13 pass, `component_diagonal` 3/3 | none. Scope beyond: transient MRF, porosity-weighted and anisotropic resistance, porous jump (F-113), MRF torque known miss (-3.41 % against 3 %) | - | - | - | yes (flip to done and move the scope to a new item) |
| F-052 | Radiation: P1, S2S, DO | PARTIAL | ran rcfd-api `radiation` 5/5; read ADR 0012, docs/reference-data.md | only a 1D grey slab: P1 is compared with its own analytic solution (verification), and with the exact transport solution only at tau = 20 (Milne asymptote, 0.41 %). No multi-dimensional grey enclosure, no exact-RTE comparison at intermediate tau | S (reword) / M (2D enclosure) | none for P1; a 2D exact solution needs a verified source | yes | yes (reword to the P1 slab and close, S2S/DO stay in F-130) |
| F-093 | Claude Code mod live monitors | PARTIAL | ran a stdio timing of `panel.snapshot` (release: 5.8 / 12.8 / 30.2 ms min/median/max, monitor sample current to the iteration); read register.tsx | update within 1 s is shown only on the service side (500 ms poll plus at most 30 ms); never observed in a host UI | S | needs a Claude Code host to run the mod | no | yes (retire as superseded by F-144, or accept the service-side timing) |
| F-112 | 0D reactor toolkit | PARTIAL | ran rcfd-reactor `reactor` with the GRI data, 10 pass + 1 known miss | no comparison with PUBLISHED GRI-Mech 3.0 ignition delays (none verified); no CH4/air ignition delay at all (only H2/air); the PSR check is a blowout residence-time trend for H2/air, not a temperature trend against literature; 1D flame speed (title) not implemented | M (reference) / L (flame speed) | missing verified reference data | yes for the ignition comparison if a source is found | yes (which published source counts) |
| F-126 | Turbomachinery rotating interfaces | PARTIAL (note stale) | ran rcfd-api `rotor_interface` 2 pass + known miss, `mixing_plane_run` 8 pass + known miss, `mrf` M5; read ADR 0075, 0078, 0086 and docs/milestone-fan.md | pressure ratio and efficiency not reproduced: the FAN-01 attempt (ADR 0075) missed every limit (torque -38 % / -46 %), the later studies did not close it. Mass across the interface is met (M5 5e-12, mixing plane 2.5e-14 per band) | L | fan milestone work; Delta p_ts is an installation value a rotor-only model cannot match | no | yes (reword: which case and quantity) |
| F-143 | 2D mesh and field viewport | PARTIAL | ran rcfd-gui `viewport_raster` 13 pass + 3 ignored records | "shows the lid-driven vortex" has only a data proxy (v10: top-row mean ux 0.573, return flow min ux -0.182, speed maximum in the top rows); nobody has looked at the picture | S | none | yes | yes (human look, or a measurable rewording) |
| F-206 | Additional RANS closures | PARTIAL | ran rcfd-turb `mixing_length` 2/2 and rcfd-api `mixing_length_tree` 4/4 (Re_tau 395 channel within the 10 % limit) | pipe profile against Nikuradse data (no pipe case, no Nikuradse data); v2-f not implemented; curvature correction and its rotating-channel and U-duct cases not implemented | L | missing reference data (Nikuradse, Re_tau 395 DNS, curvature cases) | no | no |
| F-216 | Per-region model sets | PARTIAL | ran rcfd-api `regions` 5/5; read coupled_cht.rs and ADR 0029 | only turbulence = laminar per region for momentum; no per-region energy, radiation or species model; no published finite-conductivity CHT benchmark; turbulence next to solids is rejected (coupled_cht `e2_turbulence_and_property_laws_are_rejected`) | L | F-051 (turbulent CHT), missing reference data | no | no |
| F-228 | Population balance | DONE | ran rcfd-fv `popbal_bins` 9 pass, `popbal_qmom` 8/8 (Q4 constant kernel worst moment 1.44e-12, limit 1e-8), `popbal_transport` 3/3 | none. Scope beyond: case-tree wiring, interphase coupling (needs F-127), DQMOM, spatial moment transport | - | - | - | yes (flip to done, split the coupling scope) |
| F-238 | Report, surface and post catalogue | DONE | ran rcfd-post `w26c_catalogue` 17 pass (S1 to S3 and B3 orders 1.96 to 2.04, D1 worst 20.6 eps against 64 eps), rcfd-api `w26c_reports` 2/2 | none. Scope beyond: case-tree keys and VTU/CSV output for surfaces, rakes, point clouds, transforms and comparisons | - | - | - | yes (flip to done, split the case-tree and file-output scope) |

Counts: DONE 3 (F-031, F-228, F-238), PARTIAL 7, TODO 0, OVERSTATED 0. The F-126 note is understated rather than overstated: it
still says the acceptance was "NOT ATTEMPTED", but ADR 0075 attempted it on FAN-01 and missed.

## Items that are not DONE

**F-052.** The acceptance asks for "a gray enclosure benchmark" against "the analytic solution", without naming either. The tested
slice is the grey slab between two grey plates (R1 2.6e-9 against the analytic P1/Marshak flux for tau 0.1 to 10, unequal
emissivities, scattering; R2 0.41 % against the Milne transport asymptote at tau = 20; R3 Rosseland; R4 energy balance). R1 checks
P1 against its own solution, not against the radiative transfer equation, so it is code verification. The note also understates
the delivery: Rosseland is tested as well. Proposed rewording: "P1 grey slab matches the analytic P1 solution (1e-3) and the exact
transport limits: optically thin two-plate exchange and the thick Milne asymptote (1 %)". The thin limit (tau -> 0, exact
two-surface grey exchange) is not yet a separate check, which is an S task. With that rewording the item closes and S2S/DO stay
in F-130. Unverified: the Milne value 0.7104 is confirmed only through secondary sources, and the slab mapping
Psi = 4/(3 tau + 6 x 0.7104) is derived here (docs/reference-data.md).

**F-093.** The service side is measured and re-measured. In release, `panel.snapshot` with a 40 x 12 speed raster takes 5.8 to
30.2 ms, and the newest returned monitor sample is the current iteration. With the 500 ms poll, a sample can reach the panel
within about 0.53 s. The note's 210 to 250 ms was a debug build. The draw in a Claude Code host has never been observed, and this
audit cannot run the TypeScript test. The item is already demoted and superseded by F-144. Recommendation: the user retires it,
or accepts the service-side timing as meeting the acceptance.

**F-112.** All registered checks reproduce (R7 H2/air constant-volume tau_ign 3.054e-4 / 8.616e-5 / 4.422e-5 s at 1000 / 1100 /
1200 K; HP CH4/air 2225.52 K; PSR blowout brackets 1.49e-5, 1.03e-5 (600 K) and 4.29e-5 s (phi 0.5)). Unmet clauses:
(1) "H2/air and CH4/air ignition delay agree with published GRI-Mech 3.0 based results within 5 percent": no published value is
used, and no CH4/air ignition delay is computed at all. (2) "PSR blowout temperature trend matches literature": the test checks
the trend of blowout residence time for H2/air (preheat shortens it, lean lengthens it) with no literature number; temperature at
blowout is printed only. (3) 1D flame speed (title) does not exist. Closing (1) needs a verified published table of GRI-3.0
ignition delays (a paper that tabulates them, not another code's source). That is a data search and then an S code task. (2)
needs a literature blowout curve. Flame speed is an L task and could move to F-225. Unverified: CH4/air T_ad 2225 K is from memory
(a secondary source lists 2224.25 K from CEA but 2156.25 K for GRI/Cantera, unexplained); the Robertson digits are from memory.
Known miss: GRI thermo continuity 5.12e-3 at 1000 K (a property of the data file).

**F-126.** The note is stale. Wave 34D (ADR 0075) made the acceptance attempt on FAN-01, a published low-speed axial fan
(Zenodo 10.5281/zenodo.20037409, CC BY 4.0). Attempt 3 converged and missed every limit: torque 1.320 against 2.142 N m (-38 %)
at design and -46 % off design; Delta p_ts -3.8 and -84.4 Pa against 126.5 and 63.9 Pa. The ignored test
`fan01_validation.rs` known miss records this. ADR 0078 found the dominant cause to be numerical swirl. ADR 0086's grid study
(1.850 / 1.967 / 2.038 N m) failed its own empty-passage check, so no agreement is claimed. ADR 0078 also showed that the
measured Delta p_ts is an installation value: the rotor-only result moves 8 Pa with the outlet type alone, so pressure ratio and
efficiency "within published scatter" cannot be met without modelling the installation. Further problems with the acceptance
text: FAN-01 is a fan, not a compressor; the fetched Rotor 37 report is a transonic compressor, not low speed. The
"mass conserved across the interface" clause is met (M5 continuity 5e-12, net interface flux 3e-14; mixing plane 2.5e-14 per
band). The note should be updated and the acceptance reworded, for example "torque and total-to-total pressure rise of FAN-01
within the registered tolerance". This belongs to the fan milestone, not one wave. Unverified: the "published scatter" (+-3 Pa)
is a round robin on 3-D printed copies (Vourakis and Karlsson 2022), not an FAU uncertainty.

**F-143.** Clauses 2 and 3 are met (colour map and range logic unit-tested: raster on every pixel, legend ticks, 7 window sizes,
headless egui frame). Clause 1, "cavity speed field shows the lid-driven vortex", has only the v10 data proxy (top-row mean ux
0.5731, min ux -0.1822, speed maximum in the top two rows), which the test itself names "not a visual claim". Two ways to close
(S): the user looks at a rendered PNG, or the clause is reworded into something measurable, such as the raster's centreline ux
against Ghia et al. at Re 100 (column u_Re100 of validation-data/reference/w31c_ghia1982_u_centerline_read.csv, already
committed) plus a test that the vortex-centre pixel is a speed minimum. Known miss: luma is not monotone by 0.2126 from 8-bit
quantisation (v1, registered).

**F-206.** None of the three clauses is met as written. (1) The mixing length is validated on a periodic Re_tau 395 channel
(worst 4.0 % linear, 3.1 % Nikuradse profile, limit 10 %), not a pipe against Nikuradse data. There is no pipe generator and no
Nikuradse data in the tree. (2) v2-f does not exist. The Re_tau 395 DNS values in mixing_length_tree.rs are typed in from memory
(Moser, Kim and Mansour; unverified); the only DNS file committed is mkm_chan180_sampled.csv at Re_tau 180. (3) Curvature
correction and its rotating-channel and U-duct cases do not exist. The title adds EARSM, a generalised two-equation model,
production limiters and Prt/Sct options, none delivered. Size L and several waves. Splitting the item per model would make it
tractable.

**F-216.** Delivered: `regions.<name>.turbulence = laminar` sets the momentum viscosity on faces touching the region. Exact
equality with a laminar run (R1) and exact mask semantics (R2) pass. Unmet: turbulence transport is still solved in laminar
regions; there are no per-region energy, radiation or species models; region coupling through interfaces is missing; the
published conjugate benchmark with a conducting wall has not been run. The existing coupled CHT check C3 (ADR 0029) uses
high-conductivity walls to reproduce de Vahl Davis Nu 1.118 (+0.03 %). By its own ADR it is a consistency check, not a
finite-conductivity benchmark. The coupled CHT path rejects turbulence (`e2_turbulence_and_property_laws_are_rejected`), so a fluid
region with a turbulence model next to a solid cannot be set up today. Blocked by F-051 and by verified finite-conductivity CHT
reference data. The de Vahl Davis values are confirmed only through secondary sources.

## DONE items: scope beyond the acceptance

**F-031.** The acceptance ("packed-bed Ergun pressure drop and rotating channel tests pass") is met by rcfd-api tests/sources.rs
(packed bed +0.001 %, porous box +0.02 %, rotating channel profile 0.062 % and pressure +0.20 %, all with a 1 % limit; rest
state at round-off). The MRF work of ADR 0008 adds more passes and eight registered misses (torque -3.41 / -4.12 % against 3 %,
spurious speed 1.41e-3 against 1e-3, M1 pressure 7.55e-2 against 5e-2), re-measured here unchanged. Unverified: Ergun 150 / 1.75
constants are checked only against a secondary web source (Ergun 1952 was not read).

**F-228.** Constant-kernel Smoluchowski moments: QMOM Q4 1.44e-12 (limit 1e-8, N = 2 and 3); bins B1 (Flory, constant kernel
on a linear grid) 1.2e-11 and B2 m0 3.8e-12. The closed forms are derived in the tests (exponential initial distribution,
m_k = k! (1 + tau)^(k-1)), so no external value is needed. One-way only; nothing is wired into the case tree.

**F-238.** Library acceptance met with the registered ADR 0048 limits (sphere area, flux and std orders 1.96 to 2.04 and n = 32
errors below 5e-3; box std order 2.002; identical solutions 0; uniform offsets across mesh pairs worst 20.6 eps, limit 64 eps).
"Machine precision" was registered as 64 eps, which is reasonable. Six scalar report types reach the case tree (w26c_reports
2/2). The remaining case-tree keys and file output are scope beyond the acceptance.
