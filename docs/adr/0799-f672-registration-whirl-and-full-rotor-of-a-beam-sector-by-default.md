# ADR 0799: F-672 registration: the whirl paths and full_rotor of a beam sector by default

Status: **REGISTERED 2026-11-25, written before any code of F-672 and before any run of its checks.** Before this text no
library or test code of this registration was written, compiled or run: the files under "What was read" were read and
nothing else was done. Results go in the addenda at the end; nothing above them is changed after a run.

This is a further registration of roadmap item F-672 under rule 13: it adds no acceptance clause and closes the item's gap
("the whirl paths and full_rotor take a beam sector without the beams and full_rotor_beams options, in one further
registration, the options kept as no-ops"). It answers the brief
[docs/adr/briefs/f672-beam-opt-ins-default.md](briefs/f672-beam-opt-ins-default.md) and follows
`docs/process/agent-rules.md`, version 5. Written by Claude Opus 5.5. Parent records:
[ADR 0744](0744-f672-registration-beam2-in-the-cyclic-constraint-by-default.md) (its question 1 and the maintainer's answer
of 2026-11-21 at its end), [ADR 0601](0601-f674-registration-full-rotor-and-expansion-of-a-cyclic-sector-with-beam2.md)
(`full_rotor_beams`) and ADR 0598 (F-675, the beam Coriolis route of the whirl paths). Read at main dcb4a1df.

## What is asked

The maintainer answered question 1 of ADR 0744 on 2026-11-21 "Yes, both", on its recommendation (b): the whirl paths and
`full_rotor` take a beam sector without `CyclicOptions::beams` and `CyclicOptions::full_rotor_beams`, the two options kept
as documented no-ops; the controls this retires (N-W of ADR 0744, NC4 of ADR 0595 as restated in ADR 0744, NC1 of ADR 0601,
NC1 of `sa_rotating_cyclic_shells.rs`) are retired as ignored records and restated; nothing else is approved. Decided by the
lead in the brief: the options stay in the API, accepted and without effect, their doc comments saying so; a call that sets
an option on gives bits equal to a call that does not; a model without beams keeps its bits.

So the change is: **a cyclic constraint with the shells option takes Beam2 on the whirl paths and in `full_rotor` as it did
with `shells(true).beams(true)` and with `shells(true).beams(true).full_rotor_beams(true)`.** Rule 3's "defaults stay
bit-identical" is lifted for exactly the calls listed under "What changes, case by case", by the maintainer's answer, and
for nothing else. The change turns refusals into results; no result of a call that was accepted before changes.

The acceptance of F-672, unchanged (read from `docs/roadmap.json` with a script): "A cyclic sector holding Beam2 elements
(the orientation vector and the rotation unknowns rotated per sector) gives the static displacements and the modes of each
harmonic index of the full ring of beams within 1e-10, the defaults bit-identical." Its static and modal clauses have their
passing rows in ADR 0744 under the shells option alone; this registration adds the whirl paths and `full_rotor`, whose
agreement with the ring is that of ADR 0598 and ADR 0601 under the options, carried over by bit equality (D1, D2).

## What was read

- `AGENTS.md`, `docs/process/agent-rules.md` (version 5), the brief, ADR 0744 whole, ADR 0601 whole.
- `crates/rcfd-fem/src/cyclic.rs`: the module doc (lines 25 to 47), `shells_opted_in`, `beams_opted_in`,
  `full_rotor_beams_opted_in`, `check_beams_option`, `check_cyclic_beam_boundaries`, `boundary` (lines 185 to 330),
  `check_cyclic_model` (870 to 900), `FullRotor` (1071 to 1095) and `full_rotor` (1239 to 1345).
  `crates/rcfd-fem/src/model.rs`: `CyclicOptions` (lines 545 to 612) and the cyclic arm of `add_constraint` (1262 to 1300).
  `crates/rcfd-fem/src/rotating.rs`: `RotatingOptions` (185 to 280). `crates/rcfd-fem/src/rotating/whirl.rs`:
  `coriolis_matrix_with_shells`, `coriolis_assembly` (60 to 100), `RotatingPart::coriolis` and `whirl_modes` (705 to 735).
  By search, every caller of `coriolis()` in `rotating/whirl.rs`, `rotating/whirl/state.rs`, `rotating/whirl/state/sector.rs`,
  `rotating/whirl/state/arnoldi.rs` and `gyro.rs`, and `SparseStateOptions` (arnoldi.rs lines 30 to 65).
- Tests: `sa_cyclic_beams_default_adr0744.rs` (the list of its functions, E-S to N-W whole),
  `sa_cyclic_beams_full_rotor_f674.rs` (helpers, B3, NC1 and NC2), `sa_rotating_cyclic_beams_f675.rs` (helpers, the W and SM
  rows, B3, NC1 and NC2), `sa_rotating_cyclic_shells.rs` (head, `Record`, NC1 to NC4), `sa_cyclic_beams_f672.rs`,
  `sa_ring_whirl_beams_f679.rs`, `sa_free_sector_prestress_f680.rs` (the list of their functions and their options),
  `sa_rotating_cyclic_shells_default_bits.rs` (the form of a bit record), and a search of every test of the workspace for
  `beams(true)`, `full_rotor_beams`, `SOLID_ROUTINES`, `UnsupportedKind` and `kind: "Beam2"`.
- `CyclicOptions` is used in no crate but `rcfd-fem` (searched): no case key, schema, golden or hash table names it.
- For the Linux record: ADR 0792 amendment 1 (a record per platform, a platform without its record prints and does not
  compare) and ADR 0150 D4 (the image `rcfd-linux:ubuntu24.04-rust1.99.0`, ubuntu 24.04 and the gate's toolchain), and
  `scripts/make_example_projects.py` (its `docker run` and volumes). The image and the volume
  `rcfd-examples-cargo-registry` are on this machine (listed, nothing run in them).
- `scripts/process_report.py` sections c, r, n and s (what a registration must hold).

### Audit: every test that asserts a refusal that this registration lifts

Every sector whirl path reaches the one function `RotatingPart::coriolis`: `RotatingSector::whirl_modes`,
`whirl_campbell` (whirl.rs), `split_modes`, `split_campbell` (gyro.rs), `whirl_state`, `whirl_stability`
(state/sector.rs) and `whirl_state_sparse` (state/arnoldi.rs) call `self.part.coriolis()`. `full_rotor` is one function.

| # | Test | What it asserts that lifts | After this registration |
|---|---|---|---|
| T1 | `sa_cyclic_beams_default_adr0744.rs::a3_nc1_nc2_nc4_restated_and_n_x_n_w` | NC4: `full_rotor` of W refused `UnsupportedKind { kind: "Beam2" }` with `shells(true).beams(true)` and with `shells(true)`; N-W: `RotatingSector::whirl_modes` `k = 1` of W with `shells(true)` refused naming `beam::SOLID_ROUTINES` | **Retired** as an ignored record; restated in the new file: NC1, NC2, A3, N-X copied, NC4 and N-W as acceptance |
| T2 | `sa_cyclic_beams_full_rotor_f674.rs::nc1_nc2_refusals` | NC1: `full_rotor` of W with `shells(true).beams(true)` refused `UnsupportedKind { kind: "Beam2" }` | **Retired** as an ignored record; restated: NC1 as acceptance, NC2 copied |
| T3 | `sa_rotating_cyclic_shells.rs::nc_refusals_without_the_options` | NC1: `full_rotor` of a three-node Beam2 arc with `shells(true)` refused `UnsupportedKind { kind: "Beam2", operation: "full_rotor" }` | **Retired** as an ignored record; restated: NC1 as acceptance, NC2, NC3, NC4 copied |
| - | `sa_cyclic_beams_f672.rs::nc1_to_nc4_refusals_amended`, `..._run1_record` | NC4 as T1 | already ignored records (ADR 0744, ADR 0595); not touched |

Refusals that stay, asserted by tests that are not touched: `sa_rotating_cyclic_beams_f675.rs::nc1_nc2_refusals_without_the_option`
(NC1: the whirl of the ring as a `RotatingPart` without a cyclic constraint and without `RotatingOptions::beams`; NC2: the
public `coriolis_matrix` and `coriolis_matrix_with_shells` of W), `sa_tie.rs` N6 (`full_rotor` of an extended boundary),
`si_beam_transformation.rs` N3 (Beam2 with a cyclic constraint without the shells option on the static and modal paths). No
other test of the workspace asserts a refusal of `full_rotor` or of a whirl path on a beam sector (`sa_estimate.rs` R1 is
the stress recovery, not a cyclic path).

## What changes, case by case

A model with a Beam2 block and one cyclic constraint, matched boundaries, unless stated.

| Call | Constraint options | Before | After |
|---|---|---|---|
| the seven sector whirl paths above (Coriolis on or off: each calls `coriolis()`), and `RotatingPart::coriolis` and the part's own whirl paths on such a model | `shells(true)` | refused at the Coriolis assembly: `Element { NotYetSupported { variant: "Beam2", package: beam::SOLID_ROUTINES } }` | **taken**, with the bits of `shells(true).beams(true)` (ADR 0598). **Changed** |
| the same | `shells(true).beams(true)` (with or without `full_rotor_beams`, with or without `RotatingOptions::beams`) | taken (ADR 0598) | the same bits |
| `full_rotor` and the expansions of its `FullRotor` | `shells(true)`, `shells(true).beams(true)` | refused `UnsupportedKind { kind: "Beam2", operation: "full_rotor" }` | **taken**: each Beam2 block becomes `N` blocks, copy `j` with `R^j v` (ADR 0601 D2), with the bits of `shells(true).beams(true).full_rotor_beams(true)`. **Changed** |
| the same | `shells(true).beams(true).full_rotor_beams(true)` | taken (ADR 0601) | the same bits |
| `full_rotor` | no shells option (a Beam2 block beside solids) | refused `UnsupportedKind { kind: "Beam2" }` | the same (new control N-F) |
| `full_rotor` | interpolation or axis nodes | refused, key `cyclic` (ADR 0456 D4) | the same |
| `add_constraint` | `beams` without `shells`; `full_rotor_beams` without `beams` | refused, key `cyclic` (ADR 0595 D1a, ADR 0601 D1) | the same (question 1) |
| every cyclic entry check | `beams` with `interpolation` or `axis_nodes` (also on a model without beams) | refused, key `cyclic` (ADR 0595 D1) | the same (question 1) |
| a `RotatingPart` of a model without a cyclic constraint | any | Beam2 needs `RotatingOptions::beams` (ADR 0598) | the same |
| `coriolis_matrix`, `coriolis_matrix_with_shells` | any | refuse Beam2 | the same |

Not changed: every model without a Beam2 block (D1, D2 show that it runs the code it ran before); every call that was
accepted before; the static and modal paths (ADR 0744); every case file (no key reaches `CyclicOptions`).

`FullRotor::block(j, b)` of a beam sector with the shells option alone gives the split block indices of ADR 0601 D2 (no
caller saw other indices: the call was refused).

## Design

**D1 (the whirl paths).** In `RotatingPart::coriolis` (`rotating/whirl.rs`) the beam Coriolis route
`coriolis_assembly(model, axis, threads, shells, true)` is taken when `self.options.beams ||
crate::cyclic::shells_opted_in(self.model)`; it was `self.options.beams || crate::cyclic::beams_opted_in(self.model)`.
Nothing else of the function changes. The derivation of the matrix is ADR 0598 D1 and D2 (the Coriolis matrix of the beam
section, `crate::beam::element_coriolis`), and the sector stands for the ring of its beams rotated by `R^j` (ADR 0595 D2,
D3); neither is repeated. Bits, by cases on the model:

1. A cyclic constraint with `shells` and `beams`: `add_constraint` refuses `beams` without `shells` (ADR 0595 D1a), so
   `beams_opted_in` implies `shells_opted_in`; both conditions are true, and the same call with the same arguments is made.
2. A cyclic constraint with `shells` and no constraint with `beams`, and `RotatingOptions::beams` off: before,
   `coriolis_matrix_with_shells`, that is `coriolis_assembly(model, axis, threads, true, false)`; after,
   `coriolis_assembly(model, axis, threads, true, true)`. The two closures differ only in the first test `beams &&
   ctx.kind == ElementKind::Beam2`. Without a Beam2 block it is false for every element, so every element takes the routine
   it took before, in the same order: the same bits. With a Beam2 block the call was refused and is now the beam route.
3. No cyclic constraint with `shells`: both conditions are `self.options.beams`; unchanged.

**D2 (`full_rotor`).** In `full_rotor` (`cyclic.rs`) the Beam2 blocks are taken and split when the model has a cyclic
constraint with the shells option: `let split_beams = shells;` (the value of `shells_opted_in(model)` the function already
computes) in place of `full_rotor_beams_opted_in(model)`. The kind check and the block loop read `split_beams` as they do
now; neither changes. Bits, by cases: (1) `full_rotor_beams` set: `add_constraint` refuses it without `beams` and `beams`
without `shells`, so it implies `shells`; the same branch is taken for every block. (2) `shells` without `full_rotor_beams`
and no Beam2 block: `split_beams` is now true, but the split branch is entered only for a Beam2 block, so every block takes
the branch it took before (one `add_block` with its `N` copies, the property cloned), and the block map stays the identity.
(3) No `shells`: `split_beams` false as before. With a Beam2 block under (2) the call was refused and is now the split of
ADR 0601 D2, whose checks against the independently built ring (G1, O1, L, F, Z0, ZM, P, RQ, G) were made with the option.
The `debug_assert!(split_beams || fb == b)` stays (true in each case).

**D3 (dead code).** `beams_opted_in` and `full_rotor_beams_opted_in` (`cyclic.rs`) have no caller left and are removed.
`shells_opted_in` is used as it is.

**D4 (the options afterwards).** `CyclicOptions::beams` and `CyclicOptions::full_rotor_beams` stay public fields with their
builders, accepted, and without effect on any result: their doc comments say so (since ADR 0744 and ADR 0799 the shells
option alone takes Beam2 on every cyclic path and in `full_rotor`). The refusals of an option combination stay as they are:
`beams` without `shells` and `full_rotor_beams` without `beams` (`add_constraint`), `beams` with `interpolation` or
`axis_nodes` (`check_beams_option`). Lifting them would turn three more refusals into results and retire NC1 and NC2 of
ADR 0595 (restated in ADR 0744) and NC2 of ADR 0601, which the answer does not name; that is question 1. So "a call that
sets an option on gives bits equal to a call that does not" is checked on every call in which the options are accepted.
`RotatingOptions::beams` is not changed: it is what a `RotatingPart` without a cyclic constraint needs (F-675 NC1); its doc
says that a sector with the shells option takes Beam2 without it. The doc comments of `RotatingPart::coriolis`,
`coriolis_matrix_with_shells`, `full_rotor` and the module doc of `cyclic.rs` are rewritten to say the same.

**D5 (the retired tests and the new files).** T1, T2 and T3 each get one line `#[ignore = "ADR 0799: ..."]`; their bodies
are not touched. Two new test files, each headed with ADR 0799 and F-672:

- `crates/rcfd-fem/tests/sa_beam_opt_ins_default_f672.rs`: the wheel sector W of ADR 0595 (the builders of
  `sa_rotating_cyclic_beams_f675.rs` copied: axis `(1, 2, 2)/3`, origin `(0.3, -0.2, 0.5)`, `N = 6`, ring and spoke blocks
  with oblique orientations, hub clamped), the ADR 0556 Shell4 frustum sector (the `shell_sector` of
  `sa_cyclic_beams_full_rotor_f674.rs` copied), and the builders that T3 needs from `sa_rotating_cyclic_shells.rs`, copied.
  It holds E-W, E-F, E-WS, E-FS, T1 to T3 restated and N-F.
- `crates/rcfd-fem/tests/sa_beam_opt_ins_default_f672_bits.rs`: the record of main (B-M).

Set-ups fixed now:

- **The three option sets.** (a) `CyclicOptions::new().shells(true)`; (b) `(a).beams(true)`; (c) `(b).full_rotor_beams(true)`.
- **The whirl paths** of a sector model, `RotatingSector::new(m, o)` with `o = RotatingOptions::new(4)` and the modal options
  of ADR 0595 (`tolerance = 1e-12`, `ResidualTest::Backward`), `.coriolis(true)`: P1 `part().coriolis()`; P2 `whirl_modes(k,
  30.0)`, `k = 0 .. 3`; P3 `whirl_campbell(&[0, 1, 2, 3], &[0.0, 30.0, 60.0])`; P4 `split_modes(k, 30.0)`, `k = 0 .. 3`; P5
  `split_campbell(&[0, 1, 2, 3], &[0.0, 30.0, 60.0])`; P6 `whirl_state(k, 30.0)`, `k = 0 .. 3`; P7 `whirl_stability(&[0, 1,
  2, 3], &[0.0, 30.0, 60.0])`; P8 `whirl_state_sparse(1, 30.0, &SparseStateOptions::new(2.0e4, 4))`. The speeds are those
  of the W rows of ADR 0598's test, well below W's lowest frequency (21.574 Hz, 135.6 rad/s, ADR 0744 A3); the shift `2e4`
  rad^2/s^2 lies near `(2 pi 21.574)^2 = 1.84e4`, so the sparse solver has eigenvalues near it.
- **The full rotor** of a sector model: `full_rotor(m)`; the model F of its mesh (a node set `r<p>` on every node, the
  material on `all`, the hub clamped, as `rotor_model` of the F-674 test); the first mode of `modal_analysis_harmonic(m,
  &[], 1, modal 4)` expanded by `expand`, and the fields `fa_e = sin(0.37 e + 0.1)`, `fb_e = cos(0.53 e + 0.4)` over the
  sector's equations by `expand_load` and `expand_field` at `k = 1` (as B3 of ADR 0601).
- **Equality of results.** Two results are equal when their `Debug` texts are equal. Rust prints an `f64` in `Debug` as the
  shortest decimal that reads back to the same value, so equal texts mean equal bits (zeros by their sign; no result here
  holds a NaN). The results are whole: `Result` of each path, every field (eigenvalues, modes, waves, stability, the full
  mesh with its block properties). No result type holds the options (read: `FullRotor`, the whirl result types).
- **The record of main (B-M).** The bits file computes three texts and hashes each by 64-bit FNV-1a over its UTF-8 bytes:
  `H_S` the shell sector under (a), P1 to P8 then the full rotor; `H_W` W under (b), P1 to P8; `H_F` W under (c), the full
  rotor. It holds one record per platform: Windows x86_64 (`cfg!(all(windows, target_arch = "x86_64"))`) and Linux x86_64
  (`cfg!(all(target_os = "linux", target_arch = "x86_64"))`); on any other platform it prints and does not compare (ADR
  0792 amendment 1: the mathematical library differs between platforms, and `sin`, `cos` build every set-up here). The
  records are made on the library of main before any library change: commit 2 is the bits file alone on top of this
  registration, run twice on Windows (this host, release) and twice on Linux (the image of ADR 0150, `docker run --rm
  --init --cpuset-cpus 0-3 --memory 16g`, the sources a `git archive` of that tree, the volume
  `rcfd-examples-cargo-registry` for the crates, `--offline`, a target folder inside the container), with the records
  empty; the printed hashes, equal between the two runs of a platform, are written into the file and committed. If the
  container cannot build the test, no Linux record is registered, that is said, and Linux prints: not a miss.
- **T1 restated** (`r_t1_...`). NC1, NC2, A3, N-X: the bodies of T1 copied. NC4 as acceptance: `full_rotor` of W under (b)
  and under (a): `Ok`, and equal to `full_rotor` of W under (c). N-W as acceptance: `whirl_modes(1, 30.0)` of W under (a),
  `RotatingOptions` as T1 (4 modes, `coriolis(true)`): `Ok` and equal to the same under (b).
- **T2 restated** (`r_t2_...`). NC1 as acceptance: `full_rotor` of W under (b): `Ok` and equal to it under (c). NC2: the
  body copied.
- **T3 restated** (`r_t3_...`). NC1 as acceptance: `full_rotor` of the arc of T3 (nodes `point(0, 0.5, 0)`, `point(1, ..)`,
  `point(2, ..)` of the ADR 0556 frustum, 30 degrees apart, two Beam2 elements, orientation the axis, `low` node 0, `high` node 2, the shells option): `Ok`, the full
  mesh has `N = 6` blocks, `rotor.block(j, 0) = j`, each of kind Beam2 with 2 elements and the section's bits; copy 0's
  orientation bit-equal to the axis, copy `j >= 1`'s within `1e-15` per component of `apply(rotation_matrix(axis, 2 pi j /
  6), axis)` computed in the test. NC2, NC3, NC4: the bodies copied.
- **N-F.** The Hex8 twin sector of ADR 0556 B3 (the `solid_sector` of `sa_rotating_cyclic_shells.rs`) with a disjoint Beam2
  element (as NC2 of T3 with a Shell4 element), the default options: `full_rotor` refused `UnsupportedKind { kind: "Beam2",
  operation: "full_rotor" }`.

## Checks

Exact limits are the tolerance table's row "Bit-identity of default output". The `1e-15` limit of T3's orientations is
ADR 0601 O1's. Where the estimate equals an exact limit the margin is written 1.

| # | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| E-W | The lift on every whirl path: W, P1 to P8 under (a), (b) and (c) | every result `Ok`; the texts of (a), (b), (c) equal | the maintainer's answer of 2026-11-21 (ADR 0744); tolerance table, "Bit-identity of default output"; D1 | holds | 1 | code verification |
| E-F | The lift in `full_rotor`: W, the full rotor and its expansions under (a), (b) and (c) | every result `Ok`; the texts equal | as E-W; D2 | holds | 1 | code verification |
| E-WS | The options on a model without beams change no bit: the shell sector, P1 to P8 under (a), (b), (c) | texts equal | the brief (lead's decision); tolerance table, "Bit-identity of default output"; D1 case 2 | equal | 1 | unchanged default output |
| E-FS | The same, the full rotor and its expansions | texts equal | as E-WS; D2 case 2 | equal | 1 | unchanged default output |
| B-M | Against the record of main: `H_S`, `H_W`, `H_F` on Windows x86_64 and on Linux x86_64 | equal to the record of the platform | rule 3; the brief; tolerance table, "Bit-identity of default output" | equal | 1 | unchanged default output |
| B-D | The record is reproducible: the two record runs of main on each platform | the three hashes equal | tolerance table, "Bit-identity of default output" | equal | 1 | unchanged default output |
| A-T1 | T1 restated: NC4 (both option sets) and N-W as acceptance | `Ok`, equal as stated | D1, D2 | holds | 1 | code verification |
| N-T1 | T1 restated: NC1, NC2, A3, N-X as in T1 | each as in T1 | ADR 0744 (its N-0595, A3, N-X rows) | as there | 1 | unchanged default output |
| A-T2 | T2 restated: NC1 as acceptance | `Ok`, equal to (c) | D2 | holds | 1 | code verification |
| N-T2 | T2 restated: NC2 as in T2 | refused, key `cyclic`, naming ADR 0601 | ADR 0601 NC2 | as there | 1 | code verification (negative control) |
| A-T3 | T3 restated: NC1 as acceptance (structure; copy 0 bits; copies `j >= 1`) | `Ok`; structure as stated; bits; `<= 1e-15` | D2; ADR 0601 O1 | holds; 0 | 1 | code verification |
| N-T3 | T3 restated: NC2, NC3, NC4 as in T3 | each as in T3 | ADR 0561 (NC2 to NC4) | as there | 1 | code verification (negative control) |
| N-F | `full_rotor` of a Hex8 sector with a Beam2 element, default options | refused `UnsupportedKind { kind: "Beam2", operation: "full_rotor" }` | D2 case 3 | as the limit | 1 | code verification (negative control) |
| R-T | The retired records, run once with `--ignored` after the change: T1 fails with exactly its lines "NC4 full_rotor, beams option", "NC4 full_rotor, shells only" and "N-W whirl_modes k = 1, shells only" missed; T2 with exactly "NC1 full_rotor without full_rotor_beams"; T3 with exactly "NC1 full_rotor of Beam2 with the shells option" | fail there and nowhere else | D5 | fail there | 1 | code verification (negative control) |
| B1 | Defaults: the existing test files, unedited but for the three ignore lines, pass: every test file of `rcfd-fem` whose name holds cyclic, rotating, whirl, beam or full_rotor (56 files at dcb4a1df), and `sa_free_sector_prestress_f680`, `sa_tie`, `sa_tie_default_bits`, `sa_gyro_default_bits`, `sa_gyro_sector_default_bits` | 0 failures | rule 7; tolerance table, "Bit-identity of default output" | 0 | 1 | unchanged default output |
| B2 | `cargo test -p rcfd-fem --release --lib` | 0 failures | rule 7 | 0 | 1 | unchanged default output |
| B3 | `cargo test -p rcfd-api --release --test baseline_hashes` (`cyclic.rs` and `rotating/whirl.rs` are shared solver files) | pass | rule 3 | pass | 1 | unchanged default output |

No row is of kind validation: the acceptance of F-672 claims agreement with the full ring computed by the same code, not
with a measured case.

### Negative controls and planned mutations

Controls: N-T2, N-T3, N-F (what stays refused), R-T (the retired records fail for the registered reason and no other).
Mutations, each on a scratch copy of the tree after the change, with its own target directory, the named tests run once:

| Id | Mutation | Must be caught by |
|---|---|---|
| Y1 (committed, rule 16) | the `beams` option matters again on the whirl paths: `RotatingPart::coriolis` reads `beams_opted_in` again (restored) in place of `shells_opted_in`. Patch `crates/rcfd-fem/tests/sa_beam_opt_ins_default_f672.mutation.patch` | E-W (every path refused under (a)), A-T1 (N-W) |
| Y2 | the `full_rotor_beams` option matters again in `full_rotor`: `split_beams = full_rotor_beams_opted_in(model)` (restored) | E-F, A-T1 (NC4), A-T2, A-T3 |
| Y3 | the refusal kept on one path: `RotatingSector::split_campbell` refuses a model with a Beam2 block unless a cyclic constraint has the beams option (`NotYetSupported` naming `beam::SOLID_ROUTINES`); the other paths lifted | E-W (P5 only) |
| Y4 | a beam sector given the old default: `full_rotor` takes Beam2 under the shells option but splits only with `full_rotor_beams`, so under (a) and (b) every copy of a Beam2 block keeps the one block and the orientation `v` of the copy branch | E-F (texts of (a) and (c) differ), A-T1, A-T2, A-T3 (orientations) |

## Checks of rule 7, and how they are run

All in the release profile, with `CARGO_TARGET_DIR` under the scratch folder, `--test-threads 1` not imposed, output to
files in the scratch folder. The registered run of the new files and of the named files is made once after the change.
Then `cargo fmt -p rcfd-fem --check` (ADR 0744 found that it stops in a worktree with "The filename or extension is too
long (os error 206)"; if it does, its output is shown and `rustfmt --edition 2021 --check` on the changed and added Rust
files takes its place), `cargo clippy -p rcfd-fem --all-targets -- -D warnings`, `cargo test -p rcfd-cli --release --test
docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`. `npm test` is not
run: `apps/studio-web` is not touched.

## Size

Size (agent): done; 0; 2; 1; 320000

(Two heavy runs: the two Linux containers, the record of main and the run after the change. One question: question 1.)

## Decision rule

- **done**: every row E-W to B3 is met in its one registered run, every mutation is caught by at least one check, the named
  existing tests pass with only T1, T2 and T3 made ignored records, and models without beams are bit-identical to main
  (E-WS, E-FS, B-M, B1 to B3).
- **partial**: a row is missed (recorded by rule 2) or not run, or a mutation escapes. The Linux part of B-M not run
  because the container cannot build the test is recorded as not run, and the decision is partial.
- **todo**: the lift cannot be made without changing the result of a call that was accepted before, or without an edit of
  `baseline_hashes.rs`, a hash table or a golden file.

## What is not covered

- The refusals of option combinations (D4, question 1).
- A `RotatingPart` of a model without a cyclic constraint keeps `RotatingOptions::beams`.
- The agreement of the whirl paths and of the full rotor of a beam sector under the shells option alone with the ring is
  carried over by bit equality from ADR 0598 and ADR 0601; it is not computed again.
- Line loads on the beams of a sector (not refused, not checked, as in ADR 0595); beam ends on extended boundaries (refused,
  N-X).
- Any platform but Windows x86_64 and Linux x86_64; on Linux only the bits file is run.
- The case schema: no case key reaches `CyclicOptions`.

## Open questions for the maintainer

1. **The refusals of an option combination.** After this registration `CyclicOptions::beams` and `full_rotor_beams` have
   no effect on any result, but three refusals still read them: `beams` without `shells` and `full_rotor_beams` without
   `beams` (`add_constraint`), and `beams` with `interpolation` or `axis_nodes`, also on a model without beams
   (`check_beams_option`). A call that sets an option there is refused where the same call without it would not be.
   Options: (a) keep the three refusals (taken here, because lifting them changes a default and retires NC1 and NC2 of ADR
   0595 and NC2 of ADR 0601, which the answer of 2026-11-21 does not name); (b) lift them in a further registration, so that
   the options are wholly inert; (c) deprecate the two options and remove them at the next change of the library's API.
   Recommendation: (b), with (c) when the API is next changed: an option that has no effect but can still cause a refusal is
   harder to explain than either a working option or none.

## Amendment 1 (2026-11-25, before the Linux record and before any library change is committed or run)

The Linux record of B-M and B-D as registered cannot be made: in the image of ADR 0150 with the volume
`rcfd-examples-cargo-registry`, `cargo test --release --locked --offline -p rcfd-fem --test
sa_beam_opt_ins_default_f672_bits` stops before compiling with "failed to select a version for the requirement `wat = "^1"`
(locked to 1.263.0)" (twice; logs `linux/bm_record_linux_run1.log`, `run2.log` in the scratch folder). No test ran. The
volumes `rcfd-cargo-cache`, `rcfd-hc-cargo`, `rcfd-w48l-registry` and `rcfd-studio-linux-registry` lack that crate too
(listed). Corrected set-up, for the record of main and for the run after the change alike: the same image and `docker run`
limits, no volume; in the container `cargo fetch --locked` downloads the crates of `Cargo.lock` from crates.io into the
container's own file system, then the test runs as registered with `--locked --offline`. Nothing else changes: no check,
limit, source or estimate, and the Windows record (two runs of main on d0c7a0cb plus the bits file, made before this
amendment) is not affected by it.

Observed in the two Windows record runs (a fact of main, not a result of this registration): the three hashes are equal
between the two runs, and in the texts of `H_W` every path is `Ok` except P7, `whirl_stability` of W under (b), which is an
error of the solver (`Err(Solver { solver: "gy`...). E-W's limit "every result `Ok`" stands as registered.
