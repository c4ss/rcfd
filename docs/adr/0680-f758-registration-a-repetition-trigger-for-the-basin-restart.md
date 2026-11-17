# ADR 0680: F-758 registration: a repetition trigger for the basin restart

Status: **REGISTERED 2026-11-17, written before any code of F-758 and before any run of its checks.** Written by the implementing
agent on branch `f758-work` (from fcb0e65e, which has F-715 / ADR 0651 and F-733 / ADR 0673 merged), worktree `w49i-work/f758`,
scratch `w49i-work/f758-scratch`. Every choice the maintainer would normally be asked about is taken on the maintainer's
standing instruction of 2026-11-11 (take the recommendation). Not yet accepted by the maintainer.

Parents: [ADR 0673](0673-f733-registration-basin-restart-on-shekel-10-and-on-the-fan.md) (F-733: the Shekel 10 miss S1, its
diagnosis, remaining question 1 (b), the O1 points hash H1 and the hash NCH), [ADR 0651](0651-f715-registration-robust-expected-improvement-on-multimodal-functions.md)
(F-715: `BayesOptions::restart`, `Restart { tau: 1e-4, rho: 2.0 }`, the basin, the fresh process, the A1 and A2b Hartmann 6
records).

## What F-758 asks

The acceptance, binding:

> with a trigger registered before the run that fires when the recent picks of plain expected improvement stay in the incumbent's basin (instead of on the size of the acquisition), the restart reaches the target on at least as many seeds as plain expected improvement on both Hartmann 6 (n0 30, budget 200, seeds 1 to 10) and Shekel 10 (n0 10, budget 150, seeds 1 to 20), every F-715 and F-733 check keeps its result, and the O1 points hash of ADR 0673 is unchanged.

The brief adds: every limit not stated there is registered here before the run; the trigger is opt-in (default off) so defaults
and the O1 hash keep their bits; the trigger's parameters (window length, basin radius) are registered before any run and are
not tuned on the acceptance seeds; every loop capped, progress printed per run; heavy runs (more than about two minutes in
release) are `#[ignore]`d records run once with a fast test on the same path; negative controls and mutations registered before
the run; rerun by name `f733_restart_second_function`, `f715_robust_ei`, `f698_surrogate`, `f716_stall_precheck`, `--lib`,
`docs_site`.

## What was read

`AGENT_RULES.md`; the brief; ADR 0673 in full (registration, amendment 1, addendum 1); ADR 0651 in full;
`crates/rcfd-api/src/design/bayes.rs` (`Restart`, `BayesOptions`, `propose_ranked`, `RestartScope`, `basin_restart`,
`minimise`); `crates/rcfd-api/tests/f733_restart_second_function.rs` (Shekel 10, `propose_run`, `a6_violations`, `o1_hash`,
the constants `H1_BASE` and the NCH value recorded in addendum 1); `crates/rcfd-api/tests/f715_robust_ei.rs` (Hartmann 6, its
minimum, `opts`); `docs/design-loop.md` (the Bayesian optimisation section). No paper was opened.

**Disclosed: nothing was run before this registration.** No prototype, no smoke run, no scratch copy. Every number below that
is not a limit comes from the addenda of ADR 0651 and ADR 0673. The only arithmetic: the window lengths `d + 1` (Hartmann 6: 7,
Shekel 10: 5, Branin: 3) and the bound on the share of restart picks in a stalled stretch derived in D1.

## Decision

### D1. The repetition trigger (opt-in `Restart::trigger: Trigger`, default `Trigger::Acquisition`)

`Restart` gains a field `trigger` of a new enum `Trigger`:

* `Trigger::Acquisition` (the default of `Restart::default()`): the trigger of ADR 0651 D1 item 1, `a* < tau sd(f)`, unchanged
  bit for bit.
* `Trigger::Repetition { window: Option<usize> }`: `tau` is not read. At a proposal step with an incumbent, after the plain pick
  `p` of ADR 0621 (the refined winner, exactly as now), the restart fires when **the plain pick and each of the last `W`
  evaluated observations lie inside the incumbent's basin**:
  * the basin is that of ADR 0651 D1 item 2, unchanged: `delta(u) = sqrt(sum ((u_i - u*_i) / l_i)^2) < rho` in the unit cube,
    `u*` the evaluated feasible point of lowest value (the first in canonical order on ties), `l` the length scales of the step's
    objective process, `rho = Restart::rho` (2 by default, not changed here);
  * "the last `W` evaluated observations" are the last `W` entries of the `observations` slice whose status is `Ok` or `Failed`
    (pending entries are skipped), **in the order given**: evaluation order in `minimise` and in the test loops, ledger order in
    `run`. With fewer than `W` evaluated observations the trigger does not fire;
  * `W = window.unwrap_or(d + 1)`, `d` the number of parameters. **Registered: `W = d + 1`** (`window: None`), so Hartmann 6
    `W = 7`, Shekel 10 `W = 5`, Branin `W = 3`.
  When it fires, everything after the trigger is ADR 0651 D1 items 3 to 5 unchanged (the fresh process outside the basin, or the
  step's process with `step_model`; the same exclusion, `f'`, feasibility factor, tie rule and pick record of kind `restart`).

Derivation of the window (first principles, not fitted). Plain expected improvement repeats itself when it keeps placing
evaluations in the same basin. `d + 1` points in general position are the fewest that span a `d`-dimensional neighbourhood (a
simplex): once a full simplex of consecutive evaluations lies in the incumbent's basin and the next plain pick is there again,
plain expected improvement has sampled that basin in every direction and is still choosing it. A shorter window would fire while
plain expected improvement is still converging along a line; a longer one (for example the `(d + 1)(d + 2) / 2` points of a local
quadratic, 28 on Hartmann 6) would leave the restart nearly unused within the budget.

What follows from D1, before any run:

* While plain expected improvement keeps choosing the basin and nothing improves, every restart pick lies outside the basin
  (by construction) and so empties the window; the next fire needs `W` further evaluations in the basin. The restart therefore
  takes at most one of every `W + 1` picks of such a stretch (Hartmann 6 1 of 8, Shekel 10 1 of 6), against up to 100 of 170
  for the acquisition trigger (ADR 0651 A1r). Plain expected improvement keeps most of the budget; the trigger is conservative
  by design.
* When plain expected improvement drifts out of the incumbent's basin (the Shekel seeds 9 and 13 of ADR 0673, where it reached
  the narrow global basin later), its picks outside the basin empty the window and the trigger stays quiet: this is the
  difference from the acquisition trigger that the acceptance asks for.
* The trigger reads the order of the observations, so with `Trigger::Repetition` `propose` is a function of the observation
  sequence, not of the set (the property of ADR 0621 / 0651 D1 that ledger line order does not matter no longer holds for this
  opt-in variant). Worker counts still give the same bits. With `Trigger::Acquisition` nothing changes.
* In a batch (`batch > 1`) every point of the batch uses the same window (the evaluated observations); the believed points of
  the batch are not in it. The registered checks use batch 1.

### D2. What does not change

`BayesOptions::default().restart` stays `None`; `Restart::default()` has `trigger: Trigger::Acquisition`, and with it the code
path of `basin_restart` is the one of ADR 0673 (the acquisition test first, then the basin). The only library file touched is
`crates/rcfd-api/src/design/bayes.rs`. No existing test file is edited (the existing tests build `Restart` through
`Restart::default()` or `..Restart::default()`, so the new field needs no edit); no golden, hash table, `Cargo.lock`, MCP tool,
case key or study method.

## Checks (limits fixed now)

New test file `crates/rcfd-api/tests/f758_repetition_trigger.rs`. Release profile. "Plain" is `restart: None`; "rep" is
`restart: Some(Restart { trigger: Trigger::Repetition { window: None }, ..Restart::default() })` (fresh process, `rho = 2`).
Targets as ADR 0651 / 0673: Hartmann 6 `<= -3.32237 (1 - 0.01)`, Shekel 10 `<= -10.5364 (1 - 0.01)`. Every run goes through
`propose` in the loop of ADR 0673 (`propose_run`, batch 1, observations in evaluation order), which is the loop of `minimise`.
E2 and A6 as ADR 0673.

**W1 (the trigger condition, checked from outside).** For every restart pick `k` of a rep run: (a) each of the last `d + 1`
observations before it lies at `delta < 2` from the incumbent of `hist[..k]` in the pick's recorded length scales; (b) the plain
pick of that step, recomputed by `propose(hist[..k])` with `restart: None` (the plain pick does not depend on the restart
options), lies there too. Violations counted (a or b).

| Id | Clause | What | Limit | Estimate |
|---|---|---|---|---|
| K1 | acceptance 1 | Hartmann 6, `n0 = 30`, budget 200, seeds 1 to 10, plain and rep: seeds at the target | rep `>=` plain | plain 6 of 10 (ADR 0621 O2, ADR 0651 A2b). The six seeds plain solves hit at 41 to 55; a fire needs 8 consecutive plain picks in the basin and then takes one pick, so a seed converging in the global basin is delayed by at most one in eight picks. The four stalled seeds (2, 6, 9, 10: every pick in the -3.2032 basin from about evaluation 55, ADR 0651 S0d) fire about 18 times in their remaining 145 picks; the acquisition trigger rescued three of them with 44 to 98 restart picks, so 0 to 2 rescues. Expected 6 to 8 |
| K2 | acceptance 2 | Shekel 10, `n0 = 10`, budget 150, seeds 1 to 20, plain and rep: seeds at the target | rep `>=` plain | plain 18 of 20 (misses 5 and 18, ADR 0673). On seeds 9 and 13 plain drifts out of the local basin, which empties the window; a fire elsewhere changes the later data, so a seed can be lost or won by chance. Expected 17 to 19: the thinnest clause |
| P0 | control | the plain runs of K1 and K2 reproduce the records | Hartmann first hits exactly `[48, -, 55, 41, 52, -, 54, 50, -, -]`; Shekel misses exactly seeds 5 and 18 | the default code path (D2) |
| K3 | non-vacuity | restart picks of the rep runs, summed per function | `>= 1` on Hartmann 6 and `>= 1` on Shekel 10 | the stalled Hartmann seeds repeat the basin for 145 picks |
| K4 | design | over every rep run of K1 and K2: W1 violations; A6 (restart picks inside the basin); E2 | 0; 0; 0 | D1 |
| K5 | record | per seed and variant: first hit, best, restart picks; the seeds rep rescues and loses; per rep run the steps at which the condition held and the restart did not fire (fewer than `d + 2` points outside the basin, or no fresh fit) | recorded | |
| D0 | D2 | `Restart::default().trigger == Trigger::Acquisition` | true | |
| H1 | acceptance 4 | the O1 points hash of ADR 0673 D3 on the F-758 code | `== 0xe2cf141320774cfc` | D2 |
| HR | acceptance 3 (the ADR 0651 path) | the same hash with `restart: Some(Restart::default())` | `== 0xfcb394c67ca8392f` (ADR 0673 addendum 1, NCH) | D2: the acquisition trigger unchanged bit for bit |
| NCR | control | the same hash with the rep restart | `!= 0xe2cf141320774cfc` and `!= 0xfcb394c67ca8392f` | on Branin the plain picks cluster in a basin once the target is near (ADR 0651 A4: the acquisition trigger fires 115 times over the 20 seeds), so `W + 1 = 4` consecutive picks in the basin occur |
| T1 | fast path | Shekel 10, `n0 = 10`, budget 150, seed 1, rep, through `propose`: restart picks; W1; A6; E2 | `>= 1`; 0; 0; 0 | about 30 s (ADR 0673 T1b); seed 1 plain converges into a basin |
| T2 | determinism | Branin, `n0 = 10`, budget 60, seed 2, `minimise` with the rep restart, workers 1 and 8 | bit-equal | D1 |
| B1 | acceptance 3 | `f715_robust_ei` and `f733_restart_second_function` (non-ignored tests), `f698_surrogate`, `f716_stall_precheck`, `cargo test -p rcfd-api --release --lib`, `docs_site`, `baseline_hashes` | pass | D2 |
| B1h | acceptance 3 | the heavy records rerun once by name: F-715 `a1_hartmann6_with_the_basin_restart_record`, `a2b_f698_o2_hit_list_unchanged_record`; F-733 `s0_s3_shekel10_plain_fresh_and_step_known_miss`, `f1_reduced_order_fan_plain_fresh_and_step_record` | each printed result equal to its addendum: A1 9 of 10 with first hits `[48, 167, 55, 41, 52, 147, 54, 50, 166, -]` and restart picks `[100, 45, 98, 103, 91, 44, 92, 102, 36, 32]`; A2b the P0 list; S0 to S3 plain 18, fresh 17, step 11, 3535 restart picks, 0 inside, E2 0 (the record fails at its S1 assertion, as recorded); F1 10, 10, 10, bit-equal, NC1 `[37, 35, 34, 36, 36, 36, 39, 36, 34, 36]` | D2 |
| B2 | rules | `cargo fmt -p rcfd-api --check`; `cargo clippy -p rcfd-api --all-targets -- -D warnings`; `git diff fcb0e65e --stat` touches only `bayes.rs`, the new test, this ADR, `docs/design-loop.md` | clean; as stated | |

K1, K2, P0, K3, K4 and K5 run in two `#[ignore]`d heavy records (Hartmann: 20 runs of 200, about 5 to 8 minutes on ten threads;
Shekel: 40 runs of 150, about 3 minutes), each run once. D0, H1, HR, NCR, T1 and T2 are the fast tests on the same code paths
(the three hashes about 40 s each, T1 about 30 s plus the W1 recomputations). Every loop is capped by its budget and prints one
line per run.

Reading taken for K3: if the trigger never fires on a function, the clause for that function is met only vacuously (equal
runs); the decision is then partial, not done.

### Negative controls

P0 (the plain runs are the recorded ones), NCR (the rep hash moves, so H1 and HR could fail) and the W1 recomputation (a plain
pick outside the basin would count).

### Mutations (scratch copy, one at a time, after the registered runs)

| Id | Mutation | Expected to fail |
|---|---|---|
| M1 | window length 0 (fires whenever the plain pick is in the basin) | T1 (W1 a) |
| M2 | the plain-pick condition dropped (the window alone) | T1 (W1 b); a registered doubt: it is caught only if a fire happens on a step whose plain pick is outside |
| M3 | the window over the first `W` evaluated observations instead of the last | T1 (W1 a, or no restart pick: the initial Latin hypercube is spread) |
| M4 | `Trigger::Repetition` read as `Trigger::Acquisition` | NCR (the hash equals HR), T1 (W1) |
| M5 | `Restart::default()` with the repetition trigger | HR, D0 |
| M6 | the window's basin measured in the unit cube (no length scales) | T1 (W1 a: a radius of 2 in the unit cube contains every point) |
| R4 | `restart` defaulting to `Some(Restart::default())` | H1 |

### Decision rule

* **done**: K1, K2, P0, K3, K4, D0, H1, HR, NCR, T1, T2, B1, B1h and B2 met; M1 and M3 to M6 and R4 caught (M2 is a
  registered doubt).
* **partial**: K1 or K2 missed (an ignored `*_known_miss` record with the limit intact), or K3 missed (an acceptance clause met
  only by equal runs), or a mutation other than M2 survives, with everything in the todo line met.
* **todo**: H1, HR, P0, B1 or B1h missed (the default bits moved or an F-715 / F-733 result changed).

## Limits (not covered)

* Two functions, one initial-design size and budget each, the seeds of the acceptance. `W = d + 1` and `rho = 2` are fixed by
  the derivation in D1 and by ADR 0651; neither is tuned, and no tuning set is used.
* The trigger is conservative by construction (at most one restart pick in `W + 1` during a stalled stretch); it is not
  expected to rescue as many Hartmann 6 seeds as the acquisition trigger (9 of 10). The acceptance asks for "at least as many as
  plain", and that is what is checked.
* With the repetition trigger `propose` depends on the order of the observations (D1); the ledger driver `run` passes them in
  ledger order. Not checked on the fan (the basin covers the box there, ADR 0673 Z5, so the restart cannot fire whatever the
  trigger).
* Library only: no study method, MCP tool, case key or golden.

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. Where "recent" comes from: the order of the observations passed to `propose` (evaluation order). The alternative, an
   explicit history of pick kinds passed by the drivers, would change the signatures of `propose`, `minimise` and `run`; not
   taken.
2. Whether the plain pick of the step must also lie in the basin: yes (the trigger is about what plain expected improvement
   would do now, not only what it did).
3. Whether to keep `tau` as a second condition: no; the acceptance asks for the trigger "instead of on the size of the
   acquisition".
4. Default off (opt-in), as the brief requires.

## Addendum 1 (2026-11-17): results

Code: 5183606e (`Restart::trigger` and `Trigger` in `bayes.rs`, the test file), then this addendum and `docs/design-loop.md`.
Each registered check ran once, in the release profile, with no amendment; logs in `f758-scratch/logs` (`run_fast.log`,
`run_k1.log`, `run_k2.log`, `run_b1_*.log`, `run_b1h_*.log`, `clippy.log`, `mutations.log`, `mut_*.log`).

### Results

| Check | Limit | Measured | Met |
|---|---|---|---|
| K1 Hartmann 6, rep against plain (seeds 1 to 10) | rep >= plain | **rep 8, plain 6**; first hits rep `[52, -, 57, 42, 52, 174, 52, 53, 174, -]`; rescued seeds 6 and 9, none lost; 279 s | yes |
| K2 Shekel 10, rep against plain (seeds 1 to 20) | rep >= plain | **rep 19, plain 18**; rescued seeds 5 and 18, lost seed 19 (best -2.79029, 20 restart picks); 156 s | yes |
| P0 plain runs reproduce the records | Hartmann `[48, -, 55, 41, 52, -, 54, 50, -, -]`; Shekel misses 5, 18 | the same; the same | yes |
| K3 the trigger fires | >= 1 per function | 129 restart picks on Hartmann (6 to 19 per run), 259 on Shekel (5 to 20 per run) | yes |
| K4 W1; A6; E2 over the rep runs | 0; 0; 0 | 0; 0; 0 (388 restart picks, each with its plain pick recomputed) | yes |
| K5 record | recorded | Ei steps where the condition held and no restart was made: 0 to 1 per Hartmann run, 0 to 10 per Shekel run (fewer than `d + 2` points outside the basin, or no fresh fit) | recorded |
| D0 default trigger | Acquisition | Acquisition | yes |
| H1 O1 points hash | == 0xe2cf141320774cfc | 0xe2cf141320774cfc | yes |
| HR hash with `Restart::default()` | == 0xfcb394c67ca8392f | 0xfcb394c67ca8392f | yes |
| NCR hash with the rep restart | differs from both | 0xd1ec15694c878503 | yes |
| T1 Shekel fast path, seed 1 | >= 1; 0; 0; 0 | 13 restart picks; W1 0; A6 0; E2 0 (35 s) | yes |
| T2 workers 1 and 8 | bit-equal | true | yes |
| B1 regression | pass | f715_robust_ei 6 passed (2 ignored), f733_restart_second_function 5 (3 ignored), f698_surrogate 19 (3 ignored), f716_stall_precheck 7 (2 ignored), `--lib` 141 (4 ignored), baseline_hashes 3 (1 ignored), docs_site 2 | yes |
| B1h F-715 / F-733 heavy records rerun by name | equal to their addenda | A1 9 of 10, first hits `[48, 167, 55, 41, 52, 147, 54, 50, 166, -]`, restart picks `[100, 45, 98, 103, 91, 44, 92, 102, 36, 32]`; A2b the P0 list; S0 -10.53644315, S1 plain 18 fresh 17, S2 step 11, S3 3535 picks, 0 inside, E2 0 (fails at its S1 assertion, as recorded); F1 10, 10, 10, bit-equal, NC1 `[37, 35, 34, 36, 36, 36, 39, 36, 34, 36]`, located, E2 0 | yes |
| B2 fmt, clippy, files | clean; as stated | `cargo fmt -p rcfd-api --check` clean; `cargo clippy -p rcfd-api --all-targets --release -- -D warnings` clean; `git diff fcb0e65e --stat`: `bayes.rs`, the new test, this ADR, `docs/design-loop.md` | yes |

Reading of K1 and K2: the repetition trigger helps on both functions. The acquisition trigger helped on Hartmann 6 (9 of 10)
and hurt on Shekel 10 (17 of 20). On Shekel 10 the repetition trigger rescues the two seeds that plain expected improvement
misses and keeps seeds 9 and 13, which the acquisition trigger lost. It loses seed 19, where the incumbent's basin is the
local minimum -2.79 and the run made 20 restart picks. From the record alone (no diagnostic run): the restart picks changed the
later data, and plain expected improvement never drifted into the global basin, as it did at evaluation 87 without them. On
Hartmann 6 it rescues two of the four stalled seeds, both at evaluation 174, with 14 and 12 restart picks; seeds 2 and 10 stay
at -3.20316. As registered (D1, Limits), it rescues fewer Hartmann seeds than the acquisition trigger, whose restart takes most
of the budget.

### Mutations

| Id | Mutation | Caught by |
|---|---|---|
| M1 | window length 0 | T1 (W1: 133 of 134 restart picks) |
| M2 | the plain-pick condition dropped | **survived** T1 (13 picks, W1 0: on seed 1 every fire was on a step whose plain pick was in the basin anyway). The NCR hash moved (0x2a759462027cd843), but NCR only tests against H1 and HR. A registered doubt |
| M3 | the window over the first observations | T1 (0 restart picks) |
| M4 | `Repetition` read as `Acquisition` | NCR (equals HR), T1 (W1: 80 of 82) |
| M5 | `Restart::default()` with the repetition trigger | HR (0xd1ec15694c878503); D0 printed `Repetition`, its assertion comes after HR in the same test |
| M6 | the window's basin in the unit cube | T1 (W1: 137 of 139) |
| R4 | `restart` defaulting to `Some(Restart::default())` | H1 (0xfcb394c67ca8392f) |

Control: the unmutated copy passes every non-ignored test of the file (`mut_CONTROL.log`).

### Deviations

None to the registered checks. The records and the B1 runs shared the machine with other agents' builds; the times are
wall-clock under that load. docs_site ran once before and once after the docs edits of this branch.

### Decision

By the registered rule: **done**. Every check K1 to B2 was met in its one run with no amendment; M1, M3 to M6 and R4 were
caught; M2 survived, as registered. On Hartmann 6 the restart with the repetition trigger reaches the target on 8 of 10 seeds
against 6 of 10 for plain expected improvement, and on Shekel 10 on 19 of 20 against 18 of 20. Every F-715 and F-733 check was
re-run and kept its result. The O1 points hash and the hash of the ADR 0651 restart are unchanged.

### Remaining questions (taken on the maintainer's standing instruction of 2026-11-11; the recommendation is given in each)

1. M2 survived: no fast check sees the plain-pick condition. Options: (a) leave it (W1 b checks it on every restart pick of
   the records, 388 picks, 0 violations, but it sees only a wrong fire, never a missing condition); (b) a fast test that
   builds an observation sequence whose window lies in the basin while the plain pick lies outside it, and checks that no
   restart is made. Recommend (b) as a small follow-up.
2. Default trigger. Options: (a) keep `Acquisition` as the default of `Restart` (the hashes stay); (b) make `Repetition` the
   default of `Restart` (changes HR and every recorded restart result of ADR 0651 / 0673). Recommend (a). The restart itself
   stays off by default either way. A change of default would change recorded results, so it is not taken here.
3. Order dependence: with the repetition trigger, the ledger driver `run` reads the observations in ledger order, so a
   reordered ledger can give other picks. Options: (a) document it (done here and in `docs/design-loop.md`); (b) store the
   pick order in the ledger entries and sort by it. Recommend (a) until the trigger is used through `run`.
4. Shekel 10 seed 19 is lost by the rep restart. It was not diagnosed by a run. Recommend no action: the clause is met, and a
   diagnosis of one seed would invite tuning `W`.

## For the lead: roadmap entries

> Proposed status: done

> Follow-up of F-733 (ADR 0673, 2026-11-17). ADR 0680, 2026-11-17. DONE (library, opt-in). The basin restart of ADR 0651 gains Restart::trigger (default Trigger::Acquisition, so every F-715 and F-733 result and the default bits are unchanged) with a second trigger, Trigger::Repetition: the restart fires when the plain pick of the step and each of the last d + 1 evaluated observations lie in the incumbent's basin (radius 2 in the fitted length scales), instead of when the acquisition is small. The window d + 1 (a simplex of consecutive evaluations) and the radius were fixed before any run and not tuned. It takes at most one pick in d + 2 while plain expected improvement keeps choosing the basin, and stays quiet when plain expected improvement drifts out of it. Hartmann 6 (n0 30, budget 200, seeds 1 to 10): 8 of 10 within 1 percent against 6 of 10 for plain expected improvement (seeds 6 and 9 rescued, none lost). Shekel 10 (n0 10, budget 150, seeds 1 to 20): 19 of 20 against 18 of 20 (seeds 5 and 18 rescued, seed 19 lost), where the acquisition trigger did 17 of 20. Every restart pick was checked from outside: its window and its recomputed plain pick in the basin, the pick itself outside it (388 picks, 0 violations). The F-715 and F-733 heavy records and fast tests were re-run with their recorded results; the O1 points hash of ADR 0673 and the hash of the ADR 0651 restart are unchanged. Six of seven mutations caught; dropping the plain-pick condition survived, as registered. With the repetition trigger, propose depends on the order of the observations. Not covered: the fan (the basin covers the box there), other functions, batches larger than one.

> Follow-up proposed: A fast check of the plain-pick condition of the repetition trigger. Deps F-758. Acceptance: a fast test with an observation sequence whose last d + 1 points lie in the incumbent's basin while the plain pick of the step lies outside it shows that the repetition trigger makes no restart pick, the surviving ADR 0680 mutation M2 (the plain-pick condition dropped) is caught by it, and the O1 points hash of ADR 0673 is unchanged.
