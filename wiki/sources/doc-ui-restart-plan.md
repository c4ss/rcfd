---
type: source
tags: [studio, plan, claude-code]
sources: []
origin: docs/ui-restart-plan.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: stable
---

# Studio: restart of the desktop UI (plan)

**Origin:** [docs/ui-restart-plan.md](../../docs/ui-restart-plan.md) at `origin/main` 2e94e5ef
**Kind:** plan (2026-10-26), with an update note of 2026-11-16

## Summary

The user's plan of 2026-10-26 to restart the desktop UI from scratch: a wgpu renderer, project folders holding many runs, background and parallel runs, time scrubbing, a Claude Code chat inside the app, and the same project tools available from Claude Code as a plugin. It states "This is a plan, not a claim of capability." The update of 2026-11-16 says the two egui apps (`rcfd-studio`, the frozen `rcfd-gui`) were removed (F-449) before the parity checks of section 6, by the maintainer's decision; the desktop Studio is now the Electron app in `apps/studio-web/` (ADR 0800). The rest of the page is the plan as it was set. Its lasting part is the project folder contract of section 8, which [[studio-guardrails]] G12 makes binding.

## Key points

- **Decisions of 2026-10-26 (first round):** (1) renderer wgpu, not OpenGL; (2) restart, not extend: new crate `rcfd-studio`, `rcfd-gui` frozen until parity; (3) projects are folders holding many runs; (4) Claude Code is the assistant, replacing the provider-key sidebar of F-145 as the primary path, with the project tools also shipped as a plugin. Decisions (1) and (2) were overtaken by the 2026-11-16 update (Electron app, ADR 0800; egui apps deleted, F-449).
- **Seven layers, each testable without a window:** L1 project format, L2 run service (detached runner: queue, N concurrent runs under a CPU and memory budget, pause, resume, cancel, crash recovery, append-only status files), L3 time-series store, L4 render core (wgpu), L5 app shell (egui on eframe's wgpu back end), L6 Claude Code bridge, L7 Claude Code plugin (an MCP server of project tools extending `rcfd-mcp`, skills, slash commands).
- **Wave order:** wave 47 foundations (L1, L2, L3, L4 v0, L6 research); wave 48 shell + plugin; wave 49 compute rendering, animation, video, compare; wave 50 computer-fan modes; "stress and strain only after a structural solver". Waves 48+ start after the user has run the previous result and reported on it.
- **Limits stated first (section 5):** agents cannot see the window (each wave ends with a view-and-report checklist); "A picture can be wrong and look right" (every colour mode has a value test); wgpu needs a GPU driver; Claude Code integration depends on what Claude Code exposes; "Stress and strain pictures need a structural solver that does not exist yet."; interactive rendering of millions of cells unmeasured.
- **Parity checks before `rcfd-gui` removal (section 6):** open and save a case; settings editor with schema validation; run with live residuals and run control; 2D field view; assistant access; tutorials. Never run: the apps were removed first (update note).
- **Claude Code research (section 7, 2026-10-26, from code.claude.com docs):** transcripts are JSONL under `~/.claude/projects/<dir>/<session-id>.jsonl` with an internal, changing format; headless `claude -p` with stream-json; headless approvals denied unless allowed (the `--permission-prompt-tool` contract NOT DOCUMENTED); two writers on one session are unsafe; `claude agents --json` lists sessions; no third-party IDE host API; plugins via `.claude-plugin/plugin.json`; MCP results warn over about 10 000 tokens, 25 000 default limit.
- **Second-round decisions (2026-10-26):** the sidebar may type into a closed session (`claude -p --resume`) and only mirrors sessions another process has open; tool approvals use a conservative allow-list first, no sidecar, no auto-approval of everything. These became guardrail G11 ([[studio-guardrails]]).
- **Project folder contract (section 8):** `rcfd-project.json`, `cases/`, `meshes/`, `runs/<run-id>/` with run-id `r-<UTC yyyymmddThhmmss>-<4 hex>`; per run `run.json` (immutable), `case.snapshot`, `status.json` (states queued, running, paused, finished, failed, cancelled; replaced atomically), `events.jsonl`, `log.txt`, `residuals.jsonl`, `fields/`, `metrics.json`; plus `designs/` (ADR 0121), `notes/`, `.rcfd/` (runner lock and socket, not versioned). Later changes are additive and go through the lead. Additive fields: study/variant/condition (ADR 0189), `settings.continue_from` and `settings.max_steps` (ADR 0200), correctors with adaptive control (ADR 0160).
- **Crates:** `rcfd-project` (47A), `rcfd-runner` (47B), `rcfd-tsstore` (47C), `rcfd-render` (47D), `rcfd-claude` (47E). Since F-449 and F-838, `rcfd-render` and `rcfd-claude` are removed; the allow-list rules of `rcfd-claude` live on as test support of `rcfd-mcp` (`crates/rcfd-mcp/tests/claude_allow/`).

## Contradictions with the wiki

> [!warning] Contradiction
> The plan says twice (header and §5.5) that a structural solver "does not exist yet". That was true on 2026-10-26. [[doc-process-documentation-review]] (G12) lists "No structural solver" statements as stale, and `docs/structural-solver.md` describes `rcfd-fem`, a finite-element structural solver ([[rcfd-fem]]). The plan does not settle the question; its update note of 2026-11-16 does not mention it. Flagged on [[studio]]. The conflict is within the repo docs.

## Pages this source informs

- [[studio]]
- [[studio-guardrails]]
- [[rcfd-project]]
- [[rcfd-runner]]
- [[rcfd-tsstore]]
- [[run]]
- [[view-and-report]]
- [[claude-plugin]]
- [[work-streams]]
