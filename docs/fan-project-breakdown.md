# Computer-fan project: breakdown into parts

Status: plan (2026-10-26), written at the user's request. This is a plan, not a claim of capability. It extends `docs/milestone-fan.md` (assistant-driven fan design iteration) from "a fan the solver can run" to "a fan model that predicts real-world behaviour and that Claude optimises over time for the computer-fan use case".

This page is for the user and for contributors choosing which part to pick up next. It gives the target, then the limits, every part with its status, the order of work, and last what the user could decide or supply that changes the plan most (the open questions). The state of the large reference fan and the solver work behind it are in the [fan milestone](milestone-fan.md); how to measure a fan for this plan is in the [fan measurement protocol](fan-measurement-protocol.md).

## 1. The target, stated plainly

A parametric fan (a mesh that spins, modified by Claude through the assistant tools) that returns, for each variant and each operating condition:

- air flow and pressure rise (fan curve, and the operating point against a system impedance such as a heat sink),
- shaft power and efficiency,
- noise (tonal and a stated broadband proxy; later psychoacoustic),
- vibration (bearing loads and housing response from imbalance and blade-passing forces),
- the spread of all of the above under manufacturing inaccuracy and under ageing (dust, wear),

and a loop in which Claude proposes the next variant, reads the metrics, and keeps a durable record so that the design improves over weeks, not over one session.

Computer fan = axial or centrifugal, about 40 to 140 mm, 1 000 to 5 000 rpm, tip speed about 5 to 25 m/s (Mach below 0.1), chord Reynolds number about 2e4 to 2e5 (transitional, laminar separation and tip-leakage dominated), working against a high-impedance system (heat sink, filter, grille). This is not FAN-01 (a large low-pressure fan); the physics that matters differs, and the validation source must differ.

## 2. Limits that shape the plan (stated first, not buried)

1. **Prediction needs calibration data.** Nothing in this plan predicts a physical fan better than the data it is checked against. The first task is to find or produce measured data for small fans (open papers, standards-based test data). A small fan tested on a rig by the user would be worth more than any amount of solver work; this is the single most valuable input the user could provide.
2. **Broadband noise** needs scale-resolving transient flow; steady RANS gives a proxy only. Tonal noise (blade-passing harmonics) can be predicted earlier and more cheaply. Every noise number reports its distance from measurement.
3. **Vibration** has two parts: forces on the rotor (imbalance, unsteady blade loads), and the response of the rotor, bearings and housing. The first needs only mass offset and the flow solution; the second needs a rotor-dynamics model, and for blade deformation a structural solver and FSI (F-139, F-242: deferred by decision until the fan workflow exists).
4. **Dust build-up is stochastic and slow.** The model can only be a proxy (thickness and roughness fields, mass added, balance shifted) until deposition data exist. It is reported as a proxy.
5. **The CFD route is not yet reliable**: the FAN-01 stall is open after waves 43 to 45 (limiter, frozen limiter, smoother limiter, seam coefficient and linearisation are each ruled out as the single cause). Parts that do not need a converged CFD fan are built first.

## 3. Parts

Status key: **exists** (done in the roadmap), **partial**, **missing**. Roadmap ids are those in `docs/roadmap.json`; statuses are as of this date and must be re-checked by whoever picks the part up.

### A. Geometry a machine can edit

| Part | What | Status | Verified by |
|---|---|---|---|
| A1 | Parameter schema for a computer fan: hub, blade count, chord, twist, camber, sweep, skew, tip gap, tip shape, frame, struts, inlet bell, motor hub; parameters with units, bounds and defaults | partial (axial blade/hub only: `rcfd_mesh::fan`, ADR 0065) | analytic section and volume checks |
| A2 | Constraint checker: collisions, minimum thickness, tip gap range, printable or mouldable limits; rejects a bad variant with a located reason before any meshing | missing | unit tests with constructed violations |
| A3 | Centrifugal / mixed-flow variant of A1 | missing | analytic checks |
| A4 | Geometry export (STL and the project's formats) so any variant is a file | partial (STL exists, F-119) | round-trip |
| A5 | Perturbation operators: blade-angle error, tip-gap variation, chord/thickness error, blade-to-blade mass offset (imbalance), surface roughness, each with a distribution | missing | statistics of the generated ensemble match the requested distribution |

### B. Mesh of a spinning fan

| Part | What | Status |
|---|---|---|
| B1 | Periodic blade-passage sector mesh with boundary layers and quality gates | partial (F-133, F-126) |
| B2 | Exact-sector generator for verification | done (wave 45B, ADR 0116) |
| B3 | Full-annulus rotor mesh plus a stationary zone with a non-conforming interface | partial (cylindrical interface works on near-isotropic meshes, ADR 0112; sliding AMI F-250 is todo) |
| B4 | Frame, struts, grille and heat-sink (system) mesh around the fan | missing |
| B5 | Automatic mesh quality verdict and sizing from the geometry parameters, with located failures | partial (F-131, F-232) |

### C. Flow solution and performance metrics

| Part | What | Status |
|---|---|---|
| C1 | Steady rotating-frame operating-point solve | partial (MRF exists; FAN-01 stall open) |
| C2 | Transient sliding-mesh solve (needed for broadband noise and unsteady loads) | missing (needs B3, F-250) |
| C3 | Low-Reynolds transition handling suitable for chord Re about 1e5 | partial (SST variants exist; transition model status to be checked) |
| C4 | Metrics extraction: flow rate, total and static pressure rise, torque, shaft power, efficiency, from any solution | partial (F-123) |
| C5 | Fan curve sweep with restart, and the operating point against a system curve | partial (F-095 parametric runner; F-113 lumped fan curves) |
| C6 | Convergence and failure reporting that a loop can act on (located reasons) | done (ADR 0067, 0077) |
| C7 | **Reduced-order tier**: blade-element or Euler-equation performance with loss correlations; gives a fan curve in milliseconds, a bound the CFD must respect, and a way to run the whole design loop before CFD converges | partial (F-218 actuator disk, F-113) |

### D. Noise

| Part | What | Status |
|---|---|---|
| D1 | Tonal estimate at blade-passing harmonics from blade loads (Gutin-type), with stated accuracy | partial (F-243) |
| D2 | Time-domain FW-H on a transient solution | done as post-processing (F-136); no fan transient to feed it |
| D3 | Steady-RANS broadband proxy, reported with its distance from measurement | partial (F-243) |
| D4 | SPL, A-weighting, octave bands, a sound-quality metric (tonality) for the optimiser | partial |
| D5 | Validation of D1 to D4 against measured small-fan noise | missing (needs data) |

### E. Vibration

| Part | What | Status |
|---|---|---|
| E1 | Imbalance force and moment from a mass offset and speed; balance grade (ISO 1940) check | missing |
| E2 | Rotor dynamics: shaft, bearings (stiffness, damping), critical speeds, response to imbalance and to blade-passing loads | missing |
| E3 | Unsteady blade-load harmonics from C1 or C2 as the forcing | missing |
| E4 | One-way load export to a structural model | partial (F-237) |
| E5 | Blade and housing structural response, modal analysis, FSI | missing, deferred by decision (F-139, F-242) |

### F. Real-world degradation

| Part | What | Status |
|---|---|---|
| F1 | Tolerance study: sample A5 perturbations, evaluate with C7 (and CFD at selected points), report spread of flow, power, noise, vibration, and which tolerances matter most | missing (F-249 optimiser is related) |
| F2 | Dust model: deposit thickness and roughness fields from particle deposition, their effect on blade drag and flow, mass added and balance shifted | missing (particle wall interaction F-254 todo; Lagrangian base exists, F-128) |
| F3 | Ageing: bearing wear raising vibration and noise, motor and bearing temperature | missing; later |

### G. Validation data

| Part | What | Status |
|---|---|---|
| G1 | Survey of open data for small fans (curves, noise, geometry) with licences | missing |
| G2 | A reference computer fan with a recorded geometry and test data, or a measured one supplied by the user | missing |
| G3 | The comparison discipline already used: limits registered before the run, misses recorded | exists |

### H. Optimisation and the assistant loop

| Part | What | Status |
|---|---|---|
| H1 | Objective definition: flow at the system operating point over a range of impedances, noise, power, vibration, robustness to tolerances | missing |
| H2 | Optimisers: Nelder-Mead, DOE exist; CMA-ES and multi-objective are todo | partial (F-095, F-249, F-260) |
| H3 | Surrogate model (for example Gaussian process) fitted to past evaluations to choose the next variant cheaply | missing |
| H4 | **Design ledger**: every variant, its parameters, mesh hash, solver version, settings, metrics and verdict, durable and queryable, so progress survives sessions and results can be reproduced | missing |
| H5 | Assistant tools: create variant, run sweep, fetch metrics, compare, propose next, read history | partial (MCP/assistant layer exists; design-loop tools partial) |
| H6 | Long-horizon improvement: Claude reads the ledger, forms hypotheses (for example "more sweep lowers tonal noise at equal flow"), tests them, and records what it learned | missing |
| H7 | Fidelity management: use C7 to screen, CFD to confirm, with the disagreement between them logged and used to correct C7 | missing |

## 4. Order of work

1. Foundations that need no converged CFD fan (wave 46): A1/A2/A5 geometry and tolerance operators, C7 with C4/C5 and D1, E1/E2 rotor dynamics, F1 tolerance study driver, H4 design ledger, G1 data survey, and the F2 dust model's geometry and balance part.
2. Close the CFD route: FAN-01 stall diagnosis or a different reference (B3/C2 with the sliding interface), then C1 on a computer-fan geometry.
3. Noise and vibration from the CFD solution (D2, D3, E3), then H3/H7 to join the tiers.
4. H6: the long-horizon loop runs on the ledger.

## 5. What the user could add that changes the plan most

- Measured data of one real computer fan (fan curve at several impedances, sound level, vibration if available) and its geometry (or the fan itself to scan). Without it the plan can predict trends, not absolute performance.
- Which fan to target first (axial 120 mm, centrifugal blower, other).
- Whether structural/FSI coupling (E5) is to be started once wave 46 completes.

## Where to go next

- [Fan measurement protocol](fan-measurement-protocol.md) for what a measured fan must supply.
- [Validation data](validation-data.md) for the small-fan sources surveyed for this plan and why none is adequate.
- [Studio plan](ui-restart-plan.md) for the desktop app that will show the computer-fan modes.
