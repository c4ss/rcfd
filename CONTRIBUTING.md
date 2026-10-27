# Contributing to rcfd

This page is for anyone who wants to submit a change to rcfd. It sets out the
clean-room policy you must agree to, then the commands and rules for preparing
a change. For what rcfd is and its current limits, read
[README.md](README.md) first; to find work, see "How you can help" in
[the roadmap overview](docs/roadmap-overview.md).

rcfd is licensed under either of Apache-2.0 (`LICENSE-APACHE`) or MIT
(`LICENSE-MIT`) at your option. Contributions are accepted under the same
terms.

## Clean-room policy (mandatory)

rcfd is a clean-room implementation. By submitting a change you confirm that:

1. You used no proprietary code or text. You did not copy, translate,
   decompile, disassemble or otherwise derive anything from proprietary CFD
   software (source, binaries, installers, help files, UI strings,
   undocumented file formats, or its documentation text).
2. Your sources are textbooks and papers only. Algorithms, correlations,
   model coefficients and benchmark data come from publicly published
   literature (textbooks, peer-reviewed papers, public standards, public
   file-format specifications). Cite the source in the doc comment, for
   example `// Patankar (1980), Eq. 5.62` or `// Ghia, Ghia & Shin (1982)`.
3. Feature lists are not implementations. Knowing that a commercial tool
   offers a capability is fine; how it does it internally is not a source.
4. File formats are implemented only from public specifications or from
   files you are licensed to examine as black-box sample data you created.
5. Benchmark reference data must be from open literature with citation.

PRs that cannot state a public source for a non-trivial algorithm will be
asked to add one before merge.

## How to make a change

Run these commands before you submit:

* `cargo fmt --all`
* `cargo clippy --workspace --all-targets -- -D warnings`
* `cargo test --workspace`

These rules apply to the code itself:

* Every roadmap item needs unit tests that exercise its acceptance criteria.
* Keep crates small and focused; see `docs/architecture.md` for the allowed
  dependency directions (the graph must stay acyclic).
