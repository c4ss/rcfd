---
type: entity
tags: [interface, python]
sources: [doc-python, doc-run-control, doc-studies]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# rcfd Python package (the Python driver)

**What it is:** A pure-Python package that drives the `rcfd` command line to script cases, runs and studies; not an in-process extension (F-083 `partial`) ([[doc-python]]).
**Where:** `python/rcfd/`; tests `python/tests/run_tests.py` via `crates/rcfd-cli/tests/python_w20c.rs` ([[doc-python]]).

## Details

- Calls `rcfd` with argument lists (no shell); validation and computation stay in the Rust binary, so Python, `rcfd run --set` and `rcfd check` agree by construction ([[doc-python]]).
- API: `Case`, `RunResult`, `rcfd.study(...)` with `rcfd.List` / `rcfd.Linspace`, `StudyResult`, `rcfd.schema()`; errors `SettingsError`, `RcfdError`, `StudyError` ([[doc-python]]).
- Drives `run`, `check` and `study` only ([[doc-run-control]], [[doc-python]]); a study's exit codes are those of [[doc-studies]].
- Reads no environment variable except `RCFD_BIN` ([[doc-python]]).
- PyO3 route not delivered because "the workspace forbids `unsafe_code` and PyO3's macros expand to unsafe code" (ADR 0034 D5) ([[doc-python]]).

## Related

- [[parametric-study]], [[exit-codes]], [[rcfd-cli]]
