# ADR 0168: Job 49E, Spalart-Allmaras wall treatment with the wall cell in the buffer layer (ADR 0164 question 8)

Status: **registered 2026-10-29**, before any solver run of this job. **Amendment 1** (2026-10-30, after the 26 registered
runs, before any amended run): the opt-in had a y+ 5 switch (bistable) and over-produced nuTilda in the second cell;
both were fixed, with new arms F, AF, G. **Results in the addendum:** the default's buffer-layer error and its cause are
confirmed; the amended opt-in meets every limit; as registered, it missed LP1, LP2 and LP4 at y+ 5 (known misses). The
set-ups, limits, reading rules and the not-run
list are fixed here. Results go in an addendum. Not yet accepted by the maintainer.

Job 49E, on local main f9a520a; the worktree branch was fast-forwarded to it before any work. Number: 0165 is the highest
on main; 0166 (job 49C) and 0167 (job 49D) are taken in other worktrees; checked just before writing.

House rules of [ADR 0130](0130-wave48a-overview-and-run-record-contract.md):
- limits are fixed here before the registered runs; a missed limit becomes an ignored `*_known_miss` test with the limit
  intact;
- a defective set-up gets an amendment before the corrected run;
- defaults of existing code stay bit-identical; `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded;
- no new crate, no unsafe code, no commit trailers, nothing is pushed.

Clean-room. Our own code and published material only:
- the SA-noft2 equations of the NASA TMR page (`spalart.html`, fetched 2026-10-15, `docs/reference-data.md`);
- the TMR flat-plate SA value Cf(0.97) = 2.70565e-3 (ADR 0014) and the TMR NACA 0012 SA files committed by ADR 0164;
- Schlichting's local and Kármán-Schoenherr's mean friction laws.
- Kalitzin, Medic, Iaccarino and Durbin, "Near-wall behavior of RANS turbulence models and implications for wall
  functions", J. Comput. Phys. 204 (2005) 265-291: **only the abstract was read** (a library record, 2026-10-29). It
  proposes wall functions built from each model's own near-wall solution, tabulated, for SA among others. Nothing else
  of the paper is used or claimed.
- Patankar, "Numerical Heat Transfer and Fluid Flow" (1980), interface diffusivity from the series resistance between two
  nodes: recalled, not re-fetched. The form used here is derived below from first principles, so it does not rest on the
  recollection.

No vendor documentation was read. A search result pointed to a vendor manual page about SA wall treatment; it was not
opened.

Parents:
- [ADR 0164](0164-job49a-why-the-sa-drag-of-the-naca0012-is-wrong.md):
  - its flat plate (P01, P05, P40) and the known miss LP1 at y+ 5 (+19.9 %);
  - its effective force report (`viscous_effective`, `total_effective`);
  - its far-field role `turbulence = "freestream"`;
  - question 8 (this job).
- [ADR 0014](0014-sa-and-turbulence-interface.md): the SA model and its wall mode `auto`.

## The request (the maintainer, relayed: "add all those changes", including "start a wall-treatment job for SA at y+ 3-15")

1. Characterise the flat-plate Cf error against y+ = 1, 2, 3, 5, 8, 12, 15, 30, 60, through the same solver path and the
   effective force report, against published Cf correlations and the TMR SA value.
2. Find the cause in the code: how the SA wall treatment behaves in the buffer layer (nut wall value, u_tau iteration,
   blending, nuTilda wall value, first-cell treatment), against published forms.
3. An improvement only behind an opt-in key, documented in `docs/case-schema.md`, with Cf in a registered band across
   y+ 1 to 60, without tuning constants. Whether it becomes the default is a question, not a decision.
4. The effect on the NACA 0012 at 0 degrees on M1w-type meshes with wall y+ about 5, 15 and 40, against TMR's 0.00819.

## What I knew when writing this: code reading and a 1-D model, no solver run

### The code (`crates/rcfd-turb/src/sa.rs`, wall mode `auto`, the default)

Item by item, against the task's list:

| Item | What the code does | Verdict |
|---|---|---|
| u_tau iteration | Spalding's law (kappa 0.41, E 9.8) inverted by bisection from the wall cell's tangential speed and the face-normal distance `dn` | Correct for Spalding. Spalding is not the SA model's own law: see the table below. |
| nut wall value | wall face: `nu_t,w = u_tau^2 dn / u_P - nu`, so the wall shear is u_tau^2 at every y+ | Correct (a consistent wall function). Not the cause. |
| nuTilda wall value | `FixedValue 0` on the wall face | Correct (TMR). With the wall cell fixed (y+ > 5) it does not act. |
| blending | none: a hard switch at `fix_y_plus` = 5. Above, the wall cell's nuTilda is fixed and its strain is set from Spalding's gradient; below, the cell is solved | Not the cause. The 1-D model below gives about the same error on both sides of the switch (+15.1 % at y+ 5, +16.5 % at 5.31). |
| first-cell treatment | above y+ 5: nuTilda of the wall cell such that `nuTilda fv1` equals Spalding's eddy viscosity `nu (dy+/du+ - 1)` | A small inconsistency: SA's own nuTilda there is `kappa u_tau y`. At y+ 5.3 the two differ by 20 % (2.6 nu against 2.2 nu). Minor. |
| **face eddy viscosity** | `effective_viscosity`: nut **linearly interpolated** to every internal face | **The cause.** See below. |

**The mechanism.** In the buffer layer the eddy viscosity rises steeply and non-linearly. SA's own nut/nu is 0.05 at y+ 5,
0.66 at 10, 2.4 at 15 and 5.0 at 20. On ADR 0164's P05 mesh (growth 1.15) the wall-cell centre sits at y+ 5.3, the next
centre at y+ 16.7 and the face between them at y+ 10.6.
- The momentum flux through that face is `(nu + nut_f)(u_2 - u_1)/dy`.
- With nut_f linearly interpolated from the two cells (0.13 nu and 3.2 nu), the face viscosity is 2.6 nu.
- The resistance of the true profile over that interval corresponds to about 1.8 nu.
- So the velocity difference u_2+ - u_1+ is about 4.4 instead of about 6.3.

The wall function correctly maps u_1 to u_tau. But the profile above has lost about 1.9 units of u+, so the whole
velocity profile is too steep for its u_tau. For the same free stream, u_tau is too large. With U_e+ = sqrt(2/Cf) = 27.2
that is +15 % to +17 % in Cf.
- At y+ 40 the two cells lie in the log layer, where nut is linear in y, and the error is small.
- At y+ 1 to 3 several cells lie in the buffer layer, and the error is spread over several faces.

**SA's own inner-layer law** (derived here from the TMR equations). In a constant-stress layer, `nuTilda = kappa u_tau y`
is an exact solution of SA-noft2:
- with `du/dy = u_tau^2/(nu + nuTilda fv1)` and `chi = kappa y+`, `S~ = du/dy + nuTilda fv2/(kappa^2 y^2)
  = (u_tau/(kappa y)) [chi/(1 + chi fv1) + 1 - chi/(1 + chi fv1)] = u_tau/(kappa y)`;
- so `r = 1` and `fw = 1`;
- production `cb1 u_tau^2` plus diffusion `(1 + cb2) kappa^2 u_tau^2 / sigma` equals destruction `cw1 kappa^2 u_tau^2`,
  which is the definition of cw1.

So SA's law of the wall is `u+(y+) = integral_0^y+ dy'/(1 + kappa y' fv1(kappa y'))`. That is the "model's own near-wall
solution" of Kalitzin et al.'s abstract, with no constant beyond SA's kappa and cv1. Against Spalding (E 9.8), computed
by quadrature (`scripts/w49e_sa_wall.py`, `model`):

| y+ | 1 | 3 | 5 | 8 | 10 | 15 | 20 | 40 | 100 | 1e4 |
|---|---|---|---|---|---|---|---|---|---|---|
| SA u+ | 1.000 | 2.996 | 4.953 | 7.580 | 8.955 | 11.089 | 12.190 | 14.115 | 16.320 | 27.498 |
| Spalding u+ | 1.000 | 2.988 | 4.891 | 7.290 | 8.484 | 10.438 | 11.620 | 13.988 | 16.593 | 28.032 |

SA's log intercept is B = 5.03; Spalding's with E 9.8 is 5.57.

### The 1-D model (D7; a prediction, registered)

`model_1d` in the driver models a constant-stress layer (tau+ = 1) on the plate's wall layers. The first centre is at
y1+, the growth 1.15. The cells above the wall cell carry SA's inner-layer solution `nuTilda+ = kappa y+`. Integrated
from the wall function outwards to y+ 400, the model gives the u+ offset and from it the Cf error
`(1 + du+/U_e+)^-2 - 1`.
- Default: Spalding u+ of the wall cell; its nut is fixed to Spalding's above y+ 5; linear nut on the faces.
- Opt-in: as D1 below.

| y+ | 1 | 2 | 3 | 5 | 8 | 12 | 15 | 30 | 40 | 60 |
|---|---|---|---|---|---|---|---|---|---|---|
| default | +0.9 % | +3.2 % | +6.5 % | +15.1 % | +17.4 % | +11.9 % | +8.9 % | +2.7 % | +1.4 % | +0.1 % |
| opt-in (D1) | -0.7 % | -1.0 % | -1.4 % | -0.3 % | +0.2 % | +0.1 % | +0.1 % | +0.2 % | +0.3 % | +0.4 % |

The model also attributes the pieces of D1. I ran these variants before choosing; they are not registered arms:
- the interpolated-nuTilda faces alone fix y+ 1 to 5 (-1.7 % to +0.2 %) but leave +12 % at y+ 8 to 12;
- the series-resistance first face with SA's law alone fixes y+ above 5 (0.0 % to +0.4 %) but leaves +6 % to +15 % at
  y+ 3 to 5, below the switch;
- the series-resistance face with Spalding's law leaves -2 % to +5 % (the B mismatch).

ADR 0164's measured plate errors sit about 2.2 points above the model's default curve: P01 +3.1 % against +0.9 %, P05
+19.9 % against +16.5 % at its y+ 5.31, and P40 +4.3 % against +1.4 % at 39.6. So the plate set-up has a baseline of
about +2 % to +3 % that the model does not contain: its streamwise resolution, the leading edge, first-order nuTilda
convection. The opt-in cannot remove that baseline.

## D1. The opt-in: `turbulence.sa.wall_mode = "auto_sa_law"` (code; the default `auto` is unchanged)

A third value of the existing enumeration. No new key, so no default-valued line is added to written settings (the
reason ADR 0164 D1 gives). `SaWallMode::SaLaw` in `crates/rcfd-turb/src/sa.rs`:
1. **The wall law is SA's own** (`SaInnerLaw`).
   - It is tabulated once, when the mode is first used: Simpson quadrature in ln y+ from 1e-3 to 1e8, 64 intervals per
     decade.
   - It is read by cubic Hermite interpolation with the exact derivative.
   - Below the table `u+ = y+`; above it the log asymptote, continuous.
   - u_tau, the reported y+ and the wall-face `nu_t,w = u_tau^2 dn/u_P - nu` use it.
   - `turbulence.e_wall` is not used in this mode.
2. **Wall cells above `fix_y_plus` (5, unchanged)**: nuTilda is fixed to `kappa u_tau dn`, the inner-layer solution,
   and the strain to `u_tau^2/(nu + nut(y+))`.
3. **The wall-normal first face of those cells** gets the series-resistance viscosity of the inner law between the two
   centres: `nu_eff = nu (y2+ - y1+)/(u+(y2+) - u+(y1+))`.
   - Here y1 = dn, and y2 = y1 plus the wall-normal component of the step between the two centres.
   - This is the same construction as the wall face's wall function (`nu_eff,w = nu y1+/u+(y1+)`), applied to the next
     interval. With a constant flux, it makes the face flux exact for the law's profile.
   - Face selection: the internal face of the wall cell whose centre-to-centre step has the largest cosine with the
     inward wall normal, above 0.5, and whose neighbour has no wall face. The 0.5 is a geometric selection threshold,
     not a flow constant.
   - Count: `law_face_count()`.
4. **Every other internal face** gets `nut_f = nuTilda_f fv1(nuTilda_f/nu)`, with nuTilda_f linearly interpolated. This
   uses the property that makes nuTilda the working variable: it is linear in y near the wall, while nut is not.
   - Away from walls, where chi is large and fv1 is about 1, this equals the default to within the fv1 deviation.
5. The face values are under-relaxed with the model's `relax_nut`, like the wall-face values. In the default mode
   nothing of this is built or stored; `nu_face()` returns the default vector unchanged.

Code touched:
- `crates/rcfd-turb/src/sa.rs` (the variant, `SaInnerLaw`, the mode's branches, the unit test T1);
- `crates/rcfd-turb/src/lib.rs` (export);
- `crates/rcfd-api/src/schema.rs` (one enumeration value and its doc sentence);
- `crates/rcfd-api/src/turb.rs` (one match arm);
- `docs/case-schema.md`.

Tests: `crates/rcfd-api/tests/sa_wall_w49e.rs` (T2 and the limit tests).

## Set-up (decisions are mine; none is a maintainer decision)

- **D2 Plates.** ADR 0164's plate case (`w49a_sa_drag.PLATE`, unchanged below the title): SA, `rcfd run`, SIMPLEC,
  linear upwind, Re 5e6 per m, x = 0..2, inlet nuTilda 3 nu, 200 cells in x, wall layers with growth 1.15 and target y+
  at x = 0.97, 15000-iteration cap, reports `cf_v` and `cf_veff`.
  - Layers (wall layers / ny): 1: 50/70, 2: 45/65, 3: 43/63, 5: 40/60, 8: 37/57, 12: 34/54, 15: 32/52, 30: 27/47, 40:
    25/45, 60: 22/42.
  - The values at 1, 5 and 40 are ADR 0164's. The others keep the layer stack at 0.06 to 0.12 of the height (checked by
    the selftest), with ny = layers + 20.
  - Arms **Dnn** (default) and **Enn** (with D1), nn = the target y+. y+ 40 is added to the task's list for the bit
    check against P40.
- **D3 Aerofoil.** ADR 0164's F00 case: NACA 0012 at 0 degrees, Re 6e6, SA defaults, far-field role
  `turbulence = "freestream"`, the reports `cd_p`, `cd_v`, `cd_eff`, `cd_veff`, `cl_eff` and `nut_avg`, an 8000-iteration
  cap. It runs on three meshes:

  | Mesh | Grading | First cell at mid-chord | y+ estimate | sha256 | Max non-orthogonality / aspect / volume ratio |
  |---|---|---|---|---|---|
  | M1w | 1e4 | 3.494e-4 | 41.3 | `bd8ef3c6...0c0299` | 73.5 / 657 / 4.9 |
  | M1w15 | 31330.39 | 1.248e-4 | 14.8 | `62b49fc8...b4aac` | 71.4 / 809 / 5.8 |
  | M1w05 | 104430.79 | 4.160e-5 | 4.9 | `8e04edea...54451c` | 70.7 / 2426 / 6.6 |

  - M1w is ADR 0159's, rebuilt equal.
  - The other two scale M1w's first cell by 15/42 and 5/42. 42 is the wall-cell median y+ of M1w measured by ADR 0164.
  - All meshes were built by `w48y_aoa_sweep.build_mesh` through `rcfd convert`, with no check-mesh error. Full hashes
    are in `<work>/meshes/*.json`.
  - Arms: **ADyy** (default; AD40 is F00) and **AEyy** (with D1), yy = 05, 15, 40.
- **D4 Reading.** These follow ADR 0164 D4 and amendment 1:
  - The window is the last min(1000, floor(n/2)) iterations of a run of n.
  - Plate Cf(0.97): the wall law's `2 u_tau^2` with `u_tau = y+ nu / y_P`, from the model's y+ field in the final
    `solution.vtu`, interpolated between wall cells (`w49a_sa_drag.plate_wall`). In mode D1 this y+ is SA's law, so it
    is the applied shear.
  - Plate mean C_F: the integral of that local Cf; and the report `cf_veff / (2 depth)`.
  - Aerofoil: the window means of the reports.
  - The wall-cell y+ distribution comes from the final `solution.vtu`.
- **D5 Bit checks** compare every iteration's monitors of the status stream (`solver-status.jsonl`) against ADR 0164's
  stored runs in `E:\projects\cfd\w49a-work\w\runs`:
  - D01, D05, D40 against P01, P05, P40 (`cf_v`, `cf_veff`);
  - AD40 against F00 (`cl`, `cd`, `cd_p`, `cd_v`, `cd_eff`, `cd_veff`, `cl_eff`, `nut_avg`);
  - the iteration counts must be equal too.
- **D6 Binary.** Release build of this branch: `cargo build --release --locked -p rcfd-cli`, `CARGO_TARGET_DIR`
  `E:\projects\cfd\w49e-work\target`. It is copied out as `E:\projects\cfd\w49e-work\rcfd-w49e.exe`, sha256
  `9e76453ef12468ed50910f06c3c7108224499ba4c9ac848858f439ebd81c1d74`. That is the code of this commit; a code change
  before the runs is named in an amendment.
  - Runs go through the driver's `run`, at most **2** solver processes at a time (jobs 49C and 49D share the machine),
    one thread each. Free memory is checked before each batch.
  - Work folder: `E:\projects\cfd\w49e-work\w`.
- **D7** The 1-D model as above, registered as the prediction of LH1.
- **D8 Placement.**
  - New: this ADR; `scripts/w49e_sa_wall.py`; `docs/investigations/job49e-sa-wall/` (README, `results.json`,
    `results.csv`, `cf_vs_yplus.png`); `crates/rcfd-api/tests/sa_wall_w49e.rs`.
  - Changed: as listed under D1.
  - Budget **B1**: the folder at most 250 000 bytes, the PNG at most 100 000 bytes.

## Registered limits (fixed here, before any solver run of this job)

| Id | Check | Limit |
|---|---|---|
| L1 | Runs complete | every registered run (20 plates, 6 aerofoil) ends with exit code 0 or 5 |
| LR | Defaults bit-identical | D01, D05, D40 and AD40 equal ADR 0164's P01, P05, P40 and F00 in every compared monitor value and in iteration count (D5) |
| T1 | Inner law | `sa::tests::w49e_inner_law_quadrature_limits_and_inversion`: table against an independent quadrature within 1e-7, B in 4.9-5.2, monotone, u_tau inversion within 1e-10 |
| T2 | Wiring | `sa_wall_w49e::t2_auto_sa_law_is_wired`: the value parses and runs (300 iterations, coarse y+ 10 plate) to a finite positive friction different from the default; an unknown value is an error at the key |
| LH1 | The mechanism, default | default plates: measured Cf(0.97) error against TMR minus the 1-D model's default error at the run's own y+(0.97), shifted by D01's measured offset (model at D01's y+ subtracted from D01's error): within **3 points** at all 10 y+ |
| LH2 | Buffer-layer error exists | default plates: the largest Cf(0.97) error over y+ 5, 8, 12, 15 is **>= +10 %** |
| LH3 | The aerofoil sees it | AD05: friction `cd_veff` **>= +8 %** against TMR's friction part 0.006831 |
| LP1 | Opt-in, against TMR | Enn: wall-law Cf(0.97) within **+-5 %** of 2.70565e-3 at all 10 y+ |
| LP2 | Opt-in, y+ independence | Enn: abs(Cf(0.97)/Cf(0.97) of E01 - 1) **<= 3 %** at all 10 y+ |
| LP3 | Opt-in, report = applied shear | Enn: report C_F (`viscous_effective`) against the integral of the wall-law local Cf within **2 %** at all 10 y+ |
| LP4 | Opt-in, mean friction | Enn: wall-law mean C_F within **+-5 %** of Kármán-Schoenherr at Re_L 1e7 (0.002934) at all 10 y+ |
| LA1 | Opt-in aerofoil cd | AE05, AE15, AE40: `cd_eff` within **+-6 %** of TMR 0.008192 |
| LA2 | Opt-in aerofoil friction | AE05, AE15, AE40: `cd_veff` within **+-6 %** of TMR's friction part 0.006831 |
| LA3 | Opt-in aerofoil y+ independence | (max - min) of AE `cd_eff` over the three meshes / 0.008192 **<= 4 %** |
| B1 | Storage | investigation folder <= 250 000 bytes, PNG <= 100 000 bytes |
| P1 | Driver | `python scripts/w49e_sa_wall.py selftest` passes |
| G0 | Hygiene | `cargo fmt -p rcfd-turb -p rcfd-api` clean; clippy `-D warnings` of rcfd-turb and rcfd-api (lib and tests) clean; `git diff f9a520a -- crates/rcfd-api/tests/baseline_hashes.rs` empty and the test passes; `rcfd-fv` `w33b_default_bits`, `rcfd-cli` `workspace_graph`, `case_hashes_w48u`, `w48v_case_hashes` pass; rcfd-turb's `flat_plate_sa` (ADR 0014's S checks, default mode) passes |

Why these bands:
- **LP1's 5 %** is the task's example band. The plate set-up's own baseline is about +2 % to +3 % (ADR 0164's P01 +3.1 %),
  and the 1-D model puts D1 between -1.4 % and +0.4 %. So the expected range is about +1 % to +3.5 %: 1.5 points of
  headroom.
- **LP2's 3 %** isolates the wall treatment from that baseline. The model's spread of D1 is 1.8 points.
- **LH1's 3 points** covers the model's missing effects (streamwise variation, production at the second cell). Its
  agreement with ADR 0164's three plates was within 1.2, 2.2 and 0.7 points, with the offset taken from P01.
- **LA1 and LA2's 6 %.** ADR 0164's F00 (y+ 40, default) was +2.6 % (cd) and +2.5 % (friction). D1's change at y+ 40 is
  small. The finer meshes add an O-grid pressure-drag change that is not modelled.

**Reported, no limit** (nothing is tuned to them):
- each plate against Schlichting's local `cf = (2 log10 Re_x - 0.65)^-2.3` (0.002881 at x = 0.97; TMR's SA value is
  -6.1 % from it, a model property);
- the molecular / wall-law ratio;
- D1 minus default at y+ 1;
- each aerofoil run's split (pressure against TMR's 0.001361), its iterations, the wall-cell y+ median, 10 % / 90 %
  quantiles and range, and the number of wall cells above y+ 5 (those that get the series-resistance face);
- wall times.

**Reading rules.**
- A limit is evaluated once, on the registered runs, by the driver's `collect` as committed.
- A limit whose run did not complete (or diverged) is reported as not evaluated, not as met, and counts as missed for its
  test.
- **The M1w05 runs may diverge at the trailing edge** (ADR 0159 question 5: M1, wall y+ about 1, diverged there). A
  diverged AD05 or AE05 is recorded as such; no corrected set-up is run without an amendment first.

## Honest differences and limitations (stated before any result)

- D1 assumes the equilibrium inner layer on the wall cell and on its first face. That is the same assumption as every wall
  function, but now over two cells instead of one. Near separation, the stagnation point and the trailing edge it is a
  model, not a resolution. Fine meshes (wall y+ below 5) use it only through the interpolated-nuTilda faces.
- The series-resistance face uses the wall-normal distance between the centres. The momentum operator uses its own face
  distance. On wall-aligned layers (the plate, the O-grid) the two agree; on skewed near-wall cells they do not, and no
  correction is made.
- The interpolated-nuTilda face value also acts away from walls, wherever chi is moderate (free stream with nuTilda = 3 nu,
  wake edges). It is not the default's face value there. Its effect on the aerofoil's pressure drag is reported, not
  limited.
- D1 has not been tested with DES / DDES or with the transient algorithms; nothing forbids the combination.
- The plates have no upstream symmetry run-up, unlike TMR (ADR 0014).

## Out of scope

- Any change of SA coefficients, of `fix_y_plus`, or of Spalding's constants.
- SST and k-epsilon wall treatments. The same linear-nut face interpolation exists there; that is question 3.
- Angles other than 0 degrees, the 500-chord far field, the trailing-edge divergence (ADR 0159 question 5).

## Questions for the maintainer (registered now; answers are not assumed)

1. **Default of the SA wall mode.**
   - (a) Keep `auto` as the default, with D1 opt-in.
   - (b) Make `auto_sa_law` the default. That changes every SA result with wall cells above y+ about 1.
   - (c) Drop D1.

   Recommendation in the addendum, after the results.
2. **The `auto` documentation** says it "works from y+ about 1 into the log layer". ADR 0164's P05 and this job's
   characterisation measure that claim. Options:
   - (a) correct the text in `docs/case-schema.md` and the schema's doc string to name the buffer-layer error;
   - (b) leave it.

   Recommendation in the addendum.
3. **The same face interpolation in SST / k-epsilon.** Whether to characterise their buffer-layer error in a job of its
   own. Recommendation in the addendum.

## Amendment 1 (2026-10-30, after all 26 registered runs and before any amended run): two defects of D1; arms F, AF and G

**What had run.** All 26 registered runs ended (L1). The registered results are committed as collected (commit 8d59b02).
They stand as the record of D1 as registered: every registered limit is evaluated on them as registered, with a
`*_known_miss` test for each miss and the limit intact. Nothing below moves a registered limit onto other runs.
- Met: L1, LR, LH1, LH2, LH3, LP3, LA1, LA2, LA3.
- Missed: LP1, LP2 and LP4, each at y+ 5 only (E05).
- Details are in the addendum.

**What I saw, and the profile diagnostics that led to this amendment.** I read the stored `solution.vtu` of E05, E08,
E12, E15, E01 and E03 with a scratch script; no run was made for this.
1. **Defect 5: the switch at `fix_y_plus` = 5 makes D1 bistable at y+ 5.**
   - E05's wall-cell y+ is pinned at 4.97 to 5.05 along x = 0.8 to 1.2. D05's falls smoothly from 5.40 to 5.22.
   - Above 5 the cell is fixed and gets the series-resistance face; below 5 it is free and gets the interpolated face.
     The two branches give different u_tau, so the cell flips between them.
   - E05 ran to the cap (exit 5) with Cf(0.97) +7.5 % against TMR. D5's 1-D jump at the switch is only about
     0.9 points.
   - AE40 (wall y+ minimum 5.08) and AE05 also ran to the cap; AD40 met its targets at iteration 261.
2. **Defect 6: the second cell's nuTilda is over-produced next to the series-resistance face.**
   - At x = 0.97 the second cell's nuTilda/(kappa u_tau y) is 1.26 (E05), 1.28 (E08), 1.19 (E12) and 1.14 (E15). The
     defaults' values are 1.06 to 1.09.
   - Its strain comes from the Green-Gauss gradient with the face velocity linearly interpolated between the two cells.
     Across the buffer-layer profile that velocity is too low (E05: 7.83 u_tau against the inner law's 8.98 at the face).
     So the strain of the second cell is about 34 % high.
   - Its eddy viscosity then makes the next face too viscous. E05 loses 0.4 of u+ between the second and third cells,
     E08 0.22.
   - The 1-D model (D7) has no production and could not show this.
   - Provenance: this correction was designed after these profile diagnostics, not before the runs.

**Correction (code, opt-in only; the default `auto` is unchanged).** In mode `auto_sa_law`:
- **(a) No switch.** Every wall cell with u_tau > 0 gets nuTilda = kappa u_tau dn, its law strain and the
  series-resistance face, at every y+. `fix_y_plus` is not used in this mode.
  - Why: nuTilda = kappa u_tau y is the exact constant-stress solution at every y+ (derived above), so the switch has no
    basis in this mode.
  - The 1-D model's predictions move at y+ 2 to 4 (the cells were free there): y+ 1 -0.7 %, 2 -0.6 %, 3 +0.5 %, 5 +0.6 %.
    Above 5 they are unchanged (+0.1 % to +0.4 %).
- **(b) The law's face velocity in the gradient.** In the Green-Gauss velocity gradient of the two cells beside a
  series-resistance face, the face velocity's component along the wall cell's tangential direction is the inner law's
  `u_tau u+(y_f+)`, with y_f = dn plus the wall-normal offset of the face centre. It replaces the linear interpolation.
  - This is the same consistency as the series-resistance flux (the face quantity from the law, not from the
    interpolation), with no constant.
  - The gradient feeds only SA's vorticity, and the DDES `grad_mag`. It does not feed the momentum equation.
- Code: `crates/rcfd-turb/src/sa.rs`. `law_face_gradient` is new. The SaLaw wall-cell branch and the series-resistance
  branch lose their `fix_y_plus` condition. The lazy builds of the table and the face list move ahead of the vorticity.
  The default path's text changes only by `let mut gr`, so the default bits are re-checked (LR_A1). Also changed: the
  doc sentences in `schema.rs` and `docs/case-schema.md`.
- **Binary** for every amended run: `E:\projects\cfd\w49e-work\rcfd-w49e-am1.exe`, sha256
  `89b0ecda12fc3a832e4d7841e64b3d7a747a351b953f13de8113b0f4c960992b`, the code of the amendment commit.

**New arms** (same cases; the amended binary; at most 2 solver processes):
- **Fnn**: Enn with the amended opt-in, nn = 01 ... 60.
- **AFyy**: AEyy with the amended opt-in, yy = 05, 15, 40.
- **G05, G40, GA40**: D05, D40 and AD40 again, the default bits of the amended build.

**New limits** (fixed now; the registered limits keep their values and their runs):

| Id | Check | Limit |
|---|---|---|
| L1_A1 | Runs complete | every F, AF and G run exits 0 or 5 |
| LR_A1 | Defaults bit-identical, amended build | G05, G40, GA40 equal P05, P40, F00 in every compared monitor value and in iteration count |
| LP1_A1 to LP4_A1 | LP1 to LP4 on Fnn | the registered bands unchanged: +-5 % against TMR; 3 % against F01; 2 % report against integral; +-5 % against Kármán-Schoenherr; at all 10 y+ |
| LA1_A1 to LA3_A1 | LA1 to LA3 on AFyy | unchanged: +-6 % cd; +-6 % friction; spread <= 4 % |
| M1_A1 | (a) removes the pin | every Fnn: wall-cell y+ non-increasing along x = 0.8 to 1.2 |
| M2_A1 | (b) removes the over-production | F05 ... F60: second cell's nuTilda/(kappa u_tau y) at x = 0.97 **<= 1.10** |
| M3_A1 | Steady runs settle | F05 ... F60, AF15, AF40 end with exit 0 (residual and turbulence-change targets met before the cap). F01 to F03 and AF05 are excluded: their defaults D01 to D03 and AD05 also ran to the cap |
| G0 | Hygiene | as registered, on the final code, including `flat_plate_sa` |

Reading rules unchanged. Not changed: the cases, the meshes, the window, every band.

**Honest difference added.** Without the switch, every wall cell gets the equilibrium inner-layer value and the
series-resistance face. That includes a wall cell resolving the viscous sublayer (y+ about 1) and cells where u_tau tends
to zero (stagnation, separation, the trailing edge). Below y+ about 3 the law is the viscous law u+ = y+ and imposes
little. Near separation it is the equilibrium assumption of every wall function.

## Addendum: results (2026-10-30)

Commits:
- 22dd93f: registration, D1 code, driver, T1/T2.
- 3b1b967: the T2 clippy fix.
- 8d59b02: the registered results, collected as committed.
- 812f078: amendment 1, its code and driver.
- Then the amended results, the limit tests and this addendum.

Binaries:
- `rcfd-w49e.exe` (registered) for the 26 registered runs;
- `rcfd-w49e-am1.exe` (amendment 1) for F, AF and G.

Host: shared with jobs 49C and 49D, at most 2 of my solver processes, 16 to 58 GB free. Wall times in `results.json` are
rough.

Results: [`docs/investigations/job49e-sa-wall/`](../investigations/job49e-sa-wall/README.md).

### The finding

1. **The default's buffer-layer error is real, and its mechanism is the one identified before the runs.**
   - Plate Cf(0.97) against TMR in `auto`: +3.1 / +4.9 / +7.7 / **+19.9** / **+19.7** / +14.2 / +11.4 / +5.7 / +4.3 /
     +2.9 % at wall y+ 1 / 2 / 3 / 5 / 8 / 12 / 15 / 30 / 40 / 60.
   - The 1-D model of the linear face eddy viscosity (D7), shifted by D01's offset (+2.2 points), predicts every one of
     them within 1.2 points (LH1).
   - On the NACA 0012 at 0 degrees, the mesh with median wall y+ 5 gives friction +17.4 % and cd +17.8 %. Median 15 gives
     +9.5 % and +9.3 %; median 42 (ADR 0164's F00) +2.5 % and +2.6 %.
   - Not the cause: the u_tau iteration, the wall-face viscosity and the wall nuTilda. The y+ 5 switch of `auto` is not
     the cause either: the error is the same on both sides of it.
2. **The opt-in `auto_sa_law`, as amended, removes it.**
   - Plate: **+2.3 % to +4.4 %** against TMR at all 10 y+, and within 2.0 % of its own y+ 1 value.
   - Aerofoil cd: **+2.0 %, +1.3 %, +2.3 %** at median wall y+ 5, 15, 42. The spread is 1.0 % of TMR, against 15.3 % for
     the default.
   - The remaining +2 % to +3 % is the plate set-up's baseline, already present at y+ 1 in every mode. It is not a wall
     treatment error.
3. **As registered, the opt-in missed at y+ 5 for two reasons that amendment 1 fixed.**
   - Its y+ 5 switch was bistable: E05's wall y+ was pinned at 4.97 to 5.05, and Cf was +7.5 %.
   - The second cell's strain came from a linearly interpolated face velocity, so its nuTilda was 1.14 to 1.28 times
     kappa u_tau y.
   - After the amendment: the wall y+ is monotone on every plate, the second-cell ratio is at most 1.065, and every F
     plate from y+ 5 up and AF15/AF40 meet their residual targets (F: 187 to 296 iterations).

### Limits

| Id | Limit | Measured | Outcome |
|---|---|---|---|
| L1 | 26 registered runs exit 0 or 5 | all; exit 5 (cap) for D01-D03, E01-E03, E05, AD05, AE05, AE40 | met |
| LR | D01, D05, D40, AD40 bit-identical to P01, P05, P40, F00 | 30 000 / 634 / 406 / 2088 values, 0 different, same iteration counts | met |
| T1 | inner law | passes | met |
| T2 | wiring | passes | met |
| LH1 | default vs 1-D model within 3 points | largest difference 1.19 points (y+ 5); the others -1.05 to +0.77 | met |
| LH2 | default max over y+ 5-15 >= +10 % | +19.9 % | met |
| LH3 | AD05 friction >= +8 % | +17.4 % | met |
| LP1 | E: +-5 % against TMR | +2.3 to +3.9 % except **E05 +7.49 %** | **missed**, `lp1_opt_in_against_tmr_known_miss` |
| LP2 | E: within 3 % of E01 | <= 1.5 % except **E05 +5.08 %** | **missed**, `lp2_opt_in_yplus_independence_known_miss` |
| LP3 | E: report vs law integral 2 % | <= 1.8 % (E03) | met |
| LP4 | E: mean C_F +-5 % of Kármán-Schoenherr | -0.1 to +2.4 % except **E05 +6.16 %** | **missed**, `lp4_opt_in_mean_friction_known_miss` |
| LA1 | AE cd +-6 % | +5.1 / +1.9 / +2.9 % | met |
| LA2 | AE friction +-6 % | +5.6 / +2.1 / +2.6 % | met |
| LA3 | AE spread <= 4 % | 3.2 % | met |
| L1_A1 | F, AF, G exit 0 or 5 | all; exit 5 for F01-F03 and AF05 (as their defaults) | met |
| LR_A1 | G05, G40, GA40 bit-identical (amended build) | 634 / 406 / 2088 values, 0 different | met |
| LP1_A1 | F: +-5 % against TMR | +2.3 / +3.2 / +4.4 / +4.2 / +3.1 / +2.6 / +2.5 / +2.4 / +2.4 / +2.4 % | met |
| LP2_A1 | F: within 3 % of F01 | <= 2.0 % (F03) | met |
| LP3_A1 | F: report vs law integral 2 % | <= 0.58 % | met |
| LP4_A1 | F: mean C_F +-5 % of K-S | -0.1 to +2.0 % | met |
| LA1_A1 | AF cd +-6 % | +2.0 / +1.3 / +2.3 % | met |
| LA2_A1 | AF friction +-6 % | +2.6 / +1.5 / +2.1 % | met |
| LA3_A1 | AF spread <= 4 % | 1.0 % | met |
| M1_A1 | F wall y+ non-increasing over x 0.8-1.2 | all 10 | met |
| M2_A1 | F05-F60 second-cell ratio <= 1.10 | 1.065 (F05) down to 0.918 (F60) | met |
| M3_A1 | F05-F60, AF15, AF40 exit 0 | all exit 0 | met |
| B1 | folder <= 250 000 bytes, PNG <= 100 000 | test `b1_storage` | met |
| P1 | selftest | passes | met |
| G0 | hygiene on the final code | see below | see below |

G0 in detail:
- fmt clean.
- clippy `-D warnings` of rcfd-turb and rcfd-api (lib and tests) clean.
- `baseline_hashes.rs` unchanged; the test gives 3 passed and 1 ignored, as before.
- `case_hashes_w48u`, `w33b_default_bits` and `workspace_graph` pass.
- `w48v_case_hashes` is a tool test, ignored as before.
- `flat_plate_sa`: FLAT_PLATE_SA_RESULT.

### Reported, no limit

- Plates against Schlichting's local law (0.002881): default -3.2 % (y+ 1) to +12.6 % (y+ 5); F -3.9 % to -2.0 %. TMR's
  SA value is itself 6.1 % below Schlichting.
- The molecular / wall-law ratio of F: 1.004 (y+ 1), 0.75 (15), 0.36 (40), 0.26 (60). The default `viscous` report needs
  the effective component from about y+ 5 up (ADR 0164).
- Opt-in minus default at y+ 1: -0.8 points (+2.3 % against +3.1 %). It comes from the interpolated-nuTilda faces of the
  resolved buffer layer; the 1-D model gave -1.6 points.
- Aerofoil pressure part against TMR's 0.001361: default +20.1 / +8.6 / +3.0 %, AF -0.8 / 0.0 / +3.3 % (y+ 5 / 15 / 42).
  The default's buffer-layer error also inflates the pressure drag, presumably through a thicker boundary layer.
- Wall-cell y+ of the meshes (median; 10-90 %; range):
  - M1w05: 5.4; 3.2-7.2; 0.8-7.5.
  - M1w15: 15.6; 10.0-20.7; 2.1-21.6.
  - M1w: 42.1; 26.7-56.6; 5.5-60.8.

  The number of wall cells that got a series-resistance face (`law_face_count`) was **not counted** on the aerofoil
  (deviation). Every wall cell of the O-grid has a radial neighbour, so all 256 are expected to qualify. The trailing-edge
  fan, where the grid lines turn, was not checked against the 0.5 cosine threshold.
- Steady stop: the y+ 1-3 plates and the M1w05 aerofoil run to the cap in every mode. Their targets are not met, though
  Cf and cd settle. The other plates and aerofoils meet their targets in 187 to 335 iterations, except E05 and AE40 (the
  registered switch).
- Wall times:
  - plates: 8 s (y+ 60) to 990 s (y+ 1, at the cap);
  - aerofoil: 66-110 s converged, 1700-2150 s at the cap.

### Deviations

1. **Amendment 1.** It corrects two defects of D1, found from the profile diagnostics of the registered runs. Its
   correction was designed after seeing E05 and E08, as the amendment states. The registered limits stay on the
   registered runs (three known misses). The amended arms have their own limits, with the same bands.
2. **PC restart (2026-10-30, about 00:30).** The host restarted without warning while F03 and AF05 were running.
   - F03 had reached iteration 13 492 of 15 000; AF05 iteration 2660 of 8000.
   - Their folders were moved to `E:\projects\cfd\w49e-work\w\killed-by-restart\`.
   - Both were rerun from scratch with the same amended binary (sha256 checked again). Both ended normally: exit 5 at the
     cap, like their defaults.
   - All other runs had finished before the restart.
3. **Driver changes after registration** (amendment 1): `plate_profile` (the M1 and M2 data), the generalised evaluation
   (the `_A1` keys), the F, AF and G arms, and the CSV line terminator. The amended `collect` reproduced the registered
   evaluation unchanged: every registered key compared equal before the amended runs started.
4. **Scratch diagnostics.** A scratch script read the profiles of the stored registered runs, which led to amendment 1.
   It is not committed; its logic is `plate_profile`. The interim `collect` calls wrote to a scratch folder and changed
   nothing.
5. **Before the registration** I looked at 1-D model variants, each piece alone, to choose D1. This is disclosed in
   "What I knew". Of Kalitzin et al. 2005 only the abstract was read.

### Not run

- The full workspace test suite and the gate; Linux.
- SST and k-epsilon in the buffer layer (question 3).
- SA with DES / DDES or the transient algorithms in mode `auto_sa_law`.
- Non-equilibrium checks of the opt-in: separation, a pressure-gradient case, angles other than 0 degrees.
- The 500-chord far field.
- Plate streamwise refinement; the +2 % to +3 % baseline is not attributed further.
- Skewed near-wall cells. The series-resistance face uses the wall-normal distance, and this is untested off wall-aligned
  layers.

### Questions for the maintainer (none is decided here)

1. **Default SA wall mode.**
   - (a) Keep `auto` as the default, with `auto_sa_law` opt-in (as now).
   - (b) Make `auto_sa_law` the default. That changes every SA result with wall cells above y+ about 1: buffer-layer
     meshes by up to 15 % in drag, M1w-like meshes by about 0.3 %.
   - (c) Drop it.

   **Recommendation (a) now, and (b) after one more job.** That job would check the opt-in in non-equilibrium flow: a
   separated or adverse-pressure-gradient case (e.g. the TMR bump or a backward-facing step) and an aerofoil angle sweep.
   The opt-in imposes the equilibrium inner layer on two cells at every y+, and nothing here tests that away from
   equilibrium.
2. **The `auto` documentation** ("works from y+ about 1 into the log layer", in the schema doc string and
   `docs/case-schema.md`).
   - (a) Correct it: "+20 % friction at wall y+ 5-8, +11 % at 15 (ADR 0168); use y+ <= 1 or >= 30, or `auto_sa_law`".
   - (b) Leave it.

   **Recommendation (a)**, a doc-only change with no effect on results.
3. **SST and k-epsilon.** They use the same linear face interpolation of nut (`effective_viscosity`). ADR 0063 already
   saw a buffer-layer fall for k-epsilon.
   - (a) A characterisation job: the same plate y+ sweep for SST `automatic` and for k-epsilon.
   - (b) Leave it.

   **Recommendation (a)**, with this job's driver and 1-D model as the template.
4. **The steady stop at y+ <= 3 and on M1w05.** These runs go to the cap in every mode, though Cf and cd settle.
   - (a) Leave it.
   - (b) Find which target (residual or turbulence change) blocks.

   **Recommendation (a)** for now; it is not a wall-treatment item.
