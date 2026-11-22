---
type: index
updated: 2026-11-22
---

# Index

Catalog of every wiki page. Read this first on any query. Format: `- [[page-name]] — one-line summary (n sources, updated YYYY-MM-DD)`

## Overview

- [[overview]] — Synthesis after P0: what rcfd is, how it is built and worked on, where it stands, open contradictions (8 sources, updated 2026-11-22)

## Plans

- [[ingest-plan]] — Phased plan to ingest the existing repo docs; P0 done (updated 2026-11-22)

## Sources

- [[doc-architecture]] — The workspace crate dependency graph and its layering rules, for contributors adding or moving code. (origin `docs/architecture.md`, updated 2026-11-22)
- [[doc-contributing]] — Licence grant, the clean-room rule, the checks to run before submitting, and the registration-first rule for roadmap work. (origin `CONTRIBUTING.md`, updated 2026-11-22)
- [[doc-glossary]] — The project's own terms as other pages and the Studio's explanations use them. (origin `docs/glossary.md`, updated 2026-11-22)
- [[doc-index]] — The map of the documentation, grouped by intent: tutorials, tasks, reference, how-and-why, status/plans/reports. (origin `docs/index.md`, updated 2026-11-22)
- [[doc-process-agent-rules]] — How work is issued, recorded, checked and merged. (origin `docs/process/agent-rules.md`, updated 2026-11-22)
- [[doc-product]] — Product definition of the Studio: platform, stack, users, purpose, positioning, constraints, principles and accessibility. (origin `PRODUCT.md`, updated 2026-11-22)
- [[doc-readme]] — What rcfd is (clean-room AGPL-3.0 finite-volume CFD suite in Rust: CLI solver, solver service, MCP server, desktop app), its main limits, how to ... (origin `README.md`, updated 2026-11-22)
- [[doc-roadmap-overview]] — Plain-language status: what rcfd is, counts by line of work, simulation milestones, what works, what does not, current priorities and what comes next. (origin `docs/roadmap-overview.md`, updated 2026-11-22)
- [[karpathy-llm-wiki]] — The LLM-wiki pattern: an LLM builds and maintains a persistent, interlinked markdown wiki instead of query-time RAG. (origin `https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f`, updated 2026-11-22)

## Entities

- [[crate-graph]] — The dependency graph of the 19 workspace crates under `crates/` and its layering rule. (1 sources, updated 2026-11-22)
- [[fan-01]] — The reference fan case, a 500 mm low-pressure axial fan from the FAU / EAA open data set (CAD and measured characteristic, CC BY 4.0). (3 sources, updated 2026-11-22)
- [[rcfd-api]] — The API crate over the solver crates. (3 sources, updated 2026-11-22)
- [[rcfd-cli]] — The `rcfd` command-line binary (`rcfd check`, `rcfd run`, `rcfd project init`, ...). (1 sources, updated 2026-11-22)
- [[rcfd-core]] — Base crate with no rcfd dependencies: scalar type, units, errors, logging, the settings tree, hashing. (1 sources, updated 2026-11-22)
- [[rcfd-dist]] — Transport-agnostic distributed-memory layer: communicator, halo exchange, distributed matrix and Krylov solvers. (1 sources, updated 2026-11-22)
- [[rcfd-fem]] — Finite-element structural solver: the solid data model and structured builders. (2 sources, updated 2026-11-22)
- [[rcfd-fv]] — The finite-volume core. (The architecture page gives no description; ADR 0115 places the gradient limiters in `rcfd_fv::gradient`.) (1 sources, updated 2026-11-22)
- [[rcfd-gpu]] — Optional CUDA device solvers behind cargo feature `cuda`. (1 sources, updated 2026-11-22)
- [[rcfd-io]] — File input and output. (The architecture page gives no description; to confirm in P1.) (1 sources, updated 2026-11-22)
- [[rcfd-linalg]] — Linear algebra and linear solvers. (1 sources, updated 2026-11-22)
- [[rcfd-mcp]] — The MCP server: project tools behind `--project`, as MCP descriptors over the workspace layer. (1 sources, updated 2026-11-22)
- [[rcfd-mesh]] — Mesh data structures. (1 sources, updated 2026-11-22)
- [[rcfd-particles]] — One-way coupled Lagrangian particle tracking. (1 sources, updated 2026-11-22)
- [[rcfd-post]] — Post-processing and VTK output, including VTK output of a solid result and the solid glue modules. (1 sources, updated 2026-11-22)
- [[rcfd-project]] — Studio layer L1, the project folder: run records, atomic files, path safety, poll watcher. (1 sources, updated 2026-11-22)
- [[rcfd-reactor]] — 0D reactor toolkit with a stiff ODE integrator. (1 sources, updated 2026-11-22)
- [[rcfd-runner]] — Studio layer L2, the run service: runs the `rcfd` binary as child processes and owns every run of a project. (1 sources, updated 2026-11-22)
- [[rcfd-tsstore]] — Studio layer L3, the time-series field store of a run (playback of stored fields). (1 sources, updated 2026-11-22)
- [[rcfd-turb]] — Turbulence models. (1 sources, updated 2026-11-22)
- [[rcfd-workspace]] — The project folder's commands, queries and single write rule, shared by the MCP server and the Studio; the project journal and change feed. (2 sources, updated 2026-11-22)
- [[roadmap]] — The list of all f-items with dependencies, acceptance and status. (3 sources, updated 2026-11-22)
- [[simulation-milestones]] — Named simulations with a stated result; the per cent counts landed prerequisite items, and only the 'headline result' says whether the result agrees. (1 sources, updated 2026-11-22)
- [[studio]] — The desktop application, an Electron app in `apps/studio-web/` (ADR 0800), in development and run from a checkout; no installer yet. (4 sources, updated 2026-11-22)

## Concepts

- [[adr]] — A numbered page in `docs/adr/` that records one decision or one batch of work: its options, the limits set before the work and the measured results. (2 sources, updated 2026-11-22)
- [[benchmark-descriptor]] — A `case.json` file in `cases/` describing a validation case and its reference data. (1 sources, updated 2026-11-22)
- [[bit-identical]] — A change leaves the existing default results unchanged to the last bit, checked by stored result hashes. (2 sources, updated 2026-11-22)
- [[case-file]] — A TOML or JSON file that describes one simulation: mesh, physics, boundaries, solver and reports. (1 sources, updated 2026-11-22)
- [[clean-room]] — Every algorithm comes from published literature or first principles, with no vendor code, binaries or documentation text. (4 sources, updated 2026-11-22)
- [[f-item]] — A roadmap item id such as F-028. (3 sources, updated 2026-11-22)
- [[field-store]] — A field is a value in every cell (`ux`, `uy`, `p`). (2 sources, updated 2026-11-22)
- [[gate]] — `cargo fmt --check`, `cargo clippy` with warnings as errors, and the whole test suite in release mode on a clean copy of the commit; over an hour; ... (3 sources, updated 2026-11-22)
- [[known-miss]] — A registered limit the current code does not meet; its test is kept, `#[ignore]`d, with the limit unchanged. (2 sources, updated 2026-11-22)
- [[llm-wiki]] — A knowledge base the LLM writes and maintains, rather than documents searched at query time (1 sources, updated 2026-11-22)
- [[mcp]] — The interface through which an AI assistant calls rcfd's tools. (3 sources, updated 2026-11-22)
- [[mrf]] — A steady solve in which a rotating zone is computed in its own rotating frame (ADR 0008). (2 sources, updated 2026-11-22)
- [[provenance]] — The record of where a piece of knowledge came from, including every case in which vendor documentation was consulted (`docs/provenance.md`). (2 sources, updated 2026-11-22)
- [[registered-limit]] — A pass/fail limit written down before the run it judges. (2 sources, updated 2026-11-22)
- [[registration]] — The first commit of every roadmap item: an ADR that fixes the checks, their numeric limits and the decision rule before any code or run. (3 sources, updated 2026-11-22)
- [[report]] — A number a run computes as it goes, named in the case's `[reports]` table, e.g. (1 sources, updated 2026-11-22)
- [[roadmap-status]] — The three status marks of an f-item. (3 sources, updated 2026-11-22)
- [[run]] — One solve of a case by the project's runner, kept in `runs/<run id>/` with settings, status, log and results; it goes on when the Studio is closed. (2 sources, updated 2026-11-22)
- [[template]] — A ready-made case the engine can write into a project (`new_case {name, template}`); the engine checks the case first, never replaces a file, and ... (1 sources, updated 2026-11-22)
- [[unverified]] — UNVERIFIED marks a value not checked against its source; 'NOT read' means a source was not opened. (2 sources, updated 2026-11-22)
- [[wave]] — A numbered batch of work, sometimes split into lettered parts (wave 35C). (1 sources, updated 2026-11-22)

## Analyses

_none yet_
