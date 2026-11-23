# The solid case file

A TOML file of its own for a linear solid model of `rcfd-fem` (F-386, [ADR 0438](adr/0438-f386-registration-solid-case-file.md)).
It is not a flow case: the flow reader (`rcfd run`, `Case::load`) refuses it and the solid reader refuses a flow case. It is read
and run by the library function `rcfd_api::solid_case::run_solid_case_file(path, out_dir)`, which writes the `.vtu` and a JSON
report. There is no CLI subcommand or MCP tool for it yet. A case with one of the two tables of
[reduced parts and superelements](#reduced-parts-and-superelements) is run by `run_solid_case_any(path, out_dir)`, which
runs every solid case file.

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
| `[superelements.<name>]` | `file` (a reduced part file, relative to the case file), `set` (the node set of the model the part attaches to); both required |
| `[reduction]` | `interface` (a node set, required), `modes` (fixed-interface modes kept, default 0 = static condensation), `interface_enrichment` (Krylov blocks, default 0 = none), `file` (the part file written, default `part.cms`) |

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

## Reduced parts and superelements

Two tables, both off unless given (F-616, [ADR 0785](adr/0785-f616-registration-case-file-keys-for-superelements-and-interface-enrichment.md));
a case without them runs as before, bit for bit.

**A part case** has a `[reduction]` table. Its model is built as any case's and reduced to the node set `interface` by
the fixed-interface reduction of [ADR 0469](adr/0469-f430-registration-substructuring-craig-bampton.md) with `modes`
modes; `interface_enrichment = p` adds `p` Krylov blocks of the interface enrichment of
[ADR 0532](adr/0532-f603-registration-craig-bampton-interface-enrichment.md) (one block is the residual flexibility of
the omitted modes for the inertia load of the constraint modes). Nothing is solved: the run writes the part file and a
report with `analysis` = `reduction` and the sizes of the part. `[analysis]`, `[solver]`, `[superelements.<name>]` and
`output.vtu` do not apply and are refused. The constraints of a part must have the value 0.

```text
[mesh]
element = "hex8"
lengths = [0.5, 0.1, 0.08]
divisions = [8, 2, 2]

[materials.steel]
youngs_modulus = 2.1e11
poisson_ratio = 0.3
density = 7850.0

[constraints.clamp]
set = "xmin"
components = ["x", "y", "z"]

[reduction]
interface = "xmax"
modes = 5
interface_enrichment = 1
file = "root.cms"
```

**A host case** names part files in `[superelements.<name>]` tables. Each part is attached to the model at the nodes of
`set` as a superelement ([ADR 0536](adr/0536-f553-registration-reduced-parts-as-superelements.md)): every boundary node of
the part must lie on a node of the set (within 1e-9 of the largest extent of the mesh) and every node of the set must be a
boundary node of the part. The parts are attached in ascending order of their names; the unknowns of the combined model
are the model's, then the generalised coordinates of each part. The loads the part was reduced with act on the combined
model.

```text
[superelements.root]
file = "root.cms"
set = "xmin"
```

- `[analysis] type = "static"` solves the combined model directly (`solver.method` must be `direct`); `type = "modal"`
  gives its lowest modes with the `modes`, `tolerance` and `residual_test` of the table.
- The report has `superelements` (per part: `name`, `file`, `set`, `boundary_unknowns`, `generalised_coordinates`,
  `offset`), `unknowns` of the combined model, and for a static run the `displacement` of the model's own nodes, for a
  modal run `frequencies` and `eigenvalues`. The `.vtu` is the model's mesh with `displacement` or `mode_<k>`.
- Not available in a run with superelements: stresses and reactions, the displacements inside a part, a CG solver, and
  a harmonic analysis (the case file has none). The node sets where parts attach must be free of constraints, and every
  constraint of the model must have the value 0.
- `run_solid_case_file` refuses a case with one of the two tables and names `run_solid_case_any`.
