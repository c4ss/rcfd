---
type: source
tags: [mesh, moving-mesh, sliding, overset, six-dof, how-and-why]
sources: []
origin: docs/dynamic-mesh.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: stable
---

# Dynamic mesh and rigid-body dynamics (F-063, F-229; ADR 0033)

**Origin:** [docs/dynamic-mesh.md](../../docs/dynamic-mesh.md) at `origin/main` 2e94e5ef
**Kind:** how-and-why page (464 lines)

## Summary

How rcfd moves the mesh during a run, integrates a flow-driven rigid body, and where the method stops. Status: partial. Incompressible laminar PISO/PIMPLE with implicit Euler on a mesh whose points move every step, and the explicit compressible solver with prescribed motion or a layered piston. ADRs 0033, 0090, 0191, 0213, 0215 and many later ones.

## Key points

- **Case tree limits:** Euler only (PISO/PIMPLE); library moving-mesh step has BDF2 opt-in since F-345 (ADR 0379); `step_sliding` and `replace_mesh` stay Euler-only. No turbulence, energy, scalars, particles, sources, rheology, statistics or restart.
- **Sliding interfaces, full annulus** (F-250, ADR 0216): rotor block turning against a stator across a cylinder, interface rebuilt every step (`rcfd_fv::sliding`, `PimpleSolver::step_sliding`). **"In the case tree since F-303 (ADR 0637)"**: `[mesh.sliding]` (`angular_velocity`, `rotor_walls`) on `mesh.generator = "annulus_3d_cylindrical_interface"`; laminar constant-property piso / pimple, Euler only, no expression boundary values, no restart. Coincident-node steps: F-710 (ADR
  0638) couples parts so uniform flow stays uniform to `1e-14`; F-724 (ADR 0659) `FluxScale` opt-in (making it default "is open for the maintainer").
- **Planar and sector sliding with periodic seams** (F-304, ADR 0629, library): `sliding_wrap_a` / `sliding_wrap_b` patches; beyond one period F-704 (ADR 0634); coincidences F-720 (ADR 0640), coupled opt-in F-721 (ADR 0641); sector vs full annulus within `2.0e-2 U_in` over 400 steps (F-725, ADR 0661); opt-ins `rotational_periodic_implicit` (F-743, ADR 0664), `coupled_non_orth` (F-744, ADR 0667), pressure part on translational (F-752, ADR 0672) and rotational pairs (F-753, ADR 0674), `coupled_ddt_correction` (F-754, ADR 0675), fractional step (F-757, ADR 0687), `fractional_body_force` (F-771, ADR 0691). **Sector in the case tree since F-711 (ADR 0646)**: `[mesh.sliding]` on `"annulus_3d_sector_cylindrical_interface"`; opt-in `[mesh.sliding.coincidences] coupled = true` (F-730, ADR 0654). "The planar slider has no case key."
- **Local remeshing** of tet meshes (F-247, ADR 0215): library only, compressible, axis-aligned planar walls.
- **Overset (chimera)** (F-134, ADR 0219): library only, one rigidly moving component over a static background; many opt-ins (outer iterations F-335, BDF2 F-338, prescribed fringe flux F-445, ring gradient F-479, continuous donors F-468, geometry corrections F-368/F-472/F-487). Cylinder Re 40: drag within 0.62 percent of body-fitted (limit 5 percent); Taylor-Green order 1.62 (limit 1.7, missed).
- **Layering** on a `box_3d` stack with piston `xmax` for compressible and PISO/PIMPLE (ADR 0090, 0191, 0213, 0257); thresholds fixed at 0.4 and 1.6 in the tree.
- **Coupling:** loose explicit six-DOF coupling unstable above added-mass ratio 2 (ADR 0250); strong coupling `rcfd_fv::fsi::strong_step` (F-251) and `rcfd_fv::cosim` (F-139, ADR 0256) in the library, not the tree (F-350).
- **PISO time-step limit** (F-244/F-299, ADR 0211/0214; F-530, ADR 0451): unstable above $\nu\, dt/dx^2$ of about 3 to 4 across the finest cells in narrow gaps; consistent correctors (`[solver.consistent_correctors] enabled = true`, F-301, ADR 0223) remove it; at large diffusion numbers also use outer iterations. Opt-ins: `outer_pressure_tolerance` (F-536, ADR 0458), `outer_scale_window` (F-550, ADR 0617), `lag_onset_refusal` (F-535, ADR 0471); no case keys.
- Known misses: near-piston velocity lag 3e-4 vs 1e-6 (cause not proven); `Symmetry` + averaged diagonal gives a spurious tangential force.
- **Method:** prescribed displacement / rotation, `rigid` or `smoothing` interior (Laplacian, inverse distance, inverse volume), GCL by swept volumes, ALE relative flux, `rest_patches` (ADR 0183 P-B), six-DOF RK4/Newmark with quaternions. F-244 falling block within 0.55 percent of lubrication terminal velocity; spring piston frequency within 0.13 percent.
- Force reports on moving walls use the fixed-flux gradient wall pressure by default since ADR 0346.

## Contradictions with the wiki

- **Sliding mesh from a case file.** This page says the full-annulus sliding interface is "in the case tree since F-303 (ADR 0637)" and the sector "in the case tree since F-711 (ADR 0646)", laminar constant-property piso/pimple, Euler only. This agrees with [[doc-readme]] and disagrees with [[doc-roadmap-overview]] ("not available"). See [[sliding-mesh]].
- Inside this page: the F-304 bullet still says of the sector "no case key (the full-annulus key of F-303 does not cover them; ADR 0637 Q3)", while the same bullet later says the sector is in the case tree since F-711 (ADR 0646). The older sentence was not updated. The sentence "data set per face on rotor patches are the caller's to set on the new mesh; sectors of different pitch are refused" also appears twice.

## Pages this source informs

- [[dynamic-mesh]]
- [[sliding-mesh]]
- [[overset-mesh]]
- [[piso]]
- [[cosimulation-protocol]]
- [[rcfd-fv]]
- [[rcfd-mesh]]
- [[component-diagonal]]
- [[known-miss]]
