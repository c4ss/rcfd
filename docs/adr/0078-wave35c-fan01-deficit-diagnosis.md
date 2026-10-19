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
