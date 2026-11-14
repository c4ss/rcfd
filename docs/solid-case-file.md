# The solid case file

A TOML file of its own for a linear solid model of `rcfd-fem` (F-386, [ADR 0438](adr/0438-f386-registration-solid-case-file.md)).
It is not a flow case: the flow reader (`rcfd run`, `Case::load`) refuses it and the solid reader refuses a flow case. It is read
and run by the library function `rcfd_api::solid_case::run_solid_case_file(path, out_dir)`, which writes the `.vtu` and a JSON
report. There is no CLI subcommand or MCP tool for it yet.

Values are SI numbers or unit strings (`"200 GPa"`, `"1 MPa"`, `"7850 kg/m^3"`) checked against the dimension of the key. Named
tables (`[constraints.<name>]` and the like) are processed in ascending order of their names. Every problem (an unknown key, a
wrong type or unit, a missing key or set, a refusal of the model) is reported with its dotted key and line, all problems of a
file together.

## Tables

| Table | Keys |
| --- | --- |
| top level | `title` |
| `[mesh]` | `source` = `generator` (default) or `file`. Generator: `generator` = `box`, `element` = `hex8` / `tet4` / `tet10`, `origin` (3 lengths, default 0), `lengths` (3 lengths), `divisions` (3 integers). File: `file` (a Gmsh `.msh`, relative to the case file), `quadratic` (upgrade Tet4 to Tet10). Sets of the mesh: `all`; per box face or Gmsh patch a node set and a surface set (`xmin` .. `zmax`, or the physical names); per Gmsh volume group an element set |
| `[node_sets.<name>]` | `box_min`, `box_max`: the nodes inside the box, bounds inclusive |
| `[materials.<name>]` | `element_set` (default `all`), `youngs_modulus`, `poisson_ratio`, `density` (default 0), `expansion` (default 0) |
| `[constraints.<name>]` | `set` (a node set), `components` (`["x", "y", "z"]` or a part), `value` (length, default 0) |
| `[loads.<name>]` | `kind` = `traction` (`set` a surface set, `traction` 3 pressures), `pressure` (`set`, `pressure`), `nodal_force` (`set` a node set, `force` 3 forces per node), `gravity` (`acceleration`, no set) |
| `[analysis]` | `type` = `static` (default) or `modal`; modal: `modes` (default 6), `tolerance` (1e-10), `residual_test` = `relative` / `backward` |
| `[solver]` | static only. `method` = `direct` (default) or `cg`; for `cg`: `preconditioner`, `fill_level` (default 1) and `ordering` = `natural` / `node_rcm` (for `ic_fill` only), `tolerance` (1e-10), `max_iterations` (20000) |
| `[output]` | `vtu` (default `result.vtu`), `report` (default `report.json`); empty = not written |

Preconditioners of `method = "cg"`: `block_jacobi` and `ic0` (ADR 0282), `ic_fill` (the default, ADR 0357), `amg` (ADR 0305),
`amg_quadratic` (ADR 0370) and `amg_quadratic_smoothed` (ADR 0375) for Tet10 meshes.

## Example

A cantilever with the root held as beam theory assumes (from `crates/rcfd-api/tests/solid_cases/cantilever_tet10.toml`):

```text
[mesh]
element = "tet10"
lengths = [2.0, 0.1, 0.1]
divisions = [40, 2, 2]

[node_sets.root_n0]
box_min = [-1e-9, -1e-9, 0.049999999]
box_max = [1e-9, 1e-9, 0.050000001]

[materials.steel]
youngs_modulus = "200 GPa"
poisson_ratio = 0.3

[constraints.root_x]
set = "xmin"
components = ["x"]

[constraints.root_n0]
set = "root_n0"
components = ["y", "z"]

[loads.tip]
kind = "traction"
set = "xmax"
traction = [0.0, 0.0, "-100 kPa"]
```

## Outputs

- Static: the `.vtu` of `rcfd_post::solid_output::write_solid_vtu` (displacement, nodal stress, von Mises, principal stresses
  and directions, reactions; element stress). Modal: the mesh with one point array `mode_<k>` per mode.
- The report (`format` = `rcfd-solid-report/1`): `mesh` counts, `unknowns`; static: `displacement` (largest magnitude and its
  node, componentwise minimum and maximum), `stress` (largest nodal and element von Mises), `reactions` (per constraint the sum
  of the reaction vectors over its set's nodes, and `total`), `solver` (name, iterations, restarts, relative residual, shift,
  breakdowns); modal: `frequencies` (Hz), `eigenvalues`, `solver` (iterations, shift, residuals); `outputs` (paths written).
