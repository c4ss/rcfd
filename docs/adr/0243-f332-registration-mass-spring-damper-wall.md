# ADR 0243: F-332 registration: frequency-dependent wall impedance of the compressible solver (mass-spring-damper wall, time domain), against the closed-form reflection coefficient in magnitude and phase

Status: **REGISTERED 2026-11-12**, written before any code of F-332 and before any run of its checks. Results go in an
addendum; nothing above the addendum is changed after a run. Not yet accepted by the maintainer.

Parents: [ADR 0233](0233-f243-registration-impedance-and-quiet-boundary.md) (F-243: `CompBc::Impedance`, `impedance_wall`,
the tube harness; its limit D7 "real, frequency-independent impedance only" is what this job lifts),
[ADR 0237](0237-f331-registration-sponge-layer-of-the-compressible-solver.md) (F-331: the pattern of an opt-in model with
its own step path and its own state advanced with the stage weights).

Number: the lead gave 0243. Branch `f332-frequency-dependent-impedance` from main eec021c5. Worktree
`E:/projects/cfd/w49i-work/f332`.

## What was run and read before this text (disclosed)

- **Read:** the F-332 entry of `docs/roadmap.json`; ADR 0233 in full (registration and addendum); of ADR 0237 the head,
  the decisions, the check tables, the mutation table, the decision rule, the limits and the mutation and deviation parts
  of its addendum (not the derivations DR3 to DR8 and not amendment 1 in detail); `crates/rcfd-fv/src/compressible_bc.rs`
  and `crates/rcfd-fv/src/compressible.rs` in full; `crates/rcfd-fv/tests/impedance_f243.rs` (the harness and the I, Q, T
  checks); the F-243 and F-331 sections of `docs/acoustics.md`; the public items of `crates/rcfd-fv/src/sponge.rs`.
- **Searched:** which crates name `CompBc::` (`rcfd-api` `comp.rs` and three of its tests). This job adds no variant to
  `CompBc`, so none of them is touched.
- **Built, not run:** `cargo test --release -p rcfd-fv --no-run` on the unchanged tree (to warm the target directory).
  No test was run.
- **Arithmetic, one scratch Python script (standard library, no rcfd code, not committed):** the closed-form reflection
  coefficients of the tables below (complex division), the decay rates `alpha` and `exp(-3 alpha)`, the wall step limits,
  and the largest modulus of the three Runge-Kutta stability polynomials on the disc `|z + 1| <= 1` (1.0000 for all
  three). Nothing in it models the solver.
- **By hand:** the derivations DR1 to DR7 and the estimates under the limits.

No library or test code of F-332 existed when this text was written and no check below was run.

House rules: limits are fixed here and are not changed after a run; a missed limit becomes an ignored `*_known_miss` test
with the limit intact and the measured number recorded, and is diagnosed, not tuned away; a defective set-up gets a
corrected test by an amendment committed before the corrected run, the old test staying as an ignored record; each
registered run is made once; the wall's parameters and the three frequencies are fixed here and are not searched for
after a run; defaults stay bit-identical; `baseline_hashes.rs`, the hash records and the goldens are not touched; no new
third-party crate, no unsafe code; unsigned commits, no trailers, no push.

## What F-332 asks

Acceptance (binding): "The reflection coefficient of a mass-spring-damper wall matches the closed form in magnitude and
phase within 2 percent at three frequencies around the resonance." Deps: F-243. The item comes from ADR 0233 (limit D7 and
the proposed follow-up).

**How "within 2 percent in magnitude and phase" is read (fixed now).** Magnitude: `| |R|/|R_cf| - 1 | <= 0.02`. Phase:
`|arg(R) - arg(R_cf)| <= 0.02 min(1, |arg(R_cf)|)` radian, the difference taken on the circle and `arg` in `(-pi, pi]`.
This is the stricter of the two usual readings at every frequency: 2 percent of the closed-form phase where that phase is
below one radian, and 0.02 rad (1.15 degrees, 0.32 percent of a cycle) elsewhere. A reading "2 percent of a cycle" (7.2
degrees) is not used. Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).

## Derivations

Notation as ADR 0233: ambient `rho0`, `p0`, `c`, at rest, `Z0 = rho0 c`; outward unit normal `n` of the boundary face;
`u_n = u.n`; `w+ = p' + Z0 u_n'` runs towards the wall, `w- = p' - Z0 u_n'` away from it. Time factor `e^{+i omega t}`
throughout (so a mass has the reactance `+omega m`).

**DR1. The wall.** Per unit area the wall is a plate of mass `m` [kg/m^2] on a spring `k` [Pa/m] and a damper `R` [Pa
s/m]; `xi` is its displacement along `n` (into the wall), `v = d xi/dt`. Newton's law with the fluid pressure on its face:

    m dv/dt + R v + k xi = p_face - p_ref,        d xi/dt = v.

The fluid at the wall moves with the wall, `u_n = v` (the wall face of the mesh does not move: the displacement is small
against a wavelength; this is the usual linearised, locally reacting wall). For `e^{i omega t}`: `p' = Z(omega) u_n` with

    Z(omega) = R + i (omega m - k/omega),

the three-parameter model (mechanical oscillator: Kinsler et al. 2000, ch. 1 and 10; as a time-domain wall condition: Tam
and Auriault 1996). The undamped resonance is `omega0 = sqrt(k/m)`, where `Z = R`. By ADR 0233 DR1, which does not use
that `Z` is real,

    R_c(omega) = (Z(omega) - Z0)/(Z(omega) + Z0),

the ratio of the reflected to the incident complex pressure amplitude at the wall plane. With `R = 0`: `|R_c| = 1` at
every frequency (the wall stores energy and returns it).

**DR2. The face state (finite amplitude).** Given the wall velocity `v`, the face keeps the entropy, the tangential
velocity and the outgoing Riemann invariant of the interior cell, exactly as ADR 0233 DR5, and has the normal velocity `v`
(a piston): with `u_n,i`, `a_i`, `p_i` of the interior cell

    a_f = a_i - (gamma - 1)/2 (v - u_n,i),   p_f = p_i (a_f/a_i)^(2 gamma/(gamma - 1)),   rho_f = rho_i (p_f/p_i)^(1/gamma).

The solver takes the physical flux of this state (the "direct" path of the characteristic conditions). Linearised: `p_f' =
w+_i - Z0 v`. So the wall obeys

    m dv/dt = p_f(v; interior) - p_ref - R v - k xi      (linear: m dv/dt + (R + Z0) v + k xi = w+_i),

a damped oscillator driven by the incoming wave; the term `Z0 v` is the radiation damping. With `w+_i = 2 p_inc` at the
wall this gives `v = 2 p_inc/(Z + Z0)` and `p_ref = p_f' - p_inc = p_inc (Z - Z0)/(Z + Z0)`: DR1.

**DR3. The limit `m, k -> 0`.** With `m = 0` the equation of DR2 is algebraic: `p_ref + k xi + R v - p_f(v) = 0`. This is
`g(u) = 0` of ADR 0233 DR5 with the reference pressure `p_ref + k xi`. So for `m = 0` the face state is
`impedance_wall(gas, p_ref + k xi, R, interior, n)`, the wall carries `xi` only (`d xi/dt = u_n,f`), and for `m = 0`, `k =
0` the face state is `impedance_wall(gas, p_ref, R, interior, n)` bit for bit (`p_ref + 0 xi` is `p_ref` in floating
point for every finite `xi`). For `m > 0` the steady solution of DR2 (`dv/dt = 0`, `k = 0`) is the same root, so a light
wall relaxes to the impedance wall with the rate `(R + Z0)/m`.

**DR4. Time integration.** `(xi, v)` per wall face are advanced with the solver's Runge-Kutta stages: every stage
evaluates the rates of DR2 from the stage's interior state and the stage's `(xi, v)`, the fluxes of the stage use the
face state of the same `(xi, v)`, and `(xi, v)` take the same convex stage combinations as the conserved state (Shu and
Osher 1988), as the memory `Q` of ADR 0237 D2 does. The coupled system is one system of ordinary differential equations
integrated by one scheme, so the order of the scheme holds for it.

**DR5. Stability of the wall equation and the step limit.** The linear wall equation has the matrix `[[0, 1], [-k/m,
-d/m]]`, `d = R + Z0`, with the eigenvalues `lambda = -a +- sqrt(a^2 - k/m)`, `a = d/(2m)`. Each of the three schemes is
a convex combination of forward-Euler steps of size `dt` (SSP coefficient 1), so its stability polynomial `P` satisfies
`|P(z)| <= 1` on the disc `|z + 1| <= 1` (Gottlieb, Shu and Tadmor 2001; checked numerically beforehand and as W5). The
eigenvalues of the step matrix are `P(dt lambda)`, so the wall equation is stable when `|1 + dt lambda| <= 1` for both
eigenvalues:

- complex pair (`a^2 < k/m`): `(1 - a dt)^2 + dt^2 (k/m - a^2) <= 1`, that is `dt <= 2a/(k/m) = d/k`;
- real pair: `dt <= 2/|lambda|_max`, and `|lambda|_max < 2a = d/m`, so `dt <= 2m/d` suffices;
- `m = 0`: one eigenvalue `-k/d`, `dt <= 2d/k`; the stricter `d/k` is used.

In the complex case `d/k < 4m/d`, in the real case `2m/d <= 2/|lambda|_max`, so in every case

    dt_wall = 0.8 min( 2m/(R + Z0) if m > 0,  (R + Z0)/k if k > 0 )

is sufficient (infinite for `m = k = 0`); 0.8 keeps a distance from the edge of the disc, where forward Euler does not
decay. `Z0` is taken as `rho a` of the interior cell of the face. The analysis is linear, with frozen coefficients, for
the wall equation alone: the fluid cell is covered by the solver's own CFL limit, and to first order the wall does not
feed back into its own forcing (`w+_i` comes from the interior).

**DR6. Free decay (closed form for check F1).** Fluid at rest at `p_ref`, wall released from `xi(0) = xi0`, `v(0) = 0`:
`w+ = 0`, so `m xi'' + (R + Z0) xi' + k xi = 0` and, for `a^2 < k/m`, `omega_d = sqrt(k/m - a^2)`:

    xi(t) = xi0 e^{-a t} (cos(omega_d t) + (a/omega_d) sin(omega_d t)),    v(t) = -xi0 (k/m)/omega_d e^{-a t} sin(omega_d t).

The wall radiates into the tube; nothing comes back before `t = 2 L/c`.

**DR7. Measuring the complex reflection coefficient of a tone.** Steady tone of angular frequency `omega`, `k_w =
omega/c`, at the distance `s` from the wall: `p'(s, t) = Re[(A e^{i k_w s} + B e^{-i k_w s}) e^{i omega t}]` (the
incident wave arrives at `s` before it arrives at the wall), `R_c = B/A`. Two least-squares steps, both linear:

1. per probe cell `j`: over the samples `(t_n, p_n - p0)` of the window fit `a cos(omega t) + b sin(omega t) + d`;
   `P_j = a - i b` (the constant `d` takes a mean offset);
2. over the probes: minimise `sum_j |P_j - A e^{i k_w s_j} - B e^{-i k_w s_j}|^2` for the complex `A`, `B` (2 x 2 normal
   equations); `R = B/A`.

This is the two-microphone decomposition (Chung and Blaser 1980) with many microphones in the least-squares sense (Jang
and Ih 1998), in the time domain with a known frequency. Its errors: (i) the wavenumber of the scheme differs from
`omega/c` (the decomposition extrapolates over `s` with the exact one): a phase error of `2 dk s`; (ii) numerical damping
between the probes and the wall lowers `|R|`; (iii) what is left of the wall's start-up transient, which decays as `e^{-a
t}` and is not a tone; (iv) the closure of the boundary cell. They are measured together by the same harness on walls
whose reflection is known and real (checks K).

On (iv), worked beforehand: the face takes the outgoing invariant of the cell centre, half a cell before the wall, which
by itself would advance the reflection by `k_w dx/2` (first order). But the boundary cell's own outgoing flux is first
order upwind (`c w+_N`), which makes `w+_N` the low-pass of the upstream face value with the time constant `dx/c`, that
is a delay of `dx/c` from the upstream face, which is the value at the wall to second order. The estimate below uses the
second-order value and names the first-order one as the pessimistic case.

## Decisions

- **D1. One opt-in wall model, `MsdWall { p, r, m, k }`** in `compressible_bc.rs`: `p` the reference pressure, `r` [Pa
  s/m], `m` [kg/m^2], `k` [Pa/m], all finite, `r, m, k >= 0`, `p > 0`. It is set per patch with
  `CompressibleSolver::set_msd_wall(patch, Option<MsdWall>)` and replaces the patch's `CompBc` while it is set. It is not
  a `CompBc` variant: the wall has state, `CompBc` is a `Copy` value read by stateless functions
  (`characteristic_face_state`), and `rcfd-api` matches on `CompBc`. Taken on the maintainer's standing instruction of
  2026-11-11 (take the recommendation); the alternative, a variant `CompBc::MassSpringDamper`, would leave the public
  stateless face-state function without an answer for it.
- **D2. State per wall face:** displacement `xi` and velocity `v` (two numbers per boundary face of a patch with a wall;
  `v` is not used for `m = 0`). Start at zero; `msd_wall_state(patch)` and `set_msd_wall_state(patch, ..)` read and set
  them (restart, check F1).
- **D3. Face state and rates:** DR2 for `m > 0` (`compressible_bc::piston_face`), DR3 for `m = 0` (`impedance_wall` with
  the reference `p + k xi`). A supersonic outflow relative to the wall returns the interior state, as `impedance_wall`
  does. A displacement that makes `p + k xi` non-positive is an error of the step.
- **D4. Time integration:** DR4, in a separate step path taken only while a wall is set (`step` branches as it does for
  the sponge); without a wall the existing statements run unchanged. `flux_sum` gets one more optional argument (the wall
  face states); with `None` it executes the existing statements.
- **D5. Stability and refusals.** `msd_dt_limit()` is the smallest `dt_wall` of DR5 over the wall faces (infinite without
  a wall). `max_dt()` returns the smaller of the CFL step and this limit while a wall is set, so `run_to` and every loop
  on `max_dt()` stay stable; a light mass or a stiff spring then costs steps, it does not blow up. `step(dt)` with `dt`
  above the limit is refused with an error (no silent instability). `set_msd_wall` refuses negative or non-finite `r`, `m`,
  `k`, a non-positive or non-finite `p` and an unknown patch. A mass of exactly zero selects the algebraic form of DR3,
  which has no mass limit; a spring of exactly zero has no spring limit.
- **D6. Not with a sponge and not on moving meshes.** `set_msd_wall` is refused while a sponge is set and `set_sponge`
  while a wall is set (the two step paths are not merged here); `step_ale` and `replace_mesh` return an error while a wall
  is set (the state is per face index; DR1 is not derived for a moving face).
- **D7. No case-schema wiring** (the job's instruction; a new key needs an approved edit of a registered hash test). No
  file of `rcfd-api` is edited.
- **D8. The wall and the frequencies of the acceptance, fixed now.** `Z0 = 1`, `c = 1`. Wall M: `R = 0.5 Z0`, `m =
  1/pi`, `k = 16 pi`, so `omega0 = 4 pi` (wavelength 0.5) and `m omega0/Z0 = 4`. Frequencies: wavelengths 0.625, 0.5, 0.4,
  that is `omega/omega0` = 0.8, 1, 1.25, where the reactance is `-1.8`, `0`, `+1.8`. Wall E: the same with `R = 0`.

Code: `crates/rcfd-fv/src/compressible_bc.rs` (`MsdWall`, `piston_face`, unit tests W), `crates/rcfd-fv/src/compressible.rs`
(one field, the methods of D1, D2, D5, the walled step, the optional argument of `flux_sum`, the refusals), a new test
file `crates/rcfd-fv/tests/msd_wall_f332.rs`. No existing test is edited.

## Checks and limits (registered)

Common: `gamma = 1.4`, `rho0 = 1`, `p0 = 1/1.4`, `c = 1`, `Z0 = 1`; solver defaults (HLLC, MUSCL van Leer, SSP-RK3, CFL
0.4), every step `max_dt()` unless stated; `eps = 1e-4 p0`; waves are exact isentropic simple waves (ADR 0233).

### W. Algebra (fast unit tests)

| id | check | limit |
|---|---|---|
| W1 | `piston_face`, 20 pseudo-random subsonic states x `v/a_i` in {-0.3, 0, 0.2}: normal velocity `= v`; entropy; outgoing invariant; tangential velocity | each `<= 1e-12` relative |
| W2 | equilibrium: `f = impedance_wall(p_ref, z, ..)`, `v* = f.u.n`, 20 states x `z/(rho a)` in {0.1, 1, 10}: `piston_face(.., v*)` against `f` (density, pressure, velocity); the rate `m dv/dt` of `MsdWall { p_ref, r: z, m: 1, k: 0 }` at `v*` against `p_i` | `<= 1e-12` relative each |
| W3 | linear limit of the rates: perturbations of size `1e-6` about the ambient state in three mixes, wall M with `xi = 1e-8`, `v = 3e-7`: `m dv/dt` against `w+_i - (R + Z0) v - k xi`; `d xi/dt = v` | `<= 1e-10` absolute; exactly |
| W4 | `m = 0`: the face state satisfies `p_f - p - r u_n,f - k xi = 0`, `d xi/dt = u_n,f`; with `k = 0` it equals `impedance_wall(p, r, ..)` | `<= 1e-12 p_i`; exactly; bit for bit |
| W5 | step limit: the four cases of DR5 against the formula; `|P(z)| <= 1` for the three schemes on a grid of the disc `|z + 1| <= 1`; for 200 pseudo-random `(r, m, k, Z0)` both eigenvalues satisfy `|1 + (dt_wall/0.8) lambda| <= 1` | `1e-15` relative; `1 + 1e-12`; `1 + 1e-12` |
| W6 | refusals (D5, D6): negative, NaN and infinite `r`, `m`, `k`; `p <= 0`; unknown patch; wall with a sponge set and sponge with a wall set; `step_ale` and `replace_mesh` with a wall; a state of the wrong length; `step(dt)` with `dt = 2 x` the wall limit; `set_msd_wall(patch, None)` accepted | error / ok |
| D1 | the decomposition of DR7 on synthetic data: `p(s, t)` of DR7 with `A = 0.7 e^{0.4 i}` and the three `R_cf` of table M, probes at 400 distances in `[0.1, 0.6]`, 4000 sample times in `[4.6, 6.6]` with uneven steps, plus a constant offset: `|R - R_cf|` | `<= 1e-9` |

Estimates: round-off for all; W3 the nonlinear terms are `1e-12`; D1 the normal equations are well conditioned (probe span
0.8 to 1.25 wavelengths).

### L, B. The limit `m, k -> 0` and the defaults (fast)

- **L1.** `box_2d(400, 1, 1.0, 1/400)`, the pulse of ADR 0233 set-up P, `xmin` `Fixed(ambient)`; `xmax` once
  `CompBc::Impedance { p0, z }` and once `MsdWall { p0, r: z, m: 0, k: 0 }` for `z` in {0, 1/3, 3}; 1400 steps of
  `max_dt()` (the pulse is reflected in them): `max_dt()` of every step and the conserved state of every cell after every
  100 steps are **bit-identical** (`to_bits`). Control: with `k = 1` the final state differs.
- **B1.** The same grid and pulse, `xmax` `Slip`, 200 steps: a solver on which a wall was set and cleared again
  (`set_msd_wall("xmax", None)`) against one never touched: `max_dt()` and state bit-identical.
- **B2.** `cargo test --release -p rcfd-fv --lib compressible`, `--test compressible_bc`, `--test compressible_ale_f258`,
  `--test compressible_ale_f258_records` (the F-258 bit records), `--test impedance_f243`, `--test sponge_f331` (the
  non-ignored tests of each) pass unedited.

### F1. Free decay of the wall against DR6 (fast)

`box_2d(400, 1, 1.0, 0.0025)`, fluid ambient at rest, `xmin` `Fixed(ambient)`, `xmax` wall M with the state `xi = xi0 =
1e-6`, `v = 0` on its face; steps of `max_dt()` until `t >= 1.0` (the radiated wave reaches `xmin` at `t = 1`). After
every step: `e_xi = |xi - xi_cf|/xi0`, `e_v = |v - v_cf|/(omega0 xi0)`, with `a = 0.75 pi = 2.3562`, `omega_d = 12.3435`.

| id | measure | limit | estimate |
|---|---|---|---|
| F1 | largest `e_xi` and largest `e_v` over the steps | `<= 2e-3` each | the wall equation is integrated to third order (`(omega0 dt)^3` per step with `dt = 5e-4`: below `1e-6`); the radiation term rests on `w+ = 0` in the boundary cell, which the reconstruction disturbs at the order `(k_w dx)^2 = 1e-3` of the radiated wave, a relative error of the damping of that order: `2e-4` of `xi0`. A wall advanced by forward Euler would be off by `omega0^2 dt t e^{-a t}/2`, `6e-3` at its largest |

### S. Stability of the coupling (fast)

Set-up: `box_2d(400, 1, 1.0, 1/400)`, the pulse and the probe of ADR 0233 set-up P (`R = A_ref/A_inc` at the first cell
with `x > 0.75`, incident before `t = 0.5`, reflected after), `xmin` `Fixed(ambient)`, until `t >= 1.0`. Two solvers in
lockstep with the same `dt` = the smaller of their `max_dt()`: the wall under test and a reference boundary. The CFL step
of this grid is `5e-4`.

| id | wall under test | wall limit | reference | limit | estimate |
|---|---|---|---|---|---|
| S1 (light mass) | `MsdWall { p0, r: 3, m: 4e-4, k: 0 }` | `1.6e-4` | `Impedance { p0, z: 3 }` | `max_dt() == msd_dt_limit()` at the first step; `|R - R_ref| <= 0.005` | `omega m = 0.011` at the pulse's frequencies: the reflection differs from the real one by `1.4e-3` in quadrature, which moves the peak at second order |
| S2 (stiff spring, no mass) | `MsdWall { p0, r: 3, m: 0, k: 4e4 }` | `8e-5` | `Slip` | the same two | `1 - R_c = 2/(Z + 1)` with `Z = 3 - 1000 i`: `2e-3` in quadrature |

Why this is a stability check: without the wall limit S1 would run at `dt lambda = -5` and S2 at `-5`, outside the
stability interval `[-2.51, 0]` of RK3 (mutation MU6).

### K, M, E. The tube with a tone (heavy, `#[ignore]`, release): calibration, acceptance, energy

Set-up Z: `box_2d(6400, 1, 8.0, 0.00125)` (`dx = 0.00125`; 500, 400 and 320 cells per wavelength), wall at `x = 8` on
`xmax`, `xmin` `Fixed(ambient)`. Initial right-running train `p - p0 = eps E(x) sin(2 pi x/lambda)`, `E = 1` on `[0.75,
7.0]`, `sin^2` ramps on `[0.25, 0.75]` and `[7.0, 7.5]`, zero elsewhere. Run until `t >= 6.6`. Probes: every cell whose
centre is at a distance `s` in `[0.1, 0.6]` from the wall (400 cells). Window: the steps with `4.6 <= t <= 6.6`. `R` by
DR7 with `omega = 2 pi/lambda`, `k_w = omega`.

Timing: the plateau reaches the wall at `t = 1.0`; three time units later (`e^{-3a}` = `8.5e-4` for wall M, `9.0e-3` for
wall E) the reflection of the settled wall has reached the farthest probe at `t = 4.6`; the end of the incident plateau
passes the nearest-to-`xmin` probe at `t = 6.65`; the reflected front reaches `xmin` at `t = 8.5`, after the end. No
sponge is needed.

| id | `xmax` | lambda | `R_cf` (magnitude, phase in rad) | limit |
|---|---|---|---|---|
| K1, K2, K3 | `CompBc::Impedance { p0, z: 0.5 }` (ADR 0233) | 0.625, 0.5, 0.4 | `1/3`, `pi` | magnitude `| |R|/|R_cf| - 1 | <= 0.01`; phase `<= 0.01` rad |
| K4 | `CompBc::Slip` | 0.5 | `1`, `0` | the same |
| M1 (below) | wall M | 0.625 | `0.797309`, `-0.965685` (-55.33 degrees) | magnitude `<= 0.02`; phase `<= 0.0193` rad |
| M2 (at resonance) | wall M | 0.5 | `0.333333`, `pi` | magnitude `<= 0.02`; phase `<= 0.02` rad |
| M3 (above) | wall M | 0.4 | `0.797309`, `+0.965685` | magnitude `<= 0.02`; phase `<= 0.0193` rad |
| E1, E2, E3 | wall E (`R = 0`) | 0.625, 0.5, 0.4 | `1`; `-1.014197`, `pi`, `+1.014197` | `| |R| - 1 | <= 0.01`; phase `<= 0.02` rad |
| G (reported, no limit) | wall M on `box_2d(3200, 1, 8.0, 0.0025)` | 0.5 | as M2 | none: the deviation at half the resolution |

The K limit is half the acceptance tolerance: the measurement has to resolve what M is judged by. M is the acceptance
(read as fixed above). E is the energy statement.

Estimates. Measurement and scheme (K, and the same part of M and E): damping of a tone over the mean two-way path of 0.7,
from ADR 0233 (0.11 percent of a pulse over 0.5 at 80 cells per width; T1 0.19 percent at 200 cells per wavelength), at
twice that resolution: below `1e-3` of `|R|`; wavenumber error `(k_w dx)^2/6 = 4e-5` relative, `2 dk s = 5e-4` rad;
boundary closure (DR7 iv): second order, `(omega dx/c)^2/2 = 2e-4`; pessimistic first-order case `k_w dx/2 = 6.3e-3` to
`9.8e-3` rad, still inside the K limit; nonlinearity `eps/p0 = 1e-4`. Transient: wall M `8.5e-4` at the start of the
window, averaged over the window `2e-4`; wall E `9e-3` at the start, averaged `3e-3` (at resonance it has almost the
tone's frequency and counts in full). Sum: K about `2e-3` and `1e-3` rad; M about `2e-3` and `2e-3` rad, a tenth of the
limit; E about `4e-3` and `4e-3` rad.

### Negative controls

- NC1: the K1 and K3 measurements (the damper alone, `Z = R`) judged against the closed form of wall M at the same
  frequencies: magnitude deviation `>= 0.5` and phase deviation `>= 2` rad (closed form: 0.58 and 2.18). A wall without
  mass and spring fails the acceptance by a factor above 25, so M tests the frequency dependence.
- NC2: `step(dt)` with the CFL step in set-up S1 (above the wall limit) is refused (part of W6); MU6 shows what happens
  without the refusal.
- L1's control (`k = 1` differs) and B1.

## Mutations (registered; scratch copies, not committed)

Each is applied to `compressible_bc.rs` or `compressible.rs` in a scratch copy of the tree; then `cargo test --release -p
rcfd-fv --lib compressible_bc` and `cargo test --release -p rcfd-fv --test msd_wall_f332` (the fast tests) are run once.

| id | mutation | expected to be caught by |
|---|---|---|
| MU1 | sign of the spring force (`+k xi` in `m dv/dt`) | F1 |
| MU2 | radiation damping lost: the wall is driven by the interior pressure `p_i` in place of the face pressure `p_f(v)` | W2, W3, F1 |
| MU3 | the wall advanced by forward Euler with the rates of the first stage only (the stages see a frozen wall) | F1 (estimate `6e-3` against `2e-3`); possibly nothing else |
| MU4 | displacement not advanced (`d xi/dt = 0`) | W3, W4, F1, S2 |
| MU5 | the wall face state not passed to the fluxes (the fluid sees the patch's `CompBc`, a rigid wall) | L1, F1, S1 |
| MU6 | the wall limit removed from `max_dt` and the refusal removed from `step` | W6, S1, S2 (instability) |
| MU7 | `m = 0` path ignores the spring (`p + k xi` replaced by `p`) | W4, S2, L1's control |
| MU8 | mass doubled in the rate (`2m dv/dt = ...`) | W3, F1 |

A mutation that no fast check catches is reported as surviving. The heavy checks are not run against mutations (cost).

## Decision rule (registered)

- **Acceptance met in the library** when M1, M2 and M3 meet both limits, K1 to K4 meet theirs (the measurement resolves
  the tolerance), E1 to E3, F1, S1, S2, L1, B1, B2, W1 to W6 and D1 meet theirs, NC1 holds and every mutation is caught.
- If M meets its limits and K misses, the acceptance is **not shown** (the measurement is not proven); if M misses, it is
  **not met**; either way the miss is kept as a `*_known_miss` test, diagnosed and named.
- **Proposed roadmap status.** `done` needs, beyond "acceptance met", that the wall can be selected from a case file; by D7
  that is not part of this job, so the status this job can reach is **partial** (acceptance met in the library, wiring
  open). If M, K, E, F, S or L misses or a mutation survives: **partial** with the miss named and the acceptance "not met"
  or "not shown". **todo** stays if the walled step is not usable at all (for example unstable in F1).
- G decides nothing.

## Limits

- Locally reacting wall with one degree of freedom per face; no coupling between neighbouring faces (no plate bending, no
  membrane tension), no cavity model, no multi-pole fit of a measured impedance curve.
- Linearised kinematics: the mesh face does not move, the wall velocity is a transpiration velocity; valid for
  displacements small against the cell size and the wavelength. Mass crosses the face (`rho v` per area) and comes back
  when the wall returns: the totals of the solver are not conserved to round-off with a wall.
- Normal incidence is checked. At oblique incidence the closed form is `(Z cos - Z0)/(Z cos + Z0)` (ADR 0233 DR2), not
  checked here.
- Explicit integration: a light mass or a stiff spring limits the step (DR5); the limit is a linear estimate with frozen
  coefficients.
- Not with a sponge, not on moving meshes (D6). Library only (D7). Gas at rest, Cartesian grids, perfect gas, inviscid.
- Verification against closed forms; no measurement is compared.

## Sources

- C. K. W. Tam, L. Auriault, AIAA J. 34 (1996) 917-923 (time-domain impedance condition, three-parameter model).
- K.-Y. Fung, H. Ju, AIAA J. 39 (2001) 1449-1454 (impedance conditions on characteristics in the time domain).
- L. E. Kinsler, A. R. Frey, A. B. Coppens, J. V. Sanders, *Fundamentals of Acoustics*, 4th ed., Wiley, 2000, ch. 1
  (damped oscillator, mechanical impedance), ch. 6 and 10 (reflection, impedance tube).
- A. D. Pierce, *Acoustics*, Acoustical Society of America, 1989, sec. 3-3.
- J. Y. Chung, D. A. Blaser, J. Acoust. Soc. Am. 68 (1980) 907-913 (two-microphone decomposition).
- S.-H. Jang, J.-G. Ih, J. Acoust. Soc. Am. 103 (1998) 1520-1526 (least squares over several microphones).
- C.-W. Shu, S. Osher, J. Comput. Phys. 77 (1988) 439-471; S. Gottlieb, C.-W. Shu, E. Tadmor, SIAM Review 43 (2001)
  89-112 (SSP schemes as convex combinations of Euler steps).
- E. F. Toro, *Riemann Solvers and Numerical Methods for Fluid Dynamics*, 3rd ed., Springer, 2009, ch. 3.

All are cited from memory of their content; none was opened for this job and no equation number is quoted. Every formula
used is derived above from Newton's law for the wall, the linear acoustic equations and the isentropic simple-wave
relations. No vendor code or documentation was used.

## Open questions (with recommendations)

- Q-A. Case-schema wiring (keys for `r`, `m`, `k` on a boundary, the reference pressure from `pressure_value`). Not taken:
  it needs an approved edit of a registered hash test. Recommendation: a follow-up item, together with the wiring of ADR
  0233's impedance (the real impedance is the case `m = k = 0`).
- Q-B. The reading of "2 percent in phase". Taken as fixed above (the strictest reading). No default depends on it.
- Q-C. Wall together with a sponge, and on moving meshes: refused here (D6). Recommendation: merge the two step paths
  when a case needs both; an ALE form only with a use case.
