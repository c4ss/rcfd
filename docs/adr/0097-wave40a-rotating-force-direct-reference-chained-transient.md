# ADR 0097: Wave 40A, independent rotating-force far field (F-136) and chained transient vs single transient (F-241)

Status: registered before the registered tests were run. Limits are fixed; a miss becomes an `#[ignore]`d `*_known_miss`
test with the limit unchanged. No library code changes: every addition is test code, so defaults and the baseline hashes
are untouched. Clean-room: no literature value is used; both references below are derived from first principles in this
ADR. The acceptance text of both items is unchanged. The user decided both plans: F-136 is set done if the independent
check passes, F-241 is set done if the chained transient matches the single transient run.

Gaps re-checked before writing ([partials group 5](../audit/partials-group-5.md), [partials group 9](../audit/partials-group-9.md)):
W3 in crates/rcfd-post/tests/fwh_impermeable.rs compares the FW-H code with `Rotor::harmonic_amplitude`, a Gutin-type
Bessel formula derived in this repository (ADR 0066) and never checked against Gutin (1936) or Lowson (1965). Both sides
of that comparison are project code. For F-241, C2 compares the chain with the single steady run; the transient stage
starts from the converged steady state, so that check is weak.

## F-136: direct retarded-time solution of a rotating point force (test file crates/rcfd-post/tests/fwh_impermeable.rs)

Reference (first principles). A point force F(t) acting on the fluid at y(t) is the source term -div[F delta(x - y(t))]
of the wave equation for p'. The free-space solution of (1/c0^2) d2q/dt2 - lap q = s(t) delta(x - y(t)) for a subsonic
path is the Lienard-Wiechert potential q = s(tau) / (4 pi r (1 - M_r)), evaluated at the retarded time tau = t - r/c0,
with r = |x - y(tau)| and M_r = v(tau).(x - y)/(c0 r). By linearity

  p'(x, t) = - sum_i d/dx_i [ F_i(tau) / (4 pi r (1 - M_r)) ]_ret.

The test evaluates this directly: retarded time by its own Newton iteration, the x derivative by the 8th-order central
difference `d8` already in the file (step h = 0.05 m) in the observer coordinates. Path and force are analytic,
y = RT (cos(Omega tau + phi_b), sin(Omega tau + phi_b), 0), F = T_b e_z + D_b e_phi(tau), not the sampled surface data.
Nothing of `rcfd_post::fwh` or `rcfd_post::rotor` is called. The FW-H code (formulation 1A) works with source-time
derivatives of sampled data; the reference has no time derivative at all, so the two paths share no formula. The sign
convention is the one W2 already uses (exact stationary dipole p' = x/(4 pi r^2)(F/r + F'/c0) = -d/dx[F/(4 pi r)]).

Smoke (rule d; reference only, the FW-H output was not compared): test `r0_direct_reference_self_check`.
* Stationary force F = 5 sin(w tau) e_x at the origin, w = (pi/2) c0, observer (60, 80, 0): largest error against the W2
  closed form 1.07e-13 of the amplitude (64 times over one period).
* Step study, rotor below, h = 0.05 against h = 0.1, RMS of the difference over one revolution relative to the RMS of the
  signal: largest 4.22e-10 (B = 3, theta = 90 degrees, r = 2000); all eight cases between 5.8e-12 and 4.2e-10. With an
  8th-order stencil the error at h = 0.05 is about 2^-8 of that. The reference error is therefore at least six orders of
  magnitude below the limits below.
* Signal levels: RMS 1.0e-5 to 1.0e-4 Pa at r = 2000 m (B = 3, theta = 135 degrees: 2.3e-6), 2.9e-3 to 1.5e-2 Pa at
  r = 15 m; the mean (static near field T cos(theta)/(4 pi r^2)) is 4.2e-7 Pa at 2000 m and 5.3e-3 Pa at 15 m.

Registered checks.
* R0 `r0_direct_reference_self_check` (kept as a test; limits set after the smoke above and stated as such): stationary
  dipole error <= 1e-9 of the amplitude; h against 2h <= 1e-8 relative for every rotor case of R1.
* R1 `r1_rotating_force_fwh_matches_direct_retarded_time_solution` (decides the item). Rotor of W3: `rotor_surface(B,
  30/B, 9/B, 256, 4)` (MT 0.3, RT 1, 256 samples per revolution, 4 revolutions), B = 1 and B = 3. Observers
  (r sin theta, 0, r cos theta) at r = 2000 m for theta = 45, 90, 135 degrees and at r = 15 m for theta = 60 degrees.
  Observer times: 256 per revolution over one revolution starting one revolution inside `valid_window` (as W3).
  Compared quantity: the FW-H `p_loading` series against the reference. Limits:
  - e = RMS(fwh - ref) / RMS(ref - mean(ref)) <= 1e-3 (mean removed in the denominator so that the static part cannot
    dilute the error at r = 15 m);
  - amplitude of each harmonic m B Omega, m = 1..3, whose reference amplitude is at least 1e-3 of the largest of the
    three: |a_fwh / a_ref - 1| <= 1e-3.
  The 1e-3 value is the FW-H series limit of ADR 0066 (W1, W2, W4); the reference error (R0) is below 1e-9, so the limit
  measures the FW-H error only.
* R2 `r2_gutin_type_formula_matches_direct_retarded_time_solution` (informative; does not decide the item):
  `Rotor::harmonic_amplitude(m, 2000, theta)` against the reference harmonic amplitudes for the r = 2000 observers of
  R1, same 1e-3 filter. Limit 1e-2 relative (the W3 limit). The far-field formula drops terms of relative size 1/(k r)
  with k r >= 0.3 * 2000 = 600, that is at most 1.7e-3 in amplitude, and they enter in quadrature to first order.

Decision rule: R1 passing sets F-136 done (with R0 passing). An R2 miss becomes `r2_..._known_miss` and is reported, and
the Gutin-type formula is then marked as disagreeing with the direct solution in the note; it does not block the flip
since R1 checks the FW-H code without it.

## F-241: chained transient segments against one transient run from rest (test file crates/rcfd-api/tests/chain_w19c.rs)

Set-up. The cavity of C1/C2 (`BASE`: 16 x 16, nu 0.01, lid 1, upwind) switched to the transient solver of C1
(`TRANSIENT_SOLVER`: PISO, dt 0.05, end_time 1.0, other solver settings default).
* Chained: `chain.sequence = ["first", "second"]`, stage `first` sets `solver.end_time=0.5`, stage `second` sets
  `solver.end_time=1.0`; `chain::run_chain` from rest (no `restart.from`). The second stage restarts from
  `out/first/restart.rcfd` through the chain machinery; its restart report must be `exact`. The final state is loaded from
  `out/second/restart.rcfd`.
* Single: the same case without the chain section, `Simulation::new` and the run-control driver from rest to 1.0.

Registered check T1 `c7_chained_transient_segments_equal_the_single_transient_run_bit_for_bit`: ux, uy, p, face flux and
time identical bit for bit (tolerance 0), the second stage's restart report `exact`, both runs 20 steps.

Why bitwise is the right tolerance. The same-mesh transient restart restores u, p, flux, time, step number and the full
time history (`PimpleState::restore`), and restart_w19c S2 already shows a restored run continues bit for bit. What the
chain adds is a stage with another `end_time`. The step size is dt = min(0.05, end_time - time) and a run stops when
time >= end_time - 1e-12. Floating-point accumulation of 0.05 gives 0.49999999999999994 after ten steps, so stage
`first` takes ten full steps of 0.05 and stops without a clipped step, the same ten steps as the single run; both then
take the same remaining steps to 1.0 (the last one clipped identically). The split at 0.5 was chosen for this reason, by
arithmetic before any run. Splits at 0.3 or 0.7 would clip the last step of the first stage by a few ulp (0.3: last step
0.04999999999999999), which changes dt and rules out bitwise equality; such splits are not covered by T1.

Smoke (rule d, no field comparison): chain with stages ending at 0.1 and 0.2: exit 0, both stages run, stage `b` restart
report `exact`, stage `a` finished at 0.1 after 2 steps, stage `b` at 0.2 after 4 steps (step count continues).

Scope statement. The acceptance names a steady-then-transient chain; that remains covered by C1 (bitwise equal to the
manual procedure) and C2 (close to the steady run). T1 adds the reading the user chose: chained transient segments reach
the same final state as one uninterrupted transient run.

Decision rule: T1 passing sets F-241 done; a miss becomes `c7_..._known_miss` and the item stays partial.

## Addendum 1: measured values (first and only run of the registered tests)

All registered checks pass; no amendment, no deviation from the registered set-ups.

F-136 (crates/rcfd-post/tests/fwh_impermeable.rs):
* R0: stationary dipole 1.07e-13 of the amplitude (limit 1e-9); h against 2h 5.8e-12 to 4.2e-10 (limit 1e-8).
* R1: FW-H series against the direct retarded-time solution, e = 1.87e-8 to 2.03e-8 for B = 1 and 3.0e-8 to 3.4e-8 for
  B = 3, at r = 2000 m (45, 90, 135 degrees) and r = 15 m (60 degrees) (limit 1e-3). Harmonic amplitudes 1.6e-8 to
  2.3e-7, the largest for B = 3, m = 3 at r = 15 m, whose amplitude is 3e-3 of m = 1 (limit 1e-3).
* R2: the in-repo Gutin-type formula against the direct solution at r = 2000 m, 8.5e-8 to 3.98e-5, the largest at
  135 degrees, B = 1, m = 1 (limit 1e-2). The same 135-degree pattern W3 showed (4.0e-5) appears here, so it belongs to the
  far-field formula, not to the FW-H code. The formula is now checked against an independent solution; its attribution
  to Gutin (1936) and Lowson (1965) is still UNVERIFIED (the papers were not read).

F-136 is set done: R1 reproduces the rotating-dipole (rotating point force) field from an independent first-principles
solution in near and far field, the monopole checks of ADR 0013 stand, and the sampled-file path is covered by A7 and S3.

F-241 (crates/rcfd-api/tests/chain_w19c.rs):
* T1 `c7_...`: the chained transient (0 to 0.5, exact restart, 0.5 to 1.0; 10 and 20 steps) equals the single transient
  run from rest in ux, uy, p, face flux and time bit for bit (tolerance 0). Stage `second` reported `exact`.

F-241 is set done under the reading the user chose. The limitation stated above holds: a split whose stage end clips a
step by a few ulp is not bitwise equal and is not tested.
