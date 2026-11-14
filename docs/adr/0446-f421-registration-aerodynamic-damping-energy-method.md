# ADR 0446: F-421 registration: aerodynamic damping and added mass of a mode by prescribed motion of the wall (energy method)

Status: **REGISTERED 2026-11-14, written before any code of F-421 and before any run of its checks.** Not yet accepted by the
maintainer. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parents: the design stub [ADR 0324](0324-aerodynamic-damping-by-the-energy-method.md) (SA-14) and the advanced-tier plan
[ADR 0310](0310-structural-solver-advanced-tier-plan.md). Builds on [ADR 0440](0440-f408-registration-advanced-tier-scaffold.md)
(F-408: the declared module `rcfd_post::solid_aerodamp`, which this item fills), [ADR 0434](0434-f401-registration-piston-on-an-elastic-bar.md)
(F-401: the open column, `ChannelMotion`, the discrete model of the pairing D2, the viscous miss VS), [ADR 0410](0410-f400-registration-dynamic-solid-participant.md)
(F-400: `DynamicSolid`), [ADR 0263](0263-f362-registration-interface-mapping.md) (F-362: `InterfaceMap::build`, the transposed
map conserves work), [ADR 0284](0284-mass-matrix-and-modal-analysis.md) (F-383: the consistent mass matrix), [ADR
0252](0252-f351-registration-added-mass-deficit-wall-pressure.md) (F-351: the prescribed wall gradient of `FlowWall`). Roadmap
item F-421 (deps F-408, F-362, F-383, F-400, F-401). Size M. Number 0446 given by the lead. Branch `f421-aero-damping` (from main
`46d78b64`), worktree `w49i-work/f421`, scratch `w49i-work/f421-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs; each registered run made once; a
missed limit becomes an ignored `*_known_miss` test with the limit intact; defaults bit-identical; no edit of
`baseline_hashes.rs`, the hash tables or a golden file; existing tests are not edited; no new dependency, no unsafe code;
clean-room (every formula below is derived here; the energy method is the work integral of the interface force over a cycle,
Carta, J. Eng. Power 89 (1967) 419, cited from memory as the stub does; nothing of it is used beyond the first-principles
definition below).

## What F-421 asks

The acceptance of the roadmap entry, binding: "Every limit not stated here is registered in the ADR before the run. Force
harmonics: for a synthetic force history with known in-phase and quadrature parts the extraction returns them within 1e-10, and
the work per cycle equals pi q0 times the quadrature part within 1e-10. Piston on a column of incompressible fluid with an open
end (the case of F-401): added mass rho_f A L_f within 2 percent and a damping ratio below the registered viscous and time-step
estimate. Piston driving a plane channel of gap h and length L_f: force per piston velocity equal to A times the
oscillating-channel impedance i w rho L_f/(1 - tanh(kappa)/kappa), kappa = (h/2) sqrt(i w/nu), real and imaginary parts within
3 percent at Womersley numbers 1 and 5 after the registered extrapolation in the time step, and the low-frequency limit 12 mu
L_f/h^2 within 3 percent. The damping ratio of the bar on the open column from this method and from the decay of the coupled
run of F-401 are both recorded. A history that has not become periodic within the registered tolerance is refused with the
cycle-to-cycle change."

Clauses: (c0) every other limit registered here; (c1) synthetic history: in-phase and quadrature parts within `1e-10`; (c2)
work per cycle equals `pi q0` times the quadrature part within `1e-10`; (c3) open column: added mass `rho_f A L_f` within 2 %;
(c4) open column: damping ratio below the registered viscous and time-step estimate; (c5) plane channel: real and imaginary parts
of the force per piston velocity within 3 % of `A i w rho L_f/(1 - tanh(kappa)/kappa)` at Womersley 1 and 5 after the registered
time-step extrapolation; (c6) the low-frequency limit `12 mu L_f/h^2` within 3 %; (c7) the bar's damping ratio from this method
and from the decay of F-401's coupled run, both recorded; (c8) a non-periodic history refused with the cycle-to-cycle change.

## What was read (no Rust run)

ADR 0324 (whole), 0434 (whole, with amendment 1 and the addendum: the VS miss), the F-408 lines of ADR 0440 that declare
`solid_aerodamp`; `crates/rcfd-post/src/solid_fsi.rs` (whole), `crates/rcfd-post/src/solid_aerodamp.rs` (the stub),
`crates/rcfd-fv/src/cosim.rs` (the traits, `FlowWall`, `InterfaceMap`), the set-ups of `crates/rcfd-post/tests/si_fsi_piston.rs`
(bar, channel, flow, parts, run), `rcfd_fem::mass::consistent`, `rcfd_mesh::gen::Grading`, `PimpleSettings::piso`. Nothing was
built or run in Rust.

**Arithmetic before this registration (disclosed).** One Python script, `w49i-work/f421-scratch/prearith.py` (numpy), on the
closed form of D3 and on the one-dimensional models of D4 (time-discrete, and with the cell-centred finite-volume profile across
the gap): the values and errors quoted in D3, D4, D6 and the estimate column below; the transient decay of D4; the column
numbers of D5 (`tan(w dt)/2 = 3.14573e-2`, `g cos(w dt) = 0.997698` at 100 steps per period); the bar numbers of D7 with the
continuous mode (`m_s` 12401.66 kg at ratio 0.5, 10799.26 kg at ratio 4; `zeta_method` `1.7557e-2`, `2.8961e-2`) against the
decay computed from F-401's recorded energy ratios (`0.10722`, `0.026602` in ADR 0434: `1.7769e-2`, `2.8861e-2`, differences
`-1.2e-2`, `+3.5e-3`); the geometric gradings (`Expansion(21.26)` gives a first cell `h/8` on 120 cells over `100 h`,
`Expansion(5.52)` a first cell `h/4` on 30 cells over `20 h`; rounded below to 21 and 5.5). The tests compute their own
predictions; no printed number is pasted into a test, and F-401's energy ratios are not used by a test (the coupled run is made
again, D7).

## Design

### D1. Harmonics, work, refusal (the library, `rcfd_post::solid_aerodamp`)

The mode coordinate is prescribed as `q(t) = q0 sin(w t)`, `t` measured from the start of the run, the wall at its reference at
`t = 0`. The generalised force `Q(t)` (the fluid's force on the structure projected on the mode, D2) is fitted, by least
squares over a set of samples `(t_j, Q_j)`, by

`Q(t) ~ Q_m + Q_p sin(w t) + Q_q cos(w t)`:

`Q_p` the **in-phase** part (with the displacement), `Q_q` the **quadrature** part (with the velocity `q0 w cos(w t)`). The
normal equations are a 3 x 3 symmetric system solved by Gaussian elimination with partial pivoting. On `N` uniform samples that
cover one period (`t_j = t_0 + j dt`, `N dt w = 2 pi`) the columns are orthogonal and the fit is the discrete Fourier
coefficients, exact for any trigonometric polynomial of degree below `N/2` other than the first harmonic's (no leakage of the
mean or higher harmonics).

Linear mode with fluid: `Q = -m_a q'' - c q' - k_a q`, so `Q_p = (m_a w^2 - k_a) q0`, `Q_q = -c w q0`. The library reports the
**added mass** `m_a = Q_p/(w^2 q0)` (the in-phase part read as a mass; `-Q_p/q0` read as a stiffness is also given), the
**damping** `c = -Q_q/(w q0)`, the **impedance** (force resisting the motion per velocity, `e^{i w t}` convention) `Z = c + i w
m_a`, and the **work per cycle** of the fluid on the structure

`W = integral over a period of Q dq = integral Q q0 w cos(w t) dt = pi q0 Q_q`

(the sine and the mean integrate to zero against the cosine). `W < 0` is damping. The **damping ratio** of a mode of modal mass
`m` (the whole mass that moves with the mode, the structure's and the fluid's in-phase part) is `zeta = c/(2 m w) = -W/(2 pi w^2
q0^2 m) = -W/(4 pi E)`, `E = m w^2 q0^2/2` the energy of the mode.

The library's independent work integral (for c2): on uniform samples of one period, the periodic trapezoid rule `W_trap = sum_j
Q_j q0 w cos(w t_j) dt`, which is exact (to rounding) for any trigonometric polynomial of degree below `N - 1`; so `W_trap = pi q0
Q_q` holds to rounding on such a history, by a different computation.

**Periodicity and refusal.** A run of `C` cycles of `N` steps gives samples at `t_n = n dt`, `n = 1 .. C N` (the force at the end
of every step, the time the coupling sees it). Cycle `c` is `n = (c - 1) N + 1 ..= c N`. Each cycle is fitted; the **cycle-to-cycle
change** is `delta = |H_C - H_{C-1}|/|H_C|`, `H = (Q_p, Q_q)`, Euclidean norms. The response (the last cycle's fit) is returned
only if `delta <= tolerance`; otherwise the error "the force history is not periodic: the cycle-to-cycle change of the first
harmonic between cycles C-1 and C is <delta> (tolerance <tol>)" with `delta` printed in the `{:e}` form of Rust (round trip).
Refused also: `C < 2`, `N < 8`, a non-finite sample, a non-positive or non-finite `q0`, `w`, tolerance. The registered tolerance
of every flow run below is `1e-6`.

### D2. Prescribed motion of the wall in a mode (the driver)

`drive_mode(fluid, map, motion)` with a `cosim::Fluid`, an `InterfaceMap` (F-362) and `ModeMotion { shape, amplitude, omega,
steps_per_cycle, cycles }`, `shape` one vector per interface node of the map: `dt = 2 pi/(w N)`; for `n = 1 .. C N`: node
displacements `shape q(t_n)`, wall points by `map.displacement_to_points`, `fluid.advance(dt, points)` gives the face forces at
`t_n`, nodal forces by the transposed map `map.forces_to_nodes`, the generalised force `Q_n = sum_i shape_i . f_i`. It also sums
the discrete work of the face forces along the face displacements (`map.face_displacements`), trapezoidal in the step, for the
record. The fluid must start at the wall's reference position (`q(0) = 0`) and at rest; the driver does not checkpoint. Since
the transposed map conserves work (ADR 0263), `Q` is the power-conjugate of `q`. Refused: a shape of the wrong length or
non-finite, `N < 8`, `C < 2`, non-positive amplitude or frequency, and any error of the fluid or the map (passed on with the step
number).

### D3. The oscillating plane channel (derived; the roadmap's formula verified)

A channel of gap `h` (`-h/2 < y < h/2`), long in `x`, no-slip walls, fluid of density `rho` and kinematic viscosity `nu`, driven
by a piston across the whole gap at `x = 0` and open (`p = 0`) at `x = L_f`. Away from the ends the flow is parallel and fully
developed: `u(y, t)`, `-p_x = G(t)` uniform in `x` (continuity with `v = 0`), and

`u_t = G/rho + nu u_yy`, `u(+-h/2) = 0`.

With `G = Re(Ghat e^{i w t})`, `u = Re(uhat(y) e^{i w t})`: `i w uhat = Ghat/rho + nu uhat''`, so `uhat = (Ghat/(i w rho))(1 -
cosh(k y)/cosh(k h/2))`, `k = sqrt(i w/nu)` (principal root). The mean over the gap is `ubar = (Ghat/(i w rho))(1 -
tanh(kappa)/kappa)`, `kappa = k h/2 = (h/2) sqrt(i w/nu)`. The flux through every section is the piston's, so `ubar` is the piston
velocity `V`. The pressure at the piston is `G L_f` above the open end; the force resisting the piston is `F = G L_f A` (`A` the
piston area), and

`F/V = A i w rho L_f/(1 - tanh(kappa)/kappa)`:

**the roadmap's formula is correct**, with `kappa` as defined there and `A` the piston area (per unit width `A = h`). `|kappa| =
(h/2) sqrt(w/nu)` is the Womersley number `Wo` of the plane channel (half-gap). Low frequency: `tanh(kappa)/kappa = 1 - kappa^2/3 +
2 kappa^4/15 - ...`, so `1 - tanh(kappa)/kappa = (kappa^2/3)(1 - 2 kappa^2/5 + ...)`, `kappa^2 = i w h^2/(4 nu)`, and

`F/V = A (12 mu L_f/h^2)(1 + 2 kappa^2/5 + ...) = A (12 mu L_f/h^2 + i (6/5) w rho L_f + O(w^2))`, `mu = rho nu`:

the plane Poiseuille resistance (the roadmap's low-frequency limit `12 mu L_f/h^2`, which is the force per velocity **per unit
piston area**; the check compares `Re(F/V)/A` with it) and the added mass `(6/5) rho L_f A` of the parabolic profile. Numbers
(arithmetic): `Re(Z)/(12 mu L_f A/h^2)` = 1.000007 at `Wo = 0.25`, 1.001901 at `Wo = 1`, 1.552957 at `Wo = 5`; `Im/Re` = 0.0250,
0.3992, 6.086.

The ends: the piston imposes a uniform velocity across the gap, so near it the profile goes from the plug to the developed one
over about `h/4` (the first Papkovich-Fadle decay rate of the Stokes channel, about `4.2/h` for the full gap, from memory; not
used beyond the order), and the corners where the piston meets the no-slip walls are scraper corners (Taylor's scraper, from
memory) with a pressure of order `mu V/r`, whose integral over the piston face grows like `mu V ln(h/Delta)` on a mesh of size
`Delta`. Both add a force of order `(1 to 10) mu V` per unit depth against the developed `12 mu V L_f/h`: a relative part of order
`h/L_f`. The registration takes `L_f = 100 h` (estimate below 1.5 %; an order-of-magnitude estimate, not derived further). The
open end with `p = 0` and a zero-gradient velocity is consistent with the developed flow and is not corrected.

### D4. The time step and its extrapolation (derived)

The flow step (moving-mesh PISO, implicit Euler, the wall moved from `q_{n-1}` to `q_n` within the step) imposes on step `n` the
wall velocity `(q_n - q_{n-1})/dt` and returns the force at `t_n`. For the parallel flow of D3, with `uhat z^n`, `z = e^{i w dt}`,
the implicit Euler step reads `(1 - z^{-1})/dt uhat = Ghat/rho + nu uhat''`: the continuous problem with `i w` replaced by `s =
(1 - z^{-1})/dt`; the flux of step `n` is `(q_n - q_{n-1})/dt = s qhat z^n`. So the force sampled at `t_n` against the motion
`qhat z^n` gives the discrete impedance

`Z_dt = A rho L_f s^2/(i w (1 - tanh(kappa_s)/kappa_s))`, `kappa_s = (h/2) sqrt(s/nu)`,

which is analytic in `dt` with `s = i w + w^2 dt/2 + O(dt^2)`: the error is first order. For the plug (`nu -> 0`, the open column
of F-401) it gives `Z_dt = i w rho L_f A g e^{-i w dt}`, `g = (2 sin(w dt/2)/(w dt))^2`: the force is the second difference centred
one step back (ADR 0434 D2), a lag `dt` that reads as a damping `c = w rho L_f A g sin(w dt)` and an added mass `rho L_f A g
cos(w dt)`.

**The registered extrapolation**: runs at `N = 200` and `N = 400` steps per period, Richardson of order 1, `Z_ext = 2 Z_400 -
Z_200` (real and imaginary parts alike). A third run at `N = 100` gives the observed order `p = log2(|Z_100 - Z_200|/|Z_200 -
Z_400|)` (companion). Arithmetic on the model (continuous profile): the errors of `Z_400` are `+9.3e-2` (real) and `-1.9e-3`
(imaginary) at `Wo = 5`, `+6.2e-3`, `-2.0e-2` at `Wo = 1` (the lag rotates the larger part into the smaller); after the
extrapolation `+1.8e-4`, `+2.7e-4` (`Wo = 5`), `+8.6e-5`, `+2.9e-4` (`Wo = 1`), `+8.2e-5` (real, `Wo = 0.25`); observed order
1.0008, 1.0079, 1.0008.

The space step across the gap, cell-centred finite volumes with the half cell at the wall (the model with 40 cells, continuous
in time): `+4.8e-3`, `-1.6e-3` (`Wo = 5`), `-1.2e-3`, `-9.4e-4` (`Wo = 1`), `-1.2e-3` (real, `Wo = 0.25`); the full model (both)
after the extrapolation `+5.0e-3`, `-1.4e-3` (`Wo = 5`), `-1.1e-3`, `-6.5e-4` (`Wo = 1`), `-1.2e-3` (`Wo = 0.25`).

**The viscous splitting of the PISO step** (ADR 0434's VS miss: the non-iterated step moves the plug by a part proportional to
`nu`). For the channel, viscosity is the physics, and the splitting is an error of the step that the model above does not
contain. It is a time-step error of a consistent scheme: if it is first order it is removed by the extrapolation with the lag,
and the observed order (companion O) shows whether the error of the runs is first order. Its size is not derived; it is the
largest unknown of this registration.

**Start and transient**: the run starts from rest with the wall at its reference (`q(0) = 0`, `q'(0) = q0 w`): an impulsive start.
The transient of the parallel flow is made of the profiles of zero flux (the piston fixes the flux), the even ones decaying at `nu
(2 mu_j/h)^2`, `tan(mu_j) = mu_j`, `mu_1 = 4.4934`: per period a factor `exp(-2 pi mu_1^2/Wo^2)`: `6.3e-3` at `Wo = 5`,
`1e-55` at `Wo = 1`. So 6 cycles at `Wo = 5` (change between cycles 5 and 6 of order `1e-12` of the transient's start) and 3 at
`Wo <= 1`; the transient near the ends decays faster (shorter scales). Tolerance `1e-6` (D1).

### D5. The open column (F-401's case)

The channel of F-401 (ADR 0434 D4): `box_mesh([24, 4, 4], [32, 1, 1], [Expansion(15), Uniform, Uniform])` shifted by `L_s = 4 m`,
`Symmetry` on the four side walls, `p = 0` at `xmax`, `PimpleSettings::piso(1e-4)` with central convection and the solver
tolerances of F-401, `FlowWall` with the prescribed wall gradient, `ChannelMotion(mesh, "xmin", 0, L_f, 1e-9)`; `rho_f = 3925
kg/m^3` (F-401's ratio 4), `A = 1 m^2`, `L_f = 32 m`. The wall is driven through the matching map of the bar's interface (the
`xmax` face of F-401's Hex8 bar 20 x 4 x 4, `MapSettings::projection(1e-9)`; rows the identity, ADR 0434 M0) in the piston mode
`shape = e_x` at every interface node, `q0 = U = 1e-3 m`, at F-401's ratio-4 frequency `w` (the root of D1 of ADR 0434), `N =
100` (F-401's `dt = T/100`), `C = 3`.

The plug has no shear (ADR 0434 D3), so the physical damping is zero and the column's in-phase part is `rho_f A L_f w^2 q0`.
The discrete flow is the plug model of D4: added mass `rho_f A L_f g cos(w dt)` (`-2.30e-3` relative at `N = 100`) and damping
ratio, with the column's own added mass as the modal mass,

`zeta_col = c/(2 m_a w) = -Q_q/(2 Q_p) = tan(w dt)/2 = 3.1457e-2`

(the **registered time-step estimate**). The **registered viscous estimate** is a bound: `zeta_visc = 1e-4`. Reason: ADR 0434
measured the viscous splitting on this column as a tip motion of `8.3e-5 U` over 25 steps for `nu` raised to `1e-2`, proportional
to `nu`; at `nu = 1e-4` that is `8e-7 U` over a quarter period, a damping ratio of order `1e-6`; the bound is 100 times that.
Check c4: `zeta_col <= tan(w dt)/2 + zeta_visc`.

### D6. The plane channel (c5, c6)

Gap `h = 0.02 m`, depth `h` (one cell, `Symmetry` on `zmin`, `zmax`), `A = h^2 = 4e-4 m^2`, `L_f = 100 h = 2 m`; `box_mesh([120,
40, 1], [L_f, h, h], [Expansion(21), Uniform, Uniform])` (first cell about `h/8` at the piston, 40 cells across the gap); no-slip
walls `ymin`, `ymax` (`FixedValue` zero velocity); `p = 0` at `xmax`, velocity zero gradient there (the default); the piston is
`xmin`, moved by `ChannelMotion(mesh, "xmin", 0, L_f, 1e-9)` (points of the side walls slide along them: a wall moving in its own
plane, which sweeps no volume). `rho = 1000 kg/m^3`, `w = 2 pi rad/s`, `nu = w (h/2)^2/Wo^2` (`Wo = 5`: `2.51327e-5 m^2/s`; `Wo =
1`: `6.28319e-4`; `Wo = 0.25`: `1.00531e-2`), the force's `nu` the same. `PimpleSettings::piso(nu)`, central convection, solver
tolerances of F-401. Amplitude `q0 = 1e-3 h = 2e-5 m` (the convective and geometric effects are of order `q0/h` in the second
harmonic and `(q0/h)^2` in the first). The wall is driven through `InterfaceMap::new` with identity rows on the piston's wall
points (the structure here is the piston itself: one interface node per wall point, `shape = e_x`), so `Q` is the total axial
force on the piston. Runs at `N = 100, 200, 400`, `C = 6` (`Wo = 5`), `C = 3` (`Wo = 1`, `0.25`).

`Z = c + i w m_a` from the last cycle (D1), extrapolated (D4); compared with `A i w rho L_f/(1 - tanh(kappa)/kappa)` (complex
`tanh` computed by the test from `exp`), and at `Wo = 0.25` the real part with `12 mu L_f A/h^2` (c6: the low-frequency limit is
checked at `Wo = 0.25`, where the formula is the limit to `7e-6`).

Estimate of the error after the extrapolation: model of D4 (`<= 5.0e-3` real, `<= 1.4e-3` imaginary) plus the ends of D3 (`<=
1.5e-2`, order of magnitude) plus the PISO splitting (not derived): `<= 2e-2`.

### D7. The bar on the open column (c7)

Method: the column of D5 at F-401's ratio 4 (the run of D5 itself) and the same column with `rho_f = 490.625 kg/m^3` at the
ratio-0.5 frequency (`N = 100`, `C = 3`). The mode of the bar on the column (ADR 0434 D1): `phi(x) = sin(k x)/sin(k L_s)` (unit
at the tip), `k L_s` the root; its structural modal mass `m_s = phi^T M phi` with the consistent mass matrix of F-383
(`rcfd_fem::mass::consistent` of F-401's bar model, `phi` on the `x` unknowns, zero on the others); the fluid's part is the
measured added mass `m_a` (D1; the interface moves by `phi = 1` uniformly, so the generalised force is the total axial force).
`zeta_method = c/(2 w (m_s + m_a))`, `w` the root frequency at which the column is driven.

Decay: F-401's coupled run made again here (ADR 0434 D4 set-up copied: matching wall, Aitken, ratios 0.5 and 4, 1001 steps at
`T/100`), `zeta_decay = -ln(E_1001/E_1)/(2 Omega_m (t_1001 - t_1))` (the solid's energy, as ADR 0434 EN; `Omega_m` the measured
crossing frequency, ADR 0434 D7). Both are recorded (c7). Estimate (arithmetic with the continuous mode and F-401's energy
ratios): `zeta_method/zeta_decay - 1` = `-1.2e-2` (0.5) and `+3.5e-3` (4); the release excites higher coupled modes (3.6 % of the
tip at ratio 0.5), which are damped more by the lag and make the energy decay slightly faster. Companion BZ: `<= 5e-2`.

## Registered checks

Test file `crates/rcfd-post/tests/sa_aerodamp.rs` (the stub's name). Every value is printed; a test fails at the end if any of
its checks missed. Heavy runs (the channel runs W5, W1, LF) are `#[ignore]` records run once with `--ignored`; the fast tests
cover the same code path (QF).

### A. The acceptance

| Id | Clause | Quantity | Limit | Estimate |
|---|---|---|---|---|
| A0 | c0 | this file committed before any code | holds | holds |
| H1 | c1 | synthetic: `Q_n = Q_m + Q_p sin + Q_q cos + 0.3 sin(2 w t) - 0.2 cos(3 w t)` with `Q_m = 0.7`, `Q_p = 1.3`, `Q_q = -0.45` at `t_n = n dt`, `N = 64`, `C = 3`, `q0 = 2.5e-3`, `w = 7`: `max(|Q_p,fit - Q_p|, |Q_q,fit - Q_q|)/sqrt(Q_p^2 + Q_q^2)`, every cycle, and through the periodic response | `<= 1e-10` | `1e-15` |
| H2 | c2 | the same history: `|W_trap - pi q0 Q_q,fit|/(pi q0 sqrt(Q_p^2 + Q_q^2))`, every cycle; and the response's `work_per_cycle` against `pi q0 Q_q` (the known part) | `<= 1e-10` | `1e-15` |
| C1 | c3 | open column (D5): `|m_a/(rho_f A L_f) - 1|` | `<= 2e-2` | `2.30e-3` |
| C2 | c4 | open column: `zeta_col - (tan(w dt)/2 + zeta_visc)`, `zeta_visc = 1e-4` | `<= 0` | `-1e-4` |
| W5 | c5 | `Wo = 5`: `|Re Z_ext/Re Z - 1|` and `|Im Z_ext/Im Z - 1|` | `<= 3e-2` each | `<= 2e-2` (model `5.0e-3`, `1.4e-3`) |
| W1 | c5 | `Wo = 1`: the same | `<= 3e-2` each | `<= 2e-2` (model `1.1e-3`, `6.5e-4`) |
| LF | c6 | `Wo = 0.25`: `|Re Z_ext/(12 mu L_f A/h^2) - 1|` | `<= 3e-2` | `<= 2e-2` (model `1.2e-3`) |
| B1 | c7 | ratios 0.5 and 4: `zeta_method` (D7) | record | `1.76e-2`, `2.90e-2` |
| B2 | c7 | ratios 0.5 and 4: `zeta_decay` (D7) | record | `1.78e-2`, `2.89e-2` |
| P1 | c8 | synthetic: H1's history plus `0.1 e^{-t/T} cos(w t)` (a decaying transient), tolerance `1e-6`: refused; the message names the change; the number in the message parsed back equals the change computed by the test from its own discrete Fourier sums of cycles 2 and 3 within `1e-12` relative; with tolerance 1 the same history is accepted with that change | all hold | holds |

### C. Companions

| Id | Quantity | Limit | Estimate |
|---|---|---|---|
| C3 | column: `|zeta_col - tan(w dt)/2|` and `|m_a/(rho_f A L_f g cos(w dt)) - 1|` (the discrete plug of D4) | `<= 1e-4` each | `1e-6`, `1e-5` |
| C4 | column: the response's cycle-to-cycle change; the driver's face work over the last cycle against `pi q0 Q_q` (record) | `<= 1e-6`; record | `1e-10`; agrees to `(w dt)^2` |
| O | W5, W1, LF: the observed order `p` of the real and imaginary parts from `N = 100, 200, 400` (LF: real part only) | `0.8 <= p <= 1.2` | `1.00` |
| PR | every channel run: cycle-to-cycle change of the last two cycles | `<= 1e-6` (else the run is refused, a miss of W5/W1/LF) | `<= 1e-10` |
| BZ | `|zeta_method/zeta_decay - 1|`, ratios 0.5 and 4 | `<= 5e-2` | `1.2e-2`, `3.5e-3` |
| BC | the coupled runs of D7: every step converged (cap 10); `abs(Omega_m/omega - 1)` | all; `<= 2e-2` | 3 iterations; `1.6e-4`, `1.1e-3` (ADR 0434) |
| H0 | synthetic: H1's history on uniform samples that do not cover a period (`N = 64` samples over 0.7 period) with only the first harmonic and the mean: the least-squares fit recovers `Q_m`, `Q_p`, `Q_q` within `1e-10` relative | `<= 1e-10` | `1e-13` |
| QF | fast channel: `L_f = 20 h`, `box_mesh([30, 8, 1], ..., [Expansion(5.5), Uniform, Uniform])`, `Wo = 1`, `N = 50, 100`, `C = 3`, `Z_ext = 2 Z_100 - Z_50`: real and imaginary parts against the formula | `<= 0.2` each | `<= 0.11` (space 3 %, time `5e-3`, ends of `20 h` up to 7.5 %) |
| DR | refusals: driver (shape length, non-finite shape, `N = 4`, `C = 1`, amplitude 0, frequency `NaN`); harmonic fit (length mismatch, fewer than 3 samples, non-finite sample); periodic response (`C = 1`, tolerance 0); each a message naming the cause | all hold | holds |

### N. Negative controls

* NC1: `Wo = 5` without the extrapolation: `|Re Z_200/Re Z - 1| >= 5e-2` (the lag of D4 is visible, so the extrapolation is what
  W5 checks). Estimate `+0.19`.
* NC2: `Wo = 5`, `N = 200`: the response of the first two cycles only is refused at the tolerance `1e-6` (a real start-up
  transient is refused, c8 on a flow run). Estimate: change of order `1e-4` (the impulsive start's non-plug part, about 20 % of
  the flux response at `Wo = 5`, decays by `6e-3` per period).
* NC3: P1 is refused at `1e-6` and accepted at tolerance 1 (in P1).

### S. Existing tests (unedited, release)

No line of `rcfd-fv`, `rcfd-fem`, `rcfd-api` or any shared solver file changes; the library change is the new content of
`rcfd-post/src/solid_aerodamp.rs` (declared by ADR 0440, called by nothing in the library; its `PENDING` becomes `None`). S1:
`cargo test -p rcfd-post --release --test si_fsi_piston` (the module whose helpers this item uses). No test reads
`solid_aerodamp::PENDING` (searched). The baseline hashes are not run (no shared solver file changes; stated in the addendum).

### M. Planned mutations (each alone, on a `git archive` copy of the test commit in the scratch folder, after the registered run)

* MU1: the fit swaps the sine and cosine columns: expected caught by H1, C1.
* MU2: the fit drops the mean column: expected caught by H1 (the mean leaks only off a full period: H0).
* MU3: `work_per_cycle` returns `pi q0 Q_p`: expected caught by H2.
* MU4: the cycle-to-cycle change compares cycles 1 and `C` instead of `C - 1` and `C`: expected caught by P1 (the number).
* MU5: the refusal test inverted (`delta < tolerance` refused): expected caught by P1, C1 (the column refused).
* MU6: the driver moves the wall one step late (`q(t_{n-1})` in step `n`): expected caught by C2, C3.
* MU7: the damping ratio without the factor 2: expected caught by C2, C3, BZ.

## Decision rule

* **done**: every check of A met (B1, B2 recorded), every companion of C met, the controls hold, S1 passes, every planned mutation
  caught.
* **partial**: any check of A missed (recorded as `*_known_miss`, limit intact) or not run; a check met only after an amendment
  made after its run; a companion missed with a diagnosed cause; a control that does not hold; a mutation not caught.
* **todo**: no prescribed-motion run of the flow possible.

## Limits (what is not covered)

* Closed forms only: a plug column and a plane channel, one-dimensional modes of the wall (a piston). No bending wall, no
  transverse wall motion (`ChannelMotion` is axial), no rotating passage, no fan blade (ADR 0310: not planned).
* Linear response: one amplitude, no amplitude study; the force's higher harmonics are fitted out, not reported.
* The periodicity test is on the first harmonic of the last two cycles; a slow drift of the mean is not refused.
* The ends of the channel (D3) are estimated, not derived; `L_f = 100 h`, one mesh, no study in the cells or the length.
* No case key, CLI, MCP tool or service call.

## Open points taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. The force is sampled at the end of each step (the time the coupling sees it), so the lag of the pairing is part of the
   measured damping and is removed by the extrapolation in `dt` (D4), rather than shifted by half or one step in the extraction.
2. The damping ratio of the column uses its own added mass as the modal mass (`-Q_q/(2 Q_p)`); the bar's uses the structural
   modal mass of the coupled mode plus the measured added mass (D7).
3. The low-frequency limit is checked by a harmonic run at `Wo = 0.25` with the same machinery rather than a steady run.
4. The channel length `100 h` from the estimate of the end effects (D3).

## Amendment 1 (2026-11-14, written after the registered run of the fast tests of commit 8100655a and before the corrected run)

**What the registered run showed** (log `f421-scratch/run1_fast.log`, made once, `cargo test -p rcfd-post --release --test
sa_aerodamp -- --test-threads=1`, 17.8 s): every fast check met except one part of P1: "the number in the message parsed back"
reported "no number after ' is '". The heavy records were not run yet; this amendment does not touch them.

**P1's parse is defective by construction.** The library's message is "... the force history is not periodic: the
cycle-to-cycle change of the first harmonic between cycles 2 and 3 is 1.063580170800776e-2 (tolerance 1e-6)"; the test took
the text after the **first** " is ", which is "not periodic: ...", so it read the word "not" and never reached the number. The
refusal itself and its wording were met (P1 refused: met; accepted at tolerance 1 with the change equal to the test's own within
`1.9e-14`: met). Correction (the set-up of the check only, the limit `1e-12` relative unchanged): **P1c** takes the text after
the **last** " is ". The registered test `p1_non_periodic_history_refused_with_the_change` stays as an ignored record (attribute
only); `p1c_non_periodic_history_refused_with_the_change` repeats it with the corrected parse. P1 counts as met only after this
amendment.
