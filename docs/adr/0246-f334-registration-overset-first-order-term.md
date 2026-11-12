# ADR 0246: F-334 registration: the first-order error term of the calculated component cells of the overset coupling

Status: **REGISTERED 2026-11-12**, written before any code of F-334 and before any run of its checks. Not yet accepted by the
maintainer. Before this text no check was run: the code and the documents listed under "What was read" were read, the
arithmetic under "Arithmetic before this registration" was done by hand, and the unchanged tree of main was compiled once
(`cargo build --release -p rcfd-fv --tests`, to fill the target directory of the new worktree; nothing was executed).
Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0232](0232-f314-registration-overset-observed-order-miss.md) (F-314: the set-up RL, the fit
`5.4e-5 (h/H)^2 + 1.0e-4 (h/H)`, H3, H10, H11, question Q3 whose recommendation is this item),
[ADR 0219](0219-f134-registration-overset-coupling-of-a-moving-component-mesh.md) (F-134: `OversetSolver`, the model M1 to
M11), [ADR 0238](0238-f335-registration-overset-outer-iterations.md) and
[ADR 0241](0241-f338-registration-overset-bdf2.md) (outer iterations and BDF2, opt-ins; defaults unchanged). Roadmap item
F-334 (depends on F-314).

Number: the lead gave 0246 to F-334. Branch `f334-overset-first-order-term` from main 2c07d031, worktree `w49i-work/f334`.

House rules (ADR 0130, ADR 0213, ADR 0219, ADR 0232):

- limits and deciding numbers are fixed here, before the code and the runs, and are not changed after a run; the limits of
  F-134, F-314, F-335 and F-338 are not changed, their tests and known-miss records are not edited;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the number recorded, and the cause is
  diagnosed, not tuned away;
- a defective set-up gets a new corrected test by an amendment written and committed before the corrected run, and the old
  one stays as an ignored record;
- a corrective design that the diagnosis makes necessary is registered in an amendment committed before its code and runs,
  with its derivation; it is opt-in, and without it every existing overset result is bit-identical;
- `crates/rcfd-api/tests/baseline_hashes.rs`, the hash tables and the goldens are not edited or re-recorded;
- each registered run is made once, and nothing is re-run to pick a result (a deterministic run that two tests share is the
  same run);
- no new third-party crate, no unsafe code, no commit trailers.

Clean-room: everything below is derived from the finite-volume definitions of the solver, Taylor's theorem and Lagrange
interpolation on a uniform grid. No paper, book, code or vendor document was read for this work.

## What F-334 asks

Acceptance (quoted): "Set-up RL of ADR 0232 (component at rest, fringe phase locked) on levels 1 to 4: the root mean square
of the error of the calculated component cells converges at order 1.7 or more between successive levels, or the ADR names
the source of the first-order term (coefficient 1.0e-4) with a number."

**Reading (registered here).**

1. The first branch is the check T below: three orders (levels 1 to 2, 2 to 3, 3 to 4), each at least 1.7, on the default
   code or with a corrective that an amendment registers as an opt-in.
2. "Names the source with a number" is read as: one hypothesis below is confirmed by its registered causal measurement (a
   switch that removes or moves the term), with the size of the term it accounts for.
3. This is a diagnosis first. The registration changes no library code (tests only).

## What was read (no run)

- ADR 0232 in full, ADR 0219 (the model M1 to M11, amendments), the summaries of ADR 0238 and ADR 0241.
  `crates/rcfd-fv/src/overset.rs` in full, the public interface of `crates/rcfd-mesh/src/overset.rs`,
  `crates/rcfd-fv/tests/overset_f314.rs` (the helpers, RL, the tests of amendments 3 and 4), `pressure_bc`, `face_flux`,
  the default conditions of a field.
- Numbers taken from ADR 0232 (RL, `200 L` steps): the root mean square of the error of the calculated component cells,
  `e_L` below, is `1.549e-4`, `6.39e-5`, `3.98e-5` at levels 1, 2, 3 (orders `1.277`, `1.166`); with half the step at level
  3 it is `1.0085` of that (H10); by ring it is flat (H11); `kappa_L` (the truncation of the component fringe over
  `h_B^2`) is `0.0887`, `0.0891`, `0.0892`.

What the code read shows, relevant to the candidates:

1. The four patches of the component's outer boundary carry the default conditions (zero gradient for `u` and `p`); every
   cell with a face on them is a fringe cell, whose rows are replaced. No calculated row contains a coefficient of those
   faces.
2. The convection scheme of RL is `Central` on every face; no limiter exists on the overset path (the setting is refused).
3. A flux built from an interpolated velocity is used on the faces without a calculated cell only (M8, M9), and those
   enter no equation. The flux of a face between a calculated and a fringe cell is the flux of the pressure equation,
   built from the interpolated `HbyA` and `rAU` of the fringe cell and its interpolated pressure.
4. One stencil per fringe cell carries all four interpolated quantities (`u`, `p`, `HbyA`, `rAU`). A test can replace
   the stencils of a prepared step (`OversetStep::conn` is public) but cannot treat the quantities apart.
5. In the closed domain of RL the conservative mode cannot be switched off as a control: the interpolated mode leaves the
   pressure system inconsistent (ADR 0219 amendment 2).
6. The error of the check is taken at the cell centres of the composite geometry at the pose, at `t = 0.5 = steps * dt`.

## Arithmetic before this registration (disclosed, by hand)

- Under the model `e_L = a / L^2 + b / L` the first-order coefficient of a pair of levels `L` and `2 L` is
  `b = L (4 e_2L - e_L)`. Levels 1 and 2 of ADR 0232: `4 * 6.39e-5 - 1.549e-4 = 1.007e-4`. The fit predicts
  `e_4 = 5.42e-5 / 16 + 1.007e-4 / 4 = 2.86e-5` (order 1.15 between levels 3 and 4).
- `rAU = 1 / (1/dt + 4 nu / h^2)` on a uniform part: with `a = 4 nu dt / H^2 = 0.162 L` it is `dt / (1 + a)` on the
  background and `dt / (1 + a / 0.5625)` on the component; the two differ by 10 percent (level 1) to 25 percent (level 3).
- With the transient Rhie-Chow term the defect `D = phi - (u)_f . S` of a face obeys `D_new = theta D + R`, `theta = rAU_f
  / dt`; in a steady state `D = R / (1 - theta)` and `rAU / (1 - theta) = h^2 / (4 nu)`, which does not depend on `dt`. A
  jump `J` of the fringe pressure against the smooth continuation of the pressure of the calculated cells gives `R =
  -(J / h)(rAU_F / 2 + rAU_C / 4)`, so a net-flux defect `q` (a mean normal velocity) of an interpolated fringe velocity
  is carried by a jump of about `J = (8 nu / 3 h) q`: a pressure jump of first order for a velocity defect of second order,
  independent of `dt`. A uniform jump along a closed fringe is a level and moves no fluid; whether its variation does is not
  decided by this arithmetic.
- A linear error analysis of the coupling by hand (Dirichlet data of the size of the interpolation truncation for `u` and
  `p`, a stable operator) gives a second-order error of the calculated cells for every one of the candidates below. I
  could not derive a first-order term on paper. The hypotheses are therefore decided by switches, not by argument.
- Phase of the background fringe in its component donor cells in RL: the fringe ring lies `4.0 H`, `3.25 H`, `3.33 H`,
  `3.125 H` from the component centre at levels 1 to 4, and its centres fall at the fractions `0.667`, `0.333`, `0.667`,
  `0.333` of a donor cell, which are mirror images. RL locks the component fringe exactly and the background fringe up to
  that reflection.
- With the background at level `L_b` and the component at level `L_c` the component fringe centres fall at the fractions
  `0.375 r + 0.75 r j` of a background cell, `r = L_b / L_c`: the phase is locked within a family of constant `r`.

## Notation

RL of ADR 0232 amendment 3: background `40 L x 40 L` on `[0, pi]^2`, `H = pi / 40`; component of half width `7.5 H` with
`20 L` cells per side, its corner on the background node `(9 H, 9 H)`, no rotation, at rest; hole `max(|x_ref|, |y_ref|) <
3.1 H`; Taylor-Green, `nu = 0.1`, the settings PS of ADR 0219 (central convection, two correctors), conservative mode,
start from the exact fields, `200 L` steps to `t = 0.5`. `RL(L_b, L_c)` is the same with the two parts at different levels
and `200 max(L_b, L_c)` steps. The error of a cell is `eps_c = |u_c - u_ex(x_c, t)| / (A e^{-2 nu t})` (`A = 1` unless
stated). `e` is the root mean square of `eps_c` over the calculated component cells (CC), weighted with the cell volumes.
`b^ = 4 e_2 - e_1` is the first-order coefficient of the levels 1 and 2 of a variant (for a family: of its two members, in
units of the level of the coarser member). `b0` is `b^` of the unchanged RL as run by this file (ADR 0232: `1.007e-4`), and
`rho_V = b^_V / b0` for a variant `V`.

## Hypotheses, measurements and the numbers that decide

| Id | Hypothesis | Measurement | Deciding number |
| --- | --- | --- | --- |
| Hd | the reference or the norm of the check is at fault (cell-centre against cell-mean values; the exact solution sampled at the wrong time or place) | (i) `b^` with the exact cell mean in place of the centre value (for this solution the mean over a square cell of side `h` is the centre value times `(sin(h/2) / (h/2))^2`); (ii) the largest error of a calculated cell directly after the construction of the solver at `t = 0` (C2) | ruled out: `abs(rho_mean - 1) <= 0.1` and C2 met; confirmed: `rho_mean <= 0.5` |
| Ht | the term is a time error (`dt` is proportional to `h` in RL); H10 of ADR 0232 measured level 3 only, by a norm | levels 1 and 2 with `200 L` and `400 L` steps: `tau_L`, the root mean square over CC of the difference of the two velocity fields (same cells), over the decay factor; the first-order time error of the run with `200 L` steps is `2 tau_L` | ruled out: `2 tau_L <= 0.2 b0 / L` at both levels; confirmed: `>= 0.7 b0 / L` at both |
| Hs | the term belongs to the single-mesh solution, not to the coupling | the background alone (the library step, `200 L` steps, levels 1 and 2): `b^_s` of the root mean square over the background cells whose centre lies in the calculated region of the component | ruled out: `b^_s <= 0.25 b0`; confirmed: `b^_s >= 0.5 b0` |
| Hc | the treatment of the component's outer boundary | level 1 with the velocity condition of the four outer patches changed from zero gradient to a fixed value zero: the largest difference of `u` over the non-hole cells at `t = 0.5` | ruled out: `<= 1e-10`; otherwise open, to be measured further |
| Hf | the ratio `h / H`: the term belongs to the spacing of one of the two parts | the families `B2 = RL(2,1), RL(4,2)` and `C2 = RL(1,2), RL(2,4)`: under `e = a_B / L_b^2 + a_C / L_c^2 + b_B / L_b + b_C / L_c` (per family, the `a` are the family's own) `b^_B2 = b_B / 2 + b_C` and `b^_C2 = b_B + b_C / 2`, so `b_B = 2 (b0 - b^_B2)`, `b_C = 2 (b0 - b^_C2)`; the share `s_C = b_C / (b_B + b_C)`; consistency `k = (b^_B2 + b^_C2) / (1.5 b0)` | component: `s_C >= 0.75`; background: `s_C <= 0.25`; otherwise mixed; not separated when `abs(k - 1) > 0.3` |
| Hq | the term is caused by the truncation of the linear donor interpolation (of some interpolated quantity, on one of the two fringes), although that truncation is of second order | levels 1 and 2 with the stencils replaced after every `prepare` by the nine-cell stencil that is exact for biquadratic fields (below), on the component fringe only (`Qc`), on the background fringe only (`Qb`), on both (`Qcb`) | confirmed: `rho_Qcb <= 0.25`; attributed to one fringe when its `rho <= 0.25` and the other's `>= 0.75`; ruled out: `rho_Qcb >= 0.75`; otherwise mixed |
| Hn | the term travels by a path that is linear in the velocity (the interpolated velocity and its mass defect, diffusion) or by one that is quadratic (the interpolated pressure, convection) | levels 1 and 2 with the amplitude `A = 1/2` (`u = A u_TG`, `p = A^2 p_TG`, an exact solution with the same decay); `eps` is relative to `A`, so a linear path gives `rho_A = 1` and a quadratic one `rho_A = 1/2` | linear: `rho_A >= 0.85`; quadratic: `rho_A <= 0.65`; otherwise mixed |
| Ha | the pressure coupling: (a1) the mass defect of the interpolated velocity across the overlap, which the pressure equation absorbs; (a2) the interpolated fringe pressure; (a3) the pressure level (the reference cell and the offset `delta`) | (a3) levels 1 and 2 with the reference cell moved from cell 0 to the last background cell (the opposite corner); (a1), (a2) through Hq and Hn; printed with them: `delta`, the mean normal velocity defect of the interpolated velocity through the two closed fringe boundaries, the pressure error by class with the level removed, the pressure jump at the fringe faces | (a3) ruled out: `abs(rho_pin - 1) <= 0.05`. (a1) confirmed: Hq confirmed and Hn linear; (a2) confirmed: Hq confirmed and Hn quadratic; both ruled out: Hq ruled out |
| Hb | the flux at the fringe faces, as opposed to the cell velocity | `D_f = abs(phi_f - (u)_f . S_f) / abs(S_f)` at `t = 0.5`, root mean square over the faces between a calculated and a fringe component cell (`D_CF`), the same for the background (`D_BF`) and over the faces between two calculated component cells (`D_CC`), levels 1 to 4 | present: the order of `D_CF` or of `D_BF` is below 1.5 on at least two of the three pairs of levels while that of `D_CC` is at least 1.7 on each; ruled out: both at least 1.7 on each pair; a cause only together with a switch (Hq) |
| He | the convection scheme or a limiter next to the interpolated cells | by reading (item 2) there is no limiter and one scheme on all faces; the settings are asserted in the test; Hn measures the size of the quadratic path | ruled out: the settings hold and Hn is linear, or Hq is confirmed (the switch leaves the scheme untouched); otherwise open |

The nine-cell stencil of Hq. On a uniform Cartesian part with the cells `(i, j)`, for a point at the offset `(xi, eta)`
from the centre of its donor cell `(i0, j0)` in units of the cell size (`abs(xi), abs(eta) <= 1/2`), the weights are the
products `l_m(xi) l_n(eta)`, `m, n` in `{-1, 0, 1}`, of the quadratic Lagrange polynomials through the three centres,
`l_-1(s) = s (s - 1) / 2`, `l_0(s) = 1 - s^2`, `l_1(s) = s (s + 1) / 2`, on the cells `(i0 + m, j0 + n)`. Lagrange
interpolation through three points reproduces every quadratic in one variable, the product every biquadratic; the weights
sum to one. The truncation is of third order. The stencils are built in the test (component weights in the reference
frame); all nine cells must be calculated cells of the other part.

## Registered checks

New test file `crates/rcfd-fv/tests/overset_f334.rs`; release (`cargo test --release -p rcfd-fv --test overset_f334`). No
library code and no existing test is edited. The tests that take more than about two minutes are `#[ignore]` with their
run time as the reason and are run once for the addendum.

| Id | Check | Limit |
| --- | --- | --- |
| T (the acceptance) | RL, levels 1 to 4 (`200 L` steps): the orders of `e_L` between levels 1 and 2, 2 and 3, 3 and 4 | `>= 1.7` each |
| C1 | every step of every overset run of this file: the continuity of the calculated cells | `<= 1e-9` |
| C2 | RL, levels 1 and 2, the solver as constructed at `t = 0`: the largest `eps_c` over the calculated cells | `<= 1e-14` |
| K1 | RL, levels 1 to 4 (kinematic): `kappa_L` against `kappa_1` | within 2 percent |
| K2 | the nine-cell stencils of RL, levels 1, 2 and 4 (kinematic): all nine cells calculated; the field `(1 + x + 2 y)(1 - x + y) + x^2 y^2` is reproduced at every fringe centre; the largest truncation of the exact velocity over all fringe cells converges | exact; `<= 1e-11`; order `>= 2.7` between levels 1 and 2 and between 2 and 4 |
| CM (control) | coincident meshes, levels 1 and 2: the component has the half width `8 H`, `16 L` cells per side and its corner on the node `(8 H, 8 H)`, so every component cell coincides with a background cell and every stencil reduces to its donor (the weights of ADR 0219 M4 leave about `8e-7` on each neighbour); `200 L` steps. The largest difference of `u` between every calculated cell and the cell at the same place of the background alone (the library step on the background mesh) | `<= 1e-6` |
| ND (negative control) | RL, levels 1 and 2 with every stencil replaced by its donor cell with the weight 1: `rho_donor` | `>= 3` |

CM says that with an interpolation without truncation the coupling (the rows, `HbyA` and `rAU` of the fringe, the offset
`delta`, M9) adds nothing to the single-mesh solution. If it fails, the coupling itself has a defect and the hypotheses
above are not the whole list.

Printed, no limit: for every run the run time, the largest continuity, the largest and the mean `abs(delta)`; for RL at
every level `e_L`, the largest CC error, the root mean square and the largest error per class, `b` of every pair, the
prediction `2.86e-5` against `e_4`; the mean error vector over CC and the share of the error that is a multiple of the exact
velocity; the pressure error per class with the mean over the calculated background cells removed, and the difference of
the class means of CC and of the calculated background cells; the mean and the root mean square of the pressure jump at the
fringe faces, `(p_F - p_C) - (p_ex,F - p_ex,C)`; the mean normal velocity of the face-interpolated cell velocity through
the closed boundary of the calculated component cells and through `Gamma_B`; `D_CF`, `D_BF`, `D_CC`; every `b^` and `rho`.

### Mutations (scratch copy, not committed)

Each is applied alone to a `git archive` copy of the final tree in `E:/projects/cfd/w49i-work/f334-scratch`, the named test
is run once and must fail; the numbers go in the addendum. `scripts/limit_audit.py` is run on the new test file and every
hit is read.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | `rcfd_fv::overset`, `overset_pressure`: the offset `delta` is not applied (returned as zero after the first solve) | C1 |
| MU2 | `rcfd_mesh::overset`, `linear_weights`: inverse-distance weights (only `sum w = 1`) | K1 |
| MU3 | the test's nine-cell weights: `l_0(s) = 1 - s`-type linear weights on two cells per direction in place of the quadratic ones | K2 |
| MU4 | `rcfd_fv::overset`, `piso`: the fringe `HbyA` is not interpolated (M6 skipped for `HbyA`) | CM |

A corrective registered by an amendment brings its own mutations.

### Classes

A miss of T, C1, C2, K1 or K2 is reported with its number and kept as an ignored `*_known_miss` test with the limit
intact. A control (CM, ND) that does not hold, or a mutation that does not make its check fail, is a defective set-up:
corrected by an amendment before any re-run, never counted as a pass. The hypotheses are decided by the numbers of the
table above only. A hypothesis that the switches of this registration leave undecided is followed up by an amendment
(written and committed before its runs) or listed as open.

## Predictions (mine, before any run)

T missed, with `e_4` between `2.6e-5` and `3.1e-5`. C1, C2, K1, K2 met. CM holds (difference near `1e-8`). ND holds. Hd,
Ht, Hc and (a3) ruled out. Hs ruled out. Hf, Hq and Hn: I have no prediction that I trust; the hand analysis says that a
second-order truncation cannot produce the term, so either Hq is ruled out and the term is not an interpolation error at
all, or the analysis misses a path. Hb: `D_CF` of first order would not surprise me (the pressure jump of the arithmetic
above).

## Decision rule

- **done**: T is met (on the default code, or with an opt-in corrective registered by an amendment, the defaults
  bit-identical), C1 to K2 are met, the controls held and the mutations failed their checks.
- **partial**: T is not met and the source of the first-order term is named by a confirmed hypothesis with its number (the
  second branch of the acceptance); or T is met only on some pairs; or a control or mutation did not hold and was not
  corrected.
- **todo**: T is not met and no hypothesis is confirmed, or no registered measurement could be run.

## Limits of this registration

- Taylor-Green in RL only (uniform Cartesian parts, parallel grids, one component at rest, `nu = 0.1`, `t = 0.5`).
- `b^` rests on two levels and on the model `a / L^2 + b / L` for a root mean square; the fields of two levels are not
  compared cell by cell. Level 4 is the finest flow level.
- Hq replaces the stencil of all four interpolated quantities at once; it cannot tell `u`, `p`, `HbyA` and `rAU` apart.
  Hn tells the linear from the quadratic path only.
- The families of Hf have a phase of their own each; only the first-order coefficient is compared between them.
- The conservative mode cannot be switched off in the closed domain, so (a1) is decided through Hq and Hn, not directly.

## Open questions

Listed with options and a recommendation in the addendum, after the results.

## Amendment 1 (2026-11-12, written after the registered runs and before any further run)

The registered tests were committed (`ff117ee5`, compiled, formatted and linted without running) and then run once each,
release, on the library of main 2c07d031: the kinematic test and CM together (144 s), T (952 s), the switches (617 s) and
the families (866 s). All numbers are in addendum 1. Met: C1 (every run), C2, K1, K2; CM held (`7.2e-9`, `2.7e-9`); ND held
(`rho_donor = 170`). Decided by their registered numbers: Ht ruled out (`0.099`, `0.089`), Hs ruled out (`0.175`), Hc ruled
out (`0`, bit for bit), (a3) ruled out (`rho_pin = 1.0000`), Hn linear (`rho_A = 0.9993`), Hq confirmed (`rho_Qcb = 0.179`;
`rho_Qc = -0.038`, `rho_Qb = 0.588`, so not attributed to one fringe by the rule), Hf mixed (`s_C = 0.489`, `k = 1.077`).
Four results need this amendment:

| Id | Limit or deciding number | Measured |
| --- | --- | --- |
| T | orders `>= 1.7` | `1.277`, `1.166`, `1.626` (`e_L` `1.5486e-4`, `6.3902e-5`, `3.9830e-5`, `2.4949e-5`) |
| the fit of ADR 0232 at level 4 | predicted `2.86e-5` | `2.4949e-5`; `b` of the levels 2 and 4 is `7.18e-5`, not `1.007e-4` |
| Hd | ruled out: `abs(rho_mean - 1) <= 0.1`; confirmed: `rho_mean <= 0.5` | `rho_mean = 0.617`: neither |
| (a1) | confirmed: Hq confirmed and Hn linear | confirmed by that rule; but the mean normal velocity defect of the interpolated velocity is `-1.6e-5`, `-8.8e-7`, `-1.2e-7`, `-2.0e-8` at levels 1 to 4, far below `e_L` |

**T** is missed and is kept as the ignored test `t_rl_levels_1_to_4_known_miss` with the limit intact. It is not run again.

**The fit.** `e_L L^2` is `1.549e-4`, `2.556e-4`, `3.585e-4`, `3.992e-4`: it grows by `1.01e-4`, `1.03e-4` and then by
`0.41e-4`. A term `1.0e-4 / L` would keep it growing by `1.0e-4` per level. The model `a / L^2 + b / L` of ADR 0232 does
not describe level 4, so `b^` (which rests on that model) is a measure of the levels 1 and 2 only, as the limits of this
registration say.

**(a1).** The rule that I registered for (a1) cannot tell the mass defect from any other path that is linear in the
velocity; the direct number (the defect itself) is three orders of magnitude below the error. (a1) is reported as
"confirmed by the letter of the rule, contradicted by the direct number", and is not used to name the source. The flux
defect `D_CF` (Hb) is `8.6e-5`, `1.5e-5`, `6.2e-6`, `3.1e-6` (orders `2.53`, `2.17`, `2.43`), so the flux and the cell
velocity agree at the fringe faces at second order: Hb is ruled out by its number.

**What the switches show beyond their deciding numbers.** With the nine-cell stencils on both fringes the error of the
calculated component cells is *larger* at the levels 1 and 2 (`3.241e-4`, `8.553e-5`, order `1.92`), it is almost a pure
multiple of the exact velocity (share `0.986`, `0.978`), and it equals the error of the background alone over the same
region (`3.235e-4`, `8.529e-5`, Hs). With the linear stencils the share is `0.47` to `0.51` and the mean error vector has
the opposite sign. So the error with the library stencils looks like the sum of two parts of opposite sign: the
discretisation error that a single mesh has too (an amplitude error, second order), and the response of the calculated
cells to the truncation of the linear interpolation. `rho_mean = 0.617` fits this: the cell mean differs from the centre
value by `-h^2 / 12` times the exact velocity, a multiple of the exact velocity again, and it changes `b^` by 38 percent,
which a first-order term of another shape would not allow.

New hypothesis, with direct measures that do not rest on the model `a / L^2 + b / L`:

| Id | Hypothesis | Measurement | Deciding number |
| --- | --- | --- | --- |
| H12 | there is no first-order term: `u_base - u_ex = (u_Q - u_ex) + R`, where the error with the nine-cell stencils `e_Q` and the response `R = u_base - u_Qcb` (the two runs on the same mesh, cell by cell) are each of second order and of opposite sign, and the low order of `e_L` is their partial cancellation | RL at levels 1 to 4 with the library stencils and with `Qcb` (levels 1 to 3 also with `Qc` and `Qb`): the root mean square over CC of `e_Q` and of `R` at `t = 0.5`, their orders between successive levels, and the cosine of `R` with the error field of the `Qcb` run | confirmed: the orders of `e_Q` (TQ below) and of `R` are at least 1.7 on each of the three pairs and the cosine is at most `-0.5` at every level; ruled out: the order of `R` is below 1.5 on the pairs 2 to 3 and 3 to 4 (then `R` carries the term and is diagnosed further); otherwise undecided |

New check: **TQ**, RL at levels 1 to 4 with the nine-cell stencils on both fringes (`200 L` steps): the orders of the root
mean square of the error of the calculated component cells between successive levels, limit `>= 1.7` each. The levels run
in three test processes (1 and 2; 3; 4); the pair 1 to 2 is asserted in its test, the pairs 2 to 3 and 3 to 4 are formed
in the addendum from the printed numbers, and a pair that misses gets an ignored record test with the numbers.

Printed with it, no limit: the error of every run with the cell mean as the reference; `R` of `Qc` and of `Qb` (levels 1
to 3); for every `R` its root mean square at `t = 0.0025`, `0.025`, `0.125`, `0.25`, `0.5` (whether it is there after the
first steps or grows), its mean vector, its root mean square over the rings 1 and 2 and over the rings from 4 on.

Tests: `a1_response_levels_1_and_2`, `a1_response_level_3`, `a1_response_level_4` (ignored, slow, run once each; they
repeat the RL runs of T, which are deterministic). The run helper keeps the velocity at the five times since this
amendment; the registered runs are unchanged. Prediction (mine): TQ met with orders near 1.9 to 2.0; the order of `R`
between 1.6 and 1.9 with the cosine near `-0.9`; I do not know whether `R` reaches 1.7 on every pair.

The decision rule is unchanged. A confirmed H12 names the source (the second branch of the acceptance) with the numbers of
the two parts; T stays missed on the library's stencils.

## Amendment 2 (2026-11-12, written after the runs of amendment 1 and before any further run; the last diagnostic of this registration)

The three tests of amendment 1 ran once each (323 s, 926 s, 1180 s). **TQ is met**: with the nine-cell stencils on both
fringes `e_Q` is `3.2411e-4`, `8.5532e-5`, `3.9022e-5`, `2.2269e-5`, orders `1.922`, `1.935`, `1.950`. The response `R =
u_base - u_Qcb` is `4.3758e-4`, `1.3537e-4`, `7.1102e-5`, `4.2366e-5`: orders **`1.693`, `1.588`, `1.800`**; its cosine with
the error of the `Qcb` run is `-0.961`, `-0.931`, `-0.900`, `-0.884`. By its rule **H12 is undecided**: `R` does not reach
1.7 on two of the three pairs and is not below 1.5 on the last two. What the numbers do say:

- `e_L^2 = e_Q^2 + R^2 + 2 cos e_Q R` reproduces `e_L` to four digits at every level, so the error with the library
  stencils is the sum of the two parts, and they cancel to 88 to 96 percent. `R` is the larger one (1.35 to 1.90 times
  `e_Q`).
- `R L^2` is `4.376e-4`, `5.415e-4`, `6.399e-4`, `6.779e-4`. Its increments from level to level are `1.04e-4`, `0.98e-4`,
  `0.38e-4`: the coefficient `1.0e-4` of ADR 0232 is the first-order part of `R` on the levels 1 to 3, and it falls at
  level 4.
- by fringe (levels 1 to 3): the response to the truncation at the component fringe, `R_c = u_base - u_Qc`, is
  `4.0455e-4`, `1.1765e-4`, `5.9108e-5` (orders `1.782`, `1.698`); the response to the truncation at the background fringe,
  `R_b = u_base - u_Qb`, is `4.8742e-5`, `2.1922e-5`, `1.4073e-5` (orders **`1.153`, `1.093`**), `4.9e-5`, `4.4e-5`,
  `4.2e-5` times `1 / L`. `R_b` is of first order on these levels. Its mean vector hardly falls (`(4.7e-6, -1.27e-5)`,
  `(5.8e-6, -8.8e-6)`, `(4.8e-6, -7.1e-6)`), it is flat over the rings, and between `t = 0.25` and `0.5` it doubles
  (`7.5e-6` to `1.41e-5` at level 3) while `R_c` grows by a fifth.

So the term sits in the response of the calculated component cells to the truncation of the linear interpolation, most
clearly in the part that starts at the background fringe, travels through the background cells of the overlap and enters
the component through its own fringe. Why a second-order truncation gives a first-order response there is not known. Two
measurements are registered; no further diagnostic is added after them. `beta^ = 9 R_3 - 4 R_2` is the first-order
coefficient of a response on the levels 2 and 3 (`3.90e-5` for `R_b`, `9.85e-5` for `R`).

| Id | Hypothesis | Measurement | Deciding number |
| --- | --- | --- | --- |
| H13 | the first-order part of `R_b` is made by the transient Rhie-Chow term of the step (the memory `D_new = theta D + R` of the arithmetic before the registration, which makes the flux of a face follow a change of the fringe velocity with a coefficient proportional to `h / nu`) | RL at levels 1 to 3 with `ddt_correction` off (a setting of the library, not refused on the overset path), with the library stencils, `Qb` and `Qcb`: `R_b` and `R` of these runs, their orders and `beta^` | confirmed: `beta^` of `R_b` at most `0.25` of `3.90e-5` and the order of `R_b` between levels 2 and 3 at least 1.7; ruled out: `beta^` at least `0.75` of it; otherwise mixed |
| H14 | `R_b` at `t = 0.5` is a signal that is still arriving (it has to cross the overlap), so its size at a fixed time is not the size of a settled error | RL at levels 1 and 2 run on to `t = 1` (`400 L` steps) with the library stencils, `Qb` and `Qcb`: `R_b` at `t = 0.5`, `0.75`, `1` | still arriving: `R_b(1) / R_b(0.5) >= 1.5` at level 2; settled: `<= 1.1`; otherwise in between |

Printed with them, no limit: `e` of every run and its orders (whether the order of the error with the library stencils
changes without the transient term), every response at the five times, the errors and responses of the long run at
`t = 0.75` and `1`.

Tests `a2_without_the_transient_rhie_chow_term` and `a2_run_on_to_t_1` (ignored, slow, run once each). The run helper
takes the end time and the setting since this amendment; the registered runs are unchanged. Prediction (mine): H14 "still
arriving" (the doubling between `0.25` and `0.5`); H13 uncertain: the memory is the one place where I found a factor `1 /
h` on paper, but I could not derive the first-order response from it.

The decision rule is unchanged.

## Addendum 1: results (2026-11-12)

Measured against the limits and deciding numbers registered above and in amendments 1 and 2; none was changed. Release
builds on Windows, the library of main 2c07d031 unchanged: this branch adds one test file and this ADR, and `git diff
2c07d031 -- crates/*/src` is empty, so no default and no pinned result can have moved. The registration is `50ebb370`; the
tests as registered `ff117ee5`; amendment 1 with its tests `defc5ff0`, amendment 2 with its tests `eaa0fb7e`, each
committed before its runs; the gate-run check `418a5738`. Each registered run was made once. The logs are in
`E:/projects/cfd/w49i-work/f334-scratch` (`f334-fast.log`, `f334-t_rl_levels_1_to_4.log`,
`f334-s_switches_levels_1_and_2.log`, `f334-f_families_of_different_levels.log`, `f334-a1_*.log`, `f334-a2_*.log`,
`f334-MU1.log` to `f334-MU4.log`, `f334-final.log`).

### What the diagnosis found, in short

1. **The order check is missed.** RL on levels 1 to 4: the root mean square of the error of the calculated component
   cells is `1.5486e-4`, `6.3902e-5`, `3.9830e-5`, `2.4949e-5`, orders `1.277`, `1.166`, `1.626` (limit 1.7).
2. **The fit of ADR 0232 does not hold at level 4.** `5.4e-5 (h/H)^2 + 1.0e-4 (h/H)` predicts `2.86e-5`; measured
   `2.49e-5`. The first-order coefficient is `1.007e-4` on the levels 1 and 2, `7.2e-5` on the levels 2 and 4.
3. **The source is the truncation of the linear donor interpolation.** With stencils that are exact for biquadratic
   fields on both fringes (nine cells, supplied by the test) the same error is `3.2411e-4`, `8.5532e-5`, `3.9022e-5`,
   `2.2269e-5`, orders **`1.922`, `1.935`, `1.950`** on levels 1 to 4. That error is almost a pure multiple of the exact
   velocity and equals the error of the background alone over the same region (`3.235e-4`, `8.529e-5`).
4. **The error with the library stencils is a difference of two larger parts.** `u_base - u_ex = (u_Q - u_ex) + R` with
   `R = u_base - u_Q` the response of the calculated cells to the truncation: `R` is `4.3758e-4`, `1.3537e-4`,
   `7.1102e-5`, `4.2366e-5` (orders `1.693`, `1.588`, `1.800`), 1.35 to 1.90 times the other part and opposed to it
   (cosine `-0.961` to `-0.884`). They cancel to 88 to 96 percent, so the library stencils give the *smaller* error at
   levels 1 and 2 (by factors 2.1 and 1.3), the same at level 3 and the larger one at level 4 (1.12). The coefficient
   `1.0e-4` is the increment of `R L^2` per level (`1.04e-4`, `0.98e-4`; `0.38e-4` from level 3 to 4).
5. **Its first-order part is clearest in the response to the background fringe.** By fringe, levels 1 to 3: `R_c`
   (component fringe) `4.0455e-4`, `1.1765e-4`, `5.9108e-5`, orders `1.782`, `1.698`; `R_b` (background fringe)
   `4.8742e-5`, `2.1922e-5`, `1.4073e-5`, orders **`1.153`, `1.093`**. `R_b` is still growing at `t = 0.5` (1.56 times
   larger at `t = 1`).
6. **Why a second-order truncation gives a response below second order is not found.** Ruled out with numbers: a time
   error, the single-mesh solution, the outer boundary of the component, the reference cell, the convection and pressure
   (quadratic) path, a mass defect of the interpolated velocity, the flux at the fringe faces, the transient Rhie-Chow
   term. With coincident meshes the coupling reproduces the background alone to `7e-9`.

### Hypotheses

| Id | Deciding number (registered) | Measured | Verdict |
| --- | --- | --- | --- |
| Hd reference or norm | ruled out: `abs(rho_mean - 1) <= 0.1` and C2; confirmed: `rho_mean <= 0.5` | `rho_mean = 0.617`; C2 `0` | **neither** by the rule. With the cell mean as the reference the orders are `1.397`, `0.994`, `1.513` (`1.1974e-4`, `4.5474e-5`, `3.0388e-5`, `1.9662e-5`): the reference does not make the miss, but it moves the numbers because both parts are near multiples of the exact velocity |
| Ht time error | ruled out: `2 tau_L <= 0.2 b0 / L` | `tau` `4.99e-6`, `2.25e-6`: ratios `0.099`, `0.089` | **ruled out** |
| Hs single-mesh solution | ruled out: `b^_s <= 0.25 b0` | `b^_s = 1.76e-5`, `0.175` of `b0` (`3.2353e-4`, `8.5285e-5`, order 1.92) | **ruled out** |
| Hc outer boundary of the component | ruled out: `<= 1e-10` | `0` (bit for bit) | **ruled out** |
| Hf one of the two spacings | component `s_C >= 0.75`, background `<= 0.25`; not separated when `abs(k - 1) > 0.3` | `b^_B2 = 8.09e-5`, `b^_C2 = 8.18e-5`, `b_B = 3.96e-5`, `b_C = 3.79e-5`, `s_C = 0.489`, `k = 1.077` | **mixed**: half to each spacing |
| Hq truncation of the linear interpolation | confirmed: `rho_Qcb <= 0.25`; one fringe: its `rho <= 0.25` and that of the other `>= 0.75` | `rho_Qcb = 0.179`, `rho_Qc = -0.038`, `rho_Qb = 0.588` | **confirmed**; not attributed to one fringe |
| Hn linear or quadratic path | linear: `rho_A >= 0.85` | `rho_A = 0.9993` (`1.5744e-4`, `6.4530e-5` at half the amplitude) | **linear** |
| Ha (a3) pressure level | ruled out: `abs(rho_pin - 1) <= 0.05` | `1.0000` (every printed digit equal) | **ruled out** |
| Ha (a2) interpolated pressure | confirmed: Hq confirmed and Hn quadratic | Hn linear | **ruled out**. `delta` is `9.09e-4`, `2.49e-4`, `1.20e-4`, `6.66e-5` and falls to `0.239` and `0.246` of that at half the amplitude: it closes the levels of the two interpolated pressures, a quadratic and second-order quantity |
| Ha (a1) mass defect | confirmed: Hq confirmed and Hn linear | met by the letter | **not used** (amendment 1): the defect itself, the mean normal velocity of the interpolated velocity through the closed fringe boundary, is `-1.6e-5`, `-8.8e-7`, `-1.2e-7`, `-2.0e-8`, three orders below the error at level 4. The rule I registered was too coarse |
| Hb flux at the fringe faces | present: order of `D_CF` or `D_BF` below 1.5 on two pairs; ruled out: both `>= 1.7` on each | `D_CF` `8.58e-5`, `1.49e-5`, `6.18e-6`, `3.07e-6` (`2.53`, `2.17`, `2.43`); `D_BF` (`2.54`, `2.43`, `2.65`); `D_CC` (`2.94`, `2.64`, `2.61`) | **ruled out** |
| He convection scheme or limiter | ruled out: settings hold and Hn linear, or Hq confirmed | both | **ruled out** |
| H12 two second-order parts of opposite sign | confirmed: orders of `e_Q` and of `R` `>= 1.7` on three pairs and cosine `<= -0.5`; ruled out: `R` below 1.5 on the last two pairs | `e_Q` `1.922`, `1.935`, `1.950`; `R` `1.693`, `1.588`, `1.800`; cosine `-0.961`, `-0.931`, `-0.900`, `-0.884` | **undecided**: the cancellation is confirmed, `R` is not of second order on these levels |
| H13 transient Rhie-Chow term | confirmed: `beta^` of `R_b` `<= 0.25` of `3.90e-5`; ruled out: `>= 0.75` | without the term `R_b` `1.1337e-4`, `3.3401e-5`, `1.8946e-5` (`1.763`, `1.398`), `beta^ = 3.69e-5` (`0.946`); `R` `6.0276e-4`, `1.6519e-4`, `8.3263e-5` (`1.867`, `1.690`), `beta^ = 8.86e-5` (`0.90`) | **ruled out** |
| H14 `R_b` still arriving at `t = 0.5` | still arriving: `R_b(1) / R_b(0.5) >= 1.5` at level 2 | `1.563` (`2.1922e-5`, `2.9114e-5`, `3.4255e-5` at `t = 0.5`, `0.75`, `1`); level 1 `1.751` | **still arriving** |

### Checks

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| T (the acceptance), RL levels 1 to 4 | orders `>= 1.7` | `1.277`, `1.166`, `1.626` | **missed** (known-miss test) |
| TQ, the same with the nine-cell stencils | orders `>= 1.7` | `1.922`, `1.935`, `1.950` | met |
| C1 continuity, every overset run | `<= 1e-9` | largest `7.1e-14` | met |
| C2 error of the calculated cells at `t = 0`, levels 1, 2 | `<= 1e-14` | `0` | met |
| K1 `kappa_L` against `kappa_1`, levels 2, 3, 4 | within 2 percent | `0.48`, `0.63`, `0.69` percent (`0.08869`, `0.08912`, `0.08924`, `0.08931`) | met |
| K2 nine-cell stencils, levels 1, 2, 4 | all calculated; biquadratic field `<= 1e-11`; order `>= 2.7` | all calculated; `7.4e-15`; `3.002`, `3.001` (`2.72e-5`, `3.40e-6`, `4.24e-7`); smallest weight `-0.1215` | met |
| CM (control) coincident meshes, levels 1, 2 | `<= 1e-6` | `7.2e-9`, `2.7e-9` (`delta` `1.6e-8`, `4.0e-9`) | held |
| ND (negative control) donor cell only | `rho_donor >= 3` | `170` (`5.343e-3`, `5.615e-3`) | held |

Printed, no limit. RL with the library stencils, levels 1 to 4, largest and root-mean-square error by class:

| Level | CF | CC | BF | BC |
| --- | --- | --- | --- | --- |
| 1 | `9.25e-4` / `4.01e-4` | `2.94e-4` / `1.549e-4` | `6.10e-4` / `3.76e-4` | `8.76e-4` / `5.58e-4` |
| 2 | `2.53e-4` / `1.06e-4` | `1.27e-4` / `6.39e-5` | `2.38e-4` / `1.32e-4` | `2.35e-4` / `1.41e-4` |
| 3 | `1.23e-4` / `5.38e-5` | `7.25e-5` / `3.98e-5` | `9.32e-5` / `6.70e-5` | `1.16e-4` / `6.45e-5` |
| 4 | `7.06e-5` / `3.09e-5` | `4.49e-5` / `2.49e-5` | `7.23e-5` / `4.27e-5` | `6.67e-5` / `3.66e-5` |

The mean error vector over CC is `(1.16e-5, -4.21e-5)`, `(1.25e-5, -3.07e-5)`, `(1.03e-5, -2.14e-5)`, `(7.2e-6,
-1.41e-5)`; the share of the error that is a multiple of the exact velocity `0.47` to `0.51` (with the nine-cell stencils
`0.986`, `0.978`). Pressure error of CC with the level removed `6.46e-4`, `1.95e-4`, `1.05e-4`, `6.85e-5` (orders `1.73`,
`1.52`, `1.50`). The families: `RL(2,1)` `1.0958e-4`, `RL(4,2)` `4.7631e-5`, `RL(1,2)` `3.9185e-4`, `RL(2,4)` `1.1842e-4`.
The switches at levels 1 and 2: `Qc` `3.0230e-4`, `7.4624e-5`; `Qb` `1.1200e-4`, `4.2807e-5`; `Qcb` `3.2411e-4`,
`8.5532e-5`. The response at `t = 0.0025`, `0.025`, `0.125`, `0.25`, `0.5`, level 3: `R` `6.1e-6`, `2.43e-5`, `4.28e-5`,
`5.41e-5`, `7.11e-5`; `R_b` `8.4e-7`, `2.5e-6`, `3.3e-6`, `7.5e-6`, `1.41e-5`; both are flat over the rings at `t = 0.5`.
Run on to `t = 1`, levels 1 and 2: the error with the library stencils `2.3721e-4`, `9.3709e-5` (order `1.340`), with the
nine-cell stencils `3.6361e-4`, `9.6096e-5` (`1.920`); `R_b` `8.5332e-5`, `3.4255e-5` (`1.317`). Without the transient
Rhie-Chow term, levels 1 to 3: the error with the library stencils `3.2150e-4`, `9.5222e-5`, `5.2566e-5` (`1.755`,
`1.465`), with the nine-cell stencils `3.2800e-4`, `8.5605e-5`, `3.8994e-5` (`1.938`, `1.939`). Run times: kinematic and
CM 144 s; T 952 s (level 4: 655 s); the switches 617 s; the families 866 s; amendment 1 323 s, 926 s, 1180 s; amendment
2 481 s, 228 s (the machine was shared with other builds and up to three of these ran at once).

### Mutations

Applied alone to a `git archive` copy of `418a5738` (`E:/projects/cfd/w49i-work/f334-scratch/mut`, own target directory)
by `mutate.py` (an exact replacement that must occur once), the named test run once, the file restored and compared byte
for byte (equal in all four).

| Id | Must fail | Result |
| --- | --- | --- |
| MU1 the offset `delta` not applied | C1 | failed: continuity `4.94e-3` (`c_rl_level_1_continuity_and_start`) |
| MU2 inverse-distance weights | K1 | failed: `0.97`, `1.95`, `2.92` against `0.02` |
| MU3 linear weights in the nine-cell stencil of the test | K2 | failed: the biquadratic field `9.4e-3`; orders `1.985`, `1.993` |
| MU4 the fringe `HbyA` not interpolated | CM | failed: `3.96e-2`, `5.96e-2` |

`scripts/limit_audit.py`: no hit in the new file.

### Deviations and disclosures

- The unchanged tree was compiled once before the registration was committed (stated in its status line); nothing ran.
- Before each run the tests were compiled without running, laid out by rustfmt and linted.
- My predictions that were wrong: `e_4` (predicted `2.6e-5` to `3.1e-5`, measured `2.49e-5`); the rule for (a1). I had no
  prediction for Hq, Hf and Hn, and my hand analysis (a second-order truncation cannot give the term) is contradicted by
  the measurement and not repaired.
- The test of T was renamed to `t_rl_levels_1_to_4_known_miss` and ignored after its one run (amendment 1); it was not
  run again. The gate-run check `c_rl_level_1_continuity_and_start` was added after the runs to give MU1 a fast check; it
  repeats the level-1 run.
- The pairs 2 to 3 and 3 to 4 of TQ are formed here from the printed numbers of three test processes, as amendment 1
  says; only the pair 1 to 2 is asserted in a test.
- The switches test did not print the errors with the cell mean; amendment 1 printed them.
- Deterministic runs were repeated where tests share them (RL at levels 1 to 4); every repeat printed the same digits.
- The nine-cell stencil is test code for two uniform Cartesian parts. It is not a library interpolation.

### Not run

- The existing overset suites and the rest of `rcfd-fv`: the library is unchanged; the lead runs the gate. The new file
  without the ignored tests ran once on the final tree: 3 passed, 8 ignored, 48 s.
- No level above 4, no moving component, no cylinder case, no corrective in the library: none is registered.
- The four interpolated quantities were not switched apart (`u` against `HbyA`); the pressure is excluded by Hn.
- `Qc` and `Qb` at level 4.

### After the tests

`cargo fmt -p rcfd-fv --check` clean. `cargo clippy -p rcfd-fv --all-targets -- -D warnings` clean (the only crate
touched, tests only).

### Decision by the registered rule

**Partial.** T is not met on the stencils of the library (`1.277`, `1.166`, `1.626`), and the source of the term is named
by a hypothesis confirmed by its registered number: Hq, the truncation of the linear donor interpolation (`rho_Qcb =
0.179`), borne out on four levels by TQ (`1.922`, `1.935`, `1.950`). The controls held and the four mutations failed
their checks. Said plainly: the term is located, measured and removable by a third-order interpolation that exists only
in the test; no library code was changed; the mechanism by which the response to a second-order truncation stays below
second order (`R` at `1.59` to `1.80`, its background-fringe part at `1.09` to `1.15`) is not found, and H12, the reading
that there is no first-order term at all, is undecided.

### Open questions (each with options and a recommendation)

- **Q1. The status of F-334.** Options: (a) partial, as the rule gives; (b) done on the second branch of the acceptance
  (the source is named with a number). Recommendation (a): the mechanism is open and the order check is missed on the
  stencils of the library. Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation); not a
  decision the maintainer made in person, and (b) is the maintainer's to choose.
- **Q2. A donor interpolation of higher order in the library (F-336).** It is now the known way to the order check.
  Options: (a) do F-336 next and add RL on levels 1 to 4 to its acceptance (text below); (b) leave F-336 optional.
  Recommendation (a), taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation). It stays
  an opt-in.
- **Q3. Should the higher-order interpolation become the default once it exists?** It would change a default, and it
  makes the error larger on the coarse levels (factor 2.1 at level 1) while restoring the order. Not taken: open, for the
  maintainer.
- **Q4. The mechanism.** Options: (a) a follow-up item on the response to the background-fringe truncation (text below);
  (b) leave it recorded, because a third-order interpolation removes the term whatever its mechanism. Recommendation (a)
  at low priority, after F-336, taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).
- **Q5. The order check of the coupling.** The root mean square of the calculated component cells with the library
  stencils is a difference of two parts that cancel to 88 to 96 percent, so its observed order says little about either.
  Options: (a) keep T as a known miss and take TQ (once F-336 exists) as the order check; (b) check the two parts
  separately (`e_Q` and `R`). Recommendation (a), taken on the maintainer's standing instruction of 2026-11-11 (take the
  recommendation).

### For the lead: roadmap entries

No roadmap file was edited on this branch.

**F-334**, proposed status: `partial`. Proposed note:

> From ADR 0232 question 3, recommendation taken on the maintainer's standing instruction of 2026-11-11. ADR 0246 (registration of F-334, 2026-11-12, branch f334-overset-first-order-term): PARTIAL, a diagnosis, tests only, no library change. The order check is missed: RL on levels 1 to 4 gives 1.549e-4, 6.39e-5, 3.98e-5, 2.49e-5 in the root mean square of the calculated component cells, orders 1.277, 1.166, 1.626 (limit 1.7); the fit of ADR 0232 predicted 2.86e-5 at level 4. The source is named: the truncation of the linear donor interpolation. With stencils exact for biquadratic fields on both fringes (nine cells, test code) the error is 3.24e-4, 8.55e-5, 3.90e-5, 2.23e-5, orders 1.922, 1.935, 1.950, and equals the error of the background alone. The error with the library stencils is the sum of that part and of the response to the truncation, which is 1.35 to 1.90 times larger, opposed to it (cosine -0.96 to -0.88) and converges at 1.693, 1.588, 1.800; they cancel to 88 to 96 percent, so the library stencils give the smaller error at levels 1 and 2 and the lower order. The coefficient 1.0e-4 is the increment per level of that response times L^2 (1.04e-4, 0.98e-4, then 0.38e-4). Its part from the background fringe is of first order on levels 1 to 3 (4.9e-5, 2.2e-5, 1.41e-5: 1.153, 1.093) and still growing at t = 0.5; the part from the component fringe converges at 1.782, 1.698. Ruled out with numbers: a time error, the single-mesh solution, the outer boundary of the component, the reference cell, the interpolated pressure and the convection (the error is linear in the amplitude to 0.07 percent), a mass defect, the flux at the fringe faces, the transient Rhie-Chow term; with coincident meshes the coupling gives the background alone to 7e-9. Open: why the response to a second-order truncation stays below second order. Controls held, four mutations caught. Follow-ups (ADR 0246 questions 2 and 4, recommendations taken on the maintainer's standing instruction of 2026-11-11): F-336 with RL added to its acceptance, and a new item on the response to the background fringe.

**F-134**, status unchanged (`partial`). In its note, replace the sentence "Found there and open: a first-order term of the calculated component cells with the coefficient 1.0e-4." by:

> Found there and traced by ADR 0246 (F-334) to the truncation of the linear donor interpolation: with the component at rest the root mean square of the calculated component cells converges at 1.277, 1.166, 1.626 on levels 1 to 4 and at 1.922, 1.935, 1.950 with a third-order interpolation supplied by the test; the mechanism is open.

**F-336**, proposed new acceptance (the lead's to take or leave):

> An opt-in donor interpolation that reproduces quadratic fields; with it the largest error over the calculated and fringe cells of TG_L converges at order 1.7 or more on levels 1 to 3 with dt proportional to h^2, the root mean square of the error of the calculated component cells of RL of ADR 0232 converges at order 1.7 or more on levels 1 to 4 (ADR 0246 TQ: 1.922, 1.935, 1.950 with test-supplied stencils), the uniform-flow check U1 of ADR 0219 holds, and the default stencils and results are bit-identical.

Proposed follow-up item (the number is the lead's): **Overset: the response to the truncation of the background-fringe interpolation.** Depends on F-334. Acceptance:

> The response R_b of ADR 0246 (the difference of the RL runs with the library stencils and with a third-order interpolation on the background fringe: 4.87e-5, 2.19e-5, 1.41e-5 at levels 1 to 3, orders 1.153 and 1.093) is traced to a named term of the coupled step by a switch that brings its order to 1.7 or more on levels 1 to 4, or is shown to reach order 1.7 on finer levels or at a later time, or the ADR names why not with a number.
