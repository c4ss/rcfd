# ADR 0735: F-828 and F-814 proposal: session journals on disk, and Claude Code session discovery from the engine

Status: **ACCEPTED 2026-11-17 (partly declined).** The maintainer accepted Q1, Q2, Q3 and Q5 with their recommendations
and dropped Q4: Claude Code session discovery stays in the Studio, and P3 is declined (see
[The maintainer's answers](#the-maintainers-answers-2026-11-17)). P1 and P2 are built on branch `adr0735` (see
[Built](#built)).

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

(Line numbers are those of main at 14a2d490, before this ADR was built.)

### Claude Code sessions of a project (E-814)

- `crates/rcfd-claude` is removed (c284aa1d, ADR 0800 section 10). Its discovery (`crates/rcfd-claude/src/discover.rs`
  at f5903048^, 288 lines: `project_dir_name`, `same_path`, `parse_agents`, `live_pids`, `classify`) is now a TypeScript
  port in the Studio's main process (`apps/studio-web/src/main/claude/discover.ts:1-156`): the transcript folder
  `<claude home>/projects/<folder with every non-alphanumeric character replaced by '-'>`, each transcript confirmed by
  its `cwd`, and `claude agents --json` for live sessions; a session is open, closed or unknown.
- F-814's acceptance said "through rcfd-claude"; it was amended on 2026-11-17 to "through the Studio's own
  implementation of the same rules" (ADR 0801 Addendum 20), and F-814 is done. No engine crate holds the rule.

## Proposal

### P1. The session's source, recorded (E-828c) (accepted, built)

`session.create` records in its create entry, additively: `source.resolved` (the sandbox-resolved absolute path, for a
`path` source) and `source.hash` (`fnv64:` of the bytes read, `rcfd_project::records::hash_bytes`). `journal.get` and
`session.list` gain `source: {kind: "path" | "toml" | "settings" | "empty", path, hash}` per session. These are new
members; existing members keep their values.

### P2. Journal files, opt-in (E-828c) (accepted, built)

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

### P3. Claude Code session discovery (E-814) (declined by the maintainer, 2026-11-17)

Proposed: a library module `rcfd_workspace::claude_sessions` with the rule of the removed `discover.rs`, an MCP project
tool `list_claude_sessions` and a CLI command `rcfd project claude-sessions`, so that the Studio's TypeScript port could
go. **Declined (Q4 dropped):** session discovery stays in the Studio's main process
(`apps/studio-web/src/main/claude/discover.ts`). No `rcfd_workspace::claude_sessions` module, no MCP tool and no CLI
command are added; the project tool list is unchanged. E-814 of ADR 0801 is closed as declined.

### How defaults and `baseline_hashes.rs` stay unchanged

- No solver path changes; `crates/rcfd-api/tests/baseline_hashes.rs` is not edited or re-recorded.
- Without `--journal-dir`, the service writes no file. `journal.replay` and `rcfd run --journal` read old documents as
  now (no `source.hash`: no check).
- P1's new members change reply shapes of `session.create`'s journal entry, `journal.get` and `session.list` (G4: the
  maintainer's call, Q1, answered: always on, additive).

## Alternatives

1. **The Studio writes the journal file** (D-L1 (a)). Answered no by the maintainer.
2. **Embed the source text in the journal instead of a hash.** A journal then replays without the file, but a large
   `.json` case is copied into every journal write. Offered as Q2 (b); not taken.
3. **Keep the TypeScript discovery.** Works today; the rule then lives only in the Studio and Claude Code cannot use it
   (G16). Offered as Q4 (b). This is where the maintainer left it (Q4 dropped).
4. **Bring `rcfd-claude` back as a crate.** It was removed on purpose. Not taken.

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

## The maintainer's answers (2026-11-17)

1. Q1 (a): the `source` reply members (resolved path and hash) are always on, additive.
2. Q2 (a): a path source records its resolved path and the hash of its bytes, not its text.
3. Q3 (a): `rcfd run --journal` refuses a source file whose bytes changed, with both hashes; `--journal-ignore-hash`
   runs it anyway.
4. Q4: dropped. Claude Code session discovery stays in the Studio's TypeScript
   (`apps/studio-web/src/main/claude/discover.ts`); no `rcfd_workspace::claude_sessions` and no MCP or CLI tool for it.
   P3 and test J5 are declined.
5. Q5 (a): one `<session>-<created>.journal.json` per session, replaced on each accepted edit, and only under the
   opt-in `rcfd serve --journal-dir`.

## Decisions made while building (2026-11-17)

The ADR left these open; each is recorded here so it can be overruled.

- **D1. Where the Studio's service keeps its journals: `<state dir>/journals`.** The ADR names no place. The Studio
  starts one `rcfd serve` for the whole Studio, not per project (`apps/studio-web/src/main/service/spawn.ts`), and it
  outlives a project switch and the Studio itself (G7), so a project folder is the wrong owner. The state directory
  already holds that service's `service.json` and `service.log` (`RCFD_STUDIO_STATE_DIR`, default the user data
  folder). The Studio passes `--journal-dir <state dir>/journals`; with `RCFD_STUDIO_SERVICE_ROOT` set to a folder that
  does not hold it, the flag is left out (the service would refuse a journal folder outside its sandbox) and a session
  number's reproduce command stays "not recorded". A service the Studio attaches to (`RCFD_STUDIO_SERVICE`) or finds
  again from an earlier start keeps whatever flags it was started with. The Studio writes no journal file itself.
- **D2. Which file `rcfd run --journal` reads for a `path` source.** As before, the path relative to the journal file's
  folder; when no file is there, the recorded `source.resolved` (the file the service read). A journal of a session
  made from a relative path, kept in `--journal-dir`, then still runs. Old documents (no `source.resolved`) read
  exactly as before. The hash check uses the same file.
- **D3. A journal file that cannot be written does not fail the request.** The setting change was accepted; the failure
  is said on the service's standard error and in the session's log (`log.tail`). `journal.save` does report its own
  write failure (error -32000).
- **D4. `journal.save` is only on a service with journal files on.** Then, and only then, `service.info` lists it at the
  end of `methods` and gives `journal_dir`, as the `data.*` methods are listed only when on (ADR 0350). Otherwise it is
  an unknown method and `service.info` and `METHODS` (69) are unchanged.
- **D5. `rcfd serve --journal-dir` outside `--root` stops the service** with exit 1 and the sandbox's message, before it
  listens; a folder that is missing is made.
- **D6. The created stamp is UTC to the second**, taken at `session.create`. Two services that share one journal folder
  and make their session `s1` in the same second would write the same file name; the Studio's single service does not.
- **D7. A session number's provenance in the Studio** names the case file and its hash for a `path` source, says
  "none: the session was made from case text / settings / the engine's defaults" for the other kinds, adds a "Journal
  file" row when the service keeps one, and shows and copies `rcfd run --journal <file>` in place of "Not recorded". An
  engine without this ADR gives no `source` and the Studio says "not recorded" as before. The e2e expectation of
  `tests/e2e/settings-rcfd.spec.ts` for a session made from the defaults is restated from "not recorded" to the defaults
  wording.

## Tests that would prove it

- **J1** a session made from a file has `source.hash` equal to `hash_bytes` of the file; from `toml` or `settings`,
  `kind` is that and `hash` null.
- **J2** with `--journal-dir`, after create, three sets and a remove, the file equals `journal.get` (plus
  `source.hash`), and `rcfd run --journal <file>` gives the same `case_hash` and the same stored fields and residuals as
  the session run with `run.start`.
- **J3** a source file changed after the journal was written: refused with both hashes; with the flag it runs.
- **J4** without `--journal-dir` no file appears under the sandbox; `service.info`, `journal.get`'s existing members and
  `session.list`'s existing members equal the base commit's.
- **J5** (declined with Q4) `claude_sessions::list` on fixture transcript folders. Not built.
- **J6** `baseline_hashes.rs` and the service tests pass unedited.

## Unblocks

F-828 row E-828c (a session number's source file and reproduce command). F-814 row E-814 is closed as declined.

## Built

Built 2026-11-17 on branch `adr0735` (from main 14a2d490).

- P1: `crates/rcfd-api/src/session_journal.rs` (new): `hash_bytes` (the `fnv64:` spelling of
  `rcfd_project::records::hash_bytes`, from `rcfd_core::hash::fnv64`; rcfd-api does not depend on rcfd-project),
  `source_summary`, `stamp`, `file_name`, `write_atomic`, `source_file` (D2) and `check_source` (Q3).
  `session.create` records `source.resolved` and `source.hash` for a `path` source; `journal.get` gives `source` and
  `file`; `session.list` gives `source` per session (`crates/rcfd-api/src/service.rs`).
- P2: `Service::enable_journal_files(dir)` (sandboxed, once); the session's file is replaced by write-then-rename at
  create and after every accepted set and remove; `journal.save {session, path}` and `service.info` `journal_dir` only
  when on (D4). `rcfd serve --journal-dir <dir>` and `rcfd run --journal-ignore-hash` (`crates/rcfd-cli/src/main.rs`,
  usage text); `runctl::case_from_journal` reads the file of D2. Documented in `docs/service-protocol.md` and
  `docs/run-control.md`.
- Studio: `startService` passes `--journal-dir <state dir>/journals` (`journalDirFor`, D1); a session number's
  provenance reads `source` and `file` from `journal.get` and shows and copies `rcfd run --journal <file>` (D7;
  `src/shared/provenance.ts`, `src/renderer/provenance/Provenance.tsx`). The Studio writes no journal file.
  `capabilities/coverage.json`: `service:journal.get` and `cli:rcfd run --journal` have a UI path;
  `cli:rcfd run --journal-ignore-hash` is exempt under F-828 (the Studio does not offer to skip the check); the
  inventory and `docs/studio-coverage.md` regenerated (`npm run capabilities`).
- P3: not built (declined).
- Tests: J1, J2 (service half, sandbox), J4 in `crates/rcfd-api/tests/session_journal_adr0735.rs` (4 tests); J2 (run
  half: settings, last residuals and `solution.vtu` byte for byte against the session's `run.start`), J3 and the
  sandbox refusal of `rcfd serve` in `crates/rcfd-cli/tests/journal_files_adr0735.rs` (3 tests); unit tests of the
  module in `session_journal.rs` (4 tests); J6: `baseline_hashes.rs`, the `service.rs` unit tests,
  `service_base_dir_w48j.rs`, `runctl.rs`, `service_tcp.rs`, `cosim_service_f360.rs` (69 methods), `run_control.rs`
  and `cosim_serve_f444.rs` pass unedited. Studio: `tests/unit/session-journal-adr0735.spec.ts` (3 tests, one with the
  real `rcfd`), the e2e restatement of D7 in `tests/e2e/settings-rcfd.spec.ts`.
