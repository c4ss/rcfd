# ADR 0342: F-440 registration: the force, moment and force-coefficient reports of a moving-mesh case with the wall pressure of the kept fixed-flux gradient (opt-in, `[dynamic_mesh.force_reports] wall_pressure`)

Status: **REGISTERED 2026-11-12**, written before any code of F-440 and before any run of its checks. Not yet accepted by
the maintainer. Before this text no check was run and nothing was compiled: the code and documents under "What was read"
were read and the arithmetic under "Arithmetic before this registration" was done by hand. Results go in the addendum at
the end; nothing above the addendum is changed after a run.

Parents: [ADR 0269](0269-f358-registration-wall-pressure-of-the-body-force-case-key.md) (F-358: the key
`wall_pressure` of `[dynamic_mesh.six_dof.coupling]`, the kept vector of `ale::set_follow_conditions` merged with
`set_rest_conditions`, `Simulation::dynamic_wall_gradients`, AU1b and its finding that the case tree's force reports are
low by `1/(2N)` on moving walls, question Q3 from which this item comes), [ADR
0252](0252-f351-registration-added-mass-deficit-wall-pressure.md) (F-351: `sixdof::patch_force_moment_wall_gradient`, why
the default wall pressure is first order on a wall with a prescribed gradient), [ADR
0266](0266-f350-registration-strong-body-flow-coupling-case-key.md) (F-350: why keys go into optional tables, read 6).
Roadmap item F-440 (depends on F-358).

Number: the lead gave 0342 to F-440. Branch `f440-moving-wall-force-reports` from main 7d87bc00, worktree `w49i-work/f440`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run; a missed limit becomes an
ignored `*_known_miss` test with the limit intact; a defective set-up is corrected by an amendment before the corrected
run; each registered run made once; defaults bit-identical; `baseline_hashes.rs`, the hash tables and the goldens not
edited; no existing test edited; no new crate, no unsafe code; `pimple.rs`, `pimple/*.rs` and `overset.rs` not edited.
Planned edits: `crates/rcfd-api/src/sim.rs`, `crates/rcfd-api/src/dynmesh.rs`, `docs/case-schema.md`,
`docs/dynamic-mesh.md`, one new test file. No line of `rcfd-fv` or `rcfd-post` is planned to change. Clean-room: no new
algorithm. The corrected wall pressure is ADR 0252's (a Taylor expansion from the wall-cell centre to the face with the
normal part of the cell gradient replaced by the prescribed gradient, derived there); this item routes it into the reports.

## What F-440 asks

The acceptance of `docs/roadmap.json`, binding in full:

> An opt-in key makes the force, moment and force-coefficient reports of a moving-mesh case take the wall pressure of the
> kept fixed-flux gradient (the face pressure of sixdof::patch_force_moment_wall_gradient) on the following and resting
> walls; on the rigid column of ADR 0269 AU1b the report's force deficit falls from 2.5e-2 to at most 1e-4; without the key
> every report value is bit-identical; registered before the run.

Clauses: (c1) an opt-in key; (c2) with it the force, moment and force-coefficient reports take the face pressure of
`patch_force_moment_wall_gradient` on the following and the resting walls; (c3) on AU1b the report's force deficit falls
from `2.5e-2` to at most `1e-4`; (c4) without the key every report value bit-identical; (c5) registered before the run.
The lead's brief adds: settle where the key lives (an optional table, ADR 0266 and 0269), confirm by reading that no
golden or hash-table edit is needed, K0 of ADR 0269 passes unedited; the default stays (ADR 0269 Q1 is the maintainer's);
heavy runs as ignored records with a fast test.

## What was read (main 7d87bc00, read only)

1. ADR 0269 in full (D1 to D4, AU1, amendments 1 and 2, AU1b: the box starting at rest, PISO, `dt = 0.01`, steps 3 to 12,
   the report's pressure part low by `2.500460e-2`, the corrected body force by `4.37e-6`; X1: with the step converged the
   pressure-only deficits are `1.250000e-2`, `3e-9` to `4e-9` and `2.500000e-2`; T5 and Q3; the listing L and LS). ADR 0252
   on the cause (the least-squares gradient of the wall cell sees the stored zero-gradient wall value, so its normal part
   is half the true one; the default extrapolation misses a quarter cell of `g`, the stored value `p_c` half a cell).
   ADR 0266 read 5 and 6 (`coerce` fills every absent non-optional field; an absent optional object stays out of the data
   at any depth and `get` reads its schema defaults; a plain key needs an H1 `DEFAULT_KEYS` entry).
2. `crates/rcfd-fv/src/sixdof.rs`: `face_loads` computes, on a face with a finite entry `g` of the vector, `pf = p_c +
   G_c . d` then `pf += (g - G_c . n)(d . n)`, `G = least_squares(geom, p.values, p.boundary_values(geom))`, `d = x_f - x_c`,
   `n = S/max(|S|, 1e-300)`; empty faces skipped; `patch_face_forces(.., nu, .., Some(w))` returns `rho (pf S - nu ...)` per
   face (with `nu = 0`, `rho = 1`: `scale(S, pf)` scaled by 1, so each component is `S_k * pf`).
3. `crates/rcfd-api/src/sim.rs`: `eval_report` (`ReportKind::Force | Moment | ForceCoefficient` take
   `ig::wall_force_and_moment` or, for the effective viscosity, `ig::wall_force_and_moment_face_viscosity`, both with
   `&self.pressure_face_values()`); `pressure_face_values` is the boundary value of `pressure_bc(geom, u, p)` (zero gradient
   on every fixed-velocity face, so `p_c` on a moving wall); `ReportDef::from_case` (`reports` is `Schema::named(report)`,
   every report's fields filled by `coerce`); `eval_all_reports`, `report_values`; the dynamic-mesh step sets `self.geom =
   s.geom.clone()` on success; `dynamic_wall_gradients`.
4. `crates/rcfd-post/src/integrals.rs` `force_and_moment`: the pressure part is `scale(S_f, rho p_face[i])` summed over
   `patch_faces` in order; the moment `(x_f - origin) x dF`.
5. `crates/rcfd-api/src/dynmesh.rs`: the loose step copies the merged follow-and-rest vector into `wall_dpdn` before the
   flow step (prescribed and six-DOF motion alike); the strong step leaves the last iterate's vector; the layered step
   (`step_layered`) keeps none; the compressible run never calls `DynMesh::step`. `check`, `check_coupling`, the optional
   tables `six_dof.coupling` and `compressible`.
6. **Who else reads `pressure_face_values` or `wall_force_and_moment`**: `boundary_field("p")` (area reports of `p`, not
   in this item), `design/adjoint.rs` and `rcfd-post` `turbo` (static walls). Not changed.
7. **Which existing inputs combine a moving mesh and a force, moment or force-coefficient report** (a search of every
   file under `crates/*/tests`, `crates/*/src`, `cases/` and `docs/` holding `[dynamic_mesh]` for `type = "force"`,
   `"moment"`, `"force_coefficient"`): **none**. So the default could flip without moving any existing test value or
   hash; that is still the maintainer's decision (it changes what a user's case reports).

### Where the key lives, and why no golden or hash-table edit is needed (by reading)

- A key per report inside `[reports.<n>]` is filled into every report of every case by `coerce` (read 3), so both texts
  of every case with a report gain a line: an H1 `DEFAULT_KEYS` edit (ADR 0266 read 6). Not available.
- A plain key of `dynamic_mesh` is filled likewise. Not available.
- The key of ADR 0269, `[dynamic_mesh.six_dof.coupling] wall_pressure`, is the body force's and is refused with a
  prescribed motion (ADR 0269 D3), while AU1b, the acceptance's case, is a prescribed motion. Not it.
- **An optional table `[dynamic_mesh.force_reports]`**: absent from every case that does not write it, so the settings
  hash, `to_toml`, `to_json_string` and the mesh of every existing input are unchanged and no `DEFAULT_KEYS` entry is
  needed (ADR 0266 read 6, ADR 0269 read 7, the same route as `[dynamic_mesh.compressible]` and `six_dof.coupling`). It
  sits under `dynamic_mesh` because the kept vector exists only with a moving mesh. The goldens
  (`crates/rcfd-mcp/tests/golden`, the CLI help) hold no case-schema key (ADR 0266 read 6; re-checked: no golden names
  `mesh_speed` or `torsion_rest_angle`). A new TOML block in `docs/case-schema.md` is printed by H1 as new and does not fail
  it; no existing TOML block is edited. K0 checks this reading.

## Arithmetic before this registration (disclosed)

- **The report with the key equals the corrected body force's pressure part.** With the key, the report's face pressure
  on a face with a finite entry is `pf` computed by the operations of read 2 in the same order on the same `p`, the same
  geometry (`self.geom` is the solver's after a step, read 3) and the same vector, so it is the same bits. Then
  `scale(S, 1 * pf)` (report, `rho = 1`) and `scale(scale(S, pf), 1)` (sixdof) are equal bit for bit per face, and the sums
  over the faces of `xmin` in the same order are equal: the report's pressure part equals
  `patch_force_moment_wall_gradient(.., nu = 0, ..)` bit for bit (`rho = 1` in every case below).
- **AU1b with the key.** The report's pressure-part deficit is then the pressure-only corrected deficit of the PISO run.
  ADR 0269 measured `4.37e-6` worst over steps 3 to 12 for the corrected force with its viscous part, and `3e-9` for the
  pressure-only corrected deficit with the step converged; estimate a few `1e-6`, against the limit `1e-4` of the
  acceptance. Without the key the run and the report are AU1b's: `2.500460e-2` (printed to 7 digits there).
- **The total, the moment and the coefficient.** The viscous part of the report (`rcfd_post`, `nu = 0.01`) on a plug flow
  moving with the wall is small: AU1b's default with its viscous part was `1.250471e-2` against X1's converged
  pressure-only `1.250000e-2`, a difference of `4.7e-6` that holds the viscous part and the PISO residual together.
  Estimate for the total force deficit with the key: below `1e-5`. The moment about the y axis through the origin is
  `M_y = sum_f z_f F_x,f`; the pressure is uniform over the four faces of `xmin` by the symmetry of the column (faces at `z
  = 0.05, 0.15`), so `M_y = 0.1 F_x` and its deficit against `0.1 F_exact` is the force's (to the viscous moment). The
  coefficient is `F/(0.5 rho u_ref^2 a_ref)`, linear in `F`: the same deficit.
- **A following wall instead of a wall at rest.** The same column with `moving_patches = ["xmin"]` (the wall follows the
  mesh) under pure translation along `x`: `set_follow_conditions` sets `u_b = w = (w_x, 0, 0)`, `set_rest_conditions`
  `(m_f/|S_f|) n = (w . n) n = (w_x, 0, 0)` on the face of normal `-e_x`. The wall data and the prescribed gradient are
  the same, so the deficits are expected equal to the resting wall's (not claimed bit for bit).
- **Cost.** AU1b: 12 PISO steps on 80 cells, under a second in release; every check below is of that size or smaller.
  Nothing heavy, so no ignored record is needed for the checks; the listing LS below runs in the scratch copy.

## Design

**D1. The key.** The optional table `[dynamic_mesh.force_reports]` with one key `wall_pressure`, values
`"boundary_value"` (default: the stored face value of the pressure condition, the reports as they are) and
`"prescribed_gradient"`. It acts on the `force`, `moment` and `force_coefficient` reports of every component.

**D2. The reports** (`sim.rs`). A new public `Simulation::report_pressure_face_values()`: `pressure_face_values()`, and
when the key is `prescribed_gradient` and the kept vector has one entry per boundary face, on every face that is not
empty and has a finite entry `g`, the face pressure of read 2 (`G` from `self.pressure()` and its own boundary values on
`self.geom`). The two wall closures of `eval_report` call it in place of `pressure_face_values()`. Without the key it
returns `pressure_face_values()` itself, so the report path computes the same operations: bit-identical. Before the first
step (the vector empty) the stored value is kept. `DynMesh` reads the key in `from_case` (`report_wall_gradient: bool`,
`DynMesh::report_wall_gradient()`).

**D3. Static checks** (`dynmesh::check`, the section enabled): an unknown value is refused by the schema's enumeration at
the key; `prescribed_gradient` with `deformation = "layering"` (the layered step keeps no vector) or with
`solver.algorithm = "compressible"` (no following or resting walls, and no force reports) is refused at
`dynamic_mesh.force_reports.wall_pressure`. Nothing while `dynamic_mesh.enabled = false`.

**D4. Documentation.** `docs/case-schema.md`: the row of the key; the sentence of ADR 0269's paragraph that says the
reports are not changed points to the new table; a new paragraph with a new TOML block (no existing block edited).
`docs/dynamic-mesh.md`: a short paragraph.

## Registered checks

New file `crates/rcfd-api/tests/force_reports_moving_wall_f440.rs`, release. `column(motion, table)` is AU1b's text
(ADR 0269 amendment 2: `box_3d` 20 x 2 x 2 of `[1, 0.2, 0.2]`, `nu = 0.01`, `rho = 1`, PISO with the tolerances of
ADR 0269, `dt = 0.01`, rigid, `displacement_expression_x = "0.01 [m] * (cos(2 * pi * t / 1 [s]) - 1)"`) with `xmin` at
rest (`rest_patches = ["xmin"]`) or following (`moving_patches = ["xmin"]`), plus five reports: `fp` (force, `xmin`,
component `pressure`, direction `[1, 0, 0]`), `ft` (the same, `total`), `my` (moment, `xmin`, direction `[0, 1, 0]`,
origin `[0, 0, 0]`), `cf` (force coefficient, `xmin`, `pressure`, `u_ref = 2`, `a_ref = 0.04`), `fo` (force, `xmax`,
`total`). The exact force is AU1b's `F_ex = -g_m L A` (`g_m` the mean kept entry over `xmin`, `L = 1`, `A = 0.04`); the
deficit of a report `R` is `1 - R/R_ex` with `R_ex = F_ex` (`fp`, `ft`), `0.1 F_ex` (`my`) and `F_ex/(0.5 x 4 x 0.04)`
(`cf`). "Worst" is over steps 3 to 12 (AU1b's window).

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| K0 (c4) | unedited: ADR 0269's K0 (`rcfd-api` `case_hashes_w48u`, `baseline_hashes`, `w48v_adaptive_correctors`, `rules_w19c`, `mesh_transform_w48u`, `physics_schema`, `tree_wiring_f259`, `layering_ale_f259`, `registers`, `dynamic_mesh`, `exp_rot_pab`, `adapt_case_f342`, `strong_coupling_case_f350`; `rcfd-cli` `cli`; `rcfd-mcp` `default_f1`, `project_list_f270`) and `wall_pressure_case_f358`, `force_effective_w49a`, `workflow` (laminar force reports) | all pass as before, ignored stay ignored, no file edited | reads 1, 7 |
| T0 (c1, c4) | the resting column without the table: no `force_reports` in `to_toml`, `to_json_string`; `get` reads `"boundary_value"`; with the table holding `wall_pressure = "prescribed_gradient"`: both texts hold it, the settings hash differs | as stated | ADR 0266 read 6 |
| Z0 (c4) | resting column, no table: before the first step and after each of 12 steps, every report value equals bit for bit the value computed in the test by the existing formula (`wall_force_and_moment` with `pressure_face_values`, `physics.nu`, `rho`; the dot with the unit direction; `force_coefficient`) | bit-identical, 13 evaluations x 5 reports | the default path unchanged |
| Z1 (c4) | resting column: a table with `wall_pressure = "boundary_value"` against no table: every report value and `u`, `p`, `phi` at step 0 and after every step | bit-identical | the default |
| Z2 | resting column: the key `prescribed_gradient` against no table: `u`, `p`, `phi` after every step (the key changes reports only) | bit-identical | D2 |
| E1 (c2) | resting column with the key, steps 1 to 12: on every face of `xmin`, `S_k * pf` with `pf` from `report_pressure_face_values` equals `patch_face_forces(geom, u, p, 1, 0, [xmin], Some(kept))` component `k` bit for bit (`k = 0, 1, 2`); on every other boundary face `report_pressure_face_values` equals `pressure_face_values` bit for bit | bit-identical | arithmetic |
| E2 (c2) | the same run: `fp` against the x force of `patch_force_moment_wall_gradient(.., nu = 0, ..)` | relative `<= 1e-14` | 0 |
| E3 (c4 on other patches) | the same run: `fo` (the outlet, no kept gradient) with the key against no table | bit-identical | NaN entries skipped |
| **A1 (c3)** | resting column, the key: worst deficit of `fp` | `abs(.) <= 1e-4` (acceptance) | a few `1e-6` |
| A1c (control of A1) | resting column, no table: worst deficit of `fp` | within `1e-8` of `2.500460e-2` (AU1b), and `> 1e-4` | `2.500460e-2` |
| A2 (c2, total) | resting column, the key: worst deficit of `ft` | `abs(.) <= 1e-4` | `< 1e-5` |
| A3 (c2, moment) | resting column, the key: worst deficit of `my`; no table: within 1 percent of `2.5e-2` | `abs(.) <= 1e-4`; as stated | the force's |
| A4 (c2, coefficient) | resting column, the key: worst deficit of `cf`; no table: within 1 percent of `2.5e-2` | `abs(.) <= 1e-4`; as stated | the force's |
| F1 (c2, following wall) | following column, the key: worst deficits of `fp`, `ft`, `my`, `cf`; no table: worst of `fp` | `abs(.) <= 1e-4` each; within 1 percent of `2.5e-2` | as A1 to A4 |
| N (D3) | `wall_pressure = "bogus"`: a diagnostic at `dynamic_mesh.force_reports.wall_pressure`; the key with `deformation = "layering"` and with `solver.algorithm = "compressible"`: that path among the diagnostics; the resting column with the key: none; the resting column with the key and `enabled = false`: none | as stated | D3 |
| LS (listing, in the scratch copy) | the default flipped in `from_case` (MU3 below): `baseline_hashes`, `case_hashes_w48u`, `dynamic_mesh`, `strong_coupling_case_f350`, `wall_pressure_case_f358`, `force_effective_w49a`, `workflow`: passes or fails with the number | printed | all pass (read 7) |

Printed, no limit: every deficit per step, the mean kept gradient.

### Negative controls

A1c is the control of A1 (the same run without the key misses A1's limit by a factor 250); the no-table halves of A3, A4,
F1 likewise. Z2 and E3 show that the key does not reach the flow or the faces without a kept gradient.

### Mutations (scratch copy, not committed)

Each applied alone to a `git archive` copy of the final tree by an exact replacement that must occur once; the named
tests run once and must fail; the file restored and compared byte for byte.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | the reports ignore the key (`report_pressure_face_values` always the stored values) | E1, A1 |
| MU2 | the normal correction `(g - G . n)(d . n)` dropped | E1, A1 |
| MU3 | the default flipped (a case without the table takes `prescribed_gradient`) | Z0 |
| MU4 | the finite-entry filter removed (every face of the vector corrected) | E1, E3 |
| MU5 | the static check of layering and compressible removed | N |
| MU6 | the length guard removed (the vector indexed before the first step) | Z1 |

## Predictions (mine, before any run)

A1 a few `1e-6`; A1c `2.500460e-2`; A2 below `1e-5`; A3, A4 as A2 and A1; F1 the same as the resting wall to about
`1e-12`; Z0 to Z2, E1 to E3 bit-identical; LS: all pass. The check I trust least is A3 (the viscous moment and the
symmetry assumption).

## Decision rule

- **done**: K0, T0, Z0 to Z2, E1 to E3, A1, A1c, A2 to A4, F1, N met; LS run and listed; every mutation caught. The default
  stays as it is (the maintainer's).
- **partial**: K0 fails (the table taken out, the code and its tests kept, the schema step the maintainer's); or an
  acceptance check (A1, A1c, A2 to A4, F1) missed (kept as `*_known_miss`, diagnosed); or LS not run; or a mutation not
  caught.
- **todo**: the reports do not take the corrected wall pressure with the key.

T0, Z0, Z1, Z2 are preconditions: a miss means the change is not proposed for merging.

## Open choices taken

Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation); none changes a default.

- **T1. One table for all reports of the case** (`[dynamic_mesh.force_reports]`), not a key per report (read 3: a key per
  report needs an H1 edit; an optional sub-table per report is possible but adds a table to every report's schema for one
  key). Listed as question Q2.
- **T2. All components** (`total`, `pressure`, `viscous`, the effective and molecular variants) take the corrected face
  pressure; the viscous part is unchanged.
- **T3. The default is not changed** (ADR 0269 Q1, Q3; read 7 lists no existing input that would move).
- **T4. `boundary_field("p")`** (area reports of the face pressure) is not changed: the acceptance names force, moment
  and coefficient reports only. Listed as question Q3.

## Limits of this registration

One geometry (the rigid column in pure translation); no rotating wall, no smoothed mesh, no six-DOF body in the checks
(the six-DOF path keeps the vector by the same lines, ADR 0269 W1). A wall cell off the face normal is covered by ADR 0269
SK1 to SK3 for the same formula, not re-run here. The sliding step keeps no vector in the tree (ADR 0269 Q4) and is not
covered. The Studio and GUI editors read the schema and are not touched.

## Amendment 1 (2026-11-12, after the code was written, before any run of a check or a mutation)

**MU6 is an equivalent mutation** (seen by reading the code of D2, nothing run). `report_pressure_face_values` loops over
the entries of the kept vector, so an empty vector (before the first step, or the layered step) corrects no face with or
without the length guard; the tree never holds a non-empty vector of another length (the vector is written only by the
loose and strong steps on the mesh of the step, and a mesh-changing step keeps none). No check can catch MU6 through the
tree. It stays in the table and is run once as registered; its result is recorded. The decision rule's "every mutation
caught" is read as "every mutation that is not equivalent caught", MU6 counted as equivalent if it survives. No check,
limit or other mutation is changed.

## Addendum 1: results (2026-11-12)

### What was built

- `crates/rcfd-api/src/dynmesh.rs`: the optional table `[dynamic_mesh.force_reports]` with the key `wall_pressure`
  (`boundary_value` default, `prescribed_gradient`) in the schema of `dynamic_mesh` (D1); `check_force_reports`, called by
  `check` (D3); `DynMesh::report_wall_gradient`.
- `crates/rcfd-api/src/sim.rs`: `Simulation::report_pressure_face_values` (D2); the two wall closures of `eval_report`
  call it in place of `pressure_face_values`.
- `docs/case-schema.md`: the row of the key, one sentence of ADR 0269's paragraph extended, a new section with a new TOML
  block (no existing block edited). `docs/dynamic-mesh.md`: one paragraph.
- `crates/rcfd-api/tests/force_reports_moving_wall_f440.rs` (new): 6 tests, all active, about 0.7 s in release; nothing
  heavy, so no ignored record.
- Not changed: `rcfd-fv`, `rcfd-post`, `schema.rs`, `comp.rs`, `rcfd-cli`, `rcfd-mcp`, `baseline_hashes.rs`, the hash
  tables, the goldens, every existing test.

### Checks

Runs, each once: the new file on 7fe28ee9 (`cargo test --release -p rcfd-api --test force_reports_moving_wall_f440 --
--nocapture --test-threads=1`, 6 passed in 0.73 s; the log is `w49i-work/f440-scratch/run1.log`, not committed); K0 on
7fe28ee9 in release; the mutations and LS on a `git archive` copy of 7fe28ee9 (its test file replaced by the formatted
one of c18cda2f, which differs only by `cargo fmt`).

| Check | Limit | Measured | Met |
| --- | --- | --- | --- |
| K0 (c4) | all pass, no file edited | `rcfd-api`: `adapt_case_f342` 6 (2 ignored), `baseline_hashes` 3 (1 ignored), `case_hashes_w48u` 1, `dynamic_mesh` 5 (1 ignored), `exp_rot_pab` 6, `force_effective_w49a` 4, `layering_ale_f259` 7 (1 ignored), `mesh_transform_w48u` 13, `physics_schema` 11, `registers` 7, `rules_w19c` 8 (1 ignored), `strong_coupling_case_f350` 9 (1 ignored), `tree_wiring_f259` 9, `w48v_adaptive_correctors` 5 (4 ignored), `wall_pressure_case_f358` 11 (3 ignored), `workflow` 5; `rcfd-cli` `cli` 8; `rcfd-mcp` `default_f1` 2, `project_list_f270` 1; all passed, the ignored as before | yes |
| T0 (c1, c4) | as stated | no `force_reports` in either text without the table, `get` reads `boundary_value`; with the table present; settings hash `d9b06e9617131355` against `09caf29135c09c6e` | yes |
| Z0 (c4) | bit-identical, 13 x 5 | 0 of 13 evaluations differ | yes |
| Z1 (c4) | bit-identical | 0 of 13 differ (reports and flow); step 0 with the key equals no table | yes |
| Z2 | bit-identical | 0 of 13 differ | yes |
| E1 (c2) | bit-identical | 0 differing values over 12 steps (wall faces, 3 components; every other boundary face) | yes |
| E2 (c2) | `<= 1e-14` | `0` (bit for bit, as derived) | yes |
| E3 | bit-identical | 0 of 12 steps differ | yes |
| **A1 (c3)** | `abs <= 1e-4` | **`4.597e-6`** | yes |
| A1c (control) | within `1e-8` of `2.500460e-2`, `> 1e-4` | `2.5004597e-2` | yes |
| A2 (total) | `abs <= 1e-4` | `4.369e-6` | yes |
| A3 (moment) | `abs <= 1e-4`; no table 1 % of `2.5e-2` | `4.369e-6`; `2.5004369e-2` | yes |
| A4 (coefficient) | `abs <= 1e-4`; no table 1 % of `2.5e-2` | `4.597e-6`; `2.5004597e-2` | yes |
| F1 (following wall) | `abs <= 1e-4` each; no table 1 % | `cf`, `fp` `4.597e-6`, `ft`, `my` `4.369e-6`; `2.5004597e-2` (the resting wall's numbers to the printed digits; at step 3 `1.2753401e-6` against `1.2753400e-6`) | yes |
| N (D3) | as stated | bogus: the key only; layering and compressible: the key among the diagnostics; valid and disabled: none | yes |
| LS | printed | with the default flipped: `baseline_hashes` 3, `case_hashes_w48u` 1, `dynamic_mesh` 5, `strong_coupling_case_f350` 9, `wall_pressure_case_f358` 11, `force_effective_w49a` 4, `workflow` 5: all pass | run |

The moment's deficit equals the total force's to the printed digits (the uniform wall pressure of the derivation). The
total's `4.369e-6` is ADR 0269 AU1b's corrected body force (`4.37e-6`): the two now share the face pressure and differ
only in the viscous formula, which is small here.

### Mutations

Each applied alone to the copy by an exact replacement that had to occur once (`f440-scratch/mutate.py`), the named tests
run once, the file restored and compared byte for byte (equal each time).

| Id | Must fail | Result |
| --- | --- | --- |
| MU1 the reports ignore the key | E1, A1 | both failed |
| MU2 the normal correction dropped | E1, A1 | both failed |
| MU3 the default flipped | Z0 | failed |
| MU4 the finite-entry filter removed | E1, E3 | the test of E1 to E3 failed (at E1, its first assertion) |
| MU5 the static check removed | N | failed |
| MU6 the length guard removed | Z1 | **survived**: equivalent, as amendment 1 found by reading before any run |

### Deviations and disclosures

- Amendment 1 (before any run): MU6 is an equivalent mutation; the decision rule counts it as such. It survived.
- E1 compares a zero area component as a value (`0.0 == -0.0`), not as bits: the sign of a zero product can differ
  between `S_k pf` and `rho (S_k pf - 0 t)`; every non-zero component is compared bit for bit. Written in the test before
  the run.
- `cargo fmt` reformatted the new test file after the run (c18cda2f, no change of code).

### Not run

The whole-workspace suite and the gate (the lead's); the debug profile of the new file; a six-DOF body, a smoothed or
rotating moving wall with the key (the vector is kept by the same lines, ADR 0269 W1); `rcfd run` on a case with the key.

### After the tests

`cargo fmt -p rcfd-api --check`: clean. `cargo clippy --release -p rcfd-api --all-targets -- -D warnings`: clean.

### Decision by the registered rule

K0, T0, Z0 to Z2, E1 to E3, A1, A1c, A2 to A4, F1 and N are met; LS ran and is listed; MU1 to MU5 failed their named tests
and MU6 is equivalent (amendment 1). **done**, proposed as such; the default stays `boundary_value` (the maintainer's).

### Remaining questions (each with options and a recommendation)

- **Q1. The default.** No existing input combines a moving mesh and a force report (read 7) and LS moves no test, so a
  flip would change only what new cases report. Options: (a) keep `boundary_value`; (b) flip to `prescribed_gradient`
  together with ADR 0269 Q1. Recommendation (b) once the maintainer approves. Not taken (it changes a default result).
- **Q2. The granularity.** Options: (a) one table for the case (built); (b) an optional sub-table per report.
  Recommendation (a).
- **Q3. `boundary_field("p")`** (area averages and integrals of `p` on a moving wall) keeps the stored value. Options: (a)
  leave (the acceptance names force reports); (b) extend the key to it. Recommendation (a) until it is needed.

### For the lead: roadmap entries

Proposed status of F-440: **done** (the default the maintainer's).

Note text for F-440:

> From ADR 0269 question 3, recommendation taken on the maintainer's standing instruction of 2026-11-11. DONE (ADR 0342, 2026-11-12; registered before the code, one amendment before any run). The optional table [dynamic_mesh.force_reports] with wall_pressure = "prescribed_gradient" (default "boundary_value"; a key per report or in dynamic_mesh would be filled into every case by coerce and need an H1 hash-table entry, ADR 0266; the six_dof.coupling table is refused for prescribed motion) makes the force, moment and force-coefficient reports of a moving-mesh case take, on the following and resting walls, the face pressure of sixdof::patch_force_moment_wall_gradient with the kept fixed-flux gradient (Simulation::report_pressure_face_values), bit for bit per face. On the rigid column of ADR 0269 AU1b the report's force deficit falls from 2.5004597e-2 to 4.6e-6 (limit 1e-4); total force, moment and coefficient likewise (4.4e-6, 4.4e-6, 4.6e-6), the same with the wall following the mesh. Without the table every report value and the run are bit-identical, and the key does not change the flow. No existing input combines a moving mesh and a force report; with the default flipped no test moves. K0 unedited; five mutations caught, one equivalent. Open, the maintainer's: the default (ADR 0342 Q1, with ADR 0269 Q1).
