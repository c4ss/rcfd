---
version: 1
slug: "web-src-renderer-project-projecthome-tsx-41df8cd1"
primary_target: "apps/studio-web/src/renderer/project/ProjectHome.tsx"
related_targets: ["apps/studio-web/src/renderer/project","apps/studio-web/src/main/project"]
---

# Project home (Electron Studio, F-805)

Scope: the project home of the Electron Studio in `apps/studio-web/`: the window with no project open (open a folder, the
recent list) and the home of an open project folder (what the folder holds, read through the engine, kept live). Built
inside the established DESIGN.md world (Aicher system); no new tokens, no new hue jobs.

Mode: Operate.

Audience and job: a learner or engineer who has, or has been given, a project folder. They open it, see at once what it
holds (cases, meshes, runs, notes) and whether anything is wrong, and watch it change while Claude Code, the CLI or an
editor works on it. Frequency: at every start of the Studio, and left open while working elsewhere.

Proof and content: every entry and number comes from the engine's project server (`project_info`, `list_runs`,
`list_meshes`, `read_feed`); case and note names come from the folder contract until the engine has a query for them
(F-805 decision D2). The engine's own feed sentences ("cases/b.toml changed outside the workspace") are the change
text. Nothing synthetic.

Constraints: `docs/studio-guardrails.md` G2 (no engine logic), G6 (empty and error states say what is missing and the
next step; never suggest a capability that does not exist: no New case, no Start run, no open-a-case until their items
land), G8 (state as pictogram plus word; keyboard; focus), G12 (no project state of the app's own; the recent list holds
paths and times only), G13 (Playwright, screenshots at 1440x900). The first h1 of the empty window is exactly
"No project open" (F-801 tests).

States and ranges: no project (first start, recent list empty, recent folder gone); engine not configured; opening;
not an rcfd project; engine failed at start; open with 0 to thousands of entries per folder (runs cut at 2000 with the
total stated); a list the engine could not read; project issues from `project_info`; live link lost (feed unreadable)
and project server stopped (Reconnect).

Unresolved for later items: what a click on a case, run, mesh or note opens (F-806, F-812, F-808); new case and run
actions (F-807, F-813, through the action layer F-827); paused and cancelled have no pictogram in DESIGN.md yet (word
only here; F-806).

## Direction contract

THESIS: The project home is one ruled index of the folder, filtered by kind, read through the engine, with a lane that
shows every change as the engine reports it. It refuses the file-tree-plus-preview explorer and the dashboard of
count tiles.

OWN-WORLD: DESIGN.md unchanged: sky header band with the project as a white tab, ink rules between modules, silver
hairlines between rows, Archivo with tabular figures, state pictograms with words, fail red only for failures, orange
only for focus. Rows, not cards.

STORY: The visitor opens a folder, sees its four folders as one index with counts, filters to what they need, and sees a
row marked "changed" seconds after Claude Code or an editor touched the file; when the link breaks they are told in
words and can reconnect.

FIRST VIEWPORT: Header band: brand cell, project tab, Open project, live status word and Close on the right. Under it a
project strip (folder path, created, solver, schema, issues). Then a 40px tool row: filter field and a segmented kind
toggle with counts (All, Cases, Meshes, Runs, Notes). Left two-thirds: the index, grouped by kind, each group headed by
its folder and what it holds. Right 380px: Changes lane, newest first. Ink disclaimer footer.

FORM: command-first index (own list position 7 of 7, dealt second; the dealt lead, a single chronological ledger, was set
aside on a factual ground: the engine gives no modification time for cases, meshes or notes), with the change lane of
position 3 (dealt third) as its signature. Seed key b584871f.

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance
