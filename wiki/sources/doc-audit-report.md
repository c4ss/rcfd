---
type: source
tags: [audit, model-output, history]
sources: []
origin: docs/audit-report.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: stable
---

# rcfd independent audit report (2026-10-14)

**Origin:** [docs/audit-report.md](../../docs/audit-report.md) at `origin/main` 2e94e5ef
**Kind:** audit record of one date and commit (model output)

> [!caution] Unreviewed model output
> Every claim on this page is drawn from a model-run audit (Claude Sonnet 5.5, 2026-10-14, `main` at `ff530c2`, 31 commits). The report says: "This report is model output; every number below was read from the repository or from a command run during the audit, but none of it has been reviewed by a human." It is a record of that date; for current item status read the [[roadmap]].

## Summary

> [!caution] Unreviewed model output
> The audit read roadmap.json, ROADMAP.md, reference-data.md, the ADRs and the BFS findings; diffed roadmap.json and every test file across all 31 commits; compared three pre-commit roadmap snapshots kept outside the repo; read the assertions behind each done/partial item; ran `cargo test --workspace --release`, fmt, clippy and the ignored heavy tests. "No vendor software, install or vendor material was read." Nothing in the repo other than the report was changed.

## Key points

> [!caution] Unreviewed model output
> All points below are the audit's claims as of 2026-10-14, not checked by a human.

- **Git history cannot prove "no criterion was changed":** the first commit (`f71f6e2`, 2026-10-13) is a squashed import of waves 1-6; across all 31 commits every item's `acceptance` text is byte-identical to the first commit; earlier edits are known only from the repo's self-disclosures, which were "in every case I could check, present and candid".
- **Tests:** 458 passed, 0 failed, 16 ignored, in 83 test suites; every ignored test that was run passed.
- `cargo clippy --workspace --all-targets -- -D warnings` failed only because of an untracked, git-excluded local helper (`crates/rcfd-api/examples/baseline_hash.rs`).
- **Done items not demonstrated by their own acceptance:** F-073 (1M-cell, 2 s render never measured), F-027 (criterion says V-cycles; evidence uses W-cycles, 14 of a limit of 15, ignored test), F-001, F-002, F-004 (criteria name CI runs and a review that had not happened).
- **Hash-identity claims not reproducible:** the hash tool was a "throwaway, not committed" example excluded via `.git/info/exclude`, and no baseline hash values were committed. (The later `crates/rcfd-api/tests/baseline_hashes.rs` of [[bit-identical]] is the committed gate; that link is the wiki's reading, not the audit's.)
- **Counts:** 53 done / 21 partial / 40 todo of 114 (63.5 weighted, 55.7 %); with the five proposed corrections (F-001, F-002, F-004, F-027, F-073 to partial) 48 / 26 / 40 (61.0, 53.5 %).
- **Edited or replaced criteria (15 rows):** e.g. F-111 IAPWS tolerance widened 1e-8 to 1e-7 (disclosed, "physically justified"); F-111 Sutherland air viscosity asserted at 1.5 % against a pre-registered 1 %; F-122's absolute bound replaced by a refinement ratio of at least 1.7; F-061's residual assertion deleted after an amendment; F-042's reattachment length never asserted; F-031's original criterion restored by changing the wall-pressure construction.
- **ROADMAP.md vs roadmap.json drift:** notes of 33 items differ; statuses, titles, acceptance and dependencies agree for all 114. Recommendation: generate the markdown from the JSON (done later, see [[roadmap]]).
- **Vendor-derived content:** no vendor code, text or data in the repo; the roadmap notes of 23 items used vendor-specific identifiers (later reworded generically, per the report's own later edit) ([[clean-room]]).
- **Not checked:** hosted CI, Linux builds, ParaView, glTF validator, any real local model server, any GUI appearance; whether reference values typed from memory are correct.

## Contradictions with the wiki

> [!caution] Unreviewed model output
> The audit's counts (114 items) are of 2026-10-14 and are superseded by later counts on [[roadmap]] (786 items on 2026-11-18). Not a contradiction, a dated record.

## Pages this source informs

- [[roadmap]]
- [[roadmap-status]]
- [[bit-identical]]
- [[clean-room]]
- [[known-miss]]
