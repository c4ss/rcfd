# ADR 0489: F-411 registration: power-law creep with time hardening, a backward Euler radial return with its consistent tangent, on a minimal quasi-static time-stepping driver over the Newton driver

Status: **REGISTERED 2026-11-14, written before any code of F-411 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-411 was written, compiled or run in the repository: the files listed under "What was
read" were read, and the arithmetic below was done by hand and evaluated with plain numpy prototypes in the scratch folder
(`w49i-work/f411-scratch/probe_point.py`, `probe_extra.py`, `probe_cylinder.py`, `probe_bar.py` and their `.log` files; no rcfd
code; disclosed, their numbers quoted below). The bit record B0 of the default paths is a test file written and run on the base
after this commit and before any library code (D9). Results go in the addenda at the end; nothing above the addenda is changed
after a run.

Registration of the stub [ADR 0314](0314-creep-power-law-implicit.md) (work package SA-4 of the plan
[ADR 0310](0310-structural-solver-advanced-tier-plan.md)). Roadmap item F-411 (deps F-408, F-397). Number 0489 given by the
lead. Branch `f411-work`, worktree `w49i-work/f411`, scratch `w49i-work/f411-scratch`.

**The base.** The branch starts at 9cf92ae6 (main, with F-422, F-423, F-424 and F-410). That commit does not compile
`rcfd-fem`: the arc-length `Problem { .. }` initializer of F-424 (`nonlinear::controlled_attempt`) lacks the `context` field that
F-410 added. On the lead's instruction the one-line fix of main (`context: PointContext::default()`, with the comment "ADR 0485
D3: the arc-length step of ADR 0480 is static, at the default point context") is commit 81960c8d, before this registration. The
base of every bit record below is 81960c8d.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code, the crate graph
unchanged; no existing test edited except by the records-and-copies scheme of ADR 0291 (`sa_scaffold.rs`, D10); clean-room.

## What F-411 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. The tangent agrees with central differences within 1e-6. Uniaxial constant stress: creep strain A sigma^n t within 1e-8 for m = 0 and A sigma^n t^(m+1)/(m+1) within 0.1 percent for m = -0.5. Relaxation at fixed strain, n = 3 and 5: sigma(t) = (sigma0^(1-n) + (n - 1) E A t)^(1/(1-n)) within 0.5 percent at the registered step count, observed order >= 0.9. Thick cylinder b/a = 2 in plane strain under internal pressure, n = 4: the stress starts at Lame within 1 percent and tends to the stationary field sigma_r = -p ((b/r)^(2/n) - 1)/((b/a)^(2/n) - 1), sigma_theta = p ((2 - n)/n (b/r)^(2/n) + 1)/((b/a)^(2/n) - 1) within 2 percent after the registered redistribution time. Radial bar clamped on the axis of rotation, n = 3: the tip creeps at the rate A times the integral of (rho Omega^2 (L^2 - r^2)/2)^n over the length within 0.5 percent (the stress is statically determinate). A step whose creep-strain increment exceeds the registered limit is halved; a non-converged local iteration is an error with the element.

The brief adds: (b1) reuse the integration-point history of F-422 and the time-aware path of F-410 (`solve_from_at`, the point
context with time); a quasi-static time-stepping driver (F-410's proposed follow-up) is either built here, minimal and with its
scope registered, or the tests loop over `solve_from_at`; not two drivers; (b2) the plane-strain cylinder by the slice approach
of ADR 0475 or a registered one; (b3) models without the new material keep their bits, `baseline_hashes` unchanged, a
default-bits test at the base before any library change; (b4) probe a quantity before a `1e-12`-class limit, `1e-11` for sums of
about `1e3` terms; (b5) the records-and-copies scheme for `sa_scaffold.rs` and the other `*_default_bits.rs` files that record a
refusal of the pending creep variant; (b6) the fan blade under centrifugal load: the radial bar clause matters.

Clauses: (a0) every limit registered here; (a1) the tangent against central differences, `1e-6`; (a2) uniaxial constant stress,
`m = 0`, `1e-8`; (a3) the same, `m = -0.5`, `1e-3`; (a4) relaxation at fixed strain, `n = 3` and `5`, `5e-3` at the registered
step count, observed order `>= 0.9`; (a5) the thick cylinder: Lame at the start within `1e-2`, the stationary field within
`2e-2` after the registered redistribution time; (a6) the rotating radial bar, the tip creep rate within `5e-3`; (a7) a step
whose creep-strain increment exceeds the registered limit is halved; (a8) a non-converged local iteration is an error with the
element.

Readings, registered:

- **The law.** The data model of ADR 0440 D2 is `Material::Creep { base, a, n, m }`, "`d eps_c/dt = a sigma^n t^m`". Read as
  the von Mises (Prandtl-Reuss) generalisation: the creep strain rate is deviatoric and normal to the von Mises surface,
  `d eps_c/dt = (3/2) A q^(n-1) t^m s`, `s` the stress deviator, `q = sqrt(3/2 s:s)` the equivalent stress; the equivalent
  creep strain rate is `A q^n t^m`, and in uniaxial stress `d eps_c,xx/dt = A sigma^n t^m` (the acceptance's form). `t` is the
  time of the analysis, counted from `t = 0`, the instant of loading (time hardening). No temperature factor: the data model
  has none (Limits; follow-up).
- **"Creep-strain increment"** (a7) is the largest increment of the equivalent creep strain (`Delta eps_bar = int A q^n t^m
  dt`, the seventh state value, D1) over the integration points of the model in one step.
- **"Uniaxial constant stress"** (a2, a3) at the material point under stress control (U1, U2) and, as companions, one Hex8
  element under a dead load through the driver (U1e, U2e).
- **"The stress tends to the stationary field"** (a5) is read on the element means of the polar stresses (C2), because the
  fully integrated Hex20 locks under the isochoric creep flow and its point pressure oscillates inside the element with a
  zero element mean (arithmetic 6). The point-wise values are reported, not checked; the point-wise deviatoric part `sigma_theta
  - sigma_r`, free of the pressure, is checked point by point (C2d).
- **"Clamped on the axis of rotation"** (a6): the root section at `r = 0` held axially, with symmetry planes through the axis of
  the bar (no lateral clamp, so that the root is in uniaxial stress like the rest of the bar; a full clamp would restrain the
  Poisson contraction where the stress, and the creep rate, is largest).

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0314 (stub), ADR 0310 (the SA-4 entry, sections 2.3 and 2.4), ADR 0440 (D2, D3: the creep
  state of 7 values), ADR 0475 (whole: the J2 radial return, D3 the history in the driver, D5 the sphere slice and its diagnosis
  of the point scatter), ADR 0485 (whole: the time on the driver, D4 `solve_from_at`, its proposed follow-up 1, the time
  stepping driver).
- `crates/rcfd-fem/src/nonlinear.rs` (whole), `material.rs` (`PointContext`, `stress`, `elasticity_matrix`, `state_size`),
  `model.rs` (`Material`, `creep`, `pending`, `check_elements`), `mat_creep.rs`, `mat_j2.rs`, `mat_visco.rs`, `hex20.rs`
  (`upgrade_to_hex20_mapped`), `builders.rs` (`hex_box`, its sets), `mesh.rs` (`SolidFormulation`: `BBar` and `Incompatible`
  still pending, ADR 0293), `hex8x.rs`.
- Tests: `sa_scaffold.rs` (`pending_variants`, `m1_to_m4_after_f410`,
  `x1_modules_and_pending_after_f415_f420_f422_f416_f417_f432_f423_f424_f410`), `sa_j2.rs` (sphere helpers), `sa_visco.rs`,
  `sa_visco_default_bits.rs`. A search of `crates/` for `Creep`/`creep` in tests and in `rcfd-api`, `rcfd-cli`: only
  `sa_scaffold.rs` (and the word in `sa_visco.rs`, the viscoelastic creep check) refer to the creep variant; no
  `*_default_bits.rs` file records its refusal.

## Arithmetic done before the code (disclosed)

1. **The return map** (first principles; the backward Euler integration of a von Mises-type rate law with the radial return is
   the scheme of the J2 return of ADR 0475 with the yield condition replaced by the rate law, as in the creep and viscoplastic
   returns of J. C. Simo and T. J. R. Hughes, Computational Inelasticity, Springer, 1998, and of the creep literature, cited
   from memory, NOT re-read; derived here). Isotropic base `G = E/(2(1 + nu))`, `K = E/(3(1 - 2 nu))`, `D` its elasticity. State
   `[eps_c (6, engineering shear), eps_bar]` (ADR 0440 D3, 7 values). Over a step `[t0, t1]`, `dt = t1 - t0`, with the stress
   held at its end value (backward Euler in the stress):
   * trial `sigma_tr = D (eps - eps_c^n)`, `s_tr` its deviator, `q_tr = sqrt(3/2 s_tr:s_tr)`;
   * time factor `tau = int_{t0}^{t1} t^m dt = (t1^(m+1) - t0^(m+1))/(m + 1)` (`= dt` for `m = 0`, taken as `dt` exactly; finite
     for `-1 < m < 0` from `t0 = 0`): the time exponent is integrated exactly, so a constant stress gives the closed form at any
     step size;
   * the deviator stays radial, `s = (q/q_tr) s_tr`, and `Delta eps_bar = (q_tr - q)/(3 G)` with the scalar equation `g(q) =
     q_tr - q - 3 G A tau q^n = 0` on `(0, q_tr]`;
   * `eps_c += Delta eps_bar (3/2) s_tr/q_tr` (shear components doubled: engineering), `eps_bar += Delta eps_bar`, `sigma =
     sigma_tr - 3 G Delta eps_bar s_tr/q_tr`.
   For `tau = 0` (no time increment) or `q_tr = 0` the step is elastic: `sigma = sigma_tr`, tangent `D`, state copied.
   **Local iteration.** `g(q_tr) = -3 G A tau q_tr^n < 0`, `g(0+) = q_tr > 0`, `g' = -1 - 3 G A tau n q^(n-1) < 0`, `g'' = -3 G A
   tau n (n - 1) q^(n-2) <= 0` for `n >= 1`: decreasing and concave, so Newton from `q_0 = q_tr` decreases monotonically to the
   unique root (the tangent of a concave function lies above it, so each iterate stays at or right of the root). Stop when
   `|dq| <= 1e-14 q_tr`; at most `MAX_LOCAL_ITERATIONS = 50` iterations, else an error (key `"creep iteration"`, with the
   iterations and `q/q_tr`; D1). For `n < 1` the function is convex and Newton may leave the interval: refused at the update
   (key `"n"`; Limits).
   **Consistent tangent.** `theta = q/q_tr`, `beta = dq/dq_tr = 1/(1 + 3 G A tau n q^(n-1))`, `N = s_tr/|s_tr|` (the tensor
   norm with the shear weights 2): `C = K 1 (x) 1 + 2 G theta I_dev + 2 G (beta - theta) N (x) N` (in the Voigt form of ADR 0475
   D1: `I_dev` with 1/2 on the shear diagonal, strain with engineering shear). Derivation: `d sigma = K 1 tr d eps + theta 2 G
   I_dev d eps + s_tr d theta`, `d theta = (beta - theta) d q_tr / q_tr`, `d q_tr = 3 G (s_tr/q_tr) : d eps`, and `3 G s_tr (x)
   s_tr / q_tr^2 = 2 G N (x) N`. Symmetric.
   Probe (`probe_point.py`, `probe_point.log`): five cases (`n = 3, 4, 5, 3, 1.5`, `m = 0, -0.5, 0.3, 0, 0`), random strains
   `+-1e-3` and committed creep strains `+-2e-4` (seed `0xF411`), `A = 1e-26`, `t1 = 2 s`, `dt = 0.5 s`, central differences of
   step `1e-10`: `max |C - C_fd| / max |C|` at most `1.33e-9`; 2 to 22 local iterations.
2. **Uniaxial constant stress** (first principles). With the stress held, the update is exact for any step: `eps_c,xx(t) = A
   sigma^n int_0^t s^m ds = A sigma^n t^(m+1)/(m+1)`. Probe at the material point under stress control (`sigma = 2e6 Pa`, `A =
   1e-25`, `n = 3`, 100 steps of `0.1 s`): largest relative error over the step ends `5.8e-13` (`m = 0`), `2.0e-13` (`m = -0.5`).
   The limits `1e-8` and `1e-3` are the acceptance's; the probe shows four decades of margin at `m = 0`.
3. **Relaxation at fixed strain** (first principles). Uniaxial stress, fixed axial strain: `eps = sigma/E + eps_c` constant, so
   `d sigma/dt = -E A sigma^n` and `sigma(t) = (sigma0^(1-n) + (n - 1) E A t)^(1/(1-n))` (the acceptance's form; `m = 0`).
   Backward Euler in the stress is first order. Set-up: `E = 2e9 Pa`, `nu = 0.3`, `eps0 = 1e-3` (`sigma0 = 2e6 Pa`), `A = 1/((n
   - 1) E sigma0^(n-1))` (the time scale `t* = 1/((n - 1) E A sigma0^(n-1)) = 1 s`), uniform steps to `t = 10 s` (`sigma(10 s) =
   sigma0/sqrt(11)` for `n = 3`). Probe (`probe_point.log`, `probe_extra.log`), largest relative error over the step ends:
   | steps | `n = 3` | `n = 5` |
   |---|---|---|
   | 100 | `1.337e-2` | `5.573e-3` |
   | 200 | `6.787e-3` | `2.829e-3` |
   | 400 | `3.421e-3` | `1.426e-3` |
   | 800 | `1.717e-3` | `7.156e-4` |
   Registered step count **800** (`dt = 0.0125 s`), the order from 400 to 800: probe `0.9941` (`n = 3`), `0.9943` (`n = 5`).
   On the total Lagrangian driver with the axial displacement held, the Green-Lagrange strain `E_xx` is held, the lateral faces
   are free and the state is homogeneous, so the law of the point acts on `E_xx` and `S_xx` exactly (the lateral strains move,
   `S_yy = S_zz = 0`; as ADR 0485 arithmetic 5): the check compares `S_xx(t)` with the closed form for `sigma0 = S_xx(0)`, the
   stress of the instantaneous step.
4. **The halving** (a7). The one-element relaxation of item 3 (`n = 3`, `t* = 1 s`) in one step `[0, 1 s]`: the equivalent creep
   increment of the step is `2.291e-4` at `dt = 1`, `1.523e-4` at `0.5`, `9.321e-5` at `0.25`, and the sub-steps of `0.25` to `1
   s` give `9.32e-5, 7.26e-5, 5.84e-5, 4.82e-5` (probe `probe_extra.log`). With the limit `1e-4` the step is halved twice and
   finished in four sub-steps of `0.25 s` (the halved length kept to the end of the step, as `nonlinear::solve_with` keeps its
   halved load increment). The element's `S_xx(0)` is larger than `E eps0` by the factor `1 + eps0/2` (the Green-Lagrange
   strain), which changes the increments by about `1.5e-3` relative: the decision "two halvings" has a margin of 7 percent at
   the quarter step and 52 percent at the half step.
5. **Local non-convergence** (a8). The monotone Newton of item 1 converges linearly while `3 G A tau q^(n-1) >> 1` (each
   iteration takes about `q/n` off), then quadratically; the count grows with the logarithm of the step. Probe (`n = 5`, the
   strain `(1e-3, -3e-4, -3e-4, 0, 0, 0)` from the virgin state, `A` of item 3, `dt` in units of `t*`): 45 iterations at `1e20
   t*`, 50 at `1e22`, 56 at `1e25`, 66 at `1e30`, 86 at `1e40` (`probe_extra.log`). A step of `1e40 t*` is therefore a
   non-converged local iteration with the cap of 50; every other registered step needs at most about 25.
6. **The thick cylinder in plane strain** (first principles; the stationary creep field of a thick tube is classical, e.g.
   F. K. G. Odqvist, Mathematical Theory of Creep and Creep Rupture, 1966; H. Kraus, Creep Analysis, Wiley, 1980, chapter on
   thick-walled cylinders; J. T. Boyle and J. Spence, Stress Analysis for Creep, Butterworths, 1983; R. K. Penny and D. L.
   Marriott, Design for Creep, McGraw-Hill, 1971; all cited from memory, NOT re-read: the field is derived here). In the
   stationary state the stress is constant, the elastic strain rates vanish, the strain rate is the creep rate, which is
   deviatoric: `d eps_r + d eps_theta = 0` (plane strain, `d eps_z = 0`, so `s_z = 0`, `sigma_z = (sigma_r + sigma_theta)/2`).
   Compatibility with the radial velocity `v = C/r`: `d eps_theta = C/r^2`. The flow rule: `d eps_theta = (3/4) A q^(n-1)
   (sigma_theta - sigma_r)` with `q = (sqrt 3/2) |sigma_theta - sigma_r|`, so `sigma_theta - sigma_r = B r^(-2/n)`. Equilibrium
   `d sigma_r/dr = (sigma_theta - sigma_r)/r` and `sigma_r(a) = -p`, `sigma_r(b) = 0` give the acceptance's fields, independent
   of `E`, `nu`, `A` and of the time factor (`t^m` scales `C` only). At `n = 1` they are the Lame fields. The elastic start
   (Lame, plane strain): `sigma_r = -p ((b/r)^2 - 1)/((b/a)^2 - 1)`, `sigma_theta = p ((b/r)^2 + 1)/((b/a)^2 - 1)`, independent
   of `nu`.
   **Redistribution time.** The estimate of the stub: the elastic strain over the creep rate, `t* = 1/(E A p^(n-1))`. Set-up:
   `a = 1 m`, `b = 2 m`, `E = 2e9 Pa`, `nu = 0.3`, `p = 2e5 Pa` (`p/E = 1e-4`), `n = 4`, `m = 0`, `A = 1/(E p^3) = 6.25e-26`
   (`t* = 1 s`).
   **Probe** (`probe_cylinder.py`): small-strain plane strain on a quarter annulus of 8-node serendipity quadrilaterals with 3 x
   3 Gauss points (the in-plane rule of the Hex20 slice of D6), nodes on their circles, consistent bore pressure, the return of
   item 1 with `eps_z = 0`, global Newton to `1e-11`. Lame at `t = 0`, the largest point error over `sigma_r`, `sigma_theta`
   relative to `p`: `2.29e-2` (4 x 4 elements), `7.16e-3` (8 x 8, 8 x 12, 8 x 16), `3.44e-3` (12 x 12, 12 x 8): the radial
   resolution decides. Creep, 12 radial x 8 circumferential elements, 40 steps growing by the factor 1.05 to `5 t*` (first step
   `4.14e-2 t*`; `cyl_12x8_t5_40.log`), errors relative to `p` against the stationary field:
   | `t/t*` | point-wise | element means | point-wise `sigma_theta - sigma_r` |
   |---|---|---|---|
   | 0.89 | `3.86e-2` | `3.38e-2` | `3.94e-2` |
   | 1.97 | `1.42e-2` | `3.74e-3` | `4.53e-3` |
   | 2.75 | `1.82e-2` | `1.29e-3` | `1.61e-3` |
   | 3.74 | `2.32e-2` | `4.22e-4` | `5.72e-4` |
   | 5.00 | `2.97e-2` | `1.22e-4` | `3.43e-4` |
   The point-wise error falls to `1.3e-2` near `1.4 t*` and then grows linearly in time (`2.21e-1` at `20 t*` on 8 x 8,
   `cyl_8x8_t20_60.log`), while the deviatoric part and the element means converge: **volumetric locking** of the fully
   integrated serendipity element under the isochoric creep flow (9 pressure constraints per element against about 6 degrees
   of freedom), with a point pressure that oscillates inside the element about the right mean. No locking-free formulation is in
   the repository (`BBar`, `Incompatible` pending, ADR 0293; Hex8 only). Hence the reading of (a5) on the element means (C2) and
   the point-wise deviatoric check (C2d). Registered redistribution time `5 t*`. Largest equivalent creep increment of a step
   `1.30e-4` (the last step), the equivalent creep strain at `5 t*` at most `2.49e-3`: the geometric terms of the total
   Lagrangian driver are of relative size `2.5e-3` or below (against the limit `2e-2`).
7. **The rotating radial bar** (first principles). A bar on `0 <= r <= L` rotating at `Omega` about an axis through `r = 0`,
   free at `r = L`: the axial force at `r` carries the centrifugal load of `[r, L]`, `sigma(r) = rho Omega^2 (L^2 - r^2)/2`,
   whatever the material (statically determinate), constant in time; the creep strain rate is `A sigma^n` (`m = 0`), the tip
   creep rate `int_0^L A sigma^n dr`; for `n = 3`, `int_0^L (L^2 - r^2)^3 dr = (16/35) L^7`, so the rate is `A (rho Omega^2/2)^3
   (16/35) L^7`. The elastic strain does not change, so the tip rate is this value from the first instant.
   **Discretisation probe** (`probe_bar.py`, `probe_bar.log`): one-dimensional, the uniaxial law, consistent body load, `t* =
   1/(E A sigma_root^2) = 1 s`, 50 steps of `0.1 t*`, the rate over `[4, 5] t*` against the closed form: quadratic elements with 3
   Gauss points `-2.71e-4` (4 elements), `-1.48e-5` (8), `-7.8e-7` (16); linear elements `-1.9e-2`, `-4.6e-3`, `-1.1e-3`.
   Quadratic along the bar: Hex20.
   **Three-dimensional terms** (estimates): the centrifugal load also pulls the section outward (`rho Omega^2 y`); with the
   section `[-w, w]^2` and free lateral faces this gives `sigma_yy` of about `rho Omega^2 w^2/3` on average, which lowers the
   equivalent stress by about half of it: relative to `sigma` at the root `w^2/(3 L^2)`, times `n` in the rate. Total Lagrangian
   terms: the dead load is nominal, `S_xx = P_xx/F_xx`, lower by the total strain, times `n` in the rate. Set-up chosen from
   these: `L = 0.05 m`, `w = 5e-4 m` (`w^2/(3 L^2) = 3.3e-5`), `rho = 1200 kg/m^3`, `Omega = 250 rad/s` (`sigma_root = rho
   Omega^2 L^2/2 = 9.375e4 Pa`, elastic strain `4.7e-5`), `E = 2e9 Pa`, `nu = 0.3`, `A = 1/(E sigma_root^2) = 5.689e-20` (`t* =
   1 s`); at `5 t*` the creep strain at the root is about `2.3e-4`, the total `2.8e-4`, so the total Lagrangian term is at most
   about `3 x 2.8e-4 = 8.4e-4` at the root and less over the length. Sum of the estimates `<= 1.5e-3` against the limit `5e-3`.
8. **Default bits** (b3). The isotropic, orthotropic, tabulated, J2 and viscoelastic materials take the lines they took: the
   new arm of `evaluate` and of `Material::stress` matches `Material::Creep` only, `check_elements` gains one refusal of the
   creep variant, the classification of a failed attempt gains one key that only the creep update emits (D4), and the new
   driver and the new element function are new functions. The static driver gains one refusal of the creep variant.

## Design

### D1. The material (`mat_creep.rs`)

`PENDING = None`; `FOLLOW_UP = "a follow-up of ADR 0489"`; `STATE_SIZE = 7` (ADR 0440 D3, unchanged); `MAX_LOCAL_ITERATIONS =
50`; `LOCAL_TOLERANCE = 1e-14`; `ITERATION_KEY = "creep iteration"`. `update(material, strain, state, point) -> Result<Update>`
with `Update { stress, tangent, state: [f64; 7], creeping: bool, iterations: usize }`: the return of arithmetic 1 from the
committed `state` over the step of the point context: `dt = point.time_increment` (none: `dt = 0`, the elastic response from
the committed state), `t1 = point.time` (needed when `dt > 0`, else refused, key `"time"`), `t0 = t1 - dt` (refused below `-4
eps t1`, key `"time"`: time hardening counts from `t = 0`; a rounding residue is taken as 0). Refused: another material (key
`"material"`), an orthotropic base (`NotYetSupported`, location `"an orthotropic base"`, package `FOLLOW_UP`), `n < 1` (key
`"n"`), a state of the wrong length or with a non-finite value (key `"state"`), a non-converged local iteration (key
`ITERATION_KEY`, the iterations and `q/q_tr` in the reason). Accessors `creep_strain(state)`, `equivalent_creep_strain(state)`,
`time_factor(m, t1, dt)`.

### D2. The constitutive call and the checks (`material.rs`, `model.rs`)

`Material::stress` of a creep material returns `update`'s stress and tangent for the committed state at the point context given
(the state is not changed; the driver gets the new state from the evaluation, D3). `Material::elasticity_matrix` refuses it
(`NotYetSupported`, location `"the element-constant elasticity matrix"`, package `mat_creep::FOLLOW_UP`), as J2 and the
viscoelastic material. `check_elements` refuses it where the history flag is off (every path but the nonlinear driver), naming
`mat_creep::FOLLOW_UP`. The constructor `Material::creep` is unchanged.

### D3. History in the Newton driver (`nonlinear.rs`, no second loop)

The arm of `evaluate` that gives J2 and the viscoelastic material their trial state (ADR 0475 D3, ADR 0485 D3) gains the creep
material: `mat_creep::update` from the committed state of the attempt at the point context of the attempt. A new public
`element_force_and_tangent_at(element, coords, material, displacement, state, context)` is `element_force_and_tangent` at a
point context (for check T2); the existing function is unchanged. `nonlinear::solve` and `solve_with` (no time) refuse a creep
material (`NotYetSupported`, location `"material i, element set 's': the nonlinear static solve (no time)"`, package
`mat_creep::FOLLOW_UP`), as the viscoelastic one.

### D4. The located local error

A non-converged local iteration surfaces in the element loop as `FemError::Element { element, source: InvalidValue { key:
"creep iteration", .. } }` (the element-located error of `assemble_with`). The classification of a failed attempt (`classify`)
treats this key like the inversion key `"deformation gradient"`: the attempt's failure keeps the located error, and
`solve_from_at` returns it as it is (`FemError::Element` with the element). Only the creep update emits the key.

### D5. The quasi-static time-stepping driver (`nonlinear::solve_in_time`), the one driver of this item

(b1) A minimal driver, registered here, and no loop in the tests except for the comparison Q1:

`pub fn solve_in_time(model, prescribed, times, options: &TimeSteppingOptions) -> Result<TimeHistory>`, `TimeSteppingOptions {
newton: NewtonOptions, max_creep_increment: f64, max_halvings: usize }` (`new(max_creep_increment)`: Newton `NewtonOptions::new(1)`,
5 halvings; `max_creep_increment` positive, `f64::INFINITY` allowed: no limit), `TimeHistory { steps: Vec<TimeStep> }`,
`TimeStep { time, u, states, internal_force, halvings, substeps, max_creep_increment, iterations }`. Scope:

* The model's dead loads (`nodal_load_vector(model, &[])`) and the prescribed displacements (the model's and `prescribed`),
  **constant in time**, applied at `t = 0` by one attempt of `solve_from_at` from `u = 0` (the prescribed values set) at the
  context `with_time(0, 0)`: the instantaneous response, the first entry of `steps` (time 0).
* `times`: the step-end times, finite, strictly increasing, the first positive (key `"times"`). Each step `[t_k, t_{k+1}]` is
  taken in sub-steps of length `h`, `h = t_{k+1} - t_k` at first; a sub-step is one attempt of `solve_from_at` from the
  committed displacement and states at `with_time(t + h, h)` (the last sub-step ends exactly at `t_{k+1}`: a remainder below
  `1e-12 h` is absorbed, as `solve_with`).
* After a converged attempt the largest equivalent creep increment over the points of the creep materials (D1 state value 7)
  is compared with `max_creep_increment`; above it the attempt is discarded and `h` halved. A failed Newton attempt also halves
  `h`. A located creep-iteration error (D4) is returned at once, not halved (a8). After `max_halvings` halvings of one step the
  error is `FemError::Solver { solver: "creep time stepping", reason }` naming the step, its times and the increment or the
  failure. The halved `h` is kept to the end of the step and reset at the next step.
* `TimeStep` records the committed state at each step end only, with the halvings and the number of sub-steps of the step and
  the largest committed creep increment among its sub-steps.

Not in scope (left to F-410's proposed follow-up, now to be written over this driver): load and displacement histories in
time, a temperature, output between the step ends. The viscoelastic material runs on it as on the loop of `solve_from_at`
(check Q1), without an increment limit.

### D6. The thick cylinder (C1, C2, C2d)

Plane strain by a slice (b2), registered: `hex_box([1, 0, 0], [1, pi/2, hz], [12, 8, 1])` in the parameters `(r, theta, z)`,
`hz = 1/12 m` (the radial element size), upgraded by `upgrade_to_hex20_mapped` with `(r, theta, z) -> (r cos theta, r sin
theta, z)` (every node on its circle; positive Jacobian, arithmetic 6). Constraints: `u_y = 0` on the node set `ymin` (`theta =
0`), `u_x = 0` on `ymax` (`theta = pi/2`), `u_z = 0` at every node (plane strain; a node set of all nodes). Bore pressure `p` by
`Load::pressure` on the surface set `xmin` (`r = a`). Material of arithmetic 6. Driver D5 with the 40 times `t_k = dt0 (1.05^k -
1)/0.05`, `dt0 = 5 x 0.05/(1.05^40 - 1)` s (`t_40 = 5 s`), `max_creep_increment = 5e-4` (probe `1.30e-4`; halvings reported),
the driver's Newton defaults. The stress of a point: `element_second_piola_kirchhoff` at the committed displacement and state
(the default context gives the committed stress, D1: `tau = 0`), turned to polar components by the angle of the point's
reference position; the element mean is the mean over its 27 points weighted by `w_q det J_q`, compared with the same weighted
mean of the closed form at the points.

### D7. The rotating radial bar (B1)

`hex_box([0, 0, 0], [L, w, w], [10, 1, 1])` upgraded to Hex20 (quarter of the section `[-w, w]^2`), `u_x = 0` on `xmin` (root,
axially held), `u_y = 0` on `ymin`, `u_z = 0` on `zmin` (symmetry planes through the axis of the bar); `Load::centrifugal([0, 0,
1], [0, 0, 0], Omega)` (axis `z` through the root); material and constants of arithmetic 7. Driver D5, 50 steps of `0.1 s` to `5
s`, `max_creep_increment = INFINITY`. The tip: the node at `(L, 0, 0)`; the rate `(u_x(5 s) - u_x(4 s))/(1 s)`.

### D8. The halving and the local error (H1, H2, NC1)

H1, H2: the one-element relaxation of arithmetic 4 by the driver. NC1: `hex_box([0, 0, 0], [2, 1, 1], [2, 1, 1])` (Hex8),
element 0 in the set `e0` with the isotropic base, element 1 in `e1` with the creep material (`n = 5`, `A` of arithmetic 5 with
`sigma0 = 2e6 Pa`), symmetry planes on `xmin`, `ymin`, `zmin`, `u_x = 2e-3` on `xmax`; the driver with the times `[1e40]`.

### D9. The bit record of the default (B0)

`crates/rcfd-fem/tests/sa_creep_default_bits.rs`, written after this commit and run on the base 81960c8d before any library code;
its digests (FNV-1a 64 over the bits) are written into the file then. Content: the twelve of ADR 0485 D8 (`nonlinear::solve` of
an elastic Hex8 bar and of a J2 bar, `solve_from` with a spring, `nl_transient` of an elastic and of a J2 bar with their state
bytes, the Tet10 internal force and tangent, `Material::stress` isotropic and J2, `solve_static` and `modal_analysis` of a Hex20
cantilever, the refusal of J2 by `solve_static`), and three of F-410's paths: `solve_from_at` of the three-branch viscoelastic
Hex8 of ADR 0485 R1 at the strain step and 5 increments of `0.02 s`, `nl_transient` of a viscoelastic Hex8 bar started by a
velocity (30 steps, state bytes), and `element_force_and_tangent` of a distorted Hex20 with J2 from a non-zero state.

### D10. Records and copies

`sa_scaffold.rs`: `x1_modules_and_pending_after_f415_f420_f422_f416_f417_f432_f423_f424_f410` (row `mat_creep`) and
`m1_to_m4_after_f410` (the M1 and M4 rows of `Creep`) become ignored records citing ADR 0291; their copies `..._f410_f411` and
`m1_to_m4_after_f411` pass with `mat_creep` landed, `Creep` pending `None`, its state size 7 and its M4 row: `elasticity_matrix`
refused naming `mat_creep::FOLLOW_UP`, `stress` at the default context `Ok` with the bits of `apply(D, eps)` (no time increment:
elastic from the virgin state). Any other non-ignored test of the crate that fails because `Creep` landed is handled the same way
(record and copy), listed in the addendum.

## Checks (limits fixed now)

| Id | Clause | What | Limit | Estimate behind the limit |
|---|---|---|---|---|
| T1 | a1 | material tangent of `mat_creep::update` against central differences (step `1e-10`), the five cases of arithmetic 1: `max |C - C_fd| / max |C|` | `<= 1e-6` each | probe `<= 1.33e-9` |
| T2 | a1 | element tangent `element_force_and_tangent_at` against central differences of the element force (step `1e-9` times the element size), distorted Hex8 and Hex20 (seed `0xF411_0489`), a non-zero committed creep state, `n = 4`, `m = -0.5`, at `with_time(2, 0.5)`, with creep in the step at every point | `<= 1e-6` each | T1 plus the total Lagrangian element, `2e-11 .. 8e-11` (F-397, ADR 0475 T2) |
| U1 | a2 | material point, stress control `sigma = 2e6 Pa`, `A = 1e-25`, `n = 3`, `m = 0`, 100 steps of `0.1 s`: `|eps_c,xx - A sigma^n t| / (A sigma^n t)` at every step end | `<= 1e-8` | probe `5.8e-13` |
| U1e | a2 (companion) | one Hex8 (unit cube, symmetry planes), dead traction `sigma0 = 0.2 Pa` on `x = 1`, `A = 1.25e-9`, `n = 3`, `m = 0`, driver D5, 100 steps of `0.1 s` (Newton residual tolerance `1e-12`), `eps_c,xx` of every point against `A sigma0^n t` | `<= 1e-8` | geometric `n x 2e-10`; Newton `3e-10` |
| U2 | a3 | as U1 with `m = -0.5`, against `A sigma^n t^(m+1)/(m+1)` | `<= 1e-3` | probe `2.0e-13` |
| U2e | a3 (companion) | as U1e with `m = -0.5` | `<= 1e-3` | as U1e |
| R1 | a4 | one Hex8 (unit cube, symmetry planes), `u_x = 1e-3` on `x = 1`, `n = 3`, `A` of arithmetic 3, driver D5, 800 steps of `0.0125 s` to `10 s`: `max |S_xx - sigma(t)| / sigma(t)` over the step ends and points, `sigma0 = S_xx(0)` | `<= 5e-3` | probe `1.717e-3` |
| R2 | a4 | as R1 with `n = 5` | `<= 5e-3` | probe `7.156e-4` |
| R3 | a4 | the same at 400 steps; `log2(e_400/e_800)` for `n = 3` and `n = 5` | `>= 0.9` each | probe `0.994` |
| C1 | a5 | cylinder D6 at `t = 0`: point-wise `max(|sigma_r - Lame|, |sigma_theta - Lame|) / p` | `<= 1e-2` | probe `3.44e-3` (Q8 proxy) |
| C2 | a5 | cylinder D6 at `t = 5 t*`: element means, `max(|mean sigma_r - mean closed|, |mean sigma_theta - mean closed|) / p` over the elements | `<= 2e-2` | probe `1.22e-4` |
| C2d | a5 (companion) | the same, point-wise `|(sigma_theta - sigma_r) - closed| / p` | `<= 2e-2` | probe `3.43e-4` |
| C2r | a5 (report) | the point-wise `sigma_r`, `sigma_theta` error at `5 t*`; the element-mean error at every step end | reported, no limit | probe `2.97e-2` (locking, arithmetic 6) |
| B1 | a6 | bar D7: `|rate / (A (rho Omega^2/2)^3 (16/35) L^7) - 1|` | `<= 5e-3` | probe `1.5e-5` (1-D, 8 elements); 3-D and geometric `<= 1.5e-3` (arithmetic 7) |
| H1 | a7 | arithmetic 4, `max_creep_increment = 1e-4`, times `[1]`: `halvings == 2`, `substeps == 4`, committed increment `<= 1e-4`; the result (displacement, states, internal force) bit-equal to the driver at times `[0.25, 0.5, 0.75, 1]` without a limit | exact | arithmetic 4 |
| H2 | a7 | the same with `max_halvings = 1`: refused (`FemError::Solver { solver: "creep time stepping" }`), the reason names the step and the increment | refused | arithmetic 4 |
| NC1 | a8 | D8: the step of `1e40 s` gives `FemError::Element { element: 1, source: InvalidValue { key: "creep iteration" } }`, not halved (the reason does not mention a halving); the material point refuses the same step with the key | located error | arithmetic 5 (86 iterations) |
| Q1 | b1 | the driver against a loop of `solve_from_at` written in the test: the first 20 steps of R1 and the viscoelastic element of ADR 0485 R1 (strain step and 20 increments of `0.02 s`), every displacement, state and internal force | bits equal | D5 |
| N1 | b3 | B0: every digest of the base 81960c8d | exact | D9 |
| N2 | negative control | the linear paths refuse the creep material (`solve_static`, `modal_analysis`, `transient::Integrator::new`, `stiffness`) naming `mat_creep::FOLLOW_UP`; `elasticity_matrix` refused; `nonlinear::solve` refused (location `"... the nonlinear static solve (no time)"`); an orthotropic base refused at the update; `n = 0.5` refused at the update (key `"n"`); `dt > 0` without a time refused (key `"time"`); `solve_in_time` with times not increasing refused (key `"times"`) | all | D1, D2, D3, D5 |
| N3 | negative control | the R1 element through `solve_from` (default context, no time) at the step and 5 more attempts: every state value exactly `0.0` | exact | D1: `tau = 0` |
| N4 | b3 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | no shared output path changed |
| X1 | b5 | the records and copies of D10 | pass | ADR 0291 |

Files: `crates/rcfd-fem/tests/sa_creep.rs` (T1 to Q1, N2, N3), `sa_creep_default_bits.rs` (N1). A check that would take more
than about two minutes in release is an ignored heavy record run once, with a fast test on the same code path.

## Planned mutations (on a scratch copy, one at a time)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the time factor taken at the step end, `tau = dt t1^m` | U2, U2e (not U1: `m = 0`) |
| MU2 | the tangent without its last term, `beta` replaced by `theta` | T1, T2 |
| MU3 | the creep strain not added to the state | U1, R1, B1, C2 |
| MU4 | the driver never halves (the increment limit ignored) | H1, H2 |
| MU5 | the driver passes no time increment (`with_time(t, 0)`) | U1e, R1, B1, C2 |
| MU6 | the local equation with `q^(n-1)` in place of `q^n` | U1, R1, B1 |
| MU7 | `check_elements` ignores the history flag for the creep material | N2 |
| MU8 | `classify` does not keep the creep-iteration error located | NC1 |
| MU9 | the flow increment without the engineering factor on the shear components | T1, T2, C2 |

## Decision rule

- **done**: every check of the table met, every planned mutation caught by at least one check, fmt and clippy clean, B0 and the
  baseline hashes unchanged.
- **partial**: B0 and the baseline hashes unchanged, and at least the clauses (a1), (a2), (a3), (a4), (a7), (a8) met, with a miss
  of (a5) or (a6) diagnosed and recorded as a `*_known_miss` test.
- **todo**: otherwise (in particular any change of the default bits).

## Limits (not covered)

- Isotropic base only (an orthotropic base is refused at the update). `n >= 1` only (refused below). No temperature factor
  (Arrhenius or other): the data model of ADR 0440 has `a, n, m` only; a temperature term is a follow-up that changes the data
  model.
- Time hardening counts from `t = 0`, the start of the driver; no strain hardening; no primary-creep law other than `t^m`; no
  creep rupture or damage.
- Small-strain creep on the Green-Lagrange strain and the second Piola-Kirchhoff stress of the total Lagrangian driver (large
  rotation, small strain), as J2 and the viscoelastic material.
- Fully integrated elements lock under the isochoric creep flow (arithmetic 6): the point pressure of a Hex20 drifts in long
  creep; element means are reliable, point values are not. A locking-free element (ADR 0293, pending) is the remedy.
- The driver D5 holds loads and prescribed values constant in time; the static load-factor driver refuses the material; the
  linear paths refuse it. The nonlinear transient integrator passes the time of its sub-step and so takes the material, without
  an increment limit and without a check of this item.
- One mesh per check, no convergence study beyond the probes; no polymer data (decision A1 of ADR 0310): every constant is an
  input of a check.

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. **Driver or loop** (b1). A minimal driver (D5), taken: the halving clause (a7) needs a driver, and F-410's follow-up can extend
   this one (histories, temperature) instead of building a second.
2. **The reading of (a5) on element means**, taken (arithmetic 6): the point-wise reading measures the locking of the element,
   not the creep law; it is reported (C2r).
3. **`n < 1` refused at the update** rather than solved by a bracketed iteration, taken: the stub's laws are Norton laws with
   `n >= 1`, and the monotone Newton of arithmetic 1 is the one whose convergence is shown.
4. **Case-file key.** Not added (as F-410). Recommendation for the lead: a material entry `creep = { base, a, n, m }` and a study
   kind "creep" over the driver D5, offered when the maintainer opens the case file to the nonlinear studies.
