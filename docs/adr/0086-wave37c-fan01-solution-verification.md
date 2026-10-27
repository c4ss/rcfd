# ADR 0086: Wave 37C, the first solution-verification study of FAN-01 (three-mesh family, Richardson extrapolation, observed order, GCI) and the same study for the empty passage

Status: accepted (registration, then addendum). The registration was committed before any registered solver run of this wave. Limits and decision rules are fixed now and are not changed after a run; a miss of a
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
* Construction smoke (test `smoke_build_candidates`, both constructions, bladed and empty, release, scratch tree on 86145b9; no solve): every count set
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

Test `smoke_solve_family` (release, scratch tree on 86145b9, one run each). Every run is STABLE by the L1 criterion. Values at iteration 300 are NOT
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

## Addendum: measured values, verdicts, misses and deviations

Release builds of the scratch tree on 86145b9 plus the test file; the registration commit 5967578 preceded every registered run. Each registered
run was made once, two at a time on a shared machine (other wave-37 agents were running), by launching the built test executable directly
(`registered_run` with `W37C_RUN`). The limits are the registered ones, unchanged. No solver, mesh or default code was changed. Code state: the
runs were made on 86145b9. The main-repository HEAD at commit time also contains waves 37A and 37D. 37A adds the opt-in `solver.velocity_correction
= "face_flux"`, not used here. 37D added construction I and then removed it; `camber_seam_h` is kept with its B1 hash test. Neither changes a path
these runs use with their default keys. The non-ignored tests of this file run in the final gate on the HEAD.

### The six runs

| Run | Cells | Iterations | Minutes | Outcome | f_a | f_b | last-1000 change f_a / f_b | final res_u |
|---|---|---|---|---|---|---|---|---|
| E3 empty x0.75 | 5 040 | 8 000 | 7.1 | CONVERGED (cap rule) | L -0.009518 N m | swirl 6.26632 Pa | 8.2e-8 / 3.0e-6 | 3.1e-5 |
| E2 empty x0.75 sqrt 2 | 13 882 | 8 000 | 21.8 | CONVERGED (cap rule) | 0.059609 | 9.23038 | 1.9e-6 / 3.1e-6 | 3.1e-5 |
| E1 empty x1.5 | 38 688 | **7 918** | **60.0** | **NOT CONVERGED (time)** | 0.105977 | 5.48377 | 1.8e-8 / 7.4e-8 | 4.9e-5 |
| B3 bladed x0.75 | 4 920 | 8 000 | 7.1 | CONVERGED (cap rule) | torque 1.849759 N m | Delta p_ts 46.7776 Pa | 1.9e-5 / 0.171 | 1.03e-3 |
| B2 bladed x0.75 sqrt 2 | 13 627 | 8 000 | 25.6 | CONVERGED (cap rule) | 1.967401 | 64.2329 | 3.9e-7 / 6.4e-5 | 1.00e-3 |
| B1 bladed x1.5 | 38 184 | 8 000 | 58.8 | CONVERGED (cap rule) | 2.038428 | 73.9881 | 5.1e-6 / 9.7e-5 | 1.21e-3 |

No run tripped the guard or diverged; mass balance 1e-8 or better. No run reached the residual tolerance 1e-5: the bladed `res_u` stays at about
1e-3 from iteration 100 on and the empty-passage `res_u` at 3e-5 to 5e-5, while the functionals are settled (changes above). E1 was stopped by the
registered 60-minute wall clock 82 iterations before the cap, with its functionals constant to 2e-8 N m and 7e-8 Pa over the last 1000 iterations.

Reported, no rule (full annulus; bladed / empty, F1 / F2 / F3):
* Bladed: blade torque 2.0222 / 1.9519 / 1.8343, hub 0.0162 / 0.0155 / 0.0155, shroud on the fluid -0.0446 / -0.0298 / -0.0077 N m; Delta p_tt
  (mass-averaged absolute) 157.25 / 147.02 / 131.78 Pa; outlet swirl energy 23.48 / 22.66 / 26.82 Pa; outlet L 1.9931 / 1.9448 / 1.9742 N m; the
  angular-momentum excess (outlet L minus the wall torque on the fluid) **-0.0008 / 0.0072 / 0.1321 N m**, against 2.061 N m in the ADR 0078 E0
  base (H-slit, `upwind`): with the limited scheme on the C-grid the balance closes to 0.04 % of the torque on F1.
* Empty passage (exact: L 0, swirl 0, Delta p_tt <= 0): angular-momentum excess 0.0903 / 0.0371 / -0.0386 N m (hub and shroud torque 0.016 /
  0.023 / 0.029 N m); Delta p_tt **+4.53 / +9.21 / +4.85 Pa** (a duct whose walls are at rest creates total pressure; spurious, as X2d's 16.8 Pa).
  The outlet L of F1 (0.106 N m) is 5.2 % of the bladed F1 torque; X2a (H-slit, `upwind`) was 2.254 N m.

### Part G evaluation (registered rules; `evaluate`)

Ratios: bladed r21 1.4098, r32 1.4044; empty r21 1.4073, r32 1.4018.

| Functional | f1 / f2 / f3 | eps21 / eps32 | s, p | f_ext | Class (registered) | Band (Fs, order min(p, 1)) | Alternative (1.25, min(p, 2)) |
|---|---|---|---|---|---|---|---|
| torque [N m] | 2.038428 / 1.967401 / 1.849759 | -0.0710 / -0.1176 | +, **1.511** | **2.1428** | **VALID** | Fs 1.25: +-0.2167 (10.6 %) = [1.8218, 2.2551] | +-0.1305 |
| Delta p_ts [Pa] | 73.988 / 64.233 / 46.778 | -9.755 / -17.455 | +, **1.738** | **85.94** | **VALID** | Fs 1.25: +-29.76 (40 %) = [44.23, 103.74] | +-14.94 |
| empty L [N m] | 0.105977 / 0.059609 / -0.009518 | -0.0464 / -0.0691 | counterfactual: (+, 1.210) | counterfactual: (0.1965) | **NOT CONVERGED** (E1); counterfactual with E1's settled values: VALID | counterfactual: (+-0.1423 = [-0.0363, 0.2483]) | counterfactual: (+-0.1132) |
| empty swirl energy [Pa] | 5.484 / 9.230 / 6.266 | 3.747 / -2.964 | counterfactual: (**-**) | - | **NOT CONVERGED** (E1); counterfactual with E1's values: NOT MONOTONE | - | - |

Numbers in parentheses marked "counterfactual" use E1's values although E1 is NOT CONVERGED by the registered rule. They are not results;
by the rule a study that is not VALID gives no band and no order.

Iteration-error check: every last-1000 change is below 0.1 min(|eps21|, |eps32|) (largest relative: B3's Delta p_ts 0.171 Pa against the limit
0.976 Pa). The check compares two end points only, so a bounded oscillation could be understated. Checked from the logs (values printed every 500
iterations): over the last 2000 iterations the largest spread (max - min) is B3's, 7.9e-4 N m in torque and 0.25 Pa in Delta p_ts (46.53 to 46.78
Pa over iterations 1000-8000). That is below the limits 7.1e-3 N m and 0.976 Pa. All other runs spread by 3e-4 Pa / 8e-6 N m or less.

Each member carries a slightly different flow: the inlet speed is fixed, the annulus is faceted, so Q is 1.401232 / 1.398776 / 1.395055 m^3/s
(F1 / F2 / F3). This is geometric discretisation error and part of the band. With the slope of the measured characteristic it is worth about
1 Pa of the 27 Pa Delta p_ts change and about 0.004 N m of the torque change (an estimate, not measured).

### Checks and readings

| id | Limit | Measured | Result |
|---|---|---|---|
| S1 family builds, `check_mesh`, empty mesh without blade faces and with more cells, r >= 1.3 | all | as registered; r 1.4044-1.4098 | pass (not ignored) |
| G0 order solver on exact power laws | 1e-8 / 1e-10 | p and f_ext recovered (1.6, 2.0, 0.9) | pass (not ignored) |
| E-L, E-S (R-E): empty studies VALID and the band contains 0 | VALID and contains 0 | NOT CONVERGED (E1 at the wall clock); counterfactual with E1's settled values (not a result): swirl energy NOT MONOTONE, L moving away from 0 with refinement | **MISS** (`e_l_e_s_empty_passage_band_contains_zero_known_miss`) |
| B-M torque study | VALID | VALID, p 1.51 | met |
| B-P Delta p_ts study | VALID | VALID, p 1.74 | met (`b_m_b_p_bladed_studies_are_valid`, heavy, ignored) |

**R-E: not met**, so by the registered rule the bladed bands below are "not shown credible on this family". The verdict does not hang on E1's
82 missing iterations: with E1's settled values the swirl-energy study is still not monotone (eps32 = f3 - f2 = -2.96 Pa, so a monotone
sequence would need f1 above f2 = 9.23 Pa; F1 is settled at 5.48 Pa, constant to 7e-8 Pa over its last 1000 iterations). As a counterfactual
observation only, the L sequence moves away from the exact zero with refinement. On the empty passage the error of the limited relative-frame scheme on these C-grid meshes does NOT
decrease with this refinement: L grows (-0.010, 0.060, 0.106 N m) and the duct still creates 4.5-9.2 Pa of total pressure. The likely reason is
outside this study's reach. The family does not refine the wall-normal stacks, the C-grid smoother produces cells of different shape at each count,
or the limiter clips a different set of cells. None of these was tested.

**R-B (contain in principle), bladed, with the "not shown credible" label:**
* Torque: band [1.822, 2.255] N m; measured 2.1418 is **inside** (|meas - f1| = 0.103 N m = 4.8 %); the alternative band (+-0.130) also contains it.
  The Richardson value 2.1428 N m lies 0.05 % from the measured shaft torque. That coincidence is not agreement and is not claimed. The band is
  +-10.6 %. The wall-normal error is excluded. The empty-passage check did not show the band to be credible. The measured power may contain bearing
  loss, and the density is assumed. What can be said: on this family, the numerical error of the torque could in principle account for the gap to
  the measurement. ADR 0075's first-order run was 38 % short.
* Delta p_ts: band [44.2, 103.7] Pa; measured 126.46 Pa is **outside** (|meas - f1| = 52.5 Pa; f_ext 85.9 Pa, 40.5 Pa short); the alternative band
  is narrower and also excludes it. Within this model, numerical error at this resolution does not explain the Delta p_ts gap. That is consistent
  with ADR 0078 question 2: the measured value is an installation value, and the rotor-only Delta p_ts is not determined by the rotor to the
  registered tolerance.

### Findings (robustness; reported, nothing fixed)

* The limited `linear_upwind` with SST is stable on all three C-grid meshes, bladed and empty (first evidence on the C-grid; ADR 0081 L1 was on the
  H-slit).
* The steady SIMPLEC iteration does not reach `res_u` 1e-5 on any of the six runs. It stalls at about 1e-3 (bladed) and 3e-5 to 5e-5 (empty) while
  the functionals are constant to 1e-5 relative. Every run had to use the cap rule, so a tolerance-based stop in a design loop would never fire on
  these meshes. Not diagnosed.
* At 38 k cells, 8000 iterations take 59-60 minutes on the shared machine (0.30 s per iteration alone, about 0.45 with two runs). The 60-minute
  guard of rule (e) is binding at this resolution: E1 lost by 82 iterations, B1 finished at 58.8 minutes. A finer member (x2 folds anyway) is not
  runnable within the rule.
* The empty passage's spurious outlet angular momentum and total-pressure rise do not fall with refinement on this family (above). The bladed
  angular-momentum balance does close.

### Deviations (candid)

* The test file hard-coded the family before the construction smoke. The family was registered only after the smoke, with the selection rule
  written in the ADR; the smoke built all candidate sets, so no set was dropped.
* The queue script was started once with the order B3, B1, B2. It was stopped before it launched any run and restarted in the registered order
  B3, B2, B1.
* E1 was stopped by the registered wall clock while another of my runs (B3, then B2, then B1) shared the machine with it. Running two at a time
  was allowed by the registration. No re-run was made, because the registered verdict would not change (above).
* The `assess` / `study` refactor of the evaluation and the two registered check tests were written after the runs. The evaluation numbers above
  come from the `evaluate` test of the build that made the runs. The refactored `evaluate` reproduces them to every printed digit. The two study
  tests themselves were not re-run on the six runs (about 3 h).
* The registration was committed from the scratch copy of the file (a temporary index in the main repository); the test file is committed after
  the runs.

### Unverified references

Celik et al. (2008, J. Fluids Eng. 130, 078001) for the unequal-ratio order formula, Roache (1998) for the GCI and its factors of safety: both
recalled, not re-read. Air density 1.2 kg/m^3 assumed (ADR 0075).

### Final gate

Fresh `git archive` of 53688b3, which also contains the other wave-37 agents' commits up to it: `cargo fmt --all -- --check` clean, `cargo
clippy --workspace --all-targets -- -D warnings` clean, `cargo test --workspace --release --no-fail-fast` **1881 passed, 0 failed, 531 ignored**
(no LNK1201). The test run took about 2 h 05 min, over the 60-minute run guideline, as in wave 36. This file contributes 2 passed and 6 ignored
tests. Later commits by waves 37A, 37B and 37D (docs only, up to 67860c4) and this paragraph are not re-gated; this paragraph is a docs-only
commit.
