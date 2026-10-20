# ADR 0083: Wave 36D, the cause of the FAN-01 laminar smoke miss, and C-grid changes for the failing controls, the trailing-edge lean and the nose compression

Status: registered before any code of this wave was written. Limits are fixed; a miss becomes an `#[ignore]`d `*_known_miss` test with the limit
unchanged, a defective set-up gets a NEW corrected test and the old one stays ignored. Baseline hashes are never re-recorded; every change is opt-in.
Clean-room: first principles and published literature only. Parent: ADR 0079 (wave 35D) and its addendum: construction G (case value
`c_grid_construction = "camber_seam"`, F3 + checked range) on FAN-01-C: 83.26 deg / skewness 3.978 / aspect 227.2, P10 missed (continuity 7.92e-4 at
iteration 10, 6.62e-3 over 200-300); R-G FAN-01 20 of 39 in-range combinations build, blade B 35 of 38; A' with G 66.18 / 0.889 / 394.4, B' with G
80.94 / 8.897 / 8805; mechanisms M5 (trailing-edge lean, stack along the in-plane normal) and M6 (nose compression of the first outer row by the smoother)
each fixed by a single change (C5 `lean_stack`, C6 `nose_rows_algebraic`) that raised another maximum (B' non-orthogonality 78.17 -> 85.89 and -> 81.69)
and lost R-E builds, so both were removed. Nothing here is a fan performance claim. Solver physics files are not touched by this wave.

## What was run before this ADR was written (disclosed)

All on HEAD c89495f unmodified, in a scratch copy, release builds:
* P10 of ADR 0079 reproduced: continuity 7.915e-4 at iteration 10, 6.619e-3 at most over 200-300, largest relative speed 62.95 m/s, `|m_in + m_out|`
  7.2e-5 of `m_in`, no failure (18 s for 300 iterations on 11 536 cells).
* The ADR 0079 attribution on FAN-01-C (`fan01_attribution`): the non-orthogonality maximum (83.26 deg) is the i-face between C cells (19, 9) and
  (20, 9) of layer 11, i.e. the last row next to the lower seam corner (line 20 = h_c ends on the corner); the same face is 80.5 deg in the tip-gap
  layers 12-20 with zero node-line tilt (an in-plane defect). Next: wake-stack j-faces of wake line 5 (rows 1/2, 80.2 deg, layers 14-20, in-plane).
  The skewness maximum (3.978) is the wake/outer j-face of line 5, rows 2/3, layer 10; then pressure/outer i-faces of lines 37/38, row 5, tip layers
  (3.5). Blade-stack faces reach 43 deg / 0.60, nose/outer faces 1.01: on FAN-01-C the M5/M6 mechanisms are NOT where the maxima are.
* Construction-only smoke of scaled FAN-01 counts with G (y+ = 30 rule, n_core 1, n_bl 2, growth 1.2, front_gap 0.3; (n_up, n_chord, n_down, n_side,
  n_nose, n_span, n_gap)): x0.75 = (6, 12, 6, 8, 3, 10, 5): 4 920 cells, 78.39 deg / 1.826 / 168.9; x1 = (8, 16, 8, 10, 4, 14, 7): FAN-01-C; x1.25 =
  (10, 20, 10, 12, 5, 18, 9): 22 320 cells, 79.60 / 9.662 / 237.3; x1.5 = (12, 24, 12, 15, 6, 21, 10): 38 184 cells, 83.36 / 3.762 / 252.4; x2 =
  (16, 32, 16, 20, 8, 28, 14): fold (layer 22, C line 11, row 6). No solve has been run on any of these meshes.
* Re-reading the ADR 0079 R-G log (per-combination lines, which were correct): of the 19 FAN-01 in-range failures, 15 have `n_side >= 24`; of the 16
  folds, 11 are reported at C lines 0-6, row 2 (the wake/outlet end, the first row beyond the stack), the rest at nose lines; `folded_quad` reports only
  the first folded quad of the first fallback step (scan order layer, row, line), so these locations are biased to low line numbers.
* ADR 0079 Part 2 log: with C5 the B' quality has 4 580 faces above 60 deg (E: 420) and its blade-stack non-orthogonality is 77.38, so the 85.89 deg
  maximum is outside the blade stack (location not recorded); with C6 the 81.69 deg maximum is in the blade stack (suction/pressure, not nose).
* FNV-1a hashes (as ADR 0079 B1) of construction G, recorded now for check B1 below: S on blade B `0x59b26da9febc9b2e`, FAN-01-C `0x00b3e9673d470e52`.

## Part A: the cause of the P10 miss (diagnosis; run first, before any generator change)

P10 set-up exactly as ADR 0079 (laminar, rotating frame, SIMPLEC, `upwind`, 300 iterations, built through the case with `camber_seam`). Note: the
solver's `continuity` is the LARGEST cell mass imbalance divided by the largest face area, so it is set by a single cell, and its absolute value is not
comparable between meshes; only the ratio `L / c10` (L = maximum over iterations 200-300, c10 = value at iteration 10) and the signal's shape are.

Runs (each once, 300 iterations):
* **A0** FAN-01-C (x1); **A1** x0.75; **A2** x1.25; **A3** x1.5 (the counts above; mesh y+ rule unchanged, so the first cell is the same 5.654e-4 m).
* **A4** FAN-01-C with `physics.nu = 1.5e-4` (ten times; Re based on the design relative speed and the mid chord falls from about 1.4e5 to 1.4e4;
  `mesh.fan.nu` stays 1.5e-5, so the mesh is A0's).
* **A5** FAN-01-C, 1 500 iterations (is the growth bounded, periodic or monotone).

Recorded every iteration (from `Simulation::flux` and the cell faces, the same sum the solver uses): the argmax cell of `|sum phi|`, its tag
`(block, a, b, k)` (from the C-grid built with the same options), whether it is a **poor cell** (any of its faces above 70 deg non-orthogonality or
skewness 2) or a face neighbour of one (the **poor region**), and the same max-imbalance measure restricted to cells outside the poor region (`c10_np`,
`L_np`). Reported per run: c10, L, L / c10, the fraction `f_poor` of iterations 200-300 whose argmax lies in the poor region, the number of distinct argmax
cells over 200-300, `L_np / c10_np`, the largest relative speed and its cell, and the history every 10 iterations.

**Verdict rule (fixed now)**:
* **MESH** if on A0 `f_poor >= 0.5` and `L_np <= c10_np`, and A4 still has `L / c10 > 1`.
* **FLOW** (laminar unsteadiness at the design Reynolds number) if A4 has `L / c10 <= 1`, and on A0 `f_poor < 0.5` and `L_np > c10_np`.
* otherwise **MIXED**, stated with the numbers.

The count series A0-A3 is reported as a trend of `L / c10` and of `f_poor`; it does not enter the rule, because every count moves the worst faces
(see the smoke above). Existing evidence (ADR 0079): the (a2) mesh (F2, no radial filter; 87.2 deg) met P10 on the same set-up.
Consequence: FLOW makes P10 a defective set-up for a mesh check (a corrected set-up is then registered in an amendment before it runs, the old test
stays ignored); MESH or MIXED keep P10 as the acceptance smoke of Part C unchanged.

## Part B: generator changes (new opt-in fields of `CGridOptions`, default `false`)

### Diagnostics first (measurements only, development data)

* **DF** (the failing controls): on every FAN-01 R-G in-range combination that failed, a development hook lists ALL folded quads of every fallback step
  by layer and region (wake lines / suction / nose / pressure / F block; row), and for the wake lines the per-line `stack_cells` and first height
  `d1` of the neighbouring lines. **Hypothesis HF**: the wake-end folds come from the switch of the wake stack on/off (`growth_sum(d1, n_bl) <= 0.25
  dm`) or of `d1` between neighbouring wake lines or layers. Falsified if no folded wake quad of the first step sits on a line whose neighbour has a
  different `stack_cells` or a `d1` differing by more than the growth ratio.
* **DW** (FAN-01-C, line 5): the same `stack_cells` / `d1` listing for the wake lines of layers 9-20, and the in-plane shape of the cells (5, 1..3).
  **Hypothesis HW**: the FAN-01-C skewness maximum and the 80 deg wake-stack faces have the cause HF. Falsified as for HF.
* **DC** (seam corner, FAN-01-C layers 10-20): the angles of the four C cells and the two F cells at the irregular corner node, and the angle at which
  lines 19, 20, 21 meet the seam / front edge.
* **DL / DN** (B'): the code of C5 and of C6 (ADR 0079 record commit 5348934) is reinstated in a development copy only, and the D1 attribution of ADR
  0079 is run on B' with each, to locate the 85.89 deg (C5) and 81.69 deg (C6) maxima.

### Candidate families (the exact change of each is specified in Amendment 1, written after the diagnostics and before any run of a change)

* **H1** wake-end: a wake stack and first height that do not switch between neighbouring lines or layers (for example the stack kept on every wake line
  with `d1` continuous in the line index), if DF / DW support HF.
* **H2** seam corner: the arrival of the lines at the irregular corner node (for example the corner placement rule or the targets of lines h_c +- 1).
* **H3** trailing-edge lean: C5 with its side effect removed at its source as located by DL (for example a lean factor faded along the wake, or
  continuous between neighbouring lines).
* **H4** nose rows: C6 with its side effect removed at its source as located by DN.
* **H5** control range: a range re-registered on the FAN-01 failure data (DF), only for the new construction.

### Decision rule (registered; 10 % and 1 % are protocol thresholds, not accuracy limits)

Order H1, H2, H3, H4, each on top of the changes accepted before it (baseline: G). Measured once per change on S, A', B' (blade B), FAN-01-C (FAN-01) and
the R-G list of ADR 0079 (used here as development data). A change is **accepted** iff (i) its target measure improves by at least 10 % (H1: FAN-01
R-G in-range builds, or, if H1 is aimed at FAN-01-C, its wake skewness maximum; H2: FAN-01-C maximum non-orthogonality; H3: B' blade-stack maximum
skewness; H4: B' nose/outer maximum skewness); (ii) no maximum (non-orthogonality, skewness, aspect) of A', B' or FAN-01-C rises by more than 1 %;
(iii) closure (1e-13) and topology on S, A' and FAN-01-C, periodicity (1e-12) on S, A', FAN-01-C, stacks (A' and FAN-01-C 1e-12, B' 4.9e-11; under H3
along the 3-D surface normal as ADR 0079 C5) pass and S, A', B', FAN-01-C build; (iv) the R-G in-range builds do not fall, per fan. **Registered
package**: if H3 and H4 each fail (ii) only, H3 + H4 together are measured once as one change with both targets required in (i). A rejected change is
removed from the code (the ADR keeps its numbers). H5 moves no node and is decided by Part C.

Construction **H** = G + the accepted changes; if at least one change is accepted it gets the case value `c_grid_construction = "camber_seam_h"` (schema
and `docs/case-schema.md`); `camber_seam` stays G, bit-identical. If none is accepted, there is no H and Part C is run on G.

## Part C: acceptance on H (run once, after Amendment 1 freezes H and, for H5, the range)

* **C1** FAN-01-C with H: ADR 0079 P1, P3-P8 (same limits), P2 quality target 70 deg / 2.0 / 300 (unchanged from ADR 0079).
* **C2** P9 (`FvGeometry::new`, `check_mesh` ok, `rules::check_case` without an error-level finding) through the case with `camber_seam_h`.
* **C3** P10 (the ADR 0079 criterion) through the case with `camber_seam_h`, unless Part A finds FLOW (then the corrected set-up of the amendment).
* **C4** registered scan **R-H** (combinations different from the development scan, R-E and R-G): n_chord {15, 22, 30} x n_nose {3, 5, 8} x n_side
  {10, 13, 18, 26} with n_nose + 2 <= n_chord, and front_gap {0.2, 0.28, 0.45, 0.75}, on blade B (S counts otherwise) and FAN-01 (FAN-01-C counts
  otherwise) with H. Gates: (a) every in-range combination builds and validates with positive volumes: 100 % per fan; (b) every out-of-range combination
  is refused with a located error naming a control: 100 %. Reported: the fraction that also meets 70 / 2.0 / 300, the median non-orthogonality, the
  construction steps. R-G is re-run with H and reported beside it (no gate).
* **C5** A' and B' with H: the Q1 gates of ADR 0072 (A' 70 / 2.0 / 300, B' 70 / 2.0 / 3000), reported as ADR 0079 did for G.

## Part D: default behaviour bit-identical

B1: the ADR 0079 hashes (S and A' with `pursuit` and with E) and the G hashes above (S, FAN-01-C) are reproduced exactly after every change; the
default FAN-01 refusal message is unchanged. `crates/rcfd-api/tests/baseline_hashes.rs` untouched and passing; the ADR 0079 tests pass unchanged.

## Not claimed

Fan performance; any comparison with measured FAN-01 data; turbulence; accuracy of the smoke solve (bounded only).

## Amendment 1 (written after Part A and the Part B diagnostics and development, before any registered Part B or Part C run; no limit changed)

### Part A results (test `part_a_p10_cause` in `crates/rcfd-api/tests/fan01_p10_w36d.rs`, each run once; the harness smoke was run A0 itself, which is deterministic)

The recomputed max imbalance equals the solver's `continuity` at every printed iteration (harness check).

| Run | cells | c10 | L (200-300) | L / c10 | L_np / c10_np | f_poor | distinct argmax | largest relative speed |
|---|---|---|---|---|---|---|---|---|
| A0 x1 (FAN-01-C) | 11 536 | 7.915e-4 | 6.619e-3 | **8.36** | 6.69 | 0.42 | 16 | 62.95 m/s |
| A1 x0.75 | 4 920 | 2.514e-3 | 6.81e-5 | 0.027 | 0.027 | 0.00 | 7 | 50.77 |
| A2 x1.25 | 22 320 | 6.392e-4 | 2.631e-4 | 0.41 | 0.51 | 0.00 | 10 | 44.61 |
| A3 x1.5 | 38 184 | 9.103e-4 | 2.369e-4 | 0.26 | 0.21 | 0.52 | 15 | 46.15 |
| A4 x1, nu x10 | 11 536 | 5.262e-4 | 1.156e-2 | **22.0** | 14.9 | 0.40 | 18 | 56.96 |
| A5 x1, 1 500 it. | 11 536 | as A0 | as A0 | 8.36 | 6.69 | 0.42 | 16 | 68.37 |

* A0: the argmax alternates between two cells that are NOT poor: F-block cell (a, b, k) = (6, 6, 2) (one row upstream of the front edge, pitchwise
  between the nose and the upper seam, the first layer above the hub stack; worst face 34.4 deg, aspect 17, volume ratio 6.5 to a neighbour) and wake
  cell (4, 6, 12) (47.2 deg, aspect 66). The momentum residual stays at 0.35-0.43 and the pressure residual at 0.07-0.08 from iteration 70 on: the
  iteration does not converge.
* A5: the continuity oscillates between 1.5e-3 and 5e-3 up to iteration 1 500 with the same argmax cells (for some hundred iterations the seam-row
  cell (30, 9, 8), 73.3 deg): bounded, not monotone growth, not convergence.
* A4: at a tenth of the Reynolds number the same F-block cell is the argmax and the ratio is larger (22.0).

**Verdict by the registered rule: MIXED.** Not MESH (f_poor = 0.42 < 0.5 and L_np > c10_np), not FLOW (A4 still grows). Stated with the numbers: the
miss is a non-converging, bounded oscillation of the steady SIMPLEC iteration located away from the worst-quality faces, in the first span layer above
the hub stack upstream of the leading edge and in the wake at mid-span; it does not disappear at a tenth of the Reynolds number (so it is not the
laminar unsteadiness of the design Reynolds number), and it is specific to this count set: x0.75, x1.25 and x1.5 converge (L / c10 0.03, 0.41, 0.26).
The (a2) mesh of ADR 0079 (same counts, no radial filter) met P10. The cause is a property of this particular mesh that the face quality measures do
not flag; the only feature found at the argmax cell is a volume jump of 6.5 to a neighbour. P10 keeps its role as the acceptance smoke of Part C.

### Part B diagnostics

* **DF** (development hook, not committed): the 19 FAN-01 in-range R-G failures. All wake lines of every failing combination carry the full stack
  with `d1 = h1` (continuous to 1 %): **HF falsified**. Step 0 of the fallback chain folds everywhere (hundreds of quads in the nose rows and the F
  block); several combinations miss only by 2-30 nose quads in rows 2-5 (just beyond the stack) at step 1 or 7; two (n_chord 36 / n_nose 6 /
  n_side 24 and n_chord 18 / n_nose 6 / n_side 14) have no in-plane fold at steps 0-1 and fail as a 3-D tangle (`Mesh::from_cell_faces`); front_gap 0.9
  is the `n_up` refusal at the hub (the front edge 10.5 mm from the inlet).
* **DW** (FAN-01-C, layers 8-21): every wake line has the full stack and `d1` 5.65e-4 to 5.71e-4 m: **HW falsified**; the line-5 skewness is not a
  stack switch.
* **DC** (FAN-01-C, layers 9-20): at the lower seam corner the C side spans 151 deg (seam at -50 deg, front edge at +102 deg) and holds only the
  corner line h_c, which arrives at 94 deg, 7.6 deg from the front edge: the cell (h_c, n_side - 1) is a sliver and its i-face is the 83.26 deg maximum.
  The seam cell next to the corner is 3.5 mm long, the front-edge cell next to it 64 mm.
* **DL** (C5 on B' with E): the 85.89 deg maximum is on hub boundary faces of wake/outer cells (lines 22 and 150-152, rows 17-19, layer 0, node-line
  tilt 57-65 deg): it sits in the wake, where C5 applies the trailing-edge lean factor to every wake line.
* **DN** (C6 on B' with E): the 81.69 deg maximum is the stack-edge j-face of line 71 = h_c - 1, the first line outside the nose lines that C6
  freezes (rows 7/8, mid-span): the hard edge of the frozen region.

### Part B development (disclosed; all on development data)

* Post-smoothing local untangling on the 19 failures: 2 build (n_chord 36 / n_nose 9 / n_side 24 at step 7 with 89.07 deg / 13.2 / 2661, and
  front_gap 0.18 at step 7). FAN-01-C, FD and S are bit-identical with it (hashes `0x00b3e9673d470e52`, FD `0xb6d2a2f4b549dfd5`, S
  `0x59b26da9febc9b2e`).
* C6 on the 19 failures: 3 build.
* H2 (corner): the corner line's last node moved to the bisector of seam and front edge in every smoothing iteration (in full, or by a quarter of the
  rotation per iteration, which converges to the same): FAN-01-C and FD fold at layers 8-9 (line 19, row 8). **No H2 candidate is registered.**
* C5 with the lean factor faded along the wake on B' with E: 77.38 / 7.081 / 8531.6 (E 78.17 / 7.084 / 8531.6), blade-stack skewness 2.057 -> 1.098.
* C6 with the frozen rows tapered by one row per line: B' 83.90 deg (worse; the edge moves, it does not vanish). C6 as a smooth post-blend on B' with
  E: 78.17 / 4.847 / 8518.7, nose/outer skewness 7.084 -> 1.311.

So the B' measurements of H3 and H4 below are not blind (developed on B' with E; the registered runs are on G); A', FAN-01-C and R-G are not known.

### The changes, frozen (new opt-in fields of `CGridOptions`); order H1, H3, H4 on top of G

* **H1 `post_untangle`**: after the in-plane smoothing of a layer, the local untangling that precedes it (`UNTANGLE_PASSES`, free nodes around folded
  quads to the mean of their four neighbours) is applied once more. Target: FAN-01 R-G in-range builds (G: 20 of 39).
* **H3 `lean_stack`**: C5 of ADR 0079 (every blade wall stack built with the in-plane offsets divided by `cos(lambda)`, lambda between the in-plane
  wall normal and the 3-D surface normal) on the blade lines; on the wake lines the factor is `c_te + (1 - c_te)(1 - fade(p / max(1, n_down / 4)))`, p
  the wake line's distance from the trailing edge in lines, so it is the trailing-edge value next to it and 1 from a quarter of the wake on. Stacks
  (iii) along the 3-D surface normal as registered. Target: B' blade-stack maximum skewness (G value measured in the run).
* **H4 `nose_blend`**: after the smoothing of a layer (no freezing inside it), the C-line nodes of rows `n_b < j < n_side` move back towards the
  algebraic grid by `w = fade(dist / n_nose) fade((j - n_b - 1) / max(1, (n_side - n_b) / 4))`, dist the line's distance in lines from the nose lines
  `[h_c, n_i - h_c]` (0 on them), `fade` the smooth step from 1 to 0. Target: B' nose/outer maximum skewness.
* H2: none (above). The registered H3 + H4 package applies as registered (only if each fails (ii) alone).
* **H5** (range of H, checked with `c_grid_construction = "camber_seam_h"` only, located errors before any construction): `0.15 <= n_nose / n_chord
  <= 0.3`; `max(8, n_bl + 6) <= n_side <= min(2 n_chord, 16)`; `0.15 <= front_gap <= 0.6`. Chosen from the R-G FAN-01 failures (14 of the 19 have
  n_side 24 or 40, one has n_nose / n_chord 0.375, one front_gap 0.9). It is a restriction, not a fix. Under it, the R-H list has 11 in-range
  combinations per fan (n_chord / n_nose (15, 3), (22, 5), (30, 5), (30, 8) x n_side {10, 13}, and front_gap {0.2, 0.28, 0.45}).

## Amendment 2 (written after the registered Part B sequence, before any Part C run; no limit changed)

### Part B: the registered sequence (test `part_b_step`, each step run once; about 12 min each)

Harness check (rule (d)): the baseline step G reproduces the G hashes of the registration (S `0x59b26da9febc9b2e`, FAN-01-C `0x00b3e9673d470e52`)
and the ADR 0079 numbers (A' 66.18 / 0.889 / 394.4, B' 80.94 / 8.897 / 8805.0, FAN-01-C 83.26 / 3.978 / 227.2); B1 of ADR 0079 (S and A' with
`pursuit` and E) reproduced on the code with the new options. R-G in-range builds with G: blade B 36 of 39, FAN-01 20 of 39 (ADR 0079 reported blade B
"35 of 38"; the G range admits 39 blade B combinations of the R-G list, and 36 build on this code: a counting discrepancy in the ADR 0079 summary,
not a change of code; FAN-01 agrees).

| Step | Target measure | A' / B' / FAN-01-C maxima (non-orth, skewness, aspect) | (iii) closure, periodicity, stacks | R-G builds B / FAN-01 | Decision |
|---|---|---|---|---|---|
| G (baseline) | - | 66.18 / 0.889 / 394.4; 80.94 / 8.897 / 8805.0; 83.26 / 3.978 / 227.2 | pass | 36 / 20 | - |
| + H1 `post_untangle` | FAN-01 R-G builds 20 -> **22** (+10 %) | all identical (S, A', B', FAN-01-C hashes unchanged) | pass | 36 / 22 | **ACCEPTED** |
| + H3 `lean_stack` | B' blade-stack skewness 2.057 -> **1.098** | 66.18 / 0.888 / 394.4; 80.93 / 8.894 / 8805.0; 83.26 / 3.978 / 227.2 (none up by more than 1 %) | pass (wall stacks along the 3-D normal: A' 1.5e-13, FAN-01-C 6.1e-14, B' 5.0e-12; in-plane form 0.24 by design) | 36 / 22 | **ACCEPTED** |
| + H4 `nose_blend` | - | S does not build (3-D tangle, internal face 649, `mesh`) | **fails** (S must build) | - | **REJECTED** (iii); removed |

The H3 + H4 package does not apply (H4 failed (iii), not only (ii)). Note: with G the B' non-orthogonality maximum (80.94 deg) is a blade-stack face;
H3 moves it by 0.01 deg only (on E it had moved 78.17 -> 77.38 in development), so the trailing-edge lean is not the cause of the G maximum.

### Construction H, frozen

H = G + H1 `post_untangle` + H3 `lean_stack` + the H5 range (`range_h`), `CGridOptions::camber_seam_h_construction()`, case value
`c_grid_construction = "camber_seam_h"`. `camber_seam` stays G (B1 test of the G hashes). Part C runs once on H as registered. Prediction (from the
step table, FAN-01-C with H differs from G only in the blade stacks): C1 P2 MISSED (about 83 deg / 4.0), C5 A' aspect and B' all three MISSED.

## Amendment 3 (written after the Part C runs, before the corrected C5 run; no limit changed)

C5 as registered ("A' and B' with H") is a defective set-up created by Amendment 1: the H5 range excludes both (A' n_side 20 > 16; B' n_side 30 > 16
and n_nose / n_chord 0.33 > 0.3), so `c5_q1_gates_with_h` refused them with located range errors and measured nothing. It stays ignored as a
defective set-up. **Corrected C5b**: A' and B' with the mesh options of H and the range check off (`checked_range: false`), the same Q1 gates.
What I already know: these are the meshes of the Part B step G + H1 + H3 (the G range admits A' and B'), measured there: A' 66.18 / 0.888 / 394.4,
B' 80.93 / 8.894 / 8805.0, so C5b is not blind; prediction: A' aspect MISSED, B' all three MISSED.

## Addendum: measured values, misses and deviations

Release builds, scratch tree of this wave on c89495f; each registered run once. Code state of the measurements: record commit bd215ce (H with the
H5 range); H5 was then rejected by its registered acceptance and removed in the next commit (below), which moves no node. Limits are the registered ones, unchanged. No external reference
value enters any gate. No fan performance claim. Solver physics files were not touched.

### Part A: the cause of the P10 miss

Verdict by the registered rule: **MIXED** (table in Amendment 1). The P10 growth on FAN-01-C is a bounded oscillation of the steady SIMPLEC iteration
that never converges (momentum residual 0.35-0.43 throughout; continuity 1.5e-3 to 5e-3 up to iteration 1 500); its largest cell imbalance sits on
cells that are NOT poor by the face measures (F-block cell (6, 6, 2) above the hub stack upstream of the leading edge, 34 deg, volume ratio 6.5 to a
neighbour; wake cell (4, 6, 12), 47 deg); it persists at a tenth of the Reynolds number (L / c10 22.0), so it is not the laminar unsteadiness of the
design Reynolds number; and it is specific to this count set: x0.75, x1.25 and x1.5 converge (L / c10 0.027, 0.41, 0.26). Refinement trend of L / c10
(x0.75, x1, x1.25, x1.5): 0.027, 8.36, 0.41, 0.26: no monotone trend; the x1 mesh is the outlier. Not established: which property of the x1 mesh
causes it (no quality measure tried flags those cells).

### Part B

H1 `post_untangle` ACCEPTED (FAN-01 R-G builds 20 -> 22, every other mesh bit-identical), H2 none registered (the corner rule tried folded the
tip layers), H3 `lean_stack` ACCEPTED (B' blade-stack skewness 2.057 -> 1.098, no maximum up by more than 1 %), H4 `nose_blend` REJECTED (S does not
build with it) and removed; table in Amendment 2. Development diagnostics (Amendment 1): HF and HW falsified (no wake-stack switching); the FAN-01-C
non-orthogonality maximum is the lower seam corner of the tip layers, where the corner line arrives 7.6 deg from the front edge.

### Part C: acceptance on H = G + H1 + H3 + H5 range (case value `camber_seam_h`), as registered

| Check | Limit | Measured | Status |
|---|---|---|---|
| C1 P1 builds, validates, volumes > 0 | - | 11 536 cells, step 0 | met |
| C1 P2 non-orthogonality / skewness / aspect | 70 / 2.0 / 300 | **83.26** / **3.978** / 227.2 | **MISS** (`c1_p2_fan01_h_quality_gates_known_miss`), as predicted |
| C1 P3 closure, topology | 1e-13 | 7.97e-16, ok | met |
| C1 P4 periodicity | 1e-12 | 1.48e-13 | met |
| C1 P5 stacks (radial; wall along the 3-D normal) | 1e-12 | 5.6e-14; 6.1e-14 (in-plane form 0.12 by design) | met |
| C1 P6 bladed vs empty C nodes, volume identity | 0, 1e-11 | 0, 1.3e-16 | met |
| C1 P7 wall nodes on the exact surface | 1e-12 m | 0 m | met |
| C1 P8 solid volume against the section-area quadrature | reported | -2.5e-3 | - |
| C2 P9 FvGeometry, check_mesh, rules | accepted, ok, no error | accepted, ok, SV101 warning only | met |
| C3 P10 laminar smoke | max over 200-300 <= value at 10 | 7.957e-4 at 10, **7.456e-3** over 200-300; finite, no failure, 62.1 m/s | **MISS** (`c3_..._known_miss`) |
| C4 (a) R-H in-range builds | 100 % per fan | blade B 11 of 11; FAN-01 **10 of 11** (front_gap 0.45: 3-D tangle, C cell (37, 4), layer 11) | **MISS** for FAN-01 (`r_h_in_range_combinations_build_known_miss`) |
| C4 (b) R-H out-of-range refused, located | 100 % | 58 of 58 | met |
| C4 reported: meet 70 / 2.0 / 300; median non-orth; steps | - | blade B 10 of 11, 61.8 deg, all step 0; FAN-01 0 of 11, 84.6 deg, 4 step 0 and 6 step 7 | - |
| C4 reported: R-G with the H mesh options (G range) | - | blade B 36 of 39, FAN-01 22 of 39 (G: 36, 20) | - |
| C5 A' / B' with H | - | refused (outside the H range) | **defective set-up** (Amendment 3) |
| C5b A' with the H mesh options | 70 / 2.0 / 300 | 66.18 / 0.888 / **394.4** | aspect **MISS** (`c5b_..._known_miss`) |
| C5b B' with the H mesh options | 70 / 2.0 / 3000 | **80.93** / **8.894** / **8805.0** | **MISS** |

### Verdicts after Part C

* **H5 (the narrower range) REJECTED and removed.** It was registered to be "decided by Part C"; C4 (a) missed for FAN-01 (10 of 11), so by preamble
  rule (b) (no keep clause was registered for H5) it is removed: `range_h` and `control_range_problems_h` are gone, `camber_seam_h` checks the range of
  `camber_seam`, and the R-H tests are removed with it (their numbers above stay recorded; C4 (b) 58 of 58 was met on H5). The meshes of C1-C3 and C5b
  are unchanged by the removal (a range check moves no node; FAN-01-C, A' and B' are inside the G range), so nothing is re-measured.
* **H is kept** (G + H1 + H3) because the Part B rule that accepted H1 and H3 was registered with the keep clause "Construction H = G + the accepted
  changes ... gets the case value `camber_seam_h`"; it is kept although C1 P2, C3 (P10) and, with H5, C4 (a) on FAN-01 missed. What H improves is
  limited: of the two FAN-01 combinations H1 gained in the registered run (n_chord 36 / n_nose 9 / n_side 24 at step 7, 89.07 deg / 13.19 / 2661; and
  front_gap 0.18 at step 7, 83.36 deg / 1.50 / 224), the first is above the 85 deg error threshold of `check_mesh`, so it is a mesh the solver path
  refuses. H3 is not inert on FAN-01-C: its hash changes (`0x21104bbbdd9b9129` against G `0x00b3e9673d470e52`), the maxima agree to four digits, and the
  P10 ratio moves from 8.36 (G, A0) to 9.37 (H, C3).

### Part D: default behaviour

B1 met: the ADR 0079 hashes of S and A' with `pursuit` and E, and the G hashes of S (`0x59b26da9febc9b2e`) and FAN-01-C (`0x00b3e9673d470e52`), are
reproduced on the final code (`b1_existing_constructions_are_bit_identical`, `b1_construction_g_is_bit_identical`); the default FAN-01 refusal
message is unchanged. `baseline_hashes.rs` untouched (final gate below). The new options are off by default and only `camber_seam_h` sets them.

### What this wave delivered, in short

The P10 miss is diagnosed as a mesh-specific, non-converging oscillation located away from the worst faces and not caused by the Reynolds number
(MIXED by the rule; the property of the x1 mesh that causes it is not identified); H adds two FAN-01 control combinations (one of them unusable,
89 deg) and halves the y+ = 1 blade-stack skewness without moving any other maximum by more than 1 %; the FAN-01 quality target, the smoke criterion
and robustness on FAN-01 are still missed; the narrower range H5 was rejected and removed.

### Deviations from the registered plan (candid list)

* The Part A harness smoke was run A0 itself (deterministic; no choice involved). The verdict rule was applied as registered.
* H2 was not specified: the one corner rule tried in development folded the tip layers (Amendment 1).
* The H3 and H4 forms were developed on B' with construction E after the DL / DN diagnostics, so their registered B' measurements were not blind
  (A', FAN-01-C and R-G were).
* The H5 range was chosen after seeing the R-G failures (registered before R-H ran); it made the registered C5 set-up defective (A' and B' outside
  the range), corrected by C5b, whose result was known from Part B (Amendment 3). H5 was then rejected and removed (above); the first draft of this
  addendum had kept it as "a restriction", which the rules do not allow without a registered keep clause.
* ADR 0079 reported blade B R-G "35 of 38" in range; on this code G gives 36 of 39 (the G range admits 39 blade B combinations); FAN-01 agrees (20 of 39).
* The H4 measurement stopped at the first failure (S did not build), so its A', B' and FAN-01-C numbers were not recorded.
* The development hook that lists all folded quads and the C5 / C6 / corner development variants were not committed.

### Unverified references

Thompson, Warsi & Mastin (1985) for the elliptic smoother (recalled, as ADR 0072 / 0079). Nothing else.
