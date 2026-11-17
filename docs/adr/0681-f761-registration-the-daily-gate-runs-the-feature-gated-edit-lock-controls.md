# ADR 0681: F-761 registration: the daily gate runs the feature-gated edit-lock controls

Status: **REGISTERED 2026-11-17, written before any code of F-761 and before any run of its checks.** Not yet accepted by the
maintainer. Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-761 (deps F-755, F-723), proposed by [ADR 0676](0676-f755-registration-a-forced-control-of-the-edit-locks-starvation-and-of-a-lock-left-behind.md)
addendum 1: its test file `crates/rcfd-project/tests/edit_lock_f755.rs` starts with `#![cfg(feature = "test-hooks")]`, so the
workspace step of the daily gate ([ADR 0653](0653-f723-registration-daily-full-gate-with-failures-attributed-to-adrs.md)) builds it
as an empty binary and its seven controls never run there. Number 0681 given by the lead. Branch `f761-work` from fcb0e65e,
worktree `w49i-work/f761`, scratch `w49i-work/f761-scratch`.

House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here; each registered run made once; existing tests not edited;
no Rust file, hash table or golden touched; stdlib Python only; clean-room (a CI step order and cargo's documented feature
resolution, nothing else); roadmap files not edited. Choices the maintainer would normally be asked about are taken on the
maintainer's standing instruction of 2026-11-11 (take the recommendation), except where listed as open.

## What F-761 asks

> Every limit not stated here is registered in the ADR before the run. The daily gate runs cargo test -p rcfd-project --release --features test-hooks --test edit_lock_f755 as its own step with its failures attributed to ADR 0676, without enabling the feature in any other build step (cargo tree of the workspace step shows no test-hooks); the gate's other results unchanged.

## What was read

- `.github/workflows/gate.yml`: steps fmt, clippy, `tests` (`python scripts/gate_test.py --tsv gate-out/times.tsv --heartbeat 300`,
  stdout to `test.log`), summary, upload logs, attribution (`if: failure() && steps.tests.outcome == 'failure'`), upload attribution.
- `scripts/gate_test.py`: its positional arguments are the cargo package selection (default `--workspace`) and are passed to
  `cargo test <select> --release --no-fail-fast --no-run --message-format=json-render-diagnostics`; after the tests it runs
  `cargo test <select> --release --no-fail-fast --doc`. `cargo test --doc` together with `--test <name>` is an error in cargo
  (the doc-test target and a named target are exclusive), so the feature step cannot use the script unchanged.
- `scripts/gate_attribute.py`: reads one `--tsv` and one `--log`; failures are keyed `(binary stem, test)`, looked up in the index
  (`tests[].adrs`, `.items`; since ADR 0679 also `primary_adr`, which the attribution does not copy); its bisect probe
  `cargo_probe` builds `cargo test -p <crate> --release --test <target>` with no feature, so a failure of a feature-gated
  binary would compile to zero tests in a probe.
- One run of the unchanged `scripts/adr_test_index.py` at fcb0e65e: the 7 tests of binary `edit_lock_f755` have `adrs`
  0135, 0669, 0676 and `primary_adr` 0676 (evidence E0, the registering ADR). So the mapping to ADR 0676 exists already; what is
  missing is that the failures reach the attribution and that it names 0676 as the primary ADR.
- `crates/rcfd-project/Cargo.toml`: `test-hooks = []`, `default = []`; no other crate of the workspace names the feature
  (grep over `crates/*/Cargo.toml`).
- `python -m unittest discover -s scripts` at fcb0e65e: **85 tests, OK, 4 skipped** (24 s). This is the baseline.

## Design

**D1, one more step in `gate.yml`, after `tests`.** Step id `hooks`, name "tests with the test-hooks feature (ADR 0681)":
`python scripts/gate_test.py --no-doc --tsv gate-out/times_hooks.tsv --heartbeat 300 -p rcfd-project --features test-hooks --test edit_lock_f755 > gate-out/test_hooks.log`
with the same `tee` into `progress.log` (append). It runs `if: always() && steps.tests.outcome != 'skipped'`, so it runs
when the workspace tests failed and not when an earlier step (fmt, clippy) already stopped the job. Its cargo command lines are
`cargo test -p rcfd-project --features test-hooks --test edit_lock_f755 --release --no-fail-fast --no-run ...` and the per-test
executions of the built binary, which is the acceptance's `cargo test -p rcfd-project --release --features test-hooks --test
edit_lock_f755` with the build and the run separated and the test run one process per test as in every gate step. `timeout-minutes: 30`.

**D2, `gate_test.py --no-doc`** (additive; default off, so every existing invocation is unchanged): skip the closing
`cargo test --doc`. The doc-tests of rcfd-project run in the workspace step, without the feature.

**D3, isolation of the feature.** Cargo unifies features within one resolve, one invocation. The workspace step is one
invocation `cargo test --workspace`, in which no member enables `test-hooks`; the feature step is a different invocation. In the
shared `target` folder cargo gives a library built with a different feature set a different metadata hash and file name, so the
two builds sit side by side and neither overwrites the other. A new cheap step "feature isolation (ADR 0681)" runs before `tests`:
`cargo tree --workspace --locked -e features` and fails if the text `test-hooks` occurs in it, so a later change that enables the
feature from another crate or from the workspace manifest turns the gate red at once. The existing build steps (fmt, clippy,
tests) are not edited.

**D4, attribution reads both logs.** `gate_attribute.py attribute --tsv` and `--log` accept several files (`nargs="+"`; one file
works as before); `read_failures` merges them, a build failure of either log is reported (joined by `; ` when both). New option
`--feature-binary BINARY=FEATURE` (repeatable) so the bisect probe of that binary builds with `--features FEATURE`
(`cargo_probe(entry, rec, features)`); the gate passes `--feature-binary edit_lock_f755=test-hooks`. Each failure record gets
`primary_adr` (from the index, null when absent) and the summary table shows it first and in bold in the ADR cell, only when the
test has one (so a record without it renders as before). The attribution step runs
`if: failure() && (steps.tests.outcome == 'failure' || steps.hooks.outcome == 'failure')`, with `--tsv gate-out/times.tsv gate-out/times_hooks.tsv`
and the same for `--log`; the step "summary" gets one more line for the hooks log (the existing awk over `test.log` is unchanged);
the upload steps already take the whole `gate-out/` folder.

**D5, documentation.** `docs/test-process.md` section 7 names the new step and its files. `docs/releasing.md` is not changed.

## Checks

| id | check | limit (fixed now) | estimate behind it |
|---|---|---|---|
| C1 | `gate.yml` parses as YAML (python, PyYAML if installed, else a stdlib check of the step list by a small parser) and lists the steps of job `gate` | parse ok; the step names fmt, clippy, feature isolation, tests, hooks, summary, upload logs, attribution, upload attribution in this order (9 `name:`-bearing steps after the 3 `uses:` setup steps) | by construction |
| C2 | the lines the diff of `gate.yml` removes | at most 6 (the attribution `if:`, its two command lines, the summary lines I extend) and no line of the fmt, clippy or tests steps | D1 to D4 |
| C3 | the new step's commands run locally through `gate_test.py --no-doc ... -p rcfd-project --features test-hooks --test edit_lock_f755` | exit 0; 7 test processes; 7 `ok` in `times_hooks.tsv`, all with binary `edit_lock_f755`; no `cargo test --doc` line | ADR 0676 addendum: 7 tests, each at most a few seconds |
| C4 | `cargo tree --workspace --locked -e features` | 0 lines with `test-hooks` | no crate names the feature |
| C5 | control of C4: `cargo tree -p rcfd-project --features test-hooks -e features` | at least 1 line with `test-hooks` | the check can see the feature |
| C6 | artefacts of the default build: `cargo test -p rcfd-project --release --lib --no-run --message-format=json`, before and after the feature step | every `compiler-artifact` has `features` without `test-hooks`; the executable path and its size and modification time are the same before and after | D3 |
| C7 | the feature build's artefacts | `features` contains `test-hooks` for the rcfd-project lib and the test binary, and the paths differ from the default build's | D3 |
| C8 | negative control: a scratch copy of the tree with a failing test (`assert!(false)`) appended to `edit_lock_f755.rs`, run through the C3 commands and `gate_attribute.py attribute --rerun` with both tsv/log arguments | exit of the step non-zero; `attribution.json` has exactly 1 failure, `binary` `edit_lock_f755`, `primary_adr` `0676`, `adrs` containing `0676`, `rerun.outcome` `FAILED`, `flaky` false; `attribution.md` row contains `**0676**` | D4 |
| C9 | control of C8: the same run with the original file | 0 failures in `attribution.json` ("No failed test.") | |
| C10 | bisect probe command for a feature binary | contains `--features test-hooks` with `--feature-binary edit_lock_f755=test-hooks`, and does not contain `--features` without it (unit test) | D4 |
| C11 | `python -m unittest discover -s scripts` from a clean worktree | all pass, at least 85 + 6 = 91 tests run, skipped count not above 4 | six new unit tests: `--no-doc` doc skip, multi-file `read_failures`, merged build failure, `primary_adr` in record and in the table cell, row unchanged without it, `cargo_probe` with and without features |
| C12 | defaults unchanged | the 85 existing tests pass without edit; `summary_md` of a record without `primary_adr` is byte-identical to the old rendering (unit test) | D4 |
| C13 | fmt and clippy | no Rust file is touched; none run | |

## Negative controls and planned mutations

- C8 and C9 are the controls of the attribution; C5 is the control of C4; the default-build artefacts (C6) are the control of the
  feature build's (C7).
- M1: `gate.yml` hooks step with `--features test-hooks` removed in a scratch copy: the step runs 0 tests (`edit_lock_f755` compiles
  empty), so C3's "7 test processes" is the check that catches it (limit 7).
- M2: the attribution step's `if:` without the `hooks` clause: a small test in `test_gate_attribute.py` reads `gate.yml` as text and
  requires `steps.hooks.outcome` in the attribution step's `if:`.
- M3: `read_failures` reading only the first tsv: the multi-file unit test fails.
- M4: `cargo_probe` ignoring the features: C10's unit test fails.

## Decision rule

Done if C1 to C12 are met. Partial if C8 or C3 is met only in part, or if the YAML structure cannot be checked as registered.
The real GitHub run is not made here (no workflow dispatch is allowed to this agent); it is left to the next daily gate, so the
status proposed cannot be more than done-by-local-evidence, stated as such.

## Limits (what is not covered)

- The real runner: Windows timing of the step, the cargo resolver on a clean target folder in CI, and the Actions expression
  semantics of `if:` are not run; they are checked by reading and by the YAML parse.
- Other feature-gated tests: only `edit_lock_f755` is named; a future gated file needs its own `-p/--features/--test` list (open
  question below).
- The workspace step's artefacts are checked at the crate of the feature (rcfd-project) with `--lib --no-run`, not by a whole
  workspace build, which the house rules do not allow here.
- Bisect over a gated binary is checked by the probe command text, not by running a bisect.
