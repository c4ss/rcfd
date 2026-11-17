# ADR 0736: F-829 proposal: a project trash with restore, and compare-and-set for `settings.set`

Status: **ACCEPTED 2026-11-17.** The maintainer accepted this ADR on 2026-11-17, taking every open question on its
recommendation (answers below). Built the same day; see "As built".

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

## The maintainer's answers (2026-11-17)

1. Q1 (a): the trash is `.rcfd/trash/` inside the project.
2. Q2 (a): trashing a run that another run continues from is refused only while that other run is unfinished.
3. Q3 (a): `empty_trash` is in a new tool class `delete`, which no allow-list holds by default, so Claude Code asks.
4. Q4 (a): a separate `trash_results` tool takes a run's `fields` and `output`.
5. Q5 (a): compare-and-set takes `base_value` or `base_hash`.
6. Q6 (a): a settings conflict is the new service code 1007.
7. Q7 (a): there is no automatic emptying.

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

## As built (2026-11-17)

- **Library.** `crates/rcfd-project/src/trash.rs`. An item is `.rcfd/trash/<token>/`, with `item.json` (`token`, `kind`
  run|results|case, `path`, `parts`, `trashed`, `who`, `reason`, `hash`, `bytes`, `actor`) and `data/` holding what moved.
  The token is `t-<yyyymmddThhmmss>-<4 hex>`. A move takes the edit lock on the path and then on the token, writes
  `item.json` first, renames into `data/`, and rolls back on a failure. A restore refuses an occupied path. Empty removes
  only tokens whose `item.json` reads. `hash` is set for a case file only (the engine's `fnv64:` of its bytes); a folder
  carries null, so nothing hashes gigabytes of fields. `atomic::lock_edit` (an `EditGuard`) is new and public;
  `with_retry` is `pub(crate)`.
- **Workspace.** `crates/rcfd-workspace/src/trash.rs` holds the six commands, appended to `COMMANDS` after `wait_run`
  (25 to 31):
  - `trash_run`: 1002 for a run that has not ended; 1003 with `data.runs` while an unfinished run has
    `settings.continue_from` equal to it.
  - `trash_results`: `parts` from `fields`, `output`; a part the run does not hold is refused.
  - `trash_case`: the path must be under `cases/` (1004); a missing file is 1202; `base_hash` is required and compared
    under the lock (1200 with `current_hash`).
  - `list_trash`: newest first, with `offset`/`limit` and the text budget; replies `{items, bytes, total, offset,
    next_offset, skipped}`.
  - `restore_trash`: a bad token is -32602, a missing one 1202, an occupied path 1200 with `data.path`.
  - `empty_trash`: `tokens`, `older_than_days`, `reason`; replies `{removed, count, bytes}`.

  The journal kinds are `trashed`, `restored` and `trash_emptied`, with the path, token, parts, hash, bytes, actor and
  reason.
- **Change feed.** `feed.rs` has a sentence for each kind. A trashed case claims "absent" and a restored case claims its
  hash, so neither shows as an external change. A restored run's events are read from their current length.
- **MCP.** `Access::Delete` (name `delete`) is added to `policy.rs`. The six tool descriptors come after `wait_run`:
  `list_trash` is read, the four moves are `write_files`, and `empty_trash` is `delete`. `destructiveHint` is true only for
  the class `delete`. The `initialize` text names the trash tools.
- **Allow-lists.** The engine side (`crates/rcfd-mcp/tests/claude_allow`, `DELETE_TOOLS`, `is_delete_tool`,
  `is_whole_server`) and the Studio side (`apps/studio-web/src/main/claude/allow.ts`, `DELETE_TOOLS`, `isDeleteTool`,
  `isWholeServer`) refuse two kinds of rule:
  - a tool of the class delete, under any server name;
  - a rule that names a whole MCP server (`mcp__<server>`), which would allow its delete tools too.

  So no first list, no "Allow this from now on" and no allow-list file can hold one. `list_trash` is not added to the
  first allow-list, the same choice ADR 0188 Q6 (a) made.
- **Service.** `settings.set` and `settings.remove` take `base_value` or `base_hash`, checked after the state checks and
  before the write.
  - `base_value` is compared with `case.get(path)`, null when absent; numbers are compared as numbers.
  - `base_hash` is compared with the `{:016x}` content hash; a hash that is not a string is -32602.
  - A mismatch is the new `code::CONFLICT` 1007, with `data {path, current_value, current_hash}`.

  A call without either answers exactly as before.
- **CLI.** These run through the same workspace commands, with the actor `cli`:
  - `rcfd project trash <dir> (--run <id> | --results <id> [--parts fields,output] | --case <path> --base-hash <hash>)
    [--reason <text>]`
  - `rcfd project trash-list <dir>`
  - `rcfd project restore <dir> <token> [--reason <text>]`
  - `rcfd project empty-trash <dir> [--token <token>]... [--older-than-days <n>] [--reason <text>]`

  They exit 0 on success, 1 on a refusal (with `{code, message, data}` on stderr) and 2 on a usage error.
- **Tool-list tests restated, no golden edited:**
  - `tool_list_f275.rs`: 31 project tools, 48 tools in all.
  - `project_list_f270.rs`: the six new tools are checked at the end, then cut before the ADR 0733 block.
  - `project_runs_w48f.rs`: 48.
  - `project_w48f.rs`: F2 leaves `list_trash` out of the first allow-list; F3 is 17 + 31.
  - `default_f1` and the default count in `crates/rcfd-mcp/src/tests.rs` (34) do not change, because the project tools are
    listed only with `--project`.
- **Studio (F-829).** `apps/studio-web`:
  - *Main process.* The trash goes over the project port's new `trash` operation (`what` is run, results, case, restore or
    empty; checked in `src/main/project/bridge.ts`), to `ProjectSession.trash`, and from there to the run-control server
    (`RunControl.callTrash`).
    - A case is sent with the engine's hash of it as `base_hash`, read through `read_project_file` just before.
    - A run's results are sent with the parts the run holds; the main process reads the folder names and writes nothing.
    - `list_trash` is read with the other lists on the read-only server, and the result is `view.trash`.
  - *Home: moving.* Each row of an ended run in Runs has two icon buttons, "Move the results of run … to the trash" and
    "Move run … to the trash". Each case row in Cases has one. Every move is confirmed in the F-829 decision panel, which
    names what moves and what stays. A refusal comes back in the engine's words, with the way out.
  - *Home: Undo.* Every move is an F-829 history step (`History.recordStep`, scope `project:<folder>`). Undo restores it
    with `restore_trash`; redo moves it again. The home's undo controls sit in its tool row, and Ctrl+Z works there too.
  - *Home: the Trash list.* It appears only while the trash holds anything, with each item, its kind, when it moved and
    its size. Restore is a button on each row. Empty trash goes through the decision panel, which names the items and
    their size and says that nothing brings them back; only the tokens listed are removed.
  - *Settings.* Every write of the case settings (`SettingsSurface`) sends the value the page read as `base_value`,
    except when the page's tree does not hold the path. In that case the engine's default for an optional object is never
    taken for a change by another writer.
    - A 1007 shows under the key, or under the entry for a removal, with the value now and two choices: Set mine (with
      the value now as its base) or Keep.
    - An undo or redo of a settings step carries the value it expects as its base (`withBase`). A 1007 is the same
      conflict outcome as the history's own read. "Undo anyway" sends no base.
  - *What the Studio never does.* It never moves or removes a file itself (G12; the unit check on `rm`, `unlink` and
    `rename` is unchanged).
  - *Coverage.* The capability inventory gains the six tools, the four commands and their options, mapped to these
    controls. The `--reason` options and `--older-than-days` are exempted: the Studio gives no reason and does not empty
    the trash by age.
- **G22.** No budget was raised:
  - At rest, the home gains only icon buttons and the undo arrows. Their words are accessible names and tooltips, which
    are not counted.
  - The Trash list is not shown while the trash is empty.
  - The conflict blocks and panels are exceptions.

  The Runs view (F-806/F-807, budgets 14 and 16) gained no trash control. With so few words to spare, a delete there
  waits for its own item; the home's Runs table covers every run.

### Tests run (2026-11-17, release, headless)

- `cargo clippy --workspace --all-targets --locked -- -D warnings`: clean. `cargo fmt --check` passes on the touched
  packages.
- `cargo test --release`, with the touched files only:
  - `rcfd-mcp`: `trash_adr0736` (7: T1 to T5, E, A), `tool_list_f275` (7), `project_list_f270` (1), `project_runs_w48f`
    (3), `project_w48f` (10), `allow_file_w48e` (4), `default_f1` (2), `feed_f273` (3), `runner_config_adr0733` (1), and
    the lib (18).
  - `rcfd-api`: `settings_cas_adr0736` (3: T6), the lib (141), `workflow` (5), and `baseline_hashes` (3 passed, 1
    ignored; unedited: T7).
  - `rcfd-project`: the lib (3), `edit_lock_w48f` (4), `journal_f273` (4) and `watch` (2).
  - `rcfd-workspace`: the lib (5), `feed_replay_f275` (1), `journal_f273` (9) and `save_f270` (8).
  - `rcfd-cli`: `project_trash_adr0736` (4) and `project_init_f835` (6).
- T3's "a run of the case started before still finishes with the same fields" was checked in a weaker form: the run's
  snapshot is unchanged and `verify` passes. No runner run was made.
- The dependency check in `trash_run` and a later `start_run` that continues from the run can race; this is documented
  in the code.
- Studio (`npm ci`, typecheck, `npm run build && npm run package:dir`, the touched specs only): 203 unit tests in 16 specs pass:
  - `trash.spec.ts` is new (8): the main process's trash calls, the port check, the outside history step, `withBase`, a
    stale base refused with 1007, and the race the base closes.
  - `history.spec.ts` was restated: the "no engine delete" gap became "deletes only through the trash".
  - `claude.spec.ts` gains a test that no allow-list holds the delete class, with the same list as the engine side.
  - `project-home.spec.ts` was restated: the reads now include `list_trash`.
  - `run-control`, `pictograms`, `capabilities`, `actions-*`, `settings-*`, `case-editor`, `security`, `compare` and
    `f837` pass unedited.

  96 e2e tests in 13 specs pass against the real engine:
  - `trash.spec.ts` is new (3): move a run and Undo it byte for byte; move results and Empty trash; move a case and
    Restore it.
  - `word-budget` passes with no budget raised.
  - `undo`, `undo-rcfd`, `undo-case`, `settings`, `settings-rcfd`, `case-editor`, `claude-allow`, `a11y`, `project`,
    `theme-surfaces` and `run-control` pass.
