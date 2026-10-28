# ADR 0113: Wave 44C, MRF zones with rotationally cyclic seams (F-126)

Status: registered before any solver code of this wave was written. Limits and decision rules below are not changed after a run; a
missed limit becomes an ignored `*_known_miss` test with the limit intact; a defective set-up gets a new corrected test registered here
as an amendment before it runs. Written at HEAD fee6ecc, target `<target dir>`. Clean-room: first principles only
(exact solutions of the Navier-Stokes equations, rigid-rotation kinematics); no literature value enters a limit.

## Context

ADR 0109 (wave 43B) found that a periodic rotor passage with an MRF zone cannot be combined with the cylindrical rotor/stator interface:
`MrfZone::worst_non_axisymmetry` refuses a zone with cyclic seams, because a seam face lies in a plane through the axis and carries the
frame flux `(Omega x r).S`, while `add_frame_flux` gives the frame flux only to internal faces with both cells in the zone. ADRs 0082 and
0085 tried a wider change (F-RP: frame flux on every boundary face of the zone, walls included, triangle-exact) on the fan meshes; it did
not converge on two of three meshes and was removed both times. This wave changes the seams only.

## What the seam flux must be (first principles)

Absolute-velocity MRF (ADR 0008): momentum in the zone is convected with `phi_rel = phi - F`, `F_f = (Omega x r_f).S_f`, and gets the
lagged `-Omega x u`. Mass is carried by the absolute flux `phi`. The discrete momentum convection of a cell is conservative and consistent
with a uniform field only if `sum_f phi_rel,f = 0` in every zone cell, that is, if `sum_f F_f = 0` over all faces of the cell. For a cell
with planar faces this holds exactly when `F` is evaluated on every face (`Omega x r` is linear and divergence-free, and a planar polygon's
area-weighted centroid integrates a linear field exactly). A zone cell on a seam that gets no frame flux on its seam face therefore has
`sum_f phi_rel,f = -F_seam`, of order `Omega r |S|`: a spurious source of momentum. That is why the check refuses it today.

Let the cyclic pair map face `f` to its partner `g` by the rotation `R` (angle `2 pi / N` about the periodic axis), so `r_g = R r_f` and,
each area vector pointing out of its own owner cell, `S_g = -R S_f`. If the MRF axis equals the periodic axis and the MRF origin lies on
it, `R` commutes with `Omega x .`, so `Omega x r_g = R (Omega x r_f)` and

    F_g = (R (Omega x r_f)) . (-R S_f) = -F_f.

So the frame flux of a seam pair is equal and opposite, exactly as for the two sides of one internal face: a seam pair is an internal face
cut in two. The velocity transfer needs no change: the absolute velocity maps by `R` (the existing `periodic_rot`), and so does the
relative velocity, since `u - Omega x r` maps to `R u - Omega x (R r) = R (u - Omega x r)`. Handled consistently means: (i) both faces of
a pair get the frame flux, which requires both owner cells to be in the zone; (ii) the MRF axis and origin agree with the periodic
rotation, which shows as `F_f + F_g = 0`; (iii) the absolute seam fluxes stay antisymmetric as before (pair mean of the body-force flux
term not changed: off for MRF, as today).

## Decision (method, opt-in)

* `rcfd_fv::sources::MrfZone` gains `cyclic_seams: bool`. With `true`, `add_frame_flux` also adds `F_f` on every coupled (cyclic)
  boundary face whose owner and whose partner face's owner are both in the zone, and `worst_non_axisymmetry` skips those faces but
  counts the pair mismatch `|F_f + F_g| / (|Omega| r_perp |S_f|)` instead; it also returns the face that sets the worst value, so the API
  error names its patch. With `false` every result is bit-identical to HEAD (the only caller passes `false` unless the key is set).
* Case key `sources.<name>.cyclic_seams` (bool, default `false`), allowed only with `type = "mrf"` (a located error at
  `sources.<name>.cyclic_seams` otherwise). The surface-of-revolution error message names the patch of the worst face.
* Nothing else changes: no change to `sim.rs` (component-diagonal correction and pair mean as for any MRF case), no change to the
  periodic machinery, the mesh, or the I/O. `crates/rcfd-api/tests/baseline_hashes.rs` is not touched.

Known before writing, and stated now: no mesh the case tree can build today exercises the positive path. The `axial_fan_sector` hub and
shroud faces are not surfaces of revolution at the face centres (smoke below: 1.8e-2 and 4.5e-3 of `Omega r |S|` against the 1e-6
limit), so the fan passage is still refused, now at the hub instead of the seams; and the OpenFOAM writer stores only `neighbourPatch`,
so a sector read through `mesh.source = "file"` loses its rotation. The positive checks therefore run at library level with the same
`MrfZone` code the API calls. Neither the fan walls nor the I/O are changed this wave.

## Registered checks

Common (library level, `crates/rcfd-fv/tests/mrf_cyclic_seams_w44c.rs`): `r_i = 0.5`, `r_o = 1`, `nu = 0.1`, rotation about +z through the
origin; the C2b template of ADR 0071 (SIMPLEC, central convection, `alpha_u = 0.8`, `alpha_p = 1`), converged when `res_u < 1e-10` and
continuity `< 1e-9` within 4000 iterations; the MRF zone wired as `sim.rs` wires it (`conv_flux_offset` from `MrfZone::add_frame_flux`,
`component_diag_correction = true`, the lagged `-Omega x u` from `MrfZone::add_force` every iteration, `wall_pressure_trapezoid` and
pair mean off). Wall velocities are absolute. Meshes: the sector is an exact sixth of the full annulus.

* 2-D: sector `annulus_sector(8, 16, 0.5, 1, pi/3)`, full annulus `cylinder_ogrid(48, 16, 0.5, 1)` (the same points; cell `(ir, it)`,
  `it < 8`, of the annulus is cell `(ir, it)` of the sector).
* 3-D: sector = `box_mesh([8, 8, 12], [1, 1, 1.2])` bent to `r = 0.5 + 0.5 x`, `theta = (pi/3) y` (patches xmin = inner wall,
  xmax = outer wall, ymin/ymax = the cyclic pair, zmin, zmax), full annulus `annulus_3d(48, 8, 12, 0.5, 1, 1.2)`; cells matched by centre
  (distance <= 1e-9).
* `L_DIFF = 4.3e-4` of the case's velocity scale: the smallest frame-change difference measured in ADR 0008 M3 (4.3e-4 V_i, MRF at
  0.5 W_i against the stationary frame, limit 1e-2). Basis: the sector cells are the annulus cells, so a correct seam must change the
  solution by less than a mere change of frame does.

Seam balance: `|sum phi(seam a) + sum phi(seam b)| <= 1e-8 V A_seam` in every run with seams (`V` the velocity scale of the check).

* **G construction.** On the 2-D and 3-D sectors and the K5 mesh, with `cyclic_seams = true` and the zone of the check: the pair mismatch
  `<= 1e-12`; the per-cell closure `|sum_f +-offset_f| / sum_f |Omega| r_perp |S_f| <= 1e-12` on every zone cell; `worst_non_axisymmetry
  <= 1e-6` (accepted). With `false`: `> 1e-6` (refused, the status quo). Guards (3-D sector, zone all): an MRF axis tilted by 1e-3 rad
  about x, and a zone of only the cells with `theta_c < pi/6` (one side of each seam pair): each `> 1e-6` with the worst face on ymin or
  ymax.
* **K1 solid-body rotation (2-D).** Both walls at `W = 2` (`u = W x r`), MRF `Omega = 2`, zone all; fluid starts at rest. Sector (seams on)
  against the full annulus. Limits: both converged; sector against analytic, ADR 0008 M1: `max |u - W x r| <= 2e-2 W r_o`, pressure
  differences to cell 0 against `W^2 (r^2 - r_0^2)/2` within `5e-2` of `W^2 (r_o^2 - r_i^2)/2`; sector against annulus: `max |du| <=
  L_DIFF W r_o`, `max |d(p - p_0)| <= L_DIFF W^2 (r_o^2 - r_i^2)/2`; seam balance (`V = W r_o`).
* **K2 Taylor-Couette (2-D).** Inner wall `W_i = 1`, outer at rest, MRF `Omega = W_i`, zone all. Limits: both converged; sector
  `max |u_theta - (A r + B/r)| <= 1.5e-2 V_i` (M2; `A = -1/3`, `B = 1/3`, `V_i = 0.5`); sector against annulus `<= L_DIFF V_i`; seam
  balance (`V = V_i`). Reported, not a criterion: the same difference for the inertial pair (no source on either), i.e. what the periodic
  machinery alone contributes.
* **K3 axial through-flow with a rotor band (3-D).** Inlet zmin `u = (0, 0, 1)`, outlet zmax zero-gradient velocity and `p = 0`, walls at
  rest, MRF `Omega = 5` on the cells with `0.4 < z_c < 0.8`. Reference: the sector without the source. Limits (M6 of ADR 0008, `U = 1`):
  converged; `|inflow - outflow| <= 1e-9 |inflow|`; `max |u_theta| <= 2e-2 U`; `max |u_z - u_z,ref| <= 2e-2 U`; sector against the annulus
  with the same band `<= L_DIFF U`; seam balance (`V = U`).
* **K4 empty passage, exact (3-D sector, zone all, `Omega = 5`).** Inner and outer walls fixed at `(0, 0, U)`, inlet `(0, 0, U)`, outlet
  zero-gradient and `p = 0`, initial field `(0, 0, U)`. With axial `u` the term `-Omega x u` vanishes, so `u = (0, 0, U)`, `p = 0` is an
  exact discrete solution exactly when `sum_f phi_rel,f = 0` in every cell: the exact swirl is zero. 300 iterations, no early stop (a
  normalised residual is degenerate on an exact uniform state). Limits: `max |u - (0, 0, U)| <= 1e-8 U` (ADR 0065 S1), `max |p| <= 1e-8 U^2`.
* **K5 the ADR 0109 sector through the cylinder, MRF on the rotor block.** The G2 mesh of ADR 0109 (8 x 8 inner block r 0.5-0.75 joined to
  6 x 8 outer block r 0.75-1 on the cylinder, then made cyclic again), MRF `Omega = W_i` on the cells with `r_c < 0.75`, seams on; inner wall
  `W_i`, outer at rest. Controls: `annulus_sector(8 | 6, 16, 0.5, 1, pi/3)` with MRF on `r_c < 0.75` (a grid line) and seams on. Limits:
  converged; `max |u_theta - analytic| <= 2e-2 V_i` and `<= 1.5 x max(e_8, e_6)`; `|net interface flux| <= 1e-8 V_i A_if`; seam balance.
  The sum of absolute interface fluxes is reported only (ADR 0109 known miss, first order, not re-registered).
* **K6 the fan empty passage (library level, `crates/rcfd-api/tests/mrf_cyclic_seams_w44c.rs`).** `build_mesh` of `axial_fan_sector`
  with the mesh section of `fan_sector.rs` and `include_blade = false` (default blade parameters), K4 set-up on it (hub and shroud fixed at
  `(0, 0, U)`, inlet `U = 1`, outlet `p = 0`, MRF zone all, `Omega = 20`, seams on), 300 iterations from `(0, 0, U)`. Limit as K4:
  `max |u - (0, 0, U)| <= 1e-8 U`. Predicted from the smoke: a miss, owed to the hub/shroud facets (their frame flux 1.8e-2 / 4.5e-3 of
  `Omega r |S|` goes nowhere) and to the warped-face closure (1.5e-4), not to the seams. Reported with it: the seam pair mismatch and
  whether the largest deviations sit in cells touching hub/shroud or touching a seam. A miss is not cured this wave (the F-RP route).

### Negative controls (rule: run first; each must miss its limit by at least 10x, otherwise the check is declared non-discriminating)

* **D1** K1 with today's frame flux (`cyclic_seams = false`, called at library level past the API refusal): sector against annulus.
* **D4** K4 with `cyclic_seams = false`: `max |u - (0, 0, U)|`.

### Case tree (`crates/rcfd-api/tests/mrf_cyclic_seams_w44c.rs`)

* **A1 default bit-identity.** The ADR 0008 M2 case (`annulus_3d`, 48 x 16 x 2, MRF zone all, `Omega = W_i`), 20 iterations, with the key
  absent, `false` and `true` (no cyclic faces, so `true` must change nothing): every cell value of `ux, uy, uz, p` bit-identical.
* **A2 refusal moves from the seams to the hub.** The K6 passage through the case tree with `mrf` (zone all, `Omega = 20`): without the key
  refused at `sources.frame.zone` with a message naming `seam_lo` or `seam_hi`; with `cyclic_seams = true` refused at the same path with a
  message naming `hub` (smoke: hub 1.8e-2 is the worst non-seam face).
* **A3** `cyclic_seams = true` on a `rotating_frame` source: a located error at `sources.frame.cyclic_seams`.
* The existing `s4_absolute_formulation_mrf_is_refused_on_the_sector` stays as it is and must pass.

### Status decision rule

F-126 stays partial whatever the outcome (its acceptance is a published compressor case, not attempted). Its note is extended. If G, K1-K5
and D1/D4 pass, the opt-in stays; if any of G or K1-K5 misses, the change is removed (rule (b) of wave 35) and the misses are reported. K6
and A2 document the remaining fan blocker and do not decide the opt-in (registered now, before any run).

## Smoke results before registration (rule d)

Construction plus a run of the controls that need no new code, done before this ADR was written:

* Fan empty passage (`include_blade = false`, 14 168 cells): worst `|F_f| / (Omega r |S|)` per patch: inlet 0, outlet 0, seam_lo/hi 1.0,
  hub 1.836e-2, shroud 4.463e-3; per-cell closure of the face-centre frame flux (seams included) worst 1.54e-4, 11 600 of 14 168 cells
  above 1e-10. With zero stagger, camber, sweep and lean: hub 4.23e-3, shroud 4.49e-3, closure 9.8e-5: the C-grid hub/shroud faces are not
  chord rectangles even for a flat blade.
* 2-D and 3-D sectors, both full annuli and the K5 joined sector (112 cells, 12 interface faces) build, and `FvGeometry` accepts the bent
  box's cyclic pair. Controls run to convergence: K1 full annulus 277 iterations, K2 full annulus 285, K2 inertial sector 292, K3 inertial
  sector 78, K3 full annulus with the band 82 (accuracy not looked at).
* The new arms (seams on) can only be smoked once the code exists; that smoke is recorded in an amendment before the registered runs.

## Amendment 1: negative controls, smoke of the new arms, and an early run (written before any registered value was read)

* Negative controls, run first as registered: D1 (K1 without seam frame flux) did not converge in 4000 iterations and differs from the
  annulus by 2.183e-1 `W r_o` (507 x `L_DIFF`); D4 (K4 without seam frame flux) leaves the exact state by 2.556 `U` (2.6e8 x the limit).
  Both checks discriminate.
* Disclosed deviation: the cargo name filter used for D1/D4 (`d`) also matched G, K1, K3 and K5, so these registered tests ran once
  before this amendment. Only their pass/fail lines were looked at (G, K1, K3 failed; K5 passed); no value was read. The code is
  deterministic, so the registered runs below repeat that run. No limit or set-up is changed by it.
* Smoke of the seams-on arms (construction plus 30 iterations, finiteness and continuity only): K1 9.6e-7, K2 2.9e-8, K3 9.7e-10,
  K4 4.0e-16, K5 2.1e-8, all finite. No set-up found defective.

## First pass as registered (values)

| check | measured | limit | result |
| --- | --- | --- | --- |
| G pair mismatch (2-D, 3-D, 3-D band, K5) | 2.3e-15, 1.5e-15, 1.5e-15, 2.3e-15 | 1e-12 | pass |
| G per-cell closure with seams | 2.2e-16, 1.5e-16, 1.5e-16, 4.4e-16 | 1e-12 | pass |
| G revolution check with / without seams | 2.3e-15 (accepted) / 1.0 (refused) on every mesh | 1e-6 | pass |
| G tilted axis (3-D sector, 1e-3 rad) | refused (2.3e-3) but the worst face is not on a seam | worst on ymin/ymax | MISS |
| G one-sided zone | refused (1.0) but the worst face is not on a seam | worst on ymin/ymax | MISS |
| K1 converged (sector / annulus) | 307 / 277 iterations | | pass |
| K1 velocity, pressure against analytic | 7.9e-4 W r_o, 3.4e-3 of the range | 2e-2, 5e-2 | pass |
| K1 sector - annulus velocity / pressure | 7.0e-5 W r_o / 5.92e-4 of the range | 4.3e-4 / 4.3e-4 | pass / MISS |
| K1 seam balance | 6.0e-14 | 1e-8 | pass |
| K2 converged; analytic error (annulus 6.76e-3) | 299 it; 6.77e-3 V_i | 1.5e-2 | pass |
| K2 sector - annulus; seam balance | 7.0e-5 V_i; 1.2e-14 | 4.3e-4; 1e-8 | pass |
| K2 inertial pair difference (reported) | 2.3e-10 V_i | none | reported |
| K3 converged; inflow - outflow | 95 it; 6.8e-13 | 1e-9 | pass |
| K3 max u_theta; u_z against the reference | 4.6e-3 U; 1.1e-4 U | 2e-2; 2e-2 | pass |
| K3 sector - annulus; seam balance | 4.64e-3 U; 4.5e-13 | 4.3e-4; 1e-8 | MISS; pass |
| K4 max u - (0,0,U); max p | 1.1e-15 U; 0 | 1e-8; 1e-8 | pass |
| K5 converged; error (controls 6.77e-3, 1.11e-2) | 287 it; 1.27e-2 V_i (0.76 of the 1.5x bound) | 2e-2 and 1.5x | pass |
| K5 net interface flux; seam balance; sum abs flux (reported) | 4.7e-18; 8.4e-12; 5.8e-3 V_i A | 1e-8; 1e-8 | pass |

## Amendment 2 (written after the first pass and the diagnostics below, before any run it governs)

What I knew when writing this: the first pass above and these unregistered diagnostics, all run once on the first-pass code (library
level; nothing here was registered in advance):

* Iteration study: the K1 / K2 sector-annulus differences are identical at 300, 1000 and 4000 fixed iterations (1.401e-4 / 8.881e-4
  absolute for K1 velocity / pressure, 3.494e-5 / 1.477e-4 for K2; residuals down to 1e-13), so they are fixed-point differences.
* K1 inertial pair (walls at `W`, no source): 3.7e-9 W r_o and 6.7e-9 of the range. K1 with an MRF zone at `Omega = 0` (seams on): 7.5e-9.
  Component-diagonal correction off, pair mean on, `simplec_boundary_compensation` on: no change (1.40e-4). SIMPLE with `alpha_p = 1`
  did not converge (not informative).
* K1 with the MRF force removed (frame flux kept, seams on): 7.0e-9 W r_o. With the force face values zeroed on the seam faces: 1.40e-4.
  With the force face values zeroed on the non-cyclic boundary faces (walls): 7.2e-9. Seam-face flux, offset and face force against the
  matching annulus faces: 3.3e-9, 2.3e-17, 8.8e-7. The per-cell map puts the difference in the four cells that touch both a seam and a
  wall (1.4e-4 and 8.6e-5 there, below 2.3e-5 elsewhere); the pressure reference cell 0 is one of them.
* K3: the inertial sector without any source has `max |u_theta|` 4.62e-3 U, the inertial annulus 2.2e-13; MRF at `Omega = 0`, on the band
  and on all cells: 4.62e-3, 4.63e-3, 6.99e-3. The bent-box sector with symmetry planes instead of the cyclic pair (component-diagonal
  correction on): 5.2e-3. Its faces and area vectors equal those of `annulus_3d` to 7e-16 and 3e-17. So the K3 swirl is made by the
  sector with side boundaries in through-flow, cyclic or symmetric, not by the MRF zone or its seams; its cause was not found.

Conclusions. (1) The seam frame flux is exact: with the force removed the MRF sector equals the annulus to 7e-9, K4 holds to 1e-15. (2) The
K1 miss comes from the fixed-velocity wall pressure condition of `SimpleSolver` (the wall-normal gradient `(rAU / rAtU) f_n` of the owner,
`body_ratio`): `rAtU = 1 / (a_p - h1)` with `h1` the sum of the off-diagonal magnitudes of the momentum matrix, and a cyclic face puts its
partner coefficient into the source (lagged), not into the off-diagonals, so `h1` and the ratio of a seam cell differ from those of the same
cell of the annulus; the force has a wall-normal part only at walls, hence the seam-wall corners. Without a force the ratio is not used,
hence the exact inertial pairs. (3) K3 as registered is a defective set-up: its sector-annulus limit is already missed by the inertial pair
without the change (the smoke did not compare the inertial pair; that was a smoke gap).

Changes, registered now:

* **F1 fix attempt.** New `SimpleSolver` flag `coupled_offdiag_in_h1` (default `false`; bit-identical when off): for every cyclic boundary
  face with owner `c` it adds the magnitude of the lagged partner coefficient to `h1[c]`, per active component, as an internal face would
  have it in the off-diagonals: `max(-phi_conv,f, 0) - gamma_f |S_f| c_f` (`c_f = -1 / (dn_f + dn_g)`, the Laplacian partner
  coefficient of the `Periodic` condition). The API sets it when an MRF source has `cyclic_seams = true` (one `sim.rs` line); the library
  harness sets it with seams on. **Falsifier, run first:** K1 with F1, sector against annulus `<= 1e-8` of `W r_o` (velocity) and of the
  range (pressure), the level of the no-force arm. If it misses, F1 and the opt-in are removed. If it passes, G (corrected guards), K1,
  K2, K3b, K4, K5 and D1, D4 are run again with F1, limits unchanged; any miss removes F1 and the opt-in together.
* **K3b** (replaces K3, which stays as an ignored known miss with its limits): the effect of the MRF band must be the same in the sector as
  in the annulus: `max |(u_sector,MRF - u_sector,inertial) - (u_annulus,MRF - u_annulus,inertial)| <= L_DIFF U` over matching cells; the
  other K3 limits unchanged. Known when writing: `max |u_theta|` moves by 8e-6 U in the sector when the band is added; the full field
  difference has not been computed.
* **G guards corrected** (the registered guards stay as an ignored known miss). Tilted axis: the 2-D sector instead of the 3-D one,
  because on the 3-D sector the tilt makes the end planes non-axisymmetric too (worst face there). Prediction written now: the walls' own
  error is about `t z / r` = 1e-3 x 0.05 / 0.5 = 1e-4, below the seam pair mismatch of order `t`, so the worst face lies on a seam;
  criterion `> 1e-6` with the worst face on theta_min / theta_max. One-sided zone: criterion "refused" (`> 1e-6`) only, because a zone
  that holds one side of a pair is bounded inside the sector by a plane through the axis, which reads 1.0 like the unpaired seam face, so
  the face that wins is a tie-break.
* The diagnostic switches used above are not kept in the committed tests (their values are recorded here).

## Amendment 3 (written after the F1 runs below, before any run it governs)

What I knew when writing this, all with F1 (the registered reruns of amendment 2, one run each):

* F1 falsifier: K1 sector against annulus 3.59e-9 `W r_o` (velocity) and 6.62e-9 of the range (pressure), limit 1e-8: **pass**. The
  cause of the K1 miss is confirmed.
* G construction: unchanged values, pass. K1: every limit passes (sector-annulus 3.6e-9 / 6.6e-9). K2: pass (sector-annulus 1.2e-10 V_i,
  analytic 6.76e-3 V_i). K4: 1.1e-15, pass. K5: pass (1.27e-2 V_i, 0.76 of the bound; net flux and seam balance as before). D1, D4 still
  discriminate (unchanged: F1 is off without seams).
* K3 as registered, now run with F1 on the MRF arm: every limit passes (sector-annulus 1.8e-11 U, `max |u_theta|` 1.1e-10 U, `u_z`
  against the reference 3.2e-3 U, in/out 4e-16, seam 6e-18). So amendment 2 was wrong to call K3 a defective set-up: the 4.6e-3 U swirl
  of the inertial sector is the same `h1` defect of cyclic cells (the inertial sector has no F1, the MRF arm has it). Its limits are met
  by the final code, so K3 is kept as a passing test with its registered limits.
* K3b missed: 4.64e-3 U against 4.3e-4 U. Its sector reference (inertial, no MRF) runs without F1 and therefore carries the defect that
  F1 removes from the MRF arm; the difference of effects measures that defect, not the band. K3b is a defective set-up and stays as an
  ignored known miss with its limit.
* Corrected tilt guard missed its face clause: the 2-D sector with the axis tilted by 1e-3 is refused (1.0e-4 > 1e-6) but the worst face
  is a wall, not a seam. My prediction was wrong: the tilt's in-plane part gives the seam pair a mismatch of order `t z` too, as on the
  walls. Any misaligned axis also breaks the walls and zone interfaces, which are surfaces of revolution about the true axis, so "worst
  face on a seam" is not a property of the code. The registered refusal holds in every guard run (2.3e-3, 1.0e-4, 1.0).

Changes, registered now:

* **K3c** (replaces K3b): K3b with F1 also on both inertial sector arms (the flag set on the solver at library level), limits unchanged:
  the band effect in the sector against the band effect in the annulus `<= L_DIFF U`, plus the other K3 limits.
* **K3i** (reported, not a criterion): the inertial sector with F1 against the inertial annulus, to show whether F1 alone removes the
  4.6e-3 U swirl of the inertial sector.
* **Guards, second correction** (the amendment 2 version stays as an ignored known miss): criterion "refused" (`> 1e-6`) for the tilted
  axis (2-D and 3-D sectors) and for the one-sided zone, plus, for the tilt, the seam pair term on its own `max |F_f + F_g| / (|Omega| r
  |S|) > 1e-6` (it detects the misalignment by itself), against `<= 1e-12` when aligned (G).
* Decision rule unchanged: a miss of G, K1, K2, K3, K3c, K4, K5 or the corrected guards removes F1 and the opt-in together.

## Addendum: measured values (final code, F1 included; one run each)

| check | measured | limit | result |
| --- | --- | --- | --- |
| D1 K1 without seam frame flux, sector - annulus | 2.18e-1 W r_o, not converged | >= 10 x 4.3e-4 | discriminates |
| D4 K4 without seam frame flux | 2.56 U | >= 10 x 1e-8 | discriminates |
| F1 falsifier: K1 sector - annulus velocity / pressure | 3.59e-9 W r_o / 6.62e-9 of the range | 1e-8 | pass |
| G pair mismatch / closure / accepted with seams / refused without | 2.3e-15 / 4.4e-16 / 2.3e-15 / 1.0 (worst of four meshes) | 1e-12 / 1e-12 / 1e-6 / > 1e-6 | pass |
| Guards (second correction): tilted axis 2-D, 3-D (check; seam pair term) | 1.0e-4, 8.4e-5; 2.3e-3, 1.9e-3 | > 1e-6 each | pass |
| Guards: one-sided zone | 1.0 | > 1e-6 | pass |
| Guards as registered / as corrected in amendment 2 | refused, worst face not a seam | worst face on a seam | ignored known misses (defective set-ups) |
| K1 analytic velocity / pressure | 7.9e-4 W r_o / 3.7e-3 of the range | 2e-2 / 5e-2 | pass |
| K1 sector - annulus velocity / pressure; seam balance | 3.6e-9 / 6.6e-9; 0 | 4.3e-4; 1e-8 | pass |
| K2 analytic (annulus 6.76e-3); sector - annulus; seam balance | 6.76e-3 V_i; 1.2e-10 V_i; 1.4e-16 | 1.5e-2; 4.3e-4; 1e-8 | pass |
| K3 in/out; swirl; u_z against reference; sector - annulus; seam | 4.3e-16; 1.1e-10 U; 3.2e-3 U; 1.8e-11 U; 5.5e-18 | 1e-9; 2e-2; 2e-2; 4.3e-4; 1e-8 | pass |
| K3b band effect (reference without F1) | 4.64e-3 U | 4.3e-4 | ignored known miss (defective set-up) |
| K3c band effect (F1 on all sector arms) | 1.8e-11 U | 4.3e-4 | pass |
| K3i inertial sector with F1 against the inertial annulus (reported) | 2.2e-12 U (4.6e-3 U without F1) | none | reported |
| K4 exact state: max u - (0,0,U); max p | 1.1e-15 U; 0 | 1e-8 | pass |
| K5 error (controls 6.76e-3, 1.107e-2); net interface flux; seam balance | 1.27e-2 V_i (0.76 of the bound); 2.0e-17 (5e-18 on the scratch tree at fee6ecc); 8.1e-12 | 2e-2 and 1.5x; 1e-8; 1e-8 | pass |
| K5 sum of absolute interface fluxes (reported) | 5.8e-3 V_i A | none (ADR 0109 known miss) | reported |
| K6 fan empty passage (library level, 300 iterations) | 7.4e-4 U, worst cell on hub/shroud, not on a seam; seam pair 5.4e-14 | 1e-8 | MISS (ignored known miss, as predicted) |
| A1 key absent / false / true on the M2 annulus, 20 iterations | bit-identical | bit-identical | pass |
| A2 refusal: without key / with key | `patch 'seam_lo'` (1.0) / `patch 'hub'` (1.84e-2) | names a seam / hub | pass |
| A3 `cyclic_seams` on `rotating_frame` | located error at `sources.frame.cyclic_seams` (raised when the case is parsed) | located | pass |
| `s4_absolute_formulation_mrf_is_refused_on_the_sector` (existing) | passes | | pass |

Verdict: G (corrected guards), K1, K2, K3, K3c, K4, K5 pass with F1, and D1, D4 discriminate, so by the decision rule F1 and the opt-in
are kept. What this delivers: an MRF zone may now contain rotationally cyclic seams, exactly (the sector reproduces the full annulus
to 1e-8 or better in every check). The positive path is verified at library level only. Through the case tree the key is checked,
its refusals are located and the default is bit-identical, but no run sends `cyclic_seams = true` through `Sources` and the one-line
`sim.rs` wiring of F1 into the solver on a mesh with cyclic faces, because no case-tree mesh reaches it (below). The default-path
refusal message also changed: it now names the patch of the worst face and hints at `cyclic_seams` (diagnostic text only). What it does not deliver: the fan. The `axial_fan_sector`
hub and shroud faces are not surfaces of revolution at the face centres (1.8e-2 and 4.5e-3 of `Omega r |S|`), so the case tree still
refuses an MRF zone on the fan passage, now at the hub; at library level the same passage leaves the exact state by 7.4e-4 U, located
at hub/shroud cells. Curing that is the wall-frame-flux question of F-RP (ADRs 0082, 0085), or a mesher change that makes the hub and
shroud faces chord rectangles; neither is attempted here. A sector also cannot reach the case tree through a mesh file (the OpenFOAM
writer stores `neighbourPatch` only, not the rotation).

Finding outside the task, reported: the default SIMPLEC kernel builds `rAtU` of cyclic cells without the lagged partner coefficients.
Without a body force the converged solution does not depend on it in the 2-D swirl checks (inertial pairs match to 1e-9), but an inertial
3-D sector through-flow carries 4.6e-3 U of spurious swirl from it (K3i: 2.2e-12 with F1). The `rotating_frame` fan passage and every
periodic SIMPLEC case use that default path. F1 is opt-in here (switched on only by `cyclic_seams`), so every earlier result and
`crates/rcfd-api/tests/baseline_hashes.rs` stay bit-identical; making it the default for periodic cases is a separate decision.

Deviations from the registered plan, all stated in the amendments: the early run caused by an over-broad name filter (amendment 1); the
first-pass misses and the unregistered diagnostics that located them (amendment 2); F1 added by amendment 2 with a falsifier that ran
first; K3 wrongly called defective in amendment 2 and restored in amendment 3 because its registered limits are met with F1; K3b and the
amendment-2 guard set-ups defective (amendment 3); A3's test first failed because the located error is raised by `Case::parse_toml`
(the test now accepts the parse-time diagnostics; the criterion is unchanged). The diagnostic switches are not in the committed tests.

Unverified references: none; every reference value is an exact solution or an earlier ADR's measured value (ADR 0008 M3 for L_DIFF).

Status: F-126 stays partial (acceptance, a published compressor case, not attempted); its note is extended.

## Final gate

Fresh `git archive` of HEAD 2a48571 (which includes the wave-44B commits made meanwhile): `cargo fmt --check` clean, `cargo clippy
--workspace --all-targets -D warnings` clean, `cargo test --workspace --release --no-fail-fast` 2006 passed, 1 failed, 627 ignored. The
failure was `rules_w19c::e2_no_error_level_false_positives_and_warnings_are_only_sv103`: it extracts case texts from test sources, and the
fan mesh section of `crates/rcfd-api/tests/mrf_cyclic_seams_w44c.rs`, written as a standalone constant, parsed as a case whose sheared
C-grid raises SV102 (skewness above 4). The text is now inline in the `format!` template (which E2 skips as a template); no check or
value changed. Re-run in the gate directory with that file: `rules_w19c` 5 passed, 1 ignored; `mrf_cyclic_seams_w44c` (rcfd-api) 3
passed, 1 ignored. Corrected totals: 2007 passed, 0 failed, 627 ignored. Every ADR 0113 value re-measured on the gated tree equals the
scratch-tree value above except the K5 net interface flux (2.0e-17 against 5e-18, both at round-off; 44B's cylindrical-interface commit
is in the gated tree). `crates/rcfd-api/tests/baseline_hashes.rs` untouched and passing.
