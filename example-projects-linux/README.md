# Example projects, Linux x86_64 build

The finished example projects of `../example-projects/` (ADR 0142, docs/adr/0142-wave48h-example-projects.md) with
runs made on x86_64 Linux (ADR 0150, docs/adr/0150-job48p-linux-example-stores.md). Open these on Linux, and the ones
in `../example-projects/` on Windows:

    sh scripts/studio.sh example-projects-linux/wind-tunnel

| Folder | What is inside | Start with |
|---|---|---|
| `wind-tunnel/` | flow past a cylinder (vortex shedding), a NACA 0012 aerofoil (k-omega SST) and a laminar backward-facing step, each with a variant and both runs | `wind-tunnel/notes/README.md` |
| `quick-start/` | a lid-driven cavity and a heated cavity, each with a variant and both runs; every run takes one or two seconds | `quick-start/notes/README.md` |

Why a second tree: mesh generators call the C library's libm, whose last bits differ between Windows and Linux (ADR
0137), so the mesh a run stored equals the mesh the same case builds only on the platform that made the run. Each tree
holds the runs of one platform; the case files, the mesh recipe, the notes and `rcfd-project.json` are the same as in
`../example-projects/`, from where the script copies them before a rebuild (the notes' run tables list this tree's
runs). The notes speak of `target\release\rcfd-studio.exe`; on Linux it is `target/release/rcfd-studio`.

The runs were made by the real solver through the run service with `scripts/make_example_projects.py` in Docker, in
the image of `scripts/docker/ubuntu-24.04-rust.Dockerfile` (Ubuntu 24.04 with glibc 2.39, the CI runners' system, and
Rust 1.99.0, the gate's toolchain). From a checkout, on a machine with Docker:

    python scripts/make_example_projects.py --docker                 rebuild this tree and put it in place
    python scripts/make_example_projects.py --docker --rerun-check   rebuild it in a temporary copy and compare
    python scripts/make_example_projects.py --docker --add-missing   copy the inputs, run only the cases with no run here
    python scripts/make_example_projects.py --check                  check both trees and their shared inputs

Change an input in `../example-projects/`, rebuild that tree on Windows and this one with `--docker`; `--check` and the
tests (`crates/rcfd-mcp/tests/example_projects_w48h.rs`, `scripts/test_make_example_projects.py`) report a tree whose
inputs differ from the other's. `manifest.json` lists the committed runs, the platform that made them and the size caps,
which apply to each tree on its own. No result in these folders is a validation (see each project's notes).
