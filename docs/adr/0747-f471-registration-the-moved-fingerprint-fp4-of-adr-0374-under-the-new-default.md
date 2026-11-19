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

## Amendment 1 (2026-11-19): the mutation M1 was defective; a corrected mutation M1a, registered before its run

Written after the run of the new test file on the code commit 5536aa90 (4 passed) and after the runs of M1, M2 and M3 on
the scratch copy, and before any run of M1a. No test, no limit and no registered value changes.

**What happened.** M1 (the `Default` of `AmgSettings` back to `coarse_lu_interchanges_first: false`) was run once on the
scratch copy and was **not caught**: all four tests of `immersed_single_corrector_fp_adr0747` passed
(`logs/0747-mutation-M1.log`). M2 and M3 were caught by F2 as registered.

**Why.** The pressure solve of a `PimpleSolver` does not read that default. Since ADR 0743 D2 it takes its settings from
`LinearOptions::amg_settings()`, which writes `coarse_lu_interchanges_first: !amg_coarse_lu_interleaved` over the default
settings. With `amg_coarse_lu_interleaved` false the pressure AMG has the corrected order whatever the `Default` of
`AmgSettings` says, so M1 changes no operation of any run of this file. The defect is in the plan of the mutation, which
was copied from ADR 0743 (where M1 must fail tests that build an `Amg` from `AmgSettings::default()` directly, and does);
the registration of this ADR did not follow the call path. The committed patch of this file would not make it fail, so it
is replaced.

**M1a (committed, rule 16), in place of M1:** in `crates/rcfd-linalg/src/backend.rs`, `LinearOptions::amg_settings`
returns `coarse_lu_interchanges_first: false` always (the pressure solve of the flow solvers takes the interleaved order
of before ADR 0743 by default, which is what "the default switched back" means on this call path). It must fail **F1 at
FP4**, with the left value the old `0x279c4c4ed4cf89be`. The patch
`crates/rcfd-fv/tests/immersed_single_corrector_fp_adr0747.mutation.patch` is replaced by this mutation in the commit of
this amendment; its first line stays `# expect-fail: cargo test --release -p rcfd-fv --test
immersed_single_corrector_fp_adr0747`. M1a is run once on the scratch copy after that commit. The patch of ADR 0743
(`amg_coarse_lu_default_adr0743.mutation.patch`) is untouched and still fails its own file.

**Consequence for the decision.** The registered rule says done needs every mutation "caught by the tests named for them
without an amendment after a run". This amendment is written after a run, so by that rule the outcome of this registration
cannot be done on the mutations alone, whatever M1a gives; the addendum reports it so.

## Addendum 1: results (2026-11-19)

Built as registered. Commits: the registration aa64693e (with the maintainer's answer appended to ADR 0743), the code
5536aa90, amendment 1 with the corrected mutation patch 7f4ae935. All runs in the release profile, each registered run made
once, on Windows, `RAYON_NUM_THREADS` 4. Logs are in `w49i-work/f471-coarse-lu-default-scratch/logs` with the prefix
`0747-`. No limit and no registered value was changed. No library file changed. No registered hash, fingerprint or recorded
value other than FP4 of ADR 0374 N0 moved, and no test of the four long files failed.

### Results

| # | Check | Limit | Measured | Met |
|---|---|---|---|---|
| A3 | every file of the list of ADR 0743 passes | 0 failures | 0 failures in 46 test files and 3 `--lib` runs (list below) | yes |
| A4 | `baseline_hashes` | every hash equal | 3 passed, 1 ignored (as on main) | yes |
| A5 | `case_hashes_w48u`, `w48v_case_hashes`, `varden_ideal_hash` | every hash equal | 1 passed; 0 run (its one test is ignored on main); 1 passed | yes |
| L1 | `immersed_solid_f219` | 0 failures | 11 passed, 0 failed, 2 ignored; 1696 s | yes |
| L2 | `immersed_unsteady_force_f346` | 0 failures | 9 passed, 0 failed, 5 ignored; 1713 s | yes |
| L3 | `immersed_piso_stop_f355` | 0 failures | 15 passed, 0 failed, 2 ignored; 749 s | yes |
| L4 | `vof_waves` | 0 failures | 6 passed, 0 failed, 2 ignored; 435 s | yes |
| F1 | N0 under the default | FP1 to FP3 registered, FP4 `0xda894fae1d52a3ad` | equal (the test passed) | yes |
| F2 | N0 under the old order | all five registered, FP4 `0x279c4c4ed4cf89be` | equal (the test passed) | yes |
| F3 | which factorisation | F 20 systems, none; B 40 systems, none; `Sa` 20 systems, exactly (step 3, corrector 0, coarsest 90, `[35]`); the hash the FP4 of F1 | as stated (the test asserts each with equality and passed) | yes |
| F4 | differing values after steps 1 and 2 | 0 and 0 | 0 and 0 | yes |
| F4a | differing values after steps 3 and 20 | at least 1 each | at least 1 each; the counts were not read (see deviations) | yes |
| D1 | the retired record with `--ignored` | fails at "FP4", left `0xda894fae1d52a3ad` | fails at "FP4" (line 273), left 15747205181312508845; it prints FP1 `dd07702a17b9a36f`, FP2 `e4d227de316e6de4`, FP3 `3ff4a94abc88ca99`/11855 and `3fb06aeaae8972f1`/11855, FP4 `da894fae1d52a3ad` | yes |

The new file `immersed_single_corrector_fp_adr0747`: 4 passed in 6.8 s.

**The four long files, times** (seconds from the start of the file's run to the line that reports the test; a file's tests
run on parallel threads, two files ran at a time, and the short list ran beside them for part of the time):

- `immersed_solid_f219`, 1696 s: `a3_two_d_two_strouhal_number` 1696 s (gate limit 10 800 s, half of it 5400 s),
  `a1_a2_two_d_one_drag_and_lift_on_m3` 895 s, `two_d_one_plain_forcing_printed` 74 s, the other eight within 10 s.
- `immersed_unsteady_force_f346`, 1713 s: `a2t_c1_two_d_two_on_t` 1713 s (gate limit 10 800 s, half of it 5400 s),
  `a2_two_d_two_on_m2` 773 s, `a3a_steady_two_d_one_on_m2` 71 s, the other six within 19 s.
- `immersed_piso_stop_f355`, 749 s: `a_two_d_two_on_m2_strouhal` 749 s, `d1_to_d8_variants` 296 s,
  `sb_bounded_runs_the_variants_that_stopped` 135 s, `d9_located_cause` 98 s, the others within 54 s.
- `vof_waves`, 435 s: `w2b_beach_long_window` and `w2b_control_long_window` 435 s, `w1_w3_generation_and_beach` 226 s,
  the other three within 23 s.

**A3, the repeated list** (passed / ignored): `rcfd-linalg` `--lib` 20/1, `amg_coarse_lu_f474` 5/2, `amg_contrast` 2/0,
`amg_coarse_lu_default_adr0743` 4/0; `rcfd-fv` `--lib` 115/6, `consistent_correctors_f301` 12/4,
`consistent_defaults_f301` 6/0, `consistent_extensions_f322` 8/0, `consistent_periodic_defaults_f367` 1/0,
`consistent_periodic_f367` 13/2, `cylinder` 2/1, `immersed_bounded_stop_f466` 7/4, `immersed_dfg_convergence_f354` 6/12,
`immersed_early_state_start_f501` 3/2, `immersed_lag_estimate_f483` 5/7, `immersed_lockin_f731` 6/1,
`immersed_lockin_f750` 8/3, `immersed_lockin_f756` 8/2, `immersed_moving_f308` 7/2,
`immersed_multistep_linearisation_f494` 4/6, `immersed_single_corrector_f363` 3/8 (N0 now among the ignored),
`immersed_start_projection_f515` 4/2, `immersed_step_linearisation_f482` 3/6, `immersed_transverse_f707` 4/3,
`transient_buoyancy` 3/0, `vof_bubble_diagnosis_w41c` 1/20, `vof_bubble_refinement_w40d` 1/12, `vof_dambreak_amg` 1/0,
`vof_height_function_w42b` 2/2, `amg_definiteness_f474` 1/1, `coupled` 2/1, `gradient_pressure_f291` 3/0,
`varden_ideal_hash` 1/0, `sixdof_flow_f244` 8/1, `piso_thin_gap_f299` 18/2, `amg_default_fluid_adr0743` 2/6; `rcfd-api`
`--lib` 159/4, `baseline_hashes` 3/1, `case_hashes_w48u` 1/0, `w48v_case_hashes` 0/1,
`consistent_correctors_case_f301` 4/8, `w35b_lag_check` 6/0, `w48t_cylinder_spokes` 3/13, `vof_wiring_tree` 4/0; and the
four long files above.

### Mutations (each alone on the scratch copy, the test file once, the file restored byte for byte every time)

| Mutation | Registered to fail | Result |
|---|---|---|
| M1 (the `Default` of `AmgSettings` back to `false`) | F1 at FP4 | **not caught**: 4 passed. A defect of the plan, not of the code (amendment 1): the pressure path does not read that default |
| M1a (amendment 1, committed patch: `LinearOptions::amg_settings` returns the old order always) | F1 at FP4 | caught: F1 fails at "FP4" with left 2854240165307582910 (`0x279c4c4ed4cf89be`); F3 (the hash) and F4 fail as well |
| M2 (`LinearOptions::amg_settings` ignores the flag) | F2 at FP4 | caught: "FP4 under the old order", left 15747205181312508845; F4 fails as well |
| M3 (`pressure.rs` line 484 calls `pm.solve_amg`) | F2 at FP4 | caught: the same two failures |

### Checks of rule 7

`cargo fmt -p rcfd-fv --check` and `-p rcfd-linalg --check`: clean. `cargo clippy -p rcfd-fv -p rcfd-linalg --all-targets --
-D warnings`: clean. `cargo test -p rcfd-cli --release --test docs_site`: 2 passed. `python scripts/process_report.py
--check`: one failure, `docs/adr/index.md` not current (the lead's file).

Files changed since 5c8a18ff: `crates/rcfd-fv/tests/immersed_single_corrector_f363.rs` (one attribute added above
`n0_fingerprints_of_the_defaults`, lines 255 to 258 as `cargo fmt` writes it; nothing else); new
`crates/rcfd-fv/tests/immersed_single_corrector_fp_adr0747.rs` and its `.mutation.patch`; this ADR; the answer section
appended to ADR 0743. No shared solver file.

### Deviations and disclosures

- Amendment 1: M1 was not caught and was replaced by M1a after a run.
- The new test file was run without `--nocapture`, so the counts of F4a and the printed lists of F3 were not read; each
  registered run is made once, so it was not run again. F3 is asserted with equality, so its values are known; of F4a only
  "at least 1" is known.
- The registration (D1) wrote the retiring attribute on one line; `cargo fmt` breaks it over four lines, as the
  registration allowed. It is one attribute.
- The long files and the short list were built on 5536aa90 and partly ran after 7f4ae935 was committed; the two commits
  differ only in this ADR and in the patch file, and nothing was rebuilt. The summaries of the logs name 7f4ae935.
- The times of the long files are not those of a test run alone (as the gate measures them).

### Not run, not done

- The test files that reach a scalar AMG without naming it (registered as not covered; the lead's full gate).
- The committed patch was checked with `git apply --check` and M1a was run on the scratch copy by the same text
  replacement; the lead's push script itself was not run.

### Size

Size (agent): partial; 0; 4; 0; about 215000

### Decision

By the registered rule: **partial**, for one reason only: the rule asks that every mutation be caught "without an
amendment after a run", and M1 was not caught and was replaced by amendment 1 after its run. Everything else of "done" is
met: A3 to A5, L1 to L4, F1 to F4a and D1; no registered value other than FP4 moved; M1a, M2 and M3 are caught; `fmt` and
`clippy` are clean. The miss is diagnosed (a mutation that changes no operation on the call path under test) and concerns
no clause of the acceptance of F-471, all of which now have a passing row (ADR 0743 A1, A2, A4, A5; A3 here). Whether the
item is done is the lead's decision.

## Open questions for the maintainer

None.

## For the lead: roadmap entries

Status proposed for F-471: done, if the lead takes the miss of the mutation plan (amendment 1) as closed by M1a; partial
by the letter of the registered rule otherwise.

> ADR 0747, 2026-11-19, partial by its rule (one planned mutation was an equivalent mutant and was replaced by amendment 1), all checks met: N0 of ADR 0374 is retired as an ignored record and restated (FP1 to FP3 unchanged, FP4 0xda894fae1d52a3ad under the default and 0x279c4c4ed4cf89be under the old order); one factorisation takes the interchange (step 3 of the Sa run, coarsest 90, step 35) and the two orders agree in every bit before it; immersed_solid_f219 (11 passed, 1696 s), immersed_unsteady_force_f346 (9, 1713 s), immersed_piso_stop_f355 (15, 749 s) and vof_waves (6, 435 s) pass under the new default; the whole list of ADR 0743 passes; no other registered value moved.

> Gap: every clause of the acceptance has a passing row (ADR 0743 A1, A2, A4, A5; ADR 0747 A3). What remains is the lead's full gate over the test files that reach a scalar AMG without naming it (ADR 0743, "What is not covered"); a registered value that moves there is not covered by the approval of 2026-11-19 and goes to the maintainer.

No new item is proposed. Note proposed for F-363, one line:

> ADR 0747, 2026-11-19 (F-471): the fingerprint FP4 of ADR 0374 N0 moved with the corrected order of the coarsest dense LU (approved by the maintainer on 2026-11-19); n0_fingerprints_of_the_defaults is an ignored record and immersed_single_corrector_fp_adr0747.rs restates it with the old value reproduced under the old order.
