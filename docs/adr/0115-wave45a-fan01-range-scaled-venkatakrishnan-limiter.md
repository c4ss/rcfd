# ADR 0115: Wave 45A, an opt-in range-scaled Venkatakrishnan limiter for `linear_upwind` on the stalled FAN-01 passage

Status: **ACCEPTED (registered 2026-10-26) before any registered run.** Limits and decision rules below are not changed after a run. A
missed limit becomes an ignored `*_known_miss` test with the limit intact. A defective set-up gets an amendment written before the
corrected run. Defaults stay bit-identical. `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded. Clean-room: first principles,
our own ADRs, and the public literature listed at the end (UNVERIFIED). Parents: ADRs 0081 (part L), 0086, 0110, 0111. Written on branch
`w45a` from main 25422d4 (worktree `<scratch dir>`, target `<target dir>`). The user accepted the drafted
decisions: the range-scaled form, the k set {0.002, 0.01, 0.05} with the smallest passing k, no case key if the empty passage passes and B3
misses (the library variant stays), and P4 required unless its feasibility smoke shows it infeasible, then amended before registration.

## Context (what is known, with numbers)

* The ADR 0086 FAN-01 runs stall in limit cycles of the Barth-Jespersen (BJ) limiter (ADR 0110 part S).
  * On the empty passage E3 the factors psi flip with period two. Freezing them converges E3 (S1: `res_u` 1.9e-12), but the frozen fixed
    point is not a BJ fixed point (its probe returns to 3.2e-5 / 1.4e-5), and freezing diverges on E2 and E1 (L5).
  * On the bladed B3, freezing psi makes the oscillation grow (`max_du` 2.2 -> 16.8 m/s at c17, the blade-root leading edge).
  * First-order `upwind` converges B3 (S10) to a different, first-order solution: torque 1.335 N m against the stalled 1.849 N m.
* ADR 0111: the linearisation at a cycle point found the leading eigenvalue -1.3306 at hub mid-chord cells; it did not predict the frozen
  iteration and attributed nothing. ADR 0111 F-R (relaxed deferred correction) converged E3 and E1 to true BJ fixed points, missed on E2
  (`rho_med` 0.103) and was removed.
* ADR 0110 S1v ran the library's existing `Limiter::Venkatakrishnan(5)` on B3 from iteration 1000: `rho_med` 0.94, `rho_du` 1.29, torque
  1.774 N m, no effect on the stall. That variant uses `eps^2 = (k V^(1/3))^3` (`gradient.rs`, checked): a length cubed compared with velocity
  differences squared. With k = 5 and V^(1/3) about 0.011 m it is (0.055 m)^3 = 1.7e-4, against psi-relevant differences of 0.1-10 m/s, so
  on this mesh it is Venkatakrishnan's smooth function with eps effectively zero. S1v therefore did not test an eps large enough to switch
  the limiter off where the local variation is small, the property the literature gives for its better steady convergence (recalled,
  UNVERIFIED).
* Unlimited `linear_upwind` trips the steady guard at iteration 7 on E3 (ADR 0110 R-U, tip-layer cells next to the seam). An eps that is
  too large brings that back, so eps is a bounded trade-off and its values are registered here, not tuned.

## What I already knew when registering (rules c, d; disclosed)

Code written before this registration (committed after it, in the implementation commit): the library variant, the case key and its
checks, and every test below. Runs made before registration, each once:
* **P5 hashes at main.** The P5 test was compiled against unchanged library code (main 511bed4) and its three FNV-1a hashes recorded:
  key absent and `none` 0xec721116c0963182 (both trip the guard, `velocity_runaway`, within the 30 iterations: unlimited `linear_upwind`
  diverges on B3, as on E3), `barth_jespersen` 0x26e5a0c524957698. With the implementation, P5, P6 and the located-error test were run once
  during development and passed (exact checks without a tunable number).
* **Z set-up smoke** (`w45a_smoke`): 20 BJ + 20 iterations at each k on E3, E2, E1 and B3: all finite, no guard trip. Cost: E3 0.035 s,
  E2 0.09 s, E1 0.27 s, B3 0.04 s per iteration (a Z1 sweep at one k is about 21 min, Z2 about 2 min).
* **The range R at iteration 1000 of the default run** (u_x / u_y / u_z, m/s): E3 42.28 / 33.29 / 9.10; E2 41.57 / 32.52 / 10.88; E1
  42.10 / 32.54 / 10.38; B3 45.79 / 47.31 / 58.85. **The draft's "about 78 m/s" is wrong** (it took 2 Omega r_shroud for every component);
  R is per component and on the empty passage u_z has the smallest range. The resulting eps = k R spans 0.018 m/s (k 0.002, u_z, E3) to
  2.9 m/s (k 0.05, u_z, B3), which still brackets the measured cycle amplitudes (0.065 m/s on E3 to 2.2 m/s on B3). The k set is kept
  (user decision).
* **P4 feasibility smoke** (`w45a_p4_feasibility_smoke`, BJ and unlimited only; the new limiter was not run on it):
  * the drafted doubly periodic set-up does **not** converge: after 20 000 iterations `res_u` / `res_p` 0.044 / 0.099 (16^2) and 0.012 /
    0.025 (32^2) with either scheme, L2 error 0.29-0.30 (16^2) and 0.19 (32^2). Diagnosis (`w45a_tg_diag`, 16^2 unlimited, 4000
    iterations, started from the exact field): at nu = 1 it converges to 3e-15 (L2 6.9e-3); at nu = 0.1 it leaves the exact state between
    iterations 500 and 1000 (L2 4.8e-2 -> 3.0e-1, the pressure range falls from 1.0 to 0.71-0.77) and keeps moving. The forced cellular
    flow at U/(nu k) = 10 in the periodic box is not an attracting steady state of the iteration (not further diagnosed; it is the same with
    and without a limiter). **Defective set-up: P4 is amended below before registration.**
  * the corrected set-up (the same exact solution, the exact velocity prescribed on all four sides, as in the Kovasznay L3 set-up)
    converges to the tolerance 1e-10: BJ in 659 / 3248 / 5879 iterations (16^2 / 32^2 / 64^2), L2 2.625e-2 / 1.793e-3 / 4.724e-4 (fine-pair
    order 1.92), share of cells with psi < 0.99 0.125 / 0.031 / 0.008; unlimited in 2557 / 3634 / 5821 iterations, L2 2.544e-2 / 1.719e-3 /
    4.712e-4 (fine-pair order 1.87). At 64^2 BJ costs 0.26 % of L2 error against unlimited: the P4 error limit is narrow, and it is kept as
    drafted.
* Not run before registration: P1, P2, P3, P4 with the new limiter, any Z or Z2 arm.

## Decisions

1. **D1, a new library variant** `rcfd_fv::gradient::Limiter::VenkatakrishnanRange(k)`. `Venkatakrishnan(f64)` and `BarthJespersen` are
   untouched (same expressions; `mms_poor_meshes.rs` and `limiters_bound_reconstruction` cannot move). The y-function is the one already in
   `gradient.rs`, with `eps^2 = (k R)^2`, R = max over cells - min over cells of the limited component at the current iterate (boundary
   values not included). R = 0 (a constant field) gives psi = 1 through the exactly zero gradient; there is no special branch.
2. **D2, case key** `solver.linear_upwind_limiter = "venkatakrishnan_range"` with `solver.linear_upwind_limiter_k` (float, default 0.01).
   Same scope and located error as the ADR 0081 L0c check (steady `simple` / `simplec` with `linear_upwind`). Located errors on
   `solver.linear_upwind_limiter_k`: k not finite or <= 0 with the value; k different from its default without the value (the settings
   tree does not record whether a key was written, so an explicit `0.01` without the value cannot be detected; stated). The default stays
   `none`; key absent, `none` and `barth_jespersen` are bit-identical by construction and by P5.
3. **D3, registered k set** {0.002, 0.01, 0.05}. No other k is run under this ADR.
4. **D4, selection:** k* = the smallest k of D3 that passes Z1 on all three empty members and Z2. Z1 is run at all three k (each member run
   whatever the previous one gave); Z2 at every k that passes Z1.
5. **D5, scope of limiting** as ADR 0081: only the convection face extrapolation of the momentum components (relative velocity, per
   Cartesian component).

## Registered checks

Common protocol (Z arms): the ADR 0110 switch arm, `arm()` in `fan01_stall_w43c.rs`: 1000 default BJ iterations, then the limiter switched
through `simple_solver_mut`, 2000 more iterations, no early stop. Windows: the last 500 iterations before the switch and the last 500 of
the arm; `rho_med`, `rho_du` as ADR 0110 (`verdict`). A steady-guard trip ends the arm as a miss with the guard's located message
recorded. Wall clock over 60 minutes for one test: NOT EVALUABLE. Recorded in every arm: the first iteration at which the tolerance is met
and the first at which `is_converged()` is true (new field `Arm::conv_first`).

### Z1, the empty passage (E3, E2, E1; exact absolute swirl 0), each member at each k (`w45a_z1_k0002`, `_k001`, `_k005`)

* **(a)** `rho_med <= 0.01` and `rho_du <= 0.01`.
* **(b)** `is_converged()` true at the end (the full conjunction, SST `res_nut` included).
* **(c)** probe: one more iteration with the same limiter and k gives `res_u`, `res_p` <= 1e-5 (a fixed point of the new map).
* **(d)** one-sided exact-swirl check: `|L| <= L_c + max(0.005 L_c, 1e-4 N m)` and `E_s <= E_c + max(0.005 E_c, 0.01 Pa)` with (L_c, E_c) =
  E3 (0.009518 N m, 6.26632 Pa), E2 (0.059612, 9.23040), E1 (0.105977, 5.48377), the ADR 0111 window-worst values (the arm also prints
  its own window-worst values). Reported, no limit: L and E_s per member and their trend.

### Z2, B3 (every k that passes Z1; `w45a_z2_k0002`, `_k001`, `_k005`)

(a), (b), (c) as Z1. Torque and Delta p_ts reported against 1.849 N m / 46.6 Pa (stalled BJ) and 1.774 N m (S1v), without a limit.

### Accuracy and boundedness (all three k)

The draft applied these to k* and every k that passes Z1. **Registered here at all three k** (the library variant stays whatever happens
and accepts any k, so its accuracy is checked at every registered k). The key decision below uses k*.
* **P1, the step profile** (`limiters_bound_reconstruction` set-up, 40 x 1 x 1 box; `rcfd-fv` unit test `w45a_range_venkatakrishnan_p1_p2`):
  psi in [0, 1]; reconstructed face values within [-0.08, 1.08]; psi > 0.99 at cell 5 (flat region).
* **P2:** psi = 1 within 1e-12 in cells 5..35 for a linear field; psi = 1 exactly for a constant field.
* **P3, Kovasznay** (the ADR 0081 L3 set-up unchanged, Re 40, 12x16 / 24x32 / 48x64; `w45a_p3_kovasznay_order`): fine-pair L2 order >= 1.8.
* **P4, a smooth interior extremum (amended before registration, see "What I already knew"):** the exact steady Navier-Stokes solution
  u = U (sin x cos y, -cos x sin y), p = (U^2/4)(cos 2x + cos 2y), body force 2 nu u_exact (cell values and face values `f . S_f` at the
  face centres), on [0, 2 pi]^2 with **the exact velocity on all four sides** instead of doubly periodic; u_x has interior extrema at
  (pi/2, pi) and (3 pi/2, pi), u_y at (pi, pi/2) and (pi, 3 pi/2). U = 1, nu = 0.1, meshes 16^2 / 32^2 / 64^2, SIMPLE 0.7 / 0.3,
  tolerances 1e-10, cap 20 000, started from the exact velocity (`w45a_p4_taylor_green_order`). Limits: every mesh converges; the
  fine-pair L2 order >= 1.8; the 64^2 L2 error <= the BJ 64^2 L2 error of the same run. Reported: L-inf orders, the share of cells with
  psi < 0.99. The doubly periodic version stays as the ignored smoke record.
* **P3 and P4 route** (changed from the draft's "through the case key", registered here): the case is built with `barth_jespersen` and the
  library variant is set through `simple_solver_mut` before iteration 1, which P6 shows is bit-identical to the key. This keeps P3 and P4
  valid if the key is removed under the decision rule. The P4 body force is set the same way (the case tree has no spatially varying body
  force; `step()` does not overwrite it without energy, UDF or sources, `sim.rs`).
* **P6, the key reaches the solver** (`w45a_p6_case_key_reaches_the_solver`, non-ignored): on E3, the case key with k = 0.01 against the BJ
  case with the library variant set before iteration 1, 30 iterations: u, p, phi `to_bits` identical, no guard trip. k = 0.01 (the default)
  instead of the draft's k*, which is known only after the Z runs; the code path is the same for every k.
* **P5, defaults** (non-ignored): key absent, `none`, `barth_jespersen` give B3 after 30 iterations the FNV-1a hashes recorded at main
  (above; `w45a_p5_defaults_are_bit_identical_to_main`); `baseline_hashes.rs`, `limiters_bound_reconstruction`, `mms_poor_meshes` pass
  unchanged; located errors (`w45a_range_limiter_located_errors`): `venkatakrishnan_range` with `upwind`, `pimple`, `coupled` (path
  `solver.linear_upwind_limiter`); k = 0, k < 0, k without the value, k with `barth_jespersen` (path `solver.linear_upwind_limiter_k`).

### Negative control

N1 is the ADR 0110 reference itself (BJ, E3): its `rho_med` is 1 by construction against the 0.01 limit, so Z1 discriminates.

## Status decision rule (registered)

* The case key value is kept opt-in and documented iff P1-P6 pass at k*, and Z1 (all three members) and Z2 pass at k*.
* If Z1 passes at some k and Z2 misses at every such k: the key is **not** added ("converges the empty passage only"). If Z1 misses at
  every k: nothing is added.
* In every miss case the library variant `VenkatakrishnanRange` stays, with the Z, P1-P4 tests (ignored known misses where they missed).
  Removed with the key, registered now: the schema value and `linear_upwind_limiter_k`, the `sim.rs` mapping and checks, the
  `docs/case-schema.md` text, the located-error test and P6 (their results stay in the addendum). P5's default-hash test stays.
* A registered known miss: a complete registered run whose measured value violates a limit, or a guard trip in an arm. Not a known miss:
  a harness error (amendment and re-run), a run stopped by the 60-minute cap (NOT EVALUABLE).
* F-126 and the fan milestone stay partial whatever happens.

## Run order

P1/P2, P5, P6, located errors (seconds); P3, P4; Z1 at k = 0.002, 0.01, 0.05; Z2 at every k that passed Z1. Each registered run once, at
most two release runs at a time across the wave.

## Out of scope

A fully differentiable limiter; the Michalak / Ollivier-Gooch smoothed BJ; limiter freezing and relaxed deferred correction; any default
change; meshes finer than F1, B2 / B1; agreement with FAN-01 data; the growth of the spurious L with refinement (ADR 0110 part L); the
`rotating_frame` seam coefficient (ADR 0117).

## Addendum

To be written after the registered runs.

## References (public literature; UNVERIFIED in content)

* T. J. Barth, D. C. Jespersen, "The design and application of upwind schemes on unstructured meshes", AIAA paper 89-0366 (1989).
* V. Venkatakrishnan, "Convergence to steady state solutions of the Euler equations on unstructured grids with limiters",
  J. Comput. Phys. 118 (1995).
* C. Michalak, C. Ollivier-Gooch, "Accuracy preserving limiter for the high-order accurate solution of the Euler equations",
  J. Comput. Phys. 228 (2009).

No limit depends on these. The forced Taylor-Green solution is checked from first principles: (u.grad)u = (U^2/2)(sin 2x, sin 2y) =
-grad p, and nu lap u = -2 nu u is balanced by the body force.
