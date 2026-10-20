# ADR 0082: Wave 36C, where the spurious swirl of the rotating-frame sector comes from, and an opt-in absolute-velocity rotor passage

Status: registered before any code of this wave was written and before any solver run of this wave. Limits and decision rules are fixed; a miss of a
check becomes an `#[ignore]`d `*_known_miss` test (or a reported miss where the code it needs is removed) with the limit intact, a defective set-up gets
a NEW corrected test and the old one stays ignored. Baseline hashes are never re-recorded. Clean-room: first principles only; nothing from vendor
documentation or other CFD codes. Parents: ADR 0078 (X2: the empty passage leaves with 2.254 N m; inertial control 1.6e-4 N m; minmod 0.0026 N m net
but 20 Pa swirl energy; X2f: the same error with the frame source removed; X2c did not converge), ADR 0008 (`mrf`, absolute-velocity formulation),
ADR 0065 S4 (`mrf` refused on the sector), ADR 0077 (steady guard, partner seam gradient), `docs/milestone-fan.md`. No fan performance claim is made.

## What was known when this was written

* Every number of ADR 0078 and its amendment 4. Code read (read-only) on HEAD c89495f: `rcfd_fv::convection` (upwind matrix; TVD by deferred
  correction; periodic faces upwind between the owner and the lagged rotated partner value, minmod adds the full central value there),
  `rcfd_fv::diffusion::laplacian` (two-point flux `Gamma_f |S|/|d| (u_N - u_O)`, the secondary gradient only when a gradient is passed), the
  SIMPLE/SIMPLEC kernel (`simple.rs`: no transposed stress term; Green-Gauss pressure gradient with linearly interpolated face values and the trapezoid
  wall values of the rotating frame), `RotatingFrame::add_force` (Coriolis lagged, centrifugal analytic at cell and face centres), `MrfZone`
  (frame flux `(Omega x r).S` only on internal faces with both cells in the zone; the zone's boundary faces must be surfaces of revolution,
  `worst_non_axisymmetry`, limit 1e-6), `sim.rs` (the turbulence model is updated with the absolute flux `phi` also inside an MRF zone).
* No run of any kind has been made in this wave.
* Arithmetic only (no run): a first-order upwind face value equals the central value plus `(|phi_f|/2)(u_O - u_N)` per Cartesian component. For the
  rigid part of the relative velocity, `w_rig = -Omega x r` (Omega along +z), `u_O - u_N = Omega z x d` with `d = r_N - r_O`, and this dissipative flux
  applied at the two cell centres has the angular-momentum defect `z . (d x (|phi|/2) Omega z x d) = (Omega/2) |phi_f| |d_perp|^2 >= 0` per face
  (`d_perp` the part of `d` normal to the axis). It is sign-definite: upwind acts as a numerical viscosity `|w_n| |d| / 2` in Laplacian form on a
  rigid rotation, whose antisymmetric gradient has zero physical (symmetric) stress but a non-zero Laplacian-form stress, i.e. a torque. A rough
  estimate (tangential cell width about `r Delta theta` with `Delta theta` = 40 deg / 10 cells, `|w_theta| = Omega r`) gives
  `rho Omega^2 (Delta theta / 2) integral r^2 dV` = about 2.2 N m for the full annulus, the size of X2a's 2.254 N m. The same argument applied to the
  physical Laplacian-form viscous term `div(nu_eff grad w)` gives about `2 rho Omega integral nu_eff dV`, about 0.025 N m with the inlet eddy viscosity
  (`k/omega` = 1.1e-3 m^2/s) over the 0.059 m^3 annulus; more where the eddy viscosity is larger.

## Hypotheses and their falsifiers (registered; the diagnostics run before any fix code is run)

Notation: `G_C` is the generation of absolute angular momentum about +z in cell C, defined from the discrete momentum equation of the cell with every
face force applied at its face centre: `G_C = sum_f z.(r_f x [phi_f w_f + p_f S_f - tau_f]) + Omega sum_f phi_f r_perp,f^2`, outward fluxes, where
`phi_f w_f` is the convective momentum flux with the scheme's face value, `p_f S_f` the pressure flux of the cell's Green-Gauss gradient, `tau_f` the
viscous flux of the Laplacian operator (the solver's own `Gamma_f = nu_eff` faces) and the last term the angular momentum of the frame velocity carried
by the flux. With `R_C` the cell's momentum residual (the same fluxes minus `V_C a_C`, `a_C` the cell value of the frame acceleration),
`G_C = sum_T G_T,C + z.(r_C x R_C)` exactly, with the terms

* `G_conv,C = sum_f z.((r_f - r_C) x phi_f w_f)`, `G_p,C = sum_f z.((r_f - r_C) x p_f S_f)`, `G_visc,C = -sum_f z.((r_f - r_C) x tau_f)`,
* `G_frame,C = Omega sum_f phi_f r_perp,f^2 + z.(r_C x V_C a_C)` (Coriolis torque against the flux of the frame angular momentum), split into a
  Rhie-Chow part `Omega sum_f (phi_f - wbar_f . S_f) r_perp,f^2` (`wbar_f` the linearly interpolated cell velocity) and the remainder,
* `G_res,C = z.(r_C x R_C)` (convergence / reconstruction residual).

Summed over the cells the interior faces cancel. `sum_C G_C = E_s + S_seam`, with `E_s` the solver-consistent excess (absolute angular-momentum outflow
through inlet and outlet with the solver's face values, plus the torques of the solver's own pressure and viscous wall fluxes, i.e. what a conservative
scheme would make zero) and `S_seam` the sum over the periodic face pairs (each pair: the two halves' `z.(r_f x F_f)`; zero for an exactly rotated
pair). The ADR 0078 excess (report-based wall torque, molecular viscosity) is printed next to `E_s`. All values are multiplied by 9 (full annulus) and by
`rho` = 1.2.

* **H1 (upwind convection of the rigid part).** The generation is the first-order upwind dissipation acting on the rigid part of the relative velocity.
  Closed form computed from the converged flux: `C_up = 9 rho (Omega/2) sum_f |phi_f| |d_perp,f|^2` over internal and periodic faces (periodic faces:
  `d` from the owner centre to the partner centre mapped into the owner's frame). Falsified if any of: (i) `G_conv` < 0.7 `E_s` in X2a; (ii) `C_up`
  outside [0.7, 1.3] `G_conv` in X2a; (iii) the wall-adjacent plus seam-adjacent cells carry more than 50 % of `G_conv` (the mechanism is volumetric).
* **H2 (Laplacian-form viscous stress on the rigid part).** `G_visc` is a real contribution of size `C_visc = 9 rho Omega sum_f Gamma_f |S_f| |d_perp|^2 / |d|`
  (internal faces; the wall faces reported separately), predicted 1e-2 to 1e-1 N m. Called significant if `|G_visc|` >= 0.1 `E_s`; falsified as a
  contribution if `|G_visc|` < 0.01 `E_s`.
* **H3 (frame source inconsistency, including Rhie-Chow).** Predicted `|G_frame|` < 0.1 `E_s` (X2f already cleared the frame terms by a different
  route). If the Rhie-Chow part exceeds 0.1 `E_s`, it is split further (body-force face term against pressure part) in an amendment before any fix run.
* **H4 (pressure lever arms on non-orthogonal faces).** Predicted `|G_p|` < 0.1 `E_s`.
* **H5 (seam pairs).** Predicted `|S_seam|` < 0.01 `E_s`.
* **Closure.** `|sum_C G_res,C|` <= 0.05 `E_s`, otherwise the attribution is reported as NOT CLOSED and no term is called responsible.
* **Reconstruction check.** For every cell and component, the face-flux sums of the rebuilt convection and Laplacian fluxes equal the row results
  `(A u - b)` of `rcfd_fv::convection` / `laplacian` called with the solver's own `phi_conv`, boundary coefficients, `Gamma` and scheme, to
  1e-10 of the largest row magnitude. If not, the budget is not used.

Location report (no limit): `sum` and `sum |.|` of every term by region: upstream of the hub leading-edge plane, chord band, downstream; wall-adjacent cells
(a face on `hub` or `shroud`), seam-adjacent cells (a face on a periodic patch), inlet/outlet-adjacent cells; 8 radial bins; and the generation per metre
of axial length of the three axial regions. The budget runs on X2a (upwind) and on X2d (minmod; its net is near zero, so the `+/-` map is what locates
the 20 Pa). Minmod face values are rebuilt with the kernel's formula (least-squares gradient, the limiter on internal faces, the central value on
periodic faces).

## Runs of the diagnosis

All on this wave's code (release build), single thread per run, at most two runs at a time, one run per set-up (nothing re-run to pick a result), the
ADR 0078 stop rules (8000 iterations, continuity non-finite or above 1e6, 60 min wall clock; plus the ADR 0077 steady guard record: a recorded trip
ends the run as diverged).

* **X2a-36.** ADR 0078 X2a unchanged, first. Reproduction rule: outlet L 2.25440 N m and Delta p_ts -59.984 Pa to every printed digit (ADR 0078
  amendment 4 on d88f816). If it does not reproduce, the other runs stop until the difference is explained in an amendment. The budget is computed at
  its end.
* **X2d-36.** ADR 0078 X2d unchanged (expected 0.0026 N m, 19.97 Pa, to the printed digits) with the budget.

## The fix (specified now; written only if H1 is not falsified)

If H1 stands, the remedy is to stop convecting the rigid part, i.e. the absolute-velocity formulation already in the code (`mrf`, ADR 0008), whose
convected quantity in an empty passage is `c = 0` exactly. Today it is refused on the sector because its frame flux lives only on internal faces, so
the boundary faces of the zone (blade, seams) must be surfaces of revolution. Fix **F-RP** (opt-in): key `sources.<name>.rotor_passage` (bool, default
`false`, `type = "mrf"` only; refused with a located diagnostic for any other type):

1. the frame flux `(Omega x r_f).S_f` is also applied on every boundary face whose owner is in the zone (walls, periodic seams, inlet, outlet), so the
   convecting relative flux is exactly zero on a wall rotating with the zone (whose absolute velocity `Omega x r` is given at face centres);
2. the surface-of-revolution check is applied only to internal faces between a zone cell and a non-zone cell (the frozen-rotor interface); instead,
   each zone cell's frame-flux closure `|sum_f (Omega x r_f).S_f|` (all faces of the cell) divided by `|Omega| sum_f r_perp,f |S_f|` is computed at
   construction and must not exceed 1e-10 (an exact identity for planar faces; larger values come from warped faces and would make the relative flux a
   mass source); a violation is a located error `sources.<name>.rotor_passage`. Measured at construction on every mesh of this ADR and recorded in
   amendment 1 before any run of the fix; if a registered mesh violates it, F-RP is NOT RUNNABLE on that mesh and amendment 1 says so;
3. the turbulence model is updated with the relative flux `phi - offset` instead of `phi` (k and omega are frame-invariant scalars, steady in the
   rotating frame);
4. nothing else changes; with the key absent or `false` every path is the present one.

Schema and `docs/case-schema.md` document it. The case that uses it: `type = "mrf"`, `zone = "all"`, `angular_velocity = Omega`, absolute boundary
conditions (inlet `(0, 0, U)`; shroud `fixed` 0 with `turbulence = "wall"`; hub and blade `fixed` with `velocity_expression_x/y = -Omega y, Omega x`
and the turbulence wall treatment), and the harness reads the velocities as absolute (`om_f = 0`).

## Acceptance of F-RP (all must pass; a fix that fails any of A0-A4 is removed, rule (b) of wave 35, nothing kept "for later")

Meshes: the X2 empty passage (13 524 cells) is M1. M2 and M3 refine every direction by 1.5 and 2.25: `n_up` 8/12/18, `n_chord` 12/18/27, `n_down`
8/12/18, `n_side` 10/15/23, `n_span` 16/24/36, `n_gap` 7/11/16, `n_mid` 1/2/2, `n_bl` 2/3/5, `first_cell_height` 2e-4/1.333e-4/8.889e-5 m,
`growth` 1.2/1.2^(1/1.5)/1.2^(1/2.25). Every other setting as X2. The per-direction largest and median cell-size ratios M1/M2, M2/M3 (axial,
pitchwise, radial) are measured at construction and recorded in amendment 1 (the order uses the nominal ratio 1.5; the measured ratios are reported).

Arms: **Y(h)** = the empty passage with F-RP (`mrf`, `rotor_passage = true`, absolute conditions: inlet axial, hub and shroud at rest), on M1-M3;
**B(h)** = the inertial control = ADR 0078 X2b on M1-M3 (the same boundary conditions as Y without the source: the MRF treatment is the only
difference); **R(h)** = ADR 0078 X2a (relative frame, upwind) on M2, M3 (M1 is X2a-36). Iteration-error check on M1: B(M1) and Y(M1) at the
registered tolerance 1e-5 and at 1e-7; if L changes by more than 10 % in either, all B and Y runs use 1e-7 (the M1 runs at 1e-7 are then the ones used).

* **A0 bit-identical when off.** (i) The full workspace suite at the final gate passes with the wave-35 totals plus this wave's tests, and
  `baseline_hashes.rs` is unchanged; (ii) X2a-36 re-run on the final code reproduces 2.25440 N m / -59.984 Pa to every printed digit; (iii) an ADR 0008
  M1 `mrf` case run 20 iterations with the key absent and with `rotor_passage = false`: all cell fields bit-identical.
* **A1 construction.** The closure check of item 2 passes on M1-M3 and on the FAN-01 E0 mesh; on the first iteration of the Y(M1) and F1 smokes the
  largest `|phi - offset|` on the faces of walls that rotate with the zone (F1: `blade`, `hub`) is at most 1e-12 `Omega r |S|` (catches a sign error),
  and on every periodic pair `|phi_rel,f + phi_rel,g|` <= 1e-12 `Omega r |S|`.
* **A2 empty-passage angular momentum, tied to the inertial result.** `|L_out(Y(h))| <= 10 |L_out(B(h))|` on M1, M2 and M3 (each pair at the same
  tolerance).
* **A3 empty-passage energy.** Outlet `rho <c_theta^2/2>_m` of Y(h) <= 0.2 Pa (1 % of X2d's 19.97 Pa; a decision threshold of this protocol) and the
  duct total-pressure change `Delta p_tt(Y(h)) <= 0` (a passive duct with walls at rest cannot gain total pressure), on M1-M3.
* **A4 order from the 3-mesh study.** The exact outlet angular-momentum flux is 0, so the error is `e_h = |L_out(Y(h))|`. Observed order
  `p = ln(e_M1 / e_M3) / ln(2.25)` >= 0.5 and `e_M3 <= e_M2 <= e_M1` (monotone). Decided now for the floor case: if on M1 `|L(Y)|` changes by more than
  50 % between the tolerances 1e-5 and 1e-7 run at the iteration check (so `e` is iteration error, not discretisation error), A4 is reported "not
  measurable (iteration floor)" and is neither passed nor missed; acceptance then rests on A0-A3. The same order is computed for B(h) and reported.
* Diagnosis, not acceptance: H1 predicts the relative-frame error to be first order: `p_R = ln(L(R(M1)) / L(R(M3))) / ln(2.25)` in [0.5, 1.5]. If
  R(M2) or R(M3) does not converge (X2c did not), the prediction is reported untested. `C_up` is also computed on M2 and M3 from Y's relative flux
  (`phi - offset`), with the same prediction (falls as `h^1`).

## FAN-01 with F-RP, once

**F1** = ADR 0078 E0 (13 332 cells, upwind, SST, design point Q = 1.4032 m^3/s) with F-RP and the absolute conditions above, the ADR 0078 stop rules,
the ADR 0078 quantities (torque of blade + hub on the fluid x 9, Delta p_ts and Delta p_tt with the ADR 0075 definitions, outlet swirl energy, cut-U
angular momentum, L excess). Run once, only if A0-A4 pass (A0(i) is the final gate; A0(ii), (iii) are run before F1). Reported as changes against
E0 (torque 1.32007 N m, Delta p_ts -3.813 Pa, Delta p_tt 112.520 Pa, outlet swirl energy 58.82 Pa, cut U 0.666 N m, L excess 2.061 N m) and against the
measured 2.142 N m / 126.46 Pa without claiming agreement. F1 is not an F-126 attempt; F-126 stays MISSED whatever it gives.

## Code policy

Tests in a new file `crates/rcfd-api/tests/fan01_rotor_frame_w36c.rs` (long runs ignored; construction, closure, bit-identity and the reconstruction
check on a short run not ignored when fast). The budget is test code only. F-RP in `crates/rcfd-api/src/sources.rs` (frame flux, check),
`crates/rcfd-api/src/sim.rs` (turbulence flux), `schema.rs`, `docs/case-schema.md`. `fan01_diagnosis_w35c.rs`, `fan01_validation.rs` and their tests are
not edited. Every new set-up gets a construction check and a 30-iteration smoke (continuity finite) before its registered run; the smoke values are
recorded in amendment 1.

## Amendment 1 (2026-10-20): X2a-36 result, the verdicts of H1-H5, and the departure to write F-RP; written before any F-RP code

What I knew when writing this: the X2a-36 run and its budget below (and a deterministic re-run of the same run with one extra print, the per-region
closed form, added after I had seen the first output; it is disclosed and not scored); X2d-36 was running and its result was not known. No F-RP code
existed and no other run of this wave had been made, apart from the not-ignored short bookkeeping test (5 iterations: reconstruction worst 1.8e-14,
the per-cell terms add up to `E_s + S_seam` to round-off).

X2a-36 (13 524 cells, 374 iterations, converged by tolerance, 0.7 min): outlet L 2.25440 N m, Delta p_ts -59.984 Pa: the reproduction rule is MET (every
printed digit). Budget (N m, full annulus): `E_s` 2.30241 (the ADR 0078 report-based excess of the same run 2.31265), `S_seam` 2.2e-6; `G_conv`
2.44178 (1.061 `E_s`), `G_p` 0.01106 (0.005), `G_visc` 0.00371 (0.0016), `G_frame` -0.15414 (-0.067; its Rhie-Chow part +0.13080 and the remainder
-0.28494, which alone is 0.124 `E_s`, partly cancel), `G_res` -2.4e-8; the terms add up to `E_s + S_seam` = 2.30241; reconstruction worst 2.4e-14.
Closed forms: `C_up` 2.62771 (`C_up / G_conv` = 1.076), `C_visc` 0.00387 (interior faces).

Location (`G_conv`, its closed form `C_up` per region in brackets, from the re-run): upstream 0.731 (0.736), chord band 0.378 (0.405), downstream
1.333 (1.487); wall-adjacent cells 0.034 (0.012; 0.3 % of the volume); seam-adjacent cells 1.566 (1.536; 1 288 cells holding 44 % of the volume,
because the upstream and downstream blocks have few pitchwise cells); radial bins: the mesh has no cell centres between r = 0.171 and 0.203 m
(mid-span radial cells several cm tall), and the bin 0.203-0.219 carries 1.028. The generation sits where `|phi| |d_perp|^2` is large: the large
cells of the upstream / downstream blocks and of mid-span.

Verdicts by the registered rules:
* H1 (i) `G_conv >= 0.7 E_s`: 1.061, passes. (ii) `C_up` in [0.7, 1.3] `G_conv`: 1.076, passes. (iii) wall-adjacent plus seam-adjacent cells carry
  at most 50 % of `G_conv`: the seam-adjacent cells alone carry 64 %: **H1 is FALSIFIED as registered** (through its location clause). The registered
  clause did not normalise by volume; it stays as registered and the falsification stands.
* H2: `|G_visc|` = 0.0016 `E_s` < 0.01 `E_s`: H2 falsified as a contribution (also below the predicted 1e-2 N m floor).
* H3: `|G_frame|` = 0.067 `E_s` < 0.1: holds (the further Rhie-Chow split is not triggered: the rule is on the total; the parts are reported above).
* H4: 0.005, holds. H5: 1e-6, holds. Closure (`|G_res|` <= 0.05 `E_s`): passes. Reconstruction (1e-10): passes.

Departure from the registered plan (rule (c) of wave 35), stated before any F-RP code: the ADR registered "the fix is written only if H1 is not
falsified". H1 is falsified through (iii) while (i) and (ii) identify the term: convection carries 106 % of the excess and the closed form of the
upwind dissipation acting on the rigid part matches it to 8 %. The task of this wave asks to isolate the responsible term and to decide whether a fix
is possible; the term is isolated, so F-RP is written anyway, with the acceptance A0-A4 unchanged. Added now, as a new falsifier of the mechanism and
not as a relaxation: F-RP removes exactly the rigid part from the convected quantity, so if A2 fails on Y(M1) (`|L(Y)|` not down to 10 x the inertial
control), then either the mechanism read from (i)/(ii) is wrong or F-RP is wrong, and in both cases F-RP is removed.

Before any F-RP solver code, the registered closure check (item 2 of F-RP) is measured on M1, M2, M3 and the E0 mesh by a construction-only test
(geometry only): worst value, its location and the distribution. If it exceeds 1e-10 on a registered mesh, F-RP is NOT RUNNABLE as registered and any
remedy (for example a frame flux exact on the triangulated faces, which would change the A1 wall check) needs its own amendment before a run. The
same test records the largest and median cell extents (axial, radial, pitchwise) of M1-M3; the order of A4 keeps the nominal ratio as registered and
is reported next to the measured largest-cell ratios.
