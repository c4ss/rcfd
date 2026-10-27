# ADR 0063: Wave 31B, diagnosis of the SST wall-resolved skin-friction deficit and of the k-epsilon wall-function mesh dependence

Status: registered before any experiment of this wave (this file is committed first; results go into addenda, limits are never widened).
Branch main, written at HEAD d0c7ed1 (plus uncommitted work of other agents, none of it in `crates/rcfd-turb/src`).

## Deficits under diagnosis (both diagnosed-but-unproven in ADR 0060 and 0061)

(A) `KOmegaSst` wall-resolved (default `SstWallMode::Automatic`), TMR 2DZP flat plate, three meshes (y+ 1, 0.5, 0.25 at x = 2):
Cf below Karman-Schoenherr at equal Re_theta by 8.65 / 6.44 / 5.18 / 4.36 % (Re_theta 5000 / 7500 / 10000 / 12500, finest), -10.6 ... -6.2 % (medium), -13.2 ... -8.9 % (coarse);
order 0.55; unchanged (< 0.1 %) by linear-upwind k and omega or by doubling the streamwise cells; log-law fit over y+ 50 to 300 gives slope 2.7766 (1/kappa_eff, kappa_eff 0.360) against 2.4866 of the TMR data.
Channel Re_tau 178 (SST): U_b+ 16.07, 15.70, 15.54, 15.42, 15.33, 15.27 for ny 24, 40, 56, 80, 112, 160; the converged value is about 3 % below DNS (15.679).

(B) `KEpsilon` Standard with the standard wall function, channel Re_tau 178 (uniform mesh, body force): U_b+ error +6.1 / +16.4 / -3.6 % for ny 8 / 12 / 16 (cell-centre y+ 22 / 15 / 11), a bump near y+ 15 and a steady fall to -29 % when the first cell centre enters the buffer layer.

## Sources to be read (not from memory) and recorded in the addendum

* NASA TMR SST page (https://tmbwg.github.io/turbmodels/sst.html): equations, F1, F2, CD, limiter, constants, boundary conditions (the old host turbmodels.larc.nasa.gov redirects to nasa.gov).
* NASA TMR 2DZP flat plate page (https://tmbwg.github.io/turbmodels/flatplate_sst.html): CFL3D/FUN3D SST Cf(x = 0.97).
* A published wall-function formulation for part (B) (Launder and Spalding 1974 as reproduced in public documentation; Grotjans and Menter 1998 for the scalable form), read by fetch in the experiments phase.
Menter 1994 and Menter, Kuntz, Langtry 2003 themselves are paywalled; if the original text is not obtainable, only the TMR transcription is claimed as read.

## Hypotheses and discriminating experiments

All SST experiments use the TMR 2DZP set-up of `validation_flat_plate_wall_law.rs` (same runner, same Re_theta definition of ADR 0061 addendum 4), medium mesh (140 x 110, y+ 0.5) for bisection,
coarse (70 x 55) for cheap checks, and the finest (280 x 220) at most once per candidate fix. Probe switches live in a self-contained test-side copy of `sst.rs`
(`crates/rcfd-turb/tests/diag_w31b_sst_probe.rs`), first checked to reproduce `KOmegaSst` bit for bit (or to 1e-10 in Cf) when all switches are off; no solver source is touched by an experiment.

| id | hypothesis | experiment (single factor against the baseline) |
|----|------------|--------------------------------------------------|
| HA1 | wall omega value: 10 * 6 nu/(beta1 y^2) is imposed on the wall FACE with y the centre-to-face distance, so the transported first-cell omega differs from the analytic sublayer omega 6 nu/(beta1 y^2) by a mesh-dependent factor | print omega_1/(6 nu/(beta1 y1^2)) and omega_2/analytic for the three meshes; factor on the face value 1, 10, 100 (diagnostic only, not a tuning) |
| HA2 | omega at the first cell versus at the wall: Menter-style fixing of the first-cell value to the analytic form (and the blend with the log value) gives a different, y+-independent answer | probe switch: first-cell omega fixed to 6 nu/(beta1 y1^2) (keeping the face value) |
| HA3 | the Spalding-based `Automatic` overrides (wall strain, turbulent share of production, Spalding shear) rather than the plain TMR low-Re treatment cause the deficit | library `LowRe` mode (k = 0 on the wall, laminar shear, no overrides) on coarse and medium; probe switch: Automatic with the plain Green-Gauss strain in the wall cell |
| HA4 | cross-diffusion and F1 blending near the wall | print F1 (wall-normal profile at Re_theta 10000): expected 1 up to the outer layer; probe switch: CD term off |
| HA5 | production limiter 10 beta* k omega | print max P/(10 beta* k omega) over the field (inactive on a plate if < 1); probe switch: limiter off |
| HA6 | wall-distance accuracy on the stretched mesh | compare `wall_distance()` with the cell-centre y of the plate (flat wall, exact reference) |
| HA7 | k wall condition: zero gradient (Automatic) versus k = 0 | probe switch with k = 0 on the wall face while the rest stays Automatic |
| HA8 | gradient operator of the strain on the stretched near-wall mesh (Green-Gauss with linear weights) | probe switch: least-squares gradient for the strain |
| HA9 | comparison definition (Re_theta from the cell-centre trapezoid, edge velocity at y = 0.25) | Cf versus x at fixed x = 0.97 against the TMR CFL3D SST value 0.00268299 (no Re_theta needed); Re_theta recomputed with 2x finer integration bound (y up to 0.10 and 0.20) |
| HA10 | the deficit is a property of the published model itself (an independent code of the same model reproduces it) | Cf(x = 0.97) of the three meshes and its Richardson extrapolation against 0.00268299 (CFL3D 273 x 193; TMR GCI of that sequence 0.28 %) |

The channel SST drift (U_b+ falling with ny) gets HA1, HA2, HA3 on ny 40 and 80 (cheap, 10 s runs) and the wall-layer sequence ny 24..160 for the best candidate.

Part (B), k-epsilon wall function in the channel (uniform mesh, ny 4 ... 32, same set-up as `validation_channel_re180.rs`):

| id | hypothesis | experiment |
|----|------------|------------|
| HB1 | the standard function is applied with a y* switch at y*_lam (11.06 for E 9.8) and `Smooth` blending; the bump at y+ 15 is the cell crossing the switch (y* = Cmu^1/4 sqrt(k) y/nu is below y+ in the buffer layer) | print per ny: y+ from the wall shear, y* of the wall cell, branch weight, nu_t,w/nu, wall-cell G and eps against the log values; compare the three `WallTreatment` values (Switch, Hysteresis, Smooth) |
| HB2 | below y* ~ 11 the model falls back to the viscous branch (G = 0, eps = 2 nu k/y^2, nu_t,w = 0) of a high-Re k-epsilon without damping functions, which is outside its design | same sequence with `WallFunction::Scalable` (y* clipped at y*_lam, formulation of Grotjans and Menter): if U_b+ becomes mesh independent as y+ falls, the dependence is the validity range of the Standard function |
| HB3 | the wall-cell production/epsilon forms differ from the published standard wall function | term-by-term comparison with the published formulation (wall shear tau_w = rho Cmu^1/4 sqrt(k) U kappa / ln(E y*), G = tau_w Cmu^1/4 sqrt(k)/(kappa y), eps = Cmu^3/4 k^3/2/(kappa y)) |
| HB4 | k zero-gradient at the wall and the nearest wall face only | covered by HB1 print and by the code read; no separate run |

## Registered acceptance limits (written before any run; never widened)

Influence: a switch is called influential if it moves Cf(Re_theta = 10000) of the medium mesh by more than 1.0 % relative to the unchanged probe baseline (the streamwise/convection variants moved it by < 0.11 %, the noise floor).
Isolation of a cause for (A): a single-factor switch is called the isolated cause only if, with the switch, (i) the coarse/medium/(fine if run) spread of Cf(Re_theta 10000) is at most 2 % of its finest value, and (ii) on the finest mesh Cf is within 5 % of Karman-Schoenherr at Re_theta 5000, 7500, 10000, 12500 and the mesh-convergence check of ADR 0061 D3 (|Cf_f - Cf_m|/Cf_f <= 1.5 %) holds. Only an isolated cause that is a coding or boundary-condition error against the published formulation may be fixed; a fix is added as a NEW opt-in key (library field on `KOmegaSst`, default unchanged, schema untouched unless stated in an addendum), with every existing result bit-identical and `baseline_hashes` unchanged (a fix that would change a baseline stops this wave).
Model-property conclusion: if the baseline finest-mesh Cf(x = 0.97) (or its Richardson extrapolation) lies within 2 % of the TMR CFL3D 0.00268299, the deficit against Karman-Schoenherr is recorded as a property of the published model as reproduced by an independent code (the TMR page gives that value, not the empirical law, as the SST reference) and no defect is claimed for it; the remaining mesh dependence is then reported with its measured size.
Channel SST: an isolated cause must reduce the U_b+ change between ny 80 and ny 160 to below 0.5 % (baseline: 0.9 %, 15.418 to 15.274).
Part (B): the cause is called isolated if (a) the position of the U_b+ bump coincides (within one mesh of the ny sequence) with the wall-cell y* crossing y*_lam and (b) the Scalable function gives a U_b+ spread of at most 5 % over ny with y+ <= 22 where Standard spreads by more than 15 %. No solver change is planned for (B): Scalable exists as an option; a defect in the Standard form would be claimed only from a term-by-term mismatch with the published formulation (HB3) that a switch reproduces.
No coefficient is tuned to DNS or TMR data; factor scans (HA1) are diagnostics and are not adopted unless a published formulation prescribes the value.

## Process

ADR first, committed; probe tests are ignored diagnostics (no limit); old known-miss tests stay ignored with their limits; baseline hashes and all existing tests untouched; new tests for any corrected behaviour are new.

## Addendum 1 (before any solver change): deviations so far and the limits registered for the k-epsilon opt-in treatment

Probe infrastructure (no solver source changed yet): `crates/rcfd-turb/tests/diag_w31b_sst_probe.rs` (test-side copy of `sst.rs` with switches; `probe_matches_library`: worst relative Cf difference 0.0 on the coarse mesh, 1249 iterations each)
and `crates/rcfd-turb/tests/diag_w31b_channel_wf.rs` (k-epsilon channel sequences). Both are ignored diagnostics without limits.

DEVIATION 1 (disclosed): while running HA9 the profile-based Re_theta of ADR 0061 addendum 4 turned out to be negative at x = 0.06 and 0.10 (coarse mesh) and 25 to 35 % below the momentum integral
int Cf/2 dx in the range x = 0.5 to 1; the symmetry top at y = 1 makes u/U_e at y = 0.25 differ from the outer-layer values by about 0.2 percent and the integrand of the free-stream part accumulates (0.15 x 0.002 = 3e-4 against theta ~ 1e-3).
The registered HA9 variants (integration bounds 0.10 and 0.20) do not remove it; two further Re_theta definitions were therefore added to the probe (not registered beforehand): (a) theta from the profile with the edge at the first cell above the
nu_t maximum where nu_t/nu < 1 (edge velocity there); (b) theta = int_0^x Cf/2 dx' (zero-pressure-gradient momentum integral). Both are reported next to the registered definition. The registered D1 limits and the ignored `d1_..._known_miss` test are untouched;
the new readings are additional evidence and never replace the registered measurement.

Limits registered for the k-epsilon treatment (part B) before any code for it is written. Candidate (named after the experiment that suggested it, HB3): `WallTreatment::Equilibrium`, a NEW enum variant; default `Smooth` and all existing arms unchanged.
In every wall cell, whatever y*: `eps = Cmu^(3/4) k^(3/2)/(kappa y)` and `G = tau_w^2 / (kappa Cmu^(1/4) sqrt(k) y)` with `tau_w = (nu + nu_t,w) u_t / y` (`nu_t,w` from `nut_wall_from_k`, i.e. the laminar shear below y*_lam),
the equilibrium forms of the standard wall function as given in a secondary description (recorded in [provenance](../provenance.md); U* = ln(E y*)/kappa for y* > 11.225, U* = y* below; zero normal gradient of k; fetched, see addendum 2). These forms are the classical Launder and Spalding (1974) equilibrium wall functions (paper not read); as used here they are UNVERIFIED against a published source, replace before relying on it. 11.225 itself is the root of y* = ln(E y*)/kappa for kappa 0.4187, E 9.793.
Acceptance (channel Re_tau 178, uniform mesh, ny = 6, 8, 10, 12, 14, 16, 20, 24, 32, i.e. cell-centre y+ 30 down to 5.6, converged to du < 1e-9 and change < 1e-8):
(E1) |U_b+/15.6787 - 1| <= 10 % on every mesh; (E2) (max - min)/mean of U_b+ over these meshes <= 5 %; (E3) default-treatment results bit-identical (existing tests unchanged, `baseline_hashes` unchanged).
If E1 or E2 fail the candidate is not claimed, the code is not kept as a feature and the part-B conclusion stays at the diagnosis. The comparison treatments `Smooth`, `Switch`, `Hysteresis` and `WallFunction::Scalable` stay in the measured table.
No library schema key is added in this wave (the new variant is selectable through the library field `KEpsilon::wall_treatment`; wiring it into the settings tree would touch `rcfd-api` files owned by other agents of the wave).

## Addendum 2 (results; written after the experiments, limits above untouched)

Outcome in one paragraph: neither deficit is isolated as a coding or boundary-condition defect against a published formulation, so NO solver change was made in this wave (no new key, no baseline touched, `baseline_hashes` unchanged).
(A) The registered Re_theta is biased low and exaggerates the deficit; what remains is a slowly (first order) decaying sensitivity to the published wall-face omega factor 10, not a defect.
(B) The mesh dependence is the validity limit of the log-law function with a viscous fall-back branch and a positive feedback of y* on k; the published equilibrium forms were tried as an opt-in treatment, removed the bump but not the fall, missed the registered limits E1 and E2 and were removed again.

### Sources actually read (WebFetch, 2026-10-18)

* https://tmbwg.github.io/turbmodels/sst.html (NASA TMR, SST-2003 as used by the TMR): the model equations, F1, F2, the cross-diffusion floor (1e-10), the production limiter 10 beta* rho k omega, nu_t = a1 k / max(a1 omega, S F2), the constants and the wall conditions k_w = 0, omega_w = 10 * 6 nu / (beta1 d1^2) with d1 the first-cell distance. (turbmodels.larc.nasa.gov redirects away and was not used.)
* https://tmbwg.github.io/turbmodels/flatplate_sst.html (NASA TMR 2DZP, SST): the 273 x 193 CFL3D reference, Cf(x = 0.97) = 0.00268299 with GCI 0.28 % (p = 1.21), plate x in [0, 2] behind a symmetry section, U = 1, nu = 2e-7.
* A secondary description of the standard wall functions, not a published source (recorded in [provenance](../provenance.md); UNVERIFIED against a published source, replace before relying on it): y* = Cmu^1/4 k^1/2 y/nu, log law for y* > 11.225 and U* = y* below, k and epsilon not solved in wall-adjacent cells but set from the equilibrium wall-function forms, zero normal gradient of k. Two neighbouring pages of the same source were also fetched; the scalable-function text there was seen only through the fetch summary.
NOT read (secondary descriptions only): Menter (1994), Menter, Kuntz, Langtry (2003), Launder and Spalding (1974), Grotjans and Menter (1998). The original wall-function texts are therefore not claimed.

### Part A: results

Meshes: coarse 70 x 55 (y+ 1), medium 140 x 110 (y+ 0.5), finest 280 x 220 (y+ 0.25); all runs converged (du < 1e-9). Cf deviation from Karman-Schoenherr at Re_theta 5000/7500/10000/12500 with the registered Re_theta, baseline: coarse -13.2/-11.0/-9.7/-8.9 %, medium -10.6/-8.3/-7.0/-6.2 %, finest -8.65/-6.44/-5.18/-4.36 % (reproduces ADR 0061). `probe_matches_library`: with all switches off the relative Cf difference to the library is 0.0 (coarse).

| id | experiment | result | verdict |
|----|------------|--------|---------|
| HA1 | omega_1/analytic on the finest mesh: cell 0 0.857, cell 1 1.43, then 1.39 ... 1.25. Factor on the wall-face omega 1 / 10 / 100, medium Cf(10000): 2.5717e-3 / 2.4485e-3 / 2.3722e-3 (+5.0 % / 0 / -3.1 %) | strongly influential. Factor 10 is the published TMR value, so no defect. With factor 1 the finest deficit is -6.28/-4.08/-2.86/-2.07 % (registered Re_theta) and +0.49 ... +0.70 % (momentum-integral Re_theta). Cf(10000) spread coarse/medium/finest with factor 1 is 3.1 % of the finest (limit 2 %) and the finest misses 5 % at Re_theta 5000 (-6.3 %): NOT isolated under the registered rule. Not adopted (no tuning) | influential, published value, no fix |
| HA2 | first-cell omega fixed to 6 nu/(beta1 y1^2) (`cell1_analytic`), medium | Cf(10000) -0.45 %, deficits slightly larger (-11.0 ... -6.6 %) | negative |
| HA3 | `LowRe` library mode and the plain wall strain | Cf(10000) +0.08 % (coarse), +0.01 % (medium); plain strain +0.08 % (coarse); inside the 0.11 % floor | negative |
| HA4 | F1 profile at Re_theta 10000 (finest): 1.000 for y+ 10 ... 1000, 0.04 at y+ 3000. Cross-diffusion off: Cf(10000) +3.7 % (coarse), +3.9 % (medium) | F1 is 1 through the wall layer (k-omega branch). Cross-diffusion is influential (> 1 %) but is a term of the published model (sst.html); not removable, finest not run | influential, published term, no fix |
| HA5 | max P/(10 beta* k omega) = 0.12 ... 0.14; limiter off | Cf identical to 7 digits (coarse) | negative |
| HA6 | max relative |wall_distance - y| over the column | 0.0 (exact on the flat plate) | negative |
| HA7 | k = 0 on the wall face with Automatic | Cf(10000) -0.007 % (coarse) | negative |
| HA8 | least-squares gradient for the strain | Cf identical to the printed digits (coarse) | negative |
| HA9 | Re_theta definition | see Deviation 1: the profile Re_theta (edge at y = 0.25 under the symmetry top) is negative for x < 0.1 and far below int Cf/2 dx for larger x (finest, x = 0.98: 5782 against 7891). Finest deviations from K-S with the momentum integral: -2.40/-2.15/-1.95/-1.79 %; with the nu_t-edge profile: -3.40/-2.99/-2.69/-2.46 %; medium -4.71 ... -3.73 % and -5.55 ... -4.36 %. Integration bound y <= 0.10 / 0.15 / 0.20 moves the registered Re_theta at x = 0.97 and the deviation there to -6.98 / -7.85 / -8.23 % (finest) | the registered measurement exaggerates the deficit by about 3 points; registered D1 limits and the ignored known-miss test unchanged |
| HA10 | Cf(x = 0.97) against CFL3D 0.00268299. Baseline coarse/medium/finest 2.5770/2.6514/2.6984e-3 = -3.95/-1.18/+0.58 %, observed order 0.66, Richardson value 2.779e-3 (+3.6 %). Factor 1: 2.8329/2.7749/2.7589e-3 (+5.6/+3.4/+2.8 %), order 1.9, extrapolated 2.753e-3 (+2.6 %) | the literal rule (finest baseline within 2 %) would call the deficit a property of the published model, but the Richardson value is not within 2 %: ambiguous, no model-property conclusion claimed. A fixed-x comparison also depends on the virtual origin (the TMR case has a symmetry section upstream of the plate, rcfd starts the plate at the Dirichlet inlet); whether this explains the +3 % was not tested | unresolved |

Log-law fit slope over y+ 50 ... 300: 2.78 (kappa_eff 0.36) for the baseline on all meshes, 2.76 with factor 1, 2.78 with cross-diffusion off: not explained. Term-by-term comparison with TMR sst.html (equations, F1 and F2 arguments, CD floor, limiter, nu_t with S and F2, constants, k_w = 0, omega_w factor 10 with the cell-centre distance): agrees with `sst.rs`. rcfd-only additions, none influential at the 0.2 % level where tested: Spalding-based `Automatic` overrides, k zero gradient and omega fixing for y+ > 5, nu_t cap, first-order k and omega convection (linear upwind moves Cf < 0.1 %, ADR 0061).

Channel Re_tau 178, SST, U_b+ (DNS 15.6787):

| ny | 24 | 40 | 80 | 160 |
|----|----|----|----|-----|
| baseline | 16.066 | 15.696 | 15.418 | 15.274 |
| LowRe | 16.061 | not run | 15.418 | 15.274 |
| wall-face omega factor 1 | 15.127 | 15.160 | 15.154 | 15.144 |
| factor 100 | 16.706 | 16.081 | not run | 15.360 |
| first-cell omega analytic | 16.153 | 15.748 | 15.444 | 15.286 |
| cross-diffusion off | 16.066 | 15.696 | 15.418 | not run |

Baseline change ny 80 to 160: 0.94 % (limit for an isolated cause 0.5 %). Factor 1 passes that limit (0.07 %; spread over ny 24 ... 160 is 0.2 %) at 3.4 % below DNS, but it is a scan of a published constant: reported, not adopted. Conclusion for A: the y+ dependence is the first-order decay of the solution's sensitivity to the wall-face omega value at the published factor 10; no defect is isolated.

### Part B: results (channel Re_tau 178, uniform mesh, converged to change < 2e-10)

| treatment | ny 8 | 10 | 12 | 14 | 16 | 20 | 24 | 32 |
|-----------|------|----|----|----|----|----|----|----|
| cell-centre y+ | 22.3 | 17.8 | 14.8 | 12.7 | 11.1 | 8.9 | 7.4 | 5.6 |
| Standard, Smooth (default), U_b+ error % | +6.1 | +8.5 | +16.4 | +4.8 | -3.6 | -14.7 | -21.6 | -29.2 |
| wall-cell y* (Smooth) | 21.3 | 16.4 | 7.45 | 6.18 | 5.39 | 4.31 | 3.57 | 2.60 |
| wall-cell k (Smooth), u_tau^2 | 3.05 | 2.83 | 0.84 | 0.79 | 0.78 | 0.78 | 0.77 | 0.73 |
| Standard, Switch / Hysteresis | +5.1 | +24.3 | +8.4 | -2.3 | -9.9 | -19.6 | -25.3 | -31.4 |
| Scalable | +5.1 | +4.9 | +4.9 | +4.9 | +5.2 | +6.1 | +6.8 | +7.9 |
| Equilibrium G and eps (removed candidate) | +4.1 | +4.2 | +4.3 | +4.4 | +1.8 | -8.7 | -15.2 | -22.4 |

* HB1 (isolated): the bump is the wall cell entering the viscous branch. At ny 10 (Smooth) y* is still 16.4; at ny 12 the wall-cell k has collapsed from about 3.05 to 0.84 u_tau^2 (G/G_log 0.03, eps/eps_log 0.14) and y* = Cmu^1/4 sqrt(k) y/nu = 7.45 is far below y+ = 14.8: with G = 0 the wall-cell k decays, y* falls and the cell stays viscous (positive feedback). Switch and Hysteresis collapse already at ny 10 (y* 10.75, first crossing of 11.06). Criterion (a): the bump (ny 10 for Switch/Hysteresis, ny 12 for Smooth) coincides within one mesh with the first mesh whose wall-cell y* is below y*_lam: met.
* HB2 (isolated): `WallFunction::Scalable` (y* clipped at y*_lam) gives +4.9 ... +7.9 % over ny 4 ... 32 (ny 4 to 6: +6.5 ... +5.5 %), a spread of 2.8 % of the mean, against 45.6 points for Standard: criterion (b) met. There is no k collapse. For y* below y*_lam the wall-cell effective viscosity is below nu (nu_t,w/nu -0.06 ... -0.54), a consequence of the clipped y* in the shear formula.
* HB3: the fetched secondary description of the standard wall function sets equilibrium G_k and epsilon in every wall cell and switches only the velocity law at y* = 11.225. rcfd drops to G = 0 and eps = 2 nu k/y^2 below y*_lam (`wall_cell_sources_ext`): a discrepancy with the fetched form for y* < 11. The published forms were tried as `WallTreatment::Equilibrium` (limits E1, E2, E3 registered in Addendum 1 before any code): U_b+ error +4.1 ... +4.4 % for y+ >= 12.7 (bump removed) but +1.8, -8.7, -15.2, -22.4 % for y+ 11.1 ... 5.6. E1 (<= 10 % everywhere) fails at ny 20, 24, 32; E2 (spread <= 5 %) fails (about 27 %). By the registered rule the candidate is not claimed; the variant and its code were removed and `kepsilon.rs` equals HEAD. The remaining fall below y* about 11 is therefore not explained by the production/dissipation forms; it appears with the equilibrium forms and laminar wall shear, so it belongs to the high-Re k-epsilon core of the channel once the wall cell lies in the buffer layer (no low-Re damping); this attribution was not tested separately.
* HB4: k zero gradient and the nearest wall face only: read in the code, consistent with the fetched description.

Conclusion for B: the mesh dependence of the Standard wall function in the channel is the validity limit of a high-Re k-epsilon model with a log-law wall function (first-cell centre y+ well above 30 for a clean log-law result; here +6 % at y+ 22), amplified by a bifurcation of the wall-cell k when the viscous branch is selected. `WallFunction::Scalable` in the existing library is mesh independent to 2.8 %; no solver code was changed. Old known-miss tests and their limits stay as they are.

### Deviations from the registered plan

1. Re_theta definitions beyond HA9 (Deviation 1 of Addendum 1). 2. The finest mesh was run twice (baseline as the reference for the new Re_theta definitions, and factor 1). 3. Not every treatment was run on every mesh (cells marked "not run"); HA2, HA3, HA5, HA7, HA8 were run on the coarse mesh (HA2, HA3 also medium), the effects being below the noise floor. 4. The scalable-function text of the secondary description was seen only through the fetch summary and Grotjans-Menter was not read.

### Not delivered

No fix (no isolated cause). Unexplained: the log-law slope 2.78, the +3 % fixed-x difference against CFL3D after extrapolation (possible leading-edge set-up effect), the fall of the k-epsilon channel result for y* below about 11 with equilibrium forms. No test for corrected behaviour (none exists). Added: ignored diagnostics only (`diag_w31b_sst_probe.rs`, `diag_w31b_channel_wf.rs`).
