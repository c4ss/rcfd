# ADR 0448: F-412 registration: direct harmonic solve by a complex symmetric envelope L D L^T, loss factors, dashpots, base motion

Status: **REGISTERED 2026-11-14, written before any code of F-412 and before any run of its checks**. Not yet accepted by the
maintainer. Before this text nothing of F-412 was written, compiled or run: the documents and code under "What was read" were
read, and the arithmetic under "Arithmetic before this registration" was done by two numpy scripts on the 1-D chain (not the
library) in the scratch folder `w49i-work/f412-scratch` (`chain.py`, `pivots.py`). Results go in the addendum at the end;
nothing above the addendum is changed after a run.

Roadmap item F-412 (deps F-408, F-373, F-374, F-383, F-399), structural solver, advanced tier (plan
[ADR 0310](0310-structural-solver-advanced-tier-plan.md)), size M, stub [ADR 0315](0315-direct-harmonic-solve-complex-factorisation-structural-damping.md).
Branch `f412-direct-harmonic` from main 46d78b64, worktree `w49i-work/f412`, scratch `w49i-work/f412-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits are fixed here and not changed after a run; a missed limit becomes an ignored
`*_known_miss` test; a defective set-up gets a corrected test by a committed amendment before the corrected run; each registered
run is made once. Everything is opt-in: no existing function changes, no shared solver file is edited, so every default result
keeps its bits by construction. No new dependency, no unsafe code.

## What F-412 asks

The acceptance of `docs/roadmap.json`, binding:

> Every limit not stated here is registered in the ADR before the run. rcfd-linalg: a random complex symmetric diagonally
> dominant matrix of order 200 is solved to a residual of 1e-12 relative; a real matrix gives the real factorisation's solution
> within 1e-13; a pivot below the registered threshold is refused with the equation. Fixed-free bar, nu = 0, loss factor 0.02,
> tip force: tip response F tan(k L)/(E* A k) with E* = E (1 + i eta) and k = w sqrt(rho/E*), amplitude and phase within 0.5
> percent at 0.5, 1.0 and 1.5 times the first frequency. With Rayleigh damping the direct solve equals the mode superposition of
> F-399 with all modes within 1e-8 on a small model. Tip dashpot c on the same bar: response F/(E A k cot(k L) + i w c) within
> 0.5 percent, and with c = A sqrt(E rho) the amplitude is F |sin(k L)|/(E A k) (no resonance) within 0.5 percent at five
> frequencies. Base motion of the clamped end, undamped: tip amplitude 1/cos(k L) of the base within 0.5 percent at half the
> first frequency. A table of modulus against frequency reproduces the constant-modulus result when constant (1e-12). An
> undamped solve at a resonance is refused with the frequency; results are bit-identical for 1 and 8 threads.

Clauses, numbered for the table: a1 random complex order 200; a2 real matrix; a3 small pivot; a4 loss-factor bar; a5 Rayleigh
against F-399; a6 tip dashpot; a7 matched dashpot; a8 base motion; a9 modulus table; a10 resonance refused; a11 threads.

In the formula of a4, `E* A k` means `E* A k*` with the complex `k* = w sqrt(rho/E*)` of the same sentence (one wave number);
derived below. The other closed forms are verified below and are correct as written.

## What was read (main 46d78b64, read only)

ADR 0315 (stub), ADR 0440 (F-408 scaffold; the stubs `rcfd_linalg::cskyline` and `rcfd_fem::harmonic_direct` with
`PENDING = Some("ADR 0315 (F-412)")`, the check X1 of `crates/rcfd-fem/tests/sa_scaffold.rs` that asserts it; the complex type
`rcfd_linalg::complex::C64` with Smith's division), `crates/rcfd-linalg/src/skyline.rs` (F-373, ADR 0274: envelope L D L^T
after reverse Cuthill-McKee, `dot4` with four fixed accumulators, pivot test `theta = (w + 1) eps (|a_ii| + sum |u l|)`),
`crates/rcfd-fem/src/harmonic.rs` (F-399, ADR 0391 and ADR 0400: convention `x(t) = Re(X e^{i w t})`, `HarmonicLoad`,
`HarmonicResponse` with `stress`, `ModalBasis::all_modes`, `ModalDamping::Rayleigh { mass, stiffness }` meaning
`C = mass M + stiffness K`), `assemble.rs` (`assemble_with`, `stiffness_with`), `mass.rs` (`consistent_with`),
`constraint.rs` (`prescribed_equations`, `reduce`), `model.rs` (`Material::youngs_modulus`, `material_indices`), the bar of
`crates/rcfd-fem/tests/si_harmonic.rs` (`hex_box` of `[0, 1] x [0, 0.04]^2`, y and z held everywhere, x held at `xmin`, steel
`E = 2e11`, `rho = 7850`, `nu = 0`) and the notes of F-373, F-383 (the Hex8 bar equals the discrete 1-D chain within 2.2e-14),
F-399, F-408 in `docs/roadmap.json`. The standing decision on retired scaffold checks (ADR 0291 pattern, as in
`si_harmonic.rs::c6_refusals_after_landed`).

## Closed forms (first principles)

Bar `0 <= x <= L`, axial displacement `u(x, t) = Re(U(x) e^{i w t})`, modulus `E*`, area `A`, density `rho`. The equation
`rho A u_tt = (E* A u_x)_x` gives `U'' + k*^2 U = 0`, `k* = w sqrt(rho/E*)` (the principal root; with `E* = E (1 + i eta)`,
`Im k* < 0`).

- **a4** Clamped at 0, tip force `F` at `L`: `U = a sin(k* x)`, `E* A U'(L) = F`, so `U(L) = F sin(k* L)/(E* A k* cos(k* L)) =
  F tan(k* L)/(E* A k*)`. The roadmap's `E* A k` is this `E* A k*`.
- **a6** Undamped `E`, tip dashpot `c` (force `-c u_t` at `L`): `E A U'(L) = F - i w c U(L)`, `U = a sin(k x)`, `k = w/c0`,
  `c0 = sqrt(E/rho)`: `a (E A k cos kL + i w c sin kL) = F`, `U(L) = F/(E A k cot(kL) + i w c)`. As written.
- **a7** With `c = A sqrt(E rho)`, `w c = A w sqrt(E rho) = E A k`, so `U(L) = F/(E A k (cot kL + i))` and
  `|U(L)| = F |sin kL|/(E A k |cos kL + i sin kL|) = F |sin kL|/(E A k)`. As written. Zero at `kL = n pi` (`w = 2n w1`): the
  five frequencies avoid even multiples of `w1`.
- **a8** Base `U(0) = U_b`, free tip, undamped: `U = U_b cos kx + b sin kx`, `U'(L) = 0` gives `b = U_b tan kL` and
  `U(L) = U_b (cos kL + sin^2 kL/cos kL) = U_b/cos kL`. As written; at `w = w1/2`, `kL = pi/4`, `1/cos = sqrt 2`.
- First frequency: `f1 = c0/(4 L)` (continuum; `w1 = 2 pi f1`). "0.5, 1.0, 1.5 times the first frequency" use this `w1`.
- **Stress** (extension S1): `sigma = E* U'(x) = F cos(k* x)/(A cos(k* L))` for the a4 bar.

## Arithmetic before this registration (disclosed)

`chain.py`: the Hex8 bar with y, z held, `nu = 0` and a uniform section motion is the 1-D chain of `N` linear elements with
`K_e = E* A/h [1 -1; -1 1]`, `M_e = rho A h/6 [2 1; 1 2]` (F-383 measured the equality of frequencies within 2.2e-14). The chain
response was solved densely in numpy (complex) and compared with the closed forms, relative complex error `|X_h - X|/|X|`:

| Case | N = 32 | N = 64 | N = 80 | N = 100 |
|---|---|---|---|---|
| a4 at 0.5, 1.0, 1.5 `w1` | 1.4e-5, 1.00e-2, 1.3e-3 | 3.6e-6, 2.5e-3, 3.2e-4 | 2.3e-6, 1.61e-3, 2.1e-4 | 1.5e-6, 1.03e-3, 1.3e-4 |
| a4 phase error at 1.0 `w1`, rad | 1.0e-2 | 2.5e-3 | 1.61e-3 | 1.03e-3 |
| a6 `c = 0.1 A sqrt(E rho)` at 0.5, 1.0, 1.5 | | | 2.3e-6, 2.5e-4, 2.1e-4 | |
| a7 amplitude at 0.5, 1.0, 1.5, 2.5, 3.0 | | | 1.1e-6, 3e-10, 1.0e-4, 3.4e-4, 2.3e-7 | |
| a7 complex error, same | | | 1.6e-6, 2.5e-5, 1.5e-4, 4.9e-4, 6.8e-4 | |
| a8 at 0.5 `w1` | 2.0e-5 | 4.9e-6 | 3.2e-6 | 2.0e-6 |

The resonant case a4 at `w1` dominates and falls as `N^-2` (the frequency error of the chain, `(k h)^2/24`, against the width
`eta`; amplitude error about `2 eps^2/eta^2`, phase about `2 eps/eta` with `eps` the relative frequency error). **Mesh
registered: N = 80 cells**, estimate 1.61e-3 (a margin of 3.1 under 5e-3); 320 free unknowns.

`pivots.py`: relative pivots `|d_i|/(|a_ii| + sum |u_ij l_ij|)` of the undamped chain `K - lambda_1 (1 + delta) M` (natural
order and reversed), `N = 10` and `N = 80`. The smallest is the last pivot, `0.006 delta` to `1.0 delta` for `delta >= 1e-12`
(at `delta = 0`: 7.5e-17 to 6.8e-13, rounding). So a resonance threshold `tau_r = 1e-8` on the relative pivot refuses an
undamped solve within `delta <= 1e-8` to `1.6e-6` of a resonance and accepts `delta = 1e-3` with a ratio above 6e-6 (margin
600). Exact eigenvalue of the dense basis: `delta` about 1e-14, ratio <= 1e-13.

## Design

### D1 `rcfd_linalg::cskyline` (fills the stub; new code only in that file)

- `ComplexCsr { n, row_ptr, col, val: Vec<C64> }`, square, columns sorted within rows: `new` (validates the structure),
  `from_real(&CsrMatrix)`, `from_parts(re, im)` (union of the two patterns), `matvec`.
- `ComplexSkylineLdlt::factor` / `factor_with(&ComplexCsr, &ComplexSkylineOptions)`: the algorithm of `skyline.rs` (ADR 0274
  D1 to D4) in complex arithmetic, without conjugation: `A = L D L^T`, `L` unit lower on the envelope after reverse Cuthill-McKee
  (the same `rcm_order` call on the same pattern, so the same order as the real factor), `u_ij = a_ij - sum u_ik l_jk`,
  `l_ij = u_ij/d_j` (Smith's division of `C64`), `d_i = a_ii - sum u_ij l_ij`, the sums by a complex `dot4` with four fixed
  accumulators in the order of the real one. Options: ordering (`SkylineOrdering` of ADR 0274, reused), symmetry tolerance
  (default 1e-12: `|a_rc - a_cr| <= delta sqrt(|a_rr| |a_cc|)`, no conjugate: a Hermitian matrix with an imaginary
  off-diagonal part is refused as unsymmetric), and `min_relative_pivot` `tau` (default 0).
- **Pivot test (the registered threshold of a3):** refuse `d_i` when `|d_i| <= max(theta_i, tau s_i)` or not finite, with
  `s_i = |a_ii| + sum_j |u_ij l_ij|` and `theta_i = (w_i + 1) eps s_i`, `w_i` the envelope width of row `i` (the real test of ADR
  0274 D4 with `|.|`). The error `ComplexSkylineError::Pivot { row, position, pivot, threshold }` names the original equation;
  others: `Malformed`, `NonFinite { row, col }`, `Unsymmetric { row, col, a_rc, a_cr }`, `BadOrdering`.
- For a real matrix every complex operation has a zero imaginary part and the real part is the real operation bit for bit (the
  product `(a + 0i)(b + 0i) = ab - 0`, Smith's quotient with `d = 0` is `a/c`, `|x + 0i| = |x|`), so the solution equals
  `SkylineLdlt`'s bit for bit (estimate of a2: 0).
- **Why no pivoting (from first principles; Higham, Accuracy and Stability of Numerical Algorithms, 2nd ed., on complex
  symmetric systems, cited from memory and not used for a number):** (i) a strictly diagonally dominant complex matrix keeps
  strict diagonal dominance in every Schur complement (the real proof carries over with `|.|`), so every pivot is non-zero and
  the entries do not grow beyond `2 max |a_ij|` (a1); (ii) `A = K' + i B` with `B` symmetric positive definite (hysteretic or
  Rayleigh damping on a constrained model) has every leading principal submatrix `A_k` non-singular, because
  `Im(x^H A_k x) = x^H B_k x > 0` for `x != 0`; existence of the factor follows; its accuracy is measured (a4 to a8 compare to
  the 1-D chain within 1e-8, H2), not claimed. A semidefinite `B` (dashpots only) or `B = 0` is guarded by the pivot test.

### D2 `rcfd_fem::harmonic_direct` (fills the stub; new code only in that file)

Per frequency `w`, on the model reduced by its zero prescribed displacements (`x(t) = Re(X e^{i w t})`, ADR 0391):

`A(w) = sum_m s_m(w) (1 + i eta_m(w)) K_m - w^2 M + i w (alpha M + beta K + C_d)`, `A_FF X_F = F_F - A_FP G_P`,

- `K_m` the stiffness of the elements of material `m` (the element matrices of `stiffness_with`, assembled per material by
  `assemble_with`; `K = sum_m K_m` in material order), `M` the consistent mass (`consistent_with`).
- **Loss factor per material** (`LossFactor`, keyed by the material's element set): `Constant(eta)` (`s = 1`), or
  `Table(ModulusTable)`: storage modulus `E'(f)` and loss factor `eta(f)` against frequency `f = w/(2 pi)` in Hz, at least two
  points, strictly ascending, `E' > 0`, `eta >= 0`, linear interpolation in `f`, a frequency outside the table refused (no
  extrapolation); `s = E'(f)/E` for an isotropic elastic material (the stiffness is linear in `E` at fixed `nu`; other variants
  refused for a table). The coefficient is formed as `(s, s eta)` in both cases, so a constant table gives the bits of
  `Constant` (estimate of a9: 0). This is how the complex modulus of ADR 0313 can enter later (a table of `E'` and `eta`).
- **Rayleigh** `alpha M + beta K` (the meaning of `ModalDamping::Rayleigh { mass, stiffness }`), with the elastic `K`.
- **Dashpots** `Dashpot::ground(node, component, c)` (a viscous term `c` on the diagonal) and `Dashpot::between(a, b, component,
  c)` (`c [1 -1; -1 1]` on the two equations); the pattern is the union of the mesh pattern and these pairs. A missing node or
  component and a negative or non-finite `c` refused with the dashpot's index.
- **Base motion** `BaseMotion`: complex amplitudes `G` at prescribed equations (by node and component, or a node set); each must
  be a zero prescribed equation of the model, otherwise refused with its node and component. The response is the absolute
  displacement (the base values at their equations).
- The model's prescribed values must be zero (a non-zero static value refused, as ADR 0391), the transformation path (linear
  constraints, rigid sets, elastic supports, cyclic) is refused (listed under Limits).
- **Factor:** `ComplexSkylineLdlt` in reverse Cuthill-McKee order. **Resonance (a10):** when every imaginary part of `A_FF` is
  zero (undamped), the factor gets `min_relative_pivot = RESONANCE_PIVOT = 1e-8` (from `pivots.py`, above); any pivot refusal is
  returned as `FemError::InvalidValue { key: "frequency" }` with `w` in rad/s (`{w:e}`) and in Hz, the reduced equation and its
  node and component.
- Output: `HarmonicResponse` of F-399 (amplitude, phase, `stress`), `response(load, w)`, `response_with_base(load, base, w)`,
  `sweep(load, ws)` (each element bit-equal to `response` at its frequency). Threads only in the assembly (bit-identical by ADR
  0275); the factor and the solve are sequential: bit-identical for 1 and 8 threads.

### D3 Scaffold check X1 (ADR 0440)

Both stubs land, so `PENDING` becomes `None` for `harmonic_direct` and `cskyline` and X1 of `sa_scaffold.rs` (which asserts
`Some("ADR 0315 (F-412)")` for both) no longer holds. By the standing decision (ADR 0291 pattern, as
`si_harmonic.rs::c6_refusals_after_landed`) X1 becomes an ignored record citing ADR 0291 and ADR 0448, unedited otherwise, and
`x1_modules_and_pending_after_landed` repeats it with those two rows expecting `None`. Taken on the maintainer's standing
instruction of 2026-11-11 (take the recommendation).

## Checks

Tests: `crates/rcfd-linalg/tests/sa_cskyline.rs` (L), `crates/rcfd-fem/tests/sa_harmonic_direct.rs` (H, R, N), release profile.
Bars: the si_harmonic bar of 80 Hex8 (`hex_box([0;3], [1, 0.04, 0.04], [80, 1, 1])`, steel, `nu = 0`), tip force 1 N as 0.25 N
on each `xmax` node; tip quantity at the first `xmax` node. Relative complex error `e = |X_h - X|/|X|`; amplitude error
`| |X_h| - |X| |/|X|`; phase error `|wrap(arg X_h - arg X)|`.

| Check | Clause | Quantity | Limit | Estimate |
|---|---|---|---|---|
| L1 | a1 | random complex symmetric, strictly diagonally dominant, order 200 (seed `0xF412_0448`, splitmix64; pair `i < j` stored with probability 0.05; parts uniform in `[-1, 1)`; `a_ii = (sum_j |a_ij| + 1) e^{i theta}`, `theta` uniform in `[0, 2 pi)`), random complex `b`: `||b - A x||_2/||b||_2` | `<= 1e-12` | about 1e-15 |
| L2 | a2 | 7-point Laplacian of 6^3 (216): `max |x_c - x_r|/max |x_r|` against `SkylineLdlt`; and `max |Im x_c|` | `<= 1e-13`; `== 0` | 0 (bit-equal); 0 |
| L3 | a3 | `[[i, 1], [1, -i]]` (natural order; `d_2 = -i - 1/i = 0`) and `[[1, 0, 1], [0, 2i, 0], [1, 0, 1]]` (natural; `d_3 = 0`): refused `Pivot` with `row` 1 and 2, message naming `row 1` / `row 2` | refused, row as stated | as stated |
| L3n | a3 control | `[[i, 1], [1, -i + 1e-6]]`: accepted, residual | `<= 1e-10` | 1e-16 |
| L3t | a3 | `min_relative_pivot`: `[[1, 0.9], [0.9, 1]]` (`|d_2|/s_2 = 0.19/1.81 = 0.105`) refused with `tau = 0.2`, accepted with `tau = 0.05` | as stated | as stated |
| L4 | a1 checks | non-finite entry refused with row and column; `a_01 = 1 + i`, `a_10 = 1 - i` (Hermitian) refused `Unsymmetric` at (0, 1) or (1, 0); a malformed CSR (unsorted columns) refused | as stated | as stated |
| L5 | a1 | the order-200 solve: factor and solution bits equal for two factorisations (determinism) | 0 bits | 0 |
| H1a..c | a4 | loss factor 0.02, 0.5, 1.0, 1.5 `w1`: amplitude error | `<= 5e-3` | 2.3e-6, 1.61e-3, 2.1e-4 |
| H1d..f | a4 | same: phase error relative to `|arg X|` (the strict reading of "phase within 0.5 percent") | `<= 5e-3` | 4e-6, 1.0e-3, 2e-6 |
| H1g..i | a4 | same: `e` (bounds both amplitude and phase in radians) | `<= 5e-3` | 2.3e-6, 1.61e-3, 2.1e-4 |
| H2 | a4 to a8 (set-up) | every bar response of H1, H4, H5, H6 against the 1-D chain solved in the test (complex Thomas algorithm, the same `E*`, `c`, base): `e` | `<= 1e-8` | about 1e-10 (condition about 4e5 at resonance times eps) |
| H3 | a5 | cantilever `hex_box([0;3], [0.6, 0.1, 0.1], [6, 2, 2])`, steel `nu = 0.3`, `xmin` held, tip load `(0.3, 1, -0.5)` N on node set `xmax` with imaginary part `(0, 0.2, 0)`; Rayleigh `alpha = 0.02 w_1`, `beta = 0.02/w_1` (`w_1` of the basis); direct against `HarmonicSolver` with `ModalBasis::all_modes` (162 unknowns), no static correction, at 0.5, 1.0, 1.7, 2.5, 4.0 `w_1`: `max_j |X_d - X_m|/max_j |X_m|` | `<= 1e-8` | about 1e-12 |
| H4a..c | a6 | undamped bar, tip dashpot `c = 0.1 A sqrt(E rho)` (c/4 on each `xmax` node, ground), 0.5, 1.0, 1.5 `w1`: `e` | `<= 5e-3` | 2.3e-6, 2.5e-4, 2.1e-4 |
| H5a..e | a7 | `c = A sqrt(E rho)`, 0.5, 1.0, 1.5, 2.5, 3.0 `w1`: amplitude error against `F |sin kL|/(E A k)` | `<= 5e-3` | 1.1e-6, 3e-10, 1.0e-4, 3.4e-4, 2.3e-7 |
| H6 | a8 | undamped bar, base motion 1 m on `xmin` component 0, no load, 0.5 `w1`: `|U(L)|/|U_b|` against `1/cos(pi/4)` | `<= 5e-3` | 3.2e-6 |
| H7 | a9 | a4 bar with `Table([(0, E, 0.02), (1e5 Hz, E, 0.02)])` against `Constant(0.02)`, 0.5, 1.0, 1.5 `w1`: `max_j |X_t - X_c|/max_j |X_c|` | `<= 1e-12` | 0 |
| H7b | a9 extension | table `[(0, E, 0), (2 f_q, 2 E, 0.04)]` at `f_q = 0.5 f1` (`s = 1.5`, `eta = 0.02`) against a bar of `1.5 E` with `Constant(0.02)` | `<= 1e-10` | about 1e-13 |
| H8 | a10 | undamped bar of 10 Hex8, at `w = sqrt(lambda_1)` of `ModalBasis::all_modes`: refused, key `"frequency"`, message containing `format!("{w:e}")` | refused, as stated | refused |
| H8n | a10 control | same at `w = sqrt(lambda_1) (1 + 1e-3)`: accepted, finite | accepted | relative pivot about 6e-5 |
| H9 | a11 | responses of H1 (1.0 `w1`), H3 (1.0 `w_1`), H4 (1.0), H6 with assembly threads 1 and 8: bits | 0 bits differ | 0 |
| S1 | extension (stress) | H1 at 0.5 `w1`: `sigma_xx` at the integration points against `F cos(k* x)/(A cos(k* L))`: `max |s_h - s|/max |s|` | `<= 1e-3` | about 1e-5 |
| SW1 | extension | `sweep` over H1's three frequencies bit-equal to `response` at each | 0 bits | 0 |
| R1..R9 | refusals | each refused with the stated key and naming the item: R1 loss factor `-0.1` (`"loss factor"`, the set); R2 a set without a material (`"loss factor"`, its name); R3 table not ascending, R4 frequency outside the table (`"modulus table"`, the frequency); R5 dashpot on a missing node, R6 negative `c` (`"dashpot"`, its index); R7 base motion on a free equation (`"base motion"`, node and component); R8 a non-zero prescribed value (`"constraint"`); R9 an elastic support (transformation path, `"constraint"`); R10 negative or non-finite `w` (`"frequency"`); R11 load of the wrong length (`FieldLength`) | as stated | as stated |
| X1 | D3 | `x1_modules_and_pending_after_landed` passes; the old X1 is an ignored record | as stated | as stated |

### Planned mutations (scratch copy, `w49i-work/f412-scratch/mut`)

| Id | Where | Change | Registered catchers |
|---|---|---|---|
| M1 | cskyline factor | the update sum conjugates `l_jk` | L1, H1, H3 (L2 cannot: real) |
| M2 | cskyline pivot test | test removed | L3 (both), H8 |
| M3 | cskyline solve | the diagonal step multiplies by `d` instead of dividing | L1, L2, L3n, H1 |
| M4 | harmonic_direct | loss term `K (1 - i eta)` | H1d..i (the amplitude is unchanged), H2 |
| M5 | harmonic_direct | dashpot term `i c` (without `w`) | H4, H5, H2 |
| M6 | harmonic_direct | base right-hand side `+ A_FP G` | H6, H2 |
| M7 | harmonic_direct | Rayleigh `alpha M + beta K` multiplied by `w^2` instead of `w` | H3 |
| M8 | harmonic_direct | table interpolation takes the lower point | H7b |
| M9 | harmonic_direct | resonance tolerance not passed (0) | H8 |
| M10 | harmonic_direct | loss factor applied to `K - w^2 M` (mass damped too) | H1, H2 |

## Decision rule

**done** if every registered check (L, H, S1, SW1, R, X1) is met in its single registered run and every planned mutation is
caught by at least one registered check; **partial** if a registered check missed, a mutation was not caught, or a check was met
only after an amendment made after its run; **todo** if the factorisation or the direct solve cannot be built.

## Limits (what is not covered)

- The transformation path (linear constraints, rigid sets, elastic supports, cyclic symmetry of ADR 0296): refused; it needs the
  complex right-hand side through `T` (follow-up).
- Pre-stress as an added stiffness (in the stub's goal, not in the acceptance): not built; follow-up.
- Non-zero static prescribed values; a table for non-isotropic materials; a frequency-dependent dashpot.
- No pivoting: an undamped or lightly damped system with a near-zero leading minor away from a resonance is refused rather than
  pivoted. One factorisation per frequency: a sweep of a large model costs one factor per point (recorded, not optimised).
