# ADR 0026: Wave 18A: algebraic subgrid-scale models (F-045) and algebraic RANS closures (F-206)

Status: accepted. Everything under "Decisions" and "Registered checks" was written before any of the new code was compiled
or run (the only numbers computed beforehand are closed forms and predictions from a throw-away Python script outside the
repository; they are marked "closed form" or "prediction"). The limits are not changed, widened, replaced or deleted after a
run. A miss is reported as measured and the assertion moves to an ignored `*_known_miss` test with the registered limit
intact; the cause is diagnosed, not tuned away. A test set-up defect gets a NEW corrected test and the old one stays as a
record. Addenda (file names, mesh sizes that were not fixed here) may be appended at the end without touching registered
limits.

Roadmap: F-045 (Smagorinsky, WALE), F-206 (algebraic zero-equation / mixing-length). F-207 and the rest of F-045/F-206 are
listed under "Not attempted".
Clean-room: published literature only; nothing from vendor material, vendor installs or vendor manuals. No vendor
names in the repository. Formulas and constants quoted from memory are marked "(from memory, unverified)"; closed forms that
are derived below carry their derivation and need no reference.

## Sources (nothing fetched for this ADR; all marked from memory)

* Smagorinsky, Mon. Weather Rev. 91 (1963) 99: `nu_sgs = (Cs Delta)^2 |S|`, `|S| = sqrt(2 S_ij S_ij)` (from memory,
  unverified). `Cs` about 0.17 for isotropic turbulence follows from Lilly's argument with a Kolmogorov constant of about 1.5,
  `Cs = (1/pi) (3 C_K / 2)^(-3/4)` = 0.173 for `C_K` = 1.5 (Lilly 1967; Pope, Turbulent Flows, 2000, sec. 13.4; from memory,
  unverified). Default `cs = 0.17`.
* Nicoud and Ducros, Flow Turb. Combust. 62 (1999) 183 (WALE): `nu_sgs = (Cw Delta)^2 (Sd_ij Sd_ij)^(3/2) /
  [(S_ij S_ij)^(5/2) + (Sd_ij Sd_ij)^(5/4)]`, `Sd_ij = (g2_ij + g2_ji)/2 - delta_ij g2_kk/3`, `g2_ij = g_ik g_kj`,
  `g_ij = d u_i / d x_j` (from memory, unverified). The constant is quoted as 0.325 for `Delta = V^(1/3)` in the common
  implementations and 0.5 in the original (from memory, unverified, the two are not reconciled here). Default `cw = 0.325`.
* Deardorff 1970: filter width `Delta = V^(1/3)` (from memory, unverified).
* Van Driest, J. Aero. Sci. 23 (1956) 1007: damping `1 - exp(-y+/A+)`, `A+` about 25 to 26 (from memory, unverified). Moin and
  Kim (1982) use it on the length of Smagorinsky in channel LES (from memory, unverified). Defaults `a_plus` = 25 for the
  subgrid model and 26 for the mixing length.
* Prandtl mixing length `nu_t = l^2 |dU/dy|`, `l = kappa d` near a wall, upper limit `0.09 delta` (Pope 2000, sec. 7.1; from
  memory, unverified). Nikuradse's pipe mixing length `l/R = 0.14 - 0.08 (1 - y/R)^2 - 0.06 (1 - y/R)^4` (Schlichting,
  Boundary-Layer Theory, from memory, unverified); the measured Nikuradse data themselves are NOT in the repository and not
  used.
* Channel DNS reference: MKM chan395 samples as already verified in `docs/reference-data.md` ("Channel DNS"): `y+ : U+` =
  30: 13.493, 50: 14.856, 100: 16.534, 150: 17.646, 300: 19.745, centreline `U_c+` = 20.133, bulk `U_b+` = 17.545 (relative to
  the true `u_tau`, `Re_tau` = 392.24). The test uses the nominal `Re_tau` = 395 (nu = 1/395) with `u_tau` = 1; the 0.7
  percent `Re_tau` mismatch is far below the limits.
* Taylor-Green vortex as in `docs/reference-data.md` ("Taylor-Green vortex", verified): `u = sin x cos y`, `v = -cos x sin y`.

## Scope decisions stated up front (before looking at any result)

* F-045 delivers two eddy-viscosity subgrid models, **Smagorinsky and WALE**, as registered turbulence models usable in the
  **transient** algorithms (piso, pimple) of the existing engine. They are evaluated explicitly each time step from the
  velocity at the start of the step (no relaxation, no pseudo time).
* **DES-class hybrids are NOT attempted.** `sa.rs` solves a steady segregated pseudo-time sweep (no time-derivative term, no
  history); an SA-based DES with a modified length scale `min(d, C_DES Delta)` would be a steady-state model, not a
  time-accurate one, so it does not "fit cleanly". Recorded as a named gap, not a partial claim.
* **Not attempted** (all remain open): decaying isotropic turbulence (no 3D periodic box generator in the settings tree, no
  verified spectrum or decay reference in `validation-data`, so no slope is claimed or reported), channel LES, dynamic
  Smagorinsky, Vreman, sigma, wall-modelled LES, SAS, delayed DES variants, stress-blended hybrid, embedded LES zones,
  inflow turbulence generators (F-207 entirely), a filter-width option other than `V^(1/3)`, a subgrid model in the
  compressible or variable-density engines, subgrid heat/scalar flux (Pr_t, Sc_t).
* F-206 delivers the algebraic **mixing-length** closure (linear `kappa d` with upper cap, optional Nikuradse profile, optional
  van Driest damping) as a registered model usable steady (simple, simplec) and transient. The acceptance statement
  "mixing-length pipe profile agrees with the Nikuradse data" is **not met**: there is no full-pipe generator
  (`cylinder_ogrid` is an annulus) and the Nikuradse measurements are not in the repository, so the registered physical check
  is the plane-channel profile at `Re_tau` = 395 against the MKM DNS. Not attempted: further zero-equation forms (e.g.
  Baldwin-Lomax, indoor-air zero-equation), v2-f, explicit algebraic Reynolds-stress variants, generalised two-equation
  model, curvature correction, production limiters, turbulent Prandtl/Schmidt options, per-coefficient expression override.
* Status of both roadmap items stays `partial` whatever the numbers.

## Decisions

### D1 Model formulas, `rcfd-turb/src/les.rs` (new)

* Velocity gradient `g_ij = d u_i/d x_j` per cell by the existing Green-Gauss routine (`eddy::strain_rate` conventions, boundary
  values from the velocity boundary conditions). `S_ij = (g_ij + g_ji)/2`, `|S| = sqrt(2 S_ij S_ij)`.
* Filter width `Delta_c = V_c^(1/3)` from the cell volume. For `box_2d` the cell depth is `lx/nx`, so `Delta` equals the in-plane
  spacing only on square cells (`lx/nx = ly/ny`); the Taylor-Green meshes below are square.
* `smagorinsky`: `nu_sgs = (Cs Delta f)^2 |S|`, `f = 1` (no damping) or `f = 1 - exp(-y+/A+)` (`wall_damping = van_driest`).
* `wale`: formula above, with the guard `nu_sgs = 0` when the denominator is exactly 0 (uniform flow is 0/0). No wall damping
  option (the operator scales as `y^3` by construction; `wall_damping` other than `none` is a located error for WALE).
* Wall faces (patches whose turbulence role is `wall`): the eddy viscosity on a wall face is 0, i.e. the effective face
  viscosity is `nu` (wall-resolved LES; a wall model is not provided). Other boundary faces take the owner-cell value as the
  existing `effective_viscosity`.
* `y+` for van Driest and for the `y_plus` report: every cell is assigned the nearest wall face by distance to the face
  centre (brute force, once at construction); `u_tau^2 = nu |u_t,c - u_t,w| / d_n` at that face (`d_n` the cell-centre normal
  distance, `u_t` the tangential part of the velocity relative to the boundary value), `y+ = d u_tau / nu` with `d` the exact
  wall distance of `wall.rs`. This is an estimate from the first cell and is only meaningful when that cell is in the
  viscous layer (y+ below about 5); it is lagged (computed from the velocity passed to `update`).
* The model is explicit: `update(u, phi)` returns the largest relative change of `nu_sgs`, with no relaxation, whatever
  `turbulence.relax_nut` is. `initialize_from_nut` stores the given viscosity until the next `update`.

### D2 Mixing length, `rcfd-turb/src/zeroeq.rs` (new)

* `mixing_length`: `nu_t = l^2 |S|` (same `|S|`), `l = min(l_prof f, l_max)`, with `f` as above (`A+` = 26),
  `profile = linear`: `l_prof = kappa d`, `l_max = l_max_ratio R`; `profile = nikuradse`: `l_prof = R (0.14 - 0.08 (1 - xi)^2 -
  0.06 (1 - xi)^4)`, `xi = min(d/R, 1)` and no further cap (`kappa` and `l_max_ratio` unused). `R` = `turbulence.length_scale`
  (half height of a channel, radius of a pipe), required > 0. Wall faces take `nu_t = 0` (wall-resolved only; the log-law
  wall-function variant `MixingLength::nut_with_walls` stays a library helper). Steady use relaxes `nu_t` with
  `turbulence.relax_nut` (Picard, as in the existing channel test); transient use is explicit (no relaxation).

### D3 Interface and wiring (additive; the interface is not redesigned)

* Registry keys `smagorinsky`, `wale`, `mixing_length`; `turbulence.model` gains the same three enum values (the registry key
  is the model name, `model_key` already passes it through). `ModelContext` gains `wall_damping`, `length_scale`,
  `mixing_profile`, `time_accurate` (all with neutral defaults for the other models). New tree keys:
  `turbulence.wall_damping` (`none` default | `van_driest`; smagorinsky and mixing_length), `turbulence.length_scale` [m]
  (default 0; mixing_length), `turbulence.mixing_profile` (`linear` | `nikuradse`).
* Coefficients (`turbulence.coefficients.<group>.<name>`, shown with docs and ranges in the JSON schema, plug-in interface
  `coefficients()` / `set_coefficient()`): `smagorinsky`: `cs` 0.17 [0, 0.5], `a_plus` 25 [1, 100]; `wale`: `cw` 0.325 [0, 1];
  `mixing_length`: `kappa` 0.41 [0.2, 0.6], `l_max_ratio` 0.09 [0, 1], `a_plus` 26 [1, 100]. Defaults are not zero-sentinels. A
  non-default value in the group of a model that is not selected is a located validation error (as for SA).
* `turb::check`: new classes `sgs` (smagorinsky, wale) and `algebraic` (mixing_length) so that nothing falls into the
  k-epsilon branch by elimination. Located errors (path in brackets): an SGS model needs a transient algorithm
  [`turbulence.model`]; `mixing_length` accepts simple, simplec, piso, pimple; the existing RANS models and plug-ins keep the
  steady-only rule; no wall patch is required (a periodic box is valid) except when `wall_damping = van_driest` or for
  `mixing_length` [`turbulence.wall_damping`, `turbulence.model`]; `turbulence.k_initial/epsilon_initial/omega_initial`,
  `variant`, `wall_treatment`, `sa.*` are rejected for the new models as for SA; inlet `turbulence_spec` is not used;
  `wall_damping` on WALE; `length_scale <= 0` or `mixing_profile` set for another model; energy equation together with the new
  models is rejected as for RANS (no turbulent heat flux).
* `sim.rs` transient branch: `t.change = t.model.update(&s.u, &s.phi); s.nu_face = Some(t.model.nu_face())` before the step
  (no-op without a turbulence model, so the baseline hashes are untouched). Live switching goes through the existing
  `reconfigure_turbulence`; the switch from an SGS model to a RANS model in a transient session is rejected by `check`.

## Registered checks

All limits are the ones written here. "Rel." means relative to the exact value. Tests: `rcfd-turb/tests/sgs_closed_forms.rs`
(S1), `rcfd-turb/tests/sgs_mesh.rs` (S2, S3), `rcfd-turb/tests/sgs_taylor_green.rs` (TG), `rcfd-turb/tests/mixing_length.rs`
(ML1, ML2), `rcfd-api/tests/sgs_tree.rs` (T1 to T5), `rcfd-api/tests/mixing_length_tree.rs` (ML3 to ML6).
Parameters `Cs` = 0.17, `Cw` = 0.325, `Delta` = 0.1 in S1 (so `(Cs Delta)^2` = 2.89e-4, `(Cw Delta)^2` = 1.05625e-3).

### S1 closed forms of the pointwise functions (rel. 1e-12 unless stated)

Closed forms below were derived by hand and the digits computed once with an independent script.
* S1a pure shear `g_12 = 2` (all else 0): `S_12 = S_21 = 1`, `|S| = sqrt(2 * 2) = 2`; Smagorinsky `= 2.89e-4 * 2 = 5.78e-4`.
  `g2 = 0` exactly, so WALE `= 0` **exactly** (`== 0.0`).
* S1b pure strain `g = diag(e, -e, 0)`, `e = 1.5`: `S:S = 2 e^2`, Smagorinsky `= (Cs Delta)^2 2 e = 8.670e-4`. `g2 = diag(e^2, e^2, 0)`,
  `Sd = diag(e^2/3, e^2/3, -2e^2/3)`, `Sd:Sd = (2/3) e^4`, WALE `= (Cw Delta)^2 e (2/3)^(3/2)/(2^(5/2) + (2/3)^(5/4)) =
  1.377838789072e-4`.
* S1c uniform flow `g = 0`: both exactly 0 and finite.
* S1d rigid rotation about z with rate `w = 3` (`g_12 = -w`, `g_21 = w`): `S = 0` exactly, Smagorinsky exactly `0.0`. WALE is **not**
  zero: `g2 = diag(-w^2, -w^2, 0)`, `Sd = diag(-w^2/3, -w^2/3, 2w^2/3)`, `Sd:Sd = (2/3) w^4`, `S:S = 0`, so
  WALE `= (Cw Delta)^2 (2/3)^(1/4) w = 2.863288848939e-3`. (This is a property of the operator as written; it is registered as the
  derived value, not claimed as desirable.)
* S1e a general tensor `g = [[0.4, -0.7, 0.2], [1.1, -0.3, 0.5], [-0.6, 0.8, -0.1]]` (not divergence free; the pointwise
  functions do not need that): Smagorinsky `= 4.596826405250e-4`, WALE `= 2.189365993742e-4`.
* S1f near-wall scaling. Field `u = (a y + b y z, 0, c y)` with `a = b = c = 1`, divergence free, evaluated at `z = 0`:
  `g_12 = a + b z`, `g_13 = b y`, `g_32 = c`, all other entries 0, so `g2_12 = b c y` is the only non-zero entry of `g2`,
  traceless, `Sd:Sd = b^2 c^2 y^2/2`, `S:S = ((a+bz)^2 + b^2 y^2 + c^2)/2`. Leading order for `y -> 0`: WALE `= (Cw Delta)^2
  (b c y)^3 / (2^(3/2) SS0^(5/2))`, `SS0 = ((a+bz)^2 + c^2)/2 = 1`. Limits: WALE / (leading form) `- 1` in `[-1e-5, 1e-5]` at `y = 1e-3`
  (neglected relative terms are about `1.25 y^2` and `y^(5/2)`); least-squares slope of `ln nu` against `ln y` over
  `y` in {1e-4, 1e-3, 1e-2} within `3 +- 1e-3`; Smagorinsky slope within `0 +- 1e-3` (finite at the wall: the known
  deficiency of the plain Smagorinsky model). Van Driest factor: `nu` with damping equals `nu_plain * (1 - exp(-y+/A+))^2`
  to 1e-12 and its slope in `y+` over {1e-3, 1e-2} (undamped part constant) is `2 +- 1e-3` (exponent 2 at the wall).
* S1g filter width: `V^(1/3)` of the cells of `box_mesh([4,3,2],[2,1.5,1])` (cells 0.5 x 0.5 x 0.5) is 0.5, and of
  `box_mesh([8,2,1],[2,1,1])` (cells 0.25 x 0.5 x 1, non-cubic) is also 0.5; both to 1e-14 rel.

### S2 mesh level, linear velocity field (`sgs_mesh.rs`)

`box_mesh([6,5,4],[1,1,1], [Uniform, Symmetric(3), Uniform])`, `u = G x` with `G` of S1e, boundary values on all six patches set to
`G x_f` per face. Green-Gauss is exact for a linear field with exact face values.
* S2a model `nu` per cell against the S1 closed forms of `G` with `Delta_c = V_c^(1/3)` (volumes from the mesh): max rel. error
  `<= 1e-12` for Smagorinsky and WALE (damping off, no wall patches declared).
* S2b effective face viscosity: internal faces `nu + w nu_P + (1-w) nu_N`, boundary faces `nu + nu_owner`, to `1e-12`; with all
  six patches declared walls the boundary faces are exactly `nu`.
* S2c `u` uniform: both models `<= 1e-14` absolute everywhere; `u` rigid rotation (`g_12 = -3`, `g_21 = 3`, boundary values
  exact): Smagorinsky `<= 1e-12` absolute (Green-Gauss round-off), WALE within `1e-10` rel. of S1d's closed form scaled by the
  cell `Delta`.

### S3 van Driest wiring (`sgs_mesh.rs`)

`box_2d(8, 16, 1, 1, periodic_x)`, `nu = 0.01`, `u = (gamma y, 0, 0)`, `gamma = 4`, `ymin` wall at rest, `ymax` wall moving with
`(gamma, 0, 0)`; boundary values exact. Then `u_tau^2 = nu gamma` at both walls (relative velocity), `u_tau = 0.2`,
`y+ = 20 d`, `d = min(y, 1 - y)`. Expected `nu_sgs = (Cs Delta_c f)^2 gamma` with `f = 1 - exp(-y+/25)`; rel. error `<= 1e-12` for
all cells (Smagorinsky with damping). Mixing length same field in ML2.

### TG Taylor-Green vortex with the subgrid models (`sgs_taylor_green.rs`; library level, `PimpleSolver` with the model updated each step)

`box_2d(n, n, 2 pi, 2 pi, true, true)` (square cells), `u = (sin x cos y, -cos x sin y)`, `p = (cos 2x + cos 2y)/4`, `nu = 0.05`, `piso`,
central convection, `ImplicitEuler`, `dt = 0.01` (the library runs below use fixed `dt`). Mean kinetic energy per volume
`K = (1/2V) sum |u|^2 V_c`, `K(0) = 1/4`. Runs: L laminar, S Smagorinsky (0.17), W WALE (0.325), Z Smagorinsky with `cs = 0`.
Analytic facts (closed form): for this field `S_11 = -S_22 = cos x cos y`, `S_12 = 0`, `|S| = 2|cos x cos y|`,
`S:S = 2 cos^2 x cos^2 y`. For WALE `g2 = (a^2 - b^2) diag(1,1,0)` with `a = cos x cos y`, `b = sin x sin y`, so with
`q = a^2 - b^2`: `Sd:Sd = (2/3) q^2` and `S:S = 2 a^2`.
* TG0 (t = 0, `n = 32`, `Delta = 2 pi/32`): the cell mean of Smagorinsky `nu_sgs` against `(Cs Delta)^2 * 2 * (2/pi)^2 = 9.032e-4` within
  5 percent; the cell maximum within `[0.9, 1.0] * 2 (Cs Delta)^2`. WALE: cell mean against `(Cw Delta)^2 * 0.248403` (2D quadrature of the
  closed-form pointwise expression, 4000^2 points) within 8 percent. All `nu_sgs >= 0`.
* TG1 (`n = 16`, initial extra dissipation): the closed-form SGS dissipation `2 nu_sgs S:S` has the mean `8 (Cs Delta)^2 (4/(3 pi))^2 =
  1.44101 (Cs Delta)^2` at `t = 0`. With amplitude decay `e^{-2 nu t}` the mean over `[0, tau]` of the `A^3` term gives the factor
  `(1 - e^{-6 nu tau})/(6 nu tau)`. Registered: `|((K_L(tau) - K_S(tau))/tau) / (1.44101 (Cs Delta)^2 (1 - e^{-6 nu tau})/(6 nu tau)) - 1| <= 0.15`
  at `tau = 0.1` (10 steps). (Prediction: the measured value is about 8 percent low because the Green-Gauss gradient of a
  wave of 16 cells per period is low by `sin h / h`, 2.5 percent, entering as the third power.) The difference of the two runs is
  used so that the time-integration error of the laminar decay cancels.
* TG2 (sign and monotone decay, `n = 16`, `T = 4`, 400 steps): for S and W, `K` is non-increasing at every step (violations
  counted with tolerance `1e-14 K`: 0 allowed), `K > 0`, and `min nu_sgs >= 0` at every step; `K_S(t_k) < K_L(t_k)` and
  `K_W(t_k) < K_L(t_k)` for every step `k >= 1`.
* TG3 (effect size, so the check has teeth): `K_S(4)/K_L(4) <= 0.97` (prediction from the amplitude ODE `A' = -2 nu A - 2.882 (Cs Delta)^2
  A^2`, assuming the Taylor-Green shape persists: 0.920); `K_W(4)/K_L(4) <= 0.995` (prediction 0.982); `K_W(4) > K_S(4)` (WALE dissipates
  less than Smagorinsky for this flow; closed-form ratio of initial SGS dissipation 0.0702 (Cw)^2 against 1.441 Cs^2, i.e. 1:5.6).
* TG4 `cs = 0`: run Z equals run L, `max |u_Z - u_L| <= 1e-13` after 100 steps (the model adds exactly 0).
* TG-iso: decaying isotropic turbulence: not run (see scope). No slope is claimed.

### T1 to T5 settings tree (`rcfd-api/tests/sgs_tree.rs`)

Case `box_2d` periodic in x and y, `nx = ny = 16`, `lx = ly = 2 pi`, `nu = 0.05`, `solver.algorithm = "piso"`, `convection = "central"`,
`dt = 0.01`, `initial.velocity_expression_x = "sin(x)*cos(y)"`, `_y = "-cos(x)*sin(y)"` (syntax to be fixed by the expression
parser; recorded in an addendum if it differs).
* T1 `turbulence.model = "smagorinsky"` runs 20 steps, `simulation.turbulence.key` is `smagorinsky`; the `nut` field after the
  first step equals the library function applied to the initial field on the same mesh, max abs difference `<= 1e-12`; same for
  `wale`.
* T2 coefficients: `cs` x 1.2 scales the first-step `nut` field by 1.44 (max rel. deviation `<= 1e-9`); `cw` x 1.2 likewise;
  `turbulence.coefficients.smagorinsky.cs = 0` equals the laminar tree run, `max|u| difference <= 1e-13` after 20 steps; explicit
  defaults equal omitted keys bit-for-bit.
* T3 live switching through the service: `smagorinsky` -> `wale` after 10 steps keeps `u` bit-identical across the switch, the key
  changes, and the following step's `nut` differs from the Smagorinsky one by more than 1e-9 of its max.
* T4 located errors: `smagorinsky` with `simplec` [`turbulence.model`, message names the transient algorithms]; `cs = 2` [path
  `turbulence.coefficients.smagorinsky.cs`, message contains the range]; `cw` non-default with model `smagorinsky` [path of
  `cw`]; `wall_damping = van_driest` without a wall patch [`turbulence.wall_damping`]; `wall_damping = van_driest` with `wale`
  [`turbulence.wall_damping`]; `mixing_length` with `length_scale = 0` [`turbulence.length_scale`]; `k_epsilon` with `piso` still
  rejected [`turbulence.model`]; energy equation with `wale` [`turbulence.model`].
* T5 the exported JSON schema lists the three new `turbulence.model` values and the groups `smagorinsky`, `wale`, `mixing_length`
  under `turbulence.coefficients`, each coefficient with a non-empty doc and `min`/`max`; `rcfd_turb::model_keys()` contains the three keys.

### ML mixing length

* ML1 (closed forms, rel. 1e-12): Nikuradse `l/R` at `xi = 1`: 0.14; at `xi = 0.5`: 0.11625 (`0.14 - 0.08 * 0.25 - 0.06 * 0.0625`); series at the
  wall `l/R = 0.4 xi - 0.44 xi^2 + 0.24 xi^3 - 0.06 xi^4` exactly (the profile is a polynomial in `xi`); `|l/(0.4 xi R) - 1| <= 1.2e-4` at
  `xi = 1e-4`. Linear profile: `l(d) = min(0.41 d, 0.09 R)`: 0.041 at `d = 0.1, R = 1`, 0.09 at `d = 0.5`. Beyond `R`, Nikuradse `l = 0.14 R`.
* ML2 (mesh): the S3 Couette field with the `mixing_length` model: `nu_t = l^2 gamma`, `l` from ML1 (linear with and without van Driest, `A+` = 26;
  Nikuradse with `R = 0.5`) at the exact wall distance; rel. error `<= 1e-12`.
* ML3 (profile against the one-dimensional reduction of the same model, through the tree): periodic `box_2d` channel, `h = 1`
  (domain `y` in `[0, 2]`), `nu = 1/395`, body acceleration 1 (`u_tau = 1`), wall-resolved graded mesh with first cell `y+` about 0.5, `mixing_length`
  linear with `kappa` 0.41, `l_max_ratio` 0.09, `length_scale = 1`, `wall_damping = van_driest`, `simplec`, converged (`max_du <= 1e-10`
  or 8000 iterations; not converged counts as NOT MET). Reduction: total stress `1 - y/h = (nu + l^2 U') U'` for `y <= h` gives
  `U' = (-nu + sqrt(nu^2 + 4 l^2 (1 - y/h)))/(2 l^2)`, `U(y)` by Simpson quadrature with 20000 panels. Limit: `|U_FV - U_red|/U_red <= 0.02` at
  every cell centre with `U_red > 0.5`.
* ML4 (against DNS, same run): linear interpolation of the converged `U+(y+)` (`u_tau` = 1, so `y+ = 395 y`) at 30, 50, 100,
  150, 300 and the centreline cell, and the bulk velocity: each within 10 percent of the MKM values of the Sources list (limit
  of the F-206 statement for the other channel closures; same limit used here). Values at `y+` = 5, 10, 15, 20 are printed without a limit.
* ML5 (Nikuradse profile applied to the channel, `R = h`, van Driest on): same ML4 checks and limit. This is the pipe-formula analogy on a
  channel, not the registered pipe acceptance, and is reported that way.
* ML6 (tree equals library): the channel run through the tree equals a run where the same model is built directly and supplied
  through a registered plug-in factory (`turbulence.model = "plugin"`), `max |u| difference <= 1e-12` after 60 iterations, for the default
  options, `nikuradse`, `kappa = 0.38`, `l_max_ratio = 0.07`, `wall_damping = none`; each of these changes the result relative to the default
  run by more than 1e-6 (teeth).

## Hygiene

* H1 `baseline_hashes` bit-identical (default settings); existing tests unchanged and passing. The k-epsilon, SST and SA code is not touched.
* H2 `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --release --no-fail-fast` on a `git archive`
  copy of the final commit, `CARGO_TARGET_DIR=<target dir>`.
* H3 every deviation disclosed in "Results"; unverifiable references marked in `docs/reference-data.md`.
* H4 roadmap statuses stay `partial` (F-045: DES, dynamic models, decaying turbulence not delivered; F-206: pipe/Nikuradse data, further models not delivered).

## Results

Measured on the final code (release, `CARGO_TARGET_DIR=<target dir>`). Limits are those registered above; none was changed.

**S1 closed forms** (`rcfd-turb/tests/sgs_closed_forms.rs`): all seven tests met; every closed form reproduced to 1e-12 relative; near-wall WALE slope 2.999972 (limit 3 +- 1e-3), Smagorinsky slope 0.000005 (0 +- 1e-3), van Driest exponent within 1e-3 of 2.
**S2 mesh** (`sgs_mesh.rs`): linear field on the graded box, max relative error Smagorinsky 7.8e-16, WALE 3.1e-15 (limit 1e-12); face viscosity error 0; uniform field max 9.9e-19 / 3.0e-19 (limit 1e-14); rigid rotation Smagorinsky 1.0e-17 (limit 1e-12), WALE relative 6.0e-16 (limit 1e-10).
**S3 van Driest wiring** (Couette box): max relative error 6.5e-16 (limit 1e-12).
**TG** (`sgs_taylor_green.rs`): TG0 Smagorinsky mean nu_t 9.0022e-4 against 9.0313e-4 (-0.32 %, limit 5 %), max/top 0.9840 (limit 0.9 to 1.0); WALE mean 1.0045e-3 against 1.0115e-3 (-0.70 %, limit 8 %). TG2 met (no energy increase; min nu_t 2.88e-4 Smagorinsky, 1.45e-50 WALE). TG4 met (cs = 0 equals laminar, difference 0). TG3 `K_S(4)/K_L(4)` 0.95593 (limit 0.97) met, `K_W(4)/K_L(4)` 0.94632 (limit 0.995) met.
**Misses of the registered checks (two, both ignored `*_known_miss` tests with the registered assertion intact):** TG1 measured 3.74549e-3 against the closed form 6.32683e-3 (ratio 0.5920, limit 0.85 to 1.15); TG3 ordering `K_W > K_S` measured `K_W/K_S` = 0.98995. Cause: addendum A1 (viscous term of the solver in Laplacian form). Replacement checks of A1, limits fixed by the derivation before the re-run: TG1b Smagorinsky ratio 0.9472 (limit 1 +- 0.15) met; TG1c WALE measured 4.75828e-3 against 5.03325e-3, ratio 0.9454 (limit 1 +- 0.15) met; TG3b `K_W(4) < K_S(4)` met. The earlier prediction `K_S/K_L` = 0.920 assumed the full stress divergence; the measured 0.956 is consistent with the Laplacian form.
**T1 to T5** (`rcfd-api/tests/sgs_tree.rs`): all met. T1 tree nu_t against the library function: difference 0 for both models (limit 1e-12). T2 constant x 1.2 scales nu_t by 1.44 to 6.2e-16 (Smagorinsky) and 5.0e-16 (WALE) of the maximum (limit 1e-9); cs = 0 equals the laminar run exactly (limit 1e-13); explicit defaults equal omitted keys bit for bit. T3 live switch keeps u, v, p bit-identical; next-step nu_t differs by 1.6 of its maximum (limit > 1e-9); a switch to k_epsilon in the transient session is refused and leaves the run untouched. T4 located errors as registered, with one deviation: cs = 2 is refused by `Case::set` (range check, located message naming `turbulence.coefficients.smagorinsky.cs`) rather than by the semantic check, so the test asserts the located error of `set`. T5 schema lists the models, groups, docs and ranges.
**ML1, ML2** (`rcfd-turb/tests/mixing_length.rs`): met; max relative error about 2.9e-16.
**ML3** (channel, 80 rows in y with 30 wall layers of growth 1.1 and first cell 1/395, centre at y+ = 0.5; simplec; 7100 iterations, largest velocity change per iteration 9.3e-11 over a 50-iteration chunk): maximum relative difference to the 1D reduction 2.12e-3 over 39 rows (limit 0.02) met. The convergence measure is defined as the largest change of the cell velocity over 50 iterations divided by 50 (the ADR said "max_du"; this is the exact definition used).
**ML4** (linear profile with van Driest, against DNS at Re_tau 395): U+ at y+ = 30, 50, 100, 150, 300: 12.955 (-3.99 %), 14.420 (-2.94 %), 16.054 (-2.90 %), 17.197 (-2.55 %), 19.887 (+0.72 %); centreline 20.704 (+2.83 %); bulk 17.423 (-0.69 %); worst 3.99 % (limit 10 %) met. Printed only: y+ = 5, 10, 15, 20 give 4.837, 8.308, 10.294, 11.513 (DNS 4.82, 8.44, 10.657, 12.007).
**ML5** (Nikuradse profile with R = h and van Driest, a channel analogy of the pipe formula, reported as such): 13.324 (-1.25 %), 14.987 (+0.88 %), 17.000 (+2.82 %), 18.174 (+2.99 %), 20.220 (+2.41 %); centreline 20.761 (+3.12 %); bulk 17.979 (+2.47 %); worst 3.12 % (limit 10 %) met.
**ML6** (tree against a plug-in factory that builds `MixingLengthModel` from the library, 60 iterations): difference 0 for default, Nikuradse, kappa 0.38, l_max_ratio 0.07 and wall_damping none (limit 1e-12); the four changed variants differ from the default by 0.38, 0.24, 0.19 and 1.9 in velocity (limit > 1e-6).
**H1** `baseline_hashes` passes unchanged and is not re-recorded.

### Deviations and disclosures
* The centreline value of ML4/ML5 is the value of the cell row next to the centreline (centre within half a row height); no extrapolation.
* The channel mesh of ML3 to ML5 has first cell height 1/395 and the y+ = 0.5 of the registered text refers to its cell centre.
* Subgrid and mixing-length viscosity enter through the existing face-viscosity path (`div(nu_eff grad u)`); the transposed stress term is absent (named gap).
* In transient runs the algebraic models are evaluated explicitly from the velocity at the start of the step, not under-relaxed.
* The plug-in twin of ML6 builds the library model itself because `turbulence.length_scale`, `mixing_profile` and `wall_damping` are refused for `model = "plugin"`.

### Unverified references
Smagorinsky 0.17 and 0.1 to 0.2 range, WALE 0.325, van Driest 25 and 26, Nikuradse 0.14, 0.08, 0.06, and the Re_tau 395 DNS values (U+ at the listed y+, centreline 20.133, bulk 17.545) were written from memory of the published literature and could not be checked against the papers here; they are marked in `docs/reference-data.md`. No Nikuradse pipe data and no decaying-isotropic-turbulence spectrum or slope exist in the tree: no slope or pipe-data claim is made.

### Not delivered
DES-class hybrid (the Spalart-Allmaras model is a steady pseudo-time sweep with no time derivative); dynamic Smagorinsky, Vreman, sigma, WMLES, SAS, delayed DES, stress-blended models; decaying isotropic turbulence (no periodic 3D box, no verified reference); channel LES; the full subgrid-stress divergence (transposed term); SGS in the compressible and variable-density engines; SGS heat and scalar flux, Pr_t/Sc_t; F-207 entirely; the "pipe vs Nikuradse data" acceptance; Baldwin-Lomax style zero-equation models; v2-f, EARSM, curvature correction; per-coefficient expression override. F-045 and F-206 stay `partial`.

### Addendum A1 (written after the first run of the Taylor-Green tests; registered limits above are NOT changed)

First run of `sgs_taylor_green.rs`: TG0, TG2 and the Smagorinsky/WALE parts of TG3 met; TG1 missed (measured extra dissipation 0.592
of the registered closed form, limit 0.85 to 1.15) and the ordering `K_W(4) > K_S(4)` of TG3 missed (measured `K_W/K_S` = 0.98995).
Diagnosis (derivation, not tuning): the registered closed forms used the dissipation of the full stress divergence,
`2 nu_sgs S:S`. The momentum solver discretises the viscous term as `div(nu_eff grad u)` with a face viscosity (the same
form that the RANS models use); the transposed part `div(nu_sgs (grad u)^T)` is not present. For a variable `nu_sgs` this is a
different operator, whose energy dissipation is `nu_sgs g_ij g_ij` (`g_ij g_ij = 2 (a^2 + b^2)` for this vortex) instead of
`2 nu_sgs S:S` (`4 a^2`). Closed forms of the mean initial dissipation (2D quadrature, 4000^2 points): Smagorinsky
`nu_sgs g:g` gives `0.900633 (Cs Delta)^2` (exactly `4 (Cs Delta)^2 [(4/(3 pi))^2 + (2/(3 pi))^2]`, against 1.44101 for the
`2 nu S:S` form; ratio 0.625, and the measured/registered ratio 0.592 is that 0.625 times the Green-Gauss factor of 0.95 for the
16-cell wave); WALE `nu g:g` gives `0.313661 (Cw Delta)^2` (against 0.070217 for the `2 nu S:S` form: WALE is largest where
`S` is small and `q = a^2 - b^2` is not, which is exactly where the two forms differ most). So WALE dissipates more than
Smagorinsky in this solver (initial `5.109e-3` against `4.014e-3` at `n = 16`), the opposite of the registered ordering.
The omission of the transposed term is a property of the existing viscous discretisation (rcfd-fv `pimple.rs`, shared); it is
NOT changed here and is a named gap of F-045 ("subgrid-stress divergence in Laplacian form only").
The original assertions stay as ignored `*_known_miss` tests. New corrected checks, limits fixed here from the derivation:
* TG1b (replaces TG1 for the Laplacian-form solver): `|((K_L(tau) - K_S(tau))/tau) / (0.900633 (Cs Delta)^2 (1 - e^{-6 nu tau})/(6 nu tau)) - 1| <= 0.15`.
* TG1c: the same for WALE with `0.313661 (Cw Delta)^2`, limit `0.15`.
* TG3b (replaces the ordering of TG3): `K_W(4) < K_S(4)`.
