---
type: source
tags: [meta, wiki]
sources: []
origin: https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f
created: 2026-11-22
updated: 2026-11-22
status: stable
---

# Karpathy — "LLM Wiki" (gist, 2026-04-04)

**Origin:** <https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f>
**Kind:** idea file, meant to be handed to an agent which then builds the specifics with the user.

## Summary

Instead of retrieving from raw documents at query time (RAG), an LLM incrementally builds and
maintains a persistent, interlinked markdown wiki between the user and the raw sources. Knowledge is
compiled once and kept current. Obsidian is the IDE, the LLM the programmer, the wiki the codebase.

## Key points

- Three layers: immutable raw sources → LLM-owned wiki → schema file (CLAUDE.md / AGENTS.md) co-evolved
  by user and LLM. See [[llm-wiki]].
- Operations: **ingest** (one source may touch 10-15 pages), **query** (answer with citations, file
  good answers back as pages), **lint** (contradictions, stale claims, orphans, missing pages/links, gaps).
- `index.md` = content catalog read first on every query; claimed to scale to ~100 sources /
  a few hundred pages without embeddings.
- `log.md` = append-only, greppable entries `## [date] op | title`.
- Obsidian tips: Web Clipper, local attachment folder + download hotkey, graph view, Marp, Dataview.
- The wiki is a git repo of markdown: history and branching for free.
- Why it works: maintenance is what kills human wikis; the LLM makes bookkeeping nearly free.

## Pages this source informs

- [[llm-wiki]]
- schema: `CLAUDE.md`
