---
type: source
tags: [process, clean-room]
sources: []
origin: CONTRIBUTING.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: stable
---

# CONTRIBUTING.md

**Origin:** [CONTRIBUTING.md](../../CONTRIBUTING.md) at `origin/main` 2e94e5ef
**Kind:** contribution policy

## Summary

Licence grant, the clean-room rule, the checks to run before submitting, and the registration-first rule for roadmap work.

## Key points

- Contributions grant the maintainer the right to distribute under AGPL-3.0 and commercial licences.
- [[clean-room]]: no copying, translating, decompiling or deriving from proprietary CFD software; cite open sources in doc comments (e.g. `// Patankar (1980), Eq. 5.62`); knowing a feature exists is fine, how it works inside is not a source.
- Before submitting: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --release --lib --bins` (what CI runs), and `cargo test --release -p <crate>` for changed crates. The maintainer runs the full [[gate]].
- Roadmap work starts with a [[registration]] ADR committed alone, fixing checks and numeric limits before code; rules in `docs/process/agent-rules.md` ([[doc-process-agent-rules]]).
- Roadmap items need unit tests for their acceptance criteria; the [[crate-graph]] stays acyclic.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[clean-room]]
- [[gate]]
- [[registration]]
- [[crate-graph]]
