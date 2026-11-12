# ADR 0266: F-350 registration: the strong body-flow coupling selectable from the case tree (`[dynamic_mesh.six_dof.coupling]`)

Status: **REGISTERED 2026-11-12**, written before any code of F-350 and before any run of its checks. Not yet accepted by
the maintainer. Before this text nothing was compiled and no test was run: the code and documents under "What was read"
were read, and the arithmetic under "Arithmetic before this registration" was done by hand. Results go in the addendum at
the end; nothing above the addendum is changed after a run.

Parents: [ADR 0250](0250-f251-registration-strong-fluid-body-coupling.md) (F-251: `rcfd_fv::fsi::strong_step`,
`Relaxation`, `relaxation_from_added_mass`, the spring-mounted piston SPm and its checks S1 to S3, L1 to L3; its questions
Q2 and Q3, from which this item comes), [ADR 0252](0252-f351-registration-added-mass-deficit-wall-pressure.md) (F-351:
`strong_step_with`, `WallPressure`; its case key is F-358, a separate item, not done here),
[ADR 0211](0211-f244-registration-flow-coupled-six-dof-validation.md) (F-244: the spring-mounted piston SP),
[ADR 0033](0033-wave20b-dynamic-mesh.md) (D7, the loose coupling the case driver runs today), and
[ADR 0257](0257-f259-registration-tree-wiring-of-prism-and-incompressible-layering-and-moving-characteristic-faces.md),
[ADR 0262](0262-f342-registration-case-key-for-the-adaptive-steady-loop.md) (the route by which case keys were added
this morning: optional tables with no hash table or golden edited, and their K0). Roadmap item F-350 (depends on F-251).

Number: the lead gave 0266 to F-350. Branch `f350-strong-coupling-case-key` from main edfc0b9f, worktree `w49i-work/f350`.

House rules (`w49i-work/AGENT_RULES.md`):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the number recorded, and the cause is
  diagnosed, not tuned away; a defective set-up gets a corrected test by an amendment written and committed before the
  corrected run, and the old one stays as an ignored record;
- defaults stay bit-identical: a case without the new table gives the bytes it gave before;
- `crates/rcfd-api/tests/baseline_hashes.rs`, the hash tables (`case_hashes_w48u.rs`, `w48v_case_hashes.rs`) and the
  goldens are not edited or re-recorded; no existing test is edited;
- each registered run is made once, and nothing is re-run to pick a result;
- no new third-party crate, no unsafe code, the crate graph unchanged (`rcfd-api` already depends on `rcfd-fv`);
- `crates/rcfd-fv/src/pimple.rs`, `pimple/*.rs` and `overset.rs` are not edited (held by another branch); `fsi.rs` and
  `sixdof.rs` are not planned to change: the driver calls the public `strong_step` as it is;
- clean-room: nothing here is a new algorithm. The numerics are those of ADR 0250 (derived there); this item calls them
  from the case tree.

## What F-350 asks

The acceptance of `docs/roadmap.json`, binding in full:

> A case with dynamic_mesh.six_dof.coupling = "strong" (default "loose") and its relaxation, added-mass estimate,
> tolerance, force scale and iteration cap as keys runs the spring-mounted piston at a mass ratio of 4 through the case
> driver and reproduces ADR 0250 S1 to S3 (omega within 2 percent, no growth of same-side turning points over 10 periods,
> dt halving within 0.5 percent), the same case with coupling = "loose" stops with a located error or grows; a step that
> reaches the cap stops the run with a located message unless a key says to continue; every static check of the new keys
> is located; with the default every existing case hash is bit-identical.

Clauses: (a1) a key selecting strong (default loose), with keys for the relaxation, the added-mass estimate, the
tolerance, the force scale and the iteration cap; (a2) the piston at ratio 4 through the case driver reproduces S1 to S3;
(a3) the same case with loose stops with a located error or grows; (a4) a step at the cap stops the run with a located
message unless a key says to continue; (a5) every static check of the new keys is located; (a6) with the default every
existing case hash is bit-identical. The lead's brief adds: tree against library equality bit for bit for the strong
path; a case without the new keys bit-identical (`baseline_hashes`, `case_hashes_w48u`, the six-DOF tree tests
unedited); the route of ADR 0257 / 0262 and their K0; heavy runs as records run once with a fast test.

## What was read (main edfc0b9f, read only)

1. ADR 0250 in full (D3 to D8, SPm, S0 to S5, L1 to L5, Q1 to Q6, the addendum's numbers: S1 `1.005081` at `T/100`,
   `1.005063` at `T/200`, turning ratios `0.92452` and `0.96141`, loose `10 X` at steps 28 and 31, ends at 34 and 40;
   3.0 iterations a step). ADR 0252's summary and `strong_step_with`. ADR 0211's SP. ADR 0257 read 3 and K0, ADR 0262
   read 4 and K0.
2. `crates/rcfd-fv/src/fsi.rs` in full: `strong_step(flow, body, dt, &BodyCoupling, &StrongCoupling, points)`;
   `StrongCoupling::validate` (factor in `(0, 2)`, Aitken cap `>= initial`, tolerance `> 0`, scales `>= 0` and finite,
   `max_iterations >= 1`); at the cap the step is kept and `converged = false`; a failure leaves flow and body at `t_n`;
   it refuses `corrector_control`; it calls `set_follow_conditions` only (no `set_rest_conditions`).
   `crates/rcfd-fv/tests/strong_fsi_f251.rs` (SPm: `box_mesh([20, 2, 2], [1, 0.2, 0.2])`, the piston `xmin`, `xmax`
   fixed with `p = 0`, four slip sides, Laplacian smoothing with the piston at the body's `x`; the crossings and
   turning-point helpers).
3. `crates/rcfd-api/src/dynmesh.rs` in full: the schema of `dynamic_mesh` (`six_dof` is a plain object), `check`,
   `DynMesh::from_case`, `DynMesh::step` (the loose loop: `patch_force_moment` at `t_n`, `Body::advance`, `new_points`
   (for a body `map_point` of the initial points, then the smoothing with the moving patches prescribed), `move_mesh`,
   `set_follow_conditions`, the rest conditions, `step_ale_with_walls`). `Body::map_point` maps an initial point by the
   body's pose relative to its initial pose, so the case's mesh is the mesh at the body's initial position.
4. `crates/rcfd-api/src/sim.rs`: `build_mesh` (`box_3d` is `gen::box_mesh(.., Uniform)`, the SP mesh), the transient
   settings (`PimpleSettings::piso(nu)` with the case's convection, `n_correctors` 2, the linear-solver keys), the
   refusals (`solver.corrector_control = "adaptive"` is already refused with `dynamic_mesh`), the transient step's
   `dynmesh` branch (an `Err` of `DynMesh::step` becomes `failure_message(step, time, e)` and stops the run; on `Ok`
   `self.geom` is updated), `dynamic_body`, `pimple_solver_mut`, `take_runtime_warnings`.
5. `crates/rcfd-core/src/settings.rs`: `coerce` fills every absent non-optional field with its default and leaves an
   absent optional object out (ADR 0155); `get` into an absent optional object reads the schema defaults; `set` creates
   it. `content_hash` omits `hash_omit_default` fields at their default, `to_json` / `to_toml` do not.
6. **Whether a key can be added without a hash-table or golden edit** (the brief: confirm by reading).
   - A new plain key inside the existing table `dynamic_mesh.six_dof` (for instance `coupling = "loose"`) is filled
     with its default into the data of every case by `coerce` (read 5), so `Case::to_toml` and `to_json_string` of every
     existing case gain a line. `hash_omit_default` would keep the settings hash but not the two texts; H1
     (`case_hashes_w48u.rs`) compares both texts with R0 and accepts a default only through an entry in its
     `DEFAULT_KEYS` (ADR 0163, 0181, 0183, 0190, 0193 each edited that table). **So a new key inside `six_dof` is not
     hash-omitted when absent in the sense H1 needs: it needs a hash-table edit, which is not available here.**
   - An optional table (`Schema::optional`) nested in `six_dof` stays out of the data of every case that does not
     write it, at any depth (`coerce` is recursive; `get` reads defaults through it), so the settings hash, both texts
     and the mesh of every existing input are unchanged; no `DEFAULT_KEYS` entry is needed. A new source (a new TOML
     block in `docs/`) is printed by H1 as new and does not fail it; an existing TOML block of `docs/` must not be
     edited (H1 keys a block by the hash of its text).
   - `baseline_hashes.rs` hashes solver results of cases without the table. The goldens (`crates/rcfd-mcp/tests/golden`)
     hold no case-schema key (no file in the repository other than `dynmesh.rs` and `docs/case-schema.md` names
     `torsion_rest_angle`, the last key of `six_dof`). `rules_w19c` E2 evaluates the TOML of `cases/`, of
     `docs/case-schema.md` and of test literals holding `[mesh]` with the setup rules; the new texts are plain channels
     with no rule of `setup_rules.json` applying by reading; it is in K0.
   - **Conclusion (by reading): the settings go in an optional sub-table `[dynamic_mesh.six_dof.coupling]`, whose key
     `scheme` takes `"loose"` (default) or `"strong"`.** The acceptance's `dynamic_mesh.six_dof.coupling = "strong"` is
     therefore written `[dynamic_mesh.six_dof.coupling] scheme = "strong"` (a TOML key cannot be both a string and a
     table); absent table = `scheme = "loose"`. K0 below checks the reading; if K0 fails, the table is taken out, the
     driver and its tests stay, and the schema step is reported as the maintainer's.

## Arithmetic before this registration (disclosed)

- **The case of SPm in the tree.** `box_3d` starts at `x = 0`, and the tree's mesh is the mesh at the body's initial
  position (read 3), while SPm's mesh has its piston at the body's `x` (initially `X = 0.01`) and its outlet at `L0 = 1`.
  The tree case therefore takes `lx = L0 - X = 0.99` with the body at `position = [X, 0, 0]` and the spring's rest at the
  origin: the piston face sits at `x_mesh = x - X`, the column is `0.99 - (x - X) = 1 - x`, SPm's `L0 - x` exactly, with
  20 cells, cross-section `0.2 x 0.2 = 0.04` and the outlet at `p = 0`. The flow is invariant under the translation by
  `X`; SPm's smoothed mesh of the piston at `x` is uniform along `x` (Laplacian, fixed outlet, slip sides) up to the
  smoothing tolerance, as is the tree's. So the tree run is SPm translated, equal up to rounding and the solver
  tolerances. `M = 0.01`, `k = (2 pi)^2 (M + 0.04) = 1.9739208802178716`, the inertia `[1, 1, 1]`, `translation_locked =
  [false, true, true]`, RK4. The settings of SPm: `piso`, `central`, `n_correctors` 2 (the tree's default and
  `piso()`'s), `momentum_rel_tol = pressure_rel_tol = 1e-13`, `pressure_abs_tol = 1e-16`, both caps 5000; the momentum
  absolute floor stays `piso()`'s `1e-14` (no key; SPm used `1e-16`). The smoothing tolerance of the tree is
  `max(1e-12, 1e-14)` relative (SPm: `1e-13`).
- **Reproduction.** These differences (a translation, the smoothing tolerance `1e-12` against `1e-13`, the momentum
  floor `1e-14` against `1e-16` on velocities of order `omega X = 0.06`) move `x` by far less than `1e-8 X` a step; the
  crossing times by under `1e-8` of a period over 11 crossings. ADR 0250 printed `1.005081` and `1.005063` (6 decimals,
  rounding `5e-7`), so the tree's numbers are registered to agree with them within `1e-5` (R1, R2) and its largest
  turning ratios with `0.92452` and `0.96141` within `1e-4` (5 decimals printed).
- **The settings of the strong runs** are ADR 0250's registered ones: Aitken from `relaxation_from_added_mass(M, rho A
  L0) = 1/3`, Aitken cap 2, tolerance `1e-8`, `force_scale = k X = 0.019739208802178716`, `moment_scale = 0`, cap 20.
- **E2 (fixed relaxation, the other keys away from their defaults).** `added_mass = 0.032` (0.8 of `rho A L0`), so `w =
  0.02/0.052 = 0.3846`; the gain of the sub-iteration at ratio 4 is 2.952 (ADR 0250 C1), `q = 1 - 0.3846 x 2.952 =
  -0.135`; to `tolerance = 1e-10` from a first residual of order 1: `ln(1e-10)/ln(0.135) = 11.5`, so 12 to 13 iterations
  a step, under the cap 30.
- **The cap.** With `max_iterations = 1` the first step from rest has `F^1 = 0` (fluid at rest) and `Ft^1 != 0`, so its
  test quantity is 1 and the cap is reached at step 1.
- **Cost.** ADR 0250's whole file (17 tests, about 6000 strong steps of 3 solves on 80 cells) ran in 67 s in release.
  The heavy record here takes 1150 + 2300 strong steps: about 30 s in release, several minutes in debug (hence
  `cfg_attr(debug_assertions, ignore)` like ADR 0250's heavy tests, and the fast equality tests E1, E2 run the same code).

## Design

**D1. The key.** An optional table `[dynamic_mesh.six_dof.coupling]` (`Schema::optional`), absent from every case that
does not write it. Keys and defaults:

| Key | Values, default | Meaning |
| --- | --- | --- |
| `scheme` | `"loose"` (default), `"strong"` | loose: the loop of ADR 0033 D7, unchanged; strong: `rcfd_fv::fsi::strong_step` every step |
| `relaxation` | `"aitken"` (default), `"fixed"` | Aitken's factor (ADR 0250 D6, cap 2) or a fixed factor |
| `added_mass` | float `>= 0`, kg, default 0 | the estimate `m'`; the fixed factor, or the Aitken start, is `2M/(2M + m')` (ADR 0250 D5); 0 gives 1 |
| `tolerance` | float `> 0`, default `1e-8` | the convergence test of ADR 0250 D7 |
| `force_scale` | float `>= 0`, N, default 0 | the floor of the force norm in the test |
| `moment_scale` | float `>= 0`, N m, default 0 | the floor of the moment norm in the test |
| `max_iterations` | integer `>= 1`, default 20 | the cap of flow solves a step |
| `at_cap` | `"stop"` (default), `"continue"` | a step that reaches the cap stops the run with a located message, or is kept and the run goes on (one warning event) |

The Aitken cap is 2 (`StrongCoupling::aitken`), no key. The keys other than `scheme` are inert while `scheme = "loose"`;
the whole table is inert while `dynamic_mesh.enabled = false`, like the rest of the section.

**D2. The driver** (`dynmesh.rs`). `DynMesh::from_case` builds, for `scheme = "strong"`, a `StrongDrive` with the
`BodyCoupling` (`physics.rho`, `physics.nu`, the force patches of the body (default the moving patches), the following
patches, the integrator) and the `StrongCoupling` of the keys (validated by `StrongCoupling::validate`, a failure located
at the table). `DynMesh::step` with a body and a `StrongDrive` calls `strong_step(s, body, dt, ..)` with a `points`
function that is the existing `new_points` written for a given body (the same operations: `map_point` of the initial
points, the smoothing with the moving patches prescribed, or the rigid map); on `converged = false` with `at_cap =
"stop"` it returns the step's `StepInfo` and holds a stop message located at
`dynamic_mesh.six_dof.coupling.max_iterations` (iterations, the last test quantity, the tolerance, the two remedies),
which `Simulation::step` turns into the run's failure with `failure_message(step, time, ..)` after updating its geometry
(the step was taken; ADR 0250 D7 keeps it consistent). Every step's record (iterations, converged, last test quantity)
is kept (`Simulation::coupling_records`), and with `at_cap = "continue"` the first capped step gives one warning event
(`kind = "coupling_cap"`) in `take_runtime_warnings`. Without the table, or with `scheme = "loose"`, `DynMesh::step` runs
the existing lines unchanged (the branch is taken before them only when a `StrongDrive` exists).

**D3. The static checks** (`dynmesh::check`, located at the new keys, only when `dynamic_mesh.enabled` and the table is
written):

| Id | Condition | Path |
| --- | --- | --- |
| N1 | `scheme = "strong"` with `motion` other than `six_dof` | `dynamic_mesh.six_dof.coupling.scheme` |
| N2 | `scheme = "strong"` with `rest_patches` (the strong step sets the following walls only) | `dynamic_mesh.six_dof.coupling.scheme` |
| N3 | `scheme = "strong"` with `integrator = "newmark"` (ADR 0250 derived and tested the factor for RK4 only, its Q6) | `dynamic_mesh.six_dof.coupling.scheme` |
| N4 | `tolerance` not positive or not finite | `dynamic_mesh.six_dof.coupling.tolerance` |
| N5 | `force_scale`, `moment_scale` negative or not finite | their paths |
| N6 | `added_mass` negative or not finite | `dynamic_mesh.six_dof.coupling.added_mass` |
| N7 | `max_iterations < 1` | `dynamic_mesh.six_dof.coupling.max_iterations` |
| N8 | an unknown value of `scheme`, `relaxation`, `at_cap` (the schema's enumerations); an unknown key in the table | the key; the table |

**D4. Documentation.** `docs/case-schema.md` gains the rows of the table and a new TOML block (no existing block edited).

## Registered checks

All in the new file `crates/rcfd-api/tests/strong_coupling_case_f350.rs`, release. The case text `piston(mu, dt, table)`
is the SPm case of the arithmetic, `end_time = 11.5`.

| Id | Check | Limit | Estimate behind it |
| --- | --- | --- | --- |
| K0 (a6, gate of the key) | with the table in the schema, unedited: `rcfd-api` `case_hashes_w48u`, `baseline_hashes`, `w48v_adaptive_correctors`, `rules_w19c`, `mesh_transform_w48u`, `physics_schema`, `tree_wiring_f259`, `layering_ale_f259`, `registers`, `dynamic_mesh`, `exp_rot_pab`, `adapt_case_f342`; `rcfd-cli` `cli`; `rcfd-mcp` `default_f1`, `project_list_f270` | all pass, no file edited | read 6 |
| T0 (a1, a6) | the SPm case without the table: `to_toml` and `to_json_string` contain no `coupling`, `get("dynamic_mesh.six_dof.coupling.scheme")` reads `"loose"`; with the table: both texts contain it, the settings hash differs | as stated | ADR 0155 |
| Z (a6) | `mu = 0.2`, `T/100`, 100 steps: the case with a table `scheme = "loose"` and every other key away from its default against the case without the table: `u`, `p`, `phi`, body `x`, `v` after every step | bit-identical | the loose lines are not changed |
| E1 (tree = library, Aitken) | `mu = 4`, `T/100`, 60 steps, the registered settings as keys: against `strong_step` called in the test on the flow solver of a second `Simulation` of the same case (`pimple_solver_mut`), with a body, a `points` function, a `BodyCoupling` and a `StrongCoupling` built in the test from the case's numbers: `x`, `v` and iterations after every step, `u`, `p`, `phi` at the end | bit-identical; every step converged | the same calls in the same order |
| E2 (tree = library, fixed) | as E1, 40 steps, `relaxation = "fixed"`, `added_mass = 0.032`, `tolerance = 1e-10`, `force_scale = 0.5 k X`, `moment_scale = 1e-3`, `max_iterations = 30` | bit-identical; every step converged; iterations largest `<= 20` | 12 to 13 iterations |
| S1 (a2) | strong, `T/100`, 1150 steps: `omega_m/omega` | `abs(. - 1) <= 0.02` (acceptance) | 1.005081 |
| S2 (a2) | strong, `T/200`, 2300 steps, against `T/100` | relative `<= 0.005` (acceptance) | 1.7e-5 |
| S3 (a2) | both runs: the largest ratio of successive same-side turning points; upward crossings | `<= 1 + 1e-3` (acceptance); `>= 11` | 0.92452, 0.96141; 11 |
| S4 | both runs: every step converged; iterations largest; mean | all; `<= 8`; `<= 5` | 4; 3.0 |
| R1 (a2, reproduction) | S1's and S2's `omega_m/omega` against ADR 0250's `1.005081` and `1.005063` | `<= 1e-5` absolute each | arithmetic: under `1e-7` |
| R2 (a2, reproduction) | S3's turning ratios against `0.92452` and `0.96141` | `<= 1e-4` absolute each | under `1e-6` |
| L (a3) | the same case with `scheme = "loose"` (and without the table), `T/100` and `T/200`, at most 1150 steps: the first step with `abs(x) >= 10 X`, or the step at which the run stops with a failure located by step and time (`dynamic_mesh: step ..`) or by the divergence guard | `<= 60` for both; a stop, if any, has such a message | steps 28 and 31; stops at 34 and 40 |
| C1 (a4) | `mu = 4`, strong, `max_iterations = 1`, `at_cap` default: the run stops at step 1; its failure starts with `dynamic_mesh: step 1` and contains `dynamic_mesh.six_dof.coupling.max_iterations`; the record of step 1 is not converged | as stated | arithmetic |
| C2 (a4, "unless a key says to continue"; the loose step is `K = 1`) | `mu = 0.2`, `T/100`, 100 steps, strong with `max_iterations = 1`, `at_cap = "continue"`, against the case without the table: `u`, `p`, `phi`, `x`, `v` after every step; records; warning events | bit-identical; 100 records, none converged; exactly one warning of kind `coupling_cap` | ADR 0250 Q1 (`K = 1` is the loose step) through the tree |
| N (a5) | each of N1 to N8 (N5 twice, N8 four times: `scheme`, `relaxation`, `at_cap`, an unknown key) gives a diagnostic at its path; each named text is otherwise the valid strong case, which has none; the same bad table with `dynamic_mesh.enabled = false` gives none | as stated | D3 |

Printed, no limit: the turning points, the iteration histograms, how the loose runs ended, the failure texts.

### Mutations (scratch copy, not committed)

Each is applied alone to the final tree by an exact text replacement that must occur once; the named tests are run once
and must fail; the file is restored and compared byte for byte with a kept copy.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | the driver ignores `scheme` (always the loose lines) | E1, C1 |
| MU2 | `at_cap` ignored: a capped step never stops the run | C1 |
| MU3 | the factor from the added mass written `M/(M + m')` | E1, E2 |
| MU4 | `tolerance` not read (the default `1e-8` always) | E2 |
| MU5 | `force_scale` not read (0 always) | E1 |
| MU6 | the static check of a positive tolerance removed | N |

### After the code

`cargo test -p rcfd-api --release` for the new file and K0 (and `rcfd-cli` `cli`, `rcfd-mcp` `default_f1`,
`project_list_f270`); `cargo fmt -p rcfd-api --check`; `cargo clippy -p rcfd-api --all-targets -- -D warnings`. The
whole-workspace suite is not run (the lead's gate).

## Predictions (mine, before any run)

S1 `1.005081`, S2 `1.7e-5`, S3 `0.9245`, `0.9614`; S4 4 and 3.0; R1 differences under `1e-7`; L steps 28 and 31 with
stops at 34 and 40 by a mesh-move error; C1 stops at step 1; C2 equal. The check I trust least is R1/R2: they assume
the translated mesh and the looser floors move nothing visible at six digits.

## Decision rule

- **done**: K0, T0, Z, E1, E2, S1 to S4, R1, R2, L, C1, C2 and N are met and every mutation fails its tests. The
  acceptance stays the maintainer's.
- **partial**: K0 fails (the table is removed, the driver and its tests stay, the schema step is the maintainer's); or
  an acceptance check (S1 to S3, L, C1, C2, N) is missed (kept as `*_known_miss`, diagnosed); or R1, R2 or S4 is missed
  and not shown by an amendment to be a defective set-up; or a mutation is not caught.
- **todo**: the strong path does not run through the case driver.

T0, Z, E1 and E2 are preconditions: a miss means the change is not proposed for merging and the cause is recorded.

## Open choices taken

Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation); none changes a default.

- **T1. An optional sub-table**, not a key inside `six_dof` (read 6: a key inside `six_dof` needs an H1 table edit).
  Options: (a) `[dynamic_mesh.six_dof.coupling] scheme = ...`; (b) a key `six_dof.coupling` with a `DEFAULT_KEYS` entry
  (the maintainer's approval). Taken (a); (b) listed as a question.
- **T2. At the cap the run stops by default** (ADR 0250 Q3 (a)), with `at_cap = "continue"` as the key (Q3 (b)); the
  library is unchanged.
- **T3. Strong with Newmark is refused** (N3) until the factor is derived for it (ADR 0250 Q6 (b)).
- **T4. The wall pressure stays the default extrapolation** (`strong_step`, not `strong_step_with`): its key is F-358.

## Limits of this registration

One geometry (the piston column), translation only; the moment path of the coupling is exercised only by E2's
`moment_scale` (the moment is zero). No rotating body in a flow, no Newmark, no rest patches with the strong step, no
`strong_step_with` (F-358). The Studio's and GUI's editors are not touched (they read the schema). No case file in
`cases/`.
