---
type: source
tags: [tutorial, studio, mcp]
sources: []
origin: docs/tutorials/desktop-assistant.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Tutorial: the desktop Studio and Claude Code

**Origin:** [docs/tutorials/desktop-assistant.md](../../docs/tutorials/desktop-assistant.md) at `origin/main` 2e94e5ef
**Kind:** tutorial (only the CLI steps executed)

## Summary

Open a project in the Electron [[studio]], add and edit a cavity case, run it, look at it through time, compare two runs, trace a number to its source, and register the project server with Claude Code.

## Key points

- "The tutorial test executes only the command-line steps"; every Studio step is `not-executed`. The Studio's Playwright tests use a fake `claude` program; "Nobody has walked through this page step by step against the running app."
- The Studio is a client of the engine: reads the project through `rcfd-mcp`, runs through `rcfd-runner`, edits through `rcfd serve`. It starts them from the folder of the `rcfd` named by `RCFD_STUDIO_RCFD`; "It never looks for them on `PATH`."
- No installer (F-820); measured on Windows 11, Linux tests pass in CI under a virtual display, macOS not measured.
- Example project: `example-projects/quick-start` on Windows, `example-projects-linux/quick-start` on Linux (a stored mesh equals the mesh its case builds only on the platform that made the run; ADR 0150).
- Case editor Save writes only the changed keys; a file changed by another program is not overwritten (three choices shown).
- Use with Claude Code registers `claude mcp add rcfd-project --scope local -- "<rcfd-mcp>" --project "<folder>"`; the Studio embeds no Claude Code; Claude Code's own prompts govern each tool call.
- Not yet: no installer, no project name field, no iso-surfaces or streamlines, no assistant in the window; the removed egui apps had a runner budgets dialog, a log panel, Refine x2 and more.

## Contradictions with the wiki

> [!warning] Contradiction (within the repo docs)
> Step 5 says "A run started this way stores the fields the case asks for, and no others" and "What the Studio does not do yet" says "Run and Queue… here start the case as written". [[doc-run-control]] (Field store) says Run, Queue… and Run the example "call `start_run` with `store: "plan"` written out ... so they use the same plan". One of the two pages is stale.

> [!warning] Contradiction (within the repo docs)
> "No delete of a run, a result or a case: the engine has none, so the Studio offers none." But step 9 mentions "a case it moves to the trash in Trash", and [[doc-mcp-tools]] lists `trash_run`, `trash_results`, `trash_case`, `restore_trash` and `empty_trash` (ADR 0736). A trash is not a delete, but `empty_trash` "removes for good".

## Pages this source informs

- [[studio]], [[executable-tutorials]], [[rcfd-mcp]], [[rcfd-runner]], [[field-store]], [[run]]
