# ADR 0679: F-737 registration: wave-tag mapping and a primary ADR in the test-to-ADR index

Status: **REGISTERED 2026-11-17, written before any code of F-737 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no code of F-737 was written; the numbers below were read from a run of the unchanged
`scripts/adr_test_index.py` at c2ec31e4 (disclosed in "What was read"). Results go in the addenda at the end; nothing above the
addenda is changed after a run.

Roadmap item F-737 (deps F-723), proposed by [ADR 0653](0653-f723-registration-daily-full-gate-with-failures-attributed-to-adrs.md)
addendum 2 (81 integration-test binaries left unmapped, most of them early wave files whose ADR names the wave, not the file).
Number 0679 given by the lead. Branch `f737-work` from c2ec31e4, worktree `w49i-work/f737`, scratch `w49i-work/f737-scratch`.

House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here; each registered run made once; existing tests not edited;
no Rust file, hash table or golden touched; stdlib Python only; clean-room (nothing outside the repository is used); the roadmap
files are not edited. Choices the maintainer would normally be asked about are taken on the maintainer's standing instruction of
2026-11-11 (take the recommendation), except where listed as open.

## What F-737 asks

> Every limit not stated here is registered in the ADR before the run. The index maps wave-named test files (w34a, wave17a, w48a) to the ADRs named after their wave, and each test gets a primary ADR (the registering ADR); the integration-test binaries left unmapped in ADR 0653 addendum 2 (81) fall below a registered number; a registered sample of primary ADRs is checked by reading.

## What was read

- `scripts/adr_test_index.py` (evidence kinds E1 item in the file name, E2 an ADR cites the .rs file, E3 an ADR cites a distinctive
  function, E4 a roadmap note cites file or function, E5 unit-test module path), `scripts/gate_attribute.py` (reads only
  `tests[].adrs`, `.items`, the `binaries` and `items` tables of the index), `.github/workflows/gate.yml` (uploads the index and
  attribution.json; no field of the index is parsed there).
- One run of the unchanged index at c2ec31e4 (`f737-scratch/base_index.json`, 77 s): 1170 integration binaries of which 1089 mapped;
  all tests 7229 of 7713 (93.7 %). **79 integration binaries with tests are unmapped** (ADR 0653 said 81 at its commit; the
  difference is two binaries with 0 tests found and the commits since) plus 2 with 0 tests found. This is the baseline of the counts below.
- ADR file names: 120 distinct wave tags `waveNN[x[d]]` (for example `0072-wave34a-...`, `0023-wave17a-...`, `0130-wave48a-...`);
  no tag is used by two ADR files. A probe of the baseline index with the tag regex below: 1042 tests (157 binaries) carry a wave tag;
  of the 82 distinct tags 26 have no wave ADR (`49a` to `49r`, `48i` to `48z`, `31`). Wave ADRs end at 0144 (`wave48j`).
- Of the 79 unmapped binaries, the ones whose tag has a wave ADR (by hand from the list): `cgrid_case_w34a`, `convergence_wave17a`,
  `divergence_guard_w32c`, `radial_frame_w35c`, `w26c_reports`, `w37a_velocity_correction`, `run_flags_w48a`, `events_w48a`,
  `contract_w48a`, `sst_moving_wall_w34d`: **10**. A count of which binary stems occur as a whole word in ADR text (excluding 0653,
  which lists them): about 13 studio and other stems occur in at most 3 ADRs, for example `browser_refresh_r12` (0141),
  `live_preview_cost_j` (0144), `mesh_transform_u` (0169); about 30 stems are generic words in many ADRs or in none.

## Design

**Wave tag (rule W).** The tag of a test is read from the stems of its file and of its binary with
`(?:^|_)(?:w|wave)(\d{2})([a-z]\d?)?(?=_|$)` (lower case), normalised to `NN` plus the optional letter and digit (`w34a` and `wave34a`
give `34a`). The tag of an ADR is read from its file name with `^\d{4}-wave(\d{2})([a-z]\d?)?-`. Rule: **exact match only**. A test tag
that equals the tag of exactly one ADR file gets that ADR as evidence E6. A test tag with no equal ADR tag but that is a proper
prefix of ADR tags (`w48g` against `48g1` to `48g4`, `w31` against `31a` to `31c`) is **ambiguous**: it maps nothing and is listed with its
candidates. A tag with no ADR at all is listed as "no wave ADR". Two ADR files with the same tag would also be ambiguous (none now).
E6 applies to every test with a tag, mapped or not (an ADR named after the wave is evidence for each of its tests). It is additive:
no test loses an ADR.

**Binary named in ADR text (rule B, evidence E7).** For a test binary stem `s` that contains `_`, has at least 10 characters and is
unique among the .rs file stems of the workspace, the ADRs whose text contains `s` as a whole word are E7 evidence, provided at most
3 ADRs do (a stem in more ADRs is a generic word and is listed as "too common", not guessed). ADRs 0653 and 0679 are not evidence
(they list the binaries). The roadmap notes are not used for E7.

**Primary ADR (rule P).** For every test with at least one ADR, `primary_adr` is chosen by evidence tier, first tier that has an ADR
wins: E1 (the ADRs of the item named in the file name) > E6 > E2 > E3 > E7 > E5 > E4. Within the tier the candidates are ordered:
file name contains `registration` or `-wave` first, then the lowest number (a registration is written before the code, so the
earliest of the registering ADRs is the one that named the test first; later addenda have higher numbers). The test gets
`primary_adr` and `primary_evidence` (the tier); the `adrs` entries of the index get `primary_tests`. The rule does not use git.
Reason: a git-history rule (the ADR first citing the file in time) needs the ADR dates, which the ADR files do not carry uniformly;
the number order is the order of writing, and the sample check by reading below is the test of the rule.

**Additive format.** `tests[]` gains `wave`, `primary_adr`, `primary_evidence`; `coverage` gains `wave` (counts and the ambiguous,
no-ADR and too-common lists) and `primary`; `adrs[n]` gains `primary_tests`. No existing field changes its name or type; `tests[].adrs`
may only gain entries.

## Checks (limits fixed now)

| id | limit | estimate behind it |
|---|---|---|
| W1 | every integration binary whose test tag equals the tag of exactly one wave ADR file is mapped to it: 100 % of those; at least 9 of the 10 listed above leave the unmapped list | all 10 have an exact tag in the ADR file list |
| W2 | ambiguous tags and tags without a wave ADR are listed, none is mapped (checked by the negative controls) | rule W |
| U1 | integration binaries with tests that are unmapped: **at most 65** (baseline 79) | 10 by rule W, about 4 by rule B (about 13 stems, some already mapped through another test, some with generic cites); 79 - 14 = 65 |
| P1 | every test with at least one ADR has exactly one `primary_adr`, and it is in the test's `adrs`; a test without ADR has none: 100 % | by construction |
| P2 | sample A: 20 test files drawn with `random.Random(737).sample` from the sorted list of distinct test files that have a primary ADR; each read, with its primary ADR; at least **17 of 20** agree (the ADR is the one that registered or first described the file) | ADR 0653 T4 had 20 of 20 on the same kind of check; the tier order is new |
| P3 | sample B: the files whose primary evidence is E6 or E7, 15 drawn with seed 737 from the sorted list (all if fewer than 15); at least **12 of 15** agree | the weakest evidence, so a lower limit (80 %) |
| A1 | additive: every test of the baseline index keeps every ADR it had; the share of tests whose `adrs` grew is at most 12 % of all tests | 1042 tagged tests, of which about 700 have a wave ADR: at most 9 % |
| A2 | all tests mapped to an ADR: at least the baseline share, 93.7 % | A1 and U1 only add |
| C1 | `python -m unittest scripts/test_adr_test_index.py scripts/test_gate_attribute.py scripts/test_gate_test.py` passes, with the new tests; the existing tests are not edited | |
| C2 | `scripts/test_gate_attribute.py` (end to end) passes unchanged, and `gate_attribute.py attribute` on the new index lists for the real-failure sample of ADR 0653 T5 the ADRs it listed before plus possibly a wave ADR; the workflow file is not changed | the script reads `adrs` and `items` only |
| C3 | if `docs/` other than the ADR is touched, `cargo test -q --release -p rcfd-cli --test docs_site` passes (CARGO_TARGET_DIR in the scratch folder) | |

Not a limit: the number of E6 or E7 mappings of tests that were already mapped.

## Negative controls and planned mutations

Controls, in `scripts/test_adr_test_index.py` (fake workspace, written before the run of the real index):

- N1 a planted file `zz_w99x_case.rs` (tag with no wave ADR) stays unmapped and is in the "no wave ADR" list.
- N2 a planted tag `w50` with ADR files `wave50a` and `wave50b` is ambiguous: not mapped, both candidates listed. A planted tag
  `w51a` with two ADR files of tag `51a` is ambiguous in the same way.
- N3 a planted file `y_wave10a_q.rs` with the one ADR `wave10a` is mapped with E6 and that ADR is primary.
- N4 a stem cited by 4 ADRs is "too common" and not mapped; a stem cited by 2 ADRs is mapped to both; a stem cited only by ADR 0653 is not.
- N5 a test with E1 (ADR 5) and E2 (ADR 3, lower number) has primary 5; two ADRs in one tier, one with `registration` in its name and a lower
  number elsewhere, the registration one is primary.
- N6 old fields: the index of the fake workspace still has every field the 0653 tests read.

Mutations on a scratch copy: M1 rule W with a prefix match accepted, caught by N2; M2 E7 threshold 3 raised to 99, caught by
N4; M3 tier order E2 before E1, caught by N5; M4 `registration` preference dropped, caught by N5; M5 E6 skipped for tests that already
have an ADR, caught by the wave test on a mapped file; M6 `primary_adr` chosen from the highest number, caught by N5. Each mutation
must make at least one test fail.

## Decision rule

**done** if W1, W2, U1, P1, P2, P3, A1, A2, C1, C2 (and C3 if docs are touched) are all met and every mutation is caught. **partial** if
any limit is missed (the limit stays and the miss is recorded in the addendum with the measured number) or a part was not run. Each
run is made once; a defective set-up gets an amendment before a corrected run.

## Limits (what is not covered)

- Tests whose files carry neither a wave tag nor an item number nor a citation (about 65 binaries, for example `varden_*`, `gpu/*`,
  `assistant/*`) stay unmapped; they need an ADR written for them or a decision of the maintainer, not a guess.
- Wave tags of the 26 waves with no wave ADR (`49a` to `49r`, `48i` to `48z`, `31`) are reported, not mapped; the ADRs of those waves are
  named after items.
- The primary ADR is the registering ADR by rule P, not proved from git; P2 and P3 test the rule on 35 files at most.
- The workflow on GitHub is not run; the attribution output is checked locally only.

## Amendment 1 (2026-11-17): the primary rule had no tier for the ADR a test file names itself

Written and committed before any code of the amendment and before the corrected run. The first run of the registered code
(index `f737-scratch/new_index.json`) gave these numbers, all with the limits of the table above intact:

| id | measured on the first run | met |
|---|---|---|
| W1 | all 10 listed binaries mapped to their wave ADR; the tag set: 56 tags with exactly one wave ADR, 1 ambiguous (`31`: ADRs 0062 to 0064), 25 tags without a wave ADR | yes |
| U1 | 49 integration binaries with tests unmapped (baseline 79; limit 65): 10 by rule W, 20 by rule B and other evidence already in the index | yes |
| P1 | 0 violations in 7713 tests | yes |
| A1 | 0 tests lost an ADR; 835 of 7713 tests (10.8 %) gained one (limit 12 %) | yes |
| A2 | all tests mapped 96.1 % (limit 93.7 %) | yes |
| P3 | sample B (15 of 135 files, seed 737): 14 agree; the one disagreement is `tick_cost_r6.rs` (primary 0139, the file says ADR 0133 R6) | yes (limit 12) |
| P2 | sample A (20 of 1243 files, seed 737): **11 agree, 9 disagree** (limit 17) | **no** |

The nine disagreements of sample A, by reading: `design/nsga2.rs` (0220, the file names ADR 0209), `runner_start_r9.rs` (0143; the
file names ADR 0133), `sa_cyclic_beams_full_rotor_f674.rs` (0595; the file names ADR 0601), `design/pareto.rs` (0118 by E5; written
for F-249), `simple.rs` (0023; the SIMPLE code is older), `starter_run_m.rs` (0186; the file names ADR 0147), `reaction_f255.rs` (0057;
the file names ADR 0208), `sliding_case_f303.rs` (0216; the file names ADR 0637), `sa_newton_f624_bits.rs` (0517; the file names
ADR 0566). Cause: in seven of the nine the test file's own header comment names the ADR that registered it (`F-303 (ADR 0637)`),
and rule P does not read it; E1 and E2 take ADRs from the item's note (which cites parent ADRs) or from any ADR that lists the
file. The two others (`pareto.rs`, `simple.rs`) are source files with no ADR in the header and stay wrong by rule P.

The P2 miss is recorded; the limit stays (17 of 20). The set-up of rule P is the defect, so a corrected rule is registered here and
run once on a fresh draw, as the house rules allow.

**Rule P' (amends rule P only).** A new top tier **E0** is added: the first `ADR NNNN` named in the file's leading comment block
(the `//!` and `//` lines before the first line of code of the file that holds the test) when ADR `NNNN` exists in `docs/adr` and
is not 0653 or 0679. E0 is also evidence (the ADR is added to the test's `adrs`, additive). Order of the tiers:
E0 > E1 > E6 > E2 > E3 > E7 > E5 > E4. Nothing else changes.

**Corrected checks (the table above with these replacements, limits unchanged unless stated):**

| id | limit |
|---|---|
| P2' | sample A': 20 files drawn with `random.Random(7370).sample` from the sorted list of distinct test files with a primary ADR under rule P'; each read; at least **17 of 20** agree (the same limit as P2; sample A is not reused, the seed is new; any overlap with sample A is disclosed) |
| P3' | sample B': 15 files with primary evidence E6 or E7 under rule P', seed 7370; at least 12 of 15 agree |
| U1', A1', A2', P1' | U1 (at most 65), A1 (no ADR lost; at most 12 % of tests gained one, counting E0 gains), A2 (at least 93.7 %), P1 (100 %) re-checked on the corrected run |
| E0-yield | not a limit: how many files have an E0 ADR, and how many of them differ from the first-run primary |

New negative controls: N7 a file whose header names ADR 0105 and whose name carries item F-020 (ADRs 0110 and 0111) has primary 0105
with evidence E0; N8 a header naming a non-existent ADR, and a header naming ADR 0653, give no E0; N9 an `ADR` mention after the first
code line is not read. New mutation M7: E0 dropped from the tier list, caught by N7.

Decision rule as above, with P2' and P3' in place of P2 and P3; the first-run P2 miss is stated in the addendum whatever the result
of P2'. If P2' is met, the proposed status is done with that disclosure; if it is missed, partial.

## Addendum 1: results (2026-11-17)

Code: `scripts/adr_test_index.py` (evidence E0, E6 and E7, `primary_adr`, `primary_evidence`, `wave` per test; `coverage.wave` and
`coverage.primary`; `adrs[n].primary_tests`), new tests `scripts/test_adr_test_index_wave.py` (13 tests, N1 to N9). The existing
tests of the index and of `gate_attribute.py` are not edited; `gate.yml` and `gate_attribute.py` are unchanged. No Rust file, golden
or hash table touched. Inputs and logs in `w49i-work/f737-scratch` (`base_index.json` the unchanged index, `new_index.json` the
first run, `new_index2.json` the corrected run, `sample.json`, `sample2.json`, `measure.py`, `mutate.py`, `mutations.log`,
`docs_site.log`). Each index run was made once: the baseline, the first run of the registered rule, and the corrected run of
Amendment 1.

| id | limit | measured (corrected run, rule P') | met |
|---|---|---|---|
| W1 | all binaries with an exact one-ADR tag mapped; at least 9 of the 10 listed leave the unmapped list | 10 of 10 (`cgrid_case_w34a` 0072, `convergence_wave17a` 0023, `divergence_guard_w32c` 0067, `radial_frame_w35c` 0078, `w26c_reports` 0048, `w37a_velocity_correction` 0084, `run_flags_w48a`, `events_w48a` and `contract_w48a` 0130, `sst_moving_wall_w34d` 0075); 56 tags have exactly one ADR | yes |
| W2 | ambiguous and no-ADR tags listed, not mapped | 1 ambiguous (`31`: 0062, 0063, 0064), 25 tags without a wave ADR (`48i` to `48z`, `49a` to `49r`); controls N1, N2 | yes |
| U1' | unmapped integration binaries with tests at most 65 (baseline 79) | **25** (first run, before E0: 49). Remaining: rcfd-api 8 (`mesh_params`, `non_conformal_case`, `properties`, `runctl`, `scalars`, `sources`, `udf`, `workflow`), rcfd-assistant 6, rcfd-cli 3, rcfd-core 1, rcfd-fv 2, rcfd-io 2, rcfd-post 2, rcfd-turb 1 | yes |
| P1' | every mapped test has exactly one primary ADR in its `adrs`; none without ADR | 0 violations in 7713 tests (7577 have a primary: E0 7121, E2 417, E4 17, E5 14, E7 7, E3 1) | yes |
| A1' | no test loses an ADR; at most 12 % of tests gain one | 0 lost; **1222 of 7713 (15.8 %) gained one** (first run, wave and E7 only: 835, 10.8 %) | **no** |
| A2' | all tests mapped at least 93.7 % | 98.2 % (7577 of 7713; integration binaries 1143 of 1170, 97.7 %) | yes |
| P2' | sample A' (20 of 1272 files, seed 7370, no overlap with sample A) at least 17 agree | **17 agree**, 3 disagree | yes (at the limit) |
| P3' | sample B' (E6 or E7 primary: the pool has only 4 files after E0 took over the wave files) at least 12 of 15 | **1 of 4 agree**; 12 of 15 cannot be reached | **no** |
| C1 | unit tests pass | `python -m unittest discover -s scripts`: 85 run, OK, 4 skipped (the E2E tests behind `GATE_TEST_E2E`); the two index test files are 12 existing and 13 new tests | yes |
| C2 | `test_gate_attribute.py` unchanged and passing; the ADR 0653 T5 failing test keeps its ADRs | passes; `w49r_default_meshes_are_bit_identical` has ADRs 0181, 0653 before and after, primary 0181 | yes |
| C3 | docs_site passes (an ADR file was added under docs/) | `cargo test -q --release -p rcfd-cli --test docs_site`: 2 passed, 0 failed | yes |

Mutations (`mutations.log`, run on a scratch copy of `scripts/`): M1 a prefix match accepted for a tag (caught by
`test_n2_ambiguous_not_mapped`), M2 E7 threshold 99 (`test_n4_binary_named_in_adr_text`), M3 E2 before E1
(`test_n5_primary_tier_order`, `test_wave_is_added_to_a_file_that_is_already_mapped`), M4 `registration` preference dropped
(`test_n5_registration_before_lower_number`), M5 E6 skipped for tests that already have an ADR (the same two as M3), M6 highest
number first (`test_n4_binary_named_in_adr_text`, `test_n5_registration_before_lower_number`), M7 E0 dropped
(`test_n7_header_adr_is_primary`): 7 of 7 caught.

**Sample A (first run, rule P, seed 737): 11 of 20 agree, the limit of 17 was missed** (the nine disagreements are named in Amendment 1).
Sample B (first run, 15 files, seed 737): 14 agree, one disagrees (`tick_cost_r6.rs`), so E6 itself is accurate.

**Sample A' (corrected run, rule P', seed 7370)**, each file read with its primary ADR (the header comment of the file and the ADR text):

| file | primary (evidence) | reading |
|---|---|---|
| `rcfd-mesh/src/gen.rs` | 0047 (E2) | disagree: the F-012 generators are older than wave 26b; the ADR only cites the file |
| `rcfd-fem/tests/sb_fan_blade_builder.rs` | 0505 (E0) | agree (F-593 registration) |
| `rcfd-fv/tests/natural_convection.rs` | 0029 (E2) | agree (wave 19a coupled solves, F-050; the ADR lists the file) |
| `rcfd-api/src/rheology.rs` | 0212 (E2) | disagree: F-110 code; 0212 is F-296 and only names it |
| `rcfd-mcp/tests/project_w48f.rs` | 0135 (E0) | agree |
| `rcfd-fv/tests/mixture_drift.rs` | 0056 (E0) | agree |
| `rcfd-api/src/design/cmaes.rs` | 0222 (E0) | agree (F-260 registration) |
| `rcfd-fem/tests/sa_gyro_sector_schur.rs` | 0492 (E0) | agree |
| `rcfd-api/tests/postprocessing.rs` | 0021 (E0) | agree |
| `rcfd-fem/tests/sa_cms_residual_default_bits.rs` | 0474 (E0) | agree |
| `rcfd-fv/tests/immersed_start_projection_f515.rs` | 0425 (E0) | agree |
| `rcfd-fv/tests/remesh_f247.rs` | 0215 (E0) | agree |
| `rcfd-fv/tests/overset_motion_f442.rs` | 0359 (E0) | agree |
| `rcfd-fem/tests/sa_visco_default_bits.rs` | 0485 (E0) | agree |
| `rcfd-api/tests/f295_seam_diffusivity.rs` | 0205 (E0) | agree |
| `rcfd-api/tests/compressible_bc_case.rs` | 0006 (E0) | agree |
| `rcfd-studio/tests/followups_n.rs` | 0148 (E0) | agree |
| `rcfd-particles/tests/film_gas.rs` | 0206 (E0) | agree |
| `rcfd-fv/tests/transient_buoyancy.rs` | 0352 (E2) | disagree: the file belongs to F-050 (transient gaps); 0352 (F-443) lists it among files run |
| `rcfd-fv/src/radiation.rs` | 0012 (E0) | agree |

Sample B' (the whole pool of 4): `rcfd-fv/tests/guarded_secant.rs` 0055 (E7) agree (its header names `docs/adr/0055-wave29a-...` G2);
`rcfd-api/tests/library_api.rs` 0014 (E7) disagree (the ADR lists suites that passed; the file is F-082); `physics_service.rs` 0144 (E7)
disagree (a list of tests run); `transient_energy.rs` 0160 (E7) disagree (a list of hashes). So E7 gives ADRs that name a binary, not its
registering ADR.

Deviations, all disclosed:

1. **P2 was missed on the first run and rule P was amended** (Amendment 1, committed before the corrected run). Sample A' met the
   unchanged limit of 17 only at its edge (17 of 20); with a binomial spread of about two on 20 the true rate is between 75 % and 95 %.
2. **A1 was missed by the amendment.** E0 adds the header ADR to `adrs` of many tests; the share of tests that gained any ADR is
   15.8 % against 12 %. No test lost an ADR. The limit is kept and the number recorded. The extra ADRs are the ADRs the files name
   themselves, so the cost is more candidates per failure in `gate_attribute.py`, not wrong ones.
3. **P3' could not be met** because the E6 or E7 pool is 4 files: E0 takes the wave files first (their headers name the wave ADR).
   The registered "12 of 15" is not reachable on 4 files and is recorded as missed; E6 itself was judged on sample B of the first run (14 of 15).
4. The report line of the first run printed the whole `E7 too common` list (335 binaries, a count that includes binaries other evidence
   maps); it was changed to print the count only. No number of a check changed.
5. The `docs/` change is this ADR; `docs/test-process.md` is not changed (its section 7 does not describe the new fields).

**Decision by the registered rule: partial.** W1, W2, U1', P1', A2', P2', C1, C2 and C3 are met and all seven mutations are caught,
but A1' (15.8 % against 12 %) and P3' (1 of 4) are missed, and the first-run P2 miss is on record.

Remaining questions, each with options and a recommendation:

1. **Primary from E7?** Options: (a) keep E7 as the tier before E5 (now); (b) take E7 out of the primary tiers and keep it as plain
   evidence (membership only). Recommendation (b): 3 of 4 E7 primaries were wrong and only 7 tests have E7 as primary, so (b) costs
   nothing in coverage and gives "no primary" instead of a wrong one.
2. **Primary for E2-only source files** (all 3 disagreements of sample A' are E2 on an old file or on a list of files run). Options:
   (a) leave; (b) take the earliest ADR that names the file by git order; (c) no primary without E0, E1 or E6. Recommendation (b), as a
   follow-up with its own registered rule.
3. **The 12 % limit of A1.** Options: keep the miss on record (done) or accept 16 % as the measured cost of E0. Recommendation: keep it
   on record; the limit was an estimate made before the headers were read.
4. **The 25 binaries still unmapped** have no header ADR, no item and no tag. Options: write the ADR text that names them, or add a
   header comment to each file (a Rust edit, not allowed here). Recommendation: a follow-up that writes the ADR text; the index does not guess.

## For the lead: roadmap entries

Proposed status and note of F-737:

> Proposed status: partial.

> Test-to-ADR index, wave tags and a primary ADR (follow-up of F-723, ADR 0653). PARTIAL. ADR 0679, 2026-11-17: the index maps wave-named test files to the ADR named after their wave by an exact tag rule (`w34a` and `wave17a` to the ADRs `wave34a` and `wave17a`; a bare tag with several ADRs, `w31`, is listed as ambiguous and not mapped; 25 tags have no wave ADR), reads the first `ADR NNNN` of a test file's own header comment as evidence, and gives each test a `primary_adr` with its evidence (additive fields; `gate_attribute.py` and `gate.yml` unchanged and passing). Integration binaries unmapped with tests fell from 79 to 25 (limit 65); all tests mapped 93.7 % to 98.2 %; no test lost an ADR. Sample check by reading: the first rule (without the header tier) gave 11 of 20 and missed 17; the amended rule (Amendment 1) gave 17 of 20 on a fresh draw (limit 17). Missed: the share of tests that gained an ADR is 15.8 % (limit 12 %), and the evidence "binary named in an ADR" is not a registering ADR (1 of 4). 7 of 7 mutations caught.

> Follow-up proposed: Primary ADR only from registering evidence in the test-to-ADR index. Deps F-737. Acceptance: Every limit not stated here is registered in the ADR before the run. The primary ADR is taken from the header ADR, the item's registration, the wave ADR and a file citation only, never from a binary named in an ADR's text or a list of tests run; the 7 tests whose primary was E7 and the E2-only source files named in ADR 0679 sample A' (`gen.rs`, `rheology.rs`, `transient_buoyancy.rs`) get the earliest ADR that names the file, or none, by a registered rule; a registered sample of at least 30 primary ADRs is read with at least 90 % agreement; the attribution JSON of `gate_attribute.py` is unchanged for the existing consumers.

> Follow-up proposed: ADR text for the 25 integration-test binaries still unmapped in the test-to-ADR index. Deps F-737. Acceptance: Every limit not stated here is registered in the ADR before the run. Each of the 25 binaries listed in ADR 0679 addendum 1 is named in the ADR that registered or first described it, found by reading the code and the git history, with no Rust file edited; the index shows at most 5 unmapped integration binaries with tests; a registered sample of the new mappings is read.
