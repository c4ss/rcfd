# Axial fan: one blade passage of FAN-01, in 3D

This example project shows a three-dimensional case in the rcfd Studio (ADR 0176). The other example projects hold
one-cell-thick 2D cases, which the Studio draws face-on; this mesh is not flat, so the Studio draws it in 3D and the view
can be rotated.

**What it is for: looking at the 3D geometry and the flow pattern. It is not a validated fan prediction.** The run in
this folder uses the first-order `upwind` convection scheme, because the steady solve does not converge on this mesh
with the second-order scheme the fan studies registered (see Limits). No number here agrees with the measured fan data,
and none is offered as a prediction.

| Case | What it is | Mesh |
|---|---|---|
| `cases/fan01-passage-upwind.toml` | one periodic blade passage of the FAN-01 low-pressure axial fan (FAU / EAA, CC BY 4.0 CAD and data): 9 blades, hub radius 0.124 m, tip radius 0.2475 m, 2.5 mm tip gap, 1486 rpm, design flow 1.4032 m^3/s; steady, k-omega SST, SIMPLEC, solved in the rotating frame of the blade | C-grid from the `axial_fan_sector` generator in the case, the coarsest member F3 of the ADR 0086 family: 4 920 cells |

The passage is a 40-degree sector (360 / 9) of the annulus between hub and casing, with the blade in the middle, the
inlet upstream and the outlet downstream along the fan axis z. Its patches: `inlet`, `outlet`, `hub`, `shroud` (the
casing), `blade`, and the periodic pair `seam_lo` / `seam_hi` on the sector's two side faces. The fields are the relative
velocity (`ux`, `uy`, `uz`, in the frame that turns with the blade), the kinematic pressure `p` (pressure / density, m^2/s^2) and the eddy viscosity `nut`.

The design ledger `designs/variants.ledger.jsonl` (ADR 0121) holds the run's report values.

## Open it

1. Build: `cargo build --release -p rcfd-studio -p rcfd-runner -p rcfd-cli`.
2. Copy this folder out of the repository if you want to start runs of your own (new runs would otherwise show up as
   untracked files in the repository).
3. Start `target\release\rcfd-studio.exe <this folder>` (on Linux: `sh scripts/studio.sh example-projects-linux/axial-fan`),
   or Project > Open project... in the Studio.

## What to click, in order

1. Project panel: select `cases/fan01-passage-upwind.toml`. The Case editor shows its settings and the Viewport shows
   the mesh preview, labelled "3D". Middle drag rotates the view, right drag pans, the wheel zooms, a double click fits
   the passage into the view. "patch colours" tells the blade, hub, casing, inlet and outlet apart.
2. Runs panel: select the run of the case. The Viewport shows `p` on the surfaces of the passage, in 3D as the
   preview. Choose `uz` (the axial velocity) or `nut` in the "field" box and hover for values.
3. Scrub: the run is steady, so the slider runs over iterations (a snapshot every 150). The Plots panel shows the
   residuals and the reports: `m_blade`, `m_hub` (the moments of one passage about the fan axis; the torque of the whole
   rotor is -9 (`m_blade` + `m_hub`)), `m_in`, `m_out` (mass flows) and `p_in`, `p_out` (area averages of `p`).

## The run in this folder

Written by `scripts/make_example_projects.py` from the run folder; wall-clock time of the machine that made it.

<!-- generated: runs (scripts/make_example_projects.py) -->

| Case | Run | State | Steps | Wall time | Snapshots | Results (final values) |
|---|---|---|---|---|---|---|
| `cases/fan01-passage-upwind.toml` | `runs/r-20261030T223519-0489` | finished (not_converged) | 1500 | 65.5 s | 10 | m_blade -0.153, m_hub -0.002782, m_in -0.186, m_out 0.186, m_shroud 0.007274, p_in -41.57, p_out 0 |

<!-- end of generated runs -->

## Limits

* **Not validated, first order.** The case is the bladed run B3 of ADR 0110 (`../../crates/rcfd-api/tests/fan01_stall_w43c.rs`)
  with three changes: `solver.convection = "upwind"` instead of the Barth-Jespersen-limited `linear_upwind`,
  `solver.max_iterations = 1500` instead of 100000, and an `[output]` table that stores fields. With the second-order
  scheme the residuals stall near 1e-3 in a limit cycle and never meet the tolerances (ADR 0110). First-order `upwind`
  is the less accurate solution: when ADR 0110 (S10) switched the stalled run to it, the u and p residuals fell below
  1e-5 and the fan torque was 1.335 N m, 38 % below the measured shaft torque of 2.142 N m; the stalled second-order run
  gave 1.850 N m (ADR 0086). The run here starts from the uniform initial state: at its iteration cap of 1500 the u
  and p residuals are below 1e-5 (4.7e-6 and 7.9e-7) but the solver reports `not_converged` (exit 5), and the torque
  -9 (`m_blade` + `m_hub`) is 1.40 N m, 35 % below the measured value (ADR 0176). The measured pressure rise is an installation value (bellmouth, struts, diffuser) that a rotor-only passage
  cannot reproduce. The state of the fan work is in `../../docs/milestone-fan.md`.
* Coarse mesh: 4 920 cells, no mesh study here; the largest face non-orthogonality is about 78 degrees, so `rcfd check`
  warns (SV101).
* The density `physics.rho = 1.2` is assumed (ADR 0075).
* To try the second-order scheme in a copy: set `solver.convection = "linear_upwind"` and add
  `linear_upwind_limiter = "barth_jespersen"` under `[solver]`; the run then stops at `max_iterations` without meeting the
  tolerances.
* The fields are stored in single precision (f32, ADR 0133); the solver runs in double precision.
* The run record holds the absolute paths of the machine that made it (`case_source`, `case_dir` in `run.json`); they are
  information only.
