# Contributing to rcfd

This page is for anyone who wants to submit a change to rcfd. It sets out the
clean-room policy you must agree to, then the commands and rules for preparing
a change. For what rcfd is and its current limits, read
[README.md](README.md) first; to find work, see "How you can help" in
[the roadmap overview](docs/roadmap-overview.md).

rcfd is licensed under either of Apache-2.0 (`LICENSE-APACHE`) or MIT
(`LICENSE-MIT`) at your option. Every contribution is licensed as MIT OR
Apache-2.0, without additional terms.

## Clean-room policy (mandatory)

rcfd is a clean-room implementation. By submitting a change you confirm that:

1. You used no proprietary code or text. You did not copy, translate,
   decompile, disassemble or otherwise derive anything from proprietary CFD
   software (source, binaries, installers, help files, UI strings,
   undocumented file formats, or its documentation text). Do not decompile,
   disassemble or inspect vendor binaries. Do not use vendor manuals, help
   pages or theory guides (including unofficial copies) as a source for
   anything, not even as a secondary check of a value, and do not add vendor
   binaries, code or documentation text to the repository.
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

Declare provenance in the pull request: for every algorithm, constant, data
file or table you add, name the published source (or say it is derived from
first principles), and say where any data came from and under which licence.
Mark a value you could not check against its source as UNVERIFIED. The
project's own record of what was consulted while it was built is
[docs/provenance.md](docs/provenance.md).

## Sign-off (DCO)

Every commit must carry a `Signed-off-by: Your Name <email>` line
(`git commit -s` adds it). By signing off you certify the Developer
Certificate of Origin, version 1.1, as published at developercertificate.org:
in short, that you wrote the change or otherwise have the right to submit it
under the project's licence. Your contribution is licensed as MIT OR
Apache-2.0.

## How to make a change

Run these commands before you submit:

* `cargo fmt --all`
* `cargo clippy --workspace --all-targets -- -D warnings`
* `cargo test --workspace --release` (about an hour; debug mode is much
  slower). While you work, test only the crates you changed with
  `cargo test --release -p <crate>`. The project's gate script,
  `python scripts/gate_test.py`, runs the same release tests with more of
  them at once ([docs/test-process.md](docs/test-process.md)).

These rules apply to the code itself:

* Every roadmap item needs unit tests that exercise its acceptance criteria.
* Keep crates small and focused; see `docs/architecture.md` for the allowed
  dependency directions (the graph must stay acyclic).
