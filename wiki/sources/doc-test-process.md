---
type: source
tags: [gate, testing, process, ci]
sources: []
origin: docs/test-process.md
origin_commit: 09647baa
created: 2026-11-22
updated: 2026-11-23
status: draft
---

# The test and gate process: measurements and options

**Origin:** [docs/test-process.md](../../docs/test-process.md) at origin/main 09647baa (first read at 2e94e5ef; section 7.4, item 7, changed by ADR 0776)
**Kind:** repo doc: measured record of the gate (ADR 0127, ADR 0129), the daily CI gate with failure attribution (ADR 0653), per-merge checks and the Studio npm job.

## Summary

Measured on 2026-10-26 on the gate machine (16 logical processors, 96 GB, shared by up to six agents and two gate slots). The gate is `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` and the workspace tests in release, run on a `git archive HEAD` copy with a fresh target directory in one of two slots. `scripts/gate_test.py` runs each test as its own process: "At the same commit it cut the test stage from 103 min (47E gate) to 66.8 min (-35 %)", its list of tests and outcomes equals the original gate's, and it caught all three negative-control breaks. Since ADR 0653 the full gate runs daily in CI and failures are attributed to ADRs; merges run a reduced check list.

## Key points

- Per-test time limit (ADR 0129) frees a slot from a hung test; gate limits for a few long tests live in `scripts/gate_limits.tsv` (ADR 0740 re-run budget).
- `--jobs 5` cut the test phase by 13.2 % (64.1 to 55.7 min) under load, "below the registered 25 %, so the default stays 3".
- Run 1 numbers: 2093 tests, 10 578 test-seconds; the slowest single test took 969 s; "eight tests over 5 min take 36 % of all test-seconds".
- Tests that cannot fail (`scripts/limit_audit.py`, five classes i to v): 80 hits, 2 real weaknesses (`rcfd-project/tests/listing_budget.rs` L1 negative control; `rcfd-fv/tests/schemes_wave13.rs` T2 vacuous case). 12 non-ignored data-gated tests (GRI-Mech, public Fluent and PLOT3D files) always skip in the gate and count as passed, because their data is not in git. No test was edited; options are listed for the user.
- Mutation spot-check of registered limits (183 ids of ADRs 0115 to 0126, 12 drawn with `random.Random(1290127)`): "10 of 12 detected"; not detected: ADR 0123 S7 (cancel time 0.116 s against a 5 s limit) and ADR 0119 R3 (accepts any error containing "stall").
- Daily `Full gate` workflow at 02:00 UTC on `main` (ADR 0653); skipped when the commit already passed. fmt runs package by package because `cargo fmt --all` exceeds the Windows command-line limit (os error 206). Feature-gated test step for `test-hooks` (ADR 0681).
- Failed runs: `scripts/gate_attribute.py` re-runs failures within a budget and writes `attribution.json`; optional bisect.
- Test-to-ADR index (`scripts/adr_test_index.py`, evidence kinds E1 to E5): at d40cd665 maps 6986 of 7472 tests (93.5 %), 5424 of 5856 non-ignored (92.6 %).
- Per-merge checks (§7.4) via `python scripts/push_main.py --run`: fmt and clippy of touched packages; the ADR's registered tests; unit tests of touched crates; `baseline_hashes`; `docs_site` when docs change; `process_report.py --check`; diff-selected tests within a 30-minute budget; committed mutation patches (`*.mutation.patch` with `# expect-fail:`); `npm test` for the Studio. A failing command is retried twice (ADR 0752; passes are named flaky). Success appends a line to `docs/process/ledger.tsv`; a green full-gate line moves the local tag `verified`.
- Since ADR 0776 (F-860, 09647baa), §7.4 item 7: "The tests the index maps to the merge's ADRs (check 2) are charged to the budget first and never cut, and the plan prints their recorded minutes"; inside the group of the touched crates, the binaries named after a changed source file (the file's stem, or the stem and `_`) come first; a left-out run's `leftout` line "records the checkout's commit at the start and at the end, with `checkout_moved=yes` when they differ" ([[adr-thread-process-measures-and-roadmap-form]]).
- §7.5: sharded gate runs (`-f shards=4`, checked by `scripts/gate_shards_check.py`), a skip-pattern count, a left-out run, and `cargo mutants --in-diff` (not installed as of 2026-11-18; not run on this repository yet).
- §8: the Electron Studio is tested by npm in the CI job `studio-web` on Ubuntu 24.04 and Windows; "a green gate says nothing about the Studio".

## Contradictions with the wiki

None found. Consistent with [[gate]] and [[doc-changelog]] (the full gate runs on `main` in CI, not before each merge).

## Pages this source informs

- [[gate]], [[known-miss]], [[registered-limit]], [[verification-vs-validation]], [[doc-releasing]]
