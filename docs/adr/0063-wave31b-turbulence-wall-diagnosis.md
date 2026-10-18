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
