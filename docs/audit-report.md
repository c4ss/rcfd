# rcfd independent audit report

Date: 2026-10-14. Repository state audited: branch `main`, HEAD `d1b43f0` (31 commits, working tree clean apart from one
locally excluded file, see section 6). Auditor: an independent model-run audit (Claude Sonnet 5.5). This report is model
output; every number below was read from the repository or from a command run during the audit, but none of it has been
reviewed by a human. Nothing in the repository other than this file was changed. This is a record of that date and commit;
for the current status of each item, read the [roadmap](ROADMAP.md).

Method: read `docs/roadmap.json`, `docs/ROADMAP.md`, `docs/reference-data.md`, `docs/adr/`, `docs/bfs-convergence-findings.md`;
diffed `roadmap.json` and every test file across all 31 commits; compared the current roadmap against three pre-commit
snapshots of the roadmap kept outside the repository; read the assertions of the tests that back each done/partial
item; ran `cargo test --workspace --release`, `cargo fmt --check`, `cargo clippy`, and the ignored heavy tests, using
`CARGO_TARGET_DIR=<target dir>`. No vendor software, install or vendor material was read.

## 1. Headline findings

1. **Git history cannot prove "no criterion was changed".** The first commit (`24a9802`, 2026-10-13 17:05) already contains
   the work of waves 1 to 6 as one squashed import, and each later feature commit adds its tests once, in their final form.
   Across all 31 commits the `acceptance` text of every roadmap item is byte-identical to the first commit; only `status` and
   `note` change. The edits that did happen (tolerance widened, check replaced) happened before the commit that first
   recorded them and are known only from the repository's own self-disclosures (test headers, `docs/reference-data.md`,
   roadmap notes). Those disclosures are, in every case I could check, present and candid. The three snapshots in the scratch
   folder show only two acceptance-text edits (F-094 and F-125, section 3) and none to a numeric tolerance.
2. **Test suite: all green.** 458 tests passed, 0 failed, 16 ignored, in 83 test suites. All 16 ignored tests were run
   except the explorations; every one that was run passed (section 4).
3. **`cargo clippy --workspace --all-targets -- -D warnings` FAILS in this checkout**, but only because of an untracked,
   git-excluded local helper (`crates/rcfd-api/examples/baseline_hash.rs`, lint `no_effect_replace`). Without that file (lib,
   bins, tests, the tracked `cavity` example) clippy is clean. In a fresh clone CI would pass. `cargo fmt --check` passes.
4. **Several "done" items are not demonstrated by their own acceptance text** (section 5): F-073 (the 1M-cell, 2 s render
   criterion is never measured), F-027 (criterion says V-cycles; the 1M evidence uses W-cycles, 14 of a limit of 15, in an
   ignored test), F-001, F-002 and F-004 (criteria name CI runs and a review that have not happened).
5. **Hash-identity claims are not reproducible from the repo.** F-031, F-050/ADR 0005 d1 and the F-061 notes repeatedly say
   "baseline runs bit-identical (hash check)". The tool that computes the hashes is a "throwaway, not committed" example
   excluded through `.git/info/exclude`, and no baseline hash values are committed. The constant-density-limit tests
   (ADR 0005 d2/d3) do exist and pass; the bit-identity claim does not have a committed gate.
6. **Counts:** 53 done / 21 partial / 40 todo overall (63.5 of 114 weighted, 55.7 %). Gap items F-104..F-139: 9 done / 8
   partial / 19 todo (13.0 of 36, 36.1 %). If the corrections in section 7 are accepted, overall becomes 48 / 26 / 40 (61.0
   of 114, 53.5 %) and gap items stay 9 / 8 / 19 (the F-001/002/004/027/073 changes are outside the gap range).
7. **ROADMAP.md has drifted from roadmap.json** in notes (33 items); statuses, titles, acceptance text and dependencies
   agree for all 114 items. The most misleading drift is F-006 and F-028/F-042/F-060, where ROADMAP.md still says reference
   values were "from memory / unverified" while the JSON says they were cross-checked (section 8).
8. **Vendor-derived content:** no vendor code, text or data is in the repository. But the roadmap notes of about 20 gap
   items used vendor-specific identifiers for settings and modules instead of generic capability names, and `ROADMAP.md`
   cited an inventory file that is not in the repo. These should be generic capability descriptions (section 9).
   (Later edit: those notes were reworded generically; this report no longer quotes the identifiers.)

## 2. Edited or replaced criteria, tolerances and tests

"Evidence" says where the edit is documented. "Commit" is the first commit in which the final text appears; where the
change was made before that commit, the before-state is only known from the disclosure and I say so. I could not recover
any before-state from git for items marked (pre-commit).

| # | Item | Before | After | Commit / evidence | Assessment |
|---|------|--------|-------|-------------------|------------|
| 1 | F-043 SST flat plate | Roadmap text has no number. The F-043 note says the plate "misses the +-5 percent band against White's formula", which implies a 5 % band against White `0.455/ln^2(0.06 Re_x)` (the correlation in `rcfd-mesh::layers::flat_plate_cf`) was the intended check; no committed version of the test asserts it | Test asserts Cf(0.97) within 5 % of the NASA TMR CFL3D value and within 5 % of the power law `0.0576 Re_x^-0.2` for Re_x <= 5e6 (`crates/rcfd-turb/tests/flat_plate_sst.rs`); White deviation of -6.6 to -8.5 % is printed, not asserted | `f859455` (first appearance; the roadmap criterion text, "Flat plate / BFS / NACA0012 validation passes", never contained a number). Disclosed in the F-043 note | Substituted reference. I reproduced the result: Cf(0.97) = 0.002663 (-0.7 % vs TMR 0.002683), White -6.6..-8.5 %, power law -3.8..+4.6 %. The assertion passes because the chosen reference is the one SST lies closest to; the note says so. The TMR value 0.002683 used in the test differs from the 0.002691 the validation-data pack later found (note in F-125, "tolerance unchanged"). Also: BFS and NACA0012 parts of the criterion are not attempted, so partial is right |
| 2 | F-111 IAPWS viscosity/conductivity tolerance | 1e-8 relative | 1e-7 relative (`rcfd-core/src/iapws.rs` lines 545-573) | (pre-commit) in `77dd778`; disclosed in `ed5899b` (docs/reference-data.md: "set at 1e-8 first, widened to 1e-7 after the 433.15 K, 1 kg/m^3 viscosity point missed it by 3.4e-8 ... a post-hoc change, disclosed") | Post-hoc widening, but physically justified (the table prints 7 to 9 digits, 3.4e-8 is the last-digit rounding) and the IF97 property tests stay at 1e-8. The roadmap note states "1e-8 / 1e-7" |
| 3 | F-111 air viscosity | Pre-registered 1 % for 300-500 K | `air_viscosity_law` (Sutherland) asserted at **1.5 %** (`rcfd-core/src/fluids.rs` line 763); the 1 % assert is applied to the cubic fit laws, which are in-sample fits to the same 3-digit table | (pre-commit) `77dd778`; disclosed in `ed5899b` and in the F-111 note ("Sutherland VISCOSITY NOT met: -0.22, -1.08, -1.46 percent") | The 1 % criterion is **not met** for Sutherland air viscosity and is met for fit laws only circularly (leave-one-out 400 K row -0.22 %). The test is still named `air_properties_match_published_tables_within_one_percent_from_300_to_500_k`, which overstates what it asserts. F-111 is partial, correctly |
| 4 | F-122 WASM user function, position-dependent source | `max abs(u) < 1e-6` in a closed box with `a = (3x,0,0)` | Replaced by "spurious velocity and pressure error fall by at least 1.7 when 10x10 is refined to 20x20" (`rcfd-api/tests/udf.rs` lines 4-14) | (pre-commit) `cda48da`; documented in the test header and the F-122 note ("replaced, not loosened"); measured 4.44e-2 -> 1.09e-2 (4.1x) and 3.86e-2 -> 1.94e-2 (2.0x) | A real replacement of an unmet check by a weaker one (the absolute bound is gone; the remaining spurious velocity is 1.1e-2 on 20x20). The 1.7 floor was set before the 20x20 run per the note, and the pressure ratio 2.0 clears it only by 0.3. The roadmap criterion itself ("changes the solution as expected; cannot access fs/network; ABI versioned") is met by the constant-source, isolation and ABI tests |
| 5 | F-061 API-level nozzle test | Header: residual targets `tolerance_u = tolerance_p = 1e-8` | `e62c9c8` amended it (res_u < 1e-4, "amended after the first run"); `44c6013` restored the original header text and **deleted the residual assertion entirely** (`rcfd-api/tests/variable_density.rs`) | `e62c9c8`, `44c6013`, `c65ffb4` (git diff verified) | Honest in effect: the header now states "(a) is NOT met: res_u = 2.6e-5, res_p = 6e-13 at the 3000-iteration cap" and the F-061 note repeats it. But the run is never flagged converged, and no assertion guards the residual. Only the mass-flow (1e-6) and throat Mach (2 %) checks remain |
| 6 | F-061 choked nozzle | n/a (criterion unchanged: choked mass flow within 1 %) | Test `a2_choked_nozzle_mass_flow_and_supersonic_branch` is `#[ignore]` (about 80 s by the comment; 265 s measured here with 11 tests in parallel). Start-up needed under-relaxation 0.7/0.5/0.7 and a 40-stage back-pressure continuation not registered in the ADR | `aaea0de`; disclosed in the F-061 note ("start-up choices were not registered, the tolerances were not changed") | The 1 % criterion (the only quantitative acceptance of F-061 beyond the subsonic duct) is verified solely by an ignored test, so `cargo test --workspace` does not guard it. I ran it: mass flow -4.05e-4 at inlet and outlet, Mach error 9.2e-4, p/p0 error 1.3e-3. Passes |
| 7 | F-042 k-epsilon BFS reattachment | Criterion: "within literature scatter"; case file gives 6.26 +- 0.10 and a 5.5-7 band | Test `step_kepsilon.rs` (ignored) **never asserts x_r**; the only assertion added in `380e861` is that the smooth/hysteresis wall treatments converge. Reported values: Standard 5.36-5.46 (outside the band), RNG 6.32-6.38 and Realizable 6.40-6.51 (inside 5.5-7, 0.01-0.25 outside the +-0.10 window) | `24a9802` (test), `380e861` (convergence assertion); numbers in the F-042 note | The "6.26 +- 0.10 window" is never an assertion in any version. The note is accurate (RNG/Realizable inside the scatter band, Standard not) and status is partial, which is right; but the criterion is verified by reading printed output. I re-ran the Standard variant: converged at 800 iterations, x_r/h = 5.359 |
| 8 | F-042 channel DNS references | DNS 20.1 / 17.4 in the test | Verified values from the MKM `chan395.means`: 20.13 and 17.54 (note) | The test still uses 17.4 for the bulk value; with 17.54 the bulk errors of RNG (+6.4 %) and Realizable (+7.9 %) are smaller | Harmless; the check is slightly conservative |
| 9 | F-115 interpolate and restart | Hypothesis (not in the criterion): iteration count drops when starting from the coarse solution | Not an acceptance gate; recorded as a negative finding (103 interpolated vs 89 uniform iterations) | `c8ff420` (`rcfd-api/tests/interpolate.rs` header "added after the first run, recorded, not an acceptance gate") | Criterion text unchanged and the roadmap says so. But the criterion that is asserted, "first-iteration normalised residual lower than uniform", is near-vacuous: the uniform start gives 1.0 by construction because the residual is normalised by the solution's own variation. The practical benefit (fewer iterations) was measured negative. Status partial is right |
| 10 | F-031 rest-state in a rotating frame | Criterion fixed beforehand: spurious speed < 1e-3 Omega R, pressure error < 1 % of range | Wave 9 replaced the assertion by "both errors fall with refinement at order > 0.8" (criterion missed: 5.2e-3, 1.62 %); wave 10 (`d1b43f0`) restored the original criterion and the test now asserts speed < 1e-3 and pressure error < 1 % | `e132211` (weak test), `d1b43f0` (original criterion restored, `git show d1b43f0` verified) | The restoration is genuine, but it was achieved by changing the wall-pressure construction (wall value chosen so the wall-cell normal gradient equals the body force) and the stationarity helper now uses a speed-scale floor of 1e-3 for the rest state (`run_with_floor`). The result, 5e-12, is round-off because the discretisation now imposes the balance exactly in wall cells; the roadmap note says this ("the usual fixed-flux wall treatment; only the cases above were checked") |
| 11 | F-050 / ADR 0005 hydrostatic column | n/a | Setup uses mu = 10 Pa s and k = 100 W/m/K instead of air values, because with air properties the steady-conduction column cannot be resolved in double precision | `aaea0de` (`rcfd-fv/tests/varden_hydrostatic.rs` header; note in F-050) | Disclosed. The test therefore checks the balance of pressure, gravity and the ideal-gas EOS, not the behaviour with realistic transport properties. Limits (1e-3, 2e-3 density error) are unchanged from the ADR |
| 12 | F-125 criterion text | A vendor-specific wording of the same rule | "CI never reads from any proprietary install" | roadmap snapshots kept outside the repository -> current (pre-commit; the first commit already has the new text) | Neutral, appropriate wording change |
| 13 | F-094 criterion text (status todo) | "Scripted eval: 5 canned prompts ..." | "Scripted eval with the mock provider (CI) and, optionally, a user-supplied real or local model: ..." | scratch snapshots (pre-commit) | Clarifies that CI uses the mock provider; criterion is arguably weaker. Item is todo, so no effect on counts |
| 14 | F-027 AMG | "Poisson on 1M cells converges ... under 15 V-cycles" | Evidence uses W-cycles (`cycle_index: 2`) as the default; 14 W(1,1)-cycles to 1e-6 (limit 15, marginal); the small-size test allows up to 20 V-cycles (`amg.rs` line 605) | pre-first-commit; disclosed in the F-027 note ("plain V-cycles degrade ... hence W-cycle default") | A different cycle than the criterion names, an unspecified tolerance (1e-6 chosen), and the test is ignored. I re-ran it: 5 levels, stand-alone 14 W-cycles (rate 0.368) and AMG-CG 11 iterations, both assertions pass |
| 15 | F-028 cavity Ghia "within 2 percent" | n/a | Interpreted as max absolute error <= 0.02 U_lid at the Ghia points, u only for Re 400 and 1000, u and v for Re 100 | `24a9802` (`rcfd-fv/tests/cavity.rs`) | An absolute-to-lid-speed reading; per-point relative error would be much larger near zero crossings. Measured errors (0.003-0.009) are well inside either lid-relative reading. The Re 400 and 1000 cases are ignored tests (run here, pass: 0.00323 and 0.00443) |

Also checked and found unchanged since the first commit: the `#[ignore]` set (16 tests) is the same as in the commit where each
test was first added; no `#[ignore]` was added to an existing test by a later commit, and no test was reverted.
`421ba19` (line-ending normalisation) and `4c9fcf5` (line-ending restore) change no assertions (verified with
`git diff -w --ignore-cr-at-eol`).

## 3. Summary table: every done or partial item (74 items)

Columns: claimed status; verified status; notes. "V" = verified by my reading of tests/notes and, where marked, by running
the test. "Ign." means the evidence is an `#[ignore]` test (I ran it). Proposed corrections are in section 7.

| Item | Criterion (short) | Claimed | Verified | Notes |
|------|-------------------|---------|----------|-------|
| F-001 | build/test pass in CI on Windows+Linux; acyclic graph | done | **partial** | Local build and tests pass here; the CI workflow has never run on a host (note says so). Acyclic graph enforced by test |
| F-002 | CI runs fmt, clippy -D warnings, tests; clean-room policy | done | partial | `ci.yml` runs all three; never executed on a CI host. fmt and clippy pass locally on tracked files |
| F-003 | unit conversion tests; dimension checking | done | done | Tests pass |
| F-004 | ADRs merged under docs/adr and reviewed | done | **partial** | Note: "human review/merge still pending", and names ADRs 0001-0003 although 0004 and 0005 exist |
| F-005 | round-trip, schema export, dotted-path set/get | done | done | |
| F-006 | cases/ with public reference data; runner lists them | done | done | Ghia and de Vahl Davis tables cross-checked in wave 6a; NACA0012 has no mesh generator |
| F-010 | invariants; analytic volumes | done | done | |
| F-011 | metrics match hand values | done | done | |
| F-012 | meshes pass F-011, used by cases | done | done | |
| F-013 | reads public sample meshes | done | done | Real Gmsh 4.15.2 output |
| F-014 | readers load a sample and pass F-011 | partial | partial | CGNS missing, hand-built samples only |
| F-015 | write then read identical | done | done | |
| F-016 | balance within 5 %, halo tests | done | done | Weighted RCB, not a graph partitioner; imbalance test < 0.05 (2..16 parts) |
| F-020 | BC tests, trait | done | done | |
| F-021 | exact for linear on distorted mesh; order | done | done | Measured L2 order about 1.5, not 2 (disclosed) |
| F-022 | order and boundedness | done | done | |
| F-023 | order 2 on skewed mesh | done | done | Orders 1.75/1.92/1.98 |
| F-024 | decay ODE orders | done | done | |
| F-025 | reproducible parallel assembly | done | done | |
| F-026 | residual reduction | done | done | |
| F-027 | 1M cells under 15 V-cycles | done | **partial** | W-cycles not V-cycles, 14 of 15, tolerance 1e-6, Ign. (run: pass) |
| F-028 | cavity Re 100/400/1000 within 2 % of Ghia | done | done | Ign. for Re 400/1000 (run here: pass, 246 s total, not 15 min). u only at 400/1000. Lid-relative reading |
| F-029 | Taylor-Green under 1 %; cylinder St within 5 % | done | done | Taylor-Green is a default test. Cylinder St 0.1588 vs 0.165 (-3.8 %) is Ign. (run here: pass, 465 s). Note: TG KE error changed from 0.38 % to 0.30 % after a fix to the test's initial pressure sign |
| F-030 | bit-identical restart; CSV | done | done | |
| F-031 | Ergun bed and rotating channel; rest state | partial | partial | Acceptance met; rest-state result is round-off only because of the wall treatment (row 10). Hash-identity claim not reproducible |
| F-032 | coupled solver needs fewer outer iterations than SIMPLE | partial | partial | Met against classic SIMPLE 0.7 (165 vs 610) but equal to SIMPLEC at equal relaxation (165 vs 167); slower in wall time. The note says all this. Ign. `coupled_variants_report` re-ran and printed the same table |
| F-040 | y+, wall distance, log law | done | done | |
| F-042 | channel within 10 % of DNS; BFS in scatter | partial | partial | Row 7. Channel asserts centre/bulk vs DNS and the log layer vs Reichardt (not vs DNS profile) |
| F-043 | flat plate / BFS / NACA0012 | partial | partial | Row 1. Flat plate Ign. (run: pass, 263 s); BFS and NACA0012 not attempted |
| F-050 | de Vahl Davis Nu within 2 % | partial | partial | Acceptance met (default tests); scope partial. References first typed from memory, later cross-checked |
| F-053 | species mass conserved to machine precision | done | done | Boundary-flux balance 1e-16 versus limit 1e-12 |
| F-060 | Sod L1; oblique shock within 1 % | done | done | Shock angle error 0.025 %. Ign. fine-grid wedge re-run: 39.316 vs 39.314 deg |
| F-061 | nozzle isentropic; choked within 1 % | partial | partial | Rows 5 and 6. Acceptance met for ideal gas; choked check only in an Ign. test (run: pass) |
| F-070 | files open in ParaView; values match | done | done (caveat) | ParaView never run; VTK python library used instead |
| F-071 | integrals on analytic fields | done | done | |
| F-073 | CLI renders contour for 1M cells in under 2 s | done | **not demonstrated** | No test or note measures any render time or any 1M-cell mesh; 5 small tests only |
| F-074 | JSON schema, HTML report | done | done | |
| F-080 | validate reports precise errors; schema | done | done | |
| F-081 | subcommands documented and tested | done | done | |
| F-082 | example builds cavity and runs 10 iterations | done | done | |
| F-084 | recorded session replays to identical hash | done | done | Journal not written to disk by the CLI |
| F-085 | start, residual events, slice, stop; protocol doc | done | done | JSON-RPC only; gRPC not provided (criterion allows either) |
| F-090 | MCP client drives cavity | done | done | Own client only |
| F-091 | path escape, caps, approval flags | done | done | |
| F-092 | Claude Code panel renders and round-trips | partial | partial | Demoted, not verified in a live host |
| F-093 | plots update within 1 s | partial | partial | Demoted; by calculation |
| F-104 | expressions with dimension checking | done | done | |
| F-105 | profile and table inputs | done | done | Limits 2 % and 3 % set in advance; measured 0.66 %, 1.7 % |
| F-106 | pseudo-transient converges cavity and BFS where plain SIMPLE needs lower relaxation | partial | partial | The premise was found false after the h_from_solution fix (plain SIMPLEC now converges); Re_h = 400 step still needs specific recipes. Honestly recorded |
| F-107 | periodic channel; 60-degree sector | done | done | Sector vs full ring 1.1e-5 |
| F-108 | flat-plate mesh from target y+ meets it within 10 % | partial | partial | Block meshes only; solved y+ 0.48-0.54 for target 0.5 (Ign. flat-plate test, run: pass) |
| F-109 | report-driven stop with recorded reason | done | done | |
| F-110 | power-law analytic profile; Carreau limits | done | done | Law forms from memory, marked unverified |
| F-111 | air/water within 1 % of tables; mixture rules; no licensed data | partial | partial | Row 3. Water: all IF97 verification points at 1e-8, transport at 1e-7 (row 2) |
| F-114 | non-matching blocks equal conformal; conservation to round-off | done | done | Planar interfaces only |
| F-115 | lower initial residual than uniform init; modes selectable | partial | partial | Row 9 |
| F-116 | time average matches post-computed; limits honoured | done | done | Ign. shedding test run: 12 sign changes, mean difference 5e-15. Note the comparison is incremental statistics versus post-computation from the same run, so it checks bookkeeping, not an independent average |
| F-117 | GLB loads in a validator and a viewer | partial | partial | Validator and viewer never run, so the acceptance is not demonstrated (the note says so) |
| F-118 | Fluent 2D and 3D import | partial | partial | The one real file test (`elbow.msh`) is skipped when absent, so default CI exercises only self-written files |
| F-120 | channel-flow workflow runs; serialisable; MCP tool | done | done | |
| F-121 | check-only, continue-from, interrupt, status stream | partial | partial | Acceptance met |
| F-122 | WASM momentum source; sandbox; versioned ABI | done | done | Row 4 |
| F-125 | fetch script, checksum, licence for each dataset | partial | partial | 7 of 8 datasets; Hannover compressor unresolved |
| F-140 | headless smoke open/save/reload | done | done | Never looked at visually |
| F-141 | layout unit tests | done | done | |
| F-142 | tree covers every schema field | done | done | |
| F-143 | cavity speed field shows vortex | partial | partial | No human viewing |
| F-144 | plots update while running | partial | partial | No human viewing |
| F-145 | headless frames render pending approval card | done | done | |
| F-146 | fixture transcripts; key never in errors | done | done | OpenAI shapes from memory of docs (fetch blocked); no live provider |
| F-147 | full mock-provider loop | done | done | |
| F-148 | unit tests on synthetic series | done | done | Note is stale: it says stop reasons "once they exist" but F-109 is done |
| F-149 | keys redacted; 0600 on unix | partial | partial | Unix path never run |
| F-150 | offline quick actions; verified with a local server by a human | partial | partial | Human verification pending |

## 4. Test results

Commands run with `CARGO_TARGET_DIR=<target dir>`, rustc/cargo 1.99.0, Windows 11, 16 cores.

- `cargo test --workspace --release`: exit 0. 83 test suites, **458 passed, 0 failed, 16 ignored**.
- `cargo fmt --check`: exit 0, no diff.
- `cargo clippy --workspace --all-targets -- -D warnings`: **exit 101**, one error:
  `crates/rcfd-api/examples/baseline_hash.rs:98: error: replacing text with itself (clippy::no_effect_replace)`. That file is
  not tracked (`git ls-files` does not list it; `.git/info/exclude` hides it; its header says "Throwaway ... not committed").
  With `--lib --bins --tests --benches` plus the tracked `cavity` example, clippy exits 0 with no warnings. So CI on a fresh
  clone should pass, but a developer running the exact documented command in this checkout gets a failure.
- Ignored tests (16 total). Run individually in release mode with `--ignored --nocapture`:

| Test | Result | Wall time (parallel runs) |
|------|--------|------|
| rcfd-fv `cavity` re400, re1000 | pass; max error vs Ghia 0.00323 and 0.00443 | 246 s (not the 15 min the note claims) |
| rcfd-fv `cylinder` Strouhal | pass; St = 0.1588 vs 0.165 | 465 s |
| rcfd-api `statistics` shedding | pass; mean-field difference 4.9e-15, 12 probe sign changes | 305 s |
| rcfd-fv `coupled_variants_report` | pass (report only) | 181 s |
| rcfd-fv `varden_cavity` t1, t2 | pass | 245 s |
| rcfd-fv `varden_nozzle` a2 choked | pass; mass flow -4.05e-4, Mach error 9.2e-4 | 265 s |
| rcfd-turb `flat_plate_sst` | pass; Cf(0.97) = 0.002663 | 263 s |
| rcfd-turb `step_kepsilon` (default variant Standard) | pass (asserts convergence only); x_r/h = 5.359, converged at 800 | 131 s |
| rcfd-fv `compressible::oblique_shock_fine_grid` | pass; 39.316 vs 39.314 deg | 95 s |
| rcfd-linalg `poisson_one_million_cells` | pass; 14 W-cycles, AMG-CG 11 | 7 s |
| rcfd-fv `step_laminar` sweep | pass (prints only, no assertion on x_r) | 587 s |
| `explore` (natural_convection), `explore_step` (pseudo), `explore` (step_laminar) | skipped on purpose: exploration entries, no acceptance role | |

Sweep note: `laminar_step_validation_sweep` has no pass/fail assertion on reattachment length (it prints values), so its
pass verifies nothing. It reproduced the F-106 note values for Re_h 100/200/389 (4.949, 8.182, 11.527 versus 4.95, 8.19,
11.54). No test took longer than 15 minutes (longest 587 s with 11 test processes sharing 16 cores).

## 5. Status checks: done items whose evidence is ignored, missing, or weaker than the criterion

- **F-073 (done)**: criterion "renders a contour image in under 2 s for a 1M-cell mesh". There is no timing and no 1M-cell
  render anywhere in the repo (searched `Instant::now`, `1_000_000`, `million` in `rcfd-post`). Not demonstrated.
- **F-027 (done)**: V-cycle criterion versus W-cycle evidence, tolerance chosen by the author, marginal 14/15, ignored test.
- **F-001, F-002 (done)**: criteria require passing CI on two OSes; no hosted run exists.
- **F-004 (done)**: criterion includes "reviewed"; note says review pending.
- **F-028, F-029 (done)**: the Re 400/1000 cavities and the cylinder Strouhal gates exist only as ignored tests. They pass
  when run. A default `cargo test --workspace` therefore gives no protection for those parts of the criteria.
- **F-116 (done)**: the vortex-shedding part is an ignored test; the averaging check compares two computations on the same run.
- **F-061 (partial)**: choked-flow gate is ignored; API wiring residual target dropped from the test (rows 5, 6).
- **F-042 (partial)**: reattachment never asserted (row 7).
- **F-148 (done)**: stale note only.
- **F-070 (done)**: "opens in ParaView" never run; read back with the VTK python library.
- No done item had a missing test file. No `#[ignore]` was added after a test was first committed.

## 6. Unreproducible hash claims

`baseline_hash.rs` is the only implementation of the "bit-identical baseline" check cited in F-031 (wave 10 note), in the
F-050 update and as ADR 0005 check d1 ("hashes of the existing paths identical before/after (every commit)"). It is
excluded from git and no expected hash values are stored anywhere in the repo, so a reviewer cannot repeat the check.
Recommend either committing it with expected values as a test or removing the claim from the notes.

## 7. Recount and recommended status corrections (proposals only; roadmap not edited)

Current (roadmap.json, verified by script): total 114 = 53 done, 21 partial, 40 todo. Weighted (partial = 0.5): 63.5 (55.7 %).

Gap items F-104..F-139 (36 ids): 9 done (F-104, 105, 107, 109, 110, 114, 116, 120, 122), 8 partial (F-106, 108, 111, 115,
117, 118, 121, 125), 19 todo. Weighted 13.0 of 36 (36.1 %). Non-gap items: 44 done, 13 partial, 21 todo (50.5 of 78).

Proposed changes:

| Item | From | To | Reason |
|------|------|----|--------|
| F-001 | done | partial | criterion requires CI on Windows and Linux; never run on a host |
| F-002 | done | partial | same; the CI file has never been executed |
| F-004 | done | partial | "reviewed" not met; note is stale (lists ADRs 0001-0003 only) |
| F-027 | done | partial | V-cycle vs W-cycle, marginal, ignored test only |
| F-073 | done | partial (or todo for the performance clause) | 1M-cell, 2 s timing never measured |

With these five: 48 done / 26 partial / 40 todo, weighted 61.0 (53.5 %). Gap-range counts are unchanged (9 / 8 / 19).

Non-status recommendations:
1. Add a committed pass/fail assertion for the k-epsilon BFS reattachment (or state in the criterion that it is a reported
   value), and for `laminar_step_validation_sweep`.
2. Rename the air test or split the 1 % fit-law assertion from the 1.5 % Sutherland guard; mark the fit law check as in-sample.
3. Run the cavity Re 400/1000, cylinder, choked nozzle and flat plate tests in a scheduled CI job; they take 2 to 8 minutes each.
4. Either commit the baseline-hash tool with expected values or drop the claims (section 6).
5. F-115: reword the criterion to measure something non-vacuous (iterations or error to the converged solution).
6. Fix the F-148 note, the stale ADR list in F-004, and the F-106 note's reference to completed work.
7. Re-run `cargo clippy` in a clean clone to confirm the one local failure is the excluded file.

## 8. ROADMAP.md versus roadmap.json drift

Parsed both files: 114 items in each, identical ids, titles, dependency lists, acceptance text and status (53 [x], 21 [~],
40 [ ] in the markdown). Differences are in `note` fields only, 33 items (26 with no markdown note, 7 with different text):

- ROADMAP.md has **no note** for 24 done/partial items (F-029, 071, 073, 074, 080, 081, 082, 084, 085, 090, 091, 092, 093,
  140-150) and two todo items (F-055, F-095) that roadmap.json documents. In particular F-029 (cylinder result) and F-092/F-093 (demotion to
  optional extras) are invisible in the markdown.
- Stale content in the markdown: **F-006** says the Ghia/de Vahl Davis tables were "transcribed from memory; verify against the
  papers before using as hard gates", while the JSON says they were cross-checked in wave 6a. **F-028, F-042, F-060** carry the
  pre-6a wording ("from memory / not checked") versus verified wording in the JSON. **F-107** lacks the "Done: ..." detail.
  F-112 and F-126 notes are shorter.
- The JSON header says it is the "source of truth for automation"; no automatic check ties the two files together. I
  recommend generating the markdown from the JSON.

## 9. Vendor-derived content check

Grep of tracked files (253) for vendor and product names and for install-path patterns, and a read of each hit:

- Neutral names and public references: `rcfd-io::fluent` module, `docs/mesh-import.md` (states that no vendor text, tool or
  file was used), `validation-data/manifest.json` and `docs/validation-data.md` (the Fluent elbow mesh comes from the public
  OpenFOAM-11 tutorial, GPL, fetched not vendored), `docs/case-schema.md` (Fluent profile syntax not implemented),
  `sim.rs` doc comment (counterpart of a vendor interpolate-file command and initial-file option), `schema.rs`
  (accepted mesh file formats). Acceptable.
- Constants cited from a non-published source: `kepsilon.rs` header and `docs/reference-data.md` (RNG constants and
  the wall-function constants). These are textbook constants, but the citations were not of papers (see
  [provenance](provenance.md)), and
  the CONTRIBUTING rule is "textbooks/papers only". Low risk, worth a second source. (Later edit: the entries now name a
  published source where one exists and are otherwise labelled unverified; see `docs/reference-data.md`.)
- Guard code: `rcfd-io/src/datapack.rs` rejects manifest URLs that point at a proprietary install or a local path; its test
  uses a fake URL. Fine.
- **Needs a decision:** the roadmap notes of 23 items (F-014, 042, 104-109, 113, 118, 119, 124, 126, 128-132, 134, 136-139)
  described their gaps with vendor-specific setting and module identifiers rather than generic capability names, and
  `ROADMAP.md` cited an inventory file that is not in the repository. No code, prose or data was copied, and the features
  themselves are generic CFD capabilities. Recommend rewording these as generic capability descriptions and keeping any
  such inventory out of the repository. (Later edit: done; the roadmap notes now use generic wording.)
- Material kept outside the repository during the audit is not part of the repository. `validation-data/` and `cases/`
  contain only small public-literature tables and the fetch manifest.

## 10. What I could not check

- Hosted-CI behaviour, Linux builds, ParaView, glTF validator, any real local model server, any GUI appearance.
- Pre-commit history of tolerances: reconstructed only from disclosures. If any edit was made without disclosure, git cannot show it.
- Whether the reference values typed from memory (several remain flagged "unverified" in `docs/reference-data.md`, for example
  the non-Newtonian law forms and mixture rules) are correct; I did not fetch any source.
- I did not re-derive any physics or run Linux.
