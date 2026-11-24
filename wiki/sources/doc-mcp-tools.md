---
type: source
tags: [interface, mcp, assistant]
sources: []
origin: docs/mcp-tools.md
origin_commit: 975beadb
created: 2026-11-22
updated: 2026-11-24
status: draft
---

# MCP and API tools

**Origin:** [docs/mcp-tools.md](../../docs/mcp-tools.md) at `origin/main` 09647baa (first read at 2e94e5ef; one clause changed)
**Kind:** reference page

## Summary

Every tool of the MCP server `rcfd-mcp`: access class, service method and purpose, the rules all tools share (access classes, policy, errors, sandbox, limits), the design and ledger tools, and the 25 project tools of `--project` mode with the project journal and the Claude Code plugin's edit hook.

## Key points

- Access classes `read`, `mutate`, `execute`, `write_files`, and `delete` (only `empty_trash`, ADR 0736). `tools/list` marks each with `annotations.readOnlyHint` and `_meta."rcfd/access"`; `_meta."rcfd/needsApproval"` is true for every class except `read` ([[mcp-access-class]]).
- Policy: read-only mode refuses non-`read` tools (1100), an approval hook before every non-`read` call, an allow list.
- Errors: schema mismatch is JSON-RPC -32602; execution failures are tool results with `isError: true` and `{"error": {"code", "message", "data"?}}`; messages are located ([[located-error]]).
- Sandbox: paths resolve against `--root`; `..` and absolute paths outside are 1004. Limits `--max-cells`, `--max-steps`, `--max-run-seconds`, `--max-design-evaluations`.
- Session tool list: 34 tools (`list_cases`, `create_case`, `set_setting`, `run`, `status`, `run_headless`, the `design_*` and `design_ledger_*` tools, `render_image`, `create_project`). With `--project` the 16 session tools and `create_project` are not offered; the other 17 stay; 25 project tools are added, 42 in all.
- Design tools keep state in a study directory (`study.json`, `evaluations.jsonl`); repeating a call is safe (`cached: true`). With `--project`, `design_run_study`, `design_evaluate` and `design_run_operating_map` run on the project's runner and have class `execute` (ADR 0196 B, ADR 0200 S9); matching runs are adopted after a server restart (ADR 0201).
- Project tools: `project_info`, `list_runs`, `run_status`, `read_metrics`, `read_residuals`, `field_summary`, `render_field`, `derive_picture` (ADR 0738), `compare_runs`, `list_changes`, `read_feed`, `read_project_file`, `hash_project_file` (ADR 0734), `edit_case`, `write_note`, `new_case`, `check_case_mesh`, `preview_mesh`, `import_mesh`, `list_meshes`, `runner_config` / `set_runner_config` (ADR 0733), `case_facts`, `refine_case_mesh` (ADR 0737), `list_sites` (ADR 0739), `start_run`, `pause_run`, `resume_run`, `cancel_run`, the trash tools, `wait_run`. Every result stays under 36 000 bytes.
- Project errors: 1200 file changed since its hash was read; 1201 edit lock held 2 s; 1202 no such run/file/field/snapshot.
- Edits are compare-and-replace under a cooperative lock (`replace_if_hash`, `.rcfd/locks/`); writers that skip the lock show up in the feed as `external`. Project journal `.rcfd/edits.jsonl` (ADR 0188), one line per accepted write, with `seq` and `actor`.
- `start_run` applies the field-store plan by default (ADR 0741); ~~"the tool's own description of `store` still names the earlier default until its change is approved"~~ (2e94e5ef). At 09647baa the tool's own description "says so since ADR 0774": the lead removed the clause at the merge of F-862 ([[adr-thread-mcp-tools]]).
- The plugin's edit hook (`rcfd-mcp --edit-hook`, ADR 0195 P6) refuses Claude Code writes under `cases/`, `runs/`, `designs/` (exit 2); it fails open; checked on Claude Code 2.1.319, "behaviour under `bypassPermissions` is not verified".

## Contradictions with the wiki

_None found at ingest._ (Counted: the tool table has 34 rows; 16 session tools plus `create_project` are dropped with `--project`, 17 stay, plus 25 project tools = 42, as the doc says.)

## Changes at 975beadb (F-339)

Two session tools, `design_uq` (tolerance study) and `design_sobol` (Sobol indices), class `write_files`, run the library's `design::uq::run` and `design::sensitivity::run` on the named evaluator `rcfd.fan_rom` and record the study in a design ledger; "They add nothing of their own to the numbers", and "They are not service methods; the server calls the library." The kept session tools of project mode rise from 17 to 19 (ADR 0784) ([[adr-thread-mcp-tools]]).

## Pages this source informs

- [[mcp]], [[mcp-access-class]], [[rcfd-mcp]], [[rcfd-workspace]], [[template]], [[field-store]], [[located-error]], [[project-journal]]
