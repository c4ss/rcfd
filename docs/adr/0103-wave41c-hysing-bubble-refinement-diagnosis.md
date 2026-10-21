# ADR 0103: Wave 41C, diagnosis of the Hysing rising-bubble refinement miss (F-055)

Status: registered before the diagnostic runs. The decision rules below are fixed; results go into the addendum, misses and
deviations included. Clean-room: first principles and the Hysing et al. (2009) case-1 values already read in ADR 0096
(V_c,max 0.2417 at t 0.9213, y_c(3) 1.0813). Written at HEAD a2e0bfd (scratch copy `<scratch dir>`, target
`<target dir>`). Test file: `crates/rcfd-fv/tests/vof_bubble_diagnosis_w41c.rs`.

## Problem

ADR 0100 part B: the case-1 bubble (40 x 80, 80 x 160, 160 x 320, dt = 0.1/n, AMG pressure) gives y_c(3) 1.0282 / 1.0178 /
1.0124 and V_c,max 0.2204 / 0.2187 / 0.2180, peak at t 0.850 / 0.829 / 0.820. The sequence converges (order about 1) to a bubble
about 10 % too slow. An error that converges to a wrong limit is an inconsistency of the discrete model (it does not vanish as
h -> 0); errors of interpolation, upwinding, time step or interface width vanish in the limit and can only explain why the coarse
grid lands closer.

Gaps used below (ADR 0100): e_v(40) = 0.0213, e_v(80) = 0.0230, e_y(40) = 0.0531, e_y(80) = 0.0635.
"Significant shift" on 40 x 80 means |dV_c,max| >= 0.0043 or |dy_c(3)| >= 0.0106 (20 % of the 40 x 80 gap).

## Candidates

* C1 surface tension and curvature (CSF from the alpha field smoothed twice; curvature estimate, capillary balance).
* C2 interface advection and compression (algebraic compression flux, Zalesak limiter).
* C3 density and viscosity jump treatment: mixture rho and mu linear in alpha, arithmetic face mu, and the momentum diffusion in
  Laplacian form `div(mu grad u)` only.
* C4 pressure-velocity coupling with the capillary and gravity force (balanced-force face flux, transient Rhie-Chow term).
* C5 time stepping (implicit Euler, PISO without outer iterations, dt / dt_cap 0.47 to 0.95).
* C6 (added, not in ADR 0100's list) first-order upwind momentum convection (the default `ConvScheme::Upwind` of the set-up).

Prime suspect before any run (first principles): for incompressible flow with variable viscosity the viscous force is
`div(mu (grad u + grad u^T))`. With div u = 0 the second part is `div(mu grad u^T) = (grad u) . grad mu`, which is zero in each
fluid and a surface term at the viscosity jump (ratio 10 here). Without it the discrete model imposes continuity of
`mu du_t/dn` at the interface instead of the tangential-stress condition `mu (du_t/dn + du_n/dt)`. That error is a property of the
model, not of the grid, so it does not vanish under refinement. Size estimate: (mu_1 - mu_2) U / R times the perimeter,
9 x 0.24 / 0.25 x 1.57 = 14, against the net buoyancy (rho_1 - rho_2) g pi R^2 = 173: a few percent to 10 percent of the force
balance. This is plausibility only. The VOF momentum equation in vof.rs has the Laplacian form only (named as not delivered in
the F-055 note). The transposed part exists for the single-phase solver as `pimple::transposed_stress_source` (ADR 0032).

## Code change (opt-in, defaults bit-identical)

`VofSettings::full_viscous_stress: bool` (default `false`). When `true`, `VofSolver::step` adds
`pimple::transposed_stress_source(geom, u, mu_f)` (the explicit `div(mu (grad u)^T)` from the previous iterate, face `grad u`
from linearly interpolated Green-Gauss cell gradients, internal faces only, as ADR 0032) to the momentum right-hand side `m.b`
before relaxation, so it also enters `H` and therefore the corrected velocity. With `false` no line of the existing path runs
differently (baseline hashes untouched). The case tree keeps refusing `solver.viscous_stress = "full"` with multiphase unless D2
confirms (then the key is wired, see below).

## Smoke (rule d, before this text; what I knew when writing the rules)

* D0 control (defaults, 40 x 80, t = 3): y_c(3) 1.0282, V_c,max 0.2204 at 0.850, 15 s. Equal to ADR 0100, so the scratch build
  with the new (off) option reproduces the registered series point.
* 40 x 80 to t = 0.05 (20 steps), all finite: full stress V_c 0.0252, mu ratio 1 0.0245, PIMPLE 3 0.0254, linear upwind 0.0254,
  n_smooth 0 0.0253. These are early-time values only; no t = 3 value of any variant was seen.
* 160 x 320 with full stress to t = 0.3 (480 steps, 98 s): finite, y_c 0.5200, V_c 0.1242. So the explicit term is stable on the
  finest grid over the start-up; no reference value at t = 0.3 was compared.
* Static bubble (below) 40 x 40 to t = 0.05: finite; Laplace error -5.1 % (density ratio 10) and -5.0 % (ratio 1), max|U|
  1.4e-2 and 5.3e-3. These short-time values were seen before writing D1's rules.

## Registered experiments and decision rules

All rising-bubble runs: the w40d set-up, t = 3, dt = 0.1/n, AMG, one change each. Every test is `#[ignore]` (diagnostic, run once).

* D1 (C1, C4) static bubble: the Hysing fluids (rho 1000 / 100, mu 10 / 1, sigma 24.5) with g = 0, bubble R = 0.25 centred in the
  unit square, no-slip walls, n x n cells for n = 40, 80, 160 (and density ratio 1, rho 1000 / 1000, for n = 40, 80), dt = 0.1/n,
  t = 1. Measured: signed Laplace error e_L = (dp - sigma/R)/(sigma/R) (inside cells alpha < 0.01 and r < R - 3h, outside
  alpha > 0.99 and r > R + 4h), max|U| at t = 1.
  * Curvature-magnitude rule (with D3): S = (V_c,max(1.1 sigma) - V_c,max(0.9 sigma)) / 0.2. C1 is rejected as the primary cause
    if |S e_L(160)| < 0.0046 (20 % of e_v(80)); confirmed if |S e_L(160)| >= 0.0115 (50 %) with the sign that slows the bubble.
  * Parasitic-current rule: C1/C4 rejected as primary if max|U|(t = 1) on 160 x 160 <= 0.024 (10 % of V_ref) and not larger than
    on 80 x 80; flagged "contributing" if it grows from 80 to 160.
  * Density-jump rule (C3 rho, C4): the balanced-force treatment is rejected as a cause if e_L at ratio 10 and ratio 1 differ by
    less than 0.01 on 40 and 80.
* D1b (C1) initial curvature: -kappa of the initial exact circle (exact 4) in cells 0.05 < alpha < 0.95 on the bubble grids
  n = 20 ... 320 (construction only). Reading: rms relative error that does not fall from 80 to 320 means a non-convergent
  curvature estimate; that alone does not confirm C1 (D1 and D3 decide).
* D2 (C3 viscosity) transposed stress on, 40 x 80 and 80 x 160.
  * Confirm as primary cause if on 80 x 160 e_v <= 0.0115 and e_y <= 0.0318 (half the defaults' gaps) and the errors no longer
    grow from 40 to 80 (e(80) <= e(40) for both metrics).
  * Reject if on 80 x 160 |V_c,max - 0.2187| < 0.0046 and |y_c(3) - 1.0178| < 0.0127.
  * Otherwise "contributing", with the share of the gap it removes.
* D2c control (viscosity ratio 1, mu_2 = mu_1 = 10, 40 x 80, option off and on): the option must act at the jump only:
  |dV_c,max (on - off)| at ratio 1 <= 0.1 |dV_c,max (on - off)| at ratio 10 (D2 40 x 80 minus D0). If not, the D2 shift is not
  attributable to the jump and a D2 "confirm" is void.
* D3 (C1) sigma x 0.9 and x 1.1 on 40 x 80: gives S for D1. Significant-shift reading as above.
* D4 (C4, C5) on 40 x 80: dt / 2; PIMPLE with 3 outer iterations; transient Rhie-Chow off. Each rejected as primary cause if not
  a significant shift against D0.
* D5 (C2, C1) on 40 x 80: c_alpha 0.5; n_smooth 0; n_smooth 4. Same reading.
* D6 (C2) translation: AlphaTransport alone, the bubble moved by the prescribed uniform flux v = 0.2 for t = 2.5 (exact
  displacement 0.5) on 40 x 80 and 80 x 160, dt = 0.1/n, c_alpha 1. C2 rejected as a speed error if |y_c - 1.0| <= 1e-3 on both
  grids (2 % of e_y(40)); the L1 shape error is reported.
* D7 (C6) linear-upwind momentum convection on 40 x 80 and 80 x 160. Reported as shifts; by construction this error vanishes
  under refinement, so it can rank C6 but not explain the wrong limit.
* Not run, with reasons: the rising bubble at density ratio 1 (no buoyancy, nothing rises, no reference); surface tension off
  (no first-principles drag value for a deforming 2-D bubble at Re 35; the Stokes paradox rules out a closed form); harmonic face
  viscosity (an interpolation error that vanishes in the limit; ranked behind any limit-changing cause).

## The single 160 x 320 run (registered now, before any D2 result)

If D2 is "confirm" or "contributing", the one 160 x 320 run is the ADR 0100 B1-B4 series (40, 80, 160; limits unchanged) with
`full_viscous_stress = true`, as a new test `b1f_hysing_bubble_error_decreases_under_refinement_full_viscous_stress` in
`vof_bubble_refinement_w40d.rs`. The existing defaults test stays the ignored known miss (defaults are unchanged, so it still
misses). If b1f passes all of B1-B4, it is kept non-ignored (it adds about 30 min to the release gate) and `solver.viscous_stress
= "full"` is wired to the VOF host of the case tree (the refusal lifted, schema text updated); F-055 then meets the rising-bubble
clause with the option on only, and the note says so; whether `full` becomes the VOF default is the user's decision (it would move
every VOF result, so it is not done here). If b1f misses, it stays as an ignored known miss with the limits intact. The option is kept
in every outcome (registered now under wave-35 rule b): it is the physically complete viscous stress, it is off by default, and the
D2 / D2c diagnostics need it to stay reproducible. If D2 is "reject", no 160 x 320 run is made.
