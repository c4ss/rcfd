# ADR 0204: F-291 registration: the gradient options on turbulence transport, momentum, the pressure equation and the electric potential

Status: **REGISTERED 2026-11-06**, written before any code of F-291 and before any run of its checks. Not yet accepted by the
maintainer. Results go in an addendum at the end.

Parent: [ADR 0190](0190-f256-registration-case-tree-wiring-of-the-gradient-options.md) Q4 and its decision record of 2026-11-06
(answer (b): the options extended to further equations under new checks, as a new item; the recommendation was (a)). Roadmap item
F-291 (depends on F-256). Closest precedents: ADR 0190 (F-256: the wiring and its checks G1 to G4), [ADR 0193](0193-f246-registration-pressure-interpolation-variants.md)
(F-246: an option of the steady kernel under the same kinds of checks) and [ADR 0202](0202-f289-registration-conjugate-heat-flow-report-with-the-gradient-options.md)
(F-289: a derived flux that follows the solved operator).

Read at main 3b89aac. Number: 0203 is the highest on main; the caller assigned 0204 to F-291 (branch `f291-gradient-options`).

House rules ([ADR 0130](0130-wave48a-overview-and-run-record-contract.md)): limits are fixed here before the code and the runs and
are not changed after a run; a miss is reported as measured and its assertion moves to an ignored `*_known_miss` test with the
limit intact; a defective set-up is corrected by an amendment written before the corrected run, the old test kept as an ignored
record; defaults stay bit-identical; no settings hash of an existing case changes; `crates/rcfd-api/tests/baseline_hashes.rs` and
`case_hashes_w48u.rs` are not edited (K1 makes an edit of the latter unnecessary); no new third-party crate; no commit trailers.
Clean-room: our own ADRs and first principles only.

## What F-291 asks

The roadmap entry: the gradient options of F-256 (face correction, secondary limiter, boundary-gradient treatment) also act on the
turbulence transport equations, momentum, the pressure equation and the electric potential, each under new checks of the kinds
F-256 used (ADR 0190 G1 to G4): invalid values give located errors, a tree run equals the library run with the same options to a
limit fixed by its registration ADR, the options change the result on a non-orthogonal mesh, and a case without the keys is
bit-identical (baseline hashes unchanged).

## What the code read shows (main 3b89aac, nothing run)

1. **Where the options act today.** `rcfd_fv::diffusion::laplacian_opt` (plain `laplacian` for `None`, `laplacian_with`
   otherwise) is called by the scalar sweep (`SweepExtras::gradient`, set by `EnergySolver::gradient` and
   `ScalarTransport::gradient`) and the P1 energy assembly (ADR 0190 K3); the conjugate `heat_flow` report follows the energy
   solver's options (ADR 0202). Every other assembly calls `laplacian` with or without a gradient.
2. **Momentum.** The steady kernel (`SimpleSolver::iterate`) assembles, per active component `k`,
   `laplacian(g, gamma_nu, bc, grad)` with `grad = least_squares_partner(u_old_k, ub_k, partners)` when
   `convection.needs_gradient() || non_orth_correctors > 0`, else `None`; beside it the test-only masked Laplacian of ADR 0175.
   The transient kernel (`PimpleSolver::step_with`, which the moving-mesh step also runs) assembles
   `laplacian(g, gamma_nu, bc, grad)` with `grad = least_squares(conv_cells, bv)` under the same condition (`conv_cells` the
   BDF2-extrapolated values on the first outer iteration of a BDF2 step, else the current values). The fractional step
   (`pimple/fractional.rs`) has its own assembly; the coupled solver calls `laplacian` without a gradient (no secondary flux at
   all); variable density, VOF and compressible are other solvers.
3. **Pressure.** Both kernels solve the pressure equation in `pressure::solve_pressure_inner`: with `non_orth_correctors > 0` every
   pass assembles `laplacian(gamma_eff, bc.coeffs, grad)` with `grad = least_squares(p, boundary_values_pbc(p))` of the pass's
   current pressure (or ADR 0193's V2-c gradient function); with 0 correctors there is no gradient and no secondary flux. The same
   face term `k . g_f` (`g_f` the interpolated cell gradient) appears in the flux correction (`corrected_flux`,
   `corrected_flux_with`: internal faces, gradient of the new pressure) and, for SIMPLEC with
   `simplec_non_orth_compensation`, in `simplec_non_orth_flux_correction` (`(rAtU - rAU)_f k . g_f`, gradient of the old
   pressure, internal faces). Fixed-velocity faces are Neumann for the pressure (`a = 1`): no boundary secondary flux there. On
   fixed-pressure faces (`a = 0`) the equation adds the owner-gradient secondary flux and the flux correction takes the orthogonal
   boundary gradient only, so with correctors that term is in the equation and not in the flux (Q3). The PISO/PIMPLE
   pressure-lag estimate (`lag_map`, ADR 0073) and its pre-run trial solve their own pressure equations with `solve_pressure`.
4. **Turbulence transport.** `KEpsilon::transport` (k, epsilon), `KOmegaSst::transport` (k, omega) and the nuTilda matrix of
   `SpalartAllmaras::update` assemble `laplacian(gamma, bc, grad)` with `grad = least_squares(field, boundary values)` when the
   model's `convection` (the case key `turbulence.convection`, default `upwind`) needs a gradient, else `None`. The Reynolds
   stress model calls `laplacian` without a gradient; the algebraic models (smagorinsky, wale, mixing_length) transport nothing;
   a plugin model is opaque behind `TurbulenceModel`. k-epsilon, SST and plain SA run with simple/simplec; SA with des/ddes with
   piso/pimple. The tree builds the model in `TurbState::new` (`rcfd_turb::create_model`), and `reconfigure_turbulence` rebuilds
   it through the same function.
5. **Electric potential.** `ElectricState::update` (`electric.enabled`, with prescribed flow and energy only, ADR 0035) calls
   `rcfd_fv::electric::solve` once per prescribed-flow iteration: up to 20 sweeps of `sweep_ext` with `ConvScheme::LinearUpwind`
   on a zero flux and `SweepExtras { fixed, ..Default }`, so the potential always has a cell gradient and a secondary flux,
   whatever `solver.convection` is. The Joule heat, the boundary current density and `boundary_power` use
   `conductive_face_flows(.., true)`, the default operator's flows. The Lorentz source with `potential = "solve"` (ADR 0039,
   `lorentz.rs`) solves its own potential through `solve_with_source`.
6. **Rules.** `check_gradient_options` (ADR 0190 R2 to R4) refuses non-default options when `solver.convection` computes no
   gradient (R2) or no energy or scalar equation takes them (R3). With further equations taking the options, R2 and R3 as written
   would refuse cases where the options act (the pressure with correctors under `upwind` convection, the potential under any
   convection).
7. **Hash and written text.** A new key of `[solver]` is written by `Case::to_toml` at its default, which `case_hashes_w48u.rs`
   H1 compares with R0 unless its `DEFAULT_KEYS` list is extended (ADR 0190 H2, ADR 0193 H2). An optional object
   (`Schema::optional`, ADR 0155) stays out of the data until a value inside it is set, so neither the written text nor the
   settings hash of an earlier case changes; ADR 0192 and ADR 0194 added such tables with `hash_omit_default`.

## Decisions

**K1. Selectors** (Q1). An optional table `[solver.gradient_equations]` (`Schema::object().optional().hash_omit_default()`) with
four booleans, each default `false` and `hash_omit_default`: `momentum`, `pressure`, `turbulence`, `electric_potential`. A `true`
selector makes that equation take the options of the four F-256 keys too (the one option set of the case); the energy and scalar
diffusion terms take them as before (F-256, unchanged). No F-256 key changes its meaning, and a case without the table, or with
the table at its defaults, runs the code it ran before.

**K2. Mapping.** `gradient_selected(case, name)` is ADR 0190's mapping (K2: `None` while the four F-256 keys hold their defaults)
when the selector `name` is true, and `None` when it is false. `None` leaves every assembly as it is today.

**K3. Library.**

* `rcfd-fv/src/diffusion.rs`: two public helpers that state, for one face term, the rule `laplacian_with` applies:
  `secondary_cell_gradient(geom, cells, bvals, grad, opts)` (the gradients scaled by the limiter of `opts.secondary_limiter`,
  computed as `laplacian_with` computes them; `None` without a limiter) and `secondary_face_gradient(geom, f, grad, cells,
  face_correction)` (the interpolated gradient of internal face `f`, its component along the cell-centre line replaced by
  `(phi_N - phi_P)/abs(d)` when `face_correction`). `laplacian_impl` is not changed.
* **Momentum.** `SimpleSolver::gradient_momentum` and `PimpleSettings::gradient_momentum` (public, `None` from `new` and from the
  settings constructors). The momentum Laplacian becomes `laplacian_opt(.., grad, cells, bvals, opts)` with the values the
  gradient was computed from (SIMPLE: `u_old_k`, `ub_k`; PIMPLE: `conv_cells`, `bv`). The test-only masked Laplacian is unchanged.
* **Pressure.** `SimpleSolver::gradient_pressure` and `PimpleSettings::gradient_pressure`. `solve_pressure_inner` takes the
  options: each pass assembles `laplacian_opt(geom, gamma_eff, bc.coeffs, grad, p, bv, opts)` with the pass's `p` and `bv =
  boundary_values_pbc(p)` (`bv` is formed only with options). New public entry points `pressure::solve_pressure_parts_opt` (with
  ADR 0193's optional gradient function and the options) and `pressure::corrected_flux_opt` (`corrected_flux_with` whose internal
  faces use the gradients of `secondary_cell_gradient` and `secondary_face_gradient` of the new pressure; boundary faces as
  before, Q3); the existing functions call them with `None` and keep their arithmetic. `SimpleSolver`: the pressure solve, the
  flux correction (also inside `velocity_correction = face_flux`) and `simplec_non_orth_flux_correction` (the same face rule with
  the old pressure) take `gradient_pressure`. `PimpleSolver`: each corrector's pressure solve and flux correction take it. The
  pressure-lag estimate is unchanged (Q4).
* **Electric potential.** `electric::solve_opt` and `electric::solve_with_source_opt` pass the options in
  `SweepExtras::gradient`; `solve` and `solve_with_source` call them with `None`. `joule_heat`, `face_currents` and
  `boundary_power` are unchanged (Q2).
* **Turbulence** (`rcfd-turb`). A public field `gradient: Option<GradientOptions>` (`None` from `new`) on `KEpsilon`, `KOmegaSst`
  and `SpalartAllmaras`; their transport Laplacian becomes `laplacian_opt` with the field's cell and boundary values. A trait
  method `TurbulenceModel::set_gradient_options(&mut self, Option<GradientOptions>) -> Result<(), String>`, by default `Ok` for
  `None` and an error for `Some` (the model has no transport equation that takes them); the three models set their field. The
  Reynolds stress model, the algebraic models and plugins keep the default.
* **Not wired, stated now:** the fractional step, the coupled solver, variable density, VOF, compressible, the Reynolds stress
  model, the Lorentz potential (Q5), the pressure-lag estimate (Q4), the reports and the derived electric quantities (Q2).

**K4. Case wiring** (`rcfd-api`). `Simulation::new`, after the checks of K5, sets `gradient_selected(case, "momentum")` and
`gradient_selected(case, "pressure")` on the steady kernel (`SimpleSolver` fields) and on the transient kernel (its settings).
`TurbState::new` calls `model.set_gradient_options(gradient_selected(case, "turbulence"))` after the model's other options, a
refusal located at `solver.gradient_equations.turbulence` (so `reconfigure_turbulence` does the same). `ElectricState::from_case`
keeps `gradient_selected(case, "electric_potential")` and `update` passes it to `solve_opt`.

**K5. Located errors** (each a `Diagnostic` at the path named; the first applicable rule wins; within a rule the selectors are
taken in the order `momentum`, `pressure`, `turbulence`, `electric_potential`). Made in `Simulation::new` where ADR 0190's R2 to R4
are, except R6-T.

* **R1 (values).** The schema refuses a selector that is not a boolean and an unknown key in the table, each at its key.
* **R5 (a selector without options).** A true selector while `gradient_face_correction`, `gradient_secondary_limiter` and
  `gradient_boundary` all hold their defaults; at the selector.
* **R6 (the equation is not solved by a kernel that takes the options)**, at the selector:
  * `momentum`, `pressure`: the built engine is not the steady (simple, simplec) or the transient (piso, pimple) constant-density
    kernel, or the flow is prescribed (`frozen_flow`), or `time_advance = "fractional_step"`;
  * `turbulence`: no turbulence model, or the flow is prescribed; **R6-T**: the model refuses the options (rsm, smagorinsky, wale,
    mixing_length, plugin), made in `TurbState::new` with the model's message;
  * `electric_potential`: `electric.enabled` is false.
* **R7 (no secondary flux)**, at the selector: `momentum` with `solver.convection` = `upwind`, `central` or `blended` and
  `non_orthogonal_correctors = 0`; `pressure` with `non_orthogonal_correctors = 0`; `turbulence` with `turbulence.convection` =
  `upwind`, `central` or `blended`. The potential always has one (item 5).
* **R8 (two treatments of the non-orthogonal pressure gradient).** `pressure` with `solver.pressure_interpolation =
  "body_force_weighted"` (ADR 0193 V2-c gives that gradient); at `solver.gradient_equations.pressure` (Q6).
* **ADR 0190 R2 and R3**: made as before when no selector is true, skipped when one is (R5 to R8 then decide); **R4** unchanged,
  last.

**K6. Separable commits.** (1) this text; (2) `rcfd-fv` and `rcfd-turb` (K3) with the library test; (3) `rcfd-api` (K1, K2, K4,
K5) with the tree tests; (4) documents and roadmap; (5) the addendum.

## Acceptance checks and limits (fixed here, before any code)

**Common.** Mesh W of ADR 0190: `wedge_2d`, `nx = ny = 16`, `lx = ly = 1`, `ramp_start = 0`, `ramp_angle = 30` (patches `xmin`,
`xmax`, `ymin` the ramp, `ymax`). Option sets as ADR 0190: O1 `gradient_face_correction = true`; O2 `gradient_boundary =
"corrected"`; O3 `gradient_boundary = "dropped"`; O4 `gradient_secondary_limiter = "barth_jespersen"`; O5
`gradient_secondary_limiter = "venkatakrishnan"` (K 5); O6 the same with `gradient_limiter_k = 0.5`; C2 = O1 + O2 + O5. "The
default" is a case without the F-256 keys and without the table.

**Measure.** Each case names its compared fields. `S_phi` is `max_c abs(phi_c)` of the default tree run of that case. For two runs
`a` and `b` of a case, `d(a, b)` is the largest over the compared fields of `max_c abs(a_phi - b_phi) / S_phi`.

**Cases** (case texts inline in `crates/rcfd-api/tests/gradient_equations_f291.rs`; no case file, fixture or docs TOML block is
added). Each case is the default with the selector named and the set's keys added.

| Case | Text | Selector | Compared | Sets |
|---|---|---|---|---|
| M1 | ADR 0190's S1 cavity without the scalar: `ymax` `velocity = "fixed"`, `velocity_value = [1, 0, 0]`, the other patches fixed zero; `physics.nu = 0.1`; `simplec`, `convection = "linear_upwind"`, `max_iterations = 60`, `tolerance_u = tolerance_p = 1e-14` | `momentum` | `ux`, `uy`, `p` | O1 to O6, C2 |
| M2 | M1 with `algorithm = "pimple"`, `dt = 0.03125`, `end_time = 0.5` (16 steps, exact in binary), `time_scheme = "euler"` | `momentum` | `ux`, `uy`, `p` | C2 |
| P1 | channel on W: `xmin` `velocity = "fixed"`, `velocity_value = [1, 0, 0]`; `xmax` `velocity = "zero_gradient"`, `pressure = "fixed"`, `pressure_value = 0`; `ymin`, `ymax` fixed zero; `nu = 0.1`; `simplec`, `linear_upwind`, `non_orthogonal_correctors = 1`, 60 iterations, tolerances 1e-14 | `pressure` | `ux`, `uy`, `p` | O1 to O6, C2 |
| P2 | P1 with `simplec_non_orthogonal_compensation = true` | `pressure` | `ux`, `uy`, `p` | C2 |
| P3 | P1 with `pimple`, `dt = 0.03125`, `end_time = 0.5`, `euler` | `pressure` | `ux`, `uy`, `p` | C2 |
| T1 | M1 with `[turbulence]` `model = "k_omega_sst"`, `convection = "linear_upwind"`, `k_initial = 0.01`, `omega_initial = 1.0`, and `boundaries.ymax.turbulence = "wall"` | `turbulence` | `ux`, `uy`, `p`, `k`, `omega` | O1 to O6, C2 |
| T2 | T1 with `model = "k_epsilon"` and `epsilon_initial = 9e-4` in place of `omega_initial` | `turbulence` | `ux`, `uy`, `p`, `k`, `epsilon` | C2 |
| T3 | T1 with `model = "spalart_allmaras"`, without `k_initial` and `omega_initial` | `turbulence` | `ux`, `uy`, `p`, `nuTilda` | C2 |
| E1 | ADR 0190's E1 (prescribed zero flow, steady energy) with `max_iterations = 20` and `[electric]` `enabled = true`, `conductivity = 1.0`; `xmin` `electric = "potential"`, `electric_potential = 1.0`; `ymin` `electric = "potential"`, `electric_potential = 0.0`; the other patches insulated | `electric_potential` | `electric_potential` | O1 to O6, C2 |

**Library runs** (`rcfd_fv` and `rcfd_turb` only, on `Simulation::geom`; no `rcfd-api` helper), each set up as the tree sets it up
and given the set's options on the field named:

* M1, P1, P2: `SimpleSolver::new(geom, u, p, SimpleSettings::new(0.1, Coupling::Simplec))`, `convection = LinearUpwind`,
  `alpha_u = 0.9`, `alpha_p = 1.0`, `non_orth_correctors` 0 (M1) or 1 (P1, P2), `simplec_non_orth_compensation` (P2); `u` with
  `FixedValue::vector` on the fixed patches and `ZeroGradient` on `xmax` (P), `p` with `FixedValue::scalar(0)` on `xmax` (P) and
  `ZeroGradient` elsewhere; options on `gradient_momentum` (M1) or `gradient_pressure` (P1, P2); 60 `iterate` calls.
* M2, P3: `PimpleSolver::new` with `PimpleSettings::pimple(0.1, 1)`, `convection = LinearUpwind`, `n_correctors = 2`,
  `non_orth_correctors` 0 (M2) or 1 (P3), `alpha_u = 0.9`, `alpha_p = 1.0`, `ddt_correction = true`, `time_scheme =
  ImplicitEuler`, options on `settings.gradient_momentum` or `settings.gradient_pressure`; 16 `step(0.03125)` calls.
* T1 to T3: `rcfd_turb::create_model` of the model's key with the `ModelContext` `TurbState::new` builds from the case (walls
  `["xmax", "xmin", "ymax", "ymin"]`, the sorted patch names; `k0`, `eps0`, `omega0` as the case; `nutilde0 = 3 nu`; `kappa =
  0.41`, `e_wall = 9.8`; `convection = LinearUpwind`; `relax_turb = 0.7`, `relax_nut = 0.5`; the other fields at the schema
  defaults), `apply_wall_options` with the default `WallOptions`, for SA `set_option` of `des`, `des_delta` and `c_des` at their
  defaults, then `set_gradient_options`; a `SimpleSolver` as M1 with `h_from_solution = true`; 60 times `update(u, phi)`, `nu_face =
  Some(model.nu_face())`, `iterate()`.
* E1: `ScalarField::uniform("phi", geom, 0)` with `FixedValue::scalar(1)` on `xmin` and `FixedValue::scalar(0)` on `ymin`;
  conductivity 1 in every cell; `SolveControl { tol_abs: 1e-300, tol_rel: 1e-13, max_iter: 5000 }`; 20 calls of
  `electric::solve_opt(geom, phi, sigma, ctl, 20, opts)`.

**Library set-up K** (`crates/rcfd-fv/tests/gradient_pressure_f291.rs`): ADR 0016's skewed mesh K at `n = 8`
(`box_mesh([8; 3], [1; 3])` warped by `x' = x + 0.7 y + 0.3 z`, `y' = y + 0.3 z`), a closed box: zero `FixedValue` velocity on
every patch, so `pressure_bc` makes every boundary face Neumann (`fixed_u`); cell pressures `p_c = cos(1.3 x + 0.2) sin(0.9 y +
0.4) cos(1.1 z) + 0.05 (-1)^c` at the centres (local extrema, so both limiters act); face diffusivity `gamma_f = 1 + 0.3 sin(2.1 x
+ 0.7 y - 0.4 z)` at the face centres; `grad = least_squares(p, boundary_values_pbc(p))`. For each set (and the default, `None`):
`(A, b) = laplacian_opt(geom, gamma, pbc.coeffs, Some(grad), p, bv, opts)`, the pressure operator of K3, and `phi =
corrected_flux_opt(geom, gamma, pbc, 0, p, 0, Some(grad), opts)`. `S = max_c sum_{f in c} abs(phi_f)`. Set-up K-B: the same with
`xmax` also at a fixed pressure 0 (velocity `ZeroGradient` there).

| Id | Check | Limit |
|---|---|---|
| P | `check_mesh` of W built through the tree: maximum non-orthogonality; of K: the same | W at least 20 degrees, K at least 25 (set-up; a miss stops the test) |
| F | every compared field of the default run of each case is finite and has `S_phi > 0`; every tree run ends after its 60 iterations, 16 steps or 20 iterations | set-up (a miss stops the test); a set's run with a value that is not finite: G2 and G3 of that set are NOT MET |
| G1 | the located errors of the table below | the paths as stated |
| G2 | each case: the control `d(default tree, library with None)`, and for each set `d(tree, library with the set)` | `<= 1e-12` |
| G3 | each case and set: `d(tree with the set, default tree)` | `> 1e-9` |
| G3-K | M1, P1, T1, E1: `d(O5, O6)` of the tree runs (the K value reaches the limiter) | `> 1e-12` |
| G4 (i) | each case: without keys, and with the four F-256 keys and the table with its four selectors written at their defaults | same settings content hash; every compared value equal bit for bit after the run |
| G4 (ii) | `baseline_hashes.rs` | passes unedited |
| G4 (iii) | `case_hashes_w48u.rs` (H1 and its controls); and the written text of `Case::new()` (`to_toml`, `to_json_string`) | passes unedited; no `gradient_equations` in either text |
| G4 (iv) | the library run of M1, M2, P2, P3, T1, T2, T3 and E1 with `Some(GradientOptions::default())` against `None` | every compared value equal bit for bit |
| P-lib | K, the default and each set: per cell `abs(div(phi)_c - ((A p)_c - b_c))` (the flux correction carries the face flux of the pressure operator it is paired with) | max `<= 1e-12 S` |
| P-lib-T | K, O1, O4, O5, O6, C2: the same with `phi` from `corrected_flux_with` (no options) against the operator with the set | max `> 1e-9 S` (the check can fail) |
| P-lib-D | K: `corrected_flux_opt` with `None` and with `Some(GradientOptions::default())` against `corrected_flux_with`; `solve_pressure_parts_opt` with both against `solve_pressure_parts` (`PressureControls` with `p_ref = (0, 0)`, three non-orthogonal correctors, `tol_abs = 1e-300`, `tol_rel = 1e-10`, `max_iter = 2000`, no AMG, the default linear options; `phi_hby_a` the face flux of the velocity `(sin(1.1 y), cos(0.9 z), sin(0.7 x))` at the cell centres, interpolated; start `p = 0`) | equal bits on every face and cell |
| P-lib-B | K-B, the default and each set: the P-lib measure over the cells with a fixed-pressure face, and over the others | reported (Q3); the others `<= 1e-12 S` |
| D | `rcfd-api`: `gradient_options_f256.rs`, `pressure_interp_f246.rs`, `cht_flows_f289.rs`, `turb_models.rs`, `rules_w19c.rs`; `rcfd-fv`: `gradient_sweep_f256.rs`, `pressure_interp_f246.rs`, `cht_flows_f289.rs`, `electric.rs` | pass unedited |

**G1 cases** (`Case::parse_toml`, then `Simulation::new`; the path of the first diagnostic). A selector named in a row is set in
`[solver.gradient_equations]`; O1 and O5 are the option keys of the sets:

| Rule | Case | Refused at |
|---|---|---|
| R1 | M1 with `[solver.gradient_equations]` `momentum = "yes"` | `solver.gradient_equations.momentum` |
| R1 | M1 with `[solver.gradient_equations]` `energy = true` | `solver.gradient_equations.energy` |
| R5 | M1 with `momentum = true` and no option key | `solver.gradient_equations.momentum` |
| R5 | M1 with `momentum = true` and `gradient_limiter_k = 1` only | `solver.gradient_equations.momentum` |
| R6 | ADR 0190's E1 (prescribed flow) with `momentum = true` and O1 | `solver.gradient_equations.momentum` |
| R6 | M1 with `algorithm = "coupled"`, `momentum = true` and O1 | `solver.gradient_equations.momentum` |
| R6 | M1 with `algorithm = "piso"`, `time_advance = "fractional_step"`, `dt = 0.03125`, `end_time = 0.5`, `momentum = true` and O1 | `solver.gradient_equations.momentum` |
| R6 | M1 with `momentum = true`, `turbulence = true` and O1 (laminar) | `solver.gradient_equations.turbulence` |
| R6-T | M1 with `turbulence.model = "mixing_length"`, `turbulence.length_scale = 0.5`, `momentum = true`, `turbulence = true` and O1 | `solver.gradient_equations.turbulence` |
| R6 | M1 with `momentum = true`, `electric_potential = true` and O1 | `solver.gradient_equations.electric_potential` |
| R7 | M1 with `convection = "upwind"`, `momentum = true` and O1 | `solver.gradient_equations.momentum` |
| R7 | P1 with `non_orthogonal_correctors = 0`, `pressure = true` and O1 | `solver.gradient_equations.pressure` |
| R7 | T1 with `turbulence.convection = "upwind"`, `turbulence = true` and O1 | `solver.gradient_equations.turbulence` |
| R8 | P1 with `physics.body_force = [0, -10, 0]`, `pressure_interpolation = "body_force_weighted"`, `pressure = true` and O1 | `solver.gradient_equations.pressure` |
| R4 | M1 with `momentum = true`, O1 and `gradient_limiter_k = 1` | `solver.gradient_limiter_k` |
| control | M1 with `momentum = true` and O1 | accepted |
| control | ADR 0190's E1 with O1 (no table) | accepted |
| control | M1 with `algorithm = "coupled"`, no keys | accepted |
| control | M1 with `piso`, `fractional_step`, `dt`, `end_time` as above, no keys | accepted |
| control | M1 with `mixing_length`, `length_scale = 0.5`, `momentum = true` and O1 | accepted |
| control | M1 with `convection = "upwind"`, `non_orthogonal_correctors = 1`, `momentum = true` and O1 | accepted |
| control | P1 with `pressure = true` and O1 | accepted |
| control | T1 with `turbulence = true` and O1 | accepted |
| control | P1 with the body force and `body_force_weighted` as above, `momentum = true` and O1 | accepted |
| control | M1 with `momentum = true`, O5 and `gradient_limiter_k = 1` | accepted |

Every refusal has a control that differs from it in the one setting the rule names, so the refusal is caused by that setting.
The refusals of cases without the table are ADR 0190's (its G1 passes unedited, D).

**Can fail.** Six mutations, each made, its test run, the file restored and compared byte for byte with the kept copy:
M1, the line of K4 that sets `gradient_momentum` on the steady kernel removed: G2 M1 O1 must fail; M2, the line that sets
`gradient_pressure` on the steady kernel removed: G2 P1 O1 must fail; M3, the line that sets the transient kernel's momentum
options removed: G2 M2 C2 must fail; M4, the `set_gradient_options` call of `TurbState::new` removed: G2 T1 O1 must fail; M5,
`ElectricState` keeps `None`: G2 E1 O1 must fail; M6, the face correction skipped in `corrected_flux_opt`: P-lib O1 must fail.
G3 is the teeth of the library wiring: an equation whose library assembly ignores the options leaves its tree and library runs
equal to the default.

`scripts/limit_audit.py` is run on the new test files.

**Gate.** fmt per package, clippy `--workspace --all-targets -D warnings`, every test in release on a `git archive` copy of the
gated commit (`scripts/gate_test.py`, one process per test, three jobs), serialised by the machine's gate lock. A failure of a known
flaky test is rerun alone and reported with its name.

**Decision rule.** Every check met: F-291 is done. A miss: an ignored `*_known_miss` test with the limit intact and the number
recorded, and F-291 stays open (partial) with the miss named.

## Open questions

**Q1. Names and place of the selectors.** (a) The optional table `[solver.gradient_equations]` of K1: four booleans that add
equations to the energy and scalar terms, which always take the options; (b) no selector: the F-256 keys act on every equation that
has a secondary flux (this changes the meaning of F-256's keys for every case that sets them, and its R3 refusal); (c) one option
set per equation (sixteen keys). (a) cannot give options to momentum without giving them to an energy equation that has a gradient.
Recommendation (a), reversible before any case uses it. Proceeding on (a).

**Q2. The electric current and the Joule heat with the options.** With `electric_potential = true`, `joule_heat`, the boundary
current density and `boundary_power` still use the default operator's face flows (item 5), so on skewed cells they are not the
flows of the solved potential. (a) Leave it and say so in the docs; (b) give them `conductive_face_flows_opt` with the potential's
options, as its own item with a current balance (inflow equals outflow, Joule heat equals the electric power, to the solver
tolerance), as ADR 0202 did for the heat flow; (c) refuse the selector with Joule heating. Recommendation (b) as its own item.
Proceeding on (a), documented.

**Q3. The flux correction on fixed-pressure faces.** The pressure equation has a secondary flux on fixed-pressure boundary faces
(with correctors) and the flux correction does not (item 3); the options keep that: the internal faces of the flux follow the
options, the boundary faces do not change, and `gradient_boundary = "dropped"` removes the term from the equation so that the two
agree there. P-lib-B reports the size. (a) Leave it; (b) under the options, give the flux correction the boundary term the
equation has (owner, corrected or dropped), so that the corrected flux closes every cell's balance; (c) an item that measures what
the default's mismatch does to the mass balance of a converged run on a skewed outlet first (fixing the default needs the
bit-identical rule lifted). Recommendation (c) first, then (b) if the measurement shows it matters. Proceeding on (a).

**Q4. The pressure-lag estimate.** The PISO/PIMPLE estimate of ADR 0073 (and the SV301 pre-run trial) solves the default pressure
operator also with `pressure = true`. (a) Leave it (it estimates a linear map; the limiters are not linear); (b) give it the
options. Recommendation (a). Proceeding on (a).

**Q5. The Lorentz potential** (ADR 0039, `sources.<n>.potential = "solve"`). (a) Not wired; `electric_potential` names the
potential of `electric.enabled` (R6 refuses it without); (b) wire it under the same selector, with checks of its current
reconstruction, as a new item. Recommendation (a) until a case needs it. Proceeding on (a).

**Q6. The options with `body_force_weighted`.** (a) Refuse the pair (R8); (b) let the options act on ADR 0193's V2-c gradient.
Recommendation (a), the two were never checked together. Proceeding on (a).

**Q7. Other solvers and models.** The coupled solver, the fractional step, variable density, VOF, compressible, the Reynolds stress
model and the algebraic models are refused (R6). (a) Leave them; (b) new items. Recommendation (a). Proceeding on (a).

## Not in F-291

New gradient schemes, limiter changes, a default change, the derived electric quantities (Q2), the flux correction's boundary faces
(Q3), the pressure-lag estimate (Q4), the Lorentz potential (Q5), the solvers and models of Q7, the momentum of the ADR 0175
test-only masked Laplacian, a claim that the options improve accuracy (ADR 0016 found none on the scalar).

## Where to go next

* [ADR 0190](0190-f256-registration-case-tree-wiring-of-the-gradient-options.md) for the options, their keys and their wiring.
* [ADR 0016](0016-wave13-numerics.md) D2 for what the options do on the manufactured scalar problem.
* [Case schema](../case-schema.md), section "Gradient options of the energy and scalar diffusion term".

## Amendment 1 (2026-11-06): the set-up of T2, and what the first runs showed about P1 and M2

Written after the first runs and before any corrected run. Run before this text, with the code of K3 to K5: the library test
(`gradient_pressure_f291.rs`: P, P-lib, P-lib-T, P-lib-D, P-lib-B), G1, and G2 with G3 of M1, M2, P1, P2, P3, T1 and E1; T2 as
registered (below). Not run: T3, G4 (i) and G4 (iv). No limit is changed.

**Finding 1: T2 as registered does not parse.** `Case::parse_toml` refuses T2 at `boundaries.ymax.turbulence` ("moving walls are
not supported by the wall functions (the wall velocity must be zero)"): the k-epsilon wall functions refuse the lid of M1 as a
wall, which T1 (SST) accepts. The test `g2_g3_t2_t3_k_epsilon_and_spalart_allmaras_c2` stopped there, before T3. A set-up defect;
corrected as follows.

* **T2b (corrected):** T2 with `boundaries.ymax.turbulence = "zero_gradient"` in place of `"wall"`: the lid is neither a wall nor
  an inlet of the model (zero-gradient k and epsilon there), the walls are `xmax`, `xmin` and `ymin` (sorted names). The library
  run of T2b passes those walls. Compared fields, sets and limits as T2.
* T3 runs as registered, in a test of its own.
* The registered test stays as the ignored record `g2_g3_t2_t3_k_epsilon_and_spalart_allmaras_c2_setup_defect`; the corrected
  tests are `g2_g3_t2b_k_epsilon_c2` and `g2_g3_t3_spalart_allmaras_c2`.
* G4 (i) and G4 (iv), not yet run, take T2b in place of T2.

**Finding 2 (disclosed): P1's default run is not a converged flow.** After its 60 iterations the default run of P1 has largest
speeds of 15.2 (`ux`) and 30.1 (`uy`) against the inlet's 1, and a pressure scale of 320: SIMPLEC with a non-orthogonal corrector
and without `simplec_non_orthogonal_compensation`, the configuration ADR 0050 D1 found unstable. G2 and G3 compare the same 60
iterations and stay as registered; G3's changes on P1 are therefore changes of a diverging iterate, not of a flow. P2 (the same
channel with the compensation) has scales of 3.19 and 0.93 against the inlet's 1. M2's default run (transient cavity) has a
largest speed of 6.42 against the lid's 1 after its 16 steps; disclosed, no change.

* **P2-all (supplementary, not binding):** P2 with every set O1 to O6 and C2, G2 and G3 with the registered limits (`<= 1e-12`,
  `> 1e-9`) and G3-K, in the test `g2_g3_p2_all_sets_supplementary`; the scales of its default run are printed. It is evidence
  of the options on a channel that does not diverge; P1 stays the binding case of the pressure checks.

**Q8 (new). The P1 set-up.** (a) Keep P1 binding as registered (the checks hold whatever the iterate does) with P2-all as
supplementary evidence; (b) make P2-all the binding all-set case of the pressure. Recommendation (a): (b) would change the
acceptance after a run. Proceeding on (a).
