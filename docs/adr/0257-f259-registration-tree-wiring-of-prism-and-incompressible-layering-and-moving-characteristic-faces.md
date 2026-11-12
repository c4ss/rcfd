# ADR 0257: F-259 registration: the case tree runs prism and incompressible layering, characteristic conditions on moving faces and the mesh-speed time step

Status: **REGISTERED 2026-11-12**, written before any code of this part of F-259 and before any run of its checks. Not yet
accepted by the maintainer. Before this text nothing was run and nothing was compiled: the code and the documents listed
under "What was read" were read, and the arithmetic under "Arithmetic before this registration" was done by hand. Results
go in the addendum at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0191](0191-f259-registration-case-tree-and-cli-wiring-of-layering-and-compressible-ale.md) (F-259, first
part: the tree runs ADR 0090's library; its refusals E5 and E6, its test L, its W7 "not built", its decision record of
2026-11-06), [ADR 0207](0207-f258-registration-compressible-ale-boundaries-and-mesh-speed-time-step.md) (F-258: moving
characteristic faces, `max_dt_ale`; its Q1 and Q2), [ADR 0213](0213-f257-registration-prism-layering-and-incompressible-layering.md)
(F-257: `ExtrudedStack`, `PimpleSolver::replace_mesh`, the set-ups P-Δ, G5L and G5X; its Q1),
[ADR 0155](0155-job48u-rotate-the-mesh-instead-of-the-flow.md) (optional tables),
[ADR 0223](0223-f301-registration-consistent-piso-correctors-moving-mesh-lag-estimate-case-key-and-benchmarks.md) (an
optional table added with no hash table or golden edited, its check K0). Roadmap item F-259 (depends on F-230 and F-063).

Number: the lead gave 0257 to this part of F-259. Branch `f259-tree-wiring` from main 2e0eefd0, worktree `w49i-work/f259`.

House rules (the campaign's rules of 2026-11-12, ADR 0130, ADR 0191):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the number recorded, and the cause is
  diagnosed, not tuned away; a defective set-up gets a corrected test by an amendment written and committed before the
  corrected run, and the old one stays as an ignored record;
- defaults stay bit-identical: a case that does not ask for the new behaviour gives the bytes it gave before;
- `crates/rcfd-api/tests/baseline_hashes.rs`, the hash tables (`case_hashes_w48u.rs`, `w48v_case_hashes.rs`) and the goldens
  are not edited or re-recorded;
- each registered run is made once, and nothing is re-run to pick a result;
- no new third-party crate, no unsafe code, no commit trailers, no push;
- clean-room: nothing here is a new algorithm. The numerics are those of ADR 0090, ADR 0207 and ADR 0213 (derived there
  from first principles); this item only calls them from the case tree.

## What F-259 still asks

The acceptance (roadmap, binding): case-tree and CLI wiring of layering and compressible ALE; invalid keys give located
errors; a tree run equals the library run to 1e-12; a case without the keys is bit-identical (baseline hashes unchanged).
ADR 0191 wired the library of ADR 0090. Since then F-257 (ADR 0213) and F-258 (ADR 0207) built the library work that ADR
0191 W7 lists as "not built": prism layers, layering on the PISO/PIMPLE path, characteristic conditions on moving faces
and the mesh-speed time step. This registration wires them.

**Decisions that bind this work** (the lead's brief of 2026-11-12):

- ADR 0207 Q1 (a) and ADR 0213 Q1 (a) are taken on the maintainer's standing instruction of 2026-11-11 (take the
  recommendation): F-258 and F-257 are wired into the tree under F-259. The three E5 inputs of ADR 0191's test L, which
  assert the old refusal, become an ignored record: the test `l_located_errors` is marked ignored with the reason and is
  otherwise not edited; a new test asserts the new behaviour and repeats its other inputs.
- ADR 0207 Q2 (b) is **not** taken: the tree's moving compressible step keeps `dt = max_dt()`. The mesh-speed step is
  offered only as an opt-in key whose absence leaves every existing case bit-identical (ADR 0207 Q2 (c)).
- A key is added only if no registered hash table or golden needs an edit (established by reading, below).

## What was read (main 2e0eefd0, read only)

1. **`crates/rcfd-api/src/dynmesh.rs`, `comp.rs`, `sim.rs`, `schema.rs`** (the dynamic-mesh parts). `check_compressible`
   refuses a characteristic `compressible_bc` on a patch that can move (E5). `check_layering` and `DynMesh::from_case` refuse
   layering without the compressible solver (E6). `Layers` holds a `LayerStack` and compares its mesh with the case's bit
   for bit. `CompState::advance_moving` takes `dt = max_dt()`. `DynMesh::step` (PISO/PIMPLE) moves the points with
   `new_points`, then `move_mesh`, `set_follow_conditions`, `set_rest_conditions`, `step_ale_with_walls`; `Simulation::step`
   sets `Simulation.geom` from the solver after it. `box_3d` is `gen::box_mesh`. `Simulation::pimple_solver()` gives read
   access to the transient solver (its `settings`, `u`, `p`, `phi` and `geom` are public).
2. **The library** (`rcfd-mesh` `layering`, `rcfd-fv` `compressible`, `pimple::moving`): `ExtrudedStack::uniform(section,
   n, length, a_min, a_max)` with `Section::rect` and `Section::rect_triangles`, `points`, `mesh`, `set_piston`, `adjust`,
   `face_sources`; `remap`, `remap_face_flux`; `PimpleSolver::replace_mesh(geom, u, p, phi)`, which refuses a time scheme
   other than implicit Euler and a set face viscosity, body force, source force, sink, face jump or immersed forcing;
   `CompressibleSolver::max_dt_ale()` (`max_dt()` when there are no kept mesh speeds: before the first ALE step and
   after a `replace_mesh`); `step_ale` accepts the four characteristic conditions on a moving face since ADR 0207.
3. **Whether a key can be added without a hash-table or golden edit.**
   - `Schema::hash_omit_default` (ADR 0156 D6) keeps the settings hash, but the key is written by `Case::to_toml` and
     `to_json_string`, so H1 (`case_hashes_w48u.rs`) needs an entry in its `DEFAULT_KEYS` for every such key (its header:
     ADR 0163, 0181, 0183 P-AB, 0190, 0193 each edited that table). That is a hash-table edit: **not available here**.
   - `Schema::optional` (ADR 0155; `rcfd-core/src/settings.rs`, the unit test
     `optional_objects_stay_out_of_the_data_until_set`): an optional object that the input does not write stays out of the
     data, so the settings hash, the TOML text and the JSON text of every existing input are unchanged; `get` of a path
     into the absent object gives the schema defaults. H1 compares exactly those three and the mesh hash; its
     `DEFAULT_KEYS` holds no optional table (`mesh.transform` and `solver.consistent_correctors` are optional tables and
     are not in it). `baseline_hashes.rs` hashes solver results, not the schema. The goldens are the MCP tool lists and
     `project_shapes.json` (`crates/rcfd-mcp/tests/golden`); none of them holds a case-schema key (no `rotate_z_deg`,
     `consistent_correctors` or `dynamic_mesh` in them). No file holds the exported JSON Schema of the case. ADR 0223 added
     the optional table `[solver.consistent_correctors]` and its K0 held with no test edited.
   - **Conclusion (by reading): new keys can be added as optional tables with no edit of a hash table or a golden.** The
     keys of this registration are optional tables. K0 below is the check that the reading was right; if K0 fails, the
     tables are taken out again and the schema step is reported as the step that needs the maintainer.
4. **The tests that assert the old refusals**: only `l_located_errors` of `crates/rcfd-api/tests/layering_ale_f259.rs` (its
   three E5 inputs). Its E6 input (P-T with `piso`) stays a diagnostic at `dynamic_mesh.deformation` under this design (D2).
   `crates/rcfd-cli/tests/layering_ale_f259.rs` X2 asserts E8 and E2, which stay.
5. **The incompressible transient step of `Simulation::step`**: with a dynamic mesh the static checks already refuse
   turbulence, energy, statistics, scalars, particles, sources, rheology, restart and the field store. Objects sized to
   the mesh that are still possible: velocity-expression conditions (`expr_bcs`), the boundary-condition catalogue,
   lumped components (their face jumps), a Marangoni stress, user functions.

## Arithmetic before this registration (disclosed)

- **G5L's layer events** (ADR 0213 I): piston displacement `t^2` with `a0 = 2`, `h0 = 0.05`, removal when the piston layer
  is below `0.4 h0`: at displacements `0.05 m - 0.02`, `m = 1..7` to `t = 0.6` (displacement 0.36). ADR 0213 measured 7
  removals, and 7 additions on G5X.
- **The mesh-speed step at the P-X piston** (ADR 0191 P-X: 20 layers of `h0 = 0.05`, cross-section `0.1 x 0.1`, one cell;
  peak piston speed `0.3 pi / 4 = 0.236`, Mach 0.2 with `a = 1.18`). In the piston cell of height `h` the sum of
  `(|u.n - w_n| + a) |S|` over the faces is about `(a + 1.1 a) 0.01 + 4 a (0.1 h)` with the mesh speed (the gas next to the
  piston moves with it) and `(1.2 a + 1.1 a) 0.01 + 4 a (0.1 h)` without; with `h = 0.03` that is `0.033 a` against
  `0.035 a`, a step larger by about 6 percent while the piston cell is the smallest cell and the piston is fast. Over the
  stroke the piston cell is the smallest cell roughly 60 percent of the time and the piston speed varies as a sine, so the
  step count with the mesh-speed step is expected lower by 1 to 4 percent than the 625 steps ADR 0191 measured (X1).
  The control DT-n below is set at 0.2 percent.
- **The set-up T-U**: translation speeds up to `0.05 x 2 pi = 0.31` in x and `0.02 x 4 pi = 0.25` in y, so every patch of
  the box sees a normal mesh speed above 0.02 at some step (U-set).

## Design

**D1. Characteristic conditions on moving faces (F-258).** The refusal E5 is removed from `check_compressible`: a
`total_inlet`, `pressure_outlet`, `nonreflecting` or `farfield` patch may move under `rigid`, `smoothing` and `layering`.
No key. The step is `step_ale` as before (ADR 0191 W4), which since ADR 0207 evaluates those conditions in the frame of
the face (ADR 0207 M3); a face whose swept volume is exactly 0 keeps its static path. No existing valid case changes: the
cases E5 refused did not run.

**D2. The cross-section of `box_3d` (F-257).** An optional table `[mesh.box_3d]` with one key:

- `section`: `"box"` (default), `"rectangles"` or `"triangles"`.
  - `"box"`: `gen::box_mesh`, as before (ADR 0191 W3), bit for bit.
  - `"rectangles"`: the mesh of `ExtrudedStack::uniform(Section::rect([ny, nz], [ly, lz]), nx, lx, 0.4, 1.6)`: the same
    hexahedra as `"box"` in the face order of the extruded stack.
  - `"triangles"`: the same with `Section::rect_triangles` (each rectangle of the cross-section cut into two triangles):
    `2 ny nz` prisms per layer.
  The patches are `xmin, xmax, ymin, ymax, zmin, zmax` (walls) for all three. `"rectangles"` and `"triangles"` need
  `nx >= 2` (located at `mesh.nx`).
- Layering takes the stack that the section names: `"box"` the `LayerStack` of ADR 0191 (compressible only), the other two
  the `ExtrudedStack` (compressible and PISO/PIMPLE). The comparison of the stack's mesh with the case's, bit for bit,
  stays (ADR 0191 W3).

**D3. Layering on the PISO/PIMPLE path (F-257).** `deformation = "layering"` with `solver.algorithm = "piso"` or
`"pimple"` is accepted when `mesh.box_3d.section` is `"rectangles"` or `"triangles"`. The piston is `xmax`
(`moving_patches = ["xmax"]`), at `lx + d_x(t)`. The step is ADR 0213's loop: `set_piston(lx + d_x(t + dt))`,
`ale::move_mesh(geom, stack.points(), dt)`, `set_follow_conditions` on the following patches, `step_ale_with_walls`; then
`adjust`, and on an event the new mesh, `face_sources`, the volume-weighted `remap` of `u` and `p`, `remap_face_flux` of
`phi` and `PimpleSolver::replace_mesh` (ADR 0213 M3, M4). `Simulation.geom` follows the solver, as before. A failure of
any of these stops the run with the `dynamic_mesh: step N, t = ...:` message (ADR 0033 D8).

**D4. The mesh-speed time step (F-258), opt-in.** An optional table `[dynamic_mesh.compressible]` with one key:

- `time_step`: `"absolute"` (default) or `"mesh_speed"`. With `"mesh_speed"` the moving compressible step takes
  `dt = max_dt_ale()` (ADR 0207 M1, M2: the mesh speeds of the last ALE step; `max_dt()` on the first step and on the step
  after a layer event), clipped to the end time as before. With `"absolute"`, or without the table, `dt = max_dt()`: the
  same operations as before.

**D5. Located errors** (static unless marked "build"):

- N1 an unknown value of `mesh.box_3d.section` or of `dynamic_mesh.compressible.time_step`: the parse error at that path;
  an unknown key inside either table: a parse error whose path starts with the table's path.
- N2 `mesh.box_3d.section` other than `"box"` with a generator other than `box_3d` or with `mesh.source = "file"`: at
  `mesh.box_3d.section`.
- N3 `section` other than `"box"` with `mesh.nx < 2`: at `mesh.nx` (static when layering is on, otherwise build, with the
  mesh).
- N4 (E6 kept) layering with `piso` / `pimple` and `section = "box"`: at `dynamic_mesh.deformation`, naming
  `mesh.box_3d.section`. Layering with any other algorithm than `compressible`, `piso`, `pimple`: at
  `dynamic_mesh.deformation`.
- N5 `time_step = "mesh_speed"` with `dynamic_mesh.enabled = true` and a solver other than `compressible`: at
  `dynamic_mesh.compressible.time_step`. (Like every `dynamic_mesh` key, it is inert while `enabled = false`.)
- N6 (build) layering with `piso` / `pimple` together with an object sized to the mesh (read 5: a velocity expression on
  a patch, a catalogue condition, lumped components, a Marangoni stress, user functions): at `dynamic_mesh.deformation`.
- ADR 0191's E1 to E4 and E7 to E13 are unchanged. E7 (`mesh.generator` not `box_3d`), E8 (`moving_patches` other than
  `["xmax"]`), E9 to E12 apply to layering with either solver. The incompressible checks of ADR 0033 (time scheme,
  following patches with a fixed velocity, ...) apply to incompressible layering as to every incompressible moving mesh.

**D6. Not changed.** `rcfd-fv`, `rcfd-mesh`, `rcfd-cli`; `CompState::advance`; `DynMesh::new_points`; every existing key
and default; the thresholds 0.4 and 1.6 (ADR 0191 Q1, not asked).

## Set-ups (case text inline in the tests; no case file, fixture or docs TOML block is added)

- **T-U** (F-258 through the tree): `box_2d`, `nx = 12`, `ny = 8`, `lx = 1.2`, `ly = 0.8`; the compressible keys of ADR
  0191 (HLLC, van Leer, RK3, CFL 0.4, `gamma = 1.4`) with `compressible.gas_constant = 1`, `end_time = 1`; the uniform
  state `rho = 1`, `p = 1`, `u0 = (0.3, 0.1, 0)`; `xmin` `total_inlet` (`temperature_value = 1 + 0.2 |u0|^2 / 1.4`,
  `total_pressure_value` its power 3.5, `flow_direction = u0`), `xmax` `pressure_outlet` (1), `ymin` `farfield`
  (`pressure_value = 1`, `temperature_value = 1`, `mach_value = |u0| / sqrt(1.4)`, `flow_angle = atan2(0.1, 0.3)` in
  degrees), `ymax` `nonreflecting` (`pressure_value = 1`, `reflection_value = 0.5`); `deformation = "rigid"` with ADR
  0191 P-C's motion about the centre `(0.6, 0.4, 0)`.
- **T-D** (the mesh-speed step): ADR 0191's P-X (20 x 1 x 1 layers, `end_time = 4`, stroke period 4, `section` absent)
  with `[dynamic_mesh.compressible] time_step = "mesh_speed"`. T-D0 is P-X itself; T-Da is P-X with
  `time_step = "absolute"`.
- **T-P** (prism layers, compressible): ADR 0191's P-T (40 x 2 x 2, period 16, `end_time = 32`) with
  `[mesh.box_3d] section = "triangles"` (ADR 0213's P-Δ through the tree, on the tree's schedule).
- **T-I** (incompressible layering): ADR 0213's G5L through the tree: `box_3d`, `nx, ny, nz = 20, 2, 2`,
  `lx, ly, lz = 1, 0.2, 0.2`, `section = "rectangles"`; `physics.nu = 0.01`; `algorithm = "piso"`,
  `convection = "central"`, `dt = 0.005`, `end_time = 0.6`, `momentum_rel_tol = 1e-13`, `pressure_rel_tol = 1e-13`,
  `pressure_abs_tol = 1e-16`, `momentum_max_iter = 5000`, `pressure_max_iter = 5000`; `xmax` velocity `fixed` (the
  following piston), `xmin` velocity `zero_gradient` and pressure `fixed` 0, the four sides velocity `zero_gradient`;
  `deformation = "layering"`, `moving_patches = ["xmax"]`, `displacement_expression_x = "-1 [m/s^2] * t^2"`.
- **T-X** (additions): T-I with `displacement_expression_x = "1 [m/s^2] * t^2"`.
- **T-C** (the CLI): P-X with `section = "triangles"` (2 prisms per layer) and ADR 0191's three reports.

**The library run** of a set-up is a loop in the test written with library calls only, on the tree's schedule, with the
motion evaluated from the case's expression text compiled in the tree's context (ADR 0191). For T-U: `step_ale` onto
`moved_mesh` of the pose, with the `CompBc` of each patch built in the test by the formulas of the case documentation. For
T-D: ADR 0191's `LayerStack` loop with `dt = max_dt_ale()`. For T-P: the same loop on `ExtrudedStack` with
`dt = max_dt()`. For T-I and T-X: ADR 0213's channel loop (`set_piston`, `move_mesh`, `set_follow_conditions` on `xmax`,
`step_ale_with_walls`, `adjust`, `face_sources`, `remap`, `remap_face_flux`, `replace_mesh`) on a `PimpleSolver::new` made
from the geometry, the initial fields with their conditions and the settings of a `Simulation` of the same case that has
not stepped (`pimple_solver()`): the mapping of the solver keys onto `PimpleSettings` is existing code and is not what
this item wires; the mesh, the motion, the step and the layer events are.

**Equality** of a tree run and a library run: the same number of steps and the same cell count after every step; at the
end, compressible (ADR 0191): `max |rho| <= 1e-12 max rho_lib`, `max |p| <= 1e-12 max p_lib`, `max |u| <= 1e-12 c0`
(`c0 = sqrt(1.4)`) for the differences, and `|t_tree - t_lib| <= 1e-12 t_lib`; incompressible: `max |u_tree - u_lib| <=
1e-12 U` (`U = 1.2`, the piston speed `2 t` at `t = 0.6`), `max |p_tree - p_lib| <= 1e-12 max |p_lib|`,
`max |phi_tree - phi_lib| <= 1e-12 max |phi_lib|`.

## Registered checks

All tests run in release. New files: `crates/rcfd-api/tests/tree_wiring_f259.rs` and
`crates/rcfd-cli/tests/tree_wiring_f259.rs`.

| Id | Check | Limit | Estimate behind it |
| --- | --- | --- | --- |
| K0 (gate of the keys) | with the two optional tables in the schema: `rcfd-api` `case_hashes_w48u`, `baseline_hashes`, `w48v_adaptive_correctors`, `rules_w19c`, `mesh_transform_w48u`, `physics_schema`; `rcfd-cli` `cli`; `rcfd-mcp` `default_f1`, `project_list_f270` | all pass, no file edited | read 3 |
| S1 (acceptance: bit-identical) | a case without the tables (P-X, T-I without its table and with `deformation = "smoothing"`): `to_toml` and `to_json_string` contain neither `box_3d` as a table nor `dynamic_mesh.compressible`; with a table set, both contain it and the settings hash differs | as stated | ADR 0155 |
| S2 (acceptance: bit-identical) | T-Da against T-D0: the bits of the final `rho`, `u`, `p` of every cell, the final time and the step count | equal | D4: the same operations |
| D0 (acceptance: bit-identical) | the unedited tests of `crates/rcfd-api/tests/layering_ale_f259.rs` other than `l_located_errors` (K1, A-eq with A-2 and A-B2 to A-B5, A-1, B-eq, C-eq, R1, D0's three hashes), `crates/rcfd-cli/tests/layering_ale_f259.rs` (X1, X2), `rcfd-api` `dynamic_mesh`, `inverse_volume_f290`, `compressible_bc_case`, `far_field_w49f`, `exp_rot_pab`, `consistent_correctors_case_f301` | pass | no default path edited |
| K2 (set-up) | `build_mesh` of T-P and of T-I against the `ExtrudedStack` mesh of D2: `content_hash`, every point's bits, patch names | equal | D2 |
| U-eq (acceptance: tree = library) | T-U: tree run against its library run | equality | the same calls |
| U-gcl | T-U tree run, after every step: `max(abs(rho - 1), abs(u - u0) / abs(u0), abs(p - 1))` over the cells | `<= 1e-12` | ADR 0207 U1 measured 1.85e-14 |
| U-set (set-up) | T-U library run: steps; on each of the four patches the largest `abs(w_n)` of `mesh_speeds()` over the run | `>= 50`; `>= 0.02` each | arithmetic above; ADR 0191 P-C took 277 steps to `t = 2` on a finer mesh |
| DT-eq (acceptance: tree = library) | T-D: tree run against its library run (`max_dt_ale`) | equality | the same calls |
| DT-n (negative control) | T-D against T-D0: the step counts | differ by `>= 0.2` percent of T-D0's | arithmetic above (1 to 4 percent) |
| DT-m | T-D tree run: every step's mass drift | `<= 1e-12` | ADR 0191 A-B2 5.4e-13 on a longer run |
| P-eq (acceptance: tree = library) | T-P: tree run against its library run | equality | the same calls |
| P-2 | T-P tree run: `p_bar V^1.4 / (p0 V0^1.4) - 1` at `t = 32` | `abs <= 2e-2` | ADR 0213 P2 3.4e-5 |
| P-4 | T-P tree run: every step's mass drift | `<= 1e-12` | ADR 0213 P4 6.95e-13 |
| P-6 | T-P tree run: removals and additions (cell count down or up by 8 in a step) | each `>= 5` | 24 each (ADR 0213) |
| I-eq (acceptance: tree = library) | T-I: tree run against its library run | equality | the same calls |
| I-1 | T-I tree run: every step's `continuity` | `<= 1e-9` | ADR 0213 I1 3.1e-15 |
| I-3 | T-I tree run, after every step: `sum V` of `Simulation.geom` against `0.04 x_p(t)` | relative `<= 1e-13` | ADR 0191 A-B3 8.9e-16 |
| I-4 (set-up) | T-I tree run: removals (cell count down by 4) | `>= 5` | 7 (arithmetic above) |
| X-eq (acceptance: tree = library) | T-X: tree run against its library run | equality | the same calls |
| X-4 (set-up) | T-X tree run: additions | `>= 5` | 7 (ADR 0213 X4) |
| L2 (acceptance: located errors) | N1 to N6 each from one of the set-ups by one change: a diagnostic at the stated path. The inputs of ADR 0191's L for E1 to E4 and E6 to E13: a diagnostic at the registered path. Its three E5 inputs and its E5 control: no diagnostic, and 5 steps of each run without a failure. T-U, T-D, T-P, T-I, T-X: no diagnostic | as stated | D1, D5 |
| R2 | T-I with `nx, ny, nz = 4, 1, 1` and `displacement_expression_x = "-0.9 [m] * (1 - exp(-t / 1e-6 [s]))"`: the run stops with a failure that starts with `dynamic_mesh:` and contains `layering` | as stated | ADR 0191 R1 |
| X3 (CLI) | `rcfd run` of T-C (temp folder, `--out`, `--quiet`): exit 0; the last row of `monitors.csv` against an in-process run: `rho_total`, `p_mean`, `p_max` | relative `<= 1e-12` | ADR 0191 X1 measured 0 |
| X4 (CLI) | `rcfd validate` of T-C with `generator = "box_2d"`, and of T-D with `algorithm = "piso"`: exit 3, stderr names `mesh.box_3d.section` and `dynamic_mesh.compressible.time_step` | as stated | D5 |

**Mutations** (scratch copy, not committed). Each is applied alone to the final tree, the named test is run once and must
fail; the file is restored and compared byte for byte with the kept copy.

| Id | Mutation | Must fail |
| --- | --- | --- |
| M1 | `advance_moving` ignores `time_step` (always `max_dt()`) | DT-eq (and DT-n) |
| M2 | the incompressible layered step sets the piston at the start time of the step (`d_x(t)` in place of `d_x(t + dt)`) | I-eq |
| M3 | the incompressible layer event leaves the pressure unmapped by volume (a merged cell takes the pressure of its first source; a split cell's children the parent's) | I-eq |
| M4 | `section = "triangles"` builds the rectangles' mesh | K2, P-eq |
| M5 | the E5 refusal put back | U-eq, L2 |

**Decision rule.** K0, S1, S2 and D0 are preconditions (a miss: the change is not proposed for merging, the cause is
recorded; a miss of K0 alone: the tables are removed, D1 and the driver functions stay, and the schema step is reported as
the maintainer's). With them, every equality (U-eq, DT-eq, P-eq, I-eq, X-eq), L2, X3 and X4 met, the set-up checks and
controls held and every mutation failed: the acceptance of F-259 is met for all the library work it names, and F-259 is
proposed **done** (pending the maintainer's acceptance). A miss of any of them, or a check not run: **partial** with the
miss named.

## Limits (not covered)

The mesh-speed step as the default (ADR 0207 Q2 (b), open); layer thresholds as keys (ADR 0191 Q1); a cross-section other
than the two of `Section`; a piston at the first plane (ADR 0213 Q2); layering on the PISO/PIMPLE path with BDF2,
turbulence, energy, body forces, sources, velocity expressions or lumped components (refused); six-DOF with the
compressible solver (F-244); the Studio's and the workspace's generator lists (F-293); restart of a moving-mesh run;
characteristic conditions through the tree with fluxes other than HLLC or with a smoothing motion beyond the 5 steps of
L2; everything in the roadmap's out-of-scope list.
