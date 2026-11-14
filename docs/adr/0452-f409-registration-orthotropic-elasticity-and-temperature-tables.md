# ADR 0452: F-409 registration: orthotropic elasticity with material axes, orthotropic expansion, and temperature-dependent properties from tables

Status: **REGISTERED 2026-11-14, written before any code of F-409 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-409 was written, compiled or run, except the bit record of the default paths (B1,
disclosed below: a test file using only the API of main 0ca10e83, run once at that commit with no library line changed) and the
arithmetic of the closed forms (disclosed below). Results go in the addenda at the end; nothing above the addenda is changed
after a run.

Work package SA-2 of the structural solver, advanced tier ([ADR 0310](0310-structural-solver-advanced-tier-plan.md)); stub
[ADR 0312](0312-orthotropic-elasticity-and-temperature-dependent-properties.md), kept unchanged. Roadmap item F-409 (deps F-408,
F-372, F-374, F-376, F-380). Size M. Number 0452 given by the lead. Branch `f409-work` from main 0ca10e83, worktree
`w49i-work/f409`, scratch `w49i-work/f409-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code, the crate graph
unchanged; no existing test edited except the scaffold records-and-copies step (D9) of the maintainer's standing decision;
clean-room.

## What F-409 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. Orthotropy: with isotropic constants the elasticity matrix equals the isotropic one within 1e-13 relative; constants that are not positive definite are refused naming the constant. Off-axis tension of a block with the material axes turned by 15, 30, 45 and 60 degrees about z: axial strain sigma/E_x with 1/E_x = c^4/E1 + s^4/E2 + (1/G12 - 2 nu12/E1) s^2 c^2 and the shear-extension ratio from the rotated compliance within 1e-10; rotating the model and the axes together leaves the strain energy unchanged within 1e-12. Cantilever of slenderness 20 with E1 along the span and E2 = E1/10: tip deflection P L^3/(3 E1 I) + P L/(kappa G13 A) within 1 percent. Free heating gives the strains alpha_i dT in material axes with zero stress, full restraint the stress -D alpha dT, both within 1e-10. Cylindrical axes: a tube b/a = 2 with E_theta/E_r = 4 under internal pressure in plane stress has the bore hoop stress of the polar-orthotropic closed form (powers r^(k-1) and r^(-k-1), k = sqrt(E_theta/E_r), derived in the registration) within 1 percent. Tables: a bar whose temperature and so its modulus vary linearly from E0 to E1 along it stretches under an end force by F L ln(E1/E0)/(A (E1 - E0)) within 0.1 percent with 20 elements; with alpha = a0 + a1 T the free expansion is a0 (T - Tref) + a1 (T^2 - Tref^2)/2 within 1e-10; a constant table gives the bits of the constant material; a temperature outside a table is refused with the element and the temperature.

Clauses: (a0) every limit registered before the run (this document); (a1) isotropic constants give the isotropic matrix within
1e-13 relative; (a2) constants not positive definite refused naming the constant; (a3) off-axis axial strain at 15, 30, 45, 60
degrees within 1e-10; (a4) the shear-extension ratio from the rotated compliance within 1e-10; (a5) rotating model and axes
together leaves the strain energy unchanged within 1e-12; (a6) the orthotropic cantilever within 1 percent; (a7) free heating:
strains `alpha_i dT` in material axes and zero stress within 1e-10; (a8) full restraint: stress `-D alpha dT` within 1e-10; (a9)
the polar-orthotropic tube bore hoop stress within 1 percent, the closed form derived here; (a10) the bar with a varying
modulus within 0.1 percent with 20 elements; (a11) the free expansion with `alpha = a0 + a1 T` within 1e-10; (a12) a constant
table gives the bits of the constant material; (a13) a temperature outside a table refused with the element and the temperature.

The roadmap formulas were checked (arithmetic 1 to 4): `1/E_x` and the shear-extension ratio follow from the tensor rotation
with the axes turned by `+theta` (rows `e_1 = (c, s, 0)`, `e_2 = (-s, c, 0)`); the tube formula's powers are right for any
`nu_r_theta`; the bar formula is the integral of `F/(A E(x))`. None needs a correction.

## What was read (main 0ca10e83, read only)

1. `AGENT_RULES.md`; ADR 0312 (stub), ADR 0440 (F-408, the scaffold: D1 modules, D2 the `Orthotropic` and `Tabulated` variants
   and `MaterialAxes`, D3 the point context, D5 the refusals); ADR 0385 (F-485: the Hex20 cantilever with the support of beam
   theory and the refined thick cylinder, their set-ups and measured numbers); ADR 0292 T1 (the 4 x 8 cylinder record); ADR 0445
   (F-525: the Shell4 thermal load refuses a material without a single expansion coefficient, kept); ADR 0281 (the thermal
   strain load and recovery); ADR 0291 (the records-and-copies pattern of scaffold rows).
2. Code: `crates/rcfd-fem/src/{material.rs (whole), model.rs (Material and its constructors and accessors, check_elements and the
   check_* family, material_indices), assemble.rs (whole), element.rs (the Element trait: point, integration_points,
   stiffness_with_rule, strain_at), thermal.rs (whole), recover.rs (IntegrationPointField::compute), solve.rs
   (solve_static_report), mat_ortho.rs, mat_table.rs, mesh.rs (the set API)}`; the entry checks of every solve (the list of ADR
   0440 finding 4, re-read at 0ca10e83 by grep for `check_supported` and `stiffness_with`); the tests `sa_scaffold.rs` (whole),
   `sa_scaffold_bits.rs` (the digest), `si_hex20_support_bore.rs` (whole), `si_hex20.rs` (the cantilever and T1).
3. **Not read** (cited from memory): R. M. Jones, *Mechanics of Composite Materials* (the rotated compliance `S-bar` of a
   lamina, the restrictions on the engineering constants of Lempriere); S. G. Lekhnitskii, *Theory of Elasticity of an
   Anisotropic Body* (the polar-orthotropic ring); Timoshenko, *Strength of Materials* (the shear deflection `P L/(kappa G A)`
   with `kappa = 5/6`). Every formula used is derived below from first principles; the books are named for the reader only.

Findings that shape the design:

1. Every element stiffness is `element_for(kind).stiffness(coords, &d)` with one `d` per element from
   `Material::elasticity_matrix`, in `assemble::stiffness_with`; the thermal load (`thermal::element_thermal_load`) and the
   recovery (`IntegrationPointField::compute`) likewise use one `d` and `material.expansion()`. Properties that vary inside an
   element (cylindrical axes, a temperature field through a table) need the material evaluated at each integration point: the
   stub's "what could make it miss". The isotropic code is left as it is and a per-point path is added beside it (D5).
2. Every solve entry calls one of the `check_*` functions before `stiffness_with`, and `check_elements` refuses a material
   whose `pending()` is `Some`. Once `PENDING` is `None` the two variants would pass every check: the paths that are not opened
   here must keep refusing them by name (D6).
3. `Beam2` and `Shell4` take `youngs_modulus()`, `poisson_ratio()`, `expansion()`, which are NaN for `Orthotropic` and the
   base's for `Tabulated` (silently wrong for a table). They must refuse both variants.
4. The non-ignored scaffold test `m1_to_m4_material_variants_and_state_layout` asserts that `Orthotropic` and `Tabulated` are
   pending and refused by the constitutive call, and `x1_modules_and_pending_after_f413_f414` that `mat_ortho` and `mat_table`
   are pending. Both become records with a copy (D9).

## Closed forms (first principles)

Voigt order xx, yy, zz, xy, yz, zx with engineering shear strain (ADR 0270). Material axes 1, 2, 3; in material Voigt order
11, 22, 33, 12, 23, 31.

**F1 Compliance.** `eps' = S sigma'` with `S_11 = 1/E_1`, `S_22 = 1/E_2`, `S_33 = 1/E_3`, `S_12 = S_21 = -nu_12/E_1`,
`S_13 = S_31 = -nu_13/E_1`, `S_23 = S_32 = -nu_23/E_2`, `S_44 = 1/G_12`, `S_55 = 1/G_23`, `S_66 = 1/G_13`. The strain energy
`sigma' . S sigma'/2` exists only if `S` is symmetric, which gives the reciprocity `nu_ji/E_j = nu_ij/E_i` (`nu_21 = nu_12 E_2/E_1`
etc.): the three ratios `nu_12, nu_13, nu_23` are the independent ones, the others are derived, and `S` is built symmetric by
construction (check O3).

**F2 Positive definiteness.** `S` is block diagonal (normal 3 x 3, shear diagonal). With positive moduli the shear block is
positive. The normal block is positive definite iff its diagonal and its 2 x 2 principal minors and determinant are positive
(Sylvester on each ordering): `S_11 S_22 - S_12^2 > 0` iff `nu_12^2 < E_1/E_2`; likewise `nu_13^2 < E_1/E_3`,
`nu_23^2 < E_2/E_3`; and `det = (1 - nu_12 nu_21 - nu_13 nu_31 - nu_23 nu_32 - 2 nu_21 nu_32 nu_13)/(E_1 E_2 E_3) > 0`. The
refusal names `nu_12`, `nu_13` or `nu_23` for the minors (first failing in that order) and `nu_12, nu_13, nu_23` for the
determinant.

**F3 Stiffness.** `C = S^-1`: shear diagonal `G_12, G_23, G_13`, normal block the inverse of the 3 x 3 by cofactors over the
determinant. With `E_i = E`, `nu = nu`, `G = E/(2 (1 + nu))` it is the isotropic matrix (`lambda + 2 mu`, `lambda`, `mu`):
arithmetic 1 gives 1.1e-16 relative in double precision.

**F4 Rotation.** The material axes are the rows of `R` (`R[a][i] = e_a . g_i`). The strain tensor rotates as
`eps'_ab = R_ai R_bj eps_ij`. In engineering Voigt form `eps' = T eps` with, for material pair `(a, b)` and global pair `(i, j)`,
`T = f_ab * (R_ai R_bi)` for `i = j` and `T = f_ab * (R_ai R_bj + R_aj R_bi)/2` for `i != j`, `f_ab = 1` for `a = b` and 2 for
`a != b`. Energy conjugacy (`sigma . eps = sigma' . eps'` for every `eps`) gives `sigma = T^T sigma'`, so the global elasticity is
`D = T^T C T` and the global compliance `T^-1 S T^-T`. `D` is computed for `j >= i` and mirrored (exactly symmetric). For
`R = I` the matrix `T` is the identity exactly and `D = C` bit for bit.

**F5 Off-axis tension.** Axes turned by `+theta` about z: `e_1 = (c, s, 0)`, `e_2 = (-s, c, 0)`, `e_3 = z`. Uniaxial `sigma_xx`:
`sigma'_11 = sigma c^2`, `sigma'_22 = sigma s^2`, `sigma'_12 = -sigma s c`. With `eps_xx = c^2 eps'_11 + s^2 eps'_22 - s c gamma'_12`:
`eps_xx/sigma = S_11 c^4 + S_22 s^4 + (2 S_12 + S_66') s^2 c^2 = c^4/E_1 + s^4/E_2 + (1/G_12 - 2 nu_12/E_1) s^2 c^2`, the roadmap's
`1/E_x`. With `gamma_xy = 2 c s (eps'_11 - eps'_22) + (c^2 - s^2) gamma'_12`:
`gamma_xy/sigma = (2 S_11 - 2 S_12 - S_66') s c^3 - (2 S_22 - 2 S_12 - S_66') s^3 c` (here `S_66'` is `1/G_12`), and the
shear-extension ratio is `eta = gamma_xy/eps_xx`. Arithmetic 2 checks both against a numerical tensor rotation (agreement
1e-15). The test also compares the six strain components with `sigma` times the first column of the global compliance computed in
the test by its own tensor rotation of `S`.

**F6 Thermal strain.** In material axes `eps'_th = (alpha_1, alpha_2, alpha_3, 0, 0, 0) dT`; globally the tensor
`eps_th,ij = sum_a R_ai R_aj alpha_a dT` (engineering shears doubled), that is `eps_th = T^-1 eps'_th`. Free heating of a block
with three-two-one support: `eps = eps_th`, `sigma = D (eps - eps_th) = 0`. Full restraint: `eps = 0`, `sigma = -D eps_th`, in
material axes `sigma' = -C alpha dT`.

**F7 Polar-orthotropic tube** (Lekhnitskii's ring, re-derived). Plane stress in `r, theta` with rectilinear orthotropy in the
cylindrical axes: `eps_r = s_rr sigma_r + s_rt sigma_t`, `eps_t = s_rt sigma_r + s_tt sigma_t`, `s_rr = 1/E_r`, `s_tt = 1/E_t`,
`s_rt = -nu_rt/E_r`. Axisymmetric equilibrium `d sigma_r/dr + (sigma_r - sigma_t)/r = 0` holds with `sigma_t = d(r sigma_r)/dr`.
Try `sigma_r = r^(m-1)`: `sigma_t = m r^(m-1)`, `r eps_t = (s_rt + m s_tt) r^m`, compatibility `eps_r = d(r eps_t)/dr` gives
`s_rr + m s_rt = m s_rt + m^2 s_tt`, so `m^2 = s_rr/s_tt = E_t/E_r`, `m = +-k`, `k = sqrt(E_t/E_r)`, independent of `nu_rt`. Hence
`sigma_r = A r^(k-1) + B r^(-k-1)`, `sigma_t = k (A r^(k-1) - B r^(-k-1))`. With `sigma_r(a) = -p`, `sigma_r(b) = 0`:
`B = -A b^(2k)`, `A = p a^(k+1)/(b^(2k) - a^(2k))`, and the bore hoop stress

`sigma_t(a) = k p (a^(2k) + b^(2k))/(b^(2k) - a^(2k))`,

for `k = 1` Lame's `p (a^2 + b^2)/(b^2 - a^2)`. For `b/a = 2`, `k = 2`: `sigma_t(a) = 2 p 17/15 = 34 p/15 = 2.26667 p` (Lame: `5 p/3`).
Bore radial displacement `u_r(a) = a (s_rt sigma_r(a) + s_tt sigma_t(a))`. Plane stress in a 3D model: with `nu_13 = nu_23 = 0`
(material axis 3 = z) the normal stress `sigma_zz = E_3 eps_zz` decouples from the in-plane strains, and the slab with `u_z = 0`
on both faces has `sigma_zz = 0`: plane strain and plane stress coincide exactly. Arithmetic 3 checks equilibrium and
compatibility of the closed form by differences (1e-10).

**F8 Bar with a varying modulus.** Uniform `sigma = F/A`, `nu = 0` (no lateral coupling, the uniaxial state is exact in 3D):
`delta = int_0^L F/(A E(x)) dx`, `E = E_0 + (E_1 - E_0) x/L`, `delta = F L ln(E_1/E_0)/(A (E_1 - E_0))`. A Hex8 element with
`E` linear in `x` evaluated at its 2 x 2 x 2 points is a spring of stiffness `A E(x_mid)/h` (two-point Gauss exact for linear
`E`), so the discrete stretch is the midpoint rule on `1/E`: relative error `-(h^2/24) int (1/E)'' / int 1/E`; for `E_0 = 2e11`,
`E_1 = 1e11`, 20 elements arithmetic 4 gives `-1.126e-4`.

**F9 Integrated expansion.** The thermal strain of a tabulated `alpha` is `int_{T_ref}^{T} alpha(tau) d tau` (ADR 0312's goal,
the instantaneous coefficient); for `alpha = a0 + a1 T` it is `a0 (T - T_ref) + a1 (T^2 - T_ref^2)/2`. Piecewise-linear
interpolation of a linear function is exact and the trapezoid on each segment integrates it exactly: the only error is rounding.

**F10 Cantilever.** Euler-Bernoulli with Timoshenko's shear term: `delta = P L^3/(3 E_1 I) + P L/(kappa G_13 A)`,
`kappa = 5/6`, the span along material axis 1, the load along material axis 3 (shear in the 1-3 plane, `G_13`). For the model of
O6 (`L = 2`, `h = 0.1`, `E_1 = 1.4e11`, `G_13 = 7e9`, `P = 1000`) the bending part is `2.2857e-3` m and the shear part
`3.43e-5` m (1.48 percent of the total).

## Arithmetic before this registration (disclosed)

`f409-scratch/py/verify.py` (numpy, a scratch script):

1. F3: the inverse of the isotropic compliance against `lambda + 2 mu`, `lambda`, `mu` for `E = 2e11`, `nu = 0.3`: 1.1e-16.
2. F5 for the constants of O4 at 15, 30, 45, 60 degrees: `eps_xx/(sigma/E_x) - 1` within 6e-16 and `eta` within 1.4e-15 of
   the numerical rotation (`eta` = -2.2430, -1.2645, -0.6132, -0.2252).
3. F7: `sigma_r(a) = -p`, `sigma_r(b) = 0`, `sigma_t(a) = 2.266667 p`; equilibrium and compatibility by central differences at
   `r = 1.37`: 1.7e-10 and 1.3e-10.
4. F8: the midpoint-rule error `-1.126e-4`; the uniform mean modulus instead `-3.82e-2`; the modulus at the first Gauss point
   of each element (mutation Mu5) `-1.04e-2`.
5. F10: the two parts of `delta_T` (above).

The bit record B1: `crates/rcfd-fem/tests/sa_ortho_default_bits.rs` was written with the API of main 0ca10e83 only and run once
in the worktree at that commit (release), before any library code; the 14 digests it printed are its constants `BASE`. It covers
the static solve with a temperature field on Hex8, Tet10 (displacement, stress, thermal strain, energy), the Hex20 cylinder
(displacement, point and nodal stress), the assembled Hex8 stiffness, the element thermal load of a distorted Hex8 and the
Shell4 plate with a temperature field (the ADR 0445 path).

## Design

Every choice below that the maintainer would normally be asked about is taken on the maintainer's standing instruction of
2026-11-11 (take the recommendation); none changes a default result, a golden or a hash table.

### D1 `mat_ortho` (filled; `PENDING = None`)

`compliance(e, nu, g)`, `check_positive_definite(e, nu, g)` (F2, `FemError::InvalidMaterial` with the key of F2),
`material_elasticity(e, nu, g)` (F3), `strain_rotation(axes)` (`T` of F4), `rotate_elasticity(c, axes)` (`T^T C T`, mirrored),
`global_thermal_strain(axes, [eps'_1, eps'_2, eps'_3])` (F6), `cylindrical_axes(origin, axis, x)` (rows radial, tangential,
axial: `e_3 = axis/|axis|`, `e_1` the normalised part of `x - origin` normal to `e_3`, `e_2 = e_3 x e_1`; a point within
`1e-12 |axis|`-relative distance of the axis is refused, key `"axes"`). `FOLLOW_UP = "a follow-up of ADR 0452"`.
`Material::orthotropic` gains the F2 check (ADR 0440 D2 left it to this package); its other refusals are unchanged.

### D2 `mat_table` (filled; `PENDING = None`)

Column names: for an `IsotropicElastic` base `e`, `nu`, `expansion`; for an `Orthotropic` base `e_1, e_2, e_3, nu_12, nu_13,
nu_23, g_12, g_13, g_23, expansion_1, expansion_2, expansion_3`. A property without a column keeps the base's value; density is
not tabulated. `check_columns` refuses an unknown name and a repeated one (key `"columns"`). It is called by the model check of
D6 (the constructor of ADR 0440 is unchanged). `interpolate(temperatures, values, t)`: the segment `i` with `t_i <= t <= t_(i+1)`
(the first such), `v_i + (v_(i+1) - v_i) (t - t_i)/(t_(i+1) - t_i)`; a `t` outside `[t_0, t_n]` (or not finite) is refused,
key `"temperature"`, reason `temperature {t:.6} K is outside the table [{t_0:.6}, {t_n:.6}] K`. A constant column gives its value
bit for bit (`v + 0 w = v`). `integrate(temperatures, values, from, to)`: the trapezoid on each segment piece of the interval
(signed, `to < from` allowed); both ends inside the table. `evaluate(material, t)`: the base with the tabulated properties at
`t`, built through the base's validating constructor (an interpolated set that is not admissible, for instance not positive
definite, is refused). `expansion_strain(material, reference, t, delta_t)`: per material direction, the base's
`alpha delta_t` without a column, `v delta_t` for a column whose entries are all bit-equal to `v` (so a constant table keeps the
bits of the constant material: the integral of a constant over `[T_ref, T]` evaluated by segments is not bit-equal to
`alpha delta_t`), else `integrate(.., reference, t)`.

### D3 The constitutive call

`Material::elasticity_at(point)`: isotropic `isotropic_elasticity(e, nu)`; orthotropic `rotate_elasticity(material_elasticity,
axes)` with the point's axes; tabulated `evaluate(material, T)` then as its base. The axes of a point: `point.axes` when given
(the caller resolved the rule, D4); otherwise the material's `Fixed` triad; a `Cylindrical` or `PerElement` rule without
`point.axes` is refused (key `"axes"`). A tabulated material without `point.temperature` is refused (key `"temperature"`).
`Material::stress` keeps its isotropic arithmetic and gives `(apply(D, eps), D)` with `D = elasticity_at(point)` for the two
variants. `Material::elasticity_matrix` keeps the isotropic arm; it gives the rotated matrix for an orthotropic material with
`Fixed` axes and refuses the other cases of the two variants as `NotYetSupported { item: "material", location: "the
element-constant elasticity matrix", package: FOLLOW_UP }` (they need the point).

### D4 The point (`mat_ortho::MaterialPoints`)

Built once per solve for a model that has an `Orthotropic` or `Tabulated` material: the model's temperature field
(`thermal::temperature_field`) and each element's index in its material's element set. `context(e, material, x, n, nodes)`
gives the `PointContext` at the integration point at `x` with shape values `n`: the axes from the rule (`Fixed`; `Cylindrical`
at `x`; `PerElement` the triad of the element's set index), and with a temperature field `dT = point_temperature_change(n,
dT_nodal)` (the function of ADR 0281) and `T = T_ref + dT`. The axes are evaluated at every integration point (not per element).

### D5 The per-point element paths

- Stiffness: `mat_ortho::element_stiffness(points, ctx)`: the loops of `Element::stiffness_with_rule` (same operations in the
  same order) with `D_q = elasticity_at(context_q)` at each point of the kind's stiffness rule.
- Thermal load: `mat_ortho::element_thermal_load(points, ...)`: the loops of `thermal::element_thermal_load` with `D_q` and
  `eps_th,q = thermal_strain(material, context_q, T_ref, dT_q)` (isotropic base: `(s, s, s, 0, 0, 0)` with `s` of
  `expansion_strain`; orthotropic: `global_thermal_strain`).
- Recovery: `IntegrationPointField::compute` takes, for an element of the two variants only, the point's context and that thermal
  strain; for every other element its code is unchanged (the isotropic arm still uses the default context).
- The edits to shared files are branches taken only for the two variants: `assemble::stiffness_with` (a private
  `assemble_stiffness` with the closure; the isotropic arm is the same expression), `thermal::add_thermal_load` (the two variants
  go to D5 before `element_thermal_load`), `thermal::element_thermal_load` (refuses the two variants: it has neither the element
  nor the reference temperature; `FOLLOW_UP` named), `recover.rs` (above), `material.rs` (D3), `model.rs` (D6). A model without
  the two variants executes the operations of main.

### D6 Which paths take the variants

Opened: the static solve on the elimination path, `solve_static`, `solve_static_with`, `solve_static_report` with any of its
solvers (D5 changes only element matrices and loads), with the recovery. `check_supported_with_materials` (new) is the check of
`check_supported_with_structures` with the two variants allowed, then `mat_ortho::check_model`: a `PerElement` list whose length is
not that of the material's set (key `"axes"`), the columns of D2, a `Tabulated` material in a model without a temperature field
(key `"temperature"`, naming the set), the reference temperature outside a table that has an expansion column, and a `Beam2` or
`Shell4` element of either variant (`NotYetSupported { item: "material", location: "element {e} ({kind})", package: FOLLOW_UP }`;
F-525's own NaN refusal in `shell::thermal_load_vector` is kept). `solve_static_report` calls it in place of
`check_supported_with_structures` and assembles through a private `stiffness_with_materials`. Every other path keeps refusing:
`check_elements` refuses the two variants unless called from the new check, as `NotYetSupported { item: "material", variant,
location: "material {i}, element set '{set}'", package: FOLLOW_UP }` (modal, buckling, prestressed modes, nonlinear, transient,
harmonic, cyclic, rotating, damped, direct harmonic, the transformation path of linear constraints and elastic supports, the
public `assemble::stiffness`).

### D7 Not changed

`thermal::thermal_strain`, `Material::expansion`, `youngs_modulus`, `poisson_ratio` (NaN or base values as in ADR 0440; every
caller refuses the variants before), the shell and beam code, the nonlinear driver.

### D8 Tests

`crates/rcfd-fem/tests/sa_ortho.rs` (O1 to O9, K1, R1 to R4, P1, N1, N2, N4) and `crates/rcfd-fem/tests/sa_mat_table.rs` (O10
to O13, R5, R6), `sa_ortho_default_bits.rs` (B1). Small models (single elements, the 10 x 1 x 1 Hex20 cantilever, a 16 x 8 x 1
Hex20 quarter tube, a 20-element bar): every test well below the two-minute limit, none ignored.

### D9 Scaffold rows (`sa_scaffold.rs`, the standing decision)

`x1_modules_and_pending_after_f413_f414` becomes an ignored record citing ADR 0291 and gets the copy
`x1_modules_and_pending_after_f409` with the rows `mat_ortho`, `mat_table` marked `"landed"`.
`m1_to_m4_material_variants_and_state_layout` becomes an ignored record citing ADR 0291 and gets the copy
`m1_to_m4_after_f409`: M2, M3 unchanged; M1 and M4 unchanged for the four other variants; for the two landed ones M1 checks
`pending() == None` and M4 checks the constitutive call of D3 (`Orthotropic` of the file's `ortho()`, `Fixed` axes:
`elasticity_matrix` equals `rotate_elasticity(material_elasticity(..), R)` bit for bit and `stress` with the default context
gives `apply(D, eps)`; `Tabulated`: `elasticity_matrix` refused naming `Tabulated` and `FOLLOW_UP`, `stress` with the default
context refused with key `"temperature"`, and with the temperature 300 K gives the isotropic matrix of the interpolated
modulus). The ignored `r1_to_r4_n3_refusals` stays as it is. The copies use the helpers of the file; nothing else changes.

## Checks

Limits are relative to the reference value unless stated. Constants: `ORTHO` = `E = (1.4e11, 1e10, 1e10)`, `nu = (0.3, 0.3,
0.45)`, `G = (5e9, 5e9, 3.5e9)` (the scaffold's), `alpha = (-1e-6, 3e-5, 2e-5)`.

| Check | Clause | Quantity | Limit | Estimate |
| --- | --- | --- | --- | --- |
| O1 | a1 | `elasticity_at` of `E_i = 2e11`, `nu_ij = nu`, `G = E/(2 (1 + nu))` against `isotropic_elasticity`, `nu` in {0, 0.3, 0.45, -0.2}: max entry difference / max entry; the same with the axes `rot((1, 2, 3), 0.5)` | `<= 1e-13` | 2e-16; 2e-15 rotated |
| O2 | a2 | refused with `InvalidMaterial` naming the key: `nu_12 = 1.05`, `E_1 = E_2` (key `nu_12`); `nu_13 = 1.05`, `E_1 = E_3` (`nu_13`); `nu_23 = 1.05`, `E_2 = E_3` (`nu_23`); all `nu = 0.5`, equal moduli (determinant 0, key `nu_12, nu_13, nu_23`); and accepted: all `nu = 0.499`; `ORTHO`; `nu_12 = -0.5` | as stated | as stated |
| O3 | a1, a2 | `compliance(ORTHO)` symmetric bit for bit; `nu_21 = -S_12 E_2` against `nu_12 E_2/E_1`, also for 13, 23; `rotate_elasticity` symmetric bit for bit | `<= 1e-15`; `==` | 1e-16 |
| O4 | a3, a4 | one Hex8 unit cube, three-two-one support (node (0,0,0) three components, (1,0,0) y and z, (0,1,0) z), traction `+-1e6` Pa on `xmax`/`xmin`, `ORTHO` with axes turned by 15, 30, 45, 60 degrees about z (rows `(c, s, 0)`, `(-s, c, 0)`, `(0, 0, 1)`); at every integration point: `abs(eps_xx/(sigma/E_x) - 1)`; `abs(gamma_xy/eps_xx - eta)/abs(eta)` (F5); the six strains against `sigma` times the first column of the test's own rotated compliance, max abs difference / `abs(eps_xx)` | each `<= 1e-10` | 1e-14 |
| O5 | a5 | one Hex8 unit cube, `xmin` clamped (three components), forces `(1e5, 2e5, -3e5)`, `(-2e5, 1e5, 1e5)`, `(3e5, -1e5, 2e5)`, `(1e5, 1e5, 1e5)` N at the four `xmax` corners in node order, `ORTHO` with axes `rot((1, -1, 2), 0.4)`; the rotated twin: nodes `Q x`, axes rows `R Q^T`, forces `Q f`, `Q = rot((1, 2, 3), 0.7)`; `W = sum f . u/2`: `abs(W'/W - 1)`; also the recovered energy `strain_energy()` of each against its `W` | `<= 1e-12` | 1e-14 (bound `eps cond`, about 1e-13) |
| O6 | a6 | Hex20 10 x 1 x 1, `L = 2`, `h = b = 0.1` (slenderness 20), the support of beam theory of ADR 0385 D1, uniform end traction `-P/(h b)`, `P = 1000`; `E = (1.4e11, 1.4e10, 1.4e10)`, `nu = (0.3, 0.3, 0.4)`, `G = (7e9, 7e9, 5e9)`, axes global (1 along the span); `delta = sum_a f_az u_az/P` against F10, `abs(delta/delta_T - 1)` | `<= 0.01` | 3e-3 (ADR 0385 isotropic 3.9e-4; the shear part is 1.5 percent and `kappa` of an orthotropic square section may differ from 5/6 by some percent) |
| O7 | a7 | one Hex8 unit cube, three-two-one support, uniform `T = T_ref + 80`, `T_ref = 293`, `ORTHO` with `alpha` above and axes `rot((1, 2, 3), 0.5)`; at every point the strain in material axes `T eps` against `alpha_a dT` (max abs difference / `max abs(alpha dT)`); the stress max abs / `max abs(C alpha dT)` | each `<= 1e-10` | 1e-15 |
| O8 | a8 | the cube of O7 with every node fixed (three components): the stress in material axes (`R sigma R^T`) against `-C alpha dT`, max abs difference / `max abs(C alpha dT)` | `<= 1e-10` | 1e-15 |
| O9 | a9 | the quarter tube of ADR 0385 (`a = 1`, `b = 2`, thickness 0.2) on 16 x 8 x 1 Hex20 (radial x circumferential x axial), `u_z = 0` on `zmin`, `zmax`, `u_y = 0` on `ymin`, `u_x = 0` on `ymax`, bore pressure `p = 1e7`; cylindrical axes (origin 0, axis z), `E = (E_r, E_t, E_z) = (1e10, 4e10, 1e10)`, `nu = (0.1, 0, 0)`, `G = (5e9, 5e9, 5e9)`; the hoop stress `t^T sigma t` of `nodal_average(None)` at every bore node against `34 p/15`: max `abs(s/s_c - 1)`; O9b the bore radial displacement against F7 | `<= 0.01`; O9b `<= 2e-3` | 2.5e-3 (ADR 0385 8 x 8 isotropic 3.75e-3, times the curvature ratio 2.35 of the hoop profile, over 4 for half the radial size); O9b 1e-5 |
| O10 | a10 | Hex8 20 x 1 x 1 bar, `L = 1`, section 0.1 x 0.1, `Tabulated` on an isotropic base (`nu = 0`, `alpha = 0`) with `e` at 293 K `2e11` and at 693 K `1e11`, nodal `T = 293 + 400 x`, `T_ref = 293`; `u_x = 0` on `xmin`, `u_y = 0` on `ymin`, `u_z = 0` on `zmin`, traction `F/A` on `xmax`, `F = 1e5`; the mean `u_x` of the `xmax` nodes against F8 | `<= 1e-3` | 1.13e-4 (F8) |
| O11 | a11 | one Hex8 unit cube, three-two-one support, isotropic base `E = 2e11`, `nu = 0.3`, column `expansion` at 250, 350, 450, 550 K equal to `a0 + a1 T`, `a0 = 1e-5`, `a1 = 2e-8`; uniform `T = 513`, `T_ref = 293`: normal strains against F9, shear strains / F9 | `<= 1e-10` | 1e-15 |
| O11b | a11 | as O11 with the `ORTHO` base, axes `rot((1, 2, 3), 0.5)`, columns `expansion_1 = a0 + a1 T` and `expansion_2` constant `3e-5`: material-axes strains against F9 (axis 1), `3e-5 dT` (axis 2), `2e-5 dT` (axis 3, the base) | `<= 1e-10` | 1e-15 |
| O12 | a12 | the B1 Hex8 cantilever (20 x 2 x 2, temperature field, force): `Tabulated(steel, [200, 400, 600], e, nu, expansion constant at the steel values)` against steel: displacement, point stress, thermal strain, strain energy bit for bit; O12b `Tabulated(ORTHO with Fixed axes, [200, 400, 600], all twelve columns constant)` against the `ORTHO` material | `==` | `==` |
| O13 | a13 | the bar of O10 with the four nodes at `x = L` at 800 K: the static solve refused with `FemError::Element { element: 19 }`, its text containing `outside the table` and a temperature (the number after `temperature `) in `(693, 800]` | as stated | as stated |
| K1 | a9 | `cylindrical_axes((0, 0, 0), (0, 0, 2), (3 cos 0.7, 3 sin 0.7, 5))` rows against `(cos, sin, 0)`, `(-sin, cos, 0)`, `(0, 0, 1)`; a point on the axis refused | `<= 1e-15` | 1e-16 |
| P1 | design D4 | O4 at 30 degrees with `PerElement([R])` instead of `Fixed(R)`: displacement bit for bit; a `PerElement` list of two triads for one element refused (key `axes`) | `==` | `==` |
| R1 | design D6 | the scaffold cantilever (Hex8 20 x 2 x 2, clamp, tip force) with `ORTHO` (`Fixed`) and with a `Tabulated` steel and a temperature field: `solve_static`, `solve_static_with` (CG Jacobi) solve; `modal_analysis_with`, `stiffness`, `nonlinear::solve`, `Integrator::new`, `buckling::factors`, `prestressed_modes`, `solve_static_mpc`, `modal_analysis_mpc` refused `NotYetSupported { item: "material", variant, location: "material 0, element set 'all'", package: FOLLOW_UP }` | as stated | as stated |
| R2 | design D6 | a Shell4 plate with `ORTHO`: the static solve refused naming `Orthotropic`, the element and `FOLLOW_UP` | as stated | as stated |
| R3 | design D3 | `elasticity_at` of a `Cylindrical` orthotropic material with the default context refused (key `axes`) | as stated | as stated |
| R4 | design D6 | a `Tabulated` material in a model without a temperature field: the static solve refused with key `temperature` | as stated | as stated |
| R5 | design D2 | a column `density` and a repeated column `e` refused (key `columns`) by the static solve | as stated | as stated |
| R6 | design D2 | a table with an expansion column and `T_ref` outside it: the static solve refused with key `temperature` | as stated | as stated |
| B1 | rule 3 | `sa_ortho_default_bits.rs` against its `BASE`; and the existing `sa_scaffold_bits.rs`, `si_hex20_support_bore.rs`, `si_hex20.rs` and `sa_scaffold.rs` (with D9) pass | `==` | `==` |
| X1 | D9 | `x1_modules_and_pending_after_f409`, `m1_to_m4_after_f409` pass | as stated | as stated |

### Negative controls

| Control | Quantity | Expected |
| --- | --- | --- |
| N1 | O6 with the axes turned by 90 degrees about z (`E_2 = E_1/10` along the span): `delta/delta_T` | `> 5` (about 10: the check sees the axes) |
| N2 | O9 with `E_t = E_r = 1e10` (`k = 1`): bore hoop against Lame `5 p/3`, and against `34 p/15` | `<= 0.01` and `> 0.2` |
| N3 | (arithmetic 4, not run) the bar with the uniform mean modulus | 3.8e-2 off: O10's limit sees the integral |
| N4 | O4 at 30 degrees with the isotropic constants of O1 (`nu = 0.3`): `abs(gamma_xy/eps_xx)` | `<= 1e-10` |

### Planned mutations (scratch copy `w49i-work/f409-scratch/mut`, the tests `sa_ortho` and `sa_mat_table`)

| Mutation | Expected to catch |
| --- | --- |
| Mu1 | `D = T C T^T` instead of `T^T C T` | O4, O5 |
| Mu2 | the factor 2 of the shear rows of `T` dropped | O4 |
| Mu3 | `S_12 = -nu_12/E_2` | O3, O4 |
| Mu4 | cylindrical axes with radial and tangential swapped | O9 |
| Mu5 | the table property at the element's first integration point for every point | O10 |
| Mu6 | the tabulated expansion as `alpha(T) dT` (secant at the end point) instead of the integral | O11 |
| Mu7 | the material-axes thermal strain used as the global one (not rotated) | O7 |
| Mu8 | the determinant test of F2 removed | O2 |
| Mu9 | the temperature outside a table clamped to the table's end | O13 |
| Mu10 | the constant-column case of `expansion_strain` removed | O12 |

## Decision rule

- **done**: every check O1 to O13, K1, P1, R1 to R6, B1, X1 met in its single registered run, N1, N2, N4 as expected, and every
  mutation caught by at least one registered check.
- **partial**: any registered check missed (kept as a `*_known_miss` record with its limit), a mutation not caught, or a check
  met only after an amendment made after its run.
- **todo**: the static path not opened.

## Limits (what is not covered)

- Only the linear static solve of solid elements (Tet4, Hex8, Tet10, Hex20 and their formulations) with the recovery takes the
  two variants; modal, buckling, transient, nonlinear, harmonic, cyclic, rotating, damped, linear constraints and elastic
  supports refuse them (`FOLLOW_UP`), as do beams and shells (a layup is F-428).
- The material axes are evaluated at the integration points; nodal output averaging across elements of different axes is not
  treated differently from the isotropic case.
- Temperature-dependent density and tables of anything but the elastic constants and the expansion are not provided; outside a
  table nothing is extrapolated.
- The cantilever limit of 1 percent hardly sees the shear term (1.5 percent of the deflection) and not `kappa` of an orthotropic
  section; O6 confirms the span modulus and the beam limit.
- The constants of a real moulded blade and its fibre directions are inputs (decision A1 of ADR 0310), not in the repository.
