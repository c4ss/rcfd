# ADR 0224: F-245 registration: droplet evaporation with the d-squared law and TAB breakup in the particle library

Status: **REGISTERED 2026-11-08**, written before any code of F-245 and before any run of its checks. Not yet accepted by the
maintainer. Before this text no rcfd code or test was run for F-245. Three texts were read (the reading section), and a Python
model of my own (scratch, not committed, `w49i-work/f245-scratch/predict.py`) evaluated the closed forms of this text to make
the predictions; it does not contain or run rcfd code. Results go in the addendum at the end.

Summary of the results (addendum 1): in the library (`rcfd-particles`, no case key), a droplet at its wet bulb in still 800 K gas
follows the d-squared law with the derived evaporation constant `K = 3.4196739796e-7` m^2/s to 1.1e-15 (limit 2 percent), one
injected at 300 K to 7.1e-12 after its heat-up, and the vapour booked equals the liquid lost to 2.9e-16 (3.0e-12 in a spray with
breakup; limit 1e-10). The TAB breakup time and product radius of a drop at rest in a step relative velocity match their closed
forms to 1.5e-14 at Weber numbers 6.1 to 1000 and with viscosity (limit 2 percent), the source's limits within 0.82 percent, and
`n d^3` through breakup to 3.9e-16. Five negative controls held, eleven mutations failed their checks, no amendment. By the rule
F-245 is done, the acceptance still the maintainer's (Q1); case-tree wiring is Q2.

Parents: [ADR 0028](0028-wave18c-particles.md) (F-056, the tracker), [ADR 0057](0057-wave29c-lagrangian-extensions.md) (F-128:
the heat transfer of the parcels, the exchange record, the random walk), [ADR 0192](0192-f253-registration-particle-wall-film.md)
and [ADR 0194](0194-f254-registration-particle-wall-interaction.md) (the house rules of the particle items),
[ADR 0208](0208-f255-registration-particle-dispersion-from-turbulence-model-fields-and-particle-surface-reactions.md) (F-255: the
surface reaction, whose per-cell booking and refusals this work follows). Roadmap item F-245 (depends on F-128 and F-056), split
from F-128 on 2026-10-21; the user kept A1 and A2 in it on 2026-10-22.

Number: the lead gave 0224 to F-245. Branch `f245-droplet` from main c201a770, worktree `w49i-work/f245`.

House rules (ADR 0130, ADR 0192, ADR 0208):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the number recorded, and the cause is
  diagnosed, not tuned away;
- a defective set-up gets a new corrected test by an amendment written before the corrected run, and the old one stays as an
  ignored record;
- every new code path is opt-in: a library caller that does not call the new setters gets the tracker of main bit for bit;
- no case-schema key is added (S2); neither `crates/rcfd-api/tests/baseline_hashes.rs` nor `case_hashes_w48u.rs` is edited or
  re-recorded, no golden is edited or re-recorded, and no existing test is edited;
- each registered run is made once, and nothing is re-run to pick a result;
- no new third-party crate, `Cargo.lock` unchanged, no commit trailers.

Clean-room: the evaporation model is taken from a published paper and the breakup model from the published report of its
authors, both read for this text (the reading section); the integrated Clausius-Clapeyron relation is a textbook relation read in
a secondary source and derived here; the integration over a sub-step is registered here. No source listing of any spray or CFD
code was opened: web searches listed source pages of open-source and commercial implementations of the TAB model and theory pages
of a commercial code; none was opened. No vendor code, text or install was used. Status words as in ADR 0192 and ADR 0208:
"read" means read in the source named; "derived" means derived here; "registered here" marks a choice of this text that no source
and no instruction fixes.

## What F-245 asks

The roadmap entry: two parts, each gated on its own.

- (A1) a single droplet evaporating in quiescent hot gas follows the quasi-steady d^2 law with the evaporation constant derived in
  the ADR from the registered model, measured d^2 slope within 2 percent, and the droplet mass lost equals the vapour mass source
  delivered to the carrier to 1e-10 relative;
- (A2) the breakup model reproduces its own analytic breakup time and child-size relation for a drop given a step relative
  velocity within 2 percent, with liquid mass conserved through breakup to round-off (critical Weber number and model constants
  read from the source);
- out of scope: spray penetration against data (F-252), wall film (F-253), wall interaction (F-254), dispersion and reactions
  (F-255), group and file injections.

Readings of this text (readings, not decisions; Q1 and Q2 ask the maintainer):

- **"Quiescent hot gas"**: a carrier at rest, of uniform temperature above the droplet's, with no vapour far from the droplet;
  the droplet at rest (Re = 0).
- **"Follows the quasi-steady d^2 law"**: `d^2` falls linearly in time once the droplet temperature has settled at its
  equilibrium (wet-bulb) value; the slope is measured by a least-squares line through the tracked diameters.
- **"The evaporation constant derived in the ADR from the registered model"**: DK below, from the model of E1 to E5.
- **"The vapour mass source delivered to the carrier"**: the mass booked per cell for the caller (as the surface reaction of ADR
  0208 books its gas species); the carrier's fields are not changed by the library (Q2, Q3).
- **"Its own analytic breakup time and child-size relation"**: the breakup time that the model's own equations give for a drop
  at rest that sees a constant relative velocity from `t = 0` (DB), and the product-drop radius of its energy relation (eq. 22 of
  the source); with the paper's limits (its eqs. 12a, 13, 16, 23) as further checks.
- **"Liquid mass conserved through breakup to round-off"**: the parcel's liquid mass `n rho_l pi d^3/6` before and after a
  breakup event equal to round-off (1e-14 relative), and the total liquid plus vapour mass of a spray with breakup and
  evaporation equal to the injected mass at every step.
- **"Critical Weber number and model constants read from the source"**: `We_crit = 6` and the constants `C_F, C_k, C_d, C_b, K`
  and `C_v` of O'Rourke and Amsden (1987), read (B1).

## The reading

| Text | What was read | Used for |
| --- | --- | --- |
| P. J. O'Rourke, A. A. Amsden, "The TAB method for numerical calculation of spray droplet breakup", Los Alamos report LA-UR-87-2105-Rev. (DE87 011756), submitted to the 1987 International Fuels and Lubricants Meeting (SAE paper 872089). Scan of the report at the UNT Digital Library (`digital.library.unt.edu/ark:/67531/metadc1106123/m2/1/high_res_d/6118786.pdf`), 12 pages, SHA-256 `ccc109b4b1f663149641924b4f67fa78ec43dac5b5af5cb4050ab965af4f9bef`, pages rendered as images and read | eqs. 1 to 3 (the forced damped oscillator and its coefficients), 4 (its solution at constant relative speed; the first line of the printed equation is too faint in the scan to read and was derived from eq. 3, the second line and the definitions of `We`, `t_d`, `omega^2` are legible), 5 and 6 (`We_crit = 6`, `C_k C_b / C_F = 12`), 7 (`C_k = 8`), 8 (`C_d = 5`), 9 (`C_b = 1/2`), 10 (`C_F = 1/3`), 11, 12a (`t_bu = sqrt(3) sqrt(rho_l/rho_g) r/u`, large We), 13 (`t_bu = pi sqrt(rho_l r^3/(8 sigma))`, We near critical), 14 (`V_perp = C_v C_b r ydot`, "C_v is approximately unity"), 15, 16 (`tan(theta/2) = C_v (sqrt 3/3) sqrt(rho_g/rho_l)`), 18 to 22 (the energy argument, `r/r_32 = 1 + 8K/20 + (rho_l r^3/sigma) ydot^2 (6K - 5)/120`, "we take C_v = 1"), the text after 22 (`K = 10/3`, bag regime `r/r_32 = 7/3`), 23 (`rho_g u^2 r_32/sigma = 6` for large We), and the section "Numerical implementation": `omega^2 <= 0` gives `y = ydot = 0`; the amplitude `A^2 = (y - We/12)^2 + (ydot/omega)^2`; no breakup when `We/12 + A <= 1`; eqs. 25a, 25b (the update over a time step); the breakup time as the smallest root after `t^n` of eq. 26, `We/12 + A cos(omega (t - t^n) + phi) = 1`, `cos phi = (y - We/12)/A`, `sin phi = -ydot/(A omega)`, "assuming that the drop oscillation is undamped for its first period"; breakup "only if `t^n <= t_bu <= t^(n+1)`"; `r_32` and `V_perp` "using Eq. (22) and Eq. (14) with ydot evaluated at t_bu"; product radii drawn from a chi-squared distribution with Sauter mean radius `r_32`; `N^(n+1) = N^n (r^n/r^(n+1))^3`; the normal velocity added in a random direction in the plane normal to the relative velocity ("does not conserve momentum in detail but ... on the average"); `y = ydot = 0` after breakup | the breakup model B1 to B6 and every constant |
| A. Both, D. Mira, O. Lehmkuhl, "Evaporation of volatile droplets subjected to flame-like conditions", arXiv:2201.02818v1 (8 January 2022; preprint submitted to Int. J. Heat Mass Transfer), SHA-256 `52c7b9dd554ccd34d2150a68fbdb5717710a5bf25ada3ed1495283993c1a86c4`, text extracted with `pdftotext -layout` and read | section 2.1 (quasi-steady film theory; eqs. 14 to 16: `Nu_m,0 = 2 + 0.6 Re^(1/2) Pr^(1/3)` (Ranz-Marshall), `Sh_m,0 = 2 + 0.6 Re^(1/2) Sc^(1/3)`, `Re = rho_m abs(u_s) d_p/mu_m`, `Pr = c_p,m mu_m/lambda_m`, `Sc = mu_m/(rho_m D_m)`; the "1/3 law" of the mean state, eq. 17), section 2.2 (infinite conductivity; eqs. 19, 20: `dm_p/dt = -mdot`, `dT_p/dt = -Q/(m_p c_p,p) + (L_v/(m_p c_p,p)) dm_p/dt` with the heat `Q` received by the droplet; Raoult's law `X_eq = P_sat/P`; the frozen-chemistry surface mass fraction `Y = X/(X + (1 - X) W_b/W_f)`), 2.2.2 (the classical model, `B_M = (Y_f,i - Y_f,s)/(1 - Y_f,i)`), 2.2.3 (Bird's correction: eqs. 21 to 25, `beta = phi_m ln(1 + B_M)`, `phi_m = (c_p,vap,m/c_p,m)(1/Le_m)(Sh_m,0/Nu_m,0)`, `Le_m = Sc_m/Pr_m`, `Nu_m,B = beta/(e^beta - 1) Nu_m,0`, `1 + B_T = (1 + B_M)^phi_m`, eq. 24), Table 3 row B (`dm_p/dt = -pi d_p rho_m D_m Sh_m,0 ln(1 + B_M)`, `dT_p/dt = pi d_p lambda_m Nu_m,0/(m_p c_p,p) (T_s - T_p) ln(1 + B_T)/B_T + (L_v/(m_p c_p,p)) dm_p/dt`), section 3.1 and Table 4 (psychrometric wet bulb `dT_p/dt = 0`; row B: `c_p,vap,m (T_s - T_p) = B_T L_v`); the boiling point of n-heptane 371.5 K (introduction) | the evaporation model E1 to E5 and the wet-bulb relation of DK |
| P. John, H. Chandrasekhar, S. Saha, O. Owoyele, "A liquid-fueled reactor network model for enhanced NOx prediction in gas turbine combustors", arXiv:2510.13033v2, SHA-256 `7f4dc3615150dd83a1e8a1d99435f7173c619ebe08a89732fc4b04b655d64cd3`, text extracted with `pdftotext -layout` | section 2.2.1 (the TAB constants `C_k = 8, C_f = 1/3, C_d = 5, C_b = 1/2`, `K = 10/3`, eq. 13 for the product radius), sections 2.2.2 and 2.2.3 (a Frossling form `dr/dt = -rho_g D B_d Sh_d/(2 rho_l r)` with `Sh_d = (2 + 0.6 Re^(1/2) Sc^(1/3)) ln(1 + B_d)/B_d`, the same surface mass fraction, a heat transfer with `ln(1 + B_d)/B_d` on the Nusselt number), section 3.1 ("the classical D2-law") | a second, secondary reading of the TAB constants and eq. 22 (agree with the report); its heat-transfer correction differs from Bird's and is not used |
| Wikipedia, "Clausius-Clapeyron relation", read 2026-11-08 | the ideal-gas approximation `dP/dT = P L/(R T^2)` (vapour volume much larger than the liquid's, ideal-gas vapour) and its integrated form `ln(P2/P1) = -(L/R)(1/T2 - 1/T1)` for a constant `L` (molar or specific values, used consistently) | E2 (secondary; the integration is derived) |
| NIST CODATA 2022, `physics.nist.gov/cgi-bin/cuu/Value?r` and `?na`, read 2026-11-08 | `R = 8.314 462 618... J mol^-1 K^-1` (exact), `N_A = 6.022 140 76 x 10^23 mol^-1` (exact); with `k = 1.380 649 x 10^-23 J/K` (exact, already in `docs/reference-data.md`) | the gas constant `R = N_A k = 8.31446261815324` |

Discrepancy noted, nothing changed: the text extraction of arXiv:2510.13033 prints the breakup test as "A + We_g > 1.0"; the
report has `We/12 + A > 1` (the factor `C_F/(C_k C_b) = 1/12` lost in the extraction or the text). The report is followed.

## What the code read shows (main c201a770)

1. **The sub-step loop** (`tracker::advance_particle`): per sub-step, the eddy of the random walk (when on) and the seen velocity
   `u_start` at the start position; the sub-step `h` limited by the Courant fraction, the drag relaxation (explicit schemes), the
   snap `rem - h <= 1e-12 rem` and the eddy's remaining time; the motion `step_particle`; the cell walk. Inside the cell: the heat
   step, the exchange booking, the position and age update, the eddy clock, the surface reaction. At a boundary hit: the hit time
   by bisection, the same updates over `h_hit`, then erosion and the wall action. The loop ends when `rem` is spent or the parcel
   is no longer active; more than 5 000 000 sub-steps in one call is an error.
2. **Heat transfer** (ADR 0057): `Thermal {cp_p, k_f, cp_f, t_fluid}`; a parcel's temperature (aux `tp`) starts at the injector's
   `InjectorExt::temperature` or, when absent, at `t_fluid` of its cell at its first sub-step; `heat_step` relaxes it towards
   `t_fluid` of the start cell with the Ranz-Marshall Nusselt number on `Re = rho d abs(u_start - v0)/mu` and `Pr = mu cp_f/k_f`,
   exactly over the sub-step (`expm1`).
3. **The surface reaction** (ADR 0208) changes the diameter after the motion, books per cell for the caller, and is refused with
   heat transfer and the exchange record; its composition per real particle would not survive a change of the parcel count.
4. **States**: `State {Active, Deposited, Escaped, Stuck, Film}` (`repr` values 0 to 4, written as integers to `particles.vtp`);
   `rcfd-api::particle_run::state_name` matches the enum exhaustively; `Tracker::count(state)`.
5. **Fingerprint**: the parcels, then the film, erosion, rough-wall and reaction parts when set.
6. **Drag law `None`**: a ballistic parcel; with no gravity a parcel released at rest stays at rest, so the relative velocity it
   sees in a uniform gas is constant (the step relative velocity of A2). A parcel at rest has no Courant limit, so each
   `advance` call is one sub-step for it.
7. **The case tree** builds a `Thermal` for `particles.heat_transfer` (`particles.rs`); it has no carrier vapour species coupled
   to particles and no key for evaporation or breakup.

## The model

### Evaporation (Both, Mira and Lehmkuhl 2022, Bird's correction "B"; library only)

**E1. State and inputs.** With evaporation on, a parcel carries its diameter `d`, its temperature `T_p` (the temperature of ADR
0057) and its count `n`; its liquid density `rho_l` is the parcel density and stays constant. Carrier, per cell `c`: the seen
temperature `T_s = Thermal::t_fluid[c]` and the seen vapour mass fraction `Y_s[c]` (given by the caller, in `[0, 1)`); the
mean-state properties (the caller applies the 1/3 law, eq. 17): `rho_m = Flow::rho`, `mu_m = Flow::mu`,
`lambda_m = Thermal::k_f`, `c_p,m = Thermal::cp_f`; the liquid specific heat `c_l = Thermal::cp_p`. Spec (`EvaporationSpec`):
the latent heat `L_v`, the boiling temperature `T_b` at the pressure `p_b`, the carrier pressure `p`, the vapour and bath-gas
molar masses `W_f`, `W_b`, the vapour diffusivity `D_m`, the vapour specific heat `c_p,vap`, the diameter `d_min` at which a
droplet counts as evaporated, and the sub-step fraction `f`. All are constants (no temperature dependence).

**E2. Surface state.** `X(T) = (p_b/p) exp((L_v W_f/R)(1/T_b - 1/T))` (the Clausius-Clapeyron relation with a constant latent
heat, integrated from the boiling point at `p_b`; Raoult's law for a pure liquid), `R = 8.31446261815324`. The surface mass
fraction `Y_i = X/(X + (1 - X) W_b/W_f)` (frozen chemistry). `X >= 1` (the droplet at or above its boiling point at the carrier
pressure) is outside the model: `advance` returns an error (Q5). `B_M = (Y_i - Y_s)/(1 - Y_i)` and `L1 = ln(1 + B_M)` (> -1 since
`Y_s < 1`; negative for condensation).

**E3. Transfer numbers** (eqs. 14 to 16). `Re = rho_m d abs(u_start - v0)/mu_m` with the seen velocity of the sub-step's start (as
`heat_step`), `Pr = mu_m c_p,m/lambda_m`, `Sc = mu_m/(rho_m D_m)`, `Nu0 = 2 + 0.6 Re^(1/2) Pr^(1/3)`,
`Sh0 = 2 + 0.6 Re^(1/2) Sc^(1/3)`.

**E4. Rates** (Table 3 row B, eqs. 19, 22 to 24). Per real particle: mass lost per second `mdot = pi d rho_m D_m Sh0 L1`;
`phi = c_p,vap rho_m D_m Sh0/(lambda_m Nu0)`, `beta = phi L1`, Bird's factor `g(beta) = beta/(e^beta - 1)` (`g(0) = 1`;
`= ln(1 + B_T)/B_T` with `1 + B_T = (1 + B_M)^phi`); heat received `Q = pi d lambda_m Nu0 g(beta) (T_s - T_p)`;
`m c_l dT_p/dt = Q - mdot L_v`, `dm/dt = -mdot`, `m = rho_l pi d^3/6`.

**E5. The d^2 form (derived).** `dm/dt = (pi rho_l/4) d d(d^2)/dt`, so `d(d^2)/dt = -K_loc`, `K_loc = 4 rho_m D_m Sh0 L1/rho_l`.

**E6. Integration over a sub-step (registered here).** Every coefficient is taken at the start of the sub-step (`T_p`, `d`,
`Re`, the start cell) and held over its length `h` (or `h_hit` at a wall hit):

- diameter: `d_new^2 = d^2 - K_loc h` (exact for the frozen `K_loc`);
- temperature, the exponential Euler step: `T_new = T_p + h phi1(J h) F`, `F = (Q - mdot L_v)/(m c_l)`, `J = dF/dT_p` (analytic,
  with `d`, `Re` and `m` held), `phi1(z) = (e^z - 1)/z`, `phi1(0) = 1`. It is exact for `F` linear in `T_p`, stable for any `h`
  when `J < 0`, and its fixed point is `F = 0`, the wet bulb, exactly (a droplet at its wet bulb stays there);
- booking: `dm = (pi rho_l/6)(d^3 - d_new^3)` per real particle; the start cell receives `n dm` kg of vapour, and the total
  `evaporated` grows by `n dm`;
- sub-step limits, after the limits of item 1 and the eddy limit: `h <= f tau_T`, `tau_T = m c_l/(pi d lambda_m Nu0 g(beta))`;
  and when `K_loc > 0` and `d^2 - K_loc h <= d_min^2`, `h` is cut to `h_c = max(0, (d^2 - d_min^2)/K_loc)` and the parcel
  completes at the end of that sub-step if it runs to its end (no wall hit): its whole remaining liquid mass
  `n (pi rho_l/6) d^3` (the diameter of the sub-step's start) is booked, its diameter is set to 0 and its state to the new
  `State::Evaporated`; its temperature takes the step above;
- the step replaces `heat_step` for every parcel while evaporation is on.

**E7. Where.** After the motion of each sub-step, in place of `heat_step`; before the position update; the breakup (B3) after it.
Erosion and the film absorption at a wall hit use the parcel mass after the evaporation of that sub-step.

**E8. Limits of use (registered here).** `set_evaporation` needs heat transfer (`set_thermal` first) and is refused with the
surface reaction; the reaction is refused with evaporation; `advance` refuses evaporation with the exchange record (the energy and
mass booking of an evaporating parcel in the record is not designed: Q3).

### Breakup (O'Rourke and Amsden 1987, the TAB method; library only)

**B1. The equation and the constants** (eqs. 3, 6 to 10, 14, 22). `ydd = (C_F/C_b)(rho_g/rho_l)(u^2/r^2) -
(C_k sigma/(rho_l r^3)) y - (C_d mu_l/(rho_l r^2)) yd`, breakup when `y` reaches 1; `We = rho_g u^2 r/sigma`,
`W = C_F We/(C_k C_b)` (`We/12`), `1/t_d = C_d mu_l/(2 rho_l r^2)`, `omega^2 = C_k sigma/(rho_l r^3) - 1/t_d^2`. Constants of the
source: `C_F = 1/3`, `C_k = 8`, `C_d = 5`, `C_b = 1/2`, `K = 10/3`, `C_v = 1`; `We_crit = C_k C_b/(2 C_F) = 6`. Inputs:
the surface tension `sigma` and the liquid viscosity `mu_l` (spec), `rho_g = Flow::rho`, `rho_l` the parcel density, `u` the
relative speed, `r = d/2`. The spec carries the six constants (`BreakupSpec::tab(sigma, mu_l, seed)` gives the source's) and a
seed for the breakup generator.

**B2. State.** Per parcel `y` and `yd`, 0 at injection and after a breakup; a breakup count and the age at the last breakup.

**B3. Per sub-step (the source's numerical implementation, with the tracker's sub-step as its time step; registered here where
noted).** At the start of the sub-step: `u = abs(u_start - v0)` (the seen velocity, with the eddy fluctuation when the random walk
is on, as the drag), `r = d/2`; coefficients held over the sub-step.

- `omega^2 <= 0`: `y = yd = 0` at the end of the sub-step; no breakup.
- Otherwise `A^2 = (y - W)^2 + (yd/omega)^2`. `W + A <= 1`: no breakup in this sub-step.
- Otherwise the time to breakup `s` is the smallest `s >= 0` with `W + A cos(omega s + phi) = 1` (eq. 26), `cos phi = (y - W)/A`,
  `sin phi = -yd/(A omega)`. In closed form (derived): with `alpha = arccos((1 - W)/A)` in `[0, pi]` and `phi` in `(-pi, pi]`,
  `s = 0` if `y >= 1` (registered here: a drop at or past the threshold breaks at once), else `s = (-alpha - phi)/omega` when
  `phi < -alpha` and `s = (2 pi - alpha - phi)/omega` when `phi > alpha`.
- When `s <= h`, `h` is cut to `s` and the breakup is due at the end of the sub-step (registered here: the sub-step ends at the
  breakup time, so the breakup happens at `t_bu` and not at the end of a time step).
- After the motion and the evaporation of the sub-step: if the breakup is due, the sub-step ran to its end (no wall hit) and the
  parcel is still active, the breakup B4 is applied; otherwise `(y, yd)` are advanced over the sub-step's length by eqs. 25a, 25b
  (`omega^2 > 0`) or set to 0 (`omega^2 <= 0`).

**B4. The breakup.** `yd_b = yd(s)` from eq. 25b with the held coefficients (registered here: the source evaluates `yd` "at
`t_bu`" without saying which of its two solutions; Q7). `r = d/2` after this sub-step's evaporation;
`r_32 = r/(1 + 8K/20 + rho_l r^3 yd_b^2 (6K - 5)/(120 sigma))` (eq. 22, `y = 1`); every product drop of the parcel gets the radius
`r_32` (registered here; the source draws product radii from a chi-squared distribution with Sauter mean radius `r_32`: Q4); the
count becomes `n (r/r_32)^3` (the source's mass rule) and `d = 2 r_32`; the velocity gains `C_v C_b r yd_b (cos psi e1 +
sin psi e2)`, `(e1, e2)` the unit pair normal to the relative velocity of the sub-step (`rough::tangent_pair` of ADR 0194),
`psi = 2 pi U`, `U` one uniform draw of the breakup generator per breakup (drawn also when `C_v = 0`); then `y = yd = 0`, the
parcel's breakup count grows by one and its breakup age is its age.

**B5. Limits of use (registered here).** `set_breakup` is refused with the surface reaction and the reaction with breakup; `advance`
refuses breakup with the exchange record (the normal velocity changes the parcel momentum without a carrier counterpart, which
the source accepts "on the average"; Q3). Breakup with evaporation, the film, erosion, rough walls and the random walk is allowed.

**B6. Validation of the spec (registered here).** `sigma` finite and positive; `mu_l` finite and `>= 0`; `C_F, C_k, C_b` finite and
positive; `C_d, C_v` finite and `>= 0`; `K >= 5/6` and finite (so that the `yd^2` term of eq. 22 is not negative and `r_32 < r`).

### Derived relations

**DK. The evaporation constant (A1).** For a droplet at rest in a gas at rest, `Re = 0`, `Nu0 = Sh0 = 2`, so `phi = c_p,vap rho_m
D_m/lambda_m`. Both `Q` and `mdot` are proportional to `d`, so the wet-bulb temperature `T_wb` (`F = 0`) does not depend on `d`:
`pi d lambda_m 2 g(beta)(T_s - T) = pi d rho_m D_m 2 L1 L_v` and, with `g(beta) = phi L1/B_T`, `c_p,vap (T_s - T_wb) =
B_T(T_wb) L_v` (Table 4 row B of the source, read; derived again here). Its left side falls and its right side rises with `T`,
so the root is unique below the boiling point at `p`. At `T_p = T_wb` the temperature does not change (E6), `L1` is constant, and
`d(d^2)/dt = -K` for the whole life with

    K = 8 rho_m D_m ln(1 + B_M(T_wb))/rho_l = 8 lambda_m ln(1 + B_T(T_wb))/(rho_l c_p,vap)

(the two forms are equal through `1 + B_T = (1 + B_M)^phi`): the quasi-steady d^2 law, `d^2 = d0^2 - K t`, and the time to
`d_min` is `(d0^2 - d_min^2)/K`. A droplet that starts colder heats towards `T_wb` with the time scale `tau_T`, which shrinks with
`d^2`; after the heat-up the slope is `K`.

**DB. The breakup relations (A2).** From rest (`y = yd = 0`) at a constant relative speed: `A = W`, `phi = pi`, so eq. 26 gives
`cos(omega t_bu) = 1 - 1/W`,

    t_bu = arccos(1 - 1/W)/omega,    W = C_F We/(C_k C_b),

which exists if and only if `W > 1/2`, that is `We > We_crit = C_k C_b/(2 C_F) = 6` (eq. 6). With `mu_l = 0` the undamped form is
exact: `yd(t_bu) = W omega sin(omega t_bu) = W omega sqrt(2/W - 1/W^2)`, `rho_l r^3 yd^2/sigma = C_k (2W - 1)`, and eq. 22 gives

    r/r_32 = 1 + 8K/20 + C_k (2W - 1)(6K - 5)/120  =  4/3 + We/6   (the source's constants).

Limits (the source's own): `We -> 6+`: `t_bu -> pi/omega = pi sqrt(rho_l r^3/(8 sigma))` (eq. 13), `r/r_32 -> 7/3`;
`We -> infinity`: `t_bu -> sqrt(3) sqrt(rho_l/rho_g) r/u` (eq. 12a), `r/r_32 -> We/6` (eq. 23), and the normal speed over the
relative speed `C_v C_b r yd/u -> C_v (sqrt 3/3) sqrt(rho_g/rho_l)` (eq. 16). The child of a drop broken from rest has
`We_child = We r_32/r = 6 We/(8 + We) < 6`: with `C_v = 0` it does not break again at the same relative speed. With `mu_l > 0`,
eq. 26 still gives `t_bu = arccos(1 - 1/W)/omega` with the damped `omega` (the source's undamped-amplitude assumption), and `yd_b`
is the derivative of eq. 4 at `t_bu`.

## Settings and code

**S1. Library (`rcfd-particles`).**

- module `evaporation`: `R_UNIVERSAL`; `EvaporationSpec` (E1, with `validate`); `Evaporation` with `spec()`,
  `vapour_fraction() -> &[f64]`, `source() -> &[f64]` (kg of vapour per cell since the start), `evaporated()` (kg),
  `completed()` (parcels that reached `d_min`), `surface_mole_fraction(T)`, `surface_mass_fraction(T)`, and
  `rates(...) -> EvapRates` (`mdot`, `k`, `h_eff`, `f`, `jac`, `tau_t`, `b_m`, `nu`, `sh`) for the state of one parcel.
- module `breakup`: `BreakupSpec` (B1, B6; `BreakupSpec::tab(sigma, mu_l, seed)`), the TAB functions (coefficients, time to
  breakup, the update 25a/25b, `r_32`, the normal speed), `Breakup` with `spec()` and `events()`.
- `Tracker::set_evaporation(spec, y_s)`, `set_evaporation_vapour(&[f64])`, `evaporation()`; `Tracker::set_breakup(spec)`,
  `breakup()`, `distortion(i) -> (y, yd)`, `breakups(i) -> (count, age of the last)`. Refusals as E8, B5, B6 and: a second
  `set_evaporation` or `set_breakup`; `y_s` not one finite value in `[0, 1)` per cell; `set_evaporation_vapour` without
  evaporation.
- `State::Evaporated = 5` (new variant; `count`, `particles.vtp` writes 5).
- `fingerprint`: with evaporation set, after everything it holds today, the vapour sources, `evaporated`, `completed` and the
  parcels' temperatures; with breakup set, `events` and per parcel `y`, `yd`, the count and the age of the last breakup. Without
  them, unchanged.
- Unchanged: `Particle`, `Flow`, `Options`, `Thermal`, `InjectorSpec`, `InjectorExt`, every existing result.

**S2. Case tree: none (Q2).** The run path cannot hand a vapour source to a species equation or take the carrier temperature and
vapour fraction from it today, so keys alone would give a tracker that evaporates into a carrier that does not receive the vapour.
No key, no schema change, no hash or golden is touched.

**S3. rcfd-api.** One arm `State::Evaporated => "evaporated"` in `particle_run::state_name` (an exhaustive match; not reachable
from a case).

## Registered checks

All tests run in release. Library checks in `crates/rcfd-particles/tests/evaporation_f245.rs` (A, C) and
`crates/rcfd-particles/tests/breakup_f245.rs` (B, D2, E2). "Relative" is `abs(a - b)/abs(b)`. Each test computes its reference
with its own code (its own Clausius-Clapeyron, surface mass fraction, Spalding numbers, bisection, closed forms), not with the
library's functions.

### A. Evaporation (A1)

Common data E (round test inputs of the order of n-heptane in hot air at one atmosphere; not data for a real substance, except
the boiling point 371.5 K quoted from Both et al.): `rho_l = 700`, `c_l = 2200`, `L_v = 3.2e5`, `T_b = 371.5` K at
`p_b = 101325` Pa, `p = 101325` Pa, `W_f = 0.1`, `W_b = 0.029`, `rho_m = 0.5`, `mu_m = 3e-5`, `lambda_m = 0.05`, `c_p,m = 1100`,
`c_p,vap = 2500`, `D_m = 3e-5`, `d_min = 1e-7` m, `f = 0.1`. Mesh `box_mesh([1, 1, 1], [0.01, 0.01, 0.01])`; options: Stokes drag,
the analytic scheme, cell-centre interpolation, no gravity; flow at rest; `Thermal {cp_p: 2200, k_f: 0.05, cp_f: 1100, t_fluid:
[800]}`; `Y_s = [0]`; one injector at the centre, `rate = 1e4`, `t_end = 1e-4` (one parcel, released at rest at 1e-4 s, the end
of the first step), `d0 = 5e-5` m, density 700, `parcel_count = 1000`, the injection temperature as stated; steps of `1e-4` s
until the parcel's state is `Evaporated` (at most 200 steps). The test's `T_wb` is the bisection root (200 halvings on
`[250, 371.5 - 1e-9]`) of `c_p,vap (T_s - T) - B_T(T) L_v`, and `K = 8 rho_m D_m ln(1 + B_M(T_wb))/rho_l` (DK). The samples are
`(age, d^2, T_p)` of the parcel after each step while it is active.

| Id | Check | Limit |
| --- | --- | --- |
| EV1 (acceptance A1, slope) | injected at `T_wb`: the least-squares slope of `d^2` against the age over the samples with `0.05 <= d^2/d0^2 <= 0.95`, against `-K` | 2 percent |
| EV1b | the same samples: the largest absolute residual of that line | `1e-3 d0^2` |
| EV1c | `T_p` at every sample | within `1e-6` K of `T_wb` |
| EV1d | the parcel completes: state `Evaporated`; its age then against `(d0^2 - d_min^2)/K` | as stated; 2 percent |
| EV1e (acceptance A1, mass) | at the end of every step: the summed vapour sources against `n (pi rho_l/6)(d0^3 - d^3)` (`d = 0` after completion); `evaporated()` against the summed sources | 1e-10 relative; 1e-12 relative |
| EV2 (heat-up) | injected at 300 K: the slope over the samples with `0.05 <= d^2/d0^2 <= 0.25` against `-K`; `T_p <= T_wb + 1e-3` K at every sample; `abs(T_p - T_wb) <= 1e-4` K at the last sample; the mass check of EV1e at every step | 2 percent; as stated; as stated; 1e-10 relative |
| EV3 (no driving force) | `T_s = 330` K, injected at 330 K, `Y_s = Y_i(330 K)` (the test's), 100 steps: `d` against `d0`; the summed sources | 1e-9 relative; at most `1e-9 n m0` in magnitude |
| EV4 (convection) | drag law `None`, the gas at `(20, 0, 0)` m/s (`Re0 = 16.667`), injected at the test's wet bulb for `Re0` (`Nu0`, `Sh0`, `phi` of E3, E4 at `Re0`), steps of `1e-5` s (the parcel released at `1e-5` s): after the second step, `(d0^2 - d^2)/age` against `K_loc = 4 rho_m D_m Sh0 ln(1 + B_M(T_wb))/rho_l` at `Re0` | 2 percent |
| EV5 (the Jacobian) | `rates` at `T_p` = 300, 330, 356, 365 K, `Re` = 0 and 16.667, `d = 5e-5`: `jac` against the central difference `(f(T + 1e-3) - f(T - 1e-3))/2e-3` of the library's `f` | 1e-5 relative |

### B. Breakup (A2)

Common data B: mesh `box_mesh([1, 1, 1], [1, 1, 1])`; options: drag law `None`, the analytic scheme, cell-centre interpolation, no
gravity; `Flow {u: [(U, 0, 0)], grad: None, rho: 1.0, mu: 1.8e-5}`; `sigma = 0.02`, `rho_l = 700`, `d0 = 2e-4` (`r0 = 1e-4`),
`parcel_count = 100`; `U = sqrt(We sigma/(rho_g r0))`; one injector at the centre, `rate = 1e5`, `t_end = 1e-5` (one parcel at
rest at `1e-5` s), steps of `1e-5` s, 60 steps unless stated; `BreakupSpec::tab(0.02, mu_l, 7)` with `C_v` as stated.
`omega0^2 = 8 sigma/(rho_l r0^3)`. "The breakup" is read from `breakups(0)` (count, age) and the parcel after the run.

| Id | Check | Limit |
| --- | --- | --- |
| BU1 (acceptance A2) | `mu_l = 0`, `C_v = 0`, `We` in {6.1, 12, 60, 1000}: exactly one breakup; its age against `arccos(1 - 12/We)/omega0`; the child radius `d/2` against `r0/(4/3 + We/6)`; the count against `100 (4/3 + We/6)^3`; `n d^3` after against `100 d0^3`; the velocity after | one; 2 percent; 2 percent; 1e-12; 1e-14 relative; zero, bit for bit |
| BU2 (critical Weber number) | `mu_l = 0`, `We = 5.9`, 500 steps (12 periods): no breakup (count 0, `d` and `n` bit for bit); the largest sampled `y` (one sample per step) | as stated; in `[0.9, 1]` |
| BU3 (viscous) | `mu_l = 0.03`, `We = 12`, `C_v = 0`, the parcel released at `1e-8` s (`rate = 1e8`, `t_end = 1e-8`) and one step of `3e-4` s: one breakup; its age against `arccos(1 - 1/W)/omega`, `omega^2 = omega0^2 - (5 mu_l/(2 rho_l r0^2))^2`; the child radius against eq. 22 with `yd` the derivative of eq. 4 (from rest) at that time | one; 2 percent; 2 percent |
| BU3s (record) | BU3 with steps of `1e-5` s: the breakup count and age | recorded, no limit (the source's per-step re-estimate under damping) |
| BU3o (overdamped) | `mu_l = 0.05`, `We = 60`, 100 steps: no breakup; `y = yd = 0` after every step | bit for bit |
| BU4 (the source's limits) | `mu_l = 0`, `C_v = 1`. `We = 1000`: the age against `sqrt(3) sqrt(rho_l/rho_g) r0/U` (eq. 12a); `r0/r_child` against `We/6` (eq. 23); the velocity change `dv`: `abs(dv)` against `C_b r0 yd_b`, `yd_b = W omega0 sqrt(2/W - 1/W^2)` (eq. 14); `abs(dv . x)`; `abs(dv)/U` against `(sqrt 3/3) sqrt(rho_g/rho_l)` (eq. 16). `We = 6.001`: the age against `pi sqrt(rho_l r0^3/(8 sigma))` (eq. 13); `r0/r_child` against 7/3. Both: exactly one breakup | 2 percent; 2 percent; 1e-12; at most `1e-12 abs(dv)`; 2 percent; 2 percent; 2 percent; one |

### C. A spray with evaporation and breakup (A1, A2 mass)

Scenario SP: mesh `box_mesh([8, 2, 2], [0.08, 0.02, 0.02])`; options: Schiller-Naumann drag, the analytic scheme, cell-centre
interpolation, gravity `(0, 0, -9.81)`, default wall `Reflect(0.8)`, overrides `xmax` escape and `zmin` stick; gas per cell
`(30, 0, -2)` m/s, `rho = 0.5`, `mu = 3e-5`; `Thermal {2200, 0.05, 1100, t_fluid = 700 + 1250 x_c}`; evaporation spec E with
`Y_s = 0.05 x_c/0.08` per cell (`x_c` the cell centre); `BreakupSpec::tab(0.01, 4e-4, 31)` (`C_v = 1`). Injectors: `a` at
`(0.005, 0.012, 0.012)`, `rate = 2e4`, `t` in `[0, 0.004]` (80 parcels), at rest, `d = 4e-4`, density 700, `parcel_count = 20`,
seed 21, injection temperature 300 K; `b` at `(0.005, 0.008, 0.008)`, `rate = 2e4`, `t` in `[0.001, 0.005]` (80 parcels), at
rest, `d = 3e-5`, density 700, `parcel_count = 1000`, seed 22, 320 K. 300 steps of `1e-4` s. `M_inj` is the sum over the parcels
of `n0 (pi rho_l/6) d0^3` with the injector's `n0` and `d0`; `M_liq = sum n (pi rho_l/6) d^3` over all parcels; `S` the summed
vapour sources.

| Id | Check | Limit |
| --- | --- | --- |
| SP1 (acceptance A1, mass, many cells) | at the end of every step with `M_inj > M_liq`: `S` against `M_inj - M_liq` | 1e-10 relative |
| SP2 (acceptance A2, mass) | at the end of every step: `M_liq + S` against `M_inj` | 1e-12 relative |
| SP3 (coverage) | at the end: every parcel of `a` broke up at least once; at least one parcel `Evaporated`; at least one `Escaped`; the counts per state recorded | as stated |

### D. Refusals, inert paths and defaults

| Id | Check | Limit |
| --- | --- | --- |
| RF1 | evaporation refusals (each `Err`): without `set_thermal`; with a reaction set; a second `set_evaporation`; `L_v = 0`; `T_b = 0`; `p = 0`; `p_b < 0`; `W_f = 0`; `W_b = -0.029`; `D_m = 0`; `c_p,vap = 0`; `d_min = 0`; `f = 0`; `f = 1.5`; a NaN in the spec; `y_s` one value short; `y_s = [1.0]`; `y_s = [-0.1]`; `set_evaporation_vapour` without evaporation; `set_reaction` after `set_evaporation`; `advance` with evaporation and the exchange record; `advance` of a droplet injected at 380 K into `p = 101325` (above `T_b`). Controls (`Ok`): spec E with `[0.0]`; `set_evaporation_vapour(&[0.01])` after it | as stated |
| RF2 | breakup refusals (each `Err`): `sigma = 0`; `sigma = NaN`; `mu_l = -1e-3`; `C_F = 0`; `C_k = 0`; `C_b = 0`; `C_d = -1`; `C_v = -1`; `K = 0.5`; a second `set_breakup`; `set_breakup` with a reaction set; `set_reaction` with breakup set; `advance` with breakup and the exchange record. Control (`Ok`): `BreakupSpec::tab(0.02, 1e-3, 7)` | as stated |
| IN1 (inert breakup) | the RX2 scenario of ADR 0208 (`reaction_f255.rs`, without the reaction) with `set_breakup(tab(1e6, 1e-3, 5))` (every Weber number below `1e-6`): the fingerprint of the run without breakup against the leading words of the run with it | bit for bit |
| DF1 | the existing suites, unedited: `rcfd-particles` `closed_forms`, `lagrangian_ext`, `wall_film` (R1), `film_gas`, `wall_interaction` (R3), `erosion_presets`, `dispersion_f255`, `reaction_f255`; `rcfd-api` `particles_case`, `particles_ext_case`, `particle_run`, `particles_two_way`, `wall_film_f253`, `wall_interaction_f254`, `erosion_presets_f254`, `film_gas_f292`, `dispersion_f255`, H1 of `case_hashes_w48u`, `baseline_hashes` | pass (R1 to R6 compared on Windows) |

### Negative controls

| Id | Check | Limit |
| --- | --- | --- |
| NE1 | EV1's slope against `K` of the classical model (Bird's factor `g = 1`) at its own wet bulb (the test's bisection of `lambda_m (T_s - T) = rho_m D_m L1(T) L_v`) | `>= 50` percent off |
| NE2 | EV1's slope against `8 rho_m D_m B_M(T_wb)/rho_l` (`B_M` in place of `ln(1 + B_M)`) | `>= 50` percent off |
| NB1 | BU1 at `We = 12`: the age against the half period `pi/omega0` (eq. 13, the near-critical form) | `>= 40` percent off |
| NB2 | BU1 at `We = 60`: `r0/r_child` against the bag value 7/3 | `>= 50` percent off |
| NB3 | BU3: the age against the inviscid `arccos(1 - 1/W)/omega0` | `>= 20` percent off |

### Mutations (scratch, not committed)

Each is applied alone to the final tree by an exact text replacement, the named test is run once in release, and it must fail;
the file is restored and checked byte for byte against a kept copy. Their numbers go in the addendum. `scripts/limit_audit.py` is
run on the new test files and every hit is read.

| Id | Mutation | Must fail |
| --- | --- | --- |
| ME1 | `ln(1 + B_M)` replaced by `B_M` in the library's `L1` | EV1 |
| ME2 | Bird's factor `g(beta)` replaced by 1 | EV1 |
| ME3 | the Clausius-Clapeyron exponent `1/T_b - 1/T` replaced by `1/T - 1/T_b` | EV1 |
| ME4 | the vapour booked without the parcel count `n` | EV1e |
| ME5 | at completion the remaining mass not booked | EV1e |
| ME6 | `Sh0` with `Pr` in place of `Sc` | EV4 |
| MB1 | the parcel count not rescaled at a breakup | BU1 |
| MB2 | `(6K - 5)/120` replaced by `(6K - 5)/12` in eq. 22 | BU1 |
| MB3 | `W = C_F We/(C_k C_b)` replaced by `C_F We/C_k` | BU1 |
| MB4 | `omega^2` without the term `- 1/t_d^2` | BU3 |
| MB5 | the normal speed without `C_b` | BU4 |

### The gate

`cargo fmt` per package, `cargo clippy --workspace --all-targets -- -D warnings`, and every test in release on a `git archive`
copy of the gated commit, one process per test, three jobs (`scripts/gate_test.py`), in the target dir
`E:/projects/cfd/w49i-work/target-f245-gate`, taken in turn through the machine's gate lock and started with at least 19 GB of
physical memory free. A failure of a known flaky test is run again alone on the same build and reported with its name.

### Classes

DF1 and IN1 are preconditions of landing: a miss means the change is not merged and the cause is recorded. A miss of any other
limit is reported with its number and kept as an ignored `*_known_miss` test with the limit intact. A negative control that does
not hold, or a mutation that does not make its check fail, means the check is not shown to be able to fail: it is reported as a
defective set-up, corrected by an amendment before any re-run, and never counted as a pass.

## Predictions (mine; the Python model of DK and DB, not rcfd code)

| Id | Prediction |
| --- | --- |
| PR1 | DK for data E: `T_wb = 356.5724920` K, `X = 0.648102`, `Y_i = 0.863960`, `B_M = 6.350805`, `phi = 0.75`, `B_T = 3.464277`, `K = 3.41967398e-7` m^2/s (both forms of DK equal to 10 digits); lifetime of 50 um to `d_min` 7.311 ms (73 steps); `tau_T/(d0^2/K) = 2.03` |
| PR2 | EV1 slope and EV1d at round-off (below 1e-10), EV1b below `1e-12 d0^2`, EV1c below 1e-9 K, EV1e below 1e-13 |
| PR3 | EV2: the continuous model reaches `T_wb - 0.1` K at `d^2/d0^2 = 0.71`, `T_wb - 1e-4` K at 0.46; its slope over `[0.05, 0.25]` is `K (1 - 5e-12)` and over `[0.05, 0.95]` `K (1 - 0.98 percent)`; the tracker within 1e-5 of `K` on `[0.05, 0.25]`; `T_p` approaches `T_wb` from below |
| PR4 | EV3 at round-off; EV4: `Re0 = 16.667`, `Sc = 2`, `Pr = 0.66`, `Sh0 = 5.0862`, `Nu0 = 4.1327`, `T_wb(Re0) = 350.874315` K, `K_loc = 7.11305574e-7`, the tracker within 2e-3 (the diameter shrinks by 0.4 percent of `d^2` per step); EV5 below 1e-7 |
| PR5 | BU1: `U` = 34.928, 48.990, 109.545, 447.214 m/s; `t_bu` = 1.9081251e-4, 1.0389841e-4, 4.2563598e-5, 1.0257225e-5 s; `r0/r_32` = 2.35, 3.3333, 11.3333, 168 exactly in the closed form; the tracker at round-off (the breakup age is a sum of step lengths and `s`) |
| PR6 | BU2: the sampled maximum of `y` between 0.97 and 0.98333 (`2W = 0.98333`); BU3: `1/t_d = 10714`, `omega = 10666.56`, `t_bu = 1.4726362e-4` s, `r0/r_32 = 2.41894`, `y(t_bu) = 0.7927`; BU3s later than BU3 (the re-estimate from a damped state); BU3o: `omega^2 = -9.03e7` |
| PR7 | BU4: `We = 1000`: 12a 0.10 percent off, `We/6` 0.80 percent off, eq. 16 0.30 percent off, `abs(dv)` = 9.72968 m/s; `We = 6.001`: eq. 13 0.82 percent off, 7/3 0.007 percent off |
| PR8 | NE1: classical wet bulb 370.1123 K, `K` +124.5 percent; NE2 +218 percent; NB1 50 percent; NB2 +386 percent; NB3 +41.7 percent |
| PR9 | SP1 and SP2 below 1e-13; SP3: each `a` parcel breaks once (`We = 9.04`, `t_bu = 5.04e-4` s, `r/r_32 = 2.84`); a rough continuous model (`sp_check.py`, scratch) has a `b` drop evaporate 2.2 ms after its release at `x = 0.056`, and an `a` child escape at `xmax` 7.1 ms after its breakup with 0.89 of its diameter, at `z = 0.0068` |
| PR10 | the new library tests take a few seconds in release |

## Not delivered

The case-tree keys (Q2); feeding the vapour, the heat and the momentum of evaporating or breaking parcels to the carrier, and the
exchange record with either model (Q3); the chi-squared product-size distribution of the source (Q4); boiling and flash boiling
(Q5); the Abramzon-Sirignano film correction and the Langmuir-Knudsen non-equilibrium model (Q6); temperature-dependent liquid and
gas properties and the 1/3 law inside the library (the caller gives mean-state values); multicomponent droplets; any other breakup
model (WAVE, KH-RT, Reitz-Diwakar); collisions; a comparison with measured data (F-252).

## Open questions and choices for the maintainer

- **Q1. Status of F-245.** If every registered check is met, A1 and A2 are met in the library with no case-tree wiring. Marking
  F-245 done is the maintainer's decision; the roadmap says partial until then.
- **Q2. Case-tree wiring.** Options: (a) optional tables `particles.evaporation` and `particles.breakup`
  (`.optional().hash_omit_default()`, as `particles.film`), with the carrier temperature and vapour fraction as constants or from
  the energy and species fields, and the vapour source written to the report; (b) library only, as registered. Recommendation:
  (b) now; (a) with F-252, which needs the carrier vapour species and the two-way mass and energy exchange anyway, so the keys
  arrive with a carrier that receives what they produce.
- **Q3. Two-way exchange.** Evaporation books the vapour mass per cell only; the record of ADR 0057 is refused with either model.
  A complete exchange needs the vapour mass, its momentum and enthalpy, the heat drawn from the gas (`Q` with Bird's correction)
  and a decision on the breakup's normal velocity. Recommendation: a separate item before F-252.
- **Q4. Product-drop sizes.** Options: (a) every product drop at `r_32`, as registered; (b) the source's chi-squared draw with
  Sauter mean radius `r_32` (a parcel would then take one drawn radius, and the count follows the mass). Recommendation: (a) until
  a size-distribution comparison (F-252) needs (b).
- **Q5. A droplet at its boiling point.** Registered: an error. Options: (a) keep; (b) a boiling model (heat-limited
  evaporation at `T_b(p)`). Recommendation: (a) now; (b) with a flash-boiling need.
- **Q6. The evaporation model.** Registered: Bird's correction (model B of Both et al.). The source names the Abramzon-Sirignano
  model "the most relevant" for spray combustion; it differs from B only in the film thickness, and not at `Re = 0` (its eq. 28),
  so DK and A1 are unchanged by it. Recommendation: B now; Abramzon-Sirignano as an option when a convective case with data needs
  it.
- **Q7. `yd` at the breakup time.** The source evaluates `yd` "at `t_bu`" without naming the solution; registered: the damped
  update 25b. With `mu_l = 0` both agree. Recommendation: keep.
- **Q8. The inputs `f` and `d_min`.** Caller inputs without defaults in the library. Recommendation: defaults (for example `f =
  0.1`, `d_min = 1e-3` of the injected diameter) only when keys are added (Q2).
- **Q9. TAB under damping across steps.** The source re-estimates the breakup time from the damped state at every time step, so
  with viscosity the breakup time depends on the step (BU3s records it). Recommendation: keep the source's rule.

## Addendum 1: results (2026-11-08)

Measured against the limits registered above; no limit was changed and no amendment was needed. Release builds on Windows. The
registration is 18bb5e1a; the code and the tests are 7105ec0a, committed before their first run; every library check ran once on
7105ec0a (`cargo test --release -p rcfd-particles --test evaporation_f245 --test breakup_f245`, log `w49i-work/f245-run1.log`),
the suites of DF1 once after it. The runs are deterministic and none was repeated to change a result. Every registered check met
its limit, every negative control held, and every mutation made its check fail.

### A. Evaporation (`crates/rcfd-particles/tests/evaporation_f245.rs`, 7 tests, 0.01 s)

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| DK (the test's own derivation) | | `T_wb = 356.5724920247` K, `X = 0.648102`, `Y_i = 0.863960`, `B_M = 6.35080493`, `phi = 0.75`, `B_T = 3.46427741`, `K = 3.4196739796e-7` m^2/s (both forms of DK to the printed 11 digits) | as predicted (PR1) |
| EV1 (acceptance A1) slope over 66 samples | 2 percent | `3.4196739796e-7`, 1.1e-15 | met |
| EV1b residual | `1e-3 d0^2` | `9.1e-16 d0^2` | met |
| EV1c `T_p` at every sample | `1e-6` K | `5.7e-14` K | met |
| EV1d completion; age against `(d0^2 - d_min^2)/K` | `Evaporated`; 2 percent | `Evaporated` after 74 samples; `7.3106091835e-3` s, 1.2e-16 | met |
| EV1e (acceptance A1) vapour against the liquid lost at every step; `evaporated()` against the sources | 1e-10; 1e-12 | worst 2.9e-16; at completion the vapour equals the injected mass to 1.4e-16; `completed() = 1` | met |
| EV2 heat-up: slope over `[0.05, 0.25]` (14 samples); overshoot; the last sample; mass | 2 percent; `1e-3` K; `1e-4` K; 1e-10 | 7.1e-12; `T_p` never above `T_wb` (largest `T_p - T_wb = -5.7e-14` K); `-5.7e-14` K at `d^2/d0^2 = 6.6e-3`; 7.2e-16. First within 0.1 K of `T_wb` at `d^2/d0^2 = 0.704` (the continuous model: 0.709) | met |
| EV3 no driving force (`Y_s = 0.562707405451523`) | 1e-9; `1e-9 n m0` | `d` unchanged bit for bit, vapour 0, `T_p = 330` exactly | met |
| EV4 convection (`Re0 = 16.667`, `Nu0 = 4.13267`, `Sh0 = 5.086164`, `T_wb(Re0) = 350.87431484` K, `K_loc = 7.1130557434e-7`) | 2 percent | 5.9e-14 | met |
| EV5 Jacobian, 8 points | 1e-5 | worst 5.6e-9 (365 K, both Reynolds numbers) | met |

### B. Breakup (`crates/rcfd-particles/tests/breakup_f245.rs`, 8 tests, 0.13 s)

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| BU1 (acceptance A2), `We` = 6.1, 12, 60, 1000: one breakup each; age against `arccos(1 - 12/We)/omega0`; child radius against `r0/(4/3 + We/6)`; count; `n d^3`; velocity | one; 2 percent; 2 percent; 1e-12; 1e-14; zero | one each; 1.6e-15, 0, 4.8e-16, 7.8e-15; 1.6e-16, 2.3e-16, 5.8e-16, 1.5e-14; 5.3e-16, 8.6e-16, 1.4e-15, 4.4e-14; 0, 1.3e-16, 1.3e-16, 3.9e-16; zero | met |
| BU2 `We = 5.9`, 500 steps | no breakup, bits; largest `y` in `[0.9, 1]` | none, `d` and `n` bit for bit; 0.98333168 (`2W = 0.98333333`) | met |
| BU3 viscous (`1/t_d = 10714.29`, `omega = 10666.56`): age; child radius | 2 percent each | `1.472636231151e-4` s, 0 (equal bits); `r0/r_32 = 2.41893696`, 0 | met |
| BU3s (record) steps of `1e-5` s | none | one breakup at `2.199283655e-4` s (later than BU3, as predicted; the damped `y` crosses `W = 1` at `2.211e-4` s) | recorded |
| BU3o overdamped | bit for bit | no breakup, `y = yd = 0` after all 100 steps | met |
| BU4 `We = 1000`: eq. 12a; eq. 23; eq. 14; `abs(dv . x)`; eq. 16 | 2 %; 2 %; 1e-12; `1e-12 abs(dv)`; 2 % | 0.10 %; 0.80 %; 7.7e-15 (`abs(dv) = 9.7296796796` m/s); 0; 0.30 % | met |
| BU4 `We = 6.001`: eq. 13; 7/3 | 2 percent each | 0.82 %; 0.0071 % | met |

### C. The spray (in `evaporation_f245.rs`)

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| SP1 (acceptance A1, 160 parcels in 32 cells) vapour against the liquid lost, 300 steps | 1e-10 | worst 3.0e-12 | met |
| SP2 (acceptance A2) liquid plus vapour against the injected mass, 300 steps | 1e-12 | worst 8.8e-16 | met |
| SP3 coverage | as stated | all 80 parcels of `a` broke up once (80 events, no `b` parcel broke); 80 `Evaporated` (the `b` parcels), 80 `Escaped` (the `a` children), none active, deposited, stuck or in a film; vapour `1.156465e-5` kg | met |

### D. Refusals, inert paths and defaults

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| RF1, 22 refusals; 2 controls | `Err`; `Ok` | all `Err`, each with its message; `Ok`, `Ok` | met |
| RF2, 13 refusals; control | `Err`; `Ok` | all `Err`; `Ok` | met |
| IN1 inert breakup on the RX2 scenario | bit for bit | the 2862 words of the run without equal the leading words of the run with (3743 words); no breakup event | met |
| DF1 the suites listed, unedited | pass | `rcfd-particles`: all 11 test binaries pass (closed_forms 24 + 3 ignored, lagrangian_ext 13 + 1, wall_film 6, film_gas 18, wall_interaction 8, erosion_presets 2, dispersion_f255 6, reaction_f255 5 + 1); `rcfd-api`: particles_case 6 + 1, particles_ext_case 3, particle_run 9, particles_two_way 14 + 6, wall_film_f253 5 + 3, wall_interaction_f254 6, erosion_presets_f254 6, film_gas_f292 9, dispersion_f255 15 + 1, case_hashes_w48u (H1) 1, baseline_hashes 3 + 1; none edited | met |

### Negative controls

| Id | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| NE1 slope against the classical model's `K` (`T_wb = 370.112264` K, `K = 7.676567e-7`) | `>= 50` percent off | 55.5 percent | held |
| NE2 slope against `8 rho D B_M/rho_l = 1.088709e-6` | `>= 50` percent off | 68.6 percent | held |
| NB1 `We = 12` age against the half period | `>= 40` percent off | 50.0 percent | held |
| NB2 `We = 60` `r0/r_child = 11.3333` against 7/3 | `>= 50` percent off | 385.7 percent | held |
| NB3 BU3 age against the inviscid time | `>= 20` percent off | 41.7 percent | held |

### Mutations

Each was applied alone by an exact text replacement on a `git archive` copy of 7105ec0a (`w49i-work/f245-mut`, scratch script
`f245-scratch/mutate.py`, log `w49i-work/f245-mutations.log`), the named test was run once in release, and the file was restored
and compared byte for byte with the kept copy (equal in all eleven runs).

| Id | Must fail | Result |
| --- | --- | --- |
| ME1 `ln(1 + B_M)` -> `B_M` | EV1 | failed: slope 2.35 percent off; the droplet cooled 17.5 K to the mutated model's own wet bulb (EV1b `1.05e-2 d0^2`, EV1c 17.5 K, EV1d 7.0 percent also miss) |
| ME2 Bird's factor -> 1 | EV1 | failed: slope 124 percent off |
| ME3 Clausius-Clapeyron exponent reversed | EV1 | failed: `advance` refused the droplet as above its boiling point |
| ME4 vapour without `n` | EV1e | failed at the first evaporating step |
| ME5 the rest not booked at completion | EV1e | failed at the completion step |
| ME6 `Sh0` with `Pr` | EV4 | failed: 18.7 percent |
| MB1 count not rescaled | BU1 | failed: count and `n d^3` 92 percent off at `We = 6.1` |
| MB2 `(6K - 5)/12` | BU1 | failed: child radius 6.0 percent off at `We = 6.1` |
| MB3 `W` without `C_b` | BU1 | failed: no breakup at `We = 6.1` (the critical value doubles) |
| MB4 `omega^2` without the damping term | BU3 | failed: age 29.4 percent off |
| MB5 normal speed without `C_b` | BU4 | failed: `abs(dv)` 100 percent off |

`scripts/limit_audit.py` on the tree: no hit in the two new test files.

### Disclosures

- EV1e and EV2 at the first step: the parcel is released at the end of step 1 and has lost nothing; the relative form is undefined
  there, and the test requires the vapour and `evaporated()` to be exactly 0 at such a step (they were). Every later step uses the
  registered relative limits.
- PR8 stated NE1 and NE2 as `K_classical/K - 1` (+124.5 and +218 percent). The registered measure is relative to the reference
  of the negative control (`abs(slope - K_ref)/K_ref`), which gives 55.5 and 68.6 percent; both hold their `>= 50` percent limit.
- ME1 shows that the slope check alone is weakly sensitive to the mass-transfer law: with `B_M` in place of `ln(1 + B_M)` the
  droplet settles at its own wet bulb, 17.5 K colder, where its evaporation rate is within 2.35 percent of `K` (at the wet bulb the
  rate is set mostly by the heat reaching the droplet). The temperature check EV1c pins the law (17.5 K against `1e-6` K).
- RF1 "with a reaction set": the reaction refuses heat transfer, so this call is refused for the missing heat transfer; the
  reaction-specific refusal is reached by `set_reaction` after `set_evaporation` (also in RF1).
- `cargo fmt` and clippy were run before the first run: formatting, and a type alias for the refusal lists in the two test files
  (`clippy::type_complexity`); no logic changed.
- Not exercised by a test: condensation (`B_M < 0`), evaporation with the random walk, the film, erosion or rough walls; a wall
  hit in the sub-step of a completion or a breakup; the immediate breakup of a drop with `y >= 1` (`s = 0`).

### Status

Every registered check of A1 and A2 is met in the library; by the decision rule F-245 is done, with the maintainer's acceptance
still required (Q1). The roadmap entry is marked done on this branch with that note (the registration's Q1 said "partial until
then"; the rule of the brief governs). Q2 to Q9 stay open; Q2 (case-tree wiring) is the main one.

### Gate line (added at the merge, 2026-11-08)

The gate of `e3850cc5` ran on a `git archive` copy (`scripts/gate_test.py`, 3 jobs, wall 6191 s, lowest free memory 9 GB): fmt per
package 0, `clippy --workspace --all-targets -D warnings` 0, tests 3309 passed, 0 failed, 913 ignored (3309 test processes in
626 binaries). The merge with main was clean. After the merge: clippy of rcfd-particles and rcfd-api clean; `evaporation_f245`
and `breakup_f245` pass in release. Every registered check is met, so F-245 is done by the decision rule, pending the
maintainer's acceptance (Q1).
