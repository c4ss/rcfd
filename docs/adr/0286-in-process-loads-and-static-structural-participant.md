# ADR 0286: One-way coupling 2: fluid face forces in process, and a static solid as a co-simulation participant

Status: **REGISTERED 2026-11-12, written before any code of F-385 and before any run of its checks; planned by ADR 0270**.
Not yet accepted by the maintainer. Before this text nothing of F-385 was written, compiled or run: the code and documents
under "What was read" were read, and the arithmetic under "Arithmetic before this registration" was done by hand; no script,
no prototype and no library code was run. Results go in the addendum at the end; nothing above the addendum is changed after
a run.

Work package SB-16 of the structural solver, basic tier ([ADR 0270](0270-structural-solver-basic-tier-plan.md), section 3, the
entry for ADR 0286). Roadmap item F-385 (depends on F-374, F-377, F-139). Size M. Number 0286, fixed by the plan; the file name
of the planned stub is kept and the stub's text is kept unchanged at the end ("The planned stub, as written"). Branch
`f385-static-solid-participant` from main 519a87ee, worktree `w49i-work/f385`.

House rules (`w49i-work/AGENT_RULES.md`):

- limits are fixed here, before the code and the runs, and are not changed after a run; a missed limit becomes an ignored
  `*_known_miss` test with the limit intact and the number recorded, diagnosed, not tuned away; a defective set-up gets a
  corrected test by an amendment written and committed before the corrected run, the old test stays as an ignored record;
- each registered run is made once; nothing is re-run to pick a result;
- one library file is filled, the stub `crates/rcfd-post/src/solid_coupling.rs` declared by ADR 0278 (two comment lines, no
  item, nothing calls it), and one test file is added, `crates/rcfd-post/tests/sb_solid_coupling.rs`. No shared solver file
  (`rcfd-fv`, `rcfd-fem`), no golden, no hash table is touched, and nothing existing calls the new code, so every default
  result keeps its bits by construction;
- no new dependency: `rcfd-post` already depends on `rcfd-fv` (the `cosim` module) and on `rcfd-fem` (ADR 0278), so the
  workspace graph (`crates/rcfd-cli/tests/workspace_graph.rs`: `rcfd-fem` layer 2, `rcfd-post` layer 4) gains no edge; no
  unsafe code;
- clean-room: everything below is derived here from first principles (the transpose of a consistent interpolation, Betti's
  reciprocal theorem, the uniaxial stress state of linear elasticity, the fixed point of two springs in parallel). Boussinesq's
  solution of a load on an elastic half-space and the decay of self-equilibrated end loads of a strip (Papkovich-Fadle
  eigenvalues) are cited from memory, for an order-of-magnitude estimate only, and say so where used.

## What F-385 asks

The acceptance of `docs/roadmap.json`, binding:

> Every limit not stated here is registered in the ADR before the run. Fluid face forces on a wall are mapped to the nodes of a
> solid surface set by a given InterfaceMap and solved statically: resultant force, moment and work equal the fluid side's
> within 1e-12; for a matching planar interface under uniform pressure the solid's displacement equals the uniaxial closed form
> within 1e-10, and for a 3:2 non-matching one within a limit registered from the estimate of the mapping error. A StaticSolid
> implements cosim::Structure (advance is a static solve; restore returns the checkpoint bit for bit) and, with a test-only mock
> fluid whose force falls linearly with the wall displacement (stiffness k_f), coupled_step converges to u = F0/(k_s + k_f)
> within 1e-8 with the Aitken and with the least-squares update; a failing solve stops the step with the participant's name and
> both sides restored. No flow solver is coupled.

Clauses: (c1) face forces mapped to the nodes of a surface set by a given `InterfaceMap`, solved statically; resultant, moment
and work equal the fluid side's within 1e-12; (c2) matching planar interface, uniform pressure: displacement equals the
uniaxial closed form within 1e-10; (c3) 3:2 non-matching interface: within a limit registered here from the estimate of the
mapping error; (c4) `StaticSolid` implements `cosim::Structure`, advance is a static solve, restore returns the checkpoint bit
for bit; (c5) with a test-only mock fluid of stiffness `k_f`, `coupled_step` converges to `u = F0/(k_s + k_f)` within 1e-8,
with Aitken and with the least-squares update; (c6) a failing solve stops the step with the participant's name and both sides
restored; (c7) no flow solver is coupled.

## What was read (main 519a87ee, read only)

1. `AGENT_RULES.md`; ADR 0270 (section 1.1 departure 2, the alternatives of section 2.1, decision M5 and the lead's note that
   M5 (a) was taken: the participant in the basic tier, the glue in `rcfd-post`); this ADR's stub; the head, checks, mutations,
   decision rule and roadmap section of ADR 0279 (the style followed here); the roadmap entries and notes of F-139, F-360,
   F-384, F-385.
2. Code: `crates/rcfd-fv/src/cosim.rs` in full (traits `Participant`, `Structure`, `Fluid`; `InterfaceMap` with
   `forces_to_nodes`, `displacement_to_points`, `face_displacements`; `coupled_step`, `CouplingSettings::{aitken,
   least_squares}`, `AtCap`, `CouplingError`, the Aitken and least-squares updates, `SprungWall`);
   `crates/rcfd-fv/src/cosim/mapping.rs` in full (`MapSettings::projection`, the nearest-element projection that splits a
   quadrilateral into `(0, 1, 2)` and `(0, 2, 3)`, the offset correction); `Relaxation` in `crates/rcfd-fv/src/fsi.rs`;
   `crates/rcfd-fem/src/{lib.rs, solve.rs (to line 190 and the CG routine), builders.rs (to line 150)}`; the public
   signatures and doc lines of `model.rs`, `loads.rs`, `mesh.rs` (including the local faces of Hex8 and Tet4), `constraint.rs`,
   `material.rs`, `error.rs`; `crates/rcfd-post/{Cargo.toml, src/lib.rs, src/solid_coupling.rs}`; the layer table of
   `crates/rcfd-cli/tests/workspace_graph.rs`; the protocol rules of `docs/cosimulation-protocol.md` (grep: checkpoint, restore,
   structure).
3. **Not read:** ADR 0256, 0261, 0263, 0345 line by line (their decisions as reflected in the code and roadmap notes); the
   books cited from memory; any finite-element or co-simulation code outside this repository.

Findings that shape the design:

1. **The map is the coupler's.** `InterfaceMap` takes face forces to nodal forces by the transpose of the displacement map to
   the faces (`forces_to_nodes`) and displacements to points and faces (`displacement_to_points`, `face_displacements`). The
   in-process route therefore only has to number the interface nodes of a surface set, call `forces_to_nodes` and pass the
   nodal forces to `rcfd_fem::solve::solve_static_with` as `NodalForceAt`. No new mapping is written.
2. **The map builder takes elements of 1 to 4 nodes**; a quadrilateral is split into the triangles `(0, 1, 2)`, `(0, 2, 3)`
   (piecewise-linear weights). The solid face corners in the local face order of ADR 0271 are passed as these elements.
3. **The solve.** `solve_static_with(model, &[], forces, options)` with the model's own constraints. The direct solver is the
   default; `StaticSolver::ConjugateGradient { tolerance, max_iterations }` returns `FemError::Solver` ("solver cg-jacobi:
   not converged ...") at its cap: a genuine failing solve for clause c6.
4. **The coupler restores both participants after any failure** and names the participant (`CouplingError::participant`) and
   the iteration; the participant must answer `name`, `time`, `checkpoint`, `restore`, `node_coordinates`, `advance`.

## Arithmetic before this registration (disclosed, by hand)

1. **Bar.** `hex_box([0,0,0], [L, W, W], [8, 2, 2])`, `L = 4 m`, `W = 1 m`, cubes of `h = 0.5 m`; `E = 2e11 Pa`, `nu = 0.3`,
   density 0, expansion 0. Constraints (rollers): `u_x = 0` on node set `xmin`, `u_y = 0` on `ymin`, `u_z = 0` on `zmin`. They
   hold every rigid motion (a rotation about x moves `ymin` in y; about y moves `xmin` in x; about z moves `xmin` in x) and
   leave the lateral contraction free. Interface: surface set `xmax` (4 Quad4 faces, 9 nodes), area `A = W^2 = 1 m^2`.
2. **Closed form (c2).** A uniform pressure `p` on `xmax` with the rollers gives the uniaxial stress `sigma_xx = -p`, all
   other stresses zero, and `u_x = -p x/E`, `u_y = nu p y/E`, `u_z = nu p z/E`: linear, hence in the trilinear space, so the
   finite-element solution equals it up to rounding when the nodal loads are the consistent ones (`p h^2/4` per face corner of a
   square face). `p = 1e6 Pa`: `u_x(L) = -2e-5 m`. Rounding of the direct solve: condition number of `K_FF` of 135 unknowns of
   order `1e2 to 1e3`, so `<= 1e-13` relative; limit **1e-10** (acceptance), error measured as the largest `|u_h - u|` over every
   node and component divided by `p L/E`.
3. **Matching fluid wall.** Wall points: the 9 node positions of `xmax`, listed in another order than the solid's (z first,
   then y descending) so that the map is a permutation, not the identity; 4 wall faces, the squares of the solid faces, points
   counter-clockwise seen from the fluid (+x). The fluid's force on the solid: `F_f = -p A_f e_x`, `A_f = h^2`. The map
   (projection, `max_distance = 1e-9 m`) gives each wall point the weight 1 on its node up to rounding; `forces_to_nodes` gives
   `F_f/4` to each corner: the consistent load of a uniform pressure on a square Quad4 face, so arithmetic 2 holds.
4. **Balance (c1).** With `d_p = sum_i H_pi q_i` and `f_i = sum_f (1/n_f) sum_{p in f} H_pi F_f`:
   - resultant `sum_i f_i = sum_f F_f` because every row sums to 1;
   - work `sum_i f_i . q_i = sum_f F_f . d_f`, `d_f = (1/n_f) sum_{p in f} d_p`, for any weights (the transpose);
   - moment about `o`: `sum_i (x_i - o) x f_i = sum_f (xh_f - o) x F_f` with `xh_f = (1/n_f) sum_{p in f} sum_i H_pi x_i`; when
     the rows reproduce the points' coordinates (`sum_i H_pi x_i = x_p`, true for points on a planar triangulated surface,
     `MapReport::precision_defect` of rounding size) `xh_f = c_f`, the mean of the face's wall points. **The fluid side's moment
     is defined with `c_f`** (the protocol puts a face's displacement, and hence its point of application, at the mean of its
     points). Each equality is an identity of sums of a few hundred terms; rounding `<= n eps ~ 1e-14` relative to the scales
     `sum_f |F_f|`, `sum_f |c_f - o| |F_f|`, `sum_f |F_f| |d_f|`. Limit **1e-12** each (acceptance).
5. **The 3:2 wall (c3).** Wall points on the grid `{0, 1/3, 2/3, 1}^2` (in y, z, units of W) on `x = L`, 3 x 3 square faces of
   area `1/9`, uniform pressure `F_f = -p A_f e_x`. A wall point gets `A_f/4` from each face it belongs to: in one direction
   the weights `(1, 2, 2, 1)/6`, in two their products (`1/36` corner, `1/18` edge, `1/9` interior; sum 1, in units of `p A`).
   The solid face of the hexahedron's local face `[1, 2, 6, 5]` on `xmax` runs `(y0,z0), (y1,z0), (y1,z1), (y0,z1)`, so the
   map splits each solid square along the diagonal from `(y0,z0)` to `(y1,z1)`: lower triangle (`t <= s` in local `s, t`)
   with weights `(1 - s, s - t, t)` on `00, 10, 11`, upper with `(1 - t, s, t - s)` on `00, 11, 01`. Node grid `(I, J)`, `y = I
   h`, `z = J h`. The interior points: `(1/3,1/3)` (cell 00, `s = t = 2/3`): `1/3` to node 00, `2/3` to 11; `(2/3,2/3)`: `2/3`
   to 11, `1/3` to 22; `(1/3,2/3)` (lower triangle of cell 01, `s = 2/3, t = 1/3`): `1/3` each to 01, 11, 12; `(2/3,1/3)`
   (upper triangle of cell 10): `1/3` each to 10, 21, 11. Edge points split linearly along the edge (`1/3, 2/3`). Summed, in
   units of `p A / 108`: nodes 00 and 22: 11; nodes 20 and 02: 7; the four edge-mid nodes: 12; the centre: 24 (sum 108). The
   consistent loads of a uniform pressure: corners 6.75, edges 13.5, centre 27. The difference `delta f` (units `p A/108`):
   `+4.25` at 00 and 22, `+0.25` at 20 and 02, `-1.5` at the edge-mid nodes, `-3` at the centre: zero resultant, zero moment
   about the face centre (checked by hand: `sum delta y = 0`, `sum delta z = 0`): a self-equilibrated perturbation of the
   consistent load, largest `4.25/108 = 0.039 p A`, 63 % of a consistent corner load.
6. **What the perturbation does to the displacement (estimate for c3).**
   - **Weighted mean, exact.** With `f_c` the consistent load and `u_c` the closed form (arithmetic 2) and `u_m = K^{-1} f_m`
     the solution under the mapped load, Betti's theorem (`K` symmetric, the same homogeneous constraints) gives `f_c . u_m =
     f_m . u_c`. On `xmax`, `u_c = (-p L/E, nu p y/E, nu p z/E)` and `f_m` has only x components (the pressure), so `f_m . u_c =
     (-p L/E) sum f_m,x = (-p L/E)(-p A) = f_c . u_c`. Hence the mean of `u_x` over `xmax` weighted by the consistent load,
     `sum_i f_c,i u_m,x,i / sum_i f_c,i`, equals `-p L/E` up to rounding. Limit **1e-10** relative (as c2).
   - **On the loaded face, by an estimate.** The local response to `delta f`, from memory: a point load `P` on an elastic
     half-space displaces the surface at distance `r` by `(1 - nu^2) P/(pi E r)` (Boussinesq), and a uniform load `P` on a
     square of side `b` by about `1.12 (1 - nu^2) P/(E b)` at its centre. Node 00 lies on the two roller planes, which are
     planes of symmetry: mirrored, it is the centre of a `2W x 2W` face loaded by `4 x 0.039 = 0.157 p A` on its tributary
     square `h^2`, with the mirrored edge-mid loads (`-0.0278 p A` each, four at `r = h`), centre loads (`-0.0278` each, four at
     `r = h sqrt 2`), the far corner 22 (`+0.157` at `r = 2 sqrt 2 h`), the nodes 21, 12 (`-0.0139` each, eight at `r = sqrt 5
     h`) and 20, 02 (negligible). With `1 - nu^2 = 0.91`, `h = 0.5`, `A = 1`: `0.320 - 0.064 - 0.046 + 0.032 - 0.029 + 0.005 =
     0.22 p/E`, i.e. `0.055 p L/E`. Node 22 is a free corner (a quarter-space, no mirror): about twice as flexible, so up to
     about `0.11 p L/E`. The finite-element node is stiffer than the continuum under a load spread over `h^2`, so these are, if
     anything, high. Estimate of the largest error on `xmax`, over every node and component, relative to `p L/E`: **0.05 to
     0.11**. Limit **0.25** (a factor 2.3 over the high end, for the crude half-space model). Control (the case is really
     non-matching): the same quantity **>= 0.005** (a tenth of the low end).
   - **Away from the face, by Saint-Venant.** Self-equilibrated end loads of a strip of width `2c` decay as `exp(-Re(z_1) x/c)`
     with `z_1 = 2.106 + 1.125 i` the first root of `sin 2z + 2z = 0` (Papkovich-Fadle, from memory). Mirrored at the rollers
     the section is `2W` wide, `c = W`. At the nodes with `x <= L/2` (distance `>= 2W` from `xmax`): `exp(-4.2) = 0.015` of the
     face error, i.e. `<= 0.0017 p L/E`. Limit **0.01** (a factor 6, for the strip model of a square section and a coarse mesh
     of two elements across).
7. **Balance on more general data (c1).** Besides the two uniform cases, the 3:2 wall with face forces of random direction
   (each component uniform in `[-1e3, 1e3] N`, SplitMix64 seed `0xF385_0286`), on the Hex8 bar and on the same box in Tet4
   (`tet_box`, surface `xmax` of 8 triangles), moment about the origin (arms up to 4.2 m). The rollers hold every rigid
   motion, so tangential loads are taken.
8. **Coupled fixed point (c5).** Mock fluid (test code): the 4 matching wall faces of arithmetic 3, a state of the point
   displacements and a time; its force on face `f` is `F_f = (A_f/A)(F0 - k_f ubar) e_x`, `ubar = sum_f A_f d_f,x / A` the
   area-weighted mean x displacement of its faces (from the protocol's `face_displacements`), at its current state (`forces`)
   or after `advance`. The solid under the uniform end load `P e_x` (consistent by arithmetic 3) has `u_x(L) = P L/(E A)` at
   every node of `xmax` (arithmetic 2, uniaxial), so `k_s = E A/L = 5e10 N/m` exactly. (The stub's `nu = 0` is not needed:
   with the rollers the lateral contraction is free and `k_s = E A/L` for any `nu`; `nu = 0.3` is kept for all checks.) Chosen `k_f = 1.5 k_s = 7.5e10 N/m`,
   `F0 = 1e6 N`: `u* = F0/(k_s + k_f) = 8e-6 m`. The force iterate stays proportional to the uniform distribution, so the
   iteration is scalar: `g(f) = F0 - a f`, `a = k_f/k_s = 1.5`; the plain fixed point multiplies the residual by `-1.5` per
   iteration (diverges, the control C3), Aitken's factor after two iterations is the secant of a linear map (exact), and the
   least-squares update with one column is exact as well, so both converge in **3 iterations** (the third with a residual of
   rounding size). At stop, `|r| <= tol |ft|` and `r = F0 - (1 + a) f` give `|u - u*|/u* <= tol/(1 + a)`; with the coupler
   tolerance `1e-10` (chosen here, below the acceptance limit) that is `<= 4e-11`, and rounding `~1e-14`. Limit **1e-8**
   (acceptance), measured as the largest `|u_x - u*|/u*` over the 9 interface nodes and the 4 wall faces. Settings:
   `CouplingSettings::aitken(0.5)` and `CouplingSettings::least_squares(0.5)` with `tolerance = 1e-10`, cap 20, `AtCap::Stop`;
   `dt = 1e-3 s` (a static solve ignores it except for the time).
9. **Added mass (from the stub, stated).** A static structure has no mass. Against an incompressible flow the added mass of
   the wall then has the ratio `m_a/m_s = infinity` and the coupled iteration's slopes (`-lambda_j/2`, ADR 0256 C1) are
   unbounded: **`StaticSolid` is for mock fluids and for quasi-static use** (a fluid whose force depends on the wall position,
   not on its acceleration), which this ADR checks; a flow solver is not coupled to it (c7).

## Design

### D1. `rcfd_post::solid_coupling::SolidInterface`

The interface nodes of a surface set of a `SolidModel`: `SolidInterface::new(model, surface_set)` takes
`SolidMesh::surface_set_nodes` (ascending, unique), their coordinates, and as interface elements the corner nodes of each
face of the set (the first `face_corners` of `face_nodes`, in interface numbering: Tri3, Quad4, and the corners of a Tri6,
whose mid-edge nodes are interface nodes of weight 0 under the projection map). Refuses an unknown set, naming it.
`build_map(points, face_points, settings)` is `InterfaceMap::build` on these nodes and elements (convenience: a caller may give
any `InterfaceMap` of `n_nodes()` nodes). `nodal_forces(forces)` turns one force per interface node into `NodalForceAt` on the
mesh nodes; `displacements(result)` reads the interface nodes' displacements from a `SolidResult`.

### D2. The in-process route

`face_forces_to_nodes(interface, map, face_forces)`: checks `map.n_nodes() == interface.n_nodes()`, then
`map.forces_to_nodes`. `solve_face_forces(model, interface, map, face_forces, options)`: the nodal forces of the faces as
`NodalForceAt`, `solve_static_with(model, &[], forces, options)`, the model's own constraints and loads unchanged. Errors are
strings (as in `cosim`), the `FemError` text kept.

### D3. `StaticSolid`

`StaticSolid::new(name, model, surface_set, options)` (model validated: every element has one material). State: time,
the full displacement vector (zeros at time 0), the interface forces of the last advance; the checkpoint holds the three.
`Participant`: `name`, `time`, `checkpoint` (replaces), `restore` (copies back; error without a checkpoint).
`Structure::node_coordinates`: the interface coordinates. `Structure::advance(dt, forces)`: refuses a wrong length, a
non-finite force or a non-positive `dt` (naming the participant), solves statically with the forces as nodal loads, and only
then updates the state (time `+ dt`, displacement, forces); a failed solve returns the error (`static solid '<name>': <FemError
text>`) with the state unchanged. Returns the interface displacements. `options` is a public field.

### D4. Tests in `crates/rcfd-post/tests/sb_solid_coupling.rs`

Fluid-side sums (resultant, moment with `c_f`, work with the map's face displacements) and the closed forms written in the
test, independent of the library. The mock fluid `LinearWall` (arithmetic 8) and a test-only wrapper that switches the
static solid's options to a failing CG before its second advance (the failing solve itself is the library's).

## Checks

| Check | Clause | Quantity | Limit | Estimate |
|---|---|---|---|---|
| B1 | c1 | matching wall, uniform pressure, Hex8: resultant, moment (about the origin), work, each relative to its scale (arithmetic 4) | `<= 1e-12` each | `~1e-15` |
| B2 | c1 | 3:2 wall, uniform pressure, Hex8: as B1 | `<= 1e-12` each | `~1e-15` |
| B3 | c1 | 3:2 wall, random face forces, Hex8: as B1 | `<= 1e-12` each | `~1e-15` |
| B4 | c1 | 3:2 wall, random face forces, Tet4 box: as B1 | `<= 1e-12` each | `~1e-15` |
| U1 | c2 | matching wall, uniform pressure: largest `|u_h - u|` over every node and component, and over the interface displacements, relative to `p L/E` | `<= 1e-10` | `<= 1e-13` |
| U2a | c3 | 3:2 wall: mean of `u_x` on `xmax` weighted by the consistent load, against `-p L/E`, relative | `<= 1e-10` | rounding (Betti, exact) |
| U2b | c3 | 3:2 wall: largest error on the nodes of `xmax`, every component, relative to `p L/E` | `<= 0.25` | 0.05 to 0.11 |
| U2c | c3 | 3:2 wall: largest error on the nodes with `x <= L/2`, relative to `p L/E` | `<= 0.01` | `<= 0.0017` |
| U2d | c3, control | U2b's quantity | `>= 0.005` | 0.05 to 0.11 |
| S1 | c4 | checkpoint after one advance, advance with other forces, restore: time and every displacement value bit-equal to the checkpoint; an advance from it repeats the first advance from it bit for bit | 0 bits differ | 0 |
| S2 | c4 | advance: the returned interface displacements bit-equal to `solve_static_with` of the same nodal forces; time `t0 + dt` exactly | bit-equal | bit-equal |
| C1 | c5 | Aitken (`aitken(0.5)`, tolerance 1e-10): converged; largest `|u_x - u*|/u*` over the interface nodes and the wall faces | converged, `<= 1e-8` | `<= 4e-11`, 3 iterations |
| C2 | c5 | least squares (`least_squares(0.5)`, tolerance 1e-10): as C1 | converged, `<= 1e-8` | `<= 4e-11`, 3 iterations |
| C3 | c5, control | `Relaxation::Fixed(1.0)`, cap 20, `AtCap::Stop`: the step fails with participant `coupler` | fails | residual x 1.5 per iteration |
| F1 | c6 | after one converged step, a second with the solid's options `ConjugateGradient { 1e-14, 1 }`: error with participant = the solid's name (`solid`), iteration 1, the solver's text in the message; the fluid's and the solid's state (time, displacements, forces) bit-equal to before the step | as stated | as stated |
| F2 | c6 | as F1 but the solve fails in iteration 2 (after the fluid has advanced once in the step): participant `solid`, iteration 2, both states bit-equal to before the step | as stated | as stated |
| F3 | c6 | direct `advance` with a failing solve, with a wrong force count and with `dt = 0`: an error naming the participant, the state bit-equal to before | as stated | as stated |
| X1 | extension | matching wall: the mapped nodal forces against the consistent load of `Load::pressure("xmax", p)` (`loads::nodal_load_vector`), largest difference relative to the largest entry | `<= 1e-14` | `~1e-16` |
| X2 | extension | iterations of C1 and C2 | `<= 6` each | 3 |

Clause c7 holds by construction: the test file uses a mock fluid only, and nothing in the library couples a flow solver.

### Negative controls

U2d and C3 above. The mutations below are run once each on a scratch copy.

### Planned mutations (scratch copy, each run once)

| Id | Where | Change | Expected to catch it |
|---|---|---|---|
| M1 | `SolidInterface::nodal_forces` | the force of interface node `i` put on interface node `(i + 1) mod n` | B1 to B4 (moment), U1 |
| M2 | `SolidInterface::nodal_forces` | the z component dropped | B3, B4 (resultant) |
| M3 | `SolidInterface::displacements` | y and z components swapped | U1 (interface part) |
| M4 | `StaticSolid::restore` | the displacement not restored (time and forces only) | S1, F2 |
| M5 | `StaticSolid::advance` | the time not advanced | S2, C1, C2 (the coupler's time check) |
| M6 | `StaticSolid::advance` | the time advanced before the solve (state changed on a failed solve) | F3 |
| M7 | `StaticSolid::checkpoint` | keeps the first checkpoint (no replacement) | S1, F1, F2 |
| M8 | `StaticSolid::advance` | solves with half the forces | S2, C1, C2 |
| M9 | `Participant::name` of `StaticSolid` | returns `structure` instead of the given name | F1, F2, F3 |

## Decision rule

**done**: every check of clauses c1 to c6 (B1 to B4, U1, U2a to U2d, S1, S2, C1 to C3, F1 to F3) met on its registered run,
and every planned mutation caught by the registered tests. A miss among the extensions (X1, X2) is a `*_known_miss` record
and does not alone block done, unless its diagnosis shows an acceptance check rests on a defect. **partial** (Rule I): a
missed or unrun check of c1 to c6, or a mutation that survives or is caught only after a post-run amendment. **todo**: the
test file does not build.

## Limits (what is not covered)

- A static structure only: no mass, no damping, no time integration; the added-mass ratio against a real flow is unbounded
  (arithmetic 9), so it is for mocks and quasi-static use. The dynamic participant is the intermediate tier's.
- No flow solver is coupled (c7); `FlowWall` is not used.
- The closed forms are checked on a Hex8 bar with a planar interface; the Tet4 box is checked for balance only. Curved
  interfaces, Tet10 (whose mid-edge nodes get no weight from the corner-element projection, so the mapped load is not the
  consistent quadratic one) and the radial-basis map are not checked.
- The non-matching limit U2b is an order-of-magnitude estimate from continuum formulas cited from memory, not a bound.
- No transport: the participant is in process; serving it over `cosim_service` is a later item.

## The planned stub, as written

# ADR 0286: One-way coupling 2: fluid face forces in process, and a static solid as a co-simulation participant

Status: **PLANNED 2026-11-12** (layout by the basic-tier plan, [ADR 0270](0270-structural-solver-basic-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are
the acceptance draft that the registration turns into checks.

Work package SB-16 of the structural solver, basic tier. Size M.

## Goal

The in-process route: fluid face forces, as `cosim::Fluid::forces` returns them, mapped to the nodes of a solid surface set by a
given `cosim::InterfaceMap` and solved statically. On the same code a `StaticSolid` implements `cosim::Structure` (advance =
static solve; checkpoint and restore), which fixes the interface the intermediate tier's dynamic participant will use. It is
checked against a mock fluid only; coupling to the flow solver is intermediate.

## Dependencies

ADR 0275, ADR 0278; existing roadmap items: F-139.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. Fluid face forces on a wall are mapped to the nodes of a
solid surface set by a given InterfaceMap and solved statically: resultant force, moment and work equal the fluid side's within
1e-12; for a matching planar interface under uniform pressure the solid's displacement equals the uniaxial closed form within
1e-10, and for a 3:2 non-matching one within a limit registered from the estimate of the mapping error. A StaticSolid implements
cosim::Structure (advance is a static solve; restore returns the checkpoint bit for bit) and, with a test-only mock fluid whose
force falls linearly with the wall displacement (stiffness k_f), coupled_step converges to u = F0/(k_s + k_f) within 1e-8 with
the Aitken and with the least-squares update; a failing solve stops the step with the participant's name and both sides
restored. No flow solver is coupled.

## Reference to check against

Closed form: two springs in parallel, `u = F0/(k_s + k_f)` with `k_s = E A/L` of a bar (nu = 0). The protocol of
`docs/cosimulation-protocol.md` (ADR 0256) and the update of ADR 0261.

## Files

`crates/rcfd-post/src/solid_coupling.rs`, `crates/rcfd-post/tests/sb_solid_coupling.rs`.

## What could make it miss

A static structure against a real incompressible flow has no structural mass, so the added-mass ratio is unbounded: this
participant is for the mock and for quasi-static use, and the ADR must say so. Whether the participant belongs in `rcfd-post` or
in a later coupling crate is decision M5.
