# ADR 0087: Wave 37D, a front edge inclined with the stagger at the FAN-01 C-grid seam corner (opt-in construction `camber_seam_i`)

Status: registered before any construction code of this wave was written. Limits are fixed; a miss becomes an `#[ignore]`d `*_known_miss` test
with the limit unchanged, a defective set-up gets a NEW corrected test and the old one stays ignored. Baseline hashes are never re-recorded; every
change is opt-in. Clean-room: first principles only (no reference value enters a gate). Parent: ADR 0083 (wave 36D) and its addendum: construction H
(case value `camber_seam_h`) on FAN-01-C 83.26 deg / 3.978 / 227.2; the maximum non-orthogonality is the lower seam corner of the tip layers, where the
corner line arrives 7.6 deg from the front edge; the one corner rule tried (H2, the corner line's last node on the bisector) folded the tip layers;
P10 missed; FAN-01 R-G builds 22 of 39. Its suggested fix, not tried: a topology change at the tip, e.g. a front edge inclined with the stagger.
Solver physics files are not touched by this wave. Nothing here is a fan performance claim.

## What was run before this ADR was written (disclosed)

All on HEAD 6f97dcb in a scratch copy, release builds, construction only (no solve). A development hook (`dev_layer`: the algebraic and the smoothed
in-plane layer of one radius, step-0 setting) was added to the scratch copy only; it moves no node and will not be committed.

* Corner angles of H per layer (direction from the lower corner node to its neighbours, degrees from `+z` in the unrolled plane `(z, s)`; the upper
  corner is the same periodic node): hub (k 0, stagger 43.6 deg) seam 6.2, corner line 70.9, front edge 111.3, F row upstream 162.3, upper corner line
  -46.5, upper front edge -112.5: lower C side 105.1 deg, upper C side 118.7 deg; mid (k 7, 61.2 deg) seam -26.5, front edge 108.3 / -103.0: 134.8 / 76.5;
  tip (k 14-21, 69.6 deg) seam -49.9, corner line 94.2, front edge 101.5, F row upstream 143.8, upper corner line -67.5, upper front edge -97.0: lower C
  side 151.4 (cells 144.1 and 7.3), upper C side 47.1 (29.5 and 17.6), lower F cell 42.3, upper F cell 119.2. So the corner is balanced at the hub and
  degenerates with the stagger.
* The tip layer (r 0.2475, pitch 0.1728 m, chord 0.0549 m) algebraic and smoothed: algebraically the front edge is the line z = 0.1038 (pitchwise),
  the seam leaves the lower corner at -13.5 deg (the camber seam turns axial upstream of the leading edge) and the corner line arrives at 88.7 deg, 1.3
  deg from the front edge (the sliver is already in the algebraic grid: lines 20 and 21 run down along the front edge 0.5 mm apart). Smoothing moves the
  corner by 21 mm in s and 7.6 mm in z and turns the seam to -49.9 deg; the front-edge cell next to the corner is 64 mm, the seam cell 3.5 mm.
* 3-D corner-region measure on H (region defined in part C below; with H the upper corner line is `n_i - h_c`): corner region 83.26 deg (the face of
  ADR 0083, C cells (19, 9) / (20, 9), layer 11), skewness 1.042; all other faces 80.23 deg (wake line 5, rows 1/2, layer 15), skewness 3.978. G gives the
  same numbers. So a perfect corner leaves FAN-01-C at about 80.2 deg / 3.98 (the wake-line-5 faces of ADR 0083 DW, untouched by a front-edge change).
* A 2-D proxy of the in-plane face metrics (per edge of the C and F blocks of one smoothed layer: the angle between the line of the two cell centroids
  and the edge normal, centroid to edge midpoint on boundary and seam edges as `compute_metrics` does on the seam patches): H at the tip radius 80.55 deg
  at C (20, 9) / (19, 9), equal to the 3-D value of that face in the tip-gap layers (80.5 deg, ADR 0083), so the proxy is faithful for the extruded tip
  layers. At the hub 57.25 deg (smoothed), at r = 0.1857 m 86.89 deg at the upper corner line (C (28, 9) / (28, 8)): the true-radius layer is not the
  3-D mid-span layer (whose far grid comes from the remapped radius and is radially filtered).
* Hashes recorded now for part D (FNV-1a as ADR 0079 B1): H on blade B with the S counts `0x6f6342d53409da61`, H on FAN-01-C `0x21104bbbdd9b9129` (as
  ADR 0083); G reproduced (S `0x59b26da9febc9b2e`, FAN-01-C `0x00b3e9673d470e52`).

## The structural constraint (pencil, from the code)

`Layout::f` maps the F row `a = n_up` to the C nodes `c(h_c + b, ns)`, and `seam_nodes` pairs the lower seam station `h` with the upper station `h`,
both counted from the outlet. So the lower corner (station `h_c`) and the upper corner (station `h_c`, rotated) are one periodic node at one `z`, and a
straight front edge between them is pitchwise. A straight front edge perpendicular to the seam needs the upper corner `d` stations downstream of the
lower one. At the tip that offset is `pitch sin(gamma) cos(gamma)` = 0.056 m in z, three chord lengths along the passage, so `d` would be about
`n_chord`; at the hub (43.6 deg, the corner already balanced) it would be zero or negative. `d` is one integer for all layers (one index topology).
Hence a straight inclined front edge over the whole span is not possible in this C-grid; what is possible:

* **I-a (inclined corner segments, same topology)**: the corners stay one periodic node; the front edge is inclined where it meets the seams (its
  segments next to each corner turned towards the normal of the seam, i.e. with the stagger, and blended back to the existing front edge away from the
  corners), with the front-edge spacing graded so that the corner cell is comparable to the adjacent seam cell. In the periodic plane, with a front
  edge straight through the corner node, the lower and upper C sides add up to 180 deg; they are equal (90 / 90) when the front edge is perpendicular
  to the seam, against 151 / 47 now.
* **I-b (offset corner, a topology change)**: the upper corner one station downstream of the lower (`d = 1`); the front edge gets `2 n_nose + 1`
  cells; the F block gets one extra row whose lower-seam node collapses onto the lower corner (prism cells, the existing `clean_face` path); the lower
  corner and the new upper corner are irregular nodes of valence 6 (with one triangle) and 5 instead of one node of valence 6.

## Hypothesis and falsifier (registered; development data)

**HT**: the FAN-01-C corner-region faces above 70 deg (maximum 83.26) are caused by the corner geometry, the front edge meeting the seam at about 150
deg on one side and 47 deg on the other at high stagger; a front edge inclined with the stagger where it meets the seams removes them.

**Falsifier FT (run first, before any 3-D construction of a candidate)**: the 2-D proxy above on the smoothed layer (step-0 setting of the construction,
FAN-01-C counts) at the far-grid radii of the 3-D mesh (the remapped radii of layers `k = 0 .. n_span`; `k = n_span` is the tip and the gap layers).
A candidate passes FT iff (a) the largest corner-region proxy non-orthogonality over these radii falls by at least 10 % against H on the same radii
(H measured in the same run); (b) the largest proxy non-orthogonality outside the corner region does not rise by more than 1 % against H; (c) every
smoothed proxy layer is unfolded (all cell areas positive). HT is **falsified** if no candidate passes FT.

Development protocol: I-a first; I-b only if no I-a variant passes FT. At most 12 development variants in total (each listed with its proxy numbers in
Amendment 1). The variant frozen in Amendment 1 is the one with the lowest (a) value among those that pass FT. If HT is falsified, no 3-D construction
is written, part C is not run, and the falsification is the result of this wave. Holding the corner node or the seam nodes at algebraic positions is
allowed inside a variant (stated with it); a new moving-corner rule is not (H2 folded).

## Part B: the construction (frozen in Amendment 1)

A new opt-in field of `CGridOptions` (default `false`), set only by a new constructor `CGridOptions::camber_seam_i_construction()` = H + the field, and
a new case value `c_grid_construction = "camber_seam_i"` (schema enum, `docs/case-schema.md`, `docs/meshing.md`). `pursuit`, `smoothed`,
`camber_seam` (G) and `camber_seam_h` (H) are bit-identical (part D). The control range checked is G's (the narrower H5 range of ADR 0083 was rejected).

## Part C: acceptance on construction I (each run once, after Amendment 1 freezes I and the rule (d) smoke of every new set-up)

* **C1** FAN-01-C (the ADR 0079 registered counts and y+ = 30 rule) with I: P1, P3-P8 of ADR 0079 with their limits (P5 wall stacks along the 3-D
  surface normal, as ADR 0083 C1, because I includes H3 `lean_stack`).
* **C1-P2** the full quality target 70 deg / 2.0 / 300: reported, registered **predicted MISS** (the wake-line-5 faces, 80.23 deg / 3.978, lie
  outside the corner region and are not changed by a front edge).
* **C1-Q** (the registered intermediate target, justified by the numbers above: the corner is where the ADR 0083 maximum is and the only place a
  front-edge change acts): (i) over the faces of the corner-region cells the maximum non-orthogonality is at most 70 deg and the maximum skewness at most
  2.0; (ii) the FAN-01-C global maxima do not exceed H's by more than 1 %: non-orthogonality <= 84.09 deg, skewness <= 4.018, aspect <= 229.5.
  **Corner region**: C cells `(i, j)` of rows `j >= n_side - 3` with `|i - h_c| <= 2` or `|i - u_c| <= 2` (`u_c` the upper corner line: `n_i - h_c` for
  I-a, `n_i - h_c + 1` for I-b), and F cells of the last two rows within two columns of either seam, in every layer.
* **C1-B** blade B, as the ADR 0083 rule (ii) and (iii): S, A', B' build with I; no maximum (non-orthogonality, skewness, aspect) of A' or B' exceeds
  H's (ADR 0083 C5b: A' 66.18 / 0.888 / 394.4, B' 80.93 / 8.894 / 8805.0) by more than 1 %; closure (1e-13) and topology on S, A' and FAN-01-C;
  periodicity (1e-12) on S, A', FAN-01-C; stacks A' and FAN-01-C 1e-12, B' 4.9e-11 (wall stacks along the 3-D normal).
* **C2** P9 through the case with `camber_seam_i`: `FvGeometry::new` accepts, `check_mesh` ok, no error-level rule finding.
* **C3** P10 through the case with `camber_seam_i` (the ADR 0079 set-up: laminar, rotating frame, SIMPLEC, `upwind`, 300 iterations): no failure,
  continuity / res_u / res_p finite, maximum continuity over iterations 200-300 not above its value at iteration 10.
* **C4** registered scan **R-I** (combinations not in R-E, R-G, R-H or the development scans): n_chord {17, 21, 26} x n_nose {4, 6} x n_side
  {11, 16, 21}, and front_gap {0.22, 0.32, 0.55}; blade B on the S counts and FAN-01 on the FAN-01-C counts otherwise (21 per fan, all inside the G
  range). **Gate**: per fan, the number of combinations that build and validate with all volumes positive is with I at least the number with H on the
  same list in the same run. Reported, no gate: the fraction of the 21 that build (against the 100 % ideal), the fraction that meets 70 / 2.0 / 300,
  the median non-orthogonality, the construction steps, and per built FAN-01 mesh the corner-region maximum.
* Prediction (registered now, before any candidate exists): C1-P2 MISS; C3 uncertain (the ADR 0083 P10 oscillation sits on cells away from the corner).

**Keep clause** (registered): construction I and its case value are kept iff C1 (P1, P3-P8), C1-Q (i) and (ii), C1-B, C2, C3 and the C4 gate are all
met. Otherwise I is removed from the code (preamble rule (b)) and its numbers stay in this ADR. C1-P2 is not part of the keep clause.

Rule (d): before the registered runs, every new set-up is smoke-built once (FAN-01-C with I, the R-I list with H, the P10 case with `camber_seam_i`
for 3 iterations); the smoke results are recorded in an amendment written before the registered runs.

## Part D: default behaviour bit-identical

The ADR 0079 B1 hashes (S and A' with `pursuit` and E), the G hashes (S, FAN-01-C) and the H hashes above (S `0x6f6342d53409da61`, FAN-01-C
`0x21104bbbdd9b9129`, in a new B1 test) are reproduced after every change; the default FAN-01 refusal message is unchanged; `baseline_hashes.rs`
untouched and passing.

## Not claimed

Fan performance; any comparison with measured FAN-01 data; turbulence; accuracy of the smoke solve (bounded only); quality of the non-corner faces.
