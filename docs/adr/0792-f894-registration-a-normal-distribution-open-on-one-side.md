# ADR 0792: F-894 registration: a normal distribution open on one side (erfc at infinity, the kept mass, and a one-sided inverse CDF from the smaller tail)

Status: **REGISTERED 2026-11-25, written before any code of F-894 and before any run of its checks.** Before this text no
library or test code was written and no function of the library was run. What was done: reading (below), searches of the
source tree, one calculation in 120-digit decimal arithmetic (the two bounds of row C1 and the sizes under "Where the
plain form loses accuracy"; `pre.py` in the scratch folder), and the run of the link check `docs_site` the brief asks for,
on this text. Results go in addenda at the end; nothing above them is changed afterwards.

It answers the brief `docs/adr/briefs/f894-normal-one-open-side.md` ([the file](briefs/f894-normal-one-open-side.md)) and
follows [docs/process/agent-rules.md](../process/agent-rules.md), version 5. Branch `f894-normal-one-open-side` from main
1889caad, scratch `w49i-work/f894-normal-one-open-side-scratch`. Model: Claude Opus 5.5.

## What is asked

The acceptance of F-894 in `docs/roadmap.json`: "erfc(+inf) is 0 and erfc(-inf) is 2 exactly (neither is a number now),
and uq::run, sensitivity::run, design_uq and design_sobol sample a normal distribution with one finite and one open bound:
every sample lies within the finite bound and equals the closed form of the truncated inverse CDF within a limit
registered before the run; k1_one_open_side_known_miss of crates/rcfd-mcp/tests/design_uq_sobol_args_f339.rs passes with
its limit unchanged; every result with two finite or two open bounds is bit-identical. Limits not stated here are
registered before the run."

The sentence this work must be able to write: every one-sided normal samples within its finite bound and agrees with the
closed form of the truncated inverse CDF within the registered limit, and every other result is bit-identical to main.

## What was read

`docs/process/agent-rules.md` (version 5, whole); the brief (whole);
[ADR 0791](0791-f339-registration-arguments-of-design-uq-and-design-sobol.md) (whole); the item F-894 (by a script);
`uq.rs` to the end of `run`, `sensitivity.rs` (`validate`, `run`), `bayes.rs` (every call of `norm_cdf`),
`fan_rom_eval.rs` (the truncated normal), `tools.rs` (`study_params`, `study_call`), `uq_tests.rs` (U7) and `surrogate.rs`
(`predict`) of `crates/rcfd-api/src/design`; `mixfrac.rs` to the beta PDF and the use of `erfc_inv` in `flamelet.rs` of
`crates/rcfd-reactor`, and its test `b3_special_functions`; the error functions and the truncated normal of
`crates/rcfd-mesh/src/fan_design.rs`; `crates/rcfd-core/src/rng.rs`; `sampling::latin_hypercube`;
`design_uq_sobol_args_f339.rs` (set-up, B1, G1, K1) and its mutation script; `crates/rcfd-mcp/tests/common/mod.rs`; the
section on the two tools of [docs/mcp-tools.md](../mcp-tools.md) and of [docs/design-loop.md](../design-loop.md); the
[tolerance table](../process/tolerances.md). The sentences of the documentation on the open side are right and are not
changed.

## Every caller of erfc, and what it can pass

| Caller | What it can pass | At an infinity |
|---|---|---|
| `mixfrac::erf` (`x >= 2`) | any double | `erf(+inf)` is `1 - erfc(+inf)`: not a number now, 1 after; `erfc(-inf) = 1 + erf(+inf)`: 2 after |
| `mixfrac::erfc_inv` | points of its bisection in `[-6, 27]` and Newton points near them, for `0 < y < 2` (asserted) | never; a `y` that is not a number asserts |
| `flamelet::chi_profile` (through `erfc_inv`) | `y = 2 Z` with `0 < Z < 1` | never |
| `uq::norm_cdf` from `Dist::validate` and `Dist::inv_cdf` | the standardised bounds: `lo`, `hi` not NaN, `mean` finite, `0 < sd` finite, so the quotient is never NaN; infinite for an open side, or by overflow | the defect of this item |
| `uq::norm_cdf` from `bayes::expected_improvement` | `(f_min - m) / s`, `s > 0`; the incumbent and the mean are finite; `s = y_sd sqrt(var)` with `var` a rounded difference, zero or at least about 1e-16 of its scale, and `y_sd` at least a rounding unit of the data's size | not reachable (the quotient is below about 1e25) |
| `uq::norm_cdf` from `bayes::prob_feasible` | `-m / s`, as above | not reachable |
| `uq::norm_cdf` in `fan_rom_bands_f268.rs` and `uq_tests.rs`; `erfc` in `combustion.rs` | finite constants | never |

`fan_design.rs` of `rcfd-mesh` has its own `normal_cdf` and calls none of these; its truncated normal refuses infinite
bounds, so `fan_rom_tolerances` hands `Dist` finite ones. The other `erf`/`erfc` in the tree are private to test files.

## Design

**D1, `erfc`.** In the branch `x >= 2`, `x == +inf` returns `0.0` before the continued fraction: the limit of
`exp(-x^2)`. `erf(+inf) = 1 - 0` and `erfc(-inf) = 1 - erf(-inf) = 1 + erf(+inf) = 2` follow from the existing code,
exactly. No finite argument takes the new line.

**D2, the kept mass.** With `a = (lo - mean) / sd`, `b = (hi - mean) / sd`, `Phi` the standard normal CDF and
`Q(z) = Phi(-z)`: an open lower side keeps `Phi(b)`, an open upper side `Q(a)`, each one evaluation of `norm_cdf` with the
relative accuracy of `erfc`; two finite bounds keep `Phi(b) - Phi(a)` as now. The test becomes "the mass is at least
1e-12" instead of "not below", in one private function, so a mass that is not a number is refused with the same message
("truncation [lo, hi] keeps a probability of ..."). After D1 the mass is always a number (the table above), so that
refusal is a guard; it is reachable only by a unit test and by a mutation.

**D3, one open side.** For `u` clamped into the open unit interval as now (`2^-1022 <= u <= 1 - 2^-53`), the truncated
inverse CDF is the `z` with `P = Phi(z)`, where

- lower side open: `P = u Phi(b)`, and its complement `1 - P = (1 - u) + u Q(b)` (from `Phi(b) = 1 - Q(b)`);
- upper side open: `P = Phi(a) + u Q(a)`, and `1 - P = (1 - u) Q(a)` (from `Phi(a) = 1 - Q(a)`).

All four are sums and products of non-negative terms, so each has the relative accuracy of its factors (`1 - u` is exact
for `u >= 1/2` and correctly rounded below). The smaller of the two decides: `z = norm_inv(P)` if `P <= 1/2`, else
`z = -norm_inv(1 - P)`, by `Phi^-1(P) = -Phi^-1(1 - P)`; the argument of `norm_inv` is clamped as now, the sample is
`mean + sd z` clamped to the bounds. Which side is open is decided by `lo` and `hi` as given. Two open bounds keep their
branch and two finite bounds theirs, unchanged, so their results are bit-identical.

**Where the plain form loses accuracy.** `Phi(a) + u (Phi(b) - Phi(a))` in doubles near 1 is a multiple of `2^-53`, so `z`
is uncertain by `2^-53 / phi(z)`: 7.5e-11 at `z = 5`, 1.2e-5 at 7, 2.2e-2 at 8, and no sample lies above about 8.2,
whatever the bound. It happens with the upper side open and a bound far out in the upper tail, and with the lower side
open, a bound far out in the upper tail and `u` near 1. It is the loss the doc comment of `inv_cdf` names; D3 removes it
for one open side and leaves two finite bounds as they are.

**The limit of the error in z** (`v = 2^-53`, first order, a bound and not a measurement). (i) `erf` for `0 <= x < 2`:
about 25 effective additions, terms with at most `3 n v` (weighted mean `n` about 5), `exp(-x^2)` with `x^2` off by `2 v`
and the exponential by `2 v`, constants and products `5 v`: below `50 v = 5.6e-15`. (ii) `erfc` for `x < 2` is `1 - erf`,
off by 5.6e-15 absolutely, so `erfc_inv`, which solves the computed `erfc` (bisection, then Newton), is off by
`5.6e-15 (sqrt(pi) / 2) exp(x^2)`, at most 2.7e-13 at `x = 2`, that is 3.8e-13 in `z`. (iii) For `x >= 2`: `exp(-x^2)` off
by `x^2 v`, the Lentz product by at most about `7 v` a step and 50 to 100 steps until a factor is 1: below 8e-14
relatively, and `erfc / |erfc'|` is below `1 / (2 x)`. (iv) The bound's own tail: its relative error (at most
`5.6e-15 / erfc(2) = 1.2e-12`, or 8e-14 beyond 2, plus the rounding of the argument, `4 v (2 t^2)`, at most 2.2e-14) times
the Mills ratio `Q / phi` at the sample, at most 0.35 where (ii) is largest and at most 1.2533 at the median, where the
factor is 5.6e-15. The sum of the largest terms is 8e-13 in `z`. If no factor of the continued fraction ever were exactly
1, its 1999 steps could add up to 1.5e-12 relatively; with a rounding unit to hit, this is not expected. The expected size,
with random signs, is a tenth: 1e-13. Registered: **|x - x_ref| <= 1e-12 sd + 4 v (|x_ref| + |mean|)**, the second part
for the sum `mean + sd z`, its product and the rounding of the reference. 1e-12 is the limit the repository already holds
`norm_inv` to (ADR 0121, U7) and the round trip of `erfc` (`b3_special_functions`, ADR 0053).

## The reference, independent of the code under test

`f894_reference.py` (Python, the `decimal` module, committed beside the test; the test reads its output
`f894_reference.tsv`). `Phi` from the alternating Maclaurin series of `erf`,
`erf(x) = (2 / sqrt(pi)) sum (-1)^n x^(2n+1) / (n! (2n+1))`, with 760 digits (the terms reach about `exp(x^2)`, the result
about `exp(-x^2)`, 1e306 and 1e-308 at the largest `x^2 = 705`), `pi` from Machin's formula; the code under test uses
this series nowhere. `z` from `Phi(z) = P` with `P = Phi(a) + u (Phi(b) - Phi(a))` in those digits (no loss there), by
Newton's method in the same arithmetic until the step is below 1e-40; each reference is checked in the script by the
residuals `|Phi(z) / P - 1|` and `|Q(z) / (1 - P) - 1|` below 1e-30, and `Phi` at ten arguments against the series of
positive terms weighted by `exp(-x^2)` to 1e-30. Inputs are the exact values of the doubles `mean`, `sd`, the bound and
`u`; `u` is clamped as in the code, and so is `P` from below (`2^-1022`), which is what "the end of the interval" means for
an open lower side (the closed form is infinite there). The draws of the drivers are made in the script too, by its own
transcription of the generator (xoshiro256** seeded by SplitMix64, as `rng.rs` states it) and of the Latin hypercube.

Grid: for each side, 12 bounds in standard deviations from the mean. Upper side open, `lo` at -30, -8, -3, -1, -0.25, 0,
0.25, 1, 3, 5, 7, 7.034483685; lower side open, `hi` at the negatives. Parents (mean, sd): (0, 1), (0.1, 0.3),
(1013.25, 0.037); the bound is the double `mean + sd k`. The bound 7.034483685 is used with the first parent only, where
it is exact (the rounding of the others moves the mass by more than the margin). `u`: 0, 2^-54, 1e-9, 1e-3, 0.1, 0.25, 0.5,
0.75, 0.9, 0.999, 1 - 1e-9, 1 - 2^-53, 1. In all 2 x 13 x (12 + 11 + 11) = 884 points.

## Checks

Files: `crates/rcfd-api/tests/f894_normal_one_open_side.rs` (A, B, C1, C2, D1), `crates/rcfd-api/tests/f894_bit_identity.rs`
(F), `crates/rcfd-mcp/tests/f894_one_open_side_tools.rs` (C4, D2, E1, K1g), one unit test in `uq.rs` (C3).

| Id | Clause | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|---|
| B1 | erfc at the infinities | `erfc(+inf)` is +0 and `erfc(-inf)` is 2, `erf(+-inf)` is +-1, `norm_cdf(+-inf)` is 1 and 0, by their bits; `erfc(NaN)` is not a number | exact | acceptance of F-894 | equal | exact | code verification |
| A1 | closed form | `Dist::inv_cdf` on the grid against the reference table | `1e-12 sd + 4 v (abs(x_ref) + abs(mean))` | acceptance of F-894 (a limit registered before the run); [tolerances](../process/tolerances.md), closed form (tighter); ADR 0121 U7 | 1e-13 sd (bound 8e-13) | 10 | code verification |
| A2 | within the bound | the grid and 200 draws (seed 792002) of each of its distributions: finite, `lo <= x <= hi` | exact | acceptance of F-894 | inside | exact | code verification |
| A3 | control of A1 | the plain form (written in the test from `norm_cdf` and `norm_inv`) misses the limit: for parent (0, 1), `lo` 7 and 7.034483685, and `hi` 8 and 30, the largest error over the `u` of the grid, each | at least 1e-9 | acceptance of F-894; row A1 | 1e-5 and more; 2e-2 | 1e4 | code verification |
| D1 | the two drivers | samples of `uq::run` (Monte Carlo 2 x 5, Latin hypercube 2 x 5, seed 792001) and `sensitivity::run` (n 3, seed 792001) in their ledgers, on the analytic model `y = x`, for (0.1, 0.3) with `hi` 0.25 alone and `lo` -0.2 alone and (0, 1) with `lo` 7 alone and `hi` -7 alone: equal to the table, which draws its own `u`; inside the bound | as A1 | acceptance of F-894 | 1e-13 sd | 10 | code verification |
| D2 | the two tools | `design_uq` and `design_sobol` on the fan of ADR 0791 (Mo, d1) with `lo` -0.2 alone and `hi` 0.25 alone, the open side absent, `null` and as its string: `result` as text and ledger lines but for the stamp equal to `uq::run` / `sensitivity::run` with the infinite bound, for all three forms; inside the bound; control: the two-open study differs | equal, 12 calls | acceptance of F-894; tolerances, bit-identity | equal | exact | code verification |
| K1 | K1 of ADR 0791 | `k1_one_open_side_known_miss` unedited, run with `--ignored` | passes | acceptance of F-894 | passes | exact | code verification |
| K1g | K1, gated | its limits in the new file: `design_uq` with 11 entries, the ten samples within the bound, both sides; the same for `design_sobol` (10 entries, 9 samples) | as K1 | acceptance of F-894 | pass | exact | code verification |
| E1 | the server lives | one `rcfd-mcp` process on stdio: `design_uq` with `hi` 0.25 alone, `ping`, `design_sobol` with `lo` -0.2 alone, `ping` | the two results are no errors, both pings answered, the process still runs | acceptance of F-894; ADR 0791 addendum 1 (the process ended with exit code 101) | answered | exact | code verification |
| C1 | refusal, lib | upper side open: `lo` 7.034483685 accepted (mass 1.000002e-12 by the calculation above), 7.034483965 refused (0.999998e-12), 50 refused; the negatives for the lower side open; by `Dist::validate`, message "keeps a probability" | as stated | acceptance of F-894 (limits registered); the existing limit 1e-12 of `Dist::validate` | as stated | exact | code verification |
| C2 | refusal, drivers | `uq::run` and `sensitivity::run` with the refused and the NaN bounds: `parameter 'x': truncation`, no ledger file | as stated | acceptance of F-894; ADR 0784 | as stated | exact | code verification |
| C3 | refusal, NaN mass | unit test of the private function: NaN, 0 and the double below 1e-12 refused ("keeps a probability of NaN" for the first), 1e-12 accepted | as stated | acceptance of F-894 (limits registered); the brief, clause (c) | as stated | exact | code verification |
| C4 | refusal, tools | both tools with `lo` 2.212 alone and `hi` -2.012 alone (7.04 standard deviations, mass 9.6e-13): -32602, `parameter 'stagger_common_deg': truncation`, nothing written | as stated | acceptance of F-894; ADR 0784, table of errors | as stated | exact | code verification |
| F1 | finite arguments | `erfc` and `erf` at `k / 64`, `abs(k) <= 1920`, and at both signs of 0, 5e-324, 2^-1022, 1e-300, 1e-8, the neighbours of 2, 1e10, 1e100, 1e200, the largest double; `erfc_inv` at `j / 1024` (0 < j < 2048), `2^-k` (k to 1021) and `2 - 2^-k` (k to 52); `norm_cdf` at `k / 32`, `abs(k) <= 1280`; `norm_inv` at the halves of the `erfc_inv` set: the hash of the bits of each set | equal to main | acceptance of F-894 (bit-identical); tolerances, bit-identity | equal | exact | unchanged default output |
| F2 | two finite, two open, uniform, beta | `inv_cdf` at the 13 `u` and 400 draws (seed 792003): normals on [-1, 2], [5, 8], [-8, -5], [6.9, 7.5], [-1e300, 1e300] of (0, 1), [-0.6, 0.9] of (0.2, 0.4), [-0.2, 8] and [-8, 0.25] of (0.1, 0.3), [1013.2, 1013.3] of (1013.25, 0.037), three without bounds, two uniform, three beta; the texts of eleven refusals | equal to main | acceptance of F-894 (bit-identical); tolerances, bit-identity | equal | exact | unchanged default output |
| F3 | both drivers | `result_json` and the ledger lines (clock 1) of `uq::run` (Monte Carlo and Latin hypercube) and `sensitivity::run` with a two-sided and a two-open normal, a uniform and a beta input, on an analytic model and on the fan of ADR 0791 (d3, and d1 without bounds) | equal to main | acceptance of F-894 (bit-identical); tolerances, bit-identity | equal | exact | unchanged default output |
| F4 | the other callers | `flamelet::chi_profile` at `Z = k / 128`, `bayes::expected_improvement` and `prob_feasible` on a grid of finite values: the hash of the bits | equal to main | tolerances, bit-identity; rule 3 | equal | exact | unchanged default output |
| X1 | rule 3 | unedited: `design_uq_sobol_args_f339`, `design_uq_sobol_f339`, `cargo test -p rcfd-reactor`, `rcfd-api --lib`, `fan_rom_bands_f268`, `baseline_hashes` | pass | rule 3; rule 7 | pass | exact | unchanged default output |

The record of F is made on the unchanged library of main before the fix and committed with its test as the second commit;
the test is run on that commit (it passes there) and after the fix. The full values are written to the scratch folder both
times and compared file to file. The hashes are of this machine's mathematical library (`exp`), as the hash tables of the
repository are. No check is of kind validation: the acceptance claims no agreement with a measured case.

### Mutations (rule 16)

On a scratch copy of the final commit with a target folder of its own, each alone, the new test files (and
`rcfd-api --lib` for P2) run once, the file restored.

| Id | Change | Must fail |
|---|---|---|
| P1 | the line of D1 removed | B1; F2 (two open bounds are refused: their kept mass is not a number). D2 and D3 evaluate `norm_cdf` at the finite bound alone, so the one-sided rows do not see it |
| P2 | the test of D2 back to "mass below 1e-12" | C3 |
| P3 | `erfc` and the code of `Dist` as on main (the function of D2 left for C3, unused) | B1, A1, K1g; E1, the server ends |
| P4 | the plain form for one open side | A1 |
| P5 | lower side open, `1 - P = 1 - u` (the term `u Q(b)` dropped) | A1 |
| P6 | upper side open, `P = u Q(a)` (the term `Phi(a)` dropped) | A1 |
| P7 | always from `P` | A1 |
| P8 | always from `1 - P` | A1 |
| P9 | the mass of one open side as `Phi(b) - Phi(a)` | C1 (9007 `v` = 0.99998e-12 at 7.034483685) |
| P10 | two open sides through D3 | F2 |
| P11 | `erfc` 0 from `x = 27` on | F1 |
| P12 | two finite bounds through the complement where `P > 1/2` | F2 |
| P13 | lower side open, `P = u Q(b)` | A1 |
| P14 | P1 and P9 together (the mass of one open side is not a number) | B1, K1g, D2; E1 by the refusal: the server answers with an error and is not ended |
| M10, M11, M14, M15 | those of ADR 0791 (`"-inf"` read as -1; `"inf"` read as 1; an absent or `null` `lo` -1; an absent or `null` `hi` 1), the file `design_uq_sobol_args_f339` and the new `f894_one_open_side_tools` | B1 of ADR 0791 by its equality, and D2 |

The committed one is P1, `crates/rcfd-api/tests/f894_normal_one_open_side.mutation.patch`, whose first line is
`# expect-fail: cargo test --release -p rcfd-api --test f894_normal_one_open_side`. The removal of the final clamp to the
bounds is not a registered mutation: it can matter by a rounding unit only.

Size (agent): done; 0; 0; 0; 300000

## Decision rule

**done** if every registered row holds, K1 passes with its limit unchanged, M10, M11, M14 and M15 are each caught after the
fix, the committed mutation is caught, and the results of F are bit-identical; **partial** otherwise. Whether F-339 is
then done is the lead's decision at the merge. If the fix moves a golden or a default result, the work stops and reports.

## What is not covered

- The loss of accuracy of two finite bounds far out in the upper tail: those results are to be bit-identical.
- The value at the exact end of an open side: the clamp of `u` (and of `P`) is kept, as for two open bounds.
- `chi_profile` of a `Z` that is not a number asserts in `erfc_inv`; the error functions of other crates; finding 7 of
  ADR 0791 (a key of another distribution in `dist`).
- Two finite bounds whose standardised value overflows to infinity: they panic on main and are answered after D1; no row.

## Addendum 1: results (2026-11-25)

Commits: registration 12f4a3be; the record of F on main's library 08e23e1f; the repair and the rows 5357e5fc; the
committed mutation d03fd55d; a lint of the F test and the reference values 918ebee4. The registered run was made once, at
5357e5fc: `cargo test -p rcfd-api -p rcfd-mcp --release --test f894_bit_identity --test f894_normal_one_open_side --test
f894_one_open_side_tools --no-fail-fast -- --nocapture --test-threads 1`, 15 passed, 0 failed, 8 s in series.

| Id | Limit | Measured | Met |
|---|---|---|---|
| B1 | exact | `erfc(+inf)` +0, `erfc(-inf)` 2, `erf(+-inf)` +-1, `norm_cdf(+-inf)` 1 and 0, by their bits; `erfc(NaN)` not a number | yes |
| A1 | `1e-12 sd + 4 v (abs(x_ref) + abs(mean))` | 884 points, no miss, the worst 0.024 of its limit; for the parent (0, 1) the largest error is 2.4e-14 | yes |
| A2 | inside | 14 484 samples finite and within their finite bound | yes |
| A3 | at least 1e-9 | the plain form is off by up to 2.9 (`lo` 7), 3.0 (`lo` 7.034483685), 2.7e-3 (`hi` 8) and 0.13 (`hi` 30) standard deviations | yes |
| D1 | as A1 | 116 samples of both drivers, the worst 0.004 of its limit, all within the bound | yes |
| D2 | equal, 12 calls | 12 calls equal to the library's one-sided study, result and ledger; the two-open study differs | yes |
| K1 | passes | passes with `--ignored`, unedited | yes |
| K1g | as K1 | both tools, both sides | yes |
| E1 | answered | both results without error, both pings answered by the same process | yes |
| C1 | as stated | 7.034483685 accepted; 7.034483965 (mass 9.99998998e-13) and 50 refused; both sides | yes |
| C2 | as stated | six bounds, both drivers, refused by name, no file | yes |
| C3 | as stated | passes in `rcfd-api --lib` | yes |
| C4 | as stated | four calls, -32602 with the name, nothing written | yes |
| F1 | equal to main | 5 groups, 16 531 values, 0 differ | yes |
| F2 | equal to main | 17 distributions with 413 values each and the 11 refusals, 0 groups differ | yes |
| F3 | equal to main | 9 studies, 0 differ | yes |
| F4 | equal to main | 3 groups, 1795 values, 0 differ | yes |
| X1 | pass | `design_uq_sobol_args_f339` 16 passed, 1 ignored (K1, run above); `design_uq_sobol_f339` 11 passed; `rcfd-reactor` 46 passed, 15 ignored; `rcfd-api --lib` 160 passed, 4 ignored; `fan_rom_bands_f268` 4 passed, 1 ignored; `baseline_hashes` 3 passed, 1 ignored; no existing test edited | yes |

The bit-identity of F was also compared value by value: the eight dump files of main's library (made before the fix, twice,
equal) and of the repaired one are equal byte for byte (`bits_main` and `bits_fix` of the scratch folder). The script
`f894_reference.py` ran to its end: its residuals and the two series for `erf` agree within the registered 1e-30.

### Mutations

On a copy of d03fd55d with a target folder of its own, each alone, restored after; the unchanged copy passes all
(`mutate.py`, `mutations.tsv`, one log each, in the scratch folder).

| Id | Registered to fail | Failed |
|---|---|---|
| P1 | B1, F2 | B1, F2, F3 and the control study of D2 (two open bounds, refused: "a probability of NaN") |
| P2 | C3 | C3 |
| P3 | B1, A1, K1g, E1 | all but A3 and F: A1, A2, B1, C1, C2, D1; C4, D2, K1g, E1; the server ended (no reply, `erfc_inv needs 0 < y < 2`) |
| P4 | A1 | A1 (192 misses), D1 |
| P5 | A1 | A1 (42 misses), D1 |
| P6 | A1 | A1 (263 misses), D1 |
| P7 | A1 | A1 (192 misses), D1 |
| P8 | A1 | A1 (192 misses), D1 |
| P9 | C1 | C1, and A1 by its check that the distribution is accepted |
| P10 | F2 | F2, F3 |
| P11 | F1 | F1 |
| P12 | F2 | F2, F3 |
| P13 | A1 | A1 (321 misses), D1 |
| P14 | B1, K1g, D2, E1 by the refusal | those, A1, C1, D1, F2, F3; E1: -32602 "keeps a probability of NaN", the ping answered |
| M10 | B1 of ADR 0791 by its equality, and D2 | B1 ("the result differs from the library's"), B3 of that file; D2 |
| M11 | the same | B1 by the same, B3; D2 |
| M14 | the same | B1 by the same, A1, A2, C1 and N1 of that file; C4, D2 |
| M15 | the same | B1 by the same, A1, A2, C1 and N1 of that file; C4, D2 |

M10, M11, M14 and M15 are caught by the equality of the gated B1, no longer by a panic (no log holds one of `erfc_inv`); no
further test was needed. The committed mutation is P1: `git apply --check` accepts the patch on the branch and the same
edit fails B1 in the file its first line names.

### Other runs, deviations, what was not run

- `cargo fmt -p <crate> -- --check` clean for `rcfd-api`, `rcfd-mcp`, `rcfd-reactor`; `cargo clippy -p rcfd-api -p rcfd-mcp
  -p rcfd-reactor --all-targets -- -D warnings` clean. `docs_site` 2 passed before the registration's commit; the run with
  this addendum is in the report. `baseline_hashes` passes, no golden and no existing test is changed.
- Deviations: (1) clippy found a needless `clone` in the set-up of F3; removed after the registered run (918ebee4), and the
  file run again (4 passed). The mutations were run on the commit before it. (2) Row C1 prints the refused mass as
  0.999998e-12; it is 0.999999e-12 (measured 9.99998998e-13). The bound and the limit are as registered. (3) The first form of
  the registration's text was never committed: P1, P3 and P14 were set right while it was being written, before any code.
  (4) Row D2 is run with seed 792004, which the registration did not fix.
- A reference fragment `docs/reference-data.d/0792-F-894.part` was added (rule 4), so the generated
  `docs/reference-data.md` is not current, as `docs/adr/index.md`: left to the lead (rule 9).
- Worth reading: the plain form misses by up to three standard deviations at a bound of 7 and not by 1e-5, because at
  `u = 1 - 2^-53` it saturates. The same loss remains for two finite bounds far out in the upper tail, by the registration.
- The gate runs 15 tests and one unit test (about 10 s). No heavy record. Not run: the whole workspace, the integration
  tests of `rcfd-api` and `rcfd-mcp` but those named, the Studio's tests.

Size (agent), actual: done; 0; 0; 0; 330000

### Decision

**done** by the registered rule: every row holds, K1 passes with its limit unchanged, M10, M11, M14 and M15 are each caught
after the fix, the committed mutation is caught, and the results of F are bit-identical. The sentence asked for holds:
every one-sided normal samples within its finite bound and agrees with the closed form of the truncated inverse CDF within
the registered limit, and every other result is bit-identical to main.

## Open questions for the maintainer

None.

## For the lead: roadmap entries

Status proposed: done. No follow-up.

> ADR 0792, 2026-11-25, done: erfc(+inf) is 0 and erfc(-inf) is 2 exactly, and a normal distribution open on one side is sampled from the smaller of its two tails: 884 points in the registered grid and 116 samples of uq::run and sensitivity::run agree with the closed form (760-digit reference) within 0.024 of the registered limit of 1e-12 sd, every sample lies within the finite bound, design_uq and design_sobol give the library's study bit for bit for every form of the open side, the server on stdio answers such a call and the ping after it, a kept mass below 1e-12 or not a number is refused by name, K1 of ADR 0791 passes with its limit unchanged, and every result with two finite or two open bounds, uniform and beta, of both drivers and of the other callers of erfc is bit-identical to main; M10, M11, M14 and M15 of ADR 0791 are caught again, by the equality.

The same entry as `docs/roadmap/proposals/0792-F-894.json`.
