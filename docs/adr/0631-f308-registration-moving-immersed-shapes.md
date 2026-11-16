# ADR 0631: F-308 registration: moving immersed shapes (an in-line oscillating cylinder against the body-fitted moving-mesh run)

Status: **REGISTERED 2026-11-16, written before any code of F-308 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-308 was written, compiled or run: the documents and code under "What
was read" were read and the arithmetic under "Arithmetic before this registration" was done by hand. No timing probe was run;
run lengths are set from the timing disclosed in ADR 0218 (0.31 s per PISO step on 82 308 cells with AMG pressure). Results go
in the addenda at the end; nothing above the addenda is changed after a run.

Parents: [ADR 0218](0218-f219-registration-immersed-solid-with-modified-forcing.md) (F-219: the forcing; its question Q4, from
which F-308 was made: "the forcing is built once; a moving body needs a rebuild per step and a treatment of the cells that change
side (fresh cells); a separate item checked on an oscillating cylinder"), [ADR 0249](0249-f346-registration-immersed-force-in-unsteady-flow.md)
(F-346: `force_moment_unsteady`, `velocity_rate`), [ADR 0268](0268-f354-registration-dfg-2d2-force-extrema-mesh-and-time-step-convergence.md) and ADR
0363 (the D/40 and D/80 accuracy of the static forcing), [ADR 0405](0405-f494-registration-immersed-multistep-linearisation.md),
[ADR 0421](0421-f501-registration-immersed-early-state-start.md), [ADR 0425](0425-f515-registration-immersed-start-projection.md)
(the early state and the start of a run with a body), [ADR 0471](0471-f535-registration-opt-in-refusal-at-the-pressure-lag-onset.md)
and ADR 0451 (PISO diverges above a cell diffusion number of about 2.5: the meshes below keep it under 1.4), ADR 0033 and ADR
0101 (the swept-volume moving-mesh step and the oscillating cylinder C1 of `tests/ale_flows.rs`, the body-fitted path used as the
reference), ADR 0252 (F-351: the wall pressure of a moving wall from the prescribed gradient, `patch_force_moment_wall_gradient`).
Roadmap item F-308 (deps F-219). Number 0631 given by the lead. Branch `f308-work` from local main 3f9ce17a, worktree
`w49i-work/f308`, scratch `w49i-work/f308-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; existing tests not
edited; defaults bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new
dependency, no unsafe code, crate graph unchanged; clean-room. The roadmap files are not edited. Every choice below that the
maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation), except the open questions at the end.

## What F-308 asks

The acceptance, binding, verbatim (with the brief's first sentence):

> Every limit not stated here is registered in the ADR before the run. An oscillating cylinder gives the force and the wake against a body-fitted moving-mesh run within a registered limit.

Clauses: **c0** every limit registered here; **c1** an oscillating cylinder (a moving immersed shape); **c2** its force against a
body-fitted moving-mesh run within a registered limit; **c3** its wake against that run within a registered limit. The brief adds:
the force measures are the mean and amplitude of drag and lift and the phase; the wake measure is a velocity profile at registered
stations; a negative control; mutations; the immersed default path bit-identical (motion opt-in); no case-file key (open question).

### Readings, registered

- **The reference.** The acceptance names the reference: the body-fitted moving-mesh run of this repository (the swept-volume ALE
  PISO step of ADR 0033, `PimpleSolver::step_ale_with_walls`, with the follow condition of the cylinder wall). No published force
  or velocity value is used: the repository holds none for this case. To show that the reference is itself accurate to better
  than the limits, it is run on two meshes (check R1).
- **The case.** An in-line oscillation in fluid at rest. The parameters are those of the low-KC benchmark of Dütsch, Durst,
  Becker and Lienhart (J. Fluid Mech. 360 (1998) 249-271, cited from memory; only the two numbers KC = 5 and Re = 100 are taken,
  none of its data): at these values the flow is periodic and two-dimensional, the cylinder reverses through the vortex pair it
  shed in the previous half period, which is what makes it a wake test and not only an added-mass test. The fluid is enclosed by a
  fixed circular wall far away (R = 8 D) so that the two runs have the same closed domain (the body-fitted mesh is an O-grid; the
  immersed run gets the same wall as a fixed inverted immersed cylinder).
- **"Drag" and "lift".** Drag is the force component along the motion (x), lift the component across it (y). By the mirror
  symmetry of the case about y = 0 the lift is zero; it is checked as zero in both runs (no lift made by the moving forcing). The
  mean of the drag is zero by the half-period antisymmetry of the periodic state; its difference is checked against the
  amplitude.
- **"Within a registered limit".** One limit per measure below, fixed from the estimates under "Arithmetic".

## What was read

`crates/rcfd-fv/src/immersed.rs` (all: shapes, `ImmersedBody::velocity_at`, `build`, `build_impl`, the force evaluations,
`velocity_rate`), `crates/rcfd-fv/src/pimple.rs` (the `immersed` field, lines 1176 and 1897 to 1903: the forcing is read from
`self.immersed` in every outer iteration of every step and nothing derived from it is cached across steps; the refusals at 1616 and
of the moving-mesh step), `crates/rcfd-fv/src/pimple/moving.rs` (`step_ale_with_walls`), `crates/rcfd-fv/src/ale.rs`
(`move_mesh`, `set_follow_conditions`), `crates/rcfd-fv/src/sixdof.rs` (`patch_force_moment`, `patch_force_moment_wall_gradient`),
`crates/rcfd-fv/src/gradient.rs` (`least_squares`), `crates/rcfd-mesh/src/gen.rs` (`cylinder_ogrid`: patches `cylinder`,
`farfield`, `frontAndBack`; `box_2d_coords`), `crates/rcfd-fv/tests/ale_flows.rs` (C1, the oscillating cylinder on a smoothed
mesh), `crates/rcfd-fv/tests/immersed_solid_f219.rs` (set-ups, R1 to R3), ADR 0218 (all), the roadmap notes of F-219, F-337,
F-346, F-354, F-063, F-309.

## Arithmetic before this registration (disclosed, by hand)

1. **Case numbers.** D = 1, r = 0.5, nu = 0.01, rho = 1, U_m = 1. KC = U_m T / D = 5 gives T = 5, omega = 2 pi / 5 = 1.2566371,
   A = U_m / omega = 0.7957747. Re = U_m D / nu = 100, beta = Re / KC = 20. Motion `x_c(t) = -A cos(omega t)` (from rest at the
   left turning point; body velocity `U_m sin(omega t)`, a start without an impulse: the start problems of ADR 0421 and 0425 come
   from an impulsive start).
2. **Layers.** Stokes layer `delta = sqrt(2 nu / omega) = 0.126 D`; boundary layer at Re 100 about `D / sqrt(Re) = 0.1 D`.
3. **Immersed mesh G40.** A uniform core of `h = 0.025 = D/40` on `[-2, 2]^2` (160 x 160 cells; the cylinder travels to
   `|x| <= 1.30`, the stations lie in `|x| <= 1.2`, `|y| <= 1.5`), and outside it cells growing by the factor 1.08 to the box edge
   `+-8.4` (about 40 cells per side; the last stretched cell is adjusted to land on 8.4): about 240 x 240 = 57 600 cells, outermost
   cell about 0.5. `delta / h = 5`. Cell diffusion number `nu dt / h^2 = 0.08`. Mesh G20 for C1: the same with `h = 0.05`.
4. **Body-fitted mesh BF.** `cylinder_ogrid(256, 128, 0.5, 8.0, Grading::Expansion(35), 1.0)`: radial ratio `q = 35^(1/127) =
   1.0284`, first cell `s0 = 7.5 (q - 1)/(q^128 - 1) = 0.0061`, radial size 0.022 at r = 1, 0.057 at r = 2; circumferential
   spacing 0.0123 at r = 0.5, 0.0245 at r = 1. 32 768 cells. Cell diffusion number at the wall `0.01 x 0.005 / 0.0061^2 = 1.35`
   (below the PISO onset of about 2.5 of ADR 0451). Coarse reference BFc: `cylinder_ogrid(128, 64, 0.5, 8.0, Expansion(35), 1.0)`,
   first cell 0.0121, 8 192 cells.
5. **Mesh motion of BF.** Not the Laplacian smoothing of C1: the displacement is the closed-form blend `x = x0 + d(t) w(|x0|)`,
   `w = 1` for `rho0 <= 1.5`, `w = (1 + cos(pi (rho0 - 1.5)/4.5))/2` for `1.5 < rho0 < 6`, `w = 0` beyond (the wall at 8 does not
   move). The region within one diameter of the surface translates rigidly (no distortion of the layers the force depends on). The
   largest strain of the blend is `A max|w'| = 0.796 pi / 9 = 0.28` (no inverted cell). Laplacian smoothing of the same
   displacement would give a strain of `A / (r ln(R/r)) = 0.57` at the surface (arithmetic for the choice only).
6. **Time step.** `dt = T/1000 = 0.005`. Courant: immersed `2 U_m dt / h = 0.4`; body-fitted, relative tangential velocity up to
   about `2 U_m` on the 0.0123 circumferential spacing: 0.81. 4 periods = 4000 steps. Window of the force measures: periods 3 and 4
   (steps 2001 to 4000).
7. **Cost.** ADR 0218: 0.31 s per PISO step on 82 308 cells. G40 (57 600 cells): about 0.22 s, 4000 steps about 15 min; BF (32 768)
   about 0.12 s plus the geometry per step, about 10 min; G20, BFc, N1 (2 periods on G40) about 4, 2 and 7 min. The five runs are
   made on five threads of one ignored record test, wall time about 20 min; memory a few hundred MB (38 GB free at registration).
8. **Error estimates behind the limits.** The static forcing on D/40 meshes (F-219, F-346, F-354): drag within 0.3 to 0.8 per cent,
   lift up to 2.7 per cent of the DFG values, the forcing order moving the extrema by up to 1.2 points. A moving body adds the cells
   that change side each step: per step about `D U_m dt / h^2 = 8` cells on each side of the body enter or leave the solid. With the
   force of D3 (the staircase surface force at the new level) the storage of an entering cell cancels its reaction (derived in D3),
   what remains is that the staircase surface itself jumps by one cell row, a disturbance at the step frequency (1000 per period)
   that the first-harmonic projection does not see; its slow part is the first-order position error of the surface, `h/2`
   relative to D, 1.3 per cent. Estimate of the force amplitude difference: 1 to 3 per cent; of the phase: the quadrature share of
   that, about 1 degree. The body-fitted reference: second order, BFc to BF difference estimated at 1 to 2 per cent of the
   amplitude. Velocity at the stations: the immersed first-fluid-cell error is of order `h/delta` of the local velocity in the layer
   only; at the stations (at least 0.05 D from the surface) estimated below 0.03 U_m RMS, with local maxima where a vortex core
   passes a station (a 1 per cent shift of a core of size 0.3 D with a peak velocity of U_m makes about 0.1 U_m).
9. **Negative control.** With the body velocity left out of the forcing (the solid is a region of fluid at rest that changes
   place), the body pushes no fluid: the force reduces to the reaction of the cells that change side, and the station velocities
   to a fraction of U_m. Estimate: the force first harmonic differs from the reference by more than 50 per cent, the station RMS by
   more than 0.3 U_m.

## Design

### D1. The library: `crates/rcfd-fv/src/immersed_motion.rs` (new file; one `pub mod` line in `lib.rs`)

No line of `immersed.rs`, `pimple.rs` or any other solver file is changed. The PISO step reads `self.immersed` in every outer
iteration and keeps nothing derived from it across steps (read at pimple.rs 1897 to 1903, 1964, 2067), so a moving shape is the
forcing of ADR 0218 rebuilt at the pose of the new time level before each step:

- `HarmonicTranslation { amplitude: Vec3, omega: f64, phase: f64 }`: `d(t) = amplitude sin(omega t + phase)`, `velocity(t) =
  amplitude omega cos(omega t + phase)`, `acceleration(t) = -amplitude omega^2 sin(omega t + phase)`.
- `translate_shape(shape, d)`: the cylinder centre, the sphere centre, the cuboid corners moved by `d`.
- `MovingImmersed { bodies, motions, settings, gamma, velocity_in_forcing }`, `MovingImmersed::new(bodies, motions, settings,
  gamma) -> Result<_, String>` (one motion slot per body, `None` for a fixed body; finite amplitude, omega and phase; located
  errors); `velocity_in_forcing = true` from `new`.
- `bodies_at(t)`: each moving body's shape translated by `d(t)`, its rotation origin translated by `d(t)`, its velocity the body's
  own plus `velocity(t)` (the latter left out when `velocity_in_forcing` is false: the negative control N1). Fixed bodies
  unchanged.
- `forcing_at(geom, t) = ImmersedForcing::build(geom, &bodies_at(t), settings, &gamma)`: the forcing of ADR 0218 D1 and D2
  (penalty of the solid cells to the rigid velocity, modified forcing at the crossing on the first fluid cells), at the pose and
  velocity of time t.
- `step(&self, solver: &mut PimpleSolver, dt) -> Result<StepInfo, String>`: sets `solver.immersed = Some(forcing_at(&solver.geom,
  solver.time + dt))` and returns `solver.step(dt)`. Implicit Euler and BDF2 treat the forcing at the new time level, so the pose
  and the velocity are those of `t + dt`.

Fresh cells (ADR 0218 Q4): a cell that leaves the solid starts its first fluid step from its penalised velocity, the rigid velocity
of the body at its centre, which is the no-slip value of the fluid next to the surface: no special treatment (as the direct-forcing
methods of Uhlmann, J. Comput. Phys. 209 (2005) 448-476, cited from memory: only the idea that the solid region holds the rigid
velocity, so a cleared cell holds a velocity close to the fluid's). A cell that enters the solid is penalised in its first solid
step. The first-order face flux of ADR 0218 is used (`build`); the second-order flux faces of F-309 and F-355 are not offered with
motion (D5).

### D2. The body-fitted reference (test code)

The O-grid BF (arithmetic 4), the points moved by the blend of arithmetic 5 from the initial points to `x0 + d(t_{n+1}) w`,
`ale::move_mesh`, `ale::set_follow_conditions` on `cylinder`, `PimpleSolver::step_ale_with_walls`; `farfield` a no-slip wall at
rest; `frontAndBack` empty. Force: `sixdof::patch_force_moment_wall_gradient(.., ["cylinder"], .., walls)` with the wall gradients
of the step (ADR 0252: the default extrapolation is first order on an accelerated wall).

### D3. The force of the immersed run (test code, existing library functions)

`F^{n+1} = forcing^{n+1}.force_moment_unsteady(0, u^{n+1}, velocity_rate(u^{n+1}, u^n, None, dt), rho, x_c)`, the forcing of the
step. Derivation: for a cell c the step's momentum equation is `rho V (u^{n+1} - u^n)/dt + (surface terms of c) = -R_c`, `R_c`
the reaction of the forcing; summed over the solid cells S^{n+1} the internal faces cancel and `sum_S R_c + sum_S rho V (u^{n+1}
- u^n)/dt` is the discrete force of the fluid on the staircase surface of S^{n+1} (with the modification terms of the first fluid
cells as part of that surface force, ADR 0249 D1). For a cell that has just entered S, the reaction carries `rho V (u^n -
u_b)/dt` (its fluid velocity forced to the rigid one in one step) and the storage term `rho V (u_b - u^n)/dt` cancels it; the
analytic rigid acceleration in place of the time difference would leave that spike, with a non-zero mean over a half period
(about 0.3 of the force amplitude by arithmetic 8's cell count; not used). The outer wall is the second, fixed immersed body and
does not enter.

### D4. Measures

- **Force.** Over the window W (steps 2001 to 4000, `N = 2000`, two whole periods): the mean `Fbar = (1/N) sum F_n` and the first
  harmonic `Fhat = (2/N) sum F_n exp(-i omega t_n)`, per component (x drag, y lift); the amplitude `|Fhat|`, the phase `arg
  Fhat`. Reported, not checked: the parts of `Fhat_x` in phase with the body acceleration and velocity as Morison-type
  coefficients `C_a = -Re(Fhat_x conj(ahat))/(rho pi r^2 |ahat|^2)` and `C_v` likewise with the velocity normalised by `rho r
  U_m^2` (for comparison with the literature only).
- **Wake.** The velocity (both components) at the stations `x_s in {-0.6, 0, 0.6, 1.2}`, `y_j = 0.05 j`, `j = 0..30`, at the
  phases `omega t = pi/2, 3 pi/4, pi` of period 4 (steps 3250, 3375, 3500: the body passing the centre at full speed, decelerating,
  at the right turning point); points closer than 0.05 D to the surface (`|(x, y) - (x_c, 0)| < 0.55`) left out, the same set in
  every run. The value at a point: the cell whose centre is nearest, plus its least-squares gradient (`gradient::least_squares`
  per component, boundary values of the field) times the offset. Measures: `e_max = max |u_IB - u_BF| / U_m` and `e_rms =
  sqrt(mean |u_IB - u_BF|^2 / 2) / U_m` (per component) over points and phases.

### D5. Not built

Rotation of the shape's orientation (a cylinder about its own axis is covered by `omega`; a cuboid that turns is not), general
motion laws (only harmonic translation plus the body's own constant velocity and spin), free (six-degree-of-freedom) motion, the
second-order flux faces with motion, BDF2-specific force (the test uses implicit Euler; `velocity_rate` covers BDF2), the SIMPLE
solver, the moving immersed body on a moving mesh (refused by the solver, ADR 0218 R2), a case-file key (Q1).

## Checks

All numbers per unit depth (both meshes have depth 1), `rho = 1`. "BF" is the body-fitted fine run, "IB" the immersed run on G40.

| Id | Check | Limit | Estimate (arithmetic) |
| --- | --- | --- | --- |
| A1 | drag amplitude `abs(abs(Fhat_x,IB) - abs(Fhat_x,BF)) / abs(Fhat_x,BF)` (c2) | <= 0.05 | 0.01 to 0.03 (8) |
| A2 | drag phase `abs(arg Fhat_x,IB - arg Fhat_x,BF)` (c2) | <= 3 degrees | about 1 degree (8) |
| A3 | drag first harmonic, complex `abs(Fhat_x,IB - Fhat_x,BF) / abs(Fhat_x,BF)` (c2) | <= 0.06 | 0.01 to 0.035 |
| A4 | mean drag `abs(Fbar_x,IB - Fbar_x,BF) / abs(Fhat_x,BF)` (c2) | <= 0.02 | below 0.005 (both near zero) |
| A5 | lift: `abs(Fbar_y)` and `abs(Fhat_y)` of IB and of BF, over `abs(Fhat_x,BF)` (c2) | each <= 1e-3 | round-off (mirror symmetry) |
| A6 | wake `e_max` (c3) | <= 0.15 | up to 0.1 at a vortex core (8) |
| A7 | wake `e_rms` (c3) | <= 0.05 | below 0.03 (8) |
| R1 | reference: BFc against BF, complex drag harmonic difference over `abs(Fhat_x,BF)`; and wake `e_rms` | <= 0.04; <= 0.025 | 0.01 to 0.02; about 0.01 |
| C1 | trend: the A3 difference and `e_rms` of G20 larger than those of G40 | both larger | first order |
| N1 | negative control (velocity left out, 2 periods on G40, window period 2 against BF period 2, wake phases of period 2): complex drag difference and wake `e_rms` | >= 0.3; >= 0.15 (must be exceeded) | above 0.5; above 0.3 (9) |
| S1 | every run: no divergence stop, finite fields, largest step continuity residual reported | no stop | |

Fast tests (not ignored, small meshes), with limits:

| Id | Check | Limit |
| --- | --- | --- |
| T1 | `HarmonicTranslation` displacement, velocity and acceleration against the closed forms at 7 times; velocity against a central difference of the displacement (step 1e-5) | 1e-14 x scale; 1e-8 |
| T2 | `bodies_at(t)`: centre / corners and origin translated by `d(t)`, velocity the body's own plus `velocity(t)`; fixed bodies unchanged | exact (bit equality with the same arithmetic) |
| T3 | defaults: with every motion `None`, and with a zero amplitude, `forcing_at(geom, t) == ImmersedForcing::build(..)` and 5 steps through `MovingImmersed::step` give the bits of 5 `step` calls with the forcing set once | bit-identical |
| T4 | after k steps of a moving body, `solver.immersed == forcing_at(geom, solver.time)` and `solver.time == k dt` to round-off; the solid-cell set of step 1 differs from that of step k | exact; differs |
| T5 | after a step, every solid cell of the moving body holds `u_b(x_c, t_{n+1})` within `1e-3 U_m` (penalty 1e6); with `velocity_in_forcing = false` the sources of its solid cells are zero and their velocity below `1e-3 U_m` | 1e-3 |
| T6 | errors: motion count different from the body count, a non-finite amplitude, omega or phase, a gamma of the wrong length: located `Err` naming ADR 0631 | as stated |
| T7 | smoke: a cylinder of radius 0.25 oscillating with A 0.2, T 1 on a 48 x 48 box, 50 steps: no stop, finite fields and finite D3 force | no stop |

The registered heavy record is one ignored test, `f308_oscillating_cylinder_record`, run once by me, release build, all five runs
in one process on threads; it prints every measure before it asserts.

### Negative controls

N1 (heavy, above) and the fast half of it in T5. The static-cylinder control of the brief is not run: in fluid at rest a static
cylinder gives zero force and flow, which tests nothing that T3 does not.

### Planned mutations (on a scratch copy of the library, each run once against the fast tests)

| Id | Mutation of `immersed_motion.rs` | Expected to catch |
| --- | --- | --- |
| M1 | the motion velocity left out of `bodies_at` regardless of the flag | T2, T5 |
| M2 | `step` builds at `solver.time` instead of `solver.time + dt` | T4 |
| M3 | displacement sign reversed | T1, T2 |
| M4 | velocity `- amplitude omega cos` | T1 |
| M5 | acceleration sign reversed | T1 |
| M6 | rotation origin not translated | T2 |
| M7 | `velocity_in_forcing` read inverted | T3, T5 |
| M8 | `step` builds the forcing only when `solver.immersed` is `None` (once) | T4 |

## Decision rule

- **done**: A1 to A7 and R1 met, N1 exceeds both its limits, S1 holds for every run, T1 to T7 pass, at least seven of the eight
  mutations caught, the default bits unchanged (T3, and `baseline_hashes` passes).
- **partial**: any of A1 to A7 or R1 missed (with the diagnosed cause), N1 not exceeding a limit, a run stopped, or a check met
  only after an amendment.
- **todo**: the moving forcing cannot be built or run.
- C1 is reported and does not change the decision (it is not part of the acceptance).

## Limits (what this does not cover)

One case (in-line oscillation, KC 5, Re 100, two dimensions, one mesh pair for the comparison); a transverse oscillation in a
stream, a rotating or deforming body, free motion, three dimensions are not run. The time step is the same in both runs, so the
time discretisation error is common and not measured. The reference is this repository's body-fitted run, not a published data
set. The forcing with motion is the first-order one of ADR 0218. The forcing is rebuilt from the whole mesh each step (no
narrow-band update); its cost is not measured. Items of D5.

## Open questions

- **Q1. A case-file key for moving immersed bodies.** Not built: the immersed solid has no case key yet (F-307, which needs the
  maintainer's approval of a schema golden edit). Options: (a) add the motion to the F-307 table when F-307 is made; (b) a separate
  item. Recommendation (a); open (a schema golden), not taken.
- **Q2. The second-order flux faces with motion.** Taken on the maintainer's standing instruction of 2026-11-11 (take the
  recommendation): not in F-308 (first-order forcing only); proposed as a follow-up if the results ask for it.

## Amendment 1 (2026-11-16, before any run of the heavy record): set-up details the registration left unstated

Written after the library and the fast tests T1 to T7 were built and run once (all met; numbers in addendum 1) and before the
heavy record was compiled into a run. No limit, case number, mesh, window or station changes. Fixed here because the
registration did not state them:

- **Solver settings of every heavy run.** `PimpleSettings::piso(0.01)` (implicit Euler, one outer iteration, two correctors, no
  non-orthogonal correctors, momentum tolerance 1e-8 relative, pressure 1e-7 relative, pressure reference cell 0 at 0) with
  central convection and `amg_pressure = true`; nothing else changed. The same in the immersed and the body-fitted runs.
- **Penalty** `K = 1e6 /s` (`ImmersedSettings::new(1e6)`: modified forcing, `theta_min = 1e-2`): `K dt = 5000`, the ADR 0218
  guideline of `K` well above `a_P / V` (here about `1/dt + 4 nu / h^2 = 264 /s`).
- **Immersed domain.** `box_2d_coords` of the stretched axis on x and y (the same coordinates), depth 1; the four box walls
  no-slip at rest (they lie in the solid of the inverted wall body); the moving cylinder is body 0, the wall body 1 (radius 8,
  inverted, fixed). The stretched cells grow by 1.08 from `h` and are scaled to end on 8.4 (`stretched_axis` in the test).
- **Body-fitted run.** `cylinder` and `farfield` start as fixed-value walls at rest; `set_follow_conditions` sets the cylinder's
  velocity in every step; the point displacement is along x only, `x = x0 + x_c(t_{n+1}) w(|x0|)` from the initial points.
- **Wake points** lie at mid-depth `z = 0.5`; the nearest cell is found in the x-y plane; only the x and y velocity components
  enter the measures.
- **The force reference point** is the body centre (it does not enter the force).
- **The phase** of a harmonic is `atan2` of `(2/N) sum F exp(-i omega t_n)`, the A2 difference wrapped to `(-180, 180]` degrees.
- **The five runs** BF, BFc, IB40 (G40), IB20 (G20) and N1 run on five threads of `f308_oscillating_cylinder_record`; a stopped
  run (S1) ends the test before any other check is evaluated.
