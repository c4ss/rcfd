# ADR 0167: Job 49D, the NACA 0012 angle-of-attack sweep again, with ADR 0164's two fixes and a 500-chord far field

Status: **registered 2026-10-29**, before any solver run of this job. The set-up, every limit, the reading rules and the
not-run list are fixed here. Job 49D, on local main f9a520a (the worktree branch was fast-forwarded to it before any
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

## Questions for the maintainer (registered now; answers are not assumed)

1. **SST's far-field values.** The `freestream` role with SST uses the boundary's inlet settings, whose defaults
   (`intensity_length`, length unset) give omega of about 1e29. Options: (a) document that `turbulence_spec` must be set
   with that role for k-omega and k-epsilon; (b) a located error when the role is used with an unset length; (c) take the
   initial field's values by default. Recommendation: after the results, in the addendum.
