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
