# ADR 0006: Compressible boundary conditions (F-062)

Status: accepted. Scope: the density-based explicit Euler solver (`rcfd-fv` `compressible.rs`, F-060). The
pressure-based variable-density kernel (ADR 0005, `varden.rs`) keeps its own stagnation inlet and is not touched.

## Decision

New module `crates/rcfd-fv/src/compressible_bc.rs`. Four boundary types are added to `CompBc`. All of them
compute a *boundary face state* from the interior cell state and the outward unit normal `n`, by a one-dimensional
characteristic analysis normal to the face (Thompson, J. Comput. Phys. 68 (1987) 1-24; Poinsot and Lele,
J. Comput. Phys. 101 (1992) 104-129; Blazek, *Computational Fluid Dynamics: Principles and Applications*, ch. 8).
The solver then uses the *physical* flux of that face state (no Riemann solve against a ghost state). Perfect gas
with `gamma` from the solver and, for total/static temperature, the specific gas constant `R` carried by the BC.

* `TotalInlet { p0, t0, r_gas, dir }` (subsonic inflow, `dir` = unit flow direction): the characteristic leaving
  the domain carries `J = u_n + 2 a/(gamma-1)` from the interior; the face state satisfies `p0`, `T0` isentropically
  with velocity along `dir`. The speed `V` solves `V (dir.n) + 2 a(V)/(gamma-1) = J`, `a^2 = a0^2 - (gamma-1) V^2/2`
  (monotone in `V` for `dir.n < 0`, bisection). If the interior flow is not entering (`u_n >= 0` at the solution) or
  no root exists, the speed is clamped to the nearest admissible value (`V = 0`).
* `PressureOutlet { p }`: if the interior normal Mach number is `>= 1` the face state is the interior state
  (supersonic outflow); otherwise `p_f = p`, with `rho` and `u_n` corrected along the outgoing-to-the-interior
  incoming simple wave (exact relations: `rho_f = rho (p_f/p)^(1/gamma)`, `a_f = a (p_f/p)^((gamma-1)/(2 gamma))`,
  `u_n,f = u_n - 2 (a_f - a)/(gamma-1)`), tangential velocity
  and entropy extrapolated. Backflow (`u_n < 0`) uses the same subsonic formulas; it is not specially treated.
* `NonReflecting { p, sigma }`: same incoming-characteristic correction with a partial target,
  `dp = sigma (p - p_i)`: `sigma = 1` is exactly `PressureOutlet`, `sigma = 0` lets outgoing waves pass untouched
  (no pressure imposed). To first order a normally incident acoustic wave is reflected with coefficient `-sigma`.
  This is the stateless equivalent of the Poinsot-Lele pressure-relaxation term `K (p - p_ref)`; `sigma` is
  dimensionless (default 0.02, fixed *before* any run; see the checks). Waves arriving obliquely are NOT treated
  (one-dimensional characteristics; Giles-type multi-dimensional non-reflecting conditions are not implemented).
* `FarField { state }` (free-stream `rho, u, p` from Mach, flow angle, static `p`, `T`): Riemann-invariant boundary
  (Blazek section 8.3; Jameson et al.): `R_out = u_n,i + 2 a_i/(gamma-1)`, `R_in = u_n,inf - 2 a_inf/(gamma-1)`;
  `u_n = (R_out+R_in)/2`, `a = (gamma-1)(R_out-R_in)/4`; entropy and tangential velocity come from the free
  stream on inflow, from the interior on outflow; supersonic faces take the free stream (inflow) or the interior
  (outflow) state entirely.

Schema (`boundaries.<patch>`): `compressible_bc` gains `total_inlet`, `pressure_outlet`, `farfield`,
`nonreflecting`. Keys: `total_pressure_value`, `temperature_value` (total for `total_inlet`, static for `farfield`),
`flow_direction` (vector, `total_inlet`; zero = opposite of the patch normal), `pressure_value` (static),
`mach_value`, `flow_angle` (degrees in the x-y plane from +x, `farfield`), `reflection_value` (`nonreflecting`).
Precise validate errors name the key.

## Pre-registered validation checks (written before any run; limits are not changed after a run)

Defaults of the solver are used (HLLC, MUSCL van Leer, SSP-RK3, CFL 0.4); nothing is tuned per test. `gamma = 1.4`.

**(A) Algebraic face-state checks (fast unit tests).** (A1) `TotalInlet` for 20 arbitrary subsonic interior
states and directions: reconstructing `p0` and `T0` from the face state gives relative error `<= 1e-12`; the face
velocity direction equals `dir` to `1e-12`; the invariant `J` equals the interior value to `1e-12`. (A2) `FarField`
with the interior equal to the free stream returns the free stream to `1e-12` for subsonic inflow, subsonic
outflow, supersonic inflow and supersonic outflow. (A3) `PressureOutlet`: supersonic gives the interior state
exactly; subsonic gives `p_f = p` to `1e-14`, tangential velocity and `p/rho^gamma` preserved to `1e-12`.
(A4) `NonReflecting` with `sigma = 1` equals `PressureOutlet` to `1e-14`; with `sigma = 0` equals the interior state.
(A5) Uniform free stream (M 0.8, 1.25 degrees) in a box with `FarField` on all four sides stays uniform to `1e-12`
after 50 steps.

**(B) No spurious reflection (acceptance part 2).** Quiet gas `rho = 1`, `p = 1/gamma` (`a = 1`), `u = 0`,
1-D channel `x in [0,1]` on `box_2d(400, 1)`, left boundary a fixed full state, right boundary under test. Initial
right-running isentropic Gaussian pressure pulse `dp = 1e-3 p0 exp(-((x-0.5)/0.03)^2)` with the simple-wave
`d rho = dp/a^2`, `du = dp/(rho a)`. The incident amplitude `A_inc` is the maximum of `|p - p0|` at `t = 0.25`
(before the pulse reaches the boundary, so numerical diffusion cancels in the ratio). The reflected amplitude
`A_ref` is the maximum of `|p - p0|` over the whole domain at `t = 1.0` (the incident pulse left at `t ~ 0.6`; the
reflected pulse is at `x ~ 0.5`). Ratio `R = A_ref / A_inc`. Limits: `PressureOutlet` (the plain fixed-pressure
boundary) `0.8 <= R <= 1.2` (this shows that the test sees a reflection); `NonReflecting` with `sigma = 0.02`:
`R <= 0.05` and `R <= 0.1 * R(PressureOutlet)`; `NonReflecting` with `sigma = 0`: `R <= 0.02`. The legacy
`Extrapolate` and the full-state `Fixed` are measured and printed without limits.

**(C) Total-pressure inlet reproduces the F-061 isentropic nozzle.** The F-061 subsonic geometry (ADR 0005 (a1)):
2-D planar converging-diverging duct, half height `h(x) = h_t + (h_in - h_t)(1 + cos(2 pi x/L))/2`,
`h_in = h_e = 2 h_t`, `h_t = 0.05 m`, `L = 40 h_t`, slip walls, 200 x 10 cells, `R = 287`, `p0 = 101325`,
`T0 = 300`. Boundaries: `TotalInlet` (direction +x) and `PressureOutlet` with `p_e` from the isentropic relation at
`A_e/A* = 2 A_t/A*`, throat Mach 0.3 (`A_t/A* = 2.0351`). The density-based solver is marched until the maximum
relative density change per step is below `1e-9` (at most 200000 steps; not reaching it is a miss). Limits are the
F-061 ones: volume-weighted column means at `i = 10, 30, ..., 190`: Mach within 1.5 %, `p/p0` within 0.5 %, `T/T0`
within 0.3 % of the area-Mach relation; inlet mass flow within 1.0 % of `A* p0 sqrt(gamma/(R T0)) (2/(gamma+1))^((gamma+1)/(2(gamma-1)))`
with `A* = h_t/2.0351` per unit depth; inlet/outlet mass flow equal to `1e-3` (explicit steady state; looser than
the pressure-based `1e-6` of F-061 because a transient marcher stops on a finite step change). Heavy, `#[ignore]`.
Reference values: standard isentropic relations (Anderson, *Modern Compressible Flow*), same as F-061.

**(D) NACA 0012 transonic Cp within published scatter (acceptance part 1): NOT ATTEMPTED.** The kernel could run
an Euler airfoil (an O-grid from `cylinder_ogrid` plus `warp`), but no quantitative published reference is
accessible: the AGARD AR-211 data and the Vassberg and Jameson, J. Aircraft 47(4) (2010) 1152-1166 Euler results
were not retrievable here (the HiOCFD case page, arXiv 2407.03746 and arXiv 2009.07096 were fetched; none prints
Cl, Cd, shock positions or a Cp table in extractable text). A gate with values recalled from memory is not allowed,
and a different test (cylinder, free-stream preservation) would be a weaker substitute, so the criterion is
registered as not attempted and F-062 stays `partial`. Test `naca0012_transonic_cp_known_miss` is an `#[ignore]`d
placeholder that fails with this message.

**(E) Schema and API (fast).** `validate` returns a located error naming the key for: `total_inlet` without positive
`total_pressure_value` or `temperature_value`; a zero-length non-empty `flow_direction`; `pressure_outlet` /
`nonreflecting` / `farfield` without positive `pressure_value`; `farfield` without positive `temperature_value` or
negative `mach_value`; `reflection_value` outside `[0,1]`; any of these BCs without the compressible algorithm.
A small duct case with `total_inlet` + `pressure_outlet` runs 200 steps through the API with finite state.

## Consequences / limitations

* Only normally incident waves are treated as non-reflecting; oblique incidence reflects.
* No vortex correction for lifting-airfoil far fields; a far-field must be placed far from the body.
* Total inlet is for subsonic inflow with prescribed direction; supersonic inflow uses `fixed`.
