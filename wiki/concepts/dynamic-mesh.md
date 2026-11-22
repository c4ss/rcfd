---
type: concept
tags: [mesh, moving-mesh, ale, six-dof]
sources: [doc-dynamic-mesh, doc-cosimulation-protocol, doc-acoustics, adr-thread-layering-remeshing-ale, adr-thread-six-dof-fsi-cosimulation, adr-thread-sliding-mesh-rotor-stator, adr-thread-overset, adr-thread-immersed-solid]
builds_on: [ale-formulation, geometric-conservation-law, mesh-motion-smoothing, rigid-body-dynamics-six-dof]
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
- **Strong coupling / FSI:** ~~`rcfd_fv::fsi::strong_step` (F-251) and the co-simulation participant (F-139) are library only, not in the tree (F-350)~~ ([[doc-dynamic-mesh]]); superseded: the strong step is selectable from a case by `[dynamic_mesh.six_dof.coupling] scheme = "strong"` (F-350, ADR 0266, partial by one mutation; F-437 closes it), and a fluid participant comes from a case `[cosim]` table served by `rcfd serve --cosim` (F-444, ADR 0380, partial) ([[adr-thread-six-dof-fsi-cosimulation]]); see [[cosimulation-protocol]] ([[doc-cosimulation-protocol]]).

> [!warning] Contradiction — strong coupling in the case tree
> `docs/dynamic-mesh.md` ([[doc-dynamic-mesh]]) still says "No strong coupling or added-mass stabilisation in the case tree ... the tree does not use them yet (F-350)". ADR 0266 (F-350, 2026-11-12) records every acceptance clause met for the optional table `[dynamic_mesh.six_dof.coupling]` with `scheme = "strong"`, and `docs/case-schema.md` lists `dynamic_mesh.six_dof.coupling.scheme` = `loose` (default), `strong` ([[adr-thread-six-dof-fsi-cosimulation]], [[doc-case-schema]]). Within the repo docs; the ADR and the schema are newer, so the topic page looks stale. ADR 0256 Q5 had already asked to correct the same sentence.
- **Not covered:** turbulence, energy, scalars, particles, sources, rheology, statistics, restart ([[doc-dynamic-mesh]]).
- **Sponge layers** of the compressible solver can be attached to a moving mesh (F-341, ADR 0265; F-407, ADR 0344) ([[doc-acoustics]]).
- Special cases: [[sliding-mesh]], [[overset-mesh]].

### From the ADR threads (P3)

- **Layering and remeshing:** F-257, F-258, F-247, F-290 done in the library; the tree runs prism and incompressible layering, moving characteristic faces and the opt-in `[dynamic_mesh.compressible] time_step = "mesh_speed"` (ADR 0257); F-259 partial, "closing is the maintainer's decision"; local remeshing in the tree is F-305 (todo) ([[adr-thread-layering-remeshing-ale]]).
- **Mesh-speed step as default:** held by the maintainer on 2026-11-18, "waits until the fan path needs it" (ADR 0257 answers, F-365) ([[adr-thread-layering-remeshing-ale]]).
- **Strong coupling in the tree:** `[dynamic_mesh.six_dof.coupling] scheme = "strong"` (F-350, ADR 0266) and `wall_pressure` (F-358, ADR 0269); force reports `[dynamic_mesh.force_reports] wall_pressure` (F-440, ADR 0342) ([[adr-thread-six-dof-fsi-cosimulation]]). See [[strong-fsi-coupling]].
- **BDF2 on the moving-mesh step:** `PimpleSolver::set_moving_mesh_time_scheme(Bdf2)` and three-level follow conditions (F-345, ADR 0379, partial: A-TR `1.09` against 4); `set_rest_conditions`, `step_sliding` and `replace_mesh` stay Euler-only ([[adr-thread-overset]]).
- **Immersed bodies** as an alternative to moving the mesh: [[immersed-solid]] ([[adr-thread-immersed-solid]]).

## Builds on

- [[ale-formulation]]: balance laws with fluxes relative to the moving faces.
- [[geometric-conservation-law]]: swept volumes that match the cell volume change (checks G1 to G3).
- [[mesh-motion-smoothing]]: Laplacian and inverse-distance or inverse-volume smoothing, layering and remeshing.
- [[rigid-body-dynamics-six-dof]]: the six-DOF body equations.
- [[added-mass]], [[partitioned-fluid-structure-interaction]]: why the loose body coupling has a stability limit and how strong coupling lifts it.

## Related

- [[sliding-mesh]]
- [[overset-mesh]]
- [[piso]]
- [[cosimulation-protocol]]
- [[strong-fsi-coupling]], [[immersed-solid]]
- [[adr-thread-layering-remeshing-ale]], [[adr-thread-six-dof-fsi-cosimulation]]
- [[rcfd-fv]]
- [[rcfd-mesh]]

## Open questions

- Default compressible time step without mesh speed is "ADR 0207 Q2, open" ([[doc-dynamic-mesh]]). The maintainer held the change on 2026-11-18 ([[adr-thread-layering-remeshing-ale]]).
- The adaptive corrector count and SV301 still exclude dynamic meshes (ADR 0223 Q2) ([[doc-dynamic-mesh]]).
- The near-piston velocity lag (3e-4 vs registered 1e-6) has no proven cause ([[doc-dynamic-mesh]]).
