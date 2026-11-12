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

## Amendment 1 (2026-11-12, after H1 and H2, before any code or run of what it registers)

Written after the registered runs H1 and H2 were made once (their numbers go in the addendum, unchanged), before any code of
the remedy below. Nothing above this amendment is changed.

**What H1 and H2 found (the class and the branch).** H1 reproduced R1 (the printed coefficients and the message); the class
G is **sudden** (Courant 1.135993 at step 2950, `max |u|` ratio 0.9946). H2 restarted from the file of step 2900, stopped at
step 2951 with R1's message and the same Courant numbers. The stopping solve is corrector 1 of step 2951. Its inputs are
finite (M3: 0, 0, 0, 0). The whole solution is NaN (M4: 53 136 of 53 136 cells, so "cell 0" is the first index, not a
location). The matrix is a symmetric M-matrix (M6: no diagonal entry that is not positive, no positive off-diagonal, no
asymmetric pair). Incomplete-Cholesky CG solves the same system from the same start in 448 iterations to 2.5e-14. The traced
AMG-preconditioned CG (M5) has `r^T z < 0` from its first iteration (-2.0e-9), so in floating point the AMG preconditioner is
not positive definite on this system. The iteration stagnates and grows until `beta` is not finite at iteration 801
(overflow, `inf/inf`), and every later iterate is NaN. The preconditioner never returns a non-finite value for a finite
residual (M5: `None`). This is the registered branch "sudden, inputs finite, IC-CG finite and converged, AMG-CG NaN". The
NaN is made in `rcfd_linalg::cg`, in the scalar `beta = rz_new/rz` at iteration 801, after the AMG preconditioner lost
definiteness on the pressure matrix of step 2951.

**The remedy (R).** It is opt-in. A setting `PimpleSettings::pressure_ic_fallback: bool`, default `false`. When it is `true`
and a pressure solve on the plain path (`gradient_pressure = None`) returns a non-finite pressure, the solve is repeated from
the same start pressure with incomplete-Cholesky CG: the same controls with `amg = false`, which is `FvMatrix::solve`'s CG
with `Ic0` for the symmetric pressure matrix. A public counter on the solver records each repeat as (step, outer
iteration, corrector). If the repeat is also not finite, the guard stops the run as before. With `false`, or while every AMG
solve is finite, the solver does exactly what it did before. Derivation: CG needs an SPD preconditioner. IC(0) of a
symmetric M-matrix exists with positive pivots (Meijerink and van der Vorst 1977, cited from memory). Here it is not taken
from the paper but measured on the system itself (M6: converged). The repeat costs one more solve only where the AMG solve
has already failed. The capture of D1 keeps recording the failed AMG solve.

**Its checks (fixed now).**

| Id | Check | Limit |
| --- | --- | --- |
| S7 (fast) | M2, bounded, 3 steps with `pressure_ic_fallback = true` against `false` | `u`, `p`, `phi` bit-equal; no repeat recorded |
| H3a (ignored, seconds) | from the file of step 2950 (H1) with the option, one step | no stop; exactly one repeat, at (2951, 0, 1); the pressure finite |
| H3 (the run to `t = 8`) | from the file of step 2950 with the option, steps 2951 to 8000 (bit-identical to the default until step 2951: the option acts only on a non-finite pressure, and H1 had none before step 2951, because the guard would have stopped it) | no stop; S2 of ADR 0268 (at least 6 crossings, period spread `<= 1e-2`, maximum spread `<= 2e-2`, amplitude `>= 1.0`) held; then A2-T-2 evaluated: `abs(e(T2)) < abs(e(T))` for max `C_D` and for max `C_L` (T2 bounded +0.136, +0.175 percent), met or missed, either deciding |

The number of repeats in H3 and their steps are printed (no limit). A repeat count that grows quickly would say the AMG loses
definiteness often on T. That is a finding, not a failure of H3.

**Its mutations.**

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU8 | the repeat uses the AMG controls (`amg` left `true`) | H3a (run on the scratch copy with H1's files) |
| MU9 | the repeat condition `is_some()` written `is_none()` (the repeat taken after every finite solve) | S7 |

The decision rule above is unchanged. With H3 held, the branch "the run reaches `t = 8` ... so that A2-T-2 is decided" is
reached.

## Addendum 1: results (2026-11-12)

Measured against the limits registered above and in Amendment 1. No limit was changed. Every registered run was made once:
H1 from a copy of the test executable built at 40023d06 (`f466-scratch/f466_records.exe`), H2 at b386a8ac (a clippy-only
change to H2's analysis code, committed before H2 ran, which changed nothing it computes), and H3a and H3 at c96412f9. All
used `RAYON_NUM_THREADS` 4, release, Windows, on a machine shared with other agents. Logs are in `f466-scratch/h1.log`,
`h2.log`, `h3a.log` and `h3.log`. The states and the per-step CSV are in `f466-scratch/runs` (twelve states, 142 MB, written
once by H1).

Commits: registration 7ea3aa34; capture, assembly, AMG trace and tests 40023d06; clippy in H2 b386a8ac; Amendment 1 fd7b404a,
committed after H1 and H2 and before the code of the remedy; remedy and its tests c96412f9; this addendum.

### The stop, located

| Id | Check | Limit | Measured | Met |
| --- | --- | --- | --- | --- |
| S1 | restart through a file (M2, 2 + file + 2 steps against 4) | 0 differing values | 0; time and step counter equal | yes |
| S2 | capture neutral and complete | bit-equal; 6 labelled captures with the source | bit-equal; (1,0,0) ... (3,0,1); all `Some` | yes |
| S3 | replay of every capture | bit-equal, same iterations | bit-equal, same iterations | yes |
| S4 | traced CG = `cg` | bit-equal, same iterations | yes | yes |
| S5 | `apply_traced` = `apply`, `None` | bit-equal, `None` | yes | yes |
| S6 | classifier cases | as stated | as stated | yes |
| M1 | H1 from `t = 0`: R1's printed coefficients, the stop | 2.98321, -0.35990; 3.13571, -0.74178; step 2951, R1's message | 2.983214, -0.359903; 3.135711, -0.741778; step 2951, the same message character for character | yes |
| M2 | H2 from the file of step 2900: stop, Courant numbers 2901 to 2950 | step 2951, same message, all bit-equal | step 2951, same message, 0 differing | yes |
| G | class | sudden / growth / intermediate | **sudden**: Courant(2950) 1.135993, `max |u|` 2.164784 against 2.176629 at step 2000 (ratio 0.9946) | located |
| M3 | non-finite inputs of the stopping solve (step 2951, outer 0, corrector 1): face coefficient, flux, source, start pressure | counted | 0, 0, 0, 0 | located: the NaN is made inside the solve |
| M4 | non-finite cells of its solution | counted | 53 136 of 53 136 | located: "cell 0 at the inlet corner" is the first index of an all-NaN vector, not a place |
| M5 | traced CG on it | first non-finite named | `r^T z` is **negative from iteration 1** (-2.006e-9; then +1.2e-9, -1.8e-9, +4.7e-9, -2.1e-7: alternating signs, `beta` -0.36, -0.60, -1.51, -2.55, -45.2); residual stagnates at 2.5e-8 (start 2.5e-8, target 2.5e-14), grows, and **`beta` is first non-finite at iteration 801**; NaN to iteration 3000 (the cap); the preconditioner never returned a non-finite value for a finite residual | located |
| M6 | IC-CG on the same system; matrix properties; AMG | counted | IC-CG converged, 448 iterations, residual 2.5e-14, finite; diagonal not positive 0, positive off-diagonals 0, asymmetric pairs 0 (a symmetric M-matrix); face coefficient 1.0e-7 (solid) to 5.5e-4 (fluid); AMG levels 53 136, 10 745, 1653, 206, 24; coarse LU pivots 9.4e-10 to 2.3e-5, none guarded (step 2950: 1.8e-9 to 2.2e-5) | located |
| M7 | source of the solid cells, steps 2949 to 2951 | sum within 1e-12 of `max |src|` | `max |src|` 4.050e-6 to 4.057e-6, always at cell 20804 (0.17125, 0.16125); sum/max at most 1.9e-15 | yes |

Also seen: corrector 0 of step 2951 needed 406 AMG-CG iterations (every solve of steps 2949 and 2950 needed 10) and still
converged. Corrector 1 started from that answer, with an initial residual (2.5e-8) about 15 times smaller than the right-hand
side's largest entry (3.6e-7).

**Where the NaN is made.** In `rcfd_linalg::cg` with the AMG preconditioner (`FvMatrix::solve_amg`), in the pressure system of
corrector 1 of step 2951. The inputs are finite and the matrix is a symmetric M-matrix that incomplete-Cholesky CG solves. On
that right-hand side the AMG preconditioner gives `r^T M^-1 r < 0`, so CG has no positive-definite inner product and
breaks down. `beta = rz_new/rz` overflows at iteration 801, and the NaN fills every cell. The forcing, the stencil at the body
and the source of the solid cells are ruled out by M3 and M7: the source is finite and balanced, and it is the same size at
steps 2949, 2950 and 2951. So is the state of the flow, by class G and M2. The stop is a breakdown of the linear solver, not of
the bounded forcing. Why this AMG hierarchy loses definiteness here is **not** found: its coarse operators have positive
diagonals and unguarded pivots, and in exact arithmetic a Galerkin hierarchy with symmetric Gauss-Seidel smoothing is
positive definite. A loss of definiteness in floating point is likely, given the 5500:1 face-coefficient contrast at the
solid and a residual near the solver's floor, but it is not measured (question Q4).

### The run to t = 8 (Amendment 1)

| Id | Check | Limit | Measured | Met |
| --- | --- | --- | --- | --- |
| S7 | option on, 3 finite steps on M2 | bit-equal, no repeat | bit-equal, none | yes |
| H3a | from the state of step 2950, step 2951 with the option | no stop; one repeat at (2951, 0, 1); pressure finite | yes; `[(2951, 0, 1)]`; finite | yes |
| H3 | steps 2951 to 8000 with the option | no stop; S2 of ADR 0268 | no stop; 7 crossings, period spread 5.45e-6, maximum spread 1.86e-4, amplitude 2.041; **one repeat in 5050 steps** (step 2951 only) | yes |
| A2-T-2 drag | `abs(e(T2)) < abs(e(T))`, max `C_D`, bounded | strict | T 3.22695 (+0.216 %) -> T2 +0.136 % | **met** |
| A2-T-2 lift | the same, max `C_L` | strict | T 1.00253 (+1.687 %) -> T2 +0.175 % | **met** |

T with the bounded form at D/40: max `C_D` 3.22695 (+0.216 %), max `C_L` 1.00253 (+1.687 %), `St` 0.30144 (-0.146 %). The
forcing-order part P4 of ADR 0268 on T is then -0.599 point (drag) and -0.893 point (lift), the same sign and size as on M2
(-0.593, -1.172). ADR 0268's arithmetic item 3 estimated T bounded near +1.4 percent in the lift; +1.69 was measured. The run
from the state of step 2950 is the default run up to step 2950 bit for bit: the option acts only on a non-finite pressure, H1
had none before step 2951, and H3a/H3 start from H1's own state. Steps 2951 to 8000 took 2970 s.

### Mutations

Each was applied alone to a `git archive` copy (`f466-scratch/mutsrc`, separate target directory, `mutate.py`, `mutate2.py`).
The named test was run once in release, and the file was restored and compared byte for byte (equal every time):

| Id | Mutation | Must fail | Result |
| --- | --- | --- | --- |
| MU1 | writer drops `older` | S1 | failed: "S1: u, p, phi after the restart differ" |
| MU2 | assembly omits the source | S3 | failed: "S3: step 1 corrector 0: replay differs" |
| MU3 | capture stores the solution as `p0` | S3 | failed: the same |
| MU4 | capture only for corrector 0 | S2 | failed: "S2: captures" |
| MU5 | `beta = rz/rz_new` | S4 | failed: "S4: traced CG differs" |
| MU6 | traced cycle skips post-smoothing | S5 | failed: "S5: traced AMG differs" |
| MU7 | sudden limit 15 | S6 | failed (the classifier assertion) |
| MU8 | repeat with the AMG controls | H3a | failed: "H3a: stopped: ... pressure is NaN ... step 2951" |
| MU9 | repeat after every finite solve | S7 | failed: "S7: the option changed a finite run" |

### Tools and the defaults

- `cargo fmt -p rcfd-fv --check`, `cargo fmt -p rcfd-linalg --check`: clean. `cargo clippy -p rcfd-fv -p rcfd-linalg
  --all-targets -- -D warnings`: clean at c96412f9.
- `cargo test -p rcfd-api --release --test baseline_hashes`: 3 passed, 1 ignored, with the final library (c96412f9 code, the
  capture and the option in place). `cargo test --release -p rcfd-fv --lib`: 104 passed. `cargo test --release -p rcfd-linalg`:
  all passed (the AMG unit tests included).
- Existing tests on the lines changed, release: `divergence_guard` (3), `immersed_dfg_convergence_f354` (6),
  `immersed_second_order_f309` (9), `immersed_force_order_f337` (5) passed at c96412f9. `immersed_piso_stop_f355` (15),
  `immersed_solid_f219` (11) and `immersed_unsteady_force_f346` (9) passed with the library of 40023d06 (the capture without
  the option). They were not run again after c96412f9 because they take 15 to 40 minutes each. The option adds one test of
  a `false` flag on their path.
- `immersed_bounded_stop_f466`: 7 passed (S1 to S7, 1.2 s) and 4 ignored (H1, H2, H3a, H3).
- Library lines changed: `rcfd-fv/src/pimple.rs`, the `PressureBc` import, the setting `pressure_ic_fallback` and its
  default, the solver fields `pressure_capture` and `pressure_fallbacks` with `PressureCapture`, their initialisation in
  `new` and in the trial copy, and two blocks after the pressure solve (the capture and the repeat).
  `rcfd-fv/src/pressure.rs`: the new function `pressure_system_src`. `rcfd-linalg/src/amg.rs`: the new methods
  `level_sizes`, `level_min_diagonals`, `coarse_pivots`, `apply_traced` and the private `cycle_traced`. No existing
  function body changed except the corrector loop of `step_with`. The workspace suite and the gate were not run (the lead
  runs them).

### Deviations and disclosures

- The registration planned library code only for the capture, the assembly and the AMG trace (D1 to D3). The remedy is
  library code too (`PimpleSettings::pressure_ic_fallback`, `PimpleSolver::pressure_fallbacks`), registered by Amendment 1
  after the diagnosis and before its code, as the registered decision tree said it would be.
- H2's analysis code was changed after H1 ran (two clippy rewrites: `!(d > 0.0)` became `d.is_nan() || d <= 0.0`, and
  `filter().last()` became `rfind`). Both were committed before H2 ran and compute the same values.
- H3 started from H1's state of step 2950, not from `t = 0`, as Amendment 1 allowed and justified.
- The restart is a test helper (the file format of `immersed_bounded_stop_f466.rs`). rcfd-fv has no restart file of its own
  (registration reading 4).
- Not measured: why the AMG hierarchy is not positive definite on this system (Q4); whether other meshes or time steps
  hit the same breakdown.

### Decision by the registered rule

- S1 to S7 held. M1 and M2 held: the stop was reproduced from `t = 0` and from a restart.
- M3 to M7 located the stop. The NaN is made by AMG-preconditioned CG in corrector 1 of step 2951 (`r^T z < 0` from
  iteration 1, `beta` not finite at iteration 801), with finite inputs, a symmetric M-matrix and a finite balanced source of
  the solid cells.
- With the opt-in remedy of Amendment 1 the run reaches `t = 8` holding S2. A2-T-2 is decided, and met in both coefficients.
- The defaults are bit-identical (below). All nine mutations were caught by their registered tests, with no amendment after a
  run.

**Done** by the rule (pending acceptance). Read narrowly, the bounded forcing was never at fault. The default configuration
(`pressure_ic_fallback = false`) still stops on T at step 2951. The remedy is an opt-in, not a repair of the AMG.

### Remaining questions

- **Q1 to Q3** (as registered) were taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation):
  library diagnostics; the restart as a test helper; "decide" read as "evaluate".
- **Q4. The root cause in the AMG.** Options: (a) a follow-up that measures the definiteness of the hierarchy on this
  system (the Rayleigh quotient `r^T M^-1 r / r^T r` per level, the smoothed prolongator, the coarse operators in higher
  precision) and repairs it; (b) leave the opt-in as the answer. Recommendation (a): the breakdown can hit any AMG pressure
  solve with a strong coefficient contrast, not only immersed runs.
- **Q5. Make the repeat the default.** Options: (a) keep it opt-in; (b) turn it on by default. It changes no run whose solves
  are finite, but it changes runs that now stop with a NaN pressure, including any test that expects such a stop. That is a
  default behaviour change, so it is not taken here. Recommendation (b), after the gate shows no test expects an AMG NaN
  stop; it needs the maintainer.
- **Q6. A cheaper guard inside `cg`.** Options: (a) stop CG at the first `rz <= 0` and report it (one comparison per
  iteration, earlier and clearer than the NaN at iteration 801); (b) leave `cg` unchanged. Recommendation (a) as part of Q4.
  It changes the iterate of any solve that now passes through a non-positive `rz` and still converges, such as corrector 0
  of step 2951 with 406 iterations, so it needs the hash check.
- **Q7. The F-354 note.** A2-T-2 is now met (0.216 -> 0.136, 1.687 -> 0.175). Recommendation: record this in F-354's note.
  Whether F-354 becomes done depends on its own rule (A2-M-1 lift was accounted for by its second branch). Proposed below as
  done pending acceptance; the maintainer decides.

### For the lead: roadmap entries

**F-466: proposed status `done` (pending acceptance).** Proposed note:

> DONE pending acceptance (ADR 0363, 2026-11-12). From ADR 0268 question 5. The stop of DFG 2D-2 on T with build_second_order_bounded (dt 1e-3) was reproduced from t = 0 (the printed coefficients and the message of ADR 0268 R1 exactly) and from a state written to disk at step 2900 (the same stop, every Courant number bit-equal); the flow is healthy one step before (Courant 1.136, max |u| 0.995 of step 2000). Located by an opt-in capture of the pressure systems (PimpleSolver::pressure_capture, replayed bit for bit with pressure::pressure_system_src) and a read-only AMG trace: in corrector 1 of step 2951 the inputs are finite, the matrix is a symmetric M-matrix and the volume source of the solid cells is finite and balanced (sum 1e-15 of its maximum, the same size as at steps 2949 and 2950), but the AMG-preconditioned CG has r^T z < 0 from its first iteration, stagnates and overflows (beta not finite at iteration 801), so the whole pressure is NaN; cell 0 at the inlet corner is only the first index of an all-NaN vector. Incomplete-Cholesky CG solves the same system in 448 iterations. The stop is a breakdown of the linear solver, not of the forcing. Delivered (opt-in, defaults bit-identical, baseline_hashes pass): PimpleSettings::pressure_ic_fallback repeats a non-finite pressure solve with IC-CG from the same start and records it in PimpleSolver::pressure_fallbacks. With it T runs to t = 8 with one repeat in 5050 steps (S2 held, 7 crossings): max C_D 3.22695 (+0.216 percent), max C_L 1.00253 (+1.687 percent), St 0.30144 (-0.146 percent), so A2-T-2 of ADR 0268 is met (0.216 to 0.136, 1.687 to 0.175). Nine mutations caught. Open (ADR 0363 Q4 to Q6): why the AMG hierarchy loses definiteness on this system, making the repeat the default, a guard on rz <= 0 inside cg.

**F-354: no change of status proposed by this ADR (its rule is its own);** proposed addition to its note:

> ADR 0363 (2026-11-12, F-466): the stop of T with the bounded form was a breakdown of the AMG-preconditioned pressure CG at step 2951 (r^T z < 0, finite inputs); with the opt-in pressure_ic_fallback T runs to t = 8, max C_D +0.216 and max C_L +1.687 percent, so A2-T-2 is met (0.216 to 0.136, 1.687 to 0.175) and the forcing order on T moves the extrema by -0.60 and -0.89 point.

**F-355: no change of status;** proposed addition to its note:

> ADR 0363 (2026-11-12, F-466): the stop of build_second_order_bounded on T (ADR 0268) is not a stop of the forcing but of the AMG-preconditioned pressure CG (r^T z < 0 at step 2951); with the opt-in PimpleSettings::pressure_ic_fallback the run reaches t = 8.

**Proposed follow-up item** (if Q4 is answered (a)): *AMG pressure preconditioner: loss of definiteness on the immersed T
system.* Deps: F-466. Acceptance:

> On the pressure system of ADR 0363 (T, bounded immersed forcing, step 2951 corrector 1, rebuilt from the state of step 2950 and the opt-in capture) the cause of r^T M^-1 r < 0 of the smoothed-aggregation AMG is located by a registered measurement (per level, the Rayleigh quotient of the cycle, the smoothed prolongator and the coarse operators against a higher-precision product), and either the AMG is repaired so that AMG-CG converges on that system with every registered hash unchanged, or the breakdown is detected inside cg (rz <= 0) and reported with a located message.
