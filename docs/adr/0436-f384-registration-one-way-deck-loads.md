# ADR 0436: F-384 registration: loads of the F-237 deck applied to the solid surface (one-way coupling 1)

Status: **REGISTERED 2026-11-14, written before any code of F-384 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-384 was written, compiled or run: the code and documents under "What was read" were
read, and the arithmetic under "Arithmetic before this registration" was done with one disclosed python script (no library
code run). Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parent: [ADR 0285](0285-one-way-coupling-from-the-load-deck.md) (the planned stub, SB-15 of the basic tier,
[ADR 0270](0270-structural-solver-basic-tier-plan.md)). Roadmap item F-384 (depends on F-371, F-377, F-380, F-237). Size M.
Branch `f384-one-way-deck-loads` from main 041d286f, worktree `w49i-work/f384`, scratch `w49i-work/f384-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here before the code and the runs and not changed after a run (a miss
becomes an ignored `*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical: the only
library file edited is the stub `crates/rcfd-post/src/solid_loads.rs` (new items only; no existing function of any crate is
changed, so the F-237 export and the deck writer and reader produce exactly the bytes they produced before: check S1); no edit
of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited; no new dependency
(`rcfd-post` already depends on `rcfd-fem`, ADR 0278), no unsafe code; clean-room (first principles, derived below). The files
of `rcfd-fem` that another branch edits in parallel (`mpc.rs`, `loads.rs`, `free_motions.rs`) are not edited; only public
items of `rcfd-fem` are called.

## What F-384 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. A surface set of a solid mesh is turned into the shell
> surface that the F-237 export writes onto, and a deck read back (PLOAD4, TEMP) is applied to the solid: the resultant force
> and moment of the nodal loads equal deck_force_moment within 1e-12 relative; a deck element or node that matches no solid
> face or node within a stated tolerance is refused with its id. Tube: the wall pressure of the fluid mesh of the F-237 tube
> check, uniform, exported, written, parsed and applied to a thick solid tube (b/a = 2, plane strain) gives the radial
> displacement at the bore within 1 percent and the hoop stress within 2 percent of Lame's solution; with a pressure varying
> along the axis and around the circumference the solid's reactions equal the fluid's integrated force within 0.1 percent (the
> F-237 limit); TEMP of a uniform temperature gives the free expansion alpha dT r within 1e-6.

The roadmap note adds (ADR 0281, 2026-11-12): the `TEMP` values of the deck map to `Load::temperature(nodal, reference)` with
one value per solid node (a model takes one temperature field; a second is refused); the expansion coefficient comes from
`Material::isotropic_elastic`.

Clauses: (c1) a surface set of a solid mesh gives the shell surface the F-237 export writes onto; (c2) a deck read back
(`PLOAD4`, `TEMP`) is applied to the solid; (c3) the resultant force and moment of the nodal loads equal `deck_force_moment`
within 1e-12 relative; (c4) a deck element that matches no solid face, or a deck node that matches no solid node, within a
stated tolerance is refused with its id; (c5) tube, uniform wall pressure of the F-237 fluid mesh through export, text, parse
and application, thick tube `b/a = 2`, plane strain: bore radial displacement within 1 percent of Lame; (c6) the same, hoop
stress within 2 percent; (c7) pressure varying along the axis and around the circumference: the solid's reactions equal the
fluid's integrated force within 0.1 percent; (c8) `TEMP` of a uniform temperature: free expansion `alpha dT r` within 1e-6;
(c9) one temperature value per solid node, a second temperature field refused, the expansion coefficient from
`Material::isotropic_elastic`.

## What was read (main 041d286f, read only)

- ADR 0285 (the stub), ADR 0244 (F-237 registration and addendum: the tube check, `annulus_3d(96, 1, 24, 0.02, 0.05, 1, 0)`,
  patch `outer`, shell `shell_tube(0.05, 0, 1, 64, 20)`, measured force error `1.552e-4 S`), ADR 0433 (F-379: Lame's plane
  strain cylinder, its derivation D6, its error estimates D8 and the measured Hex8 numbers of its addendum), ADR 0281 (thermal
  strain; the free-expansion checks), ADR 0278 (the stub module `solid_loads`).
- Code: `crates/rcfd-post/src/structural.rs` (all: `ShellSurface`, `overlap_weights`, `map_face_field`, `patch_rects_*`,
  `shell_tube`, `quad_geom`, `fluid_force_moment`, `deck_force_moment`, `shell_deck`, `temperature_deck`),
  `crates/rcfd-io/src/nastran.rs` lines 1 to 140 (`Deck`, `Element`, `Pload4`, the writer), `crates/rcfd-post/tests/
  w27c_structural.rs` lines 1 to 140 (the R3, R4 set-ups), `crates/rcfd-fem/src/loads.rs` (`nodal_load_vector`,
  `add_face_load`, `face_shape_functions`, `face_rule`), `model.rs` (`Load`, `add_load`, `SolidResult`), `mesh.rs`
  (`face_nodes`, `face_area_vector`, `face_incidence`), `convert.rs` (`from_mesh`), `thermal.rs` (`temperature_field`: the
  second field is refused), `solve.rs` (`solve_static_with`, reactions `K u - f`), `rcfd-mesh` `annulus_3d`, `Grading`.
- Scaffold rows: `grep "F-384\|0285"` over `crates` and `docs` finds no test or scaffold row of this item (only the stub's
  module comment and ADR texts). Nothing to retire.

## Arithmetic before this registration (disclosed)

Script `w49i-work/f384-scratch/pre_arith.py` (numpy, no repository code), output:

- Radial grading `Grading::Expansion(7)` over 32 cells of `b - a = 0.05`: first cell `dr_1 = 5.019e-4` (`dr_1/a = 1.004e-2`),
  last `3.51e-3`. This is ADR 0433's `beta = 2` map to within a few percent (`r'(0)/n_r = 0.313/32 (b - a)`, i.e. `dr_1/a =
  9.8e-3` there).
- Lame (D6 of ADR 0433) at `a = 0.05`, `b = 0.1`, `nu = 0.3`, `E = 2e11`, `p = 1e5`: `u_r(a) = 4.76667e-8 m`, `sigma_theta(a)
  = 5p/3`; plane stress over plane strain at the bore 1.03147.
- Bore hoop error estimate `0.587 dr_1/a = 5.9e-3` (coefficient 0.587 = ADR 0433's measured `5.75e-3` over its `dr_1/a`);
  chord term of `u_r` for 128 chords `(2 pi/128)^2/12 = 2.0e-4`.
- A bilinear quad `(0,0,0), (1,0,0), (1.3,1,d), (0,1,0)` under unit pressure: the 3 x 3 Gauss integral of `-n dS` equals
  `quad_geom`'s `-A` to `4.4e-16` for every `d`; the moment `int x x (-n) dS` against `c x (-A)` with `quad_geom`'s centroid
  differs by `2.2e-16` at `d = 0` and by `2.17e-4` at `d = 0.01` (`|A| = 1.15`), `2.17e-3` at `d = 0.1`: linear in the warp.
- F-237 tube pressure `1e5 (1 + 0.5 cos theta + 0.3 z)` on `r = 0.05`, `z in [0, 1]`: `S = int |p| dA = 3.6128e4 N`, `F_x =
  7.854e3 N`, `F_x / S = 0.217`.

## Design

### D1. The shell surface of a surface set (clause c1)

`rcfd_post::solid_loads::shell_from_surface_set(mesh: &SolidMesh, set: &str, param: SurfaceParam) -> Result<SolidShell,
String>`, `SurfaceParam::{Plane { axes: [usize; 2] }, CylinderZ}` (the two parametrisations of F-237: `(x_i, x_j)` and `(theta,
z)` about the z axis), `SolidShell { surface: ShellSurface, solid_nodes: Vec<usize> }`:

- the shell's nodes are the nodes of the set's faces, sorted by solid node index, at the solid's coordinates (same nodes);
- one quad per face of the set, in the set's order, with the face's corner nodes in the local face order. The local face order
  of `rcfd-fem` gives the outward normal of the solid (ADR 0271; `face_area_vector`), and the solid lies on the side of the
  wall away from the fluid, so the outward normal points toward the fluid: the F-237 sign convention (`ShellSurface`, "normal
  toward the fluid"; force `-p A n_shell`) holds without a flip;
- the parameter rectangle of each quad from its corners, as `patch_rects_plane` / `patch_rects_cylinder` do for the fluid
  (cylinder: the angle of each corner taken relative to the face centre's angle, so a face across `theta = pi` is one interval);
- refused: an unknown set, an empty set, a face that is not a 4-node face (Tri3 of Tet4, Tri6 of Tet10, the 8-node face of
  Hex20: F-237's `deck_force_moment` sums `CQUAD4` only, and the deck has no 6- or 8-node shell), each naming the element and
  local face; a face with zero or invalid parameter area naming it.

The export is F-237's, unchanged: `overlap_weights(fluid rects, shell.rects)`, `map_face_field`, `shell_deck`,
`Deck::to_string_checked`, `Deck::parse`.

### D2. Matching deck to solid (clauses c2, c4)

Tolerance: an absolute distance `tol > 0` (a non-positive or non-finite value is refused). A deck grid matches the solid node
within `tol` of it; it is refused (error text names `grid <id>`) when no solid node of the candidate set is within `tol`, and
when two are (ambiguous). Candidate set: the nodes of the surface set for pressures, every node of the mesh for temperatures.
The search sorts the candidates by `x` and scans the window `|x - x_g| <= tol` (exact, no hashing).

Registered tolerance of every check below: `tol = 1e-9 m` (the models measure 0.1 m to 1 m; the solid and the deck carry the
same coordinates bit for bit in the chain, written with the round-trip format `{:?}`; `1e-9 m` is far above that rounding,
zero, and far below the smallest element edge, `5e-4 m`).

### D3. Pressures (clauses c2, c3)

`deck_pressure_loads(mesh, set, deck, sid, tol) -> Result<Vec<NodalForceAt>, String>`: for each `PLOAD4` card of set `sid`,
in deck order:

1. the element must exist in the deck and be a `CQUAD4` (else refused naming `element <id>`); each of its grids must exist and
   match a node of the set (D2; else refused naming the grid);
2. the four matched nodes, sorted, must be the corners of a face of the set (else refused naming `element <id>`): a deck
   element that matches no solid face;
3. the pressure must be uniform: `P2..P4` blank or equal to `P1` (a corner-varying card is refused naming the element: its
   consistent load is not `deck_force_moment`'s mean pressure times the area vector);
4. orientation: the deck element's area vector (deck coordinates, `quad_geom`'s `A`) must have a positive dot product with the
   face's outward `face_area_vector`; otherwise the card would pull on the solid, and it is refused naming the element;
5. the consistent nodal loads of the uniform pressure on the face, `f_a = -p int N_a (x_u x x_v) du dv` with the face's own
   functions and rule (`face_shape_functions`, `face_rule`, the same arithmetic as `rcfd-fem`'s `Load::pressure`).

The forces are returned one per node of the set in node order (a node of no loaded face gets none) for
`solve_static_with(model, &[], &forces, options)`.

Why the resultant equals `deck_force_moment` (c3), first principles: on a bilinear face `x(u, v) = sum_a N_a x_a`, `sum_a N_a =
1`, so `sum_a f_a = -p int (x_u x x_v) = -p A`, and `A = (x_2 - x_0) x (x_3 - x_1)/2` exactly for a bilinear patch (planar or
not), which is `quad_geom`'s area vector; the integrand is of degree 2 per variable, integrated exactly by the 3 x 3 rule. The
moment `sum_a x_a x f_a = -p int x(u, v) x (x_u x x_v)` is the moment of the uniform pressure on the patch; for a planar face it
is `c x (-p A)` with `c` the area centroid, which is `quad_geom`'s centroid (area-weighted triangles). On a warped face it is not
(arithmetic above: linear in the warp): not covered, the negative control N2 shows it. All faces of the checks are planar (hex
box faces; chord rectangles of `annulus_3d`). The two sides then agree to rounding.

`nodal_force_moment(mesh, forces, about) -> (Vec3, Vec3)`: `sum f`, `sum (x - about) x f`.

### D4. Temperatures (clauses c2, c8, c9)

`deck_temperature_load(mesh, deck, sid, reference, tol) -> Result<Load, String>`: every `TEMP` value of set `sid` goes to the
solid node its grid matches (D2, over all nodes; a `TEMP` grid missing from the deck's grids is refused naming it); a solid node
reached twice is refused naming the second grid; a solid node not reached is refused naming the node (index). The result is
`Load::temperature(nodal, reference)` with one value per solid node. A second temperature field is refused by the model
(`thermal::temperature_field`, ADR 0281) when the solve assembles the loads; the expansion coefficient is the material's.

### D5. The tube models (clauses c5 to c8)

Material `E = 2e11 Pa`, `nu = 0.3`, `rho = 7850`, `alpha = 1.2e-5 1/K` (`Material::isotropic_elastic`). Fluid mesh of the F-237
tube check: `annulus_3d(96, 1, 24, 0.02, 0.05, 1.0, 0.0, Uniform)`, patch `outer` (`r = 0.05`). The solids are `annulus_3d`
meshes about the same axis with `r_in = a = 0.05`, `r_out = b = 0.1` (`b/a = 2`), converted by `convert::from_mesh` (Hex8; sets
`inner`, `outer`, `zmin`, `zmax`, `all`); surface set `inner` is the bore, whose outward normal points to the axis, toward the
fluid.

- **U (uniform, c5, c6):** `annulus_3d(128, 32, 1, 0.05, 0.1, 0.01, 0.5, Expansion(7))`, a slice `z in [0.5, 0.51]` (plane
  strain makes the solution independent of `z`; the 128 x 1 shell lies inside one row of fluid faces, so `overlap_weights`
  covers it). Fluid pressure uniform `p0 = 1e5 Pa` on every face of `outer`. Constraints: plane strain `u_z = 0` at every node
  (`all`); symmetry rollers `u_y = 0` on the nodes with `|y| <= 1e-12` (the planes `theta = 0, pi`) and `u_x = 0` on the nodes
  with `|x| <= 1e-12` (`theta = pi/2, 3 pi/2`); Lame's field satisfies both, and they remove the in-plane rigid motions. 8448
  nodes. Measures as ADR 0433 D7: `u_r = (u_x x + u_y y)/r` at every node of `inner`, largest `|u_r/u_r(a) - 1|`; hoop stress
  `t^T sigma t`, `t = (-sin theta, cos theta, 0)`, of `nodal_average(None)` at every bore node, largest `|sigma_theta/(5p/3) -
  1|`.
- **V (varying, c3, c7):** `annulus_3d(64, 4, 20, 0.05, 0.1, 1.0, 0.0, Expansion(7))`, `z in [0, 1]`, covering the whole fluid
  wall; its bore shell is the 64 x 20 shell of F-237 (check K1). Fluid pressure `1e5 (1 + 0.5 cos theta + 0.3 z)` at the face
  centres (F-237's R4 field). Constraints as U. Measure: `F_R = sum` of the reactions over all unknowns (components x, y, z)
  against the fluid's `fluid_force_moment(fluid, "outer", p, about)` force: `|F_R + F_fluid| / S`, `S = sum |p_f| |S_f|` (F-237's
  measure; the reactions balance the load, so they are minus the force on the wall). `about = (0, 0, 0.5)`.
- **T (temperature, c8):** the mesh of U, constraints `u_z = 0` on `zmin` (not plane strain: the expansion is free) and the
  rollers of U. `TEMP` of the uniform `T = 393.15 K` at every node, written by F-237's `temperature_deck(structure, &T, 5)` on
  the `rcfd_mesh::Mesh` the solid was converted from, as text, parsed, mapped by D4 with `reference = 293.15 K` (`dT = 100 K`).
  Measures: largest over all nodes of `|u_r/(alpha dT r) - 1|` and of `|u_z - alpha dT (z - 0.5)| / (alpha dT H)`, `H = 0.01`.

### D6. Expected numbers

- c3: the two sides are sums of the same terms up to rounding (D3); about 1e-15 relative. Limit 1e-12 (the acceptance).
- c5: ADR 0433 measured on Hex8 32 x 64 per quarter (`dr_1/a = 9.8e-3`) a bore error 1.83e-4; here 128 chords on the full
  circle (32 per quarter), chord term `2.0e-4`; expected 2e-4 to 5e-4. Limit 1e-2 (the acceptance).
- c6: first order in `dr_1`, `0.587 dr_1/a = 5.9e-3` (ADR 0433's 16 x 32 quarter measured 1.12e-2 against the formula's 1.15e-2,
  so the chord count does not change it). Limit 2e-2 (the acceptance).
- c7: the reactions equal minus the nodal loads (equilibrium of `K u = f + R` summed over a rigid translation, to the direct
  solve's residual), the nodal loads equal the deck force (c3), the deck force is F-237's (same shell, same mapping): `1.55e-4`.
  Limit 1e-3 (the acceptance, F-237's limit).
- c8: a uniform `dT` with free boundaries has the exact discrete solution `u = alpha dT (x - x_0)` (linear, representable, the
  thermal load of ADR 0281 is in discrete equilibrium with it): rounding of the direct solve, about 1e-10 or below. Limit 1e-6
  (the acceptance).

## Registered checks

All in `crates/rcfd-post/tests/sb_deck_loads.rs` (the file named by ADR 0285), release profile, run once (`cargo test -p
rcfd-post --release --test sb_deck_loads -- --nocapture --test-threads=1`). `tol = 1e-9 m` throughout. `S_d = sum_e |p_e|
|A_e|` over the deck's loaded elements, `L_d = max |x_g - about|` over the deck's grids.

### A. The acceptance

| Id | Clause | Measure | Limit | Estimate |
| --- | --- | --- | --- | --- |
| A1 | c1, c2, c3 | three decks through text: plane (fluid `box_mesh([8, 8, 1], [1; 3])` patch `zmax`, `p = 1 + 2x + 3y`, onto the `zmin` set of `hex_box([0, 0, 1], [1, 1, 0.2], [5, 7, 2])`, plane axes `[0, 1]`), tube U, tube V: `|F_n - F_d| / S_d` and `|M_n - M_d| / (S_d L_d)` with `(F_n, M_n) = nodal_force_moment`, `(F_d, M_d) = deck_force_moment` of the parsed deck, `about` `(0.5, 0.5, 1)` for the plane, `(0, 0, 0.5)` for the tubes | each <= 1e-12 | about 1e-15 |
| A1b | c3 | plane and tube V (where `|F_d| >= 0.1 S_d`): `|F_n - F_d| / |F_d|` | each <= 1e-12 | about 1e-15 |
| A2 | c4 | refusals, on the parsed V deck: (a) one grid of a loaded element moved by `2 tol` along x: `Err` naming `grid <id>`; (b) the same moved by `tol/2`: accepted; (c) one loaded element's fourth grid replaced by the id of a grid not on its face (a grid of the next ring): `Err` naming `element <id>`; (d) the U temperature deck with one grid moved by `2 tol`: `Err` naming `grid <id>`; (e) that deck with one `TEMP` entry removed: `Err` naming the solid node; (f) one `TEMP` entry duplicated: `Err` naming the grid | as stated | D2, D4 |
| A3 | c5 | tube U: largest `abs(u_r/u_r(a) - 1)` over `inner` | <= 1e-2 | 2e-4 to 5e-4 |
| A4 | c6 | tube U: largest `abs(sigma_theta/(5 p0/3) - 1)` over `inner` | <= 2e-2 | about 5.9e-3 |
| A5 | c7 | tube V: `|F_R + F_fluid| / S` | <= 1e-3 | 1.55e-4 |
| A6 | c8 | tube T: largest `abs(u_r/(alpha dT r) - 1)` over all nodes; largest `abs(u_z - alpha dT (z - 0.5)) / (alpha dT H)` | each <= 1e-6 | 1e-10 or below |

### K. The set-ups themselves

| Id | Measure | Limit | Estimate |
| --- | --- | --- | --- |
| K1 | V: the shell of `inner` has the 1344 nodes of `shell_tube(0.05, 0, 1, 64, 20)`, each within `1e-14 m` of a distinct one, and 1280 quads whose area vectors all point to the axis (`A . (x_c, y_c, 0) < 0`) | as stated | the same formulas |
| K2 | V: the parsed deck against the fluid: `|F_d - F_fluid| / S <= 1e-3` and `|M_d - M_fluid| / (S R) <= 5e-3`, `R = 0.05` (F-237's R4 limits); the reactions' moment `sum (x - about) x R` against `-M_fluid`: `<= 5e-3` | as stated | 1.552e-4 (ADR 0244 H7), moment within R4's limit |
| K3 | U: the nodal loads of the deck against `nodal_load_vector` of a model with `Load::pressure("inner", p0)` on the same mesh: `max |df| / max |f|` | <= 1e-14 | identical arithmetic, 0 |
| K4 | U: every element pressure of the parsed deck equals `p0` exactly (F-237's R3 property) | exact | R3 |
| K5 | the mesh of U: `TEMP` of the linear field `T = 300 + 1000 x - 500 y + 200 z` written by `temperature_deck`, the deck's grids reversed in order and every grid id (and `TEMP` grid) offset by 1000, through text: every solid node's value bit-identical (`to_bits`) to the written one | exact | the match is by coordinates |
| K6 | V: equilibrium of the solve, `|F_R + F_n| / S` | <= 1e-9 | direct-solve residual |

### N. Negative controls

| Id | Control | Expected |
| --- | --- | --- |
| N1 | tube U with `u_z = 0` on `zmin` only (plane stress): A3's measure against plane-strain Lame | `> 2e-2` (about 3.15e-2) |
| N2 | one Hex8 `hex_box([0, 0, 1], [1, 1, 0.2], [1, 1, 1])` with the node at `(1, 1, 1)` moved to `z = 1.01` (the `zmin` face warped), `p = 1e5`: A1's measures | force `<= 1e-12`; moment `> 1e-6` (order 1e-4: arithmetic above) |
| N3 | the V shell with every quad's node order reversed (normal into the solid), deck written and parsed: `deck_pressure_loads` | `Err` naming an element (orientation) |
| N4 | U model with the temperature load of the deck added twice: `solve_static` | `Err` (a second temperature field) |
| N5 | `shell_from_surface_set` on the `zmin` set of `tet_box` (Tri3 faces): `Err` naming element and face; a `PLOAD4` with corner pressures `(1, 2, 3, 4)` on one V element: `Err` naming the element | as stated |

### S. Defaults

| Id | Measure | Limit |
| --- | --- | --- |
| S1 | `git diff 041d286f --stat -- crates` lists only `crates/rcfd-post/src/solid_loads.rs` and the new test file; no existing function changed, so the F-237 export and deck outputs are byte-identical by construction | as stated |

### Mutations (scratch copy `w49i-work/f384-scratch/tree` from `git archive`, not committed)

| Id | Mutation | Expected to be caught by |
| --- | --- | --- |
| MU1 | `solid_loads.rs`: the face load `+p` instead of `-p` | A1, A3, A5 |
| MU2 | the node match accepts the nearest node whatever its distance | A2 (a), (d) |
| MU3 | the temperature map uses `grid id - 1` as the node index (no coordinates) | K5, A2 |
| MU4 | the face load lumped: `-p A/4` at each corner | K3 (the resultants of rectangles do not see it) |
| MU5 | `shell_from_surface_set` reverses each quad | A1 (orientation refusal), A3 |
| MU6 | `deck_temperature_load` stores `2 reference - T` at each node (the sign of `dT` flipped) | A6 |

## Decision rule

- **done**: every A and K check met, every negative control as expected, S1 as stated, every mutation caught by at least one
  check (MU4 by K3 counts).
- **partial**: any A or K check missed (kept as a `*_known_miss` record with the limit intact and the measured number, the cause
  diagnosed), or a mutation not caught, or a check met only after an amendment made after its run.
- **todo**: the checks could not be run.

## Limits (what is not covered)

- Only 4-node (bilinear) solid faces against `CQUAD4`: Tet4, Tet10 and Hex20 surfaces are refused (the deck has no 6- or 8-node
  shell, and `deck_force_moment` sums quads only). Planar faces only for the moment equality (N2).
- Uniform element pressures only (`P2..P4` blank or equal); corner-varying `PLOAD4` is refused.
- The mapping from the fluid wall stays F-237's (plane and z-axis cylinder parametrisations); a general geometric mapping is
  F-362. The `TEMP` route is checked with a uniform field and a by-coordinates identity (K5); a temperature remapped from a
  fluid volume overlapping the solid is F-237's R5, not repeated here.
- The `PLOAD4` sign convention is fixed within the repository only (ADR 0051 D8: unverified against a third-party solver).
  `RCFDQ` (heat flux) is not applied: there is no thermal solver of the solid in this tier.
- No case key, no CLI or MCP surface; the functions are a library route.

## Open points taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. Matching by coordinates within an absolute tolerance rather than by ids (the deck's grid ids are the shell's, 1-based, not the
   solid's node indices): taken; ids are unreliable across tools, coordinates are what the export preserves bit for bit.
2. A deck card whose normal points into the solid is refused rather than applied as a pull: taken (it signals a flipped
   surface, and the acceptance's sign convention is "positive presses on the structure").
3. "1e-12 relative" is read as relative to the deck's load scale `S_d` (force) and `S_d L_d` (moment), with the force also
   relative to `|F_d|` where it is not small (A1b): the uniform tube has `F_d = 0` up to rounding, where a ratio to `|F_d|` is
   undefined.

## Amendment 1 (2026-11-14, after the registered run, before the corrected run): the set-up of N2 was defective

The registered run (commit f779af3c, log `w49i-work/f384-scratch/run1.log`) met every A and K check and every negative control
except N2: the warped face gave `|M_n - M_d| / (S_d L_d) = 1.14e-17`, not above `1e-6`. Diagnosis (first principles, no run):
the warped face of N2 is the unit square with one corner raised, `x(u, v) = (u, v, d u v)` on `[0, 1]^2`. Then `x_u x x_v = (-d
v, -d u, 1)`, and `int x x (x_u x x_v)` has the x component `int (v + d^2 u^2 v) = 1/2 + d^2/6`; `quad_geom` gives `A = (-d/2,
-d/2, 1)` and the centroid of the triangles `(0, 1, 2)` and `(0, 2, 3)`, `c = (1/2, 1/2, d/3)`, with `(c x A)_x = 1/2 + d^2/6`:
equal (and the y component by the symmetry `u <-> v`). The face is symmetric about its diagonal through the raised corner, and
for that warp the centroid formula is exact. The pre-registration arithmetic used an asymmetric quad (corner `(1.3, 1, d)`),
which the set-up did not reproduce. The control was defective; the claim it was meant to show (D3: the equality needs a planar
face) is unchanged.

Corrected control **N2a** (new test `n2a_warped_asymmetric_face`; the old test
`negative_controls_warped_and_triangles` stays unchanged as an ignored record, its N5 part met in the registered run): the same
single Hex8, the node at `(1, 1, 1)` moved to `(1.3, 1, 1.01)` (the quad of the arithmetic, on the `zmin` face), `p = 1e5`,
`about = (0.5, 0.5, 1)`. Expected: force `|F_n - F_d| / S_d <= 1e-12`; moment `|M_n - M_d| / (S_d L_d) > 1e-6`, estimate from the
arithmetic `2.17e-4 / (1.15 x 0.943) = 2.0e-4` (`|A| = 1.15`, `L_d = |(0.8, 0.5, 0.01)| = 0.943`; the difference of the moments
does not depend on the reference point because the forces agree). Run once.

By the decision rule a negative control that did not behave as expected in the registered run is not "as expected"; whatever
N2a gives, the proposal is **partial** (a check met only after an amendment made after its run).

## Addendum 1: results (2026-11-14)

The registered run: commit f779af3c, once, release, `cargo test -p rcfd-post --release --test sb_deck_loads -- --nocapture
--test-threads=1` (log `w49i-work/f384-scratch/run1.log`): 4 tests passed, 1 failed (N2), 1.40 s. The corrected control N2a
(amendment 1, commit bae5849d) was run once (log `run2_n2a.log`). Nothing else was re-run except the mutation runs on the
scratch copy.

| Id | Limit | Measured | Met |
| --- | --- | --- | --- |
| A1 plane `|F_n - F_d|/S_d`; `|M_n - M_d|/(S_d L_d)` | each `<= 1e-12` | 1.27e-16; 5.72e-17 | yes |
| A1 tube U | each `<= 1e-12` | 2.93e-16; 2.08e-16 | yes |
| A1 tube V | each `<= 1e-12` | 3.68e-17; 1.82e-16 | yes |
| A1b plane; tube V `|F_n - F_d|/|F_d|` | each `<= 1e-12` | 1.27e-16; 1.69e-16 | yes |
| A2 (a) grid moved `2 tol` | `Err` naming the grid | `deck grid 1 at [0.050000002, 0.0, 0.0] matches no solid node of surface set 'inner' within 1e-9 m` | yes |
| A2 (b) grid moved `tol/2` | accepted | `Ok` (1344 nodal forces) | yes |
| A2 (c) element matching no face | `Err` naming the element | `deck element 1: its grids match solid nodes [0, 320, 325, 1000], which are not the corners of a face ...` | yes |
| A2 (d) `TEMP` grid moved `2 tol` | `Err` naming the grid | `deck grid 1 at [...] matches no solid node of the solid within 1e-9 m` | yes |
| A2 (e) `TEMP` entry missing | `Err` naming the node | `solid node 0 has no TEMP value of set 5` | yes |
| A2 (f) `TEMP` entry duplicated | `Err` naming the grid | `deck grid 1: solid node 0 already has a TEMP value of set 5` | yes |
| A3 tube U bore `u_r` (plane strain) | `<= 1e-2` | 3.20e-4 | yes |
| A4 tube U bore hoop stress | `<= 2e-2` | 5.59e-3 | yes |
| A5 tube V `|F_R + F_fluid|/S` | `<= 1e-3` | 1.552e-4 (`F_R,x = -7846.975 N`, `F_fluid,x = 7852.580 N`, `S = 3.6122e4 N`) | yes |
| A6 tube T `u_r/(alpha dT r)`; `u_z` | each `<= 1e-6` | 9.47e-14; 5.38e-14 | yes |
| K1 shell of `inner` = `shell_tube(0.05, 0, 1, 64, 20)`; normals to the axis | as stated | 1344 nodes, 1280 quads, largest distance 0 m; all inward | yes |
| K2 deck vs fluid force; moment; reactions' moment | `<= 1e-3`; `<= 5e-3`; `<= 5e-3` | 1.552e-4; 1.76e-15; 1.56e-15 | yes |
| K3 nodal loads vs `Load::pressure` | `<= 1e-14` | 0 (bit-identical) | yes |
| K4 element pressures equal `p0` | exact | 128 of 128 | yes |
| K5 `TEMP` reversed and renumbered, bit-identical | exact | every node | yes |
| K6 equilibrium `|F_R + F_n|/S` | `<= 1e-9` | 1.37e-15 | yes |
| N1 plane stress against plane-strain Lame | `> 2e-2` | 3.12e-2 (estimate 3.15e-2) | yes |
| N2 warped face (registered set-up): force; moment | `<= 1e-12`; `> 1e-6` | 7.96e-18; **1.14e-17** | **no** (defective set-up, amendment 1) |
| N2a warped asymmetric face (amendment 1): force; moment | `<= 1e-12`; `> 1e-6` | 1.27e-16; 2.00e-4 (estimate 2.0e-4) | yes |
| N3 flipped shell | `Err` naming an element | `deck element 1: its normal points into the solid (face 1 of element 0)` | yes |
| N4 second temperature field | `Err` | `key 'temperature': a model has one temperature field; a second Load::Temperature was given` | yes |
| N5 Tri3 surface; corner-varying `PLOAD4` | `Err` naming face and element; `Err` naming the element | `face 0 of element 0 in set 'zmin' has 3 nodes ...`; `deck element 1: corner-varying PLOAD4 pressures [1.0, 2.0, 3.0, 4.0] ...` | yes |
| S1 `git diff 041d286f --stat -- crates` | the stub and the new test only | `crates/rcfd-post/src/solid_loads.rs`, `crates/rcfd-post/tests/sb_deck_loads.rs`, nothing else | yes |

Against the estimates: A3 (3.2e-4) inside the estimated 2e-4 to 5e-4, A4 (5.6e-3) at the estimated 5.9e-3, A5 and K2 equal to
F-237's 1.552e-4 to the printed digits (the same shell and mapping), N1 and N2a at their estimates. A1 and A6 are rounding, as
derived.

Mutations (scratch copy `w49i-work/f384-scratch/tree` from `git archive` of bae5849d, script `mutate.py`, own target directory,
logs `BASE.log`, `MU1.log` to `MU6.log`; the unmutated copy passed first; each mutation applied alone and reverted):

| Id | Mutation | Caught by |
| --- | --- | --- |
| MU1 | face load `+p` | A1 (plane 2.0, V 0.43), A1b, A3 (2.0), A4, A5 (0.43), K3, N2a |
| MU2 | nearest node accepted whatever its distance | A2 (a), A2 (d) |
| MU3 | temperature map by `grid id - 1` | K5 (index out of range: the renumbered ids, a panic in the test) |
| MU4 | face load lumped `-p A/4` per corner | **not caught** |
| MU5 | quads reversed in `shell_from_surface_set` | A1, A3, A5 (the orientation refusal fails every pressure test), K1 |
| MU6 | `2 reference - T` stored | A6 (2.0), K5 |

MU4 survived. Diagnosis: on a parallelogram face (every face of the A and K checks: hex box faces, chord rectangles of
`annulus_3d`) `int N_a dS = A/4` exactly, so the lumped and the consistent loads coincide and K3 cannot see the difference; the
registration's expectation that K3 catches it was wrong. The only non-parallelogram face, N2a's, has a lower limit only (its
moment difference grew from 2.0e-4 to 2.3e-2 under MU4, still `> 1e-6`). A check that compares the nodal loads of a trapezoidal
or warped face with `Load::pressure` (or with the exact `int N_a n dS`) would catch it; none was registered.

`cargo fmt -p rcfd-post --check`: clean. `cargo clippy -p rcfd-post --all-targets -- -D warnings`: clean. Only the new test file
was run (release); no existing line of any library was changed (the stub held one comment line), so no existing test covers a
changed line and no baseline-hash run is owed. The whole workspace suite was not run (the lead's gate).

**Decision by the registered rule: partial.** Every clause of the acceptance was met on the single registered run (A1 to A6), and
every K check; but the negative control N2 did not behave as expected on its registered run (defective set-up, met only by the
amended N2a after that run), and the mutation MU4 was not caught.

### Remaining questions

1. MU4 (lumped face loads) is invisible on parallelogram faces. Options: (a) a follow-up registered check on a trapezoidal and a
   warped Hex8 face comparing `deck_pressure_loads` with `Load::pressure` node by node; (b) accept, since every face the F-237
   parametrisations produce is a parallelogram. Recommendation: (a), small (one test, no library change).
2. Faces of Tet4, Tet10 and Hex20 surfaces are refused (no 3-, 6- or 8-node shell in `deck_force_moment`). Options: (a) extend the
   deck and `deck_force_moment` to `CTRIA3` and later quadratic shells; (b) leave the route to Hex8 surfaces until F-362 (general
   mapping). Recommendation: (b); (a) changes F-237's summation and needs its own registration.
3. The match tolerance is the caller's (absolute). Options: (a) keep it explicit; (b) a default relative to the bounding box.
   Recommendation: (a); a default would be a new behaviour to register.

## For the lead: roadmap entries

Proposed status of F-384: **partial**.

> ADR 0436, 2026-11-14: rcfd_post::solid_loads: shell_from_surface_set turns a surface set of 4-node solid faces into the F-237 ShellSurface (same nodes, outward normal toward the fluid, plane or z-cylinder parameter rectangles); deck_pressure_loads applies uniform PLOAD4 cards as consistent nodal loads on the matched faces (for solve_static_with); deck_temperature_load maps TEMP to Load::temperature with one value per solid node; grids matched by coordinates within a stated absolute tolerance (1e-9 m in the checks), a grid, element, corner-varying card or inward-pointing card refused by its id. Resultant force and moment of the nodal loads equal deck_force_moment within 2.9e-16 (plane and two tubes; limit 1e-12). Lame tube b/a = 2, plane strain, the uniform wall pressure of the F-237 fluid mesh exported, written, parsed and applied to a graded Hex8 tube 128 x 32: bore u_r within 3.2e-4 (limit 1 percent), hoop stress 5.6e-3 (limit 2 percent); plane-stress control 3.1 percent. Varying pressure on a 64 x 4 x 20 tube under the whole fluid wall: reactions equal the fluid force within 1.55e-4 S (limit 1e-3, the F-237 deck error). Uniform TEMP free expansion within 9.5e-14 (limit 1e-6). PARTIAL: the warped-face negative control was symmetric (centroid formula exact) and met only after amendment 1 with an asymmetric face (2.0e-4); the mutation that lumps the face load is not caught, because it is exact on parallelogram faces. Tet, Tet10 and Hex20 surfaces refused.

Follow-up item (proposed; id at the lead's choice). Title: "Deck pressure loads on non-parallelogram faces". Deps: F-384.

> Priority P3. Size S. Area: structural. Opened by ADR 0436: the consistent load of deck_pressure_loads is checked only on parallelogram faces, where lumping is exact (mutation not caught).

Acceptance of the follow-up:

> Every limit not stated here is registered in the ADR before the run. On a trapezoidal and on a warped Hex8 face the nodal loads of deck_pressure_loads equal those of Load::pressure node by node within 1e-14 relative, and the lumped-load mutation is caught.
