# ADR 0170: Job 49G, moment reports follow the effective wall-shear default (ADR 0166 question 2 (a))

Status: **registered 2026-10-29**, before any code and before any solver run of this job. The enumeration, the design,
the limits, the reading rules and the not-run list are fixed here. Not yet accepted by the maintainer. Job 49G, on local
main 1a04d46 (the merge of job 49C; the worktree branch was reset to it before any work). Number: 0167 (49D), 0168 (49E)
and 0169 (49F) are committed on other branches; 0170 was free in every worktree and in `git log --all` when this was
written.

House rules of [ADR 0130](0130-wave48a-overview-and-run-record-contract.md):
- limits are fixed here before the registered runs; a missed limit becomes an ignored `*_known_miss` test with the limit
  intact; a known miss that now meets its limit is reported and re-registered in an amendment before any change;
- a defective set-up gets an amendment before the corrected run;
- defaults of existing code stay bit-identical except where the maintainer's decision below changes them, and every such
  change is listed here before the code;
- `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded; if any of its cases would change, the job stops;
- no new crate, no unsafe code, the crate graph stays acyclic, no commit trailers, nothing is pushed.

Parents:
- [ADR 0166](0166-job49c-effective-wall-shear-and-sa-far-field-defaults.md): D1 (the force default follows the turbulence
  model), D3 (moments kept molecular), D4 (`total_molecular` / `viscous_molecular`), question 2 and test T4.
- [ADR 0164](0164-job49a-why-the-sa-drag-of-the-naca0012-is-wrong.md): `wall_force_and_moment_face_viscosity` and the
  `*_effective` values.
- The FAN-01 records: [ADR 0075](0075-wave34d-rotating-machinery-data-and-mesh-study.md) (F-126 attempt 3),
  [ADR 0078](0078-wave35c-fan01-deficit-diagnosis.md), [ADR 0082](0082-wave36c-rotating-frame-angular-momentum-source.md),
  [ADR 0086](0086-wave37c-fan01-solution-verification.md), [ADR 0110](0110-wave43c-fan01-rotor-passage-non-convergence-diagnosis.md),
  [ADR 0111](0111-wave44a-fan01-frozen-limiter-linearisation-and-relaxed-deferred-correction.md),
  [ADR 0115](0115-wave45a-fan01-range-scaled-venkatakrishnan-limiter.md),
  [ADR 0117](0117-wave45c-cyclic-partner-coefficient-in-h1-on-the-fan01-path.md), and the plan row of
  [ADR 0136](0136-fidelity-tiers-xhigh-to-xlow.md).

## The maintainer's decision (relayed)

On ADR 0166 question 2 ("(a) make the default `moment` use the effective wall shear under a turbulence model, like force
reports; FAN-01's torque monitors move ... (b) keep"; recommendation (a), with FAN-01's torque known misses re-read in that
job): **"(2) separate job"**. Read as: do (a), as its own job. The brief adds:
- laminar stays bit-identical; the old computation stays spellable (by the existing `*_molecular` values, or new `moment`
  spellings if those do not cover it); no case-hash change: enumeration values, not new keys;
- re-read every FAN-01 torque limit and known miss honestly, with numbers; do not loosen a limit;
- run only what the registered limits require and say what was not run;
- do not regenerate stored example runs and do not touch `example-projects*` (job 49F); do not touch the SST / k-epsilon
  far-field roles (job 49F).

## What I read before writing this (rule d): code, tests and records, no solver run

1. **The report** (`Simulation::eval_report`, `crates/rcfd-api/src/sim.rs`). One closure `wall(origin)` serves `force`,
   `force_coefficient` and `moment`. Since ADR 0166 the switch is:
   - `total_effective`, `viscous_effective`: the momentum face viscosity (`momentum_face_viscosity`, when it exists);
   - `total`, `viscous`: the face viscosity when `self.turb` is set **and** the kind is `force` or `force_coefficient`;
   - everything else (`total_molecular`, `viscous_molecular`, `pressure`, and `total` / `viscous` of a `moment`): `physics.nu`.
   A `moment` report does not apply `component` otherwise: it always returns the moment of the whole wall force
   (pressure plus viscous). So `component = "viscous"` or `"pressure"` on a moment gives the whole moment.
2. **What 49C provided.** ADR 0166 D4 made `total_molecular` and `viscous_molecular` valid for every report kind, and T4
   (`defaults_w49c.rs`) already evaluates a `moment` with `component = "total_molecular"`. So the old moment is already
   spellable; no new value is needed.
3. **Reports do not feed back into the solution** unless a case uses them in a `convergence.<name>` condition or an
   expression (`exprs.rs` gives reports a dimension, so expressions can read them). Searched: no committed case
   (`cases/**`, `example-projects*/**`, registry `case.json`, `scripts/*.py`, `python/`, `clients/`) has a `moment` report
   at all, and no Rust test case with a moment report has a convergence condition or an expression reading it. The
   six-degree-of-freedom body takes its load from `rcfd_fv::sixdof::patch_force_moment`, not from a report. **So every
   field, flux, iteration count and residual stop of every run below is unchanged, bit for bit; only the moment monitors
   move, and with them any harness verdict computed from them.** That is checked, not assumed (K1-K3, F1).
4. **Other torque code.** `rcfd_post::turbo::torque_on_fluid` takes an explicit `nu` and has no product caller (only its
   own unit test). The adjoint drag (`design/adjoint.rs`) is laminar `walls` drag with `settings.nu`. The Studio, MCP,
   GUI, CLI and assistant crates do not display or name moment reports or report components (searched for `moment`,
   `torque`, `force_coefficient`, `total_effective`, `component`). Their only exposure is the schema doc string.

## Enumeration: what the change moves (registered before any code)

Every Rust test with a `moment` report (searched: `type = "moment"`, the escaped `type = \"moment\"`, `json!("moment")`,
`ReportKind::Moment`):

| File | Turbulence | What reads the moment | Moves? |
|---|---|---|---|
| `fan01_validation.rs` | SST | `f126_..._known_miss` (ignored): torque limit, eta limit, cap-rule `converged` (torque change); `s1b` only builds | yes (the ignored F-126 test) |
| `fan01_gci_w37c.rs` | SST | `b_m_b_p_bladed_studies_are_valid` (ignored, met in ADR 0086): bladed f_a = torque, its cap rule and the ITERATION ERROR check; `registered_run`, `evaluate` (tools) | yes (B-M, and B-P through the cap rule, which needs *both* functionals settled) |
| | | `e_l_e_s_..._known_miss` (ignored): empty passage, f_a = outlet L, f_b = swirl energy, both from fields | no |
| | | `s1_family_builds_and_is_accepted`, `gci_recovers_a_known_power_law`: no solve / no moments | no |
| `fan01_diagnosis_w35c.rs` | SST (arms) | 22 ignored `registered` arms: torque **printed only** (no assert), incl. the printed E0 reproduction rule (1.320 N m within 0.1 %) and the cap-rule `converged` print | printed numbers only |
| | | `s2a`, `s2b`, `arms_construct_and_count_rules`, `arm_e_blade_element_estimate`: no moment value read | no |
| `fan01_rotor_frame_w36c.rs` | SST | ignored arms print torque and the report-based L excess; the ADR 0082 budget (`E_s`, `G_visc`) uses the solver's `nu_face`, not reports | printed numbers only |
| | | `budget_identity_and_reconstruction_on_a_short_run`: budget only | no |
| `fan01_stall_w43c.rs` | SST (laminar arms S4, L3) | bladed arms print torque (`functionals`); every verdict is on residual medians and `|du|`; Z1 / F-a known misses are on outlet L and swirl energy (fields) | printed numbers only |
| | | `s1_e3_...`, `frozen_limiter_hook_...`, `w44a_hooks_...`, `w45c_da_route_...`, `w45a_p5_...` (field hashes) | no |
| `defaults_w49c.rs` | laminar T1/T2; SA T3/T4 | T4: "default moment = `total_molecular`, control `total_effective` differs" | **T4 fails as written** (superseded by T4', D3) |
| `mrf.rs`, `wall_motion.rs`, `cylindrical_interface_w43b.rs` | laminar (no `[turbulence]`) | Taylor-Couette torque limits and known misses | no (laminar) |
| `mesh_transform_w48u.rs` | laminar | the moment is a rules (MT101) input, never evaluated | no |

Not changed:
- every laminar case, including variable viscosity (`rheology`, user-function viscosity, `properties`): the gate stays
  `self.turb.is_some()` (ADR 0166 D1);
- force and force-coefficient reports (already effective since ADR 0166);
- `y_plus`, `wall_shear` output, `torque_on_fluid`, the adjoint drag, the six-DOF load;
- every committed case, example and stored run: none has a moment report, so no example output and no MCP / Studio
  display changes, and `example-projects*` is not touched.

Records and gates:
- **`baseline_hashes.rs`: no case changes** (predicted, checked in G1). Its cases hash fields after 60 iterations; the only
  turbulent one (`kepsilon_channel.toml`) has `y_plus` reports only; none has a moment report.
- `case_hashes_w48u`: no settings text changes (no key, no default value changes). `rcfd-fv` `w33b_default_bits`:
  `rcfd-fv` is not touched.
- Validation bands with turbulent moments: only the FAN-01 tests above.
- ADR records with molecular torques that are **not re-run** here (reported, no limit, or diagnosis readings): ADR 0078's
  22 arms (incl. its torque-gap shares and "KN 5.9 % below the measured torque"), ADR 0082's arms, ADR 0110 / 0111 /
  0115 / 0117's printed B3 torques (1.849 N m etc.), ADR 0136's FAN-01 tier row, and `docs/milestone-fan.md`'s FAN-01
  torque summary. They stay records of the molecular moment of the version that made them (question 3).

## Decisions (mine, within the maintainer's decision; none is presented as the maintainer's)

1. **D1 The moment default follows the force default.** In `eval_report`, `total` and `viscous` take the face viscosity
   when a turbulence model is active for **every** kind that uses the wall closure (`force`, `force_coefficient`,
   `moment`). That is the only code change. Without a turbulence model nothing changes.
2. **D2 `pressure` on a moment keeps its present value** (the whole moment with `physics.nu`). A moment does not apply
   `component`; making it do so would also move laminar records (`cylindrical_interface_w43b`'s `component = "viscous"`
   torque, ADR 0109 K2). Not decided here: question 1.
3. **D3 T4 of ADR 0166 is superseded** by T4' (below), the way ADR 0166 D4 respelled ADR 0164 C2/C3. ADR 0166 D3 is
   superseded by D1 here; ADR 0166 gets a one-line status pointer.
4. **D4 Spellings.** No new value: `total_molecular` (or `viscous_molecular`, the same whole moment) gives the pre-change
   moment for every model. The doc string of `reports.<n>.component` and `docs/case-schema.md` say so.
5. **D5 The FAN-01 harnesses keep their case texts and get report-only companions.** `fan01_validation.rs` and
   `fan01_gci_w37c.rs` (bladed) get `m_blade_mol` and `m_hub_mol` (and in the GCI harness `m_shroud_mol`), moment reports
   with `component = "total_molecular"`, and print the molecular torque, its last-window change and, for the GCI, its
   values for `evaluate`. These are report lines only (item 3: no field changes). They give the old and the new torque
   from the same run, and the old one is the set-up check against the ADR 0075 / 0086 records. No harness rule, limit,
   cap, tolerance or stop changes. The registered f_a stays `-9 (m_blade + m_hub)` with the default component, which is
   now effective.
6. **D6 Records after the runs.** The ignore strings and doc comments of tests whose numbers change get the new numbers
   beside the old ones; limits unchanged. A met limit that misses is renamed `*_known_miss` (limit intact). ADR 0075 and
   ADR 0086 get one dated pointer line each.
7. **D7 Placement.** Changed: `crates/rcfd-api/src/sim.rs` (the gate and its comment), `crates/rcfd-api/src/schema.rs`
   (the `component` doc string), `crates/rcfd-api/tests/defaults_w49c.rs` (T4', header), `fan01_validation.rs`,
   `fan01_gci_w37c.rs` (D5, D6), `docs/case-schema.md`, ADR 0166 / 0075 / 0086 (pointer lines). New:
   `crates/rcfd-api/tests/moments_w49g.rs` (M1-M3), this ADR.

## Registered limits (fixed here, before any code and any run)

Tests (`cargo test -p rcfd-api`, debug or release):

| Id | Check | Limit |
|---|---|---|
| M1 | SA plate of ADR 0164 / 0166 (y+ about 40, 400 iterations), moment about z on `ymin` | `component` unset, `total` and `viscous` equal `total_effective` bit for bit; `pressure` equals `total_molecular` bit for bit (D2); control: `total_molecular` differs from the default |
| M2 | the same plate with `k_omega_sst` (the FAN-01 model) and `boundaries.xmin.turbulence_length = 0.01` (the inlet specification SST requires), 400 iterations | default and `total` equal `total_effective` bit for bit; control: `total_molecular` differs |
| M3 | laminar, constant viscosity and the T2 power law (40 iterations) | default, `total`, `viscous`, `pressure` equal `total_molecular` bit for bit (power law included: the gate is the model, not the face viscosity) |
| T4' | ADR 0166 T4 respelled (`t3_t4_...` in `defaults_w49c.rs`) | the default moment equals `m_total_effective` bit for bit; control: `m_total_molecular` differs. T1, T2, T3, T5, T6, T8 unchanged and pass |
| F1 | non-ignored FAN-01 tests: `fan01_validation` (h1, h2, h3, attempt 1, attempt 2, s1b), `fan01_diagnosis_w35c` (s2a, s2b, arms_construct, arm_e), `fan01_gci_w37c` (G0, S1), `fan01_rotor_frame_w36c` (budget identity), `fan01_stall_w43c` (s1_e3, frozen hook, w44a hooks, w45c D-a, w45a P5 hashes), `fan01_mesh_w35d` P9, `fan01_p10_w36d` C2 | pass |
| L1 | laminar moment tests: `mrf`, `wall_motion`, `cylindrical_interface_w43b`, `mesh_transform_w48u` | pass, ignored counts as before |

Runs (release build of this branch, own `CARGO_TARGET_DIR` `E:\projects\cfd\w49g-work\target`; the test executables
are launched directly, as in ADR 0086; logs in `E:\projects\cfd\w49g-work\logs`). At most 2 of my solver processes at a
time, free memory checked before each start.

| Id | Run | Limits (the registered ones, unchanged) | Set-up check |
|---|---|---|---|
| R1 | F-126 attempt 3, `f126_fan01_rotor_against_the_measured_characteristic_known_miss` (design and off-design, one process) | ADR 0075: per point CONVERGED, `|Delta p_ts - meas| <= 3 + 0.03 meas`, `|torque / meas - 1| <= 0.05`, `|eta - eta_meas| <= 0.03` | K1: the molecular companion torque, the pressure part, Delta p_ts and Delta p_tt equal ADR 0075's printed values (1.320 / 1.010 N m, 1.229 / 0.906, -3.8 / -84.4 Pa, 112.5 / 43.5 Pa) at their printed digits |
| R2 | ADR 0086 part B: `registered_run` with `W37C_RUN` = B3, B2, B1; then `evaluate` with the printed values | ADR 0086 B-M and B-P: each study VALID (cap rule, ITERATION ERROR, NOT MONOTONE, NOT ASYMPTOTIC as registered there) | K2: 8000 iterations each; Delta p_ts and the molecular companion torque equal ADR 0086's (46.7776 / 64.2329 / 73.9881 Pa; 1.849759 / 1.967401 / 2.038428 N m) at their printed digits; `evaluate` on the molecular values reproduces ADR 0086's torque row (p 1.511, VALID) |
| R3 | ADR 0082 X2a-36, `w36c_x2a_budget` (about 1 min) | none (reported) | K3: `E_s` equals ADR 0082's 2.30241 N m at its printed digits |

Reading rules:
- **The F-126 verdict is fixed without the run:** Delta p_ts comes from pressure reports, which do not move (item 3), and
  misses by about 130 Pa at the design point. R1 is run for the numbers of the torque and eta sub-limits and the cap-rule
  flag, each reported met or missed. If a torque sub-limit is now met, that is reported and re-registered in an amendment
  before any change to the test; the test stays a known miss on Delta p_ts.
- B-M or B-P not VALID: the test becomes `b_m_b_p_bladed_studies_are_valid_known_miss` with the limit intact. Both VALID:
  it stays as is, its doc comment gets the new numbers.
- **The 60-minute wall clock of ADR 0086 stays.** ADR 0086's B1 took 58.8 of 60 minutes, and this host is shared with
  jobs 49D, 49E and 49F. Rule fixed now: B1 prints its elapsed time at iteration 500; if `16 x t(500)` exceeds 57
  minutes, I stop B1 (my process) and restart it once with no other of my runs going. A B run that still stops at the
  wall clock makes B-M and B-P **not evaluated** for this job (a resource limit, not a result), and the counterfactual
  with its values at the stop is reported as such.
- A failed set-up check (K1-K3) means a defective set-up: the run's comparison is not evaluated and an amendment comes
  before any corrected run.
- Each run is made once (apart from the B1 restart rule).

Prediction, written before any run (not a limit): the viscous part of the torque moves; the pressure part does not.
ADR 0075 put the H-slit pressure part at 1.229 of 1.320 N m, so the F-126 torque cannot reach the measured 2.142 N m
unless the effective viscous part is about 10 times the molecular one: the torque sub-limits stay missed. On the GCI
family the first cell and the wall stacks are not refined, so the shift should be similar on all three members, eps21 and
eps32 change little, and B-M stays VALID with a band moved by about that shift. ADR 0086's bladed angular-momentum excess
closed with the molecular torque (-0.0008 N m on F1); if the solver's wall flux is the effective one, that closure was
partly a cancellation, and the excess with the effective torque will open by the shift. No number is predicted.

Hygiene:

| Id | Check | Limit |
|---|---|---|
| G1 | records | `git diff 1a04d46 -- crates/rcfd-api/tests/baseline_hashes.rs` empty and the test passes (3 passed, 1 ignored); `case_hashes_w48u` passes; `rcfd-fv` `w33b_default_bits` passes; `rcfd-cli` `workspace_graph` passes |
| G2 | example trees | `git diff 1a04d46 --stat -- example-projects example-projects-linux cases` empty |
| G3 | neighbours | `rcfd-api` lib tests, `force_effective_w49a`, `physics_schema`, `rules_w19c` (fenced TOML of the docs) pass |
| G4 | style | `cargo fmt -p rcfd-api -- --check` clean; clippy `-D warnings` on the rcfd-api lib and the changed test files clean |

## Not in scope

- Moment `component` parts (question 1); `torque_on_fluid` (question 2).
- Re-running the record-only FAN-01 arms (ADR 0078's 22 arms, ADR 0082's studies, ADR 0110 / 0111 / 0115 / 0117's arms):
  their verdicts do not read the moment; their printed torques stay records (question 3).
- The empty-passage part E of ADR 0086 (its functionals do not read the moment).
- Laminar variable viscosity (ADR 0166 question 4); SST / k-epsilon far fields and the NACA example runs (job 49F).

## Questions for the maintainer (registered now; answers are not assumed)

1. **Moment parts.** A moment report ignores `component` except for the viscosity switch: `viscous` and `pressure` give
   the whole moment. (a) Make `component` select the part, like force reports; that moves laminar records such as
   `cylindrical_interface_w43b`'s `viscous` torque. (b) Keep, and document it. Recommendation: (a), in its own job;
   documented now.
2. **`rcfd_post::turbo::torque_on_fluid`** takes the molecular `nu` and has no product caller. (a) Give it a face-viscosity
   variant like `wall_force_and_moment_face_viscosity`. (b) Leave it. Recommendation: (b) until a product path uses it.
3. **Records with molecular torques that are not re-run** (ADR 0078, 0082, 0110-0117, 0136's tier row,
   `docs/milestone-fan.md`). (a) One dated line in each saying its torques are molecular moments and a re-run needs
   `component = "total_molecular"` to reproduce them. (b) One line in `docs/milestone-fan.md` only. Recommendation: (b),
   plus the pointer lines in ADR 0075 / 0086 that this job adds with its results.
