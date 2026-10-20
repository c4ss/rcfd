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

## Amendment 1 (written after the FT development, before any 3-D build of a candidate, the rule (d) smoke included; no limit changed)

### FT development (test `ft` of a development file, not committed; each variant run once, all listed)

Proxy on the smoothed layers at the 15 far-grid radii, FAN-01-C counts. H (baseline, same run): corner region 86.890438 deg (layer k 7, the upper
corner line's j-face C (28, 9) / (28, 8)), outside the corner region 82.347818 deg (k 9, wake line 5, C (5, 2) / (5, 1)), smallest cell area 5.216e-7
m^2. Threshold of FT (a): 0.9 x 86.890438 = 78.201394 deg. The variants of I-a (grading: the front-edge cells grow from the nose cell to a corner cell
of `cf` times the algebraic seam cell next to the corner, two-sided, capped as `capped_cells`; inclination: the front edge `z = z_f + dz(s)` with `dz =
u cot(phi) fade(u / ell) - v cot(phi) fade(v / ell)`, `u`, `v` the pitchwise distances from the lower and upper corner, so it leaves the lower corner
and reaches the upper corner along `phi` from `+z`, `ell` = 0.1 pitch; the F rows follow with weight `(z_a / z_f)^4`; `kappa`: `phi = 90 deg + kappa
atan(sigma_le)`; `hold`: the first `hold` front-edge nodes at each corner are not moved by the smoothing; `holdc`: the corner node is not moved):

| # | Variant | corner max (a) | change | outside max (b) | min area | FT |
|---|---|---|---|---|---|---|
| 1 | grade | 79.01 | -9.07 % | 77.52 | 5.29e-7 | fails (a) |
| 2 | grade, cf 2 | 78.93 | -9.16 % | 77.25 | 5.29e-7 | fails (a) |
| 3 | grade, phi 40 | 80.37 | -7.5 % | 80.83 | 4.78e-7 | fails (a) |
| 4 | grade, phi 60 | 79.91 | -8.0 % | 79.72 | 4.98e-7 | fails (a) |
| 5 | grade, kappa 0.5 | 80.06 | -7.9 % | 80.19 | 4.90e-7 | fails (a) |
| 6 | grade, kappa 0.67 | 80.44 | -7.4 % | 80.98 | 4.76e-7 | fails (a) |
| 7 | grade, hold 1 | 86.68 | -0.2 % | 78.74 | 5.23e-7 | fails (a) |
| 8 | **grade, phi 40, hold 1** | **78.006312** | **-10.2245 %** | 81.062759 (-1.56 %) | 4.91e-7 | **passes** |
| 9 | grade, phi 40, hold 2 | 78.225028 | -9.9728 % | 82.09 | 4.71e-7 | fails (a) |
| 10 | grade, phi 40, hold 1, holdc | 86.29 | -0.7 % | 81.10 | -1.75e-5 (fold) | fails (a), (c) |
| 11 | grade, phi 60, hold 1 | 79.94 | -8.0 % | 80.26 | 4.96e-7 | fails (a) |
| 12 | grade, kappa 0.67, hold 1 | 78.397167 | -9.7747 % | 81.18 | 4.89e-7 | fails (a) |

Exactly one variant passes FT, #8, by 0.195 deg; HT is therefore not falsified by FT, and the registered rule freezes #8. The development budget (12)
is used up; no further variant or family (I-b) is tried. The corner maximum of #8 is the lower corner face of ADR 0083, C (20, 9) / (19, 9), at the
tip radius (78.01 deg against H's 80.55 there).

**The pass is not evidence for HT's mechanism** (stated now, from the angle printout of the development runs): the inclination does not survive the
smoothing. With #8 the front edge leaves the lower tip corner at 105.4 deg (H 101.5; prescribed 40), and at the upper corner the held first front-edge
node lies at 144.5 deg from the corner node, i.e. upstream of it: the corner node, free under its mean-of-six rule, moved past the held node. Most of
the gain is the grading (#1 alone: 79.01); inclination plus hold add about 1 deg. A pencil reading of the same printout (unverified as a rule): at the
tip the lower side of the corner node spans from the seam going downstream (about -50 deg) to the F-side seam going upstream (about +144 deg), 194 deg;
the C seam cell against the nearly pitchwise corner line takes about 144 of it, so a front-edge direction can only split the remaining 50 deg between
the C front cell and the F cell. The levers are the F-side seam direction and the corner-line arrival, not the front edge; they are not tried in this
wave.

### Construction I, frozen

H + one new opt-in field `CGridOptions::inclined_front` (default `false`), set only by `CGridOptions::camber_seam_i_construction()`, case value
`c_grid_construction = "camber_seam_i"`. With the field, in every layer: (1) the front-edge cells are graded two-sided from the nose cell (as now,
`3 |sp1 - sp0|`) to a corner cell equal to the algebraic seam cell next to the corner; (2) the front edge is `z = z_f + dz(s)` as above with `phi` =
40 deg at both ends and `ell` = 0.1 pitch, the C lines of the front edge aim at its nodes, and the F rows are displaced by `(z_a / z_f)^4 dz`; (3) the
first front-edge node next to each corner is not moved by the in-plane smoothing (every step of the fallback chain); the corner node keeps its
mean-of-six rule. Constants in the code, no environment input. Without the field every expression is the existing one (part D decides).

### Predictions (registered now, before the first 3-D build)

* C1-Q (i) **MISS**: the tip proxy of #8 is 78.0 deg at the lower corner, and the proxy matched the 3-D tip and gap layers of H to 0.05 deg.
* C1-Q (ii) uncertain: the wake-line-5 proxy rises at the tip (H 80.41, #8 81.06 at k 14), the 3-D H value is 80.23.
* C1-B at risk: the fixed 40 deg end inclination also applies on blade B (stagger 20-35 deg), where the corner was balanced; S may not build, or an
  A' / B' maximum may rise by more than 1 %.
* C3 uncertain (the ADR 0083 P10 oscillation is away from the corner).
* Keep clause therefore **expected to fail** and I to be removed.

Rule (d) smoke (next, before the registered runs, recorded in Amendment 2): FAN-01-C with I built once (construction only), the R-I list built once
with H, the P10 case with `camber_seam_i` for 3 iterations.

## Amendment 2 (rule (d) smoke, written before the registered part C runs; no limit changed)

Code: construction I as frozen in Amendment 1 (`inclined_front`, constants `FRONT_INCLINE_DEG` = 40, `FRONT_BLEND` = 0.1, the two-sided corner
grading, the first front-edge node at each corner held in the smoothing), case value `camber_seam_i`; the development hook is not in this code.
Part D on this code: the ADR 0079 B1 hashes (S and A' with `pursuit` and E), the G hashes and the H hashes (S `0x6f6342d53409da61`, FAN-01-C
`0x21104bbbdd9b9129`) are reproduced (`b1_existing_constructions_are_bit_identical`, `b1_construction_g_is_bit_identical`,
`b1_construction_h_is_bit_identical`).

Smoke (`smoke_w37d_set_ups`, `smoke_w37d_p10_case`; build status and counts only, no quality printed): FAN-01-C with I builds (11 536 cells, step 0);
the R-I list has 42 combinations, all inside the G range, 37 of which build with H (construction only); the P10 case with `camber_seam_i` parses,
builds and runs 3 iterations with finite residuals. No set-up is defective.

## Addendum: measured values, misses and deviations

Release builds, scratch tree of this wave; code state of the measurements: record commit 18b083c (construction I and its tests). By the keep clause
I was then removed in the next commit; that removal moves no node of any other construction (part D re-run on the final code). Each registered run
once. Limits are the registered ones, unchanged. No external reference value enters any gate. No fan performance claim. Solver physics files were not
touched.

### Part C: acceptance on construction I (case value `camber_seam_i`)

| Check | Limit | Measured | Status |
|---|---|---|---|
| C1 P1 builds, validates, volumes > 0 | - | 11 536 cells, step 0 | met |
| C1 P3 closure, topology | 1e-13 | 1.00e-15, ok | met |
| C1 P4 periodicity | 1e-12 | 1.49e-13 | met |
| C1 P5 stacks (radial; wall along the 3-D normal) | 1e-12 | 5.60e-14; 6.13e-14 | met |
| C1 P6 bladed vs empty C nodes, volume identity | 0, 1e-11 | 0, 1.32e-16 | met |
| C1 P7 wall nodes on the exact surface | 1e-12 m | 0 m | met |
| C1 P8 solid volume against the quadrature | reported | -2.54e-3 | - |
| C1-P2 non-orthogonality / skewness / aspect | 70 / 2.0 / 300 | **81.00** / **4.577** / 261.0 | **MISS** (predicted) |
| C1-Q (i) corner region | 70 deg / 2.0 | **78.07 deg** (C (19, 9) / (20, 9), layer 20) / 1.091 | **MISS** (predicted) |
| C1-Q (ii) global maxima <= 1.01 x H (H 83.26 / 3.978 / 227.2) | 84.09 / 4.018 / 229.5 | 81.00 / **4.577** / **261.0** | non-orthogonality met; skewness and aspect **MISS** |
| C1-B S builds | - | 66.21 / 0.727 / 137.2, step 0 | met |
| C1-B A' maxima <= 1.01 x H (66.18 / 0.888 / 394.4) | | **71.82** / **1.164** / **457.2** | **MISS** |
| C1-B B' maxima <= 1.01 x H (80.93 / 8.894 / 8805.0) | | **89.82** / **153.8** / **12185.1** | **MISS** |
| C1-B closure, periodicity (S, A'); stacks (A' 1e-12, B' 4.9e-11) | | S 5.0e-16 / 9.6e-14; A' 1.0e-15 / 4.3e-13; A' 3.2e-13 / 1.5e-13; B' 1.7e-11 / 5.0e-12 | met |
| C2 P9 FvGeometry, check_mesh, rules | accepted, ok, no error | accepted, ok, SV101 and SV102 warnings (H: SV101 only) | met |
| C3 P10 laminar smoke | max over 200-300 <= value at 10 | 2.100e-3 at 10, 1.394e-3 over 200-300 (ratio 0.664); finite, no failure | met by its criterion |
| C4 R-I blade B: builds with I >= with H | 21 (H) | **18** (three tangles at n_nose 4, n_side 21) | **MISS** |
| C4 R-I FAN-01: builds with I >= with H | 16 (H) | 16 | met |

Reported with C3: the largest relative speed is 242.45 m/s, in a poor cell at the upper corner line in the tip gap (C (27, 9), layer 20; H: 62.1
m/s); 89 % of the late argmax cells lie in the poor region (G in ADR 0083 A0: 42 %). The criterion is met partly because the continuity at iteration 10 is 2.6 times
H's; this is not a healthier solve than H's. Reported with C4 (no gate): blade B meeting 70 / 2.0 / 300: H 18, I 10 of 21, median non-orthogonality
63.17 -> 69.97 deg; FAN-01: none meets the target with either, median 85.26 -> 82.50 deg, built meshes below the 85 deg `check_mesh` threshold H 8 of
16, I 15 of 16 (different combinations fail: I builds two that H folds and folds or tangles two that H builds); the corner-region maximum of the 16
FAN-01 meshes built with I lies between 77.5 and 84.2 deg (none at or below 70).

### Verdicts

* **Keep clause failed** (C1-Q (i) and (ii), C1-B, C4 blade B). Construction I is **removed**: the `inclined_front` field, its constructor and
  constants, the two-sided corner grading, the smoothing hold, the case value `camber_seam_i` and its tests (C1, C1-P2, C1-Q, C1-B, C4, C2, C3, the
  smokes). Kept: `b1_construction_h_is_bit_identical` (part D, independent of I). `docs/case-schema.md` and `docs/meshing.md` were never changed.
* **HT** (an inclined front edge removes the corner faces above 70 deg): not supported. FT was passed by one variant by 0.195 deg, but, as stated in
  Amendment 1 before any 3-D build, most of that gain was the corner grading and the prescribed inclination did not survive the smoothing. In 3-D the
  corner region fell from 83.26 to 78.07 deg and the global non-orthogonality from 83.26 to 81.00 deg (now the wake-line-5 face), while the skewness
  (3.978 -> 4.577) and the aspect ratio (227.2 -> 261.0) rose, and on blade B the change is harmful (B' skewness 8.9 -> 153.8, not located).
* **The topology change proper (I-b, an index offset between the two corners with a collapsed F row) was not tried.** The registered protocol
  developed I-a first and allowed I-b only if no I-a variant passed FT; one did. So this wave measured an inclined front edge, not the offset
  topology.
* What the measurements and the pencil analysis of Amendment 1 say about the next attempt (unverified as rules, no gate): (1) with the two seams
  paired station by station from the outlet, a straight front edge perpendicular to the seam needs an index offset of about `n_chord` at the tip and
  none at the hub, so it does not fit one index topology; (2) at the tip the C seam cell next to the corner takes about 144 of the 194 deg on the
  lower side of the corner node because the corner line arrives nearly pitchwise, so the front-edge direction only splits the remaining 50 deg; the
  levers are the F-side seam direction at the corner and the arrival of the corner line, or a periodic matching with an index offset (a sheared
  C-H topology) at low solidity and high stagger (FAN-01 tip: pitch / chord 3.1, stagger 70 deg).

### Part D: default behaviour

B1 met on the final code: the ADR 0079 hashes (S and A' with `pursuit` and E), the G hashes and the H hashes (S `0x6f6342d53409da61`, FAN-01-C
`0x21104bbbdd9b9129`) are reproduced (`b1_existing_constructions_are_bit_identical`, `b1_construction_g_is_bit_identical`,
`b1_construction_h_is_bit_identical`); the default FAN-01 refusal message is unchanged; `baseline_hashes.rs` untouched.

### Deviations from the registered plan (candid list)

* The registered C1 run was started with the test filter `c1_`, which also re-ran two existing ADR 0083 tests on H (`c1_fan01_h_...` and the
  ignored `c1_p2_fan01_h_quality_gates_known_miss`); they reproduced the ADR 0083 numbers (deterministic) and choose nothing.
* FT used a 2-D proxy (the face metrics of `compute_metrics` restated for one smoothed layer at the far-grid radius), not the 3-D mesh. It matched
  the 3-D tip and gap layers of H to 0.05 deg but not the mid-span layers (proxy 86.89 deg at k 7 against a 3-D maximum of 83.26), because the 3-D
  far grid is blended with the remapped layer and radially filtered.
* The development hook (`dev_layer`, `dev_rho`, the environment-driven variant parameters) and the development test file were not committed.
* I-b was not developed (above). The 12-variant budget was used up by I-a.

### Unverified references

Thompson, Warsi & Mastin (1985) for the elliptic smoother (recalled, as ADR 0072 / 0079 / 0083). Nothing else.
