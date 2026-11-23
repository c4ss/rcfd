---
name: wiki-lint
description: Health-check the rcfd wiki (wiki/) — broken links, orphans, index drift, raw coverage, log form, pages the repo has moved past, open gaps, and embedding-based relatedness (missing links, overlapping pages, contradiction candidates) — then fix the mechanical problems and propose the rest. Use when the user says "lint the wiki", "wiki health check" or "clean up the wiki", and after a batch of ingests.
---

# Health-check the wiki

Commands run from the repo root.

1. **Mechanical checks.** `python -I wiki/tools/unwrap.py`, then `python -I wiki/tools/lint.py`. Fix every error (broken links, frontmatter, index drift via `python -I wiki/tools/build_index.py`, malformed log headings, a source page's missing `raw:` file). For orphans, add links from related pages (use `search.py similar <orphan>` to find them).
2. **Refresh the three reports** (each rewrites only its generated block):
   - `python -I wiki/tools/stale.py --write` → `analyses/stale-report.md`: source pages whose repo origin changed since `origin_commit`, the pages citing them, new ADRs and doc sections with no page, roadmap drift. This is the ingest backlog; hand it to `/wiki-ingest stale`.
   - `python -I wiki/tools/build_gaps.py` → `analyses/open-gaps.md`: every job's "Not ingested" and "Contradictions" lists.
   - `python -I wiki/tools/search.py report` → `analyses/related-candidates.md` (needs Ollama; the first run embeds every section and takes a while, later runs only changed sections): very similar pages that don't link each other, the most similar concept pairs, similar pages with no shared source.
3. **Judgement pass, driven by the reports.**
   - *Missing links:* read both pages; where the relation is real, add a link with a one-line reason in `## Related`. Skip pairs that are similar only in wording.
   - *Overlapping concepts:* check for duplicated scope; propose merge, split or cross-reference for the page-structure review. Never delete or merge pages without the maintainer.
   - *Similar pages, no shared source:* compare their numbers and claims; a real conflict gets a `> [!warning] Contradiction` callout naming both sources and a log note.
   - Also: claims superseded by newer ADRs (start from the stale report), concepts mentioned on 3+ pages without their own page (search the term), `## Open questions` a new source could close (check open-gaps for sources already tried).
4. **Fix the mechanical problems directly. Propose the rest** as a short list; make content changes only after the user agrees.
5. Append `## [YYYY-MM-DD] lint | <one-line result>` to `wiki/log.md` and re-run `lint.py`.
6. End with 3-5 questions or sources worth pursuing next.
