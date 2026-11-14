# ADR 0495: F-582 registration: the viscoelastic material in the damped modes at a reference frequency given in the options

Status: **REGISTERED 2026-11-14, written before any code of F-582 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-582 was written, compiled or run in the repository. The files listed under "What was
read" were read; the only arithmetic done beforehand is by hand (below) and one numpy computation of the first eigenvalue of the
40-element chain (disclosed: it showed that a closed form I first wrote for that eigenvalue was wrong, so the test computes it by
inverse iteration instead). The bit record B0 of the default paths is a test file written and run on the base aa0e4d5b after this
commit and before any library code (D5). Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-582 (dep F-577), follow-up of [ADR 0491](0491-f577-registration-viscoelastic-material-in-the-direct-harmonic-solve.md)
(arithmetic and the rods) and [ADR 0485](0485-f410-registration-linear-viscoelasticity-prony-series.md) (the material,
`mat_visco::complex_moduli`). Number 0495 given by the lead. Branch `f582-work` from aa0e4d5b, worktree `w49i-work/f582`,
scratch `w49i-work/f582-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number); each registered run made once; defaults bit-identical; no
edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code, the
crate graph unchanged; no existing test edited; clean-room. `harmonic_direct.rs` and `sa_harmonic_visco.rs` are edited by
another agent (F-583) and are not touched: the rod set-up of ADR 0491 is copied into the new test file.

## What F-582 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. The damped modes take a viscoelastic material at a reference frequency given in the options: the undamped basis from the storage shear and bulk moduli at that frequency and the loss stiffness of their loss moduli projected on it. On the 40-element rod of ADR 0491 (rod A) with the reference frequency at its first resonance, the first damped root equals the root of the discrete chain with the complex modulus at that frequency within 1e-8 relative, and its loss factor 2 |Re lambda|/Im lambda equals E''/E' at that frequency within 1e-2 relative (first order in the loss). Models without the material keep their bits.

Clauses: (a0) every limit registered here; (a1) the damped modes take the material at a reference frequency given in the
options; (a2) basis from the storage moduli, loss stiffness from the loss moduli projected on it; (a3) rod A at its first
resonance: first root against the discrete chain with `E*(omega_ref)`, `1e-8`; (a4) its loss factor against `E''/E'`, `1e-2`;
(a5) models without the material keep their bits.

Readings, registered:

- **(a1) The option.** `DampedOptions::viscoelastic_reference(omega, temperature)`: a circular frequency `omega` in rad/s
  (finite, `>= 0`) and the temperature of a shifted material (`None` for an unshifted one). New fields `reference_omega:
  Option<f64>` and `reference_temperature: Option<f64>`, default `None`. Without the option the model with a viscoelastic
  material is refused exactly as before (`check_supported`, `NotYetSupported`, variant `Viscoelastic`, package
  `mat_visco::FOLLOW_UP`); the option on a model without a viscoelastic material is refused (key `"reference frequency"`).
- **(a2) Storage basis.** At `omega_ref` the material has `G*`, `K*` (`mat_visco::complex_moduli`, the shift included). The
  stiffness of a viscoelastic element is linear in the moduli: `K_e(omega) = (G*/G_0) K_e^G + (K*/K_0) K_e^K` (ADR 0491
  arithmetic 1). Its real part is the stiffness of the isotropic elastic material with shear `G'` and bulk `K'` (`E' = 9 K' G'
  / (3 K' + G')`, `nu' = (3 K' - 2 G')/(2 (3 K' + G'))`, density of the base); the undamped basis is the modal analysis of the
  model with each viscoelastic material replaced by this elastic one (a `SolidModel` clone, a crate-private builder
  `with_materials`). Its imaginary part, the loss stiffness, is `H_v = int B^T D'' B` with `D'' = 2 G'' I_dev + K'' 1 (x) 1`
  assembled over the elements of the material, `G'' = Im G*`, `K'' = Im K*`; its projection `Phi^T H_v Phi` is added to the
  existing loss stiffness `H_r` of the constant loss factors of the other materials (the equation of ADR 0450,
  `lambda^2 M + lambda C + K + i H`). Rayleigh damping `b K` acts on the storage stiffness. A loss factor in the options on a
  set of a viscoelastic material is refused (key `"loss factor"`: the material carries its own).
- **(a3) "The root of the discrete chain".** The rod of ADR 0491 arithmetic 2 (Hex8, `u_y = u_z = 0` held, `n = 40`) is the
  1-D chain of linear elements with consistent mass, exactly; its only stiffness is `D_11`, and both `K_e^G` and `K_e^K` are
  multiples of the same chain stiffness `K_c`: the complex stiffness is `(Mod*/Mod_0) K_c` with `Mod* = K* + 4 G*/3` (axial;
  `G*` for the shear rod) and `Mod_0` the base's. So the storage stiffness is `(Mod'/Mod_0) K_c` and the loss stiffness
  `(Mod''/Mod_0) K_c`, proportional to each other, and **each mode of the basis is an exact eigenvector of the full complex
  problem**: `lambda^2 = -(Mod*/rho) mu_j` for every mode `j`, where `mu_j` is the `j`-th eigenvalue of the chain per unit
  modulus and unit density (`K_c1 x = mu rho M_1 x`, `K_c1` the chain stiffness for modulus 1, `M_1` the consistent mass for
  density 1). The projected problem is therefore diagonal in the modal basis and its first root is `i sqrt(Mod*(omega_ref)
  mu_1/rho)` for **any basis size**; the truncation costs nothing for the rod (not true for a general model, whose loss
  stiffness is not proportional to its storage stiffness: Limits). "The root of the discrete chain with the complex modulus at
  that frequency" is registered as `lambda_ref = i sqrt(Mod*(omega_ref) mu_1/rho)` (principal root, `Im > 0`), with `mu_1`
  the smallest eigenvalue of the 40-element chain computed in the test by inverse iteration on the tridiagonal matrices
  (independent of the library; its agreement with the library's undamped first eigenvalue is the set-up check SU). The basis
  size is registered as 6 (`DampedOptions::new(6)`), with 2 and 39 (the most the 40 free unknowns allow) as independence
  checks.
- **(a3) "With the reference frequency at its first resonance".** `omega_ref = omega_c` of ADR 0491 arithmetic 3 (the
  resonance of the continuous rod with the storage modulus), rod A `1.9790173529e3 rad/s`. The first resonance of the discrete
  chain is `6.4e-5` relative higher (dispersion); the chain's modulus is evaluated at `omega_ref`, as the acceptance says (a
  frozen modulus), not at the root's own frequency.
- **(a4) The loss factor.** For `lambda = i omega` with `omega^2 = omega_0^2 (1 + i eta)`, `eta = Mod''/Mod'`: `2 |Re
  lambda|/Im lambda = 2 tan(atan(eta)/2)`, which is `eta (1 - eta^2/4 + ...)`: for `eta = 0.05` it is `0.049968`, relative
  `6.3e-4` below `eta` (rod A), the "first order in the loss" of the acceptance. Two clauses: the acceptance's `1e-2`
  against `E''/E'`, and the exact form `2 tan(atan(eta)/2)` within `1e-8`.
- **Other rods.** Rod B (axial, `nu = 0.3`, unequal weights `g = 0.15`, `k = 0.05`, modulus `Mod* = K* + 4 G*/3`) and rod C
  (shear, modulus `G*`) of ADR 0491, each with its own `omega_c`, on the same checks: they test the shear/bulk split of the
  loss stiffness, which rod A (equal weights) cannot. `eta_B = 0.046077`, `eta_C = 0.081081` (ADR 0491 arithmetic 3).

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0491 (whole), ADR 0485 (D1, the complex moduli, the follow-ups), ADR 0450 (D3, the
  quadratic problem with a loss stiffness), ADR 0464 (the residual vectors, which take the storage stiffness here).
- `crates/rcfd-fem/src/damped.rs` (whole), `mat_visco.rs` (`parts`, `harmonic_base`, `complex_moduli`),
  `harmonic_direct.rs` (`viscoelastic_parts`, `shear_matrix`, `bulk_matrix`: read only; the two matrix builders are private to
  that file and are written again in `damped.rs`), `model.rs` (`SolidModel` fields, `add_material`, `material_indices`,
  `Material::isotropic_elastic`), `modal.rs` (`modal_analysis_with`: needs fewer modes than free unknowns).
- Tests: `sa_harmonic_visco.rs` (the rods, the closed forms, the check helpers), `sa_harmonic_visco_default_bits.rs` (the
  digest scheme).

## Arithmetic done before the code (disclosed)

1. **The split.** As ADR 0491 arithmetic 1: `K_stiff* = (G*/G_0) K^G + (K*/K_0) K^K`. Real part: `(G'/G_0) K^G + (K'/K_0)
   K^K`, the stiffness of the isotropic elastic material of shear `G'` and bulk `K'` (the stiffness is linear in `D`).
   Imaginary part `(G''/G_0) K^G + (K''/K_0) K^K = int B^T D'' B`, `D''` as above.
2. **Equal weights in 3-D.** With `G*/G_0 = K*/K_0 = E*/E_0` the loss stiffness is `(E''/E') K'`, a constant loss factor
   `eta = E''/E'` on the storage stiffness: the model with the viscoelastic material at `omega_ref` equals the elastic model
   with `E = E'(omega_ref)`, `nu` unchanged, and `LossFactor::Constant(E''/E')`. Check DE.
3. **Frozen-modulus root of the rod.** Above (a3). The test builds both tridiagonals (`K_c1`: `A/h [1 -1; -1 1]` per element
   with the first node held; `M_1`: `A h/6 [2 1; 1 2]`) and `mu_1` by inverse iteration.
4. **Estimates.** The frozen-modulus root is exact for the rod (arithmetic 3), so the difference to the library is the rounding
   of the modal analysis, the projections and the quadratic eigen-solver; ADR 0450's rod checks were at about `1e-12`; the
   conversion `E', nu'` from `G', K'` adds `1e-16`. Estimate `1e-12` for DA/DB/DC against the acceptance's `1e-8`.
5. **Negative control of the reference frequency.** Rod A at `0.5 omega_c`: `E'(0.5 omega_c)/E'(omega_c) = (1 - w/1.25)/(1 -
   w/2) = 0.9238/0.9524 = 0.970`, so the root differs by `1.5e-2` (hand arithmetic): limit `>= 5e-3`.
6. **Negative control of the split.** Rod B with the one-part modulus `E*/E_0` on `Mod_0` in place of `Mod*`: `Mod'/Mod_0 =
   0.956` against `E'/E_0 = 0.932` (hand, `nu = 0.3`, `g = 0.15`, `k = 0.05`, half weights at `omega tau = 1`), the root
   differs by `1.3e-2`: limit `>= 5e-3`.

## Design

### D1. `damped_modes`

When some material of the model is viscoelastic **and** `reference_omega` is `Some`: (1) the option is validated (finite, `>=
0`; key `"reference frequency"`); (2) every viscoelastic material gets `complex_moduli(material, omega_ref,
reference_temperature)` (refusing a missing temperature, key `"temperature"`, and an orthotropic base, `NotYetSupported`);
(3) the elastic clone of (a2) is built; `check_supported`, `modal_analysis`, `stiffness_with`, `consistent_with` and the
residual vectors run on the clone (a J2 material is refused there as before); (4) the loss stiffness `H_v` of each viscoelastic
material is assembled on the original model's element sets (`material_indices`), projected on the basis and added into `hr`.
Every other model runs the existing code operation for operation (the new code is in branches it does not enter; `hr` is
formed by the existing expression and the new term is added only when a viscoelastic material is present). A `reference_omega`
on a model without a viscoelastic material is refused (key `"reference frequency"`) before anything else.

### D2. Model

`pub(crate) fn with_materials(&self, materials: Vec<(String, Material)>) -> SolidModel` in `model.rs` (a clone with the list
replaced; the only edit of a shared file, a few lines).

### D3. Documentation

The module doc of `damped.rs` gains the paragraph of ADR 0495; the module doc of `mat_visco.rs` names the damped modes among
the paths that take the material. `docs/reference-data.md` is not changed (no source read).

### D4. Residual vectors and Rayleigh

Both act on the storage stiffness of the clone; one run (RV1: the residual vectors with a viscoelastic material run and give
finite roots), no accuracy check.

### D5. The bit record of the default (B0)

`crates/rcfd-fem/tests/sa_damped_visco_default_bits.rs`, run on aa0e4d5b before any library code, digests (FNV-1a 64 as ADR
0485 D8) written into the file from that run. Content: `damped_modes` of the 6x2x2 cantilever (`nu = 0.3`) with Rayleigh, a
ground dashpot and `Constant(0.02)` (roots, shapes, residuals); the same with `residual_vectors(true)`; a two-material
cantilever with `Constant` loss factors on both sets; a 40-element bar (rod geometry) with `new(6)`; the refusal message of
`damped_modes` for a viscoelastic rod without the option (the F-577 refusal, unchanged).

## Checks (limits fixed now)

| Id | Clause | What | Limit | Estimate behind the limit |
|---|---|---|---|---|
| SU | a3 set-up | `mu_1` of the test's inverse iteration against the library's first undamped eigenvalue of the rod with modulus `E_0` (`omega_1^2 rho/E_0`), relative | `<= 1e-9` | `1e-13` |
| DA | a3 | rod A, 40 Hex8, `omega_ref = omega_c`, `new(6)`: first reported root against `lambda_ref`, `abs(lambda - lambda_ref)/abs(lambda_ref)` | `<= 1e-8` (the acceptance) | `1e-12` (arithmetic 4) |
| DAm | a3 | rod A with 2 and 39 modes: the same | `<= 1e-8` | `1e-12` (arithmetic 3: independent of the basis) |
| LA | a4 | rod A: `abs(2 abs(Re lambda)/Im lambda - E''/E')/(E''/E')`, `E''/E'` of `complex_moduli` at `omega_ref` | `<= 1e-2` (the acceptance) | `6.3e-4` (a4) |
| LAx | a4 (exact) | rod A: the same against `2 tan(atan(eta)/2)` | `<= 1e-8` | `1e-11` |
| DB, LB, LBx | a1, a2 (unequal weights) | rod B: as DA, LA, LAx (`eta = Mod''/Mod'` of the rod's modulus) | `1e-8`, `1e-2`, `1e-8` | `1e-12`, `6e-4`, `1e-11` |
| DC, LC, LCx | a1, a2 (shear) | rod C: as DB | `1e-8`, `1e-2`, `1e-8` | same |
| DE | a2 (3-D) | the 6x2x2 cantilever (`nu = 0.3`, one term, equal weights `0.2`, `tau = 1/omega_ref`), `omega_ref` = 0.8 of its first undamped frequency, `new(6)`, Rayleigh and a ground dashpot: the six lowest reported roots against those of the elastic model `E'(omega_ref)` with `Constant(E''/E')`, same options: `max_k abs(lambda_k - lambda_k^e)/abs(lambda_k^e)` | `<= 1e-9` | `1e-13` (arithmetic 2) |
| DF | a1 (shift) | rod B with a WLF shift (`c1 = 17.44`, `c2 = 51.6 K`, `T_ref = 373 K`) at `T = 383 K` given in the options, against rod B with `tau a_T` unshifted, first root | `<= 1e-12` | `1e-14` (as T1 of ADR 0491) |
| C0 | a1 | a viscoelastic material without terms on the rod base, `omega_ref = 1`, against the elastic base (`new(6)`): first root | `<= 1e-12` | `1e-14` (the `E'`, `nu'` conversion) |
| P1 | a1 | rod B, `threads(1)` and `threads(4)`: roots and shapes | bits equal | the assembly is per element, the algebra sequential |
| RV1 | D4 | rod B with a ground dashpot and `residual_vectors(true)`: runs, every root finite | finite | D4 |
| NC1 | negative control | rod A with `omega_ref = 0.5 omega_c`: first root against `lambda_ref` of `omega_c` | `>= 5e-3` | `1.5e-2` (arithmetic 5) |
| NC2 | negative control | the elastic rod `E'(omega_c)` (no loss) in `damped_modes`: its loss factor against `eta`, relative difference | `>= 0.5` | 1 (the loss factor is 0): LA would see a dropped loss |
| NC3 | negative control | rod B: the test's reference root with the one-part modulus `E*/E_0` against the right `lambda_ref` | `>= 5e-3` | `1.3e-2` (arithmetic 6): DB would see a wrong split |
| RF | refusals | (i) a viscoelastic rod without the option: `NotYetSupported`, `Viscoelastic`, package `FOLLOW_UP`, message equal to that of B0; (ii) the option on a model without the material: key `"reference frequency"`; (iii) `omega = -1`, `NaN`, `inf`: key `"reference frequency"`; (iv) shifted material without a temperature: key `"temperature"`; (v) `Constant(0.02)` on the set of the material: key `"loss factor"`; (vi) orthotropic base: `NotYetSupported`, location `"an orthotropic base"`; (vii) viscoelastic and J2 materials together: `NotYetSupported`, variant `J2` | each as stated | D1 |
| N1 | a5 | B0: every digest of the base aa0e4d5b | equal | D5 |
| N2 | a5 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | no shared output path changed |
| N3 | a5 | `cargo test -p rcfd-fem --release` (including the existing `sa_harmonic_visco` DM1 refusal and `sa_damped*`) | pass | D1 |

Every aggregated error is required finite (a NaN anywhere is a miss; `worst` returns infinity for it). Files:
`crates/rcfd-fem/tests/sa_damped_visco.rs` (all but N1) and `sa_damped_visco_default_bits.rs` (N1).

## Planned mutations (on a scratch copy with its own `CARGO_TARGET_DIR`, one at a time)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the loss stiffness not added (`H_v` dropped) | LA, LB, LC, LAx, DA |
| MU2 | the basis from the instantaneous base (no replacement by the storage material) | DA, DB, DC, DE |
| MU3 | `omega_ref/(2 pi)` passed to `complex_moduli` | DA, DB, DC, DE |
| MU4 | shear and bulk moduli swapped in `D''` | DB, DC (not DA: equal weights) |
| MU5 | the temperature not passed to `complex_moduli` | DF (refused), RF (iv) |
| MU6 | `D''` built from the storage `G'`, `K'` | LA, LB, LC |
| MU7 | the J2 check skipped on the clone | RF (vii) |

## Decision rule

- **done**: every check of the table met, every planned mutation caught by at least one check, fmt and clippy clean, B0 and
  the baseline hashes unchanged.
- **partial**: B0 and the baseline hashes unchanged, DA and LA (the acceptance) met, and a miss elsewhere diagnosed and
  recorded as a `*_known_miss` test.
- **todo**: otherwise (in particular any change of the default bits, or DA or LA missed).

## Limits (not covered)

- The material is frozen at one frequency: every root carries the moduli at `omega_ref`, not at its own frequency (the true
  nonlinear eigenproblem `lambda^2 M + K*(lambda) = 0` is not solved; a loop that updates the reference frequency per mode is a
  follow-up).
- The truncation of the basis is exact for the rod (loss stiffness proportional to the storage stiffness); for a general model
  the loss stiffness projected on the storage modes has off-diagonal terms and the truncation is the usual modal one
  (ADR 0450).
- The isotropic base, solid elements, the elimination path only (as ADR 0450).
