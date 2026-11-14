# ADR 0496: F-428 registration: laminated Shell4 (layup stiffness by classical lamination theory, thermal resultants, layup mass), ply stresses in material axes and the maximum-stress, maximum-strain, Tsai-Hill and Tsai-Wu failure indices

Status: **REGISTERED 2026-11-14, written before any code of F-428 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-428 was written, compiled or run in the repository: the files listed under "What was
read" were read, and the arithmetic below was done by hand and evaluated with a plain numpy prototype in the scratch folder
(`w49i-work/f428-scratch/probe_clt.py`, `probe_clt.log`; no rcfd code; disclosed, its numbers quoted below). The bit record B0
of the default paths is a test file written and run on the base after this commit and before any library code (D9). Results
go in the addenda at the end; nothing above the addenda is changed after a run.

Registration of the stub [ADR 0331](0331-laminated-shell-and-ply-failure-criteria.md) (work package SA-21 of the plan
[ADR 0310](0310-structural-solver-advanced-tier-plan.md)). Roadmap item F-428 (deps F-409, F-392). Number 0496 given by the
lead. Branch `f428-work`, worktree `w49i-work/f428`, scratch `w49i-work/f428-scratch`. Base 9386de34.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code, the crate graph
unchanged; no existing test edited except by the records-and-copies scheme of ADR 0291 (D10); clean-room.

## What F-428 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. One isotropic ply gives the bits of the Shell4 of F-392. Layup stiffness: a symmetric layup has no coupling (|B| <= 1e-12 |A| t); [0/90]s has A11 = A22 = t (Q11 + Q22)/2; [0/+60/-60]s and [0/45/-45/90]s have A11 = A22, A66 = (A11 - A12)/2 and A16 = A26 = 0, all within 1e-12 relative. Simply supported square plate of a cross-ply symmetric layup under a sinusoidal load: centre deflection q0 a^4/(pi^4 (D11 + 2 (D12 + 2 D66) + D22)) within 1 percent at thickness/side 0.01 with 16 x 16 elements. Unsymmetric [0/90] strip heated uniformly: curvature equal to the closed form of the two-ply laminate (solved by hand in the registration) within 0.5 percent with zero resultants. Off-axis coupon at 15, 30, 45 and 60 degrees: ply stresses sigma c^2, sigma s^2 and -sigma s c within 1e-10, Tsai-Hill failure stress (c^4/X^2 + (1/S^2 - 1/X^2) s^2 c^2 + s^4/Y^2)^(-1/2) within 1e-10, and Tsai-Wu equal to the tensile and compressive strengths on the axes within 1e-12. A layup with a non-positive thickness or an unknown material is refused with the ply.

The brief adds: (b1) "one isotropic ply gives the bits" means the laminated path with one isotropic ply reproduces the Shell4
stiffness and results bit for bit; register how that is guaranteed; (b2) the strip needs an expansion in the ply; reuse the
shell thermal load of F-525 (and F-527 if it has landed: it has not); (b3) do not edit the solid element files of F-390 nor
`mat_visco.rs`, `damped.rs`, `harmonic_direct.rs`, `sa_harmonic_visco.rs` (F-582, F-583); (b4) no case-file key (a proposal in
the open questions); (b5) defaults keep their bits, `baseline_hashes` unchanged, a default-bits test at the base before any
library change; (b6) every aggregated error finite (an `f64::max` over NaN hides a failure); (b7) the fan target: injection
moulded, possibly glass-filled, orthotropic blade shells.

Clauses: (a0) every limit registered here; (a1) one isotropic ply gives the bits of Shell4; (a2) a symmetric layup has `|B| <=
1e-12 |A| t`; (a3) `[0/90]s`: `A11 = A22 = t (Q11 + Q22)/2` within `1e-12` relative; (a4) `[0/+60/-60]s` and `[0/45/-45/90]s`:
`A11 = A22`, `A66 = (A11 - A12)/2`, `A16 = A26 = 0` within `1e-12` relative; (a5) the simply supported cross-ply plate under
the sinusoidal load, `1e-2`; (a6) the heated `[0/90]` strip, the curvature within `5e-3` of the closed form, with zero
resultants; (a7) the off-axis coupon, ply stresses within `1e-10`; (a8) its Tsai-Hill failure stress within `1e-10`; (a9)
Tsai-Wu on the axes equal to the strengths within `1e-12`; (a10) a non-positive thickness or an unknown material refused with
the ply.

Readings, registered:

- **"Within 1e-12 relative"** (a3, a4) is relative to `A11` for every one of the identities (for `A16 = A26 = 0` there is no
  other scale); "`|B| <= 1e-12 |A| t`" (a2) uses the largest absolute entry of each matrix.
- **"Unknown material"** (a10): the library has no material names (the case-file key is not added, b4), so a ply material
  is unknown to the laminate when it is not one of the two elastic variants a ply can be: `IsotropicElastic` or
  `Orthotropic` (D1). Every other variant (Tabulated, Viscoelastic, Creep, J2, Hyperelastic) is refused with the index of the
  ply and the variant's name. "Refused with the ply": the error names `ply k` (0-based, bottom first) and, from a model, the
  block.
- **"Zero resultants"** (a6): the membrane forces and moments of `shell::stresses` (elastic strain, thermal resultants
  subtracted) at every element centre, `max |N| <= 1e-8 |N_th|` and `max |M| <= 1e-8 |M_th|` with `N_th`, `M_th` the largest
  thermal resultant components of the layup at the strip's `dT` (arithmetic 5). The acceptance gives no number; this one is
  registered here.
- **"Tsai-Hill failure stress"** (a8): `sigma / sqrt(I_TH)` with `I_TH` the Tsai-Hill index of the ply stresses of the coupon
  at the applied `sigma` (D6), i.e. the reserve factor times `sigma`; `X`, `Y` the tensile strengths (the coupon is in
  tension, so `sigma_1, sigma_2 >= 0`).
- **"Tsai-Wu equal to the tensile and compressive strengths on the axes"** (a9): the failure stress of the Tsai-Wu criterion
  (the positive root `R` of the load factor times the stress, D6) for a ply stress along each material axis, `(+-sigma, 0, 0)`
  and `(0, +-sigma, 0)`, is `X_t`, `X_c`, `Y_t`, `Y_c`; the shear `(0, 0, +-sigma)` gives `S` (a supplementary row of the same
  check).
- **"The bits of the Shell4 of F-392"** (a1): a Shell4 model whose block is `Layup` of one `IsotropicElastic` ply of
  thickness `t` (any angle) gives, bit for bit, what the same model with `Shell { t }` and the ply's material gives: the
  element stiffness and section, the assembled stiffness and consistent mass, the nodal load vector of a shell pressure and of
  a temperature field, the static displacements and reactions, `shell::stresses`, and the modal frequencies (B1 to B4).

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0331 (stub), ADR 0310 (sections 2.4, the SA-21 entry), ADR 0440 (D5 the layup property and
  its refusal, D6 the section-stiffness hook), ADR 0432 (Shell4, arithmetic 2 to 9, the SS plate set-up), ADR 0445 (F-525: the
  shell thermal load), ADR 0452 (F-409: the orthotropic material), ADR 0490 (the corotated shell check).
- `crates/rcfd-fem/src/laminate.rs` (the stub), `shell.rs` (whole), `mesh.rs` (`BlockProperty::Layup`, `Ply`), `model.rs`
  (`Material`, `check_elements`, the layup refusal), `mat_ortho.rs` (`compliance`, `check_positive_definite`), `thermal.rs`
  (`add_thermal_load`), `loads.rs` (`add_shell_body_load`), `mass.rs` (`consistent_with`, `mass_properties`),
  `shell_corot.rs` (`check_model`), `geometric_struct.rs` (its uses of `thickness_or_error`), `shell3.rs` (its uses).
- Tests: `si_shell.rs` (`ss_plate`, the Navier check), `si_shell_thermal.rs` (the free plate; `th4_th5_consistent_thermal_loads`
  with its row N2, "temperature on a Layup shell" refused with `InvalidValue { key: "property" }`), `sa_scaffold.rs`
  (`entries`, `shell_model`, `r2b_to_r4b_refusals_after_f578` with rows R3b, R4b, and
  `x1_modules_and_pending_after_f415_f420_f422_f416_f417_f432_f423_f424_f410_f411` with its `laminate` row),
  `sa_corot_shell_default_bits.rs` (the pattern of a bit record). A search of `crates/` for `Layup`, `Ply`, `laminate`,
  `0331`, `F-428`: only `sa_scaffold.rs`, `si_shell_thermal.rs`, the stub and the doc comments refer to them; nothing in
  `rcfd-api` or `rcfd-cli`.

## D0. What exists

ADR 0440 put in place everything that the element needs: `BlockProperty::Layup { plies }`, `Ply { material, thickness, angle }`
(bottom first; `Ply::new` refuses a non-positive or non-finite thickness and a non-finite angle; the fields are public, so a
ply can be changed after construction), and the section hook: the Shell4 stiffness `local_stiffness_section` takes a
`SectionStiffness { membrane A, coupling Option<B>, bending D, shear H (Isotropic or Matrix), drilling (G, t) }` with the
membrane-bending coupling term `B_m^T B B_b + B_b^T B^T B_m` and a general 2 x 2 shear matrix already in the integration loop;
`shell::stresses` already adds `B kappa` to `N` and `B^T eps` to `M`. With one isotropic ply and no coupling the element repeats
the operations of `local_stiffness` (ADR 0440 D6, checked by its H1 to H3). What is refused today: `model.rs` refuses a Layup
block on every path while `laminate::PENDING` is `Some`; `laminate::section_stiffness` refuses; `check_shell_model` requires a
`Shell` property; the mass, the thermal and body loads and `stresses` read the thickness of a `Shell` property only
(`thickness_or_error`, `InvalidValue { key: "property" }` otherwise).

F-527 (the through-thickness gradient) has not landed: no curvature thermal load exists. F-525 gives the membrane thermal
load of a homogeneous shell; this item reuses its structure (a nodal temperature field, `dT` interpolated by the facet's
functions at the 2 x 2 points, the load `sum_gp det B^T (resultant)` carried by `T^T`, and the subtraction of the thermal part
in `stresses`) and adds the layup's thermal resultants, membrane and bending, of a temperature uniform through the thickness
(D4). That is what the strip needs; a gradient through the thickness stays F-527.

## Design

### D1. The ply and its reduced stiffness (first principles; Jones, Mechanics of Composite Materials, chapter 2, cited from memory for the notation only)

Ply `k` (0-based, bottom first), thickness `t_k > 0`, angle `theta_k` of its material axis 1 from the element's axis `e_1`
about the normal `e_3` (counter-clockwise seen from `+e_3`). The ply's material, in its own axes `1, 2` in the plane and `3`
along the normal:

- `IsotropicElastic { E, nu, density, expansion }`: `E1 = E2 = E`, `nu12 = nu`, `G12 = G13 = G23 = E/(2 (1 + nu))`, `alpha1 =
  alpha2 = expansion`.
- `Orthotropic { e, nu, g, density, expansion, axes }` (ADR 0452: `nu = (nu12, nu13, nu23)`, `g = (G12, G13, G23)`, `nu21 =
  nu12 E2/E1`): `E1 = e[0]`, `E2 = e[1]`, `nu12 = nu[0]`, `G12 = g[0]`, `G13 = g[1]`, `G23 = g[2]`, `alpha1 = expansion[0]`,
  `alpha2 = expansion[1]`. Its `axes` rule is not used: the ply's angle gives its axes in the element (documented; Limits).

Plane stress `sigma_3 = 0` in the ply (thin shell): the reduced stiffness in material axes, engineering shear strain,
`sigma = Q eps`, from inverting the in-plane block of the compliance `[[1/E1, -nu12/E1], [-nu12/E1, 1/E2]]`:
`Q11 = E1/(1 - nu12 nu21)`, `Q22 = E2/(1 - nu12 nu21)`, `Q12 = nu12 E2/(1 - nu12 nu21)`, `Q66 = G12`, `Q16 = Q26 = 0`. For the
isotropic ply this is `E/(1 - nu^2) [[1, nu, 0], [nu, 1, 0], [0, 0, (1 - nu)/2]]` in exact arithmetic (the shell's
`plane_matrix`).

**Rotation** (first principles). With `c = cos theta`, `s = sin theta`, the stress in material axes from element axes is
`sigma_m = T_s sigma_e`, `T_s = [[c^2, s^2, 2 c s], [s^2, c^2, -2 c s], [-c s, c s, c^2 - s^2]]` (the tensor rotation in Voigt
form), and the engineering strain `eps_m = T_e eps_e`, `T_e = [[c^2, s^2, c s], [s^2, c^2, -c s], [-2 c s, 2 c s, c^2 - s^2]]`.
Then `sigma_e = T_s^{-1} Q T_e eps_e`, so `Qbar = T_s(-theta) Q T_e(theta)` (`T_s^{-1} = T_s(-theta)`). The code forms
`Qbar` by these two products (no hand-expanded formulas); the expanded forms of Jones are used only as a cross-check in the
probe. The expansion in element axes (engineering shear): `alphabar = T_e^{-1} (alpha1, alpha2, 0) = T_e(-theta) (alpha1,
alpha2, 0) = (alpha1 c^2 + alpha2 s^2, alpha1 s^2 + alpha2 c^2, 2 c s (alpha1 - alpha2))`. Transverse shear `(gamma_xz,
gamma_yz)`: `gamma_13 = c gamma_xz + s gamma_yz`, `gamma_23 = -s gamma_xz + c gamma_yz`, so `Hbar_k = R^T diag(G13, G23) R`,
`R = [[c, s], [-s, c]]`.

**Refusals** (`laminate::check_plies`, a10): for each ply in order, a thickness that is not positive and finite, an angle
that is not finite, a material that is neither `IsotropicElastic` nor `Orthotropic`: `FemError::InvalidValue { key: "ply",
reason: "ply k: ..." }` naming the ply, the value or the variant. An empty list is refused (`BlockProperty::layup` already
does; the laminate refuses it again, for a changed `Vec`). From a model, `check_shell_model` refuses with the same error whose
reason starts `block b, ply k: ...`.

### D2. The laminate (classical lamination theory; first principles, the integrals of Reddy, Mechanics of Laminated Composite Plates and Shells, chapter 3, cited from memory)

Total thickness `t = sum t_k`, mid-plane at `z = 0`, `z_0 = -t/2`, `z_{k+1} = z_k + t_k`; the ply's mid-height `zbar_k = z_k +
t_k/2`. With `u(z) = u0 + z beta` (the shell's `beta_x = theta_y`, `beta_y = -theta_x`, ADR 0432), `eps(z) = eps0 + z kappa`,
the resultants `N = int sigma dz`, `M = int sigma z dz` of `sigma = Qbar_k (eps(z) - alphabar_k dT)`:

`N = A eps0 + B kappa - N_th dT`, `M = B eps0 + D kappa - M_th dT`, with

- `A = sum Qbar_k t_k`, `B = sum Qbar_k t_k zbar_k`, `D = sum Qbar_k (t_k zbar_k^2 + t_k^3/12)` (the same integrals as
  `(z_{k+1}^2 - z_k^2)/2` and `(z_{k+1}^3 - z_k^3)/3`, in the form without cancellation);
- `N_th = sum Qbar_k alphabar_k t_k`, `M_th = sum Qbar_k alphabar_k t_k zbar_k` (per kelvin, `dT` uniform through the
  thickness);
- the transverse shear `H = kappa_s sum Hbar_k t_k` with `kappa_s = 5/6` for every layup (`shell::SHEAR_FACTOR`; a registered
  rule, the homogeneous value; the layup-dependent factor is a Limit: the checks are thin);
- the drilling pair `(G, t) = (A66/t, t)`: the penalty `alpha G t = alpha A66`, the homogeneous `alpha G t` for one isotropic
  ply;
- the mass per area `m0 = sum rho_k t_k`, its first moment `m1 = sum rho_k t_k zbar_k` and the rotary term `m2 = sum rho_k
  (t_k zbar_k^2 + t_k^3/12)` (D5).

`laminate::laminate(plies) -> Result<Laminate>` returns all of these (the general path, always the sums above).
`laminate::section_stiffness(plies)` returns the `SectionStiffness`: **one `IsotropicElastic` ply** gives
`shell::homogeneous_section(&ply.material, ply.thickness)` (the route of D3); any other layup gives `membrane A, coupling
Some(B), bending D, shear Matrix(H), drilling [A66/t, t]`. `coupling` is `Some` for every layup of the general path, also a
symmetric one (its `B` is rounding, arithmetic 2), so that no layup changes branch by a rounding.

### D3. The bits of one isotropic ply (a1, b1): a route, not identical arithmetic

The general sums do not repeat the homogeneous arithmetic: `Q66 = E/(2 (1 + nu))` against `0.5 (1 - nu) E t/(1 - nu^2)`,
`t zbar^2 + t^3/12` with `zbar = 0` against `t t t/12`, `H` as a `Matrix` against `Isotropic(5/6 G t)` (a different loop in
`local_stiffness_section`), so bit equality cannot be argued for them. The guarantee is a route: `laminate::single_isotropic
(plies)` is `Some((material, t))` exactly when the layup has one ply whose material is `IsotropicElastic`, and every Layup
branch of this item (section, mass, thermal load, stresses; D2, D4, D5, D7) takes, for it, the existing homogeneous function
with that material and thickness, the same function call with the same arguments as a `Shell { t }` block of that material.
The angle of an isotropic ply is not used (an isotropic ply is invariant under the rotation in exact arithmetic). The general
path for one isotropic ply is checked against the homogeneous section within rounding (BT5), so the route is not hiding a
disagreement.

### D4. The layup thermal load (b2; the F-525 structure with the layup's resultants)

`shell::layup_thermal_load_vector(coords, laminate, delta_t_nodal)`: at each 2 x 2 point of the stiffness, `dT = sum N_i
dT_i`, `fl += det (B_m^T N_th dT + B_b^T M_th dT)`, carried to the nodes by `T^T`; the bilinear membrane (a Layup block has no
membrane choice: `model_membrane` gives the bilinear one for every block that is not `Shell`). `thermal.rs` calls, for a Shell4
element, `shell::model_thermal_load_vector(mesh, e, coords, material, dte)`, which for a `Shell` block does exactly what
`thermal.rs` does today (`thickness_or_error` then `thermal_load_vector_with(coords, t, material, dte, model_membrane)`), for
a single isotropic ply the same call with the ply's material and thickness, and for any other layup the function above. A
Shell3 element is unchanged.

### D5. The layup mass

`shell::local_mass_layup(f, m0, m1, m2)`: the kinetic energy `int rho (u0 + z beta)^2 dz = m0 u0^2 + 2 m1 u0 . beta + m2
beta^2` with the shapes and the 2 x 2 rule of `local_mass`: `m0` on the three translations, `m2` on the three rotations (the
drilling rotation takes `m2` as in `local_mass`), and the coupling `M[u_x(i), theta_y(j)] += m1 s`, `M[u_y(i), theta_x(j)] -=
m1 s` (and the transposes), `s = det N_i N_j`. For a mid-plane-symmetric density `m1` is rounding. `model_element_mass` of a
Layup block: one isotropic ply takes `element_mass(coords, t, ply material)`, any other layup this local mass carried by
`T^T M T`.

### D6. Ply stresses and failure indices

`laminate::ply_stresses_at(plies, eps0, kappa, dT) -> Result<Vec<PlyStress>>`: for each ply at its bottom `z_k` and top
`z_{k+1}`, the mechanical strain in element axes `eps(z) - alphabar_k dT`, rotated to material axes by `T_e`, and the stress
`Q eps_m`: `PlyStress { ply, z: [bottom, top], strain: [[eps1, eps2, gamma12]; 2], stress: [[sigma1, sigma2, tau12]; 2] }`.
`laminate::layup_response(model, displacement)`: for every Shell4 element of a Layup block, the centre values of `shell::stresses`
(the strain `eps0`, the curvature `kappa` from the rows at the centre, `dT` at the centre) and its ply stresses:
`LayupResponse { element, strain, curvature, delta_t, plies }`.

Strengths `PlyStrengths::new(xt, xc, yt, yc, s)` (all positive and finite, the compressive ones as magnitudes; refused
otherwise, key `"strength"`). Criteria (`laminate::Criterion`), each giving `FailureIndex { index, reserve }` where `reserve`
is the factor on the ply's stress at which the criterion reaches failure (infinite for a zero stress) and `index` the
criterion's value at the stress (failure at 1):

- **MaxStress**: `index = max(sigma1/X, sigma2/Y, |tau12|/S)` with `X = X_t` for `sigma1 >= 0`, `X_c` with `-sigma1` otherwise
  (and `Y` alike); `reserve = 1/index`.
- **MaxStrain**: the same on the mechanical strains with the linear-elastic allowables `X_t/E1`, `X_c/E1`, `Y_t/E2`, `Y_c/E2`,
  `S/G12` (a registered reading: the repository has no strain allowables); `reserve = 1/index`.
- **TsaiHill** (Hill's quadratic form for a transversely isotropic ply as used by Tsai; Jones chapter 2, from memory):
  `index = (sigma1/X)^2 - sigma1 sigma2/X^2 + (sigma2/Y)^2 + (tau12/S)^2`, `X`, `Y` chosen by the signs as above (a
  registered rule; the coupon is in tension); `reserve = 1/sqrt(index)`.
- **TsaiWu { f12_star }** (Tsai and Wu, J. Compos. Mater. 5 (1971) 58, cited by the stub; the form from memory): `F1 = 1/X_t -
  1/X_c`, `F2 = 1/Y_t - 1/Y_c`, `F11 = 1/(X_t X_c)`, `F22 = 1/(Y_t Y_c)`, `F66 = 1/S^2`, `F12 = f12_star sqrt(F11 F22)`;
  `index = F1 sigma1 + F2 sigma2 + F11 sigma1^2 + F22 sigma2^2 + F66 tau12^2 + 2 F12 sigma1 sigma2`; the reserve `R` is the
  positive root of `a R^2 + b R - 1 = 0`, `a` the quadratic and `b` the linear part: `R = 2/(b + sqrt(b^2 + 4 a))` for `b >=
  0` and `(sqrt(b^2 + 4 a) - b)/(2 a)` for `b < 0` (both without cancellation); `a = 0` gives `1/b` for `b > 0` and infinity
  otherwise. `f12_star` must lie in `(-1, 1)` (the failure surface closed; refused otherwise, key `"f12_star"`); the default
  `Criterion::tsai_wu()` takes `f12_star = -1/2` (a common choice, attributed to Tsai and Hahn; from memory, **unverified**).
  On the axes the root is exact: `sigma1 = sigma > 0` alone gives `sigma^2 R^2/(X_t X_c) + sigma R (1/X_t - 1/X_c) = 1`, whose
  roots are `R sigma = X_t` and `-X_c` (arithmetic 7).

### D7. The model paths

- `laminate::PENDING = None`, `laminate::FOLLOW_UP = "a follow-up of ADR 0496"`. The gate of `model.rs` then passes a Layup
  block (no edit of `model.rs`).
- `check_shell_model`: a Shell4 block may have `Shell` or `Layup` (the layup checked by `check_plies`, D1); a Shell3 block with
  `Layup` is refused `NotYetSupported { item: "shell property", variant: "Layup", location: "block b (Shell3)", package:
  laminate::FOLLOW_UP }`; a shell block without either keeps its error.
- `thickness_or_error`: a Layup block gives `NotYetSupported { item: "shell property", variant: "Layup", location: "element e",
  package: laminate::FOLLOW_UP }` in place of the `InvalidValue { key: "property" }` it gives today; every path that needs one
  thickness and is not made layup-aware here refuses so: gravity, the centrifugal load and the nodal body force (`loads.rs`),
  the geometric stiffness and spin softening (`geometric_struct.rs`: buckling, pre-stressed modes, the rotating paths), the
  Shell3 functions. `shell_thickness` (public) stays `None` for a Layup block.
- Layup-aware: the stiffness (through `model_section`, no edit), the mass (D5), the thermal load (D4), the shell pressure (no
  thickness), `stresses` (D2 resultants with `N_th`, `M_th` subtracted; for a Layup element `top`/`bottom` are the element-axes
  stresses `Qbar (eps0 + z kappa - alphabar dT)` of the outer plies at `z = +-t/2`), `layup_response` (D6).
- `shell_corot::check_model` (the Newton driver, `nonlinear::solve`): a Layup block refused `NotYetSupported { item: "shell
  property", variant: "Layup", location: "block b", package: laminate::FOLLOW_UP }` (the corotated shell takes the isotropic
  material only, ADR 0490).
- The block's own material (`add_material` on its set, which the model still requires) is not used by a Layup element's
  stiffness, mass, thermal load or stresses: the plies' materials are (documented; BT4 checks it).

### D8. Files

`crates/rcfd-fem/src/laminate.rs` (the module), `shell.rs` (D3 to D5, D7: `check_shell_model`, `thickness_or_error`,
`model_element_mass`, `model_thermal_load_vector`, `layup_thermal_load_vector`, `local_mass_layup`, `stresses`, a pub(crate)
centre-strain helper), `thermal.rs` (the Shell4 branch calls `model_thermal_load_vector`: lines of the shell branch only),
`shell_corot.rs` (`check_model`), `mesh.rs` (doc comments only), `lib.rs` (doc only if needed). Tests
`crates/rcfd-fem/tests/sa_laminate.rs`, `sa_laminate_default_bits.rs`, the copies of D10. No file of F-390, F-582, F-583.

### D9. The bit record (B0, b5)

`sa_laminate_default_bits.rs`, written and run on the base after this commit, before any library code, its digests printed by
that run and written into the file in the same commit: FNV-1a 64 over the bits of (1) the static displacement and reactions of
a warped Shell4 plate (3 x 3, after `sa_corot_shell_default_bits.rs`) under a shell pressure, a nodal moment and a nodal
temperature field; (2) its `shell::stresses` (membrane, bending, shear, top, bottom of every element); (3) its consistent mass
values; (4) its first four modal frequencies; (5) the same static solve of the Shell3 twin with the temperature; (6)
`nonlinear::solve` of the Shell4 plate (pressure and moment, two steps); (7) the buckling factors of the Shell4 plate under an
in-plane end load (pre-stress through `geometric_struct`); (8) the static solve of a Hex8 cantilever under a temperature field.
B0 must give the same digests after the change.

### D10. Existing tests superseded by design (ADR 0291 records and copies)

The item makes three existing tests fail by design: `sa_scaffold.rs` `r2b_to_r4b_refusals_after_f578` (R3b: the Layup model
refused on every entry naming ADR 0331; R4b: `section_of` and `section_stiffness` refuse),
`x1_modules_and_pending_after_f415_f420_f422_f416_f417_f432_f423_f424_f410_f411` (`laminate::PENDING` is `Some`), and
`si_shell_thermal.rs` `th4_th5_consistent_thermal_loads` (N2: a temperature on a Layup shell refused with `InvalidValue {
key: "property" }`). Each gets `#[ignore = "record: ... continued in <copy>"]` (the attribute only; the body unchanged) and a
copy with only the superseded rows changed:

- `r2b_to_r4b_refusals_after_f428`: R2c unchanged; **R3c**: for each entry `E1` to `E10` of the scaffold, the layup model (the
  scaffold's `[0/90]` of two isotropic plies) against its `Shell { 0.01 }` twin: `E5` refuses the layup `NotYetSupported`
  naming `laminate::FOLLOW_UP`; every other entry gives the twin's outcome class (both `Ok`; or both `NotYetSupported` naming
  the same package; or the same error variant); **R4c**: `section_of(Layup)` and `laminate::section_stiffness` are `Ok` and
  equal.
- `x1_modules_and_pending_after_..._f411_f428`: the `laminate` row `landed`, the rest unchanged.
- `th4_th5_consistent_thermal_loads_after_f428`: N2 becomes **N2b**: the nodal load vector of the temperature on a one-ply
  isotropic Layup shell equals, bit for bit, that of its `Shell` twin; every other row unchanged.

If the whole-crate run shows another existing test that fails by design, it is handled the same way by an amendment written
before its corrected run.

## Arithmetic done before the code (disclosed)

Registered data (b7: a glass-filled moulded plastic, not a measured material; the repository has none): the orthotropic ply
`E1 = 10 GPa`, `E2 = E3 = 4 GPa`, `nu12 = nu13 = 0.35`, `nu23 = 0.45`, `G12 = G13 = 1.6 GPa`, `G23 = 1.4 GPa`, `rho = 1400
kg/m^3`, `alpha1 = 2e-5`, `alpha2 = alpha3 = 6e-5 1/K` (the `Orthotropic` constructor's checks pass: `nu_ij^2 < E_i/E_j`).
Strengths `X_t = 150 MPa`, `X_c = 110 MPa`, `Y_t = 60 MPa`, `Y_c = 90 MPa`, `S = 50 MPa`.

1. **Q** (probe): `Q11 = 1.0515247108e10`, `Q12 = 1.4721345952e9`, `Q22 = 4.2060988433e9`, `Q66 = 1.6e9 Pa` (`nu21 = 0.14`,
   `1 - nu12 nu21 = 0.951`).
2. **Layup identities** (probe, plies of equal thickness, `t = 2 mm`): `[0/90]s`: `|B|/(|A| t) = 1.5e-17`, `A11 vs t (Q11 +
   Q22)/2` `1.1e-16`, `A66 - (A11 - A12)/2` `0.18` (not quasi-isotropic: the negative control N2); `[0/60/-60]s`: `|B|/(|A|
   t) = 5.9e-17`, the three quasi-isotropic identities at most `2.8e-16`; `[0/45/-45/90]s`: `1.7e-17`, at most `1.4e-16`; the
   unsymmetric `[0/90]`: `|B|/(|A| t) = 0.107` (N1); a symmetric layup of unequal plies `[30 (0.3 mm)/-45 (0.2)/0 (0.5)]s`:
   `5.8e-17`, `A16 = 3.0e-2 A11` (not balanced: N2). Margins to `1e-12`: above three decades.
3. **The cross-ply plate** (a5; Kirchhoff's specially orthotropic plate, Navier's single term, first principles: `D16 = D26 =
   0`, `w = W sin(pi x/a) sin(pi y/a)` satisfies `D11 w_xxxx + 2 (D12 + 2 D66) w_xxyy + D22 w_yyyy = q0 sin sin` with
   `pi^4/a^4 (D11 + 2 (D12 + 2 D66) + D22) W = q0` for `a = b`). Layup `[0/90]s` of four plies `t/4`, `a = 1 m`, `t = 0.01 m`,
   `q0 = 1000 Pa`. By hand `D11 = t^3 (7 Q11 + Q22)/96`, `D22 = t^3 (Q11 + 7 Q22)/96`, `D12 = Q12 t^3/12`, `D66 = Q66
   t^3/12` (probe: the sums equal these within rounding); `D11 = 810.5503`, `D12 = 122.6779`, `D22 = 416.2285`, `D66 =
   133.3333 N m`, the denominator `2005.468 N m`, `w_c = 5.119e-3 m`. Estimate of the miss: the transverse shear adds about
   `pi^2 D11/(a^2 kappa_s G13 t) = 6.0e-4`; the 16 x 16 discretisation of Shell4 gave `7.5e-4` for the isotropic plate under
   a uniform load (ADR 0432 SS1, which includes its shear part); expected below `2e-3`, five times inside `1e-2`. The control
   N3 (the formula with `D12 + D66`) is `15.3` percent away (probe ratio `1.1534`). The frequency of the (1,1) mode (first
   principles, the same single term with `m0 omega^2`): `omega = pi^2/a^2 sqrt((D11 + 2 (D12 + 2 D66) + D22)/m0)`, `f = 18.80
   Hz` (probe); ADR 0432 FQ1 met the isotropic (1,1) within `0.39` percent at 16 x 16.
4. **The heated `[0/90]` strip, solved by hand** (a6). Two plies of `h = t/2`: ply 0 at `0` degrees on `z in [-h, 0]`, ply 1 at
   `90` degrees on `[0, h]`. The 90-degree ply has `Qbar = Q` with indices 1 and 2 exchanged and `alphabar = (alpha2, alpha1,
   0)`. Then `A11 = A22 = (Q11 + Q22) h`, `A12 = 2 Q12 h`, `A66 = 2 Q66 h`; `zbar = -h/2, +h/2`, so `B11 = (Q22 - Q11) h^2/2 =
   b`, `B22 = -b`, `B12 = B66 = 0`; `D11 = D22 = (Q11 + Q22) h^3/3`, `D12 = 2 Q12 h^3/3`. With `p = Q11 alpha1 + Q12 alpha2`,
   `q = Q12 alpha1 + Q22 alpha2`: `N_th = (p + q) h (1, 1, 0)`, `M_th = (q - p) h^2/2 (1, -1, 0)` per kelvin. All shear and
   twist terms vanish. A free strip, `N = 0`, `M = 0`:
   `(A11) e1 + A12 e2 + b k1 = n`, `A12 e1 + A11 e2 - b k2 = n`, `b e1 + D11 k1 + D12 k2 = m`, `-b e2 + D12 k1 + D11 k2 = -m`,
   with `n = (p + q) h dT`, `m = (q - p) h^2 dT/2`. The ansatz `e1 = e2 = e`, `k2 = -k1 = -k` satisfies all four when
   `(A11 + A12) e + b k = n` and `b e + (D11 - D12) k = m`; the system is non-singular, so this is the solution:
   `k = (m (A11 + A12) - b n)/((A11 + A12)(D11 - D12) - b^2)`. Inserting: the numerator is `h^3 dT/2 [(q - p) S - (Q22 - Q11)
   (p + q)]` with `S = Q11 + Q22 + 2 Q12`, whose `alpha1` coefficient is `-2 (Q11 Q22 - Q12^2)` and `alpha2` coefficient `+2
   (Q11 Q22 - Q12^2)` (expanded by hand), so the numerator is `h^3 dT Delta (alpha2 - alpha1)`, `Delta = Q11 Q22 - Q12^2`; the
   denominator is `h^4 [((Q11 + Q22)^2 - 4 Q12^2)/3 - (Q22 - Q11)^2/4] = h^4 [(Q11 - Q22)^2 + 16 Delta]/12` (with `(Q11 +
   Q22)^2 = (Q11 - Q22)^2 + 4 Q11 Q22`). Hence, with `t = 2 h`,

   `kappa_x = -kappa_y = 24 Delta (alpha2 - alpha1) dT / (t ((Q11 - Q22)^2 + 16 Delta))`, `kappa_xy = 0`,
   `eps_x = eps_y = (n - b kappa_x)/((Q11 + Q22 + 2 Q12) h)`, `gamma_xy = 0`.

   Checks of the hand result: (i) `alpha1 = alpha2` gives no curvature; (ii) with `Q12 = 0`, `Q11 = E1`, `Q22 = E2` it is
   Timoshenko's bimetal strip of equal thicknesses (`kappa = 24 n dalpha dT/(t (n^2 + 14 n + 1))`, `n = E1/E2`; the bimetal
   formula from memory, the agreement by algebra: `24 E1 E2/((E1 - E2)^2 + 16 E1 E2)` equals it); (iii) the probe solves the
   6 x 6 `[A B; B D]` system numerically: `kappa_x` equal to the closed form within `0` (bits), `kappa_y` within `2.2e-16`,
   `eps` within `2.2e-16`. Sign: the top ply expands more along `x` when `alpha2 > alpha1`, so `kappa_x > 0` with `u = u0 + z
   beta`. Registered strip: `t = 2 mm`, `L x W = 0.1 x 0.02 m`, 20 x 4 Shell4 elements, `dT = 10 K` (`T = 303.15 K` on
   `T_ref = 293.15 K`), node 0 held in its six unknowns; `kappa_x = 0.2832464631 1/m`, `eps = 3.7915115e-4` (probe). A linear
   check (the deflection `kappa L^2/8 = 3.5e-4 m` is a sixth of the thickness; geometric nonlinearity is not asked).
   **Why the element is exact here**: the state is uniform (`eps0`, `kappa` constant, no shear strain); the nodal values of
   `u = e x`, `v = e y`, `beta = (k x, -k y)`, `w = -k (x^2 - y^2)/2` (plus the rigid motion fixed by node 0) give in every
   element the exact membrane strain and curvature (bilinear fields reproduce linear `u`, `v`, `beta`), a zero MITC4 shear
   strain at the tying points (`(w_2 - w_1)/L + (beta_1 + beta_2)/2 = 0` along each edge for quadratic `w`) and a zero
   drilling term (`theta_z = omega = 0`); the resultants are `N_th dT`, `M_th dT` at every 2 x 2 point, so the internal force
   equals the consistent thermal load at every unknown: the nodal interpolant is the discrete solution. Hence the 5e-3 of the
   acceptance and a supplementary `1e-8` (rounding of a 600-unknown solve; F-525 found its free plate's strains and stresses
   within `1e-14`).
5. **Thermal resultants of the strip** (probe, per kelvin): `N_th = 580.44 N/(m K)` on `x` and `y`, `M_th = (-8.412e-3,
   8.412e-3, 0) N/K`; at `dT = 10`: `|N_th| = 5804 N/m`, `|M_th| = 0.0841 N`.
6. **The off-axis coupon** (a7, a8). One orthotropic ply at `theta` (15, 30, 45, 60 degrees), `t = 2 mm`, `L x W = 0.1 x 0.02
   m`, 10 x 2 elements; the edge `x = L` loaded by the consistent nodal forces of `sigma = 1 MPa` (`F = sigma W t = 40 N`,
   `F/4, F/2, F/4` on its three nodes, along `x`); `u = 0` on `x = 0`, `v = 0` at node 0, `w = theta_x = theta_y = 0` at every
   node (no bending: one ply has `B = 0` and the load is in-plane). The exact state is uniform: `u = eps_x x`, `v = gamma_xy x
   + eps_y y` meets the supports, `theta_z = gamma_xy/2` is its drilling rotation, so the element is exact (the membrane patch
   test) and the ply stresses are `T_s (sigma, 0, 0) = (sigma c^2, sigma s^2, -sigma s c)`. Probe of the closed forms (the
   criteria evaluated on these stresses; no element): ply stresses exact to `5.8e-17`, Tsai-Hill failure stress against the
   acceptance's formula within `2.2e-16` (126.84, 95.77, 76.82, 66.56 MPa at 15, 30, 45, 60 degrees). The 1e-10 of the
   acceptance covers the rounding of the solve (no probe of the element; the membrane patch test of ADR 0432 met `7.4e-14`).
   Negative control N5: the maximum-stress failure stress is `20` to `30` percent above Tsai-Hill's (probe: ratio `1.27,
   1.21, 1.30, 1.20`), so the Tsai-Hill check is not met by another criterion.
7. **Tsai-Wu on the axes** (a9; probe): `R sigma` for `(+-1 MPa, 0, 0)`, `(0, +-1 MPa, 0)`, `(0, 0, +-1 MPa)` is `150, 110, 60,
   90, 50, 50 MPa` with the error `0` in the probe. The limit `1e-12` of the acceptance is about four thousand ulps.

## Checks (limits fixed now)

Every aggregated error is the maximum of finite values; a non-finite value is a miss of its check (b6). Relative errors are
`|x/x_ref - 1|` unless stated.

| id | clause | quantity | limit | estimate |
|---|---|---|---|---|
| BT1 | a1 | `section_stiffness([ply(steel, t, theta)])` against `homogeneous_section(steel, t)`, `theta = 0` and `0.3` | equal (`==`, bits) | D3: the route |
| BT2 | a1 | the warped 3 x 3 plate of B0, Layup of one isotropic ply against `Shell { t }`: assembled stiffness values, consistent mass values, nodal load vector (shell pressure, nodal moment, temperature field) | bits equal | D3 |
| BT3 | a1 | the same: static displacements and reactions, `shell::stresses` (all five arrays), the first four modal frequencies | bits equal | D3 |
| BT4 | a1 | BT2 and BT3 with the Layup block's own material changed (density, modulus and expansion different from the ply's) | bits equal to the `Shell` twin of the ply's material | D7: the block material unused |
| BT5 | (a1) | the general path `laminate(plies)` of one isotropic ply against `homogeneous_section`: `A`, `D` entries, `H` against `5/6 G t I` | max relative to the largest entry `<= 1e-14`; `B == 0` exactly (`zbar = -t/2 + t/2 = 0`) | rounding of a few operations |
| L1 | a2 | `max |B| / (max |A| t)` for `[0/90]s`, `[0/60/-60]s`, `[0/45/-45/90]s` and the unequal `[30/-45/0]s` | `<= 1e-12` each | arithmetic 2: at most `5.9e-17` |
| L2 | a3 | `[0/90]s`: `A11` and `A22` against `t (Q11 + Q22)/2`, `Q` computed in the test from the constants (D1, not the library) | `<= 1e-12` | `1.1e-16` |
| L3 | a4 | `[0/60/-60]s`, `[0/45/-45/90]s`: `|A11 - A22|`, `|A66 - (A11 - A12)/2|`, `|A16|`, `|A26|`, each over `A11` | `<= 1e-12` | at most `2.8e-16` |
| L4 | (a3) | `[0/90]s` at `t = 0.01`: `D11`, `D22`, `D12`, `D66` against the hand forms of arithmetic 3 | `<= 1e-13` | rounding |
| PL1 | a5 | the SS `[0/90]s` plate, 16 x 16, `t/a = 0.01`, sinusoidal load: `|w_c / w_Navier - 1|`, `w_Navier = q0 a^4/(pi^4 (D11 + 2 (D12 + 2 D66) + D22))` with the hand `D` of arithmetic 3 | `<= 1e-2` | arithmetic 3: below `2e-3` |
| PL2 | (a5) | its (1,1) frequency against `pi^2/a^2 sqrt((D11 + 2 (D12 + 2 D66) + D22)/m0)` | `<= 1e-2` | ADR 0432 FQ1: `3.9e-3` |
| ST1 | a6 | the heated `[0/90]` strip: `kappa_x` and `-kappa_y` of every element (`layup_response`) against the closed form of arithmetic 4 | `<= 5e-3` | exact up to rounding |
| ST2 | a6 | its resultants: `max |N| / |N_th dT|`, `max |M| / |M_th dT|` over the elements (`shell::stresses`) | `<= 1e-8` each | exact up to rounding |
| ST3 | (a6) | supplementary: ST1 at `1e-8`; `|kappa_xy| / kappa_x`; `eps_x`, `eps_y` against the closed form; `|gamma_xy| / eps` | `<= 1e-8` each | arithmetic 4 |
| CP1 | a7 | the off-axis coupon at 15, 30, 45, 60 degrees: `(sigma1, sigma2, tau12)` of every element, bottom and top (`layup_response`) against `sigma (c^2, s^2, -s c)` | max error / `sigma` `<= 1e-10` | exact up to rounding |
| CP2 | a8 | its Tsai-Hill failure stress `reserve sigma` per element against `(c^4/X^2 + (1/S^2 - 1/X^2) s^2 c^2 + s^4/Y^2)^(-1/2)`, `X = X_t`, `Y = Y_t` | `<= 1e-10` | arithmetic 6 |
| TW1 | a9 | Tsai-Wu (`f12_star = -1/2`) failure stress `reserve sigma` for `(+-sigma, 0, 0)`, `(0, +-sigma, 0)` against `X_t`, `X_c`, `Y_t`, `Y_c`; and `(0, 0, +-sigma)` against `S` | `<= 1e-12` each | arithmetic 7: `0` |
| TW2 | (a9) | the same with `f12_star = 0` and `0.3` (`F12` does not act on the axes) | `<= 1e-12` | algebra |
| FC1 | (a8) | maximum stress and maximum strain on the axes: `reserve sigma` against the five strengths (stress) and, for the strain criterion under the uniaxial stress `(sigma, 0, 0)` and `(0, sigma, 0)`, against `X_t` and `Y_t` (the axial allowable governs: arithmetic below) | `<= 1e-12` | algebra |
| RF1 | a10 | a layup whose ply 1 has its thickness set (public field) to `0`, `-1e-3`, `NaN`: `section_stiffness`, `laminate` refuse `InvalidValue { key: "ply" }` with `"ply 1"` in the message | refused | D1 |
| RF2 | a10 | a layup whose ply 2 has a `J2`, `Tabulated`, `Hyperelastic`, `Creep`, `Viscoelastic` material: refused, the message names `ply 2` and the variant | refused | D1 |
| RF3 | a10 | a Shell4 model with such a layup (thickness `-1e-3` in ply 1; a J2 ply 0): `solve_static` refuses naming `block 0` and the ply | refused | D7 |
| RF4 | (a10) | an angle `NaN` set on a ply, an empty ply list, `PlyStrengths` with a zero or negative strength, `f12_star = 1` and `-1`: refused | refused | D1, D6 |
| MS1 | (D5) | the mass of a free layup plate (`mass_properties` of the consistent mass): total against `area sum rho_k t_k`; for a two-ply layup with densities `1400` and `2800` the centre's offset along `e_3` against `m1/m0 = (rho1 - rho0) t/(4 (rho0 + rho1))` | `<= 1e-12` relative (offset relative to `t`) | first principles |
| RX1 | D7 | an existing test superseded: R3c, R4c of D10 | as registered in D10 | D7 |
| RX2 | D7 | a Shell4 Layup model (two orthotropic plies) under gravity, and its buckling factors under an in-plane pre-stress (`buckling::factors`), and `nonlinear::solve`: each refused `NotYetSupported` naming `laminate::FOLLOW_UP` (directly or inside `FemError::Element`) | refused | D7 |
| X1 | D7 | the `laminate` row of the X1 copy: `PENDING` is `None` | holds | |
| N2b | D10 | the temperature on a one-ply Layup shell: its nodal load vector equals the `Shell` twin's | bits equal | D3 |

Negative controls (each must show the check is not vacuous):

| id | control | expected |
|---|---|---|
| N1 | L1 on the unsymmetric `[0/90]` | `max |B| / (max |A| t) > 1e-3` (probe `0.107`) |
| N2 | L3 on `[0/90]s` and on the unequal `[30/-45/0]s` | `|A66 - (A11 - A12)/2| / A11 > 1e-3` for `[0/90]s` (probe `0.18`); `|A16| / A11 > 1e-3` for the unequal layup (probe `3.0e-2`) |
| N3 | PL1 against the wrong formula with `D12 + D66` | `> 5e-2` away (probe `15.3` percent) |
| N4 | ST1 with `alpha2 = alpha1` (isotropic expansion) | `max |kappa| t / (alpha1 dT) <= 1e-8` (no mismatch, no curvature) |
| N5 | CP2 against the maximum-stress failure stress | the two differ by more than `10` percent at each angle (probe `20` to `30`) |
| N6 | RF1 to RF3 with a valid layup | accepted (`Ok`) |
| B0 | the bit record of D9 | the base digests, unchanged |
| B1 | `baseline_hashes` of `rcfd-api` | passes unchanged |

FC1 strain arithmetic: under `(sigma, 0, 0)` the strains are `(sigma/E1, -nu12 sigma/E1, 0)`; the transverse ratio
`nu12 sigma E2/(E1 Y_c) = 1.56e-3 sigma/MPa` is below the axial `sigma/X_t = 6.7e-3 sigma/MPa`, so the axial governs; under
`(0, sigma, 0)` the strains are `(-nu21 sigma/E2, sigma/E2, 0)`, the ratios `nu21 sigma E1/(E2 X_c) = 3.2e-3` against
`sigma/Y_t = 1.67e-2`, so the transverse governs.

## Planned mutations (on a scratch copy, one at a time)

| id | mutation | expected to be caught by |
|---|---|---|
| MU1 | the ply angle's sign reversed in `T_e` and `T_s` (rotation the wrong way) | CP1 (`tau12` sign) |
| MU2 | `B` with `-zbar` (z from the top) | ST1 (sign), N1 unaffected |
| MU3 | `D` without the `t_k^3/12` term | L4, PL1, BT5 |
| MU4 | the route of D3 removed (one isotropic ply through the general path) | BT1, BT2, BT3 |
| MU5 | the thermal load without its `M_th` part | ST1 |
| MU6 | `stresses` of a Layup element without the thermal resultants | ST2 |
| MU7 | Tsai-Wu with `F1 = 1/X_c - 1/X_t` | TW1 |
| MU8 | Tsai-Hill without its `-sigma1 sigma2/X^2` term | CP2 |
| MU9 | `check_plies` accepting a non-positive thickness | RF1, RF3 |
| MU10 | `Q22 = E2` (without `1/(1 - nu12 nu21)`) | L2 |
| MU11 | `m0` from ply 0 only | MS1 |
| MU12 | the mass coupling `m1` with the wrong sign | MS1 (the centre's offset) |
| MU13 | `thickness_or_error` returning the first ply's thickness for a Layup (a silent homogeneous fallback) | RX2 (gravity, buckling) |

## Decision rule

- **done**: every check of the table met, every negative control as expected, every planned mutation caught by at least one
  check, B0 and B1 unchanged, fmt and clippy clean.
- **partial**: B0 and B1 unchanged and BT1 to BT4, L1 to L3, RF1 to RF3 met, with any other miss diagnosed and recorded as a
  `*_known_miss` test.
- **todo**: otherwise (in particular any change of the default bits).

## Limits (not covered)

- Only Shell4 takes a layup; Shell3 refuses it (D7). The corotated Newton path, buckling and pre-stressed modes, spin
  softening and the rotating paths, gravity, the centrifugal load and the nodal body force refuse a Layup block naming the
  follow-up: the fan blade's centrifugal load on a layup is the first follow-up (proposed below).
- The transverse shear factor is `5/6` for every layup; the shear stiffness of a thick or strongly heterogeneous layup is
  approximate (no layup-dependent factor, no first-order shear deformation correction of the factor).
- No interlaminar stresses, no delamination, no progressive failure; failure indices are point-wise at the ply faces at the
  element centre.
- The temperature is uniform through the thickness (F-527 adds the gradient); the material properties are at the reference
  state (no `Tabulated` ply).
- An `Orthotropic` ply uses `E1, E2, nu12, G12, G13, G23, alpha1, alpha2`; its `axes` rule and its out-of-plane constants are not
  used; the ply angle is relative to the element's first axis `e_1` (along its first edge), so a curved mesh whose element
  axes turn carries the plies with them (no global fibre direction; a follow-up).
- The strengths are inputs (no material data in the repository); the maximum-strain allowables are the linear-elastic ones.
- No case-file key (b4): the layup is a library-level property.

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. **Bits of one isotropic ply: route or identical arithmetic.** Taken: the route of D3 (identical arithmetic is not possible
   without rewriting the homogeneous section). Recommendation: keep.
2. **The shear factor of a layup.** Taken: `5/6` for every layup. Recommendation: a follow-up for the energy-consistent factor
   if a thick layup is needed.
3. **`f12_star` of Tsai-Wu.** Taken: `-1/2` default, settable within `(-1, 1)`.
4. **The block material of a Layup block.** Taken: required by the model as for any block, not used by a Layup element.
5. **Tsai-Hill with signed strengths.** Taken: `X_t`/`X_c` and `Y_t`/`Y_c` by the sign of the stress.

Not taken (open, for the maintainer): **the case-file key** (b4; it changes the case schema and its golden). Proposal: in a
shell block of the case file, `layup = [{ material = "<name>", thickness = <m>, angle_deg = <deg> }, ...]` bottom first, as an
alternative to `thickness`, with `[[material]]` entries naming the ply materials, and `[[ply_strength]]` keyed by the material
name for the failure output. Recommendation: that form, with angles in degrees in the file and radians in the library.

## Amendment 1 (2026-11-14): the superseded test of `si_shell_thermal.rs` is misnamed in D10; written before its corrected run

D10 names `th4_th5_consistent_thermal_loads` as the test holding row N2 ("temperature on a Layup shell"). The row is in the
next test of the file, `b3_n1_n2_mixed_model_bits_and_refusals` (found when the copy was made, before any run of it). The
record and copy of D10 apply to that test: `#[ignore = "record: ..."]` on `b3_n1_n2_mixed_model_bits_and_refusals`, and the copy
`b3_n1_n2_mixed_model_bits_and_refusals_after_f428` with N2 replaced by N2b (as registered), every other row unchanged.
`th4_th5_consistent_thermal_loads` is not touched. No limit changes.
