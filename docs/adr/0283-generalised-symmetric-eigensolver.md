# ADR 0283: Eigensolver for K x = lambda M x: dense Jacobi and subspace iteration on the direct factorisation

Status: **REGISTERED 2026-11-12, written before any code of F-382 and before any run of its checks; planned by ADR 0270**.
Not yet accepted by the maintainer. Before this text nothing of F-382 was written, compiled or run: the code and documents
under "What was read" were read, and the arithmetic under "Arithmetic before this registration" was done with a short Python
script of closed forms (eigenvalues of second-difference matrices and their Kronecker sums, ratios of them, iteration
estimates), kept in the scratch folder `w49i-work/f382-scratch/arith.py`. It computes no eigenvalue numerically and runs no
library code. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Work package SB-13 of the structural solver, basic tier ([ADR 0270](0270-structural-solver-basic-tier-plan.md), section 3, the
entry for ADR 0283). Roadmap item F-382 (depends on F-373). Size M. Number: 0283, fixed by the plan; the file name of the
planned stub is kept. Branch `f382-eigensolver` from main 4d23f4dc, worktree `w49i-work/f382`.

House rules (`w49i-work/AGENT_RULES.md`):

- limits are fixed here, before the code and the runs, and are not changed after a run; a missed limit becomes an ignored
  `*_known_miss` test with the limit intact and the number recorded, diagnosed, not tuned away; a defective set-up gets a
  corrected test by an amendment written and committed before the corrected run, the old test stays as an ignored record;
- each registered run is made once; nothing is re-run to pick a result;
- defaults bit-identical: nothing existing changes. The only edit of an existing file is one `pub mod eigen;` line in
  `crates/rcfd-linalg/src/lib.rs`; no existing function, test or golden is edited; nothing outside `crates/rcfd-linalg` changes;
- no new third-party crate, no unsafe code, the crate graph unchanged (`rcfd-linalg` keeps `rcfd-core` and `rayon`);
- clean-room: cyclic Jacobi, subspace iteration with the Rayleigh-Ritz step and the Sturm count by Sylvester's law of inertia
  are textbook methods, derived below from first principles; the sources are cited from memory and nothing of them is used
  beyond what is derived here.

## What F-382 asks

The acceptance of `docs/roadmap.json`, binding:

> Every limit not stated here is registered in the ADR before the run. rcfd-linalg gains a dense symmetric eigensolver and the
> lowest m eigenpairs of K x = lambda M x (K symmetric, M symmetric positive definite) by subspace iteration on the
> factorisation of K + sigma M, with a count of the eigenvalues below a value from the signs of D. Checks: the tridiagonal
> matrix (2, -1) of n = 200 with M = I: the lowest 10 eigenvalues equal 4 sin^2(j pi/(2 (n + 1))) within 1e-10 relative; the
> 5-point Laplacian on 30 x 30: repeated eigenvalues come with their multiplicity and the vectors are M-orthonormal within
> 1e-10; the same with a non-uniform diagonal M against the dense solver within 1e-9; residual |K x - lambda M x| <= 1e-8 |K x|
> for every returned pair; a singular K (free chain) with sigma > 0 returns its zero eigenvalue below 1e-9 of the first
> non-zero one; the count below the m-th eigenvalue equals m - 1; no convergence within the cap is an error stating the
> residuals.

Clauses: (a1) a dense symmetric eigensolver in `rcfd-linalg`; (a2) the lowest m eigenpairs of `K x = lambda M x` by subspace
iteration on the factorisation of `K + sigma M`; (a3) a count of the eigenvalues below a value from the signs of `D`; (a4)
tridiagonal (2, -1), n = 200, `M = I`: the lowest 10 equal `4 sin^2(j pi / 402)` within 1e-10 relative; (a5) 5-point
Laplacian 30 x 30: repeated eigenvalues with their multiplicity, vectors M-orthonormal within 1e-10; (a6) the same with a
non-uniform diagonal `M`, against the dense solver within 1e-9; (a7) `|K x - lambda M x| <= 1e-8 |K x|` for every returned
pair; (a8) a singular `K` (free chain) with `sigma > 0`: the zero eigenvalue below 1e-9 of the first non-zero one; (a9) the
count below the m-th eigenvalue equals m - 1; (a10) no convergence within the cap is an error stating the residuals. The lead's
brief adds: derive the dense method (cyclic Jacobi, its convergence test and why it is accurate for symmetric matrices),
subspace iteration with the Rayleigh-Ritz step on the reduced pair, the shift and its choice, the convergence test per
eigenpair, the Sturm count from the signs of `D` of `K - mu M` (Sylvester) and how it proves that no eigenvalue was missed, the
number of start vectors relative to m with the reason; (a11) mass orthonormality of the eigenvectors checked to a stated
tolerance; (a12) the existing `rcfd-linalg` tests, including `sb_direct.rs`, pass unedited.

## What was read (main 4d23f4dc, read only)

1. `AGENT_RULES.md`; ADR 0270 sections 2 (2.1 to 2.6: generic numerics in `rcfd-linalg`; 2.2 determinism for any thread count
   and every refusal located; 2.5 item 4: `rcfd_linalg::eigen` beside `skyline`) and 3 (the entries of 0274, 0283, 0284: the
   modal analysis of ADR 0284 needs a free box, six zero eigenvalues below 1e-6 of the seventh, and M-orthonormal shapes within
   1e-9); the stub of this ADR; F-382 and F-373 in `docs/roadmap.json`.
2. ADR 0274 in full and `crates/rcfd-linalg/src/skyline.rs`: `SkylineLdlt::factor_with(&CsrMatrix, &SkylineOptions)`,
   `SkylineOptions { ordering, pivots, symmetry_tolerance }`, `PivotRule::{Positive, NonZero}` (refuse `d_i <= theta_i`,
   resp. `|d_i| <= theta_i`, `theta_i = (w_i + 1) eps (|a_ii| + sum_j |u_ij l_ij|)`), `solve(&self, b)`, `pivots()` (in
   elimination order), `order()`, `inertia() -> (negative, positive)`, `SkylineError` (`Pivot { row, position, pivot,
   threshold }` etc.). Sequential with a fixed operation order. Measured (ADR 0274 addendum): 4.6e9 multiply-adds per second;
   30^3 Laplacian factor 0.79 s and 109 MB; inertia of shifted 12^3 Laplacians exact (C11); a singular Neumann 12^3
   Laplacian refused at its last pivot (R1); the path Laplacian (diagonal 1, 2, ..., 2, 1) refused with the pivot exactly 0
   in natural and RCM order (C6).
3. `crates/rcfd-linalg`: `lib.rs` (module list, no `eigen`), `matrix.rs` (`CsrMatrix { n, ncols, row_ptr, col, val }`,
   `from_triplets` sums duplicates and sorts, `matvec` parallel over fixed row chunks with each row summed sequentially, so
   bit-identical for any thread count; `diag`), `vecops.rs` (`dot`, `norm2` with fixed chunking), `reorder.rs`; the tests
   `sb_direct.rs` (its LCG, grid builders, the rayon pool test), `reorder.rs`, `amg_contrast.rs`. There is no dense
   eigensolver in the crate (the only dense LU is inside `amg.rs`).
4. The machine: AMD Ryzen 7 9800X3D (8 cores, 16 threads), 22.4 GB free at the time of reading (other agents build).

## Arithmetic before this registration (disclosed)

All numbers from `f382-scratch/arith.py` (closed forms; nothing solved numerically).

- **T** (tridiagonal (2, -1), n = 200, Dirichlet): `lambda_j = 4 sin^2(j pi / 402)`: `lambda_1 = 2.442861e-4`, `lambda_9 =
  1.975497e-2`, `lambda_10 = 2.437942e-2`, `lambda_11 = 2.948648e-2`, `lambda_21 = 1.067686e-1`; relative gap
  `(lambda_10 - lambda_9) / lambda_10 = 0.19`; `||K||_2 / lambda_1 = 1.64e4`, `eps x` that `= 3.6e-12`.
- **L** (5-point, 30 x 30, diagonal 4, Dirichlet by omission): `lambda_{jk} = 4 sin^2(j pi / 62) + 4 sin^2(k pi / 62)`. In
  ascending order: 1: (1,1) 0.020522706432; 2, 3: (1,2) (2,1) 0.051201470711; 4: (2,2) 0.081880234990; 5, 6: (1,3)
  0.101982840416; 7, 8: (2,3) 0.132661604695; 9, 10: (1,4) 0.172345729976; 11: (3,3) 0.183442974400; 12, 13: (2,4)
  0.203024494255; ...; 19: (2,5) 0.292246885206; 23: (1,6) 0.368734470802. Relative gaps 10 to 11: 0.060, 11 to 12: 0.096.
  `eps ||K|| / lambda_1 = 8.7e-14`. With m = 11 the 11th is simple and four doubled eigenvalues lie below it; with m = 9 the
  9th is the first of a doubled pair (the cluster crosses m, used by check E8).
- **N** (L with the diagonal mass `m_ij = 1 + 0.5 i / 29 + 0.25 (j / 29)^2`, in `[1, 1.75]`, not symmetric under `i <-> j`):
  no closed form; by Courant-Fischer `lambda_k(K, M) in [lambda_k(K) / 1.75, lambda_k(K)]`: `lambda_1 in [0.0117, 0.0205]`,
  `lambda_11 in [0.105, 0.183]`.
- **F** (free chain, path Laplacian of n = 100 nodes, diagonal 1, 2, ..., 2, 1, `M = I`): `lambda_k = 4 sin^2(k pi / 200)`, `k =
  0..99` (eigenvectors `cos((i + 1/2) k pi / n)`; derived by inserting them into the row equations): `lambda_0 = 0`,
  `lambda_1 = 9.868793e-4`, `lambda_4 = 1.577060e-2`, `lambda_5 = 2.462332e-2`, `lambda_13 = 1.644907e-1`; `1e-9 lambda_1 =
  9.87e-13`. `rho = tr K / tr M = 1.98`.
- **B** (consistent-mass bar, extension): `K = (1/h)(2, -1)`, `M = (h/6)(4, 1)`, n = 200, `h = 1/201`; both are Toeplitz
  tridiagonal with the eigenvectors `sin(i j pi / 201)`, so `lambda_j = (6 / h^2) (1 - cos t_j) / (2 + cos t_j)`, `t_j = j pi
  / 201`: `lambda_1 = 9.869805`, `lambda_10 = 988.9713` (continuum `(j pi)^2`: 9.8696, 986.96), `lambda_max = 4.8e5`.
- **S** (decoupled pair, negative control of the count): block 1 the (2, -1) matrix of n = 50, block 2 the same times 1.5, `M =
  I`. Union in ascending order: 0.00379 (1), 0.00569 (2), 0.01516 (1), 0.02274 (2), 0.03405 (1), 0.05108 (2), 0.06041 (1),
  0.09061 (2), 0.09412 (1), 0.13506 (1), 0.14117 (2). Block 1 alone: 5th 0.09412, 6th 0.13506; below their midpoint 0.1146 lie
  9 eigenvalues of the union (5 of block 1, 4 of block 2).
- **Convergence rates** (D6): per iteration the residual of pair i falls by `r_i = mu_i / mu_{q+1}`; iterations to `tau =
  1e-10` about `1 + ln tau / ln r`. T, m = 10, q = 20: `r = 0.228`, 16.6 iterations; L, m = 11, q = 22: `r = 0.497`, 34.0; L,
  m = 9 (m' = 10), q = 18: `r = 0.590`, 44.6; F, m = 5, q = 13, `sigma = 0.01`: `r = 0.148`, 13.0 (with `sigma = 1.98e-4`:
  0.097); B, m = 10, q = 20: `r = 0.225`, 16.4; S (block 1 alone), m = 5, q = 13: `r = 0.135`. Under the mutation q = m + 1,
  T has `r = lambda_10 / lambda_12 = 0.695`, about 63 iterations.
- **Start-vector count** (D7): for a spectrum `lambda_k ~ k^(2/3)` (a 3-D body) the work `q x iterations / m` is 129 at `q =
  1.5 m`, 102 at `2 m`, 97 at `2.7 m`, 97 at `3 m`, 104 at `4 m`; `q = m + 8` at m = 20 needs 94 iterations against 51 for
  `q = 2 m`.
- **Dense cost**: one Jacobi sweep of n = 900 is `n^2 / 2 = 4.05e5` rotations of about `12 n` flops, `4.4e9` flops; 8 to 12
  sweeps at 2e9 to 4e9 flops per second: 10 to 30 s for the reference of check E9.

## Design

**D1. Dense symmetric eigensolver: cyclic Jacobi.** For a symmetric `A` (n x n, row-major) find `A = V Lambda V^T` with `V`
orthogonal. A Jacobi rotation in the plane `(p, q)`, `J = I` except `J_pp = J_qq = c`, `J_pq = s`, `J_qp = -s`, `c^2 + s^2 =
1`, gives `B = J^T A J` with

`b_pq = (c^2 - s^2) a_pq + c s (a_pp - a_qq)`.

Setting `b_pq = 0` with `t = s / c`: `t^2 + 2 tau t - 1 = 0`, `tau = (a_qq - a_pp) / (2 a_pq)`. The root of smaller modulus,
`t = sign(tau) / (|tau| + sqrt(1 + tau^2))` (`t = 1` for `tau = 0`; `t = 1 / (2 tau)` for `|tau| > 1e150`, where `tau^2`
would overflow), has `|t| <= 1`, so the rotation angle is at most `pi / 4`; `c = 1 / sqrt(1 + t^2)`, `s = t c`. Then (by
expanding `J^T A J` and using `b_pq = 0`) `b_pp = a_pp - t a_pq`, `b_qq = a_qq + t a_pq`, `b_pq = b_qp = 0`, and for `k != p,
q`: `b_kp = b_pk = c a_kp - s a_kq`, `b_kq = b_qk = s a_kp + c a_kq`; the eigenvector matrix is updated as `V <- V J`:
`v_kp <- c v_kp - s v_kq`, `v_kq <- s v_kp + c v_kq`. Storage: `A` full (both triangles, rows `p` and `q` updated contiguously
and mirrored into the columns), `V` stored transposed so that its columns `p`, `q` are contiguous.

*Convergence.* With `off(A)^2 = sum_{i != j} a_ij^2`: the Frobenius norm is invariant under `J`, row and column pairs
satisfy `b_kp^2 + b_kq^2 = a_kp^2 + a_kq^2`, and the 2 x 2 block keeps its Frobenius norm, so `b_pp^2 + b_qq^2 = a_pp^2 + a_qq^2
+ 2 a_pq^2` and `off(B)^2 = off(A)^2 - 2 a_pq^2`: every rotation removes the rotated entry's weight from the off-diagonal part.
In a cyclic sweep (row by row: `p = 0..n-2`, `q = p+1..n-1`) with angles at most `pi/4` the off-diagonal part tends to zero
(Forsythe and Henrici, Trans. AMS 94 (1960) 1) and ultimately quadratically per sweep (Henrici 1958; Golub and Van Loan,
Matrix Computations, section on the Jacobi methods; all from memory; only the monotone decrease above is derived here, and the
sweep cap D1-cap below turns a failure of the quadratic phase into a located error).

*Convergence test* (the relative test of Demmel and Veselic, SIAM J. Matrix Anal. Appl. 13 (1992) 1204, from memory): the pair
`(p, q)` is rotated only if `|a_pq| > eps sqrt(|a_pp| |a_qq|)`, `eps = f64::EPSILON`; the iteration has converged when a full
sweep performs no rotation. Termination in floating point: a rotation sets `b_pq` to exactly 0 and creates rounding errors of
at most a few `eps` of the entries it combines (`|c a_kp| + |s a_kq|`), which are themselves off-diagonal entries; so the
rounding noise is relative to the shrinking off-diagonal part and does not stall at a floor. Cap (D1-cap): 50 sweeps (8 to 12
expected for n <= 1000), then `EigenError::DenseNotConverged { sweeps, off }`.

*Why it is accurate for symmetric matrices.* Each rotation is an orthogonal similarity; its computed result is the exact
rotation of `A_k + E_k` with `||E_k||_F <= c eps ||A_k||_F` (two rows of a few flops each), and orthogonal transformations
keep both norms and errors (`||J|| = 1`), so after all rotations the computed diagonal is exactly the spectrum of `A + E` up to
the remaining off-diagonal part `F`, with `||E||_2 <= c' R eps ||A||_2`, `R` the number of rotations touching an entry (about
`sweeps x 2`, per row pair). At the stop, `|f_pq| <= eps sqrt(|a_pp a_qq|) <= eps ||A||_2`, so `||F||_2 <= ||F||_F <= n eps
||A||_2`. For symmetric matrices Weyl's inequality gives `|lambda_i(A + E + F) - lambda_i(A)| <= ||E + F||_2` for every `i`,
independent of the separation of the eigenvalues (a non-normal matrix has no such bound; that is the reason the symmetric
case is accurate); eigenvectors are accurate to `||E + F|| / gap` (Davis and Kahan's sin theta theorem, from memory), and the
accumulated `V` is orthogonal to `O(R eps)`. Estimate: `|lambda_hat - lambda| <~ sqrt(R) eps ||A||` (independent rounding),
`<= (R + n) eps ||A||` (worst case).

*Output.* Eigenvalues ascending (ties by the final diagonal index), each vector's sign chosen so that its entry of largest
modulus (first such index) is positive: deterministic.

*Validation:* `a.len() = n^2`, every entry finite (`NonFinite { row, col }`), `|a_rc - a_cr| <= 1e-12 max |a|`
(`Unsymmetric { row, col }`, first offender in row-major order); the solver uses `(a_rc + a_cr) / 2`.

**D2. Dense generalised problem.** For symmetric `K` and SPD `M` (dense): Cholesky `M = L L^T` (column `j`: `l_jj = sqrt(m_jj
- sum_k l_jk^2)`, refused with `MassNotPositiveDefinite { row: j, pivot }` when the radicand is `<= (j + 1) eps m_jj`, the
pivot test of ADR 0274 D4 for the dense case), `C = L^{-1} K L^{-T}` by two triangular solves, symmetrised, Jacobi (D1) on `C`,
`x = L^{-T} y`. Then `K x = lambda M x` and `X^T M X = Y^T Y = I`: the vectors are M-orthonormal. Used for the Rayleigh-Ritz
step (D5) and as the reference of check E9.

**D3. The problem and the shift.** `K` symmetric, `M` SPD (both `CsrMatrix`, same n). For any `sigma`, `K phi = lambda M phi`
is equivalent to `A phi = mu M phi` with `A = K + sigma M`, `mu = lambda + sigma`: the same vectors, the eigenvalues moved by
`sigma`. If `sigma > -lambda_1`, `A` is SPD (`x^T A x = x^T K x + sigma x^T M x >= (lambda_1 + sigma) x^T M x > 0` by the
Rayleigh quotient bound), so it is factorised by ADR 0274 with `PivotRule::Positive`. A free body has `lambda_1 = 0` (rigid
motions, `K` singular) and needs `sigma > 0`.

*The choice.* A shift slows convergence (`r_i = (lambda_i + sigma) / (lambda_{q+1} + sigma)` grows with `sigma`) and costs
accuracy of `lambda = mu - sigma` only at the level `eps mu` (absolute); it must be large enough that `A` passes the pivot
test with margin (ADR 0274 D4: `d_i / a_ii >= 1 / kappa(A)` for an SPD matrix, against `~ 2 (w + 1) eps`). `Shift::Value(s)`
uses `s` as given; a refusal is `EigenError::ShiftNotPositiveDefinite { shift, source }`. `Shift::Auto` (the default) takes
the first `sigma` of `0, 1e-4 rho, 1e-3 rho, 1e-2 rho, 1e-1 rho, rho, 10 rho` (`rho = sum |k_ii| / sum m_ii`, the mean
eigenvalue for `M = I`, a scale of the spectrum) whose factorisation is accepted and whose smallest pivot ratio `min_i d_i /
a_ii` is `>= 1e-8`; none: `ShiftNotPositiveDefinite` with the last. The ratio floor rejects `sigma = 0` for a singular `K`
whose last pivot happens to be a positive rounding error above `theta_i` (a computed factor that singular would amplify the
null vectors by `~1/eps` and make the subspace numerically rank-deficient); `1e-8` is far above `2 (w + 1) eps` (`< 1e-11` for
`w < 1e4`) and below `1 / kappa` of the matrices of the checks (`>= 1e-5`). For the free chain F, `sigma = 0` is refused (pivot
exactly 0, ADR 0274 C6) and `1e-4 rho = 1.98e-4` is taken: about `lambda_1 / 5`, so convergence is barely slower and the
eigenvalues keep their accuracy.

**D4. Subspace iteration** (Bathe and Wilson, Int. J. Numer. Meth. Engng 6 (1973) 213; Bathe, Finite Element Procedures, the
chapter on eigenproblems; Parlett, The Symmetric Eigenvalue Problem, the chapter on subspace iteration; from memory; derived
here). `T = A^{-1} M` is self-adjoint in the M inner product (`(T x)^T M y = x^T M A^{-1} M y`, symmetric) with eigenvalues
`1 / mu_i` and M-orthonormal eigenvectors `phi_i`. Write the start block `X_0 = Phi C`; then `T^k X_0 = Phi diag(mu_i^{-k}) C`,
so relative to the component along `phi_i` the component along `phi_j`, `j > q`, falls like `(mu_i / mu_j)^k <= (mu_i /
mu_{q+1})^k`: `span(T^k X_0)` converges to the invariant subspace of the q smallest `mu`, provided the q x q block of `C` on
`phi_1..phi_q` is non-singular. Iteration `k = 1, 2, ...` (q columns):

1. `Y = M X` (q sparse products);
2. `Xbar = A^{-1} Y` (q solves with the one factor);
3. each column of `Xbar` scaled to unit M-norm (with `Ybar = M Xbar`, scaled alike), and `Y` scaled with it, so that still
   `A Xbar = Y` (the span is unchanged; it keeps the reduced mass matrix near unit diagonal);
4. Rayleigh-Ritz (D5) on `span(Xbar)`: reduced `A_r = Xbar^T Y` (`= Xbar^T A Xbar`), `M_r = Xbar^T Ybar`, both symmetrised;
   `A_r Z = M_r Z diag(theta)` by D2 (Cholesky of `M_r` refused: `EigenError::RankDeficient { iteration }`), `theta`
   ascending;
5. `X = Xbar Z` (M-orthonormal: `X^T M X = Z^T M_r Z = I`), `mu_i = theta_i`, `lambda_i = mu_i - sigma`;
6. the convergence test (D6) and, once it passes, the Sturm check (D8).

Cost per iteration: q solves (`2 |Env| + n` each), `3 q` sparse products (`Y`, `Ybar`, and `A x` in D6), `O(n q^2)` for the
reduced matrices and `X = Xbar Z`, `O(q^3)` for D2. Sequential with a fixed order except the row-parallel `CsrMatrix::matvec`,
whose rows are summed sequentially: the result is bit-identical for any thread count (ADR 0270 2.2). The dot products of
step 4 use one sequential routine (four accumulators, as ADR 0274 D3).

*Start vectors* (Bathe's first column, the rest pseudo-random): column 0 = `M 1` (the diagonal of a lumped mass; it has a
large component along rigid motions and low modes); columns `1..q-1` from the LCG of `sb_direct.rs` (`s <- s x
6364136223846793005 + 1442695040888963407`, entry `((s >> 11) / 2^53) x 2 - 1`), one stream from the seed
`0xD1B54A32D192ED03`, column after column. A pseudo-random block has a non-singular projection on `phi_1..phi_q` with
probability one; a deficient start is what the Sturm check (D8) detects. `SubspaceOptions::start` may give the q columns
instead (used by the negative control E13).

**D5. The Rayleigh-Ritz step.** In `span(Xbar)` seek `x = Xbar z` with the Galerkin condition `Xbar^T (A x - theta M x) = 0`:
this is the reduced pair `(Xbar^T A Xbar) z = theta (Xbar^T M Xbar) z`, and `Xbar^T A Xbar = Xbar^T Y` because `A Xbar = Y`
(no product with `A` needed). By Courant-Fischer restricted to the subspace (Poincare's separation theorem), `theta_i >=
mu_i` for every `i`: Ritz values are upper bounds, and among all vectors of the subspace they are the stationary values of
the Rayleigh quotient, so the Ritz pairs are the best the subspace can give and the residual of each converges at the rate of
its vector. The `Z` of D2 is `M_r`-orthonormal, hence `X` is M-orthonormal to rounding; after the first iterations `M_r` is
close to the identity (the columns of `Xbar` converge to nearly M-orthogonal directions, and step 3 normalises them), so its
Cholesky factor is well conditioned. In the first iteration it may not be (all columns are drawn towards `phi_1`; condition
about `(mu_q / mu_1)^2`, `1e5` for T) and the error committed then is removed by the following iterations (subspace iteration
only needs the span to be right).

**D6. Convergence test per eigenpair.** For every Ritz pair the residual of the shifted operator,

`rho_i = ||A x_i - mu_i M x_i||_2 / ||A x_i||_2`,

with `A x_i` and `M x_i` computed afresh from `x_i` (the true residual, not a recursive one). Note `A x - mu M x = K x - lambda
M x` exactly, so the numerator is the acceptance's; the denominator `||A x||` is used because `||K x||` vanishes for a zero
mode, while `||A x|| ~ mu ||M x||`, `mu >= sigma > 0`. A pair is converged when `rho_i <= tau`, `tau =
SubspaceOptions::tolerance = 1e-10` (default). The relative K-residual of the acceptance is then `rho_i ||A x|| / ||K x|| ~
rho_i (lambda_i + sigma) / lambda_i`: for `sigma = 0` it is `rho_i`; for F with `sigma = 0.01` the factor at `lambda_1` is 11.1,
giving `<= 1.1e-9 <= 1e-8`. Accuracy implied: for a symmetric pencil, `|theta_i - mu_j| <= ||r_i||^2 / (gap ||x||^2)` (second
order) and `<= ||r_i||_{M^-1}` (first order) for the nearest `mu_j`: at `rho = 1e-10` the eigenvalue error is far below 1e-10
relative when the gap is `>= 1e-5` relative. Floor of `rho` in floating point: about `c eps ||A|| / mu_i` (the backward error of
the solve, ADR 0274 D4): `3.6e-12 c` for T at `lambda_1`, `8.7e-14 c` for L, both far below `tau`.

The wanted block: the iteration needs pairs `1..m'` converged, where `m' >= m` is the end of the cluster containing the m-th
Ritz value: `m' = m`, increased while `m' < q` and `mu_{m'+1} - mu_{m'} <= g mu_{m'+1}`, `g = 1e-6` (D8 explains why a cluster
must not be split); `m' = q` (the cluster fills the subspace) after convergence of pairs `1..q` is the error
`ClusterExceedsSubspace { m, q }`. Converged: `rho_i <= tau` for `i = 1..m'`. Then the Sturm check (D8) runs once; the first m
pairs are returned (values `lambda_i`, vectors, `rho_i`), with `iterations`, `shift`, `subspace_size` and the Sturm record.

*Cap:* `SubspaceOptions::max_iterations = 200` (default; 4 x the 3-D estimate of D7 and 6 x the largest of the arithmetic).
Not converged after the cap: `EigenError::NotConverged { iterations, tolerance, residuals }` with `rho_1..rho_{m'}` of the
last iteration, all printed by `Display` (clause a10).

**D7. Number of start vectors.** Default `q = min(n, max(2 m, m + 8))`; requires `1 <= m <= n - 1` (`BadCount` otherwise;
`m = n` is the dense solver's job), so `q >= m + 1`. Reasons: (i) the m-th pair converges at `r_m = mu_m / mu_{q+1}`, so `q > m`
is needed for any rate below 1 and guard vectors beyond m make `r_m` independent of a near-degeneracy at `m, m + 1`; (ii) for
a 3-D body (`lambda_k ~ k^(2/3)`, Weyl's asymptotics) the total solve work `q x iterations` is flat between `q = 2 m` and `3 m`
(102 and 97 per m in the arithmetic) and rises below; the `O(n q^2)` terms favour the smaller, so `2 m`; (iii) for small m the
floor `m + 8` keeps at least eight guards, enough to hold a free body's six rigid modes plus a doubled mode beside the wanted
ones (ADR 0284's free box). Bathe and Wilson's `min(2 m, m + 8)` (from memory) is the smaller choice; it gives `q = m + 8` for
`m > 8`, which by (ii) costs about 1.8 x the work at m = 20 on 3-D spectra. `SubspaceOptions::subspace_size` overrides `q`
(must satisfy `m < q <= n`).

**D8. The Sturm count and why no eigenvalue was missed.** For a value `s`, factor `K - s M` with `PivotRule::NonZero` (ADR
0274): `P (K - s M) P^T = L D L^T`. This is a congruence (`L P` non-singular), so by Sylvester's law of inertia `K - s M` and
`D` have the same numbers of negative, zero and positive eigenvalues. `K - s M` is congruent to `M^{-1/2} K M^{-1/2} - s I`,
whose eigenvalues are `lambda_i - s`. Hence

`nu(s) = #{i : d_i < 0} = #{eigenvalues lambda of (K, M) with lambda < s}`

(when `s` is no eigenvalue; then no `d_i` is zero). Public function `count_below(K, M, s, ordering) -> Result<usize,
EigenError>`; a factor refused by the `NonZero` rule (a pivot `|d_i| <= theta_i`: `s` numerically at an eigenvalue, or a
breakdown without pivoting) is the error `CountFailed { shift, source }`. In floating point the count is exact for a matrix
within the backward error of the factorisation; without pivoting that error is `c eps |L| |D| |L^T|`, which is small unless a
leading block is nearly singular (growth); ADR 0274 C11 measured exact counts on shifted 12^3 Laplacians.

*Proof that no eigenvalue is missed.* After convergence the solver has `m'` M-orthonormal Ritz pairs with relative residuals
`<= tau`, and Ritz values `theta_{m'} < theta_{m'+1}` separated by more than `g theta_{m'+1}`. Take `s = theta_{m'} + f
(theta_{m'+1} - theta_{m'})` (in `lambda`: minus `sigma`), `f = 1/2`. (i) By Kahan's residual theorem for a block of
orthonormal Ritz vectors (from memory; the one-vector form is the first-order bound of D6), there are `m'` eigenvalues, counted
with multiplicity, within `~ tau mu` of `theta_1..theta_{m'}`; all are `< s` because `s - theta_{m'} >= g mu / 2 = 5e-7 mu >>
tau mu`. (ii) Ritz values are upper bounds (D5): `lambda_i <= theta_i`, so `nu(s) >= m'` in exact arithmetic. (iii) If
`nu(s) = m'`, the eigenvalues below `s` are exactly the `m'` approximated ones: no eigenvalue below `s` is missing, in
particular none below `lambda_{m'}`, and the returned m are the lowest m. If `nu(s) > m'`, some eigenvalue below `s` has no Ritz
approximation (a start block deficient in a mode, or a mode not yet resolved): the error `SturmMismatch { shift, counted,
expected }`; `nu(s) < m'` contradicts (ii) and means the count itself is wrong, the same error. A cluster crossing m must be
completed (`m'`), because a shift between two equal Ritz values is an eigenvalue: `K - s M` is singular there.

*Moving the shift* (the plan's "registered rule"): if the factorisation of `K - s M` is refused, `f` is moved to `1/4`, then
`3/4`, in that order (the count is the same everywhere in `(lambda_{m'}, lambda_{m'+1})`); all three refused:
`CountFailed` with the last shift. The false alarm (`nu(s) > m'` because `lambda_{m'+1} < s` while `theta_{m'+1}` is still far
above it) needs a guard Ritz value in error by more than half the gap; when pairs `1..m'` have converged the guard `m' + 1`
has its eigenvalue error at about `tau^(2 ln r_{m'+1} / ln r_{m'})` (1e-17 for T), so it does not occur in the checks.

**D9. API** (new module `crates/rcfd-linalg/src/eigen.rs`, declared by one `pub mod eigen;` line; no re-export):

- `struct DenseEigen { values: Vec<f64>, vectors: Vec<Vec<f64>>, sweeps: usize }` (vector `j` = `vectors[j]`);
  `fn symmetric_eigen(a: &[f64], n: usize) -> Result<DenseEigen, EigenError>` (D1);
  `fn generalized_eigen_dense(k: &[f64], m: &[f64], n: usize) -> Result<DenseEigen, EigenError>` (D2).
- `enum Shift { Auto, Value(f64) }` (default `Auto`); `struct SubspaceOptions { shift, tolerance (1e-10), max_iterations
  (200), subspace_size: Option<usize> (None: D7), ordering: SkylineOrdering (Rcm), start: Option<Vec<Vec<f64>>> }`.
- `fn lowest_eigenpairs(k: &CsrMatrix, m: &CsrMatrix, count: usize, opts: &SubspaceOptions) -> Result<GeneralizedEigen,
  EigenError>`; `struct GeneralizedEigen { values, vectors, residuals, iterations, shift, subspace_size, sturm: SturmCheck }`,
  `struct SturmCheck { shift, counted, expected }`.
- `fn count_below(k: &CsrMatrix, m: &CsrMatrix, s: f64, ordering: &SkylineOrdering) -> Result<usize, EigenError>` (D8).
- `enum EigenError` with `Display` and `std::error::Error`: `DimensionMismatch { what }`, `NonFinite { matrix, row, col }`,
  `Unsymmetric { matrix, row, col }`, `MassNotPositiveDefinite { row, pivot }`, `DenseNotConverged { sweeps, off }`,
  `BadCount { requested, n, subspace }`, `BadStart { reason }`, `ShiftNotPositiveDefinite { shift, source: SkylineError }`,
  `RankDeficient { iteration }`, `NotConverged { iterations, tolerance, residuals }`, `ClusterExceedsSubspace { m, q }`,
  `SturmMismatch { shift, counted, expected }`, `CountFailed { shift, source: SkylineError }`. Sparse inputs are validated
  first: square, equal n, finite entries, symmetric by the rule of ADR 0274 D6 (`|a_rc - a_cr| <= 1e-12 sqrt(|a_rr a_cc|)`),
  each located (`matrix` = "K" or "M").

## Set-ups

New test file `crates/rcfd-linalg/tests/sb_eigen.rs`; matrices from `CsrMatrix::from_triplets`. All runs in release.

- **T**: (2, -1), n = 200, `M = I`; m = 10; default options (Auto shift).
- **L**: 5-point, 30 x 30, unknown `p = i + 30 j`, diagonal 4, `-1` per grid neighbour; `M = I`; m = 11 (E7), m = 9 (E8).
- **N**: L with `M = diag(1 + 0.5 i / 29 + 0.25 (j / 29)^2)`; m = 11; reference: `generalized_eigen_dense` of the dense copies
  (n = 900).
- **F**: path Laplacian of 100 nodes (diagonal 1, 2, ..., 2, 1), `M = I`, m = 5; `Shift::Value(0.01)` (E10) and `Shift::Auto`
  (E11).
- **S**: block-diagonal (2, -1) n = 50 and 1.5 x (2, -1) n = 50 (unknowns 0..49 and 50..99), `M = I`, m = 5. Deficient start:
  q = 13 columns, entries of the LCG (seed `0x2545F4914F6CDD1D`) on unknowns 0..49 and exactly 0 on 50..99; the block
  structure keeps them exactly 0 (zero right-hand side entries meet exactly zero coupling entries of `L`).
- **B**: consistent-mass bar, n = 200 (arithmetic), m = 10, default options.
- **Dense**: D1 on T as a dense matrix (all 200); D1 on the 5-point 12 x 12 (n = 144: closed form `4 sin^2(j pi/26) + 4
  sin^2(k pi/26)` with multiplicity); D2 on B dense (all 200).
- Norms: 2-norms; M-orthonormality `max_{i,j} |x_i^T M x_j - delta_ij|`, computed in the test from the returned vectors.

## Registered checks

| Id | Clause | Check | Limit | Estimate behind it |
| --- | --- | --- | --- | --- |
| E1 | a1 | D1 on T dense: `max_j |lambda_hat_j - lambda_j|` over all 200; `max |V^T V - I|`; `max_j ||A v_j - lambda_j v_j||` | `<= 1e-12`; `<= 1e-12`; `<= 4e-12` | D1: `sqrt(R) eps ||A||` with `R ~ 2 x 10 sweeps x 200`: 6e-14; worst `(R + n) eps ||A|| = 3.7e-12` (eigenvalues use the typical, so margin 16) |
| E2 | a1 | D1 on the 12 x 12 Laplacian (144): eigenvalues against the sorted closed form with multiplicity; orthogonality; residual | `<= 1e-12`; `<= 1e-12`; `<= 8e-12` | as E1 with `||A|| = 8`, n = 144: 1e-13 typical |
| E3 | a1 (extension) | D2 on B dense: `max_j |lambda_hat_j - lambda_j| / lambda_max`; `max |X^T M X - I|` | `<= 1e-12`; `<= 1e-12` | D1 on `C` (`||C|| = lambda_max`), `kappa(M) = 3`: 1e-14 |
| E4 | a1 | dense refusals: length `n^2 + 1` (`DimensionMismatch`); NaN at (3, 5) (`NonFinite { row: 3, col: 5 }`); `a_25 = 1`, `a_52 = 1 + 1e-9` (`Unsymmetric { row: 2, col: 5 }`); `M = diag(1, 1, -1)` in D2 (`MassNotPositiveDefinite { row: 2 }`); control: `[[0, 1], [1, 0]]` gives `-1, 1` | as stated; control within `1e-15` | D1 validation, D2 pivot test; `tau = 0`, one rotation |
| E5 | a2, a4, a7, a11 | T, m = 10: `|lambda_j - 4 sin^2(j pi/402)| / lambda_j`; `||K x - lambda M x|| / ||K x||`; M-orthonormality; Sturm record `counted = expected = 10`; shift `= 0`; iterations | `<= 1e-10`; `<= 1e-8`; `<= 1e-10`; as stated; `= 0.0`; `<= 34` | eigenvalue: rounding `c eps kappa = 3.6e-12 c`, `c ~ 3`; residual `<= tau = 1e-10`; orthonormality `~ q eps`; D3 (pivot ratio 0.5); iterations 2 x 16.6 |
| E6 | a3, a9 | T: `count_below` at `(theta_9 + theta_10)/2` (computed) and at `theta_10 (1 - 1e-6)`; at `(lambda_j + lambda_{j+1})/2` (closed form), `j = 1..10` | `= 9`; `= 9`; `= j` | D8 (Sylvester); a tridiagonal `L D L^T` count is exact for a nearby matrix (the bisection argument); distance `2.4e-8` against backward error `~1e-17` |
| E7 | a5, a7, a9, a11 | L, m = 11: each value against the closed-form list with multiplicity (pairs 2-3, 5-6, 7-8, 9-10 both members); `|theta_a - theta_b| / theta_a` within each pair; residual; M-orthonormality; Sturm `counted = expected = 11`; `count_below((theta_10 + theta_11)/2)`; iterations | `<= 1e-10`; `<= 1e-10`; `<= 1e-8`; `<= 1e-10`; as stated; `= 10`; `<= 68` | `eps kappa = 8.7e-14`; D5 (a degenerate eigenspace is resolved by Rayleigh-Ritz); 2 x 34 iterations |
| E8 | cluster rule (extension) | L, m = 9 (the 9th is half of a pair): 9 values against the closed form; Sturm `expected = 10` (m' = 10), `counted = 10`; residual; M-orthonormality | `<= 1e-10`; as stated; `<= 1e-8`; `<= 1e-10` | D6, D8; 44.6 iterations |
| E9 | a6, a7, a11 | N, m = 11 against D2 of N (dense, n = 900): `|theta_i - lambda_i^dense| / lambda_i^dense`; residual; M-orthonormality; control: the dense pairs `i <= 11` have `||K v - lambda M v|| <= 1e-10 ||K v||`; dense wall time recorded | `<= 1e-9`; `<= 1e-8`; `<= 1e-10`; control as stated; recorded | dense: `sqrt(R) eps ||C||` with `R ~ 2 x 10 x 900`: 2e-13 absolute, 1.5e-11 relative at `lambda_1 >= 0.0117`; subspace ~1e-12; dense time 10 to 30 s |
| E10 | a8, a7, a11 | F, `Shift::Value(0.01)`: `|lambda_0| / lambda_1`; `lambda_1..4` against `4 sin^2(k pi/200)`; residual `||K x - lambda M x|| / ||K x||` for `k = 1..4` and, for the zero mode, `/ ||(K + sigma M) x||`; M-orthonormality; `count_below((theta_0 + theta_1)/2) = 1`; control: `Shift::Value(0.0)` gives `ShiftNotPositiveDefinite` | `<= 1e-9`; `<= 1e-10`; `<= 1e-8`; `<= 1e-10`; as stated; as stated | `lambda_0 = mu_0 - sigma`: second-order error `(tau sigma)^2 / gap` plus rounding `eps ||A|| ~ 1e-15`, against `9.9e-13`; residual factor 11.1 (D6): 1.1e-9; ADR 0274 C6 |
| E11 | D3 (extension) | F, `Shift::Auto`: shift `= 1e-4 x 1.98` (as computed by the rule); the limits of E10 | as stated | D3 |
| E12 | a10 | T with `max_iterations = 3`: `Err(NotConverged { iterations: 3, tolerance: 1e-10, residuals })`, `residuals.len() = 10`, `max residuals > 1e-10`, `Display` contains "residuals" and every residual in `{:e}` form | as stated | `r_10^2 = 0.05`: far from converged after 3 |
| E13 | D8 negative control (extension, needed for M4) | S with the deficient start: `Err(SturmMismatch { counted: 9, expected: 5 })`; S with the default start: `Ok`, the 5 lowest of the union against the closed form, Sturm `5 = 5` | as stated; `<= 1e-10` | arithmetic (9 below 0.1146) |
| E14 | extension (consistent mass, ADR 0284 use) | B, m = 10: against the closed form; residual; M-orthonormality | `<= 1e-10`; `<= 1e-8`; `<= 1e-10` | `eps lambda_max / lambda_1 = 1.1e-11`; 16.4 iterations |
| E15 | ADR 0270 2.2 (extension) | N, m = 11, inside rayon pools of 1 and 8 threads: values, vectors, residuals, iterations | bit for bit | D4 |
| E16 | a12 | `cargo test -p rcfd-linalg --release`: every existing test passes, no existing test file edited; `git diff main --stat`: `lib.rs` one added line and new files only | as stated | D9 |
| E17 | refusals (extension) | `count = 0` and `count = n` (`BadCount`); K of 10 and M of 11 (`DimensionMismatch`); NaN in M at (4, 4) (`NonFinite { matrix: "M", row: 4, col: 4 }`); K with `k_12 = -1`, `k_21 = -1 + 1e-9` (`Unsymmetric { matrix: "K", row: 1, col: 2 }`) | as stated | D9 validation |

**Negative controls** inside the checks: E10's `Shift::Value(0.0)` (a singular K is refused, so the `sigma > 0` is what makes
F solvable); E13's deficient start (the count catches a missed mode) beside its default start (no false alarm); E9's dense
residual control; E4's zero-diagonal control.

**Mutations** (on a `git archive` copy of the final tree in `w49i-work/f382-scratch`, own target directory; each applied alone
by exact text replacement, the named tests run once, the file restored and compared byte for byte):

| Id | Mutation | Must fail |
| --- | --- | --- |
| M1 | Jacobi: `b_kq = s a_kp - c a_kq` instead of `s a_kp + c a_kq` | E1, E2 |
| M2 | Jacobi rotation test `eps` replaced by `1e-6` | E1 (or E2) |
| M3 | `lambda = mu` (the shift not subtracted) | E10 |
| M4 | the Sturm check skipped (always accepted) | E13 |
| M5 | `count_below` counts positive pivots | E6 |
| M6 | the convergence test looks at the first pair only | E5 |
| M7 | default `q = m + 1` | E5 (iterations) |
| M8 | the cluster extension removed (`m' = m`) | E8 |
| M9 | the reduced mass `M_r` replaced by the identity | E5 |

**Decision rule.** F-382 is proposed **done** (pending the maintainer's acceptance) if E1, E2, E4 to E7, E9, E10, E12, E13 and
E16 are met (every acceptance clause and the brief's proof of no missed eigenvalue) and every mutation fails its test. E3, E8,
E11, E14, E15 and E17 are registered extensions: a miss among them is a `*_known_miss` record and a follow-up item and does not
alone block done (except that M8 is then uncaught, which makes the result partial). A miss or an unrun check among the done set
or an uncaught mutation (after the test is checked to cover it): **partial**, with the miss named. No subspace result on T:
**todo**.

## Limits (what is not covered)

- No Lanczos or block Lanczos (the faster method for many modes; intermediate tier if modal work needs more than a few tens of
  modes); no shift-and-invert into the interior of the spectrum beyond the one shift; no spectrum slicing.
- The factorisation is the direct solver of ADR 0274: the reach is its reach (about 1e5 unknowns in minutes); one factor per
  solve plus one per Sturm count.
- The Sturm count without pivoting (ADR 0274 D4) has no stability guarantee for strongly indefinite shifts; refusals are
  reported, not repaired beyond the moving rule of D8.
- The dense solver is `O(n^3)` per sweep with about ten sweeps: for references up to about n = 1000.
- `M` is assumed SPD in the sparse path; a semi-definite `M` (massless unknowns) is not supported; it is refused only when the
  reduced mass becomes singular (`RankDeficient`).
- No complex, unsymmetric or quadratic eigenproblems; no damping.

## Open points taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. Residual of a zero mode (clause a7): `||K x||` is a rounding level there, so the zero modes of F are checked against
   `||(K + sigma M) x||` (D6); the non-zero ones against `||K x||` as written.
2. A cluster crossing m: the solver converges and verifies the whole cluster but returns m pairs, as asked (D6, D8).
3. `q = max(2 m, m + 8)` instead of Bathe and Wilson's `min(2 m, m + 8)` (D7).
4. `Shift::Auto` with the pivot-ratio floor `1e-8` (D3).
