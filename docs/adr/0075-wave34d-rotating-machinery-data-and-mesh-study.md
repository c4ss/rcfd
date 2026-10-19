# ADR 0075: Wave 34D, published rotating-machinery data, the F-126 acceptance attempt, and a registered mesh study of the rotating-frame checks

Status: accepted. Everything under "Registered" was written and committed before any of the new test code was written or run and before any
solver run on a published rotating-machinery case. Limits are not changed, widened or deleted after a run; a miss becomes an ignored
`*_known_miss` test with the limit intact, a defective set-up gets a NEW corrected test and the old one stays ignored. Baseline hashes are never
re-recorded. Clean-room: first principles and published literature only; every reference value is cited, anything recalled from memory is marked
UNVERIFIED. Parent: ADR 0071 (wave 33D, addendum: F-126 not attempted, C2/C3 finest-mesh limits registered without a mesh study and missed,
C4 spurious radial/axial speed missed), ADR 0055/0059 (F-126 frozen rotor / mixing plane), ADR 0065/0068 (fan sector meshes), `docs/milestone-fan.md`.

## Registered

### Part A: reading published rotating-machinery data

For each source found the record (in `docs/validation-data.md` and, where it is a dataset, `validation-data/manifest.json`) states: full citation;
URL; what was ACTUALLY READ (the document or page opened and the relevant content read) versus only SEEN CITED (a reference in another paper, a
search snippet; never upgraded to "read"); geometry completeness (which geometric data are public, where); operating-condition data and measured
performance numbers actually read (table or figure; a value read off a plot is marked as such with its reading uncertainty); licence or terms of
use; whether the existing solver (incompressible SIMPLE/SIMPLEC with `rotating_frame` / `mrf`, mixing plane, rotational-periodic sector, SST) and
the `rcfd_mesh::fan` generators could in principle run it, and if not, why.

Vendoring rule: a file is added to the manifest with an upstream URL and checksum only when its licence explicitly permits redistribution or it is a
US-government work marked for public use; otherwise the dataset is listed with no files and status `unresolved` (as `hannover-compressor`).
`validation-data/data/` is git-ignored: downloaded documents are never committed. Numbers transcribed from a document into an in-repo CSV carry
the citation, the table/figure and the page.

Candidate ranking, criteria fixed now (applied in this order; a candidate failing (1) or (2) is "not runnable as is"):
1. Regime: incompressible (relative tip Mach number below 0.3), because the only rotating-frame flow solver is the incompressible pressure-based
   one; the compressible solver is explicit Euler without rotation, viscosity or turbulence.
2. Geometry public and representable: blade sections on cylinders, NACA 4-digit or double circular arc, cylindrical hub of constant radius, a
   shroud with a radial tip gap, no stator / bellmouth / spinner that the published performance depends on and the generator cannot build.
3. Measured quantity defined: which pressure rise (total-to-static, total-to-total, static-to-static), between which planes, how efficiency is
   defined, numeric values readable with a stated or estimable uncertainty.
4. Licence permits use of the numbers (published numbers may always be compared with; vendoring follows the rule above).
Ties: fewer blade rows, then simpler geometry.

### Part B: F-126 acceptance attempt on the best-fitting case

F-126 acceptance (roadmap, restated in ADR 0071): a public low-speed compressor or rotor case reproducing the overall pressure rise (pressure ratio)
and efficiency within published scatter. Outcome classes, fixed now, exactly one is reported:
* NOT RUNNABLE: no candidate passes criteria (1) and (2), or the generator rejects the published geometry (located error, fold), or a quantity the
  comparison needs cannot be computed. The report states exactly what would be needed.
* NOT CONVERGED: the run reaches its iteration cap without the registered convergence criterion, or diverges / stops on the divergence guard.
* MISSED: converged, at least one registered limit exceeded.
* MET: converged and every registered limit met.
Rules fixed now: the case is chosen by the ranking above after reading the sources; the numeric limits, the operating point(s), the compared
quantities, the turbulence treatment and the mesh are registered in a dated amendment of this ADR BEFORE any solver run on that case. The mesh is the
coarsest meaningful one by a rule fixed in that amendment (not by trying meshes). The run is done once per registered set-up. A published fan rebuilt
with the generator and compared with measurement is a validation result of that one comparison; no fan performance claim is made beyond it, and none
from any generator-only fan.

### Part C: registered mesh study of the rotating-frame checks (replaces the ADR 0071 finest-mesh limits)

Disclosure, fixed now: the ADR 0071 max-norm errors on the three meshes h, h/2, h/4 are KNOWN (Taylor-Couette 9.76e-2, 2.61e-2, 6.76e-3 V_i, orders
1.90, 1.95; Ekman against the infinite-depth spiral 4.04e-2, 1.30e-2, 3.58e-3 U_g, orders 1.64, 1.86; the `rotating_frame` spurious speed of the
C2a runs 7.9e-2, 2.4e-2, 6.4e-3 V_i). An order window on that triple passes by construction and is not counted as evidence. The blind part of this
study is therefore a FOURTH mesh h/8 that has never been run, and the Richardson analysis is made on the triple (h/2, h/4, h/8). Spacing halves in
every direction in which the solution varies (the azimuthal and radial directions of the annulus, the wall-normal direction of the Ekman box); the
direction(s) of invariance keep their cells (annulus z: 2 cells between symmetry planes; Ekman x and z: 2 cells, periodic).

Reference solutions (exact for the solved problem):
* Taylor-Couette (unchanged from ADR 0071, exact for the finite annulus): `u_theta = A r + B/r`, `A = -W_i r_i^2/(r_o^2 - r_i^2)`,
  `B = W_i r_i^2 r_o^2/(r_o^2 - r_i^2)`.
* Ekman layer, CORRECTED to the finite height of the box. ADR 0071 compared with the infinite-depth spiral; the box has the velocity fixed to
  `(U_g, 0, 0)` at `y = H = 8 delta`, where the infinite-depth solution is still off by about `e^{-8} = 3.4e-4 U_g` (about 10 % of the N = 64
  error, comparable to the discretisation error at N = 128). Derivation: with `V = (u - U_g) + i w`, ADR 0071 gives `V'' = lambda^2 V`,
  `lambda^2 = -2 i Omega/nu`, `lambda = (1 - i)/delta`, `delta = sqrt(nu/Omega)`. The conditions are `V(0) = -U_g` (no slip) and `V(H) = 0` (the
  fixed top). The solution of the two-point problem is `V(y) = -U_g sinh(lambda (H - y)) / sinh(lambda H)` (it satisfies the ODE, `V(0) = -U_g`,
  `V(H) = 0`; as `H -> inf` it tends to `-U_g exp(-lambda y)`, the ADR 0071 spiral). Then `u = U_g + Re V`, `w = Im V`.

Functionals (for Richardson extrapolation; max norms cannot be extrapolated):
* R1 (full annulus, API, `rotating_frame` at `Omega = W_i`, the C2a corrected set-up): `Q = sum_c u_theta,abs(c) V_c / (2 pi r_c lz)`, which
  approximates `int_{r_i}^{r_o} u_theta dr`; exact `Q_ex = A (r_o^2 - r_i^2)/2 + B ln(r_o/r_i)`. The polygonal cells add their own `O(h^2)` quadrature
  error (stated, part of the measured error).
* R2 (60-degree periodic sector, fv level, the C2b set-up): the seam volume flux `m_lo = sum of phi over theta_min` (the C5a quantity), exact
  `depth * int (u_theta - Omega r) dr`.
* R3 (Ekman box, fv level, the C3b set-up): the two transports per unit width `T_u = sum_c (u_c - U_g) dy`, `T_w = sum_c w_c dy` (averaged over the
  2 x 2 columns), exact `int_0^H V dy = -U_g (cosh(lambda H) - 1) / (lambda sinh(lambda H))`, `T_u = Re`, `T_w = Im` of it.

Meshes: R1 (nx, ny, nz) = (12, 4, 2), (24, 8, 2), (48, 16, 2), (96, 32, 2); R2 (n_theta, n_r) = (2, 4), (4, 8), (8, 16), (16, 32); R3 N = 16, 32, 64, 128.
Convergence (required on every mesh; a run that is not converged is a miss of the check): R1 `tolerance_u = tolerance_p = 1e-10`, cap 20000 iterations;
R2 `res_u < 1e-10` and continuity `< 1e-9`, cap 20000; R3 `max_du < 1e-11` and continuity `< 1e-9` (the C3b criterion), cap 400000.
Iteration-error check I1: after convergence on the two finest meshes, 500 further iterations change each functional by at most `1e-2 |f_{h/4} - f_{h/8}|`
and the max-norm error by at most `1e-2 e_{h/8}`.

Limits, the same for R1, R2 and R3 (for R3 for each of `T_u`, `T_w`), with `f1, f2, f3` the functional on h/2, h/4, h/8 and `f_ex` the exact value:
* L1 monotone convergence: `rho = (f2 - f3)/(f1 - f2)` lies in (0, 1); otherwise the functional is outside the asymptotic range and the check is a MISS (not
  reinterpreted).
* L2 observed order `p = log2(1/rho)` in [1.7, 2.3] (the formal order is 2: central/linear-upwind convection, linear interpolation, midpoint quadrature).
* L3 Richardson-extrapolated error against the exact value: `f_RE = f3 + (f3 - f2)/(2^p - 1)`, `|f_RE - f_ex| <= 0.2 |f3 - f_ex|` (extrapolation removes at least
  80 % of the finest-mesh error).
* L4 error estimate brackets the true error: the fine-grid convergence index `GCI = 1.25 |f3 - f2| / ((2^p - 1) |f3|)` satisfies `|f3 - f_ex| / |f_ex| <= GCI`.
Max norm `e_k = max over cells` of the ADR 0071 error measure (R3 now against the finite-height solution), meshes h ... h/8:
* L5 blind prediction of the h/8 error from the two finest known meshes: `e_pred = e_{h/4}^2 / e_{h/2}`, limit `|e_{h/8}/e_pred - 1| <= 0.2`. (For R3 `e_{h/2}`,
  `e_{h/4}` are re-measured against the finite-height solution in the same run; they are close to, not equal to, the ADR 0071 values.)
* L6 order of the finest pair `log2(e_{h/4}/e_{h/8})` in [1.7, 2.3]; every earlier pair `>= 1.4`.
* L7 the ADR 0071 finest-mesh limits restated on the mesh the study says is needed: `e_{h/8} <= 5e-3 V_i` (R1, R2) and `<= 2e-3 U_g` (R3).
The ADR 0071 tests `c2a_corrected_finest_error_known_miss`, `c2b_finest_error_known_miss`, `c3b_ekman_finest_error_known_miss` stay ignored and unchanged.
R3 also keeps the ADR 0071 spiral-amplitude criterion on h/8 (max |w| within 5 % of the analytic maximum, now of the finite-height solution).

R4, the spurious radial/axial speed of the C4 frame-invariance runs (the C4 corrected set-up: (a) inertial, (b) `mrf` `Omega = W_i`, (c) `rotating_frame`
`Omega = W_i`, (d) `rotating_frame` `0.5 W_i`, (e) `mrf` `0.5 W_i`) on the four R1 meshes. The exact value is zero, so the measured speed is itself the
error against the exact solution. Only the (48, 16, 2) values and the C2a `rotating_frame` sequence are known; the (a), (b), (d), (e) sequences have never
been run. Limits per run: L6 as above (finest pair in [1.7, 2.3], earlier pairs `>= 1.4`; a finest-mesh value below `1e-7 V_i` counts as an order pass).
Reported, no limit: the location (r, z) of the maximum on each mesh, and the mesh on which the ADR 0071 limit `1e-3 V_i` would be met if the
observed order holds. Convergence as R1. The ADR 0071 test `c4_corrected_spurious_speed_known_miss` stays ignored and unchanged.

Code policy: tests only. New files `crates/rcfd-api/tests/rotating_mesh_study.rs` (R1, R4) and `crates/rcfd-fv/tests/rotating_mesh_study.rs` (R2, R3);
the ADR 0071 test files are not edited. A miss stays as an ignored `*_known_miss` test with the limit unchanged.

## Amendment 1 (2026-10-19): Part A reading outcome, the chosen case, and the Part B registration

Written after the sources were read and after the Part C runs, BEFORE any mesh of the chosen case was built and before any solver run on it.

### Part A, sources and ranking (details in `docs/validation-data.md`, section "Rotating machinery")

| Candidate | Read / seen | Regime | Geometry public | Representable | Rank |
|---|---|---|---|---|---|
| FAN-01 (FAU/EAA low-pressure axial fan; Zenodo 10.5281/zenodo.20037409, CC BY 4.0; Zenger et al. SAE 2016-01-1805) | data files read (characteristic .mat, CAD IGES, LDA .mat, rig PDF); Schoder et al. Acta Acustica 4 (2020) 22, Schoder and Czwielong arXiv:2211.12014, Junger 2019 thesis, Vourakis and Karlsson FAN 2022 read; the SAE paper NOT read | Ma_tip 0.113, incompressible | full CAD (CC BY 4.0) | rotor only, 9 blades, sections close to NACA 4-digit on cylinders, cylindrical hub r 124 mm, duct r 250 mm, tip gap 2.5 mm; NOT the bellmouth, finite blunt hub, 3 mm root fillet, struts, motor, diffuser | 1 |
| NASA LSAC (Wasserbauer et al., NASA TM-4635, 1995) | TM read (Table I) | low speed | no section coordinates in the TM; a "publicly available" package is mentioned elsewhere, not found | 4 stages + IGV + stators | not runnable |
| DTMB 4119 propeller (via arXiv:2104.13363, Tables 1-3 read; the DTNSRDC reports not read) | secondary read | incompressible | tables public | NACA 66 (mod) thickness, a = 0.8 mean line, open propeller, no shroud | not runnable as is |
| NASA Rotor 37 (Reid and Moore, NASA TP-1337, read; the manifest copy) | read | inlet relative Mach 1.48 tip / 1.13 hub | Appendix C coordinates (MCA sections) | transonic | not runnable |
| NASA Rotor 67 (Strazisar et al., NASA TP-2879, design text pp. 1-12 read) | read in part | relative tip Mach 1.38 | Table I coordinates | transonic, contoured hub | not runnable |
| Daily and Nece 1960, ERCOFTAC rotating cases, Dring/UTRC rig | seen cited / snippets only | - | - | not fans (or a turbine stage) | not candidates |

### Part B registration: FAN-01 rotor in a straight annular duct, F-126 acceptance attempt

Geometry (generator terms): `n_blades = 9`, `r_hub = 0.124`, `r_tip = 0.2475`, `tip_clearance = 0.0025` (shroud r = 0.250), NACA 4-digit family, stations from the
cylindrical sections of the CC BY 4.0 CAD (extraction by a pure-Python IGES/NURBS evaluator written for this wave; the sections are close to, not exactly, NACA
4-digit: camber line within 0.0016 c of the NACA parabola with m = 0.0365-0.0376 at p = 0.515-0.53, maximum thickness 0.109-0.111 c at 23 % chord, which a NACA
4-digit thickness form puts at 30 %; mean-line metal angles checked: the leading edge is more tangential than the trailing edge (58 vs 42 deg from the axis at r = 140 mm),
i.e. positive camber in the generator convention for rotation about +z). Stations at r = 124 (linear extrapolation of 127/130), 127, 130, 140, ..., 240, 245, 247.5 mm
(extrapolated from 245/247): chord, stagger from the axis, camber, camber position and thickness as tabulated in `docs/validation-data.md`; sweep = x_LE(r) - x_LE(124),
lean = r (theta_LE(r) - theta_LE(124)). The published nominal design (NACA 4510, chord 103 / 58 mm) is NOT used: the as-built CAD is 4-5 % shorter in chord. Domain: inlet
plane 100 mm ahead of the hub leading edge (`z_le_root = 0.1`), `axial_extent = 0.4`; the hub is a cylinder over the whole domain.

New input needed (code, opt-in): the case hook only has a hub and a tip station with linear variation; the stagger of FAN-01 varies from 43.6 to 69.6 deg and the chord
from 99 to 55 mm non-linearly (a two-station fit is about 4 deg / 11 mm off at mid-span). Optional lists `mesh.fan.station_radius`, `station_chord`, `station_stagger`,
`station_camber`, `station_camber_pos`, `station_thickness`, `station_sweep`, `station_lean` are added; when `station_radius` is non-empty they replace the hub/tip values.
Registered checks: H1 a two-station list equal to the hub/tip values gives a mesh identical (node coordinates bit-for-bit, same cells) to the hub/tip form; H2 lists of
unequal length, or a first/last radius not at r_hub / r_tip, give a located error `mesh.fan.station_*`; H3 the existing fan examples are unchanged (no default changes).

Mesh, the coarsest meaningful one by a rule fixed now: `topology = "c_grid"` with the counts of the example `cases/axial_fan_cgrid` (n_up 8, n_chord 16, n_down 8,
n_side 10, n_nose 4, n_core 1, front_gap 0.3, n_span 14, n_gap 7, n_bl 2, growth 1.2), first cell from the generator's y+ estimate with `y_plus = 30`,
`ref_speed = 30.46` m/s (relative speed at mid-span, r = 0.186 m, at the design point), `nu = 1.5e-5`. If the C-grid generator returns an error, the H-slit topology with the
counts of `cases/axial_fan_sector` (n_up 8, n_chord 12, n_down 8, n_side 10, n_mid 1, n_span 16, n_gap 7, n_bl 2, growth 1.2, same first-cell rule, default truncation) is
used instead; if both fail the outcome is NOT RUNNABLE. No other mesh is tried.

Physics and solver: air `nu = 1.5e-5 m^2/s`, `rho = 1.2 kg/m^3` (ASSUMED: the data set gives no density or ambient state; UNVERIFIED), rotational speed 1486 rpm
(`Omega = 155.62 rad/s`, from the papers; the data files carry no speed), `rotating_frame` source about +z with centrifugal terms (the `mrf` source is refused on this mesh,
ADR 0065 S4), `k_omega_sst` with the automatic wall treatment, fully turbulent. Boundaries (velocities in the rotating frame): inlet fixed `u = (Omega y, -Omega x, U_in)`
(uniform axial absolute inflow, no swirl), turbulence `values` with k = 1.5 (0.01 U_in)^2 and omega = sqrt(k)/(Cmu^0.25 l), l = 0.07 D_h = 0.0176 m; outlet fixed static
pressure 0, zero-gradient velocity; hub and blade no-slip (rotating with the rotor); shroud at rest in the absolute frame, `u = (Omega y, -Omega x, 0)`,
`turbulence = "wall"` (the automatic role would call a moving wall an inlet). SIMPLEC, `linear_upwind`, up to 8000 iterations. Converged = `tolerance_u = tolerance_p = 1e-5`
reached, OR at the cap: torque and Delta p_ts change by less than 0.5 % over the last 1000 iterations and `|m_in + m_out| <= 1e-3 |m_in|`. Otherwise the outcome is NOT CONVERGED.

Operating points (measured values read from `characteristic_n1ug.mat` of the Zenodo record; torque derived here as P/Omega): design Q = 1.4032 m^3/s (Delta p_ts 126.46 Pa,
shaft power 333.30 W, M = 2.142 N m, eta_ts 0.532); off-design Q = 1.8039 m^3/s (63.9 Pa, 292.9 W, 1.882 N m, 0.394). U_in = Q / (pi (0.250^2 - 0.124^2)) = 9.478 and
12.185 m/s. The stall region (Q below about 1.2 m^3/s) is excluded.

Computed quantities: M = minus the moment about +z of the fluid on the `blade` and `hub` patches (pressure and viscous), times 9 (the shroud is stationary in the rig, its
moment is not shaft torque); Delta p_ts = (area-averaged static pressure on the outlet) - (area-averaged static pressure on the inlet + rho U_in^2 / 2) (the measured quantity
is the inlet-chamber total pressure to the ambient static pressure); eta_ts = Q Delta p_ts / (M Omega) (the definition of Schoder et al. 2020). Delta p_tt (outlet mass-averaged
absolute total pressure minus inlet total) is reported, no limit.

Limits (F-126 acceptance, "within published scatter"; the FAU uncertainty is not in anything read, the scatter used is the round robin of Vourakis and Karlsson 2022: +-3 Pa,
inter-laboratory agreement about 3 %): at each operating point `|Delta p_ts - measured| <= 3 Pa + 0.03 measured`, `|M/M_measured - 1| <= 0.05`, `|eta_ts - measured| <= 0.03`.
MET only if both points meet all three. Modelling differences that are part of the measured deviation, stated now: no bellmouth, no finite blunt hub, no root fillet,
no struts/motor/diffuser and no exit loss to ambient (the measured Delta p_ts is an installation value); uniform outlet pressure (no radial equilibrium); fully turbulent RANS
at chord Reynolds numbers 1.25e5-1.5e5; NACA 4-digit thickness form instead of the CAD thickness; assumed density; bearing friction in the measured torque (not stated
whether subtracted). Whatever the outcome, no fan performance claim is made beyond this comparison.
