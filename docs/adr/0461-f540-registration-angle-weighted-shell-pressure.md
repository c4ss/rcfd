# ADR 0461: F-540 registration: pressure on curved facet meshes of Shell3 (the diagnosis of the one-diagonal cylinder O1 and an opt-in angle-weighted split of each triangle's pressure)

Status: **REGISTERED 2026-11-14, written before any code of F-540 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-540 was written, compiled or run in the repository, except (disclosed below) the bit
record B1 of the default shell-pressure paths (the test file `crates/rcfd-fem/tests/sa_shell_pressure_bits.rs`, using only the
API of main 32fbefad, run once at that commit with no library line changed, `f540-scratch/b1-base.log`) and the diagnosis runs
of a plain numpy prototype in the scratch folder (`w49i-work/f540-scratch/proto.py`, a copy of the F-419 prototype
`f419-scratch/proto.py` that matched rcfd's Shell3 to `2.2e-9` on every check of ADR 0454; `diag.py`, `diag3.py`, `diag4.py`,
logs `diag1.log` to `diag4.log`; dense matrices, no rcfd code). Results go in the addenda at the end; nothing above the addenda
is changed after a run.

Follow-up of F-419 ([ADR 0454](0454-f419-registration-three-node-flat-shell.md): limit L4, observation O1, remaining question 3).
Roadmap item F-540 (deps F-419). Number 0461 given by the lead. Branch `f540-work` from main 32fbefad, worktree
`w49i-work/f540`, scratch `w49i-work/f540-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code, the crate graph
unchanged; no existing test edited (new tests in new files); clean-room.

## What F-540 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. The one-diagonal 32 x 16 cylinder (O1) within 1 % of `p R^2/(E t)` at every node.

Clauses: (a0) every limit registered before the run (this document); (a1) the cylinder quarter of ADR 0454 arithmetic 13 (R =
1, H = 0.5, t = 0.01, nu = 0.3, p = 1e5, E = 2e11, the symmetry conditions of its test, the end z = H free) on 32 x 16 cells,
every cell cut along `(i, j)-(i + 1, j + 1)` (the O1 mesh), `max_nodes |u_r / (p R^2 / (E t)) - 1| <= 1e-2`, `u_r` the
displacement along the radius through the node. The brief adds: (b1) diagnose the cause of the O1 error (facet pressure
lumping, the one-diagonal mesh bias, or membrane locking of the CST on a curved facet mesh) with arithmetic and short runs
written here before the fix is registered; (b2) a discretisation-error estimate registered; (b3) if 1 percent at every node is
not reachable by a consistent load alone, say so and register what is; (b4) any change opt-in, existing Shell3 and Shell4
results bit-identical (digests of the default paths recorded on the base before code); (b5) shared-file edits minimal (F-542
edits `rcfd-fem` concurrently).

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0454 (whole: arithmetic 13, the checks CY1, P4, O1, limit L4, remaining question 3); the head
  of ADR 0460 (the style and the B1 pattern).
- `crates/rcfd-fem/src/shell3.rs` (`facet`, `Facet3`, `pressure_load_vector`, `to_global_vector`), `loads.rs`
  (`nodal_load_vector`, `add_shell_pressure`), `model.rs` (`Load`, `Load::shell_pressure`, the `ShellPressure` arm of
  `add_load`), `beam.rs` (`element_line_load`, the only other `match` on loads that names variants); the `grep` of
  `ShellPressure` over `crates`: only `rcfd-fem` (`loads.rs`, `model.rs`, `shell.rs` documentation).
- Tests: `crates/rcfd-fem/tests/sa_shell3.rs` (the cylinder helper behind CY1 and O1), `sa_shell3_bits.rs` (the digest pattern).
- `f419-scratch/proto.py`, `estimate2.py` (the prototype's cylinder), `estimate6_cy.log` (O1 at NZ = 8, 16, 32).
- **Not read:** any finite-element code, manual or documentation outside this repository, and no paper. Everything used below
  is derived here from first principles. Recalled from memory and not used beyond the idea: angle-weighted vertex normals of
  polygon meshes (G. Thurmer and C. A. Wuthrich, "Computing vertex normals from polygonal meshes", J. Graphics Tools 3 (1998)
  43-46); the diagnosis below shows that a nodal-normal load of that kind is *not* the cure here (D7).

## Diagnosis (arithmetic and runs done before this registration, disclosed)

All runs: the prototype, the quarter of (a1), the load vector as named, the same supports; `u_r / ref`, `ref = p R^2 / (E t) =
5e-5`. The prototype reproduces F-419's rcfd O1 numbers (`0.9027895620 .. 1.0950504406`) to `1e-9`.

1. **Where the default load is uneven (arithmetic).** A cell of the quarter is a plane rectangle (two axial generators at
   `theta_i`, `theta_{i+1}`), area `A_c = 2 R sin(dth / 2) dz`, normal `n_{i+1/2}`. Shell3's consistent pressure gives `p A_t /
   3` along the facet normal at each node of a triangle (`A_t = A_c / 2`). Cut along `(i, j)-(i + 1, j + 1)`, the two diagonal
   nodes of a cell lie in both triangles and get `A_c / 3` each, the two others `A_c / 6`: the split of a cell's force depends on
   the diagonal. At an interior node the four cells give `1/3 + 1/6` of each adjacent column: the force is radial, `p A_c cos(dth
   / 2)`. At a node of the free edge z = H the cell to its right gives `A_c / 6` along `n_{i+1/2}`, the one to its left `A_c / 3`
   along `n_{i-1/2}`: a tangential part `tan(dth / 2) / 3 = tan(pi / 128) / 3 = 8.1829e-3` of the radial one, the same sign at
   every edge node (a uniform circumferential edge shear); the bottom row z = 0 gets the opposite sign; the four corners get `A_c
   / 6` or `A_c / 3` in place of `A_c / 4`. Run D0 (`diag1.log`): the tangential ratio at the free edge `-8.1829e-3` at every
   node, as computed; `u_r / ref` in `[0.9027895613, 1.0950504415]` (O1 reproduced).
2. **Not the stiffness, not membrane locking (run D1).** Radial nodal loads of the polygon's tributary area (`p A_c`, halved on
   the edges of the quarter) on the same one-diagonal mesh give `u_r / ref` in `[0.9999999994, 1.0000000008]` (`diag1.log`):
   the stiffness of the one-diagonal facet mesh reproduces the uniform expansion to `8e-10`. Arithmetic: under a uniform hoop
   stress the CST nodal forces are the edge tractions shared by the edge nodes, independent of the diagonal (the diagonal edge's
   tractions cancel between the two triangles of the cell), so the uniform expansion of the polygon is an exact discrete solution
   for loads in proportion to the cell areas on any diagonal pattern. Membrane locking would stiffen, and would show here; it does
   not. The mirrored diagonal gives the same error with the tangential sign reversed (`diag2.log`, `pattern='other'`).
3. **Not cured by a consistent load (runs D2, D3; clause b3).** (D2) the work-equivalent load on the *true* cylinder, `f_i = p
   int N_i n dS`, `N_i` linear in `(theta, z)` on each triangle of the parameter grid, `n = (cos theta, sin theta, 0)`, `dS = R
   dtheta dz`, an 8 x 8 collapsed Gauss rule: tangential ratio `-8.1816e-3`, `u_r / ref` in `[0.9029864548, 1.0952550565]`
   (`9.70e-2`); (D3) the Galerkin load with the interpolated nodal radii as normal, `p sum_j M_ij n_j`: `-8.1812e-3`, `9.73e-2`.
   The tangential part comes from the support of `N_i`, not from the facet geometry: a node of the free edge has a hat function
   covering two triangles on one side and one on the other, so `int N_i n` leans towards the side with more area, to first order
   in `dth`, on the exact surface as well. **So 1 percent at every node is not reachable by a work-consistent load of the linear
   triangle on this mesh**: the consistent loads of D2 and D3 miss it by a factor of ten like the default.
4. **The amplifier: the soft end mode (runs D4, D5).** The uneven edge loads excite the inextensional ovalisation of the open
   end (bending, no hoop stretch), whose stiffness is that of bending. (D4) the error falls in proportion to `dz` (the edge
   force per node is proportional to the cell area): `1.84e-1, 9.72e-2, 4.95e-2, 2.50e-2` at NZ = 8, 16, 32, 64 (ratios `1.89,
   1.96, 1.98`); (D5) it grows as the shell thins: `4.46e-2, 9.72e-2, 2.17e-1` at t = 0.02, 0.01, 0.005. The mode is very
   sensitive: removing the tangential part of the top row only (keeping the corner and bottom-row unevenness) gives an error of `2.10`
   (`diag2.log`): the three uneven parts of item 1 largely cancel in the default; any one of them alone is worse.
5. **Diagnosis, the answer to (b1).** The cause is the *facet pressure lumping* in the sense of the triangle's consistent split
   `A_t / 3`, which on a curved mesh makes the share of each facet's force at a node depend on how the cells are cut (item 1),
   acting on the soft open-end mode (item 4). The one-diagonal pattern is what makes the uneven shares add up along the edge
   instead of alternating (the checkerboard of CY1 alternates them, `1.76e-3`); membrane locking is excluded (item 2).

## Design: the angle-weighted split (first principles)

### Derivation

Keep each triangle's force `p A e_3` (its resultant, the facet's own) and share it among the three nodes in proportion to the
interior angles: `F_i = p A (theta_i / (theta_0 + theta_1 + theta_2)) e_3`, no nodal moments (the default has none either).

1. **The split of a rectangle cell is `A_c / 4` per node for either diagonal.** A rectangle cut along a diagonal gives two right
   triangles of area `A_c / 2` with angles `(a, pi / 2, pi / 2 - a)`. A diagonal node has the angles `a` and `pi / 2 - a` in the
   two triangles: `(A_c / 2)(a / pi) + (A_c / 2)((pi / 2 - a) / pi) = A_c / 4`; an off-diagonal node has the right angle in one:
   `(A_c / 2)(1 / 2) = A_c / 4`. This is the split of the bilinear Shell4 on a rectangle (`int N_i dA = A_c / 4`).
2. **The cylinder then has the polygon's uniform expansion as its exact discrete solution.** Every node of a column gets `p A_c /
   4` along each adjacent cell's normal; an interior node gets `p A_c cos(dth / 2)` radially, a node of the symmetry planes
   `p (A_c / 2)` along `n_{1/2}` whose tangential part the symmetry condition takes; the free edge and the corners get the halves
   and quarters of that, without a tangential part. Equilibrium of a node with the hoop force `N` of its two facets: `2 N dz
   sin(dth / 2) = p A_c cos(dth / 2)`, `N = p R cos(dth / 2)`, and with item 2 of the diagnosis the CST reproduces it, so `u_r /
   ref = cos(dth / 2) = cos(pi / (4 N_q))` at every node, independent of NZ and of the diagonal pattern: `0.9996988187` for `N_q
   = 32` (error `3.0118e-4`), `0.9987954562` for `N_q = 16` (`1.2045e-3`). Run D6 (`diag1.log`, `diag2.log`): `0.9996988181 ..
   0.9996988195` on the one-diagonal mesh, the same to `1.4e-9` on the other diagonal, the checkerboard, the herringbone (cells
   cut by column) and NZ = 4; `0.9987954559 .. 0.9987954565` for `N_q = 16`.
3. **What it is not.** It is a lumping, not the work-equivalent load: the element's force is the same, its point of application
   moves (for a general triangle the first moment of the split differs from that of `A / 3` at the nodes), an error of the order
   of the element size per element, like any lumped load. On an equilateral triangle it is the default `A / 3` (up to rounding);
   on a flat plate cut into right triangles the interior nodal forces equal the default ones (each node: `1/3 + 1/6 + 1/3 + 1/6 =
   4 x 1/4` of the cell area), so the simply supported plate of ADR 0454 is unchanged to rounding (LD3).
4. **Rejected alternatives (diagnosis runs).** (D7) the angle-weighted split along the angle-weighted nodal normal (`n_i`
   normalised sum of `theta_i^e e_3^e`): `0.9898855718 .. 1.0088711659` (`1.01e-2`, a miss): on the symmetry columns the nodal
   normal is the one facet's normal, so their radial share `cos(dth / 2)` is not in the ratio of the uniform expansion to the
   interior nodes' full share, and the soft end mode amplifies that `3e-4` mismatch thirtyfold; (D8) `A / 3` along the nodal
   normal: `1.08e-1`; (D2, D3) the consistent loads: `9.7e-2`. The facet-normal split keeps each element's resultant exactly, so a
   closed surface gets a zero total force, and a crease gets each facet's own force direction.
5. **A non-rectangular mesh (runs D9, D10, `diag3.log`, `diag4.log`).** The quarter with the interior node rows moved axially,
   `z_ij = H j / NZ + 0.3 (H / NZ) sin(3 pi i / N_q) sin(pi j / NZ)` (warped cells, triangles neither right nor equal), one
   diagonal: the angle split gives `0.9932465311 .. 1.0042112359` (`6.75e-3`), the default `1.025e-1`, the exact Galerkin load
   `1.023e-1`. The split is not exact there (the angles no longer give equal cell shares), but its error converges: `2.52e-2,
   6.75e-3, 1.84e-3` at NZ = 8, 16, 32 (ratios `3.7, 3.7`), against the default's first order.

### Discretisation-error estimate (b2), registered

With the angle-weighted split, on a cylinder whose cells are rectangles (any diagonal pattern): `u_r / ref = cos(pi / (4
N_q))` at every node, the error `1 - cos(pi / (4 N_q)) ~ pi^2 / (32 N_q^2)` of the inscribed polygon (`3.01e-4` at `N_q = 32`),
none from NZ or the diagonals. On the jittered mesh: `6.75e-3` at 32 x 16, falling at a rate close to second order in the cell
size. With the default consistent split on the one-diagonal mesh: `~ 1.55 dz / H` relative (`9.72e-2` at NZ = 16), first order.

### D1. The element load (`shell3.rs`, a new function only)

`pub fn pressure_load_vector_angle_weighted(coords: &[Vec3], p: f64) -> Result<[f64; 18]>`: `facet(coords)?` (the refusals of
the facet, including the angle limit), the angles from the local coordinates by `atan2(|u x v|, u . v)` of the two edges at each
node, `F_i = p A theta_i / (theta_0 + theta_1 + theta_2)` along local `e_3`, moments zero, `to_global_vector`. Nothing else in
`shell3.rs` changes.

### D2. The opt-in load (`model.rs`, `loads.rs`)

`Load::ShellPressureAngleWeighted { set, pressure }` (non-exhaustive, a new variant after `ShellPressure`) and the builder
`Load::shell_pressure_angle_weighted(set, pressure)`. `add_load` refuses it as `ShellPressure` (unknown set, non-finite
pressure, an element of the set that is not Shell4 or Shell3: the `ShellPressure` arm's pattern gains the new variant, its body
unchanged). `nodal_load_vector` gets a new arm after the `ShellPressure` arm; `add_shell_pressure` gets a flag `angle_weighted`:
a Shell3 element takes `pressure_load_vector_angle_weighted` when it is set, a Shell4 element takes Shell4's own consistent load
either way (on a rectangle it is already `A / 4`, item 1; Shell4 has no diagonal). The `ShellPressure` arm passes `false`: the
same calls in the same order as before (B1). No other path reads the variant (shell models are refused on the paths that do
not take shells; `beam.rs` matches with `_ => {}`).

### Which paths

Every path that takes `Load::ShellPressure` (the static solve and the load vector, elimination and transformation) takes the
new load the same way. The default `Load::shell_pressure` is unchanged.

## Checks (limits fixed now)

| id | clause | check | limit | estimate behind it |
| --- | --- | --- | --- | --- |
| A1 | a1 | O1 mesh (32 x 16, one diagonal) with `Load::shell_pressure_angle_weighted`: `max_nodes |u_r / ref - 1|` | `1e-2` | design 2: `3.0118e-4` (D6) |
| DE1 | b2 | the same model: `max_nodes |u_r / ref / cos(pi / 128) - 1|` | `1e-7` | design 2: exact discrete solution, D6 `1.4e-9` |
| DE2 | b2 | `N_q = 16`, NZ = 16, one diagonal: `max_nodes |u_r / ref / cos(pi / 64) - 1|` | `1e-7` | D6 `3e-10` |
| CY2 | b2 | the other diagonal, the checkerboard, the herringbone (cell `(i, j)` cut by `i % 2`), 32 x 16: `max_nodes |u_r / ref / cos(pi / 128) - 1|` each | `1e-7` each | D6 `<= 1.4e-9` |
| CY3 | D2 | mixed quarter 16 x 8, cells with `i % 4 == 0` Shell4, the others two Shell3 (one diagonal), the new load: `max_nodes |u_r / ref / cos(pi / 64) - 1|` | `1e-7` | design 1, 2 (Shell4 on a rectangle `A / 4`; not prototyped) |
| CY4 | b3 | the jittered mesh of design 5, 32 x 16, the new load: `max_nodes |u_r / ref - 1|` | `1e-2` | D9 `6.7535e-3` |
| P1 | b2 | against the prototype: A1 min and max (`0.9996988181`, `0.9996988195`); CY4 min and max (`0.9932465311`, `1.0042112359`) | `1e-5` relative each | the prototype matched rcfd to `2.2e-9` (ADR 0454) |
| NC1 | b1 | default `Load::shell_pressure` on the O1 mesh: min and max against F-419's measured O1 `0.9027895620`, `1.0950504406` | `1e-8` relative each | the same code as ADR 0454's run |
| NC2 | b3 | default on the jittered mesh: `max_nodes |u_r / ref - 1|` | `> 5e-2` | D9 `1.025e-1` |
| LD1 | D1 | one Shell3 element in space (ADR 0454 arithmetic 15: `(0, 0), (0.9, 0.15), (0.3, 0.7)` placed by `Q`), `p = 1e5`: sum of the nodal forces against `p A e_3`, relative to `p A`; the nodal moments | `1e-12`; `|m| = 0` | design: `sum F_i = p A e_3` |
| LD2 | D1 | weights: the right isosceles triangle `(0, 0, 0), (1, 0, 0), (0, 1, 0)`: `F_z = p (1/4, 1/8, 1/8)`; an equilateral triangle against `shell3::pressure_load_vector`, every entry, relative to `p A / 3` | `1e-12` each | design 1, 3 |
| LD3 | D1 | SS plate of ADR 0454 (32 x 32, cut `(0)`, t/a 0.01, q = 1000): `|w_c(new) / w_c(default) - 1|` | `1e-10` | design 3: interior nodal forces equal to rounding |
| LD4 | D2 | Shell4 cylinder 16 x 8: `nodal_load_vector` of the new load equals that of `Load::shell_pressure`, bit for bit | exact | D2 |
| N1 | D2 | the new load on a Hex8 element set refused (`InvalidValue { key: "shell pressure" }`); a non-finite pressure refused (`key: "pressure"`); an unknown set refused (`UnknownSet`) | exact | D2 |
| B1 | b4 | the 8 digests of `sa_shell_pressure_bits` (default `Load::shell_pressure`: load vector and static solution of the one-diagonal and checkerboard Shell3 quarters, a Shell4 quarter, a mixed quarter) equal the base 32fbefad (`f540-scratch/b1-base.log`) | exact | D2 |
| B2 | b4 | the existing tests of the touched code pass unedited: `sa_shell3`, `sa_shell3_bits`, `si_shell` and every other `rcfd-fem` test binary whose name starts `si_shell` or `sa_`, release | pass | D2 |
| B3 | b4 | `cargo test -p rcfd-api --release --test baseline_hashes` (rcfd-api depends on rcfd-fem) | pass | no fluid file touched |

A1 to CY4, P1, NC1, NC2 are one test file `sa_shell_pressure_angle.rs` run once, the LD and N checks in the same file; B1 is
`sa_shell_pressure_bits.rs` (written and run on the base before this text, as disclosed).

## Planned mutations (on a scratch copy, one at a time)

| id | mutation | expected to be caught by |
| --- | --- | --- |
| MU1 | the new weights `A / 3` (angles ignored) | A1, DE1, CY2, LD2 |
| MU2 | node `i` takes the angle of node `i + 1` | LD2, A1, DE1 |
| MU3 | the new `nodal_load_vector` arm passes `angle_weighted = false` | A1, DE1, CY2 |
| MU4 | `add_load` skips the checks for the new variant | N1 |
| MU5 | the new function pushes along `-e_3` | LD1, A1 |
| MU6 | the default `ShellPressure` arm passes `angle_weighted = true` | B1, NC1 |
| MU7 | the new mode skips Shell4 elements | LD4, CY3 |

## Decision rule

- **done:** A1, DE1, DE2, CY2, CY3, CY4, P1, NC1, NC2, LD1 to LD4, N1, B1 to B3 met, every mutation caught by a registered check,
  no amendment after a run.
- **partial:** a check missed (recorded as a `*_known_miss`) or not run, a mutation not caught, or a check met only after an
  amendment written after its run.
- **todo:** the load does not build.

## Limits (not covered)

- L1. The split is a lumping, not the work-equivalent load: on a general triangle the point of application of the element's
  force moves by up to the order of the element size; the default consistent load stays the default.
- L2. It is exact on the cylinder only because the cells are rectangles (design 1, 2); on an irregular triangulation of a curved
  surface it converges but is not exact (CY4, `6.8e-3` estimated at 32 x 16).
- L3. Shell4 elements keep their consistent load in the new mode; a warped Shell4 on a curved mesh was not studied.
- L4. The open end's inextensional mode amplifies any uneven nodal load (D7: `3e-4` into `1e-2`): a different load (a point load,
  a traction near a free edge) on a coarse open shell carries the same sensitivity; not studied.
- L5. A true-surface (curved-geometry) pressure was evaluated as a diagnosis only (D2) and is not provided: on linear triangles
  it has the same first-order edge bias as the default.

## Open questions

Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation):

1. The form of the opt-in: a new `Load` variant (taken) rather than a field on `ShellPressure` (which would change the variant's
   fields and the default arm in every match) or a model-wide option.
2. Shell4 elements keep their own consistent load in the new mode (taken; design D2).
3. The facet-normal angle split (taken) rather than a nodal-normal load (D7 misses the acceptance).

Not taken (it would change default results):

4. Whether the angle split should become the default pressure of Shell3: options (a) keep the opt-in; (b) make it the default
   (changes every Shell3 pressure result and the B1 digests, needs the maintainer). Recommendation (a), with the documentation
   pointing curved open shells to the opt-in.
