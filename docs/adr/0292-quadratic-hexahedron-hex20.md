# ADR 0292: Quadratic hexahedron (Hex20) with its face loads, mass and the quadratic upgrade of a hexahedron box

Status: **REGISTERED 2026-11-13, written before any code of F-389 and before any run of its checks; planned by ADR 0290**.
Not yet accepted by the maintainer. Before this text nothing of F-389 was written, compiled or run: the code and documents under
"What was read" were read and the arithmetic below was done by hand. Results go in the addendum at the end; nothing above the
addendum is changed after a run.

Work package SI-2 of the structural solver, intermediate tier ([ADR 0290](0290-structural-solver-intermediate-tier-plan.md),
section 3, the entry for ADR 0292). Roadmap item F-389 (depends on F-388, F-372, F-374, F-375, F-376, F-383). Size S. Number
0292, fixed by the plan; the file of the planned stub is kept and the stub's text is kept unchanged at the end ("The planned
stub, as written"). Branch `f389-hex20` from main ca2c6b17, worktree `w49i-work/f389`, scratch `w49i-work/f389-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs and not changed after a run (a miss
becomes an ignored `*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical (every model
without a Hex20 block runs the code it ran before); no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any
golden file; no existing test edited; no new dependency, no unsafe code; clean-room.

## What F-389 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. Hex20 on the reference element and on randomly
> distorted ones (registered seed): shape functions sum to 1 and derivatives to 0 within 1e-14, analytic derivatives agree with
> central differences within 1e-7, the 3 x 3 x 3 rule integrates every monomial of degree <= 5 per direction within 1e-13, the
> stiffness is symmetric within 1e-13, has rank 54 and gives no force for the six rigid motions within 1e-11. Patch test with all
> six strains on a box with displaced interior nodes: 1e-10. A uniform pressure on an 8-node face gives the nodal loads -1/12
> (corners) and 1/3 (mid-edge) of p A within 1e-13, and uniaxial tension the closed form within 1e-10. Cantilever L/h = 20, nu =
> 0.3, ONE element through the thickness and 10 along: tip deflection within 1 percent of P L^3/(3 E I) (1 + 3 E I/(kappa G A
> L^2)). Thick cylinder b/a = 2 in plane strain on a curved mesh: bore displacement within 0.2 percent and bore hoop stress
> within 1 percent of Lame. Manufactured solution of F-378: observed L2 order >= 2.7. Mass: total mass and inertia tensor of a
> box within 1e-12; first axial frequency of a fixed-free bar (nu = 0) within 0.1 percent at 8 elements. The upgrade has (nodes
> + edges) nodes.

Clauses: (a1) partition of unity of the functions and of their derivatives, reference and distorted, 1e-14; (a2) analytic
derivatives against central differences, 1e-7; (a3) the 3 x 3 x 3 rule on every monomial of degree <= 5 per direction, 1e-13;
(a4) stiffness symmetric, 1e-13; (a5) rank 54; (a6) no force for the six rigid motions, 1e-11; (a7) patch test, six strains,
displaced interior nodes, 1e-10; (a8) pressure on an 8-node face: -1/12 and 1/3 of p A, 1e-13; (a9) uniaxial tension, closed
form, 1e-10; (a10) cantilever, 1 percent; (a11) thick cylinder, bore displacement 0.2 percent, bore hoop stress 1 percent; (a12)
manufactured solution, L2 order >= 2.7; (a13) box mass and inertia tensor, 1e-12; (a14) first axial bar frequency at 8
elements, 0.1 percent; (a15) the upgrade has nodes + edges nodes.

The brief adds: keep the edits inside `hex20.rs` and the files the stub assigns (`hex20.rs`, `tests/si_hex20.rs`); confirm the
node order (VTK type 25) from a citable published cell definition; set `PENDING` to `None`; every existing test of rcfd-fem,
rcfd-post, rcfd-io passes unedited, except that the Hex20 refusal tests of ADR 0291 stop applying (they are not edited; the
assertions that fail are listed in the addendum as the remaining step for the maintainer); `baseline_hashes` in release.

## What was read (main ca2c6b17, read only)

1. `AGENT_RULES.md`; ADR 0290 (section 3, the entries for 0291 and 0292; the wave table: wave 2 holds 0292, 0293, 0296, 0298,
   0301, 0302, of which none edits `loads.rs` or `mesh.rs`); ADR 0291 (D1 to D8, the addendum); ADR 0280 (planned stub: the
   closed forms of the cantilever and Lame); this ADR's stub.
2. Code: `crates/rcfd-fem/src/{hex20.rs, element.rs, mesh.rs, recover.rs (fit_basis, extrapolation_matrix, nodal_average),
   loads.rs, mass.rs, quadrature.rs, builders.rs (hex_box, upgrade_to_tet10, MidEdgeNode), lib.rs}`; the delegations of
   `mesh.rs`, `element.rs`, `recover.rs` to `hex20`; tests `sb_mms.rs` (F-378: field, body force, error rule, cases D and T),
   `sb_static.rs` (patch, uniaxial, `rebuild`, 3-2-1 supports), `sb_modal.rs` (box Gram matrix, the bar), `si_scaffold.rs`
   (X1, X3, R1, R10 name Hex20).
3. The VTK cell definition of `vtkQuadraticHexahedron` (the public VTK class documentation,
   https://vtk.org/doc/nightly/html/classvtkQuadraticHexahedron.html, read 2026-11-13; VTK is open source, BSD licence; only the
   node order was read, no code): "point ids 0-7 are the eight corner vertices of the cube; followed by twelve midedge nodes
   (8-19)", "these midedge nodes ... lie on the edges defined by (0,1), (1,2), (2,3), (3,0), (4,5), (5,6), (6,7), (7,4), (0,4),
   (1,5), (2,6), (3,7)". That is the order declared by ADR 0291 D2 (`HEX_EDGES`): **confirmed**. The corner order is VTK type 12's,
   which the basic tier already writes with the identity map for Hex8.
4. **Not read:** any finite-element code outside this repository.

Findings that shape the design:

1. `loads::face_shape_functions` and `loads::face_rule` refuse a face of 8 nodes (`"a face of 8 nodes (faces of 3, 4 or 6 nodes
   are supported)"`), so `Load::Traction` and `Load::Pressure` on a Hex20 surface set are refused by name through the model
   path; `SolidMesh::face_area` and `face_area_vector` reach `unreachable!` for an 8-node face (a panic). Both are in shared
   files outside this package (finding 1 of the open questions). Within the package, `hex20.rs` gets the 8-node face functions,
   the face rule and the consistent face load (by face, by surface set, for pressure, uniform traction and a traction field),
   returned as per-node forces for `solve::solve_static_with`; the checks use that path.
2. Body loads, mass, recovery, volume and the `det J` check of `add_block` go through `element_for`, `ElementKind::
   shape_gradients`, `reference_nodes`, `faces`, `hex20::fit_basis` and `hex20::volume`: filling `hex20.rs` makes them work
   without an edit elsewhere.
3. `extrapolation_matrix` builds `P(nodes) V^-1` from `fit_basis` at the points of the stiffness rule: the fit basis must have 27
   functions unisolvent on the 3 x 3 x 3 Gauss points.

## Design

### D1. Node order, reference nodes, faces

Corners 0 to 7 as Hex8 (`(-,-,-), (+,-,-), (+,+,-), (-,+,-)`, then the same at `zeta = +1`); mid-edge node `8 + k` at the
middle of edge `k` of `HEX_EDGES` (finding 3 of the reading: VTK type 25). Reference coordinates: the corners `(+-1, +-1, +-1)`,
the mid-edge nodes the means of their two corners (one coordinate 0). Faces: the Hex8 faces (counter-clockwise from outside,
`HEX_FACES`), each followed by its four mid-edge nodes in the order of the face edges `(c0 c1), (c1 c2), (c2 c3), (c3 c0)`:

| Face | Corners | Mid-edge nodes |
| --- | --- | --- |
| 0 (zeta = -1) | 0 3 2 1 | 11 10 9 8 |
| 1 (zeta = +1) | 4 5 6 7 | 12 13 14 15 |
| 2 (eta = -1) | 0 1 5 4 | 8 17 12 16 |
| 3 (xi = +1) | 1 2 6 5 | 9 18 13 17 |
| 4 (eta = +1) | 2 3 7 6 | 10 19 14 18 |
| 5 (xi = -1) | 3 0 4 7 | 11 16 15 19 |

(Derived: edge (0,3) is `HEX_EDGES[3]`, node 11; (3,2) is edge 2, node 10; and so on.) The local face index of each box face
is therefore the Hex8 one, so the surface sets of `hex_box` keep their meaning after the upgrade.

### D2. Shape functions (serendipity family)

Method: Zienkiewicz and Taylor, The Finite Element Method, vol. 1, the serendipity family of quadratic hexahedra (from memory;
the functions are derived here and checked by a1, a2 and the Kronecker property). With `(xi_a, eta_a, zeta_a)` the reference
coordinates of node `a` and `s_a = xi xi_a + eta eta_a + zeta zeta_a`:

- corner: `N_a = (1 + xi xi_a)(1 + eta eta_a)(1 + zeta zeta_a)(s_a - 2) / 8`. The trilinear factor vanishes at every node with a
  coordinate opposite to the corner's; the linear factor `s_a - 2` vanishes at the three mid-edge nodes next to the corner (two
  coordinates equal to the corner's, one 0: `1 + 1 + 0 - 2`); at the corner `8 (3 - 2) / 8 = 1`.
- mid-edge with `xi_a = 0`: `N_a = (1 - xi^2)(1 + eta eta_a)(1 + zeta zeta_a) / 4` (and the two permutations). It is 1 at its node,
  0 at every corner (`1 - xi^2 = 0`) and at every other mid-edge node (a factor `1 + eta eta_a` or `1 + zeta zeta_a` vanishes, or
  `1 - xi^2` does for the three edges parallel to it).
- derivatives: corner `dN_a/dxi = xi_a (1 + eta eta_a)(1 + zeta zeta_a)(2 xi xi_a + eta eta_a + zeta zeta_a - 1) / 8` (from
  `d/dxi[(1 + xi xi_a)(s_a - 2)] = xi_a (s_a - 2) + xi_a (1 + xi xi_a)`), and cyclically; mid-edge `dN_a/dxi = -xi (1 + eta eta_a)
  (1 + zeta zeta_a) / 2`, `dN_a/deta = eta_a (1 - xi^2)(1 + zeta zeta_a) / 4`, `dN_a/dzeta = zeta_a (1 - xi^2)(1 + eta eta_a) / 4`.

The 20 functions span the serendipity space `{1, xi, eta, zeta, xi^2, eta^2, zeta^2, xi eta, eta zeta, zeta xi, xi^2 eta, xi^2
zeta, eta^2 xi, eta^2 zeta, zeta^2 xi, zeta^2 eta, xi eta zeta, xi^2 eta zeta, eta^2 xi zeta, zeta^2 xi eta}`, which holds every
polynomial of total degree <= 2: the element is complete to degree 2 (L2 order 3, energy order 2 on smooth solutions).

### D3. Element, rules, volume

`hex20::Hex20` implements `Element`: `stiffness_rule = mass_rule = quadrature::gauss_3()` (27 points, degree 5 per direction).
`element()` returns it; `PENDING = None`.

Why 27 points (arithmetic 1): for an affine element `B^T D B` has degree 4 per direction (each derivative of a serendipity
function has degree <= 2 per direction): `gauss_3` is exact. For a curved element the patch test needs `sum_q w_q det J grad N_a`
exact: `det J grad N_a = sum_j C_{j k} dN_a/dxi_j`, with the cofactors `C` of `J`. Each column of `J` has degree (1, 2, 2) per
direction for `d/dxi` (and cyclically); a cofactor such as `y_eta z_zeta - y_zeta z_eta` has degree <= (4, 3, 3); times `dN/dxi`
(1, 2, 2) it is (5, 5, 5); the other two terms are (5, 5, 5) as well. So `gauss_3` integrates `det J grad N_a` exactly on any
Hex20 geometry, and the patch test (a7) passes on curved elements to rounding. The 2 x 2 x 2 rule leaves zero-energy modes beyond
the six rigid ones (negative control N4).

`volume(det J at xi)`: `sum_q w_q det J(xi_q)` over `gauss_3`; `det J` has degree <= 5 per direction (each column degree (1, 2,
2) in its own permutation), so the volume is exact for every Hex20 geometry. Mass (arithmetic 2): `N_a N_b det J` has degree 4
per direction on an affine element (exact with `gauss_3`); on a curved one it is not exact (accepted: a mass rule, as Tet10's).

### D4. Recovery (stiffness-rule points to nodes)

`fit_basis(xi)`: the 27 triquadratic monomials `xi^i eta^j zeta^k`, `i, j, k` in 0..=2, in the order `k`, then `j`, then `i`
fastest (the rule's order). It is unisolvent on the 3 x 3 x 3 Gauss points (the tensor product of three 1-D quadratic
interpolations on 3 distinct points), so `extrapolation_matrix(Hex20)` is the triquadratic interpolant of the 27 values evaluated
at the 20 nodes. This is the registered rule (the stub's "not unique": a 20-term serendipity fit by least squares would also be
possible; the triquadratic interpolant is exact for every stress field of degree <= 2 per direction and reproduces constant and
linear stress exactly). 1-D estimate of its error at a node (arithmetic 5): a cubic `xi^3` is interpolated on the Gauss points as
`0.6 xi` (the points are the zeros of `xi^3 - 0.6 xi`), so the error at `xi = +-1` is `0.4` times the cubic coefficient.

### D5. Faces and face loads

8-node face functions on `[-1, 1]^2`, corners `(-,-), (+,-), (+,+), (-,+)`, then the mid-edge nodes `(0,-1), (1,0), (0,1), (-1,0)`
(the face edge order of D1): corner `(1 + u u_a)(1 + v v_a)(u u_a + v v_a - 1) / 4`; mid-edge with `u_a = 0` `(1 - u^2)(1 + v v_a)
/ 2`, with `v_a = 0` `(1 + u u_a)(1 - v^2) / 2`. They are the traces of the Hex20 functions on the face (same derivation as D2).
Face rule: 3 x 3 Gauss (degree 5 per direction). The consistent load `f_a = int N_a t dS = sum_q w_q N_a(q) t(x_q, x_u x x_v)`,
with `x_u x x_v` the area vector density (outward by the face orientation): pressure `t = -p (x_u x x_v)`, uniform traction `t =
t0 |x_u x x_v|`, traction field `t(x) |x_u x x_v|`. On a curved face the pressure integrand `N_a (x_u x x_v)` has degree <= 2 + 3
= 5 per direction (each tangent has degree (1, 2) or (2, 1)): the pressure load is exact on any 8-node face.

Arithmetic 3 (the corner load): on a flat parallelogram face of area `A`, `dS = (A / 4) du dv`. Corner (take `u_a = v_a = 1`,
`s = u`, `t = v`): `(1/4) int int (1 + s)(1 + t)(s + t - 1) = (1/4)(4/3 + 4/3 - 4) = -1/3`, times `A / 4`: `-A / 12`. Mid-edge:
`(1/2) int (1 - u^2) du int (1 + v) dv = (1/2)(4/3)(2) = 4/3`, times `A / 4`: `A / 3`. Sum `4 (-1/12) + 4 (1/3) = 1`. So the nodal
forces of a uniform pressure `p` are `w_a (-p A n)` with `w = -1/12` at the corners and `1/3` at the mid-edge nodes: the corner
force points **out of** the solid (the stub's surprise).

API in `hex20`: `face_shape_functions(uv) -> ([f64; 8], [[f64; 2]; 8])`, `face_rule() -> [([f64; 2], f64); 9]`, `face_load(coords
of 8 nodes, t: Fn(x, area density) -> force density) -> [Vec3; 8]`, and for a surface set of Hex20 faces `pressure_loads(mesh,
set, p)`, `traction_loads(mesh, set, t0)`, `traction_field_loads(mesh, set, t(x))`, each `Result<Vec<NodalForceAt>>` sorted by
node (sums per node in face order, then point order); a face of another kind, an unknown set and a non-finite value are refused by
name. `face_area(coords)`: the 3 x 3 Gauss sum of `|x_u x x_v|`.

### D6. The upgrade builder

`upgrade_to_hex20(mesh) -> Result<(SolidMesh, Vec<MidEdgeNode>)>` and `upgrade_to_hex20_mapped(mesh, map)`. The input is a mesh of
Hex8 blocks (another kind refused as `UnsupportedKind { block, kind, operation: "upgrade_to_hex20" }`). One new node per edge,
numbered after the existing nodes in the order of first appearance (element order, then `HEX_EDGES` order), at the midpoint
`0.5 (p_a + p_b)` of its two ends; with a map, every node of the output is `map(p)` of its input position (the corners `map(p_a)`,
the mid-edge nodes `map(0.5 (p_a + p_b))`): the input mesh is the parameter box and `map` the coordinate map (for example polar
coordinates for a cylinder: the mid-edge nodes then lie on the arcs). A non-finite mapped node is refused. Elements keep their
numbers, element and surface sets their members (D1 keeps the face indices); a node set gains the mid-edge node of each edge
whose two ends it contains (as `upgrade_to_tet10`). `add_block` checks `det J > 0` at the 20 nodes of each element. The output
has `nodes + edges` nodes (a15). `MidEdgeNode` is the type of `builders` (re-used).

### D7. What stays

Every other file is unchanged. A model without a Hex20 block calls no code of `hex20.rs` (the dispatch arms exist since ADR 0291),
so every default result keeps its bits; `baseline_hashes` is run as a check (G1).

## Registered checks

Tests in `crates/rcfd-fem/tests/si_hex20.rs`, release profile, run once (`cargo test -p rcfd-fem --release --test si_hex20 --
--nocapture --test-threads=1`). Seed `0xF389_0292` (SplitMix64 as in `sb_static.rs`, a deterministic source only). Material
`E = 2e11`, `nu = 0.3`, `rho = 7850` unless stated. Distorted elements (registered): 20 elements, each the reference nodes with
every corner coordinate moved by `0.15 (2U - 1)` and then every mid-edge node placed at the mean of its moved corners plus `0.05
(2U - 1)` per coordinate (curved edges); random points: 100 points with coordinates `2U - 1`.

| Id | Clause | Check | Limit | Estimate behind it |
| --- | --- | --- | --- | --- |
| E0 | D1 | `PENDING == None`, `Hex20.pending() == None`, `element_for(Hex20).kind() == Hex20`; 20 reference nodes, node `8 + k` the mean of the corners of `HEX_EDGES[k]` (= `ElementKind::Hex20.edges()[k]`) bit for bit; faces as the table of D1; each face's mid-edge nodes are the means of its consecutive corners | as stated | D1, VTK type 25 |
| E1 | a1 | reference: at the 20 nodes, the 27 Gauss points and the 100 random points, `|sum N_a - 1|` and `|sum_a dN_a/dxi_j|` | <= 1e-14 | sums of 20 terms of size <= 1: rounding ~1e-15 |
| E1b | a1 | distorted (20 elements, at the 27 Gauss points and the 100 random points): `|sum N_a - 1|` and `|sum_a dN_a/dx_k|` (physical gradients of `Element::point`) | <= 1e-14 | `J^-1` of size ~1 |
| E1c | D2 | Kronecker: `|N_a(xi_b) - delta_ab|` at the 20 nodes | <= 1e-15 | exact arithmetic on +-1, 0 |
| E2 | a2 | reference: `dN_a/dxi_j` against `(N_a(xi + h e_j) - N_a(xi - h e_j)) / (2h)`, `h = 1e-5`, 100 random points, all a, j | <= 1e-7 absolute | truncation `h^2 N''' / 6 ~ 1e-10`, rounding `1e-16 / 1e-5 = 1e-11` |
| E2b | a2 | distorted: `J = sum_a x_a (dN_a/dxi)^T` of `Element::point` against the central differences of `x(xi) = sum N_a x_a`, `h = 1e-5`, relative to `max |J|` | <= 1e-7 | as E2 |
| E3 | a3 | `gauss_3` on `xi^a eta^b zeta^c`, `a, b, c` in 0..=5 (216 monomials), against `prod (1 - (-1)^(k+1)) / (k + 1)` | <= 1e-13 absolute | Gauss 3 exact to degree 5; rounding ~1e-16 |
| E4 | a4 | `max |K_ij - K_ji| / max |K|`, reference element and 20 distorted | <= 1e-13 | both triangles summed in the same order of products: rounding ~1e-16 |
| E5 | a5 | rank of `K` (60 x 60): the number of eigenvalues (cyclic Jacobi in the test, to off-diagonal `<= 1e-15 max |K|`) above `1e-10 lambda_max` | = 54 on the reference and on all 20 distorted elements | D3: 27 points, six rigid modes only |
| E6 | a6 | `max_i |(K r)_i| / (max |K| max |r|)` for the three translations and the three rotations about the element centroid, reference and distorted | <= 1e-11 | `B r = 0` pointwise; rounding ~60 x 1e-16 |
| E7 | D3 | `hex20::volume` (through `SolidMesh::element_volume`) against a 5 x 5 x 5 Gauss sum of `det J` (Gauss-Legendre nodes by Newton as in `sb_mms.rs`) on the 20 distorted elements, relative; and 8 for the reference cube | <= 1e-13 | both exact (degree 5) |
| E8 | D3 | element mass of an affine element (the box `[0.3, 0.5, 0.7]` sheared by `x += 0.2 y + 0.1 z`, `y += 0.15 z`) against the 5 x 5 x 5 Gauss integral of `rho N_a N_b det J`, `max |dM| / max |M|` | <= 1e-13 | gauss_3 exact at degree 4 |
| P1 | a7 | patch: `hex_box([0.1, -0.2, 0.3], [1.2, 1.0, 0.8], [3, 3, 3])`, its interior grid nodes moved by `0.1 h_k (2U - 1)`, upgraded, then the interior mid-edge nodes (in no box-face node set) moved by `0.05 h_k (2U - 1)`; every boundary node prescribed with `u = c + A x` (`A` with nine non-zero entries, the field of `sb_static.rs`: all six strains and a rotation); interior nodal error `max |u_h - u| / max |u|` | <= 1e-10 | D3: exact quadrature of `det J grad N`; rounding of a direct solve of ~2000 unknowns |
| P2 | a7, D4 | same: integration-point strain against `sym(A)` (`max |d eps| / max |eps|`), and `nodal_average` stress against `D eps` at every node (`/ max |sigma|`) | <= 1e-10 | constant strain; the extrapolation reproduces constants |
| L1 | a8 | uniform pressure `p = 2.5e6` on each of the 6 faces (surface sets of one face) of two single elements, the box `[0.3, 0.5, 0.7]` and the affine element of E8: at every one of the 20 nodes `|f_a - w_a (-p A n)|_inf / (p |A n|)` with `w_a = -1/12` (face corner), `1/3` (face mid-edge), `0` (not on the face), `A n = (c2 - c0) x (c3 - c1) / 2` from the face's corner positions | <= 1e-13 | arithmetic 3; 9 products per entry |
| L2 | a9 | uniaxial: `hex_box([0, 0, 0], [2, 0.5, 0.4], [3, 2, 2])` upgraded; traction `+-F/A e_x` on `xmax`, `xmin` by `traction_loads` (`F = 1e6`); 3-2-1 supports at the corners (0,0,0): x, y, z, (0,0,0.4): x, y, (0,0.5,0): x; the elongation error of every `xmax` node and the lateral errors `|u_y + nu eps y|`, `|u_z + nu eps z|` of every node as in `sb_static.rs` U1, U2 | <= 1e-10 | the exact field is linear, in the space; consistent loads exact |
| L3 | finding 1 | `Load::pressure` on a Hex20 surface set through `loads::nodal_load_vector`: refused, message contains "a face of 8 nodes" (never ignored) | as stated | finding 1 |
| C1 | a10 | cantilever `hex_box([0, 0, 0], [2.0, 0.1, 0.1], [10, 1, 1])` upgraded (one element through thickness and width, ten along, element aspect 2); `xmin` clamped (three components); tip load `P = 1000 N` as the uniform traction `(0, 0, -P / h^2)` on `xmax` by `traction_loads`; deflection `delta = -sum_a f_az u_az / P` (the area mean of `u_z` over the tip face, exact for the consistent load); `|delta / delta_T - 1|` with `delta_T = P L^3 / (3 E I) (1 + 3 E I / (kappa G A L^2))`, `kappa = 5/6` | <= 0.01 | arithmetic 4 |
| T1 | a11 | thick cylinder, `a = 1`, `b = 2`, plane strain, quarter model: parameter box `hex_box([1, 0, 0], [1, pi/2, 0.2], [4, 8, 1])` upgraded with the map `(r, theta, z) -> (r cos theta, r sin theta, z)`; `u_z = 0` on `zmin` and `zmax`, `u_y = 0` on `ymin` (theta = 0), `u_x = 0` on `ymax` (theta = pi/2); bore pressure `p = 1e8` on `xmin` by `pressure_loads`. T1a: `max |u_r / u_r(a) - 1|` over the bore nodes (node set `xmin`), `u_r = u . x / |x|` | <= 0.002 | arithmetic 5 |
| T1b | a11 | hoop stress `t^T sigma t`, `t = (-sin, cos, 0)`, of `nodal_average(None)` at every bore node, `max |s / s_L(a) - 1|`, `s_L(a) = 5 p / 3` | <= 0.01 | arithmetic 5 |
| T1c | D6 | every mid-edge node of the bore has `|x| = 1` within 1e-15 | as stated | D6 (mapped, not interpolated) |
| M1 | a12 | F-378 manufactured field (`sb_mms.rs` arithmetic 1: `K`, `PHI`, `U = 1`, `E = 1`, `nu = 0.3`, unit cube), Hex20 (`hex_box` [n; 3] upgraded), n = 2, 4, 8, case D (every face prescribed, nodal values) and case T (`xmax`, `ymax`, `zmax` loaded by `sigma n` through `traction_field_loads`); errors with the 125-point Gauss rule (degree 9 per direction); L2 order of the finest pair | >= 2.7 (D and T) | complete quadratic: order 3; Tet10 met 2.7 on the same sequence (F-378) |
| M2 | a12 | same: energy order of the finest pair; both errors fall monotonically | >= 1.8; monotone | order 2 |
| B1 | a13 | `hex_box([0.1, -0.2, 0.3], [1.2, 1.0, 0.8], [4, 3, 3])` upgraded, `rho = 7850`, `mass::consistent`, `mass_properties`: `|m / m_exact - 1|`, `max |I - I_exact| / max |I_exact|` (inertia about the centre, closed form of `sb_modal.rs`), `max |c - c_exact| / |c_exact|` | <= 1e-12 | rigid motions are in the space; `rho r . s` degree 2 per direction, exact |
| B2 | a14 | bar `hex_box([0, 0, 0], [1, 0.1, 0.1], [8, 1, 1])` upgraded, `nu = 0`, `u_x = 0` on `xmin`, `u_y = 0` on `ymin`, `u_z = 0` on `zmin` (the bar of `sb_modal.rs`), `modal_analysis(ModalOptions::new(4))`; the lowest mode with axial kinetic fraction `>= 0.99` is mode 0; `|f_1 / f_exact - 1|`, `f_exact = sqrt(E/rho) / (4 L)` | <= 1e-3; and `f_1 / f_exact - 1 >= -1e-12` (Rayleigh-Ritz upper bound) | arithmetic 6 |
| U1 | a15, D6 | `hex_box([0, 0, 0], [1, 1, 1], [3, 2, 2])`: the upgrade has `nodes + edges` nodes, the edges counted in the test from the Hex8 connectivity as unique corner pairs, and both equal the closed form `36 + 75 = 111`; `mids.len() = 75`; every mid-edge node at `0.5 (p_a + p_b)` bit for bit; each box-face node set has its corners plus the mid-edge nodes of its edges (`xmin`, `xmax`: 9 + 12, `ymin`, `ymax`: 12 + 17, `zmin`, `zmax`: 12 + 17); surface sets and element sets equal; a Tet4 box refused with `UnsupportedKind` naming block 0 and "upgrade_to_hex20" | as stated | D6 |
| S1 | brief | every other test file of rcfd-fem, rcfd-post, rcfd-io, release, unedited (`git diff main -- 'crates/*/tests'` lists only `si_hex20.rs`) | 0 failed except the ADR 0291 refusal assertions of Hex20, listed in the addendum | D7 |
| G1 | brief | `cargo test -p rcfd-api --release --test baseline_hashes` | passes, file unchanged | D7 |

Arithmetic 4 (cantilever): `h = 0.1`, `L = 2`, `A = h^2`, `I = h^4 / 12`, `G = E / 2.6`: `3 E I / (kappa G A L^2) = 3 (E/G) h^2
/ (12 kappa L^2) = 3 x 2.6 x 0.01 / (12 x 5/6 x 4) = 0.00195`, the shear term 0.2 percent. `delta_T = 1000 x 8 / (3 x 2e11 x
8.333e-6) x 1.00195 = 1.6031e-3 m`. Error sources: (i) the tip-load deflection is cubic in x; the element holds `x^2 z` in `u_x`
(rotation quadratic along x) and `x^2` in `u_z` (deflection quadratic): with ten elements the bending energy of a cubic
deflection approximated by piecewise quadratics converges at `O(h^2)` in energy, about `(1/10)^2 / 12` relative, below 0.1
percent; (ii) shear locking of equal-order quadratic interpolation of `u_z` and of the rotation: grows with `(L_e / t)^2`, here
`L_e / t = 2`, estimated well below 1 percent for 27-point Hex20 (from memory, 20-node bricks of aspect 5 are within about 2
percent on slender-beam tests; the parasitic term scales with the square of the aspect); (iii) the clamp with `nu = 0.3`
restrains the Poisson contraction at the root (ADR 0280 estimates below 0.5 percent at `L/h = 20`). Expected: 0.1 to 0.8 percent
below `delta_T`. The 1 percent limit is the acceptance's; it is met with a modest margin if the estimates hold.

Arithmetic 5 (cylinder): Lame, from first principles: `u_r = C1 r + C2 / r` solves `d/dr (1/r d(r u)/dr) = 0`; `sigma_r = A - B /
r^2`, `sigma_theta = A + B / r^2` with `sigma_r(a) = -p`, `sigma_r(b) = 0`: `A = p a^2 / (b^2 - a^2) = p / 3`, `B = A b^2 = 4p/3`;
plane strain `sigma_r + sigma_theta = 2 (lambda + mu) 2 C1`, `sigma_theta - sigma_r = 4 mu C2 / r^2`: `C1 = (1 + nu)(1 - 2 nu) A /
E`, `C2 = (1 + nu) B / E`, so `u_r(a) = (1 + nu)/E ((1 - 2 nu) A a + B / a) = 1.3/E (0.4/3 + 4/3) p = 1.9067 p / E`, `s_L(a) = A + B
= 5p/3`. Errors: geometry, the quadratic arc of 11.25 degrees deviates from the circle by about `r dtheta^4 / 512 ~ 3e-6 r`;
displacement, `B/r` interpolated by quadratics over `h = 0.25`, nodal values of quadratic elements are accurate to far better
than the 0.2 percent (`h^3 f''' / 72 ~ 1e-3` of `B/a` pointwise between nodes, much less at nodes); circumferential, `cos` over
`dtheta = 0.196` by quadratics, `dtheta^3 / 125 ~ 6e-5`. Bore hoop stress: the cubic term of `B / r^2` about the centre of the
first element (`r_c = 1.125`, half-length 0.125): `-4 B / r_c^5 x 0.125^3 = -4.3e-3 B`; extrapolated from the Gauss points with
`0.4` (D4): `1.7e-3 B`, i.e. `1.7e-3 x (4/3) / (5/3) = 0.14` percent; with the circumferential derivative error estimated at a
few 1e-4 and the FE stress error at the Gauss points smaller still, about 0.2 to 0.4 percent against the 1 percent limit.

Arithmetic 6 (bar): with `nu = 0` the 3-D bar is the 1-D bar exactly (the axial field `u_x(x)` with no lateral motion is in the
space); for quadratic elements with consistent mass the eigenvalue error is `O((k h)^4)`: `k h = (pi/2)/8 = 0.196`, `(k h)^4 =
1.5e-3`, times a constant of order `1/720`: about `2e-6` relative in `lambda`, `1e-6` in `f`, against `1e-3`. Rayleigh-Ritz:
the consistent mass gives an upper bound.

**Negative controls** (each must hold, else the check is not sensitive):

| Id | Control | Expected |
| --- | --- | --- |
| N1 | P1 with the cubic field `u = 1e-3 (x^3, y^3, z^3)` (not in the space) | interior error `> 1e-6` |
| N2 | C1 on the same mesh with Hex8 (not upgraded): shear locking | `delta / delta_T < 0.9` |
| N3 | M1 case D with the lambda term of the body force reversed (F-378 N1) | L2 and energy orders `< 0.5` |
| N4 | E5 with `stiffness_with_rule(gauss_2)` on the reference element | rank `< 54` |

**Mutations** (on a `git archive` copy of the final tree in `w49i-work/f389-scratch`, own target directory; each applied alone
by exact text replacement in `hex20.rs`, `si_hex20` run once, the file restored and compared byte for byte):

| Id | Mutation | Must fail |
| --- | --- | --- |
| M1 | corner function: `s_a - 2` becomes `s_a - 1.9` (values only) | E1 |
| M2 | corner derivative: the `- 1` of `2 xi xi_a + eta eta_a + zeta zeta_a - 1` becomes `- 0.9` (derivatives only) | E2 |
| M3 | stiffness rule `gauss_2` | E5 |
| M4 | face 1 lists mid-edge node 17 in place of 13 | E0, L1 |
| M5 | upgrade: the mid-edge node at `p_a` instead of the midpoint | U1 |
| M6 | `volume` with `gauss_2` | E7 |
| M7 | mass rule `gauss_2` | E8 |
| M8 | upgrade: node sets not extended by the mid-edge nodes | U1 |
| M9 | face functions: the mid-edge factor `/ 2` becomes `/ 4` | L1 |

## Decision rule

**Done** if every check meets its limit, every negative control holds and every mutation is caught by its registered check on
the first run, and the only failures of existing tests are the ADR 0291 assertions that Hex20 is refused (expected; listed for
the maintainer); **partial** if a check misses or is not run, or a mutation survives, or another existing test fails; **todo** if
Hex20 does not assemble and solve.

## Limits (what is not covered)

- `Load::Traction` and `Load::Pressure` of the model on Hex20 surface sets stay refused by `loads.rs` (by name, L3), and
  `SolidMesh::face_area` / `face_area_vector` of an 8-node face panic (`unreachable!` in `mesh.rs`): both are shared files outside
  this package (open question 1). The face loads of Hex20 are available as per-node forces from `hex20` for `solve_static_with`.
- The mass rule is exact on affine Hex20 only; reduced integration and the 2 x 2 x 2 rule are not offered.
- No mixed Hex20/Tet10 or Hex20/Hex8 transition elements; no wedge or pyramid.
- The cantilever is checked for deflection only (no stress check is in the acceptance).

## Open questions, taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. Face loads through the model path. Options: (a) leave `loads.rs` and `mesh.rs` alone in this package (the brief's scope) and
   provide the Hex20 face loads in `hex20.rs` as per-node forces; a follow-up item adds the two dispatch arms (`8 =>` in
   `loads::face_shape_functions` / `face_rule` and in `SolidMesh::face_tangents`), which leave every other face count
   bit-identical; (b) add the arms here. Recommendation (a), because the brief keeps this package inside its own files and the
   arms are a separate small change to shared files. Taken on the maintainer's standing instruction of 2026-11-11 (take the
   recommendation).
2. The recovery rule (D4): the triquadratic interpolant of the 27 Gauss values (option a) rather than a 20-term serendipity least
   squares fit (b). Recommendation (a): exact for degree 2 per direction, the same construction as Hex8's (`fit_basis` with as
   many functions as points). Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).
3. The ADR 0291 refusal assertions of Hex20 (si_scaffold X1, X3, R1, R10) stop applying when `PENDING` becomes `None`; house
   rule 2 forbids editing them. Not taken: listed in the addendum as the remaining step for the maintainer (options: mark them
   `#[ignore]` as records of ADR 0291, or drop the Hex20 rows).

## Amendment 1 (2026-11-13, written and committed before the corrected run)

The registered run of `si_hex20` (commit d1ef9d74, once, release) printed for C1 the ratios `-0.981949` (Hex20) and
`-0.369197` (Hex8): the test computes `delta = -sum_a f_az u_az / P`, but with the load `f_az = -P s_a` (shares `s_a` summing to
1) and the deflection `u_az < 0` the registered quantity, the area mean of the downward deflection, is `+sum_a f_az u_az / P`.
A defective set-up (the sign of the measure), not a result: C1 read `|ratio - 1| = 1.98`, and N2 was met vacuously by a
negative number. Corrected: `delta = sum_a f_az u_az / P`, nothing else changes (mesh, load, clamp, limits 0.01 and 0.9). The
first-run test stays as the ignored record `c1_n2_cantilever_one_element_through_the_thickness`; the corrected checks are the
new tests `c1_amended_hex20_cantilever` and `n2_amended_hex8_cantilever_locks`, each run once.

Disclosed: the magnitudes of the first run are known before the corrected run (0.981949 for Hex20, i.e. 1.8 percent below
`delta_T`, which will miss the 1 percent limit; 0.369 for Hex8). After the first run and before this amendment, unregistered
diagnostics were run once on the cantilever and on T1 (a temporary test file, deleted, not committed; numbers in the addendum),
to diagnose the two misses; no limit, mesh or parameter is changed by them.

T1 (run once): T1a and T1c met, T1b missed (`1.1681e-2` against `1e-2`); its test is the ignored record
`t1_thick_cylinder_plane_strain_known_miss`, unchanged. So that T1a and T1c stay covered by a fast test, a new test
`t1_amended_met_clauses` runs the same set-up and asserts T1a and T1c with their registered limits, printing T1b without
asserting it (as ADR 0279 amendment 1 did for Tet4).

## The planned stub, as written

Status: **PLANNED 2026-11-12** (layout by the intermediate-tier plan, [ADR
0290](0290-structural-solver-intermediate-tier-plan.md)); the registration is written by the implementation agent before any code.
Nothing here is a registered limit: the numbers below are the acceptance draft that the registration turns into checks.

Work package SI-2 of the structural solver, intermediate tier. Size S.

### Goal

The 20-node serendipity hexahedron: shape functions, 3 x 3 x 3 Gauss rule, stiffness, consistent mass, the consistent load of its
8-node faces, strain recovery, and the builder that upgrades a box of Hex8 to Hex20 by mid-edge nodes (with a coordinate map for
curved edges). It is the element for structured meshes of thin parts: one element through the thickness bends correctly.

### Dependencies

ADR 0291; roadmap items (SI ids are this tier's packages, ADR 0290 section 7): SI-1, F-372, F-374, F-375, F-376, F-383.

### Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. Hex20 on the reference element and on randomly distorted ones
(registered seed): shape functions sum to 1 and derivatives to 0 within 1e-14, analytic derivatives agree with central differences
within 1e-7, the 3 x 3 x 3 rule integrates every monomial of degree <= 5 per direction within 1e-13, the stiffness is symmetric
within 1e-13, has rank 54 and gives no force for the six rigid motions within 1e-11. Patch test with all six strains on a box with
displaced interior nodes: 1e-10. A uniform pressure on an 8-node face gives the nodal loads -1/12 (corners) and 1/3 (mid-edge) of
p A within 1e-13, and uniaxial tension the closed form within 1e-10. Cantilever L/h = 20, nu = 0.3, ONE element through the
thickness and 10 along: tip deflection within 1 percent of P L^3/(3 E I) (1 + 3 E I/(kappa G A L^2)). Thick cylinder b/a = 2 in
plane strain on a curved mesh: bore displacement within 0.2 percent and bore hoop stress within 1 percent of Lame. Manufactured
solution of F-378: observed L2 order >= 2.7. Mass: total mass and inertia tensor of a box within 1e-12; first axial frequency of a
fixed-free bar (nu = 0) within 0.1 percent at 8 elements. The upgrade has (nodes + edges) nodes.

### Reference to check against

Closed forms of ADRs 0275, 0280 and 0284 (Timoshenko cantilever, Lame, bar frequency); exact monomial integrals over the cube; the
consistent load of a quadratic serendipity face (derivable: integral of the corner and mid-side functions over the square).
Method: Zienkiewicz and Taylor, The Finite Element Method, vol. 1 (serendipity family; from memory, the functions are checked by
the partition and derivative tests).

### Files

`crates/rcfd-fem/src/hex20.rs` (the element, its rule, its face functions and the upgrade builder),
`crates/rcfd-fem/tests/si_hex20.rs`.

### What could make it miss

The negative corner load of a quadratic face surprises a reader and any code that assumes positive nodal weights. Node order
against VTK type 25 (the mid-edge order; to be confirmed from the public VTK format description). Nodal stress extrapolation from
27 points is not unique: the package registers its rule.

## Addendum 1: results (2026-11-13)

Runs, each once, release profile: `si_hex20` at commit d1ef9d74 (`cargo test -p rcfd-fem --release --test si_hex20 --
--nocapture --test-threads=1`); after amendment 1 (commit abcc7d61) the three amendment tests once; then the existing tests of
rcfd-fem, rcfd-post, rcfd-io (`--no-fail-fast`), `baseline_hashes`, fmt, clippy; then the mutations on a `git archive` copy of
6b886605 in `w49i-work/f389-scratch/mut` with its own target directory.

| Id | Limit | Measured | Met |
| --- | --- | --- | --- |
| E0 | node order (VTK 25), faces as D1 | all as stated; `PENDING = None` | yes |
| E1 | reference sums <= 1e-14 | `sum N - 1` 4.4e-16, `sum dN` 8.9e-16 | yes |
| E1b | distorted sums <= 1e-14 | 4.4e-16, `sum grad N` 1.2e-15 | yes |
| E1c | Kronecker <= 1e-15 | 0 | yes |
| E2 | CD <= 1e-7 | 2.6e-11 | yes |
| E2b | `J` against CD <= 1e-7 | 4.3e-11 | yes |
| E3 | 216 monomials <= 1e-13 | 1.8e-15 | yes |
| E4 | symmetry <= 1e-13 (21 elements) | 8.2e-17 reference, all 20 distorted met | yes |
| E5 | rank 54 (21 elements) | 54 on all 21 (reference: null eigenvalues <= 9.9e-17, smallest non-zero 1.19e-2 of lambda_max) | yes |
| E6 | rigid motions <= 1e-11 | 5.4e-16 reference, all 20 distorted met | yes |
| E7 | volume <= 1e-13 | reference 0, distorted 6.7e-16 | yes |
| E8 | affine mass <= 1e-13 | 7.3e-16 | yes |
| P1 | patch <= 1e-10 | 4.4e-15 (208 nodes, 44 interior, 132 unknowns) | yes |
| P2 | strain, nodal stress <= 1e-10 | 9.6e-14, 3.5e-14 | yes |
| L1 | -1/12, 1/3 of p A <= 1e-13; area vectors outward | box 1.1e-16, affine 1.6e-16; outward on all 12 faces | yes |
| L2 | uniaxial <= 1e-10 | elongation 5.0e-14, lateral 9.0e-13, 1.6e-12 | yes |
| L3 | model pressure on a Hex20 face refused by name | "a face of 8 nodes (faces of 3, 4 or 6 nodes are supported)" | yes |
| C1 | `abs(delta / delta_T - 1) <= 0.01` | first run: defective measure (amendment 1); amended run: **1.805e-2** (ratio 0.981949) | **no** (known miss) |
| T1a | bore `u_r` <= 0.002 | 7.3e-5 | yes |
| T1b | bore hoop stress <= 0.01 | **1.168e-2** | **no** (known miss) |
| T1c | bore mid-edge nodes on the circle <= 1e-15 | 1.1e-16 | yes |
| M1 | L2 order >= 2.7 | D 3.049, T 3.053 | yes |
| M2 | energy order >= 1.8, monotone | D 2.024, T 2.012; monotone | yes |
| B1 | mass, centre, inertia <= 1e-12 | 2.9e-15, 2.1e-15, 1.0e-15 | yes |
| B2 | `abs(f_1/f - 1) <= 1e-3`, `>= -1e-12`, mode 0 axial | 1.03e-6 (1261.887 Hz), mode 0 axial | yes |
| U1 | 111 = 36 + 75 nodes, midpoints, sets, refusal | 111 = 36 + 75, 75 mids, midpoints bit-equal, sets 21/21/29/29/29/29, Tet4 refused naming block 0 | yes |
| S1 | other tests of rcfd-fem, rcfd-post, rcfd-io unedited, 0 failed except ADR 0291's Hex20 refusals | 77 test binaries pass (481 passed, 61 ignored), including `si_scaffold_bits` and `si_scaffold_output`; `si_scaffold` fails in 4 tests, all because Hex20 is no longer refused (below); `git diff ca2c6b17 -- crates/*/tests` lists only `si_hex20.rs` | yes (as registered) |
| G1 | `baseline_hashes` passes | 3 passed, 1 ignored (its existing record); file unchanged | yes |

Negative controls: N1 cubic field 0.2226 (> 1e-6); N2 (amended) Hex8 ratio 0.369197 (< 0.9; the first run's -0.369 was vacuous,
amendment 1); N3 lambda reversed: orders 0.001 and -0.003 (< 0.5); N4 rank with `gauss_2` 48 (< 54). All hold.

Mutations (each alone, first run; all caught):

| Id | Caught by |
| --- | --- |
| M1 corner `s - 1.9` | E1 (0.1), E1b, E1c; also B1, B2, M1 |
| M2 corner derivative `- 0.9` | E2, E2b; also L2, B1, B2, M1, T1a |
| M3 stiffness `gauss_2` | E5 (rank 48 on all 21 elements); also P1, T1a |
| M4 face 1 node 17 for 13 | E0, L1; also M1 case T |
| M5 mid-edge node at `p_a` | U1 (the upgrade is refused by `add_block` with `InvertedElement` before the position check is reached), and every test using the upgrade |
| M6 `volume` with `gauss_2` | E7 |
| M7 mass `gauss_2` | E8 only (the mass properties stay exact with 8 points, as D3 says) |
| M8 node sets not extended | U1; also P1, P2, M1, M2, B2, T1a |
| M9 face mid-edge `/ 4` | L1; also L2, M1 case T, T1a |

The mutated file was restored and compared byte for byte. fmt: `cargo fmt -p rcfd-fem --check` clean (rustfmt changed the
layout of the two new files once, commit 6b886605, no change of tokens). clippy: `cargo clippy -p rcfd-fem --all-targets --release
-- -D warnings` clean.

### The existing tests that stop applying (remaining step for the maintainer)

`crates/rcfd-fem/tests/si_scaffold.rs` (ADR 0291), not edited:

- `x1_the_scaffold_exists`: `assert_eq!(rcfd_fem::hex20::PENDING, Some("ADR 0292"))` (line 80) fails with `None`; the later
  `assert_eq!(ElementKind::Hex20.pending(), Some("ADR 0292"))` (line 92) would fail too.
- `x3_placeholder_elements_refuse`: the Hex20 row (`(ElementKind::Hex20, "ADR 0292")`, line 170): `stiffness` is no longer
  `NotYetSupported` (on the test's degenerate coordinates it is now `InvalidValue { key: "jacobian", .. }`); the Beam2 and Shell4
  rows are not reached after the panic.
- `r1_to_r9_static_solve_refuses_each_new_variant`: R1 (`hex20_model()`, lines 309 to 329, the case at line 374) now solves (zero
  load, zero displacement) instead of refusing; R2 to R9 are not reached after the panic.
- `r10_refused_on_every_static_path`: its first case, R1 through CG, solves; the rest is not reached.

Options: (a) mark the four tests `#[ignore = "record of ADR 0291: Hex20 refused until ADR 0292"]` and add copies without the
Hex20 rows, so that the Beam2, Shell4, formulation, constraint and support refusals stay checked; (b) drop the Hex20 rows in place.
Recommendation (a) (house rule 2 keeps records; R2 to R9 and the Beam2 and Shell4 rows of X3 are still needed). Not taken (it
edits existing tests): it needs the maintainer.

### Diagnosis of the two misses

Unregistered diagnostics, each run once after the first run and before amendment 1, in a temporary test file (deleted, not
committed); they change no limit, mesh or parameter.

C1, ratio `delta / delta_T` of the clamped cantilever by mesh (elements along x, and through thickness = width) and `nu`: 10 x 1,
nu 0.3: 0.981949; 10 x 1, nu 0: 0.999037; 20 x 1: 0.990098; 40 x 1: 0.993462; 10 x 2: 0.986600; 20 x 2: 0.992485; 40 x 4:
0.995265; 40 x 4, nu 0: 0.999995. With `nu = 0` the registered mesh is within 0.1 percent: the element bends correctly with one
element through the thickness and does not lock (Hex8 on the same mesh: 0.369). The 1.8 percent is the full clamp with `nu =
0.3`: it suppresses the Poisson contraction at the root (registration item iii), a real 3-D effect of about 0.5 percent at `L/h =
20` (40 x 4) that the coarse mesh spreads over the first element (length 2h) and so overstates. Arithmetic 4 underestimated it on
the coarse mesh; the closed form of the acceptance contains no clamp effect.

T1b, largest relative error of the bore hoop stress by mesh (radial x circumferential): 4 x 8: 1.168e-2 (registered); 4 x 16:
1.152e-2; 8 x 8: 3.75e-3; 8 x 16: 3.67e-3; 16 x 32: 1.03e-3; the largest error at the Gauss points themselves: 7.1e-3 (4 x 16),
2.0e-3 (8 x 8), 5.4e-4 (16 x 32). The error is uniform around the bore (mean 1.14e-2 at 4 x 16) and set by the radial resolution,
at about second order; the circumferential resolution does not matter. Arithmetic 5 counted only the extrapolation of exact Gauss
values (0.14 percent) and missed the finite-element stress error at the Gauss points in the steep `B / r^2` field (0.7 percent at
`h = 0.25`), which the extrapolation carries to the bore.

### Deviations

- Amendment 1 (sign of the cantilever measure; the first-run magnitudes were known before the corrected run, disclosed there).
- B1 uses the inertia about the centre in the form `m (l_j^2 + l_k^2) / 12` (diagonal, zero off it) instead of the Gram matrix of
  `sb_modal.rs`; the same closed form.
- E4 to E6 print one line per element (21 each); the table gives the reference values and "all met".

### Decision by the registered rule

**Partial**: C1 (1.805e-2 against 1e-2) and T1b (1.168e-2 against 1e-2) miss, both diagnosed (the full clamp with `nu = 0.3` on a
coarse mesh; the radial resolution of the bore stress); every other check met, every negative control held, every mutation caught;
the only failing existing tests are the ADR 0291 refusals of Hex20 listed above.

### Remaining questions

1. The model path of Hex20 face loads (open question 1): (a) a follow-up adds `8 =>` arms to `loads::face_shape_functions` and
   `loads::face_rule` (delegating to `hex20`) and to `SolidMesh::face_tangents` (which panics with `unreachable!` for an 8-node
   face through `face_area` / `face_area_vector`); every other face count unchanged bit for bit; (b) leave it. Recommendation (a),
   soon: a public function panics on a valid Hex20 mesh.
2. C1: (a) a follow-up registers the cantilever with the support of beam theory (`u_x = 0` on the root face, `u_y`, `u_z` fixed on
   its neutral line), the full clamp recorded beside it; (b) keep the full clamp and accept 2 percent at one element.
   Recommendation (a): the acceptance's closed form assumes the beam-theory support.
3. T1b: (a) a follow-up registers the bore hoop stress on a radially refined mesh, its element count derived from the
   second-order rate of the diagnostic above (disclosed); (b) keep 4 radial elements. Recommendation (a).
4. The ADR 0291 refusal tests: option (a) of the section above. Needs the maintainer.

### For the lead: roadmap entries

F-389: proposed status **partial**. Note:

> ADR 0292, 2026-11-12: Hex20 serendipity element in rcfd-fem (hex20.rs; node order confirmed as VTK type 25 from the published vtkQuadraticHexahedron definition; 3 x 3 x 3 rule for stiffness and mass; triquadratic recovery; 8-node face loads as per-node forces; upgrade_to_hex20 and upgrade_to_hex20_mapped). Met: partition 4.4e-16, derivatives against central differences 2.6e-11, monomials 1.8e-15, symmetry, rank 54 and rigid motions on 21 elements, patch 4.4e-15, face loads -1/12 and 1/3 of p A to 1.6e-16, uniaxial 1.6e-12, bore displacement 7.3e-5, manufactured solution L2 orders 3.05 (cases D and T), box mass and inertia 3e-15, bar frequency 1.03e-6 at 8 elements, 111 = 36 + 75 nodes; nine mutations caught. Missed (known misses, diagnosed): cantilever 1.8 percent (ratio 0.981949; the full clamp with nu = 0.3, 0.999 with nu = 0) and bore hoop stress 1.17 percent on 4 x 8 elements (radial resolution). Model-path pressure and traction on Hex20 faces stay refused by loads.rs, and SolidMesh::face_area panics on an 8-node face (follow-up). si_scaffold X1, X3, R1, R10 of ADR 0291 fail because Hex20 is no longer refused: needs the maintainer.

F-388: note addition:

> ADR 0292, 2026-11-12: with Hex20 landed, si_scaffold x1_the_scaffold_exists, x3_placeholder_elements_refuse, r1_to_r9_static_solve_refuses_each_new_variant and r10_refused_on_every_static_path fail on their Hex20 rows; recommended: keep them as ignored records of ADR 0291 and add copies without the Hex20 rows (needs the maintainer).

Follow-up items:

- Title: Hex20 face loads through the model path. Deps: F-389.

> Every limit not stated here is registered in the ADR before the run. Load::Traction and Load::Pressure on surface sets of Hex20 faces give through nodal_load_vector the per-node forces of hex20::pressure_loads and hex20::traction_loads bit for bit; SolidMesh::face_area and face_area_vector of an 8-node face equal hex20::face_area and the 3 x 3 Gauss area vector within 1e-15 relative (no panic); every load vector and area of Tri3, Quad4 and Tri6 faces is bit-identical to before; the baseline hashes are unchanged.

- Title: Hex20 cantilever with the beam-theory support and the bore stress on a refined mesh. Deps: F-389.

> Every limit not stated here is registered in the ADR before the run. Cantilever L/h = 20, nu = 0.3, one Hex20 through the thickness and 10 along, with the support of beam theory at the root (u_x = 0 on the root face, u_y and u_z fixed on its neutral line), registered before the run: tip deflection within 1 percent of P L^3/(3 E I) (1 + 3 E I/(kappa G A L^2)), the full clamp recorded beside it. Thick cylinder b/a = 2, plane strain, Hex20 with the radial element count derived in the registration from the second-order rate of ADR 0292's diagnostic: bore hoop stress within 1 percent and bore displacement within 0.2 percent of Lame.
