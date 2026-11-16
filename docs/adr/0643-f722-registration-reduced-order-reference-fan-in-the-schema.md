# ADR 0643: F-722 registration: a reduced-order reference fan in the schema (a named constructor beside the default)

Status: **REGISTERED 2026-11-16, written before any code of F-722 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-722 was written and no Rust code was run. The derivation arithmetic
was done in two Python scripts in the scratch folder (`w49i-work/f722-scratch/derive_f722.py`, a copy of the ADR 0639 script
`f713-scratch/derive4.py` with the one line `A0` changed to zero hub camber, and `derive_f722b.py`, the predictions on the
rounded stations); their numbers are disclosed below. Results go in the addenda at the end; nothing above the addenda is
changed after a run.

Roadmap item F-722 (deps F-713), proposed by [ADR 0639](0639-f713-registration-schema-default-fan-in-the-reduced-order-model.md)
"For the lead" (follow-up) and its remaining questions 1 to 3. Parents: [ADR 0118](0118-wave46a-computer-fan-schema-checker-perturbations.md)
(schema `rcfd_mesh::fan_design::ComputerFan`, checker, s4 meshes), [ADR 0119](0119-wave46b-reduced-order-axial-fan-tier.md)
(reduced-order fan `rcfd_post::fan_rom`). Number 0643 given by the lead. Branch `f722-work` from 32f3c320 (the F-713 tip on
local main d40cd665), worktree `w49i-work/f722`, scratch `w49i-work/f722-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number); each registered run made once; existing tests not edited;
defaults bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new
dependency, no unsafe code; clean-room. The roadmap files are not edited. Every choice below that the maintainer would normally
be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).

Clean-room: the design rule is the one of ADR 0639 (the ADR 0119 strip equations: thin-airfoil lift `2 pi alpha_e` up to
`cl_max`, Euler work, the ADR 0119 loss terms, free-discharge static pressure); the geometry conditions are elementary
(direction of a NACA 4-digit camber line at its leading edge, `dy_c/dx = 2 m / p` at `x = 0`, from the closed form
`y_c = m (2 p x - x^2) / p^2`). No source outside the repository was read.

## What F-722 asks

> Every limit not stated here is registered in the ADR before the run. A named ComputerFan constructor beside the default (ComputerFan::default() and every existing test unchanged in bits and result) gives a 120 mm 7-blade fan, derived by the ADR 0639 rule with a hub section whose camber line runs downstream (stagger + atan(2 m / p) below 90 deg at every station), that runs unstalled in RomModel::default() at 3000 rpm for every flow in [0.4, 1] of its free delivery, passes ComputerFan::check with Limits::default(), and meshes with h_slit and the camber_seam_h C-grid at the ADR 0118 s4 specs; its usable operating range against quadratic systems is reported.

Clauses:

- **a1** every limit registered here;
- **a2** a named constructor beside the default; `ComputerFan::default()` and every existing test unchanged in bits and result;
- **a3** it gives a 120 mm, 7-blade fan;
- **a4** derived by the ADR 0639 rule, with `stagger + atan(2 m / p) < 90 deg` at every station;
- **a5** unstalled in `RomModel::default()` at 3000 rpm at every flow in `[0.4, 1] q_f`;
- **a6** `check(&Limits::default())` empty;
- **a7** meshes with h_slit (`build_sector_mesh`) and the camber_seam_h C-grid at the ADR 0118 s4 specs;
- **a8** its usable operating range against quadratic systems reported.

Reading of "unstalled" (a5): as ADR 0639 a1, no strip off the attached branch (`FanPoint::stalled` empty). The rise of the static
characteristic below its peak (ADR 0639 P1c) is not stall in this sense; it bounds the usable range of a8 and is reported there
(ADR 0639 question 2, recommendation (a)).

## What the code read shows (base 32f3c320, read only)

1. `ComputerFan::default()` is `impl Default for ComputerFan` in `crates/rcfd-mesh/src/fan_design.rs` (216-257). A new
   associated function in the existing `impl ComputerFan` block changes no line of it, no `PARAMS` entry and no serialised
   form; no golden, hash table or `baseline_hashes.rs` entry holds a `ComputerFan` (ADR 0639 read 2).
2. `rcfd_post::fan_rom::alpha_l0` returns exactly 0 for camber 0, so a camber-0 section has zero-lift angle = stagger.
3. The h_slit generator (`fan_mesh.rs` `build_sector_mesh`) cuts each radial layer at the camber-line points `x = trunc_le`
   (0.02) and `1 - trunc_te` (0.99) and requires each axial plane to lie on the axially monotone branch of both surfaces
   (`ZCurve::s_at_z`); it returns at the first failing layer, so ADR 0639 P4 measured the hub layer only. The camber_seam_h
   C-grid (`fan_cgrid.rs`) requires the sampled camber line to run downstream monotonically, the pressure surface to run
   downstream at the trailing edge, and the front edge `z_min - front_gap c > 0`.
4. On P (ADR 0639) both generators failed at the hub, whose camber line left the leading edge at `76 + atan(2 * 0.06 / 0.4)` =
   92.70 deg from the axis (upstream first).

## Design (fixed now)

New associated function `ComputerFan::rom_reference() -> ComputerFan` in `fan_design.rs`, beside the default (ADR 0639 question
1, recommendation (c), taken on the standing instruction). Everything but `stations` is the default's (built as
`..ComputerFan::default()`), so the 120 mm frame, 7 blades, `r_hub` 0.0215, `r_tip` 0.058, 1 mm gap and the `PARAMS` scalars
are the default's.

Stations by the ADR 0639 rule (set-up P's derivation, unchanged): at each control radius (span 0, 0.5, 1) the zero-lift angle
`gamma` is the one that puts that radius at `alpha_e = 7.5 deg` at `0.4 q_f`, with `q_f` solved self-consistently; chords 0.016
/ 0.029 / 0.036 m, thicknesses 0.055 / 0.042 / 0.035, skew 0 / 4 / 8 deg, sweep 0, camber position 0.4, as P. The one change
from P is ADR 0639 question 3, recommendation (a), taken on the standing instruction: **hub camber 0** (mid and tip were already
0), so `stagger = gamma` at every station, rounded to 0.1 deg.

`derive_f722.py` (the ADR 0639 design rule, unchanged equations): `gamma` = 69.8114 / 75.4245 / 77.6289 deg, so stagger
**69.8 / 75.4 / 77.6 deg** (P's hub had `gamma` 69.768 from 76.0 - 6.232; the mid and tip staggers equal P's).

| station | span | chord [m] | stagger [deg] | camber | camber_pos | thickness | sweep | skew [deg] |
|---|---|---|---|---|---|---|---|---|
| hub | 0 | 0.016 | 69.8 | 0 | 0.4 | 0.055 | 0 | 0 |
| mid | 0.5 | 0.029 | 75.4 | 0 | 0.4 | 0.042 | 0 | 4 |
| tip | 1 | 0.036 | 77.6 | 0 | 0.4 | 0.035 | 0 | 8 |

Every section is a symmetric NACA 00xx; `stagger + atan(2 m / p)` = 69.8 / 75.4 / 77.6 deg, and the interpolated sections in
between have camber 0 too, so the camber line is the straight chord at most 77.6 deg from the axis everywhere.

Meshability (estimate, not a proof): with a straight camber line, the axial plane at `x = 0.02` lies `0.02 c cos(gamma)`
downstream of the leading edge, inside both surfaces' monotone branches as long as the most upstream point of the thin side,
where `dy_t/dx = cot(gamma)`, sits upstream of `x = 0.02`: with `dy_t/dx ~ 5 t 0.2969 / (2 sqrt x)` that is `x` about 0.012 at
the hub (`t` 0.055, cot 0.368) and 0.014 at the tip (`t` 0.035, cot 0.219). The trailing-edge plane `x = 0.99` is inside when
`y_t(1) sin(gamma) < 0.01 c cos(gamma)`: `y_t(1)` = 0.0063 t, i.e. 3.5e-4 at the hub against 0.0037. The checks below measure it.

Checker (estimate): hub axial extent in the axial view `0.016 sin 69.8` = 0.01502 m (plus a thickness projection of about 0.3 mm)
against the hub pitch `2 pi 0.0215 / 7` = 0.01930 m: no `AxialOverlap`; minimum wall `0.055 * 0.016` = 0.88 mm >= 0.8 mm.

Predicted (`derive_f722b.py`, rounded stations, ADR 0119 default losses, 40 strips): `q_f` = 0.028259 m^3/s; at `0.4 q_f` the
largest incidence 8.215 deg (stall at 10.031); over `[0.4, 1] q_f` incidence -3.01 to 8.22 deg; stall onset fraction 0.309; `dp_s`
26.30 Pa at 0.4 `q_f`, 20.10 at 0.7; peak of `dp_s` on the 601-flow grid 26.83 Pa at 0.457 `q_f`, so the largest quadratic system
coefficient with an operating point on the falling characteristic is `k_max = 26.83 / (0.457 q_f)^2` = 1.609e5 Pa s^2/m^6;
against `dp = k Q^2`: `q / q_f` = 0.9148 / 0.8473 / 0.7051 / 0.5634 at `k` = 1e4 / 2e4 / 5e4 / 1e5. Negative controls: every
stagger 8 deg lower stalls on the grid of its own `q_f` (0.043464) up to 0.555; `cl_max` 0.8 up to 0.451.

The constructor's rustdoc states that it is a model fan of low flow coefficient for the reduced-order tier, not a measured fan,
and gives its usable range from this ADR's measured numbers (written after the run; the doc text is not a check).

## Common procedure (as ADR 0639, fixed now)

`chain(cf, model)`: `cf.axial_fan(0, 0.0, 0.0)` -> `RomFan::from_axial_fan` -> `FanRom::new(.., model)`; `Operating { rpm 3000,
rho 1.2, nu 1.5e-5 }`. Free delivery `q_f`: `FanRom::operating_point(op, SystemCurve { k: 0, l: 0 }, 1e-4, 0.12, 400)`. Grid G:
601 flows `q_j = q_f (0.4 + 0.6 j / 600)`. Onset fraction `f_s`: the first fraction, stepping down from 0.400 by 0.001, with a
stalled strip. Usable range: `k_max` by bisection (60 halvings) on `k` in `[0, 1e6]` of "`operating_point(op, SystemCurve { k,
l: 0 }, 1e-4, 0.12, 400)` succeeds", and `q_min` the flow of the operating point at the accepted end. Meshes: blade 0 by
`axial_fan(0, 0.03, 0.04)` with the s4 `smoke_specs` of `fan_design_w46a.rs` (copied verbatim, as ADR 0639 P4).

## Checks (limits fixed now)

| id | clause | what | limit | estimate behind it |
|---|---|---|---|---|
| A1a | a3 | `rom_reference()`: `n_blades`, `frame.side`, `r_tip + tip_gap` | 7; 0.12 m; <= 0.06 m | by construction |
| A1b | a2, a3 | every field but `stations` equal to `ComputerFan::default()`'s; `rom_reference() != default()` | equal; differ | by construction |
| A1c | a4 | the stations equal the table above (exact `f64` literals) | equal | by construction |
| A2 | a4 | `stagger + atan(2 m / p)` at every station (`p` > 0; 0 when `m` = 0) | < 90 deg each (69.8 / 75.4 / 77.6) | closed form |
| A3a | a5 | `operating_point` for `q_f` | succeeds | derivation |
| A3b | a5 | stalled grid flows on G | 0 of 601 | largest incidence 8.22 < 10.03 |
| A4a | a6 | every scalar in `PARAMS` bounds | in bounds | as P |
| A4b | a6 | `check(&Limits::default())` | empty | estimate above; P was accepted |
| A5a | a7 | `build_sector_mesh(&af, &ss)` | Ok, cells > 0 | estimate above |
| A5b | a7 | `build_cgrid_mesh_with(&af, &cs, &camber_seam_h_construction())` | Ok, cells > 0 | straight camber line |
| A5c | reported | the pursuit C-grid (`CGridOptions::default()`), cell counts and `quality()` | none | |
| A6a | a8 | `k_max` | in [1.53e5, 1.69e5] (5 %) | 1.609e5 |
| A6b | a8 | `q_min / q_f` (the operating point at `k_max`) | in [0.44, 0.48] | 0.457 |
| A6c | a8 | `q / q_f` at `k` 1e4 / 2e4 / 5e4 / 1e5 | within 0.01 of 0.9148 / 0.8473 / 0.7051 / 0.5634 | Python |
| A6d | a8 | `k` = 2e5 (above `k_max`) | refused | rising characteristic below the peak |
| R1 | a4 (rule reproduced) | `q_f` | within 1 % of 0.028259 | Python; ADR 0639 reproduced P to 1.2e-5 |
| R2 | a4 | largest incidence at 0.4 `q_f` | within 0.15 deg of 8.215 | Python |
| R3 | a4 | `f_s` | in [0.28, 0.34] | 0.309 |
| R4 | a8 | peak of `dp_s` on G: fraction; value | in [0.44, 0.48]; within 2 % of 26.83 Pa | Python 0.457, 26.829 |
| R5 | reported | `[q_f, 1.25 q_f]` (101 flows): stalled count, smallest incidence; `dp_s` at 0.4 and 0.7 `q_f` | none | |
| B1 | a2 | `git diff --stat 32f3c320` | only this ADR, `fan_design.rs` (lines added inside `impl ComputerFan`, none removed), the new test file, at most a topic doc | by construction |
| B2 | a2 | the existing tests that cover `fan_design.rs` and the F-713 tests: `cargo test -p rcfd-mesh --release --test fan_design_w46a`, `cargo test -p rcfd-mesh --release --lib fan_design`, `cargo test -p rcfd-post --release --test f713_schema_fan_rom` | same pass / ignore counts as on the base | no line they run is changed |

### Negative controls (each must fail where the reference passes)

| id | control | limit |
|---|---|---|
| NC1 | A2's function on P's hub (stagger 76, camber 0.06, p 0.4) | 92.70 deg, >= 90 (the function reports it) |
| NC2 | reference with every stagger -8 deg, ROM on its own `q_f` | stalled grid flows > 0 (Python: up to 0.555) |
| NC3 | reference with `RomModel { cl_max: 0.8, .. }` | stalled grid flows > 0 (Python: up to 0.451) |
| NC4 | reference with hub chord 0.022 (the default's) | `check` reports `AxialOverlap` at `stations[0].chord` (0.0206 > 0.0193) |
| NC5 | P of ADR 0639 (hub camber 0.06, stagger 76) through camber_seam_h at the s4 specs | refused (as ADR 0639 P4) |

## Planned mutations (scratch copy of the worktree, each run once against the new tests)

| id | mutation | expected to be caught by |
|---|---|---|
| M1 | `rom_reference`: hub camber 0 -> 0.06 (P's hub camber) | A1c, A2, A5b, R1 |
| M2 | `rom_reference`: tip stagger 77.6 -> 72.6 | A1c, A3b or R2, R1 |
| M3 | `rom_reference`: hub chord 0.016 -> 0.022 | A1c, A4b |
| M4 | `rom_reference`: `r_tip` set to 0.059 instead of the default's | A1b |
| M5 | `fan_rom.rs` `solve_strip`: stall threshold `cl_max / TAU` -> `2.0 * cl_max / TAU` | NC2, NC3, R3 |

## Decision rule

- **done**: every check A1 to A6, R1 to R4, B1, B2 and every negative control met.
- **partial**: every acceptance check (A1 to A5, A6 measured and reported) and B1, B2 met, but a prediction limit (A6a to A6d,
  R1 to R4) or a negative control missed; or a5 met but a7 missed (the reference not meshable), with the cause diagnosed.
- **todo**: A3 (stall) or A4 (checker) missed, or B1 / B2 missed (an existing result changed).

## Limits (not covered)

- One speed (3000 rpm), one density, no stator, free discharge; the reference is a model fan (no measured data). Its low flow
  coefficient is what the ROM's 10 deg attached window asks for (ADR 0639), not a claim about real 120 mm fans.
- The ADR 0118 mass, ensemble, perturbation and unbalance tests, the FEM blade tests and the F-697 / F-698 objectives are not
  run on the reference; nothing in the repository reads it yet.
- The meshes are the s4 smoke meshes (built, quality reported); no flow solve on them.
- No ADR 0118 checker variant is re-based on the reference (they keep the default as base).

## Addenda

### Amendment 1 (2026-11-16, after the one registered run, before any further run)

The registered run missed A5b (results in addendum 1). By rule 2 the A5 test becomes the ignored record
`a5_the_reference_meshes_at_the_s4_specs_known_miss` with its limits intact. So that the met clauses of A5 keep running, one test
is added, `a5_the_reference_meshes_at_the_s4_specs_met_limits`: the same code and the same limits for A5a and NC5, A5b only
printed (the same deterministic computation, so its numbers are those of the registered run; it is run once to confirm that).
No limit is changed. One diagnostic run after the miss (scratch `f722-scratch/zz_f722_diag.rs`, log `diag1.log`, never
committed) locates the fold and evaluates camber_seam_h at other `front_gap` and `n_nose` inside the documented control range
and on the reference with lower hub stagger; it is reported in addendum 1 as diagnosis, not as a check, and changes no station
of `rom_reference()`.

## Addendum 1: results (2026-11-16)

One registered run (`f722-scratch/run1.log`, release, `cargo test -p rcfd-post --release --test f722_rom_reference_fan --
--nocapture --test-threads 1`: 5 passed, 1 failed), then after amendment 1 one run of the split tests (`run2.log`: 6 passed, 1
ignored; the numbers are those of run 1).

| id | limit | measured | met |
|---|---|---|---|
| A1a | 7 blades; side 0.12 m; `r_tip + gap` <= 0.06 m | 7; 0.12; 0.059 | yes |
| A1b | fields but stations equal the default's; fans differ | equal; differ | yes |
| A1c | stations equal the table | equal | yes |
| A2 | `stagger + atan(2 m / p)` < 90 deg at every station | 69.8 / 75.4 / 77.6 deg | yes |
| A3a | `operating_point` for `q_f` succeeds | ok, 0.0282591 m^3/s | yes |
| A3b | stalled grid flows 0 | 0 of 601 | yes |
| A4a | scalars in `PARAMS` bounds | in bounds | yes |
| A4b | `check(&Limits::default())` empty | empty | yes |
| A5a | h_slit builds, cells > 0 | 11648 cells (max non-orthogonality 85.37 deg, skewness 16.7, aspect 827) | yes |
| A5b | camber_seam_h builds, cells > 0 | refused: "the line family is folded at layer 0 (r = 0.02150), C block around line 1, row 2: the leading-edge fan cannot be built for this blade with the front_gap 0.3 and n_nose 6 given" | **no** (record `a5_..._known_miss`) |
| A5c | reported | pursuit C-grid rejected: "the wake line (s from -0.0150 to -0.0259) leaves the passage (-0.0172, 0.0021)" at the hub layer | - |
| A6a | `k_max` in [1.53e5, 1.69e5] | 1.60806e5 Pa s^2/m^6 (just above: "rising fan characteristic") | yes |
| A6b | `q_min / q_f` in [0.44, 0.48] | 0.45708 (0.012917 m^3/s) | yes |
| A6c | `q / q_f` within 0.01 of 0.9148 / 0.8473 / 0.7051 / 0.5634 | 0.91480 / 0.84728 / 0.70508 / 0.56338 (`dp` 6.68 / 11.47 / 19.85 / 25.35 Pa) | yes |
| A6d | `k` 2e5 refused | refused: operating point 0.011493 m^3/s on a rising characteristic | yes |
| R1 | `q_f / 0.028259` in [0.99, 1.01] | 1.000005 | yes |
| R2 | largest incidence at 0.4 `q_f` within 0.15 of 8.215 deg | 8.2147 deg | yes |
| R3 | `f_s` in [0.28, 0.34] | 0.309 | yes |
| R4 | peak fraction in [0.44, 0.48]; value / 26.83 in [0.98, 1.02] | 0.457; 0.99996 (26.829 Pa) | yes |
| R5 | reported | `[q_f, 1.25 q_f]`: 0 of 101 stalled, smallest incidence -5.99 deg; on G incidence -3.01 to 8.21 deg; `dp_s` 26.30 / 20.10 Pa at 0.4 / 0.7 `q_f` | - |
| NC1 | P's hub >= 90 deg | 92.699 deg | yes |
| NC2 | stagger -8 deg: stalled grid flows > 0 | 156 of 601, highest 0.555, `q_f` 0.043464 | yes |
| NC3 | `cl_max` 0.8: stalled grid flows > 0 | 52 of 601, highest 0.451 | yes |
| NC4 | hub chord 0.022: `AxialOverlap` at `stations[0].chord` | found, overlap 1.351e-3 m at r 0.0215 | yes |
| NC5 | P through camber_seam_h refused | refused: "the camber line does not run downstream monotonically" | yes |
| B1 | diff lists only this ADR, `fan_design.rs` (added lines only), the new test | `git diff --stat 32f3c320`: these three; `fan_design.rs` lines 449-477 added, 0 removed | yes |
| B2 | same pass / ignore counts as on the base | `fan_design_w46a` 13 passed 2 ignored, `--lib fan_design` 1 passed, `f713_schema_fan_rom` 5 passed 2 ignored, on the base (`base_b2.log`) and with the change (`b2.log`) | yes |

Usable operating range against quadratic systems (a8): at 3000 rpm the reference delivers 0.02826 m^3/s free (`dp_s` 0) and has
an operating point against `dp = k Q^2` for every `k` from 0 to 1.608e5 Pa s^2/m^6, i.e. for flows from `q_f` down to 0.457
`q_f` (0.01292 m^3/s, 26.83 Pa). A stiffer system meets the curve on its rising part below the peak and is refused, although no
strip stalls above 0.31 `q_f`.

Mutations (scratch copy `f722-scratch/mut`, `mutate.sh`, logs `mut_M*.log`): five of five caught.

| id | mutation | caught by |
|---|---|---|
| M1 | hub camber 0.06 | A1c, A3b (81 stalled), R1 (1.041), R2 (13.14 deg), R3, R4, A6a, A6b. Not A2 as expected: 69.8 + atan(0.3) = 86.5 deg < 90 (the expectation used P's stagger) |
| M2 | tip stagger 72.6 | A1c, A3b (129 stalled), R1 (1.105), R2, R3, R4, A6a to A6c |
| M3 | hub chord 0.022 | A1c, A4b (`AxialOverlap` 1.351e-3 m), R2 (8.048) |
| M4 | `r_tip` 0.059 | A1b, A4b (`FrameWall`), R1 (1.055), R4, A6a, A6c |
| M5 | stall threshold doubled | R3 (no onset), NC2 and NC3 (0 stalled) |

Defect of the mutation script, disclosed: its first M5 run (`mut_M5_stale.log`) built against the M4 source, because restoring
`fan_design.rs` by `mv` of the copy gave it an mtime older than the M4 build and cargo did not rebuild it. M5 was run again after
touching both files (`mut_M5.log`, above); M1 to M4 each edited the same file afresh and are not affected.

`cargo fmt -p rcfd-mesh --check`, `cargo fmt -p rcfd-post --check`: clean. `cargo clippy -p rcfd-mesh -p rcfd-post --all-targets
-- -D warnings`: clean. `baseline_hashes` not run: the only library change is a new associated function (and its rustdoc,
extended after the run with the measured numbers), which no existing path calls.

### Diagnosis of the A5b miss (scratch `zz_f722_diag.rs`, `diag1.log`, after the run; not a check)

- No in-range specification builds it: camber_seam_h refuses the reference at every `front_gap` in {0.15, 0.3, 0.5} and `n_nose`
  in {4, 6, 9} (inside the ADR 0079 control range for `n_chord` 24) with the same fold, and at `front_gap` 1.0 the front edge
  leaves the domain.
- The hub section is not the cause: the default with the reference's hub section (stagger 69.8, chord 0.016, camber 0) builds
  (12841 cells, 89.6 deg); the reference with its hub stagger lowered to 40 to 67.5 deg still folds (at layer 3 or 0); the
  default with the reference's tip (77.6 deg, camber 0) folds at layer 5 (mid-span). The fold is reported at the first folded
  layer of the whole coupled grid (`folded_quad` runs after the radial filter), so "layer 0" in the registered run does not
  locate the cause at the hub.
- The cause is the high stagger of the outer span (75.4 to 77.6 deg): the camber-seam C-grid builds its leading-edge fan from
  seams that leave the nose at the camber slope, and ADR 0079 states that its control range "does not make strongly staggered
  blades (FAN-01 tip, 70 deg) robust". The reference is above that. The ADR 0639 rule needs those staggers (zero-lift angles 75.4
  and 77.6 deg keep the outer strips attached at 0.4 `q_f`), so a5 and a7 conflict in the present generator, not in the rule.

### Deviations

The M1 expectation "A2" was wrong (see the mutation table); the mutation was caught by seven other checks. The mutation-script
defect above. Nothing else.

### Not run

The ADR 0118 mass, ensemble and unbalance tests, the FEM blade tests and the F-697 / F-698 objectives on the reference (nothing
reads it); flow solves on the h_slit mesh; the whole workspace (the lead's gate).

### Decision by the registered rule

**partial.** Every acceptance check but A5b is met, with B1, B2 and the five negative controls: `ComputerFan::rom_reference()`
sits beside the unchanged default, is a 120 mm 7-blade fan derived by the ADR 0639 rule with camber 0 at every station
(`stagger + atan(2 m / p)` 69.8 / 75.4 / 77.6 deg), has no stalled strip on [0.4, 1] `q_f` (nor down to 0.31), is accepted by
the checker, meshes with h_slit, and its usable range against quadratic systems is `k` <= 1.608e5 Pa s^2/m^6 (flows down to 0.457
`q_f`). It does not mesh with the camber_seam_h C-grid at the s4 specs (nor at any other in-range `front_gap` / `n_nose`), so a7
is half met; the cause is diagnosed (outer staggers above the generator's robust range).

### Remaining questions (each with options and a recommendation)

1. **The camber_seam_h C-grid on strongly staggered blades.** Options: (a) generator work so that camber_seam_h (or an opt-in
   construction beside it) builds blades with stagger up to about 80 deg, with the reference at the s4 specs as its acceptance
   case; (b) accept h_slit as the reference's mesh and drop the C-grid clause for it; (c) a second derivation rule with lower
   outer staggers (it would stall: ADR 0639's bound needs these zero-lift angles unless the ROM's attached window widens).
   Recommendation: (a) as a follow-up (proposed below), with (b) as the state meanwhile; (c) is ruled out by ADR 0639's bound.
2. **The h_slit mesh quality of the reference** (85.4 deg non-orthogonality, skewness 16.7): reported, no limit was registered.
   Recommendation: leave it to the follow-up of question 1, which registers quality limits for both generators on the reference.
3. **Where the reference is used.** Options: (a) nothing yet (now); (b) make it the base of the F-697 / F-698 objectives in place
   of set-up F of ADR 0620. Recommendation: (a) here; (b) needs its own item, as it changes the inputs of registered tests.

## For the lead: roadmap entries

> Proposed status: partial

> Follow-up of F-713 (ADR 0639 questions 1 to 3). ADR 0643, 2026-11-16. PARTIAL (the C-grid clause missed). ComputerFan::rom_reference() beside the unchanged default (ComputerFan::default() and every existing test unchanged: fan_design_w46a 13 passed 2 ignored, f713_schema_fan_rom 5 passed 2 ignored, before and after; only added lines in fan_design.rs). A 120 mm 7-blade fan derived by the ADR 0639 rule with camber 0 at every station (stagger 69.8 / 75.4 / 77.6 deg, chord 0.016 / 0.029 / 0.036 m, every other field the default's; stagger + atan(2 m / p) below 90 deg everywhere). In RomModel::default() at 3000 rpm: free delivery 0.02826 m^3/s (the Python derivation reproduced to 5e-6), no stalled strip at any of 601 flows on [0.4, 1] q_f (largest incidence 8.21 deg against 10.03), stall from 0.31 q_f; accepted by ComputerFan::check with Limits::default(); meshed by h_slit at the s4 specs (11648 cells, max non-orthogonality 85.4 deg). Usable range against dp = k Q^2: k up to 1.608e5 Pa s^2/m^6, flows from q_f down to 0.457 q_f (26.83 Pa peak of the static characteristic; stiffer systems meet its rising part and are refused); q / q_f 0.915 / 0.847 / 0.705 / 0.563 at k 1e4 / 2e4 / 5e4 / 1e5. Missed (known-miss record): the camber_seam_h C-grid folds its leading-edge fan at every in-range front_gap and n_nose; diagnosed to the outer staggers of 75 to 78 deg (the default with the reference's hub section builds, the default with its tip does not), above the range ADR 0079 states the construction is robust for. Negative controls: stagger -8 deg stalls from 0.555 q_f, cl_max 0.8 from 0.451, hub chord 0.022 gives AxialOverlap, P's hub (92.7 deg) is refused by camber_seam_h. Five of five mutations caught.

> Follow-up proposed: camber_seam_h C-grid on strongly staggered blades. Deps F-722. Acceptance: Every limit not stated here is registered in the ADR before the run. The camber_seam_h C-grid (or a new opt-in construction beside it, camber_seam_h unchanged in bits on every existing case) builds ComputerFan::rom_reference() blade 0 at the ADR 0118 s4 specs with no folded cell and registered non-orthogonality, skewness and aspect limits, and the ADR 0079 and ADR 0083 cases keep their results; the h_slit mesh of the reference is measured against the same limits.
