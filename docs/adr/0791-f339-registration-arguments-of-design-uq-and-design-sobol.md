# ADR 0791: F-339 registration: every argument of `design_uq` and `design_sobol` sent at a value that tells a wrong reading from the right one, and the string and null bounds

Status: **REGISTERED 2026-11-25, written before any test of this work and before any run of its checks.** Before this text
no test or library code was written and no file of the repository was changed. What was done: reading (below), searches of
the source tree, one run of `python scripts/process_report.py --check` on the unchanged tree (0 failures) and a release
build of the unchanged library into the scratch folder's own target folder. No study was run and no number of a result was
computed. Results go in addenda at the end; nothing above them is changed afterwards.

It answers the brief `docs/adr/briefs/f339-uq-sobol-test-gaps.md` ([the file](briefs/f339-uq-sobol-test-gaps.md)) and
follows [docs/process/agent-rules.md](../process/agent-rules.md), version 5. Branch `f339-uq-sobol-test-gaps` from main
c49eff4b, scratch `w49i-work/f339-uq-sobol-test-gaps-scratch`. Model: Claude Opus 5.5.

## What is asked

The acceptance of F-339 is in [ADR 0784](0784-f339-registration-mcp-tools-for-tolerance-and-sobol-studies-of-the-reduced-order-fan.md),
section "What is asked". This work closes two findings of the reader of ADR 0784 (its section "A reader's report"):

- **1-1**: no test sends `"-inf"`, `"inf"` or `null` as a bound of a normal distribution, though deviation 2 of addendum 1
  says one does.
- **2-1**: the reference inputs take values at which a swapped Beta `a` and `b`, a dropped `mean` or `system_l` or the
  default `seed` give the same bytes; `circular_arc`, `cmp "<="` and a non-zero `se_abs` are never sent.

The maintainer's word (the brief, 2026-11-25): one agent for the two gaps. The sentence this work must be able to write:
for each named wrong reading a committed or recorded mutation makes it, a registered test fails with the mutation and
passes without it. X1's miss (ADR 0784) is not touched.

## What was read

`docs/process/agent-rules.md` (version 5, whole); the brief (whole); ADR 0784 (whole);
`crates/rcfd-mcp/tests/design_uq_sobol_f339.rs` and its mutation patch (whole); the item F-339 of `docs/roadmap.json` (by a
script); `crates/rcfd-api/src/design/tools.rs` from `bound_at`'s section to the end, and its helpers `f64_of`, `str_arg`,
`opt_str`, `opt_usize`, `ledger_at`; `uq.rs` (the distributions, the spec, `run`, `result_json`), `sensitivity.rs` (`run`,
`result_json`) and `fan_rom_eval.rs` (whole) of `crates/rcfd-api/src/design`; `crates/rcfd-post/src/fan_rom.rs` to the end
of `free_vortex_design`; `camber_line` of `crates/rcfd-mesh/src/fan.rs`; `erf`, `erfc` and `erfc_inv` of
`crates/rcfd-reactor/src/mixfrac.rs`; of `crates/rcfd-mcp/src/lib.rs` the two descriptors, `validate_args` and the two arms
of `dispatch`; `crates/rcfd-mcp/tests/common/mod.rs` (head); the section on the two tools of
[docs/mcp-tools.md](../mcp-tools.md); the newest registrations for the form. The reader's scratch folder was listed, not
read further: its findings stand in ADR 0784.

## Findings of the reading (before any run)

1. **The library cannot sample a normal distribution that is open on one side only.** `erfc(+inf)` forms
   `del = c * d = inf * 0` in its first step and returns a value that is not a number, so `uq::norm_cdf` is not a number
   at both infinities. `Dist::validate` then finds the kept mass not a number and lets it pass (`NaN < 1e-12` is false), and
   `Dist::inv_cdf`, whose branch for two open sides is not taken, hands `norm_inv` a value that is not a number:
   `erfc_inv` asserts `0 < y < 2` and panics. The samples are drawn outside the panic guard of the evaluator, and
   `rcfd-mcp` has none, so such a call is expected to panic in the server. `lo` and `hi` are each optional by
   [docs/mcp-tools.md](../mcp-tools.md) ("no truncation on that side"); only both or neither work. `tools.rs` reads such a
   call rightly; the defect is in `rcfd-reactor` (`erfc`) or in `uq.rs` (its callers). Neither file is this work's and the
   brief forbids a change of solver source: it is reported, held as the record K1 below, and proposed as a follow-up.
2. **Therefore the open forms are sent in pairs** (both sides open, each in its own form), and the wrong reading that has
   to differ is a finite bound on one side with the other far out but finite (`-8` or `8`, more than 26 standard deviations
   from the mean, where the library is finite).
3. **`result` repeats the spec for `design_uq` but not the distributions for `design_sobol`**, and neither repeats the
   evaluator's settings. A wrong reading of a spec value can so be seen in the echo alone. The controls below therefore
   compare the results without their `spec` (the numbers), wherever the argument can change a number.
4. **Some settings change no number of a usable sample.** `cd_normal` acts in stalled strips, and a sample whose operating
   point has a stalled strip is refused; `camber_pos` is not read by the circular arc. Every field of the evaluator's
   settings is in the `geometry_hash` of each ledger entry (`FanRomModel::hash_of`). Each equality below is therefore of
   two texts: the reply's `result`, and the ledger file but for the stamp.
5. **Other arguments with the same weakness** (the existing tests cannot tell a wrong reading), from reading every
   argument: four keys of `model` are never sent with success (`cl_max`, `cd_normal`, `annulus_drag`, `secondary_drag`);
   `c0` (343), `observer_r` (1), `observer_theta` (pi / 2), `n_scan` (100), `camber_pos` (0.5) are natural defaults; the
   sections share chord, camber and camber position, the chord of 0.030 is also `r_hub` and the first radius; the bounds of
   the uniform and the normal distribution are symmetric; the inputs are in the evaluator's order; `cmp ">="` is never
   beside a `<=`; `sampler "monte_carlo"` is never sent by name; `metrics` never has two names; the defaults of
   `min_batches` are seen in the echo only; no nested `null` (`stop.min_batches`, `se_abs`, `se_rel`, `metrics`,
   `evaluator_settings.model`) is sent; the default of `seed` is never used; no study has another number of inputs than
   three, so the limit of `design_sobol` is the same for `5 n` and for `n (d + 2)`. All are registered below.
6. **Not distinguishable, so not registered as an equality**: `workers` (the results do not depend on it, by design; only
   its refusal at 0 is, row W1). **Not reachable through the server**: a `null` for an optional top-level argument (the
   argument check refuses the type), so row N2 calls the handlers directly.
7. **An observation, no row**: `dist` accepts a key of another distribution (a `mean` in a uniform one) and ignores it.
   ADR 0784 says an unknown key is refused; whether a known key of another kind is one is not said there. Left to the lead.

## Design (registered)

One new file, `crates/rcfd-mcp/tests/design_uq_sobol_args_f339.rs`, on an in-process `McpServer` over a sandbox root, and
its mutation patch. `tools.rs` is not changed. The references are built by hand from the Rust types and run by the library
with a ledger of their own (clock 1), and share no code with the tools' parsers. "Equal" below means: the reply's `result`
as text is the library's `result_json` as text (equal text is equal bits, ADR 0784, "Arithmetic"), `entries_appended` is
the number of the reference's lines, and the ledger files are equal line by line once the stamp is set to 0. "Differs"
means: the library's result for the wrong input, without `spec`, is another text than for the right one; where finding 4
says no number can change, the ledger lines differ.

### Set-up (fixed here, not searched for)

Fan F(family): 7 blades, hub 0.0305 m, tip 0.0575 m, tip gap 0.0011 m; two base sections at the radii 0.029 and 0.059 m
(chord 0.032 and 0.027 m, camber 0.045 and 0.035, camber position 0.42 and 0.46); staggered by `free_vortex_design` at
`Q_D = 0.045` m3/s with the vortex `45 / 1.2 / omega`, the operating state 2950 rev/min, 1.18 kg/m3, 1.55e-5 m2/s, and the
model Mo below, which gives 8 sections, no two with an equal value in a field. Model Mo: `n_strips 8`, `cl_max 1.05`,
`cd_normal 1.7`, `friction false`, `annulus_drag 0.025`, `secondary_drag 0.015`, `tip_cd 0.5` (no key at its default).
Condition: the same state, `k = 0.7 dp / Q_D^2` and `l = 0.3 dp / Q_D` with `dp` the fan's static rise at `Q_D`, so that the
system curve passes through the design point, `c0 345`, observer 1.5 m and 1.1 rad, `n_scan 24`. Estimate by hand for the
hub strip: blade speed 9.9 m/s, axial 6.0 m/s, swirl 3.8 m/s, lift coefficient about 0.7 against `cl_max` 1.05, that is
3 degrees of stagger from stall, so samples within 1 degree are expected to be usable.

Studies: d1 is one input, `stagger_common_deg` normal, mean 0.1, sd 0.3, untruncated; d3 is three inputs in the order
`tip_gap_delta` uniform on `[-1.5e-4, 2.5e-4]`, `stagger_common_deg` normal (0.2, 0.4) on `[-0.6, 0.9]`, `chord_rel` beta
(2, 3.5) on `[-0.012, 0.018]`. Unless a row says otherwise `design_uq` runs `batch 5`, `max_samples 10` (2 batches, as a
target of 0 is not met) and `design_sobol` `n 3`, seed 339101. Guard of every reference: at least two usable samples or
rows; a guard that fails is a defect of the set-up and is corrected by an amendment, not a result.

### Rows: input, library call, mutation

"`uq`" is `uq::run` and "`sobol`" `sensitivity::run` with the stated spec on the stated model; both tools means one call of
each. The mutations are in the table after the checks.

| Row | Input sent | Library call it must equal | Differs (control) | Mutations |
|---|---|---|---|---|
| B1 | d1 with each of the nine pairs of `lo` in {`"-inf"`, `null`, absent} and `hi` in {`"inf"`, `null`, absent}, both tools | `uq`, `sobol` with `lo = -inf`, `hi = inf` | `lo = -0.2, hi = 8`; `lo = -8, hi = 0.25` (a finite bound on either side) | M10 to M15 |
| B2 | `lo` and `hi` in turn as `"Infinity"`, `"-Infinity"`, `"-Inf"`, `"INF"`, `"+inf"`, `"nan"`, `"NaN"`, `"none"`, `""`, `" inf"`, `"-0.5"`, `"0.5"`, `true`, `[]`, `{}` | refused: -32602, `params[0].dist.<key>: expected a finite number`, no file | B1 (the two strings are accepted) | M16, M17 |
| B3 | `lo: "inf"` (with `hi` absent, and 0.5); `hi: "-inf"` (with `lo` absent, and -0.5) | refused by the spec's check: -32602, `parameter 'stagger_common_deg': truncation`, no file | B1 | M18 |
| B4 | uniform and beta with `lo` `"-inf"`, `null`, absent; `hi` `"inf"`, `null`, absent | refused: -32602, `params[0].dist.<key>: expected a finite number`, no file | D1 (finite ones are accepted) | M19, M20 |
| D1 | d3, both tools | the same inputs in the same order | Beta (3.5, 2); Beta (2, 2); mean 0; normal without bounds; normal on `[-0.9, 0.6]`; uniform on `[-2.5e-4, 1.5e-4]`; the inputs in the evaluator's order | M1, M2, M38 |
| E1 | d3 on F(`naca4`) with every field, `model` with all seven keys, both tools | the hand-built model | each of the 7 model keys at its default; each of the 9 condition values at a natural one (`rpm 3000`, `rho 1.2`, `nu 1.5e-5`, `k` and `l` 0, `c0 343`, `observer_r 1`, `observer_theta pi / 2`, `n_scan 100`); each of the 5 scalars of the fan and each of the 5 fields of the sections (the second section's value in the first) | M3, M21 to M27 |
| E2 | d3 on F(`circular_arc`), both tools | the hand-built model with the circular arc | the same numbers as `naca4` | M7 |
| S1 | seed 339101 and 2^53 + 1, both tools (d3) | the same seeds | seed 1; seed 2^53 | M4, M6 |
| S2 | no `seed`, both tools | seed 1 | seed 0; seed 2 | M5 |
| C1 | d1, requirements `q <= 1` and `eta_s >= 2` | the two, in this order | `q >= 1`; `eta_s <= 2`; one or both exchanged | M8, M28 |
| A1 | d1, `stop` `{batch 4, max_samples 16, min_batches 2, se_abs 1, metrics ["q", "torque"]}` | the same rule | `se_abs 0`; `min_batches 1` | M9, M40 |
| A2 | d1, `se_abs 1` with `metrics ["q"]`: Monte Carlo without `min_batches`; by the name `"monte_carlo"`; `lhs` without; `lhs` with `min_batches 3` | `min_batches` 1, 1, 2, 3 | Monte Carlo with 2; `lhs` with 3 and with 2; the other sampler | M29 to M31 |
| N1 | d1 on F with `evaluator_settings.model` `null` (the default model) and `stop` with `min_batches`, `se_abs`, `se_rel`, `metrics` `null`, `batch 3`, `max_samples 3`; and the same with those keys absent | the defaults: `RomModel::default()`, 1, 0, 0, all | model Mo | M32 to M34 |
| N2 | the handlers `uq_study` and `sobol_study` directly, with every optional top-level argument `null`; through the server the same is refused as a JSON-RPC error | the defaults: seed 1, Monte Carlo, quantiles 0.05, 0.5, 0.95, confidence 0.95, no requirement, 1 worker | the refusal through the server | M37, M39 |
| W1 | the handlers directly with `workers` 0 | refused: -32602, `workers:`, no file | N2 (absent is 1) | M35 |
| G1 | d1, `design_sobol` with `max_design_evaluations` 100: `n` 34 (102) and 33 (99) | 34 refused with 1004; 33 accepted, 100 entries | each other | M36 |

Why the rules of A1, A2 and C1 bite, without a run: every `q` of a usable sample lies in `(0, A U_tip]`, below 0.15 m3/s
(`A = 7.46e-3 m2`, `U_tip = 17.8 m/s`), so its standard error is below 1 and the rule with `se_abs 1` on `q` is met at
the first batch it may stop; the torque is below 1 N m (power below 10 W at 309 rad/s); a target of 0 is not met unless all
samples are equal; `q <= 1` is never violated and `q >= 1` always; `eta_s >= 2` is always violated (an efficiency
below 1) and `eta_s <= 2` never. For a Latin hypercube one replicate gives no standard error.

## Checks

| Id | Clause | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|---|
| B1 | 1-1 | row B1 of the table above | equal (18 calls), controls differ | acceptance of F-339 (the tools run `uq::run` and `sensitivity::run`); [tolerances](../process/tolerances.md), bit-identity | equal | exact | code verification |
| B2 | 1-1 | row B2 | code and key as stated, no file | ADR 0784, table of errors (a malformed `dist`); acceptance of F-339 | refused | exact | code verification |
| B3 | 1-1 | row B3 | code and key as stated, no file | ADR 0784, table of errors (`validate` of the spec); acceptance of F-339 | refused | exact | code verification |
| B4 | 1-1 | row B4 | code and key as stated, no file | ADR 0784, arguments (`lo` and `hi` optional for the normal only); acceptance of F-339 | refused | exact | code verification |
| D1 | 2-1 | row D1 | equal, controls differ | acceptance of F-339; tolerances, bit-identity | equal | exact | code verification |
| E1 | 2-1 | row E1 | equal, controls differ | acceptance of F-339; tolerances, bit-identity | equal | exact | code verification |
| E2 | 2-1 | row E2 | equal, control differs | acceptance of F-339; tolerances, bit-identity | equal | exact | code verification |
| S1 | 2-1 | row S1 | equal, controls differ | acceptance of F-339; tolerances, bit-identity | equal | exact | code verification |
| S2 | 2-1 | row S2 | equal, controls differ | ADR 0784, arguments (default 1); acceptance of F-339 | equal | exact | code verification |
| C1 | 2-1 | row C1 | equal, controls differ | acceptance of F-339; tolerances, bit-identity | equal | exact | code verification |
| A1 | 2-1 | row A1 | equal, controls differ | acceptance of F-339; tolerances, bit-identity | equal | exact | code verification |
| A2 | 2-1 | row A2 | equal, controls differ | ADR 0784, arguments (the defaults 1 and 2); acceptance of F-339 | equal | exact | code verification |
| N1 | 1-1 | row N1 | equal, control differs | ADR 0784, arguments (the defaults); acceptance of F-339 | equal | exact | code verification |
| N2 | 2-1 | row N2 | equal; -32602 through the server | ADR 0784, arguments (the defaults); acceptance of F-339 | equal | exact | code verification |
| W1 | 2-1 | row W1 | code and key as stated, no file | ADR 0784, arguments (`workers` at least 1); acceptance of F-339 | refused | exact | code verification |
| G1 | 2-1 | row G1 | as stated | ADR 0784, "The sandbox and the ledger" (`n (d + 2)`); acceptance of F-339 | as stated | exact | code verification |
| X1 | rule 3 | existing tests, unedited: `cargo test -p rcfd-mcp --release` (with `design_uq_sobol_f339`), `cargo test -p rcfd-api --release --lib` | pass | rule 3; rule 7 | pass | exact | unchanged default output |
| K1 | record | finding 1: `design_uq` with d1 and one open side (`hi` 0.25 alone; `lo` -0.2 alone) | the call returns a result, every stagger of the ledger within the bound | [docs/mcp-tools.md](../mcp-tools.md) ("no truncation on that side"); ADR 0784, arguments | a panic (finding 1): expected miss | exact | code verification |

K1 is a record of a defect outside `tools.rs`, known from the reading and not a wrong reading of an argument; it is an
ignored test `k1_one_open_side_known_miss` with its limit intact, run once, and is **not** part of the decision rule below.
If it passes, finding 1 is wrong and the addendum says so.

### Mutations (rule 16)

Each in `tools.rs` on a scratch copy of the final commit, the new test file run once, the file restored. A mutation the
compiler refuses is not a mutation and is replaced by an amendment.

| Id | Wrong reading | Must fail |
|---|---|---|
| M1 | Beta `a` and `b` swapped | D1 |
| M2 | `mean` dropped (0) | D1 |
| M3 | `system_l` dropped (0) | E1 |
| M4 | `seed` dropped (always 1) | S1 |
| M5 | the default `seed` is 0 | S2 |
| M6 | `seed` read through a double | S1 |
| M7 | `circular_arc` read as `naca4` | E2 |
| M8 | `"<="` read as `>=` | C1 |
| M9 | `se_abs` dropped (0) | A1 |
| M10 | `"-inf"` read as -1 | B1 |
| M11 | `"inf"` read as 1 | B1 |
| M12 | a `null` bound not taken as open | B1 |
| M13 | an absent bound refused | B1 |
| M14 | an absent or `null` `lo` is -1 | B1 |
| M15 | an absent or `null` `hi` is 1 | B1 |
| M16 | any string is an open bound | B2 |
| M17 | a string is parsed as a double | B2 |
| M18 | either string is the open bound of its own side | B3 |
| M19 | `lo` of a uniform read as the normal's | B4 |
| M20 | `hi` of a beta read as the normal's | B4 |
| M21 | `model.cl_max` dropped | E1 |
| M22 | `model.cd_normal` dropped | E1 |
| M23 | `model.annulus_drag` and `model.secondary_drag` swapped | E1 |
| M24 | `c0` 343 and `observer_r` 1 whatever is sent | E1 |
| M25 | `observer_theta` pi / 2 and `n_scan` 100 whatever is sent | E1 |
| M26 | `camber_pos` 0.5 whatever is sent | E1 |
| M27 | `chord` of every section from the first | E1 |
| M28 | `">="` read as `<=` | C1 |
| M29 | `"monte_carlo"` by name read as `lhs` | A2 |
| M30 | the default `min_batches` of Monte Carlo is 2 | A2 |
| M31 | the default `min_batches` of `lhs` is 3 | A2 |
| M32 | `null` `se_abs` and `se_rel` not taken as 0 | N1 |
| M33 | `null` `metrics` not taken as all | N1 |
| M34 | `null` `model` not taken as the default | N1 |
| M35 | `workers` dropped (always 1) | W1 |
| M36 | the limit of `design_sobol` is `5 n` | G1 |
| M37 | `null` `quantiles` and `confidence` not taken as the defaults | N2 |
| M38 | the inputs in reverse order | D1 |
| M39 | `null` `seed` not taken as the default | N2 |
| M40 | only the first of `metrics` is kept | A1 |

The committed one is M1, `crates/rcfd-mcp/tests/design_uq_sobol_args_f339.mutation.patch`, whose first line is
`# expect-fail: cargo test --release -p rcfd-mcp --test design_uq_sobol_args_f339`.

Size (agent): done; 0; 0; 0; 230000

## Decision rule

**done** if every row of the table of checks but the record K1 holds, each named wrong reading is caught by its mutation
(M1 to M40, each by a row it is registered to fail), and the existing tests of `rcfd-mcp` and `rcfd-api --lib` pass
unedited. **partial** otherwise. This is the brief's rule; K1 is left out of it for the reason given with it, said here
before the run, and whether the item can read done beside it is the lead's decision. ADR 0784's X1 is not decided here.

If a row shows `tools.rs` wrong, the failure is recorded first, an amendment registers the fix, then the fix is made; a
fix that would change a golden or a default result is not made and is reported.

## What is not covered

- A fix of finding 1; a change of any library or server source; the goldens, the tool list, the count literals.
- The argument check of the server (`validate_args`) beyond the refusal of N2; the stdio transport (finding 2-2 of the
  reader); the accuracy of the drivers and of the fan, which are unchanged. No check of kind validation is claimed: the
  acceptance claims no agreement with a measured case, and no reference value is added.
- Keys of another distribution in `dist` (finding 7).

## Addendum 1: results (2026-11-25)

Registration e2a43a88, tests b853a0cd, committed mutation 7db781c3. The registered run was made once, at b853a0cd:
`cargo test -p rcfd-mcp --release --test design_uq_sobol_args_f339 -- --nocapture --test-threads 1`, 16 passed, 0 failed,
1 ignored (K1), 34.0 s in series (about 9 s with the default threads). `tools.rs` was not changed and no row showed it wrong.

| Id | Limit | Measured | Met |
|---|---|---|---|
| B1 | equal, controls differ | 18 calls equal (nine pairs of forms, both tools); a finite bound on either side changes the numbers of both drivers | yes |
| B2 | refused with the key, no file | 15 values for `lo` and for `hi`, both tools, 60 refusals; nothing written | yes |
| B3 | refused with the key, no file | 4 pairs, both tools; nothing written | yes |
| B4 | refused with the key, no file | 12 forms; nothing written | yes |
| D1 | equal, controls differ | both tools equal; 7 wrong readings differ in both | yes |
| E1 | equal, controls differ | both tools equal; 26 wrong fields give another ledger, 21 of them other numbers too (deviation 1) | yes |
| E2 | equal, control differs | both tools equal; `naca4` differs in both | yes |
| S1 | equal, controls differ | both tools equal for 339101 and 2^53 + 1; seed 1 and 2^53 differ | yes |
| S2 | equal, controls differ | both tools equal seed 1; seeds 0 and 2 differ | yes |
| C1 | equal, controls differ | equal; 4 wrong readings differ | yes |
| A1 | equal, controls differ | equal, 2 batches of 4; without `se_abs` or `min_batches` the numbers differ | yes |
| A2 | equal, controls differ | 4 forms equal, batches 1, 1, 2, 3; the other sampler and counts differ | yes |
| N1 | equal, control differs | absent keys and `null` equal the defaults; another model differs | yes |
| N2 | equal; refused through the server | both handlers equal the defaults; 8 refusals through the server | yes |
| W1 | refused with the key, no file | both handlers; nothing written | yes |
| G1 | 102 refused, 99 accepted | 1004 naming 102 and 100; 99 evaluations, 100 entries | yes |
| X1 | pass | `rcfd-mcp`: 144 passed, 0 failed, 11 ignored over 36 test results (`design_uq_sobol_f339` 11 passed); `rcfd-api --lib`: 159 passed, 0 failed, 4 ignored; no existing test edited (second run, deviation 2) | yes |
| K1 | a result, samples within the bound | the first call panics: `erfc_inv needs 0 < y < 2`, `crates/rcfd-reactor/src/mixfrac.rs:161` | **no** (record, as expected) |

K1 confirms finding 1. Beside the checks, one call through the built `rcfd-mcp` on stdio (a normal with `hi` 0.25 and no
`lo`, then a `ping`) ended the server process with exit code 101, the panic on standard error, no reply to either message
and no file written: a client that sends one documented form ends the server.

### Mutations

On a scratch copy of b853a0cd, each alone in `tools.rs`, the new file run once, the file restored; the unchanged copy
passes (16 passed). All 40 are caught by the row registered for them (the script and the logs are in the scratch folder,
`mutate.py`, `mutations.tsv`).

| Id | Registered to fail | Failed |
|---|---|---|
| M1 | D1 | D1, E1, E2, N2, S1, S2 |
| M2 | D1 | A1, A2, B1, C1, D1, E1, E2, N1, N2, S1, S2 |
| M3 | E1 | A1, A2, B1, C1, D1, E1, E2, N1, N2, S1, S2 |
| M4 | S1 | A1, A2, B1, C1, D1, E1, E2, N1, S1 |
| M5 | S2 | N2, S2 |
| M6 | S1 | S1 |
| M7 | E2 | E2 |
| M8 | C1 | C1 |
| M9 | A1 | A1, A2 |
| M10 | B1 | B1, B3 |
| M11 | B1 | B1, B3 |
| M12 | B1 | B1 |
| M13 | B1 | A1, A2, B1, B2, B3, C1, G1, N1, W1 |
| M14 | B1 | A1, A2, B1, C1, G1, N1 |
| M15 | B1 | A1, A2, B1, C1, G1, N1 |
| M16 | B2 | B2 |
| M17 | B2 | B2 |
| M18 | B3 | B3 |
| M19 | B4 | B4 |
| M20 | B4 | B4 |
| M21 | E1 | A1, A2, B1, C1, D1, E1, E2, N2, S1, S2 |
| M22 | E1 | A1, A2, B1, C1, D1, E1, E2, N2, S1, S2 |
| M23 | E1 | A1, A2, B1, C1, D1, E1, E2, N2, S1, S2 |
| M24 | E1 | A1, A2, B1, C1, D1, E1, E2, N1, N2, S1, S2 |
| M25 | E1 | A1, A2, B1, C1, D1, E1, E2, N1, N2, S1, S2 |
| M26 | E1 | A1, A2, B1, C1, D1, E1, E2, N1, N2, S1, S2 |
| M27 | E1 | A1, A2, B1, C1, D1, E1, E2, N1, N2, S1, S2 |
| M28 | C1 | C1 |
| M29 | A2 | A2 |
| M30 | A2 | A2, B1, C1, D1, E1, E2, N1, N2, S1, S2 |
| M31 | A2 | A2 |
| M32 | N1 | N1 |
| M33 | N1 | N1 |
| M34 | N1 | N1 |
| M35 | W1 | W1 |
| M36 | G1 | G1 |
| M37 | N2 | N2 |
| M38 | D1 | D1, E1, E2, N2, S1, S2 |
| M39 | N2 | N2 |
| M40 | A1 | A1 |

M10, M11, M14 and M15 make a one-sided bound, so their calls fail in B1 by the panic of finding 1, not by the equality. Two
further mutations, not registered, make both sides finite where the library does not panic: X42 (the two strings read as
-1 and 1) fails B1 by the equality; X41 (both defaults -1 and 1) fails A1, A2, C1 and N1, and B1 by the panic of a mixed
pair.

The committed mutation is M1, `crates/rcfd-mcp/tests/design_uq_sobol_args_f339.mutation.patch`; `git apply --check` accepts
it on the branch, and the same edit in the copy fails six tests.

### Other runs, deviations, what was not run

- `cargo fmt -p rcfd-mcp -- --check` clean; `cargo clippy -p rcfd-mcp --all-targets -- -D warnings` clean. `rcfd-api` was
  not touched. `docs_site`: 2 passed before the registration's commit; the run with this addendum is in the agent's report.
- Deviations: (1) of E1's 26 wrong fields, `cl_max`, `cd_normal`, `nu`, `n_scan` and `radius` change no number of these
  samples; finding 4 named only `cd_normal` and, for the circular arc, `camber_pos`. They differ in the ledger line, which
  the equality compares and which the test was written to look at before the run; so equality can fail for each, but for
  these five only through the `geometry_hash`. (2) The first run of X1 is void: it shared the target folder with the
  mutation copy, cargo did not rebuild `rcfd-api` for the branch (the log compiles `rcfd-mcp` and not `rcfd-api`) and linked
  the library of M40, the last mutation; `a1_an_absolute_target` failed, as under M40, and every other test passed. X1 was
  then run from an empty target folder of its own, as given in the table. The registered run and K1 were made before the
  copy existed. (3) Two unregistered mutations and one stdio call, above; they decide nothing. (4) G1's accepted call has
  no equality with the library, only its counts, as registered.
- The gate runs the new file's 16 tests (about 10 s); K1 is ignored. No heavy record. Not run: the Studio's tests, the
  whole workspace, the integration tests of any crate but `rcfd-mcp`, `baseline_hashes` (no solver file touched).

Size (agent), actual: done; 0; 0; 0; 240000

### Decision

**done** by the registered rule: every row but the record K1 holds, M1 to M40 are each caught by the registered row, and
the existing tests of `rcfd-mcp` and `rcfd-api --lib` pass unedited. K1 is a known miss of the library, outside `tools.rs`
and outside the rule as registered before the run; the lead decides whether F-339 reads done beside it and beside X1 of
ADR 0784. The sentence asked for holds: for each named wrong reading a recorded mutation makes it, a registered test fails
with the mutation and passes without it.

## Open questions for the maintainer

None. Finding 1 is a defect with one reading of the documentation; its repair is proposed as a follow-up. Finding 7 (a key
of another distribution is ignored in `dist`) is left to the lead.

## For the lead: roadmap entries

Status proposed: done.

> ADR 0791, 2026-11-25, done: every argument of design_uq and design_sobol is sent at a value that tells a wrong reading from the right one (the string, null and absent bounds of a normal distribution in every pair, a swapped Beta a and b, mean, system_l, the seed and its default, circular_arc, cmp <=, se_abs, and the further arguments found by reading): 16 tests hold against the library bit for bit, with the library's differing result for the wrong readings, 40 registered mutations of tools.rs are each caught by the row registered for it, one is committed, and the existing tests of rcfd-mcp and rcfd-api --lib pass unedited; tools.rs was right in every row; found beside it and recorded as a known miss outside the decision rule (K1): the library panics, and the server process ends, on a normal distribution open on one side only.

Follow-up proposed (key `one-open-side`, deps F-339):

> A normal distribution open on one side in the tolerance and Sobol drivers. Acceptance: erfc(+inf) is 0 and erfc(-inf) is 2 exactly (neither is a number now), and uq::run, sensitivity::run, design_uq and design_sobol sample a normal distribution with one finite and one open bound: every sample lies within the finite bound and equals the closed form of the truncated inverse CDF within a limit registered before the run; k1_one_open_side_known_miss of crates/rcfd-mcp/tests/design_uq_sobol_args_f339.rs passes with its limit unchanged; every result with two finite or two open bounds is bit-identical. Limits not stated here are registered before the run.

The same entries as `docs/roadmap/proposals/0791-F-339.json`.
