# ADR 0363: F-466 registration: the stop of DFG 2D-2 on T with the bounded second-order immersed forcing

Status: **REGISTERED 2026-11-12, written before any code of F-466 and before any run of its checks**. Not yet accepted by the
maintainer. Before this text nothing of F-466 was written, compiled or run: the documents and code under "What was read" were
read, and the arithmetic under "Arithmetic before this registration" was done by hand. Results go in an addendum; a defective
set-up, or a remedy chosen after the diagnosis, is registered by an amendment written and committed before its code and its
run; nothing above an amendment or the addendum is changed after a run.

Parents: [ADR 0268](0268-f354-registration-dfg-2d2-force-extrema-mesh-and-time-step-convergence.md) (F-354: the record R1,
T with `build_second_order_bounded`, stopped at step 2951; its question Q5, from which F-466 was made),
[ADR 0254](0254-f355-registration-immersed-second-order-forcing-in-transient-piso.md) (F-355: the bounded form),
[ADR 0234](0234-f309-registration-immersed-face-flux-at-the-surface-crossing.md) (F-309: the face flux at the crossing and the
volume source of the solid cells), [ADR 0218](0218-f219-registration-immersed-solid-with-modified-forcing.md) (the DFG set-up
and the T mesh), ADR 0067 (the divergence guard), ADR 0031 D3 (the restartable state `PimpleState`). Roadmap item F-466
(deps F-354, F-355).

Number: the lead gave 0363. Branch `f466-bounded-forcing-stop` from main f2190b72, worktree `w49i-work/f466`, scratch
`w49i-work/f466-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run; a missed limit becomes an ignored
`*_known_miss` test with the limit intact; each registered run is made once; defaults stay bit-identical (anything that does not
ask for the new capability produces the bytes it produced before); no hash table, golden or existing test file is edited
(`immersed_dfg_convergence_f354.rs` is not edited, its helpers are copied); no new third-party crate, no unsafe code;
unsigned commits without trailers; no push.

Clean-room: nothing is taken from vendor code or documentation. The algorithms examined are rcfd's own (preconditioned
conjugate gradients, Hestenes and Stiefel 1952, as written in `rcfd-linalg/src/solvers.rs`; smoothed-aggregation AMG, Vanek,
Mandel and Brezina 1996, as written in `rcfd-linalg/src/amg.rs`; both cited from the code's own headers, not from memory of
the papers). The reference values are the numbers printed by ADR 0268's record R1 (`f354-scratch/rec_r1_t_bounded.log`) and
the FeatFlow values already in `docs/reference-data.md` (as used by ADR 0268).

## What F-466 asks

The acceptance of `docs/roadmap.json`, binding:

> The stop of DFG 2D-2 on the T mesh with `build_second_order_bounded` (ADR 0268 R1: step 2951, pressure NaN in the pressure
> solve, inlet corner cell) is located by a registered measurement (the step reproduced from a restart, the linear solve and
> the source of the solid cells examined), and either the run reaches t = 8 and its max C_D and max C_L decide the decrease
> from T to T2 of ADR 0268 (A2-T-2), or the configuration is refused with a located message.

Readings (each a reading, not a decision of the maintainer; questions Q1 to Q3):

- **"located"**: the measurements M1 to M7 below give (a) whether the field was healthy one step before the NaN or was
  growing (class G), (b) the first operation that produces a non-finite number, named by function and quantity (and, for an
  instability, the step and cell where it starts), and (c) which of the inputs of the pressure solve (face coefficient,
  right-hand side, the volume source of the solid cells, start pressure) were finite.
- **"the step reproduced from a restart"**: a run started from a state written to disk shortly before step 2951 stops at the
  same step with the same message (M2).
- **"decide the decrease"**: with the run at t = 8, ADR 0268's clause A2-T-2 (`abs(e(T2)) < abs(e(T))` for max `C_D` and for
  max `C_L`, bounded forcing, T2 numbers of R3: +0.136 and +0.175 percent) is evaluated, met or missed; either outcome decides
  it. A miss is a result for F-354, not a failure of F-466.

## What was read and done before this text (disclosed)

Read: the F-466, F-354 and F-355 entries of `docs/roadmap.json`; ADR 0268 in full (set-up, R1, addendum, Q5);
`crates/rcfd-fv/tests/immersed_dfg_convergence_f354.rs` (the helpers `t_mesh`, `dfg_fields`, `forcing_of`, `run_2d2`,
`window`, `s2`); the R1 log and launch script in `w49i-work/f354-scratch`; of `crates/rcfd-fv/src/pimple.rs` the
`PimpleSolver` fields, `new`, `scratch`, the divergence guard (`trip`, `guard_end`, `Divergence::message`,
`first_nonfinite`), the momentum predictor's immersed terms, the pressure correctors (lines 1694 to 1927 of f2190b72) and
`snapshot`/`restore`/`PimpleState`; `ddt.rs` (`History`); `pressure.rs` (`solve_pressure_src`, `solve_pressure_inner`);
`fvmatrix.rs` (`solve_amg`); `rcfd-linalg/src/solvers.rs` (`cg`) and `amg.rs` in full; `immersed.rs` (`FluxFace`,
`build_impl`, `reconstruct_face_flux`, `flux_growth`).

Run: nothing. Free memory was read (24 GB of 96 GB free, CPU load 45 percent).

What the reading established (no run):

1. **`first_nonfinite` returns the lowest cell index.** The guard reports "cell 0" for any pressure vector whose first entry is
   not finite, in particular for a vector that is NaN everywhere. Conjugate gradients spread a non-finite scalar (`alpha`,
   `beta`) to every entry of `x` in one `axpy`. So "cell 0 at the inlet corner" is **not yet a location**: it is equally the
   signature of a NaN made in a scalar of the Krylov iteration. M4 decides it.
2. **The guard does not check the volume source of the solid cells.** `phiHbyA` is checked after `reconstruct_face_flux`, and
   the source is computed from the same finite `phiHbyA` and the finite rigid fluxes, so it is finite whenever `phiHbyA` is;
   M3 measures it rather than assuming it.
3. **The Courant limit of the guard is 1e6** (`DEFAULT_DIVERGENCE_COURANT_LIMIT`). A run can grow for hundreds of steps (Courant
   10, 1e3, 1e5) without tripping, until a number overflows. A dot product of a residual of size about `1e154` overflows to
   infinity and `inf/inf` is NaN. The R1 log printed only `t = 1` and `t = 2` (Courant 1.121, 1.138); nothing is known of
   steps 2001 to 2950. M1 decides between a sudden NaN in a healthy field and the end of a growth.
4. **There is no restart file in `rcfd-fv`.** `PimpleSolver::snapshot`/`restore` hold the whole restartable state in memory
   (fields, time, step counter, the three velocity histories and the flux history with their step sizes, ADR 0031 D3; every
   field of `PimpleState` and `History` is public); nothing else that `step` reads is left out (the step history is read only by
   the adaptive corrector control, unused here). `rcfd-api` has a case-level restart (`sim/restart.rs`) that does not apply to a
   test-built solver. F-466 therefore **writes the state to disk itself, from one run made once** (H1 below): a little-endian
   binary dump of `PimpleState` in the test file, read back by `restore`. Check S1 proves the round trip bit-exact.
5. **The internals of the pressure solve are local to `step_with`.** The face coefficient `gamma_j`, the pressure boundary
   data, the right-hand-side flux and the solid-cell source exist only inside the corrector loop; a test cannot rebuild them
   without re-implementing the momentum predictor. Examining "the linear solve" needs an opt-in capture in the library (D1).
6. **Possible non-finite operations with finite inputs**, read from the code: in `cg`, `alpha = rz/pq` with `pq` tiny but not
   zero (the guard is `pq == 0.0`), `beta = rz_new/rz`, and overflow of `dot` or `norm2`; in the AMG, Gauss-Seidel's division
   by a coarse diagonal, the damped-Jacobi prolongator's division by the filtered diagonal (guarded since ADR 0052), the power
   iteration's division by a norm, and the dense LU, whose pivot guard replaces a pivot below `1e-300` by `1e-300` (a singular
   coarsest operator then gives `x/1e-300`, an overflow). M5 and M6 locate the first one taken.

## Arithmetic before this registration (disclosed)

- **Cost.** R1 took 2695 s for 2951 steps of 53 136 cells with one thread on a loaded machine (0.91 s per step). H1 (the same
  2951 steps) is estimated at 0.5 to 1 h with four threads under load; H2 (51 steps from step 2900 plus the analysis) at 2 to
  5 minutes; a run to `t = 8` at 1.3 to 2.5 h.
- **Checkpoint size.** Per state: `u` 3 x 53 136, `p` 53 136, three velocity histories of up to three levels, the flux and its
  history (two or three levels of about 160 000 faces): about 10 MB. Twelve states: about 120 MB on the scratch disk.
- **Capture size.** Per pressure solve: the face coefficient and the flux (about 160 000 faces each), the source, the start and
  end pressures (53 136 each) and the boundary data: about 4 MB; two correctors per step, three steps: about 25 MB.
- **Healthy numbers.** The developed 2D-2 flow on T has a maximum Courant number of 1.12 to 1.14 at `dt = 1e-3` (R1 log) and
  `max |u|` near 2.3 (Courant times the cell size `0.0025` over `dt`, about 2.8 in the direction sum; the measured value of
  step 2000 is used as the healthy reference, so no number is assumed here). A growth that overflows a dot product within the
  3000 pressure iterations of a solve needs field values of about `1e150`, which is not reachable in one step from a healthy
  field: if the field is healthy at step 2950, the NaN is made inside the solve of step 2951.

## Design (D1 to D3)

**D1. Opt-in capture of the pressure systems (`rcfd-fv`, `pimple.rs`).** A public field `pressure_capture:
Option<Vec<PressureCapture>>` on `PimpleSolver`, `None` from `new` (and in the internal trial copy). When it is `Some`, every
pressure solve of the plain pressure path (`gradient_pressure = None`) pushes one `PressureCapture` with the step, outer
iteration and corrector, the face coefficient `gamma_j`, the pressure boundary data, the right-hand-side flux, the volume
source of the solid cells (`None` without flux faces), the controls, the start pressure, the solution returned by the solve
(before the guard looks at it), the initial residual and the iteration count. With `None` the solver executes exactly the
operations it executed before (one branch on an `Option`, no arithmetic), so every default result keeps its bits;
`baseline_hashes` and the existing immersed tests are run to show it.

**D2. The assembled system, from the outside (`rcfd-fv`, `pressure.rs`).** A public function `pressure_system_src(geom,
gamma, bc, phi, ctl, vol_src) -> Option<FvMatrix>` that assembles the matrix and right-hand side of the first pass of
`solve_pressure_inner` for the case of the capture (no non-orthogonal correctors, no periodic coupling, no gradient options;
`None` otherwise), by the same operations in the same order (divergence, minus the source, `coupled_gamma`, `laplacian` with no
gradient, minus the divergence, the reference-cell pin when there is no Dirichlet face). `solve_pressure_inner` is not changed.
Check S3 proves that solving it with `solve_amg` from the captured start pressure gives the captured solution bit for bit.

**D3. A read-only trace of the AMG cycle (`rcfd-linalg`, `amg.rs`).** Public methods on `Amg`: `level_sizes()`,
`level_min_diagonals()`, `coarse_pivots()` (the smallest and largest absolute pivot of the dense LU and the number of pivots
the `1e-300` guard replaced), and `apply_traced(r, z) -> Option<(usize, &'static str)>`, which performs the preconditioner
application with the same operations as `Preconditioner::apply` and returns the first level and stage (pre-smoothing,
restriction, coarse solve, prolongation, post-smoothing) after which a non-finite value appears. `cycle` and `apply` are not
changed; the new code only reads. Check S5 proves `apply_traced` equal to `apply` bit for bit.

The test file `crates/rcfd-fv/tests/immersed_bounded_stop_f466.rs` (new) holds: the copied DFG helpers; the checkpoint writer
and reader; a traced copy of `rcfd_linalg::cg` (the same operations, recording per iteration `pq`, `alpha`, `rz`, `beta`,
the residual norm and the finiteness of `x`, `r`, `z`); the fast checks S1 to S6; the heavy records H1 and H2.

## The runs

| Run | What | Cost |
| --- | --- | --- |
| H1 | R1 of ADR 0268 again (T, bounded, `dt = 1e-3`, the same helpers, from `t = 0`), with per-step records (Courant number, pressure iterations, initial pressure residual, `max |u|` with its cell, `max |p|`), states written at steps 2000, 2100, ..., 2900, 2940 and 2950 (twelve files), the per-step records written to a CSV; it runs until the guard stops it or to step 8000 | 0.5 to 1 h |
| H2 | a fresh solver from the state of step 2900 (file), steps to the stop with the capture on from step 2949; its per-step Courant numbers compared with H1's; the analysis M3 to M7 on the captured systems of steps 2949, 2950 and 2951 | minutes |

`RAYON_NUM_THREADS` 4 (the results do not depend on the thread count, `docs/run-control.md`). Each is run once, in release,
from a copy of the test executable built from the commit that adds the tests.

## Registered checks

### Set-up checks (fast; the gate runs them)

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| S1 | restart through a file: 2D-2 on M2 with the bounded forcing, 4 steps without a stop against 2 steps, the state written to a file, a new solver (new geometry, forcing, settings) restored from the file, 2 more steps | `u`, `p`, `phi`, time and step counter bit-equal (0 differing values) | `restore` holds the whole state (reading 4) |
| S2 | the capture does not change the run and records every solve: 3 steps on M2 with the capture on against off | `u`, `p`, `phi` bit-equal; 6 captures labelled (step 1 to 3, outer 0, corrector 0 and 1); every capture has the source (`Some`) | D1 |
| S3 | replay: for each capture of S2, `pressure_system_src` then `solve_amg` from the captured start pressure | the solution bit-equal to the captured one, the iteration count equal | D2 |
| S4 | the traced CG equals `rcfd_linalg::cg` with the AMG preconditioner on the system of step 3, corrector 1 of S2 | the solution bit-equal, the iteration count equal | the same operations |
| S5 | `Amg::apply_traced` equals `Preconditioner::apply` on a finite vector (the right-hand side of the same system); it reports `None` | bit-equal; `None` | D3 |
| S6 | the classifier G on given numbers (below): (Courant, `max |u|`/`u_2000`) = (1.2, 1.5): sudden; (1.2, 2.5): intermediate; (12, 1.0): growth; (1.2, 6): growth; (3, 2.5): intermediate | as stated | the definitions below |

### The measurements (heavy, H1 and H2)

| Id | Measurement | Limit or decision number | Estimate |
| --- | --- | --- | --- |
| M1 (reproduction from `t = 0`) | H1: `C_D`, `C_L` at `t = 1` and `t = 2`; the stop | equal to R1's printed values to their 5 digits (2.98321, -0.35990; 3.13571, -0.74178); stopped at step 2951 with R1's message text exactly | the same code on the path since 07d51775 (no library change between) |
| M2 (reproduction from a restart) | H2: the step and message of the stop; the Courant numbers of steps 2901 to 2950 | step 2951 and the same message; every Courant number bit-equal to H1's | S1 |
| M3 (inputs of the solve) | the captures of step 2951: the number of non-finite values in the face coefficient, the flux, the source, the start pressure | counted; **0 in all four means the NaN is made inside the solve** | readings 2 and 6 |
| M4 (extent) | the number of non-finite cells in the solution of the stopping solve | counted; **`n_cells` (53 136) means "cell 0" is the first index of an all-NaN vector, not a location** | reading 1 |
| M5 (the iteration) | the traced CG on the stopping system from its start pressure: the first iteration and scalar (`pq`, `alpha`, `rz`, `beta`, the residual norm) that is not finite, and whether the preconditioner output `z` was non-finite for a finite `r`; then `apply_traced` on that `r`: level and stage | located (named) | reading 6 |
| M6 (the preconditioner or the system) | the stopping system solved from its start pressure by incomplete-Cholesky CG (`FvMatrix::solve`, the same controls); the matrix: diagonal positive everywhere, off-diagonals non-positive, exactly symmetric; the smallest and largest face coefficient; `coarse_pivots` and `level_min_diagonals` of its AMG | IC-CG finite and converged or not; counts of violations | |
| M7 (the source of the solid cells) | for the captures of steps 2949, 2950, 2951: `max |src|`, the sum of `src` over the body, its cell | printed; the sum within `1e-12 max |src|` of zero (ADR 0234 D3: the body neither creates nor removes fluid) | the construction of `reconstruct_face_flux` |

**The class G** (from H1's records; `u_2000` is `max |u|` at step 2000, healthy by the R1 log):

- **sudden** if Courant(2950) `<= 1.5` and `max |u|`(2950) `<= 2 u_2000`: the field is healthy one step before the NaN;
- **growth** if Courant(2950) `>= 10` or `max |u|`(2950) `>= 5 u_2000`; then the **onset** is the first step after 2000 with a
  Courant number above 1.5, located by the cell of `max |u|` at that step (its distance from the cylinder centre, and whether it
  is the fluid cell or the image cell of a flux face);
- **intermediate** otherwise (reported with its numbers; it decides nothing).

1.5 is 1.3 times the developed 1.14 (a healthy run varies by a few percent over a period); 10 is far outside any healthy value.

### What follows from the measurements (decided now)

- **sudden, inputs finite (M3 = 0), IC-CG finite and converged (M6), AMG-CG NaN**: a defect of the AMG-preconditioned CG on
  that matrix, located by M5. The remedy is registered by Amendment 1 before its code: a change inside the linear solver that
  leaves every solve that does not produce a non-finite number bit-identical, or an opt-in; then the run to `t = 8` with it.
- **sudden, inputs finite, IC-CG also fails**: a defect of the system (M6 names the violated property); Amendment 1 registers a
  refusal with a located message (or a remedy) before its code.
- **inputs not finite**: the stop is upstream of the solve; M3 names the quantity; Amendment 1 as above.
- **growth**: an instability of the run, located by its onset; the linear solve is where it overflows. Amendment 1 registers a
  remedy or a located refusal before its code.
- **not reproduced** (M1 does not stop): H1 continues to step 8000, its window is evaluated as R1 would have been (S2 of ADR
  0268), A2-T-2 is decided by its numbers, and the stop is reported as not reproduced (the item is then partial: nothing is
  located).

A run to `t = 8` under Amendment 1 starts from `t = 0`, unless Amendment 1 registers a check that the remedy is bit-identical
to the default until the stop, in which case it starts from the state of step 2950 (written by H1) and that check is run
first. It must hold S2 of ADR 0268 (6 crossings, period and maximum spreads, amplitude, no stop) to decide A2-T-2.

### Negative controls and mutations (scratch, not committed)

S6 holds the cases that must be refused. Each mutation is applied alone to a scratch copy of the tree, the named fast test is run
once in release, and the copy is restored and compared byte for byte:

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | the checkpoint writer drops the `older` level of the velocity histories (writes "absent") | S1 |
| MU2 | `pressure_system_src` omits the subtraction of the volume source | S3 |
| MU3 | the capture stores the solution of the solve as the start pressure | S3 |
| MU4 | the capture is pushed only for corrector 0 | S2 |
| MU5 | the traced CG computes `beta = rz/rz_new` | S4 |
| MU6 | `apply_traced` skips the post-smoothing | S5 |
| MU7 | the classifier's sudden limit written as 15 in place of 1.5 | S6 |

The heavy records are not run under the mutations.

## Decision rule

- **done**: S1 to S6 held; M1 and M2 held (the stop reproduced from `t = 0` and from a restart); M3 to M7 measured and the
  first non-finite operation named (for a growth: the onset located); and either the run reaches `t = 8` under Amendment 1
  holding S2 of ADR 0268, so that A2-T-2 is decided, or the configuration is refused with a located message (registered in
  Amendment 1, with its own checks met); defaults bit-identical (`baseline_hashes`, the existing immersed tests); every
  mutation caught by the test it must fail, without a post-run amendment.
- **partial**: the stop located but neither branch reached; or a registered check missed (kept as `*_known_miss`); or a
  registered mutation not caught, or caught only after a post-run amendment; or the stop not reproduced.
- **todo** (unchanged): the stop neither reproduced nor located.

## Limits (not covered)

One mesh (T), one time step, two dimensions, one forcing; the measurements examine one stop, not the stability of the bounded
form in general. The capture is a diagnostic of the plain pressure path only (no gradient options, no non-orthogonal
correctors, no periodic coupling). The checkpoint file is a test helper, not a supported format (Q2). The capture and the AMG
trace are not exposed in the case schema.

## Open questions

- **Q1. Library code in a "measurement" item.** Options: (a) the opt-in capture, the assembly function and the read-only AMG
  trace (D1 to D3), defaults bit-identical; (b) no library code, the solve examined only through its result. Taken on the
  maintainer's standing instruction of 2026-11-11 (take the recommendation): (a); (b) cannot examine the linear solve, which the
  acceptance names.
- **Q2. The restart file.** Options: (a) a test helper in the F-466 test file; (b) a supported on-disk checkpoint of
  `PimpleSolver` in the library. Taken on the standing instruction: (a); (b) is a separate item if wanted.
- **Q3. "Decide the decrease"** read as "evaluate A2-T-2, met or missed". Options: (a) that reading; (b) F-466 done only if
  A2-T-2 is met. Taken on the standing instruction: (a); the acceptance says "decide".
