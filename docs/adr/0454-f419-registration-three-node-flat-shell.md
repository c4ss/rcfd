# ADR 0454: F-419 registration: the three-node flat shell Shell3 (constant-strain membrane, Reissner-Mindlin bending with the MITC3 edge-tied transverse shear and the Lyly-Stenberg-Vihinen shear stabilisation, the drilling penalty of Shell4, consistent mass, pressure, body and thermal loads, resultants and surface stresses, mixed meshes with Shell4)

Status: **REGISTERED 2026-11-14, written before any code of F-419 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-419 was written, compiled or run in rcfd: the files listed under "What was read" were
read, and the arithmetic below was done by hand and evaluated with a plain numpy prototype of the element derived here
(`w49i-work/f419-scratch/proto.py`, `estimate1.py` to `estimate9_ct.py`, logs `estimate*.log`; dense matrices, no rcfd code;
disclosed, its numbers quoted below). Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0322](0322-triangular-shell-element.md) (the stub, work package SA-12), [ADR 0310](0310-structural-solver-advanced-tier-plan.md)
(the plan), [ADR 0440](0440-f408-registration-advanced-tier-scaffold.md) (F-408, the scaffold: `shell3.rs`, `ElementKind::Shell3`,
the section hook `shell::section_of`), [ADR 0432](0432-f392-registration-flat-facet-shell.md) (F-392, Shell4: the conventions,
the drilling penalty, the frame, the closed forms), [ADR 0435](0435-f522-registration-shells-on-the-transformation-and-body-loads.md)
and [ADR 0445](0445-f525-registration-shell-nodal-body-force-and-thermal-loads.md) (shell loads), [ADR 0437](0437-f521-registration-shell-solid-output.md)
(shell results in the solid output), [ADR 0417](0417-f391-registration-six-unknowns-and-beam.md) (F-391, six unknowns per node).
Roadmap item F-419 (deps F-408, F-391, F-392). Size M. Number 0454 given by the lead. Branch `f419-work` from main 8c1a4db2,
worktree `w49i-work/f419`, scratch `w49i-work/f419-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact); each registered run made once; defaults bit-identical; no edit of `baseline_hashes.rs`, of the hash
tables or of a golden file; no new dependency, no unsafe code; clean-room.

## What F-419 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. Membrane patch test and constant-curvature bending patch
> test on an irregular flat mesh, and on a mesh mixing Shell3 and Shell4, within 1e-8. Simply supported square plate under uniform
> load, 32 x 32 squares each cut in two, nu = 0.3: centre deflection within 2 percent of the Navier series at thickness/side 0.01
> and within 3 percent at 0.001, and within 1 percent of each other for the two diagonal directions. Cantilever strip, nu = 0: tip
> deflection within 1 percent. Pinched ring strip, 32 elements per quarter: loaded diameter within 3 percent of (pi/4 - 2/pi) P
> R^3/(E I). Cylinder under internal pressure: radial displacement p R^2/(E t) within 1 percent. First frequency of the simply
> supported plate within 2 percent. A free plate has six zero frequencies below 1e-6 of the seventh; a single element has exactly
> six zero eigenvalues; a triangle with an angle below the registered limit is refused with the element.

Clauses: (a1) every limit not stated is registered here; (a2) membrane patch test, irregular flat mesh of Shell3, 1e-8; (a3)
constant-curvature bending patch test, the same mesh, 1e-8; (a4) both patch tests on a mesh mixing Shell3 and Shell4, 1e-8; (a5)
simply supported plate, 32 x 32 squares each cut in two, nu = 0.3, centre deflection within 2 percent of Navier at t/a = 0.01;
(a6) the same within 3 percent at t/a = 0.001; (a7) the two diagonal directions within 1 percent of each other; (a8) cantilever
strip, nu = 0, tip deflection within 1 percent of `P L^3 / (3 E I)`; (a9) pinched ring strip, 32 elements per quarter, loaded
diameter within 3 percent of `(pi/4 - 2/pi) P R^3 / (E I)`; (a10) cylinder under internal pressure, radial displacement within 1
percent of `p R^2 / (E t)`; (a11) first frequency of the simply supported plate within 2 percent; (a12) a free plate has six zero
frequencies below `1e-6` of the seventh; (a13) a single element has exactly six zero eigenvalues; (a14) a triangle with an angle
below the registered limit is refused naming the element. The stub (ADR 0322 "Goal") and the brief add: (b1) models without Shell3
give the bits they gave before (digests recorded on the base before any code); (b2) the transverse shear isotropic in the element
(independent of the node numbering); (b3) the drilling treatment of Shell4; (b4) consistent mass, pressure and body loads,
resultants and surface stresses; (b5) the closed forms verified here (Navier, the ring, the cylinder, the plate frequency) and the
discretisation-error estimate registered for every limit; (b6) the scaffold rows naming ADR 0322 retired by the standing decision
(ignored record citing ADR 0291, new copy with the row landed); (b7) paths that cannot take shells refuse Shell3 with a located
error; (b8) the shell output of `rcfd-post` (ADR 0437).

Readings, registered:

- **(a2, a3, a4) "within 1e-8":** as ADR 0432: the largest difference between the computed and the exact nodal unknowns (all six at
  every interior node) relative to the largest exact nodal value of the field; and the element resultants against the exact
  constant resultants relative to their largest component. The patch lies in a plane placed in space by a registered rotation.
- **(a5, a6) "within x percent of the Navier series":** `|w_c / (c q a^4 / D) - 1| <= x / 100`, `c` summed in the test (`m, n <=
  799`), `D = E t^3 / (12 (1 - nu^2))`, the hard support of ADR 0432 arithmetic 11.
- **(a7):** `|w_c^(0) / w_c^(1) - 1| <= 1e-2` at both thicknesses, `(0)` every square cut along its diagonal from node `(i, j)` to
  `(i + 1, j + 1)`, `(1)` along the other diagonal.
- **(a8):** the mean deflection of the two tip nodes against `P L^3 / (3 E I)`.
- **(a9) "32 elements per quarter":** 32 divisions of the quarter along the circumference, each strip cell cut in two (64
  triangles); the change of the diameter through the load points, the closed form of ADR 0432 arithmetic 13 (re-checked below).
- **(a10):** every node of the registered mesh (arithmetic 14), `max |u_r / (p R^2 / (E t)) - 1| <= 1e-2`.
- **(a11):** the lowest computed frequency against `omega_11 = 2 pi^2 / a^2 sqrt(D / (rho t))` (ADR 0432 arithmetic 15), both
  diagonal directions.
- **(a12) "six zero frequencies below 1e-6 of the seventh":** taken literally on the frequencies: of the eight lowest frequencies
  of the unconstrained plate, `max_{i <= 6} |f_i| / f_7 <= 1e-6` and `f_7 > 0`. Arithmetic 16 shows that this is at the rounding
  level of any shell model and fixes the plate for it.
- **(a13) "exactly six zero eigenvalues":** of the 18 eigenvalues of the global stiffness of one element in space, exactly six have
  `|lambda| <= 1e-10 lambda_max`, and `lambda_7 >= 1e-7 lambda_max` (no eigenvalue between: the six are separated, not a choice of
  threshold within a cluster).
- **(a14):** the smallest interior angle below `MIN_ANGLE_DEGREES = 1` (arithmetic 1) refused on the static solve, the modal
  analysis and the stiffness assembly with `FemError::Element { element, source: InvalidValue { key: "angle", .. } }`, the message
  naming the angle and the limit.

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0322 (stub); ADR 0432 (whole: the conventions, the checks, the closed forms); ADR 0450 (head,
  the style); the heads of ADR 0435, 0437, 0440, 0445.
- `crates/rcfd-fem/src`: `shell.rs` (whole), `shell3.rs` (placeholder), `model.rs` (`check_elements`, `check_supported*`,
  `add_load` for `ShellPressure`), `assemble.rs` and `mass.rs` (the dispatch), `loads.rs` (the shell pressure, body loads,
  `add_body_load_with`, `ShellRule`), `thermal.rs` (the shell thermal load), `mpc.rs` (`check_transformation_model`), `solve.rs`
  (the multigrid refusal), `modal.rs` (`frequency`), `mesh.rs` (the Shell3 rows), `element.rs` (`element_for`); the
  `check_supported` call sites of the other paths (grep).
- `crates/rcfd-post/src/solid_output.rs` (`vtk_from_fem`, `has_shell`, `shell_fields`, `solid_vtu_string`).
- `crates/rcfd-mesh/src/surface.rs` (the surface check: a triangle with an angle below `min_angle_deg = 1.0` is a sliver).
- Tests: `crates/rcfd-fem/tests/si_shell.rs` (the helpers and the checks of Shell4), `sa_scaffold.rs` (the rows naming ADR 0322),
  `si_shell_thermal_bits.rs` (the digest pattern).
- **Not read:** any finite-element code, manual or documentation outside this repository, and no paper. Cited from memory (marked
  NOT read in `docs/reference-data.md`): P.-S. Lee and K.-J. Bathe, "Development of MITC isotropic triangular shell finite
  elements", Comput. Struct. 82 (2004) 945-962 (MITC3: the covariant shear tied at the edge mid-points with the rotational term
  `c (s, -r)`); M. Lyly, R. Stenberg and T. Vihinen, "A stable bilinear element for the Reissner-Mindlin plate model", Comput.
  Methods Appl. Mech. Engrg. 110 (1993) 343-357 (the shear modulus replaced by `k G t^3 / (t^2 + alpha h^2)`); K.-U. Bletzinger,
  M. Bischoff and E. Ramm, "A unified approach for shear-locking-free triangular and rectangular shell finite elements", Comput.
  Struct. 75 (2000) 321-334 (DSG3, with which the same stabilisation and `alpha = 0.1`, `h` the longest edge, are used); J. C.
  Nedelec, Numer. Math. 35 (1980) 315 (the lowest-order edge space); Hughes and Brezzi (drilling, as ADR 0432); Timoshenko and
  Woinowsky-Krieger (Navier, as ADR 0432). Everything used is re-derived below; the one number taken from these sources is
  `alpha = 0.1` (arithmetic 4), taken once and not varied.

## Scaffold rows retired (standing decision: the latest copy becomes an ignored record citing ADR 0291, a new copy has the row landed)

`grep "0322"` in `crates/*/tests`: only `crates/rcfd-fem/tests/sa_scaffold.rs`. The running tests among the hits:

| test | row | why |
| --- | --- | --- |
| `x1_modules_and_pending_after_f413_f414` (or the latest X1 copy on main when the code is written) | `shell3::PENDING == Some("ADR 0322 (F-419)")` | becomes `None` |
| `k1_shell3_kind` | `ElementKind::Shell3.pending() == Some("ADR 0322 (F-419)")` | becomes `None`; the placeholder still refuses naming Shell3 (its package becomes `shell3::SOLID_ROUTINES`) |
| `r2b_to_r4b_refusals` | R2b: the unloaded Shell3 plate refused on E1 to E6, E9, E10 naming `ADR 0322 (F-419)` | Shell3 is taken where Shell4 is (E1 to E4, E9, E10 succeed); E5 and E6 refuse naming `shell3::FOLLOW_UP` |

Each gets `#[ignore = "record of ADR 0440 before ADR 0454 (F-419) landed Shell3, retiring its row; continued in <copy> (ADR 0291
pattern, maintainer standing decision)"]` and nothing else; each copy keeps every other row as it was (R3b, R4b, K1's other rows)
and states the new behaviour of the retired row. The ignored records already in the file stay as they are. F-409 edits the X1
rows concurrently: the X1 edit is kept to the one copy.

## Arithmetic done before the code (disclosed)

1. **Facet, frame, angle.** Nodes `x_0, x_1, x_2` (the order of the mesh), `n = (x_1 - x_0) x (x_2 - x_0)`, `e_3 = n / |n|`, area
   `A = |n| / 2`, `e_1 = (x_1 - x_0) / |x_1 - x_0|`, `e_2 = e_3 x e_1`, centroid `c`, local coordinates `(X_i, Y_i) = ((x_i - c) .
   e_1, (x_i - c) . e_2)`. A triangle is flat: no offsets, `T` is the plain rotation (per node `diag(Lambda, Lambda)`). Refused:
   `|n| = 0` or non-finite (`InvalidValue { key: "geometry" }`), the smallest interior angle below **`MIN_ANGLE_DEGREES = 1.0`**
   (`InvalidValue { key: "angle" }`, the message naming the angle and the limit). The limit is the sliver angle of the repository's
   surface check (`rcfd_mesh::surface`, `min_angle_deg: 1.0`), so a surface the mesher passes is not refused by the element; a
   judgement, not tuned. Check AN1 uses the triangle `(1, 0, 0), (2, 0, 0), (1.5, h, 0)` with base angles `atan(2 h)`: `h = 0.5
   tan(0.9 deg) = 7.8544e-3` (refused) and `h = 0.5 tan(1.1 deg) = 9.6013e-3` (taken).
2. **Membrane (CST).** Linear `u, v`, `N_i,X = b_i / (2 A)`, `N_i,Y = c_i / (2 A)` with `b_i = Y_j - Y_k`, `c_i = X_k - X_j` (`i, j,
   k` cyclic); strains `(u_X, v_Y, u_Y + v_X)` constant; `N = A_m eps`; exact for every linear field, so the membrane patch test
   holds on any mesh (prototype, the irregular patch in space: `1.3e-11`, `estimate1.log`).
3. **Bending.** Linear `w, theta_x, theta_y`, `beta_x = theta_y`, `beta_y = -theta_x` (ADR 0432 arithmetic 3); curvatures
   `(beta_x,X, beta_y,Y, beta_x,Y + beta_y,X)` constant; `m = D_b kappa`; surface stresses `N / t +- 6 m / t^2`.
4. **Transverse shear: the MITC3 edge tying, derived.** On edge `e` from node `i` to node `j` (`e = 0, 1, 2` for `(0, 1), (1, 2),
   (2, 0)`), length `l_e`, tangent `t_e`, mid-point `m_e`, the tangential shear of the displacement field at the mid-point is
   `g_e = (w_j - w_i) / l_e + (beta_i + beta_j) / 2 . t_e` (the chord rule: `w` linear along the edge, `beta` averaged). The assumed
   field is the lowest-order edge space `gamma(X, Y) = (a_1 - b Y, a_2 + b X)` (Nedelec): its tangential component is constant along
   any straight line (`(-Y, X) . t` is the moment of `t` about the centroid, constant on the line), and the three coefficients are
   fixed by `gamma(m_e) . t_e = g_e`, a 3 x 3 system with rows `(t_x, t_y, m_x t_y - m_y t_x)`, regular for a non-degenerate
   triangle. In the natural coordinates `(r, s)` of the triangle this is a constant plus `c (s, -r)` in covariant components, which
   is MITC3 as recalled (Lee and Bathe: `e_rt^(1)` at `(1/2, 0)`, `e_st^(2)` at `(0, 1/2)`, `e_qt^(3)` at `(1/2, 1/2)`); the
   equality is from memory and is not needed: the element is defined by the equations here. Properties: (i) **isotropy**: the
   definition uses only edges and mid-points, so a relabelling of the nodes gives the same matrix (prototype `6.5e-16`, check ISO1);
   (ii) **patch test**: for `w` quadratic and `beta = -grad w` linear every `g_e = 0` (the chord rule is exact for a quadratic, the
   average for a linear field), so the constant-curvature state has zero shear and constant moments (prototype `7.2e-13`); (iii)
   **rank**: on `w` linear and `beta` constant the three edge values satisfy `sum_e l_e g_e = 0`, so shear plus bending have exactly
   the three rigid zeros (with the membrane and drilling, six per element, check SE1). Energy `int gamma . H gamma dA` by the
   three mid-edge points (exact for the quadratic integrand). **Locking:** plain MITC3 locks on a uniform mesh: the prototype gives
   `0.8988` of Navier at t/a = 0.001 on 32 x 32 (`estimate2_ss.log`), outside the acceptance's 3 percent. **Stabilisation (taken,
   after Lyly, Stenberg and Vihinen, as used with DSG3 by Bletzinger et al., from memory):** the shear stiffness of the element is
   `H_e = H t^2 / (t^2 + alpha h_e^2)`, `h_e` the longest edge, **`alpha = 0.1`** (`SHEAR_STABILISATION`; the value of those papers,
   taken once, not varied: the prototype was run with this one value). It leaves the patch tests unchanged (the patch states have
   zero shear strain), tends to the plain element as `h_e / t -> 0` and relaxes the discrete Kirchhoff constraint where `h_e >> t`.
   Prototype (32 x 32, rcfd data `E = 2e11`, `q = 1000 Pa`): `0.9981577495` at t/a = 0.01 and `0.9974430088` at 0.001, both
   diagonals equal to `3.6e-12`; convergence at t/a = 0.001: `0.958317, 0.989662, 0.997443` on 8, 16, 32 (error ratios `4.0`, `4.0`:
   second order), so the 32 x 32 error `2.6e-3` is the discretisation error. The shear forces reported are those of `H_e` (the
   discrete shear in equilibrium with the moments). Options considered: (a) this stabilisation (taken); (b) MITC3+ (a cubic bubble on
   the rotations with modified tying, Lee and Bathe 2014: recalled too imprecisely to derive here); (c) the discrete-Kirchhoff
   triangle, the stub's fallback (thin plates only; not an assumed-strain element). Taken on the maintainer's standing instruction of
   2026-11-11 (take the recommendation).
5. **Drilling (the treatment of Shell4, ADR 0432 arithmetic 5).** `alpha_d G t int (theta_z - omega)^2 dA`, `omega = (v_X - u_Y) /
   2` (constant for the CST), `theta_z` linear, the mid-edge rule (exact), `alpha_d = shell::DRILLING_FACTOR = 1e-3`. Zero for the
   rigid motions and the linear membrane states with `theta_z = omega` (patch test unaffected). One element: six eigenvalues below
   `1.2e-16` of the largest, the seventh at `3.8e-6` (t = 0.01) and `7.1e-6` (t = 0.1); with `alpha_d = 0` the seventh is a zero
   (`4e-17`) (`estimate1.log`).
6. **Mass.** `rho t int N_i N_j dA = rho t A (1 + delta_ij) / 12` on the translations and `rho t^3 / 12` times the same on the three
   rotations (Shell4's registered `theta_z` inertia; ADR 0432 arithmetic 6). Exact rigid kinetic energy except about the normal,
   where `rho t^3 A / 12` is added (check M1). Prototype: the mass of one element `8.635 = rho t A` (`estimate7_modal.log`).
7. **Loads.** Pressure along `e_3`: `p A / 3` per node (exact). Body force per volume `b(x)`: `t int N_i b dA` by the mid-edge rule
   (exact for `b` linear in `x`, so for gravity, the centrifugal load and a nodal field `sum N_j b_j`: `t A / 12 (2 b_i + b_j +
   b_k)`). Thermal (membrane, uniform through the thickness, ADR 0445): `A B_m^T A_m alpha dT_mean (1, 1, 0)`, `dT_mean` the mean
   of the three nodal values (`B_m` constant, `dT` linear); the stresses subtract `alpha dT_mean (1, 1, 0)` from the membrane strain.
8. **Resultants and stresses.** At the centroid, in the element's local axes: `N`, `m`, `Q = H_e gamma(c)` (`gamma(c) = (a_1, a_2)`),
   `sigma` at `z = +-t/2`, the `ShellStresses` of Shell4. Cantilever (arithmetic 12): the mid-span cell's two triangles have the
   surface stress, rotated to the strip axes, within `1.3e-6` of `-6 M / (b t^2)` (`estimate9b_ct.log`).
9. **Navier coefficient.** As ADR 0432 arithmetic 10: `c = 0.004062352661` (m, n <= 799; recomputed, `estimate2_ss.log`).
10. **Simply supported plate.** 32 x 32 squares of the unit square, each cut in two, `nu = 0.3`, `q = 1000 Pa`, constraints of ADR
    0432 arithmetic 11 (`u = v = theta_z = 0` everywhere, `w = 0` on the edges, the hard support). Estimates (prototype): SS1
    `-1.84e-3` (Mindlin's own `+5.2e-4` included), SS2 `-2.56e-3`, SS3 `3.6e-12`. Controls at t/a = 0.001: the plain MITC3
    (`alpha = 0`) `0.8988341478`, the displacement shear by the mid-edge rule without stabilisation `2.863079e-3`.
11. **Cantilever strip.** `L = 1`, `b = 0.1`, `t = 0.01`, `nu = 0`, 15 x 1 squares each cut along `(i, 0)-(i + 1, 1)`, clamped at
    `x = 0`, `P = 1 N` along `+z` on the two tip nodes. Prototype `0.9998605036` (`-1.39e-4`; both diagonals equal).
12. **Pinched ring.** ADR 0432 arithmetic 13: `M(phi) = (P R / 2)(cos phi - 2/pi)`, `U = P^2 R^3 / (2 E I)(pi/4 - 2/pi)`,
    `dU/dP` the shortening of the loaded diameter, `0.1487783910 P R^3 / (E I)` (re-checked: `int_0^{pi/2} (cos phi - 2/pi)^2 dphi =
    pi/4 - 2/pi` since `int cos^2 = pi/4`, `-(4/pi) int cos = -4/pi`, `(4/pi^2)(pi/2) = 2/pi`). The model of ADR 0432 (R = 1, b =
    0.05, t = 0.01, nu = 0, P = 1, symmetry at both ends of the quarter, the in-plane unknowns only) with each of the 32 cells cut
    along `(i, 0)-(i + 1, 1)`. Prototype `0.9991065416` (`-8.9e-4`).
13. **Cylinder: a free-edge effect of the triangle's consistent pressure, and the mesh registered.** The closed form `p R^2 / (E t)`
    (free ends, ADR 0432 arithmetic 14) and the facet polygon's `cos(pi / 128) = 0.999698818696` on 32 cells per quarter. Found with
    the prototype (`estimate2_cy.log` to `estimate6_cy.log`): with radial nodal loads the element gives `cos(pi / 128)` at every node
    to `3e-10` on any of the meshes, but the consistent pressure `p A / 3 e_3` of linear triangles on adjacent facets gives the nodes
    of a free edge (i) on a one-diagonal mesh, unequal weights of the two facet normals, so tangential edge loads (0.8 percent of the
    radial) opposite at the two ends that excite the soft inextensional ovalisation of the open end (`0.67 .. 1.31` of the closed
    form on 32 x 4 cells), (ii) on any alternating mesh, radial edge loads alternating `2 : 1` from node to node, a sawtooth that
    bends the edge locally. The effect is confined to the free edge and falls with the axial element size `dz`: on the checkerboard
    mesh (cell `(i, j)` cut along `(i, j)-(i + 1, j + 1)` when `i + j` is even, along the other diagonal otherwise) the spread is
    `2.3e-2, 6.7e-3, 1.8e-3, 7.2e-4` at `dz = H/4, H/8, H/16, H/32`. **Registered model:** the quarter of ADR 0432 (R = 1, H = 0.5, t
    = 0.01, nu = 0.3, p = 1e5, the symmetry conditions, the end `z = H` free), **32 x 16 cells, checkerboard**: prototype `0.9982414459
    .. 1.0016208372` over the nodes (`1.76e-3`). The one-diagonal mesh of the same size gives `0.9028 .. 1.0951` (registered as the
    observation O1 and the limit L4, not a check).
14. **Plate frequency.** 32 x 32, t/a = 0.01, rho = 7850, the plate of arithmetic 10, `ModalOptions::new(4).tolerance(1e-12)
    .residual_test(ResidualTest::Backward)` (as ADR 0432 arithmetic 15). `omega_11 / (2 pi) = 47.9864611171 Hz`. Prototype `f_1 ..
    f_4 = 48.0684613, 120.3395463, 120.5716920, 193.2516204 Hz` (`f_1` `+1.709e-3`; both diagonals `1.0017088194`, `1.0017088195`
    times `omega_11`).
15. **Single element.** The triangle `(0, 0), (0.9, 0.15), (0.3, 0.7)` (angles 57.3, 52.0, 70.7 deg) placed in space by `x = (0.3,
    -0.2, 0.5) + Q (X, Y, 0)`, `Q` the rotation of 0.7 rad about `(1, -2, 2) / 3`; steel, t = 0.01 and 0.1. Prototype: `max_{i <= 6}
    |lambda_i| / lambda_max = 1.1e-16, 7.8e-17`, `lambda_7 / lambda_max = 3.8e-6, 7.1e-6`; rigid motions `|K r| / (max |K| max |r|)
    <= 3.0e-16`; relabelling `[1, 2, 0]`, `[2, 0, 1]` `<= 6.5e-16`. The test computes the eigenvalues by cyclic Jacobi (test code).
16. **Free plate, the rounding bound of the zero frequencies (a12).** The six rigid eigenvalues come out of the shift-invert
    eigensolver as rounding: `|lambda_i| <~ eps lambda_max`, so `|f_i| / f_7 <~ sqrt(eps lambda_max / lambda_7)`. Prototype
    (`estimate7_modal.log`, `estimate8_free.log`; plate `1 x 0.8`, steel, squares cut along one diagonal): 8 x 8, t = 0.01: bound
    `2.0e-5`; 4 x 4, t = 0.05: `1.9e-6`; 4 x 4, t = 0.1: `8.7e-7`; 3 x 3, t = 0.1: `6.9e-7`; 2 x 2, t = 0.1: `5.1e-7`. The
    acceptance's `1e-6` is at this bound for every shell plate (the ratio `lambda_max / lambda_7` of a shell is at least about `1e3`:
    the membrane and the thickness shear against the first bending mode), so it cannot be met with margin by a bound. Registered:
    **FR1 on the 4 x 4 plate, t = 0.1** (bound `8.7e-7`; the dense eigensolver of the prototype gives `6.6e-7`; on Shell4 the rcfd
    shift-invert solver measured `2.5e-6` against its bound `5.7e-5`, a factor 23 below, ADR 0432 addendum), limit `1e-6` as
    written; **FR2** on the 8 x 8 plate, t = 0.01, the limit `1e-4` of ADR 0432; **FR3** the Sturm count below `lambda_7 / 4` equal to
    6 on both (exact, robust). Prototype `f_7 = 395.4141399498 Hz`, `f_8 = 525.7994847722 Hz` (4 x 4, t = 0.1); `40.8196288856`,
    `52.4645618939 Hz` (8 x 8). If FR1 misses, it is a `*_known_miss` diagnosed against this bound.
17. **Patch tests.** The patch of ADR 0432 arithmetic 17 (3 x 3 cells of the unit square, the four interior nodes moved, in space
    by `Q`, t = 0.05, its membrane and bending fields, the twelve boundary nodes prescribed). Shell3 mesh: cell `n` (row-major) cut
    along `(0, 2)` when `n` is even and `(1, 3)` when odd: 18 triangles, smallest angle 20.2 deg; prototype membrane `1.3e-11`,
    bending `7.2e-13`. **Mixed mesh:** cells 0, 4, 8 (the diagonal) Shell4, the six others cut as above (12 Shell3), two blocks with
    the same Shell property. Every element is exact for both fields and the edges are conforming (both linear along an edge), so the
    interior nodes are in equilibrium with no load; the estimate is the rounding of PT1 and PT3 (not prototyped).
18. **Loads, mass, thermal checks (b4).** A flat Shell3 plate `[0, 0.6] x [0, 0.4]` in 3 x 2 cells cut in two, placed by `Q`, t =
    0.01, steel with `alpha = 1.2e-5`: gravity `g = (0.5, -1.5, -9.81)`, the centrifugal load (axis `(0, 0.3, 1)`, origin `(0.4, 0.2,
    0)`, `omega = 120`), a nodal body force `b = (x + 2 y, -z, 3 - x)`: each nodal force against `t A_e / 12 (2 b_i + b_j + b_k)` summed
    over the elements (closed form of arithmetic 7, computed in the test from the node coordinates); pressure: the sum of the nodal
    forces equals `p A n`; thermal: the same mesh in the `x, y` plane (not rotated), held at node 0 only (all six unknowns, which
    the free expansion `u = alpha dT (x - x_0)`, `w = 0`, `theta = 0` satisfies there), under a uniform `dT = 50 K`: the
    translations equal `alpha dT (x - x_0)` and the membrane forces are zero, relative to `E alpha dT t` (a plate held at one node
    has the rounding rotation of F-525 through the soft drilling mode, so the displacement limit is relative, `1e-8`, not `1e-12`).

## Design

### D1. The element (`shell3.rs`)

`shell3::PENDING = None`. Public: `MIN_ANGLE_DEGREES = 1.0`, `SHEAR_STABILISATION = 0.1`, `FOLLOW_UP = "a follow-up of ADR 0454"`,
`SOLID_ROUTINES = "a follow-up of ADR 0454 (the solid-element routines of Shell3; its matrices are in rcfd_fem::shell3)"`; `Facet3`
and `facet(coords)` (arithmetic 1); `Shell3Options { shear: shell::TransverseShear, drilling, stabilisation }` (default: the assumed
strain, `shell::DRILLING_FACTOR`, `SHEAR_STABILISATION`; the other values exist for the controls only and are not reachable from a
model); `local_stiffness_section`, `element_stiffness`, `element_stiffness_with`, `element_stiffness_section`, `element_mass`,
`pressure_load_vector`, `body_load_vector_at`, `thermal_load_vector` (arithmetic 2 to 7) and the per-element stresses used by
`shell::stresses` (arithmetic 8). The section comes from `shell::section_of` (ADR 0440 D6), so a layup reaches Shell3 when F-428
lands; the stabilisation uses the section's thickness `drilling[1]`. `element()` stays a `Pending` naming `SOLID_ROUTINES`;
`reference_nodes`, `faces`, `shape_gradients` stay empty.

### D2. Dispatch (every site that names Shell4 gets the Shell3 branch after it; nothing else changes)

`assemble::stiffness_with` and `mass::consistent_with` (Shell3 to `shell3::model_element_*`); `loads.rs`: `Load::ShellPressure` on
Shell3 elements, gravity, the centrifugal load and the nodal body force on Shell3 (the private `ShellRule` takes the shape values
as a slice); `thermal.rs` (the Shell3 thermal load); `model.rs`: `add_load` accepts Shell3 in a shell-pressure set, `check_elements`
refuses Shell3 on the paths that do not take shells as Shell4 (`NotYetSupported { variant: "Shell3", package: shell3::FOLLOW_UP }`);
`shell.rs`: `check_shell_model` checks Shell3 blocks (Shell property, node shared with a solid, a valid facet), `shell_thickness`
and `thickness_or_error` take Shell3 (the message naming the kind), `stresses` gives Shell3 elements too, in element order;
`solve.rs` (the multigrid refusal names Shell3), `mpc.rs` (the cyclic refusal names Shell3). `rcfd-post` (b8): `vtk_from_fem` maps
Shell3 to `VTK_TRIANGLE` (5), the shell output (`has_shell`, `shell_fields`) takes Shell3 blocks with the arrays of ADR 0437. For a
model without Shell3 every branch taken is the former one, in the same order: the same bits (B1).

### D3. Which paths take Shell3

Those that take Shell4: the static solve and the modal analysis on the elimination path and on the transformation (ADR 0435), the
assembly, the mass. Every other path refuses Shell3 as it refuses Shell4.

## Checks (limits fixed now)

| id | clause | check | limit | estimate behind it |
| --- | --- | --- | --- | --- |
| B1 | b1 | digests (FNV-1a over the bits) of a Shell4 plate (stiffness, mass, the load vector of pressure, gravity, centrifugal, nodal body force and temperature, the static solution, the stresses, four frequencies), a beam cantilever (stiffness, solution) and a Hex8 box (stiffness, mass, gravity load, solution) equal those printed on the base 8c1a4db2 before any library code | exact | D2 |
| B2 | b1, b6 | every existing `rcfd-fem` and `rcfd-post` test passes, unedited except the retired rows (ignored) whose copies pass | pass | the retired-rows table |
| B3 | b1 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | no fluid file touched |
| PT1 | a2 | Shell3 membrane patch, interior unknowns | `1e-8` | arithmetic 17: `1.3e-11` |
| PT2 | a2 | its `N` against `A_m eps` relative to `max |N|`; `m`, `Q` relative to the same | `1e-8` | rounding |
| PT3 | a3 | Shell3 bending patch, interior unknowns | `1e-8` | `7.2e-13` |
| PT4 | a3 | its `m` against `D_b kappa` relative to `max |m|`; `Q` and `N` times the patch side relative to `max |m|` | `1e-8` | arithmetic 4 (ii) |
| PT5 | a4 | mixed mesh, membrane: interior unknowns; `N` of every element (Shell3 and Shell4) | `1e-8` | arithmetic 17 |
| PT6 | a4 | mixed mesh, bending: interior unknowns; `m` of every element | `1e-8` | arithmetic 17 |
| SS1 | a5 | SS plate t/a = 0.01, cut `(0)`: `|w_c / (c q a^4 / D) - 1|` | `2e-2` | arithmetic 10: `1.84e-3` |
| SS2 | a6 | the same at t/a = 0.001 | `3e-2` | `2.56e-3` |
| SS3 | a7 | `|w_c^(0) / w_c^(1) - 1|` at t/a = 0.01 and 0.001 | `1e-2` each | `3.6e-12` |
| SS4 | b5 | the Navier coefficient summed in the test against `0.004062` | `5e-7` | `3.5e-7` |
| CT1 | a8 | cantilever tip against `P L^3 / (3 E I)` | `1e-2` | arithmetic 11: `1.39e-4` |
| CT2 | b4 | the two triangles of the mid-span cell: top and bottom `sigma_xx` in the strip axes against `-+6 M / (b t^2)` | `2e-2` | arithmetic 8: `1.3e-6` |
| RG1 | a9 | ring diameter change against `(pi/4 - 2/pi) P R^3 / (E I)` | `3e-2` | arithmetic 12: `8.9e-4` |
| CY1 | a10 | cylinder, checkerboard 32 x 16: `max_nodes |u_r / (p R^2 / (E t)) - 1|` | `1e-2` | arithmetic 13: `1.76e-3` |
| FQ1 | a11 | lowest frequency against `omega_11`, both diagonals | `2e-2` each | arithmetic 14: `1.71e-3` |
| FR1 | a12 | free plate 4 x 4, t = 0.1: `max_{i <= 6} |f_i| / f_7`; `f_7 > 0` | `1e-6` | arithmetic 16: bound `8.7e-7` |
| FR2 | a12 | free plate 8 x 8, t = 0.01: the same | `1e-4` | bound `2.0e-5` |
| FR3 | a12 | Sturm count below `lambda_7 / 4` (both plates) | `6` | exact |
| SE1 | a13 | one element in space, t = 0.01 and 0.1: the count of `|lambda| <= 1e-10 lambda_max` is 6; `lambda_7 / lambda_max` | `6`; `>= 1e-7` | arithmetic 15: `<= 1.1e-16`; `3.8e-6`, `7.1e-6` |
| SE2 | a13 | its rigid motions `|K r| / (max |K| max |r|)`; `K` symmetric bit for bit | `1e-12`; exact | `3.0e-16` |
| ISO1 | b2 | the element with its nodes relabelled `[1, 2, 0]` and `[2, 0, 1]`, permuted back, against `K` relative to `max |K|` | `1e-12` | arithmetic 4 (i): `6.5e-16` |
| AN1 | a14 | a two-element model whose element 1 has a 0.9 deg angle: `solve_static_with`, `modal_analysis_with`, `stiffness` refused with `Element { element: 1, InvalidValue { key: "angle" } }`, the message naming the angle and the limit | exact | arithmetic 1 |
| AN2 | a14 | the same with 1.1 deg solves | pass | arithmetic 1 |
| AN3 | a14 | a collinear triangle refused with `key: "geometry"` naming the element | exact | arithmetic 1 |
| S1 | b7 | a Shell3 element on three nodes of a Hex8 box: static, modal, stiffness refused with `Element { element, InvalidValue { key: "node" } }` naming the first shared node | exact | D2 |
| N2 | b7 | a valid Shell3 model: `nonlinear::initial_states`, smoothed-aggregation CG, `check_supported()`, a cyclic constraint on `solve_static_mpc`: `NotYetSupported` naming Shell3 and `shell3::FOLLOW_UP`; the static solve with a nodal spring (the transformation) is taken | exact | D2, D3 |
| N3 | b7 | a Shell3 block without property refused (`property`, naming Shell3); `Load::shell_pressure` on a Hex8 set still refused (`shell pressure`); on a Shell3 set taken | exact | D2 |
| LD1 | b4 | gravity, centrifugal, nodal body force: every nodal force of the plate of arithmetic 18 against the closed form, relative to the largest | `1e-12` | arithmetic 7 |
| LD2 | b4 | pressure: sum of the nodal forces against `p A n`; nodal moments zero, relative | `1e-12` | arithmetic 7 |
| LD3 | b4 | thermal free expansion: translations against `alpha dT (x - x_0)` relative to `max`; membrane forces relative to `E alpha dT t` | `1e-8`; `1e-8` | arithmetic 18 |
| M1 | b4 | `mass_properties` of one element in space (rho 7850, t 0.01): mass `rho t A`; centre the centroid; inertia `Q (rho t J + rho t^3 A / 12 I) Q^T`, `J` the in-plane second-moment tensor of the triangle | `1e-12` | arithmetic 6 |
| PO1 | b8 | `rcfd_post::solid_output::shell_fields` of a mixed Shell3 + Shell4 result: the cell arrays equal `shell::stresses` bit for bit; `solid_vtu_string` writes cell type 5 for the Shell3 elements and 9 for Shell4 | exact | D2 |

Stronger checks (the rcfd element against the independent numpy prototype, `estimate*.log`):

| id | check | limit | prototype |
| --- | --- | --- | --- |
| P1 | SS plate ratios at 0.01, 0.001 (cut `(0)`); the two controls (N4, N5) | `1e-5` relative each | `0.9981577495`, `0.9974430088`; `0.8988341478`, `2.863079e-3` |
| P2 | cantilever tip ratio | `1e-5` | `0.9998605036` |
| P3 | ring ratio | `1e-5` | `0.9991065416` |
| P4 | cylinder min and max of `u_r / (p R^2 / (E t))` | `1e-5` | `0.9982414459`, `1.0016208372` |
| P5 | SS plate `f_1 .. f_4` | `1e-5` each | `48.0684613135, 120.3395462744, 120.5716920194, 193.2516203810 Hz` |
| P6 | free plates `f_7`, `f_8` | `1e-5` | `395.4141399498, 525.7994847722`; `40.8196288856, 52.4645618939 Hz` |

Negative controls and observation:

| id | control | expected | limit |
| --- | --- | --- | --- |
| N1 | 4 x 4 free plate with `drilling = 0` (through `assemble_with`): Sturm count below `lambda_7 / 4` | more zeros than six (`6 + 25` estimated) | `> 6` |
| N4 | SS plate t/a = 0.001 with `stabilisation = 0` (plain MITC3) | locks: ratio | `< 0.95` (arithmetic 10: `0.8988`) |
| N5 | the same with the displacement shear (`TransverseShear::FullIntegration`, the mid-edge rule) and `stabilisation = 0` | locked | `< 0.2` (`2.86e-3`) |
| O1 | cylinder on the one-diagonal 32 x 16 mesh | printed, no limit | arithmetic 13: `0.9028 .. 1.0951` |

## Planned mutations (on a scratch copy, one at a time)

| id | mutation | expected to be caught by |
| --- | --- | --- |
| MU1 | `SHEAR_STABILISATION = 0` | SS2, P1 |
| MU2 | the tying with `beta_i` instead of `(beta_i + beta_j) / 2` | PT3, ISO1 |
| MU3 | the drilling factor of Shell3 `0` | SE1, FR1, FR3 |
| MU4 | the drilling penalty without `omega` | PT1, SE2 |
| MU5 | `beta_y = +theta_x` in the bending rows | PT3, SS1 |
| MU6 | rotary inertia halved | M1, P5 |
| MU7 | pressure `p A / 2` per node | LD2, SS1 |
| MU8 | the angle check removed | AN1 |
| MU9 | `check_elements` accepts Shell3 on the other paths | N2 |
| MU10 | `h_e` the shortest edge | P1 |
| MU11 | `shell::stresses` skips Shell3 | PT2, CT2, PO1 |
| MU12 | the Shell3 body load with `t A / 4` per node (lumped) | LD1 |
| MU13 | the Shell3 thermal load omitted | LD3 |
| MU14 | `rcfd-post` maps Shell3 to `VTK_QUAD` | PO1 |
| MU15 | `check_shell_model` skips Shell3 blocks | S1, N3 |

## Decision rule

- **done:** every check of the acceptance and the brief (B1 to B3, PT1 to PT6, SS1 to SS4, CT1, CT2, RG1, CY1, FQ1, FR1 to FR3, SE1,
  SE2, ISO1, AN1 to AN3, S1, N2, N3, LD1 to LD3, M1, PO1) met, the stronger checks and the controls as stated, every mutation
  caught by a registered check, no amendment after a run.
- **partial:** a check missed (recorded as a `*_known_miss`) or not run, a mutation not caught, or a check met only after an
  amendment written after its run.
- **todo:** the element does not build.

## Limits (not covered)

- L1. The stabilisation `alpha = 0.1` is a published device, not derived here; its value is taken once. The reported shear forces
  are those of the stabilised modulus.
- L2. The CST membrane is stiff in in-plane bending (not checked); a drilling-unknown (Allman) membrane is a follow-up.
- L3. Homogeneous section by default, constant thickness per block, mid-surface at the nodes, stresses at the centroid only. The
  paths refused for Shell4 (nonlinear, transient, cyclic, harmonic, buckling, geometric stiffness, multigrid) refuse Shell3.
- L4. The consistent pressure of linear triangles on a curved surface loads a free edge with an `O(dz)` tangential or sawtooth part
  (arithmetic 13): a coarse one-diagonal mesh of an open cylinder ovalises its free end (O1). Not a defect of the element (radial
  loads give the polygon's value at every node), but a limit of the facet triangle; a curved-geometry pressure is a follow-up.

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. The shear stabilisation of arithmetic 4 (option (a)) with `alpha = 0.1`, `h_e` the longest edge.
2. The minimum angle `1 deg`, the surface check's sliver angle (arithmetic 1).
3. The free-plate clause read on the frequencies as written, on the 4 x 4 thick plate (arithmetic 16), with FR2 and FR3 beside it.
4. The checkerboard mesh of the cylinder (arithmetic 13).
5. Shell3 in the shell output of `rcfd-post` as `VTK_TRIANGLE` cells (D2).
