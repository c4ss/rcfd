---
type: concept
tags: [interface, project, mcp, studio]
sources: [doc-mcp-tools, doc-service-protocol, doc-run-control]
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

## Related

- [[rcfd-workspace]], [[rcfd-mcp]], [[studio]], [[rcfd-service-protocol]]
