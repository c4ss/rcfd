# ADR 0072: Wave 34A, the moving-shroud wall leak and the C-grid passage mesh quality

Status: registered before any code of this wave was written or run. Limits are fixed; a miss becomes an `#[ignore]`d `*_known_miss` test with the
limit unchanged, a defective set-up gets a NEW corrected test and the old one stays ignored. Baseline hashes are never re-recorded; anything that could
move a baseline is opt-in. Clean-room: first principles and published literature only; no vendor binary, no other code's source. Parents: ADR 0068 (C-grid
generator and its addendum: misses Q1, Q4, Q5, S3'), ADR 0071 (rotating-frame foundation, `coupled_body_pair_mean`), ADR 0016 (`moving_wall`),
`docs/milestone-fan.md`. Nothing here is a fan performance claim.

## Part 1: the wall flux of a moving wall that is a surface of revolution

### Derivation (done before registering)

The rigid rotation `u = Omega e_z x x` is divergence-free and is the curl of `A = -(Omega/2) r^2 e_z` (`curl(0, 0, psi) = (d psi/dy, -d psi/dx, 0) =
(-Omega y, Omega x, 0)`). By Stokes' theorem its flux through any surface bounded by the closed curve C is `-(Omega/2) oint_C r^2 dz`, a function of C only.
For a straight edge between two nodes on the cylinder r = R, `r^2` along the edge is `R^2 - t (1 - t) c^2` (c the chord length of the edge in the xy-plane),
so `int r^2 dz = dz (R^2 - c^2/6)`, and the exact flux through a facet bounded by straight edges is `(Omega/12) sum_e dz_e c_e^2` (edges in the
circulation order). It vanishes when every edge is axial (c = 0) or circumferential (dz = 0), and when opposite edges are congruent (a uniformly twisted
mesh); for the generic quads of a sheared C-grid it does not vanish. Summed over a periodic ring of facets the exact fluxes cancel (interior edges
twice with opposite sign, seam edges by the rotation, inlet/outlet edges have dz = 0), so a faceted wall that rotates in itself exchanges fluid with
the outside through each facet while the net is zero; the true wall (a surface of revolution moving in itself) has zero normal velocity everywhere.

Read in the code (read-only): `pressure::corrected_flux` sets the flux of every fixed-velocity boundary face to `u_b . S_f`, and `ExprBc::profile`
evaluates `velocity_expression_*` at the face centre. So the solver imposes `phi_f = u_w(c_f) . S_f`, which is neither zero (the impermeable true wall) nor
the exact facet integral above (the centre evaluation of a linear field is exact only on planar polygons). `wall_motion::build` already rejects a
`moving_wall` whose velocity is not tangential to a face within 1e-9, which is why the 33A shroud used `velocity = "fixed"` with expressions.

### Hypothesis and falsifier (run before any fix code)

* **L-H1**: the shroud mass flow of the converged S3' run (ADR 0068, -3.2e-6) is the purely geometric number `G = sum_shroud u_w(c_f) . S_f` with
  `u_w = (Omega y, -Omega x, 0)`, Omega = 20, independent of the flow. **Falsifier F1**: in one run of the registered S3' set-up, compare the monitor
  `m_shroud` with G computed from the same mesh. `|m_shroud - G| <= 1e-12 |m_in|` confirms L-H1; `> 1e-9 |m_in|` falsifies it (then a solver term reaches the
  wall flux and the fix below is not the remedy); in between is reported as inconclusive. The same G is computed on the 0065 H-slit example mesh and
  compared with the 0065 shroud value (1.7e-8 of the flow), reported only.
* **Characterisation (no decision depends on it)**: on the S mesh, the sum over the shroud of the exact facet fluxes by the edge formula (expected
  `<= 1e-13` of the sum of their magnitudes) and the per-face maximum of `|u_w . n| / |u_w|` and of `|u_w(c_f) . S_f - exact facet flux| / |u_w||S_f|`.

### Fix (opt-in)

`boundaries.<patch>.wall_velocity_projection = "tangential"` (default `"none"`, the 1e-9 rejection unchanged) on `velocity = "moving_wall"`: the wall
velocity of each face is replaced by its tangential part `u_w - (u_w . n) n` (n the unit face normal), so the face flux is zero to round-off and the
momentum condition has no normal component. Valid only for a wall that is invariant under its own motion (a surface of revolution rotating about its
axis, a plane translating in itself); it is wrong for walls whose normal motion is real (for example blade walls in the absolute `mrf` formulation). The
largest removed normal fraction is reported. Implementation: a helper in `rcfd-fv` (`bc.rs`) used by `rcfd-api/src/wall_motion.rs`; schema and
`docs/case-schema.md` entry. No default changes.

### Registered checks

* **T1 faceted-annulus Couette (fv level, `crates/rcfd-fv/tests/wall_leak_w34a.rs`)**. Annulus `R_i = 0.5`, `R_o = 1.0`, `L_z = 0.5`, translational
  periodic pair `zmin`/`zmax`, `nu = 0.1`, inertial frame, inner wall at `W_i = 1` (`V_i = 0.5`), outer wall at rest; exact solution `u_theta = A r + B/r`
  (ADR 0071), `u_r = u_z = 0`, zero wall-normal flux. Faceted, non-symmetric nodes: node (i, j, k) at radius `r_j` (uniform), `z_k = k L_z/n_z`, angle
  `theta = 2 pi i/n_t + eps (2 pi/n_t) sigma(i mod 3) sin(2 pi k/n_z)`, `eps = 0.25`, `sigma = (1.0, -0.5, 0.3)` (radial lines straight, end layers
  identical so the pair matches by translation). Meshes `(n_t, n_r, n_z) = (24, 6, 4), (48, 12, 8), (96, 24, 16)`. SIMPLEC, `Central`, `alpha_u = 0.8`,
  `alpha_p = 1`, pressure level pinned (no Dirichlet boundary), converged when `res_u < 1e-10` and continuity `< 1e-9` within 6000 iterations (convergence
  required). Two runs per mesh: wall velocity at the face centre (as the 33A shroud, "unprojected") and projected (the helper of the fix).
  Error `e = max over cells |u_theta - (A r + B/r)| / V_i`; spurious `max(|u_r|, |u_z|) / V_i`; wall leak `max_f |phi_f| / (|u_w| |S_f|)` and
  `sum |phi_f|` over `inner`.
  * Positive control (else set-up defect): unprojected `max_f |u_w . n|/|u_w| >= 1e-4` on every mesh.
  * Projected: wall leak `<= 1e-13` on every mesh; finest `e <= 1.5e-2 V_i` (the M2 limit of ADR 0008/0071); observed order of e `>= 1.0` for each
    refinement; `e_projected <= e_unprojected` on every mesh. If the last criterion misses, the projection is recorded as a bookkeeping change of the wall
    flux, not as an accuracy fix. Spurious speed reported for both.
* **T2 empty C-grid sector Couette (API level, `crates/rcfd-api/tests/wall_leak_w34a.rs`)**: the D2 set-up of ADR 0068 unchanged (empty C-grid sector,
  hub at 10 rad/s, shroud at 20 rad/s, inlet with the exact profile, stationary frame), once with the 33A `fixed` + expression walls and once with
  `moving_wall` + `wall_velocity_projection = "tangential"`. Error `max |u_theta - (A r + B/r)| / (20 * 0.31)` over cells. Gates: projected hub and shroud
  per-face leak `<= 1e-13`; `e_projected <= e_unprojected`; both converged (4000 iterations, tolerances 1e-12 as D2).
* **S3'' (API level)**: the S3' set-up of ADR 0068 (Omega = 20, rotating frame) with the shroud as `moving_wall`, `wall_rotation_rate = -20`,
  `wall_velocity_projection = "tangential"`. Gates (registered limit of S3' unchanged): converged within 6000 iterations; `|m_in + m_out| <= 1e-6 |m_in|`;
  `|m_shroud| <= 1e-14 |m_in|`; `|m_lo + m_hi| <= 1e-12 |m_lo|`.
* **T3 default unchanged**: a `moving_wall` with a non-tangential velocity and no projection key is still rejected with the 33A message.

## Part 2: C-grid mesh quality

### Diagnostics on the 33A construction (run before any construction change; measurements, no fix)

`CGridMesh` gets the per-cell tag (block, two in-plane indices, layer) that the builder already computes, as a public field (no behaviour change).
Test file `crates/rcfd-mesh/tests/cgrid_w34a.rs`; set-ups S, A', B' of ADR 0068 exactly.

* **G1 attribution**: for S, A', B' the 20 faces of largest non-orthogonality and of largest skewness, each with block, region, (i, j, k), face family
  (i-face, j-face, k-face) and internal/boundary (with patch). Regions of the C block: `wake` (i < n_down or i >= n_i - n_down), `suction`
  (n_down <= i < h_c), `nose` (h_c <= i < n_i - h_c), `pressure` (n_i - h_c <= i < n_i - n_down); each split into `stack` (j < n_bl) and `outer`. Plus `F` and
  `core`. Counts of faces above 70 deg per region and family. Note: `quality_summary` (the gate) measures boundary faces from the owner centre to the face
  centre, so seam and wall faces count.
* **G2 planar control (curvature off)**: the same node set with every node `(z, s, r)` of layer k placed at the Cartesian point `(r, s, z)` (s the
  unrolled coordinate of that layer), same topology. The difference to the curved mesh is the effect of curvature (facets, warp of sheared quads, sagitta).
* **G3 in-plane control (curvature and the radial variation off)**: per layer, the 2-D non-orthogonality of every in-plane edge of the C and F blocks
  (angle between the edge normal and the line joining the 2-D centroids of the two quads), maximum over layers.
* **G4 untwisted blade**: blade B with both stations equal to the mid-span station (no twist, no lean, no sweep, constant chord); S, A' counts.
* **G5 front gap**: S with `front_gap` 0.2 and 0.4 (single factor; a fold is recorded as such).

Hypotheses and their falsifiers (registered with the diagnostics):

* **N1** the non-orthogonality maximum is set by the in-plane construction: falsified for a set-up if G3's maximum is more than 5 deg below the curved
  maximum (then curvature or the radial variation drives it; G2 and G4 split the two).
* **N2** the oblique nose fan drives it: falsified if the face of maximum non-orthogonality of A' or of B' is not in the `nose` region or if fewer
  than half of the faces above 70 deg of A' are there.
* **N3** grazing pressure-side lines drive it: falsified if fewer than a quarter of the faces above 70 deg of A' are in `pressure`.
* **K1** the B' skewness is set by the curvature of the thin hub/shroud/tip layers (warp of a sheared quad, about (in-plane size x shear)/R, against
  h1 = 7.3e-6 m): falsified if G2's B' skewness exceeds 2.0, or if the worst skew face of B' is not within n_bl layers of the hub, the tip or the shroud.
  If K1 holds and G2 is below 2.0, the skewness is a geometric floor of the flat-faced hex at these counts; the construction change below may then not
  remove it and that is recorded, not fought by changing the registered counts.

### Construction change (opt-in; specified in an amendment written after G1-G5 and before any A'/B' run of it)

The new construction is selected by a new field of an options struct (or a new entry point) so that `build_cgrid_mesh` with the 33A construction stays
the default and its tests (including the Q8 fold tests) are untouched; case key `mesh.fan.c_grid_construction` (default the 33A construction). It is
developed on S and on the development counts named in the amendment only; A' and B' are run once each after the construction is frozen. Candidate
(principled, chosen by the attribution): elliptic (Winslow-type) smoothing of the C and F blocks per layer with the boundary-layer stacks frozen (Q7 exact),
the seams treated as periodic interior lines (the same displacement applied to both copies so Q6 stays exact), the front edge an interior interface, inlet
and outlet nodes sliding in their planes; or orthogonalised wall-normal lines; or curved stacks.

### Registered acceptance for the new construction (limits as ADR 0065/0068, not relaxed)

* **Q1-E** A': non-orthogonality <= 70 deg, skewness <= 2.0, aspect <= 300; B': <= 70 deg, <= 2.0, <= 3000 (`quality_summary`).
* **Q2-E, Q3-E, Q6-E, Q7-E**: the 0068 Q2, corrected Q3, Q6 (1e-12) and Q7 (A' 1e-12, B' 4 eps 0.4/h1) checks unchanged on S, A' (Q7 also B').
* **Q4a'-E (corrected set-up of Q4a)**: Q4a's third level has n_nose = n_chord = 16, outside the rule n_nose + 2 <= n_chord whatever the construction
  (a set-up defect of 0068, recorded now). Corrected levels, empty passage, all in-plane counts scaled together: (n_up, n_chord, n_down, n_side, n_nose) =
  (8, 16, 8, 8, 4), (16, 32, 16, 16, 8), (32, 64, 32, 32, 16), the other S counts. Limits of Q4a: envelope error strictly decreasing, finest <= 5e-3,
  order between the last two >= 1.5. Also run with the 33A construction (a new test; a fold is its result).
* **Q4b/Q5-E**: the registered Q4b/Q5 levels (n_chord, n_span) = (16, 8), (32, 16), (64, 32) on S, limits unchanged (finest <= 2e-3, order >= 1.5,
  strictly decreasing), with the new construction.
* **R-E robustness scan** (each combination: builds without error, `Mesh::validate`, all volumes > 0; quality reported, not gated): on S counts
  `front_gap` in {0.2, 0.3, 0.5, 1.0, 1.5, 2.0}; `n_nose` in {2, 3, 4, 6, 8}; `n_side` in {6, 8, 10, 16, 32}; `n_chord` in {12, 16, 24, 32, 64}; and the grid
  n_chord {16, 24, 32} x n_nose {2, 4, 6, 8} x n_side {6, 10, 16, 32} x blade {with, without} (96 combinations; n_nose + 2 <= n_chord holds for all).
  Criterion: every combination builds. The same lists are run with the 33A construction and the build count reported (not gated).
* **R1-E**: the example case built with the new construction raises no error-level rule finding.

Decision rule: A' and B' are run once each with the frozen construction; a measured value beyond a limit becomes an ignored `*_known_miss` test with the
limit unchanged. No re-run with another construction to pick a better value; a construction change after an A'/B' measurement is disclosed in the
addendum and its numbers are reported separately as unregistered.

## Amendment 1 (written after F1 ran and before any G diagnostic ran; no limit changed)

* **G2 as registered is not a valid control** (pencil, before running): mapping layer k to the Cartesian `(r, s, z)` with `s = r theta` turns the radial
  node lines (constant theta) into lines of slope `ds/dr = theta`, up to 2 pi / N = 1.26 rad across the passage, so the planar mesh would ADD a radial
  tilt of up to about 50 deg instead of removing the curvature. No planar map keeps both the in-plane metric of every layer and the radial lines. G2 is
  replaced by two single-factor runs of the first cell height, which is the factor named by K1 (facet warp against h1): **G2a** = B' counts with the
  first cell height of A' (y+ 30), **G2b** = A' counts with the first cell height of B' (y+ 1). K1 is falsified if G2a's skewness exceeds 2.0 or G2b's does not.
  N1 keeps its G3 criterion; G4 (untwisted blade) separates the radial variation of the in-plane grid from the rest of the 3-D effects.

## Amendment 2 (written after F1 and G1-G5 ran; before T1, T2, S3'' and any construction change ran; no limit changed)

### Results so far (recorded now; the addendum repeats them)

* **F1**: S3' converged (406 iterations); `m_shroud = -3.2216608166e-6`, `G = -3.2216608166e-6`, `|m_shroud - G| / |m_in| = 0` (bit-identical): **L-H1
  confirmed**, the shroud flow is the geometric face-centre wall flux, no solver term reaches it. `G / m_in = 6.0e-5`. Characterisation: the sum of
  the exact facet fluxes is -3.1e-16 against a sum of magnitudes of 2.4e-4, ratio 1.3e-12 (registered expectation was <= 1e-13: above expectation; the edge
  formula subtracts terms of size R^2 dz whose sum is c^2 dz/6 smaller, a cancellation of about R^2/c^2 = 1e2 to 1e3 at round-off, so 1e-12 is its
  floor); the per-face facet flux is not small: the sum of magnitudes is 0.45 % of the throughflow, the largest `|u_w . n| / |u_w|` is 1.0e-2 and the centre
  rule differs from the exact facet flux by up to 1.7e-3 `|u_w||S|`. The H-slit example mesh: G = -1.668e-8 (the 0065 value 1.7e-8 reproduced), largest
  tilt 1.7e-3.
* **G1/G3/G4/G5** (33A construction): S 79.65 deg (G3 in-plane 79.71), A' 81.57 (81.65), B' 83.73 (78.82); faces above 70 deg in the `nose` region: S
  256 of 288, A' 2662 of 2887 (92 %), B' 8771 of 10215; `pressure` A' 145 of 2887. B' maximum: a `hub` boundary face of the first layer in the wake
  (k = 0, j = 18), owner-to-face-centre angle 83.7 deg. B' skewness 4.85 on the wake cut (all layers); faces above skewness 2 in B': wake-stack j-faces
  6477, cut faces 768, k-faces (F, wake/outer, pressure/outer, nose) 1258. G4 (untwisted blade): S 78.34, A' 79.44 deg (in-plane 78.40, 79.46). G5:
  front_gap 0.2 gives 82.12, 0.4 gives 86.67 deg on S (both worse). G2a (B' counts, A' first cell) folds in the leading-edge fan (located error),
  so it is inconclusive. G2b (A' counts, B' first cell): 87.67 deg (F-block hub boundary faces at k = 0), skewness 12.68 (k-faces), aspect 5299.
* **Verdicts**: N1 holds for S, A' and B' (B' just: 4.9 deg); N2 holds for S and A', falsified for B' (its maximum is not in the nose region); N3 falsified
  (5 % in `pressure`); K1 as registered falsified (the worst skew face of B' is the wake cut, in every layer, not an end-wall layer). The strong h1
  dependence of G2b is a new finding, treated below as W1, not as a rescue of K1.

### New mechanisms (pencil derivations from a probe of the worst cells; registered with falsifiers before any fix runs)

* **W1 thin-layer rotation wedge.** The in-plane grid of every layer is built independently, so a node moves in angle between layers at the rate
  `d theta/dr` (probe: 3.7 rad/m at the F-block hub corner of G2b, where the frame follows the blade's centre angle). Rotating the upper facet of a cell
  about the axis by `d theta` tilts its plane by `d theta` against the lower facet, so the gap between them varies along an in-plane edge of length L by
  `d theta L`: the relative thickness variation is `(d theta/dr) L`, independent of the layer thickness `Delta r`. The cell (and its end faces) is then a
  thin wedge whose centroid moves along L by about `(d theta/dr) L^2 / 6`, against a half thickness `Delta r/2` (probe: L = 0.017 m, a 6 % wedge, a
  9e-5 m shift against 3.7e-6 m: the 87.7 deg). The angle error is about `atan((d theta/dr) L^2 / (3 Delta r))`: harmless in thick layers, large in the
  end-wall stacks when h1 is small. Prediction: if the angles of the nodes outside the blade stacks change between layers by `Delta theta_k` proportional
  to `Delta r_k^2` (no rotation in the thin layers) the k-face skewness and the end-wall boundary-face non-orthogonality fall below the limits.
* **W2 wake first-height wedge.** The wake stacks use `d1 = max(h1, dz/300)` (a post-registration change of 33A), so neighbouring wake lines have
  different first heights (ratio of the axial cell growth) and every wake-stack cell is a wedge whose centroid moves along the wake by about
  `L (Delta d1/d1)/6`, many times its thickness; the stack is also switched off on some wake lines. Prediction: one first height h1 and the full stack on
  every wake line remove the wake-stack and cut-face skewness above 2.

Development set-ups for these (named now): **DS** = S counts with `first_cell_height = 7.31e-6` m (the B' h1) and `n_bl = 3`; **DA** = G2b (A' counts, B'
first cell), which reproduces both mechanisms (measured above). Falsifiers, run before the construction is frozen:
* **W1** falsified if, on DA, a graded angle remap of the nodes outside the blade (and wake) stacks (single change to the 33A construction) does not bring
  the maximum non-orthogonality of k-faces and of hub/shroud boundary faces under 70 deg and the k-face skewness under 2.0.
* **W2** falsified if, on DA, one first height h1 with the stack on every wake line (single change) does not bring the wake-stack j-face and cut-face
  skewness under 2.0. Aspect is re-measured with it (33A introduced the wake rule against the aspect ratio).

### T1 set-up defect (found by a geometry-only computation before T1 ran)

T1 is closed (walls and a periodic pair, no Dirichlet pressure). The unprojected arm prescribes `u_w(c_f) . S_f` on `inner`, whose net is not zero on
warped facets (F1): -4.8e-7, -1.3e-8, -4.3e-10 on the three meshes, i.e. a solver continuity measure of 1.2e-5, 1.2e-6, 1.6e-7 that the registered
`continuity < 1e-9` can never reach (the pinned reference cell carries the imbalance). Corrected (the registered test stays, ignored, if it fails for
this reason): the unprojected arm is converged when `res_u < 1e-10` and the largest velocity change of an iteration is below `1e-12 V_i` (within 6000
iterations), its continuity reported; the projected arm keeps the registered criterion (`res_u < 1e-10`, continuity `< 1e-9`). All other T1 gates unchanged.

## Amendment 3 (written after T2, S3'', T3 and the two coarser T1 meshes ran, before the finest T1 mesh finished; no limit changed)

Measured so far: S3'' converged in 406 iterations, `|m_in + m_out| / |m_in| = 1.5e-14`, `|m_shroud| / |m_in| = 5.6e-17`, pair 6.0e-16 (pass); T2 both
arms converged, error 0.27329 (unprojected) and 0.27328 (projected) of `W2 R2` (the coarse empty sector with upwind is far from the profile in both),
projected leak 1.8e-16 (pass); T3 pass. T1 corrected, meshes 1 and 2: the projected error is ABOVE the unprojected one (3.6187e-2 vs 3.6000e-2,
1.0171e-2 vs 1.0154e-2 V_i), so the registered T1 criterion `e_projected <= e_unprojected` is expected to miss; by the registered rule the projection is then
a bookkeeping change of the wall flux (it removes the leak exactly, it does not make the solution more accurate on T1).

Reading of this (pencil): the projection changes two things at once, the mass flux (now zero) and the no-slip velocity (now tangential to the facet
instead of equal to the true wall velocity at the face centre, an `O(tilt)` change of the momentum condition). A wall that is impermeable AND moves with
the true wall velocity is the **decoupled** treatment: zero mass flux through the facet, the face-centre wall velocity kept in the momentum condition.
Registered now as a further opt-in (`SimpleSolver::impermeable`, a per-boundary-face mask, default `None`, every earlier path bit-identical): the boundary
values that carry the mass flux (`boundary_hby_a`, `corrected_flux`) use the tangential part on masked faces, the momentum matrix keeps the prescribed
value. **T1d** (new test, same meshes, set-up and solver as T1): unprojected arm (corrected criterion) against the decoupled arm (registered criterion
`res_u < 1e-10`, continuity `< 1e-9`); gates: converged, decoupled wall leak `<= 1e-13`, finest decoupled error `<= 1.5e-2 V_i`, orders `>= 1.0`, decoupled
error `<=` unprojected on every mesh. If that last gate misses as well, neither treatment improves T1 accuracy and the decoupled treatment is recorded
as a bookkeeping change too. Its API exposure is decided only after T1d (it needs a hunk in `sim.rs`, a shared file).

## Not claimed

Fan performance; any comparison with a published fan; turbulence quantities; the accuracy of the absolute `mrf` formulation next to a stationary faceted
shroud (there the mass flux is zero but the convecting relative flux is not; noted, not changed).
