# ADR 0164: Job 49A, why the steady Spalart-Allmaras drag of the NACA 0012 is wrong (ADR 0159 question 4)

Status: **registered 2026-10-29**, before any solver run of this job (set-ups, limits, reading rules and the not-run list
are fixed here). Results will be added in an addendum. Job 49A, on local main 8d2ea58 (the worktree branch was reset to
it before any work). Number: ADR 0163 is the highest in this worktree and in the main checkout. House rules of
[ADR 0130](0130-wave48a-overview-and-run-record-contract.md):
- limits are fixed here before the registered runs; a missed limit becomes an ignored `*_known_miss` test with the limit
  intact;
- a defective set-up gets an amendment before the corrected run;
- defaults of existing code stay bit-identical; `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded;
- no new crate, no unsafe code, no commit trailers, nothing is pushed.

Clean-room: our own code and published data only. The data are the NASA Turbulence Modeling Resource (TMR) NACA 0012
validation case and its SA results (CFL3D files, fetched 2026-10-29), Ladson (NASA TM 4074, 1988) as already in the
repository, and the TMR flat-plate SA value already used by ADR 0014.

Parents:
- [ADR 0159](0159-job48y-naca0012-angle-of-attack-sweep-through-stall.md): the sweep, its mesh M1w, its driver, question 4
  (this job) and question 5 (the trailing-edge divergence, not this job).
- [ADR 0014](0014-sa-and-turbulence-interface.md): the SA model, its wall mode `auto` and the flat-plate checks S1-S6.

## The request (the maintainer, relayed)

Steady SA on M1w (256 x 128 O-grid, first cell 3.5e-4 chord, wall y+ about 40, Re 6e6) gives cd about half of Ladson's
at 0 degrees and 1.4 to 4.6 times it from 4 degrees up. Its cd also kept rising for thousands of iterations. SST did
neither of these. Find out why, using published references only:
1. the TMR set-up at alpha 0, with cd split into pressure and friction;
2. the wall treatment at y+ 40, on a flat plate at y+ 1, 5 and 40 through the same solver path;
3. the iteration drift;
4. the far field and the free-stream nuTilda;
5. report with numbers.

A clear, small defect is fixed opt-in only, documented in `docs/case-schema.md`, and a default change is asked about,
not decided. No constant is tuned.

## What I knew when writing this (rule d): code reading, ADR 0159's stored runs, the TMR files

Nothing of this job had been run. What I had read and computed:

1. **The force report uses the molecular viscosity.** `Simulation::eval_report` (`crates/rcfd-api/src/sim.rs`) passes
   `self.nu` (`physics.nu`) to `rcfd_post::integrals::wall_force_and_moment`. That function takes the wall-normal
   derivative from the velocity boundary coefficients, (u_P - 0)/dn. The momentum equation instead uses the model's face
   viscosity. On a wall face that viscosity is nu + nu_t,w (`rcfd_turb::eddy::effective_viscosity` with SA's
   `wall_nut` = u_tau^2 dn/u_P - nu in wall mode `auto`), so the shear it applies is u_tau^2.
   - The report therefore gives nu u_P/dn = u_tau^2 (u+/y+). At y+ = 40 (u+ about 14 on the log law) that is about 0.35 of
     the shear the solver applies.
   - This is model-independent: k-epsilon and SST with wall functions go through the same report. ADR 0159's SST cd at
     0 degrees was 0.0036, like SA's 0.0037.
2. **The far-field nuTilda is not fixed.** `turb::role` makes a patch with `velocity = "freestream"` (ADR 0159's far
   field) a `ZeroGradient` turbulence patch. Only a fixed non-zero velocity is an inlet. So no far-field face holds
   nuTilda at `turbulence.nutilde_ratio * nu` (3 nu, the TMR value). nuTilda has the default zero-gradient condition
   (`rcfd_fv::field::default_bcs`) there, and only its initial value 3 nu.
3. **ADR 0159's stored solutions** (`E:\projects\cfd\w48y-work\w\runs\*\output\solution.vtu`, read with a scratch script,
   no solver run). The median nuTilda/nu over the cells 5 to 40 chords upstream:
   - SA-a00: 23.8 (TMR 3);
   - SA-a10: about **6.0e4**, the same in every far-field bin up to 50 chords. That is nu_t about 0.01 U c in the outer
     flow, an "outer Reynolds number" of about 100.

   The wall-law friction drag from the stored y+ field (u_tau = y+ nu/dn, tau = u_tau^2 along the wall-cell velocity,
   integrated over the 256 wall faces) is **0.00710** at 0 degrees and 0.00726 at 10 degrees. The reported total cd was
   0.0037 and 0.0396. The wall-cell y+ median is 42 (5.5 to 61) at 0 degrees.
4. **ADR 0159's cd histories** (monitors.csv), checkpoint values:

   | Run | cd at 500 | 1000 | 2000 | 4000 | 6000 | 8000 |
   |---|---|---|---|---|---|---|
   | SA-a10 | 0.0219 | 0.0284 | 0.0349 | 0.0388 | 0.0395 | 0.03957 |
   | SA-a00 | 0.003677 | | 0.003678 | | | |
   | SST-a00 | 0.0036368 (constant to 7 digits) | | | | | |

5. **TMR NACA 0012 validation, SA** (`naca0012_val_sa.html`; the CFL3D files `n0012clcd_cfl3d_sa.dat` and
   `n0012cf_cfl3d_sa.dat`).
   - Set-up: 897 x 257 grid, far field about 500 chords, nuTilda free stream = 3 nu, Re 6e6, M 0.15, point-vortex
     correction for lifting cases.
   - CFL3D: cl 0 / 1.0909 / 1.5461 and cd 0.00819 / 0.01231 / 0.02124 at 0 / 10 / 15 degrees. Seven codes span
     0.00812-0.00830 at 0 degrees and 0.01225-0.01245 at 10 degrees.
   - The friction part at 0 degrees, 2 x integral of cf dx over the upper-surface file (by `w49a_sa_drag.py tmr`), is
     **0.006831**. That leaves 0.001361 for pressure.
   - Committed as `validation-data/reference/w49a_tmr_naca0012_sa_cfl3d_clcd.csv` and `..._cf_alpha0_upper.csv`.
6. **ADR 0014's library flat plate** (S5). SA wall mode `auto` on a wall-function mesh (y+ 31.7 at x = 0.97) gave
   Cf(0.97) = 0.002863, +5.8 % against the TMR SA value 2.70565e-3. The y+ 0.5 mesh gave +2.97 %. That Cf is the model's
   u_tau^2, not the force report.

So the two hypotheses below come from ADR 0159's data, not blind. The runs test them.

## Hypotheses (registered predictions)

- **H1 (the report).** The low cd at 0 degrees is the force report's friction. The report takes the molecular viscosity
  where the solver applies a wall function. Prediction:
  - the report's viscous part at y+ 40 is about 0.35 of the wall shear the solver applies;
  - the wall-law friction is within 10 % of TMR's 0.00683;
  - with the shear the solver applies, cd at 0 degrees is within the band of TMR's 0.00819.
- **H2 (the far field).** The high cd from 4 degrees up is the free-stream nuTilda. Nothing fixes it on a `freestream`
  far field. In a steady run it grows to a level set by SA's production on the far field's small spurious vorticity
  (the fixed free stream on a circle of 50 chords without a point-vortex correction, and coarse outer cells) against a
  destruction that vanishes far from the wall. Prediction:
  - with nuTilda fixed at 3 nu on the far field, the outer nuTilda stays near 3 nu;
  - cd at 10 degrees falls to the TMR band;
  - cl at 10 degrees rises towards 1.09.
- **H3 (the drift).** The cd drift over thousands of iterations is H2's growth of the outer nuTilda, not a residual or
  relaxation property. Prediction: with the far field fixed, cd at 10 degrees no longer drifts.

## Decisions (set-up; mine, none is a maintainer decision)

1. **D1 Opt-in report components, no new key.** `reports.<n>.component` gets two more values. The default `total` and the
   values `pressure` and `viscous` are unchanged.
   - `total_effective` and `viscous_effective` take the viscous stress with the face viscosity the momentum equation used
     last (`Simulation::momentum_face_viscosity`: the steady, coupled or transient solver's `nu_face`; molecular plus eddy
     viscosity, on wall faces the wall-function value).
   - Without a face viscosity (laminar constant viscosity) they are the default values, bit for bit.
   - Code: `rcfd_post::integrals::wall_force_and_moment_face_viscosity` (new). `wall_force_and_moment` keeps its
     signature and now calls the same body with a constant viscosity, the same arithmetic in the same order.
   - Why values, not a new key: a new default-valued key under `reports` would add a line to every case's written
     settings text and break `case_hashes_w48u` (H1 of ADR 0155) again, as `solver.corrector_control` did (ADR 0163). A
     new enumeration value adds nothing at the default.
   - Documented in `docs/case-schema.md`. Whether `total` should become the effective value is question 1.
2. **D2 The far-field nuTilda through an existing key.** Arm B sets `boundaries.farfield.turbulence = "inlet"`. That key
   already exists (`turb::role`): nuTilda is fixed at `turbulence.nutilde_ratio * nu` = 3 nu on every far-field face.
   - Difference from the TMR far field: that is fixed on inflow, extrapolated on outflow. Here it is fixed on all faces,
     so also where the wake leaves 50 chords downstream. With upwind convection this affects only the boundary cells'
     diffusion.
   - No code is written for this. A `freestream` turbulence role (fixed on inflow, zero gradient on outflow, the
     `rcfd_fv::bc::Freestream` condition) is question 2. Making it the default for `freestream` patches would change
     default results, so it is not done here.
3. **D3 Arms** (driver `scripts/w49a_sa_drag.py`, work folder `E:\projects\cfd\w49a-work\w` outside the repository).
   - **A00, A10.** ADR 0159's SA case text at 0 and 10 degrees on M1w (`w48y_aoa_sweep.case_text`, unchanged), plus these
     reports:
     - `cd_p` (pressure) and `cd_v` (viscous);
     - `cd_eff` (total_effective), `cd_veff` (viscous_effective) and `cl_eff` (total_effective);
     - `nut_avg`, the volume average of `nut`. `nuTilda` is not a report field, so `nut` stands in, dominated by the
       outer cells.

     Settings: SIMPLEC, linear upwind, SA defaults (SA-noft2, SA-neg, wall mode `auto`), `max_iterations` =
     `min_iterations` = 8000, one thread.
   - **B00, B10.** A00 and A10 with D2's far-field line.
   - **C10 (report only).** B10 on **F500w**: M1w's grid with a far field of 500 chords. Grading 126272.29, so that the
     first cell at mid-chord is 3.4977e-4 chord against M1w's 3.4944e-4. `rcfd check-mesh`: maximum non-orthogonality
     73.6 degrees, aspect ratio 657, volume ratio 6.4, no error. Points and topology sha256
     `59e707a866e34ec2fef6dd13a41de6411f844ceff99a2b6a644cb45f3a70c889`. M1w's sha256 is
     `bd8ef3c6da2dd3681d38c18914c8fbdcddab77bb1381c0688e1e2dce5f0c0299`, rebuilt here equal to ADR 0159's.
   - **S00 (report only).** ADR 0159's SST case at 0 degrees (4000 iterations) with the same extra force reports. It
     shows whether SST's 0.0036 is the same report effect.
   - **P01, P05, P40.** An SA flat plate through `rcfd run`, the aerofoil's solver path (SIMPLEC, linear upwind, SA
     defaults, wall mode `auto`).
     - Flow: Re 5e6 per m (U = 1, nu = 2e-7), the TMR 2DZP plate x = 0..2, height 1.
     - Boundaries: inlet at the leading edge with nuTilda = 3 nu, symmetry on top, fixed-pressure outlet. There is no
       symmetry run-up, as in ADR 0014.
     - Mesh: `box_2d`, 200 uniform cells in x, `wall_layers` 50 / 40 / 25 with growth 1.15, ny 70 / 60 / 45,
       `target_y_plus` 1 / 5 / 40 with `layer_reference_length` 0.97 (the station).
     - 15000 iterations (`min_iterations` = the cap), reports `cf_v` (viscous) and `cf_veff` (viscous_effective).
4. **D4 Reading.** Steady values are means over the last 1000 iterations (ADR 0159 D4).
   - Friction from the reports: `cd_v` (molecular) and `cd_veff` (effective). Pressure: `cd_p`.
   - Plate local Cf at x = 0.97, from the final `output/solution.vtu`, interpolated linearly between wall-cell centres:
     - wall law: 2 u_tau^2 with u_tau = y+ nu / y_P, where y+ is the model's wall-cell field and y_P the wall-cell centre
       height;
     - molecular: 2 nu u_P / y_P.
   - Plate mean C_F over 0..2: from the report (`cf_veff`/(2 depth)), and as the integral of the wall-law local Cf.
   - Outer nuTilda: the median nuTilda/nu over cells 5 to 40 chords from mid-chord with x < -0.5 and |y| > 2 (upstream),
     from the final `solution.vtu`.
   - Drift: (mean cd over iterations 7001-8000 minus mean over 1001-2000) / mean over 7001-8000.
   - Bit comparison: every iteration's `cd` and `cl` of A00 and A10 (and S00) against ADR 0159's `monitors.csv` of the
     same angle (17 significant digits as written), all iterations present in both.
5. **D5 Placement.** New: this ADR; `scripts/w49a_sa_drag.py`; `docs/investigations/job49a-sa-drag/` (README,
   `results.json`, `results.csv`, `cd_histories.png`); the two TMR reference files; `crates/rcfd-api/tests/
   force_effective_w49a.rs` (C1-C3) and `crates/rcfd-api/tests/sa_drag_w49a.rs` (the limits, read from the committed
   `results.json`).
   - Changed: `crates/rcfd-post/src/integrals.rs` (new function, the old one delegates, plus a unit test);
     `crates/rcfd-api/src/sim.rs` (`momentum_face_viscosity`, the two components); `crates/rcfd-api/src/schema.rs` (two
     enumeration values); `docs/case-schema.md`.
   - Budget **B1**: the investigation folder at most 300 000 bytes, each PNG at most 120 000 bytes, the two reference
     files at most 30 000 bytes together.

## Registered limits (fixed here, before any solver run of this job)

Set-up: release build of this branch (`cargo build --release --locked -p rcfd-cli`, own `CARGO_TARGET_DIR`
`E:\projects\cfd\w49a-work\target`). The binary is copied out as `E:\projects\cfd\w49a-work\rcfd-w49a.exe` (sha256
`063E05249F5A82B560A07137B35FA3224B93E9CAE2570207C905A7F30CABC21D`, the build of the D1 code as registered; if that
code changes before the runs, an amendment names the new build). Runs go through the driver's `run`, at most 3 at a time,
with free memory checked before each batch.

| Id | Check | Limit |
|---|---|---|
| L1 | Runs complete | every run of A00, A10, B00, B10, P01, P05, P40 ends with exit code 0 or 5 at its cap |
| LR | Defaults bit-identical | A00 and A10: every `cd` and `cl` value equals ADR 0159's SA-a00 / SA-a10 `monitors.csv` (zero differences) |
| C1 | New components, laminar | laminar plate: `viscous_effective` and `total_effective` equal `viscous` and `total` bit for bit (`force_effective_w49a.rs`) |
| C2 | New components, SA y+ 40 | coarse SA plate at y+ 40, 400 iterations: `viscous_effective` / `viscous` > 1.5 (same test file) |
| C3 | Pressure part unchanged | same run: `total_effective - viscous_effective` equals `total - viscous` within 1e-12 relative |
| LP1 | Wall law on the plate | wall-law Cf(0.97) against TMR SA 2.70565e-3: P01 within **5 %**, P05 and P40 within **10 %** |
| LP2 | Report correct when resolved | P01: molecular / wall-law Cf(0.97) within **1 +- 0.03** |
| LP3 | H1 on the plate | P40: molecular / wall-law Cf(0.97) **<= 0.6** |
| LP4 | New component = wall law | P01 and P40: report C_F (`viscous_effective`) against the integral of the wall-law local Cf, within **2 %** |
| LA1 | H1 on the aerofoil | A00: `cd_v` / `cd_veff` **<= 0.6** |
| LA2 | Friction at alpha 0 | A00 and B00: `cd_veff` within **10 %** of TMR's friction part 0.006831 |
| LA3 | cd at alpha 0 | A00 and B00: `cd_eff` within **12 %** of TMR's 0.00819 (0.00721-0.00917) |
| LA4 | alpha 10 with the far field fixed | B10: `cd_eff` within **25 %** of 0.01231 **and** cl within **5 %** of 1.0909 |
| LA5 | H2's mechanism | outer median nuTilda/nu: A10 **>= 1000**; B00 and B10 **<= 5** |
| LA6 | H3, the drift | B10: abs(drift of `cd`) **<= 2 %** |
| B1 | Storage | investigation folder <= 300 000 bytes, PNG <= 120 000 each, the reference files <= 30 000 together |
| P1 | Driver | `python scripts/w49a_sa_drag.py selftest` passes |
| G0 | Hygiene | `cargo fmt -p` clean for rcfd-post and rcfd-api; clippy `-D warnings` of the two crates' changed targets clean; `git diff 8d2ea58 -- crates/rcfd-api/tests/baseline_hashes.rs` empty; `rcfd-fv` `w33b_default_bits` passes; `rcfd-cli` `workspace_graph` passes |

Why these bands:
- LA3's 12 % covers the y+ 40 wall-law error seen on ADR 0014's plate (+5.8 %), a first-order turbulence convection, the
  incompressible solver at the M 0.15 condition and a 50-chord far field. The far field matters little at zero lift
  (TMR's far-field page).
- LA4's cd band of 25 % is wide on purpose. TMR's far-field page says drag at high lift is sensitive to the far-field
  extent without the point-vortex correction. A rough estimate of the induced-angle error cl c/(4 pi R) at R = 50 c is
  about 0.1 degree, so cl x delta alpha is about 0.002 in cd, 15 % of the value.

**Reported, no limit** (nothing is tuned to them):
- C10 (500 chords) against B10 and TMR;
- S00's split and its effective cd;
- A10's split, drift and effective values;
- cd_p against TMR's pressure part 0.001361;
- the plate's mean C_F against Kármán-Schoenherr's mean-drag law 0.242/sqrt(C_F) = log10(Re_L C_F) at Re_L 1e7;
- y+ at x = 0.97 of the plates;
- wall times.

**Reading rules.**
- A limit is evaluated once, on the registered runs, by the driver's `collect` as committed.
- A limit whose run did not complete is reported as not evaluated, not as met.
- Arm C10 and S00 run last and may be dropped for time. They are then listed as not run.

## Honest differences from the TMR set-up (stated before any result)

- Incompressible against M 0.15 (TMR calls it "essentially incompressible"; the small compressibility effect is not
  estimated).
- A 256 x 128 O-grid with a wall cell at y+ about 40 and SA's Spalding wall law. TMR uses a 897 x 257 C-grid resolved
  to y+ below 1.
- A far field of 50 chords without the point-vortex correction. TMR uses 500 chords with the correction for lift.
- In arm B, nuTilda is fixed on the whole far field, not only on inflow (D2).
- The TMR aerofoil is the NACA 0012 scaled to close at chord 1 (11.894 % thick). This repository's `naca_thickness`
  (ADR 0092) closes the trailing edge its own way; the thickness difference is not quantified here.
- First-order upwind convection of nuTilda (SA's default here) against the TMR codes' second-order schemes.

## Out of scope

- The trailing-edge divergence at fine wall spacing (ADR 0159 question 5): no mesh or solver change for it. Anything seen
  is recorded.
- A C-grid; the point-vortex far field; a `freestream` turbulence role (question 2).
- Any change of SA coefficients or wall-law constants.
- SST or k-epsilon wall treatment beyond S00's report.
- Re-running ADR 0159's sweep with the fixes (question 3).

## Questions for the maintainer (registered now; answers are not assumed)

1. **Default of the force report with wall functions** (D1).
   - (a) Keep `total` molecular, and document that `total_effective` is the one to use with wall functions.
   - (b) Make `total` and `viscous` use the momentum face viscosity. That changes the default monitors of every RANS run
     with wall cells above y+ of about 1.

   Recommendation: decided after the results (in the addendum), not here.
2. **Far-field turbulence of `velocity = "freestream"` patches** (D2).
   - (a) A new opt-in role `turbulence = "freestream"` (fixed on inflow, zero gradient on outflow).
   - (b) Make that the default for `freestream` patches (changes default results).
   - (c) Document the `turbulence = "inlet"` workaround only.

   Recommendation in the addendum.
3. **ADR 0159's sweep.** Whether to re-run its SA arm with the fixes (a new job).
