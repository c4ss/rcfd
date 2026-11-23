---
type: source
tags: [studio, design]
sources: []
origin: DESIGN.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: stable
---

# DESIGN.md (design system of rcfd Studio)

**Origin:** [DESIGN.md](../../DESIGN.md) at `origin/main` 2e94e5ef
**Kind:** design system record (frontmatter tokens + prose)

## Summary

The visual system of the Electron Studio, recorded from the code-led prototype in `apps/studio-prototype/`. Creative North Star: **"The Wayfinding Program"**: the Studio is drawn the way Otl Aicher drew the Munich 1972 Games, one strict program in which every run, state and event is a grid-built pictogram set beside its word, and colour is a short, flat, ordered scale where each hue has a job. White ground, deep ink rules, flat colour fields; density from hairlines, not card padding. It "refuses the grey docked workbench with a single blue accent: no grey chrome, no black, no grey text, no rounded panels". Summarised on [[studio-design-system]].

## Key points

- Frontmatter tokens: colours (`ink` `#1d3a5f`, `ink-2` `#4a6382`, `sky` `#62b4e8`, `meadow` `#6cc078`, `orange` `#f08a24`, `fail` `#9b1c24`, `silver` `#c9cdd1`, `white` `#ffffff`, with wash and deep variants), two themes (`dark`, `contrast`), typography (Archivo only, 30/22/15/13/12/11/10 px), `rounded.none` 0 px, spacing 2-24 px, components.
- **The Quiet Rule (guardrail G22):** normal is implicit; the exception speaks. Visible words only for a failure, a loss, a conflict, something blocked, or a value the user must act on. Word budgets per surface are kept in `apps/studio-web/src/shared/word-budget.ts` and enforced by a test.
- Named colour rules: Colour Has a Job (sky = header band and iterating; meadow = active sub-tab and converged; orange = the running run's live segment plus focus and caret; deep red = failure); One Live Thing; Fade by Colour (text opacity never lowered; failures never fade); No Black, No Grey Text; Data Owns the Field (viridis, 256 entries from 9 stops, `#440154` to `#fde725`).
- Themes section: Light default; Dark (deep ink navy ground, never black); High contrast (for bright rooms and projectors); a fourth setting value, System, follows the OS between Light and Dark. Same Jobs Rule; Field Is Not Themed Rule.
- Typography rules: Tabular Figures (units in the caption, "Time, s"); One Family (Archivo, no mono).
- Layout: fixed desktop grid, 1180 px minimum width, no mobile layout; four rows (48 px header band, workspace, 248 px time panel, 24 px footer); workspace three columns (runs, field, Claude Code) about 1 : 1.4 : 0.72, minimums 400, 480, 300 px. Hairline Density Rule.
- Flat: no drop shadows except the select popover's lift; focus is a 2 px orange outline. All corners 0 px; pictograms on a 14-unit grid with strokes at 0, 45 and 90 degrees. State pictograms: queued (circle), running, converged (check), finished (end bar), failed (broken cross). 135-degree silver hatch means "not yet".
- Signature component: run bands, one 20 px row per run on a shared time axis, split by phase; one ink scrubber line moves field, residual cursor, monitor readout and snapshot counter together, with no easing.
- Field view: three.js WebGPU canvas; camera moves as in Unity's scene view (right button look plus W A S D, middle pan, Alt+left orbit, wheel zoom); 2D views orthographic.
- Disclaimer footer always visible. Open: case editor and project home surfaces, layouts beyond the run workspace, any layout below 1180 px.

## Contradictions with the wiki

- DESIGN.md quotes contrast ratios (e.g. ink on orange 4.59:1, ink-2 6.18:1); guardrail G8 and PRODUCT.md say there is no contrast requirement. Not a contradiction: the ratios describe choices, not a requirement ([[doc-design-themes-proposal]]).

## Pages this source informs

- [[studio-design-system]]
- [[studio]]
- [[studio-guardrails]]
