# ADR 0484: F-554 registration: zero-speed split frequencies against the Rayleigh quotients of the F-396 basis (the per-mode report of the quotient differences)

Status: **REGISTERED 2026-11-14, written before any code of F-554 and before any run of its checks**. Not yet accepted by the
maintainer. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Roadmap item F-554 (deps F-415), structural solver, advanced tier ([ADR 0310](0310-structural-solver-advanced-tier-plan.md)),
follow-up of F-415 ([ADR 0472](0472-f415-registration-coriolis-split-campbell-modal-subspace.md), `rcfd_fem::gyro`, tests in
`crates/rcfd-fem/tests/sa_gyro.rs`). Number 0484 given by the lead. Branch `f554-work` from 44b6c812, worktree
`w49i-work/f554`, scratch `w49i-work/f554-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact); each registered run made once; defaults bit-identical; no edit of
`baseline_hashes.rs`, of the hash tables or of any golden file; no existing test edited; no new dependency, no unsafe code;
clean-room. The choices below are taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).

## What F-554 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. At zero speed with the Coriolis option the split frequencies of ADR 0472 equal the square roots of the Rayleigh quotients of the F-396 rest modes within 1e-10 on the block, the ring and the radial beam of ADR 0472, and the difference between F-396's reported rest eigenvalues and those quotients is reported per mode; no default result changes.

## What was read, and the probe made before registering (disclosed)

Read: ADR 0472 (D2, Z1, amendment 1, addendum 1), `gyro.rs` (`split_modes`, `split_at`, `project_and_solve`), `rotating.rs`
(`RotatingPart::operator`, `mass`, `modes`), the set-ups `cube`, `ring`, `beam` and their options in `sa_gyro.rs`.

Probe, made once on a scratch copy of `sa_gyro.rs` (a test appended, removed afterwards, nothing committed), release profile, the
set-ups and options exactly as in ADR 0472: for every F-396 rest mode `phi_i` (`part.modes(0.0)`), the quotient
`rho_i = phi_i^T K0 phi_i / phi_i^T M phi_i` with `K0 = part.operator(0.0)` (the operator the split solve projects at rest) and
`M = part.mass()`; then `|rho_i/lambda_i - 1|` and, with the `rho` sorted ascending, `max |omega_split_i / sqrt(rho_i) - 1|` of
`split_modes(0.0)`:

| Model | modes | `max abs(rho/lambda - 1)` | `max abs(omega_split/sqrt(rho) - 1)` |
|---|---|---|---|
| CB (block on springs) | 3 | 2.1e-13 | 6.7e-16 |
| R (ring) | 9 | 9.2e-10 (modes 1, 2) | 8.7e-13 |
| F (radial beam) | 14 | 1.4e-10 (modes 0, 1) | 3.4e-11 |

The ring's other modes differ by 1e-13 to 2.4e-10, the beam's other modes by 1e-14 to 2e-12. The split-against-quotient values
agree with ADR 0472 amendment 1 (1.7e-12 and 3.4e-11 there; 8.7e-13 here on the ring, a different pairing of the modes of
the degenerate pairs). The 1e-10 limit therefore has a margin of 3x on the beam and more elsewhere.

## Design

**D1. Why the quotients are the right reference.** At rest `G = 0` and the reduced problem is `(K_r - lambda M_r) q = 0` with
`K_r = Phi^T K0 Phi`, `M_r = Phi^T M Phi` (the identity up to the eigensolver's orthonormality). Its diagonal entries are the
Rayleigh quotients `rho_i`; the off-diagonal entries are the small residual couplings of the F-396 vectors. The roots of the
reduced pencil equal the `rho_i` to second order in the off-diagonal entries over the gaps, and to first order inside an
(almost) degenerate pair (the beam's flap and lag pair, 3.4e-11). F-396's `lambda_i` is the converged eigenvalue of the subspace
solver, whose stopping test is a backward residual relative to `||K||` (ADR 0283 D6), so it carries the first-order error of
that stopping rule (up to 9e-10 against the quotient of its own vector on the ring). This is the diagnosis of ADR 0472
amendment 1; the item makes it a checked, reported fact.

**D2. A per-mode report as a public diagnostic.** `RotatingPart::rest_quotients(&self) -> Result<Vec<RestQuotient>>` in
`gyro.rs` (a new method; no existing line changes). `RestQuotient { index, eigenvalue, quotient, relative_difference }`:
`eigenvalue` is F-396's rest `lambda_i` as reported by `modes(0.0)`, `quotient = phi_i^T K0 phi_i / phi_i^T M phi_i` with
`K0 = operator(0.0)`, `relative_difference = quotient / eigenvalue - 1` (signed), in the order of F-396's modes. It is available
with or without `RotatingOptions::coriolis` (it only reads), refuses a cyclic model (key `"cyclic"`, as `split_modes`), and
returns the solver error of `split_modes` when the rest modes do not exist. It is chosen over a test-only function because the
report is a property of a part (F-416 and the cyclic follow-up can use it) and the arithmetic is five lines. Taken on the
standing instruction (recommendation).

**D3. No change of any default.** F-396's eigenvalues, `modes`, `campbell`, `split_modes` are not touched. The helper only
reads. Check D1 below shows the F-396 rest eigenvalues and modes are bit-identical before and after its calls, and the
existing `sa_gyro_default_bits.rs` (B2 of ADR 0472) passes unchanged.

**D4. The comparison.** In a new test file `crates/rcfd-fem/tests/sa_gyro_rayleigh.rs` (no existing test edited), for each of CB,
R, F built exactly as in ADR 0472 (the set-up code is copied, the existing file is not edited): the `rho_i` are taken from the
test's own recomputation by `matvec` (independent of the helper), sorted ascending, and compared with `split_modes(0.0)` by
`max |omega_i / sqrt(rho_i) - 1|` over the modes in ascending order (the split modes are ascending by construction).

## Checks

All limits fixed now. "Estimate" is from the probe above; no limit is derived from a value measured after this registration.

| Id | Check | Limit | Estimate |
|---|---|---|---|
| Q1 | count of split modes at rest = count of F-396 modes = length of the report: CB, R, F | equal | 3, 9, 14 |
| Q2 | `max abs(omega_split/sqrt(rho) - 1)` at zero speed with the option (the acceptance): CB, R, F | `<= 1e-10` each | 6.7e-16, 8.7e-13, 3.4e-11 |
| Q3 | the helper's `quotient` against the test's own recomputation, `max abs(q_helper/q_test - 1)` | `<= 1e-13` | 0 to 1e-16 (same arithmetic, written separately) |
| Q4 | the report per mode is printed (index, lambda, rho, relative difference); envelope of the largest `abs(relative_difference)`: CB, R, F | `<= 1e-11`, `<= 5e-9`, `<= 1e-9` | 2.1e-13, 9.2e-10, 1.4e-10 |
| Q5 | the helper's `eigenvalue` equals `modes(0.0).eigenvalues` by bits; indices `0..n` in order; every `quotient > 0` | each | holds |
| D1 | the F-396 rest eigenvalues and modes (bits) from `modes(0.0)` before and after calling the helper and `split_modes`; with the option off, `modes(0.0)` eigenvalue bits equal those with the option on | equal | equal |
| X1 | `rest_quotients` on a cyclic model (the clamped sector of R of ADR 0472) | error key `"cyclic"` | by construction |
| E1 | `cargo test -p rcfd-fem --release --test sa_gyro` and `--test sa_gyro_default_bits`; the whole `cargo test -p rcfd-fem --release`; `docs_site` | pass | pass |

Q4 is an envelope on the report, not an accuracy claim on F-396: it fixes what the report shows on these three models; it
catches a wrong operator or mass in the helper (MU2, MU3) while Q3 catches the others.

### Negative controls

| Id | Control | Expected |
|---|---|---|
| N1 | the old comparison of ADR 0472 Z1 on R: `max abs(omega_split/sqrt(lambda_F396) - 1)` | `> 1e-10` (4.6e-10 at the registered run of ADR 0472); shows the reference matters. On F the old value (1.03e-10) is recorded without a limit (too close to 1e-10) |
| N2 | a wrong quotient, `phi^T K0 phi / phi^T phi` (no mass weighting), on CB, compared with the split frequencies as in Q2 | `> 1e-3` |

### Planned mutations (scratch copy `f554-scratch/copy`, one at a time, logs kept)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the helper returns `lambda_i` as the quotient | Q3 (R, F) |
| MU2 | the helper uses `operator(1.0)` instead of `operator(0.0)` | Q3, Q4 |
| MU3 | the helper divides by `phi^T phi` instead of `phi^T M phi` | Q3, Q4 |
| MU4 | the helper does not refuse a cyclic model | X1 |
| MU5 | the helper reports the indices in reverse order | Q5 |
| MU6 | the helper's `eigenvalue` is taken from the quotient | Q5 |

## Decision rule

**done**: Q1 to Q5, D1, X1, E1 met at the registered run, N1 and N2 as expected, every mutation caught by a registered check.
**partial**: a check missed (recorded as `*_known_miss` with the limit kept and the number), a mutation not caught, or a clause
of the acceptance not run. **todo**: the new test file or the helper does not build.

## Limits (not covered)

- The item does not change F-396's eigenvalues (a final Rayleigh update would change default bits: the maintainer's call, option
  (c) of ADR 0472).
- Only the three set-ups of ADR 0472 (block, ring, radial beam) and the zero speed. Quotients at a speed (with `K_eff(omega)`)
  and the cyclic sector are not covered.
- The comparison in ascending order pairs split modes and sorted quotients by order; inside a degenerate pair this pairs by value,
  which is all the check needs.
