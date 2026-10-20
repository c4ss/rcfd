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
