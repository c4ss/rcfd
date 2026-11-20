# ADR 0748: F-471 registration: the nine moved fingerprints of the overset solver under the new default of the coarsest dense LU

Status: **REGISTERED 2026-11-20, written before any code of this registration of F-471 and before any run of its checks.**
Before this text no library or test code of this registration was written, nothing of the tree was built and no test was
run. What was done is reading, searches of the tree with `grep`, a comparison of two test files with `diff`, and the
conversion of the decimal values of one gate log to hexadecimal. Results go in addenda at the end; nothing above them is
changed after a run.

This is a further registration of roadmap item F-471 under rule 13: it adds no acceptance clause. It answers the brief
`docs/adr/briefs/f471-overset-fingerprints.md` and follows `docs/process/agent-rules.md`, version 3. Parent records:
[ADR 0743](0743-f471-registration-the-corrected-order-of-the-coarsest-dense-lu-as-the-default.md) (the default and the way
back through the pressure solve) and
[ADR 0747](0747-f471-registration-the-moved-fingerprint-fp4-of-adr-0374-under-the-new-default.md) (the form of a restated
fingerprint). Read at main dbbd889e. Branch `f471-overset-fingerprints`, scratch
`w49i-work/f471-overset-fingerprints-scratch`.

## What is asked

The full gate of main at 252c9279 (run 44599067147, 2026-11-20) is red with ten failures. Nine are tests of the overset
solver that assert registered fingerprints or records of default results; the tenth is the first hash of
`cases/axial_fan_cgrid/axial_fan_cgrid.toml` in `rcfd-workspace` `shipped_store_f834` D1. The maintainer answered on
2026-11-20 "Approve the moves" for the nine tests, under a further registration that adds a way back to the old order for
the overset solver, shows each old value bit for bit under it, and restates the nine tests (the answer is appended to ADR
0743 and ADR 0747 in the commit of this registration, as the brief asks). Nothing else is approved.

The acceptance of F-471 in `docs/roadmap.json` is quoted in ADR 0743 and binds unchanged. This registration serves its
third clause ("any changed hash is reported to the maintainer, not re-recorded").

## What was read

- The brief; `docs/process/agent-rules.md` (version 3); `docs/process/tolerances.md` (the row "Bit-identity of default
  output"); `scripts/process_report.py` lines 432 to 489; ADR 0743 and ADR 0747 whole.
- The gate's records in `w49i-work/gate-252c9279`: `attribution.md` whole; of `test.log` the ten panics with their left and
  right values (lines 1841 to 1844 and 10738 to 12911); of `times.tsv` the rows of the overset files.
- `crates/rcfd-fv/src/overset.rs`: the imports, lines 870 to 882 (the refused settings), 945 to 965 (`PressureSystem`,
  `solve_pressure_rows`), 1885 to 1916 (which pressure solve a corrector takes), 1989 to 2032 (`overset_pressure`), 2080 to
  2176 (`prescribed_pressure`). `crates/rcfd-linalg/src/backend.rs` lines 58 to 83 (`LinearOptions`, `amg_settings`). Every
  place that builds a scalar AMG (`grep "Amg::new\|solve_amg\|amg_settings"` over `crates/*/src`).
- Of the eight test files of the nine tests, each failing test with every helper it calls:
  `overset_f335_records.rs` whole; `overset_f487.rs` lines 1 to 265; `overset_f472.rs` lines 1 to 262 (by `diff` against
  the former: the same helpers); `overset_motion_f442.rs` lines 1 to 143 and 400 to 497; `overset_f338.rs` lines 28 to 205
  and 600 to 770; `overset_f450.rs` lines 1 to 200 and 504 to 545; `overset_f368.rs` lines 1 to 324; `overset_f479.rs`
  lines 1 to 206 and 640 to 684. `immersed_single_corrector_fp_adr0747.rs` lines 1 to 120 and its mutation patch.
- `crates/rcfd-workspace/tests/shipped_store_f834.rs` whole; `cases/axial_fan_cgrid/axial_fan_cgrid.toml`;
  `crates/rcfd-api/src/sim.rs` line 2166 (`amg_pressure = n_cells > 2000`).

Not read whole, against the brief's reading list: `overset.rs`, `amg.rs` and the eight test files outside the lines named
above. The remainder of the test files are other checks of their items; they are run, not changed.

## Readings, registered

**R1. Every pressure solve of an overset step with a coupling builds a scalar AMG.** `overset_pressure` (line 2027) and
`prescribed_pressure` (line 2172) build `Amg::new(&a, AmgSettings::default())` in the first corrector of a step and use it
as the right preconditioner of GMRES(30) (`solve_pressure_rows`). This does not depend on `PimpleSettings::amg_pressure`,
which the overset solver refuses (line 878). So every overset run takes the coarsest dense LU solve, and by ADR 0743 R1 its
bits move exactly when a coarsest factorisation of the run interchanges rows at a step `k >= 1`. The statement of ADR 0743
D2 "No record of the old behaviour runs through them" is false for `overset.rs`. A solver without a coupling takes
`solve_pressure` with `linear: s.linear` and `amg: s.amg_pressure` (line 1903), which already has the way back of ADR 0743
and builds no AMG here (`amg_pressure` is refused).

**R2. What each of the nine tests compares.** Each compares a computation of this tree with constants stored in its own
file; none compares two computations of this tree. So no test of the nine has a side that the default reaches in a second
way, and none is a candidate for "both sides should still agree". The names "equals the base tree" and "bit identical" mean
the stored values of an earlier tree. The stored values and the values of the gate log (the left side of each panic,
converted to hexadecimal here) are in the table of "The nine tests".

**R3. Values the gate did not reach.** A test stops at its first failing assertion. Behind that assertion the gate
measured nothing in four tests: R2'c of `r2_prime...` (the run TR with `set_outer_iterations(1, 1, 1)`), the fourth check
of `r2e...` (TG_1, BDF2 then implicit Euler), the linear stencils of `b1...`, and the three later values of `a6...` (the
linear stencils, G, G with the opt-in set off). Arrays compared whole (`overset_f487`, `overset_f472`,
`overset_motion_f442`, `overset_f368`) are known in every element from the log. The restated default tests assert the
unreached values as registered (unchanged). If one of them moves, it is a moved value of one of the nine tests that the
gate did not show the maintainer: it is not restated by this registration without an amendment written before the
corrected run, it is reported with both values and with its value under the old order, and it goes to the maintainer under
the fixed heading; the outcome is then partial.

**R4. The registered value of the store hash.** The table of `shipped_store_f834.rs` has `0xe4539468a88f2746` for the first
hash of `axial_fan_cgrid`, which is the decimal 16452657040940017478 of the gate log's right side. The brief writes the
hexadecimal as `0xe453d1b2ca2a6c06`; that is not the number of the table or of the log, and the table's value is used
here. The measured value is 275158313255873701, `0x03d18f072e9caca5`, as the brief has it.

## The nine tests

Old: the constant of the test. New: the left value of the panic in `test.log` of the gate (run 44599067147). "=" marks an
element that the gate measured equal to the constant; "?" one it did not reach (R3).

| # | Test | Set-up | Old | New |
|---|---|---|---|---|
| T1 | `overset_f487::d1c_l_fingerprint_equals_the_base_tree` (ADR 0387 D1c) | V on B3d, 5 steps, corrections and L; linear, quadratic | `0x6820a06f1b6a1496`, `0x861ab174c9e7dbe2` | `0x0c3ec8cd8e210f07`, `0xec0a524a76ab219e` |
| T2 | `overset_f472::d1b_opt_in_fingerprint_equals_the_base_tree` (ADR 0382 D1b) | V on B3d, 5 steps, corrections; linear, quadratic | `0x6e6117e1b91851a9`, `0x69a719b86040e649` | `0x149540b75a11eb76`, `0x688bd9678efe83d4` |
| T3 | `overset_f335_records::r2b_taylor_green_level_1_record` (ADR 0238 R2b) | TG_1, 200 steps | `75ac2daf13879872` | `a126cd75e3d481d7` |
| T4 | `overset_f335_records::r2_prime_opted_in_with_one_outer_iteration_gives_the_records` (ADR 0238 R2') | U 80 steps; TG_1 200 steps; TR 20 steps; one outer iteration | `7aa14d1e2e2da052`; `75ac2daf13879872`; `fddb420dc287502a` | =; `a126cd75e3d481d7`; ? |
| T5 | `overset_motion_f442::f0_defaults_fingerprint_equals_the_base_tree` (ADR 0359 F0) | TG_1 20 steps default; with BDF2 and C; two connectivities | `0x3deeab3cfda1d8d0`, `0x4758f6c215970201`, `0xc931bf9474a97f49`, `0xac84b084267029d0` | `0xb66f8e865defb9eb`, `0x87e9c7c8539a0a98`, =, = |
| T6 | `overset_f338::r2e_implicit_euler_chosen_by_the_method_gives_the_records` (ADR 0241 R2e) | U twice; TG_1 twice | `7aa14d1e2e2da052` twice; `75ac2daf13879872` twice | =, =; `a126cd75e3d481d7`, ? |
| T7 | `overset_f450::b1_defaults_bit_identical` (ADR 0376 B1) | TG_1 moving, 40 steps; quadratic, linear | `0xe43a026a778e83b3`, `0x9c5ab1e5b21d5368` | `0x9f56741b853c367b`, ? |
| T8 | `overset_f368::d1_defaults_fingerprint_equals_the_base_tree` (ADR 0367 D1) | F1 to F8 on B3d | `0x7ed937a9e05bb338`, `0xbf99f3bc07e5fdff`, `0x31509ceb0d028a2f`, `0xcd2ef9ebd8dc9295`, `0xca08f2d7cb22ffe7`, `0xabcd93b859c1b26e`, `0xa4894813b5a6f2ed`, `0x8b2ebdb282e07af2` | `0x2979fa929db1666b`, `0x0cf8510ac96fd9bd`, `0xb13f7bbe80984a00`, `0x27516ddc91948f05`, =, `0x073bb14083b5cfdf`, =, = |
| T9 | `overset_f479::a6_without_the_opt_in_every_bit_is_kept` (ADR 0409 A6) | TG_1 moving, 40 steps: default quadratic, default linear, G, G set off | `0xe43a026a778e83b3`, `0x9c5ab1e5b21d5368`, `0xb2b43812eb166b04` twice | `0x9f56741b853c367b`, ?, ?, ? |

T3, T4 and T6 are one run in three files (TG_1, 200 steps); T7 and T9 share the run with the quadratic stencils.

## Design

**D1. The way back of the overset solver.** In `crates/rcfd-fv/src/overset.rs` the two AMG builds (lines 2027 and 2172)
become `Amg::new(&a, s.linear.amg_settings())`, with `s` the `PimpleSettings` both functions already take; the import of
`AmgSettings` is dropped if nothing else uses it. This is the form ADR 0743 D2 gave `pressure.rs`. With the default
`LinearOptions` `amg_settings()` returns the corrected order, the same settings as `AmgSettings::default()`, so no default
operation changes; `solver.settings.linear.amg_coarse_lu_interleaved = true` gives the old order. The refusal of line 879
reads `linear.device` only and is unchanged. No key of the case schema, no golden and no other library file changes. No
algorithm is added; the derivation of the two orders is ADR 0369's, as cited by ADR 0743 R1.

**D2. The coupled, potential and variable-density solvers are left as they are** (`coupled.rs` 735, `potential.rs` 99,
`varden.rs` 564 and 1516 keep the default settings and have no way back). The gate shows no moved record through them, so
there is no old value to reproduce, and the brief allows leaving them if the registration says so. Taken on the
maintainer's standing instruction of 2026-11-11 (take the recommendation). A record that moves through one of them later
needs the same one-line change in that file.

**D3. The nine retired records.** Each of the nine tests gets one line above its `fn`:
`#[ignore = "retired record (ADR 0748, F-471): moved with the default of ADR 0743, approved by the maintainer on 2026-11-20; restated in overset_fingerprints_adr0748.rs"]`.
None of the nine has an ignore attribute today, so the plain form is used. No other character of the eight files changes.

**D4. The new test file** `crates/rcfd-fv/tests/overset_fingerprints_adr0748.rs` (header: ADR 0748, F-471). It has one
module per source file (`f487`, `f472`, `f335`, `f442`, `f338`, `f450`, `f368`, `f479`), each holding copies of exactly the
helpers its test calls, taken from the source file by line range, with one addition: the copy of `settings` sets
`s.linear.amg_coarse_lu_interleaved` from a thread-local flag of the test file. A test sets the flag for the old order
before it builds its solvers and clears it afterwards; every solver of these helpers is built on the test's own thread.
Each of the nine tests is restated twice:

- `tN_..._default`: the body of the test under the default, with the values of the column "New" and every "=" and "?"
  element as registered (R3).
- `tN_..._old_order`: the same body with the flag set: every value as registered (the column "Old").

Tests that assert only on Windows in their source (`check`, `check_hash`: T3, T4, T6) keep that form. The whole file takes
about two minutes in release by the gate's times of the nine tests; no test of it is ignored.

**D5. The store hash of `axial_fan_cgrid`** is diagnosed and not touched: neither the test nor its table is edited. The
case is solved with `simplec` on a mesh of about 11 500 cells with a rotational periodic pair, so `rcfd-api` sets
`amg_pressure` (above 2000 cells) and the pressure solve goes through `pressure.rs`, which builds a scalar AMG. The
hypothesis is that the hash moved with the default of ADR 0743. It is tested on a scratch copy of the code commit (not in
the tree, since a case cannot ask for the old order): with the tree's scalar AMG put back on the old order everywhere
(`AmgSettings::default()` with `coarse_lu_interchanges_first: false` and `LinearOptions::amg_settings` returning `false`
always), `shipped_store_f834` D1, unedited, must pass (row S1). On the unedited code commit it must fail with exactly the
one changed value (row S0). If S1 does not pass, the cause is not F-471 alone and the six candidate merges are bisected
with this one test on scratch copies made with `git archive`; that bisect is then reported and is not a registered check.
If S1 passes the bisect is not made: S0 and S1 together show that on the tip, with every other merge in, the order of the
coarsest solve alone decides the hash.

## The runs of the existing files

On the code commit, in release, each once, unedited except for the nine attributes, their ignored tests not run:

- the 21 files `overset*` of `crates/rcfd-fv/tests`: `overset_f134`, `overset_f134_records`, `overset_f314`,
  `overset_f334`, `overset_f335`, `overset_f335_records`, `overset_f336`, `overset_f338`, `overset_f349`, `overset_f353`,
  `overset_f357`, `overset_f359`, `overset_f368`, `overset_f445`, `overset_f450`, `overset_f468`, `overset_f472`,
  `overset_f479`, `overset_f487`, `overset_f503`, `overset_motion_f442`;
- the two other test files that build an `OversetSolver`: `ale_bdf2_f345`, `bdf2_remainder_f344`;
- `cargo test -p rcfd-fv --release --lib`; `cargo test -p rcfd-api --release --test baseline_hashes` (rule 3: a shared
  solver file is changed); `cargo test -p rcfd-cli --release --test docs_site` (rule 7).

## Checks

Exact limits are from the tolerance table's row "Bit-identity of default output" unless a source is given; where the
expected value equals an exact limit the margin is written 1.

| # | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| W1 | T1 under the old order (`linear.amg_coarse_lu_interleaved`): both values of the column "Old" | bit for bit | ADR 0387 D1c (the registered old result); the brief | equal | 1 | unchanged default output (under the old order) |
| W2 | T2 under the old order: both values | bit for bit | ADR 0382 D1b; the brief | equal | 1 | unchanged default output (under the old order) |
| W3 | T3 under the old order: R2b | bit for bit | ADR 0238 R2b; the brief | equal | 1 | unchanged default output (under the old order) |
| W4 | T4 under the old order: R2'a, R2'b, R2'c | bit for bit | ADR 0238 R2'; the brief | equal | 1 | unchanged default output (under the old order) |
| W5 | T5 under the old order: the four values | bit for bit | ADR 0359 F0; the brief | equal | 1 | unchanged default output (under the old order) |
| W6 | T6 under the old order: the four checks | bit for bit | ADR 0241 R2e; the brief | equal | 1 | unchanged default output (under the old order) |
| W7 | T7 under the old order: both values | bit for bit | ADR 0376 B1; the brief | equal | 1 | unchanged default output (under the old order) |
| W8 | T8 under the old order: the eight values | bit for bit | ADR 0367 D1; the brief | equal | 1 | unchanged default output (under the old order) |
| W9 | T9 under the old order: the four values | bit for bit | ADR 0409 A6; the brief | equal | 1 | unchanged default output (under the old order) |
| N1 | T1 under the default: both values of the column "New" | bit for bit | gate run 44599067147, `test.log` line 10740; the maintainer's answer of 2026-11-20 | equal | 1 | unchanged default output |
| N2 | T2 under the default: both values | bit for bit | `test.log` line 11021; the maintainer's answer | equal | 1 | unchanged default output |
| N3 | T3 under the default: `a126cd75e3d481d7` | bit for bit | `test.log` line 11282; the maintainer's answer | equal | 1 | unchanged default output |
| N4 | T4 under the default: R2'a as registered, R2'b `a126cd75e3d481d7`, R2'c as registered (R3) | bit for bit | `test.log` line 11257; ADR 0238 R2' (R2'a, R2'c); the maintainer's answer | equal; R2'c not measured by the gate (`r2c_moving_cylinder_record`, the same run without the opt-in, passed there) | 1 | unchanged default output |
| N5 | T5 under the default: the two new values, the two connectivities as registered | bit for bit | `test.log` line 11429; ADR 0359 F0; the maintainer's answer | equal | 1 | unchanged default output |
| N6 | T6 under the default: U twice as registered, TG_1 twice `a126cd75e3d481d7` | bit for bit | `test.log` line 12210; ADR 0241 R2e (both TG_1 checks have one record); the maintainer's answer | equal; the fourth check not measured by the gate | 1 | unchanged default output |
| N7 | T7 under the default: quadratic `0x9f56741b853c367b`, linear as registered (R3) | bit for bit | `test.log` line 12299; ADR 0376 B1; the maintainer's answer | equal; the linear value not measured by the gate | 1 | unchanged default output |
| N8 | T8 under the default: the eight values of the column "New" | bit for bit | `test.log` line 12591; the maintainer's answer | equal | 1 | unchanged default output |
| N9 | T9 under the default: quadratic `0x9f56741b853c367b`; linear, G and G set off as registered (R3) | bit for bit | `test.log` line 12910; ADR 0409 A6; the maintainer's answer | equal; the three later values not measured by the gate | 1 | unchanged default output |
| O1 | Each of the 21 `overset*` files of `rcfd-fv` passes, unedited except for the nine attributes | 0 failures in each | the brief (the registered decision rule); tolerance table, bit-identity | 0 (the gate: no failure in them other than the nine) | 1 | unchanged default output |
| O2 | `ale_bdf2_f345`, `bdf2_remainder_f344`, `rcfd-fv` `--lib` | 0 failures | rule 7 | 0 | 1 | unchanged default output |
| O3 | `rcfd-api` `baseline_hashes`, unedited | every registered hash equal (the test passes) | rule 3 | equal | 1 | unchanged default output |
| D1 | The nine retired records, each run once with `--ignored` on the code commit | each fails at the assertion of the gate log with the left value of the column "New" | gate run 44599067147, `test.log` (the nine panics) | as stated | 1 | code verification (negative control) |
| S0 | `shipped_store_f834` D1 on the code commit, unedited | fails; of the 20 values exactly one differs from the table: the first of `axial_fan_cgrid`, 275158313255873701 (`0x03d18f072e9caca5`) | gate run 44599067147, `test.log` lines 1843 and 1844 | as stated | 1 | code verification (negative control) |
| S1 | The same test on the scratch copy with the scalar AMG on the old order everywhere (D5) | passes: the 20 values of the table, the first of `axial_fan_cgrid` 16452657040940017478 (`0xe4539468a88f2746`) | ADR 0373 D1 (the table of the test, the registered old result) | passes (hypothesis of D5; not known) | 1 | unchanged default output (under the old order, scratch copy) |

No row is of kind validation: the acceptance of F-471 claims no agreement with a measured case.

### Negative controls and planned mutations

D1 and S0 are negative controls. Mutations, each alone on a scratch copy of the code commit (its own target directory), by
an exact text replacement checked to occur the stated number of times, the new test file run once in release, the file
restored and compared byte for byte:

| Mutation | Must fail |
|---|---|
| M1 (committed, rule 16): in `overset.rs` the AMG build of `overset_pressure` takes `rcfd_linalg::AmgSettings::default()` again (the way back ignored in the overset solver). Patch `crates/rcfd-fv/tests/overset_fingerprints_adr0748.mutation.patch` | W1 to W9 (each at its first value that differs between the columns "Old" and "New") |
| M2: `LinearOptions::amg_settings` returns `coarse_lu_interchanges_first: false` always (the overset solver on the old order by default; M1a of ADR 0747) | N1 to N9 (each at its first new value) |
| M3 (reported; not part of the decision rule): the AMG build of `prescribed_pressure` takes `rcfd_linalg::AmgSettings::default()` again | a test of W8 or W9 only if a run under the prescribed fringe flux differs between the two orders. The gate measured F5 of T8 unchanged and did not reach G of T9, so M3 may change no operation of any run of this file; if it is not caught it is reported as such, with that reason, and the second site is then covered by reading only |

The call path of M1 and M2 was followed before registering them (the lesson of ADR 0747 amendment 1): every run of T1 to
T9 except F5 and F7 of T8 and the G runs of T9 takes `overset_pressure` (the default fringe flux with a coupling), and both
read the order through `s.linear.amg_settings()` after D1. The existing files are not run under mutations.

## Size

Size (agent): done; 0; 0; 0; 250000

No run is heavy: the longest overset test of the gate is 328 s.

## Decision rule

- **done**: W1 to W9 and N1 to N9 are met (all nine tests have the old values bit for bit under the old order and the new
  values under the default); every overset file passes (O1); O2, O3 and D1 are met; M1 and M2 are caught by the tests
  named for them without an amendment after a run; `fmt` and `clippy` are clean. S0 and S1 are a diagnosis for the lead
  and the maintainer; their result is reported and does not decide done or partial.
- **partial**: a row of W, N, O or D is missed or not run (recorded by rule 2), or M1 or M2 is not caught, or a value of
  R3 moves.
- **stopped and reported**: a registered hash, fingerprint or record outside the nine tests moves, or a test of the listed
  files fails for another reason. Nothing is edited and nothing is re-recorded; the file, the test and both values are
  reported with the evidence.
- **todo** is not an outcome of this registration.

## What is not covered

- Which factorisation of each run interchanges, and at which step the two orders part: the overset solver has no capture
  of its pressure systems, and none is added here.
- The old order through the coupled, potential and variable-density solvers (D2).
- The second AMG site (`prescribed_pressure`) is covered by a run only if M3 is caught.
- Whether results that are not registered moved in the overset files beyond what their own assertions show.
- The store hash of `axial_fan_cgrid` is diagnosed, not restated; what follows from the diagnosis is the lead's and the
  maintainer's.
- The whole suites and the lead's push run.

## Open questions

None is open at registration. D2 is taken on the standing instruction and changes no result.

## Addendum 1: results (2026-11-20)

Built as registered. Commits: the registration d09b35eb (with the maintainer's answer appended to ADR 0743 and ADR 0747),
the code 7503b091, the committed mutation patch 0a1abe82. All runs in the release profile, each registered run made once,
on Windows. Logs are in `w49i-work/f471-overset-fingerprints-scratch/logs`. No limit and no registered value was changed,
and no amendment was written.

**The work stopped at two further moved values (R3).** Behind the first failing assertion of two of the nine tests the
gate had measured nothing, and two registered values there move with the default as well: the hash of TG_1 moving with the
linear stencils (ADR 0376 B1, asserted by T7 and T9) and the hash of G (ADR 0409 A6, asserted twice by T9). Both come back
bit for bit under the old order. The maintainer's approval of 2026-11-20 was given without them, so they are not restated:
the two restated default tests `f450::t7_b1_default` and `f479::t9_a6_default` are left as registered and **fail** on the
branch. What was already planned and changes nothing in the tree (the existing files, the mutations on the scratch copy,
the diagnosis of the store hash, D1) was run to the end, so that the report is complete; nothing was edited after the
stop but this addendum.

### The nine tests

| # | Test | Old (registered) | Under the old order (W) | Under the default (N) |
|---|---|---|---|---|
| T1 | `overset_f487` D1c | `0x6820a06f1b6a1496`, `0x861ab174c9e7dbe2` | equal | `0x0c3ec8cd8e210f07`, `0xec0a524a76ab219e`, as the gate |
| T2 | `overset_f472` D1b | `0x6e6117e1b91851a9`, `0x69a719b86040e649` | equal | `0x149540b75a11eb76`, `0x688bd9678efe83d4`, as the gate |
| T3 | `overset_f335_records` R2b | `75ac2daf13879872` | equal | `a126cd75e3d481d7`, as the gate |
| T4 | `overset_f335_records` R2' | `7aa14d1e2e2da052`, `75ac2daf13879872`, `fddb420dc287502a` | equal | `7aa14d1e2e2da052`, `a126cd75e3d481d7`, `fddb420dc287502a` (R2'c, not reached by the gate: unchanged) |
| T5 | `overset_motion_f442` F0 | `0x3deeab3cfda1d8d0`, `0x4758f6c215970201`, two connectivities | equal | `0xb66f8e865defb9eb`, `0x87e9c7c8539a0a98`, the connectivities unchanged, as the gate |
| T6 | `overset_f338` R2e | `7aa14d1e2e2da052` twice, `75ac2daf13879872` twice | equal | `7aa14d1e2e2da052` twice, `a126cd75e3d481d7` twice (the fourth, not reached by the gate: the new value of the record, as registered) |
| T7 | `overset_f450` B1 | `0xe43a026a778e83b3`, `0x9c5ab1e5b21d5368` | equal | `0x9f56741b853c367b` as the gate; **linear `0xb7eacfdad14044af`, moved, not seen by the gate** |
| T8 | `overset_f368` D1 | the eight values of the table of the registration | equal | the eight values of its column "New", as the gate |
| T9 | `overset_f479` A6 | `0xe43a026a778e83b3`, `0x9c5ab1e5b21d5368`, `0xb2b43812eb166b04` twice | equal | `0x9f56741b853c367b` as the gate; **linear `0xb7eacfdad14044af` and G `0xe8f1628a95798aab` twice, moved, not seen by the gate** |

### Results

| # | Check | Limit | Measured | Met |
|---|---|---|---|---|
| W1 to W9 | each test under the old order | bit for bit | every registered value equal (9 tests passed) | yes |
| N1 to N6, N8 | the test under the default | bit for bit | equal (7 tests passed) | yes |
| N7 | T7 under the default | quadratic new, linear as registered | quadratic `0x9f56741b853c367b` (met); linear `0xb7eacfdad14044af` against the registered `0x9c5ab1e5b21d5368`: fails at "N7: linear stencils under the default" (line 1529) | **no** |
| N9 | T9 under the default | quadratic new; linear, G, G set off as registered | `0x9f56741b853c367b` (met), `0xb7eacfdad14044af`, `0xe8f1628a95798aab`, `0xe8f1628a95798aab`: fails at "N9: default, linear stencils" (line 2069); G is read from the printed line | **no** |
| O1 | the 21 `overset*` files | 0 failures in each | 0 failures in each (list below) | yes |
| O2 | `ale_bdf2_f345`, `bdf2_remainder_f344`, `--lib` | 0 failures | 8 passed, 2 ignored; 4 passed, 4 ignored; 115 passed, 6 ignored | yes |
| O3 | `baseline_hashes` | passes | 3 passed, 1 ignored (as on main) | yes |
| D1 | the nine retired records with `--ignored` | each fails at the assertion of the gate log with its left value | each fails there with the left value of the gate (R2b, R2'b, R2e TG_1 `a126cd75e3d481d7`; the arrays of T1, T2, T5, T8; quadratic 11481491961575913083 in T7 and T9) | yes |
| S0 | `shipped_store_f834` D1, unedited (scratch copy of 0a1abe82) | fails with exactly one changed value | fails; of 20 values one differs: the first of `axial_fan_cgrid`, `0x03d18f072e9caca5` | yes |
| S1 | the same with the scalar AMG on the old order everywhere | passes | passes: `axial_fan_cgrid` `0xe4539468a88f2746`, `0x5234e433bd8358f4` | yes |

The new file `overset_fingerprints_adr0748`: 16 passed, 2 failed, in 13 s.

**O1, each file** (passed / ignored; seconds of the file's run): `overset_f134` 12/2 (187), `overset_f134_records` 1/0 (1),
`overset_f314` 4/8 (68), `overset_f334` 3/8 (60), `overset_f335` 5/4 (16), `overset_f335_records` 2/2 (23), `overset_f336`
5/2 (72), `overset_f338` 8/10 (90), `overset_f349` 3/10 (24), `overset_f353` 1/1 (9), `overset_f357` 5/0 (22),
`overset_f359` 5/2 (51), `overset_f368` 9/1 (34), `overset_f445` 9/5 (7), `overset_f450` 2/3 (17), `overset_f468` 1/1 (3),
`overset_f472` 7/4 (46), `overset_f479` 3/2 (13), `overset_f487` 9/1 (40), `overset_f503` 1/2 (26), `overset_motion_f442`
2/7 (3). No failure in any of them.

### The store hash of `axial_fan_cgrid` (the tenth failure of the gate)

**The cause is F-471**: the default of ADR 0743, reached through the pressure solve of `pressure.rs`. Evidence, on the
scratch copy of 0a1abe82 (the tip, with every candidate merge in), `shipped_store_f834` D1 unedited in every run:

- unedited (S0): fails; the first hash of `axial_fan_cgrid` is `0x03d18f072e9caca5`, the other 19 values are the table's;
- `AmgSettings::default()` with `coarse_lu_interchanges_first: false` and `LinearOptions::amg_settings` returning `false`
  always (S1): **passes**, the first hash is the registered `0xe4539468a88f2746` (16452657040940017478);
- not registered, the two halves alone: `LinearOptions::amg_settings` returning `false` alone: passes, the registered
  value; the `Default` of `AmgSettings` alone: fails with `0x03d18f072e9caca5`. So the solve that decides the hash takes
  its settings from `LinearOptions::amg_settings`, which only `pressure.rs` (and now `overset.rs`) reads.

So the order of the coarsest dense LU solve alone decides this hash, and the old value is reproduced under the old order.
The bisect over the six merges was not made (D5). The other nine cases do not move. Which factorisation of the case
interchanges, and at which step, was not looked at. The brief's hexadecimal for the old value (`0xe453d1b2ca2a6c06`) is a
misprint (R4). The test and its table are untouched; it still fails on this branch as on main.

### Mutations (each alone on the scratch copy of 0a1abe82, the new test file once, the file restored byte for byte)

The unmutated file already fails at N7 and N9, so those two tests say nothing under a mutation; they are left out below.

| Mutation | Registered to fail | Result |
|---|---|---|
| M1 (committed patch: `overset_pressure` takes `rcfd_linalg::AmgSettings::default()`) | W1 to W9 | caught: all nine old-order tests fail (W1 line 286, W2 540, W3 and W4 at `check` line 606, W5 1051, W6 1299, W7 1537, W8 1863, W9 2079); the seven default tests that pass unmutated pass |
| M2 (`LinearOptions::amg_settings` returns `false` always) | N1 to N9 | caught by N1 to N6 and N8 (each at its first new value); N7 and N9 fail at their quadratic value, one assertion earlier than unmutated |
| M3 (reported: `prescribed_pressure` takes `rcfd_linalg::AmgSettings::default()`) | W8 or W9 if a run under the prescribed fringe flux differs between the orders | caught by W9: under the old order G is `0xe8f1628a95798aab` in place of `0xb2b43812eb166b04`. The G run of T9 does differ between the orders, so the second site is covered by a run |

The committed patch applies (`git apply --check`). The lead's push script expects the patched file to fail; it fails
already without the patch (N7, N9), so the script's check proves nothing until question 1 is answered.

### Checks of rule 7

`python scripts/fmt_check.py rcfd-fv rcfd-linalg`: clean. `cargo clippy -p rcfd-fv -p rcfd-linalg --all-targets -- -D
warnings`: clean. `cargo test -p rcfd-cli --release --test docs_site`: 2 passed. `python scripts/process_report.py --write`
and `--check`: see the commit of this addendum (0 failures).

Shared solver file changed, `crates/rcfd-fv/src/overset.rs` (line numbers of 7503b091): 64 (the import without
`AmgSettings`), 2027 to 2029 (`overset_pressure`: two comment lines and `Amg::new(&a, s.linear.amg_settings())`), 2174 and
2175 (`prescribed_pressure`: one comment line and the same call). Test files: one attribute line in each of
`overset_f487.rs`, `overset_f472.rs`, `overset_motion_f442.rs`, `overset_f338.rs`, `overset_f450.rs`, `overset_f368.rs`,
`overset_f479.rs` and two in `overset_f335_records.rs`; new `overset_fingerprints_adr0748.rs` and its `.mutation.patch`.

### Deviations and disclosures

- The session's file tools were bound to another worktree at the start; the session was switched into the worktree of the
  brief with the harness's own command before the first file was written. Nothing was written in any other worktree.
- After the run of the new file an amendment was prepared that would have kept the two failing default tests as ignored
  `known_miss` records and restated them with the measured values. It was not made: it would have ignored tests to make a
  run pass and pinned values the maintainer has not seen. No amendment exists; the two tests fail as registered.
- Runs made after the stop: the 23 existing files (started before the stop was decided, let finish), M1 to M3, S0 and S1
  with the two unregistered halves, D1, `--lib`, `baseline_hashes`, `docs_site`. None writes to the tree.
- S0, S1 and the mutations ran on a scratch copy made with `git archive` of 0a1abe82, not in the worktree.
- The new test file is generated by a script of the scratch folder (`gen_test.py`) that copies the named helpers from the
  eight source files at d09b35eb; in the copy of `overset_f479.rs` the enum `Mode` carries `#[allow(dead_code)]`, since
  its variant `Q` is not used by A6. That line was not in the registration's description of the copies.
- Of the brief's reading list `overset.rs`, `amg.rs` and the eight test files were read in the parts the registration
  names, not whole.
- `process_report.py --write` regenerates pages that rule 9 gives to the lead (`docs/adr/index.md` among them); it was run
  and its output committed because the working conditions of this dispatch ask for it.

### Not run, not done

- The two further moved values are not restated; `f450::t7_b1_default` and `f479::t9_a6_default` fail, so **the branch
  cannot be merged as it stands**.
- The bisect of the store hash over the six merges (not needed after S1).
- Which factorisation of each run interchanges (registered as not covered).
- The whole suites and the lead's push script.

### Size

Size (agent): partial; 1; 0; 2; about 190000

### Decision

By the registered rule: **partial, and the work is stopped**. All nine tests have their registered values bit for bit under
the old order (W1 to W9), every value the gate measured is reproduced under the default, every overset file passes, O2, O3
and D1 are met, and M1, M2 and M3 are caught. N7 and N9 are missed: two values of R3 moved (the rule's "a value of R3
moves"), and by R3 they go to the maintainer and are not restated here.

## Open questions for the maintainer

1. **Two further registered values move with the approved default, inside two of the nine tests, behind the assertion at
   which the gate stopped.** (a) ADR 0376 B1, TG_1 moving for 40 steps with the default coupling and the linear stencils
   (`B1_LINEAR` of `overset_f450::b1_defaults_bit_identical` and of `overset_f479::a6_without_the_opt_in_every_bit_is_kept`):
   registered `0x9c5ab1e5b21d5368`, with the default `0xb7eacfdad14044af`. (b) ADR 0409 A6, the same run under the
   prescribed fringe flux G with the quadratic stencils (`G_EB13160D` of the A6 test, asserted with and without the opt-in
   set off): registered `0xb2b43812eb166b04`, with the default `0xe8f1628a95798aab`. Both are the registered values bit for
   bit under the old order (W7, W9). Options: (i) approve these two moves as well: an amendment of this ADR restates T7
   and T9 with the four measured values and keeps the two failing tests as ignored records; (ii) keep the overset solver
   on the old order (the way back as its default); (iii) revert the default of ADR 0743. Recommendation: **(i)**. The
   evidence is the same as for the nine: the old order reproduces each old value exactly, and both tests are already
   among the nine the answer of 2026-11-20 names.
2. **The store hash of `axial_fan_cgrid`** (`shipped_store_f834` D1, the table of ADR 0373 D1) moved with the default of
   ADR 0743: registered `0xe4539468a88f2746`, now `0x03d18f072e9caca5`; under the old order the registered value (S1).
   This is a row of a case hash table, which rule 3 keeps from an agent. Options: (i) approve the move: the lead
   re-records the one value; (ii) revert the default. Recommendation: **(i)**, with a way for a case run to ask for the
   old order only if a record of the old store is wanted (none is today).

## For the lead: roadmap entries

Status proposed for F-471: partial.

> ADR 0748, 2026-11-20, partial and stopped: the overset solver takes its AMG settings from the solver's linear options (the way back of ADR 0743 for overset.rs); the nine tests of the red gate of 252c9279 are ignored records and are restated in overset_fingerprints_adr0748.rs; all nine give their registered values bit for bit under the old order and the gate's values under the default; the 21 overset files pass; three of three mutations caught; stopped because two further registered values of T7 and T9 move that the gate had not reached (B1 linear 0x9c5ab1e5b21d5368 to 0xb7eacfdad14044af, A6 G 0xb2b43812eb166b04 to 0xe8f1628a95798aab), so two restated tests fail; the store hash of axial_fan_cgrid moved with the default of ADR 0743 through pressure.rs and is the registered value under the old order.

> Gap: the branch of ADR 0748 is not mergeable: f450::t7_b1_default and f479::t9_a6_default of overset_fingerprints_adr0748 fail at two registered values the maintainer has not seen (ADR 0748 question 1), and shipped_store_f834 D1 fails at the first hash of axial_fan_cgrid, which moved with the default of ADR 0743 (question 2). Needs the maintainer's answers; then an amendment of ADR 0748 restates T7 and T9 and the lead re-records or keeps the store hash.

No new item is proposed.

## Answer of the maintainer (2026-11-20, second), written into this ADR by the lead

The lead asked the maintainer on 2026-11-20, as three options with a recommendation (approve all three; approve the two
overset values and keep axial_fan_cgrid on the old order through a case key; revert the default). The maintainer answered
on 2026-11-20: "Approve all three". That is: the moves of B1 linear (0x9c5ab1e5b21d5368 to 0xb7eacfdad14044af; overset_f450
B1 and overset_f479 A6), of A6 G (0xb2b43812eb166b04 to 0xe8f1628a95798aab; overset_f479 A6) and of the first store hash of
cases/axial_fan_cgrid/axial_fan_cgrid.toml in shipped_store_f834 D1 (0xe4539468a88f2746 to 0x03d18f072e9caca5) under the
corrected order are approved, under ADR 0748. Nothing else is approved: any further registered value that moves stops the
work again and is reported.

## Amendment 1 (2026-11-20): T7 and T9 restated with the two further values; the store hash of `axial_fan_cgrid` restated

Written after addendum 1 and after the maintainer's second answer above, and before any code or test of this amendment
and before any run of its checks. What was done beforehand is reading (`crates/rcfd-workspace/tests/f834/mod.rs` lines 1 to
100, `crates/rcfd-api/src/sim.rs` lines 1281 to 1293 and 2920 to 2942, `crates/rcfd-fv/src/simple.rs` lines 455 to 480 and
the lines that name `linear`). No limit of this or any other ADR changes. No library file changes.

**A1. T7 and T9 (rule 2).** The two default tests that missed, `f450::t7_b1_default` and `f479::t9_a6_default`, stay in
`overset_fingerprints_adr0748.rs` as ignored records: each is renamed with the suffix `_known_miss` and gets one
`#[ignore = "known miss, ADR 0748 amendment 1: ..."]` line with the measured numbers; its body and its limits stay as
they are. This is rule 2's form for a missed limit; these two are the only tests ignored beyond the attributes the lead
names, and they are tests of this registration, not of another record. Two tests are added in the form of the others:

- `f450::t7_b1_default_amended` (N7a): quadratic `0x9f56741b853c367b`, linear `0xb7eacfdad14044af`.
- `f479::t9_a6_default_amended` (N9a): `0x9f56741b853c367b`, `0xb7eacfdad14044af`, `0xe8f1628a95798aab`,
  `0xe8f1628a95798aab`.

The old values under the old order are already running tests and are not changed: `f450::t7_b1_old_order` (W7) and
`f479::t9_a6_old_order` (W9); both passed in addendum 1.

**A2. The store hash (the lead's decisions, given with the maintainer's second answer).** In
`crates/rcfd-workspace/tests/shipped_store_f834.rs` the test
`d1_the_store_of_every_non_adaptive_shipped_case_is_unchanged` is retired with one line above its `fn`,
`#[ignore = "retired record (ADR 0748, F-471): the first hash of axial_fan_cgrid moved with the default of ADR 0743, approved by the maintainer on 2026-11-20; restated in shipped_store_adr0748.rs"]`;
its body and its table are untouched. It is restated in the new file `crates/rcfd-workspace/tests/shipped_store_adr0748.rs`
(header: ADR 0748, F-471; it restates D1 of ADR 0373, F-476, as kept by F-834), which declares `mod f834;` and holds copies
of the table and of `planned` of the retired test:

- `d1_restated_under_the_default` (S2): the body of D1 over the same ten cases, against the table with the first hash of
  `axial_fan_cgrid` `0x03d18f072e9caca5` and the nineteen other values exactly as registered. If any of the nineteen
  differs, the work stops and it is reported.
- `d1_axial_fan_cgrid_under_the_old_order` (S3): the lead asked for old-order evidence as a running test only if an
  existing API gives it without a new key of the case schema and without touching a golden. One does:
  `Simulation::simple_solver_mut()` ("diagnostics and tests") returns the SIMPLE/SIMPLEC solver of a steady run, whose
  public `settings.linear` is the `LinearOptions` the pressure solve reads. The test builds the planned case of
  `axial_fan_cgrid` as D1 does, sets `settings.linear.amg_coarse_lu_interleaved = true` on that solver before the first
  step, runs the same three steps and hashes the same two files: the registered `0xe4539468a88f2746` and
  `0x5234e433bd8358f4`. That the flag set after `Simulation::new` reaches the three steps is a hypothesis of this row (the
  solver passes `linear: s.linear` to the pressure solve in each iteration, `simple.rs` line 1251); if the row misses it
  is recorded by rule 2, and the evidence of the old value is then the scratch-copy runs S1 of addendum 1.

No key of the case schema is added, no golden and no hash table is edited.

**A3. Checks added.** Limits are from the tolerance table's row "Bit-identity of default output".

| # | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| N7a | T7 under the default: quadratic `0x9f56741b853c367b`, linear `0xb7eacfdad14044af` | bit for bit | addendum 1 (the run of N7 on 0a1abe82); the maintainer's second answer of 2026-11-20 | equal | 1 | unchanged default output |
| N9a | T9 under the default: `0x9f56741b853c367b`, `0xb7eacfdad14044af`, `0xe8f1628a95798aab` twice | bit for bit | addendum 1 (the run of N9 on 0a1abe82); the maintainer's second answer | equal | 1 | unchanged default output |
| S2 | D1 restated: the ten cases, the first hash of `axial_fan_cgrid` `0x03d18f072e9caca5`, the nineteen other values as registered | bit for bit | ADR 0373 D1 (nineteen values); gate run 44599067147 and S0 of addendum 1; the maintainer's second answer | equal | 1 | unchanged default output |
| S3 | `axial_fan_cgrid` under the old order through `simple_solver_mut` | `0xe4539468a88f2746`, `0x5234e433bd8358f4`, bit for bit | ADR 0373 D1 (the registered old result) | equal (S1 of addendum 1, by another route) | 1 | unchanged default output (under the old order) |
| O1a | `overset_fingerprints_adr0748` and the 21 `overset*` files, again | 0 failures in each | the lead's instruction with the second answer; tolerance table, bit-identity | 0 | 1 | unchanged default output |
| O4 | Every test file of `rcfd-workspace` (`cargo test -p rcfd-workspace --release`), with `shipped_store_f834` and the new file among them | 0 failures | the lead's instruction with the second answer; rule 7 | 0 | 1 | unchanged default output |
| O3a | `rcfd-api` `baseline_hashes`, again | the test passes | rule 3 | passes | 1 | unchanged default output |
| D2 | The retired D1 of `shipped_store_f834`, run once with `--ignored` | fails with exactly the one changed value, as S0 | S0 of addendum 1 | as stated | 1 | code verification (negative control) |

**A4. Mutations**, each alone on the scratch copy (brought to the code commit of this amendment), the named test file
run once in release, the file restored byte for byte:

| Mutation | Must fail |
|---|---|
| M1 again (the committed patch of `overset_fingerprints_adr0748`, applied with `git apply`): the unmutated file passes whole, so the patch's expect-fail now has a meaning | W1 to W9 |
| M2 again (`LinearOptions::amg_settings` returns `false` always) | N1 to N6, N7a, N8, N9a |
| M3 again (`prescribed_pressure` takes `rcfd_linalg::AmgSettings::default()`) | W9 |
| M4 (committed, rule 16, patch `crates/rcfd-workspace/tests/shipped_store_adr0748.mutation.patch`): `LinearOptions::amg_settings` returns `coarse_lu_interchanges_first: false` always | S2, at the first hash of `axial_fan_cgrid`, with the old value on the left |
| M5: `LinearOptions::amg_settings` returns `coarse_lu_interchanges_first: true` always (the way back cut) | S3 |

**A5. The decision rule, extended to the three values; no limit changes.**

- **done**: every row of the registration other than N7 and N9 is met as in addendum 1 or again here; N7a, N9a, S2, S3,
  O1a, O4, O3a and D2 are met; M1 to M5 are caught by the tests named for them; `fmt` and `clippy` are clean for
  `rcfd-fv`, `rcfd-linalg` and `rcfd-workspace`. N7 and N9 stay missed rows of the registration, recorded as the two
  `known_miss` tests; their values are the ones the maintainer approved in the second answer.
- **partial**: a row of this amendment is missed or not run, or a mutation is not caught. If only S3 misses, the store
  hash has no running old-order test and the outcome is partial with that stated.
- **stopped and reported**: any registered value other than the nine tests' values and the three of the second answer
  moves; one of the nineteen other store values among them.

Size (agent), amendment 1: done; 0; 0; 0; 30000
