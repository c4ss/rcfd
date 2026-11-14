# ADR 0433: F-379 registration: closed-form benchmarks, the cantilever in bending and the thick-walled cylinder under pressure

Status: **REGISTERED 2026-11-14, written before any code of F-379 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-379 was written, compiled or run: the code listed under "What was read" was read and
the arithmetic below was done by hand. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parent: [ADR 0280](0280-closed-form-benchmarks-cantilever-and-thick-cylinder.md) (the planned stub, SB-10 of the basic tier,
[ADR 0270](0270-structural-solver-basic-tier-plan.md)). Roadmap item F-379 (depends on F-371, F-375, F-376). Size M. Branch
`f379-closed-form-benchmarks` from main dcdda624, worktree `w49i-work/f379`, scratch `w49i-work/f379-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs and not changed after a run (a miss
becomes an ignored `*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical (this item
adds one test file, no library code); no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no
existing test edited; no new dependency, no unsafe code; clean-room (closed forms derived below, first principles).

## What F-379 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. Cantilever of slenderness L/h = 20 (square section, nu =
> 0.3, clamped root, tip shear force): the tip deflection is within 1 percent of P L^3/(3 E I) (1 + 3 E I/(kappa G A L^2)),
> kappa = 5/6, with Tet10 on a registered mesh, and within 5 percent with Hex8 at four elements through the thickness; the
> bending stress at the surface one section height from the clamp is within 3 percent of M c/I (Tet10); Tet4 on the same nodes
> is the registered locking control (deflection below 90 percent of the closed form). Thick-walled cylinder, b/a = 2, plane
> strain, internal pressure, quarter model: the radial displacement at the bore and at the outer wall is within 0.5 percent and
> the hoop stress at the bore within 2 percent of Lame's solution (Hex8 and Tet10 with curved edges on registered meshes), and
> the observed order of the radial displacement error with Hex8 is >= 1.8.

Clauses: (a1) the Tet10 mesh registered, tip deflection within 1 percent of `delta_T`; (a2) Hex8 with four elements through the
thickness, within 5 percent; (a3) Tet10 bending stress at the surface at `x = h` within 3 percent of `M c / I`; (a4) Tet4 on the
same nodes below 90 percent of `delta_T` (locking control); (a5) the clamp and the tip load modelled as registered here; (a6)
cylinder `b/a = 2`, plane strain, internal pressure, quarter model; (a7) Hex8: `u_r` at the bore and at the outer wall within 0.5
percent; (a8) Hex8: bore hoop stress within 2 percent; (a9) Tet10 with curved edges: the same as a7 and a8; (a10) Hex8: observed
order of the `u_r` error `>= 1.8`.

## What was read (no run)

- ADR 0280 (the stub), ADR 0270 section of SB-10, ADR 0413 (F-371: `convert::{from_mesh, split_hexahedra,
  project_mid_edge_nodes, onto_cylinder}`), ADR 0415 (the curved Hex20 quarter annulus), ADR 0292 (Hex20; its arithmetic 4 and 5,
  and its unregistered diagnostic of the full clamp: Hex20 ratios `10 x 1` 0.981949, `40 x 4` 0.995265, `40 x 4, nu = 0`
  0.999995; Hex8 `10 x 1 x 1` 0.369), ADR 0385 (F-485: the support of beam theory at the root, its derivation, measured ratio
  0.999608 for Hex20 `10 x 1 x 1`), ADR 0284 addendum (Tet10 cantilever frequencies against Timoshenko, full clamp: `40 x 2 x 2`
  3.81e-3, `80 x 4 x 4` 2.72e-3; Hex8 `40 x 2 x 2` 7 percent stiff), ADR 0277 addendum (strain energies of the clamped `L/h = 10`
  cantilever, divisions `10 x 2 x 2`: Tet4 28.3 J, Hex8 55.6 J, Tet10 79.2 J).
- `crates/rcfd-fem/src/builders.rs` (`hex_box`, `tet_box`: Kuhn split, node and surface sets `xmin .. zmax`, `all`;
  `upgrade_to_tet10`: mid-edge node at the mean of its ends, node sets gain the mid-edge nodes of their edges),
  `crates/rcfd-fem/src/model.rs` (`Load::traction`, `Load::pressure`, `Constraint::displacement`, `SolidResult::reactions`),
  `crates/rcfd-fem/src/loads.rs` (`nodal_load_vector`; face loads on Quad4 and Tri6 faces with `t = -p n`),
  `crates/rcfd-fem/src/recover.rs` (`IntegrationPointField::from_result`, `nodal_average`, Voigt order xx, yy, zz, xy, yz, zx,
  tensor shear), `crates/rcfd-fem/src/solve.rs` (`solve_static`, the default direct solve after reverse Cuthill-McKee),
  `crates/rcfd-fem/tests/si_hex20_support_bore.rs` (the beam support set-up, the `Checks` helper), `sb_static.rs` (`rebuild`).
- Scaffold rows: `grep "F-379\|ADR 0280"` over `crates/rcfd-fem/tests` finds no scaffold row of this item (the only hit,
  `si_amg_quadratic.rs:474`, is an ADR 0370 record of another item; `si_geometric.rs:79` is a number). Nothing to retire.

## Design

No library code. One new test file `crates/rcfd-fem/tests/sb_closed_forms.rs` (the file named by ADR 0280). Material `E = 2e11
Pa`, `nu = 0.3`, `rho = 7850`, no expansion. The default static solve (`StaticOptions::default()`, direct).

### D1. The cantilever and its meshes (clauses a1, a2, a4)

`L = 2 m`, `h = 0.1 m` (square section `h x h`, `L/h = 20`), `P = 1000 N` in `-z`. Three meshes on the same 2025 nodes (the grid
of spacing `L/80` along x and `h/4` across):

- **Tet10** (a1): `tet_box([0; 3], [L, h, h], [40, 2, 2])` upgraded by `upgrade_to_tet10`: 40 x 2 x 2 cells, 6 tetrahedra each,
  960 Tet10, element edge `h/2` along the axes. The Kuhn split puts one diagonal on every cell face and one through every cell,
  so the mid-edge nodes are exactly the face centres, the cell centres and the edge midpoints of the coarse grid: the node set is
  the full half-grid `81 x 5 x 5 = 2025` (check K0).
- **Tet4** (a4, "on the same nodes"): `tet_box([0; 3], [L, h, h], [80, 4, 4])`, whose vertices are that half-grid: the same 2025
  nodes and unknowns, linear elements.
- **Hex8** (a2): `hex_box([0; 3], [L, h, h], [80, 4, 4])`: four elements through the thickness and four across the width, cubic
  elements of edge `h/4`, again the same 2025 nodes.

### D2. The root support (clause a5): the clamp of beam theory, the full clamp recorded

The closed form is the Timoshenko cantilever: at the root the section's rotation `psi(0) = 0` and the deflection of the axis
`w(0) = 0`; nothing restrains the section in its own plane (Poisson strains) and the axis slope at the root equals the shear
angle `V / (kappa G A)`. A 3-D solid fixed in all three components on the root face (the "full clamp") is not this support: it
suppresses the lateral Poisson strains `eps_y = eps_z = -nu sigma_x / E` of the root fibres, a local stiffening that decays over
about one section height. ADR 0292's diagnostic measured it on Hex20: 0.4735 percent stiff at `40 x 4` (ratio 0.995265 against
0.999995 with `nu = 0` on the same mesh), more on coarse meshes. With the 1 percent limit of a1 that would consume half the
budget. The registered support is therefore the one of ADR 0385 D1 (derivation there, first principles), on these meshes:

1. `u_x = 0` at every node of the root face (`xmin`, component 0): the plane `x = 0` becomes a plane of symmetry (half of a
   beam `2L` loaded at both ends), the root section stays plane and normal to the axis (`psi(0) = 0`), and its in-plane motion is
   free.
2. `u_z = 0` at the two neutral-line nodes `n0 = (0, 0, h/2)` and `n1 = (0, h, h/2)` (both exist: `z = h/2` is a grid plane of
   every mesh of D1; for Tet10 these are corner nodes of the 40 x 2 x 2 grid).
3. `u_y = 0` at `n0` only.

Items 2 and 3 remove the in-plane rigid motions `t_y`, `t_z`, `r_x` and nothing else (statically determinate in the section
plane: reactions `R_z(n0) = R_z(n1) = P/2`, `R_y(n0) = 0`); item 1 removes `t_x`, `r_y`, `r_z`. The deflection is relative to
the root neutral line, where `u_z = 0`, which is `w(0) = 0` of the beam.

What this support leaves (bounded, first principles):

- **Warping restraint.** Item 1 forces `u_x = 0` over the whole root face, so the Saint-Venant shear warping of the section
  (`u_x` cubic in `z`, amplitude of order `V h^2 / (G A) ~ 1.3e-7 m` here) is held at the root. The restraint is a
  self-equilibrated end state; its energy is of order `(V / A)^2 h^3 / G` against the bending energy `P^2 L^3 / (6 E I)`: relative
  `~ (E/G) (h/L)^3 / 2 ~ 1.6e-4`. ADR 0385 measured the whole effect with this support at 0.999608 on Hex20 (it contains the
  discretisation too). Bound used: below 5e-4 on the deflection.
- **Concentrated reactions.** `P/2` at `n0`, `n1`: a local logarithmic compliance, ADR 0385 arithmetic: about `5e-5` relative.
- **At `x = h` (the stress of a3).** The warping restraint's axial stress at the root is of order `E` times the warping strain,
  `~ 2.6 x 1.5 P/A = 3.9e5 Pa` against the bending stress `1.14e7 Pa` at `x = h`, about 3.4 percent at the root, decaying like
  `exp(-k x / h)` with `k` of order 4 for the lowest self-equilibrated end state of a square or strip section (Papkovich-Fadle
  rate for the strip, from memory; used only as an order of magnitude): `x = h` gives `3.4 x e^-4 ~ 0.06` percent. The
  concentrated reactions at distance `>= h` from the check point: Boussinesq-type stress `~ (P/2) / (2 pi h^2) = 8e3 Pa`, 0.07
  percent. Bound used: below 0.3 percent at `x = h`.

The full clamp (`xmin` fixed in all components) is run beside the support on Tet10 and Hex8 and recorded (R1), with two checks
that follow from first principles: added prescribed zeros can only lower the compliance, so `ratio_clamp <= ratio_beam`
(strict up to roundoff), and the clamp effect stays of the size ADR 0292 measured, `ratio_clamp > 0.98` for Tet10.

### D3. The tip load and the deflection measure (clause a5)

The tip shear force is the uniform traction `(0, 0, -P / h^2)` on `xmax` (`Load::traction`, the consistent face load), not the
parabolic shear distribution of the Saint-Venant flexure solution. The two differ by a self-equilibrated tip traction whose
effect, by Saint-Venant, is local: its energy is of order `(P/A)^2 h A / G` against `P delta_T / 2`, relative `~ 3 E I h / (G A
L^3) = 3 x 2.6 x (1/12) (h/L)^3 = 8e-5`; neglected. A nodal load at one node would add a singular local compliance; not used.

Deflection: `delta = sum_i f_i u_i / P` over the unknowns, `f` the consistent load vector of the model (`nodal_load_vector`),
i.e. the area mean of `-u_z` over the tip face, the work-conjugate measure (exact for the consistent load; ADR 0292 amendment 1).

Closed form (ADR 0292 arithmetic 4, re-derived): `A = h^2 = 0.01`, `I = h^4 / 12 = 8.3333e-6`, `G = E / (2 (1 + nu)) = E / 2.6`,
`3 E I / (kappa G A L^2) = 3 x 2.6 x 0.01 / (12 x (5/6) x 4) = 1.95e-3`, `delta_T = P L^3 / (3 E I) x 1.00195 = 1.6031e-3 m`.

### D4. The bending stress at one section height (clause a3)

Section `x = h = 0.1` (a grid plane of every mesh: node index 2 of the Tet10 coarse grid). `M = P (L - h) = 1900 N m`, `c =
h/2`, `M c / I = 1900 x 0.05 / 8.3333e-6 = 1.14e7 Pa`; with the load in `-z` the top `z = h` is in tension. Measured: `sigma_xx`
of `nodal_average(model, field, field.stress(), None)` at the nodes of the section on the top and the bottom surface. Two nodes
are the check (the mid-width nodes `(h, h/2, h)` and `(h, h/2, 0)`, against `+M c/I` and `-M c/I`) and the five nodes per surface
line (`y = 0, h/4, ..., h`) a second check with the same limit (Saint-Venant's `sigma_xx = M z / I` does not depend on `y`).

Error estimate for Tet10 (quadratic displacement, linear strain per element): the flexure solution has `sigma_xx` bilinear in
`x z` and displacements with cubic terms (`x^3` in `u_z`, `x y z` in `u_y`); their quadratic approximation errs in the strains
by `~ e^2 u''' / 6` with `e = h/2`; the bending strain error so made, relative, `~ e^2 / (6 (L - h) c) = 0.0025 / (6 x 0.095) ~
4e-3` as an upper bound (most of it in the shear strain, which a3 does not check); the extrapolation of a linear field to the
nodes is exact; the root effect of D2 adds below 0.3 percent. Expected below 1 percent; limit 3 percent (the acceptance).

### D5. Expected deflections (estimates behind a1, a2, a4)

- **Tet10 40 x 2 x 2.** ADR 0284's Tet10 frequencies under the full clamp: 3.81e-3 (40 x 2 x 2) and 2.72e-3 (80 x 4 x 4) against
  Timoshenko. The frequency error is half the stiffness error: 0.76 and 0.54 percent stiff, of which the full clamp is about
  0.47 percent (ADR 0292's Hex20 40 x 4); what is left for the Tet10 40 x 2 x 2 discretisation is about 0.3 percent. With the
  support of D2 (no Poisson restraint; warping and reactions below 6e-4): ratio 0.996 to 0.999. Limit 1 percent.
- **Hex8 80 x 4 x 4** (full integration, cubic elements, `a / h = 1/4` with `a` the element length). Parasitic shear of the
  bilinear element in bending (first principles): in an element of length `a` the bending mode `u_x = kappa x z` carries the
  shear strain `kappa (x - x_c)` that the compatible deflection cannot cancel (it is piecewise linear in `x`), mean square
  `kappa^2 a^2 / 12`, independent of the number of elements through the thickness; against the bending energy density mean
  `E kappa^2 h^2 / 12` it adds the relative stiffness `(G/E)(a/h)^2 = (1/4)^2 / 2.6 = 0.024`. The Poisson strains of the
  section (`w ~ nu kappa (z^2 - y^2) / 2`, quadratic) are piecewise linear across four elements: a sawtooth strain error
  `nu kappa e_z / 2`, relative energy `~ ((lambda + 2 mu)/E) nu^2 (e_z/h)^2 = 1.346 x 0.09 / 16 = 0.0076`, about 0.01 for both
  directions. Ratio `~ 1 / (1 + 0.024 + 0.01) = 0.967`. Calibration of the formula on recorded runs: ADR 0292 Hex8 `10 x 1 x 1`
  (`a / h = 2`, one element through: the Poisson factor `1/(1 - nu^2)` in full): `1 / (1.099 + 1.538) = 0.379` against the
  measured 0.369; ADR 0277 Hex8 `10 x 2 x 2` at `L/h = 10` (`a / h = 1`): `1 / (1 + 0.385 + 0.04) = 0.70` against 55.6 / 79.2 =
  0.702. Expected 0.96 to 0.975; limit 5 percent, ratio `>= 0.95` (margin about 1.7 percent of `delta_T`).
- **Tet4 80 x 4 x 4.** ADR 0277: Tet4 `10 x 2 x 2` (element edge `e = h/2`) has 28.3 / 79.2 = 0.357 of the Tet10 energy (the
  same load, so the same ratio of deflections). Locking of constant-strain elements grows like `(e/h)^2` (parasitic shear as
  above): `1 / 0.357 - 1 = 1.80` at `e/h = 1/2`, so `0.45` at `e/h = 1/4`: ratio about 0.69 (0.53 if it scaled only like
  `e/h`). Limit: below 0.9 (the acceptance).

### D6. The cylinder: Lame's solution (clause a6), derived

Plane strain, `u = u_r(r) e_r`, `eps_r = u_r'`, `eps_theta = u_r / r`, `eps_z = 0`. Equilibrium `d sigma_r / dr + (sigma_r -
sigma_theta)/r = 0` with `sigma = lambda tr(eps) I + 2 mu eps` gives `(lambda + 2 mu) d/dr (1/r d(r u_r)/dr) = 0`, so `u_r = C1
r + C2 / r`. Then `sigma_r = 2 (lambda + mu) C1 - 2 mu C2 / r^2`, `sigma_theta = 2 (lambda + mu) C1 + 2 mu C2 / r^2`, i.e.
`sigma_r = A - B / r^2`, `sigma_theta = A + B / r^2` with `A = 2 (lambda + mu) C1`, `B = 2 mu C2`. Boundary conditions
`sigma_r(a) = -p`, `sigma_r(b) = 0`: `A = p a^2 / (b^2 - a^2)`, `B = A b^2`. Back: `C1 = A / (2 (lambda + mu)) = (1 + nu)(1 -
2 nu) A / E`, `C2 = B / (2 mu) = (1 + nu) B / E`:

`u_r(r) = (1 + nu)/E ((1 - 2 nu) A r + B / r)`, `sigma_z = nu (sigma_r + sigma_theta) = 2 nu A`.

With `a = 1`, `b = 2`, `nu = 0.3`: `A = p/3`, `B = 4p/3`; `u_r(a) = 1.3 (0.4/3 + 4/3) p/E = 1.906667 p/E`, `u_r(b) = 1.3 (0.8/3 +
2/3) p/E = 1.213333 p/E`, `sigma_theta(a) = A + B = 5p/3`. Plane stress (control N2): `u_r = ((1 - nu) A r + (1 + nu) B / r)/E`,
`u_r(a) = (0.7/3 + 5.2/3) p/E = 1.966667 p/E`, 3.15 percent above plane strain.

### D7. The cylinder model (clause a6)

Quarter `0 <= theta <= pi/2`, `a = 1 m`, `b = 2 m`, axial thickness `H = 0.1 m`, `p = 1e6 Pa`. Meshes are a box in the
parameters `(s, t, z) in [0, 1] x [0, 1] x [0, H]` (`hex_box` or `tet_box` with divisions `[n_r, n_theta, 1]`), every node
mapped (the sets are kept) by

`r(s) = a + (b - a) (e^{beta s} - 1) / (e^beta - 1)`, `theta = t pi / 2`, `(x, y, z) = (r cos theta, r sin theta, z)`,

with **`beta = 2`** (D8). The map has Jacobian determinant `r r'(s) pi/2 > 0`, so orientation and the outward faces are kept;
`SolidMesh::new` and `add_block` check `det J` again at every node.

- **Hex8**: the corners mapped; straight edges (chords of `pi / (2 n_theta)` on the walls). Levels `k = 1, 2, 4, 8`: `n_r = 4k`,
  `n_theta = 8k`. The registered mesh of a7 and a8 is `k = 8` (32 x 64, 4290 nodes).
- **Tet10 with curved edges**: `tet_box` in the parameters, `upgrade_to_tet10` there (mid-edge nodes at the parameter
  midpoints), then every node, corner and mid-edge, mapped: each element is the image of a straight-sided parameter tetrahedron,
  every edge of the bore and the outer wall lies on its circle (check K2). This is the map of all nodes rather than
  `project_mid_edge_nodes` (ADR 0413), which would leave interior mid-edge nodes on chords; both give curved boundary edges. The
  registered mesh: `n_r = 8`, `n_theta = 16` (768 Tet10, 1683 nodes).

Sets after the map: `xmin` the bore (`r = a`), `xmax` the outer wall, `ymin` the plane `theta = 0` (normal `y`), `ymax` the plane
`theta = pi/2` (normal `x`), `zmin`, `zmax`, `all`. Constraints: symmetry `u_y = 0` on `ymin`, `u_x = 0` on `ymax`; **plane
strain** by `u_z = 0` at every node (`all`, component 2; on Tet10 `all` holds the mid-edge nodes too): `eps_z = 0` exactly, the
axial reaction carries `sigma_z`. Load: `Load::pressure("xmin", p)` (positive pressing on the solid).

Measures: `u_r = (u_x x + u_y y) / r` at every node of `xmin` and of `xmax` against `u_r(a)`, `u_r(b)`, the largest relative
error over each set; the hoop stress `sigma_theta = t^T sigma t`, `t = (-sin theta, cos theta, 0)`, of
`nodal_average(None)` at every bore node, largest relative error against `5p/3`.

### D8. Expected cylinder errors and the grading (clauses a7 to a10)

- **Hex8 bore hoop stress, first order.** Along the radius a bilinear element has `u_r` linear, so `eps_r` is the element's mean
  slope at both radial Gauss points and the extrapolation to the bore node keeps it: error `C2 / a^2 - C2 / (a (a + dr_1)) ~ C2
  dr_1 / a^3` for a first element of radial size `dr_1`. In `sigma_theta = (lambda + 2 mu) eps_theta + lambda eps_r` that is
  `lambda C2 dr_1 / a^3 = 0.5769 x 1.7333 p dr_1 = 1.0 p dr_1`, relative to `5p/3`: **`0.6 dr_1 / a`**; `eps_theta = u_r / r` is
  extrapolated with an `O(dr^2)` error. A uniform 32-element radius (`dr_1 = 1/32`) gives 1.8 percent, too close to the 2 percent
  limit; hence the grading: `r'(0) = (b - a) beta / (e^beta - 1)`, with `beta = 2`: `0.313`, `dr_1 = 0.313 / n_r`; at `k = 8`
  (`n_r = 32`): `dr_1 = 9.8e-3`, error about 0.6 percent. `beta = 2` is the smallest round value that puts the estimate at a
  third of the limit; the largest radial element grows to `r'(1) / n_r = 2.31 / 32 = 0.072`.
- **Hex8 displacement, second order.** Nodal values of linear elements for an equation with the reaction-like term `(lambda + 2
  mu) u / r^2`: relative error `~ dr^2 / (12 a^2)` locally, with `dr <= 0.072` at `k = 8`: `4e-4`; the chord geometry (the walls
  are inscribed polygons of angle `dtheta = pi/128`): `~ dtheta^2 / 12 = 5e-5`. Expected below 1e-3 at the bore and the outer
  wall; limit 5e-3. Both terms scale with `h^2` when `n_r` and `n_theta` are doubled together, so the observed order of the bore
  error between `k = 4` and `k = 8` is expected near 2 (limit 1.8).
- **Tet10 curved, `8 x 16`.** Quadratic displacement on the exact geometry (isoparametric image of a smooth map): displacement
  error `O(h^3)`, expected below 1e-3; bore stress second order, `~ 0.6 x 3 dr_1^2` with `dr_1 = 0.039` (factor 3 for the tetrahedra
  against the quadratic estimate, from ADR 0292's Hex20 calibration where the same estimate over-predicted by 3): about 3e-3.
  Limits 5e-3 and 2e-2 (the acceptance).

## Registered checks

All in `crates/rcfd-fem/tests/sb_closed_forms.rs`, release profile, run once (`cargo test -p rcfd-fem --release --test
sb_closed_forms -- --nocapture`).

### A. The acceptance

| Id | Clause | Measure | Limit | Estimate |
| --- | --- | --- | --- | --- |
| C1 | a1, a5 | Tet10 40 x 2 x 2, support D2, load D3: `abs(delta / delta_T - 1)` | <= 0.01 | 1e-3 to 4e-3 |
| C2 | a2, a5 | Hex8 80 x 4 x 4, the same: `abs(delta / delta_T - 1)` | <= 0.05 | 0.025 to 0.04 |
| C3 | a3 | Tet10 run of C1: `abs(sigma_xx / (+-M c / I) - 1)` at `(h, h/2, h)` and `(h, h/2, 0)` | each <= 0.03 | below 0.01 |
| C3b | a3 | the same over the five nodes of each surface line `x = h`, `z in {0, h}` | each <= 0.03 | below 0.015 |
| C4 | a4 | Tet4 80 x 4 x 4 (same nodes), the same support and load: `delta / delta_T` | < 0.9 | about 0.69 |
| T1 | a6, a7 | Hex8 `k = 8`: largest `abs(u_r / u_r(a) - 1)` over `xmin`; the same over `xmax` against `u_r(b)` | each <= 0.005 | below 1e-3 |
| T2 | a8 | Hex8 `k = 8`: largest `abs(sigma_theta / (5p/3) - 1)` over `xmin` | <= 0.02 | about 6e-3 |
| T3 | a10 | Hex8 levels 1, 2, 4, 8: bore error `e_k` of T1; observed order `log2(e_4 / e_8)` | >= 1.8 | near 2 |
| T4 | a9 | Tet10 curved 8 x 16: T1's two measures; T2's measure | <= 0.005 each; <= 0.02 | below 1e-3; about 3e-3 |

### K. The set-ups themselves

| Id | Measure | Limit | Estimate |
| --- | --- | --- | --- |
| K0 | the Tet10, Tet4 and Hex8 cantilevers each have 2025 nodes, and each node is within `1e-12` m of a distinct point of the grid `(i L/80, j h/4, k h/4)` | as stated | D1 |
| K1 | Tet10 run of C1, reactions: `abs(sum R_z / P - 1)`, `abs(R_z(n_i) / (P/2) - 1)` for both, `abs(R_y(n0)) / P`, `abs(sum R_x) / P` | each <= 1e-8 | roundoff, about 1e-11 |
| K2 | Tet10 cylinder: every node of `xmin` and `xmax` at `r = a` resp. `b` within 1e-14 | as stated | the map |
| K3 | load totals: `abs(sum R_y(ymin) / (p a H) + 1)` and `abs(sum R_x(ymax) / (p a H) + 1)`, Hex8 `k = 8` and Tet10 | each <= 1e-9 | the chord polygon and the quadratic faces both project to exactly `a H`; roundoff |

### R. Records with a first-principles check

| Id | Measure | Limit | Estimate |
| --- | --- | --- | --- |
| R1 | the full clamp (`xmin`, three components) on Tet10 and on Hex8: `delta / delta_T` printed, and `sigma_xx` at the C3 nodes printed; Tet10: `ratio_clamp <= ratio_beam + 1e-12` and `ratio_clamp > 0.98` | as stated | Tet10 about 0.991 to 0.994 |

### N. Negative controls

| Id | Control | Expected |
| --- | --- | --- |
| N1 | Tet4 locking (C4) | below 0.9 |
| N2 | Hex8 `k = 8` with `u_z = 0` on `zmin` only (no plane-strain constraint: plane stress, the axial strain free and exactly representable): bore error against plane-strain Lame `> 0.02`, and against plane-stress Lame (D6) `<= 0.005` | 3.15 percent; below 1e-3 |
| N3 | Hex8 `k = 8` with `beta -> 0` (uniform radius, `r = a + (b - a) s`): bore hoop error `e_u`, and `e_u / e_T2` in `[2, 4.5]` (first order in `dr_1`: `(1/32) / (0.313/32) = 3.2`) | about 1.8 percent; ratio about 3 |

### S. Existing tests

| Id | Measure | Limit |
| --- | --- | --- |
| S1 | `git diff main --stat -- crates` lists only the new test file | as stated (no library code, so no baseline-hash run is owed) |

### Mutations (on a scratch copy `w49i-work/f379-scratch/tree`, not committed)

| Id | Mutation | Expected to be caught by |
| --- | --- | --- |
| Mu1 | `material.rs`: the shear modulus `mu` of the elasticity matrix times 1.02 | C1, T1 |
| Mu2 | `loads.rs`: the pressure traction `+p n` instead of `-p n` | T1, T4, K3 |
| Mu3 | `recover.rs`: `extrapolate` returns the element mean at every node | C3, T2, T4 |
| Mu4 | test: Lame's `u_r` with `(1 - nu)` in place of `(1 - 2 nu)` | T1, T4 |
| Mu5 | test: the hoop stress with `- 2 s_xy t_x t_y` | T2, T4 |
| Mu6 | test: Tet10 cylinder mid-edge nodes at the chord midpoints (corners mapped, then `upgrade_to_tet10`) | K2 only (the chord error, about `dtheta^2 / 12 = 8e-4`, is inside T4's limits: recorded) |

## Decision rule

- **done**: every A check (C1 to C4, T1 to T4) met, every K and R check met, every negative control as expected, S1 as stated.
- **partial**: any A, K or R check missed (kept as a `*_known_miss` record with the limit intact and the measured number, the
  cause diagnosed), or a mutation other than Mu6 not caught, or a check met only after an amendment made after its run.
- **todo**: the checks could not be run.

## Limits of this registration

- The cantilever's acceptance is checked with the support of beam theory (D2), taken on the maintainer's standing instruction of
  2026-11-11 (take the recommendation): the closed form is the Timoshenko beam, whose clamp fixes the section's rotation and the
  axis, not the Poisson strains of the root fibres. The full 3-D clamp is recorded (R1) and bounded, not held to 1 percent.
- One mesh per element for the cantilever; no mesh study. The 1 percent limit cannot resolve the shear term (0.2 percent).
- The tip load is the uniform traction (D3); the parabolic distribution is not run.
- The cylinder is plane strain through `u_z = 0`; the generalised plane strain of a long tube with closed ends is not covered.
  Only `nu = 0.3`; near-incompressible behaviour is not covered here.
- The order is measured on the Hex8 bore displacement only, on one smooth graded family.
