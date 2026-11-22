---
type: source
tags: [process]
sources: []
origin: docs/process/agent-rules.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: stable
---

# Process: README and agent rules (v5)

**Origin:** [docs/process/agent-rules.md](../../docs/process/agent-rules.md) at `origin/main` 2e94e5ef
**Kind:** governing rule set for every agent (version 5, 2026-11-21) + `docs/process/README.md`

## Summary

How work is issued, recorded, checked and merged. `agent-rules.md` is the rule set for every agent in both streams (campaign and Studio); `lead-process.md` is the lead's side (not ingested in P0). Replaces `w49i-work/AGENT_RULES.md` (2026-11-12).

## Key points

- Rule 1: [[registration]] before code — `docs/adr/<n>-<item>-registration-<slug>.md`, committed alone first, with design, checks table, negative controls, mutations, size, decision rule.
- Rule 2: limits never change after a run; a miss becomes an ignored `*_known_miss` test ([[known-miss]]); each registered run is made once; parameters come from derivation, not search.
- Rule 3: defaults stay [[bit-identical]]; never edit or re-record `baseline_hashes.rs`, hash tables or golden files — except pre-approved 'additive golden edits' (2026-11-18), which the lead re-records; `baseline_hashes.rs` is never in that class.
- Rule 4: [[clean-room]] from sources actually read, with a locator; citations from memory only when no source could be read, and said so. New reference values go in `docs/reference-data.d/<ADR>-<item>.part`; `reference-data.md` is generated.
- Rule 5: no new third-party crate, no unsafe code, acyclic [[crate-graph]]. Rule 6: take the recommended option on open questions under the standing instruction of 2026-11-11, except ones that change defaults/goldens/scope or need unobtainable sources.
- Rule 7: run targeted checks in release, fmt and clippy per crate, and `python scripts/process_report.py --check`; never kill a cargo process that is not yours.
- Rule 8: 'Addendum 1: results' with every check measured; partial when a clause misses; fixed heading 'Open questions for the maintainer'.
- Rule 9: only the lead writes `docs/roadmap.json`, `ROADMAP.md`, `roadmap-overview.md`, `docs/adr/index.md`, `docs/process/ledger.tsv`; agents propose entries as `docs/roadmap/proposals/<ADR>-<item>.json`.
- Rules 11-18: plain commits, no trailers, no push/merge by agents; test headers name their ADR/F-item; follow-ups of a miss register under the parent item; checks table has a kind (unchanged default output / code verification / solution verification / validation) — a claim of agreement with a measured case needs a passing validation row; every limit cites a source (`docs/process/tolerances.md`, a reference, or the parent); one committed mutation patch; a size line; ADR file names carry one kind word and ADRs are append-only.
- Process tooling: `scripts/process_report.py` (`--write` regenerates the ADR index `docs/adr/index.md`, overview numbers and area lists; `--check` in CI; `--weekly`); `scripts/push_main.py` for the lead's merge/push; `docs/process/ledger.tsv` records merges and full runs; agent briefs live in `docs/adr/briefs/`.
- Verification = check against equations or the code's earlier output; validation = check against a measured case.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[registration]]
- [[known-miss]]
- [[bit-identical]]
- [[clean-room]]
- [[crate-graph]]
- [[adr]]
- [[roadmap]]
- [[implementation-agent]]
- [[lead]]
- [[brief]]
- [[tolerance-class]]
- [[maintainer-questions]]
- [[process-report]]
- [[process-ledger]]
- [[push-main]]
- [[work-streams]]
- Lead side: [[doc-process-lead-process]]; basis: [[doc-process-documentation-review]]; what is in force: [[doc-process-framework-reconciliation]]
