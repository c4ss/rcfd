# Proposal: the Dark and High contrast themes of rcfd Studio (F-818)

Status: **approved by the maintainer on 2026-11-17 and pasted into `DESIGN.md`** (its frontmatter `themes` and its
Themes section). The two themes are chosen on the Studio's settings page (Ctrl+, or the Settings icon in the header
band) or by the palette's Theme commands. Their values live in `apps/studio-web/src/renderer/design/tokens.ts`
(`themes.dark`, `themes.contrast`), and `tokens.css` is generated from them. The decision record is ADR 0801
Addendum 17.

On 2026-11-17 the maintainer asked for "a dark mode plus one other theme". The second theme proposed here is **High
contrast**, a light theme for bright rooms and projectors.

## What a theme is

A theme is a value for every colour token of `DESIGN.md`. The jobs stay the same: every component keeps the token
`DESIGN.md` gives it (button `{colors.white}`, footer `{colors.ink}`, and so on). Only the values change. Each token
keeps its hue, so each hue keeps its single job (the Colour Has a Job Rule). In Dark, the token named `white` is still
the ground and `ink` is still the one colour of text, rules and the footer band. Their values are swapped (a deep ink
navy ground, a light ink). The names say which job a token does. They do not say how light it is.

Kept the same in every theme:

- the field colour map (viridis, 256 entries from 9 stops) and its legend: the Data Owns the Field Rule;
- the overlays drawn on the field (iso-lines and arrows in ink, streamlines in white, the mesh grid in 38% white): they
  sit on the colour map, which does not change. They use the Light values;
- type, spacing, corners, sizes, lines, motion and the pictogram grid.

The Light theme is exactly `DESIGN.md` today and stays the default. Its CSS is the unchanged `:root` block.

## Token tables

### Paste-ready frontmatter

These blocks go into `DESIGN.md`'s frontmatter, under the existing `colors:`, as two new maps. `colors:` itself (Light)
does not change.

```yaml
themes:
  dark:
    colors:
      ink: "#dce6f2"
      ink-2: "#9bb0c8"
      ink-hover: "#f3f7fb"
      sky: "#245f8c"
      sky-wash: "#1a3350"
      meadow: "#286b33"
      meadow-wash: "#183626"
      meadow-deep: "#78c884"
      orange: "#c46810"
      orange-deep: "#f3a052"
      fail: "#f07a82"
      fail-wash: "#3b1a22"
      silver: "#34475d"
      silver-wash: "#16243a"
      white: "#0f1c2c"
  contrast:
    colors:
      ink: "#0a2240"
      ink-2: "#2b4466"
      ink-hover: "#1b3d6a"
      sky: "#55ade6"
      sky-wash: "#d2e7f7"
      meadow: "#5cb86a"
      meadow-wash: "#d5ecd9"
      meadow-deep: "#1d6a2a"
      orange: "#e87500"
      orange-deep: "#8f4500"
      fail: "#8a0d17"
      fail-wash: "#f5d6d9"
      silver: "#7b8794"
      silver-wash: "#e3e8ee"
      white: "#ffffff"
```

The footer's light ink tint (`DESIGN.md`, Disclaimer footer, `#c7d3e2` in Light) becomes `#3a5170` in Dark and
`#dce6f2` in High contrast.

### The scale by job

| Token | Job (`DESIGN.md`, unchanged) | Light (`DESIGN.md`) | Dark (proposed) | High contrast (proposed) |
|---|---|---|---|---|
| ink | all text, 1px rules, primary buttons, selected toggle, tooltips, scrubber and flag, footer band | `#1d3a5f` | `#dce6f2` | `#0a2240` |
| ink-hover | hover on primary buttons | `#2a4d78` | `#f3f7fb` | `#1b3d6a` |
| ink-2 | secondary text: captions, run ids, axis ticks, placeholders, units | `#4a6382` | `#9bb0c8` | `#2b4466` |
| sky | header band; iterating phase of a run band | `#62b4e8` | `#245f8c` | `#55ade6` |
| sky-wash | hover, selected run row, your chat message, selects on the band | `#e4f0f9` | `#1a3350` | `#d2e7f7` |
| meadow | active sub-tab, Claude Code head, switch on, converged band phase | `#6cc078` | `#286b33` | `#5cb86a` |
| meadow-wash | sub-tab hover | `#e6f3e8` | `#183626` | `#d5ecd9` |
| meadow-deep | converged pictogram and residual p line on the ground | `#2f7d3b` | `#78c884` | `#1d6a2a` |
| orange | the running run's live segment, focus outline, caret | `#f08a24` | `#c46810` | `#e87500` |
| orange-deep | running pictogram on the ground | `#b35a00` | `#f3a052` | `#8f4500` |
| fail | failure only: text, failed band, failed pictogram | `#9b1c24` | `#f07a82` | `#8a0d17` |
| fail-wash | a failed run's row | `#f7e4e5` | `#3b1a22` | `#f5d6d9` |
| silver | hairlines, input borders, the "not yet" hatch, pressed button | `#c9cdd1` | `#34475d` | `#7b8794` |
| silver-wash | field viewport ground, table row and column hover | `#eef1f4` | `#16243a` | `#e3e8ee` |
| white | the ground, button and input faces, band label chips | `#ffffff` | `#0f1c2c` | `#ffffff` |
| (footer tint) | footer secondary text, on ink | `#c7d3e2` | `#3a5170` | `#dce6f2` |

Tints from `DESIGN.md` (35% white ghost lift, 30% white project-tab hover, 35% ink tab dividers) take each theme's
`white` and `ink`. One exception: in Dark the **popover lift** is tinted from the ground (`white` at 60%) and not
from ink, because an ink-tinted shadow would be a light glow. The popover keeps its 1px ink edge. Dark also sets CSS
`color-scheme: dark`, so native scrollbars and form parts are dark. Light sets none, as before.

## Proposed prose for `DESIGN.md`

For the Key Characteristics, replace "Light theme, desktop only (1180px minimum width)." with:

> - Three themes on one program: Light (the default), Dark and High contrast; desktop only (1180px minimum width).

New section after Colors:

> ## Themes
>
> A theme gives every colour token a value and changes no job. Light is the scale above and is the default. **Dark** is
> for dim rooms and long evenings. The ground is a deep ink navy, never black. Ink becomes the one light colour, and
> the fields that carry ink text (sky, meadow, orange) turn deep so light ink still reads on them. The deep variants
> and failure red, which sit on the ground, turn light. The footer band, tooltips and primary buttons stay ink, so in
> Dark they are light bands with dark text. **High contrast** is Light made for bright rooms and projectors: a deeper
> ink and ink-2, silver hairlines and the hatch dark enough to survive a washed-out projector, and fuller fields. The
> setting has a fourth value, System, which follows the operating system between Light and Dark.
>
> **The Same Jobs Rule.** A theme changes values, never jobs: each token keeps its hue in every theme, and a component
> never takes a different token in another theme.
>
> **The Field Is Not Themed Rule.** The colour map, its legend and the overlays drawn on the field are the same in every
> theme. Only the viewport's ground (silver-wash) follows the theme.

For Open, delete "a dark theme;".

## Why High contrast is the second theme

`PRODUCT.md` names learners and undergraduates as the first users, and that means classrooms and lecture-hall
projectors. `DESIGN.md` sets the scene as "a desk in daylight, reading numbers closely for hours". The parts of the
Light scale that fail first in a bright room or on a projector are the mid-light ones: the silver hairlines
(`#c9cdd1`, 1.6:1 on white), which carry all of the program's structure (the Hairline Density Rule); the "not yet"
hatch; ink-2 captions; and the washes that mark hover and selection. High contrast keeps the program and its hues
and strengthens exactly those. Silver goes to 3.7:1 on white, ink-2 to 9.9:1, and the washes get more colour.

Other candidates were weighed. A colour-blind-safe theme adds nothing: state is already shape plus word (G8), and
the field map is viridis. A dim dark variant would only repeat Dark. A print or "paper" theme belongs to F-813
(export), not to a live theme. The ratios above describe the choice. They are not a requirement, and no contrast
check is added (G8, `PRODUCT.md`).

## Open for the maintainer

1. Approve, change or reject each palette, then paste the blocks above into `DESIGN.md`. Until then, Dark and High
   contrast are labelled proposed in `tokens.ts` and `tokens.css`.
2. Dark draws the footer band, tooltips, the scrubber flag and primary buttons as **light** bands with dark text,
   because they are ink and ink is the light colour in Dark. The alternative is a second dark tone for these
   "ink fields" in Dark only. That would give `ink` two values, one for text and one for fields.
3. In Dark, light ink text on the live orange segment is 3.1:1 (`#c46810`). A brighter orange shows focus better on
   the dark ground (4.4:1 now) but makes the label harder to read. The value chosen is between the two.
4. The field overlays use the Light values in every theme (they sit on the colour map). Should they ever follow the
   theme?
5. High contrast changes only colours. A 3px focus outline for projectors would change a non-colour token.
   Recommendation: no, keep 2px.
