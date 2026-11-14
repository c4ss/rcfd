# ADR 0450: F-414 registration: a dense eigensolver for general real matrices, the quadratic eigenproblem on a modal subspace, damped and complex modes

Status: **REGISTERED 2026-11-14, written before any code of F-414 and before any run of its checks**. Not yet accepted by the
maintainer. Before this text nothing of F-414 was written, compiled or run: the documents and code under "What was read" were
read, and the arithmetic under "Arithmetic before this registration" was done by hand (no script, no library run). Results go in
the addendum at the end; nothing above the addendum is changed after a run.

Roadmap item F-414 (deps F-408, F-382, F-383), structural solver, advanced tier (plan
[ADR 0310](0310-structural-solver-advanced-tier-plan.md)), size M, stub
[ADR 0317](0317-damped-eigenproblem-general-dense-eigensolver-complex-modes.md). Branch `f414-work` from main 73f5d432, worktree
`w49i-work/f414`, scratch `w49i-work/f414-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits are fixed here and not changed after a run; a missed limit becomes an ignored
`*_known_miss` test; a defective set-up gets a corrected test by a committed amendment before the corrected run; each registered
run is made once. Everything is opt-in: no existing function changes behaviour, no shared solver file of rcfd-fv or rcfd-api is
edited, so every default result keeps its bits by construction. No new dependency, no unsafe code.

## What F-414 asks

The acceptance of `docs/roadmap.json`, binding:

> Every limit not stated here is registered in the ADR before the run. rcfd-linalg: the companion matrix of a polynomial with
> registered real and complex roots returns them within 1e-10; a random matrix of order 50 has residuals |A v - lambda v| <=
> 1e-10 |A|; a symmetric matrix reproduces the Jacobi values of F-382 within 1e-10; a Jordan block is returned with its
> eigenvalue within 1e-6 and a report, not a failure; no convergence within the registered sweeps is an error. One oscillator:
> lambda = -zeta w +/- i w sqrt(1 - zeta^2) within 1e-12. Rayleigh damping a M + b K on a solid model: every mode has the ratio
> (a/w_j + b w_j)/2 within 1e-8. Skew coupling: a two-unknown system with G = g [[0, 1], [-1, 0]] has the closed-form split
> frequencies within 1e-12. Fixed-free bar with a tip dashpot c below A sqrt(E rho): lambda_j = (c0/L) (-atanh(c/(A sqrt(E
> rho))) + i (2 j - 1) pi/2), the first three within 2 percent on 40 modes; at c above that value no oscillating root is returned
> and the result says so. Damping ratios are -Re(lambda)/|lambda|; conjugate pairs are reported once.

Clauses, numbered for the table: a1 companion; a2 random order 50; a3 symmetric against Jacobi; a4 Jordan block; a5 no
convergence is an error; a6 one oscillator; a7 Rayleigh on a solid model; a8 skew coupling; a9 tip dashpot below `A sqrt(E rho)`;
a10 tip dashpot above it; a11 damping ratios and conjugate pairs reported once.

**Clause a10 is wrong as written and is registered in a corrected form** (derived below): above the impedance `Z = A sqrt(E
rho)` the bar keeps an infinite family of oscillating roots, `lambda_j = (c0/L) (-atanh(Z/c) + i j pi)`, `j = 1, 2, ...` (the
fixed-fixed frequencies, damped), and gains one real root (`j = 0`). What changes at `c = Z` is the family, not the existence of
oscillation. The registered check a10 is: at `c = 2 Z` the real root and the first three oscillating roots of the corrected form
within 2 percent on 40 modes, and the result reports the real roots as non-oscillating (says so). Every other closed form is
verified below and is correct as written.

## What was read (main 73f5d432, read only)

ADR 0317 (stub), ADR 0440 (F-408 scaffold: the stubs `rcfd_linalg::eigen_general` and `rcfd_fem::damped` with `PENDING =
Some("ADR 0317 (F-414)")`, the complex type `rcfd_linalg::complex::C64`), the check `x1_modules_and_pending_after_landed` of
`crates/rcfd-fem/tests/sa_scaffold.rs`, ADR 0448's registration (style; F-412's `Dashpot`, `LossFactor` and per-material
stiffness by `assemble_with` in `crates/rcfd-fem/src/harmonic_direct.rs`), `crates/rcfd-linalg/src/eigen.rs` (F-382, ADR 0283:
`symmetric_eigen`, `EigenError`, `DenseEigen`), `crates/rcfd-fem/src/modal.rs` (F-383, ADR 0284: `modal_analysis`,
`ModalOptions`, `ModalResult` with modes on the full `DofMap`, M-orthonormal; ADR 0354 `ResidualTest::Backward`), the header and
function list of `crates/rcfd-fem/src/rotating/whirl/state.rs` (F-505, ADR 0416: `gyroscopic_state_eigen_dense`, a private
Hessenberg reduction and Francis double-shift QR giving eigenvalues of the gyroscopic state matrix, vectors by inverse
iteration on `Q(s)`), `crates/rcfd-fem/src/builders.rs` (`hex_box`), the bar of `crates/rcfd-fem/tests/si_harmonic.rs` (y and z
held everywhere, x held at `xmin`, steel `E = 2e11`, `rho = 7850`, `nu = 0`), and the notes of F-382, F-383, F-408 in
`docs/roadmap.json`. The standing decision on retired scaffold checks (ADR 0291 pattern).

**Reuse of the whirl solver (F-499/F-506).** Its eigenvalue routine is private to `rcfd-fem`, has no balancing and no Schur
vectors, and its reporting (stability flag, modes with `Im s > 0` or real with `Re s >= 0`) is that of a gyroscopic spectrum
symmetric about both axes; a damped spectrum is not, and its overdamped roots (`Re s < 0`, real) would be dropped. So the general
solver is written in `rcfd-linalg` (where ADR 0317 puts it) and the whirl code is left unchanged, bit for bit. It is reused as an
independent cross-check of a8 (check S2). Moving the whirl solver onto the new routine would change its bits (balancing, a
different iteration); listed as an open question, not done.

## Closed forms (first principles)

- **a6 One oscillator** `m x'' + c x' + k x = 0`, `w = sqrt(k/m)`, `zeta = c/(2 m w)`: `x = e^{lambda t}` gives `lambda^2 + 2
  zeta w lambda + w^2 = 0`, `lambda = -zeta w +/- i w sqrt(1 - zeta^2)` for `zeta < 1`; `|lambda| = w`, so `-Re lambda/|lambda|
  = zeta`. At `zeta = 1` the double root `-w` with one eigenvector of the state matrix (a 2 x 2 Jordan block).
- **a7 Proportional damping** `C = a M + b K`: on M-orthonormal undamped modes `Phi^T C Phi = diag(a + b w_j^2)`, so each mode
  is an oscillator with `2 zeta_j w_j = a + b w_j^2`, `zeta_j = (a/w_j + b w_j)/2`. As written.
- **a8 Skew coupling** `M = I`, `G = g [[0, 1], [-1, 0]]`, `K = diag(k1, k2)`: `det(lambda^2 I + lambda G + K) = (lambda^2 +
  k1)(lambda^2 + k2) + g^2 lambda^2`. With `lambda = i w`: `w^4 - (k1 + k2 + g^2) w^2 + k1 k2 = 0`, so `w^2 = (S +/- sqrt(S^2 -
  4 k1 k2))/2`, `S = k1 + k2 + g^2`, every root imaginary. For `k1 = k2 = w0^2`: `w = sqrt(w0^2 + g^2/4) +/- g/2` (the split).
- **a9, a10 Bar with a tip dashpot.** `0 <= x <= L`, clamped at 0, `rho A u_tt = E A u_xx`, a dashpot `c` at `L`: `E A u_x(L) =
  -c u_t(L)`. With `u = sinh(lambda x/c0) e^{lambda t}`, `c0 = sqrt(E/rho)`: `E A (lambda/c0) cosh(lambda L/c0) = -c lambda
  sinh(lambda L/c0)`, so with `s = lambda L/c0` and `Z = E A/c0 = A sqrt(E rho)`: **`tanh s = -Z/c`**.
  - `c < Z` (`r = c/Z < 1`): `|tanh s| = 1/r > 1`. `tanh(a + i pi/2 + i k pi) = coth a`, so `coth a = -1/r`, `a = -atanh r`,
    `s_j = -atanh(c/Z) + i (2 j - 1) pi/2`. The roadmap's form, as written.
  - `c > Z`: `tanh s = -Z/c` with `|Z/c| < 1`; `tanh` has period `i pi`, so `s_j = -atanh(Z/c) + i j pi`, `j = 0, +/-1, ...`.
    `j = 0` is a real (non-oscillating) root, `j >= 1` oscillate at `j pi c0/L` (fixed-fixed frequencies: the strong dashpot
    approaches a clamp; `c -> infinity` gives `s = i j pi`). The corrected form registered for a10.
  - `c = Z`: `tanh s = -1` has no finite root (the wave is absorbed). Not checked (see Limits).
  - Damping ratio of a real negative root: `-Re/|lambda| = 1`.
- **Loss factor** (check L1, not in the acceptance; the goal of ADR 0317 names loss factors): a constant loss factor `eta` on the
  whole model enters the eigenproblem as `lambda^2 M + (1 + i eta) K` (the complex stiffness of F-412). On the modes: `lambda^2 =
  -(1 + i eta) w_j^2`, `lambda = +/- i w_j sqrt(1 + i eta)` (principal root, `sqrt(1 + i eta) = p + i q`, `p, q > 0`). The root
  `+i w sqrt(1 + i eta) = -w q + i w p` decays with positive frequency; the other grows with negative frequency (the
  well-known non-causal root of the constant hysteretic model). The spectrum is not conjugate-symmetric, so the rule for a
  complex stiffness is: report the roots with `Im lambda > 0`.

## Arithmetic before this registration (disclosed, by hand)

**Truncation of the dashpot bar to `N` undamped modes (a9, a10).** Fixed-free modes `phi_k = sqrt(2/(rho A L)) sin(beta_k
x/L)`, `beta_k = (2k - 1) pi/2`, `phi_k(L)^2 = 2/(rho A L)`. On `N` modes: `(lambda^2 + w_k^2) q_k + c lambda phi_k(L) u = 0`,
`u = sum phi_l(L) q_l`, so `1 + c lambda sum_k phi_k(L)^2/(lambda^2 + w_k^2) = 0`, i.e. `g_N(s) = 1 + (2 c/Z) s S_N(s) = 0`,
`S_N = sum_{k <= N} 1/(s^2 + beta_k^2)`. With `S_inf = tanh(s)/(2 s)` (the partial fractions of `tanh`) `g_inf = 1 + (c/Z) tanh
s`, whose roots are those above (a check of the derivation). The tail `S_inf - S_N ~ (4/pi^2) sum_{k > N} 1/(2k - 1)^2 ~
(4/pi^2)/(4 N)` for `|s|` well below `beta_N` (here `|s| <= 10`, `beta_41 = 127`). First-order root shift: `delta s = 2 s
tail/sech^2 s`, so `|delta s|/|s| = 2 tail/|1 - tanh^2 s|`:

| `N` | tail | a9 `c = Z/2` (`1 - tanh^2 = -3`) | a10 `c = 2 Z` (`1 - tanh^2 = 3/4`) |
|---|---|---|---|
| 10 | 1.01e-2 | 6.8e-3 | 2.7e-2 |
| 20 | 5.1e-3 | 3.4e-3 | 1.35e-2 |
| 40 | 2.53e-3 | **1.7e-3** | **6.7e-3** |

The same relative error for every root `j <= 3` (and `j = 0` of a10) to first order; the margins under 2 percent are 12 and 3.
The truncation shifts each root along `s` (no phase error for a9). Near `c = Z` (`1 - tanh^2 -> 0`) the estimate diverges: the
reason the registered values stay away from `c = Z`. The truncated problem also has roots of no continuum counterpart (a fast real
root near `s ~ -(2c/Z) N` for large `c`); the checks take the root nearest each closed form, and the real root of a10 is the real
root of smallest modulus.

**Finite-element model of the bar.** `hex_box([0; 3], [1, 0.01, 0.01], [100, 1, 1])` (cubes `h = 0.01`), steel `E = 2e11`, `rho
= 7850`, `nu = 0`, y and z held at every node, x held at `xmin`: 400 free unknowns. With `nu = 0` and uniform section motion it is
the consistent-mass 1-D chain (F-383: equal within 2.2e-14). The chain's frequency error `(k h)^2/24` at `s = 8`: 2.7e-4, small
against the truncation. The other motions left (x varying across the section) start near `w h/c0 = sqrt(12 G/E) = sqrt 6`, i.e.
`s ~ 245`, above the 40th axial chain mode (`s_40 ~ 132` with the consistent-mass dispersion `w^2 h^2 = 6 c0^2 (1 - cos kh)/(2 +
cos kh)`): the lowest 40 modes are the 40 axial ones (estimate; a wrong estimate shows as a miss of a9, not a tuned set-up). The
tip dashpot: `c/4` on each of the four `xmax` nodes along x (the axial modes have equal x displacement on the section). `c0 =
5047.5 m/s`, `Z = A sqrt(E rho) = 1e-4 sqrt(2e11 x 7850) = 3962.3 N s/m`.

**Rounding estimates.** Francis QR is backward stable: the residual `||A v - lambda v|| ~ c n eps ||A||`, about 1e-14 `||A||` at
`n = 50` (a2, limit 1e-10: margin 1e4). Companion roots of modulus <= 3.2 with separations >= 0.5: error ~ condition x 1e-15, a
condition of the order 10 to 100, so 1e-13 (a1). Symmetric order 40, well-separated values: 1e-14 `max|lambda|` (a3). A Jordan
block of size `k` under an orthogonal similarity: computed eigenvalues spread by `(eps ||A||)^{1/k}` around the exact one: 1.5e-8
(`k = 2`), 6e-6 (`k = 3`); their mean errs by about `eps ||A||` (the trace is kept). So for `k = 3` the clause a4 can only hold for
the reported mean, which is what is registered. A one-unknown oscillator, `w = 3`: 1e-15 `w` (a6). Skew system: 1e-15 (a8).
Rayleigh model: the ratio from the projected `2 m` problem, `||A_bal|| ~ w_max`, absolute error of `lambda_j ~ n eps w_max`,
relative to `zeta_j` about 1e-12 (a7, limit 1e-8).

## Design

### D1 `rcfd_linalg::eigen_general` (fills the stub; new code only in that file)

All eigenvalues and eigenvectors of a dense real `n x n` matrix `A` (row-major), from published algorithms. Sources are named
from memory and nothing numeric is taken from them; every step is checked by the residual tests, not taken on trust:

1. **Balancing** (Parlett and Reinsch, Numer. Math. 13, 1969; the scaling part only, no permutation): repeated sweeps; for each
   `i` with row and column off-diagonal 1-norms `r`, `c`, a power of 2 `f` with `c f^2 ~ r` (radix 2, so exact); applied when
   `(c f + r/f) < 0.95 (c + r)`; until a sweep changes nothing. `A_b = D^-1 A D`. Optional (`balance`, default on).
2. **Hessenberg reduction** by Householder reflections (Golub and Van Loan, Matrix Computations, section 7.4), the orthogonal
   `Q` accumulated explicitly: `H = Q^T A_b Q`.
3. **Real Schur form** by the Francis implicit double-shift QR iteration (Golub and Van Loan section 7.5; Francis 1961): on the
   active window `[l, hi]`, the shifts are the eigenvalues of its trailing 2 x 2 block (passed as their sum and product), the
   bulge chased by 3-element Householder reflections and a final 2-element one, applied to the whole of `H` and accumulated
   into `Z` (starting `Z = Q`). Deflation when `|h_{k,k-1}| <= eps (|h_{k-1,k-1}| + |h_{k,k}|)` (with `||H||_F` when that sum is
   0), set to 0. **Exceptional shifts** (Wilkinson and Reinsch, Handbook for Automatic Computation II, procedure hqr) at the 10th
   and 20th iteration on a window without deflation: sum `1.5 s`, product `s^2`, `s = |h_{hi,hi-1}| + |h_{hi-1,hi-2}|`.
   **Cap: `ITERATIONS_PER_EIGENVALUE = 30` times `n` iterations in total** (the "registered sweeps" of a5); exceeding it returns
   `GeneralEigenError::NotConverged { iterations, window, subdiagonal }`, never a partial result. The option `max_iterations`
   overrides the cap (for the test of a5).
4. **2 x 2 blocks**: a deflated 2 x 2 block with real eigenvalues is split by a plane rotation whose first column is an
   eigenvector of the block (applied to `T` and `Z`), so every 2 x 2 block left holds a complex conjugate pair `(a + d)/2 +/- i
   sqrt(-disc)/2`, exactly conjugate.
5. **Eigenvectors** by back-substitution on the quasi-triangular `T` (Golub and Van Loan section 7.6; the device of EISPACK hqr2,
   from memory): for `lambda` at block `p`, `y` = the null vector of the block's `T_pp - lambda I`, then upwards `(T_ii - lambda
   I) y_i = -sum_{j > i} T_ij y_j` over 1 x 1 and 2 x 2 blocks in complex arithmetic (`C64`), a divisor or 2 x 2 determinant of
   modulus below `eps ||T||_F` replaced by `eps ||T||_F` (the hqr2 device for repeated eigenvalues). `v = D Z y`, normalised to
   unit 2-norm with its entry of largest modulus real and positive; the vector of `conj(lambda)` is `conj(v)`. Residual
   `||A v - lambda v|| / ||A||_F` reported per pair.
6. **Defect report** (a4): eigenvalues `j`, `k` are linked when `1 - |v_j^H v_k| <= DEFECT_COSINE = 1e-6` and `|lambda_j -
   lambda_k| <= DEFECT_GAP ||A||_F`, `DEFECT_GAP = 1e-3`; each connected group of two or more is a `Defect { indices, mean,
   spread }` (`mean` the arithmetic mean of its eigenvalues, `spread` the largest distance from it). The result carries the
   list; it is never an error. Estimate: a computed Jordan block of size 2 or 3 has eigenvector angles ~ 1e-8 and 1e-5 (`1 - |cos|
   ~ 5e-17`, `5e-11`), linked; a diagonal matrix with a repeated eigenvalue has exactly orthogonal computed vectors (the numerators
   of the back-substitution are 0), not linked.
7. Errors (`GeneralEigenError`): `DimensionMismatch { what }`, `NonFinite { matrix, row, col }`, `NotConverged`,
   `MassNotPositiveDefinite { row, pivot }` (D2). Values are returned in Schur order (deterministic for a given input).

API: `general_eigen(a, n)`, `general_eigen_with(a, n, &GeneralEigenOptions)` -> `GeneralEigen { values: Vec<C64>, vectors:
Vec<Vec<C64>>, residuals, defects, iterations, norm }`.

### D2 the quadratic eigenproblem (same file)

`(lambda^2 M + lambda C + K + i H) x = 0`, dense `n x n`, `M` symmetric
positive definite, `C`, `K`, `H` any real (`H` optional, the loss-factor stiffness). With `M = L L^T` (Cholesky; a failing pivot
is `MassNotPositiveDefinite`), `x = L^-T y`: `y'' + C^ y' + K^ y = 0`, `C^ = L^-1 C L^-T`, `K^ = L^-1 K L^-T` (and `H^`). The
state `z = [y; lambda y]`: `lambda z = A z`, `A = [[0, I], [-(K^ + i H^), -C^]]`.

- Without `H`: `A` is real `2n x 2n`, D1 directly; `x` = `L^-T` times the first half of `z`.
- With `H`: `A = A_r + i A_i` is complex; its eigenvalues are taken from the real embedding `B = [[A_r, -A_i], [A_i, A_r]]`
  (`4n x 4n`): if `B [u; v] = mu [u; v]` then `A (u + i v) = mu (u + i v)`; `spec(B) = spec(A) U conj(spec(A))`, and a vector of
  the conjugate part has `u + i v = 0` (derivation: `A x = mu x` gives `B [x; -i x] = mu [x; -i x]`, and `B` real gives the
  conjugate pair `[conj x; i conj x]`, with `u + i v = 0`). The `2n` eigenpairs with the largest `||u + i v||/||u - i v||` are
  kept, `z = u + i v`.
- `x` normalised to `x^H M x = 1`, entry of largest modulus real positive; per pair the backward residual `||Q(lambda) x|| /
  ((|lambda|^2 ||M|| + |lambda| ||C|| + ||K + i H||) ||x||)` (Frobenius norms); the defect report of D1 on the state vectors
  `z` (unit-normalised).

API: `quadratic_eigen(m, c, k, n)`, `quadratic_eigen_with(m, c, k, h: Option<&[f64]>, n, &GeneralEigenOptions)` ->
`QuadraticEigen { values, vectors, residuals, defects, iterations }`.

### D3 `rcfd_fem::damped` (fills the stub)

`damped_modes(model, &DampedOptions) -> Result<DampedModes>`:

1. `modal_analysis(model, &options.modal)` (F-383, unchanged): `m` undamped modes `Phi` on the full `DofMap`, `w_j`.
2. The model's stiffness and consistent mass (`stiffness_with`, `consistent_with`, as `modal_analysis` assembles them) and the
   projections `K_r = Phi^T K Phi`, `M_r = Phi^T M Phi` computed (not assumed diagonal); `C_r = a M_r + b K_r` (Rayleigh, option
   `rayleigh(a, b)`), plus `Phi^T C_d Phi` of the dashpots (F-412's `Dashpot`: `c` on the diagonal to the ground, `c [1 -1; -1 1]`
   between two nodes; a `pub(crate)` accessor of its fields is added to `harmonic_direct.rs`, additive), plus `H_r = sum_m eta_m
   Phi^T K_m Phi` of the constant loss factors per element set (F-412's `LossFactor::Constant`, `K_m` assembled by
   `assemble_with` over the elements of material `m` as F-412 does; `LossFactor::Table` refused: a frequency-dependent modulus makes
   the eigenproblem nonlinear).
3. `quadratic_eigen_with(M_r, C_r, K_r, H_r, m)` (D2): `2 m` roots.
4. **Reporting (a11)**: without `H` the spectrum is conjugate-symmetric; a root is *oscillating* when `Im lambda > REAL_TOLERANCE
   |lambda|`, `REAL_TOLERANCE = 1e-6` (the Jordan limit of a4: a critically damped pair splits by ~1.5e-8 `|lambda|`); each
   oscillating pair is reported once, by its root with `Im lambda > 0`; each real root (`|Im lambda| <= REAL_TOLERANCE |lambda|`)
   is reported once, as non-oscillating with `Im` set to 0. With `H`: the roots with `Im lambda > REAL_TOLERANCE |lambda|` and the
   real ones. Order: `(Im lambda, |lambda|)` ascending. Each `DampedMode`: `eigenvalue: C64`, `decay_rate = -Re lambda`,
   `damped_omega = Im lambda`, `frequency = Im lambda/(2 pi)`, `damping_ratio = -Re lambda/|lambda|`, `oscillating: bool`,
   `shape: Vec<C64>` (`Phi q`, full `DofMap`, `x^H M x = 1`), `residual` (D2). `DampedModes` carries the modes, the undamped
   `omegas`, the basis size, the counts `oscillating` and `non_oscillating`, the defects, and `notes: Vec<String>` (for example
   "2 real roots: non-oscillating (overdamped) motion") so that the result says when a root does not oscillate (a10).
5. Refused (`FemError::InvalidValue`): the transformation path (linear constraints, rigid sets, supports; as F-412), a negative
   or non-finite coefficient, a dashpot on a missing node or component, an unknown or repeated element set, a modulus table.
   The assembly takes the thread count; the dense algebra is sequential, so the result is bit-identical for any.

### D4 scaffold rows

`x1_modules_and_pending_after_landed` of `sa_scaffold.rs` becomes an ignored record citing ADR 0291 (its body unedited), and a copy
`x1_modules_and_pending_after_f414` has the rows `damped` and `eigen_general` marked "landed" (both `PENDING = None`). The
edit is kept to that test (F-413 may land `fatigue` in the same file).

## Checks (limits fixed now)

Tests: `crates/rcfd-linalg/tests/sa_eigen_general.rs` (G, Q), `crates/rcfd-fem/tests/sa_damped.rs` (R, B, L, T, N), release
profile. Matching of computed to exact roots: each exact root takes the nearest computed root not yet taken.

| Id | Clause | Check | Limit | Estimate |
|---|---|---|---|---|
| G1 | a1 | companion of `p = (x - 1)(x - 2)(x + 3)(x^2 - x + 4.25)(x^2 + 2x + 2)` (roots `1, 2, -3, 0.5 +/- 2i, -1 +/- i`; coefficients exact in binary), balancing on: `max |lambda - r|` | 1e-10 | 1e-13 |
| G1b | a1 | same: residuals `||A v - lambda v||/||A||_F` (catches the unbalancing of vectors) | 1e-10 | 1e-14 |
| G2 | a2 | order 50, entries uniform in [-1, 1] by splitmix64 (seed `0x5EED_0414`): every residual `||A v - lambda v||_2 <= 1e-10 ||A||_F ||v||` | 1e-10 | 1e-14 |
| G2b | a2 | same: 50 values, complex ones in exactly conjugate pairs, no defect reported (negative control of D1.6) | exact | holds |
| G3 | a3 | symmetric order 40 (splitmix64, seed `0x5EED_0415`, `(B + B^T)/2`): sorted real parts against `symmetric_eigen` (F-382): `max |.|/max|lambda|`; and `max|Im|/max|lambda|` | 1e-10 each | 1e-14 |
| G4a | a4 | Jordan block `J_4(2)` (upper triangular): each value within 1e-6 of 2, one defect of 4 indices reported, `Ok` | 1e-6 | exact |
| G4b | a4 | `Q J_2(1.5) Q^T`, `Q` a Householder reflection from a splitmix64 vector (seed `0x5EED_0416`), embedded in `diag(..., -1, 3)` (order 4): both values within 1e-6 of 1.5, one defect `{2 indices}` with mean within 1e-6 | 1e-6 | 1.5e-8 |
| G4c | a4 | `Q J_3(-0.5) Q^T` (order 3, same `Q` construction, seed `0x5EED_0417`): one defect of 3 indices, mean within 1e-6 of -0.5 (individual values recorded, not limited: estimate 6e-6) | 1e-6 | 1e-15 |
| G4d | a4 | negative control `diag(1, 1, 2)` and identity of order 3: no defect reported, values exact | exact | holds |
| G5 | a5 | order-50 matrix of G2 with `max_iterations(3)`: `Err(NotConverged { iterations: 3, .. })` naming a window | error | holds |
| G6 | hard case | cyclic permutation of order 6 (stagnates without exceptional shifts): the sixth roots of unity within 1e-10, `Ok` | 1e-10 | 1e-15 |
| G7 | refusal | a NaN entry: `Err(NonFinite { row, col })` with its position; `a.len() != n^2`: `DimensionMismatch` | error | holds |
| Q1 | a6 | `quadratic_eigen` `m = 1`, `k = w^2`, `c = 2 zeta w`, `w = 3`, `zeta = 0.05, 0.5, 0.95`: both roots against `-zeta w +/- i w sqrt(1 - zeta^2)`, `/w` | 1e-12 | 1e-15 |
| Q1b | a4, a6 | `zeta = 1`: both roots within 1e-6 `w` of `-w`, a defect of 2 reported, `Ok` | 1e-6 | 1.5e-8 |
| Q2 | a8 | `M = I`, `G = g [[0,1],[-1,0]]` as `C`, (i) `K = 4 I`, `g = 1`; (ii) `K = diag(1, 4)`, `g = 0.5`: the roots with `Im > 0` against `i w` of the closed form, `|lambda - i w|/w` | 1e-12 | 1e-15 |
| S2 | a8 | the same two systems through `rcfd_fem::rotating::gyroscopic_state_eigen_dense` (F-505, unchanged): its `omegas` against Q2's `Im lambda`, relative | 1e-12 | 1e-15 |
| Q3 | refusal | `M` not positive definite: `MassNotPositiveDefinite` | error | holds |
| R1 | a7, a11 | Hex8 cantilever `hex_box([0;3], [1, 0.1, 0.1], [10, 2, 2])`, steel `nu = 0.3`, `xmin` held, 12 modes (`ModalOptions::new(12).tolerance(1e-12).residual_test(Backward)`), Rayleigh `a = 0.2 w_1`, `b = 0.5/w_12` (`w_j` of the modal analysis): every reported `damping_ratio` against `(a/w_j + b w_j)/2`, absolute | 1e-8 | 1e-12 |
| R2 | a11 | same: 12 modes reported (one per pair), all oscillating with `Im > 0`, `damping_ratio == -Re/|lambda|` (as computed), `non_oscillating == 0`; shapes `x^H M x = 1` within 1e-10 | exact, 1e-10 | holds |
| B1 | a9 | bar above, `c = Z/2`, 40 modes (`ModalOptions::new(40).tolerance(1e-12).residual_test(Backward)`): the reported roots nearest `lambda_j = (c0/L)(-atanh(1/2) + i (2j - 1) pi/2)`, `j = 1, 2, 3`: `|lambda - lambda_j|/|lambda_j|` | 2e-2 | 1.7e-3 |
| B2 | a10 (corrected) | same bar, `c = 2 Z`: the real root of smallest modulus against `-(c0/L) atanh(1/2)`, and the oscillating roots nearest `(c0/L)(-atanh(1/2) + i j pi)`, `j = 1, 2, 3`, relative | 2e-2 | 6.7e-3 |
| B2b | a10 | same: `non_oscillating >= 1`, every real root has `oscillating == false` and `damping_ratio == 1` within 1e-12, and `notes` names the non-oscillating roots ("says so"); at `c = Z/2` (B1) `non_oscillating == 0` | exact | holds |
| B3 | a9 diagnostic | B1's error of `lambda_1` on 10, 20, 40 modes: decreasing in that order (estimate 6.8e-3, 3.4e-3, 1.7e-3) | monotone | holds |
| L1 | loss factor | R1's cantilever without Rayleigh, `LossFactor::Constant(0.04)` on `all`: 12 roots reported, each against `i w_j sqrt(1 + 0.04 i)`, `/|lambda|` | 1e-10 | 1e-12 |
| T1 | defaults/threads | R1 with 1 and 4 threads: every eigenvalue and shape entry bit-identical | bits | holds |
| N1 | refusals | `LossFactor::Table` refused; unknown element set refused; negative dashpot refused; a model with a linear constraint (transformation path) refused; each an `InvalidValue` naming the cause | error | holds |
| X1 | scaffold | `x1_modules_and_pending_after_f414` passes (damped, eigen_general landed, the other rows as before) | pass | holds |
| E1 | existing | the existing tests of the touched files pass unedited: `rcfd-linalg` (all its tests), `rcfd-fem` `sa_harmonic_direct`, `sa_scaffold` (the new and remaining), `si_rotating_whirl_state` | pass | holds |

Mutations (on a scratch copy; each must be caught by at least one registered check):

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | no exceptional shifts | G6 |
| MU2 | deflation test `eps` replaced by `1e-6` | G1, G2 or G3 |
| MU3 | back-substitution sum omits the term `j = i + 1` | G1b, G2 |
| MU4 | eigenvectors not multiplied by `D` after balancing | G1b |
| MU5 | linearisation with `+C^` in place of `-C^` | Q1 |
| MU6 | dashpot projected with `c/2` | B1, B2 |
| MU7 | both roots of a pair reported | R2 |
| MU8 | embedding keeps the pairs with the smallest `||u + i v||/||u - i v||` | L1 |
| MU9 | Rayleigh with `b M_r` in place of `b K_r` | R1 |
| MU10 | defect report disabled | G4a to G4c, Q1b |

## Decision rule

**done**: every check G1 to X1 and E1 met at its registered run, every mutation caught, no amendment after a run. **partial**:
a check missed (recorded as `*_known_miss` with its limit), a mutation not caught by a registered check, or a check met only
after an amendment made after its run. **todo**: the solver does not build or a5/a1 to a3 cannot be run.

## Limits (not covered)

- The impedance-matched dashpot `c = Z` (no continuum root) and values near it, where truncation to undamped modes converges
  slowly (`1/|1 - tanh^2|`); residual vectors (static correction modes) are not added.
- Frequency-dependent moduli (`LossFactor::Table`) in the eigenproblem; non-symmetric stiffness (follower forces) on the solid
  path (the linalg routines take any real `C`, `K`).
- The transformation path (linear constraints, rigid sets, supports) and shells and beams through `damped_modes` (the elimination
  path of `modal_analysis` only).
- Dense algebra: `O((2m)^3)` (with loss factors `O((4m)^3)`); models are reached only through the modal subspace, never directly.
- Left eigenvectors and eigenvalue condition numbers are not computed; the defect report is a heuristic on right vectors.

## Amendment 1 (2026-11-14, written and committed after the registered runs of G1 to E1 and before any code change or run it describes)

**Finding.** The R1 run printed "4 defective groups" for a damped cantilever with independent modes. A diagnostic in the scratch
folder (`f414-scratch/tools/diag_f1.rs`, run on a copy; not a registered check) on the same cantilever without damping gave 5
defect groups, each the two roots `+/- i w_j` of one mode. The shapes of the repeated bending pairs (y and z planes) are not
parallel (`|cos|` 0.13 to 0.58), so the vectors are right; the report is wrong. **Cause:** D2 applies the gap test of D1.6
relative to the norm of the state matrix (about 1e9 for this model: `K^` carries `w_12^2`), so `|lambda - conj(lambda)| = 2 w`
always passes; and the cosine of the unscaled state vectors `[y; i w y]`, `[y; -i w y]` of a real `y` is `|1 - w^2|/(1 + w^2)`,
within 1e-6 of 1 for `w > 1414 rad/s`. The report depends on the units of time. No registered check covered it (Q1b has `w = 3`).
D1.6 of `general_eigen` (a given matrix, a gap relative to `||A||_F`) is not affected.

**Correction (D2, defect report).** In `quadratic_eigen_with` two roots are linked only when `|lambda_j - lambda_k| <= DEFECT_GAP
max(|lambda_j|, |lambda_k|)` (dimensionless) and `1 - |z_j^H z_k| <= DEFECT_COSINE` on the unit state vectors as before. A
critically damped pair splits by about 1.5e-8 `|lambda|` (linked); the two roots of an oscillation differ by `2 w sqrt(1 -
zeta^2)`, relative `2 sqrt(1 - zeta^2)` (linked only within `zeta > 1 - 1.25e-7` of critical, a genuine near-defect). Nothing
else changes; `general_eigen` keeps D1.6.

**New check Q4 (limits fixed now; run once on the amended code):**

| Id | Check | Limit | Estimate |
|---|---|---|---|
| Q4a | `quadratic_eigen`, `M = I`, `C = 0`, `K = diag(w^2)`, `w = 1e2, 1e3, 1e4, 1e5` rad/s: no defect | none reported | holds |
| Q4b | the R1 cantilever with R1's Rayleigh damping through `damped_modes`: `defects` empty, no note on defects | none reported | holds |
| Q4c | one oscillator at `zeta = 1`, `w = 3` and `w = 3000`: one defect of 2, both roots within 1e-6 `w` of `-w` | 1e-6 | 1.5e-8 |
| Q4d | the registered tests `sa_eigen_general` and `sa_damped` pass on the amended code (G4, Q1b and every other check) | pass | holds |

The registered results of G1 to E1 stand as run (before this amendment). Mutation MU10 (defect report disabled) is run again on
the amended code. By the decision rule this amendment adds a check and corrects an unregistered output; it does not re-run a
registered check to change its result.

## Addendum 1: results (2026-11-14)

Registered runs: `cargo test --release -p rcfd-linalg --test sa_eigen_general` and `cargo test --release -p rcfd-fem --test
sa_damped` on the content of commit 3d11c904 (logs `f414-scratch/run_linalg.log`, `run_fem.log`), each made once. The Q4 run of
amendment 1 on 094fe9fe (`run_q4_linalg.log`, `run_q4_fem.log`).

| Id | Limit | Measured | Met |
|---|---|---|---|
| G1 companion roots | 1e-10 | 1.8e-15 | yes |
| G1b companion residuals | 1e-10 | 2.7e-16 | yes |
| G2 random order 50, residuals | 1e-10 | 8.5e-16 (96 iterations) | yes |
| G2b 50 values, exact conjugate pairs, no defect | exact | 50 values, 46 complex, pairs exact, 0 defects | yes |
| G3 symmetric order 40 against Jacobi; `max|Im|` | 1e-10 each | 4.0e-15; 0 | yes |
| G4a `J_4(2)`: values; one defect of 4 | 1e-6 | 0; one group of 4 | yes |
| G4b `Q J_2(1.5) Q^T`: values; defect; mean | 1e-6 | 3.1e-9; the two indices; 1.3e-15 | yes |
| G4c `Q J_3(-0.5) Q^T`: one defect of 3; mean | 1e-6 | one group of 3; 0 (individual values 2.6e-6, recorded) | yes |
| G4d `diag(1, 1, 2)`, `I`: no defect, exact | exact | no defect, exact | yes |
| G5 cap 3 | error | `NotConverged`, 3 iterations, rows 0 to 49 | yes |
| G6 cyclic permutation of order 6 | 1e-10 | 1.2e-15 (21 iterations) | yes |
| G7 NaN, size | error | `NonFinite { A, 2, 1 }`, `DimensionMismatch` | yes |
| Q1 oscillator `zeta` 0.05, 0.5, 0.95 | 1e-12 | 0, 0, 1.5e-16 | yes |
| Q1b `zeta = 1`: roots; one defect of 2 | 1e-6 | 3.0e-16; one group of 2 | yes |
| Q2 skew coupling (i), (ii) | 1e-12 | 6.9e-16, 1.2e-16 | yes |
| S2 against `gyroscopic_state_eigen_dense` (i), (ii) | 1e-12 | 1.2e-15, 6.4e-16 | yes |
| Q3 indefinite `M` | error | `MassNotPositiveDefinite { row 1, pivot -3 }` | yes |
| R1 Rayleigh ratios | 1e-8 | 3.5e-14 | yes |
| R2 reported once; shapes `x^H M x` | exact; 1e-10 | 12 reported, all oscillating, `Im > 0`; 8.9e-16 | yes |
| B1 `c = Z/2`, `j = 1, 2, 3` | 2e-2 | 1.45e-3, 1.37e-3, 1.21e-3 (estimate 1.7e-3) | yes |
| B2 `c = 2 Z` (corrected form), `j = 0` real, `j = 1, 2, 3` | 2e-2 | 5.9e-3, 5.9e-3, 6.1e-3, 6.3e-3 (estimate 6.7e-3) | yes |
| B2b real roots flagged and said; none at `Z/2` | exact | 2 real roots (the continuum one and the fast truncation root), ratio 1, note present; 0 at `Z/2` | yes |
| B3 error of `lambda_1` on 10, 20, 40 modes | decreasing | 6.6e-3, 3.2e-3, 1.45e-3 (estimate 6.8e-3, 3.4e-3, 1.7e-3) | yes |
| L1 loss factor 0.04: 12 roots; error | 1e-10 | 12; 3.8e-13 | yes |
| T1 threads 1 and 4 | bits | identical (7152 words) | yes |
| N1 table, unknown set, negative dashpot, transformation path | error | four `InvalidValue`, each naming the cause | yes |
| X1 `x1_modules_and_pending_after_f414` | pass | pass (the former test kept as an ignored record) | yes |
| E1 rcfd-linalg (all tests), rcfd-fem `sa_harmonic_direct`, `sa_scaffold`, `si_rotating_whirl_state` | pass | all pass | yes |
| Q4a undamped `w = 1e2` to `1e5`: no defect (amendment 1) | none | none | yes |
| Q4b R1 cantilever: no defect (amendment 1) | none | none, no note | yes |
| Q4c `zeta = 1`, `w = 3`, `3000` (amendment 1) | 1e-6 | one group of 2 each; 3.0e-16, 1.5e-16 | yes |
| Q4d both test files on the amended code (amendment 1) | pass | pass; every printed number of G1 to T1 equal to the registered run | yes |

Mutations (scratch copy `f414-scratch/copy`, script `tools/mutate.py`, logs `MU*.log`): MU1 caught by G6 (`NotConverged`);
MU2 by G1, G1b, G2 (4.5e-8, 5.5e-9, 1.3e-7); MU3 by G1b, G2 (2.3e-2, 2.4e-2), G4a, G4c, Q1b; MU4 by G1b (0.29); MU5 by Q1, Q1b;
MU6 by B1 (0.18), B2, B2b; MU7 by R2, L1, B2, B2b; MU8 by L1 (4.0e-2); MU9 by R1 (0.25); MU10 by G4a to G4c, Q1b, and on the
amended code also by Q4c. Not registered: MU11 (the gap of amendment 1 put back to an absolute 1e12) caught by Q4a. **Ten of ten
registered mutations caught.**

Deviations. (1) D1.5: a 2 x 2 block of the back-substitution is solved by Gaussian elimination with partial pivoting, and a
pivot (not the determinant) below `eps ||T||_F` is replaced: the "divisor" of D1.5 in a dimensionally consistent form. (2) Three
clippy findings were fixed after the registered run (a late `let`, and two `*x = *x * p` written as `*x *= p`; `MulAssign` is
`*self = *self * o`): bit-identical, confirmed by a run whose printed output equals the registered one line for line
(`confirm_*.log`). (3) Amendment 1, above: a design flaw of the defect report of D2, found by a diagnostic after the registered
runs (not by a registered check), corrected with the new check Q4. (4) `harmonic_direct.rs` gains a `pub(crate)` accessor of
`Dashpot` (lines 152 to 156; additive). (5) At `c = 2 Z` two real roots are reported: the continuum root `j = 0` and a fast
root of the truncation (`s ~ -(2c/Z) N`, as estimated); B2 takes the one of smallest modulus, as registered.

Not run: the whole workspace (the lead's gate); `baseline_hashes` (no file of rcfd-fv or rcfd-api changed; no default path
changed). `cargo fmt --check` and `cargo clippy --all-targets -D warnings` on rcfd-linalg and rcfd-fem: clean.
`rcfd-cli --test docs_site`: pass (docs/reference-data.md gained the F-414 rows).

**Decision by the registered rule: done.** Every registered check G1 to E1 was met at its registered run, every registered
mutation was caught, and no check was met only after an amendment made after its run (Q4 was registered by amendment 1 before
its run and met at it). Disclosed for the maintainer's judgement: amendment 1 corrects an output (the quadratic defect report)
that no registered check covered. If that is read like ADR 0284 amendment 2, the status would be partial until it is accepted.

Remaining questions:

1. Move the whirl state solver (ADR 0416) onto `general_eigen`? Options: (a) leave it (two Francis implementations, the whirl
   bits unchanged); (b) move it (one implementation; the whirl results change in the last bits, so the stored whirl values need
   the maintainer). Recommendation: (a) now, (b) when F-415/F-416 touch that code anyway.
2. The defect report uses right vectors only (a heuristic). Add left vectors and eigenvalue condition numbers? Options: (a)
   leave; (b) a follow-up. Recommendation: (a) until a user needs condition numbers.
3. Boundary dampers near the impedance converge slowly on undamped modes (`1/|1 - tanh^2|`). Options: (a) document; (b)
   residual (static correction) vectors. Recommendation: (b), as the follow-up below.

### For the lead: roadmap entries

F-414, proposed status **done**:

> Structural solver, advanced tier (plan ADR 0310). ADR 0450, 2026-11-14: rcfd_linalg::eigen_general: general_eigen of a dense real matrix by balancing (powers of 2), Householder Hessenberg reduction, Francis double-shift QR with exceptional shifts (cap 30 n iterations, NotConverged otherwise), real 2 x 2 blocks split, eigenvectors by back-substitution, and a defect report (numerically parallel vectors) for Jordan blocks; quadratic_eigen of (lambda^2 M + lambda C + K + i H) x = 0 by the Cholesky factor of M and the state matrix (a complex stiffness through the real embedding). rcfd_fem::damped::damped_modes on m undamped modes with Rayleigh damping, F-412's dashpots and constant loss factors; each conjugate pair reported once, real roots reported as non-oscillating, damping ratio -Re/|lambda|. Companion roots within 1.8e-15 (limit 1e-10); order 50 residuals 8.5e-16 (1e-10); symmetric order 40 equals the Jacobi values of F-382 within 4.0e-15 (1e-10); Jordan blocks J_2 and J_3 under similarity returned with their defect report, mean within 1.3e-15 (1e-6); the cap is an error; the cyclic permutation of order 6 needs the exceptional shifts; oscillator within 1.5e-16 (1e-12), critical damping reported as a defect; Rayleigh ratios on a Hex8 cantilever within 3.5e-14 (1e-8); skew coupling within 6.9e-16 (1e-12) and equal to the whirl solver of F-505 within 1.2e-15; bar with a tip dashpot c = Z/2 (Z = A sqrt(E rho)) on 40 modes: first three roots within 1.45e-3 (2 percent). The draft's clause for c > Z was wrong: the roots are (c0/L)(-atanh(Z/c) + i j pi), one real and the rest oscillating at the fixed-fixed frequencies; registered in that form, at c = 2 Z within 6.3e-3 with the real roots reported as non-oscillating. Constant loss factor 0.04 within 3.8e-13 of i w sqrt(1 + i eta); bit-identical for 1 and 4 threads; ten of ten mutations caught. Amendment 1 corrected the defect report of the quadratic problem (a gap relative to the roots; check Q4) after a diagnostic found it unit-dependent. The transformation path (linear constraints, rigid sets, supports) and frequency-dependent moduli are refused.

Follow-up item, proposed id the next free number, title "Damped modes: residual vectors for boundary dampers near the
impedance", deps F-414:

> Every limit not stated here is registered in the ADR before the run. damped_modes gains static residual vectors (K^-1 times the damper influence vectors, orthogonalised against the undamped modes) as an option. Fixed-free bar with a tip dashpot c = 0.9 A sqrt(E rho): the first three roots (c0/L)(-atanh(c/(A sqrt(E rho))) + i (2 j - 1) pi/2) within 2 percent on 20 modes with the residual vector, against a registered error without it; at c = 2 A sqrt(E rho) the real root -(c0/L) atanh(A sqrt(E rho)/c) within 1 percent on 20 modes; without the option every result is bit-identical to F-414's.

## Lead note, 2026-11-14

F-413 (ADR 0449) and F-414 (ADR 0450) were merged together. Their scaffold copies `x1_modules_and_pending_after_fatigue` and `x1_modules_and_pending_after_f414` are merged into one test, `x1_modules_and_pending_after_f413_f414`. That test marks fatigue, damped and eigen_general as landed. Neither single copy was ever on main. The check is the same X1 row set with both items landed, and no limit changed.
