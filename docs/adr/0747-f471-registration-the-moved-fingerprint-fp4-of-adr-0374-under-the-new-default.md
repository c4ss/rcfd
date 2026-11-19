# ADR 0747: F-471 registration: the moved fingerprint FP4 of ADR 0374 N0 under the new default of the coarsest dense LU

Status: **REGISTERED 2026-11-19, written before any code of this registration of F-471 and before any run of its checks.**
Before this text no library or test code of this registration was written and no test of the tree was built or run. What
was done is reading, searches of the tree with `grep`, reading two logs of the earlier work, and one compile of a four-line
toy file outside the tree (below). Results go in addenda at the end; nothing above them is changed after a run.

This is a further registration of roadmap item F-471 under rule 13: it adds no acceptance clause. It answers the brief
`docs/adr/briefs/f471-fp4-move.md` (written on main after the branch was made; read there, not copied) and follows
`docs/process/agent-rules.md`, version 3. It continues
[ADR 0743](0743-f471-registration-the-corrected-order-of-the-coarsest-dense-lu-as-the-default.md) on its branch
`f471-coarse-lu-default` from the commit 5c8a18ff; no commit of that work is rewritten. Scratch:
`w49i-work/f471-coarse-lu-default-scratch`.

## What is asked

ADR 0743 made the corrected order of the coarsest dense LU solve the default of the scalar AMG and stopped, by its decision
rule, at a registered fingerprint that moved: FP4 of [ADR 0374](0374-f363-registration-immersed-flux-with-one-piso-corrector.md)
N0 (`rcfd-fv` `immersed_single_corrector_f363::n0_fingerprints_of_the_defaults`), registered `0x279c4c4ed4cf89be`, with the
new default `0xda894fae1d52a3ad`. The maintainer answered question 1 of ADR 0743 on 2026-11-19 with "Approve the move"
(option (a); the answer is appended to ADR 0743 in the commit of this registration, as the brief asks). The approval covers
FP4 of ADR 0374 N0 and nothing else: no approval in advance of further fingerprints was given.

The work, from the brief:

1. The diagnosis that addendum 1 of ADR 0743 gives as unregistered is registered here and becomes a running test: under the
   old order the set-up of FP4 gives the registered value bit for bit, under the new default the new value, and one named
   factorisation takes the other interchange.
2. `n0_fingerprints_of_the_defaults` is retired as an ignored record, its body untouched, and restated in a new file.
3. The four files ADR 0743 left out are run under the new default, each once: `immersed_solid_f219`,
   `immersed_unsteady_force_f346`, `immersed_piso_stop_f355`, `vof_waves`.
4. Row A3 of ADR 0743 is repeated in full.

The acceptance of F-471 in `docs/roadmap.json` is quoted in ADR 0743 and binds unchanged. Its third clause is the one this
registration completes: "the rcfd-linalg, rcfd-fv and rcfd-api tests and the baseline hashes are run, and any changed hash is
reported to the maintainer, not re-recorded", in the reading ADR 0743 fixed (the hash tests and the test files that name the
scalar AMG, by name).

## What was read

- The brief; `docs/process/agent-rules.md` (version 3); `docs/process/tolerances.md` (the rows "Bit-identity of default
  output" and "Linear and eigenvalue solver residual"); `scripts/process_report.py` lines 432 to 489 (what it reads of a
  registration); `scripts/gate_limits.tsv`.
- ADR 0743 whole, with its addendum 1. ADR 0374: "The runs", "Registered checks", the mutations, the decision rule, and the
  row N0 of its addendum 1 (the measured values `dd07702a17b9a36f`, `e4d227de316e6de4`, (`3ff4a94abc88ca99`, 11855),
  (`3fb06aeaae8972f1`, 11855), `279c4c4ed4cf89be`).
- `crates/rcfd-fv/tests/immersed_single_corrector_f363.rs` whole; `crates/rcfd-linalg/src/amg.rs` lines 1 to 330 and 420
  to 462 (`level_sizes`, `coarse_interchanges`); `crates/rcfd-linalg/src/backend.rs` lines 58 to 83 (`LinearOptions`);
  `crates/rcfd-fv/tests/amg_default_fluid_adr0743.rs` lines 320 to 330 and 425 to 488 (how P1 of ADR 0743 captures the
  pressure systems of a run and reads the interchanges of each); `PressureCapture` in `crates/rcfd-fv/src/pimple.rs`.
- The two logs of the unregistered diagnosis, `logs/fp4-default.log` and `logs/fp4-old.log` of the scratch folder: in both
  runs of `Sa` one line `DenseLu n 90 interchanges [35]` after step 3 and no other; the old-order run prints FP4
  `279c4c4ed4cf89be`, the default run `da894fae1d52a3ad`; FP1 to FP3 are the registered values in both.
- The times of one full gate (`w49i-work/f849-gate-test-time-limit-scratch/runs/44276505780/.../times.tsv`, tests run one
  at a time): the longest tests of three of the four files are `a2t_c1_two_d_two_on_t` 2247 s and `a2_two_d_two_on_m2`
  963 s (`immersed_unsteady_force_f346`), `w2b_control_long_window` 607 s and `w2b_beach_long_window` 594 s (`vof_waves`);
  the sums per file are those of ADR 0743 (3350 s, 3308 s, 2066 s, 1543 s).

**The toy compile.** With `rustc 1.99.0`, a test function that carries `#[ignore = "..."]` and
`#[cfg_attr(debug_assertions, ignore = "...")]` together compiles with the warning `unused attribute` (lint
`unused_attributes`) when debug assertions are on, and without a warning when they are off. The file is
`toy0747/t.rs` of the scratch folder; it holds no code of the tree. `cargo clippy --all-targets -- -D warnings` builds the
test files with debug assertions on, so a bare second `#[ignore]` on `n0_fingerprints_of_the_defaults` would fail the
clippy check of rule 7. This fixes the form of the retiring line (D1).

## Readings, registered

**R1. Why FP4 moves and FP1 to FP3 do not** (from ADR 0743 R1 and R2, which rest on the derivation of ADR 0369). The two
orders of `DenseLu::solve` perform different operations only when the coarsest factorisation interchanges rows at a step
`k >= 1`; the factorisation, the hierarchy and the smoothers are the same bits under either order. A run is therefore the
same bits under both orders up to and including the last step before the first pressure solve whose coarsest factorisation
has such an interchange, and no run without such a factorisation moves at all. No source other than ADR 0743 and ADR 0369
is used; no algorithm is added.

**R2. What the unregistered diagnosis measured** (ADR 0743, addendum 1, "The changed fingerprint", and the two logs): in
the `Sa` run of FP4 (the set-up R of ADR 0254 on M2, one PISO corrector, `flux_weight_check` on, 20 steps of `dt = 1e-3`)
exactly one coarsest factorisation interchanges at a step `k >= 1`: the pressure solve of step 3, coarsest 90 unknowns, the
interchange at step 35. No factorisation of the F run (FP1) or of the B run (FP2) does. These are the expected values of
F3 below; they were measured with an `eprintln` on a scratch copy and are now measured by a test through the public
diagnostics.

**R3. Consequence of R1 and R2, registered as the prediction of F4:** the old-order `Sa` run and the default `Sa` run have
bit-equal `u` and `p` after steps 1 and 2 and differ after step 3.

## Design

No library file changes. One line is added to one existing test file, and one test file, one mutation patch and this ADR
are new.

**D1. The retired record.** In `crates/rcfd-fv/tests/immersed_single_corrector_f363.rs` one line is added above
`fn n0_fingerprints_of_the_defaults`, next to its existing `#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]`:

`#[cfg_attr(not(debug_assertions), ignore = "retired record (ADR 0747, F-471): FP4 moved with the default of ADR 0743, approved by the maintainer on 2026-11-19; restated in immersed_single_corrector_fp_adr0747.rs")]`

No other character of that file changes; the body, the registered values and the existing attribute stay. The brief names
the line as `#[ignore = "..."]`. The `cfg_attr(not(debug_assertions), ...)` form is that attribute in every build in which
the test is not ignored already: in a release build (where the gate runs it) the new line ignores it with the reason that
names ADR 0747, in a debug build the existing line ignores it as before, and the two are never active together, which the
toy compile shows is needed for clippy to stay clean. Taken on the maintainer's standing instruction of 2026-11-11 (take
the recommendation); it changes no result. If `cargo fmt` breaks the line, the attribute is committed as `cargo fmt` writes
it.

**D2. The new test file** `crates/rcfd-fv/tests/immersed_single_corrector_fp_adr0747.rs` (header: ADR 0747, F-471; it
restates N0 of ADR 0374, F-363). The DFG helpers, the forcings, `r_solver`, `fnv`, `state_hash`, `fp_run` and `fp_lag` are
copies of those of `immersed_single_corrector_f363.rs`, with one addition: each run takes a flag `old` that sets
`solver.settings.linear.amg_coarse_lu_interleaved = true` (the way back of ADR 0743 D2) before the first step or the lag
trial. The four tests carry `#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]` as the test they restate
does; each takes seconds in release (the retired test took 6.3 s).

- `f1_fingerprints_of_the_defaults_restated`: the body of N0 under the default, with FP1, FP2 and both FP3 values as
  registered by ADR 0374 and FP4 `0xda894fae1d52a3ad`.
- `f2_old_order_gives_the_registered_fingerprints`: the body of N0 with `old` set in every run: all five values as
  registered by ADR 0374, FP4 `0x279c4c4ed4cf89be`.
- `f3_one_factorisation_takes_the_other_interchange`: the three 20-step runs under the default with
  `pressure_capture = Some(..)`; for every captured pressure system the hierarchy is built again from the capture
  (`pressure_system_src`, `Amg::new` with the default settings, as P1 of ADR 0743) and `level_sizes()` and
  `coarse_interchanges()` are read. The systems with an interchange at a step `k >= 1` are listed as (step, corrector,
  coarsest size, the interchange steps `>= 1`). The hash of the captured `Sa` run is compared with the new FP4, so that
  the run that was inspected is the run of the fingerprint.
- `f4_the_two_orders_part_at_step_3`: two `Sa` solvers, old order and default, stepped together for 20 steps; after each
  step the number of values of `u` and `p` whose bits differ is printed; asserted after steps 1, 2, 3 and 20.

**D3. Nothing else is edited.** ADR 0374 is not edited (its N0 row stays the record of the old default; the lead may add a
pointer). No registered value of any other test is touched. If one moves, the work stops (decision rule).

## The runs of the existing files

- **The four long files**, each once, in release, `RAYON_NUM_THREADS` 4, as `cargo test --release -p rcfd-fv --test <file>`,
  unedited, their ignored tests not run, at most two files at a time, in the background, on the code commit of this
  registration. The wall time of each file is recorded, and for each test the time from the start of the file's run to the
  line that reports it (an upper bound of the test's own time, since a file's tests run on parallel threads). Two of their
  tests have a registered limit in the full gate (ADR 0740, `scripts/gate_limits.tsv`: 10 800 s each for
  `immersed_solid_f219::a3_two_d_two_strouhal_number` and `immersed_unsteady_force_f346::a2t_c1_two_d_two_on_t`, and
  `scripts/gate_limit_check.py` asks for at most half of it, 5400 s); their times are reported against that. The gate
  measures a test alone, so the times here are a report and have no limit of this registration.
- **The repeat of A3 of ADR 0743**: every other file of its list ("The test files that are run, and why each"), by name,
  in release, unedited, on the same commit, each once: the three `--lib` runs, `rcfd-linalg` `amg_coarse_lu_f474` and
  `amg_contrast`, the 30 short `rcfd-fv` files (with `immersed_single_corrector_f363`, which now has 3 tests that run and
  one more ignored), the 7 `rcfd-api` files, and the two new files of ADR 0743.

## Checks

The acceptance clause first (A3, with A4 and A5 as its hash tests). Exact limits are from the tolerance table's row
"Bit-identity of default output" unless a source is given; where the expected value equals an exact limit the margin is
written 1.

| # | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| A3 | Clause "the rcfd-linalg, rcfd-fv and rcfd-api tests ... are run": every file of the list of ADR 0743, unedited but for the one line of D1, passes on the code commit of this registration, the four long files among them (rows L1 to L4) | 0 failures | acceptance of F-471; rule 7; tolerance table, bit-identity | 0 | 1 | unchanged default output |
| A4 | Clause "the baseline hashes are run": `cargo test -p rcfd-api --release --test baseline_hashes`, unedited | every registered hash equal (the test passes) | rule 3 | equal (ADR 0743 A4) | 1 | unchanged default output |
| A5 | The case hash tests, unedited: `rcfd-api` `case_hashes_w48u`, `w48v_case_hashes`; `rcfd-fv` `varden_ideal_hash` | every registered hash equal (the tests pass) | rule 3 | equal (ADR 0743 A5) | 1 | unchanged default output |
| L1 | `immersed_solid_f219` under the new default, once | 0 failures; no registered value moved | acceptance of F-471; tolerance table, bit-identity | 0 (not known: ADR 0743 R3) | 1 | unchanged default output |
| L2 | `immersed_unsteady_force_f346` under the new default, once | 0 failures; no registered value moved | acceptance of F-471; tolerance table, bit-identity | 0 (not known: ADR 0743 R3) | 1 | unchanged default output |
| L3 | `immersed_piso_stop_f355` under the new default, once | 0 failures; no registered value moved | acceptance of F-471; tolerance table, bit-identity | 0 (not known: ADR 0743 R3) | 1 | unchanged default output |
| L4 | `vof_waves` under the new default, once | 0 failures; no registered value moved | acceptance of F-471; tolerance table, bit-identity | 0 (not known: ADR 0743 R3) | 1 | unchanged default output |
| F1 | N0 restated under the default: FP1 `0xdd07702a17b9a36f`, FP2 `0xe4d227de316e6de4`, FP3 (`0x3ff4a94abc88ca99`, 11855) and (`0x3fb06aeaae8972f1`, 11855), FP4 `0xda894fae1d52a3ad` | bit for bit | ADR 0374 N0 (FP1 to FP3, its addendum 1); ADR 0743 addendum 1 with the maintainer's answer of 2026-11-19 (FP4) | equal | 1 | unchanged default output |
| F2 | N0 under the old order (`linear.amg_coarse_lu_interleaved`): FP1 to FP3 as in F1, FP4 `0x279c4c4ed4cf89be` | bit for bit | ADR 0374 N0 (the registered old result) | equal (`fp4-old.log`) | 1 | unchanged default output (under the old order) |
| F3 | Which factorisation: over the captured pressure systems of the default runs, the systems whose coarsest factorisation interchanges at a step `k >= 1` | F (20 systems): none; B with two correctors (40 systems): none; `Sa` (20 systems): exactly one, at step 3, corrector 0, coarsest 90 unknowns, interchange steps `>= 1` equal to `[35]`; the hash of that `Sa` run equal to the FP4 of F1 | ADR 0743 addendum 1, "The changed fingerprint" (measured on its scratch copy); bit-identity | as stated | 1 | code verification |
| F4 | The two orders agree before that solve: values of `u` and `p` of the `Sa` run that differ in bits between the old order and the default after step 1 and after step 2 | 0 and 0 | R1 (ADR 0743 R1 and R2); bit-identity | 0 | 1 | code verification |
| F4a | Negative control of F4 and F2: the same count after step 3 and after step 20 | at least 1 each | R1; bit-identity, the other way round | of the order of the 73 216 values (18 304 cells, three velocity components and the pressure; the z velocity may stay equal) | 1e4 | code verification (negative control) |
| D1 | The retired record, run once with `--ignored` on the code commit | fails at its assertion "FP4" with the left value `0xda894fae1d52a3ad` (15747205181312508845) and reaches that assertion, so FP1 to FP3 hold | ADR 0743 addendum 1 (the failure as measured there) | as stated | 1 | code verification (negative control) |

No row is of kind validation: the acceptance of F-471 claims no agreement with a measured case.

### Negative controls and planned mutations

F4a and D1 are the negative controls; the F and B runs of F3 are the controls of its `Sa` run. Mutations, each alone on the
scratch copy of the code (the tree of ADR 0743's mutations with the new test file copied in, its own target directory), by
an exact text replacement checked to occur once, the named test run once in release, the file restored and compared byte
for byte:

| Mutation | Must fail |
|---|---|
| M1 (committed, rule 16): `Default` of `AmgSettings` back to `coarse_lu_interchanges_first: false`. Patch `crates/rcfd-fv/tests/immersed_single_corrector_fp_adr0747.mutation.patch` | F1 (at FP4) |
| M2: `LinearOptions::amg_settings` returns the default settings always (the way back cut) | F2 (at FP4) |
| M3: `pressure.rs` line 484 calls `pm.solve_amg` (the way back cut in the plain pressure path) | F2 (at FP4) |

The existing files are not run under mutations.

## Size

Size (agent): done; 0; 4; 0; 200000

The four heavy runs are the four long files (25 to 55 minutes each in the gate log read above, one test at a time).

## Decision rule

- **done**: A3, A4, A5, L1 to L4, F1 to F4a and D1 are met; no registered hash, fingerprint or recorded value other than
  FP4 of ADR 0374 N0 moved; the three mutations are caught by the tests named for them without an amendment after a run;
  `fmt` and `clippy` are clean.
- **partial**: a row is missed or not run (recorded by rule 2), or a mutation is not caught.
- **A further moved value stops the work**: if a test of the four long files or of the repeated list fails, or a
  registered hash, fingerprint or recorded value other than FP4 moves, nothing is edited and nothing is re-recorded; the
  file, the test and both values are reported, with the evidence under the old order where it can be given (on the scratch
  copy, disclosed as unregistered); the outcome is partial and the question goes to the maintainer under the fixed
  heading. What is already running is let finish.
- **todo** is not an outcome of this registration.

## What is not covered

- The test files that reach a scalar AMG without naming it (ADR 0743, "What is not covered"): the lead's full gate.
- The time limits of ADR 0740 are checked by the gate, one test at a time; the times here are a report.
- The lag estimates FP3 are not inspected for interchanges (a lag trial captures no pressure system); F1 and F2 show that
  they are the same bits under both orders.
- Whether results that are not registered moved in the four long files beyond what their own assertions show.

## Open questions

None is open at registration. The form of the retiring line (D1) is taken on the standing instruction and changes no result.
