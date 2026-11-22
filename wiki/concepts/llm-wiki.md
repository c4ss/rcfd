---
type: concept
tags: [meta, wiki]
sources: [karpathy-llm-wiki]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# LLM wiki

A knowledge base the LLM writes and maintains, rather than a pile of documents searched at query
time ([[karpathy-llm-wiki]]).

## How it differs from RAG

- RAG re-derives understanding from raw chunks on every question; nothing accumulates.
- An LLM wiki integrates each source once: summaries, entity/concept updates, cross-links,
  contradiction flags. Answers compound because good ones are filed back ([[karpathy-llm-wiki]]).

## In this vault

- Raw: `raw/` plus the repo's own docs and code (read-only).
- Wiki: `sources/`, `entities/`, `concepts/`, `analyses/`, `overview.md`, `index.md`, `log.md`.
- Schema: `CLAUDE.md`.
- Loop: ingest → query → lint, every step logged.

## Open questions

- At what size does `index.md` stop being enough and a search tool (e.g. qmd) become worth adding?
