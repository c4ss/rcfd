# ADR 0169: Job 49F, `freestream` becomes the far-field default of k-omega SST and k-epsilon; the 8 stored NACA 0012 example runs are regenerated

Status: **registered 2026-10-29**, before any code and before any solver run of this job. The enumeration, the design,
the limits, the reading rules and the not-run list are fixed here. **Results in the addendum:** the code limits F1-F7, R1
and the records (G1: no `baseline_hashes.rs` case changes) met; the 8 regenerated NACA runs **converge** (409 / 230
iterations, exit 0) and so miss the aerofoil parts of ADR 0142 G1, ADR 0155 E2 and ADR 0148 N9, now ignored known misses
with their limits intact; sizes and wall times within their limits. Amendment 1 (base commit) and amendment 2 (R2, a
reported drag-attribution run) before their runs. Not yet accepted by the maintainer. Job 49F, on local main 1248a76 (the worktree branch was fast-forwarded to it before any work; it holds
job 49C, ADR 0166). Number: 0168 is the highest in `git log --all` and in every worktree when this was written (0167 job
49D, 0168 job 49E); job 49G runs at the same time and may also be writing an ADR.

House rules of [ADR 0130](0130-wave48a-overview-and-run-record-contract.md):
- limits are fixed here before the registered runs; a missed limit becomes an ignored `*_known_miss` test with the limit
  intact;
- a defective set-up gets an amendment before the corrected run;
- defaults stay bit-identical except where the maintainer's instruction below changes them, and every such change is
  listed here before the code;
- `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded; if one of its cases would change, stop and report;
- no new crate, no unsafe code, the crate graph stays acyclic, no commit trailers, nothing is pushed.

Parents:
- [ADR 0166](0166-job49c-effective-wall-shear-and-sa-far-field-defaults.md) (job 49C): the `freestream` far-field
  default for Spalart-Allmaras (D2), the effective wall shear as the force default (D1), its questions 1 and 3.
- [ADR 0164](0164-job49a-why-the-sa-drag-of-the-naca0012-is-wrong.md): the `freestream` turbulence role (amendment 1).
- [ADR 0142](0142-wave48h-example-projects.md), [ADR 0150](0150-job48p-linux-example-stores.md),
  [ADR 0155](0155-job48u-rotate-the-mesh-instead-of-the-flow.md), [ADR 0157](0157-job48w-merge-48u-rotated-cases-with-the-linux-tree.md):
  the example projects, the per-platform trees, `--add-missing`, the size caps, the example tests' limits.

## The maintainer's instruction (2026-10-29, relayed, exact words)

"regenerate the naca runs, (1) make farfield freestream (2) separate job".

- (1) is ADR 0166 question 1 (b): `freestream` becomes the default turbulence role of `velocity = "freestream"` far fields
  for k-omega SST and k-epsilon too, with free-stream values from `turbulence.initial_intensity` and
  `turbulence.initial_viscosity_ratio` when the patch gives no inlet specification. That is this job.
- "regenerate the naca runs" is ADR 0166 question 3 (b): the 8 stored NACA 0012 example runs. That is this job.
- (2) (moments follow the force default, ADR 0166 question 2) is job 49G, a separate job. This job does not touch
  moments.

The brief adds: no existing valid case may become a validation error; RSM has no inlet condition, leave it and document
it; keep the old behaviour spellable (`turbulence = "zero_gradient"`); do not raise a cap or loosen a limit; if a
regenerated run fails its own acceptance, report it with numbers and do not tune.

## What I read before writing this (code, stored runs, no solver run)

1. **`turb::freestream_role`** (`crates/rcfd-api/src/turb.rs`, ADR 0166 D2). Under `turbulence = "auto"` it is true for
   `velocity = "freestream"` with a non-zero `velocity_value`, not a catalogue or wall-motion kind, and `is_sa(case)`.
   `turb::role` maps it to `Role::Inlet`. `TurbState::new` and `Simulation::reconfigure_turbulence` (ADR 0166
   amendment 1) both call it, so they cannot disagree.
2. **`turb::check`, `Role::Inlet` branch.** SA, plugin and algebraic models need nothing. RSM: always an error ("no inlet
   condition for the stresses"). k-epsilon and SST: `turbulence_spec = "values"` needs positive `k_value` and
   `omega_value` / `epsilon_value`; the other two specs need a non-zero inlet speed; `intensity_length` (the default) also
   needs `turbulence_length > 0`. Schema defaults: `turbulence_spec = "intensity_length"`, `turbulence_intensity = 0.05`,
   `turbulence_length = 0` ("unset"), `turbulence_viscosity_ratio = 10`.
3. **Inlet values** (`TurbState::new`). SA: `nutilde_value` if positive, else `nutilde_ratio * nu` (the initial value).
   SST: `inlet_values_sst` (Cmu = beta* = 0.09, `omega = eps / (beta* k)`); k-epsilon: `inlet_values` with the model's
   `c_mu` (`cmu_now`). Both clamp k and epsilon (omega) at 1e-10. The per-face speed of a free-stream patch is
   `|velocity_value|` (`Simulation::new`, `reconfigure_turbulence`). The free-stream role builds `rcfd_fv::bc::Freestream
   { fixed_on_inflow: true }` with the first face's values.
4. **The initial field** (`TurbState::new`). With `k_initial = 0`: `k0 = 1.5 (initial_intensity U_ref)^2` with
   `U_ref = reference_speed`: the largest of `|initial.velocity|` and the speeds of the `Inlet`-role patches;
   `eps0 = Cmu k0^2 / (nu initial_viscosity_ratio)`, SST `omega0 = eps0 / (beta* k0)` (Cmu = beta* for SST, the variant's
   Cmu for k-epsilon). Defaults `initial_intensity = 0.05`, `initial_viscosity_ratio = 10`.
5. **The NACA 0012 example cases** (`example-projects*/wind-tunnel/cases/naca0012-aoa{2,4}{,-rotated}.toml`, the two trees
   byte-equal): SST, `[boundaries.farfield] velocity = "freestream"`, no `turbulence` line, no turbulence spec, no
   `[turbulence]` key besides `model`, `initial.velocity` equal to the far-field `velocity_value` (the same literal), no
   `moment` report. The stored runs stop at `max_iterations = 2000` with `not_converged` (exit 5); at 4 degrees the
   momentum residual is 1.43e-6 at iteration 2000 against the target `tolerance_u = 1e-6`, so a different far field
   could reach the targets before 2000 (limit E1 below).

## Enumeration: what changes (registered before any code)

Searched: every `*.toml` (`cases/**`, `example-projects*/**`, test fixtures), every Rust test and source literal
(`velocity = "freestream"`, `"freestream"` set through `Case::set`), `scripts/*.py`, `python/`, `clients/`, the
registry descriptors `cases/*/case.json`, the fenced TOML of `docs/*.md`, the Studio case starter and the MCP / assistant
sources. `git grep -n freestream` over the tree outside the ADRs lists only the files below (and `rcfd-fv`'s
`bc.rs`, the condition itself; `vof_tree.rs` and `vof.rs`, pressure and multiphase refusals; `init.rs`, the velocity
initialisation; `flat_plate_sst.rs` and the `validation-data` CSVs, text).

### Change A: the far-field turbulence role of SST and k-epsilon under `auto`

| Item | What moves | Committed files changed by this job |
|---|---|---|
| `example-projects/wind-tunnel/cases/naca0012-aoa2.toml`, `-aoa4`, `-aoa2-rotated`, `-aoa4-rotated` and the same 4 in `example-projects-linux/` (SST) | k and omega on the far field, so every field and report of a run | the cases: none (D6) |
| stored runs, Windows tree: `example-projects/wind-tunnel/runs/r-20261028T152128-78e0` (aoa2), `-cb3a` (aoa4), `r-20261028T224153-afb6` (aoa2-rotated), `-d9c7` (aoa4-rotated) | they were made before ADR 0166 (molecular `cl`/`cd`) and before this change (zero-gradient k/omega far field) | replaced by 4 new runs (D7) |
| stored runs, Linux tree: `example-projects-linux/wind-tunnel/runs/r-20261028T220207-763f` (aoa2), `-6504` (aoa4), `r-20261029T000812-00f3` (aoa2-rotated), `-6c75` (aoa4-rotated) | the same | replaced by 4 new runs (D7) |
| `designs/variants.ledger.jsonl`, `notes/README.md` run table, `manifest.json` of each tree | the 4 NACA entries / rows, the run list and byte counts | rewritten by the script |
| `example-projects/wind-tunnel/notes/naca0012.md` (an input, copied into the Linux tree by the script) | its run numbers (ADR 0155 E2's `cl` pair, the drift 0.3471 to 0.3482, the residual near 1.5e-6) | updated by hand from the new runs' files (D8) |
| `crates/rcfd-api/tests/registry_setups_w39a.rs`, set-up `naca0012` (SST, free-stream far field, `rcfd check` only) | its far field becomes an inlet-role patch with the fallback values (D2); must still pass V1 (exit 0, no error-level rule) | none |
| `crates/rcfd-api/tests/defaults_w49c.rs` T6 (SST/k-epsilon `auto` = explicit `zero_gradient`) and T8 (a) (after an SA-to-SST switch, k/omega zero-gradient on every far-field face) | both state ADR 0166's SA-only default, which this change supersedes; they fail as written | respelled (D9), limits replaced by F1/F6 |
| `crates/rcfd-api/src/turb.rs` lib test `freestream_far_field_default_is_sa_only` | the same | respelled (D9) as F7 |
| `scripts/w48y_aoa_sweep.py` SST arm (ADR 0159), `scripts/w49a_sa_drag.py` arm S (ADR 0164) | `cl`, `cd` and fields of a re-run | none: they reproduce with `turbulence = "zero_gradient"` on the far field plus `component = "total_molecular"` (D5). Job 49D re-runs ADR 0159's sweep on a binary without this change; its SST numbers are then a record of the pre-49F default |
| any user case: SST or k-epsilon with a `velocity = "freestream"` far field and `auto` | fields and reports | - |
| any user case: SST or k-epsilon with an explicit `turbulence = "freestream"` and no turbulence spec | today a validation error (`turbulence_length`); afterwards valid with the fallback values (D2) | - |
| any user case of the above whose `|velocity_value|` exceeds `|initial.velocity|` | additionally the initial k and omega/epsilon (`reference_speed` now counts the far field, item 4) | none in the repository (all four NACA cases use the same vector for both) |

**Not changed by A:**
- Spalart-Allmaras (already `freestream` since ADR 0166), RSM, plugin models, subgrid and mixing-length models (D1);
- every patch that is not `velocity = "freestream"` with a non-zero `velocity_value`, and every patch with an explicit
  `turbulence` value other than `freestream`; explicit `turbulence_spec` / `turbulence_length` / `k_value` settings on a
  far field are used as before (D2);
- laminar cases, among them the cylinder examples (`cylinder-re100/200.toml`, laminar PISO) and `statistics.rs`;
- `force_effective_w49a` C1-C5 (SA; C5's error stays);
- moments (job 49G). The wind-tunnel cases have no `moment` report, so 49G's change cannot move these runs again.
- FAN-01 (`fan01_*` tests and their cases): SST, but no `velocity = "freestream"` patch anywhere in them
  (`git grep -n freestream -- crates/rcfd-api/tests` lists only `defaults_w49c`, `force_effective_w49a`,
  `registry_setups_w39a`, `sa_drag_w49a`, `statistics`, `vof_tree`).

### Records and gates

- **`baseline_hashes.rs`: no case changes** (predicted; checked in G1, the file is not edited). Its only turbulent case,
  `cases/channel/kepsilon_channel.toml`, has fixed walls and periodic ends and no `freestream` patch; its `y_plus` reports
  are untouched. The cavity, heated-cavity, `packed_bed`, `sod`, `wedge` and rotating-channel cases are laminar or
  compressible.
- **`case_hashes_w48u` (fixture `case_hashes_6867e40.txt`)**: no key and no default value changes (D4) and no case file is
  edited (D6), so every recorded settings / toml hash stays.
- **Studio plans fixture `plans_6867e40.txt`**: start plans read no turbulence role; the NACA case texts are unchanged.
- **`rcfd-fv` `w33b_default_bits`**: `rcfd-fv` is not touched.
- **Example tests that read the stored runs** (`rcfd-mcp` `example_projects_w48h`, `example_rotated_w48u`; `rcfd-studio`
  `example_projects_h`, `scale_n`, `mesh_transform_u`, `saved_store_r`): their limits are kept and are evaluated on the
  regenerated runs (E1-E4). None of them names a run id.

## Decisions (mine, within the maintainer's instruction; none is presented as the maintainer's)

1. **D1 The far-field default covers SA, SST and k-epsilon.** `freestream_role`'s `auto` branch takes
   `turbulence.model` in `spalart_allmaras`, `k_omega_sst`, `k_epsilon` (every variant). Everything else in the branch is
   ADR 0166's (velocity kind, catalogue and wall-motion kinds first, non-zero `velocity_value`). RSM stays
   `zero_gradient` under `auto`: `turb::check` rejects every inlet role for it (no inlet condition for the Reynolds
   stresses), so the default would turn each RSM far-field case into an error; an explicit `freestream` with RSM stays an
   error. Plugin models own their scalars and the algebraic models have none; both keep their role.
2. **D2 Free-stream values when the patch gives no inlet specification.**
   - "No inlet specification" means exactly the input that `turb::check` refuses today for SST and k-epsilon:
     `turbulence_spec = "intensity_length"` (the default) with `turbulence_length <= 0` (the default, "unset"). One
     helper (`turb::freestream_fallback(case, patch)`: the patch has the free-stream role, the model is SST or
     k-epsilon, and that spec) decides it for `check` and for `TurbState::new`.
   - Then, per face with the speed `U = |velocity_value|`:
     `k = 1.5 (initial_intensity U)^2`, `epsilon = Cmu k^2 / (nu initial_viscosity_ratio)`, SST `omega = epsilon /
     (beta* k)`; Cmu = beta* for SST, the model's `c_mu` for k-epsilon; the clamps (1e-10) of `inlet_values`. In code:
     `inlet_values` / `inlet_values_sst` on the `intensity_viscosity_ratio` path with `turbulence.initial_intensity` and
     `turbulence.initial_viscosity_ratio` in place of the patch's `turbulence_intensity` and
     `turbulence_viscosity_ratio`.
   - With `|velocity_value| = |initial.velocity|` (the NACA cases) these are the values of the initial field, which is
     the TMR and Menter practice (free stream = initial state). NACA examples: `nu = 1/6e6`, `U = 1`: `k = 3.75e-3`,
     `nut/nu = 10`, `omega = k / (10 nu) = 2250`. Not tuned.
   - Because the fallback applies only to inputs that are an error today, every explicit far-field specification
     (`values`, `intensity_viscosity_ratio`, `intensity_length` with a length) is honoured exactly as before, and only
     the role default (D1) moves a valid case. It applies to the explicit `turbulence = "freestream"` as well, so that
     `auto` and the explicit spelling are the same run (F1). The explicit `inlet` role and fixed-velocity inlets keep the
     `turbulence_length` error.
   - `k_initial`, `omega_initial` and `epsilon_initial` set the initial field only; the far-field fallback does not read
     them (question 1). `nutilde_value` / `nutilde_ratio` (SA) are unchanged.
3. **D3 What comes with the inlet role** (no further code): `reference_speed` counts the far field (item 4), so the
   initial k of a case with `|velocity_value| > |initial.velocity|` rises; `Simulation::new` collects the far field's
   per-face speeds; `reconfigure_turbulence` (ADR 0166 amendment 1) adds the far field when a live switch goes to SST or
   k-epsilon, with the speed `|velocity_value|`.
4. **D4 No new key, no new value.** Only the resolution of `auto` and the refusal of D2 change. Settings text and case
   hashes stay.
5. **D5 The old behaviour stays spellable:** `boundaries.<far field>.turbulence = "zero_gradient"`. To reproduce the
   stored NACA runs (and ADR 0159's / 0164's SST arms) with this build, add that line and `component = "total_molecular"`
   on the `cl` and `cd` reports (ADR 0166 D4). R1 checks this on the 4-degree example over all 2000 iterations.
6. **D6 No case file is edited**, not even a comment: the toml hashes of `naca0012-aoa2/4.toml` are recorded in
   `case_hashes_6867e40.txt` and the plans fixture, and `check_run` compares each case with its run's snapshot. If the
   comment "the residual targets are not reached on this mesh" becomes false, it is reported, not edited.
7. **D7 Regeneration procedure: delete the 4 NACA runs of a tree, then `--add-missing`** (ADR 0155 D10, ADR 0157 D3 (a)).
   - There is no `--rerun` option. A full rebuild (`--project wind-tunnel` without `--add-missing`) would also re-run the
     two laminar cylinders and the two laminar steps, which this change does not move: new ids, time stamps and paths for
     unchanged results. `--add-missing` runs only the cases without a run, through the real run service, and keeps every
     other run and the mesh byte for byte.
   - Windows tree: `git rm -r` of the 4 run folders, then `python scripts/make_example_projects.py --project wind-tunnel
     --add-missing --bin-dir E:\projects\cfd\w49f-work\target\release` (release build of this branch, `--locked`,
     `CARGO_TARGET_DIR=E:\projects\cfd\w49f-work\target`, one solver thread, the script's default).
   - Then D8's note update and a commit, so that the Linux build copies the final inputs.
   - Linux tree: `git rm -r` of its 4 run folders, then `python scripts/make_example_projects.py --docker --project
     wind-tunnel --add-missing` (ADR 0150 D4's image `rcfd-linux:ubuntu24.04-rust1.99.0`, the default `--docker-cpus 0-3`
     and `--docker-memory 16g`; `source_tar` leaves out the deleted tracked files; the container copies the Windows
     inputs first). Not concurrent with the Windows runs. Before it: no other `rcfd-linux` container running (they would
     share the named target volume), free memory checked.
   - Closing: `python scripts/make_example_projects.py --check` (both trees).
8. **D8 `notes/naca0012.md`**: the numbers that came from the old runs are replaced by the same quantities read from the
   new runs' `residuals.jsonl` / `metrics.json` (Windows tree), with a dated sentence that the runs were regenerated under
   ADR 0166's and this ADR's defaults and how to get the old behaviour (D5). ADR 0155 E2's numbers are kept as a record
   of the earlier runs, attributed. Nothing else in the notes changes. `notes/README.md`'s table is the script's.
9. **D9 Superseded tests are respelled, not loosened.** ADR 0166 T6, T8 (a) and the lib test asserted the SA-only scope,
   which the maintainer's instruction reverses. They are rewritten to the new scope (F1, F6, F7) with the same set-up
   (ADR 0166's free-stream plate, 30 iterations; the switch after 20 + 10). T5, T8 (b), (c) and T1-T4 are unchanged. A
   dated pointer goes into ADR 0166.
10. **D10 Documentation**: `docs/case-schema.md` (the far-field note under `spalart_allmaras`, the `turbulence` key
    bullet, the `turbulence_spec` bullet), the schema doc strings of `boundaries.<patch>.turbulence`,
    `turbulence_length`, `turbulence.initial_intensity` and `initial_viscosity_ratio`.
11. **D11 Placement.** Changed: `crates/rcfd-api/src/turb.rs` (`freestream_role`, the new `freestream_fallback`,
    `check`, `TurbState::new`, the lib test), `crates/rcfd-api/src/schema.rs` (doc strings),
    `crates/rcfd-api/tests/defaults_w49c.rs` (D9), the docs of D10, ADR 0166 (pointer), the example trees (D7, D8). New:
    `crates/rcfd-api/tests/far_field_w49f.rs` (F1-F6), `scripts/w49f_legacy_check.py` (R1's driver), this ADR. No change
    to `sim.rs`, `rcfd-fv`, `rcfd-turb`, the cases or `scripts/make_example_projects.py`.

## Registered limits (fixed here, before any code and any run)

Tests (`cargo test --release --locked -p rcfd-api`, own target folder):

| Id | Check | Limit |
|---|---|---|
| F1 | SST and k-epsilon on ADR 0166's free-stream plate (T5 set-up), no `turbulence` line, 30 iterations | no validation error; `ux`, `uy`, `p`, `k`, `omega` (SST) / `epsilon` (k-epsilon) and the face flux equal bit for bit to the same case with `turbulence = "freestream"`; control: the explicit `zero_gradient` case differs |
| F2 | the F1 runs (`auto`) | every inflow face (`xmin`) of `k` and `omega` / `epsilon` equals, bit for bit, the D2 formula computed in the test from `k_from_intensity`, `epsilon_from_viscosity_ratio` and the defaults (I 0.05, ratio 10, U 1, nu 2e-7, Cmu 0.09; SST `omega = eps / (0.09 k)`); every outflow face (`xmax`) equals its owner cell |
| F3 | an explicit specification is honoured: SST, `auto`, `turbulence_length = 0.1` on both far-field patches | inflow faces equal `inlet_values_sst` on the length path (bit for bit) and differ from F2's value |
| F4 | validation | SST and k-epsilon with `turbulence = "inlet"` on a fixed-velocity inlet without `turbulence_length`: the `turbulence_length` error stays; with explicit `turbulence = "freestream"` and no spec: no error; RSM with explicit `freestream`: the RSM inlet error stays |
| F5 | the NACA example cases, both trees | `Case::load` + `semantic_errors()` empty for all 8; the far field's `turb::role` is `Inlet` and `freestream_fallback` true |
| F6 | live switch (replaces ADR 0166 T8 (a)): SA 20 iterations, switch to SST, 10 iterations | inflow faces of `k` and `omega` equal F2's values bit for bit; outflow faces equal their owner cells; all finite |
| F7 | lib test (replaces `freestream_far_field_default_is_sa_only`) | `role` of a free-stream patch under `auto`: `Inlet` for SA, SST, k-epsilon; `ZeroGradient` for RSM, Smagorinsky, mixing length, laminar and a zero `velocity_value`; explicit `zero_gradient`, `freestream`, `inlet` as spelled |
| T | `defaults_w49c` T1-T5, T8 (b), (c) | pass unchanged |

Runs (release build of this branch, binaries in `E:\projects\cfd\w49f-work\target\release`; their sha256 goes in the
addendum; at most 2 of my solver processes at a time, free memory checked first):

| Id | Run | Limit |
|---|---|---|
| R1 | old behaviour (D5): `scripts/w49f_legacy_check.py` copies `example-projects/wind-tunnel` (cases, meshes, manifest of the project, no runs) to `E:\projects\cfd\w49f-work\r1`, adds `turbulence = "zero_gradient"` to `[boundaries.farfield]` and `component = "total_molecular"` to `[reports.cl]` and `[reports.cd]` of `cases/naca0012-aoa4.toml`, runs that one case through the script's `Runner` (run service, 1 thread), and compares with the committed `r-20261028T152128-cb3a` (read from git at 1248a76) | `residuals.jsonl` byte-equal (2000 lines: `cl`, `cd` and the residuals of every iteration); the 10 stored snapshots' `ux`, `uy`, `p`, `nut` equal bit for bit; state `finished`, `not_converged`, 2000 iterations |
| W1 | Windows regeneration (D7) | exactly 4 runs added, of the 4 NACA cases; each `finished`, solver state `not_converged` at iteration 2000, exit code 5 (ADR 0142 G1); the ledger's `wall_s` of each at most 180 s and the tree's sum at most 1200 s (G2); the closing check of the tree 0 problems; `git diff --stat` against 1248a76: the other 4 wind-tunnel runs, `quick-start` and `meshes/` unchanged; `wind-tunnel` at most 35 000 000 bytes, the tree at most 40 000 000 |
| L1 | Linux regeneration (D7), in Docker | the same limits for `example-projects-linux`; container exit 0; `--check` of both trees 0 problems |
| E1 | `rcfd-mcp` `example_projects_w48h` (all its non-ignored tests, among them G1/G2 above, G3 `0 < cl(2) < cl(4)`, the derived-metric and store checks) | pass on both trees; its ignored tests stay ignored |
| E2 | `rcfd-mcp` `example_rotated_w48u` (ADR 0155 E2/E3: rotated against flow-rotated `cl`, `cd` within 1e-6 at iteration 2000 and 1e-5 over 1001-2000) | pass on both trees |
| E3 | `rcfd-studio` `example_projects_h`, `scale_n`, `mesh_transform_u`, `saved_store_r` (`cargo build --release -p rcfd-claude --bin fake-claude` first, ADR 0157 amendment 2) | pass; ignored as before |
| E4 | the script's self-test `python -m unittest scripts/test_make_example_projects.py`; `--check` | pass; 0 problems |

Hygiene:

| Id | Check | Limit |
|---|---|---|
| G1 | records | `git diff 1248a76 -- crates/rcfd-api/tests/baseline_hashes.rs` empty and the test passes (3 passed, 1 ignored); `case_hashes_w48u`, `rcfd-fv` `w33b_default_bits`, `rcfd-cli` `workspace_graph` pass |
| G2 | neighbours | `rcfd-api` lib tests, `registry_setups_w39a` (V1 includes the SST NACA set-up: exit 0, no error-level rule), `force_effective_w49a`, `turb_models`, `physics_schema`, `sst_schema`, `rules_w19c`, `w48v_adaptive_correctors`, `sa_drag_w49a`, `bc_catalogue` pass |
| G3 | style | `cargo fmt -p rcfd-api -- --check` clean; clippy `-D warnings` on the rcfd-api lib and the changed / new test files clean |

Reading rules:
- **Size caps.** If after W1 the Windows `wind-tunnel` project or tree is above its cap, or L1's is, I stop: no
  regenerated run of that tree is committed, and the options go to the maintainer. No cap is raised.
- **A regenerated run that misses W1 / L1 / E1 / E2** (for example: the targets met before 2000, so `converged`; a
  wall time above 180 s; rotated and flow-rotated runs further apart than 1e-6) is reported with its numbers. The runs
  are what the new default gives and are committed as made, in a commit of their own; the missed limit moves to an
  ignored `*_known_miss` test with the limit intact (the passing part of that test stays a normal test); nothing is
  tuned. A wall-time miss caused by host load is reported as such and is not re-run to get under the limit.
- R1 is evaluated once, on the registered run; a run that does not reach its comparison is "not evaluated", not met.
- If any `baseline_hashes.rs` case changes: stop and report.

**Reported, no limit:** the new `cl`, `cd` at iteration 2000 against the stored ones (0.3482 / 0.02536 at 4 degrees,
0.1988 / 0.01888 at 2 degrees); the momentum residual at 2000; the far-field k and omega; wall times; the byte sizes of
both trees before and after.

## Not in scope

- Moments (job 49G).
- Re-running ADR 0159's or ADR 0164's sweeps (job 49D runs ADR 0159's).
- The RSM inlet condition.
- Making the far field read `k_initial` / `omega_initial` (question 1).
- The case comments and the coarse-mesh limits of the example (ADR 0142).
</content>
</invoke>

## Questions for the maintainer (registered now; answers are not assumed)

1. **Far-field values of a case that sets `k_initial` / `omega_initial` (`epsilon_initial`).** D2 takes the far field from
   `initial_intensity` and `initial_viscosity_ratio`, as recommended in ADR 0166, so such a case gets a far field that
   differs from its initial field. (a) Keep D2. (b) Use `k_initial` and `omega_initial` / `epsilon_initial` for the far
   field when they are set. Recommendation: (b) in a later job if a case needs it; no committed case sets them together
   with a free-stream far field.

## Amendment 1 (2026-10-29, after the code and its tests, before any registered run): the base commit

**Defect 1.** The registration names the base commit as 1248a76 (status, G1, W1, R1). That was the worktree branch's
commit before the fast-forward; the fast-forward moved it to local main **1a04d46** ("Merge job 49C ..."), on which this
job is built (fd5bc8e, the registration, has parent 1a04d46). 1248a76 is an old ancestor that does not even hold the
example trees. Found when G1's `git diff 1248a76 -- baseline_hashes.rs` showed main's own earlier change (d59b202,
ADR 0137), not a change of this job.

**Correction 1.** Read 1a04d46 for 1248a76 everywhere in this ADR: G1 is `git diff 1a04d46 --
crates/rcfd-api/tests/baseline_hashes.rs` empty; W1 / L1's `git diff --stat` is against 1a04d46; R1 compares with
`r-20261028T152128-cb3a` as committed at 1a04d46 (`scripts/w49f_legacy_check.py --base`, default 1a04d46). No limit
changes. No registered run had been made.

## Interim note (2026-10-29, after W1, before any test edit and before L1): the regenerated aerofoil runs converge

R1 (old spellings, new binary) met: `residuals.jsonl` byte-equal over 2000 lines and 40 of 40 stored fields bit-equal
with `r-20261028T152128-cb3a` at 1a04d46; `finished`, `not_converged`, 2000 iterations. W1 (`--add-missing`, Windows)
added exactly the 4 NACA runs, but **each meets the residual targets**: solver state `converged`, exit 0, at iteration
409 (4 degrees, both forms) and 230 (2 degrees, both forms). So W1 misses ADR 0142 G1's aerofoil expectation
(`not_converged` at 2000, exit 5), and E2 (`example_rotated_w48u`, which asserts 2000 steps) misses as well. The rest of
W1 holds: wall times 7.9-16.3 s (the solver's `wall_s`; the ledger's `wall_s`, launch to final state, which G2 limits, is 8.1-16.5 s), closing check 0 problems, `wind-tunnel` 28 095 017 bytes (was 34 826 293). Per the
registered reading rule the runs are kept as made (nothing tuned, `max_iterations` untouched), the missed limits move to
ignored `*_known_miss` tests with the limits intact, and L1 runs with the same limits (the same miss is expected there).

## Amendment 2 (2026-10-29, after W1 and L1, before its run): R2, which change moves the drag

Not a defect: one more run, registered now, reported without a limit. The regenerated `cd` at 4 degrees is 0.02820
against the stored 0.02536; two changes act together (ADR 0166's effective wall shear in `cd`, this ADR's far field).
**R2**: `scripts/w49f_legacy_check.py run --variant molecular` (the R1 procedure with only the two
`component = "total_molecular"` lines, the far field at its new default) on the 4-degree flow-rotated case, Windows, the
same binary, work folder `E:\projects\cfd\w49f-work\r2`. Reported: iterations, solver state, final `cl` and `cd`.
Reading: R2's `cd` minus the stored 0.02536 is the far field's part (molecular shear); the regenerated 0.02820 minus R2's
`cd` is the wall-shear part. No limit; it changes no committed file except this ADR.

## Addendum: results (2026-10-29)

Commits on the branch (base 1a04d46):
- fd5bc8e: this registration;
- 1d29410: the code (`turb.rs`, `schema.rs` doc strings), the tests (F1-F6 in `far_field_w49f.rs`, F7, D9's respelling of
  `defaults_w49c.rs`), the docs, R1's driver, amendment 1;
- fde3f85: the interim note (after W1);
- 93980e1: W1, the 4 Windows runs, the ledger, the run table, the manifest and the hand-updated `notes/naca0012.md` and
  `notes/README.md` (D8);
- 63d957d: L1, the same for the Linux tree (the container copied the notes);
- e40ddc0: the known misses (below);
- 5723e8b: amendment 2;
- this addendum, with R2's `--variant` option of the driver.

Binaries (Windows, release, `--locked`, `CARGO_TARGET_DIR=E:\projects\cfd\w49f-work\target`, the code of 1d29410):
`rcfd.exe` sha256 `d6e9c6501a3bde5242028ed0ffc29377e2879dd83e46f024ddde8601cfde3574`, `rcfd-runner.exe`
`3a97b27fe990af5317948715cc5f833f464b7bffbde7066bd3f0ba3dc540ad38`. Linux: built in the container from the working
tree of 93980e1 with the 4 Linux runs deleted (1611 source files; the code is 1d29410's).

Host: shared with jobs 49D and 49E (four solver processes throughout) and FAN-01 test runs of another job; 37-39 GB free.
One solver process of mine at a time (W1 and L1 run their cases one after the other; L1 after W1).

### Limits

| Id | Measured | Outcome |
|---|---|---|
| F1 | SST and k-epsilon: no validation error; `auto` = explicit `freestream` bit for bit (ux, uy, p, k, omega / epsilon, flux after 30 iterations); explicit `zero_gradient` differs | met |
| F2 | inflow faces: k = 3.75e-3, SST omega 1875.0000000000002, k-epsilon epsilon 0.6328125000000002 (nu 2e-7), bit for bit the formula; outflow faces = owner cells | met |
| F3 | `turbulence_length = 0.1`: inflow omega 1.1180 = `inlet_values_sst` (length path), not the fallback 1875; k the same | met |
| F4 | the `turbulence_length` error stays for a fixed-velocity inlet and for an explicit `inlet` role; explicit `freestream` without a spec validates; RSM with explicit `freestream` keeps its error; RSM under `auto`: `ZeroGradient`, no error on the far field | met |
| F5 | the 8 example cases validate; far field `Inlet` with the fallback | met |
| F6 | SA 20 iterations, switch to SST, 10 iterations: inflow faces k, omega = F2's values bit for bit; outflow = owner | met |
| F7 | `freestream_far_field_default_is_sa_sst_and_k_epsilon` (rcfd-api lib: 106 passed, 2 ignored, as before) | met |
| T | `defaults_w49c` T1-T5, T8 (b), (c): 5 passed | met |
| R1 | old spellings on the 4-degree case, new binary: `finished`, `not_converged`, 2000 iterations; `residuals.jsonl` 2000 of 2000 lines byte-equal; 40 of 40 stored fields (10 snapshots x ux, uy, p, nut) bit-equal with `r-20261028T152128-cb3a` at 1a04d46 | met |
| W1 | 4 runs added, exactly the NACA cases; **each `converged`, exit 0, at iteration 409 (4 degrees, both forms) and 230 (2 degrees, both forms)**; walls 16.0, 9.9, 16.5, 8.1 s, tree sum 127.5 s; closing check 0 problems; the other 4 wind-tunnel runs, `quick-start`, `meshes/` and `cases/` unchanged against 1a04d46; `wind-tunnel` 28 095 017 bytes (cap 35 000 000; was 34 826 293), tree 32 349 247 (cap 40 000 000) | **missed** (G1's aerofoil state, steps and exit code); the rest met |
| L1 | container exit 0; 14 inputs copied; 4 runs added, the same states and iteration counts as W1; walls 28.9, 15.3, 27.8, 15.2 s, tree sum 235.5 s; `--check` of both trees 0 problems; the other Linux runs, `quick-start`, `meshes/` unchanged; `wind-tunnel` 28 093 048 bytes (was 34 823 333), tree 32 346 672 | **missed** (as W1); the rest met |
| E1 | `example_projects_w48h`: 21 passed, 2 ignored (ADR 0142's crossing known miss and the new G1 known miss). G2 (every run, both trees), G3 (`0 < cl(2) < cl(4)`), S1-S9, T1-T3 pass | met except G1's aerofoil part (known miss) |
| E2 | `example_rotated_w48u`: E3 passes; E2 fails at its `steps == 2000` assertion (409) | **missed** (known miss) |
| E3 | `example_projects_h` 1 passed; `mesh_transform_u` 4; `saved_store_r` 6; `correctors_x` 6 passed, 5 ignored (its own); `scale_n` N9 fails at "snapshot 10/10  iteration 2000" (shows `snapshot 3/3  iteration 409`), otherwise 7 passed | met except N9's steady snapshot line and strip (known miss) |
| E4 | `python -m unittest scripts/test_make_example_projects.py` 15 OK; `--check` 0 problems | met |
| G1 | `git diff 1a04d46 -- crates/rcfd-api/tests/baseline_hashes.rs` empty; 3 passed, 1 ignored; `case_hashes_w48u` 1 passed; `w33b_default_bits` 1; `workspace_graph` 3 | met |
| G2 | `registry_setups_w39a` 2 passed, 1 ignored (V1 naca0012: exit 0, 0 errors, SV101 warning as before); `force_effective_w49a` 4; `turb_models` 8 (1 ignored); `physics_schema` 11; `sst_schema` 3; `rules_w19c` 5 (1 ignored); `w48v_adaptive_correctors` 5 (4 ignored); `sa_drag_w49a` 8 (3 ignored); `bc_catalogue` 22 (9 ignored); `mesh_transform_w48u` 13 | met |
| G3 | `cargo fmt -- --check` clean for rcfd-api, rcfd-mcp, rcfd-studio; clippy `-D warnings` clean on the rcfd-api lib, `far_field_w49f`, `defaults_w49c`, `example_projects_w48h`, `example_rotated_w48u`, `scale_n` | met |

Known misses (reading rule; each limit kept exactly as registered, the passing part stays a normal test):
- `example_projects_w48h::g1_aerofoil_runs_stop_not_converged_at_2000_known_miss` (ignored): ADR 0142 G1's aerofoil
  tuple (`not_converged`, 2000, exit 5). `g1_g2_runs_finished_as_registered_within_their_time` keeps G1 of every other
  run and G2 of every run.
- `example_rotated_w48u::e2_rotated_mesh_matches_rotated_flow_known_miss` (ignored), unchanged body.
- `scale_n::n9_steady_words_name_snapshot_10_and_iteration_2000_known_miss` (ignored): ADR 0148 N9's two steady
  assertions that name the stored run's 10 snapshots and 2000 iterations; `n9_every_time_says_what_it_is` keeps the
  rest (the steady slider word `iteration`, no `time` in the steady strip, the x axis, the Runs panel column).

### Reported (no limit)

| Case | stored before (Windows) | regenerated, Windows | regenerated, Linux |
|---|---|---|---|
| aoa4 | 2000 it, not_converged, cl 0.348195, cd 0.025360 | 409 it, converged, cl 0.346925148, cd 0.028203441 | 409, cl 0.346925145, cd 0.028203441 |
| aoa2 | 2000 it, not_converged, cl 0.198833, cd 0.018876 | 230 it, converged, cl 0.198177659, cd 0.021964573 | 230, cl 0.198177656, cd 0.021964573 |
| aoa4-rotated | as aoa4 | 409, cl 0.346925147, cd 0.028203441 | 409, cl 0.346925146, cd 0.028203441 |
| aoa2-rotated | as aoa2 | 230, cl 0.198177657, cd 0.021964573 | 230, cl 0.198177657, cd 0.021964573 |

- Momentum residual at the end: 4.4e-8 (4 degrees), 7.6e-8 (2 degrees); stored runs: 1.4e-6 at 2000.
- Rotated against flow-rotated (what E2 measured, at the final iteration instead of 2000): `cl` 2.6e-9 / 5.2e-9 relative
  (Windows, 4 / 2 degrees), `cd` 1.4e-9 / 7.7e-10; Linux `cl` 6.0e-10 / 3.5e-9, `cd` 2.3e-10 / 5.2e-10; equal iteration
  counts and report iterations. Largest difference in the first 30 iterations (6e-4 in `cl`).
- Far field of the examples (D2): k = 3.75e-3, omega = 2250 (nu = 1/6e6), the initial values.
- Snapshots: 3 (200, 400, 409) and 2 (200, 230) instead of 10; each NACA run folder about 2.1 / 1.9 MB instead of
  about 3.6 MB.
- **R2 (amendment 2)**, the new far field with molecular `cl` / `cd`: 409 iterations, converged, `cl` 0.347151, `cd`
  0.025420. So at 4 degrees the far field moves `cd` by +0.00006 (+0.2 %) and `cl` by -0.0010; ADR 0166's effective wall
  shear moves `cd` by +0.00278 (+11 %) and `cl` by -0.0002. The drag change of the regenerated examples is almost all
  ADR 0166's; the far field changes the convergence (targets met at 409 instead of not at 2000).
- Lift slope of this mesh: (0.346925 - 0.198178) / 2 = 0.0744 per degree (stored runs 0.0747).

### Deviations

- Amendment 1: the base commit was misnamed (1248a76 for 1a04d46) in the registration, corrected before any run.
- The interim note and the three known misses follow the registered reading rule. The regenerated runs are kept as
  made; `max_iterations`, the case files and their comments are untouched (D6).
- D8 named only the run numbers in `notes/naca0012.md`; four sentences that described the old runs' stopping at 2000
  iterations (the solver bullet, "What to click" step 2, the limits bullet, `notes/README.md`'s limits bullet) were
  rewritten too, because they had become false; the earlier numbers are kept there as attributed history. A sentence
  on the far field and on how to reproduce the old runs (D5) was added.
- The Windows `manifest.json` gained a `platform` block (os windows, x86_64, Python 3.9.13): `write_manifest` writes it
  on every run of the script since ADR 0150; the committed Windows manifest predates that.
- An attempt to commit the interim note picked up the staged deletion of the 4 Windows runs; it was undone locally
  (`git reset --soft`, not pushed) and recommitted as fde3f85 (note only) and 93980e1 (runs).

### Not run

- The full workspace test suite, the gate, the whole `rcfd-studio` suite (only the tests that read the example runs,
  E3, and `correctors_x`), Linux tests (only L1's runs and its `--check` in the container).
- `--rerun-check` of either tree (the 4 new runs are expected to be byte-reproducible by ADR 0142 D10's procedure; not checked).
- The ignored heavy FAN-01 runs (no free-stream patch, D1 cannot reach them) and ADR 0159's / 0164's sweeps.
- A k-epsilon or SST far-field validation against reference data (the plate tests are set-up checks).

### Questions for the maintainer

1. (Registered above.) Far-field values when `k_initial` / `omega_initial` are set. Recommendation: (a) keep D2 now;
   (b) later if a case needs it.
2. **The aerofoil examples now converge.** ADR 0142 G1 (aerofoils `not_converged` at 2000, exit 5), ADR 0155 E2
   (comparison at iteration 2000) and ADR 0148 N9 (steady words "snapshot 10/10  iteration 2000") describe the old runs
   and are now known misses. (a) Re-register them in a small job for the new runs: aerofoils `converged`, exit 0, at
   most 2000 iterations; E2 at the final iteration (measured: within 5.3e-9) with equal iteration counts; N9 on the
   run's own last snapshot. (b) Keep the known misses. Recommendation: (a).
3. **The case comment** `max_iterations = 2000   # the run stops here: the residual targets are not reached on this mesh
   (ADR 0142)` in the 4 aerofoil cases is now false. Editing it changes the toml hashes recorded in
   `case_hashes_6867e40.txt` and the Studio plans fixture, and makes the stored runs stale (`check_run` compares the case
   with its snapshot), so the 8 runs would have to be regenerated once more. (a) Fix the comment together with question
   2 (a), with the fixtures' aerofoil lines handled under a registered note, and regenerate the 8 runs again; (b) leave
   it. Recommendation: (a), in the same job as question 2.
4. **Job 49D's SST arm** runs on a binary without this change; its SST numbers are a record of the pre-49F far field.
   To reproduce them after this merge, add `turbulence = "zero_gradient"` to the far field (and `component =
   "total_molecular"` for ADR 0159's spelling). Nothing to decide unless 49D's results are to be re-run with the new
   default.
5. **A far field that sets only `turbulence_intensity`** (default spec, no length) gets D2's fallback, which takes
   `turbulence.initial_intensity`, not the patch's intensity. Such a case was a validation error before, so no valid case
   moves, but the patch key is silently unused. (a) Keep, documented (`docs/case-schema.md` now says so, after the
   addendum). (b) Use a patch `turbulence_intensity` / `turbulence_viscosity_ratio` that the case sets explicitly
   before the initial-field keys (ADR 0152's key-at-default rule tells them apart). Recommendation: (a) now, (b) in a
   later job.

Pointers (dated one-liners, after the addendum): ADR 0142 (G1), ADR 0148 (N9) and ADR 0155 (E2) note that their aerofoil
limits are known misses under this ADR.
