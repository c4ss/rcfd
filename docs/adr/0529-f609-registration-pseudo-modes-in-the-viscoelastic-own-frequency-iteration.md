# ADR 0529: F-609 registration: pseudo-modes in the viscoelastic own-frequency iteration

Status: **REGISTERED 2026-11-14, written before any code of F-609 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-609 was written, compiled or run in the repository. No arithmetic or probe was made
for it; the numbers used below (170 iterations, rate 0.86, share 0.986, the physical roots taking 7 and 6) are those printed
by the F-586 diagnostic and recorded in ADR 0520 Amendment 1, item 3. Results go in the addendum; nothing above the addendum
is changed after a run.

Roadmap item F-609 (dep F-586), structural solver. Number 0529 given by the lead. Branch `f609-work` from 1f142196, worktree
`w49i-work/f609`, scratch `w49i-work/f609-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number); each registered run made once; defaults bit-identical; no
edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code; no
existing test edited; clean-room. Other agents edit `damped.rs` (F-588, the stiffness assembly): the hunks here are small and
local to the own-frequency loop and the option struct. Choices taken below that the maintainer would normally be asked about are
taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).

## What F-609 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. The own-frequency iteration of the damped modes leaves the roots flagged pseudo (ADR 0467) at the reference frequency and says so in the notes; the rod B of ADR 0520 RV1 with residual vectors and a ground dashpot converges every physical root within the default 50 iterations, with the same physical roots as the iteration without that restriction within 1e-12 relative. Models without the option keep their bits.

Clauses: **a1** roots flagged pseudo stay at the reference frequency (not iterated) and the notes say so; **a2** rod B RV1
(basis 4, `OwnFrequency::new(3)`, ground dashpot `1e3` on the tip, residual vectors): every physical root converges within the
default 50 iterations; **a3** the physical roots equal those of the iteration without the restriction within `1e-12`
relative; **a4** models without the option keep their bits.

## What was read (1f142196, read only)

ADR 0520 in full (design D1 to D3, the checks, Amendments 1 and 2, the addendum, the follow-up text), ADR 0467 (the share of a
root in the residual-vector coordinates, `PSEUDO_SHARE = 0.1`, `DampedMode::pseudo`), `crates/rcfd-fem/src/damped.rs`
(`OwnFrequency`, `OwnFrequencyRoot`, the loop at the end of `damped_modes`), `tests/sa_damped_visco_own.rs` (rod B, RV1
and RV1b, `Checks`, `bits`) and `tests/sa_damped_visco_own_default_bits.rs` (the digest style).

## Design

**The restriction.** A root with `share > PSEUDO_SHARE` of the frozen solve is a pseudo-mode of the truncation (ADR 0467), not a
root of the continuum. The F-586 loop iterates each of the lowest `roots` roots of the frozen solve independently
(`w <- Im lambda`, nearest root). With the restriction, a root with `DampedMode::pseudo` in the frozen solve is not iterated:
it is reported as the frozen solve gave it, at the reference frequency, with `OwnFrequencyRoot { frozen: its eigenvalue,
omega: omega_ref, iterations: 0, change: 0, skipped: true }`. The set of roots counted by `OwnFrequency::roots` is unchanged
(the lowest `roots` reported by the frozen solve, pseudo ones included), so "the same physical roots" is a well-defined set in
both runs. Every other root runs the F-586 loop unchanged.

**Why the physical roots do not change.** The roots are iterated independently, each from its own frozen start, with no
shared state, so skipping one root cannot alter another: the expected difference of the physical roots between the restricted
and the unrestricted run is exactly zero bits, and the registered limit is the acceptance's `1e-12`.

**Option or behaviour change (decision, standing instruction).** Options: (a) a new opt-in `OwnFrequency::skip_pseudo(bool)`,
default `false`; (b) change the F-586 behaviour for every own-frequency run. Taken: **(a)**. Reason: (b) would change F-586's
registered results (the RV1 refusal at 50 iterations and the amended RV1b "root 0 takes more than 50 iterations", recorded as
ignored known-miss tests) and would make the comparison "the iteration without that restriction" unavailable at the default;
(a) keeps every F-586 test and record valid and unchanged and keeps the bits of every model that does not ask for it. No
case-file key (none exists for the own-frequency option either).

**The comparison run.** The iteration without the restriction is the same options with `skip_pseudo` off and
`max_iterations(1000)` (170 iterations needed for the pseudo root, ADR 0520). Roots of the two runs are matched by
`OwnFrequencyRoot::frozen` (identical bits in both runs by construction), not by order, because the iterated pseudo-mode moves
(to 634 rad/s) and the sorted order may differ.

**Notes.** With the restriction on, the notes carry, in addition to the F-586 text with `count` replaced by the number
iterated: "`k` roots flagged pseudo (ADR 0467) left at the reference frequency `omega_ref` rad/s, not iterated (ADR 0529)", or,
when none is flagged, "no root flagged pseudo among the `count` iterated (ADR 0529)". The text of runs without the
restriction is unchanged.

**Limits of the design.** The flag is read from the frozen solve; a root that becomes pseudo during the iteration is not
detected. A flagged root is not a root of the continuum and its reported value is the frozen one (a truncation artefact, ADR
0467).

## The test set-ups

Rod B of ADR 0520 (`Rod::new(Kind::Axial, 0.3, 0.15, 0.05)`, 40 elements, `hex_box [1, .05, .05]`), the RV1 options (basis 4,
`viscoelastic_reference(omega_c)`, ground dashpot `Dashpot::ground(tip, 0, 1.0e3)`, `residual_vectors(true)`),
`OwnFrequency::new(3)`. Rods A and C for PR and the digests of the default-bits record as in ADR 0520 B1. The new files:
`crates/rcfd-fem/tests/sa_damped_visco_own_pseudo.rs` (checks) and `sa_damped_visco_own_pseudo_default_bits.rs` (record N1,
expected digests printed on the unchanged base 1f142196 and written into the file in a commit before any library code).

## Checks (limits fixed now)

Disclosed expectations from the ADR 0520 diagnostic: frozen root 0 is the pseudo-mode (share 0.986), roots 1 and 2 are physical
and take 7 and 6 iterations; the pseudo-mode converged unrestricted takes 170 iterations at 634 rad/s from a frozen `Im` near
1251.

| Id | Clause | What | Limit | Estimate behind the limit |
|---|---|---|---|---|
| PA | a2 | RV1 with `skip_pseudo(true)` and the default 50 iterations: `Ok`, 3 modes, every root finite | holds | D1 |
| PF | a1 | pseudo flags of the three frozen roots (the run without own frequency): root 0 flagged, roots 1 and 2 not | holds | share 0.986 (ADR 0520) |
| PB | a1 | every record of a flagged root: `skipped`, `iterations == 0`, `omega == omega_ref`, and the reported eigenvalue and shape bit equal to those of the frozen solve (same options without the own frequency, matched by `frozen`) | exact | the skip returns the frozen mode |
| PD | a2 | every unflagged root: not `skipped`, `1 <= iterations <= 50`, finite | `<= 50` (default) | 7 and 6 |
| PS | a2 | the iteration counts of the physical roots | `<= 15` each | 7 and 6 |
| PE | a3 | physical roots of PA against those of the unrestricted run (`skip_pseudo` off, `max_iterations(1000)`), matched by `frozen`: `|dlambda|/|lambda|` worst, and equal iteration counts | `<= 1e-12` (acceptance); counts equal | independent loops: 0 |
| PG | a3 (control) | the unrestricted run with the default 50 iterations: `FemError::Solver`, solver `"viscoelastic own frequency"` (F-586 behaviour kept) | holds | ADR 0520 RV1 |
| PH | a3 (control) | the unrestricted root 0 (the pseudo-mode): `iterations` in `[100, 300]`, and its eigenvalue differs from the frozen one by relative `>= 1e-3` (so the restriction changes what is reported) | `[100, 300]`; `>= 1e-3` | 170; `Im` 1251 to 634 |
| PC | a1 | the notes with the restriction contain `pseudo`, `left at the reference frequency` and `ADR 0529`; the unrestricted notes do not contain `ADR 0529` | holds | D1 |
| PN | a4 | `skip_pseudo(false)` explicit equals the default bit for bit (modes, records, notes); `OwnFrequency::new(n).skip_pseudo` is `false` | bits equal | the field defaults false |
| PR | a4 | rods A and C (no residual vectors, no flagged root), 4 roots, `skip_pseudo(true)` against `false`: modes and records bit equal, all `skipped` false, the "no root flagged pseudo" note present | bits equal | nothing flagged |
| PT | D1 | RV1 with the restriction, `threads(1)` and `threads(4)`: modes and records bit equal | equal | per-element assembly |
| NC1 | negative control of PA | PG: without the restriction the set-up of PA is refused at 50 | holds | 170 needed |
| NC2 | negative control of PB | the unrestricted root 0 is not the frozen one (PH second part) | `>= 1e-3` | as PH |
| RF | refusals | with `skip_pseudo(true)`: `roots = 0`, tolerance 0, `max_iterations = 0`, own frequency without a reference frequency: each `FemError::InvalidValue` key `"own frequency"` | holds | validation unchanged |
| N1 | a4 | the new default-bits file: every digest of the base 1f142196 (rod A own, rod B residual vectors own with `max_iterations(500)`, rod C own, the 50-iteration refusal message) | equal | D3 |
| N1b | a4 | `sa_damped_visco_own.rs`, `sa_damped_visco_own_default_bits.rs` and the other `sa_damped*` tests, unchanged files, pass | pass | opt-in |
| N2 | a4 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | no shared output changed |
| N3 | a4 | `cargo test -p rcfd-fem --release` | pass | opt-in |

Every aggregated error is required finite (`worst` returns infinity for NaN, and `at_most` rejects non-finite values). Whole
check function budget: about 10 s in release.

## Planned mutations (on a scratch copy with its own `CARGO_TARGET_DIR`, one at a time)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the option ignored (no root skipped) | PA (refused at 50), PB, PC |
| MU2 | every root skipped | PB, PD, PE |
| MU3 | the skip decision inverted (physical roots skipped, pseudo iterated) | PA, PB, PD |
| MU4 | a skipped root reported at its frozen `Im` instead of `omega_ref` | PB |
| MU5 | the skip note not written | PC, PR |
| MU6 | skipping active whatever the option (default changed) | PG, PN, N1 |
| MU7 | a skipped record without the `skipped` flag | PB, PR |
| MU8 | a skipped root reported with the eigenvalue of a neighbour (the last mode) | PB |

## Decision rule

- **done**: every check met, every mutation caught by at least one check, fmt and clippy clean, N1, the older damped tests and
  the baseline hashes unchanged.
- **partial**: N1, N1b, N2 met, PA, PD and PE (the acceptance) met and a miss elsewhere diagnosed and recorded as an ignored
  `*_known_miss` test.
- **todo**: otherwise, in particular any change of default bits or PA or PE missed.

## Limits (not covered)

- A pseudo-mode's own frequency is not computed (it has no physical meaning, ADR 0467); it is reported at the reference
  frequency with the frozen eigenvalue, so it is not a fixed point.
- Only the roots of the frozen solve are tested for the flag; the 3-D cantilevers and bases without residual vectors have no
  flagged root and are covered only by the equality PR.
- No case-file key.
