# ADR 0736: F-829 proposal: a project trash with restore, and compare-and-set for `settings.set`

Status: **PROPOSED 2026-11-17.** Not accepted by the maintainer. Nothing is built: no Rust, TypeScript or test code was
written for this ADR, and nothing was run. The maintainer approves the commands below before any code.

Roadmap item F-829 (Electron Studio: undo, safe deletes and edit conflicts), ADR 0801 "Engine follow-ups of the Studio
items", rows E-829a and E-829b. Its acceptance: "deleting a run, result or case is recoverable (for example a project
trash) or confirmed in words that name what is lost; a file changed by another writer since the Studio read it is never
overwritten silently". Parents:
[ADR 0188](0188-adr-0173-amendment-4-f273-registration-project-journal-and-change-feed.md) (project journal),
[ADR 0185](0185-adr-0173-amendment-1-f270-registration-the-rcfd-workspace-crate.md) E4 and E5 (the write rule),
guardrails G2, G4, G12, G16 and G18. Number 0736 follows ADR 0735.

## Context

### Deleting (E-829a)

- Nothing in the engine deletes a run, a result or a case. `rcfd-project`, `rcfd-runner` and `rcfd-workspace` have no
  delete; the only `remove_dir_all` calls are a failed submit's temporary folder and tests
  (`crates/rcfd-runner/src/client.rs:462`, `:490`, `records.rs:172`).
- A run is `runs/<id>/` with an immutable `run.json` and `case.snapshot` (`crates/rcfd-project/src/lib.rs:1-6`). Its
  state is in `status.json`; `RunState::is_terminal` is finished, failed or cancelled
  (`crates/rcfd-project/src/records.rs:39-58`). `verify` reads every folder under `runs/`
  (`crates/rcfd-project/src/project.rs:431-450`).
- Another run can depend on a run: `continue_from` starts from `runs/<id>/output/restart.rcfd`
  (`crates/rcfd-runner/src/runner.rs:700-728`), and a design study lists its runs (ADR 0189 S6, `run.json` `study`).
- Every accepted project write leaves a line in `.rcfd/edits.jsonl` (`crates/rcfd-project/src/atomic.rs:222`,
  `crates/rcfd-project/src/journal.rs:1`), and the change feed reads it (`read_feed`). `list_changes` does not watch
  `.rcfd/` (`crates/rcfd-mcp/src/project.rs:152-153`).
- Case files are written by compare-and-replace under the edit lock: `replace_if_hash` and `replace_if_hash_with`
  (`atomic.rs:417-430`), conflict 1200 with `data.current_hash` and busy lock 1201
  (`crates/rcfd-workspace/src/lib.rs:61-66`).
- The Studio offers no delete (F-829 note).

### `settings.set` without a base (E-829b)

- `settings.set {session, path, value}` sets a key whatever the key holds now
  (`crates/rcfd-api/src/service.rs:460-513`). It reads `old` only to roll back a refused value (`service.rs:475-486`).
  Two clients of one session (the Studio's case editor and Claude Code through `rcfd call`) can overwrite each other
  silently.
- The Studio reads the value again with `settings.get` just before it sends, which leaves a window between the two
  calls.
- `journal.get` gives `case_hash`, the hash of the whole settings tree (`service.rs:912-920`).

## Proposal

### P1. The project trash (E-829a)

A trash folder `.rcfd/trash/<token>/` per trashed item, `<token>` = `t-<yyyymmddThhmmss>-<4 hex>` (the run-id rule,
`rcfd_project::runid`). Inside: the item moved whole by one rename (same volume), and `item.json` `{token, kind, path,
trashed, who, reason, hash, bytes}` written first. Library: `rcfd_project::trash::{trash, list, restore, empty}`.

MCP project tools (CLI twins `rcfd project trash|trash-list|restore|empty-trash`, G16):

- `trash_run {run, reason?}`, class `write_files`: moves `runs/<id>/`. Refused (1002) unless the run is terminal;
  refused (1003, naming them) while another run that is not terminal has `continue_from` this run. Reply `{token, kind:
  "run", path, bytes}`.
- `trash_results {run, parts: ["fields" | "output"], reason?}`, class `write_files`: moves `runs/<id>/fields/` and/or
  `runs/<id>/output/` of a terminal run into one token; `run.json`, `status.json`, `events.jsonl` and `residuals.jsonl`
  stay. Refused as `trash_run`.
- `trash_case {path, base_hash, reason?}`, class `write_files`: moves a file under `cases/` if it still has `base_hash`
  (1200 otherwise, `data.current_hash`), under its edit lock (1201 busy). Runs keep their own `case.snapshot`, so no run
  is affected.
- `list_trash {}`, class `read`: `{items: [item.json ...], bytes}` newest first.
- `restore_trash {token, reason?}`, class `write_files`: moves the item back. Refused (1200, `data.path`) when something
  now occupies the original path; then nothing moves.
- `empty_trash {tokens?, older_than_days?, reason?}`, class `delete` (a new class, Q3): removes items for good.

Each accepted call leaves one `.rcfd/edits.jsonl` line (`who` the tool, `path`, `token`, the actor and reason) and so a
change-feed entry. Under `--read-only` every write tool is 1100.

### P2. Compare-and-set for `settings.set` (E-829b)

`settings.set` and `settings.remove` take an optional base, one of:

- `base_value`: the JSON value the client last read at `path` (`null` for "absent"); compared with `Case::get(path)`
  (JSON equality after the case's own number normalisation).
- `base_hash`: `case_hash` of `journal.get` (the whole tree).

On a mismatch: error 1007 (new code `CONFLICT` in `rcfd_api::service::code`), `data: {path, current_value,
current_hash}`, nothing set, no journal entry, no event. Without a base, the call behaves exactly as now. CLI:
`rcfd call settings.set` passes them through.

### P3. The Studio

Delete in the Runs view and on the home becomes a trash with Undo (`restore_trash` of the token); the home gains a Trash
list. The decision panel (F-829) asks only for `empty_trash`. Every `settings.set` the Studio sends carries
`base_value`; a 1007 is shown as F-829 shows a conflict now.

### How defaults and `baseline_hashes.rs` stay unchanged

- No solver path changes; `crates/rcfd-api/tests/baseline_hashes.rs` is not edited or re-recorded.
- Nothing is moved or deleted unless a trash tool is called. A project with no `.rcfd/trash/` behaves as before;
  `verify` does not read `.rcfd/`.
- `settings.set` and `settings.remove` without a base answer as now; `service.info`'s method list is unchanged (no new
  method). The new error code is returned only to a caller that sent a base.
- The project tool list gains six tools: a registered restatement of `project_list_f270.rs` and `tool_list_f275.rs`, as
  ADR 0733 did; no golden is edited.

## Alternatives

1. **Delete for good, confirmed in words** (the acceptance's second option). No recovery; the Studio's undo cannot reach
   it. Recommendation: only for `empty_trash`.
2. **The system's recycle bin.** Different on each platform, invisible to Claude Code, and not journalled.
   Recommendation: no.
3. **A `trashed` flag in the run folder instead of a move.** Every reader of `runs/` would need to learn the flag.
   Recommendation: no; a move keeps `runs/` meaning what it means.
4. **Compare-and-set by a per-key version counter.** Needs new state in the session. Recommendation: no; the value is
   already there to compare.
5. **The Studio's read-then-set** (today). Leaves a window. Recommendation: no.

## Open questions for the maintainer

1. **Where the trash lives.** Recommendation: (a). (a) `.rcfd/trash/` in the project; (b) a folder outside the project;
   (c) the system recycle bin.
2. **Trash a run that another run continues from.** Recommendation: (a). (a) refuse while that run is not terminal,
   allow otherwise (its restart file is gone, its records stay); (b) always refuse; (c) always allow.
3. **A new tool class for `empty_trash`.** Recommendation: (a). (a) `delete` beside the four of
   `crates/rcfd-mcp/src/policy.rs:18-27`, never on any allow-list by default, so Claude Code asks; (b) `write_files`.
4. **Results.** Recommendation: (a). (a) `trash_results` with `fields` and `output` parts; (b) only whole runs.
5. **Compare base.** Recommendation: (a). (a) `base_value` and `base_hash`, either; (b) `base_value` only; (c)
   `base_hash` only.
6. **Error code for a settings conflict.** Recommendation: (a). (a) a new service code 1007; (b) reuse 1002 (bad state);
   (c) the workspace's 1200, which is a project code.
7. **Automatic emptying.** Recommendation: (a). (a) none; the Studio says the trash size; (b) items older than 30 days
   at runner start.

## Tests that would prove it

- **T1** `trash_run` of a finished run then `restore_trash` gives a byte-identical folder (every file's hash); `verify`
  passes before, between and after.
- **T2** `trash_run` of a queued, running or paused run is 1002 and moves nothing; of a run that a queued run continues
  from, 1003 naming that run.
- **T3** `trash_case` with a stale base is 1200 and moves nothing; a run of the case started before still finishes with
  the same fields.
- **T4** `restore_trash` onto an occupied path is 1200 and moves nothing.
- **T5** each accepted call leaves exactly one `edits.jsonl` line and one feed entry; under `--read-only` 1100.
- **T6** `settings.set` with a matching base equals the call without one (reply, journal, events); with a stale
  `base_value` or `base_hash` it is 1007 with the current value and hash, and `journal.get` is unchanged.
- **T7** `baseline_hashes.rs`, the service tests and the runner tests pass unedited; the tool list equals the restated
  golden.

## Unblocks

F-829 rows E-829a (recoverable delete of runs, results and cases) and E-829b (no silent overwrite of a session's
settings).
