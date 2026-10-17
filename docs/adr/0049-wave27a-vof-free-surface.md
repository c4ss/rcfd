# ADR 0049: Wave 27A, volume-of-fluid free-surface multiphase with interface compression and surface tension (F-055)

Status: accepted. Everything under "Decisions" and "Registered checks" was written before any code of this wave and before any run
of this wave. Limits are not changed, widened, replaced or deleted after a run. A miss is reported as measured and the assertion
moves to an ignored `*_known_miss` test with the registered limit intact; the cause is diagnosed, not tuned away. A test set-up
defect gets a NEW corrected test and the old one stays as a record. Addenda (exact numbers, deviations) may be appended at the end
without touching a registered limit.

Scope. F-055 is `todo` and is the base that blocks the case-tree wiring of F-226 (interfacial mass transfer, library only),
F-227 (wave boundaries, library only), F-228 (population balance, library only) and melt flow in F-212. This wave delivers a
vertical slice: two incompressible immiscible fluids, algebraic VOF with interface compression and a bounded explicit limiter,
continuum-surface-force tension, well-balanced gravity, transient PISO/PIMPLE coupling, wiring into the case tree. The item stops
at `partial` with named gaps (below). Clean-room: first principles and published literature only (references listed with their
verification status in `docs/reference-data.md`); no vendor code, text, names or data; nothing from vendor material.
Other waves (27B solver robustness / adaptive stepping, 27C viscoelastic / structural export) work in the same tree: this wave
adds new files (`rcfd-fv/src/vof.rs`, `rcfd-api/src/vof.rs`, tests) and only small hunks in shared files (`lib.rs`, `sim.rs`,
`schema.rs`, `case-schema.md`, `reference-data.md`, `roadmap.json`), committed through a temporary index.

## Audit (HEAD 3bf8090)

* `PimpleSolver` (`pimple.rs`) is kinematic: constant `nu`, pressure divided by density, `ddt(U) + div(phi U) - div(nu grad U)`.
  Its `body_force` (`BodyForce { cell, face }`) is a separate cell acceleration plus a face acceleration; the cell velocity is
  `HbyA + rAU f_c - rAU grad p` with a Green-Gauss `grad p`. That pattern is balanced only up to the interpolation error of the
  cell versus face force; it does not give velocity zero to round-off for a density jump. It is not reused for VOF.
* `varden.rs` is a steady/transient ideal-gas (equation-of-state) pressure-based solver. Its density is a function of pressure
  and temperature, it solves an energy equation and a compressible pressure equation. Two incompressible immiscible fluids with a
  transported phase fraction share none of that. It is not the host; its pattern (mass flux `rho phi` in the convection operator,
  density in the diagonal so that `rAU = V / a_p`) is reused in the new kernel.
* Reusable building blocks, called unchanged: `convection` (`div(F u)`, upwind implicit plus deferred higher order), `laplacian`
  (`-div(gamma grad)` with a per-face `gamma`), `add_ddt`, `face_flux`, `face_interpolate`, `pressure_bc`, `solve_pressure` (takes
  any per-face `gamma`), `corrected_flux`, `boundary_hby_a`, `green_gauss`, `FvMatrix`. `pimple.rs` is not edited, therefore every
  single-phase run and every baseline hash is unchanged by construction.
* The case tree has no phase fraction. `solver.algorithm` is one of simple, simplec, coupled, piso, pimple; `fluid.density`
  selects the ideal-gas solvers; unset values use explicit sentinels (case-schema "Every new key has an explicit unset sentinel").

## Decisions

### D1 Governing equations (two incompressible immiscible fluids, no phase change)

Phase 1 is `alpha = 1` (density `rho_1`, dynamic viscosity `mu_1`), phase 2 is `alpha = 0`. Mixture properties are linear in alpha,
`rho = alpha rho_1 + (1 - alpha) rho_2`, `mu = alpha mu_1 + (1 - alpha) mu_2` (arithmetic; cell values, linear face interpolation).

    d(alpha)/dt + div(alpha u) + div(alpha (1 - alpha) u_r) = 0,        div u = 0
    d(rho u)/dt + div(rho u u) - div(mu (grad u)) = -grad p + rho g + sigma kappa grad(alpha)

`u_r` is the artificial compression velocity (Rusche 2002 thesis form, Weller 2008 report form): in flux form
`phi_r = c_alpha |phi_f| / |S_f| (n_f . S_f)` with the interface normal `n_f = grad(alpha)_f / (|grad(alpha)_f| + delta)`,
`delta = 1e-8 / V_mean^(1/3)`, `c_alpha` the compression coefficient (default 1). The term is zero where `alpha = 0` or `1` and
acts only inside the interface layer; it is conservative (a flux). The viscous term uses the Laplacian form of the stress
divergence with the face viscosity (the transposed gradient part `div(mu (grad u)^T)` is neglected; consequence: the viscous
force at a viscosity jump is not the full Newtonian one; a named gap).

### D2 Interface capturing: explicit MULES-type flux-corrected scheme (chosen) instead of CICSAM/HRIC

Choice: flux-corrected transport (Boris and Book 1973; Zalesak 1979 multidimensional limiter), the explicit variant used in
algebraic VOF. Reasons: bounded by construction under a stated Courant restriction (CICSAM-type differencing is bounded only on
the normalised-variable diagram and needs the angle blending), the bound is local-extremum diminishing, phase volume is conserved
exactly because every face carries one flux used by both neighbours, and it composes with the compression flux of D1.

Per sub-step `dt_s` with the frozen volumetric face flux `phi` (positive out of the owner):

1. Low-order flux `F_L,f = phi_f alpha_up(f)` (first-order upwind, boundary faces `phi_b alpha_b`).
2. High-order flux `F_H,f = phi_f alpha_lin,f + phi_r,f alpha_lin,f (1 - alpha_lin,f)`, `alpha_lin` the linear interpolation
   (boundary faces: `F_H = F_L`, no compression and no anti-diffusion at a boundary).
3. Low-order solution `alpha_L = alpha - (dt_s / V) sum_f F_L,f` (sum over the faces of the cell with outward sign).
4. Anti-diffusive flux `A_f = F_H,f - F_L,f`. Zalesak limiter: for every cell the sums of anti-diffusive fluxes that would raise /
   lower it, `P+ = sum max(-a, 0)`, `P- = sum max(a, 0)` (`a` = outward `A_f` of that cell); the room `Q+ = (V / dt_s)
   (alpha_max - alpha_L)`, `Q- = (V / dt_s)(alpha_L - alpha_min)` with `alpha_max = min(1, max over the cell and its face
   neighbours of {alpha^k, alpha_L})` and `alpha_min = max(0, min over the same set)`; `R+ = min(1, Q+ / P+)`, `R- = min(1, Q- /
   P-)` (1 if the sum is zero, 0 if `Q < 0` by round-off); the face factor `C_f = min(R+_nb, R-_own)` for `A_f >= 0` (flux leaves
   the owner) and `min(R+_own, R-_nb)` otherwise; `F_f = F_L,f + C_f A_f`.
5. `alpha^{k+1} = alpha^k - (dt_s / V) sum_f F_f`. No clipping is applied (clipping would break volume conservation).

Sub-cycling: `n_sub = ceil(Co_alpha / Co_max)` with `Co_alpha = dt max_c (sum over outflow faces of |phi|) / V`, `Co_max = 0.25`
(`multiphase.alpha_courant`); the flux is frozen over the step; the time-averaged total flux `alphaPhi` of the sub-steps is kept.
Mass-momentum consistency (needed at density ratio 1000): the momentum convection uses `rhoPhi = rho_2 phi + (rho_1 - rho_2)
alphaPhi`, so that the discrete density update `rho^{n+1} = rho^n - (dt / V) sum rhoPhi` holds up to `rho_2 dt div(phi)`.

Boundedness is proved for `div(phi) = 0` exactly and for `div(phi) != 0` up to `dt |div(phi)| / V` (upwind with a non-solenoidal
flux overshoots by that amount). The pressure equation is solved to `pressure_rel_tol = 1e-12` in the registered runs; the
registered boundedness tolerance of coupled runs is tied to it (check Z1). No `alpha div(phi)` correction is applied, because it
would make the phase-volume conservation hold only to the continuity error instead of to round-off.

### D3 Surface tension (Brackbill, Kothe and Zemach 1992, continuum surface force)

`F_sigma = sigma kappa grad(alpha)` per unit volume, `kappa = -div(n)`, `n = grad(alpha_s) / (|grad(alpha_s)| + delta)` from a
smoothed fraction `alpha_s` (`n_smooth` passes of the equal-weight neighbour average `alpha_s,c = (alpha_c + sum_nb alpha_nb) /
(1 + n_nb)`, default 2). `n_f` is the face interpolation of the cell Green-Gauss gradient normalised; `kappa_c = -sum_f n_f . S_f /
V` over internal faces (boundary faces contribute nothing: a 90 degree contact angle, no wall adhesion); `kappa_f` the linear
interpolation. The tension enters as a FACE force (D4). Convention: `n` points into phase 1, a phase-1 drop has `kappa > 0` and
the pressure jump `p_in - p_out = sigma kappa` (`sigma / R` in 2-D, `2 sigma / R` in 3-D). Capillary time-step restriction
(Brackbill 1992; the constant is quoted from memory and UNVERIFIED, so the most restrictive reading is used):
`dt_cap = sqrt((rho_1 + rho_2) dx_min^3 / (8 pi sigma))`; `VofSolver::capillary_dt()` returns it and a step larger than `dt_cap`
returns a warning flag in the step info (not an error: the registered runs use `dt <= dt_cap`).

### D4 Well-balanced pressure-velocity coupling (p_rgh formulation)

`p_rgh = p - rho (g . x)`; then `-grad p + rho g = -grad p_rgh - (g . x) grad rho`. All forces are FACE quantities built with the
same `snGrad` and `lap_coeff` as the pressure term, and the cell velocity is reconstructed from the SAME face quantities:

    f_f = lap_coeff,f [ sigma kappa_f (alpha_N - alpha_P) - (g . x_f)(rho_N - rho_P) ]            (force flux, internal faces)
    phiHbyA_f = (HbyA_f . S_f) + rAU_f/dt-correction + rAU_f f_f
    div(rAU_f grad p_rgh) = div(phiHbyA)         (solved with the existing `solve_pressure`, gamma = rAU_f)
    phi = phiHbyA - rAU_f lap_coeff (p_rgh,N - p_rgh,P)           (existing `corrected_flux`)
    q_f = (phi_f - phiHbyA0_f) / rAU_f           (net face acceleration flux; phiHbyA0 = phiHbyA without the force term)
    U_c = HbyA_c + rAU_c reconstruct(q)_c,       reconstruct(q) = (sum_f S_f S_f / |S_f|)^-1 sum_f (S_f / |S_f|) q_f

(restricted to the active dimensions of the mesh). The momentum predictor uses the reconstruction of the same face force flux
minus the face `p_rgh` flux of the previous iterate, not a Green-Gauss pressure gradient. At walls and other fixed-velocity faces
`snGrad(rho) = snGrad(alpha) = 0` (zero-gradient phase fraction), hence `d p_rgh / dn = 0`, and the boundary-face force in
`phiHbyA` is zero, identical to the pressure condition. Consequence (checked in B1): in a still two-layer pool with the interface
on a cell face, the discrete `p_rgh` solves every face flux to zero, `phi = 0` and `U = 0` hold to the linear-solver tolerance, and
`p = p_rgh + rho (g . x)` reproduces the piecewise hydrostatic pressure: across the interface face
`p_N - p_P = -(g . x_f)(rho_N - rho_P) + rho_N (g . x_N) - rho_P (g . x_P) = rho_N g . (x_N - x_f) + rho_P g . (x_f - x_P)`, the
exact two-layer hydrostatic integral. With the interface inside a cell this equality is not exact by `rho g h` (O(dx) in the
interface cell) and is therefore NOT claimed; only `U = 0` is claimed for that case. The hold-up of the algebra depends on the
solver tolerance: the registered runs set `pressure_rel_tol = 1e-13`, `pressure_abs_tol = 1e-16`, `pressure_max_iter = 5000`.
Rhie-Chow transient correction: `rAU_f (rho_old,f / dt) (phi_old - U_old,f . S)` added to `phiHbyA` (same structure as the
single-phase kernel with the old density weight). The output pressure is `p = p_rgh + rho (g . x)` (Pa); the pressure level is
fixed by `p_ref` on `p_rgh` (closed domains).

### D5 Time stepping and algorithm

Euler time scheme (BDF2 is not offered with variable density: a named gap). Per step: (1) sub-cycled alpha advection with the flux
of the end of the previous step (first outer iteration only); (2) mixture properties, `rhoPhi`; (3) the PISO/PIMPLE loop with
`ddt(rho U) + div(rhoPhi U) - div(mu_f grad U) = F` in the predictor, `n_correctors` pressure correctors, `n_outer` outer
iterations with the same relaxation structure as `PimpleSolver` (alpha and the mixture properties are held over the outer
iterations). The curvature and the face forces are evaluated once per step from the new alpha. Periodic patches, non-conformal
interfaces, moving meshes, fractional step, `symmetry` walls and compressible modes are refused.

### D6 Case tree (additive; a case without `multiphase.enabled` is unchanged)

New object `multiphase` (schema in `rcfd-api/src/vof.rs`): `enabled` (false), `model` ("vof"), `density_1`, `density_2`,
`viscosity_1`, `viscosity_2` (all `-1` = unset, required positive when enabled), `surface_tension` (0), `compression` (1),
`alpha_courant` (0.25), `curvature_smoothing` (2), and the initial fraction `initial.shape` ("unset" | "box" | "sphere" |
"half_space"), `initial.box_min`/`box_max`, `initial.centre`, `initial.radius`, `initial.normal`, `initial.offset`, `initial.inside`
("phase_1" | "phase_2" for the two sides, default "phase_1"). Cells are sub-sampled `10^dim` times so a face-aligned box is exact.
Gravity is `physics.gravity`; wall conditions are the existing `boundaries.<patch>` conditions (`fixed` with value 0 = no-slip). New
fields `alpha` (and `p_rgh`) in `Simulation::field`. Located errors (path in the tree) when `multiphase.enabled` is true and:
`solver.algorithm` is not `piso`/`pimple` (path `solver.algorithm`); `fluid.density` is not `constant`; a turbulence model is
selected; `energy.enabled`; `sources`/`components` non-empty; `dynamic_mesh.enabled`; `solver.time_advance = fractional_step`;
`statistics.enabled`; a boundary uses `symmetry`, `periodic` or `partial_slip`/`marangoni`; `physics.rho` or `physics.nu` is set to a
non-default value (it would be ignored; "none shadows another value"); `physics.body_force` non-zero; a required `multiphase.*` is
unset. `multiphase.*` keys set while `enabled = false` are an error (no silent ignoring).

## Registered checks (limits fixed before any run of this wave)

Meshes are `box_2d` (one cell across, `empty` front/back). `L1` relative: `sum |alpha - alpha_ref| V / sum alpha_ref V`.
"Mixed cell" = `0.01 < alpha < 0.99`. "Interface width in cells" = (volume of mixed cells) / (interface length x dx).

**Z1 Boundedness and conservation (every run below).** Pure advection with a discretely solenoidal prescribed flux: `min alpha >=
-1e-12`, `max alpha <= 1 + 1e-12`. Coupled runs (`pressure_rel_tol <= 1e-12`): `-1e-9 <= alpha <= 1 + 1e-9` at every step (expected
1e-12 or better). Phase volume: closed domain `|sum alpha V - sum alpha_0 V| / sum alpha_0 V <= 1e-12` at every step (advection runs:
as long as the interface is away from the boundary). Expected: both at round-off.

**A. Pure advection (library test `tests/vof_advection.rs`; prescribed face flux, momentum not solved).** `c_alpha = 1`, `Co_max = 0.25`.
Algebraic VOF is much less accurate than a geometric (PLIC) scheme: the expected L1 error is of the order of one interface
cell width times the interface length per unit area, 1e-2 to 1e-1 for the shapes below.
* A1 1-D slug, 200 cells over `[0,1]` (cell height `1/200`), alpha = 1 on `[0.1, 0.3]`, `u = 1`, end `t = 0.4` (80 cells), flux
  Courant 0.8 per step (sub-cycled). Limits: Z1; L1 relative to the exact translated slug `<= 0.05` (expected 0.01 to 0.03);
  total number of mixed cells `<= 8` (expected 4 to 6; 2 interfaces). The same run with `c_alpha = 0` is reported (no limit
  except Z1; expected to smear to more mixed cells).
* A2 2-D translated disk, 64 x 64 over the unit square, `R = 0.15` at (0.3, 0.3), `u = (1, 0.5)`, `t = 0.3`. Limits: Z1; L1 `<=
  0.10` (expected 0.03 to 0.07); interface width `<= 3.0` cells (expected 1.5 to 2).
* A3 Zalesak slotted disk, 100 x 100 over `[0,100]^2`, disk centre (50, 75), `R = 15`, slot width 5 and slot bottom at y = 60, rigid
  rotation about (50, 50) through one full revolution (628 steps of `dt = 1`, `omega = 2 pi / 628`), flux from a stream function
  evaluated at the nodes (discretely divergence free to round-off). Limits: Z1; L1 vs the initial shape `<= 0.20` (expected 0.06
  to 0.15); the slot is not filled completely (alpha at the three cell centres on the slot axis between y = 62 and 70 stays `<= 0.5`)
  (expected: partly filled corners and some slot erosion). Expected failure mode: corner rounding and slot fill from the algebraic
  compression.
* A4 Single-vortex deformation (LeVeque 1996), 128 x 128 over the unit square, disk `R = 0.15` at (0.5, 0.75),
  `psi = (1/pi) sin^2(pi x) sin^2(pi y) cos(pi t / 2)`, reversed at `t = 1` and run to `t = 2` (the disk returns). Limits: Z1; L1
  vs the initial shape `<= 0.25` (expected 0.08 to 0.20; the filament is under-resolved at 128 cells). Attempted if time remains
  after A1-A3 and B-D; otherwise reported as not attempted.

**B. Hydrostatic equilibrium (solver level, `tests/vof_static.rs`).** Box 0.5 x 1.0, 10 x 20 cells, `g = (0, -9.81, 0)`,
`rho_1 = 1000` (phase 1 below, water), `rho_2 = 1` (air), `mu_1 = 1e-3`, `mu_2 = 1.8e-5`, `sigma = 0`, no-slip walls, `dt = 1e-3`,
500 steps, PISO (`n_correctors = 3`), pressure tolerances of D4.
* B1 interface on a cell face (phase 1 for `y <= 0.5`): `max |U| <= 1e-10` at every step; the pressure profile `p` (cell
  centres, differences to cell 0) equals the layered hydrostatic integral with relative error `<= 1e-10` of `rho_1 g H_1`
  (expected round-off to 1e-12); `alpha` unchanged (`max |alpha - alpha_0| <= 1e-12`).
* B2 interface inside a cell (phase 1 for `y <= 0.5 + 0.4 dy`, so one cell has alpha 0.4): `max |U| <= 1e-10` at every step (no
  pressure-profile claim). The same with `sigma = 0.07` (flat interface, `kappa = 0`): `max |U| <= 1e-10`.

**C. Static droplet, zero gravity (`tests/vof_droplet.rs`).** Unit square, 64 x 64 cells, phase-1 disk `R = 0.25` at (0.5, 0.5),
`rho_1 = rho_2 = 1`, `mu_1 = mu_2 = 0.02`, `sigma = 1` (Laplace number 1250), no-slip walls, `dt = 4e-4` (`dt_cap` =
5.5e-4 from D3, asserted in the test), `t = 1.0` (2500 steps, about 1/3 of the viscous time `R^2 rho / mu` = 3), `n_smooth = 2`.
* C1 Laplace jump: `dp = mean(p, alpha > 0.99) - mean(p, alpha < 0.01 and distance to the centre > R + 4 dx)` against
  `sigma / R = 4.0`: relative error `<= 0.10` (expected 0.01 to 0.05). The interface band (3 dx either side) is excluded.
* C2 Parasitic currents: `Ca = mu |U|_max / sigma <= 1e-2` over the whole run (expected 5e-4 to 5e-3; the literature range of
  the unsmoothed CSF at this Laplace number is 1e-3 to 1e-2; an unbalanced force would give Ca of order 1).
* C3 Drop volume: Z1 conservation (the discrete drop area is initialised by cell sub-sampling, so it is compared with the
  initial sum, not with `pi R^2`); centroid drift `<= 0.01 dx` over the run.

**D. Dam break (Martin and Moyce 1952 geometry; `tests/vof_dambreak.rs` and the same case through the case tree).** Column width
`a = 0.05715 m`, height `H = 2a` (`n^2 = 2`, the "H = 114 mm" column), tank `12 a` long and `4 a` high (120 x 40 cells, `dx =
a / 10`), column at the left wall, `g = 9.81`, water 1000 / 1e-3 and air 1 / 1.8e-5, `sigma = 0`, NO-SLIP walls (never `symmetry`:
the known `component_diagonal = "averaged"` defect), PISO `n_correctors = 3`, `dt = 1.5e-3`, run to `t* = t sqrt(g / H) = 3.6`.
Reference data (see `docs/reference-data.md`): the only quantitative Martin-Moyce numbers read (secondary source, a 2014 journal
paper's Table 1, page read, the averaging window of that table is "t* > 1" and its extraction method from the original is not
stated) are the non-dimensional average front velocities `v / sqrt(g H)` = 1.48 (H = 57 mm) and 1.69 (H = 114 mm); the primary
Z-T table was NOT obtained, and the normalisation of the original (`t sqrt(g/a)` versus `t sqrt(2 g / a)` in different papers) was
not resolved, so no pointwise front table is used. The analytic upper bound is Ritter's front velocity `2 sqrt(g H)`.
* D1 Average front velocity of the simulated front (rightmost bottom-row cell with alpha > 0.5, linearly interpolated) between
  `t* = 1` and `t* = 3` (front stays inside the tank): `v / sqrt(g H)` within 15 percent of 1.69 (that is 1.44 to 1.94) AND
  strictly below 2 (Ritter). Expected 1.5 to 1.9; a first-order upwind momentum scheme is dissipative, typically slows the front.
* D2 The front position is monotone in time after `t* = 0.5`, and `x_f(t*) - x_f(1)` is within 20 percent of `1.69 sqrt(gH)
  (t - t_1)` at `t* = 2` and 3 (expected 5 to 15 percent).
* D3 Z1 with `|sum alpha V - sum alpha_0 V| / sum alpha_0 V <= 1e-12`; boundedness 1e-9; the run completes without NaN and with
  `max Co <= 1.5` (the flow Courant number).
* Time-box: 120 x 40 cells, about 700 steps of three pressure solves each; expected well under 5 minutes in release.

**E. Rising bubble (Hysing et al. 2009, case 1; `tests/vof_bubble.rs`), attempted only if time remains after A-D, the case tree
and the documentation; otherwise reported "not attempted".** Domain 1 x 2, 40 x 80 cells, bubble `R = 0.25` at (0.5, 0.5) of
phase 2 in phase 1: `rho_1 = 1000`, `rho_2 = 100`, `mu_1 = 10`, `mu_2 = 1`, `g = 0.98`, `sigma = 24.5`, no-slip top and bottom,
free-slip side walls (otherwise no-slip, labelled), `t = 3`. Reference (UNVERIFIED unless read): maximum rise velocity 0.2417 at
`t` about 0.9317; centroid height at `t = 3` 1.0813 (finest-grid reference values of the published benchmark). Limits registered
for the coarse algebraic VOF: `|y_c(3) - 1.0813| <= 0.12` (about 20 percent of the 0.58 rise; expected 0.03 to 0.10),
`|v_max - 0.2417| / 0.2417 <= 0.25` (expected 0.05 to 0.2), Z1.

**F. Case-tree checks (`tests/vof_tree.rs`).** F1 located errors (paths as in D6) for every refusal, exactly one test per row;
F2 the hydrostatic B1 and the dam break D through the case tree reproduce the library result (`max |U| <= 1e-10`; front velocity
identical to the library run to 1e-9 relative); F3 `field("alpha")` and `field("p_rgh")` available, bounded; F4 a case without
`multiphase` builds and runs exactly as before (also covered by the baseline hashes).

**G. Regression (all of the existing suite).** `crates/rcfd-api/tests/baseline_hashes.rs` is bit-identical and not re-recorded; all
existing tests unchanged and passing; `pimple.rs` is not edited.

## Registered expected failure modes (honest list, before any run)

1. Density ratio 1000:1 with a segregated PISO and a sharp interface can produce large spurious air velocities at the interface
   (blow-up); mitigations are the consistent `rhoPhi` and a modest `Co`; if D blows up it is a known miss, not a tuning exercise.
2. The CSF curvature of a smoothed 64-cell-wide disk is accurate to a few percent, not to the discretisation order; C1 may land
   between 5 and 10 percent; C2 is the place most likely to exceed its bound (known parasitic-current behaviour of CSF).
3. A3 (Zalesak) is likely to leave rounded corners and erode the slot; the L1 limit 0.20 is the loose side of the literature range
   for algebraic schemes.
4. The first-order upwind momentum scheme slows the dam-break front and smears the water-air boundary layer; D1 may fall below
   1.44. No scheme tuning is allowed to rescue it; a higher-order momentum scheme is a new, separately registered run.
5. Boundedness 1e-9 in coupled runs depends on the pressure tolerance; a loose tolerance violates it by `dt |div phi|`.
6. B2 requires the face-consistent reconstruction; a Green-Gauss `grad p_rgh` would fail it (that is the reason for D4).
7. The Hysing bubble on 40 x 80 cells with algebraic VOF is far from the finest-grid references; E limits are loose on purpose.
8. Any test over about 5 minutes is reported with its time.

## Named gaps (why F-055 stays `partial`)

Geometric (PLIC) reconstruction and the explicit/implicit and sharp/dispersed taxonomy of the roadmap scope (only the explicit
algebraic compressive scheme is delivered); wall and jump adhesion (contact angle) and the continuum-surface-stress form of the
tension (only CSF); Marangoni stress; the transposed viscous stress at a viscosity jump; BDF2 and higher-order momentum
convection tuned for two-phase flow; adaptive time stepping tied to the capillary and alpha Courant limits (fixed `dt`);
periodic and moving-mesh variants; open boundaries with total-pressure conditions are available through the generic pressure
conditions but are not validated here; energy/turbulence coupling; three-dimensional validation; the bubble benchmark unless E
is reached; wiring of F-226 / F-227 / F-228 to the new host (not done in this wave).
