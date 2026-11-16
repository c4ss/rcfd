---
version: 1
slug: "apps-studio-prototype-src-runworkspace-tsx"
primary_target: "apps/studio-prototype/src/RunWorkspace.tsx"
related_targets: ["apps/studio-prototype"]
---

# Run workspace (Studio prototype)

Scope: the Studio run workspace, the first surface of the Electron Studio, built as a code-led design prototype in
`apps/studio-prototype/` (React 19 + Vite + react-three-fiber + three.js WebGPURenderer + React Aria Components).
The prototype is the visual reference DESIGN.md is recorded from. It is not the product. All data is synthetic and
labelled so.

Mode: Operate.

Audience and job: a learner or simulation engineer with a project folder open. They see which runs exist and their
state, follow the running one live, scrub a run through time with field, residuals and monitor moving together, and ask
Claude Code about the project.

Constraints: `docs/studio-guardrails.md` (G3 every number traceable, G6 honest limits and disclaimer, G8 WCAG 2.2 AA,
colour never alone), PRODUCT.md.

Unresolved: real data wiring (R2 reader), Claude Code bridge, the case editor and project home surfaces.

## Direction contract

THESIS: The workspace as one Aicher-strict system: every run, state and event is a grid-built pictogram with a word, and
colour is an ordered, flat scale that does jobs. It refuses the grey docked workbench with one blue accent.

OWN-WORLD: White ground; sky-blue #4FA3DB header band ruled by a 3px deep-ink #1D3A5F line; meadow green #5DB36A for the
active tab and converged; orange #F08A24 only for running; silver #C9CDD1 hairlines and queued hatch; deep red only for
failure, always with its pictogram. No black, no grey text. A neo-grotesk (Archivo) with tabular figures; square corners;
1px ink rules; pictograms on a 45/90 degree 14-unit grid.

STORY: The visitor sees at once what ran, what is running and what failed; picks a run, which comes into focus while the
others fade; scrubs time and watches field, residual curves and the Cd monitor move together; asks Claude Code why a run
failed.

FIRST VIEWPORT: Header band with projects as tabs and New run. Left 1/3: Runs table (case, id, state pictogram + word,
result) and the selected run's numbers in a hairline grid. Centre: the three.js field of cylinder-re100 with view
toggles, Mesh grid toggle and a viridis legend. Right: Claude Code chat. Bottom band: run bands timeline, residual plot
and Cd monitor, crossed by one scrubber line. Footer disclaimer in ink.

FORM: Aicher's Munich 1972 identity program, own-list position 7 of 7, seed key 3c0b0ece. Raises kept: whole-module
reflow and the mesh-grid overlay as a first-class state (Crouwel); hairline density without card padding (Japanese
high-density); selection pulls its numbers forward, the rest fades (Ikeda).

SIGNATURE: Run bands. Every run is a flat colour band segmented by phase on one shared time axis; dragging the single ink
scrubber line moves the field, the residual cursor, the Cd readout and the snapshot counter together. Motion: the scrub
is direct (no easing); band segments grow in from the left once, on load, ease-out.

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance
