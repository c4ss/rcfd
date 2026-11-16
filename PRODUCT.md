# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

<!-- The Electron renderer is Chromium, so web references apply directly. It is a desktop app: one window, no mobile layout, no browser chrome. -->

## Stack

Electron Studio in `apps/studio-web/` (ADR 0350 Q2), TypeScript, outside the Cargo workspace. Electron 45.1.0 was
measured in the spike (`bench/electron/`). Bulk field data: the Node reader of `store.rts` (R2) for local projects, the
opt-in `data.*` methods (R1) for a remote service; control and events go through the JSON-RPC service (`rcfd-service/1`).
The engine is Rust and stays Rust (`docs/studio-guardrails.md` G2).

Decided by the maintainer 2026-11-16:

- three.js for every 2D and 3D visualisation (fields, meshes, plots in the viewport), on `WebGPURenderer` with TSL node
  materials. The built-in WebGL2 fallback is the fallback.
- React 19 + Vite + react-three-fiber (with drei where it earns its place) for the UI. Per-frame updates (scrubbing,
  live fields) mutate three.js objects outside React's render loop.
- React Aria Components as the accessible base for widgets (keyboard, focus, names and roles; guardrail G8). It gets styled by DESIGN.md
  and is not used as a look.
- Each dependency is pinned and justified in its ADR (guardrail G5).

The egui apps `rcfd-studio` (egui/wgpu) and `rcfd-gui` were removed on 2026-11-16 (F-449), before parity, by the maintainer's
decision; the git history keeps them.

## Users

The first users are non-CFD learners and undergraduates learning CFD. The Studio is easy to learn and hard to master:
detail appears when it is needed, not before. Simulation engineers come next. They open a project folder, set up cases,
start several runs in the background, scrub finished and running runs through time, compare runs, and iterate on designs.
The maintainer's 120 mm axial computer-fan work is the first real engineering use.

## Product Purpose

rcfd is a clean-room, open-source (AGPL-3.0, with a commercial licence) finite-volume CFD suite. The Studio is its desktop
front end. A project is a folder that holds cases, meshes and many runs. Each run has a state (queued, running, paused,
finished, failed) and key numbers. Runs continue when the app is closed. Success means an engineer can see what ran, what
it produced and whether to trust it, without leaving the app, and a learner can get from an empty folder to a first
honest result without knowing CFD jargon.

## Positioning

The Studio shows the real solver's results, straight from project folders and runs, and has a Claude Code chat for the
project inside the app. Claude Code outside the app has the same project tools, and the edits it makes appear live in the
Studio. Every number on screen comes from the solver and can be traced back to it.

## Operating Context

- Project folders (`example-projects/wind-tunnel`, `example-projects/quick-start`) hold case files, meshes, runs with
  stored fields, a design ledger and notes. The folder contract is `docs/ui-restart-plan.md` §8 and ADR 0173.
- A detached runner service owns runs, not the window.
- Time scrubbing moves fields, residuals and monitors together. Two runs can be compared at the same time.
- 3D views: mesh and fields on surfaces, cut planes, iso-surfaces, vectors, streamlines, particles.
- Agents cannot see the window. At the end of each wave the maintainer looks at the app and reports what they saw.
- Measured on the maintainer's machine (Windows 11, RTX 5080, 60 Hz display): see ADR 0350 addendum 1.

## Capabilities and Constraints

- Parity target (ADR 0350 §5): project browser, run list and state, background runs that survive the app, time scrubbing
  with linked plots, case editor with schema validation, quick run, stop, live preview, Claude Code chat, field view with
  colour map, legend and hover, plus `docs/ui-restart-plan.md` §6.
- The Studio does not have every feature yet. It must never suggest a capability that does not exist. For example,
  stress and strain colouring needs a structural solver, which does not exist.
- Windows 11 is the only measured platform. Linux and macOS are unmeasured.
- Every capability of the simulation libraries is reachable from the Studio (guardrail G15, 2026-11-16): the UX is
  built from the engine's own inventory (case schema, service methods, MCP tools, CLI), and disclosure decides where a
  capability sits, never whether it exists.
- No UI-only capability: every Studio action is also reachable from Claude Code, the CLI and Python (G16). Results
  are reproducible from the CLI and every number shows its provenance and unit (G17).
- The hard limits are in `docs/studio-guardrails.md`.

## Brand Commitments

- Name: rcfd. The app is Studio. rcfd is an independent project: no vendor branding, wording, screenshots or assets may
  be copied (clean-room policy, `CONTRIBUTING.md`, `docs/provenance.md`). The trademark and independence disclaimers stay.
- The software is early-stage. It is not validated for engineering decisions or for safety-critical use, and it comes
  with no warranty. The UI must not suggest otherwise.

## Evidence on Hand

- Real solver runs with stored fields in `example-projects/`. The meshes are coarse on purpose, and none of the results
  is a validation.
- The ADR 0350 spike measurements (performance only, not usability).
- There are no testimonials, customers or usability studies. Do not invent any.

## Product Principles

1. Every colour, value and plot traces to a real solver number. Nothing is decorative or made up.
2. Be honest about limits: status, unverified values and known gaps stay visible.
3. Run state is always clear, and runs outlive the window.
4. Time, fields, residuals and monitors move together.
5. Show less at first and reveal more on demand: a learner's first path is short, and an engineer's depth is one step
   away, never hidden for good.
6. Everything the engine can do, the Studio can do. New engine features arrive additively, and the Studio is
   refactored when that is what it takes to make them fit without seams.

## Accessibility & Inclusion

No WCAG contrast requirement (removed by the maintainer 2026-11-16; colours follow DESIGN.md). State is shown by shape and by words as well as by colour. Field display
requires colour maps that are safe for colour-blind users. Keyboard access and visible focus are required throughout.
