# P3 addendum: ADR threads

Read `wiki/tools/ingest-agent-brief.md` first; everything there applies. This adds the ADR rules.

## Input

Your ADR list is `wiki/.handoff/p3-<job>-adrs.md`: rows of the generated `docs/adr/index.md` (ADR, title, status word, date, addenda count, item, item status). The ADR files are in `docs/adr/` of the snapshot worktree. The roadmap area pages `wiki/entities/roadmap-*.md` already hold a generated table of every item (status and gap) from `docs/roadmap.json`.

## Reading budget

You can't read ~100 ADRs in full. For each ADR, read the title, the status line and the first paragraph. Then read the decision or outcome: the `Addendum N: results` sections (the last one counts), the "decision by the registered rule" text, "Open questions for the maintainer" and "Answers of the maintainer". Skim the checks table: rows, limits and measured values. Read an ADR in full only when it is a hub: many others cite it, it sets a convention, or it is a plan/proposal that frames the thread.

## Threads

- A **thread** is a chain of ADRs on one problem. It is usually one F-item and its follow-ups (`NNNN-fMMM-...` share MMM), and sometimes several items on one subject (e.g. the PISO stability series). Group your ADRs into threads: aim for 4-15 threads per job, each covering roughly 3-40 ADRs.
- One source page per thread: `wiki/sources/adr-thread-<topic>.md`, with frontmatter `type: source`, `origin: docs/adr/ (ADR list below)`, `origin_commit: 2e94e5ef`. Structure:
  1. `## Current state` (written first, from the newest ADRs): what works, what the latest decision was, what is open. Cite items and ADRs by number.
  2. `## ADRs`: a table with ADR (relative link `../../docs/adr/<file>`), date, kind (registration / amendment / diagnosis / plan / ...), item, question in one line, outcome in one line (met / partial / known miss + the number), superseded-by.
  3. `## Key numbers and limits`: the registered limits and measured values that matter, quoted exactly.
  4. `## Open questions`: maintainer questions still unanswered, with the ADR number.
  5. `## Related`: links to concept, entity and roadmap-area pages.
- Before you create a thread page, check `wiki/sources/` for an existing `adr-thread-*` page on the same subject; another job may have created it. If one exists, add your ADRs to it (additively) instead.
- In the area page(s) of your category (`wiki/entities/roadmap-<area>.md`), write the `## Synthesis` section (replace only the italic placeholder line) and list your threads under `## ADR threads`. Never edit inside the generated block.
- Create concept/entity pages for the methods, cases and components your threads centre on (if they don't exist yet), and add to existing pages additively with citations to the thread page.
- Hub F-items (central to many ADRs, e.g. F-126 for the fan milestone) may get `wiki/entities/f-<nnn>.md`.

## Settle these where your ADRs allow

- Are transient sliding-mesh solves available from a case file? The README says yes (F-303/ADR 0637, F-711/ADR 0646). The roadmap overview says no. Moving-meshes job only.
- Does a structural solver exist? `PRODUCT.md` says no; the architecture page says `rcfd-fem` is partly built. Structural jobs only.
