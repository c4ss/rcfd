# ADR 0249: F-346 registration: immersed solid, force and moment in unsteady flow

Status: **REGISTERED 2026-11-12**, written before any code of F-346 and before any run of its checks. Not yet accepted by the
maintainer. Results go in an addendum; a defective set-up is corrected by an amendment written and committed before the corrected
run; nothing above an amendment or the addendum is changed after a run.

Parents: [ADR 0239](0239-f337-registration-immersed-force-and-moment-order.md) (F-337: `force_moment_second_order`, the
angular-momentum balance of the forced cells; its question Q3, from which F-346 was made),
[ADR 0234](0234-f309-registration-immersed-face-flux-at-the-surface-crossing.md) (F-309: `build_second_order`),
[ADR 0218](0218-f219-registration-immersed-solid-with-modified-forcing.md) (F-219: the forcing, `force_moment`, the DFG and CC
set-ups, the 2D-2 run). Roadmap item F-346 (depends on F-337).

Number: the lead gave 0249. Branch `f346-immersed-unsteady-force` from main 8c6bf3c4, worktree `E:/projects/cfd/w49i-work/f346`.

House rules (those of ADR 0218, 0234, 0239): limits are fixed here and not changed after a run; a missed limit becomes an ignored
`*_known_miss` test with the limit intact and the number recorded, and is diagnosed, not tuned away; a defective set-up is
corrected by an amendment before the corrected run, and the old test stays as an ignored record; each registered run is made
once; every new code path is opt-in, `force_moment` and `force_moment_second_order` return exactly what they returned before; no
case-schema key, hash table, golden or existing test file is edited; no new third-party crate; no unsafe code; no commit
trailers.

Clean-room: the evaluation is derived below from the momentum and angular-momentum balance of a fixed control volume that holds
a body (Batchelor, An Introduction to Fluid Dynamics, 1967, section 3.2) applied to rcfd's discrete momentum equation. The
statement that the hydrodynamic force on a body represented by a volume forcing is the reaction of the forcing plus the rate of
change of the momentum of the fictitious fluid inside the body is that of Uhlmann, J. Comput. Phys. 209 (2005) 448-476 (his
particle equations), and the same balance is in Lai and Peskin, J. Comput. Phys. 160 (2000) 705-719 and Balaras, Comput. Fluids
33 (2004) 375-404. These are cited from memory; no text of them was read for this work and no formula is copied: every formula
used is derived here. The two closed forms (the uniformly accelerated annulus and plate) are derived here from the Navier-Stokes
equations. No vendor code, text, manual or binary was used. The reference values of DFG 2D-2 are those recorded in
`docs/reference-data.md` (section "F-219 immersed solid with modified forcing", the row "DFG 2D-2 set-up": FeatFlow level 6,
`k = 1/200`, read 2026-11-07): max/min/mean `C_D` 3.2200/3.1569/3.1884, max/min `C_L` 0.9859/-1.0206, `St` 0.30188. Nothing is
supplied from memory.

## What F-346 asks

The roadmap entry: "Immersed solid: force and moment in unsteady flow"; acceptance "The force and moment of an immersed solid
subtract the rate of change of the momentum and angular momentum of its forced cells; DFG 2D-2 maximum drag and lift on M2 are
within 1.5 percent of the FeatFlow level-6 values, and a steady case returns the values of F-337 to round-off." Note: "From ADR
0239 question 3 ... ADR 0218 measured the 2D-2 extrema within 2.6 percent with the reaction of the forcing alone."

The acceptance is taken as binding. Two of its phrases need a reading; both readings are registered here, are readings and not
decisions of the maintainer, and are questions Q1 and Q2.

**Reading of "its forced cells" (Q1; from the derivation D1 below).** The cells of a body that carry forcing terms are its solid
cells (the penalisation) and its first fluid cells (the modification terms). The derivation shows that the momentum to account
for is that of the **solid cells** only (the fictitious fluid inside the staircase body), and that the momentum of the first
fluid cells is fluid momentum whose rate of change is already in the reaction of the forcing; adding or subtracting it again
counts it twice. The evaluation built is the derived one. The literal reading (solid cells and first fluid cells) is not built
in the library; it is computed in the tests as a registered negative control against the closed forms and printed for 2D-2.

"Subtract": in the convention of the forcing (the force of the body on the fluid, `-F_react`) the hydrodynamic force is
`-(forcing - dP/dt)`; in the convention of `force_moment` (the force of the fluid on the body) the same statement reads
`F = F_react + dP_S/dt`. The sign is fixed by D1 and checked by the closed forms.

**Reading of "on M2" (Q2).** ADR 0218 ran 2D-2 on its T mesh (324 x 164 cells, `D/40` around the cylinder), not on M2 (176 x
104 cells, `D/40` in the core `[0.12, 0.32] x [0.12, 0.28]`, graded outside it); the 2.6 percent of the note was measured on T.
The acceptance names M2. Both are run: the acceptance check A2 is on M2 as the text says, A2T is the same check on T, and
"done" needs both.

**Reading of "the values of F-337".** What `force_moment_second_order` returns for the same field.

## What was read and done before this text (disclosed)

Read: ADR 0239 in full; ADR 0234 (the design and the results); ADR 0218 (the design, the set-ups, the checks, the results);
`crates/rcfd-fv/src/immersed.rs` in full; `immersed_solid_f219.rs` (the DFG helpers and the 2D-2 test), the PISO test of
`immersed_second_order_f309.rs`, `immersed_force_order_f337.rs`; `crates/rcfd-fv/src/ddt.rs` (`add_ddt`: the coefficients of
the time schemes); of `crates/rcfd-fv/src/pimple.rs` the declaration of `PimpleSolver` (the velocity history `hist_u` is
private; `u`, `p`, `phi`, `immersed` are public) and the call of `reconstruct_face_flux`; `pressure_bc`; the roadmap entry;
`docs/reference-data.md` (the DFG rows).

Run: no code of F-346 exists and no check below was run. The existing test targets of the three immersed test files were built
in release on main 8c6bf3c4 while this text was written (a build, no test was run). One scratch Python program
(`f346-scratch/lattice.py`, not committed) counted the cells of the CC mesh whose centre lies inside the inner cylinder and
summed `h^2 r_c^2` over them and over their face neighbours; its numbers are in item 4 below and fix the limits of X1.

Known numbers. ADR 0218 addendum 1, 2D-2 on T with `build` and `force_moment`: max `C_D` 3.2462 (+0.81 percent of 3.2200), min
3.1805 (+0.75), mean 3.2135 (+0.79), max `C_L` 1.0113 (+2.58 percent of 0.9859), min -1.0469 (+2.58), `St` 0.30073; solid
velocity at `t = 8` at most 3.8e-5 `U_bar`. 2D-1 on M2 with `build`: `C_D` -0.17 percent, `C_L` +2.73 percent (a steady flow).
ADR 0239: 2D-1 on M2 with `build_second_order`: `C_D` 5.56246454, `C_L` 0.0108880492.

Arithmetic done beforehand (by hand, except the lattice sums), disclosed because it shaped the design and the limits:

1. **The reaction already holds the storage of the first fluid cells.** With the time term the momentum equation of a cell `c`
   is `rho V_c ddt(u_c) = sum_f g_{c,f} - dF_c` (`g_{c,f}` the force of the neighbour on `c` through `f`: pressure, viscous,
   convective; `dF_c = rho V_c (coeff u_c - source)` the terms of the cell; `ddt` the solver's time difference). Summed over a
   set `C` of cells, `sum_C dF_c = sum_{outer faces of C} g - ddt sum_C rho V u_c`. The reaction `F_react` (what `force_moment`
   sums) is therefore, at every instant, an algebraic function of the velocity field that equals a surface force minus a
   storage; no storage is missing from it in the sense of the discrete equations.
2. **Which storage is not fluid momentum.** See D1: the solid cells. For the stationary cylinder of 2D-2 their velocity is at
   most 3.8e-5 `U_bar`, so `rho sum_S V du_s/dt <= rho V_S omega 3.8e-5` with `omega = 2 pi f = 19 1/s`, `V_S = pi 0.05^2 dz`:
   about 1.1e-4 in units of the coefficients. **Prediction: the derived evaluation changes the 2D-2 coefficients by about 1e-4
   or less, the maximum drag stays at +0.81 percent (inside 1.5) and the maximum lift at +2.58 percent (outside 1.5) on T.**
   The acceptance's 1.5 percent on the lift is expected to be missed; the registration says so before the run.
3. **What the literal reading would do on 2D-2 (size only).** The first fluid cells are a ring of volume about `pi D h dz` with
   a velocity of about `theta h du/dn`, `du/dn` of order `U/delta`, `delta = D/sqrt(Re) = 0.01`: an oscillating y momentum of a
   few 1e-5 per unit depth and a rate of about `omega` times that, of the order of 1 percent of the lift amplitude 0.05 at
   `h = D/40`, of unknown phase. So the literal reading can move the 2D-2 lift maximum by about 1 percent in either direction;
   by D1 that change is a double count and not a correction, and it scales with `h^2`.
4. **Lattice sums for X1** (`lattice.py`; CC mesh `N x N` on `[0, 2.4]^2`, centre `(1.2, 1.2)`, `R1 = 0.5`; `I_B = pi R1^4/2`,
   the exact moment of X1 is `M_x = (pi/2) R1^2 (R2^2 - R1^2) = 0.294524` per unit `rho alpha`):

   | N | solid cells | first fluid cells | `I_S = sum_S h^2 r_c^2` over `M_x` | `(I_S + n h^4/6 - I_B)/M_x` | first fluid cells `sum h^2 r_c^2` over `M_x` |
   | --- | --- | --- | --- | --- | --- |
   | 48 | 316 | 60 | 0.3371 | +0.49 percent | 0.1402 |
   | 96 | 1264 | 116 | 0.3372 | +0.42 percent | 0.0647 |

   (`I_B/M_x = 1/3`.) The second-to-last column is the expected error of the derived evaluation in X1 from the staircase
   (the fluid between the staircase and the circle rotates almost rigidly and its inertia is counted with the body or with the
   fluid according to the side of the staircase it lies on).
5. **The storage of a first fluid cell against the fluid it stands for.** Along a grid line the fluid between the wall and the
   outer face of the first fluid cell has the thickness `(theta + 1/2) h`, the cell `h`. For a body that does not accelerate
   the difference is of second order (the velocity there is of order `h`). For an accelerating body it is the staircase volume
   error times the acceleration: zero for a plane wall on a grid line (`theta = 1/2`), the lattice-point error of item 4 for a
   circle.

## Design (fixed now)

**D1. The force and the moment (derived).** Let `S` be the solid cells of the body (the cells whose centre is in it) and `B`
the body. `S` is a fixed control volume bounded by the staircase faces (the faces between a solid cell and a fluid cell). The
balance of momentum of the fluid in a fixed control volume that holds a body: the force of the fluid on the body is the force
of the outside on the control volume through its surface (traction and momentum flux) minus the rate of change of the momentum
of the fluid inside the control volume and outside the body,

    F_body = T(dS) - d/dt P_fluid(S \ B).

Discretely, with the modification terms written as what they are, the replacement of the regular viscous flux of a staircase
face by the flux of the profile through the no-slip value at the crossing (ADR 0218 D2):

    F_react = sum_{s in S} dF_s + sum_{modification terms} dF_t
            = [sum over the staircase faces of the pressure force, the wall viscous force k_f (u_c - u_G)/theta_e and the
               momentum flux] - ddt sum_{s in S} rho V_s u_s
            = T_h(dS) - ddt P_S,

by item 1 with `C = S` (the regular viscous flux `k_f (u_c - u_s)` of the solid cell's equation and the modification add up to
the wall flux). `T_h(dS)` is the discrete traction on the staircase, the approximation of `T(dS)`; `P_S = sum_S rho V u_s` is
the momentum the solid cells hold. The solid cells hold the rigid velocity of the body (to `1/K`): `P_S` is the momentum of a
fictitious fluid that fills the body, not fluid momentum, while the true fluid inside `S` and outside `B` (the slivers between
the staircase and the surface) is a volume of the order of the staircase volume error. Hence

    F_body = F_react + ddt P_S                                  (the derived evaluation)
             - d/dt P_fluid(S \ B)                              (not evaluated: the slivers, item 5)
    M_body = M_react + ddt L_S,   L_S = sum_S (x_s - x_ref) x rho V_s u_s,

with `M_react` the moment of F-337 (every term at the centre of its cell; the angular-momentum form of item 1 is ADR 0239 D1
with the storage term `(x_c - x_ref) x rho V ddt(u_c)` of each cell, which for the solid cells is `ddt L_S`).

The first fluid cells do not enter: their modification terms are surface forces of the staircase faces (above), and their
momentum is that of real fluid. The same result from the control volume `R = S` plus the first fluid cells: `F_react =
T_h(dR) - ddt P_R` and `F_body = T(dR) - d/dt P_fluid(R \ B)`; the fluid of `R \ B` is the fluid of the first fluid cells, so
`ddt P_R - d/dt P_fluid(R \ B) = ddt P_S` up to item 5. Subtracting or adding the storage of the first fluid cells once more
(the literal reading) removes or doubles real fluid inertia.

For a body at rest `ddt P_S` is of the size of the solid velocity (item 2): **the reaction of the forcing is already the force
on a stationary body in unsteady flow**, and the premise of ADR 0239 Q3 (that the reaction of 2D-2 holds a time-dependent
momentum of the first fluid cells that should be removed) does not survive the derivation. The correction matters for a body
whose rigid velocity changes in time.

**D2. The library (opt-in; nothing existing changes).** In `crates/rcfd-fv/src/immersed.rs`:

- `ImmersedForcing::force_moment_unsteady(body, u, dudt, rho, x_ref) -> (force, moment)`: the pair of
  `force_moment_second_order(body, u, rho, x_ref)` plus, over the solid cells `s` of the body in their stored order,
  `dP = rho V_s dudt[s]` to the force and `(x_s - x_ref) x dP` to the moment. `dudt` is the rate of change of the cell velocity
  at the time level of `u`, one vector per cell.
- `immersed::velocity_rate(u, u_old, older, dt) -> Vec<Vec3>`: the solver's time difference of the velocity at the new level
  (`add_ddt` of `ddt.rs`): `(u - u_old)/dt` when `older` is `None` (implicit Euler, and the first step of BDF2), else with
  `older = Some((u_older, dt_old))`, `r = dt/dt_old`, `(c0 u - c1 u_old + c2 u_older)/dt`, `c0 = (1 + 2r)/(1 + r)`,
  `c1 = 1 + r`, `c2 = r^2/(1 + r)` (BDF2, variable step).

Time level and scheme: `u` is the velocity after the step to `t^{n+1}`; the forcing is implicit, so the reaction is that of
level `n+1`, and the rate is the backward difference at `n+1` that the momentum predictor holds. The caller keeps the one or two
older fields (the solver's history is private and `pimple.rs` is not touched: no hunk). At the first step the rate is the Euler
difference from the initial field, as in the solver; for a start from a field that does not satisfy the forcing the first rates
are of order `1/dt` (the impulse of the start) and are what the discrete equations hold. Crank-Nicolson and the bounded BDF2 are
not covered by `velocity_rate` (the caller may pass any `dudt`).

**D3. What stays.** `force_moment`, `force_moment_second_order`, `build`, `build_second_order` and both solvers are not edited.
With `dudt = 0` the new method adds `+0.0` to each component.

**D4. Choices (taken on the maintainer's standing instruction of 2026-11-11, take the recommendation; Q1, Q3, Q4).** The solid
cells only (Q1); a new method beside the two existing ones, not a change of either (Q3 is a change of a default and is not
taken); the rate passed by the caller, no solver hook (Q4).

## Set-ups

**2D-2 (the run of ADR 0218, unchanged).** The DFG channel, cylinder, boundary conditions, `K = 1e7`, `U_m = 1.5`, the initial
field with the blob, `PimpleSettings::piso(1e-3)` with BDF2, `LinearUpwind`, AMG pressure, two correctors, tolerances 1e-6,
`dt = 0.001`, 8000 steps, the window `[5.5, 8]`, the crossings of `C_L` minus its window mean, the extremes over the whole
periods between the first and the last crossing: all as in ADR 0218 and `a3_two_d_two_strouhal_number` (the helpers are
copied; that file is not edited). Forcing: `build` with `modified = true` (the forcing of ADR 0218, with which the 2.6 percent
was measured). Meshes: T (53 136 cells) and M2 = `dfg_family(2)` (18 304 cells). After every step three evaluations with
`rho = 1`, `x_ref` the cylinder centre: old (`force_moment`), new (`force_moment_unsteady` with `velocity_rate` of the last
three fields, Euler at step 1), and, computed in the test, the literal reading `new + rho sum_{first fluid cells} V dudt` and
its opposite sign (printed only). `C_D, C_L = F_x,y/(0.5 U_bar^2 D dz)`, `U_bar = 1`.

**X1, the uniformly accelerated annulus (closed form for the moment).** The CC set-up of ADR 0218 (`box_2d(N, N, 2.4, 2.4)`,
inner cylinder `R1 = 0.5`, outer inverted cylinder `R2 = 1`, `nu = 1`, `rho = 1`, `K = 1e8`, box walls velocity zero), but both
bodies rotate about the axis with `Omega(t) = alpha t`, `alpha = 1`, from rest. For `u_theta = alpha t r + w(r)` the azimuthal
momentum equation gives `alpha r = nu (w'' + w'/r - w/r^2)`, so `w = c (r^3 - (R1^2 + R2^2) r + R1^2 R2^2/r)`,
`c = alpha/(8 nu)` (zero at both walls), plus transients that decay like `exp(-nu (pi/(R2 - R1))^2 t)` (about `exp(-39 t)`).
The shear stress `mu r d(w/r)/dr` at `R1` is `2 mu c (R1^2 - R2^2)`; the moment of the fluid on the inner cylinder per unit
depth is

    M_x = -(pi/2) rho alpha R1^2 (R2^2 - R1^2) = -0.294524   (independent of nu and of t),

(check: with the outer wall's `-(pi/2) rho alpha R2^2 (R2^2 - R1^2)` the sum is minus the rate of the angular momentum of the
annulus, `(pi/2) rho alpha (R2^4 - R1^4)`). Run: `PimpleSettings::piso(1.0)`, BDF2, `Central`, `dt = 0.0025`, 200 steps to
`t = 0.5`; before step `n` the forcing is rebuilt (`build_second_order`) with `omega = alpha n dt` for both bodies and assigned
to `immersed`. `N = 48` and `96`. Evaluations of the inner body about the axis after every step, per unit depth: new, the
reaction alone (`force_moment_second_order`), and the literal reading. The solution is linear in time once the transients are
gone, so the time schemes add no error; the error is spatial (ADR 0239: 0.107 and 0.027 percent of the steady torque) plus the
staircase inertia of item 4.

**X2, the uniformly accelerated plate (closed form for the force).** `box_2d(8, N, 1, 1)`, `nu = 1`, `rho = 1`. One body: the
cuboid `y < 0.5` (corners `(-10, -10, -10)` and `(10, 0.5, 10)`), velocity `(a t, 0, 0)`, `a = 1`, `K = 1e8`; its surface is
the grid line `y = 0.5` (`theta = 1/2`). `ymax`: velocity zero (a wall at rest); `ymin` (under the solid): `Symmetry`; `xmin`
and `xmax`: velocity zero gradient, pressure fixed 0. With `d = 0.5` the gap and `eta` the distance from the plate,
`u = a t (1 - eta/d) + w(eta)`, `a (1 - eta/d) = nu w''`, `w = (a/nu)(eta^2/2 - eta^3/(6 d) - d eta/3)`; the shear stress of
the fluid on the plate is `mu du/d eta` at `eta = 0`:

    F_x/(L dz) = -(mu a t/d + rho a d/3) = -1.166667 at t = 0.5.

Run: `piso(1.0)`, implicit Euler, `Central`, `dt = 0.0025`, 200 steps; the forcing rebuilt before step `n` with the velocity
`a n dt` (`build_second_order`). `N = 16` and `32`. The body touches `xmin`, `xmax` and `ymin`; by ADR 0218 Q5 the reaction
then also holds the force of those boundaries on the solid cells. Here that force has no x component: the pressure is fixed 0
at the two ends, the velocity is zero gradient there, the momentum fluxes of the two ends cancel in a flow uniform in x, and a
symmetry plane carries no tangential force. This is checked (S-X2).

## Registered checks

All in the new file `crates/rcfd-fv/tests/immersed_unsteady_force_f346.rs`, release (the heavy ones carry
`#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]`). "Within `x` percent" is `abs(a - ref)/abs(ref) <= x/100`.
"New" is `force_moment_unsteady`, "reaction" is `force_moment_second_order` (its force is `force_moment`'s), "literal" is the
test-side sum of D1's literal reading.

### The acceptance

| Id | Check | Limit | Estimate behind it |
| --- | --- | --- | --- |
| A2 | 2D-2 on M2, new: max `C_D`; max `C_L` | within 1.5 percent of 3.2200; within 1.5 percent of 0.9859 | the roadmap's limit. No number exists on M2. From T (item 2) the drag is expected inside and **the lift is expected to miss** at about +2.6 percent |
| A2T | 2D-2 on T, new: the same | the same | item 2: max `C_D` 3.2462 (+0.81 percent, met), max `C_L` 1.0113 (+2.58 percent, **expected miss**) |
| A3a | steady, by iteration: 2D-1 on M2 with `build_second_order` (the run A2 of ADR 0239, M1 then M2, its convergence rule as set-up), new with `dudt = 0` against the reaction; `C_D`, `C_L` | every component of force and moment equal (`==`); 5.56246454 and 0.0108880492 to the digits given | D3; ADR 0239 |
| A3b | steady, by time stepping: CC at `N = 48`, `build_second_order`, PISO as V3 of ADR 0234 (Euler, central, `dt = 0.0025`, 400 steps; set-up: last step change `<= 1e-9`), new with `velocity_rate` of the last two fields against the reaction: z moment of the inner body, and the force | `abs(M_new - M_reaction) <= 1e-9 T`, `T = 4.18879`; force difference `<= 1e-9` per unit depth | the rate is at most `1e-9/0.0025`, times `I_S = 0.1` |

The first clause of the acceptance (the evaluation exists and is the balance) is checked by X1, X2 and U1 to U5.

### Closed forms

| Id | Check | Limit | Estimate behind it |
| --- | --- | --- | --- |
| X1 | accelerated annulus, new, at `t = 0.5`: `abs(M_new - M_x)/abs(M_x)` at `N = 48` and at `N = 96` | `<= 1.5 percent` each | item 4: 0.49 and 0.42 percent from the staircase, plus 0.1 and 0.03 percent spatial |
| X1c (negative control) | the reaction alone, same runs | error `>= 25 percent` each | item 4: `I_S/M_x` = 33.7 percent |
| X1l (negative control: the literal reading) | literal, same runs | error `>= 8 percent` at `N = 48`, `>= 4 percent` at `N = 96` | item 4: 14.0 and 6.5 percent less the staircase term |
| S-X1 (set-up) | the long-time state is reached: `M_new` at step 150 against step 200; no divergence | relative difference `<= 1e-3` | transients `exp(-39 t)`; the closed form is constant in time |
| X2 | accelerated plate, new, at `t = 0.5`: `abs(F_x/(L dz) + 1.166667)/1.166667` at `N = 16` and `32` | `<= 1 percent` each | a wall on a grid line, second order: about `(h/d)^2/10`, 0.15 percent at `N = 16` |
| X2c (negative control) | the reaction alone | error `>= 30 percent` each | the solid cells: `rho a 0.5/1.1667` = 42.9 percent |
| X2l (negative control: the literal reading) | literal | error `>= 2.5 percent` at `N = 16`, `>= 1.2 percent` at `N = 32` | one row of cells, `rho a h (1 - h/(2 d))/1.1667`: 5.0 and 2.6 percent |
| S-X2 (set-up) | the flow is uniform in x: `max abs(u_x(i, j) - u_x(0, j))` over the mesh at `t = 0.5`; `max abs(u_y)`; no divergence | `<= 1e-9` each | the set-up has no x dependence |

Printed, no limit: X1 with `build` at `N = 48`; the force on the inner cylinder of X1 (exact 0); the history of the three
evaluations every 25 steps; X2's y force.

### 2D-2 controls and set-up

| Id | Check | Limit | Estimate behind it |
| --- | --- | --- | --- |
| S2 (set-up, each of the runs T and M2) | the window of ADR 0218 S2 on the old `C_L` | at least 6 upward crossings; every period within 1 percent of the mean; the maximum of `C_L` of every period within 2 percent of their mean; `max C_L - min C_L >= 1.0`; no divergence | T: 7 crossings, 5.5e-6, 2.1e-4, 2.058 (ADR 0218). M2: not known; a miss there is a defective set-up (amendment) |
| C1 (control: the present evaluation misses) | T, old (`force_moment`): max `C_L` | error `> 1.5 percent` of 0.9859 | ADR 0218: 1.0113, +2.58 percent |
| C1r (the run is ADR 0218's) | T, old: max `C_D`, max `C_L`, `St` | 3.2462, 1.0113 and 0.30073 to the digits given | the same code and set-up, deterministic |
| D1p (the prediction of D1 for a body at rest) | T and M2: `max over the window of abs(C_new - C_old)` for `C_D` and for `C_L` | `<= 1e-3` | item 2: about 1e-4 |

Printed, no limit: for T and M2 the max, min and mean `C_D`, max and min `C_L` and `St` of the old, the new and the two literal
evaluations with their differences to the FeatFlow values; the moment coefficient `M_z/(q D)`; one more run, ignored and printed
only: 2D-2 on M2 with `build_second_order` (not run before with PISO at Re 100; if it stops, that is recorded).

### Unit checks and defaults

| Id | Check | Limit | Estimate behind it |
| --- | --- | --- | --- |
| U1 | new against the reaction plus the test-side sums `rho sum_S V dudt` and `rho sum_S (x_c - x_ref) x V dudt` (from the mesh volumes and centres and the public `solid_cells`), pseudo-random `u` and `dudt` (`lcg`, seeds `0x0346` and `0x0347`), `rho = 1.3`, `x_ref = (0.3, -0.2, 0.1)`: CC `N = 24` with `build` and `build_second_order` (both bodies), DFG M1 | `<= 1e-12` of the norm, force and moment | the same sums |
| U2 | `dudt = 0`: new against the reaction, the same five | every component equal (`==`) | D3 |
| U3 | transfer of the reference point of the new moment, `x2 = (-1.1, 0.7, 0.4)`: `M(x1) - M(x2) - (x2 - x1) x F` | `<= 1e-10` of the norm of `M(x1)` | round-off |
| U4 | a cuboid on grid lines, `[0.6, 1.8] x [0.9, 1.5]` on `box_2d(24, 24, 2.4, 2.4)` (12 x 6 solid cells), with the rigid rate `dudt(x) = a + alpha z x (x - o)`, `a = (0.7, -0.4, 0)`, `alpha = 1.1`, `o = (0.2, 0.3, 0)`, and `u = 0`: new minus reaction against the closed form `dF = rho V_B (a + alpha z x (x_cm - o))`, `dM_z(x_cm) = rho alpha V_B (Lx^2 (1 - 1/12^2) + Ly^2 (1 - 1/6^2))/12` (the moment about the centroid `x_cm`; cell-centre arms) | `<= 1e-12` relative | exact sums |
| U5 | `velocity_rate`: for `u(t) = u0 + u1 t + u2 t^2` per cell (pseudo-random `u0, u1, u2`) with `dt = 0.3`, `dt_old = 0.2`, BDF2 against `u1 + 2 u2 t`; with `older = None` against `(u - u_old)/dt` | `<= 1e-12` of the largest rate; equal (`==`) | BDF2 with a variable step is exact for a quadratic |
| N1 (defaults) | `immersed_force_order_f337.rs`, unedited, once on the final tree | every test passes (its N1: the five hashes of `force_moment`; its U1 to U4: `force_moment_second_order`); the printed numbers of ADR 0239 (CC new order 1.984; A2 5.56246454, 0.0108880492) | neither method is edited |
| N2 (defaults) | `immersed_solid_f219.rs` and `immersed_second_order_f309.rs`, unedited, once on the final tree | every test passes; the printed numbers of ADR 0218 and ADR 0234 as listed in ADR 0239 N2 | nothing on a default path changes |

### Negative controls and mutations (scratch, not committed)

Negative controls: X1c, X1l, X2c, X2l, C1 above. Mutations, each applied alone to the final tree by an exact text replacement
(checked to occur once) in `immersed.rs`, the named tests run once in release, the file restored and compared byte for byte
with a kept copy:

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | the storage term subtracted instead of added | X1, X2, U1 |
| MU2 | the storage term left out (new = reaction) | X1, X2, U1 |
| MU3 | the storage summed over the solid cells and the cells of the modification terms (the literal reading) | X1, X2, U1 |
| MU4 | the moment of the storage left out (force only) | X1, U1 |
| MU5 | the arm of the storage without `x_ref` subtracted | U3 |
| MU6 | `rho` left out of the storage | U1 |
| MU7 | `velocity_rate`: `c2` with the wrong sign | U5 |
| MU8 | `velocity_rate`: `r = dt_old/dt` | U5 |
| MU9 | `velocity_rate`: the Euler branch divided by `2 dt` | U5, X2 |

The 2D-2 runs are not repeated under the mutations (they are insensitive to the storage of a body at rest; D1p says by how
much).

## Decision rule

- **done**: A2 and A2T met (both coefficients) with S2 held; A3a and A3b met; X1 and X2 met with S-X1, S-X2 and the controls
  X1c, X1l, X2c, X2l, C1 held; C1r, D1p, U1 to U5, N1, N2 met; every mutation failed a test it must fail.
- **partial**: X1, X2 (with their set-up checks and controls), A3a, A3b, U1 to U5, N1 and N2 met and every mutation caught,
  but A2 or A2T missed in a coefficient (kept as an ignored known miss with its limit, diagnosed); or the acceptance met and
  another check missed.
- **todo** (unchanged; the library part is not proposed for a merge): X1 or X2 missed (the derived evaluation is then not shown
  to be the force), or N1 or N2 missed.

A set-up check or control that does not hold (S2, S-X1, S-X2, X1c, X1l, X2c, X2l, C1, C1r, the convergence rules of A3) is a
defective set-up: an amendment before any corrected run, never counted as a pass. If D1p is missed the prediction of D1 for a
body at rest is wrong and is diagnosed before anything else is concluded.

By the prediction of item 2 the expected outcome is **partial**: the lift of A2T, and probably of A2, outside 1.5 percent, for
a cause that is not the storage of the forced cells. That expectation is written here so that the result cannot be read as a
surprise that was explained afterwards.

## Limits (not covered)

The slivers between the staircase and the surface (item 5): the force of an accelerating body carries the staircase volume
error times the acceleration (first order for a plane wall off the grid lines, the lattice-point error for a curved body). A
body that moves through the mesh (the forcing is built for a fixed shape; ADR 0218 Q4): only the rigid velocity may change in
time, and the caller rebuilds the forcing. Crank-Nicolson and the bounded BDF2 in `velocity_rate`. No caller in the solvers,
`sixdof` or the API, no case key. The SIMPLE solver has no time (the new method with `dudt = 0` is F-337's). Non-orthogonal
meshes, the sphere, 3-D. A body that touches the domain boundary (ADR 0218 Q5) is as before. What causes the 2.6 percent of
2D-2 is not established by this item beyond the prediction that it is not the storage (see Q5).

## Open questions

- **Q1. Which cells.** Options: (a) the solid cells (derived, D1); (b) the solid cells and the first fluid cells (the literal
  wording). Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation): (a). X1l and X2l measure
  what (b) costs against a closed form.
- **Q2. M2 or T.** Options: (a) M2 as written; (b) T, the mesh of ADR 0218's 2D-2 run; (c) both. Taken on the standing
  instruction: (c).
- **Q3. Default.** Whether `force_moment` (or the force a future case key reports) becomes the unsteady evaluation is a change
  of a default: not taken, open.
- **Q4. Where the rate comes from.** Options: (a) the caller passes `dudt` and `velocity_rate` forms it from the fields the
  caller keeps; (b) a public accessor of the solver's velocity history in `pimple.rs`. Taken on the standing instruction: (a)
  (no hunk in `pimple.rs`, which other branches are editing); (b) can follow with the case key.
- **Q5. The 2.6 percent of 2D-2.** If A2/A2T miss as predicted, the candidates are the mesh (`D/40`; the steady lift of 2D-1 on
  M2 has the same +2.7 percent), the first-order forcing of ADR 0218, the zero-gradient outlet in place of do-nothing, and the
  time step. Options and a recommendation go in the addendum with the numbers.
