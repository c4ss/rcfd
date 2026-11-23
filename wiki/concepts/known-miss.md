---
type: concept
tags: [process, evidence]
sources: [doc-glossary, doc-process-agent-rules, doc-reference-data-known-misses, doc-reference-data-turbulence-finishing, doc-design-loop, doc-test-process, doc-releasing]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Known miss (and MISSED)

A registered limit the current code does not meet; its test is kept, `#[ignore]`d, with the limit unchanged. 'MISSED' is the status-note word for a registered target a result did not reach ([[doc-glossary]]).

## How rcfd uses it

- Named `*_known_miss`, with the measured number recorded; diagnosed, not tuned ([[doc-process-agent-rules]], rule 2).
- Examples in the milestone gaps: W3b of ADR 0763 (cylinder observed order 1.387 outside [1.5, 2.5]); NACA 0012 at 17 degrees ([[doc-roadmap-overview]]).
- The reference-data table of known misses after the 2026-10-14 audit ([[doc-reference-data-known-misses]]):
  - F-042 BFS Realizable 6.402, RNG 6.373 and Standard 5.359, against 6.26 +/- 0.10
  - F-061 nozzle res_u 2.62e-5, against 1e-8
  - F-043 SST vs White 0.002663 vs 0.002875 (-7.36 %), against 5 %
  - F-111 Sutherland -0.22 / -1.08 / -1.46 %, against 1 %

  See [[backward-facing-step]] and [[flat-plate]].
- ADR 0015 added misses "limits as registered, never widened", e.g. F-042 BFS-C Standard 5.350 and Realizable 6.413 against 6.16 to 6.36 ([[doc-reference-data-turbulence-finishing]]).
- Design-loop known misses: Nelder-Mead tool-loop T3 (3.1e-3 against 1e-3 after 120 candidates); NSGA-II ZDT1 with SBX (ADR 0209 MO1z); seed 16 of ADR 0209 N3 ([[doc-design-loop]]).
- `#[ignore]`d known misses are among the about 660 ignored tests the gate does not run ([[doc-releasing]]). The gate's mutation spot-check concerns non-ignored registered limits ([[doc-test-process]]).

## Related

- [[registered-limit]]
- [[roadmap-status]]
- [[simulation-milestones]]
- [[doc-reference-data-known-misses]], [[verification-vs-validation]]
- Investigation folders with known misses: [[doc-investigation-job48t-cylinder-spokes]] (K1, K2), [[doc-investigation-job49d-aoa-sweep-fixed]] (LT15, LD), [[doc-investigation-f868-naca0012-lift-curve]] (17 degrees), [[doc-investigation-job49h-te-divergence]]
