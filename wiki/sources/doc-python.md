---
type: source
tags: [interface, python]
sources: []
origin: docs/python.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Python access

**Origin:** [docs/python.md](../../docs/python.md) at `origin/main` 2e94e5ef (F-083, ADR 0034)
**Kind:** reference page

## Summary

The `python/rcfd` package scripts cases, runs and studies by driving the `rcfd` command line (argument lists, no shell). It is not an in-process extension; F-083 stays `partial`. See [[rcfd-python]].

## Key points

- Pure Python (standard library; numpy optional for fields). "The Rust binary does all validation and all computation, so the Python object, `rcfd run --set` and `rcfd check` agree by construction."
- Binary lookup `rcfd.find_binary()`: argument, `RCFD_BIN`, then `target*/release` or `debug`. No wheel; use from a checkout.
- API: `Case.load` / `from_text` / `from_dict`, `case.set` (raises `SettingsError(.path, .message)`), `case.get`, `case.check()`, `case.run()` -> `RunResult`, `rcfd.study(...)` -> `StudyResult`, `rcfd.schema()`.
- Non-zero run-control codes are outcomes in `RunResult.exit_code`, not exceptions; a case that cannot start raises `RcfdError`.
- Fields are read from the ASCII `solution.vtu`; settings sent as `--set path=<JSON>` ("Python floats round-trip exactly").
- Tests `python -I python/tests/run_tests.py`, run by `crates/rcfd-cli/tests/python_w20c.rs` (prints `SKIPPED` without Python).
- Not delivered: the PyO3 route (the workspace forbids `unsafe_code` and PyO3's macros expand to unsafe code; PyO3/maturin not in the offline cache; "the only Python here is a Microsoft Store 3.9 install"), service access, live field access, commands other than run/check/study.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[rcfd-python]], [[parametric-study]], [[exit-codes]]
