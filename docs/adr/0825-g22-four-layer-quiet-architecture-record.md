# ADR 0825: G22 four-layer quiet architecture replacing the per-surface word budget (record)

Status: RECORDED 2026-11-27, from the maintainer's direction of that day. A record of a guardrail change and the
resulting roadmap item; the follow-up item registers its own checks.

## Context and diagnosis (2026-11-27)

Reviewing the first real `export_view` screenshots ([ADR 0823](0823-f980-registration-a-view-exported-as-one-self-contained-html-page.md),
F-980) showed standing prose, raw JSON actor/PID strings, `fnv64:` hashes, file paths, sample-count debug lines and
17-digit floats printed directly on the graphics, despite [G22](../studio-guardrails.md#g22-implicit-by-default-explicit-on-exception-four-layer-quiet-architecture)
and The Quiet Rule in `DESIGN.md`.

Four structural gaps caused the per-surface word budget (`F-833`, `apps/studio-web/src/shared/word-budget.ts`) to fail:

1. **Opt-in failure mode:** new surfaces (the Canvas of [ADR 0803](0803-the-studio-as-claudes-canvas-record.md) and
   `export_view` of [ADR 0823](0823-f980-registration-a-view-exported-as-one-self-contained-html-page.md)) started
   outside the surface registry with no budget, and G22 was omitted from the ADR 0803 carryover list and the F-980 brief.
2. **The `data-words="data"` loophole:** raw machine metadata (hashes, run paths, PIDs, 17-digit floats, counts) counted
   as zero chrome words even under a budget.
3. **Conflated glance and audit layers:** G13 (automated verification) and G17 (provenance) pushed agents to print
   machine strings and full round-trip floats into visible `<p>` paragraphs.
4. **Blind authoring:** G13 treated agents as unable to inspect rendered PNGs, leaving obvious visual clutter uncaught.

## The maintainer's direction (2026-11-27)

Replace the per-surface word budget as the primary quietness control with a four-layer architecture encoded in
[docs/studio-guardrails.md](../studio-guardrails.md) (`G13`, `G17`, `G22`, `G23`) and `DESIGN.md`:

1. **Layer 1 — Glance vs Audit/Test separation:** visible pixels carry only the glance layer (graphic, short noun label,
   3–4 significant-digit formatted ticks/readouts with unit, and exceptions). Exact `f64`/`f32` values, file paths,
   hashes, actor/PID records, timestamps, sample counts and view specs live in `data-*` attributes and collapsed
   `<details>` blocks or click/keyboard popovers.
2. **Layer 2 — Universal no-prose, no-machine-strings invariants:** one automatic check across every Studio surface,
   Canvas view and exported HTML/SVG without per-surface registration or a `data-words="data"` escape hatch — banning
   visible `<p>` paragraphs at rest (outside `<details>`, exceptions, disclaimers and user-authored text), multi-clause
   sentences in visible labels, raw machine strings (`fnv64:`, `runs/r-`, JSON `{...}`, `pid`, ISO timestamps, floats
   with six or more decimal digits), and bounding visible text area on data panels.
3. **Layer 3 — Typed visual slots and one shared formatter:** panel/figure renderers take typed slots (`title`, `unit`,
   ticks, KPI, `exception`, `provenance`) and format visible numbers through one shared compact formatter, with no
   free-form prose parameter.
4. **Layer 4 — Image-only visual critic:** visual changes render PNGs and pass a separate image-only review checking for
   visible text that is inferable from the graphic, redundant, machine-formatted or written as a sentence.

## Applied immediately (2026-11-27)

- Updated [docs/studio-guardrails.md](../studio-guardrails.md) (`G13`, `G17`, `G22`, `G23`) and `DESIGN.md`.
- Applied Layers 1–3 to `export_view` in `crates/rcfd-workspace/src/view.rs` (`PanelOutput`, `fmt_glance`, collapsed
  `<details class="xv-audit">` provenance, universal `page_quiet_violations` checker and unit test) and
  `apps/studio-web/src/renderer/canvas/export.css` (tokens only), preserving all existing `data-*` and test hooks of
  [ADR 0823](0823-f980-registration-a-view-exported-as-one-self-contained-html-page.md).

## For the lead: roadmap entries

Proposal file: `docs/roadmap/proposals/0825-F-833.json` (form 1 of ADR 0755).

> Status: done (F-833 stays done; its follow-up replaces the per-surface word budget across all GUI surfaces with G22 Layers 1–4).

> Note: ADR 0825, 2026-11-27, done: G22 evolved in studio-guardrails.md and DESIGN.md to the four-layer quiet architecture (glance/audit separation, universal no-prose/no-machine-string invariants, typed visual slots with one compact formatter, image-only visual critic) and applied immediately to export_view; follow-up entered to bring all remaining Studio and Canvas GUI features into compliance.

> Follow-up (`gui-quiet-four-layers`, phase 8, deps F-833, F-976, F-980): **Studio and Canvas GUI compliance with G22 four-layer quiet architecture (Layers 1–4)**. Acceptance: Every limit not stated here is registered in the ADR before the run. Every live Studio surface, Canvas panel kind and exported HTML view separates the visible glance layer from the audit/test layer (Layer 1), routes figure/panel chrome through typed visual-slot components and one shared 3–4 significant-digit number formatter with no free-form prose caption parameter (Layer 3), passes one universal DOM/HTML quiet-invariant test covering every rendered view without per-surface registration or a `data-words="data"` escape hatch for machine strings (Layer 2: zero visible `<p>` paragraphs outside `<details>`/exceptions/disclaimers, zero raw hashes/run paths/JSON/PIDs/ISO timestamps/6+-decimal floats, zero sentence punctuation in visible labels, and a registered data-ink text-area ceiling on data panels, plus negative controls), and passes an image-only visual-critic pass on rendered 1440x900 PNGs of every Canvas layout and Studio view (Layer 4); once green across all surfaces, retire the superseded per-surface word-budget allow-list in `word-budget.spec.ts`.
