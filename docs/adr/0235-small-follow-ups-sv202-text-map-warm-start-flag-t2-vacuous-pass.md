# ADR 0235: three small follow-ups: SV202's fuller text (F-328), the `rcfd map` flag for the extended warm start (F-329), the vacuous pass of `schemes_wave13` T2 (F-269)

Status: **REGISTERED 2026-11-12**, written before any code of this ADR and before any run of its checks. Not yet accepted by the
maintainer. Results go in the addendum at the end.

Parents: [ADR 0196](0196-three-code-changes-decided-2026-11-06-f246-r2-record-design-tool-class-sv202-text.md) (C, SV202's
solver-aware text; question Q1), [ADR 0209](0209-f249-registration-nsga2-studies-nelder-mead-options-and-extended-warm-start.md)
(D4, the extended warm start; question 7), [ADR 0016](0016-wave13-numerics.md) (F-211, check T2), [ADR 0129](0129-test-time-limit-jobs5-measurement-limit-audit.md)
and `docs/test-process.md` section 6.3 item 2 (the T2 finding), [ADR 0031](0031-wave19c-restart-chain.md) (D6, the rule grammar).
Roadmap items: F-328 (part A) and F-329 (part B), to be added by the lead when this branch is merged; F-269 (part C, its note).

Number: the lead gave 0235 to this bundle. Branch `f328-small-followups` from main bf3376b1, worktree `w49i-work/f328`.

House rules ([ADR 0130](0130-wave48a-overview-and-run-record-contract.md), as in ADR 0228):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the number recorded;
- a defective set-up gets a new corrected test by an amendment written and committed before the corrected run, and the old one
  stays as an ignored record;
- defaults stay bit-identical: no solver code, case-schema key or case hash changes; `crates/rcfd-api/tests/baseline_hashes.rs`
  and `case_hashes_w48u.rs` are not edited or re-recorded; no golden is edited or re-recorded (the MCP tools-list goldens are not
  touched by any part); an edit of an existing test is a registered edit named here with its before and after;
- each registered run is made once, and nothing is re-run to pick a result;
- no new third-party crate, `Cargo.lock` unchanged, no unsafe code, no commit trailers.

Files this branch does not touch (other branches work on them): `crates/rcfd-fv/src/{overset,compressible_bc,immersed}.rs`,
`crates/rcfd-mesh/src/overset.rs`, `crates/rcfd-api/src/sim.rs`, `crates/rcfd-api/src/schema.rs`, `scripts/gate_test.py`, and the
roadmap files (`docs/roadmap.json`, `docs/ROADMAP.md`, `docs/roadmap-overview.md`, `scripts/roadmap_progress.py`, the SVGs).

## What was read (disclosed)

At main bf3376b1: ADR 0196 in full; ADR 0209 in full; ADR 0016 section F-211; `docs/test-process.md` section 6.3; ADR 0228 and
0226 for the form; `crates/rcfd-api/rules/setup_rules.json` (SV202); `crates/rcfd-api/src/rules.rs` (module comment, the built-in
include); `crates/rcfd-api/tests/rules_w19c.rs` (E1, E4, E5, E4b); `crates/rcfd-api/src/sim.rs` lines 1376 to 1391 (the
"no boundary condition for mesh patch" refusal, skipped for `compressible` and for empty and cyclic patches; read only);
`crates/rcfd-api/src/schema.rs` lines 489 to 499 (a boundary entry's `velocity` defaults to `fixed`, `velocity_value` to zero;
read only); `crates/rcfd-fv/src/compressible.rs` lines 120 to 128 (every patch `CompBc::Slip`, a patch of kind outlet
`CompBc::Extrapolate`); `crates/rcfd-cli/src/cmd_map.rs`, `crates/rcfd-cli/src/main.rs` (usage text and option helpers),
`crates/rcfd-cli/tests/map_w32c.rs` and `cli.rs`; `crates/rcfd-api/src/design/case_eval.rs` (`CaseMapEvaluator`,
`ExtendedMapEvaluator`, `map_point`), `crates/rcfd-api/src/design/opmap.rs` (`MapOptions`, `run_and_write`, `row_json`, the
restart labels `cold`, `warm`, `cold_retry`), `crates/rcfd-api/src/study.rs` (`apply_value`); `crates/rcfd-fv/tests/schemes_wave13.rs`
(`wave`, `excursion`, W1, T1, T2) and `crates/rcfd-fv/src/ddt.rs` (`add_ddt`); the F-269 and F-240 entries of `docs/roadmap.json`.

**One probe before this text** (existing behaviour, not a check): main's release `rcfd` binary of 2026-11-05 (built before ADR
0196, so with the old single SV202 text), `rcfd check --json` on two texts without `[boundaries]`: a `box_2d` mesh (nx 10, ny 2,
lx 1, ly 0.1) with `solver.algorithm = "compressible"` and `end_time = 0.001`: exit 0, `rules` exactly one entry, SV202; the same
mesh with the default algorithm: exit 3, four diagnostics "no boundary condition for mesh patch '<p>'" at `boundaries.<p>` for
xmin, xmax, ymin, ymax, `rules` empty. These two texts are A2's cases below.

Where the texts are pinned (searched over the whole tree, excluding `target/`): SV202's texts appear in `setup_rules.json`, in
`rules_w19c.rs` E5 (two constants, asserted byte for byte), in `docs/case-schema.md` (paraphrase) and in ADRs and roadmap notes
(history, not edited). No golden file, no MCP tools-list golden and no CLI help golden contains them. The `rcfd --help` text is
asserted only by `cli.rs` `help_and_version_and_unknown_command`, which checks that it contains eight words; no golden holds it.
So no part needs a golden change.

## A. F-328: SV202's fuller text (ADR 0196 question 1, option (b))

**The option.** ADR 0196 Q1 (b): "also say what the code does in the two cases the decided texts leave out: for the
incompressible solvers a case without boundaries is refused, every mesh patch needs a `boundaries.<patch>` entry, and an entry
without keys is a fixed no-slip wall; for the compressible solver a patch of kind outlet is extrapolated, not a slip wall."
Recommended there; taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation). It is a text change
only: the id, severity, category, path, condition and the `first_of` selection of ADR 0196 C stay.

**A0. The change.** In `crates/rcfd-api/rules/setup_rules.json`, the two `message` strings of SV202's `first_of` become (exact,
fixed now):

- compressible entry: `no boundary conditions are set: the compressible solver makes every patch a slip wall, except a patch of kind outlet, which it extrapolates`
- last entry: `no boundary conditions are set: this solver refuses the case, since every mesh patch that is not empty or cyclic needs an entry boundaries.<patch>; an entry without keys is a fixed no-slip wall`

The words "not empty or cyclic" come from `sim.rs` line 1380 (those kinds are skipped). `<patch>` is literal text (only `{name}`
is substituted, and SV202 has no `each`). `docs/case-schema.md`'s sentence on SV202 is updated. Nothing else in the rule file.

**Registered edit E5** (`crates/rcfd-api/tests/rules_w19c.rs`, `e5_sv202_names_the_default_of_the_solver_adr0196`): only the
values and doc comments of the two constants change. Before: `SV202_SLIP` = `no boundary conditions are set: every patch takes
the default of the compressible solver (a slip wall)`, `SV202_NO_SLIP` = `no boundary conditions are set: every patch takes the
default (a fixed no-slip wall)`. After: the two texts of A0, the compressible one in `SV202_SLIP`, the other in `SV202_NO_SLIP`.
The body of E5 is byte-unchanged.

**Checks.**

| Id | Check | Limit |
|---|---|---|
| A1 | E5 as edited: the default algorithm and every value of `solver.algorithm`'s enum but `compressible` give exactly one SV202 with `SV202_NO_SLIP`, `compressible` with `SV202_SLIP`, byte for byte; id, severity info, category and path `boundaries`, no fix; none with one boundary entry | exact strings, count 1 |
| A2a | new `e6_sv202_fuller_text_through_check_only_adr0235` (same file): `rcfd_api::runctl::check_only` on the probe's compressible text | exit 0; `rules` has exactly 1 entry, id `SV202`, message equal to the compressible text of A0 |
| A2b | same test: the probe's default-algorithm text (the claim "this solver refuses the case") | exit 3; `rules` empty; exactly 4 diagnostics, messages `no boundary condition for mesh patch '<p>'` at `boundaries.<p>` for xmin, xmax, ymin, ymax |
| A2c | same test: A2b's text with four entries `[boundaries.xmin]`, `[boundaries.xmax]`, `[boundaries.ymin]`, `[boundaries.ymax]` without keys (the claim "an entry without keys" is accepted) | exit 0; no SV202 in `rules`; 0 diagnostics |
| A3 | unchanged: E1 to E4 and E4b of `rules_w19c.rs` pass unedited (21 rules) | pass |

**Mutations** (scratch, each applied alone, restored and compared byte for byte with the commit):

| Id | Mutation | Must fail |
|---|---|---|
| MA1 | SV202's compressible entry's `value` `"compressible"` changed to `"no_such_algorithm"` | A1 and A2a |
| MA2 | the two new message strings of A0 swapped | A1 and A2a |

**Decision rule A.** A1, A2a, A2b, A2c and A3 met and both mutations fail their checks: F-328 is proposed done. A miss of A1 or
A2a means the text is not as registered (a defect of the change, fixed before merge and recorded); a miss of A2b or A2c means the
text claims something the code does not do: the test stays as `*_known_miss` with the limit intact, the claim is listed as open,
and F-328 is proposed partial.

## B. F-329: an `rcfd map` flag for the extended warm start (ADR 0209 question 7, option (c))

**The option.** ADR 0209 Q7: (a) a map option and a study key, which changes MCP descriptors and so the tools-list goldens; (b)
the extended warm start as the default, which changes map rows: not taken (it changes defaults and needs the bit-identity rule
lifted; listed as open); (c) an `rcfd map` flag only, "meanwhile". (c) is taken on the maintainer's standing instruction of
2026-11-11 (take the recommendation); (b) stays open.

**B0. The change.** `crates/rcfd-cli/src/cmd_map.rs`: a value option `--warm-start <flow|extended>` (added to the option list
whose values are skipped as positionals). Absent or `flow`: the map runs with `CaseMapEvaluator` exactly as now. `extended`: the
same map options with `ExtendedMapEvaluator { base, limits }` (ADR 0209 D4). Any other value: usage error (exit 2) `--warm-start
takes flow or extended, got '<v>'`; the option given last, without a value: usage error `--warm-start needs a value: flow or
extended`. Both refusals come before the case is loaded and before anything is written. `--cold`, `--no-retry` and the other
options combine with either value as they combine now. The usage line of `map` in `crates/rcfd-cli/src/main.rs` and the doc
comment of `cmd_map` gain `[--warm-start flow|extended]`; `docs/design-loop.md` documents the flag. No `rcfd-api` code changes.

**Checks** (new file `crates/rcfd-cli/tests/map_warm_f329.rs`, the binary through `CARGO_BIN_EXE_rcfd`, release). Case HE of
ADR 0209 (the text there, unchanged), axis `physics.nu=0.05,0.055,0.06`, `--quiet`. `REL_WARM = 1e-4` as in ADR 0209:
`|a - b| <= REL_WARM max(|a|, |b|)`.

| Id | Check | Limit |
|---|---|---|
| B1 | default rows: the map without `--warm-start` against `opmap::run_and_write` in the test process with `CaseMapEvaluator` (the same case text, points from `parse_axis` and `grid_points(.., false)`, `MapOptions { on_failure: Continue, retry_cold: true, cold: false, out_dir }`); and the map with `--warm-start flow` against the map without | `map.csv` and `map.json` byte-identical in both comparisons; exit 0 |
| B2 | the map with `--warm-start extended`: exit 0; 3 points, all `converged`; points 1 and 2 `restart` `warm`; point 0's row (its `row_json` object in `map.json`) equal to point 0 of the default map; every metric of points 1 and 2 within `REL_WARM` of the same point of the map with `--cold` (no `--warm-start`) | as stated (exact except the `REL_WARM` comparison) |
| B2x | the map with `--warm-start extended` against `opmap::run_and_write` in the test process with `ExtendedMapEvaluator` (as B1) | `map.csv` and `map.json` byte-identical |
| B2n | negative control: in the default map, points 1 and 2 have `restart` other than `warm` (HE solves the energy equation, so the velocity-and-pressure warm start refuses it) | as stated |
| B3 | refusals: `--warm-start bogus` (before `--quiet`) and `--warm-start` as the last argument | exit 2; stderr contains `--warm-start` (and `bogus` for the first); the `--out` folder does not exist afterwards |
| B4 | `rcfd --help` | stdout contains `[--warm-start flow|extended]` |
| B5 | `map_w32c.rs` and `cli.rs` pass unedited | pass |

**Mutations** (scratch, as in A):

| Id | Mutation | Must fail |
|---|---|---|
| MB1 | `extended` runs `CaseMapEvaluator` | B2 (restart) |
| MB2 | the default (absent / `flow`) runs `ExtendedMapEvaluator` | B1 |
| MB3 | an unknown value is taken as `flow` | B3 |

**Decision rule B.** B1 and B5 are the defaults: a miss of either means the change is not merged and the cause is recorded. B2,
B2x, B2n, B3 and B4 met and the three mutations failing their checks: F-329 is proposed done. A miss of B2's `REL_WARM` comparison
becomes `*_known_miss` with the limit intact (and F-329 partial).

## C. F-269: the vacuous check T2 of `schemes_wave13`

**Why T2 is vacuous** (from the code read, nothing run). ADR 0016 registered: "if `over(BDF2) >= 0.02` (the test has teeth) then
require `over(Bdf2Bounded) <= 0.25 over(BDF2)` and `over(Bdf2Bounded) <= 0.02`; if `over(BDF2) < 0.02` the check is reported as
vacuous, not as passed." `t2_bounded_time_scheme_square_wave` prints "T2 vacuous" and **returns**, so the test passes in that
case. That case is reached without the code under test doing anything: if plain `Bdf2` stops overshooting on this wave (for
example `add_ddt`'s `Bdf2` arm falling back to implicit Euler, or a change of the wave set-up), T2 passes whatever
`Bdf2Bounded` does, since the bounded scheme's result is then never examined. A pass therefore does not show the registered
claim. ADR 0129 measured `over(BDF2)` = 2.467e-2, 23 % above the threshold.

**The corrected check, no library change needed.** `docs/test-process.md` 6.3 item 2 options: (a) keep; (b) fail the vacuous
case; (c) ignore T2 and add a set-up with more overshoot. Recommended there (b). Taken: (b), applied by the house rule (the old
test kept as an ignored record, the corrected test beside it), on the maintainer's standing instruction of 2026-11-11 (take the
recommendation). A faithful check needs no library change: the precondition "the test has teeth" becomes an assertion.

**C0. The change** (`crates/rcfd-fv/tests/schemes_wave13.rs` only):

- `t2_bounded_time_scheme_square_wave` is renamed `t2_bounded_time_scheme_square_wave_setup_defect` and gets `#[ignore = "ADR
  0235 C: passes when plain BDF2 overshoots less than 0.02 (vacuous); superseded by t2b_bounded_time_scheme_square_wave"]` and a
  doc comment saying the same; its body is byte-unchanged.
- new `t2b_bounded_time_scheme_square_wave`: the same set-up (`wave(ConvScheme::Upwind, t, 100, 2.0, 0.8, 1)`, the same `over`),
  `assert!(ob2 >= 0.02)` (teeth), then `assert!(obd <= 0.25 * ob2 && obd <= 0.02)`; it prints both numbers.
- No other test of the file changes; no file of `crates/rcfd-fv/src` changes.

**Checks.**

| Id | Check | Limit |
|---|---|---|
| C1 | `t2b` passes, not ignored | `over(BDF2) >= 0.02`; `over(Bdf2Bounded) <= 0.25 over(BDF2)` and `<= 0.02`; both numbers recorded |
| C2 | the record is ignored; run by name with `--ignored` on the unmutated code it passes (it is not wrong, it is vacuous) | recorded |
| C3 | the other tests of `schemes_wave13.rs` (rcfd-fv) pass unedited | pass |

**Mutations** (scratch, in `crates/rcfd-fv/src/ddt.rs`, restored and compared byte for byte):

| Id | Mutation | Must fail | Recorded |
|---|---|---|---|
| MC1 | `add_ddt`'s `Bdf2` arm always returns the implicit-Euler fallback (plain BDF2 loses its overshoot: the vacuous case) | `t2b` (teeth) | the record `t2_..._setup_defect` run by name: expected to pass, printing "T2 vacuous", which shows the vacuity |
| MC2 | in the `Bdf2Bounded` arm `theta` is always 1 (the bounded scheme is plain BDF2) | `t2b` (`obd <= 0.25 ob2`) | |

**Decision rule C.** C1 met and both mutations make `t2b` fail (and MC1 leaves the record green): the F-269 open item "a vacuous
pass in the schemes_wave13 T2 test" is resolved, and its removal from the note is proposed. A miss of C1 becomes
`t2b_..._known_miss` with the limits intact and the item stays open.

## Predictions (mine)

- A: all met (the probe already showed A2's set-ups behave as the texts say).
- B: B1 met; B2 met with differences of order 1e-8 (ADR 0209 W1 gave 6.9e-9 on the inlet-velocity axis); B2n met.
- C: `over(BDF2)` 2.467e-2 (ADR 0129); `over(Bdf2Bounded)` below 6e-3.

## Runs and tools

Each registered test is run once in release (`cargo test --release -p rcfd-api --test rules_w19c`, `-p rcfd-cli --test
map_warm_f329 --test map_w32c --test cli`, `-p rcfd-fv --test schemes_wave13`); then `cargo fmt -p <crate> --check` for
rcfd-api, rcfd-cli, rcfd-fv and `cargo clippy -p rcfd-api -p rcfd-cli -p rcfd-fv --all-targets -- -D warnings`; then the
mutations. The whole-workspace test suite and the gate are not run on this branch (the lead gates merged trees).

## Commits

(1) this text alone; (2) A with its tests; (3) B with its tests; (4) C; (5) documents; (6) addendum 1.

## Addenda

Results follow.
