# Example projects

Finished project folders of the rcfd Studio (ADR 0142, docs/adr/0142-wave48h-example-projects.md). Each holds case files,
their meshes, completed runs with stored fields, a design ledger and notes, so the Studio shows a picture as soon as a
project is opened and a run is selected.

| Folder | What is inside | Start with |
|---|---|---|
| `wind-tunnel/` | flow past a cylinder (vortex shedding), a NACA 0012 aerofoil (k-omega SST) and a laminar backward-facing step, each with a variant and both runs | `wind-tunnel/notes/README.md` |
| `quick-start/` | a lid-driven cavity and a heated cavity, each with a variant and both runs; every run takes about a second | `quick-start/notes/README.md` |

Open one with `target\release\rcfd-studio.exe example-projects\wind-tunnel` after
`cargo build --release -p rcfd-studio -p rcfd-runner -p rcfd-cli`. To start runs of your own, copy the folder out of the
repository first; new runs inside the checkout show up as untracked files (the checks below ignore them).

The runs were made by the real solver through the run service with `scripts/make_example_projects.py`, which also
rebuilds them (`python scripts/make_example_projects.py`) and checks the committed folders against their case files
(`python scripts/make_example_projects.py --check`; a rebuild into a temporary copy with a byte comparison of the
reproducible files: `--rerun-check`). `manifest.json` lists the committed runs and the size caps; the tests
`crates/rcfd-mcp/tests/example_projects_w48h.rs` and `scripts/test_make_example_projects.py` fail when a committed example
goes stale. No result in these folders is a validation: the meshes are coarse on purpose (see each project's notes).
