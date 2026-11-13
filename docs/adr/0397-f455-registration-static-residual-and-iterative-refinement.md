# ADR 0397: F-455 registration: the residual of the direct static solve on slender cantilevers, and one step of iterative refinement

Status: **REGISTERED 2026-11-13, written before any code of F-455 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-455 was written, compiled or run: the files listed under "What was read" were read and
the arithmetic below was done by hand from numbers already recorded in ADRs 0277 and 0282 (disclosed under "Arithmetic"). Results
go in the addendum at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0277](0277-stress-and-strain-recovery-von-mises-principal.md) remaining question 1 (the Tet10 cantilever of
`L/h = 10` left a direct-solve relative residual of 6.7e-11), [ADR 0282](0282-preconditioned-cg-for-elasticity.md) amendment 4 and
record X1 (on Hex8 cantilevers the direct solve's relative residual sits at the evaluation floor `eps || |A| |x| || / ||b||`).
Roadmap item F-455 (deps F-374, F-376). Branch `f455-static-refinement` from main e9f76a1e, worktree `w49i-work/f455`, scratch
`w49i-work/f455-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs and not changed after a run (a miss
becomes an ignored `*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the hash tables or of any golden file; no new dependency, no unsafe code; clean-room.

## What F-455 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. The relative residual of the default direct static solve
> is recorded for Tet4, Hex8 and Tet10 cantilevers of slenderness 5, 10, 20 and 40 at registered meshes; with one step of
> iterative refinement (opt-in, the default bit-identical) the relative residual is at most 1e-14 on every one of them and the
> strain energy equals half the work of the loads within 1e-12; the cost of the refinement step is recorded.

Clauses: (a1) the twelve cantilevers (three kinds, `L/h` = 5, 10, 20, 40) at meshes registered here; (a2) the relative residual
of the default direct solve recorded on each; (a3) one step of iterative refinement, opt-in; (a4) the default bit-identical;
(a5) after the step the relative residual `<= 1e-14` on each; (a6) after the step the strain energy equals half the work of the
loads within 1e-12 on each; (a7) the cost of the step recorded.

The central point of this registration is clause a5. Arithmetic 1 to 3 below derive that **a relative residual of 1e-14,
measured against `||b||`, is below what any double-precision displacement vector can have on these models**, whatever
algorithm produced it and however exactly the residual is evaluated; and that, evaluated in working precision (the number
`SolverStats::relative_residual` stores), it is further below the evaluation floor. Clause a5 is therefore registered, with its
limit 1e-14 intact, as an **expected miss** on every case with the number predicted below (one case, Tet4 at `L/h = 5`, is the
closest, predicted a factor of about 4 above the limit with a range that does not reach it). The checks that do say whether the
refinement did its job are registered beside it with limits derived here (C2, C3), and clause a6, which is attainable, is
checked with its own limit.

## What was read (no run)

- ADR 0277 in full, in particular W1 (strain energy against half the work, measured 2.6e-12 on the Hex8 and 1.5e-11 on the
  Tet10 bending cantilever), its arithmetic 13 (`u^T K u = u^T f + u_F^T (K_FF u_F - f_F)`) and remaining question 1.
- ADR 0282 arithmetic 7, amendment 4 (the componentwise evaluation floor), addendum record X1 (direct solve: relative residual
  6.5e-12 against a floor of 1.3e-11 on `[32, 6, 6]`, 7.1e-11 against 1.4e-10 on `[64, 6, 6]`).
- `crates/rcfd-fem/src/solve.rs` (the static solve; `direct`, `residual_norm`, `StaticOptions`, `SolveReport`),
  `crates/rcfd-linalg/src/skyline.rs` (`SkylineLdlt::factor`, `solve`), `crates/rcfd-linalg/src/matrix.rs` (`CsrMatrix`, public
  fields, `residual` summing each row sequentially), `crates/rcfd-fem/src/constraint.rs` (`ReducedSystem`, `reduce`),
  `crates/rcfd-fem/src/recover.rs` (`IntegrationPointField::strain_energy`), `crates/rcfd-fem/tests/sb_recover.rs` (the
  cantilever of ADR 0277 arithmetic 14: `box_of`, `model_of`, `cantilever`, `w1_w2_w4_strain_energy_and_n2`), the uses of
  `SolveReport` and `StaticOptions` in `mpc.rs` (constructs a `SolveReport` literal, so that struct gains no field) and in
  `rcfd-post`.
- `crates/rcfd-api/Cargo.toml`: rcfd-api depends on rcfd-linalg (and, through rcfd-post, on rcfd-fem). This item changes no
  file of rcfd-linalg (D3).

## Design

### D1. The measures (registered precisely)

All on the reduced system `A u = b` of the static solve (`A = K_FF`, `b = f_F - K_FP g`; here `g = 0`), `u` the free
displacements as stored in double precision. `eps = 2^-52 = 2.22e-16` (`f64::EPSILON`), `u_r = eps/2` the unit roundoff.
Norms are 2-norms over the free equations.

- `rho_w = ||b - A u|| / ||b||` with `b - A u` evaluated by `CsrMatrix::residual` in working precision: exactly the existing
  `residual_norm` of `solve.rs`, the number stored in `SolverStats::relative_residual`. This is the acceptance's "relative
  residual" as the repository has always reported it.
- `rho_x = ||r_x|| / ||b||`, `r_x` the residual evaluated in compensated arithmetic (D2), each row accurate to about `u_r |r_i|
  + n_i^2 u_r^2 (|b_i| + sum_j |a_ij u_j|)` and rounded once to double. This is the true residual of the stored `u` to working
  accuracy.
- `phi_w = eps || |A| |u| || / ||b||`, `(|A| |u|)_i = sum_j |a_ij| |u_j|`: the evaluation floor of `rho_w` (ADR 0282 amendment 4).
- `phi_rep = (u_r / sqrt 3) || s || / ||b||`, `s_i = (sum_j a_ij^2 u_j^2)^(1/2)`: the representability floor of `rho_x`
  (arithmetic 2).
- `omega = ||r_x|| / (|| |A| |u| || + ||b||)`: a normwise backward-error statistic, the one ADR 0282's note names
  ("relative to `|| |A| |x| || + ||b||`"); it has the form of the Rigal-Gaches backward error `||r|| / (||A|| ||u|| + ||b||)`
  (from memory) with `||A|| ||u||` replaced by `|| |A| |u| ||`, which is what the evaluation of a residual can resolve. It is
  used here as a statistic only, not as a theorem.
- `E_K = |u^T A u - b^T u| / (b^T u) = |u^T r_x| / (b^T u)`, the products by `dot2` (D2): the defect of "the strain energy
  `U = u^T A u / 2` equals half the work `W = b^T u`", `|U - W/2| / (W/2)`, with `U` the discrete strain energy of the
  assembled stiffness (arithmetic 13 of ADR 0277).
- `E_rec = |U_rec - W/2| / (W/2)`, `U_rec = IntegrationPointField::strain_energy()` (`sum w det J eps . sigma / 2` over the
  stiffness rule) and `W = f . u` over all equations, both in working precision: ADR 0277's W1 measure, unchanged.

### D2. Compensated kernels (first principles; Knuth, Dekker, Ogita-Rump-Oishi)

- `two_sum(a, b) = (s, e)`, `s = fl(a + b)`, `e = (a - (s - b')) + (b - b')` with `b' = s - a`: `a + b = s + e` exactly
  (Knuth's TwoSum, from memory; it holds for any two doubles without overflow, by Sterbenz's lemma on the subtractions).
- `two_product(a, b) = (p, e)`, `p = fl(a b)`, `e = fma(a, b, -p)`: `a b = p + e` exactly (the fused multiply-add rounds `a b - p`
  once, and `a b - p` is representable; Rust's `f64::mul_add` is a correctly rounded fused operation on every target, in
  hardware or by its software fallback).
- `dot2(x, y)`: `(p, s) = two_product(x_0, y_0)`; for each further `i`: `(h, r) = two_product(x_i, y_i)`, `(p, q) =
  two_sum(p, h)`, `s = s + (q + r)`; result `p + s`. Ogita, Rump and Oishi, "Accurate sum and dot product", SIAM J. Sci.
  Comput. 26 (2005) 1955 (Algorithm Dot2, from memory) prove `|dot2 - x.y| <= u_r |x.y| + gamma_n^2 sum |x_i y_i|`, `gamma_n =
  n u_r / (1 - n u_r)`: the result is as if computed in twice the working precision and rounded once. The bound is re-derived
  in one line: each step's errors `q` and `r` are captured exactly; the only uncaptured error is the rounding of the sum `s`
  of the `2n` error terms, each at most `u_r` times a partial sum, so of order `n u_r` times `sum |q| + |r| <= n u_r sum |x_i
  y_i|`, giving the `gamma_n^2` term; the final `p + s` adds `u_r |x.y|`.
- `residual_compensated(A, b, x)`: for each row, `dot2` of `(b_i, -a_i1, ..., -a_in)` with `(1, x_1, ..., x_n)` (the right-hand
  side entering the compensated sum first), each row sequential, rows in order; deterministic (no threads).

### D3. The refinement step and the API (all in `rcfd-fem`)

Classical iterative refinement (Wilkinson 1963; Moler, J. ACM 14 (1967) 316; from memory) with the residual in twice the
working precision and the correction from the existing factor:

1. `L D L^T` of `A` as today (`SkylineLdlt::factor`, the pivot checks of `direct` unchanged), `u_0 = solve(b)` (the default
   solution, the same bits).
2. For each step `k = 0 .. steps - 1`: `r_k = residual_compensated(A, b, u_k)`, `d_k = solve(r_k)` (the same factor),
   `u_{k+1} = u_k + d_k` element-wise in double.

API, additive:

- New module `crates/rcfd-fem/src/refine.rs`: `two_sum`, `two_product`, `dot2`, `residual_compensated`, `refine_step(a, b,
  factor, x)` (one step in place; returns the norm of the correction), `ResidualStatistics { rho_w, rho_x, phi_w, phi_rep,
  omega }` with `residual_statistics(a, b, x)`, and `energy_defect(a, b, x)` (`E_K`). Declared in `lib.rs` (`pub mod refine;`).
- `StaticOptions` gains `pub refinement: usize` (steps; default 0) and the builder `refinement(steps)`. `StaticOptions` is
  `#[non_exhaustive]` and no literal of it exists outside `solve.rs` (read), so no caller changes.
- `solve_static_report`: with `refinement == 0` every line runs as before. With `refinement > 0`: the solver must be `Direct`
  (otherwise refused, `FemError::Solver { solver, reason }` naming ADR 0397), the model must not take the transformation path of
  `mpc.rs` (refused likewise, before that path is entered; `mpc.rs` is not touched), and the direct arm factors, solves and
  refines as above; the solver name becomes `"skyline-ldlt-refined"`, `SolverStats::relative_residual` is `rho_w` of the
  refined `u` (the same function as before), the reactions are those of the refined `u`.
- `direct` of `solve.rs` is split into a function returning the checked factor and the call `factor.solve(b)`; the same
  operations in the same order, so the default bits cannot change (checked by A7).

No file of rcfd-linalg changes; `mpc.rs` and `cyclic.rs` (another item's files) are not touched.

### D4. The cantilevers (clause a1)

ADR 0277 arithmetic 14 extended in length: section `h = b = 0.2 m`, length `L = s h` with `s = 5, 10, 20, 40`, the box
`[0, L] x [0, 0.2] x [0, 0.2]`, divisions `[s, 2, 2]` (cells `0.2 x 0.1 x 0.1 m`), Hex8 by `hex_box`, Tet4 by `tet_box` (six
Kuhn tetrahedra per cell), Tet10 by `upgrade_to_tet10` of the Tet4 box (straight edges); `E = 2e11 Pa`, `nu = 0.3`, density
7850 (unused); clamped on `xmin` (three components), traction `(0, 0, -1e6) Pa` on `xmax` (`F = 4e4 N`). The case `s = 10` is
ADR 0277's cantilever exactly (C5 uses it). Free unknowns: Hex8 and Tet4 `27 s` (135, 270, 540, 1080), Tet10 `150 s` (750,
1500, 3000, 6000). Euler-Bernoulli tip deflection `4 F s^3 / (E h)` = 1.0e-4 m at `s = 5` to 5.1e-2 m at `s = 40` (order only;
the coarse sections lock, Tet4 most).

## Arithmetic before this registration (disclosed)

1. **The evaluation floor `phi_w`.** In working precision row `i` of `b - A u` is computed with an error up to `gamma_{n_i+1}
   (|b_i| + (|A||u|)_i)`, statistically about `eps (|A||u|)_i` with random sign (ADR 0282 measured `rho_w / phi_w = 0.50` and
   0.51 for the direct solve on two Hex8 cantilevers). So `rho_w` of *any* `u` is at least about `0.3 phi_w`, refined or not.
   Scaling (ADR 0282 arithmetic 7): `phi_w ~ c (L/h)^3 sqrt(N)` at a fixed section mesh (`|A|` fixed, `u` growing as the tip
   deflection `(L/h)^3`, `||u||` as `sqrt(N)`, `||b||` fixed). Calibration on ADR 0277's Tet10 `s = 10` (this ADR's case):
   `rho_w = 6.7e-11`, so `phi_w ~ 1.3e-10` (ratio 0.5). Tet10 (`N` = 525, 1025, 2025, 4025 nodes): `phi_w ~ 1.2e-11, 1.3e-10,
   1.5e-9, 1.6e-8`. Hex8 from ADR 0282's formula calibrated on its two measured floors (factor 1.75 on arithmetic 7 there):
   `1.75 eps x 8 (a/h) s^3 sqrt(N/3) sqrt(m)` with `a/h = 0.5`, `m = 9`, `N = 9 (s + 1)`: `2.5e-12, 2.7e-11, 3.0e-10, 3.3e-9`
   (an upper value: locking lowers the deflection). Tet4: a third of Hex8 (more locking; the factor is a guess within 1 to
   10): `8e-13, 9e-12, 1e-10, 1.1e-9`. Every `phi_w` exceeds 1e-14 by a factor 80 or more, so **`rho_w <= 1e-14` is
   unattainable on all twelve cases** (A2, expected miss: `rho_w` of the refined `u` predicted at 0.3 to 1 `phi_w`, i.e.
   `>= 2.5e-13`).
2. **The representability floor `phi_rep`.** Let `u*` be the exact solution and `u = fl(u*)` its best double, `u_j = u*_j +
   delta_j`, `|delta_j| <= ulp(u_j)/2 <= u_r |u_j|`. Then the exact residual is `b - A u = -A delta`, row `i` a sum of
   `a_ij delta_j` with signs that do not correlate with the smooth field (the rounding errors of neighbouring nodes are
   independent of each other), so `|(A delta)_i| ~ (sum_j a_ij^2 delta_j^2)^(1/2)`. With `delta_j` uniform on `[-ulp/2, ulp/2]`
   its rms is `ulp(u_j) / (2 sqrt 3)`, between `u_r |u_j| / (2 sqrt 3)` and `u_r |u_j| / sqrt 3`. Hence `rho_x(fl(u*)) ~ (0.5 to
   1) phi_rep`: **no double vector has a true relative residual much below `phi_rep`.** The ratio `phi_rep / phi_w = (u_r /
   sqrt 3)/eps x R = 0.289 R`, `R = || s || / || |A||u| ||` the ratio of the root-sum-square to the absolute sum of a row's
   `|a_ij u_j|`, `1/sqrt(k)` for `k` equal terms; a Hex8 row has 81 entries, a Tet4 row about 45, a Tet10 row about 90 to 120,
   with the diagonal block largest: `R ~ 0.15 to 0.45`, so `phi_rep ~ 0.04 to 0.13 phi_w`, central 0.08. Predicted `phi_rep`:
   Tet4 `6e-14, 7e-13, 8e-12, 9e-11`; Hex8 `2e-13, 2e-12, 2.4e-11, 2.6e-10`; Tet10 `1e-12, 1e-11, 1.2e-10, 1.3e-9`. Every one
   exceeds 1e-14; the smallest, Tet4 `s = 5`, by a factor of about 6 (range 1.6 to 13 with the factor ranges above). So
   **`rho_x <= 1e-14` after refinement is predicted missed on all twelve cases** (A3, expected miss). It is checked as the
   acceptance states it, limit intact.
3. **What one refinement step reaches.** With the default solution's forward error `e_0 = u_0 - u*` and `r_0` evaluated to
   working accuracy (D2), the correction solve gives `d = -e_0 + O(kappa eps |e_0|)` (the backward error of the factor applied
   to the small `d`), so `u_1 = fl(u_0 + d) = u* + delta + O(kappa eps e_0)`: the rounding of arithmetic 2 plus a term
   `kappa eps` times the old error. The condition number of these cantilevers (ADR 0282 arithmetic 8, `kappa ~ 1.6 (a/h)
   (L/h)^3 N` with Jacobi scaling, larger without) is at most about `1e8` at `s = 40` (`kappa eps <= 2e-8`), so one step
   reaches the representability floor: **`rho_x(u_1) ~ (0.5 to 1) phi_rep`** (C2: limit `<= 2 phi_rep`) and **`omega(u_1) =
   rho_x ||b|| / (|| |A||u| || + ||b||) ~ rho_x eps / phi_w ~ 0.08 x 0.75 x 2.2e-16 ~ 1.3e-17`** (C3: limit `<= 1e-14`, margin
   about 500 at the low end of the ranges). With the residual evaluated in working precision instead (fixed-precision
   refinement, Skeel 1980, from memory) `r_0` carries an error of the size of `eps |A||u|`, about the true residual itself, and
   the forward error is not reduced: N1.
4. **The energy defect `E_K`.** `u^T A u - b^T u = -u^T r` exactly. For `u_1`: `r = -A delta + O(...)`, `u^T A delta = (A u)^T
   delta = b^T delta + O(r^T delta)`, and `b` is non-zero only on the `m` tip-face equations of the `z` component (9 for Hex8
   and Tet4, 25 for Tet10), where `u_j` are nearly equal; so `b^T delta` is a random sum of `m` terms of size `u_r b_j u_j /
   sqrt 3`: `E_K(u_1) ~ u_r / sqrt(3 m) = 2e-17` (Hex8, Tet4) and `1.3e-17` (Tet10). Limit 1e-12 (the acceptance's): margin
   `5e4`. The evaluation of `E_K` by `dot2` adds `u_r` relative to `b^T u` and `gamma_n^2` terms: negligible. For the default
   `u_0`: `E_K(u_0) = |e_0^T b + e_0^T A e_0| / (b^T u)` is the load-weighted relative forward error of the solve; calibrated on
   ADR 0277's W1 at `s = 10` (Hex8 2.6e-12, Tet10 1.5e-11, if `E_rec(u_0) ~ E_K(u_0)` there, arithmetic 5) and growing with
   `kappa` (about `(L/h)^3` to `(L/h)^4` at a fixed section): Hex8 `s = 40` `1.7e-10 to 7e-10`, Tet10 `1e-9 to 4e-9`; both
   above 1e-12 (N2). Tet4 has no recorded W1 value; it is recorded only.
5. **The evaluation floor of `E_rec`.** `U_rec` sums `w det J eps . sigma / 2` over the points; the strain `eps = sum_a B_a u_a`
   cancels the near-rigid part of the displacement: relative error per point about `eps C`, `C ~ |u| / (a |eps|) ~ 0.4 (L/h)^2
   (h/a)` at mid-length (`a` the cell size across, `h/a = 2`), so `C ~ 20` (`s = 5`) to `1300` (`s = 40`), at most `3e-13` per
   point at `s = 40`; the energy density is largest at the root where `u` and `C` are small, and the errors of the `N_p` points
   (`8 N_e` for Hex8, `N_e` for Tet4, `4 N_e` for Tet10; 480 to 3840 at `s = 40`) add with random signs: `E_rec` evaluation floor
   `~ 1e-14` (range `1e-15` to `1e-13`). The work `f . u` sums terms of one sign: error `<= n eps` relative worst case, about
   `sqrt(n) eps ~ 1e-14` typical. So **`E_rec(u_1) <~ 1e-13`** (A5, limit 1e-12, margin about 10) and `E_rec(u_0) ~ E_K(u_0)`
   (the defect of the solve dominating; consistent with ADR 0277's 2.6e-12 and 1.5e-11 being far above this floor).
6. **Cost (clause a7, record only).** A step is one compensated residual (`dot2` over a row: about 17 flops per entry instead
   of 2, plus the `fma`; `nnz` per row 81 / 45 / about 100) and one pair of envelope sweeps (`4 E` flops, `E` the envelope
   size), against the factor (`sum_i w_i^2 / 2`, `w_i` the row widths, about 50 for Hex8 and Tet4, about 150 for Tet10 after
   RCM) plus one solve. Ratio step / (factor + solve) `~ (17 nnz + 4 E) / (E w / 2 + 4 E)`: Hex8 and Tet4 0.4 to 1.5, Tet10 0.1
   to 0.4 (whether `mul_add` is a hardware instruction in this build decides the upper ends: the workspace does not enable the
   `fma` target feature, so it is probably the software routine, slower by a factor of 10 or more; the ratio may then reach 5).
   Recorded, no limit (the acceptance asks a record).

## Registered checks

All in `crates/rcfd-fem/tests/si_static_refinement.rs` (new API) and `crates/rcfd-fem/tests/si_static_refinement_defaults.rs`
(old API only, the default digest), release profile, run once each: `CARGO_BUILD_JOBS=4 cargo test -p rcfd-fem --release --test
<file> -- --nocapture --test-threads=1` (the known-miss record with `--ignored`). Every case of D4 is run in one test function;
"each" means all twelve cases unless a subset is named.

### A. The acceptance

| Id | Clause | Measure | Limit | Estimate |
| --- | --- | --- | --- | --- |
| A1 | a1, a2 | `rho_w` of the default solve (`SolverStats::relative_residual` of `solve_static` with `StaticOptions::default()`), and `phi_w`, each case | record | arithmetic 1 (`rho_w` 0.3 to 1 `phi_w`) |
| A2 | a5 (working precision) | `rho_w` of the refined solve (`refinement(1)`) | `<= 1e-14` each: **registered expected miss** | arithmetic 1: `>= 2.5e-13` on every case |
| A3 | a5 (true residual) | `rho_x` of the refined solve | `<= 1e-14` each: **registered expected miss** | arithmetic 2: 6e-14 (Tet4 `s = 5`, range 1.6e-14 to 1.3e-13) to 1.3e-9 |
| A4 | a6 | `E_K` of the refined solve | `<= 1e-12` each | arithmetic 4: about 2e-17 |
| A5 | a6 | `E_rec` of the refined solve | `<= 1e-12` each | arithmetic 5: `<~ 1e-13` |
| A6 | a7 | times (each the minimum of 5 repetitions in the run, `std::time::Instant`): factor + solve of the default; one refinement step (compensated residual, solve, update); `solve_static` default and with `refinement(1)` end to end; the ratio step / (factor + solve) | record | arithmetic 6: 0.1 to 1.5 (to 5 with a software `fma`) |
| A7 | a3, a4 | (a) `si_static_refinement_defaults.rs`: the FNV-1a 64 digest of the bits of the full displacement, the reactions and `relative_residual` of the default solve of all twelve cases equals the digest computed by the same file on main e9f76a1e before any library line of F-455 is written (amendment 1 records it); (b) the existing `rcfd-fem` tests pass unedited (`cargo test -p rcfd-fem --release`, the whole crate); (c) `cargo check -p rcfd-post --tests` passes | equal / pass / pass | D3: the default runs the same operations |

A2 and A3 are written as one ignored test `a2_a3_relative_residual_1e14_known_miss` with the limit 1e-14 intact, run once with
`--ignored`; its output records `rho_w` and `rho_x` of every case. They are expected misses by derivation (arithmetic 1 and 2),
registered now before any run, not limits tuned after one.

### C. Companions

| Id | Measure | Limit | Estimate |
| --- | --- | --- | --- |
| C1 | `rho_w / phi_w` of the default solve (the claim of ADR 0282 that the default sits at the evaluation floor) | `<= 2` each | 0.3 to 1 |
| C2 | `rho_x / phi_rep` of the refined solve (one step reaches the representability floor) | `<= 2` each | 0.5 to 1 (arithmetic 2, 3) |
| C3 | `omega` of the refined solve (the backward-error statistic of ADR 0282's note) | `<= 1e-14` each | about 1.3e-17 (arithmetic 3) |
| C4 | the library path: `solve_static_report` with `refinement(1)` gives free displacements bit-equal to the test's own `u_0 + d` (factor `SkylineLdlt::factor` of the reduced system, `d = solve(residual_compensated(A, b, u_0))`), `u_0` bit-equal to the default solve's, the solver name `"skyline-ldlt-refined"` and `relative_residual == rho_w(u_1)` bit for bit | as stated, each | D3 |
| C5 | ADR 0277 reproduced (case `s = 10`, the same model): default `rho_w` of Tet10 rounds to 6.7e-11, default `E_rec` of Hex8 to 2.6e-12 and of Tet10 to 1.5e-11 (two significant figures) | as stated | the bits of the default are those of ADR 0277's run unless the library changed since |
| C6 | refusals: `refinement(1)` with `ConjugateGradient` and with `PreconditionedCg` gives `FemError::Solver` whose text names ADR 0397; with a model on the transformation path (the Hex8 `s = 5` cantilever with one `Constraint::linear` term added) likewise, and no displacement is returned | as stated | D3 |
| U1 | `dot2` on crafted vectors: `(1e16, 1, -1e16) . (1, 1, 1) = 1` exactly; `(1 + 2^-30, -1) . (1 + 2^-30, 1) = 2^-29 + 2^-60` exactly (the low part of a product); `two_sum` and `two_product` exact on 10 000 random pairs of integer-valued doubles (seed `0xF455_0397`; sums of integers in `[-2^60, 2^60]`, products of integers in `[-2^40, 2^40]`): `a + b == s + e` and `a b == p + e` checked in `i128` | exact | D2 |
| U2 | `residual_compensated` on a random sparse integer system (seed `0xF455_0397`, `n = 200`, 10 entries per row, entries and `x_j` random integers in `[-2^40, 2^40]`, `b_i` the double nearest the exact `(A x)_i` plus a random integer in `[-2^20, 2^20]`): against the exact residual in `i128`, each row `|r_i - r_exact_i| <= 2 u_r |r_exact_i| + 2 gamma_{12}^2 (|b_i| + sum |a_ij x_j|)` | each row | D2 (the products are up to `2^80`, not exact in double: the `two_product` low parts and the `two_sum` parts are both needed) |

### N. Negative controls

| Id | Control | Expected |
| --- | --- | --- |
| N1 | fixed-precision refinement: one step with the residual by `CsrMatrix::residual` (working precision), the same factor: `E_K` on Hex8 and Tet10 at `s = 20` and `s = 40` | `> 1e-12` each (arithmetic 3: the forward error is not reduced; estimate `5e-11` to `5e-9`) |
| N2 | `E_K` of the default solve on Hex8 and Tet10 at `s = 40` | `> 1e-12` each (arithmetic 4: 1.7e-10 to 4e-9; the refinement is needed for a6) |
| N3 | U1 and U2 with plain sums and products (`x.y` summed in order; `CsrMatrix::residual`) | at least one crafted case of U1 wrong and at least one row of U2 outside the bound |

### Records (no limit)

X1: for every case and both solves (default, refined): `rho_w`, `rho_x`, `phi_w`, `phi_rep`, `omega`, `E_K`, `E_rec`, the norm
of the correction relative to `||u||`, the tip deflection; and `rho_x`, `E_K` after a second step (`refinement(2)`: the floor is
expected to stay, arithmetic 3).

### Mutations (scratch copy `w49i-work/f455-scratch/copy`, own target directory; not committed)

| Id | Mutation | Expected to be caught by |
| --- | --- | --- |
| M1 | `residual_compensated` replaced by the working-precision residual | U2, A4 (at `s >= 20`, as N1), C2 |
| M2 | `two_sum` without its error term (`e = 0`) | U1, U2 |
| M3 | `two_product` without its error term (`e = 0`) | U1, U2 |
| M4 | the correction computed but not added (`u_1 = u_0`) | A4 (as N2), C2, C4 |
| M5 | the correction subtracted (`u_1 = u_0 - d`) | A4, C2 |
| M6 | the refinement applied also with `refinement == 0` | A7 (digest), C4 (`u_0` bits) |
| M7 | the refusal on the transformation path removed | C6 |
| M8 | the solver name not changed | C4 |

## Decision rule

- **done**: every check met, and A2 and A3 met. (By arithmetic 1 and 2 this outcome is not expected.)
- **partial**: A1, A4 to A7, C1 to C6, U1, U2 met, the negative controls as expected and every mutation caught, with A2 and A3
  missed as registered (clause a5 not met: by the house rule a clause not met gives partial); or any check other than A2 and
  A3 missed with a diagnosed cause (kept as a `*_known_miss` record).
- **todo**: the checks could not be run, or A7 fails (the default bits changed).

The addendum will propose, as a remaining question, the re-wording of clause a5 to the attainable statement (C2 and C3), which
the maintainer decides.

## Limits of this registration

- Only the plain static path (`solve_static_report` without the transformation of `mpc.rs`) and only the direct solver; the
  refinement is refused elsewhere (C6). The CG routes and the transformation path keep their behaviour.
- Twelve cantilevers of one section mesh (`2 x 2` cells across); no other load, no prescribed non-zero displacement (`g = 0`;
  with `g != 0` the energy identity of D1 needs the `K_FP g` term and is not checked).
- `E_K` uses the assembled reduced stiffness; `E_rec` the recovery (ADR 0277), whose own floor is estimated, not derived
  rigorously (arithmetic 5).
- The floors of arithmetic 1 and 2 are statistical (random-sign models), not rigorous bounds; C1 and C2 test them with a factor 2.
- The timings depend on the machine and its load (other agents build in parallel); they are a record.
- `StaticOptions` gains a field, so its `Debug` text changes (`refinement: 0`); no golden contains it (none was found that prints
  `StaticOptions`).

## Open points taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. The residual of the refinement step in compensated arithmetic (twice the working precision) inside `rcfd-fem`, not in
   `rcfd-linalg` (keeps the shared linear-algebra crate unchanged; it can move there later).
2. The option as a step count on `StaticOptions` (`refinement`), refused outside the direct solve and the plain path.
3. Clause a5 checked as stated (limit 1e-14 intact) and registered as an expected miss with the derived floors; the attainable
   companions C2 and C3 registered beside it.

## Amendment 1 (2026-11-13, the reference digest of A7 (a), recorded before any library line of F-455)

`crates/rcfd-fem/tests/si_static_refinement_defaults.rs` (old API only) was written and run once on the tree of main e9f76a1e
plus the registration (no library line of F-455 existed): `cargo test -p rcfd-fem --release --test
si_static_refinement_defaults -- --nocapture --test-threads=1`, log `w49i-work/f455-scratch/run-a7-main.log`. Reference digest
**`b9d9056b0c5a7aff`**, now the constant `MAIN_DIGEST` of that file; on the branch A7 (a) asserts equality with it.

Disclosed: the run prints the default `rho_w` of each case, which is record A1 (no limit) and enters C1 and C5 (with `phi_w`,
not yet computed). Printed: Tet4 6.506e-13, 5.745e-12, 7.978e-11, 7.940e-10; Hex8 5.930e-13, 5.784e-12, 8.965e-11, 1.048e-9;
Tet10 5.965e-12, 6.746e-11, 6.966e-10, 8.005e-9 (`s = 5, 10, 20, 40`). Tet10 `s = 10` reproduces ADR 0277's 6.7e-11 (part of
C5). Every value is above 1e-14 by a factor 65 or more, as arithmetic 1 predicted for `rho_w` of any solution. No limit and no
parameter is changed by this; the checks run as registered.
