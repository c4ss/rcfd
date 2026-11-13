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
