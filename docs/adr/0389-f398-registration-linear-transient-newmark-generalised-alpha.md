# ADR 0389: F-398 registration: linear transient dynamics, Newmark and generalised-alpha, Rayleigh damping, load tables

Status: **REGISTERED 2026-11-13, written before any code of F-398 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-398 was written, compiled or run: the code listed under "What was read" was read, and
the arithmetic below was done by hand and with a short scratch script of the closed-form amplification matrix (disclosed under
"Arithmetic"; it does not call rcfd). Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parent: [ADR 0301](0301-transient-dynamics-newmark-generalised-alpha.md) (SI-11, the stub with the acceptance draft), plan
[ADR 0290](0290-structural-solver-intermediate-tier-plan.md) (section 2.3: `transient::Integrator`; section 2.5 item 4: the
integrator state of displacement, velocity, acceleration and time, saved and restored). Roadmap item F-398 (deps F-388, F-373,
F-374, F-383). Size M. Branch `f398-linear-transient` from main ecf12329, worktree `w49i-work/f398`, scratch
`w49i-work/f398-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs and not changed after a run (a miss
becomes an ignored `*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical (this item
fills the stub module `transient.rs`, which today holds two doc lines and no item, and adds a test file; no other library line
changes); no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited; no new
dependency, no unsafe code; clean-room. `mpc.rs` and `modal.rs` (another package works there in parallel) are not edited; the
public `mpc::uses_transformation` is called, read only.

## What F-398 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. Free vibration started in the first axial mode of a bar
> (the discrete eigenvector): with Newmark the amplitude after 100 periods is within 1e-9 and the total energy constant within
> 1e-10; the measured period is long by (omega dt)^2/12 within 5 percent of that amount at omega dt = 0.2 and 0.1, observed order
> >= 1.9. A step load proportional to M phi_1 gives the peak displacement twice the static one within 0.5 percent at 40 steps
> per period. Rayleigh damping with a registered ratio 0.02 on that mode: logarithmic decrement 2 pi zeta/sqrt(1 - zeta^2)
> within 1 percent. Generalised-alpha: with spectral radius 1 the result equals Newmark within 1e-12; with 0.5 and 0.8 a mode at
> omega dt = 1e3 decays per step by that factor within 1e-3 while the period error of the first mode keeps order >= 1.9. A run
> split in two by save and restore has the bits of the unsplit run; results are bit-identical for 1 and 8 threads; a non-positive
> step, a table that does not cover the run and a singular effective matrix are refused.

Clauses: (a1) Newmark amplitude after 100 periods within 1e-9; (a2) total energy constant within 1e-10; (a3) period long by
`Omega^2/12` within 5 percent of that amount at `Omega = omega dt = 0.2` and `0.1`; (a4) observed order >= 1.9; (a5) step load
`M phi_1`, peak twice the static within 0.5 percent at 40 steps per period; (a6) Rayleigh ratio 0.02 registered, logarithmic
decrement within 1 percent; (a7) generalised-alpha with `rho_inf = 1` equals Newmark within 1e-12; (a8) `rho_inf = 0.5, 0.8`: a
mode at `Omega = 1e3` decays per step by `rho_inf` within 1e-3; (a9) the first-mode period error keeps order >= 1.9 for those
two; (a10) save and restore splits a run with the bits of the unsplit run; (a11) bit-identical for 1 and 8 threads; (a12)
refusals of a non-positive step, of a table that does not cover the run, of a singular effective matrix.

## What was read (no run)

- ADR 0301 (whole) and ADR 0290 sections 2.3 to 2.5 and the SI-11 entry.
- `crates/rcfd-fem/src/transient.rs` (stub, two doc lines), `lib.rs` (module declared), `modal.rs` (`modal_analysis`,
  `ModalOptions::new`, the reduction by `constraint::reduce` of `stiffness_with` and `consistent_with`, zero prescribed values
  only, modes expanded with 0 at prescribed equations), `mass.rs` (`consistent_with`: assembled on the stiffness pattern,
  bit-identical for any thread count), `solve.rs` (`direct`: `SkylineLdlt::factor` and the pivot test `d_i <= PIVOT_TOLERANCE
  a_ii`, `PIVOT_TOLERANCE = 1e-10`), `constraint.rs` (`prescribed_equations`, `reduce`, `ReducedSystem::expand`), `mpc.rs` lines
  1 to 60 (`uses_transformation`), `loads.rs` (`nodal_load_vector`, `NodalForceAt`), `error.rs` (`InvalidValue`, `FieldLength`),
  `rcfd-linalg/src/skyline.rs` (options, `SkylineError::Pivot`), `rcfd-linalg/src/matrix.rs` (`CsrMatrix::from_triplets` sums
  duplicates in input order after a stable sort), `tests/sb_modal.rs` (the fixed-free bar, `model_of`, `Checks`),
  `tests/si_scaffold.rs` (X1 uses `transient` by path only; no refusal row of `transient` exists, so none is retired).

## Design

### D1. The equations and the two methods

The reduced system (prescribed unknowns eliminated, all prescribed values zero): `M a + C v + K u = f(t)`, `M`, `K` the reduced
consistent mass and stiffness, `C = alpha_R M + beta_R K` (Rayleigh). State at step `n`: `u_n, v_n, a_n`, time `t_n = t_0 + n
dt` (computed from the integer `n`, never accumulated).

**Newmark (1959), from memory and derived here:** the displacement and velocity are advanced with an assumed acceleration,

    u_{n+1} = u_n + dt v_n + dt^2 ((1/2 - beta) a_n + beta a_{n+1}),   v_{n+1} = v_n + dt ((1 - gamma) a_n + gamma a_{n+1}),

and equilibrium is imposed at `t_{n+1}`. `beta = 1/4, gamma = 1/2` is the average acceleration (trapezoidal) rule: for a single
undamped oscillator it gives the amplification `(1 - Omega^2/4 +- i Omega)/(1 + Omega^2/4)` of modulus 1 (so `E = v^T M v/2 +
u^T K u/2` is conserved exactly in exact arithmetic for `f = 0`, `C = 0`, being the quadratic invariant of the trapezoidal rule)
and phase `omega_h dt = 2 atan(Omega/2)`, so the period ratio is `T_h / T = Omega / (2 atan(Omega/2)) = 1 + Omega^2/12 +
O(Omega^4)`.

**Generalised-alpha (Chung and Hulbert 1993, from memory; the parameters re-derived here):** the same two update formulas, with
equilibrium at intermediate points

    M a_{n+1-alpha_m} + C v_{n+1-alpha_f} + K u_{n+1-alpha_f} = f_{n+1-alpha_f},   x_{n+1-alpha} = (1 - alpha) x_{n+1} + alpha x_n,

with `f_{n+1-alpha_f} = (1 - alpha_f) f(t_{n+1}) + alpha_f f(t_n)` (the weighted end loads, not the table at the intermediate
time: with this choice `alpha_m = alpha_f = 1/2` is algebraically the Newmark method, see below). Parameters from the
high-frequency spectral radius `rho_inf in [0, 1]`:

    alpha_m = (2 rho_inf - 1)/(rho_inf + 1),  alpha_f = rho_inf/(rho_inf + 1),  gamma = 1/2 - alpha_m + alpha_f,
    beta = (1 - alpha_m + alpha_f)^2 / 4.

Derivation of the limit (first principles). Scaled state `(u, P, S) = (u, dt v, dt^2 a)` of one undamped oscillator; as `Omega
-> infinity` the equilibrium row `(1 - alpha_m) S_{n+1} + alpha_m S_n + Omega^2 ((1 - alpha_f) u_{n+1} + alpha_f u_n) = 0`
becomes `(1 - alpha_f) u_{n+1} + alpha_f u_n = 0`, one eigenvalue `lambda_1 = -alpha_f/(1 - alpha_f)`. On `u = 0` the two
update rows give, for an eigenvalue `lambda`, `P = -(1/2 - beta + beta lambda) S` and `(lambda - 1) P = ((1 - gamma) + gamma
lambda) S`, so `beta lambda^2 + (gamma + 1/2 - 2 beta) lambda + (1/2 - gamma + beta) = 0`. Its discriminant is `(gamma +
1/2)^2 - 4 beta` (expanded), zero for `beta = (gamma + 1/2)^2/4`, the double root then `lambda_{2,3} = 1 - 2/(gamma + 1/2)`. With
`gamma = 1/2 - alpha_m + alpha_f` (Chung and Hulbert's second-order condition, cited; checked by the order checks A4, A9):
`lambda_{2,3} = (alpha_f - alpha_m - 1)/(1 - alpha_m + alpha_f)`. Setting `lambda_1 = lambda_{2,3} = -rho_inf` gives `alpha_f =
rho_inf/(1 + rho_inf)` and `alpha_m = alpha_f - (1 - rho_inf)/(1 + rho_inf) = (2 rho_inf - 1)/(1 + rho_inf)`: the formulas
above. All three roots meet at `-rho_inf` at infinity (a triple root). `rho_inf = 1` gives `alpha_m = alpha_f = 1/2`, `gamma =
1/2`, `beta = 1/4`; the generalised-alpha residual is then the mean of the Newmark residuals at `n` and `n + 1`, so starting from
an equilibrium acceleration `a_0` it reproduces the Newmark steps exactly in exact arithmetic (with rounding: equal to about
`1e-15` relative, the residual recursion `r_{n+1} = -r_n` does not amplify the rounding).

**One step, acceleration form (both methods; Newmark is `alpha_m = alpha_f = 0`):** predictors `u~ = u_n + dt v_n + dt^2 (1/2 -
beta) a_n`, `v~ = v_n + dt (1 - gamma) a_n`; then

    S a_{n+1} = (1 - alpha_f) f_{n+1} + alpha_f f_n - M w_M - K w_K,
    S = c_M M + c_K K,  c_M = (1 - alpha_m) + (1 - alpha_f) gamma dt alpha_R,  c_K = (1 - alpha_f) beta dt^2 + (1 - alpha_f) gamma dt beta_R,
    w_V = (1 - alpha_f) v~ + alpha_f v_n,  w_M = alpha_m a_n + alpha_R w_V,  w_K = (1 - alpha_f) u~ + alpha_f u_n + beta_R w_V,

and `u_{n+1} = u~ + beta dt^2 a_{n+1}`, `v_{n+1} = v~ + gamma dt a_{n+1}`. `S` is formed once (`CsrMatrix::from_triplets` of
`c_M M` and `c_K K`, deterministic) and factored once by `SkylineLdlt::factor` (RCM, positive pivots); two matrix-vector
products and one solve per step. The initial acceleration solves `M a_0 = f(t_0) - C v_0 - K u_0` with a factor of `M` (made at
the start; the reduced system, so a constrained model is handled, as ADR 0301 asks).

**Refusal of a singular matrix:** a factorisation error of `SkylineLdlt` or a pivot `d_i <= PIVOT_TOLERANCE S_ii` (the rule of
the static solve, `1e-10`) is refused with `FemError::InvalidValue { key: "effective matrix", .. }` naming the reduced equation
(and node and component on the model path); the same for `M` with key `"mass matrix"`.

### D2. Rayleigh damping

`Rayleigh { mass: alpha_R, stiffness: beta_R }`, both finite and `>= 0`, `C = alpha_R M + beta_R K` never formed. Modal damping
ratio `zeta(omega) = alpha_R/(2 omega) + beta_R omega/2`. Constructor `Rayleigh::from_ratio(zeta, omega_1, omega_2)` with the
ratio at two frequencies: `alpha_R = 2 zeta omega_1 omega_2/(omega_1 + omega_2)`, `beta_R = 2 zeta/(omega_1 + omega_2)` (solve
the two equations); with `omega_1 = omega_2 = omega`: `alpha_R = zeta omega`, `beta_R = zeta/omega`, each half of `zeta`.
**Registered for (a6): `zeta = 0.02` at the bar's first discrete frequency `omega_1`, `Rayleigh::from_ratio(0.02, omega_1,
omega_1)`.** Exact decrement `delta = 2 pi zeta/sqrt(1 - zeta^2) = 0.125688846`.

### D3. Load tables

`LoadTable::new(points: Vec<(f64, f64)>)`: at least two points, every time and value finite, times strictly increasing; the value
is piecewise linear between points (the segment found by `partition_point`, the value at a point exactly the point's value).
Refused with key `"load table"`. A table covers `[t_a, t_b]` when `t_first <= t_a` and `t_b <= t_last` (exact comparison, no
slack). `LoadHistory { pattern, table }`: `f(t) = sum_j table_j(t) pattern_j` (the sum in the order given). A pattern is a full
vector (one value per equation of the model's `DofMap`, or of the matrix size on the matrix path), finite; refused with
`FemError::FieldLength` or key `"load pattern"`. `LoadHistory::model_loads(model, table)`: the pattern of the model's own loads
(`loads::nodal_load_vector(model, &[])`). The model's own loads are not applied unless given this way.

Coverage is checked at the start (every table covers `t_0`) and before every `run(steps)` and `step()` (every table covers
`[t_n, t_{n + steps}]`); a refused call leaves the state unchanged (bits).

### D4. The integrator and its state

`Integrator::new(model, loads, options)`: refuses a model on the transformation path (`mpc::uses_transformation`: linear
constraints, rigid sets, supports; key `"constraint"`, a limit of this item), a non-zero prescribed value (key `"constraint"`, as
the modal analysis), checks `model.validate()` and `check_supported()`, assembles `K` and `M` with `options.threads` (bit-identical
for any value, ADR 0284), reduces both by `reduce` with the prescribed equations, forms and factors `S`. A model with a free rigid
motion is accepted (`S` is positive definite through `M`). `Integrator::from_matrices(k, m, loads, options)`: the same on given
symmetric matrices without constraints (the two-unknown model of A8 and A12c).

`TransientOptions { method, time_step, damping: Option<Rayleigh>, threads: Option<usize> }`; `Method::Newmark { beta, gamma }`
(finite, `>= 0`; the stability is the caller's choice), `Method::average_acceleration()` (`1/4, 1/2`, the default),
`Method::GeneralisedAlpha { rho_inf }` (`0 <= rho_inf <= 1`). The step `dt` must be finite and `> 0`: key `"time step"`.

`start(t_0, u_0, v_0)` (full vectors, zero at prescribed equations or refused with key `"initial state"`), `step()`, `run(steps)`,
`time()`, `step_index()`, `displacement()`, `velocity()`, `acceleration()` (full vectors, 0 at prescribed equations),
`kinetic_energy()` `= v^T M v/2`, `strain_energy()` `= u^T K u/2` (reduced).

`TransientState { t0, step, u, v, a }` (reduced vectors, public), with the step and the method's four parameters and the
signature carried privately: `state()` clones it, `restore(state)` checks the lengths and that `dt` and `(alpha_m, alpha_f,
beta, gamma)` have the integrator's bits (key `"state"`). `to_bytes()`: magic `RCFDTRS1`, then little-endian `u64` length and
step, the `f64` bits of `t0`, `dt`, the four parameters, then `u`, `v`, `a`; `TransientState::from_bytes` refuses a wrong magic,
a wrong length or a truncated buffer (key `"state"`). Restoring and stepping repeats the arithmetic of the unsplit run exactly:
every value the step uses (`u_n`, `v_n`, `a_n`, `t_n` from `t0` and `n`, the loads recomputed from the tables, the factor rebuilt
from the same matrices) has the same bits.

No thread is used by the steps (sequential matrix-vector products and solve), so the step bits do not depend on the thread count;
the assembly is bit-identical for any thread count already.

### D5. The set-ups

**Bar B.** `hex_box([0; 3], [1.0, 0.1, 0.1], [20, 1, 1])` (Hex8), `E = 2e11`, `nu = 0`, `rho = 7850`; constraints
`Constraint::displacement("all", 1, 0.0)`, `("all", 2, 0.0)`, `("xmin", 0, 0.0)`: only the axial unknowns of the 80 nodes off
the root are free (80 unknowns), a fixed-free axial chain. Its lowest mode is the quarter-wave axial mode (a mode with `u_x`
varying across a section has shear stiffness `G/b^2 ~ 1e13` against `E (pi/2L)^2 ~ 5e11`; the modal analysis is asked for 2
modes and the first is used). `phi_1`, `lambda_1` from `modal_analysis(&model, &ModalOptions::new(2))`; `omega_1 = sqrt(phi_1^T K
phi_1)` with `K` assembled by `assemble::stiffness` and `phi_1` M-normalised (the Rayleigh quotient; equal to `sqrt(lambda_1)` to
the eigen-residual squared). Modal coordinates: `q_n = phi_1^T M u_n`, `p_n = phi_1^T M v_n` (full `M` from `mass::consistent`).
`T_1 = 2 pi/omega_1`; `dt = Omega/omega_1`.

**Two-unknown model Q.** `M = I_2`, `K = R diag(omega_1^2, omega_2^2) R^T`, `R` the rotation by `theta = 0.3`, `omega_1 = 1`,
`dt = 0.1`, `omega_2 = 1e4` (`Omega_2 = 1e3`) and, for A8d, `omega_2 = 1e7` (`Omega_2 = 1e6`); `phi_2 = (-sin theta, cos
theta)`.

**Measuring the decay per step (a8).** ADR 0301 asks for a mode isolated in the discrete model; the per-step factor of a mode
is the spectral radius of the one-step amplification matrix of that mode, measured with the integrator itself: for the three
scaled unit states `(q, p/omega_2, s/omega_2^2) = e_j` set `u = q phi_2`, `v = p phi_2`, `a = s phi_2` by `restore`, take one
`step()` without loads, project back (`phi_2^T u`, `phi_2^T v/omega_2`, `phi_2^T a/omega_2^2`): column `j` of `A`. `r = max
|root|` of `det(lambda I - A)` (the three roots by the Durand-Kerner iteration on the monic cubic, 200 sweeps, in the test). The
state need not be an equilibrium state for this (the generalised-alpha state carries `a` as its own variable).

### Arithmetic before this registration (disclosed)

1. Average acceleration, closed form: period error `e(Omega) = Omega/(2 atan(Omega/2)) - 1`: `e(0.2) = 3.32449e-3` (ratio to
   `Omega^2/12` 0.997347), `e(0.1) = 8.32779e-4` (ratio 0.999334); order `log2(e(0.2)/e(0.1)) = 1.99713`.
2. **Generalised-alpha spectral radius at finite `Omega` (scratch script `f398-scratch/amp.py`, numpy eigenvalues of the 3 x 3
   amplification of D1, the equations as written above):** `rho_inf = 0.5`: `r(1e3) = 0.507847168`, `r(1e4) = 0.501683`,
   `r(1e6) = 0.5000780082`; `rho_inf = 0.8`: `r(1e3) = 0.807721246`, `r(1e4) = 0.801675`, `r(1e6) = 0.8000778751`. The approach
   to `rho_inf` is like `Omega^(-2/3)` (a triple root at infinity, D1, perturbed by `O(Omega^-2)` moves by `O(Omega^(-2/3))`;
   `1e3^(-2/3) = 0.01`). **So the method itself decays per step at `Omega = 1e3` by `rho_inf + 7.8e-3` (0.5) and `rho_inf +
   7.7e-3` (0.8), outside the 1e-3 of clause (a8).** The clause is registered unchanged (A8) and is expected to miss for this
   reason; the companion checks A8c (the measured factor against the closed form at `1e3`) and A8d (the clause's limit at `Omega =
   1e6`, where the method is `7.8e-5` from `rho_inf`) are registered beside it. This is a defect of the acceptance text, not of
   the code, and goes to the maintainer as a remaining question; it is not re-worded here.
3. Generalised-alpha first-mode period error (same script): `rho_inf = 0.8`: `e(0.2) = 3.50886e-3`, `e(0.1) = 8.79024e-4`, order
   1.99703; `rho_inf = 0.5`: `4.96978e-3`, `1.24809e-3`, order 1.99346; `rho_inf = 1`: as Newmark.
4. Newmark with Rayleigh damping `zeta = 0.02` on one oscillator: the discrete decrement `-2 pi ln|lambda| / arg lambda` is
   `0.125480` at `Omega = 0.1` (relative to 0.125689: `-1.66e-3`), `-4.09e-3` at 40 steps per period, `-6.6e-3` at `Omega =
   0.2`. **Registered for (a6): `Omega = 0.1`.**
5. Step load (a5): `Omega = 2 pi/40 = 0.157080`, discrete phase per step `2 atan(Omega/2) = 0.156918`; the discrete solution
   from rest is `q_n = q_s (1 - cos(n omega_h dt))`, `q_s = F/omega_1^2`; the sample nearest the half period is `n = 20`
   (`pi/0.156918 = 20.0206`), phase offset `3.2e-3` rad, `1 - cos` short of 2 by `5.2e-6`: expected `|peak/(2 u_s) - 1| ~
   3e-6`. The static `u_s` is `solve_static_with` with the nodal forces of `M phi_1` (`u_s = phi_1/lambda_1` up to the eigen-
   residual, about 1e-10).
6. Energy and amplitude (a1, a2): exact conservation, rounding only: about `1e-16` per step, over 6284 steps (`Omega = 0.1`,
   100 periods) a random walk of `~1e-14`, a drift at worst `~1e-12`. Expected below `1e-12`.
7. Zero-crossing period measure: linear interpolation of `q_n` between the two samples around a crossing is wrong in time by at
   most `Omega^3/24 / omega` (third order: the curvature of a sine vanishes at its zero); over 100 periods the relative error of
   the mean period is below `Omega^3/(24 x 200 pi) = 5e-7` at 0.2, i.e. `1.6e-4` of `Omega^2/12`. Expected A3: `|ratio - 1| =
   2.65e-3` and `6.7e-4`.
8. Decrement measure: peaks of `q_n` refined by the parabola through the three samples around each interior maximum; `delta =
   ln(P_1/P_11)/10` over ten periods (the first maximum after `t = 0` is `P_1`). The parabola's error at `Omega = 0.1` is
   `O(Omega^4) ~ 1e-5` per peak, divided by 10 in `delta`; expected A6 `-1.66e-3 +- 1e-4`.

## Registered checks

All in `crates/rcfd-fem/tests/si_transient.rs`, release profile, run once (`CARGO_BUILD_JOBS=4 cargo test -p rcfd-fem --release
--test si_transient -- --nocapture`). Relative differences are `|x - y|/|y|`. The bar B and model Q of D5.

### A. The acceptance

| Id | Clause | Measure | Limit | Estimate |
| --- | --- | --- | --- | --- |
| A1 | a1 | B, average acceleration, `u_0 = phi_1`, `v_0 = 0`, `Omega = 0.2` and `0.1`, `N = ceil(100 T_1/dt)` steps: `|A_N/A_0 - 1|`, `A = sqrt(q^2 + (p/omega_1)^2)` | each <= 1e-9 | ~1e-13 (arith. 6) |
| A2 | a2 | same runs: `max_n |E_n/E_0 - 1|`, `E = kinetic_energy() + strain_energy()` | each <= 1e-10 | ~1e-13 |
| A3 | a3 | same runs: period `T_m` from the zero crossings of `q_n` (linear interpolation, first to last crossing, `T_m = 2 (t_last - t_first)/(crossings - 1)`), `e = T_m/T_1 - 1`: `|e/(Omega^2/12) - 1|` | each <= 0.05 | 2.65e-3, 6.7e-4 |
| A4 | a4 | `log2(e(0.2)/e(0.1))` of A3 | >= 1.9 | 1.997 |
| A5 | a5 | B, average acceleration, from rest, one load history `M phi_1` (full `M`) with the table `[(0, 1), (10 T_1, 1)]`, `dt = T_1/40`, 60 steps: `max_n u_tip,n` (the `u_x` of node `xmax[0]`) against `2 u_s,tip` of the static solve: `|peak/(2 u_s) - 1|` | <= 0.005 | ~3e-6 |
| A6 | a6 | B, average acceleration, `Rayleigh::from_ratio(0.02, omega_1, omega_1)`, `u_0 = phi_1`, `Omega = 0.1`, 12 periods: `|delta/(2 pi 0.02/sqrt(1 - 0.0004)) - 1|`, `delta` as arith. 8 | <= 0.01 | 1.66e-3 |
| A7a | a7 | B, free vibration of A1 at `Omega = 0.2`, 10 periods: `max_n ||u_GA,n - u_NM,n||_inf / max_n ||u_NM,n||_inf`, `rho_inf = 1` | <= 1e-12 | ~1e-15 |
| A7b | a7 | B, the step load of A5 with the damping of A6, 5 periods, the same measure | <= 1e-12 | ~1e-15 |
| A8 | a8 | Q, `Omega_2 = 1e3`, `rho_inf = 0.5` and `0.8`: `|r - rho_inf|` (D5) | each <= 1e-3 | **expected miss: 7.85e-3, 7.72e-3** (arith. 2) |
| A9 | a9 | B, `rho_inf = 0.5` and `0.8`, `u_0 = phi_1`, `Omega = 0.2` and `0.1`, 100 periods, `e` as A3: `log2(e(0.2)/e(0.1))` | each >= 1.9 | 1.993, 1.997 |
| A10 | a10 | B, `rho_inf = 0.8`, the damping of A6, from rest, history `M phi_1` with the ramp `[(0, 0), (T_1, 1), (10 T_1, 1)]`, `dt = T_1/40`, 200 steps; split after step 77: `to_bytes`, a new integrator built from the model, `from_bytes`, `restore`, 123 steps: every `u, v, a` of steps 78 to 200 equal to the unsplit run's | bit-equal | equal (D4) |
| A11 | a11 | the unsplit run of A10 built with `threads = 1` and `threads = 8`: every `u, v, a` of steps 0 to 200 | bit-equal | equal (D4) |
| A12a | a12 | `time_step` of 0, -1e-4, NaN, +inf: `Integrator::new` refused, `FemError::InvalidValue` key `"time step"` | 4 refusals | D4 |
| A12b | a12 | the A10 set-up with the table ending at `2 T_1`: `run(200)` refused (key `"load table"`) and the state's bits unchanged; `run(40)` accepted; a table starting at `1e-3 T_1` refused by `start(0, ..)` | as stated | D3 |
| A12c | a12 | `from_matrices` with `K = diag(1, -4)`, `M = I_2`, `dt = 1`, average acceleration (`S = M + K/4 = diag(1.25, 0)`): refused, key `"effective matrix"` | refused | D1 |

### C. Companions and further checks

| Id | Measure | Limit | Estimate |
| --- | --- | --- | --- |
| A8c | Q, `Omega_2 = 1e3`: `|r - r_closed|`, `r_closed` = 0.5078471683 and 0.8077212458 (arith. 2) | each <= 1e-6 | ~1e-12 |
| A8d | Q, `Omega_2 = 1e6`: `|r - rho_inf|` (the clause's limit where the method meets it) | each <= 1e-3 | 7.8e-5 |
| C1 | B, the start of A6: `||M a_0 + C v_0 + K u_0 - f_0||_inf / ||K u_0||_inf` (reduced) | <= 1e-12 | ~1e-15 |
| C2 | the table: values at the points exact; at the midpoint of a segment `|value - mean|` | 0 bits; <= 1e-15 | exact |
| C3 | refusals, each with its key: table with 1 point, non-increasing times, a NaN value (`"load table"`); a pattern of the wrong length (`FieldLength`); `rho_inf = 1.5`, `rho_inf = -0.1`, Newmark `beta = -1` (`"method"`); Rayleigh `-1` (`"damping"`); a model with a non-zero prescribed value and a model with a nodal spring (`"constraint"`); an initial displacement non-zero at a prescribed equation (`"initial state"`); `step()` before `start` (`"state"`); `from_bytes` with a bad magic, with a truncated buffer; `restore` of a state of another `dt`, of another method (`"state"`) | each refused as stated | D3, D4 |

### N. Negative controls

| Id | Control | Expected |
| --- | --- | --- |
| N1 | A2's measure with Newmark `gamma = 0.6`, `beta = 0.3025` (numerical dissipation), `Omega = 0.2`, 100 periods | `> 1e-10` (the energy check sees dissipation) |
| N2 | A7a's measure with `rho_inf = 0.8` against Newmark | `> 1e-12` |
| N3 | A3's measure with `rho_inf = 0.5` at `Omega = 0.1` | `> 0.05` (ratio 1.498, arith. 3: the period check discriminates) |
| N4 | A10 with the restored state's `step` set to 0 (time shifted; the ramp load differs) | not bit-equal |

### S. Existing tests

| Id | Measure | Limit |
| --- | --- | --- |
| S1 | `git diff main --stat -- crates` lists only `transient.rs` and `tests/si_transient.rs`; `cargo test -p rcfd-fem --release` (every test of the crate) | 0 failed (no shared solver file touched, so no baseline-hash run is owed) |

### Mutations (scratch copy of `transient.rs`, not committed)

| Id | Mutation | Expected to be caught by |
| --- | --- | --- |
| Mu1 | the generalised-alpha load `f_{n+1}` only (the `alpha_f f_n` term dropped) | A7b |
| Mu2 | `beta_R` dropped from `w_K` (no stiffness-proportional damping in the step) | A6 (half the decrement) |
| Mu3 | predictor `(1/2 + beta)` instead of `(1/2 - beta)` | A1, A2, A3 |
| Mu4 | `to_bytes` writes step 0 | A10 |
| Mu5 | the coverage check of `run` removed | A12b |
| Mu6 | `alpha_m` and `alpha_f` exchanged | A8c, A9 or A7 (whichever) |
| Mu7 | the pivot test `d_i <= 1e-10 S_ii` removed (only the factor's own positive-pivot rule left) | A12c, or survives (the factor refuses a zero pivot itself; recorded) |

## Decision rule

- **done**: every A check met (A1 to A12c) and every C and S check met, the negative controls as expected.
- **partial**: any A check missed with a diagnosed cause, kept as a `*_known_miss` record; or a C or S check missed. With A8
  expected to miss (arith. 2), **partial is the expected outcome** unless the maintainer re-words clause (a8).
- **todo**: the checks could not be run.

## Limits of this registration

- Linear only; no Newton inside the step (nonlinear transient is another package), no prescribed motion history (non-zero or
  time-varying prescribed displacements are refused), no transformation path (linear constraints, rigid sets and supports are
  refused with key `"constraint"`; a follow-up).
- The model's own loads only as a pattern scaled by a table; follower or time-varying spatial patterns other than sums of scaled
  fixed patterns are not covered. Temperature loads enter the pattern as `nodal_load_vector` gives them.
- One direct factorisation; no iterative solver of `S`; no adaptive step; no change of `dt` within an integrator (a restore with
  another `dt` is refused).
- Only Rayleigh damping; no modal or element damping.
- The decay check measures one isolated mode of a two-unknown model; the bar's high modes are not measured.
- The thread check covers the assembly thread count only (the steps are sequential).

## Amendment 1 (2026-11-13, written after the registered run of commit 9cc83fa3 and before the corrected run)

The registered run (`cargo test -p rcfd-fem --release --test si_transient -- --nocapture --test-threads=1`, once) stopped the
test `a8c_a8d_decay_against_the_closed_form_and_at_omega_dt_1e6` after its first row: A8c for `rho_inf = 0.5` was measured
(`2.833744e-11`, met), then `Integrator::from_matrices` refused model Q with `omega_2 = 1e7` as `"effective matrix"`: "singular
at equation 0: pivot 1.0989 at most 1e-10 of the diagonal 2.5876e10". A8c for `rho_inf = 0.8` and both rows of A8d were not
measured.

Diagnosis: a defect of the set-up, not of the method. With `omega_1 = 1` and `omega_2 = 1e7` the eigenvalues of `K` differ by
`1e14`, so the effective matrix `S` has a pivot `~1e-10.6` of its diagonal after the RCM order eliminates the coupled row; the
registered pivot rule of D1 (`d_i <= 1e-10 S_ii` refused, the rule of the static solve) treats such a matrix as singular, as
it was registered to. The rule stays; the set-up changes.

Correction (fixed now, before the corrected run): A8d uses model Q with `omega_1 = 1e4` (`Omega_1 = 1e3`) and `omega_2 = 1e7`
(`Omega_2 = 1e6`), `dt = 0.1`, `theta = 0.3` as before: the eigenvalue ratio of `K` is `1e6`, the smallest pivot of `S` about
`1e-6` of its diagonal. Mode 2 stays isolated (a distinct eigenvalue; the measurement projects on `phi_2` only, D5). Limits
unchanged: A8d `|r - rho_inf| <= 1e-3` (estimate `7.8e-5`, arithmetic 2 unchanged since the amplification of a mode depends on
its own `Omega` only), A8c `|r - r_closed| <= 1e-6` for both `rho_inf` on the unchanged `omega_1 = 1`, `omega_2 = 1e4` model.
The corrected test `a8c_a8d_amended` runs A8c for both values (the `rho_inf = 0.5` row repeats deterministic code and must give
the bits of the registered run, `2.833744e-11` as printed; it is a reproduction, not a second pick) and A8d for both values. The
old test stays as an ignored record.

A8 missed as arithmetic 2 predicted (`7.847168e-3`, `7.721246e-3` against `1e-3`); it becomes the ignored
`a8_decay_per_step_at_omega_dt_1e3_known_miss` with its limit intact.

Limit added to the registration's limits by this finding: the relative pivot rule refuses an effective matrix whose pivots
fall below `1e-10` of the diagonal, so a model whose effective stiffness spans more than about ten decades (here a two-unknown
model with `(omega_2/omega_1)^2 = 1e14`) is refused although it is non-singular.

## Amendment 2 (2026-11-13, after the mutation runs and before the run of the check it adds)

Mutation Mu1 (the generalised-alpha load `f_{n+1}` only) survived: A7b, registered to catch it, uses the constant table of A5,
on which `f_n = f_{n+1}` at every step, so the mutation changes nothing there; the ramp of A10 and A11 compares the integrator
with itself. A check is added, nothing registered is changed:

| Id | Clause | Measure | Limit | Estimate |
| --- | --- | --- | --- | --- |
| A7c | a7 | B, the A10 set-up (ramp `[(0, 0), (T_1, 1), (10 T_1, 1)]`, damping of A6, `dt = T_1/40`, 200 steps, from rest) with `rho_inf = 1` against average-acceleration Newmark: the measure of A7a | <= 1e-12 | ~1e-15 (D1: equal in exact arithmetic with the weighted end loads) |

Test `a7c_ramp_load_generalised_alpha_one_is_newmark`, run once; then Mu1 is run again against it.
