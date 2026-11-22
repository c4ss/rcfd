---
name: wiki-ingest
description: Ingest a source into the rcfd wiki (wiki/) — a repo doc or ADR that changed, a loop merge, or an external open reference (paper, NASA report, lecture notes) — using embedding search to route it to the pages it touches. Use when the user says "ingest", "add this to the wiki", after a loop merge that adds an ADR/F-item/doc, or when wiki/analyses/stale-report.md lists work.
argument-hint: <repo path (docs/..., docs/adr/NNNN-...), URL of an open reference, a raw/ file, or "stale" for the stale-report backlog>
---

# Ingest a source into the wiki

Read `wiki/CLAUDE.md` first. Commands run from the repo root. The repo is read-only from the wiki's side: never edit `docs/`, `crates/`, ADRs or `raw/` files.

1. **Identify the source and its rules.**
   - *Repo doc or ADR:* source page is `sources/doc-<slug>.md` (one per doc, or per section for split docs such as `docs/reference-data.md` and `docs/case-schema.md`) or the ADR's thread page `sources/adr-thread-<topic>.md`. Record `origin:` and `origin_commit:` (the commit you read).
   - *"stale":* run `python -I wiki/tools/stale.py --write` and take its list in order: changed sources, new doc sections, new ADRs, roadmap regeneration (`python wiki/tools/gen_roadmap_pages.py "<commit>"`).
   - *External reference:* source page `sources/ref-<short-name>.md` with `origin:` (URL), `licence:` (as stated on the page), `read:` (date). Read it yourself; never write from memory. Copy into `wiki/raw/` (and set `raw:`) only if the licence is verified as public domain / US Gov public use / CC0 / CC BY; NC material and Wikipedia are cite-only; vendor documentation is never a source (clean-room).
2. **Read it all.** For PDFs whose maths does not survive text extraction, render the pages and check equations against the images.
3. **Route it with search before writing.** Run `python -I wiki/tools/search.py search "<title + 3-6 key terms>" -k 15` to find the pages that already cover this ground. For each concept the source introduces, search its name first: if a page scores high, extend that page instead of creating a near-duplicate.
4. **Discuss before writing** (unless told to batch): 3-7 takeaways, anything that contradicts existing pages, and what changes `overview.md`.
5. **Write the source page** from `wiki/templates/source.md` (thread pages: `## Current state`, `## ADRs` table, `## Key numbers and limits`, `## Open questions`, `## Related`).
6. **Spread the content.** Then run `python -I wiki/tools/search.py similar <source-page> -k 15`: pages marked NOT LINKED are candidates to update or link. Update entity/concept pages additively, citing `([[source]])`; add the source to their `sources:`; set `updated:`. Contradictions get a `> [!warning] Contradiction` callout naming both sources; superseded claims are struck through with the newer source cited, never deleted. Equations in LaTeX, one line per paragraph.
7. **Update the catalog and timeline:** `python -I wiki/tools/build_index.py`; revise `overview.md` if the big picture moved; append `## [YYYY-MM-DD] ingest | <source title>` with bullets for pages created and updated. For a loop merge use `## [YYYY-MM-DD] ingest | <ADR/F/doc id> (loop merge)` and commit with the merge.
8. **Check:** `python -I wiki/tools/unwrap.py`, `python -I wiki/tools/lint.py` (fix errors), and `python -I wiki/tools/stale.py` (the source should no longer be listed). Anything you could not read or cover goes in your report and, for jobs with a handoff, under `## Not ingested` so `build_gaps.py` keeps it.
9. **Report:** pages created and updated, contradictions, open questions for the maintainer, and next sources worth reading.
