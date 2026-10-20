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

## Amendment 2 (2026-10-20): X2d-36, the construction measurements, F-RP NOT RUNNABLE as registered, and its remedy; written before any F-RP code

What I knew when writing this: everything of amendment 1, the X2d-36 run and budget, and the construction-only measurements below (geometry only, no
solver run of F-RP; no F-RP code existed).

X2d-36 (minmod, 8000 iterations, 14.9 min, not converged by tolerance, res_u 5.3e-4 at the cap; ADR 0078 accepted X2d by its 1000-iteration rule):
outlet L 0.00258 N m and outlet `rho<c_theta^2/2>` 19.970 Pa reproduce ADR 0078 (0.0026, 19.97). Budget: `E_s` -0.00327, `S_seam` -3.3e-6, `G_conv`
+0.86140, `G_frame` -0.88300 (Rhie-Chow part +0.09790, remainder -0.98090), `G_p` 0.01226, `G_visc` 0.00671, `G_res` -6.5e-4; reconstruction worst
2.8e-14 (the minmod face values, the least-squares gradient and the non-orthogonal Laplacian term are rebuilt exactly). Closure by the registered rule
(`|G_res|` <= 0.05 `|E_s|`) is NOT met (0.198) because `E_s` is near zero; `|G_res|` is 7.5e-4 of `G_conv`. By the rule the X2d attribution is NOT
CLOSED and no term is called responsible for it. Reported as observations: the near-zero net is a cancellation of convection (+0.86) and the frame
term (-0.88, its quadrature part: the cell Coriolis torque against the flux of `Omega r^2`), both concentrated in the large downstream cells
(downstream: conv +0.834, frame -0.888; `sum |.|` 1.87 and 3.60) and of opposite sign in neighbouring radial bins (r 0.156-0.171: conv -0.425, frame
+1.155; r 0.203-0.219: conv +1.251, frame -2.090): a radial dipole of angular-momentum generation with zero net, which leaves swirl of both signs. The
budget locates generation of angular momentum, not of swirl energy; the 20 Pa are the energy of that two-signed swirl, not attributed by a rule.

Construction (amendment 1 test `w36c_construction_closure_and_extents`): the registered per-cell closure of the face-centre frame flux
`|sum_f (Omega x r_f).S_f| / (|Omega| sum_f r_perp,f |S_f|)` is far above the registered 1e-10: worst 5.3e-4 (M1), 2.1e-4 (M2), 1.7e-4 (M3), 3.6e-4
(E0 mesh); median 4.7e-9 / 1.5e-9 / 4.2e-10 / 4.3e-9; 77 % of the cells exceed 1e-10 (the faces of the camber-sheared cells are warped). **F-RP as
registered is NOT RUNNABLE on every registered mesh.** Measured with the same test: the frame flux integrated exactly over the mesh's own triangle fan of
each face (`F_tri = sum_t (Omega x c_t).S_t`, the decomposition of `polygon_geometry`, exact for the linear field `Omega x r` on each planar
triangle) closes to 1.2e-15 / 1.4e-15 / 2.2e-15 / 1.2e-15 (an identity: the cells' closed triangulated surfaces). The largest face difference
`|F_tri - (Omega x r_f).S_f| / (|Omega| r |S|)` is 5.0e-3 (M1, E0), 3.2e-3 (M2, M3).

Cell extents, largest / median (axial, radial, pitchwise r dtheta, m): M1 0.188/2.11e-3, 0.0386/7.66e-4, 0.0445/4.87e-3; M2 0.146/1.26e-3,
0.0287/3.59e-4, 0.0323/3.09e-3; M3 0.111/8.89e-4, 0.0213/2.22e-4, 0.0227/2.00e-3. Largest-cell ratios M1/M2 and M2/M3: axial 1.29, 1.31; radial
1.35, 1.35; pitchwise 1.38, 1.42 (nominal 1.5): the generator's core cells (where H1 generation lives) refine by about 1.3-1.4 per step, not 1.5. A4
keeps the nominal ratio as registered; the order with the measured largest-cell ratio is reported next to it.

Remedy (changes F-RP before any of its code or runs; acceptance otherwise unchanged): with `rotor_passage = true` the frame flux of EVERY face of the zone
(internal, interface, boundary) is `F_tri`, not `(Omega x r_f).S_f` (the default `mrf` path keeps the face-centre form). Item 2 becomes: the per-cell
closure of `F_tri` must not exceed 1e-10 (measured 2.2e-15 at most). Consequence for A1, restated now: on a wall rotating with the zone the absolute
flux of the kernel is the prescribed face-centre `(Omega x r_f).S_f`, so the relative flux there is the face's warp term, not zero. A1 (wall) becomes:
`|phi_rel,f + (F_tri,f - (Omega x r_f).S_f)|` <= 1e-12 `Omega r |S|` on every `blade` and `hub` face of F1 at iteration 1 (the relative flux equals
minus the warp term exactly: catches a sign error), and the largest `|phi_rel| / (Omega r |S|)` on those faces is reported. The seam-pair check of A1
is unchanged. Mass stays exact (`sum_f phi_rel = sum_f phi - 0` per cell).

## Amendment 3 (2026-10-20): F-RP smokes and A1, the M3 set-up defect and its replacement M0; written before any registered run of B, Y, R or F1

What I knew when writing this: everything above; F-RP is written (triangle-exact frame flux on every zone face with `rotor_passage = true`, the
interface-only revolution check, the per-cell closure check at construction, turbulence on the relative flux; default path unchanged); the not-ignored
tests pass (A0(iii): the ADR 0008 M2 annulus with the key absent and `false`, 20 iterations, bit-identical velocity, pressure and flux; the key is
refused at `sources.frame.rotor_passage` on a `rotating_frame` source; the budget bookkeeping test); the construction and 30-iteration smokes below
(and two scratch looks, disclosed: F1 to 300 iterations, and the pressure-solver iteration counts of the first 5 iterations of Y(M1), F1, E0). No B, Y
or R run beyond 30 iterations, and no F1 run beyond 300.

A1 (registered checks, iteration 1): seam pairs `|phi_rel,f + phi_rel,g|` / (Omega r |S|) Y(M1) 2.1e-13, Y(M2) 3.7e-13, Y(M3) 6.6e-13, F1 2.1e-13
(limit 1e-12: pass); F1 rotor walls (`blade`, `hub`) `|phi_rel + warp|` 2.6e-16 (limit 1e-12: pass), largest `|phi_rel|` 5.0e-3 of Omega r |S| (the
largest face warp term, as predicted). The closure check of `Simulation::new` passed on every F-RP set-up (it builds).

Smokes (continuity at iterations 1 / 10 / 20 / 30): Y(M1) 1.7e-6 / 2.7e-6 / 1.2e-7 / 1.3e-7; Y(M2) 1.6e-6 / 5.6e-7 / 3.9e-7 / 2.6e-7; Y(M3) 1.2e-6 /
7.9e-6 / 6.3e-4 / 6.6e-2 (growing); R(M2) 5.5e-4 / 3.7e-5 / 3.6e-5 / 2.7e-5; R(M3) 2.0e-4 / 7.2e-4 / 1.1e-2 / 2.8e-1 (growing); B(M2) 4.2e-6 / 1.1e-6
/ 5.4e-7 / 9.3e-6; **B(M3) 1.2e-6 / 1.9e-5 / 1.3e-2 / 1.2e-1 (growing; Delta p_tt 22.7 kPa and outlet L 8.2 N m at iteration 30 in the inertial empty
duct)**; F1 0.49 / 0.38 / 0.40 / 0.38 (and 0.30 at iteration 300, res_u 0.47, torque 1.79 N m; the pressure equation is solved in 5 iterations each
time, so the imbalance is the 1e-3 relative linear tolerance applied to a large initial imbalance next to the rotating hub and blade, not a failed
solve; E0's continuity at iteration 1 is 1.6e-4).

Set-up defect: M3 diverges within 30 iterations in all three formulations, including the inertial control B, so the registered finest mesh is a
defective set-up (the cause, the M3 counts with 5 boundary-layer cells of growth 1.084 or the 157 248-cell H-slit at 79 deg with SIMPLEC, was not
diagnosed). It stays recorded here; no run of it is used. Replacement, registered now, before any run: **M0**, one step coarser than M1 by 1.5
(`n_up` 5, `n_chord` 8, `n_down` 5, `n_side` 7, `n_span` 11, `n_gap` 5, `n_mid` 1, `n_bl` 1, `first_cell_height` 3e-4 m, `growth` 1.2^1.5; 4 320
cells; smokes finite: B(M0) 2.0e-6 / 7.4e-6 / 2.7e-6 / 7.3e-7, Y(M0) 1.8e-6 / 2.2e-6 / 2.4e-6 / 2.9e-6, R(M0) 6.0e-4 / 2.3e-4 / 2.2e-4 / 2.0e-4;
frame-flux closure of `F_tri` 9.5e-16; extents largest / median axial 0.228 / 3.15e-3, radial 0.0707 / 1.14e-3, pitchwise 0.0543 / 7.06e-3 m:
largest-cell ratios M0/M1 1.21, 1.83, 1.22, median ratios 1.49, 1.49, 1.45). The family of A2-A4 is **M0, M1, M2**; A4 becomes
`p = ln(e_M0 / e_M2) / ln(2.25)` >= 0.5 with `e_M2 <= e_M1 <= e_M0`, its floor rule unchanged (iteration check on M1). The diagnosis family R is
R(M0), R(M1) = X2a-36, R(M2), with `p_R = ln(L(R(M0)) / L(R(M2))) / ln(2.25)`. Everything else is unchanged.

## Addendum (2026-10-20): measured values, verdicts, removal of F-RP, misses and deviations

All runs: release builds of c89495f plus this wave's files, single thread per run, at most two runs at a time on a shared machine, one run per
set-up. Full annulus values, `rho` = 1.2.

### Where the spurious swirl comes from (answer to the diagnosis)

The term is the momentum convection. First-order upwind on the Cartesian components of the relative velocity acts as a numerical viscosity in
Laplacian form on its rigid part `-Omega x r`. A Laplacian-form stress on a rigid rotation (zero physical strain) is a torque that drives the relative
swirl toward zero, i.e. creates positive absolute swirl: per face `(Omega/2) |phi_f| |d_perp|^2`, sign-definite. On X2a the convection term is
2.442 N m of the 2.302 N m solver-consistent excess (106 %), and the closed form is 2.628 N m (1.076 of it). The other terms: pressure lever arms
0.011, the physical viscous term 0.004, seam pairs 2e-6, frame term -0.154 (Rhie-Chow part +0.131, quadrature remainder -0.285), residual 2e-8. It
is generated where `|phi| |d_perp|^2` is large: the large cells of the upstream (0.73) and downstream (1.33) blocks and of mid-span (one radial bin
1.03), much less in the fine chord band (0.38) and almost none in the wall layers (0.03). With minmod (X2d) the net vanishes by cancellation, not by
absence: convection +0.86 against the frame term -0.88 (its quadrature part -0.98, Rhie-Chow +0.10), both in the large downstream cells and as a
radial dipole (r 0.156-0.171: convection -0.42, frame +1.15; r 0.203-0.219: +1.25, -2.09). That leaves swirl of both signs and the 20 Pa. The X2d
budget is not closed by the registered rule (`E_s` is near zero), so this is an observation, and the budget locates generation of angular momentum,
not of swirl energy.

### Checks with limits

| Check | Limit | Measured | Result |
|---|---|---|---|
| X2a-36 reproduces ADR 0078 X2a | every printed digit | 2.25440 N m, -59.984 Pa | pass |
| X2d-36 reproduces ADR 0078 X2d | printed digits | 0.00258 N m, 19.970 Pa | pass |
| Reconstruction of the convection and Laplacian fluxes | 1e-10 | 2.4e-14 (X2a), 2.8e-14 (X2d), 8.5e-15 (R(M0)), 4.9e-14 (R(M2)) | pass |
| Budget identity (terms against `E_s + S_seam`), short run | 1e-9 | round-off | pass (not-ignored test) |
| Closure `abs(G_res)` <= 0.05 `abs(E_s)` | 0.05 | X2a 1e-8; X2d 0.198 | X2a pass; X2d NOT CLOSED |
| H1 (i) `G_conv` >= 0.7 `E_s` (X2a) | 0.7 | 1.061 | pass |
| H1 (ii) `C_up` in [0.7, 1.3] `G_conv` (X2a) | [0.7, 1.3] | 1.076 | pass |
| H1 (iii) wall- plus seam-adjacent share of `G_conv` | <= 0.5 | seam-adjacent alone 0.64 (44 % of the volume) | MISS: H1 falsified as registered |
| H2 `abs(G_visc)` significant / falsified | >= 0.1 / < 0.01 `E_s` | 0.0016 | falsified as a contribution |
| H3 `abs(G_frame)` < 0.1 `E_s` | 0.1 | 0.067 | holds |
| H4 `abs(G_p)` < 0.1 `E_s` | 0.1 | 0.005 | holds |
| H5 `abs(S_seam)` < 0.01 `E_s` | 0.01 | 1e-6 | holds |
| H1 order (diagnosis), `p_R` from R(M0), R(M2) | [0.5, 1.5] | 0.88 (all three converged: 3.4396, 2.2544, 1.6793 N m) | holds |
| F-RP item 2, face-centre frame-flux closure | 1e-10 | worst 1.3e-3 (M0) to 1.7e-4 (M3) | MISS: NOT RUNNABLE as registered (amendment 2 remedy) |
| F-RP item 2, triangle-exact frame flux | 1e-10 | 9.5e-16 to 2.2e-15 | pass |
| A0 (iii) key absent = `false` (ADR 0008 annulus, 20 iterations) | bit-identical | bit-identical | pass |
| A1 seam pairs (iteration 1) | 1e-12 | 2.1e-13 to 6.6e-13 | pass |
| A1 rotor walls `abs(phi_rel + warp)` (F1, iteration 1) | 1e-12 | 2.6e-16 (largest `abs(phi_rel)` 5.0e-3, the warp term) | pass |
| A2 `abs(L(Y))` <= 10 `abs(L(B))`, M1 | 1.63e-3 N m | 1.42e-5 N m (B 1.63e-4) | pass |
| A2, M0 | 5.26e-3 N m | Y(M0) NOT CONVERGED (8000 iterations, res_u 0.78), L 0.068 N m | MISS |
| A2, M2 | 2.2e-3 N m | Y(M2) and B(M2) NOT CONVERGED at the 60-min cap (4.4e-6 against 2.2e-4, unscored) | MISS (no converged pair) |
| A3 swirl energy <= 0.2 Pa and Delta p_tt <= 0, M1 | 0.2 Pa, 0 Pa | 0.0000 Pa, -1.541 Pa | pass |
| A3, M0 | as above | Y(M0) not converged; Delta p_tt +981 Pa | MISS |
| A4 order (`e_M2 <= e_M1 <= e_M0`, p >= 0.5) | 0.5 | Y(M0), Y(M2) not converged; the floor rule did not trigger (M1: L of Y changed by 0.01 % between tolerances 1e-5 and 1e-7, B by 0.001 %) | MISS |
| A0 (i), (ii) | gate; X2a digits | F-RP removed before the gate; no solver code of this wave is committed | not applicable |

### Verdict on the fix

F-RP (the opt-in absolute-velocity rotor passage, `sources.<name>.rotor_passage`) misses A2, A3 and A4 and is REMOVED (rule (b) of wave 35; the ADR
registered removal). It was never committed; the code is not in the repository. F1 (FAN-01 E0 with F-RP) was NOT RUN, because the ADR runs it only
if A0-A4 pass. No torque or Delta p_ts change with the fix is reported. The only bladed F-RP numbers are a disclosed 300-iteration scratch look
(torque 1.79 N m, continuity 0.30, res_u 0.47, not converged), which is not a result.

What the runs do show: a fix is possible in principle. On M1, where Y converged (567 iterations), removing the rigid part from the convected quantity
removes the source: outlet angular-momentum flux -1.42e-5 N m against 2.254 N m in the relative formulation and 1.63e-4 N m in the inertial control,
swirl energy 0.0000 Pa, duct Delta p_tt -1.54 Pa (inertial -1.53). This passes the mechanism falsifier registered in amendment 1 (A2 on Y(M1)), so
the mechanism stands by that test. The fix was not delivered because this implementation does not converge on the coarser mesh M0 (the inertial
control converges there in 124 iterations and the relative formulation in 247, so M0 is not a defective set-up) and did not converge on M2 within the
60-minute cap (nor did the inertial control B(M2): 7 063 iterations, against 169 on M1; two runs shared the machine). The bladed F1 smoke had a
continuity of about 0.4 for 300 iterations. Robustness of the lagged `-Omega x c` source with SIMPLEC on these meshes is the open item; it was not
diagnosed.

### Mesh dependence of the attribution (stated against the registered reading)

R(M0) (4 320 cells): `E_s` 3.495, `G_conv` 2.075 (0.594 `E_s`), `G_frame` 1.353 (0.387; Rhie-Chow part 0.292), `C_up / G_conv` 1.475 (outside
the [0.7, 1.3] window used for X2a). R(M2) (47 040 cells): `E_s` 1.710, `G_conv` 1.777 (1.039), `G_frame` -0.074, `C_up / G_conv` 1.065. The
attribution to convection is clean on M1 and M2; on the coarsest mesh the frame-term quadrature carries a large share. The criteria were registered
on X2a, so no verdict changes; "convection is the term" holds for M1 and M2, not for M0.

Order of the relative-frame error: nominal ratio 2.25 over M0-M2 gives `p_R` = 0.88 (pairwise 1.04 and 0.73). With the measured cell-size ratios
M0/M2 (largest: axial 1.56, radial 2.46, pitchwise 1.68; median: 2.49, 3.18, 2.28) the same two errors give 1.61 (largest axial), 1.38 (largest
pitchwise), 0.87 (median pitchwise). `C_up` falls with the error: 3.060, 2.628, 1.893 N m. The error is a first-order discretisation error that
falls with refinement, as ADR 0078 concluded, now shown on a converged three-mesh family.

### Deviations from the registered plan (candid)

* H1 was falsified through its location clause (iii), and the fix was written anyway (amendment 1, before any fix code; acceptance unchanged).
* The registered face-centre frame flux failed its own closure check; the triangle-exact frame flux and the restated A1 wall check were registered in
  amendment 2 before any F-RP code.
* M3 was a defective set-up (the inertial control diverged within 30 iterations) and was replaced by the coarser M0 (amendment 3), so the family is
  M0-M2 with a ratio of 1.5 per step, not M1-M3.
* The per-region closed form of the location report was added after the first X2a output (deterministic re-run, disclosed, not scored).
* Scratch, disclosed: F1 to 300 iterations; the pressure-solver iteration counts of the first 5 iterations of Y(M1), F1 and E0 (5-6 each).
* The steady-guard record was added to the run loop as a stop rule (registered); it never tripped.
* A0 (ii) (X2a reproduction on the final code) is moot: the final code is c89495f's solver plus a test file.
* The ignored tests that ran the Y and F1 arms were deleted with F-RP (they need the removed key); their values are only in this ADR. The remaining
  ignored tests run the budget (X2a, X2d), the construction measurements, the B and R arms.

### Unverified references

None. Every limit is derived here, measured, or a decision threshold of this protocol; no external value is used.
