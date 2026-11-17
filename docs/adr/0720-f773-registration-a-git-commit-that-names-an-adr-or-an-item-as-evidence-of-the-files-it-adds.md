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
