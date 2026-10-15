# ADR 0028: Wave 18C: Lagrangian particle tracking (F-056)

Status: accepted. Everything under "Decisions" and "Registered checks" was written before any of the new code was
compiled or run. The limits are not changed, widened, replaced or deleted after a run. A miss is reported as measured and the
assertion moves to an ignored `*_known_miss` test with the registered limit intact; the cause is diagnosed, not tuned away. A
test set-up defect gets a NEW corrected test and the old one stays as a record. Addenda (file names, mesh sizes not fixed here)
may be appended at the end without touching registered limits.

Roadmap: F-056 (one-way coupled Lagrangian tracking: drag, gravity, wall reflection / trapping). F-128 (two-way coupling,
turbulent dispersion, breakup, wall film, erosion) is NOT started and stays `todo`.
Clean-room: published literature only; nothing from vendor material, vendor installs or vendor manuals, and no
vendor names in the repository. The drag-law and size-distribution formulas were looked up on the open web (secondary
sources: journal preprints and general references; the 1933 original of the Schiller-Naumann correlation was not accessed).
Status words used below: "verified (secondary)" = the formula was found, with the same coefficients, in at least two
independent published secondary sources during this wave; "derived" = derived in this ADR; "unverified" = from memory only.

## Decisions

### D1 Crate and layering

New crate `rcfd-particles` (layer 2, with `rcfd-io` and `rcfd-fv`): depends on `rcfd-core` and `rcfd-mesh` only. It does NOT
depend on `rcfd-fv`: the velocity gradient needed for the gradient-corrected interpolation is an input (`&[[Vec3;3]]`-like
per-cell tensor, supplied by the caller). `rcfd-api` depends on it (layer 4). `docs/architecture.md` and the layer table of
`crates/rcfd-cli/tests/workspace_graph.rs` gain the crate (the test's table is extended, no assertion is weakened).

### D2 Equation of motion (one-way coupled, point particle)

Spherical particle of diameter `d`, density `rho_p`, velocity `v`, in a fluid of density `rho_f`, dynamic viscosity
`mu = rho_f nu`, velocity `u` at the particle. Particle relaxation time (Stokes) `tau_p = rho_p d^2 / (18 mu)` (derived from
Stokes drag `3 pi mu d (u - v)` divided by the mass `pi rho_p d^3 / 6`).

    dv/dt = f(Re) (u - v) / tau_p + (1 - rho_f/rho_p) g,   dx/dt = v,   Re = rho_f d |u - v| / mu,   f(Re) = C_D Re / 24.

The gravity term is gravity plus buoyancy (Archimedes) only. Not included (stated, not hidden): added mass, pressure-gradient
(fluid acceleration) force, Basset history, lift, Brownian motion, turbulent dispersion, particle rotation, particle-particle and
particle-to-fluid coupling (one-way only).

Drag laws (`C_D`):
* `stokes`: `C_D = 24/Re`, `f = 1`. Derived.
* `schiller_naumann`: `C_D = 24/Re (1 + 0.15 Re^0.687)` for `Re <= 1000`, `C_D = 0.44` for `Re > 1000`; `f = 1 + 0.15 Re^0.687`
  for `Re <= 1000`, `f = 0.44 Re/24` above. Formula verified (secondary): the same bracket `1 + 0.15 Re^0.687` and the Stokes
  limit are given by two independent secondary sources (arXiv:1506.00478 and a molecular-dynamics code's published
  documentation); the validity limit is quoted as 800 by some and 1000 by others (original not accessed), so the switch at
  1000 is marked UNVERIFIED. At `Re = 1000` the two branches differ by 0.437 against 0.44 (a 0.7 percent jump, not removed).
* `none`: `f = 0` (ballistic; used for the closed-form wall checks).

### D3 Integration (fluid velocity frozen over a sub-step)

* `analytic`: with `u` and `f` frozen over the step, `v` relaxes exponentially: `tau = tau_p/f`, `w = u + tau g_eff`,
  `v_new = w + (v - w) e^{-dt/tau}`, `x_new = x + w dt + (v - w) tau (1 - e^{-dt/tau})`. Exact for Stokes drag in a uniform
  field (derived). For `f = 0` the ballistic parabola. `f` is evaluated at the start of the sub-step.
* `euler`, `rk2` (midpoint), `rk4` (classical) integrate the system `(x, v)` with `u` re-interpolated at each stage position
  (stages that leave the start cell are located with the walk of D5).
* Sub-stepping: a flow step `dt` is split into sub-steps `h <= min(remaining, 0.5 L_c / |v|)` (`L_c = V_cell^(1/3)`), and for
  `euler`, `rk2`, `rk4` also `h <= 0.1 tau_p/f`. The analytic scheme has no relaxation limit.

### D4 Interpolation

`cell_centre`: `u(x) = u_c`. `gradient`: `u(x) = u_c + (x - x_c) . grad u_c` (exact for linear fields when the gradient is
exact). No limiter is applied (named gap: unlimited gradient interpolation can overshoot near discontinuities).

### D5 Location and tracking

Faces are triangulated in a fan around the face vertex average (the same surface for both cells that share the face, so the
partition of space into cells has no gaps or overlaps even for warped faces). A particle moves along the segment `x0 -> x1` of
a sub-step: in the current cell find the fan triangle through which the segment leaves the cell (outward crossing direction,
smallest parameter), move to the neighbour, repeat until the end point is inside (parameter monotonically increasing, so the
walk terminates; an iteration cap raises a failure rather than looping). Point location without a hint starts from the
nearest cell centre (uniform bin grid) and walks the segment from that centre to the point; if the walk leaves the domain,
a brute-force search over cell bounding boxes is the fallback. Cell membership for the brute-force reference is the same
tetrahedral decomposition (cell centre, face vertex average, two face vertices). Non-conformal interfaces: supported only when
the interface was joined into one conformal mesh by `rcfd_mesh::non_conformal::join_non_conformal`; tracking through an
unjoined sliding/non-conformal pair is not supported (named gap). Cyclic (periodic) patches are not supported (located error).

Boundary hit: when a sub-step segment exits through a boundary face, the hit time within the sub-step is refined by bisection
(60 iterations) on the signed distance of the integrated end position to the face plane, the particle is placed on the face and
the boundary action applies; the rest of the flow step is continued with the remaining time.

### D6 Boundary actions

Per patch (default from the patch kind, override per patch name): `reflect` (specular reflection with restitution `e` in
`[0, 1]`: `v' = v - (1 + e)(v . n) n`, i.e. the normal component is multiplied by `-e`, the tangential component is kept),
`stick` (deposited: removed from the tracking, counted as deposited, position kept), `escape` (leaves the domain, counted).
Defaults: `Wall` follows `particles.wall_interaction` (default `reflect`) and `particles.restitution` (default 1); `Outlet`, `Inlet`, `Generic`: escape; `Symmetry` and
`Empty`: reflect with `e = 1` (specular); `Cyclic`: error. A particle that reflects with `e = 0` and velocity then pointing out
of the wall is moved along the wall (tangential only). A particle reflected more than 100 times in one flow step is stuck
(counted as `stuck`, reported). No wall-film, rebound-angle, rough-wall or sticking-probability model (F-128).

### D7 Injection and size distributions

Injection: `point` (position, optional direction) and `surface` (a boundary patch: positions uniform by area, with the
face chosen by area and the point in the face by a uniform barycentric draw in a random fan triangle). Rate in parcels per
second from `start_time` to `end_time`; the number injected up to time `t` is `floor(rate (t - start))` (deterministic; the
fractional remainder carries over, so the total over a run is exact and independent of the step size). Each parcel
represents `parcel_count` real particles (a real number, default 1). Initial velocity: `given` vector, or `fluid` (the local
fluid velocity). Surface injection places the parcel at the face centre offset into the cell by `1e-9 L_c` along the inward
normal, only for the sampling of a located cell.
Size distribution: `constant`, or `rosin_rammler` with the published cumulative OVERSIZE mass fraction
`Y(d) = exp(-(d/d_bar)^n)` (`d_bar` the size constant, `n` the spread parameter; verified (secondary): two independent
published references give this form, one of which also states that `d_bar` is the diameter where `Y = 1/e`). Sampling by
inversion: `d = d_bar (-ln u)^(1/n)`, `u` uniform in `(0, 1]` (derived from `Y(d) = u`), clipped to `[d_min, d_max]` if set
(re-draw on rejection, so the clipped law is the truncated Rosin-Rammler law). Each parcel then represents an equal MASS share
of the distribution (so `parcel_count` is scaled by `(d_ref/d)^3` only if the user selects `equal_mass`; default
`equal_count` keeps `parcel_count` fixed and the sampled diameters follow `Y` as a mass distribution of parcels, a
documented simplification).
Random numbers: SplitMix64 (published, Steele, Lea and Flood 2014; the constants are used from memory, UNVERIFIED but
irrelevant to correctness: any 64-bit generator with the same API would do), seeded per injection from `particles.seed`;
serial, so a run is bit-for-bit reproducible.

### D8 Output

ASCII VTK XML PolyData (`.vtp`) with `Verts`, points and point data `id` (int), `diameter`, `velocity` (3 components),
`density`, `parcel_count`, `cell` (int), `age` (time since injection), `state` (0 active, 1 deposited, 2 escaped, 3 stuck).

### D9 Case tree and run hook (`rcfd-api::particles`)

`particles` section, additive; every numeric key has an explicit unset sentinel (`-1`) and a located error when it is needed
and unset; `particles.enabled = false` by default and nothing changes for existing cases (baseline hashes). Keys:
`enabled`, `density`, `drag`, `integration`, `interpolation`, `gravity` (use `physics.gravity`), `dt` (tracking step, `-1` =
the flow step), `end_time`, `seed`, `wall_interaction`, `restitution`, `walls.<patch>.{interaction, restitution}`,
`injection.<name>.{kind, position, patch, rate, start_time, end_time, velocity_mode, velocity, parcel_count, distribution,
diameter, rr_d_bar, rr_n, d_min, d_max}`, `output_file`, `output_every`. `particles::check(case)` gives located errors
(for example `particles.injection.jet.rate`). Mesh-dependent checks (unknown patch, cyclic patches) are raised when the tracker
is built.
Hook: `particles::track_frozen(case, &Simulation)` tracks on the current (frozen or converged) velocity field of the
simulation; `particles::track_transient(case, &mut Simulation)` advances the simulation step by step and advances the
particles over each flow step on the current field (fluid velocity frozen over the flow step; the end of the run is
`particles.end_time` or the simulation end). The gradient for `interpolation = gradient` is the least-squares cell gradient of
each component (`rcfd_fv::gradient::least_squares`) with the boundary values of the velocity field. The flow solver is never
changed by particles (one-way), so the existing flow results are bit-identical.

### Not attempted (stated now)

Two-way coupling, turbulent dispersion (F-128), droplets / combusting / multicomponent particle types, heat and mass
transfer, breakup, wall film, erosion, injection of cone / group / file types, parcel-count control, particle-particle
collisions, periodic patches, tracking through unjoined non-conformal interfaces, a cyclone separation case (the roadmap
acceptance names settling and cyclone sanity cases; only settling-type closed forms are registered here). F-056 is therefore
`partial` whatever the numbers.

## Registered checks

All tests in `crates/rcfd-particles/tests/closed_forms.rs` (library) and `crates/rcfd-api/tests/particles_case.rs` (tree).
Reference particle for the temporal checks: `d = 1e-4 m`, `rho_p = 2500`, `rho_f = 1.2`, `mu = 1.8e-5` (Stokes `tau_p =
rho_p d^2/(18 mu) = 0.0771605 s`), `g = (0, 0, -9.81)`. "Relative error" is `|a - b| / |b|`.

* PT1 terminal velocity. Quiescent fluid (`u = 0`), Stokes, analytic integrator, `dt = tau_p`, released at rest, integrated to
  `t = 25 tau_p`. `|v_z - v_t| / |v_t| <= 1e-6` with `v_t = d^2 (rho_p - rho_f) g / (18 mu)`. Also in the Schiller-Naumann law
  the terminal state satisfies the balance `f(Re_t) v_t / tau_p = (1 - rho_f/rho_p) g` with relative residual `<= 1e-8`
  (RK4, `dt = tau_p/20`, `t = 60 tau_p`) (PT1b).
* PT2 exponential approach. Same set-up, analytic integrator, `dt = tau_p`: at every `t_n = n tau_p`, `n = 1..10`,
  `|v_z - v_t (1 - e^{-t_n/tau_p})| / |v_t| <= 1e-8` and `|z - z_exact| / (|v_t| tau_p) <= 1e-8` with
  `z_exact = v_t (t - tau_p (1 - e^{-t/tau_p}))` (sign as for the downward direction).
* PT3 Euler / RK convergence. Velocity error at `t = tau_p` against the exact exponential, step `h = tau_p/8, /16, /32, /64`
  for `euler`; `h = tau_p/2, /4, /8, /16` for `rk2` and `h = tau_p/4, /8, /16` for `rk4`. Observed order `log2(e_h/e_{h/2})` of
  every successive pair: `euler` in `[0.9, 1.1]`, `rk2` in `[1.9, 2.1]`, `rk4` in `[3.7, 4.3]`. (The tracker's own sub-stepping
  limits are bypassed in this test: the single-particle `step_particle` function is called with the fixed `h`.)
* PT4 uniform-flow relaxation. `u = (1, 0, 0)`, `g = 0`, particle at rest, Stokes, analytic integrator. `v_x(t)/u_x = 1 -
  e^{-t/tau_p}` to `1e-12` (absolute) at `t = 0.3, 1, 2.5 tau_p` with `tau_p = rho_p d^2/(18 mu)`; the time at which `v_x/u_x`
  first exceeds `1 - 1/e`, found by bisection on the analytic expression of the integrator output, equals `tau_p` to relative
  `1e-12`. Schiller-Naumann (PT4b): `v_x` increases monotonically to `u_x`, `|u_x - v_x|/u_x <= 1e-6` at `t = 400 tau_p`
  (RK4, `dt = tau_p/20`), and the relaxation is faster than Stokes at the same time (`v_x^{SN}(tau_p) > v_x^{Stokes}(tau_p)`).
* PT5 solid-body rotation. Mesh `box_mesh([12,12,6], [4,4,1])` shifted so the axis is at the centre `(2, 2)`, velocity `u = Omega
  x r` with `Omega = 3 rad/s` about z, gradient interpolation with the exact gradient (so the field is reproduced exactly),
  `g = 0`, Stokes drag with `tau_p = 0.05 s` (a particle with `rho_p = 1000, mu = 1e-3, rho_f = 1, d` chosen so that `tau_p =
  0.05`), RK4, `dt_flow = 0.005`, particle released at `r = (3, 2, 0.5)` with `v = (0, 0, 0)`, for `t = 0.25 s`. Hand-derived
  solution (complex position `z = x + i y` relative to the axis): `tau z'' + z' - i Omega z = 0`, roots `lambda_{1,2} =
  (-1 +- sqrt(1 + 4 i Omega tau)) / (2 tau)`, `z(t) = A e^{lambda_1 t} + B e^{lambda_2 t}` with `A + B = z0`, `A lambda_1 + B
  lambda_2 = v0`. Limit: position error `<= 1e-8` (absolute, metres) at the end and at `t = 0.1 s`. The particle must stay
  inside the mesh (asserted) and cross at least 3 cells (asserted).
* PT5b Couette. `u = (gamma y, 0, 0)`, `gamma = 20`, gradient interpolation, analytic integrator, Stokes, `g = 0`, start
  `(0.5, 0.37, 0.5)` at rest in a box `[0,4]x[0,1]x[0,1]` (12 x 10 x 4 cells, y NOT on a cell centre), `t = 0.4 s` with
  `dt_flow = 0.01`: `v_x = gamma y_0 (1 - e^{-t/tau_p})`, `x = x_0 + gamma y_0 (t - tau_p (1 - e^{-t/tau_p}))`,
  `y` constant. Relative error of `v_x` and `x - x_0` `<= 1e-10`. Cell-centre variant (PT5c): the same run with `cell_centre`
  interpolation reproduces the piecewise-constant field exactly: `v_x` equals the analytic relaxation to the value
  `gamma y_c` of the cell-centre height `y_c` to `1e-12` (all sub-steps in one cell row; asserted), and differs from PT5b by
  more than `1e-3` (documents the interpolation difference).
* PT6 specular reflection. Function-level: 1000 random unit normals and velocities (seeded): `e = 1`: speed conserved to
  `1e-14` relative; `e = 0.6`: `v'.n = -0.6 v.n` and `v' - (v'.n)n = v - (v.n)n` to `1e-14` (relative to `|v|`). Tracker-level
  (PT6b): ballistic (`drag = none`) ball dropped from rest at `h = 1 m` in a closed box with a reflecting floor, `e = 0.8`,
  `g = 9.81`, run with `dt_flow = 1e-3` to the exact apex time `t_a = sqrt(2h/g)(1 + e)` (the last step is shortened to land
  on it): `z = e^2 h` and `|v_z| <= 1e-9 sqrt(2 g h)`, relative error of `z` `<= 1e-9`.
* PT7 conservation of injection. Box with floor patch `stick`, top patch `escape`, the remaining walls reflecting; two
  injections (point, rate 800 parcels/s from 0 to 0.5 s; surface on the ceiling patch, rate 300/s from 0.1 to 0.4 s), Schiller-
  Naumann drag with a random size distribution (Rosin-Rammler), flow `u = (0.3, 0, -0.2)` uniform, gravity on; run until all
  particles are deposited or escaped or `t = 5`. Exact: `injected = 400 + 90 = 490` (the floor-rate formula; `|injected - 490|
  = 0`), `injected = escaped + inside + deposited + stuck` (integers, exactly), real-particle count `sum parcel_count` conserved
  to relative `1e-12`, and none of the counts is negative. Step-size independence: the same run with `dt_flow = 0.01` and
  `0.0073` injects the same total (490 exactly).
* PT8 cell location. 1e5 random points, seeded. Meshes (each `validate()`s): (a) `box_mesh([14,12,10],[1,1,1])` warped by `x' =
  x + 0.08 sin(2 pi y) sin(2 pi z)`, `y' = y + 0.08 sin(2 pi x) sin(2 pi z)`, `z' = z + 0.08 sin(2 pi x) sin(2 pi y)` plus the
  shear `x' += 0.3 y`; (b) `box_mesh([10,16,10],[1,1,1])` with `Grading::Expansion(50)` in y and the same shear. Points are
  images of uniform points of the unit cube. Brute force: all cells whose bounding box contains the point, then the
  tetrahedral decomposition test. For every point where brute force finds at least one cell, the walk (started from a random
  cell) must return a cell in the brute-force set (100 percent; limit: zero mismatches). For every point where brute force
  finds no cell (outside the faceted domain surface) the walk must return `None` (zero mismatches). The number of points
  outside the faceted domain is reported (no limit) and must be `< 20 000` of 1e5 (a mesh set-up sanity bound).
* PT9 deposition. Ballistic: a particle released at `(0.3, 0.4, 0.7)` with `v = (0.5, 0.1, 0)`, floor `z = 0` `stick`, `drag =
  none`: deposited exactly once, at time `sqrt(2 z_0/g)` and position `(x_0 + v_x t, y_0 + v_y t, 0)`, relative error `<= 1e-9`.
  Stokes in quiescent fluid from `z_0 = 0.7`, analytic integrator: landing time `T` solves `z_0 = v_t (T - tau_p (1 -
  e^{-T/tau_p}))` (Newton on the closed form in the test); deposition time relative error `<= 1e-9`, horizontal position
  unchanged. Particle with `escape` ceiling: a particle moving up leaves at the hand-computed time; counts as escaped.
* PT10 reproducibility. The PT7 scenario run twice with `seed = 7`: all fields of every particle and all counters are
  bit-identical (`to_bits` comparison), and the VTP text is identical. Seed `8` differs (positive control).
* PT11 Rosin-Rammler. (a) the inverse sampler round trip: for 1000 `u`, `Y(d(u)) = u` to `1e-12`; (b) 200 000 samples with
  `d_bar = 1e-4`, `n = 2.5`, seed 3: the fraction of samples above `d = 0.5, 1, 1.5 d_bar` differs from `Y(d)` by at most
  `5 sqrt(Y (1 - Y)/N)` (a statistical bound; the sampled quantity is the parcel fraction, see D7); (c) `d_min`/`d_max` clipping
  keeps every sample inside the bounds and the fraction in `[d_a, d_b]` equals `(Y(d_a) - Y(d_b))/(Y(d_min) - Y(d_max))` within
  the same 5-sigma bound.
* PT12 interpolation. On the warped mesh of PT8(a) the gradient-corrected interpolation reproduces a linear field
  `a + b . x` with the exact gradient to `1e-12` at 1000 random points; `cell_centre` returns the cell-centre value exactly.
* PT13 VTP output. The file written for a known particle set contains `NumberOfPoints="N"`, the declared arrays, and parsing
  the ASCII numbers back reproduces the positions and diameters to `1e-15` relative (shortest-round-trip float printing).
* PT14 tree. Located errors (`Case::semantic_errors`): `particles.density` unset when enabled; `particles.end_time` unset;
  `particles.injection.<n>.rate` unset or non-positive; `particles.injection.<n>.patch` empty for `surface`;
  `particles.injection.<n>.position` not 3 components for `point`; unknown enum values via the schema; `rr_n <= 0`;
  `restitution` outside `[0, 1]`; `walls.<patch>` unknown patch (raised at build). Run hook: frozen-flow Couette case from
  `initial.velocity_expression_x = "20 [1/s] * y"` (`solver.frozen_flow = true`) with a particle released at rest tracked via
  `track_frozen` with gradient interpolation equals the PT5b analytic values to `1e-6` relative (the gradient is the discrete
  least-squares gradient of the exactly linear cell values and boundary values, exact to roundoff; limit looser than PT5b
  because the frozen field passes through the flux projection); the transient hook (`track_transient`, `piso`, still fluid in a
  closed box with `dt = 0.01`) gives the PT1 terminal velocity to `1e-6`. Particles disabled: tree hash and a 5-step cavity run
  unchanged; the full baseline-hash test is run unchanged and is not re-recorded.

### Tunings

None may be made. Deviations are disclosed in the Results addendum.

## Results (addendum, written after the runs; no registered limit was changed)

Tests: `crates/rcfd-particles/tests/closed_forms.rs` (release, about 1 s) and `crates/rcfd-api/tests/particles_case.rs`. Measured
against the limits registered above.

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| PT1 terminal velocity (Stokes, analytic, 25 tau) | 1e-6 | 1.39e-11 | met |
| PT1b Schiller-Naumann terminal balance residual | 1e-8 | 1.45e-15 (Re_t = 3.69) | met |
| PT2 exponential approach, velocity / position | 1e-8 | 1.5e-16 / 4.8e-16 | met |
| PT3 Euler orders (3 pairs) | [0.9, 1.1] | 1.040, 1.019, 1.010 | met |
| PT3 RK4 orders (2 pairs) | [3.7, 4.3] | 4.151, 4.075 | met |
| PT3 RK2 orders, tau/2 to tau/16 (3 pairs) | [1.9, 2.1] | first pair 2.290 | MISS |
| PT4 uniform-flow relaxation, abs error; relaxation time | 1e-12; 1e-12 rel | 2.2e-16; 1.8e-16 | met |
| PT4b Schiller-Naumann: monotone, final, faster than Stokes | 1e-6 | monotone; 1.1e-15; 0.7415 vs 0.6321 | met |
| PT5 solid-body rotation, position error at 0.1 s / 0.25 s, cells crossed | 1e-8; >= 3 | 3.68e-8 / 4.05e-9; 2 cells | MISS |
| PT5b Couette (analytic, gradient), v and x relative | 1e-10 | 0 and 3.4e-16 | met |
| PT5c Couette cell-centre equals piecewise-constant solution; differs from PT5b; all sub-steps in one cell row (asserted in A2) | 1e-12; > 1e-3; y in [0.3, 0.4] each step | 0 and 1.8e-16; 0.400; y = 0.37 +- 1e-14 each of 40 steps | met |
| PT6 reflection (1000 random), e = 1 and e = 0.6 | 1e-14 | 8.5e-16 | met |
| PT6b bouncing ball apex height / velocity | 1e-9 | 3.5e-14 rel; vz -6.5e-14 | met |
| PT7 injection conservation (dt 0.01 and 0.0073), after the A2 fix | exact | 490 = 0 + 431 + 0 + 59 (and 0 + 433 + 0 + 57); real 1225 | met |
| PT7 regression added in A2: no active particle outside the box over 300 steps | 0 | 0 | met |
| PT8 (a) warped mesh, pure walk from a random cell | 0 mismatches | 1522 of 99 510 in-domain points | MISS (strict) |
| PT8 (b) graded + sheared mesh, pure walk | 0 mismatches | 0 of 100 000 | met |
| PT9 deposition ballistic: time, x, y | 1e-9 | rel ~1e-15 | met |
| PT9 deposition Stokes: landing time (1.0023750631) | 1e-9 | 2e-15 rel | met |
| PT9 escape (time 0.5, z = 1) | 1e-9 | met | met |
| PT10 bit-for-bit reproducibility (fingerprint and VTP text); seed 8 differs | exact | identical; differs | met |
| PT11 Rosin-Rammler: inversion round trip; 3 fractions; truncated fraction | 1e-12; 5 sigma | passes; largest deviation 5.5e-4 vs bound 5.4e-3 | met |
| PT12 linear-field interpolation, warped mesh | 1e-12 | 8.9e-16 | met |
| PT13 VTP round trip | exact | exact | met |
| PT14 tree: frozen Couette v / x relative; transient settling; located errors (paths asserted, incl. schema `particles.drag`, `particles.restitution`); output file | 1e-6; 1e-6 | 0 / 8.6e-16; 5.6e-12; paths found | met |
| PT14 disabled section: 60-iteration cavity run bit-identical, equal to the registered `cavity-simple` hash 07e7461c36893384 (whole `baseline_hashes.rs` also unchanged) | exact | identical | met |
| PT14 disabled section: settings-tree hash unchanged | exact | changes (hash covers schema defaults) | MISS (ill-posed, see A2) |

Baseline hashes (`baseline_hashes.rs`) and the layering test pass unchanged on a git archive copy of the final commit.

### Misses and their diagnosis (assertions kept, moved to ignored `*_known_miss` tests)

* PT3 RK2 (`pt3_known_miss_rk2_order`): the pair tau/2, tau/4 gives 2.290, outside [1.9, 2.1]. Diagnosis: step tau/2 is outside the
  asymptotic range of a second-order method on a relaxation problem. The diagnostic test `pt3b_rk2_order_finer_steps_diagnostic`
  (steps tau/8 to tau/64, NOT the registered sequence) gives 2.069, 2.034, 2.017. It is a different measurement and does not
  count as meeting the registered check.
* PT5 (`pt5_known_miss_solid_body_rotation`): two failures. (1) The position error at t = 0.1 s is 3.68e-8 against 1e-8 (4.05e-9 at
  0.25 s is met). The ADR's prediction of about 1e-10 was an estimate made without a computation; the error is the RK4 time-stepping
  error, since `dt_flow = 0.005 = tau_p/10`: the diagnostic `pt5d_rotation_time_error_diagnostic` (a different start point, see
  below) shows errors 5.16e-8, 3.09e-9, 1.89e-10 for dt 0.005, 0.0025, 0.00125 with observed order 4.06 and 4.03. (2) The "at least
  3 cells" precondition fails (2 cells): the registered start point (3, 2, 0.5) lies on cell faces and the particle moves along one
  cell column for 0.6 m. This is a set-up defect of the precondition; the diagnostic uses the start (3.4, 2, 0.45) (4 cells crossed).
  The registered accuracy limit is still missed at the registered set-up.
* PT8 (a) (`pt8_known_miss_location_warped`): under the strict reading (pure walk, no fallback, from a random start cell) 1522 of
  99 510 points inside the faceted domain were not found. Diagnosis (`walk` ends at a boundary face for every one of them, none ends
  in a wrong cell): the straight segment from the start cell centre to the point leaves the wavy, non-convex domain. The walk itself
  never returned a wrong cell. The production `locate` (walk with the brute-force fallback described in D5) is checked by the
  additional test `pt8b_location_warped_with_fallback`: 0 mismatches inside, 0 outside, 490 points outside the faceted domain. The graded
  and sheared mesh (convex domain) has 0 mismatches with the pure walk.

* PT14 tree hash (`pt14_known_miss_tree_hash_unchanged`, ignored): `SettingsTree::content_hash` is an FNV hash of the whole data tree
  including schema defaults, so adding the `particles` section changes the hash of every case. The registered sub-check cannot be
  met by any additive section; it was not tested in the first commit (only that no error is raised and `enabled` is false). The
  assertion kept in the ignored test is that the tree carries no `particles` key. What is verified instead (a different measurement,
  and it is the behaviour that matters): the flow solution of the registered cavity is bit-identical with and without a disabled
  populated section and equals the registered baseline hash (`pt14_disabled_section_leaves_the_cavity_run_bit_identical`).

### Addendum A2: corrections after review of the first implementation commit (8d48571)

* Tracker bug found while diagnosing the PT7 `stuck` count. The first commit refined a wall hit by bisecting on the signed distance
  to the plane of the single face found by the walk. Near an edge or corner (observed at the x = 1 / z = 0 edge of the PT7 box with the
  segment lying in the y = 0.5 face plane) the curved path can leave through a different face first, so the hit point was projected
  onto the wrong plane and the particle was left slightly outside the box; the walk then reported `Inside` and the particle drifted
  away with the fluid (in the first PT7 run, tens of `Active` parcels at x about 2, z down to -4.4). Fix: the bisection now uses the
  tracker's own predicate (the chord from the sub-step start to the shortened step leaves the domain) and takes the face and hit
  point from the last bisection iterate that left. No tolerance, limit or scenario was changed (`max_bounces` stays 1000).
  Regression test `pt7c_no_particle_leaves_the_domain_regression` (300 steps, no active parcel outside the box); it failed before the fix.
* PT7 numbers after the fix: 490 = 0 + 431 + 0 + 59 (dt 0.01) and 0 + 433 + 0 + 57 (dt 0.0073). The earlier 38 / 36 "inside" and 24
  stuck counts included the mis-tracked particles and are superseded. The registered identity holds exactly.
* Why parcels are `stuck`: all listed stuck parcels sit on the reflecting wall x = 1 with a tiny negative `v_x` (1e-5 to 3e-4 m/s), sliding down it
  at nearly the settling speed, while the uniform fluid velocity (0.3, 0, -0.2) pushes them into that wall. Each reflection is followed by a
  return to the wall in a time that shrinks, so more than `max_bounces` reflections occur in one flow step (Zeno accumulation of resting
  contact). This matches the observation but the mechanism is read from positions and velocities, not from a bounce-by-bounce trace.
  Resting contact is not modelled (F-128 wall interaction); the parcels are counted and reported, not hidden.
* The ADR text of D6 above says "more than 100 reflections"; the implementation and `docs/particles.md` use 1000. This was a text
  inconsistency from the design stage, found now; the implementation value was never tuned.
* The first-commit rows for PT14 and PT5c overstated what was tested: PT14 did not test the tree-hash sub-check and asserted only
  `is_err()` for the two schema errors; PT5c did not assert the one-cell-row precondition. Both are now asserted (rows above), the
  tree-hash sub-check is a recorded miss.
* The diagnostic formerly named `pt5b_rotation_time_error_diagnostic` is renamed `pt5d_...` because `PT5b` is the registered Couette label.

### Disclosures

* PT7: see A2 for the corrected counts and the diagnosis of the `stuck` parcels. The identity `injected = inside + deposited + escaped + stuck` holds exactly.
* The PT6b text of this ADR was rewritten before any code ran (the first version described a procedure that was dropped).
* D7: `equal_mass` parcel weighting is not implemented (stated there as a non-default option; the default documented simplification is what is
  implemented). Surface injection offsets the parcel 1e-9 L_c into the cell.
* D9: the hook is a library function pair (`track_frozen`, `track_transient`); `rcfd run`, the service and the GUI do not call it.
* Cyclone separation, droplet / combusting / multicomponent types, cone / group / file injections, parcel-count control, two-way coupling,
  turbulent dispersion, periodic patches and tracking through unjoined non-conformal interfaces are not delivered. F-056 is `partial`.
* Unverified references: the Schiller-Naumann validity limit (1000), SplitMix64 constants; Schiller-Naumann coefficients and the
  Rosin-Rammler form are verified in secondary sources only (see `docs/reference-data.md`).
* Shared files touched (small hunks): `Cargo.toml`, `Cargo.lock`, `crates/rcfd-api/Cargo.toml`, `crates/rcfd-api/src/{lib,schema,case}.rs`,
  `crates/rcfd-cli/tests/workspace_graph.rs`, `docs/architecture.md`, `docs/case-schema.md`, `docs/reference-data.md`.
