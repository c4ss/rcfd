# ADR 0540: F-601 registration: a bucket search, rigid cylinder and sphere targets, and Hex20 and Tet faces in the contact module

Status: **REGISTERED 2026-11-15, written before any code of F-601 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-601 was written or run in the repository. The arithmetic below was done by hand and
with a few lines of numpy in the scratch folder (`w49i-work/f601-scratch/arith601.py`; no rcfd code). The bit record B of the
default paths is a test file written and run on the base after this commit and before any library code (D8). Results go in the
addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-601 (deps F-427), the follow-up proposed by [ADR 0507](0507-f427-registration-frictionless-contact-augmented-lagrangian.md)
(its "Limits (not covered)": the all-faces search, the rigid plane as the only rigid target, faces of Hex8 only). Number 0540
given by the lead. Branch `f601-work` from 4d0d0f1b (origin/main, with F-604's projection fix of ADR 0517 amendment 1 and
F-615's `ContactOptions::factor`), worktree `w49i-work/f601`, scratch `w49i-work/f601-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no
unsafe code, the crate graph unchanged; clean-room. The brief overrides rule 9 for this item: the F-601 entries of
`docs/ROADMAP.md` and `docs/roadmap.json` are updated on the branch in a separate last commit (the svgs are not regenerated).
Other agents edit F-574 (the Shell3 model check), F-579 (radiation impedance) and F-617 (the thermal predictor in
`nonlinear.rs` and `nl_transient.rs`). This item edits none of their files: it changes `contact.rs` only, adds test files, one
`#[ignore]` attribute on an existing test (D9), and documentation.

## What F-601 asks

The acceptance, verbatim, binding:

> Every limit not stated here is registered in the ADR before the run. Contact search and targets: a bucket search in place of the all-faces loop, rigid cylinder and sphere targets, and Hex20 and Tet faces (the limits of ADR 0507).

Clauses: **c1** every limit registered here before the run; **c2** a bucket search in place of the all-faces loop; **c3** rigid
cylinder and sphere targets; **c4** Hex20 and Tet faces. The brief adds: (b1) the bucket search gives exactly the candidate
faces, projections and results of the all-faces loop on the existing contact models, and its speed on ADR 0517's hub at 32
cells is recorded as a ratio; (b2) the rigid cylinder against Hertz's line contact (half-width and peak pressure); (b3) the
rigid sphere against Hertz's point contact (contact radius and peak pressure), the formulas cited from Johnson's *Contact
Mechanics* from memory and recorded in `docs/reference-data.md`; (b4) Hex20, Tet10 and Tet4 faces on a patch test (a uniform
pressure transmitted) and one ADR 0507 benchmark repeated on those elements; (b5) the bucket search opt-in or the default with
an exact-equality proof by tests, registered; (b6) defaults bit-identical otherwise, `baseline_hashes` unchanged, no case-file
key; (b7) mutations registered; (b8) every aggregated test error finite.

## What was read (no run)

ADR 0507 (D1 the gap and its derivatives, D2 the augmented Lagrangian, D3 the driver, D4 the areas, D7 the models, the
amendments 2 and 3, the limits, the addendum's numbers: B1 `2.1e-15`, H1/H2/H3 `3.0e-4`, `7.6e-4`, `6.1e-4`, HF `5.6e-3`,
`4.9e-4`, `1.2e-3`, T1 `2.3e-10`, T1r `5.2e-11`), ADR 0513 (the fit model and its records), ADR 0517 (D3 the hub, amendment 1:
the projection's `1e-10` floor, PJ1), ADR 0519 (the punch models, its mutations of the tangent), ADR 0533 (the factor option,
its bit records; P1: the contact set's evaluation is about 0.01 s per Newton iteration at `n = 32`). Code:
`crates/rcfd-fem/src/contact.rs` (whole), `mesh.rs` (`HEX_FACES`, `TET4_FACES`, `TET10_FACES`, `face_nodes`), `hex20.rs`
(`faces`, `face_shape_functions`, `face_rule`, `upgrade_to_hex20`), `loads.rs` (`face_shape_functions`, `face_rule` for 3, 4, 6
and 8 nodes, `add_face_load`), `builders.rs` (`hex_box`, `tet_box`, `upgrade_to_tet10`), `convert.rs` (`split_hexahedra`),
`shell.rs` and `beam.rs` (`faces()` is empty: no surface set holds a shell or beam face), the tests `sa_contact.rs`,
`sa_contact_order.rs`, `sa_contact_hub_f604.rs`, `sa_contact_project_f604_default_bits.rs`,
`sa_contact_f615_default_bits.rs`, `sa_contact_supernodal_f615.rs`, `sa_contact_fit_f599.rs`. Nothing outside the repository.

Findings that shape the design:

1. The present search (`ContactSet::kin`, deformable target) runs, for every contactor node at every evaluation, over every
   target face: the face's current corners, its reach (the search distance or the longest current edge), the bounding box
   grown by the reach, then the projection of the faces that pass the box, the gap test `|g| <= reach`, and the face of the
   smallest `|g|` with ties to the lower face index (strict `<` over ascending faces).
2. The only consumers of the contact module are the tests of `rcfd-fem`; no other crate calls it, so `baseline_hashes` cannot
   see it (it is run anyway, b6).
3. The consistent nodal share of a uniform pressure on a flat Hex20 face is `-1/12` of the face's area at each corner and
   `1/3` at each mid-edge node (arithmetic 2); on a flat Tet10 face it is `0` at the corners and `1/3` at the mid-edge nodes. A
   node-to-surface contact whose node force is `max(0, ...)` cannot carry the negative corner forces, so a Hex20 contactor
   with node contactors cannot transmit a uniform pressure: a known property of node-to-surface contact with serendipity
   faces. This decides the contactor of quadratic faces (D3).

## Design

### D1. The bucket search (c2, b1, b5)

`ContactOptions` gains `pub search_method: SearchMethod` with `enum SearchMethod { Buckets, AllFaces }` (`#[non_exhaustive]`
struct, built only by `ContactOptions::new`, so the field is not a breaking change) and the builder `search_method(..)`.
`ContactOptions::new` sets **`Buckets`, the default**. `AllFaces` keeps the present loop verbatim, as the reference.

At every evaluation of the contact set, for each pair with a deformable target, `Buckets` first computes, for every target
face in its order, the current node positions, the reach and the grown box `lo_i = min_a x_a,i - reach`, `hi_i = max_a x_a,i +
reach` by **the same expressions in the same order** as the present loop (one shared function), then a uniform grid:

- the origin `o_i = min_f lo_f,i`, the top `t_i = max_f hi_f,i`, the cell size `s_i = max_f (hi_f,i - lo_f,i)` (so a face spans
  at most two cells per axis), the cell counts `n_i = min(floor((t_i - o_i)/s_i) + 1, ...)`, enlarged cells when
  `n_x n_y n_z` would exceed `8 F + 64` (`F` faces);
- the cell coordinate `c_i(x) = min(floor((x - o_i)/s_i), n_i - 1)` (and 0 when `s_i` is 0);
- each face is listed in every cell of the range `c(lo_f) .. c(hi_f)`, faces taken in ascending order, so every cell's list is
  ascending (a compressed row list).

A contactor point at `x_s` takes, in place of all faces, the list of its cell `c(x_s)` (none when `x_s` lies outside `[o, t]` in
any axis or is not finite), and runs on each listed face, in the list's (ascending) order, the present per-face tests with the
values computed once: the node filter, the box test, the projection, the gap test and the strict `<` of the smallest `|g|`.

**Exact equality (the proof).** (i) The values `x_a`, reach, `lo`, `hi` and every per-face test are computed by the same
expressions on the same inputs as the present loop, so they have the same bits. (ii) Every face that passes the present box
test, `lo_f <= x_s <= hi_f` in each axis, is in the list of `c(x_s)`: the map `x -> floor((x - o)/s)` is monotone in `x` (a
rounded subtraction of a constant, a rounded division by a positive constant, `floor` and the clamp are each monotone), so
`c(lo_f) <= c(x_s) <= c(hi_f)`; and such an `x_s` lies inside `[o, t]`. (iii) Faces not listed fail the box test, so they would
have been skipped by the loop too. (iv) The list is ascending, so the faces that pass are visited in the loop's order and the
strict `<` keeps the same face on ties. Hence the chosen face, `xi*`, the gap, the normal and the derivatives have the same bits,
and so does every result. When any face position or reach is not finite the evaluation falls back to the present loop for that
pair (the same bits by definition). The proof is checked by tests (EQ1, EQ2, B, the existing records) and attacked by mutations
(MB1 to MB4).

Being bit-identical by construction, `Buckets` is the default, the clause's "in place of" (b5). Taken on the maintainer's
standing instruction of 2026-11-11 (take the recommendation): the alternative, opt-in, gains nothing when the bits are equal
and leaves the quadratic loop on every user's path; if any registered equality fails the default bits change and the decision
rule gives **todo**.

### D2. Rigid sphere and rigid cylinder targets (c3)

`Target` gains `RigidSphere { centre: Vec3, radius: f64 }` and `RigidCylinder { point: Vec3, axis: Vec3, radius: f64 }` (the
cylinder infinite, through `point` along `axis`, normalised by the pair). The contactor lies outside the target. For a contactor
point at `x` (first principles):

- sphere: `r = x - c`, `d = |r|`, `n = r/d`, gap `g = d - R`, gradient `dg/dx = n`, second derivative `d2g/dx2 = (I - n n^T)/d`;
- cylinder: `r = (x - p) - ((x - p) . e) e`, `d = |r|`, `n = r/d`, `g = d - R`, `dg/dx = n`, `d2g/dx2 = (I - e e^T - n n^T)/d`.

The normal is the target's outward normal at the closest point (toward the contactor), as for the plane; the force enters as
`-t G` and the tangent `eps G G^T - t d2g` exactly as D2 of ADR 0507. A point with `d = 0` or not finite has no kinematics
(inactive). Refused (`InvalidValue`, key `"rigid sphere"` / `"rigid cylinder"`): a radius not positive and finite, a centre or
point not finite, an axis of zero or not finite length. The initial-gap offsets add to these gaps as to the plane's.

### D3. Hex20, Tet4 and Tet10 faces (c4)

A surface set may now hold faces of Hex8 (4-node bilinear, "Quad4"), Hex20 (8-node serendipity, "Quad8"), Tet4 (3-node,
"Tri3") and Tet10 (6-node, "Tri6"), in the node order of `mesh.rs` (corners counter-clockwise seen from outside, then the
mid-edge nodes), with the shape functions of `loads::face_shape_functions` (`Tri`: `N = (1 - u - v, u, v)` on the unit
triangle; `Quad8` of `hex20::face_shape_functions`). Any other face is refused as now (none exists: shells and beams have no
faces).

**As a target.** Quad4 faces keep the present `project` (bits unchanged). The other shapes have `project_general`, the same
closest-point Newton and derivatives written for any shape: Newton on `(x_s - x(xi)) . a_alpha = 0` with the matrix `m_ab -
(x_s - x) . x_{,ab}` (now with `x_{,11}`, `x_{,22}`, nonzero on quadratic faces), from `xi = 0` (Quad8) or the centroid `(1/3,
1/3)` (triangles), the same 30 iterations, the `1e-14` step and the `1e-10` floor of ADR 0517 amendment 1, then the domain test:
`|xi|, |eta| <= 1 + 1e-2` (Quad8, as now) or `u, v >= -1e-2`, `u + v <= 1 + 1e-2` (triangles: the one percent of ADR 0507
amendment 2 on the unit triangle). The gap from differences of ADR 0507 amendment 3, `g = n . sum_b N_b ((X_s - X_b) + (u_s -
u_b))` (valid because `sum_b N_b = 1`). The derivatives: the derivation of ADR 0507 D1 holds for a general face once `h_ab = n .
x_{,ab}` has all three entries (re-derived: `dxi = A^-1 (T + g P)` from the variation of the stationarity with `da_b = sum
N_{a,b} dx_a + x_{,bc} dxi_c`; `dn = -m^{ab} (P_b + h_bc dxi_c) a_a`; the term `-T^T m^-1 P - g T^T C P` combines to `-T^T A^-1
P` because `m^-1 (A + g h) A^-1 = A^-1`), so

    d2g/dq2 = - T^T C T - T^T A^-1 P - P^T A^-1 T - g P^T A^-1 P,   C = m^-1 h A^-1,   A = m - g h,

over `q = (x_s, x_1 .. x_k)` (`k` the face's nodes), with `T_alpha` blocks `a_alpha, -N_b a_alpha` and `P_alpha` blocks `0, N_{b,alpha}
n`. The second derivatives of the shape functions (first principles): Tri3 zero; Tri6 corner `N_i = l_i (2 l_i - 1)`:
`N_{i,ab} = 4 l_{i,a} l_{i,b}`, mid-edge `N = 4 l_i l_j`: `4 (l_{i,a} l_{j,b} + l_{j,a} l_{i,b})`; Quad8 corner `N = (1 + u u_a)(1 + v
v_a)(u u_a + v v_a - 1)/4`: `N_uu = (1 + v v_a)/2`, `N_vv = (1 + u u_a)/2`, `N_uv = u_a v_a (2 (u u_a + v v_a) + 1)/4`; mid-edge
`(1 - u^2)(1 + v v_a)/2`: `N_uu = -(1 + v v_a)`, `N_vv = 0`, `N_uv = -u v_a`; mid-edge `(1 + u u_a)(1 - v^2)/2`: `N_uu = 0`, `N_vv =
-(1 + u u_a)`, `N_uv = -v u_a`. The reach (default): the longest current edge between consecutive corners. The box: over all the
face's nodes.

**As a contactor.** Finding 3 decides:

- Hex8 faces: the corner nodes with the consistent area of D4 (unchanged).
- Tet4 faces: the corner nodes with the consistent share `A/3` of the face's reference area (exact: `int N_a dA = A/3`).
- Hex20 and Tet10 faces: **Gauss points** (Gauss-point-to-surface; first principles, the idea is in the literature on
  quadratic contact elements, e.g. Wriggers 2006, chapter 9, from memory, not read): the contactor points are the points of
  `loads::face_rule` of the face (9 per face, the same rule as the pressure loads: the 3 x 3 Gauss rule for Quad8, the collapsed
  3 x 3 rule for Tri6), at `x_g = sum_a M_a(zeta_g) x_a` over the contactor face's nodes. A point's "area" is its weight `w_g
  |x_{,u} x x_{,v}|` on the reference face (positive), its penalty `eps w_g`, its gap that of `x_g` against the target, its
  gradient over the contactor face's nodes `M_a(zeta_g) G_s` and over the target's as before, its second derivative `J^T H J`
  with `J` the map `x_g = sum M_a x_a` (blocks `M_a M_c H_ss`, `M_a H_sb`, `H_bb`). The gap from differences:
  `g = n . sum_b N_b sum_a M_a ((X_a - X_b) + (u_a - u_b))` (faces) and the corresponding forms for the rigid targets. A target
  face that shares a node with the contactor face is skipped (the node filter of the present search).
- Why the patch test passes (b4): with matching meshes under a uniform pressure every contactor point has the gap 0 and the
  force `p w_g`, positive; the contactor nodes receive `sum_g M_a(zeta_g) p w_g`, the pressure load of `loads` exactly (the same
  rule); a matching target face evaluates its own shape functions at the same physical points, so it receives the same
  consistent load. The uniform stress is then the solution of every Newton solve of the augmentation (the penalty solve gives a
  uniform penetration `p/eps`, the next the zero gap), as for ADR 0507 B1.
- A contactor set holds node-type faces (Hex8, Tet4) or point-type faces (Hex20, Tet10), not both (refused, `InvalidValue`,
  key `"contact surface"`). Initial-gap offsets name nodes, so a point-type pair with offsets is refused (key `"initial
  gap"`).

### D4. The result

`ContactNode` gains `pub point: Option<(FaceRef, [f64; 2])>`: for a Gauss point the contactor face and the point's local
coordinates on it; `None` for a node. Its `node` is then the point's id `n_nodes + k` (`n_nodes` the mesh's node count, `k` the
point's index over the model's point-type pairs in pair order, faces in set order, points in rule order), so ids never collide
with nodes and `nodes(pair)`, the multipliers, the active sets and the error messages keep one integer per contactor point.
`area` is `w_g`, `pressure` is `force/w_g`. Nothing else changes for node contactors.

### D5. The models of the checks

Units SI unless stated; `NewtonOptions::new(1).max_iterations(30)`; penalty `100 E/h` per area with `h` the contactor body's
element size normal to the face, unless stated.

- **Existing models (EQ1).** As in their test files, copied: ADR 0507 B1 (blocks pressed, 2 steps), S1 (pressed then pulled
  apart, the `_a3` set-up, 8 steps), I (the fit, the `_a3` set-up), T2 (the total Lagrangian punch, 2 steps), ADR 0519's M3
  (the punch with an initial gap `-3e-4` on part of it, total Lagrangian), ADR 0517's hub at `n = 4` (BASE with `n = 4`), and
  the F-601 models with deformable targets (PT4, PT10, PT20, Q20 below).
- **States (EQ2).** `ContactSet::evaluate` at 20 states `u_i = A_k s_i` with `s_i` uniform in `[-1, 1]` from a fixed linear
  congruential generator (seed per state), on: the hub at `n = 8` (`A = 0.3 h`, `h = (b - a)/8`), the punch of T2 (`A = 0.05`),
  and ADR 0507 T1's warped pair (`A = 0.05`), each with the default reach and with `search_distance(0.1 h)` (`0.02` for the punch
  and T1).
- **Speed (BT).** ADR 0517's hub at `n = 32` (BASE, `n = 32`; the model only, no solve), its pair, `ContactSet::new` with each
  method; `evaluate` at `u = 0` and at the state `u_i = 0.3 h s_i` (EQ2's generator, seed 1), alternately AllFaces then Buckets,
  5 times each state: `t_a`, `t_b` the summed wall times (`Instant`), `t_b/t_a` the ratio. The results of each pair of calls
  are compared bit for bit.
- **Tangent (T1x).** ADR 0507 T1's arrangement (`eps = 10`, `u_i = 0.01 sin(1.7 i + 0.3)`, central differences with `1e-6` of
  the contact force on every equation, `max |K_c - K_fd| / max |K_c|`, every point active with the same face at every
  perturbed state):
  - **T1s** the Hex8 contactor of T1 (bottom face `T1_CONTACTOR`) against `RigidSphere { centre (0.5, 0.45, 0), radius 0.97
    }`, multipliers `(0.5, 0.4, 0.6, 0.3)`;
  - **T1c** the same against `RigidCylinder { point (0.5, 0.5, 0), axis (1, 0.2, 0), radius 0.97 }`;
  - **T1q** the same contactor against a Hex20 target: T1's lower hexahedron as a Hex20, its mid-edge nodes at the edge
    midpoints except the four of its top face, raised by `(0.03, -0.02, 0.025, -0.015)` in `z` (a curved face);
  - **T1t3** the same contactor against one Tet4 below it, nodes `(-0.2, -0.2, 1.0)`, `(-0.2, 1.8, 1.02)`, `(1.8, -0.2, 0.97)`,
    `(0.5, 0.5, 0)`, target its face 0 (nodes 0, 2, 1, outward normal up);
  - **T1t6** that tetrahedron as a Tet10, the mid-edge nodes of face 0 raised by `(0.02, -0.015, 0.01)` on the edges `(0, 1),
    (1, 2), (0, 2)`;
  - **T1g20** a Hex20 contactor: T1's upper hexahedron as a Hex20, the bottom face's mid-edge nodes lowered by `(0.01, -0.01,
    0.005, -0.005)`, its 9 points against T1's warped Hex8 target, multipliers `0.1 + 0.01 k`, `k = 0 .. 8`;
  - **T1g10** a Tet10 contactor: nodes `(0.2, 0.2, 0.99)`, `(0.8, 0.25, 0.98)`, `(0.3, 0.8, 1.0)`, `(0.4, 0.4, 1.8)` and mid-edge
    nodes at the midpoints, those of face 0 moved by `(0.01, -0.01, 0.005)` in `z`; its 9 points against T1's warped target,
    multipliers `0.1 + 0.01 k`.
- **Line contact (CL, b2).** Plane strain on a slab one element thick: `E = 1`, `nu = 0.3`, `R = 1`, `b = 0.1`; the half model
  `x in [0, W]`, `y in [0, t]`, `z in [-D, 0]`, `W = D = 10 b`, `t = h = b/16`. In `x` and in `z` the coordinates `0, h, .., 20 h`
  (to `1.25 b`) then 12 cells growing by `q = 1.352583` (bisection, arithmetic 4) to `10 b`: 32 x 1 x 32 Hex8. Constraints:
  `u_y = 0` at every node (plane strain), `u_x = 0` on `x = 0` (symmetry); `u_z` free (the body is held by the contact). Loads:
  Flamant's field of the line load `P_th = pi E* b^2/(4 R)` (arithmetic 4) on the faces `x = W` and `z = -D`, as consistent
  nodal forces of the traction `sigma n` (2 x 2 Gauss on each face; the test's own integration). Target `RigidCylinder { point
  (0, 0, R - delta_0), axis (0, 1, 0), radius R }`, `delta_0 = b^2/(4 R)` (the nodes with `x < 0.71 b` start penetrating);
  contactor the face `z = 0`; `eps = 100 E/h`; 1 step; tolerance `1e-8`. Linear. The measured load `P_fe = (2/t) sum_s t_s
  |n_z,s|`; the half-width `b_fe` from the least-squares fit of `p_s^2 = c_0 + c_1 x_s^2` over the contactor nodes with `p_s >=
  0.5 max p` (Hertz: `p^2 = p_0^2 (1 - x^2/b^2)`), `b_fe = sqrt(-c_0/c_1)`; the peak `p_c` the pressure of the node at `x = 0`,
  `y = 0`.
- **Point contact (HS, b3).** ADR 0507 D7 H's model (the quarter block, its graded mesh, Boussinesq's displacement of `F_th`
  plus `delta = a^2/R` on the far faces) with the rigid plane and the offsets replaced by `RigidSphere { centre (0, 0, R),
  radius R }`, no offsets. `F_fe = 4 sum_s t_s |n_z,s|`; `a_fe` the fit of ADR 0507 D7 H; the peak the centre node's pressure.
  **HS** fast on the H-fast mesh (`h = a/6`, 14^3 Hex8); **HSH** heavy on the H mesh (`h = a/12`, 24^3 Hex8, about 4 minutes,
  run once, alone). **SX** the H-fast mesh solved both ways (ADR 0507's plane with offsets, and the sphere).
- **Patch tests (PT, b4).** ADR 0507 B1's two blocks `[0,1]^2 x [0,1]` and `[0,1]^2 x [1,2]` of `[2, 2, 2]` cells, matching, by
  `assemble_parts`; `E = 2e11`, `nu = 0.3`; `u_z = 0` on `lower.zmin`, `u_x = 0` on both `xmin`, `u_y = 0` on both `ymin`;
  `p = 1e8` on `upper.zmax` (`Load::pressure`); contactor `upper.zmin`, target `lower.zmax`; `eps = 4e13`; 2 steps; tolerance
  `1e-13`. **PT4**: both blocks `tet_box` (Tet4, node contactors). **PT10**: both `tet_box` upgraded by `upgrade_to_tet10`
  (Gauss points). **PT20**: both `hex_box` upgraded by `upgrade_to_hex20` (Gauss points).
- **Repeated benchmark (HS20, HS10, b4).** ADR 0507's Hertz problem (a3) with the rigid sphere of HS, on quadratic meshes:
  **HS20** the graded quarter block with `h = a/4` (5 uniform cells to `1.25 a`, 4 graded, `q = 1.606703`, to `5 a`), 9^3 Hex8
  upgraded to Hex20 (straight edges); **HS10** with `h = a/3` (4 uniform cells, 4 graded, `q = 1.449034`), 8^3 Hex8 split into
  Tet4 by `split_hexahedra` and upgraded by `upgrade_to_tet10`. The Boussinesq values and the symmetry constraints at every node
  (mid-edge nodes included) on the far and the symmetry faces; `eps = 100 E/h`; tolerance `1e-8`. `F_fe = 4 sum t_g |n_z|`; the
  fit as HS over the points with `p_g >= 0.5 max p`; the peak `sqrt(c_0)` of the fit (no point lies on the axis).
- **Q20 (EQ1 only).** ADR 0507 T2's punch (Hex8) on its plate upgraded to Hex20, linear body, 2 steps (a non-matching Hex20
  target).

## Arithmetic done before the code (disclosed)

1. **Hub search cost.** ADR 0517 D3 at `n = 32`: 25 bore rows of 16 nodes (400 contactor nodes); the shaft's outer faces 16
   segments times about 50 axial cells (about 800 faces). The present loop: 320 000 face visits per evaluation, each computing
   four positions, four edges and a box (ADR 0533 P1 measured the whole evaluation at about 0.01 s). The buckets: about 800
   faces in at most 8 cells each, a query of one cell (a few faces): the per-node work falls by about two orders, the grid
   build is `O(F)`. Estimate of `t_b/t_a`: 0.05 to 0.2.
2. **Shares of a uniform pressure** (`arith601.py`): the flat Hex20 face, 3 x 3 Gauss of `N_a`: corners `-0.0833 = -1/12`,
   mid-edge `0.3333 = 1/3` of the area; Tet10 face: corners `0`, mid-edge `1/3` (exact).
3. **Point contact** (as ADR 0507 arithmetic 3): `E* = 1.098901`, `a = 0.1`, `F_th = 1.465201e-3`, `p_0 = 0.0699577`, `delta =
   0.01`. The force of a rigid sphere is along the sphere's normal, tilted by `r/R` (up to 0.1 at `r = a`): a radial surface
   traction `q = p r/R` that Hertz's theory leaves out. Its effect on the normal displacement (the half-space coupling
   `(1 - 2 nu)/(2 (1 - nu))`, Johnson chapter 3, from memory) changes the effective curvature by about `0.286 a/R`, i.e. near
   3 percent at most: `a` by about 1 percent, `p_0` by about 1 percent. With the discretisation (HF: up to `5.6e-3`): HS within
   about 2 percent, HSH within about 1.5 percent. SX: the sphere against the offsets on the same mesh, about 1 to 2 percent.
4. **Line contact** (`arith601.py`; Johnson chapter 4, from memory, re-derived here). The surface slope of the half-space under a
   pressure `p(s)` in plane strain is `du_z/dx = -(2/(pi E*)) PV int p(s)/(x - s) ds`; for `p = p_0 sqrt(1 - s^2/b^2)` the
   integral is `pi p_0 x/b` (`|x| < b`), so the slope is `-2 p_0 x/(E* b)`; the rigid cylinder needs `-x/R`: `p_0 = E* b/(2 R)`,
   and `P = pi b p_0/2 = pi E* b^2/(4 R)`, i.e. `b = sqrt(4 P R/(pi E*))`, `p_0 = 2 P/(pi b) = sqrt(P E*/(pi R))`. Here
   `P_th = 8.630749e-3`, `p_0 = 0.0549451`. Flamant's field of a line load `P` into `z <= 0` (Johnson chapter 2, from memory;
   check CL0 verifies it): `sigma_ij = 2 P z x_i x_j/(pi r^4)` over `(x, z)` (the radial stress `-2 P cos(theta)/(pi r)`, the
   others zero); `sigma_zz` integrated over `z = -D` gives `-P`; the half model's far faces carry `P/2` per unit thickness. The
   tilt (as 3, in 2D the ratio of the slopes `(1 - 2 nu) b/(2 (1 - nu) R) = 0.0286`): `b` and `p_0` by about 1.5 percent; the
   point-load far field at `10 b`: `O((b/r)^2) = 1e-2` of a small boundary term; mesh `h = b/16`: below 1 percent (H1 at `a/12`
   was `3e-4`). Estimate: within 2 percent. The grading factor `q` with `h q (q^12 - 1)/(q - 1) = 8.75 b` is `1.352583`.
5. **Patch tests.** As ADR 0507 arithmetic 2 and D3: the exact discrete solution is the uniform stress, with the residual
   penetration of the tolerance `1e-13` giving `E 1e-13/(2 x 0.5) = 2e-2 Pa`, `2e-10` of `p`. Rounding: B1 measured `2.1e-15`.
6. **Tangent.** As ADR 0507 arithmetic 5: truncation `1e-12`, rounding `1e-10` against the limit `1e-6`. The curvature part of
   the sphere, `t (I - n n^T)/d` with `t` near 0.5 and `d` near 1, against `eps A n n^T` with `eps A` near 3 (`eps = 10`, `A`
   near 0.3): dropping it (NT-s) gives an error of order 0.1.
7. **Repeated benchmark.** HS20's mesh has 10^3 corner nodes, 3700 nodes in all (about 11 000 unknowns); HS10's 4913 nodes
   (about 14 700 unknowns); each a few factorisations of seconds. A Hex20 at `h = a/4` interpolates the surface as finely as a
   Hex8 at `a/8`; a Tet10 at `a/3` (six tetrahedra per cell) as a Hex8 at about `a/6`: both expected near HF's numbers (a few
   percent with the tilt).

## Checks (limits fixed now)

Every aggregated error is the maximum of finite values; a non-finite value is a miss of its check (b8). Relative errors are
`|x/x_ref - 1|`. "Bit for bit" compares `f64::to_bits` of every value (and the equality of every integer and face).

| id | clause | quantity | limit | estimate |
|---|---|---|---|---|
| EQ1 | c2, b1, b5 | each model of D5 EQ1 solved by `contact::solve` with `AllFaces` and with `Buckets`: displacement, reactions, every contactor point's force, pressure, gap, face, local coordinates, normal and status, every Newton residual, energy, penetration, convex count and active set; every value finite | bit for bit; finite | D1 proof |
| EQ2 | c2, b1 | D5 EQ2: every `ContactNode` of every state, `AllFaces` against `Buckets`, both reaches; and at least 10 changes of a point's face across the states of the hub (the search is exercised) | bit for bit; `>= 10` | D1; the slides of `0.3 h` |
| BT | b1 | D5 BT: `t_b/t_a` on the hub at `n = 32`; the results bit for bit | `< 0.5`; equal | arithmetic 1: 0.05 to 0.2 |
| T1s | c3 | the sphere tangent against central differences | `<= 1e-6` | arithmetic 6 |
| T1c | c3 | the cylinder tangent | `<= 1e-6` | arithmetic 6 |
| CL0 | (c3) | Flamant's field: the equilibrium `div sigma` by central differences (step `1e-4 r`) at 6 points over `P/r^2`; the traction `sigma_zz, sigma_xz` at `z = 0`, `x != 0`, over `P/|x|`; the integral of `sigma_zz` on `z = -0.3` (trapezoid in `ln |x|`, `|x|` from `1e-6` to `300`, 4000 intervals, both sides) against `-P` | `<= 1e-5`, `<= 1e-12`, `<= 1e-4` | FD truncation `1e-8`; the surface traction is zero in closed form |
| CL1 | c3, b2 | `b_fe` against `sqrt(4 P_fe R/(pi E*))` | `<= 5e-2` | arithmetic 4: within 2 percent |
| CL2 | c3, b2 | `p_c` against `2 P_fe/(pi b_H(P_fe))` | `<= 5e-2` | arithmetic 4 |
| CL3 | (c3) | `P_fe` against the applied resultant (the sum of the `z` nodal forces times `2/t`) | `<= 1e-6` | equilibrium at the Newton tolerance |
| CL4 | (c3) | contactor nodes on `y = 0` with a positive force | `>= 10` | `b/h = 16` |
| CL5 | (c3) | the penetration after augmentation | `<= 1e-8` | tolerance |
| HS | c3, b3 | H-fast mesh, sphere: radius, peak, approach against `a_H(F_fe)`, `3 F_fe/(2 pi a_H^2)`, `a_H^2/R` | `0.15`, `0.20`, `0.10` | arithmetic 3: about 2 percent |
| HSH | c3, b3 | H mesh, sphere (heavy): radius, peak, approach; nodes on `y = 0` in contact; penetration | `5e-2`, `5e-2`, `3e-2`; `>= 10`; `<= 1e-8` | arithmetic 3: about 1.5 percent; 12 |
| SX | (c3) | H-fast mesh: `F_fe`, `a_fe` and the peak of the sphere against those of ADR 0507's plane with offsets | `<= 3e-2` each | arithmetic 3 |
| PT4 | c4, b4 | Tet4 blocks: every stress component at every integration point of both blocks against `(0, 0, -p, 0, 0, 0)` over `p`; every contactor point's pressure against `p`; the sum of the reactions on `lower.zmin` against `p x 1`; 9 contactor nodes | `<= 1e-8` each; 9 | arithmetic 5 |
| PT10 | c4, b4 | Tet10 blocks: as PT4; 72 contactor points (8 triangles x 9) | `<= 1e-8` each; 72 | D3, arithmetic 5 |
| PT20 | c4, b4 | Hex20 blocks: as PT4; 36 contactor points (4 faces x 9) | `<= 1e-8` each; 36 | D3, arithmetic 5 |
| T1q | c4 | the tangent on the curved Hex20 target face | `<= 1e-6` | arithmetic 6 |
| T1t3 | c4 | on the Tet4 target face | `<= 1e-6` | arithmetic 6 |
| T1t6 | c4 | on the curved Tet10 target face | `<= 1e-6` | arithmetic 6 |
| T1g20 | c4 | the Hex20 contactor's points on the warped Hex8 target | `<= 1e-6` | arithmetic 6 |
| T1g10 | c4 | the Tet10 contactor's points on the warped Hex8 target | `<= 1e-6` | arithmetic 6 |
| HS20 | c4, b4 | the Hertz problem on Hex20 with the sphere: radius, peak (`sqrt(c_0)`), approach | `0.15`, `0.20`, `0.10` | arithmetic 7 |
| HS10 | c4, b4 | the same on Tet10 | `0.15`, `0.20`, `0.10` | arithmetic 7 |
| R3 | (D2, D3) | refused (`InvalidValue`): a sphere radius `0`, `-1`, NaN; a sphere centre with NaN; a cylinder axis `(0, 0, 0)`; a cylinder radius `0`; a contactor set of one Hex8 face and one Hex20 face (key `"contact surface"`); an initial gap on a Hex20 contactor (key `"initial gap"`) | refused | D2, D3 |
| R1c | (D9) | the copy of ADR 0507's `r1_r2_refusals_a2` with its Tet4 row replaced by: `ContactSet::new` of the Tet4 contactor `xmax` against the plane succeeds, with 4 contactor nodes | holds; the other rows as registered there | D9 |
| B | b6 | D8 | equal; pass | D1 |

Negative controls (each must show its check is not vacuous):

| id | control | expected |
|---|---|---|
| NT-s | T1s without the curvature part (`eps A n n^T` alone against the differences) | `> 1e-2` (arithmetic 6) |
| NT-q | T1q without the curvature part (`eps G G^T` alone) | `> 1e-2` |
| NG | the share of a uniform pressure at a corner of a flat Hex20 face (`loads::face_rule(8)`, `face_shape_functions(8, .)`) | `< 0` (finding 3: node contactors cannot transmit it) |
| NS | EQ2 on the hub: the mean number of faces visited per point by `Buckets` against the target's face count | below 0.25 of it (the buckets filter; recorded through the timing BT and a test-only count, see D7) |

### Planned mutations (on a `git archive` copy of the final tree in `f601-scratch/mut`, target `target-f601-mut`, each alone by exact text replacement in `contact.rs`, the new fast test files run once, the file restored)

| id | mutation | expected to be caught by |
|---|---|---|
| MB1 | the query cell `c(x_s)` one higher in `x` (clamped) | EQ1 or EQ2 |
| MB2 | each face listed only in the cell of its box's lower corner | EQ1 or EQ2 |
| MB3 | the grid built from the boxes not grown by the reach | EQ2 |
| MB4 | `Buckets` visits each cell's list in descending order (ties to the higher face) | EQ1 or EQ2 (or survives: recorded; ties of equal bits occur at shared edges of matching meshes) |
| MS1 | the sphere's second derivative `(I - n n^T)/d` dropped | T1s |
| MS2 | the sphere's normal toward the centre (`-n`) in the gradient | T1s, HS |
| MC1 | the cylinder's axis component not removed (the distance to `point`) | T1c, CL1 |
| MQ1 | the Quad8 second derivatives of the shape functions set to zero | T1q |
| MQ2 | the Tri6 second derivatives set to zero | T1t6 |
| MG1 | a Gauss point's weight without the surface Jacobian | PT20, PT10 |
| MG2 | a Gauss point's force spread equally over the contactor face's nodes (`1/k` in place of `M_a`) | PT20, T1g20 |
| MT1 | the triangle domain test without `u + v <= 1 + 1e-2` | PT4 or PT10 |
| MT2 | the triangle normal reversed (`a_2 x a_1`) | PT4, T1t3 |

## Decision rule

- **done**: every check met on its registered run (the heavy record HSH included), every negative control as expected, every
  planned mutation caught by a check named for it (MB4 may survive only as recorded), B and `baseline_hashes` unchanged, the
  existing tests of `rcfd-fem` passing (with the record of D9), fmt and clippy clean.
- **partial**: B and `baseline_hashes` unchanged and EQ1, EQ2, BT met (the search), and any other miss (or a check not run)
  diagnosed and recorded as a `*_known_miss` test, or a mutation not caught.
- **todo**: otherwise, in particular any change of the default bits (B, EQ1, EQ2, an existing bit record).

## D7. Files

`crates/rcfd-fem/src/contact.rs` (D1 to D4); new test files `crates/rcfd-fem/tests/sa_contact_f601_default_bits.rs` (B),
`sa_contact_search_f601.rs` (EQ1 for the existing models, EQ2, BT, NS), `sa_contact_targets_f601.rs` (T1s, T1c, NT-s, CL0 to
CL5, HS, HSH, SX, R3's target rows), `sa_contact_faces_f601.rs` (PT4, PT10, PT20, T1q, T1t3, T1t6, T1g20, T1g10, NT-q, NG,
HS20, HS10, R3's face rows, R1c, EQ1 for PT and Q20); `crates/rcfd-fem/tests/sa_contact.rs` (D9, one attribute); docs
`docs/reference-data.md` (an F-601 section: Hertz's line and point contact, Flamant, all from memory), the contact module's
doc comment. NS counts the faces visited through a `#[doc(hidden)]` test hook `ContactSet::visited_faces(u) -> (visited,
points, faces)` that runs the search without side effects (the hub's numbers are recorded; the limit is that of NS).

## D8. The bit record (B, b6)

`crates/rcfd-fem/tests/sa_contact_f601_default_bits.rs`, written and run on the base 4d0d0f1b after this commit and before any
library code, its digests printed by that run and written into the file in the same commit, FNV-1a 64 over the bits of: (G1)
ADR 0517's hub at `n = 4` solved by `contact::solve` (displacement, reactions, every node's force, pressure, gap, face, local
coordinates, normal, every Newton residual, energy, penetration, active set); (G2) `ContactSet::evaluate` of the hub at `n = 8`
at the 20 states of EQ2, both reaches; (G3) ADR 0507 T2's punch (total Lagrangian), solved; (G4) ADR 0507 B1's blocks with
`search_distance(0.01)`, solved; (G5) ADR 0507 T1's warped pair evaluated at the 20 states. After the change these digests,
those of `sa_contact_default_bits.rs`, `sa_contact_project_f604_default_bits.rs` and `sa_contact_f615_default_bits.rs`, every
test of `sa_contact.rs` that is not ignored, and `cargo test -p rcfd-api --release --test baseline_hashes` must be unchanged
(pass).

## D9. An existing test superseded by design (ADR 0291 records and copies)

`sa_contact.rs::r1_r2_refusals_a2` registers "a Tet4 contactor face (`InvalidValue` naming the face's element)" (ADR 0507 R1);
landing Tet faces makes that row fail by design (the Tet4 face is accepted and the solve of an unsupported block fails
otherwise). The test gets `#[ignore = "record: ..."]` (the attribute only, the body unchanged), and the copy R1c (in
`sa_contact_faces_f601.rs`) keeps every other row and replaces the Tet4 row as stated in the table. If the crate's run shows
another existing test that fails by design, it is handled the same way by an amendment written before its corrected run.

## Limits (not covered)

- Friction, self-contact, dynamics, smoothing of the faceted target (as ADR 0507).
- The Gauss-point contactor is checked on matching patches, the tangent and the Hertz problem against a rigid sphere; on
  non-matching deformable pairs it may over-constrain (nine points against fewer nodes): not checked beyond EQ1's Q20 (which
  uses a Hex8 contactor).
- Tet4 node contactors on non-matching targets do not pass a patch test (point forces not consistent); not checked.
- Rigid targets are fixed in space: a moving rigid sphere or cylinder is represented by moving the body (as ADR 0507 H). A
  contactor inside a cylinder (a bore) is not a target type.
- Hertz's theories are checked in their half-space idealisation; the tilt of the rigid targets' normal (arithmetic 3, 4) is
  inside the limits, not separated.
- The total Lagrangian body with the new targets and faces is exercised only by the equality of EQ1 (the punch, Hex8).
- No case-file key, no CLI or API surface (b6).

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. **Bucket search default or opt-in** (b5): taken default, with the exact-equality proof of D1 checked by EQ1, EQ2, B.
2. **The contactor of quadratic faces**: options (a) nodes with lumped positive areas (cannot pass the Hex20 patch test,
   finding 3), (b) Gauss points of the load rule (passes it exactly, D3). Taken (b).
3. **Ids of Gauss points**: `n_nodes + k` in `ContactNode::node`, with the new field `point` (D4), rather than a second id type.

Open (not taken): a case-file key (`[[contact]]` of ADR 0507 open question 4, with `target = { sphere | cylinder }`, the
faces now accepted): changes the schema golden.
