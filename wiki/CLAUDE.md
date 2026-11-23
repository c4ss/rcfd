# rcfd wiki — schema

This folder is an Obsidian vault and an LLM-maintained wiki (pattern: Karpathy, "LLM Wiki"). The LLM owns the wiki pages. The human curates sources, asks questions, and edits this schema. This file is the contract; change it together with the maintainer when a convention stops working.

## Layers

| Layer | Where | Who writes | Rule |
|---|---|---|---|
| Raw sources | `raw/` (+ `raw/assets/` for images/attachments) | Human | Immutable. Read, never edit, rename, or delete. |
| Repo sources | `../docs/`, `../docs/adr/`, `../crates/`, `../cases/`, `../validation-data/`, `../CHANGELOG.md` | Repo workflow | Read-only from the wiki's point of view. The wiki summarizes and links; it never edits them. |
| Wiki | `index.md`, `log.md`, `overview.md`, `sources/`, `entities/`, `concepts/`, `analyses/` | LLM | Create, update, cross-link freely. |
| Schema | `CLAUDE.md` (this file), `templates/` | Human + LLM | Co-evolved. Ask before changing conventions. |

The repo docs stay the source of truth for decisions (ADRs, F-items). The wiki is the compiled, cross-linked understanding on top of them. When the wiki and a source disagree, the source wins and the wiki page gets fixed.

## Page types

| Folder | One page per | Examples |
|---|---|---|
| `sources/` | ingested source (raw file, repo doc, ADR cluster, paper) | `sources/karpathy-llm-wiki.md`, `sources/adr-0115-mrf.md` |
| `entities/` | concrete thing in or around rcfd | a crate, a solver, a case, a validation dataset, a tool, a person-role |
| `concepts/` | idea / method / phenomenon | SIMPLE, MRF, VOF, y+, under-relaxation, mesh quality |
| `analyses/` | filed query answer, comparison, investigation | "MRF vs sliding mesh for FAN-01" |
| `overview.md` | — | the evolving top-level synthesis of the whole wiki |

Start every page from the matching file in `templates/`.

## Conventions

- File names: lowercase kebab-case, `.md`. Title in the H1 may be prettier.
- Links: Obsidian wikilinks `[[page-name]]` (no folder, no extension) between wiki pages. Links to repo files: ordinary relative markdown links, e.g. `[ADR 0115](../../docs/adr/0115-....md)`.
- Frontmatter on every page (Dataview reads it):
  ```yaml
  ---
  type: source | entity | concept | analysis
  tags: []
  sources: []        # source page names this page draws on, e.g. [karpathy-llm-wiki]
  created: YYYY-MM-DD
  updated: YYYY-MM-DD
  status: stub | draft | stable
  ---
  ```
- Every factual claim on entity/concept/analysis pages cites a source page: `([[source-page]])`.
- Contradictions are never silently resolved. Add a `> [!warning] Contradiction` callout naming both sources, and record it in the log.
- Superseded claims: strike through (`~~old~~`) with the newer source cited, don't just delete.
- Keep pages focused. Split when a page passes ~400 lines or covers two concepts.
- No hard wraps: one line per paragraph, list item or callout line (Obsidian renders every newline as a line break). Fix with `python tools/unwrap.py` (idempotent; `--check` reports).
- Equations in LaTeX, which Obsidian renders with MathJax: inline `$...$`, display `$$...$$` on its own line (one line, no wrap). Symbols and variables in running text too (`$\omega$`, `$y^+$`). Code, case keys, file names and literal values from the repo stay in backticks (`solver.tolerance_u`, `k_omega_sst`). Keep the source's equation number next to the equation, e.g. `$$\tau = \rho a_1 k$$ (Menter 1992, eq. 15)`. (Maintainer decision 2026-11-22.)
- Dates are absolute ISO (`2026-11-22`), never "yesterday".
- No invented decisions: the wiki records what sources say. Open questions go in an `## Open questions` section, not into the text as if decided.

## Special files

- `index.md` — catalog of every wiki page, grouped by type, one line each: `- [[page-name]] — one-line summary (n sources, updated YYYY-MM-DD)`. Updated on every ingest and every new page. On a query, read `index.md` first, then drill down.
- `log.md` — append-only, newest at the bottom. Every entry starts with `## [YYYY-MM-DD] <op> | <title>` where `<op>` is `ingest`, `query`, `lint`, `schema`, or `init`. Last entries: `grep "^## \[" log.md | tail -5`.
- `overview.md` — the synthesis. Revise when an ingest changes the big picture.

## Operations (the loop)

Each operation has a Claude Code skill in the repo's `.claude/skills/` (adapted 2026-11-22 from the brewtiful wiki setup): `/wiki-ingest <source | stale>`, `/wiki-query <question>`, `/wiki-lint`. The skills hold the step-by-step; the summary below is the contract.

### Ingest — "ingest <file or path>"
1. Raw file: the human drops it in `raw/` (web articles via Obsidian Web Clipper; images in `raw/assets/`). Repo doc: just name the path.
2. Read the source fully. For images, read the text first, then open referenced images separately.
3. Tell the human the 3-7 key takeaways and anything that contradicts the wiki. Wait for steer unless told to batch.
4. Write `sources/<name>.md` from `templates/source.md`.
5. Update or create the entity/concept pages it touches (often 5-15). Add backlinks both ways.
6. Flag contradictions (callout + log).
7. Revise `overview.md` if the big picture moved.
8. Update `index.md`.
9. Append the log entry: `## [date] ingest | <source title>` + bullet list of pages touched.

### Query — "wiki: <question>"
1. Read `index.md`, and run `python tools/search.py search "<question>"` (local hybrid keyword + embedding search; needs Ollama running) for pages the index lines don't name. Then read the relevant pages; fall back to sources/raw only to fill gaps.
2. Answer with citations to wiki pages (`[[...]]`) and, where useful, repo files.
3. If the answer is worth keeping (new synthesis, comparison, non-trivial lookup), file it as `analyses/<name>.md`, link it from the pages it draws on, add it to `index.md`.
4. Append `## [date] query | <question>` noting whether it was filed.

### Lint — "lint the wiki"
1. Run `python tools/unwrap.py`, then `python tools/lint.py` (from `wiki/`): broken links, orphans, pages missing from the index, missing frontmatter, stale `updated` dates, math left in backticks.
2. Refresh the three generated reports (each rewrites only its block between markers):
   - `python tools/stale.py --write` → [[stale-report]]: source pages whose repo origin (file, section or ADR) changed since their `origin_commit`, the pages citing them, new ADRs and new doc sections without a page. Staleness by provenance, not by date.
   - `python tools/build_gaps.py` → [[open-gaps]]: the "Not ingested" and "Contradictions" lists of every job handoff, kept in git.
   - `python tools/search.py report` → [[related-candidates]]: very similar pages that don't link each other, overlapping concepts, similar pages with disjoint sources (needs Ollama).
3. Then the judgement pass the scripts can't do, starting from those reports: contradictions between pages, claims superseded by newer ADRs/sources, concepts mentioned in 3+ pages without their own page, missing cross-links, gaps worth a new source.
4. Fix what's mechanical; list the rest for the human with suggested sources/questions.
5. Append `## [date] lint | <summary>`.

When re-ingesting a repo source after [[stale-report]] lists it, update the page and set its `origin_commit` to the commit read, so the next report starts from there.

## Search

The wiki is past the size where `index.md` alone works (700+ pages), so search is local and embedding-based: `tools/search.py` splits each page at its `##` sections, embeds them with Ollama (default `qwen3-embedding:0.6b`; `WIKI_EMBED_MODEL` and `OLLAMA_HOST` override), caches the vectors in `.handoff/embeddings.json` (gitignored; only changed sections are re-embedded) and fuses the vector ranking with BM25 keyword ranking. Nothing leaves the machine.
- `search "<question>"`: Query step 1, and ingest routing (which pages a new source touches; whether a concept page already exists).
- `similar <page>`: nearest pages, marking those not linked (one-hop widening, orphan fixing).
- `report`: [[related-candidates]] for lint.

Exact identifiers (ADR numbers, F-items, case keys) are still best found with grep. Without Ollama, fall back to `index.md` + grep.

## Obsidian setup notes

- Open `wiki/` (not the repo root) as the vault.
- Settings > Files and links > Attachment folder path is preset to `raw/assets`.
- Bind "Download attachments for current file" to a hotkey (e.g. Ctrl+Shift+D) so clipped images are local.
- Templates core plugin points at `templates/`.
- Useful community plugins (optional): Dataview (frontmatter queries), Marp (slides from pages).
- Graph view shows hubs and orphans — a quick visual lint.

## Steady-state ingest (maintainer decision 2026-11-22)

Every loop merge to `main` (roadmap loop, studio loop, any other) that adds or changes an ADR, a `ROADMAP.md` F-item, or a doc under `docs/` or the repo root also updates the wiki in the same session:
1. Add the ADR to its thread page in `sources/adr-thread-*.md` (new thread page if none fits); update the F-item row on its `entities/roadmap-*` page; update the source page of a changed doc.
2. Update the entity/concept pages whose claims changed; flag contradictions.
3. Update `index.md` and append `## [date] ingest | <ADR/F/doc id> (loop merge)` to `log.md`.
4. Commit the wiki changes with the merge (or right after it), same rules as the merge. Subagents in a loop may draft wiki edits for their own item; only the lead session edits `index.md`, `log.md` and `overview.md`.

Until the initial ingest (`ingest-plan.md`) has built the thread pages, a loop merge only appends a `log.md` entry and an `## Ingest backlog` line in `ingest-plan.md`; the planned phase picks it up.
