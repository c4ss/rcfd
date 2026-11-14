# ADR 0432: F-392 registration: the four-node flat-facet shell Shell4 (bilinear membrane, Reissner-Mindlin bending with the MITC4 assumed transverse shear strain, a drilling penalty, rigid offsets for warped facets, consistent mass with rotary inertia, pressure load, resultants and surface stresses)

Status: **REGISTERED 2026-11-14, written before any code of F-392 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-392 was written, compiled or run in rcfd: the files listed under "What was read" were
read and the arithmetic below was done by hand and evaluated with a plain numpy prototype of the element derived here
(`w49i-work/f392-scratch/estimate.py`, `estimate2.py`, `estimate3.py`, logs `estimate*.log`; dense matrices, no rcfd code;
disclosed, its numbers quoted below). Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0295](0295-quadrilateral-shell-element.md) (the stub, work package SI-5), [ADR 0290](0290-structural-solver-intermediate-tier-plan.md)
(the plan), [ADR 0291](0291-intermediate-scaffold-modules-and-model-extensions.md) (the scaffold: `shell.rs`,
`ElementKind::Shell4`, `BlockProperty::Shell { thickness }`, the refusal row R3), [ADR 0417](0417-f391-registration-six-unknowns-and-beam.md)
(F-391: six unknowns per node, Beam2, the beam-aware checks, `BeamSolidNode`), [ADR 0422](0422-f514-registration-beam-line-and-gravity-loads.md)
(beam loads, `Load::LineLoad` as the pattern of an element-set load), [ADR 0426](0426-f513-registration-beams-on-the-transformation.md)
(beams on the transformation path), [ADR 0423](0423-f512-registration-beam-results-in-solid-output.md) and
[ADR 0430](0430-f516-registration-beam-output-layout-refusal.md) (beam output in `rcfd-post`). Roadmap item F-392 (deps F-391).
Size M. Number 0432 given by the lead. Branch `f392-shell-element`, worktree `w49i-work/f392`, scratch `w49i-work/f392-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact); each registered run made once; defaults bit-identical; no edit of `baseline_hashes.rs`, of the hash
tables or of a golden file; no new dependency, no unsafe code; clean-room.

## What F-392 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. Membrane patch test and constant-curvature bending patch
> test on a distorted flat mesh within 1e-8. Simply supported square plate under uniform load, 16 x 16 elements, nu = 0.3: centre
> deflection within 1 percent of the Navier series (0.004062 q a^4/D, recomputed in the registration) at thickness/side 0.01 and
> within 1.5 percent at 0.001 (no shear locking), the fully integrated shear control being below 20 percent at 0.001. Cantilever
> strip, nu = 0: tip deflection P L^3/(3 E I) within 0.5 percent and surface stress 6 M/t^2 at mid-span within 2 percent.
> Curved: a ring strip of radius R pinched by two opposite forces, nu = 0, 32 elements per quarter: change of the loaded diameter
> (pi/4 - 2/pi) P R^3/(E I) within 2 percent; a cylinder under internal pressure with free ends: radial displacement p R^2/(E t)
> within 0.5 percent. Frequencies of the simply supported plate: omega_mn = pi^2 (m^2 + n^2)/a^2 sqrt(D/(rho t)) within 1
> percent for (1,1) and 3 percent for (1,2) and (2,2). A free plate has six zero frequencies; a facet warped beyond the
> registered limit is refused with the element.

Clauses: (a1) every limit not stated is registered here; (a2) membrane patch test, distorted flat mesh, 1e-8; (a3)
constant-curvature bending patch test, distorted flat mesh, 1e-8; (a4) simply supported plate, 16 x 16, nu = 0.3, centre
deflection within 1 percent of Navier at t/a = 0.01; (a5) the same within 1.5 percent at t/a = 0.001; (a6) the fully integrated
shear control below 20 percent at t/a = 0.001; (a7) the Navier coefficient recomputed here; (a8) cantilever strip, nu = 0, tip
deflection within 0.5 percent of `P L^3 / (3 E I)`; (a9) surface stress `6 M / t^2` at mid-span within 2 percent; (a10) pinched
ring strip, nu = 0, 32 elements per quarter, change of the loaded diameter within 2 percent of `(pi/4 - 2/pi) P R^3 / (E I)`;
(a11) cylinder under internal pressure, free ends, radial displacement within 0.5 percent of `p R^2 / (E t)`; (a12) simply
supported plate frequencies within 1 percent for (1,1), 3 percent for (1,2) and (2,2); (a13) a free plate has six zero
frequencies; (a14) a facet warped beyond the registered limit is refused naming the element. The brief adds: (b1) solid-only and
beam-only models give the same bits (`si_scaffold_bits.rs` and every existing test unedited, except retired scaffold rows); (b2)
paths that cannot handle shells refuse them with a located error; (b3) the drilling-stiffness treatment registered (it acts on
the free-plate zeros and on the ring); (b4) the ring closed form recomputed, noting that it is the change of the diameter under
the loads for a thin ring; (b5) shells in `rcfd-post` may be refused for now: registered.

Readings, registered:

- **(a2, a3) "within 1e-8":** the largest difference between the computed and the exact nodal unknowns (all six at every
  interior node), relative to the largest exact nodal value of the field, and the element resultants against the exact constant
  resultants relative to their largest component. The patch lies in a plane placed in space by a registered rotation, so the
  frame and the transformation are part of the test.
- **(a4, a5) "centre deflection within x percent of the Navier series":** `|w_c / (c q a^4 / D) - 1| <= x / 100` with `c` the
  Navier coefficient summed here (arithmetic 10), `D = E t^3 / (12 (1 - nu^2))`. The simple support is the "hard" one (registered,
  arithmetic 11): `w = 0` and the rotation about the edge normal zero on every edge, so the Mindlin solution tends to Kirchhoff's
  without a boundary layer.
- **(a6) "the fully integrated shear control being below 20 percent":** the same model with the displacement-based transverse
  shear integrated by the full 2 x 2 rule gives `w_c / (c q a^4 / D) < 0.2` at t/a = 0.001 (the control shows the locking that the
  assumed strain removes).
- **(a8, a9):** tip deflection the mean of the two tip nodes' deflection; `M = P L / 2` the bending moment at mid-span, the
  surface stress `sigma_xx` at the top (`z = +t/2`) and at the bottom of the element whose centre is at mid-span, against
  `-6 M / (b t^2)` and `+6 M / (b t^2)` (top in compression for a tip load along `+e_3`), `b` the strip width (`M / b` per width).
- **(a10) "change of the loaded diameter":** the shortening of the diameter through the two load points; arithmetic 13 derives
  the closed form (bending energy only, thin ring) and shows it is that quantity.
- **(a11):** the radial displacement of every node, against `p R^2 / (E t)` (free ends: no axial stress, so the hoop strain is
  `p R / (E t)` whatever `nu`).
- **(a12):** `omega_mn` with `D` of the plate; the computed angular frequencies `2 pi f`, sorted, are (1,1), then the double pair
  (1,2) and (2,1), then (2,2) (arithmetic 15); both members of the double pair are compared with the (1,2) value.
- **(a13) "six zero frequencies":** of the eight lowest frequencies of the unconstrained plate, `max_{i <= 6} |f_i| / f_7 <=
  1e-4` (the limit and its estimate in arithmetic 16), and `f_7 > 0`.
- **(a14):** the warp ratio of arithmetic 1 above `WARP_LIMIT = 0.05` refused on the static solve, the modal analysis and the
  stiffness assembly with `FemError::Element { element, source: InvalidValue { key: "warp", .. } }` naming that element.

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0295 (stub), ADR 0290 section on SI-5, ADR 0417 (whole), ADR 0426 and ADR 0430 (heads and
  the entry checks), ADR 0422 (the element-set load pattern).
- `crates/rcfd-fem/src`: `shell.rs` (placeholder), `beam.rs` (whole, the pattern), `model.rs` (`DofMap::for_mesh`, `Load`,
  `add_load`, `check_elements`, `check_supported*`), `assemble.rs` (whole), `mass.rs` (whole), `solve.rs` (entry of
  `solve_static_report`, the multigrid refusal), `modal.rs` (whole entry), `mpc.rs` (`check_transformation_model`), `loads.rs`
  (`nodal_load_vector`, `add_body_load_with`, `add_line_load`), `recover.rs` (`fit_basis`, `extrapolation_matrix`), `mesh.rs`
  (kinds, `BlockProperty`, `add_block`, `set_block_property`), `element.rs` (`element_for`), `lib.rs`; the `check_supported` call
  sites of the other paths (cyclic, harmonic, transient, rotating, buckling, geometric, nonlinear).
- `crates/rcfd-linalg/src/eigen.rs` (`Shift::Auto` rule, `ResidualTest`), `skyline.rs` (public API).
- `crates/rcfd-post/src/solid_output.rs` (`solid_fields` refuses a node with other than three components, naming it;
  `beam_fields` refuses a non-Beam2 block in a beam model, naming it).
- Tests: `si_scaffold.rs` (the rows naming ADR 0295), `si_geometric.rs` C8, `si_beam.rs` (the `Checks` helper).
- **Not read:** any finite-element code, manual or documentation outside this repository. Cited from memory, as in the stub:
  Dvorkin and Bathe, Eng. Comput. 1 (1984) 77, and Bathe and Dvorkin, Int. J. Numer. Meth. Engng 21 (1985) 367 (the MITC4
  assumed transverse shear strain); Hughes and Brezzi, Comput. Methods Appl. Mech. Engng 72 (1989) 105 (a drilling rotation tied
  to the in-plane rotation by a penalty); Timoshenko and Woinowsky-Krieger, Theory of Plates and Shells (Navier's series, plate
  frequencies); the shear factor 5/6 of a homogeneous plate (Reissner, Mindlin). Everything used is re-derived below; no number
  is taken from these sources.

## Scaffold rows retired (standing decision: ignored record citing this ADR plus a running `_after_shell_landed` copy)

`grep 0295` in `crates/rcfd-fem/tests` and `crates/rcfd-post/tests`; the running tests among the hits:

| file | test | row | why |
| --- | --- | --- | --- |
| `si_scaffold.rs` | `x1_the_scaffold_exists_after_beam_landed` | `shell::PENDING == Some("ADR 0295")`, `Shell4.pending() == Some("ADR 0295")` | both become `None` |
| `si_scaffold.rs` | `x3_placeholder_elements_refuse_after_beam_landed` | Shell4 refuses `stiffness`, `integration_points`, `strain_at` naming `ADR 0295` | the solid-element routines of Shell4 refuse naming `shell::SOLID_ROUTINES` (D7) |
| `si_scaffold.rs` | `r1_to_r9_static_solve_refuses_each_new_variant_after_beam_landed`, `r10_refused_on_every_static_path_after_beam_landed` | R3 (Shell4 on Hex8 nodes refused as a pending kind, `ADR 0295`) | that model (no Shell property, nodes shared with the box) is now refused by the shell model checks (D4; check S1 here) |

Each gets `#[ignore = "record of ... before ADR 0432 (F-392, Shell4) retired the ... row; superseded by the ..._after_shell_landed
copy below (maintainer standing decision, 2026-11-13)"]` and nothing else; each copy keeps every other row as it was and states
the new behaviour of the retired row. The ignored records of ADR 0291 that name ADR 0295 stay as they are. Not retired:
`si_geometric.rs` C8 (Shell4's `geometric_stiffness` still refuses with `NotYetSupported`, the `Pending` trait default),
`v1_new_data_are_validated` (validation of `BlockProperty::shell`, unchanged), `rcfd-post` O1 (the VTK mapping of Shell4 to
`VTK_QUAD`, unchanged).

## Arithmetic done before the code (disclosed)

1. **Facet, frame and warp.** Nodes `x_0..x_3` (counter-clockwise, ADR 0291 D2), centroid `c = mean x_i`, diagonals `d_1 = x_2 -
   x_0`, `d_2 = x_3 - x_1`, `n = d_1 x d_2`, `e_3 = n / |n|` (the element normal; for a planar convex quadrilateral `|n| / 2` is its
   area). The offsets `h_i = (x_i - c) . e_3` satisfy `h_0 = -h_1 = h_2 = -h_3` (both diagonals are normal to `e_3`, so `h_0 = h_2`
   and `h_1 = h_3`, and `sum h_i = 0`). The projected nodes `p_i = x_i - h_i e_3` lie in the mean plane. **Warp ratio** `w =
   |h_0| / sqrt(|n| / 2)`: the offset relative to the size; registered limit `WARP_LIMIT = 0.05` (a judgement, not tuned: at 5
   percent the facet's error from treating the warped element as flat is of the order of the ratio, a reasonable bound for a shell
   mesh; a blade or a housing meshed finer than that is accepted; it is not a published number). Refused (naming the element): `w
   > 0.05` (key `"warp"`), `|n| = 0` or non-finite (key `"geometry"`), a non-convex or inverted projected quadrilateral (the 2D
   Jacobian at a corner `<= 0`: `FemError::InvertedElement`). In-plane axes: `e_1 = (p_1 - p_0) / |p_1 - p_0|`, `e_2 = e_3 x e_1`;
   local coordinates `(X_i, Y_i) = ((p_i - c) . e_1, (p_i - c) . e_2)`. Example used by check W1: the unit square with node 2
   lifted by `delta`: `w = 0.0445` at `delta = 0.18`, `0.0540` at `delta = 0.22` (prototype).
2. **Membrane.** Bilinear `u, v` in the isoparametric map of `(X, Y)` on `[-1, 1]^2`, strains `(u_X, v_Y, u_Y + v_X)`, `N = D_m
   eps`, `D_m = E t / (1 - nu^2) [[1, nu, 0], [nu, 1, 0], [0, 0, (1 - nu) / 2]]`, 2 x 2 Gauss. A linear field is in the
   isoparametric space, so the membrane patch test holds exactly (up to rounding) on any convex quadrilateral mesh.
3. **Bending (Reissner-Mindlin).** A point at height `z` moves by `theta x z e_3 = z (theta_y, -theta_x, 0)` in local axes, so the
   section rotations are `beta_x = theta_y`, `beta_y = -theta_x`; bilinear `w, theta_x, theta_y`. Curvatures `kappa = (beta_x,X,
   beta_y,Y, beta_x,Y + beta_y,X)`, moments `m = D_b kappa`, `D_b = t^2 / 12 D_m`, 2 x 2 Gauss. Surface stresses `sigma(z) = N / t +
   12 z m / t^3`, at `z = +-t/2`: `N / t +- 6 m / t^2`. Transverse shear `gamma = grad w + beta`, `Q = k G t gamma`, `k = 5/6`
   (`SHEAR_FACTOR`), `G = E / (2 (1 + nu))`.
4. **The assumed transverse shear (MITC4, after Dvorkin and Bathe).** Covariant components `gamma_xi = J_xi . gamma` and
   `gamma_eta = J_eta . gamma` (`J_xi = (X_xi, Y_xi)`), that is `gamma_xi = w_xi + beta . J_xi`. They are sampled at the edge
   mid-points `A = (0, -1)`, `C = (0, 1)` (for `gamma_xi`) and `D = (-1, 0)`, `B = (1, 0)` (for `gamma_eta`) with the bilinear
   fields, and interpolated `gamma_xi(xi, eta) = (1 - eta) / 2 gamma_xi^A + (1 + eta) / 2 gamma_xi^C`, `gamma_eta(xi, eta) = (1 -
   xi) / 2 gamma_eta^D + (1 + xi) / 2 gamma_eta^B`; the Cartesian strain at a Gauss point is `J^{-1} (gamma_xi, gamma_eta)`;
   energy `int k G t gamma . gamma dA` by 2 x 2 Gauss. On an edge from node `i` to node `j`, `gamma^edge = ((w_j - w_i) + (beta_i
   + beta_j) / 2 . (x_j - x_i)) / 2`. **Patch test:** for `w` quadratic and `beta = -grad w` linear, the chord rule `w_j - w_i =
   grad w(mid) . (x_j - x_i)` is exact for a quadratic and `(beta_i + beta_j) / 2 = beta(mid)` for a linear field, so every tying
   strain vanishes; the curvature of the linear `beta` is constant and exact. So the constant-curvature state has zero shear
   energy, constant moments, and balanced interior nodes on any convex mesh (prototype, distorted patch in space: `3.6e-13`,
   `estimate2.log`). **Thin limit:** along an element the tying gives one shear constraint per edge (four per element) instead
   of the eight Gauss-point constraints of the full rule, which is what removes the locking (prototype: the SS plate at t/a =
   0.001 gives `0.998740` of Navier with MITC4 and `1.770e-3` with the full rule, `estimate3.log`).
5. **Drilling rotation (registered treatment).** The flat element has no stiffness for `theta_z` (the rotation about `e_3`). Taken
   (option (a) below, after Hughes and Brezzi, from memory, re-derived): the penalty `int alpha G t (theta_z - omega)^2 dA`,
   `omega = (v_X - u_Y) / 2` the in-plane rotation of the membrane field, `theta_z` bilinear, 2 x 2 Gauss, **`alpha = 1e-3`**
   (`DRILLING_FACTOR`, a small value so that the penalty does not stiffen the membrane: a registered choice, not searched). It is
   zero for every rigid motion (`theta_z = omega` = the rotation about `e_3`) and for every linear membrane field with `theta_z =
   omega` (so the membrane patch test is unaffected), and it ties `theta_z` to the membrane, so a flat free plate has exactly six
   zero-energy modes (prototype: one element with `alpha = 1e-3` has six eigenvalues below `1.5e-16` of the largest and the
   seventh at `2.4e-6`; with `alpha = 0` a flat mesh has one extra zero per node, `estimate.log`, `estimate3.log`). In the ring
   and the cylinder the nodal rotations are about the cylinder axis, which lies in every facet's plane, and the membrane rotation
   `omega` is zero: the penalty does not act there. Options considered: (a) this penalty; (b) a spring on `theta_z` minus its
   element mean (zero for a uniform `theta_z` with no motion: a spurious seventh zero on a flat free plate, rejected); (c) an
   Allman-type membrane with drilling unknowns (more code, not needed by any check).
6. **Mass.** The kinetic energy of the same interpolation: `rho t int N_i N_j dA` on each translation and `rho t^3 / 12 int N_i
   N_j dA` on `theta_x, theta_y` (rotary inertia of the section) and, registered, on `theta_z` as well (no physical inertia; a
   positive value keeps the mass matrix definite, which the modal path requires: it refuses a free unknown without mass), 2 x 2
   Gauss (exact: the integrand is at most bicubic on a convex quadrilateral). For a flat element the rigid-motion kinetic energy
   is therefore exact except about the normal axis, where the `theta_z` term adds `rho t^3 A / 12` (check M1).
7. **Transformation and rigid offsets.** Element unknowns in global axes `(U_i, Theta_i)`; the projected point moves by `u(p_i) =
   U_i + Theta_i x r_i`, `r_i = p_i - x_i = -h_i e_3`, so with `Lambda` the rotation with rows `e_1, e_2, e_3` the local unknowns
   are `u_l = Lambda (U - r x Theta)`, `theta_l = Lambda Theta`: `T_i = [[Lambda, -Lambda [r_i]_x], [0, Lambda]]`, `K = T^T K_l T`,
   `M = T^T M_l T`, `f = T^T f_l`. A rigid motion of the nodes is a rigid motion of the projected points, and those lie in the
   null space of `K_l`, so `K r = 0` for every rigid motion of a warped facet (prototype: `3.0e-16` of `max |K| max |r|` for the
   lifted square at `delta = 0.18`). For a flat facet `T` is the plain rotation.
8. **Pressure.** `Load::ShellPressure { set, pressure }` (registered, the pattern of `Load::LineLoad`): a uniform pressure `p`
   on every element of an element set of Shell4 elements, acting along the element normal `e_3` (positive `p` pushes the surface
   towards `+e_3`); consistent local load `f_{w,i} = p int N_i dA` (2 x 2 Gauss, exact), then `T^T`. On a warped facet the offset
   gives nodal moments `r_i x F_i` as well.
9. **Resultants and surface stresses** (`shell::stresses`): per element at its centre `xi = eta = 0`, in its local axes: `N`,
   `m`, the assumed-strain `Q`, and `sigma` at `z = +t/2` (top, the `+e_3` side) and `z = -t/2`. For the cantilever the moment of
   the linear `theta_y` is constant along the element and equals the exact `M` at the centre (arithmetic 12).
10. **Navier coefficient (a7).** Simply supported Kirchhoff square plate under uniform `q`: `w(x, y) = (16 q / (pi^6 D)) sum_{m, n
    odd} sin(m pi x / a) sin(n pi y / a) / (m n ((m / a)^2 + (n / a)^2)^2)`; at the centre `w_c = c q a^4 / D`, `c = (16 / pi^6)
    sum_{m, n odd} (-1)^{(m + n) / 2 - 1} / (m n (m^2 + n^2)^2)`. Summed (`estimate3.log`): 9 terms per direction `0.004062468`,
    49 `0.0040623527`, 199 and 799 `0.004062352661`. So `c = 0.00406235266`, the acceptance's `0.004062` to its four digits.
    The test sums the series to `m, n <= 799` itself (check SS4).
11. **Plate estimates.** Mindlin shear adds about `w_s / w_K = (0.0737 / 0.004062) D / (k G t a^2) = 5.2 (t/a)^2` (the centre
    value of the Marcus moment `0.0737 q a^2`, from the membrane analogy, estimated): `5.2e-4` at 0.01, `5e-6` at 0.001. MITC4
    on 16 x 16 with consistent loading (prototype, rcfd data `E = 2e11`, `q = 1000 Pa`): `0.9992544947` at t/a = 0.01 (`-7.46e-4`)
    and `0.9987399464` at 0.001 (`-1.26e-3`); full-integration control at 0.001: `1.7700823e-3`. Constraints: at every node `u =
    v = 0` and `theta_z = 0` (the membrane problem of a flat plate under a normal load is decoupled and its solution is zero, so
    fixing it is exact; it also removes in-plane modes from the modal case), `w = 0` on the four edges, `theta_x = 0` on `x = 0,
    a`, `theta_y = 0` on `y = 0, a` (hard support).
12. **Cantilever strip.** `L = 1 m`, `b = 0.1 m`, `t = 0.01 m`, `E = 2e11`, `nu = 0`, `N = 15` elements along and 1 across
    (odd, so the mid-span is an element centre), clamped (all six unknowns) at `x = 0`, `P = 1 N` along `+z` split on the two tip
    nodes. In cylindrical bending the element is the linear Timoshenko element with the shear sampled at the mid-point: the
    curvature is constant per element and equals `M(x_c) / (E I)` (the mid-point rule is exact for the linear `M`), so the nodal
    slopes are exact and the deflection is their trapezoid integral, which for the concave quadratic slope falls short by `L h^2
    P / (12 E I)`: relative `-1 / (4 N^2) = -1.11e-3`, plus the shear share `3 E I / (k G A L^2) = 0.6 (t/L)^2 = 6e-5`; prototype
    `0.9989488888` (`-1.05e-3`). Mid-span top stress: prototype `0.9999999999` of `-6 M / (b t^2)`.
13. **Pinched ring (a10, b4).** Thin ring, radius `R`, loads `P` inward at the top and bottom. By symmetry the quarter `phi in [0,
    pi/2]` (from the side point at `phi = 0` to the load point) carries `P / 2` at its top and an unknown moment `M_0` at `phi = 0`,
    where the section does not rotate. Statics: `M(phi) = M_0 + (P / 2) R cos phi` up to the constant; zero rotation at `phi = 0`
    relative to `phi = pi/2` (`int M dphi = 0`) gives `M(phi) = (P R / 2)(cos phi - 2 / pi)`. Castigliano with the bending energy
    of the whole ring, `U = 4 int_0^{pi/2} M^2 R dphi / (2 E I) = P^2 R^3 / (2 E I) (pi/4 - 2/pi)` (`int (cos phi - 2/pi)^2 dphi =
    pi/4 - 4/pi + 2/pi`), gives the displacement of the load `P` along its line of action: `dU / dP = (pi/4 - 2/pi) P R^3 / (E I)
    = 0.14878 P R^3 / (E I)`; since the two loads are opposite and equal, `dU / dP` is the relative approach of the two load points,
    the **shortening of the loaded diameter** (quadrature check `0.1487783910`, `estimate3.log`). Axial and shear energy are left
    out: thin ring, relative `O((t/R)^2)`. Registered model: `R = 1 m`, `b = 0.05 m`, `t = 0.01 m`, `E = 2e11`, `nu = 0`, `P = 1 N`,
    `I = b t^3 / 12`, the quarter of 32 elements (nodes at `phi_i = i pi / 64`, `z = 0` and `z = b`), symmetry at `phi = 0` (`u_y =
    0`, `theta_z = 0`) and at `phi = pi/2` (`u_x = 0`, `theta_z = 0`), `u_z = theta_x = theta_y = 0` at every node (the in-plane
    problem with `nu = 0` has these zero exactly), `-P / 4` along `y` at each of the two top nodes (the quarter's half of `P`); the
    change of the diameter is twice the mean `-u_y` of the top nodes. Prototype `0.9985882399` (`-1.41e-3`: chords and the
    element's `1 / (4 N^2)`-type error).
14. **Cylinder (a11).** Free ends: axial resultant zero, hoop resultant `p R`, hoop strain `p R / (E t)`, `u_r = p R^2 / (E t)`.
    The faceted model is the inscribed polygon: each node gets `p c cos(dphi / 2)` radially (`c = 2 R sin(dphi / 2)` the chord), the
    chord tension is `p R cos(dphi / 2)`, so `u_r = cos(dphi / 2) p R^2 / (E t)`; facet bending between the nodes moves no node
    (the nodal rotations are zero by symmetry). With 32 elements per quarter `cos(pi / 128) = 0.999698818696` (`-3.01e-4`);
    prototype `0.999698818629` to `0.999698818768` over all nodes. Registered model: `R = 1 m`, half-length `0.5 m` (4 elements),
    `t = 0.01 m`, `E = 2e11`, `nu = 0.3`, `p = 1e5 Pa`, the quarter `phi in [0, pi/2]` with symmetry at `y = 0` (`u_y = theta_x =
    theta_z = 0`), at `x = 0` (`u_x = theta_y = theta_z = 0`) and at `z = 0` (`u_z = theta_x = theta_y = 0`), the end `z = 0.5`
    free, node order `(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)` (angle index `i`, axial index `j`), whose normal points
    outward (`(T + Z) x (-T + Z) = 2 T x Z`, radial), `Load::ShellPressure` of `+p`.
15. **Plate frequencies (a12).** Data of arithmetic 11 at t/a = 0.01 with `rho = 7850`; `omega_mn` with `m^2 + n^2 = 2, 5, 5, 8`.
    Prototype (dense, the same element and mass): `48.1756947, 121.7454596, 121.7454596, 194.9982871 Hz`, relative to `omega_mn /
    (2 pi)`: `+3.94e-3, +1.483e-2, +1.483e-2, +1.590e-2` (the consistent mass of the bilinear element is the dominant error; Mindlin
    lowers by about `3e-3` at (2,2)). Largest eigenvalue `1.0e12` (thickness-shear), against `lambda_1 = 9.2e4`: the default
    relative residual cannot be met (ADR 0284 amendment 1), so the modal options are registered as for the beam (ADR 0417
    amendment 1): `ModalOptions::new(4).tolerance(1e-12).residual_test(ResidualTest::Backward)`.
16. **Free plate (a13).** `1 m x 0.8 m`, 8 x 8 elements, `t = 0.01 m`, steel (`E = 2e11`, `nu = 0.3`, `rho = 7850`), no constraint,
    `ModalOptions::new(8).tolerance(1e-12).residual_test(ResidualTest::Backward)` (default shift: `Shift::Auto`). The six rigid
    eigenvalues are zero up to the rounding of the factorization of `K + sigma M`: at most about `eps lambda_max` (`lambda_max =
    9.9e11`), so `|f_i| / f_7 <~ sqrt(eps lambda_max / lambda_7) = 5.7e-5` with `f_7 = 41.127 Hz`; the dense shift-invert
    emulation of the prototype (the shift rule of `Shift::Auto`, `sigma = 7.5e5`) gives `3.3e-6`. Limit **`1e-4`**. Prototype `f_7
    = 41.1269190 Hz`, `f_8 = 52.7170891 Hz`. With `alpha = 0` the eight lowest are all below `1.4e-4 Hz` (negative control N1). A
    **warped** variant (stronger, the rigid offsets): every node lifted by `0.002 sin(1.7 k + 0.3) m` (`k` the node index in the
    grid order, rows of constant `y`), largest warp ratio `0.0132`; prototype ratio `1.45e-6`, `f_7 = 41.1488073 Hz`.
17. **Patch tests (a2, a3).** The unit square divided into 3 x 3 elements, nodes in rows of constant `Y` (`k = 4 j + i`), the four
    interior nodes moved to `(0.28, 0.40)`, `(0.71, 0.30)`, `(0.36, 0.62)`, `(0.62, 0.74)`; the plane placed in space by `x =
    (0.3, -0.2, 0.5) + Q (X, Y, 0)`, `Q` the rotation of 0.7 rad about `(1, -2, 2) / 3`; `t = 0.05`, `E = 2e11`, `nu = 0.3`. Membrane
    field `u = 1e-3 + 2e-3 X - 1.5e-3 Y`, `v = -5e-4 + 1e-3 X + 2.5e-3 Y`, `w = 0`, local `theta = (0, 0, omega)`, `omega = (1e-3 -
    (-1.5e-3)) / 2`; bending field `w = -(2e-3 X^2 - 1e-3 Y^2 + 1.5e-3 X Y) / 2 + 1e-4 + 2e-4 X`, `beta = -grad w`, `u = v = 0`,
    `theta_z = 0`; translations and rotations mapped to global axes by `Q`. Every unknown of the twelve boundary nodes prescribed to
    the exact field, no load; the interior unknowns compared. Prototype: membrane `2.1e-11`, bending `3.6e-13`.

## Design

### D1. `DofMap`

`DofMap::for_mesh(mesh)`: if no block is Beam2 or Shell4, `DofMap::uniform(n, 3)` (the former body, so the former bits);
otherwise `3` per node and `6` at each node of a Beam2 or Shell4 element. A beam-only mesh gets what it got before.

### D2. The element (`shell.rs`)

`shell::PENDING = None`. Public: constants `WARP_LIMIT = 0.05`, `DRILLING_FACTOR = 1e-3`, `SHEAR_FACTOR = 5/6`, `FOLLOW_UP = "a
follow-up of ADR 0432"`, `SOLID_ROUTINES = "a follow-up of ADR 0432 (the solid-element routines of Shell4; its matrices are in
rcfd_fem::shell)"`; `Facet` and `facet(coords)` (arithmetic 1: frame, local coordinates, offsets, warp ratio, refusals);
`TransverseShear { AssumedStrain, FullIntegration }` and `ShellOptions { shear, drilling }` (default: `AssumedStrain`,
`DRILLING_FACTOR`; the other values exist for the controls only and are not reachable from a model); `local_stiffness`,
`element_stiffness`, `element_stiffness_with`, `element_mass`, `pressure_load_vector` (arithmetic 2 to 8); `ShellStresses` and
`stresses(model, displacement)` (arithmetic 9); `shell_thickness(mesh, element)`; `check_shell_model(model)` (D4).

### D3. Which paths take shells

Shells are taken by `assemble::stiffness` / `stiffness_with`, `mass::consistent` / `consistent_with`, `solve::solve_static*` on
the elimination path (every solver except the multigrid preconditioner) and `modal::modal_analysis*` on the elimination path.
New `SolidModel::check_elements_supported_with_structures` / `check_supported_with_structures` (beams and shells, with
`beam::check_beam_model` and `shell::check_shell_model`) are called by those four entry points instead of the `_with_beams`
forms. `check_elements_supported`, `check_supported` (every other path) and the `_with_beams` forms (the transformation path of
ADR 0426) refuse a Shell4 block with `NotYetSupported { item: "element kind", variant: "Shell4", location: "block b", package:
shell::FOLLOW_UP }`. The multigrid refusal of `solve_static_report` names Shell4 too. Gravity, the centrifugal and nodal body
loads, the thermal load and `recover` reach Shell4 through the solid-element routines and are refused there (D7). The surface
loads `Traction` and `Pressure` need surface sets, which Shell4 has none of (no faces). `rcfd-post` is not changed: a shell model
is refused by `solid_fields` (a node with six components, named) and, mixed with beams, by `beam_fields` (a Shell4 block in a
beam model, named) — registered as the shell output's limit L1. For a model without Shell4 every check gives what it gave before.

### D4. Shell model checks (`check_shell_model`)

In order: every Shell4 block has a `BlockProperty::Shell` (else `InvalidValue { key: "property" }` naming the block); every node
of a Shell4 element that is also a node of a solid element is refused, the first in element and node order, with `Element {
element: shell, source: InvalidValue { key: "node", reason: "node N of the shell element is also a node of the solid element S
..." } }` (a solid has no rotational stiffness; a joint needs the transformation, a follow-up); every Shell4 element's facet
(arithmetic 1) is valid, refused as `Element { element, source }` (warp, geometry) or `InvertedElement`.

### D5. `Load::ShellPressure`

`Load::shell_pressure(set, p)`; `add_load` refuses an unknown element set, a non-finite value and an element of the set that is
not Shell4 (`InvalidValue { key: "shell pressure" }` naming it). `nodal_load_vector` adds `pressure_load_vector` of each element
(forces and moments). A model without it is unchanged.

### D6. Assembly dispatch

`stiffness_with` and `consistent_with` call `shell::model_element_stiffness` / `model_element_mass` for a Shell4 element (its
block's thickness, its material) and the former expressions for every other kind, in the same order: the same bits for the
others.

### D7. The solid-element routines of Shell4

`shell::element()` stays a `Pending` whose package is `shell::SOLID_ROUTINES`; `reference_nodes`, `faces`, `shape_gradients` stay
empty (the kind keeps zero faces, so no surface set can name a shell face).

## Checks (limits fixed now)

| id | clause | check | limit | estimate behind it |
| --- | --- | --- | --- | --- |
| D1 | b1 | unknown counts: solid-only mesh `== DofMap::uniform(n, 3)`; shell-only mesh 6 everywhere; a Hex8 box with a separate shell: 3 at the box nodes, 6 at the shell nodes | exact | D1 |
| B1 | b1 | `si_scaffold_bits.rs` passes unedited | pass | D1, D3, D6 |
| B2 | b1 | every existing `rcfd-fem` test passes, unedited except the retired rows (ignored) whose copies pass; `rcfd-post` tests pass | pass | the retired-rows table |
| B3 | b1 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | no fluid file touched |
| PT1 | a2 | membrane patch: interior unknowns against the exact field, relative to `max |exact|` | `1e-8` | arithmetic 17: `2.1e-11` |
| PT2 | a2 | membrane patch: `N` of every element against `D_m eps`, relative to `max |N|`; `m`, `Q` relative to the same | `1e-8` | rounding |
| PT3 | a3 | bending patch: interior unknowns against the exact field | `1e-8` | arithmetic 17: `3.6e-13` |
| PT4 | a3 | bending patch: `m` of every element against `D_b kappa` relative to `max |m|`; `Q` and `N` times the patch side relative to `max |m|` | `1e-8` | arithmetic 4 |
| SS1 | a4 | SS plate t/a = 0.01: `|w_c / (c q a^4 / D) - 1|` | `1e-2` | arithmetic 11: `7.46e-4` |
| SS2 | a5 | the same at t/a = 0.001 | `1.5e-2` | `1.26e-3` |
| SS3 | a6 | full-integration control at t/a = 0.001: `w_c / (c q a^4 / D)` | `< 0.2` | `1.77e-3` |
| SS4 | a7 | the Navier coefficient summed in the test (`m, n <= 799`) against `0.004062`: `|c - 0.004062|` | `<= 5e-7` | arithmetic 10: `3.5e-7` |
| CT1 | a8 | cantilever tip deflection against `P L^3 / (3 E I)` | `5e-3` | arithmetic 12: `1.05e-3` |
| CT2 | a9 | mid-span top and bottom `sigma_xx` against `-+6 M / (b t^2)` | `2e-2` | `1e-10` |
| RG1 | a10 | ring: diameter change against `(pi/4 - 2/pi) P R^3 / (E I)` | `2e-2` | arithmetic 13: `1.41e-3` |
| CY1 | a11 | cylinder: `max_nodes |u_r / (p R^2 / (E t)) - 1|` | `5e-3` | arithmetic 14: `3.01e-4` |
| FQ1 | a12 | (1,1) against `omega_11` | `1e-2` | arithmetic 15: `3.94e-3` |
| FQ2 | a12 | the two (1,2)/(2,1) frequencies against `omega_12`; (2,2) against `omega_22` | `3e-2` each | `1.48e-2`, `1.59e-2` |
| FR1 | a13 | free plate: `max_{i <= 6} |f_i| / f_7`; `f_7 > 0` | `1e-4` | arithmetic 16: `3.3e-6` (bound `5.7e-5`) |
| W1 | a14 | a two-element strip whose element 1 is the unit square lifted at a corner by `0.22` (warp `0.054`): `solve_static`, `modal_analysis`, `stiffness` refused with `Element { element: 1, InvalidValue { key: "warp" } }`, the message naming the ratio and the limit | exact | arithmetic 1 |
| W2 | a14 | the same lifted by `0.18` (warp `0.0445`) solves | pass | arithmetic 1 |
| S1 | b2 | a Shell4 element with a Shell property on four nodes of a Hex8 box: `solve_static`, `modal_analysis`, `stiffness` refused with `Element { element: shell, InvalidValue { key: "node" } }` naming the first shared node | exact | D4 |
| N2 | b2 | a valid shell model on the other paths: with a nodal spring (the transformation), `nonlinear::initial_states`, `solve_static` by smoothed-aggregation CG, `check_supported()` (the guard of cyclic, harmonic, transient, rotating, buckling): `NotYetSupported` naming Shell4 and `shell::FOLLOW_UP`; with `Load::gravity`: an `Element` error whose source names `shell::SOLID_ROUTINES`; `rcfd_post::solid_output::solid_vtu_string` of its result: refused naming node 0 and its six components | exact | D3 |
| N3 | (data) | Shell4 block without property refused (`property`); `Load::shell_pressure` on a Hex8 element set refused (`shell pressure`); a non-convex facet refused (`InvertedElement`) | exact | D4, D5 |
| K1 | (element) | the lifted square at `0.18` (warped, within the limit) and a seeded-free flat facet in space: `K r` for the six rigid motions, relative to `max |K| max |r|`; `K` symmetric bit for bit | `1e-12`; exact | arithmetic 7: `3.0e-16` |
| M1 | (mass) | `mass_properties` of one flat element `[0, 0.6] x [0, 0.4]` placed in space by `Q` (rho 7850, t 0.01): mass `rho t A`; centre the centroid; inertia about the centre `Q diag(rho t I_X + rho t^3 A / 12, rho t I_Y + rho t^3 A / 12, rho t (I_X + I_Y) + rho t^3 A / 12) Q^T` (`I_X = int Y^2 dA`) | `1e-12` (centre to the diagonal, inertia to its largest entry) | arithmetic 6 |

Stronger checks (the rcfd element against the independent numpy prototype of the same equations, `estimate3.log`):

| id | check | limit | prototype |
| --- | --- | --- | --- |
| P1 | SS plate ratios at 0.01 and 0.001; the full-integration control | `1e-5` relative each | `0.9992544947`, `0.9987399464`, `1.7700823307e-3` |
| P2 | cantilever tip ratio; mid-span stress ratio | `1e-5` | `0.9989488888`, `0.9999999999` |
| P3 | ring ratio | `1e-5` | `0.9985882399` |
| P4 | cylinder: every node against `cos(pi / 128) p R^2 / (E t)` (the polygon closed form) | `1e-6` | `7e-11` |
| P5 | SS plate frequencies | `1e-5` relative each | `48.1756947203, 121.7454596068, 121.7454596145, 194.998287129 Hz` |
| P6 | free plate `f_7`, `f_8` | `1e-5` | `41.1269190351`, `52.7170891366 Hz` |
| FR2 | warped free plate (arithmetic 16): `max_{i <= 6} |f_i| / f_7`; `f_7` against the prototype | `1e-4`; `1e-5` | `1.45e-6`; `41.1488072747 Hz` |

Negative controls:

| id | control | expected | limit |
| --- | --- | --- | --- |
| N1 | free plate with `ShellOptions { drilling: 0 }` (assembled through `assemble_with`, eigenpairs by `lowest_eigenpairs` with the options of FR1) | more than six zeros: `|f_7| / f_7^(FR1) <= 1e-4` | arithmetic 16: all eight below `1.4e-4 Hz` |
| N4 | the SS plate t/a = 0.001 with the MITC4 shear (SS2) against the control (SS3) | the control is locked, the element not: `SS2 ratio > 0.98`, `SS3 ratio < 0.2` | as SS2, SS3 |

## Planned mutations (on a scratch copy, one at a time)

| id | mutation | expected to be caught by |
| --- | --- | --- |
| MU1 | the assumed strain replaced by the full integration in the default | SS2, P1 |
| MU2 | the tying points of `gamma_xi` and `gamma_eta` exchanged | PT3 or PT4, SS1, P1 |
| MU3 | `DRILLING_FACTOR = 0` | FR1 (or the modal solve fails), P6 |
| MU4 | the drilling penalty without `omega` (a spring on `theta_z` alone) | PT1, FR1 |
| MU5 | the rigid offsets removed (`T` without the `[r]_x` block) | K1 (warped), FR2 |
| MU6 | `beta_y = +theta_x` | PT3, SS1, CT1 |
| MU7 | rotary inertia `rho t^3 / 12` halved | M1, P5 |
| MU8 | `DofMap::for_mesh` gives three unknowns to shell nodes | D1 (and every shell check) |
| MU9 | the shell-solid node check removed | S1 |
| MU10 | the warp check removed | W1 |
| MU11 | `ShellPressure` along `-e_3` | SS1, CY1 |
| MU12 | `SHEAR_FACTOR = 1` | P1 (t/a = 0.01: the shear share changes by `9e-5`) |
| MU13 | `check_elements` accepts Shell4 on the other paths | N2 |

## Decision rule

- **done:** every check of the acceptance (D1, B1 to B3, PT1 to PT4, SS1 to SS4, CT1, CT2, RG1, CY1, FQ1, FQ2, FR1, W1, W2, S1,
  N2) met, the stronger checks and the controls as stated, every mutation caught by a registered check.
- **partial:** a clause missed (recorded as a `*_known_miss`) or not run, a mutation not caught, or a check met only after an
  amendment written after its run.
- **todo:** the element does not build.

## Limits (not covered)

- L1. Output: `rcfd_post::solid_output` refuses a shell model (D3); writing Shell4 as `VTK_QUAD` cells with rotations and the
  resultants as cell data is a follow-up.
- L2. Shells on the transformation path (rigid sets, linear constraints, supports), cyclic, harmonic, transient, rotating,
  buckling, geometric-stiffness and nonlinear paths and with the multigrid preconditioner are refused; gravity, centrifugal,
  body and thermal loads on shells are refused; no geometric stiffness (so no Campbell diagram of a shell blade, ADR 0290 section
  1.3). Shell-to-solid joints are a follow-up.
- L3. Homogeneous isotropic shell, constant thickness per block, mid-surface at the nodes (no offset), no triangle, no
  composite, stresses at the element centre only. The drilling stiffness is a fiction (arithmetic 5); in-plane bending of a single
  facet carries the parasitic shear of the bilinear membrane (not checked here). Warped facets within the limit are checked by the
  rigid motions (K1) and the free warped plate (FR2) only, not by an accuracy test.
- L4. Shell and beam elements may share nodes (both have six unknowns); not checked by a registered test.

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. The drilling treatment (arithmetic 5): option (a), the penalty tying `theta_z` to the membrane rotation, `alpha = 1e-3`.
2. Rotary inertia on `theta_z` equal to that on `theta_x, theta_y` (arithmetic 6).
3. The warp limit `0.05` of the offset-to-size ratio (arithmetic 1).
4. The pressure on shells as a new `Load::ShellPressure` on element sets (D5) rather than surface sets on shell faces.
5. A shell node shared with a solid refused (D4) with a located `InvalidValue`, not a new error variant.
6. The shell output in `rcfd-post` refused for now (L1; the brief allows it).

## Amendment 1 (2026-11-14, written before any run of a check)

1. **Negative control N1, set-up corrected before its run.** `lowest_eigenpairs` verifies its result by a Sturm count above the
   last converged cluster (`eigen.rs`, read, not run): with `alpha = 0` the flat free plate has `6 + 81` zero eigenvalues (one
   extra per node), more than its subspace of 16 vectors holds, so it would refuse with `SturmMismatch` instead of returning
   eight frequencies. N1 is therefore measured by the Sturm count itself: `rcfd_linalg::eigen::count_below(K_0, M, lambda_7 / 4,
   &SkylineOrdering::Rcm)` with `K_0` the free plate's stiffness assembled through `assemble_with` with `ShellOptions { drilling:
   0 }`, `M = mass::consistent`, and `lambda_7` the seventh eigenvalue of FR1: expected `87`, registered limit `> 6`. Added as a
   stronger check of FR1 (**FR3**): the same count with the element's `K` (`assemble::stiffness`) is exactly `6`.
2. **The full-integration control SS3, parameters not stated above:** `K` by `assemble_with` with `element_stiffness_with(...,
   ShellOptions::default().shear(TransverseShear::FullIntegration))`, `f = loads::nodal_load_vector`, the prescribed equations of
   the model by `constraint::prescribed_equations`, `constraint::reduce`, and `SkylineLdlt::factor` / `solve` of `rcfd-linalg`.
   No limit changes.

## Addendum 1: results (2026-11-14)

Code: `crates/rcfd-fem/src/shell.rs` (the element, D2, D4, D7), `model.rs` (`DofMap::for_mesh`, `Load::ShellPressure`, the
`_with_structures` checks and the Shell4 refusal of the other paths), `assemble.rs` and `mass.rs` (dispatch, D6), `loads.rs`
(`add_shell_pressure`), `solve.rs` and `modal.rs` (the entry checks, the multigrid refusal naming Shell4); new tests
`crates/rcfd-fem/tests/si_shell.rs` (11 tests, 0.08 s in release) and `crates/rcfd-post/tests/si_shell_output.rs` (the
`rcfd-post` row of N2); the retired rows of `si_scaffold.rs` as ignored records with `_after_shell_landed` copies. The registered
run was made once (`f392-scratch/run1.log`, `run1-post.log`) on the test commit `e4550e7e`, release profile, `--test-threads=1`.
After it `cargo fmt -p rcfd-fem` reformatted `shell.rs` and `si_shell.rs` (layout only, commit `1ad466d6`); `si_shell` and
`si_scaffold` were run again on that commit to confirm they pass (not a registered run).

| id | check | limit | measured | met |
| --- | --- | --- | --- | --- |
| D1 | unknown counts: solid-only, shell-only, Hex8 box with a separate shell | exact | uniform 3; 54 = 6 x 9; 60 = 3 x 12 + 24 | yes |
| B1 | `si_scaffold_bits` unedited | pass | pass | yes |
| B2 | every existing `rcfd-fem` and `rcfd-post` test | pass | `rcfd-fem`: 356 passed, 0 failed, 75 ignored (the four retired rows among them); `rcfd-post`: 274 passed, 0 failed, 30 ignored | yes |
| B3 | `baseline_hashes` (`rcfd-api`) | pass | 3 passed, 1 ignored (as before) | yes |
| PT1 | membrane patch, interior unknowns | `1e-8` | `7.4e-14` | yes |
| PT2 | membrane patch, `N`; `m`, `Q` | `1e-8` | `6.4e-16`; `5.5e-16` | yes |
| PT3 | bending patch, interior unknowns | `1e-8` | `1.0e-13` | yes |
| PT4 | bending patch, `m`; `Q`, `N` | `1e-8` | `2.3e-14`; `1.6e-12` | yes |
| SS1 | SS plate t/a = 0.01 against Navier | `1e-2` | `7.46e-4` (ratio `0.9992544947`) | yes |
| SS2 | SS plate t/a = 0.001 | `1.5e-2` | `1.26e-3` (ratio `0.9987399470`) | yes |
| SS3 | full-integration control t/a = 0.001 | `< 0.2` | `1.770e-3` | yes |
| SS4 | Navier coefficient `|c - 0.004062|` | `5e-7` | `3.53e-7` (`c = 0.004062352661`) | yes |
| CT1 | cantilever tip | `5e-3` | `1.05e-3` | yes |
| CT2 | mid-span top / bottom stress | `2e-2` | `1.2e-10` / `1.2e-10` | yes |
| RG1 | pinched ring diameter change | `2e-2` | `1.41e-3` | yes |
| CY1 | cylinder `u_r`, every node | `5e-3` | `3.01e-4` | yes |
| FQ1 | (1,1) | `1e-2` | `3.94e-3` | yes |
| FQ2 | (1,2), (2,1); (2,2) | `3e-2` | `1.48e-2`, `1.48e-2`; `1.59e-2` | yes |
| FR1 | free plate `max |f_1..6| / f_7`; `f_7 > 0` | `1e-4` | `2.5e-6`; `41.127 Hz` | yes |
| FR2 | warped free plate; `f_7` against the prototype | `1e-4`; `1e-5` | `1.3e-6`; `1.3e-12` | yes |
| FR3 | Sturm count below `lambda_7 / 4` | `6` | `6` | yes |
| W1 | warp `0.054` refused on static, modal, stiffness naming element 1, the ratio and the limit | exact | `element 1: key 'warp': ... = 5.4022e-2 above the limit 0.05` (all three) | yes |
| W2 | warp `0.0445` solves | pass | pass | yes |
| S1 | shell on Hex8 nodes refused naming the node | exact | `element 2: key 'node': node 2 of the shell element 2 is also a node of the solid element 1 ...` (all three) | yes |
| N2 | spring (transformation), nonlinear states, multigrid, `check_supported`, gravity, `rcfd-post` output | exact | `NotYetSupported { Shell4, shell::FOLLOW_UP }` (locations `block 0`, `block 0, solver cg-amg-rbm`); gravity `Element { 0, NotYetSupported { SOLID_ROUTINES } }`; `rcfd-post`: `node 0 has 6 components; the solid output writes 3` | yes |
| N3 | no property; shell pressure on Hex8; non-convex facet | exact | `property`; `shell pressure`; `InvertedElement { element: 0, local_node: 2 }` | yes |
| K1 | rigid motions, warped / flat in space; symmetry | `1e-12`; exact | `3.6e-16` / `4.0e-16`; bit for bit | yes |
| M1 | mass, centre, inertia | `1e-12` | `5.6e-16`, `3.4e-16`, `3.3e-16` | yes |
| P1 | SS plate 0.01, 0.001, control against the prototype | `1e-5` | `2.6e-11`, `6.1e-10`, `5.1e-12` | yes |
| P2 | cantilever tip, stress | `1e-5` | `5.7e-11`, `1.7e-11` | yes |
| P3 | ring | `1e-5` | `1.8e-10` | yes |
| P4 | cylinder against `cos(pi/128)` | `1e-6` | `1.1e-10` | yes |
| P5 | four frequencies | `1e-5` | `3.8e-10`, `7.5e-11`, `1.2e-11`, `7.0e-12` | yes |
| P6 | free plate `f_7`, `f_8` | `1e-5` | `2.4e-12`, `4.5e-13` | yes |
| N1 | `alpha = 0`: Sturm count below `lambda_7 / 4` | `> 6` | `87` (= 6 + 81, as estimated) | yes |
| N4 | element not locked, control locked | `> 0.98`, `< 0.2` | `0.998740`, `1.770e-3` | yes |

Mutations (scratch copy `f392-scratch/mut` from `git archive` of `1ad466d6`, `f392-scratch/mutate.py`, logs
`f392-scratch/MU*.log` and `mutations.log`, one at a time, each run once): every one caught.

| id | mutation | caught by |
| --- | --- | --- |
| MU1 | full-integration shear as the default | SS2 (`0.998`), SS1, CT1, CT2, RG1, FQ1, FQ2, P1 to P6, FR2 |
| MU2 | tying points of `gamma_xi`, `gamma_eta` exchanged | the same as MU1 (the element locks), FR1 (`0.76`) |
| MU3 | `DRILLING_FACTOR = 0` | FR1 (the modal solve refuses: Sturm count 87, 11 found), W2, N2 base, PT1/PT3 (singular stiffness), CT1 |
| MU4 | drilling penalty without `omega` | PT1 (`0.53`), K1 (`1.9e-4`, `2.5e-5`), FR2 (`0.41`) |
| MU5 | rigid offsets removed | K1 warped (`4.5e-2`), FR2 (`0.64`) |
| MU6 | `beta_y = +theta_x` in the bending rows | PT3 (`5.6e-3`), SS1, SS2 (`0.43`), FQ1, FQ2, P1, P5, P6, FR2 |
| MU7 | rotary inertia halved | M1 (`1.2e-4`), P5, P6, FR2 |
| MU8 | three unknowns at shell nodes | D1 (27, 48 unknowns), and the assemblies of every other test fail |
| MU9 | shell-solid node check removed | S1 (all three: an element-matrix size error instead) |
| MU10 | warp check removed | W1 (all three solve) |
| MU11 | pressure along `-e_3` | SS1, SS2 (`2.0`), CY1 (`2.0`), P1, P4, N4 |
| MU12 | `SHEAR_FACTOR = 1` | P1 (`8.7e-5`), P2, P3, P5, P6, FR2 |
| MU13 | the other paths accept shells | N2 (spring, nonlinear states, `check_supported`) |

Other checks: `cargo fmt -p rcfd-fem --check` and `cargo fmt -p rcfd-post --check` clean (after the layout-only reformat
above); `cargo clippy -p rcfd-fem -p rcfd-post --all-targets --release -- -D warnings` clean. No hash table, golden file or
`baseline_hashes.rs` edited; no existing test edited except the four retired rows (one `#[ignore]` line each).

**Not run:** the whole workspace suite (the lead's gate).

**Deviations:** none in the limits; amendment 1 (N1 by a Sturm count, the SS3 solve) was written and committed before any run.
MU3 confirmed the reason of amendment 1: with no drilling stiffness `lowest_eigenpairs` refuses the free plate with a Sturm
count of 87.

**Decision by the registered rule: done.** Every clause of the acceptance met (margins: patch tests above 1e4, SS1 13, SS2 12,
SS3 113, CT1 4.8, CT2 above 1e8, RG1 14, CY1 17, FQ1 2.5, FQ2 1.9 at (2,2), FR1 40), the stronger checks and the controls as
stated, every mutation caught, no amendment after a run.

### Remaining questions

1. **Shell output** (L1): Shell4 as `VTK_QUAD` cells with rotations, reactions and the resultants of `shell::stresses` as cell
   data in `rcfd_post::solid_output`. Options: (a) a follow-up item as F-512 was for beams; (b) leave it to the Studio work.
   Recommendation: (a), small.
2. **Shells on the other paths and loads** (L2): rigid sets and supports on shell nodes (the transformation), gravity and the
   other body loads (`rho t g` through the consistent load of the bilinear translations, as `pressure_load_vector`). Options: (a)
   one follow-up for the transformation and the body loads, the other paths with the packages that need them (F-420, F-426); (b)
   leave until a case needs them. Recommendation: (a); a housing model needs gravity and bolted supports.
3. **The drilling factor** (arithmetic 5): `1e-3` is a registered choice; no check here is sensitive to it (the free plate needs
   it only to be non-zero). Options: (a) keep; (b) a study of in-plane bending of a facet against `alpha` with an Allman-type
   membrane as a reference. Recommendation: (a) until in-plane bending of a coarse facet mesh matters (thin blades meshed as
   surfaces bend out of plane).

## For the lead: roadmap entries

F-392: proposed status **done**. Note:

> Structural solver, intermediate tier. ADR 0432, 2026-11-14: Shell4, the four-node flat-facet shell with six unknowns per node: bilinear membrane, Reissner-Mindlin bending with the MITC4 assumed transverse shear strain (tied at the edge mid-points, derived), a drilling penalty alpha G t (theta_z - omega)^2 with alpha = 1e-3 tying the drilling rotation to the membrane rotation, rigid offsets from warped nodes to the mean plane (warp ratio above 0.05 refused naming the element), consistent mass with rotary inertia rho t^3/12, Load::ShellPressure on element sets along the element normal, shell::stresses (resultants and top/bottom surface stresses at the element centres). DofMap gives six unknowns to shell nodes; shells are taken by the static and modal elimination paths and refused elsewhere (naming a follow-up of ADR 0432), a shell node shared with a solid is refused naming the node; rcfd-post refuses a shell result naming the node. Met: membrane and bending patch tests on a distorted mesh in space 7.4e-14 and 1.0e-13 (limit 1e-8); simply supported plate 16 x 16 within 7.5e-4 (t/a = 0.01) and 1.26e-3 (t/a = 0.001) of Navier (c = 0.004062352661 recomputed), the fully integrated control at 1.8e-3 of Navier; cantilever strip tip 1.05e-3, mid-span surface stress 1.2e-10; pinched ring 1.41e-3 of (pi/4 - 2/pi) P R^3/(E I); cylinder u_r 3.0e-4 of p R^2/(E t) (the inscribed polygon's cos(pi/128) within 1.1e-10); plate frequencies 0.39, 1.48 and 1.59 percent; free plate six zeros at 2.5e-6 of f_7 (Sturm count 6; 87 without the drilling stiffness); every quantity within 6.1e-10 of an independent numpy prototype of the same element. Thirteen mutations, all caught. si_scaffold_bits and baseline_hashes unchanged.

Follow-up items (proposed):

- "Shell results in the solid output: VTK quad cells, rotations and the resultants as cell data", deps F-392, F-512.

> Every limit not stated here is registered in the ADR before the run. rcfd_post::solid_output writes a model with Shell4 elements: displacement and rotation point arrays, reaction forces and moments, Shell4 as VTK quad cells and the membrane forces, moments, shear forces and top and bottom surface stresses of rcfd_fem::shell::stresses as cell data; a model without shells writes the bytes it wrote before; the arrays read back equal the result within 1e-15.

- "Shells on the transformation path and body loads on shells: rigid sets and supports on shell nodes, gravity and centrifugal loads", deps F-392, F-513.

> Every limit not stated here is registered in the ADR before the run. Gravity on a shell model gives reactions rho t A g within 1e-12 and the consistent nodal loads of the bilinear translations; a shell plate held by a rigid set on an edge has the deflection of the clamped plate within 1e-10; a nodal spring on a shell rotation gives k_theta theta = M within 1e-10; the free rigid motions count takes shell nodes; a model without shells gives the bits it gave before.
