---
type: source
tags: [adr-thread, fan, fan-01, mrf, rotating, rotor-stator, f-126]
sources: [doc-roadmap-json]
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: stub
---

# ADR thread: FAN-01 rotating solve (F-126, frozen rotor / MRF, rotor-stator interfaces)

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread. Created by the moving-meshes job (P3) as the home of the F-126 / FAN-01 chain; the wave ADRs of that chain (file names without an item, for example 0075, 0078, 0086, 0109, 0110, 0112, 0113, 0115, 0116, 0117, 0174 to 0181) belong to the records job, which adds them here. Until then this page holds only the item-level state and the links to the transient rotor-stator path.

F-126 is the hub item of the `fan` milestone in this category: "Turbomachinery rotating interfaces: frozen-rotor and stage (mixing-plane) interfaces with sector periodicity" ([[doc-roadmap-json]]). Its steady path is the rotating frame ([[mrf]]) on the FAN-01 blade passage ([[fan-01]], [[fan-passage-mesh]]); its transient alternative is the sliding interface ([[adr-thread-sliding-mesh-rotor-stator]]).

## Current state

- **F-126 is partial** (roadmap gap at 2e94e5ef): "The acceptance (a public compressor case reproducing pressure ratio and efficiency within published scatter) is not attempted. Also missing: MRF on the bladed FAN-01 passage (blade wall refused by the revolution check), compressible mixed-out state, rothalpy, a downstream-velocity constraint, and transient and coupled solvers." ([[roadmap-moving-meshes-rotors-and-coupling]]).
- **Transient rotor-stator from a case file exists** for laminar constant-property piso / pimple with Euler: full annulus (F-303, ADR 0637) and sector (F-711, ADR 0646) on the cylindrical-interface generators, which carry no blade ([[adr-thread-sliding-mesh-rotor-stator]]). No ADR of the moving-meshes list runs a sliding interface on the bladed FAN-01 passage or with turbulence.
- **Defaults held for the fan path.** On 2026-11-18 the maintainer held several default changes with the same words, "waits until the fan path needs it": the coupled periodic sliding step (ADR 0641, F-721; ADR 0659, F-724), the bounded second-order immersed forcing (ADR 0254, F-364), the mesh-speed compressible step (ADR 0257, F-365) and the fractional-step coupled-face opt-ins (ADR 0691, F-775).
- Related items in the area that are not started: F-138 (harmonic balance and transient blade row); F-218 (actuator disk, partial) ([[roadmap-moving-meshes-rotors-and-coupling]]).

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0637](../../docs/adr/0637-f303-registration-sliding-interface-in-the-case-tree.md) | 2026-11-16 | registration | F-303 | Full-annulus sliding rotor-stator in the case tree | partial (one control threshold); see [[adr-thread-sliding-mesh-rotor-stator]] | |
| [0646](../../docs/adr/0646-f711-registration-periodic-sector-sliding-in-the-case-tree.md) | 2026-11-16 | registration | F-711 | Sector sliding in the case tree, sector against full annulus | done; see [[adr-thread-sliding-mesh-rotor-stator]] | |

_The FAN-01 wave ADRs (steady rotating-frame solve, mesh, diagnosis, limiters, cyclic seams, sector generators) are to be added by the records job._

## Key numbers and limits

- ADR 0637 D3 / ADR 0112: a sliding step whose interface non-orthogonality exceeds 70 deg stops the run (the cylindrical-interface pre-flight).

## Open questions

- Whether the transient sliding path is to be taken to the bladed FAN-01 passage (and with which turbulence model) is not stated in the ADRs of the moving-meshes list.

## Related

- [[fan-01]], [[mrf]], [[sliding-mesh]], [[fan-passage-mesh]], [[simulation-milestones]], [[computer-fan-target]]
- [[adr-thread-sliding-mesh-rotor-stator]], [[adr-thread-fan01-design-point]] (F-870, the design-point check of the same solve)
- [[roadmap-moving-meshes-rotors-and-coupling]]
