---
type: source
tags: [studio, design, surfaces]
sources: []
origin: .impeccable/surfaces/*.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: stable
---

# Impeccable surface briefs (7 Studio surfaces)

**Origin:** [.impeccable/surfaces/](../../.impeccable/surfaces/) at `origin/main` 2e94e5ef (7 files)
**Kind:** design briefs (direction contracts), one per Studio surface

## Summary

Seven design briefs written with the Impeccable design workflow, one per surface of the Studio. Each names its primary source file, scope, mode ("Operate" in all seven), audience and job, proof and content, constraints (guardrails cited by number), states and ranges, unresolved points and a direction contract (THESIS, OWN-WORLD, FIRST VIEWPORT, FINISH). The first is the prototype brief from which [[doc-design]] was recorded; the others build "inside the established DESIGN.md world" and add no new tokens or hue jobs (one exception below). Five of them say no interview was possible: the surface was shaped by an agent or subagent from its roadmap item, and the maintainer confirms or corrects at the [[view-and-report]].

## Key points

| File | Surface | Item | Points of note |
|---|---|---|---|
| `apps-studio-prototype-src-runworkspace-tsx.md` | Run workspace (prototype in `apps/studio-prototype/`, React 19 + Vite + react-three-fiber + three.js WebGPURenderer + React Aria) | prototype | "It is not the product. All data is synthetic and labelled so." Aicher Munich 1972 program; signature is the run bands; brief colours sky `#4FA3DB`, meadow `#5DB36A` (DESIGN.md later lightened them) |
| `apps-studio-web-src-renderer-case.md` | Case editor | F-812 | Save (Ctrl+S) writes only changed settings through the engine's `edit_case` (form `minimal`, lines edited in place); conflict (G18) is a ruled notice with three named choices, none preselected, no red ("a conflict is not a failure"); no TOML/JSON parsing in the Studio; new case from the engine's templates (1 today) and shipped examples (about 70) |
| `apps-studio-web-src-renderer-first.md` | A learner's first path | F-813 | Four-station wayfinding strip; example is the engine's built-in template `lid_driven_cavity`; the Studio writes nothing to a project folder (the engine has no create-project entry point, so an empty folder is "not a project yet" and watched); refuses wizards and checklists |
| `apps-studio-web-src-renderer-settings.md` | Case settings, schema-driven | F-825 | Every key of `settings.schema` gets a control at run time, no Studio-side key list; today's schema: 44 sections, 948 leaf keys, 24 named collections, 17 optional objects; first eight keys of a section shown, the rest behind "Show N more keys" until F-826's advanced flag; the Studio does not clamp, round, convert units or refuse a value |
| `web-src-renderer-claude-claudepanel-tsx.md` | Claude Code panel | F-814 | Drives the user's own Claude Code CLI; one writer per session, mirror read-only, never an approve-everything flag, conservative allow-list (G11); one new token role `chat-own` = sky-wash; session state that cannot be established is "state unknown", read-only; last 50 transcript entries |
| `web-src-renderer-project-projecthome-tsx-41df8cd1.md` | Project home | F-805 | One ruled index of the folder plus a Changes lane; every entry from the engine's project server (`project_info`, `list_runs`, `list_meshes`, `read_feed`); runs cut at 2000 with the total stated; the first h1 of the empty window is exactly "No project open" |
| `web-src-renderer-runs-runsview-tsx.md` | Run list and run state | F-806 | Every row from `list_runs`, `run_status`; time shown only where it is a simulated time; report values carry no unit "because the engine gives none"; paused and cancelled pictograms proposed, not yet in DESIGN.md |

- Guardrails cited across the briefs: G2, G3, G6, G7, G8, G10, G11, G12, G13, G15, G16, G17, G18, G19, G22 (see [[studio-guardrails]]).
- Open for the maintainer (from the briefs): place of the case panel and whether Save also lives in the header band (F-812); the eight-key cut and whether to show the engine's SI dimension strings (F-825); a `claude-head` token role (F-814).

## Contradictions with the wiki

- The prototype brief lists "G8 WCAG 2.2 AA" among its constraints; [[doc-studio-guardrails]] G8 says there is no WCAG contrast requirement (removed by the maintainer 2026-11-16), and the F-825 brief says "(no contrast requirement)". The prototype brief predates the removal. Within the repo docs.
- The Claude Code panel brief names `apps/studio-web/src/main/claude` as a related target, as [[doc-architecture]] does; [[doc-security]] names `apps/studio-web/src/main/claudecode/`, and only `claudecode/` exists in the snapshot. Within the repo docs.

## Pages this source informs

- [[studio]]
- [[studio-design-system]]
- [[studio-guardrails]]
- [[view-and-report]]
