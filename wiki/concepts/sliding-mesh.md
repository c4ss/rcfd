---
type: concept
tags: [mesh, moving-mesh, rotating, rotor-stator]
sources: [doc-dynamic-mesh, doc-component-diagonal-default, doc-readme, doc-roadmap-overview]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Sliding mesh (rotor-stator interface)

A transient rotor-stator method: a rotor block turns against a stator block, and the non-conformal interface between them is rebuilt every step. In rcfd it runs in the PISO/PIMPLE path (`rcfd_fv::sliding`, `PimpleSolver::step_sliding`; F-250, ADR 0216) ([[doc-dynamic-mesh]]).

> [!warning] Contradiction — sliding mesh from a case file
> [[doc-dynamic-mesh]] says the full-annulus sliding interface is "in the case tree since F-303 (ADR 0637)" (`[mesh.sliding]` on `annulus_3d_cylindrical_interface`) and the sector form "in the case tree since F-711 (ADR 0646)" (on `annulus_3d_sector_cylindrical_interface`), both laminar constant-property piso / pimple with Euler only. That agrees with [[doc-readme]]. [[doc-roadmap-overview]] ("What does not work yet") says transient sliding-mesh solves are not available. The repo docs disagree with each other; two of three (including the topic page) say available. Settle against ADRs 0637 and 0646 in P3.

## How rcfd uses it

- **Full annulus from a case** (F-303, ADR 0637): optional table `[mesh.sliding]` (`angular_velocity`, `rotor_walls`) on `mesh.generator = "annulus_3d_cylindrical_interface"`; laminar constant-property piso / pimple, Euler only, no expression boundary values, no restart ([[doc-dynamic-mesh]]).
- **Check:** an m = 2 vortical field with every wall at rest, sliding run against the static join after whole rotor pitches, cell for cell ([[doc-dynamic-mesh]]).
- **Coincident nodes:** when rotor and stator nodes coincide the interface graph falls into parts; since F-710 (ADR 0638) the GCL closure and the old-flux remap are solved jointly, uniform flow kept to `1e-14`; other steps bit-identical. F-724 (ADR 0659) opt-in `FluxScale`; "making `FluxScale` its default is open for the maintainer" ([[doc-dynamic-mesh]]).
- **Periodic sector and planar slider** (F-304, ADR 0629, library): `rcfd_mesh::sliding_periodic`, wrap patches `sliding_wrap_a` / `sliding_wrap_b`; beyond one period (F-704, ADR 0634); coupled coincidences opt-in (F-721, ADR 0641); sector vs full annulus within `2.0e-2 U_in` over 400 steps, mostly the lagged partner value (F-725, ADR 0661); opt-ins `rotational_periodic_implicit` (F-743), `coupled_non_orth` (F-744), `coupled_ddt_correction` (F-754): with both of the first two and one outer iteration within `1.1e-4 U_in` ([[doc-dynamic-mesh]]).
- **Sector from a case** (F-711, ADR 0646): `[mesh.sliding]` on `annulus_3d_sector_cylindrical_interface`; checked against the full-annulus tree run over two pitches; opt-in `[mesh.sliding.coincidences] coupled = true` (F-730, ADR 0654), within `1.25e-2` of the full annulus. "The planar slider has no case key." Sectors of different pitch are refused ([[doc-dynamic-mesh]]).
- **Symmetry planes:** a one-cell-deep annulus with `symmetry` planes under the default averaged diagonal deviates `2.4e-2` of `|U|` after one PISO step; `corrected` keeps it to `8e-15` through `[mesh.sliding]` (F-712, ADR 0650) ([[doc-component-diagonal-default]]). See [[component-diagonal]].

## Related

- [[dynamic-mesh]]
- [[mrf]]
- [[overset-mesh]]
- [[fan-01]]
- [[piso]]

## Open questions

- Which of the opt-ins (F-743, F-744, F-754, `FluxScale`) become defaults is not decided in this source.
- Turbulence on the sliding path: the dynamic-mesh limits exclude turbulence ([[doc-dynamic-mesh]]).
