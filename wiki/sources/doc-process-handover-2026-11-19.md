---
type: source
tags: [process, handover, lead]
sources: []
origin: docs/process/handover-2026-11-19.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: stable
---

# Handover to the lead (2026-11-19)

**Origin:** [docs/process/handover-2026-11-19.md](../../docs/process/handover-2026-11-19.md) at `origin/main` 2e94e5ef
**Kind:** handover note (one day), with maintainer answers of 2026-11-21

## Summary

Written by Claude at the end of the session that followed the merge of the process framework (main at `10ba1a34`). It lists what waited on six pull requests, findings not acted on, and who decides what. "It is a note of one day: when its lists are worked off, delete it."

## Key points

- **Six PRs**, all green on the 8 checks of ci.yml and unmerged (2026-11-19): 24 `f848-site-record-final` (F-848, ADR 0739, the only one with Rust code); 25 `reader-trial-i9` (trial of another kind of reader); 26 `gate-shards` (full gate in parallel shards, ADR 0653 Amendment 3 and Addendum 3); 27 `ledger-gates-1118` (two ledger lines: red gate at `205889bf`, green at `39269587`); 28 `sources-read-i10` (the "Sources read" addendum, batch 1 on six ADRs); 29 `structural-distillation`. A trial merge in order 24-29 conflicted nowhere; `process_report.py --check` 0 failures; all 17 `scripts/test_*.py` pass; Rust tests not run on the merged tree.
- **For the lead:** briefs to cite `docs/process/agent-rules.md` and its version; the first real `push_main.py` merge (`--run`, `--push`, `--left-out-run`, `--mutants --run` had never run); items unblocked by the 2026-11-18 decisions; the two tests at the per-test limit; shard default of the scheduled gate (1; four shards gave 2 h 32 min against 4 h 41 min for about 1.5 times the runner minutes); F-848 (failure seen once in two gates, not reproduced in 180 local runs; cause inferred); a flaky 2 ms timing test of F-456.
- **Findings not acted on:** 11 differences in citation batch 1 (no number used by the code shown wrong); 16 places both readers flagged in the I9 trial (one false sentence, ADR 0553 line 415; ten real and minor; five not defects); 3 stale structural gaps (F-607, F-578, F-541); 6 items whose gap says "The note does not say what is missing"; 13 done items whose note still opens with PARTIAL. "An ADR finding is corrected by an addendum of that ADR, never by an edit."
- **Draft item:** two immersed-solid tests within a factor of 1.2 of the 3600 s per-test limit (`immersed_solid_f219::a3_two_d_two_strouhal_number` took 2066 s, 3060 s and timed out at 3600 s; the `a2t_c1...` test of `immersed_unsteady_force_f346` 3230 s and 2880 s). Acceptance: each finishes in at most half its limit in three consecutive full gates on windows-latest, by a shorter set-up (route a) or a higher limit for these two only (route b); no numeric limit of ADR 0218 or ADR 0249 changes.
- **Scope questions and answers (maintainer, 2026-11-21: "Yes to both as recommended, go ahead"):** citation work is the weekly item of five citations, with batches only for ADRs whose numbers the code uses and that mark a value UNVERIFIED (batch 1 cost about 1.1 million tokens for six ADRs and 70 citations; the whole would be of the order of 50 million tokens); no fresh-reader rule, a reader only for an item that changes a default result or a golden. The same answer covered installing cargo-mutants 27.1.0 (installed 2026-11-21).

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[lead]]
- [[maintainer-questions]]
- [[sources-read-addendum]]
- [[gate]]
- [[push-main]]
