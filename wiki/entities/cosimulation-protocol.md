---
type: entity
tags: [coupling, fsi, protocol, service]
sources: [doc-cosimulation-protocol, doc-dynamic-mesh, doc-structural-solver]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Co-simulation protocol (`rcfd-cosim/1`)

**What it is:** the participant protocol for coupling rcfd's flow with a structural solver: wall displacement one way, fluid force the other, iterated to agreement within each time step (F-139, ADR 0256; transport ADR 0345) ([[doc-cosimulation-protocol]]).
**Where:** `rcfd_fv::cosim` (traits `Participant`, `Structure`, `Fluid`; `coupled_step`, `InterfaceMap`), `rcfd_api::cosim_service` (`RemoteStructure`, `RemoteFluid`), `rcfd_api::cosim_case`; tests `crates/rcfd-fv/tests/cosim_f139.rs`.

## Details

- **Status:** library interface whose calls can also be sent over the JSON-RPC solver service (service protocol 1.1). "There is no case key and no command-line or MCP tool yet" for the coupler; the only mock structure is a sprung wall ([[doc-cosimulation-protocol]]).
- **Participants:** fluid `FlowWall` (PimpleSolver on a moving mesh, wall pressure with the prescribed gradient, ADR 0252); structures `SprungWall` (mock), `StaticSolid` (massless, ADR 0286, "for mock fluids and quasi-static use, not for a flow solver"), `DynamicSolid` (Newmark or generalised-alpha, ADR 0410) ([[doc-cosimulation-protocol]]).
- **Calls:** describe, time, checkpoint, restore, forces, advance; SI units; displacement from the reference configuration; failed `advance` leaves the participant in any state, the coupler restores ([[doc-cosimulation-protocol]]).
- **Mapping:** consistent displacement (rows sum to 1), conservative force (transpose); builders by projection, radial basis (Wendland, support three times the longest edge), bilinear projection; `Unmapped::Refuse` default ([[doc-cosimulation-protocol]]).
- **Iteration:** converged when `|r| <= tolerance x max(|ft|, force_scale)`; fixed, Aitken or least-squares quasi-Newton (ADR 0261) update; `AtCap::Stop` default; `max_iterations = 1` is loose coupling ([[doc-cosimulation-protocol]]).
- **Added mass:** plain iteration diverges above an added-mass to structural-mass ratio of 2; safe start `2M/(2M + m_a)` ([[doc-cosimulation-protocol]]). The case driver's loose six-DOF coupling has the same limit (ADR 0250) ([[doc-dynamic-mesh]]).
- **Service methods:** `cosim.list`, `cosim.open` (lease), `cosim.describe`, `cosim.time`, `cosim.checkpoint`, `cosim.restore`, `cosim.forces`, `cosim.advance`; binary64 values cross bit for bit; a dropped connection reconnects once on `restore` ([[doc-cosimulation-protocol]]).
- **From a case:** `[cosim]` table + `rcfd serve --cosim <case>` hosts a fluid participant (F-444, ADR 0380); the case must be incompressible piso/pimple with nothing else to advance; `rcfd run` refuses it. Served run equals in-process bit for bit over 650 steps ([[doc-cosimulation-protocol]]).
- **Flow-coupled check** (F-401, ADR 0434): elastic bar closing a channel; Aitken and least-squares converge to `1e-6` in 3 iterations on the matching wall; least-squares better on a 3:2 non-matching wall ([[doc-cosimulation-protocol]]). The structural roadmap still lists two-way coupling (F-401) as partial with missed estimates ([[doc-structural-solver]]).
- **Limits:** one coupler per participant, no authentication or encryption, one wall, map built once, no heat transfer ([[doc-cosimulation-protocol]]).
- **Versioning:** incompatible change -> `rcfd-cosim/2`; added methods or reply members do not change the id ([[doc-cosimulation-protocol]]).

## Related

- [[structural-solver]]
- [[dynamic-mesh]]
- [[rcfd-fv]]
- [[rcfd-api]]
- [[rcfd-post]]

## Open questions

- Tree wiring of strong coupling and the participant interface is F-350 ([[doc-dynamic-mesh]]).
