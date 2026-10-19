# ADR 0079: Wave 35D, a C-grid mesh of the FAN-01 passage, the B' quality mechanisms, and the control range of the C-grid generator

Status: registered before any code of this wave was written. Limits are fixed; a miss becomes an `#[ignore]`d `*_known_miss` test with the limit
unchanged, a defective set-up gets a NEW corrected test and the old one stays ignored. Baseline hashes are never re-recorded; every change is opt-in.
Clean-room: first principles and published literature only. Parents: ADR 0072 (construction E and its addendum: A' aspect 395 against 300, B' 78.17 deg /
skewness 7.08 / aspect 8532, R-E scan 107 of 117, median 74.5 deg), ADR 0075 (FAN-01 geometry and the refusals of attempt 1), ADR 0065/0068 (gates).
Nothing here is a fan performance claim.

## What was run before this ADR was written (disclosed)

All on HEAD 0cef8f5 unmodified, in a scratch copy, release builds:
* FAN-01 with the ADR 0075 amendment 1 C-grid counts and the y+ = 30 rule: both `pursuit` and `smoothed` refuse with `mesh.fan.n_down`: "the wake line
  (s from -0.0683 to -0.0862) leaves the passage (-0.0774, 0.0092) [layer k=0 (r = 0.12400)]" (the 34D result reproduced).
* A' and B' with construction E rebuilt (timing): A' 14 s, B' 72 s; B' 593 250 cells, 78.17 deg, skewness 7.084, aspect 8531.6 (the 0072 numbers
  reproduced); A' aspect 395.06.
* Smoke of the laminar set-up of P10 below on the ADR 0075 attempt-2 H-slit mesh (13 332 cells): 300 iterations, all finite; continuity 8.21e-5 at
  iteration 10, maximum over 200-300 6.06e-5 (stable by the ADR 0075 amendment 3 criterion); largest relative speed 47.1 m/s (tip speed 38.9 m/s).
* Grid hashes (FNV-1a over the f64 bits of every node, the face node lists, owner/neighbour and the patch names/ranges; x86_64-pc-windows-msvc) of the
  current constructions, recorded for check B1: S pursuit `0xeebcf5467f718bf0`, S E `0xde01897c36a13987`, A' pursuit `0x3247673c05cd65bf`,
  A' E `0x35359c0db0e47e3a`.
* Arithmetic, no run: with the y+ = 30 rule (h1 = 5.65e-4 m) the present gap rule (`graded_two_sided(0.0025, 7, h1, 1.2, 2)`) does NOT refuse; it leaves
  1.3e-5 m for the three middle gap cells (about 4e-6 m each, next to 0.68 mm cells): a degenerate gap block, not a refusal.

## Part 1: a mesh of the FAN-01 passage

### Diagnosis (from the refusal and the geometry)

The C-grid centres two straight axial seams (`s = const`) on the mean of the leading- and trailing-edge `s`. At the FAN-01 hub (chord 98.9 mm, stagger
43.6 deg, pitch 86.6 mm) the blade spans 68 mm of the pitch, the trailing edge is 9.3 mm from the lower seam and the wake line (leaving along the
pressure-surface tangent, about 42 deg, turning axial over 0.2 chord) drifts 18 mm further: it crosses the seam. With straight seams no centring fits
blade and wake (68 + 18 mm of 86.6 mm). The tip gap (2.5 mm) cannot hold two stacks of 2 cells of the y+ = 30 height plus a middle (above).

### Changes (opt-in fields of `CGridOptions`, default `false`; every existing path bit-identical)

* **`camber_seam`**: per layer a passage centre line `C(z)`: the camber line of the section for `z_le <= z <= z_te` (sampled at 256 cosine-spaced
  chord points, linear in z; a camber line whose z is not strictly increasing is a located error `mesh.fan.stagger`); for `z > z_te` the wake curve of the
  construction itself (so the wake lies mid-passage by construction); for `z < z_le` the C1 extension `s_le - sigma_le lambda (1 - exp(-(z_le - z)/lambda))`
  with `sigma_le` the camber slope ds/dz at the leading edge and `lambda = 0.2 c`. Seams `s_lo(z) = C(z) - P/2`, `s_hi(z) = s_lo(z) + P` (so the upper seam is
  the lower one translated by the pitch P, which is all the periodic machinery and the smoother assume). Every place that uses the straight seam uses the
  z-dependent value: the blade-fit check (each surface node), the wake-fit check (each wake node), the wake-stack seam distance, the seam targets
  `(z_out[h], s_lo(z_out[h]))` (the `z_out` rule unchanged), the corner points `(z_f, s_lo(z_f))`, the front-edge distribution (about `C(z_f)` instead of
  `s_le`) and the F block (row `a` shifted by `C(z_a) - C(z_f)`). This is the seam of the H-slit generator (camber line inside the chord) extended C1 and
  carried along the wake.
* **`gap_fit`**: tip-gap first height `h_g = min(h1, L_g / (2 S + (n_gap - 2 n_bl) g^n_bl))`, `S = sum_{q < n_bl} g^q`: the largest height for which
  two exact stacks and a middle of cells `h_g g^n_bl` fill the gap with continuous growth. The gap radii are `graded_two_sided(L_g, n_gap, h_g, g, n_bl)`;
  the span keeps `h1`. Identity when `h_g = h1` (A', B', S, the example).
* **Hub**: no change registered now. Diagnostic DH (on the development counts, below) decides: build or fold; the end-wall first-layer (k = 0 hub faces
  and k-faces of the first `n_bl` layers) maximum non-orthogonality and skewness, the k-face maxima, and where they sit. A hub change, if any, is
  specified in an amendment before the registered FAN-01 build.

Construction **F** = construction E + `camber_seam` + `gap_fit` (+ the hub change of an amendment, if any). Case key: a new value
`mesh.fan.c_grid_construction = "camber_seam"` (schema and `docs/case-schema.md`); after Part 2 the same value selects construction **G** = F + the
accepted Part 2 changes. `pursuit` and `smoothed` are unchanged.

Development protocol: Part 1 is developed only on the FAN-01 geometry with the development counts **FD** (n_up 10, n_chord 20, n_down 10, n_side 12, n_nose 5,
n_core 1, front_gap 0.3, n_span 12, n_gap 6, n_bl 2, growth 1.2, y+ = 30 rule) and on blade B with the S counts. The registered counts are not built with
F until F is frozen in an amendment. Before freezing, measured on FD: the angle between the C lines and the seam where they meet it (if a large angle
appears, the target rule may be changed in that amendment, before the registered run).

### Registered set-up FAN-01-C

Geometry: `n_blades = 9`, `r_hub = 0.124`, `r_tip = 0.2475`, `tip_clearance = 0.0025`, `axial_extent = 0.4`, `z_le_root = 0.1`, NACA 4-digit, stations =
the 16 sections registered in ADR 0075 amendment 1, read from `validation-data/reference/w34d_fan01_cad_sections.csv` (the row r = 247.0 mm is not one of
them, as in the 34D test), sweep `x_LE(r) - x_LE(124)`, lean `r (theta_LE(r) - theta_LE(124))`. Counts: the ADR 0075 amendment 1 C-grid counts (n_up 8,
n_chord 16, n_down 8, n_side 10, n_nose 4, n_core 1, front_gap 0.3, n_span 14, n_gap 7, n_bl 2, growth 1.2), first cell from the y+ = 30 rule at
`ref_speed = hypot(1.4032 / A, Omega 0.186) = 30.46` m/s, `nu = 1.5e-5`. Built through the case (`c_grid_construction = "camber_seam"`).

Measured twice: **(a)** with F, once it is frozen, and **(b)** with G after Part 2 (so (b) is not a re-run chosen for a better number; both are reported).

Checks (each on (a) and (b)):
* P1 builds, `Mesh::validate`, all volumes > 0; `construction_step` reported.
* P2 quality, the ADR 0072 Q1-E A' gates as the target (y+ = 30 like A'; not tighter): non-orthogonality <= 70 deg, skewness <= 2.0, aspect <= 300
  (`quality_summary`).
* P3 closure `max |sum S_f| / sum |S_f| <= 1e-13`, every internal face in two cells, every boundary face in one.
* P4 rotational periodicity (the Q6 test of ADR 0068/0072, angle 2 pi / 9): seam-node rotation mismatch, seam point bijection, face-centre mismatch and
  `|S_hi + R S_lo| / |S_lo|`, all <= 1e-12.
* P5 exact stacks: blade wall stacks along the defined normals (distance and transverse) <= 1e-12 relative; radial stacks at the hub and below the tip
  `h1 g^q`, and in the gap above the tip and below the shroud `h_g g^q`, <= 1e-12 relative.
* P6 independent volume check (Q3 of 0072): C nodes of the bladed and the empty build identical; `|V_passage + V_solid - V_envelope| / V_envelope <= 1e-11`.
* P7 blade wall nodes on the exact surface: every wall node of layers 0..n_span within 1e-12 m of `AxialFan::surface_point` at its radius and chord station.
* P8 solid volume against the Gauss quadrature of the section area over the span: reported (the n_chord 16 mesh is coarse; 0072 Q4b measured 1.2e-2 there).
* P9 `FvGeometry::new` accepts the mesh; `check_mesh(..).is_ok()`; `rules::check_case` raises no error-level finding (warnings listed).
* P10 laminar smoke solve: the FAN-01 design point in the rotating frame (`rotating_frame`, Omega = 155.614 rad/s about +z, centrifugal), air `nu = 1.5e-5`,
  `rho = 1.2`, inlet `u = (Omega y, -Omega x, 9.478)`, outlet fixed pressure, hub and blade at rest in the rotating frame, shroud at rest in the absolute frame
  (fixed with expressions), laminar, SIMPLEC, `upwind`, 300 iterations. Bounded = `failure()` empty, continuity, res_u and res_p finite at every iteration,
  and the maximum continuity over iterations 200-300 not above its value at iteration 10 (the ADR 0075 amendment 3 criterion). Reported: the largest
  relative speed and `m_in + m_out`. The set-up was smoke-tested on the H-slit mesh (above).

## Part 2: the B' mechanisms, one at a time

Baseline: construction E (ADR 0072 Amendment 4) on blade B, set-ups A' and B' of ADR 0072, whose numbers are known (above).

### Diagnostic D1 (run first; measurements only)

For A' and B' with E: the 20 faces of largest non-orthogonality and of largest skewness and the 10 cells of largest aspect, each with block, region (the
0072 G1 regions), (i, j, k), face family and patch; counts above 70 deg / 2.0 per region and family. For each of those faces a geometric decomposition:
the tilt of the node lines between the two layers (physical angle of `(dz, r dtheta)` against `dr`), the wedge of its cells (relative thickness
variation of the cell across its in-plane extent), the face warp (largest distance of a face node from the plane through the face centre with the face
normal, over the face size and over the owner's thickness), and for wake lines the first-height ratio between neighbouring lines.

### Mechanisms and their falsifiers (decided on D1)

* **M1 hub first-layer wedge**: the B' non-orthogonality maximum is on an end-wall boundary face or a k-face within `n_bl` layers of the hub or the
  shroud. Falsified if the B' maximum is elsewhere (then M1 targets the faces above 70 deg in those layers, if any, else it is dropped).
* **M2 wake-cut skewness**: the B' skewness maximum is on a wake-cut face or a wake-stack j-face. Falsified if not. Prediction registered now: the wake
  faces are thin cells on a surface that turns with r, so their skewness tracks (face warp + wedge shift) / thickness, not the first-height ratio alone
  (ADR 0072 W2, one first height everywhere, made it worse).
* **M3 warped blade-stack faces**: prediction (pencil, before D1): the warp of a quad between two consecutive section chords is about
  `L Delta_r dphi/dr` (phi the in-plane angle of the surface), set by the twist of the surface and NOT by where the wall nodes sit along the chord; a
  twist-following re-placement of the wall nodes therefore cannot remove it and can only change the shear of the radial lines. Falsified if D1's
  decomposition shows the stack-face skewness dominated by shear rather than warp.
* **M4 A' aspect at the irregular seam corner** (0072 unregistered observation: the corner cells in the first layers below the shroud and the tip cap).

### Candidate changes (families named now; the exact change of each is specified in Amendment 1, written after D1 and before any run of a change)

C1 for M1: exactly radial node lines in the end-wall stack layers outside the blade wall stack (the far grid and the wake take the (z, theta) of the
layer at the edge of the end-wall stack, blended into the true section near the blade as in W1). C2 for M2: a wake first height continuous with the
blade's and thick against the face warp, or no thin stack in the wake (the wake is not a wall). C3 for M3: planar faces: the warped j-faces of the
blade stack split into two planar triangles each (a polyhedral cell; `Mesh`, `FvGeometry` and the metrics are checked on such cells first); or a
twist-following placement if D1 falsifies the warp prediction. C4 for M4: the seam-corner cells of the end-wall layers (e.g. the smoother's corner rule,
the end-wall first height at the corner). Each change is a new opt-in field of `CGridOptions`.

### Decision rule (registered; the 10 % and 1 % are decision thresholds of this protocol, not accuracy limits)

Order M1, M2, M3, M4. Each change is applied on top of the current baseline (E plus the changes accepted so far) and measured once on A' and B'. It is
**accepted** iff (i) its target measure (M1: the maximum non-orthogonality over end-wall boundary faces and k-faces of the first `n_bl` layers of B';
M2: the maximum skewness over wake-cut and wake-stack faces of B'; M3: the maximum skewness over blade-stack faces of B'; M4: A' maximum aspect)
improves by at least 10 % against the baseline; (ii) no Q1-E maximum (non-orthogonality, skewness, aspect) of A' or B' increases by more than 1 %;
(iii) Q2-E closure (1e-13) and topology on S and A', Q6-E periodicity (1e-12) on S and A', Q7-E stacks (A' 1e-12, B' 4.9e-11) pass, and S, A', B' build;
(iv) the R-E scan of 0072 builds at least 107 of 117. A rejected change is removed from the code (the ADR keeps its numbers). The Q1-E gates of A' and B'
are reported for the final construction (G restricted to blade B, i.e. E plus the accepted changes, and G itself); a gate still missed stays a known miss.

## Part 3: the control range of construction G

* Development scan (unregistered combinations, blade B and FAN-01): the build failures and poor meshes are related to the controls; the valid range is
  then registered in an amendment as a few inequalities on the controls and their ratios (for example `n_side / n_chord`, `n_nose / n_chord`,
  `front_gap`), written before the registered scan runs.
* Located errors: with `c_grid_construction = "camber_seam"` only (no new rejection on `pursuit` or `smoothed`), a specification outside the range is
  refused before any construction, with the path of the offending control and the range in the message.
* Registered scan R-G (combinations different from the development scan, fixed in that amendment), on blade B (first height 1e-3 as S) and on FAN-01
  (y+ = 30 rule): gates (a) every in-range combination builds and passes `Mesh::validate` with all volumes > 0: 100 %; (b) every out-of-range combination
  of the scan is refused with a located error naming a control: 100 %. Reported, no limit: the fraction of in-range combinations that build AND meet the
  A' gates 70 deg / 2.0 / 300, the median non-orthogonality, and the distribution of `construction_step` (a step-5 fallback is nearly algebraic).

## Part 4: default behaviour bit-identical

* B1: the hashes above of S and A' with `pursuit` and with E are reproduced exactly after every change of this wave; the FAN-01 refusal message of the
  default construction is reproduced exactly.
* B2: `crates/rcfd-api/tests/baseline_hashes.rs` untouched and passing; the 0072 tests pass unchanged.

## Not claimed

Fan performance; any comparison with measured FAN-01 data; turbulence; accuracy of the smoke solve (bounded only).

## Amendment 1 (written after D1 and two probes ran, before any Part 2 change was written or run; no limit changed)

### D1 results (construction E; test `d1_attribution_of_construction_e`)

* A': 61.38 deg (seam_hi faces of the pressure/outer cells at mid-span, in-plane), skewness 0.863 (core), aspect 395.1; no face above 70 deg or 2.0.
  Aspect top 10: the seam-corner cells C (h_c - 1, n_side - 1) and F (n_up - 1, 0) in layers 47 (shroud), 30 and 29 (tip).
* B': 78.17 deg, skewness 7.084, aspect 8531.6. Faces above 70 deg: 84 blade boundary faces of the suction/pressure stack next to the trailing edge
  (i = 40-44 and 133-135, k = 17, r = 0.115) and 90 nose-stack j-faces (line i = 88, the leading edge, between the last stack row and the first outer row,
  layers 52-63 around the tip). Faces above skewness 2: wake-stack j-faces 6477, wake-cut faces 768, nose/outer j-faces 270, suction/stack j-faces 4.
  The skewness maximum (7.084) is a nose/outer j-face (line 88, rows 8/9, mid-span); the wake-cut maximum is 4.847 (k = 60), the blade-stack maximum
  2.057 (the trailing-edge cells above). Largest non-orthogonality of an end-wall face (hub/shroud boundary or k-face in the first or last `n_bl` layers):
  64.15 deg (the shroud face of the seam-corner cell). Aspect: the seam-corner cells of layers 89, 60, 59.
* Decomposition of the worst faces: blade trailing-edge cells (78.17 deg): node-line tilt 34.5 deg, face warp 0.02-0.05 of the thickness, in-plane wedge
  0.04, shear of the thin pair 0.65, corner-Jacobian variation 2.7 %. Wake-cut faces: in-plane Jacobian variation 21.6 %, no tilt, no warp. Nose/outer:
  in-plane wedge 2.9 (the first outer row 6.6e-6 m thick against 3.1e-5 m in the algebraic grid), Jacobian variation 108 %.

Probes (scratch, not committed): (1) the 78.17 deg cell integrated as a trilinear hex (8-point Gauss) gives 78.74 deg and the same volume (1.9844e-11):
the flat-face centroid rule is NOT the cause (the 0072 Amendment 4 reading does not hold for construction E). The stack is built along the in-plane
normal, but the trailing edge leans by lambda = 34 deg out of the layer (the surface normal has a radial component of 0.54), so the wall-normal thickness
is `h1 cos(lambda(r))`; it changes by 2.5 % between the two layers of the cell (5.9-6.2 against 6.1-6.3 micrometres), and a 2.5 % thickness change over
a radial length 1100 times the thickness moves the centroid along the face by `L epsilon / 12` = 1.4e-5 m, 4.7 half-thicknesses: 78 deg. (2) The
algebraic grid of E (smoothing off) has A' aspect 177 and B' 2656 (both inside their limits) and B' skewness 4.847 (the wake cut): the smoother doubles
the seam-corner edges (A' F-corner b-edge 39 to 87 mm, C-corner last cell 20 to 40 mm; B' 19 to 62 mm) and compresses the first outer row at the nose
(B' 3.1e-5 to 9.2e-6 m), which makes the B' skewness maximum and the 90 nose-stack faces above 70 deg.

### Verdicts of the registered falsifiers

* M1 (hub first-layer wedge): **falsified**; no end-wall face is above 70 deg (64.15 at most), so by the registered rule M1 is **dropped** (W1's
  remap of construction E already removed it).
* M2 (wake): **falsified as the location of the maximum** (the B' maximum is a nose/outer face); the wake still holds 7245 faces above 2.0, and the
  registered prediction holds: the wake skewness is set by the in-plane wedge between neighbouring wake lines (W2: `d1 = max(h1, dz/300)` grows with the
  axial spacing, ratio epsilon = 0.2 per line), with `skew ~ (L / d1) epsilon / 12` (a trapezoid's centroid shift `L epsilon / 12` over the distance `d1`
  between the mirror cells across the cut): 300 x 0.216 / 12 = 5.4 against 4.85 measured.
* M3 (warp): **falsified**; warp is 2-5 % of the thickness, the maximum is the lean-induced radial thickness variation (M5 below).
* M4 (aspect at the seam corner): **confirmed** for A' and B'; the smoother is the cause (probe 2).
* New, found by D1: **M5** lean of the trailing edge with a stack along the in-plane normal (probe 1); **M6** compression of the first outer row at the
  nose by the smoother (B' skewness maximum and the 90 nose-stack faces above 70 deg).

### The single-factor changes (new opt-in fields of `CGridOptions`), order C2, C5, C6, C4

* **C2** (`wake_aspect_50`): the wake first height `d1 = max(h1, dz / 50)` instead of `dz / 300` (one constant). Derivation: with the measured
  epsilon <= 0.3 between neighbouring lines, `skew ~ 50 x 0.3 / 12 = 1.25`. Target measure as registered for M2 (B' maximum skewness over wake-cut and
  wake-stack faces, 4.847). Prediction: below 1.5.
* **C5** (`lean_stack`), replacing C3 (the named C3 families, planar triangles and twist-following placement, act on warp and shear, which D1 shows
  are not the cause; this is a departure from the registered candidate list, disclosed here before any run): every blade wall stack (and the wake
  stack, which continues it) is built with the in-plane offsets divided by `cos(lambda)`, lambda the angle between the in-plane wall normal and the 3-D
  surface normal at that wall node (surface tangent along the radius by a central difference of `AxialFan::surface_point` over `1e-6 r`; the wake
  lines take the trailing-edge value), so that the WALL-NORMAL thickness of every stack cell is exactly `h1 g^q` in every layer. Target measure as
  registered for M3 (B' maximum skewness over blade-stack faces, 2.057); also reported: the B' maximum non-orthogonality of blade-stack faces (78.17).
  Q7 under C5 (departure from decision-rule item (iii), registered now): the in-plane stack distance is `h1 g^q / cos(lambda)` by design; Q7-E is
  evaluated as the distance along the 3-D surface normal (same tolerances), and the in-plane form is reported.
* **C6** (`nose_rows_algebraic`): in the smoothing, on the C lines of the nose region (`h_c <= i <= n_i - h_c`), the first `max(1, (n_side - n_bl) / 4)`
  rows beyond the stack keep their algebraic positions (the existing `freeze` rule of the fallback chain, restricted to the nose lines). Target
  measure: the B' maximum skewness over nose/outer faces (7.084); also reported: the nose-stack non-orthogonality maximum (77.38).
* **C4** (`front_edge_held`): in the smoothing, the front-edge row (the C/F interface, corner included) keeps its algebraic positions. Target
  measure as registered for M4 (A' maximum aspect, 395.1); also reported: B' aspect.

Each is accepted or removed by the registered decision rule, applied in this order on top of the changes accepted before it.

## Amendment 2 (written after the registered Part 2 runs and the Part 1 development, before any registered FAN-01 build; no limit changed)

### Part 2: the registered sequence (test `part2_sequence`, run once; 38 min)

The harness smoke on construction E (rule (d)) found a defect of its own Q7 measure before the registered run: it measured the wall stacks with
3-D chords, whose sagitta against the cylinder is 8e-3 of the stack on A'. Corrected before the run to the unrolled-plane measure of ADR 0068/0072
(E then reproduces the 0072 values exactly: A' 3.21e-13 / 1.35e-13, B' 1.66e-11 / 4.56e-12); under C5 the in-plane distance is multiplied by
`cos(lambda)` (the component along the 3-D surface normal), as registered in Amendment 1. Baseline E: Q2 1.25e-15, Q6 3.3e-13, R-E 107 of 117.

| Change | Target measure (B' unless stated) | Other Q1-E maxima that moved by more than 1 % | Q2/Q6/Q7 | R-E | Decision |
|---|---|---|---|---|---|
| C2 `wake_aspect_50` | wake skewness 4.847 -> 1.387 (predicted below 1.5) | A' skewness 0.863 -> 1.041 | pass | 105 | REJECTED (ii, iv) |
| C5 `lean_stack` | blade-stack skewness 2.057 -> 1.098 (the 78.17 deg trailing-edge faces fall to 77.38) | B' non-orthogonality 78.17 -> 85.89 | pass (3-D normal form 5.0e-12; in-plane form 0.24 by design) | 102 | REJECTED (ii, iv) |
| C6 `nose_rows_algebraic` | nose/outer skewness 7.084 -> 1.311 (B' skewness maximum 7.08 -> 4.88, now the wake) | B' non-orthogonality 78.17 -> 81.69 | pass | 89 | REJECTED (ii, iv) |
| C4 `front_edge_held` | A' aspect 395.1 -> 288.2 (below the 300 gate) | A' non-orthogonality 61.38 -> 71.16 | pass | 105 | REJECTED (ii, iv) |

Each change removed the defect it was aimed at (the mechanisms of Amendment 1 are confirmed by their own target measures), and each one moved
another maximum up by more than 1 % and lost builds in the R-E scan, so by the registered rule none is kept. The code of the four options is
committed once as measured (a record commit) and removed in the next commit. Construction G is therefore construction F with the checked control range
(no Part 2 change); on blade B the A'/B' numbers of G are measured once at the end (registered: "G itself").

### Part 1: development on FD and S (what was tried, before freezing F)

* The registered upstream extension (`lambda = 0.2 c`) folds FD at layer 7 (r = 0.208, stagger 64 deg): the seam leaves the leading edge at the camber
  slope (about tan 70 deg) and runs nearly parallel to the front edge at the corner. Changed to `lambda_u = front_gap c / 3`, so the seam meets the front
  edge within atan(sigma_le e^-3) of the axial direction. With it FD builds: 80.88 deg, skewness 3.57, aspect 1414, construction step 0.
* The smallest angle between a C line and the seam where it meets it is 13.6 deg (FD). This is a property of the periodic pairing, not of the target
  rule: the seam pair is a pure s-translation with index-matched nodes, so the suction line of station x and the pressure line of station x end at one
  seam node; the wall-normal feet of the two lines on a camber-parallel seam lie `P sin(g)` apart along it (162 mm at the FAN-01 tip, three chords), so any
  shared node leaves both lines about `90 - g` deg off the seam. A target-rule change cannot remove it and none was made.
* A blended seam (`s_centre + alpha (C - s_centre)`, alpha the least departure from straight seams that keeps a margin from the seams) was tried so
  that blade B would stay identical to E: FD then builds only at fallback step 1 with 87.35-88.0 deg (upper seam corner of the tip and gap layers, and a
  jump of 11 layer thicknesses of a wake node between two tip layers); construction E itself on a tip-only FAN-01 (sections from r = 0.19 m) gives
  87.4 deg. Dropped: the pure camber seam (alpha = 1, as registered) is the better one for FAN-01. The Part 2 candidates added to the blended F on FD
  gave 84.4-87.4 deg (information only; they are rejected by the Part 2 rule anyway).
* DH (FD, frozen F): hub first-layer maximum 64.33 deg (the nose line, in-plane), k-faces 48.18 deg, first-layer skewness 0.65: **no hub change**.
* Blade B, S counts, frozen F: 72.36 deg, skewness 1.063, aspect 128.8 (E: 53.82 / 0.596 / 129): the camber seam is worse than straight seams for
  blade B; F is meant for blades that straight seams cannot hold. B1 (hashes of `pursuit` and E) reproduced after every change.
* The P1-P8 harness was smoke-tested on FD: closure 1.9e-15, periodicity 7.9e-14, stacks 4.2e-14 / 6.0e-14, Q3 identity 1.3e-16, wall nodes on the
  exact surface to 0 m, solid volume 1.5e-3 from the section-area quadrature.

### Construction F, frozen

E + `camber_seam` (alpha = 1: camber line inside the chord, the wake curve downstream, `lambda_u = front_gap c / 3` upstream; the wake lies mid-passage
by construction) + `gap_fit`. G = F + `checked_range` (Part 3; no change of the geometry). Prediction: FAN-01-C misses P2 (FD: 80.9 deg, 3.57, 1414)
and passes P1 and P3-P10 if its non-orthogonality stays below the 85 deg error threshold of `check_mesh`. Because no Part 2 change is accepted and the
range check does not move a node, measurement (b) of FAN-01-C with G is the measurement (a) with F if the registered counts are inside the range of
Part 3 (stated here before either runs).

## Amendment 3 (written after the registered FAN-01-C (a) build failed, before the corrected build (a2) ran; no limit changed)

**Outcome of (a)** (frozen F, registered counts, test `p1_p8_...` and `p2_...`, run once): **P1 MISS**, the mesh is not built. Every step of the
fallback chain folds; the located error reported is `mesh.fan.front_gap`: "the line family is folded at layer 11 (r = 0.24544), C block around line
23, row 2: the leading-edge fan cannot be built for this blade with the front_gap 0.3 and n_nose 4 given". P2-P10 are not measurable on (a).

Diagnosis (pencil, from the message and the construction; no further build of the registered counts): line 23 is the suction-side nose line next
to the leading-edge line 24, row 2 the edge of its stack. F splits the front edge at the centre line `C(z_f)`; upstream of the leading edge the
centre line rises along the leading-edge camber slope (about -3.7 near the tip), so `C(z_f) = s_le + 3.7 lambda_u (1 - e^-3)`, about 19 mm above the
leading edge at the tip. The suction-side nose lines start facing down and upstream and are sent to targets above the leading edge, across the
leading-edge line: a fold. FD (n_nose 5, n_chord 20) happened to untangle; the registered n_nose 4 does not.

**Change (F2)**: the front edge is split at `s_le` (straight ahead of the leading edge, as in E); the seams and the F block keep the centre line.
Development only on FD and S: FD 78.36 deg (was 80.88), skewness 3.573, aspect 1482, step 0; the P1-P8 harness on FD passes (closure 1.3e-15,
periodicity 8.0e-14, stacks 4.2e-14 / 6.0e-14, Q3 6.6e-16, wall nodes exact); DH hub first layer 37.98 deg; S on blade B 58.97 deg, 0.756, 128.1. B1 holds.

**(a2)**: the registered FAN-01-C counts with F2, P1-P10 as registered, run once. (a) stays recorded as a miss. G = F2 + `checked_range`. F2 is the
construction of the case value `camber_seam`.

## Amendment 4 (written after the registered (a2) run and a diagnosis on the registered counts, before (a3) ran; no limit changed)

**(a2) results** (F2, registered counts, run once): P1 built (11 536 cells, construction step 0, h1 5.654e-4 m, h_gap 2.867e-4 m). P2 non-orthogonality
87.22 deg (MISS), skewness 4.126 (MISS), aspect 227.2 (met). P3 closure 9.0e-16, topology ok. P4 periodicity 1.1e-13. P5 stacks radial 5.6e-14, wall
9.7e-14. P6 C nodes identical, volume identity 4.0e-16. P7 wall nodes on the exact surface to 0 m. P8 solid volume 2.5e-3 below the section-area
quadrature. P9 `FvGeometry::new` accepts; `check_mesh` error "max non-orthogonality 87.2 deg exceeds 85.0"; rules SV011 (error) and SV102 (warning,
skewness above 4): **MISS**. P10 bounded: continuity 7.47e-4 at iteration 10, at most 2.97e-4 over 200-300, no failure, largest relative speed 42.4 m/s,
`|m_in + m_out|` 7.2e-5 of `m_in`: **met**.

**Diagnosis after the run (on the registered counts, disclosed):** the faces above 85 deg are seam faces and i-faces of the outer row next to the two
seam corners in layer 10 (r = 0.240), whose node lines tilt by 74.5 deg between layers 10 and 11. Tracking the lower seam corner per layer: z = 0.1120,
0.1014, 0.1113 m in layers 9, 10, 11 (layer 10 an outlier by 10 mm against 18 mm of radius). Every layer needs the local untangling of a folded
algebraic grid; layers 10 (and its remapped far grid) need 8 passes, the tip layers 7. The control functions are computed after the untangling, so a
different number of passes gives different controls and a different smoothed grid: the smoothed far grid is not continuous in the radius. Tried
(development, disclosed): holding the corner at its algebraic position (FD 82.75 deg / skewness 5.55, worse); controls from the algebraic grid before
the untangling (FD and the registered counts fold); a radial filter with a short ramp from the stack edge (folds next to the wake stack).

**Change (F3)**: F2 + `radial_filter`: after all layers are written, two Jacobi passes move every interior layer's far node `(z, theta)` half-way
towards the linear interpolation in r of its two neighbouring layers; the weight rises from 0 at each stack edge to 1 on the seam or front edge (smooth
step); the wall and stack nodes, the outlet lines, the inlet row and the hub and shroud layers are not moved; both seam copies get the same weights, so
the pair stays an exact rotation image. G = F3 + `checked_range`.

**What I already knew when writing this:** FD with F3: 78.36 deg, skewness 3.46, aspect 149, step 0, P1-P8 harness passes; S on blade B: 58.28 deg,
0.756, 128.1; B1 holds; and the registered counts with F3 built once as a diagnosis: 83.26 deg, skewness 3.98, aspect 227, step 0 (so (a3) is not blind for
P1 and P2; P3-P10 have not been run with F3). **(a3)**: P1-P10 on the registered counts with F3, run once; (a) and (a2) stay recorded as misses.

## Amendment 5 (written after (a3), the Part 3 development scan and a Part 3 development change, before the range is coded and before R-G runs)

**(a3) results** (F3, registered counts, run once): P1 built (11 536 cells, step 0). P2 non-orthogonality 83.26 deg (MISS), skewness 3.978 (MISS), aspect
227.2 (met). P3 closure 7.9e-16, topology ok. P4 periodicity 1.8e-13. P5 stacks 5.6e-14 / 9.7e-14. P6 C nodes identical, identity 4.0e-16. P7 0 m.
P8 2.5e-3. P9 **met**: `FvGeometry::new` accepts, `check_mesh` ok, one rule finding SV101 (warning, above 70 deg), no error. P10 **MISS**: continuity
7.92e-4 at iteration 10, 6.62e-3 at most over 200-300 (finite throughout, no solver failure, `|m_in + m_out|` 7.2e-5 of `m_in`, largest relative speed
63.0 m/s in the seam-row cells next to the lower seam corner in the near-tip and gap layers, cells of non-orthogonality up to 71 deg). The (a2) mesh met
P10 and missed P9, the (a3) mesh meets P9 and misses P10: no FAN-01 mesh of this wave meets both. No further attempt is made.

**Part 3 development scan** (`part3_development_scan`, construction G = F3, combinations n_chord {12, 20, 28, 40} x n_nose {2, 3, 5, 7, 10} x n_side
{5, 8, 12, 20, 28} with n_nose + 2 <= n_chord, and front_gap {0.15, 0.25, 0.4, 0.7, 1.2, 2.5}; blade B on the S counts, FAN-01 on the FD counts):
blade B 87 of 106 build, 34 meet the A' gates; FAN-01 8 of 106 build, none meets them (folds in the leading-edge fan, located on `front_gap`).
**Development change (G)**: for the camber-seam construction only, two last fallback steps start the smoothing from a Laplace-smoothed grid (400
Jacobi passes, every free node at the mean of its four neighbours) instead of the algebraic grid, with the exact and with the one-dimensional controls.
A mesh that builds at an earlier step is unchanged (the registered FAN-01-C builds at step 0, so (a3) stands; `pursuit` and E are untouched). FAN-01 dev
scan with it: 50 of 106 build (42 at the new step 7), median 84.6 deg, 27 below 85 deg, none meets the A' gates.

**Registered control range of G** (located errors before any construction, only with `checked_range`, i.e. `c_grid_construction = "camber_seam"`),
chosen from the blade B development data (all 42 in-range blade B combinations built, 32 met the A' gates; FAN-01: 22 of 42 built, 11 below 85 deg):
* `0.15 <= n_nose / n_chord <= 0.375` (error on `n_nose`);
* `max(8, n_bl + 6) <= n_side <= 2 n_chord` (error on `n_side`);
* `0.15 <= front_gap <= 1.0` (error on `front_gap`).
S, A', B', FD and FAN-01-C lie inside. The range does not make FAN-01 robust; it states where blade-B-like geometry builds.

**R-G (registered scan, run once)**: n_chord {14, 18, 24, 36} x n_nose {3, 4, 6, 9} x n_side {9, 14, 24, 40} with n_nose + 2 <= n_chord, on blade B (S
counts otherwise) and on FAN-01 (FAN-01-C counts otherwise), plus front_gap {0.1, 0.18, 0.35, 0.6, 0.9, 1.5} on both. Gates: (a) every in-range
combination builds and validates: 100 %, per fan; (b) every out-of-range combination is refused with a located error naming `n_nose`, `n_side` or
`front_gap`: 100 %. Reported: the fraction of in-range combinations that build AND meet 70 deg / 2.0 / 300, the median non-orthogonality, the
`construction_step` distribution, and the count below the 85 deg error threshold. **Prediction (from the development data): (a) is met on blade B and
MISSED on FAN-01.**
