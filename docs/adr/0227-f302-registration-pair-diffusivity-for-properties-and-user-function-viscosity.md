# ADR 0227: F-302 registration: one diffusivity per periodic seam pair for the property and user-function viscosities, and the pimple check

Status: **REGISTERED 2026-11-08**, written before any code of F-302 and before any run of its checks. Not yet accepted by the
maintainer. Before this text, one probe of the existing code was run on main 9f95fb12 (scratch tests, not committed): the face
values and global diffusion budgets of the property viscosity and of the user-function viscosity with the existing (owner) rule,
the owner halves of the case-tree checks below, the owner halves of the transient sweep budgets, and the static scope of the
texts. The `pair` halves of the pimple checks (code already on main, item 5) were not run. Its numbers are quoted in item 8,
where they set the limits of the negative controls. Results go in the addendum at the end.

Parents: [ADR 0212](0212-f296-registration-one-diffusivity-per-periodic-seam-pair-for-the-other-models.md) (F-296: the rule D1,
R1', the face values its item 8 left out; question 1, option (b), and question 4, option (b), recommendations taken by the lead),
[ADR 0205](0205-f295-registration-one-diffusivity-per-periodic-seam-pair.md) (F-295: the pairs, the key, the laminar rule across the
seam), [ADR 0210](0210-f294-registration-transient-k-epsilon-and-spalart-allmaras-with-pimple.md) (F-294: k-epsilon and
Spalart-Allmaras on `pimple`, its check S1), F-053 and ADR 0036 (the temperature-dependent properties), F-122 and ADR 0088 (the
user functions). Roadmap item F-302 (depends on F-296 and F-294). F-300 (branch `f300-transient-rans`, ADR 0225) is not merged and
nothing here depends on it.

Number: the lead gave 0227 to F-302. Branch `f302-pair-diffusivity` from main 9f95fb12, worktree `w49i-work/f302`.

House rules ([ADR 0130](0130-wave48a-overview-and-run-record-contract.md)), as in ADR 0212:

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the number recorded, and the cause is
  diagnosed, not tuned away;
- a defective set-up gets a new corrected test by an amendment written before the corrected run, and the old one stays as an
  ignored record;
- the new code path is opt-in and the defaults stay bit-identical: `crates/rcfd-api/tests/baseline_hashes.rs` and
  `case_hashes_w48u.rs` are not edited or re-recorded, no golden is edited or re-recorded, and no existing test is edited;
- each registered run is made once, and nothing is re-run to pick a result;
- no new third-party crate, `Cargo.lock` unchanged, no commit trailers.

Clean-room: the rule is the flux balance of a pair of finite-volume faces (ADR 0205 D1), applied to the face formulas that
`crates/rcfd-api/src/properties.rs` and the user-function viscosity of `crates/rcfd-api/src/sim.rs` already use on internal faces.
Nothing was read outside the repository. No vendor code, text or install was used.

## What F-302 asks

The roadmap entry: pair diffusivity covers the temperature-dependent properties viscosity and the user-function viscosity; a
case-tree check of k-epsilon and Spalart-Allmaras with pair on pimple. The note: from ADR 0212 questions 1 and 4, taken at the
recommendation; the Reynolds stress model on a rotational seam only if it ever runs there; the lead ignored the F-294 test
`s1_scope_of_transient_kepsilon_and_sa` after the F-296 merge (it asserted that `pair` is refused with k-epsilon), and its other
accept and refuse cases, no longer run, are to be restored in the pimple check.

## What the code read shows (main 9f95fb12)

1. **The rule and its plumbing** (ADR 0205, ADR 0212) hold unchanged: `FvGeometry::periodic_pairs` lists the pairs `(f, g)`,
   `f < g`, `w = dn_g / (dn_f + dn_g)` (an error when the maps are not a pairing); the models keep a `SeamPairs`; the viscosity
   law (`Rheology`) keeps the pairs given by `Simulation::new` and writes `(w mu_P + (1 - w) mu_Q) / rho` on both faces;
   `check_periodic_seam` (`turb.rs`) is R1' of ADR 0212.
2. **The property viscosity** (`properties.rs`, F-053). `Properties::nu_faces` gives every face `mu(T_f) / rho`, with
   `T_f = face_values(T)`: `w T_o + (1 - w) T_n` on an internal face and the boundary value of the temperature field on a boundary
   face. A periodic face has the periodic condition (`rcfd_fv::bc::Periodic`, the default of a cyclic patch; the case tree sets
   no temperature condition there), whose value is `a T_P + b` with `a = w_f = dn_g / (dn_f + dn_g)` and `b = (1 - w_f) T_Q`, the
   partner value read from the current field (no lag inside the evaluation). So face `f` carries `mu(w T_P + (1 - w) T_Q) / rho`,
   the D1 value of ADR 0212 with the same operations in the same order (the same bits), and face `g` carries
   `mu(w_g T_Q + (1 - w_g) T_P) / rho` with `w_g = dn_f / (dn_f + dn_g)`, the same number in exact arithmetic. **The property
   viscosity is therefore already one value per pair under `owner`, up to rounding** (a finding of this read, like the `nuTilda`
   diffusivity of SA in ADR 0212 item 2; the probe, item 8). It acts in steady SIMPLE/SIMPLEC runs with the energy equation and
   no turbulence model (`properties::check` refuses a turbulence model, a viscosity law and a user-function viscosity with it),
   and it is evaluated in the steady engine after the user-function viscosity and the viscosity law (`sim.rs`), on the temperature
   of the previous iteration. The thermal diffusivity `alpha_faces` uses the same face temperature, so it too is one value per
   pair up to rounding (not in scope).
3. **The user-function viscosity** (`sim.rs`, steady engine, F-122). Every face calls the module's `kinematic_viscosity(x_face, 0,
   temperature, speed)`, with `temperature = w T_o + (1 - w) T_n` and `speed = w |u_o| + (1 - w) |u_n|` on an internal face and the
   owner's own values on a boundary face (`w = 1`, `n = o`), periodic faces included; `temperature` is 0 without the energy
   equation, `x_face` is the face centre. So the two faces of a pair take each its own owner's speed and temperature and its own
   centre. It acts in steady SIMPLE/SIMPLEC laminar runs only (refused with a turbulence model, a viscosity law, a property
   viscosity, a transient, coupled, compressible, variable-density or VOF run). A module may keep state in its memory; calls are
   deterministic.
4. **R1' refuses both today**: `pair` on a laminar run is accepted only with a viscosity law (`rheology::enabled`), so a laminar run
   with a property viscosity law or a user-function viscosity is a located error at the key ("a laminar run without a viscosity
   law (rheology.model or rheology.shift) has no diffusivity to pair"). No turbulence state is built for a laminar run.
5. **The pimple path of k-epsilon and SA with `pair`** (F-294 and F-296 both merged). `Simulation::step` starts the step
   (`begin_time_step`) and the `step_with` hook makes one sweep at the start of every outer iteration, then hands the solver
   `TurbState::nu_face` (the laminar rule across the seam included). `KEpsilon::update` pairs the face eddy viscosity `nut_f` of the
   k and epsilon diffusivities before the branch between the time derivative (transient) and the relaxation (steady); the SA sweep
   pairs the `nuTilda` diffusivity before its time branches; both `nu_face` apply the pairs. R1' does not look at the algorithm, so
   `pair` with k-epsilon or SA on `pimple` is accepted and runs through code already on main. No check runs that combination:
   ADR 0212 T4 ran these models on SIMPLEC only, its sweep budgets K1 and A1 on the steady relaxation and the SA-DES `begin_step`
   path, and ADR 0210 S1 asserted the refusal.
6. **The ignored F-294 test.** `s1_scope_of_transient_kepsilon_and_sa` (`crates/rcfd-api/tests/f294_transient_keps_sa.rs`) carries
   `#[ignore = "superseded by ADR 0212 R1' ..."]` since the F-296 merge. Its cases are the scope of ADR 0210 S1 and amendment 1:
   CH-K and CH-A on `pimple` with every time scheme, the k-epsilon variants, wall functions and treatment, the SA wall modes,
   limiter and negative branch, SA-DDES on `pimple` and `piso`, the two S4 sectors; the refusals with `piso` (naming `pimple` and
   `steady`), the energy equation, a dynamic mesh, `error_tolerance`, the fractional step, the moving wall of amendment 1; the
   `pair` refusal; and the dispatch answers `updates_in_outer_iterations`. Only the `pair` case is superseded.
7. **Written text and hashes.** The key is the optional object of ADR 0205 D3 with `hash_omit_default`; F-302 adds no key and
   changes no object, so every settings value, written text and hash stays as it is. Only the documentation text of the key
   changes (not part of the settings data or of any hash; no test compares it).
8. **The probe** (main 9f95fb12, existing code, release, scratch tests, not committed; the `pair` halves not run).
   - Property viscosity, library: the law of CHXP below (`mu = -0.005 + 2.5e-5 T` Pa s, `rho = 1`) on BOX, SEC and GBOX (ADR 0212)
     with `T = 300 + 20 sin(2 pi s) + 5 y` (SEC: `+ 5 z`): pairs with unequal bits 0 of 4 (BOX), 0 of 24 (SEC), 1 of 4 (GBOX, 4
     ulps); largest pair difference 0, 0, 5.6e-16 of the largest face viscosity; G of ADR 0205 G1 1.3e-16, 1.3e-16, 4.4e-17 (BOX:
     q, u_x, u_y), 4.0e-17 (SEC), 1.1e-16, 1.2e-16, 8.5e-17 (GBOX).
   - User-function viscosity, library, the loop of item 3 copied into the test: module UT (`nu = 0.0025 + 1e-4 speed + 1e-7
     temperature`) with the velocity of ADR 0212 F2r and the T field above: pairs with unequal bits 4 of 4, 24 of 24, 4 of 4;
     largest pair difference 6.67e-3, 7.94e-3, 3.30e-3 of the largest face viscosity; G 4.98e-4, 5.05e-4, 2.79e-4 (BOX),
     1.27e-3 (SEC), 1.61e-4, 1.63e-4, 1.23e-4 (GBOX). Module UX (`nu = 0.0025 + 1e-4 speed + 1e-4 x`) on BOX: 3.34e-2; G 2.93e-3,
     2.97e-3, 1.64e-3.
   - Case tree, `owner`: CHXP after 5 SIMPLEC iterations 0 of 20 pairs with unequal bits; its T 296.0 to 304.0 K after 50
     iterations (0.06 s). GCHXP (CHXP with `mesh.grading = 3`, whose pair weights are 0.5 to within a few ulps, for example
     0.5000000000000004 and 0.49999999999999956) after 5: 2 of 20 pairs unequal, largest relative difference 6.9e-16. CHXU after 5:
     20 of 20 unequal, 9.12e-3. CHX k-epsilon on `pimple` after 3 steps: 7.18e-3 (`euler`), 5.15e-3 (`bdf2`); SA 1.60e-2, 1.16e-2;
     10 steps each without a failure (0.04 to 0.2 s). LAMX on `pimple`: the largest seam face viscosity 2.726e-2 = 10.77 `nu`.
   - Static scope: CHXP and CHXU with `pair` located at the key with the R1' message; CHX k-epsilon and SA on `pimple` with `pair`
     have no static diagnostic.
   - Transient sweeps, `owner`, the set-ups K1t and A1t below: harness `B(L_owner)` 3.0e-15 (K1t BOX), 1.6e-15 (K1t GBOX),
     9.5e-16 (A1t BOX), 4.8e-16 (A1t GBOX); control `B(L_D1)` 1.91e-2, 3.46e-2, 1.40e-4, 3.39e-4; step change 0.61, 0.62, 0.118,
     0.121; smallest k after the sweep 0.380, 0.378; A1t has 4 (BOX) and 8 (GBOX) seam owner cells with `nuTilda < 0`.

## Decisions (fixed now)

**D1. The rule.** As ADR 0212 D1: for every pair `(f, g)`, `P` the owner of `f`, `Q` the owner of `g`, both faces take one value,
computed once per pair and written on both, **the formula of the internal faces with the pair's weight `w` and the cells `P`, `Q`
in place of the internal weight, owner and neighbour**:

| Face value | Internal face | Pair value (both faces) |
|---|---|---|
| property viscosity | `mu(w T_o + (1 - w) T_n) / rho` | `mu(w T_P + (1 - w) T_Q) / rho` |
| user-function viscosity | `udf(x_face, 0, w T_o + (1 - w) T_n, w abs(u_o) + (1 - w) abs(u_n))` | `udf(x_f, 0, w T_P + (1 - w) T_Q, w abs(u_P) + (1 - w) abs(u_Q))` |

`x_f` is the centre of face `f`, the face of the lower global index (question 1); the module is called once per pair, at face
`f`, and not at face `g` (question 5). `T` is the cell temperature of the energy equation, 0 without it, as in item 3. The inputs are
a scalar and a speed, so a rotational seam needs nothing more. On a uniform seam the property value is the one each face already
has (item 2); on a graded seam it removes the rounding difference.

**D2. Where it acts.** The steady laminar runs of items 2 and 3, translational and rotational seams. Unchanged: the thermal
diffusivity and the user-function conductivity of the energy equation (question 2), the convection, the lag, every model of ADR
0205 and ADR 0212, and every result with `owner`.

**D3. The key.** Unchanged: `turbulence.periodic_seam.diffusivity`, the optional object of ADR 0205 D3. Only its documentation
text changes (item 7).

**D4. Scope and refusals.**

- R1'' (replaces the laminar part of ADR 0212 R1'): `pair` with `turbulence.model = "laminar"` is accepted when a viscosity law
  (`rheology::enabled`), a temperature-dependent property viscosity (`properties.viscosity.law` other than `constant`) or a
  user-function viscosity (`udf.<name>.viscosity = true`) is set, and is a static located error at the key otherwise; the
  message names the three. The other models: as R1' (accepted with the built-in models, refused with `plugin`).
- R3: for the property viscosity and the user-function viscosity `Simulation::new` reads `periodic_pairs` and locates an error at
  the key, as for the viscosity law. `owner` never reads the pairs.
- `pair` on a mesh without periodic patches changes nothing. A live `settings.set` of the key on a laminar run is refused by the
  existing live path (rebuild the simulation), which F-302 does not change.
- ADR 0210 S1 (item 6) stays the ignored record the lead made it; its cases, with the `pair` case accepted, are the check S1t
  below. No existing test is edited.

**D5. Code (the plan, with the files).**

- `crates/rcfd-api/src/properties.rs`: the pairs of the law (`Properties::set_seam_pairs`), applied in `nu_faces` after the face
  values (D1); `viscosity_enabled(case)` (a property viscosity law is set).
- `crates/rcfd-api/src/udf.rs`: `face_viscosity(udf, geom, u, temp, pairs)`, the face loop of item 3 moved out of `sim.rs` with the
  same operations for every face that is not a seam face (and for every face without pairs), and D1 on the seam faces.
- `crates/rcfd-api/src/sim.rs`: in `Simulation::new`, the pairs given to the property law and kept for the user-function viscosity
  for `pair` (R3 at the key); the steady engine calls `udf::face_viscosity`.
- `crates/rcfd-api/src/turb.rs`: R1'' in `check_periodic_seam`.
- `crates/rcfd-api/src/schema.rs`: the documentation text of the key. `docs/case-schema.md` and `docs/udf-abi.md`: prose (no TOML
  block added or changed).

**D6. Why the defaults stay bit-identical.** Without the key (or with `owner`) neither the property law nor the user-function path
holds pairs, so every face value is computed by the existing expression in the existing order (the moved loop included) and the
new code only tests for the absent pairs; no key or object changes, so data, written text and hashes are unchanged. The pimple
checks add no code. V0 checks this.

## Checks and limits (fixed now)

Every check is run once, in release. All but the report lines are gate tests.

Texts (`crates/rcfd-api/tests/f302_common/mod.rs`), built on the CHX channel of ADR 0212 (`f296_common`: `box_2d` 4 x 20, periodic
in x, drive `(1, 0, 0)`, `initial.velocity_expression_x = 15 + 3 sin(2 pi x)`, upwind, SIMPLEC with `max_iterations = 1000`):

- **CHXP**: CHX laminar with `[energy] enabled = true`, `buoyancy = false`, `physics.conductivity = 0.05`, `physics.cp = 1`,
  `initial.temperature = 300`, `ymin` a fixed temperature `300 [K] + 20 [K] * sin(6.283185307179586 * x / 1 [m])`, `ymax` a fixed
  300 K, and `[properties.viscosity] law = "polynomial"`, `coefficients = [-0.005, 2.5e-5]`. **GCHXP**: CHXP with `mesh.grading =
  3.0` (item 8).
- **CHXU**: CHX laminar with `[udf.v] file = "v.wasm"`, `viscosity = true`, the module UT of item 8 (written from WAT into a
  directory of the test). **CHXUT**: CHXU with the energy lines of CHXP (no property law), so the temperature input is not 0.
- **CHXK, CHXA**: CHX with `k_epsilon` / `spalart_allmaras` (the model lines of ADR 0212) and the solver lines replaced by
  `algorithm = "pimple"`, `time_scheme` (`euler` or `bdf2`), `n_outer = 3`, `dt = 0.1`, `end_time = 100`. **LAMXP**: LAMX of ADR
  0212 (CHX k-epsilon with the laminar region of the first column) with the same pimple lines (`euler`).
- The CH-K, CH-A and S4 texts of ADR 0210, copied from `f294_transient_keps_sa.rs` (unchanged) for S1t.

Library set-ups: the meshes BOX, SEC, GBOX of ADR 0212, the T field and the velocity of item 8, the modules UT and UX of item 8.

### Library checks of the property and user-function viscosities (`crates/rcfd-api/tests/f302_seam_diffusivity.rs`)

| Id | Check | Limit |
|---|---|---|
| F2p | the law of CHXP (`Properties::from_case`) on BOX, SEC, GBOX: `nu_faces` without pairs (`owner`) and with `set_seam_pairs(periodic_pairs)` (`pair`). `pair`: every pair has the same bits on both faces, equal to the bits of `mu(w T_P + (1 - w) T_Q) / rho` computed in the test with the law (`PropertyLaw::eval`); every face that is not a seam face has the `owner` bits. `owner`, the finding of item 2: the largest pair difference of the largest face viscosity (the number of pairs with unequal bits reported) | exact, exact, exact; `owner <= 1e-14` |
| G2p | G of ADR 0205 G1 with the face values of F2p, fields `q` (every mesh) and `u_x`, `u_y` (BOX, GBOX) | `pair <= 1e-13`; `owner <= 1e-13` (no negative control is possible: item 2) |
| F2u | module UT, `udf::face_viscosity` on BOX, SEC, GBOX with the T field and the velocity of item 8, without and with the pairs. `pair`: every pair the same bits, equal to the bits of the module's value at `x_f` with the D1 temperature and speed (the module called in the test); every other face the `owner` bits. Non-vacuity: `owner` pair difference of the largest face viscosity | exact, exact, exact; `>= 1e-3` |
| G2u | G with the face values of F2u | `pair <= 1e-13`; `owner >= 1e-5` |
| F2x | module UX on BOX, as F2u: `pair` every pair the same bits, equal to the module's value at `x_f` (D1); the module's value at `x_g` with the same inputs differs from it by `>= 1e-3` of it (so the face is seen); `owner` pair difference | exact; `>= 1e-3`; `>= 1e-3` |

### Case-tree checks (`crates/rcfd-api/tests/f302_seam_diffusivity.rs`)

| Id | Check | Limit |
|---|---|---|
| S1p | scope of the laminar runs. Accepted with no diagnostic (`parse_toml_in`, `semantic_errors`, `Simulation::new`) with `pair`: CHXP, GCHXP, CHXU, CHXUT, and CHXR of ADR 0212; with `owner`: CHXP, CHXU. Located statically at the key with `pair`: CH laminar of ADR 0205 without any law (the message names `rheology`, `properties.viscosity` and `udf`); CHXP with `law = "constant"`; CHXP with the viscosity law replaced by a conductivity law (`[properties.conductivity] law = "polynomial"`, `coefficients = [0.05]`); CH with `model = "plugin"` (the message names `plugin`) | as stated |
| S1t | ADR 0210 S1 and its amendment 1 restored: every case of `s1_scope_of_transient_kepsilon_and_sa` as registered there (item 6), through the same `paths` / `messages` functions, with one change: CH-K and CH-A on `pimple` with `diffusivity = "pair"` are accepted with no diagnostic (R1' of ADR 0212), in place of located at the key; the dispatch answers of the registry as there | as stated |
| T4p | GCHXP, 5 iterations, the solver's momentum face viscosity (`momentum_face_viscosity`). `pair`: every seam pair has the same bits on both faces, equal to the bits of `mu(w T_P + (1 - w) T_Q) / rho` from the temperature after iteration 4 (the field the iteration reads, item 2). `owner`: the largest relative pair difference (the number of pairs with unequal bits reported) | exact, exact; `owner <= 1e-14` |
| T4u | CHXU, 5 iterations. `pair`: every seam pair the same bits, equal to the bits of the module's value at `x_f` with the D1 speed from the velocity after iteration 4. `owner`: the largest relative pair difference | exact, exact; `>= 1e-3` |
| T4k | CHXK and CHXA with `euler` and with `bdf2`, 3 steps. `pair`: every seam pair the same bits, equal to the bits of `nu + w nu_t,P + (1 - w) nu_t,Q` from the field `nut` after the step; `owner`: the largest relative pair difference | exact, exact; `>= 1e-3` |
| T5k | LAMXP, 3 steps: `pair` every seam face has the bits of `physics.nu` (the laminar rule across the seam, ADR 0205 D2); `owner` the largest seam face viscosity | exact; `>= 1.001 nu` |
| R0p | report only: CHXP with `pair`, 50 iterations, the FNV-1a record of `f296_common::record` against the CHXP record of V0b'' | reported (prediction PR5: equal) |

The T4u and T4k limits are below the probe's owner values (item 8) by a factor 5 or more.

### Library checks of the transient sweeps (`crates/rcfd-turb/tests/f302_transient_seam.rs`)

The sweep budgets of ADR 0212 (the meshes BOX and GBOX, the frozen velocity, zero face flux, the solver controls, the lag term `L`,
the budget `B(G)`, `L_owner` and `L_D1`) on the transient branches that `pimple` runs (item 5):

- **K1t**: the K1 set-up of ADR 0212 (`KEpsilon` standard, walls `ymin`, `ymax`, the k, epsilon and `nut` fields) with
  `begin_step(0.05, euler)` before the sweep (the relaxation factors are then not used). Cell terms `r = P V - (e / k*) V k' -
  V (k' - k^n) / dt` (`k^n = k*`, the first step).
- **A1t**: the A1 set-up of ADR 0212 (`nuTilda = 0.1 (sin(2 pi x) - 0.1)`, wall mode `resolved`) with `begin_time_step(0.05,
  euler)` in place of `begin_step(0.05)`. Cell terms those of A1 (the time term `V (x' - x*) / dt`).

| Id | Limit |
|---|---|
| K1t, A1t | `pair`: `B(L_D1) <= 1e-12` on BOX and GBOX. Preconditions (a miss is a defective set-up): `is_transient()` after the start of the step; in the `owner` run `B(L_owner) <= 1e-12`; A1t at least one seam owner cell with `nuTilda < 0`. Negative control: in the `owner` run `B(L_D1) >= 1e-6`. Non-vacuity: `max abs(x' - x*) >= 1e-3`; K1t also `min k' > 1e-10` |

The negative-control limit is below the probe's smallest owner value (1.40e-4) by more than a factor 100; the precondition is above
the probe's largest harness value (3.0e-15) by more than a factor 100.

### Default bits (V0)

| Id | Check | Limit |
|---|---|---|
| V0a | `baseline_hashes.rs` and `case_hashes_w48u.rs`, unedited | pass |
| V0b'' | `crates/rcfd-api/tests/f302_default_bits.rs`: FNV-1a of the fields and the face flux (as `f296_common::record`, with the case directory of the module and the field `T` added to the fields hashed), recorded on main 9f95fb12 with this text and the test only, committed before any code: CHXP, GCHXP, CHXU, CHXUT (50 iterations each), CHXK and CHXA with `euler` and with `bdf2`, LAMXP (10 steps each) | bit-identical after the code |
| V0c'' | after the code: every V0b'' run with `diffusivity = "owner"` written | its V0b'' record |
| V0d | `f296_default_bits.rs`, `f295_default_bits.rs`, `f294_default_bits.rs`, `f286_default_bits.rs` and `udf.rs`, unedited | pass |
| V0e | the gate (fmt per package, clippy `--workspace --all-targets -D warnings`, every test in release on a git-archive copy, `scripts/gate_test.py`, 3 jobs, the gate lock) on the final commit | pass, apart from the known flakes |

### Mutations (scratch, not committed)

Each is applied alone to the final tree, the named test is run once, and it must fail. Their numbers go in the addendum.

| Id | Mutation | Must fail |
|---|---|---|
| MU1 | `Properties::nu_faces` does not apply the pairs | F2p (GBOX) |
| MU2 | `Simulation::new` does not give the pairs to the property law | T4p |
| MU3 | `udf::face_viscosity` does not apply the pairs | F2u |
| MU4 | `Simulation::new` does not keep the pairs for the user-function viscosity | T4u |
| MU5 | the user-function pair value is taken at the centre of face `g` | F2x |
| MU6 | the user-function pair value uses the swapped weight `dn_f / (dn_f + dn_g)` | F2u (GBOX) |
| MU7 | R1'' accepts `pair` on a laminar run without any law | S1p |
| MU8 | the k-epsilon sweep does not pair `nut_f` when the step is transient | K1t |
| MU9 | the SA sweep does not pair the `nuTilda` diffusivity when the step is transient | A1t |
| MU10 | the pimple hook hands the solver the model's `nu_face` in place of `TurbState::nu_face` | T5k |
| MU11 | `KEpsilon::nu_face` does not apply the pairs | T4k (CHXK) |
| MU12 | `SpalartAllmaras::nu_face` does not apply the pairs | T4k (CHXA) |

MU1 and MU2 can fail their checks only where the owner values differ in their last bits (GBOX: 1 pair; GCHXP: 2 pairs in the
probe); that is why F2p includes GBOX and T4p runs on GCHXP.

### Classes

As ADR 0212: V0 is a precondition of landing (a miss of V0a or V0b'' means the change is not merged and the cause is recorded); a
miss of any other limit is reported with its number and kept as an ignored `*_known_miss` test with the limit intact; a negative
control (the owner halves of F2u, G2u, F2x, T4u, T4k, T5k, K1t, A1t) or a mutation that does not make its check fail means the check
is not shown to be able to fail: it is reported as a defective set-up, corrected by an amendment before any re-run, and never
counted as a pass. A miss of a precondition is a defective set-up of the check that uses it. The owner halves of F2p, G2p and T4p are
not negative controls but the finding of item 2: a miss is reported as a finding that contradicts the code read.

## Predictions (mine; moderate confidence)

| Id | Prediction |
|---|---|
| PR1 | F2p, F2u, F2x exact; G2p, G2u with `pair` near 1e-16; the owner values those of the probe (F2p: 0, 0, 5.6e-16) |
| PR2 | T4p, T4u, T4k, T5k exact, with the probe's owner values |
| PR3 | K1t and A1t with `pair` below 1e-14, the owner controls those of the probe |
| PR4 | S1t: every case of ADR 0210 S1 as recorded there, the `pair` case accepted |
| PR5 | R0p: the CHXP record with `pair` equals the owner record (item 2 on a uniform seam) |
| PR6 | V0 bit-identical |

## Budgets

Every gate test above takes well under 30 s in release (the probe: every text below 0.4 s for its iterations, the library checks
below 0.1 s).

## Not in scope (and why)

- The energy-equation diffusivity on a seam: the thermal diffusivity of the property laws (one value per pair up to rounding, item
  2), the user-function conductivity (each face its own centre and face temperature) and the constant conductivity (question 2).
- The Reynolds stress model on a rotational seam (ADR 0212 question 1 (c); the note: only if it ever runs there).
- F-300 (not merged), check H and FAN-01, and the default (`owner`, ADR 0205 question 3).

## Open questions for the maintainer (each with a recommendation)

1. **The position of a user-function viscosity on a pair.** The two faces of a pair are one surface at two centres. Options: (a)
   the centre of face `f`, the face of the lower index, as registered; (b) the weighted mean `w udf(x_f, ..) + (1 - w) udf(x_g, ..)`
   of two calls; (c) the centre of the owner side with the larger weight. Recommendation: (a): a periodic case needs a function that
   is periodic in position, for which the three agree; (a) is the internal-face formula with one call, and the docs say so.
2. **The energy-equation diffusivity on a seam** (not in scope). Options: (a) leave it (the property laws are already one value per
   pair up to rounding; only the user-function conductivity differs); (b) a follow-up that applies D1 to the user-function
   conductivity under the same key or an energy key. Recommendation: (a) until an energy case on a seam needs it.
3. **`pair` for the property viscosity, which changes only rounding** (item 2). Options: (a) D1 under `pair`, as registered (one
   rule for every face value under the key, exact on graded seams); (b) leave the property viscosity out of `pair`.
   Recommendation: (a), as ADR 0212 question 3 for the `nuTilda` diffusivity.
4. **The ignored F-294 S1.** Options: (a) leave it ignored with the lead's reason and run its cases in S1t, as registered; (b) edit
   the F-294 test back to live with the `pair` case accepted. Recommendation: (a): no existing test is edited and the F-294 record
   still shows what was run for ADR 0210.
5. **One module call per pair.** Under `pair` the module is not called at face `g`, so a module that keeps state between calls sees
   fewer calls than with `owner`. Options: (a) as registered; (b) call it at both faces and keep the value at `f`. Recommendation:
   (a): the ABI documents deterministic calls of the face values, and the docs will say that `pair` calls once per pair.

## Addenda

Results of the checks follow in an addendum, in the order of the tables.
