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
rule 2). `crates/rcfd-api/tests/baseline_hashes.rs` is never re-recorded.

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

- The Studio owns only the runner and the service it started itself in this session. A runner already alive when it
  opened the project (an earlier Studio's, the CLI's, Claude Code's) and a service attached to through
  `RCFD_STUDIO_SERVICE` or `--service` are never stopped by it. A runner that comes up while the Studio has the project
  open, with none alive before, is the Studio's, whoever asked for it (its Run, or Claude Code's `start_run`).
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

## G13. Testable without eyes

Agents cannot see the window. Each roadmap item has automated tests (unit tests, Playwright driving Electron) or a
view-and-report step for the maintainer, named in its acceptance. Each wave ends with a short view-and-report
checklist. Whether `scripts/gate_test.py` also runs the Studio's npm tests is the maintainer's decision.

## G14. rcfd-studio is retired

The maintainer decided on 2026-11-16 to delete `rcfd-studio` and `rcfd-gui` before parity, to free test and build
time (F-449, ahead of F-448). Git history keeps them. Capabilities only they had are roadmap items for the Electron
Studio; no doc may tell a user to run them. (Before 2026-11-16 this rule froze them until parity.)

## G15. Everything the engine can do, the Studio can do

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
does it. Any number on screen can show where it came from: run id, snapshot, file, engine version and unit. Every value
carries its unit (from the engine, G2). Builds on `docs/provenance.md`.

## G18. Non-destructive by default

Edits can be undone. Deleting a run, result or case is recoverable or confirmed in words that name what is lost. The
Studio never silently overwrites a file another writer (Claude Code, the CLI, an editor) has changed since it was read;
it shows the conflict and lets the user choose.

## G19. Explain in place

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

## G22. Implicit by default, explicit on exception

What is normal and expected is carried by the pictogram, position and layout, with no visible words of its own. Visible
words appear only for the exception: a failure, a loss, a conflict, something blocked or unavailable, or a value the user
must act on. Everything G6, G8, G18 and G19 ask to be said in words is still said, but in the accessible name, a tooltip
or focus detail, the help on demand or the command palette, not as standing text on the screen. Tests check those words
by role and accessible name, so G13 holds without painting a sentence for the test to read. G6's disclaimers, failed
runs and known misses stay visible: they are exceptions by definition. A surface that grows adds no standing prose for
its normal state; a feature that needs a sentence to be understood at rest is redesigned, not annotated. Each surface has
a budget of visible chrome words at rest (labels, notes, hints and empty-state text; not data, disclaimers or
exceptions), set from its screenshots once the surface is quieted and approved by the maintainer; a test counts them
from the DOM and fails past the budget, and raising a budget needs the maintainer's word.

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
