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
