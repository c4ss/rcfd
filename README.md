# rcfd

rcfd is a clean-room, open source (AGPL-3.0) finite-volume CFD suite in Rust.
It has a command-line solver, a solver service, an MCP server that lets an AI
assistant drive the solver, and a desktop app.

Clean-room means every algorithm comes from published literature or first
principles, with no vendor code, binaries or documentation text. Vendor
documentation was consulted in a few recorded cases: mostly contents pages, to
list capabilities, and some theory pages, as a check of model constants that
are now marked UNVERIFIED. The [provenance record](docs/provenance.md) lists
each case. [CONTRIBUTING.md](CONTRIBUTING.md) sets out the policy.

## Status and main limits

rcfd is early-stage software. It has not been validated for engineering
decisions and must not be used for safety-critical work. It comes with no
warranty of any kind (see the licences). The main gaps today:

* The steady rotating-frame solve of the FAN-01 reference fan case does not
  reach its expected operating point, and the diagnosis is open
  ([fan milestone](docs/milestone-fan.md)).
* There is no structural solver.
* Sliding-mesh transient solves run from a case file for the full-annulus
  rotor-stator generator and its sector form (`[mesh.sliding]`, F-303,
  ADR 0637; F-711, ADR 0646; laminar piso / pimple); planar sliding is in the
  library only.
* Overset (chimera) meshes are not available from a case or the app; the
  library has a first overset coupling of one moving component mesh
  (F-134, ADR 0219).
* The desktop app is being rebuilt. The new Studio runs from a checkout and
  has no installer yet; the two older egui desktop apps were removed (F-449).

![Roadmap progress: done, partly done and not started items](docs/roadmap-progress.svg)

![Roadmap progress by area](docs/roadmap-categories.svg)

[The roadmap overview](docs/roadmap-overview.md) says what works today, what
does not and what comes next. See [docs/ROADMAP.md](docs/ROADMAP.md) for the
plan.

## Quick start

You need a Rust toolchain (see the prerequisites below). From the repository
root, build the command-line solver and check one of the shipped cases:

    cargo build --release -p rcfd-cli
    target/release/rcfd --help
    target/release/rcfd check cases/sod/sod.toml

On Windows the binary is `target\release\rcfd.exe`. `check` builds the mesh and
validates the set-up without solving.

For a first real run, follow
[the quick-start tutorial](docs/tutorials/quick-start-cli.md): it writes a
small lid-driven cavity case, runs it to convergence and post-processes the
result. Apart from the build, the test suite executes every command in it (and
the `check` above).

## Build and test

Prerequisites:

* Rust 1.90 or later. With 1.90, CI type-checks the whole workspace. With
  1.99, it builds and tests everything. Tests have not been run on 1.90. Each
  crate's `rust-version` states its own minimum (1.75 to 1.85): 1.75, or the
  highest minimum its dependencies declare if that is higher. Older
  toolchains are not tested.
* CUDA support is an optional feature, off by default
  ([docs/gpu.md](docs/gpu.md)).
* Node.js 22.12 or later with npm, for the desktop Studio only.
* Python 3.9 or later is optional: the test-gate script and the data fetch
  script use it, and the Python tests skip without it.

Build and test:

    cargo build --workspace --release
    cargo test --workspace --release

The full test suite takes about an hour in release mode on a 16-thread
machine, and much longer in debug mode. The project's gate, the full check run
before work is merged, runs it with `python scripts/gate_test.py`
([docs/test-process.md](docs/test-process.md)).

The desktop Studio is the Electron app in `apps/studio-web/`. It starts the
solver service, the project server and the run service from the folder of
the `rcfd` you name, so build those first (POSIX shell; the
[desktop tutorial](docs/tutorials/desktop-assistant.md) has the PowerShell
lines and the steps in the window):

    cargo build --release -p rcfd-cli -p rcfd-runner -p rcfd-mcp
    cd apps/studio-web && npm ci && npm run build
    RCFD_STUDIO_RCFD="$PWD/../../target/release/rcfd" npm start

The older egui desktop apps, `rcfd-studio` and `rcfd-gui`, were removed
(roadmap item F-449); the git history keeps them. The local-model assistant
of `rcfd-gui` (crate `rcfd-assistant`) and the wgpu render crate
`rcfd-render` were removed after them (F-838), and then `rcfd-claude`, the
Claude Code bridge of the egui Studio: the Studio's Claude Code panel is the
only assistant.

Finished example projects that open straight to a picture are in
`example-projects/` (runs made on Windows) and `example-projects-linux/` (the
same projects, runs made on x86_64 Linux). Open the folder of your platform
with Open project folder… in the Studio. The cases of `wind-tunnel` and
`quick-start` are 2D; `axial-fan` holds a 3D blade passage of an axial fan
that the Studio shows in a 3D view you can turn (for viewing only: a
first-order run, not a validated fan prediction).

## Where to go next

* Documentation, tutorials and the example gallery start at
  [docs/index.md](docs/index.md); the test suite executes the tutorials.
* [CONTRIBUTING.md](CONTRIBUTING.md) has the clean-room rule and the commands
  to run before you submit a change.
* [The glossary](docs/glossary.md) explains the project's own terms (wave,
  gate, ADR, F-xxx ids, FAN-01 and others) that appear throughout the docs.
* [docs/architecture.md](docs/architecture.md) describes the crate layout.
* [The provenance record](docs/provenance.md) says where rcfd's knowledge
  comes from.
* [SECURITY.md](SECURITY.md) says how to report a vulnerability and which
  parts run programs or listen for requests; [CHANGELOG.md](CHANGELOG.md) is
  next to it.

## Questions and bug reports

Open a GitHub issue.
Report security problems privately, as [SECURITY.md](SECURITY.md) describes.

## Licence

rcfd is free software under the [GNU AGPL v3](LICENSE): use and change it
for any purpose, including commercial use; if you distribute it or run a
modified version as a network service, you must offer your users the source.
For use without those conditions, a [commercial licence](COMMERCIAL-LICENSE.md)
is available. Third-party
data shipped in `validation-data/` and `cases/*/reference/` keep their own
terms and attributions; see `NOTICE` and `validation-data/manifest.json`.
The licences of the third-party Rust packages rcfd depends on are listed in
[THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).

## Trademarks and independence

All product and company names mentioned in this repository are trademarks or
registered trademarks of their respective owners and are used only to describe
file formats or to compare capabilities. rcfd is an independent project and is
not affiliated with, endorsed by or sponsored by any of them.
