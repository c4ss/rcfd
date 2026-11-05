# Example projects

Finished project folders of the rcfd Studio (ADR 0142, docs/adr/0142-wave48h-example-projects.md). Each holds case files,
their meshes, completed runs with stored fields, a design ledger and notes, so the Studio shows a picture as soon as you
open a project and select a run. No result in these folders is a validation: the meshes are coarse on purpose (see each
project's notes).

The cases of `wind-tunnel/` and `quick-start/` are one cell thick (2D), and the Studio draws them face-on. `axial-fan/` is
the 3D example, drawn in a view that rotates (middle drag).

| Folder | What is inside | Start with |
|---|---|---|
| `wind-tunnel/` | flow past a cylinder (vortex shedding), a NACA 0012 aerofoil (k-omega SST) and a laminar backward-facing step, each with a variant and both runs | `wind-tunnel/notes/README.md` |
| `quick-start/` | a lid-driven cavity and a heated cavity, each with a variant and both runs; every run takes one or two seconds | `quick-start/notes/README.md` |
| `axial-fan/` | one periodic blade passage of the FAN-01 axial fan in 3D (k-omega SST, rotating frame) and one run with first-order `upwind`, for viewing the 3D geometry and flow pattern only: not a validated fan prediction (ADR 0176) | `axial-fan/notes/README.md` |

Build with `cargo build --release -p rcfd-studio -p rcfd-runner -p rcfd-cli`, then open a project with
`target\release\rcfd-studio.exe example-projects\wind-tunnel`. To start runs of your own, copy the folder out of the
repository first; new runs inside the checkout show up as untracked files (the checks below ignore them).

The real solver made the runs through the run service, with `scripts/make_example_projects.py`. That script also rebuilds
them (`python scripts/make_example_projects.py`) and checks the committed folders against their case files
(`python scripts/make_example_projects.py --check`). Its `--rerun-check` option rebuilds into a temporary copy and
compares the reproducible files byte for byte.

`manifest.json` lists the committed runs and the size caps. The tests `crates/rcfd-mcp/tests/example_projects_w48h.rs`
and `scripts/test_make_example_projects.py` fail when a committed example goes stale.
