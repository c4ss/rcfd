# ADR 0261: F-361 registration: a least-squares (interface quasi-Newton) update of the nodal force in the coupled iteration

Status: **REGISTERED 2026-11-12**, written before any code of F-361 and before any run of its checks. Not yet accepted by the
maintainer. Before this text no check was run and no flow solver was run: the code and documents under "What was read" were
read, and the arithmetic under "Arithmetic before this registration" was done with two scratch scripts on the model problem
of ADR 0256 (no flow solver). Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0256](0256-f139-registration-cosimulation-participant.md) (F-139: `rcfd_fv::cosim`, the model problem CWs,
the one-node set-up SPm1, the settings C3, checks A1 to A5, Q1, C3, N1 and its question Q2, from which this item comes),
[ADR 0250](0250-f251-registration-strong-fluid-body-coupling.md) (F-251: `fsi::strong_step`, the added-mass argument D2/D5
for why the plain fixed point diverges, Aitken's factor D6). Roadmap item F-361 (depends on F-139).

Number: the lead gave 0261 to F-361. Branch `f361-quasi-newton-coupling` from main c7b08c41, worktree `w49i-work/f361`.

House rules (ADR 0256, `w49i-work/AGENT_RULES.md`):

- limits are fixed here, before the code and the runs, and are not changed after a run; the filter tolerance, the
  relaxation of the first update and the convergence tolerance are fixed here from the derivation and are not searched for
  after seeing results;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the number recorded, and the cause is
  diagnosed, not tuned away;
- a defective set-up gets a new corrected test by an amendment written and committed before the corrected run, and the old
  one stays as an ignored record;
- defaults stay bit-identical: the fixed and Aitken paths of `coupled_step` and every path of `fsi::strong_step` compute
  exactly what they computed before; the new update is a new variant that nothing selects by default;
- `crates/rcfd-api/tests/baseline_hashes.rs`, the hash tables and the goldens are not edited or re-recorded; no existing
  test is edited (`cosim_f139.rs`, `strong_fsi_f251.rs`, `added_mass_f351.rs` must pass unedited);
- each registered run is made once; no new third-party crate (no linear-algebra crate), no unsafe code, no commit
  trailers, no push.

Clean-room. The update and the filter are derived below from first principles (a secant condition and a linear
least-squares problem solved by Householder reflections). No paper, book, code or coupling library was opened for this
work. Names, cited from memory only so the reader can place the idea (nothing is taken on their authority): the interface
quasi-Newton method with an inverse-Jacobian least-squares approximation, IQN-ILS (J. Degroote, K.-J. Bathe, J.
Vierendeels, Computers & Structures 87 (2009) 793-801); its equivalence with GMRES on linear problems (R. Haelterman, J.
Degroote, D. Van Heule, J. Vierendeels, SIAM J. Numer. Anal. 47 (2009) 2347-2368); filtering of nearly dependent columns
through the QR factorisation (R. Haelterman, A. Bogaers, K. Scheufele, B. Uekermann, M. Mehl, Computers & Structures 171
(2016) 9-17); Householder's reflections (A. S. Householder, J. ACM 5 (1958) 339-342). The termination count used for the
limits is derived and checked numerically below, not taken from those papers.

## What F-361 asks

The acceptance text of `docs/roadmap.json`, binding:

> A least-squares (quasi-Newton) update of the nodal force from the residual history of the step, opt-in beside the fixed
> and Aitken factors, converges the model problem of ADR 0256 to 1e-8 in at most 7 iterations per step and 6 or fewer on
> average (Aitken: 8.8), gives the same frequencies within 1e-6, and with one rigid node needs no more iterations than
> Aitken; registered before the run with the plain fixed point as the diverging control.

Clauses: (a1) a least-squares update from the residual history of the step, opt-in beside the fixed and Aitken factors;
(a2) on the model problem (CWs of ADR 0256 amendment 1) every step converges to `1e-8` in at most 7 iterations, mean at
most 6; (a3) the frequencies equal those of the Aitken run within `1e-6`; (a4) with one rigid node (SPm1) no more
iterations than Aitken; (a5) registered before the run, with the plain fixed point as the diverging control.

## What was read (no run)

1. ADR 0256 in full, `crates/rcfd-fv/src/cosim.rs` (`coupled_step`, `SliceRelaxer`, `CouplingSettings::validate`,
   `CoupledStep`), `crates/rcfd-fv/tests/cosim_f139.rs` (the set-ups CWs and SPm1, `cw_run_on`, `cw_settings`,
   `sp_settings`, the checks), `docs/cosimulation-protocol.md`.
2. ADR 0250 (D2 to D6) and `crates/rcfd-fv/src/fsi.rs` (`Relaxation`, `Relaxer`, `StrongCoupling::validate`).
3. The log of ADR 0256's registered run (`w49i-work/f139-scratch/run1.log`, read only): the Aitken factors of the first
   three steps, the histogram of A1 (7: 13, 8: 210, 9: 343, 10: 84, mean 8.766), N1's ratios (`1.9978` to `1.9998` per
   iteration: the map is affine to four digits over ten iterations), SPm1 at most 4 iterations.
4. Where `Relaxation` is used: `fsi.rs`, `cosim.rs`, and by construction (never by a `match`) in the three test files above.
   `CouplingSettings` and `StrongCoupling` are built with struct literals in the tests, so neither gets a new field; the
   new update is a new variant of `fsi::Relaxation`.

## Derivation

**D1. The map and its slopes.** One coupling step iterates on the nodal force `f` (18 numbers on CWs: 6 nodes, 3
components). The pass through both participants returns `ft = G(f)`, the residual is `r(f) = G(f) - f`, and the fixed point
`r = 0` is the coupled solution. ADR 0256 C1: on CWs `G` is affine to high accuracy, `dG/df = -(1/2) A_s M_s^{-1}` on the
x components, with the eigenvalues `-lambda_j/2`, `lambda_j = 4.014, 0.476, 0.191, 0.087, 0.031, 0.016` (the mass ratios of
the six modes the nodes carry). The mock structure moves along x only, so the y and z components of `f` reach nothing:
`G` does not depend on them. The returned y components are not zero (the pressure on the slightly tilted wall faces, about
one percent of the x force, ADR 0256 addendum), so the residual has them. In blocks (`a` the x, `b` the y and z
components): `dG/df = [[A, 0], [C, 0]]`, and the Jacobian of the residual `J = dG/df - I` has the eigenvalues
`-(1 + lambda_j/2)` (six, from `-3.007` to `-1.008`) and `-1` (the twelve transverse components, a single eigenvalue with a
full eigenspace, since `A` is non-singular).

**D2. The update.** Within one step keep the iterates `f^i`, outputs `ft^i = G(f^i)` and residuals `r^i = ft^i - f^i`.
After iteration `k >= 2` form the differences of the last iterates, newest first,

    v_i = r^{k-i+1} - r^{k-i},   w_i = ft^{k-i+1} - ft^{k-i},   i = 1, 2, ...,

the columns of `V` (residual differences) and `W` (output differences). For a linear map a change `V c` of the residual is
caused by the change `(W - V) c` of the input. Ask for the combination whose predicted residual is smallest,

    c = argmin_c |V c + r^k|_2,

and take as the next input the output of iteration `k` corrected by the same combination of output differences:

    f^{k+1} = ft^k + W c  =  f^k + (W - V) c + (r^k + V c).

The first part moves the input along the directions already seen so that the predicted residual is the least-squares
remainder `rho_k = r^k + V c`; the last part adds that remainder unrelaxed (the plain fixed-point step in the directions not
yet seen). This is the interface quasi-Newton update with a least-squares approximation of the inverse Jacobian (IQN-ILS):
`(W - V) c = J^{-1} V c` on a linear map, so the update is a Newton step on the part of the residual in the span of `V`.

**D3. The first iteration.** With no difference yet (`k = 1`) the update is the relaxed step `f^2 = f^1 + w0 r^1` with
`w0 = relaxation_from_added_mass(m, rho L) = 1/3` on CWs, the factor of ADR 0256 C3 that annihilates the heaviest mode on
the model (so the first column already carries the mode that makes the plain fixed point diverge). The same relaxed step is
taken whenever no column survives the filter (D5) or the least-squares coefficients are not finite.

**D4. Termination on a linear map (the count behind the limits).** Write `e^1` for the error of `f^1`; on an affine map
`r(f) = J (f - f*)`. Then: `r^2 = (I + w0 J) r^1`; and for `k >= 2`, from D2, `r^{k+1} = r^k + J ((W - V) c + rho_k) = r^k +
V c + J rho_k = (I + J) rho_k`. By induction the columns of `V` span `J K_{k-1}` with `K_m` the Krylov space of `J` and
`r^1` of dimension `m`, and `rho_k` is the least residual over `r^1 + J K_{k-1}`, the residual of `k - 1` steps of GMRES. So
the method is GMRES with one extra pass: if the minimal polynomial of `J` with respect to `r^1` has degree `d`, `rho_{d+1}
= 0`, the input `f^{d+2}` is exact and iteration `d + 2` meets any tolerance. On CWs `d <= 6 + 1 = 7` (the six modes and the
transverse eigenvalue `-1`), so exact termination is guaranteed only at iteration **9** (and at 8 without the transverse
components). Checked numerically (scratch `term_check.py`): random affine maps with `n = 1, 2, 3, 4, 6` distinct eigenvalues
converge at iterations 3, 4, 5, 6, 8, i.e. `n + 2`. So the acceptance's limits 7 (largest) and 6 (mean) are not reached by
termination in exact arithmetic: they rely on the tolerance `1e-8` being met before termination. That is what the
spectrum makes likely: the four light modes and the transverse block have `J`-eigenvalues within `0.096` of `-1`, a cluster
that a polynomial of low degree reduces by about that factor per degree, while the two separated eigenvalues (`-3.007`,
`-1.238`) take one degree each. The model arithmetic below estimates the count; the brief's "degrees of freedom plus one"
(ADR 0256 T2's "at most seven") is the GMRES count, one less than the passes of this protocol, where the first pass is an
iteration and the first update is relaxed.

**D5. Dependent columns: the filter.** The least-squares problem is solved by Householder QR of `V` (at most 18 rows, at
most `max_iterations - 1` columns) with no linear-algebra crate. The columns are taken newest first. Column `j` is reduced
by the reflectors of the columns kept before it; let `s_j` be the norm of its part below the current row (the would-be
`|R_jj|`, the part of `v_j` outside the span of the newer kept columns). **The column is dropped, with its `w_j`, for the
rest of the step, when `s_j < eps |v_j|` with `eps = 1e-2`, when `|v_j| = 0`, or when every row is used** (more columns than
rows). Then `c` follows by back substitution `R c = -Q^T r^k` over the kept columns, and every pivot satisfies `|R_jj| >= eps
|v_j| > 0`: the method cannot divide by a vanishing pivot. Dropping a column and continuing with the same reflectors is
exactly the QR of the reduced matrix, so one pass suffices. The oldest columns are the ones dropped, since the newest is
taken first and is always kept (unless it is zero).

Why `eps = 1e-2`. A column carries the non-affine part `delta` of two passes (the iterative solves of the flow step stop at
a relative residual `1e-13`; with a condition number of order `1e3` on the 20 x 8 pressure problem the solution error is
of order `1e-10` of the force) over the difference itself, which in every iteration that is not the last is at least the
residual of a non-converged iterate, `|v| >= 1e-8 |ft|` near the end: the relative noise of a late column is at most `eta =
1e-10/1e-8 = 1e-2`. A column whose own direction (`s_j/|v_j|`) is smaller than its noise carries no information and would
multiply the noise by `eta |v_j|/s_j > 1` in `c`; the filter at `eps = eta = 1e-2` removes exactly those. A column that
carries information is not dropped: consecutive residuals fall by large factors (model, first step: `2.3e-2`, `5.5e-3`,
`3.5e-5`, ...), so `v_i` is nearly `-r^{k-i}`, and the least-squares remainder is nearly orthogonal to the span of the newer
columns; such columns have `s_j/|v_j|` of order one. With one rigid node (D7) the residual is one-dimensional and every
column but the newest is parallel to it, `s_j` at rounding level: the filter keeps only the newest column, as the secant
method needs.

**D6. No reuse from earlier time steps.** The history starts empty in every coupling step and is dropped at its end. Reuse
of columns of earlier steps (or of their filtered Jacobian) is not added, not even as an option: it would make a step depend
on the history of the run, which the repeated step (`restore` and step again, ADR 0256 R1) and the protocol's statelessness
of the coupler between steps do not need, and the acceptance does not ask for it.

**D7. One rigid node: the secant method, which is Aitken's.** With one node the force residual has one non-trivial
component (the piston of SPm1: a plug, the y and z face loads are zero up to rounding). For one dimension, with the older
columns removed by the filter, D2 with one column `v = r^k - r^{k-1}`, `w = ft^k - ft^{k-1}` gives `c = -r^k/v` and
`f^{k+1} = f^k + r^k + (Delta f + v) c = f^k - Delta f r^k / v`, `Delta f = f^k - f^{k-1}`: the secant step. Aitken's
factor (ADR 0250 D6) `w_k = -w_{k-1} r^{k-1} v / v^2` with `Delta f = w_{k-1} r^{k-1}` gives `f^{k+1} = f^k + w_k r^k = f^k
- Delta f r^k/v`: the same step, and both start with the same relaxed step `w0 r^1`. So the two methods produce the same
iterates in exact arithmetic as long as Aitken's factor is inside its cap (`max = 2`; on SPm1 the factors stay near `1/3`).
They differ by rounding only: the expected iteration counts are equal in every step. That is why one rigid node must not
need more iterations than Aitken.

**D8. The frequencies.** Both methods stop each step at `|r| <= 1e-8 max(|ft|, f_scale)`; a force error at the tolerance
displaces a node by at most `1e-10 X` per step (ADR 0256 C3). Two runs that differ only by the converged iterate have
displacement histories within about `650 x 1e-10 X` and frequencies (upward crossings over 6 and 12 periods) within about
`1e-9`; the limit `1e-6` of the acceptance is a thousand times wider.

## Arithmetic before this registration (disclosed)

Scratch `w49i-work/f361-scratch/qn_model.py` (imports ADR 0256's `model.py` unchanged): the model of ADR 0256 (five-point
Laplace pressure of the wall faces, the maps `H`, `P`, `B`, lumped nodes with RK4) iterated with D2 to D5 exactly as
registered (`w0 = 1/3`, `eps = 1e-2`, Householder QR written out, newest column first), with and without a modelled y
component of the face force (`F_y = -F_x d'(y)`, the tilt of the face). The filter was run with the registered `eps = 1e-2`
and, for information only, with no filter (`eps = 0`); no other value was tried. `term_check.py`: the termination count of
D4 on random affine maps. Output in `qn_model.log`.

| Model, CWs (650 steps, `T_0/100`, tolerance `1e-8`) | Iterations: largest, mean; histogram | Columns dropped | `omega_m/omega` of `Q_0`, `Q_1` |
| --- | --- | --- | --- |
| Aitken from 1/3, x only (ADR 0256's model) | 11, 8.642; 7: 8, 8: 286, 9: 288, 10: 67, 11: 1 | | 0.9986251, 1.0114698 |
| Aitken, x and y | 11, 8.623 | | the same |
| least squares, `eps = 1e-2`, x only | 7, 5.982; 5: 13, 6: 636, 7: 1 | 68 | the same |
| least squares, `eps = 1e-2`, x and y | 7, 5.991; 5: 7, 6: 642, 7: 1 | 4 | the same |
| least squares, no filter, x and y (information) | 7, 5.991 | 0 | the same |
| first step, least squares, x and y: `|r_k|/|r_1|` | 1, 2.3e-2, 5.5e-3, 3.5e-5, 3.6e-7, 1.2e-8, 4.0e-11 | | |
| least squares against Aitken (x and y): relative difference of `omega_0`, `omega_1`; largest `abs(dQ_0)/X` | 1.3e-10, 2.5e-10; 1.2e-8 | | |

Reading. On the model the update reaches `1e-8` at iteration 6 in almost every step (the first step at iteration 7), two
to three passes before the exact termination of D4. The mean is `5.991` against the limit 6: **a margin of 0.009, the
smallest of this registration.** The flow step of ADR 0256 needed `8.766` iterations with Aitken where the model needed
`8.642` (0.12 more); a shift of that size here would put the mean above 6. The largest count, 7, is at the limit too
(one step). Both acceptance limits are therefore at risk, and a miss of the mean by a few hundredths is the outcome I
consider most likely after a pass.

## Design

**S1. `fsi::Relaxation::LeastSquares { initial, filter }`** (new variant): `initial` the factor of the relaxed first update
(D3), `filter` the column filter `eps` (D5). In `fsi.rs`: `Relaxer::new` starts it with `initial` and `Relaxer::factor`
treats it as the fixed factor `initial` (a 3-vector relaxer has no history of whole vectors); `StrongCoupling::validate`
rejects it with a message that the update exists in `cosim::coupled_step` only, so `strong_step` and `strong_step_with`
never run it. The two existing arms are untouched.

**S2. `cosim.rs`.** `CouplingSettings::validate` accepts `LeastSquares` with `initial` in `(0, 2)` and `filter` finite in
`(0, 1)`; a constructor `CouplingSettings::least_squares(initial)` (filter `1e-2`, tolerance `1e-8`, 20 iterations, stop at
the cap, as `aitken`). In `coupled_step` the update after a non-converged iteration branches: for `Fixed` and `Aitken` the
existing lines, unchanged in order and arithmetic; for `LeastSquares` a private `LeastSquaresUpdate` holding the previous
residual and output and the columns `V`, `W` of the step (new for every call of `coupled_step`, so nothing crosses a step),
which returns the next force (D2, D3) and the number of columns used. `CoupledStep` gains one field, `columns` (the number
of difference columns of every update; 0 for a relaxed update and for the fixed and Aitken factors). For the least-squares
update `factors` holds `initial` for a relaxed update and `1` for a least-squares one. A private `least_squares` routine
does the filtered Householder QR and the back substitution (D5).

**S3. Tests**: new file `crates/rcfd-fv/tests/cosim_f361.rs`; the set-ups CWs and SPm1 are copied from `cosim_f139.rs`
(not edited); heavy tests `#[cfg_attr(debug_assertions, ignore)]`. The least-squares settings on CWs: ADR 0256 C3 with
`relaxation = LeastSquares { initial: relaxation_from_added_mass(m, rho L) = 1/3, filter: 1e-2 }` (tolerance `1e-8`,
`f_scale = k X W Lz`, cap 20, stop at the cap); on SPm1: `sp_settings` of ADR 0256 with `LeastSquares { initial:
relaxation_from_added_mass(M, m_a) = 1/3, filter: 1e-2 }`.

A mock affine fluid in the test file (fast tests): `n` structural nodes, one wall point and one single-point face per node
(the identity map), face force `F_x = c - K d_x` (a fixed symmetric `K`), `F_y = 0.1 F_x` of the neighbouring node (so the
transverse block of D1 is present), its state the time and the last displacement.

## Registered checks

All in `crates/rcfd-fv/tests/cosim_f361.rs`, release, each run once.

### Acceptance

| Id | Check | Limit | Estimate behind it |
| --- | --- | --- | --- |
| QA1 (a1, a2) | CWs, 650 steps, least squares: every step converged; iterations per step, largest; mean; largest last test quantity | all; `<= 7`; `<= 6`; `<= 1e-8` | model 7 and 5.991 (D4, the table); at risk, see the reading |
| QA2 (a3) | CWs, the least-squares run against the Aitken run (ADR 0256 C3) of the same test: `abs(omega_LS/omega_A - 1)` for `Q_0` and for `Q_1`; numbers of upward crossings | `<= 1e-6` each; equal | model `1.3e-10`, `2.5e-10` (D8) |
| QA3 (a4) | SPm1, 100 steps, least squares against Aitken (`sp_settings`): steps in which the least-squares count exceeds Aitken's; total counts; `max_n abs(x_LS - x_A)` | 0; `total_LS <= total_A`; `<= 1e-8 X` | D7: the same iterates up to rounding, equal counts in every step |
| QA4 (a5, diverging control) | CWs first step, `Fixed(1)`, cap 10, `Keep`: converged; `|r_10|/|r_1|` | false; `>= 200` | ADR 0256 N1: 510.9 (slope `-2` of mode 0) |

### Set-up, fast tests and defaults

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| S1 (set-up: the reference is ADR 0256's) | the Aitken run of QA2: iteration histogram; every step converged | equal to ADR 0256 A1's `{7: 13, 8: 210, 9: 343, 10: 84}`; all | the fixed and Aitken path is unchanged and the flow step deterministic |
| S2 (set-up) | the least-squares run of QA1: the number of columns of every update is at most the iteration index minus 1, and the first update of every step is relaxed (0 columns, factor `1/3`) | all steps | D2, D3 |
| U1 (D4, fast) | mock affine fluid, 3 nodes, `K` with three distinct slopes and the transverse block: least squares converges to `1e-12` relative; iterations | converged; `<= 6` (`d + 2`, `d = 4`) | D4 and `term_check.py` (`n = 4`: 6) |
| U2 (D5, D7, fast) | mock fluid, one node, the force a cubic function of the displacement (slope `-2` at the start, not affine), least squares against Aitken (`max = 2`, from `0.25`) on 5 steps: counts per step; the iterates; the columns of every update | equal; within `1e-12` relative; `[0, 1, 1, ...]` (the filter leaves only the newest column) | D7: the secant method; D5 |
| U3 (validation) | `CouplingSettings::validate` with `LeastSquares` `initial` 0, 2, NaN, `filter` 0, 1, NaN: errors; `(1/3, 1e-2)`: accepted; `StrongCoupling::validate` with `LeastSquares`: an error | as stated | S1, S2 |
| B1 (defaults) | `cosim_f139`, `strong_fsi_f251`, `added_mass_f351` of `rcfd-fv`, release, unedited (heavy ones included where they run in release); `cargo test -p rcfd-api --release --test baseline_hashes` (`fsi.rs` is edited); `git diff main` touches only `fsi.rs`, `cosim.rs`, the new test file, this ADR and `docs/cosimulation-protocol.md` | all pass; those files | S1, S2 leave the existing arms untouched |

Printed, no limit: the histograms, the residual norms and the columns of the first three steps, the columns dropped over
the run, Aitken's counts on U1.

### Mutations (scratch, not committed)

Each applied alone to the final tree by an exact text replacement that must occur once; the new test file is run once and
the named tests must fail; the file is restored and compared byte for byte with a kept copy.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MQ1 | the filter is off (`s_j < eps |v_j|` becomes `s_j < 0`) | U2 |
| MQ2 | the least-squares right-hand side has the wrong sign (`min |V c - r^k|`) | U1, QA1 |
| MQ3 | the update starts from the input instead of the output (`f^{k+1} = f^k + W c`) | U1, QA1 |
| MQ4 | the back substitution divides by `|R_jj|` instead of `R_jj` | U1 |

`scripts/limit_audit.py` is run on the new test file and every hit is read.

### After the code

`cargo test --release -p rcfd-fv --test cosim_f361 -- --test-threads=1 --nocapture`, the suites of B1, `cargo fmt -p
rcfd-fv --check`, `cargo clippy -p rcfd-fv --all-targets -- -D warnings`. The whole-workspace suite is not run (the lead's
gate).

## Predictions (mine, before any run)

QA1: every step converged, largest 7, mean between 5.95 and 6.15 (the mean is the clause I trust least: the model gives
5.991 and the flow added 0.12 iterations to Aitken's mean); QA2 below `1e-8`; QA3 equal counts in every step and `x`
within `1e-14 X`; QA4 about 510; S1 exact; U1 6 or fewer; U2 equal.

## Decision rule

- **done**: QA1 to QA4 are met (QA4 holds), S1 and S2 hold, U1 to U3 and B1 are met, and every mutation fails its tests.
  The acceptance stays the maintainer's.
- **partial**: the least-squares update converges every step of CWs (QA1's first clause) and one of QA1's count limits,
  QA2 or QA3 is missed (kept as `*_known_miss` with the limit intact, diagnosed); or the acceptance is met and one of U1 to
  U3, B1 is missed and not shown by an amendment to be a defective set-up; or a mutation is not caught.
- **todo**: the least-squares update fails to converge a step of CWs, or B1 fails (a default changed).

A set-up check (S1, S2) or the control QA4 that does not hold means the comparison is not shown to be sound: reported as a
defective set-up, corrected by an amendment before any re-run, never counted as a pass.

## Open choices taken

Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation); none changes a default.

- **T1. A new variant of `fsi::Relaxation`, not a new field.** The settings structs are built with literals in the
  existing tests, which must pass unedited; a variant leaves them valid. Options: a separate entry point
  (`coupled_step_least_squares`), which duplicates the protocol loop.
- **T2. The strong step of ADR 0250 rejects the variant.** Its force and moment are two 3-vectors iterated separately,
  where the secant method is already Aitken's in the one-dimensional case that matters (D7). Options: a least-squares update
  of the 6-vector there too (a follow-up if a body with several active modes needs it).
- **T3. The filter is relative to the column's own norm (D5), `eps = 1e-2`.** Options: relative to the largest pivot, or
  a fixed cap on the number of columns.
- **T4. No reuse of earlier steps** (D6). Options: reuse as a separately registered option (a follow-up, if a case with
  many interface modes needs it).

## Limits of this registration

The update is tested on the model problem of ADR 0256 only (six nodes, two excited modes, a nearly affine map) and on a
one-node piston; no strongly non-linear coupling, no large interface (the dense QR costs `O(m n^2)` per update with `m`
force components and `n <= max_iterations` columns, fine for tens of thousands of components and twenty columns, not
tested). No reuse of earlier steps, no multi-vector or multi-level variant, no relaxation of the displacement. The
strong step of ADR 0250 does not get the update.
