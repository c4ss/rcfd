# ADR 0068: Wave 33A, mesh quality foundation (body-fitted C-grid passage mesh, seam-flux diagnosis, test hardening)

Status: registered before any implementation. The limits below are fixed; a miss becomes an ignored `*_known_miss` test with
the limit unchanged, a set-up defect becomes a NEW corrected test with the old one kept. Serves `docs/milestone-fan.md`
(capability link "volume mesh of a periodic blade passage") and repairs the quality miss recorded in the addendum of ADR 0065
(M4: non-orthogonality 68-90 deg against 70, skewness 0.75-66 against 2.0, aspect ratio 405-19123 against 300/3000).

Clean-room: first principles and published literature only. The C-grid idea (grid lines wrapping the body, a wake cut that
carries the grid lines downstream, H-type blocks elsewhere) is standard structured-mesh practice (Thompson, Soni & Weatherill,
Handbook of Grid Generation, 1999; Thompson, Warsi & Mastin, Numerical Grid Generation, 1985); no source of another code was
read, no vendor binary was inspected. The coordinate formulas below are constructed here.

## Scope statement (what this is NOT)

A better hex mesh of one blade passage. It is a meshing deliverable with measured quality and conservation properties. It is
NOT validated for fan performance prediction; no comparison with a published fan was made. The old generator
(`build_sector_mesh`, "H-grid with a slit") is kept unchanged and stays the default (baseline hashes and ADR 0065/0067 tests
must not move).

## Why the 0065 mesh misses (diagnosis recorded in 0065): blunt flat ends of height about 2 yt cut by planes z = const, columns
that are z = const lines, and a middle band opening linearly. The worst cells are the first columns behind the trailing plane
at mid-span. The remedy is to remove the flat end faces and the z = const column constraint, not to tune them.

## Topology (new generator `rcfd-mesh::fan_cgrid::build_cgrid_mesh`, option `mesh.fan.topology = "c_grid"`; default `"h_slit"`)

Every radial node layer k (radius r_k, radii exactly as in 0065: `graded_cells` stacks from hub, tip cap (below and above) and
shroud) carries the same 2-D multi-block grid in the unrolled plane (z, s = r theta), stacked radially and bent to
(r cos(s/r), r sin(s/r), z). Blocks, all hexahedral:

* **C block** (index i around the blade, j away from it). Half index h = 0..H, H = n_down + n_chord (h = 0 is the outlet end of
  the lower seam, h = n_down the trailing-edge node, h = H the leading-edge node); full index i = h (suction side, lower seam)
  and i = n_i - h (pressure side, upper seam), n_i = 2H. Wall curve W(i): the wake line (axial, s = s_TE, z from the outlet to
  the trailing edge; nodes shared by the two sides, i.e. the wake cut is a set of interior faces), the suction surface from the
  trailing edge to the leading edge at chord stations x_m = (1 - cos(pi m / n_chord)) / 2 (m = 0 leading edge, m = n_chord
  trailing edge; surface points `surface_zs(x_m, side)` of the exact section), and the pressure surface back. The leading- and
  trailing-edge nodes are single nodes (sharp section, closed trailing edge): no blunt end faces, no truncation, no
  monotonicity restriction on the section.
* **Boundary-layer stack**: on every blade wall node, n_bl nodes at distances y_q = sum_{p<=q} h1 g^p along the in-plane unit
  normal n (outward from the solid). n is the rotation of the central difference of the wall nodes (one-sided is not needed:
  wall nodes are interior of the chain) with two defined exceptions: leading-edge node n = -z (forward), trailing-edge node
  n = -s on the suction j-line and +s on the pressure j-line (the j-lines at the trailing edge are pitchwise so that they
  continue the pitchwise j-lines of the wake). In the wake there is no wall and no exact stack.
* **Outer part of the j-line**: from the last stack node E to the outer point O(i) with a geometric fit (`graded_cells`, ratio
  solved, first spacing h1 g^n_bl on the blade, d1_w in the wake). O(i): on the seams for i < h_c (h_c = H - n_nose), on the
  front edge (z = z_f) for h_c <= i <= n_i - h_c. Seam points of station h have z_out(h), the SAME function for the lower
  and upper seam (so the upper seam is the lower seam translated by the pitch P = 2 pi r / N in s, i.e. rotated by exactly
  2 pi/N: nodes match to round-off). Wake stations: z_out = z of the wake node (pitchwise rays, exactly orthogonal to the
  wake line). Blade stations: z_out(h) = q(h) - (q_c - z_f) exp(-(q(h) - q_c)/lambda) with q(h) the camber-line z of the chord
  station of h, q_c = q(h_c), lambda = 0.25 c (c = local chord), z_f = z_LE - 0.3 c (`front_gap`); the front edge nodes are
  graded in s from the corner (z_f, s_lo) to (z_f, s_LE) with n_nose cells (`graded_cells`, first cell at the corner equal to
  the seam cell there); mirrored on the pressure side.
* **Wake first spacing**: d1_w(h) = max(h1, min(0.5 dz_h, 0.5 W_h / n_side)), dz_h the local axial cell size, W_h the width to
  the seam.
* **F block** (inlet): rows a = 0..n_up in z from z = 0 to z_f (graded, the last cell equals the adjacent seam cell), columns
  b = 0..2 n_nose at the front-edge s nodes; row a = n_up is the front edge of the C block (shared nodes); b = 0 / 2 n_nose
  are the lower / upper seam.
* **Core block** (inside the blade, only for the tip gap): columns m = 0..n_chord at the same chord stations, n_core cells from
  the suction node to the pressure node. At the leading and trailing node the column collapses to the single wall node, so the
  first and last core cells are triangular prisms (degenerate hex faces with fewer than three distinct nodes are dropped,
  coincident nodes of a face are merged). Core cells are deleted for k < n_span (the blade), kept for k >= n_span (the tip
  gap), where their k = n_span bottom faces are the `blade` tip cap. With `include_blade = false` all core cells are kept.
* Patches: `inlet` (F row 0), `outlet` (C i = 0 and i = n_i faces), `hub`, `shroud`, `blade` (C j = 0 faces of the chain below
  the tip, the core bottom faces at k = n_span), `seam_lo` / `seam_hi` (`set_periodic_rotation(.., 2 pi/N)`). Zone `rotor`.
* The seam lines are axial lines s = s_lo(k) and s_lo + P per layer, with s_lo = s_centre - P/2, s_centre the mean of the
  leading- and trailing-edge s (the blade is centred between its seams; lean and stagger change s_centre with r, so the seam
  surface is twisted exactly as the camber-following seams of 0065 were). A blade that does not fit (the stacks do not leave
  room) is a located error.

Known limits declared in advance: a section with a sharp (wedge) leading edge (circular arc family) has a single j-line at a
wedge of angle < 90 deg and is expected to give poor leading-edge cells (not gated; the gates are measured on NACA 4-digit
sections); the wall-normal height is exact in the unrolled section plane and differs from the 3-D normal by the twist/lean rate;
rays from the stack edge to the seams are pitchwise, not wall-normal, so the cells outside the stack are sheared by up to the
local blade angle; the wake line is axial.

## Sizing arithmetic (done before registering the set-ups)

Aspect ratio is longest over shortest edge, so a cell in the first hub, shroud or tip-cap layer has aspect >= (largest in-plane
edge) / h1 whatever the topology. From the 0065 formula (U = 90 m/s, nu = 1.5e-5, mid-span chord 0.085 m): h1 = 2.2e-4 m at y+ = 30
(A') and 7.3e-6 m at y+ = 1 (B'). Hence every in-plane edge on the end walls must stay below 300 h1 = 0.066 m (A') and
3000 h1 = 0.022 m (B'). The pitch at the shroud is 0.39 m, the inlet and outlet lengths are 0.27 m and 0.31 m. Counts chosen for
this: B' has n_nose = 16 (front-edge cells 0.39/32 = 0.012 m), n_side = 30 (largest ray cell below 0.02 m), n_up = n_down = 40 (largest
axial cell below 0.02 m), n_span = 60 (largest radial cell about 0.01 m against the blade-wall normal h1). These are the set-ups;
they are not adjusted after the measurement.

* A' : y+ = 30, n_bl = 5, g = 1.2, n_up 24, n_chord 32, n_down 28, n_side 20, n_nose 8, n_core 2, n_span 30, n_gap 18.
* B' : y+ = 1, n_bl = 8, g = 1.2, n_up 40, n_chord 48, n_down 40, n_side 30, n_nose 16, n_core 2, n_span 60, n_gap 30.
* Blade B of ADR 0065 (N = 5, hub 0.10, tip 0.30, clearance 0.01, NACA 4412-like, chord 0.10 to 0.07, stagger 35 to 20 deg,
  camber 0.04, p 0.4, t 0.10 to 0.06, sweep 0 to 0.01, lean 0 to 0.005, axial extent 0.7, z_le_root 0.3), `front_gap` 0.3.
* Small set-up S (refinement, solves): n_up 8, n_chord 16, n_down 8, n_side 10, n_nose 4, n_core 1, n_span 14, n_gap 7, n_bl 2,
  g 1.2, h1 = 1e-3; levels use the stated counts.

## Amendment 1 (written before any mesh code existed or ran; nothing measured yet)

Pencil analysis of the corner geometry (no mesh was generated) showed three construction problems in the text above. They are fixed here, before implementation, and
the gates and set-ups are untouched:

1. **Wall normal everywhere** is the rotation of the central difference of the neighbouring nodes of the wall-and-wake curve (one-sided only at the two outlet ends). There
   is no exception at the leading edge (forcing -z would point 46 deg away from the true normal of a staggered, cambered nose and tangle the nose cells) and none at the
   trailing edge (a pitchwise trailing-edge j-line crosses the neighbouring pressure-side stack lines, a concave corner). The trailing-edge node has two normals (suction
   side uses the wake neighbour and the first suction node, pressure side the first pressure node and the wake neighbour).
2. **Wake line** leaves the trailing edge along the tangent of the PRESSURE surface (slope sigma_0 = ds/dz at x = 1) and bends to axial:
   s_w(z) = s_TE + sigma_0 lambda_w (1 - exp(-(z - z_TE)/lambda_w)), lambda_w = 0.2 c. (An axial wake would make a concave turn of about the stagger angle on the pressure side where
   the stack lines would fold; along the pressure tangent the pressure side is smooth and the suction side has a convex turn of the trailing-edge wedge angle.) The wake stations carry
   a stack along the wake normal (first spacing d1_w, n_bl layers) while its thickness is <= 0.25 of the width to the nearest seam, otherwise a pure geometric fit.
3. **Seam point of a station**: z_out(h) = z_a - (q_c - z_f) exp(-(z_a - q_c)/lambda) where z_a = (z of the stack edge on the lower line + z of the stack edge on the upper
   mirror line)/2 and q_c = z_a at the corner station (lambda = 0.25 c). The same z_out serves both seams. (Replaces the camber-line formula; at mid chord the two coincide.)
4. **Chord stations** x(u) = 0.85 (1 - cos(pi u))/2 + 0.15 u, u = m / n_chord (a 15 % linear share keeps the end spacing from collapsing, which the stack thickness at y+ = 30
   would otherwise exceed at the trailing edge).

## Amendment 2 (written after development runs of the small set-up S only; A' and B' have NOT been run)

Development iterations on S (a layer-by-layer check of 2-D cell areas and a per-face listing of non-orthogonality; S is not a gated set-up) changed the line construction. They
are listed here so that the later measurement can be read against them. Gates, set-up counts and `front_gap` 0.3 for A', B', S are unchanged.

1. **Pursuit lines instead of straight rays.** A straight ray from the stack edge to its seam or front-edge target grazes the wall near the pressure-side nose and folds (negative
   2-D areas on S, cell 121 of the first build). Each line now leaves along the wall normal and turns smoothly to aim at its target (direction blend weight smoothstep of
   q / (0.7 n) over the n cells outside the stack), reaching the target exactly; step lengths keep the graded fractions of the remaining distance. The outlet lines (i = 0, n_i) are
   straight so that the outlet stays planar. A Bezier variant was tried first and hooked at the trailing edge; it is not used.
2. **Common first height everywhere.** The wake stacks use d1 = h1 as the blade (not 0.5 x the local axial spacing): a first height that varies by 20 % between neighbouring lines misaligns
   the rows by more than the line spacing and gave 80+ deg faces in the wake on S. The wake cells are therefore thin in the stack rows (aspect is gated, the wake is not a wall).
3. **Front-edge nodes** start at the nose with the width of three wall cells and grow geometrically to the seam corners (previously the reverse).
4. **Known geometric limit, measured on S only:** at `front_gap` 0.3 the nose fan spreads n_nose lines over the 0.28 m front edge that lies 0.02 m ahead of the nose; the lines are
   oblique by construction. S gives max non-orthogonality 78.8 deg and 371 faces above 60 deg of 11536 cells. A larger `front_gap` (1 chord and more) tangles the pressure-side
   fan (lines graze the staggered pressure surface) and is rejected by the generator with a located error. I therefore expect A' and B' to MISS the 70 deg gate; this is a
   prediction written before they run, not a gate change.

**Decision rule (registered before A' and B' run):** A' and B' are run once each with the construction above. A measured value beyond a limit becomes an `#[ignore]`d `*_known_miss`
test with the limit unchanged; no re-run with another construction to pick a better value. A corrected set-up is added only for a defect of the set-up itself (not of the quality), and
then as a new test with the old one left ignored.

## Pre-registered checks

Tests: `crates/rcfd-mesh/tests/fan_cgrid.rs`, `crates/rcfd-api/tests/fan_cgrid_case.rs`, `crates/rcfd-api/tests/seam_flux_w33a.rs`.

* **Q1 quality** (A', B'; same numeric limits as 0065, not relaxed): max non-orthogonality <= 70 deg, max skewness <= 2.0, max aspect
  ratio <= 300 (A') / <= 3000 (B'). Full distribution (max, mean, p99, counts above 50 and 60 deg, min volume, max volume ratio) reported.
* **Q2 closure/topology** (S and A'): `Mesh::validate`; each internal face has two distinct cells, owner < neighbour; all cell volumes
  > 0; per cell |sum S_f| <= 1e-13 sum |S_f|; every face incident to 2 (internal) or 1 (boundary) cells.
* **Q3 volume conservation**: with V_env from the same mesh with `include_blade = false`, |V_passage + V_solid - V_env| <= 1e-11 V_env, V_solid
  the divergence-theorem volume of the `blade` patch plus the root footprint polygon (wall chain at k = 0).
* **Q4 refinement order, volume**: (a) envelope volume against pi/N (Rs^2 - Rh^2) L for levels (n_nose, n_side) = (4, 8), (8, 16), (16, 32) with the
  other counts of S: strictly decreasing, finest <= 5e-3, observed order between the last two >= 1.5. (b) blade solid volume (envelope - passage) against
  the quadrature of `fan.section_area(r)` over r (the exact section, no truncation), levels (n_chord, n_span) = (16, 8), (32, 16), (64, 32):
  strictly decreasing, finest <= 2e-3, order between the last two >= 1.5.
* **Q5 refinement order, area**: area of the lateral blade patch (faces of the C j = 0 chain, k < n_span) against the quadrature of |X_u x X_r| over the exact lateral surface
  (the same quadrature as ADR 0065 G2), same levels as Q4(b): strictly decreasing, finest <= 2e-3, order >= 1.5.
* **Q6 periodicity / rotation match**: (a) every `seam_lo` node rotated by +2 pi/N about z lands on a `seam_hi` node within 1e-12 m (bijection); (b) equal face
  counts, rotated face centres match within 1e-12 m; (c) area vectors: |S_hi + R S_lo| <= 1e-12 |S_lo| for every matched pair (the property the flux antisymmetry needs);
  (d) `FvGeometry::new` accepts the mesh.
* **Q7 exact wall stacks**: hub, shroud, tip cap (above and below): spacings h1 g^m, relative deviation <= 1e-12 (A'); on every blade wall node
  the distance of stack node m along the defined normal equals sum_{p<=m} h1 g^p within 1e-12 (A'); for B' the tolerance is 4 eps 0.4 / h1 (the f64
  coordinate resolution found in 0065, unchanged reasoning); h1 from y+ matches the formula within 1e-14. Normals at the leading and trailing node are the
  defined exceptions above. The 3-D distance differs by the declared twist rate and is not gated.
* **Q8 validation**: unfittable specifications (n_nose >= n_chord, n_side <= n_bl, front gap beyond the inlet, blade too wide for the pitch) return located errors, never a panic.
* **R1 rules**: the example case `cases/axial_fan_cgrid` built through the same mesh-JSON producer the service uses, `rules::check_case` returns NO finding of
  severity error (SV011 is the only error-level mesh rule, limit 85 deg); the warnings (SV101 > 70, SV102 > 4, SV103 > 1000) are reported.
* **S1' empty passage** (C-grid, `include_blade = false`, walls moving at the inflow): converged velocity equals (0,0,U) within 1e-8 U (as 0065 S1).
* **S3' laminar rotating passage** (as 0065 S3 on the C-grid mesh S; relative-frame `rotating_frame` source): converges within the cap; |m_in + m_out| <= 1e-6 |m_in|
  (registered limit unchanged); the pair mismatch (m_lo + m_hi)/|m_lo| and the per-face pair metric below are reported.

## Seam-flux diagnosis (0065 S3: relative mismatch 3.5e-6 to 2.7e-5 between the seam_lo and seam_hi mass flows)

Pair metric: e_pair = max over matched seam pairs (f, g) of |phi_f + phi_g| / sum_{seam faces} |phi|, from the converged face flux `phi`; and the patch level
(m_lo + m_hi)/|m_lo|. Read-only reading of `rcfd-fv` pressure/field code shows the pressure flux of a coupled pair constructed to be antisymmetric
(`coupled_gamma`, `boundary_hby_a` with the partner vector rotated by the patch rotation, distance weights w and 1 - w); the diagnosis tests where that
symmetry could be lost. Tests, all on the converged steady SIMPLEC solution, cell counts small:

* D1 control: plane Couette flow in a box with a TRANSLATIONAL periodic pair (existing generator) -- e_pair expected <= 1e-12.
* D2 analytic: circular Couette flow in an empty rotational sector (C-grid mesh, `include_blade = false`; hub and shroud rotating at the exact
  angular velocities of u_theta = A r + B/r, stationary frame, no source, inlet prescribed with the exact profile): the exact solution is axisymmetric, so the seam
  fluxes cancel pairwise; e_pair <= 1e-12 expected if the rotational pair is consistent.
* D3: D2 with the `rotating_frame` source at the angular velocity of the frame (hub/shroud velocities transformed): e_pair <= 1e-12 expected.
* D4: S3' (the fan mesh S) e_pair, and e_pair after replacing the non-orthogonal correction by none if the solver has such an option (read-only: no code change).

Decision rule (registered before running): e_pair is "clean" below 1e-12. (i) D1 not clean: the periodic machinery itself is not antisymmetric. (ii) D1 clean, D2
not clean: rotational-pair treatment in the solver. (iii) D1, D2 clean, D3 not: the frame source enters the face flux. (iv) D1-D3 clean and D4 not: the mismatch is mesh
specific (non-orthogonality/skewness at the seam faces, or iteration lag of the partner values), reported with the measured seam-face quality. No tuning, no
solver change required; a fix, if any, would be opt-in and is not part of this wave. `crates/rcfd-fv/src/lib.rs` (dirty from another wave) is not committed.

## Test hardening: `service::tests::mesh_file_outside_sandbox_and_runtime_cap_are_enforced`

Wall-clock dependences found by reading the test and the worker: a 0.05 s cap measured from the worker start, and the `limit` event searched in the first 500 events of
`events.poll` (the worker emits one `iteration` event per step; a faster machine emits more steps before the cap trips). The test is changed so that the outcome does not
depend on load or speed: the cap is zero (the worker checks `elapsed > cap` before the first step, so the cap trips deterministically on any clock), the events are read
across all pages with the `next` cursor, and the reason string `max_run_seconds exceeded` is asserted in addition to the `limit` event and the `stopped` state. The sandbox
assertions are unchanged. Acceptance: the test passes 100 consecutive runs, also while the machine is loaded by a concurrent build.

## Not claimed

Fan performance, a comparison with a published fan, y+ of a converged solution, quality of sections with a sharp leading edge, anything about sharp-edge turbulence.

## Addendum: measured values, misses and deviations

All values from release builds, `cargo test --release`, on the commits of this wave. Limits are the registered ones, unchanged. Reference values: Q4/Q5 use
the analytic annulus sector and the quadratures of ADR 0065 (no external reference); the circular Couette solution u_theta = A r + B / r is textbook
(UNVERIFIED against a cited page; it follows from the Navier-Stokes equations in polar form). No published fan data was used.

### Development iterations after Amendment 2 (disclosure)

A' and B' were built and measured several times while the construction changed; the registered limits were not touched. Sequence: first A' build failure
then an angle-weighted pursuit fix; `capped_cells` with an axial cap in the wake; wake first height d1w = max(h1, dz/300); the trailing-edge wedge merged into
one polyhedral cell per layer; a pitchwise stack on the outlet lines; `graded_two_sided` radial grading; CAP_FACTOR changed from 2.0 to 1.5 AFTER a B'
measurement (a post-measurement change); a smoothing attempt (aspect worsened to 396) that was removed. The sizing arithmetic for the ray cells and for
the radial cells in this ADR was wrong (the geometric fit gives much larger cells than assumed), which is why the capped constructions were added.

### Results

| Check | Registered limit | Measured | Status |
|---|---|---|---|
| Q1 A' non-orthogonality | <= 70 deg | 81.57 deg (mean 12.16, p99 64.66, 5177 faces above 60) | MISS (`q1_quality_gates_known_miss`) |
| Q1 A' skewness / aspect | <= 2.0 / <= 300 | 0.863 / 176.6 | met |
| Q1 B' non-orthogonality | <= 70 deg | 83.73 deg | MISS |
| Q1 B' skewness | <= 2.0 | 4.846 | MISS |
| Q1 B' aspect | <= 3000 | 2651 | met |
| Q2 closure, S / A' | <= 1e-13 | 5.4e-16 / 8.2e-16; owner < neighbour, incidence counts, volumes > 0 | met |
| Q3 volume conservation, as registered (flat root polygon) | <= 1e-11 | 1.6e-5 | MISS: set-up defect (`q3_volume_conservation_flat_closure`, ignored) |
| Q3 corrected (hub boundary = the core quads of the empty mesh) | <= 1e-11 | 1.8e-16 (C nodes of the bladed and empty builds identical) | met |
| Q4a envelope refinement (4,8), (8,16), (16,32) | see above | cannot be built (fold; third level also has n_nose = n_chord = 16) | MISS (`q4a_..._known_miss`) |
| Q4b/Q5 solid volume and lateral area refinement | finest <= 2e-3, order >= 1.5 | level 1 built (volume error 1.2e-2, area 5.4e-3); level 2 folds | MISS (`q4b_q5_..._known_miss`) |
| Q6 periodicity | 1e-12 | node rotation 1.1e-16; face centres 3.4e-16; area vectors 3.9e-13 (S, A', changed lean/sweep); `FvGeometry::new` accepts | met |
| Q7 stacks A' | 1e-12 | radial 3.2e-13, wall distance 1.3e-13, transverse 1.4e-13; h1 formula within 1e-14 | met |
| Q7 stacks B' | 4 eps 0.4 / h1 = 4.9e-11 | radial 1.7e-11, wall 4.1e-12 / 4.6e-12 | met |
| Q8 located errors | no panic | all cases return located errors (see deviations) | met |
| R1 rules on `cases/axial_fan_cgrid` | no error-level finding | 11 536 cells, 79.65 deg; one finding, SV101 (warning) | met |
| S1' empty passage | <= 1e-8 U | 4.1e-12 U (the residual target 1e-11 was not reached in 400 iterations; the velocity error is) | met |
| S3' m_in + m_out | <= 1e-6 \|m_in\| | 6.0e-5 (m_shroud = -3.2e-6 of m_in 5.37e-2) | MISS (`s3_prime_rotating_passage_known_miss`) |
| S3' corrected (shroud included) | 1e-6 | 1.0e-14; pair \|m_lo + m_hi\| / \|m_lo\| 1.4e-15; converged in 406 iterations | met |
| Service test hardening | 100 consecutive passes, also under load | 100 / 100 (other agents were building concurrently) | met |

The S mesh quality of Amendment 2 (78.8 deg from `quality_summary`) and the example-case value (79.65 deg from `check_mesh`) come from two different
code paths that define the angle slightly differently; not reconciled.

### Seam-flux diagnosis (b)

e_pair (limit "clean" 1e-12): D1 translational Couette 3.5e-18 (NOT converged either: res_p 0.054 after 3000 iterations, res_u 3.7e-16; a deviation from "converged solution"); D2 circular Couette, empty C-grid sector 7.0e-17 (converged, 856 iterations); D3 the same in the
rotating frame 1.6e-17 (NOT converged: res_u 0.22 after 4000 iterations; the pair metric is a structural property, but this is a deviation from "converged
solution"); D4 S3' fan passage 7.8e-17 (converged, 358 iterations; the solver default is already no non-orthogonal corrector, so the "correction replaced by none"
variant is the same run). D5 (not registered, control): the 0065 H-slit example case, 644 pairs, e_pair 8.4e-17, patch level 5.9e-16, converged.
Outcome: all registered tests are clean, so rules (i) to (iv) do not fire on this HEAD, and the 0065 mismatch of 3.5e-6 to 2.7e-5 is NOT reproduced, not even on the H-slit mesh (D5).
Attribution: wave 33D (ADR 0071, C5) found the cause in the solver, `SimpleSolver::iterate`: the body-force term of the face flux used the owner `rAU` on boundary faces, which
differ across a periodic pair, so the Coriolis and centrifugal terms made phi_lo + phi_hi nonzero; the fix (`coupled_body_pair_mean`) is opt-in and the API switches it on with the
rotating-frame source. My D3, D4 and D5 runs have the rotating-frame source and therefore the fix on; D1 and D2 have no body force. I did not run D3 or D5 with the fix off (no case-level
switch was used), so the attribution rests on ADR 0071's before/after numbers, not on my own measurement; under the registered rule it would be case (iii). No solver change was made here.

Separate from the pair: the converged rotating passage on the C-grid mesh has a mass flux through the no-slip, moving shroud of -3.2e-6 (6.0e-5 of m_in), about 200 times the
H-slit mesh value in 0065 (1.7e-8 of the flow). That is a real leak on this mesh, not only a metric problem: the faceted shroud wall moves tangentially, so its facets
carry a wall-normal velocity component. The corrected S3' test (shroud flux included) hides none of it: it reports the value above and the registered test stays ignored.

### Deviations from the registered plan

* Q3 as registered used a flat polygon on the hub; the empty mesh's hub there consists of the core quads, so the identity cannot hold to 1e-11 (set-up defect, corrected test added).
* Q4a/Q4b/Q5: the generator folds at the registered refinement levels. An unregistered scan of 144 combinations (n_chord 16/24/32, n_nose 2-8, n_side 6-32, with and without
  blade) built 44; I did not pick replacement levels from it (that would be choosing after seeing results). Its envelope volumes are non-gating observations: for example n_chord 16,
  n_nose 4, n_side 8/10/16 gave 3.7680e-2 / 3.7710e-2 / 3.7737e-2 m^3 against the analytic 3.787e-2 (the analytic value is from the registered formula, not run as a test).
* Several combinations produced raw mesh-validation errors ("directed edge appears twice", "normal does not point from owner to neighbour") that the fold check did not catch. They are
  now wrapped into located `mesh` errors with block, indices, layer and the controls; this changed only the error path (A'/B' numbers identical on rerun).
* `front_gap` values of 1 and 2 chords fold and are rejected with a located error (a known limit).
* Q8: the registered rule `n_nose >= n_chord` is implemented as `n_nose + 2 <= n_chord` (cases n_nose = 16 and 15 against n_chord 16 are both tested); `mesh.fan.n_nose` doc text corrected accordingly.
* Q8 front_gap beyond the inlet reports path `z_le_root` (not `front_gap`); the test accepts that path. A fold test and a tangled-family test were added.
* The Q7 B' 1e-12 known-miss test was removed (never registered).
* New ADR text not registered beforehand: D5 control; the S3' corrected test with the shroud flux.
* The mesh construction is not robust (see docs/meshing.md); no performance claim of any kind is made.

### Not committed / unchanged

The pre-existing dirty files (`rcfd-fv/src/lib.rs`, `rcfd-api/src/sim.rs`, ADR 0061, `w30c_*.csv`) were not touched. `baseline_hashes.rs` was not re-recorded; no baseline moves
(the new topology is opt-in, the default stays `h_slit`).
