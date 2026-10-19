# Milestone: assistant-driven fan design iteration

Status: planned (set by the user on 2026-10-18). This document is the plan, not a claim of capability.

## Goal

Claude, through the assistant tool layer, iterates on fan shape and size, sets up and runs the test cases, reads the results, and converges on designs that keep noise low while flow is high across a range of operating conditions (not at a single point).

## Staged definition

1. **Foundation** (use case 1): a user describes a case to the assistant, gets a validated laminar or RANS run and sees results in the GUI.
2. **Design loop on a simple part**: parametric geometry, automatic meshing, run, compare variants, with the assistant choosing the next variant (duct or heat-sink pressure-drop minimisation).
3. **Fan**: parametric axial fan, steady MRF/sector solve per operating point, fan characteristic curve, noise estimate, robust multi-point objective.

## Capability chain and current state (2026-10-18)

| Link | Needed | State |
|---|---|---|
| Parametric blade/hub geometry | blade count, hub and tip radius, chord, twist, camber, sweep, tip gap | wave 32A (ADR 0065): library `rcfd_mesh::fan`, NACA 4-digit and circular-arc sections, checked against analytic formulas only |
| Volume mesh of a periodic blade passage | body-fitted sector mesh with boundary layers, quality gates | wave 32A: hex sector with blunt blade edges and exact boundary-layer stacks, accepted by the periodic solver machinery; the registered quality gates are MISSED (docs/meshing.md). Wave 35D (ADR 0079): the FAN-01 passage builds on the C-grid (`c_grid_construction = "camber_seam"`), passes FvGeometry, check_mesh and the rules, misses the quality target (83 deg) and its laminar smoke residual grows; the construction is not robust at 70 deg tip stagger |
| Rotating-frame steady solve | MRF + periodic sector + SST | MRF exists; rotating-frame checks second order with a registered mesh study (ADR 0075 part C); SST accepts the stationary casing as a moving wall since wave 34D (ADR 0075); a diverging steady run is stopped by the steady divergence guard (ADR 0077); `linear_upwind` and `tvd_van_leer` diverge on the FAN-01 sector mesh, `upwind` and `tvd_minmod` do not (ADR 0077 part B) |
| Fan validation | published fan or compressor data compared | FAN-01 (FAU/EAA low-pressure fan, CC BY 4.0 CAD and characteristic) read and compared once (ADR 0075): rotor-only sector MISSES (torque -38 % at the design point, total-to-static rise negative against 126 Pa measured); F-126 not met |
| Operating map | flow/pressure sweep with restart, efficiency, characteristic curve | none (F-254 candidate) |
| Noise | time-domain FW-H (Farassat 1A), steady-RANS broadband proxies, tonal (Gutin) check, SPL/OASPL/octave bands | F-243 partial, nothing validated |
| Optimiser | DOE, gradient-free optimiser, robust multi-point and Pareto objective | none (F-095 partial, F-257 candidate) |
| Assistant tools | create variant, run sweep, fetch metrics, compare, propose next | MCP/assistant layer done; design-loop tools none |
| Solver robustness under automation | diverged runs must stop, not loop | PIMPLE/PISO guard (ADR 0067) and steady guard (ADR 0077) end diverged runs in the run-control driver and the design loop with a located, machine-readable reason (FAN-01 `linear_upwind`: stopped at iteration 7 to 8 instead of running for hours); one-corrector PIMPLE lag warning on by default (ADR 0077 part C) |

## Known limits that shape the plan

- Broadband fan noise needs scale-resolving transient flow; steady RANS can only supply source proxies. The first noise objective is therefore a stated proxy plus tonal prediction, and its distance from measured noise must be reported, not assumed small.
- The CFD solver does not see blade material. Material choice affects mass, stiffness and damping, which needs a structural solver and FSI (product-suites S-01, F-139); this milestone iterates geometry, size and speed only unless the user later adds structural coupling.
- Public fan data has been read (wave 34D, docs/validation-data.md "Rotating machinery sources"): FAN-01 is the only open low-speed case found whose blade the generator can represent. Its measured pressure rise is an installation value (bellmouth, finite hub, struts, diffuser); the first rotor-only comparison missed by a wide margin and the cause is not yet diagnosed (ADR 0075).

## Wave order

- Wave 32: parametric blade geometry and sector mesh; aeroacoustics library; design-loop infrastructure (operating-map automation, DOE and optimiser, parameter-study tools) with the PIMPLE divergence guard.
- Wave 33 (planned): a steady MRF fan solve on the generated sector mesh, operating curve, comparison with published fan or compressor data that has been read; assistant design-loop tools.
- Wave 34 (planned): noise estimate on the operating curve (proxy plus FW-H on a transient run), robust multi-point objective, a full assistant-driven iteration demo with its measured limitations.

Open decisions for the user: whether to add structural/FSI coupling for the material dimension; which published fan case to target for validation if several are available.

## Deferred by decision (2026-10-18)

Structural coupling (the material dimension: stiffness, mass, damping, blade deformation, FSI) is to be added to `docs/roadmap.json` only after the full assistant-driven fan workflow (waves 32 to 34) exists. Until then it stays in the backlog (`docs/product-suites.md` S-01 and the FSI row of `docs/parity-matrix.md`) and the open decision above is answered: geometry, size and speed only.
