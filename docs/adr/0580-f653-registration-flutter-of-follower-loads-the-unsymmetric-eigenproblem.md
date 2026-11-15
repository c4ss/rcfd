# ADR 0580: F-653 registration: flutter of follower loads, the unsymmetric eigenproblem (Beck's column)

Status: **REGISTERED 2026-11-15, written before any code of F-653 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-653 was written in the repository. No rcfd code was run for this registration: the
reference numbers below come from two plain Python scripts of the reference alone (`w49i-work/f653-scratch/beck_reference.py`
and `sym_part.py`, numpy only, logs `beck_reference.log` and the console output quoted in arithmetic 3), disclosed. The bit
record N1 (D6) is a test file written and run on the unchanged base e348eb34 after this commit and before any library line.
Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-653 (dep F-572), structural solver, advanced tier: the follow-up proposed by
[ADR 0560](0560-f572-registration-arc-length-with-a-follower-reference-load-and-an-unsymmetric-factor.md) (its remaining
question 3: the buckling path and `tangent_modes` keep the symmetric part of the load stiffness). Number 0580 given by the lead.
Branch `f653-work` from e348eb34, worktree `w49i-work/f653`, scratch `w49i-work/f653-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited;
no new dependency, no unsafe code, the crate graph unchanged; clean-room. Every choice the maintainer would normally be asked
about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation) and listed at the end.

## What F-653 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. tangent_modes and the buckling path take the full (unsymmetric) load stiffness as an option and return complex eigenvalues; Beck's column (a cantilever under a tangential follower tip force) has its flutter load 20.05 E I/L^2 within 1 percent (the coefficient to be checked against a reference held in docs/reference-data.md before registration; cited from memory); the symmetric paths keep their bits.

Clauses: (a0) every limit registered here; (a1) `tangent_modes` takes the full load stiffness as an option and returns complex
eigenvalues; (a2) the buckling path likewise; (a3) Beck's column has its flutter load within 1 percent of the coefficient,
which is checked against a reference in `docs/reference-data.md` before registration; (a4) the symmetric paths keep their bits.

Readings, registered:

- **(a3), the coefficient.** `docs/reference-data.md` held no entry for Beck's column at the base (grep for "Beck" and
  "flutter": none). The brief says the 20.05 is cited from memory and not to be trusted. It is therefore **derived** here
  (arithmetic 1) and the derived value, marked derived, is entered in `docs/reference-data.md` in the same commit as this
  registration: `p_cr = P L^2/(E I) = 20.0509536` (Euler-Bernoulli). The 20.05 of the acceptance agrees with it to its printed
  digits. The check is against the derived value.
- **(a1), (a2), "as an option".** The existing functions `follower::tangent_modes` and `follower::buckling_factors` return
  real results (`ModalResult`, `BucklingResult`); a complex result needs another return type. The option is therefore two new
  entry points next to them, `follower::tangent_modes_full` and `follower::buckling_factors_full` (D3, D4), with the same
  arguments; the existing functions are not changed in any line (open question 1).
- **(a3), the model.** "A cantilever under a tangential follower tip force": a slender solid cantilever of Hex20 clamped at
  `x = 0`, with the existing `FollowerPressure` (ADR 0480 D1) on its free end face. A pressure that follows the deformed end
  face is a force of the resultant `P = p b h` that stays normal to the end section, which in Euler-Bernoulli kinematics is
  the tangent of the axis at the tip: Beck's load. No new load is needed (open question 2: the brief's Beam2 alternative).
  "Its flutter load": the smallest load at which two eigenvalues `omega^2` of the modes about the loaded state coalesce and
  become complex, found by bisection in the load on the classification "a complex pair among the lowest six" (D5).
- **(a4).** No existing function body changes (D7): every call of the existing API takes the same operations. Checked by the
  new record N1 (written and run on the base before any library line), the existing default-bit records of `rcfd-fem` and
  `baseline_hashes` of `rcfd-api`.

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0560 (whole: D1 to D4, the limits, remaining question 3, the follow-up text); ADR 0480
  (through ADR 0560 and `follower.rs`: D1, D3, D7); ADR 0572 and 0573 (the form of "For the lead").
- `crates/rcfd-fem/src/follower.rs` (whole), `buckling.rs` (`BucklingOptions`, `BucklingResult`, `factors`, `factors_with`),
  `nonlinear.rs` (`check_driver_model`), `model.rs` (`check_elements`, `check_supported_nonlinear`), `modal.rs`
  (`ModalOptions`), `rcfd-linalg/src/eigen.rs` (`Shift`), `eigen_general.rs` (the header and the API of `general_eigen`:
  the Francis double-shift QR of ADR 0450 with eigenvectors and residuals), `skyline_lu.rs` (the header and the API of
  `EnvelopeLu`, ADR 0560 D4), `complex.rs` (`C64`), `matrix.rs` (the `CsrMatrix` API), the header of
  `rcfd-fem/src/rotating/whirl/state/arnoldi.rs` (the shift-invert Arnoldi of ADR 0428, for the form; nothing reused).
- Tests: `sa_f572.rs` (the `Checks` helper and the plate model), `sa_follower.rs` (B1 to B3, M1 to M6: the use of
  `buckling_factors`, `tangent_modes`, `Prestress::from_static`), the function list of `sa_follower_default_bits.rs`.

## Arithmetic done before the code (disclosed)

1. **Beck's column, Euler-Bernoulli, derived** (first principles; `beck_reference.py`, method 1). `E I w'''' + P w'' + m w_tt =
   0` on `0 < x < L`, `w(0) = w'(0) = 0`, `w''(L) = 0`, and at the tip the transverse balance of the shear force, the
   transverse component `P w'(L)` of the axial force and the follower's transverse component `-P w'(L)`: `E I w'''(L) = 0`.
   (Weak form: `int E I w'' v'' - int P w' v' + P w'(L) v(L) = omega^2 int m w v`; the last boundary term is the follower's, the
   source of the asymmetry.) With `p = P L^2/(E I)`, `W = m omega^2 L^4/(E I)`, `w = exp(s x/L)`: `s^4 + p s^2 - W = 0`, `a^2 =
   (sqrt(p^2 + 4 W) + p)/2`, `b^2 = (sqrt(p^2 + 4 W) - p)/2`, `w = A cos a xi + B sin a xi + C cosh b xi + D sinh b xi`; the
   clamp gives `A = -C`, `B = -b D/a`, and the two free-end conditions the 2 x 2 determinant `D(p, W) = (a^2 cos a + b^2 cosh
   b)(a^2 b cos a + b^3 cosh b) - (a b sin a + b^2 sinh b)(-a^3 sin a + b^3 sinh b)`. The flutter load is the coalescence of the
   two lowest roots `W(p)`: `D = 0` and `dD/dW = 0`, solved by Newton (derivatives by complex step and central differences),
   from `(20, 121)`, 3 iterations: **`p_cr = 20.050953619`, `W_cr = Omega_cr^2 = 121.34251014`** (`Omega_cr = 11.015558`),
   residuals `D = -6.8e-13`, `D_W = -1.9e-14`. Checks of the same script: at `p = 0` the roots give `Omega = 3.516015, 22.034492`
   (the cantilever's `1.8751^2, 4.6941^2`); at `0.99 p_cr` two real roots `W = 101.18, 144.13`; at `1.01 p_cr` no real root below
   600.
2. **The same by an independent discretisation** (method 2): Hermite cubic beam elements (`K`, consistent geometric `G`,
   consistent mass, the follower term `+P` at (tip deflection row, tip rotation column)), the coalescence of the lowest two
   eigenvalues by bisection on `p`: `N = 10, 20, 40, 80`: `p_cr = 20.0523362, 20.0510416, 20.0509591, 20.0509539`, errors
   against method 1 `+6.9e-5, +4.4e-6, +2.8e-7, +1.5e-8`, ratios 15.7, 15.9, 18.1 (the expected `h^4`). The two methods
   agree to `1.5e-8`. The coefficient 20.05 of the acceptance is confirmed, and the value used is 20.0509536.
3. **The departure of a solid model** (`beck_reference.py`, method 3, and `sym_part.py`). A Timoshenko beam (shear `kappa G
   A`, `kappa = 5/6`, `G = E/2` at `nu = 0`, rotary inertia `rho I`; the follower following the section or the axis gives the
   same tip condition `w' = psi`), by a transfer matrix of the first-order system (Taylor exponential with scaling and
   squaring; the EB limit `kappa G A -> 1e12` overflows and is not used): `L/h = 25, 50, 100`: `p_cr = 19.8277 (-1.1e-2),
   19.9946 (-2.8e-3), 20.0368 (-7.0e-4)`, the coalescence `W = 120.85` at `L/h = 50` (`-4.0e-3`). So `L/h = 50` is registered:
   the shear and rotary inertia lower the flutter load by about `2.8e-3`. The tangent of the solid about the compressed state
   is taken in the reference configuration (total Lagrangian, `nonlinear::tangent`): the axial strain at the flutter load is
   `eps = p_cr (h/L)^2/12 = 6.7e-4`, and the shortening and the initial-displacement terms shift the coefficient by `O(2 eps) =
   1.3e-3` (sign not derived). The Hex20 discretisation with 50 elements of aspect 1 and one element through the thickness
   (exact for a linear bending stress): the error is `O(h^4)` and below `1e-4` by the Hermite ratios above (a quadratic solid
   element is less accurate than the Hermite beam per element, but 50 elements on two half-waves leave a large margin).
   **Estimate for F1: `-1.5e-3`, `|dev| <= 4e-3`; limit `1e-2` (the acceptance).** For F2 (the coalescence frequency) `-4e-3`,
   limit `2e-2`.
   - The static problem with the full load stiffness (Hermite, method 2): `K q = mu (G - E) q` has **no positive real
     eigenvalue** for `N = 10, 25, 50`; the smallest moduli `156, 550, 183` (a negative real value and complex pairs, `|mu|`
     not converging with `N`: discretisation artefacts of a continuum problem whose determinant does not depend on the load,
     the known absence of divergence of Beck's column). With the symmetric part `(E + E^T)/2` in place of `E` the static
     problem has the positive real factor `p = 9.8696` (`= pi^2` to the printed digits) and the lowest `omega^2` at `1.01 p_cr`
     is real and negative, `W = -54.50` (`sym_part.py`): the symmetric approximation predicts divergence, not flutter. With
     the dead tip force (`K q = mu G q`) the lowest factor is `2.4674011 = pi^2/4` (Euler).
4. **The eigensolver** (first principles). For the pencil `A x = lambda B x`, `A` real and unsymmetric, `B` real: the
   shift-invert operator `T = (A - sigma B)^-1 B` has the eigenvalues `theta = 1/(lambda - sigma)` with the same vectors
   (`B x = theta (A - sigma B) x` gives `A x = (sigma + 1/theta) B x`); the eigenvalues nearest `sigma` are the `theta` of
   largest modulus, the ones Krylov spaces of `T` find first; infinite eigenvalues (null vectors of `B`) have `theta = 0` and
   are not found. Arnoldi's process with classical Gram-Schmidt applied twice (the "twice is enough" rule of Kahan and
   Parlett as reported by Parlett, The Symmetric Eigenvalue Problem, 1980, cited from memory, NOT read) builds `T V_m = V_m H_m
   + h_(m+1,m) v_(m+1) e_m^T`; for an eigenpair `(theta, y)` of `H_m` the Ritz vector `x = V_m y` has `||T x - theta x|| =
   |h_(m+1,m)| |e_m^T y|` (`||y|| = 1`), the estimate used for the convergence test (Saad, Numerical Methods for Large
   Eigenvalue Problems, 1992, chapter 6, cited from memory, NOT read; the identity is derived from the relation above).
   `H_m` is solved by `general_eigen` (ADR 0450). The true residual `||A x - lambda B x|| / (||A x|| + |lambda| ||B x||)` is
   computed for every returned pair and reported. Rounding: the factor of `A - sigma B` (`EnvelopeLu`, ADR 0560 D4; residual
   `8.3e-17` there) and the orthogonalisation keep the true residual at `~1e-14` for a well-conditioned pair; a nearly
   defective pair (the coalescence) has a small residual too, its eigenvalue error being `~sqrt(residual)` relative, `~1e-5`
   at the tolerance `1e-10`. On the Beck problem the split of the pair below `p_cr` is `W_2 - W_1 ~ 3.6 sqrt(delta) W_cr`
   (`delta = 1 - p/p_cr`; `101.18` and `144.13` at `delta = 0.01`), so the classification is exact down to `delta ~ 1e-11`;
   the bisection stops at the bracket width `8/2^30 = 7.5e-9`.
   - Pivots: `EnvelopeLu` factors without row interchanges. For the tangent about Beck's state at `p > pi^2/4` the symmetric
     part is indefinite; the full matrix is nonsingular (no divergence, item 3) and its leading principal minors in an order
     along the beam are clamped segments (positive definite while `4 pi^2 E I/l^2 > P`, every `l <= L` at `p < 39.5`) or
     Beck sub-columns (nonsingular). A failure of the pivot test is a refusal (an error), not a silent result; it would show
     as a failed run.
5. **The estimates of the remaining checks.** F5 (the full solver against the symmetric `tangent_modes` on a symmetric
   tangent): both converge to `1e-10` residuals; the eigenvalues of a symmetric pencil err by `O(residual^2)` in the symmetric
   solver and by `O(residual)` in the Arnoldi one: `<= 1e-10`, limit `1e-8`. F6 (the full buckling path against
   `buckling::factors` on a dead load): likewise, limit `1e-8`. F4, F5 imaginary parts of a real spectrum: a 1 x 1 block of the
   real Schur form is real exactly; two close real eigenvalues may come as a 2 x 2 block with an imaginary part `~ eps
   ||H|| / gap`; the gaps here are `>= 3e-1` relative: `<= 1e-13`, limit `1e-8`. K1 (a pencil of known spectrum, order 120): `<=
   1e-12`, limit `1e-10`.

## Design

### D1. The pencil eigensolver (`rcfd-linalg`, new module `eigen_pencil.rs`)

`pub fn nearest_pencil_eigenpairs(a: &CsrMatrix, b: &CsrMatrix, count: usize, options: &PencilOptions) ->
Result<PencilEigen, PencilError>`: the `count` eigenpairs of `A x = lambda B x` nearest the real shift, by arithmetic 4.

- `PencilOptions { shift: f64 (default 0), tolerance: f64 (default 1e-10), max_steps: usize (default 200), seed: u64 }`,
  `#[non_exhaustive]`, builders. The start vector is uniform in `(-1, 1)` from SplitMix64 with the seed (default
  `0x0580_F653`), normalised.
- `A - sigma B` assembled in CSR and factored by `EnvelopeLu::factor` (reverse Cuthill-McKee, `PivotRule::NonZero`). Each
  step: `w = (A - sigma B)^-1 (B v_j)`, classical Gram-Schmidt twice against `V_j`, `h_(j+1,j) = ||w||`. If `h_(j+1,j) <=
  1e-13` times the norm of `w` before the orthogonalisation (an invariant subspace), a new start vector is drawn and
  orthogonalised and `h_(j+1,j) = 0`.
- From `m = count + 2` on, after every step: the eigenpairs of `H_m` by `general_eigen`; the wanted set is the `count` Ritz
  values of largest `|theta|` (ties by `Im theta` descending), with the conjugate partner of the last one added when it is
  complex and its partner is outside the set (so `count + 1` may be returned); converged when every wanted pair has
  `|h_(m+1,m)| |e_m^T y| <= tolerance |theta|`, or when `m = n` (the Krylov space is the whole space). No convergence by
  `min(max_steps, n)` steps is `PencilError::NotConverged` (the steps and the worst estimate).
- The result `PencilEigen { values: Vec<C64>, vectors: Vec<Vec<C64>>, residuals: Vec<f64>, estimates: Vec<f64>, steps }`:
  `lambda = sigma + 1/theta`, ordered by `|lambda - sigma|` ascending (ties: `Im` descending), vectors `V_m y` of unit 2-norm
  with the entry of largest modulus real and positive, the true residuals of arithmetic 4.
- Refusals (`PencilError`): sizes (`DimensionMismatch`), a non-finite entry (`NonFinite`), `count == 0` or `count >= n`
  (`InvalidCount`), the factor's error (`Factor(SkylineError)`), a zero `theta` among the wanted (`InfiniteEigenvalue`), the
  dense solver's error (`Dense(GeneralEigenError)`).

### D2. The full tangent of the contributions

`Contribution::add_full` (ADR 0560 D1; `FollowerPressure` gives `-K_p` in full) is used unchanged. No change in
`nonlinear.rs` or in `FollowerPressure`.

### D3. `follower::tangent_modes_full` (`follower.rs`, new function)

The same arguments as `tangent_modes`: `K_T` from `nonlinear::tangent` plus the triplets of `add_full` of each contribution at
`(u, load_factor)` (the full, unsymmetric tangent), the consistent mass, both reduced as in `tangent_modes` (the model's
prescribed equations and `prescribed`), the same refusals (the driver's model checks, `modes`, the mass diagonal). The pencil
`K_T phi = omega^2 M phi` by `nearest_pencil_eigenpairs` with `count = options.modes`, `tolerance = options.tolerance`, the shift
`options.shift`: `None` is `0` when the constraints hold every rigid motion, refused (key `"shift"`) otherwise; `Some(Value(s))`
is `s`; `Some(Auto)` refused (key `"shift"`: the rule of ADR 0284 tests a symmetric factor). The result `ComplexModes {
eigenvalues: Vec<C64> (omega^2), modes: Vec<Vec<C64>> (on the model's equations, 0 at prescribed ones), residuals, steps,
unknowns }` with `dofs()`. A solver failure is `FemError::Solver { solver: "nearest_pencil_eigenpairs", .. }`.

### D4. `follower::buckling_factors_full` (`follower.rs`, new function)

The same arguments as `buckling_factors`: `(K + mu (K_sigma - sum K_p)) x = 0`, `K_p` the full load stiffness of each pressure
at zero displacement and load factor 1 (`load_stiffness`), `K` and `K_sigma` assembled and reduced as in
`buckling::factors_with` (the same model checks, zero prescribed values only, the rigid-motion test, the zero-shift pivot rule of
ADR 0402); the pencil `K x = mu B x`, `B = -(K_sigma - sum K_p)` reduced, by `nearest_pencil_eigenpairs` with `count =
options.factors`, `tolerance = options.tolerance`, shift 0: the `count` factors of smallest modulus, complex in general. The
transformation path is refused (key `"load stiffness"`, as ADR 0480 D3). Result `ComplexBuckling { factors: Vec<C64>, modes,
residuals, steps, unknowns }` with `dofs()`. To reach the assembly it calls the crate-internal `geometric::assemble_geometric`,
`assemble::point_stiffness` and `mpc::zero_shift_failing_pivot` (already `pub(crate)` or `pub`; no visibility change unless the
compiler requires one, recorded then).

### D5. Beck's column (tests)

- Model: `hex_box([0, 0, 0], [L, b, h], [50, 1, 1])` upgraded to Hex20, `L = 1`, `b = 0.04`, `h = 0.02` (`L/h = 50`; bending
  in `z` is the weak plane, `E I = E b h^3/12`), `E = 2e11`, `nu = 0`, `rho = 7850` (`m = rho b h`); `xmin` held in `x, y, z`.
  The reference pressure `p_ref = 20 E I/(L^2 b h)` on `xmax` (positive: pressing on the solid, compression). `u_lin`: the
  linear static solution with the dead `Load::pressure("xmax", p_ref)` (a copy of the model with that load). At the load
  coefficient `p` (in `E I/L^2`): `load_factor = p/20`, `u = load_factor u_lin`, `states = initial_states(model)`, the follower
  `FollowerPressure::new(model, "xmax", p_ref)` as the only contribution, `ModalOptions::new(6)`.
- Classification `flutter(p)`: `tangent_modes_full` returns `Ok` and some returned eigenvalue has `|Im| > 1e-6 |lambda|`. Bisection
  on `[16, 24]`: `flutter(16)` must be false and `flutter(24)` true (F1e), then 30 halvings; `p_meas` is the midpoint of the
  final bracket. `Omega^2_meas = Re(lambda) m L^4/(E I)` of the complex pair at the final upper end.
- The control F4: the follower wrapped in a contribution whose `add_full` calls the follower's `add` (the symmetric part), at
  `p = 1.01 p_cr`.
- F5: the model without the follower at `p = 1` (below Euler's `pi^2/4`, so the symmetric `tangent_modes` with shift 0 applies),
  `u = u_lin/20`, no contribution, 4 modes: `tangent_modes_full` against `tangent_modes`.
- F6, F7: `Prestress::from_static` of the dead `u_lin` solve; `buckling_factors_full(model, pre, &[], BucklingOptions::new(2))`
  against `buckling::factors(model, pre, ..new(2))` (F6), `buckling_factors_full(model, pre, &[&fp], ..new(4))` (F7),
  `follower::buckling_factors(model, pre, &[&fp], ..new(1))` (F7c, the symmetric part).

### D6. The bit record N1 (tests)

`crates/rcfd-fem/tests/sa_f653_default_bits.rs`, written after this commit and run on the unchanged base before any library
line, its FNV-1a 64 digests over the bits written into the file then: on the model of D5 (1) `follower::tangent_modes` at `p =
1` with the follower contribution, 4 modes (eigenvalues, modes, residuals, iterations); (2) `follower::buckling_factors` with
the follower, 1 factor; (3) `buckling::factors`, 2 factors (factors, modes, residuals, iterations, Sturm count).

### D7. What changes

New: `rcfd-linalg/src/eigen_pencil.rs` and its `pub mod` line in `lib.rs`; in `follower.rs` the two functions, the two result
types and their imports, and lines of the module documentation. Nothing else; in particular no line of `tangent_modes`,
`buckling_factors`, `buckling.rs`, `nonlinear.rs`.

## Checks (limits fixed now)

| Id | Clause | What | Limit | Estimate behind the limit |
|---|---|---|---|---|
| F1 | a3 | Beck's column by `tangent_modes_full` and bisection: `|p_meas/20.0509536 - 1|` | `<= 1e-2` | `-1.5e-3`, `<= 4e-3` (arithmetic 3) |
| F1e | a3 | the bracket: `flutter(16)` false, `flutter(24)` true | holds | `p_cr ~ 20.0` |
| F2 | a3 | the coalescence: `|Omega^2_meas/121.34251 - 1|` | `<= 2e-2` | `-4e-3` (arithmetic 3) |
| F3 | a1 | every returned pair of every evaluation of F1: residual | `<= 1e-8` | `~1e-12` |
| F4 | a1 (control) | the symmetric part in the full solver at `1.01 p_cr`: every returned eigenvalue `|Im| <= 1e-8 |lambda|`, and the lowest `Re lambda < 0` | holds | real spectrum; `W = -54.5` (arithmetic 3) |
| F5 | a1, a4 | a symmetric tangent (no follower, `p = 1`): 4 eigenvalues of `tangent_modes_full` against `tangent_modes`, `max |lambda_f/lambda_s - 1|`; `max |Im|/|lambda|` | `<= 1e-8`; `<= 1e-8` | `<= 1e-10`; `<= 1e-13` |
| F6 | a2, a4 | dead load only: 2 factors of `buckling_factors_full` against `buckling::factors`, relative; `|Im|/|mu|` | `<= 1e-8`; `<= 1e-8` | `<= 1e-10`; `<= 1e-13` |
| F6r | record | `20 mu_1/(pi^2/4) - 1` (Euler of the solid) | recorded | `~ -1e-3` |
| F7 | a2 | Beck's column, `buckling_factors_full` with the follower, 4 factors: `Ok`, no returned factor real (`|Im| <= 1e-8 |mu|`) with `0 < Re mu <= 2 p_cr/20 = 2.005` | holds | none positive real (arithmetic 3) |
| F7c | a2 (control) | `follower::buckling_factors` (symmetric part) on the same: its factor `mu_1` in `(0, 2.005]` | holds | `pi^2/20 = 0.4935` (arithmetic 3, EB) |
| F8 | a2 | the residuals of F6 and F7 pairs | `<= 1e-8` | `~1e-12` |
| F9 | D3, D4 | refusals: `tangent_modes_full` on the D5 model without constraints (shift `None`) and with `Shift::Auto`, key `"shift"`; `buckling_factors_full` with a linear constraint (transformation path), key `"load stiffness"` | hold | |
| K1 | D1 | a pencil of known spectrum: order 120, `B = diag(1 + u_i)`, `A = B Q D Q^-1`, `Q = I + L` (`L` strictly lower, entries uniform in `(-0.1, 0.1)`), `D` block diagonal with 20 blocks `[[a, c], [-c, a]]` and 80 real values (seed `0x0580_0001`); the 8 nearest `sigma = 0.3`: `max |lambda - lambda_exact| / |lambda_exact|`; residuals | `<= 1e-10`; `<= 1e-10` | `~1e-13`; `~1e-14` |
| K2 | D1 | `A` the 1-D Laplacian `tridiag(-1, 2, -1)` of order 200, `B = I`, 5 nearest 0: against `2 - 2 cos(k pi/201)`, relative; `|Im|/|lambda|` | `<= 1e-10`; `<= 1e-12` | `~1e-13`; 0 |
| K3 | D1 | refusals: `count = 0` and `count = n`; sizes; a NaN; `A - sigma B` singular (`[[1, 1], [1, 1]]`, `B = I`, shift 0, with a third diagonal 1) refused by the factor | hold | |
| N1 | a4 | the digests of D6 at the base | exact | no existing line changed |
| N2 | a4 | the existing default-bit records of `rcfd-fem` (in the release suite of `rcfd-fem`, unchanged) | pass | |
| N3 | a4 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | |

Files: `crates/rcfd-fem/tests/sa_f653.rs` (F1 to F9), `crates/rcfd-linalg/tests/sa_eigen_pencil_f653.rs` (K1 to K3),
`crates/rcfd-fem/tests/sa_f653_default_bits.rs` (N1). A check that would take more than about two minutes in release is an
ignored heavy record run once, with a fast test on the same code path.

## Planned mutations (on a scratch copy, one at a time)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | `tangent_modes_full` calls `add` in place of `add_full` | F1e (no flutter at `p = 24`) |
| MU2 | the pencil returns `sigma + theta` in place of `sigma + 1/theta` | K1, K2, F5 |
| MU3 | the pencil drops the imaginary part of the Ritz values | K1, F1e |
| MU4 | the convergence test accepts at `m = count + 2` whatever the estimate | K1 (or F3) |
| MU5 | `buckling_factors_full` uses the symmetric part of `K_p` | F7 (the positive real factor near `0.49`) |
| MU6 | `buckling_factors_full` with `B = +(K_sigma - sum K_p)` | F6 (the factors' sign) |

## Decision rule

- **done**: every check of the table met, every planned mutation caught by at least one check, no check met only after a
  post-run amendment, fmt and clippy clean, N1 to N3 unchanged.
- **partial**: N1 to N3 unchanged and fmt and clippy clean, but a registered check missed (kept as a `*_known_miss` test with
  its limit and the measured number), or a mutation not caught (none is declared equivalent), or a check met only after a
  post-run amendment.
- **todo**: otherwise, in particular any change of the default bits.

## Limits (not covered)

- The flutter load is found by a bisection in the test, not by a library routine (no "flutter search" API); the library
  returns the complex spectrum at a given state.
- The state about which the modes are taken is the linear one scaled (`u = lambda u_lin`), not a Newton equilibrium: the
  load-controlled driver refuses a tangent with negative pivots (ADR 0560 arithmetic 2), and above Euler's load the symmetric
  part is indefinite. For Beck's straight column the two differ by `O(eps^2)` in the stress.
- The pencil solver: real shifts only, no restart (the Krylov dimension grows to `max_steps`), no pivoting in the factor of
  `A - sigma B` (refused when a pivot fails), eigenvalues of algebraic multiplicity above one with a single vector (exact
  coalescence) returned with the accuracy `~sqrt(residual)`. Rigid-free models need a given shift.
- Shells: `tangent_modes_full` has the driver's model checks (no shells, ADR 0487 D0); `buckling_factors_full` takes the
  follower pressure on shells as the symmetric path does, not checked here. Beams: no follower load on Beam2 (open question 2).
- No case-file key.

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. **The option as new entry points.** Taken: `tangent_modes_full` and `buckling_factors_full` beside the existing functions,
   because a complex result needs another return type and a flag on `ModalOptions` or `BucklingOptions` would make the
   existing functions return a type they cannot carry. The existing functions are not touched (their bits are kept by
   construction).
2. **The follower tip force on a solid face, not a Beam2 load.** Taken: the existing `FollowerPressure` on the end face of a
   Hex20 cantilever, because `tangent_modes` runs on the Newton driver's tangent, which refuses beams
   (`check_supported_nonlinear`), so a Beam2 load would also need a beam tangent path; the solid route uses only code that
   exists and is tested (ADR 0480, ADR 0560), at the cost of the Timoshenko-scale departure (`-2.8e-3`, arithmetic 3) inside the
   1 percent. A Beam2 follower tip force with a linear (pre-stress) flutter path is left as a possible follow-up.
3. **The coefficient.** Taken: the derived `20.0509536` (two independent derivations agreeing to `1.5e-8`) is entered in
   `docs/reference-data.md` as derived, with this registration; the 20.05 of the acceptance is cited from memory and matches.
4. **Case-file keys.** Not added (a schema golden would change).
