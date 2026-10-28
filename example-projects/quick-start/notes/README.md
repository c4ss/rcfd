# Quick start: lid-driven cavity and heated cavity

This is the small finished example project of the rcfd Studio (ADR 0142): two classic closed-box flows, each as a case,
a variant and a completed run of both, made by the real solver through the run service. Every run takes about a second,
so this is the project to try the Studio's own run controls on.

| Problem | Cases | What it shows |
|---|---|---|
| Lid-driven cavity | `cases/cavity-re100.toml`, variant `cases/cavity-re400.toml` | a square box whose lid slides at 1 m/s drives one main vortex and small corner vortices; the tutorial `../../docs/tutorials/cavity.md` runs the same case |
| Heated cavity | `cases/heated-cavity-ra1e3.toml`, variant `cases/heated-cavity-ra1e4.toml` | air in a square box with a hot left wall and a cold right wall circulates by buoyancy (natural convection, Boussinesq) |

All meshes come from the generators named in the cases (`cavity_2d`, `box_2d`); `meshes/README.md` only keeps the folder.
The design ledger `designs/variants.ledger.jsonl` (ADR 0121) holds the parameters and results of the four runs.

## Open it

1. Build: `cargo build --release -p rcfd-studio -p rcfd-runner -p rcfd-cli`.
2. Copy this folder out of the repository if you want to start runs of your own (new runs would otherwise show up as
   untracked files in the repository).
3. Start `target\release\rcfd-studio.exe <this folder>`, or Project > Open project... in the Studio.

## What to click, in order

1. **Project panel**: four cases under `cases/`. Select `cases/cavity-re100.toml`: the Case editor shows its settings;
   Case > Validate checks it.
2. **Runs panel**: select the run of `cases/cavity-re100.toml`. The **Viewport** shows `p`; choose `ux` or `uy` in the
   "field" box and hover for values.
3. **Scrub**: these runs are steady, so the slider runs over iterations (a snapshot every 5 or 10): watch the vortex form
   while the solver converges. The **Plots** panel shows the residuals and the report `lid_drag` (or `q_hot` and `nu_hot`).
4. **Compare**: select the run of `cases/cavity-re400.toml` (the vortex centre moves towards the middle and the lower
   corner vortices grow), then the two heated-cavity runs with the field `T` (at Ra 1e4 the isotherms bend into an S).
5. **Run something yourself** (in a copy, step 2): Runs > Start run..., pick `cases/cavity-re400.toml`, Start. The run
   streams into the Runs and Plots panels within a second or two. Or change `physics.nu` in the Case editor, Save, and run it.

## The runs in this folder

Written by `scripts/make_example_projects.py` from the run folders; wall-clock times of the machine that made them.

<!-- generated: runs (scripts/make_example_projects.py) -->
<!-- end of generated runs -->

## Limits

* Coarse meshes (32 x 32 cavity, 40 x 40 heated cavity), no mesh study: nothing here is a validation. For orientation
  only: the heated cavity's mean hot-wall Nusselt number `nu_hot` can be set against de Vahl Davis's benchmark values
  1.118 (Ra 1e3) and 2.243 (Ra 1e4) in the repository file `../../cases/heated_cavity/reference/devahldavis1983.csv`;
  the cavity's centre-line velocities against Ghia et al. through the tutorial `../../docs/tutorials/cavity.md`.
* `nu_hot` divides the heat flow by the cell depth 0.025 m that the 2D generators give a 40 x 40 box of side 1 m; a case
  with another cell count needs another divisor.
* The fields are stored in single precision (f32, ADR 0133); the solver runs in double precision.
* The run records hold the absolute paths of the machine that made them (`case_source`, `case_dir` in each `run.json`);
  they are information only.
