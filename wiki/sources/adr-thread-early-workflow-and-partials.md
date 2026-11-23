---
type: source
tags: [adr-thread, workflow, wiring, restart, chain, setup-rules, study, python, post, ensight, design-loop, udf, docs-site, adjoint, partials]
sources: [doc-run-control, doc-studies, doc-python, doc-udf-abi, doc-tutorial-gallery, doc-partials-audit]
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: wiring and workflow features of the early waves (case tree and CLI wiring, restart and chains, studies and Python, post catalogue, design loop, UDF hooks, docs site, adjoint) and their partials close-outs

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations with results, partials close-outs), 10 ADRs, wave 16W to wave 40C.

These ADRs move library features to the user: the case tree, the CLI and the JSON-RPC service (ADR 0021), restart and chained runs with a 21-rule setup checker (ADR 0031), parametric studies and a pure-Python package (ADR 0034), a post catalogue and an EnSight Gold writer (ADR 0048), the design-loop infrastructure (ADR 0067), user-function hooks (ADR 0088), a docs site and executed tutorials (ADR 0089) and a small discrete adjoint (ADR 0091). Waves 39A and 40C close partials with tests only (ADRs 0092, 0099). See [[setup-rules]], [[parametric-study]], [[udf-sandbox]], [[executable-tutorials]], [[design-loop]].

## Current state

- **Done in this range:** F-240 (setup rules, under "a reading of 'published validation cases' registered here for the lead to accept or reject"), F-118 and F-073 (ADR 0092); F-095 (five-velocity sweep, ADR 0099); F-137 (adjoint, "set to `done` by the registered rule", ADR 0091); F-241 (ADR 0097, on [[adr-thread-early-physics-models]]); F-096 was set `partial` "as a conservative choice, and the lead may set it to `done` on the registered acceptance" (ADR 0089).
- **Still partial here:** F-115 (the interpolated start "has a lower momentum residual (ratios 0.592 and 0.591) but a higher pressure-equation residual (ratios 1.181 and 1.175)", ADR 0099); F-239 (hooks H1 to H7 only, ABI version 1, ADR 0088); F-238 and F-103 (no case-tree keys for surfaces, rakes, point clouds; "Only readers written by the same author checked the EnSight files", ADR 0048); F-083 (the PyO3 extension "stays a named gap", ADR 0034).
- **Restart and chains:** transient laminar runs write and continue restart files; the `chain` section runs stages in order; "All error-level rules except SV011 mirror checks the case loader already enforces, so through the binary an error-level case exits 3 from the loader before the rules run" (ADR 0031). See [[checkpoint]], [[exit-codes]].
- **Design loop (ADR 0067):** a PIMPLE divergence guard, operating-map automation, DOE and optimisation, assistant tools; end-to-end demo E1 reduced a pressure drop "9.942 to 5.0716, 49.0 % reduction, limit 25 %"; "the real fan is not part of this wave". Later design work: [[adr-thread-design-optimisers-and-ledger]].
- **Adjoint (ADR 0091):** FFD morphing and "a fixed-point discrete adjoint with a finite-difference-assembled dense Jacobian of the one-iteration SIMPLE/SIMPLEC map, for small meshes only"; "Turbulent gradients are refused, and gradients are not delivered inside `design::study`, the `design.*` service methods, MCP tools or case files".

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0021](../../docs/adr/0021-wave16w-wiring.md) | | wiring | F-072, F-108, F-236 | Partitioning, derived fields, sampling and inflation through the case tree, CLI and service | all met, none a known miss; P9 used `--max-iterations 1` instead of 0 | |
| [0031](../../docs/adr/0031-wave19c-restart-chain.md) | | registration + results | F-115, F-121, F-240, F-241 | Restart, field transfer, chained runs, run-control additions, setup rules | I2, I5, S5a, R1, R4, E2 short of PASS; B1 hashes unchanged | 0092, 0097, 0099 |
| [0034](../../docs/adr/0034-wave20c-study-python.md) | | registration + results | F-083, F-095 | Parametric studies (`rcfd study`) and Python access (`python/rcfd`) | S7 known miss (1.86 to 1.91 % against 1 %); S1, S3 at 1e-12 known misses | 0099 |
| [0048](../../docs/adr/0048-wave26c-post-catalogue-export.md) | | registration + results | F-103, F-238 | Post catalogue, scalar report kinds, EnSight Gold export | all limits met, no known miss; no third-party reader | |
| [0067](../../docs/adr/0067-wave32c-design-loop.md) | | registration + results | F-095, F-786 | Divergence guard, operating maps, DOE, optimisation, assistant tools | T3 known miss (best f 3.1255e-3 after 120 candidates against 1e-3); E1 met | |
| [0088](../../docs/adr/0088-wave38a-udf-hook-catalogue-v2.md) | | registration + results | F-239 | User-function hook catalogue v2, first slice (H1 to H7) | every hook meets its check (equivalence exactly 0); F-239 partial | |
| [0089](../../docs/adr/0089-wave38b-docs-site-tutorials-gallery.md) | | registration + results | F-096 | Static docs site, executed tutorials, example gallery | acceptance met in full; set partial conservatively; GUI steps `not-executed` | |
| [0091](../../docs/adr/0091-wave38d-adjoint-shape-sensitivities.md) | | registration + results | F-137 | Drag sensitivity of a 2D duct by adjoint against finite differences | A1 all six pairs (worst 1.12e-6); F-137 done | |
| [0092](../../docs/adr/0092-wave39a-partials-closeout-rules-fluent-render.md) | | partials close-out | F-073, F-118, F-240 | Setup rules on validation cases; 3D Fluent mesh; timed 1M-cell render | all met (R4 1.555 s against 2 s); three items done; V2 Graetz set-up known miss | |
| [0099](../../docs/adr/0099-wave40c-five-velocity-sweep-fixed-norm-interpolated-start.md) | | partials close-out | F-095, F-115, F-249 | Five-velocity sweep; initial residual with fixed normalisation | P1, P2 met (F-095 done); F1 missed (F-115 partial) | |

## Key numbers and limits

- ADR 0031 S5a: "all three methods took more iterations to tolerance after a transfer (96 to 103) than a cold start (89)"; S5b: "within 1e-2 of the converged solution in 10 iterations versus 23 cold".
- ADR 0067 Addendum 1: "the guard trips at step 130 (limit 300) on candidate B, because candidate A does NOT blow up within 500 steps"; the Sobol (Joe-Kuo) direction numbers "were typed from memory".
- ADR 0091: "For these 3 parameters the adjoint costs about 3 times more than one-step central differences; it breaks even at about 9 parameters on this mesh"; "All six reference values were known when the limits were written".
- ADR 0099 P2: relative dp errors "from 0.251 % to 0.309 % at ny 20" against 1 %.
- ADR 0089: the GitHub workflow "has not run, because nothing is pushed"; tests "have run on Windows only".
- ADR 0088: the last complete gate is that of 70a9406; "the re-gate of d6f33a5 was stopped by the host for low memory".

## Open questions

- ADR 0092: the registered reading of F-240's "published validation cases" is "for the lead to accept or reject"; review found two statements of it wrong, and the FAN-01 case of `fan01_validation.rs` is not covered.
- ADR 0089: F-096 `done` is for the lead.
- ADR 0099: the likely cause of the F1 miss ("the face flux rebuilt from the interpolated velocity") "was not tested".

## Related

- [[setup-rules]], [[parametric-study]], [[udf-sandbox]], [[executable-tutorials]], [[design-loop]], [[checkpoint]], [[exit-codes]], [[rcfd-python]], [[rcfd-post]]
- [[adr-thread-design-optimisers-and-ledger]], [[adr-thread-early-meshing]]
- [[roadmap-command-line-python-and-api]], [[roadmap-post-processing-and-output]], [[roadmap-design-studies-and-optimisation]], [[roadmap-quality-docs-and-validation]]
