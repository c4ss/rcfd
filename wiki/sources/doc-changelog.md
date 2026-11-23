---
type: source
tags: [release, changelog, status]
sources: []
origin: CHANGELOG.md
origin_commit: 8732f2ae
created: 2026-11-22
updated: 2026-11-23
status: draft
---

# Changelog (0.1.0, not yet released)

**Origin:** [CHANGELOG.md](../../CHANGELOG.md) at origin/main 09647baa (first read at 2e94e5ef)
**Kind:** repo doc (root): user-facing release notes; `Unreleased` says "Nothing yet".

## Summary

The prepared first release, 0.1.0, is "not yet released". It describes rcfd as "early-stage software. It has not been validated for engineering decisions, must not be used for safety-critical work, and comes with no warranty." It states roadmap counts: ~~"Of the 786 roadmap items, 389 are done, 297 partly done and 100 not started"~~ (at 2e94e5ef); ~~at 5b96a1f8 "Of the 791 roadmap items, 391 are done, 297 partly done and 103 not started"~~; ~~at 8ee32b1c "Of the 792 roadmap items, 391 are done, 297 partly done and 104 not started"~~; ~~at 09647baa "Of the 794 roadmap items, 403 are done, 299 partly done and 92 not started"~~; at 8732f2ae (the wiki merged, F-886 on main) "Of the 795 roadmap items, 403 are done, 299 partly done and 93 not started" (the lead's main, which does not hold F-886; on the tree that merges the two lines, d0fb2b2e, the sentence reads 795, 403, 299 and 93) ([[doc-roadmap-json]]). The counts line is the only change at each of these commits. Before 1.0 a minor version may break CLI options, case files, MCP tools, the service protocol, the UDF ABI, Python functions, output formats or default numerical results; a patch version does not.

## Key points

- Added: finite-volume solver (incompressible and compressible, turbulence models, heat transfer, species, one-way particles); mesh import/export, quality check, VTK output; the `rcfd` CLI, JSON-RPC service and MCP server; a Python package and WebAssembly user functions.
- For the computer-fan target, "the parts that do not need a converged fan solve: parametric blade geometry and sector meshes, tolerance tools, a reduced-order fan performance model, rotor-dynamics checks and a design ledger that keeps every variant."
- Studio libraries tested without a window; the egui Studio's wgpu render core and `rcfd-claude` bridge were removed (F-449, F-838).
- Executed tutorials, the validation data pack, and "a suite of about 7,500 tests whose numeric limits were written down before each run".
- Known limitations:
  - "The FAN-01 reference fan does not reach its operating point. The steady rotating-frame solve stalls, the comparison with the measured fan misses, and the cause has not been found". "No fan prediction in this release is validated against measurement."
  - Fan noise: FW-H, tonal estimate and broadband proxies exist, "but none is validated".
  - The structural solver is new; none of it is validated against measurement.
  - No sliding-mesh transient solves, so no rotor-stator interaction in time.
  - The old desktop app is frozen and the new one has no window yet.
  - CUDA optional and off by default; without a GPU the GPU tests skip and count as passed.
  - The full test gate "runs on `main` in CI on Windows, not before each merge"; Linux by CI (build, clippy, unit tests); macOS not checked.
  - Crates not published; library API may change.

## Contradictions with the wiki

None found.

## Pages this source informs

- [[fan-01]], [[computer-fan-target]], [[gate]], [[doc-releasing]], [[simulation-milestones]], [[design-ledger]]
