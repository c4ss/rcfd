# ADR 0581: F-644 registration: hyperelastic linear paths, near-incompressible modes, order-free damped modes on two materials, multi-material sites under mutation

Status: **REGISTERED 2026-11-15, written before any code of F-644 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-644 was written and nothing of F-644 was compiled or run: the files
under "What was read" were read and the arithmetic below was done by hand (no script, no probe, no rcfd call). The measured
numbers quoted below come from the records of ADR 0555 (its addendum and the ignored tests of `sa_hyper_paths.rs`), not from a new
run. Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-644 (dep F-620), structural solver, advanced tier, the follow-up proposed by
[ADR 0555](0555-f620-registration-hyperelastic-materials-on-the-method-step-and-the-linear-paths.md) (its known misses and remaining
questions 1 and 2). Number 0581 given by the lead. Branch `f644-work` from e348eb34, worktree `w49i-work/f644`, scratch
`w49i-work/f644-scratch`. This is a test-only item: no library line is planned (D1 shows that the options it needs exist).

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact and the measured number); each registered run made once; existing tests not edited; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe
code; clean-room. The roadmap files are not edited (the entries go in "For the lead"). Every choice below that the maintainer would
normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).

## What F-644 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. The six lowest frequencies of the BBar cantilever of ADR 0555 at K/G = 1e4 with the hyperelastic material equal those of its isotropic twin within 1e-8; the damped-mode roots and the direct harmonic response of a model with two hyperelastic materials and two loss factors equal the twin's within 1e-10 with the roots matched without regard to order; the mutations of the two multi-material call sites are caught.

Clauses: **a0** every limit registered here; **a1** the six lowest frequencies of ADR 0555's BBar cantilever with NHi
(`neo_hookean(1e6, 1e10)`, `K/G = 1e4`) against its isotropic twin, within `1e-8`; **a2** the damped-mode roots of a model with two
hyperelastic materials and two loss factors against the twin, within `1e-10`, matched without regard to order; **a3** the direct
harmonic response of the same model against the twin, within `1e-10`; **a4** the mutations of the two multi-material call sites
(`damped.rs` and `harmonic_direct.rs`, the branch `materials.len() > 1`) caught.

## What was read

`AGENT_RULES.md`; the brief; ADR 0555 in full (D1 to D8, the checks, the mutations, amendments 1 to 3, the addendum, remaining
questions 1 and 2 and the follow-up text that became F-644); ADR 0284 amendment 1 (the residual floor `c eps lambda_max /
lambda_1` of the relative residual on a slender cantilever, met there by a raised tolerance); ADR 0354 (the opt-in normwise
backward error `ResidualTest::Backward`); ADR 0572 and 0573 (the form of the "For the lead" section). Sources at e348eb34:
`crates/rcfd-linalg/src/eigen.rs` (`lowest_eigenpairs`: the subspace iteration on the factor of `A = K + sigma M`, the
Rayleigh-Ritz step, the residual of step 6 `||A x - mu M x|| / ||A x||` (Relative) or `||A x - mu M x|| / ((||A||_inf + |mu|
||M||_inf) ||x||)` (Backward), the cluster extension with `CLUSTER_GAP = 1e-6`, the Sturm check; `SubspaceOptions::default`:
tolerance `1e-10`, Relative, 200 iterations, `q = max(2m, m + 8)`), `crates/rcfd-fem/src/modal.rs` (`ModalOptions`, its builders
`tolerance` and `residual_test`, the doc of `tolerance` naming the floor; `ModalResult::residuals`, `iterations`),
`crates/rcfd-fem/src/damped.rs` (the loss-factor branch 700-725: one material scales the projected stiffness, two or more assemble
each material's stiffness apart, line 716 `ctx.material.linear_elasticity_matrix()`; `by_frequency` and `modes.sort_by` at 766:
roots ordered by the damped frequency, then `|lambda|`; `DampedModes::roots`), `crates/rcfd-fem/src/harmonic_direct.rs` (the
branch 707-724, line 719 `ctx.material.linear_elasticity_matrix()`; the one-material site 555), `crates/rcfd-fem/src/mat_hyper.rs`
(`linearised_elasticity` 52-65), `crates/rcfd-fem/tests/sa_hyper_paths.rs` (the set-ups `cantilever`, `two_materials`, `twin`, the
energies, the checks L3, L3c, L3d, L5m, L8m, L8d and the ignored records).

## Diagnosis of the ADR 0555 L3 miss (by reading and arithmetic, before any run)

The modal solver did not converge on the NHi BBar cantilever with the hyperelastic material nor with its twin: the residuals of
the six wanted pairs stalled at `1.3e-9 .. 2.7e-11` (twin `1.4e-9 .. 3.5e-11`) against the default tolerance `1e-10`, and the same
at 2000 iterations (ADR 0555 L3d). The convergence test of the default is the relative residual `rho = ||K x - mu M x|| / ||K x||`
(`sigma = 0`, the cantilever is clamped). Its rounding floor (ADR 0283 D6, ADR 0284 amendment 1) is `rho_floor ~ c eps
lambda_max / lambda_i`: rounding re-enters every component of the iterate at `~eps ||x||`, and the high components are seen by
`K` at `lambda_max`, while `||K x|| ~ lambda_i ||M x||`.

Arithmetic 1 (orders of magnitude, by hand). Twin of NHi: `E = 9 K G/(3K + G) = 2.9999e6`, `nu = 0.49995`, `rho = 1000`.
`lambda_1`: Euler-Bernoulli first bending of the 2 x 0.5 x 0.5 clamped bar, `omega_1 = 1.875^2 sqrt(E I/(rho A L^4))` with `I =
0.5^4/12 = 5.21e-3`, `A = 0.25`, `L = 2`: `sqrt(62.5)/4 = 1.98`, `omega_1 ~ 6.9 rad/s`, `lambda_1 ~ 48`. `lambda_max`: the
dilatational element mode, `~ c_p^2 (2/h)^2 d` with `c_p^2 = (K + 4G/3)/rho = 1.0e7`, `h = 0.25`, `d = 3` and a consistent-mass
factor up to 3: `~ 2e9 .. 6e9`. `lambda_max/lambda_1 ~ 4e7 .. 1.2e8`, `eps` times that `~ 1e-8 .. 3e-8`: the floor of the lowest
pair is `c = 0.05 .. 0.1` of it, the observed `1.3e-9`. NHc (`K/G = 10`, ADR 0555 L3c met at the default tolerance) has `c_p^2 =
1.13e4`, a ratio `~885` smaller: floor `~1.5e-12`. So the tolerance `1e-10` sits below the floor at `K/G = 1e4` and above it at
`K/G = 10`, whatever the material, and more iterations cannot help. It is not a defect of the linearisation (ADR 0555 L0t
`3.9e-13` and L1 NHi `2.3e-10` on the same model) nor of the eigensolver (a documented floor, with the documented remedy in the
doc of `ModalOptions::tolerance`).

Options already in the library, each opt-in, none changing a default: (i) `ModalOptions::tolerance(1e-8)` with the default
Relative test (ADR 0284 amendment 1's remedy; ADR 0555 remaining question 1 (b)); (ii) `ResidualTest::Backward` at the default
tolerance (ADR 0354), whose floor is `c eps` but whose stopping guarantee on `rho` is weaker by `(||K|| + mu ||M||) ||x|| / ||K x|| ~
lambda_max / lambda_1` (arithmetic 2); (iii) a larger subspace or a shift: they change the convergence rate, not the floor. Taken:
**(i)**, on the maintainer's standing instruction of 2026-11-11 (take the recommendation), because its stopping test bounds the
frequency error directly (arithmetic 2); (ii) is run as a record only.

## Design

### D1. No library change

The three clauses a1 to a3 are set-ups of existing calls with existing options; a4 is a property of the set-up of a2 and a3 (a
model with two material sets reaches the `materials.len() > 1` branches). No line under `crates/*/src` is changed, so every default
keeps its bits by construction and no default-bits record is needed (B1 below is the existing suite that covers the files read).

### D2. Set-ups (registered)

From `sa_hyper_paths.rs`, copied into the new test file `crates/rcfd-fem/tests/sa_hyper_paths_f644.rs` (that file is not edited):
the energies NHc `neo_hookean(1e6, 1e7)`, MRc `mooney_rivlin(3.5e5, 1.5e5, 1e7)`, NHi `neo_hookean(1e6, 1e10)`, density 1000; the
twin `Material::isotropic_elastic(E, nu, 1000, 0)` with `E = 9KG/(3K + G)`, `nu = (3K - 2G)/(2(3K + G))` of
`mat_hyper::small_strain_moduli`; **Cantilever L** (`hex_box([0, 0, 0], [2, 0.5, 0.5], [8, 2, 2])`, the block BBar, `xmin` held, the
nodal force `[0, 0, 1]` on `xmax`); **Two-material L** (`two_materials`: the same mesh, Full, element sets `root` (the 16 elements
with centre `x < 1`) with NHc and `tip` (the other 16) with MRc, the twins of each in the twin model).

- **Modes (F)**: `modal_analysis` of Cantilever L with NHi and with its twin, `ModalOptions::new(6).tolerance(1e-8)` (Relative,
  200 iterations, the default shift rule and subspace). The six frequencies of each, ascending as returned.
- **Damped modes (D)**: `damped_modes` of Two-material L, `DampedOptions::new(4)` with `LossFactor::Constant(0.02)` on `root` and
  `0.05` on `tip` (the loss factors of ADR 0555 amendment 2).
- **Harmonic (H)**: `DirectHarmonic` of Two-material L with the same loss factors, `threads(1)`, the model load
  (`HarmonicLoad::from_model`), at `omega = 50` and `500` rad/s.

### D3. The order-free match

For two lists `a`, `b` of complex roots of equal length `n`, the measure is the bottleneck assignment
`W(a, b) = min over bijections pi of max_k |a_k - b_pi(k)| / |b_pi(k)|`. Computed exactly: the `n^2` candidate values sorted,
and the smallest for which a perfect matching of the bipartite graph `{(k, l): d_kl <= t}` exists (augmenting paths, Kuhn's
algorithm; Kuhn 1955, cited from memory, and elementary). Lists of different lengths are a miss. `W` does not depend on the order
of either list, and it is not fooled by a multiset difference (a bijection is enforced, unlike the nearest-root measure of ADR
0555 L8d). Its own control is Q0.

### D4. Files

New: `crates/rcfd-fem/tests/sa_hyper_paths_f644.rs` (the checks). This ADR. No other file.

## Arithmetic (disclosed, by hand)

2. **Frequency error at the stopping test.** For an `M`-normalised Ritz pair `(mu, x)` with `r = K x - mu M x`, there is an
   eigenvalue `lambda` with `|lambda - mu| <= ||r||^2_{M^-1} / g` (Kato-Temple; the Rayleigh-Ritz bound for a cluster with `g` the
   gap to the rest of the spectrum; Parlett, The Symmetric Eigenvalue Problem, ch. 10 and 11, cited from memory). With `||r||_{M^-1}
   <= ||r||_2 / sqrt(m_min)`, `||K x||_2 ~ mu ||M x||_2 <= mu sqrt(m_max)` and `rho = ||r|| / ||K x|| <= tau`: `|lambda - mu| / mu
   <= tau^2 kappa(M) mu / g`. `kappa(M)` of the assembled consistent Hex8 mass is at most the element's `3^3 = 27` (the 1-D linear
   element mass has eigenvalues `1/2` and `1/6` of its length times density; the assembled Rayleigh quotient lies between the
   elements' bounds). Gaps: the six lowest of the clamped bar are the first bending pair (`~6.9 rad/s`, `y` and `z` degenerate, one
   cluster), torsion (`(pi/2L) sqrt(G J/(rho I_p))`, `J = 0.1406 a^4`, `I_p = a^4/6`: `~23`), the second bending pair (Euler-Bernoulli
   `~43`, lowered by shear at `L/h = 4`) and the axial mode (`(pi/2L) sqrt(E/rho) = 43`); the second bending pair and the axial mode
   may lie within a few percent. With `tau = 1e-8` and `mu/g <= 200` (a gap of `0.5 %`): `<= 1e-16 * 27 * 200 = 5.4e-13` in the
   eigenvalue, half in the frequency. (With the backward test at `1e-10` the same bound is `(1e-10 * 1e8)^2 * 27 * 200 ~ 5e-1`: no
   guarantee, the reason (ii) is a record.)
3. **True difference hyperelastic against twin (F).** The two stiffness matrices differ by the rounding of `D` (ADR 0555 L0t, NHi:
   `3.88e-13` relative to the largest entry `lambda + 2G ~ lambda`), a perturbation of the volumetric part: `x^T dK x / x^T K x <=
   (d lambda / lambda) (x^T K_vol x / x^T K x) <= 3.9e-13` in the eigenvalue, `<= 2e-13` in the frequency. Estimate of F1:
   `<= 2e-13 + 2 * 2.7e-13 ~ 8e-13`; limit `1e-8` (the acceptance).
4. **Convergence at `tau = 1e-8` (F0).** The tolerance changes only the stopping iteration: the iterates are those of ADR 0555's
   run up to it. At iteration 200 the six residuals were `<= 1.3e-9` (hyperelastic) and `<= 1.4e-9` (twin), a factor 7 below
   `1e-8`, so both stop at or before iteration 200 with the same Sturm check. Estimate: converged, residuals `<= 1e-8`.
5. **Negative control F1n.** Twin of `neo_hookean(1e6 (1 + 1e-6), 1e10)`. With `K >> G` and a locking-free BBar, `lambda_i = G
   phi_i(K/G)` with `phi_i` nearly constant in `K/G`: `d f / f ~ (1/2) dG/G = 5e-7`. Limit `>= 1e-7`.
6. **Damped modes and harmonic (D1, H1).** The two models differ by the rounding of `D` (L0t NHc, MRc: `3.29e-16`), amplified by the
   conditioning of a well-posed solve (ADR 0555 arithmetic 2): estimate `<= 1e-13`; ADR 0555 recorded `1.13e-13` (L8d, order-free
   nearest root) and `1.34e-14`, `1.36e-14` (L5m) on these very set-ups. Limits `1e-10` (the acceptance).
7. **Negative controls D1n, H1n.** The twin model with the loss factor of `tip` `0.06` instead of `0.05`. A root `lambda ~ i omega
   sqrt(1 + i eta_eff)` moves by `~ d eta_eff / 2` relative, `d eta_eff = 0.01 s_tip` with `s_tip` the share of the mode's strain
   energy in `tip`: for the first bending mode (curvature largest at the clamp) `s_tip ~ (1/2)^5 ~ 3 %` (curvature `~ (L - x)^2`),
   larger for the higher modes: `>= 1.5e-4`. The response at `omega = 50` (near the second bending and axial modes) and `500` (above
   the six lowest) moves by `~ d eta s_tip |K x| / |(K (1 + i eta) - omega^2 M) x| >= 1e-4`. Limits `>= 1e-6` each.

## Checks (limits fixed now)

Every measure is required finite before it is compared (a non-finite value or a refused call misses). "Relative" for vectors is
`|a - b|_inf / |b|_inf`, for complex vectors with `|z| = hypot(re, im)` per entry.

| Id | Clause | What | Limit | Estimate |
|---|---|---|---|---|
| F0 | a1 (set-up) | `modal_analysis` of Cantilever L BBar, NHi and its twin, `tolerance(1e-8)`: both `Ok`, six frequencies each, every residual `<= 1e-8` | `Ok` | arithmetic 4 |
| F1 | a1 | worst relative difference of the six frequencies, hyperelastic against twin (ascending, position by position) | `<= 1e-8` | arithmetic 2, 3: `~8e-13` |
| F1n | negative | F1's measure against the twin with `G (1 + 1e-6)` | `>= 1e-7` | arithmetic 5: `5e-7` |
| F1r | record | the same models with `ResidualTest::Backward` at the default tolerance: `Ok` or the error, iterations, and F1's measure | record | (ii) |
| Q0 | D3 | the measure `W`: (a) a list of 8 roots against its reverse: `0` exactly; (b) one root scaled by `1 + 1e-6`: `W` within `1e-12` of `1e-6`; (c) root 0 replaced by a copy of root 1: `W >= |r_0 - r_1| / |r_1| / 2` (a bijection is enforced); (d) lists of 8 and 7: a miss | exact as stated | D3 |
| D0 | a2 (set-up) | `damped_modes` of Two-material L, hyperelastic and twin: both `Ok`, equal and non-zero root counts | `Ok` | ADR 0555 L8m: 8 roots each |
| D1 | a2 | `W(roots_hyper, roots_twin)` | `<= 1e-10` | arithmetic 6: `1.1e-13` |
| D1n | negative | D1's measure against the twin model with `tip` at `0.06` | `>= 1e-6` | arithmetic 7 |
| D1r | record | the positional measure of ADR 0555 L8m on the same roots (to show the order effect) | record | ADR 0555: `2.0` |
| H0 | a3 (set-up) | `DirectHarmonic::new` and `response` of Two-material L, hyperelastic and twin: `Ok` | `Ok` | ADR 0555 L5m |
| H1 | a3 | relative difference of the responses at `omega = 50` and `500` | `<= 1e-10` each | arithmetic 6: `1.4e-14` |
| H1n | negative | H1's measure against the twin model with `tip` at `0.06`, at each `omega` | `>= 1e-6` each | arithmetic 7 |
| B1 | defaults | no line under `crates/*/src` changed (`git diff --stat e348eb34 -- crates/*/src` empty); `sa_hyper_paths.rs` (fast tests) and `sa_hyper_paths_default_bits.rs` pass unedited | empty; pass | D1 |
| S1 | build | `cargo fmt -p rcfd-fem --check`; `cargo clippy -p rcfd-fem --release --all-targets -- -D warnings` | clean | |

F, D, H run in one test each (`f_*`, `d_*`, `h_*` names); each takes well under a second in release (135 nodes).

### Audit of existing tests (before this registration)

No library line changes (D1), so no existing test can change its result. The ignored records of ADR 0555
(`l3_modes_against_the_twin_known_miss`, `l5m_l8m_two_materials_known_miss`) and its diagnostics (`l3d_nhi_bbar_diagnostic`,
`l8m_roots_diagnostic`) stay as they are, unedited and ignored where they are ignored: this item continues them in a new file, it
does not retire them. The mutations below change library lines on a scratch copy only.

## Negative controls

F1n, D1n, H1n (above): each measure must see a genuine difference of the size estimated, so that a met F1, D1 or H1 is not a
measure blind to the material. Q0 controls the order-free measure itself.

## Planned mutations (on a scratch copy: a `git archive` of the test commit, its own `CARGO_TARGET_DIR`, one at a time; `sa_hyper_paths_f644.rs` run for each)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | `damped.rs` 716 (the multi-material site) `linear_elasticity_matrix()` back to `elasticity_matrix()` | D0, D1 (the hyperelastic model refused) |
| MU2 | `harmonic_direct.rs` 719 (the multi-material site) `linear_elasticity_matrix()` back to `elasticity_matrix()` | H0, H1 (refused) |
| MU3 | `mat_hyper.rs` 62 `row[m] = g` to `row[m] = 2.0 * g` (the shear entries of the linearised elasticity) | F1, D1, H1 |
| MU4 (control of the measure) | `damped.rs` 766 `modes.sort_by(by_frequency)` reversed (`sort_by(\|p, q\| by_frequency(q, p))`) | **not** D1 by design (order-free); D1r changes. Declared equivalent for D1; not counted among the mutations to catch |

MU1 and MU2 are the two multi-material call sites of clause a4; MU3 checks that the measures see the linearised material at all.
A mutation giving every set the first material's linearised elasticity was considered and not registered: NHc and MRc have the
same `(G, K) = (1e6, 1e7)`, so it is equivalent for every check here. No other mutation is declared equivalent.

## Decision rule

- **done**: F0, F1, D0, D1, H0, H1, Q0 and the negative controls met; MU1, MU2, MU3 caught; B1 and S1 met.
- **partial**: any of F1, D1, H1 missed (recorded as a `*_known_miss` test with its limit and the measured value, diagnosed), or a
  negative control missed, or MU1 or MU2 not caught, or a check met only after a post-run amendment.
- **todo**: the set-ups do not run (F0, D0, H0 all missed).

## Limits (what is not covered)

The modes at `K/G = 1e4` are checked at a tolerance raised to `1e-8`, not at the default; the default still does not converge on
that model (a documented floor, not changed here). Other near-incompressible meshes, ratios above `1e4`, the Full Hex8 at that
ratio (locking), Tet elements, Beam2 and shells (a separate follow-up of ADR 0555), viscoelastic or frequency-dependent loss
factors on two hyperelastic materials, and more than two materials are not covered. The order of `damped_modes`' reported roots
is not given a canonical form (a default change, not taken).
