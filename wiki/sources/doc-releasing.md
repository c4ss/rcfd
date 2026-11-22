---
type: source
tags: [release, process, ci, gate]
sources: []
origin: docs/releasing.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Versions, checks and releases

**Origin:** [docs/releasing.md](../../docs/releasing.md) at origin/main 2e94e5ef
**Kind:** repo doc: version policy, what each automatic check proves, and the release procedure. Written 2026-10-27, before the first release.

## Summary

"No version has been tagged yet; 0.1.0 is prepared in `CHANGELOG.md`." One version number covers the whole repository. A release is "a tag `vX.Y.Z` on a commit of `main` on which the full gate passed, plus a GitHub release whose notes are the `X.Y.Z` section of CHANGELOG.md"; it has no binaries, and a published tag is never moved or reused. The checks on a pull request do not run the integration tests in `crates/*/tests`, which hold every solver validation case; the `Full gate` workflow runs the non-ignored ones daily on `main` and on demand.

## Key points

- The version is set in four places that must agree: `[workspace.package]` in `Cargo.toml`, `python/rcfd/__init__.py`, and two Claude plugin manifests; `scripts/release_notes.py` fails if they differ from the tag.
- Before 1.0: MINOR goes up when a release can break something a user relies on (CLI options or exit codes, case files, MCP tools or service methods, UDF ABI, Python functions, output formats, "or a different numerical result from an unchanged case with default settings"); PATCH for everything else. The crates' Rust library API is not part of the promise (`publish = false`). "When [1.0] comes is not decided."
- Checks table: CI `fmt`, `clippy` (Linux, Rust 1.99.0), `unit tests` (tests inside `src/` only; not the integration tests, "about 80 % of the tests, more than 99 % of the test time"), type-check with Rust 1.90, dependency licences (`deny.toml`), script self-tests, `studio-web`; the `Full gate` workflow (not the about 660 `#[ignore]` tests, nor CUDA tests without a device); `Studio installers`; weekly `Studio Linux test (Docker)`.
- GitHub switches off scheduled workflows in a public repository after 60 days without a push; start them by hand before a release.
- The full gate on a GitHub runner has not been timed; the maintainer's gate machine needs about 65 minutes for the test stage; the step stops after 290 minutes.
- Seven-step release procedure: history final; choose the number; release commit (four version places, `cargo update --workspace`, CHANGELOG heading); CI green; full gate on that exact commit; tag and push the tag; publish the draft from the `Release` workflow.

## Contradictions with the wiki

> [!warning] Contradiction
> The checks table and summary say the `Full gate` runs "daily on main" (ADR 0653), but the paragraph on GitHub's 60-day rule still says "so the weekly gate stops by itself in a quiet period". [[doc-test-process]] §7.1 describes a daily gate. Within the doc (likely a stale word).

## Pages this source informs

- [[gate]], [[doc-changelog]], [[doc-test-process]], [[bit-identical]]
