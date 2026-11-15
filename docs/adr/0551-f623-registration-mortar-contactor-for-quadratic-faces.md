# ADR 0551: F-623 registration: a mortar contactor for Hex20 and Tet10 faces

Status: **REGISTERED 2026-11-15, written before any code of F-623 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-623 was written or run in the repository. The arithmetic below was done by hand (no
rcfd code, no script). The bit record B2 of the default Gauss-point path is a test file written and run on the base after this
commit and before any library code (D7). Results go in the addenda at the end; nothing above the addenda is changed after a
run.

Roadmap item F-623 (deps F-601), the follow-up proposed by [ADR 0540](0540-f601-registration-contact-search-rigid-cylinder-and-sphere-targets-hex20-and-tet-faces.md)
(its remaining questions 2 and 3: the known misses HS20 and HS10, and the total Lagrangian body with the new faces). Number
0551 given by the lead. Branch `f623-work` from e906faa8 (main), worktree `w49i-work/f623`, scratch `w49i-work/f623-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no
unsafe code, the crate graph unchanged; clean-room; `docs/ROADMAP.md` and `docs/roadmap.json` not edited (the entries are
proposed at the end of the addendum). This item changes `crates/rcfd-fem/src/contact.rs` only, adds test files, and
documentation. No existing test is edited.

## What F-623 asks

The acceptance, verbatim, binding:

> Every limit not stated here is registered in the ADR before the run. A segment-to-segment (mortar) contactor for Hex20 and Tet10 faces that constrains one weighted gap per face node: the Hertz problem of ADR 0540 HS20 and HS10 on Hex20 and Tet10 meets its limits (radius 0.15, peak 0.20, approach 0.10) with the gap tolerance 1e-8, the patch tests of ADR 0540 PT10 and PT20 within 1e-8, the tangent within 1e-6, the total Lagrangian body included; defaults bit-identical.

Clauses: **c1** every other limit registered here before the run; **c2** a mortar contactor for Hex20 and Tet10 faces with one
weighted gap per face node; **c3** HS20 and HS10 meet radius 0.15, peak 0.20, approach 0.10 with the gap tolerance `1e-8`;
**c4** PT10 and PT20 within `1e-8`; **c5** the tangent within `1e-6`; **c6** the total Lagrangian body included; **c7** defaults
bit-identical.

## What was read (no run)

ADR 0540 (whole: D1 to D9, the checks, addendum 1 with the HS20 and HS10 misses and their diagnosis, the remaining questions),
ADR 0507 (D1, D2, D4, amendments 2 and 3), ADR 0542 D3 (the supernodal factor passed to the total Lagrangian body). Code:
`crates/rcfd-fem/src/contact.rs` (whole), `loads.rs` (`face_shape_functions` for 3, 4, 6, 8 nodes, `face_rule`), `hex20.rs`
(`face_shape_functions`, `face_rule`), `nonlinear.rs` (`NewtonOptions::new`: residual and energy tolerances `1e-10`; loads are
dead loads on the reference geometry), `builders.rs` (`tet_box`, `upgrade_to_tet10`), the tests `sa_contact_faces_f601.rs`,
`sa_contact_targets_f601.rs`, `f601_common/mod.rs` (the Hertz block, the Boussinesq values, `hertz_read`, `hertz_theory`,
`tangent_error`, `result_words`), `sa_contact.rs` (T2, the total Lagrangian punch), the logs of ADR 0540 in
`w49i-work/f601-scratch` (`run-faces.log`: the test binary of the faces, HS20 and HS10 failing after 20 augmentations each,
took 190 s). Nothing outside the repository.

Findings that shape the design:

1. ADR 0540's Gauss-point contactor puts 9 gap constraints on every Quad8 or Tri6 face (8 or 6 nodes); against the rigid
   sphere they cannot all be zero at once, and the augmentation stalls (the diagnosis of ADR 0540 addendum 1). A contactor with
   one constraint per contactor node has as many constraints as normal unknowns of the face nodes.
2. The obvious weights, the integrals `int N_a dA` of the face's own shape functions, are `-A/12` at a Quad8 corner and `0` at
   a Tri6 corner of a flat face (ADR 0540 finding 3, NG): a constraint with a negative or zero weight cannot carry a positive
   uniform pressure. The weighting functions must be changed.
3. The node contactor of the driver needs, per contactor entry, a gap, its gradient and second derivative over a list of
   nodes, an area (the penalty `eps A`) and the multiplier: the weighted gap of a node fits these slots unchanged (D2).
4. A uniform pressure load uses `loads::face_rule` (3 x 3 Gauss on Quad8, the collapsed 3 x 3 rule on Tri6). The collapsed
   rule `u = s (1 - r)`, `v = s r`, weight `w_s w_r s` integrates exactly every polynomial of total degree 4 on the triangle (a
   degree-`p` polynomial becomes degree `p + 1` in `s` and `p` in `r`, the 3-point Gauss rule is exact to degree 5); the 3 x 3
   rule integrates exactly degree 5 per direction. Hence the products `N_a N_b` of quadratic functions (degree 4, or degree 4
   per direction for Quad8) are integrated exactly on flat faces (arithmetic 2).

## Design

### D1. The weighting functions (c2)

For a contactor face with shape functions `N_a` (corners `a < c`, `c = 4` for Quad8, `3` for Tri6, then the mid-edge nodes in
the order of `loads::face_shape_functions`: Quad8 edges `(0,1), (1,2), (2,3), (3,0)` as nodes 4 to 7; Tri6 edges `(0,1), (1,2),
(2,0)` as nodes 3 to 5), the weighting functions are the transformed basis

    Phi_corner(i) = N_i + alpha (N_{c+i} + N_{c+(i+c-1) mod c}),     Phi_edge(e) = (1 - 2 alpha) N_e,     alpha = 1/4,

where `c+i` and `c+(i+c-1) mod c` are the two mid-edge nodes of the edges at corner `i`. Derivation (first principles): every
edge node is adjacent to exactly two corners, so `sum_a Phi_a = sum_corners N + 2 alpha sum_edges N + (1 - 2 alpha) sum_edges N =
sum_a N_a = 1`: the `Phi_a` are a partition of unity (needed for the patch test, D5). On a flat (affine) face of area `A`:
Tri6 `int N_corner = 0`, `int N_edge = A/3`, so `int Phi_corner = 2 alpha A/3`, `int Phi_edge = (1 - 2 alpha) A/3`; Quad8 `int
N_corner = -A/12`, `int N_edge = A/3`, so `int Phi_corner = -A/12 + 2 alpha A/3`, `int Phi_edge = (1 - 2 alpha) A/3`. Positivity
of both needs `1/8 < alpha < 1/2` (Quad8) and `0 < alpha < 1/2` (Tri6). Taken `alpha = 1/4`: the six Tri6 integrals are then
equal (`A/6` each), the Quad8 ones `A/12` (corner) and `A/6` (edge), all positive, one value for both shapes. The idea of a basis
transformation that makes the quadratic mortar weights positive is in the literature on quadratic mortar contact (Wohlmuth;
Popp, Wohlmuth, Gee and Wall, SIAM J. Sci. Comput. 34 (2012) B421; both from memory, not read); the transformation and the value
of `alpha` used here are derived above and not taken from those papers. The map `N -> Phi` is invertible (`1 - 2 alpha = 1/2 !=
0`), so the mortar matrix `int Phi_a N_b` is the face mass matrix times an invertible matrix, nonsingular.

### D2. The weighted gap of a node (c2)

For a pair whose contactor faces are Hex20 or Tet10 faces and whose options ask for the mortar contactor, the contactor
entries are the **nodes** of the contactor faces (sorted ascending, each once). The integrals are taken by the load rule of each
face (element-based integration over the contactor face, no segmentation of the target: the Gauss-point integration of mortar
integrals, e.g. Fischer and Wriggers, Comput. Mech. 36 (2005) 226, from memory, not read): at the rule's point `zeta_g` of a face
the reference weight `W_g = w_g |X_{,u} x X_{,v}|` (as ADR 0540 D3) and the point's gap `g_g` against the target, with its
gradient and second derivative, exactly as the Gauss-point contactor of ADR 0540 D3 computes them (the same search, the same
exclusion of target faces sharing a node of the contactor face, the same projection and gap from differences). Per node `a`:

    A_a = sum_{faces f containing a} sum_g Phi_a^f(zeta_g) W_g          (the node's area, reference configuration, > 0)
    c_ag = Phi_a^f(zeta_g) W_g / A_a                                    (sum_g c_ag = 1)
    gbar_a = sum_g c_ag g_g,   d gbar_a = sum_g c_ag d g_g,   d2 gbar_a = sum_g c_ag d2 g_g,

the sums over the points of the faces containing `a`, the gradients and second derivatives over the union of the nodes of those
points' kinematics (the contactor faces' nodes and the target faces' nodes, in order of first appearance). `gbar_a` is the
weighted gap `int Phi_a g dA` divided by `int Phi_a dA` (a length, m): the gap tolerance of the driver applies to it, as it
applies to a node's gap. The weights are fixed in the reference configuration (as the node areas of ADR 0507 D4 and the
Gauss-point weights of ADR 0540 D3), so `gbar_a` is a function of the positions only and its derivatives are exact sums of the
points' exact derivatives (the energy of D3 is a potential; the tangent is its exact second derivative).

A node has kinematics when every point of every face containing it has kinematics (a target within reach); otherwise it has no
gap (inactive), as a node outside every target face. Its reported normal is `m_a / |m_a|` with `m_a = sum_g c_ag n_g` (the
force of the node on the body against a rigid target is `t_a m_a`, since `sum_b M_b = 1` over the contactor face's shape
functions `M_b`); its face is `None`, its local coordinates `(0, 0)`, its `point` `None`, its `area` `A_a`.

### D3. The augmented Lagrangian (unchanged)

Each node is one entry of the existing augmented Lagrangian (ADR 0507 D2): the multiplier `lambda_a`, the penalty `eps_a =
penalty A_a`, the force `t_a = max(0, lambda_a - eps_a gbar_a)`, the residual contribution `-t_a d gbar_a`, the tangent `eps_a d
gbar_a d gbar_a^T - t_a d2 gbar_a`, Uzawa updates and the gap tolerance on `|gbar_a|` at the active nodes (amendment 3). The
driver, the search, the cycle record and the linear and total Lagrangian bodies are unchanged.

### D4. The option (c7)

`ContactOptions` (`#[non_exhaustive]`) gains `pub quadratic_contactor: QuadraticContactor` with `enum QuadraticContactor {
GaussPoints, Mortar }` and the builder `quadratic_contactor(..)`; **`GaussPoints` is the default** (ADR 0540 D3 unchanged), so
every existing call produces the same bits (B2 and the existing records). `Mortar` changes pairs whose contactor faces are Hex20
or Tet10 faces; pairs of Hex8 or Tet4 contactor faces keep their node contactors (one gap per node already). With `Mortar`:
initial-gap offsets on a quadratic contactor are refused as for the Gauss points (key `"initial gap"`); a node with `A_a <= 0`
or not finite (a strongly distorted face) is refused (key `"contact surface"`). Mortar pairs take no Gauss-point ids
(`n_nodes + k` counts only Gauss-point pairs). The test hook `visited_faces` counts the faces visited by the points of a mortar
pair.

A second test hook, `#[doc(hidden)] ContactSet::convex_tangent(u)`, returns the triplets of the contact matrix without its
curvature part (`eps G G^T` only, the matrix of ADR 0507 amendment 2) at `u`, for the negative controls NT-m.

### D5. Why the patch test passes (c4)

Matching flat meshes under a uniform pressure `p`, the uniform stress state: every point has the gap 0 (the target face's
closest point is the same physical point) and the target normal `n`. With `t_a = p A_a` the force on the contactor's node `b`
is `sum_a t_a sum_g c_ag M_b(zeta_g) n = p sum_g (sum_a Phi_a(zeta_g)) W_g M_b(zeta_g) n = p sum_g W_g M_b(zeta_g) n` (partition
of unity), the consistent pressure load of `loads` by the same rule, exactly; the target face's node `b'` receives `-p sum_g W_g
N_b'(xi_g) n` at the same physical points, its consistent load. The uniform stress is then the solution, `gbar_a = 0` at every
node, `t_a = p A_a > 0` (D1), the reported pressure `t_a/A_a = p`. The penalty solve gives the uniform penetration `p/eps`, the
next solve the zero gap (as ADR 0507 B1). Under the total Lagrangian body (dead loads on the reference geometry, contact
weights on the reference geometry) the homogeneous deformation of both blocks keeps the meshes matching and the target normal
`e_z`; the nominal traction is `p` on both sides, so again `t_a = p A_a` (the stress is not `-p` then: the second
Piola-Kirchhoff stress differs at the order of the strain `5e-4`, so the TL patch test checks pressures and reactions only).

### D6. The models of the checks

As ADR 0540 D5 unless stated, with `.quadratic_contactor(QuadraticContactor::Mortar)`.

- **PT10m, PT20m** (c4). ADR 0540 PT10 and PT20 (the blocks of `tet_box` upgraded by `upgrade_to_tet10`, and of `hex_box`
  upgraded by `upgrade_to_hex20`; `E = 2e11`, `p = 1e8`, `eps = 100 E/0.5`, 2 steps, tolerance `1e-13`, Newton
  `NewtonOptions::new(1).max_iterations(30)`), solved with both search methods (EQm). Contactor nodes: PT20 `upper.zmin` is
  2 x 2 Quad8 faces, `5 x 5 - 4 = 21` nodes; PT10 is 8 Tri6 faces (ADR 0540 measured 72 = 8 x 9 points), 9 corner nodes and 16
  mid-edge nodes (12 grid edges and 4 diagonals) = 25.
- **PW** (D1). The areas `A_a` of PT20m and PT10m against the closed form: per face of area `A` (`0.25` for a Quad8 face,
  `0.125` for a Tri6 face, both flat) `A/12` at a Quad8 corner, `A/6` at a Quad8 edge node, `A/6` at every Tri6 node, summed over
  the faces holding the node (computed in the test from the mesh's faces).
- **PT20t, PT10t** (c4, c6). PT20m and PT10m with `.kinematics(Kinematics::TotalLagrangian)` (the same set-up otherwise; all
  model constraints are zero, as the total Lagrangian driver requires).
- **HS20m, HS10m** (c3). ADR 0540 HS20 and HS10 exactly (`hertz_block(4, 5, 4)` upgraded to Hex20, `h = a/4`;
  `hertz_block(3, 4, 4)` split by `split_hexahedra` and upgraded to Tet10, `h = a/3`; the rigid sphere `centre (0, 0, R)`, radius
  `R`; the Boussinesq values and the symmetry constraints; penalty `100 E/h`; 1 step; tolerance `1e-8`; 20 augmentations; the
  default factor), with the mortar contactor. The readings of ADR 0540 HS (`f601_common::hertz_read` and `hertz_theory`): the
  fit `p^2 = c_0 + c_1 r^2` over the contactor entries with `p >= 0.5 max p` (here nodes at their reference positions, the
  pressure `t_a/A_a`), `a_fe = sqrt(-c_0/c_1)`, the peak `sqrt(c_0)`; the force **`F_fe = 4 |sum of the z reactions|`** over the
  prescribed equations (the equilibrium of the quarter body; there is no other load), in place of `4 sum t |n_z|` (which for a
  mortar node is the force of its averaged normal); the latter is also read, check FR.
- **HS20t, HS10t** (c3, c6; heavy records, ignored, run once). HS20m and HS10m with `.kinematics(Kinematics::TotalLagrangian)`
  and `.factor(DirectFactor::SupernodalNestedDissection)` (ADR 0542 D3; the envelope factor of the default Newton options is too
  slow at 11 000 unknowns), against the same small-strain Hertz values and the same limits.
- **Tangent** (c5). ADR 0507 T1's arrangement as `sa_contact_faces_f601.rs` (`tangent_error`: `eps = 10`, `u_i = 0.01 sin(1.7 i
  + 0.3)`, central differences `1e-6`, every entry active with the same face at every perturbed state), multipliers `0.1 + 0.01
  k` over the face's nodes (`k = 0 .. 7` for the Hex20 face, `0 .. 5` for the Tet10 face):
  - **M20w** ADR 0540 T1g20's model (the Hex20 contactor with its lowered mid-edge nodes against T1's warped Hex8 target);
  - **M10w** ADR 0540 T1g10's model (the Tet10 contactor against the warped Hex8 target);
  - **M20s** the Hex20 contactor of M20w against `RigidSphere { centre (0.5, 0.45, 0), radius 0.97 }` (ADR 0540 T1s's sphere);
  - **M10s** the Tet10 contactor of M10w against the same sphere;
  - **M20q** the Hex20 contactor of M20w against ADR 0540 T1q's curved Hex20 target (T1's lower hexahedron as a Hex20, its top
    mid-edge nodes raised by `(0.03, -0.02, 0.025, -0.015)`), both blocks Hex20.
- **R4** (D4). Refused with `Mortar`: an initial gap on a Hex20 contactor (key `"initial gap"`), and a contactor set mixing a
  Hex8 and a Hex20 face (key `"contact surface"`, as ADR 0540 R3).

## Arithmetic done before the code (disclosed)

1. **Weights** (D1): Tri6 `2 alpha/3 = 1/6`, `(1 - 2 alpha)/3 = 1/6`; Quad8 `-1/12 + 1/6 = 1/12`, `1/6`; sums `6/6 = 1` and
   `4/12 + 4/6 = 1`. PT20's node areas (face area `A = 0.25`): the centre corner `4 A/12`, the side corners `2 A/12`, the outer
   corners `A/12`, the 4 interior edge nodes `2 A/6`, the 8 boundary edge nodes `A/6`; total `4 A = 1`.
2. **Exactness of the rules** (finding 4): degree 4 on Tri6 (collapsed 3 x 3), degree 5 per direction on Quad8 (3 x 3); the
   areas of PW are therefore exact up to rounding: limit `1e-14` relative.
3. **Patch tests**: as ADR 0540 arithmetic 5, the uniform stress is the discrete solution; rounding `1e-14` (ADR 0540 measured
   `4e-14` with the Gauss points). Limit `1e-8` (the acceptance).
4. **Hertz with the mortar contactor**: the constraints are as many as the nodes, the mortar matrix nonsingular (D1), so a zero
   weighted gap at every active node is reachable; the Uzawa factor per augmentation is about the body's local stiffness over
   the penalty (`E/h` against `100 E/h`): about `1e-2` per augmentation from the penalty penetration `p_0/(100 E/h)`, near `2e-5`
   (`p_0 = 0.07`, `h = 0.025`), so the tolerance `1e-8` after about 3 to 5 augmentations, plus the active-set changes at the
   edge of the contact. The values: as ADR 0540 HS (Hex8, `h = a/6`: radius `5.9e-3`, peak `3.9e-3`, approach `1.1e-2`, including
   the tilt of the sphere's normal, ADR 0540 arithmetic 3); the nodal pressure `t_a/A_a` is a `Phi_a`-weighted average over the
   node's faces (support about `2 h`), which smooths the fitted `p^2` near the edge by about `(h/a)^2 = 6e-2` of its curvature
   term at most. Estimate: radius, peak and approach within about 3 to 5 percent on both meshes. Run time: ADR 0540's two failing
   solves (20 augmentations each) took 190 s together; with 3 to 8 augmentations each solve is estimated under 2 minutes.
5. **Hertz, total Lagrangian**: the strain at the centre is about `p_0/E* = 0.064`; the finite-strain effect on the force for a
   given approach is estimated at a few percent (the radius changes as its cube root). Estimate: within about 5 to 8 percent
   (uncertain); the registered limits are the acceptance's. Cost: about 11 000 (HS20) and 14 700 (HS10) unknowns, the
   supernodal factor of a few seconds per Newton iteration, 20 to 60 iterations: several minutes each (heavy, run once).
6. **FR**: `4 sum t_a |n_bar_z|` against `F_fe`: the relative difference is at most `1/|m_a| - 1`; the normals of a node's
   points differ by at most the support width over `R`, about `0.05` rad, so `1 - |m_a|` about `1e-4` (the mean square half-angle
   over two). Limit `1e-3`.
7. **Tangent**: as ADR 0540 arithmetic 6: truncation `1e-12`, rounding `1e-10` against the limit `1e-6`. NT-m: the curvature
   part `t_a d2 gbar_a`, with `t_a` near 0.1, against `eps_a G G^T` with `eps_a` near `10 x 0.04` and `|G|` near 1: ratio about
   0.2 (sphere, curvature `1/d` near 1; the curved Hex20 target, the closest-point terms of order 1): `> 1e-2`.

## Checks (limits fixed now)

Every aggregated error is the maximum of finite values; a non-finite value is a miss of its check. Relative errors are
`|x/x_ref - 1|`. "Bit for bit" compares `f64::to_bits` of every value.

| id | clause | quantity | limit | estimate |
|---|---|---|---|---|
| HS20m | c3 | Hertz on Hex20 (D6): the solve succeeds with the gap tolerance `1e-8`; contact radius, peak (`sqrt(c_0)`), approach against Hertz at `F_fe` | succeeds; `0.15`, `0.20`, `0.10` | arithmetic 4: 3 to 5 percent |
| HS10m | c3 | the same on Tet10 | succeeds; `0.15`, `0.20`, `0.10` | arithmetic 4 |
| PT10m | c4 | Tet10 blocks, mortar: every stress component at every integration point against `(0, 0, -p, 0, 0, 0)` over `p`; every node's pressure against `p`; the sum of the reactions on `lower.zmin` against `p x 1`; 25 contactor nodes | `<= 1e-8` each; 25 | arithmetic 3 |
| PT20m | c4 | Hex20 blocks, mortar: as PT10m; 21 contactor nodes | `<= 1e-8` each; 21 | arithmetic 3 |
| M20w | c5 | the tangent of the Hex20 mortar contactor on the warped Hex8 target | `<= 1e-6`; every node active, the same faces | arithmetic 7 |
| M10w | c5 | Tet10 contactor, warped Hex8 target | `<= 1e-6` | arithmetic 7 |
| M20s | c5 | Hex20 contactor, rigid sphere | `<= 1e-6` | arithmetic 7 |
| M10s | c5 | Tet10 contactor, rigid sphere | `<= 1e-6` | arithmetic 7 |
| M20q | c5 | Hex20 contactor, curved Hex20 target | `<= 1e-6` | arithmetic 7 |
| PT20t | c4, c6 | PT20m with the total Lagrangian body: every node's pressure against `p`; the reaction sum against `p x 1`; 21 nodes; every value finite | `<= 1e-8` each; 21 | D5, arithmetic 3 |
| PT10t | c4, c6 | PT10m with the total Lagrangian body: as PT20t; 25 nodes | `<= 1e-8` each; 25 | D5 |
| HS20t | c3, c6 | HS20m with the total Lagrangian body (heavy, ignored, run once): succeeds; radius, peak, approach | succeeds; `0.15`, `0.20`, `0.10` | arithmetic 5: 5 to 8 percent |
| HS10t | c3, c6 | HS10m with the total Lagrangian body (heavy): as HS20t | succeeds; `0.15`, `0.20`, `0.10` | arithmetic 5 |
| B2 | c7 | D7: the digests of the default (Gauss-point) path on quadratic faces, recorded on the base | equal | D4 |
| B | c7 | the existing records `sa_contact_default_bits.rs`, `sa_contact_project_f604_default_bits.rs`, `sa_contact_f615_default_bits.rs`, `sa_contact_f601_default_bits.rs`, every test of `rcfd-fem`, and `cargo test -p rcfd-api --release --test baseline_hashes` | pass | D4 |
| EQm | (c2) | PT10m and PT20m with `AllFaces` and with `Buckets`: every value bit for bit, finite | bit for bit | ADR 0540 D1 (the per-point search is unchanged) |
| PW | (c2) | the node areas of PT20m and PT10m against the closed form of D6 | `<= 1e-14` relative | arithmetic 2 |
| FR | (c3) | HS20m, HS10m: `4 sum t_a |n_bar_z|` against `F_fe` | `<= 1e-3` | arithmetic 6 |
| R4 | (D4) | the refusals of D6 R4 | refused with the key | D4 |

Recorded without a limit: the augmentations and Newton iterations of HS20m, HS10m, HS20t, HS10t, their run times, the force
`F_fe`, the number of active nodes, and the largest penetration at the load-rule points at the converged mortar state (the
Gauss-point contactor of ADR 0540 evaluated at the mortar solution, multipliers zero): the mortar contactor zeroes weighted
gaps, not point gaps.

Negative controls:

| id | control | expected |
|---|---|---|
| NT-m | M20s and M20q with the matrix of `convex_tangent` (no curvature part) against the differences | `> 1e-2` each (arithmetic 7) |
| NW | the untransformed weights (`alpha = 0`) on a flat Quad8 face: `int N_corner dA` (by `face_rule(8)`, `face_shape_functions(8, .)`) | `< 0` (finding 2; ADR 0540 NG) |

### Planned mutations (on a `git archive` copy of the final tree in `f623-scratch/mut`, target `f623-scratch/target-mut`, each alone by exact text replacement in `contact.rs`, the fast test files `sa_contact_mortar_f623.rs` and `sa_contact_f623_default_bits.rs` run once, the file restored)

| id | mutation | expected to be caught by |
|---|---|---|
| MM1 | `alpha = 0` (the untransformed basis) | PT20m, PT10m (the construction refuses `A_a <= 0`), PW |
| MM2 | the edge factor `1 - 2 alpha` replaced by `1` (not a partition of unity) | PT20m or PT10m, PW |
| MM3 | the target nodes' blocks of a point's gradient dropped in the node's combination | M20w, M20q, PT20m |
| MM4 | the node's second derivative dropped (`None`) | M20s, M20q, M10s |
| MM5 | `c_ag` not divided by `A_a` (the weighted gap in m^3) | PT20m, PT10m (pressure) |
| MM6 | the default `quadratic_contactor` set to `Mortar` | B2 |
| MM7 | the reference Jacobian left out of `W_g` | PW, PT20m or PT10m (pressure) |

## Decision rule

- **done**: every check met on its registered run (the heavy records HS20t and HS10t included), every negative control as
  expected, every planned mutation caught by a check named for it, B2, B and `baseline_hashes` unchanged, fmt and clippy clean.
- **partial**: B2, B and `baseline_hashes` unchanged and PT10m, PT20m and the five tangents met, and any other miss (or a check
  not run) diagnosed and recorded as a `*_known_miss` test, or a mutation not caught.
- **todo**: otherwise, in particular any change of the default bits.

## D7. Files and the bit record B2

`crates/rcfd-fem/src/contact.rs` (D1 to D4); new tests `crates/rcfd-fem/tests/sa_contact_f623_default_bits.rs` (B2),
`sa_contact_mortar_f623.rs` (PT10m, PT20m, EQm, PW, PT20t, PT10t, M20w, M10w, M20s, M10s, M20q, NT-m, NW, R4),
`sa_contact_mortar_hertz_f623.rs` (HS20m, HS10m, FR; HS20t, HS10t ignored as heavy records); the shared helpers of
`tests/f601_common` are used as they are (not edited); docs: the contact module's doc comment, `docs/reference-data.md` (an
F-623 line under the F-601 section, if one fits).

B2, written and run on the base e906faa8 after this commit and before any library code, its digests printed by that run and
written into the file in the same commit, FNV-1a 64 (`f601_common::digest`) over the words of: (G1) ADR 0540 PT10 solved by
`contact::solve` (`result_words`); (G2) PT20 solved; (G3) `ContactSet::evaluate` of ADR 0540 T1g20's and T1g10's models at
the 20 states `f601_common::state(n, 0.01, seed)`, seeds 1 to 20 (`node_words`), and the triplets of `Contribution::add` at
the first state; (G4) ADR 0540 Q20 (the punch on the Hex20 plate) solved. After the change these digests must be unchanged.

## Limits (not covered)

- Element-based integration of the mortar integrals (the contactor face's load rule, no segmentation): against a non-matching
  deformable target the integrals across target-face edges are not exact; checked only on matching patches, the rigid sphere
  and single-face targets. A node with any of its points outside every target is inactive (D2): along the edge of a target the
  contactor may penetrate there.
- The mortar contactor is opt-in; the Gauss-point contactor stays the default for Hex20 and Tet10 faces (defaults
  bit-identical). Hex8 and Tet4 contactors are not changed (node contactors).
- Frictionless, no self-contact, no dynamics (as ADR 0507, 0540).
- The weights are fixed in the reference configuration (a nominal pressure); under large changes of the contact area the
  pressure is per reference area.
- No initial-gap offsets on a mortar pair; no case-file key, no CLI or API surface.

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. **Opt-in or default**: taken opt-in (`QuadraticContactor::Mortar`), because the house rule keeps defaults bit-identical; a
   later item may switch the default after the maintainer's approval (it changes the default results of Hex20 and Tet10
   contactors).
2. **The weighting functions**: options (a) the untransformed `N_a` (cannot carry a uniform pressure, finding 2), (b) dual
   functions (biorthogonal to `N`; would condense the multipliers, but the driver uses a penalty with Uzawa updates, not
   multipliers as unknowns, so nothing is gained), (c) the transformed basis of D1 with `alpha = 1/4`. Taken (c).
3. **Integration**: options (a) segmentation of the contactor face against the target faces (exact for non-matching meshes,
   much more code), (b) element-based integration by the load rule. Taken (b), recorded under the limits.

Open (not taken): the case-file key (as ADR 0540); switching the default contactor of quadratic faces.
