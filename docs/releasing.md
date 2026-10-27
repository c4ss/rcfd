# Versions, checks and releases

Status: written 2026-10-27, before the first release. No version has been tagged yet; 0.1.0 is prepared in
`CHANGELOG.md` in the repository root.

This page is for the maintainer who cuts a release and for contributors who want to know what the automatic checks on a pull
request prove. It says what a version number means, what counts as a release, what each check covers and does not, and the steps to
cut a release. How the gate itself works and how long it takes is in [the test process](test-process.md).

## Version numbers

One version number covers the whole repository. It is set in three places, which must always agree:

* `version` under `[workspace.package]` in `Cargo.toml`; every crate inherits it (`version.workspace = true`), none sets its own;
* `__version__` in `python/rcfd/__init__.py`;
* `version` in `clients/claude-panel/.claude-plugin/plugin.json`.

`scripts/release_notes.py` fails if they differ from the tag. The `rcfd --version` output, the service and MCP `version` fields and
the design ledger's solver string are built from the Cargo version.

A version is `MAJOR.MINOR.PATCH`. Before 1.0 (now) the rules are:

* **MINOR goes up** (0.1.3 to 0.2.0) when a release can break something a user relies on: a removed or renamed command-line
  option or exit code, a case file that loaded before and no longer does, a changed MCP tool or service method, a changed UDF ABI or
  Python function, a changed output file format, or a different numerical result from an unchanged case with default settings.
* **PATCH goes up** (0.1.3 to 0.1.4) for everything else: fixes that keep all of the above, new opt-in options, documentation.
* The Rust library API of the crates is not part of this promise: no crate is published (every `Cargo.toml` has `publish = false`)
  and the internal dependencies are path-only, without version numbers. The crates are therefore not on crates.io, and the release
  workflow does not publish them.
* 1.0 will be the first version that promises anything beyond the rules above. When it comes is not decided.

## What a release is

A release is a tag `vX.Y.Z` on a commit of `main` on which the full gate passed, plus a GitHub release whose notes are the
`X.Y.Z` section of CHANGELOG.md. A release has no binaries; you build from source (README). Once its release is published a tag is never moved
or reused: a broken release is followed by a new one with a higher number.

## What the checks prove

| Check | When | Proves | Does not prove |
|---|---|---|---|
| CI `fmt` | every pull request and push to `main` | formatting matches `rustfmt.toml` | |
| CI `clippy` | same | the whole workspace, integration tests included, compiles on Linux with Rust 1.99.0 and has no clippy warning | that any test passes |
| CI `unit tests` | same; Windows only on `main` and on demand | the tests inside `src/` (`--lib --bins`, release) pass on Linux, and on Windows where run | anything about the integration tests in `crates/*/tests`: about 80 % of the tests, more than 99 % of the test time and every solver validation case |
| CI `type-checks with Rust 1.90` | same | the workspace, tests included, still type-checks (`cargo check`) with the oldest toolchain README promises | that it links or passes tests there; the lower per-crate `rust-version` values are not checked |
| CI `dependency licences and sources` | same | every dependency's licence is on the allow-list in `deny.toml`, no dependency comes from a git URL or an unknown registry | RustSec advisories (left out so a new advisory cannot fail an unrelated pull request) |
| CI `script self-tests` | same | the gate runner's time-limit rule, the limit scanner and the release-notes check work | |
| `Full gate` workflow | Sundays and on demand | fmt, clippy and every non-ignored test plus the doc-tests pass, one process per test (`scripts/gate_test.py`) | the about 660 `#[ignore]` tests; CUDA tests (no device: they skip and count as passed); wgpu tests where the runner offers no adapter (same); the 12 tests that need fetched validation data (they skip and count as passed, [test process](test-process.md) section 6.3); macOS |

GitHub switches off scheduled workflows in a public repository after 60 days without a push, so the weekly gate stops by itself in a quiet
period: start it by hand before a release and re-enable it in the Actions tab if it was switched off.

The workflow runs in a fresh checkout with no build cache, which matches the maintainer's `git archive` copy with an empty target
folder: `.gitattributes` fixes line endings to LF on every platform, and no build script reads git. The full gate on a GitHub
runner has not been timed, and tests with elapsed-time limits were measured on a faster machine. The maintainer's gate machine needs about 65 minutes for the test stage, three tests at
once on 16 logical processors (test process, section 5); a 4-core runner will take longer. The step stops after 290 minutes and the logs are uploaded
either way. A run counts only if its summary shows 0 failed and its job is green.

## Cutting a release

Every step has a check; do not go on until it holds. `X.Y.Z` is the new version, `vP` the previous tag (none for 0.1.0).

1. **History is final.** Any planned rewrite of `main`'s history is finished and pushed. A tag on a commit that is rewritten later
   points at a commit that is no longer on `main`. Check: after `git fetch`, `git rev-parse main` and `git rev-parse origin/main`
   print the same id, and no rewrite is still planned.
2. **Choose the number.** Read `git log --oneline vP..main` and the `Unreleased` section against the rules above. For 0.1.0 the
   number is already set everywhere. Check: every `Unreleased` entry fits the kind of version you chose (any entry that breaks one
   of the listed things means MINOR).
3. **Prepare the release commit** on `main`:
   * set `X.Y.Z` in the three places listed under "Version numbers";
   * run `cargo update --workspace` so `Cargo.lock` records the new crate versions; check: `cargo metadata --locked
     --format-version 1 > /dev/null` exits 0;
   * in CHANGELOG.md rename `## Unreleased` to `## X.Y.Z - YYYY-MM-DD` with today's date, and put a new, empty `## Unreleased`
     above it. For 0.1.0, replace `not yet released` in the prepared heading with the date instead;
   * check: `python scripts/release_notes.py vX.Y.Z` exits 0 and prints the notes you expect.

   Commit with the message `release X.Y.Z` and push it.
4. **CI is green** on that commit: `gh run list --commit <sha> --workflow CI` shows `success`.
5. **Full gate on that exact commit.** Merge nothing else into `main` until the tag is pushed. Either start the workflow with
   `gh workflow run gate.yml --ref main` and follow it with `gh run list --workflow gate.yml --limit 1`, or run the gate procedure of
   [the test process](test-process.md) on a `git archive <sha>` copy. Check: the gate ran on `<sha>`, finished, and reports
   0 failed (in the workflow: a green job and its summary). A gate on any other commit does not count: the version change alone alters version strings in outputs. If it fails, fix on `main` and start again at
   step 3; nothing has been tagged.
6. **Tag and push the tag:**

       git tag -a vX.Y.Z -m "rcfd X.Y.Z" <sha>
       git push origin vX.Y.Z

   Check: `git rev-parse vX.Y.Z^{commit}` prints `<sha>`.
7. **Publish.** The `Release` workflow checks the tag again and opens a draft release with the CHANGELOG section as notes. Add one
   line with the gate result and the link to its run, then publish the draft. Check: `gh release view vX.Y.Z` shows it published, on
   `<sha>`.

If the `Release` workflow fails (step 7), nothing is published. Delete the tag (`git push origin :refs/tags/vX.Y.Z`, then
`git tag -d vX.Y.Z`), fix the cause on `main` and start again at step 3. A published release is never edited to point elsewhere.

## Where to go next

* [The test process](test-process.md): what the gate runs, where its time goes and how to run it on your own machine.
* `CONTRIBUTING.md` in the repository root: what to run before you open a pull request.
* `CHANGELOG.md` in the repository root: what changed in each release.
