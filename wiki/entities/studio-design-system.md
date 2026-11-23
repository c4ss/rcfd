---
type: entity
tags: [studio, design]
sources: [doc-design, doc-design-themes-proposal, doc-impeccable-surfaces, doc-studio-guardrails]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Studio design system ("The Wayfinding Program")

**What it is:** the visual system of the Electron [[studio]], recorded in `DESIGN.md` from the code-led prototype: one strict Aicher-style signage program (Munich 1972) in which every run, state and event is a grid-built pictogram beside its word, and colour is a short, flat, ordered scale where each hue has a job ([[doc-design]]).
**Where:** [DESIGN.md](../../DESIGN.md); tokens in `apps/studio-web/src/renderer/design/tokens.ts` (`tokens.css` generated); word budgets in `apps/studio-web/src/shared/word-budget.ts`; surface briefs in `.impeccable/surfaces/`.

## Details

- **Hue jobs** (Colour Has a Job Rule): sky = header band and iterating; meadow = active sub-tab and converged; orange = the running run's live segment, plus focus and caret; deep red = failure, always with pictogram and word. One Live Thing; Fade by Colour; No Black, No Grey Text ([[doc-design]]).
- **The field:** colour from the data through viridis (256 entries from 9 stops), the same table drawing the legend; "The palette above never tints a field" ([[doc-design]]); the field is not themed ([[doc-design-themes-proposal]]).
- **The Quiet Rule** is guardrail G22: normal is implicit, exceptions speak; per-surface word budgets enforced by a test ([[doc-design]], [[doc-studio-guardrails]]).
- **Themes:** Light (default), Dark, High contrast, plus a System setting; approved 2026-11-17 (F-818, ADR 0801 Addendum 17); a theme changes values, never jobs ([[doc-design-themes-proposal]], [[doc-design]]).
- **Form:** Archivo only with tabular figures; 0 px corners; 1 px ink rules and silver hairlines, no cards; no shadows except the select popover; 2 px orange focus outline; pictograms on a 14-unit grid; 1180 px minimum width, desktop only ([[doc-design]]).
- **Signature:** run bands on one shared time axis with a single ink scrubber that moves field, residual cursor, monitor readout and snapshot counter together ([[doc-design]]).
- **Surfaces briefed** so far: run workspace (prototype), case editor (F-812), first path (F-813), case settings (F-825), Claude Code panel (F-814), project home (F-805), runs view (F-806) ([[doc-impeccable-surfaces]]).
- Contrast ratios in DESIGN.md describe choices; there is no contrast requirement (G8) ([[doc-design-themes-proposal]]).

## Related

- [[studio-guardrails]]
- [[studio]]

## Open questions

- DESIGN.md's Open section: case editor and project home surfaces, layouts beyond the run workspace, any layout below 1180 px ([[doc-design]]). Briefs now exist for the case editor and project home ([[doc-impeccable-surfaces]]), but DESIGN.md has not recorded them.
- Paused and cancelled pictograms are proposed in the runs-view brief, not yet in DESIGN.md ([[doc-impeccable-surfaces]]).
