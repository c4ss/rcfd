# ADR 0089: Wave 38B, F-096 documentation site, executed tutorials and example gallery

Status: registered before any code of this wave was written. Limits and decision rules are fixed; a miss becomes an `#[ignore]`d
`*_known_miss` test (or a reported miss) with the limit intact. Baseline hashes are never re-recorded and no solver default changes.
Parents: F-094 (assistant workflows, partial), F-150 (offline/local-model path, partial), ADR 0067 (design loop and its MCP tools),
`docs/milestone-fan.md`.

## Roadmap acceptance and what it is made into

F-096 acceptance text: "Docs site builds; tutorials executed in CI". Made numeric and buildable:

* "Docs site" = static HTML pages generated from every Markdown file under `docs/` by a small Markdown-to-HTML pass written in Rust inside
  a test (`crates/rcfd-cli/tests/docs_site.rs`). No new dependency: there is no mdBook on this machine and no Markdown crate in
  `Cargo.lock`; nothing is downloaded. The generator handles headings, fenced code, paragraphs, lists, pipe tables, inline code, bold,
  links and HTML escaping, nothing more. Output goes to `CARGO_TARGET_TMPDIR/docs-site` (inside the target directory, never the repo).
  Command: `cargo test -p rcfd-cli --test docs_site -- --nocapture` (prints the output path).
* "CI" = the cargo test gate. `.github/workflows/ci.yml` runs `cargo test --workspace` (debug) on ubuntu and windows, but this
  repository is never pushed in these waves, so the workflow has NOT run; what is measured is `cargo test --workspace` on the development
  machine (the final gate, release) plus a debug run of the two tutorial tests.
* Tutorial format: Markdown in `docs/tutorials/`. Every fenced block carries exactly one tag in its info string, after an optional
  language word:
  * `file=<name>` the block text is written to that path in the tutorial's working directory;
  * `run` (optional `exit=<n>`, default 0) every line starting with `$ ` is one command, split on whitespace with single or double quotes
    grouping, no shell; the program must be `rcfd` (CLI runner) or the block is a JSON-RPC session for `rcfd-mcp` (see below);
  * `run-slow` as `run` but executed only by an `#[ignore]`d test;
  * `output` every non-blank line must occur in the stdout+stderr of the preceding executed block;
  * `not-executed` shown, never run (GUI steps, client configuration); the page text says so.
  An untagged block is an error. Blocks run in order in a fresh temporary directory per tutorial that holds a copy of the repository's
  `cases/` directory, so commands are written as if run from the repository root.
* MCP blocks: an `mcp` block with tag `run` (or `run-slow`) holds newline-delimited JSON-RPC requests. The MCP runner
  (`crates/rcfd-mcp/tests/tutorials.rs`, the only place where `CARGO_BIN_EXE_rcfd-mcp` exists) starts the real binary as
  `rcfd-mcp --root .` with the working directory as current directory, pipes the lines in, and requires one response per request, none
  with a JSON-RPC `error`, none with `"isError": true`. One server process per block; state carried between blocks is on disk only.
  A tutorial holds CLI blocks or MCP blocks, not both (the two binaries live in different crates; `rcfd-cli` cannot dev-depend on
  `rcfd-mcp`, the layering test counts dev-dependencies).

## Registered checks

| Id | Check | Limit |
|---|---|---|
| S1 | pages generated = Markdown files under `docs/` (recursive) | equal, and at least 100 |
| S2 | relative links in generated pages that do not resolve to a generated page | 0 (http(s) and `#` links are not checked) |
| S3 | `docs/index.md` links every tutorial and `case-schema.md`, `meshing.md`, `milestone-fan.md`, `design-loop.md`, `mcp-tools.md` | all present |
| T1 | tutorials present: `quick-start-cli`, `cavity`, `mcp-design-loop`, `gallery`, `desktop-assistant` | 5 of 5 |
| T2 | untagged fenced blocks in `docs/tutorials/*.md` | 0 |
| T3 | every tutorial has at least one `run` block executed in the default suite | 5 of 5 |
| T4 | default-suite blocks: every command reaches its expected exit code, every `output` line is found, every MCP response is error-free | all |
| T5 | gallery: every `cases/*/*.toml` appears in a default-suite `rcfd check` command of `gallery.md`, and every `cases/*/` directory is named in it | all |
| T6 | mutation checks on fixture tutorials: an untagged block, a wrong expected exit code, a missing output line, a missing input file, an MCP request that returns an error each make the runner fail | 5 of 5 detected |
| T7 | wall time of each default-suite tutorial test (CLI, MCP) in a debug build on the development machine | under 60 s each |
| T8 | `run-slow` blocks executed once by the ignored tests in this wave | all pass, measured times reported |
| R1 | design study created through `rcfd-mcp --root .` (relative root) can be proposed and evaluated (see finding F1) | the MCP tutorial runs this way |

The GUI and the assistant sidebar cannot be driven headlessly by the tutorial runner: the desktop tutorial's GUI steps are `not-executed`
and are claimed only as written instructions; the headless GUI tests of F-140..F-150 remain the evidence for those features. No tutorial
claims fan performance: the fan section only builds and checks the sector mesh and states the status of `docs/milestone-fan.md`.

## Smoke results (rule d, measured before registering the limits above)

Release binaries built from bf2aa83 in a scratch copy:

* `rcfd check` on the ten `cases/*/*.toml`: all exit 0; 0.03 to 0.32 s each (fan sector 13 332 cells 0.32 s, fan C-grid 11 536 cells
  0.21 s with warning SV101). The six `cases/*/case.json` are benchmark descriptors for `rcfd cases`, not case files: `rcfd check` refuses
  them with exit 3 (unknown settings), so the gallery lists them through `rcfd cases --dir cases` and does not check them.
* Cavity 32 x 32, SIMPLEC, `linear_upwind`, nu 0.01: converged in 121 iterations, 0.44 s; `rcfd post` and `rcfd sample` work; the
  validation of a deliberately misspelt report key exits 3 with a line-numbered diagnostic.
* Capped runs (`--max-iterations 200`): fan sector 14.7 s exit 5, fan C-grid 12.8 s exit 5, the rest under 1.2 s. Uncapped runs:
  `kepsilon_channel` 3.8 s exit 0, `kepsilon_inlet` 0.6 s exit 0, `sst_channel` 8.4 s exit 0, `rotating_channel` 1.2 s exit 0,
  `profile_inlet`, `sod`, `wedge` exit 0, `packed_bed` exit 5 (stops at its own 1700-iteration cap without meeting its tolerances).
* MCP session on the contraction duct of ADR 0067 E1 (`wedge_2d` 40 x 12): create_case, check_mesh, design_create_study (Nelder-Mead),
  design_propose, design_evaluate, design_run_study (4 evaluations), design_list_study: 7 of 7 tool calls without error, 0.66 s with an
  absolute `--root`.
* Debug binaries: all ten checks 2.6 s, the cavity run 2.9 s, the sample 2.8 s, the MCP session 3.8 s.

## Finding F1 (found by the smoke, before this ADR)

With a relative sandbox root (`rcfd-mcp --root .`) or no root, `design_create_study` stores the base case path as given relative to the
current directory, and every later design call resolves it relative to the study directory (`contraction\duct.toml`), so
design_propose / design_evaluate / design_run_study / design_list_study all fail with "cannot read". With an absolute root it works
(the existing tests use absolute roots). Decision: fix in `crates/rcfd-api/src/design/tools.rs` by storing the base path made absolute
against the current directory; for an absolute root nothing changes. R1 is the regression check (the MCP tutorial runs with `--root .`).

## Not in scope

Any GUI automation or screenshot, a real or local model run (F-150 stays as it is), a search index, themes, API docs (`cargo doc` is
unchanged), and publishing the site anywhere.

## Addendum: measured results (wave 38B)

Delivered: `docs/index.md`; five tutorials in `docs/tutorials/` (`quick-start-cli`, `cavity`, `gallery`, `mcp-design-loop`,
`desktop-assistant`); the site generator and link check `crates/rcfd-cli/tests/docs_site.rs`; the shared block parser and runner
`crates/rcfd-cli/tests/tutorial/mod.rs`; the CLI runner `crates/rcfd-cli/tests/tutorials.rs`; the MCP runner
`crates/rcfd-mcp/tests/tutorials.rs`; the F1 fix in `crates/rcfd-api/src/design/tools.rs` (three lines).

| Id | Limit | Measured | Verdict |
|---|---|---|---|
| S1 | pages = Markdown files, at least 100 | 122 pages from 122 files | pass |
| S2 | 0 broken relative links | 0 of 185 relative links | pass |
| S3 | index links every tutorial and the five named docs | all 10 present | pass |
| T1 | 5 of 5 tutorials | 5 | pass |
| T2 | 0 untagged blocks | 0 | pass |
| T3 | every tutorial has a default-suite `run` block | quick-start-cli 8, cavity 3, gallery 5, desktop-assistant 1, mcp-design-loop 7 executed blocks | pass |
| T4 | all default blocks reach their exit code, outputs found, MCP responses error-free | all; 20 output blocks checked (CLI 14, MCP 6) | pass |
| T5 | every `cases/*/*.toml` checked, every `cases/*/` named | 10 of 10 files, 12 of 12 directories | pass |
| T6 | 5 of 5 defects detected | 6 fixtures covering the 5 registered defects: untagged block, wrong exit code, missing output line, missing input file (CLI); unknown tool and `check_mesh` without a session (MCP): all detected, the good fixtures pass | pass |
| T7 | each default tutorial test under 60 s, debug | CLI test 18.4 s (4 tutorials), MCP test 7.4 s | pass |
| T8 | run-slow blocks pass | release: CLI slow test 53.9 s (cavity 64 x 64, all gallery runs incl. both fan passages with exit 5 and `packed_bed` with exit 5), MCP slow test 13.8 s (the duct study run to `complete`) | pass |
| R1 | design study through `rcfd-mcp --root .` | the MCP tutorial runs with `--root .`: pass. Before the fix the same session failed at `design_propose` with a relative root and with no root (smoke); after the fix both give 7 of 7 error-free tool calls | pass |

Deviation (authoring, before any acceptance claim): the first MCP run failed T4 because the tutorial expected `"state":"converged"` from
the `run` tool, which reports `"state":"finished"` and `"converged":true`; the expected lines of the tutorial were corrected. No limit
or rule of this ADR was changed. The runner showed it is not vacuous on a real authoring error.

Not delivered or not verified:

* The GitHub workflow has not run (nothing is pushed); "executed in CI" is shown as "executed by `cargo test --workspace`" only. The
  tests are written to be portable (no shell, `/`-paths in tutorials, `--root .`) but have run on Windows only.
* Every GUI and sidebar step of `desktop-assistant.md` is `not-executed`: the page is written from the code and the roadmap notes,
  never checked against the running app; no real or local model was used.
* A fan design study is deliberately not shown (the fan solve is not validated, `docs/milestone-fan.md`); the fan section builds and
  checks the sector mesh only.
* The site is plain HTML with no search, no navigation tree beyond the index, and ADRs other than 0089 are not linked from the index
  (they are built and link-checked).

Status decision (written after the results): every registered check passes, but F-096 is set to `partial` in `docs/roadmap.json`
because the desktop-app and sidebar part of the feature title is documented but not executed, and the CI workflow itself has not run.
