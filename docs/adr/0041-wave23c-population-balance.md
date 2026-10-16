# ADR 0041: Wave 23C, population balance and polydispersed phases (F-228)

Status: accepted. Everything under "Decisions" and "Registered checks" is written before any code of this wave exists.
Limits are not changed, widened, replaced or deleted after a run. A miss is reported as measured and the assertion moves to an
ignored `*_known_miss` test with the registered limit intact; the cause is diagnosed, not tuned away. A test set-up defect gets a
NEW corrected test and the old one stays as a record. Addenda (measured numbers, file names) are appended at the end only.

## Scope, stated up front

F-228 depends on F-127 (Euler-Euler) which is `todo`; there is no multiphase base in the tree (F-055 VOF also `todo`). This wave
therefore delivers, and stops at `partial`:

1. A LIBRARY population-balance solver in `rcfd-fv` (`popbal.rs`): 0-D (well-mixed cell) discrete bins (fixed pivot) and
   moment method (QMOM) with coalescence and breakup kernels.
2. A library-level one-way transport helper (`PopBalTransport`, same file): the bin number densities are carried as passive
   scalars by the existing `ScalarTransport` machinery on a single-phase carrier flux, with the 0-D kernels as cell sources by
   Lie operator splitting. It is ONE-WAY: the population does not feel or change the flow, there is no interphase momentum, mass
   or volume-fraction coupling, no slip.
3. Not delivered (named gaps, also in the roadmap note): case-tree wiring (no new case keys, so no schema change and no edit of
   `schema.rs`, `sim.rs`, `scalars.rs`: the transport helper is exercised through the library API only; a `popbal` case section is
   the first follow-up), interphase coupling (needs F-127), breakup/coalescence driven by resolved turbulence in a k-epsilon run
   (the Saffman-Turner kernel is provided as a function of `epsilon` and `nu`, but it is not connected to a k-epsilon field of a
   run and is only checked as an identity, not against a turbulent case), DQMOM, method of classes, adaptive time stepping, moment
   transport with quadrature-based spatial fluxes (the moment method is 0-D only), variable-density or compressible carriers.

Baseline hashes (`crates/rcfd-api/tests/baseline_hashes.rs`) stay bit-identical and are not re-recorded: the only shared edit is
one `pub mod popbal;` line in `crates/rcfd-fv/src/lib.rs`. Clean-room: published methods and first principles only; every
closed form below is derived in this ADR from the moment equations (so "derived", not a data claim).

## Decisions

### D1 Governing equation and variables

Number density `n(v,t)` per unit carrier volume over particle volume `v`:
`dn/dt = 1/2 int_0^v b(v-u,u) n(v-u) n(u) du - n(v) int b(v,u) n(u) du + int_v^inf nu g(u) p(v|u) n(u) du - g(v) n(v)`
with coalescence kernel `b`, breakup rate `g`, daughter density `p(v|u)` (`int p dv = 1`, `int v p dv = u/nu`; `nu = 2`).
Moments `m_k = int v^k n dv`: `m_0` number, `m_1` volume fraction (conserved by both mechanisms).
Moment equation (used by QMOM and for all closed forms below):
`dm_k/dt = 1/2 sum_ij w_i w_j b_ij ((x_i+x_j)^k - x_i^k - x_j^k) + sum_i w_i g_i (mb_k(x_i) - x_i^k)`
(sums become integrals for the continuum), `mb_k(x) = int v^k nu p(v|x) dv`:
binary equal `2 (x/2)^k`; uniform `2 x^k/(k+1)`; symmetric beta of integer order `q` (density `2 (2q+1)!/(q!)^2 v^q (x-v)^q/x^(2q+1)`,
`q = 0` is uniform) `2 x^k prod_{r=0}^{k-1} (q+1+r)/(2q+2+r)`.

### D2 Discrete bins: fixed pivot (Kumar and Ramkrishna 1996)

Pivots `x_1 < ... < x_M` (any grid; constructors: linear `x_i = i dx`, geometric `x_i = x_1 2^((i-1)/s)`), bin number density `N_i`
(number per carrier volume). A coalescence event of pivots `j,k` creates volume `v = x_j + x_k`. With `x_l <= v < x_{l+1}` the event
number is split `a = (x_{l+1}-v)/(x_{l+1}-x_l)` to bin `l` and `1-a` to bin `l+1`. This preserves event number (1) and volume (`v`).
Pair event rate `r_jk = (1 - delta_jk/2) b_jk N_j N_k` (each unordered pair once):
`dN_j -= r`, `dN_k -= r` (twice for `j = k`: `-= 2r`), `dN_l += a r`, `dN_{l+1} += (1-a) r`. Truncation at the top: `v >= x_M`:
`dN_M += r v/x_M` (volume conserved, number not).
Breakup of bin `k` at rate `g_k`: `dN_k -= g_k N_k`; daughters deposited with the same two-pivot weights: for BinaryEqual two daughters
at `x_k/2`; for continuous daughter densities `n_ik = int hat_i(v) nu p(v|x_k) dv` by 6-point Gauss-Legendre on every
sub-interval between pivots (exact for the polynomial densities `q <= 5`). Truncation at the bottom: daughters with `v < x_1` go to bin 1
with number `v/x_1` (volume conserved, number not; a bin-1 breakup then changes nothing).
Properties stated and checked: volume `sum x_i dN_i/dt = 0` to round-off including both truncations; number rate equals the
continuum number rate evaluated on the discrete distribution (`-1/2 sum b N N + sum (nu-1) g N`) when no truncation is hit
(no mass above `x_M/2` for coalescence; no daughters below `x_1`). On a linear grid `x_i = i dx` every sum `x_j + x_k` is a pivot, so
the scheme IS the exact discrete Smoluchowski system and all moment identities hold exactly. Higher moments on non-uniform grids are
not preserved (the interpolation error per event is `a(1-a)(x_{l+1}-x_l)^2` in `v^2`), so `m_2, m_3` are over-predicted by an amount
that falls with the number of bins per doubling `s`.
Initial distributions from a density `n_0(v)`: `N_i = int hat_i(v) n_0(v) dv` (hat = the same two-pivot weights, below `x_1` weight
`v/x_1` into bin 1), which preserves `m_0` and `m_1` up to truncation. Time integration: classical RK4 with a fixed step (a linear
invariant such as the volume is preserved by every Runge-Kutta method to round-off; positivity is checked, not guaranteed).

### D3 Moment method: QMOM (McGraw 1997) with Wheeler's algorithm

`2N` moments `m_0..m_{2N-1}` determine an `N`-node Gauss quadrature. The moments are scaled by the mean size `m_1/m_0` and `m_0`, the
recurrence coefficients come from Wheeler's modified Chebyshev algorithm (equivalent to the product-difference algorithm of Gordon
1968, but stable to higher `N`), the Jacobi matrix (`alpha_k` on the diagonal, `sqrt(beta_k)` off it) is diagonalised with the
implicit QL algorithm (own implementation): nodes are the eigenvalues, weights `m_0 * (first component of the eigenvector)^2`.
Realizability: `beta_k > 0` for `k = 1..N-1` (Hankel determinants positive). A `beta_k` relative to its scale below `-1e-12` (negative
Hankel determinant), `m_0 <= 0` or a non-finite moment is a located `PopBalError::NotRealizable { index, .. }` naming the failing
moment index; `|beta_k|` below `1e-12` (relative) means the distribution has fewer than `N` support points and the quadrature is
reduced to `k` nodes (a delta distribution gives one node `m_1/m_0`). Moment evolution: `dm_k/dt` by D1 with the quadrature, RK4 in
time, the inversion is done at every stage and a failure aborts with the time and the stage.

### D4 Kernels

Coalescence: constant `K`; sum `B (u+v)`; product `C u v`; Brownian (continuum regime) `(2 k T / 3 mu)(u^(1/3)+v^(1/3))(u^(-1/3)+v^(-1/3))`;
laminar shear (Smoluchowski) `(4/3) gdot (r_u + r_v)^3`, `r = (3v/4 pi)^(1/3)`; turbulent shear (Saffman and Turner) `1.2944 sqrt(eps/nu)
(r_u+r_v)^3`, `1.2944 = sqrt(8 pi/15)`, valid for particles below the Kolmogorov scale. Breakup rate: constant `B`, power `B x^alpha`.
Daughters: BinaryEqual, Uniform, SymmetricBeta(q).

### D5 One-way transport helper

`PopBalTransport { bins: Vec<ScalarTransport>, model, ... }`: each bin is a `ScalarTransport` on the shared geometry (own boundary
conditions, `diffusivity` as given, default 0 allowed). `advance(phi, dt)` is a Lie split: (1) `ScalarTransport::advance` of every bin
with `ImplicitEuler`; (2) per cell, RK4 of the 0-D bin system over `dt` (a stand-alone integration, cell by cell), written back to the
fields and the history level. This is first order in `dt` in the splitting. Cells are independent in step (2), so the per-cell volume
`sum x_i N_i` is unchanged to round-off by the source step.

### D6 Numerical constants

Gauss-Legendre 6-point nodes on `[-1,1]`; tridiagonal QL: iteration cap 60 per eigenvalue, convergence `|e| <= 1e-16 (|d_i|+|d_{i+1}|)`.

## Derived closed forms (before gelation; initial monodisperse of volume 1 and number `N0` unless stated)

Substituting into the moment equation of D1 (`tau = K N0 t / 2`):

* Constant kernel `K`: `dm_0/dt = -K m_0^2/2`, so `m_0 = N0/(1+tau)`; `m_1` constant; `dm_k/dt = (K/2) sum_{r=1}^{k-1} C(k,r) m_r m_{k-r}`.
  For exponential initial density `n_0 = N0 e^(-v)`: `m_k(t) = k! N0 (1+tau)^(k-1)` (checked by substitution for every `k`:
  both sides equal `(K/2) N0^2 k!(k-1)(1+tau)^(k-2)`), i.e. `n(v,t) = N0/(1+tau)^2 exp(-v/(1+tau))` (the Scott/Smoluchowski exponential).
  Monodisperse start on the integer grid (the discrete system): Flory form `N_k = N0 tau^(k-1)/(1+tau)^(k+1)`; consistent with the moments:
  `sum N_k = N0/(1+tau)`, `sum k N_k = N0`; that it solves the discrete equations is checked by the test against the integrated solution.
* Sum kernel `B(u+v)`: `dm_0/dt = -B m_1 m_0`, `dm_2/dt = 2 B m_1 m_2`; `m_0 = N0 e^(-B m_1 t)`, `m_2 = m_2(0) e^(2 B m_1 t)`.
* Product kernel `C u v`: `dm_0/dt = -C m_1^2/2` so `m_0 = m_0(0) - C m_1^2 t/2`; `dm_2/dt = C m_2^2` so `m_2 = m_2(0)/(1 - C m_2(0) t)`,
  gel time `t_g = 1/(C m_2(0))`; `dm_3/dt = 3 C m_2 m_3` so `m_3 = m_3(0) (1 - C m_2(0) t)^(-3)`.
* Constant breakup rate `Gamma`, uniform daughters: `dm_k/dt = Gamma (1-k)/(1+k) m_k`: `m_k = m_k(0) exp(Gamma t (1-k)/(1+k))`
  (`m_0 = m_0(0) e^(Gamma t)`, `m_1` constant, `m_2 ~ e^(-Gamma t/3)`, `m_3 ~ e^(-Gamma t/2)`). Binary equal daughters: `m_k = m_k(0) exp(Gamma t (2^(1-k) - 1))`.
* Linear breakup `g(x) = x`, uniform daughters: `dm_0/dt = m_1`, `m_0 = m_0(0) + m_1 t`.
* Binary equal breakup, constant rate `B`, start at one pivot, generation `g` (pivots at `x_0 2^-g`, needs `s` even): `dn_g/dt = -B n_g + 2 B n_{g-1}`,
  `n_g = N0 e^(-Bt) (2Bt)^g/g!` for `g` below the last generation `G` (bin 1 absorbs the rest by volume conservation); `N = N0 e^(Bt)` up to the tail
  beyond `G`.
* Brownian equal sizes: `b(v,v) = 8 k T/(3 mu)`, hence `dN/dt|_0 = -(4 k T/(3 mu)) N^2`. Laminar shear equal sizes: `b(v,v) = 8 gdot v/pi`.
* Gamma density (shape 2, scale 1) `m_n = (n+1)!`: generalised Gauss-Laguerre (`a = 1`) two-node abscissae are the roots of
  `x^2/2 - 3x + 3`, i.e. `3 +- sqrt(3)`.

## Registered checks (limits fixed here, before any run)

Notation: relative error `|a-b|/|b|`. `s` = bins per doubling. All "exact" references are the derived forms above.

### Bins (B)

* **B1 linear grid Flory.** `M = 400`, `dx = 1`, `K = 1`, `N0 = 1`, monodisperse in bin 1, RK4 `dt = 0.01`, at `tau = 1` and `4`:
  `max_k |N_k - N_k^exact|/N0 <= 1e-7`; `|m_1 - 1| <= 1e-12`; `min_i N_i >= -1e-10 max N` over all stored steps (positivity, B9).
* **B2 geometric grid, number closure.** `s = 4`, `x_1 = 1`, 40 doublings, monodisperse at `x_1`, `K = 1`, `dt = 0.01`: `m_0` relative error vs
  `N0/(1+tau)` at `tau = 1, 4` at most `1e-8`; `|m_1 - 1| <= 1e-12`.
* **B3 exponential initial density, convergence in `s`.** `x_1 = 1e-6`, `x_M` at least `1e4` (34 doublings), `s = 2, 4, 8`, projected
  `n_0 = e^(-v)`, `K = 1`, `dt = 0.005`, at `tau = 2`: `m_0` relative error at most `1e-5`, `m_1` relative error at most `1e-8` (all `s`);
  `m_2` relative error at most `0.08` for `s = 4` and `0.03` for `s = 8`; `m_3` at most `0.10` for `s = 8`; the `m_2` and `m_3` errors decrease
  strictly from `s = 2` to `4` to `8`. Observed orders `log2(e_s/e_2s)` in bin width are REPORTED; expectation recorded: between 1 and 2, no assertion.
* **B4 sum kernel, linear grid.** `M = 600`, `B = 1`, `N0 = 1`, `dt = 0.002`, `t = 1`: `m_0` vs `e^-1` and `m_2` vs `e^2`, relative error at most `1e-6`.
* **B5 product kernel, linear grid.** `M = 600`, `C = 1`, `N0 = 1` (`t_g = 1`), `dt = 0.002`, `t = 0.5`: `m_0` vs `0.75`, `m_2` vs `2`, relative `1e-6`.
* **B6 binary-equal breakup.** `s = 2`, 20 doublings (`G = 20` generations), monodisperse at the top pivot, `B = 1`, `dt = 0.01`, `t = 1`:
  generation numbers `n_g` vs `e^-1 2^g/g!` for `g = 0..15`, absolute error at most `1e-9`; `m_0` vs `e` at most `1e-9` relative; volume at most `1e-12`.
* **B7 uniform daughters.** `s = 4`, 40 doublings, monodisperse at the top pivot (`x = 1`), `Gamma = 1`, `dt = 0.01`, `t = 1`: `m_0` vs `e` at most
  `1e-8`; `m_1` at most `1e-12`; for `s = 8` `m_2` vs `e^(-1/3)` at most `0.05` and `m_3` vs `e^(-1/2)` at most `0.10`; errors fall from `s = 4` to `8`.
* **B8 round-off conservation of the right-hand side.** Deterministic pseudo-random positive `N`, `s = 3`, 30 doublings (coalescence: constant, sum, product,
  Brownian, shear; breakup: constant+uniform, power+beta(q=2), binary): `|sum x_i f_i| / sum |x_i f_i| <= 1e-13`. Number: coalescence only with support in
  the lower half of the grid: `|sum f_i + 1/2 sum_jk b_jk N_j N_k|/(1/2 sum b N N) <= 1e-13`; binary-equal breakup with support above bin `s+1` and
  rate constant: `|sum f_i - Gamma sum N_i| / (Gamma sum N_i) <= 1e-13`.
* **B10 kernel identities.** Brownian `b(v,v) = 8kT/(3mu)` and the RHS-initial number rate for a monodisperse bin `= -(4kT/3mu)N^2` at `1e-12` relative;
  laminar shear `b(v,v) = 8 gdot v/pi` at `1e-14`; Saffman-Turner equals the laminar-shear kernel with `gdot_eff = (1.2944/(4/3)) sqrt(eps/nu)` at `1e-14`;
  all kernels symmetric and positive on a grid sample; daughter distributions: `sum_i n_ik = nu` and `sum_i x_i n_ik = x_k` at `1e-13` for every `k` whose
  daughters stay above `x_1` (uniform and beta `q = 1, 3`).

### QMOM (Q)

* **Q1 recovery of finite distributions.** Moments of the two-point law `(w, x) = ((0.3, 0.7), (1, 3))` and a three-point law `((0.2, 0.5, 0.3), (0.5, 2, 5))`
  inverted with `N = 2, 3`: nodes and weights recovered to `1e-12` relative.
* **Q2 gamma and lognormal.** Gamma(2, 1), `N = 2`: nodes `3 +- sqrt(3)` to `1e-12`, moments `m_0..m_3` reproduced to `1e-12`. Gamma(2,1) and lognormal
  (`mu = 0`, `sigma = 0.5`, `m_n = exp(n^2 sigma^2/2)`), `N = 3`: moments `m_0..m_5` reproduced to `1e-9`, nodes positive and ordered.
* **Q3 degenerate and non-realizable.** Delta distribution (`m = (2, 2c, 2c^2, 2c^3)`): one node `c`, weight 2, `1e-12`. Negative variance `(1, 1, 0.5, 1)`,
  `m_0 = 0`, `m_0 < 0`, NaN moment, and a moment set with `m_3` violating the second Hankel condition: each `Err(NotRealizable)` with the index reported, and the
  error text starts with `popbal:` and contains the index (asserted).
* **Q4 constant kernel.** Exponential initial moments `m_k = k! N0`, `K = 1`, `N0 = 1`, `dt = 0.01`, to `tau = 5`; `N = 2` (`m_0..m_3`) and `N = 3`
  (`m_0..m_5`) vs `k! N0 (1+tau)^(k-1)`: relative error at most `1e-8` for every `k`, `|m_1 - 1| <= 1e-12`. (The moment RHS only uses moments up to
  `2N-1`, so QMOM is exact here up to time integration; the check validates the machinery.)
* **Q5 sum kernel.** `N = 2`, exponential initial, `B = 1`, `t = 1`, `dt = 0.002`: `m_0` and `m_2` vs derived form, relative `1e-7`.
* **Q6 product kernel.** `N = 2`, exponential initial (`m_2(0) = 2`, `C = 1`, `t_g = 1/2`), `dt = 0.001`, at `t = 0.25` (`theta = 0.5`) and `t = 0.4`
  (`theta = 0.8`): `m_0, m_2, m_3` vs derived forms relative `1e-7`.
* **Q7 breakup.** Constant `Gamma = 1`, uniform and binary-equal daughters, `N = 2`, exponential initial, `t = 1`, `dt = 0.005`: `m_0..m_3` vs derived
  relative `1e-8`. Linear breakup `g = x`, uniform: `m_0` vs `m_0(0) + m_1 t` at `t = 2` relative `1e-8` and `|m_1 - m_1(0)|/m_1(0) <= 1e-12`.
* **Q8 bins vs QMOM.** Constant kernel, exponential initial, `tau = 2`: `m_0` of the `s = 8` bin run (B3) and QMOM `N = 2` agree to `1e-5`
  (REPORT the `m_2` difference as the bin discretisation error; no limit).

### One-way transport (T)

Mesh: `channel([nx, 1, 1], [1, 0.1, 0.01])`, uniform flux of speed `U = 1` (`face_flux`), inlet `FixedValue`, outlet and walls `ZeroGradient`,
`diffusivity = 0`, `ConvScheme::LinearUpwind`, `ImplicitEuler`. No `Symmetry` walls.

* **T1 pure advection.** Kernels off, Gaussian pulse `exp(-((x-0.25)/0.08)^2)` in bin 1 (others zero), `nx = 400`, `dt = 0.4/(nx U)`, advect to `t = 0.3`: relative L2
  error against the translated profile at most `0.06`; the bins are independent: bin 2 loaded with `3` times the same pulse equals `3` times bin 1 to `1e-12`.
* **T2 coagulating plug flow.** Constant kernel, `K N0 L/U = 4`, monodisperse inlet in the bin of volume 1 on a geometric grid (`s = 4`, 40 doublings),
  zero initial field, `nx = 200`, `dt = 0.5/(nx U)`, to `t = 5 L/U`: outlet-cell `m_0` vs `N0/(1 + K N0 x/(2U))` at `x = L`: relative error at most `0.01`;
  outlet-cell volume `m_1` over the inlet value within `1e-6`.
* **T3 source step conservation.** One source step on a cell state: `|V_after - V_before|/V_before <= 1e-12`.

### Process

* **P1** `baseline_hashes.rs` passes unchanged. **P2** final gate on a `git archive` copy: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace --release --no-fail-fast`.

## Not delivered / named gaps (registered before the work)

Case-tree wiring (`popbal` case section, output arrays), interphase momentum/mass coupling, volume-fraction-coupled kernels (a physical kernel that depends
on a resolved dispersed-phase fraction), turbulence-driven kernels from a k-epsilon run, DQMOM, method of classes, adaptive pivots, moment transport with
realizable fluxes in space, validation against a published experiment.

## References

| Quantity | Source | Status |
| --- | --- | --- |
| Fixed-pivot discretisation | S. Kumar and D. Ramkrishna (1996), Chem. Eng. Sci. 51, 1311-1332 | citation from memory, UNVERIFIED; paper not read; the scheme is restated and its properties derived/checked here |
| QMOM | R. McGraw (1997), Aerosol Sci. Technol. 27, 255-265 | citation from memory, UNVERIFIED; not read |
| Product-difference algorithm | R. G. Gordon (1968), J. Math. Phys. 9, 655-663 | citation from memory, UNVERIFIED; not read |
| Wheeler's algorithm | J. C. Wheeler (1974), Rocky Mountain J. Math. 4, 287-296; W. Gautschi, Orthogonal Polynomials | citation from memory, UNVERIFIED; algorithm reconstructed from the modified Chebyshev recurrence |
| Smoluchowski kernels | M. von Smoluchowski (1917), Z. Phys. Chem. 92, 129-168 | citation from memory, UNVERIFIED |
| Exponential solution of the constant kernel | P. J. Scott (1968), J. Atmos. Sci. 25, 54-65 | citation from memory, UNVERIFIED; the solution is derived and checked by substitution here |
| Turbulent shear kernel | P. G. Saffman and J. S. Turner (1956), J. Fluid Mech. 1, 16-30 | citation from memory, UNVERIFIED; constant `sqrt(8 pi/15)` from memory, only an identity is tested |
| Breakup daughter distributions | C. A. Hill and K. M. Ng (1995), AIChE J. 41, 1204-1216 | citation from memory, UNVERIFIED; the symmetric-beta form is defined here |
| Implicit QL tridiagonal eigenproblem | Golub and Welsch (1969), Math. Comp. 23, 221-230; standard QL | citation from memory, UNVERIFIED; implementation own |

## Results addendum (written after the runs; no registered limit was changed)

Release builds, tests in `crates/rcfd-fv/tests/popbal_bins.rs` (B), `popbal_qmom.rs` (Q), `popbal_transport.rs` (T); code in
`crates/rcfd-fv/src/popbal.rs`. All registered limits were met; there are no `*_known_miss` tests.

Measured against the registered limit:

| Check | Measured | Limit |
| --- | --- | --- |
| B1 max abs bin error vs Flory, tau 1 / 4 | 1.2e-11 / 1.4e-12 | 1e-7 |
| B1 volume drift; min N / max N over steps (B9) | <= 4.4e-16; 0 | 1e-12; >= -1e-10 |
| B2 m0 relative error tau 1 / 4 | 3.8e-12 / 1.7e-12 | 1e-8 |
| B3 m0 (all s) | 1.7e-7 | 1e-5 |
| B3 m1 (all s) | <= 4.4e-16 | 1e-8 |
| B3 m2 relative error s = 2 / 4 / 8 | 4.8e-2 / 1.1e-2 / 2.9e-3 | s=4 0.08, s=8 0.03 |
| B3 m3 relative error s = 2 / 4 / 8 | 1.6e-1 / 3.7e-2 / 9.1e-3 | s=8 0.10 |
| B3 observed order in bin width (reported) | m2 2.05, 2.01; m3 2.12, 2.03 | expectation recorded 1 to 2: the measured order is 2 (slightly above the recorded band; not asserted) |
| B4 sum kernel m0 / m2 | 1.3e-13 / 4.3e-12 | 1e-6 |
| B5 product kernel m0 / m2 | 5.9e-16 / 3.1e-12 | 1e-6 |
| B6 generation numbers abs / m0 / volume | 6.8e-10 / 8.3e-11 / 2.2e-16 | 1e-9 / 1e-9 / 1e-12 |
| B7 s = 4: m0, m1 | 7.5e-9, 3.3e-16 | 1e-8, 1e-12 |
| B7 s = 8: m2, m3 (s = 4: 3.3e-3, 7.4e-3) | 8.3e-4, 1.9e-3 | 0.05, 0.10 |
| B8 RHS volume / number conservation | 4.8e-16 / 1.2e-15 | 1e-13 |
| B10 Brownian initial number rate | equal to 12 digits (1e-12 limit met) | 1e-12 |
| Q1, Q2 recovery / reproduction | all within limits (see test output) | 1e-12 / 1e-9 |
| Q3 error cases | 5 of 5 located with the registered indices (0, 0, 2, 2, 4) | index in text |
| Q4 N = 2 and 3, worst moment error | 1.4e-12 | 1e-8 |
| Q5 m0 / m2 | 1.3e-13 / 4.3e-12 | 1e-7 |
| Q6 t = 0.25 m0, m2, m3 | 1.6e-14, 3.1e-12, 1.1e-10 | 1e-7 |
| Q6 t = 0.4 m0, m2, m3 | 2.7e-14, 1.4e-10, 4.7e-9 | 1e-7 |
| Q7 uniform / binary, k = 0..3, worst | 1.6e-13 / 1.2e-12 | 1e-8 |
| Q7 linear breakup m0 / m1 | 1.4e-14 / 0 | 1e-8 / 1e-12 |
| Q8 bins (s = 8) vs QMOM m0; m2 difference (reported) | 1.7e-7; 2.9e-3 | m0 1e-5 |
| T1 relative L2 error of the advected pulse; linearity | 3.8e-2; 1.2e-15 | 0.06; 1e-12 |
| T2 outlet m0 relative error; volume ratio | 1.5e-3; 1.0e-15 | 0.01; 1e-6 |
| T3 cell volume change by the source step | 1.9e-16 | 1e-12 |

Deviations and disclosures (none changes a registered limit):

* **B10 set-up defect.** The first version of the daughter-row assertion required a total daughter number strictly greater than 1. For
  bin 1 all uniform daughters fall below the first pivot and the volume-conserving pile-up gives number exactly 1. The first test is kept
  as `b10_kernel_and_daughter_identities_v1_setup_defect` (ignored) and the corrected `..._v2` passes.
* **B10 registered wording.** The registered statement "`sum_i n_ik = nu` for every `k` whose daughters stay above `x_1`" is vacuous for the
  continuous densities (their daughters always reach below `x_1`). What is checked instead: volume `sum x_i n_ik = x_k` to 1e-13 for every
  `k` and every density; the number sum is `2 - x_1/x_k` for the uniform density (closed form, 1e-13) and between 1 and 2 for the betas; the
  binary-equal row has number exactly 2 for `k >= 2`; the moment factors `mb_k/x^k` agree with composite-Simpson integrals of the densities
  to 1e-9. The Saffman-Turner identity is tested with the exact constant `sqrt(8 pi/15)` (1.2944 is its rounded value).
* **Q3 "second Hankel".** Realized as a three-point law with `m_4` multiplied by 0.3 (N = 3); the error carries index 4.
* **B3 order.** Observed order 2, above the recorded expectation "between 1 and 2" (the expectation was a report, not a limit). It is the
  expected value for piecewise-linear splitting of the interpolation error per event (second order in the bin width in `v^2`), a
  reading consistent with the derivation in D2.
* **T2 cost.** The registered T2 set-up (161 bins, 200 cells, 2000 steps) takes about 225 s in a release build (dominated by 322 000
  implicit transport solves), which is long for the suite. The parameters stay as registered.
* **Not done:** no case-tree wiring, no schema change (so `docs/case-schema.md` is unchanged), no turbulence-coupled run.
* The pair loop of the bin right-hand side skips pairs where either number density is exactly zero (an optimisation with identical results).

Status of F-228: `partial`. Delivered: 0-D fixed-pivot bins with coalescence (constant, sum, product, Brownian, laminar shear,
Saffman-Turner) and breakup (constant and power rates; binary-equal, uniform and symmetric-beta daughters); QMOM with Wheeler's algorithm
and located realizability errors; one-way transport of the bins in the scalar machinery (library). Named gaps: case-tree wiring,
interphase coupling (F-127), turbulence-driven kernels from a k-epsilon run, DQMOM, method of classes, spatial moment transport.
