# ADR 0393: F-490 registration: the transient integrator on the transformation path and with prescribed motion histories

Status: **REGISTERED 2026-11-13, written before any code of F-490 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-490 was written, compiled or run: the code listed under "What was read" was read, and
the arithmetic below was done by hand and with a short scratch script of the one-unknown oscillator (`f490-scratch/base.py`,
disclosed under "Arithmetic"; it does not call rcfd). Results go in the addendum at the end; nothing above the addendum is
changed after a run.

Parent: [ADR 0389](0389-f398-registration-linear-transient-newmark-generalised-alpha.md) (F-398, `transient::Integrator`; its
limits and remaining question 3 propose this item), [ADR 0296](0296-linear-multi-point-constraints-rigid-sets-elastic-supports.md)
(`mpc`: `x = T q + g`), [ADR 0384](0384-f486-registration-modal-analysis-on-the-transformation.md) (modal analysis on the
transformation; the spring bar KB). Roadmap item F-490 (deps F-398). Branch `f490-transient-transformation`, worktree
`w49i-work/f490`, scratch `w49i-work/f490-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs and not changed after a run (a miss
becomes an ignored `*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical (every
model without a linear constraint, a rigid set, an elastic support or a motion history runs exactly the lines of ADR 0389);
no edit of `baseline_hashes.rs`, of the hash tables or of a golden file; no existing test edited except as the maintainer's
standing decision of 2026-11-13 allows (below); no new dependency, no unsafe code; clean-room. Only `transient.rs` of the
library changes; `mpc.rs`, `modal.rs` and the element and assembly files (another package, F-395, works there in parallel) are
not edited: the public `mpc::transformation`, `mpc::add_supports`, `mpc::uses_transformation` and `Transformation::{t, g,
independent, n_mesh, n_unknowns, expand, transpose_times, reduce}` are called, read only.

## What F-490 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. A bar with a nodal spring at its free end (the support
> of ADR 0384) started in its first discrete mode: with Newmark the amplitude after 100 periods within 1e-9 and the period long
> by (omega dt)^2/12 within 5 percent of that amount at omega dt = 0.2 and 0.1; a rigid-set end gives the same history as the
> equivalent prescribed-only model within 1e-12; a base displacement given by a table gives the relative motion of the
> closed-form base-excited oscillator within 1 percent at 40 steps per period; models without these features give the bits of
> ADR 0389's integrator.

Clauses: (b1) spring bar, Newmark, amplitude after 100 periods within 1e-9; (b2) its period long by `Omega^2/12` within 5
percent of that amount at `Omega = omega dt = 0.2` and `0.1`; (r1) a rigid-set end gives the history of the equivalent
prescribed-only model within 1e-12; (x1) a base displacement from a table gives the relative motion of the closed-form
base-excited oscillator within 1 percent at 40 steps per period; (d1) models without these features keep the bits of ADR
0389's integrator.

Reading of (b1): "the support of ADR 0384" is the end spring of ADR 0384's model KB (`NodalSpring("xmin", 0, k/4)`, `k = 5e8`
N/m, on the end that carries no prescribed displacement; the other end free). That model is used unchanged. Reading of (r1):
a rigid set cannot have its reference motion prescribed (ADR 0296 has no such row), so "a rigid-set end" is the rigid set on
the free end of ADR 0384's model RB (`RigidSet("xmax", (1, 0.05, 0.05))` on ADR 0389's bar B), and the equivalent
prescribed-only model is bar B itself: for the histories compared (started in B's first mode, or loaded by `M phi_1`) B's motion
is uniform over each section (D5), so it satisfies every row of the rigid set and is the motion of RB too (derivation in D5).

## What was read (no run)

- ADR 0389 (whole), ADR 0384 (whole: method, D1, D2, the models KB and RB), ADR 0296 (D1 to D5: the rows, the elimination,
  `T`, `g`, the reduction `K_r = T^T K_ext T`, `f_r = T^T (f_ext - K_ext g)`).
- `crates/rcfd-fem/src/transient.rs` (all), `mpc.rs` (lines 1 to 370: `uses_transformation`, `Transformation` and its methods;
  `transformation`, lines 520 to 645; `add_supports`; `modal_analysis_mpc`, lines 1195 to 1379), `constraint.rs`
  (`Prescribed`, `prescribed_equations`: a repeated equation with another value is refused), `modal.rs` (`ModalResult`
  fields), `model.rs` (`Constraint::{linear, rigid_set}`, `ElasticSupport::nodal_spring`).
- Tests: `si_transient.rs` (all; the C3 row "nodal spring" asserts the refusal this package retires), `si_modal_mpc.rs` (KB,
  RB, `model_of`), `si_harmonic.rs` (uses `Integrator::new` on a prescribed-only model).
- **Not read:** any finite-element code, manual or documentation outside this repository.

## Design

### D1. The equations on the transformation with a moving `g`

ADR 0296 writes every admissible motion as `x = T q + g`, `x = (u, r)` over all `N` unknowns (mesh, then six reference
unknowns per rigid set). Here the values of the constraint rows may depend on time: `g(t) = sum_j s_j(t) g_j` (D2), `T`
fixed. Then `xdot = T qdot + gdot`, `xddot = T qddot + gddot`, and the admissible variations are `delta x = T delta q` at every
instant (the constraint is holonomic and linear; `g` does not vary with `q`). The equation of motion of the unconstrained body
is `M_ext xddot + C_ext xdot + K_ext x = f_ext + rho`, `rho` the constraint force, which does no work on admissible variations:
`T^T rho = 0` (ADR 0296's statement `T^T rho = K_r q - f_r` for the static case is the same identity). `K_ext` is the stiffness
with the supports (ADR 0296 D4), `M_ext` the consistent mass with zero rows and columns at the reference unknowns (ADR 0384),
`C_ext = alpha_R M_ext + beta_R K_ext` (Rayleigh of ADR 0389 D2, on the stiffness with the supports). Pre-multiplying by `T^T`
(Galerkin on the admissible variations, derived; Lagrange's equations with the time-dependent holonomic constraint give the same):

    M_r qddot + C_r qdot + K_r q = T^T f_ext(t) - T^T M_ext gddot - T^T C_ext gdot - T^T K_ext g,
    M_r = T^T M_ext T,  K_r = T^T K_ext T,  C_r = alpha_R M_r + beta_R K_r.

With prescribed displacements only, `T` is the selection of the free equations and `T^T A g = A_fp g_p`: the free equations
receive `-M_fp g_p'' - C_fp g_p' - K_fp g_p`, the inertia and damping terms of the brief and the familiar stiffness term. With
`g = sum_j s_j g_j` each history `j` contributes three fixed reduced patterns,

    p_K,j = T^T K_ext g_j,   p_M,j = T^T M_ext g_j,   p_C,j = alpha_R p_M,j + beta_R p_K,j,

and the effective load is `f_eff(t) = sum_i table_i(t) T^T pattern_i - sum_j (s_j(t) p_K,j + s_j'(t) p_C,j + s_j''(t) p_M,j)`.
This `f_eff` enters ADR 0389's step unchanged: Newmark and generalised-alpha (with the weighted end values `(1 - alpha_f)
f_eff(t_{n+1}) + alpha_f f_eff(t_n)`), the effective matrix `S = c_M M_r + c_K K_r` formed and factored once, the initial
acceleration from `M_r a_0 = f_eff(t_0) - C_r qdot_0 - K_r q_0`. The state `(t0, n, u, v, a)` holds `q, qdot, qddot` (the
reduced vectors); save and restore are those of ADR 0389.

Output (full vectors of all `N` unknowns): `displacement() = T q + sum_j s_j(t_n) g_j`, `velocity() = T qdot + sum_j
s_j'(t_n) g_j`, `acceleration() = T qddot + sum_j s_j''(t_n) g_j`. `kinetic_energy()` and `strain_energy()` keep ADR 0389's
reduced definitions (`qdot^T M_r qdot/2`, `q^T K_r q/2`, the springs included); with a motion they are the energies of `T q`,
not of the whole motion (a limit, stated). `equilibrium_residual()` is `M_r a + C_r v + K_r q - f_eff(t)`.

### D2. Motion histories and how `g'` and `g''` are taken from the table

`MotionHistory { prescribed: Vec<Prescribed>, table: LoadTable }`: per-node prescribed displacements (`Prescribed { node,
component, value }`, the type of `constraint.rs`) whose values are amplitudes scaled by the table, `u_(node, component)(t) =
value * s(t)`. `g_j` is the `g` of `mpc::transformation(model, extra_j)`, `extra_j` = every history's entries, with the values
of history `j` and 0 for the others (so the rows and `T` do not depend on `j`; `T` and the independent list of every `j` are
compared with those of `extra` all zero, and a difference is refused, key `"motion history"`). Since every row value of the
model itself is zero (below), `g` is linear in the history values and `g_j` is exact (no subtraction).

Refused, key `"motion history"`: an amplitude that is not finite; an equation named twice (in one history or in two); an
equation the model's own constraints already prescribe. The model's own rows keep zero values: a non-zero `Displacement`
value or `Linear` value is refused with key `"constraint"` (reason "zero values only; give a MotionHistory"), as ADR 0389 and
ADR 0384 refuse it; a time-dependent `Linear` value is not covered (a limit).

**Derivatives (registered).** The table gives `s` only; `s'` and `s''` at a step time `t` are the central differences of the
table with the integrator's step:

    s'(t) = (s(t + dt) - s(t - dt)) / (2 dt),   s''(t) = (s(t + dt) - 2 s(t) + s(t - dt)) / dt^2,

`t + dt` and `t - dt` evaluated as floating-point sums of the step time `t_n = t0 + n dt` and `dt`. So a motion table must cover
`[t_a - dt, t_b + dt]` for a run over `[t_a, t_b]` (exact comparison as ADR 0389 D3; refused with key `"motion table"` at
`start`, `run` and `step`, the state unchanged; also by `velocity()` and `acceleration()` at a state outside it). **Order:** for
a history `s` with four continuous derivatives the differences err by `dt^2 s'''/6` and `dt^2 s''''/12`: second order, the
order of the average-acceleration and generalised-alpha steps, so the scheme stays second order (checked: X3). The response is
that of the table's piecewise linear interpolant `s_h`: at a kink of `s_h` the second difference spreads the slope jump over
the two steps next to it (an impulse of the right total, `sum_n dt s''(t_n)` telescopes to the change of `s'`); when the table
points lie on the step times the differences are those of the samples. A history given coarsely is the user's model; a smooth
history should be sampled at `dt` or finer. Alternatives not taken: the Newmark relations inverted for the prescribed unknowns
(`a_{n+1} = 4 (g_{n+1} - g_n - dt v_n)/dt^2 - a_n` for average acceleration), whose error is not damped (the factor `-1` of the
trapezoidal rule) and depends on a start value of `g''` the table does not give; and velocity or acceleration tables given by
the user (a later option; not needed by the acceptance). Taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation).

### D3. The API (all in `transient.rs`, additive)

- `MotionHistory::new(prescribed, table)`.
- `Integrator::with_motion(model, loads, motions, options)`. If `motions` is empty and `!mpc::uses_transformation(model)` it is
  exactly ADR 0389's `Integrator::new` (the same lines; clause d1). Otherwise the transformation path of D1: `validate`,
  `check_supported`, the zero-value checks, the motion checks, `tr = transformation(model, extra_0)`, `K = stiffness_with +
  add_supports`, `M = consistent_with`, `K_r`, `M_r` by `tr.reduce` with a zero vector (ADR 0384's reduction, unchanged), the
  patterns `p_K,j`, `p_M,j`, `p_C,j`, the load patterns `T^T pattern` (a pattern of length `n` (mesh) or `N`; the mesh form is
  padded with zero reference loads), then ADR 0389's `build` (the effective matrix and its factor; a factor refusal names the
  reduced unknown: node and component, or "reference unknown `i`").
- `Integrator::new(model, loads, options)` becomes `with_motion(model, loads, &[], options)`: a prescribed-only model runs the
  old lines; a model with linear constraints, rigid sets or supports, refused until now (key `"constraint"`), is now solved
  (the retired refusal, below). A model with a non-zero prescribed value and no transformation keeps ADR 0389's refusal.
- `start(t0, u0, v0)` on the transformation path: full vectors of length `N`; `q_0 = u0[independent]`, `qdot_0 = v0[independent]`;
  refused (key `"initial state"`) if `|u0 - (T q_0 + g(t0))|_inf > 1e-9 max(|u0|_inf, |T q_0 + g(t0)|_inf)`, the same for `v0`
  with `gdot(t0)` (an inadmissible start; the tolerance admits the rounding of `T`'s coefficients and of a mode computed
  elsewhere). New `start_reduced(t0, q0, qdot0)` (either path): the reduced vectors directly. New `transformation()` (`Option`)
  and `n_unknowns()`.
- `from_matrices` unchanged.

### D4. Bits of the old path (clause d1)

`with_motion` with no motion on a prescribed-only model calls the old body of `new` line for line (the refusal of the
transformation is replaced by the dispatch before it). The functions shared by both paths (`build`, `load`, `check_coverage`,
`advance`, `expand`, `reduced`, `start`) gain a branch or a loop over the motion list that is empty on the old path: no
floating-point operation is added, removed or reordered there. Measured by a digest (check D1) of every saved state of three
ADR 0389 runs, recorded from the unchanged library before any library line of F-490 is written (amendment 1 records the value
and is committed before the library code), and by ADR 0389's own tests.

### D5. The set-ups

**KB (spring bar, ADR 0384).** `hex_box([0; 3], [1.0, 0.1, 0.1], [20, 1, 1])`, `E = 2e11`, `nu = 0`, `rho = 7850`,
`NodalSpring("xmin", 0, k/4)`, `k = 5e8` N/m, `Displacement("ymin", 1, 0)`, `Displacement("zmin", 2, 0)`. Mode 1 from
`modal_analysis(&kb, &ModalOptions::new(4))` (now on the transformation, ADR 0384; mode 0 is axial, ADR 0384 B1), `omega_1 =
sqrt(lambda_1)`, `phi_1` M-orthonormal. Modal coordinates `q_n = phi_1^T M u_n`, `p_n = phi_1^T M v_n` (full consistent `M` of
the mesh); amplitude `A = sqrt(q^2 + (p/omega_1)^2)`; the period from the zero crossings of `q_n` as ADR 0389 A3. Start `u0 =
phi_1`, `v0 = 0` (admissible: the rollers are exactly 0 on the mode, ADR 0384 H1).

**B and RB (rigid-set end).** B is ADR 0389's bar (`Displacement("all", 1, 0)`, `("all", 2, 0)`, `("xmin", 0, 0)`; 80 free
axial unknowns; old path). RB is B plus `RigidSet("xmax", (1.0, 0.05, 0.05))` (transformation path; `N = n + 6`). Why the
histories agree: with `nu = 0` and one element across the section, B's stiffness and mass map motions uniform over each section
(`u_x` equal at the four nodes of a section) to forces uniform over each section (ADR 0384's derivation for the bar), so this
subspace `U` is invariant under every step of the integrator (each step is a combination of `M`, `K` and solves of `c_M M + c_K
K` applied to vectors of `U`; the load `M phi_1` lies in `M U`). A motion of `U` satisfies every rigid-set row of RB (`u_x(a) =
t_x` with `w = 0`; the lateral rows hold through the rollers). RB's admissible space `range(T) + 0` contains `U` (with `t_x =
u_x(tip)`), and the reduced step of RB is the Galerkin restriction of B's step to its admissible space, so B's discrete
trajectory, lying in `U`, solves RB's discrete equations: the two histories are equal in exact arithmetic, and the difference
is rounding. Start for RB: `phi_1` of B padded with the reference unknowns `t_x = phi_1(tip)`, the others 0.

**X (base-excited oscillator).** `hex_box([0; 3], [1.0, 0.1, 0.1], [1, 1, 1])` (one Hex8, 8 nodes), `E = 2e11`, `nu = 0`, `rho =
7850`, `Displacement("all", 1, 0)`, `("all", 2, 0)`; one `MotionHistory` with `Prescribed(node, 0, Y)` for the four `xmin`
nodes, `Y = 1e-3` m, table `s(t) = 1 - cos(Omega_b t)` sampled at `t_j = j dt`, `j = -1, 0, ..., J` (`J dt` past the run's end
plus one step). Free: the four axial unknowns of `xmax`. By the invariance above the four move together, and with the section
sums of the consistent mass (`rho A L/6 [[2, 1], [1, 2]]`, ADR 0384) and of the stiffness (`E A/L [[1, -1], [-1, 1]]`), the
free end `u` satisfies `m u'' + m_c g'' + E A/L (u - g) = 0`, `m = rho A L/3`, `m_c = rho A L/6`, `g = Y s`. The relative
motion `z = u - g`:

    z'' + omega^2 z = -(3/2) g'',   omega^2 = 3 E/(rho L^2),

the base-excited oscillator with the participation `(m + m_c)/m = 3/2` of the consistent mass (derived; a lumped mass would give
1). `Omega_b = omega/2`, from rest (`z = z' = 0`; the table is even about 0, so `gdot(0) = 0` exactly and the rest state is
admissible). Closed form: `z(t) = -(3/2) Y Omega_b^2/(omega^2 - Omega_b^2) (cos(Omega_b t) - cos(omega t)) = -(Y/2)(cos(omega
t/2) - cos(omega t))`. With Rayleigh damping `Rayleigh::from_ratio(zeta, omega, omega)` (`alpha_R = zeta omega`, `beta_R =
zeta/omega`): `C 1 = alpha_R M 1` (`K 1 = 0` for the rigid translation), so `z'' + 2 zeta omega z' + omega^2 z = -(3/2)(g'' +
alpha_R g')`, solved in the test as the particular harmonic solution plus the damped homogeneous solution with `z(0) = z'(0) =
0` (formulae in `base.py`). `omega = 8742.6038` rad/s, `T = 7.1868581e-4` s; `dt = T/40` for X1.

Measure of X: `e_X = max_n |z_n - z(t_n)| / max_n |z(t_n)|` over `n = 1..40` (one period of the oscillator), `z_n` = the `u_x`
of the first `xmax` node minus the `u_x` of the first `xmin` node, both from `displacement()`.

### Arithmetic before this registration (disclosed)

1. Spring bar (b1, b2): the step is ADR 0389's on another symmetric pair `(K_r, M_r)`; the amplitude and the period of one mode
   depend on `Omega` only (ADR 0389 D1). Expected as ADR 0389 A1 and A3: amplitude `~1e-14`, `|e/(Omega^2/12) - 1| = 2.6e-3`
   and `6.7e-4` (the closed-form ratio `0.997347`, `0.999334`, and the zero-crossing measure of ADR 0389 arith. 7), order
   1.997.
2. Rigid end (r1): equal in exact arithmetic (D5); rounding `~1e-16` per step, the phase walk over 315 steps (`Omega = 0.2`, 10
   periods) `~1e-14`. Expected `~1e-14`.
3. Base excitation, scratch script `f490-scratch/base.py` (the one-unknown absolute equation of D5 stepped with average
   acceleration, `g'` and `g''` by the central differences of D2 on samples at the step times; the closed forms above):
   undamped, one period at 40 steps per period: `e_X = 4.541e-3`; at 80: `1.140e-3`, order `1.994`; at 20: `1.794e-2`. Two
   periods at 40: `1.157e-2` (the phase error of the free part `Omega^2/12` per radian grows linearly; this is why one period
   is registered). Damped `zeta = 0.05`, one period: `4.314e-3` at 40, `1.080e-3` at 80, order `1.998`. Mutations: the
   inertia term `-m_c g''` dropped `0.332`; the damping pattern dropped (`zeta = 0.05`) `2.75e-2`; a one-sided second
   difference `(s(t) - 2 s(t - dt) + s(t - 2 dt))/dt^2` `2.11e-2` at 40, `9.49e-3` at 80 (order 1.15).
4. Negative control N1: the closed form with participation 1 instead of 3/2 is `2/3` of the true `z`: `e ~ 0.33`.

## Registered checks

All in the new file `crates/rcfd-fem/tests/si_transient_mpc.rs`, release profile, run once (`CARGO_BUILD_JOBS=4 cargo test -p
rcfd-fem --release --test si_transient_mpc -- --nocapture --test-threads=1`). Relative differences are `|x - y|/|y|`.

### A. The acceptance

| Id | Clause | Measure | Limit | Estimate |
| --- | --- | --- | --- | --- |
| B1 | b1 | KB, average acceleration, `u0 = phi_1`, `v0 = 0`, `Omega = 0.2` and `0.1`, `N = ceil(100 T_1/dt)` steps: `|A_N/A_0 - 1|` | each <= 1e-9 | ~1e-14 (arith. 1) |
| B2 | b2 | same runs: `|e/(Omega^2/12) - 1|`, `e = T_m/T_1 - 1` (zero crossings) | each <= 0.05 | 2.6e-3, 6.7e-4 |
| R1 | r1 | RB against B, average acceleration, start of D5, `Omega = 0.2`, 10 periods: `max_n |u_RB,n - u_B,n|_inf / max_n |u_B,n|_inf` (mesh part) | <= 1e-12 | ~1e-14 (arith. 2) |
| R2 | r1 | RB against B, ADR 0389's A10 set-up (from rest, history `M phi_1` (mesh pattern) with the ramp `[(0, 0), (T_1, 1), (10 T_1, 1)]`, `rho_inf = 0.8`, Rayleigh 0.02 at `omega_1`, `dt = T_1/40`, 200 steps): the measure of R1 | <= 1e-12 | ~1e-14 |
| X1 | x1 | X, average acceleration, undamped, `dt = T/40`, 40 steps: `e_X` | <= 0.01 | 4.54e-3 (arith. 3) |
| D1 | d1 | digest (FNV-1a 64 over `to_bytes()` of every state, step 0 included, in run order) of three runs through `Integrator::new` and `from_matrices`: (i) B, Newmark, `u0 = phi_1`, `Omega = 0.2`, 200 steps; (ii) ADR 0389's A10 set-up, 200 steps; (iii) ADR 0389's model Q (`omega_2 = 1e4`), `rho_inf = 0.5`, `u0 = (1, 0.5)`, `v0 = 0`, 50 steps; plus the bits of `displacement()`, `velocity()`, `acceleration()`, `kinetic_energy()`, `strain_energy()` at the end of each | equal to the digest of the unchanged library (amendment 1) | equal (D4) |
| D2 | d1 | the existing `si_transient` tests (except the one retired below) and `si_harmonic`, release | pass | D4 |

### C. Companions and further checks

| Id | Measure | Limit | Estimate |
| --- | --- | --- | --- |
| B3 | KB runs of B1: `max_n |E_n/E_0 - 1|`, `E = kinetic_energy() + strain_energy()` (with the spring) | each <= 1e-10 | ~1e-13 |
| B4 | KB: `log2(e(0.2)/e(0.1))` | >= 1.9 | 1.997 |
| X2 | X with `Rayleigh::from_ratio(0.05, omega, omega)`, 40 steps, `e_X` against the damped closed form | <= 0.01 | 4.31e-3 |
| X3 | X1 and X2 at 80 steps per period over the same period (80 steps): `log2(e_X(40)/e_X(80))` | each >= 1.9 | 1.994, 1.998 |
| X4 | X damped, `rho_inf = 1` against average-acceleration Newmark, 80 steps at `dt = T/40`: the measure of R1 (all unknowns) | <= 1e-12 | ~1e-15 (ADR 0389 D1: equal for any load sequence) |
| X5 | X1: `displacement()` at each moved equation equals `Y * table.value(t_n)` | bit-equal at every step | D1 (`T` has no entry there, `g_j` holds `Y`) |
| X6 | X2 at the start: `|equilibrium_residual()|_inf / |f_eff(t0)|_inf`, `f_eff(t0)` taken as `|M_r a_0|_inf` (the state is at rest) | <= 1e-12 | ~1e-15 |
| X7 | X damped, `rho_inf = 0.8`, 80 steps: split after 29 by `to_bytes`, a new integrator, `from_bytes`, `restore`; and threads 1 against 8 | bit-equal | ADR 0389 D4 |
| Q1 | refusals, each with its key: a history on an equation the model prescribes; one equation in two histories; a NaN amplitude (`"motion history"`); a motion table starting at `t0` (not `t0 - dt`) refused by `start`, and `run` past `t_last - dt` refused with the state's bits unchanged (`"motion table"`); KB plus `Displacement("xmax", 0, 1e-3)` and KB plus a `Linear` value 1e-3 (`"constraint"`); RB with one tip node's `u_x` changed by `1e-3 |phi|_inf` (`"initial state"`); KB with a load pattern of length 3 (`"load pattern"`) | each refused as stated | D2, D3 |
| Q2 | `with_motion(B, loads, &[], o)` against `Integrator::new(B, loads, o)` on the A10 set-up, 200 steps | bit-equal | D3 |

### N. Negative controls

| Id | Control | Expected |
| --- | --- | --- |
| N1 | X1's measure against the closed form with participation 1 instead of 3/2 | `> 0.1` (arith. 4: ~0.33) |
| N2 | R1's measure with both started from a state outside `U`: `u_x = phi_1(x) sigma(y, z)` with `sigma = +1, -1, +1, -1` around each section (a warping of the section) at the nodes with `x < 1` and 0 at the tip (admissible in both) | `> 1e-6` (the rigid set holds the tip section flat, B does not) |
| N3 | B2's measure for KB with `Omega^2/12` replaced by `Omega^2/6` (the period check discriminates the factor) | `> 0.05` |

### S. Existing tests and the retired refusal

The C3 row "nodal spring" of `si_transient.rs::c2_c3_tables_and_refusals` (ADR 0389) asserts that `Integrator::new` refuses a
model with a nodal spring, which this package retires. By the maintainer's standing decision of 2026-11-13 that test is marked
`#[ignore]` with a reason naming ADR 0393 as a record (its body untouched), and a copy `c2_c3_tables_and_refusals_after_landed`
without that row is added (every other row stays valid). No other existing test asserts a refusal of the transient integrator
(`si_scaffold.rs` uses `transient` by path only, ADR 0389).

| Id | Measure | Limit |
| --- | --- | --- |
| S1 | `git diff main --stat -- crates` lists `transient.rs`, `tests/si_transient_mpc.rs` and the permitted edit of `tests/si_transient.rs` only; `cargo test -p rcfd-fem --release` (the crate) and `cargo check -p rcfd-post --tests` | 0 failed; builds |

### Mutations (scratch copy of `transient.rs`, not committed)

| Id | Mutation | Expected to be caught by |
| --- | --- | --- |
| Mu1 | the inertia pattern `p_M` of a motion dropped from `f_eff` | X1 |
| Mu2 | `s''` by the one-sided difference `(s(t) - 2 s(t - dt) + s(t - 2 dt))/dt^2` (coverage widened to `t - 2 dt`) | X1 or X3 |
| Mu3 | the supports not added to `K` on the transient transformation path | B1, B2 |
| Mu4 | `displacement()` without the motion part `sum_j s_j g_j` | X1, X5 |
| Mu5 | the damping pattern `p_C` dropped | X2 |
| Mu6 | the motion coverage `[t_a - dt, t_b + dt]` reduced to `[t_a, t_b]` | Q1 (accepted, or a panic) |
| Mu7 | every model dispatched to the transformation path | D1 (a survival is recorded and explained, not repaired: with prescribed rows only `K_r = T^T K T` may have the bits of `reduce`) |
| Mu8 | `p_K` dropped | X1 |

## Decision rule

- **done**: every A check met (B1, B2, R1, R2, X1, D1, D2), every C and S check met, the negative controls as expected, and
  Mu1 to Mu6 and Mu8 caught.
- **partial**: any A check missed with a diagnosed cause (kept as a `*_known_miss` record), or a C or S check missed, a
  negative control not as expected, or one of those mutations surviving.
- **todo**: the checks could not be run, or D1 fails (the default bits changed).

## Limits of this registration

- Linear only. Motion histories prescribe mesh displacements (`Prescribed`) scaled by tables; a time-dependent `Linear` value,
  a prescribed reference motion of a rigid set and a prescribed velocity or acceleration table are not covered; the model's own
  constraint values must be zero on the transformation path.
- `g'` and `g''` are central differences of the table with the integrator's step (D2); the table must cover one step beyond
  each end of the run. The response is that of the piecewise linear table.
- Rayleigh damping is taken on the stiffness with the supports (`C_ext = alpha_R M_ext + beta_R K_ext`); no damping of the
  supports alone.
- `kinetic_energy()` and `strain_energy()` are of the reduced motion `T q` (not of `g`); no reaction or constraint force output
  in time.
- The cyclic constraint is refused as before (`check_supported` or the transformation of ADR 0386 for `k = 0` only; not checked
  here). Only Hex8 models are checked.
- The rigid-set check compares histories that stay in the section-uniform subspace (D5); a history that bends the section is
  only seen by N2 (that the two differ), not compared with a closed form.

## Open points taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. `g'` and `g''` by central differences of the table with the integrator's step (D2), not by inverting the Newmark relations
   and not by user-given velocity and acceleration tables.
2. The model's own constraint values stay zero on the transformation path; motions come only from `MotionHistory`.
3. `start` on the transformation path takes full vectors of all `N` unknowns and checks admissibility with the relative
   tolerance 1e-9; `start_reduced` takes the reduced vectors.
4. Rayleigh damping on `K_ext` (with the supports).
5. The energies of the reduced motion, as ADR 0389 defines them.
