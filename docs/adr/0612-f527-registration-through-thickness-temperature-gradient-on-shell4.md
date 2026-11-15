# ADR 0612: F-527 registration: through-thickness temperature gradient on Shell4

Status: **REGISTERED 2026-11-15, written before any code of F-527 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-527 was written, compiled or run in rcfd: the files listed under "What was read" were
read and the arithmetic below was done by hand from closed forms (no script, no finite-element code). Results go in the addendum
at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0445](0445-f525-registration-shell-nodal-body-force-and-thermal-loads.md) (F-525: the membrane thermal load of
`Load::Temperature` on Shell4, uniform through the thickness; its L1 and remaining question 2 name this item and its "For the
lead" entry gives the acceptance), [ADR 0432](0432-f392-registration-flat-facet-shell.md) (F-392: the facet, the bending rows,
MITC4, `T` with rigid offsets, `shell::stresses` at the centre), [ADR 0496](0496-f428-registration-laminated-shell-layup-ply-stresses-failure.md) is referred to only for the
layup branches of `shell::stresses` and `model_thermal_load_vector`, which are not changed here. Roadmap item F-527, deps F-525.
Number 0612 given by the lead. Branch `f527-work` from `2bc0141c` (main), worktree `w49i-work/f527`, scratch
`w49i-work/f527-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number); each registered run made once; defaults bit-identical; no
edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test or ignored record edited;
no new dependency, no unsafe code; clean-room (first principles: the linear thermal strain through the thickness of a
Reissner-Mindlin plate; the classical result that a free plate under a linear through-thickness temperature bends to the
curvature `alpha dT/dz` without stress and a clamped one carries `M = -D (1 + nu) alpha dT/dz` is in the plate textbooks, e.g.
Timoshenko and Woinowsky-Krieger, Theory of Plates and Shells, the thermal-bending section, **cited from memory, not read**; it
is derived below and only the derivation is used). The roadmap files are not edited. Every choice the maintainer would normally
be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation) and marked so.

## What F-527 asks

The acceptance, verbatim, binding:

> Every limit not stated here is registered in the ADR before the run. A nodal temperature gradient through the thickness on a
> Shell4 model gives the curvature thermal load int B_b^T D_b kappa_th dA with kappa_th = alpha dT_grad; a free plate under a
> uniform gradient takes the curvature alpha dT_grad and zero moment within 1e-12; a model without a gradient gives the bits it
> gave before.

Clauses: **a1** every other limit registered here; **a2** a nodal gradient on a Shell4 model gives the load `int B_b^T D_b
kappa_th dA`, `kappa_th = alpha g (1, 1, 0)`; **a3** a free plate under a uniform gradient takes the curvature `alpha g` within
1e-12; **a4** and zero moment within 1e-12; **a5** a model without a gradient gives its former bits. From the brief: **b1** a
clamped plate under a uniform gradient gives `M = -D (1 + nu) alpha g` (derived here); **b2** Shell3 covered or refused clearly;
**b3** a membrane temperature and a gradient superpose; **b4** mutations (wrong sign, missing `1 + nu`, gradient ignored)
caught; **b5** the API opt-in (a new variant), no case-file key (an open question).

## What was read (2bc0141c, read only)

- ADR 0445 whole (design, the free-plate plates P and Q, amendments 1 and 2: the rigid in-plane rotation held only by the
  drilling penalty at the clamped node, the addendum, L1).
- Code: `crates/rcfd-fem/src/shell.rs` (`shape`, `point`, `facet`, `plane_matrix`, `covariant_row`, `shear_rows`,
  `tying_rows`, `membrane_rows`, `bending_rows`, `homogeneous_section`, `transformation`, `thermal_load_vector_with`,
  `layup_thermal_load_vector`, `model_thermal_load_vector`, `thickness_or_error`, `stresses`, `layup_centre`,
  `centre_strains`), `thermal.rs` (`temperature_field`, `add_thermal_load` with the Shell4 and Shell3 branches), `loads.rs`
  (`nodal_load_vector`, the `match` over `Load`), `model.rs` (`Load`, its constructors, `add_load` validation, the hyperelastic
  structural refusal of a temperature load), `nonlinear.rs` and `nl_transient.rs` (the refusals of `Load::Temperature`),
  `fracture.rs` (`check_loads`), `cyclic.rs` (the own-loads refusal of the harmonic path), `shell3.rs` (the bending rows and
  the thermal load: Reissner-Mindlin with its own shear treatment), `error.rs` (`NotYetSupported`).
- Tests: `si_shell_thermal.rs` (helpers `grid`, `rotation`, `Placement`, `shell_mesh`, `th_plane`, `centre_strain`, the
  `Checks` pattern), `si_shell_thermal_bits.rs` (the FNV-1a digest pattern).
- `docs/reference-data.md`: nothing on thermal bending of plates; the reference values here are closed forms.

## Arithmetic before this registration (disclosed)

1. **Kinematics and sign (from `bending_rows`, `covariant_row`).** In facet axes the rotations give `beta_x = theta_y`,
   `beta_y = -theta_x`; the in-plane displacement at height `z` along `e_3` is `u(z) = u_0 + z beta`, so `eps(z) = eps_0 + z
   kappa` with `kappa = (beta_x,x, beta_y,y, beta_x,y + beta_y,x)`, exactly the rows of `bending_rows`; the shear strain is
   `gamma = grad w + beta` (`covariant_row`: `w_xi + beta . J_xi`).
2. **The thermal strain and its load (a2).** A temperature `T(z) = T_m + z g` (`g = dT/dz` along the facet's `e_3`, K/m) gives
   the plane thermal strain `eps_th(z) = alpha (T_m - T_ref + z g) (1, 1, 0)`, so `eps_th = eps_th,0 + z kappa_th`, `kappa_th =
   alpha g (1, 1, 0)`. Plane stress: `sigma(z) = Q (eps_0 - eps_th,0 + z (kappa - kappa_th))`, `Q` the plane matrix;
   integrating over `[-t/2, t/2]`: `N = A (eps_0 - eps_th,0)`, `M = D (kappa - kappa_th)`, `A = t Q`, `D = (t^3/12) Q` (no
   coupling for a homogeneous section, the odd moments vanish). The energy `(1/2) int M . (kappa - kappa_th)` varied gives the
   load `f = int B_b^T D kappa_th dA`. With `g(xi, eta) = sum_i N_i g_i` (the facet's bilinear functions) the local load is
   `f_l = sum_gp det B_b^T D kappa_th` at the 2 x 2 points of the stiffness, carried to the nodes by `T^T`. `D kappa_th =
   alpha g (D_00 + D_01, D_10 + D_11, 0) = alpha g E t^3 / (12 (1 - nu)) (1, 1, 0)` since `D_00 + D_01 = E t^3 (1 + nu) / (12 (1 -
   nu^2))`. So `M_th = D (1 + nu) alpha g` per direction, `D = E t^3 / (12 (1 - nu^2))`.
3. **The free plate is the discrete solution (a3, a4).** Under a uniform `g` the field `beta = k (x, y)`, `w = -k (x^2 + y^2) /
   2`, `k = alpha g`, about any point (plus a rigid motion), membrane and drilling zero, solves the continuum problem with `gamma
   = grad w + beta = 0`, `kappa = (k, k, 0) = kappa_th`, `M = 0`. On the element: `beta` is linear, so the bilinear
   interpolation reproduces it exactly and the curvature at every point is `(k, k, 0)`; `w` is quadratic and not reproduced,
   but MITC4 ties the covariant shear at the edge mid-points: on a straight edge from `a` to `b`, `w_xi = (w_b - w_a) / 2`, and
   for a quadratic `w` the secant slope equals the tangent at the mid-point, `(w_b - w_a) / 2 = grad w(m) . (b - a) / 2`; `J_xi`
   at the mid-point of that edge is `(b - a) / 2` and `beta` there is `beta(m)`. So every tying value is `(grad w + beta)(m) .
   J_xi = 0`, the assumed shear is zero everywhere, and `K_b u = int B_b^T D B_b u = int B_b^T D kappa_th = f_l`, `K_s u = 0`.
   The isotropic field `k (x, y)` and `-k |x|^2 / 2` are invariant under the in-plane rotation between facets of a flat plate
   (the map `r -> (-r_y, r_x)` commutes with rotations), so every facet's local field is the same global field: `theta =
   k ((Y_0 - Y), (X - X_0), 0)` in plate axes (`theta_x = -beta_y`, `theta_y = beta_x`), `w = -k |X - X_0|^2 / 2`, zero at node 0
   with zero rotation there. Hence the discrete solution with node 0 clamped (the six rigid motions, ADR 0445 arithmetic 4) is
   this field up to rounding: curvature `alpha g (1, 1, 0)`, `M = 0`, `N = 0`, surface stresses zero.
4. **The clamped plate (b1).** Every boundary node clamped (6 components), the interior nodes free. For a uniform `g` the
   interior entries of the load vanish in exact arithmetic (the patch integral of `N_i,x` is the boundary integral of `N_i`
   over the patch boundary, where `N_i = 0`; arithmetic 7), so the discrete solution is `u = 0`, `kappa = 0`, and `M = D (0 -
   kappa_th) = -D (1 + nu) alpha g (1, 1, 0)` at every point of every element: `M_xx = M_yy = -E t^3 alpha g / (12 (1 - nu))`,
   `M_xy = 0`; the surface stresses `sigma = N / t +- 6 M / t^2` (exact for a linear `sigma(z)`, arithmetic 2): `top = -E alpha g
   t / (2 (1 - nu)) (1, 1, 0)`, `bottom` its negative. In rounding the interior load is `~eps` of the boundary entries, so `u ~
   eps |f| / k`, and the curvature `~1e-15` of `alpha g`.
5. **A linear gradient on a rectangle (G3, a2).** One element `a x b` in the `xy` plane with its first edge along `x`
   (local axes = global, local coordinates about the centre), nodes in the order of `CORNERS = (-,-), (+,-), (+,+), (-,+)`,
   signs `(a_i, c_i)`; `g = g_c + c_1 X + c_2 Y` at the nodes (`g_c` the centre value, `X, Y` about the centre). `X = (a/2)
   xi`, `N_i,X = (2/a)(a_i/4)(1 + c_i eta)`, `det = ab/4`, `k = D (1 + nu) alpha`. `f(theta_y)_i = k int N_i,X g dA` and
   `f(theta_x)_i = -k int N_i,Y g dA` (the row `kappa_y = -theta_x,y`; `kappa_xy` gets the zero third entry). With `int d xi =
   2`, `int xi d xi = 0`, `int (1 + c eta) d eta = 2`, `int eta (1 + c eta) d eta = 2c/3`: `int (1 + c_i eta) g dxi deta = 4 g_c
   + (4/3) c_2 (b/2) c_i`, so `f(theta_y)_i = k a_i (b / 8) (4 g_c + (2/3) c_2 b c_i)`, and by symmetry `f(theta_x)_i = -k c_i (a
   / 8) (4 g_c + (2/3) c_1 a a_i)`. Every other entry (forces, `theta_z`) is zero exactly: only `B_b` enters. The integrand is
   of degree 2 per variable, so 2 x 2 Gauss is exact; rounding `~1e-15`.
6. **Superposition (b3).** The membrane load (ADR 0445) enters only `B_m`, the gradient only `B_b`; the stiffness is the same;
   so the solution of both is the sum, and on a free plate under both, uniform, the membrane strain is `alpha dT (1, 1, 0)` and
   the curvature `alpha g (1, 1, 0)`, `N = 0`, `M = 0`. The load vector of the model with both is the sum of the two load
   vectors up to the order of the additions (`nodal_load_vector` adds load after load into one vector): `~eps` of the largest
   entry.
7. **Self-equilibrium (G5).** The local load has zero force entries; `T^T` maps local moments to global ones by `Lambda^T`
   only (the offset block multiplies local forces), so the global forces are exactly zero and the moments sum to `Lambda^T sum_i
   m_i`, with `sum_i m_i = int (sum_i N_i,x, sum_i N_i,y) ... = 0` (isoparametric: `sum_i N_i,x = 0`). Rounding `~1e-15` of the
   largest nodal moment, on a warped mesh too.
8. **Rounding of the free plates.** The bending-shear part: the shear-to-bending stiffness ratio of MITC4 is `kappa_s G t h^2
   / (D) = 5 (1 - nu) (h / t)^2 = 3.5 (h / t)^2` (the figure ADR 0572 quotes). Plates (the ADR 0445 TH mesh, 4 x 3 distorted
   elements on `[0, 2] x [0, 1]`, `h ~ 0.33` to `0.5`): **P** flat in `xy`, `t = 0.1` (ratio `~90`); **Q** the same mesh rotated
   about `(1, 2, 3)` by 0.7 rad and moved by `(0.3, -0.2, 0.5)`, `t = 0.1`; **R** P with `t = 0.01` (ratio `~9e3`). The curvature
   carries the forward error of the bending solve relative to its scale: `~eps` times the ratio times a mesh factor `~10`: P, Q
   `~2e-13`, R `~2e-11`. On P the in-plane and out-of-plane problems decouple exactly (facets with `e_3 = (0, 0, 1)`, a load with
   exact zeros on `u_x, u_y, theta_z`): the in-plane displacements and `theta_z` are exactly zero. On Q rounding of the
   transformation puts `eps |f|` into the membrane and drilling unknowns; the drilling soft mode at node 0 (ADR 0445 amendments
   1, 2) moves in rigid in-plane rotation and carries no strain, so it is not checked; the membrane strain it leaves is `~eps`
   of the bending displacement over `h`, i.e. relative to the bending stress scale `E alpha g t / 2` about `2e-14`. The
   out-of-plane displacement and rotations of P against arithmetic 3: rigid out-of-plane rotation about node 0 is held by the
   bending and shear stiffness of element 0 (not by the drilling penalty), so the forward error is `~eps` times the condition
   of the bending problem `~1e5`: `~2e-11` relative; registered with a looser limit 1e-9 (stronger than the acceptance).
9. **The bits of a model without a gradient (a5).** The design adds a `Load` variant and code entered only when a model holds
   it (a new `match` arm in `nodal_load_vector`; in `shell::stresses` the gradient correction only with a gradient load; the
   refusals of the nonlinear paths and fracture only for that variant). Nothing that existing models reach is reordered, so
   their bits are unchanged; checked by digests (B1) and by the existing tests (B2).

## Design

### D1. The load variant (`model.rs`), opt-in

New `Load::ShellTemperatureGradient { gradient: Vec<f64> }` (`#[non_exhaustive]`), one value per node, K/m: the temperature
gradient `dT/dz` through the thickness, `z` along each element's facet normal `e_3` (the normal of `d_1 x d_2`, the
right-hand rule of the element's node order). Constructor `Load::shell_temperature_gradient(gradient)`. `add_load` refuses a
length other than the node count (`FieldLength { key: "gradient" }`) and a non-finite value (`InvalidValue { key: "gradient"
}`). It is independent of `Load::Temperature` (a model may have either, both or neither; the reference temperature does not
enter a gradient). A model has at most one gradient load: a second is refused at the load assembly and in `shell::stresses`
(`InvalidValue { key: "gradient" }`), as a second temperature field is. No case-file key (open question 1).

### D2. The load (`shell.rs`, `thermal.rs`, `loads.rs`)

New `shell::thermal_gradient_load_vector(coords, thickness, material, gradient_nodal: &[f64]) -> Result<[f64; 24]>`
(arithmetic 2): refuses other than four values (`FieldLength { key: "gradient" }`) and a material without a finite expansion
(`InvalidValue { key: "expansion" }`, as `thermal_load_vector_with`); `D` is `homogeneous_section(material, t).bending`; at the
2 x 2 points `g = sum_i N_i g_i`, `m = D (alpha g, alpha g, 0)`, `f_l += det B_b^T m`; `T^T` to the nodes. New
`thermal::temperature_gradient_field(model) -> Result<Option<&[f64]>>` and `thermal::add_thermal_gradient_load(f, dofs, model,
gradient)`: element by element in element order; a Shell4 element of a `Shell` block (either membrane: the gradient enters only
the bending rows) gets the 24 entries as forces and moments; an element of any other kind (Shell3, Beam2, solids) or a Shell4
element of a `Layup` block with a non-zero gradient value at one of its nodes is refused, `Element { element: e, source:
NotYetSupported { item: "shell temperature gradient", variant: <kind name, or "Layup">, location, package:
thermal::GRADIENT_FOLLOW_UP } }`; such an element whose nodal values are all zero is skipped (it receives nothing). `loads.rs`
gets the arm `Load::ShellTemperatureGradient { gradient } => { temperature_gradient_field(model)?; add_thermal_gradient_load(..)
}`.

### D3. The recovery (`shell.rs`)

`shell::stresses` reads `temperature_gradient_field(model)` once. Without a gradient load every line runs as before. With one:
the homogeneous Shell4 branch subtracts `alpha g_c (1, 1, 0)` from the centre curvature before `D` is applied (`g_c = sum_i
N_i(0, 0) g_i`), and the surface stresses follow from the resultants as before (exact, arithmetic 2); a Layup element (either
branch) or a Shell3 element with a non-zero gradient at one of its nodes is refused as in D2.

### D4. The paths that do not take it

`nonlinear::check_driver_model` (the line that refuses `Load::Temperature`) and the same check of `nl_transient` refuse the
gradient load in the same way; `fracture::check_loads` refuses it ("a shell temperature gradient"). The harmonic cyclic path
already refuses every own load. Every other path that builds its load through `nodal_load_vector` takes it (the load is linear
and static).

### D5. Bits

A model without the gradient load never enters the new arm or the new branches of D3 and D4; the stresses function's only
addition on such a model is the scan of the loads for the variant, which reads and does not compute. B1 and B2 check it.

## Checks (limits fixed now)

Material: `E = 2e11`, `nu = 0.3`, `alpha = 1.2e-5`, `rho = 7850`. Gradient `g = 200` K/m unless stated (`k = alpha g = 2.4e-3`
1/m). Scales: `k`; the moment `M_s = D (1 + nu) alpha g`; the surface stress `s_s = E alpha g t / (2 (1 - nu))`.

| id | clause | check | limit | estimate |
| --- | --- | --- | --- | --- |
| G1a | a3 | plates P and Q (arithmetic 8), node 0 clamped, uniform `g`: the curvature at each element centre computed **in the test** from the solved rotations projected on the plate axes (`beta_x = theta . e_y`, `beta_y = -theta . e_x`) and the bilinear derivatives in plane coordinates, against `k (1, 1, 0)`: `max / k` | `1e-12` per plate | arithmetic 3, 8: `~2e-13` |
| G1b | a4 | the same solves, `shell::stresses`: `max |M_k| / M_s` | `1e-12` per plate | arithmetic 3, 8 |
| G1c | (stronger, a4) | the same: `max |sigma_top,k|, |sigma_bottom,k| / s_s` and `max |N_k| / (s_s t)` (`s_s t = 6 M_s / t`, the membrane force of the bending stress scale) | `1e-12` each, per plate | arithmetic 8: Q `~2e-14` |
| G1d | (stronger, a3) | plate P: `w_n` against `-k |X_n - X_0|^2 / 2` relative to `k L^2 / 2`, `L = max |X_n - X_0|`; rotations against `k ((Y_0 - Y_n), (X_n - X_0), 0)` relative to `k L`; the in-plane displacements and `theta_z` exactly zero | `1e-9`; exact zero | arithmetic 8: `~2e-11`; decoupling |
| G1e | (stronger, a3) | plate R (P with `t = 0.01`): G1a and G1b | `1e-10` each | arithmetic 8: `~2e-11` |
| G2 | b1 | plates P and Q, every boundary node clamped (6 components), uniform `g`: `shell::stresses` at every element centre: `max |M - (-M_s, -M_s, 0)| / M_s`; `max |top - (-s_s, -s_s, 0)|, |bottom - (s_s, s_s, 0)| / s_s`; and **in the test**, at the 2 x 2 Gauss points of every element, `D (kappa_h - k (1, 1, 0))` from the solved rotations against `(-M_s, -M_s, 0)`, relative to `M_s` | `1e-12` each, per plate | arithmetic 4: `~1e-15` |
| G3 | a2 | one rectangle `0.6 x 0.4` on `[0, 0.6] x [0, 0.4]`, `t = 0.02`, `g_i = 100 + 300 X_i - 150 Y_i`: `nodal_load_vector` against arithmetic 5, relative to the largest entry; forces and `theta_z` exactly zero | `1e-12`; exact | arithmetic 5: `~1e-15` |
| G4 | b3 | plate Q, node 0 clamped, `Load::Temperature` uniform 350 K against 300 and the uniform gradient: (i) the load vector against the sum of the two single-load vectors, relative to the largest entry; (ii) the membrane strain (test side, ADR 0445 TH1) against `alpha dT (1, 1, 0)` relative to `alpha dT`, the curvature (G1a) against `k (1, 1, 0)` relative to `k`; (iii) `shell::stresses`: `max |M| / M_s`, `max |N| / (E t alpha dT / (1 - nu))`, surface stresses relative to `E alpha dT / (1 - nu) + s_s` | (i) `1e-14`; (ii), (iii) `1e-12` each | arithmetic 6, 8 |
| G5 | (stronger, a2) | the ADR 0445 warped mesh (BF1 mesh unrotated, node k lifted by `0.002 sin(1.7 k + 0.3)`), `t = 0.02`, `g_k = 150 + 80 x_k - 60 y_k`: every force entry exactly zero; `|sum_i m_i| / max_i |m_i|` (moments about any point equal, the forces being zero) | exact; `1e-12` | arithmetic 7 |
| G6 | (stronger, D2) | plate Q with `Shell4Membrane::AndesOpt` (`BlockProperty::shell4_with_membrane`): the gradient load vector equals bitwise that of the bilinear block (the gradient enters only the bending rows; the same operations) | exact | D2 |
| B1 | a5 | FNV-1a digests, printed by `si_shell_thermal_gradient_bits.rs` run on a copy of 2bc0141c before any library code of F-527 (`f527-scratch/b1-base.log`), then fixed in the file: (1) plate Q `t = 0.05` with `Load::Temperature` `300 + 40 X - 25 Y` against 290 and gravity: `nodal_load_vector`, the `solve_static` displacement (node 0 clamped), every field of `shell::stresses`; (2) the same mesh as a Layup of one isotropic ply, temperature only: load vector; (3) a Hex8 box `[2, 1, 1]`, `[2, 1, 1]` elements, temperature `300 + 10 x + 5 y z` against 290: load vector; (4) a Shell3 plate (each P quadrilateral cut on its 0-2 diagonal), the plate-Q temperature: load vector and `shell::stresses` | exact | arithmetic 9 |
| B2 | a5 | `baseline_hashes` (rcfd-api, release) and the rcfd-fem test files covering changed lines, unedited: `si_shell_thermal`, `si_shell_thermal_bits`, `si_shell`, `sb_thermal`, `sb_thermal_bits`, `sb_loads`, `sa_shell3`, `sa_shell4_membrane`, `sa_corot_shell` | pass | D5 |

### Negative controls

| id | model | expected | limit |
| --- | --- | --- | --- |
| N1 | the B1 Shell3 plate with a uniform gradient: `nodal_load_vector` and `shell::stresses` (zero displacement) | `Element { source: NotYetSupported { item: "shell temperature gradient", variant: "Shell3", package: GRADIENT_FOLLOW_UP } }` both | exact |
| N2 | plate P as a Layup of one isotropic ply, uniform gradient: `nodal_load_vector` | `Element { source: NotYetSupported { variant: "Layup", .. } }` | exact |
| N3 | plate P with two gradient loads | `InvalidValue { key: "gradient" }` | exact |
| N4 | `add_load` with a gradient of `n - 1` values; with one NaN | `FieldLength { key: "gradient" }`; `InvalidValue { key: "gradient" }` | exact |
| N5 | plate P with a zero gradient everywhere | the load vector and the solved displacement exactly zero | exact |
| N6 | a Hex8 box with a zero gradient (all values 0) and the same box with a non-zero value at one node | the first: the load vector the bits of the box without the load; the second refused naming a `Hex8` element | exact |
| N7 | (sensitivity of G1b) the free plate P's moment computed in the test as `D kappa_h` from the G1a curvature, without the thermal curvature | `|M_xx| / M_s` above 0.5 (it is 1) | `> 0.5` |
| N8 | `rcfd_fem::nonlinear::solve` of plate P with the gradient (default `NewtonOptions`) | refused, `InvalidValue { key: "nonlinear" }` naming the gradient load | exact |

### Planned mutations (scratch copy `f527-scratch/tree` from `git archive` of the final commit; not committed)

| id | mutation | caught by (expected) |
| --- | --- | --- |
| MU1 | the sign of `kappa_th` flipped in `thermal_gradient_load_vector` | G1a, G3 |
| MU2 | `1 + nu` missing: `m = (D_00 alpha g, D_11 alpha g, 0)` (the Poisson coupling of `D` dropped) | G1a, G3 |
| MU3 | the gradient ignored: the `loads.rs` arm adds nothing | G1a, G3 |
| MU4 | `stresses` does not subtract the thermal curvature | G1b, G2 |
| MU5 | the sign of the thermal curvature in `stresses` flipped | G1b, G2 |
| MU6 | `T^T` left out (local entries added as global) | G1a on Q, G2 on Q |
| MU7 | the gradient at every Gauss point that of the element's first node | G3 |
| MU8 | the Shell3 refusal removed (a Shell3 element skipped silently) | N1 |
| MU9 | `1 + nu` missing in `stresses` (`kappa_th = alpha g (1, 0, 0)`) | G1b, G2 |

## Decision rule

- **done**: every check and negative control met, every planned mutation caught, B2 passing.
- **partial**: a registered check missed (an ignored `*_known_miss`), a mutation not caught, or a check met only after an
  amendment written after its run.
- **todo**: the code does not compile or the gradient path is not reached.

## Limits (not covered)

- L1. Shell3 is refused under a non-zero gradient (b2, its own bending and shear treatment needs its own check); a follow-up.
- L2. Layup shells (one ply or more) are refused under a non-zero gradient (the ply-integrated thermal moments of a gradient
  are a laminate question); a follow-up.
- L3. Beams and solids are refused under a non-zero gradient at their nodes; a beam stiffener sharing nodes with a heated
  shell can therefore not carry a gradient value at those nodes. A temperature gradient in a solid is a temperature field.
- L4. The gradient is along each element's own `e_3`: a mesh with inconsistently oriented elements gets opposite gradients in
  them (as `Load::ShellPressure` does); orientation is the user's.
- L5. Recovery only by `shell::stresses` at the element centre; the nonlinear static and transient paths and fracture refuse
  the load; the other linear paths take the load and do not recover moments.
- L6. No rcfd-api, schema or case-file key (open question 1).

## Open points taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. A separate `Load` variant (gradient per node, K/m) rather than a new optional field on `Load::Temperature`: the existing
   variant's destructuring sites stay untouched and a gradient does not need a reference temperature.
2. The gradient's sign along each facet's `e_3` rather than a global direction: it is the only direction a facet defines.
3. Refusal of Shell3, Layup, beams and solids with a non-zero value, acceptance with zero values (so a mixed model can give the
   field zero where it does not apply).

Open (not taken: would need a schema decision): **Q1** a case-file key for the gradient.

## Amendment 1 (2026-11-15, written and committed after the registered run, before any run of D1 and D2)

The registered run (`f527-scratch/run1.log`, commit `8375c4da`, release, `--test-threads=1`) missed rows of G1 and G4: G1a
(curvature) P `2.96e-12`, Q `3.36e-12`; G1b (moment) P `1.92e-12`, Q `1.58e-12`; G1c surface stresses P `1.92e-12`, Q
`1.59e-12`; G1e (plate R, `t = 0.01`) curvature `2.63e-10`, moment `1.23e-10` (limit `1e-10`); G4 (ii) curvature `3.74e-12`,
G4 (iii) moment `1.81e-12` (limits `1e-12`). Every other row of G1 to G6 and every negative control was met (G1d P `w`
`2.0e-12`, rotations `1.3e-12` against `1e-9`, in-plane exactly zero; G2 `2.5e-16`; G3 `2.2e-16`; G4 (i) `5.1e-19`, membrane
strain `2.7e-14`; G5 `6.2e-16`; G6 bitwise). The limits stay; both tests become ignored `*_known_miss` records with the measured
numbers, and running copies keep every met row with its limit and print the missed rows.

Diagnosis (to be confirmed by D1 and D2, not a change of limit): the clamped plate (G2, `u ~ 0`) recovers the moment to
`2.5e-16`, the load matches its closed form to `2.2e-16` (G3), so the load and the recovery are right; the misses are in the
free plates only, where the solve has to produce the bending field. On plate P the out-of-plane problem is exactly decoupled
(in-plane exactly zero), so the error is the forward error of the bending-shear solve. It scales with the shear-to-bending
stiffness ratio `3.5 (h / t)^2`: from P (`t = 0.1`) to R (`t = 0.01`) the ratio grows by 100 and the curvature error by 89.
Arithmetic 8 took a mesh factor 10 over `eps` times that ratio; the measured factor is `~150` (`3e-12 / (2.2e-16 * 87)`). The
mechanism: the exact discrete field (arithmetic 3) has zero assumed shear only through the cancellation of `grad w` and
`beta` at the tying points; the shear stiffness `kappa_s G t` multiplies this cancellation, so the residual of any computed
field carries `eps |K_s| |u|`, larger than the bending load by the ratio.

Added checks (registered now, before their run), test `d1_d2_diagnosis_after_amendment1`:

| id | clause | check | limit | estimate |
| --- | --- | --- | --- | --- |
| D1a | (diagnosis) | plates P and Q, the exact discrete field of arithmetic 3 (`w = -k |X - X_0|^2 / 2` along the plate normal, rotations `k ((Y_0 - Y), (X - X_0), 0)` in plate axes) given to `shell::stresses` with the uniform gradient: `max |M_k| / M_s` | `1e-13` per plate | exact linear `beta`: `~1e-15` |
| D1b | (diagnosis) | the same plates: `r(u) = K u - f` on the 114 equations not of node 0 (`K` from `assemble::stiffness`, `f` from `nodal_load_vector`), for the exact field and for the solved field of `solve_static`: `max |r(u_h)| <= 10 max |r(u_ex)|` (the computed solution is backward stable: its residual is that of the exact field rounded); both printed relative to `max |f|` | ratio `<= 10` per plate | backward-stable direct solve: ratio `~1` |
| D2 | (diagnosis) | plate S, P with `t = 0.3` (ratio `3.5 (h/t)^2 ~ 10`), node 0 clamped, uniform gradient: G1a and G1b | `1e-12` each | the measured scaling: `3e-12 (0.1 / 0.3)^2 = 3.3e-13` |

The decision is partial by the registered rule whatever D1 and D2 give.
