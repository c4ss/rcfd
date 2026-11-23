---
type: entity
tags: [example-project, studio, fan-01, mrf, 3d]
sources: [doc-example-projects]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Example project: axial fan

**What it is:** the 3D Studio example project "Axial fan: one blade passage of FAN-01 (3D)" (ADR 0176). "Use it to look at the 3D geometry and the flow pattern. It is not a validated fan prediction." Its one run uses first-order `upwind` and reports `not_converged` at its cap of 1500 iterations ([[doc-example-projects]]).
**Where:** [example-projects/axial-fan/](../../example-projects/axial-fan/) (start with `notes/README.md`); a Linux-built twin in `example-projects-linux/axial-fan/` ([[doc-example-projects]]).

## Details

- Layout: `rcfd-project.json` (created 2026-10-30T22:35:19Z); one case `cases/fan01-passage-upwind.toml`; `meshes/` empty except a README (mesh from the `axial_fan_sector` generator, C-grid); one run `runs/r-20261030T223519-0489`; `designs/variants.ledger.jsonl` (1 line). 3 499 558 bytes against a cap of 6 000 000 ([[doc-example-projects]]).
- Case: steady, k-omega SST, SIMPLEC, in the rotating frame of the blade. Fan: one periodic passage of [[fan-01]] (FAU / EAA, CC BY 4.0 CAD and data): 9 blades, hub radius 0.124 m, tip radius 0.2475 m, 2.5 mm tip gap, 1486 rpm, design flow 1.4032 m^3/s ([[doc-example-projects]]).
- Mesh: the coarsest member F3 of the ADR 0086 family, 4 920 cells; a 40-degree sector; patches `inlet`, `outlet`, `hub`, `shroud`, `blade`, periodic `seam_lo` / `seam_hi`; largest face non-orthogonality about 78 degrees (`rcfd check` warns SV101). See [[fan-passage-mesh]] ([[doc-example-projects]]).
- Fields: relative velocity `ux`, `uy`, `uz` (rotating frame), kinematic pressure `p` (m^2/s^2), `nut`; a snapshot every 150 iterations. The Studio draws it in 3D: drag turns, right drag pans, the wheel zooms, Cut plane cuts normal to x, y or z ([[doc-example-projects]]).
- Run: 1500 steps, 65.5 s, 10 snapshots; m_blade -0.153, m_hub -0.002782, m_in -0.186, m_out 0.186, m_shroud 0.007274, p_in -41.57, p_out 0. u and p residuals 4.7e-6 and 7.9e-7 at the cap, but exit 5. Whole-rotor torque, -9 x (`m_blade` + `m_hub`), is 1.40 N m, "35 % below the measured shaft torque of 2.142 N m (ADR 0176)" ([[doc-example-projects]]).
- The case is the bladed run B3 of ADR 0110 (`crates/rcfd-api/tests/fan01_stall_w43c.rs`) with three changes: `solver.convection = "upwind"` instead of the Barth-Jespersen-limited `linear_upwind`, `solver.max_iterations = 1500` instead of 100000, and an `[output]` table. With the second-order scheme the residuals stall near 1e-3 in a limit cycle (ADR 0110); ADR 0110's S10 upwind solution gave 1.335 N m (38 % below), the stalled second-order run 1.850 N m (ADR 0086) ([[doc-example-projects]], [[adr-thread-fan01-rotating-solve]]).
- The measured pressure rise is an installation value (bellmouth, struts, diffuser) that a rotor-only passage cannot reproduce; density `physics.rho = 1.2` assumed (ADR 0075) ([[doc-example-projects]]).

## Related

- [[doc-example-projects]], [[example-project-wind-tunnel]], [[example-project-quick-start]]
- [[fan-01]], [[fan-passage-mesh]], [[mrf]], [[adr-thread-fan01-rotating-solve]], [[doc-milestone-fan]]
- [[studio]], [[rcfd-project]], [[run]]
- [[adr-thread-example-projects-and-linux-values]]

## Open questions

_None recorded._
