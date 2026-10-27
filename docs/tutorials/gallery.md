# Example gallery

This page is a catalogue of the ready-made cases in `cases/`: each directory is listed below with what it shows and how far it
has been checked. Use it to pick a case to start from or to see what a case file for a given kind of flow looks like. If this is
your first run, start with the [quick start](quick-start-cli.md); for the meaning of every setting, see
[the case schema](../case-schema.md).

## How far the cases are checked

In the default test suite every case file is checked (`rcfd check`: validation, mesh build, mesh quality and the set-up rules);
the full runs are in `run-slow` blocks executed only by the slow (ignored) test. A check does not solve the case: it shows that
the case sets up, not that its result is right. Each entry below says which slow runs are expected to succeed and which are
known not to converge. All commands are run from the repository root.

## Two kinds of files

* `*.toml` files are runnable case files (the format is in [the case schema](../case-schema.md)).
* `case.json` files are benchmark descriptors (the Reynolds numbers, meshes, reference data and tolerances of a validation case) used
  by `rcfd cases` and the validation tests; they are not case files and `rcfd run` refuses them.

```console run
$ rcfd cases --dir cases
```

```text output
cavity
naca0012
```

## Benchmark descriptors

Open benchmark data for validating rcfd, with sources and licences, is described in [validation data](../validation-data.md).

| Directory | Benchmark | Reference data |
|---|---|---|
| `backward_step` | backward-facing step | see `docs/reference-data.md` |
| `cavity` | lid-driven cavity, Re 100 / 400 / 1000 | Ghia, Ghia and Shin (1982) in `cases/cavity/reference/`; tutorial: [cavity](cavity.md) |
| `channel` | plane channel / Poiseuille flow (also holds the channel case files below) | `cases/channel/reference/` |
| `flat_plate` | zero-pressure-gradient flat plate | `cases/flat_plate/reference/` |
| `heated_cavity` | differentially heated cavity | `cases/heated_cavity/reference/` |
| `naca0012` | NACA 0012 aerofoil | `cases/naca0012/reference/` |

## Runnable cases

Each entry has the same layout: the files and what each one shows, how far they are checked, then the commands.

### Channel flows (`channel`)

* `kepsilon_channel.toml`: periodic plane channel at Re_tau = 395, standard k-epsilon with wall functions, coarse on purpose.
* `kepsilon_inlet.toml`: developing channel with the inlet turbulence helpers (intensity and length scale), RNG k-epsilon.
* `profile_inlet.toml`: laminar channel whose inlet velocity is read from `profile_inlet.csv`; analytic pressure drop 2.4.
* `sst_channel.toml`: wall-resolved channel at Re_tau = 395, k-omega SST on an inflated y+ = 1 mesh.

Checked: the default test checks all four files. The slow test runs all four and expects each run to succeed.

```console run
$ rcfd check cases/channel/kepsilon_channel.toml
$ rcfd check cases/channel/kepsilon_inlet.toml
$ rcfd check cases/channel/profile_inlet.toml
$ rcfd check cases/channel/sst_channel.toml
```

```text output
kepsilon_channel.toml: ok, 56 cells
sst_channel.toml: ok, 240 cells
```

```console run-slow
$ rcfd run cases/channel/kepsilon_channel.toml --out out/kepsilon_channel --quiet
$ rcfd run cases/channel/kepsilon_inlet.toml --out out/kepsilon_inlet --quiet
$ rcfd run cases/channel/profile_inlet.toml --out out/profile_inlet --quiet
$ rcfd run cases/channel/sst_channel.toml --out out/sst_channel --quiet
```

```text output
converged
```

### Porous and rotating channels (`packed_bed`, `rotating_channel`)

* `packed_bed.toml`: air through a packed bed of 5 mm spheres (Ergun porous resistance). Its own settings stop at the 1700-iteration
  cap without meeting the tolerances, so the run exits with code 5 (not converged); this is the current state, not hidden.
* `rotating_channel.toml`: laminar channel rotating about the spanwise axis; the Coriolis force is balanced by a cross-stream
  pressure gradient.

Checked: the default test checks both files. In the slow test the packed bed run is expected to stop without converging (the exit
code given above) and the rotating channel run is expected to succeed.

```console run
$ rcfd check cases/packed_bed/packed_bed.toml
$ rcfd check cases/rotating_channel/rotating_channel.toml
```

```console run-slow exit=5
$ rcfd run cases/packed_bed/packed_bed.toml --out out/packed_bed --quiet
```

```console run-slow
$ rcfd run cases/rotating_channel/rotating_channel.toml --out out/rotating_channel --quiet
```

### Compressible flow (`sod`, `wedge`)

* `sod.toml`: Sod shock tube with the compressible Euler solver (HLLC, limited reconstruction, SSP Runge-Kutta 3).
* `wedge.toml`: Mach 2 flow over a 10 degree compression ramp; an oblique shock leaves the corner.

Checked: the default test checks both files. The slow test runs both, expects each run to succeed and looks for the end-time
message in the output.

```console run
$ rcfd check cases/sod/sod.toml
$ rcfd check cases/wedge/wedge.toml
```

```console run-slow
$ rcfd run cases/sod/sod.toml --out out/sod --quiet
$ rcfd run cases/wedge/wedge.toml --out out/wedge --quiet
```

```text output
end time 3 reached
```

### Axial fan passages (`axial_fan_sector`, `axial_fan_cgrid`)

Single periodic blade passages of a five-bladed axial fan built by the parametric fan generator ([meshing](../meshing.md)). Both are
laminar smoke cases for the geometry, the mesh and the rotating-frame plumbing, **not** predictions of fan performance; the state of
the fan work, including the missed validation against measured fan data, is in [the fan milestone](../milestone-fan.md).

* `axial_fan_sector.toml`: H-type sector mesh with blunt blade edges.
* `axial_fan_cgrid.toml`: body-fitted C-grid around the blade; `rcfd check` warns (rule SV101) that the maximum
  non-orthogonality exceeds 70 degrees.

Checked: the default test checks both files and requires the non-orthogonality warning to appear in the check output. The slow runs are
expected not to converge, as described below.

```console run
$ rcfd check cases/axial_fan_sector/axial_fan_sector.toml
$ rcfd check cases/axial_fan_cgrid/axial_fan_cgrid.toml
```

```text output
axial_fan_sector.toml: ok, 13332 cells
SV101
axial_fan_cgrid.toml: ok, 11536 cells
```

200 iterations of each do not converge (exit code 5); they show that the set-up runs, nothing more:

```console run-slow exit=5
$ rcfd run cases/axial_fan_sector/axial_fan_sector.toml --out out/fan_sector --quiet --max-iterations 200
$ rcfd run cases/axial_fan_cgrid/axial_fan_cgrid.toml --out out/fan_cgrid --quiet --max-iterations 200
```

## Other examples in the repository

* The design-loop contraction duct and the fan sector through the assistant tools: [MCP design loop](mcp-design-loop.md).
* Small cases inside the test suites (`crates/*/tests/`) and the built-in templates listed by the MCP tool `list_cases`.

## Where to go next

* [Run a lid-driven cavity and compare it with published data](cavity.md): a worked run of the cavity benchmark.
* [The case schema](../case-schema.md): every setting used by these case files.
* [Meshing](../meshing.md): the mesh generators the cases use.
