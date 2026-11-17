---
version: 1
slug: "apps-studio-web-src-renderer-settings"
primary_target: "apps/studio-web/src/renderer/settings/SettingsSurface.tsx"
related_targets: ["apps/studio-web/src/renderer/settings", "apps/studio-web/src/renderer/ui"]
---

# Case settings (Electron Studio, F-825)

Scope: the schema-driven case and settings surface of the Electron Studio for one service session, at `#/settings`
(`#/settings/<session>`). Every key of the engine's `settings.schema` gets a control made from the schema at run time;
there is no Studio-side list of keys. F-812 (case editor) wraps this surface with case-level actions (open, save,
round trip with the CLI); F-805 gives it its place in the project shell; F-826 adds labels, an advanced flag and help
text to the schema, which this surface then uses instead of the structural rules below.

Mode: Operate.

Assumptions (no interview was possible: the surface was built by an agent from the roadmap item; the maintainer
confirms or corrects at the view-and-report):

- A1. The visitor is a learner with a session made from the engine's defaults, or an engineer with a session made from
  a case file; both edit one key at a time and want the engine's verdict at once.
- A2. The surface lives inside the existing DESIGN.md world (Wayfinding Program); it extends the system with a settings
  layout, it does not add a hue, a face or a shape.

## Job and audience

A learner sees the few keys that make a first case (the head of each section) with their defaults, units and allowed
ranges, changes one, and is told by the engine, in the engine's words, whether the value was taken. An engineer finds
any of the 948 keys of today's schema in two steps (search, or section then "more"), sees what the case changed from
the defaults, and checks the whole case with the engine before running.

## Outcome and proof

- Primary action: change a setting (`settings.set` through the action layer), read the engine's reply in place.
- Secondary: check the case (`settings.validate`), add and remove named entries (`settings.set` on the entry's first
  key, `settings.remove`), reset a key to its schema default, find a key by name or description.
- Proof that it is real: the value shown after a change is the value the engine stored (its reply), an error is the
  engine's message verbatim, and the CLI command that makes the same change is shown under the last change (G16, G17).

## Selected direction

Visual authority: DESIGN.md (Inputs, Navigation, Buttons, Select, Switch, Tooltip, the Hairline Density Rule). The
surface is a three-column ruled page under the sky header band: the section index on the left, the selected section's
keys in the middle as hairline rows, and the engine's check of the case on the right. Each key row is a strict stripe:
the key's name (with its unit in the label, the Tabular Figures Rule) and the engine's description; the 26px control;
the default and the range as captions. Nothing is a card; the rows are divided by silver hairlines and the columns by
ink rules.

Disclosure (PRODUCT Principle 5, G15), derived from the structure of the schema because it carries no importance
metadata yet (F-826 will add it):

1. The section index lists the schema's sections in the engine's declaration order (the order of `required`), then
   its optional sections. A first entry, "Set in this case", lists every key whose value differs from its schema
   default (and every list with items, every named entry): what this case changed is never more than one click away.
2. In a section, its own keys come first, in declaration order; the first eight are shown, the rest are one step away
   behind "Show N more keys". A key changed from its default is always shown, wherever it sits.
3. Nested objects, optional objects and named collections are one step away as ruled disclosure rows that say how
   many keys they hold and how many are set.
4. Search (name or description) reaches every key in one step, whatever its tier. "Show every key" opens every step at
   once. Nothing is hidden for good.

## Scope and boundaries

- In: every key type the schema has today (boolean, integer, number with or without SI dimension or display unit,
  string, enumeration, lists of those, named collections, nested and optional objects) and a JSON control for any
  shape a later schema adds.
- Out (other items): opening and saving case files (F-812), undo (F-829, the action layer already records inverses),
  the command palette (F-831), the project shell (F-805), labels and help text beyond the schema's own (F-826).
- Anti-goals: no copy of the engine's checks. The Studio does not clamp, round, convert units or refuse a value; it
  encodes the text the visitor typed (a number is sent as a number, anything else as a string the engine may parse as a
  value with a unit) and shows the engine's answer.

## States and ranges

- Today's schema: 44 sections, 948 leaf keys, 24 named collections, 17 optional objects, sections of 1 to 70 own keys.
- No service configured, service connecting or lost: the page says so with the connection's reason; no controls.
- No session: the page offers the sessions the service holds and "New session from defaults".
- Run going: every control is read-only and the action layer's reason is shown once at the top.
- A change in flight, taken (value from the reply), refused (the engine's message, the failed pictogram and the word
  "failed", the typed text kept until Escape), and a change made elsewhere (another client, Claude Code): the
  `setting_changed` event reloads the tree.
- Check: not run yet, running, no problems, a list of problems each with its path as a link to the key.

## Interaction and layout

- 1440 by 900: index 264px, keys flexible, check panel 360px; each column scrolls on its own; the header band and the
  footer disclaimer stay.
- Keyboard: Tab order header, search, show every key, index, keys, check panel. The index is a listbox (arrows, Home,
  End, type-ahead). Text fields commit on Enter or when focus leaves, Escape returns to the engine's value. Switches on
  Space, selects on Enter and arrows. Every disclosure is a button with `aria-expanded`. Focus is DESIGN.md's orange
  outline everywhere.
- Help on the spot (G19): the engine's description under each key's name, in full when the row has focus or is
  hovered, and as the control's accessible description.

## Constraints and open decisions

- G2: schema, defaults, ranges and messages are the engine's; the surface reads `settings.schema` at run time.
- G8: keyboard and visible focus everywhere; state by word and shape as well as colour (no contrast requirement).
- G16: every change goes through `dispatch` of the F-827 action layer; the surface never touches the bridge.
- Open for the maintainer: the eight-key cut of rule 2 (until F-826's advanced flag), and whether the surface should
  show the engine's SI dimension strings (`m^-1 kg s^-2`) as they are or wait for F-826's units.
