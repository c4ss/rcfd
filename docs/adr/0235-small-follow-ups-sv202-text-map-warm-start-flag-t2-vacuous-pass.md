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

### Addendum 1: results (2026-11-12)

Commits on `f328-small-followups` (from main bf3376b1): registration 52db06e5; A ebc51ad5; B 05a623a6; C de016bab; documents
9d11f328; rustfmt of the new tests (layout only, no assertion changed) 6d413564. Each registered test was committed before its
first run and run once in release, at de016bab (the documents and layout commits change no tested code):
`cargo test --release -p rcfd-api --test rules_w19c`; `-p rcfd-cli --test map_warm_f329 --test map_w32c --test cli -- --nocapture
--test-threads=1`; `-p rcfd-fv --test schemes_wave13 -- --nocapture --test-threads=1`; the record of C2 by name with `--ignored`.
No limit, case or test set-up was changed after a run; no amendment was needed.

| Id | Measured | Limit | Result |
|---|---|---|---|
| A1 | E5 (edited constants) passed: default, `simple`, `simplec`, `coupled`, `piso`, `pimple` give the no-slip text, `compressible` the slip text, each byte for byte, count 1; no SV202 with a boundary entry | exact | met |
| A2a | `check_only`, compressible `box_2d` without boundaries: exit 0, `rules` one entry, SV202, the A0 compressible text | as stated | met |
| A2b | default algorithm: exit 3, `rules` empty, 4 diagnostics `no boundary condition for mesh patch '<p>'` at `boundaries.<p>` for xmin, xmax, ymin, ymax | as stated | met |
| A2c | four entries without keys: exit 0, 0 diagnostics, no SV202 | as stated | met |
| A3 | E1 to E4 and E4b passed unedited (8 passed, 1 ignored: the existing `e2_..._known_miss`) | pass | met |
| B1 | default map (630 + 1520 bytes) byte-identical to `CaseMapEvaluator` through `run_and_write`; `--warm-start flow` byte-identical to the default | exact | met |
| B2 | `--warm-start extended`: exit 0, 3 points converged, points 1 and 2 `warm`, point 0 equal to the default's; largest relative difference to `--cold` 5.67e-9 (`dp`, point 1); iterations warm 61 and 60 against cold 89 and 88 | `REL_WARM` 1e-4 | met |
| B2x | the extended map byte-identical to `ExtendedMapEvaluator` through `run_and_write` | exact | met |
| B2n | default map restarts `cold, cold, cold` | not `warm` for points 1, 2 | met |
| B3 | `bogus`: exit 2, `error: --warm-start takes flow or extended, got 'bogus'`; no value: exit 2, `error: --warm-start needs a value: flow or extended`; no out folder in either case | as stated | met |
| B4 | `rcfd --help` contains `[--warm-start flow|extended]` | as stated | met |
| B5 | `map_w32c` 2 passed, `cli` 8 passed, both unedited | pass | met |
| C1 | `t2b`: over(BDF2) 2.4667e-2, over(Bdf2Bounded) 1.1564e-9 | >= 0.02; <= 0.25 over(BDF2) and <= 0.02 | met |
| C2 | the record is ignored in the default run; by name with `--ignored` it passes, with the same two numbers | recorded | recorded |
| C3 | the other tests of `schemes_wave13.rs`: 7 passed (with `t2b`), 6 ignored (5 existing records and known misses, plus the T2 record) | pass | met |

**Mutations** (scratch, each applied alone to the tree of 6d413564, the named tests run once, the file restored and compared
byte for byte with the commit: identical every time; `git status` clean afterwards):

| Id | Measured | Result |
|---|---|---|
| MA1 | E5 failed (`compressible` got the no-slip text); E6 failed at A2a | fails, as registered |
| MA2 | E5 failed (`default` got the slip text); E6 failed at A2a | fails, as registered |
| MB1 | the B1/B2 test failed at B2 (point 1 restart `cold`, want `warm`); B3 and B4 passed | fails, as registered |
| MB2 | failed at B1 (`the default map differs from CaseMapEvaluator's`) | fails, as registered |
| MB3 | B3 failed (`bogus`: exit 0, empty stderr); B1/B2 and B4 passed | fails, as registered |
| MC1 | applied as: the `Bdf2` arm matches `None` for the older level, so it always takes its implicit-Euler fallback (the registered effect). over(BDF2) 0: `t2b` failed ("no teeth"); the record **passed**, printing "T2 vacuous" | fails, and the record's vacuity is shown |
| MC2 | applied as: `theta` shadowed by 1 after its computation. over(Bdf2Bounded) = over(BDF2) = 2.4667e-2: `t2b` and the record failed | fails, as registered |

**fmt and clippy.** `cargo fmt -p rcfd-api --check`, `-p rcfd-cli`, `-p rcfd-fv`: the first check flagged only the new test code.
After `cargo fmt -p` (6d413564, layout only) all three exit 0. `cargo clippy -p rcfd-api -p rcfd-cli -p rcfd-fv --all-targets
--release -- -D warnings`: no warning.

**Deviations.** (1) The A tests were run without `--nocapture`, so their printed lines were not kept. Every A limit is an exact
assertion, so the pass is the measurement. (2) MC1 and MC2 were written differently from their wording (see above) but have the
registered effect. (3) The extra layout commit 6d413564 is not in the commit plan.

**Not run.** The whole-workspace test suite and the gate (by this ADR's rule). `baseline_hashes.rs` and `case_hashes_w48u.rs`
are unedited, and no solver or case code changed: the diff against bf3376b1 touches `setup_rules.json`, `cmd_map.rs`, the `map`
usage line of `main.rs`, three test files, two documents and this ADR. No MCP or CLI golden holds the changed texts; none was
edited.

**Decisions by the rules.** A: every check met and both mutations caught, so F-328 is proposed done. B: B1 and B5 met (defaults
bit-identical), the rest met, three mutations caught, so F-329 is proposed done. C: C1 met, both mutations caught by `t2b`, and
MC1 left the record green, so the F-269 item "a vacuous pass in the schemes_wave13 T2 test" is proposed resolved.

**Remaining questions** (for the maintainer).

1. **The extended warm start as the map default** (ADR 0209 Q7 (b)). Not taken: it changes the map rows of energy and turbulence
   cases (here 5.7e-9 relative on HE, with about a third fewer iterations) and needs the bit-identity rule lifted. Options: (a)
   keep the flag only; (b) make `extended` the default of `rcfd map` (and later of the design tools, with Q7 (a) and its golden
   edits). Recommendation: (b) once the maintainer lifts the rule for maps; (a) until then.
2. **The design tools** (ADR 0209 Q7 (a)). `design_run_operating_map` and chained study conditions do not get the option, because
   their descriptors are in the tools-list goldens. Options: (a) leave them as they are; (b) a registered golden edit that adds
   `warm_start`. Recommendation: (b), together with ADR 0209 Q6's description edit, as one registered golden change.

## For the lead: roadmap entries

- **F-328.** Title: "SV202's fuller text: what the solvers do with a case without boundary conditions". Deps: F-240. Acceptance:
  "For the compressible solver SV202 states that every patch is a slip wall and an outlet is extrapolated. For every other solver
  it states that the case is refused unless every mesh patch that is not empty or cyclic has a boundaries entry, and that an entry
  without keys is a fixed no-slip wall. Each claim is checked through check_only (ADR 0235 A)." Status: done. Note: "ADR 0235 A
  (ADR 0196 Q1 (b), taken on the maintainer's standing instruction of 2026-11-11): a text change in setup_rules.json, a registered
  edit of rules_w19c.rs E5 and a new check E6; all checks met, two mutations caught. No golden held the text."
- **F-329.** Title: "`rcfd map --warm-start extended`: the extended warm start on the command line". Deps: F-249. Acceptance: "With
  --warm-start extended, a map of an energy or two-equation-turbulence case restarts each point warm, and its reports agree with
  cold runs within REL_WARM 1e-4. Without the flag, or with flow, the map tables are byte-identical to before (ADR 0235 B)."
  Status: done. Note: "ADR 0235 B (ADR 0209 Q7 (c)): HE map warm against cold 5.7e-9, 61 and 60 iterations against 89 and 88;
  defaults byte-identical; three mutations caught. Open: the extended start as the map default (Q7 (b)) and in the design tools
  (Q7 (a), a golden edit)."
- **F-269 note change.** Remove "a vacuous pass in the schemes_wave13 T2 test (it should fail the vacuous case);" from the open
  items and add: "The T2 vacuous pass is closed by ADR 0235 C: t2b asserts the registered precondition over(BDF2) >= 0.02
  (measured 2.467e-2, bounded 1.16e-9); the old T2 stays as an ignored setup_defect record."
