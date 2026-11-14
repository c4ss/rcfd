# ADR 0487: F-573 registration: follower pressure on shells with its load stiffness

Status: **REGISTERED 2026-11-14, written before any code of F-573 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-573 was written, compiled or run in the repository; no probe was made (the
arithmetic below is by hand). The bit record N1 of the default paths is a test file written and run on the base after this
commit and before any library code (D5). Results go in the addenda at the end; nothing above the addenda is changed after a
run.

Follow-up of F-424 ([ADR 0480](0480-f424-registration-follower-pressure-arc-length-tangent-modes.md)), proposed in its addendum.
Roadmap item F-573 (deps F-424). Number 0487 given by the lead. Branch `f573-work` at fb7c5d94, worktree `w49i-work/f573`,
scratch `w49i-work/f573-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code, the crate graph
unchanged; no existing test edited; clean-room.

## What F-573 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. ShellPressure gains a follower form on the current mid-surface with its load stiffness, against central differences within 1e-6 on Shell4 and Shell3; a thin cylinder of shells under follower internal pressure follows p = E t (lambda^2 - 1)/(2 R) within 1 percent to the stretch 1.5; the dead ShellPressure keeps its bits.

The brief adds: (b1) find out whether the nonlinear static driver takes shells at large deformation; if not, build the follower
load and its load stiffness for shells, meet the central-difference clause, register the cylinder clause as a predicted miss
with its reason and propose a follow-up for nonlinear shells, without inventing a large nonlinear shell formulation in this
item; (b2) the dead `ShellPressure`, every default and `baseline_hashes` keep their bits, a default-bits test at the base before
any library change; (b3) probe before a `1e-12`-class limit, `1e-11` for sums over about `1e3` terms.

Clauses: (a0) every limit registered here; (a1) the follower form of the shell pressure on the current mid-surface and its load
stiffness, against central differences within `1e-6` on Shell4 and on Shell3; (a2) the thin cylinder of shells under follower
internal pressure follows the thin law within 1 percent to the stretch 1.5; (a3) the dead `ShellPressure` keeps its bits.

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0480 (whole); the F-573 entry of `docs/roadmap.json`.
- `crates/rcfd-fem/src/follower.rs` (whole), `nonlinear.rs` (`check_model`, `check_driver_model`, `solve`, `solve_with`,
  `DriverExtensions`, `add_triplets`), `model.rs` (`Load::ShellPressure`, `Load::ShellPressureAngleWeighted`, their check,
  `check_elements` and its shell refusals), `loads.rs` (`nodal_load_vector`, `add_shell_pressure`, `face_shape_functions`,
  `face_rule`), `shell.rs` (`facet`, `pressure_load_vector`), `shell3.rs` (`facet`, `pressure_load_vector`,
  `pressure_load_vector_angle_weighted`), `buckling.rs` (`factors`, `factors_with`), `geometric_struct.rs`
  (`prestress_from_static`), `mesh.rs` (`BlockProperty::Shell`).
- Tests: `sa_follower.rs` (S1 to S4, B1 to B3), `sa_follower_default_bits.rs` (the form of a bit record),
  `sa_geometric_struct.rs` (the shell grid, the plate buckling P1 to P4 and their results in ADR 0473 addendum 1).

## D0. The nonlinear driver and shells (the finding, registered before any code)

The nonlinear static driver does **not** take shells. `nonlinear::check_model` calls
`SolidModel::check_elements_supported_nonlinear`, which is `check_elements(beams = false, shells = false, ..)`: a Shell4 or
Shell3 block is refused with `FemError::NotYetSupported { item: "element kind", variant: "Shell4" (or "Shell3"), package:
shell::FOLLOW_UP (or shell3::FOLLOW_UP) }`, and every block that passes must be a solid of the `Full` formulation (ADR 0404).
`shell.rs` and `shell3.rs` are linear facet elements (stiffness, mass, consistent loads, recovery); the only geometric terms
for shells are the geometric stiffness of a given membrane pre-stress (ADR 0473, the linear buckling and pre-stressed modal
paths). There is no internal force of a shell at a finite displacement, no tangent of one, and no update of the rotations: a
geometrically nonlinear shell (a corotational facet or a degenerated-solid shell with finite rotations) does not exist in the
crate.

Decision, taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation) and the brief (b1): F-573
builds the follower form of the shell pressure and its load stiffness (D1) and meets (a1) and (a3); clause (a2) is registered as
a **predicted miss**: its test (C1) is written in full with its limits and kept as an ignored `*_known_miss` record, run once
to record the refusal; a regular test (C1r) states the refusal. A nonlinear shell formulation is proposed as a follow-up
(addendum, "For the lead"); it is not built here. Two companions show what can be shown without it: the follower force on a
cylinder of shells placed at the stretched state carries the hoop law of the current geometry (C2, the force half of (a2)), and
the load stiffness of shells enters the existing linear buckling path, where the ring under hydrostatic pressure gives `3 E
I/R^3` with it and `4 E I/R^3` without (B1s to B3s, the stiffness half, as ADR 0480 did for solids).

## Arithmetic done before the code (disclosed; by hand, no probe)

1. **The follower shell pressure** (first principles; the linearisation as in ADR 0480 arithmetic 1). The mid-surface of a
   Shell4 or Shell3 facet element is the interpolation of its nodal positions (the nodes lie on the mid-surface; the rotations
   of a facet element turn the fibres, they do not move the mid-surface). On the current mid-surface `x(u, v) = sum_a N_a(u,
   v) (X_a + u_a)` (bilinear on the Shell4, linear on the Shell3), a uniform pressure `p` along the current normal gives the
   virtual work `p int delta x . (x_u x x_v) du dv`, so the nodal forces (translations only; no moments) are `f_a = +p int N_a
   (x_u x x_v) du dv`. With the functions of `loads::face_shape_functions` (Quad4 corners at `(-1,-1), (1,-1), (1,1), (-1,1)`,
   Tri3 at `(0,0), (1,0), (0,1)`), `x_u x x_v` is the right-hand normal of the node order, the side of `e_3` of `shell::facet`
   (`(x_2 - x_0) x (x_3 - x_1)`) and `shell3::facet` (`(x_1 - x_0) x (x_2 - x_0)`): positive `p` pushes towards `+e_3`, the
   convention of `Load::ShellPressure`. The solid follower of ADR 0480 is `f_a = -p int N_a (x_u x x_v)` with the same rules
   and functions: the shell form is the solid face arithmetic with the pressure negated, and the negation is exact in floating
   point. The load stiffness `K_(ai)(bj) = p int N_a [N_b,u (e_j x x_v) + N_b,v (x_u x e_j)]_i` (the same expression, the same
   negation) on the translational equations; zero on the rotational ones.
2. **Quadrature.** `face_rule(4)` is the 3 x 3 Gauss rule, `face_rule(3)` the 9-point collapsed rule of `loads.rs`. On the
   Shell4 `x_u` is constant in `u` and linear in `v`, `x_v` the reverse: `x_u x x_v` has degree 1 in each variable, `N_a x_u x
   x_v` degree 2 in each, and the stiffness integrand `N_a N_b,u x_v` degree 2 in each: the 2 x 2 rule is already exact, the 3
   x 3 rule a fortiori. On the Shell3 `x_u x x_v` is constant and the integrands are linear: exact.
3. **Central differences** (S1s). `x_u x x_v` is bilinear in the nodal positions, so `f(u)` is a quadratic polynomial of `u`:
   the central difference is exact up to rounding. Rounding: `eps |f| / h` against `|K| ~ |f| / L`, `eps L / h = 2.2e-16 /
   1e-6 = 2.2e-10` with the step `h = 1e-6 L`; estimate `<= 1e-9`, limit `1e-6` (the acceptance).
4. **The dead pressure at zero displacement** (S4s). Flat Shell4: the dead load is `p int N_a det` on the projected facet (the
   facet itself), along `e_3`, carried to the global axes by `T^T`; on a flat facet `x_u x x_v = det e_3` with `det` the
   Jacobian of the same map, so the two are equal analytically, by different operations (the local axes and `T`; the moments
   of the dead path are `T^T` of zero local moments with offsets that vanish up to rounding). Flat Shell3: `x_u x x_v = 2 A
   e_3`, `int N_a du dv = 1/6`, `f_a = p A/3 e_3`, the dead `p A/3 e_3` of `shell3::pressure_load_vector`. Rounding only, sums
   of at most a few tens of products: estimate `~1e-15` of `max |f|`, limit `1e-10` (five decades above, no probe needed for a
   limit of this class). Not bits: the operations differ (disclosed; the bits of the dead path itself are N1).
5. **Warped Shell4 at zero displacement** (W1). The dead path loads the projected facet and adds the moments `r x F` of the
   offsets; the follower loads the bilinear surface through the nodes, without moments. The resultants agree: the vector area
   of a bilinear quad is `int (x_u x x_v) du dv = (1/2)(x_2 - x_0) x (x_3 - x_1)` (the cross terms integrate to zero), the
   dead resultant is `p` times the projected area along `e_3`, and the projected facet has the same diagonals (both lie in the
   mean plane, `e_3` being their normal), so the same area vector. Limit on the resultant `1e-10` of `|F|`; the nodal
   distribution differs at the order of the warp and is recorded.
6. **The cylinder of shells at a prescribed stretch** (C2). `N` facets around (full circle), `n_z` along the length `L`, the
   nodes at radius `R`; the state `u = (lambda - 1) X_perp` (the polygon scaled by `lambda` about the axis). Each facet is a
   flat rectangle of width `2 lambda R sin(pi/N)` and length `L/n_z`, normal at the mid-angle; its follower force is `p A n` in
   total, shared equally among its 4 nodes (Shell4; `x_u x x_v` constant on a rectangle, `int N_a = 1`) or `p A/3 n` per node
   of each of its two triangles (Shell3). The normal makes the angle `pi/N` with the radial direction of each of its nodes, so
   `sum_nodes f . e_r(node) = N n_z p (2 lambda R sin(pi/N))(L/n_z) cos(pi/N) = p N lambda R L sin(2 pi/N)` for both, and every
   node's `f . e_theta` and `f . e_z` vanish by symmetry (the two facets of a node mirror each other; an end node has one facet
   on each side as well). The dead `ShellPressure` acts on the reference polygon: its radial sum is `p N R L sin(2 pi/N)`, off
   by `lambda`. These are the force half of the thin law: with a hoop tension `T` per unit length, the balance of a node is `2
   T sin(pi/N) = p 2 lambda R sin(pi/N) cos(pi/N)`, `T = p lambda R cos(pi/N)`, and the SVK membrane of `nu = 0` (`T = E t
   lambda (lambda^2 - 1)/2`) would give `p = E t (lambda^2 - 1)/(2 R cos(pi/N))`, the acceptance's law with the facet factor
   `1/cos(pi/N)` (`+1.2e-3` at `N = 64`). Rounding: the radial sum has `N (n_z + 1) = 320` terms of nodal forces, each a sum of
   at most `4 x 9 x 2` products: about `1e3` terms, limit `1e-11` (b3); the components that vanish: `1e-11` of `max |f|`.
7. **Ring buckling with shells** (B1s, B2s; ADR 0480 arithmetic 3 re-used). Inextensional ring, hydrostatic (follower)
   pressure: `p_cr = (n^2 - 1) E I/R^3`; fixed-direction forces (the buckling path without the load stiffness): `n^2 E I/R^3`;
   `3` and `4 E I/R^3` at `n = 2`, `I = t^3/12` per unit width (`nu = 0`, so `D = E I`). For shells the pressure acts on the
   mid-surface, so the `R/R_o` departure of the solid ring (`-1` percent) is absent. Expected departures: the hoop force of the
   facet polygon `p R cos(pi/96)`, `+5e-4` on `p_cr`; the bending discretisation of 24 Shell4 over the half wave of `n = 2` (the
   plate P1 of ADR 0473 with 16 Shell4 over a half wave measured `3.3e-3`; `O(h^2)` gives about `1.5e-3` here); extensibility
   `O((t/R)^2) ~ 1e-4`; transverse shear `O((t n/R)^2) ~ 4e-4`. Estimate `|dev| <= 5e-3` for both; limits `2e-2` (B1s) and `3e-2`
   (B2s), the limits ADR 0480 registered for the solid ring. The symmetry of the reduced load stiffness (S2s): the
   antisymmetric part of `K_p` is the edge term `p int (delta u x Delta u) . dx` of the loaded surface (ADR 0480 arithmetic 3);
   on the quarter-ring strip its edges are the two generators on the symmetry planes (tangent `e_z`, the term `delta u_x Delta
   u_y - delta u_y Delta u_x`, with `u_y` held on `y = 0` and `u_x` held on `x = 0`) and the two arcs `z = 0, b` (tangent
   `e_theta`, the term `delta u_z Delta u_r - delta u_r Delta u_z`, with `u_z` held): the reduced `K_p` is symmetric up to
   rounding (sums of 9 quadrature terms of at most 2 faces), limit `1e-11` of `max |K_p|` (ADR 0480 S2 measured `3.4e-16`).
8. **The cylinder clause, the predicted miss** (C1). The quarter ring of B1s with `t = 0.01` (`t/R = 0.01`, thin) under the
   internal follower pressure `p_ref = E t (1.51^2 - 1)/(2 R) = 1.2801e9` Pa in 20 equal steps, through
   `nonlinear::solve_with` with the follower as a contribution: by D0 the first call returns `NotYetSupported` (element kind
   Shell4) before any iteration. Had the driver a nonlinear shell, arithmetic 6 predicts `p/p_thin - 1 = 1/cos(pi/96) - 1 =
   +5.4e-4` at every stretch, within the 1 percent of the acceptance.

## Design

### D1. `FollowerPressure::on_shells` (`follower.rs`)

`FollowerPressure::on_shells(model, set, pressure) -> Result<FollowerPressure>`: the elements of the element set `set` (the set
of a `Load::ShellPressure`; unknown set refused, `FemError::UnknownSet` of kind element), each a Shell4 or a Shell3 (another
kind refused, key `"follower pressure"`, naming the element and its kind), a non-finite pressure refused (key `"pressure"`);
each element becomes a face of the struct with its nodes in element order, the three translational equations of each node
(`MissingComponent` if one is absent) and their reference coordinates. A private flag marks the struct as a shell pressure; the
face routines receive `-(load_factor * pressure)` for it and `load_factor * pressure` (the present expression, unchanged) for
the faces of solids, so that `force`, `load_stiffness`, the `Contribution` impl and `buckling_factors` serve both, and the solid
path executes the operations it executed before (N1). The name of the contribution: `follower pressure on the shells of
'<set>'`. `pressure()` returns the given pressure.

The documentation states: the Newton driver refuses shell models (D0), so on shells the contribution is offered for a driver
that takes them (the follow-up) and for checks; `force` and `load_stiffness` (the full matrix) are the follower load and its
load stiffness on the current mid-surface; `buckling_factors` takes shell follower pressures through the existing buckling path
(ADR 0473 D6 takes shells). The module documentation names the shell form.

No change to `Load`, `loads.rs`, `shell.rs`, `shell3.rs`, `nonlinear.rs` or `buckling.rs`: the dead `ShellPressure` and every
other path are untouched.

### D2. The element tests (S1s, S3s, S4s, W1, K1)

Models of one element: Shell4 `(0, 0, 0), (1, 0, 0.06), (1.1, 0.9, 0), (-0.1, 1, 0.08)` (warped, not a parallelogram) and
Shell3 `(0, 0, 0), (1, 0.1, 0.05), (0.2, 0.9, -0.03)`; patches: a Shell4 2 x 2 and a Shell3 2 x 2 x 2 (each square cut along
the diagonal from its first to its third corner) on the cylinder `r = 2` over `theta` in `[0, 0.5]`, `z` in `[0, 1]`; thickness
`0.01`, steel `E = 2e11, nu = 0, rho = 7850`, the element set "all", `p = 3e7`. S1s: at `u` with every unknown (rotations too)
uniform in `[-0.05, 0.05]` (SplitMix64, seed `0xF573_0487`, the elements in the order Shell4 element, Shell3 element, Shell4
patch, Shell3 patch), the dense `K = load_stiffness(u, 1)` against `K_cd` column by column, `(f(u + h e_j) - f(u - h e_j))/(2
h)`, `h = 1e-6`; measure `max |K - K_cd| / max |K|`. S3s: on the two single elements, `max |K - K^T| / max |K|`. S4s: flat
elements (the Shell4 `(0, 0, 0), (1.2, 0.1, 0), (1.0, 0.9, 0), (-0.1, 1.1, 0)` and the Shell3 `(0, 0, 0), (1, 0.1, 0), (0.2,
0.9, 0)`, each turned by the rotation of the Rodrigues vector `(0.3, -0.5, 0.7)`): `force(0, 1)` against `nodal_load_vector`
of `Load::shell_pressure("all", p)` on the same model, `max |f - f_dead| / max |f_dead|` over every equation (the dead
moments, rounding, included). W1: the warped single Shell4 element at `u = 0`: the resultant of the translational forces
against the dead one, `|F - F_dead| / |F_dead|`; the nodal `max |f_a - f_a,dead| / max |f_dead|` recorded. K1: the Shell4 and
Shell3 single elements at the S1s state: `Contribution::add` into a zero force and an empty triplet list gives the force `-f(u)`
(bits) and the triplets whose dense sum equals `-(K + K^T)/2` of the dense `load_stiffness` (bits; one element, no repeated
entry).

### D3. The cylinder (C2, C1, C1r)

C2: the full cylinder `R = 1`, `L = 0.4`, `N = 64` around, `n_z = 4`, Shell4 (and Shell3, each rectangle cut along its
diagonal from its first to its third corner), `t = 0.01`, `p = 1e6`, nodes `(R cos theta_i, R sin theta_i, z_k)` with the
facets `(i, k), (i + 1, k), (i + 1, k + 1), (i, k + 1)` (outward `e_3`, arithmetic 6). At `u = (lambda - 1)(X_x, X_y, 0)` on
the translations, `lambda = 1.5`: C2a `|sum f . e_r / (p N lambda R L sin(2 pi/N)) - 1|`; C2b `max over nodes of |f . e_theta|,
|f . e_z|` over `max |f|`; C2c the dead `ShellPressure` on the same model: `|lambda sum f_dead . e_r / sum f . e_r - 1|`, and
C2d `sum f . e_r / sum f_dead . e_r - 1 >= 0.49` (off by the factor `lambda`).

C1 and C1r: the quarter ring `R = 1`, `t = 0.01`, width `b = 0.05`, 24 Shell4 over `theta` in `[0, pi/2]` and 1 across the
width (nodes `(R cos theta_i, R sin theta_i, z)`, `z = 0, b`, facets ordered for an outward `e_3`), `nu = 0`; held: `u_z` and
the rotations about `x` and `y` on every node, `u_y` and the rotation about `z` on `theta = 0`, `u_x` and the rotation about `z`
on `theta = pi/2`. C1 (ignored, `c1_cylinder_follower_internal_pressure_known_miss`): `FollowerPressure::on_shells(model, "all",
p_ref)`, `p_ref = E t (1.51^2 - 1)/(2 R)`, `nonlinear::solve_with(model, &[], &[], NewtonOptions::new(20), contributions =
[follower])`; at every converged step `|p / p_thin(lambda) - 1| <= 1e-2` with `lambda` the mean of `|x_perp|/R` over the nodes
and `p` the step's load factor times `p_ref`, and the last `lambda >= 1.5`. C1r (regular test): the same call returns
`Err(FemError::NotYetSupported { item: "element kind", variant: "Shell4", .. })`, and on the same strip cut into Shell3
`variant: "Shell3"`.

### D4. Ring buckling with shells (B1s to B4s, S2s)

The C1 strip with `t = 0.01`: pre-stress of the linear static solve of the dead `Load::shell_pressure("all", -P_B)` (`P_B =
1e3`, the minus sign pressing inwards, against `e_3`), `Prestress::from_static`; B1s: `follower::buckling_factors(model, pre,
[FollowerPressure::on_shells(model, "all", -P_B)], BucklingOptions::new(2))`, `p_cr = mu_1 P_B` against `3 E I/R^3`, `I =
t^3/12`; B2s: `buckling::factors(model, pre, BucklingOptions::new(2))` against `4 E I/R^3`; B3s: the correlation of the radial
displacement of every node of the lowest mode of each with `cos 2 theta`, `>= 0.99`; S2s: the load stiffness at `u = 0`
reduced by `constraint::reduce` on `prescribed_equations(model, [])`, `max |K - K^T| / max |K|`; B4s (record): B1s and B2s on
the strip cut into Shell3.

### D5. The bit record of the default (N1)

`crates/rcfd-fem/tests/sa_shell_follower_default_bits.rs`, written after this commit and run on the unchanged base fb7c5d94
before any library code; its digests (FNV-1a 64 over the bits) written into the file then. Content: `nodal_load_vector` of
`Load::ShellPressure` on a flat and a warped Shell4 and a Shell3 element, and of `Load::ShellPressureAngleWeighted` on the
Shell3; `solve_static` of a Shell4 plate under `ShellPressure`; `buckling::factors` of the C1 strip under the dead external
pressure (2 factors and modes); the solid `FollowerPressure` on a Hex8 and a Tet10 element at a deformed state: `force`,
`load_stiffness`, `Contribution::add`; `follower::buckling_factors` of a small Hex8 column (`hex_box` of
`0.1 x 0.1 x 1` in 1 x 1 x 4 cells, clamped base, the follower pressure on `zmax`). Every value in a
fixed order.

## Checks (limits fixed now)

| Id | Clause | What | Limit | Estimate behind the limit |
|---|---|---|---|---|
| S1s | a1 | `max |K - K_cd| / max |K|`, Shell4 element, Shell3 element, Shell4 patch, Shell3 patch (D2) | `<= 1e-6` each | rounding only, `~2e-10` (arithmetic 3) |
| S3s | control | single elements: `max |K - K^T| / max |K|` | `>= 1e-3` each | free edges: the edge term is of the order of `K` |
| S4s | D1 | flat Shell4 and Shell3: `max |force(0, 1) - f_dead| / max |f_dead|` | `<= 1e-10` each | rounding, `~1e-15` (arithmetic 4) |
| W1 | D1 | warped Shell4 at `u = 0`: resultant `|F - F_dead| / |F_dead|`; nodal difference recorded | `<= 1e-10`; recorded | rounding (arithmetic 5); the nodal difference of the order of the warp |
| K1 | D1 | the contribution: force `-f(u)` and tangent `-(K + K^T)/2`, Shell4 and Shell3 elements | equal bits | the same operations |
| C2a | a2 (companion) | cylinder at `lambda = 1.5`: `|sum f . e_r / (p N lambda R L sin(2 pi/N)) - 1|`, Shell4 and Shell3 | `<= 1e-11` each | rounding over `~1e3` terms (arithmetic 6) |
| C2b | a2 (companion) | the same: `max |f . e_theta|, |f . e_z|` over `max |f|` | `<= 1e-11` each | symmetry, rounding |
| C2c | a2 (control) | dead `ShellPressure` on the same mesh: `|lambda sum f_dead . e_r / sum f . e_r - 1|` | `<= 1e-11` each | the reference polygon, rounding |
| C2d | a2 (control) | `sum f . e_r / sum f_dead . e_r - 1` | `>= 0.49` each | `lambda - 1 = 0.5` |
| C1 | a2 | quarter cylinder of Shell4 under follower internal pressure to the stretch 1.5 by the Newton driver: `|p/p_thin - 1|` every step, last `lambda` | `<= 1e-2` each; `>= 1.5` | **predicted miss** (D0): the driver refuses shells; ignored `*_known_miss`, run once to record the refusal |
| C1r | a2 (diagnosis) | the C1 call returns `NotYetSupported` (element kind Shell4; Shell3 on the triangle strip) | holds | `check_elements(shells = false)` (D0) |
| S2s | B (condition) | reduced `K_p` of the B1s strip at `u = 0`: `max |K - K^T| / max |K|` | `<= 1e-11` | rounding (arithmetic 7) |
| B1s | companion | shell ring buckling with the load stiffness: `|p_cr / (3 E I/R^3) - 1|` | `<= 2e-2` | `<= 5e-3` (arithmetic 7) |
| B2s | control | without (existing `buckling::factors`): `|p_cr / (4 E I/R^3) - 1|` | `<= 3e-2` | `<= 5e-3` |
| B3s | companion | lowest modes of B1s, B2s: correlation with `cos 2 theta` | `>= 0.99` each | `n = 2` lowest |
| B4s | record | B1s and B2s on the Shell3 strip | recorded | |
| N1 | a3, b2 | the digests of D5 at the base | exact | the dead paths are not edited; the solid follower executes the same operations |
| N2 | b2 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | no shared output path changed |

Files: `crates/rcfd-fem/tests/sa_shell_follower.rs` (S1s to K1, C2, C1, C1r, B1s to B4s, S2s),
`sa_shell_follower_default_bits.rs` (N1). Every test is small (the largest is the 64 x 4 cylinder, a force evaluation, and the
24-element strip's buckling); no heavy record.

## Planned mutations (on a scratch copy, one at a time)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the shell faces take the solid sign (`-p` not applied) | S4s, C2a, B1s |
| MU2 | the shell negation in the force only (the stiffness keeps the solid sign) | S1s, B1s |
| MU3 | the shell faces take the equations of the rotations (components 3 to 5) | S4s, C2a |
| MU4 | the shell faces on the reference positions (`u` ignored for shells) | S1s, C2a |
| MU5 | `on_shells` skips the Shell3 elements of the set | S1s (Shell3), S4s, C2a (Shell3) |
| MU6 | `buckling_factors` ignores its pressures | B1s |

## Decision rule

- **done**: every check of the table met (C1 included), every planned mutation caught by at least one check, fmt and clippy
  clean, N1 and N2 unchanged.
- **partial**: N1 and N2 unchanged, and S1s, S4s, K1 and C2a to C2d met, with C1 missed as predicted by D0 (recorded as the
  ignored `*_known_miss` and C1r met) and any other miss diagnosed and recorded as a `*_known_miss` test.
- **todo**: otherwise (in particular any change of the default bits).

By D0 the outcome expected is **partial**: clause (a2) needs a geometrically nonlinear shell, which this item does not build.

## Limits (not covered)

- No geometrically nonlinear shell (D0); the follower shell pressure cannot be used in the Newton driver until one exists.
- Uniform pressure on whole elements; no pressure field, no follower traction on shells, no closed-volume term.
- The follower loads the interpolated mid-surface through the nodes; on a warped Shell4 at zero displacement it differs from
  the dead `ShellPressure` (projected facet and offset moments) in the nodal distribution, not in the resultant (W1). A shell
  section offset does not exist in the crate, so the loaded surface is the nodal mid-surface.
- The driver would use the symmetric part of the load stiffness (ADR 0480 limits).
- No case-file key.

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. **The form of the API.** Options: (a) a constructor `FollowerPressure::on_shells` on the existing type; (b) a new
   `Load` variant `ShellPressureFollower`. Taken (a): a `Load` variant changes the model's public enum that the case reader,
   the schema and the API crates match on, the maintainer's call; (a) reuses the contribution, the load stiffness and the
   buckling path of ADR 0480 unchanged.
2. **The cylinder clause.** Taken: a predicted miss with a nonlinear-shell follow-up (D0; the brief b1).
3. **Warped facets.** Taken: the follower loads the bilinear mid-surface, with no offset moments (arithmetic 5); the dead
   path is left as it is.

## Amendment 1 (2026-11-14): two defects of the registered set-up, found by run 1; written before the corrected run

Run 1 (`sa_shell_follower` once at 5a3ce1f7, `--release -- --include-ignored --nocapture --test-threads 1`,
`f573-scratch/run1.log`) missed two checks whose registered expectation was wrong, not the code. Neither limit is changed; the
two tests of run 1 that contain them become ignored records with their bodies unchanged and the suffix `_known_miss`
(`s1s_s3s_k1_load_stiffness_against_central_differences_known_miss`, `c2_cylinder_at_a_prescribed_stretch_known_miss`), and
corrected tests are added.

1. **K1 tangent (both elements), "equal bits".** The contribution's triplets cover the translational equations only; the
   test's reference `-0.5 (K_ij + K_ji)` is formed at every entry of the dense matrix, so at the rotational entries it is
   `-0.0` while the dense sum of the triplets is `+0.0`: a sign of zero produced by the test, not a difference of the
   operations. Correction K1': the same comparison with `x + 0.0` applied to both sides (which maps `-0.0` to `+0.0` and leaves
   every other value unchanged), bits.
2. **C2b Shell3 (`8.19e-3`, limit `1e-11`).** Arithmetic 6 claimed that every node's `f . e_theta` vanishes by symmetry. For
   the Shell4 that holds; for the Shell3 mesh, each rectangle cut along the diagonal from its first to its third corner, a node
   of an end ring `z = 0` is corner `a` (two triangles) of the rectangle on its `+theta` side and corner `b` (one triangle) of
   the one on its `-theta` side (the top ring the reverse), so its two sides carry `2` and `1` thirds of `p A_tri n` with normals
   at `+-pi/N`: a tangential force `p A_tri sin(pi/N)/3` at each end node, opposite at the two ends. Its size against `max |f|`
   is about `sin(pi/64)/6 = 8.2e-3`, the measured value. Interior nodes have three triangles on each side and no tangential
   force; the net moment about the axis vanishes (the two ends cancel; each triangle pair has its force through the axis).
   Correction C2b' (Shell3): `max |f . e_theta|, |f . e_z|` over the nodes of the interior rings `0 < k < n_z`, over `max |f|`,
   `<= 1e-11`; and `|sum_nodes f . e_theta| / max |f| <= 1e-11` (the moment about the axis over `R`; `320` terms).

The corrected tests `s1s_s3s_k1_corrected` and `c2_cylinder_corrected` re-execute the checks of run 1 that were met (S1s, S3s,
K1 force; C2a, C2b Shell4, C2c, C2d) with the same deterministic inputs as regression guards: their registered measurements are
those of run 1. Each corrected test is run once.

## Addendum 1 (2026-11-14): results

Runs: run 1, `sa_shell_follower` once at 5a3ce1f7 (`--release -- --include-ignored --nocapture --test-threads 1`,
`f573-scratch/run1.log`); after amendment 1 (8027d0db), run 2, the two corrected tests once at 8586a41e
(`f573-scratch/run2-corrected.log`). N1 was recorded at the base fb7c5d94 (`n1-base.log`) before any library code and passed
at 8586a41e in the whole-crate run. Then the whole `rcfd-fem` suite in release (`fem-suite.log`), `baseline_hashes` of
`rcfd-api` (`baseline.log`) and `docs_site` of `rcfd-cli` (`docs_site.log`): `rcfd-fem` 636 passed, 0 failed, 159 ignored (it runs the registered files a second time, both passed); `baseline_hashes` 3 passed, 1 ignored; `docs_site` 2 passed. No limit was changed after a run.

| Id | Limit | Measured | Met |
|---|---|---|---|
| S1s Shell4 element, Shell3 element, Shell4 patch, Shell3 patch | `<= 1e-6` | `2.50e-10, 1.79e-10, 1.88e-10, 1.99e-10` | yes |
| S3s the two elements | `>= 1e-3` | `1.94, 2.00` | yes |
| S4s flat Shell4, flat Shell3 | `<= 1e-10` | `1.9e-16, 5.0e-16` | yes |
| W1 warped Shell4 resultant | `<= 1e-10` | `2.4e-16` | yes |
| W1 nodal difference | recorded | `2.30e-2` of `max |f_dead|` | recorded |
| K1 force `-f`, both elements | bits | equal | yes |
| K1 tangent (run 1) | bits | differs at the rotational entries by the sign of zero (amendment 1) | missed, a test defect; record `s1s_s3s_k1_load_stiffness_against_central_differences_known_miss` |
| K1' tangent with `x + 0.0` (run 2) | bits | equal | yes |
| C2a Shell4, Shell3 | `<= 1e-11` | `2.2e-15, 2.2e-15` | yes |
| C2b Shell4 | `<= 1e-11` | `1.1e-15` | yes |
| C2b Shell3 (run 1) | `<= 1e-11` | `8.19e-3` (the end rings, amendment 1) | missed, a set-up defect; record `c2_cylinder_at_a_prescribed_stretch_known_miss` |
| C2b' Shell3 interior rings; `|sum f . e_theta|` (run 2) | `<= 1e-11` each | `1.05e-15`; `4.2e-15` | yes |
| C2c Shell4, Shell3 | `<= 1e-11` | `2.9e-15, 3.6e-15` | yes |
| C2d Shell4, Shell3 | `>= 0.49` | `0.5000, 0.5000` | yes |
| C1 cylinder by the Newton driver | `<= 1e-2` each, last `>= 1.5` | not reached: `NotYetSupported { item: "element kind", variant: "Shell4", package: "a follow-up of ADR 0432" }` at the first call | **missed, as predicted (D0)**; ignored `c1_cylinder_follower_internal_pressure_known_miss` |
| C1r the refusal, Shell4 and Shell3 strips | holds | holds (`Shell4`, `Shell3`) | yes |
| S2s reduced `K_p` asymmetry | `<= 1e-11` | `4.3e-16` | yes |
| B1s Shell4 with the load stiffness | `<= 2e-2` | `3.31e-3` (`p_cr = 3.00993 E I/R^3`) | yes |
| B2s Shell4 without | `<= 3e-2` | `3.68e-3` (`4.01471 E I/R^3`) | yes |
| B3s correlation with `cos 2 theta` | `>= 0.99` | `1.0000, 1.0000` | yes |
| B4s Shell3 with; without | recorded | `+2.76e-3`; `+3.13e-3` (correlations 1.0000) | recorded |
| N1 default bits | exact | the 10 digests equal | yes |
| N2 `baseline_hashes` | pass | pass (3 passed, 1 ignored) | yes |

### Mutations (scratch copy of 8586a41e by `git archive`, one at a time, `f573-scratch/mutate.py`, `mutations.log`)

| Id | Caught by |
|---|---|
| MU1 the shell faces with the solid sign | S4s (`2.0`), W1, C2a (`2.0`), C2c, C2d, B1s (`1.009`); not S1s (force and stiffness flip together) |
| MU2 the stiffness keeps the solid sign | S1s (`2.0`, all four), B1s (`1.009`) |
| MU3 the equations of the rotations | S4s, W1, C2a to C2d, S2s, B1s (`0.338`); not S1s (consistent) |
| MU4 the force on the reference positions | S1s (`1.0`, all four), C2a (`0.333`), C2c, C2d |
| MU5 the Shell3 elements skipped | S1s and S3s Shell3 (NaN), S4s Shell3 (`1.0`), C2a to C2d Shell3 |
| MU6 `buckling_factors` ignores its pressures | B1s (`0.338`) |

Six of six caught, each by a check it was planned for.

### Deviations

- Amendment 1: two registered expectations were wrong. The K1 tangent reference made `-0.0` at the rotational entries, and
  for C2b on Shell3 the end rings of the triangulation carry a tangential force, so arithmetic 6 was wrong for that mesh. The
  run 1 tests are kept as ignored records, and the corrected checks met. The corrected tests re-execute the met checks of
  run 1 with the same inputs and gave the same numbers.
- None other.

### Not run

- C1 could not run (D0): there is no geometrically nonlinear shell. The whole workspace suite was not run (the brief).

### Decision

**partial** by the registered rule. N1 and N2 are unchanged, and S1s, S4s, K1 (corrected) and C2a to C2d met. C1 missed as
predicted by D0 (recorded as an ignored `*_known_miss`, with C1r met), and the two set-up defects of run 1 are diagnosed and
recorded as `*_known_miss` records. Every other check met, six of six mutations were caught, and fmt and clippy are clean.
Clauses (a1) and (a3) of the acceptance are met. Clause (a2) is not met, because the crate has no nonlinear shell.

### Remaining questions (recommendations for the lead)

- **Q1** (the cylinder clause): options (a) mark F-573 partial and add a follow-up for geometrically nonlinear shells that
  carries the cylinder clause; (b) mark F-573 done and move the clause to that follow-up. Recommendation (a): the clause is
  part of the acceptance and was not met.
- **Q2** (a `Load` variant or a case-file key for the follower shell pressure): not added (open question 1 taken as (a)).
  Recommendation: decide together with the case-file key of ADR 0480 Q1, after the nonlinear shell exists.
- **Q3** (warped facets): the follower and the dead load agree in resultant but differ by `2.3e-2` in the nodal distribution on
  the registered warped element. Recommendation: accept (the follower is the consistent load of the bilinear surface).

## For the lead: roadmap entries

Proposed status of F-573: **partial**. Note:

> Structural solver, advanced tier. Follow-up of F-424 (ADR 0480, 2026-11-14). PARTIAL. ADR 0487, 2026-11-14: FollowerPressure::on_shells, the follower form of the shell pressure on the current mid-surface of Shell4 and Shell3 elements (f_a = +p int N_a (x_u x x_v) on the translations, no moments), with its full load stiffness, by the face arithmetic of ADR 0480 with the pressure negated; a Contribution of the Newton driver, also accepted by follower::buckling_factors. Load stiffness against central differences 1.8e-10 .. 2.5e-10 on a Shell4 and a Shell3 element and patch (limit 1e-6); at zero displacement equal to the dead ShellPressure within 5e-16 on flat elements, the resultant within 2.4e-16 on a warped Shell4. Cylinder of 64 facets at the prescribed stretch 1.5: the radial resultant p N lambda R L sin(2 pi/N) within 2.2e-15, the dead load off by lambda. Ring buckling of Shell4 with the shell load stiffness 3.0099 E I/R^3 (3 E I/R^3, limit 2 percent), 4.0147 without (4 E I/R^3, limit 3 percent). The cylinder clause (the thin law to the stretch 1.5 by the Newton driver) not met: the nonlinear driver takes no shells (no geometrically nonlinear shell in the crate), recorded as a predicted known miss; follow-up proposed. Dead ShellPressure and default bits unchanged (10 digests), baseline hashes unchanged; six of six mutations caught. Two set-up defects of run 1 corrected by amendment.

Follow-up (title, deps, acceptance), for the lead to number or drop:

> (new item, number for the lead) Geometrically nonlinear shells in the Newton driver (deps F-573). Every limit not stated here is registered in the ADR before the run. Shell4 and Shell3 gain an internal force and a consistent tangent at finite displacements and rotations (a corotational facet formulation, its rotation update registered), taken by nonlinear::solve and solve_with with every default bit-identical; the tangent against central differences of the internal force within 1e-6; a cantilever strip of shells under a tip moment rolls up to a full circle within 1 percent of the closed-form tip position; a thin cylinder of shells under the follower internal pressure of ADR 0487 follows p = E t (lambda^2 - 1)/(2 R) within 1 percent to the stretch 1.5 (the clause of F-573).
