---
type: concept
tags: [interface, project, mcp, studio]
sources: [doc-mcp-tools, doc-service-protocol, doc-run-control, adr-thread-workspace-layer, adr-thread-studio-engine-entry-points, adr-thread-studio-electron, adr-thread-runner-records-and-run-sites]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Project journal, change feed and session journal

Two journals: the project journal `.rcfd/edits.jsonl` records accepted changes to a project folder; the service's session journal (`journal.get`, `rcfd-journal`) records a live session's create entry and accepted settings changes ([[doc-mcp-tools]], [[doc-service-protocol]]).

## How rcfd uses it

- Project journal (ADR 0188): one JSON line per accepted write, case creation, mesh import, run submission and accepted run request, from the MCP server and the Studio alike; `t`, `kind`, `who`, `seq` (under `.rcfd/locks/journal.lock`), `actor` {`client` `mcp` or `studio`, `instance`}, `reason` ([[doc-mcp-tools]]).
- Kinds: `edit` (with `changes` [{`path`, `old`, `new`}], at most 100), `case_created`, `mesh_imported`, `run_submitted`, `run_control`, `stale_lock_broken`, `edit_lock_remove_failed` (ADR 0676) ([[doc-mcp-tools]]).
- The change feed `read_feed` merges journal entries, run events and `external` edits (files changed with no journal line, 500 ms after modification); it keeps 1 000 entries ([[doc-mcp-tools]]).
- `--client <name>` names the actor client, so Claude Code's changes (client `mcp`) can be told from the Studio's (`studio`) ([[doc-mcp-tools]]). "Which Claude Code session an `mcp` actor belongs to is not recorded" (ADR 0188 Q4) ([[doc-mcp-tools]]).
- Session journal: `journal.get` returns `{format: "rcfd-journal", version: 1, entries, case_hash, source, file}`; `rcfd serve --journal-dir` writes one file per session (ADR 0735); `rcfd run --journal <file>` runs it, refusing a changed source file (exit 3) unless `--journal-ignore-hash` ([[doc-service-protocol]], [[doc-run-control]]).

### From the P3 ADR threads (2026-11-22)

- Design (ADR 0173 D4 (c), built by F-273, ADR 0188): an entry keeps `who` (the command) and adds an actor `{client, instance}`, the changed settings and an optional reason, so the Studio can say what changed and why; "Changes found by the watcher with no matching journal entry are labelled `external`" ([[adr-thread-workspace-layer]]).
- Measured: an `edit_case` with a reason reached another process's feed "at poll 1 of 4 (0.004 s after the result was read)", a plain write showed as `external` "at poll 3 of 4 (0.508 s)"; `changes` are cut at 100 (`changes_total` kept), and a value of 2 000 bytes is written as `{bytes, hash}` ([[adr-thread-workspace-layer]]).
- The journal lock is not fair: `LOCK_WAIT` was raised from 250 ms to 2 s on 2026-11-12, below `LOCK_STALE` (5 s); "under heavier contention a line can still be appended without `seq`, which the journal's best-effort contract allows" ([[adr-thread-workspace-layer]]).
- Later kinds: `trashed`, `restored` and `trash_emptied` (ADR 0736); `set_runner_config` journals `changes` per key with the actor and reason (ADR 0733); `run_submitted` carries `continue_from`, `max_steps` and `rerun_of` when set (ADR 0734) ([[adr-thread-studio-engine-entry-points]], [[adr-thread-runner-records-and-run-sites]]).
- Design runs and stored evaluations write no journal line (ADR 0189 Q6, no item registered) ([[adr-thread-workspace-layer]]).
- The Electron Studio polls the feed every 400 ms and shows one activity line for changes of a client other than `studio` (ADR 0801 Addendum 23) ([[adr-thread-studio-electron]]).
- Session journals on disk (ADR 0735): the Studio starts its service with `--journal-dir <state dir>/journals` (D1), "The Studio writes no journal file itself"; a journal write failure does not fail the request (D3) ([[adr-thread-studio-engine-entry-points]]).

## Related

- [[rcfd-workspace]], [[rcfd-mcp]], [[studio]], [[rcfd-service-protocol]]
