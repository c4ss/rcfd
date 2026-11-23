# Brief for wiki ingest subagents

You are one of up to 4 parallel agents compiling rcfd's repo documentation into the LLM wiki at `E:\projects\cfd\rcfd\.claude\worktrees\wiki-ingest\wiki\` (the "vault"). Read `wiki/CLAUDE.md` (the schema) and `wiki/index.md` first, then skim 3 existing pages to copy their style: `sources/doc-architecture.md`, `concepts/registration.md`, `entities/fan-01.md`.

## The snapshot

- The worktree `E:\projects\cfd\rcfd\.claude\worktrees\wiki-ingest\` is a frozen snapshot of `origin/main` at commit `2e94e5ef`. Read every repo source from this worktree, never from `E:\projects\cfd\rcfd\` (the live checkout, which other agents are changing). Put `origin_commit: 2e94e5ef` in each source page.
- Never edit anything outside `wiki/`. Never run git commands that change state (no commit, checkout, reset, stash, branch, push). The lead commits.
- Never run cargo, tests, or long scripts. Reading and writing markdown only. Python only for `wiki/tools/lint.py`.

## What you may write

- New pages under `wiki/sources/`, `wiki/entities/`, `wiki/concepts/`, `wiki/analyses/`, from `wiki/templates/`. File names lowercase kebab-case; names are global across folders, so check `index.md` and the folders for an existing page before creating one (e.g. `simple`, `piso`, `y-plus`, `amg` may already exist).
- Updates to existing pages: **additive only**. Add bullets or sections, add `[[links]]`, add sources to the frontmatter list and bump `updated:`. Never delete or reword existing text; if a claim is superseded, strike it (`~~old~~`) and add the new claim with its citation. Another agent may be editing the same page: always re-read right before an Edit, keep edits small, and retry on a conflict.
- Do **not** edit `index.md`, `log.md`, `overview.md`, `ingest-plan.md`, `CLAUDE.md`, `templates/`, `tools/`. The lead does those from your handoff file.

## Handoff file (required)

Write `wiki/.handoff/<your-job-id>.md` with:
1. `## Index lines` — one line per page you created, in the index format: `- [[name]] — one-line summary (n sources, updated 2026-11-22)` grouped under `### Sources`, `### Entities`, `### Concepts`, `### Analyses`.
2. `## Pages updated` — existing pages you added to, one line each with what you added.
3. `## Log bullets` — 3-8 bullets for the log entry.
4. `## Contradictions` — each with both sources, and whether it is with the repo docs or within them.
5. `## Overview notes` — at most 6 bullets the lead should consider for `overview.md` (big-picture changes only).
6. `## Not ingested` — anything in your scope you skipped or only skimmed, and why.

## Content rules

- Source pages: one per doc; docs over ~1000 lines get one source page per top-level section, named `doc-<doc>-<section>`; ADR threads are `adr-thread-<topic>` (see the P3 rules in your job prompt).
- Every factual claim on entity/concept/analysis pages cites a source page: `([[source-page]])`. Repo files are linked with relative paths from the page (e.g. `../../docs/meshing.md` from `sources/`).
- **No hard wraps.** Write each paragraph, list item and callout line as one line, however long. Obsidian renders every newline as a line break. Before finishing, run `python wiki/tools/unwrap.py <your files>` from the worktree root.
- Quote numbers, limits and decisions exactly; don't round, don't paraphrase a registered rule loosely.
- Record what sources say. Never invent a decision or a status. Open questions go under `## Open questions`.
- Contradictions: a `> [!warning] Contradiction` callout on the affected page, naming both sources.
- Prefer fewer, richer pages over many stubs. A concept/entity page is worth creating when it is the subject of a doc section or is mentioned across 3+ sources. General CFD concepts (SIMPLE, PISO, k-omega SST, AMG, MRF, VOF, FW-H, y+) get a concept page with "How rcfd uses it" when rcfd docs say something specific about them.
- Before finishing, run `python wiki/tools/lint.py` from the worktree root and fix broken links you caused (pages missing from the index are expected; the lead adds them).

## Final report (under 300 words)

Job id, pages created (count), pages updated (count), handoff path, contradictions found (count), lint result, anything skipped.
