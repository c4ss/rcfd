---
type: entity
tags: [example-project, studio, cylinder, naca0012, backward-facing-step]
sources: [doc-example-projects]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Example project: wind tunnel

**What it is:** the finished Studio example project "Wind tunnel: cylinder, NACA 0012 aerofoil and backward-facing step": three 2D flows, each a case, a variant and completed runs of both, made by the real solver through the run service (ADR 0142). "Every mesh is coarse on purpose and no result here is a validation" ([[doc-example-projects]]).
**Where:** [example-projects/wind-tunnel/](../../example-projects/wind-tunnel/) (start with `notes/README.md`); a Linux-built twin in `example-projects-linux/wind-tunnel/` ([[doc-example-projects]]).

## Details

- Layout: `rcfd-project.json` (created 2026-10-28T15:21:28Z, `rcfd 0.1.0`); 8 cases in `cases/`; one imported mesh `meshes/naca0012-ogrid/` (OpenFOAM polyMesh: points, faces, owner, neighbour, boundary) with `meshes/naca0012-ogrid.recipe.toml`; 8 runs in `runs/`; `designs/variants.ledger.jsonl` (8 lines, ADR 0121); notes `README.md`, `cylinder.md`, `naca0012.md`, `step.md`. 28 096 175 bytes against a cap of 35 000 000 ([[doc-example-projects]]).
- **Cylinder** (`cylinder-re100.toml`, variant `cylinder-re200.toml` with nu halved): D = 1 m, U = 1 m/s, 2D laminar; `cylinder_ogrid` generator, 64 around x 48 radial, far field 15 D (3072 cells); PISO, second-order upwind, dt 0.0625 D/U to 80 D/U (1280 steps), a sideways push so shedding starts within about 40 D/U; 80 snapshots of `ux`, `uy`, `p`, `vorticity_z`. Results: Re 100 cd 1.327, St 0.1517, cd_mean 1.323, cl_amplitude 0.3091, 5 periods, 18.4 s; Re 200 St 0.1751, cd_mean 1.291, 6 periods. The repository's validation runs (ADR 0062, 0069; 128 x 100, far field 20 D, 160 D/U) measured St 0.159 to 0.165 and mean cd 1.31 to 1.34 at Re 100; the example uses about a quarter of those cells and half the time, so "the Strouhal number is known to a few percent at best" ([[doc-example-projects]]). See [[cylinder-re100]].
- **NACA 0012** (`naca0012-aoa4.toml`, variant `naca0012-aoa2.toml`, plus `-rotated` twins): Re 6e6, k-omega SST, SIMPLEC, second-order upwind, at most 2000 iterations; mesh: the cylinder O-grid (128 around, 48 radial) moved onto the aerofoil with a 20-chord far field, as ADR 0092's set-up does (6144 cells). Results: 4 degrees cl 0.3469, cd 0.0282 (409 iterations); 2 degrees cl 0.1982, cd 0.02196 (230). Wall y+ about a thousand, maximum non-orthogonality 80 degrees (rule SV101): "Expect the lift well below the measured slope ... and the drag at about three times the measured minimum drag (0.0080)". At 8 degrees the set-up did not converge. Since ADR 0169 (2026-10-29) the far-field turbulence role defaults to `freestream` and the runs meet the 1e-6 targets; runs stored earlier stopped at the cap "not_converged" ([[doc-example-projects]]). See [[naca0012]].
- **Rotated flow vs rotated mesh** (ADR 0155): the `-rotated` cases set the angle by `[mesh.transform] rotate_z_deg = -4.0`, `rotate_origin = [0.25, 0.0, 0.0]`; cl 0.3469251470 against 0.3469251479 at 4 degrees (relative 2.6e-9), 5.2e-9 at 2 degrees; `rcfd check` warns MT101 about geometry-relative settings that do not turn with the mesh ([[doc-example-projects]]).
- **Backward-facing step** (`step-re800.toml`, Gartling's condition; variant `step-re400.toml`, nu doubled): expansion ratio 2, domain starting at the step, parabolic inflow $24\,(y - 0.5)(1 - y)$, channel 15 H (Gartling 30 H); `box_2d` 200 x 40 (8000 cells); SIMPLEC to 1e-6. Re 800: x_reattach 5.888, upper bubble 4.652 to 10.42 (H); published 6.10 H and 4.85 H to 10.48 H. Re 400: x_reattach 4.248. Not the turbulent Driver-Seegmiller step of `cases/backward_step/case.json` ([[doc-example-projects]]). See [[backward-facing-step]].
- Walkthrough in the notes: Home (Ctrl+1), Runs (Ctrl+2), Open in workspace, Field, the scrubber (iterations axis; simulated time on the flag for transient runs), Monitor series, Compare runs side by side, editing and running in a copy ([[doc-example-projects]]).

## Related

- [[doc-example-projects]], [[example-project-quick-start]], [[example-project-axial-fan]]
- [[studio]], [[rcfd-project]], [[run]]
- [[adr-thread-example-projects-and-linux-values]], [[design-ledger]]
- [[cylinder-re100]], [[naca0012]], [[backward-facing-step]], [[k-omega-sst]], [[piso]]

## Open questions

- The notes contradict each other on whether the case editor finds the aerofoil mesh by its relative path (see [[doc-example-projects]]).
- [ADR 0166](../../docs/adr/0166-job49c-effective-wall-shear-and-sa-far-field-defaults.md) notes that it "does not regenerate the 8 stored NACA 0012 example runs, although a re-run gives other `cl` and `cd`"; the notes say the runs were redone after ADR 0169 (2026-10-29). Whether the stored runs are current with every later default is not stated in the notes.
