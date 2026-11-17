# Quick start: lid-driven cavity and heated cavity

This is the small finished example project of the rcfd Studio (ADR 0142): two classic closed-box flows, each as a case,
a variant and a completed run of both, made by the real solver through the run service. Every run takes one or two
seconds, so this is the project to try the Studio's own run controls on. The meshes are coarse and there is no mesh
study, so nothing here is a validation (see Limits).

| Problem | Cases | What it shows |
|---|---|---|
| Lid-driven cavity | `cases/cavity-re100.toml`, variant `cases/cavity-re400.toml` | a square box whose lid slides at 1 m/s drives one main vortex and small corner vortices; the tutorial `../../docs/tutorials/cavity.md` runs the same case |
| Heated cavity | `cases/heated-cavity-ra1e3.toml`, variant `cases/heated-cavity-ra1e4.toml` | air in a square box with a hot left wall and a cold right wall circulates by buoyancy (natural convection, Boussinesq) |

All meshes come from the generators named in the cases (`cavity_2d`, `box_2d`); `meshes/README.md` only keeps the folder.
The design ledger `designs/variants.ledger.jsonl` (ADR 0121) holds the parameters and results of the four runs.

## Open it

1. If you want to start runs of your own, copy this folder out of the repository. New runs would otherwise show up as
   untracked files in the repository. Opening and looking write nothing into the folder.
2. Start the desktop Studio as the tutorial `../../docs/tutorials/desktop-assistant.md` says. There is no installer yet:
   build `rcfd`, `rcfd-runner` and `rcfd-mcp` in release, then run `npm start` in `apps/studio-web/` with
   `RCFD_STUDIO_RCFD` set to the `rcfd` you built.
3. Choose Open project folder… (Ctrl+O) and pick this folder.

## What to look at, in order

The Studio names many controls only in tooltips and accessible names; the names below are those.

1. Home (Ctrl+1) lists the four cases under `cases/` and their runs. Edit on the row of `cases/cavity-re100.toml` opens the
   case editor on its settings, and Check the case runs the engine's check. Ctrl+1 goes back to Home.
2. Runs (Ctrl+2): select the run of `cases/cavity-re100.toml`, then Open in workspace. The field view shows `p`. Choose `ux`
   or `uy` in Field, and point at the field to read the value of a cell.
3. Scrub: these runs are steady, so the scrubber runs over iterations and stops at the stored snapshots (every 5 or 10).
   First snapshot, then Play: watch the vortex form while the solver converges. The time panel shows the residuals and
   the report `lid_drag` (or `q_hot` and `nu_hot`, one at a time, chosen in Monitor series).
4. Compare: Compare runs (the pictogram in the field bar), then the run of `cases/cavity-re400.toml` as Compared run, and
   Side by side: the vortex centre moves towards the middle and the lower corner vortices grow. Then the two
   heated-cavity runs with the field `T`: at Ra 1e4 the isotherms bend into an S.
5. Run something yourself (in a copy, step 1 of Open it): Run on the row of `cases/cavity-re400.toml` on Home, or F5. The
   new run shows in Runs within a second or two. Or change `physics.nu` in the case editor, Save (Ctrl+S), and run it.

## The runs in this folder

`scripts/make_example_projects.py` writes this table from the run folders. Wall times are wall-clock times of the machine
that made the runs.

<!-- generated: runs (scripts/make_example_projects.py) -->

| Case | Run | State | Steps | Wall time | Snapshots | Results (final values) |
|---|---|---|---|---|---|---|
| `cases/cavity-re100.toml` | `runs/r-20261028T220751-76ca` | finished (converged) | 121 | 0.5 s | 25 | lid_drag -0.004911 |
| `cases/cavity-re400.toml` | `runs/r-20261028T220751-919f` | finished (converged) | 158 | 0.7 s | 32 | lid_drag -0.001609 |
| `cases/heated-cavity-ra1e3.toml` | `runs/r-20261028T220751-9dc3` | finished (converged) | 339 | 2.0 s | 34 | nu_hot 1.119, q_hot -0.02797 |
| `cases/heated-cavity-ra1e4.toml` | `runs/r-20261028T220751-d599` | finished (converged) | 278 | 1.6 s | 28 | nu_hot 2.257, q_hot -0.05643 |

<!-- end of generated runs -->

## Limits

* Coarse meshes (32 x 32 cavity, 40 x 40 heated cavity), no mesh study: nothing here is a validation.
* For orientation only, the heated cavity's mean hot-wall Nusselt number `nu_hot` can be set against de Vahl Davis's
  benchmark values 1.118 (Ra 1e3) and 2.243 (Ra 1e4) in the repository file
  `../../cases/heated_cavity/reference/devahldavis1983.csv`, and the cavity's centre-line velocities against Ghia et al.
  through the tutorial `../../docs/tutorials/cavity.md`.
* `nu_hot` divides the heat flow by the cell depth 0.025 m that the 2D generators give a 40 x 40 box of side 1 m; a case
  with another cell count needs another divisor.
* The fields are stored in single precision (f32, ADR 0133); the solver runs in double precision.
* The run records hold the absolute paths of the machine that made them (`case_source`, `case_dir` in each `run.json`);
  they are information only.
