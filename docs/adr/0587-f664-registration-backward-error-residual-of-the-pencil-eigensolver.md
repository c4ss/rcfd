# ADR 0587: F-664 registration: backward-error residual of the pencil eigensolver

Status: **REGISTERED 2026-11-15, written before any code of F-664 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-664 was written in the repository. No rcfd code was run for this registration (one
`cargo test --no-run` build of the unchanged base, which prints no number). The numbers of earlier runs quoted below come from
ADR 0580 (its addendum 1 and amendment 2), disclosed. The bit records N4 and N5 (D5) are test files written and run on the
unchanged base 847c5881 after this commit and before any library line. Results go in the addenda at the end; nothing above the
addenda is changed after a run.

Roadmap item F-664 (dep F-653): the follow-up proposed by
[ADR 0580](0580-f653-registration-flutter-of-follower-loads-the-unsymmetric-eigenproblem.md) (its remaining question Q1).
Number 0587 given by the lead. Branch `f664-work` from 847c5881, worktree `w49i-work/f664`, scratch `w49i-work/f664-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited
(in particular the F-653 tests and its known-miss records `f1_f2_f3_beck_flutter_known_miss`, `f6_f7_f8_buckling_known_miss`
stay as they are); no new dependency, no unsafe code, the crate graph unchanged; clean-room. Every choice the maintainer would
normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation) and listed at
the end.

## What F-664 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. nearest_pencil_eigenpairs reports the normwise backward error ||A x - lambda B x|| / ((||A|| + |lambda| ||B||) ||x||) of each pair (and the convergence test is stated in it); on Beck's column of ADR 0580 and its buckling problem every pair is within a registered limit derived from the tolerance; the eigenvalues and modes of ADR 0580 keep their bits.

Clauses: (b0) every limit registered here; (b1) `nearest_pencil_eigenpairs` reports the normwise backward error of each pair;
(b2) the convergence test is stated in terms of it (in the documentation of the function: the test, and the bound it gives on
the backward error); (b3) on Beck's column of ADR 0580 (the flutter evaluations) every pair is within a registered limit
derived from the tolerance; (b4) likewise on its buckling problem; (b5) the eigenvalues and modes of ADR 0580 keep their bits.

Readings, registered:

- **(b1), the norm.** The acceptance leaves `||A||` open. Taken: the Frobenius norm of the stored matrix and the 2-norm of the
  vectors. With these the formula is the exact backward error (arithmetic 1): the smallest `eps` for which `(A + dA) x = lambda
  (B + dB) x` with `||dA||_F <= eps ||A||_F`, `||dB||_F <= eps ||B||_F`. The 2-norm of the matrices would be the natural norm
  but costs an iteration; the 1-norm (the diagnostic of ADR 0580 amendment 2) has no such exact meaning for an unsymmetric
  matrix. Open question 1.
- **(b1), "reports".** A new field `backward_errors: Vec<f64>` of `PencilEigen` (which is `#[non_exhaustive]`), one per pair
  in the order of `values`. The existing field `residuals` keeps its measure and its bits (b5 and the brief: the F-653
  results are not edited). The field is carried into `follower::ComplexModes` and `follower::ComplexBuckling` (both
  `#[non_exhaustive]`) under the same name, as the backward error of the reduced pencil that was solved. Audit (done before
  this text, by grep): no test or golden constructs or compares `PencilEigen`, `ComplexModes` or `ComplexBuckling` as a
  whole (the F-653 tests read fields; the digest record N1 of ADR 0580 hashes the symmetric paths only), so the additive
  fields change no existing comparison.
- **(b3), "Beck's column of ADR 0580".** Every evaluation of the bisection of ADR 0580 D5 (`tangent_modes_full` at `p = 16`,
  `24` and the 30 midpoints, 6 modes, default tolerance `1e-10`, the same rule), all returned pairs. **(b4)**: the problems of
  ADR 0580 F6 (dead load, 2 factors) and F7 (the follower, 4 factors) by `buckling_factors_full`, default tolerance `1e-10`.
- **(b5).** The convergence test and every existing computed number are unchanged by construction (the new quantity is
  computed after the eigenpairs, from them); checked by the new bit records N4 (rcfd-fem: every evaluation of (b3) and the two
  problems of (b4): eigenvalues, modes, residuals, steps) and N5 (rcfd-linalg: the pencils of ADR 0580 K1 and K2 and a new
  complex pencil: values, vectors, residuals, estimates, steps), written and run on the base before any library line, and by
  the existing tests of F-653 (passing tests and N1).

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0580 (whole); ADR 0572 and 0573 (the form of "For the lead").
- `crates/rcfd-linalg/src/eigen_pencil.rs` (whole), `eigen_general.rs` (`GeneralEigen`: unit 2-norm vectors, `normalise`),
  `matrix.rs` (`CsrMatrix::from_triplets`: sorted, duplicates summed).
- `crates/rcfd-fem/src/follower.rs` (`ComplexModes`, `ComplexBuckling`, `tangent_modes_full`, `buckling_factors_full`),
  `modal.rs` and `buckling.rs` (the default tolerances `1e-10`).
- Tests: `sa_f653.rs` (whole: the Beck set-up, the bisection, the diagnostic of amendment 2), `sa_f653_default_bits.rs`,
  `sa_eigen_pencil_f653.rs` (K1 to K3).

## Arithmetic done before the code (disclosed)

1. **The normwise backward error** (first principles; the result is known in the literature, e.g. Higham and Higham,
   "Structured backward error and condition of generalized eigenvalue problems", 1998, and Tisseur, "Backward error and
   condition of polynomial eigenvalue problems", 2000, both cited from memory, NOT read; what is used is derived here). Let
   `r = A x - lambda B x`, `alpha = ||A||_F`, `beta = ||B||_F`, `s = alpha + |lambda| beta`. If `(A + dA) x = lambda (B + dB)
   x` with `||dA||_F <= eps alpha`, `||dB||_F <= eps beta`, then `r = -(dA - lambda dB) x`, so `||r|| <= (||dA||_2 + |lambda|
   ||dB||_2) ||x|| <= eps s ||x||` (the 2-norm is at most the Frobenius norm), hence `eps >= eta = ||r|| / (s ||x||)`. The
   bound is attained by the rank-one pair `dA = -(alpha/s) r x^H / ||x||^2`, `dB = (beta conj(lambda)/(s |lambda|)) r x^H /
   ||x||^2` (`dB = 0` if `lambda = 0`): `(dA - lambda dB) x = -(alpha/s) r - (beta |lambda|/s) r = -r`, and `||dA||_F = alpha
   ||r||/(s ||x||) = eta alpha`, `||dB||_F = eta beta` (a rank-one matrix has equal 2- and Frobenius norms). So `eta` is exact
   for Frobenius-norm perturbations relative to `||A||_F`, `||B||_F`. Against the 2-norm version `eta_2` (with `||A||_2`,
   `||B||_2`): `eta <= eta_2 <= sqrt(n) eta`.
2. **The convergence test and the bound it gives** (first principles, from the Arnoldi relation of ADR 0580 arithmetic 4).
   `T = (A - sigma B)^-1 B`, a Ritz pair `(theta, x = V_m y)`, `||y|| = 1` (the vectors of `general_eigen` are unit 2-norm),
   `lambda = sigma + 1/theta`. The identity `A x - lambda B x = -(1/theta) (A - sigma B) (T x - theta x)` (expand: `(A - sigma
   B) T x = B x`) and `||T x - theta x|| = |h_(m+1,m)| |e_m^T y|` in exact arithmetic give `||r|| <= ||A - sigma B||_2
   est`, `est = |h_(m+1,m)| |e_m^T y| / |theta|` (the field `estimates`), and `||A - sigma B||_2 <= alpha + |sigma| beta`:
   **`eta <= rho est`, `rho = (alpha + |sigma| beta)/(alpha + |lambda| beta)`.** The test accepts when every returned pair has
   `est <= tolerance` (or the Krylov space is the whole space, `est = 0`), so in exact arithmetic `eta <= rho tolerance`; at
   shift 0 (every problem of b3 and b4) `rho = alpha/(alpha + |lambda| beta) < 1` and `eta < tolerance`.
3. **Rounding** (an estimate, constants stated, not derived to the digit). (i) The factor: each solve satisfies `(A - sigma B +
   E) w = B v` with `||E|| <= c_1 eps ||A - sigma B||` (`c_1` covers the growth of the unpivoted `EnvelopeLu` and the band
   width); the Ritz combination of the columns `w_j` is `theta x` up to the `h` term, so the term adds `~ c_1 eps` to `eta`,
   independent of the spread. (ii) The dense eigenvector: `||H y - theta y|| ~ c_2 eps ||H||_F <= c_2 eps sqrt(m) |theta_1|`
   (`theta_1` the Ritz value of largest modulus), which adds `c_2 eps sqrt(m) S` to `est` and to `eta`, `S = |theta_1|/|theta| =
   |lambda - sigma| / min_k |lambda_k - sigma|` over the returned pairs. (iii) The residual's own evaluation: `<= k eps` (`k`
   the entries in a row, `<= 81 x 3` for Hex20), negligible. Registered: `c_1 = 1e4`, `c_2 = 100`, so `delta = eps (100
   sqrt(m) S + 1e4)`, `eps = 2.22e-16`. For the flutter evaluations (`S <= 12835/49.5 = 260` at `p = 16` by ADR 0580 F1e,
   `m <= 200`): `delta <= 8.4e-11`; for the buckling factors (`S ~ 1.0` to `1.1`): `delta ~ 2.5e-12`.
4. **The limit of b3 and b4, derived from the tolerance:** `eta <= rho tolerance + delta <= 1e-10 + 8.4e-11 < 2e-10`.
   **Registered: `eta <= 2 tolerance = 2e-10` for every pair.** Estimate: ADR 0580 amendment 2 measured the same pairs' 1-norm
   backward error at `5.2e-17 .. 2.5e-12` (disclosed); the Frobenius norm of these FE matrices is larger than their 1-norm
   (`~ sqrt(n/k)` times for `n ~ 1.5e3` unknowns and `k ~ 1e2` entries per row), so `eta` is expected below `2.5e-12`, a
   margin of about 80 to the limit.
5. **The other checks.** B3/K5 (the stated bound per pair): `eta_j <= rho_j est_j + delta_j` (arithmetic 2 and 3), the
   estimate of the left side ~`1e-12`, the right side `>= 2.5e-12`. B4/K4 (the library's number against an independent
   recomputation from the matrices by the test: a different summation order): `~1e-15` relative, limit `1e-12`. K6 (the
   rank-one perturbation of arithmetic 1 applied to the returned pair, dense complex arithmetic, order 60): the perturbed
   residual `~ n eps (alpha + |lambda| beta) ||x||`, limit `1e-13 (alpha + |lambda| beta) ||x||`; `||dA||_F / (eta alpha)` and
   `||dB||_F / (eta beta)` equal 1 to `~1e-15`, limit `1e-12`.

## Design

### D1. The backward error in `nearest_pencil_eigenpairs` (`rcfd-linalg/src/eigen_pencil.rs`)

- `PencilEigen` gains `pub backward_errors: Vec<f64>`: `eta = ||A x - lambda B x|| / ((||A||_F + |lambda| ||B||_F) ||x||)`
  of each pair (2-norms of the vectors), in the order of `values`. `||A||_F`, `||B||_F` are computed once, the squares of the
  stored values summed row by row in storage order. `eta = 0` when the denominator is 0 and `r = 0`; the residual norm when the
  denominator is 0 (as for `residuals`). The residual vector `r` is the one `finish` already forms; no new matrix product.
- Nothing else changes: the convergence test, the order of operations that produce `values`, `vectors`, `residuals`,
  `estimates`, `steps`.
- The module and function documentation state the convergence test (`est <= tolerance` for every returned pair, `est =
  |h_(m+1,m)| |e_m^T y| / |theta|`) and the bound of arithmetic 2 (`eta <= rho est`, so `eta <= tolerance` at shift 0 in exact
  arithmetic), and the meaning of arithmetic 1.

### D2. `follower::ComplexModes`, `follower::ComplexBuckling` (`rcfd-fem/src/follower.rs`)

Each gains `pub backward_errors: Vec<f64>` (the pencil's, of the reduced pencil `K_T phi = omega^2 M phi`, resp. `K x = mu B x`,
`B = -(K_sigma - sum K_p)`); the two constructors copy `eig.backward_errors`. No other line changes.

### D3. Tests

- `crates/rcfd-fem/tests/sa_f664.rs`: the Beck model and the bisection of ADR 0580 D5 (a copy of its set-up; the F-653 file is
  not edited), B1 to B4.
- `crates/rcfd-linalg/tests/sa_eigen_pencil_f664.rs`: K4 to K7, on the K1 and K2 pencils of ADR 0580 (copied constructions)
  and K5c, a new pencil with complex eigenvalues nearest the shift: order 60, `B = diag(1 + u_i)`, `A = B Q D Q^-1`, `Q = I +
  L` (`L` strictly lower, entries uniform in `(-0.1, 0.1)`), `D` block diagonal with 10 blocks `[[a, c], [-c, a]]`, `a`
  uniform in `(0.1, 1)`, `c` in `(0.05, 0.5)`, and 40 real values uniform in `(2, 5)` (seed `0x0587_0001`), 6 pairs nearest
  shift 0 (so complex pairs are among them).

### D4. What is not changed

No line of `tangent_modes`, `buckling_factors`, `buckling.rs`, `nonlinear.rs`, `modal.rs`; the existing F-653 tests and records.

### D5. The bit records N4, N5

Written after this commit with the API of the base only and run on the unchanged base 847c5881 before any library line; their
FNV-1a 64 digests (over the `to_bits` of the numbers, as N1 of ADR 0580) written into the files then.

- N4 `crates/rcfd-fem/tests/sa_f664_default_bits.rs`: (1) the bisection of ADR 0580 D5 (each evaluation's eigenvalues, modes,
  residuals and steps, in order, one digest over all 32), (2) `buckling_factors_full` dead load, 2 factors, (3) with the
  follower, 4 factors (factors, modes, residuals, steps).
- N5 `crates/rcfd-linalg/tests/sa_eigen_pencil_f664_default_bits.rs`: the K1 pencil (shift 0.3, 8), the K2 pencil (shift 0, 5)
  and K5c (shift 0, 6): values, vectors, residuals, estimates, steps.

## Checks (limits fixed now)

| Id | Clause | What | Limit | Estimate behind the limit |
|---|---|---|---|---|
| B1 | b3 | every pair of every evaluation of the ADR 0580 bisection (32 evaluations, 6 modes): `max eta` (from `ComplexModes::backward_errors`) | `<= 2e-10` (`2 tolerance`) | `< 2.5e-12` (arithmetic 4) |
| B1n | b1 | the same: `backward_errors.len() == eigenvalues.len()` for every evaluation | holds | |
| B2 | b4 | the ADR 0580 F6 (dead, 2) and F7 (follower, 4) pairs by `buckling_factors_full`: `max eta` | `<= 2e-10` | `< 2.5e-12` |
| B3 | b2 | the stated bound on every pair of the pencils of B4 (see the note): `max_j eta_j / (rho_j est_j + delta_j)`; the direct calls reproduce the follower functions' values to the bit | `<= 1`; holds | `~0.1 .. 0.5` |
| B4 | b1 | the library's `eta` of the pairs of B1 (at `p = 16, 20, 24`) and B2 against the test's recomputation from the reduced matrices assembled through the public API (as ADR 0580's diagnostic, with Frobenius norms): `max |eta_lib/eta_test - 1|` | `<= 1e-12` | `~1e-15` |
| K4 | b1 | the same comparison on K1, K2, K5c by `nearest_pencil_eigenpairs` | `<= 1e-12` | `~1e-15` |
| K5 | b2 | the stated bound on every pair of K1, K2, K5c: `max_j eta_j / (rho_j est_j + delta_j)` with the library's `estimates` and `steps` | `<= 1` | `<= 0.5` |
| K6 | b1 (meaning) | on every pair of K5c: the rank-one `dA`, `dB` of arithmetic 1 (dense complex): `||(A + dA) x - lambda (B + dB) x|| / ((alpha + |lambda| beta) ||x||)`; `max |  ||dA||_F/(eta alpha) - 1 |`, same for `dB` | `<= 1e-13`; `<= 1e-12`; `<= 1e-12` | `~1e-15`; `~1e-15` |
| K7 | b2 | K5c has a complex pair among the returned (`|Im| > 1e-6 |lambda|`) and `max eta <= 2 tolerance max(1, rho)` on K1, K2, K5c | holds; holds | |
| D | b2 | the documentation of `nearest_pencil_eigenpairs` states the convergence test and the bound `eta <= rho est` | holds (by inspection, quoted in the addendum) | |
| N4 | b5 | the digests of D5 (rcfd-fem) at the base | exact | no existing line changed |
| N5 | b5 | the digests of D5 (rcfd-linalg) at the base | exact | |
| N6 | b5 | the existing tests of F-653 (`sa_f653` non-ignored tests, `sa_f653_default_bits`, `sa_eigen_pencil_f653`) | pass | |
| N7 | b5 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | |

Note on B3: `ComplexModes` and `ComplexBuckling` carry no `estimates`, and adding them is not asked. B3 is therefore
registered on the pencils the test assembles itself (the reduced matrices of B4, at `p = 16, 20, 24` and the two buckling
problems) by calling `nearest_pencil_eigenpairs` directly with the options the follower functions use (shift 0, tolerance
`1e-10`, default steps and seed); these calls reproduce the library's pairs (checked: their values equal the
`ComplexModes`/`ComplexBuckling` values to the bit, a part of B3: `holds`), and their `estimates` and `steps` enter `rho_j
est_j + delta_j` with `delta_j` of arithmetic 3. Limit `<= 1`.

A check that would take more than about two minutes in release is an ignored heavy record run once, with a fast test on the
same code path.

## Planned mutations (on a scratch copy, one at a time)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | `eta`'s denominator `||A x|| + |lambda| ||B x||` (the old relative measure) | B1 (`~2e-8` by ADR 0580 F3), B4, K4 |
| MU2 | `|lambda|` dropped from the denominator | B4, K4, K6 |
| MU3 | the Frobenius norms without the square root | B4, K4, K6 |
| MU4 | the convergence test accepts at `1e3 tolerance` | N4, N5 (bits) |
| MU5 | `residuals` set to the new `eta` | N4, N5 |
| MU6 | `tangent_modes_full` copies `eig.residuals` into `backward_errors` | B1, B4 |

## Decision rule

- **done**: every check of the table met, every planned mutation caught by at least one check, no check met only after a
  post-run amendment, fmt and clippy clean, N4 to N7 unchanged.
- **partial**: N4 to N7 unchanged and fmt and clippy clean, but a registered check missed (kept as a `*_known_miss` test with
  its limit and the measured number), or a mutation not caught (none is declared equivalent), or a check met only after a
  post-run amendment.
- **todo**: otherwise, in particular any change of the bits of N4 to N7.

## Limits (not covered)

- The Frobenius norm, not the 2-norm: `eta` may be below the 2-norm backward error by at most `sqrt(n)` (arithmetic 1).
- The backward error is of each pair alone (unstructured: `dA`, `dB` complex and dense rank one), not a structured one (real,
  sparse, or keeping the symmetry of `M`); it says nothing on the forward error of a nearly defective pair (the coalescence),
  whose eigenvalue error is `~sqrt(eta)` relative (ADR 0580 arithmetic 4).
- The bound `eta <= rho est` is an exact-arithmetic statement; the rounding term `delta` is an estimate with stated constants.
- The convergence test itself is not changed to the backward error (the brief and b5: the bits are kept); open question 2.
- No case-file key.

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. **The norm.** Taken: Frobenius (exact meaning by arithmetic 1, one pass over the stored values), rather than the 2-norm (an
   iteration and a tolerance of its own) or the 1-norm (no exact meaning for an unsymmetric pencil).
2. **The convergence test.** Kept (the Ritz estimate of `T`), stated with its bound on `eta`; a test on `eta` itself would
   change the bits of ADR 0580 (b5) and is not taken.
3. **The field on the follower results.** Taken: `backward_errors` on `ComplexModes` and `ComplexBuckling` too, so that
   (b3)/(b4) are read from the functions ADR 0580 used, additive on `#[non_exhaustive]` structs.
