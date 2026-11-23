---
type: source
tags: [studio, design, themes]
sources: []
origin: docs/design-themes-proposal.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: stable
---

# Proposal: Dark and High contrast themes (F-818)

**Origin:** [docs/design-themes-proposal.md](../../docs/design-themes-proposal.md) at `origin/main` 2e94e5ef
**Kind:** design proposal, approved

## Summary

Status: "approved by the maintainer on 2026-11-17 and pasted into `DESIGN.md`" (its frontmatter `themes` and its Themes section). The decision record is ADR 0801 Addendum 17. On 2026-11-17 the maintainer asked for "a dark mode plus one other theme"; the second theme is **High contrast**, a light theme for bright rooms and projectors. Themes are chosen on the settings page (Ctrl+, or the Settings icon) or by the palette's Theme commands. Values live in `apps/studio-web/src/renderer/design/tokens.ts` (`themes.dark`, `themes.contrast`); `tokens.css` is generated from them.

## Key points

- A theme is a value for every colour token of DESIGN.md; jobs stay the same. In Dark, `white` is still the ground and `ink` still the one colour of text, rules and the footer band; the names say the job, not the lightness.
- Kept in every theme: the field colour map (viridis, 256 entries from 9 stops) and legend; field overlays (iso-lines and arrows in ink, streamlines in white, mesh grid at 38 % white, Light values); type, spacing, corners, sizes, lines, motion, pictogram grid. Light stays the default.
- Key values: Dark `ink` `#dce6f2`, `white` (ground) `#0f1c2c`; High contrast `ink` `#0a2240`, `silver` `#7b8794`. Footer tint `#c7d3e2` Light, `#3a5170` Dark, `#dce6f2` High contrast. Dark's popover lift is tinted from the ground (`white` at 60 %), and Dark sets CSS `color-scheme: dark`.
- Why High contrast: PRODUCT.md names learners and undergraduates first, so classrooms and projectors; the mid-light parts of Light fail first (silver hairlines `#c9cdd1`, 1.6:1 on white). High contrast takes silver to 3.7:1 and ink-2 to 9.9:1. "The ratios above describe the choice. They are not a requirement, and no contrast check is added (G8, PRODUCT.md)."
- Rejected candidates: a colour-blind-safe theme (state is already shape plus word, G8; the field map is viridis); a dim dark variant; a print or "paper" theme (belongs to F-813 export).
- Open questions it listed: the light ink "fields" in Dark vs a second dark tone; Dark orange `#c46810` (3.1:1 label, 4.4:1 focus); whether overlays ever follow the theme; a 3 px focus outline for projectors (recommendation: no, keep 2 px).

## Contradictions with the wiki

- Internal: "Open for the maintainer" item 1 still says Dark and High contrast "are labelled proposed in `tokens.ts` and `tokens.css`" until approval, while the status line says they were approved and pasted on 2026-11-17. The status line is newer.

## Pages this source informs

- [[studio-design-system]]
- [[studio]]
