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
