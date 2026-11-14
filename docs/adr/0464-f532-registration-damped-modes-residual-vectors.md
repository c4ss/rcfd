# ADR 0464: F-532 registration: static residual vectors for the damped modes (boundary dampers near the impedance)

Status: **REGISTERED 2026-11-14, written before any code of F-532 and before any run of its checks**. Not yet accepted by the
maintainer. Before this text nothing of F-532 was written in the repository. Two estimate scripts were run in the scratch folder
(`w49i-work/f532-scratch/estimate.py`, `estimate2.py`, log `estimate.log`); they use numpy on an independent one-dimensional
model and no rcfd code, and are disclosed in full under "Arithmetic before this registration". No check of this registration
was run. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Roadmap item F-532 (deps F-414), structural solver, advanced tier (plan [ADR 0310](0310-structural-solver-advanced-tier-plan.md)),
the follow-up proposed by [ADR 0450](0450-f414-registration-damped-modes-general-eigensolver.md) (its open question 3). Number
0464 given by the lead. Branch `f532-work` from main 80ebbd1e, worktree `w49i-work/f532`, scratch `w49i-work/f532-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical (the new behaviour is
opt-in); no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited (the
new tests are in new files); no new dependency, no unsafe code; clean-room.

## What F-532 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. damped_modes gains static residual vectors (K^-1 times the damper influence vectors, orthogonalised against the undamped modes) as an option. Fixed-free bar with a tip dashpot c = 0.9 A sqrt(E rho): the first three roots (c0/L)(-atanh(c/(A sqrt(E rho))) + i (2 j - 1) pi/2) within 2 percent on 20 modes with the residual vector, against a registered error without it; at c = 2 A sqrt(E rho) the real root -(c0/L) atanh(A sqrt(E rho)/c) within 1 percent on 20 modes; without the option every result is bit-identical to F-414's.

Clauses, numbered for the table: **a1** the option: residual vectors `K^-1` times the damper influence vectors, orthogonalised
against the undamped modes; **a2** `c = 0.9 Z` (`Z = A sqrt(E rho)`), the first three roots within 2 percent on 20 modes with
the residual vector; **a3** against a registered error without it; **a4** `c = 2 Z`, the real root within 1 percent on 20
modes (read with the residual vector: without it the estimate is 1.33e-2, above the limit, see below); **a5** without the
option every result bit-identical to F-414's.

**The closed forms are correct as written** (verified below, including signs and branches). Nothing is corrected.

## What was read (main 80ebbd1e, read only)

ADR 0450 in full (D1 to D4, the tip-dashpot bar, the corrected `c > Z` clause, the partial-fraction truncation estimate, the
addendum and open question 3), `crates/rcfd-fem/src/damped.rs` (F-414: `damped_modes`, `DampedOptions`, `DampedModes`,
`project`), `crates/rcfd-linalg/src/eigen_general.rs` (`quadratic_eigen_with`: a general `M_r` by Cholesky, so a non-diagonal
projected mass is taken), `crates/rcfd-fem/tests/sa_damped.rs` (the bar: `hex_box([0; 3], [1, 0.01, 0.01], [100, 1, 1])`,
steel `E = 2e11`, `rho = 7850`, `nu = 0`, y and z held everywhere, x held at `xmin`, `c/4` on each `xmax` node along x),
`crates/rcfd-fem/src/modal.rs` (`modal_analysis`: `prescribed_equations`, `reduce`, the rigid-body check),
`crates/rcfd-fem/src/constraint.rs` (`reduce`, `ReducedSystem::free`), `crates/rcfd-linalg/src/sparse_chol.rs`
(`SparseLdlt::factor`, `solve`), ADR 0457 and 0463 (the base-digest record of default paths and the one-ulp probe lesson), and
the roadmap entries F-414 and F-532.

## Closed forms (first principles; ADR 0450 derivation re-checked)

Bar `0 <= x <= L`, clamped at 0, `rho A u_tt = E A u_xx`, dashpot at `L`: `E A u_x(L) = -c u_t(L)`. With `u = sinh(lambda
x/c0) e^{lambda t}`, `c0 = sqrt(E/rho)`, `s = lambda L/c0`, `Z = E A/c0 = A sqrt(E rho)`: `E A (lambda/c0) cosh s = -c lambda
sinh s`, so **`tanh s = -Z/c`**. (A decaying root needs `Re s < 0`: the dashpot removes energy at the rate `c u_t(L)^2`.)

- `c = 0.9 Z`: `tanh s = -1/0.9`, `|tanh s| > 1`. Since `tanh(a + i pi/2) = coth a` and `tanh` has period `i pi`, `s = a + i
  (2j - 1) pi/2` with `coth a = -1/0.9`, i.e. `tanh a = -0.9`, `a = -atanh(0.9) = -1.472219`. So `s_j = -atanh(c/Z) + i (2j - 1)
  pi/2`, as written (the minus sign gives `Re s < 0`, decaying; `j = 1, 2, 3`: `Im s = 1.5708, 4.7124, 7.8540`).
- `c = 2 Z`: `tanh s = -1/2` has the real root `s = -atanh(1/2) = -0.549306` (`tanh` is odd), i.e. `lambda = -(c0/L)
  atanh(Z/c)`, as written; the oscillating family is `s = -atanh(Z/c) + i j pi` (ADR 0450).
- `c0 = 5047.545 m/s`, `Z = 1e-4 sqrt(2e11 x 7850) = 3962.32 N s/m`, `L = 1`.

## Arithmetic before this registration (disclosed)

All of it in `f532-scratch/estimate.log`. Scripts in Python with numpy, on the one-dimensional consistent-mass chain of 100
linear elements that the 3-D bar equals for `nu = 0` with y and z held (ADR 0450, F-383: modes equal within 2.2e-14); no rcfd
code was run.

**1. Truncation, first order, from the partial fractions (as ADR 0450).** On `N` undamped modes the root equation is `g_N(s)
= 1 + (2c/Z) s S_N(s) = 0`, `S_N = sum_{k <= N} 1/(s^2 + beta_k^2)`, `beta_k = (2k - 1) pi/2`; `S_inf = tanh(s)/(2s)`. The tail
`T(s) = S_inf - S_N` shifts the root by `delta s = 2 s T/sech^2 s`. Near the impedance `|sech^2 s| = |1 - tanh^2 s|` is small:
0.2346 at `c = 0.9 Z` (0.75 at `c = 2 Z`), the reason 20 modes are not enough there.

**The residual vector.** With the unit tip force `f`, `r = K^-1 f` has the modal expansion `sum_k phi_k phi_k(L)/w_k^2`; its
part M-orthogonal to the first `N` modes is `r_perp = sum_{k > N} phi_k phi_k(L)/w_k^2`. It is also K-orthogonal to them
(`phi_k^T K r_perp = w_k^2 phi_k^T M r_perp = 0`), so the Ritz basis `[Phi_N, r_perp]` is a pseudo-mode with `w_r^2 = F/G`,
tip participation `p^2 = F^2/G`, where `F = sum_{k>N} phi_k(L)^2/w_k^2` (the residual flexibility) and `G = sum_{k>N}
phi_k(L)^2/w_k^4`. The tail of the tip receptance `sum_{k>N} phi_k(L)^2/(lambda^2 + w_k^2) = F - lambda^2 G + O(lambda^4)` is
replaced by `p^2/(lambda^2 + w_r^2) = F - lambda^2 G + O(lambda^4)`: exact to second order in `lambda`. In `s`: `T_r(s) =
T_0/(1 + s^2 T_1/T_0)`, `T_p = sum_{k>N} beta_k^{-2(p+1)}`, and the remaining shift is `2 s (T - T_r)/sech^2 s`. (Static
correction and modal truncation augmentation: Craig and Kurdila, Fundamentals of Structural Dynamics, the residual
flexibility; Dickens, Nakagawa and Wittbrodt, J. Sound Vib. 1997, on mode acceleration and modal truncation augmentation;
both cited from memory, nothing numeric taken; the derivation above is what is used.)

| Set-up | `N` | first order, without | first order, with the residual vector |
|---|---|---|---|
| `c = 0.9 Z`, `j = 1, 2, 3` | 20 | 4.32e-2, 4.33e-2, 4.34e-2 | 5.3e-9, 1.5e-7, 1.0e-6 |
| `c = 2 Z`, `j = 0` (real) | 20 | 1.35e-2 | 7.0e-12 |

**2. Mesh dispersion (derived).** The interior equation of the consistent-mass chain, `u_n = e^{n kappa}`: `(lambda h/c0)^2 =
6 (cosh kappa - 1)/(2 + cosh kappa) = kappa^2 (1 - kappa^2/12 + ...)`, so a root of the chain has `kappa N = s_cont (1 + O(h))`
with `lambda h/c0 = kappa (1 - kappa^2/24)`: relative error of `s` about `|s|^2 (h/L)^2/24` = 1.9e-5, 1.0e-4, 2.7e-4 for
`j = 1, 2, 3` at `c = 0.9 Z` (the tip boundary equation adds a term of the same order). With the residual vector this is the
error that remains.

**3. The discrete chain, solved (numpy, a cross-check of 1 and 2, not a reference value).** The full 100-unknown chain with the
tip dashpot (state matrix eigenvalues) and its Ritz projections:

| Set-up | full chain against the closed form (dispersion) | 20 modes, without | 20 modes, with the residual vector |
|---|---|---|---|
| `c = 0.9 Z`, `j = 1` | 5.7e-5 | 3.77e-2 | 5.7e-5 |
| `c = 0.9 Z`, `j = 2` | 1.60e-4 | 3.69e-2 | 1.60e-4 |
| `c = 0.9 Z`, `j = 3` | 3.34e-4 | 3.54e-2 | 3.33e-4 |
| `c = 2 Z`, `j = 0` (real) | 2.7e-7 | 1.33e-2 | 2.7e-7 |
| `c = 2 Z`, `j = 1, 2, 3` | 4.2e-5, 1.65e-4, 3.7e-4 | 1.33e-2 to 1.37e-2 | 4.2e-5, 1.65e-4, 3.7e-4 |

The first-order truncation estimate over-predicts the error without the vector (4.3e-2 against 3.5e-2 to 3.8e-2: the shift is
not small); the registered band below covers both. Real roots of the 20-mode chain at `c = 2 Z`: `s = -0.5566, -64.35` without
the vector, `-0.5493, -275.6` with it (the fast one a root of the truncation, as ADR 0450 found); the real root of smallest
modulus is the continuum root in both. Tip compliance on 20 modes alone: 0.98 percent below `L/(EA)`; for a pair of opposite
forces at `x = L` and `x = 0.5`: 2.9 percent below `0.5/(EA)`. Condition number of the chain's `K`: 1.6e4.

**4. The 3-D bar with four dashpots.** The dashpot `c/4` on each of the four `xmax` nodes gives four influence vectors, so four
residual vectors. The bar's stiffness, mass and damping commute with the two reflections of the square section; the sum of the
four (the uniform tip force) is in the symmetric class, and the three others span the classes of non-uniform x motion of the
section, which do not couple to it (exactly, up to rounding). The axial roots are therefore those of `[Phi_20, K^-1 f_uniform]`
(the chain estimate above); the other three pseudo-modes carry the section's non-uniform motion, whose roots lie near `s ~ 245`
or far on the real axis (a self-equilibrated tip load is resisted within about one section width: stiffness of the order `E h
= 2e9 N/m` against `c/4 ~ 2000 N s/m`, a real root near `-1e6 1/s`, `s ~ -200`), far from every target root.

**5. Rounding, and the one-ulp question.** No limit of this registration is of the 1e-12 class: the tightest are 1e-10
(orthogonality, mass normalisation) against an estimate of about `n eps = 1e-13`, and 1e-8 (static compliance) against
`cond(K) eps ~ 1e-11` (3-D: the same order; the dense solve of the 24 x 24 projected stiffness adds `cond ~ (w_r/w_1)^2 ~ 3e4`
times `eps`). Every limit has a margin of at least 1e2 over its rounding estimate, so no one-ulp probe was made; this is
disclosed, not hidden. Bit identity (a5) is exact by construction (the default path runs the same operations).

## Design

Every choice below that the maintainer would normally be asked about is taken on the maintainer's standing instruction of
2026-11-11 (take the recommendation); none changes a default result, a golden or a hash table.

### D1 The option

`DampedOptions` gains a field `residual_vectors: bool` (default `false`) and a builder `residual_vectors(on)`. Off: the code
path of F-414 is unchanged and runs the same operations in the same order (a5). On:

1. For each dashpot `i` (in the given order) its influence vector `f_i` on the full `DofMap`: `+1` at its equation, and `-1`
   at the other node's equation for a dashpot between two nodes (the vector with `C_d = c f_i f_i^T`, as F-414 projects it).
2. `r_i = K^-1 f_i` on the free unknowns: `prescribed_equations(model, &[])` (all zero on the elimination path), `reduce` of
   the stiffness that `damped_modes` already assembles, one `SparseLdlt::factor` (rcfd-linalg, sequential), a solve per
   vector, scattered to the full `DofMap` (zero at the held equations). A model with free rigid-body motion
   (`check_rigid_body_motions`) is refused with the option (`K` singular): `InvalidValue` naming the cause.
3. **M-orthogonalisation**: classical Gram-Schmidt in the inner product `x^T M y` (consistent mass, as projected), against
   the `m` undamped modes and the residual vectors kept so far, made twice ("twice is enough", Parlett, The Symmetric
   Eigenvalue Problem, from memory). If the M-norm after it is at most `RESIDUAL_DROP = 1e-8` times the M-norm of `r_i`
   before it (an influence vector already in the span, a repeated dashpot, or both nodes held), the vector is dropped and a
   note says so. Otherwise it is M-normalised.
4. The basis `B = [Phi, R]` (`m + r` vectors) takes the place of `Phi` in every projection of F-414 (`K_r`, `M_r`, `C_r`,
   dashpots, `H_r`), and in the shapes `B q`. `quadratic_eigen_with` takes the non-diagonal `M_r`, `K_r` as they are.
5. `DampedModes` gains `residual_vectors: Vec<Vec<f64>>` (the kept, M-orthonormal vectors on the full `DofMap`; empty without
   the option); `basis` is `m + r`; a note "`r` static residual vectors added to `m` modes" when `r > 0`. With the option and no
   dashpot nothing is added and the result is that of the option off (check D2).

No other file than `crates/rcfd-fem/src/damped.rs` changes (no shared solver file of rcfd-fv or rcfd-api: `baseline_hashes`
is not concerned).

## Checks (limits fixed now)

Tests: `crates/rcfd-fem/tests/sa_damped_residual_default_bits.rs` (D1, written with the API of main 80ebbd1e only) and
`crates/rcfd-fem/tests/sa_damped_residual.rs` (the rest), release profile. The bar is ADR 0450's (above); dashpots `c/4` on each
`xmax` node along x unless stated. Matching: the reported oscillating root nearest each exact root (ADR 0450 `nearest`); the real
root is the reported non-oscillating root of smallest modulus. Basis options `ModalOptions::new(20).tolerance(1e-12)
.residual_test(Backward)`.

| Id | Clause | Check | Limit | Estimate |
|---|---|---|---|---|
| B1 | a2 | bar, `c = 0.9 Z`, 20 modes, residual vectors on: `max_j |lambda - lambda_j|/|lambda_j|`, `lambda_j = (c0/L)(-atanh(0.9) + i (2j - 1) pi/2)`, `j = 1, 2, 3` | `<= 2e-2` | 3.3e-4 (dispersion; truncation 1e-6) |
| B1n | a3 | the same without the option (F-414's path): each of the three errors `> 2e-2`, and their maximum in `[2.5e-2, 5.5e-2]` (the registered error without it) | band | 3.5e-2 to 3.8e-2 (chain), 4.3e-2 (first order) |
| B2 | a4 | bar, `c = 2 Z`, 20 modes, residual vectors on: the real root against `-(c0/L) atanh(1/2)`, relative | `<= 1e-2` | 2.7e-7 |
| B2n | a4 control | the same without the option: relative error in `[8e-3, 2e-2]` (recorded whether above 1e-2) | band | 1.33e-2 (chain), 1.35e-2 |
| B2b | a4 | `c = 2 Z` with the option: the oscillating roots `(c0/L)(-atanh(1/2) + i j pi)`, `j = 1, 2, 3`, within; every real root `oscillating == false`, `Im == 0`, damping ratio 1 within 1e-12; a note names them | `<= 2e-2`; exact | 3.7e-4; holds |
| D1 | a5 | without the option, the digests (FNV-1a of every field of `DampedModes`: each mode's eigenvalue, decay rate, damped omega, frequency, damping ratio, oscillating flag, shape, residual; roots; omegas; basis; counts; defects; notes) of nine F-414 set-ups equal the constants `BASE` printed by the file at main 80ebbd1e before any library code: bar `c = Z/2` on 40, 20, 10 modes; bar `c = 2 Z` on 40 and 20; bar `c = 0.9 Z` on 20; the S2 bar (two-node dashpots) on 20; the R1 cantilever with Rayleigh damping (4 threads); the L1 cantilever with a loss factor | `==` | `==` |
| D2 | a5 | the option on with no dashpot (the R1 cantilever with Rayleigh damping; the L1 loss factor): every field bit-identical to the option off, `residual_vectors` empty | bits | holds |
| E1 | a5 | the existing `sa_damped` (all of F-414's damped checks) and `sa_scaffold` pass unedited | pass | holds |
| O1 | a1 | B1's run: 4 residual vectors kept (one per dashpot), `basis == 24`; `max |phi_k^T M r_i|` over the 20 modes and `max |r_i^T M r_j - delta_ij|` | `<= 1e-10` each | 1e-13 |
| S1 | a1 | B1's run: the static tip compliance on the basis, `f^T B (B^T K B)^-1 B^T f`, `f` the unit axial force shared equally by the four `xmax` nodes, against `L/(E A)`, relative; and, as a control, on the 20 modes alone the deficit is `>= 5e-3` | `<= 1e-8`; `>= 5e-3` | 1e-11; 9.8e-3 |
| S2 | a1 | bar with four dashpots between each `xmax` node and the node of the same corner at `x = 0.5` (`Dashpot::between`, `0.9 Z/4` each), 20 modes, option on: compliance of `f = (1/4) sum (e_tip - e_mid)` on the basis against `(L - x_mid)/(E A)` (`x_mid` the node's coordinate); 4 vectors kept | `<= 1e-8` | 1e-11 (modes alone: 2.9e-2 deficit, recorded) |
| H1 | a1 | B1's run: every reported shape `x^H M x = 1`; a note names the residual vectors | `<= 1e-10`; exact | 1e-14 |
| T1 | a1 | B1's run with 1 and 4 threads: every eigenvalue, shape entry and residual-vector entry bit-identical | bits | holds |
| N1 | a1 | the option on a model with free rigid-body motion (the R1 cantilever without its constraints, with one dashpot): `InvalidValue` naming the rigid-body motion; two identical ground dashpots on one `xmax` node of the bar: the second vector dropped, a note says so, 1 vector kept | error; exact | holds |

Mutations (on a scratch copy; each must be caught by at least one registered check):

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | no orthogonalisation against the modes (only among the residual vectors) | O1 |
| MU2 | no M-normalisation | O1 |
| MU3 | the residual vector is the influence vector itself (no `K^-1`) | S1, S2, B1 |
| MU4 | two-node influence vector `e_a + e_b` | S2 |
| MU5 | only the first dashpot's vector kept | O1, S1 |
| MU6 | the option ignored: vectors added whenever there is a dashpot | D1, B1n |
| MU7 | shapes from the modes only (residual coefficients dropped) | H1 |
| MU8 | Gram-Schmidt in the Euclidean inner product | O1 |

## Decision rule

**done**: every check B1 to N1 met at its registered run, every mutation caught, no amendment after a run. **partial**: a check
missed (recorded as `*_known_miss` with its limit), a mutation not caught by a registered check, or a check met only after an
amendment made after its run. **todo**: the option does not build or B1, B2 or D1 cannot be run.

## Limits (not covered)

- The impedance-matched dashpot `c = Z` itself (no continuum root); only `0.9 Z` and `2 Z` are checked.
- Residual vectors for the loss-factor stiffness or for Rayleigh damping (not boundary dampers; their influence is not of low
  rank) and for loads; one vector per dashpot, so many dashpots grow the dense problem as `O((2 (m + r))^3)`.
- The transformation path (linear constraints, rigid sets, supports) stays refused, as in F-414; free-free models are refused
  with the option.
- The option stays off by default: making it the default would change F-414's results (an open question, not taken).

## Addendum 1: results (2026-11-14)

Commits: 38f88246 (this registration, alone), c47ddba6 (the D1 file with the constants `BASE` printed by its single run at
main 80ebbd1e before any library code; log `f532-scratch/d1-base.log`), d081fbbf (the option in `damped.rs` and the checks),
46fa995d (two message texts and layout, below). Registered runs on the content of d081fbbf, each made once, release profile:
`cargo test --release -p rcfd-fem --test sa_damped_residual`, `--test sa_damped_residual_default_bits`, `--test sa_damped`,
`--test sa_scaffold` (logs `f532-scratch/run_*.log`).

| Id | Limit | Measured | Met |
|---|---|---|---|
| B1 `c = 0.9 Z`, 20 modes + residual vectors, `j = 1, 2, 3` | `<= 2e-2` | 5.74e-5, 1.60e-4, **3.33e-4** (estimate 3.3e-4, the dispersion) | yes |
| B1n the same without the option: each `> 2e-2`; largest in `[2.5e-2, 5.5e-2]` | band | 3.77e-2, 3.68e-2, 3.54e-2; **3.77e-2** (estimate 3.5e-2 to 3.8e-2 chain, 4.3e-2 first order) | yes |
| B2 `c = 2 Z`, 20 modes + residual vectors, the real root | `<= 1e-2` | **2.69e-7** (estimate 2.7e-7) | yes |
| B2n the same without the option | `[8e-3, 2e-2]` | 1.33e-2 (estimate 1.33e-2), above 1e-2: the option is needed for a4 | yes |
| B2b `c = 2 Z` with the option: oscillating `j = 1, 2, 3`; real roots flagged and said | `<= 2e-2`; exact | 4.18e-5, 1.65e-4, 3.71e-4; 8 real roots, `Im = 0`, ratio 1, note present | yes |
| D1 nine F-414 set-ups without the option against `BASE` of main 80ebbd1e | `==` | 9 of 9 | yes |
| D2 option on without dashpots (Rayleigh; loss factor) | bits | identical (7291 words each), no vector | yes |
| E1 `sa_damped` (7 tests), `sa_scaffold` (9, 11 ignored records) unedited | pass | pass | yes |
| O1 4 vectors, basis 24; `max abs(phi^T M r)`; `max abs(r^T M r - delta)` | `<= 1e-10` each | 4, 24; 7.4e-17; 1.4e-15 | yes |
| S1 tip compliance on the basis against `L/(E A)`; modes alone deficit `>= 5e-3` | `<= 1e-8`; `>= 5e-3` | 4.0e-14; 9.79e-3 (estimate 9.8e-3) | yes |
| S2 two-node dashpots: 4 vectors; compliance against `(L - x_mid)/(E A)` | `<= 1e-8` | 4; 7.1e-15 (modes alone 2.93e-2 short, recorded) | yes |
| H1 shapes `x^H M x = 1`; note | `<= 1e-10`; exact | 1.1e-15; note present | yes |
| T1 1 and 4 threads with the option | bits | identical (70782 words) | yes |
| N1 rigid-body motion refused; repeated dashpot dropped and said | error; exact | `InvalidValue` (key `residual vectors`, names the rigid-body motion); 1 kept, basis 21, note | yes |

Also run (not registered): `sa_ortho_modal`, `sa_ortho_rotating` (they call `damped_modes` for refusals): pass.

Mutations (scratch copy `f532-scratch/copy` of 46fa995d, script `mutate.py`, logs `MU*.log`, `mutations.log`):

| Id | Mutation | Caught by |
|---|---|---|
| MU1 | no orthogonalisation against the modes | O1 (0.99); the roots unchanged, as expected (Rayleigh-Ritz does not depend on the basis of a span) |
| MU2 | no M-normalisation | O1 (1.0) |
| MU3 | the influence vector itself, no `K^-1` | S1 (3.3e-3), S2 (1.2e-2); B1 still met (a spike vector also enriches the span) |
| MU4 | two-node vector `e_a + e_b` | S2 (2.6e-2) |
| MU5 | only the first dashpot's vector | B1 (3.05e-2), B2 (1.05e-2), O1 (count), S1, S2 |
| MU6 | the option ignored (vectors whenever dashpots) | D1 (2 of 9), B1n, B2n |
| MU7 | shapes from the modes only | H1 (1.0) |
| MU8 | Gram-Schmidt in the Euclidean inner product | the test functions of O1, B1/B2, S2 and N1 fail: the projected mass is refused by `quadratic_eigen` ("not positive definite", pivot 4.9e-16) before O1 prints its number. Caught by registered checks failing, though not by the O1 limit itself |

**Eight of eight caught.**

Deviations. (1) After the registered run two message texts (the refusal of N1 and the note on a dropped vector) were found to
carry a run of blanks where a line continuation was meant (an editing slip); fixed in 46fa995d together with `cargo fmt` of
the new test (layout only). N1 tests that the messages contain "rigid" and "dropped", true before and after. A confirmation run
on 46fa995d (`confirm.log`) printed every line of `sa_damped_residual` identical to the registered run apart from those two
texts, and D1 9 of 9. (2) With the option, the bar at `c = 0.9 Z` reports 6 real roots with `abs(s)` from 62 to 2600 (`abs(lambda)` 3.1e5 to
1.3e7 1/s), and at `c = 2 Z` 8: the continuum root and 7 with `abs(s)` from 27 to 1750. They are roots of the strongly damped
pseudo-modes of the residual vectors (the 1-D chain estimate had one, at `s = -275.6`; the 3-D bar has four vectors), have no
continuum counterpart, lie far from every target root, and are counted in `non_oscillating`. See question 2.

Not run: the whole workspace (the lead's gate); `baseline_hashes` (no file of rcfd-fv or rcfd-api changed; no default path
changed). `cargo fmt -p rcfd-fem --check`: clean. `cargo clippy -p rcfd-fem --all-targets --release -- -D warnings`: clean.
`cargo test --release -p rcfd-cli --test docs_site` (the new ADR is a page of the site): pass, 2 of 2.

**Decision by the registered rule: done.** Every registered check was met at its registered run, every mutation was caught,
and no check was met only after an amendment (there is none; deviation 1 changes no measured value).

Remaining questions:

1. Make the residual vectors the default when dashpots are present? Options: (a) keep the option off (F-414's bits kept);
   (b) on by default (better roots near the impedance; F-414's results change in every set-up with a dashpot).
   Recommendation: (a); not taken here, since it would change default results.
2. The fast real roots of the pseudo-modes (deviation 2) are reported like physical ones. Options: (a) document (done here);
   (b) report per root the share of its shape carried by the residual-vector coordinates and flag roots dominated by them.
   Recommendation: (b), as the follow-up below.
3. One vector per dashpot: many dashpots grow the dense problem as `O((2 (m + r))^3)`. Options: (a) leave; (b) compress the
   influence block (a truncated singular value decomposition of `R` in the M inner product). Recommendation: (a) until a model
   with many dampers needs it.

### For the lead: roadmap entries

F-532, proposed status **done**:

> Structural solver, advanced tier. Follow-up of F-414 (ADR 0450). ADR 0464, 2026-11-14: DampedOptions::residual_vectors adds one static residual vector per dashpot to the modal basis of damped_modes: K^-1 of its influence vector (sparse LDL^T of the reduced stiffness), M-orthogonalised twice against the undamped modes and the vectors kept, M-normalised, dropped (and said) when already in the span; free-free models refused with the option; off by default. Fixed-free bar with a tip dashpot c = 0.9 Z (Z = A sqrt(E rho)) on 20 modes: the first three roots (c0/L)(-atanh(c/Z) + i (2j - 1) pi/2) within 3.3e-4 with the residual vectors (2 percent; what remains is the mesh dispersion, estimated 3.3e-4), against 3.5e-2 to 3.8e-2 without them (registered band 2.5e-2 to 5.5e-2, from the partial-fraction estimate). At c = 2 Z the real root -(c0/L) atanh(Z/c) within 2.7e-7 (1 percent; 1.33e-2 without). The closed forms were verified as written. Static compliance on the augmented basis equals L/(EA) within 4e-14 (tip dashpots) and (L - x)/(EA) within 7e-15 (dashpots between two nodes); vectors M-orthonormal to 1.4e-15. Without the option nine F-414 results are bit-identical to main 80ebbd1e; bit-identical for 1 and 4 threads. Eight of eight mutations caught. The pseudo-modes add fast overdamped real roots, reported as non-oscillating (|s| >= 27); follow-up proposed.

Follow-up item, proposed id the next free number, title "Damped modes: flag the roots carried by the residual vectors", deps
F-532:

> Every limit not stated here is registered in the ADR before the run. Each DampedMode reports the M-weighted share of its shape carried by the residual-vector coordinates of the basis, and roots with a share above a registered threshold are flagged as pseudo-modes of the truncation. Bar of F-532 at c = 0.9 Z and 2 Z on 20 modes with the residual vectors: the target roots of F-532 are not flagged, every other real root is; without the option every result is bit-identical to F-532's.
