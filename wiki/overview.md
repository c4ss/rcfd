---
type: overview
tags: [overview]
sources: [doc-readme, doc-product, doc-index, doc-roadmap-overview, doc-architecture, doc-glossary, doc-contributing, doc-process-agent-rules]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# rcfd — overview

Synthesis after P0 (repo docs at `origin/main` 2e94e5ef). Start here, then follow links; the map of the repo's own documentation is [[doc-index]].

## What rcfd is

A clean-room, open-source (AGPL-3.0 plus a commercial licence) finite-volume CFD suite in Rust: a command-line solver, a solver service, an [[mcp]] server that lets an AI assistant (Claude Code) drive the solver, and a desktop [[studio]] ([[doc-readme]]). Every algorithm comes from published literature or first principles ([[clean-room]], [[provenance]]). Long-term goal: an open toolset covering what commercial simulation packages do, starting with CFD, so small teams can build cheap products on it ([[doc-roadmap-overview]]).

It is early-stage and **not validated for engineering decisions** ([[doc-readme]]).

## How it is built

- 19 Rust crates in a strict layered [[crate-graph]], from [[rcfd-core]] up to [[rcfd-mcp]] and [[rcfd-cli]]. The project-folder stack is [[rcfd-project]] (L1) → [[rcfd-runner]] (L2) → [[rcfd-tsstore]] (L3), tied together for both Claude and the Studio by [[rcfd-workspace]] ([[doc-architecture]]).
- The [[studio]] is an Electron/TypeScript app outside the workspace that runs the binaries ([[doc-product]]).
- Users write a [[case-file]], check it with `rcfd check`, and start a [[run]]; runs store fields for playback ([[field-store]]) and compute [[report]]s ([[doc-glossary]]).

## How work is done

The process is evidence-first and heavily agent-driven ([[doc-process-agent-rules]]):

- Work is tracked as [[f-item]]s in the [[roadmap]] (`docs/roadmap.json` is the source of truth).
- Each item starts with a [[registration]] [[adr]] that fixes checks and [[registered-limit]]s before code.
- A miss is kept as a [[known-miss]], never tuned away; defaults stay [[bit-identical]].
- 'Done' follows the registered rule ([[roadmap-status]]); the maintainer approves lines of work, not each result.
- A lead issues briefs to agents, merges and is the single writer of the roadmap and generated pages; the [[gate]] runs the full suite on `main` daily in CI.

## Where it stands (2026-11-18)

- 786 roadmap items: 389 done, 297 partial, 100 not started (~68%) — a count of landed work, not of accuracy. The structural solver line alone has 308 items ([[doc-roadmap-overview]]).
- None of the three [[simulation-milestones]] has its headline result met: cylinder vortex shedding (89%), NACA 0012 lift (89%), assistant-driven fan design (70%).
- [[fan-01]], the reference fan, stalls in its steady [[mrf]] solve; diagnosis open. The real target is a 120 mm axial computer fan, with no measured data yet ([[doc-roadmap-overview]]).
- The structural solver ([[rcfd-fem]]) is partly built and unvalidated.

## Main threads

1. **Workspace layer** for Claude Code and the Studio (ADR 0173, F-270..F-275) — top priority since 2026-11-04.
2. **Fan programme** — FAN-01 diagnosis, the computer-fan target, design loop.
3. **Studio** — Electron rebuild (ADR 0800/0801).
4. **Structural solver** — plan ADR 0270/0290/0310.
5. **Validation milestones** — cylinder, NACA 0012.
6. **Faster, strict tests** — per-test limits, better runner.

## Contradictions

> [!warning] Contradiction — sliding mesh
> [[doc-readme]] says transient sliding-mesh solves run from a case file for the full-annulus rotor-stator generator and its sector form (F-303, ADR 0637; F-711, ADR 0646). [[doc-roadmap-overview]] ("What does not work yet", hand-written parts reviewed 2026-11-18) says they are not available. To settle against the ADRs in P3 (moving meshes thread).

> [!warning] Contradiction — structural solver in the Studio
> [[doc-product]] says a structural solver "does not exist"; [[doc-architecture]] and [[doc-roadmap-overview]] say it is partly built. See [[studio]].

## Open questions

- ADR 0763 Q1 (cylinder) and ADR 0760 Q1 (NACA 0012) wait for the maintainer ([[simulation-milestones]]).
- Close the FAN-01 diagnosis or choose another reference case? ([[fan-01]])
