# ADR 0560: F-572 registration: arc-length with a follower reference load and an unsymmetric factor of the load stiffness

Status: **REGISTERED 2026-11-15, written before any code of F-572 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-572 was written in the repository. One scratch probe was compiled and run on the
unchanged base 3378a508 and then removed from the worktree (disclosed in arithmetic 3: a test file that calls only API that
existed at the base; its source is `w49i-work/f572-scratch/zz_probe_f572.rs`, its log `f572-scratch/probe.log`). The bit record
N1 of the default paths is a test file written and run on the base after this commit and before any library line (D7). Results
go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-572 (dep F-424), structural solver, advanced tier: the follow-up named by
[ADR 0480](0480-f424-registration-follower-pressure-arc-length-tangent-modes.md) (its limits section and open question 3).
Number 0560 given by the lead. Branch `f572-work` from 3378a508, worktree `w49i-work/f572`, scratch `w49i-work/f572-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited;
no new dependency, no unsafe code, the crate graph unchanged; clean-room. Every choice the maintainer would normally be asked
about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation) and listed at the end.

## What F-572 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. The load direction of the arc-length iteration includes the derivative of the contributions with the load factor, so that a reference load made only of follower pressure is traced; a ring under follower internal pressure traced by arc-length reproduces the load-controlled path of ADR 0480 within 1e-6 at the common load factors; an unsymmetric factor of the tangent gives quadratic convergence on a free-edged follower load (registered rate), and the symmetric path keeps its bits.

Clauses: (a0) every limit registered here; (a1) the load direction of the arc-length iteration includes `d(contributions)/d
lambda`, so that a reference load of follower pressure only is traced; (a2) the ring of ADR 0480 under follower internal
pressure, traced by arc-length, equals the load-controlled path of ADR 0480 within `1e-6` at the common load factors; (a3) an
unsymmetric factor of the tangent gives quadratic convergence on a free-edged follower load, the rate registered here; (a4)
the symmetric path keeps its bits.

Readings, registered:

- **(a1)** The ring of (a2) has no dead load at all: its reference load is the follower pressure alone, which ADR 0480 D4
  refuses ("the load is zero on the free equations"). (a1) is met when that trace runs to the load factor 1 (L1) and the
  refusal stays for the default route (L1c, the control: the new load direction is an option, D3).
- **(a2)** "The load-controlled path of ADR 0480" is the R1 run of ADR 0480 (`sa_follower.rs`, `ring_path(true)`): the quarter
  ring of D5 there with `n = 8`, the follower pressure `p_ref = p_thin(1.51)` on the bore `xmin`, `NewtonOptions::new(20)`,
  `solve_with`. Its 20 load factors `k/20` and the points of an arc-length trace do not coincide, and an interpolation between
  load steps would err by far more than `1e-6`. "The common load factors" are therefore made common: the load-controlled run is
  that R1 run with a `StepControl` hook (the existing hook of ADR 0440 D7) that, inside the 20 user steps, also stops at every
  load factor of the arc-length trace below 1 (it returns `min(next arc-length load factor above the committed one, the
  driver's proposal)`). At each such load factor the two displacement vectors are compared (L2).
- **(a3)** "A free-edged follower load": a cantilever plate (D5) of Hex20 under a follower pressure on its upper face; every
  edge of the loaded face except the clamped one is free, so the antisymmetric part of `K_p` (the edge term of ADR 0480
  arithmetic 3) couples free unknowns and the symmetric part is not the tangent. "Quadratic convergence (registered rate)": the
  quadratic constant `Q = max r_(k+1) / r_k^2` over every step and every iteration `k >= 1` with `r_k <= 1e-2` (`r_k` the
  driver's relative residual, `LoadStep::residuals`), registered below with its limit; the control is the same run with the
  symmetric part (the existing path), whose `Q` must be large (linear convergence).
- **(a4)** "The symmetric path keeps its bits": every call that does not ask for the new options gives the bits it gave at the
  base: the new record N1 (D7) on the paths F-572 touches, the existing records of the driver (`sa_follower_default_bits.rs`
  of ADR 0480, `sa_newton_f618_default_bits.rs` of ADR 0542 and the others in `rcfd-fem/tests`, unchanged and run in the
  release suite) and `baseline_hashes` of `rcfd-api`.

## Audit of refusals that F-572 could lift (done before this registration)

`grep` of `crates/*/tests` for the keys and texts of the refusals near the change: `"arc length"`, "no load direction", "zero
on the free", "Envelope only", `key: "solver"`, and every test that calls `arclength::trace` (`sa_arclength.rs`,
`sa_corot_shell.rs`, `sa_corot_shell_default_bits.rs`, `sa_corot_shell_dyn.rs`, `sa_corot_shell_dyn_default_bits.rs`,
`sa_newton_supernodal_f618.rs`, `sa_scaffold.rs`, `sa_shell3_paths.rs`). Found: `sa_newton_supernodal_f618.rs` N2b
(`DirectFactor::SparseMinimumDegree` refused by `NewtonOptions`, key `"solver"`), N2d (the supernodal factor refused by the
arc-length path, key `"solver"`), N2c (the linear contact body with a supernodal `newton.factor` refused). None asserts the
zero-load refusal of `trace`. F-572 lifts none of these: the zero-load refusal stays on the default route (the new load
direction is an opt-in, `ArcLengthOptions::contribution_load`), the unsymmetric factor is a separate opt-in
(`NewtonOptions::unsymmetric`) that leaves `factor` and its refusals as they are; N2b, N2c and N2d keep passing.

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0480 (whole: D1, D4, arithmetic 1 and 3, the limits, open question 3, the addendum);
  ADR 0542 (D1, D2: the `factor` option of `NewtonOptions` and the refusal of the arc-length path); ADR 0555 for the form;
  the F-572 and F-424 entries of `docs/roadmap.json`.
- `crates/rcfd-fem/src/arclength.rs` (whole), `follower.rs` (whole), `nonlinear.rs` (the `Contribution` and `StepControl`
  traits, `add_triplets`, `contribute`, `NewtonOptions` and its check, `Problem::attempt_with`, `factor_solve`,
  `LoadFactorControl`, `controlled_correction`, `controlled_attempt`, the step loop of `solve_with`), `constraint.rs`
  (`reduce`: it reads the rows of the free equations and is valid for an unsymmetric matrix), `contact.rs` (the checks of
  `ContactOptions`, the use of `newton` and `factor`), `solve.rs` (`PIVOT_TOLERANCE`),
  `rcfd-linalg/src/skyline.rs` (whole: ordering, envelope, `dot4`, the pivot test), `reorder.rs` (`rcm_order`).
- Tests: `sa_follower.rs` (the ring of D5, `Recorder`, `ring_path`), `sa_arclength.rs`, `sa_follower_default_bits.rs` (the form of
  a bit record), `sa_shell3_paths.rs` (a given load scale from a linear solve), `sa_newton_supernodal_f618.rs` (N2).

## Arithmetic done before the code (disclosed)

1. **The load direction** (first principles). The driver's residual is `R(u, lambda) = lambda f - f_int(u) - sum_c g_c(u,
   lambda)`, `g_c` the force a contribution adds to the internal force (ADR 0440 D7: an external load that depends on `u`
   enters with a minus sign). The arc-length iteration of ADR 0480 D4 is Newton's method on `R = 0` together with the
   constraint, with the correction `du = du_r + d lambda du_f`, `K_T du_r = R`, `K_T du_f = dR/d lambda`. ADR 0480 used `dR/d
   lambda = f` (the dead loads). The exact derivative is `f_lambda(u, lambda) = f - sum_c d g_c / d lambda`. For a follower
   pressure `g = -lambda f_p(u)` (D1 of ADR 0480: the face routine receives `lambda p`, and the force is linear in `p`), so
   `d g/d lambda = -f_p(u)|_(lambda = 1)` and `f_lambda = f + f_p(u)`; for the deformation-dependent centrifugal load `g = -lambda
   Omega^2 M_perp u`, `d g/d lambda = -Omega^2 M_perp u`. With `f_lambda` the iteration is Newton's method on the augmented
   system with the exact Jacobian; its quadratic convergence (the spherical constraint is solved exactly at every iteration,
   which does not spoil the rate) is the standard result for Newton's method (Kantorovich's theorem as given in Ortega and
   Rheinboldt, Iterative Solution of Nonlinear Equations in Several Variables, 1970, chapter 12, cited from memory, NOT read; the
   rate is measured here, not assumed). Without it, a follower-only reference load has `f = 0` and `du_f = 0`: no load
   direction, the refusal of ADR 0480.
2. **The unsymmetric factor and its pivots** (first principles). The tangent with the full load stiffness is `K_T - K_p`,
   unsymmetric where the loaded surface has free edges. The factor: `A = L D U` on the envelope of the (reverse Cuthill-McKee)
   ordered, symmetrised pattern, `L` unit lower stored by rows, `U` unit upper stored by columns, no pivoting (Crout's form of
   Gaussian elimination, as in Golub and Van Loan, Matrix Computations, chapter 3, cited from memory, NOT read; the recursions
   are derived here). For `j < i` with `k` over `max(f_i, f_j) .. j - 1` (`f_i` the first envelope column of row `i`): `l~_ij =
   a_ij - sum_k l~_ik u_kj` (`l~_ik = l_ik d_k`, unscaled), `u~_ji = a_ji - sum_k l_jk u~_ki` (`u~_ki = d_k u_ki`), then `l_ij =
   l~_ij / d_j`, `u_ji = u~_ji / d_j`, `d_i = a_ii - sum_j l~_ij u_ji`. Check: `a_ij = sum_(k<j) l_ik d_k u_kj + l_ij d_j` and
   `a_ji = sum_(k<j) l_jk d_k u_ki + d_j u_ji`. The envelope of a structurally symmetric pattern holds `L` and `U` without fill
   outside it (the argument of ADR 0274 D1 for each triangle). On a symmetric matrix `u_ji = l_ij` and the recursions are those
   of `SkylineLdlt`. Existence without pivoting: if the symmetric part of `A` is positive definite, every pivot is positive.
   Proof: with `A = [[a, b^T], [c, A_22]]`, `a = e_1^T A e_1 > 0`; for `y != 0`, `alpha = b^T y`, `beta = c^T y`, `x = (t, y)`
   gives `x^T A x = a t^2 + t (alpha + beta) + y^T A_22 y > 0` for every `t`, so `y^T A_22 y > (alpha + beta)^2 / (4 a)`, and
   the Schur complement `S = A_22 - c b^T / a` has `y^T S y = y^T A_22 y - alpha beta / a > (alpha - beta)^2 / (4 a) >= 0`;
   induction. At a stable state of a load-controlled solve (the case the existing symmetric path accepts with its positive
   pivot test) the symmetric part of `K_T - K_p` is `K_T - K_p,sym`, which that path factors with positive pivots, so the same
   pivot test (`d > PIVOT_TOLERANCE a_ii`) is kept for the unsymmetric factor of the load-controlled driver; the controlled
   (arc-length) correction keeps its test `|d| > PIVOT_TOLERANCE |a_ii|` and counts the negative pivots of `D` (for an
   unsymmetric matrix not an inertia; recorded only).
3. **The probe** (the base 3378a508, existing public API only; `f572-scratch/zz_probe_f572.rs`, `probe.log`, release, run once).
   - P1, the floor of (a2): the ADR 0480 ring (R1 setting) solved by load control with 20, 7 and 13 equal steps; the final
     displacements agree to `max |u_20 - u_s| / max |u_20| = 2.0e-15` (7 steps) and `1.3e-15` (13 steps). The converged states
     of the driver do not depend on the path of increments beyond rounding (the material is elastic and Newton's last step is
     quadratic, so the converged iterate is far below the tolerance `1e-10`). Estimate for L2: `<= 1e-10`, limit `1e-6` (the
     acceptance).
   - P2, the rate of (a3): the plate of D5 at `p = E t^3/(2 L^3)` (a tip rotation of the order of 1 radian; tip deflection
     `0.635 L`, `u_x = -0.31 L`), 5 equal steps, Newton with a dense LU in the probe of `K_T - K_p` (the full matrix from
     `FollowerPressure::load_stiffness`) and with `K_T - K_p,sym`; the residual as the driver measures it (the probe's
     symmetric run reproduces the driver's `LoadStep::residuals` of `solve_with` to the printed digits). Full: in every step
     `r_1 ~ 1` (the predictor's membrane forces), then `r_3 ~ 2e-3 .. 4e-3`, `r_4 ~ 2e-3 .. 9e-3`, `r_5 ~ 2.6e-7 .. 3.9e-7`,
     `r_6 ~ 6e-11`: `r_(k+1)/r_k^2` over `k >= 1, r_k <= 1e-2` at most `8.9e2` (step 5, `k = 3` and `k = 5`), 6 iterations per
     step. Symmetric part: `r_6 / r_5^2 = 1.45e-6 / (3.85e-6)^2 = 9.8e4`, `r_8 / r_7^2 = 6.63e-9 / (1.50e-8)^2 = 2.9e7` (step 5),
     9 to 10 iterations in steps 3 to 5, the late ratios `r_(k+2)/r_k ~ 0.4`: linear convergence. At half the pressure the two
     differ much less (the neglected part is of the order of the tip rotation relative to the bending stiffness: `K_p ~ p b`
     against `E b t^3 / L^3`, the ratio `p L^3/(E t^3) = theta/2`), so the full pressure is registered.
4. **The estimates of the remaining checks.** U3 (the plate's final state by both factors): both converge to `r <= 1e-10`
   relative to the reactions; a residual of that size in a bending direction is amplified by about `(L/t)^2 = 400` relative to
   the membrane scale of the reference: `<= 4e-8` of `max |u|`, limit `1e-6`. U4 (the plate by arc-length with both options
   against the load-controlled unsymmetric path at the common load factors): as L2, both converged states of the same equations;
   the arc-length points converge to the same tolerance; `<= 4e-8`, limit `1e-6`. L3 and U5 (the constraint at every point): the
   quadratic is solved at each iteration, rounding `~1e-14` (ADR 0480 A4 measured `8.5e-15`), limit `1e-10`. The number of
   points: with `c = |u_lin|^2` (the linear solution of the reference load) and `Ds = 0.05 sqrt(2 c)` the first predictor
   splits `Ds` equally (`Dlambda = 0.05`); the ring stiffens (`dp/dlambda_s = E t lambda_s / R`) and the plate stiffens in
   membrane, so `Dlambda` grows slowly: 12 to 20 points to `lambda >= 1` for each, limit 200 points. K1 to K3 (the factor on its
   own): diagonally dominant random matrices of order 300 and half-bandwidth 12, rounding `~ 12 eps ~ 3e-15` per entry of the
   residual, limits `1e-13`; on a symmetric matrix the recursions of arithmetic 2 are those of `SkylineLdlt` up to the order of
   the sums of `d_i`, pivots within `1e-13` relative, solutions within `1e-12` relative (condition number of order 10).

## Design

### D1. Two default methods of `nonlinear::Contribution` (`nonlinear.rs`)

- `fn add_load_factor_derivative(&self, u: &[f64], load_factor: f64, out: &mut [f64]) -> Result<()>`: adds `d g / d lambda` of
  the contribution's force at `(u, lambda)` to `out` (one value per equation). Default: nothing (a force that does not depend
  on the load factor: contact, inertia, the test contributions).
- `fn add_full(&self, u, load_factor, force, tangent) -> Result<()>`: the contribution with its full (possibly unsymmetric)
  tangent. Default: `self.add(u, load_factor, force, tangent)`.

Both are new default methods of a public trait: every existing implementation compiles and keeps its behaviour.

### D2. The follower loads (`follower.rs`)

- `FollowerPressure`: `add_load_factor_derivative` subtracts `force(u, 1)` from `out`; `add_full` subtracts `force(u,
  lambda)` from the force (as `add`) and adds the triplets of `-K_p` (the full matrix of `load_stiffness`, face by face);
  `add` unchanged (the symmetric part).
- `CentrifugalFollower`: `add_load_factor_derivative` subtracts `Omega^2 M_perp u`; `add_full` the default (its tangent is
  symmetric).

### D3. The load direction of the arc-length iteration (`arclength.rs`, `nonlinear.rs`)

- `ArcLengthOptions::contribution_load: bool` (default `false`) and the builder `contribution_load(on)`. With `true`, at every
  iteration of an attempt the load direction is `f_lambda = f - sum_c d g_c / d lambda` at the iterate `(u, lambda)` (the same
  `(u, lambda)` at which the contributions were evaluated for the residual): `du_f = K_T^-1 f_lambda`, the energy's right-hand
  side `R + d lambda f_lambda`. The refusal of a zero load becomes, with the option, the refusal of a zero `f_lambda(0, 0)` on the
  free equations (the same key `"arc length"`, its own text). The default load scale `c` is `|K_0^-1 f_lambda(0, 0)|^2`.
- In `nonlinear.rs`: `LoadFactorControl` gets the default method `fn contribution_load(&self) -> bool { false }`; in
  `Problem::attempt_with` the controlled branch computes `f_lambda` only when the control asks for it and passes it to
  `controlled_correction` in place of `self.f`; `pub(crate) fn load_direction(f, contributions, u, lambda)`. Without the option
  the same operations as at the base.

### D4. The unsymmetric factor (`rcfd-linalg`, `nonlinear.rs`)

- `rcfd_linalg::skyline_lu::EnvelopeLu`: the factor of arithmetic 2. `factor_with(a, ordering: &SkylineOrdering, pivots:
  PivotRule) -> Result<EnvelopeLu, SkylineError>` (square and finite checked, no symmetry test; the orderings, the pivot rule,
  the pivot threshold `theta_i = (w_i + 1) eps (|a_ii| + sum_j |l~_ij u_ji|)` and the error type of `skyline.rs`), `factor(a)`
  (reverse Cuthill-McKee, `PivotRule::NonZero`), `order`, `pivots`, `negative_pivots`, `solve`, `solve_into`. The helpers of
  `skyline.rs` it shares (`dot4`, `first_columns`, `rcm_of`, `check_permutation`) become `pub(crate)`; no other change there.
- `NewtonOptions::unsymmetric: bool` (default `false`) and the builder `unsymmetric(on)`: the contributions are called by
  `add_full` and the reduced tangent is factored by `EnvelopeLu` (reverse Cuthill-McKee), with the pivot test of the symmetric
  path (`d > PIVOT_TOLERANCE a_ii` in the driver; `|d| > PIVOT_TOLERANCE |a_ii|` in the controlled correction, D3), the same
  error texts. It acts wherever `Problem::attempt_with` runs (`solve`, `solve_with`, `solve_from*`, `solve_in_time*`, the total
  Lagrangian contact body through `solve_from`, `arclength::trace`). `NewtonOptions::check` refuses `unsymmetric` with
  `DirectFactor::SupernodalNestedDissection` (key `"solver"`); `ContactOptions::check` refuses it with the linear contact body,
  whose loop does not run the driver's attempt (key `"solver"`, one added condition next to the existing one of ADR 0542 D3).
- `contribute` becomes `contribute_with(.., full: bool)` with `contribute(..) = contribute_with(.., false)`; `attempt_with`
  passes `o.unsymmetric`.

### D5. The models (tests)

- **Ring** (L1 to L3): the quarter ring of ADR 0480 D5 with `n = 8` (`ring_model(8)` copied into the new test file), the
  follower pressure `p_ref = p_thin(1.51) = E t (1.51^2 - 1)/(2 R)` on `xmin`, no dead load. Arc-length: `c = sum u_lin^2`,
  `u_lin` the linear static solution of the same model with the dead `Load::pressure("xmin", p_ref)` (equal to `K_0^-1 f_p(0)`
  on the free equations: S4 of ADR 0480, the follower force at `u = 0` is the dead load bit for bit), `Ds = 0.05 sqrt(2 c)`,
  `max_points = 200`, `NewtonOptions::new(1)` (20 iterations, `1e-10`, `1e-10`), `contribution_load(true)`; stop at the first
  point with `lambda >= 1`. Load control: the R1 run of ADR 0480 (`NewtonOptions::new(20)`, `solve_with`, the follower pressure
  wrapped in a recorder that keeps `(lambda, u)` of every call, as `ring_path` of `sa_follower.rs`) with the hook of reading
  (a2); the converged `u` at a load factor is the last recorded call at that load factor's bits.
- **Plate** (U1 to U5): `hex_box([0, 0, 0], [L, b, t], [10, 1, 1])` upgraded to Hex20, `L = 1`, `b = 0.1`, `t = 0.05`, `E =
  2e11`, `nu = 0`, `rho = 7850`; `xmin` held in `x, y, z`; the follower pressure `p = E t^3/(2 L^3) = 1.25e7` on `zmax`
  (pressing on the solid, `-z` at the start). U1, U2: `solve_with`, `NewtonOptions::new(5)` with `unsymmetric(true)` (U1) and
  without (U2). U4: arc-length with `c = sum u_lin^2` (the linear solution with the dead `Load::pressure("zmax", p)`), `Ds =
  0.05 sqrt(2 c)`, `max_points = 200`, `contribution_load(true)`, `NewtonOptions::new(1).unsymmetric(true)`, stop at the first
  point with `lambda >= 1`; its load-controlled reference the U1 run with the hook of reading (a2) and a recorder.

### D6. The factor on its own (tests, `rcfd-linalg`)

K1: order 300, a random pattern (seed `0xF572_0560`, SplitMix64) of half-bandwidth 12 with each off-diagonal entry present with
probability 1/2 independently in each triangle (a structurally unsymmetric pattern), values uniform in `(-1, 1)`, the diagonal
`1 + sum |a_ij|` of its row; `b` random; `|A x - b|_inf / (|A|_inf |x|_inf)`. K2: the same matrix symmetrised (`(A + A^T)/2`,
the diagonal recomputed): `EnvelopeLu` against `SkylineLdlt` (both reverse Cuthill-McKee): the pivots and the solutions. K3:
refusals: `[[1, 1], [1, 1]]` (natural order) refused with `SkylineError::Pivot`; `[[-1]]` refused by `PivotRule::Positive`,
accepted by `NonZero`; a non-square or non-finite matrix refused.

### D7. The bit record N1 (tests)

`crates/rcfd-fem/tests/sa_f572_default_bits.rs`, written after this commit and run on the unchanged base 3378a508 before any
library line; its digests (FNV-1a 64 over the bits, as ADR 0480 D8) written into the file then. Content: (1) the ring of D5
under the follower pressure by `solve_with`, 4 steps (the symmetric path); (2) the plate of D5 by `solve_with` with the
follower pressure, 2 steps at half the pressure (the symmetric path on a free-edged load); (3) the two-bar truss of ADR 0480 D6
by `trace`, 12 points; (4) the ring with the dead `Load::pressure("xmin", p_ref)` and a follower pressure of `0.1 p_ref` on
`xmin` as a contribution, by `trace`, 6 points (the mixed load of ADR 0480's limits, the dead load direction); (5) the error
text of `trace` with the follower pressure alone (the zero-load refusal); (6) `nonlinear::tangent` plus the follower's `add`
triplets assembled on the ring at a deformed state. Every field of each result in a fixed order (for a path: per point the load
factor, the displacement, the length, the halvings, the negative pivots, the residuals, the energies; the load scale; the
stopped flag).

## Checks (limits fixed now)

| Id | Clause | What | Limit | Estimate behind the limit |
|---|---|---|---|---|
| L1 | a1 | ring, follower pressure only, `trace` with `contribution_load(true)`: returns `Ok`, `stopped` (a point with `lambda >= 1`) within 200 points | holds | 12 to 20 points (arithmetic 4) |
| L1c | a1 (control) | the same without the option: `Err(InvalidValue { key: "arc length" })` (the refusal of ADR 0480 D4) | holds | the default route unchanged |
| L2 | a2 | every arc-length point with `lambda < 1`: `max_q |u_arc - u_lc| / max_q |u_lc|` against the R1 run of ADR 0480 with the hook | `<= 1e-6` each | `<= 1e-10` (P1: `2e-15`) |
| L2n | a2 | the number of common load factors (points with `lambda < 1`) | `>= 10` | 11 to 19 |
| L3 | D3 | ring, every point: `| |Du|^2 + c Dlambda^2 - Ds^2 | / Ds^2` from the stored points (free equations) | `<= 1e-10` each | `~1e-14` |
| U1 | a3 | plate, `unsymmetric(true)`, 5 steps: `Q = max r_(k+1)/r_k^2` over the steps and `k >= 1` with `r_k <= 1e-2` | `<= 1e4` | `8.9e2` (P2) |
| U1i | a3 | the same run: iterations per step (`residuals.len() - 1`) | `<= 7` each | 6 (P2) |
| U2 | a3 (control) | plate, the symmetric path (default options), 5 steps: `Q` as U1 | `>= 1e5` | `2.9e7` (P2) |
| U3 | a3 (companion) | the final displacements of U1 and U2: `max_q |u_U1 - u_U2| / max_q |u_U2|` | `<= 1e-6` | `<= 4e-8` (arithmetic 4) |
| U4 | D3, D4 | plate by `trace` with `contribution_load(true)` and `unsymmetric(true)`: `stopped` within 200 points, and at every point with `lambda < 1` `max_q |u_arc - u_lc| / max_q |u_lc|` against the U1 run with the hook | holds; `<= 1e-6` each | 12 to 20 points; `<= 4e-8` |
| U4r | record | the `Q` of the U4 points (as U1, over the points) | recorded | |
| U5 | D3 | plate, every point of U4: the constraint as L3 | `<= 1e-10` each | `~1e-14` |
| K1 | D4 | `EnvelopeLu`, order 300, structurally unsymmetric: `|A x - b|_inf / (|A|_inf |x|_inf)` | `<= 1e-13` | `~3e-15` |
| K2 | D4 | symmetric input: pivots of `EnvelopeLu` against `SkylineLdlt`, relative; solutions, `max |x_lu - x_ldlt| / max |x_ldlt|` | `<= 1e-13`; `<= 1e-12` | the same recursions |
| K3 | D4 | the refusals of D6 | hold | |
| C2 | D4 | `NewtonOptions::new(1).unsymmetric(true).factor(SupernodalNestedDissection)` refused by `solve` (key `"solver"`); `ContactOptions` with the linear body and `newton.unsymmetric` refused (key `"solver"`) | hold | |
| N1 | a4 | the digests of D7 at the base | exact | the new branches are not taken without the options |
| N2 | a4 | the existing default-bit records of `rcfd-fem` (in the release suite of `rcfd-fem`, unchanged) | pass | no default operation changed |
| N3 | a4 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | no shared output path changed |

Files: `crates/rcfd-fem/tests/sa_f572.rs` (L1 to L3, U1 to U5, C2), `crates/rcfd-linalg/tests/sa_envelope_lu_f572.rs` (K1 to
K3), `crates/rcfd-fem/tests/sa_f572_default_bits.rs` (N1). A check that would take more than about two minutes in release is an
ignored heavy record run once, with a fast test on the same code path.

## Planned mutations (on a scratch copy, one at a time)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | `FollowerPressure::add_load_factor_derivative` adds nothing | L1 (the option's zero-load refusal) |
| MU2 | that derivative with the opposite sign (`+force(u, 1)`) | L1 or L2 |
| MU3 | `FollowerPressure::add_full` adds the symmetric part (as `add`) | U1 |
| MU4 | the driver's unsymmetric branch factors with the symmetric `factor_solve` | U1 (the symmetry test refuses the tangent) |
| MU5 | `EnvelopeLu::solve`: the backward substitution with `L^T` in place of `U` | K1, U1 |
| MU6 | `controlled_correction` ignores `unsymmetric` (the symmetric factor) | U4 |
| MU7 | `trace`'s zero-load test under the option uses the dead `f` | L1 |
| MU8 | `Spherical::contribution_load` returns `false` | L1 |

## Decision rule

- **done**: every check of the table met, every planned mutation caught by at least one check, no check met only after a
  post-run amendment, fmt and clippy clean, N1 to N3 unchanged.
- **partial**: N1 to N3 unchanged and fmt and clippy clean, but a registered check missed (kept as a `*_known_miss` test with
  its limit and the measured number), or a mutation not caught (unless declared equivalent here before the run; none is), or a
  check met only after a post-run amendment.
- **todo**: otherwise, in particular any change of the default bits.

## Limits (not covered)

- The load direction is exact for contributions that implement `add_load_factor_derivative` (the follower pressure and the
  deformation-dependent centrifugal load); every other contribution is taken as independent of the load factor, as before.
- The unsymmetric factor pivots on the diagonal only (no row interchanges): exact for tangents whose symmetric part is positive
  definite (arithmetic 2), refused by the pivot test otherwise. Its cost is about twice that of the symmetric envelope factor.
  The supernodal factor stays symmetric (refused with `unsymmetric`); the buckling path and `tangent_modes` keep the symmetric
  part of the load stiffness.
- The relative residual of the driver keeps its reference (ADR 0404 D5: `max(|lambda f|, |f_int|)` with `f_int` including the
  contributions, over every equation): with a follower load only, the reference is carried by the reactions; a body without
  reactions under a follower load only is not covered (a choice of reference, not changed here).
- No case-file key; no branch switching; prescribed values of the arc-length path still zero (ADR 0480 D4).

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. **Opt-in or default.** Taken: both capabilities are options (`ArcLengthOptions::contribution_load`,
   `NewtonOptions::unsymmetric`), because making the exact load direction the default would change the bits of every existing
   arc-length trace with a follower contribution, and the unsymmetric factor changes every solve with a follower pressure.
   Recommendation for the lead: keep them opt-in until a case-file key exposes them.
2. **Where the factor lives.** Taken: `rcfd-linalg` (`skyline_lu.rs`, next to `skyline.rs` whose ordering, envelope and pivot
   rule it shares), not a private routine of `rcfd-fem`.
3. **Case-file keys.** Not added (a golden of the schema would change): recommended `continuation = { ..., contribution_load =
   true }` and `newton = { unsymmetric = true }`, for the lead.
