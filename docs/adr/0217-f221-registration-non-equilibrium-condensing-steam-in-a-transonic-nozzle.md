# ADR 0217: F-221 registration: non-equilibrium condensing (wet) steam in a transonic nozzle

Status: **REGISTERED 2026-11-07**, written before any code of F-221 and before any rcfd run of its checks. Results go in an
addendum; nothing above the addendum is changed after a run. Not yet accepted by the maintainer.

Parents: [ADR 0006](0006-compressible-boundary-conditions.md) (the density-based solver's total-pressure inlet and its
characteristic face states), [ADR 0007](0007-real-gas-eos.md) (the real-gas EOS of the pressure-based kernel; not used here),
[ADR 0036](0036-wave21b-tables-actuator.md) and [ADR 0044](0044-wave25b-closure-accuracy-references.md) (F-220 tables; not used
here, see M2), the F-060 entry of `docs/ROADMAP.md` (the explicit density-based Euler solver `rcfd-fv::compressible`: HLLC,
MUSCL with TVD limiters on primitives, SSP-RK3; Sod and wedge validated). Roadmap item F-221 (depends on F-220 and F-060,
both done).

Number: the lead gave 0217 to F-221. Main 4c590bf holds 0214 as its highest; 0215 is on `f247-local-remeshing`, 0216 on
`f250-sliding-interface`. No branch holds a 0217.

## What was run and read before this text (disclosed)

No rcfd library or test code of F-221 existed when this text was written. Before it:

- **Literature retrieval and reading** (sources in "Sources", each read in its page images or extracted text; the PDFs are
  kept outside the repository in `E:/projects/cfd/w49i-work/f221-dl`, with their SHA-256 listed below).
- **Digitizing** (scratch Python, `pymupdf`, not committed, in `E:/projects/cfd/w49i-work/f221-scratch`): the Moses and Stein
  nozzle wall and the case 203 data were read from the *vector paths* of Figs. 1 and 2 of [S1] (`digitize_ms.py`,
  `ms_data.py`), not from a raster image: each data marker is a filled square path whose centre is mapped to the plot axes by the
  figure's own major tick marks (Fig. 1: x = -60 mm at 61.731 pt, 0 mm at 155.342 pt; y = 0 at 201.201 pt, 20 mm at 77.398 pt.
  Fig. 2: x = 1.5 cm at 347.381 pt, 6.0 cm at 543.332 pt; p/p_t1 = 0.20 at 225.69 pt, 0.45 at 77.803 pt; the right axis puts
  r32 = 0 at the 0.35 level and 0.02 um at the 0.45 level, read from its labels). The wall path has 268 points; a two-arc fit
  (`fitjunction.py`, G1 below) reproduces it to 0.0045 mm RMS, 0.0105 mm maximum.
- **A quasi-1-D prototype** (scratch Rust crate `f221-scratch/proto` against this worktree's `rcfd-core`, not committed): the model
  M below integrated along the nozzle of G1 as in the reference R-Q1D below. Its numbers are quoted as model values (M1, Q1) and
  **its comparison with the case 203 data was seen before this text**: maximum deviation of p/p_t1 from the data 6.03 percent
  (the inviscid quasi-1-D pressure lies 2.5 to 4.2 percent *below* the data already in the dry part, x = 15 to 31 mm, and up
  to 5.9 percent below near the exit), onset vertex 35.956 mm against the data's 34.906 mm (+3.0 percent). The registered
  acceptance check V1 is therefore expected to miss on the pressure trace. Nothing in the model below was chosen after that
  run: every model choice was written down before the prototype was run and is taken from the sources or fixed by a stated
  rule; the prototype was re-run only for start-point and geometry sensitivity (x_s = 1, 2, 3 mm give identical results to
  the printed digits; the digitized polyline instead of the arcs moves the onset vertex to 35.324 mm), for the model values
  of M1, and in an isochoric-box mode (three runs) that sized the box checks L1 and E1 and showed the stiffness that W4b
  answers and the closure property noted under M6.

House rules (as ADRs 0207, 0211, 0214):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the measured number recorded, and the cause
  is diagnosed, not tuned away; heavy passing runs are ignored tests too (run once in release, numbers recorded);
- a defective set-up gets a new corrected test by an amendment written before the corrected run; the old one stays as an
  ignored record;
- every new code path is opt-in: a new module (`rcfd-fv::wetsteam`) and new functions in `rcfd-core::iapws`; no existing
  function changes behaviour (B1); no case-schema key is added, so `crates/rcfd-api/tests/baseline_hashes.rs`,
  `case_hashes_w48u.rs` and the goldens are not touched; no existing test file is edited;
- each registered run is made once; nothing is re-run to pick a result;
- no model constant is tuned to the data; a change the data would suggest is an open question with a recommendation;
- no new third-party crate, no commit trailers, unsigned commits, no push.

## Sources (read; the only ones used)

- **[S1]** J. Starzmann, F. R. Hughes, A. J. White, M. Grübel, D. M. Vogt, "Numerical investigation of boundary layers in wet
  steam nozzles", ASME Turbo Expo 2016, GT2016-57598. Accepted manuscript from the University of Cambridge repository (Apollo),
  handle 1810/256656 (bitstream 610737e8-9aba-4aee-bf64-7da3208d1668; SHA-256 `172644ad...ab61fa32`, printed in full in
  `docs/reference-data.md`). Read: Eq. (1) nucleation rate per unit mass of mixture
  `J = q_c/(1+phi) sqrt(2 sigma/(pi m_m^3)) rho_g^2/(rho rho_f) exp(-4 pi sigma r_crit^2/(3 k_B T_g))`, `q_c = 1`;
  Eq. (2) `dr/dt = lambda_g/(rho_f L r) (T_s - T_g)(1 - r_crit/r) / (1 + 3.78 (1 - nu) Kn/Pr)`; Eq. (3)
  `nu = R T_s/L (alpha - 0.5 - (2 - q_c)/(2 q_c) ((gamma + 1)/(2 gamma)) (c_p T_s/L))`; "alpha = 11 provides the best
  possible agreement for several low pressure test cases and this value has been used throughout". Moses and Stein nozzle:
  constant depth 10 mm, throat height 10 mm, Fig. 1 (wall arcs labelled R = 53 mm and R = 686 mm, symmetry axis), case 203
  `p_t1 = 35.87 kPa`, `T_t1 = 368.3 K`, Fig. 2 (centreline pressure and Sauter radius; "the measured pressure distribution
  and droplet sizes have been taken from Ref. [1]", Young 1982). Moore nozzle B: `p_t1 = 25.0 kPa`, `T_t1 = 358.1 K`, throat
  height 100 mm, depth 152 mm, "the geometry and grid are available in the IWSMP collaboration" (not printed).
- **[S2]** J. Starzmann, M. Schatz, M. V. Casey, J. F. Mayer, F. Sieverding, "Modelling and validation of wet steam flow in a
  low pressure steam turbine", ASME Turbo Expo 2011, GT2011-45672 (conference archive copy at torroja.dmt.upm.es; SHA-256
  `f77c3faa...6fa32`). Read: `Kn = l/(2r)`, Eq. (9) `l_Gy = 1.5 mu sqrt(R T_g)/p`, `Pr = mu c_p/lambda` with `mu` the steam
  viscosity, Eqs. (7)-(8) (Young's growth, the same as [S1] Eqs. (2)-(3)), the nucleation rate per unit volume (the same as
  [S1] Eq. (1) times `rho`), and Table 2 (Moore nozzles' measured droplet diameters; informational only).
- **[S3]** A. Afzalifar, T. Turunen-Saaresti, A. Grönman, "Comparison of moment-based methods for representing droplet size
  distributions in supersonic nucleating flows of steam", ISROMAC 2016, HAL hal-01891315 (SHA-256 `bcbaa6e7...a67f597f`). Read:
  Eq. (1) `r* = 2 sigma/(rho_d R T_v ln S)`, `S = p_v/p_s`; Eqs. (5)-(8) the single-fluid mixture equations with
  `rho_m = rho_v/(1 - Y)`, `E_m = Y E_d + (1 - Y) E_v`, `H_m = Y H_d + (1 - Y) H_v` (no slip, liquid volume neglected); "the
  transonic and supersonic sections of the Moses and Stein nozzle were a continuous part of a circular arc", throat height
  0.01 m.
- **[S4]** S. Dykas, M. Majkut, K. Smołka, M. Strozik, "Research on steam condensing flows in nozzles with shock wave",
  J. Power Technologies 93(5) (2013) 288-294, open access (SHA-256 `c70aff52...02c94`). Read: Eq. (5), the Kantrowitz
  non-isothermal correction `C = [1 + 2 (gamma - 1)/(gamma + 1) (h_v - h_l)/(R T_v) ((h_v - h_l)/(R T_v) - 1/2)]^-1`; Eq. (2)
  `h_m = (1 - y) h_v + y h_l`.
- **[S5]** IAPWS R1-76(2014), "Revised Release on Surface Tension of Ordinary Water Substance" (iapws.org; SHA-256
  `77d07ae4...7587d`). Read: `sigma = B tau^mu (1 + b tau)`, `tau = 1 - T/T_c`, `T_c = 647.096 K`, `B = 235.8 mN/m`,
  `b = -0.625`, `mu = 1.256`; valid from the triple point to `T_c`, "reasonably accurate values when extrapolated into the
  supercooled region, to temperatures as low as -25 °C"; Table 1 column 4 (calculated values).
- **[S6]** NIST CODATA (physics.nist.gov/cgi-bin/cuu/Value?k), read on 2026-11-07: Boltzmann constant
  `1.380 649 x 10^-23 J/K`, exact. The molecular mass is taken as `m_m = k_B/R` with the IF97 `R = 461.526 J/(kg K)` (so no
  separate molar mass or Avogadro constant enters).
- **[S7]** IAPWS-IF97 as implemented and verified in `rcfd-core::iapws` (F-111; the saturation-pressure Eq. 30, regions 1 and 2,
  the IAPWS 2008 viscosity and 2011 conductivity without the critical enhancement).

Read and not used: S. Dykas et al., Heat Mass Transfer 54 (2018) 2675-2681 (a de Laval nozzle whose data are only in raster
figures); the SteamLag data set of Hughes et al. (Cambridge Apollo 1810/249282; the paper that defines its cases was not
readable). Not readable here: Moore et al. 1973 (IMechE), Moses and Stein 1978, Young 1982, Bakhtar et al. 2005, the IWSMP
results paper and its geometry files, the MDPI copy of the 2023 Barschdorff-nozzle paper (HTTP 403).

Clean-room: no source code of any CFD program, and no vendor text, was read. All equations are taken from the papers above;
the implementation is written from them.

## What F-221 asks, and the reading registered here

Acceptance: "Condensing steam in a converging-diverging nozzle reproduces the published wall-pressure trace and onset location
within 5 percent". Note: "Priority P4. Size L. Area: fv. Needs an accurate real-gas steam model first."

Reading (registered):

- **the nozzle** is the Moses and Stein nozzle, case 203 ([S1] Figs. 1, 2): the only condensing nozzle whose geometry, inlet
  state and measured pressure are all readable here. Moore nozzle B is excluded: its geometry is not printed in any readable
  source (Q4);
- **the wall-pressure trace** is the 41 pressure points of [S1] Fig. 2, "pressure ... along the centreline": in a nozzle with
  plane side walls the centreline of the side wall; in a 2-D (planar, inviscid) calculation this is the pressure on the
  symmetry axis. "Within 5 percent": every one of the 41 points `|p_sim/p_data - 1| <= 0.05`;
- **the onset location** is the position of the pressure minimum that precedes the condensation pressure rise, located by the
  same rule for data and simulation (V1b); "within 5 percent": `|x_sim - x_data| <= 0.05 x_data`, x measured from the throat;
- **the capability** is an explicit density-based Euler solver for the single-fluid wet-steam mixture with a monodisperse
  droplet model (two extra transport equations), with classical nucleation and Young's growth law; inviscid, planar 2-D or 3-D
  meshes, library level (no case-schema key, Q1);
- "**an accurate real-gas steam model first**": the vapour here is a perfect gas with IF97-derived constants (M2). This is a
  stated simplification, not the accurate real-gas model the note asks for (Q2).

## The model M (registered)

**M1. Mixture** ([S3] Eqs. 5-8): no velocity slip; liquid volume neglected; one pressure; mixture density `rho`, wetness
(liquid mass fraction) `y`, droplet number per unit mass of mixture `n`. Vapour density `rho_v = (1 - y) rho`. Equation of
state `p = rho_v R T = (1 - y) rho R T`, `R = 461.526 J/(kg K)` [S7].

**M2. Caloric model.** Vapour `h_v(T) = c_pv T`; liquid `h_l(T) = h_v(T) - L(T)` at the gas temperature, with the latent heat
linear in T (Kirchhoff, constant `c_pl`): `L(T) = L_r + (c_pv - c_pl)(T - T_r)`. Mixture `h = (1 - y) h_v + y h_l` ([S4]
Eq. 2), `e = h - p/rho`. Hence, with `c_v = c_pv - R`, `dc = c_pv - c_pl`, `A(y) = c_v + y (R - dc)`,
`q(y) = y (L_r - dc T_r)`:

`T = (e + q(y))/A(y)`, `p = (Gamma(y) - 1) rho (e + q(y))`, `Gamma(y) = 1 + (1 - y) R/A(y)`, frozen sound speed
`c^2 = Gamma p/rho`.

For frozen `y` this is a perfect gas in the shifted energy `e + q`, with ratio of specific heats `Gamma(y)` (`Gamma(0)` is the
vapour's `gamma_v = c_pv/c_v`). Parameters (registered rule for every run of this ADR): `T_r = 300 K`; `L_r = h''(T_r) - h'(T_r)`
and `c_pl = c_p'(T_r)` from IF97 regions 2 and 1 at `(T_r, p_s(T_r))` [S7]; `c_pv` = IF97 region-2 `c_p` at the inlet stagnation
state. For case 203: `c_pv = 1952.4402 J/(kg K)` (`gamma_v = 1.30956`), `c_pl = 4181.3731 J/(kg K)`, `L_r = 2.4373180 MJ/kg`
(prototype values). The F-220 tables are not used: the saturation line comes from IF97 directly.

**M3. Saturation and liquid properties.** `p_s(T)`: IF97 Eq. 30 [S7], evaluated also below 273.15 K down to 200 K
(extrapolation of the formula, needed only in start-up transients; final states are checked to stay above 273.15 K, S2).
`T_s(p)`: the inverse of that equation by a safeguarded Newton iteration (relative tolerance 1e-13). Liquid density
`rho_l(T)` = IF97 region 1 at `(T, p_s(T))`, likewise extrapolated below 273.15 K. Surface tension `sigma(T)` [S5]
(extrapolated below 248.15 K only in transients). Vapour viscosity `mu` and conductivity `lambda`: IAPWS 2008/2011 as in
`rcfd-core::iapws` at `(T, rho_v)`.

**M4. Nucleation** ([S1] Eq. 1, per unit mass of mixture): `S = p/p_s(T)`; `J = 0` for `S <= 1`; otherwise
`r* = 2 sigma/(rho_l R T ln S)` ([S3] Eq. 1), `phi = 2 (gamma_v - 1)/(gamma_v + 1) (L(T)/(R T)) (L(T)/(R T) - 1/2)` ([S4]
Eq. 5 with `h_v - h_l = L(T)`), `m_m = k_B/R` [S6], `q_c = 1`,
`J = q_c/(1 + phi) sqrt(2 sigma/(pi m_m^3)) rho_v^2/(rho rho_l) exp(-4 pi sigma r*^2/(3 k_B T))`, with `sigma`, `rho_l` at `T`.

**M5. Droplet growth** ([S1] Eqs. 2-3, [S2] Eq. 9): `T_s = T_s(p)`, `L_s = L(T_s)`, `Pr = mu c_pv/lambda`,
`Kn = l/(2 r)`, `l = 1.5 mu sqrt(R T)/p`, `nu = R T_s/L_s (alpha - 0.5 - (2 - q_c)/(2 q_c) ((gamma_v + 1)/(2 gamma_v))
(c_pv T_s/L_s))`, `alpha = 11` [S1], and
`dr/dt = lambda/(rho_l L_s r) (T_s - T)(1 - r*/r)/(1 + 3.78 (1 - nu) Kn/Pr)` (negative values evaporate). For `S <= 1` the
same formula with `r* = 2 sigma/(rho_l R T ln S)` (negative), and `dr/dt = 0` when `|ln S| < 1e-14`.

**M6. Monodisperse droplets.** `r = (3 y/(4 pi rho_l n))^(1/3)` with `rho_l = rho_l(T)`; new droplets appear at `r*`. Per unit
mass of mixture: `dn/dt = J`, `dy/dt = J (4/3) pi r*^3 rho_l + n 4 pi r^2 rho_l dr/dt` (growth term only when `y > 0`,
`n > 0`). A known property of this closure, seen in the prototype before this text: in a closed box at a fixed state with no
droplets, the droplets born at `r*` keep the mean radius at `r*` and do not grow (`1 - r*/r = 0`; the prototype's `y`
wanders at the 1e-12 level while `n` rises). In a nozzle `r*` falls along the path and the droplets grow. The box checks
below therefore start with droplets present.

**M7. Conservation form.** Conserved variables `U = (rho, rho u, rho v, rho w, rho E, rho y, rho n)` with
`E = e + |u|^2/2`; fluxes of the Euler equations plus `rho y u`, `rho n u`; source `(0, 0, 0, 0, 0, rho dy/dt, rho dn/dt)`.
Mass, momentum and energy of the mixture carry no source, so the latent heat release appears through `e(T, y)` (M2) and
total mass and energy are conserved by construction; the liquid mass changes only by the source.

**M8. Validity guards.** A state with `T` outside `[200 K, 640 K]`, `y` outside `[0, 0.5)`, non-positive `rho`, `p` or `e + q`
is a run-time error naming the cell.

## The solver W (registered)

**W1. Module** `crates/rcfd-fv/src/wetsteam.rs` (new): `WetSteamModel` (M1-M8), `WetSteamSolver` (state `Vec<[f64; 7]>`),
boundary conditions `Slip`, `Extrapolate`, `Fixed(state)`, `TotalInlet` (dry inflow: the face state of
`compressible_bc::total_inlet` with `Gas { gamma: gamma_v }`, `y = n = 0`). The cell-gradient routine of `compressible.rs`
(`ls_inverse`) is made `pub(crate)` (visibility only).

**W2. Flux.** HLLC (Toro ch. 10) for a general EOS: per side `rho, u, p, E, Gamma, c`; wave speeds as in
`riemann::hllc` (Einfeldt-Roe: `S_L = min(u_nL - c_L, u_roe - c_roe)`, `S_R = max(u_nR + c_R, u_roe + c_roe)`, with
`c_roe^2 = (Gamma_roe - 1)(H~_roe - |u_roe|^2/2)`, `H~ = Gamma/(Gamma - 1) p/rho + |u|^2/2`, `Gamma_roe` the Roe average of
`Gamma`); star states with the general `E*` formula; the scalars carried as `rho* y_K`, `rho* n_K`. With `y = 0` on both sides
the formulas reduce to `riemann::hllc` with `gamma = gamma_v`.

**W3. Reconstruction** MUSCL on primitives `(rho, u, v, w, p, y, n)` with the unstructured r-ratio TVD form and least-squares
gradients of `compressible.rs`, limiter van Leer; a reconstructed side with non-positive `rho` or `p`, or `y`, `n` outside
`[0, 0.5)`, `[0, inf)`, falls back to first order on that face (as `compressible.rs` does for `rho`, `p`).

**W4. Time integration** SSP-RK3 (Shu-Osher: three forward-Euler sub-steps `U + dt L(U)` combined convexly), the source
evaluated from each sub-step's cell state (unsplit), global CFL step `cfl 2V/sum_f(|u.n| + c)|S|` with the frozen `c`,
`cfl = 0.4`. After each sub-step `rho y` and `rho n` are clipped at zero (a clip counter is kept and reported). The step also
returns the source integral `dt sum_c V_c sum_s b_s S(U_s)` of the step (RK3 weights `b = 1/6, 1/6, 2/3`) for the bookkeeping
check C1.

**W4b. Point-implicit wetness** (setting `point_implicit`, on for the nozzle, L1 and E1 runs): the growth relaxation is stiff
(prototype estimate near the nozzle exit: relaxation time about `8e-9 s` against a CFL step of about `7e-8 s`). In every
forward-Euler sub-step the whole `rho y` increment (flux and source) is divided by `1 - dt lambda`, `lambda = min(0,
dS_y/d(rho y))` at fixed `rho, rho u, rho E, rho n`, by a one-sided difference with `delta = 1e-6 max(rho y, 1e-9 rho)`
(cells with `y > 0` and `n > 0` only). A steady state (`L(U) = 0`) is unchanged by the factor; in transients the liquid is not
conserved cell by cell (a local time step for `rho y`), so the bookkeeping check C1 runs with the factor off, and C1b checks
the mixture's conservation with it on.

**W4c. Parallel evaluation:** cell sources and face fluxes may be evaluated in parallel (rayon), each into its own slot,
and summed in face order, so a run is deterministic and independent of the thread count.

**W5. Settings for the registered runs:** HLLC as W2, van Leer, SSP-RK3, CFL 0.4, point-implicit on; condensation on unless
stated.

## The nozzle set-up (registered)

**G1. Geometry** (half nozzle, symmetry axis at `y = 0`): throat half-height `H = 5 mm` at `x = 0`; for `x >= -20 mm` the arc
`h = H + R1 - sqrt(R1^2 - x^2)`, `R1 = 686 mm`; for `x < -20 mm` the arc of radius `R2 = 53 mm` internally tangent to it at
`x = -20 mm` (centre `x_c = -(R1 - R2) s`, `y_c = H + R1 - (R1 - R2) sqrt(1 - s^2)`, `s = 20/686`;
`h = y_c - sqrt(R2^2 - (x - x_c)^2)`); inlet at `x = -50 mm`, outlet at `x = 65 mm`. The radii and the throat are printed in
[S1] Fig. 1 and text, the throat-to-exit arc in [S3]; the junction `x = -20 mm` and the ends come from the digitized wall
(the fit puts the junction at -20.008 mm). Against the 268 digitized points: RMS 0.0045 mm, maximum 0.0105 mm.

**G2. Meshes:** `box_2d(nx, ny, 0.115, 1.0)` warped by `x' = -0.05 + x`, `y' = y h(x')`: fine `460 x 20` (0.25 mm at the
throat), coarse `230 x 10`. Patches: `xmin` total inlet `p0 = 35.87 kPa`, `T0 = 368.3 K`, direction `+x`; `xmax`
extrapolate (supersonic outflow); `ymax` slip wall; `ymin` slip (symmetry axis).

**G3. Initial state:** per column the quasi-1-D dry isentropic state at the column's area ratio (`gamma_v`, subsonic branch for
`x < 0`, supersonic for `x > 0`, `M = 1` in the throat column), velocity along `+x`, `y = n = 0`.

**G4. Steady state:** march until the largest relative density change in a step is below `1e-8`, at most 100000 steps
(fine) or 50000 (coarse). Every check is evaluated on the final state; not reaching the criterion is a miss of S1.

**G5. Sampling:** stations = the 41 data abscissae. "Centreline" value at a station: linear interpolation in x between the
cell centres of the first cell row (adjacent to the symmetry axis). "Column mean": volume-weighted mean over a column of cells,
interpolated in x. Onset vertex (for any 41-value sequence): the first station `k` (`1 <= k <= 39`, zero-based, in increasing
x) with `v_k <= v_(k-1)` and `v_k <= v_(k+1)`; the vertex of the parabola through stations `k-1, k, k+1`. No such station:
onset undefined (a miss).

**R-Q1D. Quasi-1-D reference** (independent solution method, coded in the test): steady inviscid quasi-1-D equations
of model M, `d(rho u h) = 0`, `dh + u du = 0`, `dp = -rho u du`, `p = (1 - y) rho R T`, `dy/dx = (dy/dt)/u`,
`dn/dx = J/u`, giving
`du/u = [-dh/h + dy (L(T)/(c_pm T) - 1/(1 - y))]/(1 - u^2/c_f^2)`, `c_pm = c_pv - y dc`, `c_f^2 = Gamma p/rho`; started at
`x_s = 2 mm` from the dry isentropic supersonic state (`gamma_v`, sonic at `x = 0`), classic RK4 in x with `dx = 2e-7 m` to
`x = 65 mm`. The droplets nucleated upstream of `x_s` along the dry path are neglected (prototype: `7e-24` per kg). Prototype
results (model values): T_min 280.83 K, peak `J = 2.10e23 /(kg s)` at x = 31.09 mm, onset vertex 35.956 mm, at 65 mm
`p/p0 = 0.1963`, `y = 0.0475`, `n = 1.390e18 /kg`, `r = 20.2 nm`.

## Reference data (registered; digitized as described above)

Moses and Stein case 203 ([S1] Fig. 2), `p_t1 = 35.87 kPa`, `T_t1 = 368.3 K`. Pressure `(x cm, p/p_t1)`, 41 points:

1.5039 0.43590, 1.6015 0.42881, 1.7043 0.42143, 1.8058 0.41424, 1.9086 0.40724, 2.0101 0.40006, 2.1129 0.39287,
2.2144 0.38587, 2.3211 0.37868, 2.4316 0.37102, 2.5421 0.36373, 2.6488 0.35664, 2.7554 0.34964, 2.8788 0.34179,
2.9983 0.33451, 3.1178 0.32722, 3.2321 0.32013, 3.3105 0.31559, 3.3709 0.31190, 3.4737 0.31048, 3.6061 0.31180,
3.6921 0.31370, 3.8053 0.31606, 3.9106 0.31720, 4.0186 0.31720, 4.1111 0.31578, 4.1959 0.31322, 4.2576 0.31029,
4.3231 0.30707, 4.4002 0.30358, 4.4876 0.29951, 4.6032 0.29412, 4.7445 0.28740, 4.8924 0.28059, 5.0504 0.27330,
5.2071 0.26602, 5.3601 0.25940, 5.5143 0.25193, 5.6556 0.24531, 5.7944 0.23935, 5.9293 0.23348.

Onset vertex of the data (G5 rule): 34.906 mm. Sauter radius `(x cm, r32 nm)`, 17 points: 3.5239 10.390, 3.6150 11.600,
3.7205 12.735, 3.8412 13.682, 3.9980 14.608, 4.1599 15.460, 4.3205 16.197, 4.4811 16.784, 4.6418 17.294, 4.8037 17.748,
4.9643 18.108, 5.1262 18.449, 5.2868 18.733, 5.4475 18.978, 5.6132 19.187, 5.7944 19.376, 5.9666 19.508.

Digitizing uncertainty: the marker centres are exact path coordinates (the squares are 1.68 pt wide); the axis maps use the
tick marks (spacing 21.767 pt per 0.5 cm and 29.578 pt per 0.05): a 0.05 pt error is 0.001 cm and 0.0004 in p/p_t1. The
figure itself may round the original data (unknown); the data are themselves a re-plot of Young 1982's copy of Moses and
Stein 1978 ([S1]).

## Checks and limits (registered)

Fast checks (gate, `crates/rcfd-fv/tests/wetsteam.rs` and `crates/rcfd-core` unit tests):

- **P1 Surface tension** vs [S5] Table 1 column 4 at 0.01, 25, 50, 75, 100 °C (75.65, 71.97, 67.94, 63.58, 58.91 mN/m):
  within 0.006 mN/m (table rounding).
- **P2 Saturation temperature:** `p_s(T_s(p)) / p - 1` within 1e-12 for p = 1, 3, 10, 30, 100 kPa; `T_s(p_s(T))` within
  1e-9 K for T = 250, 280, 300, 350, 450 K; the extended `p_s` equals `rcfd-core::iapws::saturation_pressure` bit for bit
  at 273.15, 300, 500 K.
- **M1 Model values** vs the prototype (an independent implementation of M2-M5 written before the library) at three states,
  relative 1e-6 on every quantity (`rho` from `p, T, y`):

  | state (T K, p Pa, y, r m) | p_s | T_s | sigma | rho_l | L(T) | L(T_s) | phi | r* | J (1/(kg s)) | dr/dt (m/s) |
  | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
  | 285, 12000, 0, - | 1.3890056426e3 | 322.5697759943 | 7.3950695449e-2 | 9.9946874651e2 | 2.4707520092e6 | 2.3870115016e6 | 9.2066963264e1 | 5.2173394580e-10 | 2.0308632340e22 | - |
  | 290, 10000, 0.02, 1.5e-8 | 1.9199333106e3 | 318.9575482070 | 7.3210101179e-2 | 9.9875642965e2 | 2.4596073450e6 | 2.3950629147e6 | 8.8065863443e1 | 6.6372129998e-10 | 7.6313126716e16 | 5.7695442623e-4 |
  | 300, 9000, 0.04, 2.0e-8 | 3.5365894130e3 | 316.9118416104 | 7.1685962527e-2 | 9.9651426338e2 | 2.4373180166e6 | 2.3996226574e6 | 8.0708601334e1 | 1.1124681327e-9 | 3.4277088937e-8 | 2.8906688854e-4 |

  (model parameters of case 203, M2).
- **L1 Isochoric relaxation** (a closed 2 x 2-cell box of 1 mm cells at rest, uniform `T = 285 K`, `p = 12 kPa`,
  `y = 1e-4`, `n = 1e16 /kg`, case-203 parameters, point-implicit on, `dt = 2e-8 s` fixed): every cell stays identical to
  round-off (relative spread of `rho y`, `rho n`, `rho E` `<= 1e-12`), and `y`, `n`, `T` at `t = 1e-5, 2e-5, 4e-5, 1e-4 s`
  agree with an independent RK4 integration of the isochoric ODE (`rho`, `e` fixed; `dt = 1e-10 s`; coded in the test):
  `y` within `1e-3` relative, `n` within `1e-3` relative, `T` within `0.01 K`. (Prototype: at `1e-4 s`, `y = 0.026107`,
  `n = 1.4074e17`, `T = 323.97 K`; the relaxation rate at the end `-7.7e4 /s`, so `dt |lambda| = 1.5e-3`.)
- **E1 Equilibrium:** the same box with `dt = 1e-7 s` for 20000 steps (`t = 2e-3 s`): the final `y` equals the root of the
  model's growth equilibrium at fixed `rho`, `e`, `n` (`ln S = 2 sigma/(rho_l R T r)`, `r = r(y, n)`, solved by bisection in
  the test) within `1e-4` relative.
- **C1 Conservation** (closed `12 x 6` box of slip walls, `1 x 0.5 mm`, point-implicit off, non-uniform initial state:
  `T = 290 K + 5 K sin(2 pi x/L_x)`, `p = 11 kPa (1 + 0.05 cos(2 pi y/L_y))`, `u = 50 m/s sin(pi y/L_y)`,
  `v = 20 m/s sin(pi x/L_x)`, `y = 0.01`, `n = 1e17 /kg`; 300 steps of the CFL step): total mass and total energy relative
  drift `<= 1e-12`; the liquid bookkeeping `|sum V rho y (end) - sum V rho y (0) - sum_steps source integral| <= 1e-12
  sum V rho y (end)`, likewise for `rho n`; zero clips. **C1b:** the same with point-implicit on: mass and energy drift
  `<= 1e-12`.
- **S1 Frozen wet shock tube** (condensation off, `y = 0.05` uniform, `n = 0`, 1-D `box_2d(200, 1, 1 m)`, both ends fixed
  states): left `T = 300 K`, `p = 20 kPa`, at rest; right `p = 2 kPa`, `rho = rho_L/8`, at rest (the Sod ratios); diaphragm at
  0.5 m; `t = 0.237/c_L` with `c_L = sqrt(Gamma p_L/rho_L)` (the Sod `c_L t` of F-060). Density L1 divided by `rho_L`,
  against the exact Riemann solution with `gamma = Gamma(0.05)`, `<= 4.0e-3` (F-060 measured 2.82e-3 for HLLC at gamma 1.4),
  and `max |y - 0.05| <= 1e-13`.
- **D1 Dry limit** (condensation off, `y = n = 0`): against `compressible::CompressibleSolver` with `Gas { gamma: gamma_v }`,
  HLLC, van Leer, RK3, the same steps: (a) the shock tube of S1 with `y = 0` (200 cells, the same number of steps); (b) the
  coarse nozzle (G1, `60 x 6` cells, G3 start, 200 steps of the solver's `max_dt`): largest relative difference of `rho`,
  `u`, `p` per cell `<= 1e-12` (velocity relative to the largest speed).
- **B1 Defaults:** every existing test passes unchanged; `baseline_hashes.rs`, `case_hashes_w48u.rs`, goldens and existing
  test files untouched (the gate and the diff).

Heavy checks (ignored tests, release, run once; numbers recorded in the addendum):

- **S1n Steadiness:** G4 criterion reached within the cap on the fine wet, fine dry and coarse wet runs. **S2:** the final
  state's minimum temperature `>= 273.15 K` on the wet runs (the extrapolated properties of M3 are then not used at
  steady state); clip count reported.
- **N1 Dry nozzle** (fine, condensation off) against the isentropic quasi-1-D relation (`gamma_v`): column-mean `p/p0`
  within 1.5 percent at the 41 stations; inlet mass flow within 0.5 percent of the choked value
  `rho* a* H` (per unit depth, half nozzle).
- **Q1 Wet nozzle against R-Q1D** (fine): column-mean `p/p0` within 2.0 percent at the 41 stations; column-mean onset vertex
  within 1.5 mm of the R-Q1D vertex; exit-column mass-flow-weighted `y` within 5 percent and `n` within a factor 2 of R-Q1D at
  65 mm.
- **V1 Acceptance** (fine, centreline): (a) all 41 points `|p/p_t1 sim / data - 1| <= 0.05`; (b) onset vertex within 5 percent
  of 34.906 mm. Expected from the prototype: (a) misses, (b) meets (see the disclosure above).
- **V2 Droplet radius** (fine, centreline monodisperse `r`): within 25 percent of `r32` at the 12 stations `x >= 4.1599 cm`
  (the two averages differ by definition; 25 percent is the registered allowance).
- **G1g Grid:** coarse against fine: onset vertex (centreline) within 1.0 mm; centreline `p/p_t1` within 1.5 percent at the 41
  stations.
- **NC1 Negative control:** the dry fine run's centreline compared with the data by V1(a) must miss by more than 5 percent
  (the check sees condensation); **NC2:** the wet fine column means compared with the *dry* isentropic quasi-1-D relation must
  differ by more than 2 percent at some station (Q1 sees condensation).

Mutations (each applied to a scratch copy of the library, the fast checks run; each must make at least one fast check fail):
**MU1** `phi = 0` (no Kantrowitz correction); **MU2** the factor `(1 - r*/r)` dropped; **MU3** the nucleated-mass term
`J (4/3) pi r*^3 rho_l` dropped; **MU4** `q(y)` dropped from `T` and `p` (no latent heat in the energy); **MU5** the
condensation source also added to the mixture density equation; **MU6** the frozen sound speed with `gamma_v` instead of
`Gamma(y)`.

## Decision rule (registered)

The correctness set K = {P1, P2, M1, L1, E1, C1, C1b, S1, D1, B1} and every mutation caught. The flow set F = {S1n, S2, N1, Q1, G1g,
NC1, NC2}.

- K met, F met, V1 (a) and (b) met: F-221 **partial** (`[~]`), the remaining part named (case-schema wiring Q1, the real-gas
  vapour Q2, viscous effects Q3); done (`[x]`) is the maintainer's call (Q7).
- K met, F met or missed only in part, V1 missed in (a) or (b): F-221 **partial**, the V1 miss named with its numbers and kept
  as an ignored `*_known_miss` test with the limit intact; the F misses named.
- Any miss in K: F-221 stays **todo**, the miss named.

## Not delivered (registered)

- Case-schema keys, CLI, Studio or MCP wiring (Q1).
- A real-gas vapour EOS (Q2); the vapour is a perfect gas with IF97 constants.
- Viscous terms, boundary layers, turbulence (Q3). [S1] attributes part of the dry-region pressure of this narrow nozzle to
  side-wall boundary layers.
- Polydisperse droplets (method of moments, [S1]) (Q5); heterogeneous nucleation; velocity slip; droplet temperature as a
  separate variable.
- Moore nozzle B (Q4); other published nozzles.
- Implicit time integration (F-060's open item).

## Open questions (with recommendations)

- **Q1. Case keys.** A `[wet_steam]` optional table under the compressible algorithm (hash omitted when absent). Recommendation:
  a follow-up item after Q2, with a validate-error set like ADR 0006 E.
- **Q2. Real-gas vapour.** The roadmap note asks for an accurate real-gas steam model first; [S4]-type virial or IF97 region-2
  tables (F-220 `PtTable`) in the Riemann solver need a general EOS path. Recommendation: a follow-up item; F-221 not done
  before it.
- **Q3. Boundary layers.** Recommendation: compare with a viscous run once a compressible Navier-Stokes path exists; no
  displacement correction fitted to these data.
- **Q4. Moore nozzle B.** The geometry is only in the IWSMP collaboration files. Recommendation: the maintainer obtains a public
  copy, or the item keeps Moses and Stein 203 as its only acceptance case.
- **Q5. Moments.** Recommendation: a follow-up, compared on the same case.
- **Q6. alpha = 11.** Registered from [S1], not tuned. Recommendation: keep; a sensitivity study only as a separate registered
  item.
- **Q7. Status.** Recommendation: as the decision rule says.
- **Q8. Data location.** The case-203 points live in the test file and `docs/reference-data.md`, not in `validation-data`
  (its manifest has checksum tests). Recommendation: move them in when the manifest is next touched.

## Amendment 1 (2026-11-07, written after the first run of the fast checks and before any corrected run)

The first run of the fast checks (release, `crates/rcfd-fv/tests/wetsteam.rs`, log kept outside the repository) met P1, P2,
M1, E1, C1, C1b, S1, D1a and D1b, and **missed L1**: at `t = 4e-5 s` the relative spread of the four box cells was
`8.91e-12` against the registered `1e-12` (`1.7e-15` at `2e-5 s`, `6.5e-16` at `1e-5 s`). The test stopped at that assertion;
the other L1 numbers at `4e-5 s` were inside their limits (`y` relative `1.63e-4`, `n` `2.45e-9`, `T` `-4.33e-3 K`) and the
values at `1e-4 s` were not evaluated.

Diagnosis (a scratch program, `f221-scratch/diag`, not committed): `rcfd_core::iapws::saturation_temperature`, stopped at the
registered relative tolerance `1e-13` (M3), is not smooth below that level: over 2000 pressures in 8 to 16 kPa, a step of
three units in the last place of `p` moves `T_s` by up to `6.7e-14` relative beyond the true change (about `2e-11 K`). Late in
the relaxation `T_s - T` is about 10 K, so the growth rate of cells whose pressures differ in the last bits differs by about
`2e-12` relative; the box cells drift apart at that level. The spread is not a flux asymmetry (D1 reproduces the perfect-gas
solver bit for bit) but the iteration tolerance of M3, which the L1 limit did not allow for: a set-up defect of the
implementation registered in M3.

Corrections (registered here, before the corrected run):

- **M3 amended:** `T_s(p)` is iterated until the Newton step is at most `1e-15 T` (in place of `1e-13`), i.e. to the last
  bits; P2 and every other check keep their limits.
- **L1b:** the L1 set-up and all its limits unchanged, run once with the amended M3 (`l1b_...`). L1b takes L1's place in the
  correctness set K; L1 stays as the ignored record `l1_isochoric_relaxation_matches_an_independent_ode_known_miss` with its
  numbers.
- The heavy runs had not been started; they run with the amended M3.

## Addendum: results (2026-11-07; the registered text above is unchanged)

Code: `crates/rcfd-fv/src/wetsteam.rs` (model, HLLC, solver), additions to `crates/rcfd-core/src/iapws.rs`
(`saturation_pressure_extended`, `saturation_temperature`, `region1_state`, `region2_state`, `saturated_liquid`,
`saturated_vapour`; `saturation_pressure` now calls the same Eq. 30 helper, bit for bit), `ls_inverse` and `safe` of
`compressible.rs` made `pub(crate)`, checks in `crates/rcfd-fv/tests/wetsteam.rs`. The fast checks' first run and the
mutations used the working tree before amendment 1's tolerance change; L1b and the heavy runs used commit 83c92f9 (the
amended tolerance). No case-schema key, golden, `baseline_hashes.rs`, `case_hashes_w48u.rs` or existing test file was touched.

### Fast checks (release; first run before amendment 1, L1b after it)

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| P1 surface tension vs [S5] Table 1 col. 4 | 0.006 mN/m | -0.0037, +0.0022, +0.0039, +0.0030, +0.0019 mN/m at 0.01, 25, 50, 75, 100 °C | met |
| P2 `p_s(T_s(p))/p - 1` | 1e-12 | at most 5.0e-15 (1 to 100 kPa) | met |
| P2 `T_s(p_s(T)) - T` | 1e-9 K | at most 1.8e-11 K (first run, tolerance 1e-13) | met |
| P2 extended `p_s` = IF97 `p_s` at 273.15, 300, 500 K | bit for bit | bit for bit | met |
| M1 model values vs the prototype | 1e-6 relative | worst 8.1e-8 | met |
| L1 cell spread (first run) | 1e-12 | 6.5e-16, 1.7e-15, **8.91e-12** at 1e-5, 2e-5, 4e-5 s | **missed** (amendment 1; ignored record `..._known_miss`) |
| L1b cell spread | 1e-12 | 0 at all four times | met |
| L1b `y` | 1e-3 relative | 2.2e-9, 2.3e-10, 1.63e-4, 2.6e-5 at 1e-5, 2e-5, 4e-5, 1e-4 s | met |
| L1b `n` | 1e-3 relative | at most 2.6e-9 | met |
| L1b `T` | 0.01 K | at most 4.33e-3 K (at 4e-5 s) | met |
| E1 final `y` vs the growth-equilibrium root | 1e-4 relative | 5.1e-14 (`y = 0.0262060`, `n = 1.4074e17 /kg`, `T = 324.11 K`, `S = 1.02613`) | met |
| C1 mass / energy drift | 1e-12 | 1.39e-14 / 1.72e-14 | met |
| C1 liquid / droplet bookkeeping | 1e-12 | 1.22e-14 / 1.78e-14; zero clips | met |
| C1b mass / energy drift (point-implicit on) | 1e-12 | 1.41e-14 / 1.73e-14 | met |
| S1 frozen wet shock tube, density L1 / `rho_L` | 4.0e-3 | 2.478e-3 (`Gamma(0.05) = 1.269743`, 346 steps) | met |
| S1 `max abs(y - 0.05)` | 1e-13 | 2.7e-16 | met |
| D1a dry shock tube vs `CompressibleSolver` | 1e-12 | 0 (bit for bit, 346 steps, the same `max_dt`) | met |
| D1b dry coarse nozzle vs `CompressibleSolver` | 1e-12 | 0 (bit for bit, 200 steps) | met |
| B1 defaults | the gate | see "The gate" | see below |

### Mutations (a `git archive` copy of 83c92f9, the ten fast checks run once per mutation)

| Mutation | Fast checks that failed | Verdict |
| --- | --- | --- |
| MU1 `phi = 0` | M1, E1 (its bracket assertion: the extra droplets put the root outside `[1e-6, 0.2]`), L1b | caught |
| MU2 `(1 - r*/r)` dropped | M1, E1 | caught |
| MU3 nucleated-mass term dropped | none | **survived** |
| MU4 `q(y) = 0` | C1, C1b and E1 (a cell fell below 200 K: the M8 guard), L1b | caught |
| MU5 source also into the mixture density | C1 (mass drift 8.5e-3), C1b, L1b | caught |
| MU6 frozen sound speed with `gamma_v` | none | **survived** |

Diagnosis of the survivors. MU3: the mass of droplets born at `r*` is tiny against the growth term in every fast set-up (in the
L1 box about `7.7e-8` of `y = 1.34e-3` at `1e-5 s`, against the L1 limit `1e-3`); no fast check isolates the term. MU6: the
sound speed enters only the HLLC wave-speed estimates and `max_dt`, so the mutant widens the estimated wave fan of a flux
that stays consistent and conservative, and S1 stays inside its limit: a dissipation-only mutant for these checks. Both are
weaknesses of the registered check set; neither is a defect found in the code (Q9).

### Heavy checks (release, one process, `--ignored --test-threads=1`, 54 min; the three runs computed once each)

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| S1n fine wet | below 1e-8 within 100000 steps | 9.99e-9 at step 93164 (`t = 6.84 ms`, 35 min) | met |
| S1n fine dry | below 1e-8 within 100000 steps | 9.93e-9 at step 93405 (14 min) | met |
| S1n coarse wet | below 1e-8 within 50000 steps | not reached: 8.18e-6 at the cap (sampled 2.78e-7, 2.86e-7, 2.72e-7 at steps 20000, 30000, 40000) | **missed** (ignored record `..._known_miss`) |
| S2 minimum temperature, wet runs | at least 273.15 K | fine 280.66 K, coarse 280.69 K (printed; the test's assertion was not reached after the S1n panic) | met (by the printed numbers) |
| S2 clips | reported | fine wet 81,306,117, coarse wet 22,035,135, fine dry 0 | reported, see below |
| N1 column-mean `p/p0` vs isentropic | 1.5 % | worst 0.189 % | met |
| N1 inlet mass flow vs choked `rho* a* H` | 0.5 % | -0.303 % (0.290137 against 0.291017 kg/(m s)) | met |
| Q1 column-mean `p/p0` vs R-Q1D | 2.0 % | worst 0.161 % | met |
| Q1 onset vertex vs R-Q1D | 1.5 mm | 36.144 mm against 35.956 mm (0.19 mm) | met |
| Q1 exit `y`, `n` vs R-Q1D | 5 %, factor 2 | 0.04738 against 0.04750 (-0.25 %); 1.361e18 against 1.390e18 (ratio 0.979) | met |
| **V1a** centreline `p/p_t1` vs the 41 data points | 5 % each | worst **-4.932 %** (x = 5.9293 cm: 0.22196 against 0.23348); every point below the data, from -1.6 % near the onset to -4.9 % at the exit; 0 of 41 beyond 5 % | **met** |
| **V1b** onset vertex vs 34.906 mm | 5 % | 35.570 mm, **+1.90 %** | **met** |
| V2 centreline `r` vs `r32`, 12 stations `x >= 4.16 cm` | 25 % | worst +3.85 % (x = 4.1599 cm: 16.06 against 15.46 nm) | met |
| G1g coarse vs fine, centreline `p/p_t1` | 1.5 % | worst 0.356 % (on the coarse run's final, unsteady state) | met |
| G1g onset vertex | 1.0 mm | 35.570 (fine) against 36.381 mm (coarse): 0.81 mm | met |
| NC1 dry centreline vs data | must exceed 5 % | 25.40 % | met |
| NC2 wet column means vs dry isentropic | must exceed 2 % | 28.02 % | met |

The prototype's expectation (V1a missing at 6.03 %) did not hold for the 2-D run: the column means follow the quasi-1-D
reference to 0.16 % (Q1), and the centreline pressure lies above the column mean by about 1 % near the exit, which keeps every
point inside 5 %, by a margin of 0.07 percentage points at the last station. The deficit against the data grows towards the
exit as it does in the dry part upstream of the onset (-2.2 % at 1.5 cm, -3.3 % at 2.4 cm), consistent with [S1]'s attribution
of this narrow nozzle's pressure to boundary-layer blockage, which an inviscid calculation leaves out (Q3).

**Clips (S2), diagnosed on the scratch copy** (an instrumented `clip`, the coarse wet run for 20000 steps; not committed):
9,075,135 clips in 621 cells. Nearly all are round-off-sized values (`rho y/rho` at most `1e-78` in magnitude, `n` at most
`1e-55`) in dry cells upstream of the throat (x = -11 to -5 mm, `S` = 1.3 to 1.6), where the nucleation rate is about
`exp(-200)` and the droplets born at `r*` sit on the edge of evaporation (M6). The largest clipped wetness, `y = -1.96e-4`,
occurred in cells near the outlet (x = 60 to 65 mm) during the start-up, when the condensation front entered the initially
cold (235 K) dry supersonic flow of G3 (about 100 clips per cell there). Clipping does not touch mixture mass or energy; the
liquid it adds is round-off upstream and a start-up transient downstream.

**The coarse run's unsteadiness (S1n)** is not diagnosed further: its density change per step stalls at about 3e-7 and
fluctuates up to 8e-6, while the fine run converges geometrically to 1e-8; a flicker of the nucleation front, which the
coarse grid resolves with about ten cells, is the likely cause (Q10).

### Status by the decision rule

K: P1, P2, M1, L1b (amendment 1, in place of L1), E1, C1, C1b, S1, D1 met; B1 by the gate; **mutations MU3 and MU6
survived**. F: S1n missed on the coarse run; S2, N1, Q1, G1g, NC1, NC2 met. V1 (a) and (b) met; V2 met.

The registered rule defines K as the correctness checks *and every mutation caught*, and says "any miss in K: F-221 stays
todo". Two mutations survived, so **F-221 stays todo** (`[ ]`), with the acceptance comparison met and the misses named: the
surviving mutations MU3 and MU6 and the coarse run's S1n. Q9 and Q11 ask the maintainer whether to register the missing
checks and set partial.

### Open questions added by the results

- **Q9. The surviving mutations.** Options: (a) register, as an amendment or a follow-up item, a check that isolates the
  nucleated-mass term (the `rates()` decomposition at the M1 states against the prototype) and a check of the HLLC wave-speed
  bounds against the frozen sound speed for MU6, then re-apply the decision rule; (b) leave F-221 todo. Recommendation (a).
- **Q10. Coarse-grid unsteadiness.** Options: (a) a registered study of the condensation front on 230 x 10 (time history of
  the onset position and the nucleation-rate peak); (b) accept the fine grid only. Recommendation (a), together with Q3.
- **Q11. Status.** By the rule: todo. With Q9 (a) done and met: partial, the remaining part being Q1 (case keys), Q2 (real-gas
  vapour) and Q3 (viscous terms). Done (`[x]`) remains the maintainer's call; the acceptance comparison (V1) is met at library
  level, inviscid, with a perfect-gas vapour, by a margin of 0.07 percentage points at the last station.
