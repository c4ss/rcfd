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

## Amendment 2 (written after S1-S7, S1v and L1-L3 had run, before any run it governs)

What I knew when writing it (all registered runs so far, one each; numbers go to the addendum): S1 on E3 STANDS (the frozen psi converges
to `res_u` 1.9e-12 in 250 iterations; tolerance at iteration 510). S1 on B3 is FALSIFIED: with psi frozen the oscillation at the blade-root
leading edge GROWS (`max_du` 2.2 -> 17.8 m/s in cell c17, which touches the hub and the blade; period ratio 0.24-0.72) and the five largest
cells each flip the flux sign on two faces every iteration, so S2 stands by its record rule on B3. S3 (face_flux), S4 (laminar), S5
(partner_cell), S6 (tight solves) and S7 (outlet) are FALSIFIED on B3 (`rho_med` 0.90, 3.84, 1.09, 1.09, 1.02). S1v (Venkatakrishnan(5))
gives `rho_med` 0.94. L1 is FALSIFIED (`C_psi` 0.114, 0.046, 0.028 N m on E3, E2, E1 while L rises), L2 FALSIFIED (partner_cell |L| 0.0104,
0.0595, 0.1069 N m), L3 NOT EVALUABLE (the laminar empty passage does not settle: `|du|_2` 22-33 m/s). By the registered fix rule F-Z is not
eligible (S1 did not stand on B3) and F-A is not eligible (L1 falsified).

New candidate for the bladed passage (not in the registration; found from the S1 B3 location): **S8, the explicit rotating-frame source in
cells with a small momentum diagonal**. The `rotating_frame` acceleration (Coriolis `-2 Omega x w`) enters SIMPLEC from the previous iterate.
ADR 0085's single-cell model gives the error amplification `|lambda|^2 = (1 - alpha)^2 + (alpha x)^2` with `x = 2 Omega V / a` (Coriolis
rate 2 Omega, `a` the unrelaxed momentum diagonal), unstable for `x > 1.1` at `alpha = 0.9`. At the leading-edge stagnation corner the
through-flux and so `a` are small, so `x` can exceed 1 there; the limiter's switching would then be what bounds the growth on the default run
(consistent with the frozen-psi growth). Registered now:

* **R-x record** (no new solve beyond the reference): on the B3 state at iteration 1000, `x_c = 2 Omega V_c / a_c` for every cell, `a_c` the
  diagonal of the x-momentum convection (the kernel's `convection` with the limited gradient) plus Laplacian (the current `nu_face`) matrix
  before relaxation. Reported: the distribution of `x_c` (largest values and their cells) and `x_c` of the five largest-`du` cells of the S1
  B3 final record (c17, c665, c701, c341, c53). S8 is supported by the record iff at least 3 of those 5 have `x_c > 1.1`; contradicted iff
  none has.
* **S8 switch arm**: pseudo time `pseudo_mode = "physical"`, `pseudo_dt = 1 / Omega` (6.426e-3 s; `V Omega` on the diagonal, fixed point
  unchanged, `x` falls below 1 in every cell for `a` >= 0) from iteration 1000 on B3. **S9**: the same plus psi frozen at the switch.
  Prediction if S8 is the bladed mechanism and S1 the empty-passage one: S9 STANDS; S8 alone at least CONTRIBUTES. FALSIFIED by the
  registered rule (`rho_med >= 1/3`) for S9. The verdict rule of part S applies unchanged.

New reading for the empty passage, **L5**: the stalled state is a limit cycle whose amplitude grows with refinement (`max_du` 0.065, 0.30,
0.44-0.59 m/s on E3, E2, E1, the latter two at the hub cells of the C-grid nose region, z 0.094-0.098), so the stalled L might be a property
of the cycle rather than of a discrete solution. Experiment: the **freeze family** (E3, E2, E1; psi frozen at iteration 300, 300 more
iterations). L5 STANDS iff the frozen runs converge (tolerance reached) and |L| then decreases monotonically E3 -> E2 -> E1 with observed
order >= 0.5; FALSIFIED iff the frozen runs converge and |L(E1)| >= |L(E2)|; NOT EVALUABLE if any member does not reach the tolerance.

**Fix F-ZP** (replaces F-Z; eligible iff S9 stands): the case key `solver.linear_upwind_limiter_freeze = N` (as registered for F-Z) together
with the existing keys `pseudo_mode = "physical"`, `pseudo_dt = 1 / Omega`, N = 1000. Acceptance as registered for F-Z, with the combined
settings in every run: **Z1** empty passage first (E3, E2 converge by tolerance within 8000 iterations; L and swirl energy within the ADR 0086
cap-rule tolerances of the stalled values), then **Z2** bladed (B3, B2 converge by tolerance; torque and Delta p_ts within 0.5 % of ADR 0086;
iteration counts reported with the F1 prediction), **Z3** defaults. F-ZP is a convergence fix; it cannot remove the spurious swirl.

## Amendment 3 (a correction to amendment 2, written before any run it governs)

What I knew: nothing new has run since amendment 2. Its sentence "x falls below 1 in every cell for `a >= 0`" is wrong. With the pseudo
diagonal `D = V / dt` the single-cell model is `|lambda|^2 = ((a (1 - alpha) / alpha + D)^2 + (2 Omega V)^2) / (a / alpha + D)^2`; for `a = 0`
this is `1 + (2 Omega V / D)^2 > 1` for every finite D, and with `D = Omega V` (dt = 1/Omega) a cell is stable only for `a` above about
`1.25 Omega V` (x below about 1.6, against 1.1 without pseudo time). A pseudo-time step therefore only shrinks the unstable set; a cell with a
vanishing momentum diagonal stays unstable in this model whatever dt is. Added, fixed now: **S9'** = S9 with `pseudo_dt = 0.1 / Omega`
(stable for `a` above about `0.24 Omega V`), run after S9 whatever S9 gives, same rule. F-ZP takes the larger dt among S9 and S9' that
STANDS (none: F-ZP not eligible).

## Amendment 4 (written after R-x, S8, S9 and S9' had run, before any run it governs)

What I knew: R-x CONTRADICTS S8 (no cell of B3 has `x_c > 1.1`; the largest is 0.71 at the hub outlet corner; the five oscillating
leading-edge cells have 0.027-0.043). S8 (`rho_med` 0.44), S9 (0.36) and S9' (1.82) are FALSIFIED. With psi frozen and pseudo time on, the
leading-edge cells c17, c665, c701, c341, c53 settle into an EXACT period-two cycle (period ratio 0.000; amplitude 12.6 m/s with dt =
1/Omega, 4.0 m/s with 0.1/Omega), while the functionals are constant to 1e-13: the torque of those cycle states is 1.888 and 2.095 N m
(default stalled state 1.849). So the bladed functionals of the stalled runs are values of a cycle, not of a discrete solution, and they
depend on the iteration settings by up to 13 %. With psi frozen the remaining kinks of the map are the upwind directions; an exact period-two
cycle of a frozen linear operator needs an eigenvalue at or beyond -1 bounded by such a switch.

Registered now (same switch-arm rule; each run once):
* **S10** `upwind` (limiter off) from iteration 1000 on B3: tests whether the high-order (deferred) part of `linear_upwind` at the
  leading-edge corner drives the cycle. STANDS / FALSIFIED by the rule; its fixed point differs (first order), so only its own residuals count.
* **S11** psi frozen plus `velocity_correction = "face_flux"` from iteration 1000 on B3: tests whether the period-two cycle revealed by the
  freeze is the ADR 0084 period-two pressure mode (cell / face correction mismatch).
* **R-U record** (no verdict, location only): unlimited `linear_upwind` on E3 from the start, the five largest-`du` cells of iterations 1-6
  (before the guard trip at 7), to locate where the unlimited scheme fails.

## Amendment 5 (written after S10, S11 and R-U had run, before any run it governs)

What I knew: **S10 (`upwind` from iteration 1000) STANDS** on B3: `rho_med` 4.3e-3, `rho_du` 7.4e-3, both residuals below 1e-5 at
iteration 1114; what is left is a period-two cycle of 0.015 m/s at the outlet-hub corner (the first-order solution, torque 1.335 N m,
Delta p_ts -4.0 Pa, is not the second-order one). **S11 (freeze + face_flux) is FALSIFIED** (`rho_med` 0.91, `max_du` 10.7 m/s at the
leading-edge cells). R-U: unlimited `linear_upwind` fails first in the tip-layer cells at the shroud next to the seam near the leading-edge
plane (c4971, r 0.2477, z 0.104, 77 deg; growth about x10 per iteration). Reading: on the bladed passage the cycle is driven by the
high-order (deferred, explicit) part of `linear_upwind` in the five leading-edge root cells; the Barth-Jespersen switching bounds it (a
cycle of about three iterations); frozen at the factors of one iteration of the cycle (c17: psi_y = psi_z = 1 at iteration 1000, while the
other phases have 0.6 / 0.2 and 0 / 0) it is not bounded and grows into a period-two cycle.

Registered now:
* **S12** (diagnostic, B3, switch arm): psi frozen at the **minimum over a window** of iterations: from the switch at 1000 the kernel
  computes psi as usual in iterations 1001-1010 and keeps the per-cell, per-component minimum; from iteration 1011 on it uses that minimum
  (the most-limited value seen over at least three cycles). Prediction: STANDS (the high-order part at the leading-edge cells is then held
  at its most-limited phase, which the cycle shows to be stable enough to fall back from). Same rule.
* Kernel hook (written after this amendment): `SimpleSolver::limiter_freeze: Option<(usize, usize)>` = (N, W), default `None`: psi computed
  in iterations N - W + 1 ..= N is accumulated by its minimum and, after iteration N, stored in `frozen_limiter`. `None` leaves the solver
  bit-identical.
* **Fix F-ZM** (eligible iff S12 STANDS; replaces F-Z / F-ZP, which are not eligible): case key `solver.linear_upwind_limiter_freeze = N`
  (integer, 0 = off, default 0) with W = 10 fixed, only with `convection = "linear_upwind"`, `linear_upwind_limiter = "barth_jespersen"`
  and a steady `simple` / `simplec` algorithm (located error otherwise). N = 1000 in every run below. Acceptance, in this order:
  * **Z1 empty passage first** (E3, E2; cap 8000): both converge by tolerance (1e-5); L and swirl energy within the ADR 0086 cap-rule
    tolerances of the stalled values (as registered for F-Z). F-ZM is a convergence fix; it does not remove the spurious swirl.
  * **Z2 bladed** (B3, B2; cap 8000): both converge by tolerance. The torque and Delta p_ts are REPORTED against ADR 0086 without a limit:
    amendment 4 showed that the stalled functionals are values of a cycle that move by up to 13 % with the iteration settings, so a 0.5 %
    limit against them would test the cycle, not the fix (this replaces the F-Z 0.5 % limit; stated as a departure). Reported beside it: the
    residual of the converged state with psi recomputed (one probe iteration), and the iteration counts with the F1 prediction.
  * **Z3 defaults**: key absent and 0 bit-identical (u, p, phi `to_bits`, B3, 30 iterations, non-ignored test); `baseline_hashes.rs`
    passes unchanged; located errors for the invalid combinations.
