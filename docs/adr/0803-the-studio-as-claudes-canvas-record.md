# ADR 0803: The Studio as Claude's canvas, and the end of the hand-navigated Studio (record)

Status: RECORDED 2026-11-26 by the lead, from the maintainer's direction and answers of that day. A record of a decision of
scope and priority; it registers no check. The items it enters register their own.

## The maintainer's direction (2026-11-26)

"As the lead let's go to an AI-native approach and kill the one screen fully navigable by hand studio. AI should be able
to 1) display results in a UX styled as we have when asked 2) display results in different ways depending on the
category of case we're analysing 3) show the full process from beginning of simulation to end when asked 4) show the
meshes being generated or edited from the tools and simulation values changing from claude MCP and other MCP tooling"

## Answers of the maintainer (2026-11-26)

The lead put four questions, each with options and a recommendation. The maintainer took the recommendation of each.

1. **What becomes of `apps/studio-web`.** The views stay and hand navigation goes: guardrail G15 (every capability of
   the engine reachable by hand) and its coverage test are retired, with the menus, editors and navigation built for
   it. The rendering parts (plots, the field and mesh views, the theme and its styling) stay as the surface Claude
   drives.
2. **Where Claude displays.** A live canvas and an export: the Studio window becomes Claude's canvas, driven through new
   MCP tools and live through the existing change feed (ADR 0188); the same view specification can also be written as
   a self-contained styled HTML page.
3. **The work in flight.** F-823 (the CI job builds the engine) finishes, since the canvas needs it too. The brief of the
   reason view of F-273 and F-974 are held. The lead sorts every open item of the Studio line against this direction,
   retires those that only serve hand navigation with a dated line, and keeps the rest.
4. **Priority.** The new line takes the place of "Studio desktop app" as line 2, with its budget of 40 registrations a
   week. The workspace layer (line 1) stays first: the canvas reads its feed.

## What follows, decided by the lead

- **The guardrails.** G15 is retired, and G19 (explain in place) with it: Claude explains in its own words, from the
  engine's docs, in the conversation. G16 stays and is now the principle of the line: every view the canvas shows is
  asked for by a named engine action (an MCP tool, a service method or a CLI command) that can be scripted and recorded.
  G3 (a picture can be wrong and look right), G6 (honest about limits), G8 for what is displayed (contrast, colour-blind
  safe maps, focus on what can be focused), G13 (testable without eyes) and G17 (provenance) stay. A new G23 says what
  the canvas is (below).
- **G23. Claude drives the canvas.** The open Studio shows what a client asks for through the view tools: a view is a
  versioned JSON specification (what to show, of which case or run, how it is laid out), validated by the engine and
  rendered by the Studio's existing views in its theme. A view never computes a result the engine did not give (G16).
  The Studio keeps a short way for a person to look (pan, zoom, scrub, pick, open the provenance) but no path whose only
  purpose is to reach a capability by hand.
- **The four abilities as items** (line 2, entered on this record):
  - the view specification and the tool that shows it in the open Studio (ability 1);
  - layouts chosen by the category of the case (ability 2);
  - the process view from the case's creation to its last result (ability 3);
  - the live view of a mesh being generated or edited and of values changing, as MCP clients make the changes
    (ability 4);
  - the export of a view as a self-contained styled HTML page (the answer to question 2);
  - the retirement of G15's coverage test and of the hand-only surfaces, each retired test named (the answer to
    question 1).
- **The Studio line's open items.** Retired (moved to the category "Retired: the hand-navigated Studio" of
  `scripts/roadmap_progress.py`, each with a dated line in its note): F-151, F-264 and F-266 (the wgpu and egui Studio,
  already superseded by G14), F-293 (retired by the maintainer on 2026-11-18), F-448 (parity with rcfd-studio), F-813
  (a learner's first path through the menus) and F-830 (explain in place, which Claude now does in its own words). Kept
  on the line: F-261, F-262 and F-263 (the engine's project folder, run service and field store), F-447 (field transfer
  and draw, which the live view needs), F-814 (Claude Code registered and followed live), F-817 (performance bench),
  F-821 (Linux and macOS), F-823 (CI), F-848 (remote runs) and F-890 (a failing test of a view that stays). Held:
  F-828 and F-974 (provenance), and the reason view of F-273, until the view specification exists, so that the reason
  and the provenance are shown by it rather than by a hand-built panel.
- **Order of issue.** The view specification first, since every other item of the line shows through it; then the live
  view (ability 4) and the process view (ability 3), which read the feed; then the category layouts and the export; the
  retirement of the coverage test with the first of them, so that no new view is held to G15.

## Not decided here

The case categories and their layouts, the specification's fields and the tool names are the registrations' to fix. A
change of an engine interface stays additive under G4; the MCP tool list golden changes in the additive class.
