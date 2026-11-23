---
type: concept
tags: [process, testing]
sources: [doc-glossary, doc-process-agent-rules]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Bit-identical defaults

A change leaves the existing default results unchanged to the last bit, checked by stored result hashes ([[doc-glossary]]).

## How rcfd uses it

- New behaviour is opt-in; `crates/rcfd-api/tests/baseline_hashes.rs`, hash tables and golden files are never edited or re-recorded by agents ([[doc-process-agent-rules]], rule 3).
- Exception approved 2026-11-18: purely additive golden edits, re-recorded by the lead; never `baseline_hashes.rs` ([[doc-process-agent-rules]]).

## Related

- [[gate]]
- [[registration]]
