---
type: source
tags: [coupling, fsi, protocol, reference]
sources: []
origin: docs/cosimulation-protocol.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: stable
---

# Co-simulation with a structural solver: the participant protocol (F-139; ADR 0256)

**Origin:** [docs/cosimulation-protocol.md](../../docs/cosimulation-protocol.md) at `origin/main` 2e94e5ef
**Kind:** protocol specification (389 lines)

## Summary

The participant protocol `rcfd-cosim/1` for exchanging wall displacement and fluid force with a structural solver, step by step, iterated to agreement in each time step. A library interface (`rcfd_fv::cosim`) whose calls can also go over the solver service (`rcfd_api::cosim_service`, ADR 0345). No case key for the coupler, no CLI or MCP tool.

## Key points

- Participants: fluid (`cosim::FlowWall`, a `PimpleSolver` on a moving mesh) and structure (`cosim::SprungWall` mock; `rcfd_post::solid_coupling::StaticSolid`, massless, ADR 0286; `rcfd_post::solid_dynamic::DynamicSolid`, ADR 0410). The coupler `cosim::coupled_step` owns the mapping and the iteration.
- Calls: describe, time, checkpoint, restore, forces (fluid), advance. SI units, global axes; force is the fluid's force per face or node; displacement from the reference configuration.
- Mapping $H$: rows sum to 1 (consistent displacement), force by the transpose (conservative). Builders (F-362, ADR 0263): nearest-element projection with offset correction, Wendland radial basis, bilinear projection (F-504, ADR 0414). `Unmapped::Refuse` is the default.
- Step: checkpoint, predictor, iterate; converged when `|r| <= tolerance x max(|ft|, force_scale)`; fixed, Aitken, or least-squares quasi-Newton update (ADR 0261); `AtCap::Stop` default. `max_iterations = 1` is loose coupling.
- Failure restores both participants bit for bit and returns a located `CouplingError`.
- Service: methods `cosim.list/open/describe/time/checkpoint/restore/forces/advance`, lease numbers, error codes `-32602`, `1002`, `1005`, `1006`; numbers cross bit for bit. `rcfd serve --cosim <case>` hosts a fluid participant from a case with a `[cosim]` table (F-444, ADR 0380); `rcfd run` refuses such a case.
- Relaxation: plain iteration diverges above an added-mass ratio of 2; `fsi::relaxation_from_added_mass(M, m_a) = 2M/(2M + m_a)`. Model problem: Aitken mean 8.8 iterations per step; fixed 1/3 takes 30.
- Connected to the repository's structure (F-401, ADR 0434): elastic bar closing a channel; least-squares update better on a non-matching wall.
- Limits: one coupler per participant, no authentication or encryption, one wall, no heat transfer, map built once.
- Version rule: an incompatible change gets `rcfd-cosim/2`; added methods or members do not.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[cosimulation-protocol]]
- [[structural-solver]]
- [[dynamic-mesh]]
- [[rcfd-fv]]
- [[rcfd-post]]
- [[rcfd-api]]
- [[rcfd-cli]]
