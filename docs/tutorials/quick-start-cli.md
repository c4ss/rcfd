# Quick start: the rcfd command line

This tutorial builds `rcfd`, writes a small case file, validates it, runs it and looks at the results. At the end you have a
converged lid-driven cavity run and its result files on disk. It is for anyone new to rcfd who wants one working run before
reading the reference pages. If you already have a case and want to look up a setting, go to [the case schema](../case-schema.md)
instead; for exit codes, checkpoints and stop files, see [run control](../run-control.md).

What you need: a checkout of this repository and a Rust toolchain with cargo. The mesh comes from a built-in generator, so no mesh
file is needed. ParaView is optional, for looking at the solution file.

The steps: build the binary, write a case file, validate it, check the mesh and set-up, run it, then post-process the results.

Every command on this page is executed by `cargo test -p rcfd-cli --test tutorials` (see "How these tutorials are tested" near
the end), so the commands and the quoted output lines are what the current code does.

## 1. Build the binary

```console not-executed
cargo build --release -p rcfd-cli
```

The binary is `target/release/rcfd` (`rcfd.exe` on Windows); put it on your `PATH` or call it with its full path. The commands
below are run from the repository root.

```console run
$ rcfd --version
```

```text output
rcfd
```

`rcfd --help` lists every command with its options and the exit codes (0 success, 2 usage error, 3 invalid case, 4 mesh check
failed, 5 not converged, 6 diverged, 7 interrupted, 8 wall-clock limit).

## 2. Write the case file

A case is a TOML (or JSON) file with the sections described in [the case schema](../case-schema.md). This one is a lid-driven
cavity on a built-in 16 x 16 mesh; the patch names `lid` and `walls` come from the `cavity_2d` generator
(see [meshing](../meshing.md) for the generators). Save it as `my_cavity.toml` in the repository root.

```toml file=my_cavity.toml
title = "my first cavity"

[mesh]
generator = "cavity_2d"
nx = 16
ny = 16

[physics]
nu = 0.01

[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]

[boundaries.walls]
velocity = "fixed"

[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 600
tolerance_u = 1e-6
tolerance_p = 1e-6

[reports.lid_drag]
type = "force"
patch = "lid"
direction = [1.0, 0.0, 0.0]
component = "viscous"
```

## 3. Validate the case file

`validate` checks every key and value against the schema without building anything:

```console run
$ rcfd validate my_cavity.toml
```

```text output
my_cavity.toml: ok
```

A mistake is reported with its line number and the list of known keys, and the exit code is 3. Here the report key `patch`
is misspelt as `patches`:

```toml file=broken.toml
[mesh]
generator = "cavity_2d"
nx = 8
ny = 8

[reports.lid_drag]
type = "force"
patches = ["lid"]
```

```console run exit=3
$ rcfd validate broken.toml
```

```text output
line 8: reports.lid_drag.patches: unknown setting 'patches'
```

## 4. Check the mesh and set-up

`check` validates, builds the mesh, checks its quality and evaluates the set-up rules, without solving:

```console run
$ rcfd check my_cavity.toml
```

```text output
my_cavity.toml: ok, 256 cells
```

## 5. Run the case

```console run
$ rcfd run my_cavity.toml --out results --quiet
```

```text output
256 cells, algorithm simplec
converged: residual targets met
```

The output directory holds `solution.vtu` (open it in ParaView), `monitors.json` / `monitors.csv` (residual and report
histories), `report.html` and the checkpoint `restart.rcfd`. A setting can be changed for one run without editing the file:

```console run
$ rcfd run my_cavity.toml --out results_nu002 --quiet --set physics.nu=0.02
```

```text output
converged
```

## 6. Post-process the results

`post` re-creates the reports from a results directory; `sample` writes a line sample as CSV:

```console run
$ rcfd post results --format csv --out monitors_copy.csv
$ rcfd sample my_cavity.toml --line 0.5,0,0:0.5,1,0 --points 11 --fields ux --out centreline.csv
```

```text output
wrote monitors_copy.csv
wrote centreline.csv
```

`rcfd schema --out schema.json` writes the JSON schema of the case file, for editors that validate TOML or JSON against a schema.

```console run
$ rcfd schema --out schema.json
```

## How these tutorials are tested

Every code block of a tutorial is tagged in its fence: `file=<name>` blocks are written to a fresh working directory (which also
holds a copy of `cases/`), `run` blocks are executed with the real binary and must reach their exit code (`exit=<n>`, default 0),
`output` blocks list lines that must appear in the preceding command's output, `run-slow` blocks run only in the ignored slow test,
and `not-executed` blocks (like the build command above) are never run. An untagged block fails the test. See
[ADR 0089](../adr/0089-wave38b-docs-site-tutorials-gallery.md).

## Where to go next

* [Run a cavity and compare it with published data](cavity.md): the next tutorial, a finer mesh, a centreline sample and a
  Reynolds-number sweep.
* [The example gallery](gallery.md)
* [Drive rcfd from an assistant through MCP](mcp-design-loop.md)
* [The desktop app and the assistant sidebar](desktop-assistant.md)
* [The case schema](../case-schema.md): every setting you can put in a case file.
