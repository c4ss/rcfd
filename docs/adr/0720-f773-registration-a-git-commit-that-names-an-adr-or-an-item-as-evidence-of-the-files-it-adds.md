# ADR 0720: F-773 registration: the ADR or item named by the commit that adds a test file as evidence of kind E8

Status: **REGISTERED 2026-11-17, written before any code of F-773 and before any run of its checks.** Not yet accepted by the
maintainer. Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-773 (deps F-768), proposed by [ADR 0690](0690-f768-registration-primary-adr-from-the-first-commit-that-adds-a-cited-file.md)
addendum 1 (a commit that names an ADR which does not cite the file's name, `time.rs` and ADR 0122, was dropped by the coherence
step). Number 0720 given by the lead. Branch `f773-work` from 49e172f6 (ADRs 0690, 0701, 0703, 0712 and 0714 merged), worktree
`w49i-work/f773`, scratch `w49i-work/f773-scratch`.

House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here; each registered run made once; existing tests not edited; no Rust
file, hash table or golden touched; stdlib Python only; clean-room; the roadmap files are not edited. Choices the maintainer would
normally be asked about are taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except where
listed as open.

## What F-773 asks

> Every limit not stated here is registered in the ADR before the run. The ADR named in the subject of the commit that adds a test file, and the ADR of an item named there that has a registration ADR, are evidence of kind E8 of the file's tests (`adrs`, `items` and `evidence` extended, so the primary rule of ADR 0690 needs no coherence step); the added evidence is counted before the run, the index of the final tree is compared with that of ADR 0690 in every field except the added `adrs`, `items`, `evidence` and the primary, a shallow clone gets no E8 evidence, and a fresh registered sample of at least 20 files with an E8 primary is read with at least 80 % agreement.

Out of scope, by the brief: what the primary of an unregistered file should be is the maintainer's question (F-787, F-793). Rules H0,
H1 and H2 of ADR 0701 and the retro counts are not changed; the tier order is changed only by adding E8, and where E8 sits is
registered below.

## What was read, and what was done beforehand (disclosed)

- `scripts/adr_test_index.py` at 49e172f6 (`git_history`, `commit_adr`, `registration_adrs`, `build()` with the rule `git`, the
  coherence step, `retro_primaries`), the tests `scripts/test_adr_test_index_git.py`, `..._retro.py`, `..._h3.py`, ADR 0690 (design,
  checks G1 to G11, addendum 1), ADR 0704 (the second-reader criterion).
- The index at 49e172f6 with the unchanged script, saved as `f773-scratch/idx_base.json`: **34.7 s** (saved metadata, 2 git
  processes); `coverage.primary`: 7473 tests with a primary, `by_evidence` E0 7320, E2 153; git `e2_tests` 473, `from_git` 3,
  `no_commit_rule` 468, `not_in_adrs` 2; retro h1 72, h2 78, none 320. The repository has 4676 commits and is not shallow.
- **The added evidence, counted before the code** (`f773-scratch/count_e8.py`, `count2.py`: the origin commit of each test file by the
  code of `git_history`, its subject read with the rule below, nothing else): **5695 tests in 950 files** have at least one E8 ADR;
  **6089 (test, ADR) E8 links**; **271 links in 216 tests name an ADR the test did not already have in `adrs`**; at most 4 E8 ADRs
  for one file; no test without any ADR gains one (13 tests have no ADR, none has an E8 ADR). Of the 950 files, **948 have an E0
  primary, 1 has E2 (`rcfd-claude/src/discover.rs`, ADR 0126, 3 tests) and 1 has no primary (`rcfd-project/src/time.rs`, ADR 0122,
  2 tests)**; the first E8 ADR equals the E0 primary on 919 of the 948 E0 files (29 differ). Nothing else was computed.
  **Consequence: the population of files with an E8 primary under the order registered below is 2 files, 5 tests** (3 change the
  label from E2 to E8, 2 gain a primary). The acceptance's sample of at least 20 files cannot be drawn from it; see check E7.

## Design

**Rule E8.** In `scripts/adr_test_index.py`, `build(..., e8=...)` (library default `False`, so every existing call and test keeps
the behaviour of ADR 0690 byte for byte; command line: `--e8` / `--no-e8`, default on, effective only with `--primary git`, which is
the command-line default). With `e8` on and a `full` git history (ADR 0690: the workspace root is the top of a repository that is
not shallow), for each test whose file has an origin commit (ADR 0690: the first commit that adds the path, renames followed, one
batched `git log`):

1. **Which ADRs.** From the *subject* of the origin commit only (not the body, not later commits): every `ADRs? dddd` whose ADR
   exists (first mention of each, in order), then the registration ADR (`registration_adrs`, ADR 0690) of every `F-ddd` of the
   subject that has one, in order, without repeats. Each is one evidence entry `{"kind": "E8", "adr": n, "via": "subject" or
   "F-ddd", "commit": <first 10 characters of the hash>}`. No coherence condition: the ADR need not cite the file.
2. **Extended fields.** `adrs` of the test gains these ADRs (the existing computation takes every `adr` of the evidence), `items`
   gains the items of those ADRs by the existing mapping, `evidence` gains the entries. Nothing else of a test changes.
3. **Primary.** The tier order of the git rule is **E0 > E1 > E6 > E8 > E2** (E3, E4, E5, E7 stay evidence without primary, as in
   ADR 0684). E8 sits after E0, E1 and E6 because those are the file's own header, its own item number and its own wave tag, which
   say what the author registered; it sits before E2 because a commit that names the ADR when it adds the file is a statement made
   by the file's author at the time, where E2 is a later citation. The E8 primary is the **first** E8 entry (the subject's ADR
   before its item's registration ADR, as in ADR 0690). `primary_evidence` is `E8`. A test that has E8 evidence and no E0, E1 or E6
   evidence gets this primary whatever other evidence it has. Because the candidate is in `adrs` by construction, ADR 0690's
   coherence step is not run when `e8` is on (`not_in_adrs` stays 0).
4. **Rules G and H.** What ADR 0690 rule G did for an E2 test is now done by tier E8. An E2-primary test without E8 evidence has no
   primary (as before: `no_commit_rule` or `no_origin`) and goes to rules H1 and H2 of ADR 0701 unchanged (`retro_primaries`); rule
   H0 (a header path, E0) is unchanged and outranks E8.
5. **History depth.** No `full` history, no E8 evidence: a shallow clone, a git archive copy, a repository that is not the workspace
   root, or a failing git give none (checked against the ADR 0690 output, not by assertion).
6. **Cost.** The git calls are those of ADR 0690, at most 2 and none per file. `git_history` is called before the evidence loop
   instead of after it; the call is the same.
7. **Record.** `coverage.primary.git` keeps its keys (`from_git` counts the E8 primaries, `no_commit_rule` and `no_origin` the E2
   tests without one, `not_in_adrs` is 0); a new key `coverage.primary.e8`: `{"history", "tests", "files", "links", "links_new",
   "tests_new", "primary_tests", "primary_files"}`. With `e8` off none of this changes (no new key).

**Reading the E8-primary files.** Criterion of ADR 0704 adopted without change (the question: the ADR a maintainer should open
first when a test of the file fails; evidence (a) the file, (b) the text of each ADR that cites the file, (c) the git history;
verdicts agree or disagree with a reason; the five rules for several ADRs; borderline rows resolved to agree and counted). Two
differences from ADR 0704, fixed now: the commit subject is part of what is read here, because rule E8 uses it, so a verdict of
agree needs more than the subject: the ADR must also register or first describe the file in the sense of (b), or the file's own
header names it; and a file with an E8 primary is never "agree-null". **Who reads:** the implementing agent (the author of the rule;
disclosed). ADR 0704 found a first reader's agreement too high; no second reader is run here and a second reading of the files is
proposed as a follow-up (E7).

## Checks (limits fixed now)

| id | check | limit | estimate behind it |
|---|---|---|---|
| E1 | added evidence of the final tree, counted from the run with `--e8` against the figures counted beforehand: tests with E8, files with E8, links, links and tests that add an ADR not in `adrs` | exactly 5695, 950, 6089, 271, 216 | the count before the code, same origin function, same subjects; the new commits of this branch add no Rust file and no subject that names an ADR of an old file |
| E2 | the final tree indexed twice (code final): `--no-e8` (ADR 0690) and `--e8`; compared field by field: every test field except `adrs`, `items`, `evidence`, `primary_adr`, `primary_evidence`; `binaries`, `crates`, top-level `items`, `adrs[n].files`; `coverage` except `primary`; `wave` | 0 differences; the `adrs[n].items/tests/primary_tests` may differ and the differences are all explained by the E8 links | E8 changes nothing it does not name |
| E3 | `adrs` and `items` of every test are supersets of the ADR 0690 values; the added ADRs are exactly the E8 ADRs not already present; `evidence` of every test equals the ADR 0690 evidence plus its E8 entries | 0 violations in 7847 tests | by construction |
| E4 | primary: every test with an E0, E1 or E6 primary evidence under ADR 0690 keeps its primary and evidence; the E2 tests that have no E8 evidence keep their ADR 0690 primary (null, or the H1/H2 ADR); tests with `primary_evidence` E8 | other tests 0 differences; E8 primaries exactly **5 tests in 2 files** (`discover.rs` 0126, `time.rs` 0122); `with_primary` 7473 to 7475, by_evidence E8 5, E2 150, E0 7320; retro h1 72, h2 78, none 320 to 318 | the count before the code |
| E5 | **shallow clone** (`git clone --depth 1` of the committed final tree, file URL) and **git archive copy**, indexed with `--e8` and with `--no-e8` | history `shallow` / `none`; 0 E8 evidence entries; the two indexes equal in every field; E8 primaries 0 | by construction (an origin is only read on a full history) |
| E6 | git processes of one index run on the real tree, counted by wrapping `subprocess.run` in a one-off driver and by a test on the fake repository | at most 2, none per file (the limit of ADR 0690) | by construction |
| E7 | **sample of files with an E8 primary** read as above: the population of the final tree; if it has at least 20 files, a sample of 20 is drawn once with `random.Random(7730).sample` from the sorted list, else all of them are read | agreement (agree over files read) at least 80 %; **the acceptance's "at least 20 files" is not drawable: the population is 2 files by the count before the code, and the limit of 20 files is registered here as missed by construction** (status partial, whatever the readings give) | the 2 files named in E4; 0126 is cited by the commit and registers the session discovery; 0122 is the project folder library and `time.rs` is one of its modules, which ADR 0690 read as `disagree-null` because 0122 does not cite it by name |
| E8 | **supplementary reading, no part of the acceptance**: the files with an E8 candidate whose first E8 ADR differs from their E0 primary (29 at the base tree, the final tree's list is used): 20 drawn once with `random.Random(7730).sample` from the sorted list; each read as above for both ADRs. The question it answers: would E8 above E0 be better? | reported only; as a decision aid, E8 right (and E0 not) in at least 16 of 20 would support moving E8 above E0; fewer leaves the order as registered | no estimate; header ADRs are written by the author of the file, so I expect E0 right in most of them |
| E9 | run time of the index with `--e8` on the final tree, saved metadata, one run, against 34.7 s at the base | at most 44.7 s (before + 10 s, the limit of ADR 0690 G4) | the log pass is 0.5 s; the extra work is a regex per file |
| E10 | `python -m unittest discover -s scripts` from the worktree (existing tests unedited, 4 skipped E2E tests ignored) and `cargo test -q --release -p rcfd-cli --test docs_site` | pass; 2 passed | docs touched |
| E11 | the first E8 ADR equals the E0 primary (automatic, informational, no reading) | 919 of 948 files, as counted | the count before the code |

Mutations are registered below. The samples are drawn once, from the index built after the code is final.

## Negative controls and planned tests

Tests in a new file `scripts/test_adr_test_index_e8.py`, on the fake git repository of `test_adr_test_index_git.py` (imported, not
edited; a second repository where the new cases need it):
- T1 a file added by a commit naming `ADR 0140`, which does not cite the file: E8 evidence 0140 in `adrs` and `evidence`, primary
  0140 (ADR 0690 gave none, `not_in_adrs`), `primary_evidence` E8, `adrs` extended.
- T2 an item (`F-020`, registration ADR 0105) in the subject: E8 evidence 0105; both ADR and item: two entries, the ADR first,
  primary the ADR; `items` extended with the items of the added ADRs.
- T3 a later commit that names another ADR adds no E8 evidence; a renamed file keeps the origin commit's evidence; a file deleted and
  added again keeps its first.
- T4 the initial import (no ADR or item named), an ADR number that does not exist, and an item without a registration ADR: no E8.
- T5 the body of the commit is not read.
- T6 tiers: a test with E0 (header) evidence keeps its E0 primary though E8 evidence exists; E8 beats E2; an E3-only test with E8
  evidence gets the E8 primary (rule 3 of the design); an E2-only test without E8 evidence still goes to H1/H2 of ADR 0701.
- T7 `e8=False`, and `--no-e8`, are ADR 0690 exactly (the whole index equal to the `--primary git` index of the existing tests).
- T8 `--e8` differs from `--no-e8` only in `adrs`, `items`, `evidence`, `primary_adr`, `primary_evidence` of tests and in the
  `coverage.primary` and `adrs[n]` totals they feed.
- T9 shallow clone, copy without `.git`, enclosing repository: no E8 evidence.
- T10 at most 2 git processes.
- T11 the command line without a flag has E8; `--no-e8` and `--primary registered` have none.

Mutations on a scratch copy of `scripts/`: M1 E8 from the ADR of the subject only (the item dropped); M2 E8 tier after E2; M3 E8
tier before E0; M4 E8 only when the ADR already cites the file (the coherence step kept); M5 the newest add commit instead of the
oldest; M6 the commit body read as well as the subject; M7 the shallow check removed; M8 the command-line default off; M9 `e8` off
still adds the evidence; M10 an E8 entry for an ADR number that does not exist; M11 the E8 primary is the last entry instead of the
first. Each is caught or recorded as not caught.

## Decision rule

**done** if E1 to E11 are all met and all mutations are caught; **partial** if any limit is missed (the number is recorded and the
limit stays), which includes E7's "at least 20 files", missed by construction; **todo** if E2, E3, E4 or E5 fails (a wrong or
changed result).

## Limits (what is not covered)

- The tier order registered here puts E8 below E0, E1 and E6, so the population of E8 primaries is 2 files. Whether E8 should rank
  above E0 is the question E8 (supplementary) informs; the order is not changed after the run.
- The origin is the first commit by `git log --reverse`, subject only (the limits of ADR 0690 apply); a wave tag or an item in a
  body is not read; an `ADRs 0100 and 0101` subject gives 0100 only.
- A subject that names an ADR only in passing ("see ADR dddd") is E8 evidence like any other; the reading of E7 and E8 says how often
  that is a wrong primary.
- A first reader who is also the author of the rule; no second reader.
- `adrs[n].tests` grows by the 271 new links; consumers that list the tests of an ADR (`--adr`, the gate's per-merge check) now
  include them. This is the extension the acceptance asks for, and it changes what `--adr` prints for 216 tests.

## Addendum 1: results (2026-11-17)

Code: `scripts/adr_test_index.py` (`e8_adrs`, `E8_TIERS`, `build(..., e8=False)`, the tier E8 in `primary_adr`, the command-line
`--e8` / `--no-e8`, default on) in commit 4c7c588d, and the new tests `scripts/test_adr_test_index_e8.py` (11 tests, T1 to T11). No
existing file edited except `adr_test_index.py` (67 insertions, 10 deletions); `gate_attribute.py`, `gate.yml` and every existing test
file are byte-identical to 49e172f6. No Rust file, golden or hash table touched. Inputs and logs in `w49i-work/f773-scratch`
(`meta.json`, `idx_base.json`, `idx_final_off.json`, `idx_final_on.json`, `idx_shallow_*.json`, `idx_archive_*.json`, `compare.py`,
`compare.log`, `drive.py`, `mutate.py`, `mutations.log`, `samples_e8.json`, `gather.py`). The index code was final (4c7c588d) before
every run below; each index run was made once, on that commit. One deviation in the tests: my first version of T11 expected `null`
for `--primary registered` on `g_notcited`, but rule R gives the lowest citing ADR (0130); I corrected the expectation in the test
before the commit (no registered run was involved).

| id | limit | measured | met |
|---|---|---|---|
| E1 | 5695 tests, 950 files, 6089 links, 271 links, 216 tests | **5695, 950, 6089, 271, 216** (`coverage.primary.e8` and `compare.py`) | yes |
| E2 | `--no-e8` against `--e8` on 4c7c588d: 0 differences outside the extended fields | 0 differences in every other test field (7847 tests), `binaries`, `crates`, top-level `items`, `adrs[n].files` and the keys of `adrs`, `coverage` except `primary`; 41 of the ADR entries differ in `items`, `tests`, `primary_tests`, all from the E8 links | yes |
| E3 | supersets, added ADRs exactly the E8 ADRs, evidence equals old plus E8 | 0 violations in 7847 tests | yes |
| E4 | other tests 0 differences; E8 primaries 5 tests in 2 files; `with_primary` 7475, E8 5, E2 150, E0 7320; retro 72, 78, none 318 | primary changed on exactly 5 tests: `discover.rs` 0126 E2 to E8 (3), `time.rs` none to 0122 E8 (2); `with_primary` 7475; `by_evidence` E0 7320, E2 150, E8 5; retro h1 72, h2 78, none 318, the `files` map equal; `from_git` 5, `not_in_adrs` 0 | yes |
| E5 | shallow clone and git archive copy: 0 E8 entries, `--e8` equals `--no-e8` | `shallow` (depth-1 clone of 4c7c588d, `--is-shallow-repository` true) and `none`: 0 E8 entries, 0 E8 primaries, the `tests`, `adrs`, `binaries`, `crates`, `items` of the two runs equal, `coverage` equal but the new `e8` key (all zero); 1 git process | yes |
| E6 | at most 2 git processes, none per file | 2 on the real tree (counted by wrapping `subprocess.run`, `git_calls` 2), 1 on the clone and the copy; T10 on the fake repository | yes |
| E7 | files with an E8 primary read, at least 80 % agreement; at least 20 files | **2 files** (the whole population), **2 of 2 agree** (100 %); the sample of 20 files is not drawable | agreement yes, **20 files no (missed by construction, as registered)** |
| E8 | supplementary: E8 right and E0 not in at least 16 of 20 | **1 of 20** (see below) | no (decision aid: E8 stays below E0) |
| E9 | at most 44.7 s | 20.9 s with `--e8` (19.2 s with `--no-e8`; the base run of the registration, 34.7 s, was made at a busier moment of the shared machine, so the spread of the machine is larger than the difference) | yes |
| E10 | `python -m unittest discover -s scripts`; `docs_site` | 149 tests, OK, 4 skipped (138 before plus the 11 new); `docs_site` see the last line | yes |
| E11 | first E8 ADR equals the E0 primary on 919 of 948 files | 919 of 948 (29 differ) | yes |

**Mutations** (`mutations.log`, on a scratch copy of `scripts/`, run with `test_adr_test_index_e8.py` and `test_adr_test_index_git.py`):
11 of 11 caught. M1 item dropped (T2, T4); M2 tier after E2 (T1, T2, T6, T11); M3 tier before E0 (T6, T8); M4 coherence kept (T1 to
T4, T11); M5 newest add (T3, and T13 and T6b of ADR 0690); M6 body read (T4, T5 and five tests of ADR 0690); M7 shallow check removed
(T9, and T9 of ADR 0690); M8 command-line default off (T11); M9 `e8` off still adds (T1, T2, T6, T7, T11); M10 nonexistent ADR (T4);
M11 last entry as primary (T2, T6).

### E7: the two files with an E8 primary

Reader: the implementing agent (the author of the rule), by the criterion of ADR 0704 with the two differences registered above; no
second reader.

| file | primary | verdict | reason |
|---|---|---|---|
| `crates/rcfd-claude/src/discover.rs` | 0126 (E2 before) | agree | added by 975f340d "session discovery, tolerant transcript tail (ADR 0126)"; ADR 0126 (Wave 47E, the bridge library) D1 lists the module, D6 states the two discovery sources and cites the file in four lines; same ADR as under ADR 0690 |
| `crates/rcfd-project/src/time.rs` | 0122 (none before) | agree (borderline) | added by 46226203 "project folder library (Studio L1, ADR 0122)"; ADR 0122 does not name `time.rs`, but D4 says the UTC formatting uses the Hinnant civil-from-days algorithm that the file's header cites, and its check T1 (known dates, 100 000 consecutive days) is what the two tests of the file do (`known_dates`, `days_step_one_calendar_day_at_a_time`). ADR 0690 counted this file disagree-null because the ADR does not cite it by name; criterion (b) accepts a registration by content here |

Borderline rows: 1. The 100 % rests on two files, one borderline; it cannot support the acceptance's 80 % on 20 files.

### E8: the supplementary reading (20 of the 29 files whose first E8 ADR differs from the E0 primary)

The 29 files and the 20 drawn with `random.Random(7730).sample` are in `samples_e8.json`. Read once, same reader, same criterion, for
both ADRs of each file.

- **E0 right, E8 first wrong: 19 of 20.** In most of them the commit subject names the ADR that the work builds on before the ADR
  that registers it ("the reduction of ADR 0474 ... (ADR 0511 D4)", "the state form of ADR 0411 ... " with the header naming the
  second ADR); `sb_loads.rs` names ADR 0281 (thermal strain) before its own ADR 0276; `mesh_transform_u.rs` names ADR 0133 (run control,
  only a dated note) before ADR 0155; `w27c_registry.rs` was touched by an F-237 commit whose ADR 0244 only cites it as an existing test,
  while the header's ADR 0051 registered its checks R1 and R2.
- **E8 right, E0 wrong: 1 of 20**, `crates/rcfd-mcp/src/edit_hook.rs`: its header names ADR 0173 (D5 (b), the umbrella workspace
  layer) first and ADR 0195 P6 second; ADR 0195 is the F-275 registration that lists the module and its checks, and the commit names
  only 0195.
- Over all 948 E0 files with E8: the first E8 ADR equals the E0 primary on 919 (96.9 %), the last on 898; the E0 primary is among
  the E8 ADRs on 943. For the 29 that differ, the E0 primary is among the E8 ADRs on 24 and is the last on 22.

Result: E8 is not better than E0 where they differ (1 of 20), so E8 stays below E0; the order registered is kept.

The 20 files read (E0 primary, then the E8 ADRs in order):

- `crates/rcfd-fem/tests/sa_cms_pattern_default_bits.rs` (E0 0511, E8 0474, 0511)
- `crates/rcfd-fem/tests/sa_rotor.rs` (E0 0476, E8 0450, 0476, 0417)
- `crates/rcfd-fem/tests/sb_modal.rs` (E0 0284, E8 0273, 0284)
- `crates/rcfd-fem/tests/si_rotating_whirl_sparse.rs` (E0 0427, E8 0411, 0427)
- `crates/rcfd-fem/tests/sa_prestress_mixed.rs` (E0 0486, E8 0473, 0486)
- `crates/rcfd-fem/tests/sa_cms_enrich_default_bits.rs` (E0 0532, E8 0511, 0532)
- `crates/rcfd-post/tests/sa_solid_rotor.rs` (E0 0476, E8 0450, 0476, 0417)
- `crates/rcfd-studio/tests/mesh_transform_u.rs` (E0 0155, E8 0133, 0155)
- `crates/rcfd-fem/src/rotating/whirl/state/arnoldi.rs` (E0 0428, E8 0416, 0428)
- `crates/rcfd-fem/tests/si_knife_edge_closed_form.rs` (E0 0394, E8 0390, 0394)
- `crates/rcfd-mcp/src/edit_hook.rs` (E0 0173, E8 0195)
- `crates/rcfd-fem/tests/sa_gyro.rs` (E0 0472, E8 0450, 0472)
- `crates/rcfd-fem/tests/sb_loads.rs` (E0 0276, E8 0281, 0276)
- `crates/rcfd-fem/tests/sa_gyro_sector_split_schur_default_bits.rs` (E0 0527, E8 0492, 0527)
- `crates/rcfd-fem/tests/sa_cms_residual.rs` (E0 0474, E8 0469, 0464, 0474)
- `crates/rcfd-io/tests/w27c_registry.rs` (E0 0051, E8 0244)
- `crates/rcfd-fem/tests/si_eas_hg3.rs` (E0 0630, E8 0614, 0630)
- `crates/rcfd-fem/tests/si_rotating_whirl_state_sparse.rs` (E0 0428, E8 0416, 0428)
- `crates/rcfd-fv/tests/coupled_ddt_fractional_bdf2_f757.rs` (E0 0687, E8 0675, 0687)
- `crates/rcfd-fem/tests/si_modal_mpc.rs` (E0 0384, E8 0296, 0384)

### What the numbers say

- The extension works as asked and changes little in the primary: the index differs from ADR 0690 only in the five extended fields,
  `adrs` grows by 271 links in 216 tests (all by ADRs the commits named), 5 tests change primary. The coherence step is gone and the
  one commit that it dropped (`time.rs`, ADR 0122) is now read right.
- The acceptance's sample of at least 20 E8-primary files cannot be drawn: 948 of the 950 files with an E8 candidate already have an
  E0 primary, and the E2-only tests that have no E0 evidence have, in 468 of 473 tests, a first commit that names no ADR and no item with a
  registration ADR (the initial import and F-numbered commits of items without a `-fNNN-` ADR). Moving E8 above E0 would give a
  population of 950 files and would be wrong for 19 of 20 of those that differ.

### Remaining questions

1. Status. Options: (a) partial, because the sample of 20 is not drawable (2 files) and the acceptance's clause is not met; (b) done,
   arguing that the mechanism and its 2 of 2 are as asked. Recommendation: (a).
2. Where E8 sits: kept below E0, E1 and E6 and above E2 (registered). Option: above E0 would give a population of 950 files but, by
   the reading E8, is wrong for 19 of 20 of the differing files. Recommendation: keep.
3. The command-line default has E8 on, so the gate's `adrs` lists now include the 271 new links and `--adr` prints 216 more tests.
   Options: (a) keep, taken here on the standing instruction (the acceptance asks for the extension); (b) default off and opt-in.
   Recommendation: (a).
4. The 468 E2 tests whose first commit names no ADR or item stay without primary (H1/H2 as before). Items named in such a subject
   whose ADRs are named only in the roadmap note, not in a file name, could be E8 evidence too (the registration ADR is defined by
   the file name, as in ADR 0690). Options: (a) a follow-up that counts it before the run; (b) leave. Recommendation: (a).
5. A second reader for the 2 + 20 files read here (ADR 0704 found the first reader's agreement too high). Recommendation: a
   follow-up, cheap, since the criterion is registered.

## For the lead: roadmap entries

Proposed status and note of F-773:

> Proposed status: partial.

> Test-to-ADR index, gate attribution. Follow-up of F-768 (ADR 0690, 2026-11-17). PARTIAL. ADR 0720, 2026-11-17: evidence of kind E8 in `scripts/adr_test_index.py` (command-line default on, `--no-e8` gives the output of ADR 0690, library default off): on a full git history the ADRs named in the subject of the commit that first adds a test's file (every `ADR dddd` that exists, then the registration ADR of every `F-ddd` that has one) extend `adrs`, `items` and `evidence` of its tests, with no coherence step; the primary tier order is E0 > E1 > E6 > E8 > E2 and the first E8 ADR is the primary. Counted before the code and met exactly: 5695 tests in 950 files, 6089 links, 271 links in 216 tests naming an ADR the test did not have. The index of the final tree differs from that of ADR 0690 in no field but the extended ones and the primary (0 differences in 7847 tests, 5 tests change primary: `discover.rs` 0126 and `time.rs` 0122); a shallow clone and a git archive copy get 0 E8 entries and the same index; 2 git processes, 20.9 s; 11 of 11 mutations caught; 149 unit tests OK. The acceptance's sample of at least 20 files with an E8 primary cannot be drawn: the population is 2 files (948 of the 950 files with E8 already have an E0 primary), both read agree by the implementing agent (no second reader). A supplementary reading of 20 of the 29 files whose first E8 ADR differs from the E0 primary found E8 right in 1 and E0 right in 19, so E8 stays below E0. Maintainer's question open: F-787, F-793.

> Follow-up proposed: A second reader for the E8-primary files and the supplementary sample of ADR 0720. Deps F-773. Acceptance: Every limit not stated here is registered in the ADR before the reading. The 2 files with an E8 primary and the 20 files of the supplementary sample of ADR 0720 (`random.Random(7730)`, listed in its addendum) are read by a reader who did not write the rule, against the criterion of ADR 0704 registered before the reading, and agree with the first reading on at least 85 % of the files; the disagreements are listed with a reason; no limit of ADR 0720 is changed by the second reading.

> Follow-up proposed: Items named in a commit subject whose ADRs are named only in the roadmap note as evidence of the files the commit adds. Deps F-773. Acceptance: Every limit not stated here is registered in the ADR before the run. The ADRs that the roadmap note of an item names (`ADR dddd` in the note, the mapping the index already uses for E4) are evidence of kind E8b of the tests of a file whose first commit names that item in its subject and whose item has no registration ADR by file name; the added evidence is counted before the run, the index of the final tree is compared with that of ADR 0720 in every field except the added `adrs`, `items`, `evidence` and the primary, a shallow clone gets no E8b evidence, the tier of E8b in the order is registered before the run, and a fresh registered sample of at least 20 files with an E8b primary is read by a reader and a second reader with at least 80 % agreement, or the population is counted and the sample limit stated as missed by construction.

`docs_site` on the final tree (`cargo test -q --release -p rcfd-cli --test docs_site`): 2 passed.
