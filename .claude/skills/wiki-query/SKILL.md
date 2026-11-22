---
name: wiki-query
description: Answer a question from the rcfd wiki (wiki/) using local embedding search, with [[wikilink]] citations, and offer to file the answer as an analysis page. Use when the user asks how rcfd does something, what a model/method/ADR/F-item is, where a milestone stands, or says "ask the wiki" / "wiki:".
argument-hint: <question>
---

# Answer a question from the wiki

Read `wiki/CLAUDE.md` first (conventions: citations, LaTeX, no invented decisions). Commands run from the repo root.

1. **Find pages by meaning, not just by name.** Run
   `python -I wiki/tools/search.py search "<question>" -k 12`
   It fuses keyword (BM25) and embedding (Ollama, local) rankings over every page section. Also skim the matching lines of `wiki/index.md`. If a search term is an exact identifier (ADR number, F-item, case key), add `grep -ril "<id>" wiki --include=*.md`.
   If Ollama is not running, search.py says so: fall back to `index.md` + grep and tell the user search was degraded.
2. **Read the top pages**, then widen one hop where it helps:
   - `python -I wiki/tools/search.py similar <page> -k 8` lists the nearest pages and marks those not linked from it (related material the links miss);
   - follow `builds_on:` down for the foundations of a method, and `## Related` / `## How rcfd uses it` up to rcfd's use;
   - for ADR or item status, the `adr-thread-*` and `roadmap-*` pages; for numbers, the `doc-reference-data-*` page.
   Use files in `docs/` or `wiki/raw/` only to check a specific claim.
3. **Answer** in the form that fits: prose, a table, a Mermaid diagram, equations in LaTeX. Cite pages inline as `([[page]])`. Keep what the wiki supports apart from your own inference, and say which is which. Never present a maintainer decision that the sources don't record.
4. **If the wiki can't answer**, say so, and name what would: a repo doc or ADR not yet ingested (check `wiki/analyses/stale-report.md`), an open source to read (check `wiki/analyses/open-gaps.md`, which lists sources already tried), or a question for the maintainer.
5. **File it if it's worth keeping** (a comparison, a status roll-up, a derivation, a gap map, a new connection): offer to save it as `wiki/analyses/<slug>.md` from `wiki/templates/analysis.md` with the question at the top. When saving: link it from the pages it draws on, run `python -I wiki/tools/build_index.py`, append `## [YYYY-MM-DD] query | <question>` to `wiki/log.md` (note "filed as [[slug]]"), and run `python -I wiki/tools/lint.py`. If not saved, still append the log line with "not filed".
