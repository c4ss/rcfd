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
