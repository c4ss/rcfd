# ADR 0167: Job 49D, the NACA 0012 angle-of-attack sweep again, with ADR 0164's two fixes and a 500-chord far field

Status: **registered 2026-10-29**, before any solver run of this job. The set-up, every limit, the reading rules and the
not-run list are fixed here. **Results in the addendum (2026-10-30):** every limit met except LT15 (cd at 15 degrees
+26.0 %, band 25 %) and LD (the drift metric), which are known misses. No amendment was needed; one driver correction to D7's text (deviation 2). Not yet accepted by the
maintainer. Job 49D, on local main f9a520a (the worktree branch was fast-forwarded to it before any
work; branch `job49d-aoa-sweep-fixed`). Number: 0165 is the highest ADR in the main checkout and in every worktree; 0166 is
left to job 49C as the brief says.

House rules of [ADR 0130](0130-wave48a-overview-and-run-record-contract.md):
- limits are fixed here before the registered runs; a missed limit becomes an ignored `*_known_miss` test with the limit
  intact;
- a defective set-up gets an amendment before the corrected run;
- defaults of existing code stay bit-identical (no crate source changes in this job);
- `crates/rcfd-api/tests/baseline_hashes.rs` is not touched;
- no new crate, no unsafe code, no commit trailers, nothing is pushed.

Clean-room: our own code and published data only (Ladson 1988 via the NASA TMR, and the TMR CFL3D SA values, both already
in `validation-data/reference/`).

Parents:
- [ADR 0159](0159-job48y-naca0012-angle-of-attack-sweep-through-stall.md): the sweep, its mesh M1w, its driver, its rules
  D4 (steady), D7 (the curve), L2 (slope) and L3 (stall).
- [ADR 0164](0164-job49a-why-the-sa-drag-of-the-naca0012-is-wrong.md): the two causes of the wrong SA drag and their
  opt-in fixes (D1 the `total_effective` force component, amendment 1 the far-field `turbulence = "freestream"` role);
  its question 3 (this job), question 6 (500 chords now, the point-vortex correction later) and question 7 (a drift
  metric relative to the run length).

## The request (the maintainer, relayed)

Re-run ADR 0159's sweep with ADR 0164's two fixes, through their opt-in keys (job 49C is making them the defaults on its
own branch; explicit keys keep these results reproducible either way), on 500-chord far fields:
- the same angles (0 to 20 in steps of 2, 12 to 18 in steps of 1, extension 22 and 24);
- steady SA as the primary arm, steady SST report-only;
- the same mesh family M1w (256 x 128, wall y+ about 40), with the far field at 500 chords; the mesh builder of
  `scripts/w48y_aoa_sweep.py` gets a far-field parameter whose default keeps ADR 0159 reproducible;
- cl, cd and stall against Ladson (cl_max 1.612 at 17.1 degrees) and against the TMR SA values (0, 10 and 15 degrees);
  no tuning;
- the polar, cl_alpha, the drag polar, the stall angle, and the effect of each fix (a four-way comparison at a few
  angles);
- a drift metric relative to the run length (last quarter against the quarter before), registered before use.

Not this job: the trailing-edge divergence at fine wall spacing (ADR 0159 question 5); the point-vortex correction; DDES.

## What I knew when writing this (rule d)

No solver iteration of this job had run. What I had read, built or computed:

1. **ADR 0159's and ADR 0164's records** (the ADRs, `docs/investigations/job48y-aoa-sweep/` and `.../job49a-sa-drag/`).
   Known results that bear on the limits below:
   - ADR 0164 F00 (both fixes, 50 chords, alpha 0): cd (`total_effective`) 0.008402, **+2.6 %** against TMR's 0.008192.
   - ADR 0164 **C10** (both fixes, **500 chords**, alpha 10): cd 0.013547 (window mean; final iterate 0.013516),
     **+10.0 %** (+9.8 %) against TMR's 0.012311; cl 1.06495, **-2.4 %** against 1.0909. It met the residual targets at
     iteration 533. Its case is, by construction, this job's SA-a10 apart from the title and the report list, so SA-a10
     is a re-run of C10 on today's main (D8).
   - ADR 0164 F10 (50 chords): cd +26 %, cl -3.3 %. ADR 0159 SA-a10 (no fixes, 50 chords): cd 0.0396, cl 0.982.
   - ADR 0159's SA curve (no fixes, 50 chords) was steady at every angle, peaked at 20 degrees (1.528) and fell below
     0.9 cl_max first at 24 degrees. Whether that was helped by its outer eddy viscosity of about 6e4 nu (ADR 0164) is
     not known.
2. **Code reading.**
   - `turb::role`: `auto` on a `velocity = "freestream"` patch and the explicit `zero_gradient` both give
     `Role::ZeroGradient` today; `inlet` and `freestream` give `Role::Inlet`, and `freestream` builds
     `rcfd_fv::bc::Freestream { fixed_on_inflow: true }` with the inlet values (`TurbState::new`).
   - For SST the inlet values come from `inlet_values_sst`. With the defaults (`turbulence_spec = "intensity_length"`,
     `turbulence_length` = 0, unset) the length is floored at 1e-30, which gives omega of about 1e29 on the inflow faces.
     With `turbulence_spec = "intensity_viscosity_ratio"`, intensity 0.05 and ratio 10 the inflow values equal SST's
     initial field (`turbulence.initial_intensity` 0.05, `initial_viscosity_ratio` 10, the same speed 1 and the same
     beta*), which ADR 0159's zero-gradient far field held.
   - `solver.min_iterations` does not hold a run to the cap: the residual targets (1e-6) still stop it (ADR 0164
     defect 4). With the `freestream` role ADR 0164's runs stopped after 261 to 577 iterations.
3. **Meshes built** (not a solver run) with this branch's driver and the registration binary (below): M1w sha256
   `bd8ef3c6da2dd3681d38c18914c8fbdcddab77bb1381c0688e1e2dce5f0c0299` (ADR 0159's) and **M1w-far500** (M1w with the
   far field at 500 chords, grading 126272.29057048868, first cell at mid-chord 3.4977e-4 chord, `rcfd check-mesh`
   maximum non-orthogonality 73.56 degrees, volume ratio 6.38, no error) sha256
   `59e707a866e34ec2fef6dd13a41de6411f844ceff99a2b6a644cb45f3a70c889`, ADR 0164's F500w.
4. **`rcfd validate`** of SA-a00, SA-a10, N-a18 and SST-a12 as written by the driver: valid, warning SV101 only.
   `python scripts/w49d_aoa_sweep_fixed.py selftest` and `python scripts/w48y_aoa_sweep.py selftest` pass.
5. **The drift metric on ADR 0164's stored status streams** (computed before choosing LD's threshold, disclosed): the
   relative change of cd (`total_effective`) between the last quarter and the quarter before is -0.45 % for C10
   (533 iterations), -0.27 % for F10, 0.00 % for F00 and A00, and **+0.64 % for A10** (ADR 0159's set-up at 10 degrees,
   8000 iterations, whose cd rose 18.7 % between iterations 1001-2000 and 7001-8000). On cl at 0 degrees the relative
   metric is meaningless (cl about 0), so it is applied to cd only.

## Decisions (set-up; mine, none is a maintainer decision)

1. **D1 Angles.** alpha = 0, 2, 4, 6, 8, 10, 12, 13, 14, 15, 16, 17, 18, 20 (ADR 0159 D1) **and always 22 and 24**
   (16 angles). ADR 0159 ran the extension only when the base angles showed no stall; here it always runs, so that no run
   waits for another's result. Angle by `[mesh.transform] rotate_z_deg = -alpha` about the quarter chord, flow along +x,
   from the free stream at every angle.
2. **D2 Mesh M1w-far500.** M1w with the far-field circle at 500 chords and the grading changed so that the first radial
   step stays M1w's: `grading_for_first(geometric_first(1e4, 128) * 50 / 500, 128)`, ADR 0164's expression in the same
   order, so the points are ADR 0164's F500w bit for bit (sha256 above).
   - Builder: `scripts/w48y_aoa_sweep.py` gets `far_field_variant(name, far)` and `mesh ... --far C`. Without `--far`,
     every registered mesh is built as before (M1w's sha256 above is ADR 0159's).
   - Point-vortex correction: none (out of scope, ADR 0164 question 6).
3. **D3 Arms** (driver `scripts/w49d_aoa_sweep_fixed.py`, work folder `E:\projects\cfd\w49d-work\w` outside the
   repository). Every case is ADR 0159's case text (`w48y_aoa_sweep.case_text`) with:
   - the title naming this job;
   - the default reports `cl` and `cd` with `component = "total"` **written out** (the molecular wall shear, today's
     default);
   - extra reports `cl_eff`, `cd_eff` (`total_effective`), `cd_p` (`pressure`), `cd_veff` (`viscous_effective`), and for
     SA `nut_avg` (volume average of nut); reports do not change the solution (ADR 0164 LR);
   - the far-field turbulence line of the arm, **written out**:

   | Arm | Runs | Model, cap | Far-field turbulence |
   |---|---|---|---|
   | **SA** (primary) | 16 angles of D1 | SA (ADR 0159 D3 defaults), 8000 iterations, `min_iterations` 8000 | `turbulence = "freestream"` (nuTilda 3 nu on inflow faces) |
   | **N** (four-way) | 0, 10, 15, 18 | as SA | `turbulence = "zero_gradient"` (ADR 0159's far field) |
   | **SST** (report only) | 0, 4, 8, 12, 14, 16, 18, 20 (ADR 0159 amendment 2) | SST, 4000 iterations, `min_iterations` 4000 | `turbulence = "freestream"`, `turbulence_spec = "intensity_viscosity_ratio"`, intensity 0.05, ratio 10 (= the initial field, What I knew 2) |

   Everything else is ADR 0159's: SIMPLEC, linear upwind, SA-noft2 with SA-neg and wall mode `auto`, Re 6e6, one thread,
   the last snapshot stored. N's explicit `zero_gradient` keeps it ADR 0159's far field if job 49C changes what `auto`
   means; that explicit value means the same as `auto` today (What I knew 2). The molecular columns (`component =
   "total"`) depend on today's meaning of `total`; if job 49C changes that value's meaning, a re-run of this job's cases
   gives other molecular numbers (the effective ones and the solutions are not affected).
4. **D4 The four-way comparison** (report only) at **0, 10, 15 and 18 degrees**, from two runs per angle on the same
   mesh. The effective force changes only the report, so one run gives the force both ways:

   | Column | Run | Reports |
   |---|---|---|
   | old set-up | N | `cl`, `cd` |
   | effective force only | N | `cl_eff`, `cd_eff` |
   | freestream role only | SA | `cl`, `cd` |
   | both fixes | SA | `cl_eff`, `cd_eff` |

   The far-field size is not a column: all four are at 500 chords. The 50-to-500-chord effect is reported separately from
   existing data (ADR 0159's curve; ADR 0164 F10 against C10).
5. **D5 Values and the steady rule.** A run of n iterations; the window is its last min(1000, floor(n/2)) iterations
   (ADR 0164 amendment 1).
   - The value of a monitor: the **final iterate** when the run met the residual targets (exit 0); the **window mean**
     when it ran to its cap (exit 5). Reason: an exit-0 run is converged and its window can hold the start-up tail (ADR
     0164's LA4 miss was partly that). Both are recorded.
   - Steady (ADR 0159 D4, on the effective values): the window range of `cl_eff` <= 0.005 and of `cd_eff` <= 0.0005.
6. **D6 Drift metric** (ADR 0164 question 7), registered here before use: for a run of n iterations,
   **D = (mean of cd_eff over iterations (3n/4, n] - mean over (n/2, 3n/4]) / mean over (3n/4, n]** (integer division).
   Also recorded: the same on the molecular cd, and the absolute change of cl_eff between the same two quarters.
7. **D7 The sweep curve C(alpha)**: the SA arm's values of `cl_eff` and `cd_eff` (D5). An angle at which SA is not
   steady by D5 enters C with its window mean, flagged "not steady"; there is no DDES fallback in this job (not run).
8. **D8 C10 re-run** (report only): every iteration's `cl`, `cd` and `cd_eff` of SA-a10 against ADR 0164's C10 status
   stream (`E:\projects\cfd\w49a-work\w\runs\C10\solver-status.jsonl`), as numbers. main has merged other jobs since
   ADR 0164's build, so a difference is information, not a miss.
9. **D9 Limits as tests.** `crates/rcfd-api/tests/aoa_sweep_w49d.rs` reads the committed `results.json` and asserts each
   limit (no solver run); slope and stall are recomputed from the committed curve. A missed limit is an ignored
   `*_known_miss` test with the limit unchanged.
10. **D10 Placement.** New: this ADR; `scripts/w49d_aoa_sweep_fixed.py`; `docs/investigations/job49d-aoa-sweep-fixed/`
    (README, `results.json`, `results.csv`, `histories.csv`, `cl_alpha.png`, `polar.png`); the test file of D9. Changed:
    `scripts/w48y_aoa_sweep.py` (the far-field parameter and its self-test lines only). No crate source, no
    `Cargo.toml`/`Cargo.lock`, no example file, no existing test.

## Registered limits (fixed here, before any solver run of this job)

Set-up: release build of f9a520a (`cargo build --release --locked -p rcfd-cli`, `CARGO_TARGET_DIR`
`E:\projects\cfd\w49d-work\target`), copied out as `E:\projects\cfd\w49d-work\rcfd-w49d.exe`, sha256
`056669b7ade535b8dd19ec89eb378f6a3e8ed5426ae828228beb2d0ccc74d869`. Runs through the driver's `run`, **at most 2 solver
processes at a time** (jobs 49C and 49E share the host), free memory checked before each batch, wall cap 6 h per run.
Order (cost, not a rule): the SA arm, then N, then SST.

| Id | Check | Limit |
|---|---|---|
| L1 | Runs complete | every run of the SA and N arms (and every SST run made) ends with exit code 0 (residual targets) or 5 (cap) |
| L2 | Lift slope | least squares cl = a + b alpha (alpha in radians) through C at 0, 2, 4, 6, 8 degrees: **0.90 <= b / (2 pi) <= 1.05** (ADR 0159's band, unchanged) |
| L3 | Stall | on C over all 16 angles: some alpha after the peak with **C(alpha) <= 0.90 cl_max** (ADR 0159's rule, unchanged). The base angles alone are reported |
| L4 | Symmetry | **abs(cl_eff(0)) <= 0.005** |
| LT0 | TMR, alpha 0 | cd (C) within **12 %** of TMR's 0.0081925 (ADR 0164 LA3's band) |
| LT10 | TMR, alpha 10 | cd within **15 %** of 0.0123105 **and** cl within **5 %** of 1.0909 |
| LT15 | TMR, alpha 15 | cd within **25 %** of 0.0212366 **and** cl within **5 %** of 1.5461 |
| LD | Drift | every SA run: **abs(D) <= 0.005** (D6) |
| P2 | Mesh | M1w and M1w-far500 sha256 equal the values above |
| B1 | Storage | `docs/investigations/job49d-aoa-sweep-fixed/` **<= 400 000 bytes**, each PNG <= 120 000 bytes; nothing under `example-projects*/` changed |
| P1 | Driver | `python scripts/w49d_aoa_sweep_fixed.py selftest` and `python scripts/w48y_aoa_sweep.py selftest` pass |
| G0 | Hygiene | `cargo fmt -p rcfd-api -- --check` clean; `cargo clippy --release --locked -p rcfd-api --tests -- -D warnings` clean; `git diff f9a520a -- crates/rcfd-api/tests/baseline_hashes.rs` empty |

Why these bands and this threshold:
- **LT0, 12 %**: ADR 0164 LA3's band (y+ 40 wall law, first-order nuTilda convection, incompressible against M 0.15). F00
  at 50 chords was +2.6 %; at zero lift the far field matters little.
- **LT10, 15 %**: 12 % as at 0 degrees plus the far-field error of a fixed free stream at 500 chords without the
  point-vortex correction (ADR 0164's estimate of 15 % at 50 chords, scaled by 1/R: about 1.5 %), and more of the
  suction-side wall cells in the buffer layer at lift (ADR 0164 P05: +20 % on the plate at y+ 5). **C10 is known at
  +9.8 % (final iterate) and cl -2.4 %**, so LT10 is a reproduction check of C10 on today's main, not an independent test.
  The cl band is ADR 0164 LA4's.
- **LT15, 25 %**: unknown at 500 chords; nearer the stall the suction-side boundary layer thickens towards the trailing
  edge, where a y+ 40 wall law and a first-order nuTilda are least trustworthy, so the band is wider than at 10 degrees
  (ADR 0164 LA4's 25 %). cl 5 % as at 10.
- **LD, 0.5 %**: the metric compares adjacent quarters, so a geometrically converging run shows a small D and a slowly
  drifting one a larger D. On ADR 0164's streams it gives -0.45 % (C10), -0.27 % (F10) and +0.64 % (A10, the drift that
  ADR 0159's set-up showed). 0.5 % is chosen so that it flags A10's drift; I computed those values before choosing it
  (What I knew 5). It is a known weakness that the metric is weak on slow drifts: A10's 18.7 % drift over 8000
  iterations shows as 0.64 %.

**Reported, no limit** (nothing is tuned to them):
- the four-way table (D4) and the pressure part of cd in it; the outer nuTilda/nu (median over cells 5 to 40 chords
  upstream, ADR 0164's reading) of every SA and N run;
- cl_max, alpha_peak and the first angle below 0.9 cl_max against Ladson (1.6116 at 17.13); cl and cd at every angle
  against Ladson interpolated; the slope factor against Ladson's;
- the SST curve, its slope factor and its stall by L3's rule;
- the comparison with ADR 0159's curve (50 chords, no fixes);
- D8's C10 comparison; the steady flag, D, iterations, exit code and wall time of every run.

**Reading rules.**
- A limit is evaluated once, on the registered runs, by the driver's `collect` as committed.
- A limit whose run did not complete is reported as not evaluated, not as met.
- SST runs are report-only and may be dropped for time; dropped ones are listed as not run (L1 then covers the SST runs
  made).

## Honest differences from TMR and the experiment (stated before any result)

- Incompressible against M 0.15.
- A 256 x 128 O-grid with wall cells at y+ about 40 (5.5 to 61 at 0 degrees in ADR 0159) and SA's Spalding wall law,
  against TMR's 897 x 257 C-grid resolved to y+ below 1.
- 500 chords without the point-vortex correction (TMR: about 500 chords with it).
- First-order upwind convection of nuTilda against the TMR codes' second-order schemes.
- This repository's `naca_thickness` (ADR 0092) against TMR's sharp-trailing-edge NACA 0012 (11.894 % thick, chord 1).
- Fully turbulent RANS against Ladson's tripped boundary layer; steady RANS of a separating aerofoil depends on the model;
  every angle starts from the free stream, so hysteresis is not seen.
- SST's far field holds intensity 0.05 and viscosity ratio 10, not TMR's SST free-stream values; SST is report-only and
  is compared with the experiment, not with TMR's SA values.

## Out of scope / not run (registered)

- SA-DDES at any angle (ADR 0159's arm DD); a not-steady SA angle stays flagged in C (D7).
- The grid check on M2w (ADR 0159 G1).
- The trailing-edge divergence at fine wall spacing (ADR 0159 question 5); wall-resolved meshes.
- The point-vortex far-field correction; a C-grid; Linux.
- Any change of SA or SST coefficients, of a mesh parameter other than the far-field radius, or of any default.

## Addendum: results (2026-10-30)

Commits:
- f0334a2: the registration and the driver.
- 6a4d260: a plot and the limit tests, before any evaluation.
- 57bcd89: the results, the D7 correction of the driver (deviation 2) and this addendum; then a note on G0.

Every run used `E:\projects\cfd\w49d-work\rcfd-w49d.exe`, the registration's binary. Its sha256 was checked again after
the restart (deviation 1). Work folder: `E:\projects\cfd\w49d-work\w`. Results:
[`docs/investigations/job49d-aoa-sweep-fixed/`](../investigations/job49d-aoa-sweep-fixed/README.md).

The host was shared with other jobs throughout, so wall times are rough: 56 s to 37 min per run. At most 2 of my
solver processes ran at once.

### Limits

| Id | Limit | Measured | Outcome |
|---|---|---|---|
| L1 | every run exit 0 or 5 | 28 of 28. 16 SA: exit 0 from 0 to 13 degrees and at 16 (residual targets met at iterations 254 to 2008); exit 5 at 14, 15 and from 17 up. 4 N: exit 5. 8 SST: exit 0 up to 16, exit 5 at 18 and 20 | met |
| L2 | 0.90 <= slope / 2 pi <= 1.05 | 0.1081 per degree = **0.986** x 2 pi (intercept 0.0030) | met |
| L3 | some C after the peak <= 0.9 cl_max | peak **1.542 at 16 degrees**; at 17 degrees 1.048 (0.68 cl_max; an iteration mean, not steady). The base angles alone give the same | met |
| L4 | abs(cl(0)) <= 0.005 | -1.4e-6 | met |
| LT0 | cd(0) within 12 % of TMR | 0.008399, **+2.5 %** | met |
| LT10 | cd within 15 %, cl within 5 % | cd 0.013516, **+9.8 %**; cl 1.0656, **-2.3 %** | met |
| LT15 | cd within 25 %, cl within 5 % | cd 0.026762, **+26.0 %**; cl 1.4932, -3.4 % | **missed** (cd), `w49d_lt15_..._known_miss` |
| LD | every SA run abs(D) <= 0.5 % | over at 12 (-0.58 %), 13 (-0.52 %), 16 (-0.70 %), 17 (+0.78 %) and 24 (+1.40 %) | **missed**, `w49d_ld_..._known_miss` |
| P2 | mesh hashes | M1w and M1w-far500 equal the registered values | met |
| B1 | folder <= 400 000 bytes, PNG <= 120 000 | 311 288 bytes with the README (test `w49d_b1_storage_budget`); PNGs 32 596, 31 519 and 103 190 bytes | met |
| P1 | self-tests | both pass | met |
| G0 | fmt, clippy, baseline_hashes | `cargo fmt -p rcfd-api -- --check` clean; `cargo clippy --release --locked -p rcfd-api --tests -- -D warnings` clean (all rcfd-api test targets); `git diff f9a520a -- crates/rcfd-api/tests/baseline_hashes.rs example-projects example-projects-linux` empty | met |

**About the two misses.**
- **LT15.** It misses the cd band by 1.0 point; cl is inside its band. ADR 0164 left the remaining +10 % at 10 degrees
  unattributed (the y+ 40 wall law, wall cells in the buffer layer, first-order nuTilda, the O-grid). That remainder
  grows towards the stall: +2.5 % at 0, +9.8 % at 10, +26.0 % at 15. At 15 degrees cd splits into a pressure part of
  0.0223 and a friction part (`viscous_effective`) of 0.0045. TMR's 15-degree split is not in the repository, so it is
  not known which part holds the excess. The band was fixed before the run and is not changed.
- **LD.** The metric does what it was chosen for on the old set-up: the N runs give +1.2 % to +1.4 % at 10, 15 and 18
  degrees, the drift ADR 0164 found. Its misses on the fixed set-up have two causes.
  - At 12, 13 and 16 degrees the runs met the residual targets (1e-6) after 648, 777 and 2008 iterations. Their two
    quarters still hold the start-up tail, as ADR 0164's LA6 did.
  - At 17 and 24 degrees the steady iteration never settles (next section), so no metric of a steady value applies.

### The sweep (SA, both fixes, 500 chords)

The full table is in the folder's README.

- **Attached range.** From 2 to 15 degrees, cl is within 0.013 of Ladson's, and within 0.006 from 12 to 15. ADR 0159
  fell increasingly below Ladson from 6 degrees up, by 0.14 at 15.
- **Stall.** Steady SA peaks at **1.542 at 16 degrees**. Ladson peaks at 1.612 at 17.13. So SA is 4.3 % low and about
  1 degree early.
  - From 17 degrees up, steady SIMPLEC does not converge: cl swings between about 0 and 2 up to the 8000-iteration
    cap.
  - The flagged iteration means are 1.05, 1.00, 1.08, 1.23 and 1.27 at 17, 18, 20, 22 and 24 degrees. Ladson's 1.00
    at 18.02 is close to the 18-degree mean, but an iteration mean is not a flow value.
  - ADR 0159's steady SA stayed attached to 20 degrees because of its outer eddy viscosity of about 6e4 nu. The N runs
    show this at 18 degrees.
- **Drag.**
  - At 0 degrees cd is 0.0084, against Ladson's 0.0081.
  - From 6 degrees up it is increasingly above the measurement: +14 % at 10 and +46 % at 15.
  - Against TMR's fully turbulent SA: +2.5 % at 0, +9.8 % at 10 and +26 % at 15.
- **13 and 16 degrees** are flagged "not steady" by D5's window rule, although both runs met the residual targets. By
  D7 they enter C with their window means:

  | alpha | window mean (in C) | final iterate |
  |---|---|---|
  | 13 | 1.3423 | 1.3429 |
  | 16 | 1.5420 | 1.5436 |
- **D8.** SA-a10 equals ADR 0164's C10 at all 1599 compared values (cl, cd and cd_eff over 533 iterations), and both
  runs end at iteration 533. Today's main reproduces ADR 0164's amended build bit for bit on this case.

### Four-way comparison (D4; cl / cd, 500 chords)

| alpha | old | effective only | freestream only | both | TMR |
|---|---|---|---|---|---|
| 0 | 0.0000 / 0.00365 | 0.0000 / 0.00842 | 0.0000 / 0.00364 | 0.0000 / 0.00840 | 0.0000 / 0.00819 |
| 10 | 0.9878 / 0.03849 | 0.9879 / 0.04344 | 1.0654 / 0.00949 | 1.0656 / 0.01352 | 1.0909 / 0.01231 |
| 15 | 1.3563 / 0.08234 | 1.3567 / 0.08640 | 1.4926 / 0.02381 | 1.4932 / 0.02676 | 1.5461 / 0.02124 |
| 18 | 1.5016 / 0.11982 | 1.5022 / 0.12334 | 0.9982 / 0.31946 | 0.9981 / 0.32078 | |

- **The effective force** changes only the friction: cd rises by 0.003 to 0.005 where the flow is attached.
- **The freestream role** changes the solution.
  - Outer nuTilda/nu is 3.0 at every SA angle with the role. Without it: 7.3 at 0 degrees, and 5.9e4, 8.1e4 and 8.9e4
    at 10, 15 and 18 degrees.
  - At 10 degrees the pressure part of cd falls from 0.0363 to 0.0075.
  - cl rises by 0.08 at 10 degrees and by 0.14 at 15.
  - At 18 degrees the old set-up stays attached (cl 1.50); with the role the steady iteration separates.
- Neither fix alone is right at lift; both are needed.
- **Far-field size.** With the old set-up, going from 50 to 500 chords (against ADR 0159) changes cl by at most 0.006
  and cd by 1-3 %. With both fixes, cd at 10 degrees goes from +26 % (ADR 0164's F10, 50 chords) to +9.8 % here.

### SST (report only, both fixes)

| | 0 | 4 | 8 | 12 | 14 | 16 | 18 | 20 |
|---|---|---|---|---|---|---|---|---|
| cl | 0.0000 | 0.4368 | 0.8596 | 1.2457 | 1.4063 | 1.5089 | 1.0385 | 1.0597 |
| cd | 0.00832 | 0.00893 | 0.01125 | 0.01723 | 0.02342 | 0.03553 | 0.3174 | 0.3666 |

- Slope 0.980 x 2 pi.
- Peak 1.509 at 16 degrees; first angle below 0.9 cl_max at 18.
  - The window rule flags 16 degrees, although that run met the targets at iteration 1826.
  - 18 and 20 degrees are not steady.
- In ADR 0159, SST peaked at 14 degrees with 1.397.
- With both fixes, SST and SA now agree within 0.02 in cl up to 14 degrees (0.033 at 16), and within 2 % in cd up to
  12 degrees.

### Deviations

1. **PC restart.** The host restarted without warning while N-a00 and N-a18 were running.
   - Their partial folders were moved to `E:\projects\cfd\w49d-work\w\aborted-by-restart\` and are not counted.
   - Both runs were made again from the free stream, with the same binary and case.
   - No SST run had started. Every other run had written its `wall.json` before the restart.
2. **D7 in the driver.** The registration commit's `collect` put the D5 value (the final iterate when exit 0) into C at
   every angle. D7's text says an angle not steady by D5 enters with its window mean. I found this while reading the
   first full evaluation, and corrected the driver to the ADR's text before committing any results.
   - It changes only 13 and 16 degrees: cl 1.3429 becomes 1.3423, and 1.5436 becomes 1.5420.
   - No limit outcome changes: L3's peak stays at 16 degrees, and the LT angles are steady.
   - The report-only SST curve uses the same rule.
3. **Run order.** The SA arm ran first, two at a time. The N and SST runs then went in two single-process queues, so
   never more than 2 solver processes at once. One N run started while SA-a24 was still running.
4. **Report additions after registration** (report only, no limit):
   - `histories.png`, showing the not-steady SA runs and the N runs, thinned to stay under B1's PNG cap;
   - at most 61 samples per run in `histories.csv`;
   - final iterates next to the window means in the curve.
5. **Python.** The plots use matplotlib from the user's site packages without `python -I`, as in ADR 0159.

### Not run

- SA-DDES at 17 degrees and up, where steady SA does not converge (registered as not run; question 2).
- The M2w grid check, wall-resolved meshes, and the trailing-edge divergence (ADR 0159 question 5).
- The point-vortex far-field correction; Linux.
- The workspace test suite and the gate. What ran instead:
  - the new test file: 8 passed; the 2 known misses fail, as expected, under `--include-ignored`;
  - fmt, and clippy of all rcfd-api test targets (`--tests`);
  - the two driver self-tests.

  No crate source changed, so `baseline_hashes` was not re-run.

## Questions for the maintainer (1 registered, 2-5 from the results; none is decided here)

1. **SST's far-field values with the `freestream` role.** The defaults (`intensity_length`, no length) give omega of
   about 1e29 on the inflow faces. Options:
   - (a) document that `turbulence_spec` must be set;
   - (b) a located error when the role meets an unset length;
   - (c) default to the initial field's values.

   **Recommendation (b)**, together with job 49C's defaults. A silent 1e29 is the kind of value nobody notices.
2. **Stall from 17 degrees, where steady SA does not converge.** Options:
   - (a) a DDES or URANS arm (ADR 0159 question 3) at 16 to 20 degrees on M1w-far500 with both fixes;
   - (b) accept the steady curve with its flagged means.

   **Recommendation (a).** The peak at 16 degrees is credible; the post-stall values are not.
3. **The remaining drag excess at lift** (+9.8 % at 10 degrees and +26 % at 15, against TMR). The candidates are ADR
   0164's unattributed list. Options:
   - (a) a wall-treatment job (ADR 0164 question 8, the buffer layer), using this sweep's 10 and 15 degree cases as
     checks;
   - (b) second-order nuTilda convection;
   - (c) leave it.

   **Recommendation (a), then (b).**
4. **The drift metric (LD).** It catches slow drift (the N runs: +1.2 % to +1.4 %). It also flags converged runs that
   stopped early, and runs that swing. Options:
   - (a) in future jobs, evaluate D only on runs that reached their cap, and count exit 0 at the residual targets as
     converged;
   - (b) keep it as registered.

   **Recommendation (a)**, registered in the next job that uses the metric.
5. **Which curve is now the repository's NACA 0012 reference.** Options:
   - (a) this one (SA, both fixes, 500 chords) replaces ADR 0159's as the sweep the repository quotes;
   - (b) keep both.

   **Recommendation (a)**, keeping ADR 0159 as the record of the defect.
