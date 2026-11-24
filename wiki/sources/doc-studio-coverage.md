---
type: source
tags: [studio, coverage, generated]
sources: []
origin: docs/studio-coverage.md
origin_commit: 975beadb
created: 2026-11-22
updated: 2026-11-24
status: stable
---

# Studio capability coverage

**Origin:** [docs/studio-coverage.md](../../docs/studio-coverage.md) at `origin/main` 709e302e (first read at 2e94e5ef; regenerated at the merge of F-451 for `rcfd duct-modes`)
**Kind:** generated status page (do not edit)

## Summary

Lists what the rcfd engine can do and whether the Electron Studio can reach it, as guardrail G15 asks: every case-schema key, service method, MCP tool and CLI command has a UI path or a recorded, dated exemption that names the roadmap item expected to cover it (F-824, ADR 0800). `npm run capabilities` in `apps/studio-web` asks the binaries (`rcfd serve --stdio` for `service.info` and `settings.schema`, `rcfd-mcp` for `tools/list` with and without `--project`, and the `--help` of `rcfd`, `rcfd-runner`, `rcfd-mcp`), writes `apps/studio-web/capabilities/inventory.json`, and writes the page from the inventory and the hand-kept map `capabilities/coverage.json`. `tests/unit/capabilities.spec.ts` fails on an unmapped capability, a malformed entry, a stale page, or an inventory that differs from the engine (skipped with the reason when no `rcfd` binary is found).

## Key points

- Engine read: rcfd 0.1.0, service protocol `rcfd-service/1` version 1.1, MCP server rcfd 0.1.0 (MCP protocol 2025-06-18).
- Summary table: service methods 69 (20 UI path, 49 exemption, 40 not named by their item); MCP tools 70 (35 / 35 / 26); CLI commands ~~53 (23 / 30 / 23)~~ 54 (23 / 31 / 24); CLI options ~~214 (35 / 179 / 126)~~ 220 (35 / 185 / 132); case schema keys 951 (951 / 0 / 0); monitor format keys 6 (6 / 0 / 0); **all ~~1363, 1070 with a UI path (78.5 %), 293 exemptions, 215 whose item does not name them~~ 1370, 1070 with a UI path (78.1 %), 300 exemptions, 222 whose item does not name them** (at 709e302e).
- The change at 709e302e is `rcfd duct-modes` and its six options (F-451, ADR 0783): seven capabilities without a UI path, exempted under F-809 derived pictures (now 4 commands and 32 options there) and "not named" by it. A new line among the groups no Studio item names: "Acoustics tools (acoustics, fwh, duct-modes of F-451): no Studio item covers them; nearest home derived pictures (F-809)." ([[adr-thread-acoustic-wave-solver]], [[adr-thread-studio-electron]]).
- A case key stands for every value of its enumeration (the schema-driven surface of F-825 offers them all).
- Biggest exemption homes: F-807 run control (29 service methods, 18 MCP tools, 11 CLI commands, 69 options).
- Groups no Studio item names yet (each needs the maintainer: a clause, a new item, or leave it to Claude Code and the CLI): acoustics and 0D reactor; `rcfd call`; starting the service from the window; co-simulation (`cosim.*`); design studies, operating maps, design ledger; service-session events; exports to files; runner log options; line and plane samples (iso-surfaces and streamlines have no engine entry point, ADR 0801 addendum F-809); mesh tools; particle reports; `rcfd post`; restarts, headless runs, status streams; in-memory service-session runs; task workflows; the validation-data pack (`rcfd data`); the site agent (F-848, ADR 0739); `rcfd-mcp` options of F-814; waiting for a run; writing notes.
- Left out of the inventory: `data.*` bulk methods (protocol 1.2, ADR 0350; only in an embedding service; F-804); cosim participants; removed `rcfd-studio`/`rcfd-gui`; per-method argument schemas; exit codes and error codes.
- Engine-side gap proposals (each needs its own ADR and the maintainer, under G4): a `rcfd help --json` command list; an additive `optional_methods` member in `service.info`; `rcfd-mcp --list-tools [--project]`.
- Open for the maintainer: granularity of the map (applied, reversible: methods/tools/commands one by one, case keys by top-level section, CLI options by command); unplanned capabilities pointing at their nearest item; whether the Studio offers in-memory service-session runs; building `rcfd-cli`, `rcfd-runner`, `rcfd-mcp` in the studio-web CI job (F-823) so a stale inventory fails CI.

## Contradictions with the wiki

- The page does not mention structural or solid capabilities at all, so it does not settle whether a structural solver exists (see [[studio]]).
- It counts 951 case schema keys; the settings surface brief in [[doc-impeccable-surfaces]] says "948 leaf keys" in "today's schema". Likely different dates; both within the repo docs.

## Changes at 975beadb (F-339)

Capabilities ~~1370~~ 1372 (MCP tools ~~70~~ 72), 1070 covered (78.0 %), exemptions ~~300~~ 302; `mcp:design_uq` and `mcp:design_sobol` exempt under F-807 (unplanned) as the other design tools.

## Pages this source informs

- [[studio-guardrails]]
- [[studio]]
- [[mcp]]
