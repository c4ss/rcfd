# ADR 0445: F-525 registration: nodal body-force fields and thermal loads on Shell4

Status: **REGISTERED 2026-11-14, written before any code of F-525 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-525 was written, compiled or run in rcfd: the files listed under "What was read" were
read and the arithmetic below was done by hand from closed forms (no script, no finite-element code). Results go in the addendum
at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0435](0435-f522-registration-shells-on-the-transformation-and-body-loads.md) (F-522: gravity and the centrifugal
load on Shell4 through `shell::body_load_vector` and the shell rule of `loads::add_body_load_with`; its D2 keeps
`Load::BodyForceNodal` and `Load::Temperature` on a shell refused through the solid-element routines naming
`shell::SOLID_ROUTINES`, its L2 and remaining question 3 option (a) name this item), [ADR 0432](0432-f392-registration-flat-facet-shell.md)
(F-392: Shell4, the facet, the membrane rows, the transformation `T` with rigid offsets, `shell::stresses` at the element
centre), [ADR 0281](0281-thermal-strain-load.md) (the thermal load `int B^T D eps_th dV` of a nodal temperature field on solids,
the recovered stress `D (eps - eps_th)`), [ADR 0440](0440-f408-registration-advanced-tier-scaffold.md) (F-408: the material
variants, the `Layup` property and the section hook; its bit records are not touched). Roadmap item F-525, deps F-522. Size S.
Branch `f525-shell-thermal-nodal-loads` from 5325c69d, worktree `w49i-work/f525`, scratch `w49i-work/f525-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical (a model without Shell4
elements gives the bytes it gave); no edit of `baseline_hashes.rs`, of the hash tables or of any golden; existing tests and
ignored records not edited, except the `#[ignore]` line of a retired refusal test; no new crate, no unsafe code; clean-room
(first principles: the consistent load of an interpolated field and the membrane thermal strain of plane stress).

## What F-525 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. Load::BodyForceNodal on a shell model gives the consistent
> loads t int N_i b dA of the interpolated field within 1e-12; a uniform temperature change on a free shell plate gives the
> membrane strain alpha dT and zero stress within 1e-12; a model without shells gives the bits it gave before.

Clauses: (a1) every limit not stated is registered here; (a2) `Load::BodyForceNodal` on a shell model gives the consistent loads
`t int N_i b dA` of the interpolated field `b = sum_j N_j b_j` within 1e-12; (a3) a uniform temperature change on a free shell
plate gives the membrane strain `alpha dT` within 1e-12; (a4) and zero stress within 1e-12; (a5) a model without shells gives the
bits it gave before. From the brief: (b1) how a temperature field on shell nodes maps to membrane thermal strain is registered
(uniform through the thickness; a gradient through the thickness is out of scope, said so); (b2) the supports of the free plate
remove only the rigid motions; (b3) the retired refusal row gets an ignored record citing this ADR and a running `_after_landed`
copy; (b4) `baseline_hashes` is run because `loads.rs`, `thermal.rs` and `shell.rs` are shared solver files used by rcfd-api;
(b5) `sa_scaffold_bits` passes unedited.

## What was read (5325c69d, read only)

- ADR 0435 (D2, D4, the checks, L2, addendum, remaining question 3), ADR 0432 (the facet, arithmetic 7 the transformation, D7 the
  placeholder element), ADR 0281 (the thermal strain `alpha dT (1, 1, 1, 0, 0, 0)`, the load at the stiffness rule's points, the
  recovery `D (eps - eps_th)`), ADR 0440 D5 and D6 (the `Layup` property is refused by every solve until `laminate` lands; the
  section hook `section_of`).
- Code: `crates/rcfd-fem/src/loads.rs` (whole: `nodal_load_vector`, `add_body_load_with` with the beam and shell rules,
  `add_shell_body_load`; the `BodyForceNodal` arm calls `add_body_load` without a shell rule, so a Shell4 element reaches the
  placeholder element and is refused), `thermal.rs` (whole: `add_thermal_load` calls `element_thermal_load` for every element,
  which refuses a Shell4 element through `element_for(Shell4)`), `shell.rs` (whole: `facet`, `point`, `membrane_rows`,
  `plane_matrix`, `homogeneous_section`, `transformation`, `body_load_vector`, `thickness_or_error`, `stresses`), `model.rs`
  (`Load::BodyForceNodal` and `Load::Temperature` are validated at `add_load` for length and finiteness; `Material::expansion`
  gives NaN for a variant without one value), `recover.rs` (`IntegrationPointField::compute` goes through `element_for`, so it
  refuses a shell model; unchanged here).
- Tests: `si_shell.rs` (`s1_n2_n3_refusals_after_landed`, row R3 "nodal body force on a shell" refused naming
  `shell::SOLID_ROUTINES`, the row this item retires; helpers `grid`, `shell_mesh`, `rotation`, `plane`), `si_beam_loads.rs` (row
  "N2 temperature on a beam", unchanged), `si_scaffold.rs` (the placeholder element of Shell4 refuses its solid routines:
  unchanged, the placeholder stays), `sb_thermal_bits.rs` (the digest pattern used by B1 here), `sa_scaffold_bits.rs`,
  `si_scaffold_bits.rs`. No other running test asserts a refusal this item lifts (grep of `SOLID_ROUTINES`, `body_force_nodal` and
  `Load::temperature` over `crates/*/tests`).

## Arithmetic before this registration (disclosed)

1. **The consistent load of a nodal field.** With `b(xi, eta) = sum_j N_j b_j` on a facet, `F_i = t int N_i b dA = t sum_j M_ij
   b_j`, `M_ij = int N_i N_j det dxi deta`. On a flat facet `det = J0 + J1 xi + J2 eta` (bilinear map of plane coordinates), so the
   integrand is of degree at most 3 in each variable and the 2 x 2 Gauss rule of `body_load_vector` is exact. Closed form (test
   side, independent of `shell.rs`): with `N_i = (1 + a_i xi)(1 + c_i eta) / 4`, `M_ij = (1/16) [J0 P0(a) P0(c) + J1 P1(a) P0(c) +
   J2 P0(a) P1(c)]`, `P0(a) = 2 + 2 a_i a_j / 3`, `P1(a) = 2 (a_i + a_j) / 3`; `J0, J1, J2` from the Jacobian determinant of the
   plane map evaluated at `(0, 0)`, `(1, 0)`, `(0, 1)` (exact for a linear function up to rounding). Rounding: a sum of 16 terms per
   node and element, `~1e-15` relative to the largest force.
2. **The membrane thermal strain (b1).** A Shell4 node carries one temperature of `Load::Temperature`; it is read as the
   temperature of the mid-surface, uniform through the thickness. The thermal strain in the facet's local axes is `eps_th = alpha
   dT (1, 1, 0)` (engineering shear, no shear part); plane stress leaves the normal strain free, so it has no stiffness and no
   load; a uniform temperature through the thickness has no curvature part, so the bending and transverse shear loads are zero.
   `dT(xi, eta) = sum_i N_i (T_i - T_ref)` (the facet's bilinear functions). The membrane resultant is `N_th = A eps_th`, `A =
   E t / (1 - nu^2) [[1, nu, 0], [nu, 1, 0], [0, 0, (1 - nu) / 2]]`, so `N_th = E t alpha dT / (1 - nu) (1, 1, 0)`. The local
   load `f_l = sum_gp det B_m^T A eps_th` at the 2 x 2 points of the stiffness (the same `membrane_rows`), carried to the nodes by
   `T^T` (forces and, on a warped facet, offset moments), as the pressure and body loads are. The recovered membrane force at the
   centre is `N = A (eps_c - eps_th,c)` with `dT_c = sum_i N_i(0, 0) dT_i`; moments and shear unchanged.
3. **The free plate is exact.** Under a uniform `dT` the free expansion `u = alpha dT (x - x_0)`, zero rotations, is reproduced by
   the element: a bilinear isoparametric quadrilateral interpolates a linear field exactly, its membrane strain is `alpha dT (1, 1,
   0)` at every point, so `K_m u = int B_m^T A B_m u = int B_m^T A eps_th = f_l`; the bending and shear parts see zero rotations and
   zero `w`; the drilling term `theta_z - (v_x - u_y) / 2 = 0`. Hence `K u = f` and the solution is the free expansion up to
   rounding, the strain is `alpha dT (1, 1, 0)` and the stress `A (eps - eps_th) = 0`.
4. **Supports (b2).** One corner node (node 0) has its six components prescribed zero: exactly the six rigid motions of the plate
   (a rigid motion is fixed by the translation and rotation of one point), so the support is statically determinate and the free
   expansion about node 0 (zero rotation) meets it: no reaction, the plate is free. No other support.
5. **Rounding of the free plate.** Plate P: in the global `xy` plane, 4 x 3 distorted elements on `[0, 2] x [0, 1]`, `t = 0.01`.
   Its facets have `e_3 = (0, 0, +-1)` exactly, so the global stiffness has exact zeros between the in-plane and the out-of-plane
   unknowns and the load has exact zeros on `u_z, theta_x, theta_y`: those are 0 exactly; the in-plane problem has a condition
   `~1e3` to `1e4` (drilling against membrane, `DRILLING_FACTOR = 1e-3`), rounding `~1e-13` relative at most. Plate Q: the same
   in-plane mesh rotated about `(1, 2, 3)` by 0.7 rad and moved by `(0.3, -0.2, 0.5)`, `t = 0.05`. Rounding couples the
   membrane load into the bending unknowns at `eps |f|`; the plate held at one corner is soft in bending: `k_mem / k_bend ~ 12 (L /
   t)^2 c^-1` with `L = 2`, `c ~ 0.1`: `2e5`, so the out-of-plane displacement and the rotations carry `~ 2e-16 * 2e5 = 4e-11` of
   `alpha dT L` (hence the 1e-8 limit of TH3b). This bending rounding does not enter the membrane strain (the local membrane and
   bending fields are decoupled; in the projection onto the plate axes it enters at `eps` times itself). In the surface stress the
   bending part is `6 M / t^2` with `M ~ D kappa`, `kappa ~ dw / h^2`: relative to `E alpha dT / (1 - nu)` it is `~ 6 eps h / (t c
   (1 - nu)) = 6 * 2.2e-16 * 0.5 / (0.05 * 0.1 * 0.7) = 2e-13`, a factor 5 under the 1e-12 limit (the margin is stated; a miss
   would be diagnosed against this estimate, not tuned).
6. **A linear temperature on a rectangle (TH4).** A single element `a x b = 0.6 x 0.4` in the `xy` plane, `dT = c0 + c1 X + c2 Y`
   at the nodes. `f_x,i = k int N_i,X dT dA`, `f_y,i = k int N_i,Y dT dA`, `k = E t alpha / (1 - nu)`; with `X = (a/2) xi`, `Y =
   (b/2) eta`, `N_i,X = (2/a)(a_i/4)(1 + c_i eta)`; the integrand is of degree 2 per variable: 2 x 2 Gauss is exact. Closed form in
   the test by the 1D integrals `int (1 + c eta) d eta = 2`, `int eta (1 + c eta) = 2c/3`, `int xi d xi = 0`, `int xi^2 = 2/3`.
7. **Self-equilibrium of the thermal load (TH5).** For any field, `sum_i N_i,X = 0`, `sum_i X_i N_i,Y = 0`, `sum_i Y_i N_i,X = 0`
   (isoparametric), and the thermal resultant is isotropic, so the local load has zero resultant and zero moment in the plane of
   the facet; `T^T` moves each local force from the projected point `p_i` to the node `x_i` with the moment `(p_i - x_i) x F_i`, so
   `sum (x_i x F_i + m_i) = sum p_i x F_i = 0` on a warped facet too. Rounding `~1e-15`.
8. **Uniform field against gravity (BF2).** A uniform nodal field `b` gives at a point `sum_j N_j b = b sum_j N_j`, against `rho g`
   with `rho = 1`, `g = b`: equal up to the rounding of `sum_j N_j = 1` (`~2 eps`), relative `~5e-16`.
9. **The bits of a model without shells (a5).** The `BodyForceNodal` arm gets a shell rule and the thermal load a shell branch,
   both entered only for a Shell4 element; the solid expressions, their order and the element order are unchanged. In a mixed model
   whose shell and solid nodes are disjoint (the only kind accepted, ADR 0432 D4), an entry at a solid node gets only solid
   contributions in the same order, so it is the bits of the solid-only model (B3).

## Design

### D1. Nodal body-force fields on Shell4 (`shell.rs`, `loads.rs`)

New `shell::body_load_vector_at(coords, thickness, b)` with `b: Fn(&[f64; 4], Vec3) -> Result<Vec3>`, the shape-function values
at the point and the point: the code of `body_load_vector` with `b(&q.n, x)` in place of `b(x)`; `body_load_vector` becomes
`body_load_vector_at(coords, thickness, |_, x| b(x))` (the same operations in the same order: the gravity and centrifugal bits of
shells are unchanged). The shell rule of `loads::add_body_load_with` takes the values as well:
`Fn(usize, &[f64; 4], Vec3) -> Result<Vec3>`; gravity and centrifugal ignore them. The `BodyForceNodal` arm passes a shell rule
`b = sum_j N_j values[node_j]` over the element's nodes in element order, and no beam rule (a beam keeps refusing through the
solid routines naming `beam::SOLID_ROUTINES`). The solid closure is unchanged.

### D2. Thermal loads on Shell4 (`shell.rs`, `thermal.rs`)

New `shell::thermal_load_vector(coords, thickness, material, delta_t_nodal: &[f64]) -> Result<[f64; 24]>` (arithmetic 2):
refuses other than four temperatures (`FieldLength { key: "temperature" }`) and a non-finite expansion of the material
(`InvalidValue { key: "expansion" }`); `A` is `homogeneous_section(material, t).membrane`; the local load at the 2 x 2 points;
`T^T` to the nodes. `thermal::add_thermal_load` gets a branch for Shell4 elements: the thickness by `thickness_or_error` (a block
without a Shell property, a `Layup` among them, refused naming the element, as gravity is), the 24 entries added as forces and
moments; every other element as before. `shell::stresses` reads `thermal::temperature_field(model)`; with a field, the membrane
strain at the centre becomes `eps - alpha dT_c (1, 1, 0)` before `A` (and the coupling) is applied; without one, the former code.
The placeholder element of Shell4 stays (the solid-element routines, `IntegrationPointField` of a shell model, still refuse).
Module docs of `loads.rs`, `thermal.rs`, `shell.rs` name this ADR.

### D3. Bits

A model without Shell4 elements: the shell rule and the shell branch of D1 and D2 are entered only for a Shell4 element; the
`stresses` change only for a shell element of a model with a temperature field (no such model could be solved before); so every
model without shells gives the bits it gave (B1 to B3). A shell model under gravity or the centrifugal load: D1 leaves the
operations of `body_load_vector` as they were (covered by `si_shell_transformation.rs`, `si_shell_rotating_disc.rs`, unedited).

## Checks (limits fixed now)

| id | clause | check | limit | estimate behind it |
| --- | --- | --- | --- | --- |
| BF1 | a2 | flat distorted 3 x 2 mesh on `[0, 1.5] x [0, 1]` (interior nodes moved in plane by `(0.07, -0.05)`, `(-0.04, 0.06)`), rotated about `(1, 2, 3)` by 0.7 rad and moved, `t = 0.02`, `BodyForceNodal` with `b_j = (100 + 30 X_j - 20 Y_j, -50 + 10 X_j Y_j, 200 - 40 Y_j)` (plane coordinates of node j): every node's force from `nodal_load_vector` against `t sum_e sum_j M^e_ij b_j` (arithmetic 1), relative to the largest force; each nodal moment relative to the largest force times 1 m | `1e-12` each | arithmetic 1: `~1e-15` |
| BF2 | (stronger, a2) | warped mesh (the BF1 mesh unrotated, each node lifted by `0.002 sin(1.7 k + 0.3)`), uniform field `b = (3, -2, -9.81)` against `Load::Gravity { b }` with density 1: every entry, relative to the largest entry | `1e-13` | arithmetic 8: `~5e-16` |
| TH1 | a3 | plates P and Q (arithmetic 5), `alpha = 1.2e-5`, `T = 350` at every node, `T_ref = 300`, node 0 clamped (6 components): the membrane strain at each element centre from the solved displacements projected onto the plate axes and the bilinear derivatives (test side), against `alpha dT (1, 1, 0)`: `max |eps - eps_th| / (alpha dT)` | `1e-12` per plate | arithmetic 3, 5: `~1e-13` |
| TH2 | a4 | the same solves, `shell::stresses`: `max |N_k| / (E t alpha dT / (1 - nu))` and `max |sigma_top,k|, |sigma_bottom,k| / (E alpha dT / (1 - nu))` | `1e-12` each, per plate | arithmetic 3, 5: P `~1e-13`, Q `2e-13` |
| TH3a | (stronger, a3) | in-plane displacements projected on the plate axes against `alpha dT (X_i - X_0, Y_i - Y_0)`, relative to `alpha dT max |X - X_0|`; plate P only: `max |u_z| / (alpha dT L)`, `max |theta_x|, |theta_y|, |theta_z| / (alpha dT)` | `1e-12` | arithmetic 5: P exact zeros out of plane, `~1e-13` in plane |
| TH3b | (stronger, b2) | plate Q: the out-of-plane component and the rotations as in TH3a | `1e-8` | arithmetic 5: `4e-11` |
| TH4 | (stronger, b1) | one rectangle `0.6 x 0.4`, `t = 0.01`, `dT = 20 + 30 X - 15 Y` at the nodes: the forces of `nodal_load_vector` against arithmetic 6, relative to the largest; moments and out-of-plane forces relative to the largest force (times 1 m for moments) | `1e-12` | arithmetic 6: `~1e-15` |
| TH5 | (stronger, b1) | the BF2 warped mesh, `t = 0.02`, `T_k = 300 + 40 x_k - 25 y_k + 500 z_k` against `T_ref = 290`: `|sum F_i| / max |F_i|`; `|sum (x_i x F_i + m_i)| / (max |x_i| max |F_i|)` | `1e-12` each | arithmetic 7: `~1e-15` |
| B1 | a5 | digests (FNV-1a over the bytes of every entry) of `nodal_load_vector` of four solid models (Hex8 box `[2, 1, 1]` and Tet10 box `[2, 1, 1]`, each with a `BodyForceNodal` field `(x + 2 y, -z, 3 - x)` per node plus a temperature `300 + 10 x + 5 y z` against 290 (`alpha = 1.2e-5`), and each with gravity plus centrifugal) equal the digests printed by the same file run on a copy of 5325c69d before any library code of F-525 (`f525-scratch/b1-base.log`) | exact | arithmetic 9 |
| B2 | a5, b4, b5 | `baseline_hashes` (rcfd-api, release), `sa_scaffold_bits`, `si_scaffold_bits`, `sb_thermal_bits`, and the rcfd-fem test files covering the changed lines (`sb_loads`, `sb_thermal`, `sb_recover`, `si_shell`, `si_shell_transformation`, `si_shell_rotating_disc`, `si_beam_loads`, `si_scaffold`, `sa_scaffold`) pass unedited (apart from the `#[ignore]` line of R0) | exact | D3 |
| B3 | (stronger, a5) | mixed model: the Hex8 box of B1 and, on separate nodes, the BF1 plate (unrotated), each its material; `BodyForceNodal` and `Temperature` on all nodes: the entries of every solid node are the bits of the Hex8-only model with the same fields, and the entries of every shell node the bits of the shell-only model | exact | arithmetic 9 |
| R0 | b3 | `si_shell.rs` `s1_n2_n3_refusals_after_landed` gets only the line `#[ignore = "record of ADR 0435 before ADR 0445 (F-525, nodal body-force fields and thermal loads on Shell4) retired the R3 nodal body force row; superseded by the s1_n2_n3_refusals_f525_after_landed copy below (maintainer standing decision, 2026-11-13)"]`; the copy keeps every other row and replaces R3 by R4 | exact | the pattern of ADR 0435 |
| R4 | (retired-row copy) | the base cantilever of `si_shell.rs` with `BodyForceNodal (0, 0, -1e4)` at its 10 nodes solves; `|sum R_z + t A b_z + F_z| / (t A |b_z|)` | `1e-8` | the R2 limit of ADR 0435 for the same solve |

### Negative controls

| id | model | expected | limit |
| --- | --- | --- | --- |
| N1 | a Beam2 model (two elements) under `Load::Temperature` | `Element { source: NotYetSupported { package: beam::SOLID_ROUTINES } }` (unchanged) | exact |
| N2 | a Shell4 block with a `Layup` property (one ply of the BF1 material, 0.01 m) under `Load::Temperature`: `nodal_load_vector` | `Element { source: InvalidValue { key: "property" } }` | exact |
| N3 | `IntegrationPointField::compute` of plate P with its temperature | `Element { source: NotYetSupported { package: shell::SOLID_ROUTINES } }` (unchanged) | exact |
| N4 | plate P with `T = T_ref` at every node | the load vector is exactly zero and so is the solved displacement | exact |
| N5 | (sensitivity of TH2) the membrane force of plate P computed in the test as `A eps` from the TH1 strain, without the thermal strain | `|N_xx| / (E t alpha dT / (1 - nu))` above 0.5 (it is 1) | `> 0.5` |

### Planned mutations (scratch copy `f525-scratch/tree` from `git archive` of the final commit; not committed)

| id | mutation | caught by (expected) |
| --- | --- | --- |
| MU1 | the `BodyForceNodal` arm passes no shell rule | BF1, BF2 (refused) |
| MU2 | the shell rule of `BodyForceNodal` interpolates with the element's nodes reversed | BF1 |
| MU3 | the thickness left out of `thermal_load_vector` | TH1, TH3a, TH4 |
| MU4 | the thermal strain `(1, 1, 0)` replaced by `(1, 0, 0)` | TH1, TH2, TH4 |
| MU5 | `stresses` does not subtract the thermal strain | TH2 |
| MU6 | `T^T` left out of `thermal_load_vector` (local components added as global) | TH1, TH2 on plate Q, TH5 |
| MU7 | the temperature at every Gauss point taken as that of the element's first node | TH4 |
| MU8 | the sign of the thermal strain in `stresses` flipped | TH2 |
| MU9 | the solid `BodyForceNodal` interpolation summed over the nodes in reverse order | B1 |
| MU10 | the offsets left out of the transformation used by `thermal_load_vector` (offset moments dropped) | TH5 |

## Decision rule

- **done**: every check of the table and every negative control met, every planned mutation caught, B2 passing.
- **partial**: a registered check missed (recorded as `*_known_miss`), a mutation not caught, or a check met only after an
  amendment made after its run.
- **todo**: the code does not compile or the shell paths of D1 and D2 are not reached.

## Limits (not covered)

- L1. A temperature gradient through the thickness (a curvature thermal load `int B_b^T D_b kappa_th`): `Load::Temperature` holds
  one value per node, so a gradient needs a second nodal field (a new load variant and its validation, a schema question); not
  trivial, out of scope, a follow-up.
- L2. Thermal loads on a `Layup` shell (ply expansion, the thermal coupling `B`) stay refused (the laminate package is pending,
  ADR 0440 D5); orthotropic shell materials are not taken (a NaN expansion is refused).
- L3. Thermal stress of shells is recovered only by `shell::stresses` at the element centre; `IntegrationPointField` and the
  solid recovery keep refusing shells. Beams keep refusing thermal and nodal body-force loads.
- L4. The paths that refuse shells (buckling, cyclic, geometric, harmonic, transient, nonlinear, rotating) are unchanged.
- L5. No rcfd-api, schema or case-file key is added: the loads are reachable through the library only.

## Open points taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. The temperature of a shell node is the mid-surface temperature, uniform through the thickness (b1, L1), rather than refusing
   shells under a temperature load until a gradient field exists.
2. `shell::stresses` subtracts the thermal strain when the model has a temperature field (D2) rather than a new function: no
   shell model with a temperature could be solved before, so no former result changes.
3. A `Layup` block under a temperature is refused by the thickness check, as gravity is (N2), rather than with a laminate
   package name.

## Amendment 1 (2026-11-14, written and committed after the registered run, before the run of TH3c)

The registered run (`f525-scratch/run1.log`, test commit `ab36fef0`) missed three rows of TH3a: the in-plane displacement of
plate P `2.1e-10` and of plate Q `1.3e-10` against `alpha dT (X - X_0)`, and the rotations of plate P `3.0e-10` of `alpha dT`
(limit `1e-12` each). TH1 (the strain, `8.5e-15`) and TH2 (the stress, `2.6e-15`) were met in the same solves, so the error
carries no strain: it is a rigid motion. Diagnosis (not a change of limit): the in-plane rigid rotation about node 0 is held only
by `theta_z` of node 0 through the drilling penalty of its one element, `alpha_d G t A_e = 1e-3 * 7.7e10 * 0.01 * 0.17 ~ 1.3e5`
N m, against membrane stiffnesses `E t ~ 2e9` N/m; arithmetic 5 counted the drilling only in the in-plane condition and missed
that this soft rotation is the one the rounding of the membrane solve excites. Out-of-plane rows of TH3a for P are exact zeros as
estimated. The missed rows stay as registered: the test of the run becomes the ignored record
`th1_th3_n3_n5_free_plate_under_a_uniform_temperature_known_miss` (limits intact, measured numbers in its ignore text), and a
running copy `th1_th3_n3_n5_free_plate_after_known_miss` keeps every met row with its limit and leaves out the three missed rows.

Added check (registered now, before its run), to confirm the diagnosis:

| id | clause | check | limit | estimate |
| --- | --- | --- | --- | --- |
| TH3c | (stronger, diagnosis of TH3a) | plates P and Q: the in-plane displacements minus `alpha dT (X - X_0)` minus the in-plane rigid rotation `omega e_Z x (X - X_0)` fitted by least squares (`omega = sum (r_i x d_i)_Z / sum |r_i|^2`), relative to `alpha dT max |X - X_0|`; and `max |theta_i . e_Z - omega| / (alpha dT)` | `1e-12` each | if the diagnosis holds, the rounding of the strain field: `~1e-14` |

The decision is partial by the registered rule whatever TH3c gives (a registered check missed).

## Amendment 2 (2026-11-14, written and committed after the run of TH3c, before the corrected run)

The run of TH3c (`f525-scratch/run2.log`, commit `0d1d3d1c` of the copy test) met its first row (the in-plane displacements less a
fitted rigid rotation: `3.6e-15` on P and Q, limit `1e-12`) and missed its second row (`2.35e-10` on P, `1.40e-10` on Q). The
second row's set-up is defective: its maximum runs over node 0, whose `theta_z` is held at zero by the support, so it measures
`|0 - omega| = |omega|` (the printed `omega / (alpha dT)` is `2.3528e-10` on P and `-1.4010e-10` on Q, equal to the missed
numbers). A diagnostic on a scratch copy (`f525-scratch/tree`, not a registered check) found the residual `|K u - f|` of the
computed solution `9.3e-10` N on P and `1.0e-8` N on Q against `|f| = 8.6e5` and `3.6e6` N (`1.1e-15`, `2.9e-15` relative), the
same order as the residual of the exact free expansion (`8.1e-10`, `4.1e-9` N): the solve is backward stable and the TH3a error is
the forward error of a soft mode. The mode: translations and `theta_z` of every node rotating rigidly by `omega`, except
`theta_z` of node 0, held at zero; its energy lives only in the drilling term of the elements at node 0, `~ alpha_d G t A_e / 16
~ 8e3` N m, so a residual moment `~1e-9` N m gives `omega ~ 1.4e-13` rad, the measured `2.35e-10 alpha dT`.

The test of that run stays as the ignored record `th1_th3_n3_n5_free_plate_after_known_miss`. Corrected row (registered now,
before its run), in the running copy `th1_th3_n3_n5_free_plate_after_amendment2` (every other row of the copy unchanged):

| id | clause | check | limit | estimate |
| --- | --- | --- | --- | --- |
| TH3d | (stronger, diagnosis of TH3a) | plates P and Q: `max |theta_i . e_Z - omega| / (alpha dT)` over the nodes that are not nodes of element 0 (node 0's element, where the mode carries `theta_z` from 0 to `omega`); the values at the other three nodes of element 0 are printed, not checked | `1e-12` | the mode has `theta_z = omega` outside element 0; its spread into the membrane field is held by stiffnesses `1e3` times the drilling one: `~2e-13` |
