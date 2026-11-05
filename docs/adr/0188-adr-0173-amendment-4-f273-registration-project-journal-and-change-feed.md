# ADR 0188: ADR 0173 amendment 4, F-273 registration: the project journal and the change feed

Status: **REGISTERED 2026-11-05**, written before any code of F-273. Not yet accepted by the maintainer.
Parent: [ADR 0173](0173-workspace-layer-between-the-project-folder-the-mcp-server-and-the-studio.md) (proposed, not accepted),
requirement R3, work item W6, check C3, decision D4, roadmap item F-273; it builds on
[ADR 0185](0185-adr-0173-amendment-1-f270-registration-the-rcfd-workspace-crate.md) (F-270),
[ADR 0186](0186-adr-0173-amendment-2-f271-registration-new-simulations-from-the-mcp-server.md) (F-271) and
[ADR 0187](0187-adr-0173-amendment-3-f272-registration-mesh-set-up-from-the-mcp-server.md) (F-272, merged at 5fdde2d).
ADR 0173 says that every limit of its checks is fixed by an amendment before any test or run; this is that amendment for F-273.
Results go in the addendum at the end.

Read at main 5fdde2d. Number: 0187 is the highest on main; no branch and no worktree holds a 0188.

ADR 0173 is not accepted. Where it gives a recommendation, this text uses it and says so; that is a recommendation taken, not a
maintainer decision. Where the code read below raises a question ADR 0173 does not answer, the options and a recommendation are
listed (Q1 to Q7), and work proceeds only on a choice that is reversible.

House rules ([ADR 0130](0130-wave48a-overview-and-run-record-contract.md)): defaults stay bit-identical; no solver result, case
hash or byte of an existing case changes; no case-schema key and no run-record field is added;
`crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded; no new third-party crate; no commit trailers. For the journal:
lines change only by added fields and added kinds, and a line written before F-273 still reads.

## What F-273 asks

The roadmap entry: every accepted write, case creation, mesh import, run submission and run control call appends an entry to
`.rcfd/edits.jsonl` with an actor, a sequence number, the settings changed (path, old and new value) and an optional reason
(additive fields only); one ordered feed merges the journal, the run records and events, and the watcher's changes; a change made
by one process appears in a second process's feed within the limit fixed by an amendment of ADR 0173; a write made outside the
workspace appears as external; and the Studio updates its views from the feed and shows who changed what and why (ADR 0173 R3,
W6, C3, D4).

ADR 0173 D4 (c), its recommendation, is used: an entry keeps `who` (the command), and adds an actor `{client, instance}`, the
changed settings and an optional reason. Linking an MCP actor to a Claude Code session is NOT VERIFIED (D4) and is not built
(Q4).

## What the code read shows (main 5fdde2d, nothing run)

1. **The journal today.** `rcfd_project::atomic::replace_if_hash` writes the file, then appends
   `{t, kind: "edit", who, path, old_hash, new_hash}` to `.rcfd/edits.jsonl` (`log_edit`, `atomic.rs` line 256), while it holds
   the file's edit lock. There is no project-wide lock and no sequence number; the append is best effort (an error is ignored).
   `acquire` appends `{t, kind: "stale_lock_broken", who, path, age_s}` when it removes a lock older than 30 s. No crate reads the
   file.
2. **Writers.** `edit_case` (`who` `edit_case`), `write_note` (`write_note`) and the Studio's save (`save_case`) go through
   `replace_if_hash`. Case creations (`rcfd_workspace::cases::create_with`, used by `new_case` and by the Studio's case starter),
   mesh imports (`import_mesh`) and run submissions and controls (`rcfd_workspace::runs::RunService`, used by the workspace and by
   the Studio) write no line. Nothing in the workspace records a design (that is F-274).
3. **Tests that read the journal.** `rcfd-project/tests/edit_lock_w48f.rs` (4 lines with their fields; a `stale_lock_broken`
   line then an `edit` line; 400 lines chaining `old_hash` to `new_hash` from two writers), `rcfd-mcp/tests/project_w48f.rs` F5
   (2 lines, `who` and `old_hash`) and E1 (no file after a refused edit), `rcfd-workspace/tests/save_f270.rs` (1 line, or none
   after a conflict), `rcfd-studio/tests/workspace_f270.rs` A3 and A4 (none, or 1 with `who`, `path` and both hashes),
   `rcfd-workspace/tests/mesh_f272.rs` C8-E (unchanged by a refusal) and `rcfd-workspace/tests/new_case_f271.rs` C-N (no file
   after five creations).
4. **Result shapes are pinned.** `rcfd-mcp/tests/golden/project_shapes.json` (F3 of `project_runs_w48f.rs`) pins the key shape of
   every project tool's result, `list_changes` as `{changes: ["string"], since, token, total, truncated}`; F3 reads each change as
   a string. `mesh_f272.rs` C8-E asserts that an `edit_case` without a mesh key returns exactly seven keys.
5. **The watcher.** `rcfd_project::Watcher` hashes files up to 1 MiB with FNV-1a 64, the hash `records::hash_bytes` writes as
   `fnv64:<hex>`; it skips `.rcfd/` and atomic-write temp names, and `changes_since` gives paths only.
6. **Run events.** `Project::create_run` writes `run.json`, `status.json` and a creation event (`state`, `from` null, `to`
   queued). The runner appends `state`, `warning` and `stop` events, and `launched`, `cancel_requested` and one `solver` event per
   solver event line it reads.
7. **The Studio.** The project worker (`crates/rcfd-studio/src/watch.rs`) polls its `Watcher` every 500 ms (`POLL`) and sends
   `Changed` (paths), `FileChanged` (a tracked file's bytes) and `Listing`. `Studio::apply` (`studio.rs`) keeps the last 50
   changed paths outside `runs/` for the "Changed on disk" list and the mesh preview. The run worker (`poll.rs`) polls the runs
   every 250 ms. `Studio::say` puts a message in the status bar and the log panel; existing Studio tests read `last_message()`.
   The Studio makes a `RunService` in `poll.rs`, `quick_run.rs`, `store_plan.rs` and `stop.rs`, and has no list of `notes/`.
8. **The maintainer's worktree** `studio-case-lineage` changes, among others, the Studio's `lib.rs`, `main.rs`, `studio.rs` (not in
   `apply`), `ui.rs`, `poll.rs` and `store_plan.rs`, `rcfd-project/src/project.rs` and `records.rs`, `rcfd-mcp/src/project.rs`
   and `rcfd-runner`. It does not change `rcfd-project/src/atomic.rs`, the Studio's `watch.rs`, `case_starter.rs`, `quick_run.rs`
   or `stop.rs`.
9. **What a process knows about itself.** The standard library gives the process id, but not the process's start time, and on
   Windows not its parent's id; the workspace forbids unsafe code and F-273 adds no crate.
10. **The MCP argument check** (`validate_args`) knows `type`, `enum`, `minimum`, `maximum`, `required`, `additionalProperties`
   and `items`, not `maxLength`.

## Choices of this registration

**J1. The journal line (ADR 0173 D4 (c) and the design's deep layer, used).** One line per accepted write, as today: the
workspace extends the line `replace_if_hash` writes (a new `rcfd_project::atomic::replace_if_hash_with`, which takes the added
fields) and never appends a second line for a write. Fields added: `seq` (J2), `actor` (J3) when the writer has one, `reason`
(J4) when given, and the entry's own fields (J5). New kinds: `case_created`, `mesh_imported`, `run_submitted`, `run_control`.
`design_recorded` is not written: no workspace command records a design before F-274. A `write_note` line keeps `kind: "edit"`
(Q2). Lines written before F-273 are read as they are.

**J2. The sequence number.** Each line is appended under a project-wide lock, `.rcfd/locks/journal.lock` (created with
`create_new`, removed after the append), held only while the next number is found and the line written. `seq` is the last `seq`
in the file plus 1, or 1 when there is none: the reader looks at the last 64 KiB, and at the whole file only when those hold no
`seq`. A file whose last byte is not a newline (a torn line) gets a newline first, so the new line stands alone. Limits: a writer
waits at most 250 ms for the lock; a lock older than 5 s is taken to be left by a crashed writer, is removed, and the line written
then carries `journal_lock_broken_s` (its age); when the lock cannot be had in the wait, the line is appended without `seq`, so
the journal stays best effort as it is today and the write is not undone. A writer may hold a file's edit lock while it waits
for the journal lock, never the reverse. The `stale_lock_broken` line goes through the same append.

**J3. The actor (ADR 0173 D4 (b), used).** `{client, instance: {pid, started}}`, where `started` is the time (RFC 3339, UTC, ms)
at which the process first made an actor: std has no process start time (item 9), and with the pid it tells two processes of one
pid apart. `rcfd_workspace::journal::Actor::this_process(client)`. `rcfd-mcp --project` uses client `mcp`
(`ProjectTools::open`); the Studio's project worker uses `studio` for its saves and creations. `Workspace::with_actor` and
`RunService::with_actor` set it; a writer without one (the library calls as they are today: `Workspace::open`,
`save_case`, `cases::create_with`, `RunService::new`) writes no `actor` field. The Studio's run starts and controls are journaled
without an actor in F-273: two of its four `RunService` constructions (`poll.rs`, `store_plan.rs`) are in files of the
maintainer's worktree (Q3). Clients `cli` and `python` are not built: neither calls the workspace.

**J4. The reason (ADR 0173 D4 (c), used).** An optional argument `reason` of `edit_case`, `write_note`, `new_case`,
`import_mesh`, `start_run`, `pause_run`, `resume_run` and `cancel_run`: a string of at most 500 characters. Not a string, or
longer: -32602, checked before anything is read or written. A blank reason is not recorded. A `dry_run` writes no line.

**J5. What each entry records.** `who` is the command (J1). Values are the case's values as `rcfd_api::case::Case::get` reads
them (defaults included, units converted).

* `edit` from `edit_case`: `changes` [{`path`, `old`, `new`}], one per key of `set` in key order, `old` before the edit and `new`
  after it.
* `edit` from a Studio save (`save_case`): `changes` holds every setting whose value differs between the file before the save
  and the text saved, both read as the case is read; a setting only one side holds has `null` on the other. No `changes` when
  either text does not read as a case. So a save of the editor's whole text, which spells out every default, lists only the
  settings whose values changed.
* `edit` from `write_note`: no `changes` (a note is not a case).
* `case_created`: `path`, `hash`, `bytes`, `from` (as `new_case` gives it, absent for the Studio), `data_files` [{`path`, `hash`,
  `written`}]. `who` `new_case`, `start_run` (the case it makes), `case_starter` (the Studio) or `create_with` (a library call).
* `mesh_imported`: `path` (`meshes/<name>`), `source`, `format`, `files` [{`path`, `bytes`, `hash`}], `cells`, `faces`,
  `patches` (their count), written only when the import wrote a file (an import of a mesh already there writes no line).
* `run_submitted`: `run`, `case`, `set` (every entry the run gets, the start plan's first), `priority`, `threads`, and `mem_mb`
  when given. `who` `start_run` or `submit`.
* `run_control`: `run`, `requested` (`pause`, `resume` or `cancel`), only when the request was accepted. `who` the tool name, or
  `pause`, `resume`, `cancel` for a library call.

Limits: at most 100 `changes` (and 100 `set` entries), with `changes_total` (`set_total`) when the list was cut; a value whose
JSON text is over 1 000 bytes is written as `{"bytes": n, "hash": "fnv64:..."}`. Command results are unchanged: no key is added
to any result (item 4).

**J6. The feed (ADR 0173 D3 (a), used: the files stay the truth and readers poll).** `rcfd_workspace::feed::Feed` reads one
project; `Feed::poll` makes one pass, and `Workspace::changes(since)` gives the entries numbered after `since` and the latest
number. One pass:

1. the watcher's poll (an `rcfd_project::Watcher`, as before);
2. the journal from the byte position reached: a torn last line waits for the next pass; a complete line that is not a JSON
   object is skipped and counted (`skipped_lines`); a journal shorter than the position is read again from its start;
3. for each changed `runs/<id>/events.jsonl`, its new lines from that run's byte position: `state`, `warning` and `stop` events
   enter the feed (kinds `run_state`, `run_warning`, `run_stop`, with `run` and `case`); `solver`, `launched`,
   `cancel_requested` and other kinds do not, and nor does progress (step and time stay with the run poller);
4. every other changed path outside `runs/`: it is explained by a journal entry of this or an earlier pass (kept 60 s) that names
   that path with the hash the file has now (`hash_bytes`, the watcher's FNV-1a 64), and is then not reported again. An
   unexplained path waits, and is reported as `external`, with its path only, once it is still unexplained 500 ms after its
   modification time (after it was first seen, for a removed file). The wait covers a write whose journal line comes after the
   watcher saw the file: `replace_if_hash` writes the file before the line, and the line can wait up to 250 ms for the journal
   lock (J2).

Paths under `runs/` are never external; `.rcfd/` is not watched. Entries are numbered from 1 in the order the reader finds them:
journal lines in file order, then run events by run id and file order, then external paths in path order. The reader keeps the
last 1 000 entries and writes nothing. Each entry has `n`, `source` (`journal`, `run` or `external`), `kind`, `t`, `seq`, `path`,
`run`, `who`, `actor`, `reason`, `changes`, the line's other fields, and `text`: one line for people, for example `edit_case by
mcp (pid 4242): cases/a.toml physics.nu 0.01 -> 0.02; why: to reach Re 50`.

**J7. The MCP descriptors.** A new read tool `read_feed {since?, limit?}`, listed right after `list_changes` (Q1).
`rcfd_workspace::COMMANDS` gains it at the same place. Without `since`: `{cursor, entries: [], total: 0}`. With `since`:
`{since, cursor, entries, total, next, dropped, skipped_lines}`, the entries numbered after `since`, oldest first, at most `limit`
(1 to 200, default 50) and within `TEXT_BUDGET` (`next` the number of the last one returned, null when none; `dropped` how many
entries after `since` are no longer kept). The reader is made at the first call, so a call without `since` starts it. The eight
tools of J4 gain the property `reason` (`maxLength` 500 for clients; the workspace enforces it, item 10); `start_run`'s is written
above `case`, away from the `threads` and `mem_mb` lines of the maintainer's hunk. `list_changes` is unchanged. The project
sentence of `initialize`'s instructions names `read_feed` and `reason`. `rcfd_claude::allow::PROJECT_READ_TOOLS` and the
plugin's skills are unchanged (Q6).

**J8. The Studio (ADR 0173 R3 and "Edges", in part).** Studio files changed: `src/watch.rs` and `src/studio.rs` (one arm of
`apply`), and the new test file `tests/feed_f273.rs`. The project worker keeps a `Feed` in place of its `Watcher`: the paths it
sends and the events that follow from them are the same as before, and the new entries of each pass go out as a new event,
`Event::Feed`. The worker saves through `rcfd_workspace::save::save_case_by` and creates through `rcfd_workspace::cases::create_by`,
with the actor `studio` (`who` `save_case` and `case_starter`). `Studio::apply` says the `text` of each journal entry whose actor
is present and is not this Studio's own; entries without an actor, run entries and external paths add no message (the run list
and the "Changed on disk" list show them already, and existing tests read `last_message()`). The reactions of ADR 0173's R3
table that exist already stay as they are: the browser's lists (`Listing`), an open editor's reload or conflict (`FileChanged`,
`watch_b6.rs`), the mesh preview's rebuild on a changed case path, and the run list (the run worker). Not built in F-273: the run
worker reading the feed (`poll.rs`), a panel listing the feed, and a list of `notes/` (Q5).

**J9. Separable commits.** (1) this text; (2) `rcfd-project`: the journal lock and `seq`, `replace_if_hash_with`; (3) the
workspace's journal fields, `reason`, the feed and their tests; (4) the MCP descriptors and their tests; (5) the Studio; (6)
documents; (7) the addendum. Each builds and passes on its own.

## Open questions

**Q1. Where the feed is offered to MCP (ADR 0173 "Edges": `list_changes` returns the feed).** `list_changes`' result is pinned
(item 4) and its token is the watcher's, not the feed's number. (a) A new read tool `read_feed`; `list_changes` stays as it is;
(b) `list_changes` gains an opt-in argument that returns feed entries, with a second meaning for `since` and `token`; (c)
`list_changes` returns the feed by default, its pinned shape changed. Recommendation (a), as ADR 0186 Q1 and ADR 0187 Q1 did;
F-275 decides the project-mode list and may merge the two. Proceeding on (a); reversible.

**Q2. The kind of a `write_note` line.** ADR 0173's design lists `note` among the new kinds, but `write_note` writes `kind:
"edit"` today. (a) Keep `edit` (lines change only by added fields); the feed's text names the path under `notes/`; (b) write
`note`, changing what an existing line kind means. Recommendation (a); proceeding on (a); reversible (one constant).

**Q3. The Studio's run starts and controls carry no actor (J3).** (a) Leave them without one until the maintainer's Studio
worktree lands, then one `with_actor` at each of the four `RunService` constructions; (b) add it now in `quick_run.rs` and
`stop.rs` (not in that worktree) and later in `poll.rs` and `store_plan.rs`; (c) a process-wide default actor that the Studio sets
once. Recommendation (a): (b) labels some Studio runs and not others, (c) labels an in-process MCP stand-in as the Studio in tests.
Proceeding on (a).

**Q4. Linking an MCP actor to a Claude Code session (ADR 0173 D4 (b), NOT VERIFIED).** (a) Record the parent process id in the
actor, so a reader can match it with `claude agents --json`; std has no parent id on Windows and the workspace forbids unsafe
code (item 9), so this needs a platform call through a crate, which is the maintainer's to add; (b) Claude Code passes a session
id to a plugin's stdio server: no documented way (ADR 0135 lists the substituted values); (c) no link. Before (a) or (b), check on
an installed build whether the server's parent process is the session's process. Proceeding on (c): nothing claims a link.

**Q5. The Studio's views beyond messages.** A panel listing the feed with who, what and why, a `notes/` list, and the run worker
reading the feed in place of its own poll are changes to `ui.rs` and `poll.rs`, in the maintainer's worktree. (a) After that
worktree lands; (b) now. Recommendation (a); proceeding on (a).

**Q6. `read_feed` and the Studio bridge's allow-list.** As ADR 0187 Q6: (a) leave `PROJECT_READ_TOOLS` and the plugin's skills as
they are, so the Studio's sidebar asks before `read_feed` runs; (b) add it. Recommendation (b), since ADR 0134 W48-11 (a) made
that list the read-class project tools; it changes what runs without asking, so it is the maintainer's. Proceeding on (a).

**Q7. A reader starts at the present.** A feed made later does not show what happened before it was made, so a new Claude Code
session sees no history through `read_feed`. (a) As built; the journal stays readable with `read_project_file`
(`.rcfd/edits.jsonl`); (b) `read_feed` gains a `from_seq` that replays the journal from a sequence number. Recommendation (b)
later, with F-275's list. Proceeding on (a).

## Acceptance checks and limits (fixed here, before any code)

* **C3-J (journal lines, workspace).** In one project, with a workspace whose actor is `{client: "mcp"}` of this process: an
  `edit_case` with `reason` writes one `edit` line with `seq` 1, `who` `edit_case`, that actor, `changes` [{`physics.nu`, 0.01,
  0.02}] and the reason; a `write_note` line `seq` 2 without `changes`; a `new_case` from `channel/profile_inlet.toml` one
  `case_created` line with the case's and the data file's hashes; an `import_mesh` of a Gmsh sample one `mesh_imported` line with
  the reader's cell, face and patch counts, and the same import again no line; each `seq` one more than the one before. Nothing is
  written by: a `dry_run`; a refused edit (1003); a conflict (1200); a reason of 501 characters or a number (-32602, the file's
  bytes unchanged); a refused name in `new_case`. A blank reason gives no `reason` field. A workspace without an actor writes no
  `actor` field. Every command result has the keys it had before F-273 (the seven of `edit_case`, item 4).
* **C3-D (the Studio save's changes, workspace).** `save_case_by` with the editor's whole text (`Case::to_toml` of the tree) of a
  short case file with `mesh.nx` changed from 16 to 10: `changes` is exactly [{`mesh.nx`, 16, 10}]. The cut and the large-value
  rule of J5, on 120 entries and on a value of 2 000 bytes (a unit test of the function that writes `changes`): 100 entries with
  `changes_total` 120, and that value as `{bytes: 2000, hash}`.
* **C3-S (sequence numbers).** Two threads, 200 accepted edits each through `replace_if_hash`: the journal holds 400 lines whose
  `seq` are 1 to 400, each once, increasing in file order, and the `old_hash` to `new_hash` chain of `edit_lock_w48f.rs` holds.
  A journal holding a held lock file: an append waits at least 250 ms and writes a line without `seq`; a lock file older than 5 s
  is removed and the line has `seq` and `journal_lock_broken_s`.
* **C3-O (lines written before F-273).** A journal holding, byte for byte as main 5fdde2d writes them, an `edit` line and a
  `stale_lock_broken` line, then a line that is not JSON, then a torn line: the feed's reader (from the start) gives two entries
  without `seq` or `actor`, `skipped_lines` 1, and leaves the torn line; the next append writes `seq` 1 on a line of its own; the
  reader then gives that entry and still counts one skipped line.
* **C3-F (the feed, one process).** (1) An `edit_case` gives one `journal` entry and no `external` entry for its path. (2) A
  `std::fs::write` of `notes/x.md` gives one `external` entry with that path, no actor and no changes, within 4 polls of 250 ms.
  (3) The race of J6 made on purpose: the file written with `atomic_write`, a poll, then the matching `edit` line appended, then
  polls for 1 s: one `journal` entry and no `external` entry. (4) A run made with `Project::create_run` (its creation event)
  and given a `state` event, a `warning`, a `stop` and two `solver` events: four run entries (`run_state` twice, `run_warning`,
  `run_stop`) and none for `solver`; no `external` entry for any path under `runs/`. (5) The project folder's files and bytes are the same before and
  after 20 polls. (6) `since` older than the kept entries reports `dropped`.
* **C3 (ADR 0173 C3, two processes).** The `rcfd-mcp` binary serves a project; the test process polls a `Feed` every 250 ms.
  An `edit_case` with `reason` made through the binary appears in the test process's feed within 4 polls (1 s) after the tool's
  result is read, with `actor.client` `mcp`, `actor.instance.pid` the child's process id, `changes` [{`physics.nu`, old, new}] and
  the reason; the number of polls is recorded. A `std::fs::write` by the test process appears as `external`, with its path only,
  within 4 polls. The other way: a `save_case_by` with actor `studio` in the test process appears in the binary's `read_feed`
  with that actor and its `changes`. Control: an `edit_case` without `reason` has no `reason` field.
* **C3-R (runs, MCP binary and runner).** `start_run {case, reason}` writes one `run_submitted` line with the run id, the case,
  `actor.client` `mcp` and the reason; a `cancel_run` of a long run one `run_control` line; after the run ended, `read_feed` holds
  `run_state` entries for that run ending in `cancelled`, and a `pause_run` of the ended run (1002) writes no line.
* **C3-St (the Studio, headless).** A project open in the Studio with the watcher at 500 ms and a case in the editor; a workspace
  in the test process with actor `mcp`: (1) an `edit_case` of the open case with a reason: within 4 Studio periods (2 s) the
  editor holds the new value and a message names `mcp`, the setting, the old and new values and the reason; (2) a `new_case`:
  the browser lists it and a message names it; (3) control: the Studio's own save writes a line with `actor.client` `studio`,
  `who` `save_case` and `changes` [{`mesh.nx`, 16, 10}], and adds no message naming `studio`; (4) control: an `edit_case` from a
  workspace without an actor reloads the editor and adds no feed message. A case made with the case starter writes a
  `case_created` line with `who` `case_starter` and the Studio's actor.
* **C-L (tool list).** Under `--project`: 56 tools. `read_feed` comes right after `list_changes`, access `read`, required `[]`,
  `additionalProperties` false. The eight tools of J4 differ from the F-272 list only by the property `reason`. Without
  `read_feed` and with those eight properties removed, the list is the one ADR 0187's C-L requires, byte for byte. The initialize
  instructions under `--project` name `read_feed`. Without `--project`, `default_f1.rs` passes unchanged.
* **C-1 (parity).** With no edit to them: `edit_lock_w48f.rs` and the other tests of `rcfd-project`, `save_f270.rs`,
  `mesh_f272.rs`, `runner_stdio_w48l.rs`, `sv301_mcp_w48z.rs`, `correctors_w49j.rs`, `new_simulations_f271.rs`,
  `mesh_setup_f272.rs`, `default_f1.rs`, and every test of `rcfd-studio` (`workspace_f270.rs`, `watch_b6.rs`, `editor_b5.rs` and
  the rest). Edited by design, and only there: `new_case_f271.rs` C-N (the assertion that no journal exists after five creations
  becomes: the journal holds exactly five `case_created` lines, one per new file); `project_list_f270.rs` (C-L); the tool counts
  of `project_runs_w48f.rs` (two, 55 to 56) and `project_w48f.rs` F3 (`33 + 22` to `33 + 23`); the test helper
  `rcfd-mcp/tests/common/mod.rs` gains the child's process id. `workspace_graph.rs` passes.
* **Can fail.** Three mutations, each restored and the file compared byte for byte with the kept one afterwards: (1) the wait
  of J6 set to 0: C3-F (3) labels the edit `external`; (2) the journal lock of J2 not taken: C3-S finds a repeated or missing
  `seq` (or the result is reported as it is, if two threads do not collide); (3) the MCP adapter's actor removed: C3 fails on
  `actor.client`. `scripts/limit_audit.py` is run on the new tests.
* **Defaults.** No solver result, case hash or existing case byte changes; `baseline_hashes.rs` and the H1 hashes untouched;
  no case-schema key or run-record field added; no command result gains a key; `Cargo.lock` unchanged (no dependency added). The
  full gate (fmt per package, clippy with `-D warnings`, every test in release on a `git archive` copy, one process per test,
  three jobs) reports no failure; a flaky failure is reported as a failure with its name.

## Not in F-273

The run worker, a feed panel and a `notes/` list in the Studio (Q5); design entries (F-274); hiding session tools and the plugin
hook (F-275); a link to a Claude Code session (Q4); replaying the journal from a sequence number (Q7); the runner pushing
progress (ADR 0173 D3 (b)).

## Where to go next

* [ADR 0173](0173-workspace-layer-between-the-project-folder-the-mcp-server-and-the-studio.md) for the layer and its decisions.
* [ADR 0185](0185-adr-0173-amendment-1-f270-registration-the-rcfd-workspace-crate.md) for the write rule and the Studio's save.
* [ADR 0135](0135-wave48f-plugin-and-project-tools.md) for the edit lock and the first journal line.
* [MCP tools](../mcp-tools.md) for the project tools.

## Amendment 1 (2026-11-05, before the change it covers)

The first run of the C3-O workspace test found that C3-O contradicts J2. J2 closes a torn last line with a newline before the
next line; the torn bytes are then a complete line that is not JSON, which the reader skips and counts. So after the append the
reader counts two skipped lines, not "still one" as C3-O said. C3-O's last clause becomes: the reader then gives that entry and
counts two skipped lines (the line that was not JSON, and the torn line closed by the append). Nothing else changes.

## Amendment 2 (2026-11-05, before the change it covers)

C-1 and item 3 miscounted the creations of `new_case_f271.rs` C-N: the test makes six case files (two examples, the template,
TOML, settings and a copy), not five. The edit by design of C-1 becomes: the journal holds exactly six `case_created` lines,
one per new case file. Item 3's "after five creations" reads "after six creations".
