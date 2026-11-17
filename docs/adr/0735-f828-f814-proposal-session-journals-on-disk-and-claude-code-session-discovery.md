# ADR 0735: F-828 and F-814 proposal: session journals on disk, and Claude Code session discovery from the engine

Status: **PROPOSED 2026-11-17.** Not accepted by the maintainer. Nothing is built: no Rust, TypeScript or test code was
written for this ADR, and nothing was run. The maintainer approves the commands below before any code.

Roadmap items F-828 (Electron Studio: provenance inspector and reproduce-from-CLI) and F-814 (Electron Studio: Claude
Code panel), ADR 0801 "Engine follow-ups of the Studio items", rows E-828c and E-814. Both are records of a session that
the engine holds or could read and the Studio cannot name today: a service session's source and journal, and a project's
Claude Code sessions. Parents: [ADR 0800](0800-electron-studio-roadmap.md) section 10 (rcfd-claude retired),
[ADR 0144](0144-wave48j-mesh-run-toggle-and-live-preview.md) (`base_dir` of `session.create`), guardrails G2, G4, G11,
G12 and G16. The maintainer's answer to D-L1 (ADR 0801 Addendum 20): the engine records a session's journal, the Studio
writes none. Number 0735 follows ADR 0734.

## Context

### A service session's record (E-828c)

- `session.create` takes `path`, `toml` or `settings`, and `base_dir` (`crates/rcfd-api/src/service.rs:1442-1500`). Its
  first journal entry is `{op: "create", source: {toml, settings, path, base_dir}}` with `path` as the client gave it
  (`service.rs:1443-1451`); the file's bytes and hash are not recorded, and the sandbox-resolved path is not either.
- `settings.set` and `settings.remove` append `{op: "set", path, value}` and `{op: "remove", path}`
  (`service.rs:505-506`, `:535`). `journal.get` returns `{format: "rcfd-journal", version: 1, entries, case_hash}`
  (`service.rs:912-920`). `session.list` returns ids and states only (`service.rs:434-440`).
- `rcfd run --journal <file>` runs a journal document (`crates/rcfd-cli/src/main.rs:74`, `:411-437`) through
  `runctl::case_from_journal` (`crates/rcfd-api/src/runctl.rs:686-720`), which reads a `path` source relative to the
  journal file's folder (`runctl.rs:695-696`). A journal of a file-made session therefore depends on that file as it is
  when replayed, and nothing tells a reader whether it changed.
- No journal is written to disk by the engine. The Studio could write one (D-L1, answered no).

### Claude Code sessions of a project (E-814)

- `crates/rcfd-claude` is removed (c284aa1d, ADR 0800 section 10). Its discovery (`crates/rcfd-claude/src/discover.rs`
  at f5903048^, 288 lines: `project_dir_name`, `same_path`, `parse_agents`, `live_pids`, `classify`) is now a TypeScript
  port in the Studio's main process (`apps/studio-web/src/main/claude/discover.ts:1-156`): the transcript folder
  `<claude home>/projects/<folder with every non-alphanumeric character replaced by '-'>`, each transcript confirmed by
  its `cwd`, and `claude agents --json` for live sessions; a session is open, closed or unknown.
- F-814's acceptance says "through rcfd-claude". No engine crate holds the rule now, so the CLI and Claude Code cannot
  list a project's sessions as the Studio does (G16).

## Proposal

### P1. The session's source, recorded (E-828c)

`session.create` records in its create entry, additively: `source.resolved` (the sandbox-resolved absolute path, for a
`path` source) and `source.hash` (`fnv64:` of the bytes read, `rcfd_project::records::hash_bytes`). `journal.get` and
`session.list` gain `source: {kind: "path" | "toml" | "settings" | "empty", path, hash}` per session. These are new
members; existing members keep their values.

### P2. Journal files, opt-in (E-828c)

- `rcfd serve --journal-dir <dir>` (sandboxed like every service path). With it, the service writes each session's
  journal document to `<dir>/<session>-<created yyyymmddThhmmss>.journal.json` at `session.create` and after every
  accepted `settings.set` and `settings.remove`, by write-then-rename. The document is what `journal.get` returns, plus
  `source.hash`. Without the flag nothing is written and every reply is as in P1.
- `journal.get` gains `file` (the path, or null when off). `journal.save {session, path}` (also opt-in) writes the
  document once to a path the client names.
- `rcfd run --journal <file>` checks `source.hash` against the file it loads for a `path` source and refuses a mismatch
  with exit 3 (`INVALID`, `crates/rcfd-cli/src/main.rs:34`) and the two hashes, unless `--journal-ignore-hash` (Q3).
- The Studio's reproduce command for a session number is `rcfd run --journal <file>`; with the flag off it stays "not
  recorded".

### P3. Claude Code session discovery (E-814)

- A small library module `rcfd_workspace::claude_sessions` with the rule of the removed `discover.rs`: `list(project) ->
  {sessions: [{session, transcript, state: open | closed | unknown, pid, name, title, modified}], warnings}`, the
  transcript folder read-only and `claude agents --json` run with a timeout of 5 s (its failure is a warning and makes
  every state unknown).
- MCP project tool `list_claude_sessions {}`, class `read`; CLI `rcfd project claude-sessions [--json]`.
- The Studio's main process calls the CLI (or the tool through its read-only project server) and its TypeScript port
  goes. The writer rule (the Studio types only into closed sessions and those it starts) stays in the Studio, since it
  is the Studio's own writing.

### How defaults and `baseline_hashes.rs` stay unchanged

- No solver path changes; `crates/rcfd-api/tests/baseline_hashes.rs` is not edited or re-recorded.
- Without `--journal-dir`, the service writes no file. `journal.replay` and `rcfd run --journal` read old documents as
  now (no `source.hash`: no check).
- P1's new members change reply shapes of `session.create`'s journal entry, `journal.get` and `session.list` (G4: the
  maintainer's call, Q1). If the maintainer prefers, they are behind the same opt-in as ADR 0730's `--ui` instead.
- `list_claude_sessions` adds one project tool: a registered restatement of `project_list_f270.rs` and
  `tool_list_f275.rs`, no golden edited. It is not on the default list (no `--project`).

## Alternatives

1. **The Studio writes the journal file** (D-L1 (a)). Answered no by the maintainer.
2. **Embed the source text in the journal instead of a hash.** A journal then replays without the file, but a large
   `.json` case is copied into every journal write. Offered as Q2 (b).
3. **Keep the TypeScript discovery.** Works today; the rule then lives only in the Studio and Claude Code cannot use it
   (G16). Offered as Q4 (b).
4. **Bring `rcfd-claude` back as a crate.** It was removed on purpose; one module in `rcfd-workspace` is enough.
   Recommendation: no.

## Open questions for the maintainer

1. **Source members on by default.** Recommendation: (a). (a) the new `source` members always (additive reply members);
   (b) behind `rcfd serve --ui` (ADR 0730); (c) only in the journal files of P2.
2. **What a path source keeps.** Recommendation: (a). (a) the resolved path and the hash; (b) also the file's text, so
   the journal replays without the file; (c) the path only, as now.
3. **A changed source file at `rcfd run --journal`.** Recommendation: (a). (a) refuse with both hashes, with
   `--journal-ignore-hash` to run anyway; (b) warn and run; (c) no check.
4. **Session discovery in the engine.** Recommendation: (a). (a) `rcfd_workspace::claude_sessions` with the MCP tool and
   the CLI command; (b) keep the Studio's port and count it as meeting F-814 ("through rcfd-claude" read as "by the rule
   rcfd-claude had"); (c) a CLI command only.
5. **Journal file names.** Recommendation: (a). (a) `<session>-<created>.journal.json`, one per session, replaced on
   each edit; (b) one append-only `journal.jsonl` per service.

## Tests that would prove it

- **J1** a session made from a file has `source.hash` equal to `hash_bytes` of the file; from `toml` or `settings`,
  `kind` is that and `hash` null.
- **J2** with `--journal-dir`, after create, three sets and a remove, the file equals `journal.get` (plus
  `source.hash`), and `rcfd run --journal <file>` gives the same `case_hash` and the same stored fields and residuals as
  the session run with `run.start`.
- **J3** a source file changed after the journal was written: refused with both hashes; with the flag it runs.
- **J4** without `--journal-dir` no file appears under the sandbox; `service.info`, `journal.get`'s existing members and
  `session.list`'s existing members equal the base commit's.
- **J5** `claude_sessions::list` on fixture transcript folders and fixture `claude agents --json` outputs gives the same
  sessions and states as the Studio's `discover.ts` on the same fixtures (the port's own unit fixtures), including a
  folder name that two working folders share and a failed listing.
- **J6** `baseline_hashes.rs` and the service tests pass unedited; the project tool list equals the restated golden.

## Unblocks

F-828 row E-828c (a session number's source file and reproduce command) and F-814 row E-814 (session discovery from the
engine, so the TypeScript stopgap can go).
