# ADR 0647: F-706 registration: private-ledger repository markers beyond git and Cargo

Status: **REGISTERED 2026-11-16, written before any code of F-706 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-706 was written in the branch, and nothing was run; no arithmetic
was done beforehand (the checks below are exact, set-membership and string checks). Results go in the addenda at the end;
nothing above the addenda is changed after a run.

Roadmap item F-706 (deps F-700, F-699), proposed by [ADR 0632](0632-f700-registration-ledger-records-for-objectives-surrogates-and-measured-data.md)
(F-700) remaining question 1, recommendation (b). Parents: ADR 0632 D2 and check A8 (`Ledger::private`, `repository_ancestor`),
[ADR 0622](0622-f699-fidelity-management-screen-confirm-correct.md) D7 and check P3 (the fidelity driver's privacy split and its
path refusals), [ADR 0185](0185-adr-0173-amendment-1-f270-registration-the-rcfd-workspace-crate.md) and ADR 0135 (the project
folder that `rcfd-mcp --project` and the Studio open). Number 0647 given by the lead. Branch `f706-work` from local main
d40cd665 (F-700 and F-699 merged), worktree `w49i-work/f706`, scratch `w49i-work/f706-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run; each registered run is made once;
existing tests are not edited; defaults bit-identical; no edit of `baseline_hashes.rs`, of the hash tables or of any golden; no
new dependency, no unsafe code; clean-room; the roadmap files are not edited. Choices the maintainer would normally be asked
about are taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), and say so.

Clean-room: nothing here is an algorithm from a source. The markers are the documented on-disk layout of each tool, used only as
names: a Mercurial working copy has a `.hg` directory at its root; a Subversion working copy has a `.svn` directory at its root
(since Subversion 1.7, and in every directory before it); both from general knowledge of those tools, cited from memory, and
only the directory name is used. The rcfd project marker is read from this repository's code (below). No vendor code, text or
install was used.

## What F-706 asks

The acceptance, binding:

> Ledger::private refuses a path under a directory holding .hg, .svn or the project-mode marker of ADR 0185, with a located message, and every ADR 0632 check keeps its result. Every limit not stated here is registered in the ADR before the run.

Clauses:

- **a1** `Ledger::private` refuses a path under a directory holding `.hg`;
- **a2** the same for `.svn`;
- **a3** the same for the project-mode marker of ADR 0185;
- **a4** the refusal message is located (names the directory that holds the marker, and the marker);
- **a5** every ADR 0632 check keeps its result.

Brief constraints: negative controls (a path outside any marker accepted) and mutations registered before the run; library only;
MCP goldens unchanged; defaults bit-identical.

## What the code read shows (d40cd665, nothing run)

1. `crates/rcfd-api/src/design/ledger.rs` lines 691 to 720: `repository_ancestor(path)` makes `path` absolute (current directory
   for a relative path), canonicalises the nearest existing ancestor and keeps the rest of the path as given, then returns the
   first of `full.ancestors().skip(1)` (the path itself skipped) for which `d.join(".git").exists()` or `d/Cargo.toml` reads and
   contains `[workspace]`. Lines 736 to 747: `Ledger::private` returns
   `Err("private ledger: {path} lies inside the repository tree at {ancestor}; give a path outside any repository")` when that
   is `Some`, before anything is written.
2. Callers of `Ledger::private` / `repository_ancestor`: `design/fidelity.rs:1351` (the driver, `fidelity::run`, with
   `spec.privacy`; the error is prefixed `fidelity: `, and the check runs before any evaluation, ADR 0622 P3); the F-700 unit
   tests (`ledger_f700_tests.rs` A8: a path under `crates/rcfd-api/target/` is refused, the message contains
   `inside the repository tree` and the canonical workspace root, and `repository_ancestor` returns that root); the F-699 tests
   (`tests/f699_fidelity.rs`, private ledgers under the system temp directory, and P3 under `crates/rcfd-api/target/`). No MCP
   tool, CLI command or default path calls either (grep of `crates/`).
3. The project-mode marker. ADR 0185 moves the project folder of ADR 0135 (the folder `rcfd-mcp --project <dir>` and the Studio
   open) into `rcfd-workspace` on top of `rcfd-project`. ADR 0185 itself names no file; the code does:
   `crates/rcfd-project/src/project.rs:12` `pub const MANIFEST: &str = "rcfd-project.json";` (`Project::open` reads it;
   `Project::create` writes it with `cases/ meshes/ runs/ designs/ notes/ .rcfd/`), and `crates/rcfd-mcp/src/edit_hook.rs:154`
   decides "an rcfd project" by `root.join(rcfd_project::project::MANIFEST).is_file()`. So the marker is the file
   `rcfd-project.json`.
4. `rcfd-api` does not depend on `rcfd-project` (its `Cargo.toml`); `rcfd-workspace` depends on both.

## Design

D1. Marker list, checked per ancestor directory in this fixed order, the nearest ancestor first (the path itself skipped, as
today):

| order | marker | test on directory `d` |
|---|---|---|
| 1 | `.git` | `d/.git` exists (unchanged; a worktree's `.git` is a file) |
| 2 | `Cargo.toml` with `[workspace]` | `d/Cargo.toml` reads and contains `[workspace]` (unchanged) |
| 3 | `.hg` | `d/.hg` exists |
| 4 | `.svn` | `d/.svn` exists |
| 5 | `rcfd-project.json` | `d/rcfd-project.json` exists |

`exists()` is used for the three new markers, as for `.git`: a marker of the wrong kind (a file `.hg`, a directory
`rcfd-project.json`) still refuses, which errs toward refusing (taken on the maintainer's standing instruction of 2026-11-11,
take the recommendation). Only exact names match (`.hgignore`, `rcfd-project.json.bak` do not). The project's `.rcfd/`
directory is not a marker: it is created with the manifest and the manifest is what `edit_hook` and `Project::open` test.

D2. The name of the project marker lives in `ledger.rs` as `pub const PROJECT_MARKER: &str = "rcfd-project.json"`, not by a new
`rcfd-api -> rcfd-project` dependency (that edge is allowed by the layering of `docs/architecture.md` but changes `Cargo.lock`
and the crate graph for one string). A test in `rcfd-workspace` (which depends on both) checks
`PROJECT_MARKER == rcfd_project::project::MANIFEST`, so a rename of the manifest fails a test. Taken on the standing
instruction.

D3. New `pub fn repository_marker(path) -> Option<(PathBuf, &'static str)>`: the nearest ancestor and the name of the first
marker (in D1 order) it holds; the marker names are `.git`, `Cargo.toml [workspace]`, `.hg`, `.svn`, `rcfd-project.json`.
`repository_ancestor(path)` keeps its signature and becomes `repository_marker(path).map(|(d, _)| d)`; for a path whose nearest
marker is `.git` or a workspace `Cargo.toml` its result is unchanged (the walk and the order of the old tests are kept, and a
new marker only adds directories).

D4. Message: `private ledger: {path} lies inside the repository tree at {ancestor} (marker {marker}); give a path outside any
repository`. It keeps the text A8 and P3 test (`inside the repository tree`, the ancestor, `repository`) and adds the marker.
Like today the ancestor is shown canonical (on Windows with the verbatim prefix, ADR 0632 question 2 (a)).

D5. Nothing else changes: `Ledger::new`, every public ledger and every default path are untouched (no default path calls
`Ledger::private`); the fidelity driver picks the new refusals up through its existing call. Library only; no MCP tool, case key,
schema or golden.

## Set-ups

All under a fresh directory `T = temp_dir()/rcfd_f706_<tag>_<pid>` (removed and created at the start of each test). Pre-condition
**N0**: `repository_marker(T/probe.jsonl)` is `None` (the temp directory is outside every marker); if it is not, the test fails
as a defective set-up, not as a miss.

- **H**: `T/hgrepo/.hg/` (empty directory). Private paths `T/hgrepo/ledger.jsonl` and `T/hgrepo/a/b/ledger.jsonl` (`a/` not
  created).
- **S**: `T/svnrepo/.svn/` likewise, with `T/svnrepo/ledger.jsonl` and `T/svnrepo/a/b/ledger.jsonl`.
- **P**: `T/proj/rcfd-project.json` (file, content `{}`), with `T/proj/ledger.jsonl` and `T/proj/designs/x/ledger.jsonl`.
- **PW** (in `rcfd-workspace`): a real project from `rcfd_project::Project::create(T/proj, "f706", "test")`, private path
  `T/proj/designs/private.jsonl`.
- **N** (nesting): `T/outer/.hg/` and `T/outer/inner/rcfd-project.json`, path `T/outer/inner/x/ledger.jsonl`; and
  `T/o2/rcfd-project.json` with `T/o2/i2/.svn/`, path `T/o2/i2/ledger.jsonl`.
- **O** (order in one directory): `T/both/.git/` and `T/both/.hg/`, path `T/both/ledger.jsonl`.
- **C** (negative controls, each expected accepted): `T/plain/ledger.jsonl`; `T/names/ledger.jsonl` with files `.hgignore`,
  `.hgtags`, `hg`, `.svnignore`, `svn`, `rcfd-project.json.bak`, `rcfd-project.jsonl`, `project.json` and directories `.hg_old`,
  `_svn` in `T/names`; `T/near/ledger.jsonl` with `T/sib/.hg/` and `T/sib/rcfd-project.json` in a sibling; `T/down/ledger.jsonl`
  with `T/down/sub/.hg/`, `T/down/sub/.svn/`, `T/down/sub/rcfd-project.json` below it; `T/crate/ledger.jsonl` with
  `T/crate/Cargo.toml` holding `[package]` only. Nine marker-like names and four directories.
- **D** (driver): `T/drv/.hg/`, privacy path `T/drv/private/ledger.jsonl`, public `T/pub/ledger.jsonl`; a low `FnModel` and a
  private `HighSource` (fidelity `measured`, `private() == true`) both counting calls; the three parameters, metrics and spec of
  ADR 0622's S1 (`FidelitySpec::new` with `Candidates::Lhs { n: 20, seed: 706 }`).

## Checks

| id | clause | set-up | limit (pass when) |
|---|---|---|---|
| B1 | a1, a4 | H, both paths | `Ledger::private` is `Err` for both; each message contains `inside the repository tree`, the display of `canonicalize(T/hgrepo)`, and `(marker .hg)`; `repository_marker` returns `(canonical T/hgrepo, ".hg")`; neither ledger file nor `T/hgrepo/a` exists afterwards |
| B2 | a2, a4 | S | as B1 with `T/svnrepo` and `.svn` |
| B3 | a3, a4 | P | as B1 with `T/proj` and `rcfd-project.json` |
| B3w | a3 | PW, in `rcfd-workspace` | `PROJECT_MARKER == rcfd_project::project::MANIFEST`; `Ledger::private(T/proj/designs/private.jsonl)` is `Err` naming canonical `T/proj` and `rcfd-project.json`; the file is not created |
| B4 | a4 | N | the first path names `canonical T/outer/inner` and `rcfd-project.json`, its message not containing `.hg`; the second names `canonical T/o2/i2` and `.svn` |
| B5 | D1 order | O | marker `.git`, ancestor `canonical T/both` |
| B6 | negative controls | N0 and C | `repository_marker` is `None` and `Ledger::private` is `Ok` with `is_private()` for all 5 paths; on `T/plain/ledger.jsonl` one `measured` entry appends and reads back equal (0 skipped) |
| B7 | a1 through the driver (ADR 0622) | D | `fidelity::run` is `Err` starting `fidelity: private ledger:` and containing canonical `T/drv` and `.hg`; 0 low calls, 0 observe calls; neither the private nor the public file exists |
| B8 | a5 | existing tests, unchanged files | every test of `design::ledger::f700_tests` (11, A1 to A8 and A11 to A13) and `design::ledger::tests` passes; the whole `tests/f699_fidelity.rs` passes (P3 included); the `design::` unit tests of rcfd-api pass (ADR 0632 A10); the rcfd-mcp tools-list goldens `default_f1` and `project_list_f270` pass (A9) and `git diff d40cd665 -- crates/rcfd-mcp` is empty; no existing test file differs from d40cd665 |
| B9 | defaults | diff | `git diff d40cd665 --stat` touches only `ledger.rs` (inside `repository_ancestor`, a new `repository_marker` and `PROJECT_MARKER`, the message of `Ledger::private` and its doc lines), the two new test files, this ADR and `docs/design-loop.md`; no shared solver file, so `baseline_hashes` is not required (rule 3) |

Run profile: release (`cargo test -p rcfd-api --release`, `cargo test -p rcfd-workspace --release`, `cargo test -p rcfd-mcp
--release --test <golden tests>`). B1 to B7 are new tests: `crates/rcfd-api/tests/f706_private_markers.rs` (B1, B2, B3, B4, B5,
B6, B7) and `crates/rcfd-workspace/tests/private_marker_f706.rs` (B3w). Each is expected to take well under a second.

## Mutations (run on a `git archive` copy in the scratch folder, each alone, after the registered run)

| id | mutation | expected to be caught by |
|---|---|---|
| M1 | `.hg` dropped from the marker list | B1, B7 |
| M2 | `.svn` dropped | B2, B4 |
| M3 | the project marker dropped | B3, B3w, B4 |
| M4 | every new marker tested with `is_dir()` instead of `exists()` | B3, B3w, B4 (the manifest is a file) |
| M5 | the outermost marked ancestor returned instead of the nearest (`filter(..).last()`) | B4 |
| M6 | a directory counts as marked when any entry's name starts with a marker name | B6 |
| M7 | the marker name left out of the message | B1, B2, B3 |
| M8 | `PROJECT_MARKER` misspelt `rcfd_project.json` | B3w, and B3 (set-up P writes the literal name `rcfd-project.json`, not the constant) |
| M9 | `Ledger::private` never refuses | B1, B2, B3, B7, and ADR 0632 A8 |

## Decision rule

- **done**: B1 to B9 met in the one registered run, and every mutation caught by at least one check.
- **partial**: B8 or B9 met but one of B1 to B7 missed (recorded as `*_known_miss`), or a mutation not caught and not shown
  equivalent.
- **todo**: B8 missed (an ADR 0632 check changed its result) and not resolved by a registered amendment.

## Limits (not covered)

- A path check is not access control (ADR 0622 point 7): a private ledger can still be copied into a repository, a tree can be
  moved under one afterwards, and a symbolic link or junction from inside a repository to an unmarked place is followed by
  `canonicalize` only for the part of the path that exists.
- Other version-control systems (Bazaar `.bzr`, Darcs `_darcs`, Fossil `.fslckout` / `_FOSSIL_`, Pijul `.pijul`, Jujutsu `.jj`)
  and Subversion's Windows `_svn` (`SVN_ASP_DOT_NET_HACK`) are not markers; listed as a question.
- A case-insensitive file system makes `.HG` match `.hg` (Windows, macOS default); a case-sensitive one does not. Not tested.
- The marker of a project folder is only its manifest; a folder that has only `.rcfd/` is not refused.
- No new MCP or CLI path; the message is a library error.

## Addendum 1: results (2026-11-16)

Built as registered (D1 to D5) in `crates/rcfd-api/src/design/ledger.rs` (base lines 691 to 720 and 735 to 739; new
`PROJECT_MARKER`, `MARKERS`, `repository_marker`; `repository_ancestor` now delegates; the refusal adds `(marker <name>)`). New
tests `crates/rcfd-api/tests/f706_private_markers.rs` (B1 to B7) and `crates/rcfd-workspace/tests/private_marker_f706.rs` (B3w).
`docs/design-loop.md` names the markers. One registered run, release profile, on this machine (Windows 11, NTFS temp directory).

| check | limit | measured | met |
|---|---|---|---|
| N0 | temp directory outside every marker | `None` in all 8 new tests | yes |
| B1 `.hg` | both paths refused, located, marker named, nothing created | both refused at `\?\C:\...\rcfd_f706_b1_<pid>\hgrepo (marker .hg)`; no file, no `a/` | yes |
| B2 `.svn` | as B1 | both refused at `...\svnrepo (marker .svn)`; nothing created | yes |
| B3 manifest | as B1 | both refused at `...\proj (marker rcfd-project.json)`; no file, no `designs/` | yes |
| B3w real project | constant equal; refused naming the project root and manifest; no file | `PROJECT_MARKER == MANIFEST`; refused at `...\rcfd_f706_b3w_<pid>\proj (marker rcfd-project.json)`; no file | yes |
| B4 nearest | inner directory and its marker, no `.hg` in (a) | (a) `...\outer\inner (marker rcfd-project.json)`, no `.hg`; (b) `...\o2\i2 (marker .svn)` | yes |
| B5 order | `.git` before `.hg` | `(canonical both, ".git")` | yes |
| B6 negative controls | 5 of 5 accepted, `None`, measured entry round trip | 5 of 5 accepted, all `None`; 1 entry appended and read back equal, 0 skipped | yes |
| B7 driver | `fidelity: private ledger:` with `T/drv` and `.hg`; 0 and 0 calls; no files | `fidelity: private ledger: ... at \?\...\drv (marker .hg)`; low 0, observe 0; neither file exists | yes |
| B8 ADR 0632 checks | all pass unchanged | `design::` unit tests 76 passed, 0 failed, 3 ignored (11 of 11 `f700_tests`, 8 of 8 `ledger::tests` among them); A8 refusal now `... at \?\E:\projects\cfd\w49i-work\f706 (marker .git)`, ancestor equal to the workspace root as before; `tests/f699_fidelity.rs` 16 passed, 0 failed, 3 ignored (P3 (a) names the same ancestor, `.git`); rcfd-mcp `default_f1` 2 passed, `project_list_f270` 1 passed; `git diff d40cd665 -- crates/rcfd-mcp` empty; no existing test file changed | yes |
| B9 diff | only the listed files; no shared solver file | `ledger.rs`, two new test files, this ADR, `docs/design-loop.md` | yes |

Mutations (`git ls-files` copy in `w49i-work/f706-scratch/mut`, script `mutate.py`, runner `runmut.sh`, log `mutations.log`;
each alone; the f706 tests, B3w and ADR 0632 A8 run for each):

| id | caught by | expected |
|---|---|---|
| M1 no `.hg` | B1, B7 | B1, B7 |
| M2 no `.svn` | B2, B4 | B2, B4 |
| M3 no manifest | B3, B4, B3w | B3, B3w, B4 |
| M4 `is_dir()` | B3, B4, B3w | B3, B3w, B4 |
| M5 outermost ancestor | B4 | B4 |
| M6 name prefix | B6 | B6 |
| M7 no marker in message | B1, B2, B3, B4, B7, B3w | B1, B2, B3 |
| M8 misspelt constant | B3, B4, B3w | B3w, B3 |
| M9 never refuse | B1, B2, B3, B4, B7, B3w, A8 | B1, B2, B3, B7, A8 |

Nine of nine caught. A8 passed under M1 to M8 as expected (the copy has no `.git`; its ancestor is found by the workspace
`Cargo.toml`, as in a `git archive` copy).

`cargo fmt -p rcfd-api --check` and `cargo fmt -p rcfd-workspace --check`: clean. `cargo clippy -p rcfd-api -p rcfd-workspace
--all-targets --release -- -D warnings`: clean after one change.

Deviations, disclosed:

1. After the registered run clippy refused the type of `MARKERS` (`type_complexity`); a type alias `MarkerTest = fn(&Path) ->
   bool` was added (no behaviour change), and `cargo fmt` reflowed the two new test files. The 8 new tests were run once more
   on the final source as a build confirmation (8 passed); the mutations ran on the source of the registered run (the alias is
   not on any mutated line).
2. ADR 0632 A8 was run twice: within the `design::` set and alone with `--nocapture` to record its message.
3. `baseline_hashes` not run: no shared solver file changed (rule 3). The whole workspace was not run (the lead's gate).

Decision by the registered rule: **done** (B1 to B9 met in the one registered run, nine of nine mutations caught).

### Remaining questions

1. Other version-control markers (`.bzr`, `_darcs`, `.fslckout` / `_FOSSIL_`, `.pijul`, `.jj`, Subversion's `_svn`). Options:
   (a) keep the five; (b) add them. Recommendation: (a) until a user asks; each is one line in `MARKERS` and one check. Taken on
   the maintainer's standing instruction of 2026-11-11 (take the recommendation).
2. A folder holding only `.rcfd/` (a project whose manifest was removed). Options: (a) not a marker; (b) also refuse.
   Recommendation: (a); `rcfd-mcp --project` and the edit hook do not treat such a folder as a project either.
3. Windows' verbatim prefix in the message (ADR 0632 question 2) stays as recommended there.

## For the lead: roadmap entries

> Proposed status: done

> Follow-up of F-700 (ADR 0632 remaining question 1 b). ADR 0647, 2026-11-12. DONE. Library only (no MCP tool, case key or golden edit): Ledger::private now refuses a path whose nearest ancestor directory holds .git, a Cargo.toml with [workspace], .hg, .svn or the rcfd project manifest rcfd-project.json (the marker of the project folder that rcfd-mcp --project and the Studio open, rcfd_project::project::MANIFEST, kept equal to ledger::PROJECT_MARKER by a test in rcfd-workspace); the new ledger::repository_marker returns that directory and the marker, and the message names both (for example "lies inside the repository tree at ...\hgrepo (marker .hg)"). Checks: both paths under each of .hg, .svn and the manifest refused with the canonical directory and the marker named and nothing created; a private ledger in a project folder made by Project::create refused; the nearest of two nested markers named; .git first in one directory; 5 of 5 negative controls accepted (plain folder; .hgignore, rcfd-project.json.bak and seven more look-alike names; markers in a sibling and in a sub-directory; a non-workspace Cargo.toml) with a measured entry round trip; the fidelity driver refuses a privacy path under .hg before any evaluation (0 low and 0 high calls, no file). Every ADR 0632 check keeps its result (76 design:: unit tests, 16 F-699 tests, the two MCP tools-list goldens, the A8 and P3 refusals still at the workspace root by .git). Nine of nine mutations caught. Not covered: other version-control systems (Bazaar, Darcs, Fossil, Pijul, Jujutsu, _svn), case-insensitive matching, links that point out of a repository; a path check is not access control.

> Note for F-700: Its remaining question 1 is answered by F-706 (ADR 0647): Ledger::private also refuses .hg, .svn and rcfd-project.json ancestors.

No follow-up is proposed (remaining question 1 recommends adding further markers only when asked).
