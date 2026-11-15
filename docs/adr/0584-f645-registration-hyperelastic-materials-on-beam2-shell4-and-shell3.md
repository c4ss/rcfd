# ADR 0584: F-645 registration: hyperelastic materials on Beam2, Shell4 and Shell3 on the linear paths

Status: **REGISTERED 2026-11-15, written before any code of F-645 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-645 was written and nothing of F-645 was compiled or run: the files
under "What was read" were read and the arithmetic below was done by hand (no script, no probe, no rcfd call). Results go in the
addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-645 (dep F-620), structural solver, advanced tier, the follow-up named by the refusal of
[ADR 0555](0555-f620-registration-hyperelastic-materials-on-the-method-step-and-the-linear-paths.md) D3
(`mat_hyper::LINEAR_FOLLOW_UP`, "a follow-up of ADR 0555"). Number 0584 given by the lead. Branch `f645-work` from 8d531fab
(local main, which includes F-644, ADR 0581), worktree `w49i-work/f645`, scratch `w49i-work/f645-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact and the measured number); each registered run made once; existing tests not edited; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe
code; clean-room. The roadmap files are not edited (the entries go in "For the lead"). Every choice below that the maintainer would
normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).

## What F-645 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. A Beam2 cantilever and a Shell4 plate with a neo-Hookean or Mooney-Rivlin material give on the linear static solve and the modes the results of the isotropic material of the same small-strain moduli within 1e-10.

Clauses: **a0** every limit registered here; **a1** a Beam2 cantilever with a hyperelastic material, `solve_static`, equals its
isotropic twin within `1e-10`; **a2** the same cantilever, `modal_analysis`, equals the twin within `1e-10`; **a3** a Shell4 plate,
`solve_static`, within `1e-10`; **a4** the Shell4 plate, `modal_analysis`, within `1e-10`. "A neo-Hookean or Mooney-Rivlin": both
energies are run on every row (the stricter reading). Shell3 is in the title: it takes the same route (D2), so the same rows are
run on a Shell3 plate (registered here, not an acceptance clause).

## What was read

`AGENT_RULES.md`; the brief; ADR 0555 (D1 to D3: the linearised material `mat_hyper::linearised_elasticity`, the three methods
`Material::linear_elasticity_matrix`, `linear_elasticity_at`, `linear_stress`, the refusal of D3 on a structural element or a
temperature load); ADR 0581 (the residual floor `c eps lambda_max / lambda_1` of the default relative residual, its diagnosis and
the "For the lead" form); ADR 0572, 0573 (the form of the "For the lead" section). Sources at 8d531fab: `crates/rcfd-fem/src/model.rs`
(`SolidModel`, `check_elements` 1205-1335, the refusal 1300-1323; `Material::youngs_modulus`, `poisson_ratio`, `density`,
`expansion` 464-492: NaN for the hyperelastic material except the density), `crates/rcfd-fem/src/material.rs` (439-505),
`crates/rcfd-fem/src/mat_hyper.rs` (1-66, `small_strain_moduli`), `crates/rcfd-fem/src/beam.rs` (`moduli` 239-243 and its five
callers, `axial_force` 724-742, which reads `youngs_modulus` directly), `crates/rcfd-fem/src/beam_warping.rs` (421, `beam::moduli`),
`crates/rcfd-fem/src/shell.rs` (`moduli` 297-302, `homogeneous_section` 456-466, `section_of`, `model_section`, the thermal
lines 1271, 1692, 1934 reading `expansion`), `crates/rcfd-fem/src/shell3.rs` (889-920: stiffness and mass through
`homogeneous_section` and `density`; 1019 `expansion`), `crates/rcfd-fem/src/laminate.rs` (a ply must be isotropic or
orthotropic: a hyperelastic ply stays refused, unchanged), `crates/rcfd-fem/src/mat_ortho.rs` (`check_model`), `crates/rcfd-fem/src/
geometric_struct.rs` (181, the beam pre-stress through `beam::axial_force`), `crates/rcfd-fem/src/rotor.rs` (542, its own rotor
model, not a `SolidModel` path), the callers of `check_supported_with_materials` and `check_elements_supported_with_structures`
(solve, modal, buckling, cms, cyclic, geometric, mpc, rotating, beam_warping). Tests: `sa_hyper_paths.rs` (the energies, `hyper`,
`rf1_to_rf4_refusals`), `sa_geometric_struct.rs` (the beam and plate set-ups `beam_model`, `grid`, the buckling of B1), and the
audit below.

## Audit of the tests that assert the refusal (before any code)

Searched `crates/*/tests` for `LINEAR_FOLLOW_UP`, "follow-up of ADR 0555", "a structural element" and every test file naming the
hyperelastic material (`sa_arclength_sphere_f649`, `sa_dg`, `sa_dgm`, `sa_dgm_default_bits`, `sa_emc`, `sa_hyper`,
`sa_hyper_default_bits`, `sa_hyper_lock`, `sa_hyper_paths`, `sa_hyper_paths_default_bits`, `sa_hyper_paths_f644`, `sa_laminate`,
`sa_scaffold`). One live assertion of the structural refusal exists: **`sa_hyper_paths.rs::rf1_to_rf4_refusals`, row RF1**: a 2 x 2
Shell4 plate with `neo_hookean(1e6, 1e7)` on `solve_static` with a default model is refused naming `LINEAR_FOLLOW_UP` and "a
structural element". `sa_laminate.rs` RF2 (a hyperelastic ply refused by `section_stiffness`) and `sa_scaffold.rs` (ignored records
and the pending table) do not touch the element refusal. No Beam2 or Shell3 refusal is asserted. Lifting the refusal by default
would break RF1, whose retirement (record and copy) needs the maintainer's approval. Hence D1: the lift is opt-in and RF1 stays
true.

## Design

### D1. The opt-in

A new model setting `SolidModel::set_hyperelastic_structures(&mut self, on: bool)` with the getter `hyperelastic_structures()`,
default `false` (in `SolidModel::new`). The refusal of ADR 0555 D3 for a hyperelastic material set holding a Beam2, Shell4 or Shell3
element is made only when the setting is off (its text, package and location unchanged); the temperature-load refusal of D3 is
kept in both cases (the hyperelastic material has no expansion coefficient, `expansion()` is NaN). With the setting on, every
linear path that takes structural elements (the callers of `check_elements` with `history = false` listed above) takes the
hyperelastic material on them as its linearised material (D2). The setting is read nowhere else; a model that does not set it runs
every line it ran before. Why a setting and not a lift: RF1 (the audit); and the linearised material on a beam or a shell is the
small-strain law reduced by the element's kinematic assumptions (uniaxial with shear for Timoshenko, plane stress for the shells),
which is a modelling statement the user makes explicitly. Taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation); making it the default is remaining question 1.

The alternative of a field on `Material::Hyperelastic` was rejected (it changes the public variant's shape, used by pattern and by
the API crate), and of a path option (`StaticOptions`, `ModalOptions`, ...) because nine paths would each need it.

### D2. The moduli of a structural element

The structural elements do not read a 6 x 6 elasticity: Beam2 reads `(E, G)` (`beam::moduli`, `G = E / (2 (1 + nu))`), Shell4 and
Shell3 read `(E, nu, G)` (`shell::moduli` in `homogeneous_section`, the membrane and bending plane-stress matrices
`E t / (1 - nu^2) [...]`, the transverse shear `kappa G t`, the drilling `(G, t)`), and `beam::axial_force` reads `E`. The
linearised material of ADR 0555 D1 is the isotropic elasticity of `(G, K)` of `mat_hyper::small_strain_moduli`; its Young modulus
and Poisson ratio are the isotropic identities (first principles, from `lambda = K - 2G/3` and `E = G (3 lambda + 2G) / (lambda +
G)`, `nu = lambda / (2 (lambda + G))`):

    E = 9 K G / (3K + G),    nu = (3K - 2G) / (2 (3K + G)),

and `E / (2 (1 + nu)) = G` exactly (arithmetic 1). A new function `mat_hyper::small_strain_young_poisson(energy) -> (E, nu)`
evaluates them with `s = 3K + G` once. A new crate-internal `Material::structural_young_poisson(&self) -> (f64, f64)` returns it
for `Hyperelastic` and `(self.youngs_modulus(), self.poisson_ratio())` for every other variant. `beam::moduli`, `shell::moduli` and
`beam::axial_force` read it instead of the two getters. For every non-hyperelastic material the values are the same two calls as
before, so the bits are those of before (D4). The public getters `youngs_modulus` and `poisson_ratio` are not changed (NaN for the
hyperelastic material, as documented). The mass reads `density`, already that of the hyperelastic material. `G` is recomputed from
`(E, nu)` by the existing formula rather than taken from the energy, so a hyperelastic structural element is exactly the isotropic
element of `(E, nu)` (the twin of D3 gives the same bits when its `(E, nu)` are the same bits; arithmetic 2).

### D3. Set-ups (registered)

Energies (density 1000): **NHc** `neo_hookean(1e6, 1e7)` (`G = 1e6`, `K = 1e7`, from `sa_hyper_paths.rs`) and **MRs**
`mooney_rivlin(2.5e5, 1.5e5, 4e6)` (`G = 2 (c10 + c01) = 8e5`, `K = 4e6`; a Mooney-Rivlin with both constants non-zero and moduli
different from NHc, so that a mistake in either is seen). The **twin** of an energy: `Material::isotropic_elastic(E, nu, 1000, 0)`
with `(G, K)` from `mat_hyper::small_strain_moduli` and `E`, `nu` by the closed form of D2 written out in the test (`s = 3K + G`,
`E = 9 K G / s`, `nu = (3K - 2G)/(2 s)`). The hyperelastic model sets `set_hyperelastic_structures(true)`; the twin model does not.

- **Beam B**: Beam2 along `x`, `L = 1`, 10 elements, `rectangle_section(0.05, 0.03, 2.6e-7, 5/6)` (unequal `I_y`, `I_z`: no
  degenerate bending pair), orientation `[0, 1, 0]`; node sets `x0` (node 0), `xl` (node 10); all six components of `x0` held.
  Static loads: `nodal_force("xl", [0.1, 0.2, 0.3])`, `nodal_moment("xl", [0.01, 0.02, 0.03])`, `gravity([0, 0, -9.81])` (axial,
  both bending planes, torsion, the mass).
- **Plate P4 / P3**: a rectangular grid on `[0, 1] x [0, 0.6]` in the `x-y` plane, `10 x 6` cells, Shell4 (P4) or Shell3 with each
  cell cut from its first to its third node (P3, the cut of `sa_geometric_struct.rs::grid`), `shell(0.01)`; the column `x = 0`
  held in all six components; the corner node `(1, 0.6)` set `c`. Static loads: `nodal_force("c", [0.1, 0.2, 0.3])`,
  `nodal_moment("c", [0.01, 0.02, 0.005])` (bending, twist and drilling), `gravity([0, 0, -9.81])` (membrane, bending, transverse
  shear and drilling are all loaded).
- **Static**: `solve_static(&model, &StaticOptions::default())`.
- **Modes**: `modal_analysis(&model, &ModalOptions::new(6).residual_test(ResidualTest::Backward))` (the default tolerance `1e-10`,
  200 iterations, the default shift and subspace). The opt-in backward residual (ADR 0354) and not the default relative one:
  arithmetic 3 puts the relative floor of these thin rubber structures at `~1e-8 .. 1e-7`, above the default tolerance, as in ADR
  0581's diagnosis.
- **Buckling Bb** (a registered extra, it covers the changed `axial_force` line): Beam B with `L = 1`, 8 elements, the planar and
  pinned constraints of `sa_geometric_struct.rs` B1 (`u_z`, `theta_x`, `theta_y` held at every node; `u_x`, `u_y` at `x0`, `u_y` at
  `xl`), the pre-stress of `nodal_force("xl", [-1, 0, 0])` by `Prestress::from_static`, `factors(.., &BucklingOptions::new(2))`.

### D4. Defaults

The only lines changed outside the new functions: the refusal condition in `check_elements` (an added `!self.hyperelastic_structures
&&`; with the default `false` the condition is the old one) and the three reads of D2 (for a non-hyperelastic material the same
calls in the same order). `SolidModel` gains a `bool` field: its `Debug` and `PartialEq` include it; no golden, hash table or
serialised form holds a `SolidModel` (`baseline_hashes.rs` hashes results). Checked by the existing tests (B0 below) and
`cargo test -p rcfd-api --release --test baseline_hashes`.

### D5. Files

`crates/rcfd-fem/src/model.rs` (field, setter, getter, the refusal condition), `crates/rcfd-fem/src/mat_hyper.rs`
(`small_strain_young_poisson`, the module doc), `crates/rcfd-fem/src/material.rs` (`structural_young_poisson`),
`crates/rcfd-fem/src/beam.rs` (`moduli`, `axial_force`), `crates/rcfd-fem/src/shell.rs` (`moduli`); new test file
`crates/rcfd-fem/tests/sa_hyper_structures.rs`; this ADR.

## Arithmetic (disclosed, by hand)

1. `E / (2 (1 + nu))` with D2: `1 + nu = (2s + 3K - 2G) / (2s) = 9K / (2s)`, so `E / (2 (1 + nu)) = (9KG/s) (s / (9K)) = G`. NHc: `s
   = 3.1e7`, `E = 9e13 / 3.1e7 = 2.9032e6`, `nu = 2.8e7 / 6.2e7 = 0.45161`. MRs: `s = 1.28e7`, `E = 2.88e13 / 1.28e7 = 2.25e6`, `nu =
   1.04e7 / 2.56e7 = 0.40625`. Both admissible for plane stress (`1 - nu^2 > 0`).
2. **Expected difference hyperelastic against twin.** The test evaluates the same expression in the same order as the library on
   the same `(G, K)` (both from `small_strain_moduli`), so `(E, nu)` are the same bits, every element matrix is the same bits and
   every result is the same bits: expected `0`. Should the two evaluations differ by an ulp (a compiler contraction, say), the
   matrices differ by `~2e-16` relative in a smooth parameter; the static answer moves by `<= ~2e-16` times a sensitivity `<= 1`
   plus the rounding of two direct solves of nearly equal matrices; on these small, moderately conditioned models (`~400` unknowns)
   that is estimated `<= 1e-13`. Limits `1e-10` (the acceptance) on every row.
3. **Modal floor of the default relative residual** (ADR 0581). Beam B: `lambda_1 = 3.516^2 E I / (rho A L^4)` with `I/A = h^2/12 =
   7.5e-5` (the weak plane, `h = 0.03`), NHc: `12.36 * 2.9e6 * 7.5e-5 / 1000 = 2.7`. `lambda_max` of the Timoshenko element of
   `l_e = 0.1`: the shear-rotation element mode `~ kappa G A / (rho I) * c = 0.83e6 * 12 / (1000 * 9e-4) * c ~ 1.1e7 c` with `c` up to
   a few: `lambda_max / lambda_1 ~ 1e7`, `eps` times it `~ 2e-9`, the floor of ADR 0581 (`c = 0.05 .. 0.1` of it) `~ 1e-10`: at or
   above the default `1e-10`. Plate: `lambda_1 ~ 1.875^4 D/(rho t L^4)`, `D/(rho t) = E t^2 / (12 rho (1 - nu^2)) = 0.030`: `0.37`;
   `lambda_max` the transverse-shear and rotary element mode `~ kappa G t / (rho t^3/12) = 1e8`: ratio `~3e8`, floor `~3e-9`: above.
   So the default relative test would stop on neither model; the backward test (floor `c eps`, ADR 0354) is used. Its weaker
   stopping guarantee does not matter here: the rows compare two runs with the same matrices (arithmetic 2), not a run against a
   closed form. Estimate: both runs converge within 200 iterations (backward residuals `<= 1e-10`), with the same iteration count.
4. **Negative controls.** The twin of the energy with `G (1 + 1e-6)` (NHc: `neo_hookean(1e6 (1 + 1e-6), 1e7)`). `dE/E = (dG/G) 3K
   / (3K + G) = 0.97e-6`, `d nu = -(9K/(2 s^2)) dG` (NHc `-4.7e-8`, relative `1e-7`). The tip motion of Beam B is mostly bending
   (`1/E`) with torsion and shear (`1/G`): relative change `~1e-6`; eigenvalues `~ E` or `G`: `~1e-6`; the plate the same through
   `E / (1 - nu^2)` and `G`. Expected `~5e-7 .. 1e-6`; limits `> 1e-8` (the check sees a perturbation 100 times smaller than its
   expected effect, and 100 times larger than the limit of the acceptance).

## Checks (limits fixed now)

Measures: static `e_u = max_i |u_h,i - u_t,i| / max_i |u_t,i|` over the displacement values (all six components); modes `e_lambda =
max_k |lambda_h,k / lambda_t,k - 1|` over the six eigenvalues, `e_phi = max_k |phi_h,k - s_k phi_t,k|_inf / |phi_t,k|_inf` with `s_k
= sign(phi_h,k . phi_t,k)`; buckling `e_f = max_k |f_h,k / f_t,k - 1|`. Every value entering a maximum is required finite (a NaN or
infinity is a miss). Every value is printed.

| Check | Clause | Set-up | Measure | Limit | Estimate |
|---|---|---|---|---|---|
| S-B-NH, S-B-MR | a1 | Beam B, static, NHc / MRs vs twin | `e_u` | `<= 1e-10` | 0 (arith. 2) |
| M-B-NH, M-B-MR | a2 | Beam B, modes | `e_lambda`, `e_phi` | `<= 1e-10` each | 0 |
| S-P4-NH, S-P4-MR | a3 | Plate P4, static | `e_u` | `<= 1e-10` | 0 |
| M-P4-NH, M-P4-MR | a4 | Plate P4, modes | `e_lambda`, `e_phi` | `<= 1e-10` each | 0 |
| S-P3-NH, S-P3-MR | (title) | Plate P3, static | `e_u` | `<= 1e-10` | 0 |
| M-P3-NH, M-P3-MR | (title) | Plate P3, modes | `e_lambda`, `e_phi` | `<= 1e-10` each | 0 |
| Bb-NH | extra | Beam Bb buckling, NHc vs twin | `e_f` | `<= 1e-10` | 0 |
| C-B, C-P4, C-P3 | control | NHc against the `G (1 + 1e-6)` twin: static and modes | `e_u`, `e_lambda` | `> 1e-8` each | `5e-7 .. 1e-6` |
| N1 | control | without the setting: Beam B, P4, P3 with NHc, `solve_static` and `modal_analysis` | refused naming `LINEAR_FOLLOW_UP`, "a structural element" | holds (6 rows) | holds |
| N2 | control | with the setting: Beam B with NHc and a temperature load, `solve_static` | refused naming `LINEAR_FOLLOW_UP`, "a temperature load" | holds | holds |
| N3 | control | the setting on an isotropic Beam B and P4: static displacement and the six eigenvalues against the same model without it | bits equal | holds | holds |
| N4 | control | `SolidModel::new(..).hyperelastic_structures()` | `false` | holds | holds |
| B0 | defaults | existing tests: `sa_hyper_paths` (RF1 among them), `sa_hyper_paths_default_bits`, `sa_hyper_paths_f644`, `sa_geometric_struct`, `sa_laminate_default_bits`, `sa_beam_warping_f626_bits`, rcfd-fem lib tests; `rcfd-api --test baseline_hashes` | pass | pass | pass |

Rows run in release (`cargo test -p rcfd-fem --release --test sa_hyper_structures`). The models are small (at most `77` nodes, 462
unknowns); the file is expected to run in seconds.

## Planned mutations (on a scratch copy, one at a time)

1. `small_strain_young_poisson`: `9 K G / (3K + G)` -> `9 K G / (3K + 2G)`. Expected caught by every S and M row.
2. `small_strain_young_poisson`: `(3K - 2G)` -> `(3K - G)` in `nu`. Expected caught by every S and M row.
3. `shell::moduli` reads `youngs_modulus`, `poisson_ratio` again (the hyperelastic branch removed): NaN. Caught by the P4 and P3 rows.
4. `beam::moduli` reads the two getters again. Caught by the Beam B rows.
5. `beam::axial_force` reads `youngs_modulus` again. Caught by Bb only (the static and modal rows do not read it).
6. The refusal condition ignores the setting (always lifted). Caught by N1 and by the existing RF1.
7. The refusal condition ignores the setting (never lifted). Caught by every S, M, Bb row (refused).

## Decision rule

**Done** if every acceptance row (S-B, M-B, S-P4, M-P4 with both energies) meets its limit in the one registered run, every
control row holds, B0 passes, mutations 1 to 7 are each caught by a registered check, fmt and clippy are clean. **Partial** if an
acceptance row misses (recorded as `*_known_miss`), or a control or B0 fails, or a mutation escapes. The P3 and Bb rows are
registered extras: a miss there makes the proposed status partial for the title's Shell3 or for buckling only, stated as such.
**Todo** if no acceptance row could be run.

## Limits (not covered)

- The default stays the refusal: the linearised structural material needs `set_hyperelastic_structures(true)`; it is not exposed in
  the case schema or the API crate (that would change the schema golden: maintainer's step).
- Only `solve_static`, `modal_analysis` and the beam buckling are checked. The other linear paths that take structures (cyclic,
  rotating, cms, mpc transformation, beam warping, harmonic and transient through the modes) take the same moduli through the same
  three functions but are not run here.
- A temperature load with a hyperelastic structural element stays refused (no expansion coefficient).
- A hyperelastic ply of a layup stays refused (`laminate::check_plies`); Shell3 layups are refused anyway.
- The nonlinear paths of shells (`shell_corot`) keep refusing a non-isotropic material; nothing nonlinear is opened here.
- The linearised material is the small-strain tangent: no claim on large rotations or finite strain of beams and shells.

## Addendum 1: results (2026-11-15)

The registered run was made once (`cargo test -p rcfd-fem --release --test sa_hyper_structures -- --nocapture
--test-threads=1`, log `w49i-work/f645-scratch/run1.log`, 0.15 s), at commit b93e7ebd (library and tests committed before the run).

| Check | Limit | Measured | Met |
|---|---|---|---|
| S-B-NH, S-B-MR `e_u` (a1) | `<= 1e-10` | 0, 0 | yes |
| M-B-NH, M-B-MR `e_lambda`, `e_phi` (a2) | `<= 1e-10` | 0, 0; 0, 0 (7 / 7 iterations, backward residuals <= 1.1e-11) | yes |
| S-P4-NH, S-P4-MR `e_u` (a3) | `<= 1e-10` | 0, 0 | yes |
| M-P4-NH, M-P4-MR `e_lambda`, `e_phi` (a4) | `<= 1e-10` | 0, 0; 0, 0 (6 / 6 iterations, residuals <= 2.8e-11) | yes |
| S-P3-NH, S-P3-MR `e_u` (Shell3) | `<= 1e-10` | 0, 0 | yes |
| M-P3-NH, M-P3-MR `e_lambda`, `e_phi` (Shell3) | `<= 1e-10` | 0, 0; 0, 0 (6 / 6 iterations, residuals <= 1.7e-11) | yes |
| Bb-NH `e_f` (beam buckling) | `<= 1e-10` | 0 (factors 8.8917, 34.884 both) | yes |
| C-B static / modes | `> 1e-8` | 9.68e-7 / 1.00e-6 | yes |
| C-P4 static / modes | `> 1e-8` | 9.45e-7 / 9.86e-7 | yes |
| C-P3 static / modes | `> 1e-8` | 9.45e-7 / 9.86e-7 | yes |
| N1 (6 rows) | refused, "a structural element" | refused, every row | yes |
| N2 | refused, "a temperature load" | refused | yes |
| N3 Beam2, Shell4 static and eigenvalue bits | equal | equal | yes |
| N4 default | `false` | `false` | yes |
| B0 | pass | rcfd-fem lib 14, `sa_hyper_paths` 17 (RF1 among them), `sa_hyper_paths_default_bits` 1, `sa_hyper_paths_f644` 4, `sa_geometric_struct` 9, `sa_laminate_default_bits` 1, `sa_beam_warping_f626_bits` 1, `rcfd-api --test baseline_hashes` 3: all pass (ignored records not run) | yes |

Every difference is exactly 0, as arithmetic 2 expected: the library and the test evaluate the same expression on the same `(G,
K)`. The first beam eigenvalue `2.688` matches the hand estimate `2.7` of arithmetic 3. The controls (`5e-7 .. 1e-6` expected) give
`9.5e-7 .. 1.0e-6`. fmt (`cargo fmt -p rcfd-fem --check`) clean; `cargo clippy -p rcfd-fem --all-targets -- -D warnings` clean.

Mutations (scratch copy `w49i-work/f645-scratch/mut`, script `mutate.py`, log `mutations.log`), each caught:

| Mutation | Caught by |
|---|---|
| 1 `E = 9KG/(3K + 2G)` | every S, M and Bb row (`3.1e-2 .. 6.3e-2`) |
| 2 `nu` with `3K - G` | every S, M and Bb row (`6.0e-5 .. 1.1e-2`) |
| 3 `shell::moduli` back to the getters | every P4 and P3 row and C-P4, C-P3 (NaN entry refused by the factorisation) |
| 4 `beam::moduli` back to the getters | every Beam B row, Bb, C-B (NaN) |
| 5 `beam::axial_force` back to `youngs_modulus` | Bb only (the pre-stress of a Beam2 element not finite), as registered |
| 6 setting ignored, always lifted | N1 (6 rows) and the existing RF1 of `sa_hyper_paths.rs` |
| 7 setting ignored, never lifted | every S, M, Bb, C row and N2 (refused as a structural element first) |

Deviations: none. Not run: the other linear paths that take structures (cyclic, rotating, cms, mpc transformation, beam warping,
harmonic and transient), as declared in "Limits"; the whole workspace suite (the lead's gate).

Decision by the registered rule: **done**. Every acceptance row met in the one registered run, every control held, B0 passed, seven
of seven mutations caught, fmt and clippy clean. The Shell3 and buckling extras met too.

### Remaining questions

1. **The default.** The hyperelastic material on a structural element needs `set_hyperelastic_structures(true)`; by default it is
   refused as before. Options: (a) keep the opt-in; (b) make it the default, retiring `sa_hyper_paths.rs::rf1_to_rf4_refusals` as
   an ignored record with a copy whose RF1 row accepts (the ADR 0291 pattern, which needs the maintainer's approval).
   Recommendation: (b) once the other linear paths that take structures have a check (question 2): the linearised material is
   exact by construction and the refusal now protects nothing. Until then (a).
2. **The other linear paths.** Cyclic, rotating, cms, mpc and beam warping take the same moduli through the same three
   functions but are unchecked. Options: (a) a follow-up with one twin row per path; (b) accept by construction. Recommendation:
   (a), a small test-only item (proposed below).
3. **The case schema.** The setting is not in the case schema or the API crate (that would change the schema golden).
   Recommendation: add it with question 1 (b) or not at all: with a default lift there is nothing to expose.

## For the lead: roadmap entries

> Proposed status: done.

> Structural solver, advanced tier. Follow-up of F-620 (ADR 0555). DONE. ADR 0584, 2026-11-15: the linear paths take a hyperelastic (neo-Hookean or Mooney-Rivlin) material on Beam2, Shell4 and Shell3 elements as its linearised material, opted in by SolidModel::set_hyperelastic_structures(true) (default off: the refusal of ADR 0555 D3 and its test RF1 stand); a structural element takes the isotropic element of the small-strain E = 9KG/(3K + G), nu = (3K - 2G)/(2(3K + G)) through mat_hyper::small_strain_young_poisson, read by beam::moduli, shell::moduli and beam::axial_force (unchanged bits for every other material: baseline hashes and the default-bits records pass). A temperature load stays refused. Against the isotropic twin of the same small-strain moduli, a Beam2 cantilever and a Shell4 and a Shell3 plate, each with a neo-Hookean and a Mooney-Rivlin material, give static displacements, the six lowest eigenvalues and mode shapes (backward residual test) with difference exactly 0 (limit 1e-10), and the beam buckling factors 0; the twin with G (1 + 1e-6) is seen at 9.5e-7 .. 1.0e-6. Seven of seven mutations caught. Not covered: the other linear paths with structures (cyclic, rotating, cms, mpc, beam warping), the case schema, the default lift (needs retiring RF1, maintainer's approval).

> Follow-up proposed: Hyperelastic structural elements on the other linear paths and by default. Deps F-645. Acceptance: Every limit not stated here is registered in the ADR before the run. With a neo-Hookean material on Beam2 and Shell4 elements, the cyclic, rotating, cms and mpc-transformation paths give the results of the isotropic twin of the same small-strain moduli within 1e-10; on the maintainer's approval the refusal of ADR 0555 D3 on structural elements is lifted by default with RF1 retired as a record and its copy accepting.
