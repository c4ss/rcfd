# Electron Studio guardrails

Status: APPROVED by the maintainer 2026-11-16 (G1 to G14 as drafted; G15 to G21 added the same day at the maintainer's request; G22 added the same day). Written by the lead from the house rules, ADR 0350 and
`docs/ui-restart-plan.md`.

These are the fixed limits of the Electron Studio (F-448 and its successors). Every item of the Studio roadmap names the
guardrails it touches, and its acceptance shows that it keeps them. A change that would break a guardrail needs the
maintainer's word and an ADR, like a change to a house rule.

Product truth lives in `PRODUCT.md`, the visual system in `DESIGN.md`. This file only
states what must not happen.

## G1. Clean room

No layout, wording, icon, screenshot, workflow diagram or asset taken from commercial CFD software, its manuals or its
help files; nothing decompiled or recovered from a binary. Knowing that a feature exists is fine; copying how it looks or
what it is called is not. Fonts, icons and images are original, or under OFL, MIT, Apache-2.0 or a licence that works
with AGPL-3.0, and each is listed in `THIRD_PARTY_LICENSES.md`.

## G2. Rust is the engine

The Studio has no solver, mesher, physics, unit conversion or case validation logic of its own. All of those numbers
come from the engine, through the service (`rcfd-service/1`), the opt-in `data.*` route (R1), or the TypeScript reader
of `store.rts` (R2, ADR 0350 addendum 1). Case validation uses the engine's schema, not a copy of it.

## G3. A picture can be wrong and look right

- Every field value the renderer gets is equal to the Rust reader's value, bit for bit (ADR 0350 P12), and a test keeps
  it so.
- Colour maps, iso-surfaces, streamlines and other derived pictures match the engine's reference value for value
  (`rcfd_post::render`; it was `rcfd-render`'s until F-838 removed that crate), or
  within a tolerance stated before the code is written.
- Every number shown traces to a solver value; nothing is decorative, smoothed out of sight or made up.

## G4. The engine's interfaces change only by addition

New service methods are opt-in, behind a minor protocol version. No existing method, reply shape, golden, CLI help
text, MCP tool list or default changes without the maintainer (house rule 3). Existing tests are not edited (house
rule 2). `crates/rcfd-api/tests/baseline_hashes.rs` is never re-recorded. A new field in a record the engine writes is
an addition, and older readers ignore it: for example `started_by` and `studio_pid` in `.rcfd/runner.json` (the
maintainer, 2026-11-17, ADR 0801 Q-RL-2), which no golden or baseline hash covers.

## G5. Dependencies

- Each npm dependency is pinned to an exact version, with the lock file committed. Its licence is compatible with
  AGPL-3.0 and is listed. Each one is justified in the ADR that adds it; the dependency count is kept small.
- A new Rust crate or dependency, a native Node addon, or `wasm-bindgen` needs the maintainer (house rule 5).
- The JavaScript lives in `apps/studio-web/`, outside the Cargo workspace (ADR 0350 Q2).

## G6. Honest about limits

- The Studio never suggests a capability that does not exist (for example, stress and strain colouring needs a
  structural solver).
- The disclaimers stay where users see them: early-stage, not validated for engineering decisions, not for
  safety-critical use, no warranty.
- Unverified values, known misses and failed runs stay visible; they are never hidden to make a view look clean.
- No invented customers, testimonials, benchmarks or validation claims anywhere in the app or its copy.

## G7. Runs outlive the window

The detached runner (`rcfd-runner`) owns runs. Closing, crashing or reloading the Studio never stops, pauses or corrupts
a run without the user's choice. When the Studio starts again it reconnects and shows the true state of every run.

Amended 2026-11-17. The maintainer, after closing the Studio and finding `rcfd-runner serve --project …` and a paused
`rcfd run` still holding `target\release`: "the runner should close with studio by default if no external (existing)
runner env var is specified". For active runs, their answer was: ask. So:

- The Studio owns only the runner and the service it started itself in this session. The runner records who started
  it, as `started_by` ("studio", "cli" or "mcp") in `.rcfd/runner.json`, with `studio_pid` for the Studio (the
  maintainer, 2026-11-17, Q-RL-2). The Studio owns a runner whose record says "studio" with its own pid, and no other:
  not the CLI's, not Claude Code's (`mcp`), not another Studio's. Only a record without `started_by` (an older runner)
  falls back to the looks: a runner already alive when the Studio opened the project is adopted, and one that comes up
  while it has the project open, with none alive before, is the Studio's. A service attached to through
  `RCFD_STUDIO_SERVICE` or `--service` is never stopped by it.
- On a close (the window, Alt+F4, quit, Close project), an owned runner with no run queued, running or paused is shut
  down quietly (`rcfd-runner shutdown`). With active runs the close asks, naming each run: Stop runs and close (each run
  is cancelled through the engine and keeps its records; then the runner is shut down), Keep them running and close (the
  runner stays and the next start adopts it), or Cancel (the default). One prompt asks this and save-or-discard together.
- The Studio's own `rcfd serve` closes with it: the project's runs are the runner's, not the service's. It is kept
  only while one of its sessions has a solve going in the service itself (running or paused).
- The setting "Keep the runner going after the Studio closes" (off by default) and the `--keep-runner` switch keep
  the earlier behaviour: the runner goes on, and nothing is asked about it.
- A crash, a kill or a reload still stops nothing. A close that cannot ask (the test mode's quit, a page that does not
  answer) keeps a runner with active runs.

ADR 0801, addendum "Runner closes with the Studio" (2026-11-17), has the details and decisions.

## G8. Accessibility

No WCAG contrast requirement (the maintainer removed it 2026-11-16; colour values follow DESIGN.md). Colour is never
the only carrier of state: shape and words carry it as well. Field
colour maps are safe for colour-blind users. Everything can be reached from the keyboard, focus is always visible, and
reduced-motion settings are respected.

## G9. Performance does not regress below the spike

The limits measured in ADR 0350 become regression limits for the product shell, on the maintainer's machine:

| Id | What | Limit |
|---|---|---|
| P2 | Solver event to renderer | p99 <= 20 ms |
| P4 | 1e6-cell scalar to GPU buffer | p50 <= 30 ms, p99 <= 60 ms |
| P5 | Scrubbing at 1e6 cells | >= 10 snapshots per s, no frame > 150 ms |
| P7 | First picture of the 1e6-cell surface | <= 1.5 s |
| P8 | 1e6 triangles, orbit | 60 frames per s, p99 frame <= 25 ms |
| P10 | Cold start to the project view | <= 2.5 s |

A benchmark script that anyone can run repeats them. Runs longer than about two minutes are heavy records, run once by
hand (house rule 7).

## G10. Electron security

`contextIsolation` on, `nodeIntegration` off, the renderer sandboxed, a strict Content Security Policy, no remote
content loaded, and an allow-list of IPC calls through the preload script. The service listens on loopback only unless
the user says otherwise. The Studio reaches another machine only through a run site the user named in their sites file
and confirmed once (host key fingerprint and engine version shown); project data goes there only for a run the user
started there. Electron stays on a release its maintainers still support.

(The sentence on run sites was added on 2026-11-17 by the maintainer's answer to ADR 0739 Q3, option (a).)

## G11. Claude Code

One writer per Claude Code session. Open sessions are mirrored read-only; the Studio may resume and type into a closed
one. Tool approvals use a conservative allow-list; there is no "approve everything" (decisions of 2026-10-26).

## G12. The project folder is the contract

The Studio reads and writes the project folder only as `docs/ui-restart-plan.md` §8 and ADR 0173 define it. Edits made
elsewhere (Claude Code, the CLI, a text editor) show up live. The Studio never keeps project state that exists only
inside the app.

## G13. Testable with eyes and without

Every roadmap item has automated tests (unit tests, Playwright driving Electron, or HTML structural checks) **and**, for any change that touches a rendered view or exported graphic, a visual review step on rendered PNGs (G22 Layer 4). Each wave ends with a short view-and-report checklist for the maintainer. Whether `scripts/gate_test.py` also runs the Studio's npm tests is the maintainer's decision.

## G14. rcfd-studio is retired

The maintainer decided on 2026-11-16 to delete `rcfd-studio` and `rcfd-gui` before parity, to free test and build
time (F-449, ahead of F-448). Git history keeps them. Capabilities only they had are roadmap items for the Electron
Studio; no doc may tell a user to run them. (Before 2026-11-16 this rule froze them until parity.)

## G15. Everything the engine can do, the Studio can do

Retired 2026-11-26 by the maintainer's direction of that day ([ADR 0803](adr/0803-the-studio-as-claudes-canvas-record.md)): the Studio is Claude's canvas (G23), not a screen navigated by hand. The text below is kept as it was.

Every capability of the simulation libraries is reachable from the UI: each case-schema key, service method, MCP
tool, CLI command, post-processing output and solver option. This is how the Studio's UX is built, not a later pass.

- The inventory comes from the engine itself (the case schema, the method list of `service.info`, the MCP tool list,
  the CLI help), not from a list kept by hand. A coverage test fails when a capability has no UI path and no recorded,
  dated exemption.
- Learner-first disclosure (PRODUCT.md, Principle 5) decides where a capability sits, never whether it exists.
- New engine features arrive additively and should need as little Studio code as possible: a schema-driven surface is
  the default way in.
- The Studio's own code and structure may be refactored freely so that a feature fits in without seams. On the engine
  side, refactoring stays additive under G4: new methods and new schema metadata (labels, units, ranges, an "advanced"
  flag) are fine, and changing an existing method, golden or default needs an ADR and the maintainer (decided
  2026-11-16).

## G16. No UI-only capability

Everything the Studio can do, Claude Code, the CLI and Python can do too. Every UI action is a named action that calls
the engine (a service method, an MCP tool or a CLI command) and can be scripted or recorded; the Studio never reaches a
result by a path only it has. This is G15 read the other way, and it keeps the Claude Code positioning true.

## G17. Reproducible, with provenance one click away

A run started from the Studio can be run again from the CLI with the same result, and the Studio shows the command that
does it. Any number on screen can show where it came from: run id, snapshot, file, hash, engine version and unit. Every
value carries its unit (from the engine, G2). Provenance stays **one click away** (a popover or, in static HTML exports
without JavaScript, a collapsed `<details>` block plus `data-*` attributes) — never printed as standing paragraphs,
hashes, PIDs or raw JSON under panels (G22). Builds on `docs/provenance.md`.

## G18. Non-destructive by default

Edits can be undone. Deleting a run, result or case is recoverable or confirmed in words that name what is lost. The
Studio never silently overwrites a file another writer (Claude Code, the CLI, an editor) has changed since it was read;
it shows the conflict and lets the user choose.

## G19. Explain in place

Retired 2026-11-26 by the maintainer's direction of that day ([ADR 0803](adr/0803-the-studio-as-claudes-canvas-record.md)): the Studio is Claude's canvas (G23), not a screen navigated by hand. The text below is kept as it was.

Every control, term and value has help on the spot, taken from the engine's own documentation, case schema and
glossary, not written separately for the Studio. Every error names the problem and the way to recover. A learner never
has to leave the app to understand a word on the screen.

## G20. Every action has a command

Every action G15 makes reachable is also a command: findable by name in a command palette, and with a keyboard shortcut
where it is frequent. The palette is generated from the same action list as G16, so nothing is left out. This is how the
Studio is hard to master without being hard to learn.

## G21. Never block

Engine work never runs on the renderer's main thread. Long operations (opening a run, loading fields, computing a
derived picture) show progress and can be cancelled, and the window stays responsive while runs, loads and scrubbing
happen.

## G22. Implicit by default, explicit on exception (four-layer quiet architecture)

What is normal and expected is carried by the graphic, pictogram, position and layout — never by standing prose or
machine-formatted metadata. Visible words appear at rest only as minimal noun labels, compact formatted values with
units, and exceptions (a failure, a loss, a conflict, something blocked, an unvalidated model, or a value the user must
act on).

A per-surface word budget is insufficient on its own (new surfaces fail open, and `data-words="data"` exempts machine
dumps). Every Studio view, Canvas surface and exported HTML/SVG graphic enforces four structural layers:

1. **Layer 1 — Glance vs Audit/Test separation:** Visible pixels carry only the *glance layer* (picture, short noun
   label, 3–4 significant-digit formatted ticks/readouts with unit, and exceptions). Exact `f64`/`f32` values, file
   paths, FNV-64 hashes, actor/PID records, ISO timestamps, sample counts and view specs belong in the *audit/test
   layer* (`data-*` attributes and a collapsed `<details>` block or click/keyboard popover).
2. **Layer 2 — Universal no-prose, no-machine-strings invariants:** Enforced automatically across every rendered view
   and exported HTML/SVG without per-surface registration or a `data-words="data"` escape hatch. Outside `<details>`,
   `.exception`/`.disclaimer` and user-authored text (case files, notes):
   - No `<p>` paragraphs visible at rest in graphics or canvas panels.
   - No multi-clause sentences in visible labels (no trailing `.`, no `;`).
   - No raw machine strings visible at rest (`fnv64:`, `runs/r-`, JSON `{...}`, `pid`, ISO timestamps `T\d\d:\d\d:\d\d`,
     or floats with six or more digits after the decimal point).
   - Data-ink ceiling: visible text chrome stays a small fraction of a data panel's area.
3. **Layer 3 — Typed visual slots and one shared formatter:** Panel and figure renderers accept typed slots (`label`,
   `unit`, `range`/ticks, `kpi`, `exception`, `provenance`) and format visible numbers through one shared compact
   formatter. Renderers do not accept free-form caption or explanatory-note prose parameters.
4. **Layer 4 — Image-only visual critic:** Any change touching a rendered view or export renders PNGs and passes a
   separate visual-critic review that inspects only the PNG (not the code) to flag any visible text that is inferable
   from the graphic, redundant, machine-formatted or written as a sentence.

G6's disclaimers, failed runs and known misses stay visible: they are exceptions by definition. The earlier per-surface
word-budget test (`apps/studio-web/tests/e2e/word-budget.spec.ts`) remains in place until the universal Layer 2
invariants cover every GUI surface.

## G23. Claude drives the canvas

Added 2026-11-26 ([ADR 0803](adr/0803-the-studio-as-claudes-canvas-record.md)). The open Studio shows what a client asks for through the view tools: a view is a versioned JSON specification (what to show, of which case or run, how it is laid out), validated by the engine and rendered by the Studio's existing views in its theme. A view never computes a result the engine did not give (G16). The Studio keeps a short way for a person to look (pan, zoom, scrub, pick, open the provenance) but no path whose only purpose is to reach a capability by hand. The same specification can be exported as a self-contained styled HTML page. Both the live canvas and exported HTML pages obey G22's four-layer quiet architecture (G17 provenance stays inside popovers or collapsed `<details>` + `data-*`).

## Open (for the maintainer)

- The UI framework and component approach (see `PRODUCT.md`, Stack).
- Platforms beyond Windows 11: answered 2026-11-17 for Linux (tested in Docker first, then in CI, weekly; ADR 0801
  Addendum 19); macOS not asked. Linux is recorded as "WebGL 2 on software rendering": its field views and plots run on
  Chromium's WebGL 2 fallback over Mesa's llvmpipe, WebGPU and the GPU lines of G9 are unmeasured there until a Linux
  machine with a GPU is available.
- Installers: answered 2026-11-17, built in CI (ADR 0801 Addendum 19). An installed Studio uses the engine it ships
  with (`<resources>/bin`, never PATH; the variables still override it) and starts its service with the first
  project. Updates install over the old version (checked on Windows in CI: the settings survive); signing waits for
  the maintainer's certificate (the certificate type is decided later), and a release-page notice follows once builds
  are signed.
