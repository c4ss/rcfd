---
version: 1
slug: "apps-studio-web-src-renderer-first"
primary_target: "apps/studio-web/src/renderer/first/FirstPath.tsx"
related_targets: ["apps/studio-web/src/renderer/first","apps/studio-web/src/renderer/project/ProjectHome.tsx","apps/studio-web/src/main/project/home.ts","apps/studio-web/src/shared/actions/first.ts"]
---

# A learner's first path (Electron Studio, F-813)

Scope: the first-run path of the Electron Studio on the project home: with no project open, a folder the engine refuses
as no project, and an open project that holds nothing but the quick-start example, through make, run, watch and the
first result. Built inside the established DESIGN.md world (Aicher system); no new tokens, no new hue jobs.

Mode: Operate.

Audience and job: a learner or undergraduate who has never run a CFD solver (PRODUCT.md, first users). They want to see
the solver produce something real in a few minutes, with honest limits, and to find the full controls when they are
ready. Frequency: once or twice per person; then the path gets out of the way (it disappears once the project holds a
case of their own, and can be hidden).

Proof and content: the example is the engine's built-in template `lid_driven_cavity` (crates/rcfd-workspace,
templates.rs); every number on the result is the engine's (`run_status`, `metrics.json`, the run's field store); the
disclaimer is README.md's own wording. Nothing synthetic, no validation claim.

Constraints: guardrails G2 and G12 (the Studio writes nothing to a project folder; the engine has no create-project
entry point, so an empty folder is shown as not a project yet and watched), G6 (limits, disclaimer and failures
visible; an unavailable action shown with its reason), G8 (keyboard walk, focus moves to the next step's action), G15
and G19 (full control and precise terms one step away), G17 (the same run from the command line), G22 (the Quiet Rule:
no standing prose at rest). The empty window's first h1 stays "No project open"; F-805's empty-state sentences stay
unique on the page.

States and ranges: no project; empty folder or folder with files but no project file (watching or not); make (template
offered or not; engine failure); run (start failure); watch (queued, running, paused; runner slow); result (converged;
finished without settling; failed; cancelled; no stored fields); hidden.

## Direction contract

THESIS: The path is a wayfinding strip of four stations over one action at a time. The station tells where you are, the
button what to do; everything else is one step away. It refuses the onboarding wizard of modal cards, the checklist
with progress percentages and the paragraph-per-step tutorial.

OWN-WORLD: DESIGN.md unchanged: ink-ruled strip, the current station as the deep ink field with a white numeral, a drawn
ink check for a passed station (meadow stays for converged runs), the number grid for results, fail-wash only for
failures, orange only for focus, the ink disclaimer footer.

STORY: The learner opens a folder; if it is empty they are told in one line what is missing and what to do, and the
folder opens by itself when the project file appears. They press Make the example, then Run the example, watch the
round count rise, and see the result: three numbers, the flow picture, and what the result is not.

FIRST VIEWPORT: Under the project strip, a band: "Your first result", the station strip, a close icon. Under it the
current step: one primary action and Explain. On the result: the state pictogram with the run id, the number grid, the
limits note, Open the run in Runs, Edit the case, Explain; on the right the field viewport with a plain-word field
toggle. The index of the folder stays below.

FORM: wayfinding strip plus single-action step; help on demand (Explain) holds the plain explanation, full control,
precise terms and the same run from the shell.

FINISH: screenshots at 1440x900 in apps/studio-web/test-results-visual/f813/ for the maintainer's view-and-report;
proposed DESIGN.md additions (station strip, help on demand) in the F-813 report.
