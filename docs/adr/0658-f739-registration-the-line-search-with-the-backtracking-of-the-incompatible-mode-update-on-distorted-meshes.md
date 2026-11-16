# ADR 0658: F-739 registration: the line search with the backtracking of the incompatible-mode update as a registered remedy on distorted meshes (the eight meshes of ADR 0649 and three further seeds of the 8^3 block at 0.25 h; a sufficient-decrease test of the mode update measured beside it)

Status: **REGISTERED 2026-11-16, written before any code of F-739 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-739 was written, compiled or run: the files listed under "What was
read" and the logs of ADR 0649 (`w49i-work/f719-scratch/run1-default.log`) and ADR 0655 (`w49i-work/f732-scratch/run1-default.log`,
`mutations.log`) were read, and the arithmetic below was done by hand (disclosed). No probe was run for this item. Results go
in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-739 (deps F-732), the follow-up proposed by
[ADR 0655](0655-f732-registration-the-inverted-iterate-of-the-incompatible-hex8-at-the-punch-edge.md) ("For the lead",
follow-up proposed; its remaining question 1, option (a) with (b) measured beside it). The element is the incompatible-modes
Hex8 at finite strain of [ADR 0608](0608-f596-registration-incompatible-modes-hex8-at-finite-strain.md) with the stabilisation
of ADR 0614 D1 and the rule `IncompatibleStabilisation::LinearStress` of ADR 0630 D1 (rule L); the reference is the F-bar Hex8
of ADR 0506; the meshes are those of
[ADR 0649](0649-f719-registration-the-stabilised-incompatible-hex8-under-load-on-distorted-meshes.md) D2. Number 0658 given by
the lead. Branch `f739-work` from main 5333c312 (F-732 / ADR 0655 merged), worktree `w49i-work/f739`, scratch
`w49i-work/f739-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical (the new behaviour opt-in); no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden
file; no existing test edited; no new dependency, no unsafe code, the crate graph unchanged; clean-room. Every choice below that
the maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation), except the open questions at the end.

## What F-739 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. Rule L with NewtonOptions::line_search and the opt-in backtracking of ADR 0655 on the eight distorted meshes of ADR 0649 and on the 8^3 block at 0.25 h with three further registered seeds: punch reaction, corner bulge, tip reaction and shortening within the ADR 0636 tolerances of the F-bar Hex8 and the loaded state positive definite; a sufficient-decrease test of the mode update measured beside it on the same set-ups (does the backtracking alone then pass); the mutations of the backtracking caught by the item's own checks; the default bits unchanged.

Clauses: **c0** every limit registered here; **c1** the remedy R: rule L with `NewtonOptions::line_search(true)` and
`SolidModel::set_incompatible_mode_backtracking(true)` (ADR 0655 D1, unchanged); **c2** on the eight distorted meshes of ADR
0649 (block `4^3`, `8^3`, cantilever `10 x 2 x 2`, `20 x 4 x 4`, each at `0.1 h` and `0.25 h`, the ADR 0649 seeds); **c3** on
the `8^3` block at `0.25 h` with three further registered seeds; **c4** punch reaction and corner bulge (block), tip reaction
and shortening (cantilever) within the ADR 0636 tolerances of the F-bar Hex8 on the same mesh; **c5** the loaded state of R
positive definite; **c6** a sufficient-decrease test of the mode update measured beside R on the same set-ups, answering
whether the backtracking alone (no line search) then passes; **c7** the mutations of the backtracking caught by this item's own
checks; **c8** the default bits unchanged. The brief adds: any library change opt-in; `baseline_hashes` if a shared solver file
changes; negative controls and mutations registered before the run; a run over about two minutes in release an ignored heavy
record with a fast test on the same path.

### Readings, registered

- **c1: R.** Exactly the combination that ADR 0655 reported as passing: rule L, `NewtonOptions::new(10).threads(4)
  .line_search(true)` (ADR 0607 D1), the model's `set_incompatible_mode_backtracking(true)` (domain test only, as merged), the
  driver's defaults otherwise. No new library code is used by R.
- **c2: the eight meshes.** ADR 0649 D2 exactly: SplitMix64, seeds `0xF719_0004` (block `4^3`), `0xF719_0008` (block `8^3`),
  `0xF719_1010` (cantilever `10 x 2 x 2`), `0xF719_1020` (cantilever `20 x 4 x 4`), three draws per interior node in node order,
  every component moved by `a h (2U - 1)`, `a = 0.1` and `0.25`; the load cases of ADR 0636 readings c1, c2 (block: confined
  punch `x, y <= 0.5` on `zmax` at `u_z = -0.1`, measures `R = -sum_punch r_z / mu`, `w_c = u_z(1, 1, 1)`; cantilever `[0, 1] x
  [0, 0.2]^2`, `xmin` clamped, `xmax` `u_y = 0.3`, measures `P = sum_tip r_y / mu`, `d = -mean_tip u_x`); `neo_hookean(mu =
  1e6, kappa(nu = 0.49))`. The helpers of `si_eas_hg5.rs` and `si_eas_hg6.rs` are copied (existing tests are not edited).
- **c3: the three further seeds.** `0xF739_0001`, `0xF739_0002`, `0xF739_0003` on the `8^3` block at `a = 0.25`, the generator
  of c2. A seed whose mesh fails M0 (an element with a corner `det J <= 0`) is not a valid mesh of this family; it is replaced
  by the next seed of the sequence (`0xF739_0004`, `0xF739_0005`, ...) until three valid meshes are found, and the replacement
  is printed and recorded. The rule is fixed now, before any mesh of these seeds was built; no seed is chosen by a solve result.
- **c4: the tolerances.** ADR 0636 (and ADR 0649 reading c4): `0.15` on the coarse meshes (block `4^3`, cantilever `10 x 2 x
  2`), `0.10` on the refined ones (block `8^3`, cantilever `20 x 4 x 4`, the three seeds), for each of `e_R = |R / R_Fbar - 1|`,
  `e_w`, `e_P`, `e_d`; F-bar solved on the same mesh in the same test with the driver's defaults.
- **c5: positive definite.** SL of ADR 0649: at the converged loaded state of R, no negative eigenvalue of the free block
  `K_ff` of the driver's tangent (`count_below`, shift 0) and its lowest eigenvalue over its largest diagonal `> 0`
  (`lowest_eigenpairs`). The driver's tangent of a model with the opt-in is evaluated with the backtracking.
- **c6: the sufficient-decrease test, and "measured beside it".** A new opt-in (D1) on top of the backtracking: a step of the
  modes is also halved when its iterate does not decrease the mode residual sufficiently. Variant **S**: rule L with the
  backtracking and the decrease test, **without** the line search (the question "does the backtracking alone then pass"), on
  the same eleven set-ups. S is measured: on each set-up its outcome (converged or the error), its two measures against F-bar
  and SL are printed and set against the limits of R, and "passes" is recorded per set-up; no limit binds S's values (c6 asks
  for a measurement). What binds are the checks of D1's arithmetic (DT0, DT1a, DT1b) and the controls NCB, NCS, NC4 below.
- **c7: the mutations caught by this item's own checks.** The five mutations of ADR 0655 (MB1 to MB5, restated for the code of
  D1) and the mutations of the decrease test, each run once on a scratch copy against the default tests of the new file
  `si_eas_hg7.rs` alone; "caught" is a check of that file that misses under the mutation. ADR 0655 `mutations.log` already
  shows what each MB mutation does to R on the `8^3` block at `0.25 h` (the two together fail under MB1, MB2, MB4 and MB5) and
  to the default under MB3 (it fails at element 475, `0.8906`, not at element 476 in `[0.67, 0.68]`); here those outcomes fall
  on checked rows (R on DB4, NC1).
- **c8: default bits.** Without the new opt-in the library performs the same operations in the same order as at 5333c312 (D1),
  with the backtracking of ADR 0655 alone too; the records: the bit tests `si_eas_hg3_bits.rs`, `si_eas_hg_default_bits.rs`,
  `si_eas_default_bits.rs`, `si_fbar_default_bits.rs`; `cargo test -p rcfd-api --release --test baseline_hashes`
  (`nonlinear.rs` is a shared solver file); `si_eas_hg6.rs` (its NC2 records the backtracking of ADR 0655 bitwise against the
  default on two set-ups, and its default tests pin the ADR 0655 outcomes); and NC4 (the decrease opt-in without the
  backtracking changes no bit).

## What was read

- `AGENT_RULES.md`; the brief; ADR 0655 (whole: registration, amendments 1 and 2, Addendum 1, the roadmap section); ADR 0649
  (whole); ADR 0636 (its tolerances, through ADR 0649 reading c4); ADR 0572 and 0573 (the form of the roadmap section, through
  ADR 0655).
- `crates/rcfd-fem/src/nl_incompatible.rs` (`LOCAL_TOLERANCE`, `LOCAL_ITERATIONS`, `ModeFactor`, `mode_residual`,
  `converged`, `backtracked`, `evaluate`, `modes_and_strains_opt`, the public check functions), `model.rs` (the backtracking
  flag, setter and getter), `nonlinear.rs` (the two call sites of `nl_incompatible::evaluate`, `NewtonOptions::line_search`,
  the step records), `nl_transient.rs` (the energy record's call of `modes_and_strains_opt`), `rotating.rs`
  (`unit_centrifugal_model`).
- `crates/rcfd-fem/tests/si_eas_hg5.rs` (the cantilever set-up and helpers), `si_eas_hg6.rs` (whole); the logs named above.

## Arithmetic done before the code (disclosed)

1. **The reference values on the eight meshes** (ADR 0649 `run1-default.log`, 8 printed digits): F-bar block `4^3`: `0.1 h`
   `R = 0.38349042`, `w_c = 0.07137650`; `0.25 h` `0.38316202`, `0.07044819`. Block `8^3`: `0.34263132`, `0.06370837`;
   `0.34220247`, `0.06389947`. Cantilever `10 x 2 x 2`: `P = 6.17271412e-4`, `d = 5.24022571e-2`; `6.19924741e-4`,
   `5.23693020e-2`. Cantilever `20 x 4 x 4`: `5.72841645e-4`, `5.23430902e-2`; `5.75783694e-4`, `5.23261081e-2`. The F-bar code
   is unchanged since, so FR3 (the same values to `1e-6`) is a reproduction of a recorded run; the rounding of the printed
   digits is at most `7.8e-8` relative (`w_c` of `8^3` at `0.1 h`, 7 significant digits); ADR 0655 NC3 measured `8.9e-9`,
   `7.3e-8` on the `8^3`, `0.25 h` pair.
2. **Expected values of R.** On the seven meshes where the default rule L converged (ADR 0649) every solve took 10 increments
   with no halving; the line search changes the global iterates only where a full step does not reduce the residual (ADR 0607
   D1), and the backtracking acts only on an inverted mode iterate (ADR 0655 NC2: bitwise the default on two of these set-ups).
   The converged equilibrium is the same root within the driver's tolerance, so R's values are expected within `1e-6` of the
   default's, and the errors those of ADR 0649: `e_R`, `e_w` `0.1202`, `0.0406` (`4^3`, `0.1`); `0.1278`, `0.0870` (`4^3`,
   `0.25`); `0.0664`, `0.0349` (`8^3`, `0.1`); `e_P`, `e_d` `0.0580`, `0.0058`; `0.0694`, `0.0045` (`10 x 2 x 2`); `0.0203`,
   `0.0027`; `0.0297`, `0.0028` (`20 x 4 x 4`). On the `8^3` block at `0.25 h` R was measured by ADR 0655: `0.0750`, `0.0287`,
   lowest over max diagonal `1.81e-3`. SL of rule L on the seven: ADR 0649 (`6.63e-3`, `6.01e-3`, `1.94e-3`, `1.57e-5`,
   `1.53e-5`, `5.16e-6`, `4.54e-6`). The largest error against its limit is `0.1278 / 0.15` (block `4^3`, `0.25 h`).
3. **Expected values on the three seeds** (no probe, an estimate): the regular `8^3` block has `e_R = 0.366596 / 0.344038 - 1 =
   0.0656`, `e_w = 0.0407` (ADR 0636), the seed `0xF719_0008` at `0.25 h` `0.0750`, `0.0287`, at `0.1 h` `0.0664`, `0.0349`:
   the distortion moves `e_R` by about one point and `e_w` by about one point. Expected on a new seed: `e_R` in `0.06` to
   `0.09`, `e_w` in `0.02` to `0.05`, against `0.10`. Whether a new seed needs the remedy at all (the default may converge or
   fail at another element) is not known; the default is solved on each seed and reported. A seed's mesh can, at `0.25 h`, have
   an element that inverts or whose corner ratio is smaller than `0.076` (ADR 0649's smallest); that is the risk of c3, not
   estimated.
4. **The sufficient-decrease test (D1) by first principles.** At the modes `alpha_j` with the mode residual `r_j = r(alpha_j)`
   (the 9 mode rows of the extended force) and the mode block `K_j = dr / d alpha` of the extended tangent (exact: the condensed
   tangent built from it matches central differences of the condensed force to `9.1e-10`, ADR 0655 E2), the Newton step is
   `delta = K_j^{-1} r_j` and the trial `alpha(t) = alpha_j - t delta`. The merit `f(t) = |r(alpha(t))|_2^2 / 2` has `f'(0) =
   r_j . (-K_j delta) = -|r_j|^2 = -2 f(0)`. The Armijo condition of the backtracking line search for nonlinear equations
   (Dennis and Schnabel, Numerical Methods for Unconstrained Optimization and Nonlinear Equations, 1983, section 6.3 and
   algorithm A6.3.1, cited from memory; the derivation here is self-contained) is `f(t) <= f(0) + c t f'(0)`, that is
   **`|r(alpha(t))|_2^2 <= (1 - 2 c t) |r_j|_2^2`**, with `c = 1e-4` (the constant of that algorithm). Since `f(t) = f(0) (1 -
   2 t) + O(t^2)`, the condition holds for all `t` below some `t_0 > 0` whenever `r_j != 0`; halving from `t = 1` reaches it in
   finitely many halvings in exact arithmetic. One sequence `t = 2^-m`, `m = 0, 1, ..., MODE_HALVINGS = 20` serves both tests:
   a trial is accepted when no point inverts **and** the decrease holds. The local iteration of ADR 0608 D2 stops at `|r|_inf
   <= 1e-12 scale` (`LOCAL_TOLERANCE`), so a step is only taken when the residual is at least `1e-12` of the scale of its terms,
   four orders above the rounding of the residual (`~1e-16` of the scale times the 33 x 33 sums); the decrease test is not
   expected to fail by rounding before convergence. After 20 halvings without an accepted trial, the evaluation is refused with
   the iteration error (as an exhausted inversion halving is); the driver then halves its increment as before.
5. **Why the decrease test addresses the miss of ADR 0655** (an expectation, not a limit). ADR 0655 E1: at the trial `u_T`
   (punch `0.0892`) the domain-only backtracked iteration of element 475 converged only after 19 steps, with its mode residual
   rising from `1.6e5` to `6.7e6` (`|.|_inf`) after its first halved step and `det F` falling to `0.024` for six iterates; at
   the driver's farther iterates the same wandering exceeded the cap of 25. Each such rise is refused by the decrease test (a
   rise by a factor 40 in `|.|_inf` is a rise by at least `40 / 3 > 1` in `|.|_2`, since `|r|_inf <= |r|_2 <= 3 |r|_inf` for
   nine components), so the damped iteration is monotone in `|r|_2`. Whether that keeps it within 25 steps at the driver's far
   iterates is what S measures. Expected (not registered as a limit): S passes the `8^3` block at `0.25 h`.
6. **DT, the record at `u_T`.** `u_T = u(0.089) + 0.2 (u(0.089) - u(0.088))` with `u(.)` the converged states of the
   backtracking alone (no line search, no decrease test) at the punch `0.088` and `0.089` (10 increments each): ADR 0655 E1
   exactly (same code path: D1 leaves the domain-only backtracking unchanged, so the same bits). DT0: the record of the
   domain-only iteration at `u_T` has an accepted step with `|r_{j+1}|_2 > |r_j|_2` (from arithmetic 5, expected a ratio of at
   least `13`); DT1a: under the decrease test every accepted step satisfies `|r_{j+1}|_2^2 <= (1 - 2 c t_j) |r_j|_2^2` (by
   construction; the measure is the largest of `|r_{j+1}|_2^2 / ((1 - 2 c t_j) |r_j|_2^2)`, limit `<= 1`); DT1b: the decrease
   iteration converges at `u_T` within the cap (expected from arithmetic 4; an iteration that needs more than 25 steps is the
   miss).
7. **NC4** (first principles): the decrease flag acts only inside the backtracked branch; with the backtracking off the driver
   takes the plain branch whatever the flag, so every bit is the default's.
8. **Cost.** ADR 0649's whole file (30 solves, `8^3` and `20 x 4 x 4` among them) ran in 21.7 s; ADR 0655's four tests in 102 s
   (with replicas and long local iterations). This item: 11 set-ups times F-bar, R, S (33 solves) plus the default on 4, the
   backtracking alone on 3, NC4's 2: about 42 solves, split over five tests; expected 20 to 60 s each in release. A test over
   two minutes is made an ignored heavy record after its single run (its limits and code unchanged), the fast tests keeping
   the same code path (R on DB4 is in the fast block test).

## Design

### D1. The opt-in sufficient-decrease test of the mode update (library, `nl_incompatible.rs`, opt-in per model)

- A public enum `nl_incompatible::IncompatibleModeUpdate { Plain, Backtracking, BacktrackingWithDecrease }` with `From<bool>`
  (`false` is `Plain`, `true` is `Backtracking`). `Plain` is the local iteration of ADR 0608 D2; `Backtracking` that of ADR
  0655 D1, unchanged; `BacktrackingWithDecrease` additionally halves a trial whose residual fails arithmetic 4's test, in the
  same halving sequence (at most `MODE_HALVINGS = 20`), with `DECREASE_FRACTION = 1e-4`. After the halvings the evaluation is
  refused with the iteration error. The convergence test, the tolerance, the cap of 25 steps and the condensation at the
  converged modes are those of ADR 0608 D2.
- The internal functions `converged`, `backtracked`, `evaluate`, `modes_and_strains_opt` take the enum in place of the bool of
  ADR 0655; for `Plain` and `Backtracking` the arithmetic and its order are those of 5333c312 (the residual norm of the base
  is computed and kept but enters no value).
- Opt-in: `SolidModel::set_incompatible_mode_decrease(on: bool)` (default `false`) with the getter
  `incompatible_mode_decrease()`, and `incompatible_mode_update()` (`Plain` without the backtracking, whatever the decrease flag;
  `Backtracking` with it; `BacktrackingWithDecrease` with both). The driver's two evaluations of an Incompatible Hex8
  (`nonlinear.rs`) and the energy record (`nl_transient.rs`) pass `model.incompatible_mode_update()`; the centrifugal copy
  (`rotating.rs`) keeps the decrease flag as it keeps the backtracking.
- For checks: `nl_incompatible::mode_iteration_record(coords, material, displacement, state, context, stabilised, update)`
  returns the accepted iterates of the local iteration (per iterate: `|r|_2`, `|r|_inf`, the halvings `m` of the step that led
  to it, the scale) and the outcome (the converged modes or the error), from the same `converged` as the driver's evaluation.
- Source: arithmetic 4 (Dennis and Schnabel 1983, cited from memory; the condition derived above). Only the residual norm is
  used (no energy), so the test does not need the stabilised mode equations to be the gradient of an energy.

### D2. The test file `crates/rcfd-fem/tests/si_eas_hg7.rs`

Helpers copied from `si_eas_hg5.rs` and `si_eas_hg6.rs`. Options per solve: line search, backtracking, decrease.

- `r_and_s_on_the_four_blocks`: M0, M1, FR3, R and S on the four block meshes of ADR 0649; SL of R; NC1, NCB, NCS on the `8^3`
  block at `0.25 h`.
- `r_and_s_on_the_four_cantilevers`: the same for the cantilevers; NC4.
- `r_and_s_on_three_further_seeds`: the seed rule of c3, M0, M1, F-bar, R, S, SL of R; the default rule L reported.
- `dt_decrease_record_at_the_trial_of_element_475`: arithmetic 6.

## Checks

Every measure is required finite before it is compared (a non-finite value or a failed solve misses).

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| M0 | c2, c3 | smallest corner `det J` / centre `det J`, each of the 11 meshes | `> 0` | ADR 0649 arithmetic 1 (eight meshes); seeds unknown (c3 rule) |
| M1 | c2, c3 | moved nodes: block `4^3` 27, `8^3` 343 (and each seed), cantilever `10 x 2 x 2` 9, `20 x 4 x 4` 171 | exactly | ADR 0649 |
| FR3 | c4 | F-bar on the eight meshes against arithmetic 1 (`R`, `w_c`; `P`, `d`): `|x / x_recorded - 1|` | `<= 1e-6` each | rounding, `<= 7.8e-8` |
| RB1 | c2, c4 | R, block `4^3`, `0.1 h`: `e_R`, `e_w` | `<= 0.15` each | `0.1202`, `0.0406` |
| RB2 | c2, c4 | R, block `4^3`, `0.25 h` | `<= 0.15` each | `0.1278`, `0.0870` |
| RB3 | c2, c4 | R, block `8^3`, `0.1 h` | `<= 0.10` each | `0.0664`, `0.0349` |
| RB4 | c2, c4 | R, block `8^3`, `0.25 h` (ADR 0649 DB4) | `<= 0.10` each | `0.0750`, `0.0287` (ADR 0655) |
| RC1 | c2, c4 | R, cantilever `10 x 2 x 2`, `0.1 h`: `e_P`, `e_d` | `<= 0.15` each | `0.0580`, `0.0058` |
| RC2 | c2, c4 | R, cantilever `10 x 2 x 2`, `0.25 h` | `<= 0.15` each | `0.0694`, `0.0045` |
| RC3 | c2, c4 | R, cantilever `20 x 4 x 4`, `0.1 h` | `<= 0.10` each | `0.0203`, `0.0027` |
| RC4 | c2, c4 | R, cantilever `20 x 4 x 4`, `0.25 h` | `<= 0.10` each | `0.0297`, `0.0028` |
| RS1 to RS3 | c3, c4 | R, block `8^3`, `0.25 h`, the three seeds: `e_R`, `e_w` | `<= 0.10` each | `0.06` to `0.09`, `0.02` to `0.05` (arithmetic 3) |
| SLR | c5 | R at its loaded state, each of the 11 set-ups: negative eigenvalues of `K_ff`; lowest / largest diagonal | `0`; `> 0` | 0; arithmetic 2 |
| S | c6 | the decrease variant (backtracking + decrease, no line search) on the 11 set-ups: outcome, the two errors, SL, "passes" against R's limits | measured (no limit) | passes on DB4 (arithmetic 5); elsewhere as R |
| DT0 | c6, c7 | at `u_T`, element 475, the domain-only record: largest `|r_{j+1}|_2 / |r_j|_2` over the accepted steps | `> 1` | `>= 13` (arithmetic 5) |
| DT1a | c6, c7 | at `u_T`, the decrease record: largest `|r_{j+1}|_2^2 / ((1 - 2 c t_j) |r_j|_2^2)` | `<= 1` | `< 1` by construction |
| DT1b | c6 | at `u_T`, the decrease record converges within 25 steps | yes | yes (arithmetic 4) |
| NC1 | c7, c8 | the default (no line search, no backtracking) rule L on the `8^3` block at `0.25 h` | fails, element 476, at a load factor in `[0.67, 0.68]` | ADR 0649, ADR 0655 |
| NCB | c6 | the backtracking alone (no line search, no decrease) on the same set-up | fails | ADR 0655: element 475, `0.890625` |
| NCS | c6, c7 | S on the same set-up against NCB: the outcome differs (S converges, or fails with another error text) | differs | S converges |
| NC4 | c8 | the decrease flag on with the backtracking off, rule L, cantilever `10 x 2 x 2` at `0.25 h`, against the default: displacement and reaction values differing in bits | `0` | arithmetic 7 |
| B6 | c8 | `si_eas_hg3_bits.rs`, `si_eas_hg_default_bits.rs`, `si_eas_default_bits.rs`, `si_fbar_default_bits.rs` | pass | |
| S5 | c8 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | |
| S6 | | `cargo fmt -p rcfd-fem --check`, `cargo clippy -p rcfd-fem --all-targets -- -D warnings` | clean | |
| S7 | c8 | the existing tests of the changed lines: `cargo test -p rcfd-fem --release --lib`, `--test si_eas_hg6` (ADR 0655 incl. its NC2), `si_eas_hg5`, `si_eas_hg3`, `si_hex8x`, `si_eas`, `si_eas_hg`, `si_eas_hg2` (the transient energy record and the rotating copy) | pass | |

Reported without a limit: S on every set-up (all of it); the default rule L on the three seeds (outcome, error); the
increments, halvings and searched steps of every solve; F-bar's SL; R against the default rule L where both converge (`|R_R /
R_default - 1|`); the full records of DT; the converged modes of the two records at `u_T` against each other; S with the
backtracking and the decrease on the cantilever `10 x 2 x 2` at `0.25 h` against the default (bits differing, reported: the
decrease test may halve a non-monotone full step where nothing inverts).

### Negative controls

| Id | Control | Expected |
|---|---|---|
| NC1 | the default on DB4 (checked above) | fails as in ADR 0649: the set-up reproduces the miss, and the backtracking is off by default |
| NCB | the backtracking alone on DB4 (checked above) | fails as in ADR 0655: the question of c6 is not already answered without the decrease test |
| NCS | the decrease variant against NCB (checked above) | differs: the decrease opt-in reaches the driver |
| NC4 | the decrease flag without the backtracking (checked above) | bitwise the default |
| DT0 | the domain-only record at `u_T` (checked above) | a rise: `u_T` is a state at which the decrease test has something to refuse |

### Planned mutations (on a scratch copy, each run once against the default tests of `si_eas_hg7.rs` alone)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MB1 | the driver ignores the backtracking (passes `IncompatibleModeUpdate::Plain` at both call sites in `nonlinear.rs`) | RB4, SLR on DB4 (R fails, ADR 0655 `mutations.log`); NCS; DT (no `u_T`: the backtracking-alone solves fail at `0.675`) |
| MB2 | the halved step not halved (`t = 2^-m` replaced by 1 in `backtracked`) | RB4, SLR on DB4; DT |
| MB3 | the backtracking always on (the driver passes `Backtracking` when the model's update is `Plain`) | NC1 (the default fails at element 475, ADR 0655) |
| MB4 | at the first inverted iterate the previous iterate returned as converged | RB4, SLR on DB4 (ADR 0655: the two together fail) |
| MB5 | the halved step taken from the inverted iterate instead of from `alpha_j` | RB4, SLR on DB4 (ADR 0655: the two together fail) |
| MS1 | the driver ignores the decrease (`incompatible_mode_update()` returns `Backtracking` with both flags on) | NCS (S becomes NCB) |
| MS2 | the decrease test removed in `backtracked` (the `BacktrackingWithDecrease` branch accepts any non-inverted trial) | DT1a (the record shows DT0's rise); NCS |
| MS3 | the decrease measured against the residual at `alpha = 0` of the evaluation instead of `r_j` | uncertain: DT1a only if a rise stays below the first residual; registered as possibly not caught |

## Decision rule

- **done**: M0, M1, FR3, RB1 to RB4, RC1 to RC4, RS1 to RS3, SLR, DT0, DT1a, DT1b, NC1, NCB, NCS, NC4, B6, S5, S6, S7 met in
  their single registered run; S measured on all eleven set-ups (its outcome recorded); MB1 to MB5, MS1, MS2 each caught by at
  least one check of `si_eas_hg7.rs`; no amendment. (MS3 is recorded either way.)
- **partial**: B6, S5, NC4 met, but a row of R missed on some set-up, or another check missed, or one of MB1 to MB5, MS1, MS2
  not caught, or a check met only after an amendment.
- **todo**: B6, S5 or NC4 fails (a default bit changed), or R cannot be run.

## Limits (not covered)

- One material and Poisson ratio, one load level per case, the meshes of ADR 0649 and three more seeds of one of them; no
  statistics beyond those eleven set-ups; the boundary nodes not moved.
- R uses the backtracking with the domain test only (as merged); the decrease test is measured beside it (S) and S's values
  bind no limit; the combination of the line search with the decrease test is not run.
- No tangent check (T-FD) of the decrease variant: the decrease test changes the path to the root of the mode equations, not
  the condensation at it (ADR 0608 D2), which ADR 0655 E2 checked at `9.1e-10`.
- The element stays without state (every evaluation from `alpha = 0`); `hex8_force_and_tangent_at` has neither opt-in.
- The trial `u_T` of DT stands in for the driver's failing iterate (ADR 0655 E1), which the driver does not return.
- The default stays without the backtracking and the decrease test (open question 2).

## Open questions (taken or open)

1. **The design of the decrease opt-in**: a second per-model flag acting only with the backtracking, the Armijo condition on the
   residual norm with `c = 1e-4`, one halving sequence of 20 for both tests, a refusal after it: taken on the maintainer's
   standing instruction of 2026-11-11 (take the recommendation).
2. **Default.** Making R (or S) the default would change default results (where a default solve fails today, and the bits of a
   failed attempt that later converges): open for the maintainer, not taken.

## Amendment 1 (2026-11-16, written after the single registered run and before any further run)

The registered run (test commit d9289b11, log `run1-default.log` in the scratch folder; the four tests one after the other,
60 s, 20 s, 77 s, 17 s) met every check of `r_and_s_on_the_four_blocks`, `r_and_s_on_the_four_cantilevers` and
`dt_decrease_record_at_the_trial_of_element_475`, and in `r_and_s_on_three_further_seeds` M0, M1 on all three seeds and RS1,
SLR on the first (`0xF739_0001`; no seed was replaced). **R fails on the seeds `0xF739_0002` and `0xF739_0003`**: `element
420: the modes of an Incompatible Hex8 did not converge in 25 local iterations (relative mode residual 3.05e-1)`, load factor
`0.9375` to `0.940625`, and `element 476: ... did not converge in 25 local iterations (relative mode residual 3.73e-1)`, load
factor `0.85625` to `0.859375`, each after 5 halvings. RS2, RS3 and SLR on those two seeds are non-finite and missed. S fails
on the same two seeds at the same elements with the decrease test exhausted: `|r|_2 = 167.07343` against `167.07289` (element
420, load factor `0.94375`) and `324.61004` against `324.60719` (element 476, `0.84375`) after 20 halvings: at `t = 2^-20` the
residual still rises, by `3.2e-6` and `8.8e-6` relative, where an exact Newton direction predicts a fall by about `t = 9.5e-7`
(arithmetic 4).

Records, as the house rules require: the test `r_and_s_on_three_further_seeds` is kept unchanged as the ignored record
`r_and_s_on_three_further_seeds_known_miss` (the limits intact, the measured values in its ignore text); no limit is changed.
The registered rule gives **partial** whatever follows.

### A1.1 The met rows as a default test

`r_and_s_on_three_further_seeds_met_rows`: the same code and limits for every row of the registered test except RS2, RS3 and
SLR on the seeds `0xF739_0002` and `0xF739_0003`, which it prints instead of checking. It is a regression test of rows already
met (as ADR 0649 A1.1, ADR 0655 A1.1), not a new registered run.

### A1.2 Diagnosis (registered now, no limit, run once, ignored record `a1_diagnosis_seeds_2_and_3`)

Question: why the local mode iteration of element 420 (seed 2) and element 476 (seed 3) fails, with and without the decrease
test; in particular whether the Newton direction of the modes is a descent direction of `|r|_2` there (arithmetic 4 assumes the
mode block `K_aa` is the exact Jacobian of the mode residual). Runs, each once, rule L on the `8^3` block at `0.25 h`:

- **E1**: R solved to the punch at `0.0930` and `0.0937` (seed 2) and at `0.0850` and `0.0855` (seed 3), 10 increments each;
  the trial `u_T = u_b + (p_T - p_b) / (p_b - p_a) (u_b - u_a)` with `p_T = 0.0939` (seed 2) and `0.0858` (seed 3), inside the
  failing increments. At `u_T` for the failing element: the library's records (`mode_iteration_record`) with `Backtracking` and
  `BacktrackingWithDecrease` (iterates, halvings, outcome).
- **E2**: at `u_T`, a replica of the local iteration in the test (the extended force and tangent by
  `enhanced_force_and_tangent_with`, the 9 x 9 mode block solved by Gaussian elimination with partial pivoting, the decrease and
  inversion halvings of D1, 25 steps), printing per iterate `|r|_2`, `|delta|_inf / |alpha|_inf`, the smallest and largest
  eigenvalue of the symmetric part of `K_aa` and its asymmetry `max |K_aa - K_aa^T| / max |K_aa|`, and the consistency `max |K_aa
  - K_fd| / max |K_aa|` of `K_aa` against central differences of the mode residual in `alpha` (step `1e-9`), and the measured
  slope `(|r(alpha - t delta)|_2 - |r|_2) / (t |r|_2)` at `t = 1e-4, 1e-6, 1e-8` (`-1` for an exact Newton direction).
- **E3**: the same replica measures at the converged modes of R's state `u_b` (where the iteration converges).
- If a solve of E1 fails, it is printed and the replica is run at the last converged state instead.

Interpretation registered: a consistency measure `<= 1e-5` with a slope near `-1` at small `t` says the direction is a descent
direction and the stall is the nonlinearity of a step that is large against the modes (`|delta| / |alpha|` large; a
near-singular `K_aa`, smallest eigenvalue near zero or negative); a consistency measure `>= 1e-3` or a slope `>= 0` at `t = 1e-8`
says the mode block is not the Jacobian of the mode residual there, so neither a Newton iteration nor the decrease test of D1
can be expected to converge; a negative eigenvalue of the symmetric part says the mode problem is not a minimum there (no
energy descent). The decision rule is unchanged.

## Amendment 2 (2026-11-16, written after the diagnosis of amendment 1 and before the mutation runs)

The mutations were registered against the default tests of `si_eas_hg7.rs`. They are run, each once, on a scratch copy of
**commit 8da3a0f6** (the amendment 1 tests: the registered seed test is an ignored record there, its met rows a default test),
so that every default test passes unmutated; "caught" is a default test of that copy that fails under the mutation (a check
missed). No limit and no test is changed.

## Addendum 1: results (2026-11-16)

The registered run was made once on the test commit d9289b11 (`run1-default.log` in the scratch folder; 60 s, 20 s, 77 s,
17 s, the four tests one after the other); the met rows of A1.1 once (`run2-met-rows.log`, 61 s, every checked row identical
to the registered run); the diagnosis of A1.2 once (`a1-diagnosis.log`); the mutations once each on a scratch copy of
8da3a0f6 (amendment 2; `mutations.log`, `mut-M*.log`); B6, S5, S7 and `docs_site` once (`b6.log`, `s5-baseline.log`,
`s7-lib.log`, `s7-tests.log`, `docs-site.log`). No limit, set-up, seed or measure was changed after a run; no seed was replaced
by the rule of c3 (all three passed M0).

### Results

| Id | Limit | Measured | Met |
|---|---|---|---|
| M0 eight meshes | `> 0` | block `0.6977`, `0.2179`, `0.6648`, `0.0762`; cantilever `0.8557`, `0.6113`, `0.6611`, `0.0773` | yes |
| M0 seeds `0xF739_0001` to `_0003` | `> 0` | `0.0680`, `0.0957`, `0.1788` | yes |
| M1 | `27`, `343`, `9`, `171`; `343` per seed | all as registered | yes |
| FR3 (16 values) | `<= 1e-6` | largest `7.3e-8` (`w_c`, `8^3`, `0.25 h`) | yes |
| RB1 `e_R`, `e_w` | `<= 0.15` | `0.1202`, `0.0406` | yes |
| RB2 | `<= 0.15` | `0.1278`, `0.0870` | yes |
| RB3 | `<= 0.10` | `0.0664`, `0.0349` | yes |
| RB4 (ADR 0649 DB4) | `<= 0.10` | `0.0750`, `0.0287` (11 increments, 2 halved, 3 searched steps) | yes |
| RC1 `e_P`, `e_d` | `<= 0.15` | `0.0580`, `0.0058` | yes |
| RC2 | `<= 0.15` | `0.0694`, `0.0045` | yes |
| RC3 | `<= 0.10` | `0.0203`, `0.0027` | yes |
| RC4 | `<= 0.10` | `0.0297`, `0.0028` | yes |
| RS1 (`0xF739_0001`) | `<= 0.10` | `0.0743`, `0.0495` | yes |
| RS2 (`0xF739_0002`) | `<= 0.10` | **NaN: R fails** (element 420, modes not converged in 25 local iterations, load factor `0.9375`) | **no** (known miss) |
| RS3 (`0xF739_0003`) | `<= 0.10` | **NaN: R fails** (element 476, the same, load factor `0.85625`) | **no** (known miss) |
| SLR, negative count | `0` | `0` on nine set-ups; NaN on RS2, RS3 | **no** on two |
| SLR, lowest / max diagonal | `> 0` | block `6.63e-3`, `6.01e-3`, `1.94e-3`, `1.81e-3`; cantilever `1.57e-5`, `1.53e-5`, `5.16e-6`, `4.54e-6`; seed 1 `1.74e-3`; NaN on RS2, RS3 | **no** on two |
| S (measured) | no limit | **passes** on the eight meshes (DB4 included: `0.0750`, `0.0287`, 10 increments, no halving, positive definite) and on seed 1; **fails** on seeds 2 and 3 (the decrease test exhausted at elements 420 and 476) | measured |
| DT0 | `> 1` | `263` (domain-only record at `u_T`: `|r|_2` from `4.0e4` to `1.06e7` in one step; 20 iterates) | yes |
| DT1a | `<= 1` | `0.524` (decrease record: 7 iterates, 3 halvings, monotone) | yes |
| DT1b | converged | converged, modes within `2.5e-13` of the domain-only record's | yes |
| NC1 | fails, element 476, `[0.67, 0.68]` | fails, element 476, `0.675` | yes |
| NCB | fails | fails, element 475, `0.890625` (as ADR 0655) | yes |
| NCS | differs | S converges where NCB fails | yes |
| NC4 | `0` bits | `0` (and S against the default on that set-up: `0`, reported) | yes |
| B6 | pass | the four bit records pass | yes |
| S5 | pass | 3 passed, 1 ignored | yes |
| S6 | clean | `cargo fmt -p rcfd-fem --check`, `cargo clippy -p rcfd-fem --all-targets -- -D warnings` clean | yes |
| S7 | pass | `--lib` 14; `si_eas_hg6` 4, `si_eas_hg5` 3, `si_eas_hg3` 4, `si_hex8x` 8, `si_eas` 8, `si_eas_hg` 8, `si_eas_hg2` 3 passed | yes |

Reported (no limit). R reproduces the default rule L bit for bit where the default converges (`0.0` relative on the seven
meshes: neither the line search nor the backtracking acts there). F-bar on the seeds: `R = 0.34387169`, `0.34915749`,
`0.34360196`, SL positive definite. **The default rule L fails on all three new seeds** (seed 1 element 475 inverted at
`0.809`; seed 2 element 420 inverted at `0.9375`; seed 3 element 476, modes not converged, at `0.856`): the punch edge of the
`8^3` block at `0.25 h` defeats the plain element on four of four seeds; R rescues two of them (seed `0xF719_0008` and seed 1).

**Diagnosis A1.2.** E1: R converges to the punch `0.0930`, `0.0937` (seed 2) and `0.0850`, `0.0855` (seed 3). At the trial
`u_T` of seed 2 (punch `0.0939`) both library records converge (domain-only in 19 iterates with a rise to `8.1e5`, the
decrease variant in 8): `u_T` is not the driver's failing iterate (as in ADR 0655 E1). At the trial of seed 3 (punch `0.0858`)
**both fail**: the domain-only record wanders (`|r|_2` up to `6.5e7`) and stops at the cap; the decrease record falls
monotonically from `1.06e5` to `223.1` in six iterates and then stalls there (halvings `2, 9, 13, 19`, then none accepted). E2
(the replica, which follows the decrease record): the mode block is the exact Jacobian of the mode residual at every iterate
(consistency `4e-10` to `1.1e-9`, asymmetry `1e-16`), and the Newton direction is a descent direction at small `t` (slope
`-1.0000` at `t = 1e-8` at every iterate), so the stall is not an inconsistent tangent. What changes along the stall is the
mode block: its symmetric part becomes **indefinite or nearly singular** (smallest eigenvalue `-3.8e4` at iterate 3, then
`+3.6e4`, `-3.3e3`, `-6.8e2`, `+1.2e2`, `+48` against a largest of `5.2e7`) while `|delta|_inf / |alpha|_inf` grows from
`0.14` to `195`, and the slope at `t = 1e-4` turns from `-1` to `+25` and `+163`: the iterate is drawn to a point where `|r|_2`
has a local minimum (`223`, far from zero) with a near-singular `K_aa`, the classical stagnation of a Newton method with a
merit-function line search. E3: at the converged modes of the states of R before the failing increments the mode block is
positive definite (smallest `1.2e5` and `1.95e5`) and consistent (`4.7e-10`, `4.8e-10`). By the interpretation registered in
A1.2: the direction is a descent direction and the stall is a step large against the modes with a near-singular, at times
indefinite, `K_aa`; along the way from `alpha = 0` to the root at such a state the mode problem of the stabilised element is not
a minimum. Neither backtracking (domain only, or with the decrease test) can lead the iteration from `alpha = 0` around that
region; a start nearer the root (the committed modes, ADR 0655 question 1 (c)) or a trust region on the modes would be needed.

### Deviations

- **RS2, RS3, SLR on two seeds** (amendment 1): R fails there; the known-miss record is
  `r_and_s_on_three_further_seeds_known_miss`, the met rows run as `r_and_s_on_three_further_seeds_met_rows`.
- **Arithmetic 3** expected R to pass a new seed; the punch edge of this mesh family is harder than one seed showed: the default
  fails on every seed tried, R on two of four.
- **Arithmetic 4** expected the decrease test not to fail before convergence; it does fail at a stagnation point with a
  near-singular, indefinite mode block (E2), which arithmetic 4 assumed away.
- **Arithmetic 5** expected S to pass DB4: it does, and more simply than R (10 increments, no halving, no line search; the same
  `R`, `w_c` to 8 digits).
- **Mutations** were run on the amendment 1 tests (amendment 2).

### Mutations (scratch copy of 8da3a0f6, the default tests of `si_eas_hg7.rs`, each run once; amendment 2)

| Id | Caught | By |
|---|---|---|
| MB1 (the driver ignores the backtracking) | **yes** | RB4, SLR on DB4 (R fails), RS1, SLR on seed 1, NCS, DT0, DT1a, DT1b (the backtracking-alone solve to `0.088` fails at `0.747`) |
| MB2 (the step not halved) | **yes** | the same rows as MB1 except NCS |
| MB3 (the backtracking always on) | **yes** | NC1 (the default fails at element 475, `0.8906`) |
| MB4 (the previous iterate returned as converged) | **yes** | RB4, SLR on DB4, RS1, SLR on seed 1, NCS, DT0, DT1a, DT1b (a singular global tangent at `0.747`) |
| MB5 (the halving from the inverted iterate) | **yes** | RB4, SLR on DB4, RS1, SLR on seed 1, DT0, DT1a, DT1b |
| MS1 (the driver ignores the decrease test) | **yes** | NCS |
| MS2 (the decrease test removed in `backtracked`) | **yes** | DT1a (`6.9e4`), NCS |
| MS3 (the decrease against the first residual) | **no** (registered as possibly not caught) | every check passes; the DT records are unchanged (the only halved step there is the first). Reported: under MS3 S **passes** seed 3 (`e_R = 0.0787`, `e_w = 0.0490`, positive definite) and still fails seed 2 at a later load factor (`0.972`) |

Seven of eight caught: all five mutations of the backtracking of ADR 0655 by this item's own checks (clause c7), and the two
of the decrease test registered as catchable. MS3 is a weaker decrease test (not monotone with respect to the last iterate),
and it happens to pass one more seed: a pointer for a follow-up, not a result of this item.

### Not run

Nothing registered was skipped. The whole rcfd-fem suite was not run (the daily gate runs it, rule 7): `si_eas_hg7.rs` (its
default tests and its two ignored records each once), the lib tests, `si_eas_hg6`, `si_eas_hg5`, `si_eas_hg3`, `si_hex8x`,
`si_eas`, `si_eas_hg`, `si_eas_hg2`, B6, S5, `docs_site`. No test is a heavy record (the longest default test takes 61 s).

### Decision

**partial**, by the registered rule: B6, S5, NC4 met (no default bit changed: the bit records, the baseline hashes, the ADR
0655 tests and NC4), and M0, M1, FR3, RB1 to RB4, RC1 to RC4, RS1, DT0, DT1a, DT1b, NC1, NCB, NCS, S6, S7 met; S measured on
all eleven set-ups; every mutation of the backtracking caught by this item's own checks (MB1 to MB5) and MS1, MS2 caught; but R
fails on two of the three further seeds (RS2, RS3 and their SLR missed), so R is not a remedy for the punch edge of this mesh
family in general. On the eight meshes of ADR 0649 R meets every row (DB4 included: `0.0750`, `0.0287`, positive definite),
and the sufficient-decrease test alone (S, no line search) passes all eight as well and the first new seed: with it the
backtracking alone passes, which answers the question of c6 on the eight meshes.

### Remaining questions

1. **The two failing seeds.** Options: (a) a warm start of the local mode iteration from the converged modes of the committed
   state (ADR 0655 question 1 (c); adds element state, a different design); (b) a trust region (Levenberg-Marquardt) on the
   mode step instead of the backtracking, which handles the indefinite, near-singular `K_aa` of E2; (c) a non-monotone decrease
   test (MS3 passed seed 3; not derived here); (d) record it as a limit of rule L at a singular punch edge on strongly
   distorted meshes and close. Recommendation: (a), measured beside (b), on the same eleven set-ups; (a) starts inside the
   basin of the root, which E3 shows is well conditioned. Proposed below.
2. **Default.** R and S stay opt-in; making either the default would change default results where a default solve fails
   today. Open for the maintainer (not taken).
3. **Which opt-in to recommend** on distorted meshes: S passed every set-up R passed, with fewer increments on DB4 and without
   the line search. Recommendation: document S (the backtracking with the decrease test) as the preferred opt-in for the
   incompatible element at finite strain, R as the fallback; no default change.

## For the lead: roadmap entries

> Proposed status: partial.

> Structural solver, intermediate tier. Follow-up of F-732 (ADR 0655). PARTIAL. ADR 0658, 2026-11-16: rule L of the stabilised incompatible-modes Hex8 with the driver's line search and the opt-in backtracking of the mode update of ADR 0655 (the remedy R, now registered) on the eight distorted meshes of ADR 0649 and on the 8^3 block at 0.25 h with three further seeds (0xF739_0001 to _0003), against the F-bar Hex8 on the same meshes with the ADR 0636 tolerances. Met on the eight meshes: punch reaction within 12.0, 12.8, 6.6 and 7.5 per cent (DB4, the ADR 0649 known miss, now solved), corner bulge within 8.7 per cent, tip reaction within 5.8, 6.9, 2.0 and 3.0 per cent, shortening within 0.6 per cent, loaded state positive definite; and on the first new seed (7.4 and 5.0 per cent). Missed on two of the three new seeds: R fails at 94 and 86 per cent of the load (elements 420 and 476, the local mode iteration not converged in 25 steps); diagnosed: the mode block is the exact Jacobian and the Newton direction a descent direction, but on the way from alpha = 0 the mode block turns indefinite and near singular (smallest eigenvalue from -3.8e4 to +48 against 5.2e7) and the iterate stagnates at a local minimum of the mode residual far from zero; the default rule L fails on all three new seeds. New opt-in SolidModel::set_incompatible_mode_decrease (with the backtracking, a step is also halved without the Armijo decrease of the mode residual, c = 1e-4): measured beside R without the line search, it passes all eight meshes (DB4 in 10 increments without halving) and the first new seed and fails on the same two seeds; at the trial state of ADR 0655 element 475 its record is monotone where the backtracking alone rises by a factor 263. No default bit changed (the bit records of ADR 0506, 0608, 0614, 0630, the ADR 0655 tests and the rcfd-api baseline hashes pass; the decrease flag without the backtracking is bitwise the default). All five mutations of the ADR 0655 backtracking and two of three of the decrease test caught by the item's own checks. Not covered: a warm start or a trust region for the modes, other materials, statistics beyond eleven set-ups.

> F-732 (note, ADR 0658, 2026-11-16): the combination reported by ADR 0655 is registered and checked in F-739: it passes the eight distorted meshes of ADR 0649 (DB4 at 7.5 and 2.9 per cent, positive definite) and one of three further seeds of the 8^3 block at 0.25 h; the mutations of the backtracking that escaped ADR 0655 are all caught there.

> Follow-up proposed: A warm start of the incompatible-mode iteration from the committed modes, measured beside a trust region on the mode step, for the seeds of ADR 0658 where the line search with the backtracking fails. Deps F-739. Acceptance: Every limit not stated here is registered in the ADR before the run. Rule L on the eleven set-ups of ADR 0658 (the eight distorted meshes of ADR 0649 and the 8^3 block at 0.25 h with seeds 0xF739_0001 to _0003) and three more registered seeds: an opt-in warm start of the local mode iteration from the modes of the committed state (element state added only behind the opt-in), and a trust-region (Levenberg-Marquardt) mode step measured beside it on the same set-ups; punch reaction, corner bulge, tip reaction and shortening within the ADR 0636 tolerances of the F-bar Hex8 and the loaded state positive definite for the registered remedy; the mutations of the new update caught by the item's own checks; the default bits unchanged.
