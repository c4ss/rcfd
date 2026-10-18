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
| Parametric blade/hub geometry | blade count, hub and tip radius, chord, twist, camber, sweep, tip gap | none |
| Volume mesh of a periodic blade passage | body-fitted sector mesh with boundary layers, quality gates | partial pieces (swept hex, octree, tet, annulus stage), no blade mesher |
| Rotating-frame steady solve | MRF + periodic sector + SST | MRF exists; periodic sector fan case unvalidated |
| Fan validation | published fan or compressor data compared | none (turbo is conservation-checked only) |
| Operating map | flow/pressure sweep with restart, efficiency, characteristic curve | none (F-254 candidate) |
| Noise | time-domain FW-H (Farassat 1A), steady-RANS broadband proxies, tonal (Gutin) check, SPL/OASPL/octave bands | F-243 partial, nothing validated |
| Optimiser | DOE, gradient-free optimiser, robust multi-point and Pareto objective | none (F-095 partial, F-257 candidate) |
| Assistant tools | create variant, run sweep, fetch metrics, compare, propose next | MCP/assistant layer done; design-loop tools none |
| Solver robustness under automation | diverged runs must stop, not loop | missing PIMPLE divergence guard |

## Known limits that shape the plan

- Broadband fan noise needs scale-resolving transient flow; steady RANS can only supply source proxies. The first noise objective is therefore a stated proxy plus tonal prediction, and its distance from measured noise must be reported, not assumed small.
- The CFD solver does not see blade material. Material choice affects mass, stiffness and damping, which needs a structural solver and FSI (product-suites S-01, F-139); this milestone iterates geometry, size and speed only unless the user later adds structural coupling.
- No public fan validation data has been read yet; the first task of the validation stage is to find and read some.

## Wave order

- Wave 32: parametric blade geometry and sector mesh; aeroacoustics library; design-loop infrastructure (operating-map automation, DOE and optimiser, parameter-study tools) with the PIMPLE divergence guard.
- Wave 33 (planned): a steady MRF fan solve on the generated sector mesh, operating curve, comparison with published fan or compressor data that has been read; assistant design-loop tools.
- Wave 34 (planned): noise estimate on the operating curve (proxy plus FW-H on a transient run), robust multi-point objective, a full assistant-driven iteration demo with its measured limitations.

Open decisions for the user: whether to add structural/FSI coupling for the material dimension; which published fan case to target for validation if several are available.
