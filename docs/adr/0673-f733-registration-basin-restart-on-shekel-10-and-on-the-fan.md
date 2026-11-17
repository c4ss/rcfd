# ADR 0673: F-733 registration: the basin restart on Shekel 10 and on the reduced-order fan, a check on its fresh process, and a hash of the default O1 points

Status: **REGISTERED 2026-11-17, written before any code of F-733 and before any run of its checks.** Written by the implementing
agent on branch `f733-work` (from 5f501379, which has F-698 / ADR 0621, F-715 / ADR 0651 and F-716 / ADR 0645 merged), worktree
`w49i-work/f733`, scratch `w49i-work/f733-scratch`. Every choice the maintainer would normally be asked about is taken on the
maintainer's standing instruction of 2026-11-11 (take the recommendation). Not yet accepted by the maintainer.

Parents: [ADR 0651](0651-f715-registration-robust-expected-improvement-on-multimodal-functions.md) (F-715: `BayesOptions::restart`,
`Restart { tau: 1e-4, rho: 2.0 }`, the surviving mutation R2, amendment 2 and its check A2d, remaining questions 1, 2 and 4),
[ADR 0621](0621-f698-surrogate-model-gaussian-process-and-expected-improvement.md) (F-698: the R1 fan set-up F3, the O1 run),
[ADR 0645](0645-f716-registration-stall-pre-check-for-the-fan-initial-design.md) (F-716: the pre-check in the same loop; a restart
step has no ranked fallback list; its B1 digest precedent).

## What F-733 asks

The acceptance, binding:

> on Shekel 10 or a registered second multimodal function, and on the F-698 R1 fan set-up, the restart of ADR 0651 with its registered tau and rho reaches the target on at least as many seeds as plain expected improvement, a check decides whether the fresh process of the restart is needed (the surviving mutation R2), and a hash of the default proposal points of the F-698 O1 run pins the default bits.

The brief adds: every limit not stated there is registered here before the run; defaults bit-identical; heavy runs (more than
about two minutes in release) are `#[ignore]`d records run once, with a fast test on the same path; every loop capped, progress
printed per run; negative controls and mutations registered before the run; rerun by name `f715_robust_ei`, `f698_surrogate`,
`f716_stall_precheck`, `f697_fan_objective`, `--lib`, `docs_site`.

## What was read

`AGENT_RULES.md`; the brief; ADR 0651 in full; ADR 0621 (the R1 check, addendum 1); ADR 0645 (status, its B1 digest);
`crates/rcfd-api/src/design/bayes.rs` (`Restart`, `propose_ranked`, `basin_restart`, `minimise`), `tests/f715_robust_ei.rs`,
`tests/f698_surrogate.rs` (O1, the F3 set-up, `r1_r2_reduced_order_fan_record`), `tests/f716_stall_precheck.rs` (the copied F3
set-up, `fnv`, B1); the F-715 scratch mutation script and `mut_R2.log` (read only); `docs/design-loop.md`,
`docs/reference-data.md` (the ADR 0621 / 0651 rows). No paper was opened.

**Shekel 10, from memory (NOT read).** Dixon and Szego (1978), in the usual form: on `[0, 10]^4`,
`f(x) = - sum_{i=1}^{10} 1 / (sum_{j=1}^{4} (x_j - C_ij)^2 + beta_i)`, `beta = (1, 2, 2, 4, 4, 6, 3, 7, 5, 5) / 10`, the rows of
`C`: (4,4,4,4), (1,1,1,1), (8,8,8,8), (6,6,6,6), (3,7,3,7), (2,9,2,9), (5,3,5,3), (8,1,8,1), (6,2,6,2), (7,3.6,7,3.6); the
global minimum about `-10.5364` near (4,4,4,4). Both the constants and the minimum are from memory; check S0 verifies the
minimum numerically (a local minimisation from (4,4,4,4) and 40 seeded multistarts), and `docs/reference-data.md` labels it
"from memory".

## Disclosed: scratch prototype, smoke runs and one base computation, before this registration

As in ADR 0651, the candidates were tried in a scratch copy (`f733-scratch/proto`, a `git archive` of 5f501379, target
`f733-scratch/target`) with a scratch test `zz_f733_smoke.rs`; logs `f733-scratch/logs`. Nothing of it is in the branch before
this commit. The prototype adds `Restart::step_model` (D1 below). No seed of a registered check (1 to 20) was used for Shekel or
the fan.

| Smoke | Set-up | Result |
|---|---|---|
| Z0 | Nelder-Mead on Shekel 10 from (4,4,4,4) and 40 seeded starts | `-10.53644315` at (4.000747, 3.999509, 4.000747, 3.999509) from (4,4,4,4); the 40 starts find nothing lower; `f(4,4,4,4) = -10.53628` |
| Z1 | Shekel 10, `n0 = 20`, budget 150, seeds 101 to 110: plain EI; restart (fresh, ADR 0651); restart scored by the step's own process (the R2 variant) | 10, 10 and 10 of 10 (first hits 49 to 136); about 30 s per run |
| Z2 | as Z1 with `n0 = 10`, seeds 111 to 120 | plain 10 of 10 (36 to 144), fresh **10 of 10** (36 to 142), step **7 of 10** (111, 113, 116 stay at -2.87, -2.79, -4.07 with 90 to 112 restart picks) |
| Z3 | the F-698 R1 set-up (F3, `bayes::run`, budget 50), seeds 101 to 105: plain; fresh; step | every run bit-identical across the three (0 restart picks); best / `P_ref` 0.99311 to 0.99360, target at 16 to 20 |
| Z4 | Z3 at budget 100, seeds 101 to 103, plain and fresh | identical again, 0 restart picks; the smallest best-acquisition over the spread of the `Ok` values per run 8e-29 to 1.5e-22, so the trigger condition holds |
| Z5 | diagnosis of Z3 (seeds 101 to 103, plain, picks 20, 30, 40, 49) | fitted length scales 2.2 to 10 in the unit cube (the power over the feasible region is nearly linear; the optimum lies on the stall boundary); 11 to 16 `Ok` points, **none** at `delta >= 2` from the incumbent, so `basin_restart` returns at its `outside.len() < d + 2` test every time |
| Z6 | the O1 points hash (H1 below) computed on an unmodified `git archive` of 5f501379 | `0xe2cf141320774cfc` over 19200 bytes (20 seeds x 60 points x 2 coordinates x 8 bytes), 41 s |

Z5 is the expected cause on the fan: with `rho = 2` (ADR 0651, not changed here) a basin measured in length scales of 2 to 10
covers the whole unit cube, as ADR 0651 mutation R5 found for a unit-cube radius. The restart is therefore inert on the R1
set-up, and the fan clause is met by equality of runs, not by a rescue (registered as such: F1b, and in the limits).

## Decision

### D1. The R2 variant as an opt-in field (`Restart::step_model: bool`, default `false`)

To let a test decide R2 without mutating the code, `Restart` gains `step_model`. With `false` (the default of
`Restart::default()`) `basin_restart` is the code of ADR 0651 unchanged. With `true` the fresh process is not fitted and the
restart candidates are scored by `EI(m, s; f')` from the step's own objective process (the one of the plain pick, with its
imputed points), with the same basin exclusion, the same `f'` (best feasible value outside the basin), the same feasibility
factor and the same tie rule: exactly the scoring of ADR 0651 mutation R2 (`rs.models.f.predict(u)` in place of
`fresh.predict(u)`), except that the unused fresh fit is not computed (with `step_model` a fit failure of the fresh process
can no longer cancel a restart). The recorded mean, sd and EI of a pick come from the process that scored it. The pick kind
stays `restart`.

### D2. What does not change

`BayesOptions::default().restart` stays `None`; with `restart: None` nothing new is reached, and with `Some(Restart::default())`
the restart is that of ADR 0651 bit for bit (D1). No existing test file is edited; no golden, hash table or `Cargo.lock`
change; no MCP tool, case key or study method. The only library file touched is `crates/rcfd-api/src/design/bayes.rs`.

### D3. The hash of the default O1 points

"The default proposal points of the F-698 O1 run": `bayes::minimise` on Branin (`[-5, 10] x [0, 15]`) with
`BayesOptions { seed: s, n0: Some(10), ..Default::default() }` (the ADR 0621 O1 options, `opts(s, 10)`), budget 60, seeds 1 to
20 in order, one thread; every evaluated point in order, each coordinate as `f64::to_bits` little-endian bytes, hashed with
FNV-1a 64 (the `fnv` of ADR 0645). The constant is the one computed on 5f501379 (Z6), which carries the F-698 defaults as
F-715's A2d and F-716's B1 kept them.

## Checks (limits fixed now)

New test file `crates/rcfd-api/tests/f733_restart_second_function.rs`. Release profile. "Plain" is `restart: None`, "fresh"
is `Some(Restart::default())`, "step" is `Some(Restart { step_model: true, ..Restart::default() })`. Shekel target:
`f <= -10.5364 + 0.01 * 10.5364 = -10.431036`. Fan target: best feasible `<= P_ref (1.01)`, `P_ref = 1.455477 W` (ADR 0621
addendum 1). E2 (in the box, no point within 1e-6 of an earlier one, unit cube) applies to every loop. The A6 basin test of
ADR 0651 (every restart pick at `delta >= rho` from the step's incumbent, in the recorded length scales) applies to every
restart pick of every loop driven through `propose`.

| Id | Clause | What | Limit | Estimate |
|---|---|---|---|---|
| S0 | reference | Shekel 10: Nelder-Mead from (4,4,4,4) (4000 evaluations, xtol 1e-12) and from 40 seeded starts (seeds 0 to 39) | the minimum found within `1e-4` of `-10.5364`; no start lower than it by more than `1e-9` | Z0: `-10.536443`, 4.4e-5 from the 4-decimal value |
| S1 | acceptance 1 | Shekel 10, `n0 = 10`, budget 150, seeds 1 to 20, through `propose`: seeds at the target, plain and fresh | fresh `>=` plain | Z1, Z2: 20 of 20 each |
| S2 | acceptance 3 (R2) | the S1 runs and the step variant on the same seeds: seeds at the target | fresh `>=` step `+ 3` (fresh needed) | Z2: 10 against 7 on ten seeds, so about 20 against 14 |
| S3 | design | every restart pick of the fresh and step runs of S1/S2 outside the basin (A6); E2 over all 60 runs | 0; 0 | ADR 0651 A6 |
| S1r | record | per seed and variant: first hit, best, restart picks | recorded | |
| F1 | acceptance 2 | the F-698 R1 set-up (F3, `bayes::run`, `spec_f3`, budget 50, default `n0` 15), seeds 1 to 10, plain and fresh: seeds at the fan target | fresh `>=` plain | Z3: identical runs; F-698 and F-716 NC1: plain 10 of 10 |
| F1b | design | the same runs: every entry's parameters bit-equal between plain and fresh, and the count of restart picks | bit-equal on every seed; recorded | Z3, Z5: the basin covers the box |
| NC1 | control | the plain F1 runs: failed counts per seed | exactly `[37, 35, 34, 36, 36, 36, 39, 36, 34, 36]` (ADR 0645 NC1, the F-698 log) | same set-up |
| F1c | record | the step variant on the same set-up and seeds (hits, restart picks); per fresh run the number of steps whose plain acquisition is below `tau` times the spread of the `Ok` values so far, and the largest count of `Ok` points at `delta >= rho` from the incumbent over the picks | recorded | Z4, Z5 |
| F1d | design | every failed fan entry `diverged` with a reason naming its point (ADR 0621 R1); E2 over the fresh runs | all; 0 | |
| H1 | acceptance 4 | the hash of D3 on the F-733 code | `== 0xe2cf141320774cfc` | Z6, D2 |
| NCH | control | the same hash with `restart: Some(Restart::default())` (else as D3) | `!= 0xe2cf141320774cfc` | ADR 0651 A2d: seed 2 differs |
| T1 | fast path | Shekel 10, `n0 = 10`, budget 40, seed 1, fresh and step through `propose`: restart picks in each; A6; E2 | `>= 1` each; 0; 0 | Z2: 51 to 112 restart picks per 140 model picks |
| T2 | fast path | F3, `bayes::run`, budget 45, seed 1, plain and fresh: entries bit-equal; restart picks of fresh; failures located | bit-equal; 0 (recorded); all | Z3 |
| T3 | D1 | `Restart::default().step_model` is `false`; Branin (`n0 10`, budget 60, seed 2, `minimise`) fresh and step: the points differ; the step run's restart picks (through `propose`) satisfy A6 | false; differ; 0 violations | the restart fires on seed 2 (ADR 0651 A8b) |
| B1 | regression | `f715_robust_ei`, `f698_surrogate`, `f716_stall_precheck`, `f697_fan_objective` (non-ignored tests, unedited), `cargo test -p rcfd-api --release --lib`, `docs_site` | pass | D2 |
| B2 | rules | `cargo fmt -p rcfd-api --check`; `cargo clippy -p rcfd-api --all-targets -- -D warnings`; `git diff 5f501379 --stat` touches only `bayes.rs`, the new test, this ADR, `docs/design-loop.md`, `docs/reference-data.md` | clean; as stated | |

S0 to S3 run in one `#[ignore]`d heavy record (60 Shekel runs, about 5 minutes with ten threads on the shared machine), F1 to
F1d in another (30 fan runs, about 2 to 3 minutes); each is run once. T1 to T3, H1 and NCH are the fast tests on the same code
paths (H1 and NCH about 40 s each). Each run prints one line when it finishes; every loop is capped by its budget.

Reading taken for S2 (fixed now): S2 is the check that decides R2. Met: the fresh process is needed (the R2 variant loses at
least three of twenty seeds), and R2 is caught. Missed: the fresh process is not shown to be needed on Shekel 10; the test
becomes `s2_..._known_miss` with the limit intact, R2 is recorded as surviving, and the recommendation is to simplify. Either
outcome decides the clause. The threshold 3 is about 1.5 standard deviations of the difference of two counts of 20 at the
smoke rates (sqrt(20 x 0.3 x 0.7) = 2.0), so a met S2 is not a coin toss.

### Mutations (scratch copy, one at a time, after the registered runs)

| Id | Mutation | Expected to fail |
|---|---|---|
| R2 | the fresh path scored by the step's process (ADR 0651 R2: `fresh.predict` replaced by `rs.models.f.predict` with `step_model` false) | S2 (fresh equals step), T3 (the points no longer differ) |
| R4 | `restart` defaulting to `Some(Restart::default())` | H1 |
| R6 | `step_model` ignored (always the fresh process) | T3 |
| R7 | `Restart::default()` with `step_model: true` | T3 (the default), and T1's fresh run becomes the step run (recorded) |
| R8 | default `candidates_per_dim` 999 instead of 1000 | H1 (any change of the default proposal path) |
| R1 | the trigger never fires | T1 (no restart picks) |

### Decision rule

* **done**: S0, S1, S3, F1, F1b, NC1, F1d, H1, NCH, T1 to T3, B1 and B2 met; S2 decided either way (met, or recorded as a
  known miss as above); R4, R6, R7, R8 and R1 caught, and R2 caught when S2 is met.
* **partial**: S1 or F1 missed (an ignored `*_known_miss` record), or NC1 or F1b missed with a diagnosed cause, or a mutation
  other than R2 survives.
* **todo**: H1, B1 or S0 missed (the default bits moved, an existing result changed, or the reference is wrong).

## Limits (not covered)

* One second function (Shekel 10), at one initial-design size and budget, twenty seeds. The restart's `tau` and `rho` stay those
  of ADR 0651 (fixed on Hartmann 6), as the acceptance asks; they are not tuned for Shekel 10 or the fan.
* On the fan the restart is not exercised (Z5): with length scales of 2 to 10 in the unit cube a basin of radius 2 contains the
  whole box. F1 is met by equal runs; nothing here says what the restart would do on a multimodal fan objective.
* S2 decides R2 on Shekel 10 only; on Hartmann 6 the R2 variant did 10 of 10 against 9 of 10 (ADR 0651 mutation log), which is
  not re-run.
* The hash pins the O1 driver path (`minimise`, no failures, no constraints, batch 1); the ledger `run` path keeps the B1 digest
  of ADR 0645.
* Library only: no study method, MCP tool, case key or golden.

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. Second function: Shekel 10 (named in the acceptance; a narrow global basin among nine others). `n0 = 10` over 20 (Z1 did not
   separate the variants).
2. How the R2 check is made: an opt-in field (`step_model`) rather than a mutation, so the decision is a committed test.
3. The fan clause with an inert restart: run as registered (F1, F1b); a basin radius bounded by the box (for example `rho`
   relative to the largest length scale) is a design change with its own parameters, left as a remaining question, not done
   here.

## Amendment 1 (2026-11-17, after the registered run of the fast tests, before any corrected run): T1 had no restart pick

The registered run of the fast tests (commit 1de27b10, log `f733-scratch/logs/run_fast.log`) met H1, NCH, T2 and T3 and
missed T1: on Shekel 10 seed 1 with budget 40 neither the fresh nor the step run made a restart pick (0 and 0; both best
-2.86724, the same points). The estimate was defective: the smoke counted 51 to 112 restart picks over 140 model picks, but
said nothing about how early the first one comes; within 30 model picks the trigger did not fire, so T1 did not reach the
restart path at all. The limit is not at fault; the set-up is.

T1 stays in the file as the ignored record `t1_shekel10_fast_path_known_miss` (limit intact, measured 0 and 0). Correction
(registered now): `t1b_shekel10_fast_path_budget_150`, the T1 set-up with the budget of the registered Shekel runs (150), seed
1, fresh and step, the same three limits (restart picks `>= 1` each, 0 inside the basin, E2 0). Estimate: every one of the 20
smoke runs at `n0 = 10`, budget 150 (Z2, both variants) had 51 or more restart picks. About one minute (two runs in
parallel), so it stays a fast test. Run once. The decision rule is unchanged with T1b in the place of T1; mutation R1 is run
against T1b.

## Addendum 1 (2026-11-17): results

Code: 1de27b10 (`Restart::step_model` in `bayes.rs`, the test file), a44c52b5 (amendment 1: T1 kept as a known miss, T1b),
e4c0276f (the Shekel record renamed `s0_s3_shekel10_plain_fresh_and_step_known_miss` after its run, limits intact), then the
docs. Each registered check ran once; logs in `f733-scratch/logs` (`run_fast.log`, `run_t1b.log`, `run_s.log`, `run_f.log`,
`run_b1_*.log`, `diag_s1.log`, `mutations.log`, `mut_*.log`). Release profile throughout.

### Results

| Check | Limit | Measured | Met |
|---|---|---|---|
| S0 Shekel 10 minimum | within 1e-4 of -10.5364; no start lower | -10.53644315 at (4.000747, 3.999509, 4.000747, 3.999509), 4.3e-5 from the value from memory; lowest of 40 starts the same | yes |
| S1 Shekel 10, fresh against plain (seeds 1 to 20) | fresh >= plain | **fresh 17, plain 18** (fresh misses 9, 13, 18; plain misses 5, 18) | **no** |
| S2 fresh against step (the R2 decision) | fresh >= step + 3 | fresh 17, step 11 (difference 6); measured from the record's line, its assertion comes after S1 and did not execute | yes: the fresh process is needed |
| S3 restart picks outside the basin; E2 | 0; 0 | 3535 restart picks over the 40 restart runs, 0 inside; E2 0 | yes |
| S1r record | recorded | first hits plain 31 to 136, fresh 31 to 150, step 31 to 147; restart picks 55 to 114 per run | recorded |
| F1 fan, fresh against plain (seeds 1 to 10) | fresh >= plain | 10 and 10 (best / `P_ref` 0.99304 to 0.99351) | yes |
| F1b fresh bit-equal to plain | bit-equal | every seed bit-equal; 0 restart picks | yes |
| NC1 plain failed counts | exactly [37, 35, 34, 36, 36, 36, 39, 36, 34, 36] | the same | yes |
| F1c record | recorded | step 10 of 10, 0 restart picks; per fresh run 8 to 20 steps below the trigger and at most 0 `Ok` points outside the basin | recorded |
| F1d failures located; E2 | all; 0 | true; 0 (78 s for the 30 runs) | yes |
| H1 O1 points hash | == 0xe2cf141320774cfc | 0xe2cf141320774cfc | yes |
| NCH the same with the restart | != | 0xfcb394c67ca8392f | yes |
| T1 Shekel fast path, budget 40 | >= 1 restart pick each; 0; 0 | 0 and 0 restart picks | **no** (defective set-up, amendment 1) |
| T1b (amendment 1), budget 150 | >= 1 each; 0; 0 | 82 and 84; 0; 0 | yes |
| T2 fan fast path | bit-equal; located | true (0 restart picks); true | yes |
| T3 `step_model` option | false; differ; 0 | false; true; 10 step restart picks, 0 inside | yes |
| B1 regression | pass | f715_robust_ei 6 passed (2 ignored), f698_surrogate 19 (3 ignored), f716_stall_precheck 7 (2 ignored, its B1 digest included), f697_fan_objective 22; `--lib` 141 passed (4 ignored); docs_site 2 | yes |
| B2 fmt, clippy, files | clean; as stated | `cargo fmt -p rcfd-api --check` clean; `cargo clippy -p rcfd-api --all-targets --release -- -D warnings` clean; `git diff 5f501379 --stat`: `bayes.rs`, the new test, this ADR, `design-loop.md`, `reference-data.md` | yes |

### Diagnosis of the S1 miss (from the record, and one scratch diagnostic run disclosed here)

The three seeds that plain expected improvement solves and the restart does not (9, 13) or that both miss (18) were rerun in a
scratch copy only to read the pick records (`diag_s1.log`; the registered numbers are those above, nothing was re-decided).
On each of them the restart fires first at evaluation 44 to 53, with the incumbent in a local basin (-2.871, or -2.790), and
then makes 91 to 103 of the remaining picks. The global minimiser lies 5.2 to 5.9 length scales from the incumbent, so it is
outside the excluded basin (the length scales are 0.075 to 0.079 of the unit cube; the basin radius is about 0.15). The
restart does not exclude it, but none of its picks comes within 0.05 (unit cube) of it, where plain expected improvement put
15 and 16 points on seeds 9 and 13 and reached the target at 103 and 91. So on Shekel 10 the trigger does not mean that plain
expected improvement is stuck: its acquisition stays below `1e-4` of the spread while it still drifts into the narrow
global basin (width about 0.03 of the box), and the restart, which takes over most of the remaining budget and scores against
the best value outside the basin (a worse one), spends it near the other local minima. `tau` and `rho` were fixed on Hartmann 6
(ADR 0651) and were not tuned here, as the acceptance asks.

### Mutations

| Id | Mutation | Caught by |
|---|---|---|
| R2 | the fresh path scored by the step's process | T3 (fast); the Shekel record: fresh 11 = step 11, so S2 not met (and S1 fails first, plain 18) |
| R4 | `restart` defaulting to `Some` | H1 |
| R6 | `step_model` ignored | T3 |
| R7 | `Restart::default()` with `step_model: true` | T3 (the default) |
| R8 | default `candidates_per_dim` 999 | H1 (and T3) |
| R1 | the trigger never fires | T1b, T3, NCH (with no restart the hash equals the base) |

Control: the unmutated scratch copy passes every non-ignored test of the file (`mut_control.log`).

### Deviations

* T1 made no restart pick within 40 evaluations; amendment 1 kept it as a known miss and added T1b at budget 150 before its
  run.
* S1 missed: the Shekel record is kept as `s0_s3_shekel10_plain_fresh_and_step_known_miss` with every limit intact. Its S2
  assertion did not execute; S2 is read from the printed counts.
* The F1 record took 78 s, under the two-minute mark; it stays `#[ignore]`d as registered.

### Decision

By the registered rule, with T1b in the place of T1 (amendment 1): **partial**, because S1 was missed. On Shekel 10 the restart
of ADR 0651, with its registered `tau` and `rho`, reached the target on 17 of 20 seeds against 18 of 20 for plain expected
improvement. Every other check was met: S0, S3, F1, F1b, NC1, F1d, H1, NCH, T1b, T2, T3, B1 and B2. S2 decided R2: the fresh
process is needed (17 against 11), and R2 is caught. All six mutations were caught. The fan clause is met by runs that are
bit-equal to plain expected improvement, because the restart never fires there (Z5, F1c). The hash clause is met, and it pins
the default O1 points to the base.

### Remaining questions (taken on the maintainer's standing instruction of 2026-11-11; the recommendation is given in each)

1. The restart hurts on Shekel 10 (one seed net: seeds 9 and 13 lost, seed 5 gained, seed 18 missed by both). Options: (a) leave the restart opt-in and off as it is,
   with this result recorded; (b) a follow-up that changes the trigger so that it fires only when plain expected improvement
   repeats itself (for example the last k picks inside the incumbent's basin) instead of on the size of the acquisition, to
   be registered on Hartmann 6 and Shekel 10 together; (c) tune `tau` and `rho` on Shekel 10. Recommend (a) now and (b) as
   the follow-up below. (c) would fit the parameters to the test functions.
2. The fan never reaches the restart (the basin covers the box). Options: (a) nothing, since the fan objective of F3 is
   unimodal over its feasible region; (b) bound the basin by the box, for example a radius in length scales capped so that
   the ellipsoid covers at most a fixed share of the unit cube. Recommend (a) until a fan objective with more than one basin
   exists (a multimodal fan set-up would be its own item). (b) is a design change with its own parameter.
3. Keep `Restart::step_model`? It is opt-in and only serves the R2 decision. Recommend keeping it as a documented variant (S2:
   11 of 20, worse than the fresh process). No caller depends on it.
4. Default on or off: off (ADR 0651 question 2 still holds; S1 makes the case stronger).

## For the lead: roadmap entries

> Proposed status: partial

> Follow-up of F-715 (ADR 0651, 2026-11-16). ADR 0673, 2026-11-17. PARTIAL (library, opt-in). The basin restart of ADR 0651 with its registered tau 1e-4 and rho 2 was run on a second multimodal function and on the fan. On Shekel 10 (n0 10, budget 150, seeds 1 to 20) it reached the minimum within 1 percent on 17 of 20 seeds against 18 of 20 for plain expected improvement: registered limit missed, kept as a known-miss record. On the seeds it misses (two of them solved by plain expected improvement), the restart fires at evaluations 44 to 53 from a local basin and its 91 to 103 picks never come near the narrow global basin that plain expected improvement still finds later, so a small acquisition does not mean plain expected improvement is stuck. On the F-698 R1 fan set-up (seeds 1 to 10, 50 evaluations) every run is bit-equal to plain expected improvement (10 of 10 within 1 percent of the grid optimum, F-698 failed counts reproduced exactly): the restart never fires, because the fitted length scales (2 to 10 in the unit cube) put the whole box inside the basin. BayesOptions::restart gains Restart::step_model (default false), which scores the restart by the step's own process; on Shekel 10 it does 11 of 20 against 17 for the fresh process, so the fresh process is needed and the surviving F-715 mutation R2 is now caught. A hash of the 1200 default O1 points (Branin, seeds 1 to 20) equals the hash computed on the base 5f501379 and differs with the restart on. Shekel 10 minimum (from memory) confirmed numerically at -10.536443. Six of six mutations caught; a fast check that made no restart pick within 40 evaluations was corrected under a recorded amendment. Not covered: a trigger that detects repetition rather than a small acquisition, a multimodal fan objective, tau and rho beyond Hartmann 6.

> Follow-up proposed: A repetition trigger for the basin restart. Deps F-733. Acceptance: with a trigger registered before the run that fires when the recent picks of plain expected improvement stay in the incumbent's basin (instead of on the size of the acquisition), the restart reaches the target on at least as many seeds as plain expected improvement on both Hartmann 6 (n0 30, budget 200, seeds 1 to 10) and Shekel 10 (n0 10, budget 150, seeds 1 to 20), every F-715 and F-733 check keeps its result, and the O1 points hash of ADR 0673 is unchanged.
