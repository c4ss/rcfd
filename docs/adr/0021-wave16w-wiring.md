# ADR 0021: Wave 16W, wiring of existing library features (partitioning F-236, derived fields and sampling F-072, inflation report F-108, heat_flow conductivity validation)

Status: accepted. Everything under "Decisions" and "Registered checks" was written before any wiring code or test existed.
Limits are not changed, widened, replaced or deleted after a run. A miss is reported as measured and the assertion moves to an
ignored `*_known_miss` test with the registered limit intact; the cause is diagnosed, not tuned away. A test set-up defect
gets a NEW corrected test and the old one stays as a record. Addenda (file names, mesh sizes not fixed here) may be appended
at the end without touching a registered limit.

Scope: surface the library features of ADR 0020 (partitioning, derived fields, sampling, inflation) through the case tree,
the CLI and the JSON-RPC service, and close one validation gap (item W). No solver numerics change: nothing here renumbers a
mesh, alters a matrix or touches a field during iterations. Baseline hashes (`crates/rcfd-api/tests/baseline_hashes.rs`)
stay bit-identical and are not re-recorded. Clean-room: published methods and first principles only; no vendor code, text,
data or names. Every expected number is derived below from definitions, closed forms or invariants; no registered check
depends on a literature value.

## Decisions

### D1 Shared-file discipline

The new sections live in new modules, each with its own `schema()` and `check()`: `rcfd-api::decomp` (section
`decomposition`), `rcfd-api::postproc` (section `postprocessing`), `rcfd-api::inflate` (inflation runner). The shared files
get one `.field(...)` line (`schema.rs`), one `out.extend(...)` line (`case.rs`), a short hook in `Simulation::new`
(mesh-dependent checks), `export_vtu` (extra arrays) and `runctl::write_results` (patch files, sample CSVs), plus small
accessors on `Simulation`. The boundary-condition code of the parallel wave is not touched.

### D2 `decomposition` (F-236)

Keys: `method` = `none` (default) | `bisection` | `inertial` | `sectors` | `graph`; `parts` (int, default 1); `rule`
(bisection: `longest` default | `cycle` | `x` | `y` | `z`); `sector_kind` (sectors: `circumferential` default | `radial`);
`origin` (default `[0,0,0]`), `axis` (default `[0,0,1]`), `reference` (default `[1,0,0]`) for sectors; `weights.mode` =
`none` (default) | `box` with `weights.box_min`, `weights.box_max`, `weights.inside`, `weights.outside` (cell weight
`inside` when the cell centre lies in the closed box, else `outside`; both default 1); `tolerance` (graph and smoothing,
default 0.03), `max_passes` (default 20); `smooth` (bool, default false: apply `smooth_partition` after the strategy);
`reorder` (bool, default false: the report adds the bandwidth of the natural and of the reverse Cuthill-McKee ordering);
`write_field` (bool, default false: `rcfd run` writes the part id as the cell array `partition` in the VTK files).

`method = none` is inert. **The solver runs in one process and ignores `decomposition`** for its numerics: the section is
used by `rcfd partition`, the service method `mesh.partition` and `write_field`; the schema documentation and
`docs/case-schema.md` say so (this is the lesson of the silent `heat_flow` conductivity key, item W). `reorder` only
REPORTS bandwidth; the mesh and the solvers are never renumbered (that would change summation order and the baseline
hashes, as ADR 0020 A1 already ruled out). `bisection` uses `Bisection { rule, weights }`, `inertial` uses
`Bisection { Inertial }`, `sectors` uses `Sectors`, `graph` uses `GraphPartitioner { weights, tolerance, max_passes }`.

Report (`decomp::run(case, mesh) -> PartitionReport`, serialised for CLI and service): `method`, `n_parts`, `n_cells`,
`part_cells` (cells per part), `part_weights`, `imbalance` (`weighted_imbalance`, which equals the count imbalance for unit
weights), `edge_cut` (`partition::edge_cut`), `ghost_cells` (per part, from `Decomposition::from_assignment`), `smoothed_moves`
(when `smooth`), `bandwidth` `{natural, rcm}` (when `reorder`), and `part` (the cell-to-part vector; omitted from the CLI
JSON unless `--parts-vector` is given, always present in the Rust struct).

Errors are located `Diagnostic`s. Static checks (`decomp::check`, run by `semantic_errors`): `parts < 1`
(`decomposition.parts`); `method = none` with `parts > 1` (`decomposition.method`); `sectors` with a zero `axis`
(`decomposition.axis`) or a `reference` parallel to the axis (`decomposition.reference`); `weights.mode = box` with
`box_min > box_max` in any component (`decomposition.weights.box_max`); `weights.inside <= 0` or `weights.outside <= 0`
(`decomposition.weights.inside` / `.outside`); `tolerance < 0` (`decomposition.tolerance`). Mesh-dependent check (in `run`
and in `Simulation::new`): `parts > n_cells` (`decomposition.parts`).

### D3 `rcfd partition <case|mesh> [--method m] [--parts n] [--rule r] [--weights-box ...] [--smooth] [--reorder] [--out f.vtu] [--json]`

Reads a case (or a mesh file/directory with CLI-given settings; a mesh file needs `--method`), overrides the tree with the
given options (the overridden tree is validated like a case file; invalid combination exit code 3), prints the report
(text or JSON) and, with `--out`, writes a `.vtu` with the cell array `partition`. Exit codes follow the existing table (0,
1 runtime, 2 usage, 3 invalid case). JSON-RPC: `mesh.partition` (session; optional `method`, `parts`, `rule`, `smooth`,
`reorder` overrides) returns the same report JSON. MCP tools are NOT added: the MCP test asserts the tool catalogue and a new
tool is a separate decision (the service method is available to any JSON-RPC client).

### D4 `postprocessing` (F-072)

* `derived`: list, **opt-in** (default empty: default `solution.vtu` files do not change). Names: `vorticity` (cell vector
  `vorticity`), `q_criterion` (cell scalar `q_criterion`), `total_pressure` (cell scalar `p_total`), `mach` (cell scalar
  `mach`), `wall_shear` (patch-face vector `wall_shear`), `wall_y_plus` (patch-face scalar `wall_y_plus`). Names do not
  clash with the existing arrays `y_plus` (turbulence, cell), `mach` (density-based and variable-density engines),
  `vorticity_z` (a report/probe field, not an array). The derived list is authoritative: listed arrays are written even if
  `output.fields` would filter them.
* Cell arrays go into `solution.vtu` and snapshots (`export_vtu`); `wall_shear` / `wall_y_plus` are patch-face data and are
  written with `write_vtp` as `<dir>/wall_<patch>.vtp` for each patch in `postprocessing.wall_patches` (default: every
  patch of kind wall) at the end-of-run export (`write_results`). The VTP carries both arrays when both are listed.
* Library calls (no new numerics): `derived::velocity_gradient` with `velocity().boundary_values(geom)`, `vorticity`,
  `q_criterion`, `wall_shear(geom, patch, u, u_wall = boundary velocity array, rho, nu)`, `y_plus`,
  `total_pressure_incompressible`, `total_pressure_isentropic`.
* Physics restrictions, each a located error (path `postprocessing.derived` unless stated):
  - `wall_shear` / `wall_y_plus`: laminar constant-viscosity incompressible engines only. Rejected when
    `turbulence.model != laminar` (the library estimate uses the laminar `nu`, not the wall-function shear), when
    `fluid.density` is `ideal_gas` or `real_gas`, when `solver.algorithm = compressible` (boundary values of those engines are
    the owner-cell state, which would give zero shear), and when a non-Newtonian `rheology` law or a temperature-dependent
    `properties.viscosity` is set. `rho = physics.rho`, `nu = physics.nu`.
  - `mach`: only for the density-based (`compressible`) and variable-density engines, where the engine's own `mach` array is
    written unchanged; for incompressible runs it is an error (no sound speed is defined).
  - `total_pressure`: incompressible engines use the DYNAMIC pressure `rho * p + 0.5 rho |u|^2` (the solver pressure `p` is
    kinematic, so the kinematic value is multiplied by `physics.rho` before calling `total_pressure_incompressible`); the
    ideal-gas variable-density engine uses `total_pressure_isentropic(p_abs, mach, fluid.gamma)` with the engine's `p_abs`
    and `mach`; it is an error for `real_gas` (the isentropic ideal-gas relation does not apply) and for the density-based
    engine (its pressure convention is not asserted here).
  - Unknown names: error listing the known names.
  - `postprocessing.wall_patches`: every name must be a patch of the mesh (`postprocessing.wall_patches`, checked when the
    mesh exists, in `Simulation::new`, so `rcfd validate` reports it) and of kind wall.
* `samples.<name>`: `type` = `line` (default) | `plane`; line: `p0`, `p1`, `points` (int, default 50, at least 2);
  plane: `origin`, `e1`, `e2` (orthogonal directions), `extent1`, `extent2`, `n1`, `n2` (default 10, at least 1); `fields`
  (list; default `["speed", "p"]`; any `Simulation::field` name plus `vorticity_x`, `vorticity_y`, `vorticity_z`,
  `q_criterion`); `gradient` (bool, default true: `phi_c + grad_c . (x - x_c)` with the least-squares gradient built from
  the cell values and, where the engine has them, the boundary values; where a field has no boundary values the cell value
  is used). Output `<dir>/sample_<name>.csv` at the end-of-run export, in the `SampleSet::to_csv` format. Pressure sampled
  is the solver pressure (kinematic for incompressible engines); the CSV header names the field `p` unchanged.
  Located errors: `points < 2` (`postprocessing.samples.<n>.points`), `p0 == p1` (`...p1`), `e1`/`e2` zero or not
  orthogonal (`...e2`), `n1`/`n2 < 1`, unknown field name (`postprocessing.samples.<n>.fields`), `T` without
  `energy.enabled`.
* CLI `rcfd sample <case> [--line x0,y0,z0:x1,y1,z1 [--points n]] [--plane ox,oy,oz:e1:e2:ext1:ext2:n1:n2]
  [--fields a,b] [--no-gradient] [--max-iterations n] [--out file-or-dir]`. **A sample needs a solution and the repository has
  no reader for solution files, so `rcfd sample` builds the simulation and runs it** (`Simulation::run(n)`; default `n` =
  `solver.max_iterations`, `--max-iterations 0` samples the initial field), then evaluates the ad-hoc sample given by the
  options or, without options, every `postprocessing.samples.*` of the case. One sample goes to `--out` as a CSV file, several
  to a directory. Checkpoint loading is not used (it supports only steady SIMPLE/SIMPLEC without turbulence; this limitation
  is stated, not worked around). JSON-RPC: `field.sample` (session, `type`, geometry and `fields`, `gradient`) on the live
  session's state returns `{names, inside, csv, n_samples, n_inside}`.

### D5 `rcfd inflate` (F-108)

`rcfd inflate <case|mesh> --patch p --layers n (--first h | --uniform total | --aspect a | --last-ratio r --first h)
[--growth g] [--y-plus y --u-tau u --nu nu] [--out f.vtu] [--json]` calls `inflate_patch`. A y+ target is converted with
`first_cell_height_from_y_plus(y, u_tau, nu)` and used as `first`. Output keys: `patch`, `sizing` (the serialised
`LayerSizing`), `cells_before`, `cells_after`, `report` (the serialised `InflateReport`). Exit codes: 2 usage (missing or
inconsistent options, zero layers), 1 library error (unknown patch, periodic mesh, invalid growth), 4 when the inflated mesh
fails its quality check (the report is still printed). JSON-RPC: `mesh.inflate` on the session mesh returns the same JSON.
Not a case-tree section: inflating changes the mesh and the volume fill into a fluid domain is F-234, so a case key would
promise a solver mesh that this wave does not build.

### D6 Item W: `reports.<n>.conductivity` with the variable-density solver

The variable-density `heat_flow` evaluation (`sim.rs`, `Engine::VarDensity`) calls `heat_flow_out(patch)`, which takes the
conductivity from the engine (`Pr`-based) and never reads `reports.<n>.conductivity`. Decision: **a located validation
ERROR** at `reports.<n>.conductivity` when `fluid.density` is `ideal_gas` or `real_gas`, the report type is `heat_flow` and
the value is greater than 0 ("the variable-density solver takes the conductivity from fluid.* and ignores this key; remove
it"). `Diagnostic` has no warning level and an error cannot be missed. Value 0 (unset) stays valid. Constant-density runs
and the compressible solver are unchanged (the compressible solver already rejects `heat_flow`). The schema documentation of
the key is corrected (it previously said 0 falls back to `physics.conductivity` without mentioning that variable-density
runs ignore the key). No existing case or test uses the combination (searched in `cases/`, `crates/*/tests`).

## Derivations and constants used by the checks

* **Partition count balance.** For the unweighted `bisection`/`graph` strategies the part sizes differ by less than one cell
  from `N/n`, so imbalance `<= 1/floor(N/n)`; the sector strategies cut equal-count bands (difference `<= 1`, same bound).
  The test meshes have `N >= 400` and `n <= 16`, so the bound is `<= 1/25 = 0.04 < 0.05`, the library's registered limit.
  The graph refinement only moves cells into parts below `(1 + tolerance) W/n` with `tolerance = 0.03` and never lowers
  a part below one cell, so its imbalance is `<= max(seed, 0.03 + 1/(W/n))`; with `W/n >= 25` that is `<= 0.07`: the bound
  used for `graph` is therefore the registered 0.05 *plus the observation*, not a derivation. If the graph partitioner
  exceeds 0.05 on a case mesh this is a miss to be reported (the library's own tests got 0.030 worst case).
* **Test meshes.** `Q1`: case generator `box_2d`, `nx = 40, ny = 10, lx = 4, ly = 1` (400 cells). `Q2`: case generator
  `cylinder_ogrid` with the parameters fixed in the test file and recorded in an addendum (at least 400 cells, curved).
  `Q3`: `box_2d`, `nx = 40, ny = 4` (160 cells; `n <= 8`, bound `1/20 = 0.05`, only for the reorder bound).
* **Reorder bound.** For a 2D grid of `n1 x n2` cells (`n1 >= n2`) ADR 0020 A2 gives RCM bandwidth `<= 2 n2 - 1`; for
  `Q3` that is 7.
* **Linear velocity test field** (box_2d, `initial.velocity_expression_x = "1 [1/s] * x + 2 [1/s] * y"`,
  `..._y = "3 [1/s] * x - 1 [1/s] * y"`, no iteration is run): `du/dx = 1, du/dy = 2, dv/dx = 3, dv/dy = -1`; vorticity
  `z = dv/dx - du/dy = 1`, `x = y = 0`; `Q = -0.5 sum_kj A_kj A_jk = -0.5 (1 + 1 + 2 * 2 * 3) = -7`. The least-squares
  gradient is exact for a linear field only when every neighbour value is the field value, which holds for cells without a
  boundary face (the boundary values are the boundary conditions, not the field). The analytic comparison is therefore
  restricted to cells that have no boundary face, stated in each check.
* **Couette wall shear** (box_2d `lx = 2, ly = 0.5, nx = 8, ny = 10`, `physics.rho = 1.2`, `physics.nu = 0.05`, lower wall
  at rest, upper wall `U = 3` in x, initial field `ux = U y / ly` evaluated at cell centres, no iteration): the first-cell
  difference quotient `(u_c - u_wall)/dn` is exact for a linear profile, so `|tau| = rho nu U / ly = 1.2 * 0.05 * 3 / 0.5 =
  0.36` on both walls (direction `+x` on the lower wall, `-x` on the upper wall: force of the fluid on the wall in the
  library's sign convention, `rho nu (u_c - u_wall)/dn`), and `y+ = dn sqrt(|tau|/rho)/nu` with `dn = ly / (2 ny) = 0.025`:
  `y+ = 0.025 * sqrt(0.3) / 0.05 = 0.27386127875258306`.
* **Total pressure, incompressible**: `rho = 1.2`, `initial.pressure = 3` (kinematic), uniform velocity `(2, 0, 0)`
  (no expression): `p_total = 1.2 * 3 + 0.5 * 1.2 * 4 = 6.0`.
* **Inflation y+ target**: `first_cell_height_from_y_plus(0.8, 0.05, 1e-5) = 3.2e-4` (ADR 0020 I6).
* **VTK round trip**: the writer prints floats with `{:?}` (shortest representation that parses back exactly), so values
  parsed from a `.vtu`/`.vtp`/CSV equal the library output bit for bit. All VTK/CSV value comparisons are therefore EXACT
  (difference 0.0), not tolerance based. The test files contain a small parser (no VTK reader exists in the repository).

## Registered checks

Test files: `crates/rcfd-api/tests/decomposition.rs`, `postprocessing.rs`, `inflate_report.rs`, `report_conductivity_vd.rs`;
`crates/rcfd-cli/tests/wiring.rs` (runs the built binary); service checks in `decomposition.rs` / `postprocessing.rs`.

### Item W (heat_flow conductivity, variable density)

* **W1** an ideal-gas variable-density case (taken from `tests/variable_density.rs`) with a `heat_flow` report and
  `conductivity = 2.0` fails `Case::parse_toml` with a diagnostic whose path is exactly `reports.<n>.conductivity`, which
  carries the TOML line of that key (exact line number).
* **W2** positive controls: the same case without the key, and with `conductivity = 0.0`, has no diagnostic; a
  constant-density case with `conductivity = 2.0` still validates (`report_conductivity.rs` passes unmodified).
* **W3** `rcfd validate` on the failing case exits 3 and prints the path.

### F-236 decomposition

All on `Q1` and `Q2`, methods `bisection` (rules longest, cycle, x, y, z), `inertial`, `sectors` (radial and circumferential
about the z axis, origin at the mesh centre), `graph`, `n` in `{2, 3, 5, 8, 16}`.

* **D1 equals the library**: the part vector of `decomp::run` equals the direct library call (`Bisection`, `Sectors`,
  `GraphPartitioner` with the same parameters) element for element: 0 differences in every case.
* **D2 cover**: length `N`, every id `< n`, every part non-empty, `sum(part_cells) == N` exactly, `Decomposition::from_assignment`
  succeeds.
* **D3 balance**: report `imbalance` equals `partition::weighted_imbalance` bit for bit and is `<= 0.05` for every method,
  mesh and `n`.
* **D4 edge cut**: report `edge_cut` equals an independent count of internal faces whose cells differ in part (exact);
  `edge_cut(graph) <= edge_cut(bisection longest)` for every mesh and `n` (the roadmap criterion, exact integers).
* **D5 weights**: `weights.mode = box` (`inside = 4`, `outside = 1`, box covering the half `x > mid` of Q1/Q2):
  `bisection`, `inertial`, `graph` give weighted imbalance `<= 0.05` for `n` in `{2, 4, 8, 16}`; the part vector equals the
  direct call with the same weight vector (0 differences); positive control: the same weights with `method = bisection`
  but `weights.mode = none` has weighted imbalance `> 0.05` for at least one `n`.
* **D6 smoothing**: `smooth = true` equals `partition` followed by `smooth_partition` with the same parameters (0
  differences); cut after `<=` cut before; cover intact.
* **D7 reorder**: `bandwidth.natural` and `bandwidth.rcm` equal the library values computed from the mesh internal-face
  list (exact); on `Q3`, `bandwidth.rcm <= 7`; the mesh used by a following run is unchanged (`Simulation` fields equal
  with and without `reorder`: 0 differences after 5 iterations).
* **D8 VTK**: `rcfd partition --out f.vtu` and `rcfd run` with `write_field = true` write a `partition` array that parses
  back to exactly the part vector (0 differences, `N` entries); a default run writes no `partition` array and no derived
  array, and its `solution.vtu` is byte-identical to one produced with `[decomposition] method = "none"` and an empty
  `[postprocessing]` present.
* **D9 CLI JSON**: keys `method, n_parts, n_cells, part_cells, part_weights, imbalance, edge_cut, ghost_cells` present with
  the right types; `sum(part_cells) == n_cells`; with `--reorder` also `bandwidth.natural` and `bandwidth.rcm`; exit code 0.
* **D10 located validation**: each of the errors listed in D2 is returned with exactly the path stated there; TOML cases
  carry a line number. An unknown `method` value reports `decomposition.method` (schema enum). Exit code 3 from the CLI.
* **D11 service**: `mesh.partition` returns a report equal (JSON value) to the library report for the same session case.
* **D12** `baseline_hashes` passes unmodified.

### F-072 derived fields and sampling

* **P1 vorticity and Q through the case tree**: the linear field above on `box_2d` (`nx = 12, ny = 10`) with
  `derived = ["vorticity", "q_criterion"]`; `export_vtu` writes `vorticity` (3 components) and `q_criterion`; parsed values
  equal `derived::vorticity(velocity_gradient(...))` and `q_criterion` of the same inputs exactly (0.0 difference, all
  cells); on cells without a boundary face `|omega_z - 1| <= 1e-12`, `|omega_x|, |omega_y| <= 1e-12`, `|Q + 7| <= 1e-12`.
  At least one such cell must exist (positive control on the selection: count `>= 40`).
* **P2 total pressure, incompressible**: `p_total` is `6.0` in every cell to 1e-12 relative (and equals
  `total_pressure_incompressible(rho * p, rho, u)` exactly); the kinematic-to-dynamic conversion is thus verified.
* **P3 variable density**: an ideal-gas variable-density case (from `tests/variable_density.rs`, a few iterations):
  `derived = ["mach", "total_pressure"]`; `mach` array equals `sim.field("mach")` exactly; `p_total` equals
  `total_pressure_isentropic(p_abs, mach, fluid.gamma)` per cell to 1e-12 relative.
* **P4 wall shear and y+**: the Couette case above, `derived = ["wall_shear", "wall_y_plus"]`, `wall_patches = ["ymin",
  "ymax"]`: `wall_ymin.vtp` and `wall_ymax.vtp` exist, 8 faces each; the parsed arrays equal `derived::wall_shear` /
  `derived::y_plus` exactly; `|tau|` is `0.36` to 1e-12 relative on every face, the x component has sign `+` at `ymin` and
  `-` at `ymax`; `wall_y_plus` is `0.27386127875258306` to 1e-12 relative.
* **P5 located restrictions** (exact paths): `wall_shear` with `turbulence.model = "k_epsilon"`, with
  `fluid.density = "ideal_gas"`, with `solver.algorithm = "compressible"`, with a `rheology` law, and `mach` for an
  incompressible case, `total_pressure` with `real_gas`: each reports `postprocessing.derived`; an unknown name
  reports `postprocessing.derived`; an unknown or non-wall `wall_patches` entry reports `postprocessing.wall_patches`
  (from `Simulation::new`). Positive control: the valid Couette case validates.
* **P6 line sampling**: on the linear field (box_2d `nx = 12, ny = 10`, `lx = 4, ly = 1`), line from `(0.8, 0.3, 0.05)` to
  `(3.2, 0.7, 0.05)` with 20 points (all in cells without a boundary face), fields `ux, uy`, gradient on: CSV header is
  exactly `s,x,y,z,ux,uy,inside`; 20 rows; the CSV text equals the direct library result (`sample_line` + `add_field` with
  the same least-squares gradient) byte for byte; sampled values equal the linear field `x + 2y`, `3x - y` to 1e-12 absolute.
  (The z coordinate lies inside the one-cell-thick mesh; the mesh thickness is read from the mesh, and the line z is set to
  the mid-plane in the test.)
* **P7 plane sampling**: plane `origin = (0.8, 0.3, mid)`, `e1 = x`, `e2 = y`, extents `2.4 x 0.4`, `n1 x n2 = 6 x 4`:
  24 rows, header `x,y,z,ux,uy,inside`, CSV equals the direct library CSV byte for byte, values equal the linear field to
  1e-12.
* **P8 outside points**: a line that leaves the mesh has `inside = 0` and empty value fields exactly for the points the
  library reports outside (count equal to `20 - SampleSet::n_inside`, which is positive).
* **P9 CLI**: `rcfd sample case.toml --max-iterations 0 --line ... --fields ux,uy --out f.csv` writes a file equal to the
  P6 CSV byte for byte; a case with `postprocessing.samples.s1` and `rcfd run` write `sample_s1.csv` in the output directory
  equal to the CLI output for the same sample (byte for byte), when run with the same iteration count; exit codes 0; an invalid
  sample gives exit 3 with the path.
* **P10 service**: `field.sample` returns `csv` equal to the P6 CSV byte for byte.
* **P11 located sample errors**: the errors listed in D4 return exactly the stated paths.
* **P12** default `solution.vtu` unchanged (see D8) and `baseline_hashes` passes unmodified.

### F-108 inflation report

On `box_2d` case (`nx = 8, ny = 4`), patch `ymax`.

* **N1**: `rcfd inflate --first 0.01 --growth 1.2 --layers 6 --json` prints a `report` JSON equal (value equality, exact) to
  the serde value of `inflate_patch(&mesh, "ymax", FirstGrowth{0.01, 1.2, 6}).1` computed directly.
* **N2** keys `patch, sizing, cells_before, cells_after, report` present; `cells_after - cells_before == 8 * 6` exactly.
* **N3 y+ target**: `--y-plus 0.8 --u-tau 0.05 --nu 1e-5 --layers 4 --growth 1.2`: `report.first_height_min` equals `3.2e-4` to
  1e-12 relative.
* **N4** `--out f.vtu`: the file's `NumberOfCells` equals `cells_after`.
* **N5 errors**: unknown patch exits 1 with the patch name in the message; `--layers 0` exits 2; `--first` missing for a
  `first-growth` sizing exits 2; `--growth 0` exits 1.
* **N6 service**: `mesh.inflate` returns a JSON value equal to the N1 output.

### Documentation

`docs/case-schema.md` gains sections for `decomposition` and `postprocessing` and the corrected `conductivity` note;
`docs/mesh-quality.md` links the inflation report; `docs/reference-data.md` is checked for unverifiable references (this
wave introduces none: every number is derived above); `docs/roadmap.json` is updated honestly (F-236 note only, status
unchanged unless every sentence of the roadmap acceptance is met by tests; F-072 and F-108 stay partial unless stated).

## Not attempted (stated in advance)

Multilevel graph partitioning; solver use of partitions or of RCM ordering; MCP tools; case-tree key for inflation; volume
fill (F-234); lambda2 or other vortex criteria; wall-function wall shear; a solution-file reader (so `rcfd sample` runs the
case); iso-clip, offset surfaces, point clouds and user variables (F-238); GUI surfaces.

## Addendum after implementation (no registered limit changed)

Files: `crates/rcfd-api/src/{decomp,postproc,inflate}.rs`, `crates/rcfd-cli/src/cmd_wiring.rs`; tests
`crates/rcfd-api/tests/{decomposition,postprocessing,inflate_report,report_conductivity_vd}.rs`, `crates/rcfd-cli/tests/wiring.rs`.

Measured (release): D1 to D4 worst unit-weight imbalance 0.0278 over 90 cases (limit 0.05); D5 worst weighted imbalance 0.0240
(0.05); D7 natural bandwidth 40, reverse Cuthill-McKee 5 on Q3 (bound 7); P1 worst analytic error 1.4e-14 on 80 interior cells
(1e-12, minimum 40); P2 and P3 0 (1e-12); P4 |tau| 1.4e-15, y+ 2.6e-15 (1e-12); P6 1.8e-15, P7 8.9e-16 (1e-12). All other
registered checks pass as stated; none was a known miss.

Deviations, disclosed:

* Extra check beyond the registered list: `decomposition.write_field = true` with `method = "none"` is a located error at
  `decomposition.write_field` (otherwise the request would silently do nothing).
* P9 compared `rcfd sample` with `rcfd run` using `--max-iterations 1` instead of 0: `rcfd run` rejects 0 iterations
  (`solver.max_iterations` range), so both commands ran the same single iteration; the 0-iteration `rcfd sample` form is
  exercised separately only in the library-level P6/P7 checks, not through the binary.
* Q2 (cylinder o-grid) parameters used: nx = 48, ny = 12, r_in = 1, r_out = 5, grading = 4 (576 cells).
* The D2 derivation of the graph-partitioner imbalance bound is an observation, not a proof; the 0.05 limit was registered
  before running and held.
* `rcfd inflate` exits 4 when the inflated mesh has quality errors (not registered; unexercised by tests).

Not delivered (as registered): MCP tools, case-tree inflation key, volume fill, lambda2, wall-function shear, solution-file
reader, multilevel partitioning, solver use of partitions or RCM.
