# ADR 0639: F-713 registration: the schema default fan in the reduced-order model (why it stalls, and what a usable default costs)

Status: **REGISTERED 2026-11-16, written before any code of F-713 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-713 was written. One diagnostic run of the base code was made before
this text (disclosed below, "Diagnosis"), from a scratch test file that was moved out of the worktree after the run and never
committed (`w49i-work/f713-scratch/zz_f713_diag.rs`, log `diag1.log`), and the derivation arithmetic of set-up P was done in
Python scripts in the scratch folder (`derive.py`, `derive2.py`, `derive3.py`, `derive4.py`; disclosed below). Results go in
the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-713 (deps F-697), proposed by [ADR 0620](0620-f697-registration-fan-objective-over-impedances-and-tolerances.md)
addendum 1, remaining question 2. Parents: [ADR 0118](0118-wave46a-computer-fan-schema-checker-perturbations.md) (the schema
`rcfd_mesh::fan_design::ComputerFan`, its checker and ensembles), [ADR 0119](0119-wave46b-reduced-order-axial-fan-tier.md)
(the reduced-order fan `rcfd_post::fan_rom`). Number 0639 given by the lead. Branch `f713-work` from d40cd665, worktree
`w49i-work/f713`, scratch `w49i-work/f713-scratch`.

House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here; each registered run made once; existing tests not edited;
defaults bit-identical; no edit of `baseline_hashes.rs`, of a hash table or of a golden; no new dependency, no unsafe code;
clean-room; the roadmap files are not edited. Every choice the maintainer would normally be asked about is taken on the
maintainer's standing instruction of 2026-11-11 (take the recommendation), except where listed as open.

Clean-room: everything below is the reduced-order model's own equations (ADR 0119: blade-element strips, thin-airfoil lift
`2 pi alpha_e` up to `cl_max`, Euler work, free-discharge static pressure) and elementary geometry of the ADR 0118 checker. No
source outside the repository was read.

## What F-713 asks

> ComputerFan::default() through RomFan::from_axial_fan runs unstalled at 3000 rpm for every flow above 40 percent of its free delivery in the reduced-order model, every ADR 0118 schema, checker and ensemble test keeps its result, and no golden or hash table changes (or the needed change is listed for the maintainer before any edit). Every limit not stated here is registered in the ADR before the run.

Clauses:

- **a1** `ComputerFan::default().axial_fan(0, 0, 0)` through `RomFan::from_axial_fan`, `FanRom::new(.., RomModel::default())`,
  3000 rpm (rho 1.2, nu 1.5e-5 as ADR 0620), has no stalled strip at any flow in `[0.4 q_f, q_f]`, `q_f` its free delivery.
  Reading of "every flow above 40 percent of its free delivery": the fan curve of a fan ends at free delivery (`dp_s = 0`);
  above it the fan is driven by the system and has no operating point against a passive system. The binding range is therefore
  `[0.4 q_f, q_f]`; `[q_f, 1.25 q_f]` is reported, without a limit.
- **a2** every ADR 0118 schema, checker and ensemble test (`crates/rcfd-mesh/tests/fan_design_w46a.rs`) keeps its result;
- **a3** no golden or hash table changes, or the needed change is listed for the maintainer before any edit.

## What the code read shows (base d40cd665, read only)

1. `ComputerFan::default()` (`fan_design.rs` 216-257): 7 blades, `r_hub` 0.0215, `r_tip` 0.058, tip gap 1 mm, NACA 4 sections,
   three control points (span 0, 0.5, 1): chord 0.022 / 0.029 / 0.036 m, stagger 40 / 52 / 62 deg, camber 0.06 / 0.055 / 0.05
   at 0.4, thickness 0.055 / 0.042 / 0.035, skew 0 / 4 / 8 deg, no sweep. `PARAMS` carries defaults for the scalars only; the
   station fields have `default: None`.
2. No golden file, hash table or `baseline_hashes.rs` entry holds a `ComputerFan` (searched: `rcfd-mcp`, `rcfd-cli`,
   `rcfd-api/src`, every `.json` golden; only `docs/roadmap.json` names the type). `a3` is therefore met by any change that
   touches library code only, and trivially by a change that touches none.
3. `ComputerFan::default()` is the input of more than the ADR 0118 tests: `rcfd-fem/tests/sb_fan_blade.rs` and
   `sb_fan_blade_builder.rs` (structural blade, registered numerics on blade 0 of the default), `rcfd-api/tests/f697_fan_objective.rs`
   (set-up U, the static unbalance of the default) and the `..ComputerFan::default()` base of set-ups F in F-697 and F-698.
   Changing the default changes the bytes those tests compute (house rule 3).
4. The ROM stall rule (`fan_rom.rs` `solve_strip`): a strip is attached while `|alpha_e| <= cl_max / (2 pi)` = 10.031 deg with
   `cl_max` 1.1; `FanRom::operating_point` refuses any point with a stalled strip. `dp_s = dp_t - rho c_x^2 / 2 - swirl / q`
   (free discharge, no stator).
5. The ADR 0118 checker tests that depend on the station values of the default are the fifteen `bad_variants` of
   `c2_c3_constructed_violations_are_located` (each edits the default and expects a code at a path), `c1` (the default is
   accepted), `s4` (meshes of blade 0 build) and the mass / unbalance tests (`i1` to `i4`, statistics and quadrature limits).
   Two variants pin the axial extent of the blade near the hub: **"axial fit"** (`z_le_root` 0.006: the blade must reach past
   `depth - strut_depth - min_axial_gap` = 0.022 m at a radius whose nearest control point is station 0, i.e. span <= 0.25)
   and **"root off hub"** (hub from 0.004: the root section must start before 0.004, so the hub leading edge cannot be swept
   back by 2 mm or more).

## Diagnosis (one run of the base code, disclosed)

Run (`zz_f713_diag.rs`, release): the default through the chain of a1, free delivery from a scan of `dp_s` at 0.5 L/s steps with
linear interpolation, then the strips at fractions of it. Measured:

- free delivery `q_f` about 0.07650 m^3/s (`c_x` 8.39 m/s; flow coefficient `phi = c_x / U` 1.217 at the hub strip, 0.464 at
  the tip strip);
- at `q_f`: `dp_t` 50.7 Pa, of which the exit axial dynamic pressure `rho c_x^2 / 2` is 42.3 Pa (83 %); incidence of strips
  0, 10, 20, 30, 39: 2.13, 4.45, 5.58, 6.04, 5.60 deg (stall at 10.03);
- at 0.80 `q_f` no strip stalled (largest incidence 9.79 deg at strip 30); at 0.75 `q_f` 25 of 40 strips stalled, strips 0 to
  10 attached (strip 10 at 9.18 deg); at 0.50 `q_f` and below all 40 stalled.

The diagnosis, registered:

1. **The free delivery is already loaded.** With free discharge, `dp_s = 0` needs `dp_t = rho c_x^2 / 2 + swirl`, so every
   strip sits at 2 to 6 deg of incidence at `q_f`; only 4 to 8 deg of the 10 deg attached window is left for the whole
   throttled range.
2. **The default's flow coefficient at free delivery is high** (0.46 at the tip). Between `q_f` and `0.4 q_f` the inflow angle
   from the axis at the tip strip rises by `atan(1 / (0.4 phi)) - atan(1 / phi)` = 14.4 deg at `phi` 0.464 (closed form);
   the loading feedback (more swirl lowers the vector-mean relative velocity) takes off about a third of it (`pi sigma phi /
   (2 sqrt(1 + phi^2))` = 0.4 at the tip solidity 0.69), which is still more than the 4 to 6 deg left. The outer three
   quarters of the span reach `cl_max` first, at about 0.78 `q_f`; the hub, with its higher `phi` and solidity, last.
3. Neither the mapping `RomFan::from_axial_fan` (a field copy) nor the ROM has a defect here: the stall comes from the default's
   geometry against the ROM's narrow attached window (no cascade or 3-D effect raises `cl_max`; ADR 0119 "NOT modelled").
   The ADR 0119 set-up C free-vortex row behaves the same way (ADR 0620 smoke 2: unstalled only above 76 % of its free
   delivery).

### What a default meeting a1 needs, and why it conflicts with a2 (derivation, disclosed arithmetic)

`derive.py`: the strips of ADR 0119 with every loss off (attached branch, `dc = pi sigma W alpha_e`, `dp0 = rho U dc`), the
free delivery from `sum (U dc - dc^2/2) dq = c_x^2 / 2 * q`. It reproduces the default to 4 % in `q_f` (0.0799 against 0.0765)
and 0.5 deg in incidence.

- Keeping a strip attached from `q_f` down to `0.4 q_f` while the free delivery stays loaded needs a low flow coefficient at
  free delivery, i.e. a zero-lift line far from the axis. Bound: fix the hub zero-lift angle `gamma_h` (stagger plus
  `alpha_L0`), take the smallest `c_x` at which the hub strip is still attached at 0.4 of it, and put every other strip at the
  stall limit at `0.4 q_f` (the largest loading the window allows); if `dp_s < 0` at that `c_x` even then, no geometry with that
  hub angle can meet a1. The smallest feasible `gamma_h` is 61.8 deg with the default chords, 60.1 deg at solidity 1.1 and 58.1
  deg at solidity 1.3 everywhere (losses would raise it).
- The ADR 0118 variant "axial fit" needs the blade's axial extent near the hub above about `0.016 m - sweep`, and "root off hub"
  needs the hub sweep below 2 mm, so the extent must exceed about 0.014 m; the default's own axial-overlap check bounds the
  circumferential extent `c sin(stagger)` by the hub pitch 0.0193 m. Together (thin sections) `cot(stagger) >= 0.014 / 0.0193`,
  stagger at most about 54 deg near the hub, and with camber `>= 0` the zero-lift angle is at most that. 54 < 58: **estimate,
  not a proof** (thick sections and other chords move both numbers by a few degrees): a default that meets a1 makes the "axial
  fit" variant stop finding its violation, i.e. changes an ADR 0118 checker result (a2).

So the item cannot be done by editing `ComputerFan::default()` without changing an ADR 0118 test result (and the default's bytes,
house rule 3). This ADR therefore **does not edit `ComputerFan::default()` or any library code**. It registers the diagnosis as
checks, builds the candidate default P that the derivation gives (in the test file only), measures a1 on it, measures exactly
which ADR 0118 results it would change, and lists that for the maintainer.

### Set-up P, the candidate default (derived, fixed now)

`derive3.py` / `derive4.py`: the ADR 0119 equations with the default loss terms (laminar friction `2.656 / sqrt(Re)`, annulus
drag `0.020 s / h`, secondary drag `0.018 C_L^2`, tip leakage with `C_d = pi / (pi + 2)`), attached branch, 40 strips, an
independent Python re-implementation of the formulas in `fan_rom.rs`. Design rule (fixed before any number was seen): at
each of the three control-point radii, the zero-lift angle `gamma` is the one that puts that radius at `alpha_e = 7.5 deg`
(2.5 deg below stall) at `0.4 q_f`, with `q_f` solved self-consistently (bisection on `c_x` until the designed blade's `dp_s`
is 0 at `q_f`). Stations are linear in span, as the schema.

Other choices, each with its reason:

- Everything but the stations is the default's (n_blades, radii, gap, `z_le_root`, family, hub, frame, density, roughness), so
  the `PARAMS` defaults and `s2` are untouched.
- Three control points at span 0, 0.5, 1 (the `bad_variants` index `stations[2]` as the tip).
- Hub station keeps camber 0.06 at 0.4 and thickness 0.055 (the collision, passage-gap, axial-overlap, draft and undercut
  variants override the hub stagger, chord or thickness but keep its camber, so their hub sections stay the base's sections);
  hub chord 0.016 (instead of 0.022) so that the high hub stagger does not overlap axially (`0.016 sin 76 = 0.0155 < 0.0193`;
  thickness 0.88 mm >= `min_wall` 0.8 mm; solidity 0.83).
- Mid and tip camber 0 (the zero-lift angles 75.4 and 77.6 deg would need stagger 81 and 83 deg with the default cambers,
  outside the `PARAMS` bound 79 deg); chord and thickness as the default.
- Stagger = `gamma - alpha_L0`, `alpha_L0` -6.232 deg at the hub (measured in the diagnosis), 0 at camber 0; rounded to 0.1 deg.

| station | span | chord [m] | stagger [deg] | camber | camber_pos | thickness | sweep | skew [deg] |
|---|---|---|---|---|---|---|---|---|
| hub | 0 | 0.016 | 76.0 | 0.06 | 0.4 | 0.055 | 0 | 0 |
| mid | 0.5 | 0.029 | 75.4 | 0 | 0.4 | 0.042 | 0 | 4 |
| tip | 1 | 0.036 | 77.6 | 0 | 0.4 | 0.035 | 0 | 8 |

Predicted (Python, rounded stations): `q_f` = 0.028266 m^3/s; at `0.4 q_f` the largest incidence 8.228 deg (the interpolation
between the stations adds 0.7 deg to the design value); over `[0.4, 1] q_f` incidence from -2.99 to 8.23 deg; `dp_s` 26.3 Pa at
0.4 `q_f`, 20.1 at 0.7; stall onset at 0.311 `q_f`. P is a fan of low flow coefficient (about 0.15 at the tip at free delivery):
in this model it delivers 0.028 m^3/s against 0 Pa and about 26 Pa at 0.011 m^3/s at 3000 rpm. Not a measured fan.

Negative-control predictions (same Python): P with every stagger 8 deg lower (NC2) stalls from 0.556 of its own `q_f`
(0.04347); P with `cl_max` 0.8 (NC3) from 0.452 `q_f`.

## Common procedure (fixed now)

`chain(cf, model)`: `cf.axial_fan(0, 0.0, 0.0)` -> `RomFan::from_axial_fan` -> `FanRom::new(.., model)`; `Operating { rpm 3000,
rho 1.2, nu 1.5e-5 }`. Free delivery `q_f`: `FanRom::operating_point(op, SystemCurve { k: 0, l: 0 }, 1e-4, 0.12, 400)` (the
highest-flow zero of `dp_s`, with its slope and stall refusals). Grid G: 601 flows `q_j = q_f (0.4 + 0.6 j / 600)`, `j = 0..600`.
Onset fraction `f_s`: for D1 the highest grid fraction with a stalled strip plus 0.001; for P the first fraction, stepping down
from 0.400 by 0.001, at which a strip is stalled. "Stalled" is `FanPoint::stalled` non-empty (branch not `Attached`).

## Checks (limits fixed now)

| id | clause | what | limit | estimate behind it |
|---|---|---|---|---|
| D1a | a1, NC1 | default: `q_f` | within 1 % of 0.07650 | diagnosis (interpolated scan, so not exact) |
| D1b | a1, NC1 | default: number of stalled grid flows | > 0 (the default misses a1) | diagnosis: stalled at 0.75 |
| D1c | diagnosis 2 | default: `f_s` | in [0.75, 0.80] | diagnosis: 0.80 attached, 0.75 stalled |
| D1d | diagnosis 2 | default: strips 0 to 9 at the highest stalled grid flow | none stalled | diagnosis: strip 10 attached at 0.75 |
| D2a | diagnosis 1 | default at `q_f`: incidence of every strip | in [1, 7] deg | diagnosis 2.13 to 6.04 on five strips |
| D2b | diagnosis 1 | default at `q_f`: `(rho c_x^2 / 2) / dp_t` | in [0.75, 0.90] | diagnosis 0.83 |
| D2c | diagnosis 2 | default: `atan(1/(0.4 phi)) - atan(1/phi)`, `phi` of strip 39 at `q_f` | >= 14 deg | 14.4 at 0.464 |
| P1a | a1 on P | `operating_point` for `q_f` | succeeds | derivation |
| P1b | a1 on P | stalled grid flows | 0 | largest incidence 8.23 < 10.03 |
| P1c | a1 on P | `dp_s` along G | strictly decreasing (every step < 0) | derivation 26.3 -> 20.1 -> 0 |
| P1d | derivation | `q_f` | within 1 % of 0.028266 | Python re-implementation of the same equations |
| P1e | derivation | largest incidence at `0.4 q_f` | within 0.15 deg of 8.228 | same |
| P1f | derivation | `f_s` | in [0.28, 0.34] | 0.311 |
| P1g | reported | smallest incidence and stalled count on 101 flows in `[q_f, 1.25 q_f]` | none | |
| P2a | a2 (c1, NC1 of ADR 0118) | P: every scalar in `PARAMS` bounds; `P.check(&Limits::default())` | in bounds; no violation | geometry in "Set-up P" |
| P2b | a2 (s2) | P's fields other than `stations` | equal to the default's | by construction |
| P3 | a2 (c2, c3) | the fifteen `bad_variants` edits of `fan_design_w46a.rs` applied to P (copied verbatim) | the set of variants that do not find their (code, path, radius) is exactly {"axial fit"}; the C3 mould-off control removes the code for "axial overlap", "draft", "undercut" | derivation (the conflict) |
| P4 | a2 (s4) | P blade 0 (`axial_fan(0, 0.03, 0.04)`): `build_sector_mesh` and `build_cgrid_mesh_with(camber_seam_h)` with the `s4` specs (copied) | both build, cells > 0; pursuit reported | ADR 0118 s4 |
| NC2 | NC | P with every stagger -8 deg: stalled grid flows (on its own `q_f`) | > 0 | Python: onset 0.556 |
| NC3 | NC | P with `RomModel { cl_max: 0.8, .. }`: stalled grid flows (own `q_f`) | > 0 | Python: onset 0.452 |
| B1 | a3, rule 3 | no library file, golden, hash table or existing test edited | `git diff --stat d40cd665` lists only this ADR, the new test file and `docs/fan-project-breakdown.md` at most | by construction |

D1 is the negative control of the a1 check: the same procedure that P must pass fails on the unchanged default.

## Planned mutations (scratch copy of the worktree, each run once against the new tests)

| id | mutation | expected to be caught by |
|---|---|---|
| M1 | `fan_rom.rs` `solve_strip`: `a_s = cl_max / TAU` -> `2.0 * cl_max / TAU` | D1b, NC3 |
| M2 | `fan_rom.rs` `point`: drop `- 0.5 * op.rho * cx * cx` from `dp_s` | D1a, P1d |
| M3 | `fan_rom.rs` `FanRom::new`: `a0: alpha_l0(..)` -> `a0: -alpha_l0(..)` | D1a or D2a, P1d |
| M4 | `fan_design.rs` `check`: never push `Code::AxialOverlap` | P3 |

## Decision rule

- **done**: a1, a2 and a3 met by `ComputerFan::default()`. Not reachable in this ADR: the default is not edited (it would change
  the default's bytes and, by the derivation, an ADR 0118 result).
- **partial**: D1 and D2 met (the diagnosis holds) and B1 met. The P results then say what the maintainer can choose: P1 and P2
  met with P3 as predicted means "a default meeting a1 exists, and adopting it needs the listed ADR 0118 re-base".
- **todo**: D1 or D2 missed (the diagnosis is wrong).

## Limits (not covered)

- No change of `ComputerFan::default()`; a1 is not met by the default in this ADR, by design of the rule above.
- The FEM tests on the default blade (`sb_fan_blade*.rs`), the F-697 set-up U and the ADR 0118 mass, ensemble and unbalance tests
  are not run on P (they would only run on it if the default changed).
- The infeasibility of a1 together with "axial fit" is an estimate on thin sections and the lossless model, not a proof.
- P is a model fan (no measured data); its low flow coefficient is what the ROM's 10 deg attached window demands, not a claim
  about real 120 mm fans, whose stall lies outside what the ROM models (ADR 0119).
- Other speeds than 3000 rpm: the strip equations are Reynolds-dependent only through the friction term, so the stall fraction
  moves little, but it is not checked.

## Addenda

### Amendment 1 (2026-11-16, written after the test code was written and compiled, before any run of it)

One control added to P3, no limit changed: **P3-0**, the fifteen copied variants applied to the unchanged
`ComputerFan::default()` must all hold (code found at the path, radius where given, NC1 in bounds, no `out_of_bounds`, blade 3
only for "thin blade 3", C3 for the mould codes). It checks the verbatim copy of `bad_variants` against the ADR 0118 result.
Limit: every variant holds. The tests are in `crates/rcfd-post/tests/f713_schema_fan_rom.rs` (rcfd-post depends on rcfd-mesh,
so one file reaches the schema, the checker, the sector generators and the ROM).

### Amendment 2 (2026-11-16, after the one registered run, before any further run)

The registered run missed P1c and P4 (results in addendum 1). By rule 2 the two tests become ignored records with their limits
intact: `p1_the_candidate_is_unstalled_down_to_forty_percent_known_miss` and `p4_the_candidate_meshes_as_s4_known_miss`. So
that the met clauses of P1 keep running, one test is added, `p1_the_candidate_is_unstalled_down_to_forty_percent_met_limits`:
the same code and the same limits for P1a, P1b, P1d, P1e and P1f, without P1c (it is the same deterministic computation, so
its numbers are those of the registered run; it is run once to confirm that). No limit is changed. One diagnostic run after
the miss (scratch `zz_f713_diag2.rs`, log `diag2.log`, never committed) located the rise of P1c and evaluated three system
curves on P; it is reported in addendum 1 as diagnosis, not as a check.

## Addendum 1: results (2026-11-16)

One registered run (`f713-scratch/run1.log`, release, `cargo test -p rcfd-post --release --test f713_schema_fan_rom`), then
after amendment 2 one run of the split tests (`run2.log`: 5 passed, 2 ignored; the numbers are those of run 1).

| id | limit | measured | met |
|---|---|---|---|
| D1a | `q_f / 0.07650` in [0.99, 1.01] | 1.000014 (`q_f` 0.0765011 m^3/s) | yes |
| D1b | stalled grid flows > 0 | 390 of 601 | yes |
| D1c | `f_s` in [0.75, 0.80] | 0.790 | yes |
| D1d | strips 0 to 9 attached at the highest stalled grid flow | stalled strips 27, 28, 29 at 0.789 `q_f` | yes |
| D2a | incidence at `q_f` in [1, 7] deg | 2.128 to 6.045 deg | yes |
| D2b | `(rho c_x^2/2) / dp_t` in [0.75, 0.90] | 0.8328 (`dp_t` 50.74 Pa) | yes |
| D2c | tip inflow-angle rise >= 14 deg | 14.382 deg at `phi` 0.4642 | yes |
| P1a | `operating_point` for `q_f` succeeds | ok, 0.0282657 m^3/s | yes |
| P1b | stalled grid flows 0 | 0 of 601 | yes |
| P1c | `dp_s` strictly decreasing along G | largest step +1.925e-2 Pa: `dp_s` rises from 26.308 Pa at 0.4 `q_f` to a peak of 26.838 Pa at 0.457 `q_f` | **no** (record `p1_..._known_miss`) |
| P1d | `q_f / 0.028266` in [0.99, 1.01] | 0.999988 | yes |
| P1e | largest incidence at 0.4 `q_f` within 0.15 deg of 8.228 | 8.2282 deg | yes |
| P1f | `f_s` in [0.28, 0.34] | 0.310 | yes |
| P1g | reported | `[q_f, 1.25 q_f]`: 0 of 101 stalled, smallest incidence -5.97 deg; on G incidence -2.99 to 8.23 deg; `dp_s` 26.31 / 20.11 Pa at 0.4 / 0.7 `q_f` | - |
| P2a | P in bounds, no checker violation | in bounds, no violation | yes |
| P2b | fields other than stations equal the default's | equal | yes |
| P3-0 | every copied variant holds on the default | all fifteen hold (as ADR 0118 c2 / c3) | yes |
| P3 | lost set on P exactly {"axial fit"}; C3 holds | {"axial fit"} (no `AxialFit` at all; on the default the value is 2.2853e-2 > 0.022 m); the other fourteen and C3 hold | yes |
| P4 | h_slit and camber_seam_h build P blade 0 | both rejected at the hub layer r = 0.0215: h_slit "axial plane z = 3.19866e-2 misses the blade surface [3.2e-2, 3.58708e-2]", camber_seam_h "the camber line does not run downstream monotonically"; pursuit (reported) rejected, the wake line leaves the passage | **no** (record `p4_..._known_miss`) |
| NC2 | stagger -8 deg: stalled grid flows > 0 | 156 of 601, highest stalled 0.555 (Python 0.556), `q_f` 0.043471 | yes |
| NC3 | `cl_max` 0.8: stalled grid flows > 0 | 52 of 601, highest stalled 0.451 (Python 0.452) | yes |
| B1 | diff lists only this ADR and the new test | `git diff --stat d40cd665`: the ADR and `crates/rcfd-post/tests/f713_schema_fan_rom.rs` | yes |

Mutations (scratch copy `f713-scratch/mut`, `mutate.sh`, logs `mut_M*.log`): four of four caught.

| id | mutation | caught by |
|---|---|---|
| M1 | stall threshold doubled | D1c (0.421), NC2 and NC3 (0 stalled), P1f |
| M2 | `rho c_x^2 / 2` dropped from `dp_s` | D1a (1.382), D1c, D2a, D2b, P1d (1.078), P1e |
| M3 | sign of `alpha_L0` flipped | D1a (0.751), D1c, D2a, D2c, NC2 and NC3 (no `q_f`: stalled), P1d, P1f |
| M4 | `AxialOverlap` never pushed | P3-0 and P3 ("axial overlap" lost) |

`cargo fmt -p rcfd-post --check`: clean. `cargo clippy -p rcfd-post --all-targets -- -D warnings`: clean. No library file was
changed, so no existing test can change its result; `baseline_hashes` was not run (no shared solver file touched).

### Diagnosis of the two misses

- **P1c.** Every strip of P is attached on `[0.4, 1] q_f`, but the total-to-static characteristic peaks at 0.457 `q_f`. Between
  0.457 and 0.4 `q_f` (`diag2.log`): `dp_t` rises 43.44 to 45.88 Pa, the exit axial dynamic pressure falls 1.21 to 0.92 Pa, and
  the swirl kinetic energy lost at the free discharge (no stator) rises 15.40 to 18.65 Pa, so `dp_s` falls 26.84 to 26.31 Pa.
  A high-stagger rotor puts most of its work into swirl. Consequence: `FanRom::operating_point` refuses a system that crosses
  P's curve below 0.457 `q_f` ("rising fan characteristic: stall region") although no strip is stalled; against `dp = k Q^2`
  P gives 0.705 `q_f` at `k` 5e4 and 0.563 `q_f` at 1e5, and refuses 2e5. The registered clause a1 ("runs unstalled", the strip
  criterion) is met by P; the stricter P1c is not.
- **P4.** At the hub, stagger 76 deg with NACA camber 0.06 at 0.4 makes the camber line's leading-edge direction `76 + atan(2 m /
  p) = 92.7 deg` from the axis: the camber line runs upstream first, so camber_seam_h cannot carry its seams, and the h_slit
  axial planes miss the very short hub section (axial extent about 3.9 mm). A meshable candidate needs `stagger + atan(2 m / p)`
  below 90 deg with some margin at every section; at the hub that means camber below about 0.03 at 0.4, or none. This is a
  second ADR 0118 result (s4) that P would change, which the registration did not predict.

### Deviations

None from the registration and amendments. The scratch diagnostic `zz_f713_diag2.rs` ran after the registered run, located the
P1c rise and evaluated three system curves; it is reported above as diagnosis only.

### Not run

The ADR 0118 mass, ensemble and unbalance tests, the FEM blade tests (`sb_fan_blade*.rs`) and the F-697 / F-698 tests on P:
they read `ComputerFan::default()`, which is not changed. The whole workspace (the lead's gate).

### Decision by the registered rule

**partial.** D1, D2 and B1 are met: the diagnosis holds (the default's free delivery is already loaded to 2 to 6 deg of the 10
deg attached window, and its high flow coefficient at free delivery makes the tip inflow angle rise 14.4 deg down to 0.4 `q_f`;
stall from 0.79 `q_f`, outer strips first). Clause a1 is not met by `ComputerFan::default()`, which this ADR does not change:
a default that meets a1 changes ADR 0118 results. Measured on the derived candidate P: a1 is met (no stalled strip down to 0.31
`q_f`), the checker accepts it and the `PARAMS` scalars are untouched, but the "axial fit" checker variant loses its violation
(predicted) and neither sector generator meshes its hub (not predicted). a3: no golden or hash table holds a `ComputerFan`;
none changes.

### For the maintainer: what adopting P as `ComputerFan::default()` would change (not done)

- `fan_design.rs` `Default for ComputerFan`: the three stations of set-up P (table in "Set-up P"); no other field, no `PARAMS`
  entry.
- `fan_design_w46a.rs`: the `c2` variant "axial fit" (`z_le_root` 0.006) finds no `AxialFit` on P; it needs a re-based edit (for
  example a larger `z_le_root` or a hub sweep) and the maintainer's approval to edit an existing test. `s4` fails on P (both
  generators); P's hub camber would have to change first, and then the collision and passage-gap variants, which keep the hub
  camber, must be checked again. `i1` to `i4`, `e1` to `e3`, `s1` to `s3`, `c1`, `c4`, `c5`: not run on P.
- The default's serialised bytes change (house rule 3), and with them the inputs of `sb_fan_blade.rs`, `sb_fan_blade_builder.rs`
  (structural blade numerics) and of the F-697 set-up U (static unbalance); not run on P.
- No golden, hash table or `baseline_hashes.rs` entry changes.

### Remaining questions (each with options and a recommendation)

1. **Where should a schema fan usable in the ROM live?** Options: (a) leave the default as it is and keep set-up F of ADR 0620
   as the ROM's working row; (b) change `ComputerFan::default()` to a meshable variant of P and re-base the ADR 0118 "axial
   fit" variant, the s4 meshes and the FEM blade tests (a default change with edits of existing tests: the maintainer's call);
   (c) a named constructor next to the default (for example `ComputerFan::rom_reference()`) holding a meshable variant of P,
   default unchanged. Recommendation: (c). The default is the reference of the checker variants and of the structural tests,
   and the ROM needs a low-flow-coefficient blade that real 120 mm fans do not resemble. Not taken here: (b) changes a default
   result, and (c) does not deliver what the item names (the default).
2. **The rise of P's static characteristic below 0.457 `q_f` (P1c).** Options: (a) accept it (a1 asks for unstalled strips) and
   state that operating points exist on `[0.46, 1] q_f`; (b) require a falling characteristic down to 0.4 `q_f` in the
   follow-up, which needs less swirl at low flow (a lower design incidence, hence a still lower flow coefficient).
   Recommendation: (a) for the ROM tier, with the usable operating range stated wherever the reference fan is documented.
3. **Meshability of the high-stagger hub (P4).** Options: (a) hub camber 0 (zero-lift angle = stagger, 69.8 deg) in the
   follow-up's fan, derived again by the same rule; (b) teach the sector generators camber lines that run upstream first.
   Recommendation: (a); (b) is generator work for a blade no one asked to mesh.

## For the lead: roadmap entries

> Proposed status: partial

> Follow-up of F-697 (ADR 0620 question 2). ADR 0639, 2026-11-16. PARTIAL (diagnosis; the default is not changed). Why ComputerFan::default() stalls in the reduced-order model (registered, checked on the base code): at 3000 rpm its free delivery is 0.0765 m^3/s and is already loaded (free discharge needs dp_t = rho c_x^2 / 2 + swirl; 83 percent of dp_t is the exit dynamic pressure), so every strip sits at 2.1 to 6.0 deg of the 10.03 deg attached window (cl_max 1.1 / 2 pi); with its high flow coefficient (0.46 at the tip at free delivery) the tip inflow angle rises 14.4 deg down to 0.4 q_f, and the outer strips stall from 0.79 q_f. Neither from_axial_fan nor the model is at fault. A default meeting the acceptance needs a hub zero-lift angle of about 58 deg or more (lossless bound), which the ADR 0118 checker variants "axial fit" and "root off hub" do not allow together (estimate: hub stagger at most about 54 deg). Measured on the derived candidate P (stagger 76.0 / 75.4 / 77.6 deg, hub chord 0.016, hub camber 0.06, mid and tip camber 0, every other field the default's): no stalled strip at any of 601 flows on [0.4, 1] q_f (q_f 0.02827 m^3/s, largest incidence 8.23 deg, stall from 0.31 q_f, the Python derivation reproduced to 1.2e-5 in q_f), accepted by the checker; but the "axial fit" variant loses its violation (the other fourteen keep theirs), neither sector generator meshes its hub (the camber line runs upstream first), and its static characteristic peaks at 0.457 q_f (two known-miss records). Negative controls: stagger -8 deg stalls from 0.555 q_f, cl_max 0.8 from 0.451. Four of four mutations caught. No library file, golden or hash table changed. Open for the maintainer: whether a ROM-usable schema fan replaces the default (re-basing the ADR 0118 axial-fit variant, s4 and the FEM blade tests) or sits beside it (recommended).

> Follow-up proposed: Reduced-order reference fan in the schema. Deps F-713. Acceptance: Every limit not stated here is registered in the ADR before the run. A named ComputerFan constructor beside the default (ComputerFan::default() and every existing test unchanged in bits and result) gives a 120 mm 7-blade fan, derived by the ADR 0639 rule with a hub section whose camber line runs downstream (stagger + atan(2 m / p) below 90 deg at every station), that runs unstalled in RomModel::default() at 3000 rpm for every flow in [0.4, 1] of its free delivery, passes ComputerFan::check with Limits::default(), and meshes with h_slit and the camber_seam_h C-grid at the ADR 0118 s4 specs; its usable operating range against quadratic systems is reported.
