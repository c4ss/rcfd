---
type: concept
tags: [process, testing]
sources: [doc-glossary, doc-readme, doc-contributing]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Gate (full check of a commit)

`cargo fmt --check`, `cargo clippy` with warnings as errors, and the whole test suite in release mode on a clean copy of the commit; over an hour; runs on `main` in CI, not before each merge ([[doc-glossary]]).

## How rcfd uses it

- Run with `python scripts/gate_test.py`; a merge is first checked by the tests its changes select ([[doc-readme]]).
- [[doc-roadmap-overview]] says the whole suite runs as a gate every day.

## Related

- [[bit-identical]]
- [[known-miss]]
