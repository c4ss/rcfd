# ADR 0535: F-612 registration: the series-ratio indicator of the perturbation Schur Ritz error and its fallback to the dense computation

Status: **REGISTERED 2026-11-14, written before any code of F-612 and before any run of its checks**. Not yet accepted by the
maintainer. No probe program was run for this registration. The only numbers used are the records of ADR 0527 (its text and the
scratch logs `w49i-work/f584-scratch/probe1.log` and `run1.log` of the F-584 runs, which were read, not re-run): they are
disclosed under "Arithmetic before this registration". Results go in the addendum at the end; nothing above the addendum is
changed after a run.

Roadmap item F-612 (deps F-584), structural solver, advanced tier ([ADR 0310](0310-structural-solver-advanced-tier-plan.md)),
the follow-up proposed by [ADR 0527](0527-f584-registration-corrected-frequency-and-perturbation-schur-error.md) (remaining
question 1 b). Number 0535 given by the lead. Branch `f612-work` from 679081a1, worktree `w49i-work/f612`, scratch
`w49i-work/f612-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical and the results of ADR 0527 (and 0492, 0482, 0481) bit-identical; no edit of `baseline_hashes.rs`, of the hash
tables or of any golden file; no existing test edited; no new dependency, no unsafe code; clean-room. The brief overrides rule 9
for this item: the F-612 entries of `docs/ROADMAP.md` and `docs/roadmap.json` are updated on the branch in a separate last
commit (the svgs are not regenerated). Every choice below that the maintainer would normally be asked about is taken on the
maintainer's standing instruction of 2026-11-11 (take the recommendation); none changes a default result, a golden or a hash
table.

## What F-612 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. With SplitSchur::Perturbation each split mode reports the ratio delta_2/delta_1 of ADR 0527 and falls back to the dense computation of ADR 0492 where it exceeds a threshold registered from the records of ADR 0527; on the disc sector of ADR 0411 with 4 harmonic modes per index at 0.4 and 0.8 omega_ref the resulting schur_eta agrees with that of ADR 0492 within 1 percent where it exceeds 1e-9, the cost stays below half of the dense path, and the results of ADR 0527 keep their bits.

Clauses: **b1** each split mode reports the ratio `delta_2/delta_1` of ADR 0527 D1; **b2** it falls back to the dense
computation of ADR 0492 where the ratio exceeds a threshold registered from the records of ADR 0527; **b3** on DS of ADR 0411
with B4 at `0.4` and `0.8 omega_ref` the resulting `schur_eta` agrees with the dense one (ADR 0492) within 1 percent where it
exceeds 1e-9; **b4** the cost stays below half of the dense path; **b5** the results of ADR 0527 keep their bits. The brief adds:
a decision whether the fallback is a new variant, a default-bits record first, `baseline_hashes` unchanged, no case-file key,
mutations the checks catch, every aggregated test error finite.

Readings fixed here (each taken on the standing instruction):

- **R1, new variant.** `SplitSchur::Perturbation` stays exactly as F-584 built it (its `schur_eta`, `corrected_frequency` and
  every other field keep their bits); the fallback is a new variant `SplitSchur::PerturbationChecked`. The indicator is
  reported (new field) by both `Perturbation` and `PerturbationChecked`, so "with `SplitSchur::Perturbation` each split mode
  reports the ratio" holds literally; only the fallback needs the new variant. Recommended because it is the only reading under
  which b5 can be checked bit for bit.
- **R2, the ratio.** `r = abs(delta_2 / delta_1)` of ADR 0527 D1 for the split mode `a` (both numbers are already computed in
  `schur_perturbation`). The sign is dropped: `delta_2` is of either sign in principle, the threshold is on the size of the
  second-order term against the first. Where `delta_1 = 0` or `r` is not finite the ratio is undefined: it is reported `None`
  and the mode falls back (the safe side).
- **R3, "where it exceeds".** Fallback where `r > RATIO_LIMIT` (strictly), or `r` undefined, or the perturbation estimate
  itself refused (`None`, ADR 0527 D1 refusals): then the dense computation of ADR 0492 `schur_error` for that split mode
  (its value and its root, bit for bit those of `SplitSchur::Dense`).
- **R4, scope of b3.** As ADR 0527 G1: the 48 split modes of the 8 B4 runs matched to the reference modes (best complex MAC
  `>= 0.99`, distinct); "within 1 percent" `abs(checked/dense - 1) <= 0.01`; "where it exceeds 1e-9": the dense `schur_eta` (J1;
  J2 is the other reading, the checked value). The unmatched top half of the enlarged basis is recorded only (as ADR 0492,
  0527).
- **R5, cost.** The wall time of `split_modes` summed over the 8 B4 runs, `PerturbationChecked` over `Dense`, each pair
  measured back to back (as ADR 0527 G7), limit 0.5 (the acceptance's "below half").

## What was read (679081a1, read only)

ADR 0527 whole (D1 the derivation, D4 why the series ratio reaches 0.62, the records, the addendum), ADR 0492 D1/D2 (the dense
frozen solve, `schur_error`), the sources `crates/rcfd-fem/src/gyro.rs` (`sector_split_at`, `schur_error`,
`schur_perturbation`, `pencil_norm`, `HarmonicSplitModes`) and `rotating.rs` (`SplitSchur`, `RotatingOptions`), and the tests
`sa_gyro_sector_split_schur.rs` (set-ups, matching, the test-side dense reference of D1, `second_order`) and
`sa_gyro_sector_split_schur_default_bits.rs` (the digest pattern). No published source is used: the indicator is a number the
perturbation sum already computes; the threshold comes from the repository's own records.

## Design

### D1. The indicator and the threshold

`schur_perturbation` returns, beside `(schur_eta, theta')`, the ratio `r = abs(delta_2 / delta_1)` (`None` for an empty
complement and for the undefined cases of R2). The values `(schur_eta, theta')` are computed by the same operations in the same
order as in F-584 (no bit changes).

**Threshold `RATIO_LIMIT = 0.08`**, fixed from the records of ADR 0527 (arithmetic 1): the truncation miss of the second-order
sum is about `(delta_2/delta_1)^2` (ADR 0527 D4: 0.616^2 = 0.379 against 0.383; at small `r` the recorded misses are `r^2` within
12 percent: `r = 0.092` gives 8.6e-3, `r = 0.108` gives 1.17e-2). The 1 percent limit is therefore crossed at `r = sqrt(0.01) =
0.1`; the threshold is `0.8` of that, so `r^2 = 0.0064` is expected, and with the 12 percent excess 7.2e-3, under the limit. It
is a number fixed by this derivation, not searched for; it is not moved after the run.

### D2. API (registered)

`rotating.rs`: `SplitSchur` (non-exhaustive) gains the variant `PerturbationChecked` (documented: the perturbation estimate of
ADR 0527 with the indicator of ADR 0535, dense fallback where `r > 0.08` or `r` undefined or the estimate refused).
`gyro.rs`: `HarmonicSplitModes` (non-exhaustive) gains `perturbation_ratio: Vec<Option<f64>>` (the indicator `r`: `Some` for a
split mode whose estimate was computed by the perturbation path with a defined ratio and a non-empty complement, `None`
otherwise: always with `Dense`, without `schur_eta`, for an empty complement, for an undefined ratio) and `schur_fallback:
Vec<bool>` (true for a split mode whose `schur_eta` (and `theta'`) is the dense computation's: only with
`PerturbationChecked`), both parallel to `modes.modes`. `pub const SPLIT_SCHUR_RATIO_LIMIT: f64 = 0.08` in `gyro.rs`. No
case-file key, no CLI output, no change to `RotatingPart`. With `PerturbationChecked`, `corrected_frequency` follows
`schur_eta` per mode as ADR 0527 D2 (the dense root for a fallback mode).

### D3. Bits (b5)

With the defaults (`Dense`, `corrected_frequency` false) and with `Perturbation` the values of `schur_eta`,
`corrected_frequency` and every other existing field are those of 679081a1 bit for bit; the two new fields are `None`/`false`
for `Dense` and the default and `Some(r)`/`false` for `Perturbation`. `PerturbationChecked` computes the split modes, `eta` and
everything else first as before; only `schur_eta`, `corrected_frequency` (fallback modes) and the two new fields differ.
`damped.rs`, `cyclic.rs`, `mpc.rs` are not edited; in `rotating.rs` only the enum gains a variant.

### D4. Why the cost clause is expected to be missed (predicted before the run)

The dense per-mode cost is `n_b` complement solves; the perturbation per-mode cost is one. The F-584 wall times of the 8 runs
(`run1.log`): perturbation 0.108 s, dense 0.792 s (ratio 0.136), dominated by the doubled indices `k = 1, 2` (dense 0.18 to 0.20
s each, 16 split modes of 8 matched plus 8 unmatched, so about 0.011 s of dense work per mode) while `k = 0, 3` cost 0.008 to
0.013 s. The records of the ratio (`probe1.log`, `d2/d1`, matched modes) put `r > 0.08` for 28 of the 48 matched modes (at 0.4:
`k = 1` 4 of 8, `k = 2` 8 of 8, `k = 3` 2 of 4; at 0.8 the same), and the unmatched modes are the poor top half (ADR 0492's
records: `schur_eta` up to 1.3; their `r` was not recorded, probably above 0.08). So about 80 percent of the dense work of `k =
1, 2` is expected to fall back, giving `t_checked / t_dense` of about `0.136 + 0.8 * 0.864 = 0.83`, between 0.6 and 0.9. The
accuracy clause and the cost clause conflict on this basis: the modes that need the dense value are most of the work. The limit
0.5 is kept; **J6 is predicted missed** and recorded as a known miss with the measured number and the fraction of fallback
modes. A cheaper fallback (dense only where `schur_eta` is above a floor, a higher-order sum) is a follow-up, not part of this
item.

## Set-ups (fixed now)

As ADR 0527: steel DS (`N = 6`, Hex20 on `hex_box([0.25, 0, -0.1], [0.75, 2 pi/6, 0.2], [2, 1, 1])` mapped to the annulus,
`xmin` clamped, `Cyclic("ymin", "ymax", e_z, 0, 6)`), `ModalOptions::new(m_b).tolerance(1e-9)`, 400 iterations,
`RotatingOptions::new(m_b).modal(..).coriolis(true).split_correction(true).corrected_frequency(true)`; **B4** (`m_b = 4`),
**B41** (`m_b = 41`); `omega_ref` the lowest rest whirl frequency over `k = 0 .. 3` of B4 without the option (1655.3226
rad/s); speeds `0.4` and `0.8 omega_ref`; reference `RotatingSector::whirl_modes(k, Omega)`; **DSS** DS without stress
stiffening at `10 omega_ref`. The runs: **dense** (`Dense`), **perturbation** (`Perturbation`), **checked**
(`PerturbationChecked`), and the **default** (no `split_schur`, no `corrected_frequency`). Test-side reference of `delta_2 /
delta_1`: the test-side dense algebra of ADR 0527 (`dense_of`, `dense_e`, `second_order`), copied into the new test file.

## Arithmetic before this registration (disclosed)

1. The threshold of D1 from ADR 0527: the table of `probe1.log` (matched modes, `eta2/schur-1` against `d2/d1`): all modes with
   `d2/d1 <= 0.076` have a miss of at most 6.34e-3 (0.076: 6.34e-3, 0.075: 6.06e-3, 0.074: 5.88e-3, 0.070: 5.51e-3); the first
   mode above 1 percent has `d2/d1 = 0.108` (1.17e-2); the modes 0.083 and 0.092 (7.7e-3, 8.6e-3) are within 1 percent and
   fall back under the threshold 0.08, which is conservative. The ratio of ADR 0527 is the test-side one; the library value
   equals it to the 1e-7 of ADR 0527 G6 (checked in J3).
2. The count of D4 from the same table (28 of 48 matched above 0.08; 20 of the 34 modes whose dense `schur_eta` exceeds 1e-9),
   and the wall times from `run1.log`.
3. No code was written and no program run before this registration.

## Checks (limits fixed now)

Tests in a new file `crates/rcfd-fem/tests/sa_gyro_sector_split_schur_checked.rs` (all but J7) and
`sa_gyro_sector_split_schur_checked_default_bits.rs` (J7, digests recorded at 679081a1 before any library line), release
profile. Every aggregated value in a check is required finite (a non-finite value fails the check, never enters a `max`).

| Id | Clause | Check | Limit | Estimate |
|---|---|---|---|---|
| J1 | b3 | DS B4, `k = 0 .. 3`, `0.4` and `0.8 omega_ref`: matches as R4 (in the dense run); for every match whose dense `schur_eta` exceeds 1e-9: `abs(checked/dense - 1)` (the same split mode index); the count | `<= 0.01`; count `>= 30` | 6.3e-3 (D1, arithmetic 1; non-fallback modes) and 0 (fallback modes); 34 |
| J2 | b3 | the same where the checked `schur_eta` exceeds 1e-9 | `<= 0.01` | as J1 |
| J3 | b1 | `perturbation_ratio`: `None` everywhere in the default and the dense runs; in the perturbation and checked runs `Some` finite on every split mode of B4 (none refused in ADR 0527) with equal bits between the two runs; for every matched split mode `abs(lib - test-side abs(d2/d1)) <= 1e-4`; the count | `None`; `Some` finite, equal; `<= 1e-4`; 48 | about 1e-6 (ADR 0527 G6 1.1e-7 on `eta`) |
| J4 | b2 | in the checked run `schur_fallback[j] == (ratio[j].is_none() or ratio[j] > 0.08)` for every split mode of every B4 run; `false` everywhere with `Perturbation`, `Dense`, the default | each | D2 |
| J5 | b2 | the checked run's `schur_eta` and `corrected_frequency` per split mode: bit equal to the dense run's where `schur_fallback`, bit equal to the perturbation run's where not; `schur_eta` and `corrected_frequency` consistent bit for bit (ADR 0527 G5) in the checked run | bit equal | D2 |
| J6 | b4 | `split_modes` wall time summed over the 8 B4 runs, checked over dense (each pair measured back to back) | `<= 0.5` | **0.83: predicted miss** (D4; 0.6 to 0.9) |
| J7 | b5 | digests recorded at 679081a1 by `sa_gyro_sector_split_schur_checked_default_bits.rs` (written with the API of that commit, committed before any library line): the perturbation run (words of ADR 0527 G12 plus `schur_eta` plus `corrected_frequency`) of DS B4 `k = 0 .. 3` at 0.4 and 0.8, the dense run of the same, the perturbation run of DS B41, the perturbation run of DSS at `10 omega_ref`: all equal after the change; ADR 0527's tests (`sa_gyro_sector_split_schur`, `sa_gyro_sector_split_schur_default_bits`), the ADR 0481, 0482, 0492 default-bits tests and `sa_gyro_sector_schur` pass; `cargo test -p rcfd-api --release --test baseline_hashes` passes; `cargo test -p rcfd-fem --release` passes | `==`; pass | holds by construction (D3) |
| J8 | b5 | the checked run against the default: every field of the words of ADR 0527 G12 (all but `schur_eta`, `corrected_frequency`) equal bit for bit | `==` | D3 |
| J9 | b2 | DS B4 at `0.4 omega_ref`, `k = 0 .. 3`: without `corrected_frequency` the checked run's `corrected_frequency` is all `None` and its `schur_eta` still `Some`; with `PerturbationChecked` but without `split_correction`, and without `coriolis`: `schur_eta`, `corrected_frequency`, `perturbation_ratio` `None` and `schur_fallback` false | each | D2 |
| J10 | b2 | DS B41, both speeds, every index, checked run: `schur_eta == Some(0.0)`, `corrected_frequency == Some(theta)` bit for bit, `perturbation_ratio` `None`, no fallback | each | D1 |
| J11 | b2 | DSS at `10 omega_ref`, `k = 0 .. 3`, checked run: `schur_eta`, `corrected_frequency`, `perturbation_ratio` `None` and no fallback (all four indices unstable as in ADR 0527 G10; the "Some where eta is Some" half is vacuous) | each | ADR 0527 G10 |

Records (not in the decision rule): per matched mode `r`, the dense, perturbation and checked `schur_eta`, the count of
fallback split modes per run (matched and unmatched) and in all, the wall times per index of the three runs.

## Negative controls and mutations

| Id | Control | Expected |
|---|---|---|
| NJ1 | the unchecked perturbation run on the scope of J1: `max abs(perturbation/dense - 1)` | `> 0.1` (0.3835 in ADR 0527): the check J1 can fail |
| NJ2 | the fallback count over the J1 scope (dense `schur_eta > 1e-9`) | `>= 10` and `<= 30` of 34 (20 expected): the threshold neither never nor always fires |

Mutations (on a scratch copy of the worktree with its own target folder, each run once against the new test file; each must be
caught by a registered check other than the predicted miss J6):

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MJ1 | `RATIO_LIMIT` 0.08 raised to 0.2 | J1, J2 (modes with `r` 0.108 to 0.196 stay perturbation, misses 1.2e-2 to 0.13), J4 |
| MJ2 | the comparison inverted (fallback where `r <= limit`) | J1, J4 |
| MJ3 | the ratio reported as `delta_1/delta_2` | J3, J4 |
| MJ4 | the fallback mode's `theta'` taken from the perturbation sum (`schur_eta` dense) | J5 |
| MJ5 | the fallback never taken (limit infinite) | J1, J4, NJ2 |
| MJ6 | the fallback always taken | J4, J5, NJ2 |
| MJ7 | the mirror terms dropped from `delta_2` (the ratio and the estimate wrong) | J3 |
| MJ8 | the default `split_schur` set to `PerturbationChecked` | J7, J8 |
| MJ9 | `Perturbation` also falling back (the unchecked variant changed) | J7 |
| MJ10 | the ratio filled also on the `Dense` path | J3 |
| MJ11 | the undefined ratio not falling back | not caught by the DS runs (no undefined ratio occurs there); not run, listed under limits |

## Decision rule

**done**: every check J1 to J11 met at its registered run, NJ1 and NJ2 as expected, every mutation MJ1 to MJ10 caught, no
amendment after a run. **partial**: a check missed (kept as a `*_known_miss` with its limit), a negative control not as
expected, a mutation not caught, or a check met only after an amendment made after its run. **todo**: the module does not
build or the DS runs cannot be made. By D4 the expected outcome is **partial** (J6 predicted missed).

## Limits (not covered)

- The threshold is a rule of the DS sector with four modes per index and a basis of the ADR 0482 kind; it is not a bound: the
  indicator is the size of the second-order term against the first, and a mode with `r < 0.08` can in principle have larger
  third-order terms. J1 is a check of this basis only.
- One disc geometry, one basis size with a non-empty complement, no fan sector.
- The undefined-ratio and the refused-estimate fallbacks are not exercised by any DS case (MJ11).
- The cost of the fallback is that of the dense path for the fraction of modes it takes (D4); no cheaper fallback.
- No case-file key, no CLI output; `split_campbell` computes neither value.
