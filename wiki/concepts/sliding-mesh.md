---
type: concept
tags: [mesh, moving-mesh, rotating, rotor-stator]
sources: [doc-dynamic-mesh, doc-component-diagonal-default, doc-readme, doc-roadmap-overview, adr-thread-sliding-mesh-rotor-stator, adr-thread-coupled-periodic-faces]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Sliding mesh (rotor-stator interface)

A transient rotor-stator method: a rotor block turns against a stator block, and the non-conformal interface between them is rebuilt every step. In rcfd it runs in the PISO/PIMPLE path (`rcfd_fv::sliding`, `PimpleSolver::step_sliding`; F-250, ADR 0216) ([[doc-dynamic-mesh]]).

> [!warning] Contradiction — sliding mesh from a case file
> [[doc-dynamic-mesh]] says the full-annulus sliding interface is "in the case tree since F-303 (ADR 0637)" (`[mesh.sliding]` on `annulus_3d_cylindrical_interface`) and the sector form "in the case tree since F-711 (ADR 0646)" (on `annulus_3d_sector_cylindrical_interface`), both laminar constant-property piso / pimple with Euler only. That agrees with [[doc-readme]]. [[doc-roadmap-overview]] ("What does not work yet") says transient sliding-mesh solves are not available. The repo docs disagree with each other; two of three (including the topic page) say available. Settle against ADRs 0637 and 0646 in P3.

> [!note] Settled by the ADRs (P3, 2026-11-22): available from a case file, with limits
> ADR 0637 (F-303) records "c1 is met: a case file with `[mesh.sliding]` runs from `rcfd run`, the rotor turning 6 pitches and the interface rebuilt each step (36 topology changes), and the binary's numbers equal the in-process run's bits". F-303 is partial only by one negative control's threshold (NA2c `3.2e-2` against `5e-2`); Q6 recommends done. ADR 0646 (F-711, **done**) runs the sector form from `rcfd run` past one pitch. So [[doc-readme]] and [[doc-dynamic-mesh]] are right, and the line of [[doc-roadmap-overview]] ("Sliding-mesh transient solves, needed for rotor-stator interaction and broadband noise, are not available") is stale: its hand-written parts were "last reviewed on 2026-11-18", after both ADRs of 2026-11-16. What the ADRs do not deliver, and what the overview line may have meant: no turbulence, energy, scalars, sources, body force, BDF2, restart, compressible solver or parallel runs; one rotor at constant speed; no blade (the generators are plain cylindrical-interface annuli and sectors); planar sliding library only ([[adr-thread-sliding-mesh-rotor-stator]]). This is a contradiction within the repo docs, resolved in favour of the README.

## How rcfd uses it

- **Full annulus from a case** (F-303, ADR 0637): optional table `[mesh.sliding]` (`angular_velocity`, `rotor_walls`) on `mesh.generator = "annulus_3d_cylindrical_interface"`; laminar constant-property piso / pimple, Euler only, no expression boundary values, no restart ([[doc-dynamic-mesh]]).
- **Check:** an m = 2 vortical field with every wall at rest, sliding run against the static join after whole rotor pitches, cell for cell ([[doc-dynamic-mesh]]).
- **Coincident nodes:** when rotor and stator nodes coincide the interface graph falls into parts; since F-710 (ADR 0638) the GCL closure and the old-flux remap are solved jointly, uniform flow kept to `1e-14`; other steps bit-identical. F-724 (ADR 0659) opt-in `FluxScale`; "making `FluxScale` its default is open for the maintainer" ([[doc-dynamic-mesh]]).
- **Periodic sector and planar slider** (F-304, ADR 0629, library): `rcfd_mesh::sliding_periodic`, wrap patches `sliding_wrap_a` / `sliding_wrap_b`; beyond one period (F-704, ADR 0634); coupled coincidences opt-in (F-721, ADR 0641); sector vs full annulus within `2.0e-2 U_in` over 400 steps, mostly the lagged partner value (F-725, ADR 0661); opt-ins `rotational_periodic_implicit` (F-743), `coupled_non_orth` (F-744), `coupled_ddt_correction` (F-754): with both of the first two and one outer iteration within `1.1e-4 U_in` ([[doc-dynamic-mesh]]).
- **Sector from a case** (F-711, ADR 0646): `[mesh.sliding]` on `annulus_3d_sector_cylindrical_interface`; checked against the full-annulus tree run over two pitches; opt-in `[mesh.sliding.coincidences] coupled = true` (F-730, ADR 0654), within `1.25e-2` of the full annulus. "The planar slider has no case key." Sectors of different pitch are refused ([[doc-dynamic-mesh]]).
- **Symmetry planes:** a one-cell-deep annulus with `symmetry` planes under the default averaged diagonal deviates `2.4e-2` of `|U|` after one PISO step; `corrected` keeps it to `8e-15` through `[mesh.sliding]` (F-712, ADR 0650) ([[doc-component-diagonal-default]]). See [[component-diagonal]].

### From the ADR threads (P3)

- **Status by item:** F-250 partial (library; interface mass `1.51e-10` against `1e-10` with two non-orthogonal correctors, met with six), F-303 partial (one control threshold), F-304 done, F-704 partial in the roadmap, F-710, F-720, F-711, F-730 done, F-712, F-721, F-724, F-725 partial; F-729 (planar in the tree) todo ([[adr-thread-sliding-mesh-rotor-stator]]).
- **Run-time guard:** a step whose interface non-orthogonality exceeds 70 deg stops the run with a located message (`mesh.sliding`); monitors `sliding_angle`, `sliding_interface_flux`, `sliding_gcl`, `sliding_non_orthogonality` (ADR 0637 D3) ([[adr-thread-sliding-mesh-rotor-stator]]).
- **Sector against full annulus:** within `1.15e-2 U0` over 2.1 pitches (ADR 0646); with a non-axisymmetric inlet `1.981e-2 U_in` (ADR 0661), mostly the lagged partner value on the coupled faces; the opt-ins of [[adr-thread-coupled-periodic-faces]] bring it to `1.104e-4` with one outer iteration ([[adr-thread-sliding-mesh-rotor-stator]], [[adr-thread-coupled-periodic-faces]]).
- **Defaults held:** the maintainer held the coupled periodic step and `FluxScale` as defaults on 2026-11-18, "waits until the fan path needs it" (ADR 0641 answers) ([[adr-thread-sliding-mesh-rotor-stator]]).
- **Force on a rotor wall:** no caller evaluates a force on the sliding step; its default evaluation "misses `0.41 g dn` on a rotor wall" (ADR 0269 Q4) ([[adr-thread-six-dof-fsi-cosimulation]]).

## Related

- [[adr-thread-sliding-mesh-rotor-stator]], [[adr-thread-coupled-periodic-faces]], [[adr-thread-fan01-rotating-solve]]
- [[dynamic-mesh]]
- [[mrf]]
- [[overset-mesh]]
- [[fan-01]]
- [[piso]]

## Open questions

- Which of the opt-ins (F-743, F-744, F-754, `FluxScale`) become defaults is not decided in this source.
- Turbulence on the sliding path: the dynamic-mesh limits exclude turbulence ([[doc-dynamic-mesh]]).
- The defaults of F-721 / F-724 were held on 2026-11-18; F-746 and F-775 (coupled-face defaults) are todo or held ([[adr-thread-sliding-mesh-rotor-stator]], [[adr-thread-coupled-periodic-faces]]).
- ADR 0637 Q6 (F-303 done) and ADR 0634 Q5 (F-704 done) are the lead's to confirm ([[adr-thread-sliding-mesh-rotor-stator]]).
