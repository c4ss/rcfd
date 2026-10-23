# ADR 0113: Wave 44C, MRF zones with rotationally cyclic seams (F-126)

Status: registered before any solver code of this wave was written. Limits and decision rules below are not changed after a run; a
missed limit becomes an ignored `*_known_miss` test with the limit intact; a defective set-up gets a new corrected test registered here
as an amendment before it runs. Written at HEAD 2f1a3ce, target `<target dir>`. Clean-room: first principles only
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
