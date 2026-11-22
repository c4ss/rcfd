---
type: concept
tags: [case, validation]
sources: [doc-glossary, doc-milestones-json, doc-reference-data, doc-validation-data]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Benchmark descriptor

A `case.json` file in `cases/` describing a validation case and its reference data. Not a case file; `rcfd run` refuses it ([[doc-glossary]]).

## How rcfd uses it

- Example: `cases/naca0012/case.json` for the NACA 0012 milestone ([[doc-roadmap-overview]]).
- `docs/milestones.json` lists `cases/naca0012/case.json` among the cases of the `aerofoil-lift` milestone, next to test files and scripts ([[doc-milestones-json]]).
- Descriptors sit beside in-repo reference tables, such as `cases/cavity/reference/*.csv`, `cases/flat_plate/reference/correlations.csv`, `cases/naca0012/reference/naca4_thickness.txt` and `cases/heated_cavity/reference/devahldavis1983.csv`. [[doc-reference-data]] records where each value in those tables comes from ([[doc-reference-data]]).
- In-repo reference tables (`cases/*/reference`) are also listed in `validation-data/manifest.json` with `local` paths and checksums, and `rcfd cases list` shows the data behind each case ([[doc-validation-data]]).

## Related

- [[case-file]]
- [[naca0012]], [[lid-driven-cavity]], [[flat-plate]], [[backward-facing-step]], [[differentially-heated-cavity]], [[validation-data-pack]]
- [[simulation-milestones]]
