# ADR 0727: F-846 registration: what switching the default local mode update of the incompatible Hex8 to the warm start would move (a census of every driver solve of the existing tests of the element under the switched default, and the committed states through the save and restart paths)

Status: **REGISTERED 2026-11-17, written before any code of F-846 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no test or instrument code of F-846 was written and nothing of this item was run (no probe). The
numbers quoted below come from the ADR texts and from reading the code; the arithmetic was done by hand (disclosed). Results
go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-846 (deps F-844), the follow-up proposed by
[ADR 0725](0725-f844-registration-the-local-mode-update-of-the-incompatible-hex8-as-a-default-measured.md) (its remaining
question 1). Number 0727 given by the lead. Branch `f846-work` from e6f7ed2c (ADRs 0655, 0658, 0660, 0723, 0725 merged),
worktree `w49i-work/f846`, scratch `w49i-work/f846-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run; each registered run made once;
defaults bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing
test edited; no new dependency, no unsafe code; clean-room. Every choice below that the maintainer would normally be asked
about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except the open questions at
the end.

**This is a measurement, not a default change.** No library line is committed. The switch itself stays the maintainer's
decision (ADR 0725 remaining question 1).

## What F-846 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. With the model's warm start turned on in test-only copies of each case (no default changed), the existing nonlinear tests of the incompatible Hex8 and the rcfd-api baseline cases that use the element: the number of results whose bits change, the largest relative change of each recorded measure against a registered tolerance, every solve whose outcome changes, and the committed element states through the save and restart path; the default bits unchanged.

Clauses: **c0** every limit registered here; **c1** the warm start (`SolidModel::set_incompatible_mode_warm_start(true)`,
ADR 0660 D1, alone: the plain update, as ADR 0725's WS) turned on in copies of each case, no default changed; **c2** the
existing nonlinear tests of the incompatible Hex8; **c3** the rcfd-api baseline cases that use the element; **c4** the number
of results whose bits change; **c5** the largest relative change of each recorded measure against a registered tolerance;
**c6** every solve whose outcome changes; **c7** the committed element states through the save and restart path; **c8** the
default bits unchanged.

## What was read

- `AGENT_RULES.md`; the brief; ADR 0725 (whole); ADR 0660 D1 and its questions; ADR 0666 (the warm start on the transient
  path, the restore rule D3), ADR 0668 D1, D2 (the readings that cut a warm state to its material part); ADR 0572, 0573 and
  0725 (the roadmap section).
- `crates/rcfd-fem/src/model.rs` (`SolidModel::new`, the four mode-update setters), `nl_incompatible.rs` (`evaluate`,
  `warm_state`, `check_state`, `LOCAL_TOLERANCE`), `nonlinear.rs` (`solve_with`, `solve_increment`, `solve_from*`,
  `solve_in_time_with`, `LoadStep`, `NonlinearResult`, `Equilibrium`, `TimeStep`, `NewtonOptions::new` tolerances),
  `nl_transient.rs` (`NlTransientState::to_bytes`, `from_bytes`, `Integrator::state`, `restore`, `run_with`), `arclength.rs`
  (`trace`, `ArcLengthPath`), `rotating.rs` (the model copy at line 344).
- `crates/rcfd-fem/tests/si_eas_hg_default_bits.rs`, `si_eas_hg3_bits.rs`, `si_eas_default_bits.rs` (whole), the transient
  part of `si_eas_hg9.rs` (`w_on_the_transient_cantilever`, its TS1 and NCR), `crates/rcfd-api/tests/baseline_hashes.rs`,
  `crates/rcfd-api/src/solid_case.rs` (head).

## Readings, registered

### c2: the existing nonlinear tests of the element, and how they were found

`grep -rl "Incompatible" crates/*/tests` (and case-insensitively, `grep -rli incompatible`, which adds only
`crates/rcfd-fv/tests/wall_leak_w34a.rs`, a flow test where the word is about a boundary condition) gives 35 files of
`crates/rcfd-fem/tests`; one of them, `f703/r_body.rs`, is the body included by `si_eas_hg3_rd.rs` and `si_eas_hg3_rl.rs`. No
shared module under `tests/common`, `tests/*_common`, `tests/f664_beck` names the formulation. That is **34 test binaries**:

`si_eas`, `si_eas_default_bits`, `si_eas_hg`, `si_eas_hg2`, `si_eas_hg3`, `si_eas_hg3_bits`, `si_eas_hg3_rd`, `si_eas_hg3_rl`,
`si_eas_hg4` to `si_eas_hg21` (18 files), `si_eas_hg_default_bits`, `si_eas_hg_smooth`, `si_eas_local_update_f844`,
`si_fbar`, `si_hex8x`, `si_hex8x_estimate`, `si_hex8x_linear`, `si_scaffold`.

Which of their solves is "nonlinear with the element" is not decided by reading 50 000 lines: the census below records every
driver solve these binaries make on a model with at least one Incompatible Hex8, so the list of cases is the run's own output
(per binary, per test function, per solve). Files whose default tests make no such solve (expected: the linear files
`si_hex8x_linear`, `si_hex8x_estimate`, `si_scaffold`, most of `si_hex8x`) are reported with zero records.

The flag is read only on the nonlinear paths: `nonlinear.rs` (the element evaluation in the internal force and the tangent,
lines 1050 and 1149; the yielded-point record, 1986; the creep reading, 3642), `nl_transient.rs` (1066 the restore, 1449 the
energy record) and `rotating.rs` (354, the copy of the model keeps the flag). The linear paths do not read it.

### c1: "turned on in test-only copies of each case (no default changed)"

Two instruments.

- **A, the census (scratch, never committed).** Two copies of the test commit made by `git archive` in the scratch folder:
  **base** and **flip**. Both get the same instrument (below); flip also gets one changed line, `incompatible_mode_warm_start:
  false` to `true` in `SolidModel::new` (model.rs line 1025). Reading: `SolidModel::new` is the only constructor of a model
  (`grep -rn "SolidModel::new" crates/rcfd-fem/src`: the one library call is `rotating.rs` 344, which copies the flag), so the
  flip is exactly "every model of every case built with the warm start turned on", without editing any of the 34 files; it is
  also literally what the default switch would be. No test file is changed in either copy. Nothing of A is committed.
- **B, in-process (committed, test-only).** A new test file `crates/rcfd-fem/tests/si_eas_warm_default_f846.rs` that rebuilds
  the incompatible-element nonlinear cases of the two default bit records (`si_eas_hg_default_bits`, `si_eas_hg3_bits`) twice,
  the default model and a copy with the opt-in, compares them bit by bit in the same process, and measures the two save and
  restart paths (c7). It confirms its copies by reproducing the existing records' digests.

**The instrument of A.** A module `f846_dump` added to the scratch copies of `rcfd-fem` (`src/f846_dump.rs`, `pub(crate)`)
and a wrapper around each public driver entry: `nonlinear::solve_with` (so `solve`), `solve_increment`, `solve_from_field_reusing`
(so `solve_from`, `solve_from_at`, `solve_from_reusing`), `solve_in_time_with` (so `solve_in_time`), `nl_transient::Integrator::
run_with` (so `run`, `step`, `step_with`), `arclength::trace`. A thread-local depth counter records only the outermost entry
(a driver calling another driver is one record). A record is written only when the environment variable `F846_DUMP` names a
directory and the model has at least one Incompatible Hex8. It reads the result and writes a text file (values in Rust's
shortest round-trip `{:e}` form) named by the thread name (libtest names each test's thread by the test), the entry and its
index within that test. It changes no value. The record of each entry:

| Entry | Outcome | Values recorded |
|---|---|---|
| `solve_with` | `ok`, or `err` and the error text | `u` (the displacement), `reactions`, `load_factors`, per increment the residual count and halvings, `states` (per element its length and values) |
| `solve_increment`, `solve_from_field_reusing` | as above | `u`, `internal_force`, the residual count, `states` |
| `solve_in_time_with` | as above | per step end `u`, `internal_force`, `halvings`, `substeps`, `iterations`, and the last step's `states` |
| `Integrator::run_with` | as above | per step report `halvings`, `iterations`; at the end the state's `u`, `v`, `a`, `internal_force`, `states` |
| `arclength::trace` | as above | per point `load_factor`, `displacement`, `halvings`, residual count; `states` at the end |

**The runs of A.** For each of the 34 binaries, in each copy, once: `cargo test -p rcfd-fem --release --test <binary>` (the
default tests; the ignored heavy records are not run, see Limits) with `F846_DUMP=<copy>-dump/<binary>`, its own
`CARGO_TARGET_DIR` per copy, `CARGO_BUILD_JOBS=4`, the log kept. The comparison is a script in the scratch folder
(`compare.py`) that pairs the records of base and flip by (binary, test, entry, index) and computes the measures of c4 to c6.
A record present in one copy only (a test that stopped earlier, or a different number of solves) is reported as **unpaired**
with its test.

### c3: the rcfd-api baseline cases that use the element

`crates/rcfd-api/tests/baseline_hashes.rs` runs flow cases only (`Simulation`, the cavity paths and the shipped cases under
`cases/`). `grep -rn "Incompatible\|incompatible\|SolidFormulation\|BlockProperty" crates/rcfd-api/src` finds nothing; `rcfd_fem`
is used by `solid_case.rs` and `solid_parallel.rs` only, the linear static and modal solid case file of ADR 0438, whose
`mesh.element` is `hex8 | tet4 | tet10` with no formulation. So **no rcfd-api baseline case uses the element**: expected 0.
This is checked by running `baseline_hashes` on the flip copy as well (BH-F) and on the worktree (BH).

### c4, c5, c6: the measures and the tolerance

- **A result** is one record (one outermost driver entry). Its **value groups**: displacement (`u`, and `v`, `a` for the
  transient), force (`reactions` or `internal_force`), load factors, and the **material part** of the states (per element its
  first `points x state_size` values: the warm start adds nine values after them, c7). Counts (increments, halvings, residual
  counts, iterations) are compared separately.
- **c4, a result whose bits change**: a paired record, both `ok`, in which at least one value of a value group differs in its
  bits. Reported: the number of such results, of the paired `ok` results, and of the values that differ.
- **c5, the relative change of a measure**: per value group of a paired `ok/ok` record, `|b - a|_inf / |a|_inf` (`a` base, `b`
  flip; 0 when both are zero vectors; infinite when `a` is zero and `b` is not or the lengths differ); per group the largest
  over all records, with its binary and test. **Registered tolerance `T = 1e-6`.** Derivation: the driver's defaults stop at a
  relative residual and energy of `1e-10` (`NewtonOptions::new`); the local iteration stops at `|r|_inf <= 1e-12` of the mode
  scale (`LOCAL_TOLERANCE`) and converges quadratically, so the warm and the cold root of an element agree to a few rounding
  units. Two runs that converge to the same equilibrium can then differ by the last Newton correction, at most the residual
  tolerance times the condition of the reduced tangent; for the near-incompressible blocks (`nu = 0.49`, bulk over shear
  about 50, and the mesh factor `~ n^2 <= 256`) that is about `1e-10 x 1e4 = 1e-6`. A measure above `T` is a finding (named),
  not a miss of this item: the acceptance asks for the change against the tolerance, and the tolerance is the reading.
- **c6, a solve whose outcome changes**: a paired record with `ok` in one copy and `err` in the other; or `err` in both with a
  different error text; or `ok` in both with a different number of increments or of halvings (for the transient a different
  halving count of a step; for the arc length a different number of points). A different number of Newton iterations alone is
  reported as an **iteration change**, not an outcome change. Also per test function: **passed in base and failed in flip, or
  the reverse**, a **test outcome change**, with the first failing assertion text.
- **Classes of a test outcome change** (read from the failing assertion, rule registered now): (i) a bit record of the
  default path (a digest or a bit count against the default); (ii) a control that asserts the default differs from an
  opt-in, or that the default commits no modes; (iii) an assertion on the default's outcome (the default fails at a named
  element, or not); (iv) anything else (an accuracy or physics limit of a converged result): a **regression** of the switch.

## What is known before the run, and arithmetic (disclosed)

1. **The first increment does not move.** The driver evaluates every iterate of an attempt with the committed states of the
   start of the attempt. At the first increment of a solve from the initial states these are empty, so the warm evaluation
   finds no committed modes and iterates from `alpha = 0` exactly as the default (`evaluate`, the `None` branch of
   `warm_state`): the iterates, forces and tangents are those of the default bit for bit, and only the trial states gain nine
   values. Hence a solve of **one increment without a halving has bit-identical values** (the states aside), and so does
   every call of `internal_force`, `tangent`, `Integrator::start` and the strain-energy record at the initial states. A
   halving restarts from the same committed states, so it does not change this.
2. **From the second increment on** the start differs (committed modes against `alpha = 0`), the local roots agree to
   rounding (ADR 0660 arithmetic 3 and 8; ADR 0725 NCW: 252 values on DC2), so the bits are expected to change in almost
   every multi-increment record, with relative changes far below `T`.
3. **Outcomes.** ADR 0725: where the default converges the warm start converged too, with the same increments, halvings and
   iterations on all seven such set-ups; the default fails DB4, DS1, DS2, DS3, where the warm start passes. Those four, in
   `si_eas_hg6`, `si_eas_hg7`, `si_eas_hg8` and `si_eas_local_update_f844`, are expected to change outcome (`err` to `ok`).
4. **Tests expected to fail under the flip** (an estimate, not a limit): `si_eas_hg_default_bits` (class i: its Q1 block, CB2
   cantilever and NT1 bar have 4, 5 and 3 increments or steps), `si_eas_hg3_bits` (class i: the opted CB2 solve); the controls
   of `si_eas_hg8`, `si_eas_hg9`, `si_eas_local_update_f844` that compare an opt-in with the default or assert that the
   default commits no modes (class ii); the default-outcome rows of `si_eas_hg6`, `si_eas_hg7`, `si_eas_hg8` and F-844's RP0,
   RP1 (class iii). `si_eas_default_bits` (Full and BBar paths, and the linear Incompatible path) is expected to pass.
   Class (iv) is expected to be empty.
5. **The save and restart paths.** (a) The transient: `NlTransientState::to_bytes` writes per element its value count and
   values, so the nine modes are in the bytes; `Integrator::restore` accepts, with the warm start, an Incompatible element of
   `k` or `k + 9` values (ADR 0666 D3) and, without it, only `k` (ADR 0666 NCR, measured there with W). (b) The static driver:
   `NonlinearResult::states` and `Equilibrium::states` carry the nine modes; `solve_increment` restarts from given states
   (ADR 0688 D2: the iterates of `solve_with` from the same committed state, bit for bit); the default model refuses a state of
   `k + 9` values (`check_state`, key `"state"`), the warm model accepts `k` (no modes: from `alpha = 0`). (c) **There is no
   file-level save of the static driver's states in the repository**, and rcfd-api and the CLI have no nonlinear solid path:
   the only byte-level save and restart of element states is that of the transient integrator. These are recorded as found.
6. **Cost** (estimates): building the 34 test binaries in release in each copy about 10 to 20 minutes; their default tests
   ADR 0725 ran in 51 s, ADR 0723's fast test and the other fast rows are in the tens of seconds, so about 10 to 20 minutes a
   copy. Each record is small (the largest default meshes are `8^3`); the dump is a few hundred MB at most.

## Design

### D1. The census (A; scratch)

As reading c1. Scripts in `w49i-work/f846-scratch`: `instrument.py` (writes `f846_dump.rs`, wraps the six entries, applies the
flip to the flip copy only), `run_census.sh` (the 34 binaries in each copy), `compare.py` (the measures). The per-test pass and
fail is read from the libtest log of each copy.

### D2. The in-process file (B; committed)

`crates/rcfd-fem/tests/si_eas_warm_default_f846.rs`, default tests (fast):

- `f846_default_records_with_the_warm_start`: the cases of `si_eas_hg_default_bits` that reach the element (the c1 `6^3`
  patch tangent and force at `0.6` from the initial states; the ADR 0608 Q1 block solve, 4 increments; the CB2 cantilever
  `25 x 1 x 1`, `alpha = 1`, 5 increments; the NT1 bar `nl_transient`, 3 steps) and of `si_eas_hg3_bits` (the opted `6^3` and
  `4^3` patches, tangent and force; the opted CB2 solve; the opted `4^3` patch E state, strain energy and internal force),
  copied (existing tests are not edited). Each built as there (the default) and as a copy with
  `set_incompatible_mode_warm_start(true)`. Per case: the default's digest against the existing record's `EXPECTED` (DF), the
  values differing between the two, the largest relative change of each value group, the outcome (increments, halvings,
  residual counts) and the length of every Incompatible element's committed state.
- `f846_transient_save_and_restart`: the NT1 bar (`4 x 1 x 1`, steel, tip load `1e8`, `dt = 1e-5`), warm: 6 steps
  uninterrupted (the reference); 3 steps, the state through `to_bytes` and `from_bytes`, restored into a fresh warm integrator,
  3 more steps. Controls: the default integrator given the warm state; a warm integrator given the warm state with its modes
  cut off; the migration: the default integrator's state after 3 steps (no modes) restored into a warm integrator, 3 more
  steps, against the warm reference.
- `f846_static_restart`: the Q1 block, warm: `solve` with 4 increments (the reference) against the chain of four
  `solve_increment` calls at the reference's load factors, each from the previous `Equilibrium`'s displacement and states.
  Controls: the default model given the warm states at the second link; the warm chain with the modes cut off at the second
  link; the migration: the default chain to the first link, then the warm model from its (modeless) states.

## Checks

Every measure is required finite before it is compared (a non-finite value or a missing state misses).

### The census (A)

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| C0 | c2 | base: the default tests of the 34 binaries | every test passes (a base failure is recorded and its test excluded from the test-level counts) | the daily gate |
| C1 | c8 | flip: `si_eas_default_bits` (no Incompatible nonlinear solve) | passes | arithmetic 4 |
| C2 | c1 | flip: `si_eas_hg_default_bits` | fails (the flip reaches the driver) | arithmetic 4 |
| CF1 | c4 | paired `ok/ok` records of one increment and no halving (`solve_with`), or a `solve_increment` / `solve_from` from states with no committed modes | `0` values differ in every one | arithmetic 1 |
| SL | c7 | flip, every `ok` record with states: every Incompatible element's state length is base's plus 9; base has no such extra | holds for every element | ADR 0660 D1 |
| N0 | c2 | records per binary and test (the case list) | reported | |
| N4 | c4 | results whose bits change, of the paired `ok/ok`; values differing | reported | arithmetic 2 |
| N5 | c5 | per value group the largest relative change, its record | reported against `T = 1e-6` (above `T`: a finding, named) | arithmetic 2 |
| N6 | c6 | the solves whose outcome changes (each listed); iteration changes; unpaired records | reported | arithmetic 3 |
| NT | c6 | tests whose outcome changes, each with its class (i) to (iv) and assertion text | reported; class (iv) listed one by one | arithmetic 4 |
| BH-F | c3 | `cargo test -p rcfd-api --release --test baseline_hashes` on the flip copy | passes | reading c3 |

### The in-process file (B)

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| DF | c1 | the default copies' digests against `si_eas_hg_default_bits` and `si_eas_hg3_bits` `EXPECTED` (the nine digests of the copied cases: four of the first, five of the second) | equal | the copies |
| Z1 | c4 | the patches' tangent and force at the initial states, the E state: values differing, warm against default | `0` | arithmetic 1 |
| Z2 | c4 | the multi-increment solves: the first increment's residuals, warm against default | `0` bits differ | arithmetic 1 |
| NCW | c1 | the CB2 solve: values differing | `> 0` | arithmetic 2 |
| WB | c4, c5, c6 | per case: values differing, largest relative change per value group, increments, halvings, residual counts | reported against `T`; same increments and halvings (an estimate, reported) | arithmetic 2, 3 |
| WL | c7 | every Incompatible element of every warm solve: committed state of `points x state_size + 9` values; default `points x state_size` | holds | ADR 0660 D1 |
| R1 | c7 | transient: the warm state at step 3: every element of the bar has 9 values (steel: `state_size 0`) | holds | arithmetic 5 |
| R2 | c7 | transient: the state after `to_bytes`, `from_bytes`: `u`, `v`, `a`, the internal force and every state value | `0` bits differ | arithmetic 5 |
| R3 | c7 | transient: after the restore and 3 steps, against the uninterrupted 6: `u`, `v`, `a`, internal force, states | `0` bits differ | ADR 0666 TS1 |
| NCR1 | c7 | transient: the default integrator's `restore` of the warm state | refused | ADR 0666 NCR |
| NCR2 | c7 | transient: the warm state with its modes cut off, restored into a warm integrator, 3 steps: accepted; values differing from R3's | accepted; `> 0` | arithmetic 2 |
| RM | c7 | transient migration: the default state at step 3 restored into a warm integrator: accepted; 3 steps; values differing from the warm reference, largest relative change | accepted; reported against `T` | arithmetic 5 |
| S1 | c7 | static: the chain of four `solve_increment` against `solve` (4 increments), warm: `u` and every state value | `0` bits differ | ADR 0688 D2 |
| S2 | c7 | static: every Incompatible element's state after each link | 9 values (steel) | arithmetic 5 |
| NCS1 | c7 | static: the default model's `solve_increment` given the warm states | refused (key `"state"`) | arithmetic 5 |
| NCS2 | c7 | static: the warm chain with the modes cut off at the second link: values of the final `u` differing from S1's | `> 0` | arithmetic 2 |
| SM | c7 | static migration: default to the first link, warm from there: accepted; final `u` against the warm reference: values differing, largest relative change | accepted; reported against `T` | arithmetic 1, 2 |

### Default bits and static checks

| Id | Clause | Measure | Limit |
|---|---|---|---|
| B | c8 | `git diff --stat e6f7ed2c -- crates/*/src` at the tip | empty |
| BH | c8 | `cargo test -p rcfd-api --release --test baseline_hashes` on the worktree | passes |
| S6 | | `cargo fmt -p rcfd-fem --check`; `cargo clippy -p rcfd-fem --release --all-targets -- -D warnings` | clean |
| S7 | | release: `cargo test -p rcfd-fem --release --lib`; `--test si_eas_hg6`, `si_eas_hg7`, `si_eas_hg8` (ADR 0655, 0658, 0660), `si_eas_local_update_f844` (ADR 0725), their default tests; the new file (the registered run) | pass |
| S8 | | `cargo test -p rcfd-cli --release --test docs_site` (docs/ touched) | pass |
| S9 | | every commit builds | builds |

### Negative and positive controls

| Id | Control | Expected |
|---|---|---|
| C1 | the switch does not reach Full and BBar paths, nor the linear Incompatible path | `si_eas_default_bits` passes in flip |
| C2 | the flip reaches the driver | `si_eas_hg_default_bits` fails in flip |
| CF1, Z1, Z2 | the first increment and the initial states do not move | `0` |
| NCW | the opt-in of B reaches the driver | `> 0` |
| NCR1, NCS1 | the default refuses a state with modes | refused |
| NCR2, NCS2 | the committed modes are used at the restart | `> 0` |
| DF | B's copies are those of the records | equal digests |

### Planned mutations (each on a scratch copy of the test commit, run once against the default tests of the new file)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the warm builder of B ignores the flag | NCW (`0`), WL, R1, S2 |
| MU2 | the transient restart restores the state with its modes cut off | R3 (bits differ) |
| MU3 | the default copy of the Q1 block uses 5 increments | DF |
| MU4 | B's bit counter compares a vector with itself | NCW (`0`), NCR2, NCS2 |
| MU5 | the static chain passes the initial (empty) states at every link | S1 (bits differ), S2 |
| MU6 | the census without the flip | the base run itself is this mutation: C2 must pass there and fail in flip (no separate run) |

## Decision rule

- **done**: the census run once on both copies for the 34 binaries' default tests and compared (N0 to NT reported); C0 (with
  any base failure recorded), C1, C2, CF1, SL, BH-F met; every row of B met in its single registered run; B, BH and S6 to S9
  met; MU1 to MU5 caught and MU6 realised by C2; no amendment.
- **partial**: B met, but a limit row missed, a registered run not made (for example a binary of the census that does not
  finish), a mutation not caught, or a row met only after an amendment.
- **todo**: B fails, or the census was not run.

A measure above `T`, a test of class (iv) and an outcome change are findings of the measurement, not misses: they are what
the item asks for.

## Limits (not covered)

- **The ignored heavy records** of the 34 binaries (about 55 tests, 80 to 200 s each, the `16^3` traces among them) are not
  run in the census: their fast companions cover the same code paths (house rule 7), but their recorded numbers under the
  switch are not measured. A follow-up is proposed for them if the census shows a class (iv) change or a measure above `T`.
- The census covers the driver entries listed; a test that builds its own Newton iteration on `internal_force` and `tangent`
  with states it commits itself is covered at the test level only (pass or fail), and by arithmetic 1 where those states are
  the initial ones.
- One switch only: the warm start alone (ADR 0725's WS). The combination W of ADR 0660 is not measured as a default.
- The value groups are the drivers' outputs; derived measures of a test (a punch reaction, an energy) are judged by the test's
  own assertions under the flip (NT), not recomputed.
- The static driver has no file-level save; the transient integrator's byte path is the only save and restart of element
  states measured at the byte level (arithmetic 5).
- No default is changed: the switch is the maintainer's decision.

## Open questions (taken or open)

1. **The design** (the census by a flipped constructor in a scratch copy as the "test-only copies of each case", the
   instrument and its entries, the value groups, `T = 1e-6`, the outcome and class rules, the B cases, the file name): taken
   on the maintainer's standing instruction of 2026-11-11 (take the recommendation).
2. **Whether the warm start becomes the default**: not taken (it changes default results, two bit records and the length of
   the committed states); open for the maintainer, informed by this measurement.

## Amendment 1 (2026-11-17, before the comparison of the census)

Written while the census runs of both copies were under way and before any record of them was compared (`compare.py` had not
been written or run); the in-process file had been run once (its registered run) and its mutations made.

1. **SL is defective as registered.** Its clause "base has no such extra" assumes every model of the base copy is a default
   model. It is not: eleven of the 34 binaries call `set_incompatible_mode_warm_start` themselves (`si_eas_hg8` to
   `si_eas_hg14`, `si_eas_hg20`, `si_eas_hg21`, `si_eas_hg_smooth`, `si_eas_local_update_f844`; found by `grep -rn
   set_incompatible_mode_warm_start crates/rcfd-fem/tests`), with `true` for their warm variants and with a variable that is
   `false` for some default variants. A model that sets the flag itself, either way, is the same model in both copies: the
   flip of the constructor's default cannot reach it. SL is kept and evaluated as registered (it will miss on those records,
   by construction). The corrected rows, used for the reading:
   - **SL'**: every paired `ok/ok` record with states falls in one of two classes: **switched** (every Incompatible element's
     state in flip is base's plus 9 values, every other element's equal) or **explicit** (every element's state length equal
     in both copies: the model sets the flag itself). Limit: no record in neither class.
   - **CX** (a new control): every explicit record: `0` values differ in every value group, and the same counts. Expected:
     the flip does not reach a model that sets the flag (reading c1).
   - The measures N4, N5, N6 are reported over the switched records; the explicit records are listed by count only.
   Records without states (an `err` in either copy, or an entry without states) are classified by their test.
2. **CF1's second clause cannot be evaluated**: the records do not hold a solve's input states, so "a `solve_increment` /
   `solve_from` from states with no committed modes" is not identifiable from them. CF1 is evaluated on its first clause
   (the `solve_with` records of one increment and no halving); the second is recorded as not run.

By the registered decision rule a row met only after an amendment, or not run, makes the outcome **partial**; the reason
will be stated in the results.

## Addendum 1: results (2026-11-17)

Logs in `w49i-work/f846-scratch`: `run1-b.log` (the new file's three default tests, its single registered run on 3e14b06b,
0.4 s), `mutations.log` and `mut-MU*.log`; the census: `logs-base/`, `logs-flip/` (one libtest log per binary, `_build.log`,
`_summary.log`), `dump-base/`, `dump-flip/` (the records, 19 MB and 25 MB), `census-compare.log` (the output of
`compare.py`); `instrument.py`, `run_census.sh`, `compare.py`; `s7-*.log`, `b-src-diff.log`, `bh-worktree.log`,
`bh-flip.log`, `s8-docs.log`. The two census copies are `git archive` of 3e14b06b with the instrument; they differ in the one
line of the flip (checked by `diff -r`). Each registered run was made once. No limit was changed after a run. Amendment 1 was
written and committed before the census was compared.

### The census: what the switch moves in the existing tests

Both copies ran the default tests of all 34 binaries (base 4 min, flip 5 min, after a 30 s build each). **Base: every
binary passed** (C0). The records: base 321, flip 305; 305 paired, 16 base-only (the four tests below that stop in flip
after their first solve). 23 binaries make records. 11 make none: `si_eas_default_bits`, `si_eas_hg2`, `si_eas_hg3`,
`si_eas_hg18`, `si_eas_hg20`, `si_eas_hg21`, `si_fbar`, `si_hex8x`, `si_hex8x_estimate`, `si_hex8x_linear`, `si_scaffold`.
Their default tests call no driver on a model with an Incompatible Hex8, or only element-level and assembled entry points from
the initial states.

| Paired records | Count | Reading |
|---|---|---|
| `ok` in both, **switched** (flip's states carry the nine modes) | 140 | the switch acts (binaries `si_eas`, `si_eas_hg`, `si_eas_hg3_bits`, `_rd`, `_rl`, `si_eas_hg4` to `si_eas_hg7`, `si_eas_hg14` to `si_eas_hg17`, `si_eas_hg19`, `si_eas_hg_default_bits`) |
| `ok` in both, **explicit** (equal state lengths: the model sets the flag itself) | 119 | the switch cannot act (`si_eas_hg8` to `si_eas_hg13`, part of `si_eas_hg14`, `si_eas_hg_smooth`, `si_eas_local_update_f844`): **0 values differ, 0 counts differ** (CX) |
| `err` in both, same error text | 33 | unchanged failures (`si_eas_hg8` 4, `si_eas_hg13` 3, `si_eas_hg14` 7, `si_eas_hg19` 10, `si_eas_local_update_f844` 9) |
| `err` in base, `ok` in flip | 13 | outcome changes (N6) |
| `ok` in base, `err` in flip | **0** | |

**c4 (N4).** Of the 140 switched records, **112 change bits**. Over the 137 whose outcome did not change, 102 714 of 259 875
recorded values differ (displacement, velocity, acceleration, reactions, internal force, load factors, the material part of
the states). The 28 that do not move are 9 solves of one increment (CF1) and 19 records where the modes are reached from
`alpha = 0` and from the committed modes in the same rounding: `si_eas_hg14` (3 `solve_from`), `si_eas_hg15` (12
`solve_increment`), `si_eas_hg19` (4 solves of 10 increments). Per binary the records that change: `si_eas` 7, `si_eas_hg` 6,
`si_eas_hg3_rd` 6, `si_eas_hg3_rl` 6, `si_eas_hg3_bits` 1, `si_eas_hg4` 8, `si_eas_hg5` 14, `si_eas_hg6` 14, `si_eas_hg7` 28,
`si_eas_hg15` 7, `si_eas_hg16` 7, `si_eas_hg17` 3, `si_eas_hg19` 2, `si_eas_hg_default_bits` 3.

**c5 (N5).** The largest relative change of each value group over the switched records without an outcome change, against
`T = 1e-6`:

| Value group | Largest `|b - a|_inf / |a|_inf` | Record | Within `T` |
|---|---|---|---|
| displacement `u` | `7.8e-11` | `si_eas_hg15` `f779_fast_amended`, `solve_increment` 15 | yes |
| velocity `v` | `7.9e-18` | `si_eas` `nt1_strain_energy_of_an_incompatible_bar` | yes |
| acceleration `a` | `4.8e-17` | the same | yes |
| reactions | `6.6e-14` | `si_eas_hg4` `pc_cantilever_against_fbar`, solve 3 | yes |
| internal force | `1.6e-11` | `si_eas_hg15` `f779_fast_amended`, `solve_increment` 15 | yes |
| load factors | `0` | (equal wherever the increments are equal) | yes |
| states, material part | `8.5e-15` | `si_eas` `rf1_incompatible_taken_after_f596_amended`, `solve_in_time` (a history material) | yes |

**No measure is above `T`**; the largest is 13 000 times below it. (The residuals at convergence, recorded but not a value
group, are rounding noise: their relative change reaches `4e-3` on values of `1e-16`.)

**c6 (N6): the 16 solves whose outcome changes, every one toward convergence.**

- 13 solves that fail in base converge in flip: the inverted points of the distorted `8^3` block DB4 (element 476 at
  `0.675`; `si_eas_hg5` 1, `si_eas_hg6` 2, `si_eas_hg7` 1) and of the distorted `4^3` block (element 53 at load step 7;
  `si_eas_hg5` 1, `si_eas_hg6` 2); the backtracking alone on DB4 (element 475, `0.8906`) and the seeds of ADR 0658 under the
  opt-ins of `si_eas_hg7` (elements 420 and 476; `si_eas_hg7` 5 in all); and the punch block of
  `si_eas_hg4` `pb_punch_block_against_fbar` (base: a singular tangent after 5 halvings in load step 10), which converges.
- 3 solves that converge in both need fewer halvings in flip: `si_eas_hg6` `ca_cause_of_the_inverted_point` solve 3 (12
  increments with 5 halvings to 10 with none), `si_eas_hg6` `plain_element_4_cubed_reported` solve 2 (2 halvings to none),
  `si_eas_hg7` `r_and_s_on_the_four_blocks` solve 9 (2 halvings to none).
- 1 iteration change without an outcome change: `si_eas_hg6` `plain_element_4_cubed_reported` solve 3 (one increment 5
  Newton iterations to 4).

No solve that converges in base fails in flip.

**NT: the 8 tests whose outcome changes** (all pass in base and fail in flip; none the reverse):

| Class | Tests | The failing assertion |
|---|---|---|
| (i) a bit record of the default path | `si_eas_hg_default_bits` `b1_...`, `si_eas_hg3_bits` `b3_...` | "B1 ADR 0608 Q1 block Incompatible solve: digest changed"; "B3 opted CB2 cantilever 25 x 1 x 1 alpha 1 solve: digest changed" |
| (ii) a control of an opt-in against the default | none | the tests with such controls (`si_eas_hg8`, `si_eas_hg9`, `si_eas_local_update_f844`) set the flag on every variant themselves |
| (iii) an assertion on the default's outcome | `si_eas_hg6` `db4_met_rows`, `si_eas_hg7` `r_and_s_on_the_four_blocks` | NC1 "default: solve failed", "error names element 476", the failing load factor in `[0.67, 0.68]` (now NaN: no failure); in `si_eas_hg7` also NCB "backtracking alone on DB4: solve failed" |
| (iv) other | `si_eas` `pt1_to_pt4_pm_ps_patch_at_finite_strain`, and its copies `si_eas_hg` `r_pt1_...`, `si_eas_hg3_rd` `r_pt1_...`, `si_eas_hg3_rl` `r_pt1_...` | `called Result::unwrap() on an Err value: FieldLength { key: "state", expected: 0, found: 9 }` |

**The class (iv) finding is not a change of accuracy but a refusal.** The four patch tests hand the driver's committed states
(`NonlinearResult::states`) to the public element entry point `nl_incompatible::modes_and_strains` to read the modes.
Every public element-level entry point of the element (`modes_and_strains`, `modes_and_strains_with`,
`condensed_force_and_tangent*`, `mode_iteration_record*`, `trust_region_record`, `enhanced_force_and_tangent*`, and the
`nonlinear::hex8_*` entries with the Incompatible formulation) checks the state against the material layout (`check_state`,
`points x state_size` values). A warm state has nine more, so they refuse it. Under the switch every caller that reuses the
driver's committed states at the element level breaks, until those entries accept a warm state: cut it to its material
part, as ADR 0668 D1 did for the creep and viscoelastic readings, and start the local iteration from the modes, as the
crate-internal `modes_and_strains_warm` of ADR 0666 D2 does.

**c3 (BH-F).** `baseline_hashes` on the flip copy passes (3 passed, the registered ignored one ignored). No rcfd-api baseline
case uses the element (reading c3), so the switch moves none of them.

### The in-process file and the save and restart paths (B)

All of B's rows were met in its single run (`run1-b.log`):

- DF: the nine digests of the copied cases equal the records' `EXPECTED` (the copies are faithful).
- Z1: the patches' tangent and force at the initial states and the E state: `0` values differ. Z2: the first increment's
  residuals of the three solves: `0` bits differ (arithmetic 1 holds).
- WB: the Q1 block (4 increments, 16 Newton iterations in both), the CB2 cantilever and the opted CB2 cantilever (5
  increments, 29 iterations in both) keep their increments, halvings and iterations. Values differing: `u` 50 of 192, 293 of
  312, 293 of 312; reactions 73, 12, 11. Largest relative change: `u` `9.3e-17`, `1.8e-15`, `1.3e-15`; reactions `2.5e-16`,
  `9.5e-16`, `2.2e-15`. The NT1 bar (3 steps, 2 iterations each in both): 26 of 61 recorded values, `v` 25, `a` 26; `u`
  `2.7e-18`, `v` `7.9e-18`, `a` `4.8e-17`, the strain energy `1.4e-16`. NCW: 293 values of `u` differ on CB2.
- WL: every warm state has 9 values (`state_size 0`), every default state 0.
- **The transient save and restart (c7).** R1: every element of the warm state at step 3 has 9 values. R2: through
  `to_bytes` (2320 bytes) and `from_bytes`, `0` values differ. **The nine modes survive the bytes.** R3: restored into a
  fresh warm integrator, 3 more steps against the uninterrupted 6: `0` values differ in `u`, `v`, `a`, the internal force and
  the states. **The restart is bit-identical.** NCR1: the default integrator refuses the warm state, with the text "material
  states of 4 elements that do not match the model's layout (4 elements)". The refusal is right, but the text names the
  element count where it means the per-element lengths (remaining question 3). NCR2: the warm state without its modes is
  accepted and moves 58 values of `u`, `v`, `a`, so the restart uses the saved modes. RM (migration): a default state, saved
  before a switch, is accepted by the warm integrator, and after 3 steps differs from the warm reference in 30, 28 and 15
  values of `u`, `v`, `a`, by `1.1e-17`, `1.1e-17` and `1.1e-16` relative. Every element then carries its modes.
- **The static restart (c7).** S1: the chain of four `solve_increment` calls from the committed states equals `solve` with 4
  increments bit for bit (`u` and states). S2: every element's state carries the nine modes after every link. NCS1: the
  default model refuses the warm states (`element 0: key 'state': length 9, expected 0`). NCS2: with the modes cut off at the
  second link, 53 values of the final `u` move. SM (migration): the warm model accepts the default states and converges;
  53 of 192 values of `u` differ from the warm reference, by `9.3e-17`. There is no file-level save of the static driver's
  states in the repository (arithmetic 5): the in-memory restart is the static path.

### The checked rows

| Id | Limit | Measured | Met |
|---|---|---|---|
| C0 | every base test passes | 34 binaries, every default test passed | yes |
| C1 | `si_eas_default_bits` passes in flip | passed (and makes no record) | yes |
| C2 | `si_eas_hg_default_bits` fails in flip | fails (B1 Q1 digest changed) | yes |
| CF1 | `0` values differ (first clause) | 9 records, `0` | yes |
| CF1, second clause | `0` | **not run** (the records hold no input states; Amendment 1) | not run |
| SL (as registered) | holds for every element of every record | **missed on 119 records**, the explicit ones, by construction; holds on all 140 switched | **no** |
| SL' (Amendment 1) | no record in neither class | `0` | yes, after the amendment |
| CX (Amendment 1) | `0` differences on the explicit records | `0` of 119 | yes, after the amendment |
| N0, N4, N5, N6, NT | reported | above | reported |
| BH-F | passes | passed (3 passed, 1 ignored as registered there; 126 s with the build) | yes |
| DF | equal | 9 of 9 equal | yes |
| Z1 | `0` | `0`, `0`, `0`, `0` | yes |
| Z2 | `0` | `0` on each of 3 | yes |
| NCW | `> 0` | 293 | yes |
| WL | holds | holds (4 cases) | yes |
| R1 | holds | holds | yes |
| R2 | `0` | `0` | yes |
| R3 | `0` | `0` | yes |
| NCR1 | refused | refused | yes |
| NCR2 | accepted; `> 0` | accepted; 58 | yes |
| RM | accepted; reported | accepted; `1.1e-16` largest (within `T`) | yes |
| S1 | `0` | `0` | yes |
| S2 | 9 values | 9 at every link | yes |
| NCS1 | refused (key `"state"`) | refused, key `'state'` | yes |
| NCS2 | `> 0` | 53 | yes |
| SM | accepted; reported | accepted; `9.3e-17` (within `T`) | yes |
| B | empty | empty (`b-src-diff.log`, 0 bytes) | yes |
| BH | passes | passed (3 passed, 1 ignored as registered there) | yes |
| S6 | clean | `cargo fmt -p rcfd-fem --check` and `cargo clippy -p rcfd-fem --release --all-targets -- -D warnings` clean | yes |
| S7 | pass | release: `--lib` 15 passed; `si_eas_hg6` 4 passed (2 ignored), `si_eas_hg7` 4 (2), `si_eas_hg8` 5 (2), `si_eas_local_update_f844` 4 (3); the new file 3 passed | yes |
| S8 | pass | `cargo test -p rcfd-cli --release --test docs_site`: 2 passed | yes |
| S9 | builds | 36e89fee (docs), 3e14b06b (built for the runs), 595dbd58 (docs), the results commit | yes |

### Mutations (each on a scratch copy of 3e14b06b, run once against the new file's three tests)

| Id | Caught | By |
|---|---|---|
| MU1 (the warm builder ignores the flag) | yes | NCW (`0`), WL (4 cases), R1, S2; and NCS1, NCS2, NCR1, NCR2 |
| MU2 (the transient restart restores without the modes) | yes | R3 (133 values differ); NCR2 (`0`) |
| MU3 (the default Q1 copy with 5 increments) | yes | DF (digest `0x208b96098c8727b8`) |
| MU4 (the bit counter compares a vector with itself) | yes | NCW, NCS2, NCR2 (`0`) |
| MU5 (the static chain from the initial states at every link) | yes | S1 (lengths differ), S2, NCS1, NCS2 |
| MU6 (the census without the flip) | realised | the base run: C2's binary passes there and fails in flip |

### Deviations

- SL was defective as registered (Amendment 1). It missed on the 119 records of models that set the flag themselves, and
  SL' and CX replace it for the reading.
- CF1's second clause was not evaluated (Amendment 1).
- Estimate 4 was wrong in two places. Class (ii) is empty, because the tests with such controls set the flag on every
  variant. Class (iv) is not empty: the four patch tests, a refusal rather than a change of accuracy.
- The census of 34 binaries' default tests is wider than a usual per-item check (house rule 7), but it is the measurement
  the item asks for, and it took 9 minutes in all. The ignored heavy records were not run (Limits).

### Not run

The ignored heavy records of the 34 binaries (Limits); CF1's second clause. The whole rcfd-fem suite was not run (rule 7;
the daily gate).

### Decision

**partial**, by the registered rule: a limit row (SL) missed as registered, two rows (SL', CX) were met only after Amendment
1, and a clause of a row (CF1, second) was not run. The cause is a defect of the registration, not of the measurement:
SL assumed that every base model is a default model. Every other row was met, every mutation registered as catchable was
caught, and every clause of the acceptance was measured on the default tests: c4 (112 of 140 switched results change bits),
c5 (every measure within `T`, largest `7.8e-11`), c6 (16 solves change outcome, all toward convergence, none the reverse; 8
tests change outcome, 4 of them by a refusal of the warm state at the element entry points), c7 (the nine modes survive the
bytes and both restarts bit for bit; the default refuses them), c8 (no library line changed; `baseline_hashes` unchanged).

### Remaining questions

1. **The default switch** (open, for the maintainer; not taken because it changes default results, two bit records and
   existing tests). What it would move, measured: the last bits of 112 of the 140 switched results (largest relative change
   `7.8e-11`); 13 failing solves that converge and 3 with fewer halvings; the records B1 and B3 (class i, a re-record needing
   approval); two default-outcome rows (class iii: `si_eas_hg6` NC1, `si_eas_hg7` NC1 and NCB, which would have to set the
   flag to `false` themselves, an edit of existing tests); and the four patch tests of class (iv), which break until the
   element entry points accept a warm state. Options: (a) switch now, with those edits; (b) first make the public element
   entry points accept a warm state (the follow-up below), then switch with (a)'s record and test edits; (c) keep the opt-in.
   Recommendation: (b). The refusal of class (iv) would otherwise reach every caller that reads the driver's states at the
   element level, inside and outside the tests.
2. **The heavy records under the switch.** They were not measured. Options: (a) a separate item; (b) run them as part of the
   switch item, which has to run them anyway. Recommendation: (b). The fast companions moved by at most `7.8e-11`, with no
   accuracy class change.
3. **The text of the transient restore's refusal** names the element count ("material states of 4 elements that do not
   match the model's layout (4 elements)") where the per-element lengths differ. Options: (a) name the first element and
   its two lengths in a later item; (b) leave it. Recommendation: (a), with the follow-up below; it changes an error text, not
   a result.

## For the lead: roadmap entries

> Proposed status: partial.

> F-846 (ADR 0727, 2026-11-17): what switching the default local mode update of the incompatible Hex8 to the warm start of ADR 0660 D1 would move, measured test-only, no default changed. A census ran the default tests of all 34 test binaries of the element on two scratch copies, one with the constructor's default flipped, and recorded every outermost driver solve on a model with the element (321 records; rcfd-api has no baseline case with the element, and baseline_hashes passes on the flipped copy). Of the 140 results the switch reaches, 112 change bits, by at most 7.8e-11 relative (registered tolerance 1e-6). The 119 results of models that set the flag themselves do not move. Sixteen solves change outcome, every one toward convergence: 13 that fail today converge (the inverted points of the distorted blocks and the ADR 0658 seeds, a singular punch tangent) and 3 need fewer halvings; none that converges fails. Eight tests would fail: the two default bit records (B1, B3), two rows asserting the default's failure on DB4, and four patch tests. Those four break because the public element entry points (modes_and_strains and the others) refuse a committed state with the nine modes. The modes survive the transient save through bytes and both restarts bit for bit, and the default model refuses them. Partial because the state-length row was registered too strictly (it assumed every base model was a default one, corrected by an amendment before the comparison) and one clause of a control could not be evaluated. Five of five mutations of the in-process file caught; the heavy records not run.

> F-844 (ADR 0725, 2026-11-17), add at the end of its note: ADR 0727 measures what the switch would move in the existing tests: no measure beyond 7.8e-11 relative, 16 solves toward convergence and none away, and four tests that break because the element entry points refuse a warm state.

> Follow-up proposed: The element entry points of the incompatible Hex8 given a committed state with the nine modes of the warm start. Deps F-846. Acceptance: Every limit not stated here is registered in the ADR before the run. The public element-level entry points of the incompatible Hex8 (modes_and_strains and its variants, condensed_force_and_tangent and its variants, the mode iteration records, enhanced_force_and_tangent and its variants, and nonlinear::hex8_* with the Incompatible formulation) accept a state of the material layout followed by nine modes, by an opt-in or by a rule that leaves every current call bit-identical: the material part used as now and the modes as the start of the local iteration, measured against the driver's own evaluation on the ADR 0727 patch tests under the warm start; the transient restore's refusal names the first element and its two lengths; the default bits unchanged.
