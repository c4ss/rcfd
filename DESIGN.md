---
name: rcfd Studio
description: A desktop CFD workspace drawn as one strict signage program, where every run, state and event is a pictogram with a word and colour does jobs.
colors:
  ink: "#1d3a5f"
  ink-2: "#4a6382"
  ink-hover: "#2a4d78"
  sky: "#62b4e8"
  sky-wash: "#e4f0f9"
  meadow: "#6cc078"
  meadow-wash: "#e6f3e8"
  meadow-deep: "#2f7d3b"
  orange: "#f08a24"
  orange-deep: "#b35a00"
  fail: "#9b1c24"
  fail-wash: "#f7e4e5"
  silver: "#c9cdd1"
  silver-wash: "#eef1f4"
  white: "#ffffff"
typography:
  display:
    fontFamily: "Archivo, Helvetica Neue, Arial, sans-serif"
    fontSize: "30px"
    fontWeight: 700
    lineHeight: 1
    letterSpacing: "-0.03em"
    fontFeature: "tnum"
  headline:
    fontFamily: "Archivo, Helvetica Neue, Arial, sans-serif"
    fontSize: "22px"
    fontWeight: 700
    letterSpacing: "-0.02em"
    fontFeature: "tnum"
  title:
    fontFamily: "Archivo, Helvetica Neue, Arial, sans-serif"
    fontSize: "15px"
    fontWeight: 700
    letterSpacing: "-0.01em"
  title-sm:
    fontFamily: "Archivo, Helvetica Neue, Arial, sans-serif"
    fontSize: "13px"
    fontWeight: 700
  body:
    fontFamily: "Archivo, Helvetica Neue, Arial, sans-serif"
    fontSize: "12px"
    fontWeight: 400
    lineHeight: 1.35
    fontFeature: "tnum"
  label:
    fontFamily: "Archivo, Helvetica Neue, Arial, sans-serif"
    fontSize: "11px"
    fontWeight: 500
    fontFeature: "tnum"
  chart-micro:
    fontFamily: "Archivo, Helvetica Neue, Arial, sans-serif"
    fontSize: "10px"
    fontWeight: 500
    fontFeature: "tnum"
rounded:
  none: "0px"
spacing:
  hair: "2px"
  xs: "4px"
  sm: "6px"
  md: "8px"
  lg: "12px"
  xl: "16px"
  xxl: "24px"
components:
  button:
    backgroundColor: "{colors.white}"
    textColor: "{colors.ink}"
    typography: "{typography.body}"
    rounded: "{rounded.none}"
    padding: "0 10px"
    height: "26px"
  button-hover:
    backgroundColor: "{colors.sky-wash}"
  button-pressed:
    backgroundColor: "{colors.silver}"
  button-primary:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.white}"
    rounded: "{rounded.none}"
    padding: "0 10px"
    height: "26px"
  button-primary-hover:
    backgroundColor: "{colors.ink-hover}"
  button-ghost:
    backgroundColor: "transparent"
    textColor: "{colors.ink}"
    height: "26px"
  input:
    backgroundColor: "{colors.white}"
    textColor: "{colors.ink}"
    rounded: "{rounded.none}"
    padding: "0 8px"
    height: "26px"
  segmented-toggle-selected:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.white}"
    height: "26px"
  header-band:
    backgroundColor: "{colors.sky}"
    textColor: "{colors.ink}"
    height: "48px"
  project-tab-selected:
    backgroundColor: "{colors.white}"
    textColor: "{colors.ink}"
  sub-tab-selected:
    backgroundColor: "{colors.meadow}"
    textColor: "{colors.ink}"
    height: "32px"
  sub-tab-hover:
    backgroundColor: "{colors.meadow-wash}"
  run-row:
    height: "48px"
    padding: "0 12px"
  run-row-selected:
    backgroundColor: "{colors.sky-wash}"
  run-row-failed:
    backgroundColor: "{colors.fail-wash}"
    textColor: "{colors.fail}"
  run-band:
    height: "20px"
  scrubber-flag:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.white}"
    typography: "{typography.chart-micro}"
    padding: "2px 6px"
  tooltip:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.white}"
    typography: "{typography.label}"
    padding: "6px 8px"
  footer:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.white}"
    typography: "{typography.label}"
    height: "24px"
---

# Design System: rcfd Studio

## Overview

**Creative North Star: "The Wayfinding Program"**

The Studio is drawn the way Otl Aicher drew the Munich 1972 Games: one strict program in which every run, state and event is a grid-built pictogram set beside its word, and colour is a short, flat, ordered scale where each hue has a job. Someone at a desk in daylight, reading numbers closely for hours, should be able to tell from the screen alone what ran, what is running and what failed, without hunting through it.

The world is white ground, deep ink rules and flat colour fields. Density comes from hairlines rather than card padding: panels sit edge to edge, divided by 1px ink or silver lines, and the workspace reflows by whole modules instead of floating boxes. Selection pulls the chosen run's numbers forward while everything else fades. The one place the world gives way is the field view: there the data owns the colour, through a perceptual colour map, and the chrome stays ink and white around it.

It refuses the grey docked workbench with a single blue accent: no grey chrome, no black, no grey text, no rounded panels.

**Key Characteristics:**
- Flat colour fields with jobs (sky, meadow, orange, deep red) on white, ruled in deep ink.
- State is always a pictogram plus a word; colour only reinforces it.
- Square corners everywhere; 1px rules and hairlines instead of cards and shadows.
- One neo-grotesk (Archivo) with tabular figures throughout.
- Hairline density, so many numbers fit on one screen, with selection bringing one run forward.
- Light theme, desktop only (1180px minimum width).

## Colors

A short Aicher scale of flat, mid-light colours, each with one job, read against white and set off by deep ink. The build lightened the brief's sky (#4FA3DB) and meadow (#5DB36A) so that ink text on them holds 5:1; the build values in the frontmatter are the record.

### Primary
- **Deep Ink** (ink): the only dark. All text, 1px rules, primary buttons, the selected segmented toggle, tooltips, the scrubber line and its flag, field annotations, the cylinder body and frame in the field, and the footer band. It stands in for black.
- **Deep Ink, raised** (ink-hover): hover on primary buttons only.

### Secondary
- **Sky** (sky): the header band, and the iterating phase of a run band. Never body text (2.28:1 on white).
- **Sky Wash** (sky-wash): hover on buttons and list items, the selected run row, the user's chat message, and selects that sit on the header band.
- **Meadow** (meadow): the active sub-tab, the Claude Code head, the switch track when on, and the converged phase of a run band.
- **Meadow Wash** (meadow-wash): sub-tab hover.
- **Meadow Deep** (meadow-deep): the converged pictogram (and the related residual p line) where it stands on white, so the mark holds contrast (5.1:1).

### Tertiary
- **Signal Orange** (orange): only the running run's live band segment, the focus outline and the text caret. Ink on orange reads at 4.59:1.
- **Orange Deep** (orange-deep): the running pictogram where it sits on white inside the run bands (4.8:1).
- **Failure Red** (fail): failure only, as text, the failed band segment and the failed pictogram, always with the word "Failed" or "failed". 8.12:1 on white.
- **Failure Wash** (fail-wash): the background of a failed run's row. Fail on fail-wash is 6.65:1.

### Neutral
- **White** (white): the ground everywhere, button and input backgrounds, and the chip behind band labels placed after a band's end.
- **Ink Two** (ink-2): secondary text (column captions, run ids, axis ticks, placeholders, units), 6.18:1 on white. Tinted from ink; it is the only "lighter" text colour.
- **Silver** (silver): hairlines between cells and panels, input borders, the queued and not-yet-computed hatch, the pressed button fill. Never text.
- **Silver Wash** (silver-wash): the field viewport ground, table row and column hover.

### Named Rules
**The Colour Has a Job Rule.** Each hue means one thing: sky = header band and iterating; meadow = active sub-tab, Claude Code and converged; orange = the running run's live segment, plus focus and caret; deep red = failure. A new surface may not give a hue a second meaning.

**The One Live Thing Rule.** Orange fills exactly one band segment at a time: the live segment of the running run. Other emphasis uses ink.

**The Fade by Colour Rule.** Faded or de-emphasised content changes its fill opacity or switches its text to ink-2; text opacity is never lowered. Failures never fade.

**The No Black, No Grey Text Rule.** Deep ink replaces black; ink-2 replaces grey. Silver is for lines and hatch, never for words.

**The Data Owns the Field Rule.** Inside the field view, colour comes from the data through viridis (a 256-entry table built from 9 control stops, #440154 to #fde725), and the same table draws the legend. The palette above never tints a field.

## Typography

**Display Font:** Archivo (with Helvetica Neue, Arial)
**Body Font:** Archivo (same family)

**Character:** One neo-grotesk in three weights (400, 500, 700), with tabular figures set on the body so numbers line up in columns and do not jitter while they change. The hierarchy comes from size and weight, never from a second family.

### Hierarchy
- **Display** (700, 30px, line-height 1, -0.03em): the monitor readout (for example the Cd value) at the scrubber's time.
- **Headline** (700, 22px, -0.02em): values in the selected run's number grid.
- **Title** (700, 15px, -0.01em): the brand mark in the header band.
- **Title small** (700, 13px): case names in the runs table and the view title; 500 weight for project tabs.
- **Body** (400 and 500, 12px, 1.35): the default for controls, table cells, chat and panel heads (700 for heads). Chat paragraphs are capped at 60ch; empty-state copy at 42 to 46ch.
- **Label** (400 and 500, 11px): column heads, number captions, legends, captions, tooltips, footer.
- **Chart micro** (500, 10px): text inside the time panel only (band segment labels, band end labels, axis ticks, residual decades, the scrubber flag, the not-computed note).

### Named Rules
**The Tabular Figures Rule.** Every number is set in tabular figures, and units sit in the label or caption ("Time, s"), not after each value.

**The One Family Rule.** Archivo carries everything; there is no display face and no mono.

## Layout

A fixed desktop grid with a 1180px minimum width and no mobile layout, by design. The page is four rows: a 48px header band, the workspace, a 248px time panel and a 24px footer. The workspace is three columns (runs, field, Claude Code) in about 1 : 1.4 : 0.72 proportions, with minimums of 400, 480 and 300px. The time panel is three columns: 200px row labels, the shared time axis, and a 380px monitor.

Panels sit edge to edge and are divided by 1px ink rules (structure) or silver hairlines (subdivision), with no gutters and no card padding. Spacing runs on 2, 4, 6, 8, 12, 16 and 24px; cells use 8 to 12px insets. Controls are 26px tall, tab rows 32px, run rows 48px and run bands 20px, so rows stack in strict stripes. The field frame is locked to the domain's aspect ratio and centred in its viewport, with inlet and outlet labels above it and the legend and caption below.

**The Hairline Density Rule.** Divide with lines, not with space or boxes: a new panel takes a 1px ink rule on its structural edge and silver hairlines inside.

## Elevation & Depth

The system is flat. Depth comes from rules and tone: ink rules mark structure, washes mark hover and selection, and the selected run row is boxed with inset 1px ink lines. Focus is a 2px orange outline. The one lifted surface is the select popover, which floats over the workspace and needs to separate from it.

### Shadow Vocabulary
- **Popover lift** (`box-shadow: 0 6px 18px rgba(29, 58, 95, 0.18)`): select popovers only, tinted from ink, together with a 1px ink border.

### Named Rules
**The Flat Field Rule.** Panels, rows, bands and buttons have no drop shadows. Only floating overlays get the ink-tinted popover lift.

## Shapes

Every corner is square (0px): buttons, inputs, tabs, popovers, tooltips, chips, bands and the switch knob. The form language is the Aicher pictogram grid: state marks and interface icons are drawn on a 14-unit square grid with strokes only at 0, 45 and 90 degrees, square line caps and mitred joins (2px stroke for states, 1.75px for icons). The single circle in the system is the queued pictogram, a waiting ring. Hatching at 135 degrees (silver and white, 4px stripes) means "not yet": queued runs and the part of a running run that is not computed yet.

State pictograms: queued (circle), running (an L turning into a forward arrow), converged (check), finished (end bar), failed (broken cross).

## Components

### Buttons
Plain, ruled and square: a white face inside a 1px ink line.
- **Shape:** square (0px), 26px tall, 10px side padding, 500 weight, 6px gap to a 14px icon.
- **Default:** white with an ink rule; sky-wash on hover; silver when pressed.
- **Primary:** deep ink with white text; ink-hover on hover. Used for New run, Send and Play.
- **Ghost:** no border or fill, used on the header band; a 35% white lift on hover.
- **Reference chip:** 22px tall with a silver hairline and label type, for run and log references in chat.
- **Focus:** 2px orange outline, 1px offset, on every focusable element.

### Segmented toggles
A row of joined buttons sharing their 1px ink rules (Surface, Iso-lines, Streamlines; Follow live). The selected item turns deep ink with white text. An unavailable option is shown as a dashed silver chip in ink-2 with a tooltip that says why.

### Inputs / Fields
- **Style:** white, 26px tall, a silver hairline border, square, ink-2 placeholder, orange caret.
- **Focus:** the border turns deep ink.
- **Select:** a field-styled button with a rotated square caret; on the header band it takes an ink border over sky-wash.
- **Switch:** a 28 by 14px ink-ruled track with a square knob; meadow when on.

### Navigation
- **Project tabs** live in the sky header band: 13px, 500 weight, divided by 35% ink lines; the selected project becomes a white tab at 700 weight that opens into the page.
- **Sub-tabs** sit in a 32px row ruled below in ink: the selected tab is a meadow field at 700 weight, hover a meadow wash.

### Runs table
Sortable columns (case and run, state, result), 48px rows ruled by silver hairlines. The selected row turns sky-wash, boxed with inset ink lines. Other rows fade, with their text turning ink-2 and pictograms half-filled, except a failed row, which keeps its fail-wash background and red word. The state cell is always pictogram plus word.

### Number grid
The selected run's values in a three-column hairline grid: an 11px ink-2 caption over a 22px bold value, headed by the case name with its pictogram and state word.

### Run bands (signature)
Every run is one 20px row on a single shared run-time axis, its flat colour segments split by phase (sky iterating, meadow converged, orange live, red failed; silver hatch for queued and not computed yet). Segments grow in from the left once on load (700ms, ease-out, staggered 90ms; off under reduced motion). Each band's end carries its pictogram and word ("converged · 13.2 s", "failed · it 211"): placed after the end on a white chip when there is room, otherwise inside the end with no chip, so the label never hides the band. The selected band gets an ink underline and bold label; the others drop to 40% fill.

One deep ink scrubber line, with its own 18px flag lane above the bands, crosses the bands, axis and residual plot; it turns 2px orange while dragged or focused. Dragging it moves the field, residual cursor, monitor readout and snapshot counter together, directly and with no easing. The flag reads "t = 12.60 s" in white on ink and flips to the other side near the start of the axis.

### Field view
A three.js WebGPU canvas whose colour comes from the raw scalar on the GPU through the viridis table that also draws the CSS legend. The frame keeps the domain's aspect ratio, labelled with inlet and outlet above. Annotations are small ink chips with white text; they and the end labels hide as soon as the view is moved, and a Reset view button appears in the top right corner. The overlays (iso-lines in ink, streamlines in white, mesh grid in 38% white) are first-class states. Under the frame are the legend (ink-ruled bar with ticks) and a caption naming the case, quantity, unit, time, iteration and colour map.

### Claude Code panel
A meadow head with a square ink dot. Your messages sit in sky-wash on the right; Claude's answers sit between silver hairlines with no box, bold on the numbers that matter, and reference chips beneath.

### Disclaimer footer
A 24px deep ink band with white label text that is always visible, with secondary text in a light ink tint (#c7d3e2, 7.6:1).

## Do's and Don'ts

### Do:
- **Do** pair every state colour with its pictogram and its word (queued, running, converged, finished, failed).
- **Do** draw new pictograms and icons on the 14-unit grid with 0, 45 and 90 degree strokes and square caps.
- **Do** keep orange for the running run's live segment, focus and caret only.
- **Do** fade by fill opacity or by switching text to ink-2 (6.18:1), never by text opacity, and never fade a failure.
- **Do** use the deep variants (meadow-deep, orange-deep) when a state mark sits on white.
- **Do** divide panels with 1px ink rules and silver hairlines, square-cornered, with no gutters.
- **Do** set every number in tabular figures and put its unit in the caption.
- **Do** keep the disclaimer footer visible on every surface.
- **Do** colour fields from the raw value through one table that also draws the legend.

### Don't:
- **Don't** use black or grey text; deep ink and ink-2 cover every text job.
- **Don't** round corners on any container, control or band.
- **Don't** add drop shadows to panels, rows or buttons; only floating popovers lift.
- **Don't** use sky, meadow or orange as text on white; they are fields and fills that carry ink text.
- **Don't** show deep red for anything but a failure, and never without its pictogram and word.
- **Don't** tint the field with interface colours, or show a field without its legend and caption.
- **Don't** add a second typeface or a monospace for numbers.

## Open

Not yet decided, and not covered by this record: a dark theme; the case editor and project home surfaces; density and layout beyond the run workspace (other panels, compare views); any layout below 1180px wide.
