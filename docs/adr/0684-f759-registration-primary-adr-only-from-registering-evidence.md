# ADR 0684: F-759 registration: a primary ADR only from registering evidence in the test-to-ADR index

Status: **REGISTERED 2026-11-17, written before any code of F-759 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no code of F-759 was written; the numbers below were read from a run of the unchanged
`scripts/adr_test_index.py` at fa7fc82e (disclosed in "What was read"). Results go in the addenda at the end; nothing above the
addenda is changed after a run.

Roadmap item F-759 (deps F-737), proposed by [ADR 0679](0679-f737-registration-wave-tag-mapping-and-a-primary-adr-in-the-test-to-adr-index.md)
addendum 1 (remaining questions 1 and 2: three of the four primaries from "a binary named in an ADR" were wrong, and all three
disagreements of its sample A' were file citations on an old file or on a list of files run). Number 0684 given by the lead.
Branch `f759-work` from fa7fc82e, worktree `w49i-work/f759`, scratch `w49i-work/f759-scratch`.

House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here; each registered run made once; existing tests not edited; no Rust
file, hash table or golden touched; stdlib Python only; clean-room (nothing outside the repository is used); the roadmap files are
not edited. Choices the maintainer would normally be asked about are taken on the maintainer's standing instruction of 2026-11-11
(take the recommendation), except where listed as open.

## What F-759 asks

> Every limit not stated here is registered in the ADR before the run. The primary ADR is taken from the header ADR, the item's registration, the wave ADR and a file citation only, never from a binary named in an ADR's text or a list of tests run; the 7 tests whose primary was E7 and the E2-only source files named in ADR 0679 sample A' (the files `gen`, `rheology` and `transient_buoyancy`) get the earliest ADR that names the file, or none, by a registered rule; a registered sample of at least 30 primary ADRs is read with at least 90 % agreement; the attribution JSON of `gate_attribute.py` is unchanged for the existing consumers.

## What was read

- `scripts/adr_test_index.py` (rule P of ADR 0679: tiers E0 > E1 > E6 > E2 > E3 > E7 > E5 > E4; inside a tier an ADR file named
  `registration` or `-wave` first, then the lowest number; `NOT_EVIDENCE = {0653, 0679}` is applied to E0 and E7 only, not to E2),
  `scripts/gate_attribute.py` (`adr_cell`, the `primary_adr` field of the attribution record: a missing primary prints as the plain
  ADR list), `scripts/test_adr_test_index_wave.py`, `.github/workflows/gate.yml` (the index and the attribution run in one job whose
  `actions/checkout` has `fetch-depth: 0`, line 101).
- A run of the unchanged index at fa7fc82e (`cargo metadata` saved; 57 s, no cargo build): 7733 tests, 7597 with an ADR; primary by
  evidence E0 7141, E2 424, E4 17, E5 14, E3 1, E7 0. Distinct files with a primary: 1276 (E0 1185, E2 83, E5 4, E4 3, E3 1).
  Tests with an E7 evidence entry: 2637, all with a primary from E0 or E2. Nothing else was computed before this text.
- **A defect found in that read.** ADR 0679 itself names, with their `.rs` file names, the files of its samples, so the index reads
  it as E2 evidence: `guarded_secant`, `library_api`, `physics_service` and `transient_energy` of the sample B' list have primary
  ADR 0679 (E2) in this tree, where at c2ec31e4 they had E7. The 7 E7 tests of ADR 0679 are therefore hidden behind a citation
  by the ADR that reported them. The rule below excludes the ADRs that list files as samples or as listings from E2 evidence.

## Design

**Rule R (primary from registering evidence).** `primary_adr` is the first tier of **E0 > E1 > E6 > E2** that has an ADR; the
evidence kinds E3 (a function name), E4 (a roadmap note), E5 (a module path) and E7 (a binary named in an ADR's text) are never
primary. They stay evidence: `tests[].adrs`, `items`, `evidence` and `adrs[n].tests` are bit for bit what they were; only
`primary_adr`, `primary_evidence` and `adrs[n].primary_tests` change. A test with an ADR but no E0, E1, E6 or E2 evidence has
`primary_adr = null` (the gate prints its ADR list as it does for any test without a primary).

- E0, E1, E6: unchanged (ADR 0679 rule P': the registration or wave ADR first inside the tier, then the lowest number).
- **E2, the earliest ADR that names the file.** A file citation is a file name in the ADR text (`.rs` token, existing rule). The
  primary of an E2-only test is the **lowest ADR number** among the ADRs that cite its file, leaving out the listing ADRs
  `LISTING_ADRS = {0653, 0679, 0684}` (ADR 0653 lists test binaries, ADR 0679 lists its sample files, this ADR lists its sample
  files). Inside the E2 tier the registration preference is dropped: the earliest wins.
- **Why the number and not git.** An ADR number is given by the lead in the order the ADRs are made; git order of the first commit of
  the ADR file is the other candidate. The number is chosen because the rule then needs no history at gate time (the gate's
  checkout has `fetch-depth: 0` today, `gate.yml` line 101, but a shallow clone or a copied archive, as used by agents, would change
  the answer) and is the same on every machine. Whether the number order is the git order is checked, once, as R4 below; git is
  an audit, not an input.
- The default of the library call `build()` keeps rule P of ADR 0679 (so the existing tests of the two test files, which assert the
  old primary of an E7 file, stay as they are); the command line takes `--primary registered` (default) or `--primary p0679`.
  `gate.yml` and `gate_attribute.py` are not edited; the gate's call has no flag and gets rule R.

**Reading by sample.** A file *agrees* when the primary ADR is the ADR that registered the file's item or first described the file:
the file's header comment and the ADR title and first paragraph are read; for an E2 file also the first git commit that adds the file
(`git log --diff-filter=A --follow -- file`) is read for the ADR number or item in its subject. A file whose primary is `null` is not
sampled. A primary that is a later ADR that only lists, extends or runs the file *disagrees*. Each read is recorded with a reason.

## Checks (limits fixed now)

| id | check | limit | estimate behind it |
|---|---|---|---|
| R1 | no test has `primary_evidence` in E3, E4, E5, E7; every primary is in the test's `adrs`; a primary is E0, E1, E6 or E2 | 0 violations | by construction |
| R2 | for every E2 primary: it equals the lowest number among the ADRs with an E2 evidence entry of that test, outside LISTING_ADRS (recomputed from the index, not from the code path) | 0 violations | by construction |
| R3 | the tests that had E7 in the old rule: the ones whose old primary at this tree with the listing ADRs left out of E2 is E7 (found by running rule P with LISTING_ADRS removed) , and the three files named in ADR 0679 sample A' (the files `gen`, `rheology` of rcfd-api src and `transient_buoyancy`): none has a primary from E3, E4, E5, E7; each has either the lowest-number E2 ADR or `null`; every one listed with its new primary | 0 violations; the list is recorded | by construction |
| R4 | git audit of the number rule: for the files with primary E2, the lowest number among the citing ADRs equals the citing ADR whose file was first added to git (`git log --diff-filter=A`, oldest commit, ties by number) | at least 90 % of the E2 files (83 in this tree) | ADRs are numbered in making order; reordering by the lead should be rare |
| R5 | attribution unchanged: building the index of this tree with `--primary p0679` and with `--primary registered`: every test has equal `adrs`, `items`, `evidence`, `file`, `name`, `wave`; `adrs[n].tests`, `items`, `binaries`, `crates` and every `coverage` entry except `primary` equal; `python -m unittest scripts/test_gate_attribute.py scripts/test_gate_hooks_f761.py scripts/test_adr_test_index.py scripts/test_adr_test_index_wave.py` pass with no edit to them; `gate_attribute.py` and `gate.yml` byte-identical to fa7fc82e | 0 differences, all pass | by construction |
| R6 | coverage cost: tests with an ADR and no primary | at most 100 (all tests with an ADR 7597 in the baseline; E3, E4, E5 primaries 32 and the E7 and E2-of-listing cases) | 32 read now; 60 more allowed for the E2 of the listing ADRs |
| R7 | **sample C**: 40 distinct test files drawn with `random.Random(759).sample` from the sorted list of distinct files with a primary under rule R (the whole pool, uniform); each read as defined above | at least **36 of 40 agree** (90 %, the acceptance) | E0 is 93 % of the pool and was 95 % right in ADR 0679's sample A'; the E2 share (6 %) is where the misses are |
| R8 | **sample D**: the E2 stratum: all files with primary E2 if there are at most 20, else 20 drawn with `random.Random(7590).sample` from their sorted list; plus the 3 named files (counted apart, listed, not in the limit) | at least **16 of 20 agree** (80 %); this is a weaker limit than the acceptance and is registered as such: the 90 % of the acceptance is carried by sample C, D shows what the E2 rule gives where it is the only evidence | the sample A' E2 files were 1 of 4 right under the registration-first rule; the earliest-number rule should do better but is unmeasured |
| R9 | `python -m unittest discover -s scripts` from the clean worktree; `docs_site` | pass (ignoring the 4 skipped E2E tests); 2 passed | |

The samples are drawn once, from the index built after the code is final. Overlap with ADR 0679's samples A (seed 737) and A'
(seed 7370) is counted and disclosed, not removed.

## Negative controls and planned mutations

Tests in a new file `scripts/test_adr_test_index_primary.py` on a fake workspace (the style of `test_adr_test_index_wave.py`):
- N1 a test with only an E7 ADR: `adrs` has it, `primary_adr` is `null`; with `primary_rule="p0679"` the primary is that ADR.
- N2 a test with only E3, only E4, only E5 evidence: in `adrs`, no primary.
- N3 a test file cited by ADRs 0120, 0130 and 0140, where 0140 is a `registration` file: the primary is 0120.
- N4 a test file cited by a listing ADR (0653) and 0150: primary 0150; cited by 0653 only: `null`.
- N5 E0, E1, E6 each still beat E2 (an ADR with a lower number cites the file).
- N6 the index of one fake workspace under both rules: `adrs`, `items`, `evidence` and `adrs[n].tests` equal, `primary_tests` differ.
- N7 `main` with no flag uses rule R; `--primary p0679` the old rule.
- N8 a test with a primary of `null` keeps its row in the `gate_attribute` table as the plain ADR list (existing A2 test covers the
  cell; a fixture record is added).

Mutations on a scratch copy of `scripts/`: M1 E7 allowed again in the primary tiers (caught by N1), M2 E2 picks the highest number
(N3), M3 E2 keeps the registration preference (N3), M4 LISTING_ADRS not applied (N4), M5 E3 allowed in the primary tiers (N2), M6
the primary filter removes the E7 evidence from `adrs` (N6), M7 the CLI default flipped to p0679 (N7). Each is caught or recorded as
not caught.

## Decision rule

**done** if R1 to R9 are all met and all mutations are caught; **partial** if any limit is missed (the number is recorded and
the limit stays), in particular if R7 or R8 is missed; **todo** if R1, R2 or R5 fails (a wrong or changed result). A missed R8 with
R7 met is partial with the E2 rule reported as the weak part.

## Limits (what is not covered)

- Rule R gives no primary to a test whose only ADR evidence is a function name, a roadmap note, a module path or a binary name; it
  does not guess a better ADR. The coverage of `primary_adr` falls by the number recorded in R6.
- The lowest ADR number is the earliest by the lead's numbering, not by authorship: a file first described in a late ADR that
  was given a low number (a numbering reuse) would be misread; R4 measures this.
- An early ADR that merely names a file, before the ADR that registered it, is still taken (the rule asks for the earliest that
  names it); the reading in R8 shows how often.
- Header ADR (E0) is taken as the file's own statement and is not checked against git beyond the sample.
- The listing ADRs are a fixed list; a future ADR that lists files must be added to it (this is the reason the list is a named
  constant with a test).
- Wave tags, E1 item mapping and the 25 unmapped binaries of ADR 0679 are not touched.

## Addendum 1: results (2026-11-17)

Code: `scripts/adr_test_index.py` (`REGISTERED_TIERS`, `LISTING_ADRS`, `primary_adr(..., rule)`, `build(..., primary_rule)`, command-line
`--primary registered|p0679`, default `registered`), new tests `scripts/test_adr_test_index_primary.py` (9 tests, N1 to N8). The
existing tests, `gate_attribute.py` and `gate.yml` are not edited (byte-identical to fa7fc82e). No Rust file, golden or hash table
touched. Inputs and logs in `w49i-work/f759-scratch` (`meta.json`, `idx_p0679.json` and `idx_reg.json` the two index runs of this
tree, `measure.py` and `measure.txt`, `samples.json`, `show.py`, `mutate.py`, `mutations.log`). Each index run was made once
(one per rule, on the committed code); each sample was drawn once.

| id | limit | measured | met |
|---|---|---|---|
| R1 | 0 violations (no E3, E4, E5, E7 primary; primary in `adrs`) | 0 violations in 7733 tests; primary evidence E0 7141, E2 413 | yes |
| R2 | E2 primary is the lowest non-listing number, recomputed from the index | 0 violations | yes |
| R3 | tests with an E7 old primary and the three named files: no E3, E4, E5, E7 primary, each the earliest E2 ADR or null | with the listing ADRs left out of E2 the old rule gives E7 to 6 tests (`guarded_secant` 2, `library_api` 2, `transient_energy` 2); the 7th of ADR 0679, `physics_service.rs`, now has ADR 0679 as its only ADR. All 7 have `null`, their `adrs` unchanged (0055, 0014, 0160, 0679). The three named files keep their primary: `gen.rs` **0047** (the lowest of 23 citing ADRs), `rheology.rs` (rcfd-api src) **0212**, `transient_buoyancy.rs` **0352**: the earliest ADR that names the file is the one the old rule gave | yes |
| R4 | git order equals number order for at least 90 % of the E2 files | **78 of 78** (0 disagreements) | yes |
| R5 | attribution unchanged | 0 differences between the two index runs in every test field except `primary_adr` and `primary_evidence` (7733 tests), in `adrs[n].files/items/tests`, `binaries`, `crates`, `items` and `coverage` except `primary`; `python -m unittest discover -s scripts`: 102 run, OK, 4 skipped (the E2E tests behind `GATE_TEST_E2E`), existing files unedited; `gate_attribute.py` and `gate.yml` unchanged | yes |
| R6 | tests with an ADR and no primary at most 100 | **43** (E4 17, E5 14, E3 1, E2 of a listing ADR only 11); 7554 of 7597 mapped tests keep a primary | yes |
| R7 | sample C, 40 files (seed 759), at least 36 agree | **39 of 40 agree** (97.5 %); 38 are E0, 2 are E2 | yes |
| R8 | sample D, the E2 stratum, 20 of 78 files (seed 7590), at least 16 agree | **3 of 20 agree** | **no** |
| R9 | unit tests and `docs_site` | 102 run, OK; `docs_site` 2 passed (`cargo test -q --release -p rcfd-cli --test docs_site`, run on the final tree) | yes |

Mutations (`mutations.log`, on `scripts/adr_test_index.py`, restored after each): M1 E7 allowed (N1, N4, N7), M2 highest number in E2
(N3, N6), M3 registration preference in E2 (N3, N6), M4 listing ADRs not applied (N4), M5 E3 allowed (N2), M6 the E7 evidence removed
from `adrs` (N1, N6), M7 CLI default p0679 (N7): **7 of 7 caught**.

**Sample C (seed 759)**, 40 of 1263 distinct files with a primary, read as defined above (the header comment, the ADR title and the
first commit that adds the file). The 38 E0 files all agree: the primary is the ADR named in the file's own header and the first
commit names the same ADR or item (`wave_drive_f478` 0378, `particles_two_way` 0059, `si_nonlinear_rollback` 0406, `sa_emc_default_bits`
0514, `f237_heat_deck` 0244, `w36b_lag_known` 0081, `si_buckling_sign_asymmetric_f642` 0559, `duct_modes` 0351, `preview_g1` 0138,
`cylindrical_interface_w43b` 0109, `live_preview_gpu_j` 0144, `immersed_solid_f219` 0218, `seam_flux_w33a` 0068, `data_service_f447`
0350 (an amendment of the Electron ADR registers F-447), `sa_contact_default_bits` 0507, `nsga2` 0209, `project_runs_w48f` 0135,
`runner_wfix` 0128, `rng` 0067 (moved by ADR 0118, written under 0067), `mrf_cyclic_seams_w44c` 0113, `sa_cms_enrich_default_bits`
0532, `f295_seam_diffusivity` 0205, `w48z_followups` 0160, `si_static_refinement_defaults` 0397, `journal_f273` 0188,
`f286_transient_sst` 0199, `seam_no_harm_f344` 0245 (an amendment of F-344's registration), `sa_f665` 0589, `sa_sparse_chol` 0447,
`f697_fan_objective` 0620, `sa_contact_supernodal_f615` 0533, `sa_laminate_body_default_bits` 0502,
`sa_ortho_cyclic_prestress_default_bits` 0465, `si_shell_thermal_gradient_shell3_layup` 0626, `sa_j2_default_bits` 0475,
`cosim_serve_f444` 0380, `sa_random_default_bits` 0477, `overset_f356` 0260). The two E2 files: `fan_cgrid_case.rs` 0068 agree (its
header names `docs/adr/0068-wave33a-...` and the first commit is "Wave 33A"); `rcfd-api/src/service.rs` 0037 **disagree** (a file of
the initial import; ADR 0037 only lists it among the files it edits, "one method").

**Sample D (E2 stratum, seed 7590)**, 20 of 78 files, with the reason of each:

| file | primary | reading |
|---|---|---|
| `rcfd-linalg/src/matrix.rs` | 0136 | disagree: initial import; the fidelity-tier ADR only names it |
| `rcfd-fv/tests/natural_convection.rs` | 0029 | agree: F-050 cavity; ADR 0029 sets the checks the file runs (named in A' as well) |
| `rcfd-fv/src/compressible.rs` | 0005 | disagree: initial import; ADR 0005 names it to say it cannot do natural convection |
| `rcfd-post/src/monitors.rs` | 0160 | disagree: F-074 file; 0160 is a follow-up job |
| `rcfd-fv/src/convection.rs` | 0016 | disagree: initial import; 0016 adds schemes to it |
| `rcfd-core/src/scalar.rs` | 0136 | disagree: initial import |
| `rcfd-io/src/container.rs` | 0350 | disagree: F-015 file named by the Electron spike ADR |
| `rcfd-fv/src/sources.rs` | 0085 | disagree: F-031 code; 0085 is a remedy of a rotor-passage run |
| `rcfd-linalg/src/vecops.rs` | 0136 | disagree: initial import |
| `rcfd-fv/src/geometry.rs` | 0205 | disagree: initial import; 0205 edits it for seam pairs |
| `rcfd-api/tests/interpolate.rs` | 0099 | agree: ADR 0099 has a section on this file (F-115 close-out) |
| `rcfd-claude/src/discover.rs` | 0126 | agree: the first commit names ADR 0126 |
| `rcfd-fv/tests/transient_buoyancy.rs` | 0352 | disagree: F-050 file (named in A'); 0352 lists it |
| `rcfd-project/src/time.rs` | 0128 | disagree: written for ADR 0122's project folder; 0128 is an audit fix |
| `rcfd-fv/src/pimple.rs` | 0005 | disagree: initial import |
| `rcfd-io/tests/gmsh_samples.rs` | 0173 | disagree: initial import; 0173 is a workspace-layer ADR |
| `rcfd-turb/tests/channel_sst.rs` | 0014 | disagree: F-043 file; 0014 is the SA ADR |
| `rcfd-api/src/stats.rs` | 0136 | disagree: F-116 file; the fidelity ADR names it |
| `rcfd-api/tests/statistics.rs` | 0166 | disagree: F-116 file; 0166 changes defaults |
| `rcfd-api/src/rheology.rs` | 0212 | disagree: F-110 code (named in A'); 0212 is F-296 |

The named file `gen.rs` (0047, lowest of 23 citing ADRs): **disagree** (the F-012 generators are in the initial import; 0047 adds
the hexahedral meshing next to them). Overlap with ADR 0679's samples: sample D holds three files of sample A' (`natural_convection`,
`rheology`, `transient_buoyancy`); sample C holds none of A'; overlap with sample A (seed 737) was not checked.

**What the samples say.** On E0 (94 % of the files) the primary is right in 38 of 38 files read. On E2-only files the earliest ADR
that names the file is the registering ADR in 4 of 22 files read (3 of 20 in D, 1 of 2 in C; A' read 1 of 4 on the other rule). The
number rule is as good as the git rule (R4: 78 of 78 equal), so the weak part is the evidence, not the order: more than half of the
E2 files are from the initial import (no registering ADR exists), and the earliest ADR that names such a file edits or mentions it.

Deviations, all disclosed:

1. **R8 missed (3 of 20, limit 16).** The limit is kept and the number recorded; it is diagnosed, not tuned: no amended E2 rule was
   run on the same sample.
2. The 7 E7 tests of ADR 0679 are 6 tests in this tree plus `physics_service.rs` (see R3): ADRs 0679 and 0684 cite the files of
   their samples and so hid the old E7 primaries behind their own file citations. `LISTING_ADRS` is the fix; it is a fixed list
   (Limits).
3. The three named files keep their old primary by the registered rule. The acceptance says they "get the earliest ADR that names
   the file, or none"; the earliest is the same ADR, which the reading judges wrong for `gen.rs` and `rheology.rs` (and
   `transient_buoyancy.rs`).
4. Sample D contains `transient_buoyancy.rs` and `rheology.rs` by the draw as well as by the named list; each is counted once.
5. Git history was used only as audit (R4, the reading); `gate.yml` `fetch-depth: 0` (line 101) is therefore not needed by rule R.

**Decision by the registered rule: partial.** R1 to R7 and R9 are met and 7 of 7 mutations are caught, so the acceptance clause
"a registered sample of at least 30 primary ADRs read with at least 90 % agreement" is met by sample C (39 of 40), the primary is
never taken from a binary named in an ADR's text, a list of tests run, a function, a roadmap note or a module path, and the attribution
data are unchanged; but R8 is missed: where the file citation is the only evidence, the earliest ADR that names the file is usually
not the one that registered it. The "earliest ADR that names the file" clause is implemented as registered; it does not make those
primaries right.

Remaining questions, each with options and a recommendation:

1. **Primary for E2-only files.** Options: (a) keep the earliest citing ADR (now; 4 of 22 right); (b) a primary from E2 only when
   the first git commit that adds the file names an ADR or an item that has a registration ADR, else `null`; (c) no primary from E2
   at all. Recommendation (b), as a follow-up with its own registered sample: the add commit usually names the item (`sources.rs`
   F-031, `discover.rs` ADR 0126, `stats.rs` F-116, `channel_sst.rs` F-043 in sample D), and the initial-import files would get
   `null` instead of a wrong ADR; it needs history at gate time (`gate.yml` line 101 has `fetch-depth: 0`) and a fall-back to `null`
   where the history is shallow. Not measured here.
2. **Listing ADRs.** `LISTING_ADRS` is a fixed list (0653, 0679, 0684). Options: (a) keep the list and add a number when an ADR lists
   files; (b) exclude every ADR that cites more than N files from E2. Recommendation (a) until question 1 is decided; (b) needs a
   registered N.
3. **Library default.** `build()` keeps rule P of ADR 0679 so that the existing tests (one asserts an E7 primary) stay unedited; only
   the command line, and so the gate, uses rule R. Options: (a) keep; (b) flip the library default and edit that assertion.
   Recommendation (a) until the maintainer wants those tests re-recorded.

## For the lead: roadmap entries

F-759 proposed status: **partial**.

> Primary ADR only from registering evidence in the test-to-ADR index. Follow-up of F-737 (ADR 0679, 2026-11-17). PARTIAL. ADR 0684, 2026-11-17: `scripts/adr_test_index.py` takes the primary ADR only from the header ADR, the item's registration, the wave ADR and a file citation (default of the command line, `--primary p0679` keeps the old rule); evidence from a function name, a roadmap note, a module path or a binary named in an ADR's text stays in the test's ADR list but gives no primary (43 of 7597 mapped tests have none), and the file-citation primary is the earliest citing ADR (lowest number; ADRs 0653, 0679 and 0684, which list files, left out); the old E7 primaries are null, and the attribution data of `gate_attribute.py` is unchanged (0 differences in 7733 tests, the existing tests unedited, git order equals number order for 78 of 78 files). Sample by reading: 39 of 40 uniform files agree (limit 36); missed: the file-citation-only stratum, 3 of 20 (limit 16), because most of those files come from the initial import and the earliest ADR that names one only edits or mentions it; the three files of ADR 0679 sample A' keep their primary (it is already the earliest) and are still not the registering ADR. 7 of 7 mutations caught.

> Follow-up proposed: Primary ADR of file-citation-only tests from the commit that adds the file. Deps F-759. Acceptance: Every limit not stated here is registered in the ADR before the run. A test whose only primary evidence is a file citation takes its primary from the first git commit that adds the file (an `ADR NNNN` in its subject, or an item `F-NNN` whose registration ADR exists), else no primary, by a rule registered before the run, with the history depth the gate has stated and a fall-back to no primary where the history is shallow; the 20 files of ADR 0684 sample D (seed 7590) and a fresh registered sample of at least 20 file-citation-only files are read with at least 80 % agreement; the attribution data of `gate_attribute.py` is unchanged for the existing consumers.
