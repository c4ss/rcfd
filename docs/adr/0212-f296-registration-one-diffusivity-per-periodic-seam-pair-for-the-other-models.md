# ADR 0212: F-296 registration: one diffusivity per periodic seam pair for the other models

Status: **REGISTERED 2026-11-07**, written before any code of F-296 and before any run of its checks. Not yet accepted by the
maintainer. Before this text, one probe of the existing code was run on main 54167b0 (scratch tests, not committed): the face
values and global diffusion budgets of every model named below with the existing (owner) rule, the sweep budgets of k-epsilon,
Spalart-Allmaras and the Reynolds stress model, and the seam pair differences of the case-tree texts. Its numbers are quoted in
item 9, where they set the limits of the negative controls. Results go in the addendum at the end.

Parents: [ADR 0205](0205-f295-registration-one-diffusivity-per-periodic-seam-pair.md) (F-295: the rule, the key, the pairs,
question 1 with option (b) taken by the lead), ADR 0155 (optional objects), ADR 0156 D6 (`hash_omit_default`), ADR 0036 (laminar
regions), ADR 0032 (the Reynolds stress model), ADR 0026 (the algebraic models), F-110 (rheology). Roadmap item F-296 (depends on
F-295).

Number: the lead gave 0212 to F-296.

House rules ([ADR 0130](0130-wave48a-overview-and-run-record-contract.md)), as in ADR 0205:

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the number recorded, and the cause is
  diagnosed, not tuned away;
- a defective set-up gets a new corrected test by an amendment written before the corrected run, and the old one stays as an
  ignored record;
- the new code path is opt-in and the defaults stay bit-identical: `crates/rcfd-api/tests/baseline_hashes.rs` and
  `case_hashes_w48u.rs` are not edited or re-recorded, and no golden is re-recorded;
- each registered run is made once, and nothing is re-run to pick a result;
- no new third-party crate, no commit trailers.

Clean-room: the rule is the flux balance of a pair of finite-volume faces (ADR 0205 D1), applied to the face formulas that the
models of `crates/rcfd-turb` and the viscosity laws of `crates/rcfd-api/src/rheology.rs` already use on internal faces. No vendor
code, text or install was used.

## What F-296 asks

The roadmap entry: the rule of F-295 (`turbulence.periodic_seam.diffusivity = "pair"`) is applied to the face viscosity or
diffusivity of k-epsilon, Spalart-Allmaras, the Reynolds stress model, the subgrid and mixing-length models and the rheology face
viscosity, each with a global-budget check on a periodic box with a differing viscosity (the owner rule must miss it, the pair
rule must meet it); defaults bit-identical (baseline hashes unchanged).

## What the code read shows (main 54167b0)

ADR 0205 items 1 to 3 (the periodic faces, the periodic condition with its lagged partner value, the diffusion operator) hold
unchanged. `FvGeometry::periodic_pairs` (F-295) lists the pairs `(f, g)`, `f < g`, with `w = dn_g / (dn_f + dn_g)`.

1. **k-epsilon** (`kepsilon.rs`, every variant). The momentum face viscosity `KEpsilon::nu_face` is `effective_viscosity(nu, nut,
   wall_nut)`: a periodic face takes `nu + nu_t` of its own owner. Every sweep computes the face eddy viscosity
   `nut_f = effective_viscosity(0, nut)` once and gives k and epsilon the diffusivities `nu + nut_f / sigma_k` and
   `nu + nut_f / sigma_e` (`transport`); a periodic face again takes its owner's `nu_t`. The model has no time derivative on
   main (F-294 adds one on another branch) and makes one sweep per call. Wall cells take the wall-function production and
   epsilon; the wall epsilon is imposed with a dominant diagonal, so the epsilon equation has no conservation row in a wall cell.
2. **Spalart-Allmaras** (`sa.rs`). `nu_face` is `effective_viscosity(nu, nut, wall_nut)` (owner on periodic faces); with wall mode
   `auto_sa_law` the internal faces are then overwritten by the law's face viscosity, the boundary faces are not. The `nuTilda`
   diffusivity is different: internal faces take `nu + w e_P + (1 - w) e_N` with the cell values `e = f_n(nuT) nuT` (SA-neg) or
   `max(nuT, 0)`, and every boundary face takes `nu + f_n(b) b` at its boundary value `b`, all divided by `sigma`. On a periodic
   face `b` is the periodic face value `w nuT_P + (1 - w) nuT_Q`, which the two faces of a pair compute from the same two cells.
   **So the `nuTilda` diffusivity is already one value per pair** (equal bits on a uniform seam, a last-bit difference on a graded
   one; item 9): its diffusion is conservative across the seam today. It differs from the internal-face rule where `nuT < 0` on a
   seam cell (`f_n` of the interpolated value, not the interpolation of `f_n nuT`).
3. **The Reynolds stress model** (`rsm.rs`). `nu_face` is `effective_viscosity(nu, nut)` (owner). The explicit momentum source
   removes the part that the solver already treats implicitly through `nu + nu_t` with a Laplacian of `effective_viscosity(0,
   nut)` (owner), so it must use the same face values as `nu_face` to remove that part exactly. The stress and epsilon
   diffusivities are `nu + c (k/eps)_f R_nn,f` with `c = c_s` (stresses) or `c_eps` (epsilon), `(k/eps)_f` and `R_f` linearly
   interpolated on internal faces and taken from the owner on boundary faces, and `R_nn,f = max(0, n_f . R_f . n_f)`. The model
   has an implicit-Euler time derivative (`begin_step`, one sweep per step). `tensor_divergence` (the explicit stress divergence)
   interpolates across translational seams and takes the owner on rotational ones; `body_force_from_cells` takes the owner on
   every boundary face. Neither is a diffusivity.
4. **Smagorinsky, WALE, mixing length** (`les.rs`, `zeroeq.rs`). All three give the momentum face viscosity through
   `WallInfo::nu_face` = `effective_viscosity(nu, nut, wall faces at 0)`: owner on periodic faces. They have no transported scalar.
5. **Rheology** (`rheology.rs`, F-110). `Rheology::nu_faces` interpolates the cell viscosities `mu(shear, T)` linearly to internal
   faces and takes the owner on boundary faces, divides by `rho`, then relaxes every face against the previous call,
   `nu = prev + relax (nu - prev)` (face by face, `rheology.relax`, default 0.7). A viscosity law cannot be combined with a
   turbulence model (`rheology::check`), so it runs with `turbulence.model = "laminar"`. It is built in `Simulation::new`.
6. **The case tree** (ADR 0205 items 8 and 9, R1). `TurbState::new` calls `set_option("periodic_diffusivity", "pair")` for every
   model when the key is `pair` and keeps the pairs for the laminar-region rule of `TurbState::nu_face`; `check_periodic_seam`
   refuses `pair` with every model but `k_omega_sst` (R1 of ADR 0205), so today only SST gets there. The trait's default
   `set_option` refuses an unknown name. A live `settings.set` of a `turbulence.*` path rebuilds the turbulence state through
   `TurbState::new` (`reconfigure_turbulence`), and on a simulation built without a turbulence model it is refused with the
   message to rebuild it.
7. **The test of ADR 0205 S1** (`crates/rcfd-api/tests/f295_seam_diffusivity.rs`, `s1_scope_of_the_seam_diffusivity`) asserts that
   `pair` with `k_epsilon` and `spalart_allmaras` is refused. F-296 accepts both, so that test no longer describes the code (D4).
8. **Not named by F-296**: the temperature-dependent property viscosity (`properties.rs`, `nu_faces`), the user-function viscosity
   (`udf.<n>.viscosity`, per face in `sim.rs`) and the face values of `tensor_divergence` and `body_force_from_cells` above. Each
   takes the owner (or its own blend) on a periodic face.
9. **The probe** (main 54167b0, existing code, release, not committed). Meshes BOX, SEC and GBOX of ADR 0205 (GBOX: amendment 2,
   `w = 0.8`), `nu = 0.1`.
   - Face values, every model through `create_model` and `initialize_from_nut(0.1 (1 + 0.5 sin(2 pi s)))` (k_epsilon,
     spalart_allmaras, smagorinsky, wale, mixing_length, rsm): the largest pair difference of `nu_face` is 7.83e-2 (BOX), 0.155
     (SEC), 8.83e-2 (GBOX) of the largest `nu_face`, the same for all six; the global diffusion budget G of ADR 0205 G1 with
     `nu_face` is 8.67e-3, 8.79e-3, 4.88e-3 (BOX: q, u_x, u_y), 3.23e-2 (SEC: q), 9.08e-3, 9.19e-3, 6.85e-3 (GBOX). Every model
     refuses the option name `periodic_diffusivity` today.
   - Rheology (`power_law`, `consistency_index = 0.0025`, `power_index = 0.6`, `relax = 0.7`) on the velocity of ADR 0205 G1:
     pair difference 5.76e-3 (BOX), 3.28e-2 (SEC), 2.26e-2 (GBOX) of the largest face viscosity; G 7.34e-4, 7.44e-4, 3.73e-4 (BOX),
     6.08e-3 (SEC), 2.85e-3, 2.89e-3, 1.97e-3 (GBOX).
   - Sweep budgets (the set-ups K1, A1, A1p and Q1 below, one sweep each, `owner`): the budget with the lag term of the face
     values the sweep used (`L_owner`) closes to 5.3e-15 or less in every case (k-epsilon 6.0e-16 / 5.3e-15, SA 2.6e-15 / 1.1e-15
     / 2.3e-15 / 2.0e-16, RSM 1.2e-16 to 2.5e-15), so the cell terms and the lag term below are those of the sweep. With the lag
     term of the D1 value (`L_D1`) the owner sweeps miss by: k-epsilon k 4.67e-3 (BOX), 4.23e-2 (GBOX); RSM 4.37e-2 (normal
     stresses), 7.57e-3 (`Rxy`), 2.29e-2 (epsilon) on BOX and 5.14e-2, 2.49e-2, 5.72e-2 on GBOX; SA with `nuTilda < 0` at a seam
     cell 1.40e-4 (BOX), 3.39e-4 (GBOX); SA with positive `nuTilda` 2.6e-15 and 2.2e-15 (the existing rule is D1 there, item 2;
     its two pair values differ by 0 on BOX and 5.6e-17 on GBOX). The largest step change is 0.11 to 1.35.
   - The RSM momentum source budget M1 with `owner`: 4.65e-2, 2.45e-2 (BOX, x and y), 5.22e-2, 3.31e-2 (GBOX).
   - Case tree (texts below, `owner`, largest relative pair difference of the solver's momentum face viscosity): CHX k_epsilon
     2.03e-2 after 5 SIMPLEC iterations, spalart_allmaras 5.19e-2, mixing_length 2.98e-2, laminar power law 3.11e-2; TGS
     smagorinsky (piso) 2.72e-2 after 3 steps, wale (pimple) 0.136, rsm (piso) 1.48e-4 after 5 steps; LAMX k_epsilon: the
     largest seam face viscosity 11.4 `nu` after 3 iterations. Every text builds and runs in well under a second.

## Decisions (fixed now)

**D1. The rule.** For every pair `(f, g)`, `f < g`, `P` the owner of `f`, `Q` the owner of `g`, `w = dn_g / (dn_f + dn_g)`, both
faces take one value, computed once per pair and written on both: **the formula the model uses on its internal faces, with the
pair's weight `w` in place of the internal weight and the cells `P`, `Q` in place of owner and neighbour.** Where a diffusivity is
`nu + a_f` with `a` a cell value interpolated to the face, this is ADR 0205 D1, `nu + w a_P + (1 - w) a_Q`. Per model:

| Model | Face value | Pair value (both faces) |
|---|---|---|
| k-epsilon | momentum `nu + nu_t` | `nu + w nu_t,P + (1 - w) nu_t,Q` |
| k-epsilon | k, epsilon: `nu + nut_f / sigma` | `nut_f = w nu_t,P + (1 - w) nu_t,Q` |
| Spalart-Allmaras | momentum `nu + nu_t` | `nu + w nu_t,P + (1 - w) nu_t,Q` |
| Spalart-Allmaras | `nuTilda`: `(nu + e_f) / sigma` | `(nu + w e_P + (1 - w) e_Q) / sigma`, `e` the cell values of item 2 |
| Reynolds stress | momentum `nu + nu_t` | `nu + w nu_t,P + (1 - w) nu_t,Q` |
| Reynolds stress | momentum-source viscosity `nu_t` | `w nu_t,P + (1 - w) nu_t,Q` |
| Reynolds stress | stresses, epsilon: `nu + c (k/eps)_f R_nn,f` | `nu + c (w kappa_P + (1 - w) kappa_Q) max(0, w r_P + (1 - w) r_Q)` |
| Smagorinsky, WALE, mixing length | momentum `nu + nu_t` | `nu + w nu_t,P + (1 - w) nu_t,Q` |
| rheology | `mu_f / rho` | `(w mu_P + (1 - w) mu_Q) / rho`, before the relaxation |

For the Reynolds stress model `kappa = k/eps` of the cell (the existing `max(., 1e-30)` floors) and `r_P = n_f . R_P . n_f`,
`r_Q = n_g . R_Q . n_g`, each owner's normal stress on its own face, with `max(0, .)` after the interpolation as on internal faces.
On a translational seam (`n_g = -n_f`) this is the internal-face formula exactly; on a rotational seam each normal stress is
taken in the frame of its own face, which is what makes the two owners comparable. The rheology relaxation is face by face, so a
history written in `pair` mode stays one value per pair. Wall faces are never seam faces. Rejected: as in ADR 0205 D1.

**D2. Where it acts.** The models and face values of the table, steady and transient, translational and rotational seams. The
laminar-region rule of ADR 0205 D2 (a pair that touches a laminar cell takes `nu` on both faces of the momentum face viscosity)
lives in `TurbState::nu_face` and applies to every model that keeps the pairs, so it extends to them without new code. The scalars
with a Schmidt number and the transposed stress source follow the momentum face viscosity (ADR 0205 D2). Unchanged: k-omega SST
(F-295), the lag, the convection, the face values of item 8, and every result with `owner`.

**D3. The key.** Unchanged: `turbulence.periodic_seam.diffusivity`, `"owner"` (default) | `"pair"`, the optional object of ADR
0205 D3 with `hash_omit_default`. Only its documentation text changes (the schema text is not part of the settings data or of any
hash). Case text as in ADR 0205.

**D4. Scope and refusals.**

- R1' (replaces ADR 0205 R1): `pair` is accepted with `k_epsilon`, `k_omega_sst`, `spalart_allmaras`, `smagorinsky`, `wale`,
  `mixing_length` and `rsm`, and with `laminar` when a viscosity law is set (`rheology::enabled`: `rheology.model` other than
  `newtonian` or a `rheology.shift`). It is a static located error at `turbulence.periodic_seam.diffusivity` (`semantic_errors`)
  with `laminar` without a viscosity law (nothing to pair) and with `plugin` (a plugin model owns its diffusivities; the option is
  not passed to it). The message names the accepted models and the viscosity law.
- R2: unchanged (a value other than `owner` and `pair` is the enumeration's located error).
- R3: for every model, `set_option("periodic_diffusivity", "pair")` returns an error when the periodic maps are not a pairing
  (`periodic_pairs`), and the model keeps the owner rule; `TurbState::new` locates it at the key (as for SST). For the viscosity
  law `Simulation::new` locates it at the key. `owner` never reads the pairs.
- `pair` on a mesh without periodic patches changes nothing. A live `settings.set` of the key rebuilds a turbulence model as in
  ADR 0205; on a laminar run with a viscosity law it is refused by the existing live path (item 6: rebuild the simulation), which
  F-296 does not change.
- ADR 0205 S1 (item 7): the test `s1_scope_of_the_seam_diffusivity` becomes an ignored record whose reason names R1' of this ADR;
  its text is not otherwise changed. Its accepted cases, its laminar refusal and its R2 case are cases of S1' below, with the
  k_epsilon and spalart_allmaras cases accepted. This is the only edit of an existing test.

**D5. Code (the plan, with the files).**

- `crates/rcfd-turb/src/eddy.rs`: `SeamPairs`, the pairs of a model (none for `owner`), with `set(geom, mode)` (R3),
  `mode()`, `pairs()` and `apply(geom, nu, cell, face)` (`pair_periodic_faces` when set); `seam_mode(value)`, the parse of
  `owner` / `pair` for the option. `SeamDiffusivity` and `pair_periodic_faces` stay as they are.
- `crates/rcfd-turb/src/kepsilon.rs`: a `SeamPairs` field, `set_periodic_diffusivity(mode)`, `periodic_diffusivity()`; `nu_face`
  applies it to `nu_t` with `nu`, the sweep to `nut_f` with 0.
- `crates/rcfd-turb/src/sa.rs`: the same field and methods; `nu_face` applies it after the `auto_sa_law` override; the sweep
  applies it to the `nuTilda` diffusivity (cell values `e`, with `nu`) before the division by `sigma`.
- `crates/rcfd-turb/src/rsm.rs`: the field; `set_option("periodic_diffusivity", ..)`; `nu_face` and the momentum-source viscosity
  apply it; the sweep writes the pair value of D1 on the stress and epsilon diffusivities.
- `crates/rcfd-turb/src/les.rs`: a `SeamPairs` in `WallInfo`, applied in `WallInfo::nu_face`; `AlgebraicSgs::set_option`.
- `crates/rcfd-turb/src/zeroeq.rs`: `MixingLengthModel::set_option`.
- `crates/rcfd-turb/src/model.rs`: `KEpsilonModel::set_option` (the option only) and the option in `SpalartAllmaras`'s
  `set_option`.
- `crates/rcfd-api/src/rheology.rs`: the pairs of the law (`set_seam_pairs`), applied in `nu_faces` before the relaxation.
- `crates/rcfd-api/src/sim.rs`: in `Simulation::new`, the pairs given to the viscosity law for `pair` (R3 at the key).
- `crates/rcfd-api/src/turb.rs`: R1' in `check_periodic_seam`. `TurbState::new` is unchanged.
- `crates/rcfd-api/src/schema.rs`: the documentation text of the key. `docs/case-schema.md`: prose (no TOML block added or
  changed).
- `crates/rcfd-api/tests/f295_seam_diffusivity.rs`: the `#[ignore]` of D4 on `s1_scope_of_the_seam_diffusivity`.

**D6. Why the defaults stay bit-identical.** Without the key (or with `owner`) no model and no viscosity law holds pairs, so every
face value is computed by the existing expression and the new code only tests for the absent pairs; the data, written text and
hashes are unchanged (the schema object is unchanged); SST is not touched. V0 checks this.

## Checks and limits (fixed now)

Every check is run once, in release. All but the report lines are gate tests. Set-ups shared by the library checks: the meshes
BOX, SEC, GBOX of ADR 0205 (with amendment 2), `nu = 0.1`, `s = x` (BOX, GBOX) or `theta / angle` (SEC), the F field
`nu_t = 0.1 (1 + 0.5 sin(2 pi s))`, and for the G checks the fields `q` (all meshes) and `u_x`, `u_y` (BOX, GBOX) of ADR 0205 G1.
`pair` is set with `set_option("periodic_diffusivity", "pair")` (the hook of the case tree) or the model's
`set_periodic_diffusivity`.

### Library checks (`crates/rcfd-turb/tests/f296_seam_diffusivity.rs`)

The models through `create_model` with a context of `nu = 0.1`, the model walls of ADR 0205 (`ymin`, `ymax`; SEC `inner`,
`outer`), `k0 = 1`, `eps0 = 1`, `nutilde0 = 0.1`, `length_scale = 0.5`, defaults otherwise; the state from
`initialize_from_nut(F field)`. Models: `k_epsilon`, `spalart_allmaras`, `smagorinsky`, `wale`, `mixing_length`, `rsm`.

| Id | Check | Limit |
|---|---|---|
| F2 | each model, each mesh: `nu_face` in `owner` and in `pair` (the same state). `pair`: every pair has the same bits on both faces, equal to the bits of `nu + w nu_t,P + (1 - w) nu_t,Q` computed in the test from the model's `nut()`; every face that is not a seam face has the bits of the `owner` result. Non-vacuity: in `owner` the largest pair difference `>= 1e-3` of the largest `nu_face` | exact, exact, exact; `>= 1e-3` |
| G2 | each model, each mesh, each field: the global diffusion budget G of ADR 0205 G1 with `nu_face` | `pair <= 1e-13`; negative control `owner >= 1e-5` |
| R3' | BOX with the broken map of ADR 0205 R3, each model: `set_option(.., "pair")` returns `Err`, and `nu_face` afterwards has the bits of the `owner` result; `set_option(.., "owner")` returns `Ok` | as stated |
| M1 | `rsm` on BOX and GBOX, the F state, velocity `u` of G: `abs(sum_c V_c (s_c + d_c)_i) / sum_c abs(V_c (s_c + d_c)_i)`, `i = x, y`, with `s` the model's `momentum_source(u)` and `d` = `tensor_divergence` of its stresses (so that `V (s + d)` is the Laplacian residual of the momentum-source viscosity) | `pair <= 1e-13`; `owner >= 1e-5` |

**Sweep budgets.** One sweep on a frozen velocity `u = (0, 1 + 0.5 sin(2 pi x + 0.5), 0)` (normal to the walls `ymin`, `ymax`, so
the tangential wall speed is 0) with a zero face flux (no convection, so no convective seam lag), linear solver
`tol_rel = 1e-14`, `tol_abs = 1e-300`, `max_iter = 1000`, on BOX and on GBOX. With `x*` the field before the sweep and `x'`
after it, `r_c` the cell terms of the solved equation (for every cell the net diffusive outflow of the cell: the sources and sinks
of the model's sweep evaluated in the test with the model's own public functions, plus the time term where there is one), and the
seam lag term of ADR 0205 item 2 for one common value `G` per pair,

```text
L(G) = sum_pairs G (|S_f| (x'_P - x*_Q) + |S_g| (x'_Q - x*_P)) / (dn_f + dn_g),
```

the budget is `B(G) = abs(sum_c r_c - L(G)) / sum_c abs(r_c)`: the global balance of the sweep, with the lagged partner value
accounted for. If the sweep used the value `G` on both faces of every pair, `B(G)` is round-off; if not, `B(G)` holds the
difference of the face values times the jump across the seam. `L_owner` puts on each face the value the existing rule gives it
(not one value per pair), `L_D1` the pair value of D1 on both.

- **K1** (k-epsilon, k): `KEpsilon` standard with walls `ymin`, `ymax`, `relax_turb = 1`, `relax_nut = 1`; `k = 1 + 0.5 sin(2 pi x)`,
  `eps = 1 + 0.3 sin(2 pi x + 1)`, `nut` the F field. Cell terms `r = P V - (e / k*) V k'` with `P = nu_t S^2`, `e = eps*` in the
  interior and the wall-cell values of `wall_cell_sources` (smooth treatment, tangential speed 0) in wall cells. Only k: the
  epsilon equation has no conservation row in a wall cell (item 1); it uses the same `nut_f` (D5), which MU2 unpairs.
- **A1** (Spalart-Allmaras, `nuTilda`): `SpalartAllmaras` with walls `ymin`, `ymax`, wall mode `resolved`, `nuTilda` zero gradient
  on the walls, `nuTilda = 0.1 (sin(2 pi x) - 0.1)` (negative at the `xmax` seam cells), `begin_step(0.05)`. Cell terms: the
  production, destruction and `cb2` terms of the sweep and `V (x' - x*) / dt`.
- **A1p** (the same with `nuTilda = 0.1 (1 + 0.5 sin(2 pi x))`, positive): the finding of item 2.
- **Q1** (Reynolds stress, every component with a non-zero cell term and epsilon): `ReynoldsStress` (SSG), isotropic stresses
  `R = 2/3 k delta`, `k = 1 + 0.5 sin(2 pi x)`, `eps = 1 + 0.3 sin(2 pi x + 1)`, `begin_step(0.05)`. Cell terms from
  `homogeneous_rates` and `stabilisation` and the time term. `Rxz` and `Ryz` stay zero in this set-up and are not checked
  (their cell terms are all zero; the test asserts it).

| Id | Limit |
|---|---|
| K1, A1, Q1 | `pair`: `B(L_D1) <= 1e-12` on BOX and GBOX. Precondition (a miss is a defective set-up of the harness): in the `owner` run `B(L_owner) <= 1e-12`. Negative control: in the `owner` run `B(L_D1) >= 1e-6`. Non-vacuity: `max abs(x' - x*) >= 1e-3`; K1 also `min k' > 1e-10` (the floor of the sweep did not act) |
| A1p | `pair`: `B(L_D1) <= 1e-12`; `owner`: `B(L_owner) <= 1e-12` and `B(L_D1) <= 1e-12` (no negative control is possible: the existing rule is D1 for positive `nuTilda`, item 2); `max abs(x' - x*) >= 1e-3` |

The negative-control limits are below the probe's smallest owner value (1.40e-4, SA) by more than a factor 100; the precondition
is above the probe's largest harness value (5.3e-15) by more than a factor 100.

### Case-tree checks (`crates/rcfd-api/tests/f296_seam_diffusivity.rs`)

Texts (`crates/rcfd-api/tests/f296_common/mod.rs`): **CHX**, the CH channel of ADR 0205 (`box_2d` 4 x 20, periodic in x, drive
`(1, 0, 0)`, upwind) with `initial.velocity_expression_x = "15 [m/s] + 3 [m/s] * sin(6.283185307179586 * x / 1 [m])"`, SIMPLEC,
and the model lines `k_epsilon`; `spalart_allmaras`; `mixing_length` with `length_scale = 1.0`; `laminar` with `[rheology]`
`power_law`, `consistency_index = 0.0025`, `power_index = 0.6` (**CHXR**); **TGS**, the doubly periodic Taylor-Green box of
`f286_default_bits.rs` (16 x 16, `2 pi`, `nu = 0.05`, `dt = 0.01`) with the vortex shifted by 0.5 in x (`sin(x + 0.5) cos(y)`,
`-cos(x + 0.5) sin(y)`, so that the seam pairs differ), `smagorinsky` on `piso`, `wale` on `pimple` (`n_outer = 3`, `euler`),
`rsm` (`k_initial = epsilon_initial = 1`) on `piso`, central convection; **LAMX**, CHX k_epsilon with the laminar region of ADR
0205 LAM (`box_min = [-1, -1, -1]`, `box_max = [0.25, 3, 1]`); **NPK**, CHX k_epsilon without the seam and the drive, `xmin` a fixed
inlet `(15, 0, 0)` with `turbulence_length = 0.14`, `xmax` a fixed-pressure outlet.

| Id | Check | Limit |
|---|---|---|
| S1' | scope. Accepted with no diagnostic (`parse_toml`, `semantic_errors`, `Simulation::new`) with `pair`: CHX k_epsilon, spalart_allmaras, mixing_length, CHXR, TGS smagorinsky, wale, rsm, and the CH texts of ADR 0205 S1 (SST on SIMPLEC and on pimple, NP). Accepted with `owner`: CH (SST), CH k_epsilon, CH laminar. Located statically at the key (the error of `Case::parse_toml` or `semantic_errors`, ADR 0205 amendment 1) with `pair`: CH laminar without a viscosity law (the message names `rheology`), CH with `model = "plugin"`, `plugin = "k_epsilon"`. Located at the key: `diffusivity = "harmonic"` | as stated |
| S2' | CHX k_epsilon: the settings content hash without the key equals the one with `owner`; with `pair` it differs | as stated |
| T4 | the solver's momentum face viscosity (`momentum_face_viscosity`) after `n` iterations or steps: CHX k_epsilon, spalart_allmaras, mixing_length (5), TGS smagorinsky, wale (3), rsm (5). `pair`: every seam pair has the same bits on both faces, equal to the bits of `nu + w nu_t,P + (1 - w) nu_t,Q` from the field `nut`; `owner`: the largest `abs(nu_f - nu_g) / max nu_f` over the pairs after the last iteration | exact, exact; `owner >= 1e-3`, rsm `>= 1e-5` |
| T4r | CHXR, 5 iterations: `pair` every seam pair the same bits; `owner` as T4 | exact; `>= 1e-3` |
| T5 | LAMX, 3 iterations: `pair` every seam face has the bits of `physics.nu`; `owner` the largest seam face viscosity `>= 1.001 nu` | exact; `>= 1.001 nu` |
| F2r | the viscosity law of CHXR (`Rheology::from_case`) on BOX, SEC, GBOX with the velocity of G (z component `0.3 z`): `nu_faces` in `owner` and in `pair` (a fresh law each); `pair`: every pair the same bits, equal to `(w mu_P + (1 - w) mu_Q) / rho` from `viscosity(strain_rate, T)` of the cells; the other faces the `owner` bits; a second `pair` call on the velocity `1.1 u` (relaxed against the first, `relax = 0.7`) keeps every pair at one value. Non-vacuity: `owner` pair difference `>= 1e-3` of the largest face viscosity | exact, exact, exact, exact; `>= 1e-3` |
| G2r | G of ADR 0205 G1 with the face viscosity of F2r | `pair <= 1e-13`; `owner >= 1e-5` |

The T4 limits are below the probe's owner values (item 9) by a factor 16 or more (rsm: 1.48e-4 against 1e-5).

### Default bits (V0)

| Id | Check | Limit |
|---|---|---|
| V0a | `baseline_hashes.rs` and `case_hashes_w48u.rs`, unedited | pass |
| V0b' | `crates/rcfd-api/tests/f296_default_bits.rs`: FNV-1a of the fields and the face flux (as `f295_default_bits.rs`), recorded on main 54167b0 with this text and the test only, committed before any code: CHX k_epsilon, spalart_allmaras, mixing_length, CHXR (50 iterations each), TGS smagorinsky (20 steps), wale (10), rsm (20), TGS laminar with the CHXR law but `consistency_index = 0.05` on `piso` (10 steps, the transient path of the law), LAMX (50), NPK (50) | bit-identical after the code |
| V0c' | after the code: every V0b' run with `diffusivity = "owner"` written; NPK with `pair` | V0c1': its V0b' record; V0c2': the NPK record |
| V0d | `f295_default_bits.rs` and `f286_default_bits.rs`, unedited | pass |
| V0e | the gate (fmt per package, clippy `--workspace --all-targets -D warnings`, every test in release on a git-archive copy, `scripts/gate_test.py`, 3 jobs, the gate lock) on the final commit | pass, apart from the known flakes |

### Mutations (scratch, not committed)

Each is applied alone to the final tree, the named test is run once, and it must fail. Their numbers go in the addendum.

| Id | Mutation | Must fail |
|---|---|---|
| MU1 | `KEpsilon::nu_face` does not apply the pairs | F2 (k_epsilon) |
| MU2 | the k-epsilon sweep does not pair `nut_f` | K1 |
| MU3 | `SpalartAllmaras::nu_face` does not apply the pairs | F2 (spalart_allmaras) |
| MU4 | the SA sweep does not pair the `nuTilda` diffusivity | A1 |
| MU5 | the RSM sweep does not pair the stress and epsilon diffusivities | Q1 |
| MU6 | the RSM pair value uses the swapped weight `dn_f / (dn_f + dn_g)` | Q1 (GBOX) |
| MU7 | the RSM momentum-source viscosity is not paired | M1 |
| MU8 | `WallInfo::nu_face` does not apply the pairs | F2 (smagorinsky) |
| MU9 | `Rheology::nu_faces` does not apply the pairs | F2r |
| MU10 | `Simulation::new` does not give the pairs to the viscosity law | T4r |
| MU11 | `KEpsilonModel::set_option` accepts `pair` without keeping the pairs | T4 (k_epsilon) |
| MU12 | R1' accepts `pair` on a laminar run without a viscosity law | S1' |

### Classes

As ADR 0205: V0 is a precondition of landing (a miss of V0a or V0b' means the change is not merged and the cause is recorded); a
miss of any other limit is reported with its number and kept as an ignored `*_known_miss` test with the limit intact; a negative
control (the owner halves of F2, G2, M1, K1, A1, Q1, T4, T4r, T5, F2r, G2r) or a mutation that does not make its check fail means
the check is not shown to be able to fail: it is reported as a defective set-up, corrected by an amendment before any re-run, and
never counted as a pass. A miss of a precondition (the harness of K1, A1, Q1) is a defective set-up of the check that uses it.

## Predictions (mine; moderate confidence)

| Id | Prediction |
|---|---|
| PR1 | F2 and F2r exact for every model and mesh; G2, G2r and M1 with `pair` near 1e-16, with `owner` the probe's values |
| PR2 | K1, A1 and Q1 with `pair` below 1e-14; their owner controls the probe's values (4.7e-3 to 5.7e-2, SA 1.4e-4 and 3.4e-4) |
| PR3 | A1p at round-off in both modes |
| PR4 | T4, T4r and T5 exact; V0 bit-identical |

## Budgets

Every gate test above takes well under 30 s in release (the probe: the library budgets 0.02 s together, every case-tree text
below 0.1 s for its iterations).

## Not in scope (and why)

- k-omega SST (F-295, unchanged), the lag of the periodic partner value and the convection on a seam (ADR 0205).
- The face values of item 8 (property viscosity, user-function viscosity, the RSM stress divergence and body force): not named by
  F-296 (question 1).
- k-epsilon and Spalart-Allmaras on `pimple`: F-294 is not on main. Its sweeps call the same diffusivity code, so after both
  merge they use the pair values without further code; a check of that combination is question 4.
- Check H, FAN-01 and the default (ADR 0205 questions 2 and 3, answered by the lead: the default stays `owner`).

## Open questions for the maintainer (each with a recommendation)

1. **The face values not named by F-296** (item 8). Options: (a) leave them; (b) a follow-up item that applies D1 under the same key
   to the property viscosity and the user-function viscosity; (c) also the RSM stress divergence on rotational seams, which needs
   the stress tensor rotated into the partner's frame (not a pair rule). Recommendation: (b) at low priority, and (c) only if the
   Reynolds stress model is ever run on a rotational seam (it has no wall treatment, so FAN-01 cannot use it).
2. **ADR 0205 S1 as an ignored record** (D4). Options: (a) as registered, S1 ignored with its reason and its cases in S1'; (b) edit
   the F-295 test to the new scope. Recommendation: (a): ADR 0205's test then still shows what was run for it.
3. **`pair` for the `nuTilda` diffusivity of SA**, which is already one value per pair (item 2). Options: (a) D1 as registered
   (the internal-face rule, which differs where `nuTilda < 0` on a seam cell); (b) leave `nuTilda` to its existing rule under both
   values of the key. Recommendation: (a): one rule for every diffusivity under the key, and the same formula as SA's internal faces.
4. **k-epsilon and SA on pimple with `pair`** after F-294 and F-296 are both merged. Options: (a) nothing more (the same code
   paths); (b) a T4-type case-tree check on pimple added when the branches are merged. Recommendation: (b), by the job that merges
   the second of the two.
5. **`pair` on a laminar run without a viscosity law** (R1'). Options: (a) refused, as registered (and as ADR 0205 R1); (b)
   accepted with no effect. Recommendation: (a): a refusal tells the user that nothing is paired.

## Addenda

Results of the checks follow in an addendum, in the order of the tables.
