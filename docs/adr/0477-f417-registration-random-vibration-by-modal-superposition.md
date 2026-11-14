# ADR 0477: F-417 registration: random vibration by modal superposition (force and base-acceleration spectra, spectral moments, root-mean-square displacement and stress, root-mean-square von Mises stress)

Status: **REGISTERED 2026-11-14, written before any code of F-417 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-417 was written, compiled or run in the repository: the files under "What was read"
were read, the arithmetic under "Arithmetic before this registration" was done by plain-Python scripts in the scratch folder
`w49i-work/f417-scratch` (`proto.py`, `diag.py`, `march.py`, `march_run.py`, `refs.py`, `refs2.py`; pure Python, no numpy, no
rcfd code), and one probe of the existing F-399 solver (`f417-scratch/probe`, a scratch binary calling only the API of the base
57808e24) measured the linearity of `HarmonicSolver::response` in the load (arithmetic 6). The bit record B1 of the default paths
is a test file written and run on the base after this commit and before any library code (D9). Results go in the addenda at the
end; nothing above the addenda is changed after a run.

Roadmap item F-417 (deps F-408, F-399), work package SA-10 of the advanced tier (plan
[ADR 0310](0310-structural-solver-advanced-tier-plan.md), stub [ADR 0320](0320-random-vibration-by-modal-superposition.md),
kept unchanged), size S. Number 0477 given by the lead. Branch `f417-work` from main 57808e24, worktree `w49i-work/f417`, scratch
`w49i-work/f417-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no
unsafe code, the crate graph unchanged; existing tests not edited, except that the X1 check of `sa_scaffold.rs` whose `random` row
this work retires is kept as an ignored record citing ADR 0291 and continued in an `_after_f417` copy; clean-room.

## What F-417 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. One mode under a flat one-sided force spectrum G: mean-square displacement (pi/2) f_n Q G/k^2 with Q = 1/(2 zeta), and under a flat base acceleration spectrum the mean-square absolute acceleration (pi/2) f_n Q G (for Q well above 1; the exact integral is computed in the registration), both within 0.5 percent at zeta = 0.02 on the registered grid, observed convergence under grid refinement; zero-crossing rate f_n within 1 percent. Two well separated modes: the mean square is the sum within 1 percent. A band-limited spectrum equals the numerical integral registered beforehand within 1e-6. For a uniaxial stress state the root-mean-square von Mises stress equals the root-mean-square stress within 1e-10. Two fully correlated patterns equal one summed pattern within 1e-12; uncorrelated ones add in mean square. A spectrum with a negative value or a non-increasing frequency list is refused with the entry.

Clauses, numbered for the table: a1 one mode, force, mean-square displacement within 0.5 percent; a2 one mode, base, mean-square
absolute acceleration within 0.5 percent (the exact integral computed here); a3 observed convergence under grid refinement; a4
zero-crossing rate within 1 percent; a5 two well separated modes, the sum within 1 percent; a6 band-limited spectrum against a
registered numerical integral within 1e-6; a7 uniaxial stress, root-mean-square von Mises equal to the root-mean-square stress
within 1e-10; a8 fully correlated patterns equal one summed pattern within 1e-12; a9 uncorrelated patterns add in mean square;
a10 a negative value refused with the entry; a11 a non-increasing frequency list refused with the entry.

The brief adds: (b1) reuse the mode-superposition path of F-399 (ADR 0391) and the existing stress recovery, and the spectral
helpers of `fatigue.rs` (F-413, ADR 0449) without duplicating them; (b2) models that do not use the new module keep their bits,
`baseline_hashes` unchanged, a default-bits test recorded at the base; (b3) a case-file key is the maintainer's call (recommended,
not added); (b4) the `random` row of the X1 scaffold check retired by the records-and-copies scheme; (b5) the F-417 entries of
`docs/ROADMAP.md` and `docs/roadmap.json` updated on the branch in a separate last commit (the brief overrides rule 9 of
AGENT_RULES for this item, as for ADR 0473).

Readings fixed here. "Flat one-sided force spectrum G" is a one-sided spectral density in Hz (`int_0^inf G df` is the mean
square), flat over a registered finite band `[0, 10 f_n]` (a spectrum is a finite table, zero outside it). "One mode" is a
one-degree-of-freedom system `m x'' + c x' + k x = F` (D2 takes it as a modal channel with one mode), and is also checked through
the solid-model path with a one-mode basis (M1, M2). "Mean-square absolute acceleration (pi/2) f_n Q G" is that of a unit
response per unit base acceleration (it does not depend on the mass). "The numerical integral registered beforehand" of a6 is the
mean-square response of the one-mode system under a band-limited table, integrated here by an independent fine quadrature
(arithmetic 4); "within 1e-6" is relative. "The root-mean-square stress" of a7 is that of the single non-zero component. "Two fully
correlated patterns" have the correlation coefficient 1 and the same spectrum; "add in mean square" is checked at the 1e-12 of a8.
"Refused with the entry" means the error names the zero-based index of the offending entry and its value.

## What was read (main 57808e24, read only)

ADR 0320 (stub), ADR 0310 (the SA-10 entry, the module table, the public names of section 2 with `random::response`), ADR 0391
(F-399 registration), ADR 0449 (F-413: `fatigue::spectral_rate`, `narrow_band_damage_rate`, the style), ADR 0473 (the style of a
registration and its bit record), `crates/rcfd-fem/src/random.rs` (stub of ADR 0440, `PENDING = Some("ADR 0320 (F-417)")`),
`harmonic.rs` (F-399 and F-491: `ModalBasis`, `ModalDamping::coefficients` giving `c_r = 2 zeta_r omega_r`, `HarmonicSolver::new`,
its private `prepare` (modal forces `phi_r^T F` and the static residual) and `transfer` (`H_r = 1/(lambda_r - omega^2 + i c_r
omega)`), `HarmonicResponse::stress` (`sigma(x) - sigma(0)` by `IntegrationPointField::compute`), `participation`), `fatigue.rs`
(the narrow-band section: `spectral_rate(m0, m2)` returns `(sqrt m0, sqrt(m2/m0))` and refuses `m0 <= 0`), `recover.rs`
(`IntegrationPointField`, `von_mises`, Voigt order xx, yy, zz, xy, yz, zx with tensor shear), `mass.rs` (`consistent_with`,
`rigid_motions`), `constraint.rs` (`prescribed_equations`), `mpc.rs` (`uses_transformation`), `model.rs` (`DofMap`),
`builders.rs` (`hex_box` with the node set `all` and the face sets `xmin` .. `zmax`), the tests `si_harmonic.rs` (the `Checks`
helper) and `sa_cms_default_bits.rs` (the FNV-1a digest record), `sa_scaffold.rs` (X1 and its copies to
`x1_modules_and_pending_after_f415_f420`), and the F-399, F-408, F-413 entries of `docs/roadmap.json`.

## Closed forms (first principles; textbooks cited from memory)

Sources, cited from memory for the published form only, nothing copied: S. H. Crandall and W. D. Mark, Random Vibration in
Mechanical Systems (Academic Press 1963); D. E. Newland, An Introduction to Random Vibrations, Spectral and Wavelet Analysis (3rd
ed., Longman 1993); P. H. Wirsching, T. L. Paez and K. Ortiz, Random Vibrations: Theory and Practice (Wiley 1995); J. W. Miles,
J. Aeronaut. Sci. 21 (1954) 753; S. O. Rice, Bell Syst. Tech. J. 23 (1944) 282 and 24 (1945) 46; for the von Mises form D. J.
Segalman, C. W. G. Fulcher, G. M. Reese and R. V. Field, J. Vib. Acoust. 122 (2000) 42. Every formula used is derived below.

**Response spectrum.** For a stationary input with one-sided spectral density `G(f)` (Hz) and a linear system with complex
frequency response `Y(f)` (output per unit input, `omega = 2 pi f`), the output's one-sided spectral density is `|Y(f)|^2 G(f)`
and its mean square `m_0 = int |Y|^2 G df` (Wiener-Khinchin; Newland ch. 7). Several inputs `a` with cross-spectral densities
`G_ab(f)` (real symmetric here, D4): `S_ij(f) = sum_ab G_ab Re(Y_ia conj(Y_jb))` between outputs `i`, `j`; covariance `C_ij = int
S_ij df`. Spectral moments `m_k = int f^k S df`; zero up-crossing rate `nu_0 = sqrt(m_2/m_0)` and peak rate `sqrt(m_4/m_2)`
(Rice), both in Hz.

**One mode, force.** `x'' + c x' + k x = F/m`: `Y = 1/(k (1 - r^2 + 2 i zeta r))`, `r = f/f_n`. With a flat `G` on `[0, R f_n]`:
`m_0 = (G f_n/k^2) I_0(R)`, `I_k(R) = int_0^R r^k/D(r) dr`, `D = (1 - r^2)^2 + (2 zeta r)^2`. `I_0(inf) = pi/(4 zeta)` (residues:
`D = (r^2 + 2 s r + 1)(r^2 - 2 s r + 1)`, `s = sqrt(1 - zeta^2)`, the two poles in the upper half-plane give `int_-inf^inf dr/D =
pi/(2 zeta)`), hence `m_0(inf) = (pi/4) f_n G/(zeta k^2) = (pi/2) f_n Q G/k^2`: the acceptance's form, exact for the infinite
band. The substitution `r = 1/s` maps `r^2 dr/D(r)` onto `ds/D(s)`, so `I_2(inf) = I_0(inf)` and `nu_0 = f_n` exactly for the
infinite band; velocity and acceleration multiply by `(2 pi f)^2` and `(2 pi f)^4`.

**One mode, base acceleration.** Relative coordinate `y = x - x_b`: `y'' + c y' + k y = -a_b` (unit mass); absolute acceleration
`x'' = a_b + y''`, transmissibility `T = (1 + 2 i zeta r)/(1 - r^2 + 2 i zeta r)`, `|T|^2 = (1 + 4 zeta^2 r^2)/D`. Mean square
`G f_n (I_0 + 4 zeta^2 I_2)`; infinite band `(pi/(4 zeta))(1 + 4 zeta^2) G f_n = (pi/2) f_n G (Q + 1/Q)`: the acceptance's form
times `1 + 1/Q^2` (`+0.16` percent at `zeta = 0.02`; "for Q well above 1"). The relative displacement is `-1/(k...)`, the force
case with `F/m = -a_b`.

**Two modes.** `Y = u_1 H_1 + u_2 H_2`; `m_0 = m_0,1 + m_0,2 + 2 u_1 u_2 int Re(H_1 conj H_2) G df`. For flat `G` and equal ratios
the cross term relative to `m_0,1 + m_0,2` is `2 rho sqrt(m_1 m_2)/(m_1 + m_2)` with the CQC coefficient `rho = 8 zeta^2 (1 + b)
b^(3/2)/((1 - b^2)^2 + 4 zeta^2 b (1 + b)^2)`, `b = f_1/f_2` (derived by the same residues; Wirsching et al., from memory):
`1.04e-3` at `b = 1/3`, `zeta = 0.02`, equal shares (arithmetic 3). "Well separated" is `f_2 = 3 f_1`.

**Root-mean-square von Mises stress.** `q(s) = sigma_vm^2 = s_xx^2 + s_yy^2 + s_zz^2 - s_xx s_yy - s_yy s_zz - s_zz s_xx + 3
(s_xy^2 + s_yz^2 + s_zx^2)` (Voigt, tensor shear) is a quadratic form `s^T A s`; for a zero-mean random stress with covariance
`C`, `E[q] = tr(A C) = C_11 + C_22 + C_33 - C_12 - C_23 - C_31 + 3 (C_44 + C_55 + C_66)` and the root-mean-square von Mises
stress is `sqrt(E[q])` (Segalman et al.'s definition, from memory; first principles). Uniaxial (`C_11` only): `sqrt(C_11)`, the
root-mean-square stress, exactly (a7). Its rate is `sqrt(tr(A C_2)/tr(A C_0))` with `C_2` the covariance weighted by `f^2` (an
extension; for a uniaxial state the rate of the component).

**Correlated patterns.** With `G_ab = rho_ab sqrt(G_a G_b)`: `rho = 1`, `G_a = G_b = G` gives `G (|Y_1|^2 + |Y_2|^2 + 2 Re(Y_1
conj Y_2)) = G |Y_1 + Y_2|^2`, the response of the summed pattern by linearity (a8); `rho = 0` gives `G_1 |Y_1|^2 + G_2 |Y_2|^2`,
the sum of the separate mean squares (a9); `rho = 1`, `G_b = 4 G` gives `G |Y_1 + 2 Y_2|^2` (supplementary S8b).

## Arithmetic before this registration (disclosed)

1. `proto.py`, `diag.py`: a first grid (key points at `f_n + s zeta f_n`, `s` a power of two, four steps per octave, 3- and
   4-point Gauss-Legendre) gave `-8.7e-5` on `I_0(10)` at the coarsest level, dominated by intervals at `|s| = 4 .. 8` where
   `1/D` changes by a factor four: rejected for D3.
2. `march.py`, `march_run.py`: the grid of D3 (steps `ell(f)/p`, 3-point Gauss-Legendre). Errors of `I_0(10)`, `I_2(10)` and
   `I_0 + 4 zeta^2 I_2` against the fine references, `zeta = 0.02`: `p = 4` (`R = 1`, 141 nodes) `-1.14e-7`, `-1.13e-7`,
   `-1.14e-7`; `p = 8` (`R = 2`) `-1.40e-9`; `p = 16` (`R = 4`) `-2.05e-11`; `p = 32` (`R = 8`) `-3.2e-13`. Observed orders 6.4,
   6.1, 6.0. `nu_0` against `f_n sqrt(I_2/I_0)`: `7.6e-10` at `R = 1`. **Registered: 3-point Gauss-Legendre, `p = 4 R`, `R = 1`
   by default.**
3. `refs.py`, two modes `f_1 = 50`, `f_2 = 150` Hz, `zeta = 0.02`, unit modal masses, gains `u_1 = 1`, `u_2 = 3^1.5` (equal
   shares), flat `G = 1` on `[0, 1500]`: exact `m_0 = 4.035601798182644e-07`; closed sum `C_1 + C_2 = 4.031441804149936e-07`
   (`u_r^2 (pi/2) f_r Q/(2 pi f_r)^4`); ratio `1 + 1.032e-3`; against the two single-mode integrals `1 + 1.036e-3`; CQC cross share
   `1.038e-3`. The grid at `R = 1` (240 nodes): `-1.06e-7` from the exact.
4. `refs.py`, band-limited, one mode `f_n = 50`, `zeta = 0.02`, `Y = 1/(1 - r^2 + 2 i zeta r)` (unit `k`): linear table
   `(30, 0), (40, 1), (55, 1), (75, 0.5)` (Hz, `G` in units^2/Hz): reference `int G |Y|^2 df = 1886.2401793570568`; log-log table
   `(20, 1), (45, 4), (60, 4), (100, 0.5)`: `7584.40095699183`. Both by 4-point Gauss-Legendre on 20000 sub-intervals of each of
   up to 22 pieces split at the table points and at `f_n + s zeta f_n`, `|s| <= 64` (the same with fewer sub-intervals agrees to
   the last digit or two). Grid of D3 at `R = 1`: `-1.10e-7`, `-1.08e-7`; `R = 2`: `-1.4e-9`, `-1.5e-9`.
5. `refs.py`, `refs2.py`: `I_0(10) = 39.26957079652609` (`pi/(4 zeta) = 39.26990816987242`, ratio `1 - 8.59e-6`, the tail
   `1/(3 R^3) = 3.3e-4`), `I_1(10) = 38.77258066850795`, `I_2(10) = 39.16923599186199`, `I_4(10) = 49.006230409610886`,
   `I_0 + 4 zeta^2 I_2 = 39.33224157411308` (against `pi/(4 zeta)`: `1 + 1.587e-3`), `sqrt(I_2/I_0) = 0.9987216695502159`. The
   spectrum mean squares: linear table `35.0` exactly, log-log table `180.92544802681783` (segment `G_i f_i ((f_i+1/f_i)^(alpha+1)
   - 1)/(alpha + 1)`); the grid without modes integrates the linear table to `2e-16` and the log-log table to `-1.25e-6`. The
   base-motion formula of M2 for a gain `a` (below) equals the fine integral of `|1 + a r^2/(1 - r^2 + 2 i zeta r)|^2` on `[0, 10]`
   for `a = 4/pi` to the last digit (`64.39620774718054`); the grid at `R = 1`: `-1.1e-7`.
6. Probe (`f417-scratch/probe`, API of 57808e24 only): Hex8 cantilever `1 x 0.1 x 0.08` m, 16 x 2 x 2, `E = 2.1e11`, `nu = 0.3`,
   `rho = 7850`, clamped on `xmin`, 8 modes (75.9 .. 1470 Hz), `zeta = 0.02`; unit y-forces at the tip nodes `(1, 0.1, 0.08)` and
   `(1, 0, 0)`; over 400 frequencies `5 .. 2000` Hz, `max |Y_12 - Y_1 - Y_2|/max |Y_12|`: `1.5e-15` without the static
   correction, `1.9e-13` with it (the separate solves `K^-1 F` round differently). Hence a8 is registered without the static
   correction (estimate `1e-15`) and the static-correction case as a supplementary check at `1e-11` (S8s).

## Design

### D1. The spectrum (`random::Spectrum`)

A one-sided spectral density table `(f_i, G_i)`, `i = 0 .. n-1`, frequencies in Hz, with linear interpolation
(`Spectrum::new`) or log-log interpolation (`Spectrum::log_log`, `G = G_i (f/f_i)^alpha_i` on each segment), zero outside
`[f_0, f_n-1]`. Refused (key `"spectrum"`), each message naming the zero-based entry and its value: fewer than two entries;
lengths differ; a frequency not finite or negative (log-log: not positive); a frequency not above the previous one ("entry `i`:
frequency `f_i` is not above entry `i-1` (`f_i-1`)"; a10/a11); a value not finite or negative (log-log: not positive). `flat(f_lo,
f_hi, g)`; `value(f)`; `mean_square()` exact per segment (trapezoid; log-log the power-law integral, `alpha = -1` by the
logarithm); `frequencies()`, `values()`; `scaled(factor)` (factor finite `>= 0`).

### D2. Modal channels (`random::ModalChannels`) and the response integral

A model-independent description of linear outputs: modes `r` with `lambda_r` and the damping term `c_r` (as
`ModalDamping::coefficients`), and for each input pattern `a` and output `o` the modal gains `u_oar` and a frequency-independent
residual `s_oa`:

`Y_oa(omega) = sum_r u_oar H_r(omega) + s_oa`, `H_r = 1/(lambda_r - omega^2 + i c_r omega)`;

the quantity multiplies by `(i omega)^k` (`Quantity::Displacement`, `Velocity`, `Acceleration`, `k = 0, 1, 2`), and an optional
absolute term `b_o` (base excitation, one pattern) is added to the acceleration: `A_o = b_o - omega^2 Y_o`. With a spectrum
`G_a` per pattern and the correlation `rho` (D4), `random::spectral_response(channels, spectra, correlation, quantity, group,
grid)` integrates on the grid of D3: per output the moments `m_0, m_1, m_2, m_4` and the spectral density at the nodes; for
consecutive groups of `group` outputs the covariances `C_0`, `C_2` (`group` x `group`). Every loop in a fixed order, one thread
(the result does not depend on the machine). Refused (key `"random"`): inconsistent sizes, a non-finite gain, a group size not
dividing the outputs, an absolute term with more than one pattern.

### D3. The frequency grid and the quadrature

Breakpoints: the union of the frequencies of all spectra, `[f_lo, f_hi]` their span. Mode `r`: `f_r = sqrt(max(lambda_r, 0))/(2
pi)`, half-power half-width `b_r = c_r/(4 pi)` (`= zeta_r f_r`). Local scale `ell(f) = min(max(f, f_ref), min_r sqrt((f - f_r)^2 +
b_r^2))`, `f_ref` the smallest positive value among the breakpoints and the `f_r`. Between consecutive breakpoints, from the left:
`f_k+1 = f_k + ell(f_k)/p`, clipped to the next breakpoint, `p = 4 R` (refinement `R >= 1`, default 1: **the registered grid**);
3-point Gauss-Legendre on each step (nodes `0`, `+-sqrt(3/5)`, weights `8/9`, `5/9`). The scale is the distance to the nearest
peak (where `1/D` varies on that scale) and the frequency itself away from them (power laws vary on a log scale): steps shrink
geometrically towards a peak and stop at `b_r/p`. Refused: a mode with `c_r = 0` and `f_lo <= f_r <= f_hi` (undamped in the
band: the mean square is infinite), naming the mode; more than 4 000 000 nodes. The nodes never fall on a breakpoint, so the
jump of a band-limited table is integrated exactly piecewise.

### D4. Several patterns: the correlation

`G_ab(f) = rho_ab sqrt(G_a(f) G_b(f))` (`G_aa = G_a`), `rho` real, constant over frequency, symmetric with unit diagonal and
positive semi-definite: `rho + 1e-12 I` must have a Cholesky factor with positive pivots (refused otherwise, key
`"correlation"`, naming the pivot). `None` is uncorrelated (`rho = I`). Quad-spectra (a frequency-dependent phase between
patterns) and frequency-dependent coherence are not included (Limits).

### D5. Stress: covariance and von Mises

Per integration point the 6 x 6 covariances `C_0` and `C_2` (D2 with `group = 6`). `RandomStress::rms(p, c) = sqrt(C_0[c][c])`;
`von_mises_mean_square(C)` the form `tr(A C)` above; `rms_von_mises(p) = sqrt(tr(A C_0))`; `von_mises_rate(p) = sqrt(tr(A
C_2)/tr(A C_0))` (`None` at zero). Moments of a scalar: `Moments { m0, m1, m2, m4 }` with `rms()`, `zero_crossing_rate()` by
`fatigue::spectral_rate` (reused; refuses `m0 <= 0`), `peak_rate()`. The narrow-band damage of a random stress stays in
`fatigue::narrow_band_damage_rate` (fed by `rms` and `zero_crossing_rate`), not duplicated.

### D6. The solid-model path (`random::RandomSolver`)

`RandomSolver::new(model, basis, options)` builds the F-399 `HarmonicSolver` (reused: the basis check, the modal damping, the
static correction and its refusals) and keeps the model's equation count. `response(model, excitation, request)`:

- `Excitation::force(pattern, spectrum)`, `Excitation::forces(patterns, spectra, correlation)`: each pattern a load vector over
  the model's equations.
- `Excitation::base(direction, spectrum)`: every prescribed equation moves with the base, a rigid translation along the unit
  `direction`; the relative motion is the response to the load `-M r` (`r` the translation from `mass::rigid_motions`, `M` from
  `mass::consistent_with`, zero at prescribed equations), and the absolute acceleration adds `r` (D2's `b_o`). Displacement and
  velocity outputs are relative, acceleration absolute; stress from the relative displacement. Refused (key `"base
  excitation"`): no prescribed equation, a non-zero prescribed value, the transformation path (elastic supports, linear
  constraints), a zero direction.
- `RandomRequest`: output equations (any of the model's), the quantity, whether the stress is wanted, the refinement.

The gains `u_oar = phi_r[o] (phi_r^T F_a)` and residuals `s_oa = x_s,a[o]` come from a new public method of the F-399 solver,
`HarmonicSolver::modal_decomposition(load)` (D7). Stress gains: `sigma(phi_r) - sigma(0)` and `sigma(x_s,a) - sigma(0)` by
`IntegrationPointField::compute`, as `HarmonicResponse::stress` (the stress is affine in the displacement). Result:
`RandomResponse { frequencies, weights, equations, psd, moments, stress: Option<RandomStress> }`.

### D7. The one edit of a shared file

`harmonic.rs` gains one public method, `HarmonicSolver::modal_decomposition(&self, load: &[f64]) -> Result<(Vec<f64>,
Option<Vec<f64>>)>`: the modal forces `phi_r^T f` and the static residual of a real load, from the existing private `prepare`
(no line of an existing function changes). Every existing result keeps its bits by construction; B1 records them.

### D8. Records retired

`random::PENDING` becomes `None`: `x1_modules_and_pending_after_f415_f420` (`sa_scaffold.rs`) becomes an ignored record citing
ADR 0291 and continues as `x1_modules_and_pending_after_f417` (the `random` row "landed"). Nothing else refers to the stub.

### D9. The bit record of the default (B1)

`crates/rcfd-fem/tests/sa_random_default_bits.rs` uses only the API of the base 57808e24 and is run once on the base (this ADR
committed, no library line changed) to print its digests (FNV-1a 64 of the `to_bits`, as ADR 0469 D1), which become its constants;
after the change it runs unchanged and compares. Paths: the F-399 `HarmonicSolver` of the probe's cantilever with the static
correction (response at three frequencies, a sweep, the modal amplitudes, the response stress), the same without the correction,
a free body with inertia relief (ADR 0400), `participation`, and `fatigue::spectral_rate` with `narrow_band_damage_rate`.

### D10. Names in the topic documentation

`docs/reference-data.md` gains the F-417 section (sources, cited from memory). No case-file key is added (Q1).

## Checks (limits fixed now)

Common: `zeta = 0.02`, the registered grid `R = 1` unless stated. Every value is printed. One-mode system (S1 to S4, Q3): `f_n =
50` Hz, `m = 2` kg, `k = m (2 pi f_n)^2`, `G = 1e-2` N^2/Hz flat on `[0, 500]` (`R_band = 10`), as `ModalChannels` with one mode
(`lambda = k/m`, `c = 2 zeta omega_n`, gain `1/m` (force) or `-1` with absolute term `1` (base, `G` in (m/s^2)^2/Hz)). Bar B: Hex8
`1 x 0.1 x 0.1` m, 40 x 1 x 1, `E = 2e11`, `nu = 0`, `rho = 7850`, `u_y = u_z = 0` on `all`, `u_x = 0` on `xmin` (an axial rod);
tip pattern `0.25` N along x at each of the four `xmax` nodes; one-mode basis = the first mode of `modal_analysis` (tolerance
`1e-12`, backward residual, 1 thread); flat `G = 1` on `[0, 10 f_1]`. Cantilever C: the probe's model (arithmetic 6), 8 modes,
patterns `P_1`, `P_2` the unit y-forces at `(1, 0.1, 0.08)` and `(1, 0, 0)`, flat `G = 1` on `[5, 2000]` Hz, outputs the six
equations of those two nodes and the stress. Metrics on C: displacement `max_o |a_o - b_o|/max_o b_o`; stress `max_p max_ij
|a_ij - b_ij|/max_k b_kk(p)` (on `C_0`).

| Id | Clause | Quantity | Limit | Estimate |
|---|---|---|---|---|
| S1 | a1 | one mode, force: `|m_0/((pi/2) f_n Q G/k^2) - 1|` | `<= 5e-3` | `8.7e-6` (truncation `8.6e-6`) |
| S1x | supp. | the same against `G f_n I_0(10)/k^2`, `I_0(10) = 39.26957079652609` | `<= 1e-6` | `1.1e-7` |
| S2 | a2 | one mode, base: `|m_0(abs. acc.)/((pi/2) f_n Q G) - 1|` | `<= 5e-3` | `1.59e-3` (the exact integral is `1 + 1/Q^2` above) |
| S2x | a2 | against the exact finite-band integral `G f_n 39.33224157411308` | `<= 1e-6` | `1.1e-7` |
| S3 | a3 | errors `e_R` of S1x and of S2x at `R = 1, 2, 4`: `e_1 > e_2 > e_4` and the observed orders `log2(e_R/e_2R) >= 4` (R = 1, 2) | each | orders `6.4`, `6.1` |
| S4 | a4 | `|nu_0/f_n - 1|` of the displacement | `<= 1e-2` | `1.28e-3` |
| S4x | supp. | `|nu_0/(f_n 0.9987216695502159) - 1|` | `<= 1e-6` | `8e-10` |
| S5 | a5 | two modes (arithmetic 3) as `ModalChannels`: `|m_0/(C_1 + C_2) - 1|` | `<= 1e-2` | `1.03e-3` |
| S5x | supp. | against the exact `4.035601798182644e-07` | `<= 1e-6` | `1.1e-7` |
| S6 | a6 | band-limited linear table (arithmetic 4), one mode `f_n = 50`, gain `lambda` (unit `k`): `|m_0/1886.2401793570568 - 1|` | `<= 1e-6` | `1.1e-7` |
| S6b | supp. | the log-log table: `|m_0/7584.40095699183 - 1|` | `<= 1e-6` | `1.1e-7` |
| S7 | a7 | bar B with every mode (`ModalBasis::all_modes`, 160 unknowns), static correction on, under the tip pattern and under base excitation along x: `max_p |rms_vm(p)/rms_xx(p) - 1|` | `<= 1e-10` each | rounding (`sigma_yy = sigma_zz = 0` exactly at `nu = 0`, shear from rounding only) |
| S7x | supp. | `von_mises_mean_square`: pure shear `C_44 = s^2` gives `3 s^2`; equal biaxial fully correlated (`C_11 = C_22 = C_12 = s^2`) gives `s^2`; uniaxial on yy and on zz `s^2`; `C = v v^T` for 5 vectors `v` (fixed) gives `von_mises(v)^2`; relative | `<= 1e-12` each | rounding |
| S8 | a8 | cantilever C, static correction off: `(P_1, P_2)`, `G_1 = G_2 = G`, `rho = [[1, 1], [1, 1]]` against `P_1 + P_2` under `G`, displacement `m_0` and stress `C_0` metrics | `<= 1e-12` each | `1e-15` (arithmetic 6) |
| S8s | supp. | S8 with the static correction on | `<= 1e-11` each | `2e-13` |
| S8b | supp. | S8 with `G_2 = 4 G` against `P_1 + 2 P_2` under `G` | `<= 1e-12` each | `1e-15` |
| S9 | a9 | cantilever C, static correction on: `rho = I` (`None`) against `m_0(P_1) + m_0(P_2)` and `C_0(P_1) + C_0(P_2)` | `<= 1e-12` each | rounding (the same terms) |
| R1 | a10 | `Spectrum::new([10, 20, 30], [1, -1, 1])` refused, key `"spectrum"`, message naming `entry 1` and `-1` | holds | holds |
| R2 | a11 | `[10, 20, 20]` and `[10, 30, 20]` refused naming `entry 2` | holds | holds |
| R3 | supp. | refused: one entry, lengths 3 and 2, a NaN frequency, a negative frequency, a NaN value, log-log with a zero value and with frequency 0; a correlation not symmetric, with diagonal 0.9, and `[[1, 1.5], [1.5, 1]]`; a pattern count different from the spectra; an undamped mode (`c = 0`) inside the band (naming mode 0); base excitation of a model without prescribed equations and of one with an elastic support | each | holds |
| M1 | supp. | bar B, one mode, static correction off, tip pattern: tip `m_0` against `G f_1 (phi_L p)^2 I_0(10)/lambda_1^2` (`p = phi_1^T F`); and at every point `rms_xx` against `|sigma_xx(phi_1)| |p| sqrt(G f_1 I_0(10))/lambda_1`; relative | `<= 1e-5` each | `1.1e-7` |
| M2 | supp. | bar B, one mode, base along x: tip absolute acceleration `m_0/(G f_1)` against `10 (1 - a)^2 + a (2 - a) I_0 + (4 zeta^2 a (2 - a) - 2 a (1 - a)) I_2`, `a = phi_L Gamma_1` (`participation`), relative | `<= 1e-5` | `1.1e-7` |
| M2a | supp. | `|a/(4/pi) - 1|` (the rod's first mode, `phi(L) Gamma = 4/pi`) | `<= 1e-3` | `1e-4` |
| M3 | supp. | bar B, every mode: relative displacement `m_0` under base along x against the force path with the pattern `-M r` built in the test (`consistent`, `rigid_motions`), max relative over the tip equations | `<= 1e-12` | rounding |
| Q1 | supp. | `Spectrum::mean_square` of the tables of arithmetic 4 against `35.0` and `180.92544802681783`, relative | `<= 1e-12` each | rounding |
| Q2 | supp. | channels without modes, residual 1: `m_0` against `mean_square`, linear and log-log | `<= 1e-13`, `<= 1e-5` | `2e-16`, `1.25e-6` |
| Q3 | supp. | one mode, force: `m_1`, `m_2`, `m_4` against `G f_n^(k+1) I_k(10)/k^2` (`I_1 = 38.77258066850795`, `I_2 = 39.16923599186199`, `I_4 = 49.006230409610886`); velocity `m_0` against `G (2 pi f_n)^2 f_n I_2/k^2`, acceleration `m_0` against `G (2 pi f_n)^4 f_n I_4/k^2`; relative | `<= 1e-6` each | `1.1e-7` |
| Q4 | supp. | `zero_crossing_rate` equals `fatigue::spectral_rate(m0, m2).1` bit for bit; `von_mises_rate` of S7's points against the `xx` component's `sqrt(C_2/C_0)`, relative | holds; `<= 1e-10` | holds; rounding |
| Q5 | supp. | cantilever C, static correction on, pattern `P_1`: tip `m_0` (three equations) against `sum_k w_k G |Y_j(f_k)|^2` with `Y` from the F-399 `HarmonicSolver::response` at the response's own nodes `f_k` and weights `w_k`, relative | `<= 1e-11` | rounding (two evaluations of the same sum over 1e3 nodes) |
| B1b | b2 | every digest of `sa_random_default_bits.rs` equal to the base's | equal | equal |
| X1 | b4 | `x1_modules_and_pending_after_f417` passes (`random` landed) | holds | holds |

Negative controls:

| Id | Quantity | Expected |
|---|---|---|
| N1 | S2 against the force form `G f_n I_0(10)` (without `4 zeta^2 I_2`): `|ratio - 1|` | `> 1e-3` (`1.6e-3`): the base integral is resolved |
| N2 | S5 against the two single-mode results of the library: `ratio - 1` | in `[5e-4, 2e-3]` (`1.04e-3`): the cross term is resolved |
| N3 | cantilever C, `rho = I` against the summed pattern: tip `u_y` metric | `> 0.1` (the two forces are nearly coherent in the response) |
| N4 | bar B with `nu = 0.3` (S7's set-up otherwise): `max_p |rms_vm/rms_xx - 1|` | `> 0.1` (`sigma_yy = sigma_zz = nu/(1 - nu) sigma_xx`: `0.43`) |

## Planned mutations (on a scratch copy, one at a time)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the grid without the mode scale (`ell = max(f, f_ref)`) | S1x, S2x, S6 |
| MU2 | `H_r` with `c_r` in place of `c_r omega` | S1 |
| MU3 | `G_ab = rho_ab G_a G_b` (no square root) | S8b |
| MU4 | the von Mises form with `+ C_12` | S7x |
| MU5 | the absolute term of the base acceleration dropped | S2 |
| MU6 | the frequency test `<` for `<=` (equal frequencies accepted) | R2 |
| MU7 | velocity multiplied by `omega^2` | Q3 |
| MU8 | the Gauss-Legendre weights `8/9` and `5/9` exchanged | S1x |
| MU9 | the residual `s_oa` of the static correction dropped | Q5 |

## Decision rule

- **done:** every check above met (S1 to S9, R1 to R3, M1 to M3, Q1 to Q5, B1b, X1), every negative control as expected, the
  existing tests of `rcfd-fem` passing (with the record of D8), the baseline hashes passing, fmt and clippy clean.
- **partial:** a clause of the acceptance missed or not run, with the miss recorded as a `*_known_miss` test and diagnosed; or a
  supplementary check missed.
- **todo:** the module not reaching the solid-model path.

## Limits (not covered)

- Real cross-spectra only, with a constant correlation coefficient: no quad-spectra, no frequency-dependent coherence, no
  spatially distributed (partially coherent) pressure fields.
- Base excitation of all supports together as one rigid translation: no multi-support excitation, no rotational base motion; the
  transformation path (elastic supports, linear constraints) refused for the base (the force path takes whatever the F-399 solver
  takes).
- Gaussian stationary inputs and modal damping only; no non-proportional damping (the complex modes of F-414 are not used), no
  pre-stressed basis beyond what `HarmonicOptions` carries.
- No broadband fatigue beyond the narrow-band rate of F-413 (Dirlik and similar need `m_0 .. m_4` of the von Mises equivalent,
  which is not Gaussian); no peak factors or extreme-value statistics; no shock response spectra (ADR 0310).
- No case-file key, no CLI, no API schema.
- No force spectrum of the fan exists (a scale-resolving flow run would supply one); the package is machinery.

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

- Q1. Case-file key: not added (b3). Recommended for the lead: `analysis = "random"` with `[[spectrum]]` tables (`frequencies`,
  `values`, `interpolation = "linear" | "log-log"`), `[[random_load]]` (a load set or base direction with its spectrum), an
  optional `correlation` matrix, and the outputs `rms_displacement`, `rms_stress`, `rms_von_mises`, `zero_crossing_rate`.
- Q2. Interpolation default linear (`Spectrum::new`), log-log on request: test specifications are usually given in dB/octave
  (log-log); the linear form is the one whose integral is exact and the acceptance's flat spectra are the same in both.
- Q3. Output quantities under base excitation: relative displacement and velocity, absolute acceleration (the quantities a
  specification and a stress check use).

## Addendum 1 (2026-11-14): results

Each registered run was made once, on commit 100d4449 (`cargo test -p rcfd-fem --release --test sa_random -- --nocapture
--test-threads 1`, log `f417-scratch/run1.log`); the commit after it (3a042264) is `cargo fmt` of the F-417 files and one clippy
fix (`!(d > 0.0)` written `d.is_nan() || d <= 0.0`, the same test), and the full release suite ran on it. The record B1b was
written and run on the base 57808e24 before any library line (commit ef95afda).

| Id | Clause | Limit | Measured | Met |
|---|---|---|---|---|
| S1 | a1 | `<= 5e-3` | `8.706e-6` | yes |
| S1x | supp. | `<= 1e-6` | `1.145e-7` | yes |
| S2 | a2 | `<= 5e-3` | `1.587e-3` | yes |
| S2x | a2 | `<= 1e-6` | `1.145e-7` | yes |
| S3 | a3 | decreasing, orders `>= 4` | force and base: `e = 1.145e-7, 1.398e-9, 2.049e-11`, orders `6.36`, `6.09` | yes |
| S4 | a4 | `<= 1e-2` | `1.278e-3` (`nu_0 = 49.9361` Hz) | yes |
| S4x | supp. | `<= 1e-6` | `7.6e-10` | yes |
| S5 | a5 | `<= 1e-2` | `1.032e-3` | yes |
| S5x | supp. | `<= 1e-6` | `1.056e-7` | yes |
| S6 | a6 | `<= 1e-6` | `1.097e-7` | yes |
| S6b | supp. | `<= 1e-6` | `1.083e-7` | yes |
| S7 | a7 | `<= 1e-10` | force `0`, base `0` | yes |
| S7x | supp. | `<= 1e-12` | `0` to `2.2e-16` | yes |
| S8 | a8 | `<= 1e-12` | displacement `1.5e-18`, stress `1.06e-15` | yes |
| S8s | supp. | `<= 1e-11` | displacement `3.9e-16`, stress `4.94e-12` | yes |
| S8b | supp. | `<= 1e-12` | `1.7e-16`, `6.5e-16` | yes |
| S9 | a9 | `<= 1e-12` | `1.7e-15`, `3.9e-15` | yes |
| R1 | a10 | holds | "entry 1: value -1e0 is negative or not finite" | yes |
| R2 | a11 | holds | both refused naming entry 2 | yes |
| R3 | supp. | each | all 15 refused with their key | yes |
| M1 | supp. | `<= 1e-5` | tip `1.145e-7`, stress `5.7e-8` (`f_1 = 1261.967` Hz) | yes |
| M2 | supp. | `<= 1e-5` | `1.125e-7` | yes |
| M2a | supp. | `<= 1e-3` | `1.285e-4` (`a = 1.273403`) | yes |
| M3 | supp. | `<= 1e-12` | `0` | yes |
| Q1 | supp. | `<= 1e-12` | `0`, `1.1e-16` | yes |
| Q2 | supp. | `<= 1e-13`, `<= 1e-5` | `2.2e-16`, `1.248e-6` | yes |
| Q3 | supp. | `<= 1e-6` | `m1 1.14e-7`, `m2 1.13e-7`, `m4 8.9e-8`, velocity `1.13e-7`, acceleration `8.9e-8` | yes |
| Q4 | supp. | bits; `<= 1e-10` | equal; `0` | yes |
| Q5 | supp. | `<= 1e-11` | `2.2e-16` | yes |
| B1b | b2 | equal | all 22 digests equal | yes |
| X1 | b4 | holds | `x1_modules_and_pending_after_f417` passes | yes |

Negative controls: N1 `1.596e-3 > 1e-3`; N2 `1.036e-3` in `[5e-4, 2e-3]`; N3 `0.4997 > 0.1`; N4 `0.4286 > 0.1` (the
`nu/(1 - nu)` of the estimate). All as expected.

Mutations (scratch copy `f417-scratch/mut`, `mutate.py`, log `mutations.log`), each caught by the registered check: MU1 (no mode
scale) by S1x, S2x, S6 and 19 others; MU2 (`c` for `c omega`) by S1 and 24 others; MU3 (no square root) by S8b only; MU4
(`+ C_12`) by S7x (and N4); MU5 (no absolute term) by S2, S2x, M2, S3; MU6 (`<` for `<=`) by R2 only; MU7 (velocity by
`omega^2`) by Q3 only; MU8 (weights exchanged) by S1x and 21 others; MU9 (residual dropped) by Q5 only. 9 of 9 caught.

Other runs: `cargo test -p rcfd-fem --release` (all targets, on 3a042264): 563 passed, 0 failed, 125 ignored (the record of D8
among them); `cargo test -p rcfd-api --release --test baseline_hashes`: 3 passed; `cargo test -p rcfd-cli --release --test
docs_site`: 2 passed; `cargo fmt -p rcfd-fem --check` clean; `cargo clippy -p rcfd-fem --all-targets -- -D warnings` clean. The
whole workspace suite was not run (the lead's gate).

Deviations: S8s stress `4.94e-12` is 25 times the probe's estimate `2e-13` (the probe normalised by the largest amplitude of the
vector, S8s per point by the largest diagonal there, which is smaller near the free end); within its limit, recorded. The only
shared file touched is `crates/rcfd-fem/src/harmonic.rs`: lines 735 to 742 added (the method of D7), no existing line changed.
`sa_random_default_bits.rs` was reformatted by `cargo fmt` after its record commit (layout only, the digests unchanged and
passing).

**Decision (rule above): done.** Every acceptance clause (a1 to a11) and every supplementary check met in one run, every negative
control as expected, 9 of 9 mutations caught, the existing tests of `rcfd-fem` and the baseline hashes passing, fmt and clippy
clean.

Remaining questions:

- Q1 (case-file key): open for the maintainer, recommendation in the registration (`analysis = "random"` with spectrum tables,
  random loads, an optional correlation and the four outputs); a follow-up below.
- Q4 (new): broadband fatigue of the random stress (Dirlik or similar on the moments of the von Mises equivalent): options (a) a
  follow-up item using `Moments` and `RandomStress::von_mises_rate`, (b) leave the narrow-band rate of F-413 as the only estimate;
  recommended (a), below. The Dirlik formula needs a cited source the repository does not have.
- Q5 (new): complex cross-spectra (quad-spectra) and frequency-dependent coherence for partially coherent pressure loads:
  recommended as a follow-up once a broadband load of the fan exists.

## For the lead: roadmap entries

Proposed status of F-417: **done**. Note:

> ADR 0477, 2026-11-14: rcfd_fem::random (stub of ADR 0440 filled, PENDING None): Spectrum tables (linear or log-log, refused with the entry for a negative value or a non-increasing frequency), ModalChannels and spectral_response on a grid of steps of a quarter of the local scale (the distance to the nearest half-power peak, or the frequency) with 3-point Gauss-Legendre, several patterns with a constant real correlation, moments m0, m1, m2, m4 with the zero-crossing rate by fatigue::spectral_rate, and RandomSolver on the F-399 HarmonicSolver (new method modal_decomposition) for force patterns and a base acceleration (relative displacement and velocity, absolute acceleration), with the stress covariance per integration point and the root-mean-square von Mises stress sqrt(tr(A C)). Every registered check was met in one run: one mode 8.7e-6 (force) and 1.59e-3 (base; the exact integral (pi/2) f_n G (Q + 1/Q) is 1.6e-3 above) from (pi/2) f_n Q G, 1.1e-7 from the exact finite-band integrals, observed orders 6.4 and 6.1 under refinement; zero-crossing rate 1.28e-3 below f_n (finite band); two modes 1.03e-3 above the sum (the CQC cross term); band-limited linear and log-log tables 1.1e-7 from the registered integrals; uniaxial rms von Mises equal to the rms stress exactly on an axial Hex8 rod (force and base); fully correlated patterns equal the summed pattern within 1.1e-15 (4.9e-12 with the static correction), uncorrelated ones add within 3.9e-15; the one-mode rod through the model path within 1.2e-7 of the closed forms; 9 of 9 mutations caught; default bits unchanged. Follow-ups proposed: case file and CLI, broadband fatigue, complex cross-spectra.

Follow-ups (numbers given by the lead):

> Title: random response in the case file and the CLI. Deps: F-417, F-533. Acceptance: Every limit not stated here is registered in the ADR before the run. A case with analysis = "random", a spectrum table, a force load set or a base direction and a modal damping ratio gives the root-mean-square displacement, stress, von Mises stress and zero-crossing rate of the library call on the same model bit for bit; a case without the new keys keeps its bits and the baseline hashes are unchanged; a negative spectrum value, a non-increasing frequency list and a correlation that is not positive semi-definite are refused with the key and the entry.

> Title: broadband fatigue of a random stress from its spectral moments. Deps: F-417, F-413. Acceptance: Every limit not stated here is registered in the ADR before the run. The broadband damage rate (formula registered from a cited source) reduces to the narrow-band rate of F-413 within 1 percent for a one-mode response at zeta = 0.01; for a two-mode response with well separated modes the counted damage of a synthesised Gaussian history (registered seed, 1e5 cycles, rainflow of F-413) lies within 10 percent of the broadband rate and the narrow-band rate is above it; the moments m0, m1, m2, m4 of RandomSolver feed it unchanged; a non-positive m0 or m2 is refused.

> Title: complex cross-spectral matrices and frequency-dependent coherence of random loads. Deps: F-417. Acceptance: Every limit not stated here is registered in the ADR before the run. A cross-spectral table G_ab(f) with real and imaginary parts, Hermitian and positive semi-definite at every table frequency (refused otherwise with the frequency), reproduces the constant real correlation of F-417 bit for bit when its imaginary part is zero; two patterns with a pure delay tau (G_12 = G e^{-i 2 pi f tau}) on a one-mode system give the closed-form mean square within 1e-6; a field with coherence exp(-|x_a - x_b|/L) on a plate tends to the uncorrelated result as L goes to 0 and to the fully correlated one as L grows, each within 1e-8 at the registered limits.
