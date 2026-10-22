# ADR 0110: Wave 43C, why the FAN-01 rotor-passage steady run does not converge (diagnosis on the coarsest mesh, opt-in fixes)

Status: registered before any registered run and before any solver code of this wave. Limits and decision rules are fixed here and are
not changed after a run; a miss becomes an ignored `*_known_miss` test with the limit intact; a defective set-up gets an amendment written
before the corrected run. Defaults stay bit-identical; `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded. Clean-room: first
principles only; the one literature statement used is marked UNVERIFIED. Written at HEAD 739244a (scratch clone
`<scratch dir>`, target `<target dir>`). Parents: ADR 0075, 0077-0087, 0101-0104, `docs/milestone-fan.md`.
No agreement with measured fan data is a question of this ADR or is claimed.

## The defect (three symptoms)

1. The ADR 0086 FAN-01 runs (C-grid `camber_seam_h`, k-omega SST, SIMPLEC, Barth-Jespersen-limited `linear_upwind`, `rotating_frame`)
   never reach the residual tolerance 1e-5: bladed `res_u` stalls at about 1e-3, the empty passage at 3e-5 to 5e-5, while the functionals
   are constant to 1e-5 relative. ADR 0084 (laminar FAN-01-C, `upwind`, `face_flux`) stalls at 1e-3 to 7e-3. Wave 41B (ADR 0102 R3) falsified
   the missing SIMPLEC boundary compensation as the cause.
2. The finest member (38 k cells) hits the 60-minute guard (8000 iterations at 0.30-0.45 s), because only the 8000-iteration cap rule ends
   these runs.
3. On the empty passage (exact absolute swirl 0) the spurious outlet angular-momentum flux L grows with refinement (-0.0095, 0.0596,
   0.1060 N m on F3, F2, F1) and the swirl energy is not monotone (6.27, 9.23, 5.48 Pa).

## What I already knew when writing this (rules c, d; disclosed)

Test file `crates/rcfd-api/tests/fan01_stall_w43c.rs` (the ADR 0086 case and mesh text copied unchanged, plus a location record). The record
prints every 250 iterations: `res_u`, `res_p`, continuity, `max_du`, the number of outlet faces with inflow, the functionals, `|du|_2 =
||u^n - u^(n-1)||_2` over the cells, the period ratio `||u^n - u^(n-2)|| / ||u^n - u^(n-1)||` (0 for a period-two cycle, about 1 for a
three-iteration cycle, about 2 for a monotone drift), the share of `sum |du|^2` in the 10 largest cells and in the largest 1 %, the five
largest cells (radius, z, largest face non-orthogonality, patches touched), their Barth-Jespersen factors psi (x/y/z) at iterations n and
n - 1, their flux-sign flips between n - 1 and n, and the counts of cells whose psi changed by more than 1e-3 and of faces whose flux changed
sign. Smoke runs (release, one each):

* **B3** (bladed F3, 4 920 cells, 500 and 1000 iterations): reproduces ADR 0086 (median `res_u` / `res_p` over iterations 500-1000 1.09e-3 /
  4.87e-4; torque 1.8493 N m, Delta p_ts 46.64 Pa at 1000; 0.044 s per iteration). From iteration 100 on `max_du` = 2.2 m/s every iteration,
  `|du|_2` 5.2-7.0 m/s, period ratio 1.05-1.31 (about three iterations per cycle), 37-49 % of `sum |du|^2` in 10 cells and 75-78 % in the
  largest 1 %. The largest cells sit at the blade root leading edge: c665 (blade wall, r 0.1256, z 0.1003, 28 deg), c17 (hub and blade, r
  0.1243, z 0.0998, 26 deg), c341 (blade, r 0.1249, z 0.1000), then interior and seam cells near the hub at z 0.14-0.18 (36-51 deg). None is
  near the 83-degree faces. Their psi jump between consecutive iterations (c665 psi_x 1.000 -> 0.846; c341 0.282 -> 0.122; c545 0.145 ->
  0.822; c1025 psi_x 0.137 -> 1.000) with no flux-sign flip on their faces; c17 stays at psi_x = 0. 1 312-1 352 cells change psi by more
  than 1e-3 per iteration, 14-15 faces (none of the top cells') flip flux sign. 48 outlet faces carry inflow, constant from iteration 100 to
  1000.
* **E3** (empty F3, 5 040 cells): reproduces ADR 0086 (L -0.009518 N m, swirl energy 6.26632 Pa from iteration 250 on, constant in every
  printed digit; median `res_u` / `res_p` 3.03e-5 / 6.88e-6). From iteration 250 on the state is an exact period-two cycle: period ratio
  0.008-0.019, `max_du` 0.0651 m/s and `|du|_2` 0.107 m/s identical at 500, 750, 1000; 81 % of `sum |du|^2` in 10 cells, 94 % in the largest
  1 %; largest cells c3765 / c3766 (interior, second layer below the shroud, r 0.2482, z 0.238 / 0.303, 48 / 42 deg). Their psi flip every
  iteration (c3765 psi_x 0.977 <-> 0.260, psi_y 0.551 <-> 1.000; c3766 psi_x 0.980 <-> 0.532), no flux-sign flip on their faces. 195-234
  cells change psi per iteration, 4 faces flip flux sign. No outlet inflow.
* **Set-up smokes** (`smoke_setups`, 20 + 20 or 40 iterations, every arm below): all finite without a guard trip except **unlimited
  `linear_upwind`**, which trips the steady guard (continuity growth) at iteration 27 on B3 (switched at 20) and at 7, 9, 9 on E3, E2, E1
  from the start. So "remove the limiter" is not a runnable discriminator on these meshes (recorded, not registered).
* Code read: `turbulence = "wall"` is set only on the casing, but `crates/rcfd-api/src/turb.rs` `role` makes every fixed zero-velocity patch
  (hub, blade) a wall, so the turbulence treatment of the argmax cells is the wall treatment (not a candidate). The limiter of
  `linear_upwind` is `crates/rcfd-fv/src/gradient.rs` `limiter`, applied per Cartesian component of the relative velocity in the convection
  term only (ADR 0081). The library also has `Limiter::Venkatakrishnan(k)` with `eps^2 = (k V^(1/3))^3`; with k = 5 and V^(1/3) about 0.011 m
  this is 1.7e-4 (m/s)^2 against psi-relevant velocity differences of 0.1-10 m/s, and its per-cell min over faces, the neighbour max/min and
  the final `.min(1)` are still kinks: it is NOT a clean test of the smoothness hypothesis and is reported only.

## Part S: the stall, ranked mechanisms (rank from the smoke evidence above)

Common protocol of a **switch arm** (B3 unless stated): 1000 reference iterations of the ADR 0086 set-up (reference window: iterations
500-1000), then one change applied to the solver through `Simulation::simple_solver_mut`, then 2000 more iterations (arm window: the last
500). With `rho_med` = the larger of the ratios (arm median / reference median) of `res_u` and `res_p`, and `rho_du` = `|du|_2` at the end /
`|du|_2` at the switch:

* **STANDS** iff `rho_med <= 0.01` (a hundredfold drop) and `rho_du <= 0.01`;
* **FALSIFIED** iff `rho_med >= 1/3`;
* **CONTRIBUTES** otherwise (also when `rho_med <= 0.01` but `rho_du > 0.01`: a residual drop without stopping the cycle).

A **set-up arm** (a change that cannot be switched on a running case) runs the reference for 1000 iterations and the variant for 3000 from
the start, with the same windows and the same rule (`rho_du` from the last record of each). These are protocol thresholds (decision rules
of a diagnosis, not accuracy limits): a factor 100 is far outside the 1.05-1.3 spread of the stalled medians between windows in the smoke,
and 1/3 is the largest drop still read as "no effect".

| Rank | Mechanism | Experiment (id, test) | Prediction if the mechanism is the cause |
|---|---|---|---|
| 1 | **Barth-Jespersen limit cycle**: psi is a non-differentiable function of the iterate; at the stalled state the map switches between limiter branches every iteration (period two on E3, about three on B3), so no fixed point is approached | **S1** freeze psi: from the switch on the kernel uses the psi of the switch iteration for every component instead of recomputing it (diagnostic hook, below). E3: switch at 250, 250 more iterations (`s1_e3_frozen_limiter_converges`, non-ignored, about 25 s); B3: switch at 1000, 2000 more (`s1_b3_frozen_limiter`, ignored) | E3 and B3 STAND; on E3 also `res_u` and `res_p` below 1e-5 within the 250 iterations |
| 2 | **Upwind-direction switching** at faces with flux near zero (the other kink of the convection operator) | **S2** record in the S1 arms: the faces of the five largest-`du` cells at the end of the frozen arm | stands iff S1 is CONTRIBUTES or FALSIFIED on a mesh AND at least one of the five largest cells of that frozen arm has a flux-sign flip in the final record; falsified otherwise (the smoke saw no flip on any top cell, disclosed) |
| 3 | **Local pressure-velocity mode** on the strongly non-orthogonal C-grid cells (the ADR 0084 period-two pressure mode, locally) | **S3** `velocity_correction = "face_flux"` (`reconstructed_correction`, fixed point unchanged), switch arm `s3_face_flux` | STANDS |
| 4 | **SST coupling** (k and omega lagged, wall treatment at the leading-edge stagnation corner) | **S4** laminar B3 (no turbulence model; walls and scheme unchanged), set-up arm `s4_laminar` | STANDS |
| 5 | **Periodic-seam gradient**: the least-squares gradient on seam faces uses the interpolated face value, not exact for linear fields where the C-grid cells are not aligned across the seam (ADR 0077) | **S5** `periodic_gradient = "partner_cell"` (`periodic_gradient_partner`), switch arm `s5_partner` | STANDS |
| 6 | **Linear-solver tolerance** (momentum relative 1e-3 / 200 iterations, pressure 1e-3 / 2000) | **S6** momentum and pressure relative 1e-10 (absolute 1e-16 / 1e-18, caps 2000 / 20 000), switch arm `s6_tight` | STANDS |
| 7 | **Outlet backflow switching** (48 inflow faces on B3) | **S7** outlet as catalogue `pressure_outlet` at 0 Pa with `prevent_reverse_flow = true`, set-up arm `s7_outlet` | STANDS |
| 8 | **Slow convective decay of a swirl mode** (a global, slowly converging mode rather than a stall) | none run: the smoke already contradicts it (a localised exact or near-exact cycle, functionals constant to the printed digits) | reported as contradicted by the smoke, not as a registered verdict |

Reported beside S1, not a discriminator (reason above): **S1v** Venkatakrishnan(5) switch arm on B3.

The S1 diagnostic hook (written after this registration): `SimpleSolver::frozen_limiter: Option<[Vec<f64>; 3]>`, default `None` (the existing
path, bit-identical by construction). With `Some(psi)` the `linear_upwind` limiter of component k uses `psi[k]` instead of calling
`gradient::limiter`. The S1 test computes psi with the kernel's own call (`least_squares` gradient of the current iterate, the current
boundary values, `limiter(.., BarthJespersen)`), so at the switch iteration the frozen and the recomputed psi are identical. The hook is kept
whatever S1 gives (it is what the registered S1 tests need; rule (b) keeping registered here).

## Part L: why L grows with refinement on the empty passage (exact swirl 0)

Mechanism **L1** (rank 1): the rigid part `-Omega x r` of the relative velocity is linear in the Cartesian components, so unlimited
`linear_upwind` would convect it exactly; wherever psi < 1 the scheme moves toward first-order upwind on the rigid part, which is the
ADR 0082 torque `(Omega/2) |phi| |d_perp|^2` per face (sign-definite), weighted by the clipped share. Experiment **L1** (no new solver run
beyond the settled states): on the default E3, E2, E1 states after 600 iterations, the psi-weighted closed form
`C_psi = sum over internal faces of k (-zx(x_P - x_N, phi_f du_f))` with `du_f,j = (1 - psi_up,j) Omega (z x (x_f - x_up))_j` for j = x, y
(psi of the upwind cell, the mean of the two last iterates; `k = 9 rho`; for psi = 0 this is ADR 0082's `C_up`). Prediction: `C_psi` rises
E3 -> E2 -> E1 as the signed L does. **STANDS** iff `C_psi(E3) < C_psi(E2) < C_psi(E1)`; **FALSIFIED** iff `C_psi(E1) < C_psi(E2)`; otherwise
CONTRIBUTES. Reported: `(C_psi(E1) - C_psi(E3)) / (L(E1) - L(E3))` (a trend test, not an equality: `C_psi` is one-signed, L(E3) is negative).

Mechanisms tested by a family (E3, E2, E1, 600 iterations each from the start, the change applied before iteration 1; ADR 0086 ratios
r21 1.4073, r32 1.4017):
* **L2** periodic-seam gradient: `partner_cell` family (`l2_partner_family`).
* **L3** SST: laminar family (`l3_laminar_family`).
* **L4** a family that is not geometrically similar (the C-grid smoother): not testable by a switch; read by elimination only.

Family rule: **STANDS** iff `|L|` decreases monotonically E3 -> E2 -> E1 with an observed order (ADR 0086 fixed-point formula on |L|) of at
least 0.5; **FALSIFIED** iff `|L(E1)| >= |L(E2)|`; CONTRIBUTES otherwise; NOT EVALUABLE if any member's L changes by more than 1e-3 N m over its
last 100 iterations.

## Fixes (registered now; each only if its mechanism stands)

* **F-Z** (if S1 stands on both E3 and B3): opt-in case key `solver.linear_upwind_limiter_freeze` (integer N >= 0, default 0 = off): with
  N > 0 the psi computed in iteration N is used in every later iteration (the S1 hook driven by the case). Allowed only with
  `linear_upwind_limiter = "barth_jespersen"`, steady `simple` / `simplec` (located error otherwise). Registered N for every run below: 1000.
  Acceptance, in this order:
  * **Z1 empty passage first** (E3 and E2, cap 8000): `res_u` and `res_p` <= 1e-5 reached (the run converges by tolerance), and L and the
    swirl energy within the ADR 0086 cap-rule tolerances of the stalled ADR 0086 values (`max(0.5 % |L|, 1e-4 N m)`, `max(0.5 %, 0.01 Pa)`;
    E3 -0.009518 / 6.26632, E2 0.059609 / 9.23038). F-Z is a convergence fix: it cannot make the spurious swirl zero, and Z1 does not ask it to.
  * **Z2 bladed** (only after Z1 passes; B3 and B2, cap 8000): converges by tolerance; torque and Delta p_ts within 0.5 % of the ADR 0086
    values (B3 1.849759 / 46.7776, B2 1.967401 / 64.2329). The iteration counts to tolerance on F3 and F2 give the predicted F1 count
    (reported as a prediction with the F1 cost 0.30 s per iteration; F1 is not run, symptom 2).
  * **Z3 defaults**: key absent and `0` bit-identical (u, p, phi `to_bits` on B3 after 30 iterations, non-ignored test); `baseline_hashes.rs`
    passes unchanged; a located error for an invalid combination.
* **F-A** (if L1 stands): opt-in `solver.linear_upwind_limited_velocity` = `"relative"` (default) or `"absolute"`: with a `rotating_frame`
  source, the limiter acts on the absolute velocity `w + Omega x r` (cell values, boundary values and gradient minus the exact rigid part)
  and the rigid gradient is added back unlimited, so the rigid part is convected exactly. Located error without a `rotating_frame` source
  or with another scheme. Acceptance: **A1 empty-passage family first** (E3, E2, E1 from the start, 600 iterations each, the family rule
  above): STANDS by the family rule, and the swirl energy also decreases monotonically. **A2** (only after A1): B3 with F-A (and with F-Z
  if F-Z was accepted) reported: torque, Delta p_ts, convergence; no limit, no agreement claim. **A3 defaults** as Z3.
* A fix that misses its acceptance is removed (rule (b)); its tests stay as ignored `*_known_miss` records with the limits unchanged.

## Run order and rules

S1 (E3, then B3), S1v, S2 (read from S1), S3, S4, S5, S6, S7; L1, L2, L3; then F-Z (Z1 before Z2) and F-A (A1 before A2). Each registered
run once, at most two at a time. No run over 60 minutes. Tests in `crates/rcfd-api/tests/fan01_stall_w43c.rs` (ignored when over 60 s).

## Not claimed

Agreement with the FAN-01 measurement; any accuracy of the converged values beyond the empty-passage check; anything about meshes finer than
F1.

### Unverified references

That non-differentiable limiters such as Barth-Jespersen are known to stall steady convergence and that smooth limiters (Venkatakrishnan,
J. Comput. Phys. 118, 1995) were introduced for this reason: recalled, UNVERIFIED. Nothing here depends on it.

## Amendment 1 (written after the registration commit and the S1 hook code, before any registered run)

What I knew when writing it: nothing beyond the registration; no registered run has been made. While writing the S1 tests I found that the
registration fixes the windows only for the 1000 + 2000 arm. For S1 on E3 (switch at 250, 250 more iterations) a "last 500 before the switch"
window would contain the start-up transient (res_u 1 at iteration 1, 1.3e-4 at 100). Corrected E3 set-up, fixed now: switch at iteration
**500** (the smoke shows E3 in its exact cycle from 250 on), 250 iterations after the switch, both windows of length `w = min(500, n1 / 2)`
= 125 (reference: iterations 376-500; arm: the last 125). The same `w` rule applies to every arm (it gives 500 for every 1000 + 2000 arm and
set-up arm, as registered). The E3 test stays non-ignored (750 iterations, about 35 s in release). Nothing else changes.
