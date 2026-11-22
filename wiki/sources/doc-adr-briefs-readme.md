---
type: source
tags: [process, briefs]
sources: []
origin: docs/adr/briefs/README.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: stable
---

# Briefs (docs/adr/briefs/README.md)

**Origin:** [docs/adr/briefs/README.md](../../docs/adr/briefs/README.md) at `origin/main` 2e94e5ef
**Kind:** folder README (form of a brief)

## Summary

A brief is what the lead tells an agent when it issues a piece of work. Each is kept as a file, written and committed by the lead on main before the agent starts. "It is the record that the work was issued, of what the agent was told to read, and of what produced the result: the rules version and the model." The registration cites its brief by path (agent rules, rule 1). See [[brief]].

## Key points

- File name `<branch>.md`, with any `/` in the branch name written as `-`; one file per dispatched branch, in both streams.
- Header lines read by `scripts/process_report.py` by prefix: `Branch:`, `Items:` (F-ids or "none"), `ADR:`, `Rules version:`, `Model:`, `Issued:` (YYYY-MM-DD), `Size (lead):` with five values (outcome; further registrations; heavy runs; questions for the maintainer; bytes to read).
- Two sections: `## Reading list` (one path per line with its size in bytes) and `## Task`.
- An abandoned dispatch gets one appended line: `Closed without merge (<YYYY-MM-DD>): <why>`. "A brief with no such line whose branch no ledger line names is work that is issued and not merged."
- Briefs are not ADRs: they have no number and the test index does not read them.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[brief]]
- [[lead]]
- [[process-report]]
- [[process-ledger]]
