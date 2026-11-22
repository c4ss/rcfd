---
type: source
tags: [fan, fan-01, design, meshing, io]
sources: []
origin: docs/designs/m-fan-b.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# M-FAN-B: surface import and meshing pipeline (design)

**Origin:** [docs/designs/m-fan-b.md](../../docs/designs/m-fan-b.md) at origin/main 2e94e5ef
**Kind:** repo design doc by job J4, dated 2026-11-04, "Status: Proposed", for review by the maintainer. Registration: ADR 0184.

## Summary

M-FAN-B is the second milestone in the parent design's build order: after EXP-ROT, before the rotor-only FAN-01 run (M-FAN-A1), which runs on the blade mesh this milestone produces. Pass condition (G9): "a case meshes and runs the FAN-01 blade from a surface file with units, with prism layers and a hash-keyed rebuild." Items: F-279, F-280 (OBJ and glTF readers), F-108, F-132, F-232, F-234. The doc details the whole milestone and gives a file-level plan for step 1 (F-280) only. "Rules this document introduces ... are proposals of job J4."

## Key points

- The problem: surface-pipeline library code exists (diagnose/repair F-232, remesh F-131, tetrahedralize F-132, octree fill F-234, inflate F-108) but "there is no CLI or case-file hook"; binary STL loses patch names; STL and OBJ carry no unit; prism layers are not merged into a fluid mesh on unstructured walls (octree fill missed non-orthogonality 77.4 degrees against 70, known miss); the FAN-01 mesh in use "is a NACA-section fit to the published CAD, not the CAD itself", and `cad_model.igs` (Zenodo, CC BY 4.0) is not vendored and has no fetch entry.
- Goals B1 to B5: readers (unit a required argument, never guessed; boundary types never guessed; no new dependency); surface intake with refusal of open surfaces; volume mesh with merged prism layers (route open question Q1); `geometry.*` case keys with a hash-keyed rebuild and a patch diff that blocks a run on a removed BC patch; acceptance on the FAN-01 blade with limits A1 to A7 in ADR 0184.
- Reader rules: bitwise vertex welding; OBJ `o` objects are patches (groups optional); glTF nodes are patches, transforms applied, negative determinant reverses triangle order; compressed glTF and sparse accessors refused.
- Non-functional: parser rules of ADR 0011 (1 GiB cap, no panic, fuzzed); determinism; no network; no default change.
- Open questions Q1 (route T recommended, route O as cross-check), Q2 (who exports the acceptance surface; recommendation: the maintainer, committed with attribution), Q3 (one blade in a straight box duct recommended), Q4 (`geometry.variant` later). "This is a recommendation only."
- Step 1 plan: `crates/rcfd-io/src/common.rs`, new `obj.rs` and `gltf.rs`, registry entries, tests R1 to R11, doc updates including [[doc-provenance]] "File formats".

## Contradictions with the wiki

None found.

## Pages this source informs

- [[fan-01]], [[rcfd-io]], [[rcfd-mesh]], [[doc-designs-exp-rot]], [[provenance]]
