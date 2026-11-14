# ADR 0524: F-590 registration: Benasciutti-Tovo broadband damage rate as a second method

Status: **REGISTERED 2026-11-14, written before any code of F-590 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-590 was written, compiled or run in the repository. The arithmetic under "Arithmetic
before this registration" was done by plain Python with numpy (not the library) in the scratch folder `w49i-work/f590-scratch`
(`probe.py`). Results go in the addendum at the end; nothing above the addendum is changed after a run.

Roadmap item F-590 (deps F-566), structural solver, advanced tier, follow-up of [ADR 0497](0497-f566-registration-broadband-fatigue-dirlik-from-spectral-moments.md)
(open question Q1). Number 0524 given by the lead. Branch `f590-work` from 615ad811, worktree `w49i-work/f590`, scratch
`w49i-work/f590-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact and the measured number recorded); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code; clean-room. The
brief overrides rule 9 for this item: the F-590 entries of `docs/ROADMAP.md` and `docs/roadmap.json` are updated on the branch in a
separate last commit (the svgs are not regenerated).

## What F-590 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. The rate (formula registered from a cited source) reduces to the narrow-band rate of F-413 within 1 percent for a one-mode response at zeta = 0.01 and agrees with the Dirlik rate of F-566 within a limit registered from the one-mode and two-mode cases of ADR 0497; the counted damage of the synthesised history of ADR 0497 lies within 10 percent of it; a non-positive m0 or m2 is refused.

Clauses: a1 formula registered from a cited source; a2 one-mode `zeta = 0.01`: within 1 percent of the narrow-band rate; a3
agreement with Dirlik within a limit registered from the one-mode and two-mode cases; a4 the counted damage of the ADR 0497
synthesised history (seed 20261114, `N = 2^23`, `fs = 6000`, `rainflow_periodic`, `miner`) within 10 percent; a5 a non-positive
`m0` or `m2` refused.

Readings fixed here. "One-mode response" and "two-mode case" are the systems S (`f_n = 50`, `zeta = 0.01`, flat `G = 1e-2` on
`[0, 500]`, displacement) and T (`f_1 = 50`, `f_2 = 150`, `zeta = 0.02`, gains 1 and `3^1.5`, flat `G = 1` on `[0, 600]`) of ADR
0497, through `spectral_response` and the same grid. "Narrow-band rate of F-413" is `narrow_band_damage_rate(nu0, rms, m, c)`
with `nu0 = sqrt(m2/m0)`, `rms = sqrt(m0)` (`spectral_rate`).

## What was read (worktree at 615ad811, read only)

ADR 0497 (all of it, including its addendum: counted rate of T `9.527921724e-8`, `212925` cycles, Dirlik rate `8.970802210e-8`),
`crates/rcfd-fem/src/fatigue.rs` (`narrow_band_damage_rate`, `spectral_rate`, `broadband_damage_rate`, `dirlik_coefficients`,
`gamma`, `positive`, `invalid`), `crates/rcfd-fem/src/random.rs` (`Moments` has `m0, m1, m2, m4`; `Moments::broadband_damage_rate`
and `narrow_band_damage_rate` exist, so **`m1` is already provided and no moment code is added**), `crates/rcfd-fem/tests/sa_broadband.rs`
(the systems S and T, the synthesis, the `Checks` helper), `docs/reference-data.md` (the F-413 entry).

## The method

### Source and the registered formula

D. Benasciutti and R. Tovo, "Spectral methods for lifetime prediction under wide-band stationary random processes",
Int. J. Fatigue 27 (2005) 867-877 (cited from memory; the publication details and **the fitted expression for b are unverified**,
not read in this repository). The damage of a Gaussian stationary zero-mean stress is a combination of the narrow-band damage
`D_NB` (Rayleigh amplitudes) and a lower bound `D_RC` on the rainflow damage:

```
alpha1 = m1 / sqrt(m0 m2)         alpha2 = m2 / sqrt(m0 m4)
D_RC   = alpha2^(m-1) D_NB        (Madsen's bound as used by Benasciutti and Tovo, recalled)
D_BT   = [ b + (1 - b) alpha2^(m-1) ] D_NB
b      = (alpha1 - alpha2) [ 1.112 (1 + alpha1 alpha2 - (alpha1 + alpha2)) exp(2.11 alpha2) + (alpha1 - alpha2) ] / (alpha2 - 1)^2
```

`D_NB` is the F-413 rate with the zero up-crossing rate `nu0 = sqrt(m2/m0)`: `nu0 (sqrt(2 m0))^m Gamma(1 + m/2)/c`. This is the
registered reading (the rate of peaks `sqrt(m4/m2)` in `D_NB` is the alternative; see the probes, it disagrees with Dirlik by
37 percent on T). The fitted `b` is the one I recall as the 2005 expression; sanity limits derived below (not a verification):
`b = 1` when `alpha1 = 1` (`alpha2 < 1`), `b = 0` when `alpha1 = alpha2`, and the whole bracket reduces to `D_NB` when
`alpha2 = 1` (then `alpha2^(m-1) = 1`, `D_BT = D_NB` for any `b`). The simpler estimate `b = (alpha1 - alpha2)/(1 - alpha1)`
(Tovo-Benasciutti, a different paper, also recalled) is **not** used: it diverges when `alpha1 -> 1` (probe: `b = 9.1` on S).

Open question (Q1): the exact fitted `b` of the 2005 paper must be verified against the publication; if it differs from the
recalled form, F-590 reports the discrepancy and does not move. The probes below are consistent with the recalled form (the
rate agrees with Dirlik to `5.2e-4` on T) but that is corroboration only.

### Design (code)

In `crates/rcfd-fem/src/fatigue.rs` (a new section appended after the broadband section; no existing function changes):

- `pub struct BenasciuttiTovo { alpha1, alpha2, b }` and `pub fn benasciutti_tovo_coefficients(m0, m1, m2, m4) ->
  Result<BenasciuttiTovo>`.
- `pub fn benasciutti_tovo_damage_rate(m0, m1, m2, m4, m, c) -> Result<f64>` and `benasciutti_tovo_damage_rate_curve(.., &SnCurve)`
  (a table or an endurance limit refused). Key `"benasciutti-tovo"`.

In `crates/rcfd-fem/src/random.rs`, one added method `Moments::benasciutti_tovo_damage_rate(&self, m, c)` passing `m0, m1, m2,
m4` unchanged. No method selector enum is added: the method is the function called, so no existing call changes.

Refusals (key `"benasciutti-tovo"`, the message naming the quantity and its value): `m0` not finite or `<= 0`; `m2` not finite or
`<= 0`; `m4` not finite or `<= 0`; `m1` not finite or `< 0`; `alpha1 > 1 + 1e-12` (message contains `alpha1`); `alpha2 > 1 +
1e-12` (message contains `alpha2`); `b` not finite or outside `[-1e-12, 1 + 1e-12]` (message contains `b`); `m` or `c` not
finite and positive. When `|alpha2 - 1| < 1e-12` the rate is `D_NB` (the bracket is 1; `b` is not evaluated).

### Case file

No key. Open question Q2: consistent with the pending `[fatigue] method` key of F-605 (ADR 0497 Q2): the value
`"benasciutti-tovo"` joins `"narrow-band"` and `"dirlik"`. Recommended: add it there, not here.

## Arithmetic before this registration (disclosed)

`probe.py` (trapezoid quadrature of the displacement densities, 2e6 points, `m = 3` unless stated; Dirlik as ADR 0497):

1. S, `zeta = 0.01`: `alpha1 = 0.994259`, `alpha2 = 0.941868`, `b = 0.8542`; BT over narrow band `0.9835421337160092` (`m = 3`),
   `0.9689420971927436` (`m = 5`); BT over Dirlik `0.990078` (`m = 3`), `0.97576` (`m = 5`). **So the one-mode check of clause a2
   is expected to MISS (1.65 percent against 1 percent; 3.1 percent for `m = 5`)**: it is registered with the limit of the
   acceptance and the miss will be recorded as it is.
2. S at smaller `zeta`, `m = 3`: BT over narrow band `0.9978597` (`zeta = 0.001`), `0.9958514` (`0.002`), `0.9905279` (`0.005`).
   The deviation shrinks with `zeta` as a method exact for narrow-band spectra should. A supplementary check at `zeta = 0.001`
   is registered below.
3. T, `m = 3`: `alpha1 = 0.8782`, `alpha2 = 0.7300`, `b = 0.6484016641667036`; BT over Dirlik `0.9994764275532475`, BT rate
   `8.966106719691442e-8` (against counted `9.527921724e-8` of ADR 0497: counted over BT `1.0626`); `m = 5`: BT over Dirlik
   `0.92945`. BT over narrow band `0.83575` (`m = 3`).
4. Alternatives probed: peak-rate `D_NB` gives BT over Dirlik `1.369` on T and `1.051` on S; the simple `b` gives `1.318` on T
   and `1.930` on S. They are not used (the registered reading was fixed above from the recalled paper; the probes only
   corroborate the choice and are disclosed here).
5. `b` outside the unit interval: `(alpha1, alpha2) = (0.5, 0.9)` gives `b = 1.1454`, `(0.2, 0.6)` gives `b = -2.1551`.

No other number was computed. The synthesis seed (20261114) is that of ADR 0497.

## Checks (limits fixed now)

Common: `SnCurve::power(m, c)`, `m = 3`, `c = 1` unless stated. "Rel" is `|a/b - 1|`. Every aggregate is checked finite before it is
compared. S and T as above. Dirlik rate is `broadband_damage_rate`.

| Id | Clause | Quantity | Limit | Estimate |
|---|---|---|---|---|
| C1 | a2 | S: `\|BT/narrow-band - 1\|`, `m = 3` | `<= 1e-2` | `1.65e-2` (**expected miss**) |
| C1b | a2 | the same, `m = 5` | `<= 1e-2` | `3.1e-2` (**expected miss**) |
| C1x | a1 | S: the ratio against `0.9835421337160092` | rel `<= 1e-5` | `1e-7` |
| C1z | a2 | supplementary: S with `zeta = 0.001` (same band), `\|BT/narrow-band - 1\|` | `<= 1e-2` | `2.1e-3` |
| C2 | a3 | S, `m = 3`: `\|BT/Dirlik - 1\|` | `<= 5e-2` | `9.9e-3` |
| C2b | a3 | T, `m = 3`: `\|BT/Dirlik - 1\|` | `<= 5e-2` | `5.2e-4` |
| C2c | a3 | S, `m = 5` | `<= 1e-1` | `2.4e-2` |
| C2d | a3 | T, `m = 5` | `<= 1e-1` | `7.1e-2` |
| C2x | a1 | T: BT over Dirlik against `0.9994764275532475`; `b` against `0.6484016641667036` | rel `<= 1e-5` | `1e-7` |
| C3 | a4 | T: counted rate (the ADR 0497 synthesis) over BT rate, `\|.. - 1\|` | `<= 1e-1` | `6.3e-2` |
| C3b | a4 | closed cycles counted | `>= 1e5` | `212925` |
| C3c | a4 | counted rate against `9.527921724e-8` (reproduces ADR 0497) | rel `<= 1e-6` | `0` |
| C4 | a5 | `m0` in `{0, -1, NaN, inf}`, `m2` in `{0, -1, NaN}` refused, key `"benasciutti-tovo"`, message names it; `Moments::default()` refused | all | by construction |
| C4b | a5 | further refusals: `m4 <= 0`; `m1 < 0`; `alpha1 > 1` (`m1^2 > m0 m2`, message contains `alpha1`); `alpha2 > 1` (message contains `alpha2`); `b` out of range at `(1, 0.5, 1, 1/0.81)` and `(1, 0.2, 1, 1/0.36)` (message contains `b`); `m <= 0`; `c <= 0`; a table curve; a curve with endurance limit | all | by construction |
| C5 | scaling | amplitude `m_k -> k^2 m_k`: rate scales `k^m`; frequency `m_k -> s^k m_k`: rate scales `s` | rel `<= 1e-12` | `1e-15` |
| C6 | degenerate | `alpha2 = 1` (`m0 = 2, m1 = 4, m2 = 8, m4 = 32`): rate equals the narrow-band rate, finite | rel `<= 1e-12` | `0` |
| C6b | structure | `b = 1` limit: moments with `alpha1 = 1`, `alpha2 = 0.5` give `b` within `1e-12` of 1 and the rate equals the narrow-band rate; `alpha1 = alpha2 = 0.5` give `b = 0` and rate `0.5^(m-1)` times narrow band | rel `<= 1e-12` | `1e-16` |
| C7 | feed | `Moments::benasciutti_tovo_damage_rate` bit-equal to the free function on S and T moments | equal bits | exact |
| B1 | defaults | the whole existing `sa_broadband`, `sa_fatigue` and `sa_random` test files pass unchanged; the diff to the base is additions only (no existing line of fatigue.rs or random.rs changed); `baseline_hashes` passes | equal | by construction |

## Negative controls and planned mutations (on a scratch copy, one at a time)

Negative controls inside the tests: C2 on T separates BT from narrow band (narrow band over BT is `1.197`, so a method equal to
the narrow-band rate fails C2b); C1x and C2x pin the numbers, so a wrong `b` fails them; C4b with the two `b` values exercises both
sides of the range.

| Id | Mutation | Expected to be caught by |
|---|---|---|
| M1 | `alpha2^m` instead of `alpha2^(m-1)` | C1x, C2x, C3, C6b |
| M2 | `b` from `alpha1` only: `b = (alpha1 - alpha2)/(1 - alpha1)` | C1x, C2x, C2, C4b |
| M3 | `D_NB` with the peak rate `sqrt(m4/m2)` | C1x, C2x, C2b, C3 |
| M4 | `exp(2.11 alpha1)` for `exp(2.11 alpha2)` | C1x, C2x |
| M5 | the constant `1.112` replaced by `1.0` | C1x, C2x |
| M6 | `alpha1 = m1/sqrt(m0 m4)` | C1x, C2x, C4b |
| M7 | `b` clamped to `[0, 1]` instead of refused | C4b |
| M8 | `Moments::benasciutti_tovo_damage_rate` passes `m2` as `m1` | C7 |
| M9 | the `alpha2 = 1` branch removed | C6 |
| M10 | refusal of `m2` weakened to `m2 < 0` | C4 |
| M11 | the `alpha1 > 1` refusal removed | C4b (if the later `b` refusal masks it, recorded as an equivalent as in ADR 0497 M10) |

## Decision rule

- **done:** every check C1 to C7 and B1 met, every mutation caught, the existing tests of `rcfd-fem` and the baseline hashes
  passing, fmt and clippy clean.
- **partial:** a clause of the acceptance missed or not run, the miss recorded as an ignored `*_known_miss` test with its numbers,
  or a supplementary check missed. A miss of C1 or C1b (expected) makes the status **partial**.
- **todo:** the formula not implemented.

## Limits (not covered)

- Gaussian, stationary, zero-mean stress; no mean-stress correction in the rate; no non-Gaussian correction.
- BT is an interpolation between the narrow-band damage and a lower bound of the rainflow damage; it is not exact for any
  spectrum and for a one-mode response at `zeta = 0.01` and the band of S it is expected to differ from the narrow-band rate by
  1.65 percent (arithmetic 1).
- The fitted `b` is recalled from the 2005 paper and not verified; other `b` variants exist (Q1).
- Single power law without endurance limit; table curves refused (as F-413, F-566).
- No case-file key, no CLI, no API schema; no `RandomStress` method (a stress component feeds it through `RandomStress::moments`).

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

- Q2. Case-file key: not added; recommended to extend the pending `[fatigue] method` of F-605 with `"benasciutti-tovo"`.
- Q3. Separate function pair, no selector enum (an enum `BroadbandMethod` would be a convenience for F-605; recommended there).

## Open questions listed, not taken

- Q1. Verification of the fitted `b` against the publication (a paper the repository does not have).

## Addendum 1: results

Registered runs made once (`cargo test -p rcfd-fem --release --test sa_benasciutti_tovo -- --nocapture`), on the code commit 6d9325c4.
Two clippy and fmt forms were fixed before the run record (missing docs of three fields, two unused helper methods of the test
copy); no limit or number changed.

| Id | Quantity | Limit | Measured | Met |
|---|---|---|---|---|
| C1 | S, `zeta = 0.01`, `m = 3`: BT over narrow band, `\|.. - 1\|` | `<= 1e-2` | `1.6458e-2` (ratio `0.983542133`) | **NO** (ignored `c1_..._m3_known_miss`) |
| C1b | the same, `m = 5` | `<= 1e-2` | `3.1058e-2` (ratio `0.968942097`) | **NO** (ignored `c1b_..._m5_known_miss`) |
| C1x | ratio against `0.9835421337160092` | rel `<= 1e-5` | `2.5e-10` | yes |
| C1z | `zeta = 0.001`, `m = 3` | `<= 1e-2` | `2.14e-3` | yes |
| C2 | S, `m = 3`, BT over Dirlik | `<= 5e-2` | `9.92e-3` | yes |
| C2b | T, `m = 3` | `<= 5e-2` | `5.24e-4` | yes |
| C2c | S, `m = 5` | `<= 1e-1` | `2.42e-2` | yes |
| C2d | T, `m = 5` | `<= 1e-1` | `7.06e-2` | yes |
| C2x | T: `b` against `0.6484016641667036`; BT/Dirlik against `0.9994764275532475` | rel `<= 1e-5` | `4.6e-9`; `6.0e-11` | yes |
| C3 | T: counted rate over BT rate, `\|.. - 1\|` (seed 20261114) | `<= 1e-1` | `6.27e-2` (counted `9.527921724e-8`, BT `8.966105345e-8`) | yes |
| C3b | closed cycles | `>= 1e5` | `212925` | yes |
| C3c | counted rate against ADR 0497 | rel `<= 1e-6` | `3.2e-11` | yes |
| C4, C4b | all refusals (`m0`, `m2`, `Moments::default()`, `m4`, `m1`, `alpha1`, `alpha2`, both `b` out of range, `m`, `c`, table, endurance) with key and the quantity named | all | all (messages printed) | yes |
| C5 | amplitude `k^m`; frequency `s` | `<= 1e-12` | `3.3e-16`; `0` | yes |
| C6, C6b | `alpha2 = 1` gives narrow band, finite; `b = 1` and `b = 0` limits | `<= 1e-12` | `0` for all | yes |
| C7 | method bit-equal to the free function and the curve form on S and T | equal bits | equal | yes |
| B1 | existing tests unchanged; diff additions only; `baseline_hashes` | equal | see gates below | yes |

Deviations: none from the registered set-up. The probe estimates held: the expected miss of C1 and C1b came out at `1.6458e-2` and
`3.1058e-2` (estimates `1.65e-2`, `3.1e-2`).

Diagnosis of the miss (not repaired): BT combines `D_NB` and `alpha2^(m-1) D_NB` with the weight `b = 0.854` on S; with
`alpha2 = 0.942` the rainflow-bound term `0.942^2 = 0.887` (m = 3) pulls the rate `1.6` percent below `D_NB` although the spectrum
is one narrow peak (the flat band `[0, 500]` Hz adds a broadband tail to the response, `alpha2 < 1`). The Dirlik rate is also
below the narrow band by `0.66` percent for the same set-up, and BT is `0.99` of Dirlik. The 1 percent of the acceptance is met by
BT only for smaller damping (C1z, `zeta = 0.001`: `0.21` percent; the probe gave `0.5` percent at `zeta = 0.005`: crossing 1 percent
near `zeta = 0.0055`). So the clause "reduces to the narrow-band rate within 1 percent at `zeta = 0.01`" is not met by this formula
on this band. It is not tuned (no other `b`, no peak-rate `D_NB`, no other band was run).

Mutations (scratch copy `f590-scratch/mut`, own target dir, one at a time, whole `sa_benasciutti_tovo` file):

| Id | Mutation | Caught by |
|---|---|---|
| M1 | `alpha2^m` | C1x, C2x, C2b, C2d, C3, C6b |
| M2 | `b = (alpha1 - alpha2)/(1 - alpha1)` | the tests c1x_c1z_c2x, c2, c3, c5_c6_c7 |
| M3 | peak rate in `D_NB` | c1x_c1z_c2x, c2, c3, c5_c6_c7 |
| M4 | `exp(2.11 alpha1)` | c1x_c1z_c2x, c2 |
| M5 | `1.112 -> 1.0` | c1x_c1z_c2x, c2 |
| M6 | `alpha1 = m1/sqrt(m0 m4)` | c1x_c1z_c2x, c2, c3, c5_c6_c7 |
| M7 | `b` clamped | c4_refusals (the `b` ranges) |
| M8 | method passes `m2` as `m1` | c1x_c1z_c2x, c2, c3, c5_c6_c7 |
| M9 | `alpha2 = 1` branch removed | c5_c6_c7 (C6: the 0/0 gives a refusal or NaN) |
| M10 | `m2` refusal weakened to `m2 < 0` | **not caught**: `m2 = 0` gives `alpha1 = inf`, which the `alpha1` refusal rejects with a message that contains `m2` among the moments; equivalent at the message level, as M10 of ADR 0497. Recorded, not repaired. |
| M11 | `alpha1 > 1` refusal removed | c4_refusals (`alpha1 > 1` accepted, rate `3.76`) |

Gates: `cargo test -p rcfd-fem --release`: 866 passed, 0 failed, 240 ignored (199 test binaries ok); `cargo fmt -p rcfd-fem --check` clean; `cargo clippy -p rcfd-fem --all-targets -- -D warnings` clean; `cargo test -p rcfd-api --release --test baseline_hashes`: 3 passed, 1 ignored; `cargo test -p rcfd-cli --release --test docs_site`: 2 passed.

Decision by the registered rule: **partial**: C1 and C1b missed (recorded as ignored known-miss tests with the limits intact), every
other check met, M10 survived as a message-level equivalent, the other ten mutations caught.

Open questions (options and recommendation):

- Q1. The fitted `b` is recalled and unverified. Option: verify against the 2005 paper. Recommended: yes, before F-590 is called done;
  the agreement with Dirlik (`5.2e-4` on T) corroborates the recalled form but cannot separate it from a close variant.
- Q4. The acceptance clause "within 1 percent at `zeta = 0.01`" is not met by BT on the band of S (1.65 percent). Options: (a) change
  the acceptance to 2 percent at `zeta = 0.01` and 1 percent at `zeta <= 0.005`; (b) keep it and leave the item partial; (c) a
  different band for S (a parameter change to be registered before a new run). Recommended: (a), a registered amendment by the lead.
- Q2, Q3 as above (case-file key and selector with F-605).

## For the lead: roadmap entries

Proposed status of F-590: **partial**. Note:

> ADR 0524, 2026-11-14: rcfd_fem::fatigue gains the Benasciutti-Tovo broadband damage rate of a Gaussian stationary stress from m0, m1, m2, m4 (D. Benasciutti and R. Tovo, Int. J. Fatigue 27 (2005), cited from memory, the fitted weight b unverified): benasciutti_tovo_coefficients, BenasciuttiTovo, benasciutti_tovo_damage_rate and benasciutti_tovo_damage_rate_curve, and Moments::benasciutti_tovo_damage_rate; no existing item changed. PARTIAL by the standing rule: the one-mode reduction to the narrow-band rate at zeta = 0.01 is missed (1.646e-2 for m = 3, 3.106e-2 for m = 5 against 1e-2; ignored known-miss tests); it holds at zeta = 0.001 (2.1e-3). Met: BT over Dirlik 9.9e-3 (S) and 5.2e-4 (T) for m = 3 (limit 5e-2), 2.4e-2 and 7.1e-2 for m = 5 (limit 1e-1); counted damage of the ADR 0497 history (seed 20261114, 212925 cycles) within 6.27e-2 of the BT rate (limit 1e-1); refusals of non-positive m0, m2, m4, m1 < 0, alpha1 or alpha2 above 1, a weight b outside [0, 1], m, c and table or endurance curves; scaling and limits b = 0 and b = 1; ten of eleven mutations caught (M10 equivalent at message level). Limits: Gaussian zero-mean, power law without endurance limit, no case-file key (the value benasciutti-tovo to join the pending method key of F-605).

> Follow-up (no new item proposed): verify the fitted weight b of Benasciutti and Tovo 2005 against the publication; amend the F-590 acceptance to 2 percent at zeta = 0.01 (1 percent at zeta <= 0.005), then the item is done.
