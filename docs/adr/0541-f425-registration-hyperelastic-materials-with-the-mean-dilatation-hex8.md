# ADR 0541: F-425 registration: hyperelastic materials (compressible neo-Hookean and Mooney-Rivlin) on the total Lagrangian driver, the F-bar Hex8 of ADR 0506 as the mean-dilatation element

Status: **REGISTERED 2026-11-15, written before any code of F-425 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-425 was written, compiled or run in the repository: the files listed under "What was
read" were read, and the arithmetic below was done by hand and evaluated with plain numpy prototypes in the scratch folder
(`w49i-work/f425-scratch`: `probe_hyper.py` (the energy, stress and tangent of D1 against central differences, the tangent at
rest, the small-strain remainder; log `probe_hyper.log`), `probe_small.py` (the distribution of the small-strain remainder over
random strain directions; `probe_small.log`), `probe_homog.py` (the compressible homogeneous states against the incompressible
closed forms, simple shear, the sphere quadrature; `probe_homog.log`) and `probe_sphere1d.py` (a spherically symmetric
one-dimensional model of the inflated shell on linear elements, mean dilatation or full integration, to estimate the radial
discretisation error; `probe_sphere1d.log`); no rcfd code; disclosed, their numbers quoted below). The bit record B0 of the
default paths is a test file written and run on the base after this commit and before any library code (D9). Results go in the
addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-425 (deps F-408, F-390, F-397), structural solver, advanced tier, work package SA-18 of
[ADR 0310](0310-structural-solver-advanced-tier-plan.md); the plan stub is [ADR 0328](0328-hyperelasticity-nearly-incompressible.md),
this is its registration. Number 0541 given by the lead. Branch `f425-work` from 8fcf5b40, worktree `w49i-work/f425`, scratch
`w49i-work/f425-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited
except the records-and-copies scheme of ADR 0291 (D8); no new dependency, no unsafe code, the crate graph unchanged; clean-room.
Every choice below that the maintainer would normally be asked about is taken on the maintainer's standing instruction of
2026-11-11 (take the recommendation), except the open questions at the end.

## What F-425 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. The tangent agrees with central differences within 1e-6; a rotation of the deformed state leaves the energy unchanged within 1e-10; at a strain of 1e-6 the stress equals linear elasticity with the same shear and bulk modulus within 1e-6 relative. With bulk/shear = 1e4: uniaxial nominal stress mu (lambda - lambda^-2) (neo-Hookean) and 2 (C1 + C2/lambda)(lambda - lambda^-2) (Mooney-Rivlin) for stretches 0.7 to 3, equibiaxial mu (lambda - lambda^-5), simple shear tau = mu gamma with the normal stress difference mu gamma^2, each within 0.1 percent after the registered compressibility correction. Thick spherical shell B/A = 2, neo-Hookean, inflated to the inner stretch 1.5: pressure (mu/2) ((1/l_b^4 + 4/l_b) - (1/l_a^4 + 4/l_a)) with l_b^3 = 1 + (A/B)^3 (l_a^3 - 1) (recomputed by quadrature in the registration) within 1 percent with the mean-dilatation element, the standard Hex8 on the same mesh being the registered locking control. A non-positive modulus is refused; an inverted element is an error with the element.

The roadmap note adds that the mean-dilatation B-bar Hex8 of ADR 0493 exists on the linear paths and that its F-bar form for
finite strain came with F-587 (ADR 0506); the brief says to use the B-bar/F-bar Hex8 of the nonlinear driver as the
mean-dilatation element, to adopt the step-local compressible neo-Hookean of F-607 (ADR 0523 D2, its open question 1) so that
F-611 can replace the step-local energy, not to edit `nl_transient/dg.rs` (F-611 does), and to register the agreement of the
two energies at a point if cheap. Further: (b1) register the compressibility correction of the incompressible closed forms and
the volumetric function of Mooney-Rivlin; (b2) cite sources from memory in `docs/reference-data.md`, flagged; (b3) defaults
bit-identical, the default-bits record first; `baseline_hashes` unchanged; no case-file key (open question); (b4) every
aggregated test error required finite; (b5) hunks in `nonlinear.rs` and `nl_transient.rs` small and isolated (F-617 edits them),
`contact.rs` not touched (F-601).

Clauses: **a0** every limit registered here; **a1** the tangent against central differences within `1e-6`; **a2** a rotation
of the deformed state leaves the energy unchanged within `1e-10`; **a3** at a strain of `1e-6` the stress equals linear
elasticity with the same shear and bulk modulus within `1e-6` relative; **a4** with `kappa/mu = 1e4` the homogeneous states
(uniaxial neo-Hookean and Mooney-Rivlin for stretches `0.7` to `3`, equibiaxial neo-Hookean, simple shear `tau` and `N1`)
within `1e-3` after the registered compressibility correction; **a5** the thick sphere `B/A = 2`, neo-Hookean, inner stretch
`1.5`, the pressure of the closed form (recomputed by quadrature here) within `1e-2` with the mean-dilatation element, the
standard Hex8 on the same mesh the locking control; **a6** a non-positive modulus refused; **a7** an inverted element an error
naming the element.

### Readings, registered

- **(a1)** Two levels: T1 the material tangent `D = dS/dE` of D1 against central differences of the stress of D1 (the
  constitutive point); T2 the element tangent of the driver (`nonlinear::hex8_force_and_tangent_at`, Full and BBar) against
  central differences of its internal force, which with a hyperelastic material is the gradient of the element energy.
- **(a2)** R1 at the point: `W(E(QF))` against `W(E(F))`; R2 at the element: the element energy `sum_q w_q W(E_q)` (the F-bar
  strain for BBar) of the configuration `x` and of `Q x`; companion R3: the internal force of `Q x` equals `Q` times that of
  `x` node by node.
- **(a3) The small-strain clause, and why its literal reading is expected to miss.** "The stress equals linear elasticity with
  the same shear and bulk modulus": the second Piola-Kirchhoff stress `S(E)` of D1 against `D_lin E` with `D_lin` the isotropic
  matrix of shear modulus `G = mu` (Mooney-Rivlin `2 (c10 + c01)`) and bulk modulus `K = kappa`, the strain measure being the
  driver's (Green-Lagrange, so that the comparison isolates the material). Any nonlinear energy has `S(E) = D_lin E + O(|E|^2)`;
  the relative difference is `c |E|` with `c` of order one (arithmetic 3): at `|E| = 1e-6` and `kappa/mu = 2.17` the prototype
  gives a median `2.2e-6` and a worst `4.9e-6` over random directions (max-norm relative), at `kappa/mu = 1e4` up to `7.1e-3`
  (the volumetric response `kappa (J - 1)` to the second-order `J - 1 = -tr(E^2) + ...` of a deviatoric strain). SS1 is the
  literal reading on a registered sample, and it is **expected to miss** (estimate `4.9e-6` against `1e-6`); it is registered
  as the acceptance states it, not changed. The clause's content (the linearisation is linear elasticity) is registered as SS2
  (the tangent at `E = 0` equals `D_lin`), SS3 (the odd part `(S(E) - S(-E))/2` equals `D_lin E` to third order) and SS4 (the
  remainder is quadratic). Open question 1 proposes the acceptance text.
- **(a4) The compressibility correction.** The closed forms are those of the incompressible material; the material of the
  check is compressible with `kappa/mu = 1e4`. Registered correction: the exact homogeneous solution of the compressible energy
  (D1) in principal stretches, the free stretch(es) solved from zero lateral nominal stress by Newton's method on the closed-form
  derivative `P_i = dW/dlambda_i` (arithmetic 4, written independently of D1 in the test). The correction is `Delta = P_comp -
  P_inc`; the check is `|P_FE - (P_inc + Delta)| / |P_inc| <= 1e-3`, i.e. the FE nominal stress against the exact compressible
  value relative to the closed form. The size of the correction is reported (arithmetic 4: `|Delta / P_inc| <= 4.0e-4` for the
  acceptance's cases, so the uncorrected closed forms would also be within `1e-3`; Mooney-Rivlin equibiaxial, a companion, has
  `2.4e-3` at `lambda = 3`). Simple shear with the homogeneous field prescribed on the boundary has `J = 1` exactly and the
  closed forms `tau = mu gamma`, `N1 = mu gamma^2` (neo-Hookean) hold for the compressible material without correction (`Delta =
  0`, arithmetic 4).
- **(a5)** Displacement control of the inner surface (every inner node moved radially to `1.5 |X|`), the pressure from the
  reactions by virtual work (D6: `p_FE = sum_a R_a . x_a / (3 V_cav)`, the energy-conjugate pressure `dPi/dV` of the homothetic
  family of the inner surface), against the closed form `p_c` at `l_a = 1.5`. No compressibility correction: the bound is
  arithmetic 5.
- **(a6)** The moduli of the energies are `mu` and `kappa` (neo-Hookean) and `2 (c10 + c01)` and `kappa` (Mooney-Rivlin); zero,
  negative and non-finite values refused by the constructors of ADR 0440 (they exist; the check runs them).
- **(a7)** A model whose prescribed motion inverts one element: `nonlinear::solve` returns `FemError::Element` with that
  element's index and the source key `"deformation gradient"` (Full and BBar); the element functions refuse the inverted
  element with that key; the constitutive call of D1 refuses `det C <= 0` with that key.

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0328 (stub); ADR 0506 (whole: the F-bar element D1, its arithmetic 1 to 3, the dispatch D2,
  the element entry points D5, the records scheme D7, the bit record D8); ADR 0493 (D1, the B-bar); ADR 0523 (D2 the step-local
  neo-Hookean, D5, the open questions); ADR 0440 (D2, D3, D5: the scaffold's variants, the pending refusals); ADR 0475 (D2: how
  J2 landed on the nonlinear paths only).
- `crates/rcfd-fem/src/mat_hyper.rs`, `nonlinear_md.rs` (stubs), `material.rs` (`HyperelasticEnergy`, `Material::stress`,
  `elasticity_matrix`, `state_size`), `model.rs` (`Material`, `pending`, `check_elements` and its callers), `nonlinear.rs`
  (head, `evaluate_with`, `point_update`, `fbar_kinematics`, `evaluate_fbar`, the D5 entry points, `check_model`,
  `NewtonOptions`, `attempt_with`, `solve_with`), `nl_transient.rs` (`Integrator::new`, `strain_energy`),
  `nl_transient/dg.rs` (the energy D2, `surrogate`, `energy`), `nl_transient/emc.rs` (`check_model`, `algorithmic_stress`),
  `harmonic_direct.rs` (`check_viscoelastic_model`), `follower.rs` (head), `shell_corot.rs` (`check_model`), `mesh.rs` (the mesh
  builder), `constraint.rs` (`Prescribed`), `error.rs`.
- Tests: `si_fbar.rs` (the checks harness), `sa_dg.rs` (F1h), `sa_emc.rs` (F1ec), `sa_laminate.rs` (RF2), `sa_scaffold.rs`
  (`pending_variants`, `m1_to_m4_after_f411`, `x1_modules_and_pending_after_..._f427`), `sa_dg_default_bits.rs` (the digest
  scheme), `sa_follower.rs` (the ring).
- `docs/roadmap.json` (F-425, F-587, F-611), `docs/reference-data.md` (the F-607 section).

## Arithmetic done before the code (disclosed)

1. **The energies** (first principles; the isochoric-volumetric split of Flory and of Simo, Taylor and Pister, in the form of
   Holzapfel, Nonlinear Solid Mechanics, 2000, chapter 6, cited from memory, NOT re-read; every formula below derived here). `C =
   I + 2E`, `I1 = tr C`, `I2 = ((tr C)^2 - tr C^2)/2`, `J = (det C)^(1/2)`, `Ibar1 = J^(-2/3) I1`, `Ibar2 = J^(-4/3) I2`,
   `U(J) = (kappa/2)(J - 1)^2`:
   neo-Hookean `W = (mu/2)(Ibar1 - 3) + U(J)` (exactly the form of ADR 0523 D2, adopted, brief); Mooney-Rivlin `W = c10 (Ibar1 -
   3) + c01 (Ibar2 - 3) + U(J)`, the same volumetric function (registered: one volumetric function for both energies, so that the
   two differ only in the isochoric part, and the small-strain bulk modulus is `U''(1) = kappa`). Stress `S = 2 dW/dC`, with
   `C - (I1/3) I = 2 dev E` and `I1 C - C^2 - (2/3) I2 I = 2 dev E + 4 (t E - E^2) - (4/3)(t^2 - q) I` (`t = tr E`, `q = tr E^2`;
   derived from `C = I + 2E`, both traceless, written so that no O(1) terms cancel at small strain):
   `S = 2 c10 J^(-2/3) C^(-1) (2 dev E) + 2 c01 J^(-4/3) C^(-1) [2 dev E + 4 (t E - E^2) - (4/3)(t^2 - q) I] + kappa (J - 1) J
   C^(-1)` (`C^(-1)` commutes with `E`, so each product is symmetric). Tangent `CC = 4 d^2W/dC dC` (with `d C^(-1)/dC = -C^(-1)
   (.) C^(-1)` symmetrised, written `-C^(-1) o C^(-1)`, `d J^(-2/3)/dC = -(1/3) J^(-2/3) C^(-1)`, `d J^(-4/3)/dC = -(2/3)
   J^(-4/3) C^(-1)`):
   `CC = 4 c10 J^(-2/3) [-(1/3)(I x Ci + Ci x I) + (I1/9) Ci x Ci + (I1/3) Ci o Ci]`
   `+ 4 c01 J^(-4/3) [I x I - II - (2/3) I1 (I x Ci + Ci x I) + (2/3)(C x Ci + Ci x C) + (4/9) I2 Ci x Ci + (2/3) I2 Ci o Ci]`
   `+ kappa J (2J - 1) Ci x Ci - 2 kappa J (J - 1) Ci o Ci`,
   `Ci = C^(-1)`, `II` the symmetric fourth-order identity; in the driver's Voigt layout `D[r][c] = CC_ijkl`, `r = (i, j)`, `c =
   (k, l)` in the order `(0,0), (1,1), (2,2), (0,1), (1,2), (2,0)` (engineering shear strains, tensor shear stresses, `S_I = D_IJ
   e_J`). The neo-Hookean part and the volumetric part are those of ADR 0523 D2 term by term (`c10 = mu/2`). At `E = 0`: `CC =
   2 (2 c10 + 2 c01)(II - I x I / 3) + kappa I x I`, linear elasticity with `G = 2 (c10 + c01)`, `K = kappa`. Prototype
   (`probe_hyper.log`): `D` against central differences of `S` (`h = 1e-7`) `3.3e-10` to `1.2e-9` for neo-Hookean and
   Mooney-Rivlin at `kappa/mu = 2.17` and `1e4`, strain amplitudes `1e-3`, `0.1`, `0.4`; asymmetry `<= 6e-17`; the tangent at
   rest against `D_lin` `0` (bits).
2. **The mean-dilatation element** (first principles; Simo, Taylor and Pister, Comput. Methods Appl. Mech. Engrg. 51 (1985)
   177-208, and Nagtegaal, Parks and Rice, Comput. Methods Appl. Mech. Engrg. 4 (1974) 153-177, cited from memory, NOT re-read).
   The F-bar Hex8 of ADR 0506 D1 evaluates the material at `F_bar = (J_bar / J)^(1/3) F`, `J_bar` the element mean of `det F`.
   `det F_bar = J_bar` and the isochoric part `det(F_bar)^(-1/3) F_bar = J^(-1/3) F` is that of `F`. With an energy of the split
   form `W = W_iso(Cbar) + U(J)` (both of D1), the element energy is `sum_q w_q W(F_bar,q) = sum_q w_q W_iso(Cbar_q) + V U(J_bar)`:
   the three-field element of Simo, Taylor and Pister with a constant volume ratio `J_bar` and pressure `p = U'(J_bar)` per
   element, condensed. No new element is written: the F-bar Hex8 of the driver with the material of D1 is the mean-dilatation
   element of the acceptance (check MD1), and the internal force of ADR 0506 (`f = sum_q w_q Bbar_NL^T S_bar`) is the gradient of
   this energy (ADR 0506 arithmetic 2), so its consistent tangent applies unchanged with `C_t = D` of D1 (symmetric).
3. **The small-strain remainder** (a3). Expanding D1: `S(E) - D_lin E = O(kappa |E|^2 + mu |E|^2)` with the volumetric term
   `kappa (J - 1) J C^(-1)`, `J - 1 = t - (q - t^2/2) + ...`; for a deviatoric `E` the stress is `2 mu E` and the remainder
   `-kappa q I + ...`, relative `(kappa / mu) |E| / 2`. Prototype (`probe_small.log`, 4000 random directions, `|E|` the tensor
   norm `1e-6`): `kappa/mu = 2.1667` (the moduli of `nu = 0.3`): max-norm relative remainder min `6.5e-7`, median `2.2e-6`, worst
   `4.9e-6` (neo-Hookean) and `5.2e-6` (Mooney-Rivlin `c01/c10 = 2/3`); `kappa/mu = 1e4`: worst `2.2e-3` (the engineering-shear
   direction `7.1e-3`). The odd part `(S(E) - S(-E))/2` against `D_lin E`: `3.6e-11` to `3.4e-10` (the prototype forms `C^(-1)`
   by inversion; D1 does not subtract O(1) terms). The remainder at `1e-6` over that at `1e-5`: `0.1` for a quadratic remainder.
4. **Homogeneous states** (principal stretches; the incompressible closed forms of Rivlin, of Treloar, The Physics of Rubber
   Elasticity, 3rd ed. 1975, and of Ogden, Non-linear Elastic Deformations, 1984, cited from memory, NOT re-read; re-derived
   here: uniaxial `P = dW/dlambda` with `lambda_2 = lambda_3 = lambda^(-1/2)` gives `mu (lambda - lambda^-2)` and `2 (c10 +
   c01/lambda)(lambda - lambda^-2)`; equibiaxial `lambda_3 = lambda^-2` gives `mu (lambda - lambda^-5)` and `2 (c10 + c01
   lambda^2)(lambda - lambda^-5)`; simple shear `F = I + gamma e1 x e2`, `J = 1`, Cauchy `sigma = mu dev b` plus the
   pressure `kappa (J - 1) = 0`, so `tau = mu gamma`, `N1 = sigma11 - sigma22 = mu gamma^2`, `N2 = 0`; Mooney-Rivlin `tau = 2
   (c10 + c01) gamma`, `N1 = 2 (c10 + c01) gamma^2`, `N2 = -2 c01 gamma^2`). Compressible exact solution (the correction of
   (a4)): `P_i = c10 J^(-2/3)(2 lambda_i - (2/3) I1 / lambda_i) + c01 J^(-4/3)(2 lambda_i (I1 - lambda_i^2) - (4/3) I2 /
   lambda_i) + kappa (J - 1) J / lambda_i`, the lateral stretch from `P_2 = 0` (uniaxial) or `P_3 = 0` (equibiaxial). With `mu =
   1e6`, `kappa = 1e10`, Mooney-Rivlin `c10 = 3.5e5`, `c01 = 1.5e5` (`2 (c10 + c01) = mu`), the stretches `0.7, 0.8, 0.9, 1.25,
   1.5, 2, 2.5, 3` (`probe_homog.log`): `Delta / P_inc` from `-2.7e-5` (`0.7`) to `-2.04e-4` (`3`) uniaxial neo-Hookean, `-3.2e-5`
   to `-1.53e-4` Mooney-Rivlin, `-1.06e-4` to `-4.0e-4` equibiaxial neo-Hookean, `-1.5e-4` to `-2.43e-3` equibiaxial
   Mooney-Rivlin; `|J - 1| <= 6.0e-4`. Simple shear: the ratios to the closed forms `1` within `1e-15` at `gamma = 0.1, 0.5, 1,
   2`, both energies, `N2 = 0` (neo-Hookean).
5. **The sphere** (the inflation of an incompressible thick spherical shell, Green and Zerna, Theoretical Elasticity, and Ogden
   1984, cited from memory, NOT re-read; derived here). Incompressible map `r^3 - a^3 = R^3 - A^3`, hoop stretch `lambda = r/R`,
   `lambda^3 = 1 + (a^3 - A^3)/R^3`, so `l_b^3 = 1 + (A/B)^3 (l_a^3 - 1)` and `dr/r = -dlambda / (lambda (lambda^3 - 1))`.
   Equilibrium `d sigma_r/dr = 2 (sigma_r - sigma_theta)/r`, `sigma_r(b) = 0`, `sigma_r(a) = -p`, neo-Hookean `sigma_theta -
   sigma_r = mu (lambda^2 - lambda^-4)`: `p = int_a^b 2 mu (lambda^2 - lambda^-4) dr/r = 2 mu int_{l_b}^{l_a} (lambda^-2 +
   lambda^-5) dlambda = (mu/2)((1/l_b^4 + 4/l_b) - (1/l_a^4 + 4/l_a))`, the acceptance's form. **Recomputed by quadrature**
   (`probe_homog.log`, 20-point Gauss-Legendre): in `lambda` `0.7554331187123394 mu`, in `R` (the integrand `2 mu (l^2 - l^-4)/r
   (R/r)^2`) `0.7554331187123391 mu`, the closed form `0.7554331187123393 mu`; `l_b = 1.0905176678637094`. The pressure has a
   maximum near `l_a = 1.83` (`0.8162 mu`); at `1.5` the slope is `0.45 mu` per unit stretch, so displacement control is
   well-posed and pressure control would be too. Compressibility at `kappa/mu = 1e4`: `|J - 1| ~ p/kappa ~ 1e-4`, a pressure
   change of relative order `1e-4` times a factor of order one; bound `1e-3`, below the limit by ten.
   **Radial discretisation error** (`probe_sphere1d.py`, a spherically symmetric model: linear elements in `R`, two-point Gauss,
   the element mean of `J` in the volumetric term (mean dilatation) or full integration, `kappa/mu = 1e4`, energy minimised by
   Newton's method with complex-step gradients, `p = dPi/dV` at the inner radius): mean dilatation `6.8e-4` (8 geometric layers),
   `2.7e-4` (12), `1.3e-4` (16), `O(h^2)`; full integration `8.4` (8 layers), `3.7` (12): the locking factor. The angular
   discretisation of the three-dimensional mesh (D6, an angular cell of `7.5` to `9.5` degrees) adds an error of the same order
   at an equal element size (estimate `2e-3`, not probed); total estimate `3e-3`, limit `1e-2` (the acceptance), margin about 3.
6. **Central differences (T1, T2).** Truncation `O(h^2 / l^2)`, rounding `eps |S| / (h |D|)`; at `h = 1e-7`: `1e-9` (the
   prototype's `3e-10` to `1.2e-9`). Element level (ADR 0506 arithmetic 6) at `h = 1e-7` with displacements of order `0.1` to
   `1`: rounding `1e-9`, truncation `1e-14`. Limit `1e-6`, the acceptance's.
7. **The Newton tolerance at `kappa/mu = 1e4`.** ADR 0506 arithmetic 4 met a rounding floor of the relative residual `1.14e-10`
   at `K/G = 4999` (the driver's default `1e-10`). Registered for every run with `kappa/mu = 1e4`: `residual_tolerance = 1e-8`
   (floor about `2e-10` at twice the ratio, margin 50); the error it allows in a stress is of relative order `1e-8 kappa / mu =
   1e-4` at most, below the limits of (a4) and (a5) by ten and a hundred, and below the companion H*c's `1e-6` only when the
   homogeneous problem is solved to the residual floor (it is: one element, quadratic convergence; estimate `1e-9`). Energy
   tolerance the default `1e-10`.
8. **Default bits.** The arms of `Material::stress` and `elasticity_matrix` of every other variant keep their operations; the
   new arm is taken for `Material::Hyperelastic` only; the model check gains a branch for `Hyperelastic` only; `nonlinear.rs` is
   not changed (its `point_update` already calls `Material::stress` for a material without history, so the driver takes the
   hyperelastic material by D2 alone); `nl_transient.rs` gains a refusal for `Hyperelastic` on the Method step only (D4).

## Design

### D1. `mat_hyper.rs`: the energies

`PENDING = None`; `FOLLOW_UP = "a follow-up of ADR 0541"` (the paths that do not take the material name it). Public functions of
a `HyperelasticEnergy` and a strain (Voigt, engineering shear): `energy(energy, strain) -> Result<f64>`, `energy_parts(energy,
strain) -> Result<(f64, f64)>` (the isochoric and the volumetric part), `stress(energy, strain) -> Result<[f64; 6]>`,
`stress_and_tangent(energy, strain) -> Result<([f64; 6], Matrix6)>`, `small_strain_moduli(energy) -> (f64, f64)` (`G`, `K`).
The arithmetic of arithmetic 1, evaluated without cancellation at small strain: `det C - 1 = 2t + 4 i2(E) + 8 det E` (`i2 =
(t^2 - q)/2`), `ln J = ln_1p(det C - 1)/2`, `J - 1 = expm1(ln J)`, `J^(-2/3) - 1 = expm1(-2 ln J / 3)` and `J^(-4/3) - 1`
likewise, `Ibar1 - 3 = 3 (J^(-2/3) - 1) + 2 t J^(-2/3)`, `Ibar2 - 3 = 3 (J^(-4/3) - 1) + J^(-4/3)(4t + 2t^2 - 2q)` (from `I2 = 3
+ 4t + 2t^2 - 2q`), `C^(-1)` by the adjugate over `det C`. A strain with `det C <= 0` or non-finite is refused with
`FemError::InvalidValue { key: nonlinear::INVERSION_KEY }` (`"deformation gradient"`).

### D2. `material.rs`: the constitutive call

`Material::stress` of `Hyperelastic` returns `mat_hyper::stress_and_tangent` (no state, the point context not read);
`elasticity_matrix` of `Hyperelastic` is refused `NotYetSupported { location: "the element-constant elasticity matrix", package:
mat_hyper::FOLLOW_UP }` (as J2). `state_size` stays 0.

### D3. `model.rs`: the paths

`Material::pending()` of `Hyperelastic` becomes `None` through `mat_hyper::PENDING`. `check_elements` refuses `Hyperelastic`
where it refuses J2 (`history = false`: the linear static, modal, harmonic, buckling, transient and every other linear path),
`NotYetSupported { item: "material", variant: "Hyperelastic", location: "material i, element set 's'", package:
mat_hyper::FOLLOW_UP }`. The nonlinear driver (`solve`, `solve_with`, `solve_from`, `solve_in_time`, the arc-length step, the
follower and tangent-mode paths, which check with `history = true`) takes it; the corotated shells refuse it as every material but
the isotropic one (`shell_corot::check_model`, unchanged); the direct harmonic solve refuses it through `elasticity_matrix`
(unchanged code).

### D4. `nl_transient.rs`: the Method step

`Integrator::new` refuses a `Hyperelastic` material on a step that is not the energy-momentum step with `NotYetSupported {
item: "material", variant: "Hyperelastic", location: "material i, element set 's': the nonlinear transient Method step",
package: mat_hyper::FOLLOW_UP }` (before D3 this was the pending refusal; it stays a `NotYetSupported`, so `sa_dg.rs` F1h holds
unchanged; the energy-momentum step with a hyperelastic material keeps the step-local energy of ADR 0523 until F-611). One
isolated block after `nonlinear::check_driver_model`. `dg.rs` is not edited.

### D5. `nonlinear_md.rs`

`PENDING = None`; the module documents that the mean-dilatation element of ADR 0328 is the F-bar Hex8 of ADR 0506 with the
energies of D1 (arithmetic 2) and gives `pub fn block_property() -> BlockProperty` (`BlockProperty::solid(SolidFormulation::BBar)`)
for a block that should take it. No arithmetic.

### D6. The sphere (SP1, SPL)

Mesh, registered: the octant `x, y, z >= 0` of the shell `A = 1 <= |X| <= B = 2`. Surface grid: the three faces `x = 1`, `y =
1`, `z = 1` of the unit cube, each with an `n x n` grid (`n = 6`) of uniform cells in its two face coordinates on `[0, 1]`,
projected radially to the unit sphere; nodes of the shared edges and of the corner merged (coordinate match within `1e-12`). Radii
`R_k = A (B/A)^(k/m)`, `k = 0 .. m`, `m = 8`. Elements: per face cell and layer a Hex8 (the cell's corners at `R_k` then at
`R_{k+1}`), the corner order reversed when its volume is negative. `3 x 36 x 8 = 864` elements. Material: neo-Hookean `mu =
1e6`, `kappa = 1e10`, density 1000. Block BBar (SP1) or the default Full (SPL). Boundary values, as `Prescribed` scaled by the
load factor: every inner node (`k = 0`) all three components `u = (l_a - 1) X`, `l_a = 1.5`; every other node with `X_i = 0`
(exactly, by the construction) `u_i = 0`. `nonlinear::solve` with `NewtonOptions::new(10)`, `residual_tolerance = 1e-8`,
`threads(4)`. `p_FE = sum_{inner a} R_a . x_a / (3 V_cav)` with `R` the reactions, `x = X + u`, `V_cav = |(1/3) sum_faces int x .
n dA|` over the deformed inner faces (bilinear, `2 x 2` Gauss, exact for the cubic integrand). SP1: `|p_FE - p_c mu| / (p_c mu)`.

### D7. The other set-ups

- **Homogeneous (H1 to H6).** One Hex8, the unit cube `[0, 1]^3` (`hex_box`, `[1, 1, 1]`), materials of arithmetic 4,
  formulations Full and BBar (each case run with both). Uniaxial: `u_x = 0` on `xmin`, `u_y = 0` on `ymin`, `u_z = 0` on `zmin`,
  `u_x = lambda - 1` on `xmax` (as `Prescribed` per node, scaled by the load factor), steps `ceil(|lambda - 1| / 0.1)` (Newton
  defaults with `residual_tolerance = 1e-8`). Equibiaxial: also `u_y = lambda - 1` on `ymax`; `z` free. Stress from
  `hex8_second_piola_kirchhoff` at the solution, `P11 = lambda S11` (the deformation is diagonal), the lateral stretch from the
  node `(1, 1, 1)`. Simple shear: the `2 x 2 x 2` box of the unit cube, every boundary node `u = gamma Y e1` prescribed, the
  centre node free; set-up check: the centre node's displacement equals `gamma Y e1` within `1e-10 gamma`; `F = I + gamma e1 x
  e2`, `sigma = F S F^T / J` at every point of every element; `tau = sigma12`, `N1 = sigma11 - sigma22`, `N2 = sigma22 -
  sigma33`; `gamma = 0.1, 0.5, 1, 2`, steps `ceil(gamma / 0.1)`. Every measure the worst over the points.
- **T1, R1, SS*, EC1** at the point: the energies `NH(mu = 1e6, kappa = 2.1667e6)`, `NH(1e6, 1e10)`, `MR(3.5e5, 1.5e5,
  2.1667e6)`, `MR(3.5e5, 1.5e5, 1e10)`; strains from a seeded generator (xorshift64*, seed `0xF425_0541`), `E = amp (G + G^T)/2`
  of a standard normal `G`, amplitudes `1e-3`, `0.1`, `0.4`, a draw with `det C <= 0` skipped and counted (set-up: at least 8
  of 10 kept per amplitude). R1: `F = I + 0.3 G'`, `det F > 0`, `Q` the rotation by `0.7, 1.9, 3.0` rad about the axes `(1, 2,
  3)/|.|`, `(0, 0, 1)`, `(1, -1, 0.5)/|.|`.
- **T2, R2, R3, MD1** at the element: the distorted Hex8 of ADR 0506 T1 (unit cube, corners moved by `0.1 (2U - 1)`, seed
  `0xF425_0542`), formulations Full and BBar, energies `NH(1e6, 2.1667e6)`, `NH(1e6, 1e10)`, `MR(3.5e5, 1.5e5, 1e10)`;
  displacements: (a) random amplitude `0.1`; (b) a rotation by `pi/2` about `z` plus random amplitude `0.05`; (c) the stretch
  `diag(1.6, 0.8, 0.8) - I` plus random amplitude `0.02`. `h = 1e-7`.

### D8. Records and copies

`sa_scaffold.rs::m1_to_m4_after_f411` asserts `Hyperelastic` pending with `"ADR 0328 (F-425)"` and its constitutive call
refused; `x1_modules_and_pending_after_f415_..._f427` asserts `mat_hyper::PENDING` and `nonlinear_md::PENDING`. Both fail by
design after D1 and D5. Each gets `#[ignore = "record ...: ADR 0541 (F-425) landed ..., continued in ..._after_f425 (ADR 0291
pattern, maintainer standing decision)"]` and a copy `m1_to_m4_after_f425` (Hyperelastic `pending() == None`, its M4 row: the
element-constant matrix refused naming `mat_hyper::FOLLOW_UP`, the constitutive call at `1e-4` equal in bits to
`mat_hyper::stress_and_tangent`) and `x1_modules_and_pending_after_..._f427_f425` (the two rows `None`). Any other existing test
that asserts the pending refusal of `Hyperelastic` is treated by the same scheme and listed in the addendum. No other existing
test is edited.

### D9. The bit record B0 (`crates/rcfd-fem/tests/sa_hyper_default_bits.rs`)

Written after this commit and run on the base 8fcf5b40 before any library code; FNV-1a 64 digests, the expected values printed
by that run and written into the file then. Content (API of the base only): `Material::stress` and `elasticity_matrix` of the
isotropic, orthotropic (fixed axes), tabulated (at a temperature), J2, viscoelastic and creep materials at a strain (bits of the
results, the messages of the errors); `nonlinear::solve` of a stretched and rotated Hex8 box, Full and BBar (displacement,
reactions, residuals); `solve_static` of a Hex8 cantilever (displacement); the refusal messages of `solve_static` for the J2,
viscoelastic and creep materials; `nl_transient` on the Method step (isotropic bar, 5 steps: state bytes, energies); the
energy-momentum step with the neo-Hookean material and `DiscreteGradient` (the cube of `sa_dg.rs`, 5 steps: state bytes, the
strain energy).

## Checks

Every measure is required finite before it is compared (a non-finite value misses). "Relative" is to the largest absolute entry
of the reference unless stated.

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| T1 | a1 | D7 point: `max |D - D_fd| / max |D|`, `D_fd` central differences of `mat_hyper::stress`, `h = 1e-7`, four energies, three amplitudes | `<= 1e-6` each | `1.2e-9` |
| T1s | a1 | the same: `max |D - D^T| / max |D|` | `<= 1e-12` | `6e-17` |
| T2 | a1 | D7 element: `max |K - K_fd| / max |K|` of `hex8_force_and_tangent_at`, Full and BBar, three energies, states (a) to (c) | `<= 1e-6` each | `1e-8` |
| R1 | a2 | D7 point: `|W(E(QF)) - W(E(F))| / |W(E(F))|`, 10 `F`, three `Q`, four energies | `<= 1e-10` | `1e-15` |
| R2 | a2 | D7 element: element energy of `Q x` against that of `x`, relative, Full and BBar, three energies, three `Q`, states (a) to (c) | `<= 1e-10` | `1e-15` |
| R3 | a2 | the same: `max_a |f_a(Q x) - Q f_a(x)| / max |f|` | `<= 1e-10` | `1e-14` |
| SS1 | a3 | 64 seeded directions, `|E| = 1e-6` (tensor norm): `max |S(E) - D_lin E| / max |D_lin E|`, `G`, `K` of `small_strain_moduli`, energies `NH(1e6, 2.1667e6)` and `MR(3.5e5, 1.5e5, 2.1667e6)` | `<= 1e-6` (the acceptance) | `4.9e-6`: **expected miss** (arithmetic 3) |
| SS2 | a3 | the tangent at `E = 0` against `D_lin`, `max |D(0) - D_lin| / max |D_lin|`, four energies | `<= 1e-14` | `0` |
| SS3 | a3 | SS1's directions: `max |(S(E) - S(-E))/2 - D_lin E| / max |D_lin E|`, four energies | `<= 1e-8` | `3.4e-10` |
| SS4 | a3 | SS1's remainder at `|E| = 1e-6` over that at `1e-5` (worst direction of each), the energies of SS1 | in `[0.08, 0.12]` | `0.1` |
| SSr | a3 | SS1 at `kappa/mu = 1e4` (`NH(1e6, 1e10)`), reported | none | `2e-3` |
| H1 | a4 | uniaxial neo-Hookean, 8 stretches, Full and BBar: `|P_FE - (P_inc + Delta)| / |P_inc|` | `<= 1e-3` each | `1e-9` |
| H2 | a4 | uniaxial Mooney-Rivlin, the same | `<= 1e-3` each | `1e-9` |
| H3 | a4 | equibiaxial neo-Hookean, the same | `<= 1e-3` each | `1e-9` |
| H4 | a4 | simple shear neo-Hookean, 4 `gamma`, Full and BBar: `|tau - mu gamma| / (mu gamma)` and `|N1 - mu gamma^2| / (mu gamma^2)` | `<= 1e-3` each | `1e-9` |
| H1c to H3c | a4 | H1 to H3 against the exact compressible value relative to it (`|P_FE - P_comp| / |P_comp|`) | `<= 1e-6` each | `1e-9` |
| H4c | a4 | H4 with `|N2| / (mu gamma^2)` | `<= 1e-6` | `1e-12` |
| H5 | a4 | equibiaxial Mooney-Rivlin (companion), after the correction | `<= 1e-3` each | `1e-9` |
| H6 | a4 | simple shear Mooney-Rivlin (companion): `tau`, `N1`, `N2 = -2 c01 gamma^2`, relative to their closed forms | `<= 1e-3` each | `1e-9` |
| H0 | a4 | set-ups: lateral `|S22|, |S33|` (uniaxial), `|S33|` (equibiaxial) at most `1e-6` of `|S11|`; the shear centre node within `1e-10 gamma`; `Delta` reported | as stated | |
| SP0 | a5 | the test's 20-point Gauss quadrature of `2 int (lambda^-2 + lambda^-5) dlambda` against the closed form | `<= 1e-12` relative | `2e-16` |
| SP1 | a5 | D6, BBar: `|p_FE - p_c| / p_c` | `<= 1e-2` | `3e-3` |
| MD1 | a5 | D7 element, BBar, the three energies, states (a) to (c): `|sum_q w_q W(E_bar,q) - (sum_q w_q W_iso(E_q) + V U(J_bar))|` relative | `<= 1e-12` | `1e-15` |
| RF1 | a6 | `neo_hookean` with `mu` or `kappa` in `{0, -1, NaN, inf}`, `mooney_rivlin` with `c10 + c01 <= 0` (`(0, 0)`, `(-1e5, 5e4)`), `kappa` in `{0, -1, NaN}`, non-finite `c10`, `c01`: refused (`InvalidMaterial`) | every row | ADR 0440 |
| INV1 | a7 | `[0, 2] x [0, 1]^2`, two Hex8, every node prescribed, the nodes at `x = 2` moved by `u_x = -1.5`: `nonlinear::solve` (one step) returns `FemError::Element { element: 1, source: InvalidValue { key: "deformation gradient" } }`, Full and BBar, NH and MR | exact | D1, ADR 0404 |
| INV2 | a7 | `hex8_force_and_tangent_at` of an inverted Hex8 (Full, BBar) and `mat_hyper::stress` at `E11 = -0.6`: `InvalidValue { key: "deformation gradient" }` | exact | D1 |
| RF2 | D3, D4 | a Hex8 cube with the neo-Hookean material: `solve_static`, `modal::solve` (or the modal entry point), `Material::elasticity_matrix` refused `NotYetSupported { variant: "Hyperelastic", package: mat_hyper::FOLLOW_UP }`; `nl_transient::Integrator::new` on the Method step the same; on the energy-momentum step with `DiscreteGradient` accepted (ADR 0523) | exact | D2 to D4 |
| EC1 | D1 | D7 point: `max |S - dW/de| / max |S|`, `dW/de` central differences of `mat_hyper::energy` (`h = 1e-6`), amplitudes `0.1`, `0.4` | `<= 1e-6` | `4e-10` |
| DG1 | brief | the energy-momentum integrator (`DiscreteGradient`) of a one-element neo-Hookean model started at a displacement (state (c) of D7 on the unit cube, `xmin` held): `strain_energy()` (the step-local energy of ADR 0523) against `sum_q w_q mat_hyper::energy(E_q)` | `<= 1e-12` relative | `1e-15` |
| RC1 | D8 | the copies `m1_to_m4_after_f425` and `x1_modules_and_pending_after_..._f427_f425` pass | passes | D8 |
| B0 | b3 | `sa_hyper_default_bits.rs`: every digest equal to the base's | bit-equal | D9 |
| S1 | b3 | `cargo test -p rcfd-fem --release` | 0 failed | |
| S2 | b3 | `cargo test -p rcfd-api --release --test baseline_hashes`; `cargo test -p rcfd-cli --release --test docs_site` | pass | |
| S3 | | `cargo fmt -p rcfd-fem --check`, `cargo clippy -p rcfd-fem --all-targets -- -D warnings` | clean | |

### Negative controls

| Id | Control | Expected |
|---|---|---|
| SPL | D6 with the default Full Hex8 (the acceptance's locking control): SP1's measure | `>= 0.1` (1D probe `8.4`); a failed solve is recorded as the control not measured |
| N2 | SS1's measure with `D_lin` of `G = 1.1 mu` (a wrong shear modulus) | `>= 1e-2` |
| N3 | H1's measure against the uncorrected closed form is reported; and with the Mooney-Rivlin closed form evaluated at `c01 = 0` for the H2 data | H2 data `>= 1e-2` at `lambda = 3` |

### Planned mutations (on a scratch copy, each run once against `sa_hyper.rs`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the term `(I1/3) Ci o Ci` of the `c10` tangent dropped | T1, T2 |
| MU2 | the sign of `(2/3)(C x Ci + Ci x C)` of the `c01` tangent flipped | T1 (Mooney-Rivlin) |
| MU3 | the volumetric `kappa J (2J - 1)` replaced by `kappa J^2` | T1, T2 |
| MU4 | the stress with `J^(-1/3)` in place of `J^(-2/3)` in the `c10` part (tangent and energy unchanged) | T1, EC1, H1 |
| MU5 | the `c01` part dropped from the stress and the tangent | H2, EC1 |
| MU6 | the energy's volumetric part `(kappa/2)(ln J)^2` (stress unchanged) | EC1, DG1 |
| MU7 | the energy with the engineering shear taken as the tensor shear | R1, EC1 |
| MU8 | the refusal of D4 removed | RF2 |
| MU9 | the refusal of D3 removed | RF2 |
| MU10 | the inversion test of D1 removed (`det C <= 0` passes) | INV2 |

## Decision rule

- **done**: every check above met in its single registered run (T1, T1s, T2, R1 to R3, SS1 to SS4, H0 to H6 with their
  companions, SP0, SP1, MD1, RF1, INV1, INV2, RF2, EC1, DG1, RC1, B0, S1 to S3), the controls as stated, every planned mutation
  caught by a registered check, `baseline_hashes` unchanged.
- **partial**: the code lands with B0, S1 and the checks of (a1), (a2), (a6), (a7) met, and a check of (a3), (a4) or (a5) is
  missed (recorded as a `*_known_miss` test with the limit intact, diagnosed), or a control does not behave as stated, or a
  mutation is not caught. SS1 is expected to miss (arithmetic 3); if it does, the item is partial by this rule, with the
  clause's first-order content met by SS2 to SS4 if they are.
- **todo**: B0 or S1 fails and cannot be fixed without changing a default result, or a check of (a1), (a2), (a6) or (a7) is
  missed.

## Limits (not covered)

- The material is taken by the nonlinear driver only (static, quasi-static in time, arc-length, follower, tangent modes); the
  linear paths refuse it naming `mat_hyper::FOLLOW_UP` (a linearised hyperelastic material is the isotropic one of its
  small-strain moduli); the nonlinear transient Method step refuses it; the energy-momentum step keeps the step-local energy of
  ADR 0523 (F-611).
- The arc-length step, the follower contributions and the tangent modes take the material through the driver's assembly without
  checks of their own here.
- No case-file key (`[material] kind = "hyperelastic"` would change the schema golden; open question 3).
- Mooney-Rivlin with one negative constant is accepted by the constructor of ADR 0440 (only `c10 + c01 > 0` is required); its
  stability (convexity) is not checked.
- The mean-dilatation element is the Hex8 one (the F-bar of ADR 0506); tetrahedra and Hex20 take the standard element; no
  mixed displacement-pressure element.
- The sphere is one mesh (D6); the convergence of the three-dimensional error with the mesh is not measured (the 1D probe gives
  the radial order).

## Open questions (with the recommendation)

1. **The small-strain clause (a3).** Literal (SS1): the second-order remainder `c |E|` of any nonlinear energy is `2e-6` to
   `5e-6` at `|E| = 1e-6` for compressible moduli and up to `7e-3` at `kappa/mu = 1e4`, so "within 1e-6 relative at a strain of
   1e-6" cannot hold for a general strain direction. Options: (a) keep the text, the item partial on SS1 (taken here, as the
   rules require); (b) amend the acceptance to "the tangent at zero strain equals linear elasticity within 1e-14 and the stress at
   a strain of 1e-6 within 1e-5 relative (kappa/mu of order one)"; (c) "the first-order part within 1e-8". Recommend (b), with
   SS2 to SS4 as its checks.
2. **The nonlinear transient Method step** (D4): refused here. Recommend a follow-up that opens it (the strain energy of
   `Integrator::strain_energy` from `mat_hyper::energy`, the Mooney-Rivlin energy included), together with F-611.
3. **A case-file key** `[material] kind = "hyperelastic"` with `energy = "neo_hookean" | "mooney_rivlin"`, `mu`, `kappa`, `c10`,
   `c01`: not added (it changes the schema golden). Recommend adding it with the block property `formulation = "bbar"` of ADR
   0506 open question 3, so that a nearly incompressible block can be set up from a case file.
4. **Citations.** Holzapfel 2000, Ogden 1984, Treloar 1975, Green and Zerna, Simo, Taylor and Pister 1985, Nagtegaal, Parks and
   Rice 1974, Rivlin (the simple shear normal stresses, late 1940s) and Bonet and Wood (Nonlinear Continuum Mechanics for Finite
   Element Analysis, the compressible neo-Hookean) are cited from memory, NOT read; volumes, pages and years as recalled.
   Recommend the maintainer checks them; every formula used is derived here.

## Amendment 1 (2026-11-15, written after the code and before any run of `sa_hyper.rs`)

Set-up details fixed while writing the test, before its single registered run; no limit changes.

- **DG1.** The displacement of state (c) of D7 is not zero at `xmin` (`u_y = -0.2 y`, `u_z = -0.2 z` there), so "`xmin` held"
  cannot hold with it. Corrected: the one-element unit cube without constraints (the free cube of `sa_dg.rs`), started at the
  displacement of state (c) with zero velocity.
- **INV1, INV2.** The energies `NH(1e6, 2.1667e6)` and `MR(3.5e5, 1.5e5, 2.1667e6)`. The inverted element of INV2 is the
  distorted Hex8 of D7 with the nodes of positive reference `x` moved by `u_x = -1.5 x`.
- **R1 set-up.** At least 5 of the 10 draws of `F` with `det F > 0` per energy.
- **RF2.** One more row: `nonlinear::solve` of the same model (two load steps) succeeds.
- **N3.** The smallest relative difference over the points of the element.
- **Pre-check (disclosed, not a registered run).** After the code and the copies of D8, the existing tests that name the
  hyperelastic material or the pending states were run once to find other records needed by D8: `sa_dg`, `sa_emc`,
  `sa_laminate`, `sa_dg_default_bits`, `sa_emc_default_bits`, `si_hex20`, `si_scaffold`, `sa_scaffold` (log
  `f425-scratch/precheck-affected.log`): all pass, so D8 needs only the two copies of `sa_scaffold.rs`.
