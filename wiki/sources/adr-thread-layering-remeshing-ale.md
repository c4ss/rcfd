---
type: source
tags: [adr-thread, moving-mesh, ale, layering, remeshing, compressible, case-tree]
sources: []
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: layering, local remeshing, compressible ALE and their case-tree wiring

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations with result addenda), 6 ADRs, 2026-11-05 to 2026-11-12.

Topology-changing moving meshes and the compressible ALE path: the tree wiring of compressible ALE and of ADR 0090's piston layering (F-259), inverse-volume smoothing (F-290), compressible ALE boundaries and the mesh-speed time step (F-258), prism and incompressible layering (F-257), local remeshing on tetrahedral meshes (F-247), then the tree wiring of the rest (F-259, ADR 0257). See [[dynamic-mesh]].

## Current state

- **Library done:** F-258 (ADR 0207), F-257 (ADR 0213), F-247 (ADR 0215), F-290 (ADR 0203, the value `inverse_volume` of `dynamic_mesh.smoothing.method` with `smoothing.exponent`).
- **Tree:** a transient compressible run moves its mesh under a prescribed `dynamic_mesh` motion, and `dynamic_mesh.deformation = "layering"` runs the piston stack on `mesh.generator = "box_3d"` (ADR 0191). Since ADR 0257 the tree runs prism layering (`[mesh.box_3d] section = "rectangles"` or `"triangles"`), layering with piso / pimple, characteristic conditions on moving faces and the opt-in `[dynamic_mesh.compressible] time_step = "mesh_speed"`.
- **F-259 stays partial:** "Every registered check of the remainder is met; closing is the maintainer's decision" (roadmap gap). ADR 0191's decision record (2026-11-06): "F-259 stays partial". ADR 0257 Q5 recommends done.
- **Held on 2026-11-18** (ADR 0257 answers): the mesh-speed time step as the default of moving compressible tree runs (ADR 0207 Q2, F-365): "Held. The default change is not approved now: it would move existing results and waits until the fan path needs it."
- **Not in the tree:** local remeshing (F-305, todo; F-247 is library and compressible only); layer thresholds as keys (fixed at 0.4 and 1.6, ADR 0191 Q1 / ADR 0257 Q2).

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0033](../../docs/adr/0033-wave20b-dynamic-mesh.md) | | registration + results (added by p3-records-a) | F-063, F-229, F-244, F-247, F-251, F-257, F-290, F-345 | Dynamic mesh (ALE with a discrete GCL, displacement-diffusion smoothing) and six-DOF bodies, loose coupling | every row met, several after corrections; the piston cell-velocity lag 2.9e-4 misses G5 (iii) and T2; F-230 not attempted | 0090 |
| [0090](../../docs/adr/0090-wave38c-dynamic-mesh-layering.md) | | registration + results (added by p3-records-a) | F-230, F-247, F-257, F-259, F-290 | Layer addition and removal at a piston, compressible ALE, inverse-volume stiffness | every check met (A1 1.53e-5, A2 3.42e-5 against 2 %); F-230 partial by the rule (remeshing, tree wiring missing) | 0191 |
| [0098](../../docs/adr/0098-wave40b-taylor-couette-converged-stretched-third-level-fast-piston.md) | | partials close-out (added by p3-records-a) | F-204, F-208, F-246, F-247 | A Mach 0.2 piston run for F-230 (optional check) | A1 / A2 2.935e-4 / 4.345e-4 against 2e-2; F-230 status unchanged | |
| [0191](../../docs/adr/0191-f259-registration-case-tree-and-cli-wiring-of-layering-and-compressible-ale.md) | 2026-11-05 | registration | F-259 | Tree and CLI wiring of layering and compressible ALE | partial; decision record 2026-11-06 (Q2 answered: F-290; Q3: F-259 stays partial; Q4: `box_3d` in the Studio with F-293) | 0257 |
| [0203](../../docs/adr/0203-f290-registration-inverse-volume-mesh-smoothing-in-the-case-tree.md) | 2026-11-06 | registration | F-290 | Inverse-volume mesh smoothing in the case tree | done | |
| [0207](../../docs/adr/0207-f258-registration-compressible-ale-boundaries-and-mesh-speed-time-step.md) | 2026-11-07 | registration | F-258 | Compressible ALE boundaries and the mesh-speed time step | done in the library; S1 far-field outlet `4.56e-4` (limit `1e-2`); Q2 (mesh-speed default) open | 0257 |
| [0213](../../docs/adr/0213-f257-registration-prism-layering-and-incompressible-layering.md) | 2026-11-07 | registration | F-257 | Prism layering and layering in the incompressible PIMPLE path | done in the library (`PimpleSolver::replace_mesh`, the first topology change in the PISO path) | 0257 |
| [0215](../../docs/adr/0215-f247-registration-local-remeshing-on-unstructured-tetrahedral-meshes.md) | 2026-11-07 | registration | F-247 | Local remeshing on unstructured tetrahedral meshes | done in the library; A1 `3.69e-6` at `t = T` (limit `2e-2`); conservation per event about `1e-15` (limit `1e-13`) | |
| [0257](../../docs/adr/0257-f259-registration-tree-wiring-of-prism-and-incompressible-layering-and-moving-characteristic-faces.md) | 2026-11-12 | registration | F-259 | Tree runs prism and incompressible layering, moving characteristic faces, mesh-speed step | every check met, proposed done; default held 2026-11-18 | |

## Key numbers and limits

- ADR 0215 A1: `V/V0 = 0.400000`, 729 cells, `p_bar = 3.606763` against the isentrope's `3.606750`; A2 at `t = 2T` `9.45e-6` (1860 cells); B1v, B1m, B1p, B1e per event `1.55e-15`, `1.22e-15`, `1.19e-15`, `1.11e-15`.
- ADR 0213 reading: "The cell velocity next to the piston lags the plug flow by 4.7e-3 of `a0 t` on the layered mesh, against the 2.9e-4 of ADR 0033's smoothed mesh"; printed, not checked ("That is a reading, not a diagnosis").
- ADR 0257 Q1: the mesh-speed step as default "changes the bits of every moving compressible tree run (625 to 613 steps on P-X)".
- Layer thresholds 0.4 and 1.6 of `lx / nx`, no key (ADR 0191, ADR 0257).

## Open questions

- ADR 0257 Q5 / roadmap gap: F-259 done is the maintainer's decision.
- ADR 0257 Q2, Q3, Q4: thresholds as keys; one section value for both hexahedral meshes; `box_3d` sections in the Studio (F-293).
- ADR 0213, 0215, 0207 Q-lists stay open for the maintainer (each addendum: "Q1 to Q5 stay open", "Q1 to Q7 stay open").

## Related

- [[dynamic-mesh]], [[mesh-adaptation]]
- [[roadmap-moving-meshes-rotors-and-coupling]]
- [[doc-dynamic-mesh]], [[doc-case-schema-dynamic-mesh]]
- Earlier history (added by p3-records-a): ADR 0033 (wave 20B) built the ALE step "with a discrete geometric conservation law, mesh smoothing by displacement diffusion, and a loose, explicit coupling of the body to the flow", inert unless `dynamic_mesh` enables it; ADR 0090 (wave 38C) added layering at a piston and compressible ALE, and notes that at peak piston Mach 0.05 "the case is nearly quasi-static ... It is not a sensitive test of numerical entropy production"; ADR 0098 repeated the piston at Mach 0.2. See [[adr-thread-early-numerics-transient-accuracy]], [[adr-thread-six-dof-fsi-cosimulation]].
