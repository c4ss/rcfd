# ADR 0800: The Electron Studio roadmap, generated from guardrails, PRODUCT.md and DESIGN.md

Status: ACCEPTED 2026-11-16 (questions answered by the maintainer, section 6). Asked for by the maintainer on 2026-11-16 ("create a new roadmap for the electron studio
that is going to be generated from a set of guardrails and from a PRODUCT.md and DESIGN.md"). The number 0800 and the
items F-800 to F-849 are a block reserved by the maintainer on 2026-11-16, so that this roadmap cannot collide with the
numbers the roadmap loop takes in order (F-704 and up, ADR 0633 and up).

## 1. Context

- ADR 0350 measured an Electron shell over the rcfd APIs (F-447) as fast enough and exact by both bulk routes. F-448
  (parity) waits for the maintainer's go-ahead; F-449 (retiring `rcfd-studio` and `rcfd-gui`) follows it.
- F-448 was one item. It is too large to run, review or measure as one piece, and it had no product or design authority
  beyond the parity list of ADR 0350 §5.

## 2. Decision: three authorities, one roadmap

The Electron Studio is planned from three documents, and every item below names the parts of them it serves:

| Authority | File | Holds | Decided |
|---|---|---|---|
| Guardrails | `docs/studio-guardrails.md` | G1 to G14: what must not happen (clean room, Rust is the engine, bit-exact pictures, additive interfaces, dependencies, honesty, runs outlive the window, accessibility (keyboard, focus, state in words; no contrast requirement since 2026-11-16), performance floor, Electron security, Claude Code, project folder contract, testable without eyes, rcfd-studio frozen); G15 to G21: what the Studio must do (everything the engine can do; no UI-only capability; reproducible with provenance; non-destructive; explain in place; every action a command; never block) | G1 to G14 approved as drafted, 2026-11-16; G15 to G21 asked for by the maintainer the same day |
| Product | `PRODUCT.md` | Users (learners first, engineers next), purpose, positioning, operating context, stack, principles | impeccable `init`, 2026-11-16; stack chosen by the maintainer |
| Design | `DESIGN.md` (+ `.impeccable/design.json`) | The visual system: palette with one job per hue, Archivo with tabular figures, square hairline forms, state pictograms, run bands, field view | Recorded by impeccable's documenter from the shipped prototype `apps/studio-prototype/` |

Decisions of the maintainer on 2026-11-16 that the roadmap rests on:

- Stack: three.js for every 2D and 3D picture on `WebGPURenderer` with TSL (WebGL2 fallback); React 19 + Vite +
  react-three-fiber; React Aria Components as the accessible widget base; the app in `apps/studio-web/`.
- Visual world: a new one (not a port of the egui "technical-report plate"). The direction round (impeccable seed
  `3c0b0ece`) chose the Aicher system: Otl Aicher's Munich 1972 identity program, used only for type, palette, density
  and one signature move (run bands with a linked scrubber).
- The prototype came first, and DESIGN.md was recorded from it. The prototype is a reference, not the product. Its data
  is synthetic.

## 3. How the roadmap is generated

1. Every parity check of ADR 0350 §5 and `docs/ui-restart-plan.md` §6 becomes at least one item.
2. Every guardrail that needs work, not only restraint, becomes an item or an acceptance clause (G3 picture checks, G7
   kill-the-app test, G9 benchmark, G10 hardening, G13 test harness).
3. Every capability in PRODUCT.md (Operating Context, Capabilities) and every component in DESIGN.md that the parity set
   does not already cover becomes an item.
4. Every "Open" line in the three documents becomes an item marked NEEDS THE USER.
5. G15 makes coverage the organising rule of the UX: the engine's own inventory (`settings.schema`, the method list of
   `service.info`, the MCP tool list, the CLI help) is the list of what the Studio must reach, a coverage test keeps it
   honest, and a schema-driven surface lets new engine features appear with no Studio code. The Studio may be refactored
   freely to make a feature fit; engine-side changes stay additive under G4, through an ADR (maintainer, 2026-11-16).
6. Acceptance is written so an agent can check it headlessly (Playwright driving Electron, unit tests, value comparisons
   with the Rust reader or `rcfd-render`), plus a named view-and-report step where only the maintainer can judge (G13).

## 4. The items

F-800 to F-832, category "Studio desktop app". F-448 now depends on the parity subset (F-801 to F-814, F-816, F-817,
F-822) and on the items that make G15 to G21 real (F-824, F-825, F-827 to F-832); its acceptance is unchanged. F-449 is unchanged. The items and their traces are in `docs/ROADMAP.md` and
`docs/roadmap.json`.

| Id | Item | Guardrails | PRODUCT / DESIGN |
|---|---|---|---|
| F-800 | Go-ahead and repository layout for `apps/studio-web/` | G5, G14 | Stack |
| F-801 | Hardened Electron shell and test harness | G10, G13, G9 (P10) | Stack |
| F-802 | Design tokens and accessible primitives from DESIGN.md | G8, G1 | DESIGN Colors, Typography, Components |
| F-803 | Service client: control and events | G2, G4, G7, G9 (P2) | Operating Context |
| F-804 | Bulk field data: R2 reader and R1 route, bit for bit | G3, G9 (P4) | Stack |
| F-805 | Project home and the live project folder | G12 | Operating Context |
| F-806 | Run list and run state | G6, G7, G8 | DESIGN Runs table |
| F-807 | Run control: quick run, stop, queue | G2, G7 | Capabilities |
| F-808 | Field viewport on three.js WebGPU | G3, G9 (P7, P8) | DESIGN Field view |
| F-809 | Derived pictures: iso, streamlines, cut planes, vectors | G3, G6 | Operating Context |
| F-810 | Run bands and linked time scrubbing | G3, G9 (P5, P6) | DESIGN Run bands |
| F-811 | Residual and monitor plots | G3, G9 (P3) | Principles 4 |
| F-812 | Case editor with the engine's schema | G2, G12, G15 | Principle 5 |
| F-813 | A learner's first path | G6 | Users, Principle 5 |
| F-814 | Claude Code panel | G11 | Positioning |
| F-815 | Compare two runs | G3 | Operating Context |
| F-816 | Keyboard and assistive-technology pass | G8 | DESIGN Do's and Don'ts |
| F-817 | Performance regression bench for the product shell | G9 | — |
| F-818 | Dark theme | G8 | DESIGN Open |
| F-819 | Large meshes and polyhedral fan meshes | G9 | Capabilities (unmeasured) |
| F-820 | Installers, signing and updates | G10 | Guardrails Open |
| F-821 | Linux and macOS | G9 | Guardrails Open |
| F-822 | Tutorials that name the GUI | G6 | ui-restart-plan §6 |
| F-823 | Studio npm tests in the gate | G13 | Guardrails G13 |
| F-824 | Capability inventory from the engine and a UI coverage test | G15, G2 | Principle 6 |
| F-825 | Schema-driven case and settings surface | G15, G2, G8 | Principles 5, 6; DESIGN Inputs |
| F-826 | Engine: UI metadata in the case schema (needs its own ADR) | G15, G4 | Principle 6 |
| F-827 | One action layer for every UI action | G16, G20, G18 | Positioning |
| F-828 | Provenance inspector and reproduce-from-CLI | G17, G3 | Principle 1 |
| F-829 | Undo, safe deletes and edit conflicts | G18, G11, G12 | Operating Context |
| F-830 | Explain in place from the engine's docs | G19 | Users, Principle 5 |
| F-831 | Command palette and keyboard shortcuts | G20, G8 | Users (hard to master) |
| F-832 | Responsiveness budget and cancellable work | G21, G9 | Principle 3 |

## 5. Limits

- Agents cannot judge look and feel; every surface item ends with a view-and-report step for the maintainer.
- The prototype's numbers are synthetic; none of its pictures is evidence of anything.
- The design system covers one surface (the run workspace). Project home, the case editor and the learner's path
  extend it through impeccable (`shape`, then build), and DESIGN.md is updated only when the maintainer approves a
  system change.
- The dependency list of the prototype (react, react-dom, three, @react-three/fiber, react-aria-components,
  @fontsource/archivo; dev: vite, @vitejs/plugin-react, typescript, @types/*) is the starting point for F-800's
  justification, not an approval of it.

## 6. Answers of the maintainer (2026-11-16)

- **Q1, go-ahead for F-448:** yes. F-800 to F-802 may start now.
- **Q2, the prototype:** keep `apps/studio-prototype/` as the design reference until F-448 passes, then remove it.
- **Q3, where Studio npm tests run:** in CI, as a job in `.github/workflows/ci.yml` (not a local-only step, not inside
  `scripts/gate_test.py`).

## 7. Change of 2026-11-16: no contrast requirement

The maintainer removed every WCAG AA contrast requirement from the Studio (G8, PRODUCT.md, F-802, F-816, F-818) and chose
to revert the eight contrast-driven token changes of F-802, so the product tokens equal DESIGN.md again. Keyboard
access, visible focus, names and roles, state carried by shape and words as well as colour, colour-blind-safe field
maps and reduced motion stay.
