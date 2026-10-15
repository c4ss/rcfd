# ADR 0016: Wave 13 numerics: scheme additions (F-211), poor-mesh options (F-208), wall motion and shear conditions (F-204)

Status: accepted. Everything under "Decisions" and "Registered checks" was written before any of the new tests was
compiled or run. The limits are not changed, widened, replaced or deleted after a run. A miss is reported as measured and
the assertion moves to an ignored `*_known_miss` test with the registered limit intact; the cause is diagnosed, not tuned
away. A test set-up defect gets a NEW corrected test and the old one stays as a record. Addenda (file names, mesh sizes
that were not fixed here) may be appended at the end without touching registered limits.

Roadmap: F-211, F-208, F-204 (and the closing of the wall-options hook of ADR 0015, check C5, item 0 below).
Clean-room: published literature only; nothing from vendor material, vendor installs or vendor manuals. No vendor
names in the repository. Where a formula is quoted from memory the reference is marked "(from memory, unverified)"; closed
forms that are derived below carry their derivation and need no reference.

## Item 0: wall-options hook (ADR 0015 C5), context

`TurbulenceModel::apply_wall_options(&mut self, &WallOptions)` (defaulted) was added; `WallOptions`, `apply_kepsilon`,
`apply_sst` moved from `rcfd-api` to `rcfd-turb::model` (the trait lives in `rcfd-turb`, which cannot depend on `rcfd-api`;
`rcfd_api::wall_opts` re-exports them and reads the options with the extension trait `WallOptionsFromCase`).
`TurbState::new` calls the hook once after `create_model` for the built-in models; it does not call it for
`turbulence.model = "plugin"` (plugins own their wall treatment and `wall_opts::check` already rejects wall options on them;
calling it would overwrite what the plugin factory chose, which is what the equality test relies on). Registered check
(from ADR 0015 C5, unchanged): the run through the tree equals the library run to 1e-12 after 60 iterations for each of
`wall_function = scalable | enhanced`, `roughness_height` (with and without explicit `roughness_constant`),
`wall_omega = low_re`, `sst_variant = bsl`, SST roughness. The "library run" is the same case with the model supplied by a
registered plugin factory that builds the library model and sets the option fields directly. Tests
`c5_tree_run_equals_library_run`, `c5b_enhanced_and_explicit_roughness_constant` in `rcfd-api/tests/wall_options.rs`.
Results are in "Results" at the end. The channel wall cell has k_s+ about 0.4 for k_s = 1e-3, so the wall is hydraulically
smooth and the option has no effect; k_s = 2e-2 (k_s+ about 8) is used so that the check is not vacuous; every compared option
must change the tree result relative to the default run (asserted).

## Decisions

### D1 Scheme additions (F-211), `rcfd-fv/src/convection.rs`, `ddt.rs`

All new spatial schemes keep the existing structure: implicit upwind matrix plus deferred correction `F (phi_HO - phi_UD)`.

* `ConvScheme::Blended(beta)`: `phi_f = (1 - beta) phi_UD + beta phi_CD` with the existing linear-weight central value, written as
  `phi_UD + beta (phi_CD - phi_UD)`. `beta = 0` is upwind, `beta = 1` is central. No gradient needed.
* High-resolution bounded blend: the Gamma scheme of Jasak, Weller and Gosman (1999, Int. J. Numer. Meth. Fluids 31:431,
  from memory, unverified): on the normalised variable `phi~_C = (phi_C - phi_U)/(phi_D - phi_U)` the face value is
  `phi_UD + gamma (phi_CD - phi_UD)` with `gamma = min(1, phi~_C / beta_m)` for `0 < phi~_C < 1` and `gamma = 0` otherwise
  (upwind). The existing TVD code computes `r = (phi_C - phi_U)/(phi_D - phi_C) = 2 g.d/(phi_D - phi_C) - 1`
  (Darwish and Moukalled, r-formulation), so `phi~_C = r/(1+r)` and the scheme is a limiter `psi(r) = gamma` in the existing
  form `phi_UD + psi lambda (phi_D - phi_C)`. `beta_m` in `[0.1, 0.5]`, default 0.2 (range from memory, unverified). The
  scheme satisfies the convection boundedness criterion (Gaskell and Lau 1988, from memory, unverified); it is not a Sweby TVD
  limiter for `beta_m < 0.5` (gamma exceeds 2r for small r), which is why the boundedness check below is a measurement.
* Compressive: the Roe-Sweby limiter family `psi_k(r) = max(0, min(k r, 1), min(r, k))` (Roe 1985, Sweby 1984, from
  memory, unverified). `k = 1` is minmod, `k = 2` is superbee, `k in (1,2)` is more compressive than minmod. Extension
  `k in [0, 1)` is project-defined: `psi_0 = 0` is upwind for `r > 0`, and `psi_k <= psi_1` (less compressive than minmod).
  `k in [0, 2]`, default 1.5. TVD for the whole range (psi <= min(2r, 2)).
* Bounded central: central differencing where the normalised variable is inside the bounded region, upwind elsewhere. In NVD
  terms (`phi~_f = 1.5 phi~_C` for `phi~_C < 1/2`, `(1 + phi~_C)/2` for `1/2 <= phi~_C <= 1`, `phi~_C` otherwise) this is
  `psi(r) = clamp(r, 0, 1)`, which on the NVD is identical to Harten's minmod; it is therefore implemented as its own named
  scheme with the clamp limiter and a test records the equivalence to `Tvd(Minmod)` (bitwise). Whether this is the intended
  "bounded central differencing" of the literature is not claimed (Jasak 1996 thesis, from memory, unverified).
* Periodic (coupled) faces: the existing periodic branch of `convection()` adds the central correction for every
  non-upwind scheme. That is unbounded for limited schemes. Existing schemes are NOT changed (baseline hashes). The new
  schemes use at periodic faces: `Blended(beta)`: `beta` times the central correction (so beta 0 is upwind there and beta 1
  is identical to `Central`); Gamma, compressive and bounded central: upwind only (no correction), i.e. first order on the
  seam. The boundedness checks run on non-periodic domains.
* Time: `TimeScheme::Bdf2Bounded`, project-defined (no verified literature source): per cell, a blend
  `theta BDF2 + (1 - theta) Euler` of the two implicit time derivatives with
  `theta = clamp(rho, 0, 1)` where `rho = s_n / s_{n-1}` is the ratio of the last two backward difference quotients,
  `s_n = (phi^n - phi^{n-1})/dt_n`, `s_{n-1} = (phi^{n-1} - phi^{n-2})/dt_{n-1}`. If `|s_{n-1}|` and `|s_n|` are both below
  `1e-12 (|phi^n| + |phi^{n-1}| + |phi^{n-2}|)` then `theta = 1`; if only `|s_{n-1}|` is below that threshold then
  `theta = 0`. On the first step (no history) Euler, on the second step (no `phi^{n-2}`) BDF2 with `theta = 1`. For smooth
  data `rho = 1 + O(dt)`, so `1 - theta = O(dt)` and the O(dt^2) accuracy of BDF2 is kept; at a temporal extremum or
  an accelerating front `rho <= 0` or decreasing and the scheme falls back toward the monotone Euler. `History` gets two
  optional fields (`oldest`, `dt_older`).
* Settings tree: `solver.convection` gains `blended`, `high_resolution`, `compressive`, `bounded_central`; new
  `solver.blend_factor` (0 to 1, default 0.75), `solver.high_resolution_beta` (0.1 to 0.5, default 0.2),
  `solver.compressive_k` (0 to 2, default 1.5); `solver.time_scheme` gains `bdf2_bounded`. Other users of
  `conv_scheme(name)` (turbulence transport, etc.) get the defaults of the parameters.

### D2 Poor-mesh options (F-208), `rcfd-fv/src/gradient.rs`, `diffusion.rs`, `scalar.rs`

Scope decision, stated up front: the item lists five options. This wave implements and checks three at the library level
for the scalar convection-diffusion sweep (`sweep`, used by energy, passive scalars and turbulence transport):

1. Face-gradient correction ("corrected" compact form, Jasak 1996 / Mathur and Murthy 1997, from memory, unverified): the
   interpolated cell gradient `g_f` in the non-orthogonal term is replaced by
   `g_f + [ (phi_N - phi_P)/|d| - g_f . e ] e`, `e = d/|d|`. This is also the provided treatment of warped faces: warped faces
   are handled by the existing triangle-fan face centre and area vector, and the correction removes the along-`d` part of the
   gradient interpolation error that grows with warping. No separate "warped-face gradient correction" is implemented.
2. Secondary-gradient limiting: the Barth-Jespersen or Venkatakrishnan limiter (existing `gradient::limiter`) applied to the
   gradient used in the explicit non-orthogonal term of the diffusion operator.
3. Boundary-gradient treatment on Dirichlet faces: `Owner` (existing: owner-cell gradient), `Corrected` (the same face-gradient
   correction with the boundary value), `Dropped` (no non-orthogonal term on boundary faces; for comparison).
4. Solution limits: case-level `limits.temperature.{enabled,min,max}` and `limits.scalar.{enabled,min,max}` applied to the
   energy temperature and to the passive scalars after each solve; the number of clipped cells is reported in the status. Off
   by default (baseline hashes unchanged).

NOT attempted, stated now: pressure-interpolation variants (needs an incompressible manufactured-solution harness with a
per-cell momentum source and changes in the SIMPLE kernel; not started), a separate warped-face gradient correction, and
case-tree wiring of the three gradient options (library `GradientOptions` in `SweepInputs`; not claimed through the tree). The
status of F-208 is therefore `partial` whatever the numbers.

### D3 Wall motion and shear conditions (F-204), `rcfd-fv/src/bc.rs`, `rcfd-api/src/sim.rs`

Boundary kinds of `boundaries.<patch>.velocity`: `moving_wall`, `shear`, `partial_slip`, `marangoni`, all impermeable
(wall-normal velocity zero) and all `turb::Role::Wall`. (A plain `fixed` condition with a non-zero value stays an inlet for
the turbulence role and for the inlet speed, because that is existing behaviour. Translation of a wall was already possible
with `fixed`; it is re-checked here, not claimed as new.)

* `moving_wall`: `velocity_value` (translation, must be tangential to every face: located error otherwise) plus an optional
  rotation `wall_rotation_rate` [rad/s] about `wall_rotation_axis` through `wall_rotation_origin`, wall velocity
  `u_w = u_t + Omega x (x_f - x_0)` per face. Two walls with opposite rates are counter-rotating.
* `shear`: specified tangential traction `shear_stress` [Pa, vector, projected onto the face tangent plane] exerted on the
  fluid by the boundary. With outward normal `n`, `mu d u_t/d n = tau`, i.e. in kinematic form `d u_t/d n = tau/(rho nu)`
  (`physics.rho`, `physics.nu`). Face value `u_f = u_Pt + d_n g_t` (normal component zero), implemented per component
  with the same lagged cross coupling as `Symmetry`.
* `partial_slip`: Navier slip with slip length `slip_length` `l` [m]: `u_f,t - u_w,t = l d u_t/d n_in`, discretely
  `u_f,t = (l u_Pt + d_n u_w,t)/(d_n + l)`; `l = 0` is no-slip, `l -> infinity` is `symmetry`. A specularity form is accepted
  as an alternative input: Maxwell's slip law `u_s - u_w = ((2 - sigma)/sigma) lambda du/dn` with the tangential momentum
  accommodation coefficient `sigma = 1 - s` (`s` = specularity in `[0, 1)`) and the mean free path `lambda`
  (`mean_free_path`), giving `l = lambda (1 + s)/(1 - s)` (Maxwell 1879; from memory, unverified). `s >= 1` is a located
  error (use `symmetry`). It is not built on `Mixed` with a fraction (that would be mesh dependent).
* `marangoni`: thermocapillary stress `tau = (d sigma/d T) grad_s T` with `marangoni_coefficient` = `d sigma/d T`
  [N/(m K)] and the tangential temperature gradient `grad_s T = (I - n n) grad T` taken from the least-squares gradient of the
  temperature field in the boundary-adjacent cell (exact for a linear field with exact boundary values), evaluated each
  iteration / step, then applied as `shear`. Needs the energy equation.
* The report `moment`/`force` already integrates the wall stress from the boundary-condition gradient
  (`wall_force_and_moment`); it is used for the torque checks, unchanged.

## Registered checks

All "observed orders" are `log2(e_h / e_{h/2})` of the L2 error (volume-weighted RMS over cells against the exact value at the
cell centre) on the FINEST TWO grids of the stated sequence. "Converged" for deferred-correction runs means: sweep until the
largest change of the solution in one sweep is below 1e-13 (relative to the solution range) or 600 sweeps; a run that stops
on the sweep cap is reported as not converged and the check that uses it is reported as NOT MET (not passed).

### F-211 (`rcfd-fv/tests/schemes_wave13.rs`, default tests)

Steady order test O1. Domain `[0,1] x [0,0.1]`, `box_2d(n, 2, ...)`, 2D one cell thick, `u = (1,0,0)`, `Gamma = 0`,
exact solution `phi(x) = exp(x)`, source `S = phi'(x)` entered as the exact cell average `(e^{x_r} - e^{x_l})/h`,
Dirichlet `phi = exp(x)` at `xmin` and `xmax` (face values from the exact function), zero-gradient on `ymin`/`ymax` (zero
flux). The profile is monotone, so no limiter clips at an extremum; limited schemes keep the design order there (reason: for
`r` slightly below 1 the limiters evaluate to their second-order branch). Grids `n = 64, 128, 256`. Limits on the observed
order of BOTH pairs (64->128, 128->256):
* O1a second-order set `Central`, `Blended(1)`, `Gamma(beta_m = 0.2)`, `Compressive(k = 1.5)`, `Compressive(k = 2)`,
  `BoundedCentral`: order in `[1.85, 2.3]`.
* O1b first-order set `Upwind`, `Blended(0.5)`, `Compressive(k = 0)`, `Compressive(k = 0.5)`: order in `[0.9, 1.15]`.
* O1c all runs converged.

Blend factor B1: on the O1 problem at `n = 64`, converged: `max|phi_{Blended(0)} - phi_{Upwind}| <= 1e-12` and
`max|phi_{Blended(1)} - phi_{Central}| <= 1e-10`. Tree-level B2 (`rcfd-api/tests/schemes_wave13.rs`): the cavity case (Re 100,
SIMPLEC, 60 iterations) with `solver.convection = "blended"`, `blend_factor = 0` equals `"upwind"` and `blend_factor = 1`
equals `"central"` in `ux`, `uy`, `p`: `<= 1e-10`. B3: `BoundedCentral` equals `Tvd(Minmod)` bitwise on the O1 problem.

Boundedness W1 (square wave). Non-periodic `[0,2] x [0,0.1]`, `n = 200`, `u = (1,0,0)`, `Gamma = 0`, initial
`phi = 1` for `0.1 <= x <= 0.35` and 0 elsewhere (cell-centre sampling), inlet `phi = 0` (Dirichlet), outlet zero gradient,
implicit Euler, `dt = 0.5 h` (CFL 0.5), 80 steps (front travels 0.4), 20 deferred-correction sweeps per step. Limits, for
`Gamma(0.2)`, `Compressive(1.5)`, `Compressive(2)`, `BoundedCentral`: `max(phi) - 1 <= 1e-3` and `0 - min(phi) <= 1e-3`.
Positive control (the test can fail): `Central` must violate the bounds by more than `0.02` in `max(phi) - 1` or
`-min(phi)`. Sharpness: the number of cells with `0.1 < phi < 0.9` at the trailing and leading edge together is
`<= 0.6` times the `Upwind` number for `Gamma(0.2)` and `Compressive(2)`.

Time T1 (temporal order). `phi' = -lambda phi`, `lambda = 1`, `phi(0) = 1`, error at `t = 1` against `exp(-1)`, steps
`dt = 0.2, 0.1, 0.05, 0.025` (`add_ddt` with a diagonal spatial matrix on a single-cell-like mesh; no spatial error). Observed
order of the finest two pairs: Euler `[0.9, 1.15]`; BDF2 `[1.8, 2.3]`; `Bdf2Bounded` `[1.8, 2.3]`.
Time T2 (bounded time scheme, square wave). Same domain/wave as W1, spatial scheme `Upwind` (one linear solve per step),
`n = 100`, `dt = 2 h` (CFL 2), steps to `t = 0.8`. Define `over = max(max(phi) - 1, -min(phi), 0)`. If
`over(BDF2) >= 0.02` (the test has teeth) then require `over(Bdf2Bounded) <= 0.25 over(BDF2)` and
`over(Bdf2Bounded) <= 0.02`; if `over(BDF2) < 0.02` the check is reported as vacuous, not as passed.
Tree T3: `solver.time_scheme = "bdf2_bounded"` accepted and runs a transient cavity step sequence (finite, differs from
`bdf2` by more than 0).

Acceptance statement of the roadmap item is met only if O1, B1, B2, W1, T1 pass; T2 and T3 are reported.

### F-208 (`rcfd-fv/tests/mms_poor_meshes.rs`, default tests where affordable)

Manufactured solution in physical coordinates, steady convection-diffusion `div(F phi) - div(Gamma grad phi) = S`,
`u = (1, 0.7, 0.4)` (constant, divergence free), `Gamma = 0.1`, `phi = 1 + sin(1.3x + 0.4) cos(1.1y) cos(0.9z + 0.2)`, `S` from
the analytic derivatives evaluated at the cell centre and multiplied by the volume, Dirichlet on all boundaries from the
exact value at the face centre, `ConvScheme::Central`. Meshes are the images of `box_mesh([n,n,n], [1,1,1], ...)` under
the maps below, `n = 8, 16, 32`:
* G (good): identity.
* G2 (mild): `x' = x + 0.02 sin(2 pi y)`, `y' = y + 0.02 sin(2 pi x)` (in plane). Precondition at `n = 16`: max
  non-orthogonality `<= 12` degrees.
* K (skewed): `x' = x + 0.7 y + 0.3 z`, `y' = y + 0.3 z`. Precondition at `n = 16`: max non-orthogonality `>= 25` degrees.
* W (warped, non-planar faces): `x' = x + 0.1 sin(2 pi y) sin(2 pi z)`, `y' = y + 0.1 sin(2 pi x) sin(2 pi z)`,
  `z' = z + 0.1 sin(2 pi x) sin(2 pi y)`. Precondition at `n = 16`: max face non-planarity
  `1 - |S_f| / sum_tri |S_tri| >= 1e-4` over the triangle fan of the face, and max non-orthogonality `>= 20` degrees.
* S (stretched): `Grading::Expansion(50)` in y (last/first cell 50), uniform x and z. Precondition at `n = 16`: max cell aspect
  ratio of `check_mesh` `>= 10`.
Preconditions are asserted values (printed); a precondition failure is a set-up defect.

Option sets (`GradientOptions`): C0 = existing behaviour (least-squares gradient, interpolated face gradient, `Owner`
boundary); C1 = corrected face gradient + `Corrected` boundary; C2 = C1 + Venkatakrishnan limiter with `K = 5` on the
secondary gradient. Solutions converged by the rule above (non-orthogonal terms are deferred).
* M1: observed L2 order (n = 16 -> 32) of C0 and C1 on each of G, G2, K, W, S: `>= 1.7`. (Design order is 2; the global value
  is below 2 where one-sided boundary-cell gradients enter, see the gradient test in `gradient.rs` which documents about 1.5
  for the gradient itself, which is why the limit is 1.7 for the solution and not 2.)
* M2: C2 on each mesh: `>= 1.5` (limiting reduces the local order at extrema; this weaker limit is registered for the limited
  option only).
* M3 (no degradation on good meshes): at `n = 32` on G and on G2: `err(C1)/err(C0) <= 1.10` and `err(C2)/err(C0) <= 1.25`.
* M4 (reported, no limit): `err(C1)/err(C0)` on K, W, S at `n = 32` and the `Dropped` boundary variant on K.
* L1 (solution limits, `rcfd-api/tests/limits_wave13.rs`): a heated-cavity case with `limits.temperature.enabled = true` and a
  band narrower than the natural range ends with every cell inside the band to 1e-14 and a positive clipped count; the same case
  without limits leaves the band (positive control).

Pressure-interpolation variants, separate warped-face correction, tree wiring of the gradient options: not attempted (D2).

### F-204 (`rcfd-api/tests/wall_motion.rs`, `rcfd-fv/src/bc.rs` unit tests)

Laminar steady SIMPLEC, `box_2d(8, 20, 1, 1, periodic_x = true)`, `h = 1`, `nu = 0.1`, `rho = 1` unless stated; linear profiles
are reproduced exactly by the central discretisation, so these limits are tight and are weak tests on their own (stated).
* D1 plane Couette: `ymin` `moving_wall` `u = 0`, `ymax` `moving_wall` `u = (2,0,0)`: `max|u - 2 y|/2 <= 1e-8`; counter-translating
  `ymin` `u = -1`, `ymax` `u = +1`: `max|u - (2y - 1)| <= 1e-8`. Cell-centre values against the exact line.
* D2 specified shear: `ymin` no-slip, `ymax` `shear` `tau = (0.5, 0, 0)` Pa: `u = (tau/(rho nu)) y = 5 y`,
  `max|u - 5 y|/5 <= 1e-8`.
* D3 partial slip: `ymin` `partial_slip` `l = 0.3`, `ymax` `moving_wall` `u = 1`: `u(y) = (y + l)/(h + l)` (derivation: `u = a + b y`,
  `u(0) = l u'(0)` gives `a = l b`, `u(h) = 1` gives `b = 1/(h+l)`), `max|u - u_exact| <= 1e-8`; `l = 0` reproduces the
  no-slip run to `1e-12`; specularity `s = 0.5`, `lambda = 0.1` (-> `l = 0.3`) reproduces the `l = 0.3` run to `1e-12`; `s = 1`
  is a located error.
* D4 thermocapillary layer: `box_2d(160, 24, 20, 1)`, `ymin` no-slip with `T = gamma x`, `ymax` `marangoni` with `T = gamma x`
  (expression), side walls no-slip with `T = gamma x`, `gamma = 1`, `d sigma/dT = -0.1`, `rho = 1`, `nu = 1`, conductivity so
  that the thermal diffusivity is 10 (Pe about 3e-3, Re about 2e-2; `T` stays linear). Derivation: parallel flow, no-slip bottom,
  shear `tau` at the top, zero net flux, `u = A xi^2 + B xi` (`xi = y/h`), `A/3 + B/2 = 0`, `mu u'(h) = tau`: `u = (tau h/(4 mu))
  (3 xi^2 - 2 xi)`, `tau = (d sigma/dT) gamma = -0.1`, so `u(xi) = -0.025 (3 xi^2 - 2 xi)` at `rho = mu = 1`.
  Sampling station: the cell column nearest `x = 10` (mid-length, aspect ratio 20). Limit: `max_y |u - u_exact| <= 0.03 * 0.025`
  (the largest magnitude of the exact profile is 0.025, at the surface).
  Library check D4b: with a frozen linear `T` the `Marangoni` face stress equals the `shear` condition with `tau = (d sigma/dT) gamma` to `1e-12`.
* D5 Taylor-Couette torque: 2D annulus `cylinder_ogrid(n_theta = 128, n_r, r_in = 1, r_out = 2, Uniform, depth = 1)`, `nu = 0.1`,
  `rho = 1`, inner patch `cylinder` rotating at `Omega_1 = 1`, outer patch `farfield` at `Omega_2 = -0.5` (counter-rotating), steady
  laminar. Derivation: `u_theta = A r + B/r`, `A = (Omega_2 R_2^2 - Omega_1 R_1^2)/(R_2^2 - R_1^2) = -1`,
  `B = (Omega_1 - Omega_2) R_1^2 R_2^2/(R_2^2 - R_1^2) = 2`; the viscous torque on the inner cylinder per unit depth has magnitude
  `4 pi mu B = 4 pi * 0.1 * 2 = 2.5133` (`mu r^2 d(u/r)/dr = -2 mu B` at `r = R_1`, times area `2 pi R_1`, times arm `R_1`; equals
  `4 pi mu (Omega_1 - Omega_2) R_1^2 R_2^2/(R_2^2 - R_1^2)`). Grids `n_r = 32, 64, 128`; the `moment` report (z component about the
  origin) on `cylinder` against the exact value (nominal radii; the polygon with 128 chords has radius factor `cos(pi/128) = 0.9997`,
  well below the limit). Limit: `|M/M_exact - 1| <= 0.01` on the finest grid (`n_r = 128`). Trend printed. Expected first-order wall
  gradient error of about `1.5 dn` relative (one-sided difference over half a cell), so the finest grid is chosen for it; if it misses
  it is a `known_miss` with that diagnosis checked against the trend.
  D5b rigid rotation: both patches at `Omega = 1` (inner `u = r`, outer `u = r`): the torque on the inner cylinder is `<= 1e-3` of the D5
  exact torque (validates the transpose term of the wall stress); the velocity field matches `u_theta = r` to `1e-6`.
* D6 turbulence role: with `turbulence.model = "k_epsilon"`, the four new kinds give `Role::Wall` (unit test in `turb.rs`).

MRF-frame Taylor-Couette (ADR 0008) is not attempted.

## Hygiene

* H1 `baseline_hashes` bit-identical (default settings); existing tests unchanged and passing.
* H2 `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --release --no-fail-fast` on a
  `git archive` copy of the final commit, `CARGO_TARGET_DIR=<target dir>`.
* H3 every tuning and deviation disclosed in "Results"; unverifiable references are marked in `docs/reference-data.md`.
* H4 roadmap statuses stay `partial` unless the whole acceptance text of the item is met.

## Results

(To be filled in after the runs.)
