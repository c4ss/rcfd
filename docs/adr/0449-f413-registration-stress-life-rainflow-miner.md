# ADR 0449: F-413 registration: high-cycle fatigue (stress-life curves, mean-stress corrections, the amplitude of a harmonic tensor, rainflow counting, linear damage, narrow-band damage)

Status: **REGISTERED 2026-11-14, written before any code of F-413 and before any run of its checks**. Not yet accepted by the
maintainer. Before this text nothing of F-413 was written, compiled or run: the documents and code under "What was read" were
read, and the arithmetic under "Arithmetic before this registration" was done by three Python scripts (pure Python and numpy, not
the library) in the scratch folder `w49i-work/f413-scratch` (`rainflow.py`, `narrowband.py`, `factors.py`). Results go in the
addendum at the end; nothing above the addendum is changed after a run.

Roadmap item F-413 (deps F-408, F-376, F-399), structural solver, advanced tier (plan
[ADR 0310](0310-structural-solver-advanced-tier-plan.md)), work package SA-6, size M, stub
[ADR 0316](0316-high-cycle-fatigue-stress-life-rainflow-miner.md) (kept unchanged). Branch `f413-work` from main 73f5d432,
worktree `w49i-work/f413`, scratch `w49i-work/f413-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits are fixed here and not changed after a run; a missed limit becomes an ignored
`*_known_miss` test; a defective set-up gets a corrected test by a committed amendment before the corrected run; each registered
run is made once. Everything is opt-in: the only library file written is the stub `crates/rcfd-fem/src/fatigue.rs` (ADR 0440), no
existing function changes, no shared solver file is edited, so every default result keeps its bits by construction. No new
dependency, no unsafe code, the crate graph unchanged.

## What F-413 asks

The acceptance of `docs/roadmap.json`, binding:

> Every limit not stated here is registered in the ADR before the run. Curves and corrections reproduce N = (sigma_a/sigma_f)^(1/b)/2, sigma_a/(1 - sigma_m/sigma_u) (Goodman), sigma_a/(1 - (sigma_m/sigma_u)^2) (Gerber) and Soderberg within 1e-12; a mean stress at or above the strength is refused. Harmonic amplitude: for a normal and a shear component 90 degrees out of phase the von Mises amplitude equals the closed-form maximum over the phase within 1e-10. Cantilever with a static and a harmonic tip force: the largest damage lies on the root surface and the life there equals the hand value from 6 P L/(b h^2) within the registered factor that a 2 percent stress error allows. Plate with a small circular hole (radius/width 0.05) under remote harmonic tension: stress amplitude at the hole edge within 3 percent of 3 sigma (Kirsch) and the life the Basquin life of that stress. Counting: n cycles of constant amplitude give n cycles of that range; a large cycle carrying m small ripples gives 1 and m; a periodic history started at its maximum closes with no residual; a history and its mirror image give the same counts; two load blocks give n1/N1 + n2/N2 within 1e-12. Narrow band: the damage rate nu0 (sqrt(2) s)^m Gamma(1 + m/2)/C within 1e-12 as a function, and the counted damage of a synthesised narrow-band history (registered seed, 1e5 cycles) within 5 percent of it.

Clauses, numbered for the table: a1 curves and corrections within 1e-12; a2 mean at or above the strength refused; a3 harmonic
von Mises amplitude 90 degrees out of phase; a4 cantilever (location and life); a5 plate with a hole (stress amplitude and life);
a6 constant amplitude; a7 large cycle with ripples; a8 periodic history started at its maximum; a9 mirror image; a10 two blocks;
a11 narrow-band rate as a function; a12 synthesised narrow-band history.

Every closed form of the acceptance was verified below and is correct as written. Two readings are fixed here: "Soderberg" is
`sigma_a/(1 - sigma_m/sigma_y)` with the yield strength (the acceptance names it without a formula), and "mirror image" is read
both ways, the history negated (reflection about the time axis) and the history reversed in time; both are checked.

## What was read (main 73f5d432, read only)

ADR 0316 (stub), ADR 0310 (the entry of SA-6, section 7), ADR 0440 (F-408 scaffold: the stub `rcfd_fem::fatigue` with
`PENDING = Some("ADR 0316 (F-413)")`, check X1 of `crates/rcfd-fem/tests/sa_scaffold.rs`), ADR 0448 (F-412: `DirectHarmonic`,
its `stress` with the complex modulus, the style of this registration), ADR 0433 (F-379: the cantilever with the support of beam
theory, D2, and its measured bending stress; the polar map of the Lame cylinder), `crates/rcfd-fem/src/recover.rs` (F-376, ADR
0277: `IntegrationPointField`, `nodal_average` with the per-element extrapolation, `von_mises`, `principal`, Voigt order xx, yy,
zz, xy, yz, zx with tensor shear), `harmonic.rs` (F-399: `HarmonicResponse`, `HarmonicStress { re, im, positions }`, the
convention `x(t) = Re(X e^{i w t})`, `HarmonicLoad::from_model`), `harmonic_direct.rs` (`DirectHarmonic::new`, `response`,
`stress`), `hex20.rs` (`upgrade_to_hex20_mapped`: every node is `map(p)` of its parameter position, sets kept; the 3 x 3 x 3 rule
and the triquadratic fit of the recovery), `builders.rs` (`hex_box` and its node and surface sets `xmin` .. `zmax`), `model.rs`
(`Load::traction` on a surface set), `mesh.rs` (`FaceRef`, sets), `rcfd-core/src/rng.rs` (xoshiro256** seeded through
SplitMix64), `crates/rcfd-fem/tests/sb_closed_forms.rs` (`rebuild`, the beam support), and the notes of F-376, F-399, F-408, F-412
in `docs/roadmap.json`. The standing decision on retired scaffold checks (ADR 0291 pattern, as applied by ADR 0448 D3).

## Closed forms (first principles and cited definitions)

**Basquin** (Basquin 1910, in the strain-life notation of Morrow; standard texts, e.g. Dowling, Mechanical Behavior of Materials,
cited from memory, no number taken from them): `sigma_a = sigma_f (2 N)^b`, `b < 0`, `2 N` the reversals to failure. Solving,
`2 N = (sigma_a/sigma_f)^(1/b)`, `N = (sigma_a/sigma_f)^(1/b)/2`: the acceptance's form, the factor 2 on the reversals included.
As a power law `N = C S^-m`: `m = -1/b`, `C = sigma_f^m/2`.

**Mean-stress corrections** (Goodman 1899, Gerber 1874, Soderberg 1930; definitions as in the standard texts): the fully
reversed amplitude `sigma_ar` equivalent to `(sigma_a, sigma_m)` from the lines (parabola) through `(0, sigma_ar)` and the
strength on the mean axis:

- Goodman `sigma_a/sigma_ar + sigma_m/sigma_u = 1`: `sigma_ar = sigma_a/(1 - sigma_m/sigma_u)`, as written.
- Gerber `sigma_a/sigma_ar + (sigma_m/sigma_u)^2 = 1`: `sigma_ar = sigma_a/(1 - (sigma_m/sigma_u)^2)`, as written.
- Soderberg `sigma_a/sigma_ar + sigma_m/sigma_y = 1`: `sigma_ar = sigma_a/(1 - sigma_m/sigma_y)`.

Each denominator vanishes at the strength and is negative beyond it (Gerber: at `|sigma_m| = sigma_u`), so a mean at or above it
has no equivalent amplitude: refused (a2). A compressive mean: Goodman and Soderberg would credit it (`sigma_ar < sigma_a`),
Gerber would penalise it. Default **no credit and no penalty** (`sigma_ar = sigma_a` for `sigma_m < 0`), the formula on request
(`compressive_credit`); taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation): the conservative
common practice, and the acceptance's formulas are checked on tensile means.

**Safety factor** against a design life `N_d` (extension): the factor `lambda` on the whole stress (mean and amplitude together,
a proportional load factor) at which the life is `N_d`; `S = sigma_f (2 N_d)^b` (or the table's, and not below an endurance
limit). None: `lambda = S/sigma_a`. Goodman `lambda sigma_a/(1 - lambda sigma_m/sigma_u) = S`: `lambda = S/(sigma_a + S
sigma_m/sigma_u)`; Soderberg with `sigma_y`. Gerber: `S (sigma_m/sigma_u)^2 lambda^2 + sigma_a lambda - S = 0`, the positive
root in the cancellation-free form `lambda = 2 S/(sigma_a + sqrt(sigma_a^2 + 4 S^2 sigma_m^2/sigma_u^2))`. With no credit a
compressive mean gives `S/sigma_a`.

**Amplitude of a harmonic tensor** (a3). `sigma(t) = Re((A + i B) e^{i w t}) = A cos th - B sin th`, `th = w t`. The von Mises
stress is `sqrt(q(s))` with the quadratic form `q(s) = ((s_xx - s_yy)^2 + (s_yy - s_zz)^2 + (s_zz - s_xx)^2)/2 + 3 (s_xy^2 +
s_yz^2 + s_zx^2)` and its bilinear form `p(s, r)` (`q(s) = p(s, s)`):
`q(th) = q(A) cos^2 th + q(B) sin^2 th - 2 p(A, B) sin th cos th = (q_A + q_B)/2 + (q_A - q_B)/2 cos 2th - p_AB sin 2th`,
so `max_th q = (q_A + q_B)/2 + sqrt(((q_A - q_B)/2)^2 + p_AB^2)` and the amplitude is its square root (the history is
symmetric about zero, `sigma(th + pi) = -sigma(th)`, so the maximum is the amplitude). For `s_xx = s cos th`, `s_xy = t cos(th +
pi/2)` (`A = (s, 0, 0, 0, 0, 0)`, `B = (0, 0, 0, t, 0, 0)`): `q_A = s^2`, `q_B = 3 t^2`, `p_AB = 0`, amplitude
`max(s, sqrt(3) t)`; in phase (`A = (s, 0, 0, t, 0, 0)`, `B = 0`) `sqrt(s^2 + 3 t^2)`. The mean stress of a von Mises evaluation
is the **signed von Mises** of the static tensor, the sign of its trace (`+` for a zero trace); taken on the standing instruction.

**Largest principal amplitude** (extension): `max_th lambda_1(A cos th - B sin th)`; since `lambda_1(-S) = -lambda_3(S)`, this is
`max over th in [0, pi)` of `max(lambda_1, -lambda_3)`. No closed form in general: 720 phases, then golden-section refinement on
the bracket of the best phase to `1e-12` rad (each `lambda` by `recover::principal`). The mean is `n . S_m n` with `n` the
principal direction of the maximising phase. Closed forms checked: `s_xx = s cos th`, `s_yy = -s2 sin th`: `max(s, s2)`; pure
shear amplitude `t`: `t`; proportional `A + i B = (l + i mu) S`: `|l + i mu| max|eig S|`.

**Rainflow counting** (a6 to a10): defined by the closing of hysteresis loops (Matsuishi and Endo 1968; the four-point form of
Downing and Socie, Int. J. Fatigue 4 (1982) 31, and of ASTM E1049-85 section 5.4.4; all cited from memory, the standard's worked
example not used). D4 below. Properties derived, then confirmed by `rainflow.py` (below): (i) a periodic history rotated to start
at its turning point of largest modulus and closed by repeating it leaves, after the four-point pass, the residue `[M, v, M]`
(proof: with `M` the first point and a global extreme, every inner pair `(x_k, x_k+1)` has `|x_k - x_k+1| <= |M - x_1|`; the
residue's ranges can only decrease after the first, and the last pair before the closing `M` is then removable), counted as one
closed cycle, so no residual; (ii) the result does not depend on the rotation (each loop closes regardless of where the period is
cut); (iii) negating the history maps every cycle `(r, m)` to `(r, -m)`; (iv) reversing a periodic history traverses each closed
loop the other way: the same cycles.

**Miner** (Palmgren 1924, Miner 1945): `D = sum_i n_i/N_i`, `n_i = 1` for a closed cycle and `1/2` for a residual half cycle;
`N_i` the life of the cycle's amplitude `r_i/2` at its mean `m_i` through the correction; an infinite life adds 0. Summed in the
order of the cycles with Neumaier's compensated summation (so that `10^5` equal terms keep the 1e-12 of a10).

**Narrow band** (a11): a stationary Gaussian process with standard deviation `s` and rate of zero up-crossings `nu0`; for a
narrow band its peaks follow the Rayleigh density `p(S) = S/s^2 exp(-S^2/(2 s^2))` (Rice 1944), one per up-crossing, each a cycle
of amplitude `S` (Miles 1954; Bendat 1964; cited from memory). With `N = C S^-m`:
`E[D]/T = nu0 E[S^m]/C`, `E[S^m] = int_0^inf S^m S/s^2 e^{-S^2/(2 s^2)} dS`; with `u = S^2/(2 s^2)`, `S = s sqrt(2 u)`,
`S dS = s^2 du`: `E[S^m] = (sqrt 2 s)^m int_0^inf u^{m/2} e^{-u} du = (sqrt 2 s)^m Gamma(1 + m/2)`. The acceptance's form.
From one-sided spectral moments `m_k = int f^k G(f) df` (Hz): `s = sqrt(m_0)`, `nu0 = sqrt(m_2/m_0)` (Rice).

**Gamma function** (D5): `ln Gamma` by Stirling's series (Abramowitz and Stegun 6.1.40) for `x >= 15` with the Bernoulli numbers
`B_2 .. B_14` (exact rationals: 1/6, -1/30, 1/42, -1/30, 5/66, -691/2730, 7/6), the first omitted term `B_16/(16 15 x^15) <= 3e-20`;
for `x < 15` the recurrence `Gamma(x) = Gamma(x + n)/(x (x + 1) .. (x + n - 1))`. Rounding about `|ln Gamma| eps + n eps`, below
1e-14 relative for the arguments used.

**Cantilever** (a4): Euler-Bernoulli root stress `sigma = M c/I = P L (h/2)/(b h^3/12) = 6 P L/(b h^2)`; static `P_s` gives the
mean, harmonic `P_a` the amplitude. **Hole** (a5): Kirsch (1898; Timoshenko and Goodier, Theory of Elasticity, cited from memory):
in an infinite plate under remote tension `sigma_0` along x the hoop stress at the hole edge `theta = 90 deg` from the load is
`3 sigma_0`, independent of Poisson's ratio in plane stress.

## Arithmetic before this registration (disclosed)

- `rainflow.py` (a pure-Python form of D4): 2000 random periodic histories of 3 to 40 points (Python's `random`, seed 1, half of
  the values rounded to 0.1 to make ties): rotation, reversal and negation gave the same sorted counts in every case, bit for bit
  (0 failures). `[S, -S] x 5` gave 5 cycles of range `2S`; a big cycle `-S .. S` with 4 ripples of depth `0.05 S` gave one cycle
  `2S` and 4 of range `0.05 S` (to rounding, 4e-17); blocks `[2, -2] x 3 + [1, -1] x 4` gave 3 cycles of 4 and 4 of 2 (also with
  offset means); the plain pass on `[S, -S] x 3 + [S]` leaves `[S, -S, S]`.
- `narrowband.py` (the synthesis of B2 with numpy's generator, not the registered stream; seeds 0 to 19): `2^22` samples, centre
  `2^22/32` (32 samples per cycle), flat band `+-2` percent, complex Gaussian coefficients, `irfft`, the periodic count of
  `rainflow.py`, `m = 3`, against `nu0 T (sqrt 2 s)^3 Gamma(2.5)` with the realised `s` (mean of `x^2`) and `nu0 T = sqrt(sum k^2
  |c_k|^2/sum |c_k|^2)`: ratio mean 0.99605, standard deviation 0.00365, range 0.98784 to 1.00656 (about 131 000 cycles each).
  With 64 samples per cycle the mean is 0.99970 (the 0.4 percent is the sampling of the peaks, `(pi/32)^2/2` per amplitude times
  `m`); `m = 4`: mean 0.996, sd 0.010; band `+-1` percent: 0.994, sd 0.004. **Registered: `m = 3`, 32 samples per cycle, `+-2`
  percent**, estimate `0.996 +- 0.004`, margin about 10 standard deviations to the 5 percent.
- `factors.py`: the life factors below, `omega_1` of the cantilever (Euler-Bernoulli `1.875104^2 sqrt(E I/(rho A L^4)) = 1024.64`
  rad/s), the plate's first longitudinal frequency (about `2 pi c_0/(4 A) = 7.9e4` rad/s), the radial grading of the plate mesh
  (first element `0.0305 r`, last `1.41 r`, angular step at the hole `0.0654 r`), and the finite-width factor of the hole from
  the polynomial of Peterson's handbook for Howland's results (`K_tn = 3 - 3.14 (d/W) + 3.667 (d/W)^2 - 1.527 (d/W)^3`, cited
  from memory, used only for the estimate, not for a limit): `d/W = 0.1`, `K_tn = 2.721`, gross `3.023`, 0.8 percent above 3.

## Design

### D1 Curves (`SnCurve`)

`SnCurve::basquin(sigma_f, b)` (`sigma_f > 0`, `b < 0`, finite), `SnCurve::power(m, c)` (`N = c S^-m`, `m > 0`, `c > 0`),
`SnCurve::table(lives, amplitudes)` (at least two points, lives strictly ascending and positive, amplitudes strictly descending
and positive; `log N` linear in `log S` between points; outside, the end segment extended), each with
`.with_endurance(sigma_e)` (`sigma_e > 0`: an amplitude at or below it has an infinite life). `life(sigma_ar) -> Result<f64>`
(`f64::INFINITY` for an infinite life; a negative or non-finite amplitude refused; zero gives infinity), `strength_at(n)` (the
inverse, not below the endurance limit), `power_law() -> Option<(m, c)>` (Basquin and power curves without an endurance limit).
Refusals carry the key `"stress-life curve"`.

### D2 Mean stress (`MeanStress`) and equivalent stress (`Equivalent`)

`MeanStress::None`, `Goodman { ultimate }`, `Gerber { ultimate }`, `Soderberg { yield_strength }`, with `compressive_credit`
(default false) on a `MeanStressCorrection { kind, compressive_credit }`; `equivalent_amplitude(sigma_a, sigma_m)` as above;
refused with key `"mean stress"` and both values when the mean is at or above the strength (Gerber with credit: `|sigma_m| >=
sigma_u`). `safety_factor(curve, correction, sigma_a, sigma_m, design_life)`.

`Equivalent::VonMises` (amplitude of D-closed form, mean signed von Mises) and `Equivalent::MaxPrincipal` (D-principal above).
Free functions `harmonic_von_mises_amplitude(re, im)`, `harmonic_max_principal(re, im) -> (amplitude, direction)`,
`signed_von_mises(s)`, `equivalent_history(&[[f64; 6]], Equivalent) -> Vec<f64>` (proportional histories: signed von Mises, or the
principal value of largest modulus with its sign).

### D3 Fields

`FatigueSpec { curve, mean, equivalent, design_life: Option<f64> }`. `evaluate_harmonic(spec, mean: &[f64; 6], re, im) ->
Result<PointFatigue>` with `PointFatigue { amplitude, mean, equivalent_amplitude, life, damage_per_cycle (1/life, 0 when
infinite), safety_factor: Option<f64> }`. `FatigueField::harmonic(spec, mean: Option<&[[f64; 6]]>, re, im, frequency_hz)`: one
`PointFatigue` per point (integration points of `HarmonicStress`, or nodal values the caller extracts), `damage_rate(p) =
frequency_hz damage_per_cycle`, `largest_damage() -> Option<usize>` (first index of the largest damage per cycle). Lengths must
agree (`FemError::FieldLength`), the frequency finite and `>= 0`; a point refused by the correction is refused naming the point
index (key `"mean stress"`). Sequential, fixed order.

### D4 Counting

`turning_points(x)` (non-finite refused, key `"history"`; plateaus merged; interior points that are not reversals dropped; first
and last kept). `rainflow(x) -> Result<Counts>` with `Counts { cycles: Vec<Cycle>, residue: Vec<f64> }`, `Cycle { range, mean,
count }`: the four-point pass over the turning points, a stack, for the last four `a b c d` the pair `(b, c)` closes a cycle when
`|b - c| <= |a - b|` and `|b - c| <= |c - d|` (removed, range `|b - c|`, mean `(b + c)/2`, count 1); the residue is reported;
`Counts::with_half_cycles()` adds each residue range as a half cycle (count 1/2). `rainflow_periodic(x)`: drops a closing point
equal to the first, turning points of the cyclic sequence, rotation to the first turning point of largest modulus, closing by
repeating it, the pass, the residue `[M, v, M]` counted as one cycle; residue empty. `miner(cycles, curve, correction) ->
Result<f64>` (Neumaier summation).

### D5 Narrow band

`ln_gamma(x)`, `gamma(x)` (`x > 0`) as above; `narrow_band_damage_rate(nu0, rms, m, c)` `= nu0 (sqrt 2 rms)^m Gamma(1 + m/2)/c`
(`nu0 >= 0`, `rms >= 0`, `m > 0`, `c > 0`, finite; key `"narrow band"`); `narrow_band_damage_rate_curve(nu0, rms, curve)` through
`power_law` (a table or an endurance limit refused); `spectral_rate(m0, m2) -> (rms, nu0)`.

### D6 Scaffold check X1 (ADR 0440)

The stub lands, so `fatigue::PENDING` becomes `None` and `x1_modules_and_pending_after_landed` of `sa_scaffold.rs` (which expects
`Some("ADR 0316 (F-413)")`) no longer holds. By the standing decision (ADR 0291 pattern, as applied by ADR 0448 D3) it becomes an
ignored record citing ADR 0291 and ADR 0449, unedited otherwise, and `x1_modules_and_pending_after_fatigue` repeats it with the
`fatigue` row expecting `None` ("landed"). The edit to that file is kept to this. Taken on the maintainer's standing instruction
of 2026-11-11 (take the recommendation).

## Models of a4 and a5

**Cantilever (F checks).** `hex_box([0; 3], [0.5, 0.05, 0.05], [20, 2, 4])` upgraded by `upgrade_to_hex20` (`L = 0.5`, `b = h =
0.05`, `L/h = 10`), steel `E = 2e11`, `nu = 0` (the 3-D solution is then the plane flexure solution: no anticlastic or Poisson
effects at the root), `rho = 7850`. Support of beam theory (ADR 0433 D2): `u_x = 0` on `xmin`, `u_z = 0` at every `xmin` node with
`z = h/2` (the root neutral line), `u_y = 0` at the one of them with `y = 0`; this is the half of a beam `2L` loaded at both ends,
no clamped corners (a fully clamped free corner is singular). Static: `Load::traction("xmax", [0, 0, -P_s/(b h)])`, `P_s =
4166.667` N (`sigma_m = 100` MPa at the root); harmonic: a second model with `P_a = 6250` N (`sigma_a = 150` MPa),
`DirectHarmonic` undamped at `w = 0.01 omega_1 = 10.2464` rad/s (dynamic amplification `1 + O(1e-4)`, not corrected). Stresses:
`IntegrationPointField::from_result` (static), `DirectHarmonic::stress` (harmonic), each averaged to the nodes by `nodal_average(...,
None)` (linear, so `re` and `im` separately). Curve: Basquin `sigma_f = 900` MPa, `b = -0.09`; Goodman `sigma_u = 600` MPa, no
compressive credit; `Equivalent::VonMises`. These are illustrative steel-like values, not data of a material (decision A1 of ADR
0310). Hand: `sigma_ar = 150/(1 - 100/600) = 180` MPa, `N_hand = (180/900)^(-1/0.09)/2 = 2.919466e7`. **Registered factor of a
2 percent stress error:** the whole stress (mean and amplitude, one linear model) scaled by `1.02` and `0.98`:
`N/N_hand in [0.767541, 1.308433]`.

Estimate of the root nodal stress: Hex20 represents `u_x = z w'(x)` (the monomial `x^2 z` is in the serendipity set) and the
triquadratic fit of the 27 points is exact for a stress linear in `x` and `z`; the error comes from the cubic `u_z`, the support
reaction on the neutral line (Filon-type local correction at the opposite surface, order `0.04/(L/h) = 0.4` percent), and the
dynamic factor (1e-4). Expected within 1 percent of `6 P L/(b h^2)`; the life within the factor.

**Plate with a hole (K checks).** Quarter plate with symmetry: `r = 0.005`, full width `W = 0.1` (`r/W = 0.05`), half width `B =
0.05`, half length `A = 0.1`, half thickness `t = 0.001` with `u_z = 0` on `zmin`, `nu = 0` (plane, exact). Parameter mesh
`hex_box([0; 3], [1, 1, t], [24, 24, 1])` upgraded by `upgrade_to_hex20_mapped` with the map `(s, q, z) -> C(q) + g(s) (O(q) -
C(q))`, `C = r (cos th, sin th)`, `th = pi q/2`, `O(q)` the point at arc length `q (A + B)` along the outer path `(A, 0) -> (A,
B) -> (0, B)` (the corner at `q = 1/3`, a node line since 24 is a multiple of 3), `g(s) = (e^{4 s} - 1)/(e^4 - 1)`. Sets of the
parameter box: `ymin` is the line `y = 0` (`u_y = 0`), `ymax` the line `x = 0` (`u_x = 0`); the loaded surface `right` = the
faces of the surface set `xmax` whose nodes all have `x = A` within 1e-12 m, `Load::traction("right", [sigma_0, 0, 0])`,
`sigma_0 = 50` MPa. `DirectHarmonic` undamped at `w = 100` rad/s (`(w/w_1)^2 = 1.6e-6`). Basquin as above, `MeanStress::None`,
`Equivalent::VonMises`, no static stress. 576 Hex20, 4275 nodes. Kirsch `3 sigma_0 = 150` MPa, `N(150 MPa) =
(150/900)^(-1/0.09)/2`; factor of a 3 percent stress error `N/N(3 sigma_0) in [0.720053, 1.402751]`.

Estimate: finite width `+0.8` percent (above), the loaded edge at `20 r` (`(r/x)^2 = 2.5e-3` locally, much less at the hole), the
mesh: first radial element `0.03 r` against the gradient `-7 sigma_0/r` and curvature `33 sigma_0/r^2` of Kirsch's hoop stress
along `x = 0` (fitted quadratically within the element: below 0.3 percent). Expected `1.003` to `1.013` of `3 sigma_0`.

## Checks

Tests: `crates/rcfd-fem/tests/sa_fatigue.rs`, release profile. Relative difference `|x/y - 1|`. Random data from
`rcfd_core::rng::Rng::new(0xF413_0449)`.

| Check | Clause | Quantity | Limit | Estimate |
|---|---|---|---|---|
| C1 | a1 | Basquin `life` against `(sigma_a/sigma_f)^(1/b)/2` in the test, `sigma_f = 900e6`, `b` in {-0.05, -0.09, -0.12}, `sigma_a` in {50, 150, 400, 900, 1200} MPa; and `strength_at(life(s)) = s` | `<= 1e-12` | 1e-15 |
| C2 | a1 | Goodman, Gerber (`sigma_u = 600e6`), Soderberg (`sigma_y = 400e6`) equivalent amplitudes against the three formulas, `sigma_a` in {50, 150, 300} MPa, `sigma_m` in {0, 50, 150, 300, 399} MPa (Soderberg below 400); and the corrected life equals Basquin of the formula | `<= 1e-12` | 1e-16 |
| C2c | a1 | `sigma_m = -200` MPa: default `sigma_ar = sigma_a` (bit-equal) for the three; with credit Goodman `sigma_a/(1 + 1/3)`, Soderberg `sigma_a/(1 + 1/2)`, Gerber `sigma_a/(1 - 1/9)` | `== `; `<= 1e-12` | as stated |
| C3 | a2 | refused with key `"mean stress"`: Goodman at `sigma_m = sigma_u` and `1.1 sigma_u`, Gerber at `sigma_u`, Gerber with credit at `-sigma_u`, Soderberg at `sigma_y`; accepted at `0.999` of each | as stated | as stated |
| C4 | a1 | table of Basquin at `N` = 1e3, 1e5, 1e7 (`b = -0.09`): life at the table amplitudes equals `N_i`, and at 20 amplitudes from 30 to 1500 MPa (inside and outside) equals Basquin | `<= 1e-12` | 1e-14 |
| C5 | a1 | endurance limit 100 MPa: `life(100e6) = inf`, `life(99e6) = inf`, `life(101e6)` finite and equal to Basquin; `strength_at(1e12) = 100e6` | as stated | as stated |
| C6 | a1 | power curve `m = -1/b`, `c = sigma_f^m/2` against Basquin at the C1 amplitudes; `power_law` of Basquin equals that `(m, c)` | `<= 1e-12` | 1e-15 |
| H1 | a3 | `harmonic_von_mises_amplitude` with `s_xx = s`, `s_xy = i t` against `max(s, sqrt 3 t)`, `(s, t)` in {(100, 20), (100, 100/sqrt 3), (100, 80), (0, 50), (70, 0)} MPa; in phase against `sqrt(s^2 + 3 t^2)` | `<= 1e-10` | 1e-16 |
| H2 | a3 extension | 200 random pairs `(A, B)` (components uniform in `[-1, 1)` x 100 MPa): sampled maximum over 200 000 phases `<= amp (1 + 1e-12)` and `amp <= sampled (1 + 1e-8)` | as stated | 2e-10 |
| H3 | extension | `harmonic_max_principal`: `s_xx = s cos`, `s_yy = -s2 sin` (`A = (s, 0, ..)`, `B = (0, s2, 0, ..)`) gives `max(s, s2)` for `(s, s2)` = (100, 60), (60, 100); pure shear amplitude 70: 70; 50 random proportional `(l + i mu) S`: `|l + i mu| max |eig S|` | `<= 1e-9` | 1e-12 |
| H4 | extension | `signed_von_mises`: `+-sigma` uniaxial gives `+-sigma` (`<= 1e-15`), pure shear positive, hydrostatic 0; `MaxPrincipal` mean of a uniaxial mean `80` MPa with a uniaxial harmonic along x equals 80 MPa (`<= 1e-12`); `equivalent_history` of a uniaxial history equals it (`<= 1e-15`) | as stated | as stated |
| S1 | extension | safety factor with `N_d = 1e7`, `(sigma_a, sigma_m)` = (150, 100) and (150, -100) MPa, each correction: the life at `lambda (sigma_a, sigma_m)` equals `N_d` (`<= 1e-10`); Goodman `lambda` against `S/(sigma_a + S sigma_m/sigma_u)` (`<= 1e-12`) | as stated | 1e-14 |
| F1 | a4 | node of largest damage over all nodes (nodal-averaged stresses): `x = 0` and `z = h` | within 1e-12 m | as stated |
| F2 | a4 | life at that node over `N_hand` | in `[0.767541, 1.308433]` | 1.0 +- 0.12 |
| F3 | a4 diagnostic | von Mises amplitude and signed von Mises mean at that node against `6 P_a L/(b h^2)`, `6 P_s L/(b h^2)` | each `<= 0.02` | below 0.01 |
| F4 | a4 extension | at the integration points (`FatigueField` on `DirectHarmonic::stress` and the static field directly): the point of largest damage has `x < L/20` and `z > 3 h/4` | as stated | as stated |
| F5 | extension | at the F1 node the field's `life`, `damage_rate` equal `evaluate_harmonic` on the same tensors and `frequency_hz / life` | bit-equal; `<= 1e-15` | 0 |
| K1 | a5 | `|s_xx|` amplitude (nodal average, largest of the nodes at `(0, r)`) over `3 sigma_0` | within 3 percent | 1.003 to 1.013 |
| K2 | a5 | node of largest damage at `(0, r)` | within 1e-9 m in x and y | as stated |
| K3 | a5 | life at that node equals `SnCurve::life` of its von Mises amplitude (`<= 1e-12`), that amplitude within 3 percent of `3 sigma_0`, and life over `N(3 sigma_0)` in `[0.720053, 1.402751]` | as stated | as stated |
| Q1 | a6 | periodic `[S, -S] x n`, `n` = 1, 7, 1000, `S = 120e6`: `n` cycles, every range `== 2S`, mean `== 0`; periodic sampled sine `S sin(2 pi k/16)`, `k = 0 .. 16 n - 1`, `n = 50`: 50 cycles of range within 1e-12 of `2S` | as stated | exact |
| Q2 | a7 | periodic `-S`, then `m` ripples `(l_i, l_i - d)` with `l_i = -S + 2 S i/(m + 1)`, `d = 0.05 S`, then `S`; `m` = 1, 4, 25: one cycle of range `2S` and `m` of `|range - d| <= 1e-12 S` | as stated | 4e-17 S |
| Q3 | a8 | the Q1 and Q2 histories and 20 random histories (12 to 60 points) rotated to start at their maximum: `rainflow_periodic` has an empty residue and no half cycles; the plain `rainflow` of the history closed by its first point leaves the residue `[M, min, M]`; the periodic counts of every rotation are equal (sorted, bit-equal) | as stated | as stated |
| Q4 | a9 | for the Q1 to Q3 histories and 200 random periodic histories: negated history gives the same ranges and negated means; reversed history the same counts (sorted multisets, bit-equal) | as stated | exact (`rainflow.py`) |
| Q5 | a10 | periodic `[S1, -S1] x n1 + [S2, -S2] x n2`, `S1 = 300e6`, `n1 = 1000`, `S2 = 150e6`, `n2 = 100000`: counts `n1` of `2 S1` and `n2` of `2 S2`; `miner` (Basquin, no correction) against `n1/N(S1) + n2/N(S2)` | `<= 1e-12` | 1e-16 |
| Q6 | extension | plain `rainflow([-2, 1, -1, 2] x 100 MPa)`: one cycle (range 200, mean 0) and residue `[-200, 200]`; `miner(with_half_cycles)` against `1/N(100) + 0.5/N(200)` | `<= 1e-12` | 1e-16 |
| Q7 | extension | `turning_points([0, 1, 1, 2, 1, 1, 0])` is `[0, 2, 0]`; `[0, 1, 2, 3]` is `[0, 3]` | exact | exact |
| B1 | a11 | `narrow_band_damage_rate(12.5, 40e6, m, 1e30)` against `nu0 (sqrt 2 s)^m Gamma/C` with `Gamma(1 + m/2)` exact (factorials, `Gamma(n + 1/2) = (2n)! sqrt(pi)/(4^n n!)`), `m` in {3, 4, 5, 6, 9, 10}; `gamma(x + 1)/(x gamma(x))` for `x` in {0.3, 1.7, 3.37, 7.3, 12.9, 20.5} | `<= 1e-12`; `<= 1e-13` | 1e-15 |
| B2 | a12 | synthesised history: `2^22` samples, `k` in `[128450, 133693]` (`+-2` percent of `131072`), coefficients `c_k = g1 + i g2` (Box-Muller from `Rng::new(0xF413_0449)`, two uniforms per pair, in increasing `k`), `x = Re sum c_k e^{2 pi i k n/N}` by a radix-2 FFT in the test; `rainflow_periodic`, `miner` with `SnCurve::power(3, 1e30)`, against `nu0 T (sqrt 2 s)^3 Gamma(2.5)/C` with `s^2` the mean of `x^2` and `nu0 T = sqrt(sum k^2 |c_k|^2/sum |c_k|^2)`; cycles counted `>= 1e5` | within 5 percent | 0.996 +- 0.004 |
| R1..R8 | refusals | each refused with its key: R1 Basquin `sigma_f <= 0`, `b >= 0`, NaN; table of one point, not ascending, not descending (`"stress-life curve"`); R2 negative amplitude (`"stress-life curve"`); R3 a NaN in a history (`"history"`); R4 field lengths differ (`FieldLength`); R5 narrow band `rms < 0`, `nu0 < 0`, table curve, endurance limit (`"narrow band"`); R6 a field point at the strength refused naming its index (`"mean stress"`, `point 1`); R7 negative frequency of a field (`"frequency"`); R8 design life `<= 0` (`"design life"`) | as stated | as stated |
| X1 | D6 | `x1_modules_and_pending_after_fatigue` passes; the previous X1 is an ignored record | as stated | as stated |

F and K run once each in release (each below a minute by the sizes: 3.2e3 and 1.3e4 unknowns); B2 about 2 s.

### Planned mutations (scratch copy, `w49i-work/f413-scratch/mut`)

| Id | Where | Change | Registered catchers |
|---|---|---|---|
| M1 | Basquin life | the factor 2 on reversals dropped | C1, C4, C6, F2, K3 |
| M2 | Gerber | exponent 1 | C2 |
| M3 | mean-stress refusal | `>` instead of `>=` | C3 |
| M4 | harmonic von Mises | `sqrt(q_A + q_B)` (in-phase assumption) | H1, H2 |
| M5 | rainflow closing | `<` instead of `<=` | Q1, Q3 |
| M6 | periodic count | no rotation | Q3, Q4 |
| M7 | Miner | half cycles counted as 1 | Q6 |
| M8 | narrow band | `Gamma(1 + m)` | B1, B2 |
| M9 | Stirling series | sign of the `B_4` term | B1 |
| M10 | signed von Mises | sign of minus the trace | F1, F2 |
| M11 | Goodman safety factor | `- S sigma_m/sigma_u` | S1 |
| M12 | max principal | `lambda_1` only (no `-lambda_3`) | H3 |

## Decision rule

**done** if every registered check (C, H, S1, F, K, Q, B, R, X1) is met in its single registered run and every planned mutation
is caught by at least one registered check; **partial** if a registered check missed, a mutation was not caught, or a check was
met only after an amendment made after its run; **todo** if the module cannot be built.

## Limits (what is not covered)

- No stress-life curve of any polymer is in the repository (decision A1 of ADR 0310): every life is as good as its input; the
  values used are illustrative.
- Non-proportional multiaxial loading (critical plane), strain-life, notch sensitivity, surface and size factors: not included.
  The equivalent stresses assume proportional loading for histories; for a harmonic tensor the amplitude is the maximum over the
  phase of the chosen invariant, which is exact for that invariant but is not a critical-plane method.
- The narrow-band rate is the Rayleigh (narrow-band) bound; wide-band corrections (Wirsching-Light, Dirlik) are not included.
- The root stress of a real fillet needs a converged mesh (ADR 0335); here a square-edged beam with the support of beam theory.
- No wiring into the case file, CLI or API (a follow-up).

## Addendum 1: results (2026-11-14)

The registered run: commit ce1c9f50 (the library of 1776b351 and the tests), `cargo test -p rcfd-fem --release --test sa_fatigue
-- --test-threads=1`, made once, log `f413-scratch/run_fatigue.log` (9 tests, 1.85 s in total: the cantilever 0.18 s, the plate
0.80 s for 12 825 unknowns). X1: `cargo test -p rcfd-fem --release --test sa_scaffold` (9 passed, 5 ignored, among them the two
X1 records). No amendment was written; no check was re-run. The only later commit to code is 2b796700, rustfmt layout of the two
new files (no token changed).

| Check | Limit | Measured | Met |
|---|---|---|---|
| C1 Basquin life; `strength_at(life(s))` | `<= 1e-12` | 0; 2.2e-16 | yes |
| C2 Goodman, Gerber, Soderberg; corrected life | `<= 1e-12` | 0, 0, 0; 0 | yes |
| C2c no credit (bit-equal); with credit Goodman, Soderberg, Gerber | `==`; `<= 1e-12` | bit-equal; 0, 0, 0 | yes |
| C3 five refusals (key `"mean stress"`); 0.999 of each accepted | as stated | all refused, all accepted | yes |
| C4 table at its points; table = Basquin 30 to 1500 MPa | `<= 1e-12` | 6.7e-16; 7.1e-14 | yes |
| C5 endurance limit | as stated | `life(100), life(99) = inf`, `life(101 MPa) = 1.7933e10` = Basquin, `strength_at(1e12) = 1e8` | yes |
| C6 power law = Basquin; `power_law` | `<= 1e-12`; equal | 6.7e-16; `(11.1111, 1.5508e99)` | yes |
| H1 90 degrees out of phase; in phase | `<= 1e-10` | 0; 0 | yes |
| H2 sampled above closed form; closed form above sampled | `<= 1e-12`; `<= 1e-8` | 0; 1.07e-10 (the sampling step) | yes |
| H3 max principal closed forms | `<= 1e-9` | 6.7e-16 | yes |
| H4 signed von Mises; shear, hydrostatic; MaxPrincipal mean; history | as stated | 0; as stated; 0; 0 | yes |
| S1 life at `lambda` times the stress; Goodman `lambda` | `<= 1e-10`; `<= 1e-12` | 2.0e-15; 0 | yes |
| F1 node of largest damage on the root surface | `x = 0`, `z = h` | node 915 at `(0, 0.0125, 0.05)` | yes |
| F2 life over hand life | `[0.767541, 1.308433]` | 0.952655 (2.781244e7 against 2.919466e7) | yes |
| F3 amplitude; mean against `6 P L/(b h^2)` | `<= 0.02` each | 3.66e-3; 3.55e-3 | yes |
| F4 integration point of largest damage | `x < L/20`, `z > 3h/4` | `(0.00282, 0.0125, 0.04859)` | yes |
| F5 field = point evaluation; damage rate | bit-equal; `<= 1e-15` | bit-equal; 1.1e-16 | yes |
| K1 `s_xx` amplitude at `(0, r)` over `3 sigma_0` | within 3 percent | +1.234e-2 | yes |
| K2 node of largest damage | at `(0, r)` | node 1225 at `(0, 0.005, 0.001)` | yes |
| K3 life = Basquin of its amplitude; amplitude; life ratio | `<= 1e-12`; 3 percent; `[0.720053, 1.402751]` | 0; +1.263e-2; 0.869850 | yes |
| Q1 `[S, -S] x n`; sampled sine | as stated | 1, 7, 1000 cycles of `2S`; 50 cycles, error 0 | yes |
| Q2 ripples `m` = 1, 4, 25 | 1 and `m` | 1 and 1, 1 and 4, 1 and 25 | yes |
| Q3 started at the maximum (27 histories) | as stated | no residual, plain residue `[M, min, M]`, rotation invariant | yes |
| Q4 negated; reversed (227 histories) | bit-equal | equal; equal | yes |
| Q5 counts; Miner against `n1/N1 + n2/N2` | as stated; `<= 1e-12` | 1000 and 100 000; 0 | yes |
| Q6 cycle and residue; Miner with half cycles | as stated; `<= 1e-12` | as stated; 0 | yes |
| Q7 turning points | exact | exact | yes |
| B1 narrow-band rate; Gamma recurrence | `<= 1e-12`; `<= 1e-13` | 2.0e-15; 8.9e-16 | yes |
| B2 cycles; counted against the rate | `>= 1e5`; within 5 percent | 131 154; -8.24e-3 (`nu0 T = 131104.4`, `s = 73.592`) | yes |
| R1 to R8 (18 refusals) | keys as stated | each refused with its key and text | yes |
| X1 `x1_modules_and_pending_after_fatigue` | passes | passes; the old X1 ignored | yes |

Remarks on the measured numbers. K1 (+1.23 percent) is at the upper end of the estimate (0.3 to 1.3 percent). The finite width
alone accounts for about +0.8 percent (Howland's factor, from memory), so the check measures the finite plate rather than Kirsch's
infinite one. F3 (+0.37 percent) matches the support effect estimated at 0.4 percent. B2 (-0.82 percent) lies 1.1 standard
deviations from the numpy estimate (`-0.4 +- 0.37` percent; the 0.4 percent bias comes from sampling the peaks at 32 samples per
cycle).

### Mutations (scratch copy `f413-scratch/mut`, the whole `sa_fatigue` suite per mutation, logs `f413-scratch/mut_M*.log`)

| Id | Change | Caught by |
|---|---|---|
| M1 | Basquin without the factor 2 | C1, C2, C6, F2, K3, Q5, Q6, S1 |
| M2 | Gerber exponent 1 | C2, C2c, S1 |
| M3 | refusal at `>` | C3, R6 |
| M4 | `sqrt(q_A + q_B)` | H1, H2 |
| M5 | closing with `<` | Q3 (not Q1, see below) |
| M6 | no rotation | Q1, Q3, Q4 |
| M7 | half cycles counted 1 | Q6 |
| M8 | `Gamma(1 + m)` | B1, B2 |
| M9 | sign of the `B_4` term | B1 |
| M10 | signed von Mises by minus the trace | F1, F4, H4, R6 (not F2: the life at the wrong node was within the factor) |
| M11 | Goodman safety factor `- S sigma_m/sigma_u` | S1 |
| M12 | `lambda_1` only | H3 |

Every mutation was caught by a registered check. Two registered catchers did not catch their mutation (Q1 for M5, F2 for M10);
another registered catcher caught each. M5 escaped Q1 because `rainflow_periodic` counts every second window of three of a residue
longer than `[M, v, M]` as a cycle: with the mutation a constant-amplitude residue `[S, -S, S, ..]` still gives `n` cycles. The
proof of closed forms (i) says such a residue cannot occur with the registered rule. The count is then right only by accident, so
a residue longer than 3 should be refused as an internal error (open question 2).

### Deviations and what was not run

- None of the registered set-ups was changed. The planned catcher lists of M5 and M10 were wider than what caught them (above).
- The message of the Gerber refusal with credit at `-sigma_u` says "at or above the strength" for a negative mean. The text is
  checked by C3, so it was left unchanged (open question 3).
- Not run: the whole workspace suite (the lead's gate). `baseline_hashes` was not run because no shared solver file and no
  rcfd-fv or rcfd-api source was touched. `cargo fmt -p rcfd-fem --check` is clean and `cargo clippy -p rcfd-fem --all-targets --
  -D warnings` is clean. `cargo test -p rcfd-cli --release --test docs_site` passed (2 tests).

### Decision by the registered rule

**done**: every registered check was met in its single registered run, without an amendment, and every planned mutation was caught
by at least one registered check.

### Remaining questions

1. Wiring into the case file, the CLI and the API (fatigue fields from a harmonic case). Options: (a) a follow-up item that adds a
   `fatigue` block to the solid case; (b) leave it as a library until the fan blade case needs it. Recommendation (a), after the
   F-412 transformation-path follow-up, so that cyclic blade models can use it.
2. A residue longer than `[M, v, M]` in `rainflow_periodic`. Options: (a) return an internal error; (b) keep the current pairing.
   Recommendation (a), with a mutation test like M5. This is a one-line follow-up, because the registered rule cannot produce
   such a residue.
3. The message of the Gerber refusal for a compressive mean. Options: (a) say "at or beyond the strength in modulus"; (b) leave
   it. Recommendation (a), with a new check, in the follow-up of question 2.
4. Wide-band damage (Dirlik, Wirsching-Light) and a critical-plane amplitude for non-proportional loading. Recommendation: a
   follow-up item if the random-vibration work (F-417) needs it. The narrow-band rate is the conservative bound.

## For the lead: roadmap entries

Proposed status of F-413: **done**.

> Structural solver, advanced tier (plan ADR 0310, 2026-11-12). Size M. ADR 0449, 2026-11-14: rcfd_fem::fatigue (stub of ADR 0440 filled, PENDING None) with Basquin, power and log-log table curves with an endurance limit, Goodman, Gerber and Soderberg corrections (a mean at or above the strength refused, no compressive credit by default), the closed-form von Mises amplitude of a harmonic tensor maximised over the phase, the largest principal amplitude, the signed von Mises mean, fields of life, damage rate and safety factor, four-point rainflow with a periodic form, Miner with compensated summation, and the narrow-band rate with a Stirling Gamma. Every registered check was met in one run: curves and corrections within 0 to 7e-14 of the closed forms; harmonic amplitude 90 degrees out of phase exact; Hex20 cantilever (L/h = 10, support of beam theory): largest damage at the root top surface, life 0.9527 of the hand value from 6 P L/(b h^2) (registered factor [0.7675, 1.3084] for 2 percent), stresses 0.37 percent off; quarter plate with a hole r/W = 0.05 (576 Hex20, harmonic through DirectHarmonic): hole-edge amplitude +1.23 percent of 3 sigma, the largest damage there and its life the Basquin life of that stress; counting checks exact (constant amplitude, ripples, no residual, negation and reversal invariant, two blocks within 0); narrow band within 2e-15 as a function and -0.82 percent for a synthesised history of 131 154 cycles; 12 of 12 mutations caught.

Follow-up items:

- **Fatigue in the solid case file and CLI** (deps F-413, F-386).

> Every limit not stated here is registered in the ADR before the run. A solid case with a harmonic step and a fatigue block (curve, mean-stress correction, equivalent stress, design life) writes the fields of life, damage rate and safety factor at the nodes; on the plate with a hole of ADR 0449 the written fields equal the library's within 1e-12, a case without a fatigue block gives bit-identical outputs, and a mean at or above the strength is refused with the point and the case path.

- **Rainflow residue guard and refusal messages** (deps F-413).

> Every limit not stated here is registered in the ADR before the run. rainflow_periodic returns an internal error when the residue of the closed sequence is longer than three points, and a mutation that closes loops with a strict inequality is then caught by the constant-amplitude check; the Gerber refusal of a compressive mean at or beyond the strength says so in modulus; all checks of ADR 0449 still pass unedited.
