# ADR 0731: F-835 proposal: an engine command that creates a project (`rcfd project init` and MCP `create_project`)

Status: **ACCEPTED 2026-11-17.** The maintainer approved the proposal on 2026-11-17 and answered every open question with
its recommendation (see [Decisions](#decisions-2026-11-17)). Built on branch `adr0731` (see [Built](#built)).

Roadmap item F-835 (Electron Studio: new project). Its acceptance asks for "rcfd project init and an MCP create_project,
through rcfd_project::Project::create, ADR first". Parents: [ADR 0800](0800-electron-studio-roadmap.md), guardrails G2,
G4, G12, G14, G15 and G16 (`docs/studio-guardrails.md`); the project layer of
[ADR 0173](0173-workspace-layer-between-the-project-folder-the-mcp-server-and-the-studio.md). Number 0731 follows ADR 0730.

## Context

- `rcfd_project::Project::create(root, name, solver_version)` (`crates/rcfd-project/src/project.rs:118-131`) makes the
  folder and the six standard folders (`DIRS`, `project.rs:14`), writes `rcfd-project.json` (`MANIFEST`, `project.rs:12`)
  with `write_json_new`, which never replaces a file (`crates/rcfd-project/src/atomic.rs:197`), and opens it. A folder
  that exists but has no manifest is allowed (`project.rs:117`).
- No CLI, MCP or service call reaches it. Outside tests its only caller was the egui Studio removed by F-449
  (`crates/rcfd-studio/src/project_view.rs:185` at b40c0816^). The `rcfd` command list has no `project` command
  (`crates/rcfd-cli/src/main.rs:202-228`), and `rcfd-cli` does not depend on `rcfd-project`
  (`crates/rcfd-cli/Cargo.toml:19-27`).
- `rcfd-mcp --project <dir>` stops with an error when the folder is not a project (`crates/rcfd-mcp/src/main.rs:56-72`).
  Without `--project` it serves the default tool list, held byte for byte by `crates/rcfd-mcp/tests/golden/tools_list_default.json`
  (`crates/rcfd-mcp/tests/default_f1.rs:8`).
- The Studio opens a folder by starting `rcfd-mcp --project <folder> --read-only`
  (`apps/studio-web/src/main/project/server.ts:92`). For an empty folder it shows "Make a project here" as unavailable,
  watches the folder and opens it when the manifest appears (`apps/studio-web/README.md:801-805`). The first-path e2e test
  writes the manifest itself in place of the missing command (`apps/studio-web/tests/e2e/first-path.spec.ts:50-63`).
- The version a run records is the text `rcfd --version` prints (`crates/rcfd-runner/src/client.rs:255-257`), which is
  `rcfd <CARGO_PKG_VERSION>` (`crates/rcfd-cli/src/main.rs:202-204`).

## Proposal

### P1. A shared function

`rcfd_workspace::create_project(dir: &Path, name: Option<&str>) -> Result<Value, RpcError>`: checks the folder, calls
`Project::create(dir, name, "rcfd <version>")` unchanged, and returns
`{root, name, schema_version, created, solver_version_created}`, the manifest fields that `project_info` already reports.
`name` defaults to the folder's last component.

Errors (workspace codes, `crates/rcfd-workspace/src/lib.rs:55-66`):

- 1200 the folder already holds `rcfd-project.json`; nothing is written. `data.root` names it.
- 1004 the path is a file, or (MCP) outside `--root`.
- -32602 an empty name, a name longer than 200 characters, or a name with a control character.
- -32000 any other I/O failure, with the path and the OS message. Folders made before the failure stay (`Project::create`
  makes folders before the manifest); the message says which.

### P2. CLI

`rcfd project init <dir> [--name <name>]`: prints the P1 result as one JSON line. Exit 0 on success, 1 on a refusal or
failure (message on standard error: `error: <dir>: <reason>`), 2 on a usage error. `rcfd --help` gains one line.

### P3. MCP

`create_project {path, name?}`, access class `write_files`, on the server without `--project` (a server with a project
already has one). With `--root`, `path` must resolve inside it (1004 otherwise). `--read-only` refuses it (1100). The result
is the P1 object. This adds one entry to the default tool list (Q2).

### P4. The Studio

The main process runs `rcfd project init <folder> --name <name>` (the `rcfd` it already starts for the service,
`apps/studio-web/src/main/service/spawn.ts:90`) as a named action, then opens the folder as today. "Make a project here"
becomes available. The first-path test drops its stand-in. The Studio still writes nothing into a project folder (G2, G12).

### How defaults and `baseline_hashes.rs` stay unchanged

- `Project::create`, the manifest format and every existing command are unchanged; nothing in a solver path is touched.
- No case, run or result changes, so `crates/rcfd-api/tests/baseline_hashes.rs` and the case hash tables are untouched.
- The default MCP tool list changes only if Q2 is answered (a), and then by a registered restatement in the style of
  `crates/rcfd-mcp/tests/project_list_f270.rs:1-19` (the golden is not re-recorded or edited). `rcfd --help` is checked by
  `contains` (`crates/rcfd-cli/tests/cli.rs:38-54`), so the added line breaks no test; it is still a help text change (Q4).

## Alternatives

1. **CLI only, Claude Code uses Bash.** Smallest change and no golden touched, but the acceptance names an MCP tool, and
   an MCP client without a shell could not make a project. Recommendation: both, if you approve the golden restatement.
2. **The command in `rcfd-runner` (`rcfd-runner init`), which already depends on `rcfd-project`.** No new dependency
   edge, but project creation is not the run service's job. Recommendation: no.
3. **A flag `rcfd-mcp --project <dir> --create`.** Creates and serves in one start, but hides a write in a start-up flag
   and gives no plain CLI command. Recommendation: no.
4. **A service method `project.create`.** The service is session-based and sandboxed to one root; projects are not its
   concept. Recommendation: no.
5. **The Studio writes the manifest (as the test stand-in does).** Breaks G2 and G12. Recommendation: no.

## Open questions for the maintainer

1. **New dependency edge `rcfd-cli -> rcfd-project` (and `-> rcfd-workspace`).** Both are workspace crates; no third-party
   crate is added. (a) allow; (b) put the CLI command in `rcfd-mcp`'s binary instead. Recommendation: (a).
2. **MCP `create_project` on the default tool list.** (a) add it, with a registered restatement of `default_f1.rs`'s check
   (the golden plus exactly this tool); (b) CLI only for now. Recommendation: (a), because the acceptance asks for it.
3. **A folder that exists and is not empty.** (a) allowed, as `Project::create` allows it, and the reply lists the
   top-level entries that were already there; (b) refused unless `--existing`; (c) always refused. Recommendation: (a).
4. **`rcfd --help` text.** (a) add the line `project init <dir> [--name <name>]`; (b) leave help unchanged and document
   the command only in `docs/`. Recommendation: (a).
5. **A journal line on creation (`project_created` in the ADR 0188 journal).** (a) no, the manifest's `created` is the
   record; (b) yes. Recommendation: (a).
6. **The solver version string.** (a) `rcfd <version>`, the text runs record; (b) the bare version. Recommendation: (a).

## Tests that would prove it

- **P-T1** `rcfd project init` in a new folder: exit 0, the folder opens with `Project::open`, the six folders exist, and
  the manifest equals one made by `Project::create` with the same name and version except `created`.
- **P-T2** a folder that already holds a manifest: exit 1, the message names the folder, and every file is byte-identical
  afterwards.
- **P-T3** a path that is a file: exit 1, nothing created. An empty or control-character name: exit 2 (CLI) or -32602.
- **P-T4** `--name` absent: the name is the folder name.
- **P-T5** MCP: `create_project` with `--root` and a path outside it gives 1004 and creates nothing; with `--read-only` it
  gives 1100; with `--project` it is not listed.
- **P-T6** the default tool list equals the golden plus `create_project` at its registered place, and nothing else
  (if Q2 (a)); otherwise the golden test passes unedited.
- **P-T7** `baseline_hashes.rs` and the MCP project-mode list test pass unedited.
- **Studio** e2e: from an empty folder, "Make a project here" (button and palette) runs the command and opens the new
  project; the first path runs on it with no stand-in.

## Decisions (2026-11-17)

The maintainer took the recommendation of each open question:

1. Q1 (a): `rcfd-cli` may depend on `rcfd-workspace` (and `rcfd-project`). Built: `rcfd-cli -> rcfd-workspace` as a
   dependency; `rcfd-project` is a dev-dependency only (the tests open what the command made), since the command needs
   nothing of it directly. Both are workspace crates; no third-party crate is added.
2. Q2 (a): `create_project` is on the default tool list, as its last tool. `default_f1.rs` is restated (the golden plus
   exactly this tool); the golden is not re-recorded or edited.
3. Q3 (a): a folder that exists and is not empty is allowed; nothing in it is replaced, and the reply's `existing` lists
   the top-level entries that were already there.
4. Q4 (a): `rcfd --help` gains one line, `project init <dir> [--name <name>]` with its purpose.
5. Q5 (a): no journal line; the manifest's `created` is the record.
6. Q6 (a): the solver version recorded is `rcfd <version>`, the text `rcfd --version` prints.

## Built

- P1: `rcfd_workspace::create_project` (`crates/rcfd-workspace/src/init.rs`), over `Project::create` unchanged, with the
  errors of P1 and `existing` (Q3).
- P2: `rcfd project init <dir> [--name <name>]` (`crates/rcfd-cli/src/cmd_project.rs`); a bad name is a usage error
  (exit 2), a refusal or failure exit 1 with `error: <dir>: <reason>`.
- P3: MCP `create_project {path, name?}` on the server without `--project` (`crates/rcfd-mcp/src/lib.rs`).
- P4: the Studio's New project… (the empty home and the command palette) and Make a project here (a folder that is not a
  project yet) run `rcfd project init <folder>` in the main process and open the project (F-835); the first-path e2e test
  makes its projects through them (F-813).
- Tests: P-T1 to P-T4 and Q3 in `crates/rcfd-cli/tests/project_init_f835.rs`; P-T3 (names) and P-T5 in
  `crates/rcfd-mcp/tests/create_project_f835.rs`; P-T6 in `crates/rcfd-mcp/tests/default_f1.rs`; P-T7 by
  `project_list_f270.rs`, `tool_list_f275.rs` and `baseline_hashes.rs`, unedited; the Studio in
  `apps/studio-web/tests/e2e/first-path.spec.ts` and `tests/unit/project-home.spec.ts`.
