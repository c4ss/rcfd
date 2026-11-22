---
type: concept
tags: [physics, rotating]
sources: [doc-glossary, doc-readme, doc-meshing, doc-component-diagonal-default, adr-thread-fan01-rotating-solve, adr-thread-sliding-mesh-rotor-stator]
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
- F-126 (frozen-rotor and stage interfaces with sector periodicity) is partial; its gap names "MRF on the bladed FAN-01 passage (blade wall refused by the revolution check)" and "transient and coupled solvers" among what is missing ([[adr-thread-fan01-rotating-solve]]).
- The transient alternative, a sliding rotor-stator interface, runs from a case file on the bladeless cylindrical-interface annulus and sector (F-303, F-711), laminar only ([[adr-thread-sliding-mesh-rotor-stator]]). See [[sliding-mesh]].

## Related

- [[adr-thread-fan01-rotating-solve]]
- [[fan-01]]
- [[sliding-mesh]]
- [[fan-passage-mesh]]
