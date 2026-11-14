# ADR 0500: F-387 registration: demonstration on a fan blade, static response to centrifugal and pressure load and the modes at rest; user page

Status: **REGISTERED 2026-11-14, written before any code of F-387 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no test or library code of F-387 was written in the repository. One feasibility probe of the
meshers was run outside the repository before this text (disclosed under "Probe"): it built blade surfaces, ran the tetrahedral
mesher and counted elements; it assembled no matrix, solved nothing and computed no quantity that a check below compares. Results
go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-387 (deps F-371, F-375, F-376, F-383, F-386, F-132), the stub [ADR 0288](0288-fan-blade-static-and-modal-demonstration.md)
of the basic-tier plan [ADR 0270](0270-structural-solver-basic-tier-plan.md) (work package SB-18). Number 0500 given by the lead.
Branch `f387-work` from b1629305, worktree `w49i-work/f387`, scratch `w49i-work/f387-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact and the measured number); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code, the crate graph
unchanged; no existing test edited; clean-room. The files of F-428 (shell and laminate), F-563 (rotor bearings) and F-576
(viscoelastic time stepping, nonlinear driver) are not touched. **No library file is changed by this item** (design below): the
work is one test file, the user page and two documentation edits.

## What F-387 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. One blade of the parametric fan geometry, meshed in quadratic tetrahedra and clamped at the hub: under the centrifugal load at a registered speed the reactions equal rho omega^2 V r_c within 1e-8 and under a uniform pressure the integrated pressure force within 1e-8; the strain energy equals half the work of the loads within 1e-8; between two meshes (the second with at least twice the elements) the tip displacement and the first natural frequency change by less than 5 percent, both numbers recorded; as the flat-plate limit, an untwisted rectangular cantilever plate of the blade's span, mean chord and thickness has its first bending frequency between the Euler-Bernoulli beam value and that value divided by sqrt(1 - nu^2), with a 1 percent margin on each side. The page docs/structural-solver.md states the scope, the limits and that nothing is validated against a measured blade.

Clauses: (a0) every limit registered here; (a1) one blade of the parametric fan geometry in quadratic tetrahedra, clamped at the
hub; (a2) centrifugal load at a registered speed: reactions against `rho omega^2 V r_c`, `1e-8`; (a3) uniform pressure: reactions
against the integrated pressure force, `1e-8`; (a4) strain energy against half the work of the loads, `1e-8`; (a5) two meshes, the
second with at least twice the elements: tip displacement and first natural frequency change by less than 5 percent, both
recorded; (a6) the flat plate of the blade's span, mean chord and thickness: first bending frequency in `[0.99 f_EB, 1.01 f_EB /
sqrt(1 - nu^2)]`; (a7) the page `docs/structural-solver.md` states scope, limits and that nothing is validated against a measured
blade.

## Readings (registered)

- **(a1) The blade.** `rcfd_mesh::fan_design::ComputerFan::default()` (the 120 mm, 7-blade fan of ADR 0118, "an illustration of
  the schema, not a measured fan"), blade 0 as `AxialFan` by `axial_fan(0, 0.0, 0.0)`: hub radius `r_hub = 21.5 mm`, tip radius
  `r_tip = 58 mm`, span `L = 36.5 mm`, NACA 4-digit sections, chord 22, 29, 36 mm, stagger 40, 52, 62 deg, thickness 5.5, 4.2,
  3.5 percent of the chord, skew 0, 4, 8 deg at span 0, 0.5, 1 (linear between, `AxialFan::station_at`). Axis `z` through the
  origin. The solid is the region between the suction and pressure surfaces of `AxialFan::surface_zs` on the cylinders `r_hub <=
  r <= r_tip`, closed by the hub and tip caps (the same construction as `AxialFan::blade_surface`).
- **(a1) Quadratic tetrahedra, primary route S (structured).** The test builds Tet4 elements directly on the parametric surface
  (D1 below) and lifts them with `rcfd_fem::builders::upgrade_to_tet10` (mid-edge nodes at the midpoints of the straight edges, not
  projected onto the curved blade surface). Two meshes: **M1** with `n_c = 12` chordwise cells, `n_t = 2` through the thickness,
  `n_s = 8` spanwise, and **M2** with `(24, 2, 16)`: M2 has four times the elements of M1 (every chordwise and spanwise cell split in
  two; the thickness layers kept, the Tet10 being quadratic through the thickness). Element counts (D1): M1 `6 (n_c - 2) n_t n_s +
  3 * 2 n_t n_s = 1056`, M2 `4416`.
- **(a1) Supplementary route D (the tetrahedral mesher of F-132).** `blade_surface(16, 20)` tagged (hub cap 1, tip cap 2, pressure
  side 3, the rest 0), `rcfd_mesh::tetmesh::tetrahedralize` with `TetOptions { refine: false, ..Default::default() }`,
  `rcfd_fem::convert::from_tet_mesh` (F-371), `upgrade_to_tet10`. Why route S is the primary one: see "Probe". Route D runs the
  identity checks R1, P1, E1 and records its tip displacement and first frequency against M2 (no limit on those). A miss of a
  route-D check is recorded as a known miss and does not by itself change the decision (registered here, decision rule).
- **(a1) Clamped at the hub.** Every node of the hub cap (`r = r_hub`, the node set `hub`, mid-edge nodes included) held in all
  three components (`Constraint::displacement("hub", c, 0.0)`, `c = 0, 1, 2`). The plain elimination path.
- **Material (nominal, not data).** Isotropic elastic, PBT-like: `E = 2.5 GPa`, `nu = 0.38`, `rho = 1300 kg/m^3` (the density of
  `ComputerFan::default()`), no expansion. These are round numbers of the order of an unfilled PBT; they are not taken from a data
  sheet and are not data of any fan.
- **(a2) The registered speed.** `2000 rpm`, `omega = 2000 * 2 pi / 60 = 209.43951 rad/s` (33.3 Hz, within the speed range of a 120
  mm computer fan). `Load::centrifugal([0, 0, 1], [0, 0, 0], omega)`.
- **(a2) Which `V r_c`.** The vector `S = sum_e V_e c_e` over the Tet4 corners of the mesh (`V_e` the volume of element `e`, `c_e`
  its corner mean), with only its components normal to the axis (`S_perp = (S_x, S_y, 0)`): the first moment of the **meshed**
  volume about the axis, computed in the test from the node coordinates, independently of the library. `V = sum_e V_e`, `r_c =
  |S_perp| / V`. For straight-edged Tet10 the body-force load sums exactly to `rho omega^2 S_perp` (the shape functions sum to one
  and the integrand is linear), so the comparison is an identity of the discretisation. The check: `e_R = |sum_hub R + rho
  omega^2 S_perp| / (rho omega^2 |S_perp|)` with `R` the library's reactions (`SolidResult::reactions`, `K u - f` at the held
  equations), summed per component over the hub nodes. The sign: the reaction balances the outward centrifugal load.
- **(a3) The uniform pressure.** `p = 50 Pa` (an order of the static pressure of a 120 mm fan) on the pressure side (surface set
  `pressure`, every face of the pressure surface, leading and trailing edges included, caps excluded), `Load::pressure("pressure",
  50.0)` (positive presses on the solid). "The integrated pressure force" is `F_p = -p sum_f A_f` with `A_f` the outward area
  vector of the flat pressure face `f`, computed in the test from the corner coordinates (`(b - a) x (c - a) / 2`, oriented out of
  its element). The check: `e_P = |sum_hub R + F_p| / |F_p|`.
- **(a4) Strain energy and work.** For each load case separately, `U = IntegrationPointField::compute(model, u).strain_energy()`
  (ADR 0277, `sum w det J eps . sigma / 2`), `W = sum_i f_i u_i` with `f = rcfd_fem::loads::nodal_load_vector(model, &[])` (the
  consistent loads the solve used; the reactions do no work because `u = 0` where they act). The check: `e_E = |U - W/2| / (W/2)`.
- **(a5) Tip displacement.** Under the pressure load: `d_tip = |u_bar|`, `u_bar = (1/A_tip) int_tip u dA` the area mean over the
  tip cap faces (surface set `tip`); for a straight-sided six-node triangle `int N_corner dA = 0` and `int N_mid dA = A/3` (the
  closed form of the quadratic Lagrange triangle), so `int_tip u dA = sum_f (A_f / 3) (u_m1 + u_m2 + u_m3)`. Also under the
  centrifugal load (`d_tip^c`), with the same limit (C1b). Change: `|d_tip(M2) / d_tip(M1) - 1|`.
- **(a5) First natural frequency.** `modal_analysis` with `ModalOptions::new(4).tolerance(1e-12).residual_test(ResidualTest::
  Backward)` (ADR 0354; envelope factorisation, the default; shift by the default rule: the hub holds every rigid motion) on the
  clamped blade without load (modes at rest). `f_1` is `frequencies[0]`. Change: `|f_1(M2) / f_1(M1) - 1|`. The effective mass
  fractions of the four modes (`(phi^T M r_k)^2 / (r_k^T M r_k)`, `r_k = mass::rigid_motions(model, [0; 3])[k]`, six rigid motions)
  are recorded to say what each mode is.
- **(a6) The flat plate.** Span `L = r_tip - r_hub`; mean chord `c_m = (1/L) int c(r) dr`; thickness `t_m = V_b / (L c_m)` with `V_b
  = int A(r) dr` the volume of the parametric blade (`A(r) = AxialFan::section_area(r)`, exact section area of ADR 0065; the
  cylinder sections unroll without change of area, so `dV = A dr`), both integrals by 8-point Gauss-Legendre on each of the two
  station intervals (the chord is linear there, the area smooth). The plate: `tet_box([0; 3], [L, c_m, t_m], [16, 12, 2])` lifted to
  Tet10, clamped in all components on `xmin`, the same material. `f_b` is the frequency of the lowest mode whose `z`-translation
  effective mass fraction is at least 0.3 (a cantilever's first bending mode carries 0.613 of the mass; a torsion mode carries
  none). `f_EB = (beta_1 L)^2 / (2 pi L^2) sqrt(E t_m^2 / (12 rho))`, `beta_1 L = 1.8751040687` (the root of `1 + cos x cosh x = 0`,
  the constant of `sb_modal.rs`). The window: `0.99 f_EB <= f_b <= 1.01 f_EB / sqrt(1 - nu^2)`.

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0288 (the stub), ADR 0270 (head and the SB-18 section), ADR 0495 (the form), ADR 0397 (its
  result note: the floors of the residual and of the energy defect).
- `crates/rcfd-mesh/src/fan.rs` (`AxialFan`, `station_at`, `surface_zs`, `blade_surface`, `BladeSurface`), `fan_design.rs`
  (`ComputerFan::default`, `axial_fan`, `blade_mass`), `tetmesh.rs` (`TetOptions`, `TetMesh`, `tet_quality`), `surface.rs`
  (`TriSurface`).
- `crates/rcfd-fem/src/convert.rs` (`from_tet_mesh`, `split_hexahedra`), `builders.rs` (`tet_box`, `upgrade_to_tet10`), `mesh.rs`
  (`SolidMesh` API, `face_nodes`, `face_incidence`, `element_volume`, the six-node face), `model.rs` (`Load`, `Constraint`,
  `SolidModel`, `SolidResult`), `solve.rs` (`StaticSolver`, `StaticOptions::refinement`, the reactions), `constraint.rs`
  (`reactions`), `modal.rs` (`ModalOptions`, `ModalResult`), `mass.rs` (`consistent`, `rigid_motions`), `recover.rs`
  (`IntegrationPointField`, `strain_energy`).
- Tests: `sb_modal.rs` (the `Checks` helper, `BETA_L`, the effective-mass identification), `sb_convert.rs` (the mesher route).
- `crates/rcfd-cli/tests/docs_site.rs` (S1 to S3: every page built, no broken relative link, the index links a fixed list),
  `docs/index.md`, `docs/solid-case-file.md`, `docs/fan-project-breakdown.md` (item E5).

## Probe (disclosed, run before this text)

A scratch program (`w49i-work/f387-scratch/probe`, outside the repository, its own target directory) built `blade_surface(n_chord,
n_span)` of the blade above, tagged it as in route D, ran `tetrahedralize` and, on success, `from_tet_mesh` and `upgrade_to_tet10`,
and printed counts, `tet_quality` and the dimensions `L`, `c_m`, `t_m` (the latter with one Gauss rule over the whole span, not the
registered per-interval rule). Results:

| Surface | Options | Outcome |
|---|---|---|
| (16, 8) | default (refinement on) | "Steiner point budget exhausted", after 54 s |
| (16, 8) | `refine: false` | the same, after 57 s |
| (16, 20) | `refine: false` | 2263 Tet4, 791 points (89 surface Steiner points), 0.16 s; min dihedral 0.16 deg, max radius-edge 182, fraction of radius-edge `<= 2` 0.49; Tet10 4633 nodes |
| (12, 24) | `refine: false` | 4315 Tet4 (1039 surface Steiner points), 1.0 s; min dihedral 0.28 deg; Tet10 9295 nodes |
| (32, 40) | `refine: false` | budget exhausted, after 22 s |

The tag counts were as expected (pressure faces `2 n_chord n_span`). Dimensions: `L = 36.5 mm`, `c_m = 29.0 mm`, `t_m = 0.850 mm`
(one-rule value). **Consequence for the design:** the mesher meshes the thin blade only at some surface resolutions, never with
refinement, and with slivers (dihedral angles below 0.3 deg); no pair of successful resolutions found had an element ratio of at
least two. Two meshes with a controlled ratio and without slivers come from a structured split (route S); the mesher route is kept
as a supplementary run on the one surface that succeeded. No other resolution or option was tried, and route D's surface is fixed
here from that table, not searched further.

## Arithmetic done before the code (disclosed)

1. **Plate window (hand and one Python line for the arithmetic).** With `L = 0.0365 m`, `c_m = 0.029 m`, `t_m = 0.8499 mm` (probe
   value), `E = 2.5e9`, `rho = 1300`, `nu = 0.38`: `sqrt(E/rho) = 1386.75 m/s`, `t_m / sqrt 12 = 2.4535e-4 m`, `(beta_1 L)^2 =
   3.516015`, `f_EB = 3.516015 / (2 pi L^2) * 1386.75 * 2.4535e-4 = 142.91 Hz`; `1 / sqrt(1 - 0.38^2) = 1.08110`, upper value
   `154.50 Hz`; the window with the margins `[141.48, 156.04] Hz`. The test computes these from the registered integrals; the
   numbers here are the estimate only.
2. **Why the plate lies in the window (Kirchhoff plate, first principles).** For any admissible `w(x, y)` of the clamped-free plate
   the bending energy density `D (w_xx^2 + w_yy^2 + 2 nu w_xx w_yy + 2 (1 - nu) w_xy^2) / 2`, `D = E t^3 / (12 (1 - nu^2))`, is at least
   its minimum over `w_yy` with `w_xy = 0`, `E t^3 w_xx^2 / 24`; with that lower energy every strip `y = const` is an Euler-Bernoulli
   cantilever, whose Rayleigh quotient is at least `omega_EB^2`: so `omega_plate >= omega_EB`. The cylindrical field `w = W(x)` is
   admissible and gives the strip with `E / (1 - nu^2)`: so `omega_plate <= omega_EB / sqrt(1 - nu^2)`. The solid model adds shear
   deformation and rotary inertia, lowering the frequency by about `c (t/L)^2` with `c` of order 1 to 10, `(t/L)^2 = 5.4e-4`: at most
   about `5e-3`, inside the 1 percent margin. The finite elements are stiffer than the continuum (Rayleigh-Ritz): the discretisation
   error moves `f_b` up. From memory (not used as a reference, not checked): a cantilever plate of `L/b` near 1.26 lies a few
   percent above the beam value. Estimate `f_b / f_EB` between 1.01 and 1.06 with the Tet10 error of 0.5 to 2 percent, against the
   upper limit `1.0919`.
3. **Loads.** Centrifugal: `rho omega^2 V r_c ~ 1300 * 43865 * 9.0e-7 * 0.0398 = 2.0 N`. Pressure: `p L c_m ~ 0.053 N`. Tip
   deflection under pressure, strip estimate `p L^4 / (8 E t^3 / 12) = 8.7e-5 m` (linear range: 0.1 of the thickness).
4. **Identity floors.** The centrifugal and pressure loads of a straight-edged Tet10 sum exactly to `rho omega^2 S_perp` and `-p sum
   A_f` (shape functions sum to one; the face and volume rules integrate constants and linear functions exactly). The sum of the
   reactions equals minus the sum of the loads up to the residual of the solve and the rounding of `K u`. ADR 0397 measured, for
   cantilevers of `L/h` 5 to 40, a default relative residual up to `8e-9` and after one refinement step (`StaticOptions::
   refinement(1)`, the direct solve) a residual at the representability floor, `2e-13` to `1.8e-9`, and a recovered-energy defect of
   `1.6e-12` to `7.1e-10`. The blade (`L/t ~ 43`) sits at the slender end. The static solves are registered with `StaticSolver::
   Direct` and `refinement(1)`. Estimates: `e_R`, `e_P` `1e-11` to `1e-9`; `e_E` up to `1e-9`. Route D (slivers, larger floor):
   up to `1e-8`, at the limit.
5. **Mesh change.** Tet10 with two layers through the thickness represents the linear bending strain through the thickness; the
   in-plane error of M1 (chord cells 0.5 to 3.8 mm, span cells 4.6 mm) is estimated at 1 to 3 percent in `f_1` and `d_tip`, so the
   change M1 to M2 at 1 to 3 percent, against the limit 5 percent.
6. **Mesh volume.** The chordwise facets lose `1/3` of the first cell's area at the leading edge (`y_t ~ sqrt x`), `1.6e-3` of the
   section for `n_c = 12` (cosine spacing, `x_1 = 0.017`), plus the trapezoidal error elsewhere: `|V_M - V_b| / V_b` estimated
   `3e-3` (M1) and `1e-3` (M2).

## Design

### D1. The structured blade mesh (route S), in the test

Chord parameter `x_i = (1 - cos(pi i / n_c)) / 2`, `i = 0..n_c`; thickness fraction `w_k = k / n_t`, `k = 0..n_t` (`k = 0` suction,
`k = n_t` pressure); radius `r_j = r_hub + L j / n_s`, `j = 0..n_s`. The point `(i, k, j)`: `st = station_at(r_j)`, `(z_u, s_u) =
surface_zs(st, x_i, Suction)`, `(z_l, s_l) = surface_zs(st, x_i, Pressure)`, `(z, s) = (1 - w_k)(z_u, s_u) + w_k (z_l, s_l)`, point
`(r cos(s/r), r sin(s/r), z)` (the cap construction of `blade_surface`). At `i = 0` and `i = n_c` the two surfaces meet (NACA
closed edges): the points `(0, k, j)` for all `k` are one node per `j`, likewise `(n_c, k, j)` (merged by index; the node takes the
suction point). Every grid cell is split into the six Kuhn tetrahedra of the same index-space diagonal (`(0,0,0)` to `(1,1,1)`, as
`tet_box` does: conforming between neighbours); after the merge a tetrahedron with a repeated node is dropped. In a cell next to an
edge the collapsed face leaves a triangular prism and three of the six tetrahedra (the Kuhn split restricted to the prism), so the
count is `6 (n_c - 2) n_t n_s + 6 n_t n_s`. Each tetrahedron is oriented positive by its signed volume; a set-up check (S0)
requires that every tetrahedron had the same sign before orientation (an inverted cell would show as the other sign) and none is
degenerate. Sets: element set `all`; node sets `hub` (`j = 0`), `tip` (`j = n_s`); surface sets from `face_incidence` (boundary faces,
one member): `hub` (all corners at `j = 0`), `tip` (`j = n_s`), `pressure` (every corner at `k = n_t` or a merged edge node, not all
at `j = 0` or `j = n_s`). Then `upgrade_to_tet10`.

### D2. Route D, in the test

As "Readings", with the tags of the probe: hub cap = triangles with every vertex at radius `r_hub` within `1e-12` relative, tip
likewise, pressure = side triangles whose vertices are all in the loop positions `a = 0` or `a >= n_chord` of the rows
(`blade_surface` numbers row `j` point `a` as `j 2 n_chord + a`, `a < n_chord + 1` the suction side from the leading edge).

### D3. The checks, the plate and the records

One test file `crates/rcfd-fem/tests/sb_fan_blade.rs` (no `rcfd-io` is needed: the mesher and the blade are in `rcfd-mesh`, which
`rcfd-fem` depends on). Tests: `s_routes_and_m1_m2` (S0 to S2, R1, P1, E1 on M1 and M2, C1, C1b, C2, NC1 to NC4, records),
`d_mesher_route` (route D), `pl_flat_plate` (PL0, PL, NC5), and the ignored heavy record `heavy_m3_and_plate_refinement` (M3 =
`(24, 4, 16)` and the plate on `[24, 18, 3]`: the change of `f_1`, `d_tip`, `f_b` recorded, no limit). The `Checks` helper of
`sb_modal.rs` is copied; every aggregated error is required finite (a NaN is a miss).

### D4. Documentation

`docs/structural-solver.md` (new user page: scope of the structural solver as built, the blade demonstration, its limits, that
nothing is validated against a measured blade), linked from `docs/index.md`; item E5 of `docs/fan-project-breakdown.md` gets its
status text. No library documentation changes.

## Checks (limits fixed now)

| Id | Clause | What | Limit | Estimate |
|---|---|---|---|---|
| S0 | a1 set-up | M1, M2: every Tet4 of D1 of one sign before orientation, `min V_e / mean V_e > 0`; Tet10 accepted by `SolidMesh::add_block` | holds | D1 |
| S1 | a1 set-up | `|V_M - V_b| / V_b`, M1; M2 | `<= 2e-2`; `<= 5e-3` | `3e-3`; `1e-3` (arithmetic 6) |
| S2 | a1 set-up | M1, M2: pressure faces `2 n_c n_s`, hub faces `2 n_t (n_c - 1)`, tip the same; elements `1056`, `4416`; M2 at least twice M1 | equal; ratio `>= 2` | D1 |
| R1 | a2 | `e_R`, centrifugal at 2000 rpm, M1 and M2 | `<= 1e-8` each | `1e-11` to `1e-9` |
| P1 | a3 | `e_P`, 50 Pa on `pressure`, M1 and M2 | `<= 1e-8` each | `1e-11` to `1e-9` |
| E1 | a4 | `e_E`, each of the two load cases, M1 and M2 (four values) | `<= 1e-8` each | `<= 1e-9` |
| C1 | a5 | `|d_tip(M2)/d_tip(M1) - 1|`, pressure; both values recorded | `< 0.05` | 1 to 3 percent |
| C1b | a5 | the same under the centrifugal load | `< 0.05` | 1 to 3 percent |
| C2 | a5 | `|f_1(M2)/f_1(M1) - 1|`; both values recorded | `< 0.05` | 1 to 3 percent |
| MR | a5 set-up | the modal residuals of M1, M2 reported by the library, backward test | `<= 1e-12` (the option) | converged |
| PL0 | a6 set-up | the plate's first bending mode (D3 rule) is its mode 1 | holds | the flap mode is the lowest |
| PL | a6 | `0.99 f_EB <= f_b <= 1.01 f_EB / sqrt(1 - nu^2)` | both | `f_b / f_EB` 1.01 to 1.06 (arithmetic 2) |
| DR1, DP1, DE1 | a2 to a4, route D | as R1, P1, E1 on the mesher's mesh | `<= 1e-8` each | up to `1e-8` |
| DREC | record | route D: `d_tip`, `f_1` against M2, `tet_quality` | none | slivers stiffen |
| A7 | a7 | `cargo test -p rcfd-cli --release --test docs_site` with the new page linked from the index | pass | D4 |
| N0 | defaults | `cargo test -p rcfd-api --release --test baseline_hashes` and `cargo test -p rcfd-fem --release` | pass | no library change |

Every aggregated error is required finite.

## Negative controls

| Id | What | Limit | Estimate |
|---|---|---|---|
| NC1 | M1, centrifugal axis through `(1 mm, 0, 0)`: `e_R` against `S_perp` about the origin | `>= 1e-2` | `1 mm / r_c = 2.5e-2` (the difference is `rho omega^2 V d`) |
| NC2 | M1, pressure case: the reactions against the force of the suction-side faces, `|sum R + F_s| / |F_p|` | `>= 1` | about 2 (the closed surface: `A_s ~ -A_p`) |
| NC3 | M1, pressure case: `e_E` with `u` scaled by `1 + 1e-3` | `>= 5e-4` | `1e-3` |
| NC4 | M1 in Tet4 (no lift): `|f_1(Tet4 M1)/f_1(M2) - 1|` | `>= 0.10` | linear tetrahedra lock in bending: 30 percent or more |
| NC5 | the plate's second mode: `f_2 > 1.01 f_EB / sqrt(1 - nu^2)` (the window does not hold every mode) | holds | `f_2` about 2.5 `f_1` (torsion) |

## Planned mutations (on a scratch copy with its own `CARGO_TARGET_DIR`, one at a time)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the centrifugal body force with `omega` in place of `omega^2` (`loads.rs`) | R1 |
| MU2 | the pressure load halved (`loads.rs`) | P1 |
| MU3 | the energy density `0.5` replaced by `0.5 (1 + 1e-6)` (`recover.rs`) | E1 |
| MU4 | the density in the element mass times 1.25 (`mass.rs`) | PL (lower bound: `f_b` falls by 10.6 percent) |
| MU5 | the mid-edge node of `upgrade_to_tet10` at 0.45/0.55 of the edge in place of 0.5 (`builders.rs`) | R1 (the elements curve, the corner volumes no longer give the load), S1 not (it reads the corners) |

## Decision rule

- **done**: every check of the table except the route-D rows met, every negative control as stated, every planned mutation caught
  by at least one check, fmt and clippy clean, `docs_site` and `baseline_hashes` pass.
- **partial**: S0 to S2 met and R1, P1, E1 met on M1, with any other miss (C1, C1b, C2, PL, a mutation not caught) diagnosed and
  recorded as a `*_known_miss` test; or the page not passing.
- **todo**: the blade cannot be meshed in route S, or R1, P1 or E1 missed on M1.
- A route-D miss is recorded as `*_known_miss` and reported; it does not change the decision above.

## Limits (not covered)

- Nothing is validated against a measured blade: there is no measured or published reference for this blade (fan project
  breakdown, part G). The material numbers are nominal. The checks are identities of the discretisation, mesh convergence and the
  flat-plate bounds of a different body.
- Linear static and modes at rest only: no stress stiffening, spin softening or Coriolis terms of the rotating blade (other items:
  ADR 0395 and the rotating packages), no prestressed modes, no contact with the hub, no fillet at the root.
- The hub clamp holds every node of the cap: a rigid hub; the hub's own flexibility and the root fillet are not modelled.
- The mid-edge nodes are on straight edges: the curved blade surface is faceted (S1 measures the volume error). The pressure acts
  on the faceted surface.
- The tetrahedral mesher route meshes the thin blade only on some surfaces and with slivers (Probe); the structured split of D1 is
  a test helper, not a library mesher.
- The speed and the pressure are single registered values; no operating map.

## Open questions

1. **A case-file key for the demonstration.** The solid case file (F-386, `docs/solid-case-file.md`) has no centrifugal load and no
   blade mesh source. Options: (a) leave the demonstration a library test (as now); (b) a follow-up item adding `kind =
   "centrifugal"` (`axis`, `origin`, `speed`) to `[loads.<name>]` and a mesh source `generator = "fan_blade"` taking a
   `ComputerFan` and the D1 divisions; (c) only the load. Recommendation: (b), as a follow-up item; not taken here (the brief says
   no case-file key, and the solid schema's tests would change).
2. **A library blade mesher.** Options: (a) keep D1 in the test; (b) move it to `rcfd_fem::builders` (`fan_blade_tet10(&AxialFan,
   [n_c, n_t, n_s])`) with its own checks; (c) fix the tetrahedral mesher for thin bodies (F-132). Recommendation: (b) together with
   question 1, and (c) stays with F-132.

Each taken item in the addendum is marked "taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation)"; none of the two above is taken.

## Amendment 1 (2026-11-14, written after the first registered run and before the corrected run): the set-up check S0

The first registered run (`f387-scratch/run1.log`, test `s_routes_and_m1_m2`) missed the first part of S0: "every Tet4 of D1 of
one sign before orientation" counted 528 positive and 528 negative tetrahedra on M1, 2208 and 2208 on M2. **The check was
defective, not the mesh:** the six Kuhn tetrahedra of a cell are built by walking the three index directions in the six orders of
`PERMS`, and an odd order (`[0, 2, 1]`, `[1, 0, 2]`, `[2, 1, 0]`) lists the same kind of tetrahedron with the opposite orientation
(two vertices exchanged). `tet_box` corrects this by the parity flag of its `KUHN` table; D1 corrects it by the signed volume.
"One sign" must therefore be read after the parity: the corrected S0 requires that the sign of every tetrahedron's volume times the
parity sign of its order (`+1` even, `-1` odd) is the same for all tetrahedra of the mesh (an inverted cell would show as the other
product). The second part of S0 (no degenerate tetrahedron, met: `min V_e / mean V_e` 8.5e-3 on M1, 1.1e-3 on M2) and every other
check are unchanged.

Every other check of that run was met (numbers in addendum 1). The old test stays as an ignored record (`#[ignore]`, named in the
attribute as superseded by this amendment); the corrected test `s_routes_and_m1_m2_amended` repeats the same computation
(deterministic: the same numbers are expected for every other check) with the corrected S0, and is the test the gate runs. No limit
is changed.
