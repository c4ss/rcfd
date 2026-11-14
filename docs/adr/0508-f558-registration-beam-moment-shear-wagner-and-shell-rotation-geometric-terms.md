# ADR 0508: F-558 registration: moment, shear and Wagner terms of the beam geometric stiffness and the t^2/12 rotation terms of the shells (lateral-torsional buckling under uniform moment)

Status: **REGISTERED 2026-11-14, written before any code of F-558 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text one probe was run on the unchanged library (disclosed in "Probes before registering") and the
arithmetic below was done by hand and evaluated with a plain python script (no rcfd code). The bit record B0 of the default
paths is a test file written and run on the base after this commit and before any library code (D8). Results go in the addenda
at the end; nothing above the addenda is changed after a run.

Roadmap item F-558 (deps F-420), the follow-up named by [ADR 0473](0473-f420-registration-stress-stiffening-of-beams-and-shells.md)
(its Limits and its question Q2). Number 0508 given by the lead. Branch `f558-work`, worktree `w49i-work/f558`, scratch
`w49i-work/f558-scratch`. Base 031b09c6.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no
unsafe code; no case-file key (a proposal is under the open questions); clean-room.

## What F-558 asks

The acceptance, binding (verbatim):

> Every limit not stated here is registered in the ADR before the run. The beam's geometric stiffness gains the bending-moment,
> shear-force and Wagner terms and the shells' the t^2/12 rotation terms, opt-in: the lateral-torsional buckling moment of a
> simply supported I or rectangular beam under uniform moment equals the closed form (pi/L) sqrt(E I_z G J) (1 + (pi^2 E
> C_w)/(G J L^2))^(1/2) within 1 percent at 16 elements; without the option every result of ADR 0473 bit-identical.

Clauses: (a0) every limit registered here; (a1) the beam gains the bending-moment, shear-force and Wagner terms, opt-in; (a2)
the shells gain the `t^2/12` rotation terms, opt-in; (a3) LTB moment of a simply supported rectangular beam, 16 elements,
within `1e-2`; (a4) the same for an I beam; (a5) without the option every result of ADR 0473 bit-identical.

Readings, registered:

- **(a4), the I section, is not testable with this element.** `BeamSection` has the area, two second moments, the torsion
  constant and two shear factors, and the element has 6 DOF per node: no warping DOF, no warping constant `C_w`. For an I
  section the closed form has `C_w = I_z h_c^2/4 != 0` (arithmetic 2: a factor 1.241 on the section of check I1). The FE
  moment without warping is the `C_w = 0` value, so the I-section clause is missed by definition, not by a defect of the new
  terms. It is registered as an ignored record `ltb_i_section_known_miss` (run once, number recorded, limit 1e-2 intact) and
  the item can be at most **partial** (decision rule). Adding a warping DOF is a follow-up (open questions Q1).
- **(a3), the rectangle**, has `C_w = b^3 h^3/144` (thin rectangle, from memory of the standard result, unverified; its effect is
  registered as 0.12 percent, arithmetic 1). The closed form of the check is the acceptance's formula with this `C_w`.
- **"Simply supported"**: fork supports, `v = w = theta_x = 0` at both ends, `u = 0` at one end, the bending rotations
  free; torsion held at the ends (warping free: not modelled).
- **"Uniform moment"**: equal and opposite end moments about the strong axis (`theta_y` DOF), `M_ref` each, by nodal moments;
  the pre-stress is the static solve of that load.
- **"Opt-in"**: a method on `Prestress` (D6) that carries the element end forces and the flags; nothing else changes; no
  case-file key.

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0473 (D1 the beam, D2 the shells, D4 the pre-stress, D6 the paths, its Limits and Q2), ADR 0502
  (the layup extension of `geometric_struct.rs`).
- `crates/rcfd-fem/src`: `beam.rs` (shape functions `plane_shape_coefficients`, `plane_functions`, `plane_slopes`, `field_rows`,
  `local_geometric_stiffness`, `local_stiffness`, `to_global`, `axial_force`, `section_forces`), `geometric_struct.rs` (whole),
  `geometric.rs` (`Prestress`, `assemble_geometric`), `shell.rs` and `shell3.rs` (`local_geometric_stiffness`,
  `element_geometric_stiffness`), `buckling.rs` (`factors_with`, the two uses of `assemble_geometric`).
- Tests: `sa_geometric_struct.rs` (helpers, the plate set-up of P1), `sa_geometric_struct_bits.rs` (the digest pattern).

## D0. What exists

The beam's geometric stiffness is `N` times the integral of the slopes of the deflection functions of each bending plane, plus
the axial term and Wagner's `N (I_y + I_z)/A int phi'^2`; moments, shears and the section's cubic Wagner terms are not read
(ADR 0473 D1). The shells' is the membrane resultant on the three translations; the rotations take no term (ADR 0473 D2).

## Design

### D1. The beam: second-order strain measures of a rotated section (first principles; cited from memory, derived here)

Local axes `e_1` (axis), `e_2`, `e_3`; translations `u = (u, v, w)`, section rotations `theta = (phi, theta_y, theta_z)`,
`phi` the twist. For a rotation vector `theta` the section frame is `R = exp(Theta)`, `Theta = skew(theta)`. The geometrically
exact beam strains in the section frame (Reissner; Simo; cited from memory, unverified, re-derived) are `Gamma = R^T (e_1 + u')
- e_1` (force strains) and the material curvature `skew(K) = R^T R'`. Expanding `R = I + Theta + Theta^2/2`:

- `Gamma = (u' - theta x e_1) + Gamma_2`, `Gamma_2 = -theta x u' + (1/2) theta x (theta x e_1)`;
- `K = theta' + K_2`, `K_2 = -(1/2) theta x theta'`.

The second-order energy of the resultants `F = (N, V_y, V_z)` and `M = (T, M_y, M_z)` (section frame, tension and the right-hand
moment positive, taken at the end of the element and varied linearly, D3) is `int (F . Gamma_2 + M . K_2) dx`, so, writing out
(`theta x e_1 = (0, theta_z, -theta_y)`, `theta x (theta x e_1) = (-(theta_y^2 + theta_z^2), phi theta_y, phi theta_z)`):

- `F . Gamma_2 = N [-(1/2)(theta_y^2 + theta_z^2) - theta_y w' + theta_z v'] + V_y [(1/2) phi theta_y - theta_z u' + phi w'] +
  V_z [(1/2) phi theta_z - phi v' + theta_y u']`;
- `M . K_2 = -(1/2) { T (theta_y theta_z' - theta_z theta_y') + M_y (theta_z phi' - phi theta_z') + M_z (phi theta_y' - theta_y
  phi') }`.

With the Euler constraint (`theta_y = -w'`, `theta_z = v'`) the `N` term is `(1/2) N (v'^2 + w'^2)`, the classical one (the check
that fixes the derivation: it is the term the element already has). The existing `N` terms of ADR 0473 D1 are kept as they are
(deflection-function slopes, `N I_p/A phi'^2`); the new terms are the `V` terms and the `M` terms above, built on the element's
own fields: `u'` linear, the lateral slopes `v', w'` of the deflection functions of each plane (`plane_slopes`), the section
rotations `theta_y, theta_z` and their derivatives from the rotation functions `psi(xi) = (b_1 + 2 b_2 xi + b_3 (3 xi^2 + phi/2))/L`
(`plane_functions`), `psi' = (2 b_2 + 6 b_3 xi)/L^2`, and `phi` linear. The quadratic form is integrated by four-point Gauss (the
integrand has degree 4 in `xi`).

**Moment terms** = `M . K_2` (bending moments `M_y`, `M_z`, and the torsion moment `T`). **Shear-force terms** = the `V_y`, `V_z`
part of `F . Gamma_2`. **Wagner terms** (the cubic fibre terms): the fibre at `(y, z)` of a section twisting by `phi` moves
laterally by `(-z phi, y phi)`, so the second-order fibre strain is `(1/2)(y^2 + z^2) phi'^2`; with the section stress `sigma_x = N/A
+ M_y z/I_y - M_z y/I_z` (the signs of `u_x = z theta_y - y theta_z`) its energy is `(1/2) phi'^2 [N I_p/A + M_y beta_y - M_z
beta_z]`, `beta_y = int z (y^2 + z^2) dA/I_y`, `beta_z = int y (y^2 + z^2) dA/I_z`. The `N` part is the existing term; the new
Wagner terms are `(1/2)(beta_y M_y - beta_z M_z) int phi'^2 dx` with `beta` inputs, both zero for a section with two axes of
symmetry (the rectangle and the I of the checks) and nonzero for a section with one (a channel, a T): they are on the interface
and checked on the element (check H2), not on a beam.

**Lateral-torsional buckling follows**: for `M_y = M` uniform and the Euler constraint, `M . K_2 = -M phi v''` up to boundary
terms (`int theta_z phi' = [theta_z phi] - int theta_z' phi`), the classical coupling; with `E I_z v'''' ...` the energy `(1/2)[E
I_z a^2 k^4 + G J b^2 k^2] - M k^2 a b` of `v = a sin kx`, `phi = b sin kx`, `k = pi/L`, has the critical `M^2 = E I_z G J k^2`
(arithmetic 1), the closed form with `C_w = 0`; the warping term adds `E C_w b^2 k^4` and gives the `(1 + pi^2 E C_w/(G J L^2))`
factor. With an axial thrust `P` the same determinant gives `(E I_z k^2 - P)(G J - P r_0^2) = M^2` (`r_0^2 = (I_y + I_z)/A`), i.e.
`M^2 = M_0^2 (1 - P/P_z)(1 - P/P_phi)`, `P_z = pi^2 E I_z/L^2`, `P_phi = G J/r_0^2` (the Timoshenko and Gere interaction, cited
from memory, re-derived here; check C1).

What the element does not model, so the closed form differs by it (registered, arithmetic 1): the pre-buckling curvature enters
the exact (nonlinear) theory through the factor `(1 - I_z/I_y)^(-1/2)`, absent from a stress-resultant geometric stiffness that
is linear in the pre-stress; `C_w`; the prebuckling deflection.

### D2. The shells: the t^2/12 rotation terms (first principles)

The director kinematics of the shell elements are `u(z) = u + z theta x e_3` (ADR 0473 D3). The in-plane displacement of the
director is `z d`, `d = theta x e_3 = (theta_y, -theta_x, 0)` in the facet axes. The pre-stress is uniform over the thickness; the
second-order strain `(1/2) u_{k,alpha} u_{k,beta}` integrated against `sigma_{alpha beta}` over `z in [-t/2, t/2]` gives, by
`int z dz = 0` and `int z^2 dz = t^3/12`,

`(1/2) N_{alpha beta} [ u_{k,alpha} u_{k,beta} + (t^2/12) (theta_{x,alpha} theta_{x,beta} + theta_{y,alpha} theta_{y,beta}) ]`,
`N = t sigma` the membrane resultant (the existing term is the first). The new term is the geometric stiffness `K[(a, c), (b,
c)] = (t^2/12) int grad N_a . N grad N_b dA` for `c = theta_x, theta_y` (the drilling rotation takes none), the same integral as
the translations with the same 2 x 2 Gauss (Shell4) or the exact constant gradient (Shell3). A Layup block takes the same term
with its total thickness (the through-thickness uniform pre-stress `N/t` of ADR 0502 D2; for a laminate with stiff outer plies the
true `int z^2 sigma dz` differs: Limits).

Thick-plate interpretation (arithmetic 3): for a simply supported plate with the Reissner-Mindlin kinematics the term lowers
the buckling load by the factor `n_1/n_0`; at `t/a = 0.2` it is 0.9577 (4.2 percent). The relative motion of the load with the
option is the check of the term; the absolute load of a coarse thick-shell model also carries the shell element's own
discretisation error (`+-2 percent` in the probe), which is why S3 compares the ratio (the same model with and without the term).

### D3. The element end forces

The opt-in carries, per Beam2 element, the twelve local end forces `f = K_loc u_loc` of the static displacement (the element's own
stiffness, `local_stiffness`, with the displacement transformed to local axes), computed by `Prestress::with_beam_resultants(model,
displacement, wagner)`. The section resultants at `x = 0` are `-f(node 0)` and at `x = L` are `+f(node 1)` (the force of the rest
of the structure on the cut face; the element has no load between its nodes, where `M` is linear and `V` constant). Within the
element `F` and `M` vary linearly from their end values; `N` of the new terms is not used (the existing term keeps the
element-average `N` of the stress). A distributed load inside the element (gravity, centrifugal load) is represented by its
end values only: the bending moment of such an element is quadratic; the linear interpolation is registered as a limitation.

### D4. API

- `beam::local_resultant_geometric_stiffness(length, section, material, end_forces, wagner) -> ElementMatrix`: the sum of the
  moment, shear and Wagner terms (three public functions `local_moment_geometric_stiffness`, `local_shear_geometric_stiffness`,
  `local_wagner_geometric_stiffness`), local axes; `element_resultant_geometric_stiffness` (global).
- `beam::local_end_forces(coords, section, orientation, material, ue) -> [f64; 12]`.
- `shell::local_rotation_geometric_stiffness(f, thickness, n)` and `shell3::...`, added to the base matrix by
  `geometric_struct` when the option is on.
- `Prestress::with_beam_resultants`, `Prestress::with_shell_rotation_terms`; `scaled` scales the end forces, `masked` zeroes
  those of the masked elements; the flags and forces are `None`/`false` for every existing constructor. `assemble_geometric`
  takes the old path (the old function, the same bytes) when neither is set.

### D5. Where the new matrices act

Only `geometric::assemble_geometric` (hence the buckling factors, the pre-stressed modes and the rotating part) calls
`geometric_struct`; the new variant of `element_geometric_stiffness` is called only when the prestress has an option on. No
nonlinear driver is touched (the files of F-587 and F-427 are not edited).

### D6. The default-bits record

`tests/sa_geometric_struct_f558_bits.rs` is written and run on the base (031b09c6 plus this ADR, no library line changed): the
digests (FNV-1a 64 of ADR 0291) of the element geometric stiffness of a Beam2 in three orientations, a Shell4 and a Shell3 (global
matrices), of the assembled geometric stiffness of a beam model, a Shell4 and a Shell3 plate under a static pre-stress, of the
buckling factors of the beam and of both plates, and of the pre-stressed modes of the beam. It runs unchanged after the change and
compares with the constants (check B0), and again after the change with the option set to nothing (`Prestress` untouched).

## Probes before registering

On the unchanged library (a throwaway test, `tests/zz_probe_f558.rs`, deleted before the first commit): the lowest buckling
load of a simply supported square plate `a = 1`, `nu = 0.3`, uniaxial `N`, Shell4 and Shell3 with the set-up of ADR 0473 P1
(`w = 0` on the edges) but with the tangential rotation held on each edge (hard simple support, `theta_x = 0` on `x = 0, a` and
`theta_y = 0` on `y = 0, a`; without it the free rotation gives a softer plate than the Navier solution, 7 percent below at
`t/a = 0.1`), `N_cr/E`:

| t | cells | Shell4 | Shell3 (tri) |
|---|---|---|---|
| 0.01 | 16 | 3.6327e-6 | 3.6408e-6 |
| 0.1 | 8 / 16 / 24 | 3.4932e-3 / 3.4397e-3 / 3.4300e-3 | 3.4401e-3 (16) |
| 0.2 | 8 / 16 / 24 | 2.3524e-2 / 2.3152e-2 / 2.3060e-2 | 2.3431e-2 (16) |

The thin plate `4 pi^2 D/a^2 = 3.615e-6 E` is met within 0.5 percent. This is the only run of rcfd code before the registration.

## Arithmetic done before the code (disclosed)

1. **Rectangle.** `E = 2e11`, `nu = 0.3`, `G = E/2.6`, `b = 0.01` (the width, along `e_2` of the element), `h = 0.2` (along `e_3`), `L
   = 3`: `A = 2e-3`, `I_y = b h^3/12 = 6.667e-6` (strong), `I_z = h b^3/12 = 1.667e-8`, `J = (h b^3/3)(1 - 0.63 (b/h)(1 - b^4/(12
   h^4))) = 6.4567e-8` (input to the section; the closed form uses the same `J`), `kappa = 5/6`. `M_0 = (pi/L) sqrt(E I_z G J)
   = 4260.9 N m`; `C_w = b^3 h^3/144 = 5.556e-11`, `(1 + pi^2 E C_w/(G J L^2))^(1/2) = 1.001226`; `M_cr = 4266.1 N m`. `P_z = pi^2 E
   I_z/L^2 = 3655.4 N`. Neglected by the element: `(1 - I_z/I_y)^(-1/2) = 1.00125` and `C_w` (0.12 percent); discretisation
   of 16 elements estimated at `<= 2e-3`; total estimated `<= 5e-3`. Pre-buckling curvature `M/(E I_y) = 3.2e-3 /m`, deflection
   `kappa L^2/8 = 3.6e-3 m`: small.
2. **I section** (check I1, record): flanges `b_f = 0.1`, `t_f = 0.01`, web `h_w = 0.18`, `t_w = 0.006`, `L = 3`: `A = 3.08e-3`,
   `I_z = 1.6699e-6`, `I_y = 2.0983e-5`, `J = 7.9627e-8`, `h_c = 0.19`, `C_w = I_z h_c^2/4 = 1.5071e-8`, `M_0 = 47364 N m`, the `C_w`
   factor `1.2408`, `I_z/I_y = 0.080`. The model without warping gives the `C_w = 0` moment, so the miss is about `1/1.2408 - 1 = -19.4 percent`, and the neglected
   `(1 - I_z/I_y)^(-1/2) = 1.043` of the exact theory lowers the closed form's own reference by a further 4 percent in the other
   direction (about `-16 percent` in all). Registered as expected to miss by 10 to 25 percent; the number is recorded.
3. **Thick plate, Reissner-Mindlin, simply supported, hard (Navier)**: `w = W sin kx sin ky`, `psi_x = B cos kx sin ky`, `psi_y =
   B sin kx cos ky`, `k = pi/a`, square, uniaxial `N_xx`. With `s = kappa G t`, `d = D k^2` (`D = E t^3/(12 (1 - nu^2))`) the
   energy per `a^2/4` is `(1/2) x^T (K - N G) x`, `x = (W, B)`, `K = [[2 s k^2, -2 s k], [-2 s k, 4 d + 2 s]]`, `G = k^2 diag(1,
   c)`, with `c = 0` without the rotation term (the base element) and `c = t^2/6` with it (`2 (t^2/12)` from two rotation
   components). `det = 0` gives `(2 s - N)(4 d + 2 s - N k^2 c') - 4 s^2 = 0`, `c' = t^2/6`. Without the term `n_0 = 8 s d/(4 d + 2
   s)` (thin: `4 d = 4 pi^2 D/a^2`). Numbers (`kappa = 5/6`, `nu = 0.3`, `a = 1`, units of `E`): `t = 0.1`: `n_0 = 3.4222e-3`, `n_1
   = 3.3725e-3`, `n_1/n_0 = 0.98546`; `t = 0.2`: `n_0 = 2.35983e-2`, `n_1 = 2.25990e-2`, `n_1/n_0 = 0.95765`. The Shell4 probe at 16
   x 16 is `-1.9 percent` of `n_0` at `t = 0.2` (probe table), so the absolute comparison with `n_1` alone does not separate the
   two (the base is `+2.4 percent` of `n_1`): the ratio is the check.
4. **Hand element entries.** (i) Uniform `M_y = M`, Euler limit (shear factor `1e9`): `-(1/2) M (theta_z phi' - phi theta_z')`
   with `int theta_z dx = d(L) - d(0)` and `phi` linear gives `K[phi_0, v_1] = +M/L`, `K[phi_1, v_1] = -M/L`, `K[phi_0, v_0] = -M/L`,
   `K[phi_1, v_0] = +M/L` (the four entries between the twist and the lateral translation `v`, local axes, sign of `psi_xy = +theta_z`). (ii) Wagner: `(1/2) beta M int
   phi'^2 dx` gives `K[phi_a, phi_b] = beta M/L (+1 if a = b, -1 otherwise)` with `beta M = beta_y M_y - beta_z M_z`. (iii) Shear:
   `V_y phi w'`, `int phi_0 w_1' dx = 1/2`, `int phi_1 w_1' = 1/2`, `int phi_0 w_0' = int phi_1 w_0' = -1/2` give `K[phi_a, w_1] = +V_y/2`,
   `K[phi_a, w_0] = -V_y/2` for `a = 0, 1`. (iv) Rigid rotation: the translation rows of the full `N`, `V` geometric stiffness
   applied to a rigid rotation `omega` about each local axis equal `omega x F` at node 1 and `-omega x F` at node 0 (the rotation
   of the force, `F = (N, V_y, V_z)`); the rows were checked by hand for the three axes against the terms of D1. (v) Shell4,
   unit square, `N_xx = n`, counter-clockwise nodes: `K[(a, c), (b, c)] = (t^2/12) n g_{ab} m_{ab}`, `g` the 1D `int N' N'` `[[1, -1],
   [-1, 1]]` of the `x` index and `m` the `[[1/3, 1/6], [1/6, 1/3]]` of the `y` index, `c = theta_x, theta_y`: node 0 to 0 `n t^2/36`, node
   0 to 1 `-n t^2/36`, 0 to 2 `-n t^2/72`, 0 to 3 `n t^2/72`. (vi) Shell3, vertices `(0,0), (1,0), (0,1)`, `A = 1/2`, `N_xx = n`:
   `(t^2/12) n A g_a g_b` with `g = (-1, 1, 0)`: entries `n t^2/24 (1, -1, 0; -1, 1, 0; 0, 0, 0)`.

## Checks (limits fixed now)

Every aggregated error is asserted finite before it is compared.

| id | check | limit | estimate |
|---|---|---|---|
| B0 | default-bits record: the digests of D6 on the base and after | equal (bit) | exact |
| L1 | rectangle LTB, uniform moment, 16 elements, `M_cr = lambda M_ref` against `(pi/L) sqrt(E I_z G J)(1 + pi^2 E C_w/(G J L^2))^(1/2)` (acceptance) | `|ratio - 1| <= 1e-2` | `5e-3` |
| L2 | the same at 8 elements | `<= 2e-2` | `8e-3` |
| L3 | `M_ref` of opposite sign: the lowest positive factor | `|f(+M)/f(-M) - 1| <= 1e-8` | `1e-10` |
| L4 | `M_ref` scaled by 3: `lambda M_ref` unchanged | `<= 1e-9` | `1e-11` |
| L5 | the same beam rotated: axis along `(1, 1, 0)/sqrt2` (frame invariance) | `|M_cr/M_cr(axis x) - 1| <= 1e-6` | `1e-8` |
| C1 | interaction with thrust: `P_ref = 0.3 P_z`, `M_ref = M_0` proportionally; `lambda` against the root of `x^2 = (1 - 0.3 x)(1 - 0.3 x P_z/P_phi)` (`x lambda`), `P_phi = G J A/(I_y + I_z)` | `<= 1.5e-2` | `6e-3` |
| H1 | the four entries of arithmetic 4(i), shear factor `1e9` | `max |K - hand|/(M/L) <= 1e-7` | `1e-9` |
| H2 | Wagner entries (4(ii)), `beta_y = 0.7`, `beta_z = -0.4`, `M_y = 3`, `M_z = 2` | `<= 1e-12` | `1e-15` |
| H3 | shear entries (4(iii)), Euler limit | `<= 1e-7 V` | `1e-9` |
| H4 | the three function sums: `local_resultant = moment + shear + wagner` | `<= 1e-14 max` | `1e-16` |
| R1 | rigid rotation about each local axis, translation rows of `K_G(N, V)` (4(iv)), `N = 5, V_y = 2, V_z = -3`, `L = 2`, shear factor 5/6 | `<= 1e-12 max|F|` | `1e-14` |
| R2 | the end-force extraction: the static solve of the L1 model gives `M_y(0) = M_y(L) = M_ref`, other components zero | `<= 1e-8 M_ref` | `1e-10` |
| S1 | Shell4 element matrix difference (with the term minus without) against 4(v), unit square and a rotated copy (global matrix = `T^T` hand `T`) | `<= 1e-12 n t^2` | `1e-15` |
| S2 | Shell3 as 4(vi) | `<= 1e-12 n t^2` | `1e-15` |
| S3 | thick plate `t/a = 0.2`, Shell4 16 x 16, hard SS (probe set-up): `(N_cr with)/(N_cr without)` against `n_1/n_0 = 0.95765` | `|r/r_closed - 1| <= 5e-3` | `2e-3` |
| S4 | Shell3 16 x 16 x 2, same | `<= 1e-2` | `4e-3` |
| S5 | thin plate `t/a = 0.01`, Shell4 16 x 16: the ratio against its closed `n_1/n_0` (`0.99983572`, arithmetic 3 at `t = 0.01`) | `|r - r_closed| <= 5e-5` | `1e-5` |
| N1 | without the option the same models give the base digests (B0) and `N_cr(base) > N_cr(with)` | strict | |
| N2 | rectangle without the moment option: the L1 model (`N` terms only) has no positive factor below `10 M_0` (no LTB) | the factor list empty or `lambda M_ref > 10 M_0` | |
| N3 | shell option with zero pre-stress: the matrix is zero | exact | |
| I1 | I section (arithmetic 2), 16 elements, `(pi/L) sqrt(E I_z G J) sqrt(1 + ...)`: ignored record `ltb_i_section_known_miss` | `<= 1e-2` (expected to miss) | `-16 percent` |

Negative controls: N1 to N3 above; L3 is a sign control; the shell option changes nothing of a beam model and the beam option
nothing of a shell model (checked inside B0: the same models with the other option set give the base digests).

## Planned mutations (on a scratch copy, one at a time)

| id | mutation | caught by |
|---|---|---|
| X1 | drop the moment terms | L1, L2, C1, H1 |
| X2 | moment terms times 2 | L1, H1 |
| X3 | Wagner sign flipped | H2 |
| X4 | shear terms `V_y` sign flipped | H3, R1 |
| X5 | end-moment sign convention flipped at node 0 (`M(0) = +f`) | R2, L1 |
| X6 | shell term `t^2/8` for `t^2/12` | S1, S2, S3, S4 |
| X7 | shell term also on `theta_z` | S1 |
| X8 | `scaled` does not scale the end forces | L4 |
| X9 | option flags ignored (always off) | L1, S3 |
| X10 | option flags ignored (always on in the base path) | B0 |

## Decision rule

**done**: B0, L1 to L5, C1, H1 to H4, R1, R2, S1 to S5, N1 to N3 met and the I section (a4) met. **partial**: everything met except
I1 (the expected case: the element has no warping DOF) or one of L2, C1, S4, S5 recorded as a known miss with a diagnosed cause. **todo**: L1 or
B0 missed.
Because (a4) cannot be met without a warping DOF, the proposed status is **partial** even if every other check is met (a
clause of the acceptance not met).

## D8 (the bit record) and D9 (existing tests)

B0 is committed after this ADR and before any library line; its constants are those printed on the base. No existing test is edited.

## Limits (not covered)

- No warping DOF: `C_w` is not modelled (the I section is a recorded miss).
- No distributed load inside an element (end forces only, linear `M`, constant `V`).
- The nodal-moment rotation terms of the semi-tangential or quasi-tangential families are not modelled: the energy Hessian of
  D1 is symmetric and exact for the rotation-vector second variation, but a structure whose nodal moments are applied
  externally at rotated nodes needs the load stiffness of a moment follower (not part of this item).
- Wagner `beta` coefficients are one pair for all beam elements of the model (the section has no shear-centre or sectorial data).
- A Layup shell takes `t^2/12` of its total thickness with the uniform stress `N/t`.
- Shell3 and Shell4 with a non-through-thickness-uniform pre-stress (bending pre-stress) take the membrane resultant only.
- The corotated Newton path and the nonlinear drivers are untouched.

## Open questions (to be taken on the maintainer's standing instruction of 2026-11-11 when answered; none changes a default)

- **Q1. Warping DOF.** Options: (a) a 14-DOF beam element with a warping DOF and `C_w` in `BeamSection` (follow-up item, makes the
  I section testable); (b) leave the I section as the recorded miss. Recommendation (a), as a separate item (`BeamSection`, the
  mass, the stiffness, the loads, the section forces and the hash tables all change).
- **Q2. Case-file key.** Options: (a) none now; (b) `[structural] geometric_terms = "full"` on the buckling and the pre-stressed
  mode blocks, the default `"classical"` (today's behaviour). Recommendation (b) with the default unchanged, as a separate item
  (the schema golden changes). No key is added here.
- **Q3. Per-section Wagner coefficients.** Options: (a) uniform input as here; (b) `BeamSection::mono_symmetric(beta_y, beta_z)`.
  Recommendation (b) when a channel or a T section is wanted (changes the section constructor and its hash).

## Amendment 1 (2026-11-14): two set-ups of the registration are defective; written after the library code was compiled and before any test of F-558 was run

Two checks of the table cannot be run as written. No result of F-558 exists yet; the B0 digests of the base were recorded before.

1. **L5.** A beam along `(1, 1, 0)/sqrt2` cannot be given fork supports with per-component displacement constraints (the
   supports act on the global components). Replaced: L5 is the same LTB model along the global `y` axis with the orientation
   vector `z` (so `e_1 = y`, `e_2 = z`, `e_3 = x`: the local DOF `u, v, w, theta_y` are the global components 1, 2, 0 and the
   rotation about `z`), the end moments about `z` (component 5), the supports permuted; same limit `|M_cr/M_cr(axis x) - 1| <= 1e-6`.
   New **L5b** (element level, skew): the global matrix of `element_resultant_geometric_stiffness` of an element along
   `(1, 1, 0)/sqrt2` (and the orientation rotated with it) against `Q K Q^T` of the axis-`x` element, `Q` the rotation of the
   coordinates: `max|difference| <= 1e-12 max|K|` (estimate `1e-15`).
2. **R1.** An end-force set with shear and no moments is not in equilibrium (the moments change along the element by `M_y' =
   V_z`, `M_z' = -V_y`, from the moment balance of the element about node 0). The shear terms and the moment terms jointly
   give `omega x F` in the translation rows (by parts, `int M' (dtheta/dq) dx`: the moment terms supply the half that the shear
   terms lack: the shear terms alone give half of it; hand check for the twist about `e_1`, row `w_1`, `V_y/2 + V_y/2`). R1
   is run with the equilibrated set: `f = (-N, -V_y, -V_z, 0, 0, 0 | N, V_y, V_z, 0, L V_z, -L V_y)`; the compared quantity
   and the limit (`<= 1e-12 max|F|`, all six translation rows, the three rotation axes) are unchanged.

## For the lead: roadmap entries

(To be filled after the run, in the addendum.)
