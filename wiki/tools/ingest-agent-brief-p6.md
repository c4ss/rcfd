# P6 addendum: foundation pages

Read `wiki/tools/ingest-agent-brief.md` first; everything there applies. Then read `wiki/ingest-plan.md` section "P6 — Foundations" and decisions 5-6, `wiki/analyses/foundations-map.md` (your rows) and `wiki/analyses/foundations-sources.md` (your branch's references). This brief adds the P6 rules.

## Sources (strict)

- Every claim on a foundation page cites a `ref-*` source page made from a reference **you actually read in this job**, with a locator (section, equation number, page). Use WebFetch to read it. If you cannot read a source, you cannot cite it; do not write from memory. If a node has no readable source, leave it out and list it under `## Not ingested` in your handoff.
- Source pages: `wiki/sources/ref-<short-name>.md` (e.g. `ref-menter-1992-nasa-tm-103975`), `type: source`, frontmatter `origin: <URL>`, `licence: <as stated on the page>`, `read: 2026-11-22`. Body: full citation, URL, licence, what it covers (sections), and per section our own summary with locators. Check `wiki/sources/ref-*` first: another job may have made it; then add your sections to it.
- **Copy into `raw/` only** if the licence is verified on the page as public domain / US Government public use, CC0 or CC BY (not BY-SA, not NC, not "all rights reserved", not unknown). Otherwise cite only. Never copy Wikipedia.
- **Wikipedia:** a cited secondary source for elementary concepts only (stress, shear, viscosity, Taylor series). Never the sole source of a model-level claim (SST, PISO, AMG, Rhie-Chow, FW-H, FEM elements, GCI).
- **Non-commercial material** (MIT OCW, LibreTexts/OpenStax NC books): read and cite, never copy.
- **Never** vendor documentation (ANSYS, Fluent, CFX, STAR-CCM+, COMSOL, Siemens, Autodesk, MSC, Abaqus, etc.). That is the clean-room rule.
- Our own words. Quote only short phrases. Equations may be written out with the source's equation number.

## Page form

Each foundation node is `wiki/concepts/<node>.md`, using the name from the map. Frontmatter:
```yaml
type: concept
tags: [foundation, <branch>]
sources: [ref-...]
builds_on: [node, node]   # direct prerequisites from the map, adjusted if the sources say otherwise
created: 2026-11-22
updated: 2026-11-22
status: draft
```
Sections:
1. `## In short`: 1-3 sentences for a reader with first-year calculus and physics.
2. `## Explanation`: the concept, its defining equations with symbols explained, and what it adds to its prerequisites. For a model: what problem the previous model had and how this one fixes it.
3. `## Builds on`: links to the `builds_on` nodes, each with one line on what is used from it.
4. `## Leads to`: links to the nodes and rcfd pages that build on this one.
5. `## How rcfd uses it`: links up to the existing rcfd concept/entity pages, ADR threads and roadmap areas, with one line each, cited to those wiki pages. Only what the wiki already says. Don't invent rcfd facts.

Depth stop: stop where a reader with first-year calculus, linear algebra and physics would follow. 30-120 lines per page.

## Existing rcfd concept pages

For the rcfd pages your branch leads to (e.g. `k-omega-sst`, `piso`, `amg`): add `builds_on: [...]` to the frontmatter and a `## Builds on` section with links. Additive only. If they have an "In short" gap, you may add a sourced `## In short` too.

## Coordination

Shared-root nodes (stress, strain, viscosity, conservation law, ...) belong to job p6-roots. Other jobs link to them by their map name even if the page does not exist yet. Lint broken links to shared-root names are expected until p6-roots finishes. Create a node outside your branch only if the map lists it in your branch.

## Handoff

As in the main brief, plus `## Not ingested`: nodes skipped for lack of a readable source, with what you tried.
