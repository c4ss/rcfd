# ADR 0467: F-548 registration: flag the damped-mode roots carried by the residual vectors

Status: **REGISTERED 2026-11-14, written before any code of F-548 and before any run of its checks**. Not yet accepted by the
maintainer. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Roadmap item F-548 (deps F-532), structural solver, advanced tier ([ADR 0310](0310-structural-solver-advanced-tier-plan.md)),
the follow-up proposed by [ADR 0464](0464-f532-registration-damped-modes-residual-vectors.md) (its open question 2 and
deviation 2). Number 0467 given by the lead. Branch `f548-work` from main 69cb996a, worktree `w49i-work/f548`, scratch
`w49i-work/f548-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test); each registered run made once; defaults bit-identical (new behaviour opt-in, here: it is reported only
with the option of ADR 0464); no edit of `baseline_hashes.rs`, of the hash tables or of any golden file; no existing test
edited; no new dependency, no unsafe code; clean-room. Every choice taken below that the maintainer would normally be asked
about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).

## What F-548 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. Each DampedMode reports the M-weighted share of its shape carried by the residual-vector coordinates of the basis, and roots with a share above a registered threshold are flagged as pseudo-modes of the truncation. Bar of F-532 at c = 0.9 Z and 2 Z on 20 modes with the residual vectors: the target roots of F-532 are not flagged, every other real root is; without the option every result is bit-identical to F-532's.

Clauses: **a1** each `DampedMode` reports the M-weighted share of its shape carried by the residual-vector coordinates;
**a2** roots with a share above a registered threshold are flagged; **a3** bar at `c = 0.9 Z` and `2 Z`, 20 modes, option on:
the target roots of F-532 are not flagged; **a4** every other real root is flagged; **a5** without the option every result is
bit-identical to F-532's.

## What was read (main 69cb996a, read only)

ADR 0464 in full (D1, the checks, the addendum: deviation 2, question 2, the follow-up text), `crates/rcfd-fem/src/damped.rs`
(`damped_modes`, `DampedMode`, `DampedModes`, the basis `phi = [modes, residual vectors]`, `project`), and
`crates/rcfd-fem/tests/sa_damped_residual.rs` and `sa_damped_residual_default_bits.rs` (the bar, `tip`, `nearest`,
`real_root`, the digest and the nine base constants).

## Design

The basis `B = [Phi_m, R_r]` is M-orthonormal in the sense of ADR 0464 (the residual vectors are twice M-orthogonalised against
the modes and each other and M-normalised; `M_r = B^T M B` is the identity up to rounding). A root has the Ritz shape `x = B q`
(`q` the complex eigenvector of the projected problem). Split `q = (q_M, q_R)`. The part of the shape carried by the residual
vectors is `x_R = R q_R`; its **share** is the M-weighted fraction

`share = (q_R^H (M_r)_RR q_R) / (q^H M_r q)`, in `[0, 1]` (up to rounding),

that is `x_R^H M x_R / x^H M x` when the cross block `(M_r)_MR` (zero to rounding) is neglected, and equal to the sum of
`|q_i|^2` over the residual coordinates divided by the sum over all coordinates for an exactly orthonormal basis. It uses the
`M_r` that `damped_modes` already forms. Without the option or without a dashpot there are no residual coordinates and the
share is exactly `0.0`.

A root is a **pseudo-mode of the truncation** (`pseudo = true`) when `share > PSEUDO_SHARE`; `pub const PSEUDO_SHARE: f64 =
0.1`, fixed now from the arithmetic below, not by a search. Fields added: `DampedMode::residual_share: f64`,
`DampedMode::pseudo: bool`, `DampedModes::pseudo_modes: usize` (count of reported modes flagged), and a note "`k` roots
flagged as pseudo-modes of the residual vectors (share above 0.1)" appended after the existing notes only when `k > 0` (so
without the option the notes are those of F-532). No existing field changes. Taken on the standing instruction: the threshold
is a constant, not an option (one fewer public knob; the shares are reported, so a caller may apply another).

### Why a threshold of 0.1 (derivation, then disclosed arithmetic)

A physical root of the truncated model approximates a continuum root; its shape is the continuum mode, whose expansion on the
undamped modes decays like `1/beta_k^2` (the dashpot term `s phi_k(L)/(s^2 + beta_k^2)`), so the part of its M-norm outside
the first `m` modes is a small tail and the residual vector (a stand-in for that tail) carries a small share, growing with
the order of the root as it nears the truncation (`s ~ beta_m`). A pseudo-root of the residual vector is a root of the
strongly damped coordinate that the vector adds, whose shape is mainly that vector: share of order 1. The threshold sits
between.

Arithmetic before this registration, run once (script `w49i-work/f548-scratch/estimate.py`, log `estimate.log`; numpy only, no
rcfd code): the one-dimensional consistent-mass chain of 100 elements of ADR 0464 (`nu = 0` bar), 20 modes plus the residual
vector of the tip force, state-matrix eigenvalues, share as defined above (the chain has one residual vector; the 3-D bar has
four).

| Set-up | target family (`s = lambda L/c_0`) | share | other roots | share |
|---|---|---|---|---|
| `c = 0.9 Z` | `j = 1, 2, 3` (`Im s` 1.57, 4.71, 7.86) | 1.05e-5, 5.5e-5, 1.46e-4 | the 20th oscillating root; one oscillating pseudo-root `s = -40.1 + 61.8 i` | 1.3e-2; 0.399 |
| `c = 2 Z` | real root `s = -0.5493`; `j = 1, 2, 3` | 3.2e-6; 1.9e-5, 7.5e-5, 1.7e-4 | the 20th oscillating root; real pseudo-root `s = -275.6` | 1.5e-2; 0.68 |

By the structure of the 3-D bar (ADR 0464 estimate 4) the four dashpots give four residual vectors, one in the axial symmetry
class (the chain above) and three in classes of non-uniform tip motion that do not couple to it; the overdamped pairs of the
latter are decoupled from the modes of the axial class, so a share near 1 is expected (a conjecture of the estimate, checked
by M1 below as a limit with margin). The largest share of a physical root in the table is 1.5e-2 and the smallest share of a
pseudo-root is 0.40: the threshold 0.1 is 6.7 times above the first and 4 times below the second.

Base digests (before any code): the existing file `sa_damped_residual_default_bits.rs` at main 69cb996a prints 9 of 9 digests
equal to the constants of ADR 0464 (log `f548-scratch/base-d1.log`). The digests of the option-on set-ups (bar `c = 0.9 Z` and
`2 Z`, 20 modes, option on, every existing field) are printed at the base by the new file `sa_damped_flag_default_bits.rs`,
written with the API of main 69cb996a only and committed before any library line; they become the constants of its second
test.

## Checks (limits fixed now)

Tests in new files `crates/rcfd-fem/tests/sa_damped_flag_default_bits.rs` (D1, D3) and
`crates/rcfd-fem/tests/sa_damped_flag.rs` (the rest), release profile. Bar, dashpots, `tip`, `nearest`, `real_root` as ADR 0464
(`ModalOptions::new(20).tolerance(1e-12).residual_test(Backward)`). The target roots: oscillating `lambda_j = (c0/L)(-atanh(0.9)
+ i (2 j - 1) pi/2)` (`c = 0.9 Z`) and `(c0/L)(-atanh(1/2) + i j pi)` (`c = 2 Z`), `j = 1, 2, 3`, each matched by the reported
oscillating root nearest it; at `c = 2 Z` also the real root `-(c0/L) atanh(1/2)`, matched by the non-oscillating root of
smallest modulus.

| Id | Clause | Check | Limit | Estimate |
|---|---|---|---|---|
| P1 | a1 | option on, `c = 0.9 Z` and `2 Z`: every reported mode: `residual_share` in `[0, 1 + 1e-10]`; and `abs(residual_share - s_ind) <= 1e-10`, `s_ind` = (sum over the residual vectors of `(r_i^T M Re x)^2 + (r_i^T M Im x)^2`) over `x^H M x`, from the public `shape`, `residual_vectors` and the model's consistent mass | limits | rounding 1e-14 |
| P2 | a2 | the flag is exactly `residual_share > PSEUDO_SHARE` for every reported mode (both set-ups); `PSEUDO_SHARE == 0.1`; `pseudo_modes` equals the count flagged; a note naming that count is present iff the count is positive | exact | holds |
| T0 | a3 | `c = 0.9 Z`: the three target roots: `pseudo == false` and share | `false`; `<= 1e-2` | 1.5e-4 |
| T1 | a3 | `c = 2 Z`: the real target root and the three oscillating targets: `pseudo == false` and share | `false`; `<= 1e-2` | 1.7e-4 |
| R1 | a4 | `c = 0.9 Z`: every non-oscillating root reported (6 in ADR 0464) has `pseudo == true` | all | share near 1 |
| R2 | a4 | `c = 2 Z`: every non-oscillating root reported except the target root of smallest modulus (7 in ADR 0464) has `pseudo == true` | all | 0.68 to 1 |
| M1 | a4 margin | the smallest share among the R1 and R2 roots | `>= 0.3` | 0.68 |
| M2 | a2 margin | the largest share among the reported oscillating roots that are not flagged, both set-ups; recorded also: how many oscillating roots are flagged | `<= 5e-2` | 1.5e-2 |
| D1 | a5 | the nine digests of the existing set-ups without the option equal the nine constants of `sa_damped_residual_default_bits.rs`; in addition every `DampedMode` of these results has `residual_share == 0.0` (bits), `pseudo == false`, and `pseudo_modes == 0` | `==` | holds |
| D2 | a5 | option on with no dashpot (the cantilever with Rayleigh damping; the loss factor): result bit-identical to the option off in every field of the existing lists; shares 0, no flag | bits | holds |
| D3 | a5 | option on, the two bar set-ups (`0.9 Z`, `2 Z`, 20 modes): the digest of every existing field (the list of D1 plus the residual vectors) equals the constants printed at main 69cb996a by `sa_damped_flag_default_bits.rs` before any library line (the new fields change no old number, even with the option) | `==` | holds |
| E1 | a5 | existing `sa_damped`, `sa_damped_residual`, `sa_damped_residual_default_bits`, `sa_scaffold` pass unedited | pass | holds |
| H1 | a1 | 1 and 4 threads, option on: every `residual_share` and `pseudo` bit-identical | bits | holds |

Mutations (on a scratch copy; each must be caught by at least one registered check):

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | share of the mode coordinates instead (`1 - share`) | T0, P1 |
| MU2 | flag when the share is below the threshold | T0, R1, P2 |
| MU3 | only the first residual vector's coordinate counted in the share | R1, R2, P1 |
| MU4 | `PSEUDO_SHARE = 0.9` | R2 (the pseudo-root of share 0.68 at `c = 2 Z`), P2 |
| MU5 | `PSEUDO_SHARE = 1e-4` | T0, T1, M2 |
| MU6 | `pseudo_modes` not set, or the note missing | P2 |
| MU7 | a flag set in the default path (share 1.0 without the option) | D1 |

## Decision rule

**done**: every check met at its registered run, every mutation caught, no amendment after a run. **partial**: a check missed
(recorded as `*_known_miss` with its limit), a mutation not caught by a registered check, or a check met only after an
amendment made after its run. **todo**: the change does not build or the bar runs cannot be made.

## Limits (not covered)

- The share is a diagnostic of the Ritz shape. It is not a bound on the error of a root, and a physical root with a large
  tail (the highest roots near the truncation) can exceed 0.1 and be flagged; M2 records, it does not forbid it for the
  flagged ones.
- The threshold is fixed from one-dimensional chain estimates and the structure of the 3-D bar, not from a general theory; it
  is a constant of the library, not an option.
- Residual vectors of loads, of Rayleigh or loss-factor damping: none exist (ADR 0464), so nothing is flagged there.
- Roots flagged are kept in the results (reported, not removed).

## Amendment 1 (2026-11-14, written after the first run of D3 and before its corrected run)

The first run of the checks (`cargo test --release -p rcfd-fem --test sa_damped_flag_default_bits`, option-on test D3) printed
0 of 2 digests equal: D3 as registered digests every existing field including the notes, and with the option on a result with
flagged roots carries one more note by design (the design appends it after the existing notes when `k > 0`; D3 was
over-written, the registration's "no existing field changes" refers to the numbers and the notes without flagged roots). This
is a miss of D3 as registered: its test stays as `d3_option_on_results_unchanged_known_miss` (ignored, limit intact). The
corrected check **D3b**: the digest of every number of the option-on results (all existing fields except the notes, plus the
residual vectors) equals constants printed at main 69cb996a by the same file in a scratch archive of that commit before the
corrected run; the notes of those results are compared by the content `existing notes ++ [the new line]` in
`sa_damped_flag.rs` P2 (the note is present iff flagged roots exist). Per the decision rule this makes D3 met only after an
amendment, hence the decision cannot be **done**.

## Addendum 1: results (2026-11-14)

Commits: e1297d3d (registration, alone), f93ea030 (the bit record file at main 69cb996a, constants printed before any library line),
then the library change and checks, bf9b007f (amendment 1), and the D3b constants. Registered runs, each made once, release
profile: `cargo test --release -p rcfd-fem --test sa_damped_flag` (log `f548-scratch/run_flag.log`), `--test
sa_damped_flag_default_bits`, and the existing `sa_damped` (7), `sa_damped_residual` (5), `sa_damped_residual_default_bits` (1),
`sa_scaffold` (9, 11 ignored records), all unedited and passing. The D3b constants were printed in a scratch archive of main 69cb996a
(`f548-scratch/base`).

| Id | Limit | Measured | Met |
|---|---|---|---|
| P1 share in [0, 1]; against the share recomputed from the public shapes and residual vectors | `<= 1e-10` | 1.2e-15 (0.9 Z), 2.2e-15 (2 Z) | yes |
| P2 flag exactly `share > 0.1`, count, note | exact | 7 flagged (0.9 Z), 8 (2 Z); holds | yes |
| T0 `c = 0.9 Z` targets not flagged; share | `false`; `<= 1e-2` | none flagged; 1.46e-4 | yes |
| T1 `c = 2 Z` real target and oscillating targets not flagged; share | `false`; `<= 1e-2` | none flagged; 1.70e-4 (real target 3.2e-6) | yes |
| R1 `c = 0.9 Z`: every real root flagged | all | 6 of 6 (share 1.0000) | yes |
| R2 `c = 2 Z`: every real root except the target flagged | all | 7 of 7 | yes |
| M1 smallest share of those roots | `>= 0.3` | 1.0 (0.9 Z); 0.682 (2 Z; estimate 0.68) | yes |
| M2 largest unflagged oscillating share; recorded count of flagged oscillating | `<= 5e-2` | 1.27e-2, 1.50e-2; 1 each (0.399 and 0.182) | yes |
| D1 nine digests without the option, base constants; shares 0, no flag | `==` | 9 of 9; clean | yes |
| D2 option on without dashpots equals off | bits | identical | yes |
| D3 option-on digests including notes | `==` | **0 of 2** | **no** (miss, below) |
| D3b corrected: option-on numbers without the notes | `==` | 2 of 2 | yes, after amendment 1 |
| E1 existing tests | pass | pass | yes |
| H1 1 and 4 threads | bits | identical | yes |

The one-dimensional estimate agreed with the 3-D bar: the pseudo-root of the axial class has share 0.399 at `c = 0.9 Z` (an
oscillating root, not real) and 0.682 at `c = 2 Z` (real); the other pseudo-roots have share 1.0 exactly (decoupled classes).
The chain's 1.3e-2 and 1.5e-2 for the 20th physical root were reproduced (1.27e-2, 1.50e-2).

Deviation and miss: D3 as registered digested the notes, and an option-on result with flagged roots has one more note by design;
the check was over-written (the registration meant the numbers). Missed as registered; recorded as the ignored
`d3_option_on_results_unchanged_known_miss` with its limit; corrected as D3b by amendment 1 before its run. Also: `cargo fmt`
re-laid out the bits file after its base commit (layout only; the constants were printed by the earlier layout).

Mutations (scratch copy `f548-scratch/copy`, `mutate.py`, logs `MU_*.log`; MU7 was re-run by hand because the script stopped before it):

| Id | Caught by |
|---|---|
| MU1 `1 - share` | P1, T0, R1, M1 |
| MU2 flag when below | P2, T0, R1, M2, D1 |
| MU3 only the first residual vector counted | P1, R1, R2, M1 |
| MU4 threshold 0.9 | P2, R2, M2 |
| MU5 threshold 1e-4 | P2, T0, T1 |
| MU6 count not set | P2 |
| MU7 share 1.0 without the option | D1, D2 (both files) |

Seven of seven caught. Not run: the whole workspace and `baseline_hashes` (only `damped.rs` of rcfd-fem changed; no default
path changed). `cargo fmt -p rcfd-fem --check` clean, `cargo clippy -p rcfd-fem --all-targets --release -- -D warnings` clean.

**Decision by the registered rule: partial.** Every roadmap clause is met (a1 to a5 by P1 to R2 and D1, D2, D3b), but D3 as registered
missed and D3b was met only after an amendment made after the run.

Remaining questions:

1. The share is a diagnostic; an oscillating pseudo-root (share 0.399, 0.182 here) is also flagged. Options: (a) keep (a flag on
   share only, as registered); (b) restrict flags to real roots. Recommendation: (a).
2. A caller-set threshold. Options: (a) constant `PSEUDO_SHARE` (here); (b) an option. Recommendation: (a) until a second use.

### For the lead: roadmap entries

F-548, proposed status **partial**:

> Structural solver, advanced tier. Follow-up of F-532 (ADR 0464). ADR 0467, 2026-11-14: each DampedMode of damped_modes with the residual vectors reports residual_share, the M-weighted share of its shape carried by the residual-vector coordinates of the basis (0 without them), and pseudo = share > PSEUDO_SHARE (0.1, fixed from a one-dimensional chain estimate: physical roots at most 1.5e-2, pseudo-roots at least 0.18 to 0.40); DampedModes::pseudo_modes counts them and a note says so. Bar of F-532 at c = 0.9 Z and 2 Z on 20 modes: target roots not flagged (shares at most 1.7e-4), every other real root flagged (6 and 7 roots, shares 0.68 to 1.0); one oscillating pseudo-root each (0.399, 0.182) also flagged; share equals the one recomputed from shapes and residual vectors within 2.2e-15. Without the option the nine F-414/F-532 results are bit-identical (shares 0, no flag). Partial: the registered check D3 (option-on digests including notes) missed because an option-on result gains a note by design; the corrected D3b (numbers without notes) was met after amendment 1. Seven of seven mutations caught.
