# ADR 0435: F-522 registration: shells on the transformation path and body loads on shells (rigid sets, supports and linear constraints on shell nodes, the free rigid motions count with shell nodes, gravity and centrifugal loads on Shell4)

Status: **REGISTERED 2026-11-14, written before any code of F-522 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-522 was written, compiled or run in rcfd: the files listed under "What was read" were
read and the arithmetic below was done by hand and evaluated with a short script of closed forms
(`w49i-work/f522-scratch/estimate.py`, log `estimate.log`; plain Python with numpy, no finite-element code and no rcfd code;
disclosed, its numbers quoted below). Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0432](0432-f392-registration-flat-facet-shell.md) (F-392: Shell4, six unknowns at shell nodes, `shell.rs`; its D3
refuses shells on the transformation path naming `shell::FOLLOW_UP` and gravity, centrifugal and body loads through the
solid-element routines naming `shell::SOLID_ROUTINES`; its limit L2 and remaining question 2 option (a) name this item),
[ADR 0426](0426-f513-registration-beams-on-the-transformation.md) (F-513: the rigid-set rows `theta_a - w = 0` at six-unknown
nodes, `mpc::check_transformation_model`, `free_motions` with rotational unknowns),
[ADR 0422](0422-f514-registration-beam-line-and-gravity-loads.md) (F-514: gravity on beams by an element rule in
`loads::add_body_load_with`, the pattern followed here), [ADR 0429](0429-f518-registration-beam-entry-refusal.md) (F-518: a
check that observes the entry refusal before the transformation is built, the pattern of S3 here),
[ADR 0296](0296-linear-multi-point-constraints-rigid-sets-elastic-supports.md) (the transformation `x = T q + g`). Roadmap item
F-522 (deps F-392, F-513). Number 0435 given by the lead. Branch `f522-shells-transformation-body-loads`, worktree
`w49i-work/f522`, scratch `w49i-work/f522-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact); each registered run made once; defaults bit-identical; no edit of `baseline_hashes.rs`, of the hash
tables or of a golden file; no new dependency, no unsafe code; clean-room.

## What F-522 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. Gravity on a shell model gives reactions rho t A g within
> 1e-12 and the consistent nodal loads of the bilinear translations; a shell plate held by a rigid set on an edge has the
> deflection of the clamped plate within 1e-10; a nodal spring on a shell rotation gives k_theta theta = M within 1e-10; the free
> rigid motions count takes shell nodes; a model without shells gives the bits it gave before.

Clauses: (a1) every limit not stated is registered here; (a2) gravity on a shell model: the reactions sum to `-rho t A g` within
1e-12; (a3) gravity on a shell model: the nodal loads are the consistent loads `f_i = rho t g int N_i dA` of the bilinear
translations; (a4) a shell plate held by a rigid set on an edge has the deflection of the clamped plate within 1e-10; (a5) a nodal
spring on a shell rotation: `k_theta theta = M` within 1e-10; (a6) `free_rigid_motions` counts models with shell nodes; (a7) a
model without shells gives the bits it gave before. The title adds (t1) supports on shell nodes (springs: a5) and (t2)
centrifugal loads on shells. The brief adds: (b1) solid-only, beam-only and shell models on the existing paths keep their bits
(`si_scaffold_bits.rs` and every existing test unedited, except retired rows); (b2) retired refusal rows follow the standing
decision (an ignored record citing this ADR plus a running `_after_landed` copy).

Readings, registered:

- **(a2) "reactions rho t A g within 1e-12":** the static solve of a shell model under `Load::Gravity { g }` alone; the sum of
  the reaction forces over every constrained unknown of components 0 to 2, `R`, against `-rho t A g`: `|R + rho t A g| /
  (rho t A |g|) <= 1e-12`, `A` the total mid-surface area computed in the test independently of the element (two triangles per
  facet). The model is registered in arithmetic 5 so that the reactions are rounding-limited well below 1e-12.
- **(a3) "the consistent nodal loads of the bilinear translations":** the load vector `loads::nodal_load_vector` of a shell
  model under gravity, node by node, against `f_i = rho t g sum_e int_e N_i dA` evaluated in the test by the closed form of the
  bilinear map (arithmetic 2, no quadrature), relative to the largest nodal force; the nodal moments of a flat mesh are zero
  (relative to the largest force times 1 m).
- **(a4) "the deflection of the clamped plate":** registered as the comparison with **the same plate and the same load with the
  edge nodes fully clamped (all six unknowns prescribed to zero), solved on the elimination path**; every unknown of every node
  (translations and rotations), relative to the largest value of each field, within 1e-10. Arithmetic 3 derives why a rigid set
  on an edge whose body is held equals that clamp: the rigid set ties the rotations as well as the translations (ADR 0426 D1).
  Two ways of holding the set's body are checked: one node of the edge clamped (H1), and the set extended over the face of a
  fixed Hex8 block (H2, the joint of a shell to a solid that ADR 0432 D4 leaves to the transformation).
- **(a5) "k_theta theta = M":** `ElasticSupport::NodalSpring` on component 4 (`theta_y`) of the root nodes of a shell strip; `M`
  the moment each spring carries (known by statics and symmetry, arithmetic 4); `|k_theta theta - M| / M` per spring.
- **(a6) "takes shell nodes":** `free_rigid_motions` gives the counts known by hand on shell models (arithmetic 6), including
  counts that need the rotation of the rigid fields at shell nodes, and the free fields are in the null space of the shell
  stiffness.
- **(t2) centrifugal loads on shells: included** (registered decision, the brief's "if cheap and checkable against a closed
  form"). The consistent load `f_i = rho t omega^2 int N_i r_perp(x) dA` with the same 2 x 2 rule; checked against two closed
  forms that are exact for the element (arithmetic 7): a radial strip spinning about an axis normal to it, whose nodal
  displacements are those of the 1D bar `u(x) = rho omega^2 (L^2 x - x^3 / 3) / (2 E)` up to rounding, and the resultant
  `rho t omega^2 A r_perp(x_c)` on a distorted plate in space. The plane-stress rotating disc of the brief is a discretisation-
  limited check (polygonal boundary, bilinear radial field) for which no estimate was made here; it is a proposed follow-up,
  not a check of this item.

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0432 (whole), ADR 0426 (whole), ADR 0422 (head, D3, B4), ADR 0429 (head, derivation, checks).
- `crates/rcfd-fem/src`: `shell.rs` (whole), `loads.rs` (whole), `mpc.rs` (module doc, `transformation` with the rigid-set rows,
  `check_transformation_model`, `solve_static_mpc`, `modal_analysis_mpc`, `support_triplets`, `add_supports`, `MpcSolution`,
  `ReferenceMotion`), `free_motions.rs` (whole: no element check, `field_at` gives rotations at components 3 to 5 and the
  rigid-set rotation rows at every six-unknown node, so shell nodes are already counted by its rules), `model.rs`
  (`DofMap::for_mesh`, `check_elements*`, `check_supported*`, `SolidResult`), `solve.rs` (`solve_static_report`: the
  `_with_structures` check, then the dispatch to `solve_static_mpc`), `modal.rs` (`modal_analysis_with`, the same), `thermal.rs`
  (head), `builders.rs` (`hex_box`).
- Tests: `si_shell.rs` (whole: `s1_n2_n3_refusals` has the rows "N2 nodal spring (transformation)" and "N2 gravity on a shell",
  which this item retires), `si_beam_transformation.rs` (the helpers and S2, N3, Z6), grep of every test of `rcfd-fem` and
  `rcfd-post` for `shell::FOLLOW_UP`, `SOLID_ROUTINES` and `Shell4` (no other running test asserts a refusal this item lifts).
- **Not read:** any finite-element code, manual or documentation outside this repository. Everything used is derived below from
  first principles (rigid-body kinematics, the bilinear map, the 1D bar); nothing is cited from memory.

## Retired rows (standing decision: ignored record citing this ADR plus a running `_after_landed` copy)

| file | test | row | why |
| --- | --- | --- | --- |
| `si_shell.rs` | `s1_n2_n3_refusals` | "N2 nodal spring (transformation)": refused naming `shell::FOLLOW_UP` | the transformation path now takes shells (D1) |
| `si_shell.rs` | `s1_n2_n3_refusals` | "N2 gravity on a shell": an `Element` error naming `shell::SOLID_ROUTINES` | gravity on a shell is now taken (D2) |

The test gets `#[ignore = "record of ADR 0432 before ADR 0435 (F-522, shells on the transformation and body loads) retired the
N2 nodal spring and gravity rows; superseded by the s1_n2_n3_refusals_after_landed copy below (maintainer standing decision,
2026-11-13)"]` and nothing else. The copy `s1_n2_n3_refusals_after_landed` keeps every other row as it is and replaces the two
retired rows by: R1, the spring model solves (`mpc::solve_static_mpc`) and the reaction forces at the root, the spring force
and the tip forces balance along `z`, `|sum R_z + S_z + F_z| / |F_z| <= 1e-10`; R2, the gravity model solves and `|sum R +
rho t A g| / (rho t A |g|) <= 1e-8` (the base model is thin, `t = 0.01` on elements of 0.25: the rounding estimate of
arithmetic 5 gives about `1e-10` for it, so the limit of the acceptance is not claimed on this model; the acceptance is G3);
and adds R3, `Load::body_force_nodal` on the same model still refused with an `Element` error naming `shell::SOLID_ROUTINES`
(the solid-element routines of Shell4 still refuse the body loads that this item does not take).

## Arithmetic done before the code (disclosed)

1. **Six unknowns at shell nodes on the transformation.** Shell nodes have six unknowns `(u, theta)` in global axes (ADR 0432
   D1), as beam nodes do. Every row of the transformation is written per equation of `DofMap` (prescribed, linear, spring) or per
   node by its number of components (rigid set: the rows `u_a - t - w x d_a = 0` and, at a six-unknown node, `theta_a - w = 0`,
   ADR 0426 D1). So `mpc::transformation`, `support_triplets`, `add_supports`, the reduction and the expansion need no change for
   shells: the only obstacle is the entry check (`check_supported_with_beams`, which refuses Shell4). The element matrices come
   from `assemble::stiffness_with` and `mass::consistent_with`, which already dispatch Shell4 (ADR 0432 D6) after
   `check_elements_supported_with_structures` (which runs `shell::check_shell_model`).
2. **The consistent gravity load of the bilinear translations (a3).** For a planar facet with the bilinear map `x(xi, eta) = a_0 +
   a_1 xi + a_2 eta + a_12 xi eta` (in in-plane coordinates; `a_1 = (-x_0 + x_1 + x_2 - x_3) / 4`, `a_2 = (-x_0 - x_1 + x_2 + x_3)
   / 4`, `a_12 = (x_0 - x_1 + x_2 - x_3) / 4`), `x_xi = a_1 + a_12 eta`, `x_eta = a_2 + a_12 xi`, so the Jacobian determinant
   is `det = J_0 + J_1 xi + J_2 eta`, `J_0 = a_1 x a_2`, `J_1 = a_1 x a_12`, `J_2 = a_12 x a_2` (2D cross products; the `xi eta`
   term is `a_12 x a_12 = 0`). With `N_i = (1 + xi_i xi)(1 + eta_i eta) / 4`, `int N_i dA = int int N_i det dxi deta = J_0 + (J_1
   xi_i + J_2 eta_i) / 3` and the area `A_e = 4 J_0`. A uniform body force per unit volume `b = rho g` over the thickness `t` gives
   `f_i = rho t g (J_0 + (J_1 xi_i + J_2 eta_i) / 3)`; `sum_i f_i = rho t g A_e` (the reactions of a2), and for a constant `b`
   `sum_i x_i x f_i = (int x dA) x rho t g = A_e x_c x rho t g` (`x_c` the area centroid; the nodal positions interpolate `x`
   exactly). The 2 x 2 Gauss rule integrates `N_i det` (degree 2 per variable) exactly. Distorted patch of ADR 0432 arithmetic
   17 (unit square, 3 x 3 elements, interior nodes moved): area 1, area centroid `(0.5, 0.5)`, the consistent loads differ from
   the equal quarter split `A_e / 4` by up to `3.75e-2` of the largest (script; negative control N1).
3. **A rigid set on an edge equals the clamp (a4).** A rigid set `S` with reference point `x_R` imposes `u_a = t + w x (x_a -
   x_R)` and, at a six-unknown node, `theta_a = w` (ADR 0426 arithmetic 1, D1), for every `a` in `S`. **H1:** one node `a_0` of
   `S` is clamped (`u_{a_0} = 0`, `theta_{a_0} = 0`): then `w = theta_{a_0} = 0` and `t = u_{a_0} - w x d_{a_0} = 0`, so every node
   of `S` has `u_a = 0` and `theta_a = 0`: the six unknowns of every edge node vanish, which is the clamp. Without the rotation
   rows only `t + w x d_{a_0} = 0` would hold and the edge would be free to turn about the line of its collinear nodes (a
   mechanism: the plate would not be held). **H2:** `S` also contains the nodes of a face of a Hex8 block whose every unknown is
   prescribed to zero: `t + w x d_a = 0` at four non-collinear points fixes `t = w = 0` (ADR 0426 arithmetic 6), and the shell
   edge is again clamped. In both, the elimination of ADR 0296 makes every unknown of `S` a dependent with an empty expression
   (value 0) and leaves the plate's other unknowns independent with `T` the identity on them and `g = 0`, so `K_r = T^T K T` is
   the clamped plate's `K_FF` (products by 1 and sums with exact zeros) and `f_r = T^T f` its load: the two solves differ only by
   the order of the factorisation, if at all. ADR 0426 measured bit-equal results for the same situation with beams (its L1
   static, M1 modal). If the orders differ, the difference is of the order of `eps` times the condition of `K_FF` acting on the
   load; ADR 0432 P1 measured `2.6e-11` between two different implementations of the SS plate at `t/a = 0.01`; the plate here has
   `t/a = 0.05` (the condition from shear to bending scales with `(a/t)^2`, 25 times lower): about `1e-12`. Limit (acceptance)
   **`1e-10`**. With the reference point moved to a node (H1b), the rows describe the same rigid motion: the same result.
   **H3 (stronger, the set not held):** a cantilever plate clamped at `x = 0` with a rigid set on its free edge `x = 1` and a
   force and moment at the reference point (`ReferenceLoad`): every tip node moves rigidly, `u_a = t + w x d_a`, `theta_a = w`
   (the dependent expressions evaluated in rounding: `1e-12`), and the root reactions balance the reference load (statics; the
   transformation carries the reference load to the edge as `T^T`, ADR 0426 arithmetic 1).
4. **The spring on a shell rotation (a5).** Strip `L = 1 m` along `x`, `b = 0.2 m`, 4 x 1 elements, `t = 0.05 m`, `E = 2e11`,
   `nu = 0`, `rho = 7850`, in the `x-y` plane (normal `+z`). Root nodes (0 and 5, at `x = 0`): components 0, 1, 2, 3, 5
   prescribed to zero, a nodal spring `k_theta` on component 4 (`theta_y`) of each. `E I = E b t^3 / 12 = 4.1667e5 N m^2`;
   `k_theta = 10 E I / L = 4.1667e6 N m/rad` (a registered choice: stiff enough that the rounding of the plate's shear rows does
   not reach the spring's rotation, below). **K1:** a moment `M_y = 3000 N m` at each root node: the rotation about the root line
   (the `y` axis) is a rigid motion of the plate (no strain energy, ADR 0432 K1) held only by the springs, so `theta_y = M /
   k_theta = 7.2e-4` at every node, `u_z = -theta_y x`, and the spring force `-k_theta theta = -M`. **K2:** a moment `M_y = 1500
   N m` at each tip node instead: by symmetry about the strip's centre line each root spring carries `M = 1500 N m`, `theta_root =
   3.6e-4`; with `nu = 0` the strip is in cylindrical bending under the constant moment `M_tot = 3000 N m`; the constant-curvature
   state with linear `beta` and quadratic `w` is exact for the element (zero tying strains, ADR 0432 arithmetic 4 and its bending
   patch test) and the equal split of the tip moment is the consistent load of a uniform edge moment, so `theta_tip = theta_root +
   M_tot L / (E I) = 7.56e-3` and `u_z,tip = -(theta_root L + M_tot L^2 / (2 E I)) = -3.96e-3 m` at the nodes up to rounding.
   Rounding: a residual `r ~ eps |K| |u|` per row (`|K| ~ k G t = 3.2e9`, `|u| ~ 4e-3`: `r ~ 3e-9`) moves the root rotation by
   about `r / k_theta ~ 1e-15` against `theta_root = 3.6e-4`: about `1e-12` with a factor 10 for the sums. Limit (acceptance)
   **`1e-10`**.
5. **Gravity reactions (a2) and their rounding.** The reaction sum is `-sum_F r_F + (K e)^T u` rounding, `r_F` the residual of the
   free rows: about `eps sum_rows |K| |u|`. Model G3: the distorted patch of arithmetic 2 placed in space by the rotation `Q` of
   0.7 rad about `(1, -2, 2) / 3` and the origin `(0.3, -0.2, 0.5)` (as ADR 0432 arithmetic 17), `t = 0.1 m`, `E = 2e11`, `nu =
   0.3`, `rho = 7850`, `g = (0.5, -1.5, -9.81) m/s^2`, the twelve boundary nodes clamped (all six unknowns): `rho t A |g| = 7800.2
   N`; the centre deflection about `0.00126 q a^4 / D = 5.3e-7 m` (clamped square plate, used only for the size), the shear rows
   `|K| |u| ~ 3.4e3 N` against nodal loads of `856 N`; with 96 rows `eps 3.4e3 96 / 7800 = 9e-15` (script `1.4e-14` with 16 nodes
   and a factor 4). Limit (acceptance) **`1e-12`**, margin about 100. The retired-row copy R2 (the base cantilever of `si_shell`:
   `t = 0.01`, `L = 1`, elements of 0.25, tip deflection about `5.3e-3 m`, `k G t w ~ 3.4e6 N` per row against a total load of 77
   N): `eps 3.4e6 10 / 77 ~ 1e-10`, limit `1e-8`.
6. **Free rigid motions with shell nodes (a6).** The rules of ADR 0426 arithmetic 7 apply unchanged (rotations at components 3
   to 5 of every six-unknown node). Counts by hand on the spring strip of arithmetic 4 without its constraints: Z1 free: 6. Z2
   root translations prescribed: the motions with `u = 0` at two points of the `y` axis are the rotations about that axis: 1. Z3
   Z2 and a nodal spring on `theta_y` of node 0: 0 (were the rotational unknowns read as zero, the spring row would vanish: 1). Z4
   node 0 translations prescribed, its `theta_x` prescribed and `Constraint::Linear theta_z(0) = 0`: the rotations about node 0
   with `w_x = w_z = 0`: 1 (zero-reading: 3). Z5 the plate of H1 with only the rigid set on its edge: 6 (a rigid set holds no
   rigid motion). Z6 the H1 model: 0; the H2 model: 0. Z7: each of Z1's six free fields `X c` times the shell stiffness, relative
   to `max |K| max |X c|`: rounding (ADR 0432 K1 measured `3.6e-16` for one element); limit `1e-12`.
7. **Centrifugal load on shells (t2).** Body force per unit volume `b(x) = rho omega^2 r_perp(x)`, `r_perp` the component of `x -
   x_O` normal to the axis; consistent load `f_i = t int N_i b(x(xi, eta)) dA` by the 2 x 2 rule at the mid-surface points
   `x(xi, eta) = sum N_j p_j` (`p_j` the projected nodes); the integrand `N_i r_perp det` is of degree 3 per variable (bilinear `N_i`,
   bilinear `x`, `det` linear in each), so the rule is exact. **C1:** strip `L = 1 m` along `x` from the axis, `b = 0.1 m`, 8 x 1
   elements, `t = 0.01 m`, `E = 2e11`, `nu = 0`, `rho = 7850`, axis `z` through the origin, `omega = 100 rad/s`; every node's
   components 1 to 5 prescribed to zero and the root nodes' component 0 too. The load's `x` part `rho t omega^2 x` does not depend
   on `y`, the trial field `u_x = U(x)` (equal at the two nodes of a section) has no shear strain and, with `nu = 0` and `u_y = 0`,
   the element reduces exactly to the 1D two-node bar of stiffness `E t b / h` with the consistent load of the linear body force,
   whose nodal values are exact (the 1D Green's function is piecewise linear with kinks at the nodes); the antisymmetric part
   in `y` is not loaded. So `u_x(x_k) = rho omega^2 (L^2 x_k - x_k^3 / 3) / (2 E)` at every node, `u_x,tip = rho omega^2 L^3 / (3
   E) = 1.3083e-4 m`, up to rounding. Limit **`1e-10`** relative to `u_x,tip`. **C1b (stronger):** the nodal loads of C1 against
   the closed form of a rectangle (a node at `x_0` of an element `[x_0, x_0 + h]`: `f_x = rho t omega^2 (b / 2) h (x_0 / 2 + h / 6)`,
   at `x_0 + h`: `(b / 2) h (x_0 / 2 + h / 3)`; `f_y = rho t omega^2 (h / 2) b^2 / 6` at `y = 0` and `b^2 / 3` at `y = b`), summed
   over elements, relative to the largest; limit `1e-12`. **C2:** the distorted patch in space of arithmetic 5 (with `t = 0.1`),
   axis `(1, 2, -0.5)` through `(0.1, 0.2, -0.3)`, `omega = 50 rad/s`: `sum f = rho t omega^2 A r_perp(x_c)` (`r_perp` is affine
   in `x`), `x_c` the area centroid (`Q (0.5, 0.5, 0) +` origin); limit `1e-12` relative to its norm.
8. **The entry refusal before the transformation is built (pattern of ADR 0429).** `solve_static_mpc` and `modal_analysis_mpc`
   call `check_transformation_model` before `transformation`, which runs before `stiffness_with` (whose own shell check would
   otherwise refuse the shared node later). A model with a shell node shared with a solid and two `Linear` rows on one unknown,
   `u = 0` and `u = 1e-3` (set A), would give the contradiction from `transformation` if the entry check were missing; one
   cancelling row `u - u = 0` (set B) gives "cancel to zero" from `transformation` on the modal path. With the entry check, both
   paths give the shell-solid refusal `Element { element: shell, InvalidValue { key: "node" } }` naming the shared node. Controls
   without the shared node give the contradiction texts themselves.
9. **Body load on a warped facet (G5).** D2 applies the body force at the projected mid-surface points `p_i = x_i - h_i e_3`
   (ADR 0432 arithmetic 1) and carries it to the nodes as `(F_i, r_i x F_i)`, `r_i = p_i - x_i`, `F_i = t int N_i b dA`. Then
   `sum_i F_i = t int b dA` and the moment about the origin is `sum_i (x_i x F_i + r_i x F_i) = sum_i p_i x F_i = t int x x b(x)
   dA` (the projected points interpolate the mid-surface, `sum N_i p_i = x`), both over the planar quadrilateral of the `p_i`
   (area `A_p = |d_1 x d_2| / 2`). Gravity: `sum F_i = rho t g A_p`, moment `A_p x_p x rho t g` (`x_p` the area centroid).
   Centrifugal (`b = rho omega^2 P (x - x_O)`, `P = I - a a^T`): `sum F_i = rho t omega^2 P (A_p x_p - A_p x_O)`, moment
   `rho t omega^2 (epsilon_kij (S P^T)_ij - A_p x_p x P x_O)` with `S = int x x^T dA`; the test computes `A_p`, `x_p`, `S` by two
   triangles (`int_T x x^T dA = (A_T / 12)(sum v v^T + (sum v)(sum v)^T)`), independently of the element's rule. Without the
   offset moments (MU11) the moment would differ by `sum_i h_i e_3 x F_i = h e_3 x sum_i s_i F_i` (`h_i = s_i h`, `s_i = xi_i
   eta_i`, so `sum_i s_i N_i = xi eta`): for a uniform `b` this is `h e_3 x t b int xi eta det dxi deta = 0` (`det` is linear in
   each variable), so **gravity cannot see the offset moments**; for the centrifugal load `b` contains `x(xi, eta)`, whose `xi eta`
   term (`a_12`) makes it non-zero on a non-parallelogram. Registered facet: nodes `(0, 0, 0)`, `(1.2, 0, 0)`, `(1.0, 0.8, 0.12)`,
   `(0.1, 1.0, 0)`, `t = 0.1`, `rho = 7850`; script: offsets `+-0.0381`, warp ratio `0.0392` (below 0.05), `A_p = 0.94422`;
   centrifugal about `z` through the origin, `omega = 50 rad/s`: the element's rule against the triangle closed forms `2.0e-16`
   (resultant) and `1.9e-17` (moment, relative to `max |x_i|` times the resultant), the moment without the offsets `8.7e-4`.

## Design

### D1. The transformation takes shells (`mpc.rs`)

`check_transformation_model` calls `model.check_supported_with_structures()` instead of `check_supported_with_beams()` (a Shell4
block passes; `shell::check_shell_model` refuses a block without its property, a shell node shared with a solid and an invalid
facet, ADR 0432 D4). The cyclic refusal is extended: a model with a Beam2 block and a `Constraint::Cyclic` is refused as before
(naming `beam::FOLLOW_UP`, unchanged text); a model with a Shell4 block (and no Beam2) and a `Constraint::Cyclic` gives
`NotYetSupported { item: "constraint", variant: "Cyclic", location: "constraint i, a model with Shell4", package:
shell::FOLLOW_UP }` (the cyclic rows rotate translations only). Nothing else in `mpc.rs` changes (arithmetic 1). The module doc
names this ADR. The comments of `solve_static_report` and `modal_analysis_with` that say the transformation path refuses shells
are corrected (comments only). Every other user of the transformation (buckling, cyclic, geometric, harmonic, transient,
nonlinear, rotating) keeps its own `check_supported`, so it keeps refusing shells naming `shell::FOLLOW_UP`.

### D2. Body loads on shells (`shell.rs`, `loads.rs`)

New `shell::body_load_vector(coords, thickness, b) -> Result<[f64; 24]>`: the consistent load `F_i = t sum_gp w det N_i b(x_gp)`
in global components at the projected points (`x_gp = sum N_j p_j`), carried to the nodes by `T^T` of ADR 0432 arithmetic 7
(forces `F_i` and, on a warped facet, moments `r_i x F_i`). `loads::add_body_load_with` takes a second optional rule, for Shell4
elements, `Fn(usize, Vec3) -> Result<Vec3>` (the body force per unit volume of element `e` at a point); with it a Shell4 element
gets `body_load_vector` with its block's thickness (refused naming the element without a Shell property), forces and moments
added at its nodes; without it, or for every other element, exactly the former code. `Load::Gravity` passes the shell rule
`rho_e g`; `Load::Centrifugal` passes `rho_e omega^2 r_perp(x)` (the same `perpendicular` function as the solids). The beam rule
of gravity is unchanged; centrifugal on a beam and `Load::BodyForceNodal` and `Load::Temperature` on a shell are not taken (they
keep refusing through the solid-element routines, naming `beam::SOLID_ROUTINES` / `shell::SOLID_ROUTINES`). The module docs
name this ADR.

### D3. What needs no change

`free_motions.rs` (arithmetic 6: no element check, the rotation rules hold at shell nodes; its doc gets one sentence naming
this ADR), `transformation`, `support_triplets`, `add_supports`, `ReferenceLoad`, reactions and constraint forces at rotational
equations, `DofMap`.

### D4. Bits

A model without Shell4: D1's check is the same sequence (`check_elements(true, true)` differs from `check_elements(true, false)`
only on a Shell4 block; `check_shell_model` returns at once without one) and the cyclic refusal of a beam model is the same; D2's
shell branch is entered only for a Shell4 element; the solid and beam expressions and the element order are unchanged. So every
model without shells gives the same bits (B1 to B4). Shell models on the elimination path without gravity or centrifugal loads
are untouched.

## Checks (limits fixed now)

| id | clause | check | limit | estimate behind it |
| --- | --- | --- | --- | --- |
| G1 | a2, a3 | gravity load vector of the distorted patch in space (arithmetic 5): `|sum f - rho t A g| / (rho t A |g|)`; `|sum (x_i x f_i + m_i) - A x_c x rho t g| / (max |x_i| rho t A |g|)` | `1e-12` each | arithmetic 2: rounding `~1e-15` |
| G2 | a3 | each node's force against `rho t g sum_e (J_0 + (J_1 xi_i + J_2 eta_i) / 3)` (closed form in the test, patch coordinates, then `Q`), relative to the largest force; each nodal moment relative to the largest force times 1 m | `1e-12` | arithmetic 2 |
| G3 | a2 | the patch with its 12 boundary nodes clamped, `solve_static` under gravity: `|R + rho t A g| / (rho t A |g|)`, `R` the sum of the reaction forces | `1e-12` | arithmetic 5: `~1e-14` |
| G3m | (stronger, a2) | the reaction moments about the origin: `|sum (x_a x R_a + R^theta_a) + A x_c x rho t g| / (max |x_a| rho t A |g|)` | `1e-12` | arithmetic 5 |
| G5 | (stronger, a3; t2) | the warped facet of arithmetic 9: gravity `(0.5, -1.5, -9.81)` and, separately, the centrifugal load about `z` through the origin at `omega = 50`: the resultant against the closed form, relative to its norm; the moment of the nodal forces and moments about the origin against the closed form, relative to `max |x_i|` times the resultant's norm | `1e-12` each | arithmetic 9: `2e-16` |
| G4 | a7 | a Hex8 box (`hex_box([0,0,0], [1,1,1], [2,2,2])`) with a separate shell plate (nodes appended, not shared) under gravity and, separately, under a centrifugal load: the entries of the box's equations equal those of the box alone, bit for bit | exact | D4 |
| H1 | a4 | plate `[0,1] x [0,1]`, 4 x 4 elements, `t = 0.05`, `nu = 0.3`, `rho = 7850`, `Load::shell_pressure("all", 1000)`; rigid set on the five nodes of `x = 0`, reference `(0, 0.5, 0)`, node 0 clamped: every unknown against the clamped plate (elimination path), relative to the largest of each field (translations, rotations) | `1e-10` | arithmetic 3: bit-equal or `~1e-12` |
| H1b | (a4) | H1 with the reference point at `(0, 0, 0)` | `1e-10` | arithmetic 3 |
| H2 | a4 | the plate joined by a rigid set to the `xmax` face of a fixed Hex8 block (`hex_box([-0.2, 0, -0.1], [0.2, 1, 0.2], [1, 2, 1])`, every block node prescribed on components 0 to 2), reference `(0, 0.5, 0)`: every plate unknown against the same mixed model with the plate edge clamped (elimination path) | `1e-10` | arithmetic 3 |
| H2m | (stronger, a4) | modal analysis of H2 and of its clamped counterpart, `ModalOptions::new(4).tolerance(1e-12).residual_test(ResidualTest::Backward)`: the four lowest frequencies, relative | `1e-8` each | arithmetic 3 (ADR 0426 M1: bit-equal) |
| H3 | (stronger, a4) | the plate clamped at `x = 0`, rigid set on the five nodes of `x = 1`, reference `(1, 0.5, 0)`, `ReferenceLoad` force `(150, 300, 500) N`, moment `(100, -40, 25) N m`: each tip node `u_a - t - w x d_a` relative to the largest tip translation and `theta_a - w` relative to `|w|` | `1e-12` | arithmetic 3 |
| H3r | (stronger, a4) | H3: root reaction forces plus the reference force, relative to `|F|`; root reaction moments about the origin plus `x_R x F + M`, relative to `|x_R| |F| + |M|` | `1e-10` each | statics; rounding `~3e-12` (arithmetic 5's rule) |
| K1 | a5 | spring strip (arithmetic 4), moment at the root nodes: `|k_theta theta_y - M| / M` at each root node; the spring forces `|S + M| / M` (`MpcSolution::support_forces`); every node's `theta_y` against `theta_root` and `u_z` against `-theta_root x`, relative | `1e-10` each | arithmetic 4: `~1e-12` |
| K2 | a5 | moment at the tip nodes: `|k_theta theta_root - M| / M` at each root node; `theta_y,tip`, `u_z,tip` against arithmetic 4, relative | `1e-10` each | arithmetic 4 |
| Z1 to Z6 | a6 | counts of arithmetic 6: 6, 1, 0, 1, 6, 0 (H1), 0 (H2) | exact | arithmetic 6 |
| Z7 | a6 | Z1's free fields in the null space of the shell stiffness | `1e-12` | arithmetic 6 |
| C1 | t2 | spinning strip (arithmetic 7): `max_k |u_x(x_k) - u(x_k)| / u_x,tip` over every node | `1e-10` | exact element, rounding |
| C1b | (t2) | C1's nodal loads against the rectangle closed form | `1e-12` | arithmetic 7 |
| C2 | t2 | centrifugal resultant on the distorted patch in space | `1e-12` | arithmetic 7 |
| S3 | (D1) | shared model (a Shell4 element with a Shell property on four nodes of the `xmax` face of `hex_box([0,0,0], [2,1,1], [2,1,1])`, `xmin` held), set A on `u(first shared node, 0)`: `solve_static_mpc` refused with `Element { element: 2, InvalidValue { key: "node" } }` whose text names `node {first} `; `modal_analysis_mpc` with set A and with set B: the same | exact | arithmetic 8 |
| S3c | (control of S3) | the same constraints with the shell on four new nodes at `x = 3` (not shared; constraint on its first node): `solve_static_mpc` set A `InvalidValue { key: "constraint" }` containing "contradicts"; `modal_analysis_mpc` set A "takes zero values only", set B "cancel to zero" | exact | arithmetic 8 |
| R1 | (retired row) | the `si_shell` copy: spring model solves; `|sum R_z + S_z + F_z| / |F_z|` | `1e-10` | rounding (estimate `~1e-12`) |
| R2 | (retired row) | the `si_shell` copy: gravity model solves; `|R + rho t A g| / (rho t A |g|)` | `1e-8` | arithmetic 5: `~1e-10` |
| R3 | (retired-row copy) | `Load::body_force_nodal` on the base shell model refused with `Element { source: NotYetSupported { package: shell::SOLID_ROUTINES } }` | exact | D2 |
| B1 | a7, b1 | `si_scaffold_bits.rs` passes unedited | pass | D4 |
| B2 | a7, b1, b2 | every existing `rcfd-fem` test passes, unedited except the retired test (ignored) whose copy passes | pass | D4 |
| B3 | a7 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | D4 |

Negative controls:

| id | control | expected | limit |
| --- | --- | --- | --- |
| N1 | G2's consistent loads against the equal quarter split `rho t g A_e / 4` per element | they differ (the check sees the consistent load) | `> 1e-3` relative to the largest force (arithmetic 2: `3.75e-2`) |
| N2 | a shell model (the spring strip with a Hex8 box `hex_box([0, 1, 0], [1, 1, 1], [1, 1, 1])` added, separate) with `Constraint::cyclic("ymin", "ymax", z, 0, 4)` on `solve_static` and `modal_analysis` | `NotYetSupported { variant: "Cyclic", package: shell::FOLLOW_UP }` | exact |
| N3 | the H1 model on paths that do not take shells: `check_supported()` (the guard of cyclic, harmonic, transient, rotating, buckling); `nonlinear::initial_states`; `solve_static` with the block-Jacobi preconditioned CG (the transformation) | `NotYetSupported` naming Shell4 and `shell::FOLLOW_UP`; the same; `InvalidValue { key: "solver" }` (unchanged) | exact |
| N4 | a beam model (two Beam2 elements, the section of ADR 0426 arithmetic 3) under `Load::centrifugal` | `Element { source: NotYetSupported { package: beam::SOLID_ROUTINES } }` (unchanged) | exact |

## Planned mutations (on a scratch copy, one at a time)

| id | mutation | expected to be caught by |
| --- | --- | --- |
| MU1 | the shell rule of gravity left out (`None` passed) | G1, G2, G3 (refused through the solid routines) |
| MU2 | the shell gravity load as the equal quarter split `rho t g A_e / 4` | G2 |
| MU3 | the thickness left out of `body_load_vector` | G1, G2, G3, C1, C2 |
| MU4 | the centrifugal shell rule evaluated at the element's vertex centroid for every Gauss point | C1b, C2 (C1 possibly) |
| MU5 | `omega` instead of `omega^2` in the centrifugal shell rule | C1, C1b, C2 |
| MU6 | `check_transformation_model` back to `check_supported_with_beams` | H1, H2, H3, K1, K2, R1 |
| MU7 | the Shell4 cyclic refusal of D1 removed | N2 |
| MU8 | the rotation rows `theta_a - w = 0` of a rigid set left out (`mpc::transformation`) | H1 (mechanism: refused or wrong), H2, H3 |
| MU9 | `check_transformation_model(model)?` removed from `solve_static_mpc` | S3 (static) |
| MU10 | `check_transformation_model(model)?` removed from `modal_analysis_mpc` | S3 (modal) |
| MU11 | the moments `r_i x F_i` of `body_load_vector` dropped (forces only) | G5 (the centrifugal moment row, `8.7e-4`; the gravity row cannot see it, arithmetic 9) |

## Decision rule

- **done:** every check of the acceptance (G1, G2, G3, H1, H2, K1, K2, Z1 to Z7, B1 to B3) and the stronger checks (G3m, G4,
  G5, H1b, H2m, H3, H3r, C1, C1b, C2, S3, S3c, R1 to R3) met, N1 to N4 as stated, every mutation caught by a registered check.
- **partial:** a clause missed (recorded as a `*_known_miss`) or not run, a mutation not caught, or a check met only after an
  amendment written after its run.
- **todo:** nothing of the path builds.

## Limits (not covered)

- L1. Shells on the cyclic, harmonic, transient, rotating, buckling, geometric-stiffness and nonlinear paths and with the
  multigrid preconditioner stay refused (each its own package). The harmonic transformation's rigid-set rows are not extended.
- L2. `Load::BodyForceNodal` and `Load::Temperature` on shells stay refused (solid-element routines); stress recovery of shells is
  `shell::stresses` only. Foundations (`ElasticSupport::Foundation`) act on solid faces only.
- L3. Body loads on warped facets: the resultant and its moment are checked on one facet (G5); the accuracy of a warped-facet
  mesh under body loads is not checked (as ADR 0432 L3 for its other loads).
- L4. A shell node shared directly with a solid stays refused (ADR 0432 D4); the joint is the rigid set (H2). A pinned joint
  (translations only) is a set of linear rows written by the user (ADR 0426 L2).
- L5. The rotating plane-stress disc of the brief is not a check of this item (discretisation-limited; follow-up proposed).

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. Centrifugal loads on shells included (t2), checked by two closed forms that are exact for the element; the rotating disc left
   to a follow-up. Recommended and taken.
2. A shell model with a cyclic constraint refused on the transformation path naming `shell::FOLLOW_UP` (D1) rather than solved
   with translation-only cyclic rows. Recommended and taken (as ADR 0426 open question 2 for beams).
3. The body force on a shell evaluated at the mid-surface points of the projected facet (D2), so that a warped facet's load acts
   where its stiffness acts (the rigid offsets of ADR 0432 arithmetic 7). Recommended and taken.

## Amendment 1 (2026-11-14, written before any run of a check)

Found on writing the tests, before any of them was run:

1. **R2, set-up.** The base model of `si_shell`'s retired test (`cantilever_model`) carries a tip force of `(0, 0, 1) N` at each
   of its two tip nodes; the retired gravity row adds gravity to it. The copy keeps that model, so the balance is `|R + rho t A g
   + F| / (rho t A |g|)` with `F = (0, 0, 2) N` (`A = 0.1 m^2`, `t = 0.01 m`). Limit unchanged (`1e-8`).
2. **Parameters not stated above, fixed now:** G4's shell plate is `[0, 1]^2` at `z = 2`, 2 x 2 elements, `t = 0.02`, steel
   `nu = 0.3`, its nodes after the box's; the centrifugal load of G4 is about `z` through `(0.3, 0.2, 0)` at `omega = 40`. K1's
   `u_z = -theta_root x` row is relative to `theta_root L` (`L = 1`). The H2 block and plate share the material (steel, `nu =
   0.3`); the pressure acts on the element set of the plate. N2's strip has `nu = 0`, the box `nu = 0.3`. N4's beam is two Beam2
   elements from the origin along `x` (`L = 1`, orientation `(0, 1, 0)`), clamped at the origin, `omega = 10` about `z`. The S3
   models have the Shell4 element as element 2 (after the two Hex8 elements), thickness `0.01`.

No limit changes.
