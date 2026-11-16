# ADR 0646: F-711 registration: periodic (sector) sliding in the case tree, a sector rotor-stator generator and the sector against the full annulus

Status: **REGISTERED 2026-11-16, written before any code of F-711 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library, tree or test code of F-711 was written and nothing was run; the code of the base was
read only (and built, unchanged, to warm the target directory; no test ran). The arithmetic below was done in
`w49i-work/f711-scratch/arith.py` (disclosed; it runs no solver). Results go in the addenda at the end; nothing above the
addenda is changed after a run.

Roadmap item F-711 (deps F-303, F-704), proposed by [ADR 0637](0637-f303-registration-sliding-interface-in-the-case-tree.md)
(F-303) question Q3 and by [ADR 0634](0634-f704-registration-sliding-beyond-one-period-re-imaging-of-the-rotor.md) (F-704)
question Q3. Parents: ADR 0637 (the table `[mesh.sliding]`, `crates/rcfd-api/src/sim/sliding.rs`, its set-ups TP, TS, NAc and
WT, its follow-ups and its Q7 finding on symmetry planes), [ADR 0629](0629-f304-registration-planar-and-sector-sliding-interfaces.md)
(F-304: the periodic join, the step M5, M6), ADR 0634 (F-704: `mesh_turned`, the re-imaging step R2, its check F of the sector
against the full annulus), [ADR 0638](0638-f710-registration-sliding-interface-at-coinciding-nodes.md) (F-710: the coupling of
`sliding_step` at coinciding nodes), ADR 0640 and ADR 0641 (F-720, F-721, read on their branches: the base periodic step keeps
a uniform flow at coincidences but leaves the parts' means in the remapped flux for a flow without the parts' symmetry; F-721's
coupled periodic step is not in this base). Number 0646 given by the lead. Branch `f711-work` from the local main d40cd665
(F-304, F-704, F-303 and F-710 merged), worktree `w49i-work/f711`, scratch `w49i-work/f711-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number); each registered run made once; existing tests not edited;
defaults bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new
dependency, no unsafe code; clean-room. The roadmap files are not edited. Every choice the maintainer would normally be asked
about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except where listed as open.

Clean-room: the method is ADR 0629's periodic join and step and ADR 0634's re-imaging (derived there from the definition of a
periodic image and of a vector field periodic under a rotation, `v(R x) = R v(x)`); the wall conditions are ADR 0216 M6 /
ADR 0637 D4 read in the image of the rotor (below, P2). No source was read; no vendor code, text or install was used.
Reference values are the repository's own runs (the full-annulus tree run of ADR 0637) and closed forms (a uniform field).

## What F-711 asks

The acceptance, binding:

> A case file runs a rotor sector sliding against a stator sector of the same pitch beyond one pitch (PeriodicSlidingInterface::mesh_turned) through an optional table, with a sector rotor-stator generator; the sector run against the full-annulus tree run of ADR 0637 within a registered limit; defaults bit-identical. Every limit not stated here is registered in the ADR before the run.

Clauses:

- **c1** a case file runs a rotor sector sliding against a stator sector of the same pitch, beyond one pitch, through
  `PeriodicSlidingInterface::mesh_turned` (the re-imaging steps of ADR 0634 in the run), through an optional table, with a
  sector rotor-stator generator;
- **c2** the sector run against the full-annulus tree run of ADR 0637 (`[mesh.sliding]` on
  `annulus_3d_cylindrical_interface`) within a registered limit;
- **c3** defaults bit-identical: a case that does not ask for the sector sliding runs the same operations; no hash table and
  no golden edited.

The item's title names planar sliding too; the acceptance names the sector only. The planar slider has no generator in the
tree (a periodic channel pair); it is left out here and proposed as a follow-up (Q3).

## What the code read shows (base d40cd665, read only)

1. **The tree path of F-303** (`sim/sliding.rs`): `present` (the data holds `/mesh/sliding`), `check` (static, through
   `dynmesh::check`; refuses another generator than `annulus_3d_cylindrical_interface`), `new_sliding` (the blocks from
   `sim::sliding_blocks`, `SlidingInterface::cylindrical`, `mesh_at(0)` with the zone `rotor`, the 70 deg pre-flight,
   `Simulation::from_geometry`, the refusals of ADR 0637 D5, the classification D4 of the patches with rotor faces),
   `SlidingRun::step` (`mesh_at(Omega (t + dt))`, the pre-flight, `sliding_step`, `rotor_wall_conditions`, `step_sliding`), the
   monitors `sliding_angle`, `sliding_interface_flux`, `sliding_gcl`, `sliding_non_orthogonality` pushed by the step's match
   arm in `sim.rs`; `Simulation::sliding_interface_faces`.
2. **D4 would misclassify a sector's seams.** Its loop takes every patch with a rotor face; on a sector the cyclic patches
   `theta_min`, `theta_max` hold rotor faces and read `velocity = "fixed"` with value 0 by default (no `[boundaries]` entry is
   needed for a cyclic patch, `from_geometry` skips `Cyclic` patches), so they would become rotor walls. The sector path must
   skip cyclic patches (D4s below).
3. **The generators** (`sim.rs`): `annulus_3d_cylindrical_interface` builds `annulus_3d(nx, ny, nz, r_in, r_interface, lz,
   0, ga)` and `annulus_3d(nx_right, ny_right, nz_right, r_interface, r_out, lz, 0, gb)` (`cylindrical_interface_blocks`;
   grading and `isotropic_interface` through `cylindrical_interface_layer_size(r_if, 2 pi, ...)`, whose second argument is the
   blocks' angular span); `annulus_3d_sector` builds `gen::annulus_3d_sector(nx, ny, nz, r_in, r_out, lz, angle, g)` (patches
   inner, outer, zmin, zmax and the rotational cyclic pair `theta_min` / `theta_max`, every face planar, node angles `angle .
   i / n_theta`, cell index `ir + n_r (it + n_theta k)` as `annulus_3d`). `check_mesh_options` refuses `r_interface` and
   `isotropic_interface` on every generator but `annulus_3d_cylindrical_interface` and `sector_angle` on every generator but
   `annulus_3d_sector`.
4. **The enumeration** `mesh.generator` (`schema.rs` line 78) is an `enumeration`; its values appear in no golden (the MCP
   goldens `crates/rcfd-mcp/tests/golden` hold no generator name, read by search), in no hash table, and no test iterates
   them. The Studio preview (`rcfd-studio/src/preview.rs`, `count_keys`, the cell estimate) falls back to "other" / "unknown"
   for an unlisted generator.
5. **The library** (`rcfd_mesh::sliding_periodic`, `rcfd_fv::sliding_periodic`, `PimpleSolver::step_sliding_periodic`):
   `PeriodicSlidingInterface::sector(rotor, "outer", stator, "inner", origin, axis, angle, tol)` (`0 < angle < pi`),
   `mesh_turned(turned >= 0)`, `PeriodicSlidingMesh` (`report.join.interface_faces`, `report.wrap_pairs`, `image`, `turned()`,
   `is_rotor_face`, `periodicity()`), `periodic_sliding_step(old_geom, old, new, phi, u, dt)` (refuses an image change other
   than 0 or 1), `PeriodicSlidingStep { mesh_flux, reimage, gcl_residual, .. }`; `step_sliding_periodic(dt, step, wall_dpdn)`
   turns the rotor cells' velocity and history on a re-imaging step. There is no wall-condition function for the periodic
   mesh: `rcfd_fv::sliding::rotor_wall_conditions` takes a `SlidingStep` and a `SlidingMesh` (the full annulus).
6. **Coincidences.** The full annulus of the tree has `nx = 144`, `nx_right = 120` (ADR 0637 TP): rotor nodes at
   `theta + i pi/72`, stator nodes at `j pi/60`; they meet when `theta` is a multiple of `pi/360`. The sector of pitch `pi/3`
   with 24 and 20 chords has the same node angles. The tree full-annulus path is coupled at coincidences since F-710; the
   periodic step is not (ADR 0640: immune for a uniform invariant flow, a latent defect for a flow without the parts'
   symmetry; ADR 0641's coupling is on another branch). ADR 0637's step `dt = pitch / 8` lands on a coincidence at every whole
   rotor pitch.

## Design (derived; additive)

**G1. The sector rotor-stator generator** `mesh.generator = "annulus_3d_sector_cylindrical_interface"` (a new value of the
enumeration; read 4: no golden, no hash table): a rotor sector `annulus_3d_sector(nx, ny, nz, r_in, r_interface, lz, angle,
ga)` and a stator sector `annulus_3d_sector(nx_right, ny_right, nz_right, r_interface, r_out, lz, angle, gb)`, `angle =
sector_angle` in degrees, `0 < sector_angle < 180` (the periodic join's `(0, pi)`), the counts `0 = same as the rotor's`, the
gradings `ga`, `gb` as in `cylindrical_interface_blocks` (with the sector's angle passed to
`cylindrical_interface_layer_size` for `isotropic_interface`), joined at offset 0 by
`PeriodicSlidingInterface::sector(a, "outer", b, "inner", 0, +z, angle, 1e-8).mesh_at(0)`, the zone `rotor` = the rotor's
cells, ADR 0112's pre-flight (`<= 70 deg` on the direct interface faces). Without `[mesh.sliding]` this is a frozen-rotor
sector join (the seams `theta_min` / `theta_max` cyclic in both blocks, merged; the wrap patches `sliding_wrap_a` /
`sliding_wrap_b`, empty at offset 0). Both blocks share the pitch by construction (the acceptance's "the same pitch").
`check_mesh_options` allows `r_interface`, `isotropic_interface` and `sector_angle` for the new value as well (the
conditions for every other generator are unchanged). The blocks are built by one helper shared with the full-annulus
generator, which for the full annulus makes the same calls in the same order (`sector = None`).

**G2. The table** is `[mesh.sliding]` of ADR 0637 unchanged in its keys (`angular_velocity`, `rotor_walls`); it now accepts
the new generator as well. With the sector generator the run is the periodic loop (P1); with the full-annulus generator the
F-303 loop, its operations unchanged. Static checks (`rcfd validate`): the generator must be one of the two; with the sector
generator `angular_velocity >= 0` (`mesh_turned` takes turned angles `>= 0` and a step re-images forwards only, ADR 0634
limits), located at `mesh.sliding.angular_velocity`.

**P1. The loop** (ADR 0634's turned loop in the tree): before step `n + 1`, with the solver at time `t`, `turned = Omega (t +
dt)`; `new = mesh_turned(turned)` with the zone; the pre-flight on its direct interface faces (`<= 70 deg`, else a located
stop as F-303); `st = periodic_sliding_step(&s.geom, &cur, &new, &s.phi, &s.u.values, dt)`; the wall conditions (P2);
`s.step_sliding_periodic(dt, st, dpdn)`; `cur = new`; `self.geom = s.geom`. Monitors as F-303 (`sliding_angle` = the turned
angle; `sliding_interface_flux` = the flux out of the rotor through the direct faces and the rotor copies of the wrap pairs;
`sliding_gcl`; `sliding_non_orthogonality`), plus `sliding_image` (the image `k`), pushed only by the sector loop. A new
accessor `Simulation::sliding_wrap_pairs()` (`None` without the sector loop) for the tests.

**P2. Wall conditions on the rotor in the image** (new library function
`rcfd_fv::sliding_periodic::periodic_rotor_wall_conditions(u, step, old_geom, new, patches, omega_w, dt)`): as ADR 0637 D4
face by face, `u_b = (w a x (x_f - o))_t + n_f m_f / |S_f|` with `w = omega_w` on a rotor face (`new.is_rotor_face`) and `0` on
a stator face, at the face centre as the new mesh shows it (in the rotor's current image: the rigid velocity at the shown
point is the laboratory velocity at the image point turned back, which is what a periodic field holds there, ADR 0634 R1),
and `dp/dn = -n . (u_b,new - u_b,old) / dt`. On a re-imaging step (`step.reimage` set) the old boundary value of a rotor face
is in the old image; in the new image it is `R^{-1} u_b,old` (ADR 0634 R2, the rotation `step.reimage.rotation`), so the old
values of the rotor faces are turned before the difference. A translation (planar pair) is refused (not used by the tree;
Q3). On a step that does not re-image the formula is ADR 0637 D4's.

**D4s. The classification on the sector**: as ADR 0637 D4, skipping `Cyclic` patches (read 2); and a `fixed` non-zero uniform
value on a patch with rotor faces must be axial (parallel to `+z`, `abs(v_x) + abs(v_y) = 0`): a uniform vector is periodic
under the pitch rotation only if it is axial (ADR 0634 R3), and only then is it the same value in every image of the rotor;
otherwise refused, located at `boundaries.<patch>.velocity_value`. The refusals of ADR 0637 D5 apply unchanged.

**D6s. What a case without the sector asks runs.** A case without `[mesh.sliding]` and without the new generator name never
reaches the new code: the generator match gains an arm for a new name; `check_mesh_options` gains the new name in three
conditions that are false for every other generator; the F-303 loop is the same function body moved into one arm of a match
on the pair (the same calls in the same order); the shared block helper makes the same calls for `sector = None`.

## Set-ups

Mesh pair, solver and physics are ADR 0637's TP and TS except the step (below): sector **SP**: `[mesh] generator =
"annulus_3d_sector_cylindrical_interface"`, `nx = 24`, `ny = 8`, `nz = 1`, `nx_right = 20`, `ny_right = 8`, `nz_right = 1`,
`r_in = 1`, `r_interface = 1.5`, `r_out = 2`, `lz = 0.1`, `sector_angle = 60` (the first sixth of TP: node for node, read 3 and
6; 352 cells); full annulus **TP** (2112 cells). Solver TS (`piso`, central, 6 non-orthogonal correctors, Euler, the tight
tolerances) with **`dt = 0.005`**, `[physics] nu = 0.01`. Boundaries: `zmin`, `zmax` `velocity = "symmetry"` (as ADR 0637 NA;
both runs share them, so ADR 0637 Q7's finding on symmetry planes acts on both alike); `inner`, `outer` walls (fixed 0).

**The step.** `dt = 0.005` (ADR 0634 F's step, ADR 0637 U1d's) in place of ADR 0637's `pitch/8`, so that neither run lands on
a node coincidence: the periodic step's latent defect at coincidences for a flow without the parts' symmetry (ADR 0640 N1)
is not fixed in this base (F-721 is on another branch), and the m = 6 field below has no `pi/12` symmetry. Arithmetic
(arith.py): over 440 steps the turned angle comes no closer than `9.95e-6` rad to a multiple of `pi/360` (step 281), over
220 steps `2.45e-5`; the re-imaging steps are **210** and **419** (`210 dt = 1.05 > pi/3`, `419 dt = 2.095 > 2 pi/3`); 440
steps turn 2.2 rad = 2.10 pitches. Courant number about `dt / (r_in pi/72) = 0.115`, mesh Courant number `0.115`.

- **FS** (c1, c2: a non-axisymmetric periodic field): `[mesh.sliding] angular_velocity = 1, rotor_walls = "rest"`; initial
  field of the stream function `psi = C f(r) sin 6 theta`, `f = (r^2 - 1)(4 - r^2)` (lengths in m), `C = 1/12` m/s: `u_r =
  6 C f cos(6 theta) / r`, `u_theta = -C f'(r) sin 6 theta`, `f' = 2 r (5 - 2 r^2)`, written as `velocity_expression_x/y` with
  `r = sqrt(x^2 + y^2)`, `theta = atan2(y, x)`. Divergence-free (a stream function; finite-difference check `6e-10`), tangent
  to both walls (`u_r = 0` at `r = 1, 2`), periodic under the pitch rotation (`v(R x) = R v(x)`, checked `1.7e-16`), largest
  speed `12 C = 1 = U0` (at the outer wall), largest in the rotor `0.73 U0` (arith.py). Its vorticity has the azimuthal wave
  number 6 (ADR 0637 NAc's construction with `m = 6`, the smallest wave number periodic in `pi/3`). **FS-A**: the same text
  on TP (`annulus_3d_cylindrical_interface`, `nx = 144`, `nx_right = 120`); 440 steps each.
- **FW** (c2: a turning hub, the fan's case): `rotor_walls = "rotating"`, `[initial] velocity = [0, 0, 0]`; SP and **FW-A** on
  TP; 440 steps each (ADR 0637 WT with the step and length above).
- **US** (c1: mass and a uniform flow through the re-imaging steps): SP, `rotor_walls` default, `[initial] velocity = [0, 0,
  1]`, `inner`, `outer`, `zmin`, `zmax` `velocity = "fixed"`, `velocity_value = [0, 0, 1]` (axial: invariant under the pitch
  rotation); 440 steps.
- **FSL**, **FWL** (the library loop): the solver of a Simulation of FS (FW) (`pimple_solver_mut`, never stepped by the tree)
  advanced 440 steps by ADR 0634's turned loop written in the test with the library calls (`PeriodicSlidingInterface::sector`
  of the blocks `annulus_3d_sector(24, 8, 1, 1, 1.5, 0.1, pi/3)` and `(20, 8, 1, 1.5, 2, 0.1, pi/3)`, `mesh_turned` with the
  zone, `periodic_sliding_step`, `periodic_rotor_wall_conditions(.., ["inner"], 0 or 1, dt)`, `step_sliding_periodic`), turned
  angle `Omega (t + dt)`.
- **FSN**, **FWN** (negative controls): FSL (FWL) with `st.reimage = None` set after the wall conditions on the re-imaging
  steps (the rotor's vectors not re-imaged in the solver; ADR 0634 SN1 and FN in the tree's set-up).
- **CL** (the CLI): FS written to a temporary case file with `end_time = 1.1` (220 steps, one re-imaging step) and two
  reports (`volume_average` of `ux`, of `uy`), run by `rcfd run <case> --out <dir> --quiet`; the same text in process.
- **FZ** (the frozen sector generator): US without the table, 5 steps.
- **WP** (unit, `rcfd-fv`): the pair of FSL's blocks and TP's blocks in the library; a step without re-imaging from `0.0123`
  to `0.0173` rad and a re-imaging step from `pi/3 - 0.0025` to `pi/3 + 0.0025`; old boundary values on `inner` and `zmin` set
  on the old geometry to the tangential rigid velocity `(a x x_f)_t` (both pairs); the flux and velocity of the old state from
  `u = (x, -y, 0.1 z)`.

## Registered checks

All in release. "Every step" is the largest value over the steps; each such check of FS, FW and US is also printed over the
two re-imaging steps alone, with the same limit. Matching (F0): after every step every sector cell is matched to the annulus
cell whose centre is nearest (ADR 0634 F0; the sector's rotor at the reduced offset is the image of the annulus's rotor, so
the matched velocities are compared as they are). Tests: the tree checks in `crates/rcfd-api/tests/sliding_sector_case_f711.rs`,
the CLI checks in `crates/rcfd-cli/tests/sliding_sector_case_f711.rs`, the unit checks of P2 in
`crates/rcfd-fv/src/sliding_periodic.rs`.

### C. The case file (c1)

| Id | Check | Limit |
| --- | --- | --- |
| C1 (c1) | CL: `rcfd run` exits 0; its `monitors.csv` last `sliding_angle` and `sliding_image` against the in-process run, the angle `>= pi/3` and the image 1; its last `ux_mean`, `uy_mean` against the in-process run | exit 0; angle and image equal to `1e-12`; relative `<= 1e-12` (as ADR 0637 C1) |
| C2 | CL: `rcfd validate` of the case with `angular_velocity = -1` | exit 3, the message naming `mesh.sliding.angular_velocity` |
| C3 (c1, set-up) | FS: the steps where `sliding_image` increases are exactly `{210, 419}`; final `sliding_angle` `>= 2 pi/3` | as stated |

### K. The generator and the keys (c1, c3)

| Id | Check | Limit |
| --- | --- | --- |
| K1 (c1) | FZ's generator mesh: 352 cells, zone `rotor` = cells 0 to 191; patches `inner` 24, `outer` 20, `zmin` 352, `zmax` 352, `theta_min` 16, `theta_max` 16 (cyclic, rotation 60 deg), `sliding_wrap_a` 0, `sliding_wrap_b` 0 faces; 40 direct interface faces and no wrap pair (`24 + 20 - 4`: at offset 0 the nodes meet at 0 = `pi/3` modulo the pitch, `pi/12`, `pi/6`, `pi/4`, read 6; the full annulus at angle 0 has `240 = 6 . 40`, ADR 0637 amendment 2); the pre-flight `<= 70 deg` | as stated |
| K2 | FZ runs 5 steps: velocity and pressure measures as U1s | `<= 1e-10` each |
| L0s | the sector Simulation's mesh at angle 0 against FZ's generator mesh | `content_hash` equal |
| K0s | FS without the table: the data has no `/mesh/sliding`, `to_toml` has no `sliding` (as ADR 0637 K0) | as stated |
| K3 | unchanged refusals: `sector_angle = 40` on `annulus_3d_cylindrical_interface` and `r_interface = 0.7` on `annulus_3d_sector` | `Err` each, located at the key (as before) |
| D1 (c3) | `cargo test -p rcfd-api --release --test baseline_hashes` (shared files `sim.rs`, `sim/sliding.rs`, `schema.rs`) | pass (unedited) |
| D2 (c3) | unedited suites: `rcfd-api` `sliding_case_f303` (L1, L2: the F-303 tree loop equals ADR 0216's library loop bit for bit, so the full-annulus path is unchanged), `sliding_coincidence_f710`, `case_hashes_w48u`, `cylindrical_interface_w44b`, `revolved_sector_w45b`, `mesh_transform_w48u`, `rules_w19c`, `dynamic_mesh`, the `rcfd-api` library tests; `rcfd-fv` `sliding_periodic_f304`, `sliding_reimaging_f704`, the `rcfd-fv` library tests; `rcfd-cli` `sliding_case_f303`, `docs_site` | pass |

### L. The tree loop is the library loop

| Id | Check | Limit |
| --- | --- | --- |
| L1s | FS against FSL after 440 steps: `max_c abs(u_tree - u_lib) / U0`, `max_f abs(phi_tree - phi_lib) / (U0 max abs(S_f))` | `<= 1e-12` each |
| L2s | FW against FWL after 440 steps, as L1s | `<= 1e-12` each |

### F. The sector against the full annulus (c2)

| Id | Check | Limit |
| --- | --- | --- |
| F0 (set-up) | FS / FS-A and FW / FW-A, every step: every sector cell's nearest annulus cell centre within `1e-9`, the match one-to-one | as stated |
| F1 (c2) | FS against FS-A, every step `max_c abs(u_sector - u_annulus) / U0` | `<= 2e-2` |
| F2 | FS, last step, the volume-weighted rms of the same difference | `<= 1e-2` |
| F3 (c2) | FW against FW-A, every step `max_c abs(u_sector - u_annulus) / (Omega r_in)` | `<= 2e-2` |
| F4 | FW, last step, the rms | `<= 1e-2` |
| FN1 (negative control of F1) | FSN against FS-A, F1's measure | `>= 2e-2` (F1's limit) and `>= 5` times F1's measured value |
| FN2 (negative control of F3) | FWN against FW-A, F3's measure | `>= 2e-2` and `>= 5` times F3's measured value |
| X1 (set-up) | every step: the sector's direct interface faces plus wrap pairs number 44 (`24 + 20`, no coincidence), the annulus's interface faces 264 (`144 + 120`); the sector has at least one wrap pair on every step | as stated |
| X0 (set-up) | FS, FS-A, FW, FW-A, US: every step `sliding_non_orthogonality` | `<= 70 deg` |
| M1 | FS (sector), every step `abs(sliding_interface_flux) / (U0 A_I)` (`A_I` the direct and rotor-copy areas), `sliding_gcl`, `continuity` | `<= 1e-10`, `<= 1e-12`, `<= 1e-9` (ADR 0637 M1) |

Estimates. F1, F3: ADR 0634 F1 measured `8.0e-3 U_in` between the same sector and full-annulus pair in the library over 640
steps of this `dt`, with the radial source flow; the difference comes from the wrap pairs, coupled with the orthogonal
pressure coefficient where the annulus has internal faces with the non-orthogonal correction (ADR 0629 Q4), in proportion to
the pressure gradient across the seam. FS's pressure scale is `U0^2` (as the source flow's `dp_pot = 0.375 U_in^2`) and FW's
the centrifugal `(Omega r_in)^2`; the same limit `2e-2` leaves a factor 2.5 over ADR 0634's measured value. F2, F4: the
difference sits near the seams and the interface (ADR 0634 F2: `5.0e-3` measured against `1e-2`). FN1: the rotor vectors left
in the old image are turned by `+60 deg` against the field, `abs(R - I) = 1`; the rotor-region speed of FS is `0.73 U0` at the
start, `0.76` of it left by the slowest mode at the first re-imaging step (`t = 1.05`), and with the wall-slip loss that ADR
0637 amendment 2 measured (`0.333 / 0.842`) taken as a pessimistic factor about `0.22 U0` (arith.py), a factor 11 over
`2e-2`. FN2: the hub layer `sqrt(nu t) = 0.10` at `t = 1.05` (1.6 radial cells) carries swirl of order `Omega r_in` in the
rotor's first rows of cells; turned by 60 deg, an error of that order (`>= 0.3`), far above `2e-2`. X1 follows from read 6
and the arithmetic of the step (no coincidence). M1 as ADR 0637 (the inner chords of the sector sweep no net volume: a curve
turned about the axis sweeps `delta (|x_end|^2 - |x_start|^2) / 2`, zero for ends on one circle; the seams' fluxes cancel in
pairs).

### U. Uniform axial flow through the re-imaging steps (c1)

| Id | Check | Limit |
| --- | --- | --- |
| U1s (c1) | US, after every step `max_c abs(u_c - U) / abs(U)` and `max_c abs(p_c - pbar) / abs(U)^2` (`pbar` the volume mean) | `<= 1e-10` each (ADR 0634 S9 measured `6.2e-15` on S3 in the library) |
| U2s | US, every step `sliding_gcl`, `continuity`, `abs(sliding_interface_flux) / (abs(U) A_I)` | `<= 1e-12`, `<= 1e-9`, `<= 1e-10` |

### W. The wall conditions in the image (unit, `rcfd-fv`)

| Id | Check | Limit |
| --- | --- | --- |
| W1p | WP, the step without re-imaging, `periodic_rotor_wall_conditions` on `["inner", "zmin"]`, `omega_w = 1`, against `rcfd_fv::sliding::rotor_wall_conditions` on the full-annulus step at the same angles, face matched to the annulus face of the nearest centre (`<= 1e-9`): every face value and `dpdn` | values `<= 1e-12` absolute; `dpdn` `<= 1e-9` absolute (`1e-12 / dt` scaled by 5) |
| W2p | WP, the re-imaging step (sector turned `pi/3 - 0.0025` to `pi/3 + 0.0025`, image 0 to 1), against the annulus step at the same turned angles, the sector face matched to the annulus face at its centre turned by `+pi/3`: the sector value against `R^{-1}` of the annulus value, `dpdn` against the annulus `dpdn` | as W1p |
| W3p | WP, both steps: every stator face of `zmin` gets `[0, 0, 0]` | exact |
| W4p | the function with a mesh that is not the step's (another face count) and with a planar pair | `Err` each |

### E. Refusals

| Id | Check | Limit |
| --- | --- | --- |
| E1s | `Simulation::new` of FS with, one at a time: `angular_velocity = -1`; `sector_angle = 200`; `sector_angle` absent; `[boundaries.inner] velocity_value = [1, 0, 0]` (non-axial on a rotor patch); `generator = "annulus_3d_sector"`; `turbulence.model = "k_epsilon"` | `Err` each, the message naming the key or `mesh.sliding` |
| E2s | FS: `supports_restart()` | false |

### Mutations (scratch copy, not committed)

Each applied alone to the final tree by an exact text replacement on a `git archive` copy, the named tests run once; each
must fail.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | the tree's sector loop keeps the old image (`new.image = cur.image` before the step: no re-imaging) | F1 |
| MU2 | `periodic_rotor_wall_conditions` does not turn the old values of the rotor faces on a re-imaging step | W2p |
| MU3 | `periodic_rotor_wall_conditions` gives every face the rotor's wall speed | W1p or W3p |
| MU4 | the tree's sector loop does not keep the new mesh (`cur` not replaced) | L1s or C1 (a refused or failed step counts) |
| MU5 | the sector generator builds the stator block with the rotor's count `nx` in place of `nx_right` | K1 |

### Decision rule

**Done**: C1, C3 (c1), K1, K0s, L0s (c1), F1 and F3 (c2) with F0 held, D1 and D2 (c3), L1s, U1s, M1, W1p, W2p met; the set-up
checks X0, X1 held; the negative controls FN1 and FN2 held; every mutation caught; no golden or hash table edited. A miss of
an acceptance check, or a control or mutation that does not show its check can fail: **partial** with the miss named. The
tree cannot run the sector case past one pitch: **todo**. Every other check is supporting and reported.

## Limits (not covered)

The planar slider in the tree (no periodic channel generator; Q3); a sector about `z` through the origin only, pitch below
180 deg, both blocks of one pitch (the generator makes them so); forward rotation (`angular_velocity >= 0`, ADR 0634 Q6);
steps that land on node coincidences on the periodic path (the latent defect of ADR 0640 N1 for flows without the parts'
symmetry; F-721's coupling is not in this base; the set-ups avoid coincidences, X1); the non-orthogonal correction on the
wrap pairs (ADR 0629 Q4; measured through F only) and the pre-flight on the wrap faces (the measure of ADR 0112 is of internal
faces; the wrap pieces are the same polygons as the annulus's direct faces at the seam); non-axial uniform values on rotor
patches; everything F-303 refuses (D5) and its limits (no expression boundary values, no restart, the compressible solver,
parallel runs); the Studio preview counts the new generator's cells as "unknown".

## Open questions taken on the standing instruction

- **Q1. How the sector reaches the tree.** (a) A new generator value `annulus_3d_sector_cylindrical_interface` and the
  existing table `[mesh.sliding]` (taken: the acceptance names a sector rotor-stator generator; the frozen sector join is
  useful on its own; the table's keys mean the same; read 4: no golden or hash table holds the enumeration); (b)
  `sector_angle` on `annulus_3d_cylindrical_interface` (overloads a generator documented as a full annulus); (c) a sector key in
  `[mesh.sliding]` (the frozen generator would then have no sector). Taken (a) on the maintainer's standing instruction of
  2026-11-11 (take the recommendation).
- **Q2. The step of the comparison.** (a) `dt = 0.005`, no coincidence in either run (taken: the periodic step's coincidence
  coupling is not in this base, F-721); (b) ADR 0637's `dt = pitch/8` (lands on coincidences every rotor pitch on both runs).
  Taken (a) on the standing instruction.
- **Q3. Planar sliding in the tree.** (a) A follow-up item (a periodic channel rotor-stator generator, the planar wall
  conditions, a planar check against a closed form) (taken); (b) in F-711. Taken (a) on the standing instruction; proposed in
  the roadmap entries.
