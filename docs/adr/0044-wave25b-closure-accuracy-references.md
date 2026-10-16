# ADR 0044: Wave 25B, closure accuracy repairs and reference verification (F-217, F-220, F-227, chemistry output evidence, literature audit)

Status: accepted (decisions and numeric limits registered before any code change or post-fix run). Results are in the addendum at
the end, written after the runs.

## Context

Wave 21B (ADR 0036), 21C (ADR 0037), 23A (ADR 0039) and 23B (ADR 0040) recorded misses as ignored `*_known_miss` tests with the
registered limit intact, and wrote many literature citations from memory (marked UNVERIFIED in `docs/reference-data.md`). This wave
(a) repairs or diagnoses the misses, (b) adds independent evidence where a passing check shared code with its reference (C4), and
(c) verifies the citations against public pages. Rules carried over: limits below are registered before the corresponding code is
changed or run; no registered limit is widened, replaced or deleted; old known-miss tests stay ignored with their limit; a newly
passing item gets a NEW passing test with the same registered limit; all existing tests and `docs/baseline` hashes stay bit-identical.

Pre-change survey (done before registering): `CrossBothUnmixed` / `crossflow_unmixed` is used only by `crates/rcfd-fv/src/hx.rs`,
its tests, the schema enumeration and doc string, and docs; no baseline case, no `rcfd-api` test and no case under `cases/` evaluates
it numerically. The fit can therefore be replaced behind the same API without changing any existing passing test or hash. `PtTable::eval`
is relied on by T8 (bilinear exactness) and the liquid half of T9 and is not changed.

## D1 F-217 both-unmixed crossflow: exact series replaces the curve fit

Derivation (first principles, clean-room). Hot stream (the `C_min` stream) flows in `x`, cold stream in `y`. With
`xi = N x`, `eta = N Cr y` (so `xi in [0, N]`, `eta in [0, M]`, `M = N Cr`, `N = UA/C_min`, `Cr = C_min/C_max`):
`h_xi = -(h - c)`, `c_eta = h - c`, `h(0, eta) = 1`, `c(xi, 0) = 0`. The double Laplace transform gives
`c_hat = 1/(q (p + q + p q)) = (1/q) sum_{n>=1} [(p+1)(q+1)]^(-n)`, hence
`c(xi, eta) = sum_{n>=1} [xi^(n-1) e^(-xi)/(n-1)!] P(n, eta)`, with `P(n, y) = Pr(Poisson(y) >= n) = 1 - e^(-y) sum_{k<n} y^k/k!`
(regularised lower incomplete gamma, integer shape). The mean cold outlet temperature is `(1/N) int_0^N c(xi, M) dxi`, the energy balance
gives `eps = c_bar/Cr`, and `int_0^N xi^(n-1) e^(-xi)/(n-1)! dxi = P(n, N)`, so

`eps(N, Cr) = (1/M) sum_{n>=1} P(n, N) P(n, M)`,   `M = N Cr`.

(This is the Schmidt/Mason-type Poisson-tail series; it is derived here and not copied. Whether it coincides with the published
form is a literature question handled in Part 2 only if a page states it.)
Implementation rules: the first tail is `-expm1(-x)`; `P(n+1, x) = P(n, x) - e^(-x) x^n/n!` is advanced forward with the pmf in log
space (`ln n!` by exact sum for `n < 20`, Stirling series after), so no upper tail is obtained by subtracting from 1 once it is
small; each stream starts at `n_s = max(1, floor(x - 12 sqrt(x) - 20))` where `P` is set to 1 (error below `e^-70`), the terms
below the smaller start are exactly 1, the loop ends at `min(N, M) + 12 sqrt(min) + 40` where the product is negligible. `Cr = 0`
keeps the exact `1 - e^(-N)`. Result clamped to `<= 1` only to remove round-off overshoot. The fit is removed (its formula is kept in
the ADR and in one diagnostic test); no option flag, since nothing depends on it.

Derived limits used as registered checks (all derived here, not recalled):
* small `N`: `eps = N - N^2 (1 + Cr)/2 + O(N^3)` (series terms `n = 1` and `n = 2`).
* `Cr < 1`, `N -> inf`: for `n` in the support of `P(n, M)`, `P(n, N) ~ 1` and `sum_{n>=1} P(n, M) = M` exactly, so `1 - eps` is
  exponentially small, about `exp(-(sqrt N - sqrt M)^2)`.
* `Cr = 1`, `N -> inf`: `sum P(n,N)^2 ~ N - sqrt(N) * 2 int_0^inf Phi (1-Phi) dz = N - sqrt(N)/sqrt(pi)` (Gaussian approximation of the Poisson
  tail, `int Phi(1-Phi) dz = 1/sqrt(pi)`), so `1 - eps ~ 1/sqrt(pi N)` (algebraic, not exponential).

Registered checks (new tests in `crates/rcfd-fv/tests/heat_exchanger.rs`, named `h5_*`; written before the implementation):
* **X1 marched 2-D agreement.** Reference: the existing independent cell recursion `cross_unmixed(cells, n, cr)` (each cell a cocurrent
  exact exchange), Richardson-extrapolated `2 f(800) - f(400)` (second order in the cell size). Grid `N in {0.1, 0.5, 1, 3, 8}`,
  `Cr in {0.1, 0.25, 0.5, 1}`: relative deviation `<= 1e-4` for every point. The reference convergence `|f_R(400,800) - f_R(200,400)|/f` is
  printed. If the reference itself is not converged to 1e-4 this is reported as a reference-limited result, the limit is not widened.
* **X2 small-N slope.** `|eps/N - 1| <= 1e-6` at `N = 1e-7`, `Cr in {0.25, 0.5, 1}`; second-order coefficient
  `|(eps - N)/N^2 + (1 + Cr)/2| <= 1e-3 (1 + Cr)/2` at `N = 1e-4`. The old registered H2 limit (`|eps/N - 1| <= 1e-3` at `N = 1e-3`, `Cr = 0.5`)
  is also asserted; the exact solution deviates by `(1 + Cr)N/2 = 7.5e-4` there. (The literal "within 1e-6 at N = 1e-3" is impossible for
  the exact solution; the 1e-6 limit is registered at `N <= 1e-7`.)
* **X3 bounds.** On the registered H2 grid (`N = 0.05 i`, `i = 1..200`; `Cr = 0.05 j`, `j = 1..19`, plus `Cr = 1`): `parallel <= cross <= counterflow` with
  slack `1e-12`, `0 <= eps <= 1`, `eps` nondecreasing in `N` and nonincreasing in `Cr` (slack `1e-12`): zero violations. Strict inequality
  `parallel < cross < counterflow` at `N = 1, Cr = 0.5`. `ntu_for_effectiveness` round trip `|back/N - 1| <= 1e-9` for the registered H4 grid.
* **X4 limits.** `Cr -> 0`: `|eps(N, 1e-9) - (1 - e^(-N))| <= 1e-8` for `N in {0.1, 1, 5}` (this evaluates the series, not the `Cr = 0` branch);
  `Cr = 0.5`, `N = 400`: `1 - eps <= 1e-9`; `Cr = 1`, `N = 1e4`: `|(1 - eps) sqrt(pi N) - 1| <= 0.05` (derived asymptotic with `O(1/sqrt N)` correction);
  `eps(N, Cr)` finite and in `[0, 1]` for `N = 1e6`, `Cr in {0.01, 0.5, 1}`, and the call at `N = 1e4` finishes (run time reported).
* **X5 old tests.** The three ignored tests (`h2_cross_unmixed_slope_known_miss`, `h2_limits_and_ordering_known_miss`, `h3_cross_unmixed_known_miss`) are
  run with `--ignored` and are expected to pass; they stay `#[ignore]` with a note; `h5_*` re-assert the same limits (slope `1e-3` at `N = 1e-3`,
  the full ordering grid, H3 `2e-2` against the 800x800 recursion). One diagnostic test asserts that the retired fit still deviates
  by more than 2 percent at `N = 0.5, Cr = 1` (documents what was replaced).

## D2 F-220 vapour half of T9: a density-specific basis, opt in

Scope fact: the failing T9 vapour table has a dummy saturation line and is entirely vapour; the saturation-line interpolation (ln p vs 1/T,
monotone Hermite in the saturation table) cannot influence it, and is not touched. The error is in the bilinear interpolation of `rho` itself.

Decision: add an opt-in method `PtTable::eval_density_ideal_scaled(t, p)` that interpolates bilinearly the quantity `rho T/p` (equal to
`1/(R_s Z)`, constant for an ideal gas) and returns `(p/T) * q`. It is specific to a density grid and positive `T`, `p`; documented as such;
`eval` is unchanged (default), no schema or stored-state change, not applicable to the liquid grid or other properties. A bilinear interpolant
of `Z`-like data is exact for an ideal gas and its error for steam is set by the curvature of `1/Z` only.

A priori estimate (second virial, before running): `Z - 1 = B(T) p/(R T)`; at 380 K `Z - 1 = -0.013` per 100 kPa. The pressure curvature of `1/Z` is
`(0.013 x)^2` with `x = p/100 kPa`, interpolation error over a 10 kPa cell `h^2 f''/8` with `f'' = 2 a^2`: `4e-7`. The temperature curvature of
`Z - 1 ~ T^-n` (n about 5) at 15 kPa (`Z - 1 = -2e-3`): `(h/T)^2 n(n+1)/8 * 2e-3` about `1e-6`. Expected maximum mid-cell error a few `1e-6`, i.e. at least
a factor 10 below the limit. Plain bilinear on `rho` is expected to remain at `2.64e-4` (its known miss stays ignored and is also re-measured).

Registered checks (new tests in `crates/rcfd-core/tests/tabular.rs`, `t9b_*`):
* **B1** an ideal-gas density table (`rho = p/(R T)`, coarse 5 K x 10 kPa grid): the scaled method reproduces it at 25 interior points to `1e-12` relative;
  plain `eval` does not (its error is larger than `1e-5`, discrimination).
* **B2** nodes reproduced to `1e-12` relative for the registered T9 vapour grid.
* **B3** the registered T9 vapour grid (`T = 380..500 K` step 5, `p = 10..100 kPa` step 10, `iapws::water_state`): at every cell centre the scaled method has relative error `<= 1e-4`
  (the registered limit, unchanged); additionally at the quarter points `(0.25, 0.75)^2` of every cell, same limit. Result and the plain-`eval` number are printed.
* **B4** errors: `t <= 0` or `p <= 0` or non-finite node values give a located `Err`; a NaN corner gives the same located error as `eval`; out of axes is the same `Err`.

If B3 fails the miss stays recorded and the principled change is reported as insufficient; no further tuning.

## D3 F-227 wave damping: test the stated cause separately, then decide

Cause under test: in `WaveTank` the relaxation multiplies `eta` and `u` by `w` after every step, i.e. a continuous damping rate `alpha = -ln(w)/dt`
that grows without bound as `dt` shrinks. With the registered exponential blend over `L = 0.05 lambda = 2` cells the outer cell has
`w(0.75) = 0.7436` (rate 3.0 1/s, against `omega = 0.785 1/s`): a strongly absorbing layer in front of the wall, not a rigid one.

Analytic argument (registered before running). (i) Thin uniform layer of thickness `l`, damping `alpha` on `eta` (volume leak) with wall behind it:
`R = (1 - Y)/(1 + Y)`, `Y = alpha l/c - i k l`. For `l = 0.05 lambda` (`k l = 0.314`), `R` is not near 1 unless `alpha l/c` is small
(`alpha l/c = 0.3` gives `|R| = 0.55`, `0.6` gives `0.31`). `|R|` tends to 1 both as `alpha -> 0` and as `alpha -> infinity` (a stiff layer forces `eta = 0`),
non-monotone in between. Hence "an abrupt zone reflects at least half" has no content without a bound on `alpha l/c`; with the registered blend
(`alpha l/c` of order 0.6 to 0.9) the continuum estimate is `R` of 0.2 to 0.3, in line with the measured `0.214`. The premise is ill-posed, not the code.
(ii) Sponge `sigma(x)`, damping both fields at the same rate: a travelling wave decays as `exp(-int sigma dx/c)`, so the round trip gives
`R = exp(-2 int sigma dx/c)`. The test's sponge (`sigma_max = 3c/(2 lambda)`, `n = 2`, `L = 2 lambda`) has `int sigma dx = sigma_max L/(n+1) = c`, so `R = e^-2 = 0.1353`;
the measured `0.135` is round-trip attenuation, not a free parameter. (The sponge variant had no registered limit; this registers one now.)

Registered checks (new tests in `crates/rcfd-fv/tests/waves.rs`, `w8_*`; none changes `w7d`/`w7e`):
* **R-ref: independent discrete frequency-domain solver (in the test).** It solves the steady harmonic problem of the same staggered scheme,
  following the code's step order exactly: `u^{n+1}_j = wu_j [u^n_j - (g dt/dx)(eta^n_j - eta^n_{j-1})]`, then
  `eta^{n+1}_i = we_i [eta^n_i - (h dt/dx)(u^{n+1}_{i+1} - u^{n+1}_i)]`, `z = exp(-i omega dt)`, from the wall (`u_N = 0`) backwards (a two-term backward
  recursion, unknown scale), and decomposes the lossless region into the two discrete plane waves `q^(+-i)` with
  `q + 1/q = 2 - 4 sin^2(omega dt/2)/(c dt/dx)^2` (the discrete dispersion of the undamped scheme). The weight arrays are those the tank builds (the same position rule).
* **W8-1** time-domain `R` (the existing `reflection` helper) equals the solver's `R` within `5e-3` absolute for: the registered abrupt exponential zone (`L = 0.05 lambda`),
  the same blend with `L = 1`, `4`, `8` cells, and the registered sponge (`L = 2 lambda`, `n = 2`). The solver value for the registered abrupt zone is also printed (expected near 0.21).
* **W8-2** dt dependence: same weights, `dt = T/160` (CFL 0.25): time-domain `R` equals the solver's within `5e-3`; and `R(dt/2)` differs from `R(dt)` by more than `0.1` for the
  registered abrupt zone (the damping rate per second doubled, the per-step weight fixed): demonstrates the cause. (Direction not pre-claimed because `|R|` is non-monotone in the rate.)
* **W8-3** sponge: `|R - exp(-2 int sigma dx/c)| <= 5e-3` with `int sigma dx` by the Simpson quadrature of W6 (`R = 0.1353` expected).
* **W8-4** continuum estimate (ODE integration of the layer with `alpha_e(x) = -ln(we)/dt`, `alpha_u(x) = -ln(wu)/dt`, piecewise constant on the tank cells and face volumes, wall at the end):
  `|R_ode - R_discrete| <= 0.25 R_discrete` for the registered abrupt zone. Loose by design: it is the physical explanation, with errors of order `(k dx)^2` and of the rate definition.
* **W8-5 corrected well-posed thin-zone expectation** (replacement for the registered W7(d) premise, which stays ignored): a thin zone `L = 0.05 lambda` with
  uniform per-step weight `w = 0.99` on both fields (sponge with exponent `1e-9`, `sigma = -ln(0.99)/dt`; `alpha l/c = 0.04`) reflects `R >= 0.8` in the
  time-domain run and in the solver (continuum estimate 0.93); and a graded strong zone (`w = 0.8`) reflects `R <= 0.4` (estimate 0.17).
  The statement is: a thin zone is a rigid reflector iff its integrated per-second damping is small.

## D4 Chemistry output evidence (C4 independent check)

C4 (ADR 0037) compares the VTU arrays with `cell_rates`, the same code the output uses, so it checks plumbing and units, not rates. It is kept.
New test `c4b_*` in `crates/rcfd-cli/tests/chemistry_run.rs` with NO call to `cell_rates`, `CellModel`, `ChemState` or any reactor routine:
* **Isothermal first-order closed form**: closed box, `A => B`, `A_pre = 5`, `Ea = 0`, `b = 0`, fixed temperature, heat release off, `write_rates = true`, uniform `rho = 1.5`,
  `W = 0.020 kg/mol`. Final `Y_A` must equal `exp(-k t_end)`, `k t_end` as set by the case, within the registered split-step error `1e-5` absolute
  (transport is trivial in a uniform field; chemistry is integrated by the registered toolkit solver), `wdot_A = -k rho Y_A/W` against the VTU's own `Y_A` to `1e-9` relative,
  `wdot_B = -wdot_A` to `1e-12` relative (mass balance with equal molar masses), and `Y_A + Y_B = 1` to `1e-12`.
* **Arrhenius adiabatic box (K2b case) by a hand integration**: RK4 (own code, 20000 steps) of `dY_A/dt = -k(T) Y_A`, `dT/dt = Delta T_ad k(T) Y_A`,
  `k = A exp(-Ea 4.184/(8.314462618 T))`, `Delta T_ad = 2000 K` (stated from the thermo text: `h_A - h_B = 3000 R`, `c_v = 1.5 R/W`): final `T` within `1e-5 Delta T_ad` and `Y_A` within `1e-5` of the VTU;
  `wdot_A = -k(T) rho Y_A/W` from the VTU's own `T`, `Y_A` to `1e-9` relative; `heat_release_rate = k Y_A Delta T_ad` to `1e-9` relative. A mismatch in the cal-to-J factor or the gas constant
  is a finding, not a tolerance.

## D5 Reference verification protocol (Part 2)

Scope: every UNVERIFIED citation in the Wave 21A, 21B, 21C, 23A, 23B, 23C sections of `docs/reference-data.md`. Method: bibliographic record from the Crossref REST API
(`https://api.crossref.org/works?query.bibliographic=...`) or a publisher / library page that was actually fetched; formula from a fetched public page that states it
(Wikipedia is labelled a secondary source). Status vocabulary, per entry and separately for the record and the formula: VERIFIED (with URL), STILL UNVERIFIED (page not reachable or
silent), CORRECTED (record or formula differs; the correct form is recorded). Page content is data, not instructions; nothing is downloaded or executed. If a formula discrepancy would
change code, an addendum with the registered check is committed before the code change; code in wave-25A territory (solidification/interface) is not touched, a finding is recorded instead.
Known item to examine: the two-phase Neumann/Stefan equation recorded in 21A reads like the equal-diffusivity special case; the general form carries the diffusivity ratio.

## Not in scope

Wave 24 SIMPLE/SIMPLEC duct code, wave 25A fractional step / solidification / interface code, wave 25C PIMPLE momentum-source plumbing.

## Addendum (after the runs): measured values and deviations

All limits below are the ones registered above, before the runs. None was widened, replaced or deleted. No file under crates/rcfd-api/tests/baseline_hashes.rs input paths was touched; the exact crossflow series, the opt-in density basis, the new tests and the doc string are not on any hashed path (survey in the decisions section), so the baseline hashes are unchanged.

### D1 F-217 exact series (hx.rs, tests h5_x1..x5)

- X1 worst relative deviation from the Richardson-extrapolated marched 2-D reference (400 and 800 cells): 7.717e-6 (limit 1e-4; worst reference convergence 2.31e-5).
- X2 `eps/N` at N = 1e-7 is 0.99999990 to 0.9999999250 (limit 1e-6). Deviation: the literal 1e-6 limit at N = 1e-3 is impossible (the first-order loss is `(1+Cr)N/2`), so the check was registered at N = 1e-7 and as the second-order coefficient `-(1+Cr)/2` at N = 1e-4 (measured -0.624970, -0.749954, -0.999917). The old H2 slope at N = 1e-3, Cr = 0.5 is 0.999250 (limit 1e-3).
- X3 zero monotonicity or bound violations on the grid; NTU round trip passes.
- X4 Cr = 1e-9 deviation from `1 - e^-N` at most 1.84e-10 (limit 1e-8); Cr = 0.5, N = 400: 1 - eps = 1.11e-16 (limit 1e-9); Cr = 1, N = 1e4: 1 - eps = 5.64186e-3 versus the derived `1/sqrt(pi N)` = 5.64190e-3; N = 1e6 takes about 1 ms.
- X5 the three older ignored tests (H2 slope, H2 ordering, H3 against 800x800) pass under `--ignored` (H3 worst deviation 9.22e-7). They stay `#[ignore]` with a note, limits intact. The retired fit deviated 3.33e-2 at N = 0.5, Cr = 1. Independent check: the published series quoted at the NTU-method Wikipedia page agrees with the implementation to 9 digits at five points.

### D2 F-220 vapour table (tabular.rs, tests t9b_b1..b4)

- B1 ideal-gas table reproduced to 3.89e-16 by the scaled basis (plain bilinear 4.27e-5).
- T9 vapour table: scaled basis centre worst 1.239e-5 and quarter points worst 9.751e-6, limit 1e-4 (both met). Plain bilinear centre 2.642e-4, so the original known miss stays ignored with the limit intact.
- Deviation: the pre-run estimate in this ADR ("a few 1e-6") underestimated the error by about 3x; the result is still 8x inside the limit. The basis `q = rho T/p` is physically motivated (ideal gas gives constant q), not fitted to the T9 table. `eval` is unchanged; the new method is opt-in `eval_density_ideal_scaled`.

### D3 F-227 wave damping (tests w8_0..w8_5, w8_4b)

- Independent discrete frequency-domain solver agrees with the time-domain run to 1.16e-7 worst (limit 5e-3); lossless |R| = 1.000000000000.
- Exponential zones of 1, 2, 4, 8 cells: R = 0.79671, 0.21440, 0.07259, 0.05051. The registered sponge: R = 0.13476, matching `exp(-2 int sigma dx/c)` = 0.13534 with int sigma dx / c = 1.00000 (W8-3, limit 5e-3). So the sponge R = 0.135 is the round-trip attenuation e^-2, not a bug.
- W8-2: halving dt (T/160) gives R = 0.04647 against 0.21440 at T/80 (|change| > 0.1 met). Deviation: my pre-run guess of the direction (R rising) was wrong, the registered check was the size of the change only.
- Cause test (per-step multiplication by w < 1): confirmed as the mechanism, but the premise of W7(d) is ill-posed. The registered exponential blend gives outer-cell w(0.75) = 0.7436, a damping rate of about 3.0 per second against omega 0.785, a strong absorber. A thin zone is a near-rigid reflector only when its integrated per-second damping is small (W8-5: uniform w = 0.99 gives R = 0.92359, limit >= 0.8; w = 0.8 gives R = 0.10550, limit <= 0.4). |R| is not monotone in the rate.
- MISS: W8-4 continuum estimate (relative limit 25 percent): continuum 0.27398 against discrete 0.21440, +27.8 percent; the test is kept as `w8_4_continuum_estimate_known_miss` (ignored). With the (1-w)/dt rate the continuum gives 0.33445; for 4 and 8 cells it gives 0.00910 and 0.00175 against 0.0726 and 0.0505, because the per-step damping sigma dt reaches 0.3 to 1.3 in the outer cells, outside the continuum's validity.
- Post-hoc diagnostic `w8_4b` (labelled post-hoc, limit 5 percent chosen after seeing the miss): uniform thin layers w = 0.99 and 0.97 agree with the discrete solution within 0.1 and 0.4 percent, which supports the diagnosis that the continuum estimate fails only where per-step damping is large.
- The old `w7d` test stays ignored (R = 0.214 against >= 0.5, note added). `w7e` is unchanged. No change to waves.rs source.

### D4 chemistry independent checks (chemistry_run.rs, c4b and c4c)

C4 is kept unchanged.
- c4b isothermal first-order (k = 5/s, dt = 1e-3, end 0.4): Y_A against `exp(-kt)` max abs 2.0e-15 (limit 1e-5); wdot_A against `-k rho Y_A/W` 2.8e-16 (limit 1e-9); `wdot_B + wdot_A` = 0; `sum Y - 1` = 0.
- c4c adiabatic Arrhenius box against my own RK4 hand integration (A exp(-Ea 4.184/(8.314462618 T)), dT_ad = 2000 K), stopped at 1.1e-4 s: T = 1729.196822 K, Y_A = 0.53540159; |T - T_hand|/dT_ad = 2.92e-10 and |Y - Y_hand| = 2.92e-10 (limits 1e-5); wdot_A, wdot_B and heat release rate relative deviations about 1.07e-10 (limits 1e-9). This confirms the cal to J factor 4.184, R and dT_ad independently of the production code path.
- Set-up defect, disclosed: the first run used the K2b end time of 2e-3 s, far past thermal runaway (about 1.2e-4 s) where Y_A underflows, so relative errors of the final-state rates were degenerate (1.0). The end time was corrected to 1.1e-4 s with the same limits; the degenerate form was never committed.

### D5 Reference verification

The verification table is in docs/reference-data.md, section "Wave 25B reference verification". Summary: record verified for Kim and Moin, Zang et al., Timmermans et al., Kumar and Ramkrishna, McGraw, Saffman and Turner, Hill and Ng, Gordon, Wheeler, Smoluchowski, Jacobsen et al., Hartmann; corrected: Scott is William T. Scott; Lee report is probably 1979 (primary unconfirmed); formulas verified (secondary or derived): effectiveness-NTU forms, exact both-unmixed series, JONSWAP shape factors, Pierson-Moskowitz Hs-wp form, Neumann one-phase limit; the ADR 0035 two-phase Neumann equation is the equal-diffusivity special case of the general condition (no code change). Still unverified: Saffman-Turner constant, Lee and Zwart primaries (Zwart R_B 1e-6 vs 2e-6 m reported by different sources), Jacobsen blend attribution and exponent, shell-and-tube formula, Nusselt, Schrage, Rohsenow, Stokes second order, Brownian and shear kernels, Kumar-Ramkrishna formulas, Wheeler algorithm text. No code changed because of a reference discrepancy.
