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
