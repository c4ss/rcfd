# Aeroacoustics for fan noise (wave 32B, ADR 0066)

This document describes the library chain added for fan noise: a time-domain Ffowcs Williams-Hawkings (FW-H) solver
for moving impermeable surfaces, a closed-form rotor tonal estimate (Gutin type) as an independent check, three
steady-RANS broadband proxies, noise metrics, and file readers and writers. It extends the permeable-surface solver
of ADR 0013 (`docs/reactor-and-acoustics.md`, `rcfd acoustics`). Plan item: F-243 stays partial; the acceptance
text of F-243 (impedance tube, quiet boundary) is not addressed by this wave.

Status of the numbers. The FW-H solver and the metrics are verified against closed-form fields (ADR 0066 lists each
check with its limit and the measured value). The RANS-based broadband proxies are NOT validated against any measured
noise and must not be read as predictions (section "Broadband proxies"). No comparison with a measured fan was made.

## Modules and entry points

| Item | Where |
|---|---|
| FW-H 1A, moving impermeable surface, medium-frame (uniform flow) evaluation | `crates/rcfd-post/src/fwh.rs` (`Surface::impermeable`, `into_medium_frame`, `signal_moving`, `valid_window_moving`, `medium_frame_observer`) |
| Metrics (SPL, OASPL, A weighting, spectrum, 1/3-octave, sphere power, JSON/CSV) | `crates/rcfd-post/src/acoustics.rs` |
| Rotor harmonics (Gutin type) | `crates/rcfd-post/src/rotor.rs` |
| Proxies (Proudman, Curle, trailing edge) | `crates/rcfd-post/src/noise_proxies.rs` |
| Surface-series reader, builder, adapter from `surfaces::Surface` | `crates/rcfd-post/src/surface_series.rs` |
| CLI | `rcfd fwh` (`crates/rcfd-cli/src/cmd_fwh.rs`) |

## FW-H solver

Formulation 1A (Brentner and Farassat) for a surface f = 0 with unit normal n, surface velocity v and fluid state
(p', rho', u) on it. With the permeable terms
U_i = (1 - rho/rho0) v_i + rho u_i / rho0 and L_i = p' n_i + rho u_i (u_n - v_n), p' is the sum of the thickness and
loading integrals evaluated at the retarded time. The impermeable case used for fans sets u = v and rho' = 0 on the
blade: U_i = v_i, L_i = p' n_i. The `loading` mode additionally sets U = 0 (loading noise only). No volume
(quadrupole) term is computed; this is an assumption of the whole chain (quadrupoles are small for subsonic tip
Mach numbers, not zero).

Numerics: retarded time by Newton iteration per element and observer time; 4-point Lagrange interpolation of the
source data; fifth-point derivatives (fourth order) for the time derivatives of the source terms. The valid window
guard refuses observer times for which the retarded time falls outside the sampled record, and refuses surfaces whose
speed is not subsonic with respect to the medium (error text contains "subsonic").

Uniform flow. A body in a uniform stream U (body frame) is evaluated in the frame of the medium, where the medium is
at rest: x_m = x_b - U t, v_m = v_b - U, u_m = u_b - U, and the observer, fixed in the body frame, moves on
x_o(t) = x_ob - U t. The formula is evaluated at the moving observer position, which is exact for 1A in a medium at rest;
the observer time axis is the same in both frames. Assumptions: uniform U with |U| below c0; the sampled pressure is the
perturbation relative to the reference (the mean is not removed, because steady blade loading rotating in the medium
frame is a source). Steady-flow loading terms that a given CFD data surface carries (u != v) are only represented in
the permeable path; the impermeable path assumes u = v.

Limits (all measured, see ADR 0066): fourth-order convergence in the observer-time step; non-compact pulsating sphere
thickness-only closed form; compact dipole; moving observer in uniform flow of a permeable monopole sphere (W5d, 4.5e-6).
The registered impermeable-sphere-in-flow checks W5a/W5b were MISSED (7.7e-2 and 6.7e-2 against 1e-2 and 1e-3): the test
construction used constant face areas and omitted the steady-flow loading the exact field implies, so the error is
proportional to U (M = 0.01 gives 2.4e-3). They are kept as ignored `*_known_miss` tests; W5d is a separate new check.

### Surface-series input (`rcfd-surface-series` v1)

JSON (written by `SeriesSpec::to_json`, read by the CLI):

```json
{"format": "rcfd-surface-series", "version": 1, "rho0": 1.2, "c0": 340.0, "t0": 0.0, "dt": 1e-4,
 "mode": "impermeable", "flow": [0, 0, 20.0],
 "motion": {"type": "rotation", "axis": [0, 0, 1], "origin": [0, 0, 0], "omega": 150.0, "angle0": 0.0,
            "velocity": [0, 0, 0]},
 "faces": [{"area": 1e-3, "x": [0.2, 0, 0], "n": [0, 0, 1], "p": [ ... ]}]}
```

`x` and `n` are the body-frame values at `t0`; `p` is the pressure minus the reference pressure. Instead of `p` per
face, `"pressure_csv": "p.csv"` (path relative to the JSON) refers to a table `t,f0,f1,...` with a uniform time column
(non-uniform steps are rejected with `pressure_csv row N`). The adapter `faces_from_post_surface` turns a
`rcfd-post` triangulated surface (area, centroid, normal, optional flip) into faces so that a pressure sampled on it
can be written to this format. Errors carry JSON paths (`faces[2].p`, `$.format`) or CSV row/column.

### CLI

```
rcfd fwh series.json --observer x,y,z [--observer ...] [--flow ux,uy,uz] [--times t0:t1:n]
         [--out signal.csv] [--spectrum bands.csv|spectrum.json] [--narrowband file.csv]
         [--fmin 20] [--fmax 20000] [--window hann|rect]
```

Output CSV: `observer,t,p_thickness,p_loading,p_total`. Exit codes: 0 success, 1 I/O, 2 usage, 3 invalid input.
The default time grid is the full valid window at the file sampling step. A `.json` spectrum target writes an
`rcfd-spectra` v1 document (one `rcfd-spectrum` report per observer), anything else writes the 1/3-octave band CSV.
The OASPL and dB(A) per observer are printed to stderr.

## Rotor tonal harmonics (Gutin type)

A rotor of B equally spaced blades, angular velocity Omega, loading represented by a compact force at the radius R_t
(thrust T along the axis on the fluid, torque Q, i.e. a tangential force Q/R_t), radiating to a far observer at distance
r and polar angle theta from the axis (measured towards the direction of the thrust force on the fluid). Fourier
decomposition of the rotating force in the far field of the compact dipole gives, only for the harmonics m B
(m = 1, 2, ...) of the blade passing frequency,

  |p_m| = (m B Omega / (2 pi c r)) |J_{mB}(m B M_t sin theta)| |T cos theta + Q c / (Omega R_t^2)|,   M_t = Omega R_t / c.

(Pressure amplitude; rms is amplitude/sqrt 2.) It is derived here from the dipole far field and checked against the
FW-H solver on a point-force rotor (errors <= 4e-5). Assumptions: compact chordwise loading at a single radius, far
field, free field, no thickness noise (the monopole term of the blades is not included), no forward flight. The sign
convention of the torque term (a torque on the rotor means a force on the fluid along +e_phi) is stated in the
`Rotor` documentation. The original Gutin (1936) formula was not read; this is a derivation, not a transcription.

## Broadband proxies (NOT validated)

These are cheap, steady-RANS-based scalings. They contain empirical constants whose values were not calibrated in
this repository and whose literature sources were read only second-hand. The levels they give are order-of-magnitude
indicators at best, and only the dependence on speed, k, epsilon and c0 is a result.

* Proudman (isotropic turbulence quadrupole): acoustic power per volume P = alpha rho0 eps M_t^5, M_t = sqrt(2k)/c0,
  default alpha = 0.1 (secondary source). Level in dB re 1e-12 W/m^3. This is a volume density to be integrated over
  the cells (`proudman_total_power`). Ignores mean-flow refraction and the actual finite size of the eddies; for
  wall-bounded flow near surfaces it is not the dominant source.
* Curle (surface dipole): W = sum <p_dot^2> A_c S / (12 pi rho0 c0^3) for independent patches of area S with correlation
  area A_c, and far-field mean square sum <p_dot^2> A_c S (n . rhat)^2 / (16 pi^2 c0^2 r^2). The wall pressure rate
  <p_dot^2>^(1/2) = 2 pi f_c p'_rms needs a characteristic frequency and an rms wall pressure that a steady RANS run
  does not provide; they are inputs (estimates from k, a Strouhal assumption). The two expressions are mutually
  consistent (P2 check) but both depend on A_c and f_c, which are unknown.
* Trailing edge: W = K_TE rho0 u_t^5 L_e l_t / c0^2 (Ffowcs Williams and Hall type fifth-power law; only the U^5
  exponent is sourced, K_TE has no calibrated value here and defaults are placeholders that must be supplied).

What validation would require. A fan or airfoil case with a measured operating point, wall-pressure data (or a
scale-resolving simulation) and far-field spectra; calibration of alpha, A_c, f_c and K_TE on one data set; and a
comparison on a held-out second data set, with a reported error band. Until then these numbers are not predictions.

## Metrics

* SPL = 20 log10(p_rms / 2e-5 Pa); OASPL from the mean-removed series; `None` (undefined) for a zero fluctuation,
  never -inf.
* A weighting: the IEC 61672-1 analytic formula, R_A(f) = 12194^2 f^4 / ((f^2 + 20.6^2) sqrt((f^2 + 107.7^2)(f^2 + 737.9^2))
  (f^2 + 12194^2)), A(f) = 20 log10 R_A(f) - 20 log10 R_A(1000) (normalised to 0 dB at 1 kHz). The constants come from
  secondary sources; the standard itself was not read.
* Spectrum: one-sided, Hann or rectangular window, Parseval-normalised so that the sum of the mean-square bins equals
  the (windowed) mean square of the series. Only the largest power-of-two length that fits is used (`n_used`).
* 1/3-octave bands: base-10, centres 1000 x 10^(n/10), edges centre x 10^(+-1/20), half-open [lo, hi); a spectral bin
  belongs to the band containing its frequency. Bands containing f_min..f_max are reported. Empty bands have level
  `None`/null. Nominal labels are the usual rounded values (table not verified against IEC 61260-1).
* Sound power: W = sum_i w_i R^2 <p_i^2>/(rho0 c0) over a Gauss-Legendre (colatitude) x uniform (azimuth) quadrature
  of an observer sphere of radius R in the far field (`sound_power_sphere`). Exact for a monopole at any R; the
  intensity p^2/(rho0 c0) assumption needs the far field for dipoles (dipole check in the far field only).
* Output: `Report::to_json` (`rcfd-spectrum` v1), `bands_csv`, `narrowband_csv`.

## Limits of the whole chain

Subsonic surfaces only; no quadrupole; no refraction or scattering by the body or casing (free field); the uniform flow
is the only mean flow; no installation effects, ducted propagation, or modal content; pressure data sampled by the CFD
run must resolve the blade-passing frequency and its harmonics (the FW-H result is only as good as the sampled surface
pressure); a steady RANS run gives no tonal pressure history. F-243 remains partial: an acoustic impedance boundary,
quiet boundary and acoustic wave solver are not part of this wave.
