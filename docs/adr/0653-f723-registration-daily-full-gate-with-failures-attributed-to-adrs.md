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
