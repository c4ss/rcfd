# ADR 0744: F-672 registration: Beam2 in the cyclic constraint by default

Status: **REGISTERED 2026-11-19, written before any code of this registration and before any run of its checks.** Before
this text no library or test code of this registration was written and none of its checks was run. One thing was done
before it: a release build of the unchanged library (`cargo build --release -p rcfd-fem`), which runs no test. Every
number below comes from reading the code, the tests and the ADRs. Results go in the addenda at the end; nothing above them
is changed after a run.

This is a further registration of roadmap item F-672 under rule 13: it adds no acceptance clause and closes the item's gap
("Lifting the Beam2 cyclic refusal by default ... is approved and not yet made"). It answers the brief
[docs/adr/briefs/f672-beam2-cyclic-default.md](briefs/f672-beam2-cyclic-default.md) and follows
`docs/process/agent-rules.md`, version 3. Parent record:
[ADR 0595](0595-f672-registration-beam2-elements-in-the-cyclic-constraint.md) (the first registration of F-672, its
remaining question 3 and the maintainer's answer of 2026-11-18 at its end). Read at main f67e106c.

## What is asked

The maintainer approved on 2026-11-18 "accepting Beam2 elements in the cyclic constraint by default, which retires NC3 of
ADR 0556 and A-B of ADR 0593". Both retired rows use a cyclic constraint with `CyclicOptions::new().shells(true)` and no
`beams(true)`. So the change is: **a cyclic constraint with the shells option takes Beam2 elements without the beams
option.** This is an approved change of a default, so rule 3's "defaults stay bit-identical" is lifted for exactly the
calls listed under "What changes, case by case" and for nothing else. No result of a model that was accepted before
changes: the change turns refusals into results.

The acceptance of F-672, unchanged: a cyclic sector holding Beam2 elements gives the static displacements and the modes of
each harmonic index of the full ring of beams within `1e-10`, the defaults bit-identical.

## What was read

- `docs/process/agent-rules.md` (version 3), the brief, ADR 0595 whole, ADR 0741 (the form of a registration of an
  approved default change).
- ADR 0556 (D1, D2, NC3), ADR 0593 (the audit table, A-B, the answers), and by search for their refusals and options:
  ADR 0598 (F-675: the audit, D1, NC1, NC2, question 3), ADR 0601 (F-674: the audit, D1, NC1, NC2), ADR 0606 (F-680).
- `crates/rcfd-fem/src/cyclic.rs` lines 25 to 50, 170 to 470, 840 to 960, 1245 to 1300; `mpc.rs` lines 1335 to 1436
  (`check_transformation_model` and its two callers); `model.rs` lines 530 to 610 (`CyclicOptions`), 1257 to 1300
  (`add_constraint`, the cyclic arm) and 1340 to 1560 (the support checks); `solve.rs` lines 200 to 250;
  `rotating/whirl.rs` lines 690 to 735 and the list of its functions; `rotating.rs` lines 50 to 60, 192 to 202, 615 to 640.
- Tests: `sa_cyclic_beams_f672.rs` (helpers, S0, the two NC functions), `sa_cyclic_shells.rs` (head, `nc1_to_nc6_refusals`),
  `sa_hyper_structures_f666.rs` (head, the materials, `beam_b`, A-B), `si_beam_transformation.rs` (N3),
  `sa_rotating_cyclic_shells.rs` (NC1), `sa_rotating_cyclic_beams_f675.rs` (helpers, NC1, NC2), and a search of every test
  of the workspace for `beam::FOLLOW_UP`, `kind: "Beam2"` and `shells(true)` without `beams(true)`.
- `CyclicOptions` is used in no crate but `rcfd-fem` (searched): no case key, schema, golden or hash table names it.

### Audit: every test that asserts a refusal of Beam2 with a cyclic constraint

| # | Test | What it asserts | Options | After this registration |
|---|---|---|---|---|
| T1 | `sa_cyclic_shells.rs::nc1_to_nc6_refusals`, NC3 (ADR 0556) | `solve_static_mpc` and `modal_analysis_harmonic` `k = 1` of a three-node Beam2 arc refused `NotYetSupported { variant: "Cyclic" }` naming `beam::FOLLOW_UP` | `shells(true)` | **Changed.** Retired as an ignored record; restated in a new file with NC3 as acceptance |
| T2 | `sa_hyper_structures_f666.rs::a_b_beam2_cyclic_refused_for_its_own_reason` (ADR 0593 A-B) | `modal_analysis_harmonic` `k = 0` of Beam B (hyperelastic, with the setting) refused naming `beam::FOLLOW_UP` | `shells(true)` | **Changed.** Retired as an ignored record; restated in a new file |
| T3 | `sa_cyclic_beams_f672.rs::nc1_to_nc4_refusals_amended`, NC3 (ADR 0595) | `solve_static_mpc` and `modal_analysis_harmonic` `k = 0` of the wheel sector W refused naming `beam::FOLLOW_UP` | `shells(true)` | **Changed.** Retired as an ignored record; restated in a new file with NC3 as acceptance. (The brief names only the two rows of the maintainer's answer; this one is ADR 0595's own control of the same refusal, so it cannot stay.) |
| T4 | `sa_cyclic_beams_f672.rs::nc1_to_nc4_refusals_run1_record` | as T3 | `shells(true)` | already an ignored record (ADR 0595 amendment 1); not touched |
| T5 | `si_beam_transformation.rs` N3 | `solve_static` and `modal_analysis` of a Hex8 block and a beam with a cyclic constraint refused naming `beam::FOLLOW_UP` | default | unchanged, still refused (D1) |
| T6 | `sa_rotating_cyclic_shells.rs` NC1 | `full_rotor` of a Beam2 arc refused `UnsupportedKind { kind: "Beam2" }` | `shells(true)` | unchanged (D3) |
| T7 | `sa_cyclic_beams_f672.rs` NC4 (in T3), `sa_cyclic_beams_full_rotor_f674.rs` NC1 | `full_rotor` of W refused `UnsupportedKind { kind: "Beam2" }` | `shells(true).beams(true)` | unchanged (D3); the NC4 of T3 is restated |
| T8 | `sa_rotating_cyclic_beams_f675.rs` NC1, NC2 | the whirl of the ring as a `RotatingPart` without `RotatingOptions::beams`, and the public `coriolis_matrix`, `coriolis_matrix_with_shells` of W, refused naming `beam::SOLID_ROUTINES` | none, or `shells(true).beams(true)` | unchanged (D4) |

## What changes, case by case

A model with a Beam2 block and one cyclic constraint. "Entry check" is `check_transformation_model` (`solve_static_mpc`,
`modal_analysis_mpc`, and through them `solve_static`, `modal_analysis`, `modal_analysis_harmonic` at `k = 0` and every
path that solves on the transformation) or `check_cyclic_model` (`modal_analysis_harmonic`, `solve_static_harmonic`).

| Constraint options | Before | After |
|---|---|---|
| `shells(true)`, matched boundaries | refused by both entry checks, `NotYetSupported { variant: "Cyclic" }` naming `beam::FOLLOW_UP` | **taken**, with the code and the bits of `shells(true).beams(true)` (ADR 0595). **Changed.** |
| `shells(true)` with `interpolation` or `axis_nodes` (with or without `shell_extensions`) | refused as the row above | still refused, with another error: `InvalidValue { key: "cyclic" }` naming ADR 0595 and ADR 0744 (D2). **Changed (the error only).** |
| `shells(true).beams(true)` | taken (ADR 0595) | the same bits |
| `shells(true).beams(true)` with `interpolation` or `axis_nodes` | refused, key `cyclic`, naming ADR 0595 | the same |
| `beams(true)` without `shells` | refused by `add_constraint`, key `cyclic` | the same |
| default options (no shells) | refused naming `beam::FOLLOW_UP` (T5); a model of beams only cannot hold the constraint at all (no surface sets) | the same |
| any, `full_rotor` | refused `UnsupportedKind` unless `full_rotor_beams` (ADR 0601) | the same |
| `shells(true)`: `RotatingSector::modes`, `campbell` (no Coriolis) | refused at the pre-stress solve or the modal entry check, as the first row | **taken**, with the bits of `shells(true).beams(true)` (ADR 0598: these paths "took the wheel sector without a library change"). **Changed**, as a consequence of the first row |
| `shells(true)`: `RotatingSector::whirl_modes`, `whirl_campbell` (Coriolis) | refused | still refused, now by the Coriolis assembly naming `beam::SOLID_ROUTINES`: the beam Coriolis route stays the opt-in of ADR 0598 (`CyclicOptions::beams` or `RotatingOptions::beams`). **Changed (the error only).** |
| a hyperelastic material on the beams, `shells(true)` | refused for the beam (T2) before the material was looked at | reaches the material check: refused naming `mat_hyper::LINEAR_FOLLOW_UP` without `set_hyperelastic_structures(true)`, taken with it. **Changed.** |

Not changed: every model without a Beam2 block; every model without a cyclic constraint; every constraint without the
shells option; `full_rotor`; the public Coriolis functions; a `RotatingPart`; every case file (no key reaches
`CyclicOptions`).

What still refuses Beam2 with a cyclic constraint afterwards (the brief's list, as it stands today): `full_rotor` without
`full_rotor_beams`; the whirl paths without a beams option; beam ends on interpolated boundaries or on the axis; a
constraint without the shells option. Line loads on the beams of a sector are not refused and not checked, as in ADR 0595
(a global direction, not rotated per sector). The brief's "rotating paths on a beam sector" is narrower today than in ADR
0595: ADR 0598 checked them under the beams option, the modes paths follow the lift (the row above, checked by E-R), and
only the whirl paths keep their own opt-in.

## Design

**D1 (the lift).** In `check_transformation_model` (`mpc.rs`) the Beam2 refusal is taken when no cyclic constraint has the
shells option (it was: unless one has shells and beams). In `check_cyclic_model` (`cyclic.rs`) the shells branch no longer
calls `refuse_cyclic_beams`; that function has no caller left and is removed (the branch without shells refuses a Beam2
block through `check_supported`, as before). Nothing else of the two checks changes, and no row, matrix or load: the rows
are those of ADR 0556 D2 on the six-unknown beam nodes, which `cyclic_rows` emits for any pair of six-unknown nodes under
the shells option, and the sector stands for the ring whose sector `j` holds the beams rotated by `R^j`, orientation
vectors `R^j v` included (ADR 0595 D2, D3: the derivation and the objectivity of the element; not repeated). The shells
option is kept as the condition because it is what makes the boundaries node sets and emits the rotation rows; without it
the rotations of a beam node would be left free of the constraint, so T5 stays.

**D2 (beam ends on extended boundaries stay refused).** With the beams option `check_beams_option` refuses `interpolation`
and `axis_nodes` (ADR 0595 D1). Under the lift a Beam2 model reaches the same place without the option, so a new check
`check_cyclic_beam_boundaries` refuses a model that has a Beam2 block and a cyclic constraint with the shells option and
`interpolation` or `axis_nodes`: key `cyclic`, the message names ADR 0595 and ADR 0744. It is called by both entry checks
after `check_beams_option`. It returns at once for a model without a Beam2 block, so a shell sector with the extensions
of ADR 0575 runs the code it ran before.

**D3 (`full_rotor`).** Not changed: Beam2 needs `full_rotor_beams`, which `add_constraint` takes only with `beams` (ADR
0601 D1). Letting `shells` alone take it would retire NC4 of ADR 0595 and NC1 of ADR 0601, which the approval does not
name.

**D4 (the beams option afterwards).** `CyclicOptions::beams` stays a field. It is no longer needed for the static and the
modal paths. It still (a) selects the beam Coriolis route of the whirl paths (ADR 0598), (b) qualifies `full_rotor_beams`
(ADR 0601), (c) is refused without `shells` and with `interpolation` or `axis_nodes`, also on a model without beams. Its
doc comment, the module docs of `cyclic.rs` and the doc of `check_transformation_model` are rewritten to say so. Lifting
the whirl opt-in by default is question 3 of ADR 0598, not answered and not taken here.

**D5 (no result of an accepted model changes).** The two entry checks only return `Ok` or an error; for a model they
accepted before, D1 removes a test that was false and D2 adds a test that is false (no Beam2, or `beams` set and no
extension), so the same code runs afterwards. Shown by the existing tests unedited (B1), the existing bit records (B2),
`baseline_hashes` (B3) and bit-equality of the lifted default with the beams option (E-S, E-M, E-R).

**D6 (the retired tests and the new files).** T1, T2 and T3 each get one line `#[ignore = "..."]` naming ADR 0744; their
bodies are not touched. Two new test files:

- `crates/rcfd-fem/tests/sa_cyclic_beams_default_adr0744.rs` (header: ADR 0744, F-672). The wheel sector W, the ring, the
  loads, the measures and the list protocol are those of ADR 0595 ("The models", "Checks"), with the builders of
  `sa_cyclic_beams_f672.rs` copied and the sector's constraint `CyclicOptions::new().shells(true)`. It holds the ring
  checks, the bit-equality checks, T3 restated and T2 restated.
- `crates/rcfd-fem/tests/sa_cyclic_shells_refusals_adr0744.rs` (header: ADR 0744, F-672, F-628). T1 restated: NC1, NC2,
  NC4, NC5 and NC6 of ADR 0556 as they are (so each keeps a running check), NC3 as acceptance.

Set-ups fixed now:

- **T1 restated, NC3.** The arc of T1 (nodes `point(0, 0.5, 0)`, `point(1, ..)`, `point(2, ..)`: 30 degrees apart, so node
  2 is the image of node 0 under the rotation by `2 pi / 6`; two Beam2 elements, orientation the axis, `shells(true)`).
  (a) As T1 built it, without supports: both calls return something other than the refusal naming `beam::FOLLOW_UP` (the
  outcome is printed and not asserted further: the arc is unsupported, so a refusal for a singular stiffness is
  expected). (b) The same arc with node 1 clamped in its six components: `solve_static_mpc` (no load) and
  `modal_analysis_harmonic` `k = 1`, one mode, both `Ok`. The modal call uses the file's `modal_options` (tolerance
  `1e-12`, `ResidualTest::Backward`) where T1 used `ModalOptions::new(1)`: with the default relative residual test a beam
  model stops at the rounding floor of ADR 0581 (ADR 0595 arithmetic 3), which is not what this row is about.
- **T2 restated.** (a) The model of A-B itself (Beam B along `x` from the origin, hyperelastic with the setting, `low`
  node 0, `high` node 10, axis `z` through the origin, `shells(true)`): no longer the beam refusal. It is not a sector:
  node 0 lies on the axis, its image is itself and the high node is 1 m away, so the expected outcome is the refusal of
  the boundary match, `InvalidValue { key: "cyclic" }`. (b) W with the hyperelastic material `neo_hookean(1e6, 1e7)`,
  density 1000, **without** the setting: `modal_analysis_harmonic` `k = 0` refused `NotYetSupported { variant:
  "Hyperelastic" }` naming `mat_hyper::LINEAR_FOLLOW_UP` (the material's own refusal, ADR 0555 D3). (c) W with that
  material **and** `set_hyperelastic_structures(true)` against W with the isotropic twin (`E = 9 K G / (3 K + G)`, `nu =
  (3 K - 2 G) / (2 (3 K + G))`, ADR 0584 D2), both `shells(true)`: the eigenvalues of `k = 0` and `k = 1` (4 each),
  `max |lambda_h / lambda_t - 1|`. This is the Beam2 half of clause a1 of F-666, which ADR 0593 could only state as
  refused; it is run here once because T2 is retired here.

## Checks

Limits `1e-10` are the acceptance of F-672 (and of F-666 for H-T); exact limits are the tolerance table's row
"Bit-identity of default output". The estimates of the ring rows are the numbers ADR 0595 measured (Addendum 1): by D1 the
sector with `shells(true)` runs the code that sector ran with `shells(true).beams(true)`, so the same numbers are expected
to the last digit. Where the estimate equals an exact limit the margin is written 1.

| # | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| S0 | Acceptance, static: W with `shells(true)` only, the S0 loads (nodal forces, moments, centrifugal), `solve_static_mpc`, against `solve_static` of the independently built ring: `e = max(e_u, e_theta)` | `<= 1e-10` | acceptance of F-672 | `6.57e-13` (ADR 0595) | 152 | code verification |
| R1 | Acceptance, the rotation unknowns rotated: in the S0 solution `max abs(theta_4 - R theta_0) / max abs(theta)` | `<= 1e-14` | ADR 0595 R1 (substitution) | 0 | - | code verification |
| S1 to S3 | Acceptance, static: `solve_static_harmonic` `k = 1, 2, 3` against the ring under the expanded load: `e` each | `<= 1e-10` | acceptance of F-672 | `6.08e-13`, `3.92e-13`, `3.96e-13` | 164 | code verification |
| M-list | Acceptance, modes (set-up): the ring list complete to the cut-off; counts below it equal | complete; equal | ADR 0595 M-list | complete; 18 and 18 | 1 | code verification |
| M0 to M3 | Acceptance, modes: `d_k`, `k = 0 .. 3` | `<= 1e-10` each | acceptance of F-672 | `8.21e-13` the largest | 122 | code verification |
| P0 to P3 | Acceptance, modes: the backward error `e_phi` of the expanded sector modes in the ring | `<= 1e-10` each | acceptance of F-672 | `1.5e-13` the largest | 667 | code verification |
| Q1, Q2 | Acceptance, modes: the pair splits of `k = 1, 2` | `<= 1e-10` | ADR 0595 Q | `2.56e-13` | 391 | code verification |
| E-S | The lifted default has the bits of the option: W with `shells(true)` against W with `shells(true).beams(true)`: the S0 displacements and the cosine and sine parts of S1 to S3 | bits equal | tolerance table, "Bit-identity of default output"; D5 | equal | 1 | code verification |
| E-M | The same: the eigenvalues of `k = 0 .. 3`, 4 each | bits equal | as E-S | equal | 1 | code verification |
| E-R | The same on the rotating modes path: `RotatingSector::modes`, both effects, `k = 0 .. 3` at 30 rad/s, 4 eigenvalues each | bits equal | as E-S | equal | 1 | code verification |
| A3 | T3 restated, NC3 as acceptance: W with `shells(true)`: `solve_static_mpc` and `modal_analysis_harmonic` `k = 0` | both `Ok` | D1 | `Ok` | 1 | code verification |
| A1 | T1 restated, NC3 as acceptance (set-up above): (a) neither call is the refusal naming `beam::FOLLOW_UP`; (b) clamped, both `Ok` | holds | D1 | holds | 1 | code verification |
| N-0556 | T1 restated: NC1, NC2, NC4, NC5, NC6 of ADR 0556, bodies copied | each as in ADR 0556 | ADR 0556 | as there | 1 | unchanged default output |
| N-0595 | T3 restated: NC1 (amended), NC2 (both), NC4 of ADR 0595, bodies copied; NC4 also with `shells(true)` only | each as in ADR 0595 | ADR 0595 | as there | 1 | unchanged default output |
| N-X | D2: W with `shells(true).shell_extensions(true).axis_nodes(true)`, and with `.interpolation(TieOptions::new(TieMethod::NodeToSurface))` in place of `axis_nodes`, no beams option: `solve_static_mpc` and `modal_analysis_harmonic` `k = 1` | refused, key `cyclic`, the message names ADR 0744 | D2 | as the limit | 1 | code verification [negative control] |
| N-W | D4: W with `shells(true)` only, `RotatingSector::whirl_modes` `k = 1` at 30 rad/s with `coriolis(true)` | refused `Element { NotYetSupported { variant: "Beam2", package: beam::SOLID_ROUTINES } }` | ADR 0598 NC1 | as the limit | 1 | code verification [negative control] |
| NC-O | The control of S0: the ring with unrotated orientation vectors | `e >= 1e-3` | ADR 0595 NC-O | `0.778` | 778 | code verification [negative control] |
| AB-a | T2 restated: the model of A-B | not the refusal naming `beam::FOLLOW_UP`; `InvalidValue { key: "cyclic" }` | D1; the set-up above | as the limit | 1 | code verification |
| H-D | T2 restated: W, hyperelastic, without the setting | refused naming `mat_hyper::LINEAR_FOLLOW_UP`, variant `Hyperelastic` | ADR 0555 D3, ADR 0584 D1 | as the limit | 1 | code verification [negative control] |
| H-T | T2 restated: W, hyperelastic with the setting, against the twin: `max abs(lambda_h / lambda_t - 1)` over `k = 0, 1` | `<= 1e-10` | acceptance of F-666 (clause a1) | `<= 1e-13` (ADR 0593 MP-B measured 0 on Beam B; the twin's moduli differ from the linearised ones by rounding) | 1000 | code verification |
| R-T | The retired records, run once with `--ignored`: T1 fails with exactly its two NC3 lines missed, T3 with exactly its two NC3 lines, T2 fails at A-B | fail there | D6 | fail there | 1 | code verification [negative control] |
| B1 | Defaults: the existing tests of `rcfd-fem` that reach the two entry checks, unedited but for the three ignore lines, pass: `sa_cyclic_shells`, `sa_cyclic_shell_extensions`, `sa_cyclic_shell_two_sided`, `sa_cyclic_beams_f672`, `sa_cyclic_beams_full_rotor_f674`, `sa_rotating_cyclic_shells`, `sa_rotating_cyclic_beams_f675`, `sa_ring_whirl_beams_f679`, `sa_free_sector_prestress_f680`, `sa_hyper_structures_f666`, `si_beam`, `si_beam_transformation`, `si_cyclic`, `si_cyclic_harmonic`, `si_rotating_cyclic`, `sa_ortho_cyclic`, `si_shell_transformation`, and `--lib` | 0 failures | rule 7; tolerance table, "Bit-identity of default output" | 0 | 1 | unchanged default output |
| B2 | Defaults, the existing bit records: `sa_ortho_cyclic_default_bits`, `sa_ortho_cyclic_prestress_default_bits`, `sa_rotating_cyclic_shells_default_bits`, `sa_tie_default_bits`, `sa_geometric_struct_bits`, and inside B1 the solid twin record B4 of `sa_cyclic_shells` and the shell-sector record B3 of `sa_cyclic_beams_f672` | 0 failures | tolerance table, "Bit-identity of default output" | 0 | 1 | unchanged default output |
| B3 | Defaults: `cargo test -p rcfd-api --release --test baseline_hashes` (`mpc.rs` and `cyclic.rs` are shared solver files) | pass | rule 3 | pass | 1 | unchanged default output |

No row is of kind validation: the acceptance of F-672 claims agreement with the full ring computed by the same code, not
with a measured case.

### Negative controls and planned mutations

Controls: NC-O (the ring comparison sees a wrong ring), N-X, N-W and H-D (what must stay refused), R-T (the retired
records fail for the registered reason and no other). Mutations, each on a scratch copy with its own target directory,
the named tests run once:

| Id | Mutation | Must be caught by |
|---|---|---|
| Y1 (committed, rule 16) | `check_transformation_model`: the Beam2 refusal again unless `beams_opted_in`. Patch `crates/rcfd-fem/tests/sa_cyclic_beams_default_adr0744.mutation.patch` | S0, E-S, A3, AB-a (refused naming the beam) in the first new file; A1 in the second |
| Y2 | `check_cyclic_model`: the shells branch refuses Beam2 again unless `beams_opted_in` | S1 to S3, M, E-S, E-M, A3 (modal) |
| Y3 | `check_cyclic_beam_boundaries` returns `Ok` at once | N-X |
| Y4 | `check_transformation_model`: the Beam2 refusal never taken (also without shells) | the existing `si_beam_transformation` (N3, T5) |

## Size

Size (agent): done; 0; 0; 0; 420000

## Decision rule

- **done**: every row S0 to B3 is met in its one registered run, and every mutation is caught by at least one check.
- **partial**: a row is missed (recorded by rule 2) or not run, or a mutation escapes.
- **todo**: the lift cannot be made without changing the result of a model that was accepted before, or without an edit
  of `baseline_hashes.rs`, a hash table or a golden file.

## What is not covered

- `full_rotor` by default (D3), the whirl paths by default (D4, ADR 0598 question 3).
- Beam ends on interpolated boundaries or on the axis (refused, N-X); a beam node shared with a shell on a cyclic
  boundary (taken, not exercised, as in ADR 0595); line loads on the beams of a sector (not refused, not checked).
- The rotating modes path under the lift is checked for bit-equality with the option (E-R) at one speed; its agreement
  with the rotating ring is ADR 0598's, under the option.
- F-666's default lift (RF1): not part of this registration. H-T uses the setting.
- The case schema: no case key reaches `CyclicOptions`.

## Amendment 1 (2026-11-19, after the runs): the notation of five kinds

Found by `python scripts/process_report.py --check` after the runs. Five rows of the table of checks (N-X, N-W, NC-O,
H-D, R-T) write their kind as `code verification [negative control]`. The qualifier belongs in round brackets, which is
what the report script reads: the kind of each of the five is **code verification (negative control)**. No check, limit,
source or estimate changes, and no run depends on it. The table above is not edited (rule 18), so the script still
reports those five rows; that is for the lead (see "Not done" below).

## Addendum 1: results (2026-11-19)

Built as registered: the registration 85c5ae18, then the code and the tests cc8a4917, then the runs. Library changes
(shared solver files, lines of the new files): `crates/rcfd-fem/src/mpc.rs` 1359 (doc), 1367 to 1372
(`check_transformation_model`: the call of the new check and the condition `has(Beam2) && !shells`);
`crates/rcfd-fem/src/cyclic.rs` 41 to 43 (module doc), 254 to 272 (`check_cyclic_beam_boundaries`, new), 888 to 891
(`check_cyclic_model`), and the 25 lines of `refuse_cyclic_beams` removed after 897; `crates/rcfd-fem/src/model.rs` 564
to 566 (the doc of `CyclicOptions::beams`). The three retired tests have one `#[ignore = "ADR 0744: ..."]` line each and
no other change. All runs in the release profile, each registered run made once; logs `run1.log`, `rt.log`, `b1_b2.log`,
`b3.log`, `docs_site.log`, `mutations.log` in the scratch folder.

| # | Limit | Measured | Met |
|---|---|---|---|
| S0 | `e <= 1e-10` | `e_u 6.5675e-13`, `e_theta 6.3875e-13` | yes |
| R1 | `<= 1e-14` | 0 | yes |
| S1 to S3 | `e <= 1e-10` each | `6.0777e-13`, `3.9190e-13`, `3.9603e-13` | yes |
| M-list | complete; counts equal | cut-off `2.917107e6`, largest ring eigenvalue `5.320926e6`; 18 and 18 | yes |
| M0 to M3 | `<= 1e-10` each | `8.2090e-13`, `2.9377e-13`, `7.5273e-14`, `1.0303e-13` | yes |
| P0 to P3 | `<= 1e-10` each | `6.1477e-16` (3 modes), `1.4706e-13` (6), `9.9113e-17` (6), `6.5375e-17` (3) | yes |
| Q1, Q2 | `<= 1e-10` | `2.5579e-13`, `2.0357e-13` | yes |
| E-S | bits equal | 294 values equal | yes |
| E-M | bits equal | 24 values equal | yes |
| E-R | bits equal | 24 values equal | yes |
| A3 | both `Ok` | `solve_static_mpc` `Ok`; `modal_analysis_harmonic` `k = 0` `Ok` (21.574 Hz, 57.427 Hz) | yes |
| A1 | (a) not the beam refusal; (b) both `Ok` | (a) `solve_static_mpc`: `SingularStiffness { node: 0, component: 2 }` (the unsupported arc, as expected), `modal_analysis_harmonic` `k = 1`: `Ok`; (b) both `Ok` | yes |
| N-0556 | as in ADR 0556 | NC1 `UnknownSet { Surface, "low" }`; NC2 both `NotYetSupported Shell4`; NC4 "low node 8 has 6 unknowns and its high partner 0 has 3"; NC5 both naming ADR 0556; NC6 "has no node of the high boundary" | yes |
| N-0595 | as in ADR 0595 | NC1 refused by `add_constraint`, key `cyclic`, ADR 0595; NC2 both key `cyclic`, ADR 0595; NC4 `UnsupportedKind { kind: "Beam2", operation: "full_rotor" }` with the beams option and with shells alone | yes |
| N-X | refused, key `cyclic`, naming ADR 0744 | all four calls: "constraint 6: Beam2 in the cyclic constraint (ADR 0595, by default since ADR 0744) is not taken with interpolated boundaries or nodes on the axis" | yes |
| N-W | refused naming `beam::SOLID_ROUTINES` | `Element { element: 0, source: NotYetSupported { variant: "Beam2", location: "element routines", package: beam::SOLID_ROUTINES } }` | yes |
| NC-O | `e >= 1e-3` | `0.77791` | yes |
| AB-a | not the beam refusal; `InvalidValue { key: "cyclic" }` | `InvalidValue { key: "cyclic" }`: "low node 0 at [0, 0, 0] of 'x0': its image ... has no node of the high boundary 'xl' within 1e-8 (nearest: node 10 at 1e0)" | yes |
| H-D | refused naming `mat_hyper::LINEAR_FOLLOW_UP` | `NotYetSupported { variant: "Hyperelastic", location: "material 0, element set 'all': a structural element (Beam2, Shell4, Shell3)", package: "a follow-up of ADR 0555" }` | yes |
| H-T | `<= 1e-10` | 0 (the eight eigenvalues equal those of the twin to the last bit; `k = 0`: 0.23014, 0.61257, 2.4529, 3.1704 Hz) | yes |
| R-T | the retired records fail at their NC3 or A-B lines only | T1: missed exactly "NC3 ... solve_static_mpc" (`SingularStiffness`) and "NC3 ... modal_analysis_harmonic" (`Ok`); T3: exactly "NC3 shells only: solve_static_mpc" and "... modal_analysis_harmonic k = 0" (both `Ok`); T2: "A-B" (the boundary-match refusal) | yes |
| B1 | 0 failures | `--lib` 15; `sa_cyclic_shells` 8 (1 ignored: T1); `sa_cyclic_shell_extensions` 7 (1 ignored before); `sa_cyclic_shell_two_sided` 7; `sa_cyclic_beams_f672` 5 (2 ignored: T3, T4); `sa_cyclic_beams_full_rotor_f674` 8; `sa_rotating_cyclic_shells` 8 (1 ignored before); `sa_rotating_cyclic_beams_f675` 4 (1 ignored before); `sa_ring_whirl_beams_f679` 4; `sa_free_sector_prestress_f680` 5; `sa_hyper_structures_f666` 10 (4 ignored: 3 before and T2); `si_beam` 9 (4 ignored before); `si_beam_transformation` 5; `si_cyclic` 11 (1 ignored before); `si_cyclic_harmonic` 9 (1 ignored before); `si_rotating_cyclic` 7 (1 ignored before); `sa_ortho_cyclic` 7; `si_shell_transformation` 7 (2 ignored before). 0 failed | yes |
| B2 | 0 failures | `sa_ortho_cyclic_default_bits` 1, `sa_ortho_cyclic_prestress_default_bits` 1, `sa_rotating_cyclic_shells_default_bits` 1, `sa_tie_default_bits` 1, `sa_geometric_struct_bits` 1; B4 of `sa_cyclic_shells` and B3 of `sa_cyclic_beams_f672` pass inside B1. 0 failed | yes |
| B3 | pass | `baseline_hashes`: 3 passed, 1 ignored (as before) | yes |

The ring rows repeat the numbers of ADR 0595 Addendum 1 to the digits printed there, as estimated.

### Mutations (scratch copy of cc8a4917, own target directory, each run once)

| Id | Registered to be caught by | Caught by |
|---|---|---|
| Y1, committed as `crates/rcfd-fem/tests/sa_cyclic_beams_default_adr0744.mutation.patch` | S0, E-S, A3, AB-a; A1 | 6 of the 7 tests of the first file fail (S0, NC-O and M at the `k = 0` solve, E-S, A3, AB-a: all refused naming the beam); A1 (a) and (b) in the second file |
| Y2 | S1 to S3, M, E-S, E-M, A3 (modal) | S1 to S3, M, E-S, A3 (modal), and AB-a (the `k = 0` call passes `check_cyclic_model` first); 5 tests fail |
| Y3 | N-X | N-X, all four calls: with `axis_nodes` both calls return `Ok`; with `interpolation` the refusal is another one ("the low node set has no shell edge", ADR 0575), not the registered one |
| Y4 | `si_beam_transformation` N3 | N3, both calls (the boundary-match refusal in place of the beam refusal); the first new file is not affected (7 pass), as registered |

Four of four caught.

### Checks of rule 7

`cargo clippy -p rcfd-fem --all-targets -- -D warnings`: clean. `cargo fmt -p rcfd-fem --check` could not be run in this
worktree: it stops with "The filename or extension is too long (os error 206)" before formatting (the command line of
the crate's test files with this worktree's path). In its place `rustfmt --edition 2021 --check` (the repository's
`rustfmt.toml`) on the eight Rust files changed or added: clean. `baseline_hashes`: row B3. `docs_site` (`cargo test -p
rcfd-cli --release --test docs_site`): 2 passed, run again after this addendum and the topic page. `npm test` not run:
`apps/studio-web` is not touched.

### Deviations

1. Amendment 1 (the notation of five kinds).
2. One text-only change of the first new test file after its run: the type alias `Values` for `fn(CyclicOptions) ->
   Vec<f64>` (clippy `type_complexity`). No value or assertion changes; the file was compiled again by clippy and not run
   again.
3. T3 is retired beyond the two tests the brief names (registered in the audit before the code).
4. `cargo fmt -p` replaced by `rustfmt --check` on the changed files (above).
5. A1 (a): the modal call on the unsupported arc returned `Ok` where the registration expected a singular-stiffness
   refusal for both calls; the row asserted only "not the beam refusal", which holds.

### Not run, not done

- The whole workspace suite and the other integration tests of `rcfd-fem` (the lead's gate).
- `docs/adr/index.md` is not current for this ADR, and the report script lists the five kind cells of amendment 1: both
  are the lead's (the index is generated on main; the kind cells need either a line each in
  `exceptions/registration-kind.txt` or the lead's decision on the notation).
- Nothing of "What is not covered" was run.

### What F-666 will find changed

- `sa_hyper_structures_f666.rs::a_b_beam2_cyclic_refused_for_its_own_reason` is an ignored record of ADR 0744. Its model
  is not a sector (the boundary match refuses it), so it cannot become an accepting row; F-666's default lift should
  leave it as it is.
- The Beam2 half of clause a1 of F-666 is no longer refused. It has one passing row here, H-T (W, neo-Hookean with the
  setting, against the twin, `k = 0, 1`: 0), in `sa_cyclic_beams_default_adr0744.rs`.
- H-D in the same file asserts the refusal of ADR 0555 D3 on the beams of W without the setting. F-666's default lift
  turns that refusal into a result, so H-D is one more test for F-666 to retire as a record with an accepting copy, beside
  RF1. It is inside the test `ab_a_h_d_h_t_hyperelastic_beams_on_the_cyclic_path`, with AB-a and H-T, which the copy must
  keep.

### Size

Size (agent): done; 0; 0; 1; about 380000

### Decision

By the registered rule: **done**. Every row S0 to B3 is met in its one registered run and four of four mutations are
caught. The acceptance of F-672 has passing checks under the lifted default (S0 to Q2 against the full ring within
`8.3e-13`, limit `1e-10`), and every model that was accepted before keeps its bits (B1, B2, B3, E-S, E-M, E-R). The gap of
the item is closed: the refusal is lifted by default, NC3 of ADR 0556, A-B of ADR 0593 and NC3 of ADR 0595 are retired as
ignored records with restated copies.

## Open questions for the maintainer

1. **The two opt-ins that are left on a beam sector.** After this lift the static and the modal paths take Beam2 with the
   shells option alone, while the whirl paths still need `CyclicOptions::beams` (ADR 0598, its question 3) and
   `full_rotor` still needs `beams` and `full_rotor_beams` (ADR 0601, its question 1). Lifting either changes a default
   (it turns a refusal into a result) and retires controls: N-W of this ADR for the whirl paths; NC4 of ADR 0595 as
   restated here, NC1 of ADR 0601 and NC1 of `sa_rotating_cyclic_shells.rs` for `full_rotor`. Options: (a) leave both
   opt-ins; (b) lift both in one further registration, with `beams` and `full_rotor_beams` kept as documented no-ops;
   (c) lift the whirl paths only. Recommendation: (b). A sector that solves and gives its modes without an option and
   then refuses its whirl modes or its full rotor for want of one is harder to explain than either state, and both
   capabilities have their registered checks (ADR 0598, ADR 0601).

## For the lead: roadmap entries

Status proposed for F-672: done.

> ADR 0744, 2026-11-19, done: a cyclic constraint with the shells option takes Beam2 without CyclicOptions::beams (the maintainer's approval of 2026-11-18); the wheel sector of ADR 0595 with shells alone gives the full ring within 6.6e-13 (static k = 0), 6.1e-13 (harmonic static), 8.3e-13 (eigenvalues of k = 0..3), limit 1e-10, with the bits of the beams option; NC3 of ADR 0556, A-B of ADR 0593 and NC3 of ADR 0595 are ignored records restated in two new files; beam ends on interpolated or axis boundaries stay refused by a new check, full_rotor and the whirl paths keep their opt-ins; existing tests, bit records and baseline hashes unchanged; four of four mutations caught.

No follow-up item is proposed; the remaining opt-ins are open question 1.
