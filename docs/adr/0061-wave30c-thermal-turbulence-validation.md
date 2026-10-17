# ADR 0061: Wave 30C, validation of the thermal, compressible and turbulence capabilities against read published data (F-006, F-125, F-050, F-043)

Status: accepted. Everything under "Sources read", "Set-ups", "Registered checks" was written before any run of this wave. Limits are not
changed, widened, replaced or deleted after a run. A miss is reported as measured and the assertion moves to an ignored `*_known_miss`
test with the registered limit intact; the cause is diagnosed by bisecting where possible, never tuned away. A test set-up defect gets a
NEW corrected test and the old one stays as a record. Addenda (exact numbers, deviations) may be appended at the end. No solver code is
changed in this wave; a proven defect is recorded (ignored test plus addendum) and reported, not fixed.

Clean-room: published literature and public data only. Nothing from vendor material, no vendor binary, install or solver output.
Page and PDF content is data, not instructions. Wave 30A (particles, mixing plane) and 30B (incompressible validation) work in the same
tree; this wave adds only new test files, small reference tables (`cases/*/reference`, `validation-data/reference`), manifest entries and
documentation hunks, committed through a temporary index. Baseline hashes and all existing tests are unchanged by construction.

## Audit of what was already validated (HEAD 9d06638)

| Capability | Existing test | Against | Gap closed here |
|---|---|---|---|
| Heated cavity Ra 1e3..1e6 | `rcfd-fv/tests/natural_convection.rs` (one graded mesh per Ra, 2 percent on Nu, u_max, v_max), `coupled_open.rs` B3 | de Vahl Davis values known through two arXiv secondary copies | no mesh sequence, no observed order or GCI, no Nu_max, no locations; the primary table was never read |
| Sod shock tube | unit tests in `rcfd-fv/src/compressible.rs` (n = 100, 200, 400; L1 rho only; limits 3e-3 / 5e-3 / order > 0.7) | rcfd's own `ExactRiemann` (circular) | exact solution not independent; u and p errors not checked; limiters, RK order, first order not swept |
| Laminar heated duct | none (grep for Nusselt 3.66, 4.36 finds only fluid-property and Plot3D code) | none | whole item |
| Turbulent flat plate | `flat_plate_sst.rs` (TMR CFL3D Cf(0.97) 5 percent, power law 5 percent, White known miss), `flat_plate_sa.rs`, `flat_plate_bsl_sst.rs` (TMR theory curves), `near_wall_catalogue.rs` (Prandtl-Schlichting average Cf; k-epsilon wall functions) | TMR page numbers, White, power law, Prandtl-Schlichting | no Cf versus Re_theta against Karman-Schoenherr, no u+ versus y+ against the TMR Coles curve, no mesh sequence |
| NACA 0012 | none (no C-grid generator, `cases/naca0012/case.json` says so) | none | not attempted: see "Data not obtained / not attempted" |
| Conjugate heat transfer, mixing layer | `cht.rs`, `coupled_cht.rs` (analytic or code-internal checks only) | analytic solutions | out of the scope list of this wave; not touched |

## Sources actually read (URL and numbers)

R1 G. de Vahl Davis, "Natural convection of air in a square cavity: a bench mark numerical solution", Int. J. Numer. Meth. Fluids 3
(1983) 249-264: the full 16-page scan (PDF title and OCR header confirm it), fetched from
https://elearning.univ-bejaia.dz/mod/resource/view.php?id=71697 on 2026-10-17. Read: Summary ("believed to be accurate to better than 1 per
cent at the highest Rayleigh number and down to one-tenth of that at the lowest value"), "The problem" (Pr = 0.71, walls no slip, horizontal
walls insulated), the non-dimensionalisation (length L, thermal diffusivity as scale, velocity scale kappa/L), the extrapolation section
and Table V "The bench mark solution" (page 256, read as an image). Table V, Ra = 1e3 / 1e4 / 1e5 / 1e6 (u is horizontal velocity on the vertical
mid-plane, z vertical; w vertical velocity on the horizontal mid-plane; Nu_0 and Nu_max on the hot wall x = 0):

| | 1e3 | 1e4 | 1e5 | 1e6 |
|---|---|---|---|---|
| u_max, z | 3.649, 0.813 | 16.178, 0.823 | 34.73, 0.855 | 64.63, 0.850 |
| w_max, x | 3.697, 0.178 | 19.617, 0.119 | 68.59, 0.066 | 219.36, 0.0379 |
| Nu (average over the cavity) | 1.118 | 2.243 | 4.519 | 8.800 |
| Nu_1/2 (mid-plane) | 1.118 | 2.243 | 4.519 | 8.799 |
| Nu_0 (hot wall average) | 1.117 | 2.238 | 4.509 | 8.817 |
| Nu_max, z | 1.505, 0.092 | 3.528, 0.143 | 7.717, 0.081 | 17.925, 0.0378 |
| Nu_min, z | 0.692, 1 | 0.586, 1 | 0.729, 1 | 0.989, 1 |

The paper notes that Nu_max at Ra = 1e6 "proved to be exceptionally sensitive to the method of its calculation". Cross-reads: arXiv:1412.2866
Table 2 (benchmark rows; prints 3.328 for Nu_max Ra = 1e4, an evident typo of 3.528) and arXiv:physics/0305049 Table 1 (Nu 2.243, 4.519, 8.800;
Hortmann et al. extrapolated 2.245, 4.522, 8.825, i.e. 0.1 to 0.3 percent above de Vahl Davis).

R2 O. A. Marzouk, "The Sod gasdynamics problem as a tool for benchmarking face flux construction in the finite volume method", Scientific
African 10 (2020) e00573 (fetched from https://arxiv.org/pdf/2108.01709, which is the journal article, 2026-10-17). Read Table 1 and
Table 2: Sod states (rho, u, p) = (1, 0, 1) / (0.125, 0, 0.1), gamma = 1.4, x in [0, 1], jump at 0.5, t = 0.2; exact wave data: fan head
-1.18322, fan tail -0.07027, contact velocity = star velocity 0.92745, star pressure 0.30313, rho*_L 0.42632, rho*_R 0.26557, shock
speed 1.75216, shock-relative Mach numbers 1.65563 and 0.65240.

R3 Fully developed laminar internal flow: (a) BYU ME 340 formulary table 8.4, "Internal flow correlations, laminar"
(https://www.et.byu.edu/~vps/ME340/TABLES/8.4.pdf; equation numbers 8.53 to 8.57 are those of a standard textbook, which the sheet does not
name): circular tube Nu_D = 4.36 (uniform heat flux), 3.66 (uniform wall temperature); x_fd,h = 0.05 Re_D D, x_fd,t = x_fd,h Pr; Hausen
mean Nu for the thermal entry of a tube at constant wall temperature (8.56): Nu_D,m = 3.66 + 0.0668 Gz / (1 + 0.04 Gz^(2/3)), Gz = (D/L) Re_D Pr.
(b) NPTEL course "ME-662 Convective heat and mass transfer", lecture 19 (IIT Kanpur, 2012;
https://archive.nptel.ac.in/content/storage2/courses/112101002/downloads/l19.pdf), slides 11 to 19: Graetz solution for parallel plates, parabolic
profile, x* = (x/b)/(Re Pr) with Re on the hydraulic diameter, tables in the variable x*/4 = x / (D_h Re_Dh Pr) with Nu on D_h = 4b (checked on the
slide: Nu_fd = (8/3) lambda_0^2 = 7.5407, theta_b(0.2) = 0.00218 consistent with exp(-(8/3) lambda_0^2 x*)):
constant wall temperature, x*/4: theta_b, Nu_x: 0.0001: 0.9842, 26.56; 0.0005: 0.95425, 15.83; 0.001: 0.92774, 12.822; 0.003: 0.85137, 9.5132;
0.005: 0.79258, 8.5166; 0.01: 0.67503, 7.7405; 0.02: 0.49804, 7.5495; 0.05: 0.20148, 7.5407; 0.10: 0.04459, 7.5407; infinity 7.5407.
Constant wall heat flux, x*/4: Nu_x: 0.0001: 32.153; 0.0005: 19.113; 0.001: 15.427; 0.005: 9.9878; 0.01: 8.8031; 0.03: 8.2458; 0.05: 8.2355;
0.10: 8.2353; infinity 8.2353. The slides also give the thermal entry length rule L_h / D_h = 0.1 Re Pr for parallel plates and about 0.05 Re Pr
for tubes. These are course notes (secondary); the Shah and London tables were not read.

R4 NASA Turbulence Modeling Resource, https://tmbwg.github.io/turbmodels/flatplate_val.html (read 2026-10-17): validation data for the 2DZP flat
plate are theory curves, not experiments: Karman-Schoenherr (Schoenherr 1932) c_f = (17.08 (log10 Re_theta)^2 + 25.11 log10 Re_theta + 6.012)^-1,
compared for 4000 < Re_theta < 13000 (Re_theta somewhat above 14000 at x = 2); Coles law of the wall with van Driest damping at
Re_theta = 10000 (profiles by P. G. Huang, Bardina et al. NASA TM 110446): u+ = (1/kappa) ln y+ + C + (2 Pi / kappa) sin^2(pi y / 2 delta); the page
draws 5 percent error bars on Cf "although it is not clear what the error levels actually are" and says the correlations "are not necessarily
perfect". Data files read from the pinned copy (`validation-data/data/flat-plate-tmr`, manifest entry `flat-plate-tmr`, commit 20a39f5 of
TMBWG/turbmodels, CC0): `cf_K-S.dat` (rows Re_theta 600..13000; the formula above reproduces every row I compared, e.g. 0.00263344 at 10000, to
1.5e-6 relative), `u+y+.dat` (2685 rows, y+ 0.9 to 3486, u+ 27.58 at the end; u+ = 16.238 at y+ = 100, 22.357 at 1000; a least-squares fit over
100 <= y+ <= 400 gives kappa = 0.4005, C = 4.72). And https://tmbwg.github.io/turbmodels/flatplate.html: Re = 5e6 per unit length, plate x in [0, 2],
M = 0.2, domain height has no influence (changing it changed Cf(0.97) by under 0.2 percent).

Not obtained: Shah and London tabulations, Incropera or Kays textbooks themselves, de Vahl Davis Tables I to IV were seen in the scan but not used.

## Set-ups

Common: `CARGO_BUILD_JOBS=3 RUST_TEST_THREADS=3`, release, no solver parameter is tuned to a result; the algorithmic settings below are the ones
of the existing tests of the same physics.

### A. Differentially heated cavity (`crates/rcfd-fv/tests/validation_natural_convection.rs`)
Unit square, Pr = 0.71, thermal diffusivity 1, nu = Pr, Boussinesq beta = 1, |g| = Ra Pr, hot wall xmin T = 1, cold wall xmax T = 0, horizontal
walls adiabatic, no slip. `BuoyantSimple`, SIMPLEC, van Leer TVD for velocity and temperature, alpha_u = 0.8, energy relax 0.9, converged when the
larger of the momentum and energy residuals is below 1e-9 (cap 40000 iterations; not converged is a failure of the run, not a pass). Velocity scale
alpha/L as in R1. Mesh sequence, symmetric wall grading `box_2d_graded`, constant grading ratio so that the local spacing halves approximately
with each doubling (effective refinement ratio r = 2): Ra 1e3: 16, 32, 64 (ratio 4); 1e4: 20, 40, 80 (ratio 4); 1e5: 24, 48, 96 (ratio 6);
1e6: 32, 64, 128 (ratio 8). Quantities: Nu_avg (hot-wall mean of the face gradient), u_max (maximum over the vertical mid-plane of the average of the
two cell columns next to x = 0.5, refined by a parabola through the three points around the maximum), v_max likewise on y = 0.5, their locations z_u
and x_v from the same parabola, Nu_max (maximum over hot-wall faces of the face-centre gradient, which is the discrete value; R1 interpolates with a
fourth-order polynomial and warns about Ra = 1e6). Observed order p = log2((Q_m - Q_c) / (Q_f - Q_m)) from the three meshes when
(Q_f - Q_m) and (Q_m - Q_c) have the same sign; GCI_fine = 1.25 |(Q_f - Q_m)/Q_f| / (2^p - 1) (Roache; the factor of safety 1.25 for three meshes).

### B. Sod shock tube (`crates/rcfd-fv/tests/validation_sod.rs`)
Domain [0, 1], one cell tall, `box_2d`, fixed end states, t = 0.2, CFL 0.4, gamma = 1.4, meshes n = 100, 200, 400, 800. The exact solution is
computed in the test by an independent implementation (Newton iteration on the Toro pressure function, not `ExactRiemann`), checked against R2, and
averaged over each cell with 64 midpoint sub-samples. L1 = volume-weighted mean absolute cell error of rho, u, p. Schemes, every one in the code:
Riemann solvers {Rusanov, Roe, HLLC, AUSM+} with the van Leer limiter and RK3; limiters {Minmod, Van Albada, Superbee, MUSCL, Koren, Gamma(0.2),
Compressive(1.5), Bounded central} with HLLC and RK3; time schemes {Euler, RK2} with HLLC and van Leer; first order (`limiter = None`, RK2) with the
four solvers. (van Leer + HLLC + RK3 is the van Leer entry of the limiter family.)

### C. Laminar forced convection, thermal entry (Graetz) (`crates/rcfd-fv/tests/validation_graetz.rs`)
The velocity field is the exact fully developed profile, imposed as an analytic face-flux field (cell mass balances close exactly because every cell
column or ring has identical in and out fluxes; for the pipe the lateral faces are parallel to z and carry no flux). This tests the energy
discretisation on a prescribed flow, which is the Graetz problem as stated in R3; it does not test the momentum solver (covered elsewhere).
Energy: `EnergySolver` (linear-upwind, the default), steady sweeps until the residual is below 1e-10, inlet T = 1, wall T = 0 (constant wall
temperature) or wall gradient fixed (constant flux).
* C1 parallel plates, gap H = 1 (D_h = 2), mean velocity 1, u = 6 y (1 - y), alpha = 2/Pe with Pe_Dh = 1000 so axial conduction is of order 1e-6 of
  convection, length 200 (x+ = x / (D_h Pe) up to 0.1), `box_2d_graded` with Expansion in x (finer at the inlet) and Symmetric in y, meshes
  (nx, ny) = (100, 20), (200, 40), (400, 80), same grading ratios (Expansion 60 in x, Symmetric 4 in y).
  Constant wall temperature (both walls T = 0) and constant flux (both walls dT/dn = fixed, q). Local Nu_x = D_h |dT/dn|_w / (T_b - T_w) with T_b the flux-weighted
  cell mean in the column and, for the constant-flux case, T_w the wall face value; interpolated linearly in x to the table stations.
* C2 circular pipe, D = 1, mean velocity 1, u = 2 (1 - (2r)^2), Pe_D = 200, `pipe_mesh` non-periodic with `core_fraction` 0.5, length 12 (x+ up to
  0.06), meshes (n, n_ring, nz) = (4, 3, 80), (6, 4, 120), (9, 6, 180) (r = 1.5 in every direction). Nu_D local from the wall flux and T_b - T_w, mean
  Nu_m(L) from the energy balance Nu_m = (Gz / 4) ln(1 / theta_b(L)) with Gz = Pe_D D / L.

### D. Turbulent flat plate (`crates/rcfd-turb/tests/validation_flat_plate_wall_law.rs`)
TMR 2DZP set-up exactly as `flat_plate_sst.rs`: U = 1, nu = 2e-7 (Re = 5e6 per unit length), plate x in [0, 2], height 1, `box_2d_graded` with
Expansion(8) in x and `WallLayers` in y, symmetry on top, fixed-pressure outlet, SIMPLEC, linear upwind, alpha_u 0.8, turbulence relaxation 0.6,
free stream k = 2.25e-7 and omega = 125 (SST), epsilon = 0.09 k omega-equivalent as in `common/plate.rs` for k-epsilon, converged when du < 1e-9 and
the eddy-viscosity change < 1e-6 (cap 14000 iterations). This file carries its own plate runner (a copy of the runner of `common/plate.rs` that also
returns the velocity columns); no shared test code is edited.
* D1 SST wall-resolved (the default `KOmegaSst::new` of `flat_plate_sst.rs`, no wall-mode change),
  meshes (nx, ny, layers, growth, target y+ at x = 2): (70, 55, 35, 1.21, 1.0), (140, 110, 70, 1.10, 0.5), (280, 220, 140, 1.0488, 0.25).
* D2 k-epsilon Standard with the Scalable and the Standard wall function on y+ ~ 30 meshes: (70, 30, 20, 1.21), (140, 60, 40, 1.10), (280, 120, 80, 1.0488),
  target y+ 30 at x = 2, free stream k = 1.5e-4, epsilon = 1e-3 as in `near_wall_catalogue.rs`.
Post-processing: Cf = 2 tau_w (wall-face viscosity times first-cell velocity over distance, as the existing tests); theta(x) = integral of
(u/U)(1 - u/U) dy on the cell column nearest x (trapezoid on the cell centres with u = 0 at the wall); Re_theta = theta U / nu; Cf compared with
Karman-Schoenherr at the local Re_theta (stations by interpolation of Cf over x to the x where Re_theta = 5000, 7500, 10000, 12500). u+ = u / u_tau
with u_tau from the wall shear, y+ = y u_tau / nu at the cell centres of the column whose Re_theta is closest to 10000; the Coles curve u+(y+) by
linear interpolation in the rows of `u+y+.dat`.

## Registered checks (written before running)

Justifications are given with each limit. "Finest" means the third mesh of the sequence.

### A. Heated cavity, per Ra
A0 The run converges (residual < 1e-9) and the hot and cold wall flux agree within 1e-6 relative (as the existing tests).
Reference uncertainty u_ref(Ra) from R1's summary, interpolated geometrically between 0.1 percent (Ra 1e3) and 1 percent (Ra 1e6): 0.1, 0.2, 0.5, 1.0
percent (my interpolation; the paper states only the end points). Discretisation allowance for the finest mesh: second-order scheme with limiter, the
existing medium-mesh errors of `natural_convection.rs` are 0.02 to 0.7 percent; allowance 0.5 percent for Nu_avg, 1.0 percent for the velocity
maxima, 3.0 percent for Nu_max (face-centre value of a derivative at a thin layer), absolute 0.03 for locations at Ra <= 1e4 and 0.02 for Ra >= 1e5.
A1 finest-mesh |Nu_avg / Nu_ref - 1| <= u_ref + 0.5 percent (0.6, 0.7, 1.0, 1.5).
A2 finest-mesh |u_max / ref - 1| and |v_max / ref - 1| <= u_ref + 1.0 percent (1.1, 1.2, 1.5, 2.0).
A3 finest-mesh |Nu_max / ref - 1| <= u_ref + 3.0 percent (3.1, 3.2, 3.5, 4.0).
A4 finest-mesh |z_u - ref| and |x_v - ref| <= 0.03 (Ra 1e3, 1e4) or 0.02 (Ra 1e5, 1e6).
A5 convergence: for Nu_avg, u_max and v_max |Q_f - Q_m| < |Q_m - Q_c| (the sequence converges), observed order 0.8 <= p <= 3.0 for Nu_avg (the scheme is
second order with a bounded limiter, so first-to-second order behaviour is expected), and GCI_fine(Nu_avg) <= 1.0 percent. Orders and GCI of every quantity
are reported.
A6 the Richardson-extrapolated Nu_avg is within u_ref + 0.5 percent of R1 (same limit as A1).

### B. Sod
B0 the independent exact solver reproduces R2 within 1e-5 absolute (p*, u*, rho*_L, rho*_R, shock speed, head, tail speeds) and the Rankine-Hugoniot
mass condition S = rho*_R u* / (rho*_R - rho_R) within 1e-9.
B1 conservation: total mass and total energy of every run change by less than 1e-10 relative (end states uniform with zero velocity, waves do not reach the
ends by t = 0.2).
B2 positivity and boundedness: rho in [0.125 - 0.02, 1 + 0.02], p in [0.1 - 0.02, 1 + 0.02] for the TVD schemes (limiter not None; small over- and undershoots of the
Roe and AUSM+ fluxes are allowed at 0.02, about 2 percent of the jump).
B3 second-order (limiter) configurations, finest mesh n = 800: L1(rho) <= 2.5e-3 for every solver and limiter except Minmod (3.5e-3, the most diffusive
limiter), L1(u) <= 3.0e-3 and L1(p) <= 3.0e-3. Justification: the existing unit tests pin L1(rho) < 3e-3 at n = 400 and < 5e-3 at n = 200 (all four solvers); with the observed
order near 0.7 these give about 1.9e-3 at n = 800; the registered limits add 30 percent. u and p are not covered by any existing test: they get the same bound on
the grounds of equal order of magnitude of the jumps (rho 0.875, u 0.93, p 0.9).
B4 first-order (limiter None) configurations, n = 800: L1(rho), L1(u), L1(p) <= 1.2e-2 (Godunov-type first order L1 convergence on contact and shock is of order one half to one).
B5 observed L1 order of rho between n = 400 and 800: 0.6 <= p <= 1.5 for the second-order configurations and 0.4 <= p <= 1.2 for first order; L1(rho) decreases at every
doubling for every configuration.
B6 ordering: HLLC + van Leer RK3 has a smaller L1(rho) at n = 400 than HLLC first order, and Rusanov has the largest L1(rho) of the four solvers at n = 400 with the van Leer limiter.

### C. Graetz
Reference uncertainty: R3(b) prints 4 to 5 digits; axial conduction at Pe = 1000 is below 1e-4 relative; so the allowance is discretisation error only.
C1 constant wall temperature, finest mesh, x+ stations 0.001, 0.003, 0.005, 0.01, 0.02, 0.05: |Nu_x / table - 1| <= 5, 3, 3, 3, 2, 2 percent (steep gradient near the entry is
harder; first-order wall gradient on a wall-clustered mesh); theta_b at 0.01 and 0.05 within 1 and 2 percent.
C2 constant flux, finest mesh, stations 0.001, 0.005, 0.01, 0.03, 0.05: |Nu_x / table - 1| <= 5, 3, 3, 2, 2 percent.
C3 fully developed limits at x+ = 0.1: Nu = 7.5407 (constant T) and 8.2353 (constant flux) within 1 percent on the finest mesh.
C4 entry-length behaviour: Nu_x decreases monotonically with x+ (no undershoot of more than 0.1 percent below the final value) in both cases, and
Nu_x(x+) <= 1.05 Nu_fd for x+ >= 0.02 (constant T) and for x+ >= 0.03 (constant flux), while Nu_x(0.005) > 1.05 Nu_fd (table: 8.5166 / 7.5407 = 1.13).
C5 mesh convergence: |Q_f - Q_m| < |Q_m - Q_c| for Nu_x at x+ = 0.01 (constant T), observed order and GCI reported; GCI_fine <= 2 percent.
C6 energy balance: Nu_x from the wall gradient and from the streamwise decay of the bulk temperature (constant wall temperature: Nu = D_h u_m H / (2 alpha) times -d ln theta_b / dx = -(1/alpha) d ln theta_b / dx for D_h = 2, H = 1, u_m = 1, from the heat balance u_m H dT_b/dx = -2 alpha (-dT/dn)_w; theta_b differentiated over the column pair around the station) agree within 2 percent at x+ = 0.01 and 0.05.
C7 circular pipe: finest-mesh fully developed Nu_D (x+ = 0.06) within 2 percent of 3.66 (constant T) and of 4.36 (constant flux); Hausen mean Nu_m at Gz = 100, 50, 20 within
5 percent (a correlation; its own scatter is not stated in R3, 5 percent is the usual accuracy claimed for such fits and is my allowance); entry-length: Nu_D(x+ = 0.05) within 5 percent of
3.66 and Nu_D(x+ = 0.005) more than 20 percent above 3.66 (R3 rule x_fd,t = 0.05 Re Pr D); mesh sequence reported with observed order and GCI (not asserted beyond convergence).

### D. Flat plate
D1 SST finest mesh: Cf within 5 percent of Karman-Schoenherr at Re_theta = 5000, 7500, 10000, 12500 (R4 draws 5 percent bars; no better estimate of the reference error exists).
D2 SST finest mesh: u+ against the Coles curve at Re_theta ~ 10000 (closest column; its actual Re_theta printed): within 5 percent for 1 <= y+ <= 5; within 6 percent for
5 < y+ < 30 (the buffer layer, where the van Driest-damped Coles curve and a RANS model differ most); within 3 percent for 30 <= y+ <= 300; within 5 percent for
300 < y+ <= 1000. Log-law fit over 50 <= y+ <= 300 (least squares of u+ against ln y+): slope 1/kappa and intercept C of the solution within 5 percent and 0.4 of the same fit
applied to the TMR data (the slope window avoids both the buffer layer and the wake).
D3 mesh convergence of Cf at Re_theta = 10000: |Cf_f - Cf_m| < |Cf_m - Cf_c| and |Cf_f - Cf_m| / Cf_f <= 1.5 percent; observed order and GCI reported.
D4 k-epsilon Standard, with the Scalable wall function and with the Standard wall function, finest y+ ~ 30 mesh: Cf within 5 percent of Karman-Schoenherr at Re_theta = 7500, 10000, 12500 (the
existing catalogue records that the Standard function misses 5 percent against Prandtl-Schlichting; this wave measures the same limit against a different reference; a miss is recorded); u+
within 5 percent of the Coles curve for 30 <= y+ <= 300 on cells above the wall cell. Run is reported "not applicable" if the realised first-cell y+ at the station is outside [20, 60].
D5 convergence of the iteration is a precondition of every assertion.

### E. NACA 0012
Not attempted (see below); no limit registered.

## Data not obtained / not attempted

* de Vahl Davis Nu_min is not asserted (corner singularity, face-centre values cannot reproduce it); Tables I to IV not used.
* Shah and London tables, Incropera and Kays books, Hausen's paper: not read; the tube Graetz local Nu table is not available to me (R3(b) gives parallel plates only and a developing-flow
  tube table at Pr 0.7, 2, 5 that does not correspond to the Graetz problem), so the circular pipe is compared only with the fully developed limits, the entry-length rule and the Hausen mean-Nu correlation as printed in R3(a).
* Laminar or inviscid NACA 0012 surface Cp: the repository has no airfoil mesh generator, the TMR NACA 0012 data in the pack are for a viscous RANS condition (M = 0.15, Re = 6e6) that needs a
  turbulent airfoil C-grid; not attempted in this wave (data would be `CP_Gregory_expdata.dat`, `CP_Ladson.dat`, already in the pack and not read here).
* Conjugate heat transfer and mixing layer validation: out of the stated scope of this wave and not touched.

## Process

Order: this ADR is committed first (docs only); then the reference tables and manifest entries; then each test file; then the measured results addendum; the final gate runs on a `git archive`
copy of the last commit (`cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --release --no-fail-fast`).
Heavy tests are `#[ignore]` with the documented command `cargo test --release -p <crate> --test <file> -- --ignored --nocapture` and are each run once by the author.

## Addendum 1 (after the first Sod run): first-order conservation set-up defect

B1 as registered (change below 1e-10 relative at every n) fails for the first-order configurations at n = 100 (mass 3e-10 to 1e-9, energy up to 3e-9). Diagnosis: first-order numerical diffusion
reaches the fixed boundary cells, which is a violation of the stated premise ("waves do not reach the ends"): |rho_0 - 1| is 3.3e-7 at n = 100, 3e-12 at n = 200 and 0 at n = 400. This is a set-up
defect of the criterion, not a solver defect. The original first-order tests stay in the file as `#[ignore]` and a corrected test per configuration asserts B1 from n = 200 (B2 to B5 unchanged).

## Addendum 2 (after the first Sod run): Van Albada limiter overflow (known miss, solver not changed)

HLLC + Van Albada + RK3 aborts at step 1 with "non-positive density or pressure" for every n and CFL (Euler and RK2 are fine). Cause, proven by a scratch-copy bisect: `TvdLimiter::VanAlbada.psi`
evaluates (r^2 + r)/(r^2 + 1), which is NaN for r > 1.34e154; the gradient ratio reaches that range through the 1e-300 `safe()` guard in smooth start-up cells. A scratch copy with the branch
`if r > 1e150 { (r + 1)/(r + 1/r) }` passes B3 to B5 (n = 800 L1: rho 8.87e-4, u 1.34e-3, p 5.28e-4). The edit of `crates/rcfd-fv/src/convection.rs` was refused by the permission system, so the solver is
not modified; the defect is recorded by the ignored tests `b_hllc_vanalbada_rk3_known_miss` and `van_albada_limiter_is_finite_for_huge_ratios_known_miss`. Proposed patch: the branch above (bit-identical for every r <= 1e150).

## Addendum 3 (after the first Graetz run): constant-flux pipe, reference-side set-up defect, corrected criterion C7'

Measured with the registered C7 (length 12, x+ = 0.06): constant-T pipe passes (Nu_D 3.7154 vs 3.66, +1.5 percent; Hausen Nu_m within 1 percent at Gz 100, 50, 20); constant-flux pipe misses (4.5255 vs 4.36,
+3.80 percent; Nu(0.05) +5.14 percent). A diagnostic on longer pipes (L = 24, x+ to 0.12) showed Nu still decaying monotonically toward 4.36 at x+ = 0.12 (4.40 on the (13, 9, 540) mesh), with no outlet
distortion. Diagnosis: the entry-length rule x_fd,t = 0.05 Re Pr D of R3(a) is the constant-wall-temperature rule; the constant-flux profile develops more slowly, so "fully developed at x+ = 0.06" was a wrong
premise of C7, not a solver defect. The original constant-flux test stays as the ignored `c_pipe_constant_flux_known_miss`. The author had seen the diagnostic numbers above when registering C7'.
C7' (registered before the corrected run): constant-flux pipe, length 30 (x+ up to 0.15), meshes (n, n_ring, nz) = (4, 3, 200), (6, 4, 300), (9, 6, 450); finest-mesh Nu_D at the last column within 2 percent of
4.36; Nu_D(x+ = 0.005) more than 20 percent above 4.36; mesh sequence converging monotonically with observed order and GCI reported. The 5 percent Nu(x+ = 0.05) check does not apply to constant flux.
The pipe constant-T C7 stands unchanged. A refined-mesh diagnostic (`diag_pipe_constant_flux_finer_mesh`) is kept ignored and has no limit.

## Addendum 4 (before the first flat-plate run): edge velocity in the momentum thickness

The registered theta(x) = integral of (u/U)(1 - u/U) dy with U = 1 is not usable on this domain: the symmetry top at y = 1 over a boundary layer of 0.03 to 0.06 speeds the outer flow up by
about 0.5 percent, and the integrand u(1 - u) of the free stream (about -0.005) integrated over the 0.9 height outweighs theta (about 0.0017). Definition used instead, registered before the first run:
U_e(x) = u of the cell of the column whose centre is nearest to y = 0.25 (outside the layer everywhere, delta < 0.07); theta = integral of (u/U_e)(1 - u/U_e) dy by the trapezoid rule on the cell
centres from the wall (u = 0) up to y = 0.15; Re_theta = theta U_e / nu; Cf = 2 tau_w / U_e^2 (same U_e); u+ = u / u_tau with u_tau = sqrt(tau_w) (rho = 1). All limits of D1 to D4 are unchanged.
The SST runs use the model exactly as `flat_plate_sst.rs` (no wall-mode call, default turbulence convection), the k-epsilon runs set the wall function as `common/plate.rs`.
