# ADR 0653: F-723 registration: the daily full gate in CI, with failures attributed to ADRs

Status: **REGISTERED 2026-11-16, written before any code of F-723 and before any run of its checks.** Not yet accepted by the
maintainer. Stage 1 (the fmt fix, the daily schedule, the skip when nothing changed) is registered here in full; stage 2 (the
test-to-ADR index, the attribution step, its tests and the docs) is registered in amendment 1, written and committed before any
code of stage 2. Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-723, requested by the maintainer on 2026-11-16 (the lead adds the roadmap entry). Number 0653 given by the lead.
Branch `f723-work` from d40cd665, worktree `w49i-work/f723`, scratch `w49i-work/f723-scratch`. The lead asked on 2026-11-16 to
split the work in two stages so stage 1 can be pushed the same day.

House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here; each registered run made once; no Rust code changed; no edit
of `baseline_hashes.rs`, of a hash table or of a golden; no new dependency; the roadmap files are not edited. Every choice the
maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation), except where listed as open. Clean-room: nothing here is numerical; the only external facts used are the
Windows command-line limit (32767 UTF-16 characters for `CreateProcess`, from memory, stated as such) and GitHub Actions
behaviour (from memory; what cannot be checked without a run on GitHub is listed under "Limits").

## What F-723 asks (from the brief)

> Run the full gate daily in GitHub Actions and attribute every failure to an ADR, so per-ADR merges no longer need the full suites.

Clauses of stage 1: **s1** diagnose why the scheduled runs of 2026-11-08 and 2026-11-15 fail after about 30 s at `cargo fmt
--all -- --check` and fix it without changing what the gate checks; **s2** run daily (cron), main only, `workflow_dispatch`
kept; **s3** skip the scheduled run when the SHA to test equals the SHA of the last successful gate run, found with the GitHub
API and `GITHUB_TOKEN`, read-only. Clauses of stage 2 (registered in amendment 1): **s4** the test-to-ADR index with its
coverage; **s5** the attribution step (re-run, index lookup, candidate merges, optional bisect, summary, JSON, optional issues);
**s6** docs, including the per-merge check set; **s7** unit tests on fixtures.

## What was read and computed before this text (disclosed)

- `.github/workflows/gate.yml`, `scripts/gate_test.py`, `scripts/test_gate_test.py`, `docs/test-process.md` (headings),
  `docs/releasing.md` (the rows that name the workflow).
- `gh run view 43696435968 --log-failed` (2026-11-15, schedule, main): the fmt step prints `The filename or extension is too
  long. (os error 206)` and then cargo fmt's usage text, exit 1. `gh run list --workflow gate.yml`: the scheduled run of
  2026-11-01 (41290190837) passed in 1 h 22 min; 2026-11-08 (42463780842) and 2026-11-15 fail in 32 s and 34 s.
- Arithmetic from `cargo metadata --no-deps` at d40cd665, each target's `src_path` rewritten to the runner's root
  `D:\a\rcfd\rcfd` plus one separator: 24 packages, the whole workspace's target paths sum to **63802** characters; the largest
  single package (rcfd-fem, 347 targets) to **20031**, then rcfd-api 12491 (231 targets), rcfd-fv 10233 (192).
- Read-only API calls: the last successful gate runs on main are 42083180301 (eef6c094, 2026-11-05, dispatch) and 41290190837
  (edb4839f, schedule); main is d9dd237f on GitHub; the default branch is `main`; `actionlint` 1.7.12 is installed locally.

## Diagnosis (s1)

`cargo fmt --all` gathers the root file of every target of every package and hands them to one `rustfmt` process (one per
edition). On Windows the command line of a process is limited to 32767 characters; 63802 is above it, so process creation
fails with os error 206 and cargo fmt prints its usage. The run of 2026-11-01 passed because the workspace then had fewer
test targets; the growth since crossed the limit (the house rules already note the same failure on the local machine, where
the root path is longer). It is not an argument error: the same command passes on a smaller workspace.

## Design

**D1 (s1) fmt per package.** The step lists the workspace packages with `cargo metadata --no-deps --format-version 1` and runs
`cargo fmt -p <package> -- --check` for each, remembering a failure and exiting 1 at the end if any package failed (every
package is checked, not only up to the first failure). `--all` means "all workspace members and their local path
dependencies"; `cargo metadata` shows no path dependency outside the 24 members, so the set of files checked is the same. The
largest package needs 20031 characters, 61 % of the limit; the margin is recorded under "Limits".

**D2 (s2) daily.** `cron: "0 2 * * *"`. A schedule runs only on the default branch (`main`); a decide job also refuses a
scheduled run whose ref is not `refs/heads/main`. `workflow_dispatch` with its `os` input is kept and always runs.

**D3 (s3) skip when nothing changed.** A new first job `decide` on `ubuntu-24.04` (the pinned Linux runner of `ci.yml`), with
`permissions: actions: read, contents: read` only, finds the last successful gate on main: it lists successful runs of
`gate.yml` on `main` (newest first, 20) and takes the first whose job `full gate, windows-latest` concluded `success` (so a
green Ubuntu dispatch does not count for the Windows gate), and outputs its `head_sha` as `last_green`. For a scheduled run,
`run` is `false` when `GITHUB_SHA` (for the schedule event, the tip of main) equals `last_green`, else `true`. Any API
failure, or no green run found, gives `run=true` (fail open: a broken query never hides a gate). The gate job gets `needs:
decide` and `if: needs.decide.outputs.run == 'true'`; its steps and commands are otherwise unchanged except D1. `last_green`
is passed to the gate job as an environment value for stage 2.

## Checks (stage 1)

| id | what | limit (fixed now) | estimate behind it |
|---|---|---|---|
| S1 | negative control of the diagnosis: `cargo fmt --all -- --check` on the worktree at d40cd665, locally | exits non-zero and prints `os error 206` | the local root is 30 characters against 15 on the runner, so the sum is above 63802 |
| S2 | the fmt step of D1, its bash text run locally in Git Bash on the worktree | exit 0; 24 packages checked | main is formatted (the per-crate form passes for every merge, house rule 7) |
| S3 | negative control: the same step on a `git archive` copy in scratch with one mis-formatted line added to `crates/rcfd-core/src/lib.rs` (not the last package in the list) | exit 1; the output names that file; all 24 packages still checked | rustfmt `--check` prints a diff with the file name |
| S4 | `actionlint` 1.7.12 on `.github/workflows/gate.yml` | 0 findings | |
| S5 | the decide step's bash text run locally with a stub `gh` on `PATH` in four cases: (a) last green SHA equal to `GITHUB_SHA`, schedule; (b) different; (c) no green Windows job; (d) `gh` exits 1 | (a) `run=false`; (b), (c), (d) `run=true`; `last_green` equal to the stub's SHA in (a), (b) and empty in (c), (d) | D3 |
| S6 | the decide step's bash text run once locally against the real API (read-only, `GITHUB_REPOSITORY=c4ss/rcfd`, event `schedule`, `GITHUB_SHA` = d9dd237f) | `last_green` = eef6c094... (the 2026-11-05 run 42083180301, if its Windows job is the green one, else edb4839f); `run=true` | the run listing above |
| S7 | the gate's checks unchanged: `git diff d40cd665 -- .github/workflows/gate.yml` | the clippy, tests, summary and upload steps' `run`/`with` text byte-identical; only the fmt `run`, the trigger, a `decide` job, `needs`/`if` and an `env` value added | D1 to D3 |

Mutations (stage 1), each on a scratch copy of the step text: **M1** drop the remembered failure (`rc=1`) so only the last
package's status counts: S3 must fail (exit 0); **M2** invert the comparison in decide: S5 (a) and (b) must fail; **M3**
remove the fail-open default: S5 (d) must fail.

Decision rule for stage 1: every check met and every mutation caught gives "stage 1 done"; any miss is recorded with its
cause. F-723 as a whole is done only when stage 2 is done as well; the first real run on GitHub is outside what an agent can
run, and what it must show is listed in the addendum.

## Limits

- Nothing here runs on GitHub: the `decide` job's permissions, the jobs API's job names, and the skip on a real schedule are
  checked only on the first run (listed in the addendum).
- D1 keeps working while the largest package's target paths stay below 32767 characters (today 20031 on the runner); rcfd-fem
  would need about 220 more test targets of the same path length to reach it.
- The skip compares SHAs only; a green run on a commit whose tests are flaky is not re-run.

## Amendment 1 (2026-11-16, before the corrected S5 and M2, M3 runs): the stub was not on PATH

The first S5 run, and the M2 and M3 runs made with it, did not use the stub `gh`: the stub folder was prepended to `PATH` as
`E:/projects/...`, and in Git Bash the colon after the drive letter splits that entry, so bash found the real `gh.exe`. Those
runs therefore made read-only calls to the real API (all five cases printed `run=true`, `last_green=eef6c094...`, which is
the S6 situation, not S5). They are recorded here as void, not as results of S5, M2 or M3. Correction: the stub folder is
passed as `/e/projects/...` (the Git Bash form), and each case first asserts that `command -v gh` is the stub. Limits and
expected results of S5, M2 and M3 are unchanged; the corrected runs are made once. Mutation M3 is also corrected in its text:
the fail-open default is two edits (`|| ids=""` and `|| ok=0`); M3 removes both, so a failing `gh` must stop the step with
an error (run not set) instead of `run=true`; S5 (d) "exit 0 with run=true" then fails.

## Addendum 1: results of stage 1 (2026-11-16)

Each run made once (S5, M2 and M3 after amendment 1); logs and step texts in `w49i-work/f723-scratch` (`s1.log`, `s2.log`,
`s3.log`, `m1.log`, `fmt_step.sh`, `decide_step.sh`, `m2.sh`, `m3.sh`, `stub/gh`). The step texts were taken from the
workflow file with a YAML parse, so what ran is what the workflow runs.

| id | limit | measured | met |
|---|---|---|---|
| S1 | `cargo fmt --all -- --check` exits non-zero with `os error 206` | exit 1, `The filename or extension is too long. (os error 206)`, then the usage text (the CI log's failure, reproduced) | yes |
| S2 | the new fmt step exits 0, 24 packages | exit 0, 24 `fmt <package>` lines | yes |
| S3 | mis-formatted line in rcfd-core: exit 1, file named, 24 packages checked | exit 1, `Diff in ...\crates\rcfd-core\src\lib.rs:24`, 24 packages | yes |
| S4 | actionlint 0 findings | 0 findings, exit 0 | yes |
| S5 | (a) `run=false`; (b), (c), (d) `run=true`; `last_green` stub SHA in (a), (b), empty in (c), (d) | (a) false, aaaa1111; (b) true, aaaa1111; (c) true, empty; (d) true, empty. Extra cases: a dispatch with equal SHA runs (true); a schedule on another ref does not (false) | yes |
| S6 | real API: `last_green` eef6c094 (or edb4839f), `run=true` | `last_green=eef6c09479bbd8cfdf344a8342afcf048967a463` (run 42083180301, its job `full gate, windows-latest` success), `run=true` | yes |
| S7 | every other gate step byte-identical; only fmt, the trigger, `decide`, `needs`/`if`, an `env` value added | YAML compare against d40cd665: checkout, toolchain, python, clippy, tests, summary, upload steps equal; `runs-on`, `timeout-minutes` (340), `defaults`, workflow `env`, `permissions`, `concurrency`, the dispatch input equal; only the fmt step differs | yes |

| mutation | caught by | result |
|---|---|---|
| M1 the fmt loop forgets a failure (`|| true`) | S3 | exit 0 with the mis-formatted file: caught |
| M2 decide compares with `!=` | S5 (a), (b) | (a) true, (b) false: caught |
| M3 the fail-open defaults removed | S5 (d) | the step exits 1 (the decide job would fail and the gate be skipped): caught |

Deviations: amendment 1 (the first S5 set-up did not use the stub). The fmt step lists packages with `cargo metadata
--no-deps --format-version 1 --locked` and checks the list is not empty, so a failing `cargo metadata` fails the step instead of
checking nothing; it writes the names without a trailing newline because on Windows Python's text output adds `\r`, which would
glue to the last package's name. `docs/releasing.md` now says "daily on main" where it said "Sundays".

Decision by the registered rule: every stage 1 check met and every mutation caught: **stage 1 done**. F-723 stays open until
stage 2.

What only the first run on GitHub can show (nobody can run the workflow from here): (1) the `decide` job is allowed to list
runs and jobs with `actions: read` (expected; a refusal shows as `last green ...: none` in its log and the gate still runs);
(2) the first scheduled run after the push runs the gate (main has moved since eef6c094) and its fmt step prints 24 `fmt`
lines and passes; (3) the next scheduled run, with no push between, is skipped (`run: false` in the decide log, gate job
"skipped"); (4) the fmt step on `ubuntu-24.04` through a dispatch also passes (no path limit there).

## Amendment 2 (2026-11-16): registration of stage 2, written before any code of stage 2

Read before this text, in addition to the above: the artifact of the failed main gate 42083992758 (Ubuntu, eef6c094,
downloaded read-only to `f723-scratch/dl42083992758`): `times.tsv` has 3625 rows (2823 ok, 801 ignored, 1 FAILED:
`fan01_hub_mesh_w49r` / `w49r_default_meshes_are_bit_identical`), `test.log` 542 `Running` blocks; doc-test output goes to
`test.log` after the `gate_test:` line. Counted from the file tree at d40cd665: 1134 `crates/*/tests/*.rs` files, 267 of
them with an `fNNN` in the name; 634 ADR files, 371 of them named `NNNN-fMMM-...`. No index or attribution code was run.

### D4 (s4) the index, `scripts/adr_test_index.py` (stdlib only)

Tests are found statically, so the index needs no build: the targets come from `cargo metadata --no-deps`; each target's root
file is read and `mod x;` declarations are followed (`x.rs`, `x/mod.rs`, `#[path]` honoured); comments, strings and char
literals are blanked before braces are counted, so `mod m { ... }` nesting gives each `#[test]` function its libtest name
(`a::b::name`); `#[ignore]` is recorded. A test's binary is the target name with `-` replaced by `_`, the stem `gate_test.py`
writes in `times.tsv`. Evidence, each recorded with its kind:

- **E1 item in the file name**: `f(\d{2,4})` delimited by `_` or the ends of the file stem (test file or source module) gives
  item `F-NNN` (leading zeros to three digits).
- **E2 ADR cites the file**: an ADR's text contains `<stem>.rs` (word boundary before) of the test's file; for unit tests, its
  source path from `src/`.
- **E3 ADR cites the test function**: an ADR contains the function name as a word, when the name is distinctive (contains `_`,
  at least 12 characters, and the name of a test in one binary only).
- **E4 roadmap note cites the file or function**: as E2 and E3 on each item's `note` in `docs/roadmap.json` gives the item.
- **E5 module** (unit tests only): an ADR contains the module path `rcfd_xxx::a` (at least one module below the crate) of the
  test's module or a parent of it below the crate root.

Item to ADRs: ADR files named `NNNN-fMMM-...` and `ADR NNNN` in the item's note. ADR to items: the reverse. A test is
"mapped" if the union of its evidence gives at least one ADR; "item only" if it gives items without an ADR. The JSON holds
every test (crate, binary, kind, file, name, ignored, ADRs, items, evidence), per-binary and per-ADR tables and the coverage;
`--report` prints coverage and the unmapped binaries; `--adr NNNN` prints that ADR's tests as `cargo test` commands (the
per-merge "ADR's own registered tests").

### D5 (s5) the attribution, `scripts/gate_attribute.py` (stdlib only, git and gh as tools)

- Reads `times.tsv` (FAILED and TIMEOUT rows), `test.log` (the executable of each binary from its `Running` line, a
  `gate_test: build failed` line, failed doc-tests `test <path> - <item> (line N) ... FAILED`).
- `--rerun`: re-runs each failed test once, the way `gate_test.py` runs it (`<exe> <name> --exact --test-threads=1`, cwd the
  crate folder, `CARGO_MANIFEST_DIR` set, a time limit); a pass marks it **flaky**.
- Looks each test up in the index (ADRs, items, evidence).
- Candidates: the first-parent commits in `base..head` (`base` = last green, `GATE_LAST_GREEN`); for each, the files it changed
  against its first parent; relation **direct** if one is the test's file or under its crate's folder, **dependency** if under
  the folder of a workspace crate the test's crate depends on (transitively, from `cargo metadata`) or is `Cargo.toml` /
  `Cargo.lock` at the root; others are dropped. Each candidate carries the `F-NNN` and `ADR NNNN` of its subject and the ADRs
  of those items from the index. A base that is empty or not an ancestor of head gives the last `--max-commits` (30)
  first-parent commits, flagged `base_found: false`.
- `--bisect` (off by default): when more than one candidate remains, binary search over the candidates in commit order for
  the first at which the test fails, assuming it passes at base and fails at head; each probe checks the commit out in a
  `git worktree` under the scratch folder, builds only that test's target (`cargo test -p <crate> --release --test <target>`
  or `--lib`/`--bin`) and runs the one test; `--probe-cmd` replaces the build and run by a command (exit 0 = pass); stops
  when `--bisect-minutes` is used up and reports the narrowed range.
- Writes `gate-out/attribution.json`, a markdown summary (appended to `GITHUB_STEP_SUMMARY` in CI), and with `issues` (a
  subcommand, run only when the dispatch input `open_issues` is true, in a separate job with `issues: write`) one issue per
  non-flaky failure, skipped when an open issue with the same title exists.

Workflow: the gate job checks out with `fetch-depth: 0` (the history is needed); a step uploads the gate logs before the
attribution (so a slow attribution cannot lose them); the attribution step runs only when the tests step failed, with
`timeout-minutes: 40`; it and the index go to a second artifact. Dispatch inputs `bisect` (false), `bisect_minutes` (30),
`open_issues` (false). The checked commands of the gate are unchanged.

### D6 (s6) docs

`docs/test-process.md` gets a section on the daily gate, the attribution output and the per-merge check set: fmt and clippy of
the touched packages, the ADR's registered tests (`--adr`), the touched crates' `--lib` tests, `rcfd-api` `baseline_hashes`,
`rcfd-cli` `docs_site`.

### Checks of stage 2

| id | what | limit (fixed now) | estimate behind it |
|---|---|---|---|
| T1 | `python -m unittest scripts/test_adr_test_index.py scripts/test_gate_attribute.py` (fixture tree for the index: E1 to E5, module paths, nested `mod`, `#[ignore]`, comments and strings with braces; fake `times.tsv` / `test.log`; a temp git repo with merges for candidates and bisect; a fake re-run for flaky) | every test passes | written to the design |
| T2 | static enumeration against a real listing: the index run on a `git archive eef6c094` copy against `times.tsv` of run 42083992758 at the same commit | binaries of `times.tsv` found in the index: at least 99 %; (binary, test) rows found: at least 98 % | the parser follows `mod` files; macro-made tests are the expected misses |
| T3 | coverage at d40cd665 (non-ignored and all) | integration-test binaries mapped to an ADR: at least 50 %; all tests mapped: at least 40 %; reported with the unmapped list | 267 of 1134 test files carry an item; wave-named files are often cited by their ADR |
| T4 | precision: 20 mapped integration-test binaries (sorted by crate and name, every k-th, k = floor(n / 20)), checked by reading the ADR | at least 18 of 20 have a correct ADR (the ADR registers or describes the file's tests) among their ADRs | |
| T5 | dry run on the real failure: `gate_attribute.py` on the 42083992758 `times.tsv` / `test.log` with the index of T2, base edb4839f (the green gate before it), head eef6c094, no re-run | one failure listed with its crate and its ADRs from the index, and at least one candidate; every listed candidate's changed files checked with `git show --stat` to touch the crate or a dependency | the test is `w49r` (wave 49R, the fan hub mesh) |
| T6 | end-to-end (with `GATE_TEST_E2E=1`): a temp crate with a passing, a failing and a flaky test (fails once, passes once a marker file exists) through `gate_test.py`, then `gate_attribute.py --rerun` | the failing test `flaky: false`, the flaky one `flaky: true`; JSON and summary written | |
| T7 | `actionlint` on the new `gate.yml`; the gate's checked commands unchanged (YAML compare as S7, against d55c43c4) | 0 findings; the fmt, clippy and tests steps' `run` unchanged | |
| T8 | `python -m unittest scripts/test_gate_test.py` (the existing tests) | passes | `scripts/gate_test.py` is not changed |

T2 and T5 use a commit older than d40cd665 because only that commit has a real gate listing. If edb4839f is not an ancestor
of eef6c094 (history rewrite), T5 runs with the fallback and records it.

Mutations of stage 2 (scratch copies, each against T1): **MI1** E4 (roadmap notes) dropped; **MI2** the item regex anchored
only at the end of the stem; **MI3** nested `mod` blocks not prefixed to the test name; **MA1** flaky inverted (a re-run pass
marks not flaky); **MA2** the dependency relation dropped (only direct candidates); **MA3** bisect returns the last passing
candidate instead of the first failing; **MA4** the bisect budget ignored. Each must make at least one test of T1 fail.

Decision rule (F-723 as a whole): stage 1 done, T1 to T8 met and every mutation caught gives **done**, with the first GitHub
run's items listed as follow-up checks; a missed T2, T3 or T4 limit is reported with its numbers and gives **partial**; the
issues path and the bisect on CI are not run here (no push) and are listed.

## Addendum 2: results of stage 2 (2026-11-16)

Code: `scripts/adr_test_index.py`, `scripts/gate_attribute.py`, their tests `scripts/test_adr_test_index.py` (12 tests) and
`scripts/test_gate_attribute.py` (10 tests and the end-to-end T6), `.github/workflows/gate.yml` (D5), and
`docs/test-process.md` section 7 (D6). No Rust file and no golden changed; `baseline_hashes.rs` untouched; stdlib Python only.
Logs and inputs are in `w49i-work/f723-scratch` (`t2_diff.json`, `t3_report.txt`, `t3_report_early.txt`, `t4_context.txt`,
`t5_attribution.json`, `t5_summary.md`, `mutations.log`, `mutate.py`).

| id | limit | measured | met |
|---|---|---|---|
| T1 | every unit test passes | `python -m unittest scripts/test_adr_test_index.py scripts/test_gate_attribute.py scripts/test_gate_test.py`: 26 run, OK, 4 skipped (the E2E tests behind `GATE_TEST_E2E`) | yes |
| T2 | binaries of the eef6c094 `times.tsv` found in the static index: at least 99 %; rows found: at least 98 % | 501 of 501 binaries (100 %); 3590 of 3625 rows (99.03 %). All 35 missing rows come from tests made by macros, in three files: `validation_sod` 22, `validation_cylinder_re100_w31` 7, `w33b_stability` 6 (18 ok, 17 ignored). The index has 7 tests that the Ubuntu listing lacks (Windows-only or CUDA tests, `#[cfg]`) | yes |
| T3 | integration-test binaries mapped: at least 50 %; all tests mapped: at least 40 % | binaries 1053 of 1134 (92.9 %); all tests 6986 of 7472 (93.5 %); non-ignored 5424 of 5856 (92.6 %); integration tests 6446 of 6897 (93.5 %); unit tests 540 of 575 (93.9 %); 17 tests map to an item only. Unit-test binaries 19 of 28: all 19 that have tests are mapped, and the other 9 have no tests. 90 binaries are unmapped (listed below) | yes |
| T4 | at least 18 of 20 sampled binaries have a correct ADR | 20 of 20 (k = 52, list in `t4_context.txt`; each has the ADR that registered or described it, for example `sliding_coincidence_f710` 0638, `sb_static` 0275, `overset_f134_records` 0219, `tool_list_f275` 0195) | yes |
| T5 | real failure: one failure with crate and ADRs, at least one candidate, every candidate touches the crate or a dependency | `rcfd-api/fan01_hub_mesh_w49r::w49r_default_meshes_are_bit_identical`, ADR 0181 (E2, E3); edb4839f is an ancestor; 19 first-parent commits; 11 candidates, each checked to touch `crates/rcfd-api` or a dependency of it. The second candidate is `a4e94027 c_grid_grading: hash-neutral default; H1 normalises it (ADR 0181)`, the same ADR the index gives | yes |
| T6 | failing test `flaky: false`, flaky test `flaky: true`, JSON and summary written | `{'fails': ('FAILED', 'FAILED', False), 'flaky': ('FAILED', 'ok', True)}`, summary written | yes |
| T7 | actionlint 0 findings; the fmt, clippy and tests `run` unchanged against d55c43c4 | 0 findings; every earlier step's `run` is unchanged. Checkout gained `fetch-depth: 0` and the tests step gained `id: tests` (no change to what is checked). New: the attribution step, its upload, the `issues` job and three dispatch inputs | yes |
| T8 | `scripts/test_gate_test.py` passes | OK (1 run, 3 E2E skipped); `gate_test.py` unchanged | yes |

| mutation | caught by |
|---|---|
| MI1 E4 dropped | `test_e4_roadmap_note`, `test_coverage` |
| MI2 item regex anchored at the end only | `test_item_regex` |
| MI3 nested `mod` not prefixed | 5 tests (`test_enumeration_names_and_ignored` and others) |
| MA1 flaky inverted | `test_rerun_marks_flaky` |
| MA2 dependency relation dropped | `test_candidates_direct_and_dependency`, `test_bisect_in_worktrees_with_probe_cmd` |
| MA3 bisect returns the last passing candidate | `test_bisect_finds_first_failing`, `test_bisect_in_worktrees_with_probe_cmd` |
| MA4 bisect budget ignored | `test_bisect_budget` |

Unmapped integration-test binaries (81 with tests, 2 without): rcfd-api 19 (`bc_guarded_secant`, `cgrid_case_w34a`,
`compressible_bc_case`, `convergence_wave17a`, `divergence_guard_w32c`, `library_api`, `mesh_params`, `non_conformal_case`,
`physics_service`, `properties`, `radial_frame_w35c`, `runctl`, `scalars`, `sources`, `transient_energy`, `udf`, `w26c_reports`,
`w37a_velocity_correction`, `workflow`); rcfd-assistant 7; rcfd-cli 5; rcfd-core 1 (`table_expr`); rcfd-fem 5
(`sa_benasciutti_tovo`, `sa_broadband_default_bits`, `sa_random_stress_moments`, and `si_eas_hg3_rd` and `si_eas_hg3_rl` with
0 tests found); rcfd-fv 12 (`guarded_secant`, `non_conformal`, `pseudo_control`, `sixdof`, `species`, the 7 `varden_*`); rcfd-gpu 6;
rcfd-io 3; rcfd-post 3; rcfd-project 4; rcfd-reactor 1 (`cell`); rcfd-runner 1 (`contract_w48a`); rcfd-studio 12; rcfd-turb 2. The
full list is in `t3_report.txt`. Most are early wave files whose ADR names the wave (`w34a`, `wave17a`), not the file.

Deviations, all disclosed:

1. **T3 was run early.** The first T3 run (`t3_report_early.txt`) was made by mistake on the first draft of the index, before
   three clean-ups. The clean-ups were: the raw-string helper; targets with `test = false` skipped; the module folder of a target
   root taken as its own folder. They also came before the two fixes that T1 found, the `#[path]` value and the parents of a
   cited module path. Early numbers: integration binaries 1053 of 1134, all tests 6929 of 7427 (93.3 %), unit binaries 19 of 29.
   The table gives the run on the final code. Both runs meet the limits, so no result was chosen between them.
2. **T1 was run twice.** Its first run found the two parser defects named above. They were fixed and T1 was run again; the limits
   did not change.
3. **The dependency relation was narrowed after T5.** A dependency's own `tests/`, `benches/` and `examples/` are not compiled
   into the failing test's binary, so files there no longer make a candidate. T5 had listed 2 such commits as dependency
   candidates (both renamed STL files in rcfd-io tests). T5's limit held either way. A fixture merge for this case was added to
   T1, which passes.

Not run here: the workflow on GitHub (nobody can push from here), `--bisect` with the cargo probe on a real failure, and the
`issues` job with a real token. The bisect and issues logic is covered by T1 with a probe command and a fake `gh`.

**Decision by the registered rule:** stage 1 is done, T1 to T8 are met, and all 7 mutations of stage 2 were caught, so the rule
gives **done**. The items that only the first GitHub run can show are listed below as follow-up checks.

What the first run on GitHub must show (in this order):

1. In the `decide` log, a real last-green SHA (eef6c094 until a new green run), and `run: true` on the first schedule after the
   push.
2. The fmt step prints 24 `fmt <package>` lines and passes on windows-latest.
3. Checkout with `fetch-depth: 0` works on the Windows runner, and the clone time is acceptable.
4. A schedule with no push since the last green run shows `run: false` and the gate job "skipped".
5. On the first red run, the step "failure attribution" runs, puts the table in the job summary and uploads
   `gate-attribution-...`. Check the re-run's executable paths (`D:\a\rcfd\rcfd\target\release\deps\...`) and `GATE_LAST_GREEN`.
6. For a dispatch with `open_issues: true` after a red gate: `gh run download` of an artifact from the same, still-running workflow
   run, and `gh issue create` with `issues: write`.
7. For a dispatch with `bisect: true`: probe worktrees under `RUNNER_TEMP` building into the shared `target`, within
   `bisect_minutes` and inside the attribution step's 40 min.

Open questions, each taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation):

- Q1: should a dispatch on a branch other than main count as green for the skip? Options: (a) only main counts (taken; the
  query filters `branch=main`), (b) any branch. Recommendation: (a).
- Q2: should the index rank its ADRs (the registering ADR first)? Options: (a) list all citing ADRs with their evidence (taken),
  (b) add a `primary` ADR (the item ADR from E1, else the lowest-numbered citing ADR). Recommendation: (b), as a follow-up.
- Q3: if the attribution step times out because the job is near its 340 min, should it move to a separate job that downloads
  the logs? A separate job has no build, so it could not re-run or bisect. Recommendation: keep it in the gate job. The logs are
  uploaded before it starts.

## For the lead: roadmap entries

Proposed status and note of F-723:

> Proposed status: done.

> Test process. Requested by the maintainer 2026-11-16. DONE. ADR 0653, 2026-11-16: the full gate runs daily in GitHub Actions and every failed test is attributed to ADRs and merges. Cause of the failed scheduled runs of 2026-11-08 and 2026-11-15: cargo fmt --all hands every target root (63802 characters on the runner) to one rustfmt process, above the Windows limit of 32767 (os error 206, reproduced locally); the fmt step now runs cargo fmt -p per workspace member (largest 20031 characters), with the same set of files checked. gate.yml: cron daily 02:00 UTC on main, workflow_dispatch kept; a decide job (actions: read) skips a scheduled run when the last run whose full gate, windows-latest job passed on main tested the same SHA, and runs the gate when the query fails. After a failed test step, scripts/gate_attribute.py re-runs each failed test once (flaky when it passes), looks it up in the test-to-ADR index, and lists the first-parent commits since the last green gate that touched the test's file or crate (direct) or a dependency's sources (dependency), with their F-items and ADRs. It can bisect over those commits (input bisect, time budget bisect_minutes), and it writes gate-out/attribution.json, a job-summary table and, with input open_issues, one issue per non-flaky failure from a separate job with issues: write. scripts/adr_test_index.py finds tests statically (501 of 501 binaries and 99.0 % of the rows of a real gate listing) and maps them by file-name item, ADR and roadmap citations of the file or function, and module path: 1053 of 1134 integration-test binaries (92.9 %) and 6986 of 7472 tests (93.5 %) map to an ADR, and 20 of 20 sampled binaries have a correct ADR. A dry run on the real failure of gate 42083992758 names ADR 0181 and its commit among 11 candidates. docs/test-process.md section 7 gives the per-merge check set (fmt and clippy of the touched packages, the ADR's tests from --adr, the touched crates' --lib tests, baseline_hashes, docs_site). 22 Python unit tests and one end-to-end test; 10 of 10 mutations caught. Not covered until the first GitHub run: the decide job's API permissions, the skip on a real schedule, the attribution, bisect and issues paths on the runner.

> Follow-up proposed: First GitHub runs of the daily gate (ADR 0653 checks on the runner). Deps F-723. Acceptance: Every limit not stated here is registered in the ADR before the run. The first scheduled run after the push runs the gate with a non-empty last-green SHA and passes fmt with one line per package; a scheduled run with no push since a green run is skipped; a dispatch on a commit with one failing test (a test branch that is not merged) produces attribution.json with that test, its re-run, its ADRs and a candidate list, the bisect with input bisect finds the commit that broke it within bisect_minutes, and input open_issues opens one issue and no duplicate on a second run.

> Follow-up proposed: Wave-tag mapping and a primary ADR in the test-to-ADR index. Deps F-723. Acceptance: Every limit not stated here is registered in the ADR before the run. The index maps wave-named test files (w34a, wave17a, w48a) to the ADRs named after their wave, and each test gets a primary ADR (the registering ADR); the integration-test binaries left unmapped in ADR 0653 addendum 2 (81) fall below a registered number; a registered sample of primary ADRs is checked by reading.

## Amendment 3 (2026-11-18): the full gate in parallel shards, written before its code

By Claude. It carries out the item "the full run split into parallel jobs" of docs/process/documentation-process-review.md.

**Why.** The test step of the gate at 205889bf took 3 h 28 min (build 19 min, tests 3 h 09 min) of a 290 minute limit,
and the run of 2026-11-17 ran out of time. `scripts/gate_test.py --shard K/N` exists since 2026-11-18 and is not used.

**Defect found while reading, fixed here.** The split deals the tests to the shards in the order of the job list. Tests
without a recorded time keep the order of the binaries, which is the order in which cargo reports them, and that
depends on which build finished first. Two machines can therefore split differently, and a test could run twice or
not at all. In shard mode the job list is now ordered by recorded seconds (longest first), then binary name, then the
manifest path of its crate, then test name. Without `--shard` nothing changes.

**Design.**

- D7. A dispatch input `shards` (1 to 8, default 1). A scheduled run uses 1, so the daily gate is unchanged until the
  lead changes that default after a green sharded run. With 1, job names, artifact names and commands are as before.
- D8. The job `decide` gives the list of shards and the id of the newest completed gate run on main that has test
  times for the chosen runner. Every shard downloads the times of that one run, so all shards split by the same file.
  If there is none, every shard splits by count.
- D9. The job `gate` is a matrix over the shards. A shard runs fmt, clippy and the feature isolation as before, then
  `gate_test.py --shard K/N --order <times> --timeout-min 3600`. The last option keeps every test's time limit at the
  3600 s it has today, where no order file is given. The test-hooks step runs in shard 1 only. Logs and the
  attribution are per shard, in artifacts named with `-shard-K`. The job keeps its twelve steps.
- D10. `gate_test.py` in shard mode prints `gate_test: shard K/N runs m of M tests`. A job `complete` (sharded runs
  only) downloads the logs of all shards and runs `scripts/gate_shards_check.py`: the shards are exactly 1 to N, all
  give the same M, the sum of m is M, and no test is in the times of two shards. A shard that did not finish makes it
  fail. This is the check that no test was left out by the split.
- D11. `issues` reads the attribution of every shard.
- Cost: each shard builds the workspace (19 min on the Windows runner), so N shards use about N times 19 minutes more
  runner time than one job, for a wall time of about 19 min plus a share of the tests.

**Checks.**

- H1 (unit, `scripts/test_gate_shards.py`): with the binaries given in two different orders and the same times, the
  shard of every test is the same; over the N shards every test occurs once. Control: with the order key removed
  (the old sort), the two splits differ for tests without a record.
- H2 (unit): `gate_shards_check.py` accepts a complete set and fails, with a message naming the reason, on each of: a
  missing shard, two different M, a sum below M, one test in two shards.
- H3 (workflow text, same file): the matrix comes from `decide`; `--shard` and `--timeout-min 3600` are passed only
  when shards > 1; the hooks step is limited to shard 1; the artifact names carry the shard; `complete` needs `gate`
  and runs only when shards > 1; with shards = 1 the expressions give today's job and artifact names. The existing
  checks W1 to W3 of `scripts/test_gate_hooks_f761.py` pass unchanged.
- H4 (one dispatched run of the branch with 4 shards on windows-latest): `complete` is green; the sum of passed, failed
  and ignored over the shards is reported beside the last unsharded gate; wall time and runner minutes are recorded.
  A failure of a test in that run is a result of the tests, not of this amendment, and is reported as such.

**Not changed.** Which tests run, their limits, the attribution rules, the daily schedule.
