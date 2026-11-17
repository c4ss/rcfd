# ADR 0725: F-844 registration: the local mode update of the incompatible Hex8 as a default, measured (the warm start of ADR 0660 D1, the backtracking of ADR 0655 and the trust region of ADR 0660 D2, each alone as the would-be default, on the eleven set-ups of ADR 0658 and on ADR 0723's fine trace from rung 4)

Status: **REGISTERED 2026-11-17, written before any code of F-844 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no test code of F-844 was written and nothing of this item was run (no probe). The numbers quoted
below come from the ADR texts and from the published logs of ADR 0718 (`w49i-work/f796-scratch/run3-heavy.log`) and ADR 0723
(`w49i-work/f841-scratch/run2-heavy.log`); the arithmetic was done by hand (disclosed). Results go in the addenda at the end;
nothing above the addenda is changed after a run.

Roadmap item F-844 (deps F-841), the follow-up proposed by
[ADR 0723](0723-f841-registration-the-subcritical-branch-of-rule-l-s-16-3-punch-path-past-amplitude-1-65e-4-with-warm-started-local-modes.md)
(its remaining question 1, option (a)). Number 0725 given by the lead. Branch `f844-work` from cea9dd0e (F-841 merged),
worktree `w49i-work/f844`, scratch `w49i-work/f844-scratch`. The element is the incompatible-modes Hex8 at finite strain of
ADR 0608 with rule L (`IncompatibleStabilisation::LinearStress`, ADR 0630 D1), the neo-Hookean material of ADR 0636 (`mu =
1e6`, `nu = 0.49`).

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test
edited; no new dependency, no unsafe code; clean-room. Every choice below that the maintainer would normally be asked about is
taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), except the open questions at the end.

**This is a measurement, not a default change.** No library line is changed; each option is run through its existing opt-in
(`SolidModel::set_incompatible_mode_warm_start`, `set_incompatible_mode_backtracking`, `set_incompatible_mode_trust_region`).
The recommendation registered below is the agent's; **whether any option becomes the default remains the maintainer's
decision** (it would change default results, and for the warm start the length of the committed element states).

## What F-844 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. The warm start of ADR 0660 D1, the backtracking of ADR 0655 and the trust region of ADR 0660 D2, each measured as the would-be default on the ADR 0658 set-ups and on ADR 0723's fine trace from rung 4 without committed states beyond the driver's own: solves passed, iterations, and the states where the plain iteration from alpha = 0 fails at a regular root; a recommendation registered before the run; the default bits unchanged.

Clauses: **c0** every limit registered here; **c1** three options, each alone as the would-be default: WS (the warm start of
ADR 0660 D1), BT (the backtracking of ADR 0655 D1), TR (the trust region of ADR 0660 D2); **c2** on the ADR 0658 set-ups;
**c3** on ADR 0723's fine trace from rung 4, without committed states beyond the driver's own; **c4** solves passed; **c5**
iterations; **c6** the states where the plain iteration from `alpha = 0` fails at a regular root; **c7** a recommendation
registered before the run; **c8** the default bits unchanged. The brief adds: test-only (another agent edits `arclength.rs`);
a new test file `crates/rcfd-fem/tests/si_eas_local_update_f844.rs`; heavy runs of about 170 s capped, `#[ignore]`d and run
once, fast tests always; negative controls and mutations registered before the run; reruns by name of the ADR 0655, 0658 and
0660 test files (`si_eas_hg6`, `si_eas_hg7`, `si_eas_hg8`), `si_eas_hg20` and `--lib`.

### Readings, registered

- **c1: "each alone as the would-be default".** Four variants, each rule L with `NewtonOptions::new(10).threads(4)` (the
  driver's defaults otherwise: no line search) and exactly one of the three opt-ins on: **P** (none: today's default, the
  control), **WS** (`set_incompatible_mode_warm_start(true)`, the plain update), **BT** (`set_incompatible_mode_backtracking
  (true)`, domain-only: no decrease test), **TR** (`set_incompatible_mode_trust_region(true)`, from `alpha = 0`). TR alone is
  exactly ADR 0660's variant T; BT alone is ADR 0658's NCB variant; WS alone (without the backtracking and the decrease test of
  ADR 0660's W) was not run before on these set-ups. Combinations (ADR 0660's W) are not in scope (cited, not run).
- **c2: the ADR 0658 set-ups**, eleven, as ADR 0658 readings c2, c3 (and ADR 0660 reading c1 for the first eleven), by test
  file and name: the eight distorted meshes of ADR 0649 D2 built as in `si_eas_hg8.rs` `w_and_t_on_the_four_blocks` (block
  `4^3` seed `0xF719_0004` at `a = 0.1` and `0.25`, IDs **DB1**, **DB2**; block `8^3` seed `0xF719_0008`, **DB3**, **DB4**) and
  `w_and_t_on_the_four_cantilevers` (cantilever `10 x 2 x 2` seed `0xF719_1010`, **DC1**, **DC2**; `20 x 4 x 4` seed
  `0xF719_1020`, **DC3**, **DC4**), and the three seeds of `w_and_t_on_the_seeds_of_adr_0658` (`8^3` at `0.25 h`, seeds
  `0xF739_0001` to `_0003`, **DS1** to **DS3**; all three valid in ADR 0658, the replacement rule of ADR 0658 reading c3 with
  ADR 0660 A1.1 kept). Load cases and measures as there (block: punch `u_z = -0.1`, `R = -sum_punch r_z / mu`, `w_c`;
  cantilever: tip `u_y = 0.3`, `P = sum_tip r_y / mu`, `d = -mean_tip u_x`); 10 increments, 4 threads. The helpers of
  `si_eas_hg8.rs` are copied (existing tests are not edited).
- **c3: the trace, and "without committed states beyond the driver's own".** ADR 0723 D1 and D2 (`si_eas_hg20.rs`,
  `f841_nh16_warm_trace_heavy`): ADR 0711's rungs rebuilt on the plain model, then from rung 4 `S4` the fine trace with `Da =
  (a_5 - a_4) / 8`, 16 steps, 6 halvings in a row, at most 40 solves, stops "fold + 2", "steps", "lost", the same predictor,
  bordered Newton and continuity test, on the model of the option. **Reading of the clause**: the element states carried
  along the trace are exactly those the Newton driver itself would commit at an accepted state, its trial states, and nothing
  else. For BT and TR (stateless updates of a material with `state_size 0`) these are the initial, empty states: no commit at
  all. For WS they are the trial states returned by the library's warm evaluation of each element at the accepted state
  (`condensed_force_and_tangent_warm`, the per-element evaluation of the driver's ADR 0660 D1 path), used directly; ADR 0723
  instead built them test-side with `mode_iteration_record_from` and checked one element bitwise (CW). The test-side commit
  is not used here. The trace also omits ADR 0723's lowest pairs, the expensive part of a state, because the clause asks for
  solves, iterations and states, not pairs. The amplitude, `lambda`, `dlambda/da` and the inertia `nu` of the one factor of
  `analyse` are kept.
- **c4: "solve passed".** On a set-up: the driver returns the converged result at load factor 1 **and** the two measures are
  within the ADR 0636 tolerances of the F-bar Hex8 (`0.15` on DB1, DB2, DC1, DC2; `0.10` on DB3, DB4, DC3, DC4, DS1 to DS3,
  for each of `e = |x / x_Fbar - 1|`) **and** the loaded state is positive definite (SL of ADR 0649: no negative eigenvalue of
  `K_ff`, lowest over largest diagonal `> 0`). The F-bar values are those recorded in ADR 0660 arithmetic 1 (8 digits; the
  F-bar code is unchanged since, so they are not re-solved here; rounding `<= 7.8e-8` relative). On the trace: an attempt is
  passed when ADR 0711 D3's bordered Newton converges and the continuity test holds (an accepted state). The trace of an option
  is summarised by its stop, its reading (ADR 0718 D4 with ADR 0723 D4, `l_min` there that of element 3720's `K_aa` at the
  option's root), the last accepted amplitude and whether it brackets ADR 0723's fold (a sign change of `dlambda/da` between
  `a = 2.165e-4` and `2.205e-4`, within `1e-3` relative of those ends).
- **c5: the iterations reported.** Per set-up solve: the increments, the halved increments, the halvings in all, and the global
  Newton iterations (the sum over the increments of the residual count minus one). Per trace: the solves, the accepted states,
  the failed attempts, and the bordered iterations of each accepted state. Per tallied element (c6): the local iterates of the
  option's record.
- **c6: "the states where the plain iteration from alpha = 0 fails at a regular root".** At every accepted state of every
  trace, for every element, the plain local iteration from `alpha = 0` (`mode_iteration_record_from`, update `Plain`, start
  `None`; the model's default) at the state's nodal displacement. For each element where it fails, the **option's root** is
  taken: the committed modes for WS, and for BT and TR the option's own record from `alpha = 0`, which converges there because
  the state's residual was evaluated with it. Three things are then measured: `l_min`, the smallest eigenvalue of the
  symmetric part of `K_aa` at that root (**regular** if `> 0`); the option's local iterates; and whether the root is the
  **continuation root**. The continuation root is the plain iteration started from the element's root at the previous accepted
  state of the same trace, classified by ADR 0723 D3's `root(d, 0)`: "same root" if `d <= 1e-6`. A state **counts for c6**
  when at least one element there has a plain failure at a regular root. On WS's trace the tally also runs BT and TR from
  `alpha = 0` at each such element and classifies their roots against the warm root. This compares the three options at the
  same states, independent of where each option's own trace goes.
- **c7: the recommendation rule** (D4), registered here before any run.
- **c8: default bits.** No library file is changed (`git diff --stat cea9dd0e -- crates/*/src` empty at the tip, check B);
  `baseline_hashes` is not run (no shared solver file touched); NCW0 below records that the default commits no modes.

## What was read

- `AGENT_RULES.md`; the brief; ADR 0723 (whole, and `run2-heavy.log`); ADR 0660 (whole); ADR 0658 (readings, arithmetic,
  Addendum 1); ADR 0655 (D1 through ADR 0658); ADR 0718 D4 and `run3-heavy.log` (the plain fine trace); ADR 0572 and 0573 (the
  roadmap section, through ADR 0723).
- `crates/rcfd-fem/tests/si_eas_hg20.rs` (whole), `si_eas_hg8.rs` (whole); `crates/rcfd-fem/src/model.rs` (the four opt-in
  setters, `incompatible_mode_update`: `TrustRegion` when on, otherwise `Plain` / `Backtracking` / with decrease),
  `nl_incompatible.rs` (`IncompatibleModeUpdate`, `mode_iteration_record_from`, `condensed_force_and_tangent_warm`,
  `WarmEvaluation`), `nonlinear.rs` (`LoadStep`, `NonlinearResult`, `internal_force`, `tangent`).

## What is known before the run, and arithmetic (disclosed)

1. **P on the eleven set-ups** (ADR 0649, ADR 0658 Addendum 1): converges on the seven meshes other than DB4, 10 increments
   each, no halving, within the tolerances (largest `0.1278 / 0.15`, DB2), SL positive definite; **fails DB4** (ADR 0658 NC1:
   element 476 at `0.675`) and **all three seeds** (DS1 element 475 inverted at `0.809`; DS2 element 420 inverted at `0.9375`;
   DS3 element 476, modes not converged, at `0.856`). P passes 7 of 11.
2. **BT** (ADR 0655 D1, ADR 0658 NCB): on DB4 it **fails at element 475, `0.890625`**. Where P converges with no halving no
   local iterate of P inverted (an inverted iterate is an error of the evaluation, which halves the increment), and BT acts only
   on an inverted iterate, so on the seven meshes BT performs P's operations: **bitwise P** (ADR 0655 NC2 measured that on two
   set-ups). On the seeds BT alone was not reported. Expected (an estimate): BT passes 7 of 11 or fewer (it fails DB4; the
   seeds, where ADR 0658's line search with the backtracking still failed DS2 and DS3, unknown).
3. **TR** is ADR 0660's T on the same eleven set-ups, same code: it **passes the eight meshes and DS1 and fails DS2 (element
   420, 25 local iterations, load factor `0.9406`) and DS3 (element 476, the step rejected 21 times, `0.8469`)**, its values
   equal to W's within `5e-14`. Expected: 9 of 11, a reproduction.
4. **WS** alone: not run on these set-ups. The warm start changes only where the local iteration starts (ADR 0660 D1); where P
   converges, WS converges to the same root and its values agree with P's to the local tolerance, not bitwise (ADR 0660
   arithmetic 3 and 8). Where P fails, ADR 0660 E3 and LW1 showed the committed roots before the failing increments regular and
   the warm plain iteration converging from them in 4 steps without a halving. But WS has no protection against an inverted
   iterate when the warm iteration itself fails: its fallback is the plain iteration from 0, which is what fails in P.
   Expected (an estimate, not a limit): WS passes DB4 and some of the seeds.
5. **The trace with WS** is ADR 0723's warm trace with its commit replaced by the driver's own trial states. Both commits use
   the same `converged` iteration from the same start, so they give the same modes bit for bit. CW checked this for element
   3720; the start rule is that of ADR 0660 D1 in both. The trace is therefore expected to reproduce ADR 0723's table: the
   eleven accepted `lambda` values printed with 12 decimals (rounding `5e-13`), 12 solves (one rejected as discontinuous), the
   stop "fold + 2", the fold bracket `[2.165e-4, 2.205e-4]`. ADR 0723's table shows **the plain iteration from 0 failing at
   element 3720 at four of its twelve states** (steps 2, 4, the halved step 9 at `2.2046e-4`, and step 10), with `l_min` of
   element 3720's `K_aa` at the warm root `2.59e4`, `2.94e4`, `3.86e4`, `4.21e4` (regular). Other elements were not
   tallied.
6. **The plain trace** from `S4` (no option): ADR 0718's `run3-heavy.log`: `t_1 = 1.610141484e-4` accepted (`lambda
   0.959749331159`), `1.649773378e-4` accepted after one halving, then 7 local failures in a row (element 3720, an inverted
   point), lost; read "local breakdown: the local iteration"; 10 solves. Without the lowest pairs the same arithmetic gives the
   same states (the pairs do not enter the trace).
7. **The trace with BT.** At `S4` the plain iteration converges at every element (S4 was accepted by the plain trace), so BT's
   residual there is P's bit for bit. Along the trace BT changes only the elements whose plain iterate inverts, which is the
   failure mode of ADR 0718's plain trace (an inverted point at element 3720). Whether the halved steps reach the root (ADR
   0658 E1/E2: the domain-only backtracking can wander) is what is measured.
8. **The trace with TR.** ADR 0660 TR2: from `alpha = 0` TR stalled at a regular state of seed 3 (`|r|_2 ~ 99`). Whether it
   reaches the roots on the `16^3` trace, starting with `S4` itself, is not known. If TR fails at `S4` in some element (the
   trace cannot start), that is its measured outcome: "fails at rung 4".
9. **Cost** (from ADR 0723's log and ADR 0660's): the rebuild to `S4` about 45 s (observed solve 21 s, the chain 5 s, the mode
   1 s, the rungs 4 x 3 s, without ADR 0723's pairs at `S5`); an accepted warm state about 5.5 s without the pairs (9.1 s
   minus 3.4 s), a failed attempt 0.2 to 1 s; the tally about 1 s a state (4096 short records). WS heavy record: 45 + 12 x 7 =
   about 130 s; BT with the plain control: 45 + 15 + up to 12 x 7, about 145 s; TR: unknown (the trust-region step costs a QR
   per local step), registered with a time cap (D2). The set-ups: ADR 0660's tests ran 35 s (blocks), 25 s (cantilevers), 77 s
   (seeds, 12 solves of `8^3`) with 3 to 4 solves per mesh. Here there are 4 solves per mesh plus SL on the passes: blocks about
   40 s, cantilevers about 25 s, seeds about 90 s. A default test over about two minutes is made an ignored heavy record after
   its single run (limits and code unchanged), as ADR 0660 registered; the fast local tally keeps the trace's code path fast.

## Design

### D1. The set-ups (`f844_setups_blocks`, `f844_setups_cantilevers`, `f844_setups_seeds`; default tests)

Per mesh: M0 (smallest corner `det J` over the centre's `> 0`), M1 (moved nodes `27`, `343`, `9`, `171`, `343`); P, WS, BT, TR
solved; per run the outcome (the error text, the element it names, the load factor), the iterations of c5, the two errors
against the recorded F-bar values, SL for a converged run, and **passed** (c4). Printed: a table row per run, and per variant
the passes, the passed set-ups and the global iterations summed over the set-ups all four variants pass.

### D2. The traces (three heavy records, ignored, each run once)

Shared: the rebuild of ADR 0723 D1 on the plain model (the observed `16^3` solve, the symmetric chain, the mode at 0.9647, the
switch and the step to 0.9645, `amp_trace` with 5 rungs; checks OB, R1, S5 with ADR 0718's limits, without the pairs at `S5`).
Then on the model of the option the fine trace of reading c3 from `S4` (16 steps, 6 halvings, 40 solves) with a **time cap of
300 s per trace** (registered now: a trace whose elapsed time passes 300 s at the start of an attempt stops, read "time cap",
measured as such). At each accepted state the tally of reading c6 (four threads).

- `f844_trace_warm_heavy` (WS): `S4` committed with the driver's trial states (every element from `alpha = 0`, the plain
  modes), analysed on the warm model (W0 as ADR 0723), then the WS trace with that commit at each accepted state; the tally
  with BT and TR beside.
- `f844_trace_backtracking_heavy` (BT): the plain trace from `S4` first (the control PLN, reading 6), then the BT trace, no
  commit.
- `f844_trace_trust_region_heavy` (TR): the TR trace, no commit.

### D3. The fast local tally (`f844_local_tally_at_the_stall_of_seed_3`, default)

The trace's tally code on a state where the cold iterations are known to fail. The set-up is ADR 0660's: seed `0xF739_0003`,
block `8^3` at `0.25 h`. ADR 0658's remedy R (line search and backtracking) is solved to the punch `0.0850` and `0.0855`
(`u_a`, `u_b`). The trial state is the whole field `u_T = u_b + (0.0858 - 0.0855) / (0.0855 - 0.0850) (u_b - u_a)`. The
previous roots are those of the cold `Backtracking` record at `u_b` of every element (ADR 0658 E3 for element 476). The tally
of reading c6 at `u_T` runs over all 512 elements with BT's and TR's records from `alpha = 0` and the continuation from the
previous roots. Element 476 is read directly as well.

### D4. The recommendation rule (c7), registered before any run

For each option O of WS, BT, TR, the run yields: `N(O)`, the set-ups passed of eleven; **regress(O)**, true if O fails a
set-up that P passes; the trace's outcome; and `C(O)`, the number of c6 states of the WS trace at which O's iteration from its
own start reaches the continuation root at every element where the plain iteration fails. For WS this holds by
construction wherever it did not fall back.

1. An option with regress(O) is not recommended as the default.
2. An option **qualifies** if it does not regress, passes more set-ups than P (`N(O) > 7`), and its trace brackets ADR 0723's
   fold (reading c4).
3. If exactly one option qualifies, it is the recommendation. If several qualify, the one with the larger `N(O)`. On a tie,
   the stateless option (BT or TR) over WS, because the warm start lengthens every committed state of an Incompatible Hex8 by
   nine values. If still tied, the one with the fewer global iterations summed over the set-ups all of them pass.
4. If none qualifies, no single option is recommended as the default. The recommendation is then to keep the default and to
   propose a follow-up that measures ADR 0660's combination W (warm start with the backtracking and the decrease test) as the
   would-be default on the same set-ups and trace. ADR 0660 measured W on the eleven set-ups (all passed) but not on the trace.
   The option with the largest `N(O)` is named as the best single option.

Expected (an estimate from arithmetic 1 to 8, not a limit): TR 9 of 11 with an uncertain trace; BT at most 7 of 11; WS
unknown. If WS passes more than 7 of 11 without regression and its trace reproduces ADR 0723's fold, rule 3 recommends WS.

## Checks

Every measure is required finite before it is compared (a non-finite value or a missing state misses).

### The set-ups (default tests)

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| M0, M1 | c2 | as ADR 0658 on the eleven meshes | `> 0`; exactly | ADR 0658 |
| RP0 | c2, c4 | P passed per set-up | passed on DB1 to DB3, DC1 to DC4; failed on DB4, DS1 to DS3 | arithmetic 1 |
| RP1 | c2 | the element named by P's error on DB4, DS1, DS2, DS3 | `476`, `475`, `420`, `476` | arithmetic 1 |
| RB0 | c2 | BT on DB4: failed; the element named | failed; `475` | arithmetic 2 |
| RB1 | c2, c8 | BT against P on the seven meshes P passes: displacement and reaction values differing in bits | `0` each | arithmetic 2 |
| RT0 | c2, c4 | TR passed per set-up | passed on the eight meshes and DS1; failed on DS2, DS3 naming `420`, `476` | arithmetic 3 |
| NCW | c1 | WS against P on DC2: values differing in bits | `> 0` | arithmetic 4 |
| NCT | c1 | TR against P on DC2: values differing in bits | `> 0` | ADR 0660 NCT (240) |
| NCW0 | c8 | P on DC2: elements whose committed state has a value | `0` | ADR 0660 NCW0 |
| WS | c4, c5 | WS on the eleven: passed, the errors, SL, the iterations | measured (no limit) | arithmetic 4 |
| IT | c5 | the iterations of every run (D1) | reported | |

### The fast local tally (default test)

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| LT1 | c6 | element 476 at `u_T`: BT's record from `alpha = 0` | fails | ADR 0658 E1 (the domain-only record stops at the cap) |
| LT2 | c6 | element 476 at `u_T`: TR's record from `alpha = 0` | fails | ADR 0660 TR1 (stall at `97.74`) |
| LT3 | c6 | element 476 at `u_T`: the continuation (plain from the cold `Backtracking` root at `u_b`) | converges within 25 iterates | ADR 0660 LW1 (4 iterates, no halving, with update S) |
| LT4 | c6 | if element 476 is in the tally's plain-failure list: its entry's BT, TR and continuation outcomes equal LT1 to LT3 | equal (else reported "not applicable") | the same records |
| LT5 | c6 | the tally at `u_T`: the plain failures, regular ones, the options' outcomes | reported | |

### The heavy records (ignored, each run once)

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| OB, R1, S5 | c3 | as ADR 0723 (the error text of ADR 0678; `a` at 0.9645 to `1e-8`, `nu = 1`; rungs 1 to 5, `lambda_5` to `1e-12`, `a_5` to `1e-9`, `sigma_5` to `1e-6`) | ADR 0718's limits | ADR 0723's run |
| W0 | c3 | WS: `S4` committed with the driver's trial states: the free residual against the plain model's at `S4`; `sigma`; `nu` | `0` entries differ; bit-identical; equal | ADR 0723 W0 |
| RW1 | c3 | WS: `lambda` at the accepted states against ADR 0723's table (11 values, the halved step 9 included) | `<= 1e-11` each, and 11 states | arithmetic 5 (`5e-13`) |
| RW2 | c3 | WS: the stop; the solves; the bracket | "fold + 2"; `12`; brackets the fold | arithmetic 5 |
| RW3 | c6 | WS: the accepted states at which element 3720's plain iteration from 0 fails | exactly steps 2, 4, the halved 9 and 10 (`a` `1.689405272e-4`, `1.847932848e-4`, `2.204619895e-4`, `2.323515577e-4`, ADR 0723's log, each to `1e-8` relative) | ADR 0723 table |
| PLN | c3 | BT test: the plain trace from `S4`: `lambda(t_1)` against `0.959749331159`; the second accepted `a` against `1.649773378e-4`; the stop; the reading | `<= 1e-9`; `<= 1e-9` relative; "lost"; "local breakdown: the local iteration" | arithmetic 6 |
| B0 | c3 | BT: the free residual at `S4` against the plain model's | `0` entries differ | arithmetic 7 |
| T0 | c3 | TR: the free residual at `S4` evaluates; if so, entries differing from the plain model's | reported; `> 0` if it evaluates | arithmetic 8 |
| FW1 | c3 | each trace: at every accepted state the constraint (relative) and the continuity test | `<= 1e-9`; holds | ADR 0723 FW1 |
| TRC | c3, c4, c5 | each trace: stop, reading, accepted states, last `a`, solves, failures, bordered iterations, bracket of the fold | reported | |
| TAL | c6 | each trace: per accepted state the tally of reading c6 (and on WS's trace BT's and TR's outcomes beside); the c6 states | reported | arithmetic 5 |

### Default bits and static checks

| Id | Clause | Measure | Limit |
|---|---|---|---|
| B | c8 | `git diff --stat cea9dd0e -- crates/*/src` at the tip | empty |
| S6 | | `cargo fmt -p rcfd-fem --check`; `cargo clippy -p rcfd-fem --release --all-targets -- -D warnings` | clean |
| S7 | | release: `cargo test -p rcfd-fem --release --lib`; `--test si_eas_hg6`, `si_eas_hg7`, `si_eas_hg8`, `si_eas_hg20` (their default tests); the new file's default tests (the registered run) | pass |
| S8 | | `cargo test -p rcfd-cli --release --test docs_site` (docs/ touched) | pass |
| S9 | | every commit builds | builds |

### Negative and positive controls

| Id | Control | Expected |
|---|---|---|
| RP0, RP1 | the default on the set-ups | fails where ADR 0658 says, passes the seven |
| RB0, RB1 | BT acts only on an inverted iterate | bitwise P where P passes; DB4 at element 475 |
| RT0 | TR reproduces ADR 0660's T | 9 of 11 |
| NCW, NCT | the opt-ins reach the driver | bits differ from P |
| NCW0 | the default commits no modes | `0` |
| PLN | the plain trace from `S4` | ADR 0718's loss at `1.65e-4` |
| LT1 to LT3 | the cold options fail where the continuation succeeds | ADR 0658 E1, ADR 0660 TR1, LW1 |

### Planned mutations (on a scratch copy of the test commit, each run once against the default tests of the new file)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the variant builder ignores WS's flag | NCW (`0` bits) |
| MU2 | the variant builder ignores TR's flag | NCT (`0` bits), RT0 (TR then fails DB4 as P) |
| MU3 | the variant builder ignores BT's flag | RB0 (DB4 fails at element 476, not 475) |
| MU4 | "passed" requires only convergence (no tolerance, no SL) | possibly not caught (every converged run of ADR 0658 and 0660 was within the tolerances and positive definite): registered as possibly not caught |
| MU5 | the tally's continuation starts from `alpha = 0` instead of the previous root | LT3 (the continuation fails as the cold iterations) |
| MU6 | "passed" counts a failed solve as passed | RP0 (DB4, DS1 to DS3 read passed) |

The heavy records are not run under the mutations.

## Decision rule

- **done**: every checked row of the set-up tests, the fast tally and the three heavy records met in their single registered
  runs (a trace stopped by a registered cap counts as measured); every option measured on the eleven set-ups and on the trace;
  the recommendation stated by rule D4; B and S6 to S9 met; MU1, MU2, MU3, MU5, MU6 caught (MU4 recorded either way); no
  amendment.
- **partial**: B met, but a checked row missed, a registered run not made, a mutation registered as catchable not caught, or a
  row met only after an amendment.
- **todo**: B fails, or no option is measured.

## Limits (not covered)

- Each option alone. The combination W of ADR 0660 is cited from ADR 0660 and not run, either on the set-ups here or on the
  trace. The line search is not run.
- One material (hyperelastic, `state_size 0`), rule L only; the eleven set-ups and one load path of the `16^3` block. History
  materials, the transient and rotating paths are not covered (ADR 0660's limits stand).
- The trace's commit for WS is the library's per-element warm evaluation applied test-side between bordered Newton states; the
  library's arc-length driver (ADR 0711 D1, F-797) is not used.
- The tally classifies roots by ADR 0723 D3's registered threshold; "regular" is a positive `l_min` of the symmetric part of
  `K_aa` at the root, not a proof of uniqueness.
- The lowest pairs along the traces are not computed (ADR 0723 has them for the warm trace).
- No default is changed: the switch is the maintainer's decision.

## Open questions (taken or open)

1. **The design**: the variants (each opt-in alone, no line search), the reading of "the driver's own" committed states, the
   pass criterion, the tally and its continuation root, the time cap of 300 s, the recommendation rule D4, the file name: taken
   on the maintainer's standing instruction of 2026-11-11 (take the recommendation).
2. **Whether any option becomes the default**: not taken (it changes default results); open for the maintainer. This item
   only measures and recommends.

## Addendum 1: results (2026-11-17)

Logs in `w49i-work/f844-scratch`: `run1-default.log` (the four default tests, one after the other, on the test commit
e025242c; 51 s), `run2-heavy-ws.log` (79.5 s), `run3-heavy-bt.log` (93.7 s), `run4-heavy-tr.log` (198.4 s), each heavy record
run once on e025242c; `mutations.log` and `mut-MU*.log`; `s7-*.log`; `b-src-diff.log`; `s6-*.log`; `s8-docs.log`. Each
registered test was run once. No limit was changed after a run, and there was no amendment. The only code change after the
runs is the measured ignore text of the three heavy records (checked by S6).

### The set-ups (reading c4: converged, within the ADR 0636 tolerances of F-bar, positive definite)

| Set-up | P (default) | WS (warm start) | BT (backtracking) | TR (trust region) |
|---|---|---|---|---|
| DB1 block `4^3` `0.1 h` | passed | passed | passed | passed |
| DB2 block `4^3` `0.25 h` | passed | passed | passed | passed |
| DB3 block `8^3` `0.1 h` | passed | passed | passed | passed |
| DB4 block `8^3` `0.25 h` | fails: element 476 inverted, load factor `0.675` | **passed** (`0.0750`, `0.0287`; 10 increments, 45 iterations) | fails: element 475, modes not converged, `0.8906` | passed (`0.0750`, `0.0287`; 45) |
| DC1 to DC4 cantilevers | passed | passed | passed | passed |
| DS1 seed `0xF739_0001` | fails: element 475 inverted, `0.809` | **passed** (`0.0743`, `0.0495`; 46) | passed (`0.0743`, `0.0495`; 46) | passed (`0.0743`, `0.0495`; 46) |
| DS2 seed `0xF739_0002` | fails: element 420 inverted, `0.9375` | **passed** (`0.0821`, `0.0441`; 11 increments, 2 halved, 46) | fails: element 420, modes not converged, `0.9375` | fails: element 420, `0.9406` |
| DS3 seed `0xF739_0003` | fails: element 476, modes not converged, `0.856` | **passed** (`0.0787`, `0.0490`; 47) | fails: element 476, `0.856` | fails: element 476, step rejected 21 times, `0.8469` |
| **passed of 11** | **7** | **11** | **8** | **9** |

On the set-ups that every variant passed, the four variants needed the same global iterations: 125 on DB1 to DB3 and 176 on DC1
to DC4, with 10 increments each and no halving. They agreed in their measures to the printed digits (`e_R`, `e_w` of DB1
`0.1202`, `0.0406`; DC2 `e_P`, `e_d` `0.0694`, `0.0045`) and in SL (`6.63e-3` to `4.54e-6`). No variant regresses: each passes
every set-up that P passes. The set-up rows of the warm start alone equal those of ADR 0660's W (warm start, backtracking and
decrease test) in every printed digit: the extra protections of W are not needed on these set-ups once the start is warm.

### The fast local tally (`f844_local_tally_at_the_stall_of_seed_3`, 4.4 s)

At ADR 0660's trial state `u_T` of seed 3, the plain iteration from `alpha = 0` fails at one element of 512, element 476 (an
inverted point, `det F = -0.329`, after 2 iterates). There BT from 0 does not converge in 25 iterates, and TR from 0 is
rejected 21 times at `|r|_2 = 97.74`, as ADR 0660 recorded. The continuation from the element's root at `u_b` converges in 4
iterates. The tally's entry agrees (LT4). The option of this tally is BT, which has no root there, so the entry is not
counted as regular.

### The traces (reading c3: the driver's own states)

| Trace | Stop, reading | Accepted after `S4` | Last `a` | Solves, failures | Bordered iterations | Brackets the fold |
|---|---|---|---|---|---|---|
| P (plain control PLN) | lost, "local breakdown: the local iteration" | 2 | `1.649773e-4` | 10, 8 (all local: element 3720 inverted) | 3, 3 | no |
| **WS** | **fold + 2, "fold"** | **11** | `2.323516e-4` | **12, 1** (discontinuous, not local) | 3 at every state | **yes**: `[2.164988e-4, 2.204620e-4]` |
| BT | lost, "undecided" | 7 | `1.830594e-4` | 20, 13 (12 local: element 3720, modes not converged in 25; then the bordered Newton not converged in 20) | 3, 3, 3, 3, 3, 3, 2 | no |
| TR | lost, "undecided" | 10, all halved toward step 2 | `1.689173e-4` | 27, 17 (12 local: element 3720, the step rejected 21 times at `|r|_2` 7.65; then 5 bordered Newtons not converged in 20 iterations, 15 to 18 s each) | 3, 3, 3, 3, 2, 2, 2, 3, 5, 17 | no |

"Undecided" for BT and TR follows ADR 0718 D4, unchanged. Their last failure was a bordered Newton that did not converge,
which is not a local failure, so D4 does not read it as a local breakdown. Every failure before that one was local, in
element 3720.

**The states where the plain iteration from `alpha = 0` fails at a regular root (c6).** On every trace and at every accepted
state, the only element in which the plain iteration from 0 fails is **element 3720**. In the other 4095 elements it reaches
the option's root at every state of every trace.

- On WS's trace this happens at **4 of 12 states**, the four of ADR 0723: steps 2 and 4, the halved step 9 and step 10 (`a`
  `1.689405e-4`, `1.847933e-4`, `2.204620e-4`, `2.323516e-4`). At each the warm root is regular (`l_min` `2.589e4`,
  `2.939e4`, `3.860e4`, `4.210e4`), is the continuation root, and needed no fallback: none of the 4096 elements fell back in
  any of the 12 commits. At the same four states **BT from 0 reaches that root once** (step 2), and **TR from 0 never**.
- On BT's trace this happens at **1 state** (`a = 1.689405e-4`). There BT's iteration from 0 converges in 19 iterates to the
  continuation root (`l_min 2.589e4`). At step 4 (`1.8479e-4`) BT itself fails in element 3720. The plain iteration from 0
  fails there too, at a regular root, as WS's trace shows.
- On TR's trace this happens at **8 states**, all between `1.6696e-4` and `1.68917e-4`. TR converges there in 7 to 17 iterates
  to the continuation root (`l_min 2.55e4` to `2.59e4`). Its trace halves toward step 2's target `1.6894e-4` without reaching
  it.

### The checked rows

| Id | Limit | Measured | Met |
|---|---|---|---|
| M0, M1 | `> 0`; exactly | as ADR 0658 (block `0.6977`, ..., seeds `0.0680`, `0.0957`, `0.1788`); `27`, `343`, `9`, `171`, `343` | yes |
| RP0 | P passed on the seven, failed on DB4, DS1 to DS3 | as registered | yes |
| RP1 | `476`, `475`, `420`, `476` | `476`, `475`, `420`, `476` | yes |
| RB0 | BT fails DB4 at `475` | fails, `475` (`0.8906`) | yes |
| RB1 | `0` bits on the seven | `0` on each | yes |
| RT0 | TR passes the eight meshes and DS1; fails DS2 (`420`), DS3 (`476`) | as registered | yes |
| NCW | `> 0` | `252` | yes |
| NCT | `> 0` | `240` | yes |
| NCW0 | `0` | `0` | yes |
| LT1 | fails | fails (26 iterates, not converged) | yes |
| LT2 | fails | fails (16 iterates, rejected 21 times, `|r|_2 97.74`) | yes |
| LT3 | converges within 25 | 4 iterates | yes |
| LT4 | equal | equal (element 476 is in the list) | yes |
| OB, R1, S5 (three runs) | ADR 0718's limits | equal; `9.95e-11`, `nu 1`; rungs 1 to 5, `2.3e-13`, `2.2e-10`, `9.0e-8` (each run identical) | yes |
| W0 | `0`; bit-identical; equal | `0` failed commits, `0` entries; `-4.163800829103926e1` both; `1`, `1` | yes |
| RW1 | `<= 1e-11`, 11 states | 11 states, largest `4.2e-13` | yes |
| RW2 | "fold + 2"; `12`; brackets | "fold + 2"; `12`; `[2.164988e-4, 2.204620e-4]` | yes |
| RW3 | exactly 4 states, `<= 1e-8` | 4, `2.4e-10` | yes |
| PLN | `<= 1e-9`; `<= 1e-9`; "lost"; local breakdown | `6.8e-14`; `2.4e-10`; "lost"; "local breakdown: the local iteration" | yes |
| B0 | `0` | `0` | yes |
| T0 | `> 0` if it evaluates | evaluates, `12808` | yes |
| FW1 (four traces) | `<= 1e-9`; holds | `8.1e-14` (WS), `9.0e-14` (PLN, BT), `1.6e-13` (TR); holds | yes |
| B | empty | empty | yes |
| S6 | clean | `cargo fmt -p rcfd-fem --check`, `cargo clippy -p rcfd-fem --release --all-targets -- -D warnings` clean | yes |
| S7 | pass | release: `--lib` 15 passed; `si_eas_hg6` 4 passed (2 ignored), `si_eas_hg7` 4 (2), `si_eas_hg8` 5 (2), `si_eas_hg20` 1 (1); the new file 4 passed, 3 ignored | yes |
| S8 | pass | `cargo test -p rcfd-cli --release --test docs_site` passed | yes |
| S9 | builds | 006d8687 (docs), e025242c (built for the runs), the results commit | yes |

### The recommendation by rule D4

- regress: none (WS, BT and TR each pass the seven set-ups P passes).
- `N`: WS 11, TR 9, BT 8 (P 7).
- The trace brackets ADR 0723's fold: WS only.
- **One option qualifies: WS.** By rule 3 **the recommendation is the warm start of ADR 0660 D1 as the default local update
  of the incompatible Hex8 at finite strain** (rule L, the plain update, the fallback to `alpha = 0`). The tie-break was not
  needed. `C(BT) = 1` and `C(TR) = 0` of the four c6 states of WS's trace.

The default switch is the maintainer's decision and is not made here. It would change the default results of every
nonlinear solve with an Incompatible Hex8 in their last bits (NCW: 252 values on DC2, the measures equal to the printed
digits), it would change the default outcome where the default fails today (DB4, DS1 to DS3, the `16^3` trace), and it would
lengthen the committed state of every Incompatible Hex8 by its nine modes (ADR 0660 Q2).

### Mutations (each on a scratch copy of e025242c, run once against the four default tests)

| Id | Caught | By |
|---|---|---|
| MU1 (WS's flag ignored) | yes | NCW (`0` bits) |
| MU2 (TR's flag ignored) | yes | NCT (`0` bits), RT0 on DB4 and DS1 (TR then fails as P) |
| MU3 (BT's flag ignored) | yes | RB0 (DB4 fails at element 476, not 475) |
| MU4 ("passed" without the tolerance and SL) | **no** (registered as possibly not caught) | every check passes: every converged run here is within the tolerances and positive definite |
| MU5 (the tally's continuation from `alpha = 0`) | yes | LT4 (the entry's continuation fails while the direct one converges); LT3 was registered as the catcher but computes the continuation directly, so it does not see the tally's code (a deviation in the catching row, not in the outcome) |
| MU6 (a failed solve counted as passed) | yes | RP0 on DB4, DS1 to DS3; RT0 on DS2, DS3 |

Five of six caught: every mutation registered as catchable; MU4, registered as possibly not caught, is not.

### Deviations

- The TR record took 198.4 s, above the brief's 170 s. It stayed inside the registered time cap of 300 s, which did not
  act. Five bordered Newtons that did not converge cost 15 to 18 s each. This is reported as slow, not stopped.
- MU5 was caught by LT4, not by LT3 as registered (see the mutation table).
- The seeds test was estimated at about 90 s and took 24.7 s, and all four default tests together took 51 s. No default test
  became a heavy record.
- Reading c5 asks for the local iterates of the option at each tallied element. For BT and TR they are reported (19; 7 to
  17). For WS the tally reads the committed modes, so it reports 0 iterates: the warm iteration of the commit is not
  recorded. ADR 0723 X4 measured that iteration at 3 to 7 iterates.

### Not run

Everything registered was run. Not run by design (Limits): ADR 0660's combination W on the trace, the line search, history
materials, the transient and rotating paths, the lowest pairs along the traces. The whole rcfd-fem suite was not run (rule 7;
the daily gate). `baseline_hashes` was not run (no library line changed, B).

### Decision

**done**, by the registered rule: every checked row of the set-up tests, the fast tally and the three heavy records met in
their single registered runs; every option measured on the eleven set-ups and on the trace; the recommendation stated by rule
D4 (the warm start); B and S6 to S9 met; MU1, MU2, MU3, MU5, MU6 caught (MU4 recorded, not caught, as registered as possible);
no amendment. The default is unchanged; the switch is the maintainer's decision (remaining question 1).

### Remaining questions

1. **The default switch** (open, for the maintainer; not taken because it changes default results). Options: (a) make the
   warm start the default local update (the recommendation of rule D4). This changes the last bits of every Incompatible
   Hex8 nonlinear result and the length of the committed states, so it needs the maintainer's approval for any hash or golden
   it moves. (b) Keep it opt-in and document it as the recommended setting for rule L at finite strain. (c) Make ADR 0660's
   combination W the default after measuring W on the trace. Recommendation: (a), after the follow-up proposed below has
   measured what the switch would move; (b) until then.
2. **The fallback of the warm start.** A failed warm iteration falls back to the plain iteration from 0. On every set-up and
   trace here that fallback never acted, but it is exactly the iteration that fails at element 3720. Options: (a) a later
   item measuring a fallback with the backtracking (BT rescued 1 of 4 states; ADR 0660's W uses BT and the decrease test);
   (b) leave it. Recommendation: (b) until a case is found where the warm iteration fails.
3. **The trust region.** On the trace TR never got past the first state where the plain iteration fails and it fails there
   (step 2), and on the WS trace TR from 0 reached the root at none of the four states. That matches ADR 0660 TR2 (a stall
   at a regular state). Recommendation: record it as not recommended as a default (ADR 0660 question 1 (a)).

## For the lead: roadmap entries

> Proposed status: done.

> F-844 (ADR 0725, 2026-11-17): the three local mode updates of the incompatible Hex8 with rule L, each measured alone as the would-be default beside today's default, test-only, no default changed. On the eleven set-ups of ADR 0658 the default passes 7, the backtracking of ADR 0655 8, the trust region of ADR 0660 D2 9 and the warm start of ADR 0660 D1 all 11 (the 8^3 block at 0.25 h and the three seeds of ADR 0658 within 8.2 per cent of the F-bar punch reaction, positive definite). None fails a set-up the default passes, and where all four pass they take the same increments and Newton iterations. On ADR 0723's fine trace from rung 4, carrying only the element states the driver itself commits, the warm start reproduces ADR 0723's branch to the fold (11 states, lambda to 4e-13, the fold bracketed in 12 solves). The plain control is lost after 1.65e-4 as in ADR 0718, the backtracking after 1.83e-4, and the trust region never reaches 1.69e-4. Along the whole trace the plain iteration from alpha = 0 fails in one element only, element 3720, at 4 of the warm trace's 12 states, each at a regular root (l_min 2.6e4 to 4.2e4) that the warm start follows without a fallback. From alpha = 0 the backtracking reaches that root at 1 of the 4 states and the trust region at none. By the rule registered before the run, the warm start is the recommended default local update. The switch changes default bits and the length of the committed element states, so it is left to the maintainer. A fast tally at ADR 0660's stall state of seed 3 covers the trace's tally code. Default bits unchanged (no library line); five of six mutations of the measurement caught (the sixth registered as possibly not caught); no amendment.

> F-740 (ADR 0660, 2026-11-16), add at the end of its note: ADR 0725 measures the warm start alone (without the backtracking and the decrease test) as the would-be default: it passes all eleven set-ups of ADR 0658 with W's values and ADR 0723's trace to the fold, while the trust region alone passes 9 of 11 and is lost on the trace before 1.69e-4.

> F-841 (ADR 0723, 2026-11-17), add at the end of its note: ADR 0725 repeats the warm trace with the commit taken from the library's per-element warm evaluation (the driver's own trial states) and reproduces it (lambda to 4e-13); the plain iteration from alpha = 0 fails at element 3720 alone among the 4096 elements.

> Follow-up proposed: What switching the default local mode update of the incompatible Hex8 to the warm start would move. Deps F-844. Acceptance: Every limit not stated here is registered in the ADR before the run. With the model's warm start turned on in test-only copies of each case (no default changed), the existing nonlinear tests of the incompatible Hex8 and the rcfd-api baseline cases that use the element: the number of results whose bits change, the largest relative change of each recorded measure against a registered tolerance, every solve whose outcome changes, and the committed element states through the save and restart path; the default bits unchanged.
