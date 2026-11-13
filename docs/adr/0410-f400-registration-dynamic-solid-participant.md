# ADR 0410: F-400 registration: a dynamic solid as a co-simulation participant, on interface weights built from the two surfaces

Status: **REGISTERED 2026-11-13, written before any code of F-400 and before any run of its checks.** Not yet accepted by the
maintainer. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parents: the design stub [ADR 0303](0303-dynamic-structural-participant.md) (SI-13) and the intermediate-tier plan
[ADR 0290](0290-structural-solver-intermediate-tier-plan.md) (section 2.4 item 6, section 2.5 items 4 and 5). Builds on ADR 0389
(F-398, the transient integrator) and ADR 0393 (F-490, its transformation path), ADR 0256 (F-139, the protocol and
`coupled_step`), ADR 0261 (F-361, the least-squares update), ADR 0263 (F-362, `InterfaceMap::build`) and ADR 0286 (F-385,
`SolidInterface` and `StaticSolid`). Roadmap item F-400 (deps F-398, F-139, F-361, F-362, F-385). Size M. Number 0410 given by
the lead. Branch `f400-dynamic-solid-participant`, worktree `w49i-work/f400`, scratch `w49i-work/f400-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs; each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, the hash tables or a golden file; existing tests are not edited; no new
dependency, no unsafe code; clean-room (every formula below is derived here or is the standard Newmark (1959) recurrence already
implemented and cited in ADR 0389).

## What F-400 asks

The acceptance of the roadmap entry, binding: "Every limit not stated here is registered in the ADR before the run. A bar (nu = 0)
whose end face is the interface, coupled by coupled_step to a mock fluid that returns the force -m_a times the wall acceleration:
the first coupled frequency equals the root of k L tan(k L) = rho A L/m_a within 1 percent at 100 steps per period, for mass
ratios m_a/(rho A L) of 0.5 and 4; at ratio 4 the plain iteration (factor 1) diverges as the protocol predicts (registered
control) while Aitken's factor and the least-squares update converge to 1e-8 within registered iteration caps. With a mock spring
k_f and a slow load the response tends to F_0/(k_s + k_f) of F-385 within 1e-6. Mapping built by F-362 for a matching and for a
3:2 non-matching interface: resultant force, moment and work conserved within 1e-12, and the coupled frequency changes by less
than 1e-3 between the two. Restore returns the checkpoint bit for bit; a failing structural step stops the coupled step with the
participant's name and both sides restored. No flow solver is coupled."

Clauses: (c1) frequency at ratio 0.5 within 1 %; (c2) at ratio 4 within 1 %; (c3) plain iteration diverges at ratio 4 as
predicted; (c4) Aitken and least squares converge to 1e-8 within registered caps at ratio 4; (c5) spring and slow load to
`F_0/(k_s + k_f)` within 1e-6; (c6) resultant, moment, work within 1e-12 with the built map, matching and 3:2; (c7) frequency
change matching against 3:2 below 1e-3; (c8) restore bit for bit; (c9) a failing structural step stops the step naming the
participant, both sides restored; (c10) no flow solver.

## What was read (no run)

ADR 0303 and the sections of ADR 0290 named above; the roadmap entries F-139, F-361, F-362, F-385; `docs/cosimulation-protocol.md`
(the participants, and "Why relaxation is needed"); `crates/rcfd-fv/src/cosim.rs` (the traits `Participant`, `Structure`, `Fluid`,
`InterfaceMap`, `CouplingSettings`, `SliceRelaxer::factor`, `coupled_step`); `crates/rcfd-post/src/solid_coupling.rs` (whole);
`crates/rcfd-post/tests/sb_solid_coupling.rs` (set-ups, `LinearWall`, checks S1, C1 to C3, F1);
`crates/rcfd-fem/src/transient.rs` (whole); the head of `crates/rcfd-fem/tests/si_transient.rs`; `rcfd_fem::loads::nodal_load_vector`;
the `Cargo.toml` of `rcfd-post`. Nothing was built or run in Rust.

**Where the code lives, and the crate graph.** `coupled_step` and the traits `Participant`/`Structure`/`Fluid` are in `rcfd-fv`
(`rcfd_fv::cosim`); the integrator is in `rcfd-fem` (`rcfd_fem::transient`); `StaticSolid` and the stub `solid_dynamic.rs`
(declared by ADR 0291) are in `rcfd-post`, which already depends on `rcfd-core`, `rcfd-mesh`, `rcfd-io`, `rcfd-fv` and
`rcfd-fem`. **No dependency edge is added**: `DynamicSolid` goes in `rcfd-post/src/solid_dynamic.rs`, using `rcfd-fv` and
`rcfd-fem` through edges that exist. `rcfd-fv::cosim` is not changed (ADR 0290 section 2.4 item 6). The cosim case table,
the service and every golden are not touched.

**Arithmetic before this registration (disclosed).** One Python script, `w49i-work/f400-scratch/prearith.py` (numpy), on the 1D
reduction of the bar of D4 (exact, D5): the roots `k L = 1.076873986312` (ratio 0.5) and `0.480094436957` (ratio 4); with 20
linear elements and consistent mass `omega_h/omega = 1.000053286` and `1.000001900`; with the average-acceleration Newmark at
`dt = T/100`, `Omega/omega = 0.999724441` and `0.999673106` (errors `2.8e-4` and `3.3e-4` against the 1 % limit); the slope of the
plain iteration (D6) `-15.361635` (ratio 0.5) and `-59.692646` (ratio 4); the share of the higher discrete coupled modes in the
initial velocity `sin(k x)`: `8.0e-6` and `2.3e-7`; the spectral radius of the generalised-alpha method with `rho_inf = 0` at
`omega dt = 10, 100, 628, 1e4`: `0.24, 4.8e-2, 1.4e-2, 2.2e-3`. Nothing else was computed. The tests compute their own
predictions (D7); they do not use these printed numbers.

## Design

### D1. The held interface force in the integrator (the one library change outside `rcfd-post`)

The integrator of ADR 0389 takes loads as fixed patterns times tables, fixed when it is built. A participant receives a new
interface force at every advance, so `rcfd_fem::transient::Integrator` gains one opt-in method:

`run_with_force(steps, force)`: advances `steps` steps with, in addition to the load histories, the full force vector `force`
(one value per equation of the model, or per unknown on the transformation path; on that path a vector of the mesh equations is
also taken, as for load patterns) held over them, i.e. added to the load at both ends of every step: `f_n -> f_n + F`,
`f_{n+1} -> f_{n+1} + F`. Reduced like a load pattern: `F[free[i]]` on the elimination path, `T^T F` on the transformation path
(ADR 0393 D1). Refuses a wrong length or a non-finite value before any step; checks the coverage of the tables like `run`
(nothing is advanced on a refusal).

The step routine `advance` becomes `advance_with(extra: Option<&[f64]>)`; with `None` it executes the same floating-point
operations in the same order as today (the extra force is added to `f0` and `f1` only when it is given), so `run` and `step` and
everything that calls them keep their bits. This is a change to a shared solver file: the baseline hashes are run (S2).

### D2. `DynamicSolid` (`rcfd_post::solid_dynamic`)

`DynamicSolid::new(name, model, surface_set, loads, options, sub_steps)`: the interface is `SolidInterface::new(model,
surface_set)` (the set's nodes ascending, its faces' corners as interface elements, ADR 0286); the integrator is
`Integrator::new(model, loads, options)` (either path); the coupling step is `sub_steps` integrator steps (`sub_steps >= 1`).
It starts at rest at `t = 0` (`start(0, 0, 0)`); `start(t0, u0, v0)` restarts from full initial vectors (the initial
acceleration solves `M a0 = f(t0) - C v0 - K u0` with no interface force: at the start the fluid's force is the caller's
business, zero for a fluid at rest).

Behind `cosim::Structure`:

* `node_coordinates`: the interface nodes' coordinates; `elements`: the interface elements (the hook of F-444, so
  `InterfaceMap::build` and the case path can build weights from the two surfaces);
* `advance(dt, forces)`: refuses a `dt` that is not positive or differs from `sub_steps` times the integrator's step by more
  than `1e-9 dt`, a wrong count and a non-finite force, each naming the participant; otherwise the full vector `F` (the force of
  interface node `i` at the equations of its three components; a component without an equation or a prescribed one takes no
  force, its reaction is the constraint's) is held over the `sub_steps` steps (`run_with_force`), and the displacement of every
  interface node is returned (the full displacement at its three equations). An error of the integrator is returned with the
  participant's name; the integrator changes nothing on a refusal (D1);
* `time`: the integrator's time `t0 + n dt` (never accumulated);
* `checkpoint`/`restore`: the integrator's `TransientState` (its `to_bytes` holds every bit) and the interface forces of the last
  advance; `restore` gives both back bit for bit (`Integrator::restore` of the saved state).

Accessors: `interface()`, `displacement()`, `velocity()`, `acceleration()` (full vectors), `forces()` (last held), `state()`,
`n_sub_steps()`.

Why held: the protocol (`Structure::advance`) gives one force for the step. With Newmark (`alpha_f = 0`) only `f_{n+1}` enters the
equilibrium at `t_{n+1}`; at convergence the held force is the fluid's force at `t_{n+1}`, so the coupled pair is the implicit
Newmark step of the coupled system (D6). With the generalised-alpha method `alpha_f != 0` weighs the held force at both ends,
i.e. the force of `[t_n, t_{n+1}]` is the end value; this is first order in the coupling and is used here only for the slow
load (D8, where `alpha_f = 0` as well for `rho_inf = 0`).

### D3. The mock fluid (test-only, `si_solid_dynamic.rs`)

A wall of `n x n` square faces on `x = L` (points and faces as in F-385's `wall`), face areas `A_f`, `A = sum A_f`. It keeps per
wall point the displacement `d`, velocity `v` and acceleration `a` and integrates them with the same Newmark parameters as the
solid (`beta = 1/4, gamma = 1/2`) from the displacement it is given at the new time:

`a_{n+1} = (d_{n+1} - d_n - dt v_n - dt^2 (1/2 - beta) a_n)/(beta dt^2)`, `v_{n+1} = v_n + dt ((1 - gamma) a_n + gamma a_{n+1})`

(the Newmark displacement relation solved for the acceleration: a difference of displacements over the step corrected by the
stored velocity and acceleration, not a two-step finite difference, so no lag is added: D6). The force on face `f` is

`F_f = (A_f/A) (s(t) F_0 e_x - k_f d_f - m_a a_f)`, `d_f`, `a_f` the means over the face's points,

so that for a uniform motion of the wall the total is `s(t) F_0 - k_f u - m_a u''`. Mass runs: `F_0 = k_f = 0`; spring run:
`m_a = 0`. The mock checkpoints and restores `(t, d, v, a)` bit for bit and reports its time `t0 + n dt` from a step counter.

### D4. The set-ups

Bar: `hex_box([0,0,0], [L, W, W], [20, 2, 2])`, `L = 4 m`, `W = 1 m` (`A = 1 m^2`), Hex8, `E = 2e11 Pa`, `nu = 0`, `rho = 7850
kg/m^3`, all three components fixed on `xmin` (a clamp; symmetric about the bar's axis, D7), interface the surface set `xmax`
(a 2 x 2 face, 9 nodes). Wave speed `c = sqrt(E/rho) = 5047.5 m/s`.

Maps (F-362): `SolidInterface::build_map(points, faces, MapSettings::projection(1e-9))`, matching: fluid wall 2 x 2 (the same 9
points, listed in another order), 3:2: fluid wall 3 x 3 (16 points, 9 faces). The projection reproduces rigid motions (ADR 0263),
so a uniform displacement of the nodes is a uniform displacement of every wall point in both maps.

Mass runs (clauses c1 to c4, c7): `m_a = r rho A L`, `r = 0.5` and `4`; `k L` the first root of `k L tan(k L) = 1/r`
(bisection on `(0, pi/2)` in the test); `omega = k c`, `T = 2 pi/omega`, coupling step `dt = T/100`, `sub_steps = 1`, the
average-acceleration Newmark method, undamped. Initial state: `u0 = 0`, `v0 = V sin(k x) e_x` at every node (`V = 1 m/s`; zero
on `xmin`), the mock at `d = 0`, `v = V sin(k L) e_x` (the map of the nodes' velocity: uniform), `a = 0`; then `M a0 = 0`, so
the solid and the mock start with zero acceleration and the fluid's force at `t = 0` is zero. Run 320 coupled steps (3.2
periods). Settings: `CouplingSettings::aitken(0.5)` and `CouplingSettings::least_squares(0.5)` (tolerance `1e-8`, no floor),
with the cap of check I below and `AtCap::Stop`.

Frequency: the signal is the mean over the interface nodes of the `x` displacement at the end of each step; its zero crossings
after `t = 0` by linear interpolation between steps (six: at about `T/2, T, ..., 3T`); `T_m = (t_last - t_first)/(N_c - 1) * 2`,
`Omega_m = 2 pi/T_m`.

Spring run (clause c5): the same bar, matching map, `k_s = E A/L = 5e10 N/m` (exact for `nu = 0`, D5), `k_f = 1.5 k_s`,
`F_0 = 1e6 N`, `u* = F_0/(k_s + k_f) = 8e-6 m`; the generalised-alpha method with `rho_inf = 0`, coupling step `dt = 0.3 s`
(about 95 periods of the first fixed-free mode, `omega dt >= 594`), `sub_steps = 1`; slow load `s(t) = min(t/(5 dt), 1)`; 15
coupled steps (5 of ramp, 10 held), settings `aitken(0.5)` and `least_squares(0.5)`.

### D5. Why the bar is one-dimensional (exact for the uniform axial field)

With `nu = 0`, `lambda = 0` and the strain energy is `mu eps:eps` per volume. For the nodal field `u = (g(x), 0, 0)` (the same
value on every node of a cross-section) the trilinear interpolation gives `u_x` depending on `x` alone, so the only strain is
`eps_xx = g'` and the shear strains vanish; the internal force on the `y` and `z` unknowns is `int mu (du_x/dy + du_y/dx) dN/dx`
and similar, zero because `du_x/dy = du_z/dy = 0`. The consistent mass has no coupling between components. Hence the uniform
axial field is an exact solution of the discrete equations under loads that are uniform over the cross-section (consistently
distributed), and its equations are those of the 1D linear bar with `A` times the 1D element matrices (`E A/h [1 -1; -1 1]`,
`rho A h/6 [2 1; 1 2]`). The clamp `u = 0` on `xmin` holds for that field. The axial stiffness is `k_s = E A/L` exactly.

Matching map: `H` is the identity (each wall point is a node), `P` takes the mean over the four points of a face, so the
transpose gives each node a quarter of the force of each face it belongs to, which for a uniform force per area is the
consistent load of a bilinear face (`A_f/4` per corner). The mock's force for a uniform wall motion is uniform per area.
So in the matching run the coupled motion stays in the uniform axial field up to rounding.

### D6. The coupled system, its frequency and the plain iteration's slope

Matching map, uniform field: the solid advances `S a_{n+1} = r_n + F e` (D1), `S = M + beta dt^2 K` (Newmark, `c_M = 1`,
`c_K = beta dt^2`), `e` the tip unknown of the 1D chain of D5, `F` the total held force. The mock's `a_{n+1}`, from the same
Newmark relation with the same stored state (the mock's `(d, v, a)` equal the tip's `(u, v, a)` at the start of each step by
induction: the same recurrence on the same numbers, `H` the identity), is the tip's `a_{n+1}`. So the converged step solves
`(M + m_a e e^T + beta dt^2 K) a_{n+1} = ...`: exactly the Newmark step of the bar with the mass `m_a` at its tip. Its discrete
frequency is `Omega = (2/dt) atan(omega_h dt/2)` (average acceleration, undamped: the amplification matrix has eigenvalues of
modulus 1 and phase `2 atan(omega_h dt/2)` per step), `omega_h` the first eigenvalue of `K phi = omega^2 (M + m_a e e^T) phi`
of the 20-element chain. The continuous root is the limit `h, dt -> 0`. Errors (arithmetic): spatial `+5.3e-5` and `+1.9e-6`,
temporal `-3.3e-4`; together `-2.8e-4` (ratio 0.5) and `-3.3e-4` (ratio 4).

The fixed-point map of `coupled_step` within a step is affine in the held force: `ft = b + s F`, with `s = -m_a e^T S^{-1} e`
(the force changes the tip acceleration by `e^T S^{-1} e` per newton and the mock returns `-m_a` times that). The plain
iteration (`w = 1`) gives `r_{k+1} = s r_k` exactly in the uniform field, so it diverges for `|s| > 1`, oscillating. This is the
protocol's statement ("a change of `f` changes `ft` by minus [the added-mass ratio's share]", the plain iteration diverges when
that slope exceeds 1 in modulus); the protocol's figure `ratio/2` is for its own pairing (a constant-force structure and a flow
whose wall velocity is the displacement over the step), here the slope is `-m_a e^T S^{-1} e` of the Newmark pair, computed in
the test from the 1D chain: `-15.36` (ratio 0.5) and `-59.69` (ratio 4) by the arithmetic. Large because the structural mass
that answers within one step is the part a wave reaches in `dt` (`c dt = 0.13 L` at ratio 4), not `rho A L`.

Aitken (ADR 0250 D6) on an affine scalar map: from `f^2 = f^1 + 0.5 r^1` the factor `-0.5 r^1 (r^2 - r^1)/|r^2 - r^1|^2 = 1/(1 -
s)` is the exact secant, so `r^3` is rounding: three iterations per step in the matching runs. The least-squares update (ADR
0261 D-derivation: exact at iteration `d + 2` on a linear map, `d` the dimension reached) with `d = 1`: three iterations.

3:2 map: the transpose of the projection does not give the consistent pattern of the 2 x 2 face exactly, so non-uniform
patterns are excited. Both meshes and the clamp are symmetric under the eight symmetries of the square about the bar's axis and
the projection keeps them (no fluid point lies on a solid face edge except on the boundary of the face, where the edge is the
face's own), so the iteration stays in the symmetric subspace of the 27 interface forces: axial forces at corners, edge
midpoints, centre (3) and in-plane radial forces at corners and edge midpoints (2): `d <= 5`, the least-squares update ends by
iteration 7. Aitken's scalar factor has no finite termination on a map with several eigenvalues: no tighter cap than the
default 20 is derived. The non-uniform loads are self-equilibrated corrections at the tip of a bar of slenderness 4: they
change the frequency to second order; the estimate is below `1e-5` (not derived further; the acceptance limit `1e-3` is used).

### D7. What the tests compute themselves

The root `k L` (bisection, 200 halvings), the 1D chain's `omega_h` (inverse iteration on the tridiagonal pencil, 200
iterations, Rayleigh quotient), `Omega = (2/dt) atan(omega_h dt/2)`, the slope `s = -m_a e^T S^{-1} e` (tridiagonal solve). No
number of the arithmetic above is pasted into a test.

### D8. The slow load

Generalised-alpha with `rho_inf = 0`: `alpha_m = -1`, `alpha_f = 0`, `beta = 1`, `gamma = 3/2`; at `omega dt >= 594` the spectral
radius of every mode is at most `1.4e-2` (arithmetic), so 10 held steps reduce the dynamic part by `< 1e-18`. At rest the
equilibrium is `K u = F_0 - k_f u` exactly. The coupled iteration's slope there is about `-k_f/k_s = -1.5` (the displacement
answers the force statically at this step), one-dimensional with the matching map: Aitken and least squares in 3 iterations.
The converged force residual `<= 1e-8 |ft|` moves `u` by at most `1e-8 |s|/(1 + |s|) u*`: below `1e-8`. Estimate of the
deviation from `u*`: `<= 2e-8`.

## Registered checks

Test file `crates/rcfd-post/tests/si_solid_dynamic.rs` (the stub's name), plus one test of `run_with_force` in the same file.
Every value is printed; a test fails at the end if any of its checks missed. "Relative" errors are against the named reference.

### A. The acceptance

| Id | Clause | Quantity | Limit | Estimate |
|---|---|---|---|---|
| F1 | c1 | `abs(Omega_m/omega - 1)`, ratio 0.5, matching, Aitken | `<= 1e-2` | `2.8e-4` |
| F1b | c1 | the same with the least-squares update | `<= 1e-2` | `2.8e-4` |
| F2 | c2 | `abs(Omega_m/omega - 1)`, ratio 4, matching, Aitken | `<= 1e-2` | `3.3e-4` |
| F2b | c2 | the same with the least-squares update | `<= 1e-2` | `3.3e-4` |
| P1 | c3 | ratio 4, matching, `Relaxation::Fixed(1.0)`, cap 20, `Stop`, first step from the initial state: the step fails, participant `coupler`; and, from the same initial state with `AtCap::Keep` and cap 5, `max_k abs(r^{k+1}/r^k / abs(s) - 1)` over `k = 1..4` (the step's residual norms), `s` of D6 | fails; `<= 1e-6` | `1e-12` |
| I1 | c4 | ratio 4, matching: every one of the 320 steps converged to `1e-8`, iterations per step at most the cap: Aitken | cap 4 | 3 |
| I1b | c4 | the same, least squares | cap 4 | 3 |
| I2 | c4 | ratio 4, 3:2: every step converged to `1e-8`; Aitken | cap 20 | none derived |
| I2b | c4 | ratio 4, 3:2: least squares | cap 8 | `<= 7` |
| K1 | c5 | spring run, Aitken: after 15 steps `max abs(q_x/u* - 1)` over the interface nodes and over the wall faces' displacements (the map of the nodes'), and `max(abs(q_y), abs(q_z))/u*` | `<= 1e-6` each | `2e-8` |
| K1b | c5 | the same, least squares | `<= 1e-6` each | `2e-8` |
| M1 | c6 | built maps, matching and 3:2: with random face forces (`SplitMix64`, seed fixed in the test, components in `[-1e4, 1e4]` N) and random nodal displacements (`[-1e-3, 1e-3]` m): resultant, moment about the origin and work, fluid side (faces, the face means of the mapped points) against solid side (`forces_to_nodes` on the nodes), each relative to its scale as in F-385 | `<= 1e-12` each | `1e-16` |
| M1r | c6 | the same three in the runs I1 (matching) and I2 (3:2) with the last step's face forces and node displacements | `<= 1e-12` each | `1e-16` |
| M2 | c7 | `abs(Omega_3:2/Omega_matching - 1)`, ratio 4, Aitken (runs I2, F2) and least squares (I2b, F2b); ratio 0.5, Aitken (run F1 against a 3:2 run of the same set-up, cap 20) | `< 1e-3` each | `< 1e-5` |
| R1 | c8 | `DynamicSolid`: after one advance with random forces, checkpoint; an advance with other forces changes the state; restore gives the state (`TransientState::to_bytes`, time bits, held forces' bits) bit-equal to the checkpoint; the advance repeated from it gives the same returned displacements and state bit for bit; a second restore again the checkpoint | all hold | holds |
| FS1 | c9 | a load history whose table covers `[0, 3 dt]` (zero pattern): steps 1 to 3 converge, step 4 fails with participant `solid`, iteration 1, the message naming the load table; mock and solid bit-equal to before step 4 | all hold | holds |
| FS2 | c9 | a wrapper around the solid that passes a non-finite force to it at its second advance of a step: the step fails with participant `solid`, iteration 2 (after the solid advanced once in it); both sides bit-equal to before the step | all hold | holds |
| NF | c10 | no flow solver: the test file uses no `FlowWall` and no `PimpleSolver` (by inspection, stated in the addendum) | holds | holds |

### C. Companions

| Id | Quantity | Limit | Estimate |
|---|---|---|---|
| F1p, F2p | `abs(Omega_m/Omega_pred - 1)`, `Omega_pred` of D6/D7, runs F1 and F2 (Aitken) | `<= 2e-5` | `1e-6` (crossing interpolation `7e-7`, higher modes `5e-7`) |
| P2 | ratio 0.5, matching, `Fixed(1.0)`: `abs(r^2/r^1/abs(s) - 1)` | `<= 1e-6` | `1e-12` |
| I3 | ratio 0.5: matching, Aitken and least squares each at most 4 iterations per step; 3:2, Aitken at most 20; every step converged | caps | 3; none |
| K2 | spring runs: every step at most 4 iterations | cap 4 | 3 |
| A1 | `DynamicSolid` with the generalised-alpha method `rho_inf = 0.8` (`alpha_f != 0`) and `sub_steps = 2`, three advances with random forces: after each, the state is bit-equal to an `Integrator` of the same model restored to the previous state and run 2 steps with one `LoadHistory` whose pattern is the full force vector (built in the test from `dof_map().equation`) and whose table is constant 1; the returned displacements are the state's at the interface equations; the time is `t0 + 2 n dt_int` | bit-equal | holds |
| A1b | the same on the transformation path (the bar with a nodal spring support on `xmin` in `x` and rollers on `ymin`, `zmin`, as in `si_transient_mpc.rs`) | bit-equal | holds |
| D1 | `run_with_force(n, zero vector)` against `run(n)` on a bar with a positive load table: the state bit for bit | bit-equal | holds |
| Z1 | refusals: unknown surface set, `sub_steps = 0`, an advance with a step other than `sub_steps dt_int`, a wrong count of forces, `run_with_force` with a wrong length and with a non-finite value; each names the participant or the key | all hold | holds |

### N. Negative controls

* P1 and P2 are the registered diverging controls (plain iteration).
* N1 (record, not a pass condition of the acceptance): a lagging mock that takes the acceleration as the two-step difference
  `(d_{n+1} - 2 d_n + d_{n-1})/dt^2` instead of D3, ratio 4, matching, Aitken, 320 steps: the frequency error against
  `Omega_pred` is printed and must exceed the `2e-5` of F2p (it shows that F2p can see a lag); limit `>= 2e-5`. Estimate: the
  backward difference is centred at `t_n`, half a step late on average: a phase lag that changes the frequency by a quantity
  of order `(omega dt)^2` (`4e-3`) in the added-mass share; not derived further.

### S. Existing tests (unedited, release)

* S1: `cargo test -p rcfd-fem --release --test si_transient --test si_transient_mpc` (the lines changed by D1) and the
  library's unit tests of `rcfd-fem` (`--lib`).
* S2: `cargo test -p rcfd-api --release --test baseline_hashes` (shared solver file changed).
* S3: `cargo test -p rcfd-post --release --test sb_solid_coupling --test sb_solid_coupling_fail --test
  sb_solid_coupling_fail_fluid` (the neighbours in `rcfd-post`; no line of theirs changes).

### M. Planned mutations (each alone, on a `git archive` copy of the code commit in the scratch folder, after the registered run)

* MU1: `run_with_force` maps the force by `free` also on the transformation path (no `T^T`): expected caught by A1b.
* MU2: the held force added to `f_{n+1}` only: expected caught by A1 (`alpha_f != 0`).
* MU3: `DynamicSolid::restore` does not restore the held forces: expected caught by R1.
* MU4: `advance` runs one integrator step whatever `sub_steps`: expected caught by A1 (and the time check).
* MU5: the interface displacement taken from component `(c + 1) mod 3`: expected caught by F1/F2 and K1.
* MU6: the force on the interface node's `y` component dropped: expected caught by A1.
* MU7: `checkpoint` saves nothing new after the first call (keeps the first checkpoint): expected caught by R1 or FS1.

## Decision rule

* **done**: every check of A met and every companion of C met, S1 to S3 pass, N1 shows the lag (`>= 2e-5`).
* **partial**: any check of A missed (recorded as `*_known_miss`, limit intact) or not run; or a companion missed with a diagnosed
  cause; or N1 fails to show the lag (then F2p's power to see a lag is unproven).
* **todo**: no coupled run possible.

## Limits (what is not covered)

* No flow solver (ADR 0304). Mock fluids only: an added mass with Newmark-consistent kinematics, a spring with a slow load.
* The held force with the generalised-alpha method is first order in the coupling (D2); the frequency clause uses Newmark.
* Sub-steps with a mass mock: the mock integrates over the coupling step, the solid over sub-steps; their kinematics then differ
  (not checked; `sub_steps = 1` in every coupled run).
* Shell interfaces, rotations at interface nodes, Tet10 mid-edge nodes (ADR 0286 Q2), radial-basis maps in a coupled dynamic
  run: not covered.
* No case key, CLI, MCP tool or service call; `rcfd_api::solid_coupled` (ADR 0290 section 2.5 item 5) is not written.
* The initial acceleration ignores an interface force at `t0` (D2).

## Open points taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. Where the held force enters the integrator: a new method `run_with_force` (recommended, D1) rather than rebuilding an
   integrator with a constant load history at every advance (which refactors the effective matrix each time and keeps
   `rcfd-fem` untouched). Taken: the method.
2. The mock's acceleration: Newmark-consistent (D3, no lag, the coupled pair is exactly Newmark) rather than a two-step finite
   difference (lag; kept as the negative control N1). Taken: Newmark-consistent.
3. The bar's end: clamped (`xmin`, three components) rather than the rollers of F-385, to keep the symmetry of D6 for the 3:2
   map; for the uniform axial field the two are the same (D5).

## Amendment 1 (2026-11-13, written after the registered run of commit 25725152 and before the corrected run)

**What the registered run showed.** The registered run (log `f400-scratch/run1.log`, made once) passed A1, A1b, D1, M1, R1, FS2,
Z1, N1 (lag `7.3e-4`, limit `>= 2e-5`) and, inside the failing test functions, P1 (the step fails at the cap; growth `/ |s| - 1 =
5.6e-16`) and P2 (`1.2e-15`); every coupled mass run, the spring runs and FS1 stopped after two to four steps: matching runs at the
cap 4 with test quantities `6e-8` to `1.4e-3`, the 3:2 runs in their first step at the caps 20 (Aitken) and 8 (least squares),
the spring runs in step 2 at the cap 4. The tests' results are in the addendum; their limits stand.

**Diagnosis (one diagnostic print of the first six steps, matching, ratio 4, Aitken, cap 12, `AtCap::Keep`, not committed).**
The secant step leaves the residual at `7.8e-13`, `5.5e-10`, `1.0e-6` in steps 1, 2, 3: a growth of about 700 per step, from
rounding, in the patterns of the interface forces that are not uniform (the corner forces of step 5 differ by `5e-9` relative).
Cause: the mock of D3 has an added mass per face (`m_a A_f/A` times the face's own acceleration), so its added-mass matrix is
`B^T diag(m_f) B`, of rank 4 on the matching wall and 9 on the 3:2 wall, and the non-uniform face patterns have slopes of the
fixed-point map of several hundred (a non-uniform tip load reaches only the elements next to the face within a step). A component
of the force below the convergence test (`1e-8` of the total) is not iterated: it is loosely coupled, and a loosely coupled
added-mass component grows by its slope per step (the artificial added-mass instability). The 1-D argument of D5/D6 holds for the
uniform field but not for the rounding outside it. The same holds for the per-face springs of the spring run, and the 3:2 runs
excite these patterns at once (several eigenvalues of hundreds: Aitken's scalar factor cannot converge, the least-squares update
needs more than 8 columns). It is a defect of the mock's set-up, not of `DynamicSolid` (A1 and A1b are bit-equal to the
integrator).

**Correction (the set-up only; every limit unchanged).** The mock becomes a piston, as F-385's `LinearWall`: one wall acceleration
and one wall displacement, the area-weighted means of the faces' `x` components, `a_bar = sum_f (A_f/A) a_f,x`, `d_bar` likewise,
and the force `F_f = (A_f/A) (s(t) F_0 - k_f d_bar - m_a a_bar) e_x` on every face (uniform per area, axial). The points'
kinematics are still the Newmark relation of D3 (so `a_bar` is the Newmark acceleration of `d_bar`: the relation is linear). The
added-mass matrix is `m_a w w^T`, `w = B^T (A_f/A)_f e_x`, of rank 1 on both walls; the fixed-point map has the one non-zero
slope `s = -m_a w^T S^{-1} w` (on the matching wall `w` is the consistent pattern of D5, so `s` is the slope of D6, computed by the
test as before); every iterate's force lies in `span{w}` up to rounding, and the rounding outside it is mapped into `span{w}`
(not amplified). Aitken and the least-squares update then end in iteration 3 on both walls, and the coupled pair is exactly
Newmark's method on `M + m_a w w^T` (stable). This is the reading of the acceptance's "the force -m_a times the wall
acceleration" (one acceleration of the wall), taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation; the alternative, a per-face added mass with a tolerance below rounding, cannot converge).

**Corrected tests** (new functions in `si_solid_dynamic.rs`; the four registered functions that missed stay as ignored records,
unedited except for the `#[ignore]` attribute; the registered tests that passed stay as they are): `f2_..._piston`,
`f1_..._piston`, `fs1_..._piston`, `k1_k2_..._piston`, `n1_..._piston` with the same checks, ids suffixed `c` (`F2c`, `I1c`, ...),
the same limits and caps, and the same estimates except: the 3:2 caps of I2 (20) and I2b (8) stand, the estimate is now 3
iterations; M2's estimate stands (`< 1e-5`). N1 stands as registered (met); `N1c` repeats the lag control on the piston set-up
(limit `>= 2e-5`). FS2 (met) stands; it is not repeated. Each corrected run is made once.

## Amendment 2 (2026-11-13, after the mutation runs and before the run of the check it adds)

Mutation MU1 (`run_with_force` maps the force by `free` on the transformation path instead of `T^T`) was not caught: on the
model of A1b (a nodal spring support and rollers) `T` is a selection of the independent unknowns, so `T^T F = F[free]` and the
mutant is equivalent there. Added check **A1c**: A1 on the bar clamped on `xmin` with a rigid set on `xmax`
(`Constraint::rigid_set("xmax", [L, W/2, W/2])`, as in `si_transient_mpc.rs`), so that the interface nodes are dependent and `T^T`
moves their forces to the reference's six unknowns: the state after each of three advances bit-equal to the integrator with a
constant load history, the returned displacements the state's, the time `t0 + 2 n dt`. Limit: bit-equal. Expected to catch
MU1. Run once on the unmutated code, then MU1 once more.

## Addendum 1: results (2026-11-13)

Runs, each once, release profile, `CARGO_BUILD_JOBS=4`: the registered run of commit `25725152` (log `f400-scratch/run1.log`);
the corrected run of amendment 1, commit `61369116` (`cargo test -p rcfd-post --release --test si_solid_dynamic piston`, log
`f400-scratch/run2.log`); A1c of amendment 2, commit `20d467f0`. The whole test file takes 0.25 s; no test is heavy. Commit
`58d9d803` moved the missed M2c into an ignored `*_known_miss` test (limit intact) and applied `cargo fmt` and one clippy fix
(an array instead of `vec!` in the test's 1D chain); the file was then run as a whole to check that it passes: 13 passed, 5
ignored (the four records of the registered run and the known miss).

### Checks: the registered run (per-face added-mass mock, D3)

| Id | Limit | Measured | Met |
|---|---|---|---|
| F1, F1b, F2, F2b, F1p, F2p | 1 %; `2e-5` | not measured: every mass run stopped at step 2 to 5 | no (set-up defect, amendment 1) |
| I1, I1b | cap 4, all converged | stopped at step 3 to 5 at the cap (test quantity `6.9e-8` to `1.4e-3`) | no |
| I2, I2b | caps 20, 8 | stopped in step 1 (test quantity `3.4e-2`, `1.06`) | no |
| I3 | caps | stopped (matching at the cap 4, 3:2 in step 1) | no |
| P1 | fails at the cap; `<= 1e-6` | fails (`coupler`, iteration 20); growth `/ abs(s) - 1 = 5.6e-16` (`s = -59.692646`) | yes |
| P2 | `<= 1e-6` | `1.2e-15` (`s = -15.361635`) | yes |
| K1, K1b | `<= 1e-6` | stopped in step 2 at the cap 4 | no |
| K2 | cap 4 | 3 in the steps made | yes (steps made) |
| M1 | `<= 1e-12` | resultant `<= 1.3e-16`, moment `<= 4.9e-17`, work `<= 9.5e-17` (matching and 3:2, three trials each) | yes |
| M1r | `<= 1e-12` | matching `2.5e-26`, `2.6e-17`, `1.4e-16`; 3:2: no step made | matching yes; 3:2 no |
| M2 | `< 1e-3` | not measured | no |
| R1 | bit for bit | holds | yes |
| FS1 | located, restored | step 3 stopped at the cap (`coupler`), so the load table's end was not reached; the restore held | no |
| FS2 | located, restored | `solid`, iteration 2, "a non-finite interface force"; both sides bit-equal | yes |
| A1, A1b, D1, Z1 | bit for bit; refusals | all hold | yes |
| N1 | `>= 2e-5` | `7.28e-4` (the lagging per-face mock converged in every step, at most 11 iterations) | yes |

### Checks: the corrected run (piston mock, amendment 1) and A1c (amendment 2)

| Id | Limit | Measured | Met |
|---|---|---|---|
| F1c, F1bc (ratio 0.5, `k L = 1.076874`) | `abs(Omega_m/omega - 1) <= 1e-2` | `2.757e-4`, `2.757e-4` | yes |
| F2c, F2bc (ratio 4, `k L = 0.480094`) | `<= 1e-2` | `3.270e-4`, `3.270e-4` | yes |
| F1pc, F2pc | `abs(Omega_m/Omega_pred - 1) <= 2e-5` | `1.20e-7`, `1.46e-7` | yes |
| I1c, I1bc (ratio 4, matching) | cap 4, all 320 steps converged to `1e-8` | 3 in every step (mean 3.000), Aitken and least squares | yes |
| I2c, I2bc (ratio 4, 3:2) | caps 20 and 8 | 3 in every step, both | yes |
| I3c (ratio 0.5) | caps 4, 4, 20 | 3 in every step | yes |
| P1c | fails at the cap 20; `<= 1e-6` | fails (`coupler`); `5.6e-16` | yes |
| P2c | `<= 1e-6` | `1.2e-15` | yes |
| K1c, K1bc | `<= 1e-6` | `abs(q_x/u* - 1)` `1.07e-10`, `1.07e-10`; `q_yz/u*` `3.1e-14`, `3.6e-14` | yes |
| K2c | cap 4 | 3, 3 | yes |
| M1rc | `<= 1e-12` | matching `2.2e-16`, `0`, `1.5e-16`; 3:2 `0`, `2.0e-17`, `0` | yes |
| **M2c** | `< 1e-3` | **`6.279e-3`** (ratio 4, Aitken), **`6.279e-3`** (ratio 4, least squares), **`2.377e-3`** (ratio 0.5, Aitken); the 3:2 frequency is the lower one | **no** (`m2c_frequency_change_known_miss`) |
| FS1c | located, restored | `solid`, iteration 1, "key 'load table': table 0 covers [0e0, 3.63e-4], the run needs [3.11e-4, 4.15e-4]"; both sides bit-equal | yes |
| N1c | `>= 2e-5` | `7.28e-4` | yes |
| A1c | bit for bit | holds (three advances) | yes |
| NF | no flow solver | the test file uses neither `FlowWall` nor `PimpleSolver` (inspection) | yes |

The measured frequencies agree with the discrete prediction of D6 (`Omega_pred/omega = 0.999724441` and `0.999673106`, computed
in the test) to `1.5e-7`: the matching coupled pair is Newmark's method on the bar with its tip mass, as derived. The plain
iteration's growth is the slope `-m_a e^T S^{-1} e` to `6e-16`.

### Diagnosis of M2c

The transpose of the projection map gives the 2 x 2 solid face the axial weights `w = B^T (A_f/A)`: corners `0.102, 0.065,
0.065, 0.102`, edge midpoints `0.111`, centre `0.222`, against the consistent `0.0625, 0.125, 0.25` of the matching wall. One
diagnostic static solve (not committed): `w^T K^{-1} w = 1.01489 L/(E A)` on the 3:2 wall, `1.00000` on the matching wall. The
added mass loads the face with `w`, and the wall's mean displacement under that load includes the local deformation of the
face: 1.49 % more compliance in series. With the added mass dominant (ratio 4, `k L = 0.48`, nearly a mass on a spring) the
frequency falls by about half of that, `6.3e-3` measured; at ratio 0.5 less of the mode's energy is in that compliance,
`2.4e-3`. My estimate of D6 (`< 1e-5`, "second order in a self-equilibrated correction") was wrong: the correction is second
order but not small (the weights differ by up to 63 % at the corners), and the compliance of a self-equilibrated load on a 2 x 2
face is not small against `L/(E A)`. F-385 had measured the same effect statically (largest error on the loaded 3:2 face `0.131
p L/E`). The weights are also not symmetric under the square's mirror about the diagonal through the corners `(0, 0)` and
`(1, 1)` (corners `0.102` against `0.065`): the projection with its offset correction does not keep that symmetry on this wall
(observation; resultant, moment and work are still conserved, M1, M1rc).

### Mutations (copy of commit `58d9d803` from `git archive` in `f400-scratch/copy`, own target directory, script `mutate.py`, logs `MU1.log` to `MU7.log`)

| Mutation | Caught by |
|---|---|
| MU1 `T^T` replaced by `F[free]` on the transformation path | **not caught** on the first run (`MU1_first.log`): equivalent on A1b's model, where `T` is a selection; amendment 2 added A1c; rerun with A1c's test file (commit `20d467f0`): caught by A1c |
| MU2 held force added to `f_{n+1}` only | A1 |
| MU3 `restore` leaves the held forces | R1, FS2 |
| MU4 one integrator step per advance | A1 |
| MU5 interface displacement from component `(c + 1) mod 3` | F1c/F2c tests, K1c, N1, N1c, A1, FS2 |
| MU6 the `y` force of every interface node dropped | A1 |
| MU7 `checkpoint` keeps the first checkpoint | FS1c, FS2, F1c/F2c tests, K1c, N1, N1c |

### Existing tests (S)

* S1: `cargo test -p rcfd-fem --release --lib --test si_transient --test si_transient_mpc`: 1, 11 (3 ignored), 6 passed.
* S2: `cargo test -p rcfd-api --release --test baseline_hashes`: 3 passed, 1 ignored (the known `rotating_channel` record, as
  on main).
* S3: `cargo test -p rcfd-post --release --test sb_solid_coupling --test sb_solid_coupling_fail --test
  sb_solid_coupling_fail_fluid`: 11 (1 ignored), 3, 5 passed.
* `cargo fmt -p rcfd-post -p rcfd-fem --check`: clean. `cargo clippy -p rcfd-fem -p rcfd-post --all-targets --release -- -D
  warnings`: clean.

### Deviations

* Amendment 1 changed the mock (per-face added mass to a piston) after the registered run; the four test functions of the
  registered run that missed are kept as ignored records, unedited but for the attribute (the helpers gained a `piston` flag,
  false for them).
* The checks met in the registered run (P1, P2, M1, R1, FS2, A1, A1b, D1, Z1, N1) stand from that run; P1, P2 and N1 were
  repeated on the piston set-up as P1c, P2c and N1c by the amendment.
* Amendment 2 added A1c after the mutation runs (MU1 was equivalent on A1b's model).
* M2c is the only miss of the corrected run; the two test functions that contained it print its values, and the check itself
  is the ignored `m2c_frequency_change_known_miss`.
* `docs/cosimulation-protocol.md` gains a paragraph on `DynamicSolid` and the observation of amendment 1.

### What was not run

* The whole workspace test suite (the lead's gate). The radial-basis map in a coupled dynamic run. Sub-steps with a mass mock.
* An estimate of the frequency change from the 3:2 weights before the run (D6 said "not derived further"; it was wrong).

### Decision by the registered rule

**Partial.** On the corrected set-up every clause of the acceptance is met except c7: the coupled frequency changes by `6.3e-3`
(ratio 4) and `2.4e-3` (ratio 0.5) between the matching and the 3:2 map, limit `1e-3`, a diagnosed miss. The registered set-up
also needed an amendment. Met: c1 and c2 (`2.8e-4` and `3.3e-4` against 1 %), c3 (the plain iteration diverges with the
predicted slope to `6e-16`), c4 (3 iterations per step, caps 4, 8 and 20), c5 (`1.1e-10` against `1e-6`), c6 (`2.2e-16` against
`1e-12`), c8 and c9 (bit for bit), c10.

### Remaining questions

1. **The frequency change between the maps (c7).** Options: (a) keep the clause and add a follow-up that measures the change
   against the refinement of the solid face (2 x 2, 4 x 4, 8 x 8 elements across, the 3:2 ratio kept) and with the radial-basis
   map, the limit registered from the static compliance `w^T K^{-1} w` computed beforehand; (b) restate the clause as "the change
   equals the one predicted from the static compliance of the mapped weights within 10 %"; (c) accept `1e-2` for a 2 x 2 face.
   Recommendation: (a), the follow-up below; it also tells whether the `1e-3` of the acceptance is reachable with a projection.
2. **The projection's weights are not mirror-symmetric on the 3:2 wall** (corners 0.102 and 0.065). Options: (a) a check in the
   F-362 builder (is the offset correction's ring order-dependent?); (b) leave it (conservation holds). Recommendation: (a),
   folded into the follow-up of question 1.
3. **A distributed added mass and the convergence test** (amendment 1): with a fluid whose added mass acts per face, the force
   components below the convergence test are loosely coupled and grow by their slope per step (here about 700). ADR 0304
   (F-401) couples the flow solver, whose added mass is distributed. Options: (a) F-401 registers a check of the growth of the
   non-uniform part of the interface force over the run; (b) a convergence test per pattern in the coupler (changes
   `rcfd-fv::cosim`); (c) nothing until it shows. Recommendation: (a), as a sentence in F-401's note.

### For the lead: roadmap entries

F-400, proposed status **partial**, note:

> Structural solver, intermediate tier (plan ADR 0290). PARTIAL (ADR 0410, 2026-11-13; registered before the code, amendment 1 after the registered run, amendment 2 after the mutations). rcfd_post::solid_dynamic::DynamicSolid implements cosim::Structure on the transient integrator of ADR 0389/0393 (Newmark or generalised-alpha, either path): advance holds the interface forces over one or more sub-steps through the new opt-in Integrator::run_with_force (the force added at both ends of every step, reduced like a load pattern; run and step keep their bits), checkpoint and restore keep the integrator's state and the held forces bit for bit, elements gives the surface set's faces to the map builders; no dependency edge added (rcfd-post already depends on rcfd-fv and rcfd-fem), rcfd-fv::cosim unchanged. Bar 4 x 1 x 1 m, Hex8 20 x 2 x 2, nu = 0, clamped, end face the interface, coupled by coupled_step to a test-only piston mock (one wall acceleration, Newmark-consistent, force -m_a a) at 100 steps per period: first coupled frequency against the root of k L tan(k L) = rho A L/m_a 2.8e-4 (ratio 0.5) and 3.3e-4 (ratio 4) (limit 1 %), against the discrete Newmark prediction 1.5e-7; the plain iteration diverges with the derived slope -m_a e^T S^{-1} e (-59.69 at ratio 4) to 6e-16; Aitken and least squares converge to 1e-8 in 3 iterations per step on the matching and the 3:2 map built by F-362; spring 1.5 k_s with a slow load (generalised-alpha rho_inf = 0) to F_0/(k_s + k_f) within 1.1e-10; resultant, moment and work conserved to 2.2e-16; restore bit for bit; a failing structural step (load table, non-finite force) stops the step naming 'solid' in iteration 1 and 2 with both sides restored. Partial: the coupled frequency changes by 6.3e-3 (ratio 4) and 2.4e-3 (ratio 0.5) between the matching and the 3:2 map (limit 1e-3; diagnosed: the transposed projection loads the 2 x 2 face with non-consistent weights, 1.5 % more static compliance; a known-miss record). The registered per-face added-mass mock made the coupled run unstable (non-uniform patterns with slopes of hundreds below the convergence test grow by about 700 per step); amendment 1 made the mock a piston. Eight mutations caught (one after amendment 2). Baseline hashes pass. No flow solver is coupled. Open: ADR 0410 Q1 to Q3. Follow-up: F-NEW (frequency change against the face refinement and the map).

F-401, sentence to append to its note:

> ADR 0410 (F-400, 2026-11-13) amendment 1: with an added mass that acts per wall face, the interface force components below the convergence test are loosely coupled and grew by about 700 per step on the bar of F-400; the registration of this item should check the growth of the non-uniform part of the interface force over the run (ADR 0410 Q3).

Follow-up item (new id to be given by the lead), title "Coupled frequency of the dynamic solid on non-matching interfaces: face refinement and map", deps F-400, F-362, acceptance:

> Every limit not stated here is registered in the ADR before the run. On the bar of F-400 (ADR 0410) with the piston mock at mass ratio 4, the change of the first coupled frequency between the matching and the 3:2 interface is measured for solid end faces of 2 x 2, 4 x 4 and 8 x 8 elements with the projection map and with the radial-basis map; each change equals the one predicted from the static compliance w^T K^{-1} w of the mapped weights (computed before the run) within 10 percent, and the face refinement or the map for which the change is below 1e-3 is reported; the projection's weights on the 3:2 wall are checked for the square's mirror symmetries (ADR 0410 Q2) and an asymmetry is explained or removed with defaults bit-identical.
