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
