# ADR 0510: F-567 registration: complex cross-spectral matrices and frequency-dependent coherence of random loads (Hermitian positive semi-definite tables, pure delay, exponential coherence on a plate)

Status: **REGISTERED 2026-11-14, written before any code of F-567 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-567 was written, compiled or run in the repository. The files under "What was read"
were read; the arithmetic under "Arithmetic before this registration" was done by plain Python with numpy in the scratch folder
`w49i-work/f567-scratch` (`proto.py`, `proto2.py`, `proto3.py`; no rcfd code). The bit record of the default paths is a test
file written and run on the base 6c4ba2d9 after this commit and before any library line (D9). Results go in the addenda at the
end; nothing above the addenda is changed after a run.

Roadmap item F-567 (dep F-417), advanced tier, size S. Number 0510 given by the lead. Branch `f567-work` from main 6c4ba2d9,
worktree `w49i-work/f567`, scratch `w49i-work/f567-scratch`. House rules `w49i-work/AGENT_RULES.md`: limits fixed here, a miss
becomes an ignored `*_known_miss` test with the limit intact and the number recorded, each registered run made once, defaults
bit-identical, no edit of `baseline_hashes.rs`, the hash tables or any golden file, no new dependency, no unsafe code,
clean-room. The brief overrides rule 9: the F-567 entries of `docs/ROADMAP.md` and `docs/roadmap.json` are updated in a separate
last commit (the svgs are not regenerated); the proposed text is also in the addendum.

## What F-567 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. A cross-spectral table G_ab(f) with real and imaginary
> parts, Hermitian and positive semi-definite at every table frequency (refused otherwise with the frequency), reproduces the
> constant real correlation of F-417 bit for bit when its imaginary part is zero; two patterns with a pure delay tau (G_12 = G
> e^{-i 2 pi f tau}) on a one-mode system give the closed-form mean square within 1e-6; a field with coherence exp(-|x_a -
> x_b|/L) on a plate tends to the uncorrelated result as L goes to 0 and to the fully correlated one as L grows, each within
> 1e-8 at the registered limits.

Clauses: c1 a complex table, Hermitian and PSD at every table frequency, refused with the frequency; c2 bit for bit equal to
F-417 when the imaginary part is zero (and the correlation constant); c3 pure delay, one mode, closed form within 1e-6; c4
exponential coherence on a plate, both limits within 1e-8 at registered limits. Relative errors throughout unless stated.

## What was read (main 6c4ba2d9, read only)

ADR 0477 (F-417: D1 `Spectrum`, D2 `ModalChannels`, D3 the grid, D4 the real constant correlation `G_ab = rho_ab sqrt(G_a G_b)`
checked by `check_correlation`, D6 `RandomSolver`), ADR 0497 (F-566, methods added to `Moments`), `crates/rcfd-fem/src/random.rs`
(`frequency_grid`, `spectral_response` and its loop `cross(i, j) = sum_ab gab (yr yr + yi yi)`, `Excitation`, `RandomSolver::
response`), the tests `sa_random.rs`, `sa_random_default_bits.rs`, and the F-567 entry of `docs/ROADMAP.md`.

## Conventions and closed forms (first principles; textbooks cited from memory for the published form only)

Sources, from memory, nothing copied: D. E. Newland, An Introduction to Random Vibrations, Spectral and Wavelet Analysis (3rd
ed. 1993), ch. 7 to 9; S. H. Crandall and W. D. Mark, Random Vibration in Mechanical Systems (1963); J. S. Bendat and A. G.
Piersol, Random Data (4th ed. 2010) for coherence; for the exponential kernel being positive semi-definite in any dimension,
the Matern family with smoothness 1/2 (Bochner's theorem: `exp(-|x|)` is the Fourier transform of a positive density in every
dimension). Everything used is derived below.

**Cross-spectrum convention.** For the load spectra `F_a(omega)` the one-sided cross-spectral density is `G_ab(f) = E[conj(F_a)
F_b]` per Hz (the acceptance's `G_12 = G e^{-i 2 pi f tau}` is this convention with pattern 2 the delayed copy: `F_2(t) = F_1(t -
tau)` gives `F_2(omega) = F_1(omega) e^{-i omega tau}` and `G_12 = G e^{-i omega tau}`). `G(f)` is Hermitian, `G_ba = conj(G_ab)`,
and positive semi-definite. With the complex response `Y_ia(f)` of output `i` to unit load `a` (including the factor `(i omega)^k` of
the quantity), the output cross-density and covariance are

`S_ij(f) = Re sum_ab conj(Y_ia) Y_jb G_ab`, `C_ij = int S_ij df`.

Writing `G_ab = Gr + i Gi`, `Y = yr + i yi`: `Re(conj(Y_ia) Y_jb G_ab) = Gr (yr_ia yr_jb + yi_ia yi_jb) - Gi (yr_ia yi_jb -
yi_ia yr_jb)`. The first term is exactly the term of F-417; the second is new. For `i = j` and a Hermitian `G` the sum is real and
non-negative.

**Coherence form.** `G_ab = gamma_ab sqrt(G_a G_b)` with `gamma` Hermitian, unit diagonal, `gamma_ba = conj(gamma_ab)`; F-417's real
constant correlation is `gamma = rho`. `G` is PSD iff `gamma` is PSD where all `G_a > 0` (it is `D gamma D`, `D = diag
sqrt(G_a)`), and `G = 0` outside the common band.

**Pure delay, one mode.** One mode `H(f) = 1/(lambda - omega^2 + i c omega)`, `lambda = omega_n^2`, `c = 2 zeta omega_n`, loads with
gains `u_1, u_2` (`Y_a = u_a H`), flat `G`, `G_12 = G e^{-i omega tau}`, `G_11 = G_22 = G`: `S = G |H|^2 (u_1^2 + u_2^2 + 2 u_1 u_2
cos(omega tau))` (the sign of `tau` drops out for one output: only `Re G_12` enters). Independent route: the delayed copy is one
source through the combined response `Y_1 + Y_2 e^{-i omega tau}`, so `S = G |Y_1 + Y_2 e^{-i omega tau}|^2`; the two agree. Infinite
band closed form: `int_{-inf}^{inf} |H|^2 d omega = pi/(lambda c)` and the autocorrelation of the response to white noise,
`int_{-inf}^{inf} |H|^2 cos(omega tau) d omega = (pi/(lambda c)) rho(tau)`, `rho(tau) = e^{-zeta omega_n tau}(cos omega_d tau + (zeta
omega_n/omega_d) sin omega_d tau)`, `omega_d = omega_n sqrt(1 - zeta^2)`, `tau >= 0` (residues at `omega = +-omega_d + i zeta
omega_n`; the standard impulse-response autocorrelation, Newland ch. 8). With `d omega = 2 pi d f` and `int_0^inf = (1/2) int_{-inf}^{inf}`:

`m_0 = (G/(4 lambda c)) (u_1^2 + u_2^2 + 2 u_1 u_2 rho(tau))`. (CF)

The formula is for the infinite band; the registered band `[0, 400 f_n]` leaves a tail of relative size `8e-11` (arithmetic 1).
For a finite band `[0, B]` and for the two-mode checks the reference is an independent fine quadrature (D8).

**Exponential coherence.** `gamma_ab = exp(-|x_a - x_b|/L)`, real, `L > 0`. It is positive semi-definite for any set of points (the
exponential kernel is positive definite in every dimension). `L -> 0`: `gamma -> I` (the uncorrelated F-417 case, off-diagonal
`exp(-d/L)`, `0 < d_ab`); `L -> inf`: `gamma_ab = 1 - d_ab/L + O(L^-2) -> 1` (the fully correlated F-417 case, all ones). The mean
square is the quadratic form `sum_ab gamma_ab c_ab` with `c_ab = int Re(conj(Y_a) Y_b) G df`; hence the error against the limit is
`sum_ab (gamma_ab - gamma_ab^lim) c_ab`, bounded by `max_ab |gamma_ab - gamma_ab^lim| sum |c_ab|`: for `L -> 0` at most `exp(-d_min/L)` times
the cross sums, for `L -> inf` at most `d_max/L` times them (first order in `1/L`).

## Arithmetic before this registration (disclosed)

All with `f_n = 100` Hz, `zeta = 0.02`, `lambda = omega_n^2`, flat `G = 1`, `u_1 = 1`, `u_2 = 1.7`, the grid of ADR 0477 D3 at
refinement 1 re-implemented in Python (`proto.py`: 3-point Gauss-Legendre, steps `ell(f)/4`), references by 8-point
Gauss-Legendre on steps of 0.05 Hz (`proto.py`, `proto2.py`, `proto3.py`).

1. The closed form (CF) against the fine quadrature on `[0, 1000]`: `+3.9e-6` (the tail beyond 1000 Hz); on `[0, 40000]`: `+8.2e-11`.
   So (CF) is right and the band `[0, 400 f_n]` is registered for it.
2. The grid on `[0, 40000]` (192 nodes) against (CF): `tau = 1/(8 f_n)`: `-1.19e-7`; `tau = 3/(8 f_n)`: `+5.8e-8`. The cross term is
   `+62 %` resp. `-58 %` of `u_1^2 + u_2^2`. On `[0, 1000]` against the fine quadrature (141 nodes): `-1.15e-7`, `-1.13e-7`; the
   delay does not worsen the grid error of ADR 0477 (`-1.4e-9` at refinement 2, `-2e-11` at 4).
3. A table `G_12(f_k)` sampled every `h = 0.25` Hz on `[0, 1000]` (4001 entries) with linear interpolation of the real and imaginary
   parts, against the exact cosine: `tau = 1/(8 f_n)`: `-1.2e-7`; `tau = 3/(8 f_n)`: `+4.0e-6` (too coarse, so the table check uses
   the first delay only; the second is checked through the function form).
4. Two modes `f_1 = 60`, `f_2 = 140` Hz, `zeta = 0.02`, `Y_1 = H_1 + 0.5 H_2`, `Y_2 = 0.4 H_1 - 1.2 H_2`, one source delayed:
   reference `int |Y_1 + Y_2 e^{-i omega tau}|^2 df` on `[0, 1000]`; the grid (refinement 1) error `-1.12e-7` (`tau = 1/(8 f_n)`), `-1.09e-7`
   (`3/(8 f_n)`). The reference with the opposite sign of the delay (`e^{+i omega tau}`) differs by `+2.2e-2`, `+3.2e-2`, and the reference with
   the imaginary part dropped (`Re G_12 = G cos omega tau` only) by `+1.08e-2`, `+1.59e-2`: the two-mode check is sensitive to both.
5. Real frequency-dependent coherence `gamma(f) = e^{-f/200}`, one mode as above, `[0, 1000]`: grid error `-1.15e-7`.

## Design

### D1. The check of a cross-spectral matrix (`random::check_cross_matrix`)

For a matrix `G = Gr + i Gi` of `n` patterns at a frequency `f`, with `tr = sum_a Gr_aa` and `s = max_a Gr_aa` (key `"cross
spectrum"`; every message contains the frequency as `{f:e} Hz`, and the zero-based pair):

* every entry finite;
* the diagonal real and non-negative: `Gr_aa >= 0` and `|Gi_aa| <= CROSS_TOLERANCE s`;
* Hermitian: `|Gr_ab - Gr_ba| <= CROSS_TOLERANCE s` and `|Gi_ab + Gi_ba| <= CROSS_TOLERANCE s`;
* if `G` is not exactly zero: positive semi-definite in the sense that the smallest eigenvalue of the Hermitian part is at least
  `-CROSS_TOLERANCE tr`; tested exactly by the complex Cholesky factorisation of `H + CROSS_TOLERANCE tr I` (`H` = the Hermitian
  part `(G + G^H)/2`) having positive pivots (`H + eps I` is positive definite iff `lambda_min > -eps`; so no eigenvalue routine
  is needed). `CROSS_TOLERANCE = 1e-12`, the value of ADR 0477's correlation tolerance. An all-zero matrix (outside the band) is
  accepted.

### D2. The input (`CrossSpectrum`, `Coherence`)

`Coherence` (a Hermitian, unit-diagonal `gamma_ab(f)`): `Coherence::constant(rho)` (real, F-417's `rho`, checked by F-417's
`check_correlation`); `Coherence::table(frequencies, re, im)` (`re[k][a][b]`, `im[k][a][b]`, frequencies increasing, linear
interpolation of every entry, the end values outside the table; checked at every table frequency by D1 with `G = gamma`);
`Coherence::function(n, f)` with `f(freq, a, b) -> (re, im)` for `a < b` (the lower triangle is the conjugate), unit diagonal,
checked by D1 at every quadrature node. `CrossSpectrum::table(frequencies, re, im)`: the table `G_ab(f_k)` of the acceptance,
linear interpolation of every entry, zero outside `[f_0, f_{K-1}]`, checked by D1 at every table frequency.
`CrossSpectrum::coherent(spectra, coherence)`: `G_a` the F-417 `Spectrum` of each pattern, `G_ab = gamma_ab sqrt(G_a G_b)`
(`G_aa = G_a`), checked by D1 at every quadrature node (and a table coherence also at its own table frequencies).
`exponential_coherence(positions, length)`: the matrix `exp(-|x_a - x_b|/L)` (`L` finite and positive).
`spectral_response_cross(channels, cross, options)` returns the F-417 `ChannelResponse`; `Excitation::cross(patterns, cross)` and
`RandomSolver::response` take it (stress included); a base acceleration is one pattern and has no cross-spectrum.

### D3. How a real table reaches the existing arithmetic (c2)

1. Routing: a `CrossSpectrum::coherent` whose coherence is `Coherence::constant`, or a `Coherence::table` all of whose imaginary
   entries are exactly 0 and whose real entries are identical at every table frequency, calls the existing `spectral_response`
   with the spectra and that `rho`, so the result is the F-417 result by construction (the same function). Option
   `SpectralOptions::real_route` (default true) can switch the routing off.
2. Identical arithmetic order, proved by the check B3: the general loop computes `gab` in F-417's expression (`G_aa = G_a`, `G_ab =
   gamma_ab (G_a G_b).sqrt()` in this order), sums the real part exactly as F-417 does (`s += gab (yr yr + yi yi)` over `a` then `b`),
   and adds the imaginary part as a separate sum `t`, `s + t`: with every imaginary entry zero `t = +0.0` and `s + 0.0 = s`. Table
   interpolation of a constant entry `g0 + (f - f0)/(f1 - f0) (g1 - g0)` is `g0` exactly. B3 switches the routing off and compares bits.
3. `CrossSpectrum::table` with real entries: a table sampled as `G_aa = G_a`, `G_ab = rho (G_a G_b).sqrt()` at flat spectra is the
   F-417 case (the interpolation of constants is exact); B4 compares bits.

### D4. Refactoring rule

`frequency_grid` is split into `frequency_grid` (collects the breaks from the spectra) and a private function taking the breaks;
`RandomSolver::response` is split so the load decomposition and the stress path are shared and only the integrator differs. No
arithmetic of an existing path changes; the default-bit record (D9) and the existing tests prove it.

### D5. Case-file key

Not added (brief). Proposed in the open questions.

### D7. The plate

Steel (`E = 2.1e11`, `nu = 0.3`, `rho = 7850`) Hex8 plate `0.4 x 0.4 x 0.01` m, `8 x 8 x 1` elements (`hex_box`), clamped on `xmin`,
modal damping ratio 0.02, 12 modes by `modal_analysis` (tolerance 1e-12, backward residual test, one thread), static correction
off. Loads: unit z-forces at the nine top-face nodes `x in {0.15, 0.25, 0.35}`, `y in {0.1, 0.2, 0.3}` (`z = 0.01`), nine patterns,
each with the flat spectrum `G = 1` N^2/Hz on `[20, 1500]` Hz. `d_min = 0.1` m, `d_max = 0.2 sqrt(2) = 0.283` m. Output: the z displacement at the
node `(0.4, 0.2, 0.01)`; stress at all integration points (C3). "Plate size" is the side 0.4 m. The plate is a test model; it is not tuned.

### D8. References of D1 to D4, F1, F2

Computed in the test by its own code, independent of the library grid: composite 8-point Gauss-Legendre on steps of 0.05 Hz
(nodes from Newton iteration on the Legendre polynomial in the test) of the exact integrand (exact `cos`, exact complex response,
the exact interpolation formula for the tables of D3 and F2). Its own error is far below 1e-9 (steps are 1/40 of the half-power
width; checked in arithmetic 1 against (CF), `8e-11`).

### D9. Default bits

`crates/rcfd-fem/tests/sa_random_cross_default_bits.rs` (written and run at 6c4ba2d9, before any library line): FNV-1a digests of
`frequency_grid` on a mixed set of spectra and modes, `spectral_response` on one-mode and three-pattern channels with and without a
correlation, groups 1 and 3, and `RandomSolver::response` on the cantilever of `sa_random.rs` (forces uncorrelated, correlated, base
acceleration, stress). The digests are recorded as constants in a follow-up commit, before the library change; after it the same
test is run (B6) and `cargo test -p rcfd-api --release --test baseline_hashes`.

## Checks (limits fixed now)

Test file `crates/rcfd-fem/tests/sa_random_cross.rs`; every aggregated error is asserted finite before it is compared (a
`f64::max` over NaN hides failures). Parameters of section "Arithmetic": one mode `f_n = 100`, `zeta = 0.02`, `u = (1, 1.7)`, flat `G = 1`.

| id | check | limit | estimate |
|---|---|---|---|
| H1 | a 2-pattern table with `G_12 = 1 + 0.5i` and `G_21 = 1 + 0.5i` (not the conjugate) at table entry `k = 2` (150 Hz) refused, message contains `1.5e2 Hz` and the pair | refused | exact |
| H2 | `G_11 = G_22 = 1`, `abs(G_12) = 1.1` at one entry refused naming that frequency; `abs(G_12) = 1` (rank 1) accepted; `abs(G_12) = 1 + 1e-13` accepted; `abs(G_12) = 1 + 1e-10` refused (smallest eigenvalue `-1e-10 < -2e-12`) | as stated | exact |
| H3 | refused with the entry or frequency: negative diagonal, imaginary diagonal `> 1e-12 s`, a NaN, frequencies not increasing (names the entry), wrong shapes; an all-zero matrix accepted | refused / accepted | exact |
| H4 | `Coherence::function` with `gamma_12 = 0.5` below 300 Hz and `1.2` above: refused, the named frequency parsed from the message lies in `(300, 310)` Hz | as stated | the first node above 300 |
| H5 | a 3-pattern complex table (rank 2, Hermitian PSD by construction `G = B B^H`) accepted | accepted | exact |
| B1 | `Coherence::constant` (3 patterns, unequal flat, linear and log-log spectra, `rho` with 0.6, -0.3, 0.2) equals `spectral_response` bit for bit: moments, densities, covariance (`group` 1 and 3) | 0 ulp | exact (same function) |
| B2 | `Coherence::table` real and constant, 3 rows, same spectra: bits equal | 0 ulp | exact (routing) |
| B3 | the same with `real_route = false` (general loop): bits equal | 0 ulp | exact (D3.2) |
| B4 | `CrossSpectrum::table` real, sampled from flat spectra and `rho`: bits equal to F-417 | 0 ulp | exact (D3.3) |
| B5 | `RandomSolver::response` with `Excitation::cross` on the cantilever of `sa_random.rs` (two force patterns), stress on: densities, moments and stress covariances equal the `Excitation::forces` result bit for bit | 0 ulp | exact |
| B6 | the default-bit record (D9): digests of F-417 paths equal the base values after the change | equal | exact |
| D1 | `Coherence::function` pure delay `gamma_12 = e^{-i omega tau}`, band `[0, 400 f_n]`, against (CF): `tau = 1/(8 f_n)` and `3/(8 f_n)` | 1e-6 | `1.2e-7`, `5.8e-8` |
| D2 | the same on `[0, 1000]` Hz against the fine quadrature of D8 | 1e-6 | `1.1e-7` |
| D3 | `CrossSpectrum::table`, `h = 0.25` Hz, 4001 entries, `tau = 1/(8 f_n)`, `[0, 1000]`, against the fine quadrature | 1e-6 | `2e-7` |
| D4 | two modes, two loads (arithmetic 4), delay `tau = 1/(8 f_n)` and `3/(8 f_n)`, `Coherence::function`, against `int |Y_1 + Y_2 e^{-i omega tau}|^2 df` (fine quadrature) | 1e-6 | `1.1e-7` |
| D5 | the patterns exchanged and the table conjugated give the same mean square | 1e-13 relative | `1e-15` |
| D6 | the one-mode density at the nodes equals `G |H|^2 (u_1^2 + u_2^2 + 2 u_1 u_2 cos(omega tau))` | 1e-12 relative | `1e-15` |
| F1 | `Coherence::function` real `gamma(f) = e^{-f/200}`, one mode, `[0, 1000]`, against the fine quadrature | 1e-6 | `1.2e-7` |
| F2 | `Coherence::table` real frequency dependent (linear in `f`, 3 rows) against the exact interpolation formula in the quadrature reference | 1e-6 | `1.2e-7` |
| E1 | `exponential_coherence` entries equal `exp(-d/L)` for known distances | 1e-15 | `1e-16` |
| C1 | plate (D7), `L_0 = d_min/40` (`exp(-40) = 4e-18`): the mean square of the tip displacement equals the uncorrelated one (`Excitation::forces`, no correlation) | 1e-8 | `1e-16` |
| C2 | plate, `L_inf = 1e10 x` plate side (4e9 m): equals the fully correlated one (all-ones correlation) | 1e-8 | `7e-11` times the cancellation factor, see below |
| C3 | the same two limits for the von Mises stress (maximum over the integration points of the relative difference in the root-mean-square von Mises stress) | 1e-8 | as C1, C2 |
| C4 | supplementary: `L = 1e-6 x` side equals the uncorrelated result (1e-8); `L = 1e6 x` side recorded only; error at `1e6 x` side over error at `1e8 x` side in `[90, 110]` (first order in `1/L`) | as stated | `0`; `~3e-6`; `100` |
| C5 | PSD acceptance of `exponential_coherence` at `L` from `d_min/40` to `1e10 x` side by the check of D1 (no refusal) | accepted | exact |

Estimate of C2: the relative error is at most `(d_max/L) sum|c_ab| / |sum c_ab|`; `d_max = 0.283` m, `L = 4e9` m gives `7e-11` times the
cancellation factor `sum|c_ab|/|sum c_ab|` of the plate response, expected below 10; the margin to 1e-8 is a factor 20 at a
cancellation factor 7. If C2 misses, the cancellation factor is recorded and the check becomes a known miss (no new `L` is tried).

## Negative controls and mutations

Negative controls (in the test file, each a live comparison): N1 the delay of D4 with the opposite sign differs from the reference
by at least `1e-2` (arithmetic 4: `2.2e-2`, `3.2e-2`); N2 the imaginary part dropped differs by at least `5e-3` (`1.1e-2`, `1.6e-2`); N3 on
the plate, `L = d_min` differs from the uncorrelated result by at least `1e-3` and from the fully correlated one by at least `1e-3` (so
the limits C1, C2 can fail); N4 the one-mode delay of D1 with `u_2 = 0` (no cross term) differs from (CF) of `u_2 = 1.7` by at least `0.1`.

Mutations, on a scratch copy with its own `CARGO_TARGET_DIR`, one at a time, each expected to be caught by the named checks (the
run records what caught it):

| id | mutation | expected catch |
|---|---|---|
| M1 | sign of the imaginary term flipped | D4, D5 |
| M2 | imaginary term dropped | D4 |
| M3 | the Hermitian check removed | H1 |
| M4 | the PSD check without the shift (`eps = 0`) | H2 (the rank-1 case) |
| M5 | the PSD check only at the first table frequency | H2 |
| M6 | the route taken when the imaginary part is zero even if the real part varies with `f` | F2 |
| M7 | the real and imaginary sums merged in one accumulator (changes the order) | B3 (bits) |
| M8 | `exponential_coherence` with `exp(-d^2/L)` | E1 |
| M9 | the table interpolation replaced by nearest-neighbour | D3, F2 |

## Decision rule

**done** if H1 to H5, B1 to B6, D1 to D6, F1, F2, E1, C1 to C5 are met at the limits above, every negative control holds, every mutation
is caught, the existing tests of `rcfd-fem` (release), `baseline_hashes`, clippy and fmt pass. **partial** if any check misses (it
becomes an ignored `*_known_miss` with the number), a mutation is not caught, or a clause of the acceptance is not run. **todo**
otherwise.

## Limits (not covered)

The coherence of `Coherence::function` is checked at the quadrature nodes, not continuously; a base acceleration has no
cross-spectrum; stationary Gaussian loads only; coherence models other than the exponential one are user functions, none ships (a
Corcos-type wall-pressure coherence is a user function and a later item if the maintainer wants a named model); the case-file key
is not added.

## Open questions (with recommendations; none changes a default, golden or hash)

1. A case-file key for cross-spectra (a `random.cross` section naming the table or the coherence model and its length)? Option A:
   none now. Option B: a key `random.coherence = {kind = "exponential", length = L}` plus `random.cross_table`. Recommendation:
   A until the fan workflow needs a case file; the key would change the schema golden and is the maintainer's call.
2. Interpolation of `G_ab` between table frequencies: linear in the real and imaginary parts (taken; a delay phase is
   interpolated poorly for a coarse table, quantified in arithmetic 3) or in magnitude and phase. Recommendation: keep linear, state in the
   documentation that a delay needs `2 pi tau h << 1`, and use `Coherence::function` for analytic coherence.

Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation): the convention `G_ab = E[conj(F_a) F_b]`
(the acceptance's sign), the shift-by-trace PSD test with `1e-12`, the exponential coherence as a library helper.
