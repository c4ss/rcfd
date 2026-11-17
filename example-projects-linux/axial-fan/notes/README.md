# Axial fan: one blade passage of FAN-01, in 3D

This example project shows a three-dimensional case in the rcfd Studio (ADR 0176). **Use it to look at the 3D geometry
and the flow pattern. It is not a validated fan prediction.** No number here agrees with the measured fan data, and none
is offered as a prediction. The run in this folder uses the first-order `upwind` convection scheme, because the steady
solve does not converge on this mesh with the second-order scheme the fan studies registered (see Limits). The solver
also reports `not_converged` for this run, at its iteration cap of 1500.

The other example projects hold one-cell-thick 2D cases, which the Studio draws face-on. This mesh is not flat, so the
Studio draws it in 3D and the view can be turned.

* Case: `cases/fan01-passage-upwind.toml`, steady, k-omega SST, SIMPLEC, solved in the rotating frame of the blade.
* Fan: one periodic blade passage of the FAN-01 low-pressure axial fan (FAU / EAA, CC BY 4.0 CAD and data): 9 blades, hub
  radius 0.124 m, tip radius 0.2475 m, 2.5 mm tip gap, 1486 rpm, design flow 1.4032 m^3/s.
* Mesh: C-grid from the `axial_fan_sector` generator in the case, the coarsest member F3 of the ADR 0086 family:
  4 920 cells.
* Passage: a 40-degree sector (360 / 9) of the annulus between hub and casing, with the blade in the middle, the inlet
  upstream and the outlet downstream along the fan axis z.
* Patches: `inlet`, `outlet`, `hub`, `shroud` (the casing), `blade`, and the periodic pair `seam_lo` / `seam_hi` on the
  sector's two side faces.
* Fields: the relative velocity (`ux`, `uy`, `uz`, in the frame that turns with the blade), the kinematic pressure `p`
  (pressure / density, m^2/s^2) and the eddy viscosity `nut`.
* Design ledger: `designs/variants.ledger.jsonl` (ADR 0121) holds the run's report values.

## Open it

1. If you want to start runs of your own, copy this folder out of the repository. New runs would otherwise show up as
   untracked files in the repository. Opening and looking write nothing into the folder.
2. Start the desktop Studio as the tutorial `../../docs/tutorials/desktop-assistant.md` says. There is no installer yet:
   build `rcfd`, `rcfd-runner` and `rcfd-mcp` in release, then run `npm start` in `apps/studio-web/` with
   `RCFD_STUDIO_RCFD` set to the `rcfd` you built.
3. Choose Open project folder… (Ctrl+O) and pick this folder.

## What to look at, in order

The Studio names many controls only in tooltips and accessible names; the names below are those.

1. Home lists `cases/fan01-passage-upwind.toml` and its run. Edit on its row opens the case editor on its settings. The
   desktop Studio has no picture of a mesh before a run yet.
2. Runs (Ctrl+2): select the run of the case, then Open in workspace. The field view shows `p` on the surfaces of the
   passage, in 3D. Drag turns the view, right drag pans and the wheel zooms; Reset view goes back. Choose `uz` (the axial
   velocity) or `nut` in Field, and point at the surface to read the value of a cell. Cut plane cuts the passage normal
   to x, y or z.
3. Scrub: the run is steady, so the scrubber runs over iterations (a snapshot every 150). The time panel shows the
   residuals and one report at a time, chosen in Monitor series: `m_blade` and `m_hub` (the moments of one passage about
   the fan axis), `m_in` and `m_out` (mass flows), `p_in` and `p_out` (area averages of `p`). The torque of the whole
   rotor is -9 (`m_blade` + `m_hub`).

## The run in this folder

`scripts/make_example_projects.py` writes this table from the run folder. Wall time is the wall-clock time of the machine
that made the run.

<!-- generated: runs (scripts/make_example_projects.py) -->

| Case | Run | State | Steps | Wall time | Snapshots | Results (final values) |
|---|---|---|---|---|---|---|
| `cases/fan01-passage-upwind.toml` | `runs/r-20261030T223744-ad62` | finished (not_converged) | 1500 | 125.6 s | 10 | m_blade -0.153, m_hub -0.002782, m_in -0.186, m_out 0.186, m_shroud 0.007274, p_in -41.57, p_out 0 |

<!-- end of generated runs -->

## Limits

* **Not validated, first order.** The run here starts from the uniform initial state. At its iteration cap of 1500 the u
  and p residuals are below 1e-5 (4.7e-6 and 7.9e-7), but the solver reports `not_converged` (exit 5). The torque,
  -9 (`m_blade` + `m_hub`), is 1.40 N m, 35 % below the measured shaft torque of 2.142 N m (ADR 0176).
* With the second-order scheme the residuals stall near 1e-3 in a limit cycle and never meet the tolerances (ADR 0110).
* First-order `upwind` is the less accurate solution. When ADR 0110 (S10) switched the stalled run to it, the u and p
  residuals fell below 1e-5 and the fan torque was 1.335 N m, 38 % below the measured shaft torque; the stalled
  second-order run gave 1.850 N m (ADR 0086).
* The case is the bladed run B3 of ADR 0110 (`../../crates/rcfd-api/tests/fan01_stall_w43c.rs`) with three changes:
  `solver.convection = "upwind"` instead of the Barth-Jespersen-limited `linear_upwind`, `solver.max_iterations = 1500`
  instead of 100000, and an `[output]` table that stores fields.
* The measured pressure rise is an installation value (bellmouth, struts, diffuser) that a rotor-only passage cannot
  reproduce. The state of the fan work is in `../../docs/milestone-fan.md`.
* Coarse mesh: 4 920 cells, no mesh study here; the largest face non-orthogonality is about 78 degrees, so `rcfd check`
  warns (SV101).
* The density `physics.rho = 1.2` is assumed (ADR 0075).
* To try the second-order scheme in a copy: set `solver.convection = "linear_upwind"` and add
  `linear_upwind_limiter = "barth_jespersen"` under `[solver]`; the run then stops at `max_iterations` without meeting the
  tolerances.
* The fields are stored in single precision (f32, ADR 0133); the solver runs in double precision.
* The run record holds the absolute paths of the machine that made it (`case_source`, `case_dir` in `run.json`); they are
  information only.
