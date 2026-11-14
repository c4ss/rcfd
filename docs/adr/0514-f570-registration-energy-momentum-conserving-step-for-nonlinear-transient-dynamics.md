# ADR 0514: F-570 registration: the energy-momentum conserving step (Simo and Tarnow) for nonlinear transient dynamics

Status: **REGISTERED 2026-11-14, written before any code of F-570 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-570 was written, compiled or run in the repository. The files under "What was read"
were read; the arithmetic under "Arithmetic before this registration" was done by hand and by a plain Python/numpy probe in the
scratch folder `w49i-work/f570-scratch` (`probe_emc.py`, `probe_emc_run.py` with `probe_emc_run.log`; no rcfd code; disclosed,
its numbers quoted below). The bit record of the default paths (B0) is a test file written and run on the base d41012db after
this commit and before any library line (D8). Results go in the addenda at the end; nothing above the addenda is changed after a
run.

Roadmap item F-570 (deps F-423), follow-up of ADR 0479. Number 0514 given by the lead. Branch `f570-work` from main d41012db,
worktree `w49i-work/f570`, scratch `w49i-work/f570-scratch`. House rules `w49i-work/AGENT_RULES.md`: limits fixed here, a miss
becomes an ignored `*_known_miss` test with the limit intact and the number recorded, each registered run made once, defaults
bit-identical, no edit of `baseline_hashes.rs`, the hash tables or any golden file, no new dependency, no unsafe code,
clean-room. The brief overrides rule 9: the F-570 entries of `docs/ROADMAP.md` and `docs/roadmap.json` are updated in a separate
last commit (the svgs are not regenerated); the proposed text is also in the addendum. Files of other agents not touched:
`cms.rs` (F-559), the J2 sphere test (F-561), `contact.rs` and its tests (F-599).

## What F-570 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. The spinning bar of ADR 0479 over ten turns, linear
> momentum within 1e-10, angular momentum within 1e-6, energy within 1e-6 relative.

Clauses: (c1) the spinning bar of ADR 0479 (D5 there, arithmetic 4: `[10, 2, 2]` Hex8, `L = 1`, `b = 0.02`, `E = 2e9`, `nu =
0.3`, `rho = 7850`, no constraints, steady spin `omega = 2 pi rad/s` about the axis `z` through `c = (L/2, 0, 0)` plus the
translation `V = (0.5, 0.25, 0.1) m/s`, `dt = 0.01 s`, 1000 steps = ten turns), run with the new step; (c2) linear momentum
`max_n |P_n - P_0| / |P_0| <= 1e-10`; (c3) angular momentum about the origin (the point of `Integrator::momentum`, ADR 0479
arithmetic 3) `max_n |J_n - J_0| / |J_0| <= 1e-6`; (c4) total energy `E = T + W` (kinetic `v^T M v / 2` plus strain energy of
`Integrator::strain_energy`) `max_n |E_n - E_0| / |E_0| <= 1e-6`. `n` runs over every step `1 .. 1000`. Every aggregated value
is asserted finite before it enters a maximum (an `f64::max` over `NaN` hides it).

## What was read (main d41012db, read only)

ADR 0479 (F-423: D1 the step, D2 the driver changes, D3 the integrator, D4 halving, D5 the models, arithmetic 3 the momentum of
the semi-discrete system, arithmetic 4 the spinning bar, amendment 1 the rounding floor of the residual, addendum 1 the known
miss S1c, S2c), ADR 0499 D3 (the uniform temperature in `NlTransientOptions`), ADR 0506 D3 (the F-bar Hex8 in
`nl_transient.rs`), `crates/rcfd-fem/src/nl_transient.rs` (whole), `nonlinear.rs` (`displacement_gradient`, `green_lagrange`,
`b_nonlinear`, `evaluate`, `point_update`, `check_model`, `force_and_tangent`, `NewtonOptions`, `Problem::attempt_with`, the
reference of the relative residual), `geometric.rs` (`element_geometric_stiffness`), `assemble.rs` (`assemble_with`,
`element_dofs`), `mass.rs` (head), `material.rs` (`Material::stress`, `state_size`), `constraint.rs` (`reduce`),
`rcfd-linalg/src/solvers.rs` (head), the tests `sa_nl_transient.rs` and `sa_nl_transient_default_bits.rs`, the F-570 entry of
`docs/ROADMAP.md` and `docs/roadmap.json`.

## The scheme and its conservation properties (first principles)

Published source, cited from memory and NOT read: J. C. Simo and N. Tarnow, "The discrete energy-momentum method. Conserving
algorithms for nonlinear elastodynamics", ZAMP 43 (1992) 757-792. As remembered: for the St. Venant-Kirchhoff material the
midpoint step whose internal force is `B(F_{n+1/2})^T S_alg` with `S_alg = (S_n + S_{n+1})/2` conserves the energy and the
linear and angular momenta exactly; for other hyperelastic materials `S_alg` needs a correction (a discrete gradient, O.
Gonzalez, J. Nonlinear Sci. 6 (1996) 449, also from memory, NOT read). Everything used below is derived here.

Notation: nodes `a`, reference positions `X_a`, displacements `u_a`, current positions `x_a = X_a + u_a`; at a quadrature point
of the stiffness rule the shape gradients `g_a` (reference), weight `w`; `H = sum_a u_a (x) g_a`, `F = I + H`, the Green-Lagrange
strain `E = (F^T F - I)/2`; St. Venant-Kirchhoff: `S = D E`, `D` constant and symmetric, `W(E) = E . D E / 2`. Step `n -> n+1` of
length `h`: `Delta u = u_{n+1} - u_n`, `H_m = (H_n + H_{n+1})/2`, `F_m = I + H_m`, `v_m = (v_n + v_{n+1})/2`.

**The step (D1).** Kinematics `Delta u / h = v_m`, i.e. `v_{n+1} = 2 Delta u / h - v_n`. Balance on the free equations

    R(u_{n+1}) = M (v_{n+1} - v_n)/h + C v_m + f_alg - f_m = 0,
    M (v_{n+1} - v_n)/h = (2/h^2) M (u_{n+1} - u_n - h v_n),   C v_m = C Delta u / h,
    f_alg,a = sum_q w F_m S_alg g_a  (in the Voigt form of the driver: sum_q w B_NL(F_m)^T S_alg),
    S_alg = (S(E_n) + S(E_{n+1}))/2 = D (E_n + E_{n+1})/2,   f_m = (f(t_n) + f(t_{n+1}))/2.

**Identity 1 (strain).** `E_{n+1} - E_n = ((F_m + DF/2)^T (F_m + DF/2) - (F_m - DF/2)^T (F_m - DF/2))/2 = sym(F_m^T DF)` with
`DF = H_{n+1} - H_n = sum_a Delta u_a (x) g_a`, exactly (the quadratic terms cancel). Hence `S_alg : (E_{n+1} - E_n) = S_alg :
F_m^T DF = sum_a Delta u_a . F_m S_alg g_a` for symmetric `S_alg`: point by point, `Delta u . f_alg = sum_q w S_alg : (E_{n+1} -
E_n)`.

**Energy (c4).** For SVK `W(E_{n+1}) - W(E_n) = (E_{n+1} + E_n) . D (E_{n+1} - E_n)/2 = S_alg . (E_{n+1} - E_n)` (D symmetric),
so `Delta u . f_alg = W_{n+1} - W_n` with `W = sum_q w W(E_q)` on the same rule (the quantity `Integrator::strain_energy` sums).
The kinetic part: `Delta u . M (v_{n+1} - v_n)/h = v_m . M (v_{n+1} - v_n) = T_{n+1} - T_n` (M symmetric). The dot product of
`R` with `Delta u` gives

    (T + W)_{n+1} - (T + W)_n = Delta u . f_m - h v_m . C v_m + Delta u . R(u_{n+1}),

exact in exact arithmetic: without load and damping the energy changes only by `Delta u . R`, the residual left by Newton (and
the rounding). With a constant load (gravity) `T + W - f . u` is conserved the same way; with damping the dissipation is `h v_m .
C v_m >= 0` per step.

**Linear momentum (c2).** `sum_a g_a = 0` (partition of unity), so `sum_a f_alg,a = F_m S_alg sum_a g_a = 0` for every `u`;
`P_{n+1} - P_n = sum_a (M (v_{n+1} - v_n))_a = -h sum_a (C v_m - f_m + R)_a`. Without load and damping (and `C` built from `M`
and `K_0`, both annihilating a translation) only `sum_a R_a` remains. The translational part of `R` is linear in `u_{n+1}`
(`t^T R = (2/h^2) t^T M (u - u_n - h v_n)`, `t` a uniform translation), and the Newton tangent of D2 satisfies `t^T K = (2/h^2)
t^T M` (its stiffness parts annihilate a translation), so one correction removes it: only rounding remains, as ADR 0479
arithmetic 3 (about `1e-14` over ten turns).

**Angular momentum (c3).** `J = sum_a x_a x p_a`, `p_a = (M v)_a`. Bilinearity gives exactly `J_{n+1} - J_n = sum_a (x_m,a x
Delta p_a + Delta x_a x p_m,a)` with `x_m = (x_n + x_{n+1})/2`, `p_m = M v_m`. `Delta x_a = Delta u_a = h v_m,a`, and `sum_a v_m,a
x (M v_m)_a = sum_ab M_ab v_m,a x v_m,b = 0` (the consistent mass is `M_ab I`, `M_ab` symmetric). `Delta p = -h (f_alg + R)`
(no load, no damping). `sum_a x_m,a x f_alg,a` is the axial vector of the antisymmetric part of `sum_a x_m,a (x) f_alg,a = sum_q w
(sum_a x_m,a (x) g_a) (F_m S_alg)^T = sum_q w F_m S_alg F_m^T`, because `sum_a x_m,a (x) g_a = sum_a X_a (x) g_a + H_m = I + H_m =
F_m` (isoparametric: `sum_a X_a (x) g_a = I`). `F_m S_alg F_m^T` is symmetric for every symmetric `S_alg`, so the moment
vanishes: `J_{n+1} - J_n = -h sum_a x_m,a x R_a`. The angular momentum is kept for any symmetric `S_alg`; the energy needs the
SVK average. That `B` is evaluated at `F_m` (not at `F_{n+1}`) is what makes `J` exact: with `F_{n+1}` the moment is `F_m S
F_{n+1}^T`, not symmetric.

**What the trapezoidal step lacks (for the negative control).** The trapezoidal rule of ADR 0479 balances `(f_int(u_n) +
f_int(u_{n+1}))/2`, whose work over the step is not `W_{n+1} - W_n` (identity 1 needs `F_m` and `S_alg` at one point): ADR 0479
addendum 1 recorded the exponential growth of the strain energy of the spinning bar from about step 350 (`7.917e-5 J` to `996 J`
at step 850) and `dJ/J = 3.9e-6` at step 600.

**Materials.** The derivation needs `S = D E` with `D` constant in time and symmetric, no history, and a solid isoparametric
element whose internal force is `sum_q w B_NL(F)^T S` (identity 1 and `sum_a X_a (x) g_a = I`). Registered: the step accepts
`Material::IsotropicElastic` on solid blocks of the `Full` formulation (Tet4, Tet10, Hex8, Hex20 of the driver) and refuses
every other material (orthotropic, tabulated, hyperelastic, viscoelastic, creep, J2) and the F-bar Hex8 (a BBar block, ADR 0506:
its internal force is not of this form) and the shells, with `FemError::InvalidValue { key: "nl_transient" }` whose reason names
ADR 0514. The orthotropic and the tabulated material at a uniform temperature are SVK materials too; they are refused here
because `strain_energy` and the point context do not carry the point position the cylindrical axes need (open question 2).
Hyperelastic materials need the discrete gradient (open question 3).

## Arithmetic before this registration (disclosed)

1. **Probe** (`probe_emc.py`, numpy, the spinning bar of c1 built independently: the same mesh, the 2x2x2 Gauss rule of the
   stiffness, the 3x3x3 rule of the consistent mass, the steady spin from the static solve with the three symmetry planes of
   ADR 0479 D5, then the step D1 with a dense solve; residual reference `max(|f_alg|, |M (v_{n+1} - v_n)/h|)`). The steady
   start: largest displacement `6.456e-6 m`, strain energy `7.917e-5 J` (the value of ADR 0479's record), `E_0 = 5.673722 J`,
   `P_0 = (1.57, 0.785, 0.314)`, `J_0 = (0, -0.157, 2.0373)`. Twenty steps, the exact (unsymmetric) tangent and the symmetric
   tangent of D2, at the residual tolerances `1e-4 .. 1e-8` (energy criterion off) and `1e-7` with the energy criterion
   `1e-10`: `dE <= 3.0e-13`, `dP <= 2.9e-15`, `dJ <= 1.4e-13`; residual tolerance `1e-9` with the energy criterion fails at
   step 5 to 7 on the rounding floor (`1.36e-9`, `1.58e-9`), the floor of ADR 0479 amendment 1. Ten turns (1000 steps), logs `probe_emc_run.log` (exact tangent) and `one_*.log` (symmetric tangent, `probe_one.py`):

   | tangent | residual tol. | energy tol. | `dE` | `dP` | `dJ` | iterations | largest final `r` |
   |---|---|---|---|---|---|---|---|
   | exact | `1e-2` | off | `4.920e-4` | `2.75e-13` | `2.174e-4` | 3000 | `4.58e-4` |
   | exact | `1e-4` | off | `1.508e-11` | `2.46e-13` | `6.37e-12` | 4000 | `9.90e-8` |
   | symmetric | `1e-2` | off | `1.041e-7` | `3.61e-13` | `2.91e-9` | 6563 | `9.68e-3` |
   | symmetric | `1e-3` | off | `9.44e-9` | `2.19e-13` | `4.17e-9` | 4677 | `1.00e-3` |
   | symmetric | `1e-4` | off | `1.07e-13` | `2.27e-13` | `4.26e-13` | 6000 | `5.31e-6` |
   | symmetric | `1e-5` | off | `1.07e-13` | `2.27e-13` | `4.26e-13` | 6000 | `5.31e-6` |
   | symmetric | `1e-6` | off | `1.07e-13` | `1.43e-13` | `5.09e-13` | 6178 | `9.96e-7` |
   | symmetric | `1e-7` | `1e-10` | `1.08e-13` | `2.45e-13` | `5.54e-13` | 8000 | `1.00e-7` |

   Every run converged every step (the probe allowed 40 iterations; the registered 20 is above the 8 a step measured). The
   symmetric tangent's linear convergence leaves a residual nearly orthogonal to `Delta u`: even at the residual tolerance
   `1e-2` the energy drift is `1e-7`. The exact tangent at `1e-2` stops after three quadratic iterations with a residual that
   does work (`dE = 4.9e-4`). The numpy arithmetic is not the Rust arithmetic (another summation order, a dense LU against the
   envelope `L D L^T`), so the limits are not tied to these digits.
2. **Rounding floor** (ADR 0479 amendment 1, confirmed by the probe): the relative residual of the total Lagrangian force under
   a rotation of order 1 stops at about `1e-9 .. 1.3e-8`; a residual tolerance of `1e-10` (the driver default) can not be met
   on the spinning bar. Registered Newton options of the bar (D6): residual tolerance `1e-7` (amendment 1 of ADR 0479, a decade
   above the extrapolated floor), energy tolerance `1e-10`, 20 iterations, 5 halvings.
3. **Energy error bound.** Per step `|Delta E| = |Delta u . R| <= |Delta u| |R|`; `|Delta u| ~ omega r h sqrt(N_nodes) ~ 0.3 m`,
   `|R| = r x reference ~ r x 10 N`: `|Delta E| / E_0 <= 0.5 r` per step, `500 r` over ten turns if every step added with one
   sign. At the floor `r ~ 1e-9` this bounds the drift by `5e-7`; the probe shows it far smaller (the residual of the converged
   iterate is nearly orthogonal to `Delta u`). Angular momentum per step `h |x| |R| / |J_0| ~ 0.01 x 0.5 x 10 r / 2 = 0.025 r`.
4. **Pendulum and linear limit.** For a linear model (`F = I` in `B`, `E` linear) the step D1 is `M (v_{n+1} - v_n)/h + C v_m +
   K (u_n + u_{n+1})/2 = f_m`, `Delta u/h = v_m`: the trapezoidal rule (Newmark `beta = 1/4`, `gamma = 1/2`) written at the
   midpoint; ADR 0479 L1's bar B at the load factor `1e-6` is then within the geometric nonlinearity `1.2e-14` of
   `transient::Integrator` (ADR 0479 arithmetic 1). The pendulum of ADR 0479 D5 (gravity, the pin) has `E = T + W - f . u`
   conserved (constant load: `Delta u . f_m = Delta (f . u)`), its period error the midpoint rule's `(omega h)^2/12` class, as
   the trapezoidal rule's (ADR 0479 P1c measured `2.09e-4`).

## Design

### D1. The step

As above. The unknown is `u_{n+1}` on the free equations (zero prescribed values, as ADR 0479), the start of the iteration
`u_n` (as the driver of ADR 0479). After convergence: `v_{n+1} = 2 (u_{n+1} - u_n)/h - v_n`; the state's acceleration is the
step's mean acceleration `(v_{n+1} - v_n)/h` (it is not used by the step; the start's acceleration is that of ADR 0479); the
state's internal force is `f_int(u_{n+1})` of the driver (`nonlinear::internal_force_at`, the meaning it has in ADR 0479).

### D2. The Newton iteration (`nl_transient/emc.rs`, a private child module of `nl_transient`)

Not the driver of ADR 0404 (its force is `f_int(u)` at the iterate, the step's is `f_alg(u_n, u)`): an iteration of its own with
the criteria of `NewtonOptions` read as the driver reads them. Iteration `k = 0, 1, ...`: the element forces `f_alg` and the
tangent; `R`; the relative residual `r = |R_F| / ref`, `ref = max(|f_m,F|, |f_alg|, |M (v_{n+1} - v_n)/h + C v_m|)` (the
driver's reference with the inertia in it, ADR 0479 D2); converged when `k >= 1`, `r <= residual_tolerance` and the energy of
the last correction `<= energy_tolerance` times the first; at `k = max_iterations` a failure. The correction solves `K du = -R`
on the free equations with the symmetric tangent

    K = (2/h^2) M + C/h + sum_q w [B_NL(F_m)^T (D/2) B_NL(F_m) + G^T (S_alg/2) G]

(`G^T S G` the geometric stiffness of `geometric::element_geometric_stiffness`), factored by the envelope `L D L^T` with the
pivot test of `nl_transient::factor`. The exact derivative has `B_NL(F_{n+1})` on the right of the material part; the symmetric
`K` differs by `O(|H_{n+1} - H_n|)` (the rotation per step, `0.063 rad` here): the iteration converges linearly at about that
rate, which the probe measured (6 to 8 iterations a step against 3 to 4 for the exact tangent). It annihilates a
translation (c2 argument). The assembly is `assemble::assemble_with` with the options' threads (bits independent of the count,
ADR 0404 D4); the element forces are added in element order. A point with `det F_{n+1} <= 0` or `det F_m <= 0` is refused with
the located `FemError::Element { InvalidValue { key: nonlinear::INVERSION_KEY } }` and a failed iteration returns
`FemError::Solver { solver: "newton" }`: both are retried by the halving of ADR 0479 D4 (unchanged; each sub-step is a step D1
of length `h = dt / 2^l`, and the conservation holds per sub-step).

### D3. The option (opt-in)

`nl_transient::StepKind { Method, EnergyMomentum }` (`#[non_exhaustive]`, default `Method`); `NlTransientOptions::step:
StepKind` and the builder `NlTransientOptions::energy_momentum()`. `Method` is the step of ADR 0479, unchanged in every bit.
With `EnergyMomentum` the method's parameters are not used by the step (they are still recorded in the state, so a restore keeps
its checks). `Integrator::step_kind()`.

### D4. Refusals of the new step (key `"nl_transient"`, the reason names ADR 0514)

`Integrator::new` with `EnergyMomentum`: a material other than `IsotropicElastic`; a BBar or Incompatible solid block, a shell or
beam block. `run_with` / `step_with` with a non-empty contribution list (before any step; the state unchanged): a contribution
has no midpoint form here (follower loads and contact in this step are not offered). Damping (`C v_m`), load histories (`f_m`)
and the uniform temperature (passed to the point context; the isotropic material does not read it) are accepted.

### D5. The state

`NlTransientState` gains a private field `kind`. `to_bytes` of a state of the `Method` step is unchanged (magic `RCFDNLT1`); a
state of the `EnergyMomentum` step has the magic `RCFDNLE1` and the same layout. `from_bytes` accepts both and sets the kind;
`restore` refuses a state of the other kind (key `"state"`).

### D6. The models and runs

- Spinning bar (S0e to S3e, N1e, T1e): ADR 0479 D5 and arithmetic 4 with the `Centrifugal` contribution of `sa_nl_transient.rs`
  copied for the steady start (the steady solve keeps the driver defaults, as S0c there); `NlTransientOptions::new(0.01)
  .energy_momentum()` with the Newton options of arithmetic 2 (residual `1e-7`, energy `1e-10`).
- Pendulum (P1e, P2e, D1e): ADR 0479 D5, `dt = 0.005`, 400 steps (one period `T = 1.9333 s` is 387 steps), the Newton options
  of arithmetic 2.
- Bar B (L1e): ADR 0479 D5 bar B with the A5 load times `1e-6`, 80 steps of `T_1/40`, driver defaults; against
  `transient::Integrator` Newmark average acceleration, undamped and with the damping `Rayleigh::from_ratio(0.02, omega,
  omega)`.
- Save and restore (R1e, R2e): the spinning bar, 60 steps against 30 + `to_bytes` + `from_bytes` + `restore` into a new
  integrator + 30.

### D7. Files

`crates/rcfd-fem/src/nl_transient.rs` (the option, the state kind and magic, the refusals, the dispatch of `substep`),
`crates/rcfd-fem/src/nl_transient/emc.rs` (new: D1, D2), `crates/rcfd-fem/tests/sa_emc.rs` (the checks),
`crates/rcfd-fem/tests/sa_emc_default_bits.rs` (B0). No other library file. `lib.rs` is unchanged (the child module).

### D8. The bit record of the default (B0)

`crates/rcfd-fem/tests/sa_emc_default_bits.rs`, written after this commit and run on the unchanged base d41012db before any
library line; its digests (FNV-1a 64 over the bits) are written into the file then. Content, every path of `nl_transient.rs`
that D3 to D5 touch: the pendulum, Newmark average acceleration, 30 steps (state bytes after each 10); generalised-alpha
`rho_inf = 0.8` with Rayleigh damping on the pendulum, 30 steps; the spinning bar of D6 with Newmark, 20 steps; a split run
(15 + bytes + restore + 15) of the pendulum (the bytes); `kinetic_energy`, `strain_energy`, `momentum` at the end of each.
`sa_nl_transient_default_bits.rs` and `sa_nl_transient.rs` stay as they are and must pass.

## Checks (limits fixed now)

| Id | Clause | What | Limit | Estimate behind the limit |
|---|---|---|---|---|
| S0e | c1 (set-up) | steady spin solve converges; 1000 steps of the step D1 converge (`Ok`), all measures finite | `Ok` | probe: 1000 steps converged |
| S1e | c2 | spinning bar, `max_n |P_n - P_0| / |P_0|`, `n = 1 .. 1000` | `<= 1e-10` | arithmetic 1: `2.5e-13`; c2 argument: rounding `~1e-14` |
| S2e | c3 | `max_n |J_n - J_0| / |J_0|` about the origin | `<= 1e-6` | arithmetic 1: `5.5e-13`; arithmetic 3 |
| S3e | c4 | `max_n |E_n - E_0| / |E_0|`, `E = T + W` | `<= 1e-6` | arithmetic 1: `1.1e-13`; arithmetic 3 |
| TP | brief (record) | the tolerance probe: the spinning bar at residual tolerances `1e-1, 1e-2, 1e-3, 1e-4, 1e-5, 1e-6` with the energy tolerance `1` (the residual criterion alone), each the three measures of S1e to S3e and the largest final residual | record, no limit; which tolerances meet all three | arithmetic 1: the symmetric tangent meets all three down to `1e-2` (`dE = 1.0e-7`); `1e-1` not probed |
| N1e | negative control | the same bar with the `Method` step (Newmark, residual `1e-7`), 600 steps: `max_n |E_n - E_0| / |E_0|` | `>= 1e-6` | ADR 0479 addendum 1: strain energy growing e-fold per ~30 steps from step 350 |
| N2e | negative control | the same run: `max_n |J_n - J_0| / |J_0|` | `>= 1e-6` | ADR 0479 addendum 1: `3.9e-6` at step 600 |
| L1e | linear limit | bar B, A5 load times `1e-6`, against `transient::Integrator` Newmark average acceleration, undamped and damped: `max |du|_inf / max |u_lin|_inf` | `<= 1e-9` each | arithmetic 4: `1.2e-14` |
| P1e | pendulum | period from the crossings of `psi = 0` in 400 steps, against `T = 1.933334854373 s` | `<= 1e-2` | arithmetic 4: ADR 0479 P1c `2.09e-4` |
| P2e | pendulum | `max_n |E_n - E_0| / (m g L/2)`, `E = T + W - f . u` | `<= 1e-6` | energy identity; at the floor `r ~ 1e-8` |
| D1e | damping | the pendulum with `Rayleigh { mass: 0.5, stiffness: 0 }`: `max_n |E_n + D_n - E_0| / (m g L/2)`, `D_n = sum_k h v_m,k . (0.5 M) v_m,k` from the velocities | `<= 1e-6` | energy identity with dissipation |
| D2e | damping (set-up) | the same: `D_400 / (m g L/2)` | `>= 1e-2` | `0.5 s^-1` over `2 s`: most of the swing |
| R1e | state | spinning bar, 60 steps against 30 + bytes + restore + 30: equal state bytes | exact | D5 |
| R2e | state | `from_bytes(to_bytes(s)).to_bytes() == to_bytes(s)` for an `EnergyMomentum` state; its magic is `RCFDNLE1`; restoring it into a `Method` integrator and a `Method` state into an `EnergyMomentum` integrator refused (`"state"`) | exact; all | D5 |
| T1e | threads | spinning bar, 50 steps with 1 and 4 threads: equal state bytes | exact | D2 |
| H1e | halving | spinning bar with `max_iterations = 1`: the first step returns `FemError::Solver { solver: "nl_transient" }` with `after 5 halvings`; the state bytes unchanged | holds | ADR 0479 D4 |
| F1e | refusals | `Integrator::new` with `EnergyMomentum` refuses a J2 material, a NeoHookean material, an orthotropic material and a BBar Hex8 block (key `"nl_transient"`, reason contains `ADR 0514`); `step_with` with one contribution refused (`"nl_transient"`) with the state unchanged | all | D4 |
| B1 | defaults | B0: every digest of base d41012db; `sa_nl_transient_default_bits.rs` and `sa_nl_transient.rs` pass | exact; pass | D8 |
| B2 | defaults | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | no output of a case changed |

A check that would take more than about two minutes in release is an ignored heavy record run once, with a fast test on the same
code path (TP is expected to be one).

## Negative controls and mutations

Negative controls N1e, N2e (the trapezoidal step on the same bar) and D2e (the dissipation is seen). Planned mutations, on a
scratch copy, one at a time:

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | `S_alg = S(E_{n+1})` (no average) | S3e, P2e |
| MU2 | `B_NL` at `F_{n+1}` in the force (not `F_m`) | S2e, S3e |
| MU3 | `v_{n+1} = (u_{n+1} - u_n)/h` | S1e, S3e, L1e |
| MU4 | `f_m = f(t_{n+1})` (no average) | L1e |
| MU5 | the damping term `C Delta u/h` left out of the residual | L1e (damped), D1e |
| MU6 | `restore` without the kind test | R2e |
| MU7 | the material refusal removed | F1e |
| MU8 | the geometric part of the tangent left out | S0e (the iteration slows; caught if the bar fails) or none: recorded as a probe of the tangent, not required |

## Decision rule

- **done**: every check of the table met (TP is a record), MU1 to MU7 caught by at least one check, fmt and clippy clean, B1
  and B2 unchanged.
- **partial**: B1 and B2 unchanged, S1e and S2e met, and a miss of S3e or of a companion check diagnosed and recorded as a
  `*_known_miss` test, or a mutation of MU1 to MU7 uncaught.
- **todo**: otherwise (in particular any change of the default bits).

## Limits (not covered)

- Isotropic St. Venant-Kirchhoff only; no discrete gradient for hyperelastic materials, no orthotropic or tabulated material, no
  F-bar Hex8, no shells or beams, no contributions (follower loads, contact) in this step.
- The symmetric tangent converges linearly at the rotation per step; large rotations per step are slow (halving rescues).
- The rounding floor of the total Lagrangian force (ADR 0479 amendment 1, item F-571) sets the residual tolerance of the bar;
  the driver defaults (`1e-10`) cannot converge it.
- One mesh, one step size; no convergence study; no case-file key (open question 1).

## Open questions (taken on the maintainer's standing instruction of 2026-11-11, take the recommendation; none changes a default)

1. **Case-file key.** Not added (brief). Recommendation: `[study] kind = "transient"`, `nonlinear = true` (ADR 0479 open
   question 1) with `step = "energy_momentum"` (default `"method"`), refused unless `nonlinear = true`.
2. **Orthotropic and tabulated SVK materials.** Recommendation: a follow-up that carries the point position in the point context
   of the step and of `strain_energy`, then accepts them (the derivation holds for any constant symmetric `D`).
3. **Hyperelastic materials.** Recommendation: a follow-up with the discrete gradient `S_alg = S(E_m) + [(W_{n+1} - W_n - S(E_m)
   . dE) / (dE . dE)] dE` (Gonzalez, from memory), checked on a NeoHookean spinning bar.

## Amendment 1 (2026-11-14, written after the registered run of commit 100d0d05 and before any corrected run)

The registered run (`cargo test -p rcfd-fem --release --test sa_emc`, log `f570-scratch/run1.log`) met every check of the
table except two clauses of F1e; nothing else was rerun.

1. **F1e, NeoHookean and orthotropic: set-up defective.** Both models are refused by `Integrator::new`, but before the check of
   D4: `SolidModel::check_supported_nonlinear` (ADR 0479 D3, unchanged) refuses them with `FemError::NotYetSupported`
   ("material Hyperelastic ... is added by ADR 0328 (F-425)", "material Orthotropic ... is added by a follow-up of ADR 0452"),
   so the expected key `"nl_transient"` with `ADR 0514` cannot appear: the registration assumed the nonlinear transient
   integrator takes those materials, which it does not. The materials are refused, which is what D4 asks. **Corrected set-up
   (F1ec):** the same models and calls; the J2 material, the BBar block and the contribution keep the registered expectation
   (key `"nl_transient"`, reason with `ADR 0514`); the NeoHookean and the orthotropic material are required refused by
   `Integrator::new` with the energy-momentum step (any error), and so is the Prony viscoelastic material
   (`Material::viscoelastic` on the isotropic base with one term, a material that the integrator takes with the `Method` step)
   with the key `"nl_transient"` and `ADR 0514` (the check of D4 reached by a second material). Limits unchanged in kind
   (refused; all). Run once. The registered test stays as an ignored record.
2. **TP** took about two minutes of the run (six runs of ten turns): it becomes an ignored heavy record with the numbers of the
   registered run in its reason; S0e to S3e cover the same code path.

## Amendment 2 (2026-11-14, written after the mutation runs and before the run of the added check)

Mutation MU4 (`f_m = f(t_{n+1})`) was caught by no check (`f570-scratch/mutations.log`): the only loaded models (bar B of
L1e, the pendulum) have loads constant in time, for which the average of the end values is the end value. **L2e** (added,
limit fixed now): bar B of L1e (the same mesh, mode, step `T_1/40`, 80 steps, undamped) with the A5 pattern times `1e-6` on the
ramp table `(0, 0), (10 T_1, 1)`, against `transient::Integrator` Newmark average acceleration: `max |du|_inf / max |u_lin|_inf
<= 1e-9` (arithmetic 4: the trapezoidal rule written at the midpoint takes the average of the end loads, so the two agree to
the geometric nonlinearity `1.2e-14`; with MU4 the load leads by half a step, an error of order `h dF/dt` against `F`, about
`1e-1` here). Run once, then MU4 again against it. MU8 (the geometric part of the tangent) was not caught, as the registration
allowed: the iteration converges without it at this rotation per step (a slower but still convergent tangent); it is not in the
decision rule.

## Addendum 1: results (2026-11-14)

Runs: registered (commit `100d0d05`, `f570-scratch/run1.log`), amendment 1 (commit `601ffed5`, `run2.log`, F1ec only),
amendment 2 (commit `438afe4b`, `run3.log`, L2e only). Each set-up ran once. B0 recorded on the base d41012db (commit
`439b0264`, `b0-base.log`) before any library line.

| Id | Measured | Limit | Result |
|---|---|---|---|
| S0e | steady solve converged; 1000 of 1000 steps, no halving, largest final residual `9.98e-8` | `Ok` | met |
| S1e | `1.812e-13` | `<= 1e-10` | met |
| S2e | `6.623e-13` | `<= 1e-6` | met |
| S3e | `1.584e-13` | `<= 1e-6` | met |
| TP (record) | residual tolerance, energy criterion off (`dP`, `dJ`, `dE`): `1e-1`: `2.96e-13`, `4.51e-8`, `1.089e-6` (energy limit not met); `1e-2`: `2.23e-13`, `3.82e-9`, `1.04e-7`; `1e-3`: `2.21e-13`, `4.16e-9`, `9.40e-9`; `1e-4` and `1e-5`: `2.26e-13`, `6.86e-13`, `1.01e-13`; `1e-6`: `2.45e-13`, `4.93e-13`, `1.37e-13`; all 1000 steps, no halving | record | the loosest tolerance meeting all three is `1e-2`; ignored heavy record (amendment 1) |
| N1e | `8.800e-6` (Newmark, 600 steps) | `>= 1e-6` | met |
| N2e | `3.897e-6` | `>= 1e-6` | met |
| L1e | undamped `7.64e-14`, damped `6.30e-14` | `<= 1e-9` | met |
| L2e | `2.40e-14` | `<= 1e-9` | met (amendment 2) |
| P1e | 2 crossings, `T_FE = 1.933740 s`, `2.096e-4` | `<= 1e-2` | met |
| P2e | `1.23e-14` (largest final residual `9.88e-8`) | `<= 1e-6` | met |
| D1e | `1.70e-14` | `<= 1e-6` | met |
| D2e | `0.586` | `>= 1e-2` | met |
| R1e | equal state bytes | exact | met |
| R2e | round trip, magic `RCFDNLE1`, kind read back, both cross-kind restores refused (`"state"`) | exact; all | met |
| T1e | equal state bytes | exact | met |
| H1e | `FemError::Solver { solver: "nl_transient" }`, "after 5 halvings", state unchanged | holds | met |
| F1e | J2, BBar, contribution refused with `"nl_transient"` and `ADR 0514`; NeoHookean and orthotropic refused upstream with `NotYetSupported` | all with the key | set-up defective (amendment 1), ignored record |
| F1ec | J2 and viscoelastic with `"nl_transient"` and `ADR 0514`; NeoHookean, orthotropic refused; BBar and contribution with the key; state unchanged | all | met |
| B1 | the four digests of d41012db; `sa_nl_transient_default_bits.rs`, `sa_nl_transient.rs` pass | exact; pass | met |
| B2 | `baseline_hashes`: 3 passed, 1 ignored | pass | met |

Also: `cargo test -p rcfd-fem --release` exit 0 (commit `601ffed5`; `sa_emc` 9 passed, 2 ignored); `cargo test -p rcfd-cli
--release --test docs_site` exit 0; `cargo fmt -p rcfd-fem --check` and `cargo clippy -p rcfd-fem --release --all-targets -- -D
warnings` clean (on the tree with L2e).

**Mutations** (`f570-scratch/mutations.log`, `mut_MU4_l2e.log`, a git-archive copy with its own target): MU1 caught (S0e: the
first step does not converge; L1e, P1e, P2e, D1e, R1e, T1e); MU2 (S0e stops at step 593; S2e `0.36`, S3e `0.64`, P2e, D1e); MU3
(L1e, S0e, S2e, S3e, P1e, P2e, D1e); MU4 caught by no registered check (constant loads only) and by L2e of amendment 2
(`1.25e-2`); MU5 (L1e damped, D1e); MU6 (R2e both refusals); MU7 (F1ec J2 and viscoelastic accepted); MU8 not caught (allowed,
not in the rule).

**Deviations.** Library and tests in one commit (`100d0d05`); F1e's expectation for two materials was wrong (amendment 1); TP
made an ignored record after its run (amendment 1, about two minutes); L2e added after the mutation runs (amendment 2), before
its run. The conservation is at rounding (`1e-13`) with the residual tolerance `1e-7`: the symmetric tangent's residual does
almost no work on `Delta u`, so the limits of the acceptance are three to six decades above the measures.

**Decision: done** by the registered rule: every check met (F1e by its corrected F1ec), MU1 to MU7 caught (MU4 only by the check
added in amendment 2), fmt and clippy clean, B1 and B2 unchanged. If the lead reads the rule as the registered checks only,
MU4 makes it partial.

Remaining questions (each with options and a recommendation; none changes a default):

1. Case-file key: options (a) `step = "energy_momentum"` under the nonlinear transient study, (b) a separate study kind.
   Recommendation (a), as in the open questions above.
2. Orthotropic and tabulated SVK materials: they are refused today by `check_supported_nonlinear` for the whole nonlinear
   transient integrator, not only by this step. Recommendation: accept them here once the nonlinear driver takes them.
3. Hyperelastic materials: recommendation, the discrete-gradient follow-up below.
4. The exact (unsymmetric) tangent: the probe shows it converges in fewer iterations but, stopped at a loose tolerance, leaves a
   residual that does work (`dE = 4.9e-4` at `1e-2`); recommendation: keep the symmetric tangent.

## For the lead: roadmap entries

Proposed status of F-570: done.

> Structural solver, advanced tier. ADR 0514, 2026-11-14: nl_transient gains StepKind::EnergyMomentum (NlTransientOptions::energy_momentum), the Simo-Tarnow midpoint step with the internal force B_NL(F_mid)^T (S_n + S_{n+1})/2 and the velocity 2 du/h - v_n, solved by Newton with a symmetric tangent; isotropic St. Venant-Kirchhoff solids of the Full formulation only (other materials, F-bar blocks and driver contributions refused naming ADR 0514); loads averaged over the step, Rayleigh damping, halving, save/restore (state magic RCFDNLE1) and threads as the Method step. Spinning bar of ADR 0479 over ten turns (residual tolerance 1e-7): linear momentum 1.8e-13 (1e-10), angular momentum 6.6e-13 (1e-6), energy 1.6e-13 (1e-6); the trapezoidal step on the same bar drifts 8.8e-6 in energy by step 600. Tolerance probe: the three limits hold down to the residual tolerance 1e-2. Linear limit equals F-398 within 7.6e-14, pendulum energy within 1.2e-14, damped energy balance within 1.7e-14. Default step bit-identical (B0, baseline_hashes). Seven of eight mutations caught (one by a check added after the mutation run). No case-file key.

> Follow-up: energy-momentum step for hyperelastic materials (the discrete gradient of Gonzalez). Deps F-570, F-425. Acceptance: Every limit not stated here is registered in the ADR before the run. A NeoHookean spinning bar (the bar of ADR 0479 with a NeoHookean material of the same small-strain moduli) over ten turns, linear momentum within 1e-10, angular momentum within 1e-6, energy within 1e-6 relative; on an isotropic elastic model the step equals the F-570 step within 1e-12.

> Follow-up: case-file key for the nonlinear transient study and its energy-momentum step (step = "energy_momentum" under a nonlinear transient study). Deps F-570. Acceptance: Every limit not stated here is registered in the ADR before the run. A case with the key reproduces the bits of the API run of the spinning bar of ADR 0514 over 50 steps; a case without the key keeps its output hashes.
