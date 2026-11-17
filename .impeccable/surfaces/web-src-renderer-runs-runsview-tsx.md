---
version: 1
slug: "web-src-renderer-runs-runsview-tsx"
primary_target: "apps/studio-web/src/renderer/runs/RunsView.tsx"
related_targets: ["apps/studio-web/src/renderer/runs","apps/studio-web/src/main/project/runs.ts","apps/studio-web/src/renderer/project/ProjectHome.tsx"]
---

# Run list and run state (Electron Studio, F-806)

Scope: the Runs view of an open project in `apps/studio-web/` (a tab beside the project home): the runs table of
DESIGN.md and the selected run's numbers and state history. Built inside the established DESIGN.md world (Aicher
system); no new tokens, no new hue jobs. Inferred from the project home brief and the prototype run-workspace brief
(no interview was possible: the item was built by a subagent); the maintainer may correct it.

Mode: Operate.

Audience and job: a learner or engineer whose runs are going on outside the Studio (started from the CLI, Claude Code
or, from F-807, the Studio). They look for which runs are going, which ended and how, which failed and why, and they
come back after closing the Studio and expect to see the truth. Frequency: whenever a run is going; left open.

Proof and content: every row and number is the engine's answer (`list_runs`, `run_status` through the read-only
project server); the runner's and the solver's own sentences are the reasons. Time is shown only where it is a
simulated time (a steady solver writes 0; shown in words). Report values carry no unit because the engine gives none.

Constraints: `docs/studio-guardrails.md` G2, G3 (no number the engine did not give, no 0 for "none"), G6, G7 (the run
belongs to the runner; restart shows its true state), G8 (pictogram plus word; keyboard: Tab, arrows, Space selects,
Escape clears; focus in orange), G12, G13. The Fade by Colour Rule: selection fades others by colour, never by opacity
of text, and never fades a failure.

States and ranges: no runs; filter with no match; a run whose status the engine could not read; queued, running,
paused, converged, finished (not converged or end time reached), failed (before the first step, or later), cancelled;
0 to 2000 runs (the list is cut at 2000 with the total stated); not live (feed lost).

Unresolved: paused and cancelled pictograms are proposed, not yet in DESIGN.md; run controls (F-807); plots of the
selected run (F-811).

## Direction contract

THESIS: One ruled table of the project's runs, the selected run pulled forward beside it as a number grid with the
runner's own record of why it is in its state. It refuses the card dashboard of run tiles and the progress-bar wall.

OWN-WORLD: DESIGN.md unchanged: the Table primitive (48px rows, silver hairlines, selected row in sky-wash boxed in ink,
failed row in fail-wash with red word), the number grid (11px ink-2 caption over 22px bold value), ink rules between
modules, orange only for focus.

FIRST VIEWPORT: Under the header band, the project's view tabs (Home, Runs). A 40px tool row: "Runs N", filter field,
state select with counts, the time of the last read. Left: the runs table (Case and run, State, Result). Right 460px:
the selected run, or how to select one.

FINISH: screenshots at 1440x900 in `test-results-visual/f806/`; finish review by the maintainer.
