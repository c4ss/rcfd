# ADR 0392: F-482 registration: the single-corrector mode of the immersed bounded forcing by a linearisation of the whole step

Status: **REGISTERED 2026-11-13, written before any code of F-482 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-482 was written, compiled or run: the documents and code under "What was read" were
read, and the arithmetic under "Arithmetic before this registration" was done by hand from numbers already in the repository's
records. Results go in the addendum at the end; a defective set-up is corrected by an amendment written and committed before the
corrected run; nothing above an amendment or the addendum is changed after a run.

Parents: [ADR 0374](0374-f363-registration-immersed-flux-with-one-piso-corrector.md) (F-363: the set-up R, the forcings F and
B, the reproduction R0 of the stop at step 34, the record G1 of the alternating growth, G2 (B runs at `dt = 5e-4`), L3 (runs at
`2.5e-4`), question Q3 from which F-482 was made), [ADR 0388](0388-f483-registration-pressure-lag-estimate-at-an-immersed-body.md)
(F-483: the frozen pressure-lag map converges to 1.175 (F) and 1.248 (B) at step 2 and is below one by step 11 for both, so it
does not predict the stop; ruled out as the predictor in its Q3), ADR 0031 D3 (`PimpleSolver::snapshot` / `restore`, the
restartable state with the time-level history). Roadmap item F-482 (deps F-363). Branch `f482-immersed-step-linearisation` from
main 2570d5e4, worktree `w49i-work/f482`, scratch `w49i-work/f482-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run; a missed limit becomes an ignored
`*_known_miss` test with the limit intact and the number recorded; each registered run is made once; defaults stay
bit-identical; no hash table, golden or existing test file is edited (the DFG helpers of `immersed_single_corrector_f363.rs` are
copied, that file is not edited); no new third-party crate, no unsafe code; unsigned commits without trailers; no push.

Clean-room: the step is rcfd's own PISO (Issa, J. Comput. Phys. 62 (1986) 40-65, as written in `pimple.rs` and its ADRs; cited
from memory, nothing of it used here). The linearisation is first principles: a central finite difference of a differentiable
map, `J v = (S(x + e v) - S(x - e v)) / (2 e) + O(e^2)`. The Arnoldi process (an orthonormal basis of the Krylov space
`K_m(J, v_1)` by Gram-Schmidt, the projection `H_m = V_m^T J V_m` upper Hessenberg, its eigenvalues the Ritz values, the
residual of a Ritz pair `|h_{m+1,m}| |e_m^T y|`), the shifted QR iteration for the eigenvalues of a Hessenberg matrix (Givens
rotations, a Wilkinson shift, deflation at a negligible subdiagonal) and inverse iteration for an eigenvector are textbook
linear algebra (Golub and Van Loan, Matrix Computations; Saad, Numerical Methods for Large Eigenvalue Problems; both cited from
memory, the algorithms written from their definitions and checked here on matrices with known spectra, U1 and U2). No vendor
code, text, manual or binary.

## What F-482 asks

Acceptance (`docs/roadmap.json`), binding, clause by clause:

> The period-two growth of DFG 2D-2 on M2 with build_second_order_bounded and one PISO corrector (ADR 0374: stop at step 34 at
> dt 1e-3, runs at 5e-4 and 2.5e-4) is measured by a registered finite-difference linearisation of the whole step (velocity and
> pressure) about the state of an early step, whose dominant eigenvalue is real and below -1 at dt 1e-3 and of magnitude below 1
> at dt 5e-4 and with two correctors, with its mode at the first fluid cells; or the part of the step that carries the growth is
> identified by a registered measurement.

1. *a registered finite-difference linearisation of the whole step (velocity and pressure) about the state of an early step*
   (D1, D2; the linearity check E0 and the identity check U3);
2. *dominant eigenvalue real and below -1 at dt 1e-3* (E1 a to c);
3. *of magnitude below 1 at dt 5e-4* (E2) *and with two correctors* (E3);
4. *its mode at the first fluid cells* (E1 d);
5. *or the part of the step that carries the growth identified*: the fallback. This registration takes the first branch; the
   shares of the mode in the parts of the state (D3) are printed as the located description that a miss of the first branch
   would leave, not as a registered second branch.

**What "dominant" means here (taken on the maintainer's standing instruction of 2026-11-11, take the recommendation).** Any
consistent step map of a flow has eigenvalues `exp(sigma dt)` near `+1` for its slow physical modes (advection, the developing
wake), with magnitude at or slightly above one whenever the flow itself is unsteady; those are not the period-two mode and
cannot be "below 1 in magnitude" by any scheme. So the eigenvalue the acceptance speaks of is taken as **the dominant eigenvalue
of the left half plane** (`Re lambda < 0`, the family that alternates in sign from step to step): at `dt = 1e-3` it must be real,
below -1, and in addition (E1 c) of the largest magnitude of all Ritz values (dominant outright); at `5e-4` and with two
correctors no converged Ritz value of the left half plane may have magnitude 1 or more. The overall dominant Ritz value is
printed in every case.

## What was read (disclosed)

ADR 0374 in full (registration, amendments 1 and 2, addendum 1); ADR 0388 in full (registration, amendment 1, addendum 1); the
roadmap entries F-482, F-363, F-483; `crates/rcfd-fv/src/pimple.rs`: the struct `PimpleSolver` (fields), `PimpleSettings::piso`
(momentum `tol_abs` 1e-14 `tol_rel` 1e-8 `max_iter` 500; pressure `tol_abs` 1e-14 `tol_rel` 1e-7 `max_iter` 3000), the uses of
`hist_u` and `phi_hist` in the step (the BDF2 time term, the ddt flux correction, the extrapolated convecting flux), the push of
the history at the end of the step, `PimpleState`, `snapshot`, `restore`; `crates/rcfd-fv/src/ddt.rs` `History` (`old`, `older`,
`dt_old`, `push`) and the BDF2 branch of `add_ddt`; `immersed.rs` (`ImmersedForcing::body_of_cell`); `geometry.rs`
(`FvGeometry::empty`, `active`) and the mesh accessors; `crates/rcfd-fv/tests/immersed_single_corrector_f363.rs` (the DFG
helpers, `forcing`, `r_solver`); the record logs of ADR 0374 (`f363-scratch/rec_g1_g2.log`, `rec_r0_l1_l2.log`, `rec_l3.log`,
for the series of G1 and the run times); `rcfd-linalg/src/eigen.rs` (only symmetric dense eigensolvers exist; none for a general
matrix, hence D4).

Facts read from the code (no run):

- The state that one step of `PimpleSolver::step` reads and writes is held, apart from settings and fixed inputs, in `u`, `p`,
  `phi`, `time`, `step_no` and the private histories `hist_u` (three components) and `phi_hist`; `snapshot` returns all of these
  in `PimpleState` (public fields) and `restore` sets them (and clears `history`, `diverged`, the corrector schedule and the
  ALE history, none of which the set-up R reads). So the whole step is reachable from test code: **no library change**.
- After a step, `hist_u[k].old` is the new `u` component and `phi_hist.old` the new `phi` (`push`); with BDF2 at a constant step
  `hist_u[k].older` is the velocity of the step before and `phi_hist.older` its flux; `oldest` is read only by the bounded BDF2,
  not by R. The state of a BDF2 step at constant `dt` is therefore `(u_n, u_{n-1}, p_n, phi_n, phi_{n-1})`, and the step maps it
  to `(u_{n+1}, u_n, p_{n+1}, phi_{n+1}, phi_n)`.
- The step does not change `immersed` (no mutable use in `step`), so the forcing is a fixed input.

## Arithmetic before this registration (disclosed)

From the series of ADR 0374 G1 (B, one corrector, `dt = 1e-3`): the largest flux-face velocity `v_n` alternates from step 2 on
(1.42, 0.80, 1.16, 0.85, ...) with a two-step ratio near 1 up to step 12 (steps 5 to 12: 0.85, 1.00, 0.96, 1.00, 1.02, 1.01,
1.04, 1.08), then grows: steps 13 to 25 on the odd steps 1.27, 1.76, 2.40, 3.24, 4.27, 5.88, 8.65, a growth per two steps of
1.3 to 1.5, i.e. `|lambda|` about 1.15 to 1.2 per step; the largest cell velocity `a_n` alternates from step 7 on. With two
correctors the same quantities decay without alternating. A real eigenvalue near -1.15 to -1.25 of the step map at a state of
steps 10 to 20 would give this; at the state of step 5 the series is not clearly growing. **No number is predicted**; these
numbers only choose the states (below). The runs of ADR 0374 took about 0.1 s per step of M2 (G1 and G2, 270 steps in 36 s).

## Design (D1 to D4), all in a new test file

**D1. The state vector and its scaling.** For a solver after step `k >= 1` of R, with `s0 = snapshot()`: `x` collects, in this
order, the two active components (`x`, `y`) of `u_n` per cell, the same of `u_{n-1}` (`hist_u[k].older`), `p_n` per cell,
`phi_n / |S_f|` and `phi_{n-1} / |S_f|` per face that is not on an `Empty` patch. Dividing the flux by the face area makes every
entry a velocity or a kinematic pressure of order one (the eigenvalues do not depend on the scaling; the Euclidean norm of `x`
does). The `z` component (not solved, zero) and the empty faces are left at their base values. A perturbation `dx` is applied as
`base + scale * dx` entry by entry (`u` and `hist_u[k].old` together, `phi` and `phi_hist.old` together, since they are the same
level), so `dx = 0` restores `s0` bit for bit.

**D2. The step map and its finite-difference linearisation.** `S(dx)`: `restore` of the perturbed state, one `step(dt)` (no
stop allowed: a stop panics the record), then `x` of the new snapshot. `J v = (S(e v) - S(-e v)) / (2 e)` with `|v|_2 = 1` and
`e = 1e-4` (the largest entry of `e v` is at most 1e-4 m/s against a flow of order 1: the `O(e^2)` remainder is about 1e-8
relative). For the products the linear solves of the step are converged further than in R, momentum and pressure `tol_rel = 1e-12`,
`tol_abs = 0` (iteration limits unchanged): a solve stopped at `tol_rel` leaves an error of about `tol_rel` times its start
residual that differs between `S(e v)` and `S(-e v)`; at 1e-6 (R) that noise, divided by `2 e`, would be of order 1e-2 of the
product; at 1e-12 it is about 1e-8. The base state is reached with the tolerances of R (the trajectory of the stop); the map
linearised is the step of R with its solves converged, of which the step of R is the 1e-6 approximation. The difference of the
two at the base state (`|S_R(0) - S(0)|`) is printed.

**D3. Arnoldi on `J`.** Start `v_1` from a fixed linear congruential sequence (seed 482, entries uniform in `[-1, 1]`,
normalised). `m = 40` steps of Arnoldi with modified Gram-Schmidt, repeated once (two passes) against the loss of orthogonality
that the finite-difference noise would cause. The Ritz values are the eigenvalues of `H_m` (D4); for a Ritz value `theta` its
vector `z = V_m y` (`y` by inverse iteration on `H_m`), the estimated residual `|h_{m+1,m}| |y_m| / |y|`, and, for the values a
check names, the **true residual** `|J z - theta z| / (|theta| |z|)` from one more product (two for a complex pair: `J Re z` and
`J Im z`). A Ritz value is **real** if `|Im theta| <= 1e-3 |theta|` (a complex pair that close to the axis beats over more than
6000 steps) and **converged** if its true residual is at most 0.03 (the eigenvalue is then within about 3 percent, enough to
place it against -1 unless it sits within 3 percent of it; the estimated residual, used where no product is spent, has the same
limit). This separates a real negative eigenvalue from a complex pair directly: Arnoldi returns both members of a pair and the
test is on `Im theta`. Why 40: the step map's slow modes lie in a disc of about `[0, 1]` (implicit BDF2 damps the fast ones and
its parasitic root lies between 0 and 1/3 for real decay), so an eigenvalue near -1.2 is an outlier at about 1.2 from the hull
of width about 1; the Chebyshev bound for an outlier at distance `d` from an interval of half-width `c` converges as
`(w + sqrt(w^2 - 1))^{-k}` with `w = 1 + d / c`, about 0.2 per step for `d = 1.2`, `c = 0.5`: 40 steps are far more than needed
even for an outlier near -0.6 (`w = 2.2`, 0.23 per step). Cost: 80 steps per linearisation plus the residual products.

The parts of the mode (printed, the located description): the shares of `|z|^2` in the five blocks of D1; the share of the
velocity blocks in the first fluid cells.

**D4. Eigenvalues of a small Hessenberg matrix.** Complex arithmetic on a pair of `f64`; single-shift QR on the complex
Hessenberg matrix by Givens rotations with the Wilkinson shift (the eigenvalue of the trailing 2 x 2 block nearest its last
diagonal entry), deflation when `|h_{k,k-1}| <= 1e-14 (|h_{k-1,k-1}| + |h_{k,k}|)`, an exceptional shift after 10 iterations
without deflation, at most 100 m iterations (a failure panics). Inverse iteration: two solves of `(H - theta I + 1e-10 |theta|
I) y = b` by Gaussian elimination with partial pivoting, `b` all ones, normalised.

**First fluid cells.** A cell is fluid when `body_of_cell[c] == None`; a first fluid cell is a fluid cell with an interior face to
a cell that is not fluid. The mode cell of a Ritz vector is the cell with the largest `|z_ux|^2 + |z_uy|^2` of the `u_n` block.

## The runs

Set-up R of ADR 0374 (copied helpers): DFG 2D-2 on M2 (`dfg_family(2)`, 176 x 104 cells), `U_m = 1.5`, `nu = 1e-3`, `K = 1e7`,
the start field of ADR 0249, `PimpleSettings::piso` with BDF2, `LinearUpwind`, AMG pressure, momentum and pressure `tol_rel`
1e-6, `flux_weight_check = false` for every forcing (so that B with one corrector is not refused by ADR 0374 D2; two correctors
are not refused anyway). The linearisations (one each):

| Id | Forcing, correctors, `dt` | State (after step) | Role |
| --- | --- | --- | --- |
| K15 | B, 1, 1e-3 | 15 (`t = 0.015`) | the primary state of E1 |
| K5, K10 | B, 1, 1e-3 | 5, 10 | the dependence on the state (printed) |
| H30 | B, 1, 5e-4 | 30 (`t = 0.015`) | E2 |
| C15 | B, 2, 1e-3 | 15 | E3 |
| F15 | F, 1, 1e-3 | 15 | E4 (negative control: F runs 200 steps, ADR 0374 R0) |

Why the state of step 15: it is the first of the steps where G1 shows the alternating growth clearly (above), still "early"
(the mode in `v_n` at about 1.8 against 1.0 at the start, nine steps before the growth turns violent), and 19 steps before the
stop. The other two forcings are taken at the same physical time.

## Registered checks

New file `crates/rcfd-fv/tests/immersed_step_linearisation_f482.rs`. The heavy records carry `#[ignore]` and are run once by me
in release: record K (K5, K10, K15, E0, R1) and record L (H30, C15, F15). The fast checks run in release in the gate (U1 and U2
also in debug).

| Id | Clause | Check | Limit |
| --- | --- | --- | --- |
| R1 (reproduction; record K) | (set-up) | B, one corrector, `dt = 1e-3`, run on after the snapshot of step 15 with the settings of R | stops at step 34 by the Courant guard (ADR 0374 R0); a different step is a defective copy of the set-up |
| E0 (record K) | 1 | for K15, the vector `z` of `lambda_L` (below): `J z` with `e = 1e-4` against `e = 1e-5` | `|J_1e-4 z - J_1e-5 z| / |J_1e-4 z| <= 1e-3` |
| E1 a (record K) | 2 | K15: `lambda_L`, the Ritz value of smallest real part, is converged (true residual) | `<= 0.03` |
| E1 b | 2 | K15: `lambda_L` real and below -1 | `abs(Im) <= 1e-3 abs(lambda_L)` and `Re lambda_L < -1` |
| E1 c | 2 | K15: `lambda_L` dominant outright | `abs(lambda_L) >` the magnitude of every other Ritz value of `H_40` |
| E1 d | 4 | K15: the mode cell of `lambda_L` | a first fluid cell |
| E2 (record L) | 3 | H30: every Ritz value with `Re theta < 0` and `abs(theta) >= 1` has a true residual above 0.03 (no such converged eigenvalue); and `lambda_L` of H30 converged with `abs(lambda_L) < 1` | as stated |
| E3 (record L) | 3 | C15: as E2 | as stated |
| E4 (record L; negative control) | (control) | F15: as E2 | as stated |
| U1 (fast; also debug) | (tool) | D4 on two 6 x 6 matrices `A = Q T Q`, `Q` a Householder reflection `I - 2 w w^T / w^T w` (`w = (1, 2, ..., 6)`), `T` block upper triangular with known eigenvalues: (i) -1.15, 0.9, 0.5 +- 0.3i, 0.1, -0.4; (ii) 1.15 exp(+-0.9 pi i), 0.95, 0.3, -0.2, 0.05; after reduction to Hessenberg form by the Arnoldi of D3 (exact products) | every eigenvalue matched within 1e-10 |
| U2 (fast; also debug) | (tool) | D3 with exact products on a map of dimension 400, `A = Q D Q` (`Q` a Householder reflection, `D` diagonal entries `0.05 + 0.9 i / 399` except entries 0 and 1): (i) `D_00 = -1.15`, `D_11 = 0.7`; (ii) entries 0, 1 replaced by the 2 x 2 block of `1.15 exp(+-0.9 pi i)` | (i) `lambda_L` real by the test of D3, within 1e-8 of -1.15, estimated residual `<= 1e-8`, the true residual `<= 1e-8`; (ii) `lambda_L` not real, within 1e-8 of `1.15 exp(0.9 pi i)` or its conjugate, its conjugate also a Ritz value within 1e-8 |
| U3 (fast) | 1 | the map of D1/D2 on DFG M1 (`dfg_family(1)`), B, one corrector, `dt = 1e-3`, state after step 2: (a) `restore` of `dx = 0` then a step with the settings of R gives the bits of `u`, `p`, `phi` of the direct trajectory's step 3; (b) for the start vector of D3, `J v` with `e = 1e-4` against `e = 1e-5` within `1e-3` relative; (c) the `u_{n-1}` and `phi_{n-1}` blocks of `J v` equal the `u_n` and `phi_n` blocks of `v` within `1e-9` in the max norm (the exact shift of the history) | as stated |
| N1 | (defaults) | no library file changed (`git diff --stat main` lists only the new test file and docs) | as stated |

Printed, no limit: for every linearisation the twelve Ritz values of largest magnitude with their estimated residuals, the
overall dominant Ritz value, `lambda_L` with its true residual, its mode cell, whether it is fluid, first fluid, its distance to
the surface, the block shares and the first-fluid-cell share; `|S_R(0) - S(0)| / |S(0) - x_0|` at the base; the wall time of
each record; the largest `abs(u)` of every step of the base runs (the alternation).

**Negative controls.** E4 (F, which runs); E3 (two correctors); E2 (the smaller step); U3 (a) (the map at zero is the step).

### Mutations (scratch copy of the final tree, not committed)

Each applied alone by an exact text replacement checked to occur once, the named test run once in release, the file restored
and compared byte for byte with a kept copy.

| Id | Mutation | Must fail |
| --- | --- | --- |
| M1 | the perturbation of `u_n` not applied to `hist_u[k].old` | U3 (c) |
| M2 | the finite difference divided by `e` instead of `2 e` | U3 (b) or (c) |
| M3 | Arnoldi stores `h_{j+1,j}` as the square of the norm | U2 |
| M4 | a sign error in the Givens rotation of the QR step (`-s` made `s` in the update of the rows) | U1 |
| M5 | the real test inverted (`<=` made `>`) | U2 |

## Decision rule

- **done**: R1, E0, E1 a to d, E2, E3, E4, U1 to U3 and N1 met, and every mutation caught by its named check.
- **partial**: U1 to U3, N1, R1 and E0 met (the linearisation delivered and valid) and any of E1 to E4 missed: the eigenvalues
  and the parts of the mode are recorded as the located description; or a mutation not caught.
- **todo**: U1 to U3, R1 or E0 missed and not repaired (the linearisation is not shown to be valid).

A set-up check that does not hold (R1, U3 (a)) is a defective set-up: an amendment before any corrected run, never a pass.

## Limits (not covered)

One flow (DFG 2D-2) on one mesh (M2), BDF2 at a constant step, `LinearUpwind`, one penalty, AMG-preconditioned CG; two
dimensions, a body at rest. The linearisation is about one state per setting (and three for B at `1e-3`); the eigenvalue is a
property of that state's step, not a proof of the run's growth, and the map is that of the step with converged solves. The
largest-magnitude part of the spectrum is reached by Arnoldi from one start vector; an eigenvalue whose mode the start misses
by symmetry would not be found (the start is generic). Why the mode exists (which term of the step) is not registered beyond
the block shares; nothing in the solver is changed, so the refusal of ADR 0374 stays as it is.

## Open questions

- **Q1. A library diagnostic.** Options: (a) keep the linearisation in test code (registered: no library change); (b) a public
  `PimpleSolver::step_jacobian_eigs`. Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation): (a),
  with (b) a follow-up only if another stop needs it.
