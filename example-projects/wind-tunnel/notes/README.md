# Wind tunnel: three flow problems in one project

This is a finished example project of the rcfd Studio. It holds three classic external and internal flows, each as a case
file, a variant of it and a completed run of both, made by the real solver through the run service (ADR 0142). Open it to
see a picture at once, scrub a run through time, and compare a case with its variant.

| Problem | Cases | Mesh | Notes |
|---|---|---|---|
| Flow past a cylinder, vortex shedding (transient) | `cases/cylinder-re100.toml`, variant `cases/cylinder-re200.toml` | O-grid generator in the case | `notes/cylinder.md` |
| NACA 0012 aerofoil, k-omega SST (steady) | `cases/naca0012-aoa4.toml`, variant `cases/naca0012-aoa2.toml`; the same angles by a rotated mesh: `cases/naca0012-aoa4-rotated.toml`, `cases/naca0012-aoa2-rotated.toml` | `meshes/naca0012-ogrid` from `meshes/naca0012-ogrid.recipe.toml` | `notes/naca0012.md` |
| Backward-facing step, laminar (steady) | `cases/step-re800.toml`, variant `cases/step-re400.toml` | box generator in the case | `notes/step.md` |

The design ledger `designs/variants.ledger.jsonl` (format of ADR 0121, one line per run) holds the parameters and the
results of the eight runs, so a case and its variant can be compared as numbers. The two aerofoil cases ending in
`-rotated` set the angle of attack by rotating the mesh (`[mesh.transform]`, ADR 0155) instead of the flow; their
results equal those of the flow-rotated cases to about 1e-7 (`notes/naca0012.md`).

## Open it

1. Build the Studio and the two programs it starts: `cargo build --release -p rcfd-studio -p rcfd-runner -p rcfd-cli`.
2. Copy this folder out of the repository first if you want to start runs of your own (new runs would otherwise appear as
   untracked files in the repository; the committed runs are not changed by the Studio).
3. Start `target\release\rcfd-studio.exe <this folder>`, or start the Studio and use Project > Open project...

## What to click, in order

1. **Project panel**: the eight cases are listed under `cases/`, the mesh files under `meshes/`, the ledger under `designs/`.
2. **Select a case** (for example `cases/cylinder-re100.toml`): the Case editor shows its settings. Case > Validate checks
   it. If your Studio has the mesh preview of wave 48G, the Viewport shows the case's mesh.
3. **Runs panel**: the eight finished runs, one per case, with their final state and step. Select the run of
   `cases/cylinder-re100.toml`.
4. **Viewport**: the run's pressure field `p` appears on the mesh. Pick another field (`vorticity_z` for the cylinder) in the
   "field" box; hover to read values.
5. **Scrub**: drag the time slider, or use the first, previous, play, next and last buttons. A transient run (the cylinder)
   slides over simulated time; a steady run (aerofoil, step) slides over iterations. The Plots panel shows the residuals
   and the report monitors (`cd`, `cl`, ...) of the selected run, with a line at the viewport's step.
6. **Compare a variant**: select the run of the variant case (for example `cases/cylinder-re200.toml`) and look at the same
   field and time. Side-by-side views are not in the Studio yet (wave 49); the numbers of both runs are in the table below
   and in `designs/variants.ledger.jsonl`.
7. **Run a variant yourself** (in a copy of the folder, step 2): Runs > Start run..., pick the case, keep "Store fields"
   as the case sets it, Start. The new run streams into the Runs and Plots panels; the committed run stays for comparison.
   Change a value in the Case editor first (for example `physics.nu`), Save, then start a run of it.

## The runs in this folder

The table is written by `scripts/make_example_projects.py` from the run folders; times are wall-clock times of the solver
on the machine that made them (see `designs/variants.ledger.jsonl` for every value with its unit).

<!-- generated: runs (scripts/make_example_projects.py) -->

| Case | Run | State | Steps | Wall time | Snapshots | Results (final values) |
|---|---|---|---|---|---|---|
| `cases/cylinder-re100.toml` | `runs/r-20261028T152128-d5cb` | finished (finished) | 1280 | 18.4 s | 80 | cd 1.327, cl -0.3034, strouhal 0.1517, cd_mean 1.323, cl_amplitude 0.3091, periods 5 |
| `cases/cylinder-re200.toml` | `runs/r-20261028T152128-db3f` | finished (finished) | 1280 | 16.4 s | 80 | cd 1.286, cl 0.244, strouhal 0.1751, cd_mean 1.291, cl_amplitude 0.6303, periods 6 |
| `cases/naca0012-aoa4.toml` | `runs/r-20261029T230332-0566` | finished (converged) | 409 | 16.0 s | 3 | cd 0.0282, cl 0.3469 |
| `cases/naca0012-aoa2.toml` | `runs/r-20261029T230332-3d8a` | finished (converged) | 230 | 9.9 s | 2 | cd 0.02196, cl 0.1982 |
| `cases/step-re800.toml` | `runs/r-20261028T152128-806b` | finished (converged) | 740 | 23.4 s | 19 | p_inlet -0.2276, x_reattach 5.888, x_top_separation 4.652, x_top_reattach 10.42 |
| `cases/step-re400.toml` | `runs/r-20261028T152128-37c8` | finished (converged) | 429 | 13.4 s | 11 | p_inlet -0.1471, x_reattach 4.248, x_top_separation 3.856, x_top_reattach 5.271 |
| `cases/naca0012-aoa4-rotated.toml` | `runs/r-20261029T230332-2b1f` | finished (converged) | 409 | 16.5 s | 3 | cd 0.0282, cl 0.3469 |
| `cases/naca0012-aoa2-rotated.toml` | `runs/r-20261029T230332-c6be` | finished (converged) | 230 | 8.1 s | 2 | cd 0.02196, cl 0.1982 |

<!-- end of generated runs -->

## Limits (read before using a number)

* Every mesh is coarse on purpose, so that a run takes about a minute and the folder stays small. No result here is a
  validation: there is no mesh study, no time-step study and no uncertainty estimate. Each problem's note says how its
  numbers relate to the published benchmark.
* The aerofoil runs meet the residual targets well before their 2000-iteration cap since the far-field default of
  ADR 0169 (2026-10-29); the runs stored before stopped at the cap with solver state "not_converged". See
  `notes/naca0012.md`.
* The fields are stored in single precision (f32, ADR 0133): each stored value is within |x| 2^-24 of the solver's
  value. The solver itself runs in double precision.
* The Case editor of the Studio resolves relative paths against the Studio's working folder, not against the case's
  folder (ADR 0131, question 3). The aerofoil cases name their mesh as `../meshes/naca0012-ogrid`, so Case > Validate
  reports the mesh as missing unless the Studio was started in `cases/`. Runs are not affected: the run service resolves
  the path against the case's folder.
* The run records hold the absolute paths of the machine that made them (`case_source`, `case_dir` in each `run.json`,
  paths in the solver events); they are information only.
