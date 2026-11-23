---
type: entity
tags: [process, script, testing]
sources: [doc-process-lead-process, doc-process-documentation-review, doc-process-handover-2026-11-19, adr-thread-process-measures-and-roadmap-form, adr-thread-daily-gate-and-edit-lock, doc-test-process]
created: 2026-11-22
updated: 2026-11-23
status: draft
---

# Push script (scripts/push_main.py)

**What it is:** the lead's merge-to-push script, "the one list of per-merge checks" (test-process.md section 7.4) ([[doc-process-lead-process]]).
**Where:** [scripts/push_main.py](../../scripts/push_main.py), tests `scripts/test_push_main.py` with committed mutation patches `scripts/test_push_main.*.mutation.patch`.

## Details

- **Order** after a local merge: regenerate the generated files; run the script tests when `scripts/` is touched; fmt and clippy of touched packages; the ADR's registered tests; `baseline_hashes`; `docs_site` when `docs/` is touched; the process report's checks; the tests the diff selects, up to 30 minutes of recorded run time with a recorded build figure per binary; the committed mutation patches (expecting a failure); `npm test` in `apps/studio-web` when touched. Then it commits the generated files with the ledger line, pushes, and starts the selected tests that did not fit in the background ([[doc-process-lead-process]]).
- It writes no roadmap status; the lead does, afterwards ([[doc-process-lead-process]]).
- A failing check is run two further times (flaky if it then passes); one that fails every time stops the run and nothing is pushed; a stopped run restarts with the same command line without repeating what passed (`--fresh` repeats all) ([[doc-process-lead-process]]).
- `--run --base <commit>` runs one selection over several merged branches; `--gate-line` writes a full-run line; `--mutants --run` runs cargo-mutants on the changed Rust lines (weekly, in a worktree); F-863 options record the process measures ([[doc-process-lead-process]]).
- Selection rule (I11): always the item's registered tests, baseline_hashes, docs_site and lints; then the touched crates' tests, then dependants'; recent CI failures first, then shortest recorded time first. "The 30 minutes is a cap, not a target": the median summed test time of an item over 194 agent logs was 17 seconds ([[doc-process-documentation-review]]).
- History: written 2026-11-18; `--run`, `--push`, `--left-out-run` and `--mutants --run` had never run as of 2026-11-19; PR 24 (F-848) was proposed as the first real case ([[doc-process-handover-2026-11-19]]). The first `--mutants` trial (cargo-mutants 27.1.0, merge of ADR 0751): 7 mutants in 36 minutes, 5 caught, 1 missed, 1 timed out ([[doc-process-lead-process]]).

- **F-851 (ADR 0752, done):** a failed check is run two further times, and a test that then passes is named as flaky in the ledger line. A stopped run resumes. Build time is counted at 1.8 s per selected binary. `--base` takes several merges as one diff, and `--gate-times` reads the newest gate times. The change was merged "by the changed script itself, its first real run". F-860 (todo) carries three further changes: commits named in the left-out run, the ADR's tests charged to the budget, and selection by changed file ([[adr-thread-process-measures-and-roadmap-form]]).
- A finding from ADR 0751: the ledger's item field read "F-002" out of the branch name `...-f2-...`; it was corrected to F-855 ([[adr-thread-daily-gate-and-edit-lock]]).
- **F-860 (ADR 0776, done 2026-11-23):** a left-out line records `checkout_start` and `checkout_end` and, when they differ, `checkout_moved=yes`; the tests the index maps to the merge's ADRs are charged to the budget first, never cut, and the dry run prints their recorded minutes (26.7 of 30 on the merge of ADR 0748); inside a touched crate the binaries named after a changed source file (the stem, or the stem and `_`) are selected first. 10 new unit tests fail before and pass after; the 77 existing ones pass unedited ([[adr-thread-process-measures-and-roadmap-form]], [[doc-test-process]]).

## Related

- [[process-ledger]]
- [[process-report]]
- [[gate]]
- [[lead]]
