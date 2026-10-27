# Changelog

What changed in each release of rcfd, newest first, written for people who use it. Each release has its version and date; its
changes are grouped as Added, Changed, Deprecated, Removed, Fixed and Security, followed by Known limitations where a release
has them. Changes merged since the last release are under
Unreleased. What a version number promises, and how a release is made, is in [docs/releasing.md](docs/releasing.md). The full
per-item record is in [docs/ROADMAP.md](docs/ROADMAP.md) and the decision records in `docs/adr/`.

Before 1.0 a minor version (0.1 to 0.2) may break command-line options, case files, MCP tools, the service protocol, the UDF ABI,
the Python functions, output formats or default numerical results; a patch version (0.1.0 to 0.1.1) does not.

## Unreleased

Nothing yet.

## 0.1.0 - not yet released

The first release. Everything in it is new, so this entry says what it contains and what it does not do.

rcfd is early-stage software. It has not been validated for engineering decisions, must not be used for safety-critical work, and
comes with no warranty. Of the 184 roadmap items, 95 are done, 63 partly done and 26 not started; a partly done item works for some
cases and its roadmap note states the gap.

### Added

* A finite-volume solver for incompressible and compressible flow on unstructured meshes, with a range of turbulence models, heat
  transfer, species transport and one-way coupled particle tracking.
* Mesh import and export in common formats, a mesh quality check, and VTK output.
* The `rcfd` command-line tool, a JSON-RPC solver service, and an MCP server through which an AI assistant can set up, run and read
  cases.
* A Python package that drives the command-line tool (no wheel; see `docs/python.md`) and sandboxed WebAssembly user functions
  (`docs/udf-abi.md`).
* For the computer-fan target, the parts that do not need a converged fan solve: parametric blade geometry and sector meshes,
  tolerance tools, a reduced-order fan performance model, rotor-dynamics checks and a design ledger that keeps every variant.
* For the new desktop application (Studio), its libraries, tested without a window: the project folder format, a background run
  service, a time-series store, a wgpu render core and the Claude Code bridge.
* Tutorials that the test suite executes, a validation data pack with fetch scripts and checksums, and a suite of about 2,200 tests
  whose numeric limits were written down before each run.

### Known limitations

* The FAN-01 reference fan does not reach its operating point. The steady rotating-frame solve stalls, the comparison with the
  measured fan misses, and the cause has not been found ([fan milestone](docs/milestone-fan.md)). No fan prediction in this
  release is validated against measurement.
* Fan noise: the FW-H solver, the tonal estimate and the broadband proxies exist, but none is validated.
* There is no structural solver, so no stress, strain or blade deformation.
* Sliding-mesh transient solves are not available, so no rotor-stator interaction in time.
* The old desktop app is frozen and the new one has no window yet.
* The CUDA backend is optional and off by default. Without a GPU, the GPU tests skip and count as passed.
* The project's own test gate runs on Windows. Linux is checked by CI (build, clippy, unit tests); macOS is not checked.
* The Rust crates are not published and their library API may change in any release.
