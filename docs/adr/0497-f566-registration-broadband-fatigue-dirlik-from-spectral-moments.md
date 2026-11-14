# ADR 0497: F-566 registration: broadband fatigue of a random stress from its spectral moments (Dirlik damage rate from m0, m1, m2, m4)

Status: **REGISTERED 2026-11-14, written before any code of F-566 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-566 was written, compiled or run in the repository. The files under "What was read"
were read, and the arithmetic under "Arithmetic before this registration" was done by plain Python with numpy (not the library)
in the scratch folder `w49i-work/f566-scratch` (`dk.py`, `sim.py`, `sim2.py`). Results go in the addendum at the end; nothing
above the addendum is changed after a run.

Roadmap item F-566 (deps F-417, F-413), structural solver, advanced tier, follow-up of F-417 ([ADR 0477](0477-f417-registration-random-vibration-by-modal-superposition.md)).
Number 0497 given by the lead. Branch `f566-work` from 9386de34, worktree `w49i-work/f566`, scratch `w49i-work/f566-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no
unsafe code; clean-room. The brief overrides rule 9 for this item: the F-566 entries of `docs/ROADMAP.md` and `docs/roadmap.json`
are updated on the branch in a separate last commit (the svgs are not regenerated).

## What F-566 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. The broadband damage rate (formula registered from a cited source) reduces to the narrow-band rate of F-413 within 1 percent for a one-mode response at zeta = 0.01; for a two-mode response with well separated modes the counted damage of a synthesised Gaussian history (registered seed, 1e5 cycles, rainflow of F-413) lies within 10 percent of the broadband rate and the narrow-band rate is above it; the moments m0, m1, m2, m4 of RandomSolver feed it unchanged; a non-positive m0 or m2 is refused.

Clauses: a1 formula registered from a cited source; a2 one-mode response at `zeta = 0.01`, broadband rate within 1 percent of
the narrow-band rate; a3 two well separated modes, counted damage of a synthesised Gaussian history (seed, 1e5 cycles, rainflow
of F-413) within 10 percent of the broadband rate; a4 the narrow-band rate above the broadband rate there; a5 the moments of
`RandomSolver` feed it unchanged; a6 a non-positive `m0` or `m2` refused.

Readings fixed here. "Narrow-band rate of F-413" is `fatigue::narrow_band_damage_rate(nu0, rms, m, c)` with `rms = sqrt(m0)`
and `nu0 = sqrt(m2/m0)` (`fatigue::spectral_rate`). "One-mode response" is the displacement of a one-degree-of-freedom system
under a flat one-sided force spectrum on `[0, 10 f_n]` (the geometry of ADR 0477 S1). "The counted damage" is the Palmgren-Miner
damage of the cycles of `rainflow_periodic` (the history is a periodic synthesis, as in F-413 B2), divided by the duration, so a
rate in 1/s. The damage curve is the power law `N = c S^-m` of amplitude `S`; the zero-mean assumption holds (a random response
about a static state is a response difference, ADR 0477).

## What was read (worktree at 9386de34, read only)

ADR 0477 (F-417: `Moments`, `spectral_response`, `ModalChannels`, `RandomSolver`, the one-mode integrals `I_k(10)`), ADR 0449
(F-413: `narrow_band_damage_rate`, `spectral_rate`, `rainflow_periodic`, `miner`, `SnCurve::power`, the B2 synthesis), ADR 0453
(F-534), `crates/rcfd-fem/src/fatigue.rs`, `crates/rcfd-fem/src/random.rs` (the `Moments` struct has `m0, m1, m2, m4`),
`crates/rcfd-core/src/rng.rs` (xoshiro256**, seeded through SplitMix64, `uniform()` in `[0, 1)`), the tests `sa_fatigue.rs` (B2) and
`sa_random.rs` (the one-mode and bar systems), and the F-566 entry of `docs/roadmap.json`.

## The method

### Source

T. Dirlik, "Application of computers in fatigue analysis", PhD thesis, University of Warwick, 1985 (cited from memory; the
publication details are **unverified**). The empirical fit of the density of cycle ranges of a Gaussian stationary process to
its four spectral moments. Alternatives not taken (listed in the open questions): Benasciutti and Tovo (a combination of the
narrow-band and the range-counting rates, 2005), Zhao and Baker (1992). The coefficient formulas below are written from memory
of the published form; the clean-room discipline is that the code is written from them and not from any implementation. Their
recall is corroborated, not verified, by the checks C2 and C3 below (the limit to the narrow-band rate as `zeta -> 0`, the
normalisation of the density and its moments by quadrature).

### Formulas

For a zero-mean Gaussian stationary stress with one-sided spectral moments `m_k = int_0^inf f^k G(f) df` (Hz), `k = 0, 1, 2, 4`:

```
x_m = (m1/m0) sqrt(m2/m4)          gamma = m2 / sqrt(m0 m4)
D1  = 2 (x_m - gamma^2) / (1 + gamma^2)
R   = (gamma - x_m - D1^2) / (1 - gamma - D1 + D1^2)
D2  = (1 - gamma - D1 + D1^2) / (1 - R)
D3  = 1 - D1 - D2
Q   = 1.25 (gamma - D3 - D2 R) / D1
```

The normalised amplitude `Z = a/sqrt(m0)` (`a` the amplitude, half of the range) has the density

```
p(Z) = (D1/Q) exp(-Z/Q) + D2 (Z/R^2) exp(-Z^2/(2 R^2)) + D3 Z exp(-Z^2/2).
```

The rate of cycles is the rate of peaks `E[P] = sqrt(m4/m2)` (Rice, Hz). On the power law `N(a) = c a^-m` the damage of a cycle is
`a^m/c` (Miner), so the damage rate is `E[P] m0^(m/2) E[Z^m]/c`. The three terms of `p` are an exponential and two Rayleigh
densities (scales `R` and `1`); with `int_0^inf Z^m exp(-Z/Q)/Q dZ = Q^m Gamma(1 + m)` and `int_0^inf Z^m (Z/s^2) exp(-Z^2/(2 s^2)) dZ
= (sqrt 2 s)^m Gamma(1 + m/2)`,

```
E[Z^m]  = D1 Q^m Gamma(1 + m) + (sqrt 2)^m Gamma(1 + m/2) (D2 |R|^m + D3)
rate    = sqrt(m4/m2) m0^(m/2) E[Z^m] / c           (1/s)
```

(`|R|`: only `R^2` enters the density, `R` is negative for some spectra). Narrow-band limit: all the cycles are peaks of a
Rayleigh amplitude, `D3 = 1`, `D1 = D2 = 0`, `E[P] = nu0`, and the rate becomes `nu0 (sqrt(2 m0))^m Gamma(1 + m/2)/c`, the formula of
F-413. Properties used: `D1 + D2 + D3 = 1` by construction, so `p` integrates to 1; every term of `p` is a probability density
when `D1, D2, D3 >= 0`, `Q > 0`.

### Design (code)

In `crates/rcfd-fem/src/fatigue.rs` (a new section appended after the narrow-band section; no existing function changes):

- `pub fn broadband_damage_rate(m0, m1, m2, m4, m, c) -> Result<f64>` the formula above (key `"broadband"`).
- `pub fn broadband_damage_rate_curve(m0, m1, m2, m4, &SnCurve) -> Result<f64>` on the power law of a curve (a table or an
  endurance limit refused, as in the narrow-band helper).
- `pub struct DirlikCoefficients { x_m, gamma, d1, d2, d3, r, q }` and `pub fn dirlik_coefficients(m0, m1, m2, m4) ->
  Result<DirlikCoefficients>`, so the density can be inspected and integrated by a test.

In `crates/rcfd-fem/src/random.rs`, one added `impl Moments` method pair, no change of an existing item:
`Moments::broadband_damage_rate(&self, m, c)` and `Moments::narrow_band_damage_rate(&self, m, c)` that pass `m0, m1, m2, m4` (and
`m0, m2`) to the functions unchanged (the field values are not recomputed, rescaled or rounded).

Refusals (key `"broadband"`, the message naming the quantity and its value): `m0` not finite or `<= 0`; `m2` not finite or `<= 0`
(a stricter condition than the narrow-band `m2 >= 0`: a zero `m2` has no rate); `m4` not finite or `<= 0`; `m1` not finite or
`< 0`; `m1` or `m4` such that `gamma > 1 + 1e-12` (the Cauchy-Schwarz bound `m2^2 <= m0 m4` is violated); the exponent `m` or
the constant `c` not finite and positive; a Dirlik coefficient outside its range: `D1 <= 0`, `D2 < 0`, `D3 < 0`, `Q <= 0`, `R >=
1`, or a non-finite value (the message names the coefficient, the value and the moments' `x_m` and `gamma`).

Stress moments: `RandomStress` carries the covariances `C_0` and `C_2` only, so a stress component has `m0` and `m2` but not
`m1`, `m4`. The moments that feed the rate unchanged are therefore those of `RandomResponse::moments` and
`ChannelResponse::moments` (displacement, velocity, acceleration outputs); a stress is fed by scaling an output's moments by the
square of a factor (`m_k -> k^2 m_k`, check C8, the rate scales as `k^m`). Moments of the stress components (`C_1`, `C_4`) are a
follow-up (open question Q3).

## Arithmetic before this registration (disclosed)

1. `dk.py`: the coefficient formulas above evaluated on moments by trapezoidal quadrature (2e6 points) of the one-mode displacement
   density `1/((1 - r^2)^2 + (2 zeta r)^2)`, flat force spectrum on `[0, 10 f_n]`, `f_n = 50` Hz. Ratio Dirlik/narrow-band:
   `zeta = 0.01`: `0.9933984` for `m = 3`, `0.9930121` for `m = 5` (`x_m = 0.93646`, `gamma = 0.94187`, `D1 = 0.0523`, `D2 =
   0.0125`, `D3 = 0.9352`, `R = 0.312`, `Q = 0.0654`); `zeta = 0.02`: `0.9888265`, `0.9885531`; `zeta = 0.05`: `0.9748168`, `0.9707606`
   (`R = -0.1818` negative here, the coefficients all otherwise in range). The deviation from 1 grows with `zeta`, as a rate
   formula fitted to broadband spectra and exact for none should. With the ADR 0477 integrals `I_0 = 39.26957079652609`, `I_1 =
   38.77258066850795`, `I_2 = 39.16923599186199`, `I_4 = 49.006230409610886` (`zeta = 0.02`, band `[0, 10]`, `f_n` scaling `I_k f_n^k`) the
   ratio is `0.9888264677733556` (`m = 3`), `0.9885530891481514` (`m = 5`).
2. `sim.py`, `sim2.py`: two modes `f_1 = 50`, `f_2 = 150` Hz, `zeta = 0.02`, unit modal masses, gains `u_1 = 1`, `u_2 = 3^1.5` (equal
   variance shares, ADR 0477 arithmetic 3), flat `G = 1` on `[0, 600]` Hz. Moments `m_0 = 4.03522e-7`, `m_2 = 4.99984e-3`, `m_4 =
   116.262`; `x_m = 0.6411`, `gamma = 0.7300`. For `m = 3`: narrow-band over Dirlik `1.19590`. A random-phase, Rayleigh-amplitude
   synthesis (`fs = 6000` Hz, `N = 2^22`, duration 699 s, about 1.06e5 peaks), six numpy seeds (not the library's generator),
   periodic rainflow by a three-point stack from the largest point: counted rate over Dirlik `1.052, 1.080, 1.073, 1.070, 1.056, 1.051`
   (unnormalised), over narrow-band `0.880 .. 0.903`. Realised variance over `m_0`: `0.989 .. 1.015`. So the estimate for the
   registered run is a counted rate 5 to 8 percent above Dirlik (inside the 10 percent limit with about 2 percent of
   margin) and 10 to 12 percent below narrow-band. With `N = 2^23` the cycles number about 2.1e5.
3. No other number was computed. The seed of the registered synthesis (20261114) was fixed before any run of the library
   generator.

## Checks (limits fixed now)

Common: power curve `SnCurve::power(m, c)`, `m = 3` unless stated, `c = 1` (the damage is a ratio; the response amplitudes are
about `1e-3`). One-mode system S: `f_n = 50`, `zeta = 0.01`, `m_mass = 2` kg, force gain `1/m_mass`, `G = 1e-2` flat on `[0, 500]`,
as `ModalChannels` with one mode and `spectral_response` at the registered grid (the setup of ADR 0477 S1, `zeta` changed).
Two-mode system T: `f_1 = 50`, `f_2 = 150`, `zeta = 0.02`, gains `1` and `3^1.5`, `G = 1` flat on `[0, 600]`, displacement, the
registered grid. "Rel" is `|a/b - 1|`. Every aggregate is checked finite before it is compared.

| Id | Clause | Quantity | Limit | Estimate |
|---|---|---|---|---|
| C1 | a2 | S: `|broadband/narrow-band - 1|`, `m = 3` | `<= 1e-2` | `6.6e-3` |
| C1b | a2 | the same, `m = 5` | `<= 1e-2` | `7.0e-3` |
| C1x | a2 | S: the ratio against `0.9933984349739842` (arithmetic 1) | rel `<= 1e-5` | `1e-6` (grid `1e-7`) |
| C2 | a1 | `zeta = 0.02`, the ratio from `RandomSolver` moments (bar B of ADR 0477, one mode) against `0.9888264677733556` | rel `<= 1e-5` | `1e-6` |
| C2b | a1 | `zeta = 0.05` ratio of channel moments against `0.9748167797070537` (the negative `R` case) | rel `<= 1e-5` | `2e-6` |
| C3 | a1 | density of the Dirlik coefficients at `zeta = 0.02`: `int_0^60 p dZ` and `int_0^60 Z^m p dZ` for `m = 3, 5` by Simpson (`2e5` intervals) against `1` and `E[Z^m]` | rel `<= 1e-8` | `1e-10` |
| C4 | a3 | T: counted rate (periodic synthesis, seed 20261114, `fs = 6000`, `N = 2^23`, `rainflow_periodic`, `miner`, divided by `N/fs`) against the broadband rate | rel `<= 1e-1` | `+5 .. +8 %` |
| C4b | a3 | the number of closed cycles counted | `>= 1e5` | `2e5` |
| C5 | a4 | T: narrow-band rate over broadband rate | `> 1` | `1.196` |
| C5b | a4 | T: narrow-band rate over the counted rate | `> 1` | `1.10 .. 1.14` |
| C5c | neg. | T: narrow-band over broadband minus 1 | `>= 0.10` | `0.196` |
| C6 | a5 | the one-mode response of the bar B through `RandomSolver`: `Moments::broadband_damage_rate(m, c)` bit-equal to `broadband_damage_rate(moments.m0, m1, m2, m4, m, c)`; likewise the narrow-band method against `narrow_band_damage_rate(sqrt(m2/m0), sqrt(m0), m, c)` | equal bits | exact |
| C6b | a5 | T: the same bit equality on the channel moments | equal bits | exact |
| C7 | a6 | `m0` in `{0, -1, NaN, inf}`, `m2` in `{0, -1, NaN}` refused, key `"broadband"`, the message contains the name and the value; `Moments::default()` refused | all | by construction |
| C7b | a6 | further refusals: `m4 <= 0`; `m1 < 0`; `m2^2 > m0 m4` (the message contains `gamma`); `m <= 0`; `c <= 0`; a table curve; a curve with endurance limit | all | by construction |
| C8 | scaling | moments `m_k -> k^2 m_k`: rate scales as `k^m`; frequency scaling `m_k -> s^k m_k`: rate scales as `s` | rel `<= 1e-12` | `1e-15` |
| C9 | consist. | `dirlik_coefficients`: `D1 + D2 + D3 - 1` at the moments of S and T | `<= 1e-15` | `0` |
| B1 | defaults | the digest of `narrow_band_damage_rate`, `spectral_rate`, `miner` and `rainflow_periodic` on fixed inputs, and `Moments::zero_crossing_rate`, recorded at the base before any library code | equal | by construction |

The estimates of C1 to C2b come from arithmetic 1 (the registered numbers are the trapezoid values of 2e6 points, whose own
error is below 1e-7), those of C4 to C5c from arithmetic 2 (a different generator and `N = 2^22`).

## Negative controls and planned mutations (on a scratch copy, one at a time)

Negative controls inside the tests: C5c shows that the broadband rate is resolved from the narrow-band one for T (a method equal
to the narrow-band rate would fail C5 and C5c); C2b (negative `R`, odd `m`) and C1b check the signs and the exponent `m = 5`; the
one-mode reduction C1 with a spectrum far from narrow-band is not claimed (the band of S is the registered one).

Mutations, each expected to be caught by the check named:

| Id | Mutation | Expected to be caught by |
|---|---|---|
| M1 | `x_m` computed with `m0` instead of `m1` | C1x, C2, C2b |
| M2 | the cycle rate `sqrt(m2/m0)` instead of `sqrt(m4/m2)` | C1x, C4, C5 |
| M3 | `R^m` instead of `|R|^m` | C2b |
| M4 | `Gamma(1 + m)` instead of `Gamma(1 + m/2)` in the Rayleigh terms | C1, C3, C4 |
| M5 | the factor 1.25 of `Q` dropped | C3, C1x |
| M6 | `Moments::broadband_damage_rate` passes `m2` as `m1` | C6, C6b |
| M7 | the refusal of `m2` weakened to `m2 < 0` | C7 |
| M8 | `(sqrt 2)^m` replaced by `2^m` | C1, C3, C4 |
| M9 | `D3 = 1 - D1 - D2` replaced by `D3 = 1 - D1` | C3, C9 |
| M10 | the Cauchy-Schwarz test removed | C7b |

## Decision rule

- **done:** every check C1 to C9 and B1 met, every mutation caught, the existing tests of `rcfd-fem` and the baseline hashes
  passing, fmt and clippy clean.
- **partial:** a clause of the acceptance missed or not run, the miss recorded as an ignored `*_known_miss` test with its
  numbers, or a supplementary check missed.
- **todo:** the formula not implemented.

## Limits (not covered)

- Gaussian, stationary, zero-mean stress; no mean-stress correction in the rate; no non-Gaussian (kurtosis, skewness) correction.
- Dirlik is an empirical fit (a few percent against counted damage for typical spectra, more for some bimodal ones; the check
  C4 measures one case); the lower bound of its validity is the set of spectra the fit was made on (not stated in the
  repository). No Benasciutti-Tovo or Zhao-Baker alternative is built.
- The curve is a single power law without endurance limit; a table curve or an endurance limit is refused (as in F-413).
- The stress moments `m1` and `m4` of a stress component are not produced by `RandomSolver` (it carries `C_0` and `C_2`); the
  rate takes the moments of an output channel.
- No case-file key, no CLI, no API schema.

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

- Q1. Method: Dirlik. Alternatives Benasciutti-Tovo (needs only `m0, m1, m2, m4` through the bandwidth parameters, has a closed
  rate in terms of the narrow-band and range-counting rates) and Zhao-Baker. Recommended: Dirlik first (the standard in
  commercial random-fatigue tools); add Benasciutti-Tovo as a second method later for a cross-check.
- Q2. Case-file key: not added. Recommended for the lead: `[fatigue] method = "narrow-band" | "dirlik"` next to the F-417 random
  outputs, `damage_rate` as an output per stress point and the equivalent life `1/rate`.
- Q3. Stress moments: `RandomStress` should carry the covariances `C_1` and `C_4` (weighted by `f` and `f^4`) so that a
  von Mises or a component stress feeds the rate without scaling an output channel. Recommended as a follow-up of F-417.

## Addendum 1: results

(to be written after the registered runs)

## For the lead: roadmap entries

(to be written after the registered runs)
