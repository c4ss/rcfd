# ADR 0633: F-686 registration: the free-sector pre-stress on tetrahedral, layered-shell and orthotropic sectors

Status: **REGISTERED 2026-11-16, written before any code of F-686 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-686 was written and nothing was run; the arithmetic below was done by
hand (no script; disclosed). Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-686 (dep F-683), the follow-up proposed by
[ADR 0611](0611-f683-registration-the-free-sector-prestress-on-solid-and-shell-sectors.md) (its remaining question 3). Number
0633 given by the lead. Branch `f686-work` from the local main tip d9dd237f, worktree `w49i-work/f686`, scratch
`w49i-work/f686-scratch`. A test-only item unless a run shows a defect of the library (then the fix is an amendment written
before the corrected run).

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number); each registered run made once; existing tests not edited (in
particular `sa_free_sector_prestress_f680.rs` and `sa_free_sector_prestress_solids_shells_f683.rs`); defaults bit-identical; no
edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code;
clean-room. The roadmap files are not edited. Every choice below that the maintainer would normally be asked about is taken on
the maintainer's standing instruction of 2026-11-11 (take the recommendation).

## What F-686 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. RotatingSector::new with free_sector_prestress on a free Tet10 sector, a free Shell4 layup sector and a free orthotropic solid sector gives the pre-stress of a constrained-copy route within 1e-10 and a residual at the interior nodes within 1e-10 of the load; the defaults bit-identical.

Clauses: **a0** every limit registered here; **a1** `RotatingSector::new` with `free_sector_prestress` on a free Tet10 sector, a
free Shell4 layup sector and a free orthotropic solid sector gives the pre-stress of a constrained-copy route within `1e-10`
(FS); **a2** and a residual at the interior nodes within `1e-10` of the load (RX); **a3** the defaults bit-identical (DF, NC0, B2,
B3).

## What was read

`AGENT_RULES.md`; the brief; ADR 0611 (all, its test file `sa_free_sector_prestress_solids_shells_f683.rs`: models, the
constrained-copy route, the checks FS, RX, RXc, OR, GN, WS, NC0s, the mutation scripts' sites), ADR 0606 D1 to D3 (through
ADR 0611). Sources at d9dd237f: `rotating.rs` (`RotatingSector::new`, `free_sector_unit_static`, `unit_centrifugal_model`,
`check_model`, `RotatingPart::operator`), `rotating/whirl.rs` (the Coriolis assembly: a Shell4 layup takes
`shell::model_coriolis` of its mass moments), `assemble.rs` (`stiffness` refuses no layup but takes only the isotropic
elasticity; `point_stiffness`, crate-private, takes the orthotropic material per point), `loads.rs` (`shell_body_vector`: a
layup of more than one ply takes `shell::layup_body_load_vector_at` with `m0`, `m1`), `shell.rs` (`layup_body_load_vector_at`,
`facet`, `model_coriolis`, `thickness_or_error`, `membrane_thickness`, `check_shell_model`: a Shell3 block refuses a layup),
`geometric_struct.rs` (module note: the layup pre-stress over its total thickness, ADR 0502 D2), `model.rs`
(`check_elements`: a layup is no longer pending, `laminate::PENDING = None`), `mat_ortho.rs` (`check_cyclic_axes`,
`cylindrical_axes`, `without_expansion`), `material.rs` (`MaterialAxes::cylindrical`), `builders.rs` (`tet_box`,
`upgrade_to_tet10`: surface sets kept, node sets extended by the mid-edge nodes), `mesh.rs` (`Ply::new`, `BlockProperty::layup`);
the tests `sa_laminate_body.rs` (the ply data of ADR 0496 and the two-density layup of ADR 0502 arithmetic 1) and
`sa_ortho_cyclic_prestress.rs` (the orthotropic constants of ADR 0465, cylindrical axes).

## Design

### D1. The three families under ADR 0606 D1 and D2 (derived)

ADR 0611 D1 showed: for an element set whose cyclic constraint maps translations (and shell rotations) by `R`, the cyclically
symmetric rigid motions are `r_1 = (a, 0)` and `r_2 = (a x (x - O), a)`; they are in the null space of every element stiffness
that has zero strain under a rigid motion; and the consistent centrifugal load `int N_i b dV`, `b = rho r_perp(x)`, has `r^T f =
sum_q w_q b(x_q) . r(x_q) = 0` at any rule because `r` is linear in `x` and interpolated exactly. Per family:

- **Tet10** (isoparametric, quadratic; here with curved mid-edge nodes, D2). The isoparametric functions reproduce linear fields
  exactly, so `r_1`, `r_2` at the nodes give the rigid field at every point, zero strain, and the load argument above holds at
  the element's mass rule. Same as Hex20 in ADR 0611. A free Tet10 sector is singular by exactly `span(r_1, r_2)` (a connected
  solid).
- **Orthotropic solid** (Hex20 with the orthotropic material of cylindrical axes about the cyclic axis, ADR 0462). The strain of
  a rigid motion is zero independently of the material, and the material is positive definite (checked by
  `Material::orthotropic`), so the null space is the same `span(r_1, r_2)`. The load depends on the density only: as for an
  isotropic solid. The cylindrical axes are about `(a, O)`, which `check_cyclic_axes` takes.
- **Shell4 layup** (ADR 0496, 0502). The stiffness of a section `[A B; B D]` (positive definite for valid plies) with the
  Mindlin shear and the drilling penalty: a rigid motion has zero membrane, bending, shear and drilling strain, so the null
  space is again `span(r_1, r_2)`. **The load is not that of ADR 0611.** For a layup of more than one ply the library's load
  (`layup_body_load_vector_at`) is, per node, the force `m0 int N_i b(x) dA` and the moment `m1 int N_i (e_3 x b(x)) dA`, `b` at
  the mid-surface point `x`, `m0 = sum rho_k t_k`, `m1 = sum rho_k t_k zbar_k`. Its work on `r_2`:

  `r_2^T f = m0 int b . (a x (x - O)) dA + m1 int (e_3 x b) . a dA = 0 + m1 int e_3 . (r_perp(x) x a) dA` (ω = 1).

  The first term is zero pointwise (ADR 0606 D2). The second is not zero in general: it is zero for `m1 = 0` (a layup symmetric
  in density about the mid-surface) and, for `m1 != 0`, on a facet whose normal `e_3` lies in a plane containing the axis and
  mirroring the facet. Derivation: the integrand is linear in `x` and the 2 x 2 rule of a planar quad integrates `N_i det x`
  exactly, so the integral is `A e_3 . (r_perp(x_c) x a)` with `x_c` the area centroid. If the facet is mirror-symmetric about a
  plane `P` through the axis, then `x_c` is in `P`, so is `r_perp(x_c)` (the plane contains `a` and `O`), and `e_3` is in `P` (the
  normal of a planar facet symmetric about `P` and not in `P`); `r_perp(x_c) x a` is perpendicular to `P`: the product is zero.
  The frustum quads of D2 are isosceles trapezoids symmetric about their mid-meridian plane: `r_2^T f = 0` per element, to
  rounding, with `m1 != 0`.

  **A finding (disclosed before the run).** The force through the thickness of the exact body load is `int rho b(x + z e_3) dz
  = m0 b(x) + m1 r_perp(e_3)` (`b` linear in position); the library keeps `m0 b(x)` and drops `m1 r_perp(e_3)`, whose work on
  `r_2` is `m1 int r_perp(e_3) . (a x (x - O)) dA = m1 int e_3 . (a x r_perp(x)) dA`, exactly minus the moment term's. So the
  exact load has `r_2^T f = 0` on any facet, the library's only on the symmetric ones: on a layup with `m1 != 0` and a facet
  whose normal has a tangential part (a blade at zero stagger has `e_3` tangential, `|r_2^T f| = m1 A rho_c`) the centrifugal load
  of ADR 0502 D1 has a net torque about the axis, and a free sector of such facets has no reaction-free gauge. This item does not
  change the load (it is an existing result of every layup with `m1 != 0`, not opt-in; changing it would change outputs); it
  records the property by the diagnostic check OL and proposes a follow-up. The acceptance's layup sector is the frustum (D2),
  where D1 holds with `m1 != 0`.

The gauge of `free_sector_unit_static` (axial and tangential translation of the interior node farthest from the axis) is the
same for all kinds and needs only components 0, 1, 2 at the node: Tet10 and Hex20 nodes have them, Shell4 nodes have six. With
`C^T N` invertible (`rho_g > 0`) and `f` orthogonal to `N`, `lambda = 0` exactly (ADR 0606 D2). **No library change is
planned.**

### D2. The models (fixed now)

Frame of ADR 0611: `a = (1, 2, 2) / 3`, `O = (0.3, -0.2, 0.5)`, `e_a = (2, -1, 0) / sqrt 5`, `e_b = a x e_a`, the map
`(r, phi, z) -> O + r (cos phi e_a + sin phi e_b) + z a`.

- **T10**: `tet_box([0.25, 0, -0.1], [0.5, alpha, 0.3], [2, 2, 1])`, `alpha = 2 pi / 12`, `upgrade_to_tet10`, then every node
  (corners and mid-edge nodes) mapped by the map (curved Tet10; the surface sets, the element set `all` and the node sets kept
  by building a new `SolidMesh` of the mapped nodes and the same connectivity and sets). Steel `E = 2e11`, `nu = 0.3`, `rho =
  7850`. `Constraint::cyclic("ymin", "ymax", a, O, 12)`. Free. 24 Tet10 elements (4 cells of 6).
- **O20**: the Hex20 sector S20 of ADR 0611 (`hex_box([0.25, 0, -0.1], [0.5, alpha, 0.3], [3, 2, 1])`,
  `upgrade_to_hex20_mapped` with the map, `N = 12`) with the orthotropic material of ADR 0465 (`E = (1.4e11, 1.0e10, 1.0e10)`,
  `nu = (0.3, 0.3, 0.45)`, `G = (5e9, 5e9, 3.5e9)`, `rho = 1600`, expansion 0), axes `MaterialAxes::cylindrical(O, a)`
  (radial, tangential, axial). Free.
- **L2**: the frustum sector SH4 of ADR 0611 (`N = 6`, `NC = 2`, `NA = 3`, radius `1 - 0.4 s`, axial `0.8 s`) with the block
  property `BlockProperty::layup` of two plies of the ply material of ADR 0496 (`E1 = 10e9`, `E2 = 4e9`, `nu12 = 0.35`, `G12 =
  1.6e9`, `E3 = 4e9`, `nu13 = 0.35`, `nu23 = 0.45`, `G13 = 1.6e9`, `G23 = 1.4e9`, fixed identity axes), each `0.025` thick, the
  bottom one at 0 degrees with `rho = 1400`, the top one at 90 degrees with `rho = 2800` (the two-density layup of ADR 0502
  arithmetic 1, scaled to the frustum's `t = 0.05`): unsymmetric (`B != 0`) and `m1 = 0.025 x 0.0125 x (2800 - 1400) = 0.4375`
  kg/m `!= 0`. Block material steel (the plies carry their own). `CyclicOptions::new().shells(true)`. Free.

Interior nodes: the nodes in neither cyclic node set (`ymin`, `ymax`; `low`, `high`). Largest radius `r_max`: 0.75 (T10, O20),
1.0 (L2).

**The constrained-copy route** (ADR 0611 D2, unchanged): a copy with `Constraint::linear` `a . u_A = 0` at `A` = the interior node
of the smallest index and `t_B . u_B = 0` at `B` = the interior node of the largest index, plus `Load::centrifugal(a, O, 1)`,
`solve_static` default options, `Prestress::from_static`.

**The stiffness of the residual.** For T10 and L2 the public `assemble::stiffness` (it takes a Shell4 layup through
`shell::model_element_stiffness`). It does not take the orthotropic material (`check_elements_supported_with_structures`
refuses a per-point material; the per-point assembly `point_stiffness` is crate-private). For O20 the residual's `K` is
`RotatingSector::new(O20, RotatingOptions::new(3).stress_stiffening(false)).part().operator(0.0)`: with stress stiffening off no
static solve is made (the free sector is accepted), and `operator(0)` is `K` plus `0 x (K_sigma,1 - M_perp)` term by term, the
stiffness of `point_stiffness` on all equations (no supports on the transformation path). Taken on the maintainer's standing
instruction of 2026-11-11 (take the recommendation) rather than making `point_stiffness` public, which would be a library
change.

### D3. What is refused (registered, not invented)

- **A Shell3 layup** is refused by `check_shell_model` (`NotYetSupported`, variant `"Layup"`, ADR 0496 D7) on every path that
  checks shells, `RotatingSector::new` among them. RL3 records the refusal with and without the opt-in on the Shell3 frustum of
  ADR 0611 with the L2 layup. No code is written for it.
- **A layup with `m1 != 0` on facets without the mirror symmetry of D1**: not refused by the library, but the load is not
  orthogonal to `r_2` (D1 finding); not run as a sector. OL measures the torque on one facet against its closed form.

## Arithmetic (disclosed, by hand)

1. Scales: T10 as S20 of ADR 0611 (hoop strain `<= 2.2e-8`). O20: `rho r^2 / E_2 = 1600 x 0.5625 / 1e10 = 9e-8`. L2: the mean
   density `4200 / 2 = 2100`, `2100 x 1 / 4e9 ~ 5e-7`.
2. FS, the two gauges: ADR 0611 measured `1.0e-13` (S20), `1.3e-15` (SH4). T10 conditioning as S20 (curved quadratic, similar
   size), `~ 1e-13`. O20: the stiffness ratio `E_1 / G_23 = 40` raises the condition number by `~ 40` over S20: `~ 4e-12`. L2: the
   bending-to-membrane ratio of SH4 with `E1 / E2 = 2.5` and the coupling `B`: `~ 1e-14`. The limit `1e-10` (the acceptance)
   leaves at least a factor of 25.
3. RX: `eps |K| |u| x (row length) / max |f|`: S20 measured `1.6e-14`; O20 with `|K| ~ 1.4e11 x 0.17` and `|u| ~ 6e-8` gives the
   same order as S20 times `~ 3`; T10 rows `<= 3 x 27` entries as Hex20 or fewer. `<= 1e-12` expected, limit `1e-10`.
4. OR: zero by D1 (L2 by the mirror symmetry, the moment term of size `m1 / m0 ~ 4e-3 m` of the force term); rounding `~ sqrt(n)
   eps`. Limit `1e-13` as ADR 0611.
5. WS: as ADR 0611 arithmetic 4 and 5 (`Omega = omega_1 / 2` of index 2 of the copy-route sector built by
   `with_unit_prestress`, both runs give modes): limit `1e-8`. ADR 0611 measured `4.3e-14`.
6. OL (one planar Shell4 L2-layup facet in the plane of `a` and `e_a` through `O`: corners `O + r e_a + z a` with `(r, z)` =
   `(0.5, 0)`, `(0.7, 0)`, `(0.7, 0.2)`, `(0.5, 0.25)`, a trapezoid lying in a plane through the axis, the case the mirror argument of D1 excludes (the facet is in `P`, not mirrored by it); normal
   `e_3 = +- e_b`, tangential at every point of the plane): `P = m1 A e_3 . (r_perp(x_c) x a)`; `r_perp(x_c) = rho_c e_a`,
   `e_a x a = -e_b`, so `|P| = m1 A rho_c` exactly. The library's `r_2^T f` against `P`: rounding of a 24-term sum of entries of
   size `m0 A rho ~ 105 x 0.045 x 0.6`, relative to `m1 A rho_c` (a factor `m0 / m1 ~ 240` smaller): `~ 240 x 1e-15`; limit
   `1e-10 x m1 A rho_c`. The area `A = 0.2 x (0.2 + 0.25) / 2 = 0.045`, `x_c` computed in the test from the corners.

## Checks (limits fixed now)

Options: `base = RotatingOptions::new(3).coriolis(true)`; the opt-in `base.free_sector_prestress(true)`; harmonic `k = 2` for the
whirl checks (T10 and O20 have `N = 12`, L2 `N = 6`).

| Id | Clause | Check | Limit | Estimate behind it |
|---|---|---|---|---|
| FS | a1 | T10, L2, O20: `RotatingSector::new(free, opt-in).part().unit_prestress()` against the copy route: `max abs(S_new - S_copy)` over every element, point and component over `max abs(S_copy)` | `<= 1e-10` each | the acceptance; arithmetic 2 |
| RX | a2 | T10, L2, O20: `free_sector_unit_static(free, opt-in)`; `K u - f` (`K` of D2, `f` = `nodal_load_vector` of the centrifugal copy) at every equation of every interior node, `max abs` over `max abs(f)` | `<= 1e-10` each | the acceptance; arithmetic 3 |
| RXc | a2 | the same residual of the copy route's solution | `<= 1e-10` each | as RX |
| OR | D1 | T10, L2, O20: `abs(r_i^T f) / (norm(r_i) norm(f))`, `r_1`, `r_2` of D1 (L2 with `theta = a` in `r_2`) | `<= 1e-13` | arithmetic 4 |
| GN | D1 | T10, L2, O20: the gauge node interior; `abs(rho_g / r_max - 1)` | interior; `<= 1e-12` | ADR 0606 D3 |
| WS | supplementary | T10, L2, O20: `whirl_modes(2, Omega)` of the opt-in sector against `with_unit_prestress(copy route, base)`: `max abs(omega_new / omega_copy - 1)`; the counts and directions | `<= 1e-8`; equal | arithmetic 5 |
| NC0 | a3 | T10, L2, O20 free with `base` (no opt-in) | refused `FemError::SingularStiffness` | free sectors are singular without the opt-in |
| RL3 | D3 | the Shell3 frustum of ADR 0611 with the L2 layup, free, `RotatingSector::new` with and without the opt-in | both refused `NotYetSupported`, variant `"Layup"` | ADR 0496 D7 |
| OL | D1 finding (diagnostic) | the OL facet of arithmetic 6, one element, the L2 layup, `nodal_load_vector` of `Load::centrifugal(a, O, 1)`: `abs(r_2^T f - P) / (m1 A rho_c)` and `abs(abs(r_2^T f) / (m1 A rho_c) - 1)` | `<= 1e-10`; `<= 1e-12` | arithmetic 6 |
| DF | a3 | `RotatingOptions::new(3).free_sector_prestress` | `false` | ADR 0606 D3 |
| B2 | a3 | existing tests unedited: `sa_free_sector_prestress_f680`, `sa_free_sector_prestress_solids_shells_f683`, `cargo test -p rcfd-fem --release --lib rotating` | pass | no library change |
| B3 | a3 | `git diff d9dd237f -- crates/*/src` | empty; if an amendment changes a source file, `baseline_hashes` is run | D1 |

The checks FS, RX, RXc, OR, GN, WS are one test (`fs_..._against_the_constrained_copy`), NC0, RL3, DF another, OL a third.

## Negative controls

NC0 (without the opt-in the free sectors are refused, so FS tests the opt-in branch), RL3 (a refusal that exists), OL (the D1
orthogonality is a property of the frustum, not of every layup facet: a non-symmetric facet with `m1 != 0` gives a torque of the
predicted size), and the mutations.

## Mutations (on a scratch copy, own `CARGO_TARGET_DIR`)

| Id | Mutation (site, `rotating.rs`, `free_sector_unit_static` and `RotatingOptions::new`) | Expected to be caught by |
|---|---|---|
| M1 | the axial gauge constraint `a` not added | FS (the opt-in solve refused `SingularStiffness`: the test panics) |
| M2 | the tangential gauge constraint `t` not added | as M1 |
| M5 | a third gauge constraint on the radial component of `g` | FS, RX, WS |
| M6 | the default `free_sector_prestress: true` | DF, NC0 |
| M7 | the gauge node chosen nearest the axis (`rho < best` in place of `rho > best`) | GN (FS and RX are expected to stay met: the stress does not depend on the gauge) |

Each applied by a script to a `git archive` copy of the test commit, each copy with its own `CARGO_TARGET_DIR`, the new test file
run once per mutation with `--no-fail-fast`.

## Decision rule

**done**: every check FS to B3 met in the registered run, every mutation caught by at least one of its expected checks.
**partial**: any registered check missed (recorded as a `*_known_miss`), a check met only after a post-run amendment, a check not
run, or a mutation not caught. **todo**: FS and RX not run.

## Limits (what is not covered)

- Tet4, wedges, Shell4 ANDES membranes, the tabulated material, hyperelastic structures and mixed meshes are not run.
- A Shell3 layup is refused (RL3); no code for it.
- Layups with `m1 != 0` on facets without the mirror symmetry of D1 (twisted or staggered blades): the library's centrifugal
  load has a torque about the axis (D1 finding, OL), so a free sector of them has no reaction-free gauge; not run as a sector,
  and the load is not changed here.
- An orthotropic sector with axes other than cylindrical about the cyclic axis stays refused (ADR 0462).
- Sectors touching the axis, supported or constrained sectors with the opt-in (refused, ADR 0606), as ADR 0611.
- The scope without stress stiffening (ADR 0611 NS1 to NS3) is not re-run on these families.
