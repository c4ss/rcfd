---
type: source
tags: [security]
sources: []
origin: SECURITY.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: stable
---

# SECURITY.md (security policy)

**Origin:** [SECURITY.md](../../SECURITY.md) at `origin/main` 2e94e5ef
**Kind:** policy page

## Summary

How to report a vulnerability (GitHub private vulnerability reporting: Security, then Report a vulnerability; no public issue) and which parts of rcfd run programs, write files or listen for requests. "Only the `main` branch is supported; there are no releases yet."

## Key points

- `rcfd serve` listens on `127.0.0.1:7878` by default and "has **no authentication**: any local process can call it". Without `--root` it reads and writes files at any path the client names; `--addr` accepts any address, including non-loopback. "Run it only with `--root` and only on loopback."
- `rcfd-runner` starts the solver binary given by `RCFD_BIN`, or the `rcfd` binary next to the runner, and reads per-project settings from `<project>/.rcfd/runner-config.json` ([[rcfd-runner]]).
- Some `rcfd-mcp` tools write files in the project; without `--root` file access is not confined; `--read-only` refuses the tools that change state ([[rcfd-mcp]]).
- The Studio (`apps/studio-web/src/main/claudecode/`) runs the `claude` CLI only to register the engine's project server for a folder (`claude mcp add|get|remove rcfd-project --scope local`) and opens a terminal running it; it gives Claude Code no permissions of its own; Claude Code's own permission settings decide what it may call without asking (ADR 0801 Addendum 23) ([[studio]]).
- Parsers of mesh and result files (`rcfd-io`) read untrusted input ([[rcfd-io]]).
- "rcfd sends no telemetry." The local-model assistant `rcfd-assistant` was removed (F-838); the Studio's only assistant is Claude Code.

## Contradictions with the wiki

- [[doc-architecture]] places the Studio's Claude Code bridge in `apps/studio-web/src/main/claude/`; this page says `apps/studio-web/src/main/claudecode/`, which is the folder present in the snapshot. Within the repo docs.

## Pages this source informs

- [[studio]]
- [[rcfd-runner]]
- [[rcfd-mcp]]
- [[rcfd-io]]
