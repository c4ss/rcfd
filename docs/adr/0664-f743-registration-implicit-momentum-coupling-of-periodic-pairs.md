# ADR 0664: F-743 registration: implicit momentum coupling of periodic pairs

Status: **REGISTERED 2026-11-16, written before any code of F-743 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-743 was written and no run of any kind was made; the numbers quoted
are those measured and printed in ADR 0661 (and, through it, ADR 0641). Results go in the addenda at the end; nothing above the
addenda is changed after a run.

Roadmap item F-743 (deps F-725), proposed by [ADR 0661](0661-f725-registration-sector-against-full-annulus-with-a-non-axisymmetric-inlet.md)
remaining question 2 (the lag). Parents: ADR 0661 (F-725: the lockstep of the sector SS against the full annulus P from the start
`u0` consistent with the inlet, the variants `V(K, N)`, the lag identified as nearly the whole difference),
[ADR 0641](0641-f721-registration-periodic-sliding-at-coinciding-nodes-without-the-parts-symmetry.md) (F-721: the lockstep and its
step-1 difference `3.494e-2` from the start `(r_i/r) x_hat`), [ADR 0629](0629-f304-registration-planar-and-sector-sliding-interfaces.md)
(F-304: the periodic join, the seam and wrap pairs), [ADR 0046](0046-wave26a-transient-defects.md) (D1: the translationally periodic
partner made implicit in the transient momentum matrix; "rotationally periodic patches keep the lagged partner (named
limitation)"). Number 0664 given by the lead. Branch `f743-work` from main c4fa0c8c (F-725 merged), worktree `w49i-work/f743`,
scratch `w49i-work/f743-scratch`.

House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here; each registered run made once; existing tests not edited;
defaults bit-identical; no edit of `baseline_hashes.rs`, of a hash table or of a golden; no new dependency, no unsafe code;
clean-room; the roadmap files are not edited. Every choice the maintainer would normally be asked about is taken on the
maintainer's standing instruction of 2026-11-11 (take the recommendation), except where listed as open.

Clean-room: no source outside the repository was read; the design is the linear algebra of the repository's own periodic face
treatment (below), derived from first principles; the references are the repository's own runs (ADR 0661, ADR 0641).

## What F-743 asks

The acceptance, binding (from the lead's brief):

> An opt-in setting couples the two faces of every periodic pair implicitly in the momentum matrix (the partner's new value, as the pressure equation already does), with the defaults bit-identical; on ADR 0661's lockstep the sector's largest difference from the full annulus with one outer iteration is within a registered limit of ADR 0661's six-outer-iteration variant V(6, 6) (6.0e-3) on every step, and ADR 0641's step-1 difference falls below a registered limit. Every limit not stated here is registered in the ADR before the run.

Clauses:

- **a1** an opt-in setting couples both faces of every periodic pair implicitly in the momentum matrix (the partner's new
  value);
- **a2** the defaults are bit-identical;
- **a3** on ADR 0661's lockstep (from `u0`, 400 steps) with one outer iteration and the setting on, the sector's largest
  difference from the annulus is within a registered limit of `V(6, 6)`'s on every step;
- **a4** ADR 0641's step-1 difference (the same lockstep from `(r_i/r) x_hat`, one outer iteration) with the setting on falls
  below a registered limit.

**Reading of a3 (registered).** "Within a registered limit of V(6, 6)" is read per step: with `G_V(n) = max_c |delta_V(n)_c| /
U_in` (the notation of ADR 0661, `delta_V(n)` the field `u_C - u_A` of variant `V` at step `n`), the check is `|G_*(n) -
G_(6,6)(n)| <= eps` on every one of the 400 steps, `*` the new variant `V*(1, 6)` (one outer iteration, six non-orthogonal
correctors, the setting on in both solvers). The stronger field form `max_c |delta_*(n) - delta_(6,6)(n)| / U_in <= eps` (which
implies the scalar form by the triangle inequality) is registered beside it as a supporting check with the same limit.

## What the code read shows (c4fa0c8c, read only)

1. `bc::Periodic`: on a coupled face `a = w`, `b = (1 - w) q_k`, `c = -1/span`, `d = q_k/span`, with `w = dn_q/span`,
   `span = dn + dn_q`, `q` the partner cell's velocity from the current field, **rotated** by the patch rotation
   (`field.rs` `eval_coeffs`: `q = R u(own[g])`, `R = geom.periodic_rot[pi]`). So the face value of component `k` depends on
   every component `j` of the partner through `R_kj`.
2. `diffusion::laplacian`: a boundary face adds `-gamma |S| c` to the owner's diagonal and `gamma |S| d` to its right-hand
   side. `convection::convection` on a coupled face (`0 < a < 1`): outflow `phi >= 0` adds `phi` to the diagonal; inflow
   subtracts `phi q_k` from the right-hand side (upwind between owner and partner), the higher-order part a deferred
   correction from the current iterate (as on internal faces, where it is deferred from the same iterate).
3. `pimple.rs` `PeriodicLinks` (ADR 0046 D1): for **translational** patches the coefficient `coef = gamma |S| c + min(phi, 0)`
   is moved back into the matrix as the off-diagonal entry `(own[f], own[g])`, and `coef q_k^iter(own[g])` added to `b` to
   remove the lagged term; the solve is BiCGStab with ILU(0) on the CSR system, the residual and the `H` operator of the
   correctors include the link, relaxation counts the link in the dominance sum, the consistent correctors count it in `H1`.
   Rotational patches are skipped (`geom.periodic_rot[pi].is_some()`): their partner stays lagged.
4. In the default step the three components are solved one after the other inside the component loop, and
   `self.u.boundary_coeffs(&g, k)` is evaluated from `self.u.values` after the earlier components' solves: on a rotational
   patch component `y` sees the partner's **new** `x` (through `R_yx`) and component `x` the old `y`. Part of the lag is so
   Gauss-Seidel-like; the rest is the partner's old value of the own component.
5. The joined sector mesh of F-304 has rotational seam and wrap patches (`periodic_rot` set, a 60-degree pitch about z);
   the annulus P has no periodic patch. ADR 0661: `M(L) = 2.011e-2` (the lag) of `G = 1.98e-2`; `V(6, 6)` leaves `5.99e-3`
   (Q4 at the wrap pairs), `V(6, 0)` `1.48e-4`; J (ADR 0641's start, 16 steps): `V(1,6)` `3.494135e-2` at step 1, `V(6,6)`
   `1.91e-3` (largest over 16 steps), `V(10,6)` within `3.45e-7` of `V(6,6)`, `M(L)` over J = `3.5e-2` (from LC's printed
   limit `1.75e-3 = 0.05 M(L)`). Elapsed: J (five variants) 81 s, I1 (four variants, 400 steps) 1092 s.

## Design (derived)

**The implicit pair.** On a periodic face `f` of owner `r` with partner face `g` of owner `c` and patch rotation `R`, the
momentum equation of component `k` of cell `r` has, after the assembly of read 2, the partner term `-coef (R u_c)_k` on its
right-hand side (`coef = gamma_f |S_f| c_f + min(phi_f, 0)`, negative), evaluated with the iterate. The annulus has, on the
internal face between the same two cells, the neighbour **implicitly**. The implicit pair moves the partner term to the matrix:

`a_P,k u_r,k + sum_nb a_nb u_nb,k + sum_(f, g) coef_f sum_j R_kj u_c,j = b_k + sum_(f, g) coef_f sum_j R_kj u_c,j^iter - (lagged term)`

that is, the entries `coef_f R_kj` at row `(k, r)`, column `(j, c)` for every active component `j`, and `coef_f sum_(j active)
R_kj u_c,j^iter` added to `b_k` (which cancels the lagged term exactly, since the assembly of every component is made from the
same iterate, below). A component `j` that is not active (`g.active[j]` false) keeps its lagged term on the right-hand side.
For `R = I` this is ADR 0046's link. Because `R` mixes components, the three component equations become one block system of
size `n_a n` (`n_a` active components); it is solved by BiCGStab with ILU(0) (as ADR 0046's links) to the momentum solver's
controls. Its solution is the fixed point of the lag iteration (re-assembling the partner value from the latest iterate with
everything else frozen), so one outer iteration takes the partner's new value, as the pressure equation already does with its
`extras` (`pressure.rs`), and as the annulus does on its internal faces.

**Where it acts.** A new field `PimpleSettings::rotational_periodic_implicit: bool`, default `false`. With it set, in
`PimpleSolver::step_with`'s momentum predictor of every outer iteration (so also in `step_ale`, `step_sliding`,
`step_sliding_periodic`, which run that step):

- the rotational links are built per component as `PeriodicLinks::build` builds the translational ones (same face filter:
  coupled patch, not empty, not self-partnered, `0 < a < 1`; the same `coef`), each with its patch rotation;
- if at least one rotational link exists, the component solves are deferred: the three systems are assembled from the same
  iterate (no component is written into `self.u` before all three are assembled), the right-hand sides fixed as above, and one
  block solve made; translational links (ADR 0046) go into the same block system as their own component's entries;
- the reported momentum residual of the first outer iteration is the normalised residual of the block system;
- the `H` operator of the correctors subtracts `coef sum_(j active) R_kj u_c,j` with the current velocity (so `HbyA` is that of
  the block matrix);
- relaxation (`alpha_u < 1`) counts `|coef| sum_(j active) |R_kj|` in the dominance sum of row `(k, r)`; the consistent correctors
  with `periodic` count the same magnitude in `H1`;
- with no rotational link (a mesh without rotational periodic patches) the step is the default step exactly: the opt-in is a
  no-op there (translational pairs are implicit since ADR 0046).

Not changed: the pressure equation (already implicit), `boundary_hby_a` (already the current partner value), the lag map and
the adaptive corrector count (`lag_map`, `pick_correctors`: they keep the lagged rotational coupling, named limitation), the
steady solver, the fractional step, the transposed stress source.

**Default bits.** With the setting off no line of the default path changes its arithmetic: the new code is behind
`s.rotational_periodic_implicit && <a rotational link exists>`. The case tree and the API get no key (a key changes the schema
golden: open, Q2).

## Estimates (before any run)

- **a3 (K1s, K1).** `delta_*(n) - delta_(6,6)(n) = (u_C* - u_C6) - (u_A1 - u_A6)`. The lag is removed exactly in `C*` (to the
  solver tolerance `1e-13`), and in `C6` to `3e-7` (LC of ADR 0661). What remains is the difference between one and six outer
  iterations of everything else (the convecting flux, the deferred corrections, the PISO splitting), which the sector and the
  annulus share cell for cell (the cells match to `1.5e-15`, ADR 0661 G0) except where their treatment differs: Q4 at the wrap
  pairs and the rest R. So the remainder is the change of Q4 and R with the outer count. ADR 0661 measured `M(Q) = 5.98e-3` with
  the lag removed by outer iterations and `M(Q') = 5.90e-3` with one outer iteration: Q4's size changes by about 1.3 percent with
  the outer count. A field change of up to 10 percent of `M(Q)` (Q's pattern shifting with the PISO splitting error of the
  pressure at the wrap pairs) and `M(R) = 1.5e-4` give `6e-4 + 1.5e-4 = 7.5e-4 U_in`. Limit **`eps = 1.5e-3`** (a quarter of
  `V(6, 6)`'s `6.0e-3`; margin 2). Without the setting the same measure is `M(L) = 2.0e-2`.
- **a4 (J1).** From ADR 0641's start the step-1 difference of `V(1,6)` is `3.494e-2`, all of it the lag (J of ADR 0661, share
  `l = 1.000`); `V(6,6)` reaches at most `1.91e-3` over 16 steps. With the lag removed in one outer iteration the step-1
  difference should be that of `V(6,6)` plus the step-1 outer-count change of Q4 (as above, `<= 0.3 x 1.9e-3`, larger fraction
  for the violent first step): about `2.5e-3`. Limit **`5e-3`** (one seventh of `3.494e-2`; below ADR 0641's own limit `3e-2`).
- **K1J (fast form of K1).** The field form over J's 16 steps: `max |delta_* - delta_(6,6)|` about `6e-4` by the same
  argument (Q4 `1.9e-3` in J, 30 percent of it); limit **`2e-3`**.
- **Cost.** The block system is `2n` unknowns on the 2D sector (z inactive), one BiCGStab solve in place of two; the step
  costs about as much as `V(1, 6)` (82 s for 400 steps in ADR 0661). The 400-step run of `V*(1, 6)` beside `V(6, 6)` costs about
  `(1 + 6) / 14 x 1092 = 550 s`: a heavy record (`#[ignore]`, run once), J (16 steps, three variants, about 30 s) the fast test
  on the same path.

## Checks (limits fixed now)

| Id | Clause | Set-up | Check | Limit |
| --- | --- | --- | --- | --- |
| K1s | a3 | heavy record: `V*(1, 6)` and `V(6, 6)` from `u0`, 400 steps | on every step `abs(G_*(n) - G_(6,6)(n))` | `<= 1.5e-3` each step |
| K1 | a3 (supporting) | same run | on every step `max_c abs(delta_*(n) - delta_(6,6)(n)) / U_in` | `<= 1.5e-3` each step |
| J1 | a4 | fast: `V*(1, 6)` from ADR 0641's start | step 1's largest difference `G_*(1)` | `<= 5e-3` |
| K1J | a3 (fast form) | fast: `V*(1, 6)` and `V(6, 6)` from ADR 0641's start, 16 steps | `max` over the steps and cells of `abs(delta_* - delta_(6,6)) / U_in` | `<= 2e-3` |
| D0 | a2 | fast: `V(1, 6)` (setting off) from ADR 0641's start | step 1 reproduces ADR 0661 J0's `3.494135e-2` (printed to 7 digits) | within `1e-8` |
| D1 | a2 (no-op without rotation) | unit test, the doubly periodic Taylor-Green box of `pimple.rs` (translational pairs), 5 PISO steps | velocity, pressure, flux with the setting on equal those with it off | bit for bit |
| D2 | a2 (no-op without periodic pairs) | fast: the annulus A of `V*(1, 6)` against the annulus of `V(1, 6)` (setting off), 16 steps | velocity and pressure | bit for bit |
| A1s | supporting | heavy record, `V*(1, 6)` | C's `flux_residual` from step 2 | `<= 1e-12` |
| G0 | set-up | every variant, every step | every C cell's nearest A cell within `1e-9`, one-to-one | as stated |
| NC1 | control of K1J | fast: `V(1, 6)` (setting off) against `V(6, 6)`, 16 steps | the measure of K1J | `>= 1e-2` |
| NC2 | control of J1 | fast: `V(1, 6)` (setting off) | step 1's `G` exceeds J1's limit | `>= 5e-3` |
| B1 | a2 | `cargo test -p rcfd-api --release --test baseline_hashes`; the named periodic and sliding suites (addendum) | pass | as stated |

Printed, no limit: `G_*`, `G_(6,6)` and the step and location of each largest difference; the largest of `G_*(n) - G_(6,6)(n)`
with its sign; the block solve's iteration counts are not reported (no hook); elapsed times.

Mutations (on a scratch copy of the library, after the checks; each applied alone; the fast test run once per mutation; each
must fail):

- **MX1** the rotational links are not added to the block matrix (the right-hand side still fixed): the partner term is
  dropped. J1 or K1J must fail.
- **MX2** the links take `R^T` in place of `R`: J1 or K1J must fail.
- **MX3** the `H` operator of the correctors omits the rotational links: K1J or J1 must fail.
- **MX4** the right-hand side is not fixed (the lagged partner term kept beside the implicit one): J1 or K1J must fail.

## Decision rule

**Done** when K1s and J1 (the acceptance's a3 and a4), K1J, D0, D1, D2, G0, NC1, NC2 and B1 are met and all four mutations
are caught; K1 and A1s are supporting and reported. **Partial** when K1s and J1 are met but a supporting fast check, a set-up
check, a control or a mutation is missed; or when one of K1s, J1 is missed with its cause diagnosed. **Todo** when K1s and J1
are both missed, or a2 fails (D0, D1, D2 or B1).

## Limits (not covered)

- No case-file key and no API field: the setting is reachable from `PimpleSettings` only (a key changes the schema golden; Q2).
- The default is not changed (it changes every rotationally periodic transient result; Q1).
- The pressure-lag estimate, the adaptive corrector count and the lag preflight keep the lagged rotational coupling.
- The steady SIMPLE solver, the fractional step and the transposed viscous stress (rotational patches skipped, ADR 0032) are
  not changed.
- Q4 (the orthogonal-only coupling of the coupled faces, ADR 0629 Q4) is not touched: what `V(6, 6)` leaves stays (F-725's
  other follow-up).
- Relaxation (`alpha_u < 1`) and the consistent correctors with the setting are implemented as stated but not tested here.
- One mesh pair (SS against P), one time step, one inlet modulation.

## Open questions

- **Q1. The default.** (a) keep the lagged default (taken: the house rule; changing it changes every rotationally periodic
  transient result and the baseline hashes of any such case); (b) make the implicit pair the default. Not taken: it changes
  default results, so it is listed for the maintainer. Recommendation (b) after this item's checks, in its own change with the
  hashes re-recorded by the maintainer.
- **Q2. A case key.** (a) none (taken: a key changes the schema golden); (b) `solver.periodic_momentum = "implicit"`. Open for
  the maintainer (golden). Recommendation (b) as a follow-up.
- **Q3. Block solve or component sweeps.** (a) one block system over the active components (taken: exact, one solve); (b)
  component Gauss-Seidel sweeps with the own-component part `R_kk` implicit and the cross terms lagged, repeated until
  converged (no CSR of size `n_a n`, but an inner iteration count to choose). Taken (a) on the maintainer's standing instruction
  of 2026-11-11 (take the recommendation).
