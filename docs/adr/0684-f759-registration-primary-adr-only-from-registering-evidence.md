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
