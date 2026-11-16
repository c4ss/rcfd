# ADR 0661: F-725 registration: sector against full annulus with a non-axisymmetric inlet

Status: **REGISTERED 2026-11-16, written before any code of F-725 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no test code of F-725 was written and no run of any kind was made; the numbers quoted are those
measured and printed in ADR 0634, ADR 0641 and ADR 0659. Results go in the addenda at the end; nothing above the addenda is
changed after a run.

Roadmap item F-725 (deps F-721), proposed by [ADR 0641](0641-f721-registration-periodic-sliding-at-coinciding-nodes-without-the-parts-symmetry.md)
remaining question 2 (its F1 miss). Parents: ADR 0641 (F-721: the lockstep LS of the sector SS against the full annulus P,
`periodic_sliding_step_coupled`), [ADR 0216](0216-f250-registration-sliding-interface-on-a-cylinder.md) (F-250: the full
annulus P), [ADR 0629](0629-f304-registration-planar-and-sector-sliding-interfaces.md) (F-304: the periodic join; its Q4, the
coupled faces carry the orthogonal coupling only), [ADR 0634](0634-f704-registration-sliding-beyond-one-period-re-imaging-of-the-rotor.md)
(F-704: the lockstep against the annulus for a uniform inlet, `8.0e-3 U_in`), [ADR 0659](0659-f724-registration-threshold-scale-of-the-coupled-interface-solves.md)
(F-724: `periodic_sliding_step_coupled` on `FluxScale`, ADR 0641's sector run unchanged bit for bit). Number 0661 given by the
lead. Branch `f725-work` from main 9f06e866 (F-721, F-730 and F-724 merged), worktree `w49i-work/f725`, scratch
`w49i-work/f725-scratch`.

House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here; each registered run made once; existing tests not edited;
defaults bit-identical; no edit of `baseline_hashes.rs`, of a hash table or of a golden; no new dependency, no unsafe code;
clean-room; the roadmap files are not edited. Every choice the maintainer would normally be asked about is taken on the
maintainer's standing instruction of 2026-11-11 (take the recommendation), except where listed as open.

Clean-room: no source was read; no vendor code, text or install was used. The start state is a closed form (a source flow
whose strength varies with the angle, divergence-free by direct differentiation, below); the decomposition is an exact
telescoping identity of the lockstep differences; the estimates are first-principles bounds on the implicit-Euler cell
equation. The references are the repository's own full-annulus run (ADR 0216, with F-710's coupling) and its measured
numbers.

## What F-725 asks

The acceptance, binding (from the lead's brief):

> The lockstep of ADR 0641 (sector SS and the full annulus of ADR 0216, inlet periodic in the pitch, steps through node coincidences) with a limit on the largest velocity difference derived before the run to include the start transient at the seams (or from a start state consistent with the inlet), met on every step; the seam faces' orthogonal coupling (ADR 0629 Q4) identified as the source of the remaining difference or ruled out. Every limit not stated here is registered in the ADR before the run.

Clauses:

- **c1** the lockstep of ADR 0641 (SS through `periodic_sliding_step_coupled` against P through `sliding_step`, the inlet
  `U_in (1 + 0.5 sin 6 theta) x_hat`, `dt = pi/576`, 400 steps, 50 coincidence steps), with a limit on the largest velocity
  difference derived before the run, met on every step;
- **c2** the limit includes the start transient at the seams, **or** the runs start from a state consistent with the inlet;
- **c3** the orthogonal coupling of the coupled faces (ADR 0629 Q4) identified as the source of the remaining difference, or
  ruled out.

**Reading of c2 (registered; question Q1).** The second branch is taken: both runs start from the state `u0` below, which is
consistent with the inlet and divergence-free. The first branch (a limit for ADR 0641's own start `(r_i/r) x_hat` that
includes its transient) is not taken: ADR 0641 measured that run (`3.494e-2` on step 1), and any limit written for it now
would be written knowing the number. ADR 0641's start is still run here, but only for the diagnosis (J, below: which part of
the solver makes its step-1 difference), with no limit on its size.

**Reading of "the seam faces' orthogonal coupling" (registered).** ADR 0629 read 5 and Q4: on the coupled (periodic) boundary
faces the pressure equation and the momentum diffusion use the orthogonal coefficient `|S| / (dn_f + dn_g)` only, with no
explicit non-orthogonal correction, where the full annulus has internal faces that get the correction (`PimpleSettings::
non_orth_correctors`, six here). On the joined sector mesh the coupled faces are the seam pairs (the rotor's and the stator's
radial faces at the sector's ends) and the wrap pairs (the interface pieces beyond the stator's seam). c3 is read for both.

## What the code read shows (9f06e866, read only)

1. `rcfd_fv::geometry`: on an internal face `lap_coeff = |S|^2 / (S . d)` with `d` the centre-to-centre vector and the
   weight from the normal distances `d_o, d_n` to the face; on a boundary face `dn = |S . (x_f - c)| / |S|`. So
   `|S|^2 / (S . d) = |S| / (dn_o + dn_n)`: with no non-orthogonal corrector a coupled pair and an internal face between the
   same two cells have the same orthogonal coefficient and the same interpolation weight.
2. `rcfd_fv::pressure`: the coupled pair enters the pressure matrix implicitly with that coefficient (`extras`), its flux in
   `corrected_flux` is `phiHbyA - gamma |S| (p_g - p_f) / (dn_f + dn_g)` with no `k . grad p` term; internal faces get
   `gamma (k . grad p)_f` with `k = S - d |S|^2 / (S . d)` when `non_orth_correctors > 0`. `boundary_hby_a` gives the
   coupled face the distance-weighted mean of the two owners (the partner's vector rotated).
3. `rcfd_fv::bc::Periodic`: the momentum's face value on a coupled face is `w u_P + (1 - w) q`, the gradient
   `(q - u_P) / span`, with `q` the partner cell's value **from the current field** (lagged, `field.rs` `eval_coeffs`);
   `convection.rs` says the same of the central correction ("whose neighbour value is lagged"). The momentum predictor of a
   PISO step (`n_outer = 1`) therefore sees the partner's **old** value across every seam and wrap pair, where the annulus has
   the new value implicitly. With `n_outer = K > 1` the outer iterations re-assemble with the partner's latest value, so the
   lag converges away (the pressure coupling is implicit in either case).
4. `PimpleSettings::piso` has `alpha_u = alpha_p = 1`, `n_outer = 1`; outer iterations with a moving mesh are used by
   `ale_flows.rs` and `ale_bdf2_f345.rs`; `step_sliding_periodic` and `step_sliding` hand the step data to `step`, which runs
   `n_outer` outer iterations (pimple.rs line 1728) with the same step data.
5. The test file of ADR 0641 (`sliding_periodic_coupled_f721.rs`, not edited): the pairs, the solver set-up NS, the lockstep
   loop, the cell match. Its numbers (read): F1 `3.494e-2` on step 1 for both the coupled and the base sector, `2.67e-2` from
   step 2, at most `1.93e-2` in its diagnosis's later prints; F2 `8.63e-3` at step 400. ADR 0634: `8.03e-3 U_in` against the
   annulus for the uniform inlet from the consistent start `(r_i/r) x_hat`, attributed there to the wrap faces (Q4) without a
   test of the attribution.

## The start state consistent with the inlet (derived)

`u0(x) = U_in (1 + 0.5 sin 6 theta) (r_i / r) x_hat`, `x_hat` the radial unit vector, `r_i = 1`. In polar coordinates `u_r =
U_in f(theta) r_i / r`, `u_theta = 0`, so `div u = (1/r) d(r u_r)/dr + (1/r) d(u_theta)/d theta = (1/r) d(U_in f(theta) r_i)/dr
= 0`. At the inlet `r = r_i` it equals the inlet `U_in f(theta) x_hat` (the inlet profile of ADR 0641, evaluated at the face
centres, fixed in the absolute frame). It is not a solution of the momentum equation (the viscous term of the modulation and
the inertia of the varying strength are not balanced), so a transient remains, but it is a transient of the flow, the same
in both runs, not a jump between the boundary data and the field. Both runs (sector and annulus) start from `u0` at their
cell centres, `p = 0`.

## Design

No library change. One new test file `crates/rcfd-fv/tests/sliding_annulus_inlet_f725.rs`; `docs/dynamic-mesh.md` may get
one sentence. No case key, no golden, no hash table, no shared solver file (so `baseline_hashes` need not run; it is run
anyway as a check that nothing changed, B1).

**Variants.** A variant `V(K, N)` is ADR 0641's lockstep of the coupled sector C (`periodic_sliding_step_coupled`,
`step_sliding_periodic`) and the annulus A (`sliding_step`, `step_sliding`) with PS6 of ADR 0629 except `n_outer = K` and
`non_orth_correctors = N`, both solvers the same settings. `V(1, 6)` is ADR 0641's set-up exactly (the acceptance run).

- `V(1, 6)`: the acceptance run (c1, c2).
- `V(K, 6)`, `K = 6`: the momentum lag of the coupled faces removed (read 3), the orthogonal coupling (Q4) present.
- `V(1, 0)`: Q4 removed: with no non-orthogonal corrector the annulus's internal faces and the sector's coupled faces have
  the same coefficients (read 1, 2); the lag present.
- `V(6, 0)`: both removed; what remains is every other difference of the periodic treatment (the least-squares gradients at
  the seam cells, which take the face value where the annulus takes the neighbour cell; the old-level Rhie-Chow defect, which
  `AleStep` keeps on internal faces only; the sliding remap, shown equal to the annulus's to `2e-14` by ADR 0641 Q).

`K = 6` from the contraction of the lag (estimate below: each outer iteration multiplies the lag's residual by about
`a_s / a_P <= 0.16`, so after 6 iterations `0.16^5 ~ 1e-4` of it is left); checked by LC.

**Decomposition (exact).** With `delta_V(n)` the field `u_C - u_A` of variant `V` at step `n` (every sector cell against its
nearest annulus cell), the identity

`delta_(1,6) = [delta_(1,6) - delta_(6,6)] + [delta_(6,6) - delta_(6,0)] + delta_(6,0)`

splits the acceptance run's difference into the lag part `L`, the Q4 part `Q` (with the lag removed) and the rest `R`; the
other order `[delta_(1,6) - delta_(1,0)] + [delta_(1,0) - delta_(6,0)] + delta_(6,0)` gives `Q'` (with the lag present) and
`L'`. With `M(x) = max over the 400 steps and the cells of |x| / U_in` and `G = M(delta_(1,6))`, the shares are `q = max(M(Q),
M(Q')) / G`, `l = max(M(L), M(L')) / G`, `rho = M(R) / G`. By the triangle inequality `G <= M(L) + M(Q) + M(R)`.

**Identification rule for c3 (registered).** Q4 is **identified** as the source of the remaining difference when `q >= 0.5`
and `q >= max(l, rho)`; **ruled out** when `q <= 0.1`; otherwise c3 is **undecided** (the shares are reported). The same rule
names the lag (with `l`) or the rest (with `rho`) as the source where it applies.

**J (the start transient of ADR 0641, diagnosis).** ADR 0641's start `(r_i/r) x_hat` with the same variants plus `V(10, 6)`,
16 steps (the two first coincidence steps), the shares computed as above over the 16 steps. Not part of c1; it names the part
of the solver that makes ADR 0641's step-1 difference.

**The seam pairs' geometry (H1).** For every seam pair `(f, g)` of C's new mesh (the periodic pairs that are not wrap pairs),
`d = (x_f - c_f) + R (c_g - x_g)` (`R` the patch rotation that maps the partner's frame to `f`'s, `x` face centres, `c` owner
centres): the vector from the owner to the partner's image. The non-orthogonality `|d x n_f| / |d|`. On the polar sector meshes
the two cells of a seam pair are mirror images in the seam plane (uniform angular spacing), so `d` is normal to the face and
the missing correction `k . grad p` is zero on the seam pairs exactly, whatever the pressure. Q4 can then act only through the
wrap pairs; their non-orthogonality is printed.

## Estimates (before any run)

Mesh numbers: rotor cells `h_r = 0.5/8 = 0.0625`, `h_theta = r pi/72` (`0.0436` at `r = 1`); stator `h_theta = r pi/60`
(`0.0785` at `r = 1.5`); `nu = 0.1`, `dt = pi/576 = 5.454e-3`, `Omega = 1`.

- **The lag (L).** In the implicit-Euler cell equation `a_P u_P = sum a_nb u_nb + V u_P^old / dt + ...` the coupled
  neighbour enters with its old value. The tangential diffusion coefficient over the transient one is `nu dt / h_theta^2 =
  0.287` at the rotor's inner ring (radial `nu dt / h_r^2 = 0.140`), so `a_P / (V/dt) = 1 + 2 (0.287) + 2 (0.140) = 1.85` and
  `a_s / a_P = 0.155`. The rotor's cells move through the inlet pattern fixed in the absolute frame, so a seam cell's partner
  changes per step by up to `Omega dt |d u / d theta| = dt . 3 U_in = 1.64e-2 U_in` (at `r = 1`, `d/d theta (1 + 0.5 sin 6
  theta) = 3 cos 6 theta`). The lag puts a source `0.287 x 1.64e-2 = 4.7e-3 U_in` per step on the seam cell (relative to `V /
  dt`); the cell keeps about `1 / 1.85 = 0.54` of its perturbation per step, so the perturbation saturates at `4.7e-3 / (1 -
  0.54) = 1.0e-2 U_in`. The pattern's sign at the rotor seam changes every `pi/6` of rotation (96 steps), much longer than
  the cell's memory (about 2 steps), so no cancellation is assumed. Convective lag: the flux through a radial seam face is
  the tangential velocity, zero in `u0` and small after it; neglected. Estimate of `L`: `<= 1.0e-2 U_in`, at the rotor's inner
  rings next to the rotor seam.
- **Q4 and the rest (Q + R).** ADR 0634's uniform inlet (axisymmetric, steady, so no lag) measured `8.03e-3 U_in`, which is
  then `Q + R` for that flow; with the modulation the velocity at the interface reaches `1.5` times the uniform one, so `1.2e-2
  U_in`.
- **The start transient with `u0`.** On step 1 the field changes by the unbalanced viscous term of the modulation, `nu 36 (0.5
  U_in) / r^2 dt = 9.8e-3 U_in` at `r = 1`, plus the pattern's passage `1.64e-2`: a lag source on step 1 of `0.287 x 2.6e-2 /
  1.85 = 4.0e-3`, below the saturated lag of every later step. ADR 0641's start, by contrast, jumps by `0.5 U_in sin 6 theta`
  at the inlet ring on step 1 (the inlet against `(r_i/r) x_hat`): the same arithmetic gives up to `0.155 x 0.5 / 2 ~ 4e-2`
  (the jump spread over the inlet ring and its neighbour), the size of ADR 0641's `3.49e-2`. So the lag is the expected source
  of ADR 0641's step-1 difference (J tests this).
- **G.** `1.0e-2 + 1.2e-2 = 2.2e-2 U_in`; the limit **`3e-2`** (ADR 0641's limit, not raised; margin 1.36). The rms (G2): ADR
  0641's limit `1.5e-2` (measured `8.63e-3` from the inconsistent start).
- **Expected shares** (not limits): `l` about `0.45`, `q + rho` about `0.55` with their split unknown; c3 may come out
  undecided.

## Checks (limits fixed now)

| Id | Clause | Set-up | Check | Limit |
| --- | --- | --- | --- | --- |
| G1 | c1, c2 | `V(1, 6)` from `u0` | on every one of the 400 steps `max_c abs(u_c^C - u_c^A) / U_in`; printed separately over the coincidence steps and step 1 | `<= 3e-2` each step |
| G2 | c1 | `V(1, 6)` | step 400, the volume-weighted rms of the same difference | `<= 1.5e-2` |
| G0 | set-up | every variant, every step | every C cell's nearest A cell within `1e-9`, one-to-one | as stated |
| A1s | supporting | `V(1, 6)` | C's `flux_residual` from step 2 (ADR 0641 A1) | `<= 1e-12` |
| H1 | c3 (seams) | `V(1, 6)`, C's new mesh every step | the seam pairs' non-orthogonality `|d x n| / |d|`; wrap pairs printed | `<= 1e-12` |
| I1 | c3 | `V(1,6)`, `V(6,6)`, `V(1,0)`, `V(6,0)`, 400 steps from `u0` | the shares `q`, `l`, `rho`; the identification rule | Q4 identified or ruled out (else c3 undecided) |
| LC | set-up of I1 and J | J's `V(10, 6)` and `V(6, 6)` | `M(delta_(10,6) - delta_(6,6))` over the 16 steps | `<= 0.05 M(delta_(1,6) - delta_(6,6))` |
| J0 | set-up of J | J's `V(1, 6)` (ADR 0641's start) | step 1's largest difference reproduces ADR 0641's `3.4941e-2` (F-724 kept that run bit for bit) | within `1e-6` |
| I2 | diagnosis (ADR 0641 F1) | J, 16 steps | the shares over the 16 steps; the identification rule names the source of ADR 0641's step-1 difference | one part with share `>= 0.5` and the largest |
| NC1 | control of c2 | J's `V(1, 6)` | the inconsistent start's largest difference over the 16 steps exceeds G1's limit | `>= 3e-2` |
| NC2 | control of G1's measure | `V(1, 6)`, step 400 | the same measure with every C cell matched to the A cell nearest to its centre turned by one rotor cell (`pi/72`) | `>= 5e-2` |
| B1 | defaults | `cargo test -p rcfd-api --release --test baseline_hashes`; `git diff 9f06e866 -- crates/` | pass; only the new test file | as stated |

Printed, no limit: the location (radius, angle from the rotor seam and from the stator seam) of every variant's largest
difference; each variant's `G`; `M(L), M(L'), M(Q), M(Q'), M(R)`; the wrap pairs' largest non-orthogonality; elapsed times.
NC2's estimate: `|d u / d theta| (pi/72) = 3 x 0.0436 = 0.13 U_in` at the inner ring.

Cost: ADR 0641's LS (three solvers, Q-steps) took 21.7 s. `V(1, 6)` (two solvers) about 15 s, `V(1, 0)` cheaper (one
pressure pass), the `K = 6` variants six times their `K = 1` cost. The fast test is `V(1, 6)` (G1, G2, G0, A1s, H1, NC2) and J
(16 steps, five variants: J0, LC, I2, NC1). I1 runs the four variants in one loop (eight solvers); if its one run takes more
than 120 s it is marked `#[ignore]` afterwards as a heavy record (no limit changes), J covering the same code path fast.

Mutations (on a scratch copy of the library and of the test, after the checks; each applied alone; the named tests run once;
each must fail):

- **MU1** `bc.rs` `Periodic`: `span = 2 (dn + dn_q)` (the coupled faces' coefficient halved, in momentum and pressure): G1
  must fail.
- **MU2** `bc.rs` `Periodic`: the partner value replaced by the owner's (`q = u_P`): G1 must fail.
- **MU3** `pressure.rs` `boundary_hby_a`: the coupled face's weight `w = 1` (the owner's `HbyA` only): G1 or A1s must fail.
- **MU4** the test: C started from ADR 0641's `(r_i/r) x_hat` while A starts from `u0`: G1 must fail.

Decision rule: **done** when G1, G2, G0, H1, LC, J0, NC1, NC2, B1 are met, c3 is decided by I1 (Q4 identified or ruled out),
and every mutation is caught; A1s and I2 are supporting and reported. **Partial** when G1 is met but c3 is undecided, or a
set-up check, a control or a mutation is missed; also when G1 is missed with its cause named by I1. **Todo** when G1 is
missed and I1 does not name its cause.

## Limits (not covered)

- No change of the solver's coupled faces: the lag and Q4 are measured, not removed (a solver change for coupled faces
  changes every periodic case's default results, ADR 0629 Q4 (b); open).
- The first branch of c2 (a limit including ADR 0641's start transient) is not taken (Q1).
- `R` is not split further (gradients at seam cells, the old-level defect on coupled faces).
- One mesh pair (SS against P), one time step, one inlet modulation.

## Open questions taken on the standing instruction

- **Q1. Which branch of c2.** (a) a consistent start (taken: a limit for ADR 0641's start would be written knowing its
  result); (b) a limit for ADR 0641's start with its transient. Taken (a) on the maintainer's standing instruction of
  2026-11-11 (take the recommendation).
- **Q2. How to identify Q4.** (a) by removing it in both runs (`non_orth_correctors = 0`, where the coefficients of coupled
  and internal faces are equal) and the lag (outer iterations), an exact decomposition, no library change (taken); (b) an
  opt-in non-orthogonal correction on coupled faces in the solver (a shared solver file, a larger change). Taken (a) on the
  standing instruction.
