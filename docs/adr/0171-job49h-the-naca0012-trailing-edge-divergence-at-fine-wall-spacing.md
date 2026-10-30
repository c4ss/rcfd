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
