---
type: concept
tags: [physics, rotating]
sources: [doc-glossary, doc-readme, doc-meshing, doc-component-diagonal-default]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# MRF (multiple reference frame / frozen rotor)

A steady solve in which a rotating zone is computed in its own rotating frame (ADR 0008) ([[doc-glossary]]).

## How rcfd uses it

- The steady rotating-frame solve of [[fan-01]] does not reach its expected operating point ([[doc-readme]]).
- The absolute-formulation `mrf` source is refused on the bladed fan sector mesh (blade and seam faces are not surfaces of revolution; located diagnostic on `sources.<name>.zone`); smoke solves there use the relative-frame `rotating_frame` source ([[doc-meshing]]). See [[fan-passage-mesh]].
- The exact annular sector `annulus_3d_sector` (ADR 0116) has planar faces, so an MRF zone with cyclic seams is exact per cell; it has no blade, so "it does not open MRF on the fan passage" ([[doc-meshing]]).
- MRF zones always use the corrected component diagonal, whatever `solver.component_diagonal` says ([[doc-component-diagonal-default]]). See [[component-diagonal]].

## Related

- [[fan-01]]
- [[sliding-mesh]]
- [[fan-passage-mesh]]
