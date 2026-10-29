# ADR 0164: Job 49A, why the steady Spalart-Allmaras drag of the NACA 0012 is wrong (ADR 0159 question 4)

Status: **registered 2026-10-29**, before any solver run of this job (set-ups, limits, reading rules and the not-run list
are fixed here). **Amendment 1** (after the first runs, before any corrected run): arm B's far field is defective and is
replaced by arm F with a new opt-in turbulence role; runs end at the residual targets before the cap, so the window is
amended. **Results in the addendum:** both causes found and confirmed (the force report's molecular wall shear; the
unanchored far-field nuTilda); limits met except LA4 (cd at 10 degrees +25.95 %, band 25 %), LA6 (a reading-rule
artefact) and LP1 at y+ 5 (+19.9 %), which are known misses. Not yet accepted by the maintainer. Job 49A, on local main 8d2ea58 (the worktree branch was reset to
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

## Amendment 1 (2026-10-29, after the first runs and before any corrected run): arm B is defective; `min_iterations` does not hold a run to the cap

**What had run** (binary `rcfd-w49a.exe` as registered; work folder `E:\projects\cfd\w49a-work\w`):
- B10 and B00 had finished.
- P40 had finished.
- A10, A00 and P01 were running.

**Defect 3 (D2, arm B): `turbulence = "inlet"` on a far field traps nuTilda.**
- B10 settled within 500 iterations: cd (`total_effective`) 0.015484, cl 1.0550, the volume average of nut 2.4e-6.
- Then the volume average of nut grew by about a factor of 2 every 100 to 250 iterations: 5.1e-6 at 750, 1.7e-5 at 1000,
  7.5e-5 at 1250, 4.2e-3 at 1500.
- By about 1700 it was at the solver's eddy-viscosity cap of 1e5 nu (0.016667) in the whole domain, with cd 0.18. B00
  went the same way (cd 0.073 at the end).
- The cause is in the set-up, not the model. `rcfd_fv::convection::convection` treats a non-coupled boundary face as
  `diag += flux * a`, `rhs -= flux * b`. On a `FixedValue` face (a = 0, b = 3 nu) the flux leaving the domain carries the
  fixed 3 nu instead of the cell value. So the nuTilda carried towards the 50-chord outflow faces cannot leave, and it
  accumulates there. Its eddy diffusivity (nu + nuTilda)/sigma then spreads it through the domain.
- D2's statement that fixing the outflow faces "affects only the boundary cells' diffusion" was wrong.

**Correction 3 (code, opt-in, default unchanged).** A new value of `boundaries.<patch>.turbulence`: `freestream`.
- It is allowed only on a patch with `velocity = "freestream"`; otherwise a located error at that key.
- It gives the patch's turbulence scalars `rcfd_fv::bc::Freestream { u_ref: velocity_value, value: the inlet value,
  fixed_on_inflow: true }`: the inlet value (for SA `nutilde_ratio * nu` or `nutilde_value`) on faces where
  `velocity_value` enters, zero gradient where it leaves. This is the TMR far-field practice for the turbulence variable.
- It reuses the inlet path: `turb::role` maps it to `Role::Inlet`, and `TurbState::new` builds the `Freestream` condition
  instead of `FixedValue`. Every other value builds exactly what it built before.
- Tests C4 (the inflow faces hold 3 nu, the outflow faces the owner value) and C5 (the located error) are in
  `force_effective_w49a.rs`. Documented in `docs/case-schema.md`, with a warning not to use `inlet` on such a far field.
- Binary for every run from here on: `E:\projects\cfd\w49a-work\rcfd-w49a-am1.exe`, sha256
  `D09F0FBC38CFAF03DC7AB8BAF6618B093F91BB61C0223CE7ADAF170F8B561927`. Its default paths are those of the registered build;
  the only code added is the new role and its check.
- **New arms F00 and F10**: A00 and A10 with `boundaries.farfield.turbulence = "freestream"`.
- **LA2-LA6**, registered on B00/B10, are evaluated on F00/F10 with unchanged limits. B00 and B10 are reported as the
  record of the defective set-up.
- **C10** (report only, not yet run) is redefined as F10 on F500w.

**Defect 4 (D3, D4, L1): `solver.min_iterations` does not hold a steady run to the cap.**
- `Simulation::step` (`sim.rs`) checks `summary.converged` (the residual targets) before the `min_iterations` gate. That
  gate applies only to the monitor convergence conditions, as its schema text says ("Convergence conditions are only
  evaluated from this iteration on"). So a run stops when `res_u` and `res_p` fall below 1e-6.
- Seen here: P40 at iteration 203, B10 at 5842, B00 at 7448.
- The same holds for ADR 0159. Its SA-a00 stopped at 5157 (`monitors.csv` has 5157 rows; finish reason "residual
  targets met"), although its addendum says the runs at 0 and 17 degrees met the targets "at iteration 8000". That is a
  record error of ADR 0159's addendum (question 4 below); its window was still the last 1000 iterations of a run that
  had settled.

**Correction 4 (reading, no code).**
- L1: exit code 0 (targets met, at any iteration) or 5 (the cap).
- D4's window is the last min(1000, floor(n/2)) iterations of a run of n iterations.
- LA6's early window is iterations floor(n/8)+1 to floor(n/4). For n = 8000 that is the registered 1001-2000.
- The plates' 15000-iteration cap stays; they stop at their targets.

**Unchanged**: every limit value and band, the meshes, the cases of A, P, S, the tests C1-C3.

**Seen before writing this, disclosed.** P40's reports: `cf_veff` / `cf_v` = 2.918, i.e. the molecular part is 0.343 of
the effective one (LP3's prediction, before LP3 is evaluated on the wall data). A00 and A10 interim values from the
running status files: A00 `cd_eff` 0.00848 at 700 iterations; A10 nut volume average about 0.0099 (6e4 nu) from about
5000 iterations on.

**Process.** I stopped my own driver process (the Python scheduler, not a solver) after this was found, so that it would
not start C10 with the defective set-up. The three solver runs it had started (A10, A00, P01) ran on to their own end.
Their `wall.json` is written by the driver's new `adopt` command from the status file's finish event (exit code and
elapsed time), marked `"adopted": true`. The remaining runs (F10, F00, C10, P05, S00) are started by new driver calls with
the amended binary, at most 3 solver processes at a time.

New question 4: **ADR 0159's addendum** says SA-a00 and SA-a17 met the targets "at iteration 8000"; SA-a00 stopped at 5157.
Options: (a) correct that sentence by a note in ADR 0159; (b) leave it to this record. Recommendation: (a), one line.
Question 5: **`min_iterations`** does not gate the residual targets. Options: (a) document it in `docs/case-schema.md`;
(b) an opt-in key that holds steady runs to a minimum iteration count; (c) change the gate (changes when default runs
stop). Recommendation: (a) now, (b) if steady studies need fixed iteration counts.

## Addendum: results (2026-10-29)

Commits: 590fe32 (registration, D1 code, driver, TMR files), 47a3751 (amendment 1, the `freestream` role), then the
results, the limit tests and this addendum.
- Binaries: A10, A00, B00, B10, P40 and P01 used `rcfd-w49a.exe` (the registration's build). F10, F00, C10, P05 and S00
  used `rcfd-w49a-am1.exe` (amendment 1's build).
- Host: shared with other agents, CPU 80-100 %, free memory 16-17 GB, at most 3 of my solver processes at a time. Wall
  times in `results.json` are therefore rough.
- Results: [`docs/investigations/job49a-sa-drag/`](../investigations/job49a-sa-drag/README.md).

### The finding

ADR 0159's SA drag was wrong for two independent reasons. Neither is the SA model, its coefficients or its wall law.

1. **H1 confirmed: the force report.** The default force components take the molecular viscosity. On a wall-function
   mesh the reported friction is a fraction of the shear the solver applies:
   - 0.319 of it on the aerofoil at y+ about 42;
   - 0.352 on the y+ 40 plate;
   - 1.005 at y+ 1.

   That is why cd at 0 degrees was about half the measured value, for SA and for SST alike. With the opt-in
   `total_effective`, A00 (ADR 0159's case unchanged) gives:

   | | This job | TMR |
   |---|---|---|
   | cd | 0.008484 (+3.6 %) | 0.008192 |
   | friction | 0.007060 (+3.3 %) | 0.006831 |
   | pressure | 0.001424 (+4.6 %) | 0.001361 |
2. **H2 confirmed: the far-field nuTilda.** Nothing fixes nuTilda on a `freestream` far field. The median outer
   nuTilda/nu of ADR 0159's runs:
   - 23.8 at 0 degrees;
   - **6.02e4** at 10 degrees, that is 0.01 U c of eddy viscosity in the whole outer flow.

   With the corrected far field (amendment 1's opt-in `turbulence = "freestream"`) it is 3.000 at both angles. At
   10 degrees that changes the solution:

   | | A10 (ADR 0159's far field) | F10 (corrected) | TMR |
   |---|---|---|---|
   | cl | 0.982 | **1.055** (-3.3 %) | 1.0909 |
   | cd (`total_effective`) | 0.0445 | **0.0155** (+26 %) | 0.01231 |
   | pressure part of cd | 0.0374 | 0.0095 | |

   On 500 chords (C10) the corrected set-up gives cl 1.065 (-2.4 %) and cd 0.0135 (+10 %). So most of F10's remaining
   cd excess is the 50-chord far field without a point-vortex correction. TMR's far-field page names drag at high lift as
   the sensitive quantity.
3. **H3 confirmed: the drift.**
   - A10's cd rose 18.7 % between the windows 1001-2000 and 7001-8000. Its nut volume average went from 0.0021 at
     iteration 635 to 0.0099 at 5000 and held there: the outer nuTilda filling the domain.
   - With the far field corrected, F10 met the residual targets (1e-6) at iteration 577, with cd constant to 5 digits
     over its last 80 iterations and the outer nuTilda at 3 nu.
   - The steady iteration itself is not the cause.

### Limits

| Id | Limit | Measured | Outcome |
|---|---|---|---|
| L1 | exit 0 or 5 (amendment 1) | A00 0, A10 5, F00 0, F10 0, P01 5, P05 0, P40 0 (B00, B10, C10: 0) | met |
| LR | A00, A10 bit-identical to ADR 0159 | 10 314 and 16 000 cl/cd values, 0 different; both stop where ADR 0159's did (5157, 8000) | met |
| C1 | laminar effective = default, bits | equal | met |
| C2 | SA y+ 40 plate: effective / molecular > 1.5 | passes (`force_effective_w49a.rs`) | met |
| C3 | pressure part unchanged | within 1e-12 | met |
| C4, C5 | (amendment 1) inflow faces 3 nu, outflow faces owner value; located error | pass | met |
| LP1 | wall-law Cf(0.97) vs TMR 2.70565e-3: P01 5 %, P05 and P40 10 % | P01 +3.11 %, P40 +4.31 %, **P05 +19.9 %** | P01, P40 met; **P05 missed** (`..._y_plus_5_known_miss`) |
| LP2 | P01 molecular / wall law 1 +- 0.03 | 1.0045 | met |
| LP3 | P40 molecular / wall law <= 0.6 | 0.352 | met |
| LP4 | report C_F vs wall-law integral, 2 % | P01 +0.25 %, P40 -0.02 % | met |
| LA1 | A00 cd_v / cd_veff <= 0.6 | 0.319 | met |
| LA2 | friction within 10 % of 0.006831 | A00 +3.3 %, F00 +2.5 % | met |
| LA3 | cd within 12 % of 0.008192 | A00 +3.6 %, F00 +2.6 % (0.008402) | met |
| LA4 | F10: cd within 25 % of 0.01231 and cl within 5 % of 1.0909 | cd 0.015506 (window mean) **+25.95 %**; cl 1.05465 -3.3 % | **missed** (cd), `..._known_miss` |
| LA5 | outer nuTilda/nu: A10 >= 1000, F00 and F10 <= 5 | 6.02e4; 3.000; 3.001 | met |
| LA6 | F10 abs(drift) <= 2 % | -97 % (see below) | **missed**, `..._known_miss` |
| B1 | folder <= 300 000 bytes, PNG <= 120 000, reference files <= 30 000 | see the commit; reference files 11 549 bytes | met (test `w49a_b1_storage`) |
| P1 | selftest | passed | met |
| G0 | fmt, clippy, baseline_hashes, w33b_default_bits, workspace_graph | all clean / passing; `baseline_hashes.rs` unchanged and its test passes (3 passed, 1 ignored as before) | met |

**About the three misses.**
- **LA4.** Missed by 0.95 points on cd. The final iteration's cd is 0.015484 (+25.8 %); the window mean includes the
  last 288 iterations of a run that converged at 577. C10 shows the 50-chord far field as the main cause.
- **LA6.** The miss is in amendment 1's reading rule, not in the flow. F10 ended at 577 iterations, so the early window
  (iterations 73-144) lies in the start-up transient, where cd falls from 0.08 to 0.016. Over the last 288 iterations cd
  ranges from 0.015484 to 0.015600 and settles to 0.0154838. The rule was fixed before F10 ran and is not changed after
  it.
- **LP1 at y+ 5.** A real wall-treatment observation (question 8). In the buffer layer SA's wall mode `auto` over-predicts
  the plate friction by 20 %. The molecular and wall-law values agree there (0.974), so it is not the report.

### Reported, no limit

- **SST at 0 degrees (S00)**, 4000-iteration cap, exit 5:
  - cd (default) 0.003637, ADR 0159's 0.0036;
  - `total_effective` **0.008317**, +1.5 % against TMR's SA value;
  - friction 0.006901 (+1.0 %), pressure 0.001416.

  SST's half-size drag in ADR 0159 was the same report effect. Its outer turbulence was not checked: SST was not the
  subject, and its cd did not drift.
- **Plate mean C_F** against Kármán-Schoenherr (0.002934 at Re_L 1e7): P01 +0.7 %, P05 +17.7 %, P40 +2.1 %.
- **Plate y+ at 0.97**: 0.99, 5.31 and 39.6 (targets 1, 5, 40).
- **B00 and B10** (the defective far field): B10 reached F10's solution (cd_eff 0.015484, cl 1.0550) by iteration 500 and
  held it to about 1500. Then nut hit the cap (1e5 nu) in the whole domain and nuTilda grew without bound: medians 5e71
  and 1e63 nu at the end. Final cd_eff 0.084 and 0.182.
- **Wall times**:
  - A runs: 1024 and 2282 s.
  - F runs: 48 and 126 s.
  - C10: 106 s.
  - Plates: 10 to 781 s.

### Deviations

1. **Amendment 1.** Arm B's far field was defective (`inlet` traps nuTilda). It was replaced by arm F with a new opt-in
   role, and limits LA2-LA6 moved from B to F. The window rule was amended because `min_iterations` does not hold runs to
   the cap. Both were done before any corrected run.
2. **Driver stopped.** I stopped my own scheduler process (not a solver) so that it would not start C10 with the defective
   set-up. A10, A00 and P01 ran on. Their `wall.json` was written by `adopt` from the finish event (`"adopted": true`); the
   wall time there is the solver's elapsed time.
3. **Binaries.** Two builds were used (see the head of the addendum). The amended build differs only by the new role and
   its check. P05 and S00 do not use the role.
4. **Reporting additions to the driver after registration.** Report-only items: the "reported" block, the
   Kármán-Schoenherr mean C_F, and C10/S00 in it. No limit's computation changed except by amendment 1.
5. **`nuTilda` is not a report field.** `nut_avg` (volume average of `nut`) was used instead, as registered in D3. The
   outer nuTilda limit (LA5) reads the stored `solution.vtu`.
6. **TMR files.** Fetched with `curl` from the TMR site (tmbwg.github.io) on 2026-10-29. Only the clcd file and the
   0-degree upper-surface cf zone are committed.

### Not run

- The full workspace test suite and the gate. Ran instead:
  - rcfd-post's integrals tests;
  - `force_effective_w49a` (C1-C5) and `sa_drag_w49a`;
  - `baseline_hashes`, `w33b_default_bits`, `workspace_graph`, `case_hashes_w48u`, `w48v_case_hashes`, `physics_schema`,
    `sst_schema`, `turb_models`, after the amendment's code;
  - clippy of rcfd-post and rcfd-api (lib and tests) and fmt.
- Other angles of ADR 0159's sweep with the fixes (question 3). The point-vortex far field. A C-grid. Linux.
- Q5 of ADR 0159 (the trailing-edge divergence at fine wall spacing): not touched. Nothing here bears on it directly. Its
  divergence came within 15-30 iterations, far faster than the far-field growth found here, and these runs stayed on M1w.
  Whether the unanchored far field contributes is not tested.
- k-epsilon with wall functions. It uses the same report, so H1 applies to it by the same code path, but it was not run.

### Questions for the maintainer (1-3 registered, 4-5 from amendment 1, 6-8 from the results; none is decided here)

1. **Default force report with wall functions.**
   - (a) Keep `total`/`viscous` molecular and point users of wall functions at `total_effective`.
   - (b) Make the effective viscosity the default, keeping `molecular` values for the old numbers. This changes the
     default monitors of every RANS run with wall cells above y+ about 1.

   **Recommendation (b)**, in a job of its own with its own record. The default is wrong by a factor of about 3 on
   wall-function meshes, for every RANS model. Until then (a) holds.
2. **Far-field turbulence of `velocity = "freestream"` patches.**
   - (a) Keep the opt-in `turbulence = "freestream"` (done here).
   - (b) Make it the default for such patches. This changes default results of every far-field RANS case.
   - (c) Document only.

   **Recommendation (b)**, in its own job. The default lets nuTilda grow to 6e4 nu at 10 degrees and triples the drag;
   `inlet`, the obvious workaround, is worse (amendment 1). (a) is in place now.
3. **ADR 0159's sweep.** Re-run its SA arm (and the DDES overlap) with `total_effective` and the `freestream` role.
   **Recommendation: yes.** cl at 10 degrees moves from 0.98 to 1.05 and the stall picture may move with it.
4. **ADR 0159's record** (from amendment 1). Recommendation: a one-line note in ADR 0159 that SA-a00 stopped at 5157.
5. **`min_iterations`** (from amendment 1). Recommendation: document now, an opt-in hold later if needed.
6. **LA4 (cd at 10 degrees, +26 % on 50 chords).**
   - (a) A point-vortex far-field correction (Thomas and Salas 1986) as an opt-in.
   - (b) Use 500-chord far fields for aerofoil validation (C10: +10 %).

   **Recommendation (b) now, (a) as a later job.**
7. **LA6's reading rule.**
   - (a) Accept the known miss as recorded.
   - (b) Register a drift metric relative to the run length for future jobs.

   **Recommendation (a) here, and (b) for future jobs.**
8. **SA wall mode `auto` in the buffer layer** (P05: +20 % at y+ 5.3).
   - (a) A wall-treatment job: the plate at y+ 3 to 15, and the switch at `fix_y_plus` = 5.
   - (b) Leave it.

   **Recommendation (a).** M1w's wall cells span y+ 5.5 to 61 (ADR 0159's SA-a00, median 42), so some lie in the buffer
   layer. Where they are was not mapped.
