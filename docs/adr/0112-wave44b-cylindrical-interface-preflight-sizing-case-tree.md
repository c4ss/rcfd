# ADR 0112: Wave 44B, cylindrical interface: non-orthogonality pre-flight, near-isotropic interface layer, case-tree wiring (F-126)

Status: registered before any code of this wave was written. Limits and decision rules below are not changed after a run; a missed
limit becomes an ignored `*_known_miss` test with the limit intact; a defective set-up gets a new corrected test registered here as an
amendment before it runs. Written at HEAD 2f1a3ce (worktree `<scratch dir>`, branch `w44b`, target
`<target dir>`). Parent: ADR 0109 (wave 43B). Clean-room: first principles (polygon geometry, exact solutions of the
Navier-Stokes equations) and the measured values of ADR 0109; no literature value enters a limit.

## Context

ADR 0109 delivered `rcfd_mesh::non_conformal::join_non_conformal_cylindrical` and found:

* The rcfd-api steady solver (SIMPLEC, `linear_upwind`, one non-orthogonal corrector) diverged on every full-annulus mesh whose overlap
  faces were strongly non-orthogonal: 79.2 deg (registered mesh M, default options; also with `velocity_correction = "face_flux"`, and
  with face_flux plus 3 correctors), 75.7 deg (axial counts matched), 72.7 deg (counts around matched). It converged at 0.0 deg (identical
  blocks) and at 48.7 deg (mesh K2, radial cells growing towards the interface, with `face_flux`; the default correction was not tried on
  K2). The 60-degree sector (rcfd-fv, central convection, alpha_u 0.8, one axial layer) converged on its uniform G2 mesh; its interface
  angle was not measured in ADR 0109.
* No case-tree key exists for the cylindrical join (library function plus a mesh file only).
* The sum of absolute interface fluxes misses its 1e-3 limit and halves when the counts around are doubled (first order).

## Decision

Three opt-in additions; no default, no existing generator and no existing key changes behaviour.

1. **Pre-flight check (library + case level).** `rcfd_mesh::non_conformal::interface_non_orthogonality(mesh, faces, limit_deg)`
   returns a serialisable `InterfaceOrthogonality` for the given interface faces (normally `JoinReport::interface_faces`): the worst
   angle, its face, owner, neighbour and face centre, the normal and tangential parts `d_n`, `d_t` of the owner-to-neighbour centre
   vector `d` at that face, the limit, the needed normal spacing `d_t / tan(limit)`, the factor `needed / d_n` by which the spacing
   across the interface must grow (equivalently the cell sizes around/axially shrink), a flag `exceeds` and a flag `verified` (worst
   angle at or below the largest angle with a converged solver run, 48.7 deg). The angle is the `quality.rs` definition (angle between
   `S_f` and `d`). Its `message()` names the cause (cells thin radially compared with their size around and axially, so the centres on
   the two sides are offset along the interface) and the factor.
2. **Interface-layer sizing helper (opt-in).** `rcfd_mesh::non_conformal::cylindrical_interface_layer_size(radius, sector_angle,
   n_around: [usize; 2], axial_length, n_axial: [usize; 2], max_deg)` returns the radial size `h` of the interface-adjacent cells on both
   sides for which the geometric bound of the overlap-face angle is `max_deg`: the centre offset along the interface is below
   `t = (p_a + p_b)/2` around (`p = radius * sector_angle / n`; `t = 0` when the counts are equal) and below `z = (dz_a + dz_b)/2`
   axially (`dz = axial_length / n_z`; `z = 0` when the axial counts are equal, i.e. the blocks are layered uniformly over the same
   axial range as all generators here do), the centre distance across is about `(h_a + h_b)/2 = h`, so `h = hypot(t, z) / tan(max_deg)`.
   The generator option uses `max_deg = 45`: the radial size equals the bound of the in-surface offset, i.e. the interface cells are
   near-isotropic, and the bound stays below the 48.7 deg verified angle. `rcfd_mesh::gen::expansion_for_end_cell(n, len, h)` returns
   the `Grading::Expansion` ratio (last/first) whose last cell is `h` (bisection; the stator uses the reciprocal so its first cell is
   `h`); `h >= len` with `n > 1`, or `h > len` with `n = 1`, is an error.
3. **Case tree.** New generator `mesh.generator = "annulus_3d_cylindrical_interface"`: a rotor block `r_in <= r <= r_interface`
   (`nx` around, `ny` radial, `nz` axial; cell zone `rotor`) and a stator block `r_interface <= r <= r_out` (`nx_right`, `ny_right`,
   `nz_right`, 0 = same as the rotor block), both of height `lz` from z = 0, joined by `join_non_conformal_cylindrical` (tol 1e-8);
   patches `inner`, `outer`, `zmin`, `zmax`. `mesh.grading` = size of the interface-adjacent cell over the wall-adjacent cell in both
   blocks (rotor `Expansion(g)`, stator `Expansion(1/g)`; 1 = uniform). New keys: `mesh.r_interface` (default 0, must lie strictly
   between `r_in` and `r_out` for this generator) and `mesh.isotropic_interface` (default false; true grades both blocks with item 2 at
   45 deg and conflicts with `mesh.grading != 1`). After the join the pre-flight runs; above the limit `Simulation::new` fails with a
   located diagnostic at `mesh.r_interface` (there is no non-fatal warning channel at mesh build, so only the error tier exists at case
   level; the conservative default is to stop before a run that the ADR 0109 evidence says diverges). The new keys set on another
   generator are located errors. Not in `COARSENABLE` (the coarse pre-solve rejects this generator with its existing message).

### Registered limit of the pre-flight

`CYLINDRICAL_INTERFACE_MAX_NON_ORTHOGONALITY_DEG = 70`: the smallest angle at which a run diverged in ADR 0109 (72.7 deg) rounded down to
the next multiple of 10 below it; the band 48.7-70 deg is untested and is reported as not verified (`verified = false`) but not
rejected. The evidence comes from the rcfd-api 3-D SIMPLEC set-up of ADR 0109; the rcfd-fv sector set-up is a different solver
configuration and its convergence is not part of the derivation.

## Registered checks

Common: ADR 0109 geometry (`r_i = 0.5`, `R = 0.75`, `r_o = 1.0`, z axis, `lz = 0.4` full annulus, sector 60 deg depth 0.1),
`nu = 0.1`, `V_i = 0.5`. Meshes: M = `annulus_3d(48, 8, 2, 0.5, 0.75, 0.4)` + `annulus_3d(35, 8, 3, 0.75, 1.0, 0.4)`; M_z (axial counts
matched) = 48x8x2 + 35x8x2; M_t (counts around matched) = 48x8x2 + 48x8x3; K2 = `annulus_3d(96, 6, 4, ..., Expansion(3))` +
`annulus_3d(70, 6, 6, ..., Expansion(1/3))`; I = identical blocks 48x8x2 + 48x8x2; G2 = `annulus_sector(8, 8, 0.5, 0.75, pi/3)` +
`annulus_sector(6, 8, 0.75, 1.0, pi/3)`.

### P: pre-flight (rcfd-mesh unit tests, `non_conformal.rs`)

* **P1 consistency.** On M, K2 and G2, `worst_deg` equals the maximum of `quality::compute_metrics(m).non_orthogonality` over
  `rep.interface_faces` within 1e-9 deg, and `face` is a face where that maximum is attained.
* **P2 the ADR 0109 evidence.** `worst_deg` within 0.5 deg of the ADR 0109 `check_mesh` values: M 79.2, M_z 75.7, M_t 72.7, K2 48.7, I 0.0;
  `exceeds` true on M, M_z, M_t and false on K2, I; `verified` true on K2 and I; on M, `factor` = `tan(worst)/tan(70 deg)` within
  1e-12 relative and `message()` contains `non-orthogonal` and `factor`. G2 is measured and reported, no limit.
* **P3 G0.** The existing G0 planar-join hash test passes unchanged (the join code is not modified).

### H: interface-layer sizing (rcfd-mesh unit tests and rcfd-fv)

* **H1 mesh level.** For the counts of G2 (8/6 around, 60 deg, one layer each), K2 (96/70 around, 4/6 axial) and M (48/35, 2/3) with
  their radial counts (8/8, 6/6, 8/8), blocks graded by `expansion_for_end_cell` with `h` of the helper at 45 deg: the interface-adjacent
  cell radial size on each side equals `h` within 1e-12 relative (from the generated ring radii), and the joined mesh's `worst_deg` is
  at most 45 deg (the design bound).
* **H2 the ADR 0109 sector tests on the sized sector** (rcfd-fv, `tests/cylindrical_interface_w44b.rs`, template and solver of ADR
  0109 S1/S2): sized G2 (rotor 8 around x 8 radial graded to `h`, stator 6 x 8 graded from `h`). Controls: cylindrical joins of
  identical blocks with the same two radial distributions, 8/8 and 6/6 around (as K2 in ADR 0109). Limits, each for S1 (inertial) and
  S2 (rotating frame `Omega_f = W_i`): converged (`res_u < 1e-10`, continuity < 1e-9 within 4000 iterations); `max |u_theta,abs -
  analytic| <= 1.5 x max(control errors)`; `|net interface flux| <= 1e-8 V_i A_if`; seam balance `<= 1e-8 V_i A_seam`. The sum of
  absolute interface fluxes and the interface angle of the uniform and the sized G2 are reported, not criteria (the absolute flux is
  first order in the mismatch around, which this sizing does not change).

### T: case tree (rcfd-api, `tests/cylindrical_interface_w44b.rs`)

* **T1 mesh identity.** `build_mesh` of the tree case for K2 (`nx 96, ny 6, nz 4, nx_right 70, ny_right 6, nz_right 6, r_in 0.5,
  r_interface 0.75, r_out 1.0, lz 0.4, grading 3`) has the same `content_hash` as the library join of the K2 blocks with cell zone
  `rotor` = rotor cells. Exact.
* **T2 run equality.** ADR 0109 C2 (Taylor-Couette, MRF `Omega = W_i` on `rotor`, `face_flux`, 4000 iterations at most) through the
  tree and through the library mesh written with `rcfd_io::openfoam::write_dir` and read with `mesh.source = "file"`: same iteration
  count and `max |difference|` of `ux, uy, uz, p` `<= 1e-12` (absolute; `V_i = 0.5`, pressures of order 0.1).
* **T3 located errors** (path and substring): `r_interface` = 0 or >= `r_out` -> `mesh.r_interface`, `between`; `isotropic_interface`
  with `grading = 3` -> `mesh.grading`; `r_interface` or `isotropic_interface` on `annulus_3d` -> that path, `does not use`; the tree
  case with the counts of M (uniform) -> `mesh.r_interface`, `non-orthogonal` and `factor`; `isotropic_interface` with `ny = 1` and a
  rotor block thinner than `h` -> `mesh.isotropic_interface`.
* **T4 bit identity without the keys.** An `annulus_3d_interface` MRF case (24 x 4 x 2 lower, ny_right 3, `zone = "all"`, 30
  iterations) hashed with FNV-1a over the bits of `ux, uy, uz, p` and the face fluxes equals the value recorded at HEAD 2f1a3ce before any
  code change; `crates/rcfd-api/tests/baseline_hashes.rs` passes unchanged.
* **T5 the option in the full-annulus solver.** The tree case with the counts of K2 (96/70, 6/6 radial, 4/6 axial) and
  `isotropic_interface = true`: the pre-flight passes; C2 converges (`is_converged`, 4000 iterations at most, `face_flux`) and
  `max |u_theta - analytic| <= 2e-2 V_i` (the ADR 0109 M4 limit).

### Status decision rule

F-126 stays partial (its acceptance is a published compressor case, not attempted). Its note is extended. Defaults, existing keys and
`baseline_hashes.rs` unchanged.

## Amendment 1 (written after the first run of the mesh-level tests, before their re-run)

What I knew when writing this: the first run of P1/P2 and H1. H1 passed as registered (values in the addendum). P2 failed on one
assertion: K2 measures 48.745 deg, so `verified` (angle <= `CYLINDRICAL_INTERFACE_VERIFIED_DEG` = 48.7) is false on K2, while P2 requires
it true. The constant is defined above as "the largest angle with a converged solver run"; 48.7 was ADR 0109's value rounded to one
decimal, below the angle it stands for. Correction of the constant (not of a test limit): `CYLINDRICAL_INTERFACE_VERIFIED_DEG = 48.75`
(K2's 48.745 rounded up at the second decimal). P2 is re-run unchanged; all other values stay as registered. The failed first run is
recorded as a deviation.

## Smoke of the H2 set-up (rule d), before the registered H2 run

Construction plus 100 iterations (ignored test `smoke_w44b_sized_sector`): `h` = 0.11454, rotor `Expansion(69.63)`, stator
`Expansion(1/69.63)` (wall-adjacent cells 1.6e-3). All six runs (8/6, 8/8, 6/6; inertial and rotating frame) reached `res_u` about 1e-10
within 82-101 iterations, so this smoke already shows values close to the final ones (8/6 inertial error 1.353e-2 V_i, controls 9.87e-3
and 1.307e-2). Not defective; no limit changes. The registered H2 run follows.
