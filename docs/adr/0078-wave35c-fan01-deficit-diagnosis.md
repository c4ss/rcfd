# ADR 0078: Wave 35C, diagnosis of the FAN-01 deficit (controlled experiments, energy balance, blade-element estimate)

Status: accepted. Everything under "Registered" was written and committed before any code of this wave was written and before any solver run of
this wave. Limits and decision rules are not changed after a run; a miss of a check becomes an ignored `*_known_miss` test with the limit intact,
a defective set-up gets a NEW corrected test and the old one stays ignored. Baseline hashes are never re-recorded. Clean-room: first principles and
published literature only; anything recalled from memory is marked UNVERIFIED. Parent: ADR 0075 (FAN-01, F-126 attempt 3 MISSED), ADR 0072 (the
projected casing wall), ADR 0022 (`radial_equilibrium_outlet`), `docs/milestone-fan.md`.

## Context

ADR 0075 attempt 3 (H-slit sector, 13 332 cells, `upwind`, k-omega SST, `rotating_frame`, uniform fixed axial inflow 0.1 m ahead of the hub leading
edge, fixed static pressure 0 at the outlet 0.4 m from the inlet, casing as `velocity = "fixed"` with face-centre expressions) converged by the
monitor criterion at the design point Q = 1.4032 m^3/s with Delta p_ts = -3.8 Pa (measured 126.46), Delta p_tt = 112.5 Pa, torque 1.320 N m
(measured 2.142, -38 %); off-design torque -46 %. About 52 Pa (design) of the outlet energy was not explained by the uniform-outflow estimate.

Already known when this ADR was written (stated so that nothing below is chosen with hidden knowledge):
* The shaft work per unit mass of the measured fan at the design point is `P/(rho Q)` = 333.30/(1.2 x 1.4032) = 197.9 J/kg; the CFD Euler work from
  the reported torque is 122.0 J/kg. The measured shaft power may contain bearing loss, so 197.9 is an upper bound of the work done on the air.
* A pilot of the inviscid blade-element estimate of section E below (written in Python in the scratch area, same equations) gave 242 J/kg at the
  design point and 173 J/kg at Q = 1.8039; its section lift coefficients reach 1.5 near the hub. The method and its variants are fixed below
  regardless of these numbers.
* Code read (read-only) before writing: `wall_motion::build` gives a `moving_wall` the same `FixedValue(Profile::PerFace)` boundary object as a
  `fixed` wall, the SST wall law takes the wall velocity from the velocity boundary values (ADR 0075 S1), and `turb.rs` refuses every `moving_wall`
  with a turbulence wall treatment (`boundaries.<p>.velocity`). `bc_cat.rs` evaluates the `radial_equilibrium_outlet` balance `dp/dr = w^2/r` with
  `w` the tangential component of the SOLVED face velocity, which in a `rotating_frame` case is the relative velocity.
* The ADR 0072 measured facet leak of the H-slit shroud: largest `|u_w . n|/|u_w|` 1.7e-3 (the 1 % figure is the C-grid).
* Iteration cost of attempt 3: about 16 min for 8000 iterations of the 13 332-cell mesh (single thread).

## Registered

### Common set-up, quantities and stop rules

Base set-up B0 = the ADR 0075 attempt-3 design-point case exactly (`fan01_validation.rs`, `fan01_case(1.4032)`), plus report-only additions that do
not change the solution (a `moment` report on `shroud`). All experiments are at the design point only. Each run: at most 8000 iterations, the ADR 0075
convergence criterion (tolerances 1e-5, or at the cap: torque and Delta p_ts change below 0.5 % over the last 1000 iterations and mass balance 1e-3),
the ADR 0075 amendment-3 divergence stop (continuity non-finite or above 1e6), and a wall-clock stop at 60 min (outcome NOT CONVERGED (time)). A run
that does not converge is reported with its last values and is not used for attribution. One run per registered set-up; nothing is re-run to pick a
result.

Quantities computed from every converged run (absolute frame `c = w + Omega x r`; exact boundary face values from the velocity and pressure
boundary values and the face flux `phi`; `rho` = 1.2):
* `M` = minus the moment about +z of the fluid on `blade` + `hub`, times 9 (as ADR 0075), and `M_s` the same for `shroud` (report only);
  `W_M = M Omega / (rho Q)`.
* On a surface S (the `inlet` patch, the `outlet` patch, and two internal cuts): mass flow, area- and mass-averaged static pressure, mass-averaged
  absolute total pressure `p + rho |c|^2/2`, mass averages of `c_z^2/2`, `c_theta^2/2`, `c_r^2/2`, the bulk axial speed `c_bar = Q/A_S`, the
  non-uniformity term `(<c_z^2>_m - c_bar^2)/2`, the angular-momentum flux `L_S = sum rho r c_theta phi`, the backflow fraction by area and by mass
  (faces with `phi < 0`). Cuts: the internal faces whose owner and neighbour centres lie on different sides of `z = z0`, oriented in +z;
  U at `z0 = z_le_root - 0.04` m (40 mm ahead of the hub leading edge; 0.06 m in E0), D at `z0 = z_le_root + 0.09` m (18 mm behind the hub
  trailing edge at `z_le_root + 0.072`; 0.19 m in E0). Internal face values
  are the arithmetic mean of the two cells. Self-check of a cut: `|sum phi - Q| <= 1e-3 Q` (mass balance level of the convergence criterion); a cut
  that fails it is not used.
* `W_L = Omega (L_outlet - L_inlet)/(rho Q)` (Euler work from the angular-momentum flux; the stationary shroud's torque makes `W_L` slightly smaller
  than the work of the rotor). Closure reported, no limit: `W_M - W_L` against `-M_s Omega / (rho Q)` and the viscous under-report of the `moment`
  report (molecular viscosity, ADR 0075).
* Loss `= rho W_L - Delta p_tt` (mass-averaged total pressures, outlet minus inlet); split into inlet-to-U, U-to-D (rotor) and D-to-outlet
  `Delta p_t` with the work entering each segment `Omega Delta L / Q`.
* `Delta p_tt - Delta p_ts` decomposed into (mass-averaged minus area-averaged outlet p) + outlet `rho <c_z^2/2>_m` + `rho <c_theta^2/2>_m` +
  `rho <c_r^2/2>_m` minus the inlet terms. This must close to round-off (it is an identity of the definitions; check `<= 1e-9` of `Delta p_tt`).
* Spanwise: at cut D, 8 equal radial bins from the hub to the shroud: mass-averaged `r c_theta`, `c_z`, and the ideal work `Omega r c_theta`,
  compared bin by bin with the blade-element estimate at the bin centre radius.
* Reported Delta p_ts and Delta p_tt also exactly as ADR 0075 computed them (area-average monitors, owner-cell outlet values), so the numbers are
  comparable with attempt 3.

Reproduction rule (run E0 first): E0 = B0 on the HEAD of this wave. It must reproduce attempt 3 within 0.2 Pa in Delta p_ts and 0.1 % in torque
(ADR 0075 harness definitions). If it does not, all other arms are stopped until the difference is explained (an amendment states it before any
further run).

Attribution rules (fixed now). For a quantity X (Delta p_ts, Delta p_tt, M, W_L, loss, outlet kinetic terms) and an arm i: `delta_i = X_i - X_E0`;
`nu_i` = the absolute change of X over the last 1000 iterations of run i (for quantities without a monitor history: the larger of the torque and
Delta p_ts relative changes of that run times |X|). The effect is RESOLVED if `|delta_i| > 5 max(nu_i, nu_E0)`; it is SIGNIFICANT FOR THE GAP if
also `|delta_i| >= 0.05 G_X`, with `G_X = X_measured - X_E0` (Delta p_ts: 126.46 - E0; torque: 2.1422 - E0). Its share of the gap is
`delta_i / G_X`. Additivity of the combined run K: `|delta_K - sum delta_i| <= 0.2 max(|delta_K|, |sum delta_i|) + 5 nu_K`, otherwise the effects
interact and only the combined effect is quoted.

No arm is an F-126 attempt. If an arm happens to meet the ADR 0075 limits, that is reported and not claimed: F-126 stays MISSED (rule: no picking of
a favourable set-up). No fan performance claim is made.

### Arm A: the casing with the 34A treatment (task a)

E1 = E0 with the shroud as `velocity = "moving_wall"`, `wall_rotation_rate = -Omega`, `wall_velocity_projection = "tangential"`, `turbulence = "wall"`.
Needed code (opt-in): `turb.rs` refuses every `moving_wall` with a turbulence wall treatment. Change S2: for `k_omega_sst` only, a `moving_wall`
(not `shear`, `partial_slip`, `marangoni`) is accepted, because it builds the same boundary object as a `fixed` moving wall, which ADR 0075 S1 already
accepts. Checks: S2a an API SST channel (the ADR 0075 G1 channel set-up at API level, y+ 30 mesh, walls moving at `-10` m/s along x) with the walls
given as `fixed` + `velocity_value` and as `moving_wall` + `velocity_value`: after 200 iterations all cell fields (u, p, k, omega) bit-identical.
S2b the same case with `k_epsilon` is still refused at `boundaries.<wall>.velocity` or `.turbulence`. If S2a is not bit-identical, S2 is removed and E1
is reported NOT RUNNABLE. Prediction registered now (from the 1.7e-3 leak): E1 equals E0 within the resolution rule (no resolved effect on M); a
resolved effect is reported as a falsified prediction.

### Arm B: energy balance (task b)

The quantities above, on E0 and on every other arm. No new run.

### Arm C: the outlet (task c)

* C1 location: `axial_extent` 0.4 -> 0.8 (outlet 0.63 m instead of 0.23 m behind the hub trailing edge), `n_down` the smallest count for which the
  largest axial cell length downstream of the trailing edge at the hub layer is at most that of E0 (spacing near the trailing edge unchanged, the
  same boundary-layer cells).
* C2 backflow stabilisation: E0 with the outlet as `bc_kind = "pressure_outlet"`, `bc_pressure = 0`, `prevent_reverse_flow = true`.
* C3 type, radial equilibrium: E0 with the outlet as `bc_kind = "radial_equilibrium_outlet"`, `radial_reference = "area_average"`, `bc_pressure = 0`,
  ONLY after the defect check below has run; with the current code in a rotating frame the balance uses the relative swirl (hypothesis RE-H).
* Hypothesis RE-H and its falsifier X1 (run before any fix code). The ADR 0022 E2 case (swirling annulus, forced vortex `c_theta = 2 r`, plug axial
  flow 1, walls moving with the fluid, exact `p = 1.25 + 2 (r^2 - 0.625)`) solved in a frame rotating at `Omega_f` about z (`rotating_frame`,
  centrifugal on; inlet `swirl_rate = 2 - Omega_f`, walls `wall_rotation_rate = 2 - Omega_f`), `Omega_f` = 2 and 1. The exact static pressure is frame
  independent. E2's registered limits apply (outlet pressure error and mid-length pressure error `<= 3 %` of the range 1.5, outlet absolute swirl
  `c_theta` error `<= 3 %` of 2). Prediction with the current code: outlet pressure error about 50 % (`Omega_f = 2`) and 37.5 % (`Omega_f = 1`) of the
  range. If X1 passes with the current code, RE-H is falsified and no fix is made. Fix F-RE (opt-in): key `boundaries.<p>.radial_frame_rate` [rad/s]
  (default 0, `radial_equilibrium_outlet` only): the balance uses `w_theta + radial_frame_rate r`. Acceptance: X1 with `radial_frame_rate = Omega_f`
  meets the E2 limits at both `Omega_f`; the existing E2 test and `baseline_hashes.rs` pass unchanged (default bit-identical). If F-RE fails, it is
  removed (rule (b) of wave 35) and C3 is reported NOT RUNNABLE. C3 uses `radial_frame_rate = Omega`.

### Arm D: inlet and tip gap (task d)

* Inlet profile (no run): at cut U, the mass-averaged absolute swirl, the radial profile of `c_z` (8 bins), and the relative inflow angle
  `atan(w_theta / c_z)` per bin against the blade metal angle at the leading edge of the section at the bin radius (incidence).
* D1 inlet location: `z_le_root` 0.1 -> 0.3, `axial_extent` 0.4 -> 0.6 (outlet at the same distance behind the blade), `n_up` the smallest count for
  which the largest axial cell ahead of the leading edge at the hub layer is at most that of E0.
* D2 tip gap: `n_gap` 7 -> 11 (two boundary-layer cells of 0.2 / 0.24 mm on each side and 7 core cells of about 0.23 mm instead of 3 of 0.54 mm).

### Arm N: numerics (the factor a-d does not reach; needed for question 2)

First-order upwind on cells of about c/12 adds a numerical viscosity of order `W dx / 2`, an effective chord Reynolds number of order 2 n_chord,
far below the physical 1.3e5; it predicts under-turning (low work) and high loss. Two sensitivity runs (not a convergence study):
* N1 bounded second-order momentum convection: `tvd_van_leer`; if its 300-iteration smoke (ADR 0075 stability criterion: continuity finite and its
  maximum over iterations 200-300 not above its value at iteration 10) is unstable, `tvd_minmod`; if both are unstable, N1 is NOT RUNNABLE.
* N2 in-plane refinement with `upwind`: `n_chord` 12 -> 20, `n_side` 10 -> 16, `n_up` 8 -> 12, `n_down` 8 -> 12 (about 33 000 cells), everything else E0.

### Combined run K

K = E1 + C1 + D1 + D2 (all non-numerics arms that change the model rather than replace a condition; C2/C3 are alternatives to the outlet condition,
not additions), numerics as E0.

### Arm E: blade-element / Euler estimate from the CAD sections (task e), derived from first principles

Assumptions: steady incompressible flow; strip theory (each radius independent, no radial velocity, no radial equilibrium coupling); uniform axial
speed `c_x = Q / (pi (r_s^2 - r_h^2))` (9.478 m/s at the design point) everywhere; no inlet swirl; no work in the tip gap; the 16 CAD sections of
`w34d_fan01_cad_sections.csv` (r <= 247.5 mm) with NACA 4-digit camber lines of the tabulated m and p; `rho` = 1.2, `Omega` = 155.614 rad/s.

1. Cascade momentum (exact for a 2-D cascade): a control volume of one pitch `s = 2 pi r / N` whose periodic sides cancel gives the blade
   circulation `Gamma = s (c_theta2 - c_theta1) = s Delta c_theta` and the tangential force per unit span `F_theta = rho s c_x Delta c_theta`.
2. Euler work per unit mass at radius r: `Y(r) = U Delta c_theta`, `U = Omega r`; ideal total pressure rise `rho Y`; shaft torque
   `M = rho integral (r Delta c_theta) c_x 2 pi r dr`; mass-averaged work `Y_bar = integral Y c_x 2 pi r dr / Q`. Work coefficient
   `psi = Y_bar / U_tip^2`.
3. Blade forces in the vector-mean relative velocity: `w_theta,inf = U - Delta c_theta / 2`, `W_inf = sqrt(c_x^2 + w_theta,inf^2)`,
   `beta_inf = atan(w_theta,inf / c_x)` from the axis; angle of attack against the chord `alpha = beta_inf - gamma` (gamma the stagger from the axis).
   `F_theta = L cos beta_inf + D sin beta_inf` with `L = rho W_inf^2 c C_L / 2`, `D = rho W_inf^2 c C_D / 2`; with D = 0 this reduces to
   Kutta-Joukowski `L = rho W_inf Gamma`.
4. Lift: thin-airfoil theory of an isolated section, `C_L = 2 pi (alpha - alpha_L0)`,
   `alpha_L0 = -(1/pi) integral_0^pi (dy_c/dx)(cos t - 1) dt`, `x = (1 - cos t)/2`. Self-check: for p = 0.5 the NACA camber line is the parabola
   `4 m x (1 - x)` with `alpha_L0 = -2 m` exactly; the quadrature must give it to 1e-8.
5. Steps 1-4 give one implicit equation for `Delta c_theta` per radius, solved by fixed-point iteration to 1e-12.
Variants: EA inviscid linear (C_D = 0, no cap); EB `C_L` capped at 1.1 and `C_D = 0.02` (ASSUMED values typical of 11 % thick cambered sections at
Re 1.3e5-1.6e5, UNVERIFIED, not from a source read in this wave). Stated limits of the method: isolated-airfoil lift overestimates at solidity near
1 (hub, `sigma = c/s` = 1.14) where cascade guidance reduces deviation-free turning differently; no cascade correlation (Carter's rule, Weinig) is
used because none was read (they are UNVERIFIED recollections); no 3-D effects (tip leakage, secondary flow, hub boundary layer). The estimate is a
bracket of the achievable work, not a prediction to a tolerance; it is compared with `W_M`, `W_L` of the CFD and with 197.9 J/kg (measured shaft).
Both operating points are evaluated (no CFD run at the off-design point in this wave).

### Question 2: can a RANS rotor-only steady sector model meet the registered limits in principle? Decision rules fixed now

* Delta p_ts and eta_ts: if two defensible outlet treatments of arm C (C1, C2, C3 against E0) differ in Delta p_ts by more than the half-width
  of the registered limit (3 + 0.03 x 126.46 = 6.8 Pa), the rotor-only model's Delta p_ts is not determined by the rotor to within the limit, and the
  conclusion is that the Delta p_ts and eta_ts limits cannot be met in principle without modelling the installation (or redefining the compared
  quantity). Otherwise that conclusion is not drawn.
* Torque: if the blade-element bracket [EB, EA] contains or exceeds the measured shaft torque and at least one numerics arm moves M toward the
  measured value by a resolved amount, the conclusion is "the torque limit is not excluded in principle; the present deficit is at least partly
  numerical". If neither numerics arm moves M by a resolved amount, the torque gap stays unexplained and no such conclusion is drawn.

### Code policy

Tests in `crates/rcfd-api/tests/fan01_diagnosis_w35c.rs` (long runs ignored, construction / smoke / analytic checks not ignored when fast); S2 in
`turb.rs`; F-RE in `bc_cat.rs` + schema + `docs/case-schema.md` (only if RE-H is not falsified); `fan01_validation.rs` and the ADR 0075 tests are
not edited. Before the long runs, every new set-up gets a construction check and a 30-iteration smoke (continuity finite); the smoke results are
recorded in an amendment before the runs they govern.

## Amendment 1 (2026-10-19): checks, smoke results and set-up corrections, written before any registered long run

Code state of every run below and of the long runs: abd10f2 plus the wave-35C files exactly as committed in d1030ea (built in a private copy of
the tree; commits of other wave-35 agents between abd10f2 and d1030ea are not in the runs).

What I knew when writing this: the construction and the 30-iteration smoke values below, the X1 results, S2a/S2b, the N1 smoke, and the
blade-element values. No arm has been run beyond 30 iterations (N1 schemes: 300).

S2 (moving_wall with SST). S2a first failed to parse: the channel walls given as `fixed` with a non-zero velocity and no `turbulence = "wall"` are
read as inlets (role `auto`), a set-up defect of the test text; `turbulence = "wall"` was added to both walls before any S2a value existed. Measured:
largest difference of u, p, k, omega after 200 iterations 0 (bit-identical). S2b: k-epsilon refused at `boundaries.ymin.velocity` and
`boundaries.ymax.velocity`. S2 is kept.

RE-H and F-RE. X1 with the code as it was: outlet pressure error 47.2 % (`Omega_f = 2`, predicted about 50 %) and 36.8 % (`Omega_f = 1`, predicted
37.5 %) of the range: RE-H holds (ignored test `x1_..._without_the_frame_rate_known_miss`). With `radial_frame_rate = Omega_f`: outlet pressure
0.376 % / 0.397 %, outlet absolute swirl 0.001 % / 0.020 %, mid-length pressure 0.390 % / 1.650 % (limits 3 %), settled in 180 / 184 iterations;
the discrete balance of the applied pressures with the absolute swirl 3.0e-15 / 2.5e-15 of the range. The existing E2 test passes (0.408 %, 0.032 %,
2.669 %, 2.3e-15). F-RE is accepted and kept. Restriction (documented): it assumes `p` is the static pressure, i.e. `rotating_frame` with
`centrifugal = true`; only that case is checked.

Construction (13 332 cells E0, E1, C2, C3; C1 21 060; D1 21 543; D2 15 684; N2 33 076; K 34 395; all `check_mesh` ok, max non-orthogonality
79.0-79.1 deg). Count-rule DEFECT found at construction: the registered C1/D1 rule (largest axial cell at most E0's) does not keep the spacing near the
trailing / leading edge, because the generator fills the region by geometric growth from the boundary-layer cells: E0 grows downstream with ratio 3.56
(largest cell 0.165 m) and upstream with ratio 2.97 (0.067 m); C1 (n_down 24) has ratio 1.35, D1 (n_up 25) 1.28. C1 - E0 and D1 - E0 therefore mix
the location with the axial grading. Correction, two new arms (C1 and D1 run as registered):
* C0: E0 length with `n_down` the smallest count whose downstream growth ratio is at most C1's (n_down 21, ratio 1.344, 19 611 cells);
* D0: E0 length with `n_up` the smallest count whose upstream growth ratio is at most D1's (n_up 21, ratio 1.272, 19 611 cells).
Then outlet location = C1 - C0, inlet location = D1 - D0, and C0 - E0, D0 - E0 are axial-grading (numerics) effects. For the question-2 Delta p_ts rule
the comparisons that count as "defensible outlet treatments" are C1 - C0 (location), C2 - E0 (backflow stabilisation) and C3 - E0 (type); C0 - E0 is
a numerics result, not an outlet treatment.

N2 amended to in-plane refinement only: `n_chord` 20, `n_side` 16, `n_up` and `n_down` stay 8 (the axial grading is measured by C0/D0); 33 076 cells
was the registered variant, the amended one has 27 004 cells (max non-orthogonality 79.0 deg).

N1 smoke (registered rule): `tvd_van_leer` continuity 1.03e-4 at iteration 10, 1.0e69 over 200-300 (diverged); `tvd_minmod` 1.03e-4 at 10,
maximum 1.205e-4 over 200-300 (no growth beyond that, but above the iteration-10 value): both unstable by the registered rule, so N1 is NOT RUNNABLE
as registered. N1b (added now, knowing this smoke): `tvd_minmod` run with the common stop rules (8000 iterations, divergence stop, 60 min).

Hub length (raised by review before any run): the hub cylinder rotates over the whole domain, so C1, D1 and K add rotating hub surface; the
harness now prints the blade, hub and shroud moments separately (with their last-1000-iteration change) and the pressure part of each. Rotor work on
the extension arms is attributed with the blade moment and the U-to-D segment work, not with `W_M` / `W_L`; the inlet-to-U and D-to-outlet segment
work is the hub (minus shroud) friction work and is quoted as such.

Comparability of the cut diagnostics: cut U lies one cell downstream of the inlet-adjacent cell in E0's upstream grading, and cut D inside cells of
up to about 0.05 m; spanwise and incidence numbers are compared only between arms that share a grading (E0, E1, C2, C3, D2, N1b; C0 and C1; D0 and
D1).

Smoke results (30 iterations, continuity at iteration 30, all finite): E0 1.22e-4, E1 1.23e-4, C1 1.16e-4, C2 1.22e-4, C3 1.15e-4, D1 1.70e-4,
D2 1.37e-4, N2 (registered counts) 2.91e-5, K 1.28e-4, C0 5.51e-5, D0 1.33e-4. An observation from the smoke diagnostics (30 iterations, not
converged, no conclusion drawn): the absolute angular-momentum flux at the outlet (3.29 N m in E0) is about 2.6 times the reported torque on blade,
hub and shroud (1.27 N m), and cut U (ahead of the rotor) already carries 0.66 N m; with N2 the ratio is smaller (2.41 against 1.79). Whether this
persists at convergence is part of arm B; if it does, it is a candidate defect or a discretisation error of the relative-velocity formulation, and a
check is registered in an amendment before it is run.

Blade-element estimate (arm E, analytic, computed now): design point EA 242.2 J/kg (torque 2.621 N m, psi 0.163), EB 222.2 J/kg (2.405 N m,
psi 0.150); off-design EA 172.8 J/kg (2.404 N m), EB 179.4 J/kg (2.496 N m; above EA because the drag term adds tangential force and the cap is not
active). Measured shaft: 197.9 J/kg (2.142 N m) and 135.3 J/kg (1.882 N m). Both variants exceed the measured shaft torque by 12-22 % (design) and
28-33 % (off-design): the estimate overshoots the measurement on its own (isolated-airfoil lift at solidity up to 1.14, C_L up to 1.5 in EA, no
3-D losses), so the yardstick for the CFD deficit is the measured shaft work, not EA or EB; EA/EB are "the two variants", not an ordered bracket.
The torque decision rule's first condition ("the bracket contains or exceeds the measured shaft torque") is met at both points; the conclusion still
needs a resolved numerics effect.

Run order: E0 alone first (reproduction rule); then pairs, never two of N2 / K / C1 / D1 together.

## Amendment 2 (2026-10-19): E0 result, a harness correction, and check X2 registered before any other long run

What I knew when writing this: the full E0 run (below) and the 30-iteration smoke of the X2 arms (below). No other arm has run beyond 30 iterations.

E0 (registered run, 12.8 min): converged by the monitor criterion at 8000 iterations; Delta p_ts -3.813 Pa, Delta p_tt 112.520 Pa (ADR 0075
definitions), torque 1.32007 N m: reproduction rule MET (attempt 3: -3.8 Pa, 1.320 N m). Blade 1.29630 N m (pressure part 1.22821), hub 0.02377,
shroud -0.09303 N m on the fluid. Outlet: no backflow (0 by area and mass), axial non-uniformity term 0.95 Pa, `rho <c_theta^2/2>_m` = 58.82 Pa,
`rho <c_z^2/2>_m` = 54.85 Pa; `Delta p_tt - Delta p_ts` = 113.94 Pa closes to 1e-14 with these terms.

Harness correction (definition defect, found in E0's output): the registered cut check compares the cut flux with Q; the code used Q = 1.4032
(the operating point) while the flow through the faceted mesh is 1.400024 m^3/s (the inscribed polygon has 0.23 % less annulus area than the circle,
uniform inflow speed from the circle area), so both cuts failed by 2.26e-3 and the segment split was not printed. Corrected: the cuts are checked
against the inlet-patch flow of the same run (E0: U and D agree with it to 1e-5) and the segment work uses that flow. `W_M`, `W_L` keep Q = 1.4032
as registered (0.23 % effect). E0's segments recomputed from its printed values with Q = 1.400024: inlet->U work 74.1 Pa, Delta p_t -0.47 Pa;
U->D work 243.2 Pa, Delta p_t 103.3 Pa; D->outlet work 48.2 Pa, Delta p_t +7.3 Pa.

Finding in E0 that changes the diagnosis: the angular-momentum balance does not close. The absolute angular-momentum flux at the outlet is 3.288 N m
(inlet 0), the torque on the fluid from blade, hub and shroud is 1.227 N m: 2.061 N m (168 % of the wall torque) has no wall source. Cut U, ahead of
every blade, already carries 0.666 N m (mass-averaged pre-swirl 0.9-2.6 m/s), where only the hub friction (about 0.01 N m upstream) can act. The
outlet swirl kinetic energy (58.8 Pa) is what ADR 0075 left "unexplained" (52 Pa); it is swirl, not axial non-uniformity or backflow.

Hypothesis H-AM: the source is the discretisation of the relative-velocity formulation (`rotating_frame`): first-order upwind convection of the large
solid-body part of the relative velocity (`|w_theta|` 19-39 m/s, against an absolute swirl of a few m/s) on this mesh creates spurious absolute
angular momentum; it is a discretisation error (falls with refinement and with a higher-order scheme), not a code defect; it does not need the blade.
Check X2 (exact solution: an empty passage with every wall at rest in the absolute frame and swirl-free inflow has `c_theta = 0`, so every N m of
outlet angular-momentum flux is error): the E0 mesh with `include_blade = false`, hub and shroud `moving_wall` with `wall_rotation_rate = -Omega` and
the projection, otherwise E0:
* X2a `rotating_frame`, upwind; X2b inertial frame (no frame source, walls and inflow absolute), upwind; X2c `rotating_frame`, upwind, the N2
  in-plane counts (n_chord 20, n_side 16); X2d `rotating_frame`, `tvd_minmod`; X2e `rotating_frame`, `linear_upwind`.
* Decision rules: H-AM is confirmed as the mechanism class if X2a's outlet `|L|` >= 0.1 N m (5 % of E0's 2.06 N m) and X2b's `|L|` <= 1e-2 of
  X2a's. "Discretisation error, not a defect" if, in addition, X2c or X2d is below X2a by a resolved amount (the harness has no monitor of L, so, as
  registered for quantities without a monitor, the resolution is 5 x the run's larger relative torque / Delta p_ts change over the last 1000
  iterations times |L|). If X2a < 0.1 N m, H-AM is falsified (the source needs the blade) and the blade wall treatment is examined next. If X2b is large
  too, the source is not frame-specific and is examined before any other conclusion.
* Smoke (30 iterations, not converged; disclosed, no conclusion drawn from it): outlet `L` X2a 2.106 N m, X2b 0.00016, X2c 0.981, X2d 0.0129;
  X2e diverged (continuity 1.1e5 at iteration 10, 3.1e6 at 11; the 300-iteration stability smoke printed "stable true" because the loop compares a
  non-finite maximum incorrectly, a harness defect of that smoke only): X2e is NOT RUNNABLE and is not run. The X2 meshes have 13 524 cells
  (X2c 27 324).
* N1b's relevance rises with X2d: the registered N1b run is unchanged.

How the attribution reads after X2 (fixed now): blade work is measured by the blade moment (its pressure part is computed from the face pressures
used by the momentum balance); `W_L` and the outlet swirl contain the spurious source and are not rotor work; the share of the Delta p_ts gap
attributed to the spurious swirl is `rho <c_theta^2/2>_m(outlet)` of the arm minus that of a swirl distribution carrying only the wall torque, which
cannot be computed without a profile; the addendum quotes the outlet swirl energy and the angular-momentum excess side by side instead of a single
share.

Run order from here (two at a time): X2a + N1b, X2b + X2d, E1 + C2, N2 + C3, X2c + D2, C0 + C1, D0 + D1, K alone. The machine is shared: the
60-minute wall-clock stop stays as registered.

## Amendment 3 (2026-10-19): one more numerics run, registered after X2a, X2b, X2d and N1b had run

What I knew: the X2a, X2b, X2d and N1b results of the addendum (N1b: torque 1.680 N m, Delta p_ts 27.4 Pa; the angular-momentum excess falls from
2.06 to 0.19 N m). No other arm beyond 30 iterations. Added: KN = N1b + N2 (`tvd_minmod` with `n_chord` 20, `n_side` 16; 27 004 cells), run once
after the registered queue, common stop rules. Purpose: whether the two numerics changes together move the torque further (question 2). It is
not an F-126 attempt; the attribution rules apply unchanged (KN - E0 is the combined numerics effect; additivity against N1b + N2 is reported).

## Addendum: measured values, attribution, misses and deviations

All runs: release builds of abd10f2 plus the wave-35C files (S2 and F-RE as in d1030ea; the harness as in cf5a639, whose changes after d1030ea add
arms and print-outs only, the E0 path unchanged), single thread, two runs at a time on a shared machine, one run per set-up, design point
Q = 1.4032 m^3/s only. "Resolved" and "significant" are the registered rules (5 x the larger last-1000-iteration change; 5 % of the E0 gap). E0 gaps:
Delta p_ts 130.27 Pa (126.46 measured against -3.81), torque 0.8218 N m (2.1419 measured against 1.3201).

### Checks with limits

| Check | Limit | Measured | Result |
|---|---|---|---|
| S2a `moving_wall` = `fixed` moving wall with SST (API channel, 200 iterations) | bit-identical | largest difference 0 | pass (S2 kept) |
| S2b k-epsilon with `moving_wall` | refused | `boundaries.ymin.velocity`, `boundaries.ymax.velocity` | pass |
| X1 default code in a rotating frame (falsifier of RE-H) | RE-H falsified if <= 3 % | 47.2 % (Omega_f 2), 36.8 % (Omega_f 1) | RE-H holds (ignored `..._known_miss`) |
| X1 with `radial_frame_rate` (F-RE) | <= 3 % outlet p, swirl, mid-length p | 0.376 / 0.001 / 0.390 % and 0.397 / 0.020 / 1.650 % | pass (F-RE kept) |
| E2 unchanged | E2 limits | 0.408 %, 0.032 %, 2.669 %, balance 2.3e-15 | pass |
| Blade-element self-check `alpha_L0 = -2 m` (p = 0.5) | 1e-8 | 4.2e-17 | pass |
| E0 reproduction of attempt 3 | 0.2 Pa, 0.1 % | -3.813 Pa, 1.32007 N m | pass |
| N1 smoke (`tvd_van_leer`, then `tvd_minmod`) | registered stability rule | van Leer 1.0e69; minmod 1.205e-4 > 1.028e-4 | N1 NOT RUNNABLE (N1b added in amendment 1) |
| X2e (`linear_upwind`, empty passage) | stable | continuity 1.1e5 at iteration 10 | NOT RUNNABLE |
| X2c (empty passage, refined, upwind) | converged | fields grew to 1e3-1e9 without tripping the continuity stop; torque change 0.37 over the last 1000 iterations | NOT CONVERGED (not used) |
| X2 decision: X2a outlet L >= 0.1 N m and X2b <= 1e-2 X2a | registered | 2.254 N m; 1.6e-4 N m (7e-5 of X2a) | H-AM confirmed |
| X2 classification: X2c or X2d resolved below X2a | registered | X2d 0.0026 N m | discretisation error, not a code defect (by the rule; see the open item below) |

### The runs (all converged by the registered criterion unless stated)

| Arm | Cells | Min | Torque (blade) N m | Delta p_ts Pa | Delta p_tt Pa | outlet `rho<c_theta^2/2>` Pa | cut-U L N m | L excess N m |
|---|---|---|---|---|---|---|---|---|
| E0 base | 13 332 | 12.8 | 1.3201 (1.2963) | -3.81 | 112.52 | 58.82 | 0.666 | 2.061 |
| E1 projected casing | 13 332 | 16.2 | 1.3197 (1.2960) | -3.84 | 112.49 | 58.84 | 0.666 | 2.062 |
| C2 no-reverse outlet | 13 332 | 16.0 | identical to E0 in every printed digit (no backflow, the option never acts) | | | | | |
| C3 radial equilibrium (F-RE) | 13 332 | 16.5 | 1.3180 (1.2926) | -11.98 | 113.73 | 64.91 | 0.668 | 2.052 |
| C0 downstream growth 1.34 | 19 611 | 22.3 | 1.3145 (1.2902) | -4.13 | 113.12 | 58.03 | 0.667 | 2.083 |
| C1 outlet 0.63 m behind the TE | 21 060 | 24.2 | 1.3269 (1.2869) | -7.14 | 149.80 | 93.60 | 0.668 | 3.169 |
| D0 upstream growth 1.27 | 19 611 | 24.7 | 1.3131 (1.2893) | +2.61 | 119.36 | 59.18 | 0.528 | 2.082 |
| D1 inlet 0.3 m ahead of the LE | 21 543 | 27.6 | 0.8797 (0.8411) | -32.01 | 104.25 | 76.13 | 1.790 | 3.043 |
| D2 tip gap n_gap 11 | 15 684 | 17.1 | 1.3210 (1.2972) | -4.06 | 112.36 | 58.73 | 0.664 | 2.057 |
| N1b `tvd_minmod` | 13 332 | 15.9 | 1.6800 (1.6581) | +27.38 | 105.95 | 22.12 | 0.028 | 0.190 |
| N2 in-plane refined, upwind | 27 004 | 31.0 | 1.8144 (1.7929) | +42.64 | 148.59 | 47.83 | 0.407 | 1.202 |
| KN minmod + in-plane refined | 27 004 | 24.5 | 2.0163 (1.9970) | +76.17 | 160.34 | 27.06 | 0.024 | 0.117 |
| K = E1 + C1 + D1 + D2 | 34 395 | 44.2 | 0.8797 (0.8250) | -37.70 | 136.26 | 105.38 | 1.789 | 3.970 |
| X2a empty, rotating, upwind | 13 524 | 0.8 (374 its, tolerance) | no blade | | | 28.23 | 0.661 | 2.313 (outlet L 2.254) |
| X2b empty, inertial, upwind | 13 524 | 0.3 (169 its, tolerance) | no blade | | | 0.000 | 0.0001 | 0.0002 (outlet L 1.6e-4) |
| X2d empty, rotating, minmod | 13 524 | 15.4 | no blade | | | 19.97 | 0.023 | 0.009 (outlet L 0.0026) |

"L excess" = outlet angular-momentum flux minus the torque of blade, hub and shroud on the fluid (0 for a conservative converged solution). The X2
runs have no blade: their total-pressure change is a duct value, X2a +23.2 Pa, X2d +16.8 Pa, X2b -1.5 Pa. A duct whose walls are at rest can only
lose total pressure: X2a and X2d create 23 and 17 Pa that no wall supplies.

### Arm B: energy balance of E0 (closes the ADR 0075 "unexplained" outlet energy)

`Delta p_tt - Delta p_ts` = 113.94 Pa = 0 (uniform outlet pressure) + `rho<c_z^2/2>` 54.85 (non-uniformity 0.95 of it) + `rho<c_theta^2/2>` 58.82
+ `rho<c_r^2/2>` 0.27; identity residual 1e-14. No backflow at the outlet (0 by area and by mass). The ADR 0075 "unexplained 52 Pa" is outlet SWIRL,
not axial non-uniformity or backflow. Euler work: `W_M` 122.0 J/kg from the torque, `W_L` 303.9 J/kg from the outlet angular-momentum flux: the
torque balance is open by 2.061 N m (168 % of the 1.227 N m wall torque). Segments (mesh flow 1.400024): inlet->U work 74.1 Pa (pre-swirl no wall can
produce), U->D 243.2 Pa against a total-pressure rise of 103.3 Pa, D->outlet 48.2 Pa with a total-pressure RISE of 7.3 Pa in a duct. With angular
momentum and energy not conserved, `W_L` and the "loss" `rho W_L - Delta p_tt` (254.6 Pa) are not physical; the blade moment is the rotor-work
measure used below. In KN the balance closes to 0.117 N m (6 % of its wall torque) and `W_M` = 186.3 J/kg, `W_L` = 193.2 J/kg.

### Check X2 and the mechanism

X2a (empty passage, exact `c_theta = 0`): outlet angular-momentum flux 2.254 N m and cut U 0.661 N m, the same as E0's 0.666 N m with the blade;
X2b (the same in the inertial frame) 1.6e-4 N m. The source is the `rotating_frame` relative-velocity discretisation on this mesh and does not need
the blade (H-AM confirmed). X2d (minmod) brings the outlet flux to 0.0026 N m, so by the registered rule this is a discretisation error, not a code
defect; X2c (refinement leg) did not converge. Open item, not explained by the rule: X2d still carries spurious swirl of both signs (-3.4 to +1.4 m/s
in the bins of cut D, `rho<c_theta^2/2>` 20 Pa) and creates 16.8 Pa of total pressure in a duct with walls at rest: minmod removes the net
angular-momentum source but not the spurious swirl energy. Where the error is generated was not located (candidate, untested: the H-slit cells of
the chord band are sheared along the camber line even without a blade). No fix is made (task item 3 covers defects); the remedy that removes the
large solid-body part from the convected quantity, an absolute-velocity formulation on the bladed sector, is refused today (`mrf` on this mesh,
ADR 0065 S4).

### Attribution (registered rules; share of the E0 gap, positive closes it)

| Comparison | Factor | delta torque N m (share) | delta Delta p_ts Pa (share) | Verdict |
|---|---|---|---|---|
| E1 - E0 | casing projection (task a) | -0.0003 (-0.04 %), resolved | -0.03, not resolved | not significant; the registered prediction "no resolved effect on M" is falsified by 3e-4 N m |
| C2 - E0 | backflow stabilisation | 0 | 0 | no effect (no backflow) |
| C3 - E0 | outlet type: radial equilibrium | -0.0021 (-0.3 %) | -8.17 (-6.3 %) | significant for Delta p_ts, in the wrong direction |
| C1 - C0 | outlet location (+0.4 m) | +0.012 (+1.5 %; blade -0.003) | -3.01 (-2.3 %) | not significant; the extra duct adds 1.09 N m spurious angular momentum and 36.7 Pa spurious total pressure |
| C0 - E0 | downstream axial grading | -0.0055 (-0.7 %) | -0.32 (-0.2 %) | not significant |
| D1 - D0 | inlet location (+0.2 m) | -0.433 (-52.7 %) | -34.6 (-26.6 %) | significant, wrong direction: the extra upstream duct adds 1.26 N m of spurious pre-swirl and the blade unloads by 0.448 N m |
| D0 - E0 | upstream axial grading | -0.0070 (-0.9 %) | +6.42 (+4.9 %) | not significant (just under 5 %) |
| D2 - E0 | tip-gap resolution | +0.0009 (+0.1 %) | -0.24 (-0.2 %) | not significant |
| N1b - E0 | minmod convection | +0.360 (+43.8 %) | +31.19 (+23.9 %) | significant |
| N2 - E0 | in-plane refinement (12->20 chord, 10->16 pitch cells) | +0.494 (+60.2 %) | +46.45 (+35.7 %) | significant |
| KN - E0 | minmod + refinement | +0.696 (+84.7 %) | +79.99 (+61.4 %) | significant; against N1b + N2 = +0.854 / +77.6: additive by the rule (0.158 <= 0.171) |
| K - E0 | E1 + C1 + D1 + D2 | -0.440 against the sum -0.433 | -33.89 against the sum -31.79 | additive |

Inlet profile (task d): at cut U the absolute pre-swirl of E0 is 0.9-2.6 m/s (exact 0) and the incidence against the leading-edge metal angle runs
from +9.8 deg (r = 0.132) to -1.9 deg (r = 0.242); on the same mesh without spurious swirl (X2b) +11.1 to -1.2 deg: the spurious pre-swirl lowers the
incidence by about 0.7-1.3 deg. In N1b and KN the pre-swirl is 0.03 and 0.02 N m. Tip gap: D2 (7 core cells instead of 3) changes nothing resolved
that matters.

Spanwise work at cut D (`Omega r c_theta`, J/kg, hub to tip, bins with faces only): E0 157-440 rising to the tip (contaminated); N1b 154-236; N2
177-405; KN 178-273; blade-element EA 241-260, EB 210-253. N1b and KN are close in shape to the strip estimate except at the tip bin.

### Arm E: blade-element estimate against the CFD

| | Work J/kg | psi | torque N m |
|---|---|---|---|
| EA inviscid linear (design) | 242.2 | 0.163 | 2.621 |
| EB C_L <= 1.1, C_D 0.02 (design) | 222.2 | 0.150 | 2.405 |
| measured shaft (design) | 197.9 | 0.133 | 2.142 |
| E0 blade + hub | 122.0 | 0.082 | 1.320 |
| N1b | 155.3 | 0.105 | 1.680 |
| N2 | 167.7 | 0.113 | 1.814 |
| KN | 186.3 | 0.126 | 2.016 |

Off-design (Q = 1.8039, analytic only): EA 172.8 J/kg (2.404 N m), EB 179.4 (2.496); measured 135.3 (1.882). The strip estimate overshoots the measured
shaft work by 12-33 % (isolated-airfoil lift at solidity up to 1.14, no 3-D losses); it is a plausibility bound, not a target.

### Which factors account for the gap (task 2)

Torque gap 0.822 N m: the resolved part is numerical. Minmod +0.360 N m (44 %), in-plane refinement +0.494 N m (60 %), both +0.696 N m (85 %),
leaving KN 5.9 % below the measured torque (the measured shaft power may contain bearing loss; ADR 0075). The mechanism identified is spurious
absolute angular momentum from the upwind relative-velocity discretisation: 0.67 N m of pre-swirl ahead of the rotor unloads the blade, with a dose
response (D1 - D0: 1.26 N m more pre-swirl, 0.45 N m less blade torque). The casing treatment, the outlet condition (location, type, backflow option),
the inlet location as a physical change, the axial grading and the tip-gap resolution each account for less than 2 % of the torque gap.
Delta p_ts gap 130.3 Pa: numerics +31 (minmod), +46 (refinement), +80 Pa (both; KN 76.2 Pa against 126.5 measured, eta_ts 0.341 against 0.532);
E0's outlet swirl energy (58.8 Pa) is mostly spurious (KN 27.1 Pa at 53 % more torque); radial equilibrium -8.2 Pa; everything else within -3 to
+6.4 Pa. The remaining 50 Pa after KN contains the installation (bellmouth, finite blunt hub, struts, motor, diffuser, exit to ambient: not
modelled), the residual numerics (X2d), and whatever else a rotor-only model misses; these are not separated by this wave. KN is not an F-126
attempt and its torque (-5.9 %) does not meet the 5 % limit anyway; nothing is claimed from it.

### Question 2, by the registered rules

* Delta p_ts and eta_ts: C3 - E0 = -8.17 Pa exceeds the 6.8 Pa half-width (C1 - C0 = -3.0, C2 - E0 = 0): the rule fires. A rotor-only sector's
  Delta p_ts is not determined by the rotor to within the registered limit, so the Delta p_ts and eta_ts limits cannot be met in principle without
  modelling the installation or redefining the compared quantity. Caveat: the C3 effect was computed on a swirl field that is itself about 2.5 times
  too large (spurious); its size on a correct solution is not known. The conclusion also rests on the independent fact that the measured quantity is
  an installation value referred to ambient.
* Torque: both blade-element variants exceed the measured shaft torque (2.405-2.621 against 2.142 N m) and both numerics arms move the torque toward
  the measurement by resolved amounts. Conclusion as registered: the torque limit is not excluded in principle; the present deficit is at least
  partly numerical. Not shown: that any mesh or scheme available today converges the torque (no mesh study was made; the refined empty-passage
  check X2c did not converge; minmod leaves spurious swirl energy; `linear_upwind` diverges).

F-126 remains MISSED; no arm is an F-126 attempt; no fan performance claim is made.

### Deviations from the registered plan (candid list)

* The C1/D1 count rule had a defect (the largest cell does not keep the near-edge spacing); C0/D0 were added in amendment 1 before any long run.
* N1 as registered was NOT RUNNABLE; N1b (minmod) was registered after its smoke. N2 was amended to in-plane refinement only.
* The cut check compared with the operating-point Q instead of the mesh flow (0.23 % apart); corrected in amendment 2 after E0; E0's segments
  recomputed from its printed values.
* X2 (amendment 2) and KN (amendment 3) were added after E0, and after X2a, X2b, X2d and N1b had run; each amendment states what was known.
* X2e's 300-iteration smoke printed "stable true" for a diverged run (the loop ignored the solver failure flag); the harness was corrected and X2e is
  recorded as NOT RUNNABLE from its continuity history.
* S2a's first text was a set-up defect (walls without `turbulence = "wall"`), corrected before any value existed.
* The registered E1 prediction ("no resolved effect on M") is falsified by 3e-4 N m (resolved, 0.04 % of the gap).
* The 60-minute stop never triggered (longest run K, 44.2 min); the off-design point was evaluated analytically only, as registered.

### Unverified references

* EB's C_L cap 1.1 and C_D 0.02 (assumed; not from a source read in this wave). Carter's rule and Weinig's factor were not used (recalled only).
* Air density 1.2 kg/m^3 and 1486 rpm, as in ADR 0075.

## Amendment 4 (2026-10-20): can the X2 source be a code defect? Registered after the addendum above, before these runs

What I knew: every result of the addendum. Reason (review): X2b's flow has no swirl, so it cannot reveal an error that needs a large tangential
velocity (the vector rotation at the cyclic seams, convection of swirl through the camber-sheared cells, the seam gradient); the registered rule
classified X2 as "discretisation error" through X2d, which still creates 16.8 Pa of total pressure in a duct, and the refinement leg X2c did not
converge. So "not a code defect" is not yet shown. Three runs, on a fresh archive of the HEAD at hand-back (which contains wave 35A/B/D code, among it
the ADR 0077 opt-in `solver.periodic_gradient = "partner_cell"`, F-H0):
* X2a-HEAD: X2a unchanged. Reproduction rule: outlet L 2.25440 N m and Delta p_ts -59.984 Pa to every printed digit; if it reproduces, the
  measurements of this ADR carry over to that HEAD; if not, the code-state statement is corrected and the difference reported.
* X2f: X2a with only the `rotating_frame` source removed. Mesh, inflow expressions `(Omega y, -Omega x, U)`, hub and shroud `moving_wall` at
  `-Omega` with the projection are kept and are now absolute: the exact solution is the solid-body swirl `c_theta = -Omega r` carried unchanged,
  whose static pressure is not uniform, so the outlet is `radial_equilibrium_outlet` (area average 0, `radial_frame_rate` 0; exact for this
  field, X1). Exact check: L excess (outlet minus inlet angular-momentum flux minus hub and shroud torque) = 0. Rules: X2f excess >= 0.5 x X2a's
  2.313 N m: the error is the convection of a large swirl on this mesh and the frame terms are cleared; X2f excess <= 0.1 x 2.313 N m: the error
  is in the rotating-frame source / coupling terms (lagged Coriolis, face body force, wall-pressure rule, pair mean), a candidate defect to be
  located (not fixed in this wave unless located and checked); in between: inconclusive.
* X2h = X2a + `partner_cell`; X2i = X2d + `partner_cell`. If either removes at least 5 % of X2a's excess (0.116 N m) by a resolved amount, or X2i
  removes at least half of X2d's spurious total-pressure rise (16.8 Pa) or swirl energy (20.0 Pa), the "not a defect" classification is withdrawn
  for that part (the F-H0 seam-gradient defect of ADR 0077 then contributes), and N1c = N1b + `partner_cell` (FAN-01, 13 332 cells) is run once to
  measure its effect on the FAN-01 numbers. Otherwise N1c is not run.
Order: X2a-HEAD, then X2f + X2h, then X2i (+ N1c if triggered); one cargo at a time, two test threads.

### Amendment 4 results (runs on a fresh archive of 0785d45, the HEAD at hand-back)

| Run | Measured | Rule | Result |
|---|---|---|---|
| X2a-HEAD | outlet L 2.25440 N m, Delta p_ts -59.984 Pa, 374 iterations, every printed digit equal to X2a | reproduce | reproduced: the measurements of this ADR carry over to 0785d45 |
| X2f (frame source removed, solid-body swirl in the absolute frame) | L excess 2.052 N m (inlet L -10.129, outlet -8.142 N m: the solid-body swirl is not carried unchanged; total pressure -200.6 Pa) | >= 0.5 x 2.313: convection; <= 0.1 x: frame terms | >= 0.5 x: the error is the convection of a large swirl on this mesh; the rotating-frame source and coupling terms are cleared |
| X2h (X2a + `partner_cell`) | identical to X2a in every printed digit (upwind uses no momentum gradient) | 0.116 N m | no effect |
| X2i (X2d + `partner_cell`) | Delta p_tt 16.71 Pa (X2d 16.77), swirl energy 19.96 Pa (19.97), outlet L -0.0016 N m | half of 16.8 / 20.0 Pa | no effect; N1c not run |

Conclusion of the defect question: the spurious angular momentum and swirl energy come from convecting a large swirl (the solid-body part of the
relative velocity) on this H-slit mesh; they appear without any frame term (X2f) and do not involve the seam gradient (X2h, X2i). By the registered
rule it is discretisation error, and the bladed runs show it falls with in-plane refinement (L excess 2.06 -> 1.20 N m, N2) and with minmod (0.19,
N1b; 0.12 with both, KN). Not shown: convergence with h on the empty passage (X2c did not converge), and the cell-level location of the error.
Note: X2a, X2b, X2f, X2h converged by residual tolerance in under 1000 iterations, so their printed last-1000-iteration change is "inf"; their
comparisons rest on tolerance convergence (1e-5), not on the 1000-iteration rule. The incidence shift quoted in the attribution section compares
bladed E0 with the unbladed X2b 40 mm upstream, where the blade's potential field still acts; the E0 against N1b comparison (both bladed, same
grading, pre-swirl 0.67 against 0.03 N m) gives +9.8 / +10.8 deg at the hub bin and -1.9 / -1.1 deg at the tip bin, i.e. 0.8-1.0 deg.

### Final gate

Fresh `git archive` of 18b23f6: `cargo fmt --check` clean, `cargo clippy --workspace --all-targets -- -D warnings` clean, `cargo test --workspace
--release --no-fail-fast` 1853 passed, 0 failed, 494 ignored. The HEAD at hand-back (c0a3caf) differs from 18b23f6 only in a wave-35B ADR text; this
paragraph is a docs-only commit and was not re-gated.
