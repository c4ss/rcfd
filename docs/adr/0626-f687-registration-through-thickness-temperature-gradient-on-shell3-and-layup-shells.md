# ADR 0626: F-687 registration: through-thickness temperature gradient on Shell3 and Layup shells

Status: **REGISTERED 2026-11-15, written before any code of F-687 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-687 was written, compiled or run in rcfd: the files listed under "What was read" were
read and the arithmetic below was done by hand from closed forms (no script, no finite-element code). Results go in the addendum
at the end; nothing above the addendum is changed after a run.

Parent: [ADR 0612](0612-f527-registration-through-thickness-temperature-gradient-on-shell4.md) (F-527:
`Load::ShellTemperatureGradient` on Shell4 of a `Shell` block, `shell::thermal_gradient_load_vector`, the recovery of the
thermal curvature in `shell::stresses`, the refusal of Shell3 and Layup elements with a non-zero gradient that this item
lifts; its "For the lead" entry gives the acceptance). Referred to: [ADR 0496](0496-f428-registration-laminated-shell-layup-ply-stresses-failure.md)
(classical lamination theory in `laminate.rs`, the layup thermal load and recovery of a uniform temperature),
[ADR 0454](0454-f419-registration-three-node-flat-shell.md) (Shell3: constant curvature, MITC3 tying, stabilised shear, centroid recovery).
Roadmap item F-687, deps F-527. Number 0626 given by the lead. Branch `f687-work` from `9cf95fc3` (main), worktree
`w49i-work/f687`, scratch `w49i-work/f687-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number); each registered run made once; defaults bit-identical; no
edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code;
clean-room (first principles: the plane-stress ply law integrated through the thickness, classical lamination theory, as in ADR
0496; the textbook result that an unsymmetric laminate under a through-thickness temperature change both stretches and bends,
e.g. Jones, Mechanics of Composite Materials, the hygrothermal section, is **cited from memory, not read**; it is derived below
and only the derivation is used). The roadmap files are not edited. Every choice the maintainer would normally be asked about is
taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation) and marked so.

**One existing test is affected by design** (deviation registered now, before any code): `n1_to_n8_refusals_and_zero_fields` in
`si_shell_thermal_gradient.rs` asserts in rows N1 and N2 the refusals of Shell3 and of a one-ply Layup that this item removes.
It cannot pass after F-687 without an edit. The only edit made to it is an `#[ignore = "superseded by ADR 0626: ..."]`
attribute (its body unchanged, kept as the record of ADR 0612); a running copy `n3_to_n8_after_adr_0626` in the new test file
keeps rows N3 to N8 with their limits unchanged (N1 and N2 become the positive checks of this ADR). Nothing else of an existing
test is edited.

## What F-687 asks

The acceptance, verbatim, binding:

> Every limit not stated here is registered in the ADR before the run. Load::ShellTemperatureGradient on a Shell3 model and on
> a Layup Shell4 model gives the curvature thermal load (ply-integrated for a layup); a clamped plate under a uniform gradient
> gives the closed-form moment within 1e-12; a model without a gradient gives the bits it gave before.

Clauses: **a1** every other limit registered here; **a2** on a Shell3 model the gradient gives the curvature thermal load;
**a3** on a Layup Shell4 model the ply-integrated thermal load; **a4** a clamped plate under a uniform gradient gives the
closed-form moment within 1e-12 (registered for Shell3, a symmetric and an unsymmetric layup); **a5** a model without a gradient
gives its former bits. From the brief: **b1** the membrane-bending coupling of an unsymmetric layup (a thermal force resultant)
derived from lamination theory and checked in closed form for a symmetric and an unsymmetric layup; **b2** the refusals for
beams, solids and the nonlinear paths kept; **b3** the ADR 0612 B1 digests re-run; **b4** no case-file key; **b5** mutations
registered before the run; **b6** free-plate limits registered with the `(h/t)^2` solve rounding of ADR 0612 in mind, or exact-
field checks.

## What was read (9cf95fc3, read only)

- ADR 0612 whole (design D1 to D5, arithmetic 1 to 9, amendment 1 and its diagnosis, addendum, remaining questions); ADR 0496
  only for the layup sums of `laminate()` and the layup thermal load.
- Code: `crates/rcfd-fem/src/thermal.rs` (`temperature_gradient_field`, `takes_gradient`, `refuse_gradient_unless_taken`,
  `add_thermal_gradient_load`, `GRADIENT_FOLLOW_UP`), `shell.rs` (`facet`, `point`, `thermal_gradient_load_vector`,
  `layup_thermal_load_vector`, `model_thermal_load_vector`, `thickness_or_error`, `section_of`, `model_section`, `stresses`,
  `layup_centre`, `centre_strains`, `layup_stresses`, `shell3_stresses`), `shell3.rs` (`facet`, `derivatives`, `bending_rows`,
  `tying_coefficients`, `shear_rows`, `stabilised_shear`, `local_stiffness_section`, `thermal_load_vector_with`,
  `element_stresses`, `to_global_vector`), `laminate.rs` (`ply_constants`, `rotated_stiffness`, `rotated_expansion`,
  `strain_rotation`, `Laminate`, `laminate`, `single_isotropic`, `section_stiffness`, `ply_stresses_at`, `layup_response`),
  `loads.rs` (the gradient arm), `model.rs` (`Material::orthotropic`), `mesh.rs` (`Ply::new`).
- Tests: `si_shell_thermal_gradient.rs` (helpers, every check), `si_shell_thermal_gradient_bits.rs` (B1 of ADR 0612),
  `sa_laminate.rs` (the material helpers and the ply constants).
- `docs/reference-data.md`: nothing on laminate thermal bending; the reference values here are closed forms.

## Arithmetic before this registration (disclosed)

1. **Lamination theory under a linear temperature (b1, a3).** Ply `k` (bottom first, `z_k <= z <= z_{k+1}`, `t_k`, `zbar_k`
   its mid-plane) with element-axes stiffness `Qbar_k` and expansion `abar_k` (engineering shear; ADR 0496 D1). A temperature
   `T(z) = T_m + z g` (`g = dT/dz` along `e_3`, K/m; `dT = T_m - T_ref`) gives the free strain `abar_k (dT + z g)`; with `eps(z)
   = eps0 + z kappa`, `sigma_k(z) = Qbar_k (eps0 + z kappa - abar_k (dT + z g))`. With `qa_k = Qbar_k abar_k`, `int_k dz = t_k`,
   `int_k z dz = (z_{k+1}^2 - z_k^2)/2 = t_k zbar_k`, `int_k z^2 dz = (z_{k+1}^3 - z_k^3)/3 = t_k zbar_k^2 + t_k^3/12`:
   `N = A eps0 + B kappa - N_th dT - M_th g`, `M = B eps0 + D kappa - M_th dT - P_th g`, where `N_th = sum qa_k t_k` and `M_th =
   sum qa_k t_k zbar_k` are the existing per-kelvin fields of `Laminate` and the new `P_th = sum qa_k (t_k zbar_k^2 + t_k^3/12)`
   (N m/K, per K/m of gradient). So the gradient's **force resultant is `M_th g`** (the same vector that couples a uniform
   `dT` into bending), zero for a symmetric layup (the `zbar_k` cancel in pairs) and non-zero for an unsymmetric one; its moment
   is `P_th g`. The energy `(1/2) int [N; M] . ([eps0; kappa] - thermal)` varied gives the load `f = int (B_m^T M_th + B_b^T
   P_th) g dA`. One isotropic ply: `M_th = 0`, `P_th = E alpha t^3 / (12 (1 - nu)) (1, 1, 0) = D (1 + nu) alpha (1, 1, 0)`,
   the ADR 0612 load.
2. **The layup load (a3).** At the 2 x 2 points of the stiffness, `g = sum_i N_i g_i`, `f_l += det (B_m^T M_th g + B_b^T P_th
   g)` (the bilinear membrane of a layup, as `layup_thermal_load_vector`), carried by `T^T`. On a rectangle `a x b` in the `xy`
   plane, first edge along `x`, `g = g_c + c_1 X + c_2 Y` about the centre, ADR 0612 arithmetic 5 gives `I_X,i = int N_i,X g dA =
   a_i (b/8)(4 g_c + (2/3) c_2 b c_i)`, `I_Y,i = int N_i,Y g dA = c_i (a/8)(4 g_c + (2/3) c_1 a a_i)`; with `n = M_th`, `m = P_th`
   and the rows (`eps_xx = u_,X`, `eps_yy = v_,Y`, `gamma = u_,Y + v_,X`; `kappa_xx = theta_y,X`, `kappa_yy = -theta_x,Y`,
   `kappa_xy = theta_y,Y - theta_x,X`): `f(u_x)_i = n_0 I_X + n_2 I_Y`, `f(u_y)_i = n_1 I_Y + n_2 I_X`, `f(theta_y)_i = m_0 I_X +
   m_2 I_Y`, `f(theta_x)_i = -(m_1 I_Y + m_2 I_X)`, `f(u_z) = f(theta_z) = 0` exactly. Exact under 2 x 2 Gauss (degree 2 per
   variable); rounding `~1e-15`.
3. **Shell3 (a2).** The curvature rows are constant (`derivatives`), so `int B_b^T D kappa_th dA = A B_b^T D alpha g_m (1, 1, 0)`
   with `g_m = (g_0 + g_1 + g_2)/3`, exact for the linear `g` (`int g dA = A g_m`), the same mean as the Shell3 membrane thermal
   load. On a right triangle `(0, 0), (a, 0), (0, b)` (local axes = global, `e_1` along the first edge): `N_X = (-1/a, 1/a, 0)`,
   `N_Y = (-1/b, 0, 1/b)`, `m_s = D (1 + nu) alpha g_m`: `f(theta_y)_i = A N_X,i m_s`, `f(theta_x)_i = -A N_Y,i m_s` (the `kappa_xy`
   entry of `D kappa_th` is zero), every other entry exactly zero. Recovery at the centroid: `kappa - alpha g_m (1, 1, 0)`.
4. **Free plates are discrete solutions (b6).** Under a uniform `g` a free laminate takes `[eps0; kappa]` solving `[A B; B D]
   [eps0; kappa] = g [M_th; P_th]`, with `N = M = 0`. The field `u = eps0`-linear in plane (`u = e_1 X + e_3 Y/2`, `v = e_2 Y +
   e_3 X/2`), `beta = (k_1 X + k_3 Y/2, k_2 Y + k_3 X/2)`, `w = -(k_1 X^2 + k_2 Y^2 + k_3 X Y)/2` has `gamma = grad w + beta = 0`,
   linear `u` and `beta` reproduced exactly by the bilinear (or linear) interpolation, and zero assumed shear: on MITC4 by ADR
   0612 arithmetic 3 (secant slope of a quadratic = tangent at the mid-point); on MITC3 the tying value `(w_j - w_i)/l + (beta_i +
   beta_j)/2 . t` is `(grad w + beta)(m) . t = 0` on each edge for the same reason, so `a_1 = a_2 = b = 0` and the stabilisation
   scales a zero; the drilling term is zero (`theta_z` and the membrane rotation `(v_X - u_Y)/2 = 0` vanish). For orthotropic
   plies the element axes must agree between elements (ply angles are measured from each element's `e_1`, along its first
   edge): a regular grid. For isotropic plies the resultants are isotropic (`(c, c, 0)`) and invariant under the in-plane rotation
   between facets, so any flat mesh serves.
5. **Clamped plates (a4).** Every boundary node clamped (6 components). When every element has the same resultants in common
   axes (a regular grid, or isotropic resultants on any flat mesh), the interior load entries are patch integrals of `N_i,X`,
   `N_i,Y` times constants, which vanish (boundary integral of `N_i`, zero on the patch boundary): the discrete solution is `u =
   0`, and the recovery gives `N = -M_th g`, `M = -P_th g` at every centre. Faces (element axes): `top = -Qbar_top abar_top
   (t/2) g`, `bottom = +Qbar_0 abar_0 (t/2) g`. Ply stresses in material axes (`layup_response`): the mechanical strain `T_e(theta_k)
   (- abar_k z g) = -(alpha_1, alpha_2, 0)_k z g` (since `abar_k = T_e(-theta_k)(alpha_1, alpha_2, 0)`), the stress `-Q_k (alpha_1,
   alpha_2, 0) z g` at each ply's bottom and top `z`. Shell3: `M = -D (1 + nu) alpha g (1, 1, 0)`, `top = -s_s (1, 1, 0)`, `bottom
   = +s_s (1, 1, 0)`, `s_s = E alpha g t / (2 (1 - nu))`. Rounding: `u ~ eps |f| / k`, the recovered resultants `~1e-15` relative
   (ADR 0612 G2 measured `2.5e-16`).
6. **Rounding of the free-plate solves (b6).** ADR 0612 measured on the TH plate (`h ~ 0.33` to `0.5`, `t = 0.1`, isotropic
   steel) a curvature error `~3e-12` and moment `~2e-12` of scale, the forward error of the bending-shear solve growing as the
   shear-to-bending ratio `3.5 (h/t)^2` (factor `~150 eps` times the ratio). Shell3 (the TH plate cut into 24 triangles, `t =
   0.1`): its shear is scaled by `s = t^2 / (t^2 + 0.1 h^2)`, the longest edge `h ~ 0.6`: `s ~ 0.22`, ratio `3.5 (6)^2 0.22 ~ 27`
   (against `~90` for Shell4 at `h ~ 0.33`... `0.5`): estimate `~150 eps 27 ~ 9e-13`, registered with a factor of ten: `1e-11`.
   The bimetal (steel / aluminium, isotropic plies, total `t = 0.1`): the shear-to-bending ratio within a factor 1.5 of steel's;
   estimate `~3e-12` (ADR 0612's measured), registered `1e-11`. The cross-ply (E1 / E2 = 14, G13 = E2 / 2): ratio `(5/6) G13 t h^2
   12 / (D)` with `D ~ 7.5e10 t^3 / 12` is about `0.7 (h/t)^2` (smaller than steel's), but the stiffness contrast between the
   directions and the plies enters the condition (up to `E1 / E2 = 14`): estimate `~8e-12`, registered `1e-10`. The exact-field
   checks (the discrete solution of arithmetic 4 given to the recovery) carry no solve: `~1e-15`, registered `1e-13`.
7. **Bits (a5).** A model without a gradient load never enters the new code: the new arms run only with
   `temperature_gradient_field(model) = Some`, and every changed function keeps its former operations on the `None` path (the
   Shell3 recovery subtracts nothing, the layup recovery and faces compute `dT` as before, `ply_stresses_at` is unchanged and a new
   function takes the gradient). The new `Laminate` field is a new sum that does not reorder the existing ones. Checked by the
   ADR 0612 B1 digests (unchanged file) and by new digests of a multi-ply layup and a Shell3 ANDES plate printed on the base
   before any library code (B1b).

## Design

### D1. The laminate (`laminate.rs`)

New field `Laminate::p_th: [f64; 3]`, `sum qa_k (t_k zbar_k^2 + t_k^3/12)` (arithmetic 1; the `i2` already computed in the loop),
the moment per unit through-thickness gradient. The gradient's force resultant is `m_th` (no new field). New `ply_stresses_with_gradient(plies, eps0,
kappa, dt, g)`: `ply_stresses_at` with the free strain `abar (dt + z g)`; `ply_stresses_at` itself unchanged. `LayupResponse` gets
the field `gradient` (the centre gradient, 0 without a gradient load), and `layup_response` calls the new function only when the
model has a gradient load.

### D2. The loads (`shell.rs`, `shell3.rs`, `thermal.rs`)

- `shell::layup_thermal_gradient_load_vector(coords, laminate, gradient_nodal)`: arithmetic 2 (bilinear membrane), refusing other
  than four values (`FieldLength { key: "gradient" }`).
- `shell::model_thermal_gradient_load_vector(mesh, e, coords, material, g)`: a `Shell` block `thermal_gradient_load_vector` of its
  thickness (the ADR 0612 call, unchanged); a layup of one isotropic ply the same with the ply's material and thickness (as
  `model_thermal_load_vector`, ADR 0496 D3: the bits of the `Shell` twin); any other layup the new function.
- `shell3::thermal_gradient_load_vector(coords, thickness, material, g)`: arithmetic 3, refusing other than three values and a
  material without a finite expansion (as Shell3's membrane thermal load); independent of the membrane choice (only `B_b`).
- `thermal::takes_gradient`: a Shell4 element (any block) or a Shell3 element; `add_thermal_gradient_load` dispatches Shell3 to
  the Shell3 function (18 entries, 3 nodes) and Shell4 to `model_thermal_gradient_load_vector`. Beams and solids with a non-zero
  value are refused as before (`NotYetSupported`, the variant the element kind); `GRADIENT_FOLLOW_UP` now names beams and solids.

### D3. The recovery (`shell.rs`, `shell3.rs`)

- Shell3: `shell3_stresses` passes `Some(alpha g_m)` with a gradient load, and `element_stresses` subtracts it from `kappa_xx`,
  `kappa_yy` before `D` (nothing on `None`).
- Layup (not one isotropic ply): `layup_centre` also returns the centre gradient `g_c = sum_i N_i(0, 0) g_i` (with a gradient load);
  `layup_stresses` subtracts `m_th g_c` from `N` and `p_th g_c` from `M`, and the faces use `abar (dT + z g_c)`. One isotropic
  ply keeps the homogeneous branch of `stresses`, which already subtracts the curvature (ADR 0612 D3).
- `centre_strains` (used by `layup_response`) carries `g_c`.

### D4. Unchanged

The refusals of the nonlinear static and transient paths and of fracture (ADR 0612 D4); the harmonic cyclic path's refusal of own
loads; Hex/Tet/beam elements with a non-zero value. No case-file key (b4; open question Q1 of ADR 0612 stays open).

## Checks (limits fixed now)

Materials: steel `E = 2e11`, `nu = 0.3`, `alpha = 1.2e-5`, `rho = 7850`; aluminium `E = 7e10`, `nu = 0.33`, `alpha = 2.3e-5`,
`rho = 2700`; the ply material `CF`: orthotropic `E = (1.4e11, 1e10, 1e10)`, `nu = (0.3, 0.3, 0.45)`, `G = (5e9, 5e9, 3.5e9)`,
`rho = 1600`, `alpha = (-0.5e-6, 3e-5, 3e-5)`, fixed axes. Gradient `g = 200` K/m unless stated. Layups (bottom first):
**SYM** CF `[0/90/0]`, `0.03 / 0.04 / 0.03`; **UNS** CF `[0/90]`, `0.05 / 0.05`; **BIM** steel `0.05` / aluminium `0.05`; **ANG**
CF `[30/-45/60]`, `0.02 / 0.03 / 0.025`. Meshes: **TH** the ADR 0612 plate (4 x 3 distorted on `[0, 2] x [0, 1]`), placed **P**
(identity) or **Q** (rotated about `(1, 2, 3)` by 0.7, moved by `(0.3, -0.2, 0.5)`); **RG** the regular 4 x 3 grid on `[0, 2] x [0,
1]`, placed Q; **TH3** TH with each quadrilateral cut on its 0-2 diagonal (24 Shell3). The reference resultants are computed **in
the test** from the ply constants by the `z^2`, `z^3` difference forms of arithmetic 1 (not the library's `t zbar^2 + t^3/12`),
`Q` of a CF ply by hand (`nu21 = nu12 E2 / E1`), `Qbar abar` of 0 and 90 degree plies by hand (90: the swap of indices 0 and 1);
ANG only takes `rotated_stiffness` and `rotated_expansion` of `laminate.rs` (ADR 0496 checked). Scales: `M_s = max_i |P_th,i| g`,
`N_s = g (t/2) sum_k t_k max_i |qa_k,i|` (the force of a fully unsymmetric layup), the face scale `S_s = max_k max_i |qa_k,i|
(t/2) g`.

| id | clause | check | limit | estimate |
| --- | --- | --- | --- | --- |
| L1 | a2 | one Shell3 right triangle `(0,0), (0.5,0), (0,0.3)`, `t = 0.02`, steel, `g_i = 100 + 300 X_i - 150 Y_i`: `nodal_load_vector` against arithmetic 3, relative to the largest entry; forces and `theta_z` exactly zero | `1e-12`; exact | arithmetic 3: `~1e-15` |
| L2 | a3, b1 | one rectangle `0.6 x 0.4`, `g_i = 100 + 300 X_i - 150 Y_i`, layups UNS, SYM and ANG: `nodal_load_vector` against arithmetic 2 relative to the largest entry; `u_z`, `theta_z` exactly zero; for SYM the force entries relative to `N_s 0.24` are at most rounding | `1e-12` each; exact; `1e-12` | arithmetic 2 |
| L3 | a3 (one ply) | plate Q (TH), `t = 0.1`, a Layup of one steel ply against the `Shell` block: load vector, `solve_static` displacement (node 0 clamped) and every field of `shell::stresses` bitwise equal | exact | D2 (the same calls) |
| C1 | a4 | TH3 placed P and Q, steel, `t = 0.1`, boundary clamped, uniform `g`: `shell::stresses` centroid moments against `(-M_s, -M_s, 0)` (`M_s = D (1 + nu) alpha g`), faces against `-+s_s (1, 1, 0)`, membrane relative to `s_s t` | `1e-12` each, per placement | arithmetic 5: `~1e-15` |
| C2 | a4, b1 | RG placed Q, boundary clamped, uniform `g`, SYM and UNS: centre `M` against `-P_th g` relative to `M_s`; `N` against `-M_th g` relative to `N_s` (SYM: zero; UNS: non-zero); faces `top`, `bottom` against arithmetic 5 relative to `S_s`; `layup_response` ply stresses (every ply, bottom and top) against `-Q (alpha_1, alpha_2, 0) z g` relative to `S_s` | `1e-12` each, per layup | arithmetic 5: `~1e-15` |
| C3 | a4, b1 | TH placed Q, BIM, boundary clamped: as C2 (`M`, `N`, faces, ply stresses) | `1e-12` each | arithmetic 5 (isotropic resultants: any flat mesh) |
| F1 | (stronger, a2, b6) | TH3 P and Q, `t = 0.1`, node 0 clamped, uniform `g`: `shell::stresses` `max |M| / M_s`, `max |N| / (s_s t)` | `1e-11` each | arithmetic 6: `~9e-13` |
| F2 | (stronger, a3, b6) | TH P and Q, BIM, node 0 clamped: centre curvature (test side, as ADR 0612 G1a) against `k (1, 1, 0)` relative to `k` and membrane strain against `e (1, 1, 0)` relative to `|e|`, `(e, k)` the 2 x 2 closed form of arithmetic 4 (test side); `shell::stresses` `max |M| / M_s`, `max |N| / N_s` | `1e-11` each | arithmetic 6: `~3e-12` |
| F3 | (stronger, a3, b6) | RG Q, UNS, node 0 clamped: `layup_response` centre strain and curvature against the 6 x 6 closed form of arithmetic 4 (test side elimination) relative to `max |eps0|`, `max |kappa|`; `shell::stresses` `max |M| / M_s`, `max |N| / N_s` | `1e-10` each | arithmetic 6: `~8e-12` |
| X1 | (stronger, b6) | the exact discrete fields of arithmetic 4 (no solve) given to `shell::stresses`: TH3 P and Q (Shell3), TH Q with BIM, RG Q with UNS: `max |M| / M_s`, `max |N| / N_s` (Shell3: `s_s t`) | `1e-13` each | arithmetic 4: `~1e-15` |
| B1 | a5, b3 | ADR 0612's `si_shell_thermal_gradient_bits.rs`, unchanged, its seven digests | exact | arithmetic 7 |
| B1b | a5 | new `si_shell_thermal_gradient_layup_bits.rs`, digests printed on a copy of 9cf95fc3 before any library code of F-687 (`f687-scratch/b1b-base.log`), then fixed in the file: (1) RG Q, UNS, temperature `300 + 40 X - 25 Y` against 290 and gravity, node 0 clamped: load vector, displacement, every field of `shell::stresses`, every number of `layup_response`; (2) TH Q with BIM, temperature only: load vector and `shell::stresses` of a fixed displacement; (3) TH3 Q with `Shell3Membrane::AndesOpt`, temperature: load vector and `shell::stresses` of a fixed displacement | exact | arithmetic 7 |
| B2 | a5 | `baseline_hashes` (rcfd-api, release) and the rcfd-fem test files covering changed lines: `si_shell_thermal_gradient` (with the one ignore of the preamble), `si_shell_thermal_gradient_bits`, `si_shell_thermal`, `si_shell_thermal_bits`, `si_shell`, `sa_shell3`, `sa_shell4_membrane`, `sa_laminate`, `sa_laminate_body`, `sa_rotating_cyclic_shells`, `sb_loads`, `sb_thermal` | pass | arithmetic 7 |

### Negative controls

| id | model | expected | limit |
| --- | --- | --- | --- |
| N1' | (the copy of ADR 0612 N3 to N8, rows unchanged, test `n3_to_n8_after_adr_0626`) | as ADR 0612 | as ADR 0612 |
| N9 | a mixed model: the RG plate of Shell4 with a Beam2 stiffener on its first row of nodes, a non-zero gradient at those nodes | `Element { source: NotYetSupported { item: "shell temperature gradient", variant: "Beam2", package: GRADIENT_FOLLOW_UP } }` at the load and at `shell::stresses` | exact |
| N10 | the same with the gradient zero on the stiffener's nodes (non-zero elsewhere) | load taken; the beam rows of the load vector exactly zero | exact |
| N11 | (sensitivity of C2) UNS clamped: `N` computed without the coupling term, i.e. `max |N| / N_s` of the recovered field against zero | above 0.1 (the coupling is a sizeable part of `N_s`) | `> 0.1` |
| N12 | `nonlinear::solve` of TH P with BIM and a gradient; of TH3 P with a gradient | refused, `InvalidValue { key: "nonlinear" }` naming the gradient (ADR 0612 D4, unchanged) | exact |

### Planned mutations (scratch copy `f687-scratch/tree` from `git archive` of the final commit; not committed)

| id | mutation | caught by (expected) |
| --- | --- | --- |
| MU1 | layup load: `p_th` replaced by `m_th` in the bending part | L2, F2, F3 |
| MU2 | layup load: the membrane part `B_m^T m_th g` dropped | L2 (UNS, ANG), F2, F3 |
| MU3 | layup recovery: `- m_th g_c` dropped from `N` | C2 UNS, C3 |
| MU4 | layup recovery: `- p_th g_c` dropped from `M` | C2, C3 |
| MU5 | layup faces: the `z g_c` term dropped | C2 faces, C3 faces |
| MU6 | `layup_response`: the gradient dropped from the ply stresses | C2 ply stresses, C3 ply stresses |
| MU7 | `p_th` with `t_k zbar_k^2` only (the `t_k^3/12` dropped) | L2, C2, C3 |
| MU8 | Shell3 load: the sign of `kappa_th` flipped | L1, F1 |
| MU9 | Shell3 load: `1 + nu` missing (`m = (D_00 k, D_11 k, 0)`) | L1, F1 |
| MU10 | Shell3 load: the gradient of the first node in place of the mean | L1 |
| MU11 | Shell3 recovery: the thermal curvature not subtracted | C1, X1 |
| MU12 | the one-isotropic-ply route removed (the general laminate path for one ply) | L3 |
| MU13 | `takes_gradient` true for every element (the beam refusal gone) | N9 |

## Decision rule

- **done**: every check and negative control met, every planned mutation caught, B2 passing.
- **partial**: a registered check missed (an ignored `*_known_miss`), a mutation not caught, or a check met only after an
  amendment written after its run.
- **todo**: the code does not compile or the gradient path is not reached.

## Limits (not covered)

- L1. Recovery only at element centres (`shell::stresses`, `layup_response`); the faces and the ply stresses of a layup are those
  at the centre.
- L2. Shell3 takes the gradient's mean over the element (exact for its constant curvature and a linear field).
- L3. Beams and solids still refused with a non-zero value; nonlinear static, nonlinear transient and fracture still refuse the
  load; a Shell3 Layup does not exist (refused by the model check, ADR 0496 D7).
- L4. Through-thickness gradient only linear in `z`; a non-linear profile through a laminate (per-ply temperatures) is not
  modelled.
- L5. The gradient along each element's own `e_3` (ADR 0612 L4); for a layup the ply order follows the same `e_3`.
- L6. No case-file key (ADR 0612 Q1 stays open).

## Open points taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. The unsymmetric layup's gradient force `M_th g` is part of the load and of the recovery (the lamination theory result), not
   left out: leaving it out would be wrong for every unsymmetric layup.
2. Shell3 uses the element mean of the gradient, as its membrane thermal load does with the temperature.
3. `LayupResponse` gets a new field (`gradient`) rather than a new function: the struct is `#[non_exhaustive]`.
4. The existing test `n1_to_n8_refusals_and_zero_fields` gets an `#[ignore]` naming this ADR, with a running copy of N3 to N8
   (preamble), rather than edits to its rows.

Open (not taken): **Q1** a case-file key for the gradient (needs a schema decision; ADR 0612 remaining question 2).
