# ADR 0702: F-784 registration: ramp length of the S'' wake-stack remap from the wake line spacing

Status: **REGISTERED 2026-11-17, written before any code of F-784 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-784 was written; the only builds were the two disclosed probes on a
`git archive` copy of the base (below). Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-784 (deps F-782), proposed by [ADR 0699](0699-f782-registration-wake-stacks-of-construction-s-prime-at-the-thin-end-wall-layers.md)
"For the lead" (follow-up) and its remaining questions 1 and 3. Parents: ADR 0699 (construction S'', `wake_stack_remap`,
`CGridOptions::camber_seam_s_wake_remap_construction()`, the ramp `omega_p = 1 - fade(p / max(n_down / 4, 1))` in `write_layer`,
the test file `f782_cgrid_wake_remap.rs`), [ADR 0696](0696-f777-registration-construction-s-without-the-first-cell-stretch-on-strongly-staggered-blades.md)
(S', the second blade B2, the front_gap / n_nose scan SC), ADR 0692 (S), ADR 0118 (the s4 specs), ADR 0072 (W1, the remapped
radius), ADR 0083 (H3, the smooth step `fade`). Number 0702 given by the lead. Branch `f784-work` from 6642beb3 (F-726 / ADR 0692,
F-777 / ADR 0696 and F-782 / ADR 0699 merged), worktree `w49i-work/f784`, scratch `w49i-work/f784-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact and the measured number); each registered run made once; existing tests and ignored records not
edited; defaults bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new
dependency, no unsafe code; clean-room. The roadmap files are not edited. Every choice below that the maintainer would normally
be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).

Clean-room: elementary calculus (the slope bound of the cubic smooth step, the mean value theorem), the repository's own
constructions (ADR 0072 W1, ADR 0083 `fade`, ADR 0699 S'') and quality measures (`quality.rs`). No source outside the repository
was read.

## What F-784 asks

> Every limit not stated here is registered in the ADR before the run. A new opt-in field of S'' sets the length of the trailing-edge ramp of wake_stack_remap from the wake line spacing so that, at the ADR 0118 s4 specs, ComputerFan::rom_reference() blade 0 and the ADR 0696 second blade build with no folded cell within 84.5 deg non-orthogonality (the 84.953 and 84.472 deg of ADR 0699 at the first ramp cell disclosed), skewness 4.0 and aspect 1000, the wake-stack spanwise skewness stays within 1.0, a registered front_gap and n_nose scan inside the camber_seam range builds a registered fraction, and camber_seam_h, camber_seam_s, camber_seam_s_unstretched and camber_seam_s_wake_remap are unchanged in bits.

Clauses: **c1** every limit registered here; **c2** a new opt-in field of S'' that sets the ramp length from the wake line spacing;
**c3** REF (reference blade 0) and B2 (the ADR 0696 second blade) at s4 build with no folded cell; **c4** non-orthogonality <= 84.5
deg on both; **c5** skewness <= 4.0 and aspect <= 1000 on both; **c6** wake-stack spanwise skewness <= 1.0 on both; **c7** a
registered front_gap / n_nose scan inside the camber_seam range builds a registered fraction; **c8** camber_seam_h, camber_seam_s,
camber_seam_s_unstretched and camber_seam_s_wake_remap unchanged in bits. From the brief: the S'' hashes computed on a `git archive`
copy of the base before any change; the named existing tests rerun; negative controls and mutations registered.

## What was read and computed before this text (disclosed)

Read: ADR 0699 (all), `fan_cgrid.rs` (`CGridOptions`, `fade`, `remapped_radii`, `write_layer`, the wake distribution and stacks of
`build_layer`, the layer loop of `build_cgrid_mesh_with`), the test file `f782_cgrid_wake_remap.rs` (its helpers are reused), the
SC rows of ADR 0696.

**Probes on the base (disclosed; `f784-scratch/base` = `git archive 6642beb3`, one added test file `zz_probe_base.rs`, logs
`f784-scratch/probe.log`, `probe2.log`; each run once).** They build only S' and S'' (existing constructions); no code of F-784
existed. They print:

- **The S'' hashes (c8, as ADR 0699 amendment 1 did for S'):** REF `0x63011aa8288ac9c0`, B2 `0x769e2e330b0cf295`, 12 841 cells,
  construction step 5 on both (step 5 is the smoothing displacement scaled by 0: the meshes are the algebraic grids).
- **The wake line spacing** (3-D distance between neighbouring wake lines along the wake cut `j = 0`; the same on both sides).
  REF, node layers 0 to 3 (hub, thin): 2.26e-4 to 2.28e-4, 1.46e-3, 6.50e-3 to 6.53e-3, 1.13e-2, then 1.08e-2 m for `p = 0..1,
  1..2, 2..3, 3..4, ...`; node layer 4: 3.23e-4, 1.87e-3, 8.19e-3, 1.29e-2; node layers 7 to 10 (tip, thin): 5.22e-4, 2.81e-3,
  1.26e-2, 2.18e-2, 1.13e-2. B2: hub 2.51e-4 to 2.53e-4, 1.44e-3 to 1.45e-3, 6.19e-3 to 6.22e-3, 1.14e-2; tip 5.71e-4, 2.66e-3,
  1.08e-2, 2.22e-2. So the cumulative distance from the trailing edge `x_p` is, at the hub / tip: REF `x_1` 2.28e-4 / 5.22e-4, `x_2`
  1.69e-3 / 3.33e-3, `x_3` 8.18e-3 / 1.59e-2, `x_4` >= 1.95e-2 m; B2 `x_1` 2.53e-4 / 5.71e-4, `x_2` 1.70e-3 / 3.23e-3, `x_3` 7.92e-3 /
  1.40e-2, `x_4` >= 1.93e-2 m.
- **The S'' node shift against S' per wake line** (max over the rows; the stack rows `j <= n_bl` carry the maximum, the far rows
  are not larger except at the outlet line, 7.7e-7 against 6.1e-7 m). With `omega = 1` (p >= 2) this is `|b - a|` itself. REF:
  largest 8.11e-5 m (node layer 7, p = 3); node layer 3: 4.40e-5 / 5.07e-5 m (p = 2 / 3), at p = 1 (omega 0.5) 2.03e-5 m; node
  layer 4 at p = 1: 6.1e-6 m. B2: largest 8.97e-5 m (node layer 7, p = 3); node layer 3 at p = 1: 2.36e-5 m. The hub, tip and gap
  layers do not move.
- **Non-orthogonality near the trailing-edge lines** (faces with an owner or neighbour among the C cells 56 to 59 and 4 to 7).
  The face between C (56, 2) and (56, 3) at layer 3 (the first wake cell, between the trailing-edge line 56 and line 57): S'
  **84.390** (REF) / **83.554** (B2) deg; S'' 84.953 / 84.472 (ADR 0699). Next on S': REF 84.337 (C (56, 2) / (56, 3), layer 5),
  83.988 (layer 6), 83.889 (C (57, 2) / (57, 3), layer 5); B2 83.800 (C (56, 2) / (56, 3), layer 5), 83.353, 83.353. On S'' the
  faces of the second wake cell (line 57 to 58, where omega rose from 0.5 to 1) fell or stayed (REF C (57, 2) / (57, 3) layer 5:
  83.889 -> 83.672).
- **The global non-orthogonality maxima of S' and S''**, outside the wake: REF **84.688** deg at C (45, 2) / (45, 3) layer 3, and
  further blade-line faces 84.686 (line 46), 84.657 (47), 84.648 (44), 84.612 (48), 84.612 / 84.604 (46 / 45, layer 5), the same
  values on S' and on S'' (the remap does not move the blade lines). B2: 84.218 at C (45, 2) / (45, 3) layer 5, then 84.217 and
  below. ADR 0699 already listed 84.688 / 84.218 as the S' maxima.
- **The wake-stack spanwise skewness per wake cell** (the ADR 0699 W5 measure, max over the stack rows and layers). S': REF
  cells `p` 3..4 **3.2404** / 3.2175 (layer 8), cells 2..3 **1.4762** / 1.4690 (layer 8), cells 1..2 0.131, cells 0..1 0.117,
  cells 4..5 to 7..8 0.229 to 0.234 (layer 4); B2 4.0112 / 3.9899, 1.1548 / 1.1495, 0.140, 0.133, 0.245 to 0.256. S'': every cell
  0.117 to 0.235 (REF), 0.133 to 0.257 (B2), the maxima at layer 4.

Two consequences, stated before the design:

1. **The REF global clause of c4 cannot be met by a ramp.** The blade-line faces at 84.688 deg (and five more above 84.5) are not
   on a wake line; a rule that only changes the wake-line weights leaves them in bits (the W7 locality of ADR 0699). The global
   84.5 check of REF is therefore **predicted to miss at 84.688 deg** and is registered as such (R2-REF below); the item cannot be
   proposed done by this design. The part of c4 the field controls, the faces of the wake cells, is registered separately (R2w).
2. The first wake cell's face starts at 84.390 deg on S' (REF), so the ramp may add at most 0.11 deg there.

## Diagnosis that the design rests on

ADR 0699's ramp is a fixed count of lines (`omega` 0, 0.5, 1 at `p` 0, 1, 2). The first wake cell is the shortest (2.3e-4 to 5.7e-4
m along the wake), and `omega` jumps by 0.5 across it, so the node of line 57 moves by `0.5 |b - a|` (2.0e-5 m at node layer 3)
next to an unmoved node: the in-plane edge rotates by up to `atan(2.03e-5 / 2.28e-4)` = 5.1 deg (REF; B2 5.3 deg). The measured
rise of the first-cell face was 0.563 deg (REF) and 0.918 deg (B2), i.e. 0.11 and 0.17 deg per degree of that edge rotation
(the face normal turns only by the part of the shift across the face). The second cell is 6 times longer and the rise there was
negative. So the rise is set by the change of `omega` per cell times the shift divided by the cell's length along the wake.

## Design (fixed now): construction S''', `camber_seam_s_wake_ramp_construction`

One new field of `CGridOptions`, `wake_ramp_from_spacing: bool`, default `false` (so `Default`, every existing constructor and
every struct literal with `..` are unchanged), acting only together with `wake_stack_remap`. A new constructor
`CGridOptions::camber_seam_s_wake_ramp_construction()` = `camber_seam_s_wake_remap_construction()` + `wake_ramp_from_spacing:
true` (S'''). A new public field of `CGridMesh`, `wake_ramp_length: Option<f64>` (the ramp length `L` used, `None` without the
field), for the report and the checks.

With the field, in each fallback attempt:

1. Every layer's grids `a` (true section) and `b` (remapped radius, where it exists) are built as before; the writing of the
   layers is deferred to after the loop (without the field the loop is unchanged and writes immediately).
2. **D** = the largest `|b - a|` in the layer's physical in-plane coordinates `(z, r_k theta)`, over every span layer that has a
   grid `b`, every C wake line `p >= 1` (both sides) and every stack row `j <= n_bl`: the largest shift the remap makes (the
   probe: 8.11e-5 m REF, 8.97e-5 m B2). One `D` for the mesh, so that every layer uses the same ramp length.
3. **L** = `1.5 D / tan(alpha)`, `alpha` = **1 deg** (registered constant `WAKE_RAMP_TILT_DEG`), at least 1e-300 m.
4. In `write_layer`, on every C wake line, `omega_p = 1 - fade(x_p / L)` instead of the ADR 0699 count rule, with `x_p` the
   distance along the wake from the trailing edge to line `p` in the layer's true section: the sum over `q = 1..p` of the
   physical distance between the wake-cut nodes (`j = 0`) of lines `q - 1` and `q` of `a` (the wake cut is shared by the lower and
   upper lines, so both sides get the same `omega`). Everything else of the S'' rule is unchanged (`a' = a + omega (b - a)` for
   `j <= nb`, `b + w (a' - b)` beyond).

Derivation. Write `f(t) = 3 t^2 - 2 t^3` (so `omega = f(x / L)` on `[0, L]` and 1 beyond). `f'` is at most 1.5 (at `t = 1/2`),
so by the mean value theorem the change of `omega` across a wake cell of length `s_p` is at most `1.5 s_p / L`. The node shift is
`omega (b - a)`; across the cell its change is `Delta omega (b - a) + omega Delta (b - a)`, and the second part is the remapped
grid's own line-to-line structure (at `omega = 1` the line is `b`'s line), so the rotation the ramp adds to an in-plane edge of the
cell is at most `atan(Delta omega |b - a| / s_p)` <= `atan(1.5 D / L)` = `alpha`, whatever the spacing. The ramp length thus
follows from the spacing: where the cells are short, `omega` changes little per cell. The smooth step also starts flat (`f(t) =
3 t^2` for small `t`), so the first cell, whose face starts closest to the limit, gets far less than `alpha`: at the hub `x_1 / L`
is 0.033 (REF) and 0.033 (B2), `omega_1` 0.0032 / 0.0032. `alpha` = 1 deg: from the diagnosis the face rises by at most 0.17 deg
per degree of edge rotation, so `alpha` bounds the rise anywhere on the ramp by about 0.17 deg (the faces of the second and third
wake cells start at 83.9 deg or less on S', both blades); a smaller `alpha` lengthens the ramp onto the long cells at the tip, where
the thin-layer shear of the true section returns (S' 1.48 / 1.15 at cells 2..3, 3.24 / 4.01 at cells 3..4), and `alpha` = 1 deg
keeps `omega_3` = 1 on every layer of both blades (below). Taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation); it is fixed now and not searched.

Predicted ramp (from the probe numbers; `L` = 85.93 D): REF `L` = 6.97e-3 m, B2 `L` = 7.71e-3 m.

| | `omega_1` hub / tip | `omega_2` hub / tip | `omega_3` hub / tip |
|---|---|---|---|
| REF | 0.0032 / 0.016 | 0.148 / 0.467 | 1 / 1 (`x_3 / L` 1.17 / 2.3) |
| B2 | 0.0032 / 0.016 | 0.124 / 0.380 | 1 / 1 (`x_3 / L` 1.03 / 1.8) |

So S''' differs from S'' only on the wake lines `p = 1, 2` of the span layers (`omega` exactly 1 for `x_p >= L`, as on S'' for
`p >= 2`, so the lines `p >= 3` are written by the same expression with the same operands: equal in bits; the margin is smallest on
B2 at the hub, `x_3 / L` = 1.03, so the locality check uses `p >= 4` and `p = 3` is reported).

Cost (not limited): on the lines `p = 1, 2` the thin layers keep `1 - omega` of the true-section drift (S' about 1.5e-5 to 3.3e-5 m
per thin layer), against ADR 0699's 0.5 and 0.

## Development protocol (registered)

The code is written to this design. Before the registered run, development builds are allowed only to fix defects (code that
does not do what this design says) and print only the build status, the construction step and `L`; no quality metric is computed
or printed in development. No design variant is permitted: if S''' misses, the miss is the result. Every development build is
listed in an amendment.

## Common set-up (fixed now)

The s4 specs, `REF`, `B2`, the fold measure, periodicity, hash, `locate` and the W5 / W6 measures of `f782_cgrid_wake_remap.rs`
are copied into the new test file `crates/rcfd-mesh/tests/f784_cgrid_wake_ramp.rs`. `S''' = camber_seam_s_wake_ramp_construction()`.
Quality: `quality_summary` (maxima over every face and cell) and `compute_metrics` (per face). The **wake-cell faces** are the
internal faces with an owner or neighbour among the C cells `(i, j, k)` with `i < n_down` or `i >= n_i - n_down` (every row, every
layer). The **first ramp face** is the face between C (56, 2) and C (56, 3) at layer 3 (for s4, `n_i` 64, `n_down` 8).

## Checks (limits fixed now)

| id | clause | what | limit | estimate behind it |
|---|---|---|---|---|
| R1a | c3 | `build_cgrid_mesh_with(&REF, &s4, &S''')` | Ok, cells > 0 | the change is a smaller shift than S'' on the lines p = 1, 2; step 5 predicted (as S''), reported |
| R1b | c3 | no folded cell: every corner Jacobian of the C- and F-block hexes of one sign, none zero; min volume > 0 | met | as R1a |
| R1c | c3 | periodicity of the seams | <= 1e-12 m | as ADR 0699 W1c |
| R2-REF | c4 | max non-orthogonality (all faces), REF | <= 84.5 deg | **predicted MISS: 84.688 deg** at C (45, 2) / (45, 3) layer 3, a blade-line face the field does not touch (disclosed). Written as the ignored record `r2_ref_global_non_orthogonality_known_miss` from the start (the miss is predicted here) and run once with `--ignored` |
| R2-B2 | c4 | max non-orthogonality (all faces), B2 | <= 84.5 deg | predicted 84.218 (blade line 45, layer 5, unchanged) |
| R2w | c4 (the field's part) | max non-orthogonality over the wake-cell faces, REF and B2 | <= 84.5 deg | REF about 84.40 (the first ramp face, S' 84.390 + about 0.006), B2 about 83.81 (S' 83.800 at layer 5) |
| R2t | c2 (mechanism) | the first ramp face: S''' minus S' (non-orthogonality) | <= 0.03 deg on both | the linear rise `0.563 / 0.5` (REF) and `0.918 / 0.5` (B2) deg per unit omega times `omega_1` 0.003 to 0.006 of node layers 3 and 4: 0.004 / 0.009 deg |
| R3 | c5 | max skewness (all faces) | <= 4.0 | REF 2.7609, B2 2.8312 (ADR 0699, outside the wake, unchanged) |
| R4 | c5 | max aspect ratio | <= 1000 | 525.6 / 551.7 (gap layer 16, unchanged) |
| R5 | c6 | max wake-stack spanwise skewness (ADR 0699 W5 measure) | <= 1.0 on both | the tip cells 2..3 (S' 1.48 / 1.15, S'' 0.13 / 0.15) with line `p = 2` at `omega` 0.47 / 0.38 and line 3 at 1: about half the S' value or less, 0.3 to 0.8; elsewhere the S'' values 0.12 to 0.26 |
| R6 | c2 | the ADR 0699 W6 drift (thin span cells, stack rows) over the wake lines `p >= 3` | <= 5e-6 m on both | `omega_3` = 1 everywhere: the S'' values 6.5e-7 / 7.2e-7 m |
| R7 | c2 (locality) | on the algebraic grids (`smoothing: false`) of S''' and S'' of REF and of B2: every grid node outside the C wake lines `p = 1, 2, 3` of the node layers 1 to `n_span - 1` equal in bits | every node | `omega` differs only where `x_p < L`; nodes of `p = 3` that differ are counted and reported |
| R8 | c2 | `wake_ramp_length` of the S''' meshes | REF in [6.5e-3, 7.5e-3] m, B2 in [7.2e-3, 8.2e-3] m; `None` on S'' | 85.93 D with the probe's D (8.11e-5 / 8.97e-5 m); the band is the 3-D chord versus the in-plane distance and the far-row exclusion, about 1e-3 relative, widened to 7 % |
| SC | c7 | scan of S''' inside the camber_seam range: REF and B2, `front_gap` in {0.15, 0.3, 0.6, 1.0} x `n_nose` in {4, 6, 9}, the other s4 values; "builds" = R1a, R1b and R1c met | per blade at least 9 of 12 (fraction >= 0.75) | ADR 0696 built 12 of 12 of S' per blade; S''' moves only wake nodes, by less than S'' does. Quality per point reported, not a gate. Heavy (24 builds, about 2 to 3 min): an ignored record `sc_the_scan_heavy`, run once with `--ignored`; the main tests cover the same code path |
| SC0 | c7 | every scan point inside the range (`control_range_problems` empty) | 24 of 24 | ADR 0696 SC0 |
| HS | c8 | S on REF: FNV-1a mesh hash | `0x0105a2fcca924d23` | the new code is behind the new field |
| HS' | c8 | S' on REF and B2 | `0x0d257b2fcee093ca` / `0xefc53b8c413a845b` (ADR 0699) | as HS |
| HS'' | c8 | S'' on REF and B2 | `0x63011aa8288ac9c0` / `0x769e2e330b0cf295` (the base probe above) | as HS; the deferred writing is only on the new field's path |
| HB1 | c8 | `cgrid_w35d` b1 tests (G and H hashes) | pass | |
| HB2 | c8 | `f726_cgrid_staggered` `b2_camber_seam_h_on_the_default_computer_fan_is_bit_identical` | pass | |
| E1 | c8 | by name, the non-ignored tests of `f782_cgrid_wake_remap`, `f777_cgrid_unstretched`, `f726_cgrid_staggered`, `cgrid_w35d`; `f722_rom_reference_fan` (rcfd-post); `fan01_p10_w36d` `c2_fan01_h_mesh_is_accepted_by_the_solver_and_the_rules` (rcfd-api); `fan_design_w46a` `s4_realised_default_meshes` | pass | no line they run changes |
| E2 | c8 | `cargo test -p rcfd-mesh --release --lib` | 0 failed (77 passed, 2 ignored, ADR 0699) | |
| E3 | c8 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass (3 passed, 1 ignored) | `fan_cgrid.rs` is reached from the fan case builder |
| Rep | reported | S''' on REF and B2: step, cells, `QualitySummary`, the maxima located, the five largest wake-stack spanwise skewness faces, `omega_p` of lines p = 1..3 at node layers 1, 3, 4, 7, 9, the drift on lines p = 1, 2 | none | |

### Negative controls

| id | control | limit |
|---|---|---|
| NC-R2t | the R2t measure on S'' | REF 84.953, B2 84.472 at the first ramp face (to 1e-3): the measure sees the ADR 0699 rise |
| NC-R5 | the R5 measure on S' | REF 3.2404, B2 4.0112 (to 1e-4) |
| NC-R7 | the R7 comparison restricted to the lines p = 1, 2 | some node differs on both blades (the field does something) |
| NC-R8 | `wake_ramp_length` of an S'' mesh | `None` |

## Planned mutations (scratch copy of the worktree, each run once against the new test file, HB1 and HB2)

| id | mutation | expected to be caught by |
|---|---|---|
| MU1 | the new field ignored in `write_layer` (S''' writes the S'' ramp) | R2t (0.56 / 0.92 deg), R2w on REF (84.953), NC-R7 |
| MU2 | `x_p` replaced by `p` times the first spacing `x_1` (uniform-spacing assumption) | R6 and R5 (with `L / x_1` of 13 to 30, `omega_3` <= 0.2: the S' drift and skewness return), R7 (lines p >= 4 differ) |
| MU3 | `tan(alpha)` with `alpha` = 1 read as radians | R2t (`L` = 7.8e-5 m, `omega` about 1 from p = 1: the rise of about 1.1 / 1.8 deg) and R2w on REF, R8 |
| MU4 | the smooth step replaced by the linear ramp `omega = min(1, x_p tan(alpha) / D)` | R2t on B2 (`omega_1` 0.049, rise about 0.09 deg); on REF the rise is about 0.055 deg, also above 0.03 |
| MU5 | the ramp reversed (`omega = fade(x_p / L)`: 1 at the trailing edge, 0 downstream) | R6 and R5 (the S' drift and skewness return on p >= 3), R7, likely R1b (ADR 0699 MU2 folded the cells around the trailing edge) |
| MU6 | an existing expression on the default path: the blend length `0.05 a.chord` -> `0.051 a.chord` in `write_layer` | HS, HS', HS'', HB1, HB2 |

## Decision rule

- **done**: every check met (R1a to R8, SC, SC0 on both blades, HS to HB2, E1 to E3, the negative controls). Not reachable by
  this design: R2-REF is predicted to miss (disclosed above).
- **partial**: HS, HS', HS'', HB1, HB2, E1 to E3 met, S''' builds both blades fold-free and periodic (R1a to R1c), and every
  check except R2-REF met, or further checks missed (each miss an ignored `*_known_miss` record with the limit intact, located).
  If R2-REF misses only at the blade-line faces (the predicted 84.688) and R2w, R2t, R2-B2, R5 and SC are met, the ramp itself
  meets its part of the acceptance and the item is proposed partial with a follow-up for the blade-line faces.
- **todo**: S''' does not build one of the blades fold-free, or HS to HB2 or E1 to E3 missed (an existing result changed).

## Limits (not covered)

- The blade-line faces C (44..48, 2) / (.., 3) at layers 3 and 5 (REF 84.60 to 84.69 deg) are outside this item's field; not
  changed and not diagnosed here.
- Two blades at s4 plus the 12-point scan per blade; no scan of `n_down`, `n_side`, `n_span`; `alpha` is not scanned.
- The rise bound of 0.17 deg per degree of edge rotation is an estimate from two disclosed points, not a theorem; R2t and R2w test it.
- No flow solve; no case value (a schema enum value needs the schema golden, rule 6); construction S''' is library only.

## Addenda

### Amendment 1 (2026-11-17, after the code, before any registered run; no limit changed)

- No development build of S''' was made: the code (`wake_ramp_from_spacing`, `camber_seam_s_wake_ramp_construction()`, the
  `CGridMesh::wake_ramp_length` field, `WakeRamp` and `wake_shift_max` in `fan_cgrid.rs`, the deferred writing of the layers, the
  test file `f784_cgrid_wake_ramp.rs`) was compiled only (`--no-run`).
- The rows of `D` are `j <= min(n_bl, n_side)` (the same set at s4, `n_bl` 2 < `n_side` 10).
- The constants of HS'' in the test file are the base probe's values (`f784-scratch/probe.log`).
