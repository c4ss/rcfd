# ADR 0166: Job 49C, the effective wall shear and the SA far-field turbulence become defaults (ADR 0164 questions 1, 2, 4, 5)

Status: **registered 2026-10-29**, before any code and before any solver run of this job. The enumeration of what changes,
the design, the limits, the reading rules and the not-run list are fixed here. **Results in the addendum:** every
registered limit met, no known miss; amendment 1 (the live model switch) corrected a defect found by review after the
results; no stored example run and no `baseline_hashes.rs` case changes. Not yet
accepted by the maintainer. Job 49C, on local main f9a520a (the
worktree branch was fast-forwarded to it before any work). Number: 0165 is the highest in this worktree and in the main
checkout; 0167 is taken by job 49D on another branch; 0166 was free when this was written.
**2026-10-29: D3 and test T4 are superseded by [ADR 0170](0170-job49g-moment-reports-follow-the-effective-wall-shear-default.md)
(question 2 (a)): `moment` reports now follow the force default; T4 is respelled as T4'.**

House rules of [ADR 0130](0130-wave48a-overview-and-run-record-contract.md):
- limits are fixed here before the registered runs; a missed limit becomes an ignored `*_known_miss` test with the limit
  intact;
- a defective set-up gets an amendment before the corrected run;
- defaults of existing code stay bit-identical except where the maintainer's decision below changes them, and every such
  change is listed here before the code;
- `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded;
- no new crate, no unsafe code, the crate graph stays acyclic, no commit trailers, nothing is pushed.

Parents:
- [ADR 0164](0164-job49a-why-the-sa-drag-of-the-naca0012-is-wrong.md): the opt-in values `total_effective` /
  `viscous_effective` and `turbulence = "freestream"`, and its questions 1, 2, 4 and 5.
- [ADR 0159](0159-job48y-naca0012-angle-of-attack-sweep-through-stall.md): the published numbers that must stay
  reproducible.
- [ADR 0156](0156-job48v-adaptive-pressure-correctors.md) D6, [ADR 0160](0160-job48z-end-time-stamp-corrector-count-record-sv301-wiring.md)
  and [ADR 0163](0163-job48ac-case-hashes-h1-after-the-corrector-control-key.md): how a new key at its default is kept
  out of the case hash (`Schema::hash_omit_default`). [ADR 0152](0152-job48r-store-key-at-its-default-is-not-the-case-s-own.md):
  a key at its schema default is not the case's own.

## The maintainer's decision (2026-10-29, relayed)

"Add all those changes", in reply to ADR 0164's questions, with the recommendations:
1. Question 1 (b): the effective wall shear becomes the default of the force reports. `reports.<n>.component` `total` and
   `viscous` take the momentum equation's face viscosity, which is what ADR 0164 added as `total_effective` and
   `viscous_effective`.
2. Question 2 (b): `freestream` becomes the default turbulence role of far fields with `velocity = "freestream"`. Today
   it is `zero_gradient`.
3. Question 4 (a): a one-line dated note in ADR 0159 that its SA-a00 run stopped at 5157 iterations, not 8000.
4. Question 5 (a): document `solver.min_iterations` accurately: it gates only the monitor convergence conditions and does
   not stop a run that ends at the residual targets.

Question 3 (re-run ADR 0159's sweep) is job 49D, not this job. The brief adds:
- keep the old behaviours spellable as explicit values, so that ADR 0159's published numbers can be reproduced by naming
  them;
- if any case of `baseline_hashes.rs` would change, stop and report;
- do not regenerate stored example runs; list them as questions.

## What I read before writing this (rule d): code and stored runs, no solver run

1. **The force report** (`Simulation::eval_report`, `crates/rcfd-api/src/sim.rs`). One closure, `wall(origin)`, serves the
   `force`, `force_coefficient` and `moment` kinds.
   - It takes the face viscosity (`Simulation::momentum_face_viscosity`) only for the two `*_effective` values. Otherwise it
     takes `physics.nu`.
   - `momentum_face_viscosity` is `Some` for the steady, coupled and transient engines once a step has set `nu_face`.
   - `nu_face` is set by a turbulence model (`TurbState::nu_face`). It is also set in **laminar** runs by `rheology`, a
     user-function viscosity and temperature-dependent `properties` (sim.rs, the steady and transient step).
   - So "effective whenever a face viscosity exists" would change laminar variable-viscosity cases, and is not what this
     job does (D1).
   - `component` is ignored by `moment` reports except for the `*_effective` switch. A moment report always gives the
     moment of the whole wall force.
2. **The turbulence role** (`turb::role`, `crates/rcfd-api/src/turb.rs`).
   - Under `auto`, a `velocity = "freestream"` patch falls through to `Role::ZeroGradient`.
   - The explicit `freestream` (ADR 0164 amendment 1) maps to `Role::Inlet`. `TurbState::new` then builds
     `rcfd_fv::bc::Freestream { fixed_on_inflow: true }` instead of `FixedValue`.
   - `Role::Inlet` has consequences beyond the boundary condition:
     - `turb::check` requires an inlet specification for k-epsilon and SST: under the default
       `turbulence_spec = "intensity_length"`, a positive `turbulence_length`, else a located error. For SA, plugin and
       algebraic models nothing is required.
     - `turb::check` rejects an inlet role for the Reynolds stress model ("no inlet condition for the stresses").
     - `reference_speed` (the speed of the initial k) includes the patch's `velocity_value`.
     - `Simulation::new` collects the patch's per-face inlet speeds.
   - **Consequence:** the decision's literal form (every model) turns every existing k-epsilon or SST case with a
     `freestream` far field and no `turbulence_length` into a validation error. That includes the 4 NACA 0012 example
     cases of each example tree. An RSM case with such a far field would also become an error. See D2.
3. **Case hashes.** The case hash (`SettingsTree::content_hash`) and the written settings text carry values, not
   allowed-value lists. A new enumeration value adds nothing to a case that does not use it. Neither default value
   changes (`turbulence` stays `"auto"`, `component` stays `"total"`). So no key is added, `hash_omit_default` is not
   needed, and `case_hashes_w48u` / `w48v_case_hashes` keep their values (predicted; checked in G1).
4. **ADR 0159's stored runs** (`E:\projects\cfd\w48y-work\w\runs`, read only).
   - SA-a00's `monitors.csv` has 5157 rows; its status file says "residual targets met", exit 0.
   - SA-a17 has 8000 rows, also exit 0 with the targets met. So ADR 0159's "0 and 17 ... met the targets at iteration
     8000" is wrong only for 0 degrees.
5. **ADR 0164's stored runs** (`E:\projects\cfd\w49a-work\w\runs`, read only).
   - F10 is ADR 0159's SA-a10 case text plus `boundaries.farfield.turbulence = "freestream"` and extra reports (`cd_p`,
     `cd_v`, `cd_eff`, `cd_veff`, `cl_eff`, `nut_avg`).
   - It met the residual targets at iteration 577 (`monitors.csv` 577 rows).
6. **`solver.min_iterations`** (`Simulation::step`).
   - The stop reasons are tested in order: a user-function error, a particle or other override, the residual targets
     (`summary.converged`), the iteration limit or end time (`summary.done`). Only then comes
     `summary.index >= min_iterations`, the evaluation of the `convergence.<name>` conditions (stop and interrupt).
   - So `min_iterations` delays only those conditions. A steady run that meets `tolerance_u` / `tolerance_p` (and, with a
     turbulence model, `res_nut`) earlier stops there.
   - The schema text says "Convergence conditions are only evaluated from this iteration / step on". That is right but
     incomplete.
   - Neither `docs/case-schema.md` nor `docs/run-control.md` mentions the key.

## Enumeration: what the design below changes (registered before any code)

Searched: every `*.toml` case (`cases/**`, `example-projects*/**/cases`, test fixtures), every Rust test and source literal,
`scripts/*.py`, `python/`, `clients/`, the registry descriptors `cases/*/case.json`, and the stored example runs. The
search was for:
- force, force-coefficient and moment reports together with a turbulence model;
- `velocity = "freestream"` together with a turbulence model.

### Change A: `force` and `force_coefficient` reports, components `total` / `viscous` (default or explicit), with a turbulence model

| Item | Turbulence | What moves | Committed files changed by this job |
|---|---|---|---|
| `example-projects/wind-tunnel/cases/naca0012-aoa2.toml`, `-aoa4`, `-aoa2-rotated`, `-aoa4-rotated` | SST | `cl`, `cd` of a re-run. Fields unchanged: the cases have no convergence condition on them | none |
| the same 4 cases in `example-projects-linux/wind-tunnel` | SST | the same | none |
| stored runs: `example-projects/wind-tunnel/runs/r-20261028T152128-78e0` (aoa2), `-cb3a` (aoa4), `r-20261028T224153-afb6` (aoa2-rotated), `-d9c7` (aoa4-rotated) | SST | a re-run's `residuals.jsonl` monitors, `metrics.json` reports, the ledger's `cl`, the numbers in `notes/naca0012.md` | none; they stay records of the earlier default (question 3) |
| stored runs: `example-projects-linux/wind-tunnel/runs/r-20261028T220207-763f` (aoa2), `-6504` (aoa4), `r-20261029T000812-00f3` (aoa2-rotated), `-6c75` (aoa4-rotated) | SST | the same | none |
| `scripts/w48y_aoa_sweep.py` (ADR 0159's driver) | SA, SST, SA-DDES | `cl`, `cd` of every arm on a re-run | none (job 49D's file) |
| `scripts/w49a_sa_drag.py` (ADR 0164's driver) | SA, SST | `cd`, `cl`, `cd_v` of arms A, B, F, C, S; `cf_v` of the plates | none (its limits read the committed `results.json`) |
| `crates/rcfd-api/tests/force_effective_w49a.rs` C2/C3 | SA | `d_visc` / `d_total` become the effective values, so C2's ratio becomes 1 and C2 fails as written | respelled with the molecular values, limits unchanged (D4) |

**Not changed by A:**
- every laminar case, including laminar variable-viscosity cases (D1);
- every `moment` report (D3), so the FAN-01 tests are unchanged (`fan01_validation`, `fan01_gci_w37c`,
  `fan01_diagnosis_w35c`, `fan01_stall_w43c`, `fan01_rotor_frame_w36c`: SST with `moment` reports only; their
  non-ignored tests hash fields, not reports);
- the `pressure` component (computed without a viscosity);
- `y_plus` reports and the `wall_shear` post-processing output.

### Change B: the turbulence role of a `velocity = "freestream"` patch under `turbulence = "auto"`

| Item | Turbulence | What moves | Committed files changed by this job |
|---|---|---|---|
| ADR 0159's SA and SA-DDES arms (`w48y_aoa_sweep.py`) | SA | fields and every report of a re-run | none |
| ADR 0164's arms A00, A10, C10 (`w49a_sa_drag.py`). B (explicit `inlet`), F (explicit `freestream`) and S (SST) are unchanged | SA | the same; A becomes F | none |
| any user case: SA with a `freestream` far field and `auto` | SA | the same | - |

**Not changed by B:**
- any committed case, example or test literal. No committed case or Rust test runs SA with a `freestream` far field
  under `auto`. `force_effective_w49a.rs` C4 spells `freestream` explicitly; C5 tests an error that stays.
- SST, k-epsilon, RSM, plugin and algebraic models (D2), so the 8 NACA example cases (SST) keep their fields;
- `registry_setups_w39a`'s NACA 0012 set-up (SST);
- `statistics.rs`'s cylinder (laminar).

### Records and gates

- **`crates/rcfd-api/tests/baseline_hashes.rs`: no case changes** (predicted; checked in G1, the file is not edited).
  - Its cases hash fields and fluxes after 60 iterations.
  - The only turbulent one, `cases/channel/kepsilon_channel.toml`, has no `freestream` patch (periodic channel, fixed walls).
    Its only reports are `y_plus`, which feed one `window_range` condition; `y_plus` is not touched.
  - The cavity, heated-cavity, `packed_bed`, `sod` and `wedge` cases are laminar or compressible.
- **`rcfd-fv` `w33b_default_bits`**: `rcfd-fv` is not touched.
- **`case_hashes_w48u` (fixture `case_hashes_6867e40.txt`), `w48v_case_hashes`**: no settings text changes (item 3 above).
- **Studio `saved_store_r` plans fixture**: start plans do not read turbulence roles or report components.
- **MCP and Studio example tests** (`example_projects_w48h`, `example_rotated_w48u`, `example_projects_h`, `scale_n`) read
  the stored runs and do not re-run them. They pass unchanged (predicted; G2).
- **Validation tests with tolerance bands that run a turbulent case with force reports**: none outside the ignored FAN-01
  runs (moments, unchanged) and `force_effective_w49a` (above).

## Decisions (mine, within the maintainer's decision; none is presented as the maintainer's)

1. **D1 The force default follows the turbulence model.**
   - For `type = "force"` and `"force_coefficient"`, the components `total` (default) and `viscous` take the momentum
     equation's face viscosity when a turbulence model is active (`Simulation::turb` is set: RANS, DES, LES, mixing
     length, plugin) and the engine has a face viscosity. This is exactly ADR 0164's `total_effective` /
     `viscous_effective`.
   - Without a turbulence model they stay molecular. That covers laminar constant viscosity, where both are equal bit
     for bit as before, and laminar variable viscosity (`rheology`, user-function viscosity, `properties`), where they
     are not equal.
   - Why gate on the model and not on the face viscosity: the decision is about wall functions. Laminar variable-viscosity
     cases are outside it and must stay bit-identical. Whether they should follow is question 4.
2. **D2 The far-field default is Spalart-Allmaras only.**
   - Under `turbulence = "auto"`, a patch with `velocity = "freestream"` gets the `freestream` role when all of these hold:
     - `turbulence.model = "spalart_allmaras"` (SA, also its DES/DDES options);
     - the patch is not a boundary-catalogue kind or a wall-motion kind (as today, those come first);
     - its `velocity_value` is non-zero (a zero free stream has no inflow faces to fix).
   - Otherwise `auto` resolves as before.
   - One helper, `turb::freestream_role(case, patch)`, decides this for both `turb::role` and `TurbState::new`, so the two
     cannot disagree. The explicit `turbulence = "freestream"` is unchanged.
   - Why SA only:
     - SA's free-stream value is fully set by existing keys (`turbulence.nutilde_ratio`, or `nutilde_value`).
     - For k-epsilon and SST, the inlet role requires an inlet specification that existing far-field cases do not carry
       (`turbulence_length`). The literal default would turn them into errors: the 8 NACA example cases,
       `registry_setups_w39a`'s NACA 0012, and any user case.
     - Choosing a free-stream k and omega source (for example `turbulence.initial_intensity` and
       `initial_viscosity_ratio`, the TMR and Menter far-field practice) is a new decision. It is question 1, not taken
       here.
     - RSM has no inlet condition at all.
3. **D3 Moment reports keep their default.**
   - `type = "moment"` with `component` unset, `total`, `viscous` or `pressure` stays molecular, as today. `total_effective`
     and `viscous_effective` give the effective moment, as since ADR 0164.
   - The decision names force reports, and `component` is documented for `force` and `force_coefficient` only.
   - Moments follow only if the maintainer says so (question 2). Making them follow would move the torque of every
     FAN-01 run.
4. **D4 The old behaviours stay spellable.**
   - New `component` values:
     - `total_molecular` and `viscous_molecular`: the viscous stress with `physics.nu`, exactly the pre-change `total` /
       `viscous` computation (`rcfd_post::integrals::wall_force_and_moment` with `self.nu`), for every report kind and
       model.
     - `total_effective` and `viscous_effective` keep their meaning.
   - The old far-field role is the existing explicit `turbulence = "zero_gradient"`.
   - To reproduce ADR 0159 with this build:
     - add `component = "total_molecular"` to the `cl` and `cd` reports;
     - for the SA and SA-DDES arms, also add `turbulence = "zero_gradient"` to `boundaries.farfield`.
   - To reproduce ADR 0164 as well: `cd_v` and the plates' `cf_v` become `viscous_molecular`, and the A arms take
     `turbulence = "zero_gradient"`.
   - `force_effective_w49a.rs` C2/C3: the molecular reports are respelled `viscous_molecular` / `total_molecular`. The
     limits (ratio > 1.5; pressure part within 1e-12) and the plate are unchanged. C1 is unchanged.
5. **D5 No new key.** Only enumeration values are added and both defaults keep their spelling. So the written settings text
   and every case hash stay as they are (item 3).
6. **D6 Documentation.**
   - `docs/case-schema.md`:
     - the far-field note under `spalart_allmaras`;
     - the `turbulence` key bullet;
     - the force-parts paragraph;
     - a `solver.min_iterations` note.
   - `docs/run-control.md`: a `min_iterations` paragraph.
   - The schema doc strings of `reports.<n>.component`, `boundaries.<patch>.turbulence` and `solver.min_iterations`.
   - A dated one-line correction in ADR 0159 (SA-a00 stopped at 5157).
   - A one-line pointer in ADR 0164's status.
7. **D7 Placement.**
   - Changed: `crates/rcfd-api/src/sim.rs` (`eval_report`), `crates/rcfd-api/src/turb.rs` (the helper, `role`,
     `TurbState::new`, unit tests), `crates/rcfd-api/src/schema.rs` (two values, three doc strings),
     `crates/rcfd-api/tests/force_effective_w49a.rs` (D4), the docs of D6.
   - New:
     - `crates/rcfd-api/tests/defaults_w49c.rs` (T1-T6);
     - `scripts/w49c_reproduce.py`, which builds the R cases from the stored ones, runs them and compares them;
     - this ADR.
   - No file under `example-projects*/`, `cases/` or `crates/rcfd-post` changes.

## Registered limits (fixed here, before any code and any run)

Tests (debug or release, `cargo test -p rcfd-api`):

| Id | Check | Limit |
|---|---|---|
| T1 | Laminar, constant viscosity (ADR 0164's plate, laminar, 40 iterations) | `total`, `total_molecular`, `total_effective` equal bit for bit; likewise `viscous`, `viscous_molecular`, `viscous_effective` |
| T2 | Laminar, variable viscosity (the same plate with a `rheology` power law, 40 iterations) | `total` = `total_molecular` and `viscous` = `viscous_molecular` bit for bit; control: `viscous_effective` differs from `viscous` |
| T3 | SA at y+ 40 (ADR 0164's C2 plate, 400 iterations) | `total` = `total_effective` and `viscous` = `viscous_effective` bit for bit; `viscous_effective / viscous_molecular > 1.5`; `(total - viscous) - (total_molecular - viscous_molecular)` within 1e-12 relative |
| T4 | Moments, the T3 run | a `moment` report with the default component equals one with `total_molecular` bit for bit; control: `total_effective` differs |
| T5 | SA far-field default (C4's plate, free-stream inflow and outflow, no `turbulence` line, 30 iterations) | `ux`, `uy`, `p`, `nuTilda` and the face flux equal bit for bit to the same case with `turbulence = "freestream"`; inflow faces hold `nuTilda = 3 nu`; control: the case with `turbulence = "zero_gradient"` differs |
| T6 | Scope of D2 | SST and k-epsilon on T5's plate without a `turbulence` line: no validation error, and `ux`, `uy`, `p`, `k` after 30 iterations equal bit for bit to the explicit `zero_gradient` case. `turb::role` unit test: SA with a zero `velocity_value` gives `ZeroGradient`; RSM with a free stream gives `ZeroGradient`; a laminar case's role is unchanged |
| T7 | ADR 0164 C1-C5 | pass, with C2/C3 respelled (D4) |

Runs (release build of this branch, `cargo build --release --locked -p rcfd-cli`, own `CARGO_TARGET_DIR`
`E:\projects\cfd\w49c-work\target`). The binary is copied to `E:\projects\cfd\w49c-work\rcfd-w49c.exe`; its sha256 goes
in the addendum. Work folder `E:\projects\cfd\w49c-work\runs`. At most 2 of my solver processes at a time, free memory
checked first. ADR 0159's and ADR 0164's work folders are read only.

| Id | Run | Limit |
|---|---|---|
| R1 | ADR 0159's SA-a00 `case.snapshot` with D4's legacy spellings (`component = "total_molecular"` on `cl` and `cd`, `turbulence = "zero_gradient"` on the far field), the same `rcfd run` options as ADR 0159, `--max-elapsed 240` | every `cd` and `cl` of every iteration present in both equal to ADR 0159's SA-a00 `monitors.csv` (parsed as f64, compared by bits); at least 100 iterations compared |
| R2 | the same for SA-a10 | the same against SA-a10 |
| R3 | ADR 0164's F10 `case.snapshot` with its `turbulence = "freestream"` line removed (the new default), `--max-elapsed 1200` | same iteration count as F10 (577) and the same stop reason; at every iteration `cd` = F10's `cd_eff`, `cl` = F10's `cl_eff`, `cd_v` = F10's `cd_veff`, and `cd_eff`, `cd_veff`, `cl_eff`, `cd_p`, `res_u`, `res_p`, `res_nut`, `continuity`, `nut_avg` equal to F10's, all by bits |

Hygiene:

| Id | Check | Limit |
|---|---|---|
| G1 | Records | `git diff f9a520a -- crates/rcfd-api/tests/baseline_hashes.rs` empty and the test passes (3 passed, 1 ignored as before); `case_hashes_w48u`, `w48v_case_hashes` pass; `rcfd-fv` `w33b_default_bits` passes; `rcfd-cli` `workspace_graph` passes |
| G2 | Example trees | `git diff f9a520a --stat -- example-projects example-projects-linux cases` empty; `rcfd-mcp` `example_projects_w48h` and `example_rotated_w48u` pass |
| G3 | Neighbours | `rcfd-api` lib tests, `physics_schema`, `sst_schema`, `turb_models`, `registry_setups_w39a`, `rules_w19c` (fenced TOML of both docs), `w48v_adaptive_correctors` pass |
| G4 | Style | `cargo fmt -p rcfd-api` clean; clippy `-D warnings` on the rcfd-api lib and the two test files clean |

Reading rules:
- R1-R3 are evaluated once, on the registered runs, by `scripts/w49c_reproduce.py compare` as committed.
- A run that does not reach its comparison is "not evaluated", not met.
- R1/R2 compare a prefix: the wall cap stops them long before ADR 0159's 5157 / 8000 iterations. A per-iteration match of
  the prefix shows the same arithmetic path. The full length is not re-run.

**Reported, no limit:** the default `cd` / `cl` of R1/R2's cases without the legacy spellings is not run. R3 stands for the
new default; it is the same set-up.

## Not in scope

- Re-running ADR 0159's sweep (job 49D).
- Regenerating any stored example run (question 3).
- The SST and k-epsilon far-field default (question 1).
- Moments (question 2).
- Laminar variable viscosity (question 4).
- An opt-in key that holds steady runs to `min_iterations` (ADR 0164 question 5 (b)).
- `wall_shear` post-processing output.

## Questions for the maintainer (registered now; answers are not assumed)

1. **SST and k-epsilon far fields under `auto`.** The literal default needs an inlet specification these cases lack.
   - (a) Keep `zero_gradient` for them, as done here.
   - (b) Make `freestream` their default too, with free-stream values from `turbulence.initial_intensity` and
     `initial_viscosity_ratio` when the patch gives no inlet specification. That is the TMR and Menter far-field practice.
     It changes the fields of the 8 NACA example runs.
   - (c) Make it their default only where the patch has an inlet specification.

   Recommendation: (b), in a job of its own with a set-up check on the NACA example. (c) makes a role depend on an
   unrelated key.
2. **Moment reports.**
   - (a) Make the default `moment` use the effective wall shear under a turbulence model, like force reports. FAN-01's
     torque monitors move; its runs are ignored tests whose registered numbers would need re-reading.
   - (b) Keep, as done here.

   Recommendation: (a), with FAN-01's torque known misses re-read in that job. The viscous part of the torque has the same
   factor-3 error on wall-function meshes.
3. **The 8 stored NACA 0012 example runs** (4 per tree) were made under the old default. A re-run gives other `cl` and
   `cd`; the fields are unchanged.
   - (a) Leave them as records of the version that made them, and add a sentence to `notes/naca0012.md`.
   - (b) Regenerate them in a registered job. The Linux tree runs in Docker. Both trees are within about 175 KB of their
     size caps.

   Recommendation: (b), together with question 1 (b) if chosen, so that the examples change once. (a) until then; the note
   is not added here.
4. **Laminar variable viscosity** (`rheology`, user-function viscosity, `properties`). `total` takes `physics.nu`, not the
   apparent viscosity, so a non-Newtonian wall shear is reported with the wrong viscosity.
   - (a) Make the default follow the face viscosity there too. That changes those cases' reports.
   - (b) Keep (`total_effective` is the opt-in).

   Recommendation: (a), in its own job.

## Amendment 1 (2026-10-29, after the registered results, before its own test is run): the live model switch

**Defect 5 (D2, found by review after the results).**
- `Simulation::reconfigure_turbulence` (F-044, the live model switch) builds the new model with
  `TurbState::new(new_case, geom, &old.inlet_speeds)`, the inlet patches of the old model.
- Before D2 the role under `auto` did not depend on the model, so that list was right. Under D2 it does:
  - **SA to SST or k-epsilon** with an `auto` free-stream far field: the far field is in the old list, so the new model
    gets a `FixedValue` k/omega (epsilon) there. That is the inlet defect of ADR 0164 amendment 1, with values from an
    unset `turbulence_length`.
  - **SST to SA**: the far field is not in the old list, so SA keeps a zero-gradient far field. A fresh SA build of the
    same case gets `freestream`.
- Fresh builds (`Simulation::new`, every run of R1-R3) do not take this path.

**Correction 5 (code).**
- `reconfigure_turbulence` keeps only the old entries whose `turb::role(new_case, patch)` is still `Inlet`.
- It adds every patch that `turb::freestream_role(new_case, patch)` makes an inlet and that is not yet listed, with the
  per-face speed `|velocity_value|`, which is what `Simulation::new` computes for a free-stream patch.
- A switch between models whose roles agree keeps the old list unchanged, so it is bit-identical to before.

**T8** (registered now, `defaults_w49c.rs`), on T5's free-stream plate:
- (a) SA for 20 iterations, then a live switch to SST and 10 more iterations: the far-field faces of `k` and `omega` are
  finite and equal to their owner cells (zero gradient), as in a fresh SST build.
- (b) SST for 20 iterations, then a live switch to SA and 10 more iterations: the inflow faces hold `nuTilda = 3 nu`.
- (c) A live switch SA to SA (a coefficient change only) gives the same inlet list as before the correction.

R1-R3 and the binary `rcfd-w49c.exe` stay valid: they are fresh builds, and the correction touches only the live switch.

## Addendum: results (2026-10-29)

Commits on the branch:
- dc4160b: this registration;
- 6f6c701: the code, the tests and the docs;
- 43891d6: the driver;
- aa9c963: this addendum;
- 3550c10: amendment 1 (registration);
- 5b70279: amendment 1's correction and T8.

Binary of the runs: `E:\projects\cfd\w49c-work\rcfd-w49c.exe`, the release build of 6f6c701 (43891d6 adds only the
Python driver), sha256 `17E3D0CA0C14CA8D30B5C5328627BE37F2B72EED4392D0F040AF7934F3C7F9F1`.

Host: shared with jobs 49D and 49E (their two solver processes ran throughout), about 21-24 GB free. At most 2 of my
solver processes at a time: R1 and R2 together, then R3 alone.

### Limits

| Id | Measured | Outcome |
|---|---|---|
| T1 | laminar plate: all spellings equal bit for bit (forces and moments) | met |
| T2 | power-law plate: `total` = `total_molecular`, `viscous` = `viscous_molecular`, moment unchanged, bit for bit; `viscous_effective` differs (control) | met |
| T3 | SA plate y+ 40: `total` = `total_effective`, `viscous` = `viscous_effective` bit for bit; ratio > 1.5; pressure part within 1e-12 | met |
| T4 | default moment = `total_molecular` moment bit for bit; `total_effective` moment differs (control) | met |
| T5 | SA far field under `auto` = explicit `freestream` bit for bit (ux, uy, p, nuTilda, flux after 30 iterations); inflow faces 3 nu; explicit `zero_gradient` differs (control) | met |
| T6 | SST and k-epsilon: no validation error, `auto` = explicit `zero_gradient` bit for bit; `turb` unit test `freestream_far_field_default_is_sa_only` passes | met |
| T7 | `force_effective_w49a` C1-C5: 4 passed (C2/C3 respelled) | met |
| T8 (amendment 1) | (a) SA to SST: k and omega on every far-field face finite and equal to the owner cell; (b) SST to SA: inflow faces 3 nu; (c) SA to SA: inflow faces 3 nu, outflow faces the owner value. Control: with the uncorrected list (`old.inlet_speeds`, put back temporarily) T8 fails at (a) | met |
| R1 | SA-a00, legacy spellings: 1099 iterations to the wall cap (exit 8); 1099 compared, 2198 cd/cl values, **0 different** from ADR 0159 | met |
| R2 | SA-a10, legacy spellings: 876 iterations (exit 8); 876 compared, **0 different** | met |
| R3 | F10 without its `turbulence` line: 577 iterations, exit 0, the same stop reason; 577 x 12 values, **0 different**. So `cd` = F10's `cd_eff` (0.0154838 at the end), `cl` = F10's `cl_eff` (1.05521), `cd_v` = F10's `cd_veff` (0.0060170) | met |
| G1 | `baseline_hashes.rs` unchanged; its test 3 passed, 1 ignored as before. `case_hashes_w48u` passes. `w48v_case_hashes` holds only an ignored print tool (ADR 0156 N3), so nothing to pass there; the case hashes are checked by `case_hashes_w48u`. `w33b_default_bits` passes. `workspace_graph` 3 passed | met |
| G2 | no file changed under `example-projects`, `example-projects-linux`, `cases` since f9a520a; `example_projects_w48h` 21 passed, 1 ignored (as before); `example_rotated_w48u` 2 passed | met |
| G3 | rcfd-api lib 106 passed, 2 ignored; `physics_schema` 11, `sst_schema` 3, `turb_models` 8, `registry_setups_w39a` 2, `rules_w19c` 5 (fenced TOML of both docs), `w48v_adaptive_correctors` 5, `sa_drag_w49a` 8 passed; ignored counts as before | met |
| G4 | `cargo fmt -p rcfd-api -- --check` clean; `cargo clippy --release -p rcfd-api --lib --test defaults_w49c --test force_effective_w49a -- -D warnings` clean | met |

What this shows:
- **The old numbers stay reproducible by name.** ADR 0159's SA cases reproduce bit for bit with
  `component = "total_molecular"` and `turbulence = "zero_gradient"`, at 0 and 10 degrees, as far as the runs went.
- **The new default equals ADR 0164's corrected set-up.** An unchanged ADR 0159 case text now gives what ADR 0164's opt-in
  F10 gave, bit for bit: cl 1.055 and cd 0.01548 at 10 degrees, against 0.982 and 0.0396 before.
- Wall time of R3: 201 s on the shared host, against F10's 124 s.

After amendment 1's correction (5b70279), these were re-run: `defaults_w49c` (6 passed), `force_effective_w49a`,
`turb_models` (8 passed; holds the F-044 live-switch tests), `baseline_hashes` (3 passed, 1 ignored), the `turb` lib
tests, clippy and fmt. All passed or were clean. The other G tests and R1-R3 were not re-run. The correction touches only
`reconfigure_turbulence`, which no fresh build and none of those tests call except `turb_models`.

### Deviations

- **Amendment 1.** The live model switch kept the old model's inlet list. That was a defect introduced by D2 and found
  by review after the registered results. It was registered as amendment 1 before its fix and its test T8.
- T8 (c) is checked by what can be seen from outside the crate (the far-field faces after the switch), not by comparing
  the inlet list itself, which is private.

- `w48v_case_hashes` was registered in G1 as "passes". Its only test is an ignored print tool. The registered intent (the
  case hashes are unchanged) is met by `case_hashes_w48u`, which compares every shipped case against the 6867e40 fixture.
- The R1-R3 comparison output (`E:\projects\cfd\w49c-work\runs\compare.json`) is recorded in the table above and is not
  committed. D7 named no results folder.

### Not run

- The full workspace test suite and the gate. The Studio tests (`example_projects_h`, `scale_n`, `saved_store_r`) were not
  run. They read the stored runs and plans, which this job does not change, and start plans do not read the changed
  code.
- Linux.
- ADR 0159's runs to their full length (R1/R2 are prefixes of 1099 and 876 iterations).
- The SST and DD arms with the legacy spelling.
- The ignored heavy FAN-01 runs (moments unchanged by D3).
- Re-runs of the 8 stored NACA 0012 example runs (question 3).

### Questions for the maintainer

Questions 1-4 above stand unchanged; no result bears on them. In short:
1. **SST and k-epsilon far fields.** Recommendation: (b), a `freestream` default with values from `initial_intensity` and
   `initial_viscosity_ratio`, in its own job.
2. **Moments.** Recommendation: (a), follow the force default, re-reading FAN-01's torque known misses.
3. **The 8 stored NACA example runs.** A re-run now gives other cl and cd; the fields are the same. Recommendation: (b),
   regenerate in one registered job together with question 1 (b), so that the examples change once.
4. **Laminar variable viscosity.** Recommendation: (a), in its own job.
