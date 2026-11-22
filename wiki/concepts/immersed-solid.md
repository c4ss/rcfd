---
type: concept
tags: [immersed-boundary, forcing, moving-body, cartesian]
sources: [adr-thread-immersed-solid]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Immersed (non-conforming) solid

A solid body represented inside a mesh that does not fit its surface: the cells inside the body and next to its surface get a modified forcing that imposes the wall, so a Cartesian background can carry a cylinder without a body-fitted grid. In rcfd it is F-219 (ADR 0218), library only, checked on the DFG cylinder benchmarks ([[adr-thread-immersed-solid]]).

## How rcfd uses it

- **First order:** the forcing of F-219 converges at `1.023` on circular Couette flow against the registered `1.5` (known miss); the DFG acceptance numbers are within 1 percent (2D-1 `C_D` `-0.0215 %`, `C_L` `+0.669 %`, 2D-2 `St` `-0.382 %`) ([[adr-thread-immersed-solid]]).
- **Forces:** a second-order moment beside `force_moment` (F-337); for a body at rest the reaction of the forcing is already the unsteady force (F-346). On D/80 meshes the 2D-2 extrema are within `0.43` percent with either forcing (F-354) ([[adr-thread-immersed-solid]]). See [[dfg-cylinder-benchmark]].
- **Second-order forcing:** `build_second_order` stops in transient PISO; `build_second_order_bounded` runs; one PISO corrector is refused unless the start is projected (`PimpleSolver::project_immersed_start`, F-515). Making the bounded form the only constructor (F-364) was held by the maintainer on 2026-11-18 ([[adr-thread-immersed-solid]]).
- **Moving shapes:** opt-in `MovingImmersed` (F-308), checked against body-fitted moving-mesh runs for in-line and transverse oscillation; lock-in at Re 100 not yet matched (F-731, F-750, F-756 partial) ([[adr-thread-immersed-solid]]).
- **Not available:** a case-file key (needs the F-307 schema golden), rotating or deforming shapes, free motion, three dimensions beyond level sets ([[adr-thread-immersed-solid]]; F-219 gap in [[roadmap-moving-meshes-rotors-and-coupling]]).

## Related

- [[overset-mesh]] (the other way to move a body without a body-fitted global mesh)
- [[dynamic-mesh]], [[piso]], [[cylinder-re100]], [[dfg-cylinder-benchmark]]
- [[adr-thread-immersed-solid]]

## Open questions

- Case-file key for immersed and moving immersed bodies (ADR 0631 Q1 and successors; F-517 for the start projection) ([[adr-thread-immersed-solid]]).
- The AMG pressure-CG breakdown on the T mesh is remedied only by an opt-in; its root cause is ADR 0363 Q4 ([[adr-thread-immersed-solid]]).
