---
type: concept
tags: [mesh, moving-mesh, ale, six-dof]
sources: [doc-dynamic-mesh, doc-cosimulation-protocol, doc-acoustics]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Dynamic (moving) mesh and ALE

A mesh whose points move during a run, with the flow solved in the arbitrary Lagrangian-Eulerian (ALE) form: fluxes relative to the moving faces, and cell volume changes matched by the swept volumes (geometric conservation law). In rcfd status: partial (F-063, F-229; ADR 0033) ([[doc-dynamic-mesh]]).

## How rcfd uses it

- **Solvers:** incompressible laminar PISO/PIMPLE with implicit Euler on moving points; the explicit compressible solver with prescribed motion or a layered piston (`CompressibleSolver::step_ale`) ([[doc-dynamic-mesh]]).
- **Case section** `dynamic_mesh`; static errors located at the key, incompatible sections at `dynamic_mesh.enabled` ([[doc-dynamic-mesh]]).
- **Motion:** prescribed displacement `d(t)` and rotation `angle(t)` on moving patches; interior `rigid` or `smoothing` (Laplacian, inverse distance, inverse volume `1/V^k`, F-290, ADR 0203). Every moved mesh is validated ([[doc-dynamic-mesh]]).
- **GCL:** face swept volume / `dt` is the mesh flux; cell volume change equals the sum of swept volumes to round-off (checks G1 to G3) ([[doc-dynamic-mesh]]).
- **Walls:** following walls take the mesh velocity, pressure condition `dp/dn = -n.a_wall`; `dynamic_mesh.rest_patches` (ADR 0183 P-B) for walls at rest on a rigidly moving mesh (e.g. faceted hub and casing) ([[doc-dynamic-mesh]]).
- **Time schemes:** Euler only in the case tree; BDF2 opt-in in the library (F-345, ADR 0379; uniform flow to `7e-15`) ([[doc-dynamic-mesh]]).
- **Topology changes:** layering on a `box_3d` piston stack (F-230, ADR 0090; tree F-259, ADR 0191/0257; prisms F-257, ADR 0213; thresholds fixed at 0.4 / 1.6 in the tree); local tet remeshing (F-247, ADR 0215, library, compressible only) ([[doc-dynamic-mesh]]).
- **Six-DOF body:** mass, inertia, locks, springs, dampers, drag; RK4 or Newmark; quaternion orientation; loose explicit coupling, unstable above an added-mass ratio of 2 (ADR 0250). F-244: falling block within 0.55 percent of the lubrication terminal velocity; spring piston within 0.13 percent ([[doc-dynamic-mesh]]).
- **Strong coupling / FSI:** `rcfd_fv::fsi::strong_step` (F-251) and the co-simulation participant (F-139) are library only, not in the tree (F-350) ([[doc-dynamic-mesh]]); see [[cosimulation-protocol]] ([[doc-cosimulation-protocol]]).
- **Not covered:** turbulence, energy, scalars, particles, sources, rheology, statistics, restart ([[doc-dynamic-mesh]]).
- **Sponge layers** of the compressible solver can be attached to a moving mesh (F-341, ADR 0265; F-407, ADR 0344) ([[doc-acoustics]]).
- Special cases: [[sliding-mesh]], [[overset-mesh]].

## Related

- [[sliding-mesh]]
- [[overset-mesh]]
- [[piso]]
- [[cosimulation-protocol]]
- [[rcfd-fv]]
- [[rcfd-mesh]]

## Open questions

- Default compressible time step without mesh speed is "ADR 0207 Q2, open" ([[doc-dynamic-mesh]]).
- The adaptive corrector count and SV301 still exclude dynamic meshes (ADR 0223 Q2) ([[doc-dynamic-mesh]]).
- The near-piston velocity lag (3e-4 vs registered 1e-6) has no proven cause ([[doc-dynamic-mesh]]).
