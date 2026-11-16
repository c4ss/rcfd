# ADR 0667: F-744 registration: non-orthogonal correction on coupled faces

Status: **REGISTERED 2026-11-16, written before any code of F-744 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-744 was written and no run of any kind was made; the numbers quoted
are those measured and printed in ADR 0661 and ADR 0664. Results go in the addenda at the end; nothing above the addenda is
changed after a run.

Roadmap item F-744 (deps F-743), proposed by [ADR 0661](0661-f725-registration-sector-against-full-annulus-with-a-non-axisymmetric-inlet.md)
remaining question 3 (Q4). Parents: ADR 0661 (F-725: the lockstep of the sector SS against the full annulus P from the start
`u0` consistent with the inlet, the variants `V(K, N)`, Q4 the whole of what remains after the lag: `V(6, 6)` `5.99e-3`,
`V(6, 0)` `1.48e-4`), [ADR 0664](0664-f743-registration-implicit-momentum-coupling-of-periodic-pairs.md) (F-743: the opt-in
`PimpleSettings::rotational_periodic_implicit`, which removes the lag in one outer iteration; `V*(1, 6)` within `2.4e-4` of
`V(6, 6)`), [ADR 0629](0629-f304-registration-planar-and-sector-sliding-interfaces.md) (F-304: the periodic join, its Q4: the
coupled faces carry the orthogonal coupling only), [ADR 0077](0077-wave35b-solver-robustness-under-automation.md) (F-H0: the
partner-cell point of a coupled face in the least-squares gradient of the steady kernel). Number 0667 given by the lead. Branch
`f744-work` from main e326c8ba (F-725 and F-743 merged), worktree `w49i-work/f744`, scratch `w49i-work/f744-scratch`.

House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here; each registered run made once; existing tests not edited;
defaults bit-identical; no edit of `baseline_hashes.rs`, of a hash table or of a golden; no new dependency, no unsafe code;
clean-room; the roadmap files are not edited. Every choice the maintainer would normally be asked about is taken on the
maintainer's standing instruction of 2026-11-11 (take the recommendation), except where listed as open.

Clean-room: no source outside the repository was read. The correction is the over-relaxed split of the face gradient that the
repository's own `diffusion.rs` uses on internal faces (`Delta = (S.S / S.d) d`, `k = S - Delta`, the textbook decomposition
of the finite-volume diffusion flux on non-orthogonal meshes, cited from memory as the over-relaxed approach of Jasak's 1996
thesis; only the repository's form is used), applied to the pair of boundary faces with the partner cell mapped into the
owner's frame; the derivation is below. The references are the repository's own runs (ADR 0661, ADR 0664).

## What F-744 asks

The acceptance, binding (from the lead's brief):

> An opt-in explicit non-orthogonal correction on the coupled (wrap and seam) faces in the pressure equation and the momentum diffusion, defaults bit-identical; on ADR 0661's lockstep with the lag removed the sector's difference from the full annulus falls from 6.0e-3 to within a registered limit of ADR 0661's V(6, 0) (1.5e-4) on every step. Every limit not stated here is registered in the ADR before the run.

Clauses:

- **b1** an opt-in explicit non-orthogonal correction on the coupled faces (seam and wrap pairs) in the pressure equation
  (and the flux it gives) and in the momentum diffusion;
- **b2** the defaults are bit-identical;
- **b3** on ADR 0661's lockstep (from `u0`, 400 steps) with the lag removed and the correction on, the sector's largest
  difference from the annulus is within a registered limit of `V(6, 0)`'s on every step (from `6.0e-3` without it).

**How the lag is removed (registered).** By ADR 0664's opt-in with one outer iteration (the note of ADR 0664 for F-744), not by
six outer iterations: the acceptance variant is `V**(1, 6)`: `n_outer = 1`, `non_orth_correctors = 6`,
`rotational_periodic_implicit = true` and the new setting on, in both solvers. ADR 0664 showed that its `V*(1, 6)` (the same
without the new setting) stays within `2.4e-4` of `V(6, 6)`, and it costs a sixth.

**Reading of b3 (registered).** As ADR 0664 read its a3: with `G_V(n) = max_c |delta_V(n)_c| / U_in` (`delta_V(n)` the field
`u_C - u_A` of variant `V` at step `n`, every sector cell against its nearest annulus cell), the check is `|G_**(n) -
G_(6,0)(n)| <= eps` on every one of the 400 steps; the field form `max_c |delta_**(n) - delta_(6,0)(n)| / U_in` is registered
beside it as a supporting check. "Falls from 6.0e-3" is checked as the largest `G_**` over the steps, whose limit follows from
the first check and `V(6, 0)`'s `1.48e-4`.

## What the code read shows (e326c8ba, read only)

1. `diffusion::laplacian`: on an internal face the flux out of the owner is `gamma [lap (phi_N - phi_P) + k . grad_f]` with
   `lap = |S|^2 / (S . d)`, `d = c_N - c_P`, `k = S - d |S|^2 / (S . d)`, `grad_f = w grad_P + (1 - w) grad_N`
   (`w = geom.weights`); the `k` term goes to the right-hand side (`b[P] += gamma k . grad_f`, `b[N] -=`), only when a cell
   gradient is passed. On a boundary face only Dirichlet faces (`a = 0`) get a `k` term; a coupled face (`bc::Periodic`, `a =
   w = dn_q / span` in `(0, 1)`) gets none: its flux is `gamma |S| (q - phi_P) / span`, `span = dn + dn_q`.
2. `pressure::solve_pressure_inner`: every corrector pass computes the least-squares gradient of the current pressure with
   `boundary_values_pbc` (on a coupled face the value `a p_P + (1 - a) p_partner` at the face centre) and assembles the
   Laplacian with it; the coupled pair enters implicitly with `gamma_eff |S| c` (`extras`, `gamma_eff = coupled_gamma`, the
   pair's distance-weighted mean, equal on both faces). `corrected_flux_impl`: internal faces `phiHbyA - gamma (lap (p_N - p_P) +
   k . grad_f)` with the least-squares gradient of the new pressure; coupled faces `phiHbyA - gamma |S| (p_g - p_f) / span`, no
   `k` term.
3. `pimple.rs` `step_with`, momentum predictor: per component `k` one least-squares gradient of the component
   (`least_squares(&g, &conv_cells, &bv)`, `bv` the boundary values: on a coupled face the face value) when
   `convection.needs_gradient() || non_orth_correctors > 0`, used by the convection and by `laplacian_opt` (the momentum
   diffusion's `k` term on internal faces); with `Central` and `non_orth_correctors = 0` no gradient and no correction anywhere.
   The pressure solve is `solve_pressure` (no gradient options, no immersed source) and the flux `corrected_flux(...,
   non_orth_correctors > 0)`; the velocity correction uses `green_gauss` (unchanged here).
4. `gradient::least_squares_partner` (ADR 0077 F-H0): on the listed coupled faces the fit takes the partner cell's centre mapped
   into the owner's frame, displacement `(x_f - c_P) + R (c_q - x_g)` and value `R u_q` (scalar: `p_q`), in place of the face
   centre and the face value; empty list = `least_squares` bit for bit. `periodic_partner_points` builds the list for a
   vector field; only the steady kernel uses it.
5. `geometry.rs`: boundary `delta[f] = x_f - c_P`, `dn = |S . delta| / |S|`; `periodic_partner[pi]` (partner face of every face
   of a coupled patch), `periodic_rot[pi]` (`R`, partner frame to this face's frame; `None` = translation; `field::rotate`).
6. ADR 0661: the wrap pairs' non-orthogonality `|d x n| / |d|` up to `0.750`, the seam pairs `2.3e-14` off the interface rings
   and `9.08e-4` at them; `V(6, 6)`'s largest `5.99e-3` at the rotor's outer ring next to the stator seam (the wrap pairs);
   `V(6, 0)` `1.48e-4` (largest over 400 steps from `u0`). ADR 0664: `V*(1, 6)` from `u0` largest `5.891e-3` (step 102), step 1
   `7.90e-4`; the annulus with ADR 0664's setting on equals the annulus with it off bit for bit.

## Design (derived)

**The pair as an internal face.** Let `f` be a coupled face of owner `r`, `g` its partner face of owner `c`, `R` the rotation of
the pair (`I` for a translation), `S = S_f`. Map the partner cell into `r`'s frame: the image centre is `c_r + d` with

`d = (x_f - c_r) + R (c_c - x_g)`.

On the full annulus the same two cells share one internal face with `S` and centre-to-centre vector `d` (ADR 0661 read 1). The
pair's faces satisfy `R S_g = -S` (the partner face is the same polygon, oriented outward from `c`), so `S . R (c_c - x_g) =
-R S_g . R (c_c - x_g) = S_g . (x_g - c_c) = |S| dn_g`, and `S . d = |S| (dn_f + dn_g) = |S| span`: the orthogonal coefficient
`|S|^2 / (S . d)` is the repository's `|S| / span`, unchanged. The non-orthogonal remainder is

`k_f = S - d |S|^2 / (S . d)`,

and the full face flux of a scalar `phi` out of `r` is `gamma (|S| / span (phi_c - phi_r) + k_f . grad_f)`, `grad_f = w grad_r
+ (1 - w) R grad_c`, `w = dn_g / span` (the owner weight `a` of `bc::Periodic`), `grad_c` in `c`'s frame. For the partner face
`g`: `d_g = -R^T d`, `S_g . d_g = S . d`, so `k_g = -R^T k_f`, and `grad_g = (1 - w) grad_c + w R^T grad_r = R^T grad_f`, giving
`k_g . grad_g = -k_f . grad_f`: the pair carries exactly opposite corrections (conservative), and the pair is one internal face
of the annulus, term for term.

**Vector components (momentum).** With `G_c` the velocity-gradient tensor of `c` in `c`'s frame (`(G_c)_ij = d u_i / d x_j`,
row `i` the gradient of component `i`), its form in `r`'s frame is `R G_c R^T`, whose row `k` is `sum_i R_ki R grad u_i(c)`. So
the face gradient of component `k` is `w grad u_k(r) + (1 - w) sum_i R_ki R grad u_i(c)` (inactive components skipped: their
cell gradients are not formed; `R` about the inactive axis does not mix them), and the momentum diffusion of component `k`
gets `b[r] += gamma_f k_f . grad_f,k`, the sign and form of the internal faces (read 1).

**The gradient (registered choice, Q1).** The cell gradient the correction uses is the least-squares gradient with the partner
point of ADR 0077 F-H0 on every coupled face (`least_squares_partner`; for the velocity the rotated partner value, as
`periodic_partner_points`), for the coupled and the internal faces' `k` term alike, in the pressure equation, its flux and the
momentum diffusion. Reason: on the annulus the cell next to a wrap face fits its gradient through the neighbour cell; the face
value at the face centre that `least_squares` uses on a coupled face is the mean of the two cells, which sits on the segment
`d`, not at the face centre: on the wrap pairs (non-orthogonality `0.75`) the default fit is wrong by O(1) of the gradient (ADR
0077 D0, `0.17 Omega` on its seams), and `|k_f|` is of the order of `|S|` there, so the correction would be wrong at first
order. With the partner point the fit of each cell takes the same displacements and values as the annulus's, and the correction
of every face is the annulus's. The convection keeps its gradient (read 3: `Central` has none).

**Where it acts.** A new field `PimpleSettings::coupled_non_orth: bool`, default `false`, read in `PimpleSolver::step_with`
(so also `step_ale`, `step_sliding`, `step_sliding_periodic`):

- `rcfd_fv::pressure::CoupledNonOrth` (new): per coupled boundary face (a patch whose condition `is_coupled()` with a partner
  map; not empty) `f, r, c, R, w, d, k_f`; built from the geometry with the patches of the pressure (`p.bc(pi).is_coupled()`,
  the faces of `PressureBc::coupled`) and separately with those of the velocity;
- pressure: with the setting and `non_orth_correctors > 0`, `solve_pressure` is replaced by a new `solve_pressure_coupled`,
  the same solve whose every corrector pass takes the partner-point gradient (through the existing gradient hook) and adds
  `gamma_eff k_f . grad_f` to `b[r]` of every coupled face (`solve_pressure_inner` gets one more argument, `None` from every
  existing caller); the flux is `corrected_flux_with(..., Some(partner gradient of p_new))` with `-gamma_eff k_f . grad_f`
  added on every coupled face;
- momentum: with the setting and a gradient formed (read 3), the partner-point gradients of all active components are formed
  from the same values as the default gradient (`conv_cells`, `ub`), `laplacian_opt` gets the component's partner-point
  gradient, and every coupled face of the velocity adds `gamma_nu,f k_f . grad_f,k` to `b[r]` of component `k`;
- with no coupled face the opt-in changes nothing but the gradient's code path (`least_squares_partner` with an empty list is
  `least_squares` bit for bit): a no-op bit for bit;
- refused (assert, as `solve_pressure_implicit_non_orth` refuses periodic faces): the setting with `gradient_pressure`,
  `gradient_momentum`, an immersed volume source in the pressure solve, or the consistent correctors with non-orthogonal
  correctors (each has its own non-orthogonal path this item does not extend).

Not changed: the default path (every new operation behind `s.coupled_non_orth`), the pressure-lag estimate and the adaptive
corrector count (their pressure operator stays orthogonal on coupled faces), the fractional step, the steady solver, the
velocity correction's Green-Gauss gradient, the convection.

## Estimates (before any run)

- **b3 (N1, N1J).** `delta_**(n) - delta_(6,0)(n) = (u_C** - u_A**) - (u_C60 - u_A60)`. With the correction the sector's
  coupled faces are the annulus's internal faces term for term (above), and the gradients the correction uses are the
  annulus's at every cell (same points and values); the iteration of the six correctors is the same fixed-point iteration in
  both. What is left of `delta_**` is the rest `R` of ADR 0661 (the Green-Gauss gradients of the seam and wrap cells in the
  momentum's pressure term and in the velocity correction, the old-level Rhie-Chow and `ddt` corrections on internal faces only)
  at `(K, N) = (1, 6)`, against `R` at `(6, 0)`: `M(R) = 1.48e-4`. The lag is gone in both (ADR 0664 to `2.4e-4` of `V(6, 6)`,
  whose remainder ADR 0664 traced to the outer-count change of Q4, which is removed here). `R` at `(1, 6)` against `(6, 0)`: the
  same mechanisms, assumed to change by up to 100 percent with the corrector and outer counts: `|G_** - G_(6,0)| <= 1.5e-4`.
  Limit **`eps = 3e-4`** per step (margin 2; twice `V(6, 0)`'s largest), for the 400 steps and for the fast 16 steps alike.
  The field form: up to `|R_**| + |R_(6,0)| = 3e-4`; limit **`5e-4`**.
- **N2.** Largest `G_**` over the steps `<= 1.48e-4 + 3e-4`: limit **`4.5e-4`** (a thirteenth of `6.0e-3`).
- **Without the correction (controls).** `V*(1, 6)` against `V(6, 0)`: `5.89e-3 - 1.48e-4 = 5.7e-3` at its worst step; limit
  of the control **`>= 3e-3`**. Over 16 steps from `u0`: step 1 alone `7.90e-4 - G_(6,0)(1) >= 7.90e-4 - 1.48e-4 = 6.4e-4`;
  limit **`>= 5e-4`** (above `eps`, so the fast check discriminates).
- **The momentum part (for MN3).** On a wrap face `nu |k| |grad u| ~ 0.1 |S| 0.7` against the orthogonal diffusion flux
  `nu |S| / span Delta u ~ 0.1 |S| 0.05 / 0.06`: the same order, so the momentum part of Q4 is not small and a sign error in it
  is expected to show above `eps` within 16 steps.
- **Cost.** `V**(1, 6)` about `V*(1, 6)` plus the gradients (ADR 0664: 75 s for three variants over 16 steps including set-up);
  `V(6, 0)` about a quarter of ADR 0661's I1 (1092 s for four variants): the 400-step run of the three variants about 600 s: a
  heavy record (`#[ignore]`, run once); the fast test (16 steps, the same three variants and code path) about 60 s.

## Checks (limits fixed now)

Variants: `V**(1, 6)` (the acceptance variant), `V(6, 0)` (ADR 0661's reference), `V*(1, 6)` (ADR 0664's variant, the new
setting off: control and default check). Solver settings PS6 of ADR 0629 as in `sliding_annulus_inlet_f743.rs` (`Central`,
`TIGHT` solvers), both solvers of a variant the same settings.

| Id | Clause | Set-up | Check | Limit |
| --- | --- | --- | --- | --- |
| N1 | b3 | heavy record: the three variants from `u0`, 400 steps | on every step `abs(G_**(n) - G_(6,0)(n))` | `<= 3e-4` each step |
| N2 | b3 ("falls from 6.0e-3") | same run | largest `G_**(n)` over the steps | `<= 4.5e-4` |
| N1f | b3 (supporting) | same run | on every step `max_c abs(delta_**(n) - delta_(6,0)(n)) / U_in` | `<= 5e-4` each step |
| D0h | b2 | same run | largest `G_(6,0)` reproduces ADR 0661's `1.48e-4`; largest `G_*` ADR 0664's `5.891e-3` | within `5e-7`, `5e-7` |
| NC1 | control of N1 | same run | `max_n abs(G_*(n) - G_(6,0)(n))` | `>= 3e-3` |
| A1s | supporting | same run | C's `flux_residual` of `V**(1, 6)` from step 2 | `<= 1e-12` |
| N1J | b3 (fast form) | fast: the three variants from `u0`, 16 steps | on every step `abs(G_**(n) - G_(6,0)(n))` | `<= 3e-4` each step |
| N1fJ | b3 (fast form) | same | on every step the field form | `<= 5e-4` each step |
| D0J | b2 | same | `G_*(1)` reproduces ADR 0664's `7.90e-4` | within `5e-7` |
| D2 | b2 (no-op without coupled faces) | same | the annulus of `V**(1, 6)` against that of `V*(1, 6)`, velocity and pressure, every step | bit for bit |
| NC1J | control of N1J | same | `max_n abs(G_*(n) - G_(6,0)(n))` over the 16 steps | `>= 5e-4` |
| G0 | set-up | every variant, every step | every C cell's nearest A cell within `1e-9`, one-to-one | as stated |
| H2 | set-up of b1 (geometry) | the sector's and the annulus's meshes of steps 0 to 16 | every coupled face (velocity's list) has an annulus internal face with the same centre (`<= 1e-9`); its `d` and `k_f` equal the annulus face's `d`, `k` (oriented from the matched owner) | `<= 1e-10` each |
| U1 | b1, b2 | unit test, the doubly periodic Taylor-Green box of `pimple.rs` (translational pairs, orthogonal), 5 PISO steps, `non_orth_correctors = 2`, setting on against off | velocity and pressure (`k_f = 0` to rounding there) | `<= 1e-12` |
| B1 | b2 | `cargo test -p rcfd-api --release --test baseline_hashes`; `rcfd-fv --lib`; the named periodic, sliding and non-orthogonal suites (addendum) | pass | as stated |

Printed, no limit: every variant's `G` at step 1 and its largest with the step; the pairwise field differences; the step and
sign of the largest `G_** - G_(6,0)`; elapsed times; H2's largest `|k_f| / |S|` (the wrap pairs' non-orthogonality).

Mutations (on a scratch copy of the library, after the checks; each applied alone; the fast test run once per mutation; each
must fail N1J or N1fJ):

- **MN1** the pressure equation without the coupled correction (the flux keeps it).
- **MN2** the partner's gradient not rotated in `grad_f` (pressure and momentum).
- **MN3** the momentum's coupled correction with the opposite sign.
- **MN4** the coupled faces' flux correction with the opposite sign (the equation keeps it).

Diagnostic mutation (reported, not required to fail, not in the decision rule): **MN5** the plain least-squares gradient (face
values) in place of the partner-point gradient (Q1's alternative).

## Decision rule

**Done** when N1 and N2 (b3) are met, N1J, N1fJ, D0h, D0J, D2, NC1, NC1J, G0, H2, U1 and B1 are met and MN1 to MN4 are
caught; N1f and A1s are supporting and reported. **Partial** when N1 and N2 are met but a supporting or fast check, a set-up
check, a control or a mutation is missed; or when N1 or N2 is missed with its cause diagnosed. **Todo** when N1 and N2 are both
missed without a diagnosis, or b2 fails (D0h, D0J, D2 or B1).

## Limits (not covered)

- No case-file key and no API field: the setting is reachable from `PimpleSettings` only (a key changes the schema golden; Q3).
- The default is not changed (it changes every periodic transient result with non-orthogonal correctors; Q2).
- Refused combinations (gradient options, immersed volume source, consistent correctors with non-orthogonal correctors) and
  the fractional step, the steady solver, the pressure-lag estimate and the adaptive corrector count keep the orthogonal
  coupling.
- The velocity correction's and the momentum pressure term's Green-Gauss gradients keep the face value on coupled faces (part
  of `R`, not touched).
- One mesh pair (SS against P), one time step, one inlet modulation; 2-D (one layer).

## Open questions

- **Q1. The gradient of the correction.** (a) the partner-point least-squares gradient (ADR 0077 F-H0) for the correction
  (taken: the annulus's gradient at every cell; the default fit is wrong at first order on the wrap pairs); (b) the default
  least-squares gradient (face values). Taken (a) on the maintainer's standing instruction of 2026-11-11 (take the
  recommendation); MN5 reports (b)'s effect.
- **Q2. The default.** (a) keep the orthogonal-only default (taken: the house rule); (b) make the correction the default where
  non-orthogonal correctors are on. Not taken: it changes default results. Recommendation (b) together with ADR 0664's Q1 (the
  implicit pair by default, F-746), in one change with the hashes re-recorded by the maintainer.
- **Q3. A case key.** (a) none (taken: a key changes the schema golden); (b) a key beside F-746's. Open for the maintainer.

## Addendum 1: results (2026-11-16)

Measured against the limits registered above; no limit was changed and no amendment was made. Release builds on Windows, on a
machine shared with other building agents. The registration is 89e179f1 (alone, first); the library change and the tests
04ded908 (committed before their run). Each registered run was made once: the fast tests (16 s), the heavy record (291 s,
`--ignored --test-threads=1`; under the 600 s estimate), `rcfd-fv --lib` (U1 in it; U1 was then run once more alone to read its
printed line, deterministic, same result). One diagnostic run followed the mutations (disclosed below). Logs in
`w49i-work/f744-scratch` (`fast.log`, `heavy.log`, `lib.log`, `suites.log`, `baseline.log`, `mutations.log`, `diag.log`).

**Built as registered.** `PimpleSettings::coupled_non_orth` (default `false`) in `crates/rcfd-fv/src/pimple.rs` (the field and
its default; in `step_with` the momentum's coupled faces and partner-point gradients, the Laplacian's gradient, the momentum
right-hand side, the pressure solve and the flux behind the setting; unit test U1); in `crates/rcfd-fv/src/pressure.rs`
`CoupledNonOrth`, `solve_pressure_coupled`, `corrected_flux_coupled` and one more argument of the private
`solve_pressure_inner` (`None` from every existing caller); the new test file
`crates/rcfd-fv/tests/sliding_annulus_inlet_f744.rs` (the fast test, H2, the heavy record). With the setting off every changed
line of the step takes the previous arithmetic. No case key, no golden, no hash table.

### Results

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| N1 (b3) `abs(G_**(n) - G_(6,0)(n))`, every one of 400 steps from `u0` | `<= 3e-4` each step | `3.80e-5` (step 9; `G_**` smaller) | met |
| N2 (b3) largest `G_**` | `<= 4.5e-4` (from `6.0e-3`) | `1.104e-4` (step 9) | met |
| N1f (supporting) field form, every step | `<= 5e-4` | `3.94e-5` | met |
| D0h largest `G_(6,0)`; largest `G_*` | `1.48e-4`, `5.891e-3` within `5e-7` | `1.483922e-4`; `5.890778e-3` (step 102) | met |
| NC1 (control) `max abs(G_* - G_(6,0))` | `>= 3e-3` | `5.86e-3` | held |
| A1s C `flux_residual` of `V**(1, 6)` from step 2 | `<= 1e-12` | `2.61e-15` | met |
| N1J fast, 16 steps | `<= 3e-4` each step | `3.80e-5` | met |
| N1fJ fast, field form | `<= 5e-4` each step | `3.94e-5` | met |
| D0J `G_*(1)` | `7.90e-4` within `5e-7` | `7.896331e-4` | met |
| D2 the annulus with the setting on against off, 16 steps | bit for bit | equal on every step | met |
| NC1J (control) 16 steps | `>= 5e-4` | `4.65e-3` | held |
| G0 cell match, every variant, every step | `<= 1e-9`, one-to-one | `1.45e-15` (fast `9.5e-16`), yes | met |
| H2 coupled faces as annulus faces, 17 meshes, 606 faces | centre `<= 1e-9`; `d`, `k` `<= 1e-10` | `9.5e-16`; `1.5e-15`, `1.3e-16`; none unmatched or misoriented; largest `abs(k) / abs(S)` `1.107` | met |
| U1 Taylor-Green box, setting on against off | `<= 1e-12` | velocity `5.6e-16`, pressure `3.6e-14` (64 coupled faces, `abs(k) <= 7e-16`) | met |
| B1 `baseline_hashes`; `--lib`; the named suites | pass | see below | met |

Printed, no limit: step 1 `G_**` `3.8e-15` (the sector equals the annulus to rounding on the first step), `G_(6,0)` `6.6e-8`,
`G_*` `7.9e-4`; `G_**` and `G_(6,0)` both peak at step 9 (`1.10e-4`, `1.48e-4`), the start transient of the rest `R`. The
correction takes the sector's largest difference from `5.89e-3` to `1.10e-4`, below `V(6, 0)`'s own. Against the estimates: N1
`3.8e-5` against `1.5e-4`.

Suites (release, run once, all passed): `periodic` 3, `sliding_periodic_f304` 8, `sliding_periodic_coincidence_f720` 4,
`sliding_periodic_coupled_f721` 2, `sliding_annulus_inlet_f725` 3, `sliding_annulus_inlet_f743` 1, `consistent_periodic_f367`
13, `seam_no_harm_f344` 6, `w35a_seam` 2, `mrf_cyclic_seams_w44c` 12, `rotating_seam_h1_w45c` 2, and the non-orthogonal pimple
suites that run the changed lines `piso_thin_gap_f299` 18, `gradient_pressure_f291` 3, `non_conformal` 2,
`sliding_coincidence_f710` 3 (ignored records not run); `rcfd-fv --lib` 115 passed, 6 ignored; `rcfd-api` `baseline_hashes` 3
passed, 1 ignored. `cargo fmt -p rcfd-fv --check` clean; `cargo clippy --release -p rcfd-fv --all-targets -- -D warnings`
clean.

### Mutations (scratch copy `w49i-work/f744-scratch/mut` of 04ded908, script `mutate.py`, target `target-mut`; the fast test run once per mutation)

| Mutation | Must fail | Result |
| --- | --- | --- |
| MN1 the pressure equation without the coupled correction | N1J or N1fJ | **not caught** (N1J `5.5e-5`, N1fJ `6.1e-5`) |
| MN2 the partner's gradient not rotated | N1J or N1fJ | caught: N1J `1.71e-3`, N1fJ `1.74e-3` |
| MN3 the momentum correction with the opposite sign | N1J or N1fJ | caught: N1J `9.09e-3`, N1fJ `9.11e-3` |
| MN4 the flux correction with the opposite sign | N1J or N1fJ | **not caught** (N1J `3.8012e-5` against `3.8008e-5` unmutated) |
| MN5 (diagnostic) the plain least-squares gradient | none | N1J `8.2e-5`, N1fJ `9.8e-5`: within the limits, about twice the partner-point form |

Two of four required mutations caught. D0J, D2, NC1J held under every mutation (the default path untouched). The file was
restored after each (equal to 04ded908 up to line endings).

**Diagnosis of MN1 and MN4 (one diagnostic run, disclosed).** The scratch copy printed, over the fast test's 16 steps, the
largest pressure correction `gamma k . grad_f p` on the coupled faces and the largest coupled-face flux: at most `1.17e-6`
against fluxes up to `4.7e-3`, a relative `2.5e-4`; the momentum correction is up to `2.7e-4` per face. On the wrap faces `k`
is nearly tangential (the faces are pieces of the interface circle, the offset to the partner's image is along it) and this
flow's pressure is nearly radial, so `k . grad p` is small: the pressure part of the correction is below what the lockstep
resolves, and Q4 here was almost all the momentum diffusion's (MN3). The registered checks are not defective; they are blind to
the pressure part on this flow. The pressure part is built and its geometry is checked (H2, and the antisymmetry by the
derivation), but its effect is not shown by a registered check.

### Not covered

As registered: no case key; the default unchanged; the refused combinations, the fractional step, the steady solver, the
pressure-lag estimate and the adaptive corrector count keep the orthogonal coupling; the Green-Gauss gradients of the velocity
correction and the momentum's pressure term unchanged; one mesh pair, one step, one inlet, 2-D. Not shown: the pressure part's
effect (above). `docs_site` not run (only prose under `docs/`: this ADR and one paragraph of `docs/dynamic-mesh.md`; left to
the daily gate).

### Decision

By the registered rule: N1 and N2 met, every fast, set-up and default check met, but MN1 and MN4 not caught: **partial**.

### Remaining questions

1. **The pressure part (MN1, MN4).** Options: (a) a follow-up item checking the pressure part on a flow with a tangential
   pressure gradient across non-orthogonal coupled faces, with a per-cell continuity check of the corrected flux; (b) accept
   the momentum result and record done. Recommendation (a): MN4 breaks the pairing of the equation and its flux, which a
   per-cell continuity check would see at once.
2. **The default (Q2, open).** Recommendation: together with F-746 (ADR 0664's implicit pair by default), in one change with
   the hashes re-recorded by the maintainer.
3. **A case key (Q3, open).** Recommendation: beside F-746's key.

## For the lead: roadmap entries

> Proposed status: partial.

> ADR 0667, 2026-11-16: non-orthogonal correction on coupled faces. The opt-in PimpleSettings::coupled_non_orth (default false) treats every coupled (seam and wrap) pair as the internal face between the owner and the partner cell's image (rcfd_fv::pressure::CoupledNonOrth: d = (x_f - c_r) + R (c_c - x_g), k = S - d |S|^2/(S.d), face gradient w grad_r + (1 - w) R grad_c, the partner's velocity-gradient tensor rotated) and adds its explicit non-orthogonal part to the pressure equation, its flux and the momentum diffusion, with the partner-point least-squares gradient of ADR 0077 F-H0; the coupled faces' d and k equal the full annulus's internal faces to 1.5e-15 on 606 faces (the wrap pairs' |k|/|S| up to 1.1). On ADR 0661's lockstep with the lag removed by ADR 0664's implicit pair in one outer iteration, the sector's largest difference from the annulus falls from 5.89e-3 to 1.10e-4 U_in over 400 steps, within 3.8e-5 of V(6, 0) on every step (limit 3e-4; the first step equal to rounding, 3.8e-15). Defaults bit-identical (the annulus bit for bit with the setting on, ADR 0661's and ADR 0664's numbers reproduced, baseline_hashes and the periodic, sliding and non-orthogonal suites pass). Partial: two of four mutations caught; the pressure part of the correction is below the lockstep's resolution on this flow (at most 2.5e-4 of the coupled-face flux: the wrap faces' k is tangential and the pressure nearly radial), so its effect is not shown; the momentum part carries the result. No case key; the default not changed (open, with F-746).

> Follow-up proposed: "Pressure part of the non-orthogonal correction on coupled faces". Deps F-744. Acceptance: on a flow with a tangential pressure gradient across non-orthogonal coupled faces (a swirling inlet on ADR 0661's lockstep, or a skewed translational periodic channel against its unrolled twin) the opt-in of ADR 0667 brings the periodic run within a registered limit of the reference on every step where the setting without its pressure part does not, and the corrected flux is divergence-free per cell within a registered limit; defaults bit-identical. Every limit not stated here is registered in the ADR before the run.
