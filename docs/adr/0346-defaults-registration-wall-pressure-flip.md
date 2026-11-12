# ADR 0346: registration of the default flip of the two wall-pressure keys (ADR 0269 Q1, ADR 0342 Q1): `prescribed_gradient` for the body force of the six-DOF driver and for the force reports on moving walls

Status: **REGISTERED 2026-11-12**, written before any code of the flip and before any run of its checks. Before this
text nothing was run; one release build of three test binaries of the unchanged tree (b511eb75) was started to fill the
target directory of the new worktree (`--no-run`, nothing executed). Results go in the addendum at the end; nothing above
the addendum is changed after a run.

Parents: [ADR 0269](0269-f358-registration-wall-pressure-of-the-body-force-case-key.md) (F-358: the key
`[dynamic_mesh.six_dof.coupling] wall_pressure`, its listing L / LS of what moves when the default flips, its question
Q1), [ADR 0342](0342-f440-registration-moving-wall-force-reports.md) (F-440: the key `[dynamic_mesh.force_reports]
wall_pressure`, its LS, its question Q1), [ADR 0266](0266-f350-registration-strong-body-flow-coupling-case-key.md) (F-350:
the optional coupling table, its checks E1, E2, R1, R2), [ADR 0252](0252-f351-registration-added-mass-deficit-wall-pressure.md)
(the corrected wall pressure and why the old one is first order on a wall with a prescribed gradient).

## The decision being applied

The maintainer, in person, on 2026-11-12 (commit 9872a218 records it in the roadmap notes of F-358 and F-440), approved
both flips together:

1. ADR 0269 Q1, option (b): the wall pressure of the body force of the case driver's six-DOF body (`wall_pressure` of the
   optional table `[dynamic_mesh.six_dof.coupling]`) defaults to `"prescribed_gradient"` instead of `"extrapolated"`, in
   the loose and the strong scheme, for every case that does not write the key. The library defaults (`fsi::strong_step`,
   `sixdof::patch_force_moment`) are not changed (ADR 0269 Q1 (c) is not part of the decision).
2. ADR 0342 Q1, option (b): the force, moment and force-coefficient reports of a moving-mesh case (`wall_pressure` of the
   optional table `[dynamic_mesh.force_reports]`) default to `"prescribed_gradient"` instead of `"boundary_value"`.

The brief's exceptions to the house rules, and only these: the two defaults change (rule 3's bit-identity is lifted for
them); the existing tests that pin the old defaults' numbers may be edited (ADR 0266 R1, R2, E1, E2 and any other test
that moves only because of the flips), each edit recorded below with its old value, new value and reason. Still
forbidden: editing `baseline_hashes.rs`, `case_hashes_w48u.rs`, `w48v_case_hashes.rs` or any golden; if a registered hash
moves, the work stops and is reported. An explicit key with the old value must give the old behaviour bit for bit; the
static checks stay; a case where the new value is refused keeps the old behaviour silently; no previously valid case
becomes an error.

Number 0346 and branch `defaults-wall-pressure-flip` (from main b511eb75), worktree `w49i-work/flip`, given by the lead.
Clean-room: no new algorithm; the corrected wall pressure is ADR 0252's (a Taylor expansion of the wall-cell pressure to
the face with the normal part from the prescribed gradient), unchanged.

## What was read (b511eb75, read only)

1. ADR 0269 in full (D1 to D3; the listing L and LS: with the default flipped `baseline_hashes` 3 passed,
   `case_hashes_w48u` 1 passed, `dynamic_mesh`, `exp_rot_pab`, `layering_ale_f259`, `tree_wiring_f259` pass; T4 92 steps,
   `x = 0.506771 -> 0.506503`, `v = 2.210271 -> 2.209188`, `v^2/v_exact^2 = 0.999943 -> 0.999498`; ADR 0266 S1
   `omega_m/omega 1.005081 -> 1.000042`, turning `0.92452 -> 0.92469`, R1 and R2 fail, E1 and E2 fail; ADR 0266 L `10 X`
   at 28 both, stop `34 -> 35`; ADR 0266 Z, C2 `x = 9.527268e-3 -> 9.524133e-3`, both sides move together; SP `1.001307 ->
   1.000264`; ADR 0269's own T0, Z0, Z1, A2 fail by construction). ADR 0342 in full (read 7: no existing input combines a
   moving mesh and a force report; LS with the default flipped: all seven listed suites pass, its own Z0 fails under MU3;
   A1 `4.597e-6`, A2 `4.369e-6`, A1c `2.5004597e-2`). ADR 0266 in full (E1, E2 compare the tree with `fsi::strong_step`,
   which stays extrapolated; R1, R2 compare with ADR 0250's printed `1.005081`, `1.005063`, `0.92452`, `0.96141`).
2. `crates/rcfd-api/src/dynmesh.rs`: `coupling_schema` (the enumeration `wall_pressure`, default `extrapolated`),
   `force_reports_schema` (default `boundary_value`), `report_wall_gradient(case)`, `check_force_reports` (refuses
   `prescribed_gradient` with `deformation = "layering"` or the compressible solver, read from `get`, so today it would
   also fire on an absent table once the default flips), `written` (the coupling table is in the data),
   `wall_pressure(case)`, `check_coupling` (returns at once without the coupling table; refuses `prescribed_gradient` with
   a motion other than `six_dof`), `DynMesh::from_case` (reads both keys), the loose step (corrected force when the key
   asks and the kept vector is not empty), the strong step (`strong_step_with(.., self.wall_pressure, &mut
   self.wall_dpdn, ..)`), the layered step (keeps no vector). `crates/rcfd-api/src/sim.rs`
   `report_pressure_face_values` (stored values unless the key and a kept vector of one entry per boundary face).
3. `crates/rcfd-core/src/settings.rs` `coerce`: an absent optional object stays out of the data at any depth; inside a
   written object every absent field is filled with its schema default; `get` into an absent optional object reads the
   schema defaults. So after `coerce` a key that a written table leaves out cannot be told from the same key written with
   the default value; an absent table can (`written`).
4. Every input that writes either table (`git grep` of `six_dof.coupling]` and `force_reports]`): the test literals of
   `strong_coupling_case_f350.rs`, `wall_pressure_case_f358.rs`, `force_reports_moving_wall_f440.rs`; three TOML blocks of
   `docs/case-schema.md` (two coupling blocks, one force-report block); no file of `cases/`, no golden, no CLI or MCP test.
   The goldens (`crates/rcfd-mcp/tests/golden`, the CLI help) hold neither default (`git grep` of `extrapolated"` and
   `"boundary_value` outside `docs/adr` and the roadmap: only `dynmesh.rs`, the two test files of F-358 and F-440 and
   `docs/case-schema.md`).

## Design

**D1. The flip is the schema default of each enumeration.** `coupling_schema`: `Schema::enumeration("prescribed_gradient",
&["extrapolated", "prescribed_gradient"])`; `force_reports_schema`: `Schema::enumeration("prescribed_gradient",
&["boundary_value", "prescribed_gradient"])`. The value lists are unchanged; only the default moves. Then:

- *A case without the table* (every existing case outside the three test files): its data, `to_toml`, `to_json_string`
  and settings hash are unchanged (the optional table stays out of the data, read 3); `get` of the key reads the new
  default, and the run takes it. This is what the decision asks.
- *A case that writes the table and leaves the key out*: `coerce` fills the new default into its data, so its written
  text (`to_toml`, `to_json_string`) and settings hash now carry `wall_pressure = "prescribed_gradient"` where they
  carried the old value. This cannot be avoided by any route that keeps the decision: the strong scheme is only reachable
  by writing the coupling table (`scheme = "strong"`), the decision flips the strong scheme too, and a written text that
  kept saying `"extrapolated"` would, read back, give the old behaviour (the explicit old value). The text then records
  what the run does. Read 4 lists every such input; none is in a registered hash table (ADR 0266 read 6, ADR 0269 LS:
  `case_hashes_w48u` prints the three docs blocks as new and passes). The brief's "no written case text changes" therefore
  holds for every case without the tables and fails, necessarily, for these; it is disclosed here and in the report.
- *A case that writes the key*: unchanged, bit for bit (the explicit old value reads as before; checks X1).

**D2. Where the new value is refused, the default keeps the old behaviour silently.**

- Force reports with `deformation = "layering"` or `solver.algorithm = "compressible"` (ADR 0342 D3): `check_force_reports`
  refuses `prescribed_gradient` only when the table is written (`/dynamic_mesh/force_reports` in the data), and
  `report_wall_gradient(case)` is true only when the value is `prescribed_gradient` and, for an absent table, neither
  layering nor the compressible solver is selected. So an absent table there gives `boundary_value`, silently (the layered
  step keeps no vector and the compressible run never steps `DynMesh`, so the reports would be the stored values anyway;
  the resolution makes it explicit). An explicit `prescribed_gradient` there is refused at the key as before.
- The body force with a motion other than `six_dof` (ADR 0269 D3): there is no body force, so the key has no effect in
  either value; `check_coupling` already returns without the coupling table, so an absent table is never refused. An
  explicit `prescribed_gradient` with a prescribed motion is refused at the key as before.
- **The one input class that changes from valid to refused** (read 3): a written table that leaves the key out in a
  refused context (an empty `[dynamic_mesh.force_reports]` table with layering or the compressible solver; a
  `[dynamic_mesh.six_dof.coupling]` table without `wall_pressure` under a prescribed motion). After `coerce` such a table
  holds `prescribed_gradient` exactly as if the user had written it, and its written text says so; accepting it silently
  would make the written text refused on reading it back. No input in the repository is of this class (read 4). Taken on
  the maintainer's standing instruction of 2026-11-11 (take the recommendation): refuse it at the key, with the existing
  message that names the old value; listed as question Q1 with the alternative.

**D3. Unchanged.** `rcfd-fv` (the library defaults: `strong_step`, `patch_force_moment`), `rcfd-post`, `rcfd-core`,
`sim.rs` (the report path already reads `DynMesh::report_wall_gradient`), the loose and strong step code, the hash
tables, the goldens.

**D4. Documentation.** `docs/case-schema.md`: the two rows (default, absent = the new default), the prose of the two
sections that say the default is not changed (no TOML block edited: H1 keys a block by its text). `docs/dynamic-mesh.md`:
the paragraph of the case tree. The module doc and the schema docs of `dynmesh.rs`.

## The existing tests that move, and their edits (registered now, before any run)

Each edit keeps the test's purpose; the old value, the new value and the reason are recorded here, the result in the
addendum. The ignored records (`l_listing_...` of ADR 0269, `au1_..._known_miss`, `x1_...`, `n_..._registered_record`)
are not edited: they are not run, and their labels "default" now name the old default (written in the addendum).

| Test (file) | Moves because | Edit | Old | New (predicted) |
| --- | --- | --- | --- | --- |
| ADR 0266 E1, E2 (`strong_coupling_case_f350.rs`, `library_run`) | the tree now takes `prescribed_gradient`, the library side is `strong_step` (extrapolated) | the library side calls `strong_step_with(.., WallPressure::PrescribedGradient, &mut carried, ..)` with a vector carried by the test from empty, the tree's own route (the brief's second option) | tree = `strong_step` bit for bit | tree = `strong_step_with(PrescribedGradient)` bit for bit, iterations equal, all converged |
| ADR 0266 S1 with R1, R2 (same file, `strong_case_run`) | R1, R2 pin ADR 0250's numbers, which are the extrapolated wall pressure's | the strong table of `strong_case_run` gains `wall_pressure = "extrapolated"` (the explicit old value, so R1, R2 stay the reproduction of ADR 0250 they were registered as); the new default's numbers at ratio 4 are P2 below | R1 `4.2e-7`, `3.3e-7`; R2 `4.7e-6`, `1.6e-7` | the same numbers to the printed digits (explicit old value bit for bit) |
| ADR 0269 T0 (`wall_pressure_case_f358.rs`) | `get` of the key without the table | expected `"extrapolated"` -> `"prescribed_gradient"` | `extrapolated` | `prescribed_gradient` |
| ADR 0269 Z0 | "a table with only `extrapolated` equals no table" | the table holds `prescribed_gradient` (the explicit new value equals no table); the old statement becomes X1a | 0 of 100 differ | 0 of 100 differ |
| ADR 0269 Z1 | the same for the strong table | the extra key is `wall_pressure = "prescribed_gradient"` | 0 of 40 differ | 0 of 40 differ, iterations equal |
| ADR 0269 A2 | "the same without the table" is now the new default | `sp_run` with `[dynamic_mesh.six_dof.coupling] wall_pressure = "extrapolated"` (the control is the old wall pressure) | `1.001307` | `1.001307` |
| ADR 0342 T0 (`force_reports_moving_wall_f440.rs`) | `get` without the table | expected `"boundary_value"` -> `"prescribed_gradient"` | `boundary_value` | `prescribed_gradient` |
| ADR 0342 Z0, Z1, Z2 | Z0 compares the case without the table with the old formula | Z0: the explicit `boundary_value` table against the old formula; Z1: no table against the explicit `prescribed_gradient` table (reports and flow); Z2: the explicit `boundary_value` table against the explicit `prescribed_gradient` table (flow only); step 0: equal | 0, 0, 0 of 13 | 0, 0, 0 of 13 |
| ADR 0342 E3 | its control is the case without the table | the control is the explicit `boundary_value` table | 0 of 12 | 0 of 12 |
| ADR 0342 A1c and the no-table halves of A3, A4, F1 | "no table" is now the new default | `worst(.., STORED, ..)` (the explicit old value) | `2.5004597e-2` | `2.5004597e-2` (same bits) |

Predicted not to move and not edited (ADR 0269 LS): ADR 0266 T0, Z, L, C1, C2, Nb (`any` of the paths, so the extra
`wall_pressure` diagnostic of its prescribed-motion case, D2, does not fail it); `dynamic_mesh.rs` T4 (limit `2e-2`,
ratio `0.999498`); every other test of the three files (explicit keys).

## Registered checks

New file `crates/rcfd-api/tests/wall_pressure_defaults_flip.rs`, release. Texts: `piston(mu, dt, table)` (ADR 0266's,
copied), `t4(table)` (ADR 0269's copy of `dynamic_mesh.rs` T4), `column(wall, table)` (ADR 0342's, copied). "Digest" is
FNV-1a 64 over the `to_bits` of every number listed, in order, after every step.

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| H (hashes, the brief) | unedited: `baseline_hashes`, `case_hashes_w48u`, `w48v_case_hashes` (`rcfd-api`) | all pass as before (ignored stay ignored); **any failure stops the work** | ADR 0269 LS, ADR 0342 LS |
| K (unedited suites) | `rcfd-api`: `dynamic_mesh`, `exp_rot_pab`, `layering_ale_f259`, `tree_wiring_f259`, `physics_schema`, `rules_w19c`, `mesh_transform_w48u`, `registers`, `adapt_case_f342`, `w48v_adaptive_correctors`, `force_effective_w49a`, `workflow`, `cosim_service_f360`, `wall_motion`; `rcfd-cli` `cli`; `rcfd-mcp` `default_f1`, `project_list_f270`; `rcfd-fv`: `sixdof`, `sixdof_coupled`, `sixdof_flow_f244`, `strong_fsi_f251`, `added_mass_f351`, `ale_flows` | all pass as before | read 4; no `rcfd-fv` line changes |
| E (edited tests) | `strong_coupling_case_f350`, `wall_pressure_case_f358`, `force_reports_moving_wall_f440` in full, with the edits of the table above | all pass; the new values as predicted there | the table above |
| D0 (texts) | the piston, `t4` and the column without tables: no `coupling`, `wall_pressure`, `force_reports` in `to_toml` or `to_json_string`; settings hashes equal those printed by X0 on the unchanged tree; `get` of both keys reads `prescribed_gradient`. The piston with a strong table without the key: `to_toml` holds `wall_pressure = "prescribed_gradient"` (D1, disclosed) | as stated | read 3 |
| X0 (record of the old default, run once on the unchanged tree b511eb75 before any code, ignored afterwards) | the digests of X1a to X1e computed on the texts **without** the key, and on the texts with the explicit old value; the settings hashes of D0's three texts | printed; the two digests of each pair equal (ADR 0269 Z0, Z1, ADR 0342 Z1 on the old tree) | ADR 0269, 0342 |
| X1 (explicit old value = old default, bit for bit) | on the new tree, the digests of: a. the piston `mu = 0.2`, `T/100`, 100 loose steps, `wall_pressure = "extrapolated"` (`u`, `p`, `phi`, `x`, `v`); b. `mu = 4`, 40 strong steps, ADR 0266's registered table with `wall_pressure = "extrapolated"` (the same, and the iterations); c. `t4` with `extrapolated`, 92 steps (the same); d. the column at rest with `wall_pressure = "boundary_value"`, 12 steps (the five reports before the first and after every step, and `u`, `p`, `phi`); e. the same with the following wall | each equal to X0's digest of the same text without the key | bit-identical |
| Y (no key = explicit new value) | on the new tree: a. the piston of X1a without the table against `wall_pressure = "prescribed_gradient"`; b. X1b's strong table without the key against it with `"prescribed_gradient"` (and iterations); c. `t4` without against with; d. the resting column without the table against `[dynamic_mesh.force_reports] wall_pressure = "prescribed_gradient"` (reports and flow) | digests equal | `get` reads the schema default |
| P1 (SP, ADR 0269 A1/A2) | the piston `mu = 0.2`, `T/100`, 650 loose steps, no table: `omega_m/omega` | `abs(. - 1.000264) <= 1e-6`, and `abs(. - 1.001307) > 5e-4` | `1.000264` (ADR 0269 A1, printed to 6 decimals) |
| P2 (ADR 0266's piston, heavy in debug) | `mu = 4`, `T/100`, 1150 strong steps, ADR 0266's registered table without the key: `omega_m/omega`, largest same-side turning ratio, all converged | `abs(. - 1.000042) <= 1e-6`; `abs(. - 0.92469) <= 1e-5`; all | `1.000042`, `0.92469` (ADR 0269 A4 and L) |
| P3 (T4) | `t4` without the table, steps to `x >= 0.5`: steps; `x`, `v`, `v^2/v_exact^2` at the last step | `92`; each within `1e-6` of `0.506503`, `2.209188`, `0.999498` | ADR 0269 L |
| P4 (ADR 0266 L) | `mu = 4`, `T/100`, loose, no table: the step of `abs(x) >= 10 X`, the step at which the run stops | `28`; `35` | ADR 0269 L (old 28; 34) |
| P5 (ADR 0266 Z) | `mu = 0.2`, `T/100`, 100 loose steps, no table: `x` | `abs(. - 9.524133e-3) <= 1e-9` | ADR 0269 L (old `9.527268e-3`) |
| P6 (ADR 0342 A1, A2) | the resting column without the table, steps 3 to 12: worst deficits of `fp` and `ft` | `abs(. - 4.597e-6) <= 1e-9`; `abs(. - 4.369e-6) <= 1e-9` | ADR 0342 A1, A2 (old `2.5004597e-2`) |
| R (refused contexts, D2) | the column (following wall) with `deformation = "layering"`; the column (resting wall) with `solver.algorithm = "compressible"`; the piston with `motion = "prescribed"` and a displacement expression (the `six_dof` table removed): the diagnostic paths of `Case::parse_toml` + `semantic_errors` without any table equal those with the explicit old value (`boundary_value` table for the first two, coupling `extrapolated` for the third) and do not hold the key; with the explicit new value the key is among them | as stated | D2 |

Printed, no limit: every number above, each digest.

### Negative controls

P1 to P6 are each the negation of the old default's number (each old value is more than its limit away). X1 against Y:
on a six-DOF text the two digests must differ (printed; X1a's digest differs from Ya's), otherwise X1 would be blind to
the key.

### Mutations (scratch copy, not committed)

Each applied alone to a `git archive` copy of the final tree by an exact replacement that must occur once; the named
tests run once and must fail; the file restored and compared byte for byte.

| Id | Mutation (`dynmesh.rs`) | Must fail |
| --- | --- | --- |
| MU1 | the coupling default back to `"extrapolated"` | Y a, P1, D0 |
| MU2 | the force-report default back to `"boundary_value"` | Y d, P6, D0 |
| MU3 | `check_force_reports` without the written-table guard | R |
| MU4 | `wall_pressure(case)` returns `PrescribedGradient` for every value | X1 a |
| MU5 | `report_wall_gradient(case)` true for every value outside layering and compressible | X1 d |
| MU6 | the strong step passes `WallPressure::Extrapolated` whatever the key | Y b, ADR 0266 E1 |

## Predictions (mine, before any run)

H, K, E pass; X1 equal (five of five); Y equal; P1 `1.000264`, P2 `1.000042` and `0.92469`, P3 92 steps and ADR 0269's
numbers, P4 28 and 35, P5 `9.524133e-3`, P6 `4.597e-6` and `4.369e-6`; R as stated. The check I trust least is P4 (a run
that stops on a mesh error: one step either way would come from any change in the last digits).

## Decision rule

- **done**: H, K, E, D0, X1, Y, P1 to P6, R met; X0 run once; every mutation caught.
- **stopped**: any check of H fails (a registered hash moved): the work stops, nothing is committed beyond what exists,
  and the hash and its cause are reported.
- **partial**: a P check missed (kept as `*_known_miss`, the default flip still applied, the cause diagnosed); a K or E
  test fails for a reason the table above does not list (reported, not edited further); a mutation not caught.
- **todo**: X1 or Y not bit-identical (the flip is not proposed for merging).

## Open choices taken

Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation); neither changes a default result
beyond the decision itself.

- **T1. The schema default** is where the flip lives (D1), not a third value such as `"auto"`: a third value would also
  change the text of a written table and add a value nobody decided.
- **T2. A written table without the key in a refused context is refused** (D2), with the existing message naming the
  old value: question Q1.
- **T3. R1, R2 keep reproducing ADR 0250** through the explicit old value, rather than being re-recorded to new numbers:
  their registered meaning was the reproduction of ADR 0250's extrapolated runs; the new default's numbers at ratio 4 are
  checked by P2 against ADR 0269's registered and measured values.

## Limits of this registration

The library defaults are not flipped (`strong_step`, `patch_force_moment`, the `rcfd-fv` suites; ADR 0269 Q1 (c)).
No rotating six-DOF body, no smoothed moving wall with force reports, no layered run with force reports is stepped: the
layered and compressible fall-backs are checked statically (R), their run behaviour rests on ADR 0342 read 5 (the layered
step keeps no vector, the compressible run never steps `DynMesh`) and on the resolution in `report_wall_gradient`. The
whole-workspace suite and the gate are the lead's.
