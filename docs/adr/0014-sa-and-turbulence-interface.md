# ADR 0014: Spalart-Allmaras model (F-041) and the turbulence-model interface (F-044)

Status: accepted. The checks, decision rules and numeric limits below were written before any SA or interface code
was run. They are not changed, widened, replaced or deleted after a run; a miss is reported as measured and moved to an
ignored `*_known_miss` test. Roadmap: F-041, F-044 (F-157 only if a criterion below advances it).

## Sources (clean-room; published literature and NASA TMR only)

* Spalart and Allmaras, "A one-equation turbulence model for aerodynamic flows", La Recherche Aerospatiale 1 (1994) 5-21
  (AIAA 92-0439): original form. Not re-fetched.
* NASA TMR, "The Spalart-Allmaras Turbulence Model", https://tmbwg.github.io/turbmodels/spalart.html. Fetched by web
  fetch for this ADR (2026-10-15). The page gives, for SA-noft2 (c_t3 = 0, no f_t2 term): transport equation
  `D nuT/Dt = cb1 S~ nuT - cw1 fw (nuT/d)^2 + 1/sigma [div((nu + nuT) grad nuT) + cb2 |grad nuT|^2]`,
  `nut = nuT fv1`, `chi = nuT/nu`, `fv1 = chi^3/(chi^3 + cv1^3)`, `fv2 = 1 - chi/(1 + chi fv1)`,
  `S~ = Omega + nuT fv2/(kappa^2 d^2)`, `Omega = sqrt(2 W_ij W_ij)`, `fw = g [(1 + cw3^6)/(g^6 + cw3^6)]^(1/6)`,
  `g = r + cw2 (r^6 - r)`, `r = min(nuT/(S~ kappa^2 d^2), 10)`; sigma 2/3, cb1 0.1355, cb2 0.622, kappa 0.41,
  `cw1 = cb1/kappa^2 + (1 + cb2)/sigma`, cw2 0.3, cw3 2, cv1 7.1; wall nuT = 0, far field 3 to 5 nu. S~ limiters:
  (a) positive, (b) `S~ >= 0.3 Omega`, (c) Allmaras, Johnson and Spalart (2012): with `Sb = nuT fv2/(kappa^2 d^2)`,
  `S~ = Omega + Sb` if `Sb >= -c2 Omega`, else `Omega + Omega (c2^2 Omega + c3 Sb)/((c3 - 2 c2) Omega - Sb)`,
  c2 = 0.7, c3 = 0.9. SA-neg (negative nuT): production `cb1 (1 - ct3) Omega nuT`, destruction with sign "+"
  `+ cw1 (nuT/d)^2`, diffusion `(nu + nuT fn)`, `fn = (cn1 + chi^3)/(cn1 - chi^3)`, cn1 = 16, nut = 0 for nuT < 0.
* NASA TMR flat plate case, https://tmbwg.github.io/turbmodels/flatplate_sa.html (fetched): CFL3D/FUN3D, 5 grids,
  finest 545 x 385, M = 0.2, far-field nuT >= 3 nu. The page prints no absolute Cf (plots and uncertainty tables only).
* NASA TMR data file `FlatPlate/SA/cf_cfl3d.dat` (TMBWG/turbmodels, commit 20a39f549dbff988a6accc421ac84dafc4487695,
  fetched, SHA-256 2298419a2405e7048d794b4da51e3bcb23e545ddb20f315139024065919e5c6c, 447 rows): CFL3D SA Cf(x), rows
  (0.96005368, 2.70961295e-3) and (0.97008407, 2.70562153e-3). Linear interpolation at x = 0.97 gives 2.70565e-3, the
  reference value `CF_TMR_SA = 2.70565e-3`. The file does not state its grid; the page says the finest is 545 x 385, so
  "finest grid" is an inference and is marked unverified in docs/reference-data.md.
* TMR theory curves `FlatPlate_validation/cf_as_function_of_x.dat` (pinned file of validation-data/flat-plate-tmr, SHA-256
  97d01033...a8e2; Re_L = 5e6, L = 2; "theory 1, White 6-112a", "theory 2, White 6-121", "theory 3, White 6-134", from
  White, Viscous Fluid Flow, 1974): values at x = 0.97: 2.72379e-3, 2.88302e-3, 2.87459e-3 (read off the file).
* Power law `Cf = 0.0576 Re_x^-0.2` and White `Cf = 0.455/ln^2(0.06 Re_x)` as in docs/reference-data.md (verified there).
* Channel DNS for the switching test: MKM chan395 as in docs/reference-data.md.

## Implementation decisions fixed before running

* SA-noft2, kinematic incompressible form, `rcfd-turb/src/sa.rs`, steady segregated sweep like KOmegaSst. Production
  and the cb2 term explicit (positive), destruction implicit (`cw1 fw nuT/d^2` on the diagonal), diffusion
  `(nu + nuT_f)/sigma` with the boundary-face nuT taken from the boundary value. First-order upwind convection by
  default (the TMR codes use first-order for the turbulence advection too).
* S~ limiter default (c) Allmaras-Johnson-Spalart; options `min_0p3` and `positive`. SA-neg on by default
  (`negative_nutilde = true`), switchable off (then nuT is clipped at zero).
* Wall modes: `resolved` (nuT = 0 on the wall face, wall-face nu_t = 0, no wall function; needs y+ of order 1) and
  `auto` (default: Spalding law of the wall from `wall.rs` for the wall shear, the strain Omega of the wall cell from the
  Spalding gradient, and for the wall-cell y+ > 5 the wall-cell nuT is fixed to the value whose `nut = nuT fv1` equals the
  Spalding eddy viscosity `nu (dy+/du+ - 1)`; wall-face nu_t from the wall law as in the SST wall treatment).
* Inlet and initial nuT = `turbulence.nutilde_ratio` * nu (default 3, the TMR lower value), or `nutilde_value` per boundary.
* Interface (F-044): `rcfd_turb::model::TurbulenceModel` trait (update, nu_face, nut, wall_distance, scalar fields,
  y_plus, coefficient list/get/set with ranges and docs, initialisation from an eddy viscosity), adapters for k-epsilon
  and SST, SA implementing it directly, a process-wide registry (`register_model`, `create_model`, `model_keys`) so
  an external crate can add a model, selected with `turbulence.model = "plugin"` and `turbulence.plugin = "<key>"`.
  Coefficients live in `turbulence.coefficients.<model>.<name>` (k-epsilon: 0 means the variant default). The k-epsilon
  and SST numerics are not changed by this ADR; `kepsilon.rs`, `sst.rs`, `wall.rs` are owned by another agent and are
  not edited. Because the SST constants are `const` items inside `sst.rs`, SST coefficients are VISIBLE in the tree
  (with their TMR values) but a non-default value is REJECTED with a located error; SST coefficient override is
  NOT delivered by this ADR (it needs a change to `sst.rs`).
* Runtime switching: `Simulation::reconfigure_turbulence` rebuilds only the turbulence state; velocity, pressure, flux,
  iteration counter and monitors are kept. The new model starts with the old eddy viscosity (`initialize_from_nut`:
  nuT by inverting `nut = nuT fv1`, `epsilon = Cmu k^2/nut`, `omega = k/nut`, k from the settings). The service applies
  it for `settings.set` on `turbulence.*` paths when the session holds a simulation and is not Running.
* Not delivered (stated up front): per-coefficient override by expression or user function (F-044 merged-review scope);
  SST coefficient override (above); compressible SA (turbulence is incompressible-only in rcfd); SA with energy.

## Checks and numeric limits

All flat-plate runs use the TMR set-up of `rcfd-turb/tests/flat_plate_sst.rs`: Re = 5e6/m (U = 1, nu = 2e-7), plate
0..2, height 1, symmetry top, fixed-pressure outlet, plate turbulent from x = 0 (no upstream symmetry segment, unlike
the TMR grid: disclosed deviation), nuT_inf = 3 nu, 140 x 110 cells, `Grading::WallLayers` target y+ 0.5 (defined at x = 2),
growth 1.1, 70 layers, SIMPLEC, linearUpwind momentum, converged when max velocity update < 1e-9 and nu_t change < 1e-6
(the same criterion as the SST test; a run that does not reach it is reported as not converged and decides nothing).

Flat plate (heavy, `#[ignore]`, release), Cf from the wall-face viscosity and first-cell velocity as in the SST test:

* S1 (F-041 acceptance, TMR reference): `|Cf(0.97)/2.70565e-3 - 1| < 0.05`, default wall mode `auto`, y+ 0.5 mesh.
* S2 (F-041 acceptance, correlation): `|Cf/(0.0576 Re_x^-0.2) - 1| < 0.05` at x = 0.4, 0.6, 1.0 (Re_x = 2e6, 3e6, 5e6).
* S3 (F-041 acceptance, correlation): `|Cf/(0.455/ln^2(0.06 Re_x)) - 1| < 0.05` at x = 0.4, 0.6, 0.97, 1.0.
* S4 (TMR theory curves of the pinned file): at x = 0.97 within 5 percent of each of 2.72379e-3, 2.88302e-3, 2.87459e-3.
* S5 (wall-function-compatible use): mesh with target y+ 30 (layers 40, growth 1.1, ny 60, nx 140), `auto`:
  `|Cf(0.97)/2.70565e-3 - 1| < 0.10`. The run is valid only if the first-cell y+ at x = 0.97 is in [20, 60]; otherwise it
  is reported as not applicable (neither pass nor miss) and the mesh is not tuned afterwards to get a pass.
* S6 (resolved mode): y+ 0.5 mesh, `wall_mode = resolved`: `|Cf(0.97)/2.70565e-3 - 1| < 0.05`.
* Reported, not asserted: Cf and first-cell y+ along the plate, grid sensitivity if run, comparison with the SST result.
* Acceptance reading for F-041: "within 5 percent of correlation" counts as met only if S1, S2, S3 all pass.
  If S2 or S3 (or S4) miss, the status is `partial` and the miss is recorded in an ignored `*_known_miss` test, exactly
  as for F-043 (White). "Done" additionally requires S5 and S6 to pass (title scope: wall-resolved and wall-function use).
* TMR verification values of SA beyond the Cf file (e.g. grid-converged nut profile values): not available as numbers on an
  accessible page; marked unverified and not used.

Unit checks (default tests, limits fixed here):

* U1: `cw1 = cb1/kappa^2 + (1 + cb2)/sigma` equals 3.2390678 within 1e-6; `fv1(cv1) = 0.5` within 1e-14; `fv2(0) = 1`;
  `fw(r = 1) = 1` within 1e-12; `r` capped at 10 (S~ -> 0 gives r = 10).
* U2: the Allmaras limiter is continuous at `Sb = -0.7 Omega` within 1e-12 and `S~ > 0` for `Sb/Omega` in [-1e6, 0];
  S~ -> 0.1 Omega as `Sb -> -inf` within 1e-6 relative.
* U3: SA-neg: `fn(chi = 0) = 1`, `nut = 0` for nuT <= 0, `nuT` inversion `nuT(nut)` round-trips `nut` within 1e-10 relative
  over `nut/nu` in [1e-3, 1e4].

Coefficient override (default tests):

* K1 (SA): a short flat-plate run (coarse mesh, fixed 300 iterations) with `turbulence.coefficients.sa.cb1` raised from
  0.1355 to 0.1626 (+20 percent) through the settings tree changes the reported coefficient to 0.1626 exactly and
  changes the maximum nut by more than 1 percent relative to the default run; setting 0.1355 explicitly reproduces the
  default run bit-for-bit (max |difference| = 0).
* K2 (k-epsilon): `turbulence.coefficients.k_epsilon.c2 = 2.112` (+10 percent of 1.92) on the channel case after 100
  iterations changes max nut by more than 0.1 percent; an explicit 1.92 reproduces the default bit-for-bit.
* K3 (SST): `turbulence.coefficients.k_omega_sst.a1` to a non-default value is rejected with a diagnostic whose path is
  exactly `turbulence.coefficients.k_omega_sst.a1`; the default value is accepted. (Documented limitation, not an override.)
* K4 (visibility and located errors): `settings.get turbulence.coefficients` lists, for k_epsilon 5 (c_mu, c1, c2, sigma_k,
  sigma_e), k_omega_sst 11 (beta_star, a1, sigma_k1, sigma_w1, beta1, gamma1, sigma_k2, sigma_w2, beta2, gamma2,
  production_limiter), spalart_allmaras 10 (sigma, cb1, cb2, kappa, cw2, cw3, cv1, cn1, c2, c3) coefficients, each with
  a doc string and range in the exported JSON schema. A value outside its range (e.g. `sa.cb1 = -1`) is rejected by `set`
  with the path in the error; an SA coefficient set to a non-default value while `turbulence.model != spalart_allmaras`
  gives a diagnostic at that path; coefficients that do not apply (`k_epsilon.c1` with variant `realizable`) likewise.

Runtime switching (default tests, channel Re_tau 395 case of cases/channel/kepsilon_channel.toml, 4 x 14 cells, y+ 28):

* R1: at each switch through the service (`settings.set turbulence.model` on a Ready/Paused/Stopped session), the velocity
  field, the flux, the iteration counter and the session id are bit-identical before and after, the simulation object is
  not rebuilt (no restart), and the next `step` runs with the new model (status `turbulence.model` reports it).
* R2: sequence k_epsilon (6000 iterations) -> k_omega_sst (6000) -> spalart_allmaras (6000) -> k_epsilon (6000). After
  each phase the bulk velocity differs from a fresh run of that model started from the case's initial state with 6000
  iterations by less than 1 percent. (Steady state is expected to be independent of the starting field; a miss goes to
  an ignored known-miss test and is reported as such.)
* R3: invalid switches are rejected without changing the running simulation (e.g. `turbulence.model = "plugin"` with an
  unregistered `turbulence.plugin` gives a located diagnostic at `turbulence.plugin`; the old model keeps running).
* R4 (plugin): a model registered by a test through `register_model` (constant eddy viscosity with one coefficient) is
  selectable with `turbulence.model = "plugin"`, `turbulence.plugin = "<key>"`, its coefficient is overridden through
  `turbulence.plugin_coefficients.<name>.value`, and a laminar-channel Poiseuille-type check holds: with the constant nut
  of 9 nu the bulk velocity of a body-force driven channel equals the laminar value with viscosity 10 nu within 1e-3
  relative (converged, 14 x 4 cells).

Regression and hygiene:

* H1: `crates/rcfd-api/tests/baseline_hashes.rs` unchanged and bit-identical (kepsilon_channel covered); `tests/channel_sst.rs`,
  `sst_schema.rs` and `physics_schema.rs` pass unchanged.
* H2: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` pass
  (heavy tests `#[ignore]`).
* H3: SA documented in docs/case-schema.md and the exported JSON schema; unverifiable constants marked in
  docs/reference-data.md.

## Results

(filled in after the runs, without changing anything above)
