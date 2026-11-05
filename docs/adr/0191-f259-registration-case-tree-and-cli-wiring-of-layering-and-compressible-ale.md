# ADR 0191: F-259 registration: case-tree and CLI wiring of layering and compressible ALE

Status: **REGISTERED 2026-11-05**, written before any code of F-259 (the one test run made before it is disclosed below).
Not yet accepted by the maintainer. Results go in an addendum at the end.

Parents: [ADR 0090](0090-wave38c-dynamic-mesh-layering.md) (F-230: the planar layer stack, the compressible ALE step and the
piston set-up P), [ADR 0033](0033-wave20b-dynamic-mesh.md) D8 (the `dynamic_mesh` section, incompressible prescribed and
six-DOF motion), [ADR 0098](0098-wave40b-taylor-couette-converged-stretched-third-level-fast-piston.md) (the Mach 0.2
piston), [ADR 0183](0183-exp-rot-registration.md) amendment P-AB (`rest_patches`, a rigid motion without a moving patch),
[ADR 0155](0155-job48u-rotate-the-mesh-instead-of-the-flow.md) and [ADR 0163](0163-job48ac-case-hashes-h1-after-the-corrector-control-key.md)
(H1, the case-file record).

Number: 0188 is the highest on main 201ea17. The branch `f274-design-evals` holds 0189; the caller reserved 0190 for F-256
and 0192 for F-253.

House rules: limits are fixed here before the code and the runs; a miss becomes an ignored `*_known_miss` test with the
limit intact and the measured number recorded; a defective set-up is corrected by an amendment written before the corrected
run, the old test kept ignored; defaults stay bit-identical; `crates/rcfd-api/tests/baseline_hashes.rs` and the H1 record R0
are not re-recorded; no new third-party crate; no commit trailers. Clean-room: first principles and our own ADRs only.

## What F-259 asks

The roadmap entry (depends on F-230 and F-063, both done): case-tree and CLI wiring of layering and compressible ALE
(incompressible prescribed and six-DOF motion are already in the tree, ADR 0033 D8); invalid keys give located errors, a tree
run equals the library run to 1e-12, and a case without the keys is bit-identical (baseline hashes unchanged). Out of scope:
sliding interfaces (F-250), elastic-analogy smoothing, six-DOF coupling (F-244), more than one moving layered boundary,
non-planar layer surfaces, turbulence or energy in the incompressible moving-mesh path. The note says it can ship first.

**What can be wired and what is blocked.** The library work that exists is ADR 0090's: `CompressibleSolver::step_ale`
(moving slip walls, fixed-state and extrapolated faces) and `rcfd_mesh::layering::LayerStack` (a structured box stack with
one moving x plane, used by the compressible solver only). F-257 (prism layering, layering in the incompressible PIMPLE path)
and F-258 (characteristic boundary conditions on moving faces, the mesh-speed time step) are `todo` on main and have no code.
This item wires what exists. The wiring of F-257's and F-258's library work is blocked on that work and is not built here;
the checks below are the ones it would wire under.

## What the code read shows (main 201ea17, read only)

1. **The compressible ALE step.** `step_ale(new_geom, dt)` needs the same topology. It takes the swept volumes from
   `rcfd_mesh::motion::swept_volumes`, gives a moving slip wall the mirror relative to the wall, and computes the fluxes of
   fixed-state and extrapolated faces in the face frame. A face whose normal mesh speed is not exactly `0.0`
   (`vn != 0.0`) with a characteristic condition (`total_inlet`, `pressure_outlet`, `nonreflecting`, `farfield`) is an
   error. `replace_mesh(geom, u)` installs a new mesh and state. `max_dt` uses `|u.n| + c`, not the mesh speed.
2. **The layer stack.** `LayerStack::uniform(n, length, [ny, nz], [ly, lz], a_min, a_max)` needs `n >= 2`, `a_min > 0` and
   `1 + a_min <= a_max`. `mesh()` builds `box_mesh([n, ny, nz], [n, ly, lz])` and maps plane `i` to `xs[i]`, with
   `xs[i] = length * i / n` at the start. The patches are `xmin, xmax, ymin, ymax, zmin, zmax`, all walls. The piston is the
   last plane (`xmax`). `set_piston(x)` fails when `x` is not beyond the last static plane, `adjust()` makes at most one
   event, and `remap` carries cell totals.
3. **The tree.** `dynamic_mesh` (`rcfd-api/src/dynmesh.rs`) runs with `piso` / `pimple` only: a static conflict at
   `dynamic_mesh.enabled`, and `Simulation::new` refuses an engine other than the transient incompressible one at the same
   path. The compressible solver is `solver.algorithm = "compressible"` (`comp.rs`, `CompState`). Its step takes
   `dt = max_dt()` clipped to `solver.end_time`. Its shadow fields `u`, `p` and `phi` are sized once at construction.
4. **No generator gives the stack's mesh.** No case-tree generator builds `box_mesh`'s six-patch box. `box_2d` has
   `xmin, xmax, ymin, ymax` and an empty `frontAndBack`, one cell thick.
5. **Generator lists elsewhere.** The Studio (`preview.rs` `count_keys` and the estimate, `runs.rs`, `store_plan.rs`) and
   `rcfd-workspace/src/plan.rs` match generator names with a `_` arm. A new generator gets no cell estimate there and no
   error.
6. **H1** (`case_hashes_w48u.rs`) fails only when a recorded source changes or is missing. A new source (a docs TOML block,
   a fixture) is printed, not failed. Editing a recorded docs TOML block changes its source key, so no recorded block is
   edited.
7. **`baseline_hashes.rs` hashes no compressible case** (ADR 0090). D0 below records one.
8. **The T1 tests** of `crates/rcfd-api/tests/dynamic_mesh.rs` assert diagnostic paths, not texts.
9. `boundaries.<p>.velocity` defaults to `"fixed"`.

## Run before this registration (disclosed)

The test D0 (below) was written as the new file `crates/rcfd-api/tests/layering_ale_f259.rs`, with no source change, and run
twice on main 201ea17 (Windows, release). Both runs printed the same hashes:

| Case | FNV-1a 64 | Steps | End time |
|---|---|---|---|
| `cases/sod/sod.toml` | `919c6d4caef875de` | 172 | 0.2 |
| `cases/wedge/wedge.toml` | `3c87a648a9da72e0` | 1122 | 3 |
| sod with `xmax` `pressure_outlet` at 0.1 | `220cf1723c972add` | 172 | 0.2 |

The hash covers the bits of the final `rho`, `ux`, `uy`, `uz` and `p` of every cell, the final time and the step count, from
`Simulation::run` of the case text. No other run was made.

## Decisions

**W1. Scope.** Wire ADR 0090's library: the compressible ALE step under a prescribed motion (rigid or smoothing, the existing
keys) and the layering of a planar piston stack (compressible only). Not built: the library work of F-257 and F-258 and its
wiring; six-DOF motion with the compressible solver (F-244); the `InverseVolume` smoothing stiffness (Q2).

**W2. How a case selects it. No new key.**

* Compressible ALE: `solver.algorithm = "compressible"`, `compressible.mode = "transient"`, `dynamic_mesh.enabled = true`,
  `motion = "prescribed"`, `deformation = "rigid"` or `"smoothing"`, with the existing keys and meanings (ADR 0033 D4 and D5,
  ADR 0183 P-B: a rigid prescribed motion may have no moving patch).
* Layering: `dynamic_mesh.deformation = "layering"`, a new value of the existing enumeration, compressible only. The mesh is
  `box_3d` (W3) with `nx >= 2` layers along x. The piston is the patch `xmax` (`moving_patches = ["xmax"]`). Its position is
  `x_p(t) = lx + d_x(t)`, with `d_x` from `displacement_expression_x`. The stack is
  `LayerStack::uniform(nx, lx, [ny, nz], [ly, lz], 0.4, 1.6)`: `h0 = lx / nx` and ADR 0090's registered thresholds
  `a_min = 0.4`, `a_max = 1.6`, with no key (Q1).
* The schema gains two enumeration values (`box_3d` of `mesh.generator`, `layering` of `dynamic_mesh.deformation`) and no
  key. Defaults are unchanged, so the settings hash and the text `Case::to_toml` writes are unchanged for every case that does
  not use them. H1 and `baseline_hashes.rs` need no edit.

**W3. The generator `box_3d`.** `mesh.generator = "box_3d"` builds `rcfd_mesh::gen::box_mesh([nx, ny, nz], [lx, ly, lz],
uniform)`, with patches `xmin, xmax, ymin, ymax, zmin, zmax` (walls) from the existing keys `nx, ny, nz, lx, ly, lz`. It is
usable without layering. It is uniform only: `mesh.grading` other than 1, `mesh.periodic_x` and `mesh.periodic_y` are located
errors at those keys (`wall_layers` is refused already). For `box_mesh` and the stack, plane `i` is `lx * i / nx` by the same
formula, so the meshes should be equal bit for bit. K1 checks this. At run time `DynMesh::from_case` also compares the stack's
points with the case mesh's, bit for bit, and refuses at `mesh.generator` if they differ (with K1 met, this never fires).

**W4. The step.** In the compressible branch of `Simulation::step`, with a dynamic mesh:

1. `dt = max_dt()` of the current mesh (ADR 0090's shortcut, absolute velocity; the mesh-speed step is F-258), clipped to
   `solver.end_time` exactly as the fixed-mesh step; `t_new = time + dt`.
2. Rigid or smoothing: the new points are those of the incompressible path at `t_new` (the pose of the initial points, or
   the smoothing of the initial mesh with the moving patches prescribed, `fixed_patches` fixed and the others sliding; the
   same code, moved into one function without changing its operations). Then `moved_mesh`, `FvGeometry::new`, `step_ale`.
3. Layering: `set_piston(lx + d_x(t_new))`, the geometry of `stack.mesh()`, `step_ale`. Then `adjust()`: on an event, the
   geometry of the new `stack.mesh()`, the state `remap(change, u, old volumes, new volumes)` and `replace_mesh`.
4. The step's relative density and pressure changes (`res_rho`, `res_p`) and the positivity check are taken after `step_ale`
   and before a layer event (same cell count). The mass drift is taken after the event.
5. The shadow fields `u`, `p`, `phi` are rebuilt on the new geometry when the cell count changed, then refreshed.
   `Simulation.geom` is the new geometry after every step, as on the incompressible moving path.
6. A failure of the motion, the moved mesh, `set_piston`, the stack's mesh or `replace_mesh` stops the run with
   `dynamic_mesh: step N, t = ...: <error>` (ADR 0033 D8's message, as a failure). An error of `step_ale` stops it with the
   fixed-mesh path's reason, `compressible solver failed at step N: <error>`.

The fixed-mesh compressible step (`CompState::advance`) is not edited.

**W5. Located errors.** Static (`Case::semantic_errors`) unless marked "build" (`Simulation::new`). The incompressible
checks and their messages are not changed; the new ones branch off before them.

With `solver.algorithm = "compressible"` and `dynamic_mesh.enabled = true`:

* E1 `compressible.mode = "steady"`: at `dynamic_mesh.enabled`.
* E2 `motion = "six_dof"`: at `dynamic_mesh.motion` (F-244).
* E3 a non-empty `follow_patches`: at `dynamic_mesh.follow_patches` (a moving slip wall carries the mesh speed; following
  patches belong to `piso` / `pimple`).
* E4 a non-empty `rest_patches`: at `dynamic_mesh.rest_patches`.
* E5 a characteristic `compressible_bc` on a patch whose faces can move: at `boundaries.<p>.compressible_bc` (F-258). A patch
  can move under `rigid` (every patch); under `smoothing` unless it is in `fixed_patches`; under `layering` unless it is
  `xmin`. A patch that slides or stretches in its plane gets a round-off swept volume, which `step_ale` refuses (item 1).
* The incompressible conflicts "solver.algorithm other than piso / pimple" and "time_scheme other than euler" do not apply.
  Turbulence and energy stay refused by the compressible checks at their own keys. Statistics, scalars, particles, sources,
  rheology and restart stay conflicts at `dynamic_mesh.enabled`.

With `deformation = "layering"`:

* E6 `solver.algorithm` not `compressible`: at `dynamic_mesh.deformation` (layering on the incompressible path is F-257).
* E7 `mesh.source = "file"`: at `mesh.source`; `mesh.generator` not `box_3d`: at `mesh.generator`.
* E8 `moving_patches` other than exactly `["xmax"]`: at `dynamic_mesh.moving_patches`.
* E9 `displacement_expression_y`, `displacement_expression_z` or `rotation_angle_expression` set: at that key.
* E10 `mesh.nx < 2`: at `mesh.nx`.
* E11 a non-empty `fixed_patches`: at `dynamic_mesh.fixed_patches` (smoothing only).
* E12 a mesh rotation (`mesh.transform.rotate_z_deg` not a multiple of 360): at `mesh.transform.rotate_z_deg`.

With `mesh.generator = "box_3d"`:

* E13 `mesh.grading` other than 1, `mesh.periodic_x`, `mesh.periodic_y`: at that key (build).

**W6. The CLI.** No change to `rcfd-cli`. `rcfd validate` and `rcfd run` read the case through the tree; X1 and X2 check them.

**W7. Not built.** The F-257 and F-258 library work and its wiring (prism or unstructured layering, layering with
`piso` / `pimple`, characteristic conditions on moving faces, the mesh-speed time step); six-DOF with the compressible
solver; more than one moving layered boundary; non-planar layers; layer thresholds as keys (Q1); `InverseVolume` smoothing
(Q2); a cell estimate for `box_3d` in the Studio and the workspace's start plan (Q4); restart of a moving-mesh run (refused, as
before).

## Checks and limits (fixed here, before any code)

Set-ups (case text inline in the tests; no case file, fixture or docs TOML block is added):

* **P-T**, ADR 0090's piston P through the tree: `box_3d`, `nx, ny, nz = 40, 2, 2`, `lx, ly, lz = 1, 0.1, 0.1`;
  `algorithm = "compressible"`, `end_time = 32`; `flux = "hllc"`, `reconstruction = "tvd_van_leer"`, `rk_order = 3`,
  `cfl = 0.4`, `gamma = 1.4`; initial `density = 1`, `pressure = 1`, velocity 0; no `boundaries` entry (every patch a wall,
  so a slip wall); `deformation = "layering"`, `moving_patches = ["xmax"]`,
  `displacement_expression_x = "-0.3 [m] * (1 - cos(pi * t / 16 [s]))"`. P-T1 is the same with `end_time = 16`.
* **P-B**, smoothing with a static characteristic boundary: `box_2d`, `nx = 20`, `ny = 2`, `lx = 1`, `ly = 0.1`; the solver
  keys of P-T with `end_time = 1`; `boundaries.xmin`: `compressible_bc = "pressure_outlet"`, `pressure_value = 1`;
  `deformation = "smoothing"`, `moving_patches = ["xmax"]`, `fixed_patches = ["xmin"]`,
  `displacement_expression_x = "-0.2 [m] * (1 - cos(pi * t / 2 [s]))"`.
* **P-C**, a rigid motion with uniform flow: `box_2d`, `nx = 20`, `ny = 4`, `lx = 1`, `ly = 0.2`; the solver keys with
  `end_time = 2`; initial `velocity = [0.3, 0.1, 0]`, density 1, pressure 1; `xmin`, `xmax`, `ymin`, `ymax`:
  `compressible_bc = "fixed"` with that state; `deformation = "rigid"`, no moving patch,
  `displacement_expression_x = "0.05 [m] * sin(2 * pi * t / 1 [s])"`,
  `displacement_expression_y = "0.02 [m] * sin(2 * pi * t / 0.5 [s])"`,
  `rotation_angle_expression = "0.1 * sin(2 * pi * t / 1 [s])"`, `rotation_origin = [0.5, 0.1, 0]`.
* **P-X**, the CLI piston: P-T with `nx, ny, nz = 20, 1, 1`, `end_time = 4`,
  `displacement_expression_x = "-0.3 [m] * (1 - cos(pi * t / 4 [s]))"` (peak piston Mach 0.2), and the reports `rho_total`
  (`volume_integral` of `rho`), `p_mean` (`volume_average` of `p`) and `p_max` (`maximum` of `p`).

**The library run** of a set-up is a loop in the test written with library calls only (`LayerStack`, `CompressibleSolver`
with `CompSettings::default()` and `Gas { gamma: 1.4 }`, `set_bc`, `step_ale`, `remap`, `replace_mesh`,
`rcfd_fv::smoothing::smooth_displacement`, `rcfd_mesh::motion::{moved_mesh, patch_points, rotation_matrix, mat_vec}`), with
the tree's schedule: `dt = max_dt()` clipped to the end time, the motion at `t + dt`. The motion is evaluated from the case's
expression text, compiled with `rcfd_core::expr` in the context of `rcfd_api::exprs::spatial_context`, so the piston
position has the same bits on both sides (an ulp at a threshold could move a layer event by one step).

**Equality** of a tree run and a library run means: the same number of steps; the same cell count after every step; and at
the end `max |rho_tree - rho_lib| <= 1e-12 max rho_lib`, `max |p_tree - p_lib| <= 1e-12 max p_lib`,
`max |u_tree - u_lib| <= 1e-12 c0` (`c0 = sqrt(1.4)`, every component) and `|t_tree - t_lib| <= 1e-12 t_lib`.

| Id | Check | Limit |
|---|---|---|
| K1 | `build_mesh` of P-T and of P-X against `LayerStack::uniform(...).mesh()`: `content_hash` and every point's bits | equal |
| A-eq | P-T: tree run against its library run | equality |
| A-1 | P-T1 tree run: `p_bar V^1.4 / (p0 V0^1.4) - 1` at `t = 16` (`p_bar` the volume mean of `p`) | `abs <= 2e-2` (ADR 0090 A1) |
| A-2 | P-T tree run: the same at `t = 32` | `abs <= 2e-2` (ADR 0090 A2) |
| A-B2 | P-T tree run: every step's mass drift (the step summary's `continuity`) | `<= 1e-12` (ADR 0090 B2) |
| A-B3 | P-T tree run: every step, `sum V` of `Simulation.geom` against `0.01 x_p(t)` | relative `<= 1e-13` (ADR 0090 B3) |
| A-B4 | P-T tree run: removals and additions (cell count down or up by 4 in a step) | each `>= 5` (ADR 0090 B4) |
| A-B5 | P-T tree run: a removal in the step after an addition, or the reverse | none (ADR 0090 B5) |
| B-eq | P-B: tree run against its library run | equality |
| B-0 | P-B tree run: the points of `xmin` after the run against the initial ones | equal bits |
| C-eq | P-C: tree run against its library run | equality |
| C-gcl | P-C tree run: `max(abs(rho - 1), abs(u - u0) / abs(u0), abs(p - 1))` over cells and components at the end | `<= 1e-12` (ADR 0090 C1) |
| L | E1 to E13, each made from one of P-T, P-B, P-C by one change: a diagnostic at the stated path; the three set-ups themselves: none | as stated |
| R1 | P-T with `nx, ny, nz = 4, 1, 1`, `displacement_expression_x = "-0.9 [m] * (1 - exp(-t / 1e-6 [s]))"`: the run stops with a failure that starts with `dynamic_mesh:` and contains `layering` | as stated |
| X1 | `rcfd run` of P-X (case written to a temp folder, `--out`, `--quiet`): exit 0; the last row of `monitors.csv` against an in-process `Simulation::run` of the same text: `rho_total`, `p_mean`, `p_max` | relative `<= 1e-12`; `solution.vtu`'s cell count equals the in-process final count, and that is below 20 |
| X2 | `rcfd validate` of P-X with `moving_patches = ["xmin"]`, and with `motion = "six_dof"`: exit 3, stderr names `dynamic_mesh.moving_patches` (with its line) and `dynamic_mesh.motion` | as stated |
| D0 | the three hashes of the table above | equal (Windows; printed elsewhere, ADR 0137) |
| D1 | `baseline_hashes.rs` and `case_hashes_w48u.rs` (H1) | pass unedited |
| D2 | `rcfd-api` `dynamic_mesh.rs`, `exp_rot_pab.rs`, `exp_rot_pc.rs`; `rcfd-fv` `layering_w38c.rs`, `exp_rot_pab.rs` | pass unedited |

**Can fail.** Two mutations, each made, run, reverted, and the file compared byte for byte with the kept one: M1, the tree's
layer check skipped (no `adjust`), must fail A-eq; M2, the moving compressible step calls the fixed-mesh `step` in place of
`step_ale`, must fail C-gcl.

**Gate.** fmt per package, clippy `-D warnings`, every test in release on a `git archive` copy of the gated commit
(`scripts/gate_test.py`, one process per test, three jobs). `scripts/limit_audit.py` is run on the new test files.

**Decision rule.** Every check met: F-259 is set to partial (`[~]`) with the reason that the case tree and the CLI run
ADR 0090's library, while the wiring of F-257's and F-258's library work waits for that work; done or partial is the
maintainer's call (Q3). A miss: an ignored `*_known_miss` test with the limit intact and the number recorded, F-259 partial
with the miss named, or todo when no tree run works at all.

## Open questions

**Q1. Layer thresholds as keys.** (a) Fixed at ADR 0090's registered 0.4 and 1.6, with `h0 = lx / nx` (no key, so no hash or
H1 edit). (b) Two keys under `dynamic_mesh`, each with `.hash_omit_default()` and an entry in H1's `DEFAULT_KEYS`, a
registered test edit as in ADR 0183 P-AB. Recommendation (a) until a case needs other thresholds. Proceeding on (a);
reversible by adding (b) later.

**Q2. The `InverseVolume(k)` smoothing stiffness of ADR 0090 D4.** It is library work of F-230 that is not in the tree, and
F-259 names only layering and compressible ALE. (a) Leave it out. (b) A third value of `dynamic_mesh.smoothing.method`
(`inverse_volume`, exponent from `smoothing.exponent`), an enumeration value and no key. Recommendation (b) as a separate small
item. Proceeding on (a).

**Q3. Roadmap status.** (a) Partial until F-257 and F-258 exist and are wired. (b) Done: the acceptance as written is met for
the library that exists, and F-257 and F-258 gain a wiring clause in their own acceptance. Recommendation (b) if every check is
met, because "can ship first" anticipates that the later library work wires itself. This is the maintainer's call; the
roadmap is set to partial (decision rule).

**Q4. `box_3d` in the Studio and the workspace's start plan.** Their generator lists give no cell estimate for it (item 5).
The Studio files are in the maintainer's worktree `studio-case-lineage`. (a) Add `box_3d` to those lists after that worktree
lands. (b) Leave it. Recommendation (a). Proceeding with no change.

## Where to go next

* [ADR 0090](0090-wave38c-dynamic-mesh-layering.md) for the library and its measured checks.
* [ADR 0033](0033-wave20b-dynamic-mesh.md) for the `dynamic_mesh` section.
* [Dynamic mesh](../dynamic-mesh.md) and [case schema](../case-schema.md) for the keys.

## Amendment 1 (2026-11-05, before the mutation runs it covers)

M2 cannot fail C-gcl as registered. A rigid motion keeps every cell volume, and the faces of a closed cell sum to zero area
on any mesh, so a fixed-mesh step of a uniform state with fixed-state boundaries keeps it uniform on the moved mesh as well
as the ALE step does. M2 must instead fail B-eq (the smoothing piston, whose cells change volume and whose moving wall
compresses the gas only through the ALE fluxes). Nothing else changes; C-gcl stays a check of the ALE step's discrete GCL,
not of the mutation.

## Amendment 2 (2026-11-05, before the run it covers)

M1 as registered (the tree's layer check skipped) was run once and did not end. Without removals the piston layer shrinks
towards the last static plane, the explicit step shrinks with it, and the A run stepped on without reaching its end time.
It was stopped after about 4 minutes of CPU time (the unmutated A run takes 12 s for the tree and the library together),
the file was restored and compared byte for byte with the kept copy (equal). M1 can therefore not fail A-eq with a result;
it only keeps the test from ending. A terminating mutation is added and is the check of record: **M1b**, a removal gives
the merged cell the state of its first source cell instead of the volume-weighted remap (an addition is unchanged), which
must fail A-eq. Nothing else changes.
