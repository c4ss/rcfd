# ADR 0491: F-577 registration: the viscoelastic material in the direct harmonic solve, by its complex shear and bulk moduli at each frequency; the damped modes refuse it

Status: **REGISTERED 2026-11-14, written before any code of F-577 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-577 was written, compiled or run in the repository: the files listed under "What was
read" were read, and the arithmetic below was done by hand and evaluated with a plain numpy prototype in the scratch folder
(`w49i-work/f577-scratch/probe.py`, log `probe.log`; no rcfd code; disclosed, its numbers quoted below). The bit record B0 of the
default paths is a test file written and run on the base e90c2350 after this commit and before any library code (D6). Results
go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-577 (deps F-410, F-412, F-414), follow-up 2 of [ADR 0485](0485-f410-registration-linear-viscoelasticity-prony-series.md).
Number 0491 given by the lead. Branch `f577-work` from e90c2350, worktree `w49i-work/f577`, scratch `w49i-work/f577-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number); each registered run made once; defaults bit-identical; no
edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code, the
crate graph unchanged; no existing test edited; clean-room.

## What F-577 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. The direct harmonic solve takes a viscoelastic material directly, with its complex shear and bulk moduli at each frequency (unequal weights included). The tip response of a clamped bar of the standard solid under a harmonic axial force at 10 frequencies around its first resonance equals the closed form of the rod with the complex modulus E*(omega) within 1e-6 relative at 40 elements, or within a registered discretisation limit. For equal weights the result equals the LossFactor::Table route of ADR 0485 at the table points within 1e-12. The damped modes refuse the material or take it at a registered fixed frequency.

The brief adds: (b1) probe the 40-element discretisation error of the rod against the closed form before registering, and if
`1e-6` is out of reach register a limit derived from the probe and the element order; (b2) models without the material keep
their bits, `baseline_hashes` unchanged, a default-bits test at the base before any library change; (b3) probe a quantity before
a `1e-12`-class limit, `1e-11` for sums over about `1e3` terms; every aggregated error required finite (an `f64::max` over NaN
hides a failure); (b4) another agent edits `material.rs` (F-411): edits there minimal (none are planned).

Clauses: (a0) every limit registered here; (a1) the direct harmonic solve takes the viscoelastic material directly with its
complex shear and bulk moduli at each frequency, unequal weights included; (a2) the tip response of a clamped bar of the
standard solid under a harmonic axial force at 10 frequencies around its first resonance against the closed form of the rod with
the complex modulus, `1e-6` at 40 elements or a registered discretisation limit; (a3) equal weights: equal to the
`LossFactor::Table` route at the table points within `1e-12`; (a4) the damped modes refuse the material or take it at a
registered fixed frequency.

Readings, registered:

- **(a1) "Directly".** The material is given to the model (`SolidModel::add_material`) and `DirectHarmonic::new` takes it
  without a loss factor or a table in the options; the coefficients at each frequency come from `mat_visco::complex_moduli` at
  that frequency (exact, no interpolation).
- **(a2) The rod and its modulus.** The clamped bar is the arrangement of ADR 0448 and ADR 0485 V1: `hex_box([0;3], [L, B,
  B], [n, 1, 1])`, `L = 1 m`, `B = 0.05 m`, `u = 0` at `x = 0`, the lateral components `u_y = u_z = 0` at every node (a
  one-dimensional rod, arithmetic 2), the force `F = 1 N` as `F/4` on each node of `xmax`. With the lateral motion held the
  rod's stress state is uniaxial strain, and its modulus is the constrained modulus `M*(omega) = K* + 4 G*/3`. With `nu = 0`
  and equal weights `M* = E*` exactly (arithmetic 1): that is the acceptance's case (rod A). The unequal weights the acceptance
  includes cannot keep `M* = E*` (with `g != k` the ratio `M*/E*` depends on the frequency), so for unequal weights the closed
  form is the same rod formula with the rod's own modulus `M*` (rod B), and a shear rod (rod C, the same mesh with `u_x = u_z
  = 0` held, the force along `y`, modulus `G*`) checks the shear part on its own. Reading: "the complex modulus `E*(omega)`" of
  the acceptance is the modulus of the rod's one-dimensional state; for rod A it is `E*`.
- **(a2) "The standard solid".** One Prony term (one Maxwell branch in parallel with a spring): rod A `w = 2/21` (shear and
  bulk), rod B and C `g = 0.15`, `k = 0.05`; `tau = 1/omega_c` with `omega_c = (pi/(2 L)) sqrt(Mod'(omega_c)/rho)` explicit
  because `omega_c tau = 1` fixes `Mod'(omega_c)` (arithmetic 3). Then `omega_c` is the first resonance of the continuous rod
  with the storage modulus (the loss peak of the branch sits on it: rod A `eta = 0.05` exactly, as ADR 0485 V1).
- **(a2) "10 frequencies around its first resonance".** `omega_k = omega_c (0.9 + 0.2 k/9)`, `k = 0 .. 9` (from `0.9` to
  `1.1 omega_c`).
- **(a3) "At the table points".** The table of `mat_visco::modulus_table(material, f, None)` at `f_k = omega_k/(2 pi)` (the
  ten points, the same expression as the solver's `omega / (2.0 * PI)`, so the solver evaluates the table exactly at its
  points), given to the direct harmonic solve of the base material by `LossFactor::Table` on the set `all`, against the direct
  harmonic solve of the viscoelastic material, both at `omega_k`.
- **(a4)** The damped modes refuse the material (taken; the fixed-frequency alternative is proposed as a follow-up). They do so
  today through `SolidModel::check_supported` (`NotYetSupported`, variant `Viscoelastic`, package `mat_visco::FOLLOW_UP`); the
  refusal is kept as it is and checked.

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0485 (whole: D1 the material, D7 the harmonic table, the remaining questions and the
  follow-up), ADR 0448 (the checks H1, H2, H7 and its 1-D chain reference), ADR 0450 (the options, the loss factors).
- `crates/rcfd-fem/src/harmonic_direct.rs` (whole), `damped.rs` (to `damped_modes`), `mat_visco.rs` (whole), `material.rs`
  (`elasticity_matrix`, `isotropic_elasticity`), `model.rs` (`Material::Viscoelastic`, `viscoelastic`, `check_elements` and its
  callers, `density`), `assemble.rs` (`assemble_with`, `stiffness_with`), `harmonic.rs` (`HarmonicResponse::stress`).
- Tests: `sa_harmonic_direct.rs` (the bar, `chain_tip`, `tip_load`), `sa_visco.rs` (materials, the rod of V1),
  `sa_visco_default_bits.rs` (the digest scheme).

## Arithmetic done before the code (disclosed)

1. **The split of the stiffness** (first principles; the correspondence principle of linear viscoelasticity, the steady
   harmonic state of ADR 0485 arithmetic 2). The isotropic base `D_0 = D_G + D_K` with `D_G = 2 G_0 I_dev` (in Voigt with
   engineering shear strain: `2 G_0 (delta_ij - 1/3)` on the normal block, `G_0` on the shear diagonal) and `D_K = K_0 1 (x)
   1`; `G_0 = E/(2 (1 + nu))`, `K_0 = E/(3 (1 - 2 nu))` (as `complex_moduli` computes them). In the steady state the stress
   amplitude is `sigma* = 2 G* dev eps* + K* tr eps* 1 = (G*/G_0) D_G eps* + (K*/K_0) D_K eps*`, so the element stiffness,
   linear in `D`, is `K_e(omega) = (G*/G_0) K_e^G + (K*/K_0) K_e^K`, `K_e^X = int B^T D_X B`: exact for every element of the
   linear path, no interpolation. For equal weights `G*/G_0 = K*/K_0 = E*/E_0 = r*(omega) = w_inf + sum w_i i x_i/(1 + i x_i)`
   and the material is one part, `r* K_e` with `K_e` the base's stiffness: the form of `LossFactor::Table`, whose coefficient
   is `s (1 + i eta)`, `s = E'/E`, `eta = E''/E'`, i.e. `E*/E` again (arithmetic 5). For `nu = 0` and equal weights the
   constrained modulus `K_0 + 4 G_0/3 = E_0` and `M* = r* E_0 = E*`.
2. **The rod in the 3-D mesh** (first principles, as ADR 0448 for its bar). With one Hex8 across, `u_y = u_z = 0` held at
   every node and the force spread equally on the four `xmax` nodes, the four nodes of a section move together, `u_x(x)` is
   linear in each element and independent of `y`, `z`; the only strain is `eps_xx` and the only modulus `M = D_11`. The
   trilinear stiffness and consistent mass summed over a section are those of the two-node rod element, `A M/h [1 -1; -1 1]`
   and `rho A h/6 [2 1; 1 2]`: the Hex8 rod is the 1-D chain of `n` linear elements with consistent mass, exactly. The shear rod
   (`u_x = u_z = 0` held, `u_y(x)`) alike with the only strain `gamma_xy` and `D_44 = G`.
   **Closed form of the continuous rod** clamped at `x = 0`, force `F` at `x = L`: `u = C sin(kx)`, `k = omega sqrt(rho/Mod*)`
   (principal complex root), `Mod* A u'(L) = F`: `u(L) = F tan(kL)/(A Mod* k)`.
3. **The standard solid's resonance.** `Mod*(omega) = Mod_0 (1 - w + w i omega tau/(1 + i omega tau))` per part; at `omega
   tau = 1` the branch factor is `(1 + i)/2`, so `Mod'(1/tau) = Mod_0 (1 - w/2)` per part. Rod A: `E'(omega_c) = (20/21)
   E_0`, `omega_c = (pi/2) sqrt((20/21) E_0/rho) = 1.9790173529e3 rad/s`, `eta = 0.05`. Rod B (`nu = 0.3`): `M' = K_0 (1 -
   k/2) + (4/3) G_0 (1 - g/2)`, `omega_c = 2.3004337451e3 rad/s`, `eta = 0.046077` (probe). Rod C: `G' = G_0 (1 - g/2)`, `omega_c
   = 1.2095628628e3 rad/s`, `eta = 0.081081` (probe). `rho = 1200 kg/m^3`, `E_0 = 2e9 Pa` (the values of ADR 0485 V1; inputs,
   not data).
4. **The discretisation error at 40 elements** (probe: the 1-D chain of arithmetic 2 solved densely against the closed form, the
   ten frequencies of each rod). Max over the ten frequencies of `|u_h(L) - u(L)|/|u(L)|`:

   | rod | n = 20 | n = 40 | n = 80 | order 20 to 40 | order 40 to 80 |
   |---|---|---|---|---|---|
   | A | 9.914e-3 | **2.472e-3** | 6.176e-4 | 2.004 | 2.001 |
   | B | 1.062e-2 | **2.646e-3** | 6.609e-4 | 2.004 | 2.001 |
   | C | 6.409e-3 | **1.601e-3** | 4.001e-4 | 2.001 | 2.000 |

   The largest error is at `omega_c` (the peak). `1e-6` is out of reach at 40 elements: the error is the dispersion of the
   linear element with consistent mass, which raises the discrete first frequency by `(kh)^2/24` (`kh = (pi/2)/n`; `6.43e-5` at
   40), and at the resonance peak of a loss factor `eta` the response moves by about twice the relative frequency shift over
   `eta`: `2 (kh)^2/(24 eta) = 2.57e-3` (A), `2.79e-3` (B), `1.59e-3` (C), against the probe's `2.47e-3`, `2.65e-3`, `1.60e-3`.
   `1e-6` would need about 2000 elements (second order). **Registered discretisation limit: `4e-3` at 40 elements** for each
   rod (the first-order estimate of the largest of them, `2.79e-3`, with a margin of 1.4 for the higher-order terms; the probe's
   largest is `2.65e-3`), together with the observed order `log2(e_40/e_80) >= 1.9` (the element is second order) and the
   comparison with the discrete chain (below), which together say that the whole difference is the element's dispersion.
5. **The equal-weight route against the table route** (a3). Both assemble one part per material and solve `A = c K - omega^2 M`
   with the same `K` and `M` patterns. The table route's coefficient: `s = E'/E`, `c_t = (s, s eta)`, `eta = E''/E'`; the
   viscoelastic route's: `c_v = E*/E` (D1). They differ by a few units of rounding (`s eta` against `E''/E`). The base stiffness
   of the table route comes from `stiffness_with` (the element loop of `assemble_stiffness`); the viscoelastic route assembles
   `K` by `assemble_with` with `isotropic_elasticity(E, nu)` of the base and `element_for(kind).stiffness`, the same arithmetic
   if `assemble_stiffness` does that for a Hex8 (not checked before the run; if not, the entries differ by rounding). A relative
   change `delta` of the coefficient moves the response by about `delta |c K x|/|A x| <= delta / eta`-ish near the peak:
   `1e-15 / 0.05 = 2e-14`; entry-wise rounding of `K` moves it by at most the condition number times `1e-16`, about `1e-11` at
   the worst for this rod (ADR 0448 H2 measured `1.3e-11` against the chain at 80 elements and `eta = 0.02`) but in practice the
   rounding of a sum of eight Hex8 Gauss terms reproduced identically when the loops are the same. The limit `1e-12` is the
   acceptance's; the estimate is `1e-14` if the assemblies agree to the bit and up to `1e-11` if they do not (a known risk,
   registered: a miss is recorded with its number and diagnosed, not tuned).
6. **The discrete chain as a set-up check.** The 1-D chain of `n` elements with `Mod*` per element solved in the test (complex
   Thomas algorithm, as `chain_tip` of ADR 0448) is the exact finite-element solution (arithmetic 2); the difference to the
   solver is rounding times the condition number, about `1e-12` (ADR 0448 H2: up to `1.3e-11` at 80 elements, `eta = 0.02`);
   limit `1e-9`.
7. **Negative control of the split** (probe): the unequal-weight rods B and C solved with the one-part coefficient `E*/E_0` on
   the base modulus (the equal-weight route applied wrongly) differ from the correct chain by at least `1.54e-1` (B) and
   `4.72e-2` (C) over the ten frequencies; so the chain checks of B and C (limit `1e-9`) detect a route that ignores the
   unequal weights.
8. **Zero frequency.** At `omega = 0` the coefficients are the long-term ratios `g_inf`, `k_inf` (real), the solve is the
   static one with `M_inf = K_0 (1 - k) + (4/3) G_0 (1 - g)`, and the linear chain is exact for a static end load: `u(L) = F L/(A
   M_inf)`. Rounding of a chain of 40 elements (a condition number of about `n^2`): limit `1e-11`.
9. **The shift.** `complex_moduli` uses `x_i = omega a_T tau_i`; the shifted material at `T` against the unshifted one with
   `tau_i a_T` (with `a_T = shift_factor(shift, T)`) differs only by the order of the products: rounding, `1e-15/eta`; limit
   `1e-12`.

## Design

### D1. The viscoelastic route of `DirectHarmonic::new` (`harmonic_direct.rs`)

Taken only when a material of the model is `Material::Viscoelastic` (the flag `visco`); every other model runs the code of ADR
0448 as it is, operation for operation (the new code sits in branches the old models do not enter).

- **Checks.** `model.check_supported_nonlinear()` (the element checks with the history flag on, which lets the viscoelastic
  material through, then the supports) in place of `model.check_supported()`, then a J2 material refused exactly as
  `check_supported` refuses it (`NotYetSupported`, location `material {i}, element set '{set}'`, package
  `mat_j2::FOLLOW_UP`). An orthotropic base is refused by `mat_visco` (`NotYetSupported`, location `"an orthotropic base"`).
  A loss factor given in the options for the set of a viscoelastic material is refused (key `"loss factor"`: the material
  carries its own damping). A shifted material needs `DirectHarmonicOptions::temperature` (new field, default `None`, builder
  `temperature(t)`): `complex_moduli` is evaluated once at `omega = 0` in `new` so that a missing or bad temperature is refused
  there (key `"temperature"`).
- **Parts.** Per material: a viscoelastic material whose every term has equal shear and bulk weights (or no term) is one part,
  assembled with `isotropic_elasticity(E, nu)` of the base, coefficient `E*(omega)/E` (`young / e`); otherwise two parts, `D_G`
  and `D_K` of arithmetic 1, coefficients `G*(omega)/G_0` and `K*(omega)/K_0`, with `G_0`, `K_0` computed as in
  `complex_moduli`. Any other material is one part with its `elasticity_matrix` and its loss factor as before. Every part by
  `assemble_with` (as the multi-material path of ADR 0448); the total `K` (for the Rayleigh term `beta K` and the pattern) is
  the sum of the parts at their base values (the instantaneous stiffness for a viscoelastic material: Limits).
- **Coefficients at `omega`.** `Damping::Viscoelastic { material, part, temperature }`, `part` one of whole, shear, bulk:
  `mat_visco::complex_moduli(material, omega, temperature)` at each frequency.
- A new `pub(crate)` helper in `mat_visco.rs`, `harmonic_base(material) -> Result<(e, nu, g0, k0, equal)>`, gives the base
  moduli and whether the weights are equal (with the refusals of `parts`).

### D2. Stress

`DirectHarmonic::stress` of a solver with a viscoelastic part is refused (`NotYetSupported`, item `"material"`, variant
`"Viscoelastic"`, location `"the harmonic stress"`, package `mat_visco::FOLLOW_UP`): the recovery of
`HarmonicResponse::stress` evaluates the material at integration points, which the viscoelastic material refuses; the split
stress `(G*/G_0) dev sigma_0 + (K*/K_0) vol sigma_0` is a follow-up (proposed).

### D3. The damped modes

No code change: `damped_modes` keeps refusing the material through `check_supported` (reading (a4)). The module doc gains one
sentence saying so and naming this ADR.

### D4. Documentation

The module docs of `harmonic_direct.rs` and `mat_visco.rs` say that the direct harmonic solve takes the material.
`docs/reference-data.md` gains the F-577 rows (the correspondence principle and the rod closed form; derived, no source read).

### D5. No case-file key

The case file does not give the harmonic study a viscoelastic material (the maintainer's call, as ADR 0485 open question 1).

### D6. The bit record of the default (B0)

`crates/rcfd-fem/tests/sa_harmonic_visco_default_bits.rs`, written after this commit and run on the unchanged base e90c2350
before any library code; its digests (FNV-1a 64 over the bits, as ADR 0485 D8) are written into the file then. Content:
`DirectHarmonic` of the 40-element bar of ADR 0448 with `Constant(0.02)` at three frequencies (tip and every value); the same
bar with a two-point `Table`; a two-material bar (`left`, `right` element sets, `Constant` on one, `Table` on the other); a
`hex_box([0;3], [0.6, 0.1, 0.1], [6, 2, 2])` cantilever with `nu = 0.3`, Rayleigh, a ground dashpot and a dashpot between two
nodes, at two frequencies, with `DirectHarmonic::stress` of one response; a base-motion response of the bar; `damped_modes` of
the cantilever with Rayleigh, a dashpot and `Constant(0.02)` (eigenvalues and shapes); the refusal messages of the direct solve
for a J2 material, for a `Table` on a J2 set (refused first by the model check), for a set without a material, and of
`damped_modes` for a J2 material.

## Checks (limits fixed now)

| Id | Clause | What | Limit | Estimate behind the limit |
|---|---|---|---|---|
| HA | a1, a2 | rod A (axial, `nu = 0`, equal weights `w = 2/21`, `M* = E*`), 40 Hex8, the ten `omega_k`: `max_k |X_tip - u(L)|/|u(L)|`, closed form of arithmetic 2 | `<= 4e-3` (registered discretisation limit) | probe `2.472e-3`; first order `2.57e-3` (arithmetic 4) |
| HAo | a2 | rod A at 80 Hex8: `log2(e_40/e_80)` of the maxima | `>= 1.9` | probe `2.001` |
| HAd | a2 (set-up) | rod A at 40 and 80 Hex8 against the discrete chain (arithmetic 6), every frequency | `<= 1e-9` | `1e-12` |
| HB, HBo, HBd | a1, a2 (unequal weights) | rod B (axial, `nu = 0.3`, `g = 0.15`, `k = 0.05`, modulus `K* + 4 G*/3`), as HA, HAo, HAd | `4e-3`, `>= 1.9`, `1e-9` | probe `2.646e-3`, `2.001` |
| HC, HCo, HCd | a1 (shear part) | rod C (shear, `u_x = u_z = 0` held, force along `y`, modulus `G*`, the material of B), as HA, HAo, HAd | `4e-3`, `>= 1.9`, `1e-9` | probe `1.601e-3`, `2.000` |
| HD | a1 (two materials) | rod of 40 Hex8, the elements `0 .. 20` (set `left`) the elastic base of B, `20 .. 40` (set `right`) the material of B, at the ten `omega_k` of B, against the chain with the modulus per element | `<= 1e-9` | `1e-12` |
| E1 | a3 | rod A: the viscoelastic route against the base with `LossFactor::Table(modulus_table(A, f_k))` at `omega_k = 2 pi f_k`: `max_k max_j |X_v - X_t| / max_j |X_t|` | `<= 1e-12` | `1e-14` (same assembly bits) to `1e-11` (arithmetic 5) |
| DM1 | a4 | `damped_modes` of rod A with the material (`DampedOptions::new(4)`): refused, `NotYetSupported`, variant `Viscoelastic`, package `mat_visco::FOLLOW_UP` | refused as stated | D3 |
| Z1 | a1 | rod B at `omega = 0`: `|X_tip - F L/(A M_inf)| / (F L/(A M_inf))` | `<= 1e-11` | arithmetic 8 |
| T1 | a1 | rod B with a WLF shift (`c1 = 17.44`, `c2 = 51.6 K`, `T_ref = 373 K`) at `T = 383 K` given by `temperature`, against rod B with `tau a_T`, the ten `omega_k`: `max_k max_j |X_s - X_u| / max_j |X_u|` | `<= 1e-12` | arithmetic 9 |
| C0 | a1 | a viscoelastic material without terms against its base (the rod of A with `nu = 0`), at `0.5` and `1.5` of the base's first resonance `omega_e = (pi/2) sqrt(E_0/rho)`, no damping: `max_j |X_v - X_b| / max_j |X_b|` | `<= 1e-12` | coefficient rounding `1e-16` over a distance to resonance of order 1 |
| P1 | a1 | rod B, the response at `omega_c` with assembly threads 1 and 4 | bits equal | the assembly is per element, the factor sequential |
| NC1 | negative control | rods B and C: the solver against the chain with the wrong (one-part `E*/E_0`) modulus: `min_k` of the relative difference | `>= 1e-2` | probe `1.54e-1`, `4.72e-2` (arithmetic 7) |
| NC2 | negative control | `modulus_table` of the material of B refused (key `"terms"`, existing); the table route cannot give rods B and C | refused | ADR 0485 M3 |
| RF | refusals | (i) a `LossFactor::Constant(0.02)` on the set of a viscoelastic material: key `"loss factor"`; (ii) the shifted material of T1 without a temperature: key `"temperature"`; (iii) with `temperature(f64::NAN)`: key `"temperature"`; (iv) a viscoelastic material on an orthotropic base: `NotYetSupported`, location `"an orthotropic base"`; (v) a model with a viscoelastic and a J2 material: `NotYetSupported`, variant `J2`, package `mat_j2::FOLLOW_UP`; (vi) `DirectHarmonic::stress` of a response of rod A: `NotYetSupported`, location `"the harmonic stress"` | each as stated | D1, D2 |
| N1 | b2 | B0: every digest of the base e90c2350 | equal | D6 |
| N2 | b2 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | no shared output path changed |
| N3 | b2 | the existing tests of `harmonic_direct`, `damped`, `mat_visco` (`sa_harmonic_direct`, `sa_damped*`, `sa_visco*`) and the rest of `cargo test -p rcfd-fem --release` | pass | D1 |

Every aggregated error is required finite (a NaN anywhere is a miss). Files: `crates/rcfd-fem/tests/sa_harmonic_visco.rs`
(HA to RF), `sa_harmonic_visco_default_bits.rs` (N1).

## Planned mutations (on a scratch copy, one at a time)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the shear and bulk coefficients swapped (`K*/K_0` on the shear part) | HBd, HCd, HB, HC, HD |
| MU2 | `D_G` with `2 G_0` on the shear diagonal (tensor shear for engineering shear) | HCd, HC |
| MU3 | `complex_moduli` called with `f = omega/(2 pi)` in place of `omega` | HA, HB, HC, HAd, HBd, HCd, E1 (the table route evaluates at `2 pi f`); not T1 (both sides moved alike) |
| MU4 | the equal-weight test inverted (unequal weights take the one-part `E*/E` route) | HBd, HCd (NC1 says by `>= 4.7e-2`) |
| MU5 | the loss part dropped (`Re` of the coefficient only) | HA, HB, HC, E1 |
| MU6 | the J2 refusal of the viscoelastic route removed | RF (v) |
| MU7 | the temperature not passed to `complex_moduli` | T1 (refused: the shifted material needs a temperature), RF (ii) not |

## Decision rule

- **done**: every check of the table met (HA, HB, HC at the registered discretisation limit `4e-3`, as the acceptance allows),
  every planned mutation caught by at least one check, fmt and clippy clean, B0 and the baseline hashes unchanged.
- **partial**: B0 and the baseline hashes unchanged, (a1) and (a4) met (HAd, HBd, HCd, HD, DM1), and a miss of E1 (a3) or of
  the discretisation limits diagnosed and recorded as a `*_known_miss` test.
- **todo**: otherwise (in particular any change of the default bits).

## Limits (not covered)

- The isotropic base only (as ADR 0485); solid elements on the elimination path only (as ADR 0448).
- The Rayleigh term `beta K` takes the instantaneous base stiffness of a viscoelastic material (`K` at its base values); a
  model that wants only the material's damping gives no Rayleigh term.
- No harmonic stress for a viscoelastic material (refused, D2): a follow-up.
- The damped modes refuse the material (D3): a fixed-frequency or a nonlinear (frequency-dependent) eigenproblem is a
  follow-up.
- One temperature for the whole model (`DirectHarmonicOptions::temperature`); no temperature field.
- The discretisation limit `4e-3` is the dispersion of the linear element at 40 elements near the peak; it is not a property of
  the material route, whose exactness is checked against the discrete chain (`1e-9`).

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. **Damped modes: refuse or fixed frequency.** Refuse (taken): the acceptance allows it, and a fixed-frequency route needs the
   undamped basis of a substitute elastic model with the storage moduli and a projected loss stiffness of the two parts; it is
   proposed as a follow-up.
2. **The rod's modulus for unequal weights** (reading (a2)): the constrained modulus of the rod with the lateral motion held,
   taken: the 3-D rod with free lateral faces adds lateral inertia (Love's correction, about `1e-4` here, from memory) and is not
   a closed form.
3. **The 1e-6 of the acceptance** is out of reach at 40 elements (arithmetic 4); the registered discretisation limit `4e-3` is
   used, as the acceptance allows, with the order and the chain checks.

## Addendum 1: results (2026-11-14)

Registered run: commit 6a5a05d9 (`cargo test -p rcfd-fem --release --test sa_harmonic_visco -- --nocapture
--test-threads=1`, log `f577-scratch/run1.log`). Each check ran once; no set-up was corrected and no limit was changed. The bit
record B0 was printed on the unchanged base e90c2350 (`b0-base.log`) and written into the test at commit 714376ac, before any
library code. Commit 4979df79 after the run is a type alias (clippy), layout only.

| Id | Limit | Measured | Met |
|---|---|---|---|
| HA rod A (`M* = E*`), closed form at 40 Hex8, max of 10 frequencies | `<= 4e-3` (registered discretisation limit) | `2.472e-3` (at `omega_c`; `4.2e-4` at `0.9 omega_c`) | yes |
| HAo observed order 40 to 80 | `>= 1.9` | `2.001` | yes |
| HAd against the discrete chain, 40 and 80 | `<= 1e-9` | `1.35e-11` | yes |
| HB rod B (unequal weights, `K* + 4 G*/3`) | `<= 4e-3` | `2.646e-3` | yes |
| HBo | `>= 1.9` | `2.001` | yes |
| HBd | `<= 1e-9` | `1.08e-11` | yes |
| HC shear rod (`G*`) | `<= 4e-3` | `1.601e-3` | yes |
| HCo | `>= 1.9` | `2.000` | yes |
| HCd | `<= 1e-9` | `9.30e-12` | yes |
| HD two materials against the chain | `<= 1e-9` | `2.08e-12` | yes |
| E1 equal weights against the `LossFactor::Table` route at the table points | `<= 1e-12` | `1.41e-13` (0 at six of the ten points) | yes |
| DM1 damped modes refuse the material | refused as stated | `NotYetSupported`, `Viscoelastic`, `mat_visco::FOLLOW_UP` | yes |
| Z1 zero frequency, `F L/(A M_inf)` | `<= 1e-11` | `1.74e-14` | yes |
| T1 WLF shift at 383 K against `tau a_T` (`a_T = 1.475133e-3`) | `<= 1e-12` | `1.70e-15` | yes |
| C0 no terms against the base | `<= 1e-12` | `0` | yes |
| P1 threads 1 and 4 | bits | equal | yes |
| NC1 rods B, C with the one-part route | `>= 1e-2` | `1.76e-1`, `4.53e-2` | yes |
| NC2 table of unequal weights refused | key `"terms"` | refused | yes |
| RF (i) to (vi) | as stated | all six as stated | yes |
| N1 B0, 11 digests of e90c2350 | equal | equal | yes |
| N2 `baseline_hashes` | pass | pass (3 passed, 1 ignored) | yes |
| N3 `cargo test -p rcfd-fem --release` (every test binary of the crate, with the doc tests) | pass | 663 passed, 0 failed, 166 ignored | yes |

The NC1 numbers differ from the probe's (`1.54e-1`, `4.72e-2`) because the test divides by the wrong chain's response and the
probe by the right one; both are far above the limit.

E1 is not zero at four of the ten points (`1.4e-13` at the largest): the table route multiplies by `(s, s eta)` with `eta =
E''/E'`, the viscoelastic route by `E*/E`, which differ by rounding. MU4 (below) shows the estimate of arithmetic 5 for a
two-part route: the shear and bulk parts assembled apart and summed give `1.1e-12` against the table, above the limit; the
one-part route for equal weights is what keeps E1 at `1e-13`.

**Mutations** (`f577-scratch/mutate.py` on a git-archive copy of 6a5a05d9 with its own target directory, log
`mutations.log`; MU0, the copy without a mutation, passes 4 of 4):

| Id | Caught by |
|---|---|
| MU1 shear and bulk coefficients swapped | HB (`0.30`), HBo, HBd, HC (`2.56`), HCo, HCd, HD, Z1 (`2.7e-2`) |
| MU2 `2 G_0` on the shear diagonal | HC (`1.27`), HCo, HCd |
| MU3 `complex_moduli` at `omega/(2 pi)` | E1 (`2.42`), HA, HAo, HAd, HB, HBo, HBd, HC, HCo, HCd, HD; not T1 (as expected) |
| MU4 the equal-weight test inverted | HB (`0.50`), HBd, HC (`0.13`), HCd, HD, Z1, NC1 (`3e-13`, the route now equals the wrong one), and E1 (`1.11e-12`, the two-part route for equal weights) |
| MU5 the loss part dropped | E1 (`2.32`), HA, HB, HC and their chain and order checks, HD |
| MU6 the J2 refusal removed | RF (v) (J2 refused later, by its element-constant matrix, without the set in the message) |
| MU7 the temperature not passed to the coefficients | T1 (the response refused at the first frequency: the shifted material needs a temperature) |

Every planned mutation was caught.

**Deviations.** None in the checks. The B0 file has eleven digests (D6 lists the J2 refusal with a table, which repeats the
refusal message of J2 alone, as expected). Not run: the whole workspace test suite (the lead's gate).

**Shared solver files changed.** `harmonic_direct.rs`: module doc; `DirectHarmonicOptions::temperature` (field, default
`None`, builder); `Damping::Viscoelastic`, `ViscoPart`, `shear_matrix`, `bulk_matrix`, `check_viscoelastic_model`,
`viscoelastic_parts` (new, private); in `DirectHarmonic::new` the model check and the matrices branch on a viscoelastic
material, the old code unchanged inside the `else` (re-indented), and a loss factor on a viscoelastic set refused;
`coefficients` gains the viscoelastic arm; `stress` refuses a viscoelastic part. `mat_visco.rs`: module doc, `harmonic_base`
(new, `pub(crate)`). `damped.rs`: module doc only. `material.rs` and `model.rs`: not touched.

Other runs: `cargo test -p rcfd-cli --release --test docs_site`: 2 passed; `cargo clippy -p rcfd-fem --all-targets --release --
-D warnings`: clean (after 4979df79); `cargo fmt -p rcfd-fem --check`: clean.

**Decision by the registered rule: done.** Every check of the table is met, HA, HB and HC at the registered discretisation
limit `4e-3`, as the acceptance allows (the acceptance's `1e-6` is out of reach at 40 elements: the dispersion of the linear
element with consistent mass, `2.47e-3` at the peak of rod A, second order, and the solver equals the discrete chain within
`1.4e-11`). Every planned mutation is caught, fmt and clippy are clean, and B0 and the baseline hashes are unchanged.

### Remaining questions

1. **The damped modes.** They refuse the material (reading (a4)). Options: (a) leave it so; (b) take it at a fixed frequency
   `omega_ref` given in the options: the undamped basis of the storage moduli at `omega_ref` and a projected loss stiffness of
   the parts; (c) iterate each root to its own frequency (a nonlinear eigenproblem). Recommendation: (b), follow-up 1; (c)
   later if the fan's plastic blade needs the damping of more than one mode accurately.
2. **The harmonic stress.** Refused. Options: (a) leave it so; (b) the split stress `(G*/G_0) dev sigma_0 + (K*/K_0) vol
   sigma_0` per point from the elastic stress of the base. Recommendation: (b), follow-up 2 (fatigue of a plastic blade needs
   the stress amplitude).
3. **Case-file key.** Not added (ADR 0485 open question 1). Recommendation: when the maintainer opens the case file to the
   viscoelastic material, offer it to the harmonic study with a `temperature` key.
4. **Rayleigh `beta K` with a viscoelastic material** takes the instantaneous base stiffness. Options: (a) leave it and say so
   (done, module doc); (b) refuse `beta` with a viscoelastic material. Recommendation: (a).

## For the lead: roadmap entries

Proposed status of F-577: **done**. Proposed note:

> ADR 0491, 2026-11-14: the direct harmonic solve (harmonic_direct.rs) takes a viscoelastic material directly, by mat_visco::complex_moduli at each frequency: with equal weights one part, the base stiffness times E*(omega)/E; with unequal weights two parts, the stiffness of 2 G_0 I_dev times G*/G_0 and that of K_0 1 (x) 1 times K*/K_0 (exact: the stiffness is linear in the elasticity matrix). DirectHarmonicOptions::temperature serves a shifted material. A loss factor on its set and the harmonic stress are refused; the Rayleigh term takes its instantaneous stiffness. The damped modes refuse the material (NotYetSupported, mat_visco::FOLLOW_UP). Met: the tip response of a clamped 40-element Hex8 rod of the standard solid at 10 frequencies from 0.9 to 1.1 of its first resonance against the closed form of the rod with the complex modulus within 2.47e-3 (E*, nu = 0, equal weights), 2.65e-3 (unequal weights, K* + 4 G*/3) and 1.60e-3 (shear rod, G*), against the registered discretisation limit 4e-3 (1e-6 is out of reach at 40 elements: the dispersion of the linear element, observed order 2.00); every response against the discrete chain within 1.4e-11; two materials within 2.1e-12; equal weights against the LossFactor::Table route at the table points within 1.4e-13; zero frequency, shift and no-term checks within 1.7e-14, 1.7e-15 and 0; threads bit-identical; seven mutations caught; default bits (11 digests) and baseline hashes unchanged.

Follow-up 1. Title: "The viscoelastic material in the damped modes at a fixed frequency". Deps: F-577. Acceptance:

> Every limit not stated here is registered in the ADR before the run. The damped modes take a viscoelastic material at a reference frequency given in the options: the undamped basis from the storage shear and bulk moduli at that frequency and the loss stiffness of their loss moduli projected on it. On the 40-element rod of ADR 0491 (rod A) with the reference frequency at its first resonance, the first damped root equals the root of the discrete chain with the complex modulus at that frequency within 1e-8 relative, and its loss factor 2 |Re lambda|/Im lambda equals E''/E' at that frequency within 1e-2 relative (first order in the loss). Models without the material keep their bits.

Follow-up 2. Title: "The harmonic stress of a viscoelastic material in the direct harmonic solve". Deps: F-577. Acceptance:

> Every limit not stated here is registered in the ADR before the run. DirectHarmonic::stress of a response with a viscoelastic material returns (G*/G_0) dev sigma_0 + (K*/K_0) vol sigma_0 at each integration point, sigma_0 the elastic stress of the base. On the rods A, B and C of ADR 0491 at 40 elements the axial (or shear) stress at the integration points equals the modulus times the strain of the discrete chain within 1e-9 relative; for equal weights it equals the stress of the LossFactor::Table route within 1e-12. Models without the material keep their bits.
