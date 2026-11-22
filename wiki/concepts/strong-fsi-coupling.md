---
type: concept
tags: [fsi, six-dof, added-mass, coupling, aitken, quasi-newton]
sources: [adr-thread-six-dof-fsi-cosimulation, doc-cosimulation-protocol]
builds_on: [partitioned-fluid-structure-interaction, added-mass, fixed-point-iteration-and-acceleration, rigid-body-dynamics-six-dof]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Strong (implicit) fluid-structure coupling and added mass

Loose coupling exchanges force and motion once per time step; strong coupling iterates the flow step and the body (or structure) within the step until force and motion agree. A light body in a dense fluid has a large added mass (the fluid it must accelerate), and the plain loose iteration then diverges. rcfd derives and checks the threshold: the loose loop is stable up to an added-mass to body-mass ratio of 2 ([[adr-thread-six-dof-fsi-cosimulation]]).

## How rcfd uses it

- **Strong step:** `rcfd_fv::fsi::strong_step` (F-251, ADR 0250), opt-in, with Aitken relaxation and the safe start $2M/(2M + m_a)$; with `max_iterations = 1` it equals the loose coupling bit for bit. Checked on a spring-mounted piston at a ratio of 4 ([[adr-thread-six-dof-fsi-cosimulation]]).
- **From a case:** `[dynamic_mesh.six_dof.coupling] scheme = "strong"` (F-350, ADR 0266) with relaxation, added-mass estimate, tolerance, force scale and iteration cap; Newmark and rest patches are refused with it ([[adr-thread-six-dof-fsi-cosimulation]]).
- **Wall pressure of the body force:** the default extrapolation of `patch_force_moment` was first order on a fixed-flux moving wall (1.25 percent of the added mass on 20 cells, F-351); `wall_pressure = "prescribed_gradient"` corrects it (F-358), default since ADR 0346 per `docs/case-schema.md` ([[adr-thread-six-dof-fsi-cosimulation]]).
- **Co-simulation:** the same iteration drives external structural participants through `rcfd_fv::cosim::coupled_step`, with fixed, Aitken or least-squares quasi-Newton updates (F-361; Aitken 8.8 iterations a step on the model problem) ([[adr-thread-six-dof-fsi-cosimulation]], [[doc-cosimulation-protocol]]). See [[cosimulation-protocol]].

> [!warning] Contradiction — default wall pressure of the body force
> The F-350 gap in `docs/roadmap.json` ([[roadmap-moving-meshes-rotors-and-coupling]]) says "The wall pressure of the body force stays the default extrapolation (F-358)". `docs/case-schema.md` gives `dynamic_mesh.six_dof.coupling.wall_pressure` = `prescribed_gradient` "(default since ADR 0346)" ([[doc-case-schema]]), and ADR 0349 runs its case with "the default wall pressure (prescribed_gradient)" ([[adr-thread-six-dof-fsi-cosimulation]]). Within the repo docs; the roadmap gap looks stale. ADR 0346 itself is in another job's list and was not read here.

## Builds on

- [[partitioned-fluid-structure-interaction]]: loose vs strong coupling, interface conditions, Dirichlet-Neumann exchange.
- [[added-mass]]: why the loose loop diverges for light bodies.
- [[fixed-point-iteration-and-acceleration]]: Aitken relaxation and least-squares quasi-Newton updates.
- [[rigid-body-dynamics-six-dof]]: the body side of the coupling.

## Related

- [[dynamic-mesh]], [[cosimulation-protocol]], [[structural-solver]]
- [[adr-thread-six-dof-fsi-cosimulation]], [[adr-thread-structural-fsi-coupling]]

## Open questions

- F-350 done with F-437 (ADR 0349 Q1, recommended) ([[adr-thread-six-dof-fsi-cosimulation]]).
- The Newmark constant of the safe start (ADR 0250 Q6) ([[adr-thread-six-dof-fsi-cosimulation]]).
