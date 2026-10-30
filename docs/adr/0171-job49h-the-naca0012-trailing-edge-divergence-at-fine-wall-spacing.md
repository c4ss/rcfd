# ADR 0171: Job 49H, the NACA 0012 trailing-edge divergence at fine wall spacing (ADR 0159 question 5, ADR 0164 queue item A1)

Status: **registered 2026-10-30**, before any solver run of this job. Phase 1 (reproduction) is registered here; later
phases (mechanism tests, a fix) are added as amendments, each written before the runs it covers. Results go in
addenda. Not yet accepted by the maintainer.

Job 49H, on local main f24b4f3 (worktree branch `job49h-te-divergence`). Number: 0170 is the highest on main; checked
just before writing.

House rules of [ADR 0130](0130-wave48a-overview-and-run-record-contract.md):
- limits are fixed here before the registered runs; a missed limit becomes an ignored `*_known_miss` test with the limit
  intact;
- a defective set-up gets an amendment before the corrected run;
- defaults of existing code stay bit-identical; `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded; new
  behaviour only behind an opt-in key;
- no new crate, no unsafe code, no new third-party crate, no commit trailers, nothing is pushed.

Clean-room: our own code and our own ADRs only; nothing from any vendor.

## The request (the maintainer, relayed)

The simulation must be stable and accurate whatever the resolution, with no hand-tuning. Find the cause of the
divergence at the trailing edge of the NACA 0012 O-grid at fine wall spacing (low y+), then fix it behind an opt-in key,
or characterise it if it cannot be fixed safely.

## What I knew when writing this (rule d)

From the ADRs; no solver run of this job:
- [ADR 0159](0159-job48y-naca0012-aoa-sweep-up-to-and-through-stall.md) amendment 1: on M1 (256 x 128, grading 3e5, first
  cell 1.574e-5 chord at mid-chord, y+ about 2, 77.6 degrees maximum non-orthogonality) every smoke run diverged within
  30 iterations: SA at 6 degrees at iteration 15 (velocity runaway, cell 0, a trailing-edge wall cell), SA at 16 degrees
  at 11 (cell 0), SA-DDES at step 8 (cell 255, the other trailing-edge wall cell); SST at 6 degrees at 16 (cell 0). M1
  without the `te_fan` weighting: SA at 6 at 24 (cell 255). A wall cell of 4.3e-5 chord (grading 1e5): SA at 6 at 26
  (cell 254). Wall cells of 1.3e-4 and 3.5e-4 chord: 300 iterations, no divergence. The example project's own mesh: none.
  The face angles of the two trailing-edge wall cells of M1 were 8.4 and 171.7 degrees, the aspect ratio in the thousands.
  Those runs used the far-field turbulence default of that day (zero gradient); the default is now `freestream`
  ([ADR 0166](0166-job49c-effective-wall-shear-and-sa-far-field-defaults.md)).
- [ADR 0168](0168-job49e-sa-wall-treatment-in-the-buffer-layer.md): M1w05 (grading 104430.79, first cell 4.160e-5 chord,
  wall-cell y+ median 5.4, range 0.8 to 7.5; 70.7 degrees, aspect 2426) at **0 degrees** ran its 8000 iterations without
  a guard stop in the default and opt-in wall modes (exit 5, the cap).
- So the data so far differ in two ways at once (angle 0 or 6, and first cell 4.2e-5 or 4.3e-5 with the older far-field
  default), and the angle may matter. Nothing says which part of the solver fails.

## Candidate mechanisms (not tested yet; Phase 2 will discriminate and is registered by an amendment)

- **G**, cell geometry: the two sliver cells next to the sharp trailing edge (face angles 8 and 172 degrees) make the
  least-squares or Green-Gauss gradient, the face interpolation or the pressure equation ill-conditioned there.
- **W**, wall treatment: the wall shear (`auto` wall law, viscous face) is evaluated on a cell where the wall normal
  distance and the wall tangent are ill-defined (the trailing-edge fan).
- **T**, turbulence source: the SA production, limiter or destruction on those cells, from a very large strain, drives
  nuTilda (and so the face viscosity) to a runaway.
- **C**, coupling: the SIMPLEC pressure correction (the `d` coefficients of tiny cells) with the stated relaxation.
- **L**, the linear-upwind limiter or gradient on the sliver cells.

## Set-up of Phase 1 (decisions are mine; none is a maintainer decision)

The cases are ADR 0159's text, unchanged (`w48y_aoa_sweep.case_text`: SIMPLEC, `convection = "linear_upwind"`, current
defaults: `wall_mode = "auto"`, far-field turbulence `freestream`, 50-chord far field, `[mesh.transform] rotate_z_deg =
-alpha`), with `max_iterations = min_iterations = 300` and `store_interval = 1` (scratch, so the fields just before a stop
can be read), one thread, `--status-every 1 --status-monitors`. The registry is `scripts/w49h_te_divergence.py`.

| Run | Mesh | Alpha (deg) | Model |
|---|---|---|---|
| RA | M1 (sha256 `5d5c8917...523f4`, rebuilt equal on this host) | 0 | SA |
| RB | M1 | 6 | SA |
| RC | M1w05 (`8e04edea...54451c`, ADR 0168) | 0 | SA |
| RD | M1w05 | 6 | SA |
| RE | M1 | 6 | SST |
| RF | M1w15 (`62b49fc8...b4aac`, wall-cell y+ median 15.6) | 6 | SA |

Binary: `rcfd-w49h0.exe`, built from main f24b4f3 (release). Scratch: `E:\projects\cfd\w49h-work`. At most 2 solver
processes at a time.

### Reading

- "Guard stop": the run ends with the steady divergence guard's message (`crates/rcfd-fv/src/simple.rs`); the iteration and
  cell are read from it. "Clean": 300 iterations, exit 5 with no guard message.
- The trailing-edge wall cells are cell 0 and cell 255 (ADR 0159 amendment 1; the wall ring starts at the trailing edge).
  I check this against the mesh before the limit is read (a defective claim would get an amendment first).

### Registered limits of Phase 1 (reproduction)

| Limit | Statement |
|---|---|
| LR1 | **Reproduction:** at least one of RA and RB ends in a guard stop before iteration 300 |
| LR2 | **Locus:** every guard stop of RA to RE is at a cell of the trailing-edge wall region (cells 0, 1, 254, 255, or a cell whose centre is within 0.02 chord of the trailing edge) |
| LR3 | **Consistent with ADR 0168:** RC is clean |
| LR4 | **Angle:** RD ends in a guard stop before iteration 300 (ADR 0159's grading-1e5 observation at 6 degrees) |
| LR5 | **Spacing:** RF is clean |
| LR6 | **Model:** RE ends in a guard stop before iteration 300 (ADR 0159 amendment 1 for SST) |

LR1 is the condition for the job to go on as a fix; if it is missed the divergence is not reproducible on main and the job
ends as a characterisation of what was run.

## Out of scope (stated now)

C-grid generator; the point-vortex far-field correction; DDES; wall-resolved meshes finer than M1 (the non-orthogonality
refusal at 85 degrees stops those at the mesh check); any change of a default.

## Questions for the maintainer

None yet; the questions are written in the addendum, with a recommendation each.

## Amendment 1 (2026-10-30, after the six Phase 1 runs and before any Phase 2 run): what Phase 1 showed, what I read in the repository, and the Phase 2 diagnostic runs

### What Phase 1 showed (binary `rcfd-w49h0.exe`, runs in `E:\projects\cfd\w49h-work\w\runs`)

| Run | End | Cell |
|---|---|---|
| RA (M1, 0 deg, SA) | clean, 300 iterations, exit 5 | |
| RB (M1, 6 deg, SA) | guard stop, velocity runaway, iteration 15 | 0, (0.99581, -0.07837), the trailing-edge tip of the rotated mesh |
| RC (M1w05, 0 deg, SA) | clean | |
| RD (M1w05, 6 deg, SA) | guard stop, velocity runaway, iteration 28 | 0, same position |
| RE (M1, 6 deg, SST) | guard stop, iteration 16 | 0 |
| RF (M1w15, 6 deg, SA) | clean | |

Limits LR1 to LR6: **all met** (the evaluation is in the addendum). RB reproduces ADR 0159's iteration 15 and cell 0 exactly.
Two things were not predicted: M1 at **0 degrees is clean** (the divergence needs the angle, on the same mesh), and
M1w05 at 6 degrees diverges (so wall cells of 4.2e-5 chord are not safe at 6 degrees; at 0 degrees they are, ADR 0168).

Fields read from the per-iteration store of RB (scratch script, not a registered measurement): the growth starts in the first
5 iterations in the four cells next to the trailing-edge tip (0, 255 across the seam, 1, 254) and in their radial
neighbours (256, 511); cell 0 and 255 carry velocities of order 1 to 6 by iteration 5 and 30 to 60 by iteration 8, growing
about two to four times per iteration, with pressures of opposite sign in neighbouring cells. SA's `nut` there stays at
1e-7 to 1e-6 until iteration 9, so the eddy viscosity follows the runaway; it does not start it. RA's same cells stay at
|u| < 0.01 after iteration 5, with u_y exactly mirrored.

Geometry of the trailing-edge cells of M1 (unrotated, scratch script on the polyMesh): the seam between cell 0 and cell 255 is
the wake ray y = 0; cell 0 is a trapezoid of tangential length 1.56e-4 and radial height 1.56e-5 with its centre 4.2e-6 from
the wall; the vertex at the tip has an interior angle of 171.8 degrees (the wedge angle of the trailing edge, 8.2 degrees
half angle). Face non-orthogonality of cell 0: 66.7 degrees (to cell 1), 73.0 degrees (to its radial neighbour 256), 0
(to cell 255, the seam). Cell 1: 29 and 47 degrees. So the largest non-orthogonality of M1 (77.6 degrees) sits in this
region, and it is a localised feature of a sharp-trailing-edge O-grid (it is why `te_fan` exists, ADR 0159 D2).

### What I read in the repository (before choosing Phase 2)

`docs/case-schema.md` "SIMPLEC on uniformly sheared meshes" (ADR 0081 part X, ADR 0084): the default SIMPLEC
(`alpha_u` 0.9, `alpha_p` 1.0, `velocity_correction = "gradient"`) has a **period-two pressure mode** whose eigenvalue passes
-1 at 57.5 to 60 degrees of non-orthogonality (65 to 70 at `alpha_p` 0.7); on a channel the converged state is stable and the
instability is that of the start-up states. Cause (ADR 0084, linearisation): the cell velocity is corrected with the full
Green-Gauss gradient of the pressure change while the face flux is corrected with its orthogonal part only. Existing
opt-ins: `solver.velocity_correction = "face_flux"` (stable to 79 degrees on the sheared channel and box, "not tested with
turbulence models, moving meshes or face jumps"), a lower `solver.alpha_p`, `solver.non_orthogonal_correctors` with
`solver.simplec_non_orthogonal_compensation`. The wall cells at the trailing edge of M1 (73 degrees) are above that
threshold, and the angle of attack changes the start-up state (the pressure difference across the wake ray), which would
explain why 0 degrees survives. This is **a hypothesis (H-084)**, written before the Phase 2 runs; the candidates G, W,
T, C and L above remain open.

### Phase 2 set-up: one change against RB per run (M1, 6 degrees, SA, 300 iterations; the registry in `scripts/w49h_te_divergence.py`)

| Run | Change against RB | Tests |
|---|---|---|
| PA | `solver.velocity_correction = "face_flux"` | H-084 (the ADR 0084 mode) |
| PB | `solver.alpha_p = 0.3` | H-084 (lower pressure relaxation) |
| PI | `solver.alpha_p = 0.7` | the dependence on `alpha_p` (report only) |
| PH | `solver.alpha_u = 0.7` | the dependence on `alpha_u` (report only) |
| PC | `non_orthogonal_correctors = 2`, `simplec_non_orthogonal_compensation = true` | ADR 0084's table says this is stable to 70 degrees only (report; predicted to diverge) |
| PD | `algorithm = "simple"` (its defaults 0.7 / 0.3) | SIMPLE against SIMPLEC |
| PE | `turbulence.model = "laminar"` | T (turbulence source) |
| PF | `convection = "upwind"` | L (limiter / gradient of the linear-upwind scheme) |
| PG | mesh not rotated; free stream and initial velocity (cos 6, sin 6, 0) | the mesh transform (ADR 0155) |

### Phase 2 limits (fixed before the runs)

| Limit | Statement |
|---|---|
| LM1 | **H-084 supported by PA:** PA is clean for 300 iterations (exit 5, no guard stop) |
| LM2 | **H-084 supported by PB:** PB is clean for 300 iterations |
| LM3 | **Not turbulence:** PE ends in a guard stop in a trailing-edge cell (0, 1, 254, 255, 256 or 511) between iteration 10 and 20 |
| LM4 | **Not the convection scheme:** PF ends in a guard stop in a trailing-edge cell (as LM3's cells) before iteration 300 |
| LM5 | **Not the mesh transform:** PG ends in a guard stop in a cell within 0.001 chord of the trailing edge between iteration 10 and 20 |
| LM6 | **SIMPLE:** PD is clean for 300 iterations |

PI, PH, PC are reported without a limit (they set the shape of the dependence). If LM1 and LM2 are both met, Phase 3 is the
fix, registered by Amendment 2: the question is then whether the default can move (it cannot, house rules) so which opt-in
is made available automatically and safely, and what "no hand-tuning" asks of it. If either is missed, H-084 is not the whole
mechanism and the next amendment tests G, W, T, C and L by direct probes.

Runs PA to PI: at most 2 solver processes at a time, the same binary `rcfd-w49h0.exe`.

## Amendment 2 (2026-10-30, after Phase 2 and before any Phase 3 run): H-084 rejected; first-order upwind is clean; Phase 3 tests the linear-upwind extrapolation

### Phase 2 results (the full evaluation is in the addendum)

| Run | End |
|---|---|
| PA face_flux | guard stop, iteration 18, cell 255 |
| PB alpha_p 0.3 | guard stop, iteration 25, cell 0 |
| PI alpha_p 0.7 | guard stop, iteration 17, cell 0 |
| PH alpha_u 0.7 | guard stop, iteration 19, cell 1 |
| PC two correctors + compensation | guard stop, iteration 15, cell 254 |
| PD SIMPLE (0.7 / 0.3) | guard stop, iteration 12, cell 255 |
| PE laminar | guard stop, iteration 15, cell 0 |
| PG flow rotated, mesh not | guard stop, iteration 15, cell 0 (the same iteration as RB) |
| PF `convection = "upwind"` | **clean, 300 iterations** |

Limits: LM1 (PA clean) **missed**, LM2 (PB clean) **missed**, LM6 (PD clean) **missed**, LM4 (PF stops) **missed** (PF is
clean, the opposite of the prediction), LM3 (PE stops at the trailing edge between 10 and 20) met, LM5 (PG) met. So the
ADR 0084 period-two mode (H-084) is **not** the mechanism: the pressure-velocity coupling variants (SIMPLE, SIMPLEC with
three relaxation sets, `face_flux`, correctors) all diverge, turbulence is not needed (laminar diverges at the same
iteration), and the mesh transform is not involved. Only the **convection scheme** changes the outcome.

### Offline look at the extrapolation (scratch script on the stored RB fields, not a registered measurement)

`linear_upwind` extrapolates the cell value to a face with the inverse-distance-weighted least-squares gradient
(`least_squares_partner`, `crates/rcfd-fv/src/gradient.rs`), unlimited by default. For cell 0 of RB the least-squares matrix is
well conditioned (eigenvalue ratio 2.2), so the gradient is not wrong because of conditioning; but the velocity profile
between the no-slip wall (4.2e-6 chord from the cell centre) and the radial neighbour is steep, the stencil is skewed
(73 degrees), and the tangential derivative the fit returns is of order 1e4 to 1e5 per chord. At iteration 1 the face
increments `grad . (x_f - x_c)` of cell 0 are +3.6, -4.8 and -0.9 against a cell value of 0.56 (faces then carry values
of -4.2 to 4.1, outside the neighbour range); at iteration 5 they are 14, -19 and -3 against 2.8. The same cell pair
(0, 255) and cells 1 and 254 grow together. This is **H-LU:** the unlimited linear-upwind extrapolation on the sliver wall
cells of the trailing edge produces face values far outside the range of the neighbouring cells, and the deferred
correction of the convection term feeds that back into the iteration, in every coupling variant, and only when the flow
puts a strong velocity difference on those cells (6 degrees; at 0 degrees the symmetric start-up does not).

### Phase 3 set-up: the limiter that already exists (`solver.linear_upwind_limiter = "barth_jespersen"`, ADR 0081 part L, "measured stable on the FAN-01 sector mesh where the unlimited scheme diverges")

One change against the Phase 1 runs (same binary `rcfd-w49h0.exe`, 300 iterations, `store_interval = 1`):

| Run | Phase 1 counterpart | Mesh, alpha, model |
|---|---|---|
| QA | RB (guard stop at 15) | M1, 6, SA |
| QB | ADR 0159's SA at 16 (stop at 11) | M1, 16, SA |
| QC | RE (stop at 16) | M1, 6, SST |
| QD | RD (stop at 28) | M1w05, 6, SA |
| QE | RA (clean) | M1, 0, SA |

| Limit | Statement |
|---|---|
| LN1 | QA is clean (300 iterations, exit 5, no guard stop) |
| LN2 | QB is clean |
| LN3 | QC is clean |
| LN4 | QD is clean |
| LN5 | QE is clean (the limiter does not break the case that already ran) |

If LN1 to LN4 are met the unlimited extrapolation is confirmed as the trigger (H-LU) at the level these runs can show; the
fix question is then not "a new scheme" but whether and how a limiter becomes available without hand-tuning, and what
it costs in accuracy and convergence over a full run. That is Amendment 3, written after these runs: long runs
(8000 iterations) on M1 at 0 and 6 degrees against TMR's 0.008192 and the 49D curve, and the no-opt-in default's
behaviour, are not run before then.

## Amendment 3 (2026-10-30, after QA to QD and before any Phase 4 run): the limiter removes the divergence in four runs; full-length runs on M1

### Phase 3 so far

QA (M1, 6 degrees, SA), QB (M1, 16 degrees, SA), QC (M1, 6 degrees, SST) and QD (M1w05, 6 degrees, SA), each with
`solver.linear_upwind_limiter = "barth_jespersen"`, ran their 300 iterations clean (exit 5 at the cap, no guard stop).
LN1 to LN4 are **met**. QE (M1, 0 degrees, BJ) was still running when this was written; LN5 is read from it, not from here.
Together with PF (first-order upwind clean) the unlimited second-order extrapolation is the trigger: H-LU is supported at
the level of these runs. It is not yet shown that the limited scheme converges to a usable answer over a full run.

### Phase 4 set-up: three 8000-iteration runs on M1 (same binary, one thread, `min_iterations` = the cap, last snapshot only, ADR 0164's extra reports and `nut_avg`)

| Run | Alpha | Change against the default |
|---|---|---|
| LA0 | 0 | none (the default: this is the case that is clean for 300 iterations, RA, and shows the unlimited scheme's full-length answer on M1) |
| LB0 | 0 | `linear_upwind_limiter = "barth_jespersen"` |
| LB6 | 6 | `linear_upwind_limiter = "barth_jespersen"` |

LB6 without the limiter is not run (it diverges: RB). Reading follows ADR 0159 D4 / ADR 0164 amendment 1: window means over
the last 1000 iterations of `cl`, `cd` and the effective reports (`cd_eff`, `cd_veff`, `cd_p`, `cl_eff`); TMR's
NACA 0012 SA values at 0 degrees: total 0.008192, friction 0.006831, pressure 0.001361 (ADR 0164); Ladson (80 grit,
interpolated, ADR 0167/0159) at 6 degrees: cl 0.645, cd 0.0088; 49D's SA curve on M1w at 500 chords at 6 degrees:
cl_eff 0.654, cd_eff 0.00988 (a different wall spacing and far field: a comparison, not a reference).

| Limit | Statement |
|---|---|
| LL1 | LB0 and LB6 end with exit 0 or 5 (no guard stop, no wall-clock stop) |
| LL2 | **Accuracy at 0 degrees:** LB0 `cd_eff` within +-6 % of 0.008192 (the band of ADR 0168 LA1) |
| LL3 | LB0 `cd_veff` within +-6 % of 0.006831 |
| LL4 | **The limiter's cost:** LB0 `cd_eff` and LA0 `cd_eff` differ by at most 3 % |
| LL5 | **Accuracy at 6 degrees:** LB6 `cl_eff` within +-5 % of Ladson's 0.645 |
| LL6 | LB6 steady by ADR 0159 D4: over the last 1000 iterations cl's max - min <= 0.005 and cd's max - min <= 0.0005 |
| LL7 | LB6 `cd_eff` within +-10 % of 49D's 0.00988 |
| LL8 | LA0 ends with exit 0 or 5 (reported: whether the default is stable at 0 degrees over a full run on M1) |
| LL9 | QE (300 iterations, BJ, M1, 0 degrees) is clean |

LL4 and LL8 are what separate "the limiter hurts" from "the mesh at y+ 2 is not as accurate as TMR's". No change of
code is made before these results; an opt-in key or a default is Amendment 4's subject (or the addendum's question).

## Addendum: results

Data: `docs/investigations/job49h-te-divergence/results.json` and `results.csv`, written by `scripts/w49h_te_divergence.py collect`;
limits are tested in `crates/rcfd-api/tests/te_divergence_w49h.rs`. No solver code changed in this job: the opt-in key that removes the
divergence already exists (`solver.linear_upwind_limiter = "barth_jespersen"`, ADR 0081 part L).

### Limits

Met: LR1 to LR6, LM3, LM5, LN1 to LN5, LL1, LL3, LL4, LL5, LL6, LL8, LL9.
Missed (ignored `*_known_miss` tests, limits intact): LM1, LM2, LM4, LM6 (the H-084 predictions, Amendment 1), **LL2** (LB0 `cd_eff` 0.008776, +7.1 %
against TMR 0.008192, limit 6 %), **LL7** (LB6 `cd_eff` 0.011140, +12.7 % against 49D's 0.00988, limit 10 %).
LL4 is met by a margin of 0.01 % (+2.99 % against 3 %); it is not a robust pass.

### Findings

1. **Reproduced.** M1 (first cell 1.57e-5 chord) at 6 degrees hits the guard at iteration 15 in the trailing-edge tip cell 0 (SA; SST at 16); M1w05 at 6
   degrees at 28. M1 and M1w05 at 0 degrees, and M1w15 at 6 degrees, are clean. So the divergence needs fine wall spacing and an angle of attack.
2. **Not the pressure-velocity coupling, the relaxation, the turbulence model or the mesh transform.** `face_flux` (PA), `alpha_p` 0.3 and 0.7 (PB, PI),
   SIMPLE (PD), `alpha_u` 0.7 (PH), two non-orthogonal correctors (PC), laminar (PE) and the unrotated mesh with a rotated free stream (PG) all stop at
   the trailing-edge cells within 12 to 28 iterations. H-084 (the ADR 0084 period-two mode) is rejected.
3. **Mechanism (H-LU), supported but not proved by a registered run.** The unlimited `linear_upwind` face value `u_c + grad u . (x_f - x_c)` over the
   long side of the 171.8 degree tip cell (aspect 1e1 to 1e2, face non-orthogonality 67 to 73 degrees; aspect times tan(non-orthogonality) about 21 000
   on M1, 6 900 on M1w05, 1 600 on M1w15) lies far outside the neighbour range (offline, iteration 1: face increments +3.6, -4.8, -0.9 against a cell
   value of 0.56; iteration 5: 14, -19, -3 against 2.8), and cells 0, 255, 1, 254 grow together by 2 to 4 times per iteration. This is the mechanism ADR
   0077 part B documents for the FAN-01 sector mesh; the new fact is that it also arises on a standard O-grid airfoil at y+ about 2 once the flow is
   off-axis. The offline analysis and the aspect times tan metric were not registered; the metric alone does not predict divergence (M1 at 0 degrees has
   the same metric and is clean).
4. **Discriminating evidence.** `barth_jespersen` (it limits only the convection gradient; the Laplacian correction keeps the unlimited gradient) makes
   all five 300-iteration runs clean: M1 at 6 and 16 degrees, SST at 6, M1w05 at 6, M1 at 0. **Correction to Amendment 2:** PF (`convection =
   upwind`, clean) is confounded. With neither a gradient-based convection nor non-orthogonal correctors the gradient is not computed at all, so PF
   also drops the Laplacian's non-orthogonal correction; PF alone cannot say that the convection extrapolation is the cause. QA is the run that
   discriminates, because it changes only the convection gradient.
5. **Full length (8000 iterations, M1, SA, window means over the last 1000).**

   | Run | cl_eff | cd_eff | cd_p | cd_veff | State |
   |---|---|---|---|---|---|
   | LA0 default, 0 deg | 2e-7 | 0.008521 (+4.0 % TMR) | 0.001390 (+2.2 %) | 0.007131 (+4.4 %) | steady, exit 5 |
   | LB0 BJ, 0 deg | 5e-9 | 0.008776 (+7.1 %) | 0.001632 (+20 %) | 0.007143 (+4.6 %) | steady, exit 5 |
   | LB6 BJ, 6 deg | 0.64425 (Ladson -0.1 %) | 0.011140 (+12.7 % of 49D; +26.6 % of Ladson 0.0088) | 0.004218 | 0.006922 | steady, exit 5 |

   The limiter costs +3.0 % of `cd_eff` at 0 degrees, all in the pressure drag (+17 %, 0.00024), with the friction drag unchanged (+0.2 %). The default
   (unlimited) scheme is stable at 0 degrees for the full run and 4 % high; at 6 degrees the only answer available on M1 is the limited one. Lift at 6
   degrees is accurate; the total drag at 6 degrees is high by an excess in the pressure drag (0.00422 against 49D's 0.00323 on a different mesh and a
   500-chord far field). How much of that excess is the limiter (the 0-degree cost alone is 0.00024 of 0.00099), the 50-chord far field or M1's wall
   spacing is not separated by any run here.

### Scope of the claims

Stability with the limiter: 300-iteration runs only for 16 degrees (QB), SST (QC) and M1w05 (QD); full length only for SA at 0 and 6 degrees on M1. The
unlimited scheme's divergence is shown for three meshes and one family of grids. Nothing here says that the limiter removes the divergence on other
meshes, other turbulence models at full length, or transonic flow.

### Deviations

- **Scratch wiped mid-job.** `E:\projects\cfd\w49h-work` was deleted (cause unknown) between Phase 4 starting and finishing: binary, target, meshes,
  cases, logs and all Phase 1 to 3 run folders. LA0 and LB6 had finished; only their `output/monitors.csv` and `wall.json` survived (kept in
  `w49h-work/keep`), so their rows are read through the `monitors.csv` fallback in `scripts/w49h_te_divergence.py` (the same numbers as the status
  stream would give: the monitors are written every iteration). LB0 had not run. The binary was rebuilt as `rcfd-w49h1.exe` (solver code identical to
  f24b4f3; only tests, scripts and docs differ), the three meshes were rebuilt and their hashes match this ADR, and all 20 short runs and LB0 were rerun
  from scratch. The 20 reruns reproduce the first set's exit codes; the first 15 also reproduced stop iterations and cells exactly. The committed
  `results.json` comes from the rerun set.
- The offline gradient analysis, the cell-0 geometry study and the aspect times tan metric were not registered; they are evidence for H-LU and say nothing
  that a limit depends on.
- Amendment 1's H-084 prediction was wrong; the PF reading in Amendment 2 was confounded (finding 4).
- LL4 passes by 0.01 %.

### Questions for the maintainer

1. Make `barth_jespersen` the steady `linear_upwind` default? Recommend no for now: it costs 3 % of `cd_eff` at 0 degrees, LL2 and LL7 miss, and the
   default is bit-identical by house rule. Keep it opt-in and document it (done in `docs/case-schema.md`).
2. A static set-up warning for unlimited `linear_upwind` on steady cases with wall cells of high aspect times tan(non-orthogonality)? Recommend yes, as
   queue item A2, as a warning not an error, because the metric alone does not predict divergence (M1 at 0 degrees is clean).
3. The guard's message hint says to lower the relaxation factors; here that is wrong (PB, PH, PI). Recommend a hint that names the limiter when the
   locus is a wall cell with `linear_upwind`.
4. Is a C-grid generator (ADR 0159 Q2) wanted as the mesh-side alternative? Recommend yes later: it removes the tip cell's 172 degree wedge, so no
   solver compromise is needed.
5. This branch is on pre-rewrite history (f24b4f3); it needs porting after the history rewrite.
6. `w49h-work` was deleted while a job was running. Was that deliberate?
