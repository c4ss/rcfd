# Contributing to rcfd

rcfd is licensed under the AGPL-3.0 (see [LICENSE](LICENSE)) and is also offered under a
[commercial licence](COMMERCIAL-LICENSE.md). By submitting a change you confirm it is yours to submit and you grant the
maintainer a perpetual, worldwide, royalty-free right to distribute it under the AGPL-3.0 and under commercial licences,
including future versions of either. For what rcfd
is and its current limits, read [README.md](README.md); to find work, see "How you can help" in
[the roadmap overview](docs/roadmap-overview.md).

## Clean-room rule

rcfd is a clean-room implementation. Do not copy, translate, decompile or otherwise derive anything from proprietary CFD
software (source, binaries, help files, manuals, undocumented formats), and do not add vendor code or text to the
repository. Take algorithms, correlations, coefficients and benchmark data from open literature (textbooks, papers, public
standards and file-format specifications) and cite the source in the doc comment, for example `// Patankar (1980), Eq. 5.62`.
Knowing that a commercial tool has a feature is fine; how it works inside is not a source. If you cannot name a public source
for a non-trivial algorithm, say so in the PR.

## Before you submit

* `cargo fmt --all`
* `cargo clippy --workspace --all-targets -- -D warnings`
* `cargo test --workspace --release --lib --bins` (what CI runs)
* `cargo test --release -p <crate>` for each crate you changed, to run its integration tests

The full suite takes over an hour; the maintainer runs it (`python scripts/gate_test.py`, see
[docs/test-process.md](docs/test-process.md)). Roadmap items need unit tests for their acceptance criteria, and the crate
dependency graph must stay acyclic (see [docs/architecture.md](docs/architecture.md)).
