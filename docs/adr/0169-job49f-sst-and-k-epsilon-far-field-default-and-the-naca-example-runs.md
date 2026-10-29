# ADR 0169: Job 49F, `freestream` becomes the far-field default of k-omega SST and k-epsilon; the 8 stored NACA 0012 example runs are regenerated

Status: **registered 2026-10-29**, before any code and before any solver run of this job. The enumeration, the design,
the limits, the reading rules and the not-run list are fixed here. Results: addendum (not yet written). Not yet accepted
by the maintainer. Job 49F, on local main 1248a76 (the worktree branch was fast-forwarded to it before any work; it holds
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
