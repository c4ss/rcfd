# ADR 0207: F-258 registration: compressible ALE boundaries and the mesh-speed time step

Status: **REGISTERED 2026-11-07**, written before any code of F-258 and before any run of its checks. Not yet accepted by
the maintainer. Before this text one thing was run on main d953cd6, with the code of main as it is: the record tests
R1 to R4 (the new file `crates/rcfd-fv/tests/compressible_ale_f258_records.rs`, no source change), twice, which made the
records below. Results go in the addendum at the end.

Parents: [ADR 0090](0090-wave38c-dynamic-mesh-layering.md) (F-230: `CompressibleSolver::step_ale`, its frame of the
moving face D2, the layer stack D3, the "not delivered" list that F-258 comes from),
[ADR 0098](0098-wave40b-taylor-couette-converged-stretched-third-level-fast-piston.md) (the Mach 0.2 piston and its note
on the absolute time step), [ADR 0191](0191-f259-registration-case-tree-and-cli-wiring-of-layering-and-compressible-ale.md)
(F-259: the case tree runs the ALE step; its E5 refusal and W4 time step name F-258; its decision record Q3),
[ADR 0006](0006-compressible-boundary-conditions.md) (the characteristic conditions), [ADR 0033](0033-wave20b-dynamic-mesh.md)
(swept volumes and the discrete GCL), [ADR 0137](0137-linux-gate-platform-values.md) (platform values). Roadmap item F-258
(depends on F-230 and F-063).

Number: the lead gave 0207 to F-258. Main d953cd6 holds 0206 as its highest; 0204 is on the branch
`f291-gradient-options` and 0205 on `f295-seam-diffusivity`. No branch holds a 0207.

House rules ([ADR 0130](0130-wave48a-overview-and-run-record-contract.md), ADR 0191, ADR 0206):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the measured number recorded, and the
  cause is diagnosed, not tuned away;
- a defective set-up gets a new corrected test by an amendment written before the corrected run, and the old one stays as
  an ignored record;
- every new code path is opt-in: a run that does not call the new functions and has no characteristic condition on a
  moving face runs as before, bit for bit (R1 to R4);
- no case-schema key is added (S2), so neither `crates/rcfd-api/tests/baseline_hashes.rs` nor the record R0
  (`case_hashes_w48u.rs`) is touched, no golden is re-recorded, and no existing test is edited;
- each registered run is made once, and nothing is re-run to pick a result;
- no new third-party crate, no commit trailers.

Clean-room: the conditions below are derived from the Galilean invariance of the Euler equations and the
one-dimensional characteristic analysis that ADR 0006 already uses. No source was read for this work, and no vendor code,
text or install was used.

## What F-258 asks

The roadmap entry: compressible ALE boundaries and time step. Every limit not stated there is registered here, before the
run:

- `max_dt` uses `|u.n - w_n| + c`, checked against the closed form for a uniform gas on a mesh moving at `w` to round-off;
- uniform flow is preserved to 1e-12 with characteristic boundary faces that move;
- a piston-generated simple wave leaving through a moving characteristic outlet reflects with amplitude below the ADR
  limit.

Out of scope there: sliding interfaces (F-250), elastic-analogy smoothing (F-063), six-DOF coupling (F-244), more than one
moving layered boundary, non-planar layer surfaces, turbulence or energy in the incompressible moving-mesh path. The note:
split from F-247 on 2026-10-22, size S to M.

**Reading of the acceptance (registered here).**

1. "`max_dt` uses" is read as "the stable explicit step of the compressible solver on a moving mesh uses". The function
   `CompressibleSolver::max_dt()` itself stays the absolute step: the fixed-mesh path (`step`, `run_to`, `run_steady`,
   `rcfd-api`'s `CompState::advance`) and every existing moving run (ADR 0090's and ADR 0098's piston loops, the moving
   step of ADR 0191 W4 and the library loops of its tests) call it, and their bits must not change. The mesh-speed step is
   a new function (S1). Whether the existing moving runs switch to it is the maintainer's (Q2).
2. "to round-off" is registered as a relative difference of `1e-12`.
3. "Characteristic boundary faces" are the four conditions of ADR 0006 that `step_ale` refuses on a moving face today:
   `TotalInlet`, `PressureOutlet`, `NonReflecting` and `FarField`.
4. "Reflects with amplitude below the ADR limit": the amplitude is measured in pressure, against the incident pressure
   jump of the wave (T3).
5. The acceptance is library work. The case tree is not in it: its wiring (lifting ADR 0191's refusal E5, the time step of
   ADR 0191 W4) is the open part of F-259 (ADR 0191 W7 and decision record Q3) and is not built here (S2, Q1).

## What the code read shows (main d953cd6, read only)

1. **`step_ale(new_geom, dt)`** (`crates/rcfd-fv/src/compressible.rs`) takes the swept volumes `W_f` of
   `rcfd_mesh::motion::swept_volumes` from the old points to the new ones, sets the new geometry, and computes the face
   normal mesh speed `wn[f] = W_f / (dt |S_f|)` with `S_f` of the new mesh. It calls `flux_sum(u, Some(&wn))` in every
   SSP-RK stage and divides the stage totals by the stage volumes `V^n + c dV` (ADR 0090 D2). It does not keep `wn`.
2. **`flux_sum`**, boundary faces: with `vn = wn[f]` not exactly `0.0` and a slip wall, the ghost is the mirror relative to
   the wall velocity `v = vn n`. With `vn != 0.0` and a characteristic condition (`face_state` gives `Some`) it returns the
   error "compressible ALE: patch ... moves but its characteristic boundary condition does not support mesh motion". On a
   static face (`vn == 0.0`) a characteristic condition gives the face state `fs` of `compressible_bc`; it becomes the
   ghost and the face value of the gradients, and the face flux is the physical flux `fs.flux(gas, n)` ("direct" faces).
   Fixed and extrapolated faces go through `face_flux`, which with `vn != 0.0` solves the Riemann problem in the frame of
   the face and transforms the flux back (`F_rho = F'_rho`, `F_m = F'_m + v F'_rho`, `F_E = F'_E + v.F'_m + |v|^2/2 F'_rho`,
   ADR 0090 D2): the fixed state is taken as a lab-frame state and shifted into the face frame there.
3. **`max_dt()`** is `cfl min_c 2 V_c / sum_f (|u_c.n_f| + a_c) |S_f|` over the faces that are not empty, on the current
   geometry. It has no mesh speed.
4. **`replace_mesh(geom, u)`** installs any topology (layer events). **`step(dt)`** is the fixed-mesh step; it calls
   `residual`, which calls `flux_sum(u, None)`.
5. **`compressible_bc`**: `total_inlet(gas, bc, interior, n)` (subsonic inflow from `p0`, `t0`, `r_gas`, `dir`; the outgoing
   invariant `J = u.n + 2a/(gamma - 1)` from the interior; the interior state when `dir.n >= -1e-12`),
   `pressure_outlet(gas, p, sigma, interior, n)` (the interior state when `u.n >= a`; otherwise the face pressure
   `p_i + sigma (p - p_i)` with the entropy, the tangential velocity and `J` of the interior), `far_field(gas, fs, interior,
   n)` (the Riemann invariants; the free stream or the interior state when both are supersonic; entropy and tangential
   velocity from the free stream when the face normal velocity is negative). They see the interior state and the normal
   only.
6. **Who calls `step_ale` and `max_dt` on a moving mesh.** `rcfd-api` `CompState::advance_moving` (ADR 0191 W4:
   `dt = max_dt()`), the tests `layering_w38c.rs`, `ale_gcl.rs`, `exp_rot_pab.rs` (rcfd-fv), `layering_ale_f259.rs` and
   `inverse_volume_f290.rs` (rcfd-api). The case tree refuses a characteristic condition on a patch that can move
   (`dynmesh.rs` `check_compressible`, ADR 0191 E5), so through the tree the refusal of item 2 is never reached.
7. **`CompressibleSolver`** has public fields and the private `ls_inv`, so it is built only by `new`; a private field can be
   added without touching a caller.

## The model (derived)

**M1. The time step on a moving mesh.** In the frame of a face moving with the normal speed `w_n`, the Euler equations keep
their form, and the signals cross the face at `(u - v).n` and `(u - v).n +- a`, with `v = w_n n`; the largest is
`|u.n - w_n| + a`. The explicit limit of ADR 0090's form with this speed is

    dt = cfl min_c 2 V_c / sum_f (|u_c.n_f - w_n,f| + a_c) |S_f|

on the current geometry (the start of the step), over the faces that are not empty. For a neighbour cell the face normal
points into it, and `|u.(-n) - (-w_n)|` is the same speed. With every `w_n` zero this is `max_dt()` operation for
operation (`x - 0.0` is `x`).

For a uniform gas `(rho, u, p)` on a box of cells `h_x x h_y x h_z` translating at `w`, every cell has two faces of area
`h_y h_z` with `|u.n - w.n| = |u_x - w_x|`, and so on, so

    dt_w = cfl / sum_d (|u_d - w_d| + a) / h_d

with the sum over the directions whose faces are not empty. This is the closed form of T1.

**M2. Which `w_n`.** The `w_n` of the step `n -> n+1` is `W_f / (dt |S_f|)`, which for a prescribed motion depends on the
`dt` being chosen. Registered here: the mesh speed of the last ALE step stands in for that of the next (a lagged mesh
speed). `step_ale` keeps the `wn` it used; `max_dt_ale()` is M1 with those speeds, and `max_dt()` when there are none (no
ALE step yet, or a `step` or `replace_mesh` since). For a mesh moving at constant speed the lagged speed is the speed of
the step itself; otherwise the CFL number of a step is off by the change of the speed over one step (Q3 gives the
alternative).

**M3. A characteristic condition on a moving face.** By Galilean invariance each condition of ADR 0006 is a statement
about the state seen from the face. With `v = w_n n` and `rel(q) = (rho, u - v, p)`:

- `PressureOutlet {p}` and `NonReflecting {p, sigma}`: the face-frame state `q' = pressure_outlet(gas, p, sigma,
  rel(q_i), n)` (with `sigma = 1` for the pressure outlet). The static pressure is the same in both frames. The switch to
  the interior state reads the relative normal velocity `(u_i - v).n >= a_i`.
- `FarField(q_inf)`: `q' = far_field(gas, rel(q_inf), rel(q_i), n)`. The free stream is a lab-frame state, the convention
  of the fixed state on a moving face (read 2); its switches read relative normal velocities.
- `TotalInlet {p0, t0, r_gas, dir}`: the total conditions and the direction are lab-frame data (the same convention,
  Q4). The incoming gas has the lab velocity `V dir` and `a^2 = a0^2 - (gamma - 1) V^2 / 2`, `a0^2 = gamma r_gas t0`. The
  face-frame outgoing invariant is kept: `(V dir.n - w_n) + 2a/(gamma - 1) = (u_i.n - w_n) + 2 a_i/(gamma - 1)`. The `w_n`
  cancels, so `V`, and with it the face state, is that of the static `total_inlet(gas, bc, q_i, n)`, and
  `q' = rel(total_inlet(gas, bc, q_i, n))`.

The lab face state is `q = (rho', u' + v, p')`. It is the ghost and the face value of the gradients, as on a static face.
The face flux is the ALE flux `F(q).n - w_n U(q)`, computed as the physical flux of `q'` in the face frame and transformed
back with the identity of ADR 0090 D2 (read 2), which holds for the physical flux of any state: with `u = u' + v`,
`rho u'.n = rho (u.n - w_n)`, `rho u u'.n + p n = rho u' u'.n + p n + v rho u'.n`, and
`E u'.n + p u.n = (E' + p) u'.n + v.(rho u' u'.n + p n) + |v|^2/2 rho u'.n` with `E = E' + rho u'.v + rho |v|^2/2`.

A face whose `w_n` is exactly `0.0` keeps the static path of read 2, operation for operation.

**M4. Uniform flow (the discrete GCL).** If the data of every condition is a uniform state `q0` (for the total inlet: the
lab total conditions of `q0` and `dir = u0/|u0|`), every condition returns `q0` up to round-off in either branch of its
switches, so the flux of every boundary face is `F(q0).n - w_n U0`, as on the interior faces of a uniform state. ADR 0090
D2's argument then holds: `sum_f F(q0).S_f = 0` on a closed cell and `sum_f W_f = V^{n+1} - V^n`, so each stage gives
`(V U)_s = V_s U0` and the state stays `U0` up to round-off.

**M5. A simple wave (T3).** A piston that starts from rest in a gas at rest (`rho0`, `p0`, `a0`) and ends at the speed `U`
sends a right-running simple wave into the gas. Across it the invariant `u - 2a/(gamma - 1)` keeps its value in the gas at
rest, so behind the wave `u = U`,

    a1 = a0 + (gamma - 1) U / 2,   p1 = p0 (a1/a0)^(2 gamma/(gamma - 1)),   rho1 = rho0 (a1/a0)^(2/(gamma - 1))

as long as no shock forms. A boundary that imposes that invariant (the far field with the gas at rest as free stream), or
that adds no incoming wave (the non-reflecting outlet with `sigma = 0`, whose face state is the interior state seen from
the face), lets the wave out; a fixed static pressure `p0` (the pressure outlet) reflects it as an expansion that brings
the pressure back towards `p0`. Once the wave has left, the gas between the piston and the outlet is uniform at
`(rho1, U, p1)` for a boundary that does not reflect.

## Settings and code

**S1. Library (`rcfd-fv`, module `compressible`).**

- `CompressibleSolver::max_dt_moving(&self, wn: &[f64]) -> Result<f64>`: M1 with one `w_n` per face of the current mesh
  (in face order, the sign of `step_ale`'s `wn`: the outward normal of the owner); `Err` when `wn` has not one value per
  face or a value is not finite.
- `CompressibleSolver::mesh_speeds(&self) -> Option<&[f64]>`: the `wn` of the last `step_ale`, while no `step` and no
  `replace_mesh` came after it (M2).
- `CompressibleSolver::max_dt_ale(&self) -> f64`: `max_dt_moving(mesh_speeds())` when there are speeds, `max_dt()` otherwise
  (M2).
- `pub fn characteristic_face_state(gas, bc: &CompBc, interior: &Prim, n: Vec3, wn: f64) -> Option<Prim>`: the lab face
  state of M3 for the four characteristic conditions (`None` for `Slip`, `Extrapolate` and `Fixed`); with `wn == 0.0` the
  static face state of `compressible_bc`, bit for bit.
- `step_ale`: the refusal of read 2 is removed; a characteristic condition on a face with `vn != 0.0` takes M3 (state and
  flux); the step keeps its `wn` for `mesh_speeds`. `replace_mesh` drops the kept speeds.
- Not edited: `max_dt`, `step`, `residual`, `run_to`, `run_steady`, the static branches of `flux_sum` (no `wn`, or
  `vn == 0.0`), `face_flux`'s branch with `vn == 0.0`, `compressible_bc`. The transform of `face_flux`'s moving branch may
  move into a helper that does the same operations in the same order (R1 to R3 check it).

**S2. Case tree and CLI.** Not changed. ADR 0191's refusal E5 stays, and the tree's moving step keeps `dt = max_dt()`
(ADR 0191 W4). No key is added. The wiring is Q1 and Q2. The documents `docs/dynamic-mesh.md` and `docs/case-schema.md`
say what the library does and that the tree does not use it yet (prose only; no TOML block is edited).

## Registered checks

All tests run in release. The new checks are in `crates/rcfd-fv/tests/compressible_ale_f258.rs`, the records in
`crates/rcfd-fv/tests/compressible_ale_f258_records.rs`. Gas `gamma = 1.4`; `CompSettings::default()` (HLLC, van Leer,
SSP-RK3, CFL 0.4) unless a check says otherwise. "Relative" is `abs(a - b) / abs(b)`. The schedule of a moving run, unless
a check says otherwise: `dt = max_dt_ale()` clipped to the end time as ADR 0191's library loops clip it
(`dt.min((t_end - time).max(1e-300))`, the end reached when `time >= t_end - 1e-12 max(t_end, 1)`), and the mesh at
`time + dt`.

### T1. The mesh-speed time step (acceptance clause 1)

Set-up M3D: `box_mesh([5, 4, 3], [0.5, 0.6, 0.6])` (`h = (0.1, 0.15, 0.2)`), the uniform state `rho = 1`, `p = 1`,
`u0 = (0.3, -0.2, 0.1)` (`a = sqrt(1.4)`), every patch `Fixed` to it; the mesh translates at `w = (-0.5, 0.25, 0.4)`
(points `p0 + w t`); 10 steps. Set-up M2D: `box_2d(6, 4, 0.6, 0.4)` (`h = (0.1, 0.1)`, the z faces empty),
`u0 = (0.3, -0.2, 0)`, `w = (-0.6, 0.3, 0)`, otherwise as M3D. `dt_w` is the closed form of M1.

| Id | Check | Limit |
| --- | --- | --- |
| T1a (acceptance) | M3D and M2D: before each of steps 2 to 10, `max_dt_ale()` and `max_dt_moving(mesh_speeds())` against `dt_w` | relative `1e-12` |
| T1b | M3D and M2D, before step 1 and after step 10: `max_dt_moving` with every `w_n = 0` against `max_dt()`; `max_dt()` against the closed form with `w = 0` | bit for bit; relative `1e-12` |
| T1n (negative control) | M3D and M2D before step 2: `max_dt()` against `dt_w` | relative `>= 0.05` |
| T1c | the API: `mesh_speeds()` is `None` on a new solver, `Some` after `step_ale`, `None` after a `step` and after a `replace_mesh`; with `None`, `max_dt_ale()` equals `max_dt()` bit for bit; `max_dt_moving` with a list one short and with a NaN gives `Err` | as stated |

T1n shows that T1a can fail: the closed form with the mesh speed differs from the absolute step by 25 and 31 percent on
the two set-ups (M1 with these numbers).

### T2. Uniform flow with moving characteristic faces (acceptance clause 2)

Set-up U: `box_2d(12, 8, 1.2, 0.8)`; the uniform state `rho0 = 1`, `p0 = 1`, `u0 = (0.3, 0.1, 0)`. Conditions:

- `xmin`: `TotalInlet` with `r_gas = 1`, `t0 = 1 + 0.2 |u0|^2 / 1.4` (the static temperature `p/(rho r_gas) = 1`),
  `p0 = t0^3.5`, `dir = u0 / |u0|` (the lab total conditions of the state);
- `xmax`: `PressureOutlet {p: 1}`;
- `ymin`: `FarField(q0)`;
- `ymax`: `NonReflecting {p: 1, sigma: 0.5}`.

The motion of an initial point `(x, y, z)` at time `t`, with `s1 = sin(2 pi t)`, `s2 = sin(4 pi t)`, the centre
`c = (0.6, 0.4)`:

    q = (x, y) + 0.01 s1 sin(pi x / 1.2) sin(pi y / 0.8) (1, 1)
    r = c + R(0.05 s1) diag(1 + 0.03 s1, 1 + 0.02 s2) (q - c) + (0.02 s1, 0.01 s2)

(`R` the rotation about z), `z` unchanged; to `t_end = 2`. Every boundary face moves normal to itself through the dilation,
the rotation and the translation, and the interior deforms.

| Id | Check | Limit |
| --- | --- | --- |
| U1 (acceptance) | after every step, over the cells: `max(abs(rho/rho0 - 1), abs(u - u0)/abs(u0), abs(p/p0 - 1))` (`abs` of a vector its length) | `<= 1e-12` |
| U2 (set-up) | steps; on each of the four patches the largest `abs(w_n)` of `mesh_speeds()` over the run | `>= 100`; `>= 0.02` each |

### T3. A simple wave through a moving characteristic outlet (acceptance clause 3)

Set-up S: `box_2d(200, 1, 1.0, 0.1)`; gas at rest, `rho0 = 1.4`, `p0 = 1` (`a0 = 1`). Every patch is a slip wall except
`xmax`. A point at the initial `x` (in `[0, 1]`) sits at `x_p(t) + (x_o(t) - x_p(t)) x` (y and z unchanged), so the cells
stay uniform between the piston `xmin` at `x_p` and the outlet `xmax` at `x_o`:

- the piston: `u_p(t) = (U/2) (1 - cos(pi t / tau))` for `t <= tau`, `U` after, with `U = 0.1`, `tau = 0.25`;
  `x_p(t) = (U/2) (t - (tau/pi) sin(pi t / tau))` for `t <= tau`, `U tau / 2 + U (t - tau)` after;
- the outlet: `x_o(t) = 1 + W t`, `W = -0.2` (towards the piston; the gas leaves through it at a relative Mach number of
  0.2 before the wave and 0.3 after).

M5 gives `a1 = 1.02`, `p1 = 1.02^7 = 1.148686`, `rho1 = 1.4 x 1.02^5`, and the incident jump `dp = p1 - p0`. From the
characteristics: the front (speed 1) meets the outlet at `t = 1/1.2 = 0.833`; the tail (from `x_p(tau) = 0.0125` at
`t = tau`, speed `U + a1 = 1.12`) at `t = 0.960`; a wave reflected at the outlet from `t = 0.833` on runs back at `u - a`,
between -1 (the gas at rest) and `U - a1 = -0.92`, and reaches the piston not before `t = 1.52`, so whatever the outlet
reflects is still in the column in the window below (the piston is a wall). The steepest characteristics (from the largest piston acceleration `U pi / (2 tau)`
at `t = tau/2`) would cross after `t - tau/2 = 2 (a0 + (gamma - 1) u_p / 2) / ((gamma + 1) U pi / (2 tau)) = 1.34`, at
`t = 1.47`, beyond the outlet, which they reach at about `t = 0.9`: the wave leaves without a shock. In the window
`[1.1, 1.5]` the column of a boundary that does not reflect is uniform at `(rho1, U, p1)`.

The reflection `R` of a run is the largest `abs(p_c - p1) / dp` over the cells and over the ends of the steps in
`[1.1, 1.5]`. Each run takes the schedule above with its steps clipped to land on `t = 0.7` and `t = 1.5`, and ends at
`1.5`.

| Id | Check | Limit |
| --- | --- | --- |
| S0 (set-up) | far-field run at `t = 0.7` (front at 0.7, tail at 0.5165, outlet at 0.86): cells with centres in `[x_p, 0.4]` against `p1`; cells with centres in `[0.75, x_o]` against `p0`; both relative to `dp` | `<= 1e-2` each |
| S1 (acceptance) | the outlet `FarField(rest)` (the gas at rest ahead of the wave as free stream): `R` | `<= 1e-2` |
| S2 | the outlet `NonReflecting {p: 1, sigma: 0}`: `R` | `<= 1e-2` |
| SN (negative control) | the outlet `PressureOutlet {p: 1}`: `R` | `>= 0.5` |

Printed, no limit: `R` with `NonReflecting {p: 1, sigma: 0.25}`; the step counts; the times.

SN shows that the measure sees a reflection. The limit of S1 and S2, 1 percent of the incident jump, is registered here;
the scheme is second order inside and the conditions use the cell state next to the face, so the expected `R` is well
below it.

### T4. The face state of a moving face (unit)

| Id | Check | Limit |
| --- | --- | --- |
| T4a | `characteristic_face_state` with `wn = 0.0` against the functions of `compressible_bc`, for each of the four conditions and 20 pseudo-random interior states and normals (sub- and supersonic) | bit for bit |
| T4b | the frame of the switch, `PressureOutlet {p: 0.5}`, `n = (1, 0, 0)`, interior `rho = 1`, `p = 1`, `u = (0.5 a, 0.2, 0)`: with `wn = 0` the face pressure is `0.5`; with `wn = -0.6 a` (relative outflow at `1.1 a`) the face state is the interior state; interior `u = (1.2 a, 0.2, 0)` with `wn = +0.5 a` (relative `0.7 a`): the face pressure is `0.5` | relative `1e-14`; `1e-14` each component; `0.5` to `1e-14` |
| T4c | the frame of the far field, free stream at rest (`rho = 1`, `p = 1`), interior `rho = 1.1`, `p = 1`, `u = (0.3 a, 0.4, 0)`, `n = (1, 0, 0)`, `wn = +0.5 a` (relative normal velocity of the face state negative: inflow seen from the face): the face state's tangential velocity and `p / rho^gamma` against the free stream's | `<= 1e-14`; relative `1e-12` |
| T4d | `TotalInlet` (`p0 = 1.2`, `t0 = 1.05`, `r_gas = 1`, `dir = (-1, 0, 0)`, interior `rho = 1`, `p = 1`, `u = (-0.2, 0.05, 0)`, `n = (1, 0, 0)`) with `wn` of `-0.3` and `0.2` against `wn = 0` | bit for bit |

In T4b and T4c `a = sqrt(1.4)`, the sound speed of `rho = 1`, `p = 1`. In T4c the face frame sees the face normal velocity
`-0.55` (inflow: entropy and tangential velocity from the free stream); the lab-frame evaluation (read 5) sees `+0.04`
(outflow) and takes the interior's tangential velocity 0.4, so T4c tells the two frames apart; T4b does so for the
pressure outlet (the lab frame imposes the pressure in its second part and keeps the interior state in its third).

### D. Defaults

| Id | Check | Limit |
| --- | --- | --- |
| D1 | records R1 to R4 below, after the code | equal (Windows; printed elsewhere, ADR 0137) |
| D2 | the existing suites, unedited: `rcfd-fv` (the unit tests of `compressible` and `compressible_bc`, `layering_w38c`, `ale_gcl`, `exp_rot_pab`), `rcfd-api` (`layering_ale_f259` with its D0 hashes and the refusal E5 in L, `inverse_volume_f290`, `compressible_bc_case`, `far_field_w49f`, `baseline_hashes`, H1 of `case_hashes_w48u`), `rcfd-cli` `layering_ale_f259` | pass |

The records, made on main d953cd6 (Windows, release, twice, the same values): FNV-1a 64 of the bits of every conserved
value, the time, the step count and the cell count at the end of a library run whose every step is `max_dt()`:

| Record | Run | Steps | Hash |
| --- | --- | --- | --- |
| R1 | ADR 0090's piston P (40 x 2 x 2 layers, slip walls, period 16) to `t = 4` on ADR 0090's schedule, with its 3 layer events | 1104 | `6c63454d45c1e84b` |
| R2 | ADR 0191's P-B as a library loop: smoothing (`xmax` prescribed, `xmin` fixed), a static `PressureOutlet` at `xmin`, to `t = 1` | 138 | `56d082d8b41c8da9` |
| R3 | a rigid rotation and translation of `box_2d(20, 4, 1, 0.2)` with an isentropic density pulse in a uniform stream, `Fixed` on every patch, to `t = 0.5` | 71 | `6e414e610c2290bd` |
| R4 | the fixed-mesh `step` on `box_2d(30, 10, 1.5, 0.5)` with `TotalInlet`, `PressureOutlet`, `FarField` and `NonReflecting` on a stream with a pressure pulse, 150 steps | 150 | `3912775885144ca1` |

R1 to R3 run the moving branches of `flux_sum` that S1 keeps (moving slip walls, moving fixed states, a static
characteristic face on a moving mesh, whose swept volume is exactly 0); R4 runs its static characteristic branch.

### Mutations (scratch, not committed)

Each is applied alone to the final tree, the named test is run once, and it must fail; the file is restored and checked
byte for byte against a kept copy. Their numbers go in the addendum. `scripts/limit_audit.py` is run on the new test files
and every hit is read.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | `max_dt_moving` drops `w_n` (`|u.n| + a`) | T1a |
| MU2 | a moving characteristic face takes the lab physical flux `F(q).n` (the static branch) in place of the ALE flux | U1 |
| MU3 | `characteristic_face_state` evaluates the pressure-type conditions and the far field with the unshifted interior state and free stream | T4b, T4c |
| MU4 | `step_ale` does not keep its `wn` (`mesh_speeds()` stays `None`) | T1a |

### The gate

`cargo fmt` per package, `cargo clippy --workspace --all-targets -- -D warnings`, and every test in release on a
`git archive` copy of the gated commit, one process per test, three jobs (`scripts/gate_test.py`), in the target dir
`E:/projects/cfd/w49i-work/target-f258-gate`, taken in turn through the machine's gate lock and started with at least
20 GB of physical memory free. A failure of a known flaky test is run again alone on the same build and reported with its
name.

### Classes

D1 and D2 are preconditions of landing: a miss means the change is not merged and the cause is recorded. A miss of any
other limit is reported with its number and kept as an ignored `*_known_miss` test with the limit intact. A set-up check
(U2, S0) or a negative control (T1n, SN) that does not hold, or a mutation that does not make its check fail, means the
check is not shown to be able to fail: it is reported as a defective set-up, corrected by an amendment before any re-run,
and never counted as a pass.

**Decision rule.** T1a, U1 and S1 met (with D1, D2): the acceptance is met in the library, and F-258 is set to partial
(`[~]`) with that reason, because marking it done is the maintainer's (Q5). A miss of one of them: partial with the miss
named. No moving characteristic face that runs at all: todo.

## Not delivered

The case-tree and CLI wiring (E5 stays, the tree's moving step keeps `max_dt()`; Q1, Q2); a mesh-speed step with predicted
speeds (Q3); total conditions in the frame of the face (Q4); a total inlet whose face moves into the domain faster than the
inflow (seen from the face the gas then leaves; the condition is applied as written, as on a static mesh with backflow);
everything outside F-258 (the roadmap's out-of-scope list).

## Open questions and choices for the maintainer

- **Q1. The case-tree wiring.** Not in F-258's acceptance; the open part of F-259 (ADR 0191 W7). Options: (a) wire it under
  F-259's checks (located errors, a tree run equal to the library run to 1e-12, a case without the new behaviour
  bit-identical): lift E5 for the four conditions, with a registered edit of the three E5 inputs of ADR 0191's test L
  (they become runs, kept as ignored records by the house rule, with new controls); (b) keep the refusal. Recommendation
  (a), as a follow-up registration under F-259.
- **Q2. The time step of the existing moving runs.** ADR 0191 W4's tree step and the library loops of ADR 0090, 0098 and
  0191 use `max_dt()`. Options: (a) keep it (no bit changes); (b) make `max_dt_ale()` the step of the tree's moving
  compressible runs, which changes the bits of every such run and needs a registered edit of ADR 0191's library loops and
  the maintainer's lift of the bit-identical default for that path; (c) an opt-in key (it would go in an optional table so
  that H1's record R0 needs no edit). Recommendation (b) together with Q1 (a): the absolute step lets the CFL number grow by
  `|w| / (|u| + a)` at a fast wall (up to 0.49 for the registered 0.4 at ADR 0098's Mach 0.2 piston).
- **Q3. Lagged or predicted mesh speed.** Options: (a) the speed of the last ALE step (as registered, M2); (b) a predictor
  per step: move to `t + dt0`, take those speeds, `dt1 = max_dt_moving`, move to `t + dt1` (two motion evaluations, and two
  displacement solves for a smoothing motion). Recommendation (a): the lag is one step of the speed's change, small next to
  the CFL margin for the motions of the checks.
- **Q4. The frame of the total-inlet data on a moving face.** Options: (a) lab-frame total conditions and direction (as
  registered, M3; the face state is the static one, only the flux changes); (b) total conditions relative to the face, as
  for the inlet of a moving body's own frame. Recommendation (a), the convention of the fixed state and the far field on a
  moving face; (b) as a separate option if a case needs it.
- **Q5. Status of F-258.** If T1a, U1 and S1 are met, the acceptance as written is met in the library. Options: (a) done,
  with the wiring tracked by F-259; (b) partial until the tree uses it. Recommendation (a). The roadmap says partial until
  the maintainer decides.
