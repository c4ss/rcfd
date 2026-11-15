# ADR 0555: F-620 registration: hyperelastic materials on the nonlinear transient Method step and the linear paths

Status: **REGISTERED 2026-11-15, written before any code of F-620 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-620 was written, compiled or run in the repository: the files listed under "What was
read" were read and the arithmetic below was done by hand (no script, no probe, no rcfd code). The bit record B0 of the default
paths is a test file written and run on the base 89bdd903 after this commit and before any library line (D8). Results go in the
addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-620 (dep F-425), structural solver, advanced tier, the follow-up named by
[ADR 0541](0541-f425-registration-hyperelastic-materials-with-the-mean-dilatation-hex8.md) (its open question 2 and remaining
question 2: the Method step and the linear paths, a linearised material from `small_strain_moduli`, the Method step together
with F-611). Number 0555 given by the lead. Branch `f620-work` from 89bdd903, worktree `w49i-work/f620`, scratch
`w49i-work/f620-scratch`. F-611 ([ADR 0544](0544-f611-registration-energy-momentum-step-with-f425-hyperelastic-materials.md))
has landed `AlgorithmicStress::MaterialDiscreteGradient` (the energy-momentum step with F-425's point functions); it is not
duplicated or changed here.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no
unsafe code, the crate graph unchanged; clean-room. Every choice the maintainer would normally be asked about is taken on the
maintainer's standing instruction of 2026-11-11 (take the recommendation), except the open questions at the end.

## What F-620 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. Hyperelastic materials on the nonlinear transient Method
> step and the linear paths (the linearised material of the small-strain moduli), the Method step together with F-611.

Clauses: **c0** every limit registered here; **c1** the nonlinear transient `Method` step (`StepKind::Method`, Newmark or
generalised-alpha, ADR 0479) takes `Material::Hyperelastic` (neo-Hookean and Mooney-Rivlin, ADR 0541 D1); **c2** the linear
paths (static, modal, damped modes, direct and modal harmonic, linear transient, buckling and pre-stress, the recovered stress)
take it as the linearised material, the isotropic elasticity of its small-strain moduli `(G, K) = mat_hyper::small_strain_moduli`;
**c3** "together with F-611": the Method step uses the same point functions and the same strain energy (`mat_hyper::energy`) as
F-611's `MaterialDiscreteGradient`, so that the two steps agree at a state and converge to the same motion.

## What was read (base 89bdd903, read only, nothing run)

`docs/roadmap.json` (F-620, F-425, F-611); ADR 0541 (what F-425 asks, D1 to D9, the checks, the mutations, the addendum, the
open and remaining questions); ADR 0544 (whole up to its addendum: D1 to D7, the checks, the mutations, the decision rule, the
limits); ADR 0514 (the checks N1e, L1e and their measured values); ADR 0523 (lines that name the Method step). Source:
`mat_hyper.rs` (the public functions), `material.rs` (`elasticity_matrix`, `elasticity_at`, `stress`, `isotropic_elasticity`),
`model.rs` (`check_elements` and its callers, `youngs_modulus`, `poisson_ratio`, `expansion`), `nl_transient.rs`
(`NlTransientOptions`, `Integrator::new`, `start`, `damping_at`, `strain_energy`), `nl_transient/dg.rs` (`surrogate`),
`assemble.rs`, `damped.rs`, `estimate.rs`, `harmonic_direct.rs`, `hex8x.rs` (`element_data`), `mat_ortho.rs`, `thermal.rs`,
`recover.rs` (`IntegrationPointField::compute`), `shell.rs` and `beam.rs` (`moduli`), `nonlinear.rs` (`thermal_point_update`),
`transient.rs` (`TransientOptions`, `Integrator`), the call sites of `elasticity_matrix`, `elasticity_at` and `Material::stress`
in the crate. Tests: `sa_hyper.rs` (`rf2_the_paths`), `sa_dg.rs` (`f1h_refusals`), `sa_dgm.rs` (the set-ups and `f1m_refusals`),
`sa_emc.rs` (F1ec), `sa_laminate.rs` (RF2), `sa_scaffold.rs` (`m1_to_m4_after_f425`), `sa_hyper_default_bits.rs`,
`sa_laminate.rs` and `sa_cms.rs` (how shell models and direct harmonic solves are built).

## What the base does (read)

1. **The linear paths.** `SolidModel::check_elements(.., history = false)` refuses `Hyperelastic` (`NotYetSupported`, package
   `mat_hyper::FOLLOW_UP`, ADR 0541 D3). Behind the refusal every linear element matrix takes its `D` from
   `Material::elasticity_matrix()` or `elasticity_at(point)` (`assemble.rs` 238, `damped.rs` 716 and 974, `estimate.rs` 557,
   `harmonic_direct.rs` 483 and 647, `hex8x.rs` 128 and 134, `thermal.rs` 75; `mat_ortho.rs` 346 and 389 only for the per-point
   materials), and `elasticity_matrix` of `Hyperelastic` is refused (ADR 0541 D2). The recovered stress
   (`recover.rs`, used by the static result's stresses, the pre-stress of buckling and pre-stressed modes, the harmonic and
   random stresses) calls `Material::stress`, which for `Hyperelastic` is `mat_hyper::stress_and_tangent`: the nonlinear stress
   of the small strain, not `D eps`. Beam2 and Shell elements take `youngs_modulus()` and `poisson_ratio()`, which are `NaN` for
   `Hyperelastic`; the thermal load takes `expansion()`, also `NaN`.
2. **The Method step.** `nl_transient::Integrator::new` refuses `Hyperelastic` on a step that is not the energy-momentum step
   (ADR 0541 D4). Behind it the Method step takes the material through the nonlinear driver's functions (`nonlinear::tangent`,
   `internal_force`, which call `Material::stress`, so `mat_hyper::stress_and_tangent`), with no state (`state_size` 0), and its
   Rayleigh stiffness part is `nonlinear::tangent` at `u = 0`. `Integrator::strain_energy` takes, for `Hyperelastic`,
   `mat_hyper::energy` when the stress option is `MaterialDiscreteGradient` and the step-local neo-Hookean of ADR 0523
   (`dg::energy`, which refuses Mooney-Rivlin) otherwise; on the Method step the option is not read by the step and defaults to
   `Auto`, so without a change the Method step's energy would be the step-local one.
3. **Existing tests that assert the refusals lifted here** (they fail by design after D3 and D4): `sa_hyper.rs::rf2_the_paths`
   (rows "solve_static refused", "modal_analysis refused", "nl_transient Method step refused"), `sa_dg.rs::f1h_refusals` (row "NH
   with the Method step refused") and `sa_dgm.rs::f1m_refusals` (row "NH with the Method step and MaterialDiscreteGradient
   refused"). `sa_scaffold.rs::m1_to_m4_after_f425` (the element-constant matrix of `Hyperelastic` refused) and `sa_hyper.rs`
   RF2's `elasticity_matrix` row stay true by the design (D1 keeps `elasticity_matrix` as it is). `sa_emc.rs` F1ec (the
   energy-momentum step with `Auto` refuses NH) and `sa_laminate.rs` RF2 (a hyperelastic ply refused) are not touched by the
   design.

## Design

### D1. The linearised material (`mat_hyper.rs`, `material.rs`, additive)

The linearised hyperelastic material is the isotropic elasticity of its small-strain moduli. From first principles (and ADR
0541 D1, arithmetic 1): at `E = 0` the tangent `dS/dE` of `W = c10 (Ibar1 - 3) + c01 (Ibar2 - 3) + (kappa/2)(J - 1)^2` is
`2G (II - (1/3) 1 x 1) + K 1 x 1` with `G = 2 (c10 + c01)` and `K = kappa`: the isochoric invariants are stationary under a pure
dilatation and quadratic in the deviator with the coefficient `G` (`Ibar1 - 3 = 2 tr(dev E)^2 + O(E^3)`, `Ibar2 - 3` the same
to second order), and `(kappa/2)(J - 1)^2 = (kappa/2)(tr E)^2 + O(E^3)`. In Lame form `lambda = K - 2G/3`, `mu = G`. (The same
statement is in Holzapfel, Nonlinear Solid Mechanics, 2000, and Bonet and Wood, Nonlinear Continuum Mechanics for Finite Element
Analysis, the compressible neo-Hookean; Ogden 1984 for the Mooney-Rivlin form; **all cited from memory, not read**; the
derivation above is what is used.)

- `mat_hyper::linearised_elasticity(energy) -> Matrix6`: `lambda = K - 2G/3`, `D_ij = lambda + 2G delta_ij` (`i, j < 3`),
  `D_kk = G` (`k >= 3`, engineering shear), zero otherwise. Computed from `(G, K)` directly (not through `E`, `nu`): at
  `K/G = 1e4` the route through `nu = 0.49995` loses `eps/(1 - 2 nu) ~ 2e-12` relative in `lambda` (arithmetic 1).
- `mat_hyper::LINEAR_FOLLOW_UP = "a follow-up of ADR 0555"`: the package named by the refusals of D3.
- `Material::linear_elasticity_matrix(&self) -> Result<Matrix6>`: for `Hyperelastic` `mat_hyper::linearised_elasticity`,
  otherwise `self.elasticity_matrix()` (the same call, the same bits).
- `Material::linear_elasticity_at(&self, point) -> Result<Matrix6>`: for `Hyperelastic` the same matrix, otherwise
  `self.elasticity_at(point)`.
- `Material::linear_stress(&self, strain, point, state) -> Result<([f64; 6], Matrix6)>`: for `Hyperelastic` `(apply(D, strain),
  D)` with `D` of `linearised_elasticity` (after the state-length check of `stress`), otherwise `self.stress(strain, point,
  state)`.

`Material::elasticity_matrix`, `elasticity_at` and `stress` are not changed: the hyperelastic material has no element-constant
matrix (ADR 0541 D2 stays, and with it `m1_to_m4_after_f425` and RF2's `elasticity_matrix` row); its constitutive call stays the
nonlinear one (the nonlinear driver, the Method step and F-611 keep their bits).

### D2. The linear call sites (shared solver files, delegation only)

Each linear call site of D's listed in "What the base does" item 1 calls the `linear_` form instead: `assemble.rs` 238,
`damped.rs` 716 and 974, `estimate.rs` 557, `harmonic_direct.rs` 483 and 647, `hex8x.rs` 128 (`linear_elasticity_at`) and 134;
`recover.rs` 134, 136, 145 and 151 (`linear_stress`). `thermal.rs` 75 and `mat_ortho.rs` are not changed (the thermal load is
refused for the material by D3; `mat_ortho` is reached only by the per-point materials). For every other material the `linear_`
form is the call it replaces, so every default path keeps its bits (B0, B1, B2).

### D3. `model.rs`: the paths

In `check_elements`, the block "ADR 0541 D3: and the hyperelastic material" (refusal when `history = false`) is replaced by, when
`history = false` and the material is `Hyperelastic`:

- a refusal when an element of the material's set is a Beam2, Shell4 or Shell3 element (those take `E` and `nu` of the
  material, `NaN` here): `NotYetSupported { item: "material", variant: "Hyperelastic", location: "material i, element set 's':
  a structural element (Beam2, Shell4, Shell3)", package: mat_hyper::LINEAR_FOLLOW_UP }`;
- a refusal when the model has a `Load::Temperature` (no expansion coefficient; the linear thermal load would be `NaN`): the
  same with the location "material i, element set 's': a temperature load".

Otherwise the material passes, and the paths take it through D1 and D2. The paths that refuse a non-isotropic material by their
own check (the rotor path, `rotor.rs` 447; the corotated shells; the energy-momentum step with `Auto`, `emc.rs` 47; fracture's
`(E, nu)`; laminate plies) keep refusing it, unchanged. A temperature field of the options is refused for this material by the
existing finite-expansion checks (`thermal.rs` 361, key `"expansion"`), unchanged. The pending check, J2, viscoelastic, creep and
per-point branches of `check_elements` are not changed and keep their order before and after this block.

### D4. `nl_transient.rs`: the Method step (c1, c3)

- `Integrator::new`: the block "ADR 0541 D4: the Method step does not take the hyperelastic material" is removed. The Method
  step then takes the material through the driver's functions (item 2 of "What the base does"), unchanged code.
- `strain_energy`: for `Hyperelastic` the energy is `mat_hyper::energy` when the stress option is `MaterialDiscreteGradient`
  **or the step is `StepKind::Method`**; the step-local `dg::energy` stays for the energy-momentum step with `DiscreteGradient`
  and `Average` (ADR 0544 D1, their bits unchanged). On the Method step this is F-611's energy (c3) and takes Mooney-Rivlin.
- `dg.rs`, `emc.rs` and the energy-momentum branches are not changed.

### D5. Records of the existing tests that assert a lifted refusal (ADR 0291 pattern, as ADR 0541 D8)

`sa_hyper.rs::rf2_the_paths`, `sa_dg.rs::f1h_refusals` and `sa_dgm.rs::f1m_refusals` each get
`#[ignore = "record ...: ADR 0555 (F-620) opened the Method step and the linear paths to the hyperelastic material, retiring
the row(s) ...; continued in ... (ADR 0291 pattern, maintainer standing decision)"]`, the test body unchanged, and a copy in the
new file `crates/rcfd-fem/tests/sa_hyper_paths.rs` with the retired rows inverted (accepted) and every other row as it was:
`rf2_the_paths_after_f620`, `f1h_refusals_after_f620`, `f1m_refusals_after_f620` (check RC below). The ignore attribute is the
only edit of those files. If the pre-check (after the code, the test files that name the hyperelastic
material run once to find any other record) finds another test that asserts a lifted refusal, it is treated the same way and
listed in the addendum; no other existing test is edited.

### D6. Files

`crates/rcfd-fem/src/mat_hyper.rs` (D1, additive, and the module documentation), `material.rs` (D1, additive),
`model.rs` (D3: one block of `check_elements`), `assemble.rs`, `damped.rs`, `estimate.rs`, `harmonic_direct.rs`, `hex8x.rs`,
`recover.rs` (D2: the call sites, one token each), `nl_transient.rs` (D4: the removed block, one condition), the attributes of
D5, new tests `crates/rcfd-fem/tests/sa_hyper_paths.rs` (the checks) and `sa_hyper_paths_default_bits.rs` (B0). No case-file key,
no golden, `nonlinear.rs`, `contact.rs`, `dg.rs`, `emc.rs` untouched.

### D7. Set-ups (registered)

Energies (each with density 1000): **NHc** `neo_hookean(mu = 1e6, kappa = 1e7)` (`K/G = 10`); **MRc** `mooney_rivlin(c10 =
3.5e5, c01 = 1.5e5, kappa = 1e7)` (the same `G = 1e6`, `K = 1e7`); **NHi** `neo_hookean(1e6, 1e10)` (`K/G = 1e4`); the cube
energies of ADR 0544 D5: **NHk** `neo_hookean(1e5, 1e6)`, **MRk** `mooney_rivlin(3.5e4, 1.5e4, 1e6)` (density 1000). The
isotropic **twin** of an energy: `Material::isotropic_elastic(E, nu, rho, 0)` with `E = 9KG/(3K + G)`, `nu = (3K - 2G)/(2(3K +
G))` of its `(G, K)`.

- **Cantilever L**: `hex_box([0, 0, 0], [2, 0.5, 0.5], [8, 2, 2])`, block Full (NHc, MRc) or BBar (NHi, the linear B-bar of ADR
  0493), `xmin` held in the three components, `Load::nodal_force("xmax", [0, 0, f])` with `f = 1` (L1, L3 to L6) or
  `f = 3.3e-3` (L7; arithmetic 3).
- **Bar U**: `hex_box([0; 3], [1; 3], [2, 2, 2])`, `u_x = 0` on `xmin`, `u_y = 0` on `ymin`, `u_z = 0` on `zmin`, `u_x = delta =
  1e-3` on `xmax` (`Constraint::displacement`), NHc and MRc, Full.
- **Cube K (small amplitude)**: `hex_box([0; 3], [1; 3], [2, 2, 2])`, `xmin` held in the three components, NHk or MRk, `u_0 = 0`,
  `v_0 = s X_1 (5, 4, 3)` at each node (zero on `xmin`), `dt = 0.004`, 100 steps, Newmark average acceleration; `s = 1e-5` and
  `s = 1e-6`; the damped variant with `Rayleigh { mass: 1.0, stiffness: 1e-4 }` on both integrators.
- **Cube F (large strain)**: the free cube of ADR 0544 (ADR 0523 arithmetic 3): `[2, 2, 2]` Hex8 unit cube, free, `u_0 = 0`,
  `v_0 = omega e_z x r + V + alpha r + gamma r_y e_x` (`omega = 3`, `alpha = 5`, `gamma = 4`, `V = (0.3, -0.2, 0.1)`, `r = X -
  (0.5, 0.5, 0.5)`), NHk and MRk, `NewtonOptions::new(1)`; the Method step is `NlTransientOptions::new(dt)` (average
  acceleration), the F-611 step `NlTransientOptions::new(dt).energy_momentum().algorithmic_stress(MaterialDiscreteGradient)`.
- **Start state S**: Cube F's model and `v_0` with `u_0 = 0.1 r + 0.05 r_2 e_1` at each node.

## Arithmetic before this registration (disclosed, by hand)

1. **The linear D.** With `(G, K) = (1e6, 1e7)`: `E = 9e13/3.1e7 = 2.9032e6`, `nu = 2.8e7/6.2e7 = 0.45161`, `lambda = K - 2G/3 =
   9.3333e6`. The route `lambda = E nu/((1 + nu)(1 - 2 nu))` has `1 - 2 nu = 0.0968`, a relative rounding of about `eps/0.1 =
   2e-15`; at `(1e6, 1e10)` `1 - 2 nu = 3G/(3K + G) = 1.0e-4`, about `2e-12`. The tangent of `mat_hyper` at zero equals linear
   elasticity of `(G, K)` within `1.3e-16` (ADR 0541 SS2, measured), and D1 computes the same closed form, so `D1 - D_hyper(0)`
   is a few `eps` of `lambda + 2G` at every ratio.
2. **The linear paths against the twin.** A path with the hyperelastic material differs from the twin only by `D1 - D_twin`
   (item 1): a relative perturbation `delta ~ 2e-15` (`K/G = 10`) or `2e-12` (`K/G = 1e4`) of `D`. A stiffness perturbation
   `delta K` with `|delta K| <= delta |K|` changes the solution of a well-posed problem by at most `delta` times the effective
   condition of the perturbation, which for a structured perturbation of the same pattern is of order one to ten (not the full
   condition number: `delta K = delta K_lambda`, a fraction of `K` itself). Estimate `<= 2e-14` and `<= 2e-11`; limit `1e-10`
   (and `1e-8` for the NHi row, BBar at `K/G = 1e4`, whose `lambda` part dominates `K`).
3. **Linear against nonlinear at small strain (L7).** Cantilever L: bending strain at the root `6 F L/(E b h^2)` with the total
   tip force `F = 9 f` (the 9 nodes of `xmax`), `L = 2`, `b = 0.5`, `h = 0.5`, `E = 2.9e6`: `F = 0.03`, `f = 3.3e-3` give `1.0e-6`.
   The nonlinear solution differs from the linear one by the material nonlinearity (relative `c |E|`, ADR 0541 SS1 measured
   `4.7e-6` and `4.9e-6` at `|E| = 1e-6` for `K/G = 2.2`; at `K/G = 10` larger by up to the ratio of the volumetric to the
   isochoric part, estimate `c <= 20`) and the geometric one (`(w/L)^2 ~ (1e-6 L/h)^2`, negligible): estimate `<= 2e-5`; limit
   `1e-4`. The Newton tolerance `1e-10` (relative) adds `<= 1e-9`.
4. **The Method step at small amplitude (M3).** Cube K: shear wave speed `sqrt(G/rho) = 10 m/s`, pressure wave `sqrt((K + 4G/3)
   /rho) = 33.7 m/s`; the lowest clamped mode `omega_1 ~ (pi/2) 10/1 = 16 rad/s`. Velocity at `X_1 = 1`: `7.1 s`, displacement
   `~ 7.1 s/omega_1 = 0.45 s`, strain of the same order: `4.5e-6` at `s = 1e-5`. The Method step (nonlinear) and the linear
   transient integrator (same Newmark, same `dt`, the linearised material) differ by `c |E|` relative (as item 3; the frequency
   shift of a quadratic nonlinearity is second order in the amplitude, so 100 steps, about one period of the lowest mode, add
   no secular term of first order): estimate `5e-6 .. 1e-4`; limit `1e-3`. The difference is first order in the amplitude, so
   the ratio of the measures at `s = 1e-5` and `s = 1e-6` is near 10; registered `5 <= ratio <= 20` (M3b). With Rayleigh damping
   the Method step's stiffness part is the hyperelastic tangent at zero, equal to D1 within item 1: the same estimate (M3d).
5. **The Method step against F-611 at large strain (M4).** For a linear problem the energy-momentum midpoint step is the
   trapezoidal rule (ADR 0514 arithmetic, its line "the trapezoidal rule (Newmark beta = 1/4, gamma = 1/2) written at the
   midpoint"), so on Cube F the two steps differ only through the nonlinearity of the internal force, a difference of order
   `dt^2` (both steps are second order and consistent with the same equations). Cube F's deformation modes: `omega` from `~30`
   (shear) to `~230` (the mesh's highest, `2 sqrt(3) c_p/h` with `h = 0.5`); the phase error per step of each method `(omega
   dt)^2/12`, at `dt = 0.004` up to `0.07` for the highest mode, at `dt = 0.001` `4e-3`; the deformation part of the displacement
   is about `0.025` against `|u|_inf ~ 0.5` (rotation `3 rad/s` over `0.2 s`, translation `0.06`). Over `T = 0.2 s`: `D(dt) =
   |u_M(T) - u_EM(T)|_inf / |u_EM(T)|_inf` estimated `<= 1e-2` at `dt = 0.004` and `<= 1e-3` at `dt = 0.001`; registered `D(0.001)
   <= 1e-2` (margin 10) and the ratio `D(0.004)/D(0.001) >= 4` (order at least 1; second order gives 16 in the asymptotic
   range; 4 leaves room for the pre-asymptotic high modes).
6. **The start (M1, M2).** At state S both integrators solve `M a_0 = -f_int(u_0)` with the same consistent mass; the Method
   step's `f_int` from `nonlinear::internal_force` (`Material::stress`, so `mat_hyper::stress_and_tangent`), F-611's from
   `emc::internal_force` at `u_0 = u_1` (`mat_hyper::stress`): the same stress function up to the rounding of two assemblies,
   `~1e-15` relative, amplified by the mass solve by its condition (`~10` to `100` for a consistent Hex8 mass): `<= 1e-13`;
   limit `1e-12`. The strain energy of the two integrators at the same state is the same loop with the same function on the
   same displacement (D4): equal bits.
7. **The homogeneous bar (L2).** Bar U is a homogeneous uniaxial stress state, exact for Hex8: `u_y(ymax) = -nu delta`, the sum
   of the reactions on `xmax` `= E A delta / L = E delta` (`A = L = 1`) with the twin's `E`, `nu`. Only rounding: limit `1e-10`.

## Checks (limits fixed now)

Every measure is required finite before it is compared (a non-finite value misses). "Relative" is to the largest absolute entry
of the reference.

| Id | Clause | What | Limit | Estimate |
|---|---|---|---|---|
| M0 | c1 (set-up) | Cube F with NHk and with MRk on the Method step (`dt = 0.004`): `Integrator::new` accepts, 50 steps converge, `strain_energy` and `kinetic_energy` finite at every step | `Ok` | D4 |
| M1 | c3 | state S, NHk and MRk: the initial accelerations of the Method step and F-611's step, `|a_M - a_EM|_inf / |a_EM|_inf` | `<= 1e-12` | arithmetic 6: `<= 1e-13` |
| M2 | c3 | state S, NHk and MRk: `strain_energy()` of the two integrators after `start` | equal bits | arithmetic 6 |
| M3 | c1, c2 | Cube K, `s = 1e-5`, NHk and MRk: the Method step against `transient::Integrator` with the same hyperelastic material (linearised), `max_n |u_M - u_L|_inf / max_n |u_L|_inf` over 100 steps | `<= 1e-3` | arithmetic 4: `5e-6 .. 1e-4` |
| M3b | c1 (first order) | the measure of M3 at `s = 1e-5` over the one at `s = 1e-6`, NHk and MRk | `5 <= r <= 20` | arithmetic 4: `~10` |
| M3d | c1 (damping) | M3 with `Rayleigh { mass: 1.0, stiffness: 1e-4 }` on both integrators, `s = 1e-5`, NHk | `<= 1e-3` | arithmetic 4 |
| M4 | c3 | Cube F, NHk and MRk, `T = 0.2 s`: `D(0.001)` (200 steps of each) | `<= 1e-2` | arithmetic 5: `<= 1e-3` |
| M4b | c3 (order) | `D(0.004) / D(0.001)`, NHk and MRk | `>= 4` | arithmetic 5: `~16` |
| M5 | threads | Cube F, MRk, Method step, 20 steps with 1 and 4 threads | equal state bytes | the assembly is thread-independent (ADR 0479) |
| M6 | state | Cube F, MRk, Method step: 20 steps against 10 + `to_bytes` + `from_bytes` + `restore` + 10 | equal state bytes | no new state |
| M7r | record | Cube F, NHk and MRk, Method step `dt = 0.004`, 50 steps: `max_n |E_n - E_0|/|E_0|` | record | not conserving (ADR 0514 N1e) |
| M8 | c1 (F-bar) | Cube F's mesh with the BBar block, NHk, Method step, 20 steps converge, energies finite | `Ok` | the driver takes the F-bar Hex8 (ADR 0541 SP1) |
| L0 | c2 | for NHc, MRc, NHi, NHk, MRk: `|linear_elasticity_matrix - D_hyper(0)|_max / |D_hyper(0)|_max` with `D_hyper(0)` the tangent of `mat_hyper::stress_and_tangent` at zero strain | `<= 1e-14` | arithmetic 1: a few `eps` |
| L0t | c2 | the same against `isotropic_elasticity(E, nu)` of the twin | `<= 1e-10` (NHi: `<= 1e-8`) | arithmetic 1: `2e-15`, `2e-12` |
| L1 | c2 (static) | Cantilever L, `f = 1`, NHc and MRc (Full), NHi (BBar): `solve_static` against the twin, displacement, and the recovered stress (`IntegrationPointField::from_result`) | `<= 1e-10` each (NHi: `<= 1e-8`) | arithmetic 2 |
| L2 | c2 (closed form) | Bar U, NHc and MRc: `|u_y(ymax) + nu delta| / (nu delta)` at every node of `ymax`, `|sum R_x(xmax) - E delta| / (E delta)` | `<= 1e-10` each | arithmetic 7 |
| L3 | c2 (modal) | Cantilever L, NHc and MRc (Full), NHi (BBar): the six lowest frequencies of `modal_analysis` against the twin's, relative each | `<= 1e-10` (NHi: `<= 1e-8`) | arithmetic 2 |
| L4 | c2 (transient) | Cantilever L, NHc: `transient::Integrator` (average acceleration, `dt = 1e-3`, the model's loads as a constant history), 50 steps against the twin: `max_n |u - u_twin|_inf / max_n |u_twin|_inf` | `<= 1e-10` | arithmetic 2 |
| L5 | c2 (harmonic) | Cantilever L, NHc: `DirectHarmonic` with `LossFactor::Constant(0.02)` on `"all"`, the model load, at `omega = 50` and `500`: `|x - x_twin|_inf / |x_twin|_inf` (complex modulus) | `<= 1e-10` | arithmetic 2 |
| L6 | c2 (buckling) | Cantilever L with `f` replaced by an axial `Load::nodal_force("xmax", [-1, 0, 0])`, NHc: `Prestress::from_static` and `buckling::factors` (3 factors) against the twin, relative each | `<= 1e-10` | arithmetic 2 |
| L7 | c2 (linearisation) | Cantilever L, `f = 3.3e-3`, NHc and MRc: `solve_static` against `nonlinear::solve` (`NewtonOptions::new(1)`) with the same hyperelastic model, `|u_lin - u_nl|_inf / |u_nl|_inf` | `<= 1e-4` | arithmetic 3: `<= 2e-5` |
| L8 | c2 (damped modes) | Cantilever L, NHc: `damped::damped_modes` with a constant loss factor 0.02 on `"all"` (4 modes) against the twin: each root, relative | `<= 1e-10` | arithmetic 2 |
| RF1 | D3 | a Shell4 model (the `shell_model` set-up of `sa_laminate.rs`, a 2 x 2 flat plate, thickness 0.01) with NHc: `solve_static` refused `NotYetSupported { variant: "Hyperelastic", package: LINEAR_FOLLOW_UP }` | exact | D3 |
| RF2 | D3 | Cantilever L, NHc, with a `Load::temperature` (uniform 10 K above the reference): `solve_static` refused the same | exact | D3 |
| RF3 | D1 | `Material::elasticity_matrix` of NHc still refused naming `mat_hyper::FOLLOW_UP`; `Material::stress` of NHc at a strain of `1e-3` equal in bits to `mat_hyper::stress_and_tangent`; `linear_elasticity_matrix` of the isotropic, orthotropic (fixed axes) and J2 materials equal in bits to their `elasticity_matrix` (or the same error) | exact | D1 |
| RF4 | D3 | Cube F's model, NHk, on the Method step with a temperature field (`TemperatureFieldHistory::constant`): refused (key `"expansion"`) | exact | the existing check |
| RC | D5 | the copies `rf2_the_paths_after_f620`, `f1h_refusals_after_f620`, `f1m_refusals_after_f620` pass (the retired rows accepted, the other rows as before) | pass | D5 |
| B1 | defaults | B0 (D8): every digest of base 89bdd903; `sa_hyper_default_bits`, `sa_dg_default_bits`, `sa_dgm_default_bits`, `sa_emc_default_bits`, `sa_nl_transient_default_bits` pass | exact; pass | D2, D4 delegate |
| B2 | defaults | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | no case output changed |
| S1 | build | `cargo test -p rcfd-fem --release` 0 failed; `cargo test -p rcfd-cli --release --test docs_site` pass; `cargo fmt -p rcfd-fem --check`; `cargo clippy -p rcfd-fem --release --all-targets -- -D warnings` | clean | |

A check that would take more than about two minutes in release is an ignored heavy record run once, with a fast test on the same
code path; none is expected (the largest model has 135 nodes).

### D8. The default-bits record B0 (`crates/rcfd-fem/tests/sa_hyper_paths_default_bits.rs`)

Written after this commit and run on the unchanged base 89bdd903 before any library line; FNV-1a 64 digests written into the
file then (the scheme of `sa_hyper_default_bits.rs`). Content, the paths that D2 and D4 touch, with the isotropic material (and
the orthotropic material with fixed axes where named): Cantilever L with the isotropic twin of NHc, Full and BBar: `solve_static`
(displacement) and its recovered stress field; `modal_analysis` (6 frequencies and the first mode); `damped_modes` with a loss
factor; `DirectHarmonic` with a loss factor at one frequency; `transient::Integrator` 20 steps; `Prestress::from_static` and
`buckling::factors`; `estimate::recover` of the static displacement; the nl_transient Method step on Cube F's mesh with the
isotropic material (20 steps: state bytes, `strain_energy`); the energy-momentum step with NHk and `DiscreteGradient` (10 steps:
`strain_energy`, the step-local branch D4 keeps); and the refusal messages of `solve_static` and `nl_transient::Integrator::new`
for the J2, viscoelastic and creep materials on Cantilever L. Any API whose exact call is not clear from the source is replaced
by its nearest form in the file and noted there; the content is fixed by this list.

## Planned mutations (on a scratch copy: a `git archive` of the code commit with its own target, one at a time; `sa_hyper_paths.rs` run for each)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | `lambda = K + 2G/3` in `linearised_elasticity` | L0, L1, L2, L3 |
| MU2 | the shear entries `2G` instead of `G` | L0, L1, L3 |
| MU3 | `(G, K)` swapped in `linearised_elasticity` | L0, L2 |
| MU4 | `linear_stress` of `Hyperelastic` returns `Material::stress` (the nonlinear stress) | L1 (stress row) |
| MU5 | `assemble.rs` 238 back to `elasticity_matrix` | L1, L3 (refused) |
| MU6 | `harmonic_direct.rs` 647 back to `elasticity_matrix` | L5 (refused) |
| MU7 | the structural-element refusal of D3 removed | RF1 |
| MU8 | the temperature-load refusal of D3 removed | RF2 |
| MU9 | the Method-step branch of `strain_energy` removed (the step-local energy on the Method step) | M0 (MRk refused), M2 |
| MU10 | the Method-step refusal of ADR 0541 D4 kept | M0, M1, M3, RC |
| MU11 | `damped.rs` 716 back to `elasticity_matrix` | L8 (refused) |

No mutation is declared equivalent.

## Decision rule

- **done**: every check of the table met (M7r a record), every mutation caught, fmt and clippy clean, B1 and B2 unchanged.
- **partial**: B1 and B2 unchanged, M0 to M2 and L0 to L3 met, and any other check missed (recorded as a `*_known_miss` test with
  its limit and measured value, diagnosed), or a mutation uncaught, or a check met only after a post-run amendment.
- **todo**: otherwise (in particular any change of the default bits or of an existing test beyond the attributes of D5).

## Limits (not covered)

- Beam2, Shell4 and Shell3 elements with the hyperelastic material on the linear paths (refused by D3); the corotated shells,
  the energy-momentum step with `Auto` and the rotor path keep their refusals.
- No thermal strain for the hyperelastic material (no expansion coefficient: the temperature load and field are refused).
- The linear paths take the material at rest; a pre-stressed linear path (modes, buckling, harmonic about a nonlinear static
  state) takes `D` at zero strain, not the tangent at the pre-stressed state (open question 2).
- The Method step is not energy-conserving at large strain (as with any material, ADR 0514 N1e); F-611's step is the
  conserving one.
- No case-file key (ADR 0541 remaining question 3).

## Open questions (taken on the maintainer's standing instruction of 2026-11-11, take the recommendation; none changes a default)

1. **Where the linearisation sits.** Options: (a) a `linear_` family of `Material` calls at the linear call sites, the
   element-constant `elasticity_matrix` still refused for the material (no existing test row changed beyond the lifted refusals);
   (b) `elasticity_matrix` itself returns the linearised matrix (one arm, but the M4 row of `m1_to_m4_after_f425` and RF2's
   `elasticity_matrix` row would also need records, and the nonlinear paths that reach `elasticity_matrix` would silently take
   it); (c) a model-level surrogate at every linear entry point (seventeen entry points). Recommendation (a), taken.
2. **The tangent about a pre-stressed state.** A linear path about a nonlinear static state could take the hyperelastic tangent
   at that state instead of `D` at rest. Recommendation: a follow-up, with the nonlinear pre-stress path of the geometric
   stiffness.
3. **Structural elements.** Recommendation: a follow-up giving Beam2 and the shells the linearised `E` and `nu` (a plane-stress
   reduction of `D1` for the shells), with a check against the solid model.

## Amendment 1 (2026-11-15, written after the registered run of `sa_hyper_paths.rs` and before any further run)

The registered run (`f620-scratch/run1.log`, once, on d1db72c6) met every check but one: **L3, row NHi BBar** failed in
`modal_analysis` of the hyperelastic model with `Solver { solver: "lowest_eigenpairs", reason: "subspace iteration did not converge
in 200 iterations (tolerance 1e-10)" }`, the residuals of the six wanted pairs `1.3e-9 .. 2.7e-11`. The test panicked at that row,
so the twin of that row was not run. The rows NHc and MRc of L3 were met (`2.99e-14` each). No limit changes.

- `l3_modes_against_the_twin` becomes `l3_modes_against_the_twin_known_miss`, `#[ignore]`d with the run's numbers, its body and
  limits unchanged.
- **L3d (diagnostic, a record, no limit)**: the same `modal_analysis` (`ModalOptions::new(6)`) of the NHi BBar cantilever with the
  twin; whether it converges, and if both converge with a larger iteration budget, the frequencies of both. The question it
  answers: is the non-convergence a property of the near-incompressible BBar model (the twin fails the same way) or of the
  linearised material (the twin converges).
- **L3c (continuation)**: the NHc and MRc rows of L3 as their own test (`l3_nhc_mrc_rows`), the same limits, run in the suite as
  the fast test of the modal path; their registered values are those of run 1.
