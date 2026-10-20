# ADR 0086: Wave 37C, the first solution-verification study of FAN-01 (three-mesh family, Richardson extrapolation, observed order, GCI) and the same study for the empty passage

Status: registered before any registered solver run of this wave. Limits and decision rules are fixed now and are not changed after a run; a miss of a
check becomes an ignored `*_known_miss` test with the limit intact, a defective set-up gets a NEW corrected test and the old one stays ignored.
Baseline hashes are never re-recorded; no solver, mesh or default code is changed by this wave (tests and documentation only). Clean-room: first
principles and published literature only; references recalled from memory are marked UNVERIFIED. Parents: ADR 0075 (FAN-01 data, set-up, measured
design point), ADR 0078 (deficit diagnosis, X2 empty-passage check), ADR 0079 / 0083 (C-grid `camber_seam` / `camber_seam_h`), ADR 0081 (the
Barth-Jespersen-limited `linear_upwind`), ADR 0082 (location and first-order mesh dependence of the spurious angular momentum).

## Question

How large is the numerical (discretisation) error of the FAN-01 torque and Delta p_ts at the design point on the C-grid passage with the now-stable
limited `linear_upwind`, measured by a registered three-mesh study, and could that error band, in principle, contain the measured values (Delta
p_ts 126.46 Pa; torque 2.142 N m = 333.30 W / 155.614 rad/s, ADR 0075 amendment 1)? The same study on the empty passage (X2a type, exact absolute
swirl zero) tests whether the error band is credible where the exact answer is known. Agreement with the experiment is NOT a question of this ADR and
is not claimed whatever the numbers are.

## What was known or run before this registration (disclosed, rule (d))

* All earlier FAN-01 numbers: ADR 0075 attempt 3 (H-slit, `upwind`, SST) torque 1.320 N m, Delta p_ts -3.8 Pa; ADR 0078 KN (H-slit refined,
  minmod) 2.016 N m, 76.2 Pa; X2a (H-slit, `upwind`) outlet L 2.254 N m; ADR 0082: the spurious angular momentum of `upwind` is first order on a
  converged H-slit family (0.88). No FAN-01 run with SST and the limited `linear_upwind` on a C-grid has been made before this ADR, and no FAN-01
  solve of any kind on the counts of the family below except the ADR 0083 part A laminar `upwind` smokes on x0.75 and x1.5 (both converging,
  L / c10 0.027 and 0.26) and the x1 (FAN-01-C) oscillation (8.36).
* ADR 0081 L1a / L1b (limited `linear_upwind` stable on FAN-01) were measured on the H-slit attempt-2 mesh, not on the C-grid.
* Code read (read-only): `rcfd-turb/src/sst.rs` convects k and omega with first-order `Upwind` regardless of `solver.convection`, so the formal order of
  the coupled discretisation is 1 (momentum convection 2 with the limiter, which clips to first order where it acts).
* Construction smoke (test `smoke_build_candidates`, both constructions, bladed and empty, release, scratch tree on 6f97dcb; no solve): every count set
  builds and is accepted by `check_mesh`:

| Counts (n_up, n_chord, n_down, n_side, n_nose, n_span, n_gap) | Label | Cells bladed / empty | `camber_seam` non-orth / skewness / aspect | `camber_seam_h` |
|---|---|---|---|---|
| (6, 12, 6, 8, 3, 10, 5) | x0.75 | 4 920 / 5 040 | 78.39 / 1.826 / 168.9 | 78.39 / 1.578 / 168.9 |
| (8, 17, 8, 11, 4, 15, 7) | x0.75 sqrt 2 | 13 627 / 13 882 | 79.54 / 4.617 / 204.0 | 79.54 / 4.618 / 204.0 |
| (8, 16, 8, 10, 4, 14, 7) | x1 (FAN-01-C) | 11 536 / 11 760 | 83.26 / 3.978 / 227.2 | 83.26 / 3.978 / 227.2 |
| (10, 20, 10, 12, 5, 18, 9) | x1.25 | 22 320 / 22 680 | 79.60 / 9.662 / 237.3 | 79.62 / 9.666 / 237.3 |
| (12, 24, 12, 15, 6, 21, 10) | x1.5 | 38 184 / 38 688 | 83.36 / 3.762 / 252.4 | 83.37 / 3.765 / 252.4 |

  The empty build (`include_blade = false`) has no `blade` faces and more cells (the footprint filled), checked by S1 below.
* Solver smoke of the registered runs (300 iterations, ADR 0081 L1 criterion): see the table under "Smoke of the registered runs" (filled in before
  this ADR was committed; it is part of what was known).

## Registered

### Family (part F)

Rule (fixed before any solve): the coarsest member is x0.75, the coarsest in-range set (`n_side` 8 is the lower end of the `camber_seam` range
`max(8, n_bl + 6)`); the finest is x1.5 (x2 folds, ADR 0083; x1.5 is the finest built and fits a 60-minute run); the middle member is every FAN-01-C
count times 0.75 sqrt 2 rounded to the nearest integer, so that the nominal ratio is sqrt 2 per level in every count direction. FAN-01-C itself
(x1) is not a member because its nominal ratios to x0.75 and x1.5 are 1.33 and 1.5 (not equal), not because of its laminar oscillation; that
oscillation is a known risk of the neighbouring middle member (stated). Construction: `camber_seam_h` (H of ADR 0083, the construction kept last; it
differs from `camber_seam` only in the blade wall stacks).

| Member | Counts | Cells bladed / empty |
|---|---|---|
| F1 (fine) | (12, 24, 12, 15, 6, 21, 10) | 38 184 / 38 688 |
| F2 | (8, 17, 8, 11, 4, 15, 7) | 13 627 / 13 882 |
| F3 (coarse) | (6, 12, 6, 8, 3, 10, 5) | 4 920 / 5 040 |

Refinement ratio: the registered ratio is the representative cell size `h = (V / N)^(1/3)` (V the passage volume, identical for the members of one
geometry up to the blade facets), i.e. `r21 = (N1 / N2)^(1/3)`, `r32 = (N2 / N3)^(1/3)`: bladed 1.4098 and 1.4044, empty 1.4073 and 1.4017 (nominal
sqrt 2 = 1.4142). Per-direction count ratios after rounding (F3 -> F2 -> F1): n_up and n_down 1.33, 1.5; n_chord 1.42, 1.41; n_side 1.38, 1.36;
n_nose 1.33, 1.5; n_span 1.5, 1.4; n_gap 1.4, 1.43. Not refined (stated limits of the family): the first cell (y+ = 30 rule, 5.654e-4 m), `n_bl` 2,
growth 1.2, `n_core` 1, `front_gap` 0.3. The study therefore measures the error that depends on the in-plane, spanwise, axial and tip-gap resolution
at a fixed wall model and wall-normal first cell; the wall-normal near-wall error is NOT in the band. The C-grid smoother does not produce
geometrically similar meshes (maximum non-orthogonality 78.4 / 79.5 / 83.4 deg, skewness 1.58 / 4.62 / 3.77): a known risk of non-asymptotic
behaviour, not a discovery afterwards. The axial blocks where ADR 0082 located the spurious source have count ratios 1.33 and 1.5.

### Set-up (part S)

The ADR 0075 design point exactly (Q = 1.4032 m^3/s, inlet `(Omega y, -Omega x, 9.478)` with the SST inlet values, outlet fixed static pressure 0 and
zero-gradient velocity, `rotating_frame` Omega = 155.614 rad/s with centrifugal terms, nu 1.5e-5, rho 1.2 ASSUMED, `k_omega_sst`), with three
differences, all fixed now: the C-grid family above; `solver.convection = "linear_upwind"` with `linear_upwind_limiter = "barth_jespersen"`; the
casing as `moving_wall` at `-Omega` with `wall_velocity_projection = "tangential"` (the ADR 0072 / ADR 0078 E1 treatment; on the C-grid the facet
leak of the expression form is 1 %, ADR 0072). Hub and blade at rest in the rotating frame. Empty passage (part E): the same with
`include_blade = false` and the hub also `moving_wall` at `-Omega` with the projection (ADR 0078 X2a). SIMPLEC, default relaxation, tolerances
`tolerance_u = tolerance_p = 1e-5`, cap 8000 iterations, test file `crates/rcfd-api/tests/fan01_gci_w37c.rs`.

Stop and convergence rules (ADR 0075 / 0078, with a stated floor for the empty passage): a run stops on a steady-guard trip, on a non-finite
residual or continuity above 1e6 (DIVERGED), or at 60 minutes wall clock (NOT CONVERGED (time)). It is CONVERGED if the residual tolerances are
reached, or at the 8000 cap if each functional changed by less than 0.5 % of its value over the last 1000 iterations (empty passage: by at most
`max(0.5 % |value|, 1e-4 N m)` for L and `max(0.5 % value, 0.01 Pa)` for the swirl energy) and `|m_in + m_out| <= 1e-3 |m_in|`. Anything else is
NOT CONVERGED.

Functionals (full annulus; ADR 0075 / 0078 definitions): bladed: torque `M = -9 (moment of the fluid on blade + hub)`, Delta p_ts = rho (area
average of p on the outlet - area average of p on the inlet - U_in^2 / 2). Empty: outlet absolute angular-momentum flux `L` (exact 0) and outlet
`rho <c_theta^2/2>_m` (exact 0). Reported, no rule: Delta p_tt (mass-averaged absolute total pressure), outlet L and swirl energy and the
angular-momentum excess of the bladed runs, blade / hub / shroud moments.

### Richardson extrapolation and GCI (part G), fixed now

Index 1 fine, 3 coarse; `eps21 = f2 - f1`, `eps32 = f3 - f2`, `s = sign(eps32 / eps21)`. Observed order by the fixed-point form for unequal ratios
(Celik et al. 2008, J. Fluids Eng. 130, 078001, recalled, UNVERIFIED): `p = |ln|eps32 / eps21| + q(p)| / ln r21`, `q(p) = ln((r21^p - s) / (r32^p -
s))`; extrapolated `f_ext = (r21^p f1 - f2) / (r21^p - 1)`. Error band (Roache 1998, recalled, UNVERIFIED): `U = Fs |eps21| / (r21^p_b - 1)` with
**Fs = 1.25** if p lies in [0.9, 2.2] (within 10 % of the formal-order range [1, 2]) and **Fs = 3** otherwise, and **p_b = min(p, 1)** (the formal
order of the coupled scheme, part "what was known"). Reported beside it, no rule: the alternative band with Fs 1.25 and `min(p, 2)`. The order solver
is checked on exact power laws with unequal ratios (test `gci_recovers_a_known_power_law`, p 1.6 / 2.0 / 0.9 to 1e-8).

Validity classes of a functional's study (exactly one; anything not VALID gives no band and no order is quoted as a result):
* NOT CONVERGED: any of its three runs is not CONVERGED by the rule above.
* ITERATION ERROR: the largest last-1000-iteration change of the functional over the three runs exceeds `0.1 min(|eps21|, |eps32|)` (for a run that
  converged by tolerance in fewer than 2000 iterations: the change over its last half).
* NOT MONOTONE: `s <= 0` (oscillatory) or the fixed point does not converge.
* NOT ASYMPTOTIC: p outside [0.5, 4].
* VALID otherwise.

### Readings (part R), fixed now

* **R-E credibility** (the empty passage, exact value 0): for each of L and the swirl energy, if VALID, the band `[f1 - U, f1 + U]` contains 0 (pass)
  or not (MISS, `*_known_miss`). If either empty-passage functional is not VALID or misses, the bladed bands are reported but labelled "not shown
  credible on this family".
* **R-B contain in principle**: for each bladed functional that is VALID, "the numerical error band could in principle contain the experiment" iff
  `|measured - f1| <= U` (torque 2.1418 N m, Delta p_ts 126.46 Pa). The band is the discretisation error of the in-plane / spanwise / axial
  resolution only (part F). This is not agreement: Delta p_ts is an installation value (ADR 0078 question 2: the rotor-only Delta p_ts is not
  determined to the registered tolerance by the rotor); the measured shaft power may include bearing loss; the density is assumed. A band that does
  not contain the measurement says that, within this model, numerical error at this resolution does not explain the gap.
* Reported beside the bands: the empty-passage outlet L on each member against the bladed torque (same units; the spurious source of ADR 0078 / 0082
  measured with the limited scheme on the C-grid).

### Checks with limits

| id | check | limit |
|---|---|---|
| S1 | the family builds with `camber_seam_h`, bladed and empty; `check_mesh` ok; the empty mesh has no `blade` faces and more cells; r21, r32 >= 1.3 | all (not ignored, 9 s) |
| G0 | order solver on exact power laws | p to 1e-8, f_ext to 1e-10 |
| E-L, E-S | R-E for L and the swirl energy | VALID and the band contains 0 |
| B-M, B-P | the bladed studies of torque and Delta p_ts | VALID (a study that is not VALID is reported as such, ignored `*_known_miss`) |

R-B is a reading, not a limit. The run order: empty passage F3, F2, F1 first (cheaper, and their exact value decides R-E), then bladed F3, F2,
F1; at most two runs at a time; each run once.

### Not claimed

Agreement with the FAN-01 measurement; any accuracy of the wall-normal resolution or of the turbulence model; fan performance.

## Smoke of the registered runs (rule (d); 300 iterations each, ADR 0081 L1 criterion; known when this ADR was committed)

Test `smoke_solve_family` (release, scratch tree on 6f97dcb, one run each). Every run is STABLE by the L1 criterion. Values at iteration 300 are NOT
converged values and are disclosed because they were seen before this registration:

| Run | Cells | s / it | Max speed (limit 3 x 40.0) | Continuity at 10 / max 200-300 | res_u at 300 | f_a at 300 | f_b at 300 |
|---|---|---|---|---|---|---|---|
| bladed F1 | 38 184 | 0.301 | 54.8 | 4.10e-4 / 3.69e-5 | 1.21e-3 | torque 2.0390 N m | Delta p_ts 74.04 Pa |
| bladed F2 | 13 627 | 0.107 | 49.3 | 7.44e-4 / 2.86e-5 | 1.01e-3 | 1.9674 | 64.23 |
| bladed F3 | 4 920 | 0.048 | 50.1 | 8.83e-4 / 7.83e-5 | 1.27e-3 | 1.8497 | 46.58 |
| empty F1 | 38 688 | 0.280 | 40.2 | 5.20e-4 / 1.65e-6 | 5.0e-5 | L 0.1060 N m | swirl energy 5.484 Pa |
| empty F2 | 13 882 | 0.108 | 42.2 | 3.17e-4 / 1.03e-6 | 5.3e-6 | 0.0596 | 9.230 |
| empty F3 | 5 040 | 0.044 | 48.1 | 8.59e-4 / 1.06e-6 | 3.0e-5 | -0.0095 | 6.266 |

What this tells before the registered runs (stated now so that nothing is read into it afterwards): the bladed `res_u` stalls near 1e-3 by
iteration 100, so the bladed runs will probably end at the 8000 cap under the cap rule (F1 needs about 40 min of the 60); the empty-passage values
were almost settled at 300 iterations, and on them the outlet L GROWS with refinement (-0.0095, 0.0596, 0.1060 N m) and the swirl energy is not
monotone (6.27, 9.23, 5.48 Pa): if that persists, part E is NOT MONOTONE or NOT ASYMPTOTIC by the rules above, R-E is not met, and the bladed
bands are "not shown credible". The rules were written before these runs and are not changed because of them.
