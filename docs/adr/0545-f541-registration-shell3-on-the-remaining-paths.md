# ADR 0545: F-541 registration: Shell3 on the remaining paths (the N2 refusals of ADR 0454, the Shell4 checks repeated on triangles)

Status: **REGISTERED 2026-11-15, written before any code of F-541 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-541 was written, compiled or run: the files under "What was read" were read, and the
arithmetic below was done by hand (no script, no rcfd run; disclosed). Results go in the addenda at the end; nothing above the
addenda is changed after a run.

Roadmap item F-541 (deps F-419), the follow-up named by [ADR 0454](0454-f419-registration-three-node-flat-shell.md) (its row N2
and its "For the lead" entry). Number 0545 given by the lead. Branch `f541-work` from f69eb128, worktree `w49i-work/f541`,
scratch `w49i-work/f541-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact and the measured number); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code; clean-room. The
brief asks for the F-541 entry of `docs/ROADMAP.md` and `docs/roadmap.json` to be updated by this branch in a separate last
commit (the svgs not regenerated). Areas of other agents not touched: `contact.rs` (F-601), the factor choice of the Newton
driver in `nonlinear.rs` (F-618), `nl_transient/dg.rs` and the energy-momentum step (F-611). Every choice below that the
maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation).

## What F-541 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. The N2 refusals replaced by the Shell4 checks repeated on triangles.

The roadmap note: "nonlinear, transient, buckling, cyclic, multigrid take Shell3 when they take Shell4. Also depends on the Shell4
follow-ups of ADR 0432."

Clauses: **a0** every limit registered here; **a1** each refusal of the ADR 0454 row N2 replaced, on a path that takes Shell4, by
the Shell4 checks of that path repeated on triangles. The note adds **n1**: Shell3 is taken exactly where Shell4 is taken (the
five named paths), and **n2**: the Shell4 follow-ups of ADR 0432 (shells on the transformation path and body loads, ADR 0435;
shell results in the output, ADR 0454 PO1) have their triangle repeats.

The brief adds: grep for the current Shell3 refusals; for each remaining one register the Shell4 check repeated on triangles
(citing its ADR) and lift it with consistent arithmetic, or keep it refused with a registered reason (a known miss of a1); defaults
bit-identical; `baseline_hashes` unchanged; no case-file key; superseded refusal tests ignored and copied (ADR 0291 pattern);
mutations the checks catch; every aggregated test error finite.

## What was read (no run)

`AGENT_RULES.md`; the brief; ADR 0454 whole (D3, row N2 of its checks: `nonlinear::initial_states`, smoothed-aggregation CG,
`check_supported()`, a cyclic constraint on `solve_static_mpc` refused naming Shell3 and `shell3::FOLLOW_UP`); ADR 0432 "For the
lead" (its two Shell4 follow-ups); ADR 0435 (F-522) check table and `tests/si_shell_transformation.rs` (H1 to H3r, K1, K2, Z1 to
Z7, C1, C2); ADR 0473 (F-420) check table and addendum (P1 to P4, S1, S2, E1, E2 known miss, U2 to U4: Shell4 and Shell3);
ADR 0508 (F-558: S1 to S4 on both); ADR 0487 (F-573, follower pressure: K1, S1s, C2a on both); ADR 0490 (F-578, the Newton
driver: T1, S1, L1, RU3, CY3 on Shell3); ADR 0522 (F-585: arc length, `solve_from`, the nonlinear transient; A1 to A3, F1, F1b,
F1c, E1, EZ, SF1, R2 and their results) and `tests/sa_corot_shell_dyn.rs` (`roof_shell`, `roof_limit`, `a2`, `strip`,
`vibrate_corrected2`, `ez_clapeyron`, `f_eb`); ADR 0466 and ADR 0483 (the Shell3 membranes, the twisted-strip diagnosis); the
sources at f69eb128: `model.rs` (`check_elements` and its callers: the `shells` flag refuses Shell4 and Shell3 together, D3 of
ADR 0454), `solve.rs` (the multigrid refusal, Beam2, Shell4 and Shell3 together), `mpc.rs` (`check_transformation_model`: the
cyclic refusal, Beam2, Shell4, Shell3 in that order), `nonlinear.rs` (`initial_states`, `is_shell`, `solve_in_time_with`),
`nl_transient.rs` (`Integrator::new`, `strain_energy`), `arclength.rs`, `follower.rs` (`buckling_factors`, `tangent_modes`),
`buckling.rs`, `geometric.rs`, `geometric_struct.rs`, `rotating.rs`, `cms.rs`, `cyclic.rs` (`check_cyclic_model`), `transient.rs`,
`damped.rs`, `harmonic.rs`, `harmonic_direct.rs`, `mass.rs`, `assemble.rs`, `shell3.rs` (`local_stiffness_section`, `local_mass`,
`transformation`), `shell_corot.rs`; `tests/sa_shell3.rs` (`s1_n2_n3_refusals_after_f578`, the current N2 rows).

## D0. The audit (what the grep and the reading give)

There is **no path that takes Shell4 and refuses Shell3** at f69eb128. Every refusal of Shell3 sits beside the refusal of Shell4
with the same condition: `SolidModel::check_elements` (the `shells` flag), `solve.rs` (multigrid), `mpc.rs` (cyclic on the
transformation). Every dispatch that takes Shell4 takes Shell3 (`matches!(.., Shell4 | Shell3)` in `nonlinear.rs`,
`nl_transient.rs`, `arclength.rs`, `follower.rs`, `geometric_struct.rs`, `rotating.rs`, `mass.rs`, `assemble.rs`, `thermal.rs`,
`tie.rs`, `loads.rs`, `shell_corot.rs`). The rows of N2 and the five paths of the note:

| N2 row / path | Shell4 at f69eb128 | Shell3 at f69eb128 | Shell4 checks (ADR) | triangle repeats existing | gap registered here |
|---|---|---|---|---|---|
| `nonlinear::initial_states`, Newton driver | taken (ADR 0490) | taken | T1, S1, L1, RU4, CY4 (0490) | T1, S1, L1, RU3, CY3 (0490) | none |
| arc length (`arclength::trace`) | taken (ADR 0522) | taken | A1 (missed), A2 fast on Shell4 8 x 8 (0522) | A3 record (Shell3 16 x 16, no limit) | **AL1, AL2** |
| `solve_from` | taken (0522) | taken | SF1 | SF1 | none |
| nonlinear transient | taken (0522) | taken | F1, F1b, F1c, E1, EZ over 100 periods (0522) | F1, F1b, E1, EZ over 4 periods; no F1c, no 100 periods | **NT1, NT2** |
| buckling, pre-stressed modes, rotating (linear) | taken (0473, 0508) | taken | P1, P2, S1, E1, U2 to U4 (0473); S1, S3 (0508) | P3, P4, S2, E2 (known miss, diagnosed in 0483), U2 to U4 (0473); S2, S4 (0508) | none |
| follower pressure, its buckling factors | taken (0487) | taken | K1, S1s, C2a, B1s (0487) | the same on Shell3 (0487) | none |
| transformation path (0432 follow-up, ADR 0435) | taken | taken (N2 "nodal spring taken") | H1, H1b, H2, H2m, H3, H3r, K1, K2, Z1 to Z7 (0435) | none (only a nodal spring solved in 0454 N2) | **TR1 to TR6** |
| body loads (0432 follow-up, ADR 0435) | taken | taken | G1 to G5, C1, C2 (0435) | LD1 (gravity, centrifugal, nodal body force), LD3, M1 (0454) | none (C1, the spinning strip "exact element", is not exact for the triangle cut: not repeated) |
| shell output (0432 follow-up) | taken | taken | (F-512 entry) | PO1 (0454) | none |
| **multigrid** (smoothed-aggregation CG) | **refused** (`shell::FOLLOW_UP`) | refused | none (no Shell4 check exists) | | **kept refused (KM1)** |
| **cyclic** on `solve_static_mpc`, `modal_analysis_mpc`; cyclic harmonic paths | **refused** | refused | none | | **kept refused (KM1)** |
| **`check_supported()`** (the other paths: linear transient, damped modes, modal harmonic, direct harmonic, `solve_in_time`, `tangent_modes`) | **refused** | refused | none | | **kept refused (KM1)** |
| energy-momentum step | refused (`InvalidValue { key: "nl_transient" }`, ADR 0522 R2) | refused | none | | kept refused (KM1; F-611's area) |

So of the four N2 rows one (`initial_states`) was replaced by ADR 0490 and ADR 0522 with its triangle repeats, and three
(multigrid, `check_supported()`, cyclic) are refusals of Shell4 too. The note's rule "take Shell3 when they take Shell4" keeps
them: lifting Shell3 there alone would put triangles on a path that has no shell check at all (the multigrid's near-null space
is that of three-unknown nodes, ADR 0417 D4; the cyclic rows rotate translations only, ADR 0426; the linear dynamic paths have no
shell check). Lifting them for Shell4 is not this item (the Shell4 follow-ups); D3 proposes it. Those three rows are the
**known miss KM1** of clause a1, registered now.

## Design

### D1. No library code

F-541 changes no file under `crates/*/src`: every path that takes Shell4 already dispatches Shell3 through the same arithmetic
(D0). The item adds tests only: the triangle repeats of the gaps of D0 (D2), the consistency check of the kept refusals (RC1,
RC2) and the record of KM1. Defaults are therefore bit-identical by construction; N1 checks it (`git diff f69eb128 --
crates/*/src` empty) and N2 runs `baseline_hashes`. No default-bits record is written (there is no library change to guard; a
digest test of unchanged code proves nothing beyond N1). No case-file key, no golden.

### D2. The triangle repeats

Every set-up is the Shell4 set-up of the cited check with each quadrilateral `(a, b, c, d)` cut into the two Shell3 `(a, b, c)`
and `(a, c, d)` (the cut of ADR 0490 RU3 and of `sa_corot_shell_dyn::grid_conn`), the same thickness, material, sets, loads and
supports; the limit is the Shell4 limit of the cited check unless stated, with its estimate for triangles. File:
`crates/rcfd-fem/tests/sa_shell3_paths.rs` (new; helpers copied from `si_shell_transformation.rs` and `sa_corot_shell_dyn.rs`,
those files not edited).

- **TR (ADR 0435 H1 to Z7, the transformation path):** the plate `[0, 1]^2`, 4 x 4 cells cut (32 Shell3), `t = 0.05`, `nu =
  0.3`, `E = 2e11`, `rho = 7850`, `Load::shell_pressure("all", 1000)`; the strip `[0, 1] x [0, 0.2]`, 4 x 1 cells cut (8 Shell3),
  `t = 0.05`, `nu = 0`; the mixed model with the Hex8 block of ADR 0435 H2. Estimate: H1, H1b, H2, H3 are exact identities of
  the constraint algebra for any element (a rigid set on the clamped-edge nodes with the corner clamped is the clamp; the tip
  rigid set moves its nodes rigidly), rounding `~1e-12`. K1 is a rigid rotation of the strip (zero strain energy for any element
  with exact rigid motions; the Shell3 rigid motions are exact, ADR 0454 FR1, SE1). K2 is the constant-moment strip: the MITC3
  bending of Shell3 passes the bending patch test (ADR 0454 PT3, `8.4e-15`): with `nu = 0` the constant curvature `kappa = M_tot
  /(E I)` and `w = -kappa x^2 / 2` are reproduced at the nodes (the tied edge shear `(w_j - w_i)/l + mean theta` vanishes for a
  quadratic `w` and a linear `theta`), the nodal moments `M` on the two tip nodes are the consistent load of the edge moment, so
  K2 is exact to rounding. Z1 to Z6 are counts of the constraint geometry (the same as Shell4); Z7 rounding.
- **NT1 (ADR 0522 F1, F1b, F1c, E1, EZ, 100 periods):** the strip of ADR 0522 D6 (`L = 12`, `b = 1`, `t = 0.1`, `E = 1.2e6`,
  `nu = 0`, `rho = 1`, 16 cells cut: 32 Shell3) by `vibrate_corrected2` (amendment 4 of ADR 0522: modal `tolerance 1e-8`,
  `max_iterations 2000`, the start velocity `omega a phi / phi_z,tip`, `a = 1e-3 L`, 40 steps per period, the trapezoidal default)
  over 100 periods; EZ by `ez_clapeyron` (tip force `-1.042e-3`). Limits of ADR 0522: F1 `1e-2`, F1b `1e-3`, F1c `1e-2`, E1
  `1e-5`, EZ `1e-4`. Estimate: the Shell3 4-period run gave F1 `2.05e-3`, F1b `2.53e-6`, E1 `2.77e-8`, EZ `5.14e-7` (ADR 0522
  addendum, disclosed: known before this registration); the Shell4 100-period run E1 `2.8e-8`; F1c: `f_1` of Shell3 against
  Euler-Bernoulli, `|.| <= 4e-3` expected (the Shell3 cantilever static tip of ADR 0454 CT1 within `1.4e-4`; the beam-theory
  frequency of 16 linear cells, the frequency error `~ (beta h)^2 / 240`, `beta h = 1.875 / 16`, `6e-5`, plus the plate effect of
  `nu = 0`: none). Heavy (4000 steps, the Shell4 run took 63 s): an ignored record, run once.
- **NT2 (the fast companion):** F1c on the Shell3 strip (the modal solve of `vibrate_corrected2`'s options only, no transient),
  limit `1e-2`, estimate as NT1. Runs on every crate run.
- **AL1, AL2 (ADR 0522 A2 and the convergence of A1c/A3):** the quarter roof of ADR 0522 D5 (`R = 2540`, half-length `254`,
  `theta_0 = 0.1`, `t = 12.7`, `E = 3102.75`, `nu = 0.3`, hinged edge, symmetry edges, apex load `250` per unit factor) of Shell3
  `8 x 8` cells cut (128 Shell3), traced by `roof_limit` (the options of `a2_fast_roof_shell4_and_hex20`). AL1: A2 holds (stopped
  by the rule past the limit, a negative pivot after it, `P_lim` finite). AL2: `|P_lim(Shell3 8)/P_lim(Shell4 8) - 1| <= 1e-2`,
  the Shell4 `8 x 8` traced in the same test. Estimate: known before (disclosed) Shell4 `8` `2223.6` N, Shell4 `16` `2222.4` N,
  Shell3 `16` `2221.3` N (`5.0e-4` below Shell4 `16`); Shell3 `8` not run before; linear triangles at half the resolution: the
  difference at `16` times `4`, `<= 2e-3`. Limit `1e-2`.

### D3. The kept refusals (KM1) and their consistency (RC1, RC2)

RC1 (n1, "take Shell3 when they take Shell4"): for each entry path below a Shell4 model and its triangle twin (the strip of TR cut
or not, with root held; for the cyclic rows the model of ADR 0454 N2: a Hex8 box with a separate Shell3, or Shell4, element,
`Constraint::cyclic("ymin", "ymax", z, 0, 4)`) give the same outcome: both `Ok`, or both `Err` where, if the Shell4 error is
`NotYetSupported { item, variant, .. }` naming `shell::FOLLOW_UP`, the Shell3 error is `NotYetSupported` with the same `item`,
the variant `"Shell3"` where Shell4's is `"Shell4"` (else the same variant) and the package `shell3::FOLLOW_UP`; otherwise the two
errors have the same `FemError` variant and the same `key`. Paths: `solve_static` with the smoothed-aggregation CG;
`mpc::solve_static_mpc` and `mpc::modal_analysis_mpc` with the cyclic constraint; `cyclic::modal_analysis_harmonic` (harmonic
`0`); `SolidModel::check_supported`; `transient::Integrator::new`; `damped::damped_modes`; `harmonic_direct::DirectHarmonic::new`;
`nonlinear::solve_in_time`; `nl_transient::Integrator::new` with the energy-momentum step; `follower::tangent_modes` at `u = 0`;
and as positive rows `nonlinear::initial_states`, `solve_static` (default) and `modal_analysis` (`buckling::factors` is not
run: it needs a pre-stress; ADR 0473 covers it). RC1 is written as consistency, not as refusal, so a later item that lifts a path
for both kinds keeps it.

RC2 (the record of N2): the three N2 rows multigrid, cyclic (`solve_static_mpc`) and `check_supported()` are refused for Shell3
naming `shell3::FOLLOW_UP` and for Shell4 naming `shell::FOLLOW_UP` (exact). The test `s1_n2_n3_refusals_after_f578` of
`sa_shell3.rs` already asserts the Shell3 half and is not superseded (no row changes): no ignore-and-copy is needed.

KM1 (a1, predicted miss): an ignored test `n2_rows_taken_known_miss` asserts that the Shell3 cantilever of ADR 0454 solves by
the smoothed-aggregation CG, passes `check_supported()` and solves with the cyclic constraint; it fails by design (D0); run once
with `--ignored` to record the three refusal texts.

Proposed follow-ups (not items added here; for the lead): shells (Shell4 and Shell3) in the cyclic constraint (the rotation
unknowns rotated as the translations), in the smoothed aggregation (six-unknown near-null space), and on the linear dynamic paths
(linear transient, damped modes, modal and direct harmonic), each with its Shell4 checks and their triangle repeats.

## Checks (limits fixed now)

Every aggregated error is formed with a NaN-propagating maximum and checked finite: a NaN fails its check (`value <= limit` is
false for NaN, and each `holds` on a measured value also requires `is_finite`).

| Id | Clause | What | Limit | Estimate |
|---|---|---|---|---|
| TR1 | a1, n2 | ADR 0435 H1 and H1b on the Shell3 plate: translations and rotations against the clamped plate, relative to the largest of each field | `<= 1e-10` each | rounding |
| TR2 | a1, n2 | H2 (the plate joined by a rigid set to the fixed Hex8 block) against the clamped mixed model; H2m the four lowest frequencies, relative | `<= 1e-10` each; `<= 1e-8` each | rounding |
| TR3 | a1, n2 | H3 (tip rigid set, reference load): `u_a - t - w x d_a` relative to the largest tip translation; `theta_a - w` relative to `|w|`; H3r root reaction force and moment balance | `<= 1e-12`; `<= 1e-12`; `<= 1e-10` each | rounding |
| TR4 | a1, n2 | K1 on the Shell3 strip: `|k theta - M|/M` and `|S + M|/M` at each root node; every node `theta_y` and `u_z = -theta x`, relative | `<= 1e-10` each | rounding |
| TR5 | a1, n2 | K2: `|k theta_root - M|/M`; tip `theta_y` and `u_z` against `theta_root + M_tot L/(E I)` and `-(theta_root L + M_tot L^2/(2 E I))` | `<= 1e-10` each | D2: exact (patch test) |
| TR6 | a1, n2 | Z1 to Z6 the free-rigid-motion counts of ADR 0435 (6, 1, 0, 1, 6, 0, 0) on the Shell3 strip, plate, H1 and H2 models; Z7 `K X` of the free fields | exact; `<= 1e-12` | counts geometric; rounding |
| NT1 | a1 | Shell3 strip 100 periods (heavy record): F1, F1b, F1c, E1, EZ | `1e-2`, `1e-3`, `1e-2`, `1e-5`, `1e-4` | `2e-3`, `3e-6`, `<= 4e-3`, `<= 1e-7`, `<= 1e-6` |
| NT2 | a1 | F1c on the Shell3 strip (fast) | `<= 1e-2` | `<= 4e-3` |
| AL1 | a1 | A2 on the Shell3 roof `8 x 8 x 2` | holds | |
| AL2 | a1 | `|P_lim(Shell3 8)/P_lim(Shell4 8) - 1|` | `<= 1e-2` | `<= 2e-3` |
| RC1 | n1 | the consistency of D3 on each listed path | holds each | D0 |
| RC2 | a1 (record) | the three N2 rows refused for both kinds with their packages | exact | D0 |
| KM1 | a1 | `n2_rows_taken_known_miss` (ignored): the three N2 rows taken for Shell3 | predicted miss | D0 |
| N1 | defaults | `git diff f69eb128 -- crates/*/src` empty | exact | D1 |
| N2 | defaults | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | D1 |
| B1 | | `cargo test -p rcfd-fem --release` (every existing test unedited) | pass | |

## Negative controls

- **NC1** (TR1 sees a wrong hold): the Shell3 plate with the rigid set on the edge but without the corner clamp is singular or has
  six free rigid motions (Z5 = 6), so TR1's equality is not a tautology of the clamp.
- **NC2** (TR5 sees the bending stiffness): the K2 tip rotation with `E I` of `t = 0.06` instead of `0.05` in the closed form
  differs by `> 0.3` relative (`1 - (5/6)^3 = 0.42`).
- **NC3** (AL2 sees the mesh): recorded only, `P_lim(Shell3 8)` and the known `P_lim(Shell3 16) = 2221.3` N differ by less than
  AL2's limit (the check is a tolerance, not a discriminator; disclosed).
- **NC4** (RC1 is not vacuous): RC1 contains both outcomes: at least one row `Ok` for both kinds and at least one `Err` for both.

## Mutations (on a scratch copy, own `CARGO_TARGET_DIR`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | `shell3::local_stiffness_section`: the bending term weight `a` times `1.05` | TR5, NT2 |
| MU2 | `shell3::local_mass`: `mt` times `1.1` (translational mass) | NT2 |
| MU3 | `shell3::transformation`: the rotation block left as the identity (rotations not turned to global axes) | TR4 or TR5 or TR6 (Z7), TR1 |
| MU4 | `mass.rs`: the Shell3 dispatch removed (Shell3 to the placeholder) | NT2 (refused), TR2 (H2m) |
| MU5 | `shell_corot.rs`: the Shell3 Green membrane flag `green` set to `false` | AL1 or AL2 (uncertain: the corotated frame carries most of the roof's nonlinearity) |
| MU6 | `nonlinear.rs` `is_shell`: Shell3 dropped (Shell3 to the solid evaluation) | AL1 (the roof trace refused or failed); NT1 |
| MU7 | `model.rs` `check_elements`: the Shell3 refusal removed (`check_supported` takes Shell3) | RC1, RC2 |
| MU8 | `solve.rs`: Shell3 dropped from the multigrid refusal | RC1, RC2 |
| MU9 | `mpc.rs` `check_transformation_model`: the Shell3 cyclic row removed | RC1, RC2 |

MU5 is registered with its doubt; an uncaught mutation is recorded, not answered by a new check after the run.

## Decision rule

- **done:** every registered check met (TR1 to TR6, NT1, NT2, AL1, AL2, RC1, RC2, N1, N2, B1) **and** KM1 not a miss (every N2
  row replaced). By D0 KM1 is a predicted miss, so done is not reachable by this item.
- **partial:** the checks of the paths that take Shell4 (TR, NT, AL) met or missed with a diagnosed cause, RC1, RC2, N1, N2, B1
  met, KM1 missed as predicted (the three N2 rows that are Shell4 refusals too).
- **todo:** RC1 or N1 or N2 or B1 missed, or most of TR, NT, AL missed without a diagnosis.

## Limits (what is not covered)

- No path is lifted: the three remaining N2 rows wait for Shell4 on those paths (D3 follow-ups).
- The spinning strip C1 of ADR 0435 is not repeated (not exact for triangles; LD1 of ADR 0454 covers the centrifugal load
  vector).
- The roof against a published reference is the separate follow-up of ADR 0522 (Sze, Liu and Lo); AL2 is a Shell3-to-Shell4
  agreement only.
- The energy-momentum step is in F-611's area: RC1 only observes it.
