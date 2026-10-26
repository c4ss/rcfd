# ADR 0119: Wave 46B, a reduced-order axial-fan tier (strip theory, losses, fan curve, operating point, fan laws, steady-loading tones)

Status: **ACCEPTED (registered 2026-10-26)**. The limits and decision rules below were fixed before any registered test ran. House
rules of ADRs 0110-0117:
* a missed limit becomes an ignored `*_known_miss` test with the limit intact;
* a defective set-up gets a new corrected test, registered here as an amendment before it runs;
* defaults stay bit-identical, and `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded.

Written at main 8bd609a (worktree `<scratch dir>`, branch `w46b`, target `<target dir>`). Serves parts C7,
C4/C5 (reduced-order part) and D1 of `docs/fan-project-breakdown.md`. Clean-room: first principles, our own ADRs, and the labelled
recollections below. Parents: ADRs 0012 (F-113 lumped fan curve, `bisect`, `network_pressure`), 0036 (F-218 actuator disk), 0065
(`rcfd_mesh::fan`), 0066 (Gutin-type closed form, `rcfd_post::rotor`), 0075 (FAN-01 data), 0078 (arm E, blade-element estimate).

## What already existed (re-checked at 8bd609a)

* F-218 (`rcfd_fv::actuator`): momentum-theory disk only. ADR 0036 lists "blade-element lookup, tip loss, trim" as NOT provided; there
  is no blade-element code to reuse.
* F-113 (`rcfd_fv::lumped`): a piecewise-linear `FanCurve`, `network_pressure(p_ref, r1, r2, q)` and `bisect` (decreasing function,
  200 halvings). Reused: the system curve is `network_pressure(0, l, k, q)`, and every root (strip swirl, operating point) is found by
  `bisect` after a bracket is established.
* ADR 0078 arm E: an inviscid blade-element estimate of FAN-01 written inside a test file (`fan01_diagnosis_w35c.rs`), not a library.
  The new module uses the same strip equations (cascade momentum, vector-mean velocity, thin-airfoil lift); arm E is not edited.
* F-243 / ADR 0066 (`rcfd_post::rotor`): `bessel_j` and `Rotor::harmonic_amplitude`, the Gutin-type closed form, checked against the
  FW-H solver (errors <= 4e-5). Reused. One change: a signed `Rotor::harmonic_pressure` is added and `harmonic_amplitude` now returns
  its absolute value. `|k j load| == k |j| |load|` holds bit for bit in IEEE arithmetic, so `harmonic_amplitude` is unchanged (D1).
* `rcfd_mesh::fan` (`AxialFan`, `Station`, `camber_line`, `gauss_legendre`). Another wave-46 agent is extending this schema; the new
  input struct `RomFan` is small (blade count, hub/tip radius, gap, family, sections of radius/chord/stagger/camber/camber position)
  and `RomFan::from_axial_fan` only reads fields that exist on main.

## What I already knew when registering (disclosed)

* Code written before registration (the rule-d smoke needs it): `crates/rcfd-post/src/fan_rom.rs`, the `harmonic_pressure` split in
  `rotor.rs`, `crates/rcfd-api/src/design/fan_rom_eval.rs`, and the smoke `smoke_w46b` in `crates/rcfd-post/tests/fan_rom_w46b.rs`.
* Run before registration: `smoke_w46b` only (values below), and the two unit tests in `fan_rom.rs` (alpha_L0 parabola, no-intersection
  message). No registered check was run. FAN-01 was constructed (sections parsed, validated) but NOT evaluated.

## The model (decisions D-M1 to D-M8)

All quantities per strip; incompressible, steady, no inlet swirl.

* **D-M1 strips.** `n_strips` (default 40) equal radial strips on `[r_hub, r_tip]`, midpoint rule; each is an independent 2-D cascade
  (no radial velocity, no radial-equilibrium coupling). Axial velocity `c_x = Q / (pi (r_tip^2 - r_hub^2))`, uniform over the bladed
  annulus. Sections are interpolated linearly in r (clamped).
* **D-M2 velocity triangles and cascade momentum (first principles).** `U = Omega r`; `w_inf = U - dc/2`; `W = sqrt(c_x^2 + w_inf^2)`;
  `beta = atan2(w_inf, c_x)` from the axis; `alpha_e = beta - stagger - alpha_L0`. Pitch `s = 2 pi r / B`. Forces on the fluid per
  blade per unit span: `F_theta = rho W^2 c (C_L cos beta + C_D sin beta) / 2`, `F_x = rho W^2 c (C_L sin beta - C_D cos beta) / 2`.
  Swirl from `F_theta = rho s c_x dc`, solved by `bisect` on a branch (decreasing in `dc`). Total pressure rise of the strip from axial
  momentum: `dp0 = F_x / s + rho dc^2 / 2`. Torque `B sum F_theta r dr`, power `Omega` times torque, thrust `B sum F_x dr`.
  From these, `P = Q dp0_mean + sum B D W dr` is an identity (`c_x F_x + D W = F_theta w_inf`), checked in L1.
* **D-M3 section model: thin cambered plate with stall (LABELLED).** Attached: `C_L = 2 pi alpha_e` (thin-airfoil theory, first
  principles; `alpha_L0` by the thin-airfoil integral of the camber line, 24-point Gauss-Legendre on the two smooth pieces). Stall when
  `2 pi |alpha_e| > cl_max`; post-stall flat plate `C_N = cd_normal sin alpha_e`, `C_L = C_N cos alpha_e`, extra drag
  `C_N sin alpha_e`. `cl_max = 1.1` (the ADR 0078 EB value, ASSUMED, UNVERIFIED), `cd_normal = 2.0` (normal-plate drag, recollection,
  UNVERIFIED). Thickness is not modelled. No cascade (solidity) correction: isolated-section lift over-predicts at solidity near 1.
  Branch rule: the attached root if its `|alpha_e| <= cl_max / 2 pi`; else the post-stall root if its `|alpha_e|` is above; else
  (neither consistent, hysteresis gap) `C_L = +-cl_max` ("capped", counted as stalled). Preferring the attached root is the
  throttling-from-free-delivery convention.
* **D-M4 profile drag.** Laminar Blasius friction on both sides, `2 x 1.328 / sqrt(Re_c)`, `Re_c = W c / nu` (first principles; a lower
  bound at chord Re 2e4-2e5, where laminar separation bubbles add drag that is not modelled). Strips with Re outside [2e4, 2e5] are
  counted (`re_outside`), not refused.
* **D-M5 hub and endwall loss.** Howell-type drag added to `C_D`: annulus `0.020 s / h` and secondary `0.018 C_L^2` (h = blade
  height; recollection of Howell's correlations via Dixon's textbook, UNVERIFIED). The dissipation is `D W` per unit span, so these
  enter the ledger through the drag.
* **D-M6 tip-gap leakage (one model, not two).** The through-flow area is the bladed annulus, so no flow passes the gap axially; the
  gap loss is a pressure-driven leakage jet: `P_tip = B C_d tau c dp_tip sqrt(2 dp_tip / rho)`, `dp_tip = rho W^2 |C_L| / 2` of the
  outermost strip (mean pressure difference across the tip section), all of the jet head dissipated. `C_d = pi / (pi + 2)` (2-D
  free-streamline slot contraction coefficient; recollection, UNVERIFIED). Deducted from the total pressure rise; it does not change
  the shaft power.
* **D-M7 static pressure.** Total-to-static for free discharge with no stator: `dp_s = dp_t - rho c_x^2 / 2 - (swirl kinetic energy
  flux) / Q`. Efficiencies `eta_t = Q dp_t / P`, `eta_s = Q dp_s / P` (NaN when `P <= 0`).
* **D-M8 operating point.** System `dp = k Q^2 + l Q` against `dp_s`. Scan `n_scan` intervals of `[q_lo, q_hi]`; the highest-flow
  fan-above-to-below crossing is bisected. Located failures: an evaluation error (with its flow), no crossing (range and the largest
  fan-minus-system value), a rising fan characteristic at the crossing (central-difference slope > 0, "stall region"), or stalled
  strips at the crossing (their radii). Several crossings are reported in `crossings`, not refused.
* **Fan laws.** `fan_law(q, dp, P, n, rho_ratio) = (q n, dp rho_ratio n^2, P rho_ratio n^3)`; exact for the model only at equal
  Reynolds number (Blasius drag depends on Re).
* **D1 tones.** `tonal_harmonic`: the signed sum over strips of `Rotor::harmonic_pressure` with ring radius r_i, thrust `B F_x dr` and
  torque `B F_theta r dr` (all blades share an azimuth at every radius; lean/sweep phase not modelled).
* **Design evaluator (opt-in).** `rcfd_api::design::fan_rom_eval::FanRomEvaluator` implements `design::study::Evaluator`: a hook maps
  the candidate to a `RomFan`; each condition (keys `rpm`, `rho`, `nu`, `system_k`, `system_l`; an unknown key is a located failure)
  returns `q`, `dp_s`, `dp_t`, `power`, `torque`, `eta_s`, `eta_t`, `tone1_pa`, or a failed condition with the operating-point reason.
  Not wired into the `design.*` service tools. Nothing existing changes.

What it is NOT: not validated against any measured fan; no cascade, deviation, 3-D, radial equilibrium, bubble, inflow distortion,
strut, unsteady-loading, thickness-noise or compressibility effects. A design ranking from it is a hypothesis for CFD or test.

## Registered checks (`crates/rcfd-post/tests/fan_rom_w46b.rs` unless named)

Common set-up "C" (a constructed 120 mm class fan): B = 7, r_hub 0.030 m, r_tip 0.058 m, gap 1 mm, chord 30 mm, NACA camber 0.04 at
0.5 (both from the hub and tip sections), 3000 rpm, rho 1.2, nu 1.5e-5; stagger from the free-vortex design `free_vortex_design` at
Q_D = 0.045 m^3/s with `K = 45 Pa / (rho Omega)` (ideal 45 Pa), with the model under test. Default model unless stated.

* **E1 Euler work.** At q/Q_D in {0.6, 1.0, 1.2}: per strip `|U dc - [(c2^2 - c1^2)/2 + (W1^2 - W2^2)/2]| <= 1e-12 U^2`, with
  `c1^2 = c_x^2`, `c2^2 = c_x^2 + dc^2`, `W1^2 = c_x^2 + U^2`, `W2^2 = c_x^2 + (U - dc)^2`. Reason: algebraic identity, rounding only.
  NC-E1: `W2` built with `U + dc` must miss by >= 10x.
* **E2 free vortex.** C at Q_D: per strip `|dc - K/r| <= 1e-12 K/r`; torque `rho Q K`, power `rho Q Omega K` and the mass-averaged
  Euler work `Omega K` each within 1e-12 relative. Reason: closed form; the midpoint rule is exact because `r dc` is constant and the
  weight `r dr` is linear. NC-E2: design with the lossless model, evaluate with the default model (the drag missing from the design);
  the per-strip error must exceed 1e-11.
* **Z1 zero loss.** C designed and evaluated with `RomModel::lossless()`, at q/Q_D in {0.7, 1.0, 1.2}: `dp_t` against
  `rho sum U dc dQ / Q` and `eta_t` against 1, each <= 1e-12 relative; at Q_D `dp_t = 45 Pa` (closed form) within 1e-12. NC-Z1: the
  momentum route without the swirl term `rho dc^2/2` must miss by >= 10x.
* **Z2 zero lift.** C with zero camber and stagger `atan2(Omega r, c_x)` at every strip mid radius (sections at the strip radii), at
  Q_D, lossless: `|dc| <= 1e-12 U` per strip, `|dp_t| <= 1e-12 rho U_tip^2`, `|P| <= 1e-12 rho Q U_tip^2`. With the default model:
  `dp_t < 0` and `P > 0` (drag only absorbs power and lowers the pressure). NC-Z2: camber 0.02, lossless, must give
  `|dp_t| >= 1e-11 rho U_tip^2`.
* **L1 power ledger.** C, default model, at q/Q_D in {0.5, 0.75, 1.0, 1.2} (the first two have stalled strips):
  `|P - (Q dp_t + drag_loss + tip_loss)| <= 1e-12 P`. Each term is computed separately (torque integral; strip `dp0` from axial
  momentum; `B D W dr`; the tip formula). NCs, each must miss by >= 10x: (a) tip loss left out of the ledger; (b) drag loss left out;
  (c) `dp0` recomputed with the drag sign flipped in `F_x`; (d) `dp0` recomputed without the swirl term.
* **L2 tip loss.** C at Q_D: `tip_loss` equals `B C_d tau c dp sqrt(2 dp / rho)` recomputed from the outermost strip, <= 1e-12
  relative; doubling the gap doubles it (<= 1e-12) and lowers `eta_t`; gap 0 gives `tip_loss = 0` and the highest `eta_t`. NC-L2: the
  formula with `dp_tip` from the inlet relative speed `W1` must miss by >= 10x.
* **L3 drag terms.** C at Q_D (all strips attached): `drag_loss` equals `sum B rho W^3 c C_D dr / 2` with `C_D = 2.656/sqrt(Re) +
  0.020 s/h + 0.018 C_L^2` recomputed per strip, <= 1e-12 relative. NC-L3: the annulus term left out must miss by >= 10x.
* **F1 fan laws.** C at q1/Q_D in {0.6, 1.0, 1.2} and 3000 rpm, rho 1.2, nu 1.5e-5, against 4500 rpm, rho 1.0, nu 2.25e-5 (equal Re)
  at `q2 = 1.5 q1`: `dp_t`, `dp_s`, `P` equal `fan_law(...)` of point 1 within 1e-12 relative, `eta_t` equal within 1e-12. Reason:
  dimensional similarity at equal Re. NC-F1: power scaled with `n^2` must miss by >= 10x. Reported: the deviation with nu held fixed
  (the Reynolds effect of the Blasius drag).
* **R1 root finding, analytic pair.** Fan `40 - 1e4 Q^2` against `5e3 Q^2 + 200 Q` on `[1e-3, 0.1]`, 100 intervals: `q*` within
  1e-12 relative of `(-200 + sqrt(200^2 + 4 x 1.5e4 x 40)) / (2 x 1.5e4)`, `crossings = 1`. NC-R1: the closed form without the linear
  term must miss by >= 10x. R1b: on C with `k = dp_s(Q_D) / Q_D^2`, the returned point has `|dp_s - k q^2| <= 1e-10 Pa` and
  `|q - Q_D| <= 1e-9 Q_D`.
* **R2 no intersection.** C with `k` 1000 times the R1b value: an error containing "no intersection" and the scan range. NC-R2: the
  R1b system gives Ok.
* **R3 stall.** C with 20 times the R1b `k` (crossing near 0.35 Q_D): an error containing "stall". R3b: the system through
  `(0.75 Q_D, dp_s(0.75 Q_D))`: an error containing "stalled" and the radius of at least one strip. NC-R3: the R1b system is Ok.
* **M1 monotone behaviour.** C designed and evaluated with `cl_max = 100` (no stall), at q/Q_D = 0.50, 0.55, ..., 1.30: `dp_t` and
  `dp_s` strictly decreasing, and the largest `eta_t` sits at an interior node and exceeds both end values by >= 1e-3. NC-M1: the same
  with `RomModel::lossless()` (`eta_t = 1` everywhere) must fail the efficiency criterion.
* **G1 Gutin check against its closed form.** C at Q_D, default model. Loads `(R, B F_x dr, B F_theta r dr)` of all strips placed at
  one radius `R = 0.8 r_tip` and summed by `tonal_harmonic` must equal `Rotor { r_tip: R, thrust: T, torque: M }.harmonic_amplitude`
  with `T = sum [rho U dc - D W / (s c_x) - rho dc^2 / 2] 2 pi r dr` (Euler work minus loss minus swirl, the static rise) and `M = rho
  sum r dc c_x 2 pi r dr` (Euler), at m = 1 and 2, 1 m, theta = 60 deg, within 1e-12 relative. Reason: the closed form is linear in the
  loads, and T, M are independent momentum forms of the strip forces. NC-G1: the torque passed with the wrong sign must miss by >= 10x.
* **D1 defaults.** `Rotor::harmonic_amplitude` equals the pre-change expression `n Omega / (2 pi c r) |J| |load|` bit for bit (assert_eq
  on f64) at 3 harmonics x 3 angles; `fwh_impermeable.rs` passes unchanged in the gate.
* **D2 evaluator** (`crates/rcfd-api/tests/fan_rom_design_w46b.rs`). A `Study` with `FanRomEvaluator` (one parameter: a stagger
  offset added to all C sections, [-3, 3] deg; conditions `system_k` = R1b value and 1.5 times it): a 5-point factorial DOE gives ok
  results with finite metrics at the centre; Nelder-Mead (budget 30, maximise `eta_s`) runs to completion; a condition with key `bogus`
  fails with a message naming `bogus`; the R3 system fails with "stall". Functional, no numeric limit.

### V1, FAN-01 comparison (REPORTED, not pass/fail; `v1_fan01_reported`, run once)

Fixed now, before any evaluation: sections and data from `validation-data/reference/w34d_fan01_*.csv` (all 17 rows, extrapolated rows
included), NACA 4-digit family, B = 9, r_hub 0.124, r_tip 0.2475, gap 2.5 mm, 1486 rpm, rho 1.2 and nu 1.5e-5 (ADR 0075 ASSUMED),
default model. Compared at the 20 measured flows: ROM `dp_s` against measured `dp_ts` (an installation value: bellmouth, hub,
struts, diffuser are not in either model), shaft power, `eta_s` against `Q dp_ts / P_meas`. Reported: the table, the rms and the signed
differences, the design point (1.4032 m^3/s), and the ROM stall onset (the largest measured flow with a stalled strip) against the
measured onset of about 1.1 m^3/s. FAN-01 is a 500 mm fan, not a computer fan: the distance is a sanity bound on the tier, not a
calibration. Reported too: the ROM chord-Re range (FAN-01 is near the 2e5 top of the section model). No parameter is changed after the
run.

Also reported (no limit): the BPF tone of C at Q_D, 1 m, 90 and 60 deg (amplitude and SPL), and the same loads at a compact `0.8 r_tip`.

### Stated accuracy of the tones (D1)

The closed form is exact for its assumptions and agrees with the FW-H solver within 4e-5 (ADR 0066). The strip loads are model
values. The steady-loading component of a low-Mach rotor is tiny: the smoke gives 1.25e-9 Pa at 1 m for C (tip Mach 0.053; the
Bessel factor is J_7 at an argument of about 0.37, which is about 1e-9). Measured computer-fan BPF tones are, by common experience, far louder; they come from unsteady loading (struts, inflow
distortion, rotor-stator interaction) that this model does not contain (UNVERIFIED as a quantitative statement; no measured small-fan
noise is in the repository, D5). The number is a lower bound of the tonal level, not a prediction of it.

### Status decision rule (registered now)

* A miss of E1, E2, Z1, Z2, L1-L3, F1, R1-R3, M1, G1: the test becomes an ignored `*_known_miss` with its limit; the code stays (it is a
  library with no default effect) and the miss is reported. A negative control that does not discriminate makes its check
  non-discriminating (reported).
* D1 failing is a defect of this change and is fixed before hand-back with the limit unchanged. D2 failing is a defect too.
* V1 has no limit; whatever it shows is reported with the distance.

## Smoke results before registration (rule d)

`smoke_w46b`, debug build, one run:
* Design consistency at Q_D: max relative `|dc - K/r|` 2.65e-15. Default model at Q_D: dp_t 32.77 Pa, dp_s 7.92 Pa, P 2.025 W,
  eta_t 0.728, eta_s 0.176, tip loss 0.054 W, drag loss 0.496 W; 3 strips below Re 2e4 (hub, 1.9e4). Strip C_L from 0.69 (hub) to
  0.30; alpha_e 6.3 to 2.7 deg.
* Curve 0.004-0.096 m^3/s: stalled strips appear below 0.036 m^3/s (0.80 Q_D); dp_s peaks at about 0.032 (34.8 Pa) and dips to
  15.9 Pa at 0.024 (a stall dip); dp_t is strictly decreasing from 0.036 upward; power turns negative above 0.060.
* No-stall variant (`cl_max = 100`): eta_t 0.7304 at 0.5 Q_D, peak 0.7714 at 0.80 Q_D, 0.0428 at 1.25 Q_D, -1.23 at 1.30 Q_D.
* Operating points: the R1b system returns q = 0.045 exactly with crossings 1; 20 k: "rising fan characteristic ... stall region" at
  0.0156; 1000 k: "no intersection"; through 0.75 Q_D: "stalled: 5 of 40 strips beyond cl_max (r = 0.0303, ...)".
* Ledger residual -4.4e-16 of P. Tone 1.25e-9 Pa (90 deg), 4.8e-10 Pa (60 deg); compact rotor at r_tip 3.2e-9 Pa.
* Strips 40 against 80 at 0.6 Q_D (partly stalled): dp_t 48.806 / 48.680 Pa, P 2.2929 / 2.2932 W.
* FAN-01 construction: 17 sections, 20 data rows, `FanRom::new` ok (not evaluated).
* No set-up was found defective.

## Amendment 1 (2026-10-26, written after the ADR commit 92048b5 and before any registered test was written or run)

* **Z1 set-up defect, found by reading the smoke, not by a run.** The registered Z1 flows include 0.7 Q_D. The smoke shows stalled
  strips below 0.80 Q_D (12 at 0.70 Q_D with the default model), and `RomModel::lossless()` keeps the stall model, whose post-stall
  branch carries the normal-force drag `C_N sin alpha_e`. That drag is a loss by construction, so `eta_t = 1` cannot hold at 0.7 Q_D:
  the set-up does not test what Z1 states. The registered Z1 stays as the ignored `z1_zero_loss_known_miss` (limits unchanged, flows
  {0.7, 1.0, 1.2}). Corrected **Z1'** (`z1b_zero_loss_unstalled`): flows {0.85, 1.0, 1.2} Q_D, the same limits, plus an assertion
  that no strip is stalled at those flows. NC-Z1 unchanged.
* **NC-M1 wording.** "The same with `RomModel::lossless()`" means the M1 set-up (`cl_max = 100`) with every loss switched off, i.e.
  `RomModel { cl_max: 100, ..RomModel::lossless() }`. With `cl_max = 1.1` the post-stall drag would make it a different control.

## Out of scope

Cascade correlations (Carter, Weinig), deviation, radial equilibrium, centrifugal fans (A3), broadband noise (D3), calibration against
measured small-fan data (G2), roadmap edits.

## Files

`crates/rcfd-post/src/fan_rom.rs` (new), `crates/rcfd-post/src/rotor.rs` (`harmonic_pressure`), `crates/rcfd-post/src/lib.rs`
(module line), `crates/rcfd-post/tests/fan_rom_w46b.rs` (new), `crates/rcfd-api/src/design/fan_rom_eval.rs` (new),
`crates/rcfd-api/src/design/mod.rs` (module line), `crates/rcfd-api/tests/fan_rom_design_w46b.rs` (new).

## References

First principles: thin-airfoil theory, cascade momentum, Blasius laminar friction, the Euler turbomachine equation, orifice-jet
dissipation, dimensional similarity. UNVERIFIED recollections: Howell's annulus and secondary drag (0.020, 0.018, via Dixon, "Fluid
Mechanics and Thermodynamics of Turbomachinery"), the 2-D slot contraction `pi / (pi + 2)`, the normal flat-plate drag 2.0, `cl_max`
1.1 (ADR 0078). Data: FAN-01 (ADR 0075, CC BY 4.0).

## Addendum: measured values (one registered run each; debug build, `target-w46b`, commit 618c8c6)

| Check | Measured | Limit | Negative control (must be >= 1e-11 unless stated) | Result |
|---|---|---|---|---|
| E1 Euler identity | 1.6e-16 | 1e-12 | NC-E1 5.7e-2 | pass |
| E2 free vortex (dc, torque, power, work) | 2.7e-15, 9.4e-16, 8.8e-16, 9.5e-16 | 1e-12 | NC-E2 2.8e-1 | pass |
| Z1 registered, flows {0.7, 1.0, 1.2} | 9.6e-2 (19 stalled strips at 0.7 Q_D) | 1e-12 | NC-Z1 5.6e-2 | known miss (set-up defect of amendment 1, confirmed) |
| Z1' flows {0.85, 1.0, 1.2}, no stalled strip | dp_t 7.2e-16, eta_t 2.2e-16, 45 Pa 4.7e-16 | 1e-12 | NC-Z1 5.6e-2 | pass |
| Z2 zero lift | dc 3.1e-61, dp_t 3.1e-122, P 0 | 1e-12 | NC-Z2 4.2e-2 | pass; default model dp_t -4.76 Pa, P 0.350 W (signs as required) |
| L1 ledger, 0.5 / 0.75 / 1.0 / 1.2 Q_D (39, 5, 0, 0 stalled strips) | 4.5e-16 | 1e-12 | (a) tip 3.7e-3, (b) drag 1.8e-1, (c) drag sign 4.3e-2, (d) swirl 5.5e-2 | pass |
| L2 tip loss (formula, doubling) | 0, 0 | 1e-12 | NC-L2 1.7e-1 | pass; tip loss 0.0542 W; eta_t 0.7550 / 0.7282 / 0.7014 for gap 0 / 1 / 2 mm |
| L3 drag terms | 0 | 1e-12 | NC-L3 6.3e-1 | pass; drag loss 0.496 W |
| F1 fan laws (equal Re) | 3.5e-15 | 1e-12 | NC-F1 3.3e-1 | pass |
| R1 analytic pair | 0 (q* 4.540166450604437e-2), crossings 1 | 1e-12 | NC-R1 1.4e-1 | pass |
| R1b ROM residual, q - Q_D | 8.9e-16 Pa, 0 | 1e-10 Pa, 1e-9 | | pass |
| R2 no intersection | located message with range | | NC-R2 Ok | pass |
| R3 / R3b stall | "rising fan characteristic ... stall region" at 0.0156; "stalled: 5 of 40 strips (r = 0.0303, ...)" at 0.03375 | | NC-R3 Ok | pass |
| M1 monotone, interior eta_t maximum | decreasing; margin 0.041 | margin >= 1e-3 | NC-M1 margin 1.2e-15, no interior maximum | pass |
| G1 Gutin closed form (m = 1, 2; 60 deg) | 1.3e-16 | 1e-12 | NC-G1 6.7e-2 | pass |
| D1 `harmonic_amplitude` bit-identical | assert_eq on bits, 9 cases | exact | | pass |
| D2 evaluator | DOE centre ok with 8 finite metrics (q = Q_D); Nelder-Mead 10 evaluations to the bound -3 deg (eta_s 0.211); `bogus` key and the stall condition fail with located messages | functional | | pass |

Reported (no limit):
* **F1, Reynolds effect.** With nu held fixed at 4500 rpm (Re 1.5 times higher), the largest deviation from the fan laws is 3.9 % (dp_t +3.9 % and
  P -1.9 % at 1.2 Q_D; eta_t 0.4275 against 0.4038). The fan laws are exact for this model only at equal Reynolds number.
* **Tones (D1).** Set-up C at Q_D, 1 m: BPF amplitude 1.25e-9 Pa at 90 deg (-87 dB re 20 uPa, rms), 4.8e-10 Pa at 60 deg (-96 dB); the
  same loads at a compact 0.8 r_tip give 1.06e-9 Pa. The steady-loading tone of a tip-Mach-0.05 rotor is inaudible; this is a lower
  bound, as stated above, not a prediction.
* **V1, FAN-01** (default model, all choices fixed before the run; installation effects in the measurement):

| Q m^3/s | ROM dp_s Pa | measured dp_ts Pa | ROM P W | measured P W | ROM stalled strips |
|---|---|---|---|---|---|
| 0.199 | 39.9 | 252.8 | 185.2 | 346.9 | 40 |
| 0.606 | 72.9 | 204.6 | 289.3 | 324.6 | 40 |
| 0.994 | 60.4 | 134.7 | 297.8 | 286.6 | 40 |
| 1.186 | 45.2 | 137.9 | 287.4 | 314.3 | 40 |
| 1.403 (design) | 45.7 | 126.5 | 308.6 | 333.3 | 33 |
| 1.593 | 94.4 | 103.2 | 392.8 | 326.1 | 11 |
| 1.700 | 93.5 | 85.9 | 414.3 | 313.5 | 0 |
| 1.804 | 67.9 | 63.9 | 395.5 | 292.9 | 0 |
| 1.907 | 40.9 | 42.8 | 372.6 | 272.9 | 0 |
| 2.079 | -6.9 | 6.8 | 324.9 | 235.3 | 0 |

  Over all 20 points: rms dp_s difference 104 Pa (the measured design-point value is 126 Pa), rms power difference 24 %. At the design
  point the ROM gives dp_s 45.7 Pa (-64 %), P 308.6 W (-7.4 %), eta_s 0.21 against 0.53. The ROM stalls at the design point: its stall
  onset (largest measured flow with a stalled strip) is 1.59 m^3/s against about 1.1 measured. Above 1.7 m^3/s, where no ROM strip is
  stalled, dp_s is within 2 to 14 Pa of the measurement but the power is 32 to 38 % high. Chord Re 5.0e4 to 1.45e5.
  Reading, not a finding: at the design point 33 of 40 strips are stalled, so most of the span is past the threshold, not only the
  hub (solidity 1.14). The threshold is `cl_max / 2 pi` = 10.0 deg of effective incidence, and the camber adds about 4.3 deg through
  `alpha_L0`. An isolated thin plate with this `cl_max` therefore stalls where the real blade row, a cascade with a measured stall
  onset near 1.1 m^3/s, still turns the flow. The ROM has no cascade correction (D-M3). The distance is the size of an uncorrected
  strip model on a fan it was not built for. It is not a calibration, and nothing was changed after the run. It does show that this
  tier, as registered, does not bound FAN-01 below 1.6 m^3/s, and that the stall model sets the shape of the curve below its onset.
* **Efficiency maximum and the design loop.** M1 passes with stall switched off (`cl_max = 100`). With the default model, the eta_t
  maximum inside the unstalled range [0.80, 1.30] Q_D lies at the stall boundary (0.7714 at 0.80 Q_D against 0.7682 at 0.85 Q_D): the
  model's efficiency keeps rising with loading until a strip stalls. D2 shows the consequence: Nelder-Mead on eta_s ran to the
  most-loaded bound. An efficiency objective on this tier drives a design to the stall edge, so a design loop needs a stall-margin
  constraint (not provided here).

Deviations from the registered plan: Z1 (amendment 1, written before any registered run; the known miss confirmed the defect: 19
stalled strips at 0.7 Q_D, error 9.6e-2). D2's Nelder-Mead ran to the parameter bound in 10 evaluations; the check is functional, and
this is reported, not judged. No limit was changed.

## Amendment 2 (2026-10-26, written after every registered run and after V1, before the two checks below were written or run)

A review found two deliverables without a check. Both are identity or plumbing checks; neither can move V1 or any limit above.

* **S1 static-pressure deductions** (`s1_static_pressure_deductions`). C, default model, at the L1 flows {0.5, 0.75, 1.0, 1.2} Q_D:
  `dp_t - dp_s` equals `rho c_x^2 / 2 + sum (rho dc^2 / 2) dQ / Q`, recomputed from the strip fields, within 1e-12 relative. Reason:
  D-M7 is the quantity the operating point and V1 use, and no registered check covered it (F1 tests only its scaling; R1b uses it on
  both sides). NCs, each must miss by >= 1e-11: the swirl term omitted; the axial term omitted.
* **P1 input conversion** (`p1_from_axial_fan`). An `AxialFan` built by `AxialFan::linear` maps to `RomFan` field by field
  (`tip_clearance` to `tip_gap`, stations to sections, exact equality); an invalid fan (`n_blades = 1`) returns the located error
  naming `n_blades`. Functional, no numeric limit.
