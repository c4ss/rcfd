# ADR 0117: Wave 45C, the ADR 0113 seam-coefficient fix F1 (`coupled_offdiag_in_h1`) on the `rotating_frame` FAN-01 path

Status: **ACCEPTED 2026-10-26** (limits registered before the registered runs; the user accepted the wave-45 draft decisions: a bool key,
known-miss FAN-01 tests kept compiling through a test-only route, no default-flip draft before the combined wave-45 result). Written at
main ea649d1 on branch `w45c`, target `<target dir>`. House rules of ADRs 0110-0113: limits fixed before runs; a missed
limit becomes an ignored `*_known_miss` test with the limit intact; a defective set-up is corrected by an amendment before the corrected
run; defaults are bit-identical; `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded.

**The default stays off whatever the result.** Making F1 the default for periodic SIMPLEC would move baselines; that is a separate user
decision with an ADR 0114-style scan, and by the user's decision it is not drafted before the combined wave-45 result is known.
Clean-room: first principles and our own ADRs. Parents: ADRs 0008, 0086, 0110, 0113.

## Context

* ADR 0113 (finding outside its task): the default SIMPLEC kernel builds `rAtU = 1 / (a_p - h1)` of a cyclic cell without the lagged
  partner coefficients. A cyclic face keeps them in the source, not in the off-diagonals.
  * Provenance of the 4.6e-3 U: an inertial 3-D bent-box sector in through-flow at library level (ADR 0113 K3i). F1 reduces it to
    2.2e-12 U against the full annulus. It is not a FAN-01 number; the FAN-01 effect has never been measured.
  * In the 2-D swirl checks without a body force the converged solution did not depend on it (inertial pairs to 1e-9). With a body force
    the seam-wall corner cells moved (K1: 1.4e-4 without F1, 3.6e-9 with it).
* The FAN-01 path is `rotating_frame` (relative velocity, explicit Coriolis and centrifugal force) on the cyclic `seam_lo` / `seam_hi`
  sector, so it has both ingredients: cyclic cells and a body force on them.
* ADR 0110 part L: on the empty passage (exact absolute swirl 0) the outlet angular momentum L grows with refinement and is undiagnosed:
  E3 / E2 / E1 = -0.009518 / 0.059612 / 0.105977 N m (L1, 600 iterations). Falsified so far: the limited rigid torque (L1) and the seam
  gradient (L2); SST (L3) and freezing (L5) were not evaluable. The seam `rAtU` defect has not been tested as a candidate.
* Code trap (checked against the code at ea649d1): `crates/rcfd-api/src/sim.rs` line 2667, inside `Simulation::step()`, sets
  `s.coupled_offdiag_in_h1 = sources.cyclic_seams` on every step. `Sources::cyclic_seams` is true only for an MRF zone with
  `cyclic_seams = true` (`sources.rs` 611). So a `simple_solver_mut()` switch on a `rotating_frame` case is overwritten to false at the
  next step, and F1 cannot reach the FAN-01 path without an API change. ADR 0113 says the API "sets [F1] when an MRF source has
  `cyclic_seams = true`"; it does not say that the same line also clears any externally set flag.
* `rotating_frame` also switches on `wall_pressure_trapezoid` and with it `coupled_body_pair_mean` (`sources.rs` 605-610, `sim.rs`
  2664-2666). ADR 0113 ran F1 only with the pair mean off. F1 with the pair mean on is untested.

## What I already knew when registering (rules c, d; disclosed)

* The code of D1 (schema key, the `sim.rs` OR and its validation) and every test of this ADR were written before registration, so that
  the rule-d smoke could run; they are committed after this ADR, unchanged except as stated in amendments. Nothing registered was run
  before registration. The smoke values below are the only solver output I had seen.
* Draft claims re-checked against the code: `sim.rs` 2667 (above); `simple.rs` 863-872 hold the F1 code; `arm()` (fan01_stall_w43c.rs
  418) takes `solver_extra`, while `family()` (718) and `start_arm()` (684) pass `""`; the 44C K3 meshes and the 4.6e-3 / 2.2e-12 / 1.8e-11
  U values are as ADR 0113 reports them.
* On main the only case-tree generator that produces a rotationally cyclic sector is `axial_fan_sector`. Its hub and shroud faces are not
  surfaces of revolution, so a tree-level sector-against-annulus check (the exactness tier through the case tree) is not possible on main.
  It would be better served by the ADR 0116 generator (`annulus_3d_sector`, built in parallel by 45B, not used here): a tree-level R1 on
  that sector is a follow-up for the lead to decide. R1 here is at library level.

## Decisions

1. **D1, case key** `solver.cyclic_partner_in_h1` (bool, default false). `sim.rs` 2667 becomes `sources.cyclic_seams ||
   get_b(case, "solver.cyclic_partner_in_h1")`. Valid with steady `simple` / `simplec` (`Engine::Steady`); a located error at
   `solver.cyclic_partner_in_h1` for any other algorithm. A no-op on meshes without coupled faces. Because of the OR, `false` does not
   switch F1 off for an MRF zone with `cyclic_seams = true`; F1 acts on every coupled face, translational cyclic pairs included.
2. **D2** No change to `simple.rs`, the limiter, the mesh or the sources.

## Registered checks

### R, library equivalence with a `rotating_frame` body force and the pair mean on (`crates/rcfd-fv/tests/rotating_seam_h1_w45c.rs`)

* Geometry: the 44C K3 meshes. The bent-box sector `box_mesh([8, 8, 12], [1, 1, 1.2])`, r = 0.5 + 0.5x, theta = (pi/3) y, cyclic
  ymin/ymax, against `annulus_3d(48, 8, 12, 0.5, 1, 1.2)`, cells matched by centre to 1e-9.
* Set-up: initial and inlet (zmin) `u = (0, 0, 1)`, outlet zmax zero gradient with p = 0, inner/outer walls at rest in the rotating
  frame (u = 0), nu = 0.1. `RotatingFrame` Omega = 5 about +z through the origin, centrifugal on, zone all cells. Wiring as `sim.rs`
  does it in every step: the acceleration evaluated on the current velocity before each iteration, `wall_pressure_trapezoid` on,
  `coupled_body_pair_mean` equal to it (on), `component_diag_correction` off (no frame flux on this path), F1 per arm. SIMPLEC as the
  ADR 0113 C2b template (alpha_u 0.8, alpha_p 1, central convection), converged when `res_u < 1e-10` and continuity `< 1e-9` within
  4000 iterations.
* **N1, negative control, run first** (`n1_rotating_sector_without_f1_differs_from_the_annulus`): the sector without F1 against the
  annulus. It must miss the exactness tier by at least 10x (`>= 1e-7 U`, asserted). Whether it also misses the L_DIFF tier by 10x
  (`>= 4.3e-3 U`) is reported; if not, the L_DIFF tier is non-discriminating and reported only.
* **R1 / R1-exact** (`r1_rotating_sector_with_f1_equals_the_annulus`), limits:
  * both runs converged by the criterion above (a run that is not converged is a miss);
  * sector with F1 against annulus `<= 1e-8 U` (R1-exact, the ADR 0113 F1-falsifier level; 44C K3 / K3c with F1 measured 1.8e-11 U);
  * seam balance `|sum phi(ymin) + sum phi(ymax)| / A_seam <= 1e-8 U` (44C).
  * The L_DIFF tier (`<= 4.3e-4 U`, ADR 0008 M3) is printed. It is implied by R1-exact, so it cannot change the decision: if R1-exact
    passes, it passes; if R1-exact misses, the key is removed anyway.
  * Reported, no limit: F1 with the pair mean off against the annulus, and its seam balance.

### F, the FAN-01 empty-passage family (`fan01_stall_w43c.rs`, ADR 0117 block, `w45c_f_family_key_against_default`)

* E3, E2, E1 (`camber_seam_h`, the ADR 0086 case unchanged, BJ-limited `linear_upwind`, SST), 600 iterations from the start, through
  `arm()`. Two arms per member: key absent, and `cyclic_partner_in_h1 = true` in the case text (so `step()` keeps it).
* Measured: outlet L and swirl energy E_s (`outlet_stats`), their change over the last 100 iterations, `rho_med` of the key arm against
  the default arm (the larger ratio of the medians of res_u and res_p over the last 300 iterations), and the top-10 cells of
  `|u_key - u_def|` with their patch tags.
* **Harness check (a limit; registered so that a broken copy cannot pass):** the default arm reproduces the ADR 0110 L1 values to the
  printed digits, `|L_def - L_L1| <= 5e-7 N m` with L_L1 = -0.009518 / 0.059612 / 0.105977. A miss makes the F family a defective set-up
  (amendment before any rerun), not an F1 result.
* Evaluability (ADR 0110): an arm whose L changes by more than 1e-3 N m over its last 100 iterations is NOT EVALUABLE.
* **(F-a) not worse, a limit:** for each member whose default arm is evaluable: the key arm is evaluable, `|L_key| <= |L_def| +
  max(0.005 |L_def|, 1e-4 N m)` and `E_s,key <= E_s,def + max(0.005 E_s,def, 0.01 Pa)`. A member whose default arm is not evaluable is
  dropped; a default arm evaluable with a key arm not evaluable is a miss (F1 made it worse); no evaluable member means F-a is not passed.
  Reason: F1 is a consistency fix (a seam cell gets the `rAtU` of the same cell of the unsplit mesh), so it may not add spurious swirl.
  The band is the ADR 0086 cap-rule tolerance used in ADRs 0110 and 0111 for "no change".
* **(F-b) effect, a classification (not acceptance)**, only if every arm of the six is evaluable (otherwise NOT EVALUABLE):
  * **EXPLAINS THE GROWTH** iff with the key `|L|` decreases strictly E3 -> E2 -> E1 and the observed order on `|L|` (ADR 0086
    fixed-point form, f1 = E1 fine, f3 = E3 coarse, r21 1.4073, r32 1.4017) is at least 0.5;
  * otherwise **CONTRIBUTES** iff `|L_key - L_def| > max(0.005 |L_def|, 1e-4 N m)` on at least one member;
  * otherwise **NO EFFECT**.
  * Prediction, written now: CONTRIBUTES at the seam-wall corner cells, not EXPLAINS THE GROWTH. ADR 0110 places the E2 / E1 cycle at
    the hub cells of the C-grid nose region, not on the seams.

### B, the bladed passage (reported, no limit; `w45c_b3_key_from_the_start`)

* The ADR 0110 set-up arm: the reference (1000 iterations, window 500-1000) against the key from the start (3000, window 2500-3000).
  Reported: `rho_med`, `rho_du` with the ADR 0110 verdict wording, torque and Delta p_ts against the reference (stalled 1.849 N m /
  46.6 Pa). No limit: these are values taken on a cycle (ADR 0110 amendment 5).

### D, defaults (non-ignored tests)

* **D-a** (`w45c_da_key_false_is_bit_identical_and_the_key_reaches_step`): on B3 the key absent and `false` give `to_bits`-identical u,
  p, phi after 30 iterations; on E3 the solver flag after step 2 is false without the key and true with it. Identity to HEAD itself
  rests on `x || false == x` and on `baseline_hashes.rs` (a test cannot run HEAD).
* **D-b** (`cyclic_partner_h1_w45c.rs`): the key `true` on the ADR 0008 M2 annulus (no cyclic faces, the 44C A1 case), 20 iterations,
  is bit-identical to the key absent.
* **D-c:** `baseline_hashes.rs` and `mrf_cyclic_seams_w44c.rs` (both crates) pass unchanged.
* **D-d:** the key with `pimple` and with `coupled` is a located error at `solver.cyclic_partner_in_h1`.

### Status decision rule

* The key is kept opt-in and documented iff N1 discriminates at the exactness tier, R1 (convergence, R1-exact, seam balance), the F
  harness check, F-a and D-a to D-d pass. F-b and B are readings, not acceptance.
* If R1 or F-a misses, the key is removed (rule b); `coupled_offdiag_in_h1` itself stays (ADR 0113's). R1 is at library level, so its
  known-miss test still compiles. The F-family and B3 tests then stay as ignored `*_known_miss` tests that set the flag through a
  test-only route registered now: a `#[doc(hidden)] pub` bool on `Simulation` (default false, no case key) ORed into `sim.rs` 2667. It is
  added only if the key is removed.
* Whatever happens: no default changes, F-126 stays partial, and ADR 0110 part L stays open unless F-b reads EXPLAINS THE GROWTH.

### Run order

N1 first, then R1; then the F family; then B3. Each registered test is run once, by its full name with `--exact`, in its own
invocation.

## Smoke results before registration (rule d)

Run on the uncommitted D1 code (release, `target-w45c`), before this ADR was committed:

* `smoke_w45c_rotating_sector_arms` (rcfd-fv): the annulus arm (no cyclic faces, so the same with and without F1) converges by the R
  criterion at iteration 82 (res_u 9.1e-11, continuity 4.7e-13), well inside the 4000 cap, so the centrifugal force against the uniform
  p = 0 outlet is not a feasibility problem. Sector arms, 30 iterations each: F1 off / pair mean on: finite, res_u 4.8e-5, continuity
  6.1e-9; F1 on / pair mean on: finite, 5.5e-5, 1.1e-8; F1 on / pair mean off: finite, 5.5e-5, 1.1e-8. No sector-annulus difference
  was computed.
* `w45c_smoke_key_on_the_fan_path` (rcfd-api): the solver flag after step 2 on E3 is **true with the key and false without** (the key
  reaches the kernel through `step()` on a mesh with cyclic faces; the first case-tree run of F1 on cyclic faces). E3 with the key, 40
  iterations: finite, no guard trip (res_u 1.2e-2). B3 with the key, 40 iterations: finite, no guard trip (res_u 9.5e-3).
* No set-up is defective by the smoke; no amendment is needed before the registered runs.

## Out of scope

Making F1 the default; the `rotating_frame` relative-velocity discretisation error (ADRs 0078, 0082); the limiter stall (0115); MRF on
the fan passage (0116); finer meshes than F1; agreement with FAN-01 data; pseudo-time or PIMPLE paths.

## Files

* `crates/rcfd-api/src/sim.rs` (line 2667; validation next to `simplec_boundary_compensation`), `crates/rcfd-api/src/schema.rs` (one
  bool solver key), `docs/case-schema.md`.
* `crates/rcfd-fv/tests/rotating_seam_h1_w45c.rs` (new: N1, R1, smoke).
* `crates/rcfd-api/tests/fan01_stall_w43c.rs` (an ADR 0117 block appended between delimiter lines: smoke, D-a, F family, B3; nothing
  above it edited).
* `crates/rcfd-api/tests/cyclic_partner_h1_w45c.rs` (new: D-b, D-d).

## References

None outside our own ADRs. Every number above is from ADRs 0008, 0086, 0110 and 0113. The observed-order formula is ADR 0086's (Celik et
al. 2008, recalled there, UNVERIFIED).

## Amendment 1 (written after N1, R1, the F family, B3, D-a and D-b had run on the key code, before the corrected D-d runs)

What I knew when writing this: N1 discriminates (3.0e-2 U), R1 passes (6.6e-11 U, seam balance 0), F-a **missed** on E3 and E2 (the
values are in the addendum), so by the status decision rule the key is to be removed; B3 reads FALSIFIED; D-b passed.

* D-d as registered failed before reaching the check: its `cavity_2d` case text has no boundary conditions, so `Simulation::new`
  refuses it at `boundaries.lid` / `boundaries.walls` and never reaches the algorithm check. That is a defective set-up, not a miss of
  the validation. The test stays, ignored, as `dd_key_outside_the_steady_solver_is_a_located_error_known_miss` with its criterion.
* **D-d' (corrected, registered now):** the same check with the cavity's `lid` (fixed, (1, 0, 0)) and `walls` (fixed) conditions; the
  criterion is unchanged (a diagnostic at `solver.cyclic_partner_in_h1` for `pimple` and for `coupled`). It runs once on the key code
  (the code it was registered for), before the removal.
* D-a had not run yet (cargo stopped after the D-d binary failed); it runs together with D-d' on the key code.
* After the removal the key is unknown to the schema. D-a, D-b and D-d' are then kept through the test-only route where they still
  test something: D-a and D-b set the route instead of the key (route off bit-identical to the default; the route reaches `step()`; the
  route on a mesh without cyclic faces bit-identical), and D-d' becomes "the removed key is refused" (a diagnostic at
  `solver.cyclic_partner_in_h1` for a steady case too). These post-removal versions are checks of the route, not of the registered key.

## Amendment 2 (written after D-a and D-d' passed on the key code, before the removal and any run of the route)

* The registered test-only route was "a `#[doc(hidden)] pub` bool on `Simulation`". That cannot reach the F-family and B3 tests without
  editing `arm()` above the ADR 0117 block: `arm()` builds its own `Simulation`, and the only hook it offers is the `switch` closure on
  the `SimpleSolver`. The route is therefore a `#[doc(hidden)] pub` bool on `SimpleSolver`, `cyclic_partner_in_h1_test_only` (default
  false, read nowhere in rcfd-fv), ORed into `sim.rs` 2667 next to `cyclic_seams`. The tests set it through the switch before iteration
  1, so `step()` keeps F1 from the first iteration, as the key did. This is the one change to `simple.rs` (D2 said none): a struct field
  and its initialiser, no kernel line.
* Route check (registered now, reported in the addendum): the F family, rerun once through the route, reproduces the key-arm values of
  the registered run (L and E_s to the printed digits). Not a new acceptance; it shows the known-miss tests still measure what failed.

## Addendum: measured values (one registered run each; release, `target-w45c`)

| check | measured | limit | result |
| --- | --- | --- | --- |
| N1 sector without F1 - annulus (run first) | 3.04e-2 U (annulus converged at 82, sector at 89) | >= 1e-7 (exactness tier); >= 4.3e-3 reported | discriminates at both tiers |
| R1 converged (annulus / sector with F1) | iteration 82 / 90 | within 4000 | pass |
| R1-exact: sector with F1 - annulus | 6.64e-11 U | 1e-8 | pass |
| R1 L_DIFF tier (implied) | 6.64e-11 U | 4.3e-4 | pass |
| R1 seam balance | 0 (exactly) | 1e-8 | pass |
| R1 reported: F1 with the pair mean off - annulus; seam balance | 6.63e-11 U; 1.0e-14 | none | reported |
| F harness: default L E3 / E2 / E1 | -0.009518 / 0.059612 / 0.105977 N m | the ADR 0110 L1 values to 5e-7 | pass |
| F-a E3: abs(L_key); E_s,key | 0.010108 against 0.009618; 5.97814 against 6.29765 Pa | not worse | **MISS** (abs(L)) |
| F-a E2: abs(L_key); E_s,key | 0.060824 against 0.059910; 9.26279 against 9.27651 Pa | not worse | **MISS** (abs(L)) |
| F-a E1: abs(L_key); E_s,key | 0.106131 against 0.106507; 5.43131 against 5.51119 Pa | not worse | pass |
| F evaluability (last-100 change of L, default / key) | E3 9.0e-8 / 4.8e-8, E2 4.0e-6 / 2.5e-9, E1 8.2e-8 / 6.3e-7 N m | <= 1e-3 | all evaluable |
| F-b | key abs(L) 0.010108 / 0.060824 / 0.106131 N m (still rising; order 0.367); change 5.9e-4 / 1.21e-3 / 1.5e-4 N m against the bands 1e-4 / 3.0e-4 / 5.3e-4 | classification | **CONTRIBUTES** (E3, E2), not EXPLAINS THE GROWTH |
| F record: `rho_med` key against default | 1.04 / 0.37 / 1.02 | none | reported |
| F record: top cells of abs(u_key - u_def) | E3: 0.28 m/s, hub-radius cells at the outlet (outlet+seam, outlet) and interior; E2: 2.3 m/s, seam cells at r 0.245 (near the shroud), z 0.10, non-orthogonality 73 deg; E1: 0.54 m/s, hub cells at z 0.094-0.099 | none | reported |
| B3 key from the start (reported) | `rho_med` 0.656, `rho_du` 1.000: FALSIFIED; torque 1.8468 against 1.8493 N m, Delta p_ts 46.23 against 46.64 Pa | none | reported |
| D-a (on the key code): absent = false on B3 after 30 iterations; flag after step 2 on E3 | bit-identical; false without, true with the key | bit-identical; false / true | pass |
| D-b (on the key code): key true on the M2 annulus | bit-identical | bit-identical | pass |
| D-d as registered | refused at `boundaries.lid` / `boundaries.walls` before the key check | located at the key | defective set-up (amendment 1), ignored known miss |
| D-d' (on the key code): `pimple`, `coupled` | located error at `solver.cyclic_partner_in_h1` | located | pass |
| D-c: `baseline_hashes.rs`, `mrf_cyclic_seams_w44c.rs` (both crates) | pass unchanged (final gate) | pass | pass |
| Route check (amendment 2): F family rerun through the route | every printed value equal to the key run | reproduces | pass |

Verdict by the status decision rule: R1, N1, the harness and D pass, **F-a misses on E3 and E2**, so **the key is removed** (rule b).
What remains in the code: the `sim.rs` line 2667 ORs `SimpleSolver::cyclic_partner_in_h1_test_only` (doc-hidden, default false, no case
key) into the flag, as registered for this outcome (amendment 2 moved it from `Simulation` to `SimpleSolver`); `schema.rs` and
`docs/case-schema.md` are back to their main state, so the key named in D1 does not exist. Defaults are bit-identical
(`x || false == x`; `baseline_hashes.rs` passes). The F-family test is an ignored `*_known_miss` with its limits; the B3 and smoke tests run
through the route; D-a, D-b and D-d' now check the route and the refusal of the removed key (amendment 1).

What this means:
* F1 itself is right on the `rotating_frame` path with the pair mean on: a true sector reproduces the full annulus to 6.6e-11 U, against
  3.0e-2 U without it (larger than the 4.6e-3 U of the inertial K3i: the body force raises the seam defect). The pair mean has no
  measurable effect on that set-up (6.63e-11 against 6.64e-11).
* On FAN-01, F1 is not "not worse". It raises abs(L) on E3 by 6 % and on E2 by 2 %, lowers E_s on E3 and E1, and moves cells at the hub
  (E1, E3) and at the seam near the shroud (E2), not only seam-wall corner cells. The premise of F-a (a consistency fix may not add
  spurious swirl) does not hold here: the camber-seam passage is not an exact sector of an annulus mesh, so the default seam `rAtU` error
  and the other discretisation errors do not separate, and removing one can raise the total. My prediction was CONTRIBUTES at the
  seam-wall corners; it was CONTRIBUTES, but at the hub and the near-shroud seam cells.
* F1 does not explain the part L growth: with F1 abs(L) still grows E3 -> E2 -> E1 (0.0101 -> 0.0608 -> 0.1061 N m), and the change F1
  makes (at most 1.2e-3 N m) is about 1 % of the growth (0.096 N m). ADR 0110 part L stays open; the seam `rAtU` defect is now a tested
  candidate that contributes but does not cause it.
* B3: F1 does not touch the limiter stall (`rho_du` 1.000, FALSIFIED by the ADR 0110 wording); torque -0.13 %, Delta p_ts -0.9 %.
* No F1 default-flip draft follows (user decision: not before the combined wave-45 result; and F-a speaks against it on FAN-01).

Deviations (all in the amendments): D-d's set-up was defective (no boundary conditions) and was corrected by D-d'; the test-only route sits
on `SimpleSolver` instead of `Simulation`, which adds one field to `simple.rs` (D2 had said no `simple.rs` change); the F family ran a
second time, through the route, only to check the route. The D-a / D-b / D-d' tests in the tree are the post-removal versions; their
registered versions ran on the key code (the "ADR 0117 amendment 1" commit holds them).

Follow-ups for the lead (not done here): a tree-level R1 on the ADR 0116 `annulus_3d_sector`, which would put a case-tree sector against
a case-tree annulus through the `sim.rs` wiring (on main no case-tree mesh allows it); and, if 0115 converges the E family, repeating the F
family on converged states.

Unverified references: the ADR 0086 observed-order formula (Celik et al. 2008, recalled there). Every other reference value is from ADRs
0008, 0086, 0110 and 0113.

Status: F-126 stays partial; ADR 0110 part L stays open; no default changed.

## Final gate

Fresh `git archive` of HEAD 804dfef (branch `w45c` rebased onto main b7ab54b, which includes the ADR 0116 merge), gate directory
`<scratch dir>`: `cargo fmt --check` clean, `cargo clippy --workspace --all-targets -- -D warnings` clean, `cargo test
--workspace --release --no-fail-fast` **2025 passed, 0 failed, 639 ignored** (main 2020 / 0 / 634 plus this ADR's 5 passing and 5 ignored
tests). `baseline_hashes.rs` untouched and passing; `mrf_cyclic_seams_w44c.rs` passes in both crates. An earlier gate of 87b4a60 (before
the rebase onto b7ab54b) failed clippy only: `neg_cmp_op_on_partial_ord` on the two NaN-failing checks of `rotating_seam_h1_w45c.rs`, now
allowed file-wide as in `fan01_stall_w43c.rs`; its tests gave 2015 / 0 / 638 on the older base.
