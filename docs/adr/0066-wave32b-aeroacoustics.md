# ADR 0066: Wave 32B, aeroacoustics library chain for fan noise (F-243 partial, builds on F-136)

Status: accepted. The checks, numeric limits and decision rules below are written before any code of the new modules
is compiled or run. They are not widened, replaced or deleted after a run; a miss is reported as measured and the test
moves to an `#[ignore]`d `*_known_miss` test with the registered limit intact. Deviations go into a dated addendum.
Plan served: `docs/milestone-fan.md` (noise link: time-domain FW-H, steady-RANS broadband proxies, tonal Gutin check,
SPL/OASPL/octave bands). Roadmap: F-243 (stays `partial`: its acceptance text, impedance-tube reflection coefficient and
a quiet boundary reflecting less than 1 percent, is NOT addressed by this wave), builds on F-136 (ADR 0013).

## Process disclosure (before registration)

* Before this ADR was written, the A-weighting formula (constants 20.6, 107.7, 737.9, 12194 Hz, +2.00 dB) was evaluated
  once, in a throw-away script outside the repository, at the NOMINAL 1/3-octave frequencies and compared with a table
  recalled from memory (IEC 61672-1 style, 0.1 dB steps). The largest difference was 0.27 dB, at 12.5 Hz and 16 Hz. This
  taught one thing that is used below: the IEC table is tabulated at the exact base-10 frequencies 10^(k/10), not at the
  nominal ones, so the comparison is registered at exact frequencies. No other number from that run is used and the
  limits below are justified from the table's printed resolution, not from what was seen.
* Nothing else was run. The repository code of the new modules did not exist.

## Scope (all additive; no existing test, hash or output changes)

All in `crates/rcfd-post` (library) and `crates/rcfd-cli` (one new command), plus docs.

1. `fwh.rs` (extended, existing functions untouched and bit-identical): `Surface::impermeable` constructors (u := v,
   rho' := 0 so that U_i = v_i and L_i = p' n_i exactly; or loading only: u = 0, rho' = 0 so U_i = 0, L_i = p' n_i),
   `Surface::into_medium_frame(flow)` (uniform mean flow, see below) and `Surface::signal_moving` /
   `valid_window_moving` for an observer that moves on a prescribed path. `signal` is not modified.
2. `acoustics.rs`: SPL, OASPL, A-weighting (IEC 61672-1 analytic form), radix-2 FFT, Hann/rectangular power spectrum
   (Parseval normalised), base-10 1/3-octave band synthesis, spherical (Gauss-Legendre x uniform azimuth) quadrature and
   sound power from an observer sphere, CSV/JSON writers of spectra.
3. `rotor.rs`: Bessel J_n (Miller backward recurrence), Gutin-type steady-loading harmonics of a rotating point force
   (closed form derived below).
4. `noise_proxies.rs`: Curle-type surface dipole proxy, Proudman turbulence-quadrupole proxy, trailing-edge scaling
   proxy. Non-predictive of absolute level.
5. `surface_series.rs`: input format `rcfd-surface-series` v1 (JSON plus optional CSV pressure table), builder to an
   impermeable moving `fwh::Surface`, adapter from the existing post surface sampling
   (`surfaces::Surface::{area_vectors, centroids, facet_scalar}`).
6. CLI `rcfd fwh <series.json> --observer x,y,z ... [--flow ux,uy,uz] [--times t0:t1:n] [--out signal.csv]
   [--spectrum spec.csv|spec.json]`. The existing `rcfd acoustics` is unchanged.
7. Docs: `docs/acoustics.md` (new), `docs/reference-data.md` (section), `docs/roadmap.json` (F-243 note and
   status todo -> partial, acceptance explicitly not met).

## Formulas fixed before coding

Conventions: medium at rest in the "medium frame"; c the speed of sound; k = omega/c; p' acoustic pressure.

**FW-H, impermeable, uniform flow.** Formulation 1A as in ADR 0013 with U_i = v_i, L_i = p' n_i. A body fixed in a
uniform stream U_inf (body frame) is moved to the medium frame: a body point x_b is at x_m = x_b - U_inf t, the surface
velocity is v_m = v_b - U_inf, a fluid velocity u_m = u_b - U_inf, p' and the normals are unchanged, and a body-fixed
observer at x_ob has the path x_o(t) = x_ob - U_inf t. The FW-H retarded-time integral is a statement at the field point
(x, t); for a moving observer it is evaluated at x = x_o(t), which is exact (the Green's function is that of the medium
frame). Requirement: |v_m| < c. Volume quadrupole not included.

**Pulsating sphere (W1).** A sphere of radius a, surface normal velocity U_n = U0 cos(w t), in the medium at rest,
time dependence e^{-iwt}. Exact radiating-sphere field: phi = Re[A e^{i(kr - wt)}/r], u_r = d phi/dr,
p' = -rho0 d phi/dt = Re[i w rho0 A e^{i(kr - wt)}/r], u_r(a) = U0 gives A = U0 a^2 e^{-ika}/(ika - 1). Surface
pressure p'(a,t) = Re[i w rho0 U0 a/(ika - 1) e^{-iwt}]. The thickness term alone, for a stationary surface, is
p_T = (rho0/4 pi) closed-integral dU_n/dt (t - r/c)/r dS, and by the mean value property of e^{ikr}/r over a sphere
(exterior point at distance R) p_T(R,t) = -rho0 w U0 a^2 sinc(ka) sin(w(t - R/c))/R, sinc(x) = sin x / x. Thickness
alone therefore differs from the full field by the factor 1/sqrt(1 + (ka)^2) in amplitude (and a phase), which is why
W1a (thickness only) and W1b (thickness + loading with the exact surface p') are separate checks.
Realisation: x(t) = (a + eps sin(w t)) n_hat, v = eps w cos(w t) n_hat, constant element area a^2 dOmega, eps/a = 1e-5.
The kinematic velocity v enters the Mach terms; with v/c = eps k = 1.6e-5 this is a model error of order 1e-5, stated
here as part of the error budget.

**Compact dipole (W2).** Loading only, stationary sphere (a = 0.01 m, ka = 0.0157), p'(surface) = P0 sin(w t) n_x,
net force on the fluid F(t) = F0 sin(w t) e_x, F0 = P0 4 pi a^2/3. Exact point-force field
p' = (cos(theta)/(4 pi r)) (F'(t - r/c)/c + F(t - r/c)/r), theta from the x axis (this is the field used in ADR 0013 A4).
The finite-size form factor of the cos(theta) shell is 3 j1(ka)/(ka) = 1 - (ka)^2/10 = 1 - 2.5e-5.

**Rotating point force, Gutin-type harmonics (W3).** Derived here (Gutin 1936 and Lowson 1965 were NOT read; the
result is checked against the FW-H code, not against a citation). B equal blades; each carries a compact force on the fluid
F_b = T_b e_z + D_b e_phi (e_z the rotor axis in the direction of the thrust force on the fluid, e_phi the direction
of rotation, rotation rate Omega, radius R_t). Totals: T = B T_b, Q = B D_b R_t (torque, magnitude). Observer far away at
distance r, polar angle theta from +e_z. Far field of a moving point force: p' = (1/(4 pi c r)) d/dt'[F.r_hat/(1 - M_r)]
with t' = t - r/c, using the far-field retarded-time relation t' = tau - (R_t sin(theta)/c) cos(psi), psi = Omega tau +
phi_b - phi_o, and F.r_hat = T_b cos(theta) - D_b sin(theta) sin(psi), M_r = -M_t sin(theta) sin(psi),
M_t = Omega R_t/c. Since dt' = (1 - M_r) d psi/Omega, the factor 1/(1 - M_r) cancels in the Fourier integral.
Fourier coefficient at frequency n Omega (one blade, p' = sum_n c_n e^{-i n Omega t'}): with z = n M_t sin(theta),
(1/2pi) int e^{i(n psi - z cos psi)} d psi = (-i)^n J_n(z) and (1/2pi) int sin(psi) e^{i(n psi - z cos psi)} d psi =
-(1/(M_t sin theta)) (-i)^n J_n(z), so c_n = (-i n Omega/(4 pi c r)) (-i)^n J_n(z) [T_b cos(theta) + D_b/M_t] e^{i n(phi_o - phi_b)}.
Summing the B blades keeps n = m B only (factor B). The amplitude (peak) of harmonic m at frequency m B Omega:

  |p_m| = (m B Omega /(2 pi c r)) |J_{mB}(m B M_t sin(theta))| |T cos(theta) + Q c/(Omega R_t^2)|.

The rms value is |p_m|/sqrt(2). Sanity limits that are tested: the thrust term vanishes at theta = 90 degrees; for
M_t -> 0, |p_m| -> (mB Omega/(2 pi c r)) (mB M_t sin theta/2)^{mB}/(mB)! |T cos theta + Q c/(Omega R_t^2)| (small-argument
form of J); the relative sign between the thrust and torque terms is fixed by the convention above (torque force on the
fluid along +e_phi), tested at theta = 45 and 135 degrees where the two terms add and partly cancel. Validity: far field
r >> R_t and k_m r >> 1 (near-field terms O(1/(k_m r)) are not in the formula), compact chordwise loading, steady
loading, no thickness noise, no flow.

**Sound power.** Far field intensity I = <p'^2>/(rho0 c); W = closed-integral I dS over a sphere of radius R. Monopole
Q = Q0 sin(w t): p' = rho0 w Q0 cos(w(t - r/c))/(4 pi r) (purely 1/r, so the sphere integral is exact at any R),
W = rho0 w^2 Q0^2/(8 pi c). Compact dipole of force amplitude F0: W = w^2 F0^2/(24 pi rho0 c^3) (far field).

**A-weighting.** A(f) = 20 log10(R_A(f)/R_A(1000)), R_A(f) = 12194^2 f^4 / ((f^2 + 20.6^2) sqrt((f^2 + 107.7^2)(f^2 + 737.9^2)) (f^2 + 12194^2)).
This is the normalised-by-R_A(1000) variant, so A(1000) = 0 by construction (not the "+2.00 dB" offset variant, which
gives 0 only to about 1.4e-4 dB with these rounded constants). Constants and the formula were read on the Wikipedia
A-weighting page and on metricgate.com (both secondary, citing IEC 61672); IEC 61672-1 itself was not read.

**1/3-octave bands (base 10, IEC 61260-1 style, standard not read).** Centre f_c(n) = 1000 * 10^(n/10), edges
f_c * 10^(+-1/20), band = [f_lo, f_hi) half-open; a bin belongs to the band that contains its centre frequency.
Nominal labels (25, 31.5, 40, ... 20000) are the preferred numbers nearest f_c.

**Proxies (not validated; see the checks for what is and is not tested).**
* Curle-type surface dipole. Derivation in the docs (not taken from vendor text): for a surface with fluctuating
  pressure, far field p' = (1/(4 pi c)) (x_i/r^2) d/dt int p' n_i dS; for surface patches of correlation area A_c with
  mean-square pressure rate <p_dot^2> the force-rate variance is <F_dot_i F_dot_j> = int <p_dot^2> A_c n_i n_j dS, so
  <p'^2>(x) = int <p_dot^2> A_c (n.r_hat)^2 dS /(16 pi^2 c^2 r^2) and W = int <p_dot^2> A_c dS/(12 pi rho0 c^3).
  Inputs per face: area, normal, rms pressure rate (or p'_rms and a characteristic frequency f_c, <p_dot^2> =
  (2 pi f_c)^2 p'_rms^2), correlation area A_c. Steady RANS supplies none of these directly; modelling constants are
  user inputs (typical wall-pressure fluctuation p'_rms ~ a few times the wall shear stress is a literature rule, not read here).
* Proudman: P_A = alpha rho0 eps M_t^5 per unit volume, M_t = sqrt(2k)/c0, alpha = 0.1 (Sarkar and Hussaini calibration
  by DNS of isotropic turbulence). alpha and the M_t definition come from secondary sources only (original papers not read; UNVERIFIED;
  see [provenance](../provenance.md)). Assumes high Reynolds number, low Mach number, isotropic
  turbulence, no mean flow: a fan blade passage violates the last two.
* Trailing edge: only the exponent is sourced (Ffowcs Williams and Hall 1970: scattered power ~ U^5, from a search
  snippet and a review; the paper was not read). Scaling form W_TE = K_TE rho0 u_t^5 L_e l_t / c0^2 with u_t the
  turbulence velocity scale near the edge, L_e the edge length, l_t the turbulence length scale; K_TE is an uncalibrated
  user constant (default 1: only ratios between designs mean anything).

## Checks and numeric limits

Error measure: normalised RMS e = rms(p_fwh - p_exact)/rms(p_exact) over one period at 256 observer times inside the
valid window (as in ADR 0013), amplitude and phase from a DFT over whole periods. Surface quadrature: 16 Gauss-Legendre x
32 azimuth points (exact for spherical harmonics up to degree 31; the integrands are smooth). Error budget for the
sinusoidal checks (64 samples per period, w h = 0.098): 4-point Lagrange interpolation about 0.023 (w h)^4 = 2e-6,
5-point derivative (w h)^4/30 = 3e-6, model error 2e-5 (eps/a, v/c), so the predicted error is below 5e-5; the
limit 1e-3 is the ADR 0013 limit, a factor 20 above the budget, and keeps a miss meaningful.

### FW-H (file crates/rcfd-post/tests/fwh_impermeable.rs)

* W1a Pulsating sphere, thickness only (loading p' = 0), a = 1 m, k = pi/2 (wavelength 4 m, ka = 1.57), eps/a = 1e-5, observers
  R = 10 and 100 m on two different directions: e <= 1e-3 against p_T above; DFT amplitude relative error <= 1e-3;
  phase error <= 1e-3 rad.
* W1b Same sphere, thickness + loading with the exact surface pressure p'(a,t): e <= 1e-3 against the exact
  radiating-sphere field, amplitude relative error <= 1e-3, phase <= 1e-3 rad, R = 10 and 100 m.
* W2 Compact dipole: e <= 1e-3 at theta = 45 degrees (R = 100 m), and amplitude directivity |A(theta)/A(0)| - |cos theta| <= 1e-3
  absolute at theta = 0, 30, 60, 90, 120, 150, 180 degrees. Budget: form factor 2.5e-5 and numerics 1e-5; the exact
  near-field terms are in the reference, so no far-field term enters.
* W3 Rotating point force vs the Gutin-type formula above, p_loading only (u = 0, rho' = 0), R_t = 1 m, M_t = 0.3, thrust
  and torque per blade chosen so that T cos(theta) and Q c/(Omega R_t^2) are comparable (T = 30 N, Q c/(Omega R_t^2) = 30 N),
  256 source samples per revolution, observer r = 2000 m, theta = 45, 90, 135 degrees, B = 3 (m = 1, 2, 3) and B = 1
  (m = 1, 2, 3): relative amplitude error of each harmonic <= 1e-2 (budget: near field 1/(k_m r) <= 1.7e-3, Fraunhofer
  amplitude term R_t/r = 5e-4, numerics <= 1e-4; the limit is about 4x the budget), harmonics counted only where the
  reference is >= 1e-3 of the largest harmonic at that angle. Harmonic structure: for B = 3, the amplitude at n Omega for n = 1, 2,
  4, 5 is <= 1e-3 of the amplitude at 3 Omega (exact symmetry; the budget is roundoff and interpolation, 1e-5).
  Phase: the phase difference between harmonics is not tested (amplitude check only), stated as a limit of this check.
* W4 Observer-time interpolation order. Rotating point force (B = 1, M_t = 0.3, theta = 60 degrees, r = 100 m), source
  sampling 128, 256, 512 samples per revolution, reference 2048 per revolution, 200 observer times (not aligned with the
  samples) in the interior of the valid window at least 0.5 revolution away from either end; e_h = nRMS(s_h - s_2048).
  Observed orders p_a = log2(e_128/e_256) and p_b = log2(e_256/e_512) must both be >= 3.5. Justification: 4-point
  Lagrange interpolation and the 5-point derivative are fourth order; the 0.5 allowance covers the pre-asymptotic
  range. (The interior restriction avoids the second-order derivative stencils at the first and last two nodes.)
* W5 Uniform flow. (a) On axis, pulsating sphere a = 0.01 m (ka = 0.0157), eps/a = 1e-4, M = U_inf/c = 0.3 flow along +x,
  body-fixed observers at x = +1000 and -1000 m: derived closed form for a compact source at rest in the stream:
  p' = rho0 Q0 w cos(w(t - |X|/(c +- U_inf)))/(4 pi |X| (1 +- M)) (+ downstream, - upstream; 1/X^2 terms dropped,
  Q0 = 4 pi a^2 eps w). Amplitude relative error <= 1e-2, phase <= 5e-3 rad (budget: near field 1/(k|X|) <= 6e-4 times
  O(1), numerics <= 1e-4; limits 8x budget). The (1 +- M) factors differ by 86 percent between the two observers, so the check
  is sensitive to a wrong frame. (b) Off axis (45 degrees, 50 m): e <= 1e-3 against the exact Lienard-Wiechert field of
  a monopole of strength Q0 sin(w tau) moving at -U_inf in the medium frame, p' = -rho0 d phi/dt at the moving medium-fixed
  point (8th-order central differences, as ADR 0013 A5). (c) With flow = 0, `signal_moving` with a constant observer reproduces `signal`
  to a relative difference <= 1e-12.
* W6 Sound power. (a) Quadrature identity: sphere power of a spatially uniform p' = P cos(w t) is 4 pi R^2 P^2/(2 rho0 c)
  to 1e-10 relative (GL x azimuth exact; whole periods exact), and the quadrature integrates 1 and n_z^2 to 4 pi and 4 pi/3 within 1e-13.
  (b) Monopole through FW-H: pulsating sphere a = 0.01 m, eps/a = 1e-4, observers on an 8 x 16 GL sphere of radius R = 10 m,
  power from the FW-H pressure series equals rho0 w^2 Q0^2/(8 pi c) (Q0 = 4 pi a^2 eps w) within 1e-3
  (budget: sinc(ka) squared 8e-5, interpolation 1e-5). (c) Compact dipole through FW-H, R = 500 m, power equals
  w^2 F0^2/(24 pi rho0 c^3) within 1e-3 (budget: (kR)^-2 = 2e-6, form factor 5e-5).

### Metrics (file crates/rcfd-post/tests/acoustics_metrics.rs)

* M1 SPL: p_rms = 20e-6 Pa gives 0 dB (<= 1e-12), 1 Pa gives 20 log10(5e4) = 93.97940008672 dB (<= 1e-9 dB), doubling p adds
  6.02059991328 dB (<= 1e-12). OASPL of a sine of amplitude A over whole periods = 20 log10(A/(sqrt 2 * 20e-6)) (<= 1e-9 dB).
  OASPL from the Parseval spectrum (rectangular window, tone on a bin) equals the time-domain value (<= 1e-9 dB).
* M2 A-weighting: A(1000) = 0 (<= 1e-12 dB). Against the three values read on metricgate.com: 63 Hz -26.22, 125 Hz
  -16.19, 2 kHz +1.20 dB, |diff| <= 0.01 dB (printed to 0.01, so 0.005 rounding + 0.005 for the rounded pole constants).
  Against a table recalled from MEMORY (unverified, not a citation): at the exact frequencies 10^(k/10), k = 10..43, the
  values -70.4, -63.4, -56.7, -50.5, -44.7, -39.4, -34.6, -30.2, -26.2, -22.5, -19.1, -16.1, -13.4, -10.9, -8.6, -6.6, -4.8,
  -3.2, -1.9, -0.8, 0, +0.6, +1.0, +1.2, +1.3, +1.2, +1.0, +0.5, -0.1, -1.1, -2.5, -4.3, -6.6, -9.3 dB: |diff| <= 0.1 dB
  (printed resolution 0.05 dB + 0.005 dB constants rounding + 0.045 allowance for the table's own rounding of exact
  values). Finite for f in [0.1 Hz, 1 MHz]; f <= 0 or NaN -> located error.
* M3 Bands: centres 1000 * 10^(n/10), edges 10^(+-1/20); a pure tone on a DFT bin at 100, 1000, 1250, 5000 Hz lands in the
  band with nominal label 100, 1000, 1250, 5000 only (all other bands zero), tone at 1122 Hz in band 1000 and at 1123 Hz in
  band 1250 (edge 1122.018). Total band energy equals the time-domain mean square: tone on a bin, rectangular window,
  relative <= 1e-9; tone at 1003.7 Hz with Hann window, relative <= 1e-6 (Hann leakage outside 20 Hz..20 kHz is
  below 1e-12 by the k^-3 sidelobe decay). Parseval identity of `power_spectrum` (sum of bins = windowed mean square):
  relative <= 1e-12. Band levels are None (JSON null, CSV empty) for zero energy, never NaN or infinite.
* M4 Bessel: |J_n(x) (Miller) - trapezoid integral of (1/pi) int cos(n t - x sin t)| <= 1e-13 for n = 0..40 and
  x in {0.1, 1, 5, 20, 50} (the trapezoid rule is spectrally accurate for periodic integrands; 2000 nodes).
* M5 Located errors and NaN freedom, each returns Err with a message containing the quoted text: SPL p_rms < 0 or NaN
  ("p_rms"), dt <= 0 ("dt"), signal shorter than 8 samples ("samples"), non-finite sample ("sample 17" style index),
  frequency vector not increasing ("freq"), observer coincident with a surface point ("observer"), surface-series JSON
  missing key ("$.faces"), pressure series length mismatch ("faces[2].p"), non-uniform CSV time column ("row 5"),
  non-subsonic motion ("subsonic"), Proudman k < 0 ("k"), Curle negative area ("area"). All public outputs on a valid
  random-ish sweep are finite.

### Series format, adapter, CLI (files crates/rcfd-post/tests/surface_series.rs, crates/rcfd-cli/tests/fwh_cli.rs)

* S1 `rcfd-surface-series` JSON and CSV pressure table round trip (writer then reader) reproduces the data bit-exactly;
  the rotation builder keeps |x - origin| constant (<= 1e-12 relative), v = omega x r (<= 1e-12 relative) and n unit.
* S2 Adapter from `surfaces::Surface`: sum of face areas equals `Surface::area()` (<= 1e-12 relative), normals unit and equal
  to the triangle normals (<= 1e-12), centroids equal `Surface::centroids`.
* S3 CLI `rcfd fwh` on a series file reproduces the library signal to relative <= 1e-12, writes a spectrum CSV and JSON
  that parse, with OASPL in the file equal to the library value (<= 1e-12 dB), and reports malformed input with a located
  message and exit code 3 (usage errors 2, I/O 1) as `rcfd acoustics` does.

### Proxies (file crates/rcfd-post/tests/noise_proxies.rs): identities only

* P1 Proudman arithmetic equals alpha rho0 eps (sqrt(2k)/c0)^5 (<= 1e-12 relative); scaling: k x4 -> x32, eps x2 -> x2,
  c0 x2 -> /32 (<= 1e-12 relative); dB re 1e-12 W/m^3; k = 0 gives power 0 (level None).
* P2 Curle proxy: for a flat plate with all normals along z, the sphere integral (GL quadrature) of the far-field
  mean square divided by rho0 c times R^2 equals W = int <p_dot^2> A_c dS/(12 pi rho0 c^3) within 1e-10 relative.
  This is an internal consistency check of the derivation and the code; it says nothing about absolute levels.
* P3 Trailing-edge scaling: u_t x2 -> x32, c0 x2 -> /4, K_TE linear (<= 1e-12 relative).
* None of P1-P3 compares with measured noise. The documentation states that proxy levels are NOT validated and what
  validation would need (a scale-resolving or measured wall-pressure/far-field data set for a fan or airfoil with
  known operating point, then a calibration of alpha, A_c, f_c, K_TE on one data set and a held-out comparison on another).

## Decision rules

A check passes only if measured <= registered limit. Misses are reported with the measured number and moved to
`*_known_miss` (ignored) tests with the limit unchanged. Existing tests (including `crates/rcfd-post/tests/fwh.rs`,
`crates/rcfd-cli/tests/acoustics.rs`, `crates/rcfd-api/tests/baseline_hashes.rs`) are not edited. A flaw found in
the W3 derivation after a run is a deviation: it is disclosed in an addendum, the registered formula is kept as
registered and its test moved to a known miss, and a corrected formula is added as a separate new check.

## Unverified references (to be recorded in docs/reference-data.md)

Farassat 1A and the permeable terms: as in ADR 0013. Gutin (1936), Lowson (1965): not read; the formula is derived here. Curle
(1955), Proudman (1952), Lilley, Sarkar and Hussaini (NASA/ICASE report, a scanned PDF that could not be read): not read;
alpha = 0.1 and M_t = sqrt(2k)/c0 from secondary sources only, UNVERIFIED. Ffowcs Williams and Hall (1970): exponent from a
review snippet. IEC 61672-1 and IEC 61260-1: not read; A-weighting constants from Wikipedia and metricgate.com.

## Addendum 1: results, misses and deviations (written after the runs)

### Measured results (release build, git-archive overlay of the final tree)

Limits are the registered ones above; "pass" means measured <= limit.

| Check | Measured | Result |
|---|---|---|
| W1a/W1b non-compact pulsating sphere, thickness-only closed form and full radiating sphere (R = 10 and 100) | rms-relative 1.63e-5, amplitude 4.4e-6 .. 4.5e-6, phase 0.5e-9 .. 5e-9 rad | pass |
| W2 compact dipole directivity and series | directivity <= 2.5e-9, series 7.85e-5 | pass |
| W3 Gutin-type formula against FW-H (B = 1, 3; theta = 45, 90, 135; m = 1..3) | relative 1.1e-7 .. 3.97e-5; non-multiple harmonics n != mB below 2.2e-10 of the n = B line | pass |
| W4 observer-time convergence | errors 3.1e-7, 1.9e-8, 1.2e-9; orders 3.998, 4.016 | pass |
| W5a, W5b impermeable sphere in uniform flow | 7.695e-2 and 6.673e-2 | MISS (limits 1e-2 and 1e-3), ignored known-miss tests |
| W5c medium-frame transformation invariance | 0 (scale 8.8e-2) | pass |
| W5d permeable pulsating sphere in uniform flow, downstream, upstream, off axis | amplitude 4.5e-6, phase 1.3e-4 .. 2.5e-4 rad, off-axis 4.5e-6 | pass |
| W6a quadrature and sound power identity | 8.9e-16, 2.4e-15 | pass |
| W6b, W6c sound power monopole and dipole against closed forms | 9.05e-5, 1.55e-4 | pass |
| M1 .. M4 metrics (OASPL, A weighting, spectrum/bands, Bessel) | OASPL exact to the printed digits; Parseval 1.1e-15 / 4.4e-16; band assignment correct for 100, 1000, 1250, 5000 Hz and the 1122/1123 Hz edge; Bessel vs trapezoid 2.6e-15; A-weight table recalled from memory agrees within 0.049 dB (memory table, not a source) | pass |
| M5 located errors | all registered messages found | pass |
| S1 round trip and rotation kinematics | bit exact; radius 2.2e-16, velocity 0, normal 0 | pass |
| S2 adapter | areas, normals, centroids to <= 1e-12 | pass |
| S3 CLI against library, with and without flow; OASPL in the JSON spectrum | difference 0 relative; OASPL identical; exit codes 3 / 2 as registered | pass |
| P1 .. P3 proxy identities and scalings | <= 1e-12 (P2 sphere integral 1.1e-15) | pass (identities only, no validation) |

### The W5a/W5b miss

Registered: W5a (on axis, M = 0.3, amplitude <= 1e-2 and phase <= 5e-3 rad against the closed form of a compact point
source at rest in the stream) and W5b (off axis, <= 1e-3 against the Lienard-Wiechert field of a moving monopole) for
an impermeable pulsating sphere. Measured 7.695e-2 and 6.673e-2. The hypothesis, not proven: the reference is the field
of a point mass source, whereas the impermeable sphere held in a stream is a different physical problem (the steady flow
around the body carries steady loading that the constructed data omit, and the face data used constant areas), so the
error scales with U and the registered reference does not describe the constructed problem. Diagnostic run
after the miss (not a gate): at M = 0.01 the same construction gives 2.4e-3, consistent with an error proportional to U.
To check the medium-frame and moving-observer machinery itself, a new check W5d was added with the same limit as W5a
(1e-2): a permeable pulsating sphere, whose field in the medium frame is exact, observed downstream, upstream and off
axis in a stream; measured 4.5e-6 (amplitude) and phase errors of 1e-4 .. 2.5e-4 rad. W5d does not replace W5a/W5b; the
original tests remain ignored with their limits intact, and the impermeable-in-flow case is therefore NOT verified
against an exact solution in this wave.

### Other deviations from the registration

1. Band range: the registration said centres in [f_min, f_max]; with f_min = 20 that dropped the 20 Hz band. Bands
   containing f_min .. f_max are reported instead (docs/acoustics.md).
2. `parse_pressure_csv` compares each time step with the first step (relative 1e-6) rather than an average step, so a
   non-uniform column is located at its row.
3. The CLI flag set (`--observer`, `--flow`, `--times`, `--out`, `--spectrum`, `--narrowband`, `--fmin`, `--fmax`,
   `--window`) was fixed during implementation; the registration did not list flags.
4. The A-weighting table check M2 uses a table recalled from memory (see Unverified references); it is a consistency
   check on the formula, not a verification against IEC 61672-1.
5. Concurrent work of waves 32A/32C was present in the working tree; the final gate was run on a git-archive build of
   the committed state.
