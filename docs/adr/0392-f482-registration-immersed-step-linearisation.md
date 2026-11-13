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

## Amendment 1 (2026-11-13): after the records K and L, where the modes of magnitude above one live

Written after the single runs of the fast checks (`f482-scratch/fast_checks.log`) and of the records K and L (`rec_k.log`,
`rec_l.log`), before the run registered here. No limit above is changed; nothing of the records K and L is edited. What bears on
this amendment (the full results are in the addendum): the linearisation is valid (U3, E0 1.8e-5, the solves of the map agree
with those of R to 5e-8, R1 reproduced the stop at step 34); at `dt = 5e-4` (H30) and with two correctors (C15) every Ritz value
lies inside the unit circle (E2, E3 met; `lambda_L` -0.776 and -0.481, both real). At `dt = 1e-3` with one corrector the outer
spectrum is not one isolated negative eigenvalue: for B at step 15 the Ritz value of smallest real part is the complex pair
`-1.103 +- 0.346i` (`|.| = 1.156`, true residual 3.7e-3; also a real -1.073), and the largest Ritz values are pairs of positive
real part up to `|.| = 1.93`; at steps 5 and 10 the leftmost is real (-1.030, -1.265) and the largest have positive real part
(1.47, 1.31). For the first-order forcing F, which runs 200 steps, at step 15: a real -1.024 (converged) and positive pairs up to
1.58. Every one of these vectors has 97 to 100 percent of its squared norm in the pressure. So E1 b, c, d and E4 missed: the
linearisation at the state of step 15 does not separate the setting that stops (B) from the one that runs (F).

The reading (disclosed, not measured): a mode of almost pure pressure with a velocity part below a tenth of it is what a
perturbation of the pressure of the solid cells would give, where the penalty holds the velocity to about `1e-4` of its fluid
value and the pressure is weakly coupled (ADR 0388); such modes could have a magnitude above one without moving the flow (and
would then be invisible in the run's velocity), while the period-two growth of the run is a fluid mode next to the body.

**S1 (registered now; record M).** For the states K15 (B) and F15 (F), the computation of D3 is repeated once (it is
deterministic: the printed Ritz values must equal those of the records K and L to the printed six digits, a reproduction check;
a difference is a defective set-up). For the overall dominant Ritz vector and for the vector of `lambda_L` of each: the solid
share `s`, the sum of the squares of the entries of the solid cells (`u_n`, `u_{n-1}`, `p`) over `|z|^2`; the share of the
pressure block alone in the solid cells; the cell of the largest `abs(p)` entry, whether it is solid, its distance to the
surface. Limit (the hypothesis HS2, that the modes of positive real part and magnitude above one are modes of the solid
pressure): `s >= 0.9` for the overall dominant Ritz vector of both states.

**P3 (registered now; record M): the fluid part of the step.** Arnoldi as D3 (`m = 40`, `e = 1e-4`, the start of D3 with the
entries of the solid cells set to zero, normalised) on `J_f v = P J P v`, `P` setting to zero the entries of the solid cells
(`u_n`, `u_{n-1}`, `p`; the face fluxes are kept): the linearisation of the step with the solid state held at the base. For
K15 (B) and F15 (F):

| Id | Check | Limit |
| --- | --- | --- |
| P3 a | K15: `lambda_L` of `J_f` converged (true residual of `J_f`) | `<= 0.03` |
| P3 b | K15: `lambda_L` of `J_f` real and below -1 | `abs(Im) <= 1e-3 abs(lambda_L)`, `Re < -1` |
| P3 c | K15: the mode cell of `lambda_L` of `J_f` | a first fluid cell |
| P3 d | F15: as E4, for `J_f` | no converged Ritz value of `J_f` with `Re < 0` and `abs >= 1` |

Printed: the twelve largest Ritz values of `J_f`, its overall dominant, the block shares of the modes.

**P2 (printed, no limit; record M).** The runs B (to its stop) and F (34 steps), one corrector, `dt = 1e-3`, the settings of R:
per step the largest `abs(p)` over the solid cells and over the fluid cells, to see whether the solid pressure grows in the run.

If P3 a to d hold, the addendum may say that the fluid part of the step carries a period-two mode for B and none for F at
step 15 (the second branch of the acceptance, in amended form); S1 says whether the other modes are the solid pressure. The
decision is already **partial** by the rule (E1 b to d and E4 missed), whatever record M gives.

## Addendum 1: results (2026-11-13)

Measured against the limits registered above; no limit was changed. Release builds on Windows, the machine shared with other
building agents. Commits: the registration af395384; the test file with the fast checks and the records K and L 8577cafd (before
their run); amendment 1 21844e5b (before any run of record M); record M a387cf32 (before its run); the known-miss records and a
type alias for clippy 334b0550 (not run: they repeat registered records). Every registered run was made once: the fast checks
(`f482-scratch/fast_checks.log`, 0.2 s), record K (`rec_k.log`, 21 s), record L (`rec_l.log`, 26 s), record M (`rec_m.log`,
28 s). After formatting and the clippy fix the fast checks were run on the final tree in release and in debug (`final_release.log`,
`final_debug.log`; all pass), which repeats no record.

**Built.** Test code only: `crates/rcfd-fv/tests/immersed_step_linearisation_f482.rs` (the state vector of D1, the map and its
central difference of D2, Arnoldi of D3, the complex Hessenberg QR and inverse iteration of D4, the fast checks U1 to U3, the
records K, L, M and the three known-miss records). No library file, no shared solver file, no schema key, golden, hash table or
existing test file was touched (N1: `git diff --stat 2570d5e4` lists only this test file and this ADR), so the defaults are
bit-identical by construction and `baseline_hashes` was not needed.

### Checks

| Check | Limit | Measured | Met |
| --- | --- | --- | --- |
| R1 B one corrector `dt = 1e-3` run on after step 15 | stop at step 34, Courant guard | step 34, Courant 2.42e7 | yes |
| E0 `J z`, `e = 1e-4` against `1e-5` (K15) | `<= 1e-3` | 1.80e-5 | yes |
| E1 a `lambda_L` of K15 converged | true residual `<= 0.03` | 3.67e-3 | yes |
| E1 b `lambda_L` of K15 real, below -1 | `abs(Im) <= 1e-3 abs`, `Re < -1` | -1.102973 +- 0.345949i (`abs` 1.155954): below -1 in real part, **not real** | **no** (known miss) |
| E1 c `lambda_L` dominant outright | `abs` above every other Ritz value | 1.156 against 1.928602 (the pair 1.867930 +- 0.479943i) | **no** (known miss) |
| E1 d mode cell of `lambda_L` | a first fluid cell | cell 13258 at (0.21625, 0.25875), fluid, 0.0110 from the surface | **no** (known miss) |
| E2 H30 (`dt = 5e-4`) | no converged Ritz value with `Re < 0`, `abs >= 1`; `lambda_L` converged inside | none; -0.775988 real (residual 4.2e-5); overall dominant 0.978567 | yes |
| E3 C15 (two correctors) | as E2 | none; -0.481230 real (5.5e-3); overall dominant 0.969783 | yes |
| E4 F15 (first-order forcing, runs) | as E2 | -1.024233 real, converged (7.7e-3) | **no** (known miss) |
| U1 Hessenberg eigenvalues, two 6 x 6 matrices | within 1e-10 | 2.0e-15, 2.9e-15 | yes |
| U2 Arnoldi, real outlier and complex pair | as registered | (i) -1.15 real, error 4e-15, residuals 1.7e-20 (estimated) and 3.5e-15 (true); (ii) -1.0937 + 0.3554i not real, error below 1e-15, conjugate present | yes |
| U3 the map on M1 | (a) bit-equal; (b) `<= 1e-3`; (c) `<= 1e-9` | (a) equal; (b) 1.7e-5; (c) 2.2e-12 (`u`), 3.8e-12 (`phi`) | yes |
| N1 no library file changed | as stated | only the test file and the ADR | yes |
| S1 reproduction (amendment 1) | the Ritz values of K15 and F15 to six digits | equal (1.867930 +- 0.479943i, -1.102973 +- 0.345949i; 1.499585 +- 0.483319i, -1.024233) | yes |
| S1 solid share of the overall dominant Ritz vector | `>= 0.9` for K15 and F15 | 0.1309 (B), 0.1539 (F) | **no** (known miss) |
| P3 a `lambda_L` of `J_f` (B) converged | `<= 0.03` | 9.5e-4 | yes |
| P3 b real, below -1 | as registered | -1.138536, real | yes |
| P3 c its mode cell | a first fluid cell | cell 13259 at (0.21875, 0.25875), fluid, 0.0117 from the surface | **no** (known miss) |
| P3 d `J_f` of F15 | no converged Ritz value with `Re < 0`, `abs >= 1` | none (`lambda_L` -0.993338, residual 6.8e-2) | yes |

Printed, no limit. The base steps: `|S_R(0) - S(0)| / |S(0) - x_0|` 3.2e-8 to 1.2e-7 (the solves of R agree with the converged
ones). The state dimension 165296. K5: `lambda_L` -1.030201 real (residual 2.7e-3), mode cell 5678 a first fluid cell 0.00065
from the surface; overall dominant 1.468754 +- 0.082574i (1.471). K10: `lambda_L` -1.265273 real (2.2e-5), further real values
-1.152876 (1.1e-2), -1.040901 (6.7e-2); mode cell 12719, 0.0025 from the surface, not a first fluid cell; overall dominant
1.213569 +- 0.499135i (1.312). K15: also a real -1.072950 (2.2e-2). Every Ritz vector has 97 to 100 percent of its squared norm
in the pressure block (K15 `lambda_L`: `u_n` 0.009, `u_{n-1}` 0.007, `p` 0.969, fluxes 0.015). The largest pressure entry of the
overall dominant vector is a fluid cell 0.0114 from the surface above the cylinder ((0.204, 0.261) for B, (0.196, 0.261) for F);
the solid share of `lambda_L` 0.221 (B) and 0.508 (F). `J_f` of B keeps the outer spectrum of `J` (overall dominant 1.867767 +-
0.479172i) and the pair -1.105887 +- 0.346432i (5.0e-3). P2: the largest `abs(p)` of the run is about the same over the solid and
over the fluid cells at every step (170 to 240, solid and fluid within 2 percent); for B it grows from step 16 (280) to 1.3e7 at
step 31; for F it stays between 111 and 240 to step 34.

### What the measurements say

- **The linearisation is valid.** The map at zero is the step (U3 a), the central difference is linear to 2e-5 (U3 b, E0), the
  shift of the history is exact to round-off (U3 c), the tools find known spectra to round-off (U1, U2), the computation is
  reproducible to the bit (S1 reproduction), and the run of the record reproduces the stop at step 34 (R1).
- **The corrector count and the step move the whole outer spectrum.** With two correctors (C15) and at `dt = 5e-4` (H30) every
  computed Ritz value lies inside the unit circle (largest 0.970 and 0.979, the most negative -0.48 and -0.78, both real). With
  one corrector at `dt = 1e-3` the step has, at the states of steps 5, 10 and 15, many eigenvalues of magnitude 1.3 to 1.9, of
  positive real part, and in **both** forcings (B 1.93, F 1.58 at step 15): pressure modes of the fluid about four cells above the
  cylinder, not of the solid (S1 refuted the reading of amendment 1). F runs 200 steps with them, so an eigenvalue above one of
  the step at one state does not by itself make the run grow (the state changes from step to step; what the run sees is the
  product of the Jacobians along the trajectory, not measured here).
- **The period-two mode of B is in the linearisation, but it is not the dominant eigenvalue.** In the left half plane B has, at
  step 15, a converged pair -1.103 +- 0.346i (1.156) and a real -1.073, and the fluid part of the step a real -1.139 (converged
  to 1e-3); at step 10 a real -1.265, at step 5 a real -1.030 at a first fluid cell. The magnitude 1.14 to 1.27 matches the
  growth of the alternating amplitude in the run (1.15 to 1.2 per step, ADR 0374 G1). F has at step 15 a real -1.024 (and, in
  the fluid part, -0.993, unconverged), so the setting that runs carries a weaker one on the circle.
- **Location.** The mode of the leftmost eigenvalue sits at a first fluid cell at step 5 (B) and in H30, and 0.0025 (step 10) to
  0.011 (step 15) from the surface otherwise: at the body, within about four cells, not in the first fluid cells at step 15.

The first branch of the acceptance is **not reached** in the registered form: the dominant eigenvalue at `dt = 1e-3` is not the
real period-two one (E1 b, c), its mode is not at the first fluid cells at step 15 (E1 d), and the setting that runs also has a
real eigenvalue below -1 (E4). The clauses at `dt = 5e-4` and with two correctors are met. The second branch (the part of the
step that carries the growth) is not reached either: the fluid part of the step carries a real -1.139 for B and nothing
converged outside the circle for F (P3 a, b, d), but its mode cell is not a first fluid cell (P3 c), and amendment 1 is a
measurement registered after a run.

### Mutations

Applied one at a time to a `git archive` copy of 334b0550 (`f482-scratch/mut`) by `f482-scratch/mutate.py` (an exact text
replacement checked to occur once), the named test run once in release, the file restored and compared byte for byte with the
kept copy. The unmutated copy passed the fast checks first (`mut_base.log`).

| Id | Mutation | Must fail | Result |
| --- | --- | --- | --- |
| M1 | the perturbation of `u_n` not applied to `hist_u[k].old` | U3 (c) | failed: "U3 (c): shift errors 8.507e-3 3.833e-12" |
| M2 | the finite difference divided by `e` instead of `2 e` | U3 (b) or (c) | failed at (c): "shift errors 8.507e-3 8.506e-3" ((b) compares two products scaled alike) |
| M3 | Arnoldi stores `h_{j+1,j}` as the square of the norm | U2 | failed: U2 (i), the leftmost Ritz value not within 1e-8 of -1.15 |
| M4 | the sign of the Givens update of the rows (`c * ak1 - s * ak` made `+`) | U1 | failed: the QR iteration does not converge (`hess_eigs: no convergence`) in U1 |
| M5 | the real test inverted | U2 | failed: "U2 (i): not real" |

All five caught by their named checks; the file was restored equal after each (`mut_summary.log`, `mut_M*.log`).

### Tools and the defaults

`cargo fmt -p rcfd-fv --check`: clean. `cargo clippy -p rcfd-fv --all-targets -- -D warnings`: clean. Defaults: no library code
changed.

### Deviations and disclosures

- The estimated residual of a Ritz value is printed relative to `abs(theta)` (D3 writes it without that division); no check
  reads it except U2, where `abs(theta)` is 1.15.
- U3 (c) was coded first with the limit scaled by the largest entry of `v` and corrected to the registered absolute 1e-9 before
  any run.
- Amendment 1 was written after records K and L and reads their numbers; its S1 hypothesis was wrong.
- Not run: the workspace suite and the gate; other meshes, states and time steps than those registered; the channel without the
  cylinder; a linearisation of several steps; the known-miss records (they repeat registered records).

### Decision by the registered rule

U1 to U3, N1, R1 and E0 met (the linearisation delivered and valid); E2 and E3 met; E1 b, c, d and E4 missed (known misses, limits
intact); after amendment 1, S1 and P3 c missed, P3 a, b, d met. **F-482 is partial by the rule.**

### Remaining questions

- **Q1. A library diagnostic** (registration Q1). Options: (a) keep the linearisation in test code; (b) a public
  `PimpleSolver::step_jacobian_eigs`. Recommendation: (a); the test helpers are self-contained and can be copied.
- **Q2. The positive eigenvalues above one with one corrector at `dt = 1e-3`.** They exist in both forcings and the first-order
  run is bounded. Options: (a) a follow-up that linearises `k >= 10` steps along the trajectory (the growth the run sees) and the
  channel without the cylinder (whether the body makes them); (b) leave the one-step description. Recommendation: (a).
- **Q3. The refusal of ADR 0374.** It stays as built (static, one corrector, a reversed flux face). The linearisation supports the
  remark that `dt = 5e-4` is inside the circle at one state; naming a step in the message (ADR 0374 Q1 (c)) should wait for Q2 (a).

### For the lead: roadmap entries

**F-482: proposed status `partial`.** Proposed note (replacing the present one):

> PARTIAL (ADR 0392, 2026-11-13), test code only, no library change. From ADR 0374 question 3, recommendation taken on the maintainer's standing instruction of 2026-11-11. The whole step of DFG 2D-2 on M2 (u_n, u_n-1, p, phi_n, phi_n-1; 165296 unknowns) was linearised by central finite differences about early states, with the solves converged to 1e-12, and its outer spectrum computed by Arnoldi with 40 vectors and a complex Hessenberg QR written from first principles; the linearisation is valid (the map at zero is the step to the bit, linear to 2e-5, the history shift exact, known spectra found to round-off, the stop at step 34 reproduced). With two correctors and at dt 5e-4 every Ritz value lies inside the unit circle (largest 0.970 and 0.979; most negative -0.48 and -0.78, real). With one corrector at dt 1e-3 the bounded form carries the period-two mode (at step 15 a converged pair -1.103 +- 0.346i, magnitude 1.156, and a real -1.073; the fluid part of the step a real -1.139; at step 10 a real -1.265; the magnitude matches the run's growth of about 1.15 per step), at the body (0.0025 to 0.011 from the surface, a first fluid cell at step 5), but it is not the dominant eigenvalue: pressure modes of the fluid four cells above the cylinder have eigenvalues of positive real part up to 1.93, and the first-order forcing, which runs, has them too (1.58) and a real -1.024. Partial by the registered rule: known misses E1 b to d (not real, not dominant, not at a first fluid cell at step 15), E4 (the run that runs has -1.024), and after amendment 1 S1 (those modes are not in the solid) and P3 c. Open (ADR 0392 Q2): the positive eigenvalues above one of the single corrector at dt 1e-3 in both forcings, by a linearisation of several steps along the trajectory and the channel without the cylinder.

**F-363: proposed addition at the end of its note:**

> ADR 0392 (2026-11-13, F-482): a finite-difference linearisation of the whole step finds the period-two mode of the bounded form with one corrector at dt 1e-3 (-1.10 +- 0.35i at step 15, -1.27 at step 10, at the body) and every Ritz value inside the unit circle at dt 5e-4 and with two correctors; with one corrector at dt 1e-3 both forcings also have positive pressure modes up to 1.9, so the one-step spectrum does not separate the setting that stops from the first-order forcing that runs.

**Proposed follow-up** (number to be given by the lead). *Second-order immersed forcing: the single-corrector growth over several steps.* Deps: F-482. Acceptance:

> The growth of DFG 2D-2 on M2 with build_second_order_bounded and one PISO corrector at dt 1e-3 (ADR 0392: the one-step linearisation at step 15 has eigenvalues up to 1.93 for the bounded form and 1.58 for the first-order forcing, which runs, and a period-two pair of magnitude 1.16 for the bounded form) is measured by a registered finite-difference linearisation of k >= 10 consecutive steps along the trajectory, whose dominant eigenvalue per step is above one for the bounded form and below one for the first-order forcing; and the one-step eigenvalues above one of the first-order forcing are explained by a registered measurement (the channel without the cylinder, the state of the step).

Note:

> From ADR 0392 question 2, recommendation taken on the maintainer's standing instruction of 2026-11-11.
