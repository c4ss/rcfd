# ADR 0457: F-537 registration: orthotropic and tabulated materials on the modal, pre-stressed modal and transformation paths

Status: **REGISTERED 2026-11-14, written before any code of F-537 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-537 was written, compiled or run, except the bit record of the default paths (B1)
and two probes of the rounding of the modal analysis (both on the API of main dab660ed, no library line changed, disclosed
below), and the arithmetic of the beam values (disclosed below). Results go in the addenda at the end; nothing above the
addenda is changed after a run.

Parent: [ADR 0452](0452-f409-registration-orthotropic-elasticity-and-temperature-tables.md) (F-409: the `Orthotropic` and
`Tabulated` materials on the linear static solve, the per-point stiffness of its D5, the refusal of every other path naming
`mat_ortho::FOLLOW_UP = "a follow-up of ADR 0452"`, its remaining question 1, recommendation (a): "modal, prestressed modes and
the transformation path first"). Work package SA-2 of the structural solver, advanced tier ([ADR 0310](0310-structural-solver-advanced-tier-plan.md)).
Roadmap item F-537 (deps F-409). Number 0457 given by the lead. Branch `f537-work` from main dab660ed, worktree
`w49i-work/f537`, scratch `w49i-work/f537-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code, the crate
graph unchanged; no existing test edited except the records-and-copies step of the maintainer's standing decision (ADR 0291
pattern; D6 below); clean-room.

## What F-537 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. Modal analysis, prestressed modes and the transformation path (linear constraints, elastic supports) take the orthotropic and tabulated materials of ADR 0452 through the per-point stiffness; the first bending frequencies of an orthotropic Hex20 cantilever match the beam values with E1 within 1 percent; isotropic constants in an orthotropic material give the frequencies of the isotropic material within 1e-12; the paths still not opened refuse the variants by name; default paths bit-identical.

Clauses: (a0) every limit registered before the run (this document); (a1) the modal analysis, the pre-stressed modes and the
transformation path (linear constraints, elastic supports) take both variants through the per-point stiffness; (a2) the first
bending frequencies of an orthotropic Hex20 cantilever against the beam values with `E1` within 1 percent; (a3) isotropic
constants in an orthotropic material give the frequencies of the isotropic material within 1e-12; (a4) the paths not opened
refuse both variants by name; (a5) default paths bit-identical. The brief of the lead adds: (b1) a registered estimate of the
discretisation error behind the 1 percent (shear and rotary inertia of the beam reading, the mesh), the set-up not tuned;
(b2) for the pre-stressed modes with a tabulated material, the temperature at which the stiffness is taken.

## What was read (main dab660ed, read only)

1. `AGENT_RULES.md`; ADR 0452 whole (D1 to D9, the checks, addendum 1, remaining questions); ADR 0390 D4 (pre-stressed modes:
   with a zero pre-stress the bits of the modal analysis); ADR 0384 and ADR 0398 D2 (the modal analysis and the pre-stressed
   modes on the transformation); ADR 0385 D1 (the support of beam theory).
2. Code: `crates/rcfd-fem/src/{mat_ortho.rs (whole), assemble.rs (whole), modal.rs (whole), geometric.rs (whole), mass.rs
   (consistent_with), model.rs (Material accessors, check_elements and the check_* family), mpc.rs (check_transformation_model,
   solve_static_mpc, modal_analysis_mpc, refuse_nonzero_values), solve.rs (solve_static_report), buckling.rs, cyclic.rs,
   harmonic.rs, damped.rs, rotating.rs, transient.rs, nonlinear.rs, harmonic_direct.rs (their entry checks)}`; the tests
   `sa_ortho.rs` (whole), `sa_ortho_default_bits.rs`, `sa_scaffold.rs` (the rows naming the two variants and the modal paths),
   `sa_mat_table.rs` (grep: no modal row).
3. **Not read** (cited from memory, for the reader only): S. P. Timoshenko, *Vibration Problems in Engineering* (the beam with
   shear and rotary inertia); the cantilever eigenvalue `beta_1 L = 1.8751040687` (the first root of `cos x cosh x = -1`,
   recomputed in arithmetic 1).

Findings that shape the design:

1. The modal analysis checks `check_supported_with_structures` and assembles `stiffness_with`, which refuses both variants
   (`check_elements(.., point_materials = false)`). `prestressed_modes` checks `check_supported` (solids only) and assembles
   `stiffness_with` and the public `geometric_stiffness`, which checks `check_elements_supported`; `Prestress::uniform` checks
   `check_elements_supported`. `mpc::check_transformation_model` (the entry of `solve_static_mpc` and `modal_analysis_mpc`)
   checks `check_supported_with_structures`, and both assemble `stiffness_with`. `solve_static_report` already passes the
   variants to `solve_static_mpc`, which then refuses them.
2. The mass needs only the density: `Material::density` gives it for `Orthotropic` (its own) and `Tabulated` (the base's; not
   tabulated by ADR 0452 D2), and `mass::consistent_with` has no material check. The geometric stiffness takes the given
   stress and no material.
3. Every other dynamic path calls `check_supported` before it reaches the modal or transformation code: buckling
   (`factors`), cyclic (`modal_analysis_harmonic`, `full_rotor`), harmonic (`zero_prescribed`, `transformation_correction`),
   damped (`damped_modes`), rotating (`check_model`), transient, direct harmonic, nonlinear. These keep refusing with no edit.
   `rotating::modes(0)` calls `modal_analysis` after `RotatingPart::new` refused. `HarmonicSolver::new` without the static
   correction takes a modal basis as data and checks no material; it did before F-537 (a basis can be built by
   `ModalBasis::new`), so nothing changes there.
4. A `Cyclic` constraint reaches `modal_analysis_mpc` through `modal_analysis_with`. Its rows rotate the translations of one
   sector face onto the other; a material with `Fixed` axes is not cyclically symmetric, and the cylindrical axes would have to
   share the constraint's axis. Opening that is not asked: it is refused (D3).
5. The non-ignored test `sa_ortho::r1_r2_paths` asserts that `modal_analysis_with` (E3), `prestressed_modes` (E8),
   `solve_static_mpc` (E9) and `modal_analysis_mpc` (E10) refuse both variants. It becomes a record with a copy (D6).

## Arithmetic and probes before this registration (disclosed)

1. **Beam values** (`f537-scratch/py/timo.py`, numpy, a scratch script). The orthotropic cantilever of ADR 0452 O6
   (`E_1 = 1.4e11`, `G_13 = G_12 = 7e9`, `rho = 1600`, square section `h = 0.1`, `kappa = 5/6`). Euler-Bernoulli
   `f_EB = (beta_1 L)^2 sqrt(E_1 I/(rho A L^4))/(2 pi)`, `beta_1 L = 1.8751040687119611`. Timoshenko (shear and rotary inertia)
   by shooting the first-order system of `kappa G A (w'' - psi') + rho A omega^2 w = 0`,
   `E I psi'' + kappa G A (w' - psi) + rho I omega^2 psi = 0` from the clamp (`w = psi = 0`) to the free end (`psi' = 0`,
   `w' = psi`), bisection on the 2 x 2 determinant:

   | `L/h` | `f_EB` Hz | shear only | Timoshenko | Timoshenko / EB - 1 |
   | --- | --- | --- | --- | --- |
   | 20 (`L = 2`) | 37.776724 | 37.344601 | 37.327353 | -1.19 % |
   | 40 (`L = 4`) | 9.444181 | 9.416855 | 9.415729 | -0.301 % (rotary inertia -0.012 %) |

   At the slenderness of O6 the shear term alone puts the Euler-Bernoulli reading outside 1 percent. The check of clause (a2) is
   therefore made at `L/h = 40` (D7, C1), chosen here from this derivation before any run; the `L/h = 20` cantilever is
   compared with the Timoshenko value (C1c).
2. **Rounding probe 1** (a temporary test file run once in the worktree at dab660ed, deleted after the run, no library
   line changed): the isotropic Hex20 cantilever 20 x 1 x 1, `L = 4`, `h = 0.1`, steel, the support of D7, six modes,
   `ResidualTest::Backward` at 1e-12, with `nu = 0.3` and `nu` one ulp above. The frequencies moved by 4.7e-11, 3.2e-10,
   1.3e-11, 2.4e-12, 8.6e-13 and 2.5e-13. Those are the three degenerate pairs of the square section; the first pair moved the
   most. A change of `D` in the last bit moves the frequencies of a slender, degenerate model far beyond 1e-12, so clause (a3)
   cannot be read on that model.
3. **Rounding probe 2** (the same way): the Hex20 block 4 x 2 x 1 on `1 x 0.4 x 0.25`, clamped at `xmin` (elimination), and
   the Hex8 block 5 x 2 x 1 on the same box with `u_z = 0` at the root by linear constraints and springs of `5e7` N/m in z at the
   `xmax` nodes (transformation), each modal and pre-stressed (uniform `S_xx = 2e8` Pa), the same `nu` change. Largest moves:
   2.8e-14, 4.4e-15, 1.4e-14 and 1.7e-14. Clause (a3) is read on these two blocks (D7, C2). Probes 2 and 3 fixed the models and
   gave the estimates; no check of this registration was run.
4. **B1**: `crates/rcfd-fem/tests/sa_ortho_modal_default_bits.rs` was written with the API of main dab660ed only and run
   once at that commit (release); the 17 digests it printed are its constants `BASE` (log `f537-scratch/b1-base.log`).

## Design

Every choice below that the maintainer would normally be asked about is taken on the maintainer's standing instruction of
2026-11-11 (take the recommendation); none changes a default result, a golden or a hash table.

### D1 The per-point stiffness on the opened paths

The stiffness of every opened path is `assemble::stiffness_with_materials` of ADR 0452 D6 (an element of either variant takes
`mat_ortho::element_stiffness` with `D_q = elasticity_at(context_q)` at each point of the kind's stiffness rule; every other
element the expression of main). Its body after the check becomes a crate-private `point_stiffness(model, threads)` for the
callers that made their own entry check (the pre-stressed modes, which take solids only). For a model without the two
variants `point_stiffness` is `assemble_stiffness(model, threads, None)`, the function `stiffness_with` calls: same bits.

### D2 The entry checks

- `modal::modal_analysis_with`: `check_supported_with_materials` (ADR 0452 D6: structures, the variants on solids,
  `mat_ortho::check_model`, supports) in place of `check_supported_with_structures`; the stiffness `stiffness_with_materials`.
- `geometric::prestressed_modes` (both paths): a new crate-private `SolidModel::check_supported_solids_with_materials`
  (`check_elements(false, false, true)`, `mat_ortho::check_model`, supports): beams and shells stay refused as before; the
  stiffness `point_stiffness`; the geometric stiffness by a private `assemble_geometric` (the body of `geometric_stiffness`
  after its check: `prestress.check_fits` and the same assembly).
- `Prestress::uniform`: `check_elements_solids_with_materials` (the first two steps of the check above), so that a uniform
  pre-stress can be given to a model of the variants.
- `mpc::check_transformation_model`: `check_supported_with_materials` in place of `check_supported_with_structures`;
  `solve_static_mpc` and `modal_analysis_mpc` assemble `stiffness_with_materials`. The supports (`add_supports`), the mass and
  the transformation are unchanged.
- Not changed: the public `assemble::stiffness`, `stiffness_with` and `geometric::geometric_stiffness` keep their checks and
  refuse both variants; so does `buckling::factors` (it calls `check_supported`).

A model without the two variants passes each new check exactly where it passed the old one, with the same error otherwise
(`check_elements` visits blocks, then materials, in the same order; `mat_ortho::check_model` returns at once).

### D3 A cyclic constraint with a variant

`check_transformation_model` refuses a model with a `Cyclic` constraint and a material of either variant:
`NotYetSupported { item: "material", variant, location: "material {i}, element set '{set}', with the cyclic constraint {j}",
package: FOLLOW_UP }` (finding 4).

### D4 The temperature of a tabulated material on the dynamic paths (brief b2)

The stiffness of a `Tabulated` material on every opened path is taken at the temperature of the model's temperature field
(`Load::Temperature`) at each integration point of the stiffness rule, `T_q = T_ref + sum_i N_i(x_q) (T_i - T_ref)` (ADR 0452
D4), the same field and the same points as the static solve. The modal analysis ignores loads; it reads the temperature field
for the stiffness only (no thermal load enters an eigenproblem). The pre-stressed modes take the stiffness at that field and
the given pre-stress (`Prestress::from_static` of a static solve of the same model includes its thermal stress); the
geometric stiffness takes no material. The mass takes the base's density at every temperature (density is not tabulated,
ADR 0452 D2). A model with a `Tabulated` material and no temperature field is refused on every opened path (key
`"temperature"`, `mat_ortho::check_model`). A temperature outside a table is refused with the element and the temperature.

### D5 Paths not opened

Buckling, nonlinear, transient, harmonic with static correction, direct harmonic, cyclic harmonics, rotating, damped modes,
the public stiffness and geometric stiffness, beams and shells: each refuses both variants as before, naming the variant and
`FOLLOW_UP` (finding 3). `FOLLOW_UP` keeps its text (`"a follow-up of ADR 0452"`), which tests of ADR 0452 assert.

### D6 Records and copies (the standing decision)

`sa_ortho::r1_r2_paths` becomes an ignored record citing ADR 0291 and ADR 0457; its copy `r1_r2_paths_after_f537` (in
`sa_ortho_modal.rs`, with its helpers copied) has E1, E2, E3, E8, E9, E10 solving and E4 to E7 refused as before, and R2
unchanged. Nothing else in an existing test changes.

### D7 Models of the checks

- **Cantilever** (C1, N1): Hex20 20 x 1 x 1 (`upgrade_to_hex20` of `hex_box`), `L = 4`, `h = b = 0.1` (`L/h = 40`), the
  support of beam theory in its symmetric form: `u_x = 0` on `xmin`, `u_z = 0` at the root nodes with `z = h/2`, `u_y = 0` at
  the root nodes with `y = h/2` (each line holds two midside nodes; the section contracts freely, the six rigid motions are
  held); `E = (1.4e11, 1.4e10, 1.4e10)`, `nu = (0.3, 0.3, 0.4)`, `G = (7e9, 7e9, 5e9)`, `rho = 1600`, `Fixed` global axes (1
  along the span); four modes, `ResidualTest::Backward`, tolerance 1e-12. Modes 1 and 2 are the first bending pair (y and z;
  the model is symmetric under the swap of y and z, so the pair is degenerate up to rounding; the next mode, the second
  bending pair, is near 59 Hz, torsion near 120 Hz). C1c: the same with 10 x 1 x 1 on `L = 2` (`L/h = 20`).
- **Block** (C2, Q1, Z1, P1): Hex20 4 x 2 x 1 on `1 x 0.4 x 0.25`, every component of `xmin` held; six modes, Backward 1e-12.
- **Transformation block** (C2t, Q1t, Z1t, P1t, T1, T3): Hex8 5 x 2 x 1 on the same box; `u_x = u_y = 0` on `xmin` as
  displacement constraints, `u_z = 0` at each `xmin` node as a one-term linear constraint, and a `NodalSpring` in z of
  `5e7` N/m on the `xmax` nodes (T1, T3: the spring left out); six modes, Backward 1e-12.
- Materials: `ISO` = isotropic `E = 2e11`, `nu = 0.3`, `rho = 7850`, `alpha = 0`; `ISO_O` = orthotropic `E_i = 2e11`,
  `nu_ij = 0.3`, `G = E/(2 (1 + nu))`, `rho = 7850`, `alpha = 0`, axes `Fixed(I)` and `Fixed(rot((1, 2, 3), 0.5))`; `ORTHO` the
  constants of ADR 0452 (`E = (1.4e11, 1e10, 1e10)`, `nu = (0.3, 0.3, 0.45)`, `G = (5e9, 5e9, 3.5e9)`, `rho = 1600`,
  `alpha = 0`); `CYL` = `ORTHO` with `Cylindrical { origin: (0.5, -1, 0), axis: (0, 0, 1) }`; `TAB` = `Tabulated(ISO,
  [200, 600] K, e = [2e11, 1.6e11])` with the nodal temperature `T = 293 + 250 x` (`T_ref = 293`); `TAB_U` the same with a
  uniform `T = 443`.
- **Pre-stress**: `Prestress::uniform(S_xx = 2e8 Pa)` where a pre-stress is used, the zero pre-stress for Z1.

### D8 The inverse-iteration identity (Q1)

For a mode `(lambda, phi)` of the modal analysis, the static solve of the same model (the path opened by ADR 0452, verified
there) under the nodal forces `f = M phi` (`M = mass::consistent`, no other load; `alpha = 0`, so the temperature field adds
no thermal load) gives `u = K^-1 M phi`, and `lambda' = phi^T M phi / phi^T M u`. If the modal analysis and the static solve
use the same per-point stiffness, `lambda' = lambda` up to the square of the eigenvector error (with `phi = v_1 + sum eps_j
v_j`, `lambda'/lambda - 1 = sum eps_j^2 (1 - lambda/lambda_j) + ...`) and rounding. A stiffness evaluated otherwise (at the
element's first point, at the reference temperature, with the element-constant matrix) changes `lambda` but not `lambda'`.
On the transformation path the static solve is `solve_static_with` of the same model (it goes to `solve_static_mpc`, whose
`K_ext` has the supports), so Q1t also sees the supports.

## Checks

Limits relative to the reference value unless stated. Every check uses the frequencies in Hz of `ModalResult::frequencies`.

| Check | Clause | Quantity | Limit | Estimate |
| --- | --- | --- | --- | --- |
| C1 | a2, b1 | cantilever `L/h = 40` (D7), `ORTHO`-cantilever constants: modes 1 and 2 against `f_EB = 9.444181` Hz (with `E_1`), each `abs(f/f_EB - 1)` | `<= 0.01` | 3e-3: Timoshenko -3.01e-3 (arithmetic 1; shear -2.89e-3, rotary inertia -1.2e-4); one Hex20 across the depth is stiffer in shear than `kappa = 5/6` (ADR 0452 O6: the shear deflection 1.30 percent measured against 1.48 percent with `kappa = 5/6`, ratio 0.88), so about -2.7e-3; mesh (ADR 0385, 3.9e-4 on a deflection, so about +2e-4 on a frequency) |
| C1b | a2, b1 | the same against the Timoshenko value 9.415729 Hz | `<= 3e-3` | 4e-4 (the shear stiffening of the one-element depth, about +3.5e-4, and the mesh) |
| C1c | a2, b1 | cantilever `L/h = 20` (10 x 1 x 1, `L = 2`): modes 1 and 2 against the Timoshenko value 37.327353 Hz; recorded without a limit: against `f_EB = 37.776724` Hz | `<= 5e-3` | 1.4e-3 (the same stiffening: shear -1.14 % times 0.88); against EB about -1.05 % (recorded) |
| C2 | a3 | block, modal (elimination): `ISO_O` with `Fixed(I)` and with `Fixed(rot((1, 2, 3), 0.5))` against `ISO`: max over six modes `abs(f/f_ISO - 1)` | `<= 1e-12` | 1e-13 (probe 2: 2.8e-14 for a one-ulp change of `nu`; `ISO_O` differs from `ISO` in `D` by up to 6.4e-16, ADR 0452 O1) |
| C2p | a3, a1 | block, pre-stressed modes (`S_xx = 2e8`), both axes | `<= 1e-12` | 1e-13 (probe 2: 4.4e-15) |
| C2t | a3, a1 | transformation block (linear constraints and springs), modal, both axes | `<= 1e-12` | 1e-13 (probe 2: 1.4e-14) |
| C2tp | a3, a1 | transformation block, pre-stressed modes, both axes | `<= 1e-12` | 1e-13 (probe 2: 1.7e-14) |
| Q1 | a1 | block with `TAB` and with `CYL`: the identity of D8 for modes 1 to 3, max `abs(lambda'/lambda - 1)` | `<= 1e-9` | 1e-13 (the rounding of probe 2; `eps_j` below 1e-9, its square negligible) |
| Q1t | a1 | the transformation block with `TAB` and with `CYL` (springs and linear constraints), the same | `<= 1e-9` | 1e-13 |
| Z1 | a1 | block with `TAB` and with `CYL`: `prestressed_modes` with the zero pre-stress against `modal_analysis`: eigenvalues and modes | `==` (bits) | `==` (ADR 0390 D4: `K + 0`) |
| Z1t | a1 | the transformation block with `TAB` and `CYL`: the same, max relative eigenvalue difference | `<= 1e-12` | `==` expected (the same steps; the transformation's own path, not claimed bit-equal by ADR 0398) |
| P1 | a1, b2 | block with `TAB_U` (`T = 443` K): modal and pre-stressed (`S_xx = 2e8`) frequencies against `ISO` with `E = interpolate([200, 600], [2e11, 1.6e11], 443)` (D4: the point temperature) | `<= 1e-12` | 1e-15 (the point temperature `T_ref + sum N_i dT` differs from 443 by rounding of the partition of unity) |
| P1t | a1, b2 | the transformation block with `TAB_U`: pre-stressed modes against `ISO` at `E(443)` | `<= 1e-12` | 1e-15 |
| T1 | a1 | transformation block without springs, `ORTHO` with `Fixed(rot((1, 2, 3), 0.5))`: modal on the transformation (root `u_z` by linear constraints) against the elimination path (root `u_z` as a displacement constraint), max over six modes | `<= 1e-10` | 1e-14 (probe 2) |
| T3 | a1 | the same two models, static solve with `-1e4` N in z at every `xmax` node: max abs displacement difference / max abs displacement | `<= 1e-10` | 1e-13 |
| R1 | D6 | `r1_r2_paths_after_f537` (both variants on the ADR 0452 cantilever): E1, E2, E3, E8, E9, E10 solve; E4 to E7 refused `NotYetSupported { item: "material", variant, location: "material 0, element set 'all'", package: FOLLOW_UP }`; R2 (Shell4 static) refused as in ADR 0452 | as stated | as stated |
| R3 | a4 | both variants: `assemble::stiffness`, `geometric_stiffness`, `buckling::factors`, `nonlinear::solve`, `Integrator::new`, `damped_modes`, `RotatingPart::new` refused naming the variant, `"material 0, element set 'all'"` and `FOLLOW_UP`; a Shell4 plate with `ORTHO`: `modal_analysis_with` refused naming `Orthotropic`, `element 0 (Shell4)` and `FOLLOW_UP`; a block with `ORTHO` and a `Cyclic` constraint: `modal_analysis_with` refused (D3) naming `Orthotropic`, the cyclic constraint and `FOLLOW_UP`; a block with `TAB` without a temperature field: `modal_analysis` refused, key `temperature` | as stated | as stated |
| B1 | a5 | `sa_ortho_modal_default_bits.rs` (17 digests of main dab660ed: Hex20 modal with and without a temperature field, pre-stress from a static result and its modes and geometric stiffness, a uniform pre-stress and buckling, the transformation modal, static and pre-stressed modes with a linear constraint and a spring, Shell4 modal) against `BASE`; and the existing `sa_ortho_default_bits`, `sa_ortho` (with D6), `sa_mat_table`, `sa_scaffold`, `sa_scaffold_bits` and the rest of the `rcfd-fem` tests pass | `==` | `==` |

### Negative controls

| Control | Quantity | Expected |
| --- | --- | --- |
| N1 | C1 with the axes turned by 90 degrees about z (`E_2 = E_1/10` along the span): mode 1 over `f_EB` | `< 0.5` (about `sqrt(0.1) = 0.316`: the check sees `E_1`) |
| N2 | P1 against `ISO` at `E(T_ref = 293)` instead of `E(443)` | `> 1e-3` (about 4.0e-2: the temperature of D4 is the point's, not the reference) |
| N3 | C2 (`Fixed(I)`) with `G_12` of `ISO_O` times `(1 + 1e-6)` | `> 1e-9` (C2 resolves a change of `D` far below its limit) |

### Planned mutations (scratch copy `w49i-work/f537-scratch/mut`, the test `sa_ortho_modal`)

| Mutation | Expected to catch |
| --- | --- |
| Mu1 | `modal_analysis_with` assembles `stiffness_with` (element-constant `D`) with the opened check | Q1 (`TAB`, `CYL` refused), P1 |
| Mu2 | `modal_analysis_mpc` assembles `stiffness_with` | Q1t, C2t |
| Mu3 | `prestressed_modes` (elimination) assembles `stiffness_with` | C2p, P1, Z1 |
| Mu4 | the pre-stressed transformation leaves out `add_supports` | Z1t |
| Mu5 | the pre-stressed modes leave out the geometric stiffness when the model has a variant | C2p, C2tp |
| Mu6 | the cyclic refusal of D3 removed | R3 |
| Mu7 | `buckling::factors` checks `check_supported_with_materials` | R3 |
| Mu8 | the point temperature of the dynamic paths replaced by the reference (`MaterialPoints::context` with `dt = 0` when called from `point_stiffness`) | P1, P1t (Q1 does not: the static path shares the context) |

## Decision rule

- **done**: every check C1, C1b, C1c, C2, C2p, C2t, C2tp, Q1, Q1t, Z1, Z1t, P1, P1t, T1, T3, R1, R3, B1 met in its single
  registered run, N1 to N3 as expected, and every mutation caught by at least one registered check.
- **partial**: any registered check missed (kept as a `*_known_miss` record with its limit), a mutation not caught, or a check
  met only after an amendment made after its run.
- **todo**: the modal analysis not opened.

## Limits (what is not covered)

- Only the modal analysis (elimination and transformation), the pre-stressed modes (both paths) and the static solve on the
  transformation take the variants; buckling, nonlinear, transient, harmonic with the static correction, direct harmonic,
  cyclic harmonics and a cyclic constraint, rotating, damped modes, beams and shells refuse them.
- The beam reading of the 1 percent is made at `L/h = 40`; at `L/h = 20` (the slenderness of ADR 0452 O6) the Euler-Bernoulli
  value is 1.2 percent above the Timoshenko value, so the comparison there is with Timoshenko (C1c).
- The 1e-12 of clause (a3) holds on stocky, non-degenerate models (probes 1, 2); on a slender model with a degenerate pair the
  frequencies themselves move by 3e-10 for a change of `D` in the last bit, and no 1e-12 comparison of two different matrices
  is meaningful there.
- The temperature of a tabulated material is the static field of the model; a temperature that changes with the vibration,
  or a stiffness taken at another temperature than the static load case, is not provided. Density is not tabulated.
- Mode superposition (`HarmonicSolver` without the static correction) takes a basis as data, as before.
