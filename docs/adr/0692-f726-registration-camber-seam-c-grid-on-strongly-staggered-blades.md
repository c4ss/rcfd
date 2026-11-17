# ADR 0692: F-726 registration: the camber-seam C-grid on strongly staggered blades (an opt-in construction beside camber_seam_h)

Status: **REGISTERED 2026-11-17, written before any code of F-726 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-726 was written. One diagnostic run of the existing generator was made
(disclosed below, in a `git archive` copy of ff7e9989 in the scratch folder, instrumentation never committed); no construction
of this ADR existed when it ran, and it computed no quality metric. Results go in the addenda at the end; nothing above the
addenda is changed after a run.

Roadmap item F-726 (deps F-722), proposed by [ADR 0643](0643-f722-registration-reduced-order-reference-fan-in-the-schema.md)
"For the lead" (follow-up) and its remaining questions 1 and 2. Parents: [ADR 0118](0118-wave46a-computer-fan-schema-checker-perturbations.md)
(the schema `ComputerFan`, the s4 smoke specs), [ADR 0079](0079-wave35d-fan01-passage-mesh-and-cgrid-mechanisms.md) (construction
G, `camber_seam`, and its control range), [ADR 0083](0083-wave36d-cgrid-fan-mesh-quality-and-robustness.md) (construction H,
`camber_seam_h` = G + `post_untangle` + `lean_stack`), [ADR 0087](0087-wave37d-fan01-tip-corner-inclined-front-edge.md) (the
structural constraint of the seam corner). Number 0692 given by the lead. Branch `f726-work` from ff7e9989, worktree
`w49i-work/f726`, scratch `w49i-work/f726-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number); each registered run made once; existing tests not edited;
defaults bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new
dependency, no unsafe code; clean-room. The roadmap files are not edited. Every choice below that the maintainer would normally
be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).

Clean-room: the construction is elementary plane geometry (the side of a line a point lies on, straight segments between two
ordered curves, the area-preserving shear `s -> s - c(z)`); the quality measures are the repository's (`quality::compute_metrics`).
No source outside the repository was read.

## What F-726 asks

> Every limit not stated here is registered in the ADR before the run. The camber_seam_h C-grid (or a new opt-in construction beside it, camber_seam_h unchanged in bits on every existing case) builds ComputerFan::rom_reference() blade 0 at the ADR 0118 s4 specs with no folded cell and registered non-orthogonality, skewness and aspect limits, and the ADR 0079 and ADR 0083 cases keep their results; the h_slit mesh of the reference is measured against the same limits.

Clauses: **a1** every limit registered here; **a2** a new opt-in construction beside camber_seam_h (taken: the brief prefers it
when bits would move, and the changes below would move them), camber_seam_h unchanged in bits on every existing case; **a3** it
builds `rom_reference().axial_fan(0, 0.03, 0.04)` at the s4 `CGridSpec` with no folded cell; **a4** registered limits of
non-orthogonality, skewness and aspect met; **a5** the ADR 0079 and ADR 0083 cases keep their results; **a6** the h_slit mesh
of the reference at the s4 `SectorSpec` measured against the same limits.

## What was read and run before this text (disclosed)

Read: `fan_cgrid.rs` (all), `fan_design.rs` (`rom_reference`, `axial_fan`), `fan_mesh.rs` (`quality_summary`), `quality.rs`
(the metric definitions and the default thresholds: non-orthogonality warn 70 / error 85 deg, skewness warn 4.0, aspect warn
1000), ADR 0643 (A5b miss, its diagnosis, `diag1.log`), ADR 0079, ADR 0083, ADR 0087, the tests `cgrid_w35d.rs`,
`fan01_p10_w36d.rs`, `fan_design_w46a.rs`, `f722_rom_reference_fan.rs`.

The reference blade 0 in the s4 domain (printed by the run below): `axial_extent` 0.095, `z_le_root` 0.032, `r` 0.0215 to
0.058, tip gap 0.001, stations r 0.0215 / 0.0398 / 0.0580 m, chord 0.016 / 0.029 / 0.036 m, stagger 69.8 / 75.4 / 77.6 deg,
lean 0 / 0.00278 / 0.00810 m.

Diagnostic run D0 (`f726-scratch/diag`, a copy of ff7e9989 with `eprintln!` hooks in `build_cgrid_mesh_with`, test
`zz_diag.rs`; camber_seam_h at the s4 specs; fold locations only):

- Every one of the eight steps of the camber-seam fallback chain fails. Steps 0 to 6 fail the fold check. The in-plane grids
  of the layers are folded already before the radial blend and filter: at step 5 (no smoothing, the algebraic grid) in every
  layer on the pressure-side lines 43 to 49 (of 64) in rows 7 to 9 (next to the upper seam) and on the trailing-edge line 8 in
  rows 5 to 7; at steps 0 to 4 also on the lower wake lines 1 to 7 next to the stack; at step 6 around the leading edge (lines
  31 to 41, row 2). Step 7 (Laplace start, plain spacing controls) passes the fold check of the quads but is refused by
  `Mesh::from_cell_faces` ("internal face 10885: normal does not point from owner to neighbour"): 84 of 12 848 C and F hexes
  have corner Jacobians of mixed sign (concave in the plane, positive area) at lines 5 and 6 and 32 to 34, row 2, in the hub
  layers. The refusal printed by the generator is the step-0 message ("folded at layer 0 ... C block around line 1, row 2").
- So the fold is not one place. It has two mechanisms, both set by the stagger.

## The two mechanisms (pencil, from the code)

Notation: unrolled plane `(z, s)`, blade direction `t = (cos g, sin g)` with `g` the stagger (77.6 deg at the tip), camber 0, so
the camber line is straight. The suction-side normal is `n_s = (sin g, -cos g)` (it faces downstream), the pressure-side normal
`-n_s`. Pitch `p = 2 pi r / 7` (0.0521 m at the tip, 1.45 chords).

**M-a (leading-edge fan).** The front edge is the line `z_f = z_min - front_gap c`, and its nose target is `s_le`, straight
ahead of the leading edge in `z`; the passage centre line turns axial over `front_gap c / 3` upstream of the leading edge
(ADR 0079). A front-edge point `(z_f, s)` lies on the suction side of the extended camber line iff
`(z_f - z_le) sin g - (s - s_le) cos g > 0`, i.e. `s < s_le - front_gap c tan g` = `s_le - 1.37 c` at the tip (front_gap 0.3).
The lower half of the front edge (the targets of the suction-side nose lines) runs from the corner `centre(z_f) - p/2` =
`s_le - 0.95 tan(g) front_gap c / 3 - p/2` = `s_le - 1.15 c` up to `s_le`: every point of it is on the pressure side. So every
suction-side nose line has to wrap around the leading edge to reach its target, whatever `front_gap` and `n_nose` (the ratio is
independent of the chord, which is why ADR 0643 found no in-range specification that builds). At 35 deg (blade B) the bound is
`s_le - 0.21 c` and the corner `s_le - ...` below it: the targets are on the right side.

**M-b (pursuit lines).** Each C line leaves its stack along the wall normal and turns smoothly to aim at its target over 70 % of
its rows. On the blade sides the target is the seam point at the same `z` as the station (the seams are the same node list
translated by the pitch), i.e. straight across in `s`, at `90 - g` = 12.4 deg from the wall at the tip: the lines must turn by
77.6 deg after the stack, and neighbouring lines whose turn starts at different distances cross. The same holds for the wake
lines near the trailing edge, where the wake curve still runs at the stagger.

**What no construction of this topology can avoid.** A seam station and its periodic image are reached from the suction and the
pressure side of the same chord station (`seam_nodes`), so the cells between the blade and the seam carry the shear of a
parallelogram whose families meet at `90 - g`; at the seam corner the camber seam meets the pitchwise front edge at the same
angle (ADR 0087: the corner node is one periodic node at one `z`). For uniform parallelograms the face non-orthogonality is the
complement of that angle: about **77.6 deg at the tip** is a floor for every construction that keeps this topology, including
camber_seam_h where it builds. The corner wedge of `90 - g` is split by the corner line into two cells, so cells with angles of
about 5 to 8 deg (non-orthogonality up to about 85 deg in the algebraic grid) are expected there unless the smoothing opens them.

## Design (fixed now): construction S, `camber_seam_s` = H + two new opt-in fields

Two new fields of `CGridOptions`, default `false` (so `Default`, `camber_seam_construction()`, `camber_seam_h_construction()` and
every struct literal with `..` are unchanged), set together by a new constructor `CGridOptions::camber_seam_s_construction()` =
`camber_seam_h_construction()` + both fields. No case value is added (a schema enum value would change the schema golden; it is
listed as a follow-up).

**S1, `camber_front` (removes M-a; needs `camber_seam`).** With `sigma_le` the leading-edge camber slope (as now) and
`cos_le = 1 / sqrt(1 + sigma_le^2)`, `sin_le = sigma_le cos_le`, `(z_le, s_le0)` = `cam[0]`:

1. the front edge is `z_f = z_le - front_gap c cos_le` (the nose target `N` is `front_gap` chords from the leading edge along the
   extended camber line, `N = (z_f, s_le0 - front_gap c sin_le)`); it must satisfy `z_f < z_min` and `z_f > 0`, else the
   existing located errors (`z_le_root`) with the same wording;
2. the passage centre line between `z_f` and `z_le` is the straight extension `s_le0 + sigma_le (z - z_le)` (so `centre(z_f)` =
   `s_N`), and upstream of the front edge it turns axial, C1 at `z_f`: `s_N - sigma_le lam_f (1 - exp(-(z_f - z) / lam_f))` with
   `lam_f = z_f / 3` (the ADR 0079 rule of a third of the distance, here front edge to inlet, so that the seams reach the inlet
   within `atan(sigma_le e^-3)` of the axis); the F rows follow it as now;
3. the nose target of the front edge is `s_n = s_N` (instead of `s_le`), the front edge runs from `s_N - p/2` to `s_N + p/2`.

Derivation: a front-edge point `(z_f, s_N - u)`, `u` in `[0, p/2]`, is at `(z_f - z_le, s - s_le0) = (-front_gap c cos_le,
-front_gap c sin_le - u)`; with `G = front_gap c` its side value `(z - z_le) sin_le - (s - s_le0) cos_le` is
`-G cos_le sin_le + (G sin_le + u) cos_le = u cos_le`, positive (suction side) for the lower half `u > 0` and negative for the
upper half: each nose line's target is on the side of its wall point, for every stagger below 90 deg.

**S2, `straight_lines` (removes M-b).** Beyond its stack each C line is the straight segment from the stack edge to its target
(the pursuit weight `beta = 1` on every line, as the existing code already does for the outlet lines `i = 0, n_i`), and the first
cell along the segment is `first / max(cos_t, 0.1)` instead of `first` (`first = d1 g^nb` the next height of the stack, `cos_t`
the cosine between the segment and the wall normal, as computed in the code), so that the wall-normal height of the first free
cell continues the stack growth (`0.1` is the lean floor already used by `lean_stack`). Derivation: two straight segments
`[e_i, o_i]`, `[e_{i+1}, o_{i+1}]` between two curves that are ordered the same way along the passage and do not cross each
other do not intersect if the quad `e_i e_{i+1} o_{i+1} o_i` is positively oriented; on the blade sides `o - e` is nearly `+-s`
for every line (targets at the same `z` as the stations up to the corner blend), on the nose the fan of S1 is ordered from the
corner to `N`.

Everything else is H: smoothing and its fallback chain, `post_untangle`, `lean_stack`, `radial_remap`, `radial_filter`,
`gap_fit`, `checked_range` (the G range; the s4 spec is inside it: `n_nose / n_chord` 0.25, `n_side` 10, `front_gap` 0.3).
Without the two fields every expression is the existing one (each new expression sits in an `if` on a field).

Estimate on the reference (not a proof): the in-plane grid of S1 + S2 has no crossing segment in the four regions (blade sides,
nose fan, wake, outlet lines); the cells are convex parallelograms on the blade sides; the smoothing chain stops at the first
step whose mesh validates, the algebraic step at the latest.

## Development protocol (registered)

The code is written to this design. Before the registered run, development builds of S on the reference are allowed **only to
fix defects** (code that does not do what this design says) and print only the build status, the construction step and, on a
refusal, the generator's located message; no quality metric is computed or printed in development. If the design as registered
folds, at most three design variants may be tried, each derived and written down (with its fold location) in an amendment
before any registered run; the variant frozen is the first that builds. No limit below changes in any case. Every development
build is listed in amendment 1.

## Common set-up (fixed now)

`af = ComputerFan::rom_reference().axial_fan(0, 0.03, 0.04)`; the s4 specs of `fan_design_w46a.rs` `smoke_specs()` copied
verbatim (as ADR 0643): `CGridSpec { n_up 8, n_chord 24, n_down 8, n_side 10, n_nose 6, n_core 2, n_span 10, n_gap 7, n_bl 2,
growth 1.2, first_cell Height(5e-5), front_gap 0.3, include_blade true }` and `SectorSpec { n_up 8, n_chord 16, n_down 8,
n_side 10, n_mid 2, n_span 10, n_gap 7, n_bl 2, growth 1.2, first_cell Height(5e-5), trunc_le 0.02, trunc_te 0.01,
include_blade true }`. Quality: `quality_summary(&mesh)` (maxima over every face and cell, boundary faces included).

## Checks (limits fixed now)

| id | clause | what | limit | estimate behind it |
|---|---|---|---|---|
| A1a | a3 | `build_cgrid_mesh_with(&af, &cs, &camber_seam_s_construction())` | Ok, cells > 0 | design |
| A1b | a3 | no folded cell: every corner Jacobian (the 8 corners of each C- and F-block hex, from `grid` by `c_node` / `f_node`) has one common sign, none zero; `min_volume` > 0 | met | convex cells; the generator's own checks (quad areas, face orientation, volumes) pass by A1a |
| A1c | a3 | periodicity: every lower seam node rotated by `2 pi / 7` against its upper partner (`seam_nodes(k)`, every layer) | <= 1e-12 m | the seam node lists are translates (ADR 0079 P4 limit) |
| A2 | a4 | max non-orthogonality | <= 85 deg | floor 77.6 deg (tip shear and corner wedge, above); 85 is the repository's error threshold (`non_orthogonality_error`), i.e. `check_mesh` reports no error |
| A3 | a4 | max skewness | <= 4.0 | stack-to-segment kink about 0.5 (pencil, with S2's first cell); FAN-01-C with H 3.98 (ADR 0083); 4.0 is the repository's warning threshold |
| A4 | a4 | max aspect ratio | <= 1000 | radial first layer 5e-5 m against in-plane edges up to about 10 mm (wake and F cells): about 200; the trailing-edge wedge unknown; 1000 is the repository's warning threshold |
| A5 | a6 | h_slit (`build_sector_mesh(&af, &ss)`) quality against A2 to A4 | measured, met or missed per limit, not a gate | ADR 0643: 85.37 deg / 16.7 / 827, so predicted: non-orthogonality **missed**, skewness **missed**, aspect met |
| B1 | a2, a5 | the existing bit-identity tests of the constructions on the ADR 0079 / 0083 cases: `cgrid_w35d` `b1_existing_constructions_are_bit_identical` (pursuit and E on S and A'), `b1_construction_g_is_bit_identical` (G on S and FAN-01-C), `b1_construction_h_is_bit_identical` (H on S and FAN-01-C) | pass (recorded hashes) | no expression of these paths changes |
| B2 | a2 | camber_seam_h on the default `ComputerFan` blade 0 at the s4 specs (the ADR 0118 S4 mesh): FNV-1a mesh hash (as ADR 0079 B1) computed by the new test on a `git archive` copy of ff7e9989 (test file only) and on the branch | equal | as B1 |
| B3 | a5 | the ADR 0079 / 0083 case tests that exercise the C-grid, run by name: `cgrid_w35d` `p1_p8_fan01_mesh_builds_closes_is_periodic_and_exact`, `c1_fan01_h_mesh_builds_closes_is_periodic_and_exact`, `control_range_is_checked_only_by_the_camber_seam_construction`, `r_g_out_of_range_combinations_are_located_errors`; `fan01_p10_w36d` `c2_fan01_h_mesh_is_accepted_by_the_solver_and_the_rules`; `fan_design_w46a` `s4_realised_default_meshes`; `f722_rom_reference_fan` `a5_the_reference_meshes_at_the_s4_specs_met_limits` | pass | no line they run changes |
| B4 | a2 | `cargo test -p rcfd-mesh --release --lib` | same pass count as on the base, 0 failed | |
| R1 | reported | S on the reference: construction step, cell count, `QualitySummary`, the location (block, line, row, layer) of the face with the largest non-orthogonality and skewness and of the cell with the largest aspect | none | |
| R2 | reported | S on the default `ComputerFan`, on blade B with the S counts and on FAN-01-C (the ADR 0079 / 0083 cases): builds or refusal, step, quality | none | not part of the acceptance (S is opt-in) |

### Negative controls

| id | control | limit |
|---|---|---|
| NC1 | camber_seam_h on the reference at the s4 specs (the ADR 0643 A5b miss reproduced) | refused, path `front_gap`, message exactly "the line family is folded at layer 0 (r = 0.02150), C block around line 1, row 2: the leading-edge fan cannot be built for this blade with the front_gap 0.3 and n_nose 6 given; reduce front_gap or change n_nose" |
| NC2 | H + `straight_lines` only (no `camber_front`) | reported, predicted refused (M-a remains; the suction nose segments cross the leading-edge line, pencil above) |
| NC3 | H + `camber_front` only (pursuit lines) | reported, predicted refused (M-b remains) |
| NC4 | the A1b corner-Jacobian test applied to the step-7 mesh of D0 is not reproducible through the public API (the generator refuses it); instead A1b is applied to a hand-made hex with one corner pushed through the opposite face | detected (mixed sign) |

## Planned mutations (scratch copy of the worktree, each run once against the new tests and B1)

| id | mutation | expected to be caught by |
|---|---|---|
| MU1 | S1 centre line: the existing axial turn over `front_gap c / 3` kept upstream of the leading edge with `camber_front` | A1a (fold) |
| MU2 | S1: nose target `s_n = s_le` (the existing) with `camber_front` | A1a (fold) |
| MU3 | S2: pursuit weight kept (`straight_lines` ignored) | A1a (fold) |
| MU4 | S2 first cell `first / max(cos_t, 0.1)` -> `first` | A2 or A3 (predicted: the stack-to-segment jump in wall-normal height raises skewness); may survive |
| MU5 | an existing expression changed on the default path: `lam_u = front_gap c / 3` -> `/ 2.9` | B1 (G and H hashes), B2 |
| MU6 | `s_hi_at` under `camber_front`: pitch times `1 + 1e-9` | A1c, or a refusal (A1a) by `set_periodic_rotation` |

## Decision rule

- **done**: A1a to A1c, A2, A3, A4 met, A5 measured, B1 to B4 met, NC1 and NC4 met.
- **partial**: A1a to A1c, B1 to B4, NC1 met, but a quality limit A2 to A4 missed (an ignored `*_known_miss` record with the
  limit intact and the cause located by R1).
- **todo**: A1 missed (S does not build the reference fold-free, after the development protocol), or B1 to B4 missed (an existing
  result changed).

## Limits (not covered)

- One blade (the reference) at one specification (s4); the control range of S is not scanned (the ADR 0079 R-G style scan on S
  is a follow-up). Blade B and FAN-01-C with S are reported only (R2).
- The topology is kept, so the 77.6 deg shear floor stays: a mesh within the limits is not a mesh of low non-orthogonality. A
  topology change (an offset seam corner, ADR 0087 I-b, or an O/H topology) is outside this item.
- No flow solve on the meshes; no case-file value for S (follow-up); no change of h_slit.

## Addenda

### Amendment 1 (2026-11-17, after the development builds, before any registered run; no limit changed)

Development builds (the protocol above), all on the reference at the s4 specs, in scratch copies of the branch code
(`f726-scratch/dev`, `f726-scratch/diag` with the D0 hooks; logs `dev1.log`, `dev2_diag.log`, `dev3_diag.log`); no quality
metric was computed or printed:

1. S as registered (`dev1.log`): refused, step-0 message "folded at layer 0 (r = 0.02150), C block around line 2, row 2".
2. S as registered with the D0 hooks (`dev2_diag.log`): steps 0 to 4 (with smoothing) fold in the plane of the layers on the
   wake and suction-side lines next to the stack edge (row 2); steps 6 and 7 (Laplace start) fold at the outlet line and the
   leading edge (row 2). **Step 5, the algebraic grid of S1 + S2, is unfolded in every layer, also after the radial blend
   (`pre-filter fold None`); the radial filter folds it** at layer 5 (r = 0.03975, the mid-span station), C block around line 0,
   row 2.
3. Variant V1 (below) with the D0 hooks (`dev3_diag.log`): builds at step 5, 12 841 cells; the D0 corner-Jacobian hook counts 0
   of 12 848 C and F hexes with mixed signs (disclosed: this is the measure of A1b, seen once in development on the variant).

No defect of the registered code was found (the step-5 grid is the design's algebraic grid and is unfolded); the fold is a design
interaction, so the protocol's variant rule applies. One of the three permitted variants is used.

**Variant V1 (frozen): S without the radial filter** (`radial_filter: false` in `camber_seam_s_construction()`, every other field
as registered). Derivation of the fold, from the code: the filter (ADR 0079 F3) moves a far node of layer `k` towards the linear
interpolation in `r` of layers `k - 1` and `k + 1`, with a weight that rises from 0 at the stack edge as `1 - fade((j - nb) /
(ns - nb))`, i.e. 0.043 at the first free node `j = nb + 1` (two passes of half the weight: about 4.3 % of the deviation). The
station distribution of the reference is piecewise linear with a kink at mid span, the radius of layer 5 (chord slopes 0.71 and
0.38 m/m, stagger 5.6 and 2.2 deg per half span), so the wake and trailing-edge positions are kinked in `r` there: the wake offset
`s_inf - s_te = sigma0 0.2 c`, about 0.9 c, alone deviates from the interpolation of layers 4 and 6 (9 mm away on either side)
by about `0.9 x 0.33 x 0.009 / 2` = 1.3 mm, the trailing-edge position by a similar amount. 4.3 % of 2 to 3 mm is about 0.1 mm,
above the length of the first free cell next to the outlet-side wake stack (`first` = 7.2e-5 m along a nearly pitchwise
segment, `cos_t` about 1): the first free quad turns over. The algebraic grids of S vary smoothly with `r` between the kinks and
need no filter; H keeps it (bits unchanged). The fold-free step-5 result is not an argument from the measured numbers but the
registered fallback of the chain.

Construction S as frozen: `CGridOptions::camber_seam_s_construction()` = `camber_seam_h_construction()` + `camber_front` +
`straight_lines` + `radial_filter: false`. NC2 and NC3 stay as registered (H, with its filter, + one field). MU1 to MU6 are run on
this S. Prediction for the registered run: A1a met at step 5 (the development build); A2 to A4 not seen.
