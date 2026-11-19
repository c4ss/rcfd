# ADR 0516: F-589 registration: the first and fourth spectral moments of a random stress component for broadband fatigue (covariances C_1 and C_4 beside C_0 and C_2)

Status: **REGISTERED 2026-11-14, written before any code of F-589 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-589 was written, compiled or run in the repository. The files under "What was read"
were read; no arithmetic was done beforehand except the reading of the closed forms of ADR 0477 (listed below). The bit record
of the default paths (D6) is a test file written and run on the base d41012db after this commit and before any library line.
Results go in the addendum at the end; nothing above the addendum is changed after a run.

Roadmap item F-589 (deps F-417, F-566), structural solver, advanced tier, follow-up of F-566 ([ADR 0497](0497-f566-registration-broadband-fatigue-dirlik-from-spectral-moments.md), open question Q3)
and of F-567 ([ADR 0510](0510-f567-registration-complex-cross-spectra-and-frequency-dependent-coherence-of-random-loads.md)). Number 0516 given by the lead. Branch `f589-work` from d41012db, worktree `w49i-work/f589`,
scratch `w49i-work/f589-scratch`. House rules (`w49i-work/AGENT_RULES.md`): limits fixed here, a miss becomes an ignored
`*_known_miss` test with the limit intact and the number recorded, each registered run made once, defaults bit-identical, no edit
of `baseline_hashes.rs`, the hash tables or any golden file, no new dependency, no unsafe code, clean-room. The brief overrides
rule 9 for this item: the F-589 entries of `docs/ROADMAP.md` and `docs/roadmap.json` are updated in a separate last commit (the
svgs are not regenerated); the proposed text is also in the addendum.

## What F-589 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. RandomStress carries the covariances C_1 and C_4 (weighted by f and f^4) beside C_0 and C_2, the existing fields bit-identical; for a uniaxial stress the moments m0, m1, m2, m4 of a component equal those of the output channel scaled by the square of the stress per unit displacement within 1e-10; the broadband rate of F-566 fed from a stress component equals the scaled channel rate within 1e-10; a von Mises equivalent is refused for the broadband rate with the reason (it is not Gaussian).

Clauses: a1 `RandomStress` carries `C_1` and `C_4` beside `C_0` and `C_2`; a1b the existing fields bit-identical; a2 for a uniaxial
stress `m_0, m_1, m_2, m_4` of a component equal those of the output channel times the square of the stress per unit
displacement, within 1e-10; a3 the broadband rate of F-566 fed from a stress component equals the scaled channel rate within
1e-10; a4 a von Mises equivalent refused for the broadband rate with the reason (not Gaussian). The brief adds: the covariances
must work on the cross-spectral path of F-567 or be refused with a reason (registered: they work, D3), and no case-file key (open
questions).

Readings fixed here. "The stress per unit displacement" is `k_p = s_p / phi_e`, where `s_p` is the component of the stress at point `p` of the
single mode `phi` (the stress of the displacement `phi` minus the stress of zero displacement, `IntegrationPointField`) and
`phi_e` its value at the output equation `e`. For a one-mode basis without static correction the displacement of the output is
`phi_e q(t)` and the stress component `s_p q(t)` (`q` the modal coordinate), so `sigma = k_p u` exactly, a linear relation
between two outputs of the same system: the moments scale by `k_p^2`, and the rate (a power law, `rate ~ amplitude^m`) by
`|k_p|^m`. "Uniaxial" is the bar of ADR 0477 with `nu = 0` (only `sigma_xx` is non-zero); the general case is checked
additionally on `nu = 0.3` (all six components, a rank-one covariance `C_k = m_k/phi_e^2 s s^T`).

## What was read (worktree d41012db, read only)

ADR 0477 (F-417: `ModalChannels`, `spectral_response`, `RandomSolver`, the one-mode integrals `I_k(10)`, the bar B, `RandomStress`
with `C_0` and `C_2`), ADR 0497 (F-566, which noted that `RandomStress` carries `C_0` and `C_2` only; `Moments::broadband_damage_rate`,
`broadband_damage_rate` refusals, key `"broadband"`), ADR 0510 (F-567: `spectral_response_cross`, `CrossSpectrum`, `Coherence`,
`real_route`), `crates/rcfd-fem/src/random.rs` and `random/cross.rs` (the loops that accumulate `moments`, `cov`, `cov2`),
`crates/rcfd-fem/src/fatigue.rs` (through ADR 0497), the tests `sa_random.rs` (`bar`, `tip`, M1, S7), `sa_broadband.rs`,
`sa_random_default_bits.rs`, `sa_random_cross_default_bits.rs`. Numbers taken from ADR 0477, not recomputed: `I_0(10) =
39.26957079652609`, `I_1(10) = 38.77258066850795`, `I_2(10) = 39.16923599186199`, `I_4(10) = 49.006230409610886`
(`zeta = 0.02`; `m_k = G f_n^{k+1} I_k/k^2` for a one-mode force response on `[0, 10 f_n]`), and the ratio
`broadband/narrow-band = 0.9888264677733556` of ADR 0497 C2 for those moments.

## The method

### Moments of a stress component

For the output density of a (real) linear combination `s = w^T sigma` of the stress components (a component is a unit vector `w`),
the one-sided density is `S_ss(f) = w^T S(f) w`, `S_ij(f) = Re sum_ab conj(Y_ia) Y_jb G_ab`, symmetric in `i, j` for a Hermitian
`G` (the conjugate of `S_ji` is the same sum with `a, b` exchanged, whose real part is the same). So `C_k = int f^k S(f) df` is a
symmetric matrix, positive semi-definite (a positive measure `f^k df` times a PSD matrix), and the spectral moments of `s` are
`m_k(s) = w^T C_k w`. The moments of F-417 `Moments { m0, m1, m2, m4 }` of a component `c` are the diagonal entries
`C_0[c][c]`, `C_1[c][c]`, `C_2[c][c]`, `C_4[c][c]`; `m_1` and `m_4` were available for output channels in
`spectral_response` but not for the grouped stress covariance (ADR 0497, Q3).

### Design (code)

`crates/rcfd-fem/src/random.rs` and `random/cross.rs`:

- D1. `ChannelResponse` gains `first_covariance` and `fourth_covariance` (`Vec<Vec<f64>>`, row-major `group x group`, weighted
  by `f` and `f^4`), beside `covariance` and `rate_covariance`. In both loops (`spectral_response`, `spectral_response_cross`) two
  separate accumulators `cov1`, `cov4` are updated by two added statements next to those of `cov` and `cov2`; the statements and
  the arithmetic order of `cov` and `cov2` (`w * s`, `w * f * f * s`) and of `moments` are not touched. The symmetrising of the
  upper triangle runs over the new matrices in a separate loop; for a group of one output the entries are `m1` and `m4` of the
  moments (the same copy the existing code does for `m0`, `m2`).
- D2. `RandomStress` gains `first_covariance` and `fourth_covariance` (`Vec<[[f64; 6]; 6]>`) filled from the grouped response
  of the solver. New methods (the existing ones unchanged): `moments(p, component) -> Result<Moments>` (the four diagonal entries;
  a point or component out of range refused, key `"random stress"`), `combination_moments(p, &[f64; 6]) -> Result<Moments>`
  (`w^T C_k w`, non-finite weights refused), `broadband_damage_rate(p, component, m, c)` (that component's moments fed unchanged
  to `Moments::broadband_damage_rate`) and `von_mises_broadband_damage_rate(p, m, c)`, which always returns an error, key
  `"broadband"`, reason: the von Mises equivalent `sqrt(s^T A s)` of a Gaussian stress is a non-negative square root of a
  quadratic form, not a Gaussian process, so the spectral moments of its square do not give the moments of its amplitude and the
  Dirlik density of F-566 (fitted to Gaussian processes) does not apply; use a component or a combination (`combination_moments`
  with a plane normal, or a direction fixed by the caller).
- D3. Cross-spectral path (the brief's question). Registered: the covariances work there. `spectral_response_cross` computes
  `pair(i, j) = Re sum_ab conj(Y_ia) Y_jb G_ab` (the real part of the Hermitian form; ADR 0510); `C_1`, `C_4` are the same
  quantity weighted by `f` and `f^4` and the same upper-triangle copy applies (the derivation above: the real part is symmetric
  in `i, j`). The real-coherence route to `spectral_response` hands them over with the rest. Tested on a complex coherence (pure
  delay) through the solver (X2) and by comparing the two routes on a constant real coherence (X1).
- D4. Not changed: any existing field and method; the frequency grid; `Moments`; the F-566 formulas. Not added: a case-file key,
  a CLI or an API schema (open question Q1).
- D5. The stress per unit displacement is not stored: it is derived in the tests from `IntegrationPointField`, which is the
  reference of ADR 0477 M1.
- D6. Bit record of the base (before any library line): `tests/sa_random_stress_default_bits.rs`, digests recorded at d41012db of
  `RandomStress` (`covariance`, `rate_covariance`, `positions`), `RandomResponse::moments`, `ChannelResponse` (`covariance`,
  `rate_covariance`, `moments`, `psd`) for: the one-mode bar (nu = 0.3) force and base stress, the cantilever of ADR 0510 (8
  modes) with uncorrelated and correlated force patterns and a base acceleration, `spectral_response` groups 1 and 3 with and
  without correlation, and `spectral_response_cross` with `real_route(false)` on a constant real coherence and on a complex pure
  delay (groups 1 and 3), plus `broadband_damage_rate` and `narrow_band_damage_rate` on fixed moments. The same test, unchanged,
  must pass after the library change.

## Arithmetic before this registration (disclosed)

None. The estimates below are reasoned from the arithmetic of ADR 0477 and 0497 (the same integrals, the same modal scaling) and
from the fact that both sides of A1 to A3 are computed by the same loop code with a different but equivalent linear gain
(`s_p` against `phi_e`), so they differ by rounding (`~1e-15` per operation, summed over about 140 nodes and 40 modes to
`~1e-13` at worst).

## Checks (limits fixed now)

Common: bar B of ADR 0477 (`hex_box([0,0,0], [1, 0.1, 0.1], [40, 1, 1])`, `E = 2e11`, density 7850, `y` and `z` fixed on all nodes, `x`
fixed at `xmin`; `nu = 0` or `0.3`), one mode (`modal_analysis(&m, ModalOptions::new(1).tolerance(1e-12).residual_test(Backward).threads(1))`
and `ModalBasis::from_result`), `HarmonicOptions::new(Ratio(0.02)).static_correction(false).threads(1)`, tip pattern 0.25 N on
the four `xmax` nodes in `x`, output equation `e` the first of them (displacement), spectrum `Spectrum::flat(0, 10 f_1, 1.0)`, the
registered grid, stress requested. `k_p` from `IntegrationPointField` as above. "Err" of two numbers `a, b` is `0` if `a == b`,
otherwise `|a - b|/max(|a|, |b|)`; for a matrix, `max_ij |A_ij - B_ij| / max_ij |B_ij|`. Every aggregate is checked finite before it
is compared (a NaN fails; the aggregate is not an `f64::max` over values that may be NaN). The worst value over all integration
points (every point of the 40 elements, 8 Gauss points each) is the quantity compared.

| Id | Clause | Quantity | Limit | Estimate |
|---|---|---|---|---|
| B1 | a1b | the digests of D6 on the library after the change equal the digests recorded at the base | equal bits | by construction |
| B2 | a1 | group 1: `first_covariance[k][0]` and `fourth_covariance[k][0]` bit-equal `moments[k].m1`, `.m4`, on the real route and on the general complex loop (`real_route(false)`) | equal bits | exact |
| A1 | a2 | nu = 0, force and base: worst Err of `C_k[p][0][0]` against `k_p^2 m_k(channel)` for `k = 0, 1, 2, 4` over all points | `<= 1e-10` | `1e-13` |
| A1b | a2 | nu = 0.3, force: Err of the 6x6 matrix `C_k` against `s s^T m_k/phi_e^2`, `k = 0, 1, 2, 4`, worst over points | `<= 1e-10` | `1e-13` |
| A2 | a3 | nu = 0, force, `m = 3` and `m = 5`, `c = 1`: worst Err of `RandomStress::broadband_damage_rate(p, 0, m, c)` against `|k_p|^m` times `Moments::broadband_damage_rate` of the channel | `<= 1e-10` | `1e-13` |
| A3 | a1 | nu = 0, point 0: `C_1/C_0` against `f_1 I_1/I_0` and `C_4/C_0` against `f_1^4 I_4/I_0` (constants of ADR 0477 above), relative | `<= 1e-6` each | `2e-7` |
| A4 | a1 | cantilever of ADR 0510 (8 modes, static correction on, two correlated force patterns), all points, `k = 1, 4`: `C_ij^2/(C_ii C_jj)` for every pair of the six components with `C_ii C_jj > 0` is at most `1 + 1e-9`; diagonals `>= 0`; and for every component with `m_0 > 0`, `m_1^2/(m_0 m_2)` and `m_2^2/(m_0 m_4)` at most `1 + 1e-12` | as stated | `<= 1` |
| X1 | D3 | `spectral_response_cross` with `real_route(false)` against `real_route(true)` on a constant real coherence of 3 patterns, 6 outputs, groups 3 and 6: Err of `first_covariance` and `fourth_covariance` | `<= 1e-12` | `0` |
| X2 | D3 | nu = 0.3, `RandomSolver` with `Excitation::cross` of two tip patterns (0.25 N on the four tip nodes; 0.25 N on the first tip node) with equal flat spectra and the pure-delay coherence `gamma_12 = exp(-i 2 pi f tau)`, `tau = 1/(4 f_1)`: Err of `C_k` (`k = 0, 1, 2, 4`) against `s s^T m_k/phi_e^2` with `m_k` of the displacement channel of the same response; worst over points | `<= 1e-10` | `1e-13` |
| X2b | D3 | the complex part is resolved: `max_p |a_p/b_p - 1|` with `a_p` the xx entry of `C_1` at point `p` for `tau = 1/(4 f_1)` and `b_p` the same for `tau = 0` | `> 1e-3` | `0.1 .. 1` |
| V1 | a4 | `von_mises_broadband_damage_rate(p, 3, 1)` is `Err(InvalidValue { key: "broadband" })` whose text contains `von Mises` and `Gaussian`, at every point of the nu = 0 bar (where `rms_von_mises` equals `rms_xx` within 1e-10: a refusal not tied to the values) and of nu = 0.3 | all | by construction |
| V1b | a4 | positive control: the xx component of the same points is accepted, finite and positive | all | by construction |
| V2 | refusals | `moments` and `broadband_damage_rate` with a point `>= n` or a component `>= 6`; `combination_moments` with a NaN or infinite weight: refused, key `"random stress"`; an all-zero combination (zero `m_0`) refused by `Moments::broadband_damage_rate` with key `"broadband"` | all | by construction |
| L1 | combination | nu = 0.3, plane normal `n = (2, -1, 2)/3`, weights `[n_x^2, n_y^2, n_z^2, 2 n_x n_y, 2 n_y n_z, 2 n_z n_x]` (tensor shear Voigt of F-417): `combination_moments` against `(w . s)^2 m_k/phi_e^2`, Err, worst over points | `<= 1e-10` | `1e-13` |
| N1 | neg. | A1 with the `k_p` of the next point in place of `k_p`: Err at some point | `> 1e-3` | `1e-2 .. 1` |
| N2 | neg. | nu = 0, point 0: `|broadband/narrow-band - 1|` of the stress moments (so `m_1`, `m_4` matter) | `>= 5e-3` | `1.1e-2` |

The "scaled channel" of A1, A1b, A2, X2 is a different computation from the stress covariance: the channel moments come from
`spectral_response` with `group = 1` and the displacement gains `phi_e q_r`; the stress covariance from the grouped run
(`group = 6`) with the gains `s_p q_r`. A3 compares with a closed form (the integrals of ADR 0477).

## Negative controls and planned mutations (on a scratch copy, one at a time, the whole new test file run on each)

N1 and N2 are inside the tests; V1b is the positive control of V1.

| Id | Mutation | Expected to be caught by |
|---|---|---|
| M1 | real path: `cov1` weighted `w * f * f * s` (as `C_2`) | A1, A1b, A3, X1 |
| M2 | real path: `cov4` weighted `w * f * f * f * s` | A1, A1b, A3 |
| M3 | general complex path: `cov1` weighted by `f * f` | X1, X2 |
| M4 | general complex path: `cov4` not accumulated (stays 0) | X1, X2, A4 |
| M5 | the upper-triangle copy skipped for the new matrices | A1b, A4, X2 |
| M6 | `RandomStress::moments` returns `rate_covariance` as `m1` | A1, A2 |
| M7 | `von_mises_broadband_damage_rate` computes the rate of the von Mises moments | V1 |
| M8 | `combination_moments` adds `w_i C_ii` instead of `w^T C w` | L1 |
| M9 | existing path: `w * (f * f) * s` for `cov2` (changes bits of the existing field) | B1 |
| M10 | `Moments` of a component from the wrong index (`C_4[c][c]` as `m_2`) | A1, A2 |

## Decision rule

- **done:** every check B1, B2, A1 to A4, X1, X2, X2b, V1, V1b, V2, L1, N1, N2 met, every mutation caught, `cargo test -p
  rcfd-fem --release`, the `baseline_hashes` test and `docs_site` passing, fmt and clippy clean.
- **partial:** a clause of the acceptance missed or not run, the miss recorded as an ignored `*_known_miss` test with its numbers,
  or a supplementary check missed.
- **todo:** the covariances not carried.

## Limits (not covered)

- Gaussian, stationary, zero-mean stress; a linear combination of stress components is Gaussian, any non-linear equivalent
  (von Mises, principal stress, maximum shear) is not and is refused for the broadband rate (a rate for those would need a
  non-Gaussian method, not built).
- The moments are those of the real cross-density; a fixed combination direction (a plane normal) is the caller's choice, no
  critical-plane search.
- No mean-stress correction, no multiaxial fatigue criterion.
- The covariances `C_1`, `C_4` carry the quadrature error of the registered grid (ADR 0477 S3); A1 to A2 compare two
  computations on the same grid and so do not measure it, A3 does (1e-6).
- No case-file key, no CLI, no API schema; `ChannelResponse` and `RandomStress` are `#[non_exhaustive]`, so the added fields are
  source compatible.

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

- Q1. Case-file key: none added (the brief forbids it). Recommended for the lead: `[fatigue]` with `method = "narrow-band" |
  "dirlik"`, `stress = "xx" | "yy" | ... | "normal:nx,ny,nz"` (a component or a plane normal) next to the F-417 random outputs, the
  outputs `damage_rate` and `life` per integration point, and `stress = "von-mises"` refused with the reason of V1.
- Q2. Cross-spectral path: the covariances work there (D3), no refusal. Taken as recommended.
- Q3. Rate of a non-Gaussian equivalent (von Mises, principal): refused here. Option: a later method that corrects for the
  non-Gaussian equivalent. Recommended: leave refused until a method is chosen and cited.

## Addendum 1: results

Registered runs made once on the commits of this branch: `cargo test -p rcfd-fem --release --test sa_random_stress_moments --
--nocapture --test-threads=1` (6 tests, 0 failed, nothing re-run to pick a result) and `--test sa_random_stress_default_bits`. The bit
record BASE (17 digests) was printed at d41012db before any library line and is the unchanged constant of the test. Every number below
is printed by the tests.

| Id | Quantity | Limit | Measured | Met |
|---|---|---|---|---|
| B1 | 17 digests of the existing fields (`RandomStress` `covariance`, `rate_covariance`, `positions`; `ChannelResponse` fields; the rates) | equal bits | equal (`b1_default_bits_of_the_existing_stress_and_channel_fields`); the three older default-bit tests (ADR 0477, 0497, 0510) also pass | yes |
| B2 | group 1: `first_covariance`, `fourth_covariance` bit-equal `m1`, `m4`: real route, general loop, `spectral_response` | equal bits | equal | yes |
| A1 | nu = 0, force, worst Err over 320 points of `C_k[0][0]` against `k_p^2 m_k` for `m0, m1, m2, m4` | `<= 1e-10` | `1.5e-15, 1.6e-15, 1.2e-15, 1.5e-15` | yes |
| A1 | the same, base acceleration (relative displacement output) | `<= 1e-10` | `2.2e-15, 1.4e-15, 1.8e-15, 1.4e-15` | yes |
| A1 | through `RandomStress::moments`, force; base | `<= 1e-10` | `1.6e-15`; `2.2e-15` | yes |
| A1b | nu = 0.3, force, 6x6 matrix Err of `C_0, C_1, C_2, C_4` against the rank-one form | `<= 1e-10` | `1.3e-15, 2.2e-15, 1.5e-15, 1.9e-15` | yes |
| A2 | nu = 0, `m = 3` and `m = 5`: worst Err of the stress rate against `|k_p|^m` times the channel rate | `<= 1e-10` | `2.1e-15`; `3.0e-15` | yes |
| A3 | `C_1/C_0` against `f_1 I_1/I_0`; `C_4/C_0` against `f_1^4 I_4/I_0` (`f_1 = 1261.967247` Hz) | `<= 1e-6` each | `3.0e-10`; `2.6e-8` | yes |
| A4 | cantilever, 8 modes, correlated forces, 512 points, 7680 pairs: max `C_ij^2/(C_ii C_jj)` for `C_1`; `C_4`; smallest diagonal; max `m_1^2/(m_0 m_2)`; max `m_2^2/(m_0 m_4)` | `<= 1 + 1e-9`; `<= 1 + 1e-9`; `>= 0`; `<= 1 + 1e-12`; `<= 1 + 1e-12` | `0.99969`; `0.99988`; `9.8e8` and `1.9e18` (positive); `0.9501`; `0.8038` | yes |
| X1 | `real_route(false)` against `real_route(true)`, groups 3 and 6: `first_covariance`, `fourth_covariance`; real route against `spectral_response` | `<= 1e-12` | `0` (bit-equal), all six | yes |
| X2 | nu = 0.3, `Excitation::cross`, pure delay `tau = 1/(4 f_1)`: matrix Err of `C_0, C_1, C_2, C_4` | `<= 1e-10` | `1.8e-15, 1.8e-15, 1.8e-15, 1.4e-15` | yes |
| X2b | `max_p |C_1(tau)/C_1(0) - 1|` | `> 1e-3` | `0.321` | yes |
| V1 | `von_mises_broadband_damage_rate` refused with key `"broadband"`, text contains `von Mises` and `Gaussian`, at all 320 points of nu = 0 (where `rms_von_mises` equals `rms_xx`, worst `|ratio - 1| = 0`) and of nu = 0.3 | all | all | yes |
| V1b | the xx component accepted, finite and positive at all 320 points (nu = 0) | all | all | yes |
| V2 | point `>= n`, component `>= 6`, NaN, `+inf`, `-inf` weights refused with key `"random stress"`; the all-zero combination (zero moments) refused by the broadband rate with key `"broadband"` (`m0`) | all | all | yes |
| L1 | nu = 0.3, plane normal `(2,-1,2)/3`: `combination_moments` against `(w . s)^2 m_k/phi_e^2`, worst Err over 320 points and four moments | `<= 1e-10` | `1.9e-15` | yes |
| N1 | A1 with the `k_p` of the next point | `> 1e-3` | `0.889` | as expected |
| N2 | `|broadband/narrow-band - 1|` of the stress moments at point 0 | `>= 5e-3` | `1.117e-2` | yes |

Every aggregate is passed through `worst()`, which returns NaN (and so fails) if any value is not finite; no `f64::max` over a NaN.

Deviations: (1) the cantilever of A4 uses the pattern of ADR 0510 for `p1` (y force at the far top corner) and a different second
pattern (`z` 1.0 and `y` 0.5 at the node `(1, 0, 0)`) so that the stress is multiaxial; the registration said only "two correlated
force patterns". The set-up was fixed before the first run and not changed. (2) The estimates were conservative: A1 to A2
came out at about `2e-15` against the estimated `1e-13`, and A3 `C_4` at `2.6e-8` against `2e-7`. (3) The cross-spectral loop of
A1 and X2 is the same code as the real route's for the real loop, so X1 returns exactly zero (the planned estimate).

Mutations (scratch copy `f589-scratch/mut`, own target dir `target-f589-mut`, one at a time, the two new test files run on each
by `f589-scratch/mutate.py`, log `f589-scratch/mut.log`):

| Id | Mutation | Caught by |
|---|---|---|
| M1 | real path `cov1` weighted `f^2` | A1 (all four rows of `C_1`, error 0.999), A1b, L1, A4 (`m_1^2/(m_0 m_2) = 1.5e6`), X1; the A2 rate then refuses the moments inside the test (a panic, so A3 and N1 did not print) |
| M2 | real path `cov4` weighted `f^3` | A1, A1b, L1, A4, X1 |
| M3 | complex loop `cov1` weighted `f^2` | X1 (`2.5e2`), X2 (`1.5e3`) |
| M4 | complex loop `cov4` zeroed | X1 (1), X2 (1); A4 is not reached (it runs the real path, as noted in the registration table where it was named in error) |
| M5 | no upper-triangle copy for the new matrices | A1b (`0.43`), L1, X2 (`0.43`). Not A4: it reads the lower triangle `C_ij`, which the mutation leaves at zero, so `C_ij^2 <= C_ii C_jj` holds trivially (a blind spot of A4, as registered it was expected to catch M5) |
| M6 | `moments().m1` from `rate_covariance` | A1 (`RandomStress::moments`), A4 |
| M7 | von Mises rate computed from von Mises moments | V1 (both states) |
| M8 | `combination_moments` sums `w_i C_ii` | L1 (0.148) |
| M9 | `cov2` weighted `w * (f * f) * s` | B1 (the bit record) |
| M10 | `moments().m2` from `fourth_covariance` | A1 (`RandomStress::moments`), A4 |

All ten caught. M5 was caught by A1b and X2 and not by A4 (a blind spot of A4, recorded here, not repaired).

Not run: the workspace test suite (the lead runs the gate); a critical-plane search; a multi-pattern grid refinement.

Gates on this branch (all run once, after the last code change): `cargo test -p rcfd-fem --release`: 823 passed, 0 failed, 224
ignored; `cargo fmt -p rcfd-fem --check` clean; `cargo clippy -p rcfd-fem --all-targets -- -D warnings` clean; `cargo test -p
rcfd-api --release --test baseline_hashes`: 3 passed, 1 ignored (unchanged, no edit of any hash table or golden file);
`cargo test -p rcfd-cli --release --test docs_site`: 2 passed. The first `docs_site` run failed because the X2b row of the
registration table contained the text `C_1[p][0][0](tau = 1/(4 f_1))`, which the docs-site renderer reads as a link; the
wording of that table cell was changed (same quantity, same limit `> 1e-3`, written as `a_p/b_p`) and the test passed. No limit
or check was changed.

Decision by the registered rule: **done**. Every check B1, B2, A1 to A4, X1, X2, X2b, V1, V1b, V2, L1, N1, N2 met, every mutation
caught (M5 not by A4, as recorded), the existing tests and the baseline hashes passing, fmt and clippy clean.

Residual: (1) the uniaxial equality of a2 is checked on a one-mode basis, where stress is exactly proportional to the output
displacement; with several modes the ratio is not constant and the clause has no meaning, so the multi-mode stress is checked
only by A4 (positive semi-definiteness, Cauchy-Schwarz) and by the closed forms through the one-mode cases. (2) The von Mises
refusal is unconditional (it does not look at the moments); a non-Gaussian rate for the equivalent stress is open (Q3).

Open questions (options and recommendation):

- Q1. Case-file key for the broadband fatigue output: none added. Options: (a) `[fatigue] method = "narrow-band" | "dirlik"`,
  `stress = "xx" | ... | "normal:nx,ny,nz"`, outputs `damage_rate` and `life` per integration point; (b) leave it to the API.
  Recommended: (a), with `stress = "von-mises"` refused with the reason of V1; it changes a case-file schema golden and needs the
  maintainer's approval of the schema.
- Q2. Cross-spectral path: works (X1, X2), taken as recommended.
- Q3. A rate for a non-Gaussian equivalent stress (von Mises, principal stress). Options: a Gaussian-equivalent correction, a
  Monte-Carlo history through the F-413 counter, or leave refused. Recommended: leave refused until a method is chosen and cited.
- Q4. A critical-plane search over `combination_moments` (maximise the rate over plane normals). Recommended: a follow-up only if the
  fan blade-root stress needs it (the dominant direction is usually the radial normal stress, a single component).

## For the lead: roadmap entries

Proposed status of F-589: **done**. Note:

> ADR 0516, 2026-11-14: rcfd_fem::random RandomStress gains first_covariance (C_1, weighted by f) and fourth_covariance (C_4, weighted by f^4) beside covariance and rate_covariance, and ChannelResponse gains the same two matrices, from separate accumulators in spectral_response and spectral_response_cross (the real constant-coherence route and the complex loop; the existing fields are bit-identical, recorded in sa_random_stress_default_bits.rs at d41012db). New methods: RandomStress::moments(p, component), combination_moments(p, weights) for a linear combination such as a normal stress on a plane, broadband_damage_rate(p, component, m, c) and von_mises_broadband_damage_rate, which always refuses (key "broadband": the von Mises equivalent is not Gaussian). On the one-mode bar the moments m0, m1, m2, m4 of a component equal those of the output channel times the square of the stress per unit displacement within 2.2e-15 (nu = 0, force and base, 320 points; the 6x6 matrices of nu = 0.3 within 2.2e-15), the broadband rate of F-566 fed from a component equals the scaled channel rate within 3.0e-15 (m = 3 and 5), C_1/C_0 and C_4/C_0 equal the closed forms of ADR 0477 within 3.0e-10 and 2.6e-8, and a complex pure-delay cross-spectrum through RandomSolver agrees within 1.8e-15. Limits: Gaussian zero-mean stress, no critical-plane search, no case-file key.

Follow-ups (numbers to be given by the lead):

> Title: case-file key and output for the broadband fatigue life of a stress component. Deps: F-589, F-566. Acceptance: Every limit not stated here is registered in the ADR before the run. A [fatigue] section with method = "narrow-band" or "dirlik" and stress = a component or a plane normal reports the damage rate and the life 1/rate per integration point; stress = "von-mises" is refused with the reason of ADR 0516; a case without the section keeps its bits and the baseline hashes are unchanged; the schema golden is updated with the maintainer's approval.

## Answers of the maintainer (2026-11-18), written into this ADR by the lead on 2026-11-19

The maintainer decided these on 2026-11-18 as sets of options with a recommendation; until now the decision stood in the roadmap note of the item only. Nothing above this section is changed.

- **F-605, Addendum 1, open question Q1.** Asked: a `[fatigue]` section of the case file with the outputs `damage_rate` and `life`. Answer: Approved under the class rule for additive golden edits (agent rules, rule 3, from version 2): an edit that only adds entries and removes or changes none is reported by the agent as an "additive golden edit" with its diff, re-recorded by the lead and written in the ledger line of the merge. `baseline_hashes.rs` is never in this class, and an edit that changes or removes an entry still needs the maintainer's word for that item. The item may be issued.
