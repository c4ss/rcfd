# Wind tunnel: three flow problems in one project

This finished example project of the rcfd Studio holds three classic external and internal flows, each as a case file, a
variant of it and a completed run of both, made by the real solver through the run service (ADR 0142). Open it to see a
picture of a run, scrub it through time, and compare a case with its variant. Every mesh is coarse on purpose and no
result here is a validation, so read the limits at the end of this page before using a number. To start, follow "Open it";
the table names each problem's own note. ADR numbers refer to the architecture decision records in the repository's
`docs/adr/`.

| Problem | Cases | Mesh | Notes |
|---|---|---|---|
| Flow past a cylinder, vortex shedding (transient) | `cases/cylinder-re100.toml`, variant `cases/cylinder-re200.toml` | O-grid generator in the case | `notes/cylinder.md` |
| NACA 0012 aerofoil, k-omega SST (steady) | `cases/naca0012-aoa4.toml`, variant `cases/naca0012-aoa2.toml`; the same angles by a rotated mesh: `cases/naca0012-aoa4-rotated.toml`, `cases/naca0012-aoa2-rotated.toml` | `meshes/naca0012-ogrid` from `meshes/naca0012-ogrid.recipe.toml` | `notes/naca0012.md` |
| Backward-facing step, laminar (steady) | `cases/step-re800.toml`, variant `cases/step-re400.toml` | box generator in the case | `notes/step.md` |

The design ledger `designs/variants.ledger.jsonl` (format of ADR 0121, one line per run) holds the parameters and the
results of the eight runs, so you can compare a case and its variant as numbers. The two aerofoil cases ending in
`-rotated` set the angle of attack by rotating the mesh (`[mesh.transform]`, ADR 0155) instead of the flow; their
results equal those of the flow-rotated cases to about 1e-7 (`notes/naca0012.md`).

## Open it

1. If you want to start runs of your own, copy this folder out of the repository. New runs would otherwise show up as
   untracked files in the repository. Opening and looking write nothing into the folder.
2. Start the desktop Studio as the tutorial `../../docs/tutorials/desktop-assistant.md` says. There is no installer yet:
   build `rcfd`, `rcfd-runner` and `rcfd-mcp` in release, then run `npm start` in `apps/studio-web/` with
   `RCFD_STUDIO_RCFD` set to the `rcfd` you built.
3. Choose Open project folder… (Ctrl+O) and pick this folder.

## What to look at, in order

The Studio names many controls only in tooltips and accessible names; the names below are those.

1. Home (Ctrl+1) lists the eight cases under `cases/`, the mesh under `meshes/`, the runs and the notes. Filter by name
   narrows the list. The design ledger `designs/variants.ledger.jsonl` is not listed there; open it in an editor.
2. Edit on the row of a case (for example `cases/cylinder-re100.toml`) opens the case editor on its settings. Check the
   case runs the engine's check, and Check the mesh runs the engine's check of the mesh. The desktop Studio has no
   picture of a mesh before a run yet.
3. Runs (Ctrl+2) lists the eight finished runs, one per case, with their state and step. Select the run of
   `cases/cylinder-re100.toml`, then Open in workspace.
4. The field view shows the run's pressure field `p`. Choose another field in Field (`vorticity_z` for the cylinder), and
   point at the field to read the value of a cell.
5. To scrub, drag the scrubber or use First snapshot, Previous snapshot, Play and Next snapshot (with the scrubber
   focused: the arrow keys, Home and End). The axis is iterations for every run; for a transient run (the cylinder) the
   scrubber's flag also gives the simulated time. The time panel shows the residuals and one report at a time, chosen
   in Monitor series (`cd`, `cl`, ...), with a line at the shown step.
6. To compare a variant, choose Compare runs (the pictogram in the field bar), then the run of the variant case (for
   example `cases/cylinder-re200.toml`) as Compared run, and Side by side. One scrubber moves both views, at the same
   stored time; where the compared run has no snapshot at that time, its view says so. A table under the run's numbers
   gives the engine's comparison metrics; the numbers of both runs are also in the table below and in
   `designs/variants.ledger.jsonl`.
7. To run a variant yourself, work in a copy of the folder (step 1 of "Open it"). To try another value, first change it
   in the case editor (for example `physics.nu`) and Save (Ctrl+S). Then choose Run on the case's row on Home, or F5 and
   the case. The run stores the fields the case asks for. The new run shows in Runs and in the Workspace; the committed
   run stays for comparison.

## The runs in this folder

`scripts/make_example_projects.py` writes the table from the run folders. Times are wall-clock times of the solver on the
machine that made them. `designs/variants.ledger.jsonl` holds every value with its unit.

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
* Since the far-field default of ADR 0169 (2026-10-29), the aerofoil runs meet the residual targets well before their
  2000-iteration cap. The runs stored earlier stopped at the cap with solver state "not_converged". See
  `notes/naca0012.md`.
* The fields are stored in single precision (f32, ADR 0133): each stored value is within |x| 2^-24 of the solver's
  value. The solver itself runs in double precision.
* The aerofoil cases name their mesh as `../meshes/naca0012-ogrid`, a path relative to the case's folder. The desktop
  Studio's case editor opens a case file through the solver service, which resolves relative paths against the case's
  folder, and the run service does the same for runs.
* The run records hold the absolute paths of the machine that made them (`case_source`, `case_dir` in each `run.json`,
  paths in the solver events); they are information only.
