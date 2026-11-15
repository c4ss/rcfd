# ADR 0547: F-429 registration: fracture parameters, the J-integral by the domain form, the stress intensity factor and the crack-growth life

Status: **REGISTERED 2026-11-15, written before any code of F-429 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-429 was written, compiled or run: the files under "What was read" were read, and the
arithmetic below was done by hand and with a plain Python calculator (closed forms and element counts only, no rcfd code, no
finite-element run; disclosed). Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-429 (deps F-408, F-374, F-376, F-389), work package SA-22 of the advanced tier, stub
[ADR 0332](0332-fracture-parameters-domain-integral.md) (plan [ADR 0310](0310-structural-solver-advanced-tier-plan.md)). Number
0547 given by the lead. Branch `f429-work` from f69eb128, worktree `w49i-work/f429`, scratch `w49i-work/f429-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number); each registered run made once; defaults bit-identical; no
edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code;
clean-room. The brief asks for the F-429 entry of `docs/ROADMAP.md` and `docs/roadmap.json` to be updated by this branch in a
separate last commit (the svgs not regenerated). Areas of other agents not touched: `nl_transient` (F-611), the Shell3 paths
(F-541), the Beam2 warping unknown (F-598). Every choice below that the maintainer would normally be asked about is taken on the
maintainer's standing instruction of 2026-11-11 (take the recommendation).

## What F-429 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. Centre crack of half-length a in a plate of half-width W = 10 a under tension, plane strain (one layer, held in z), Hex20: K_I from J within 2 percent of sigma sqrt(pi a) sqrt(sec(pi a/(2 W))), the values of rings 2 to 4 within 1 percent of each other, and J equal to the change of potential energy between two models with half-lengths a +/- 0.02 a within 2 percent. K_I from the opening displacement within 5 percent. Penny-shaped crack of radius a in a cylinder of radius 10 a under tension (heavy record): K_I = 2 sigma sqrt(a/pi) within 3 percent, uniform along the front within 2 percent. Under pure shear loading of the centre crack the mode-I factor from the opening is zero within 1e-8 of the tension value. Growth: the cycles from a0 to a1 by the Paris law with K = d sigma sqrt(pi a) equal the closed-form integral for m = 3 and the logarithm for m = 2 within 1e-6. A front that is not straight within the registered tolerance and a ring that reaches the boundary are refused.

Clauses: **c0** every limit registered here; **c1** centre crack, K_I from J within 2 % of the secant form; **c2** rings 2 to 4
within 1 % of each other; **c3** J against the potential-energy difference of `a +/- 0.02 a` within 2 %; **c4** K_I from the
opening within 5 %; **c5** penny crack K_I within 3 % of `2 sigma sqrt(a/pi)`; **c6** uniform along its front within 2 %; **c7**
pure shear: the opening K_I zero within `1e-8` of the tension value; **c8** Paris cycles for `m = 3` and `m = 2` within `1e-6`;
**c9** a front not straight within the registered tolerance refused; **c10** a ring reaching the boundary refused.

The brief adds: register the domain-integral form (the equivalent domain integral of Shih, Moran and Nakamura with a
q-function, rings by element layers around the front), the crack-tip mesh (quarter-point or collapsed Hex20 or not, and how), the
J to K_I conversion for plane strain, the displacement-extrapolation formula, the applicability of the secant finite-width
formula, the penny-crack mesh, the Paris closed forms; sources in `docs/reference-data.md`, uncertain citations flagged; a new
opt-in module, defaults bit-identical, `baseline_hashes` unchanged, no case-file key; mutations the checks catch; every aggregated
test error finite.

## What was read (no run)

`AGENT_RULES.md`; the brief; ADR 0332 (the stub) and ADR 0310 section SA-22; ADR 0543 and ADR 0541 (form); the code at f69eb128:
`crates/rcfd-fem/src/fracture.rs` (the 4-line stub of ADR 0440, `PENDING = Some("ADR 0332 (F-429)")`), `hex20.rs` (node order of
VTK type 25, faces, the 3 x 3 x 3 rule as the stiffness rule, `traction_loads`, `upgrade_to_hex20` and `upgrade_to_hex20_mapped`
with `MidEdgeNode`), `mesh.rs` (`SolidMesh::add_block` refuses a node repeated within an element and `det J <= 0` at any node of
an element; coincident distinct nodes are not refused; `face_incidence`, `face_nodes`, `face_area_vector`, `FaceRef`,
`block_of`, `ElementBlock::formulation`), `builders.rs` (`hex_box`, the local faces `xi -1 -> 5, xi +1 -> 3, eta -1 -> 2,
eta +1 -> 4, zeta -1 -> 0, zeta +1 -> 1`), `element.rs` (`Element::integration_points` with `PointGeometry::grad` the physical
gradients and `weight = w det J`), `recover.rs` (the stress `D eps` at the points of the stiffness rule), `model.rs` (`DofMap`,
`Material::IsotropicElastic`, `Constraint::displacement`, `Load` variants, `material_of`), `solve.rs` (`StaticSolver::Direct`
envelope after reverse Cuthill-McKee, `SupernodalDirect`, `StaticOptions::refinement` of ADR 0397), `constraint::Prescribed`;
`crates/rcfd-fem/tests/si_hex20.rs` (the helpers `Checks`, `rebuild`; T1 the thick cylinder on a curved mapped mesh);
`crates/rcfd-fem/tests/sa_scaffold.rs` (the X1 rows: `fracture` is the one row still expecting `Some("ADR 0332 (F-429)")` in the
live test `x1_modules_and_pending_after_f415_f420_f422_f416_f417_f432_f423_f424_f410_f411_f428_f427_f425`). `rcfd-api`,
`rcfd-cli` and `rcfd-mcp` do not call `rcfd_fem::fracture` (grep at f69eb128): the module is called only by its own tests, so no
case result and no golden can change.

## Design

### D1. The crack front

`fracture::CrackFront` describes a front by:

* `nodes`: the mesh nodes on the front, ordered along it, an odd number `>= 3`: corner, mid-edge, corner, ... (the front edges of
  Hex20 elements are quadratic). The corner nodes at even positions `0, 2, ..., 2 N` are the **stations** `j = 0..=N`.
* `normal`: the unit normal `n` of the crack plane (normalised on input; zero refused).
* `shape`: `FrontShape::Straight { advance }` with the unit crack-advance direction `e` (in the crack plane, normal to the front,
  pointing into the ligament), or `FrontShape::Circular { centre }`, a circle in the crack plane about `centre`, with the
  advance direction at a point `x` the radial direction `e(x) = unit(d - (d . n) n)`, `d = x - centre`.
* `half_symmetric`: the model is the half on one side of the crack plane, the other half given by mirror symmetry (the ligament
  held normal to the plane); J and the opening are doubled (D3, D6).

Validation (`CrackFront::check`, called by every function of D3 and D6), all refused with `FemError::InvalidValue`, key
`"crack front"`:

* a node out of range, a node twice, an even count or fewer than 3 nodes;
* **straight front** (c9): `L = |x_last - x_first|`, unit tangent `t = (x_last - x_first)/L`; every front node at a distance from
  the line through `x_first` along `t` of at most `FRONT_TOLERANCE L`, **`FRONT_TOLERANCE = 1e-6`**; `|e . t| <= 1e-6` and
  `|e . n| <= 1e-6`; every front node within `FRONT_TOLERANCE L` of the crack plane through `x_first`; the parameters `s = (x -
  x_first) . t` strictly increasing along the list;
* **circular front**: radius `R_f` the distance of `x_first` from the axis (`centre`, `n`); every front node within
  `FRONT_TOLERANCE R_f` of the circle (radius and plane); the angles `theta` (D3) strictly increasing.

The tolerance `1e-6` is relative to the front length (straight) or radius (circular): a mesh built from exact coordinates puts the
nodes on the line or circle to about `1e-16` (rounding of the builder's arithmetic), so `1e-6` refuses only a front that is
genuinely bent (by more than a micrometre per metre) and never a rounded one. Negative controls R1b and R4b below show a
deviation of `1e-8` accepted.

### D2. Rings by element layers

`S_0` is the set of front nodes. Ring `k >= 1` is the set of solid elements that have at least one node in `S_(k-1)` and are not
in rings `1..k-1`; `S_k = S_(k-1)` united with every node of the elements of ring `k`. On a structured grid graded about the tip
this is the square annulus of elements at Chebyshev distance `k` from the tip, in a plane-strain layer the same in each layer
(nodes are touched through corners, so diagonal neighbours belong to the ring). Ring 0 is refused; a ring with no element (the
mesh exhausted) is refused.

### D3. The equivalent domain integral (c1, c2, c5, c6)

The energy release rate is computed by the equivalent domain integral (Li, Shih and Needleman 1985; Shih, Moran and Nakamura
1986; the virtual crack extension of deLorenzi 1982 in the same form; all from memory, derivation below). For a virtual
extension field `q(x)` (a vector field, `q = 0` outside the domain, `q` along the advance direction on the front), with no body
force, no thermal strain, traction-free crack faces and a homogeneous linear elastic material,

`G_bar = int_V (sigma_ij u_i,k - W delta_jk) q_k,j dV`,  `W = sigma_ij eps_ij / 2`,

and `G_bar = int_front G(s) q(s) . e(s) ds`. Derivation (2-D, then the 3-D form, first principles): Rice's
`J = int_Gamma (W n_1 - sigma_ij n_j u_i,1) ds` on a contour `Gamma` about the tip, outward normal `n`. On the annulus `A`
between `Gamma` (where `q = 1`) and an outer contour (where `q = 0`), the divergence theorem gives `int_A (F_j q)_,j dA =
-int_Gamma F_j n_j ds` with `F_j = W delta_1j - sigma_ij u_i,1` (the crack faces add nothing: `sigma_ij n_j = 0` and `n_1 = 0`
there). Equilibrium `sigma_ij,j = 0` and `W_,1 = sigma_ij u_i,j1` (a homogeneous material) make `F_j,j = 0`, so `J = int_A
(sigma_ij u_i,1 - W delta_1j) q_,j dA`. In 3-D with the vector field `q_k` the same steps give the formula above (deLorenzi's
derivative of the potential energy under the virtual motion `x -> x + q`).

Discretisation:

* **q at the nodes.** For ring `k` and station `j`: `q_a = rho_a phi_j(s_a) e(x_a)` at node `a`, `rho_a = 1` if `a` is in
  `S_(k-1)`, else 0; `phi_j` the piecewise-linear hat of station `j` in the front parameter `s` (1 at `s_j`, 0 at the neighbour
  stations `s_(j-1)`, `s_(j+1)`; at the first and last station a half hat, with `phi = 1` beyond the end, `s < s_0` or `s >
  s_N`, which only rounding produces). The parameter: straight `s_a = (x_a - x_first) . t`; circular `s_a = R_f theta_a`,
  `theta_a = atan2(d . b, d . c)` with `c = unit` radial direction of the first front node, `b = n x c`, `d = x_a - centre`.
  `q(x) = sum_a N_a(x) q_a` with the element's shape functions. The mid-edge front nodes are at the mid parameter, so on each
  front edge `q` is the linear hat exactly.
* **The integral.** Over every element of rings `1..k` (the only elements with a non-zero nodal `q`; `q` varies along the front
  inside the inner rings too), with the element's stiffness rule (3 x 3 x 3 for Hex20) at every point: the displacement gradient
  `u_i,k = sum_a u_ai N_a,k`, `eps = sym grad u`, `sigma = lambda tr(eps) I + 2 mu eps` with the element's isotropic `E, nu`,
  `W = sigma : eps / 2`, `q_k,j = sum_a q_ak N_a,j`; sum of `w det J (sigma_ij u_i,k q_k,j - W q_k,k)`.
* **Normalisation.** `J_j = G_bar / int phi_j ds`, `int phi_j ds = (s_(j+1) - s_(j-1))/2` inside, `(s_1 - s_0)/2` and
  `(s_N - s_(N-1))/2` at the ends (exact for the hat in `s`; on a circle `s` is the arc length). A half-symmetric model:
  `J_j = 2 G_bar / int phi_j ds` (the integrand of the mirror half is the mirror image for a mode-I field).

Accepted elements: solid kinds (Tet4, Hex8, Tet10, Hex20 through `element_for(kind)`), formulation `Full`, material
`IsotropicElastic`; anything else in rings `1..k` refused (key `"element"`). A model with a body load (`Gravity`, `Centrifugal`,
`BodyForceNodal`) or a `Temperature` load refused (key `"load"`): the formula has no body-force or thermal term. Crack-face
tractions given as extra nodal forces cannot be seen and are a limit.

Output `FrontValue { station, node, position, s, j, weight }` per station, `weight = int phi_j ds`.

### D4. The ring that reaches the boundary (c10)

`q` must vanish on the boundary except where the contour closes over a traction-free crack face, a mirror plane or an end face
of the front. The boundary faces are the faces of `face_incidence` met by one element. A boundary face is **allowed** if (a) all
its nodes are within `FRONT_TOLERANCE D` of the crack plane, `D` the diagonal of the mesh's bounding box (the crack faces and, in
a half-symmetric model, the ligament on the mirror plane), or (b) its unit normal `m` (from `face_area_vector`) is parallel to the
front tangent at the face centroid, `|m . tau| >= 1 - 1e-6` (`tau = t` straight; `tau = n x e(x_c)` circular; the faces of a
plane-strain layer and the mirror planes that end a quarter of a circular front). Ring `k` is **refused** if a node of `S_(k-1)`
(a node with `rho = 1`) lies on a boundary face that is not allowed (key `"ring"`, naming the ring, the node and its position).

### D5. K_I from J (c1, c5)

Irwin's relation for a mode-I front in plane strain: `K_I = sqrt(J E / (1 - nu^2))` (Anderson, Fracture Mechanics, chapter 2;
from memory; derivation: the crack-closure integral of the near-tip stress and opening of D6 gives `G = K_I^2 (kappa + 1)/(8
mu)`, `kappa = 3 - 4 nu`, `mu = E/(2 (1 + nu))`, so `G = K_I^2 (1 - nu^2)/E`). `fracture::plane_strain_k(j, e, nu)` refuses a
negative or non-finite J and `E <= 0`, `nu` outside `(-1, 0.5)`. The interior of the penny front is in plane strain (the
three-dimensional field near a smooth front is the plane-strain field of the local K; the free surface does not meet it).

### D6. K_I from the opening (c4, c7)

The near-tip opening for mode I in plane strain (Westergaard's field, Anderson chapter 2, from memory; derived from the
Williams-Irwin field `u_y = K_I/(2 mu) sqrt(r/(2 pi)) sin(theta/2) (kappa + 1 - 2 cos^2(theta/2))` at `theta = +/- pi`) is
`delta(r) = 8 K_I (1 - nu^2)/E sqrt(r/(2 pi))`, so the apparent factor at a face point a distance `r` behind the front is

`K_app(r) = E delta(r) / (8 (1 - nu^2)) sqrt(2 pi / r)`.

`fracture::opening_k(model, displacement, front, pairs, r_min, r_max)`: `pairs` are `OpeningPair { upper, lower: Option }` face
nodes (upper on the `+n` side); `delta = (u_upper - u_lower) . n`, or `2 u_upper . n` with no lower node (half-symmetric);
`r` the distance of the upper node from the front (straight: the distance from the front line; circular: `|rho - R_f|`, `rho` the
distance from the axis) and the pairs with `r_min <= r <= r_max` kept; `E, nu` of the element of the first front node. The
least-squares line `K_app = K_0 + B r` over the kept pairs; returns `OpeningFit { k: K_0, slope: B, points }`. Fewer than 2
points, a pair whose nodes are not coincident within `FRONT_TOLERANCE D` and `r_min >= r_max` refused (key `"opening"`). The
extrapolation removes the first correction of the near-tip expansion: for the Westergaard crack `K_app(r)/K_I = sqrt(1 - r/(2
a))` (arithmetic 3), whose quadratic remainder over `[0.1 a, 0.4 a]` biases the intercept by `+0.19 %`.

### D7. The growth life (c8)

The Paris law (Paris and Erdogan 1963, from memory) `da/dN = C (Delta K)^m`; `fracture::paris_cycles(c, m, a0, a1, delta_k)` with
`delta_k: Fn(f64) -> f64` integrates `N = int_a0^a1 da / (C Delta K(a)^m)` numerically: the substitution `a = a0 (a1/a0)^tau`,
`da = a ln(a1/a0) dtau`, `tau in [0, 1]`, composite Gauss-Legendre with 16 points per panel (nodes and weights by Newton's
iteration on the Legendre polynomial, to `1e-15`), panels doubled from 1 until two successive sums differ by at most `1e-13`
relative (at most `2^16` panels; refused if not reached, key `"paris"`). Refused: `C <= 0`, `m <= 0`, `a0 <= 0`, `a1 <= a0`, any
non-finite argument, and a `Delta K(a)` that is not finite and positive at a point used (key `"paris"`). The closed forms that
check it (computed in the test, not in the library): with `Delta K = d Delta sigma sqrt(pi a)` and `k = C (d Delta sigma
sqrt(pi))^m`, `N = (a1^(1 - m/2) - a0^(1 - m/2)) / (k (1 - m/2))` for `m != 2`, so for `m = 3` `N = 2 (a0^(-1/2) -
a1^(-1/2))/k`, and `N = ln(a1/a0)/k` for `m = 2`.

### D8. The builders and the crack-tip mesh

**No quarter-point or collapsed elements.** `SolidMesh::add_block` refuses an element with a node repeated (a collapsed Hex20
repeats nodes) and `det J <= 0` at any node; a Hex20 with its mid-edge nodes moved to the quarter points of the edges at the tip
(Barsoum's singular element, from memory) has `det J = 0` at the tip node, so neither is accepted by the mesh validation, and
changing that validation is shared code. The crack tip is meshed with **regular Hex20 on a grid graded geometrically towards the
tip**, with a uniform zone of `n_u` element layers about the tip (`n_u = 4` for `P` and `C`, so that rings 1 to 4 are layers of equal
square elements; `n_u = 2` for the coarse penny PF). The
domain integral needs no singular element (it is accurate away from the tip; arithmetic 4); the opening fit uses points beyond
the uniform zone (D6).

The graded 1-D spacing `graded(L, h0, g)`: after the uniform zone, `n` elements of sizes `h0 r, h0 r^2, ..., h0 r^n` over the
length `L`, `n` the least integer with `h0 sum_(i=1..n) g^i >= L`, then `r` in `(0, g]` solved by bisection (to `1e-15`
relative) so that `h0 sum r^i = L` exactly.

**Centre-cracked plate** `fracture::centre_crack_plate(&PlateCrack)` (Hex8 grid then `upgrade_to_hex20`, straight edges): half
crack length `a`, half-width `W`, half-height `H`, thickness `t` (one element layer `0 <= z <= t`), `h0`, growth `g`. The
x-lines: the tips at `+/- a`, the uniform zone `n_u h0` on both sides of each tip, `graded` from `a - n_u h0` towards `x = 0` and
from `a + n_u h0` to `W`, mirrored for `x < 0`; the y-lines: uniform `n_u h0` about `y = 0`, then `graded` to `H`, mirrored. The slit:
every node on `y = 0` with `|x| < a` is doubled, the elements above using the upper copy and those below the lower copy; the tip
nodes `x = +/- a` single. `upgrade_to_hex20` gives one mid-edge node per edge, so the crack-face edges of the upper and lower
elements get separate mid-edge nodes. Returned: the mesh with the node set `all`, node sets `right_pin` (the nodes at `x = W, y =
0`) and `left_pin` (`x = -W, y = 0`), surface sets `top, bottom, left, right`; the right-tip front `[ (a,0,0), (a,0,t/2), (a,0,t)
]`; the opening pairs at `z = 0` (every upper crack-face node at `0 < x < a` with its lower copy, corners and mid-edge nodes);
the number of element columns between the right tip and `x = W` (`columns_to_edge`).

**Penny crack in a cylinder** `fracture::penny_crack_cylinder(&PennyCrack)`: the quarter `x >= 0, y >= 0` of the half `0 <= z <=
H` of a cylinder of radius `R` (mirror planes `x = 0`, `y = 0`, `z = 0`), crack radius `a`. In the plane: a core square `[0, s]^2`
of `m x m` elements (`m = n_theta/2`); `n_t` transition layers from the square's boundary (`n_theta` equal segments, two edges)
to the circle `r_t` (linear in the layer index between the boundary point and the circle point at angle `theta_l = (pi/2) l /
n_theta`); a polar region from `r_t` to `R` at the angles `theta_l` with the radii: `graded` from `a - n_u h0` inward to `r_t`
(growth `g_in`), the uniform `n_u h0` on both sides of `a`, `graded` from `a + n_u h0` to `R` (growth `g_out`). In z: uniform `n_u h0`
above `z = 0`, `graded` to `H` (growth `g_out`). Hex8 then `upgrade_to_hex20`, then every mid-edge node whose two ends are at the
same distance `rho >= r_t` from the axis (relative `1e-12`) moved to that distance along its own direction (the arc mid-point:
the circumferential edges of the polar region curved, exact on the circles). Returned: the mesh with node sets `all`, `ligament`
(`z = 0`, `rho >= a (1 - 1e-12)`, the front included), `x0` (`x = 0`), `y0` (`y = 0`), surface set `top` (`z = H`); the front
(the `2 n_theta + 1` nodes at `z = 0`, `rho = a`, ordered by angle), the front centre `0`, normal `z`, half-symmetric.

### D9. Opt-in, shared code, the scaffold row

Everything is in `crates/rcfd-fem/src/fracture.rs` (filled; `PENDING = None`), called only by its tests; no other library file
changes, so no default can change and no default-bits record is needed (the brief asks one only if shared code is touched).
`baseline_hashes` is run anyway. `sa_scaffold.rs`: the live X1 test expects `fracture::PENDING == Some("ADR 0332 (F-429)")`; it
gets an `#[ignore = "record: ADR 0547 (F-429) landed fracture ..."]` attribute with its body unchanged and a continued X1 test
with `fracture` landed (the pattern of ADR 0291 that every earlier landing followed). No case-file key: the case file has no
fracture analysis; adding one is an open question (below).

## Arithmetic done beforehand (disclosed)

1. **The secant form and its applicability.** `a/W = 0.1`: `sqrt(sec(pi/20)) = 1.0062133`. Feddersen's secant form is an
   approximation of the centre crack in a strip of infinite height; Tada's refinement `(1 - 0.025 l^2 + 0.06 l^4) sqrt(sec(pi
   l/2))` (Tada, Paris and Irwin, from memory, stated there to be within 0.1 % of Isida's series) gives `1.0059677`, `0.024 %`
   below. The finite height `H = 2 W` of the model adds an effect I estimate below `0.1 %` (from memory of the decay of the
   height correction of a centre crack with `H/W >= 1`; not a reference). So the reference is good to about `0.15 %` against the
   limit of 2 %. With `a = 0.01 m`, `sigma = 1e8 Pa`, `E = 2e11 Pa`, `nu = 0.3`: `K_ref = 1.78347e7 Pa m^0.5`,
   `J_ref = K_ref^2 (1 - nu^2)/E = 1447.24 J/m^2`.
2. **Element counts.** Plate `h0 = a/40`, `g = 1.4`: towards `x = 0` `L = 0.9 a = 36 h0`, `n = 8`; to `W` `L = 8.9 a`, `n = 14`; in
   y `L = 19.9 a`, `n = 17`. So `2 (8 + 4 + 4 + 14) = 60` columns and `2 (4 + 17) = 42` rows: 2520 Hex20, about 18 300 nodes, about
   36 600 free unknowns (`z` held). `columns_to_edge = 4 + 14 = 18`: ring 19 is the first whose `S_18` reaches `x = W` (c10),
   the top `y = H` is 21 rows away. Penny `h0 = a/32`, `g_in = 1.25`, `g_out = 1.4`, `n_theta = 12`, `s = 0.25 a`, `n_t = 2`,
   `r_t = 0.5 a`: inward `L = 0.375 a`, `n = 6`; outward `L = 8.875 a`, `n = 14`; z `L = 9.875 a`, `n = 14`; so 28 polar layers,
   18 z layers, `36 + 24 + 336 = 396` elements per layer, 7128 Hex20, about 32 000 nodes, about 96 000 unknowns (heavy record).
   Coarse penny (PF, the fast test of the circular path): `h0 = a/8`, a uniform zone of 2 layers, `n_theta = 4`, `g_in = 1.5`,
   `g_out = 1.6`, `r_t = 0.5 a`: inward `L = 0.25 a`, `n = 2`; outward `L = 8.75 a`, `n = 8`; z `L = 9.75 a`, `n = 8`.
3. **The opening fit.** For the Westergaard crack the face opening is `delta = 4 sigma (1 - nu^2)/E sqrt(a^2 - x^2)`, so with `r
   = a - x`, `K_app(r) = sigma sqrt(pi a) sqrt(1 - r/(2 a))`. A least-squares line through 13 equally spaced points of `[0.1 a,
   0.4 a]` has the intercept `1.0019` times `sigma sqrt(pi a)` (Python, this formula only): a bias of `+0.19 %`. The finite width
   changes the level (`F = 1.006`) and little the shape. With `g = 1.4` the face nodes (corners and mid-edge nodes) in `[0.1 a,
   0.4 a]` are about 9.
4. **Domain integral accuracy.** Regular quadratic elements of size `h0 = a/40` near the tip; the error of the domain integral
   comes from the poorly represented field in ring 1, which rings 2 to 4 avoid except through the `q` taper of the inner
   elements (zero in plane strain, below). I estimate the J of rings 2 to 4 within `0.5 %` of the exact value of the mesh's
   geometry and their spread below `0.3 %`; K is half as sensitive. In the one-layer plate, `u_z = 0` and the loads are uniform in
   z, so the discrete solution is independent of z (the z-independent extension of the plane-strain solution satisfies every
   discrete equation: the terms with z-derivatives of the test functions carry `sigma_xz = sigma_yz = 0`), the terms of the
   integrand with `q_,z` vanish (`sigma_iz u_i,1 = 0`), and the two half-hat stations give the same J to rounding.
5. **The energy difference.** `Pi = -f . u / 2` at equilibrium (zero prescribed values); a centre crack grows at both tips, the
   new area `2 t da`, so `J_E = d(f . u)/da / (4 t)`, by the central difference `J_E = (f.u(a+) - f.u(a-))/(4 t 0.04 a)` (the
   difference error is `O(0.02^2)` of the second derivative, about `4e-4` relative). The meshes of `a +/- 0.02 a` have the same
   uniform zone about each tip, so their near-tip discretisation errors are the same and cancel; the change of `f . u` is
   `2 pi a 0.04 a sigma^2 t/E' = 0.25 sigma^2 a^2 t/E'` against the plate's `200 sigma^2 a^2 t/E'`, a relative difference of
   `1.3e-3`, far above the rounding of the sums (`1e-16` relative). Estimate `0.3 %`.
6. **Shear.** The mesh, its slit and the constraints are mirror-symmetric about `y = 0`, the shear tractions antisymmetric: the
   exact discrete solution is antisymmetric, `u_y(x, -y) = u_y(x, y)` and `u_x(x, -y) = -u_x(x, y)`, so the opening is zero
   except for rounding. The constraints (`u_x = u_y = 0` on `right_pin`, `u_y = 0` on `left_pin`, `u_z = 0` everywhere) remove
   the rigid motions and are consistent with both the symmetric (tension) and the antisymmetric (shear) classes: on `y = 0`
   the antisymmetric `u_x` vanishes and a rigid rotation belongs to the antisymmetric class. Rounding: with two steps of
   iterative refinement the solution is accurate to about `1e-15` of `|u|`; `|u| ~ (tau/G) W = 1.3e-4 m`, the tension opening at
   `r = 0.1 a` about `8e-6 m`, so the shear K_app is about `2e-14` of the tension value (limit `1e-8`).
7. **Penny.** `K = 2 sigma sqrt(a/pi) = 1.12838e7 Pa m^0.5` (Sneddon 1946, from memory; Tada's handbook). The finite cylinder
   `R = 10 a`, `H = 10 a`: the far field of a penny crack decays as `r^-3`, so I estimate the effect of the boundaries at about
   `(a/R)^3 = 1e-3` (estimate, not a reference). The domain integral in 3-D with elements of `h0 = a/32` radially and `a pi/24 =
   0.13 a` along the front: estimate within `1 %`, spread along the front within `0.5 %` (the core square breaks the rotational
   symmetry only far from the front).
8. **Paris.** `C = 1e-11`, `Delta sigma = 100`, `d = 1.12`, `a0 = 1e-3`, `a1 = 1e-2` (units consistent; the integrator is unit
   free): `N(m = 3) = 552 793.06`, `N(m = 2) = 5 842 917.7`. The 16-point rule on `tau` integrates `a^(1 - m/2) = exp((1 - m/2)
   tau ln 10)` (an entire function, `|(1 - m/2) ln 10| <= 1.2`) to about `1e-16` on one panel.
9. (arithmetic for G3) With `a = a1 x^2`, `da = 2 a1 x dx`, `a^1.5 = a1^1.5 x^3`, `(1 + a/a1)^3 = (1 + x^2)^3`: `N = (2/(k
   sqrt(a1))) int x^-2 (1 + x^2)^-3 dx`. Partial fractions: `1/(x^2 (1 + x^2)^3) = 1/x^2 - 1/(1 + x^2) - 1/(1 + x^2)^2 - 1/(1 +
   x^2)^3`, with the antiderivatives `-1/x`, `atan x`, `x/(2 (1 + x^2)) + atan(x)/2`, `x/(4 (1 + x^2)^2) + 3 x/(8 (1 + x^2)) + 3
   atan(x)/8`. Checked at `x = 0.7` (Python, this formula only, disclosed): the identity holds.
   The interval of R3's `opening_k` row, `[0.402 a, 0.403 a]`: with `g = 1.4` the graded spacing of the crack-face side has
   `n = 8`, `r = 1.3336`, the face points (corners and mid-edge nodes) at `0.3689 a` and `0.4216 a` about it (Python,
   this spacing formula only), so no node falls in it. The fit interval `[0.1 a, 0.4 a]` of P4 holds 10 points.

## Checks

Common: plate `P` = `centre_crack_plate` with `a = 0.01`, `W = 0.1`, `H = 0.2`, `t = 5e-4` (`0.05 a`), `h0 = a/40`, `g = 1.4`;
`E = 2e11`, `nu = 0.3`; `u_z = 0` on `all`, `u_x = u_y = 0` on `right_pin`, `u_y = 0` on `left_pin`; the solver
`StaticSolver::Direct` with `refinement(2)` (ADR 0397). Tension: `hex20::traction_loads` of `(0, sigma, 0)` on `top` and `(0,
-sigma, 0)` on `bottom`, `sigma = 1e8`. Shear: `(tau, 0, 0)` on `top`, `(-tau, 0, 0)` on `bottom`, `(0, tau, 0)` on `right`,
`(0, -tau, 0)` on `left`, `tau = 1e8`. The right-tip front, straight, `n = (0, 1, 0)`, `e = (1, 0, 0)`, two stations. `K_ref`
as arithmetic 1. Penny `C` = `penny_crack_cylinder` with `a = 0.01`, `R = 0.1`, `H = 0.1`, the parameters of arithmetic 2;
`u_z = 0` on `ligament`, `u_x = 0` on `x0`, `u_y = 0` on `y0`, `(0, 0, sigma)` on `top`; `StaticSolver::SupernodalDirect`.
Every aggregated error is asserted finite (F1) before its limit is tested. Relative errors `|v/v_ref - 1|`.

| check | clause | set-up | limit | estimate |
|---|---|---|---|---|
| P1 | c1 | `P` tension: `plane_strain_k(J)` of rings 2, 3, 4 at both stations against `K_ref`: the largest error | `<= 0.02` | 0.003 |
| P2 | c2 | `P` tension: for each station, `(max - min)/min` of the J of rings 2, 3, 4; the larger | `<= 0.01` | 0.003 |
| P3 | c3 | `J_E` of arithmetic 5 from tension models with half-lengths `0.98 a` and `1.02 a` (same parameters) against the mean over the stations of the ring-3 J of `P` | `<= 0.02` | 0.003 |
| P4 | c4 | `P` tension: `opening_k` with the builder's pairs, `[0.1 a, 0.4 a]`, against `K_ref` | `<= 0.05` | 0.01 |
| P5 | c7 | `P` shear: `|K_0|` of `opening_k` (as P4) divided by the `K_0` of P4 | `<= 1e-8` | 1e-13 |
| P6 | D3 | `P` tension: the two stations' ring-3 J: `|J_0 - J_1|/J_0` | `<= 1e-10` | 1e-13 |
| PH1 | c5 | `C` (heavy record, `#[ignore]`, run once alone): `plane_strain_k` of the ring-3 J at every station against `2 sigma sqrt(a/pi)`: the largest error | `<= 0.03` | 0.01 |
| PH2 | c6 | `C`: `(max - min)/mean` of the ring-3 K over the 13 stations | `<= 0.02` | 0.005 |
| PH3 | (record) | `C`: rings 2 and 4 printed, the largest error of each against `2 sigma sqrt(a/pi)` | printed, no limit | |
| PF | D3 circular, fast | coarse penny: `h0 = a/8`, uniform zone of 2 layers about the front (radially and in z) instead of 4, `n_theta = 4`, `s = 0.25 a`, `n_t = 1`, `r_t = 0.5 a`, `g_in = 1.5`, `g_out = 1.6`, `StaticSolver::Direct`; ring 2 K at every station against `2 sigma sqrt(a/pi)`: the largest error | `<= 0.10` | 0.04 |
| G1 | c8 | `paris_cycles` with `Delta K = d Delta sigma sqrt(pi a)`, `m = 3`, the values of arithmetic 8, against the closed form | `<= 1e-6` | 1e-15 |
| G2 | c8 | same, `m = 2`, against `ln(a1/a0)/k` | `<= 1e-6` | 1e-15 |
| G3 | D7 | `m = 3` with `Delta K = d Delta sigma sqrt(pi a) (1 + a/a1)` against the closed form of `int_a0^a1 da/(k a^1.5 (1 + a/a1)^3)` by the substitution `a = a1 x^2`: `2/(k sqrt(a1)) int_(sqrt(a0/a1))^1 x^-2 (1 + x^2)^-3 dx`, computed in the test by the partial fractions of arithmetic 9 | `<= 1e-6` | 1e-14 |
| R1a | c9 | `P` with the middle front node moved by `1e-4 t` in `x` (mesh rebuilt; zero displacement): `j_integral` refused, key `"crack front"`, message containing "not straight" | refused | refused |
| R1b | c9 control | same, moved by `1e-8 t`: accepted (finite J) | accepted | accepted |
| R2a | c10 | `P` tension: ring `columns_to_edge + 1` (19) refused, key `"ring"`, message containing "reaches the boundary" | refused | refused |
| R2b | c10 control | `P` tension: ring `columns_to_edge` (18) accepted, J finite | accepted | accepted |
| R3 | D1, D2, D6, D7 | refusals: ring 0; a front of 2 nodes; a front node twice; `advance` not normal to the front (`(1, 0, 1)/sqrt 2`); `opening_k` with `[0.402 a, 0.403 a]` (no face node of `P` in it: fewer than 2 points) and with `r_min >= r_max`; `paris_cycles` with `a1 <= a0`, `C <= 0`, `m <= 0`, `Delta K` returning 0; `plane_strain_k` of a negative J; a model with a `Gravity` load | refused (each) | refused |
| R4a | D1 | the coarse penny front given as a straight front: refused, "not straight" | refused | refused |
| R4b | D1 | the coarse penny front with the centre moved by `1e-4 a` in `x`: refused, "circle"; moved by `1e-8 a`: accepted | as stated | as stated |
| N1 | control | `P` tension, ring 3: the J with `advance = (-1, 0, 0)` equals `-J` within `1e-12` relative (q enters linearly) | `<= 1e-12` | 1e-15 |
| N2 | control | `P` tension: the ring-1 J printed against the ring-3 J (shows that ring 1 differs, so P2 is not vacuous) | printed, no limit | |
| F1 | brief | every aggregated error of the rows above finite | finite | finite |

## Negative controls and mutations

Controls are R1b, R2b, R4b (the refusals refuse no more than they should), N1 (q enters linearly with its sign), N2 (rings
differ near the tip). Mutations, each run once on a scratch copy (`f429-scratch/mut`, `CARGO_TARGET_DIR`
`w49i-work/target-f429-mut`), the fast tests of `sa_fracture.rs` run (the heavy PH rows are not run for mutations):

| mutation | change | expected to be caught by |
|---|---|---|
| M1 | the term `- W q_k,k` dropped | P1, P3 |
| M2 | the gradient transposed (`u_k,i` for `u_i,k`) | P1, P3 |
| M3 | the factor 2 of a half-symmetric model dropped | PF |
| M4 | `K = sqrt(J E)` (plane stress) | P1, PF |
| M5 | `K_app` with `sqrt(pi/r)` for `sqrt(2 pi/r)` | P4 |
| M6 | the opening from the `x` component instead of `n` | P4 (and P5) |
| M7 | the normalisation `int phi ds` replaced by the front length | P1, P6 or P3 |
| M8 | `rho` from `S_k` instead of `S_(k-1)` (one ring outward) | R2b |
| M9 | the boundary test of D4 disabled | R2a |
| M10 | the straightness test of D1 disabled | R1a, R4a |
| M11 | the Jacobian `a ln(a1/a0)` of the substitution replaced by `a` | G1, G2, G3 |
| M12 | the elements of rings `1..k-1` left out of the integral (only ring `k`) | PF (the taper along a curved front) |

## Decision rule

* **done**: P1 to P6, PH1, PH2, PF, G1 to G3, R1 to R4, N1, F1 met, every mutation caught by at least one check, fmt and clippy
  clean, the crate's tests and `baseline_hashes` pass.
* **partial**: the module built and the fast checks run, but a check of c1 to c10 missed (a `*_known_miss` record) or not run
  (PH not run counts here), or another registered check missed, or a mutation not caught.
* **todo**: the domain integral or the builders not built.

## Limits (not covered)

* Linear elastic, isotropic, homogeneous material in the domain; no body force, no thermal strain (refused), no crack-face
  traction (not detectable when given as extra forces: the user's responsibility).
* J is the energy release rate of the advance direction (mode mixity is not separated: under mixed loading J is `(K_I^2 +
  K_II^2)(1 - nu^2)/E + K_III^2/(2 mu)` and the opening gives K_I only). No interaction integral.
* Straight and circular fronts only; no other curved front, no crack growth with remeshing, no elastic-plastic J.
* No singular (quarter-point) elements: the mesh validation refuses them (D8).
* The Paris law without a threshold, a closure or a critical K; `C`, `m` and toughness are inputs.
* No case-file key.

## Addendum 1: results (2026-11-15)

Code: `crates/rcfd-fem/src/fracture.rs` (the only library file changed; the stub filled, `PENDING = None`): `CrackFront`,
`FrontShape`, `j_integral`, `FrontValue`, `plane_strain_k`, `opening_k`, `OpeningPair`, `OpeningFit`, `paris_cycles`,
`PlateCrack`, `centre_crack_plate`, `CrackedPlate`, `PennyCrack`, `penny_crack_cylinder`, `CrackedCylinder`, as in D1 to D8.
Tests: `crates/rcfd-fem/tests/sa_fracture.rs` (every row); `sa_scaffold.rs` (D9: the live X1 test given an `#[ignore = "record:
ADR 0547 ..."]` attribute, body unchanged, and the continued X1 test with `fracture` landed). No shared solver file changed.

Before the registered run two development probes on set-ups that are not registered (a plate with `h0 = a/10`, `n_u = 2`,
`g = 1.6`, `H = W`; a penny with `h0 = a/6`, `n_theta = 4`, `g = 1.7`) were run to check the pipeline (a temporary test file,
deleted, not committed): plate K from J of rings 2 to 4 within 0.2 %, opening 4 % low; penny ring 2 1.7 % low. Nothing of the
registration was changed after them. The fast registered rows were then run once (`f429-scratch/run1.log`, code of b6e87f97
before formatting), the heavy record PH once alone (`f429-scratch/heavy.log`, 96 267 unknowns, supernodal solve 35.8 s). After
the runs `cargo clippy` asked for the negated comparisons (`!(x > 0.0)`) in `fracture.rs` to be written as `x.is_nan() || x <=
0.0` and the like (the same truth values for every input, NaN included); the crate's whole test run that followed includes the
fast rows on that code (passed).

| check | limit | measured | met |
|---|---|---|---|
| P1 K from J, rings 2-4, both stations, against `sigma sqrt(pi a) sqrt(sec(pi a/(2 W)))` | `<= 0.02` | 2.12e-3 (ring 2 -2.12e-3, ring 3 -1.55e-3, ring 4 -1.53e-3) | yes |
| P2 spread of the J of rings 2-4 | `<= 0.01` | 1.20e-3 | yes |
| P3 J against the energy difference of `0.98 a`, `1.02 a` | `<= 0.02` | 7.45e-5 (`J_E` 1442.88, ring-3 J 1442.77 J/m^2) | yes |
| P4 K from the opening, `[0.1 a, 0.4 a]`, 9 points | `<= 0.05` | 1.20e-2 (`K_0` 1.7620e7 against 1.7835e7) | yes |
| P5 shear opening factor / tension value | `<= 1e-8` | 1.25e-13 | yes |
| P6 the two stations' ring-3 J | `<= 1e-10` | 2.8e-14 | yes |
| PH1 penny, ring-3 K at every station against `2 sigma sqrt(a/pi)` | `<= 0.03` | 2.24e-3 | yes |
| PH2 penny, ring-3 K along the front (13 stations) | `<= 0.02` | 2.7e-7 | yes |
| PH3 penny rings 2 and 4 (printed) | none | 2.76e-3, 2.22e-3 | (record) |
| PF coarse penny, ring 2 | `<= 0.10` | 1.27e-2 | yes |
| G1 Paris `m = 3` against the closed form | `<= 1e-6` | 4.4e-16 | yes |
| G2 Paris `m = 2` against the logarithm | `<= 1e-6` | 2.2e-16 | yes |
| G3 Paris `m = 3` with `(1 + a/a1)` | `<= 1e-6` | 6.7e-16 | yes |
| R1a front bent by `1e-4 t`: refused, "not straight" | refused | refused | yes |
| R1b front bent by `1e-8 t`: accepted | accepted | accepted | yes |
| R2a ring 19 (`columns_to_edge + 1`, 18 by arithmetic 2): refused, "reaches the boundary" | refused | refused (a node on `x = W`) | yes |
| R2b ring 18 accepted | accepted | J 1442.98 | yes |
| R3 the 13 refusals | refused | refused, each with its key | yes |
| R4a penny front as straight: "not straight"; R4b centre `1e-4 a` refused ("circle"), `1e-8 a` accepted | as stated | as stated | yes |
| N1 reversed advance gives `-J` | `<= 1e-12` | 0 | yes |
| N2 ring 1 against ring 3 (printed) | none | ring 1 7.2 % low | (record) |
| F1 every aggregated error finite | finite | finite | yes |

Mutations (`f429-scratch/mutate.py` on the copy `f429-scratch/mut` of b6e87f97, `CARGO_TARGET_DIR` `target-f429-mut`, the fast
rows; log `f429-scratch/mutations.log`), each run once:

| mutation | caught by | as registered? |
|---|---|---|
| M1 `- W q_k,k` dropped | P1 (7.7e-2), P2, P3 | yes |
| M2 gradient transposed | P3 (3.10) | P3 yes; P1 not |
| M3 half-symmetric factor 2 dropped | PF (0.30) | yes |
| M4 plane stress `sqrt(J E)` | P1 (4.8e-2) | P1 yes; PF not (4.6 % is inside its 10 %) |
| M5 `sqrt(pi/r)` | P4 (0.30) | yes |
| M6 opening from the `x` component | P4, P5 | yes |
| M7 normalisation by the front length | P1, P3, PF | yes |
| M8 `rho` from `S_k` | R2b, P1, P2, P3, P6 | yes |
| M9 boundary test disabled | R2a | yes |
| M10 straightness test disabled | R1a; R4a (then refused by the advance check, without "not straight") | yes |
| M11 substitution Jacobian dropped | G1, G2, G3 | yes |
| M12 inner rings left out of the integral | **none** | **no** |

Diagnosis of M12 (one diagnostic run of the mutated PF, `f429-scratch/m12-diagnosis.log`, not a registered record): PF moves
from `-1.27 %` to `+2.79 %`, a change of 4 % that stays inside PF's limit of 10 %. In the one-layer plate the terms of the inner
elements vanish exactly (arithmetic 4); on the circular front only the curvature term `(sigma_thth u_r/r - W) phi/R` of the
small inner elements remains. The registration expected PF to catch it; PF's limit was set for a coarse mesh and is too loose for
this effect. That is a gap in coverage, not a limit to change.

Also run: `cargo test -p rcfd-fem --release` (235 test binaries, 1005 passed, 0 failed, 276 ignored), `cargo test -p rcfd-api --release --test
baseline_hashes` (3 passed, 1 ignored: unchanged), `cargo test -p rcfd-cli --release --test docs_site`, `cargo clippy -p
rcfd-fem --all-targets --release -- -D warnings` (clean after the rewrite above), `cargo fmt -p rcfd-fem` (applied; `--check`
clean).

Deviations: none from the registered set-ups; the clippy rewrite after the runs is described above.

**Decision by the registered rule: partial.** Every check of the acceptance (c1 to c10) and every other registered check and
control is met, PH included. Mutation M12 was caught by no registered check, and the rule asks for every mutation to be caught.

Remaining questions:

1. Status. Options: (a) partial by the rule as written; (b) done, since every acceptance clause is met and the uncaught mutation
   affects only curved fronts, where PH1 still met its limit. Recommendation: (a) until a check that catches M12 exists (the
   follow-up below); the lead or maintainer may take (b).
2. A case-file key for the fracture parameters. Options: none now; a post-processing block naming a front node set and rings.
   Recommendation: none now. Not taken: it would change a golden (schema), so it stays open.
3. The citations are from memory (reference-data F-429 section); deLorenzi 1982 and the coefficients of the Tada refinement are
   the least certain. Recommendation: check them when the books are at hand; no limit depends on them (the checks are against
   the plain secant form and Sneddon's closed form).
4. Quarter-point elements are refused by the mesh validation (`det J = 0` at the tip). Recommendation: leave it; the graded mesh
   meets every limit, and accepting singular elements would change shared validation.

## For the lead: roadmap entries

Proposed status of F-429: partial by the registered rule (one mutation not caught; every acceptance clause met). Note:

> Structural solver, advanced tier (plan ADR 0310, stub ADR 0332). ADR 0547, 2026-11-15: rcfd_fem::fracture. CrackFront (straight within 1e-6 of its length or circular within 1e-6 of its radius; plane normal, advance direction, half-symmetric flag); j_integral, the equivalent domain integral int (sigma_ij u_i,k - W delta_jk) q_k,j dV over rings of element layers about the front, q = rho phi_j e with a hat per front station, refusing a ring whose q = 1 nodes reach a boundary other than the crack faces, the mirror plane and the end faces; plane_strain_k (K = sqrt(J E/(1 - nu^2))); opening_k (least-squares line of K_app = E delta sqrt(2 pi/r)/(8 (1 - nu^2)) extrapolated to r = 0); paris_cycles (Gauss-Legendre in log a); builders centre_crack_plate (slit of doubled nodes, regular Hex20 graded towards the tips; quarter-point elements are refused by the mesh validation) and penny_crack_cylinder. Met: centre crack W = 10 a, plane strain, Hex20: K from J of rings 2-4 within 2.1e-3 of the secant form (limit 2e-2), rings within 1.2e-3 of each other, J against the energy difference of a +/- 0.02 a within 7.5e-5, K from the opening within 1.2e-2 (limit 5e-2); pure shear opening factor 1.3e-13 of the tension value (limit 1e-8); penny crack in R = 10 a (heavy record, 96 267 unknowns) K within 2.2e-3 of 2 sigma sqrt(a/pi), uniform along the front within 2.7e-7; Paris m = 3 and m = 2 within 7e-16; a bent front and a ring reaching the boundary refused. Partial by the rule: mutation M12 (inner rings left out of the integral) caught by no registered check (11 of 12 caught). No case-file key.

Proposed follow-up item:

> F-429b (deps F-429): Domain integral on a front with varying J, and mode separation. Acceptance: Every limit not stated here is registered in the ADR before the run. An embedded elliptical crack whose K varies along the front against the closed form of Irwin within a registered limit, with a check that catches leaving the inner rings out of the tapered integral; an interaction integral separating K_I, K_II and K_III, the inclined centre crack within 2 percent of each closed form.
