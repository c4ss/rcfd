# ADR 0629: F-304 registration: planar and sector sliding interfaces, with the periodic pairs kept by the join

Status: **REGISTERED 2026-11-16, written before any code of F-304 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-304 was written and nothing was run; the code of the base was read
only. Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-304 (deps F-250), split from [ADR 0216](0216-f250-registration-sliding-interface-on-a-cylinder.md) question 3.
Parents: ADR 0216 (F-250: the sliding interface on a cylinder, its GCL closure M3, its old-flux projection M4, its loop SL and
its lessons), [ADR 0109](0109-wave43b-cylindrical-rotor-interface-sector.md) and F-114 (the planar and cylindrical joins of
`rcfd_mesh::non_conformal`), [ADR 0205](0205-f295-registration-one-diffusivity-per-periodic-seam-pair.md) (F-295: the
periodic pairs of `FvGeometry::periodic_pairs`), [ADR 0033](0033-wave20b-dynamic-mesh.md) (swept volumes, the moving step).
Number 0629 given by the lead. Branch `f304-work` from the local main 9cf95fc3, worktree `w49i-work/f304`, scratch
`w49i-work/f304-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number); each registered run made once; existing tests not edited;
defaults bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new
dependency, no unsafe code; clean-room. The roadmap files are not edited (the entries go in "For the lead"). Every choice below
that the maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation), except where listed as open.

Clean-room: the method is derived below from the definitions of ADR 0216 (M1 to M4), the join of F-114 / ADR 0109 (polygon
clipping, the point registry, hanging nodes) and the definition of a periodic image (a point `x` and its image `T x` are the same
point of the periodic domain). No source was read for this work and no vendor code, text or install was used. The reference
values are closed forms derived here.

## What F-304 asks

The acceptance, binding:

> A planar sliding interface and a periodic sector join conserve mass and a uniform flow to round-off; periodic pairs are kept by the join.

The lead's brief adds: every limit not stated in the acceptance is registered here before the run; library first; a planar
(translating) interface and a sector join for a periodic rotor sector; an interface between sectors of different pitch is out of
scope unless trivial (stated); register mass conservation per step to round-off with the limit in units of the solver tolerance,
a uniform flow preserved to round-off across the moving interface, periodic pairs kept (count and identity), a negative control
and mutations; no case-file key (that is F-303); defaults bit-identical.

Clauses:

- **c1** a planar sliding interface conserves mass per step to round-off;
- **c2** a planar sliding interface keeps a uniform flow to round-off;
- **c3** a periodic sector sliding join conserves mass per step to round-off;
- **c4** a periodic sector sliding join keeps a uniform flow to round-off;
- **c5** the join keeps the periodic pairs of the blocks (count and identity), and the pairs it makes are what it says;
- **c6** defaults bit-identical (no case key; existing paths unchanged).

## What the code read shows (base 9cf95fc3, read only)

1. `join_non_conformal` / `join_non_conformal_cylindrical` (`non_conformal.rs`, private `join_impl`): the patches must cover each
   other (tiling error above `1e-6` refused); the cylindrical chart has the full period `2 pi r` (shifts `0, +-P` in the point
   registry). The merged patches are rebuilt by `Mesh::from_face_lists` from `(name, kind, size)` only, so **`periodic_with` and
   `rotation` are dropped** (the module documentation says so; the G2 unit test re-applies `set_periodic_rotation` by hand).
2. A sector rotor turned by `theta` against a stator sector of the same pitch `Theta` covers `[theta, theta + Theta]` against
   `[0, Theta]`: the part `[Theta, Theta + theta]` of the rotor patch faces the stator patch only through the periodic image
   (`[0, theta]` turned by `Theta`). The existing join refuses this pair (not covered). The same holds for a planar slider
   periodic in the slide direction (translation by the period `t`).
3. `rcfd_mesh::sliding` (F-250): `SlidingMesh::face_map_from` maps internal faces by `(owner, neighbour)` and boundary faces by
   position; `rotor_swept_volumes` is rotation only.
4. `rcfd_fv::sliding`: `interface_balance(mesh, faces, target)` (graph of internal faces only), `sliding_step` (M3, M4),
   `PimpleSolver::step_sliding` (refuses a patch list that differs in number, names, kinds or sizes).
5. The FV coupling of periodic faces: `FvGeometry::new` pairs the faces of every patch with `periodic_with` geometrically (face
   centres under the patch rotation, or under the translation `mean(partner) - mean(patch)`); the pressure couples a pair
   implicitly with the orthogonal coefficient `gamma |S| (p_partner - p_own) / (dn_f + dn_g)` and `phiHbyA` takes the
   distance-weighted mean of the two owners (the partner's vector rotated), so the two faces of a pair carry opposite fluxes to
   round-off (`pressure.rs`, `boundary_hby_a`, `coupled_gamma`); no non-orthogonal correction on coupled faces. The field
   condition of a `Cyclic` patch is `Periodic`, which holds no per-face data. The moving step's mesh flux is used on every face
   (`relative_flux`), the old-level Rhie-Chow defect on internal faces only. A rotating 3-D sector with rotational seams runs on
   the moving step already (`exp_rot_pab.rs`).

## The model (derived)

**M1. Periodic chart.** The two interface patches lie on one plane or one cylinder (as F-114 / ADR 0109). Chart coordinates
`(s, z)`: on a plane, `s` along the unit slide direction `e1 = t / |t|` (the period vector `t` must lie in the plane), `z` along
`e2 = n x e1`; on a cylinder of radius `r` about the axis, `s = r phi` with `phi` measured from a direction `e1` placed between
the two patches (so that neither crosses the branch cut of `atan2`; a pitch `Theta < pi` is required), `z` the signed axial
coordinate (ADR 0109's sign rule). The period in `s` is `P = |t|` (plane) or `P = r Theta` (sector); the chart image `(s + k P,
z)` of a point is its image under `T^k` (translation by `k t`, or rotation by `k Theta`): `chart.point` places it there.
Each patch must span exactly one period in `s` (the extents equal `P` within the merge tolerance); a stator of another pitch is
refused (out of scope, stated: an interface between sectors of different pitch needs a pitch-change model, not a join).

**M2. The periodic join.** Every face `a` of the rotor patch is clipped (Sutherland-Hodgman, as F-114) against every stator face
`b` shifted by `k P`, `k in {-1, 0, 1}`:

- `k = 0`: the overlap is an ordinary internal face, owner the rotor cell, neighbour the stator cell (as F-114).
- `k != 0` (a **wrap overlap**): the overlap lies physically on the rotor side at `s` and on the stator side at `s - k P`. It
  becomes a **pair of boundary faces**: the rotor copy (the overlap polygon at `s`, owner the rotor cell, normal out of it) in a
  new patch `sliding_wrap_a`, and the stator copy (the same polygon at `s - k P`, reversed, owner the stator cell) in
  `sliding_wrap_b`; the two patches are made a periodic pair whose transform maps the rotor copy onto the stator copy
  (translation `-k t`, or rotation by `-k Theta`). For the offsets of M4 only `k = +1` occurs; the join refuses wrap overlaps of
  two signs.

Points: the registry of F-114 without period shifts; a vertex of a wrap overlap is looked up at `s` (rotor side) and at `s - k P`
(stator side) and created (by `chart.point`) where absent, so the images of the stator points that fall inside the rotor patch,
and of the rotor points that fall inside the stator patch, become ordinary points on their side, and every face edge on the
interface receives the registry points on it (hanging nodes) as in F-114. The stator point at `s = P` and the image of the stator
point at `s = 0` are one point (merged within the tolerance), so the direct and the wrap regions meet without a gap. Both
patches are tiled by the direct and the wrap overlaps together (the tiling error is checked as in F-114). Every cell stays closed:
the rotor cells' interface faces are replaced by their direct overlaps and their rotor copies, the stator cells' by their direct
overlaps and their stator copies.

**M3. Periodic pairs kept.** The merged boundary patches keep their periodic partner and transform: for each patch of either
block with `periodic_with`, the joined patch of that name gets the same partner and transform (`set_periodic` or
`set_periodic_rotation`); a name present in both blocks must have the same kind, partner and transform (else an error). The
faces of a merged patch are the rotor's then the stator's, so the geometric pairing of `FvGeometry` pairs rotor seam faces with
rotor seam faces and stator with stator (their transforms are the same). Seam faces that end on the interface receive hanging
nodes; their partners receive the images of the same nodes (M2), so the pairs still match face centre to face centre. The two
wrap patches are appended last, after every original patch, so the original boundary faces keep their positions at every offset
and the patch list (names, kinds, order) does not depend on the offset; only the two wrap patch sizes change.

**M4. The sliding mesh.** As ADR 0216 M1 with the rotor moved from its reference position by the offset `delta`: translation
`delta e1` (planar) or rotation by `delta` about the axis (sector), always from the reference points. **The offset is restricted
to `0 <= delta < P_motion`** (`|t|`, or `Theta`): one period. A rotor turning further needs the cells re-imaged by one period
(their vectors turned by `-Theta`) between two steps; not in this item (stated, follow-up proposed). Face identity across a step
(M2 of ADR 0216): an internal face off the interface keeps its `(owner, neighbour)`; a boundary face of an original patch keeps
its position; the direct interface faces and the wrap faces are new.

**M5. Mesh flux (GCL).** As ADR 0216 M3, with the rotor's swept volumes of the rigid motion (translation or rotation back from
the new mesh) on the rotor faces (owner a rotor cell, and neighbour a rotor cell or an original boundary face), zero on stator
faces and empty faces, and the interface values from the area-weighted minimum-norm closure on the graph whose edges are the
direct interface faces **and the wrap pairs** (edge between the owner of the rotor copy and the owner of the stator copy; value
`x` on the rotor copy, `-x` on the stator copy, so the pair is antisymmetric as the FV coupling needs). For a planar slide the
interface is flat and no sliver changes: the closure values are zero to round-off. For a sector, the slivers of the chord
polygon change with the overlaps (ADR 0216 M3) and the closure carries them.

**M6. Old flux on the new faces.** As ADR 0216 M4: mapped faces keep their value; a direct interface face gets the interpolation
`(u_old)_f . S_f`; a wrap pair gets `(w u_a + (1 - w) R u_b) . S_a` on the rotor copy (`w = dn_b / (dn_a + dn_b)` the weight of
the rotor cell, `R` the rotation that maps vectors of the stator side to the rotor side, identity for a translation) and minus
that on the stator copy; then the area-weighted minimum-norm correction on the graph of M5 makes the old flux of every cell
divergence-free. The old-level Rhie-Chow defect is zero on the direct interface faces (ADR 0216 M4) and is not defined on
boundary faces. The uniform-flow argument of ADR 0216 M4 carries over: for a uniform `U` invariant under the transform (any `U`
for a translation; an axial `U` for a rotation about that axis) the interpolations are `U . S` exactly, the corrections vanish to
round-off, the mesh flux closes the GCL, and the coupled faces' `phiHbyA` is `U . S`, so the state stays uniform.

**M7. Mass across the interface.** The interface flux out of the rotor is `Q_I = sum_{direct} phi_f + sum_{rotor copies}
phi_f`. Summing the continuity of all rotor cells: `Q_I - Q_in,rotor = (rotor continuity residuals) - (net flux of the rotor
seam pairs)`, and a seam pair carries opposite fluxes to round-off (read 5). So `Q_I` equals the inflow through the rotor to the
accuracy of the pressure solve. The same holds with the stator copies on the stator side.

## Settings and code (additive)

**S1 (`rcfd-mesh`).** `non_conformal.rs`: the private `Registry`, `Chart`, `clip_polygon` and `shoelace` become `pub(crate)`
(visibility only; no line of behaviour changes). New module `sliding_periodic`:
`Periodicity::{Translation { period }, Rotation { origin, axis, angle }}`;
`join_non_conformal_periodic(a, patch_a, b, patch_b, periodicity, tol) -> Result<(Mesh, PeriodicJoinReport)>` (M1 to M3;
`PeriodicJoinReport { join: JoinReport, wrap_pairs: Vec<(usize, usize)>, shift: i32 }`, `join.interface_faces` the direct
faces);
`PeriodicSlidingInterface::{planar(rotor, rotor_patch, stator, stator_patch, period, tol), sector(rotor, rotor_patch, stator,
stator_patch, origin, axis, angle, tol)}`, `mesh_at(offset)`, `n_rotor_cells`, `period`; `PeriodicSlidingMesh { mesh, report,
offset, .. }` with `is_interface`, `is_wrap`, `is_rotor_face`, `face_map_from`, `rotor_swept_volumes(old_offset)`,
`wrap_rotation()` (M4). `sliding.rs`, `motion.rs`, the existing joins: not edited.

**S2 (`rcfd-fv`).** `sliding.rs`: `interface_balance` is split into a new `graph_balance(n_cells, edges, weights, target)` that
it calls with the same edges in the same order (the same operations, so the same bits). New module `sliding_periodic`:
`PeriodicSlidingStep` (public fields: `geom`, `old_offset`, `offset`, `mesh_flux`, `vol_old`, `phi_old`, `old_defect`,
`interface_faces`, `wrap_pairs`, `gcl_residual`, `flux_residual`) and `periodic_sliding_step(old_geom, old, new, phi_old, u_old,
dt)` (M5, M6).

**S3 (`rcfd-fv`, `pimple/moving.rs`).** The body of `step_sliding` moves into a private helper with a list of patches whose size
may change (empty for `step_sliding`, so its checks and messages are as before); new `PimpleSolver::step_sliding_periodic(dt,
step, wall_dpdn)` calls it with the two wrap patches, which must be `Cyclic` in both meshes. `step`, `step_ale*`, `replace_mesh`
not edited.

**S4.** No case-tree, CLI, Studio or schema change (F-303). `docs/dynamic-mesh.md` gets prose.

## Set-ups

Solver settings PS6: ADR 0216's PS with **six** non-orthogonal correctors (ADR 0216 amendment 2: two correctors leave the lagged
non-orthogonal flux above the continuity limit on these meshes; six is the count at which its continuity checks were met; taken
here before any run, disclosed): `PimpleSettings::piso(nu)`, central convection, implicit Euler, `ddt_correction` on, momentum
and pressure solvers `tol_abs = 1e-16`, `tol_rel = 1e-13`, 5000 iterations. The **solver tolerance** below is the pressure
solver's `tol_rel = 1e-13`. Loop: ADR 0216 SL with `mesh_at`, `periodic_sliding_step` and `step_sliding_periodic`.

- **PP** (planar 2-D pair, depth 0.1): stator `rectilinear_2d(16 uniform cells on [0,1], 8 on [0,0.5])`, slider (rotor)
  `rectilinear_2d(20 on [0,1], 8 on [0.5,1])`, each with `xmin`/`xmax` a translational pair (`set_periodic` both ways);
  interface: slider `ymin`, stator `ymax`; `t = (1, 0, 0)`. Slide speed `V = 1`, `dt = 0.0049`, 100 steps (offset 0.49, 7.8
  stator pitches). The step `V dt = 0.0049` is chosen so that no step end makes a slider node meet a stator node: they meet at
  `delta = m / 80`, and `0.0049 n = m / 80` needs `n` a multiple of 125.
- **SS** (sector 2-D pair, depth 0.1): rotor `annulus_sector(24, 8, 1, 1.5, pi/3)` (interface `outer`), stator
  `annulus_sector(20, 8, 1.5, 2, pi/3)` (interface `inner`); 352 cells, one sixth of ADR 0216's pair P (same pitches, same
  overlap pattern). `Omega = 1`, `dt = 0.005`, 100 steps (0.5 rad, 9.5 stator pitches, below `Theta = 1.047`).
- **S3** (sector 3-D pair): rotor `annulus_3d_sector(12, 4, 2, 1, 1.5, 0.4, pi/3)` (interface `outer`), stator
  `annulus_3d_sector(10, 4, 3, 1.5, 2, 0.4, pi/3)` (interface `inner`); axial breakpoints differ (2 against 3 layers), so the
  seam faces receive hanging nodes. `Omega = 1`, `dt = 0.01`, 40 steps (0.4 rad, 3.8 stator pitches).

Flows:

- **PU** (PP, uniform): `U = (1, 0.5, 0)` fixed on `ymin` (stator bottom) and `ymax` (slider top); start `u = U`, `p = 0`; `nu
  = 0.1`. `Q_in = 0.05` through the bottom.
- **PF** (PP, non-uniform through-flow): inlet `ymin`, `u_b = (0, 0.5 (1 + 0.5 sin(2 pi x_f)), 0)`; outlet `ymax`, `p = 0`,
  zero-gradient velocity; start `u = (0, 0.5, 0)`, `p = 0`; `nu = 0.1`. `Q_in = 0.05` (the sine sums to zero over the 16 face
  centres).
- **SF** (SS, radial source, ADR 0216's SF on the sector): inlet `inner` (on the rotor), `u_b = U_in x_hat_f` at the new face
  centres every step, `U_in = 1`; outlet `outer`, `p = 0`, zero-gradient velocity; start `u = U_in (r_i/r) x_hat`; `nu = 0.1`.
- **SU** (S3, uniform axial): `U = (0, 0, 1)` fixed on `inner`, `outer`, `zmin`, `zmax`; start `u = U`, `p = 0`; `nu = 0.1`.

## Registered checks

All in release. Mesh checks are unit tests in `crates/rcfd-mesh/src/sliding_periodic.rs`, flow checks in
`crates/rcfd-fv/tests/sliding_periodic_f304.rs`. "Every step" is the largest value over the steps of the run. Normalisations
as ADR 0216: `mean(V)` the mean cell volume, `A_I` the summed area of the direct interface faces and the rotor copies.

### K. The periodic join and the sliding mesh (unit, `rcfd-mesh`)

Offsets: PP `{0, 0.0123, 0.3, 0.9}`; SS and S3 `{0, 0.0123, 0.3, 1.0}` rad.

| Id | Check | Limit |
| --- | --- | --- |
| K1 | PP and SS, offsets `0.0123 k`, `k = 0..4`, each against the next: every face of the new mesh that is neither a direct interface face nor a wrap face has a partner with the same owner and neighbour (boundary: same owner and patch); the unmapped faces are exactly the direct interface and wrap faces; no old face used twice | exact |
| K2 | (i) PP from 0.0123 to 0.0173: every slider face `abs(W_f - (S_f . e1) 0.005) <= 1e-12 abs(S_f) 0.005` (a planar face translated by `d` sweeps `S . d`); (ii) SS from 0.0123 to 0.0173 rad: every radial rotor face against `L (r_2^2 - r_1^2) sin(0.005) / 2` (ADR 0216 K2 (ii)); (iii) `W_f = 0` on every stator, direct interface and wrap face | (i) as stated; (ii) relative `1e-12`; (iii) exact |
| K3 | total volume at each offset against the closed form: PP `0.1`; SS `0.1 (10 . 4 sin(pi/60) - 12 sin(pi/72)) = 0.1570005`; S3 `0.4 (5 . 4 sin(pi/30) - 6 sin(pi/36)) = 0.62705` (the fan areas of the outer stator polygon and the inner rotor polygon; the merged interface polygon is periodic, so the volume does not depend on the offset) | relative `1e-12` each |
| K4 (c5) | periodic pairs kept, PP, SS and S3 at each offset: (a) the joined seam patches keep `periodic_with` and the transform of the blocks; (b) count: `FvGeometry::periodic_pairs` of the joined mesh has `n_seam + n_wrap` pairs, `n_seam = 16` (PP: 8 + 8), `16` (SS), `20` (S3: 8 + 12), `n_wrap` the join's wrap count; (c) identity: the owner-cell pairs of the seam pairs equal those of the two blocks' own `periodic_pairs` (stator cells offset by the rotor count); (d) the wrap pairs found by `FvGeometry` equal the join's `wrap_pairs`; (e) `n_wrap > 0` at every offset but 0, `n_wrap = 0` at 0 | exact each |
| K4n (negative control) | the existing joins (`join_non_conformal` for PP, `join_non_conformal_cylindrical` for SS) at offset 0: `periodic_pairs` count | `0` (it drops the pairs, read 1), against K4's `16` |
| K5 | tiling and closure at each offset: the join's worst tiling error (direct and wrap overlaps together); the worst cell closure `abs(sum sigma S_f) / sum abs(S_f)` (F-114's measure) | `<= 1e-12`; `<= 1e-13` (ADR 0109 G2 met `1e-14`; the images add one rotation's round-off, a few `1e-16` per face) |
| K6 | refusals: offset `-0.1`, offset `= P` (1 and `pi/3`), `NaN`; a period vector out of the plane (`(1, 0.1, 0)`); a stator sector of another pitch (`annulus_sector(20, 8, 1.5, 2, pi/4)`) | `Err` each, the message naming `offset`, `plane`, `period` |

### P. Planar sliding interface (c1, c2)

| Id | Check | Limit |
| --- | --- | --- |
| P1 (c1) | PF, every step `abs(Q_I - Q_in) / Q_in` | `<= 1e-10` (`1e3` solver tolerances) |
| P2 | PF, every step `abs(Q_out - Q_in) / Q_in` | `<= 1e-10` |
| P3 | PF, every step `continuity` | `<= 1e-9` (ADR 0101) |
| P4 | PF, every step the GCL residual `max_c abs(V1 - V0 - dt sum sigma m) / mean(V)` (computed in the test) | `<= 1e-12` (ADR 0101) |
| P5 | PF, every step from the second the remapped old flux `max_c abs(sum sigma phi_old) / max abs(S_f)` (ADR 0216 S5c; the first step carries the start flux of `PimpleSolver::new`) | `<= 1e-9` |
| P6 | PF, every step `sum_pairs abs(phi_f + phi_g) / Q_in` over all periodic pairs (seams and wrap) | `<= 1e-12` (round-off of the two faces of a pair, read 5) |
| P7 (set-up) | PF, the number of steps in which the owner-cell pairs of the interface (direct and wrap) change | `>= 30` (derived: 40, the meetings at `delta = m/80`, `m = 1..39`, and the first step from the coincident start) |
| P8 (c1, c2) | PU, every step `max_c abs(u_c - U) / abs(U)` and `max_c abs(p_c - p_0) / abs(U)^2` | `<= 1e-10` each |
| P9 (c1) | PU, every step `abs(Q_I - Q_in) / Q_in`, the GCL residual and `continuity` | `<= 1e-10`, `<= 1e-12`, `<= 1e-9` |
| PN1 (negative control of P1) | PF, last step, the direct interface faces alone: `abs(sum_direct phi - Q_in) / Q_in` | `>= 0.1` (the wrap pairs carry the flux of the slider's part beyond the seam, about `delta / |t| = 0.49` of it) |
| PN2 (negative control of P8) | PU with the old flux of both wrap copies set to zero after `periodic_sliding_step` (as if the wrap faces were walls): the largest `max_c abs(u_c - U) / abs(U)` | `>= 1e-6` (estimate: a missing `U . S` of about `0.5 . 0.05 . 0.1` in a cell of `3e-4` volume, over `dt`, of order `1e-2`) |
| X0p | PF, PU: worst direct-interface non-orthogonality; slide at least one stator pitch | `<= 70 deg` (ADR 0112); `0.49 >= 0.0625` |

### S. Periodic sector sliding join (c3, c4)

| Id | Check | Limit |
| --- | --- | --- |
| S1 (c3) | SF, every step `abs(Q_I - Q_in) / Q_in` | `<= 1e-10` (`1e3` solver tolerances) |
| S2 | SF, every step `abs(Q_out - Q_in) / Q_in` | `<= 1e-10` |
| S3 | SF, every step `continuity` | `<= 1e-9` |
| S4 | SF, every step the GCL residual | `<= 1e-12` |
| S5 | SF, every step from the second the remapped old flux | `<= 1e-9` |
| S6 | SF, every step `sum_pairs abs(phi_f + phi_g) / Q_in` | `<= 1e-12` |
| S7 (set-up) | SF, steps in which the owner-cell pairs of the interface change | `>= 45` (derived as ADR 0216 amendment 2 (c): 58) |
| S8 (c4) | SU, every step velocity and pressure as P8 | `<= 1e-10` each |
| S9 (c3) | SU, every step `abs(Q_I) / (abs(U) A_I)` (axial flow: no net flux), the GCL residual, `continuity` | `<= 1e-10`, `<= 1e-12`, `<= 1e-9` |
| SN1 (negative control of S1) | SF, last step, direct faces alone, as PN1 | `>= 0.1` (wrap share about `theta / Theta = 0.48`) |
| SN2 (negative control of S8) | SU with the mesh flux of the direct interface faces and the wrap faces set to zero after the step build (the GCL of the slivers not closed): the largest velocity measure | `>= 1e-6` (ADR 0216 N1 measured `1.04e-3` on the cylinder) |
| X0s | SF, SU: worst direct-interface non-orthogonality; turn at least one stator pitch | `<= 70 deg`; `0.5 >= pi/60`, `0.4 >= pi/30` |

Printed, no limit: SF `max abs(u - U_in r_i / r) / U_in` (ADR 0216 printed `6.19e-3` on the full annulus; the wrap faces have
no non-orthogonal correction, read 5); PF and SF wrap counts.

### E. Refusals of the solver path

| Id | Check | Limit |
| --- | --- | --- |
| E1 | `step_sliding_periodic` with BDF2, and with a step of PP on a solver of SS: `Err` each, the solver's `u`, `p`, `phi`, time unchanged bit for bit | as stated |
| E2 | `periodic_sliding_step` with an old flux one face short: `Err` | as stated |

### D. Defaults (c6)

| Id | Check | Limit |
| --- | --- | --- |
| D1 | no case key, so a case is unchanged by construction; `cargo test -p rcfd-api --release --test baseline_hashes` (moving.rs is a shared solver file) | pass |
| D2 | the existing suites unedited: `rcfd-mesh` unit tests (`non_conformal`, `sliding`, `motion`), `rcfd-fv` `sliding_interface_f250` and `sliding_interface_f250_records` (R1 to R3 of ADR 0216 hash the library paths) and the `rcfd-fv` unit tests of `sliding` | pass |

### Mutations (scratch, not committed)

Each applied alone to the final tree on a scratch copy, the named tests run once; each must fail.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | the join does not re-apply the periodic partners of the merged patches | K4 |
| MU2 | the wrap transform's sign flipped (rotor copy to stator copy by `+k`) | K4 |
| MU3 | `periodic_sliding_step`: the stator copy's old flux given `+x` instead of `-x` | P5 or P8 |
| MU4 | `rotor_swept_volumes`: zero | K2 |
| MU5 | `periodic_sliding_step`: the closure of M5 skipped (interface and wrap mesh flux left at zero) | S8 or S9 |

### Decision rule

**Done**: K4 (c5), P1, P8, P9 (c1, c2), S1, S8, S9 (c3, c4) and D1, D2 (c6) met, the set-up checks (P7, S7, X0) and the
negative controls (K4n, PN1, PN2, SN1, SN2) held, and every mutation caught. A miss of an acceptance check, or a control or
mutation that does not show its check can fail: **partial** with the miss named. The periodic join cannot be built or the loop
does not run: **todo**. Every other check is supporting and reported.

## Limits (not covered)

The offset is limited to one period (`0 <= delta < P`); a rotor turning further needs the re-imaging of M4 (follow-up). Sectors
of different pitch (refused). The coupled wrap faces carry the orthogonal pressure coefficient only (no non-orthogonal
correction on coupled faces in the solver, read 5): accuracy at the wrap seam is not measured against a limit. BDF2, turbulence,
energy, a face viscosity, body forces and the other refusals of `step_sliding`; partial cover; more than one interface; the case
tree (F-303); the compressible solver; parallel runs.

## Open questions taken on the standing instruction

- **Q1. Wrap overlaps as periodic boundary pairs or as internal faces with a transformed geometry.** (a) Boundary pairs in two
  appended patches (taken: the FV coupling of periodic pairs exists and is antisymmetric; the original boundary positions do not
  move); (b) internal faces (wrong `delta` and weights across the seam, vectors not rotated). Taken (a) on the maintainer's
  standing instruction of 2026-11-11 (take the recommendation).
- **Q2. Offset range.** (a) One period, re-imaging as a follow-up (taken); (b) re-imaging now (a solver operation that turns the
  rotor's cell vectors and their history between two steps). Taken (a) on the standing instruction.

## Amendment 1 (2026-11-16, written after the registered run and before any corrected run)

The registered run (once, release, two test threads, 2.8 s for the eight flow tests; the K unit tests before it, once, 0.1 s)
met every check except two, with one cause:

| Id | Limit | Measured |
| --- | --- | --- |
| P1 (c1) | `abs(Q_I - Q_in) / Q_in <= 1e-10` | `2.00e0` |
| P9 (c1, net flux part) | `abs(Q_I - Q_in) / Q_in <= 1e-10` | `2.00e0` (its GCL `1.05e-14` and continuity `5.83e-15` met) |

Diagnosis (from the printed numbers; nothing re-run): `Q_I` is defined (M7, and the test) as the flux **out of the rotor**
through the interface. In SF the inflow enters the rotor (its inner patch) and leaves through the interface, so `Q_I = +Q_in`
(S1 met, `1.06e-15`). In PF and PU the inflow enters the **stator** (the bottom `ymin` is a stator patch) and reaches the slider
through the interface, so `Q_I = -Q_in`: the measured `2.00` is `abs(-Q_in - Q_in) / Q_in`. The registered measure has the
wrong sign for the planar set-ups; a defective set-up, not a loss of mass (P2 `5.55e-16` and P3 `2.05e-12` met in PF). The
registered negative control PN1 held (`1.468`) with the same sign error, so it is corrected too. P1 and P9 stay as
registered, as ignored `*_known_miss` records with their limits and measured numbers.

Corrected checks, registered here before they run (each run once, the set-ups unchanged):

| Id | Set-up | Check | Limit |
| --- | --- | --- | --- |
| P1c (c1) | PF | every step `abs(Q_I + Q_in) / Q_in` (the flux into the slider through the interface, direct faces and rotor copies, against the inflow through the stator) | `<= 1e-10` (`1e3` solver tolerances) |
| P9c (c1) | PU | every step `abs(Q_I + Q_in) / Q_in`, with P9's GCL and continuity | `<= 1e-10`, `<= 1e-12`, `<= 1e-9` |
| PN1c (negative control of P1c) | PF | last step `abs(sum_direct phi + Q_in) / Q_in` | `>= 0.1` |

The decision rule is unchanged, with P1c in place of P1 and P9c in place of P9.
