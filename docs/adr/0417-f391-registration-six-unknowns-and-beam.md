# ADR 0417: F-391 registration: six unknowns per node (rotations in the DofMap, nodal moments, moment reactions) and the two-node shear-flexible beam Beam2 (exact prismatic Timoshenko stiffness, consistent mass with rotary inertia, section forces)

Status: **REGISTERED 2026-11-13, written before any code of F-391 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-391 was written, compiled or run: the files listed under "What was read" were read and
the arithmetic below was done by hand and evaluated with a short script of the closed forms and of a plain planar prototype of the
element derived here (`w49i-work/f391-scratch/estimate.py`, numpy, no rcfd code; disclosed). Results go in the addendum at the
end; nothing above the addendum is changed after a run.

Parent: [ADR 0294](0294-six-unknowns-per-node-and-beam-element.md) (the stub, work package SI-4 of the intermediate tier),
[ADR 0290](0290-structural-solver-intermediate-tier-plan.md) (the plan), [ADR 0291](0291-intermediate-scaffold-modules-and-model-extensions.md)
(the scaffold: `beam.rs`, `ElementKind::Beam2`, `BeamSection`, `BlockProperty::Beam`, the refusal rows R2 and X3). Roadmap item
F-391 (deps F-388, F-374, F-375, F-376, F-377, F-383). Size M. Number 0417 given by the lead. Branch `f391-six-dof-beam`, worktree
`w49i-work/f391`, scratch `w49i-work/f391-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact); each registered run made once; defaults bit-identical; no edit of `baseline_hashes.rs`, of the hash
tables or of a golden file; no new dependency, no unsafe code; clean-room.

## What F-391 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. Nodes carry three or six unknowns; a model without
> structural elements gives the bits it gave before. Beam, ONE element, any orientation in space (registered seed): tip deflection
> P L^3/(3 E I) + P L/(kappa G A), tip rotation P L^2/(2 E I), twist T L/(G J) and elongation F L/(E A) within 1e-10, section
> forces at the root equal to the applied loads within 1e-10, reactions (forces and moments) balancing the loads within 1e-10; a
> rotation of the whole model leaves the results unchanged within 1e-10. Quarter-circle arch of radius R clamped at one end, 32
> elements, slender: deflection under the tip load pi P R^3/(4 E I) within 0.5 percent. Free beam: six zero frequencies below 1e-6
> of the seventh. Pinned-pinned beam, 20 elements, slenderness 50: f_n = (n pi/L)^2 sqrt(E I/(rho A))/(2 pi), n = 1, 2, 3, within
> 0.5, 1 and 2 percent after the registered shear and rotary-inertia correction. A beam node shared with a solid element is refused
> (no rotational stiffness from the solid) with the node id.

Clauses: (a1) three or six unknowns per node; (a2) a model without structural elements gives its former bits; (a3) one element,
any orientation, seeded: tip deflection, tip rotation, twist, elongation within 1e-10; (a4) root section forces equal the applied
loads within 1e-10; (a5) reactions (forces and moments) balance the loads within 1e-10; (a6) a rotation of the whole model leaves
the results unchanged within 1e-10; (a7) quarter-circle arch, 32 elements, slender, tip deflection within 0.5 percent of
`pi P R^3 / (4 E I)`; (a8) free beam: six zero frequencies below 1e-6 of the seventh; (a9) pinned-pinned beam, 20 elements,
slenderness 50, the first three frequencies within 0.5, 1 and 2 percent of the Euler-Bernoulli value after the registered
correction; (a10) a beam node shared with a solid element refused naming the node.

Readings, registered:

- **(a1)** `DofMap::for_mesh` gives six unknowns (`u_x, u_y, u_z, theta_x, theta_y, theta_z`, small rotations about the global
  axes, in this order) to every node of a Beam2 element and three to every other node. A mesh without a Beam2 block gets exactly
  `DofMap::uniform(n, 3)`, as before. Shell4 nodes (ADR 0295) are left to that package.
- **(a2)** "bits it gave before": the existing bit record `si_scaffold_bits.rs` (static solves by every solver, stiffness pattern
  and values, modal frequencies and recovered stresses of the basic kinds, digests taken on main ee449e26) passes unedited, every
  existing `rcfd-fem` test that is not a retired scaffold row passes unedited, and `rcfd-api`'s `baseline_hashes` passes.
- **(a3)** "P L^3/(3 E I) + P L/(kappa G A)" for each bending plane with its own `I` and `kappa`; "tip rotation P L^2/(2 E I)"
  for each plane; one load case with all loads at once (superposition is exact in a linear model). Each quantity is compared in
  the element's local axes, relative to its closed form.
- **(a4)** section forces are the resultants `(N, V_y, V_z, T, M_y, M_z)` in local axes on the section, with the convention that
  they are the force and moment the part on the side of larger local `x` exerts on the other part (tension and sagging positive in
  the usual sense: `N > 0` in tension, `M_z = E I_z psi_z'`). At the root of a cantilever loaded at the tip by `F, M` they are `F`
  and `M + L e_1 x F` ("equal to the applied loads", their moment taken about the root, the only reading under which a moment at
  the root can equal a load at the tip). Forces relative to `max |F|`, moments relative to `L max |F|`.
- **(a6)** the whole model (nodes, orientation vector, loads) rotated by a registered rotation `Q`; displacement, rotation,
  reaction force and reaction moment vectors rotated back by `Q^T` and the local section forces compared directly.
- **(a7)** the configuration and the closed form are derived in arithmetic 4: the free tip load must be **parallel to the
  tangent at the clamp** (perpendicular to the tip tangent) for `pi / 4`; a load along the tip tangent gives `3 pi / 4 - 2`.
- **(a9)** the "registered correction" is the exact Timoshenko pinned-pinned frequency (arithmetic 5): the reference is
  `f_n^EB c_n` with `c_n = f_n^T / f_n^EB`, that is `f_n^T`.

## What was read (no run)

- `AGENT_RULES.md`; the brief; `docs/roadmap.json` entry F-391; ADR 0294 (stub), ADR 0291 sections on `beam.rs`, ADR 0415
  (style).
- `crates/rcfd-fem/src`: `model.rs` (whole: `DofMap`, `Load`, `Constraint`, `SolidModel::add_*`, `check_elements_supported`,
  `check_supported`, `SolidResult`), `assemble.rs` (whole), `mass.rs` (whole), `solve.rs` (whole), `constraint.rs` (whole:
  `rigid_row` already gives rows for components 3 to 5), `loads.rs` (whole), `modal.rs` (whole), `element.rs` (trait, `Pending`,
  `element_for`), `mesh.rs` (kinds, `BeamSection`, `BlockProperty`, `add_block`, `set_block_property`), `beam.rs` (placeholder),
  `error.rs` (variants), `free_motions.rs` (`free_rigid_motions` already refuses rotational unknowns), `recover.rs`
  (`IntegrationPointField::compute`, `extrapolation_matrix`), `mpc.rs` (`uses_transformation`, entry checks of
  `solve_static_mpc` and `modal_analysis_mpc`); the `check_supported` calls of `cyclic.rs`, `harmonic.rs`, `transient.rs`,
  `rotating.rs`, `buckling.rs`, `geometric.rs`, `nonlinear.rs`.
- `crates/rcfd-post/src/solid_output.rs` (`solid_fields` already refuses a node with other than three components).
- Tests: `si_scaffold.rs` (whole), `si_scaffold_bits.rs` (head and digests), `si_geometric.rs` C8, `si_nonlinear.rs` C4,
  `sb_modal.rs` F1 (free box, the 1e-6 precedent), `rcfd-post/tests/si_scaffold_output.rs`.
- **Not read:** any finite-element code, manual or documentation outside this repository. The textbook source of the method
  (Przemieniecki, Theory of Matrix Structural Analysis, 1968, the prismatic shear-flexible beam) is cited from memory, as in the
  stub; everything used is re-derived below.

## Scaffold rows retired (standing decision: ignored record citing this ADR plus a running `_after_beam_landed` copy)

`grep 0294` in `crates/rcfd-fem/tests` and `crates/rcfd-post/tests`:

| file | test | row | why |
| --- | --- | --- | --- |
| `si_scaffold.rs` | `x1_the_scaffold_exists_after_cyclic_landed` | `beam::PENDING == Some("ADR 0294")`, `Beam2.pending() == Some("ADR 0294")` | both become `None` |
| `si_scaffold.rs` | `x3_placeholder_elements_refuse_after_landed` | Beam2 refuses `stiffness`, `integration_points`, `strain_at` naming `ADR 0294` | the solid-element routines of Beam2 now refuse naming `beam::SOLID_ROUTINES` (D7) |
| `si_scaffold.rs` | `r1_to_r9_static_solve_refuses_each_new_variant_after_cyclic_landed`, `r10_refused_on_every_static_path_after_cyclic_landed` | R2 (Beam2 on Hex8 nodes refused as a pending kind, `ADR 0294`) | that model is now refused as a beam node shared with a solid (check R1 here) |
| `si_nonlinear.rs` | `a2_n1_c4_rigid_rotation_and_entry_points` | C4 Beam2 (`internal_force` refuses naming `ADR 0294`) | refuses naming `beam::SOLID_ROUTINES` |

Each gets `#[ignore = "record of ... before ADR 0417 (F-391, Beam2) retired the ... row; superseded by the _after_beam_landed copy
(maintainer standing decision, 2026-11-13)"]` and nothing else; each copy keeps every other row as it was and states the new
behaviour of the retired row. Not retired: `si_geometric.rs` C8 (Beam2's `geometric_stiffness` still refuses with
`NotYetSupported`: the trait default of `Pending`), `v1_new_data_are_validated` (validation of the data, unchanged),
`rcfd-post` O1 (the VTK mapping, unchanged).

## Arithmetic done before the code (disclosed)

1. **Local frame.** Element nodes `x_0, x_1`, `L = |x_1 - x_0| > 0`, `e_1 = (x_1 - x_0) / L`; with the block's orientation vector
   `v`, `e_3 = (e_1 x v) / |e_1 x v|`, `e_2 = e_3 x e_1` (so `e_2` is the part of `v` normal to the axis; the section axis y of
   ADR 0291 "lies in their plane"). Refused (naming the element): `|e_1 x v| <= 1e-6 |v|` (orientation parallel to the axis within
   1e-6 rad) and `L = 0`. `Lambda` has rows `e_1, e_2, e_3`; the element's twelve unknowns in local axes are `T u_g` with `T =
   diag(Lambda, Lambda, Lambda, Lambda)` (translations and small rotations are both vectors), and `K_g = T^T K_l T`,
   `M_g = T^T M_l T`.
2. **Planar Timoshenko element (exact for a prismatic member, no distributed load).** In a bending plane with deflection `d(x)`
   and section rotation `psi(x)` (the slope of the deflection in the absence of shear), shear strain `gamma = d' - psi`, energy
   `(1/2) int (E I psi'^2 + k_s gamma^2) dx`, `k_s = kappa G A`. The homogeneous equations `(k_s gamma)' = 0`,
   `E I psi'' + k_s gamma = 0` give `gamma` constant and `psi` quadratic, `d` cubic. With `xi = x / L`,
   `d = b_0 + b_1 xi + b_2 xi^2 + b_3 xi^3` and `E I psi'' = -k_s gamma`: `psi = (b_1 + 2 b_2 xi + b_3 (3 xi^2 + Phi / 2)) / L`,
   `Phi = 12 E I / (k_s L^2)`. The end values `(d_0, L psi_0, d_1, L psi_1) = A b` with rows `(1, 0, 0, 0)`, `(0, 1, 0, Phi / 2)`,
   `(1, 1, 1, 1)`, `(0, 1, 2, 3 + Phi / 2)`; `b = A^{-1} (...)` gives the shape functions ("interdependent interpolation"). The
   cantilever flexibility of the end `x = L` under an end force `V` and moment `M` is `d = V L^3 / (3 E I) + V L / k_s + M L^2 /
   (2 E I)`, `psi = V L^2 / (2 E I) + M L / (E I)` (integrating `E I psi' = M(x)`, `gamma = V / k_s`); its determinant is
   `L^4 (1 + Phi) / (12 E^2 I^2)` and its inverse, completed by equilibrium (`V_0 = -V_1`, `M_0 = -M_1 - L V_1`), is the stiffness
   in `(d_0, psi_0, d_1, psi_1)`:
   `E I / ((1 + Phi) L^3) [[12, 6L, -12, 6L], [6L, (4 + Phi) L^2, -6L, (2 - Phi) L^2], [-12, -6L, 12, -6L], [6L, (2 - Phi) L^2,
   -6L, (4 + Phi) L^2]]`. The same matrix is the energy of the shape functions integrated by 4-point Gauss (exact: the integrands
   have degree at most 4): prototype difference `3.6e-16` relative (script). The element reproduces the exact cantilever
   solution, so the one-element checks are rounding-limited.
3. **The two planes and the other actions.** Plane x-y: `d = v`, `psi = theta_z` (a positive rotation about `e_3` turns `e_1`
   toward `e_2`), `I = I_z`, `kappa = kappa_y` (`shear_y`, shear along y). Plane x-z: `d = w`, `psi = -theta_y`, `I = I_y`,
   `kappa = kappa_z`; in `(w, theta_y)` the matrix is `S k S`, `S = diag(1, -1, 1, -1)`. Axial `E A / L [[1, -1], [-1, 1]]`,
   torsion `G J / L [[1, -1], [-1, 1]]`, `G = E / (2 (1 + nu))` of the isotropic material. Consistent mass (kinetic energy of the
   same interpolation): axial `rho A L / 6 [[2, 1], [1, 2]]`; torsion `rho (I_y + I_z) L / 6 [[2, 1], [1, 2]]` (the polar moment
   of the section about its centroid, registered; `J` is a stiffness constant, not an inertia); bending in each plane
   `int (rho A N_d^T N_d + rho I N_psi^T N_psi) dx` with the interdependent shape functions, 4-point Gauss (integrands of degree 6,
   exact). Rigid motions (translation; rotation with `d` linear, `psi = d'`, `gamma = 0`) lie in the interpolation, so `K_l r = 0`
   and `r^T M r` is the exact rigid kinetic energy (D5, check M1).
4. **The arch (a7).** Centre at the origin, plane x-y, `x(phi) = R (cos phi, sin phi, 0)`, `phi in [0, pi / 2]`; clamped at
   `phi = 0` (the point `(R, 0, 0)`, tangent `e_y`), free tip at `(0, R, 0)`, tip force `P e_y`. The arch is statically
   determinate: the moment about `e_z` at `phi` is `M = ((x_tip - x) x P e_y)_z = -P R cos phi`, the axial force `N = P cos phi`
   and the shear force `V = -P sin phi` (along the tangent `(-sin, cos)` and the normal). Castigliano with the bending energy
   only, `U = int M^2 / (2 E I) R dphi = P^2 R^3 / (2 E I) (pi / 4)`, gives the tip displacement along the load `dU / dP =
   pi P R^3 / (4 E I)`: the acceptance's closed form holds for **this** load direction (along the tangent at the clamp, radial at
   the tip). For a tip load along the tip tangent `-P e_x`, `M = P R (1 - sin phi)` and the deflection is `(3 pi / 4 - 2) P R^3 /
   (E I)` (0.4535 of the other; not registered as a check). The axial and shear energies add `pi / 4 P R / (E A)` and
   `pi / 4 P R / (kappa G A)`: relative `4.0e-6` and `1.16e-5` at the registered slenderness. Registered data: `R = 1 m`, circular
   section of radius `a = 0.004 m` (`I = pi a^4 / 4`, `A = pi a^2`, radius of gyration `a / 2`, `R / r = 500`), `kappa = 0.9`
   (input), `E = 2e11 Pa`, `nu = 0.3`, `P = 1 N`, orientation vector `(0, 0, 1)` (so in-plane bending is about `e_2 = e_z`, with
   `I_y = I_z` for the circle), 32 straight elements between the nodes `x(i pi / 64)`. **The finite-element model is the polygon
   frame of 32 chords**, and since each element is exact for end loads its tip displacement is the Castigliano value of the
   polygon (bending, axial and shear energy of the chords, the internal forces known by statics): script, relative to
   `pi P R^3 / (4 E I)`: bending `-5.0187e-4`, axial `+4.0e-6`, shear `+1.155e-5`, total **`-4.863e-4`** (0.049 percent, the
   chords lying inside the arc). This polygon value is registered as the strong check A2.
5. **Pinned-pinned frequencies (a9).** For the Timoshenko beam on pins, `d = D sin(k x)`, `psi = Psi cos(k x)`, `k = n pi / L`,
   satisfy the end conditions (`d = 0`, `M = E I psi' = 0`) exactly. The equations `rho A d_tt = k_s (d'' - psi')`,
   `rho I psi_tt = E I psi'' + k_s (d' - psi)` give the 2 x 2 determinant
   `(k_s k^2 - rho A w^2)(E I k^2 + k_s - rho I w^2) - (k_s k)^2 = 0`, a quadratic in `w^2`; the lower root is the bending
   frequency `f_n^T = w / (2 pi)` (the higher root a shear mode). Registered data: `L = 1 m`, square section of side `h =
   0.02 sqrt(12) m` (`A = h^2`, `I_y = I_z = h^4 / 12`, radius of gyration `0.02 m`, slenderness `L / r = 50`), `J = 0.1406 h^4`
   (input; the torsion is constrained), `kappa = 5 / 6`, `E = 2e11 Pa`, `nu = 0.3`, `rho = 7850 kg/m^3`, 20 elements along `e_x`,
   orientation `(0, 0, 1)`. Values (script): `f^EB = 158.5733, 634.2932, 1427.1596 Hz`; `f^T = 157.3029, 614.8257, 1334.9373 Hz`;
   correction factors `c_n = 0.991989, 0.969308, 0.935381`. Without the correction the Euler-Bernoulli values are high by `0.81,
   3.17, 6.91` percent: n = 2 and 3 would miss their limits (negative control N1). The planar prototype of the element
   (arithmetic 2, 3) with 20 elements gives `157.3049, 614.9452, 1336.1467 Hz`, relative to `f^T`: `1.28e-5, 1.94e-4, 9.06e-4`;
   the same prototype without rotary inertia `1.94e-3, 7.35e-3, 1.53e-2`, without shear flexibility `6.09e-3, 2.36e-2, 5.06e-2`
   (the strong check F3 separates both).
6. **The one-element case (a3 to a5).** Data: `L = 0.8 m`, `A = 0.012 m^2`, `I_y = 3e-6`, `I_z = 5e-6`, `J = 4e-6 m^4`,
   `kappa_y = 0.85`, `kappa_z = 0.75` (all different, so a swap is seen), `E = 2.1e11 Pa`, `nu = 0.3` (`G = E / 2.6`), `rho = 7850`.
   Shear share of the tip deflection `3 E I / (kappa G A L^2)`: `5.97e-3` (y), `4.06e-3` (z). Geometry from the seed `0x0417_0391`
   of a xorshift64* generator (in the test): `x_0` uniform in `[-1, 1]^3`, `e_1` the normalised vector of three uniforms in
   `[-1, 1]`, the orientation vector likewise, drawn again while `|e_1 x v| < 0.3`. Loads in local axes, applied at the tip as
   global vectors: force `F_l = (1200, -2500, 3700) N`, moment `M_l = (450, 0, 0) N m` (the torque `T`). Closed forms of the tip
   in local axes: `u = F_x L / (E A)`, `v = F_y (L^3 / (3 E I_z) + L / (kappa_y G A))`, `w = F_z (L^3 / (3 E I_y) + L /
   (kappa_z G A))`, `theta_x = T L / (G J)`, `theta_y = -F_z L^2 / (2 E I_y)`, `theta_z = F_y L^2 / (2 E I_z)`. Root section
   forces `(F_l, T, -L F_z, L F_y)`, tip section forces `(F_l, T, 0, 0)`. Rounding: the reduced system is the 6 x 6 tip block; its
   entries span `E A / L = 3.2e9` to `12 E I_y / L^3 = 1.5e7`, condition below about `1e3` after the transformation; expected
   errors near `1e-14`.
7. **Free beam (a8).** `L = 1 m` along `(1, 1, 1) / sqrt 3` from the origin, circular section radius `0.05 m` (`L / r = 40`,
   `kappa = 0.9`), steel as in 5, orientation `(0, 0, 1)`, 10 elements, no constraint, `ModalOptions::new(7)` (default: relative
   residual 1e-10, automatic shift for the free body), as the free box F1 of ADR 0284 which met the same 1e-6. 66 unknowns.
8. **Rotation of the whole model (a6).** The one-element model of 6 and a three-element space frame (nodes `(0, 0, 0)`,
   `(0.8, 0, 0)`, `(0.8, 0.6, 0)`, `(0.8, 0.6, 0.5)`, clamped at node 0, tip force `(100, -200, 300) N` and moment
   `(10, 20, -30) N m` at node 3, the section of 6, orientation `(1, 2, 3)`, not parallel to any member), each solved as given and
   with nodes, orientation vector and loads rotated by `Q` = rotation of `1.1 rad` about `(1, -2, 2) / 3`.

## Design

### D1. `DofMap`

`DofMap::for_mesh(mesh)`: if no block is Beam2, `DofMap::uniform(n, 3)` (the former body, so the former bits); otherwise counts
`3` per node, `6` at each node of a Beam2 element, `DofMap::from_counts`. Components 3, 4, 5 are the small rotations about x, y,
z. `SolidModel::dof_map` is unchanged (it calls `for_mesh`). Everything downstream already goes through `DofMap` (assembly
pattern, `element_dofs`, scatter, constraints, reactions, `node_component`).

### D2. Loads, constraints, reactions, results

- New `Load::NodalMoment { set, moment }` (`Load::nodal_moment(set, moment)`), N m, the same moment on each node of a node set;
  `add_load` refuses a non-finite moment and a node of the set without rotations (`ComponentOutOfRange`, naming node and
  component 3). `nodal_load_vector` adds it to components 3 to 5.
- Prescribed rotations: `Constraint::displacement(set, c, value)` with `c` in 3 to 5 at six-unknown nodes (already validated
  against the `DofMap`). Moment reactions: the reactions `K u - f` at prescribed rotation equations (no change).
- Rotations in the output: `SolidResult::displacement(node)` returns six values at a beam node (no change). `mass::rigid_motions`
  gives the rotation components of the rigid rotations at six-unknown nodes (`r_{3+k} = 1` at component `3 + k`); at
  three-unknown nodes nothing changes.
- Section forces: `beam::section_forces(model, displacement) -> Result<Vec<(usize, SectionForces)>>`, one per beam element in
  element order: `f = K_l T u_e`, `start = -f[0..6]`, `end = f[6..12]` (arithmetic reading (a4): the node exerts `f_0` on the
  element at its start, so the section just after the start node carries `-f_0`).

### D3. The beam element (`beam.rs`)

`beam::PENDING = None`. Public: `frame` (arithmetic 1), `plane_shape_coefficients(phi)` (arithmetic 2, `A^{-1}`), `local_stiffness`,
`local_mass`, `element_stiffness` and `element_mass` (global, from coordinates, section, orientation and material),
`section_forces`, `SectionForces`, `rectangle_section(b, h, torsion, shear)` (`A = b h`, `I_y = b h^3 / 12` with `h` along the
section axis z, `I_z = h b^3 / 12`, the torsion constant an input as the stub says), `circle_section(radius, shear)` (`A = pi r^2`,
`I_y = I_z = pi r^4 / 4`, `J = pi r^4 / 2`). Stiffness: the closed form of arithmetic 2 and 3; mass: arithmetic 3.

### D4. Which paths take beams

Beams are taken by `assemble::stiffness` / `stiffness_with`, `mass::consistent` / `consistent_with`, `solve::solve_static*` on the
elimination path (direct, refined direct, Jacobi CG, block-Jacobi and incomplete-Cholesky CG) and `modal::modal_analysis*` on the
elimination path. `SolidModel::check_elements_supported` and `check_supported` (called by every other path: the transformation
of ADR 0296, cyclic, harmonic, transient, rotating, buckling, geometric stiffness, nonlinear) refuse a Beam2 block with
`NotYetSupported { item: "element kind", variant: "Beam2", location: "block b", package: beam::FOLLOW_UP }`, `FOLLOW_UP = "a
follow-up of ADR 0417"`; the beam paths call new `check_elements_supported_with_beams` / `check_supported_with_beams` instead. The
multigrid preconditioner (`SmoothedAggregation`, rigid-body modes of three-unknown nodes) refuses a beam model the same way. Body
loads (gravity, centrifugal, nodal body force), the thermal load and `recover` reach Beam2 through the solid-element routines and
are refused there (D7). For a model without Beam2 every check gives what it gave before (the beam test is one extra comparison of
the kind).

### D5. Beam model checks (`check_*_with_beams`)

In order: every Beam2 block has a `BlockProperty::Beam` (else `InvalidValue { key: "property" }` naming the block); every node of a
Beam2 element that is also a node of a solid element is refused, the first in element order and node order, with the new
`FemError::BeamSolidNode { node, beam, solid }` (the beam element and the first solid element using the node) — a solid element
has no rotational stiffness, and joining the two needs the rigid set of ADR 0296 (a follow-up); every beam element's frame
(arithmetic 1) exists (else `FemError::Element { element, InvalidValue { key: "orientation" | "length" } }`); the material of a
beam element is isotropic elastic (the only variant).

### D6. Assembly dispatch

`stiffness_with` and `consistent_with` call `beam::element_stiffness` / `beam::element_mass` for a Beam2 element (its block's
section and orientation, its material) and the former expressions for every other kind, in the same order: the solid entries are
the same sums, so the same bits.

### D7. The solid-element routines of Beam2

`beam::element()` stays a `Pending` whose package is `beam::SOLID_ROUTINES = "a follow-up of ADR 0417 (the solid-element
routines of Beam2; its matrices are in rcfd_fem::beam)"`: the isoparametric volume map of the `Element` trait does not apply to a
beam, whose matrices need the section. `reference_nodes`, `faces` and `shape_gradients` stay empty (no caller needs them for
Beam2).

## Checks (limits fixed now)

| id | clause | check | limit | estimate behind it |
| --- | --- | --- | --- | --- |
| D1 | a1 | unknown counts: solid-only mesh `for_mesh == DofMap::uniform(n, 3)`; beam-only mesh 6 at every node; a mesh with a Hex8 box and a separate beam: 3 at the box nodes, 6 at the beam nodes, `n_equations = 3 n_s + 6 n_b` | exact | D1 |
| B1 | a2 | `si_scaffold_bits.rs` (bit record of ee449e26) passes unedited | pass | D1, D4, D6 |
| B2 | a2 | every existing `rcfd-fem` test passes, unedited except the retired rows (ignored) whose copies pass | pass | the table of retired rows |
| B3 | a2 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | no fluid file touched |
| C1 | a3 | tip deflection `v` in local axes against `F_y (L^3 / (3 E I_z) + L / (kappa_y G A))`, relative | `1e-10` | arithmetic 6: `~1e-14` |
| C2 | a3 | tip deflection `w` against `F_z (L^3 / (3 E I_y) + L / (kappa_z G A))` | `1e-10` | idem |
| C3 | a3 | tip rotation `theta_z` against `F_y L^2 / (2 E I_z)` | `1e-10` | idem |
| C4 | a3 | tip rotation `theta_y` against `-F_z L^2 / (2 E I_y)` | `1e-10` | idem |
| C5 | a3 | twist `theta_x` against `T L / (G J)` | `1e-10` | idem |
| C6 | a3 | elongation `u` against `F_x L / (E A)` | `1e-10` | idem |
| C7 | a4 | root section forces against `(F_l, T, -L F_z, L F_y)`: forces relative to `max |F_l|`, moments to `L max |F_l|`; tip section forces against `(F_l, T, 0, 0)` likewise | `1e-10` | rounding |
| C8 | a5 | reactions: `|sum R_f + F|` relative to `max |F|`; `|R_m + (x_1 - x_0) x F + M|` (moments about the root) relative to `L max |F|`; the reactions at free equations are 0 | `1e-10`; exact | rounding |
| C9 | a3 | the same one-element case by `ConjugateGradient` (tolerance 1e-13, cap 100): tip unknowns against the direct solve, relative to `max |u|` | `1e-8` | 6 unknowns, Jacobi-preconditioned |
| R1 | a6 | one-element model and the space frame (arithmetic 8) rotated by `Q`: `max |Q^T u'_t - u_t|` and `max |Q^T u'_r - u_r|` relative to the maximum of each field; reaction forces and moments likewise; section forces of every element against the unrotated ones relative to their maximum | `1e-10` each | rounding |
| A1 | a7 | arch tip `u_y` against `pi P R^3 / (4 E I)`, relative | `5e-3` | arithmetic 4: `-4.86e-4` |
| A2 | (stronger, a7) | arch tip `u_y` against the polygon closed form (arithmetic 4, computed in the test by statics), relative | `1e-6` | exact element; rounding with `(R / r)^2 = 2.5e5` |
| F1 | a8 | free beam: `max_{i <= 6} |f_i| / f_7` | `1e-6` | arithmetic 7, the F1 precedent |
| F2 | a9 | pinned-pinned, `|f_n - c_n f_n^EB| / (c_n f_n^EB)`, n = 1, 2, 3 | `5e-3`, `1e-2`, `2e-2` | arithmetic 5: `1.3e-5, 1.9e-4, 9.1e-4` |
| F3 | (stronger, a9) | the same against `f_n^T` computed in the test from the quadratic of arithmetic 5 | `5e-5`, `5e-4`, `2e-3` | arithmetic 5: margins `3.9, 2.6, 2.2` |
| F4 | (rest, a9) | the three modes are bending in the x-y plane: `max |u_y|` over the nodes is the largest translation of each mode and the mode has `n - 1` interior sign changes of `u_y` | exact | constraints of arithmetic 5 |
| S1 | a10 | the Hex8 box `[2, 1, 1]` (div `[2, 1, 1]`) with a Beam2 block on its nodes at `(1, 0, 0)` and `(2, 0, 0)` (the R2 model of ADR 0291 with a Beam property): `solve_static`, `modal_analysis` and `stiffness` refused with `BeamSolidNode { node }`, `node` the index of `(1, 0, 0)`; the same with only the second beam node `(2, 0, 0)` on the box (the first at `(3, 0, 0)`): refused naming the index of `(2, 0, 0)`; the message names the node | exact | D5 |
| K1 | (element) | `local_stiffness` against the energy `int (E I psi'^2 + k_s gamma^2)` of `plane_shape_coefficients` by 4-point Gauss, each plane, relative to the largest entry; `K_l` symmetric bit for bit | `1e-12`; exact | arithmetic 2: `3.6e-16` |
| K2 | (element) | the global element stiffness of the seeded element times each of the six rigid motions (translation, rotation `w x (x - x_0)` with rotation unknowns `w`), relative to `max |K| max |r|` | `1e-12` | rounding |
| M1 | (mass) | `mass::mass_properties` of the seeded element (density as 6): mass `rho A L`; centre the midpoint; inertia about the centre `Lambda^T diag(rho (I_y + I_z) L, rho A L^3 / 12 + rho I_y L, rho A L^3 / 12 + rho I_z L) Lambda` | `1e-12` relative (centre to `L`, inertia to its largest entry) | arithmetic 3: rigid motions exact |

Negative controls:

| id | control | expected | limit |
| --- | --- | --- | --- |
| N1 | pinned-pinned against the uncorrected `f_n^EB` | `0.81, 3.17, 6.91` percent high (arithmetic 5, less the discretisation) | n = 2 and 3 outside `1e-2`, `2e-2`: the correction is needed |
| N2 | the clamped one-element model on the other paths: `solve_static` with a nodal spring added (the transformation), `nonlinear::initial_states`, `solve_static` by smoothed-aggregation CG, `solve_static` with `Load::gravity` added | the first three refused with `NotYetSupported` naming `Beam2` and `beam::FOLLOW_UP`; gravity refused with an `Element` error whose source names `beam::SOLID_ROUTINES` | exact |
| N3 | invalid beam data: Beam2 block without property; orientation parallel to the axis; `Load::nodal_moment` on a three-unknown node | `InvalidValue` (property); `Element { InvalidValue orientation }`; `ComponentOutOfRange` | exact |
| N4 | the one-element case with the shear term removed in the test's closed form (`P L^3 / (3 E I)` only) | C1 / C2 measure `5.97e-3` / `4.06e-3`: the checks see the shear | `> 1e-3` |

## Planned mutations (on a scratch copy, one at a time)

| id | mutation | expected to be caught by |
| --- | --- | --- |
| MU1 | `Phi = 0` in the bending stiffness (no shear flexibility) | C1, C2 (and K1) |
| MU2 | `I_y` and `I_z` swapped between the planes | C1 to C4 |
| MU3 | the x-z plane without the sign matrix `S` | C2, C4, K2 |
| MU4 | torsion with `I_y + I_z` instead of `J` | C5 |
| MU5 | no rotary inertia in the bending mass | F3 (n = 1), M1 |
| MU6 | `T` built with `Lambda^T` instead of `Lambda` | C1 to C6 (seeded orientation) |
| MU7 | section forces at the start with `+f_0` | C7 |
| MU8 | `Load::NodalMoment` added to components 0 to 2 | C5, C8 |
| MU9 | the beam-solid node check removed | S1 |
| MU10 | `DofMap::for_mesh` gives six unknowns to every node of a mesh with a beam | D1 |

## Decision rule

- **done:** every check of the acceptance (D1, B1 to B3, C1 to C8, R1, A1, F1, F2, S1) met, the stronger checks and N1 to N4 as
  stated, every mutation caught by a registered check.
- **partial:** a clause missed (recorded as a `*_known_miss`) or not run, a mutation not caught, or a check met only after an
  amendment written after its run.
- **todo:** the element does not build.

## Limits (not covered)

- L1. Output: `rcfd_post::solid_output` keeps refusing a node with six unknowns (rotations as point data and VTK line cells for
  Beam2 are a follow-up, listed in the stub's file list).
- L2. Beams on the transformation (rigid sets, linear constraints, supports), cyclic, harmonic, transient, rotating, buckling,
  geometric-stiffness and nonlinear paths and with the multigrid preconditioner are refused (D4), as are body and thermal loads on
  beams and distributed loads along a beam. Joining a beam to a solid (a rigid set, ADR 0296) is a follow-up.
- L3. Prismatic, straight, centroidal elements: no taper, offsets, shear centre, warping, or curved element; section data are
  numbers (the torsion constant of a rectangle an input).
- L4. A path that calls `stiffness_with` or `consistent_with` directly on a beam model without its own entry check gets the beam
  matrices; every such path of the crate today checks first (read above).

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. Torsional inertia `rho (I_y + I_z)` rather than `rho J` (arithmetic 3): the polar moment is the kinetic energy of a rotating
   section; recommended and taken.
2. The beam-only paths of D4 and the refusal of the others with `beam::FOLLOW_UP`: recommended (the acceptance asks static and
   modal only) and taken.
3. The new error variant `BeamSolidNode` rather than a reused one: the acceptance asks the node id named; `FemError` is
   non-exhaustive. Taken.

## Amendment 1 (2026-11-13, written before any run of a check)

A parameter the registration left unstated, fixed before the run: the modal options of the pinned-pinned beam (F2 to F4) are
`ModalOptions::new(3).tolerance(1e-12).residual_test(ResidualTest::Backward)` (zero shift by the default rule, the constraints
holding every rigid motion). Reason: the relative residual's floor grows with `lambda_max / lambda_1`, about `1.2e5` here
(element rotary modes against the first bending mode), and ADR 0354 gives the backward error with 1e-12 for slender models (as
`sb_modal_scale.rs`). The free beam (F1) keeps the default options as registered. No limit changes.
