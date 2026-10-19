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

## Not claimed

Fan performance; any comparison with a published fan; turbulence quantities; the accuracy of the absolute `mrf` formulation next to a stationary faceted
shroud (there the mass flux is zero but the convecting relative flux is not; noted, not changed).
