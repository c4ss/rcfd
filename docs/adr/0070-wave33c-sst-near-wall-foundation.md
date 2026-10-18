# ADR 0070: Wave 33C, SST near-wall foundation (term audit, flat-plate and channel checks, grid convergence)

Status: registered before any rcfd run or code change of this wave (this file is committed first; results go into an addendum, limits are never widened).
Branch main, written at HEAD 38ccbaa.

## Open items from ADR 0061, 0063, 0064

(1) The wall-resolved SST log-law slope over y+ 50 to 300 measures 2.78 (kappa_eff 0.36) where "1/kappa = 2.44" had been expected.
(2) Cf(x = 0.97) of the finest rcfd mesh was reported as +3 percent (Richardson value) against the TMR CFL3D value 0.00268299.
(3) Cf below Karman-Schoenherr (K-S) by 4 to 9 percent with the profile-based Re_theta of ADR 0061; ADR 0063 found the registered Re_theta biased low.
ADR 0063 concluded "no defect isolated" but left (1) and (2) "unexplained" and "ambiguous".

## Sources actually read in full for this wave (curl of the raw HTML, tags stripped, equation images read through their alt text), 2026-10-18

* https://tmbwg.github.io/turbmodels/sst.html (NASA TMR, SST, SST-2003 and variants): all equations, F1, F2, CD floor, limiter, nu_t, constants, BCs. READ IN FULL.
* https://tmbwg.github.io/turbmodels/flatplate.html (2DZP intro) and flatplate_sst.html (SST-Vm results): READ IN FULL. New facts used here that ADR 0063 did not have:
  (a) the TMR case is COMPRESSIBLE (M = 0.2): "if you run this case with an incompressible code, your results may be close - but not quite the same - as the grid is refined" (intro page);
  (b) the page links incompressible SST results at x = 0.97008 (`cf_incomp_results_sstv.dat`) and the CFL3D SST u+(y+) data at x = 0.97008 and 1.90334, Cf(x) of the 545 x 385 grid, and the Cf convergence of CFL3D, FUN3D;
  (c) the page states that the SST and SST-V/SST-Vm results are nearly identical for this case.
* https://tmbwg.github.io/turbmodels/channelflow_val.html and channelflow_val_sst.html (TMR 2DFDC, fully developed channel at Re 80 million, SSTm, CFL3D 161 x 513): READ IN FULL (text and links; plots only as alt text). The page states: "the value of kappa produced by this turbulence model is close to 0.410 over much of the log layer" (Karman measure KM). Data file `sst_u+y+KM_cfl3d.dat` downloaded.
* Data files downloaded and used (public, CC0 repository TMBWG/turbmodels via the page links): `flatplate_u+y+_sstv.dat`, `cf_plate_sstv.dat`, `cf_convergence_sstv.dat`, `cf_incomp_results_sstv.dat`, `sst_u+y+KM_cfl3d.dat`.
NOT read: Menter (1994) AIAA J. 32:1598, Menter, Kuntz, Langtry (2003), Wilcox (2006), Menter (1992) NASA TM 103975, Menter (1993) AIAA 93-2906. Everything attributed to them below is as transcribed on the TMR SST page (those papers are UNVERIFIED as primary text). The TMR plots (images) were not seen, only their data files.

Pre-registration disclosure: before this ADR was written I processed the downloaded reference tables (not rcfd output) and know the following reference-side facts, which motivate checks P2 and P3 below:
* TMR SST data, log fit of u+ versus ln y+ over y+ 50 to 300: slope 2.7784 at x = 0.97008 and 2.7447 at x = 1.90334 (kappa_eff 0.360 and 0.364); local slope y+ du+/dy+ 2.79 at y+ 100, 2.71 at 200, 2.74 at 300 (x = 0.97008). rcfd (ADR 0063) measured 2.7766 on the same window.
* TMR channel SST (CFL3D), Karman measure: 0.366 at y+ 100, 0.395 at 316, 0.406 at 994, a plateau of 0.409 to 0.410 for y+ about 3000 to 10000; log fit of u+ over 50 to 300 gives slope 2.693 (kappa_eff 0.371), over 300 to 3000 slope 2.469, over 1000 to 10000 slope 2.443.
  So the expectation 1/kappa = 2.44 holds only for y+ > about 1000; the window 50 to 300 sits in the model's own approach to its log layer.
* TMR incompressible SST Cf(x = 0.97008) on the 545 x 385 grid: FUN3D 2.71736e-3, SC/Tetra 2.72119e-3, GHOST 2.71085e-3 (mean 2.71647e-3), against compressible CFL3D 2.69085e-3: the compressible reference is 0.95 percent below the incompressible one.
* TMR CFL3D Cf(x) with Re_theta = int_0^x Cf/2 dx (trapezoid, Re = 5e6 per unit length, nu = 2e-7) compared with K-S at the same Re_theta: -3.61, -2.76, -2.16, -1.69 percent at Re_theta 5000, 7500, 10000, 12500 (x = 0.582, 0.940, 1.320, 1.715).
These are literature facts; no rcfd result of this wave was seen when the limits below were written. The earlier rcfd numbers from ADR 0063 are known to me (finest baseline Cf(0.97) 2.6984e-3, coarse/medium 2.5770/2.6514e-3, slope 2.7766, observed order 0.66) and the limits below were written knowing them; where a limit is expected to be missed because of them it is said so.

## Part 1: term-by-term audit of `crates/rcfd-turb/src/sst.rs` against sst.html (SST-2003 branch), as unit tests

The audit reading (before the tests) finds the following correspondences with the TMR text; each is then pinned by a test with hand-computed values (computed independently of the code with a separate script; relative tolerance 1e-12 unless stated):

| term | TMR (sst.html) | rcfd | test (in `sst.rs` `mod tests`, names `w33c_*`) |
|------|----------------|------|--------|
| constants | sigma_k1 .85, sigma_w1 .5, beta1 .075, sigma_k2 1, sigma_w2 .856, beta2 .0828, beta* .09, a1 .31, gamma1 5/9, gamma2 .44 (2003), kappa .41 | same | exact equality of the table; model kappa from `sqrt(sqrt(beta*) (beta_i/beta* - gamma_i)/sigma_wi)` = 0.408248290463863 (set 1), 0.410151556214798 (set 2); BSL gamma_i from kappa .41: 0.5531666666666668, 0.4403546666666667 |
| blending of constants | phi = F1 phi1 + (1-F1) phi2 | `mix` | F1 = 0.3: sigma_k 0.955, sigma_w 0.7492, beta 0.08046, gamma 0.4746666666666667; F1 = 1 and 0 return sets 1 and 2 exactly |
| F1 | tanh(arg1^4), arg1 = min[max(sqrt(k)/(beta* w d), 500 nu/(d^2 w)), 4 sigma_w2 k/(CD d^2)], CD = max(2 sigma_w2 grad k . grad w / w, 1e-10) | `blend_f1`, cd from Green-Gauss | four states (nu = 1.5e-5): (k, w, d, CD) = (0.006561, 10, 0.1, 0.01) -> 0.5757621252496801 (first branch active); (1e-6, 100, 0.01, 0.01) -> 0.3062538284406168 (500 nu/(d^2 w) branch); (0.01, 10, 0.1, 10) -> 0.013743826901846232 (CD branch); (0.01, 10, 0.1, -5) -> 0.90941951863331 (CD negative, floored at 1e-10) |
| F2 | tanh(arg2^2), arg2 = max(2 sqrt(k)/(beta* w d), 500 nu/(d^2 w)) | `blend_f2` | (0.006561, 10, 0.1) -> 0.9969370765859934; (1e-6, 100, 0.01) -> 0.5098299737352566; (0.01, 10, 0.1) -> 0.9998972742141851 |
| cross diffusion | 2 (1-F1) sigma_w2 / w grad k . grad w | `cross_diffusion` (new pure helper, bit-identical arithmetic) | grad k (0.2, -0.1, 0), grad w (3, 4, 0), w 25 -> 0.013696; sign split: source + implicit coefficient times w equals the term for both signs, implicit coefficient never negative |
| strain | S = sqrt(2 S_ij S_ij) | `strain_magnitude` (new pure helper) | simple shear du/dy = g: S = g; plane strain (a x, -a y): S = 2a; solid-body rotation: S = 0 (a vorticity measure would not vanish) |
| eddy viscosity | nu_t = a1 k / max(a1 w, S F2) | `nut_of` | (k, w, S, F2) = (0.01, 10, 5, 0.8) -> 7.75e-4 (strain-limited); (0.01, 10, 2, 0.9) -> 1.0e-3 (= k/w, a1 w active) |
| production and limiter | P = nu_t S^2 (incompressible exact); P~ = min(P, 10 beta* k w) in BOTH equations (SST-2003) | `limited_production` (new helper), used for k and for w | nu_t 1e-3, S 100, k 0.01, w 50 -> 0.45 (limited), S^2 nu_t = 0.1 below the cap -> 0.1; w-equation source gamma P~/nu_t checked with gamma = F1-blend |
| k and w balance terms | -beta* rho w k; -beta rho w^2 | assembled in `update` | covered through the sublayer and channel runs below, not separately |
| diffusivities | mu + sigma_k mu_t, mu + sigma_w mu_t with the blended sigmas | `gam_k`, `gam_w` | covered by the mix test |
| wall BC for w | w_wall = 10 * 6 nu/(beta1 (Delta d1)^2), k_wall = 0 | omega face value from `dn` (centre-to-face distance = first-cell centre distance on a planar wall) | on a graded box: face value equals 60 nu/(0.075 dn^2) (hand value 1.6e6 for nu 2e-7, dn 1e-5); `LowRe` sets k = 0 on the wall face, `Automatic` zero gradient |

rcfd-only additions (not in sst.html, documented, not defects): the Spalding-law wall shear, strain and production override and the omega fixing in the wall cell for y+ > 5 (`SstWallMode::Automatic`, default; ADR 0063 HA3: moves Cf by < 0.1 percent), k zero gradient at the wall in `Automatic`, the cap nu_t <= 1e5 nu, first-order upwind k and w convection (the TMR CFL3D and FUN3D runs also use first-order upwinding for the turbulence advection, flatplate_sst.html), and the stored nu_t under-relaxation.

Resolved-wall behaviour (y+ about 1), derived from the model equations, tested on the Re_tau = 395 channel mesh of `channel_sst.rs` (60 cells, first cell centre y+ = 1) in `LowRe` mode:
* the sublayer solution of nu w'' = beta1 w^2 is w = 6 nu/(beta1 y^2); the sublayer solution of nu k'' = beta* w k with that w and P negligible is k ~ y^n with n(n-1) = 6 beta*/beta1 = 7.2, n = 3.2295 (derived here, not taken from a reference).
  Limits: (S1) cell-centre w divided by 6 nu/(beta1 y^2) lies in [0.75, 1.5] for the first four cell centres; (S2) the least-squares slope of ln k over ln y between the cells with y+ in [1, 4] lies in [2.6, 3.9]. (The limits are wide because the discrete cells at y+ 1 to 4 are not asymptotic; HA1 of ADR 0063 had w/analytic 0.86 to 1.43 on a y+ = 0.25 plate mesh.)

Decision rule P1: all term tests pass -> "no term-level defect against sst.html". A failing term test is a candidate defect; it is fixed only after the hand value is re-derived from sst.html, and the fix is opt-in if any baseline moves (baseline hashes are never re-recorded).

## Part 2: flat plate (TMR 2DZP), stated set-up, mesh sequence and limits (new file `crates/rcfd-turb/tests/validation_w33c_sst_plate.rs`, heavy, `#[ignore]`)

Set-up: exactly the runner of `validation_flat_plate_wall_law.rs` (U = 1, nu = 2e-7, plate x in [0, 2], height 1, symmetry top, Dirichlet inlet k = 2.25e-7, omega = 125 (TMR: 1.125 U^2/Re_L and 125 U/L), SimpleC, linear upwind momentum, first-order upwind k and omega, `Automatic` wall mode, convergence du < 1e-9 and nu_t change < 1e-6, max 14000 iterations). A copy of the runner is made in the new test file; no shared test code is edited.
Mesh sequence (refinement ratio 2 in both directions, y+ of the first cell 1, 0.5, 0.25 at x = 2): 70 x 55, 140 x 110, 280 x 220 (the meshes of ADR 0061). Each run once; no re-runs to select a set-up.
Definitions (fixed now): Cf(x) = 2 tau_w/ue^2 from the wall cell columns as in the runner, interpolated linearly in x between column centres; x_ref = 0.970084; Re_theta^M(x) = U int_0^x (Cf/2) dx'/nu by the trapezoid over the column centres from x = 0, plus the rectangle from 0 to the first column centre (the same formula applied to the TMR CFL3D curve gives the deviations above); u+ = u/u_tau with u_tau = sqrt(tau_w), from the column nearest x_ref; log fit by least squares of u+ against ln y+ over the cell centres with y+ in [50, 300]; the TMR u+ is interpolated linearly in ln y+.

Registered checks on the FINEST mesh unless stated (references vendored in `validation-data/reference/w33c_*.csv`, manifest entries `w33c-*`):
* P2a Cf(x_ref) within 2.0 percent of the mean of the three incompressible TMR codes on the 545 x 385 grid, 2.71647e-3 (range 2.7108 to 2.7212e-3). (Informative, no limit: against CFL3D compressible 2.69085e-3.)
* P2b Cf(x) within 3.0 percent of the TMR CFL3D 545 x 385 curve (compressible) at x = 0.5, 1.0, 1.5, 1.8 (1 percent compressibility plus 2 percent).
* P2c log-law slope (fit y+ 50 to 300, column nearest x_ref) within 3.0 percent of the same fit of the TMR CFL3D SST data at x = 0.97008, 2.7784 (i.e. [2.695, 2.862]).
* P2d u+ at y+ = 30, 50, 100, 200, 300, 600 within 3.0 percent of the TMR SST data at x = 0.97008, and at y+ = 5, 10, 15 within 5.0 percent.
* P2e Cf versus K-S at Re_theta^M = 5000, 7500, 10000, 12500: the deviation (percent) differs from the TMR CFL3D deviation (-3.61, -2.76, -2.16, -1.69) by at most 1.5 percentage points.
* P2f Grid convergence of Cf(x_ref) over the three meshes: monotone convergence, observed order p from (Cf_m - Cf_c)/(Cf_f - Cf_m) with r = 2 in [0.8, 2.5], GCI_fine (safety factor 1.25) at most 3.0 percent, and |Cf_f - Cf_m|/Cf_f at most 1.5 percent. The TMR CFL3D sequence itself has p = 1.21 and GCI 0.28 percent at 4 to 16 times more cells; first-order upwind turbulence convection gives a formal order 1. EXPECTED MISS of p and of the 1.5 percent difference from the ADR 0063 numbers (p 0.66; (2.6984 - 2.6514)/2.6984 = 1.74 percent); registered anyway, to be reported as a known miss with the limit unchanged.
* P2g Log-law slope, Cf and u+ on the coarse and medium meshes are reported (no limits) next to the finest.
Informational only (no limit): the slope and KM of the TMR plate profile versus rcfd at y+ 100, 200, 300.

Decision rule P2: if P2a, P2c, P2d and P2e hold, the 2.78 slope and the Cf deficit against K-S are declared to be the behaviour of the published model, reproduced by an independent code (TMR CFL3D/FUN3D), and no defect is claimed. A miss of any of them leaves the item open and is reported as such (known-miss test, limit unchanged).

## Part 3: log layer without convection or pressure gradient (TMR 2DFDC SST), new file `crates/rcfd-turb/tests/validation_w33c_sst_wall_layer.rs`

Set-up: half channel of height 1 (symmetry plane at the top, wall at the bottom), 4 cells in x periodic, uniform body force 1 (u_tau = 1 exactly), Re_tau = 1e6 (nu = 1e-6), 150 cells in y with first cell centre at y+ = 1 (first cell height 2e-6) and growth 1.1 to the end of the layers, uniform beyond; `Automatic` wall mode; initial U = 20, k = 1, omega = k/(10 nu); converged to du < 1e-9 and nu_t change < 1e-8 (max 40000 iterations). If this mesh does not fit the generator (layers beyond the height), the layer count is reduced and the mesh printed; the check below is unchanged.
Karman measure KM = 1/(y+ du+/dy+) from central differences of the cell data in ln y+ (the cell data are u+ at cell centres).
* P3a KM within 0.02 (absolute) of the TMR CFL3D channel SST data (`w33c_tmr_sst_channel_km_cfl3d.csv`, interpolated linearly in log10 y+) at y+ = 100, 316, 1000, 3000, 10000. (The TMR run is compressible at Re 8e7 based on the height, Re_tau of order 1e6; with Re_tau = 1e6 the y/h correction at y+ 1e4 is 1 percent.)
* P3b the log-law slope over y+ 50 to 300 within 4.0 percent of the TMR channel SST data fit over the same window, 2.693; and over 1000 to 10000 within 3.0 percent of 2.443.
This part contains no convection of turbulence quantities and no streamwise pressure gradient. Decision rule P3: if P3a and P3b hold, rcfd reproduces the model's own approach to kappa = 0.41 as published, so 1/kappa = 2.44 is not the expectation for y+ 50 to 300 in any SST computation; the plate/channel slope difference (2.78 versus 2.69 in the TMR data, 3 percent) is a boundary-layer versus channel effect of the model.

## Part 4: process

* Opt-in rule: any code change is a new default-off option; the pure-function extraction in `sst.rs` is bit-identical arithmetic (checked by the existing flat-plate/channel/api tests and `baseline_hashes` unchanged).
* Heavy tests are `#[ignore]` with the limit in the doc comment; a miss becomes `*_known_miss`; the plate meshes are run once.
* Files touched: `crates/rcfd-turb/src/sst.rs` (helpers and tests), two new test files, four `validation-data/reference/w33c_*.csv` and their manifest entries, this ADR, `docs/reference-data.md` only if a line is needed. No shared file (schema.rs, lib.rs, sim.rs, main.rs) is touched.
