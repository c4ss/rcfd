# ADR 0262: F-342 registration: the case key `[mesh.adapt]` for the adaptive steady loop

Status: **REGISTERED 2026-11-12**, written before any code of F-342 and before any run of its checks. Not yet accepted by
the maintainer. Before this text nothing was compiled and no test was run: the code and documents under "What was read"
were read, and the arithmetic under "Arithmetic before this registration" was done by hand. Results go in the addendum at
the end; nothing above the addendum is changed after a run.

Parents: [ADR 0242](0242-f102-registration-adapted-cavity-and-solver-loop-hook.md) (F-102: `rcfd_fv::adaptive`,
`run_adaptive_simple`, the Re 100 cavity acceptance and its numbers, R1 from which this item comes),
[ADR 0247](0247-f343-registration-adaptation-criteria-and-suction-layer.md) (F-343: target count, edge-length weighting,
registers, `run_adaptive_simple_marked`; its R3: those options join this key),
[ADR 0257](0257-f259-registration-tree-wiring-of-prism-and-incompressible-layering-and-moving-characteristic-faces.md)
(the route by which case keys were added this morning: optional tables, its read 3 and its K0),
[ADR 0155](0155-job48u-rotate-the-mesh-instead-of-the-flow.md) and
[ADR 0223](0223-f301-registration-consistent-piso-correctors-moving-mesh-lag-estimate-case-key-and-benchmarks.md)
(optional tables added with no hash table or golden edited). Roadmap item F-342 (depends on F-102).

Number: the lead gave 0262 to F-342. Branch `f342-adapt-case-key` from main 2fa39daf, worktree `w49i-work/f342`.

House rules (`w49i-work/AGENT_RULES.md`, ADR 0257):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the number recorded, and the cause is
  diagnosed, not tuned away; a defective set-up gets a corrected test by an amendment written and committed before the
  corrected run, and the old one stays as an ignored record;
- defaults stay bit-identical: a case without the table gives the bytes it gave before;
- `crates/rcfd-api/tests/baseline_hashes.rs`, the hash tables (`case_hashes_w48u.rs`, `w48v_case_hashes.rs`) and the
  goldens are not edited or re-recorded; no existing test is edited;
- each registered run is made once, and nothing is re-run to pick a result;
- no new third-party crate, no unsafe code, the crate graph unchanged (`rcfd-api` already depends on `rcfd-fv` and
  `rcfd-mesh`), no commit trailers, no push;
- clean-room: nothing here is a new algorithm. The numerics are those of ADR 0242 and ADR 0247 (derived or cited there);
  this item only calls them from the case tree. `rcfd-fv` and `rcfd-mesh` are not edited.

## What F-342 asks

The acceptance of `docs/roadmap.json`, binding:

> An optional adapt block on a steady incompressible box-mesh case runs rcfd_fv::adaptive and reproduces the ADR 0242
> cavity numbers; cases without the block give identical hashes.

Clauses: (a1) an optional block; (a2) on a steady incompressible box-mesh case it runs `rcfd_fv::adaptive`; (a3) it
reproduces the ADR 0242 cavity numbers; (a4) cases without the block give identical hashes. The lead's brief adds: the
tree run equals the library run of ADR 0242 bit for bit (or to 1e-12 with the reason); the numbers from a case file;
located validate errors for bad values; a case without the block bit-identical (baseline hashes, `case_hashes_w48u`, the
tests ADR 0257 listed as K0); the CLI runs it; ADR 0247 R3 (its options join this key).

## What was read (main 2fa39daf, read only)

1. ADR 0242 in full; ADR 0247 D1 to D5, R1 to R3; ADR 0257 in full; `crates/rcfd-fv/src/adaptive.rs` in full;
   `crates/rcfd-fv/tests/adapt_cavity_f102.rs` in full (the cavity set-up, the sampler, the checks).
2. `crates/rcfd-api/src/sim.rs`: `build_mesh` and the generators `cavity_2d`, `box_2d`, `box_3d`; `Simulation::new` in
   full (the boundary conditions, the steady engine `Engine::Steady` with `SimpleSettings::new(nu, coupling)` and the
   options set from the case, the checks after it); `Simulation::step` (the steady branch and the common tail: monitors,
   reports, stop reason); `run`, `iteration`, `supports_checkpoint`. `crates/rcfd-mesh/src/gen.rs` `cavity_2d_rect`,
   `box_2d_graded` (cells of `axis_coords(nx, lx)` by `axis_coords(ny, ly)` by `axis_coords(1, lx / nx)`, patches `lid`,
   `walls`, `frontAndBack` resp. `xmin`, `xmax`, `ymin`, `ymax`, `frontAndBack`); `rcfd_mesh::adapt::AmrMesh::new`
   (`max_level <= 10`, six patches in the order xmin, xmax, ymin, ymax, zmin, zmax), `RefineTransfer`.
3. `crates/rcfd-api/src/schema.rs` (the `mesh` section, the `solver` defaults: `algorithm` `simplec`, `convection`
   `linear_upwind`, `alpha_u` 0.9, `alpha_p` 1.0, which are `SimpleSettings::new(.., Simplec)`'s); `dynmesh.rs` `check`;
   `registers.rs` `zone_from_setting`, `build` (a `box` register is `Register::in_box` of the cell centres, inclusive);
   `mesh_transform.rs` `apply`; `runctl.rs` `drive` and `write_results` (the CLI steps until `is_done` and writes
   `solution.vtu`, `monitors.csv` and the reports of the final state); `rcfd-cli/tests/tree_wiring_f259.rs`.
4. **Whether a key can be added without a hash-table or golden edit** (the brief: confirm by reading).
   - `case_hashes_w48u.rs` (H1): for every case file of `cases/`, the example projects, the test fixtures and every
     `docs/**/*.md` TOML block that parses, it compares the settings hash, the bytes of `to_toml` and `to_json_string`
     and the mesh hash of `build_mesh` with the record R0; `DEFAULT_KEYS` lists the keys whose default the text lists.
     An optional table (`Schema::optional`, ADR 0155) that an input does not write stays out of the data, so all four
     columns are unchanged and no `DEFAULT_KEYS` entry is needed (`mesh.transform`, `mesh.box_3d`,
     `dynamic_mesh.compressible`, `solver.consistent_correctors` are optional tables and are not in it). A source that is
     not in R0 (a new case file) is printed as "not in the record (new)" and does not fail the test. `build_mesh` stays
     the generator's mesh for every case (D2 below), so even the new case file's mesh column is the generator's.
   - `baseline_hashes.rs` hashes solver results of cases without the table.
   - The goldens (`crates/rcfd-mcp/tests/golden/*.json`: the MCP tool lists and the project shapes) hold no case-schema key;
     `rcfd-cli` `cli` checks that `rcfd schema` is a JSON-schema document, not its content. No file holds the exported
     JSON Schema of the case.
   - Tests that scan case texts: `rules_w19c` E2 evaluates every `cases/*.toml`, the TOML blocks of
     `docs/case-schema.md` and `docs/run-control.md` and every Rust literal of a test file holding `[mesh]` or `generator`
     with the setup rules; it fails on an error-level finding or a warning other than SV103 / SV101. The new case texts
     are plain cavities and channels (no rule of `setup_rules.json` applies to them by reading); it is in K0.
   - **Conclusion (by reading): the key can be added as an optional table with no edit of a hash table or a golden.** K0
     below checks the reading; if K0 fails, the table is taken out, the driver and its tests stay, and the schema step is
     reported as the maintainer's.

## Arithmetic before this registration (disclosed)

- **The 8 x 8 cavity** (fast set-up T-S): base 64 cells, fraction 0.4: cycle 1 marks `ceil(25.6) = 26`, `64 + 3 x 26 = 142`
  plus the balance closure (ADR 0242 measured 0 and 2 extra splits on 24 x 24), about 142 to 150; cycle 2 marks about 58
  to 60, about 316 to 330 cells. ADR 0242's 24 x 24 solves took 97 to 188 iterations; on these meshes about 100 to 200 per
  solve at well under 1 ms each in release: the run takes under one second.
- **Target count 300 on T-S** (T-T): `target_schedule(64, 300, 2, 1) = round(64 x (300 / 64)^0.5) = round(138.56) = 139`,
  `k_1 = ceil(75 / 3) = 25`, 139 cells before balance; cycle 2 aims at 300: 300 to 302 before balance.
- **The depth of the cavity.** `cavity_2d` (and `box_2d`) make the slab `axis_coords(1, lx / nx)` thick: `1/24` for the
  24 x 24 cavity, where ADR 0242's test used 0.1. The lattice of the key takes the generator's coordinates (D2), so the
  case file's run is not on ADR 0242's lattice. In the empty direction every face area and volume scales with the depth,
  so the discrete equations agree up to rounding; the stop rule of `SimpleSolver::run` is `max_du < tol` (a velocity,
  depth-free) and `continuity < 1e-6` (a flux sum, which scales with the depth: smaller by 2.4 here). If a solve stops at
  another iteration, the fields differ by the remaining distance to the fixed point, about `max_du r / (1 - r)` with a
  contraction `r` of 0.9 to 0.95 per iteration: `1e-7 x 20 = 2e-6` at most. Hence the comparison with ADR 0242's own run
  is at `1e-5` (R1b below), with this reason; the bit-for-bit comparison is with the library on the tree's lattice (E1,
  R1a).
- **Cost of R1**: ADR 0242 measured C 0.2 s, F (96 x 96) 77.9 s, A 2.2 s in release; R1 runs the tree's A, the library's A
  on the tree lattice, ADR 0242's A and F: about 85 s. It is an ignored record, run once.

## Design

**D1. The key.** An optional table `[mesh.adapt]` (`Schema::optional`, ADR 0155) in the `mesh` section, absent from every
case that does not write it. Its presence runs the adaptive loop; there is no `enabled` key. Keys (defaults are those of
ADR 0242's cavity run A, so a table with no key in it runs ADR 0242's loop):

| key | default | values | library |
| --- | --- | --- | --- |
| `cycles` | 2 | 0 to 20 | `AdaptiveSettings::cycles` |
| `max_level` | 2 | 1 to 10 | `AmrMesh::new` `max_level` |
| `marking` | `"fixed_fraction"` | `"fixed_fraction"`, `"target_count"` | `fixed_fraction_marks` / `target_count_marks` with `target_schedule` |
| `refine_fraction` | 0.4 | 0 to 1 | `AdaptiveSettings::refine_fraction` |
| `target_cells` | 0 | 0 to 1e7; must be > 0 with `target_count` | `n_target` |
| `indicator` | `"velocity_gradient"` | `"velocity_gradient"` (`|grad U| h`), `"velocity_gradient_unscaled"` (`|grad U|`), `"edge_length"` | `velocity_gradient_indicator` (`scale_by_length` true / false), `velocity_edge_length_indicator` |
| `edge_length_exponent` | 1.0 | >= 0 | `r` of ADR 0247 D3 |
| `register` | `""` | the name of a `[registers.<n>]` (not of type `field`), or empty | `Register::from_mask` of `registers::zone_from_setting` on each mesh |
| `register_use` | `"confine"` | `"confine"` (`restrict_marks` of the indicator's marks), `"refine"` (`register_marks`: every member below `max_level`) | ADR 0247 D4 |
| `balance` | true | | `AdaptiveSettings::balance` |
| `transfer` | `"linear"` | `"linear"`, `"injection"` | `AdaptiveSettings::transfer` |
| `max_iterations` | 20000 | 1 to 1e7 | `AdaptiveSettings::max_iter` (per solve) |
| `tolerance` | 1e-7 | 1e-14 to 1 | `AdaptiveSettings::tol`: `SimpleSolver::run` stops a solve when the largest velocity change is below it and the continuity error below 1e-6 |

**D2. The lattice.** `mesh.generator` = `cavity_2d`, `box_2d` or `box_3d` (with `[mesh.box_3d] section = "box"`),
`mesh.source = "generator"`, uniform (no grading, wall layers, periodic pairs, no mesh rotation): `AmrMesh::new` with the
generator's own coordinates (`axis_coords(nx, lx)`, `axis_coords(ny, ly)` and, in 2-D, `axis_coords(1, lx / nx)`; in 3-D
`axis_coords(nz, lz)`), refined axes `[true, true, false]` in 2-D and all three for `box_3d`, `mesh.adapt.max_level`, and the
generator's patches: `cavity_2d` `[walls, walls, walls, lid, frontAndBack, frontAndBack]` (walls, walls, walls, wall, empty,
empty); `box_2d` `[xmin, xmax, ymin, ymax, frontAndBack, frontAndBack]`; `box_3d` `[xmin, ..., zmax]`, all walls.
`build_mesh` is not changed (it stays the generator's mesh, the same points in another cell order); `Simulation::new`
builds on `amr.to_mesh()` when the table is present, on `build_mesh` otherwise, as before.

**D3. The run.** `Simulation::new` is split into the mesh step and `from_geometry(case, geom)` (the rest, unchanged).
With the table, the built simulation holds a pending adaptive run. The first `Simulation::step` runs the whole loop with
the library: `setup(geom)` = `from_geometry(case, geom)`'s steady solver's `u` (with its boundary conditions), `p` and
`settings`; then

- `fixed_fraction`, no register: `run_adaptive_simple` (indicator `velocity_gradient` or `_unscaled` through
  `scale_by_length`), or `run_adaptive_simple_with` with `velocity_edge_length_indicator(s, axes, r)`;
- `target_count`, no register: `run_adaptive_simple_target(amr, setup, settings, target_cells, indicator)`;
- with a register: `run_adaptive_simple_marked` with the marker: the register's mask on the current mesh; `refine`:
  `register_marks`; `confine`: `restrict_marks` of the marks of the two lines above (`fixed_fraction_marks`, or
  `target_count_marks` with `target_schedule(N_0, target_cells, cycles, k)` and `children_per_split`).

The simulation then holds the final solver and its geometry (`Simulation.geom`), the run is done, `converged` is the last
solve's, and the stop reason lists the cells and the convergence of each solve; one row of the monitors (`res_u`, `res_p`,
`continuity` of the last iteration and the reports on the final mesh). The cycle records are readable
(`Simulation::adaptive_cycles`). An error of the loop stops the run with a failure `mesh.adapt: ...`. The setup closure
builds a full simulation on each new mesh, so initial values, initial patches and boundary conditions are the case's on
every mesh (and are then overwritten by the transferred fields on cycles `k >= 1`, as in the library).

**D4. What the loop does not take** (a located error, D5): the loop builds `SimpleSolver::new(geom, u, p, settings)`
itself, so every option that the tree sets on the steady solver outside `u`, `p`, `settings`, and every model that the
tree's `step` adds, is refused rather than silently dropped. A build-time check compares the base simulation with that
plain state (no turbulence, energy, scalars, sources, components, rheology, properties, user functions, velocity
expressions, catalogue conditions, Marangoni stress, body force, conjugate heat transfer, electric, phase change,
chemistry, zone sources, radiation, particles, mixing plane, statistics, moving mesh; on the solver: no face viscosity,
body force, driver, pseudo-time, limiter, partner gradient, face-flux correction, boundary compensation, gradient
selectors, non-default residual norm or pressure interpolation, and the face flux of `SimpleSolver::new`), and a static
check refuses the keys that set them. Convergence conditions (`[convergence.*]`) are refused (the loop's stop rule is
D1's); `supports_checkpoint` is false with the table (a checkpoint of the adapted mesh cannot be loaded on the base mesh).

**D5. Located errors** (static, by `rcfd validate` and `Case::parse_toml`, except where marked "build"):

- V1 an unknown value or key in the table: the schema's parse error at that path (for example `mesh.adapt.indicator`,
  `mesh.adapt.cycle`);
- V2 a value out of range (`refine_fraction` 1.5, `max_level` 11, `cycles` -1, `tolerance` 0): the schema's error at
  that path;
- V3 `mesh.source = "file"`: at `mesh.source`; a generator other than the three: at `mesh.generator`;
- V4 `mesh.grading` other than 1, `mesh.wall_layers` > 0, `mesh.periodic_x` / `periodic_y`, `[mesh.box_3d] section`
  other than `"box"`, a mesh rotation: at that key;
- V5 `solver.algorithm` other than `simple` / `simplec`: at `solver.algorithm`;
- V6 `marking = "target_count"` with `target_cells = 0`: at `mesh.adapt.target_cells`;
- V7 an unknown register, or one of type `field`: at `mesh.adapt.register`; `register_use = "refine"` without a register:
  at `mesh.adapt.register_use`;
- V8 the models and options of D4 by their keys: at the key (for example `turbulence.model`, `energy.enabled`,
  `physics.body_force`, `solver.linear_upwind_limiter`, `initial.method`, `boundaries.<p>.velocity_expression_x`,
  `convergence.<n>`); the build-time comparison of D4 (build): at `mesh.adapt`, naming what it found.

**D6. Not changed.** `rcfd-fv`, `rcfd-mesh`, `rcfd-cli` sources; `build_mesh`; every existing key and default; every path
of a case without the table (the only new code on it is the presence test of the table). The static check is called from
`dynmesh::check` (one line; `case.rs` is not edited). The documentation: `docs/case-schema.md` (prose and table rows, no
TOML block) and `docs/mesh-adaptation.md` if it exists, else the section in `docs/case-schema.md` only.

## Set-ups

Case texts inline in the tests, except R1's case file `cases/cavity/adapt_re100.toml`.

- **T-S** (the fast cavity): `cavity_2d`, `nx = ny = 8`, `lx = ly = 1`; `physics.nu = 0.01`; `boundaries.lid` velocity
  `fixed` `[1, 0, 0]`, `boundaries.walls` velocity `fixed` (0); `solver.algorithm = "simplec"`, `convection =
  "linear_upwind"`, `non_orthogonal_correctors = 1`, `simplec_non_orthogonal_compensation = true` (ADR 0242's settings);
  `[mesh.adapt]` with no key (ADR 0242's loop: 2 cycles, level 2, fraction 0.4, `|grad U| h`, balance, linear transfer,
  20000 iterations, 1e-7).
- **T-T**: T-S with `marking = "target_count"`, `target_cells = 300`.
- **T-E**: T-S with `indicator = "edge_length"`, `edge_length_exponent = 2.0`.
- **T-U**: T-S with `indicator = "velocity_gradient_unscaled"`, `transfer = "injection"`, `balance = false`.
- **T-RC**: T-S with `[registers.top] type = "box"`, `box_min = [0, 0.75, -1]`, `box_max = [1, 1, 1]` and
  `register = "top"` (confine).
- **T-RR**: T-RC with `register_use = "refine"`, `cycles = 1`.
- **T-Z**: T-S with `cycles = 0`.
- **T-C** (box_2d channel): `box_2d`, `nx = 16`, `ny = 4`, `lx = 2`, `ly = 0.5`; `nu = 0.01`; `xmin` velocity `fixed`
  `[1, 0, 0]`, `xmax` velocity `zero_gradient` and pressure `fixed` 0, `ymin`, `ymax` velocity `fixed` (0);
  `solver.algorithm = "simple"`, `alpha_u = 0.7`, `alpha_p = 0.3` (`SimpleSettings::new(.., Simple)`'s), default
  convection; `[mesh.adapt]` `cycles = 1`, `max_iterations = 2000`.
- **T-3** (box_3d): `box_3d`, `nx = ny = nz = 4`, `lx = ly = lz = 1`; `ymax` velocity `fixed` `[1, 0, 0]`, the other
  five velocity `fixed` (0); ADR 0242's solver settings; `[mesh.adapt]` `cycles = 1`, `max_level = 1`,
  `refine_fraction = 0.25`, `max_iterations = 20` (the solves do not converge; equality does not need them to).
- **R1's case file** `cases/cavity/adapt_re100.toml`: T-S with `nx = ny = 24` (ADR 0242's A).

**The library run** of a set-up is a call in the test of the `rcfd_fv::adaptive` function D3 names, with a lattice built
in the test by `AmrMesh::new` from `rcfd_mesh::gen::axis_coords` as D2 states, and the `setup` closure of ADR 0242's test
(`cavity_setup` with `cavity_settings`, copied unchanged) or, for T-C and T-3, the same closure with the set-up's
conditions and `SimpleSettings::new` (plus ADR 0242's three options for T-3). For the register set-ups the mask is
`rcfd_mesh`'s `Register::in_box` of the box on each mesh (the definition of a `box` register, read 2), made into
`rcfd_fv::adaptive::Register::from_mask`. Nothing of the tree is called by the library run.

**Equality** (bit for bit): the same number of cycles and the same `CycleRecord` of each; the same lattice levels; and
`==` on every `f64` of the final `u`, `p` and `phi`.

## Registered checks

New test files `crates/rcfd-api/tests/adapt_case_f342.rs` and `crates/rcfd-cli/tests/adapt_case_f342.rs`. All in release.

| Id | Check | Limit | Estimate behind it |
| --- | --- | --- | --- |
| K0 (gate of the key) | with the table in the schema, unedited: `rcfd-api` `case_hashes_w48u`, `baseline_hashes`, `w48v_adaptive_correctors`, `rules_w19c`, `mesh_transform_w48u`, `physics_schema`, `tree_wiring_f259`, `layering_ale_f259`, `registers`, `dynamic_mesh`; `rcfd-cli` `cli`; `rcfd-mcp` `default_f1`, `project_list_f270` | all pass, no file edited | read 4 |
| S1 (a1, a4) | T-S without its table: `to_toml` and `to_json_string` contain no `adapt`, `get("mesh.adapt.cycles")` reads the default 2, and `Simulation::new`'s mesh hash equals `build_mesh`'s; with the table: both texts contain `adapt` and the settings hash differs | as stated | ADR 0155 |
| E1 (a2, tree = library) | T-S: tree run against the library run (`run_adaptive_simple`, ADR 0242's setup) | bit for bit | the same calls |
| E1n (control) | T-S tree run: the cycle cells strictly increase, the final mesh has level-2 cells, every solve converged | as stated | arithmetic: 64, about 145, about 320 |
| E2 (ADR 0247 R3, tree = library) | T-T (`run_adaptive_simple_target`), T-E (`run_adaptive_simple_with` + `velocity_edge_length_indicator`, `r = 2`), T-U (`run_adaptive_simple`, unscaled, injection, no balance), T-RC and T-RR (`run_adaptive_simple_marked` with the box register), T-C, T-3: each tree run against its library run | bit for bit | the same calls |
| E2n (control) | each of T-T, T-E, T-U, T-RC, T-RR differs from T-S's tree run (the final cell count, the levels or a bit of `u`) | as stated | each option changes the marks |
| E2t (set-up) | T-T: the final cell count | within 10 percent of 300 | ADR 0247 T1 (2307 for 2304) |
| E2r (set-up) | T-RR: every cell of level 1 has its centre in the box widened by the balance layer: `y >= 0.75 - 1/8` | as stated | register_marks refines the members; balance adds neighbours |
| Z (cycles = 0) | T-Z tree run against `SimpleSolver::new` + `run(20000, 1e-7)` on the base lattice with ADR 0242's setup | bit for bit | ADR 0242 H1 |
| R1a (a3, tree = library; ignored, heavy record) | the case file through `Simulation::new` + `run` against `run_adaptive_simple` of ADR 0242's setup on the tree's lattice (depth 1/24) | bit for bit | the same calls |
| R1b (a3, the ADR 0242 numbers; same record) | against ADR 0242's own run A (depth 0.1) and F (96 x 96, depth 0.1) with ADR 0242's sampler: the cycle cells 576, 1269, 2799; `N_A = 2799`; the dense samples within `1e-5` of A's; `D(A)` within `[0.00230, 0.00232]` and `<= 0.01`; `G(A)` within `[0.00915, 0.00917]` and `<= 0.02`; every solve converged | as stated | arithmetic (depth): differences at most about 2e-6; ADR 0242 printed 0.00231 and 0.00916 |
| L (located errors) | V1 to V8 each from T-S (or T-C, T-3) by one change: a diagnostic at the stated path; T-S, T-T, T-E, T-U, T-RC, T-RR, T-Z, T-C, T-3 and R1's file: none | as stated | D5 |
| X1 (CLI) | `rcfd run` of T-S (temp folder, `--out`, `--quiet`): exit 0; the last row of `monitors.csv` (`res_u`, `res_p`, `continuity`) against the in-process run; `solution.vtu` written | exit 0; relative `<= 1e-12` | the same run |
| X2 (CLI) | `rcfd validate` of T-S with `algorithm = "piso"` and of T-S with `refine_fraction = 1.5`: exit 3, stderr names `solver.algorithm` and `mesh.adapt.refine_fraction` | as stated | D5 |

**Mutations** (scratch copy `w49i-work/f342-scratch`, a `git archive` of the final tree, its own target directory; each
applied alone by exact text replacement, the named tests run once, the file restored and compared byte for byte):

| Id | Mutation | Must fail |
| --- | --- | --- |
| M1 | `indicator = "edge_length"` ignored (the default indicator used) | E2 (T-E) |
| M2 | the register's confinement not applied (the indicator's marks used as they are) | E2 (T-RC) |
| M3 | the 2-D lattice is 0.1 thick (ADR 0242's) instead of the generator's `lx / nx` | E1 |
| M4 | the step hook skipped (the plain steady step on the base lattice) | E1 |
| M5 | the static algorithm check removed | L |
| M6 | `target_cells` ignored (fixed fraction) | E2 (T-T) |

**Decision rule.** K0, S1 and Z are preconditions (a miss of K0 alone: the table is removed, the driver and its tests
stay, the schema step is reported as the maintainer's; a miss of S1 or Z: the change is not proposed, the cause recorded).
With them: E1, E2, R1a, R1b, L, X1 and X2 met, the controls E1n and E2n and the set-up checks E2t and E2r held, and every
mutation failed its test: F-342 is proposed **done** (pending the maintainer's acceptance). A miss of any of them, or a
check not run: **partial** with the miss named.

## Limits (not covered)

- Steady SIMPLE / SIMPLEC only, laminar, constant properties, no model or option of D4 (each a located error); transient
  adaptation, turbulence and energy on adapted meshes are not in `rcfd_fv::adaptive`.
- Uniform box meshes of three generators only; graded, inflated, periodic, rotated or file meshes are refused (the lattice
  of `AmrMesh` is a product of coordinate lists; a graded one is possible but not asked).
- No coarsening, no anisotropic refinement (ADR 0247 D5), no `field` registers (their initial fields do not follow the
  adapted mesh).
- The case's stop conditions do not apply inside the loop (D4); the monitors hold one row for the whole run.
- The Studio's and GUI's case editors are not touched; they read the schema and show the table like the other optional
  tables (not checked here).

## Open questions (taken on the maintainer's standing instruction of 2026-11-11: take the recommendation)

- **Q1. Where the table lives.** Options: (a) `[mesh.adapt]` (beside `[mesh.box_3d]` and `[mesh.transform]`, the mesh it
  changes); (b) a top-level `[adapt]`; (c) `[solver.adapt]`. Taken: (a); a new top-level section would add a section to
  every list of sections, (a) adds a table to an existing one.
- **Q2. What `converged` means for the run.** Options: (a) the last solve's; (b) every solve's. Taken: (a), the state the
  run leaves; the stop reason names each solve's.
- **Q3. Defaults of the table.** Taken: ADR 0242's run A, so an empty table is ADR 0242's loop.

## Amendment 1 (2026-11-12, written after the registered run of S1 and before its corrected run)

S1's test `s1_a_case_without_the_table_keeps_its_text_hash_and_mesh` failed at its first assertion: the text of T-S without
the table contains the substring `adapt` because its title is "F-342 T-S: adapted Re 100 cavity on an 8 x 8 base", which
`to_toml` and `to_json_string` write. The check searched for a substring that the set-up's own title holds: a defective
set-up, not a property of the key. The other assertions of the test did not run.

Corrected check **S1b** (a new test `s1b_a_case_without_the_table_keeps_its_text_hash_and_mesh`; the old test stays as an
ignored record, otherwise unedited): the same as S1 with "contain no `adapt`" read as: the settings data has no
`/mesh/adapt` (JSON pointer), `to_toml` has no line `[mesh.adapt]` and `to_json_string`'s value has no `adapt` key under
`mesh`; and with the table: the data has `/mesh/adapt`, `to_toml` has the line `[mesh.adapt]`, and the settings hash differs.
The limit (as stated) and every other clause are unchanged. S1b runs once.

## Addendum 1: results (2026-11-12)

Measured against the limits registered above; no limit was changed. Release builds on Windows. The registration is
e3636be2, the code and tests e6ad4616 (committed before any run), amendment 1 4caf2c39, the record of S1 and the test S1b
2b6f8aea. Each registered check ran once; none was repeated to change a result.

**Built as registered** (D1 to D6):

- `crates/rcfd-api/src/schema.rs`: one field, the optional table `mesh.adapt` (`crate::sim::adapt_schema`).
- `crates/rcfd-api/src/sim/adapt.rs` (new submodule of `sim`): the schema, `present`, the static check (V3 to V8),
  `base_lattice` (D2), `settings`, the build-time comparison `not_plain` (D4), `new_adaptive`, the `setup` closure, `run`
  (D3: `run_adaptive_simple`, `_with`, `_target` or `_marked`), `Simulation::step_adaptive`.
- `crates/rcfd-api/src/sim.rs`: `mod adapt` and its re-exports; the field `adapt` of `Simulation`; `new` split into the mesh
  step and `from_geometry` (the old body after the mesh, unchanged); the step hook (first statement of `step`);
  `supports_checkpoint` false with the table; the accessors `adaptive_cycles` and `adaptive_lattice`.
- `crates/rcfd-api/src/dynmesh.rs`: `check` starts from `crate::sim::adapt_check(case)` (empty without the table).
- `cases/cavity/adapt_re100.toml`; `docs/case-schema.md` (a section with a key table, no TOML block).
- Tests: `crates/rcfd-api/tests/adapt_case_f342.rs` (7 run; S1 kept as an ignored record; R1 ignored as heavy) and
  `crates/rcfd-cli/tests/adapt_case_f342.rs` (2). No existing test, hash table or golden was edited. `rcfd-fv`,
  `rcfd-mesh`, the `rcfd-cli` sources and `case.rs` are not changed.

### Results

| Check | Limit | Measured | Met |
| --- | --- | --- | --- |
| K0 | all pass, no file edited | `rcfd-api`: `case_hashes_w48u` 1, `baseline_hashes` 3 (1 ignored as before), `w48v_adaptive_correctors` 5 (4 ignored as before), `rules_w19c` 8 (1 ignored as before), `mesh_transform_w48u` 13, `physics_schema` 11, `tree_wiring_f259` 9, `layering_ale_f259` 7 (1 ignored as before), `registers` 7, `dynamic_mesh` 5 (1 ignored as before); `rcfd-cli` `cli` 8; `rcfd-mcp` `default_f1` 2, `project_list_f270` 1: all passed | held |
| S1 | as stated | failed at its first assertion: the set-up's title holds "adapted" (amendment 1); kept as an ignored record | defective set-up |
| S1b | as stated | without the table: no `/mesh/adapt` in the data, no `[mesh.adapt]` line, no `adapt` key in the JSON; defaults read (cycles 2); `Simulation::new`'s mesh equals `build_mesh`'s; with the table: present, settings hash `7d7d5df876207d92` against `6ccf21e4afaf224b` | met |
| E1 | bit for bit | records, levels, mesh hash, `u`, `p`, `phi` equal; 316 cells | met |
| E1n | as stated | cells 64, 142, 316; 88, 72, 70 iterations, all converged; level 2 present | held |
| E2 | bit for bit | T-T 301 cells, T-E 313, T-U 313, T-RC 235, T-RR 112, T-C (box_2d, simple) 142, T-3 (box_3d, 20 iterations per solve, not converged) 176: each equal | met |
| E2n | each differs from T-S | 301, 313, 313, 235, 112 cells against 316 | held |
| E2t | within 10 percent of 300 | 301 | held |
| E2r | `y >= 0.625` | lowest level-1 centre `y = 0.78125` | held |
| Z | bit for bit | 64 cells, 88 iterations; `u`, `p`, `phi` equal | met |
| R1a | bit for bit | the case file through the tree against the library on the tree's lattice: records, levels, mesh, fields equal; 2799 cells | met |
| R1b | as stated | cycle cells 576, 1269, 2799 with 97, 122, 188 iterations (ADR 0242's A the same), all converged; F 853 iterations, converged; dense samples of the tree against ADR 0242's A `9.9e-11` (limit `1e-5`); `D(A) = 0.002311` (ADR 0242's A 0.002311); `G(A) = 0.009159` (A 0.009159, F 0.009119) | met |
| L | as stated | V1 to V8 (25 inputs) each at its path; the 10 controls (nine set-ups and the case file): no diagnostic | met |
| X1 | exit 0; relative `<= 1e-12` | exit 0, `solution.vtu` written; `res_u` 9.52099157010459e-6, `res_p` 1.9198313429115884e-5, `continuity` 2.5422756261272994e-8 from `monitors.csv`, equal to the in-process run (difference 0) | met |
| X2 | exit 3, path named | exit 3 for both: `line 23: solver.algorithm: ...`, `line 11: mesh.adapt.refine_fraction: 1.5 outside [...]` | met |

The 9.9e-11 of R1b is the rounding of the slab depth (1/24 against 0.1): every solve stopped at the same iteration as ADR
0242's, so the difference is rounding, not the stop rule (the arithmetic allowed up to about 2e-6). R1 took 81 s (F 69 s);
the tree run 6.4 s.

### Mutations

`git archive` of 2b6f8aea in `w49i-work/f342-scratch/mut`, target `f342-scratch/target`, script `f342-scratch/mutate.py`
(exact replacement found once, the file restored and compared byte for byte after each).

| Id | Mutation | Must fail | Result |
| --- | --- | --- | --- |
| M1 | `edge_length` ignored | E2 (T-E) | failed: T-E cycle records |
| M2 | confinement not applied | E2 (T-RC) | failed: T-RC cycle records |
| M3 | 2-D lattice 0.1 thick | E1 | failed: E1 mesh hash |
| M4 | step hook skipped | E1 | failed: 10 plain steps instead of one adaptive step |
| M5 | algorithm check removed | L | failed: V5 located at `mesh.adapt` (the build-time backstop of D4) instead of `solver.algorithm` |
| M6 | `target_cells` ignored | E2 (T-T) | failed: T-T cycle records |

### fmt and clippy

`cargo fmt -p rcfd-api --check`, `cargo fmt -p rcfd-cli --check`: clean. `cargo clippy --release -p rcfd-api -p rcfd-cli
--all-targets -- -D warnings`: clean.

### Deviations and disclosures

- S1's set-up was defective (amendment 1); S1b ran once and met its limit.
- The box register of the library runs is `rcfd_post::registers::Register::in_box` (the registration said `rcfd_mesh`'s;
  the tree's `box` register uses `rcfd_post`'s).
- Equality also compares the mesh content hash; `Simulation::adaptive_lattice` was added to compare the tree's levels.
- The case file writes `cycles`, `max_level`, `refine_fraction` and `indicator` at their defaults (the registration said T-S
  at 24 x 24 with an empty table): the same settings.
- The sampler asks for points at `z = 0` instead of 0.05; its box test looks at x and y only.
- V8's turbulence input also gives an existing diagnostic (`boundaries.lid.turbulence_length`).
- `docs/case-schema.md` was edited after K0; `case_hashes_w48u` and `rules_w19c` ran again after it and passed (prose and a
  key table, no TOML block). The new case file appears in H1 as a new source (printed, not compared), as read 4 said.

### Not run

The whole workspace suite and the gate (the lead runs them); the Studio and GUI tests (`rcfd-studio`, `rcfd-gui`), whose
schema models were not checked; `rcfd-mcp` `tool_list_f275`.

### Decision by the registered rule

K0, S1 (as S1b) and Z held; E1, E2, R1a, R1b, L, X1 and X2 met; E1n, E2n, E2t and E2r held; every mutation failed its
test. F-342 is proposed **done**, pending the maintainer's acceptance.

### Remaining questions

- **Q4. Graded and inflated box meshes.** `AmrMesh` takes any coordinate lists, so `box_2d` with `grading` or
  `wall_layers` could be adapted too. Options: (a) keep them refused (as built); (b) build the lattice from the generator's
  graded coordinates in a follow-up with its own equality check. Recommendation (b) when a case needs it; no default
  changes.
- **Q5. Monitors of the loop.** One monitor row for the whole run. Options: (a) as built; (b) one row per solve, with the
  cell count as a monitor. Recommendation (a) until the Studio shows adaptive runs.

### For the lead: roadmap entries

**F-342**, proposed status: done (pending the maintainer's acceptance). New note:

> From ADR 0242 R1, recommendation taken on the maintainer's standing instruction of 2026-11-11. DONE pending acceptance (ADR 0262, 2026-11-12): the optional table [mesh.adapt] (absent from every case that does not write it; an empty table is ADR 0242's loop) runs rcfd_fv::adaptive on a steady laminar simple / simplec case of mesh.generator cavity_2d, box_2d or box_3d (uniform): the lattice takes the generator's coordinates and patches, the first step runs the whole loop and leaves the final mesh and solver; keys cycles, max_level, marking (fixed_fraction / target_count with target_cells), refine_fraction, indicator (velocity_gradient, velocity_gradient_unscaled, edge_length with edge_length_exponent), register and register_use (confine / refine; ADR 0247 R3), balance, transfer, max_iterations, tolerance. Tree runs equal library runs bit for bit on the 8 x 8 cavity (316 cells), with each criterion, on a box_2d channel and on a box_3d cube; the case file cases/cavity/adapt_re100.toml equals the library on its lattice bit for bit and gives ADR 0242's numbers (cells 576, 1269, 2799 with ADR 0242's iteration counts, centreline difference to the 96 x 96 solution 0.002311, Ghia 0.009159; within 9.9e-11 of ADR 0242's run, whose slab is 0.1 thick against the generator's 1/24); models and solver options the loop would drop, non-uniform or file meshes and other algorithms are located errors (also through rcfd validate); rcfd run gives the in-process run's monitors; baseline hashes, case_hashes_w48u and ADR 0257's K0 tests pass unedited; six mutations each failed their tests. Open: graded and inflated lattices (ADR 0262 Q4), per-solve monitors (Q5).

**F-343**: its note's last sentence should read:

> Follow-up (ADR 0247 R1, recommendation taken on the maintainer's standing instruction of 2026-11-11): F-348; the new options are case keys since ADR 0262 (2026-11-12, F-342): [mesh.adapt] marking = "target_count", indicator = "edge_length", register and register_use.

**F-102**: in its note, "Not delivered (follow-ups): a case-file key;" should read:

> Not delivered (follow-ups): a case-file key (since ADR 0262, 2026-11-12: [mesh.adapt], F-342);

## Addendum: gate 4, 2026-11-13

Gate 4 failed seven tests because F-342 (commit e6ad4616) shipped `cases/cavity/adapt_re100.toml`, a `[mesh.adapt]` case, without updating the tests and the list that cover every shipped case. The maintainer approved on 2026-11-13 editing the existing tests below for this purpose only; no other assertion, limit or hash table was touched (baseline_hashes.rs, the case and mesh hash tables and the goldens are unchanged).

| Test | Edit |
|---|---|
| `crates/rcfd-studio/tests/store_plan_sf.rs`, `sf1_every_shipped_case_plans_a_store` (EXPECTED, near line 52) | `EXPECTED` 10 -> 11 entries; `cases/cavity/adapt_re100.toml` added with the store line its plan gives |
| `crates/rcfd-studio/tests/shipped_store_sf2.rs`, `sf2_every_shipped_case_writes_a_readable_store` (line 64) | case count 10 -> 11 |
| `crates/rcfd-studio/tests/saved_store_r.rs`, `dv6_unedited_plans_are_bit_identical` (line 818) | "cases added after 6867e40 (no recorded plan)" list gains `cases/cavity/adapt_re100.toml` |
| `crates/rcfd-studio/tests/preview_g1.rs`, `g1_g7_shipped_cases_preview_the_solvers_mesh` (loop at line 133) | a case with `[mesh.adapt]` is compared with `build_mesh` (the un-adapted generator mesh the preview shows) and a note is printed; all other cases are compared with `Simulation::new` as before |
| `crates/rcfd-studio/tests/shipped_store_sf2.rs`, `sf2_every_shipped_case_writes_a_readable_store` (loop, line 70) | the `[mesh.adapt]` case is skipped with a printed note, so `ok` stays 10 of 11 cases (see the store gap below) |
| `crates/rcfd-studio/tests/case_starter_g2.rs`, `cs6_the_excluded_case_files_do_not_open_in_the_editor` (line 548) | the new exclusion is skipped: the adaptive case opens in the editor, and CS6 expects an excluded file not to (the count of 6 descriptors is unchanged) |

Not a test edit: `cases/cavity/adapt_re100.toml` was added to the data list `EXCLUDED` in `crates/rcfd-workspace/src/examples.rs` (reason: an adaptive-mesh case the starter does not offer), which clears `cs5_the_list_is_in_step_with_cases` and `cs5_controls_each_kind_of_drift_is_one_problem`. `docs/tutorials/gallery.md` gains a section with `$ rcfd check cases/cavity/adapt_re100.toml` (output as printed: `ok, 576 cells`, the base mesh), which clears `gallery_covers_every_case` (ADR 0089 T5).

Preview gap: the Studio preview of a `[mesh.adapt]` case builds the generator's mesh (`build_mesh`, 576 cells here), `Simulation::new` builds the base lattice (`amr.to_mesh`, the same 576 cells with another numbering, hence another mesh hash), and the first step of the run adapts the mesh (about 2799 cells after two adaptation steps, ADR 0242). So the preview shows an un-adapted mesh, not the one the run solves on.

Store gap found while fixing SF2: a run with `output.store_*` on a `[mesh.adapt]` case opens the field store on the base lattice, the first step adapts the mesh, and the first snapshot is refused with "field store: io error: tsstore: field 'ux' has 2799 values, the mesh has 576". SF2 skips this case with a note; nothing in the solver or the store was changed.

## For the lead: roadmap entries

**F-342**, proposed status: done (pending the maintainer's acceptance). New note:

> From ADR 0242 R1, recommendation taken on the maintainer's standing instruction of 2026-11-11. DONE pending acceptance (ADR 0262, 2026-11-12): the optional table [mesh.adapt] (absent from every case that does not write it; an empty table is ADR 0242's loop) runs rcfd_fv::adaptive on a steady laminar simple / simplec case of mesh.generator cavity_2d, box_2d or box_3d (uniform): the lattice takes the generator's coordinates and patches, the first step runs the whole loop and leaves the final mesh and solver; keys cycles, max_level, marking (fixed_fraction / target_count with target_cells), refine_fraction, indicator (velocity_gradient, velocity_gradient_unscaled, edge_length with edge_length_exponent), register and register_use (confine / refine; ADR 0247 R3), balance, transfer, max_iterations, tolerance. Tree runs equal library runs bit for bit on the 8 x 8 cavity (316 cells), with each criterion, on a box_2d channel and on a box_3d cube; the case file cases/cavity/adapt_re100.toml equals the library on its lattice bit for bit and gives ADR 0242's numbers (cells 576, 1269, 2799 with ADR 0242's iteration counts, centreline difference to the 96 x 96 solution 0.002311, Ghia 0.009159; within 9.9e-11 of ADR 0242's run, whose slab is 0.1 thick against the generator's 1/24); models and solver options the loop would drop, non-uniform or file meshes and other algorithms are located errors (also through rcfd validate); rcfd run gives the in-process run's monitors; baseline hashes, case_hashes_w48u and ADR 0257's K0 tests pass unedited; six mutations each failed their tests. Open: graded and inflated lattices (ADR 0262 Q4), per-solve monitors (Q5).

**F-343**: its note's last sentence should read:

> Follow-up (ADR 0247 R1, recommendation taken on the maintainer's standing instruction of 2026-11-11): F-348; the new options are case keys since ADR 0262 (2026-11-12, F-342): [mesh.adapt] marking = "target_count", indicator = "edge_length", register and register_use.

**F-102**: in its note, "Not delivered (follow-ups): a case-file key;" should read:

> Not delivered (follow-ups): a case-file key (since ADR 0262, 2026-11-12: [mesh.adapt], F-342);

## Addendum: gate 4, 2026-11-13

Gate 4 failed seven tests because F-342 (commit e6ad4616) shipped `cases/cavity/adapt_re100.toml`, a `[mesh.adapt]` case, without updating the tests and the list that cover every shipped case. The maintainer approved on 2026-11-13 editing the existing tests below for this purpose only; no other assertion, limit or hash table was touched (baseline_hashes.rs, the case and mesh hash tables and the goldens are unchanged).

| Test | Edit |
|---|---|
| `crates/rcfd-studio/tests/store_plan_sf.rs`, `sf1_every_shipped_case_plans_a_store` (EXPECTED, near line 52) | `EXPECTED` 10 -> 11 entries; `cases/cavity/adapt_re100.toml` added with the store line its plan gives |
| `crates/rcfd-studio/tests/shipped_store_sf2.rs`, `sf2_every_shipped_case_writes_a_readable_store` (line 64) | case count 10 -> 11 |
| `crates/rcfd-studio/tests/saved_store_r.rs`, `dv6_unedited_plans_are_bit_identical` (line 818) | "cases added after 6867e40 (no recorded plan)" list gains `cases/cavity/adapt_re100.toml` |
| `crates/rcfd-studio/tests/preview_g1.rs`, `g1_g7_shipped_cases_preview_the_solvers_mesh` (loop at line 133) | a case with `[mesh.adapt]` is compared with `build_mesh` (the un-adapted generator mesh the preview shows) and a note is printed; all other cases are compared with `Simulation::new` as before |
| `crates/rcfd-studio/tests/shipped_store_sf2.rs`, `sf2_every_shipped_case_writes_a_readable_store` (loop, line 70) | the `[mesh.adapt]` case is skipped with a printed note, so `ok` stays 10 of 11 cases (see the store gap below) |
| `crates/rcfd-studio/tests/case_starter_g2.rs`, `cs6_the_excluded_case_files_do_not_open_in_the_editor` (line 548) | the new exclusion is skipped: the adaptive case opens in the editor, and CS6 expects an excluded file not to (the count of 6 descriptors is unchanged) |

Not a test edit: `cases/cavity/adapt_re100.toml` was added to the data list `EXCLUDED` in `crates/rcfd-workspace/src/examples.rs` (reason: an adaptive-mesh case the starter does not offer), which clears `cs5_the_list_is_in_step_with_cases` and `cs5_controls_each_kind_of_drift_is_one_problem`. `docs/tutorials/gallery.md` gains a section with `$ rcfd check cases/cavity/adapt_re100.toml` (output as printed: `ok, 576 cells`, the base mesh), which clears `gallery_covers_every_case` (ADR 0089 T5).

Preview gap: the Studio preview builds the base lattice of a case (24 x 24 = 576 cells here), while `Simulation::new` adapts the mesh (about 2799 cells after two steps, ADR 0242), so their mesh hashes differ. The preview therefore shows the base mesh of a `[mesh.adapt]` case, not the mesh the run solves on. This is a known limitation, recorded here and in the follow-up below.

## For the lead: roadmap entries

**New follow-up of F-342**: title "Studio preview of a case with [mesh.adapt] shows the adapted mesh"; deps F-342 (the Studio mesh preview has no roadmap item of its own); acceptance:

> The Studio mesh preview of a case with [mesh.adapt] shows the mesh the run solves on (the adapted mesh of Simulation::new, with its cell count and mesh hash), and preview_g1's g1_g7_shipped_cases_preview_the_solvers_mesh compares cases/cavity/adapt_re100.toml with Simulation::new like every other case, without the base-mesh exception added on 2026-11-13.

**New follow-up of F-342**: title "Field store of a run with [mesh.adapt] writes the adapted mesh"; deps F-342, F-263; acceptance:

> A run of cases/cavity/adapt_re100.toml with output.store_interval set writes a readable field store whose mesh is the adapted mesh (2799 cells for the shipped case) with no warning, and sf2_every_shipped_case_writes_a_readable_store runs that case through the store like the other ten without the skip added on 2026-11-13.
