# Run a lid-driven cavity and compare it with published data

The lid-driven square cavity is the first benchmark of most incompressible solvers. This tutorial runs it at Re = U L / nu = 100,
samples the vertical centreline, sweeps the Reynolds number with an operating map and points at the published reference values
kept in the repository. Start with the [quick start](quick-start-cli.md) if you have not built `rcfd` yet.

## The case

A unit square, lid speed 1 in +x, nu = 0.01 (Re = 100), 32 x 32 cells, SIMPLEC with the second-order `linear_upwind` convection
scheme. The settings are explained in [the case schema](../case-schema.md).

```toml file=cavity_re100.toml
title = "lid-driven cavity, Re 100"

[mesh]
generator = "cavity_2d"
nx = 32
ny = 32

[physics]
nu = 0.01

[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]

[boundaries.walls]
velocity = "fixed"

[solver]
algorithm = "simplec"
convection = "linear_upwind"
max_iterations = 2000
tolerance_u = 1e-6
tolerance_p = 1e-6

[reports.lid_drag]
type = "force"
patch = "lid"
direction = [1.0, 0.0, 0.0]
component = "viscous"
```

## Run and sample

```console run
$ rcfd run cavity_re100.toml --out re100 --quiet
```

```text output
1024 cells, algorithm simplec
converged: residual targets met
```

`sample` runs the case again and writes the solution along a line: here the vertical centreline x = 0.5 with 33 points.

```console run
$ rcfd sample cavity_re100.toml --line 0.5,0,0:0.5,1,0 --points 33 --fields ux,uy --out centreline.csv
```

```text output
wrote centreline.csv
```

`centreline.csv` has the columns `s, x, y, z, ux, uy, inside`.

## Compare with Ghia, Ghia and Shin (1982)

The repository keeps the published centreline values in `cases/cavity/reference/ghia1982_u_centerline.csv` (columns `y`,
`u_Re100`, `u_Re400`, `u_Re1000`); the citation and the 2 % tolerance are in `cases/cavity/case.json` and
[the reference-data notes](../reference-data.md). Plot `ux` against `y` from `centreline.csv` over the reference points. When this
tutorial was written the 32 x 32 run gave ux = -0.207 at y = 0.5 against the reference -0.20581; that number is not checked by
the tutorial test (it is a rounded observation), the quantitative comparison is done by the validation tests
(`crates/rcfd-api/tests/validation_cavity_ghia.rs`).

```text not-executed
y,u_Re100,u_Re400,u_Re1000
0.5,-0.20581,-0.11477,-0.0608
```

## A Reynolds-number sweep

An operating map runs the case over a list of settings; each point restarts from the previous converged solution
(see [the design loop](../design-loop.md)). Re = 100, 200 and 400:

```console run
$ rcfd map cavity_re100.toml --axis physics.nu=0.01,0.005,0.0025 --out re_sweep
```

```text output
map: 3 points
map complete: 3 of 3 points converged
```

`re_sweep/map.csv` has one row per point with the status, the iteration count and the reports (`lid_drag`). At Re = 400 the
32 x 32 mesh is coarse; compare with the `u_Re400` column and refine to see the difference.

## Refine

A finer mesh by overriding two settings (this block runs only in the slow test):

```console run-slow
$ rcfd run cavity_re100.toml --out re100_64 --quiet --set mesh.nx=64 --set mesh.ny=64
```

```text output
4096 cells
converged
```

## What next

* The [example gallery](gallery.md) lists the other ready-made cases.
* The benchmark descriptor `cases/cavity/case.json` (listed by `rcfd cases`) records the Reynolds numbers and meshes of the formal
  benchmark.
