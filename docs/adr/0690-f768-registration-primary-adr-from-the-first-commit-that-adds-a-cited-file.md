# ADR 0690: F-768 registration: the primary ADR of a file-citation-only test from the commit that adds the file

Status: **REGISTERED 2026-11-17, written before any code of F-768 and before any run of its checks.** Not yet accepted by the
maintainer. Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-768 (deps F-759), proposed by [ADR 0684](0684-f759-registration-primary-adr-only-from-registering-evidence.md)
addendum 1 (R8 missed: on the E2 stratum the earliest ADR that names a file registered it in 3 of 20 files; the number order equals
the git order of the ADR files, so "the weak part is the evidence, not the order"). Number 0690 given by the lead. Branch
`f768-work` from ae4d2af7 (F-737 / 0679, F-759 / 0684, F-760 / 0685 and F-761 / 0681 merged), worktree `w49i-work/f768`, scratch
`w49i-work/f768-scratch`.

House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here; each registered run made once; existing tests not edited; no Rust
file, hash table or golden touched; stdlib Python only; clean-room; the roadmap files are not edited. Choices the maintainer would
normally be asked about are taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except where
listed as open.

## What F-768 asks

> Every limit not stated here is registered in the ADR before the run. A test whose only primary evidence is a file citation takes its primary from the first git commit that adds the file (an `ADR NNNN` in its subject, or an item `F-NNN` whose registration ADR exists), else no primary, by a rule registered before the run, with the history depth the gate has stated and a fall-back to no primary where the history is shallow; the 20 files of ADR 0684 sample D (seed 7590) and a fresh registered sample of at least 20 file-citation-only files are read with at least 80 % agreement; the attribution data of `gate_attribute.py` is unchanged for the existing consumers.

## What was read, and what was done beforehand (disclosed)

- `scripts/adr_test_index.py` at ae4d2af7 (rule R: `primary_adr(..., "registered")`, `REGISTERED_TIERS`, `LISTING_ADRS`, `build(root,
  meta, primary_rule)`, the command line `--primary registered|p0679`, default `registered`), `scripts/test_adr_test_index_primary.py`
  (its N7 runs the command line without a flag in a temp folder that is not a git repository and asserts the E7-only test has no
  primary), `scripts/gate_attribute.py` (`adr_cell` and the record field `primary_adr` are the only readers of the primary),
  `.github/workflows/gate.yml` (the index runs as `python scripts/adr_test_index.py --out ...`, no flag; the job's `actions/checkout`
  has `fetch-depth: 0`, line 101, which is the history depth the gate states), ADR 0684 (rule R, R4, sample D and its table of
  readings), ADR 0685 and 0681.
- Before this text: `cargo metadata` saved to scratch, one run of the unchanged index on this tree (`--metadata`, default flag,
  that is rule R): **38.6 s** wall (the "before" of the run-time check), saved as `idx_before.json`; the repository has 4519 commits
  and is not shallow (`git rev-parse --is-shallow-repository` prints `false`); one `git log --reverse -M --diff-filter=AR
  --name-status` over the whole history takes **0.5 s** and prints 6430 lines; the first commit is the initial import ("Initial
  import of rcfd: clean-room Rust CFD suite"); 3608 commit subjects contain `ADR dddd`, 2886 contain `F-ddd`. Nothing about which of
  the 78 E2 files gets which primary was computed before this text.

## Design

**Rule G (primary from the commit that adds the file).** A new `--primary git` (library `primary_rule="git"`; the library default of
`build()` stays rule P, as before). It starts from rule R, test by test, and changes only the tests whose rule-R primary evidence is
**E2** (the file-citation-only tests: no E0, E1 or E6 evidence). For these:

1. **Origin commit of a file.** One batched pass, `git log --reverse -M --diff-filter=AR --name-status --format=%x01%H%x09%s
   --relative` run in the workspace root with `core.quotepath=off`, read oldest first. A line `A path` sets `origin[path]` to that
   commit if the path has none; a line `R<score> old new` sets `origin[new]` to `origin[old]` if `old` has one, else to that commit;
   an existing origin is never replaced (a path deleted and added again keeps its first). The origin of a file is therefore the
   first commit that adds the file, following renames made at 50 % similarity (git's default) so that a moved file is not
   credited to the commit that moved it. No `--follow`, no call per file.
2. **Primary from the subject.** Of the origin commit's subject (the subject line only, not the body): the first `ADRs? dddd`
   whose ADR exists; else the first `F-ddd` that has a registration ADR, which is the ADR whose file name carries `-fddd-`
   (`ADR_FILE`, the same items the index already maps), the file named `registration` first, then the lowest number; else none.
   The subject's ADR wins over its item.
3. **Coherence.** The candidate becomes the primary only when it is one of the test's own `adrs` (the index's `adrs`, `items`,
   `evidence` are never changed by this rule). A candidate that is not in `adrs` gives **no primary** and is counted
   (`not_in_adrs`). `primary_evidence` stays `E2`.
4. **History depth.** The rule runs only on a full history. `git rev-parse --show-toplevel --is-shallow-repository` is read once in
   the workspace root: history is `full` when the top level is the workspace root (so an enclosing repository is never read) and the
   second line is `false`; `shallow` when it prints anything else (an old git that does not know the flag prints the flag back, which
   counts as shallow); `none` when the root is not the top of a git repository, git is missing or a git call fails. In `shallow` and
   `none` the E2-only tests get **no primary** (the fall-back of the acceptance; they keep `adrs`), and the index says so.
5. **Cost cap.** At most 2 git processes per index run (`rev-parse`, one `log`), none per file; asserted by a test that counts the
   calls on a fake repository.
6. **Output.** Test records keep exactly their keys. `coverage.primary` gets one new key `git`: `{"history": "full|shallow|none",
   "e2_tests": n, "from_git": n, "no_commit_rule": n, "not_in_adrs": n, "no_origin": n, "git_calls": 2}`; the existing keys
   `with_primary` and `by_evidence` are recomputed from the final primaries (they are the same kind of number as before).
   `adrs[n].primary_tests` follows the final primary.
7. **Default.** The command line default becomes `git`, so that the gate (which gives no flag) takes the acceptance's rule where
   its checkout is deep; `--primary registered` and `--primary p0679` stay. Taken on the standing instruction of 2026-11-11: the
   acceptance names the gate's history depth, which is a statement about the gate. The data of the consumers are unchanged in
   shape and in every field except the primary of the E2-only tests (check G5). `gate.yml` and `gate_attribute.py` are not edited.
   (On a git-archive copy or a shallow clone the default now gives no primary to E2-only tests where rule R gave the earliest
   citing ADR; this is the stated fall-back.)

**Reading by sample.** For each file the reader sees its header comment, the ADR named by the candidate (title, first paragraph and
whether it describes the file or only names it) and the first commit that adds the file (`git log --diff-filter=A --follow`). The
commit subject is *not* by itself the reading, because the rule uses it: a primary **agrees** when the ADR registered the file's item
or first described the file (its header, its own section on the file, or the ADR whose item the file implements); it **disagrees**
when it only lists, edits or runs the file. A file with primary `null` is **agree-null** when no ADR registered or described the
file (an initial-import file whose item has no ADR, or a file whose item's registration ADR does not exist) and **disagree-null**
when the reader finds such an ADR that the rule missed. Both counts are reported: *overall agreement* counts agree and agree-null
over all files read; *precision* counts agree over the files with a primary. The reader is the implementing agent (disclosed; the
reading uses the ADR text and the header, not only the commit).

## Checks (limits fixed now)

| id | check | limit | estimate behind it |
|---|---|---|---|
| G1 | **negative control, shallow**: `git clone --depth 1` of this worktree (file URL), index with the default flag and the same saved metadata: history `shallow`; every E2-only test has `primary_adr = null`; every other field of every test equals the full run | 0 differences | by construction |
| G2 | **negative control, no git**: a `git archive` copy, same run: history `none`, same result | 0 differences | by construction |
| G3 | git calls on the real tree (counted by wrapping `subprocess.run` in a one-off driver) and in the fake-repo test | at most 2, none per file | by construction |
| G4 | run time of the index, default flag, saved metadata, one run after the code is final, against the 38.6 s measured before | at most before + 10 s | the log pass is 0.5 s and the parse of 6430 lines is small; the margin covers the shared machine |
| G5 | data unchanged: against `idx_before.json` (rule R, same tree): every test field except `primary_adr`, `primary_evidence`; `adrs[n].files/items/tests`; `binaries`, `crates`, `items`; `coverage` except `primary`; every test whose rule-R evidence is not E2 has the same primary; every test whose rule-R evidence is E2 has `null` or an ADR in its `adrs` with `primary_evidence` `E2`; `python -m unittest scripts/test_gate_attribute.py scripts/test_gate_hooks_f761.py scripts/test_adr_test_index.py scripts/test_adr_test_index_wave.py scripts/test_adr_test_index_primary.py` pass with no edit; `gate_attribute.py`, `gate.yml` byte-identical to ae4d2af7 | 0 differences, all pass | by construction |
| G6 | independent recomputation: for every distinct E2-primary file of `idx_before.json`, `git log --diff-filter=A --follow --format=%H%x09%s -- file` per file (an audit, not the index's path), the oldest commit, the same subject rule: its candidate equals the batched rule's candidate (before the coherence step) | at least 95 % of the files | `--follow` and the rename chain agree except for copies and for files that git sees as renamed at less than 50 %; the E2 files are mostly untouched since their addition |
| G7 | coverage record: tests with an ADR and no primary after, and the split of the E2-only tests into `from_git`, `no_commit_rule`, `not_in_adrs`, `no_origin` | at most 456 tests with an ADR and no primary (the 43 of ADR 0684 plus every one of its 413 E2 tests; by construction); the split is recorded | ADR 0684 found more than half of the E2 files to be initial-import files, which have no ADR in their subject, so a large share of null is expected and is the honest answer |
| G8 | **sample D re-read**: the 20 files of ADR 0684 sample D (its table; the population is the 78 E2 files; the files are fixed by that table, none redrawn), each with its new primary or null, read as defined above | at least **16 of 20** overall agreement (80 %, the acceptance); precision reported with its denominator | the 3 agreeing files of D had commits that named their ADR; the 17 others were initial import (null expected, scored agree-null when no ADR registered them) or items whose registration ADR is not among the citing ADRs |
| G9 | **fresh sample E**: from the sorted list of distinct files with rule-R evidence E2 at the final tree, minus the 20 files of D, **25** drawn once with `random.Random(7680).sample`; each read as above | at least **20 of 25** overall agreement (80 %) | as G8 |
| G10 | precision of the non-null primaries over D and E together | at least 80 % of the files with a primary | the git rule takes the registering commit; a miss would be a commit that names an ADR which only edits the file |
| G11 | `python -m unittest discover -s scripts` from the clean worktree; `docs_site` (this ADR is a file under docs/) | pass (ignoring the 4 skipped E2E tests); 2 passed | |

Samples are drawn once, from the index built after the code is final. The overlap of E with the samples of ADR 0679 and 0684 (C, seed
759) is counted and disclosed, not removed.

## Negative controls and planned mutations

Tests in a new file `scripts/test_adr_test_index_git.py`, on a fake workspace that is a real git repository made in a temp folder
(`git init`, commits with `-c user.name -c user.email` and fixed dates; skipped with a message when `git` is missing):
- T1 a file added by a commit whose subject has `ADR 0120`, cited by ADRs 0120 and 0130: primary 0120, evidence E2.
- T2 a file added by a commit with `(F-020)` whose registration ADR 0105 cites the file: primary 0105; a file whose item has no
  registration ADR file: none.
- T3 a later commit naming ADR 0130 changes the file: the primary stays that of the first commit.
- T4 the file is renamed in a later commit named `ADR 0130`: the origin is still the first commit (rename chain).
- T5 the first commit names no ADR or item: none, although two ADRs cite the file.
- T6 the commit names an ADR that does not cite the file: none, `not_in_adrs` counted; `adrs` unchanged.
- T7 the subject has both `ADR 0120` and `F-020`: the ADR wins.
- T8 a test with E0, E1 or E6 evidence is not changed by the rule; E3/E4/E5/E7-only tests stay without primary.
- T9 a `git clone --depth 1` of the fake repository: history `shallow`, E2-only tests have no primary; a copy without `.git`: `none`.
- T10 the fake repository is a subfolder of a larger repository: history `none` (an enclosing repository is not read).
- T11 at most 2 git processes per run (`subprocess.run` wrapped and counted).
- T12 `--primary registered` on the same repository gives rule R; the command line without a flag gives rule G.

Mutations on a scratch copy of `scripts/`: M1 the newest add commit instead of the oldest (T3, T4), M2 no rename chain (T4), M3 the
shallow check removed (T9), M4 the coherence step removed (T6), M5 the item before the ADR (T7), M6 the body of the commit read as
well as the subject (T5), M7 the enclosing-repository check removed (T10), M8 the command-line default left at `registered` (T12).
Each is caught or recorded as not caught.

## Decision rule

**done** if G1 to G11 are all met and all mutations are caught; **partial** if any limit is missed (the number is recorded and the
limit stays), in particular G8, G9 or G10 (the acceptance's 80 %); **todo** if G1, G2 or G5 fails (a wrong or changed result).

## Limits (what is not covered)

- A test whose file is in the initial import, or whose origin commit names no ADR or item, gets no primary; the rule does not guess.
  A wave tag or a plain item in a commit body is not read.
- The origin is the first commit by `git log --reverse` date order, not by topology; on a history with merged side branches the
  earliest by date is taken.
- An ADR named by the commit that does not cite the file in its text is not a primary (coherence); extending `adrs` with git
  evidence (a new evidence kind) is a change of the index's data and is left to a follow-up.
- Renames below git's 50 % similarity, and files copied from another file, start a new origin.
- The reading is by the implementing agent and on 45 files of 78 plus the null cases; it is not a second reader.
- E0, E1, E6 primaries are not re-checked against git (the samples of ADR 0679 and 0684 did that).
- The gate's own checkout depth (`fetch-depth: 0`) is stated, not enforced: where the history is shallow the rule falls back to no
  primary and says `shallow` in `coverage.primary.git.history`.
