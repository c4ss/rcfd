# Aeroacoustics for fan noise (wave 32B, ADR 0066)

This page explains the library chain that produces a fan-noise estimate from a CFD run: what each part assumes and how
far its numbers can be trusted. It is for users who want such an estimate and for anyone judging one.

The Ffowcs Williams-Hawkings (FW-H) solver and the metrics are verified against closed-form fields. The registered
impermeable-sphere-in-flow checks W5a/W5b were MISSED (section "FW-H solver"). The RANS-based broadband proxies are
NOT validated against any measured noise and must not be read as predictions (section "Broadband proxies"). No
comparison with a measured fan was made.

The chain was added for fan noise in wave 32B (a wave is a numbered batch of work; see the [glossary](glossary.md)). It
consists of a time-domain FW-H solver for moving impermeable surfaces, a closed-form rotor tonal estimate (Gutin type)
as an independent check, three steady-RANS broadband proxies, noise metrics, and file readers and writers. It extends
the permeable-surface solver of ADR 0013 (`docs/reactor-and-acoustics.md`, `rcfd acoustics`). Roadmap item F-243 stays
partial: this wave does not address the acceptance text of F-243 (impedance tube, quiet boundary).

For the older permeable-surface command and its input file read [reactor-and-acoustics.md](reactor-and-acoustics.md).
For every check with its limit and measured value read [ADR 0066](adr/0066-wave32b-aeroacoustics.md).

## Limits of the whole chain

* Subsonic surfaces only.
* No quadrupole.
* No refraction or scattering by the body or casing (free field).
* The uniform flow is the only mean flow.
* No installation effects, ducted propagation, or modal content.
* Pressure data sampled by the CFD run must resolve the blade-passing frequency and its harmonics (the FW-H result is
  only as good as the sampled surface pressure).
* A steady RANS run gives no tonal pressure history.

An acoustic impedance boundary, quiet boundary and acoustic wave solver are not part of this wave. The impedance and
quiet boundaries were added later as library code (section "Impedance and quiet boundaries of the compressible solver");
the wave solver does not exist.

## FW-H solver

The solver uses Formulation 1A (Brentner and Farassat), which gives p' as the sum of the thickness and loading integrals
evaluated at the retarded time. The surface is f = 0, with unit normal n, surface velocity v and fluid state
(p', rho', u) on it. The permeable terms are U_i = (1 - rho/rho0) v_i + rho u_i / rho0 and
L_i = p' n_i + rho u_i (u_n - v_n).

The impermeable case used for fans sets u = v and rho' = 0 on the blade: U_i = v_i, L_i = p' n_i. The `loading` mode
additionally sets U = 0 (loading noise only). The solver computes no volume (quadrupole) term. This is an assumption of
the whole chain (quadrupoles are small for subsonic tip Mach numbers, not zero).

The numerics are:

* retarded time by Newton iteration per element and observer time;
* 4-point Lagrange interpolation of the source data;
* fifth-point derivatives (fourth order) for the time derivatives of the source terms.

The valid window guard refuses observer times for which the retarded time falls outside the sampled record. It also
refuses surfaces whose speed is not subsonic with respect to the medium (error text contains "subsonic").

The solver evaluates a body in a uniform stream U (body frame) in the frame of the medium, where the medium is at rest:
x_m = x_b - U t, v_m = v_b - U, u_m = u_b - U, and the observer, fixed in the body frame, moves on
x_o(t) = x_ob - U t. The solver evaluates the formula at the moving observer position, which is exact for 1A in a medium
at rest. The observer time axis is the same in both frames.

This evaluation assumes uniform U with |U| below c0, and that the sampled pressure is the perturbation relative to the
reference. The mean is not removed, because steady blade loading rotating in the medium frame is a source. Only the
permeable path represents steady-flow loading terms that a given CFD data surface carries (u != v); the impermeable path
assumes u = v.

Limits (all measured, see ADR 0066):

* fourth-order convergence in the observer-time step;
* non-compact pulsating sphere thickness-only closed form;
* compact dipole;
* moving observer in uniform flow of a permeable monopole sphere (W5d, 4.5e-6).

The registered impermeable-sphere-in-flow checks W5a/W5b were MISSED (7.7e-2 and 6.7e-2 against 1e-2 and 1e-3). The test
construction used constant face areas and omitted the steady-flow loading the exact field implies, so the error is
proportional to U (M = 0.01 gives 2.4e-3). They are kept as ignored `*_known_miss` tests; W5d is a separate new check.

## Rotor tonal harmonics (Gutin type)

The estimate treats a rotor of B equally spaced blades with angular velocity Omega. A compact force at the radius R_t
represents the loading (thrust T along the axis on the fluid, torque Q, i.e. a tangential force Q/R_t). The rotor
radiates to a far observer at distance r and polar angle theta from the axis (measured towards the direction of the
thrust force on the fluid). Fourier decomposition of the rotating force in the far field of the compact dipole gives,
only for the harmonics m B (m = 1, 2, ...) of the blade passing frequency,

  |p_m| = (m B Omega / (2 pi c r)) |J_{mB}(m B M_t sin theta)| |T cos theta + Q c / (Omega R_t^2)|,   M_t = Omega R_t / c.

This is the pressure amplitude; rms is amplitude/sqrt 2. The formula is derived here from the dipole far field and
checked against the FW-H solver on a point-force rotor (errors <= 4e-5). It assumes compact chordwise loading at a single
radius, far field, free field, no thickness noise (the monopole term of the blades is not included) and no forward
flight. The `Rotor` documentation states the sign convention of the torque term (a torque on the rotor means a force on
the fluid along +e_phi). The original Gutin (1936) formula was not read; this is a derivation, not a transcription.

## Broadband proxies (NOT validated)

These are cheap, steady-RANS-based scalings. They contain empirical constants whose values were not calibrated in
this repository and whose literature sources were read only second-hand. The levels they give are order-of-magnitude
indicators at best, and only the dependence on speed, k, epsilon and c0 is a result.

* Proudman (isotropic turbulence quadrupole): acoustic power per volume P = alpha rho0 eps M_t^5, M_t = sqrt(2k)/c0,
  default alpha = 0.1 (secondary source). The level is in dB re 1e-12 W/m^3. This is a volume density to be integrated
  over the cells (`proudman_total_power`). It ignores mean-flow refraction and the actual finite size of the eddies; for
  wall-bounded flow near surfaces it is not the dominant source.
* Curle (surface dipole): W = sum <p_dot^2> A_c S / (12 pi rho0 c0^3) for independent patches of area S with correlation
  area A_c, and far-field mean square sum <p_dot^2> A_c S (n . rhat)^2 / (16 pi^2 c0^2 r^2). The wall pressure rate
  <p_dot^2>^(1/2) = 2 pi f_c p'_rms needs a characteristic frequency and an rms wall pressure that a steady RANS run
  does not provide; they are inputs (estimates from k, a Strouhal assumption). The two expressions are mutually
  consistent (P2 check) but both depend on A_c and f_c, which are unknown.
* Trailing edge: W = K_TE rho0 u_t^5 L_e l_t / c0^2 (Ffowcs Williams and Hall type fifth-power law; only the U^5
  exponent is sourced, K_TE has no calibrated value here and defaults are placeholders that must be supplied).

Validation would require a fan or airfoil case with a measured operating point, wall-pressure data (or a
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

## Impedance and quiet boundaries of the compressible solver (F-243, ADR 0233; library only)

The density-based compressible solver (`rcfd-fv` `compressible.rs`) has an acoustic wall with a real specific impedance and
two quiet (non-reflecting) boundaries. They are reachable from Rust (`CompressibleSolver::set_bc`), not yet from a case file,
the CLI or the MCP tools. Every check with its limit and measured value is in
[ADR 0233](adr/0233-f243-registration-impedance-and-quiet-boundary.md).

| Boundary | What it imposes | Reflection of a plane wave at the angle theta from the normal |
|---|---|---|
| `CompBc::Impedance { p, z }` | `p_face - p = z u_n` (`u_n` relative to the face), `z` in Pa s/m, real, `>= 0` | `(z cos(theta) - rho c)/(z cos(theta) + rho c)` |
| `CompBc::Impedance { p, z: rho c }` (matched; quiet) | the incoming characteristic of the ambient state; holds the pressure `p` | `(cos(theta) - 1)/(cos(theta) + 1)` |
| `CompBc::NonReflecting { p, sigma: 0.0 }` (Thompson; quiet) | nothing: the face takes the interior state; does not hold a pressure | `(1 - cos(theta))/(1 + cos(theta))` |
| `CompBc::Slip` | rigid wall | `+1` |
| `CompBc::PressureOutlet { p }` (the same as `z = 0`) | the pressure `p` | `-1` at normal incidence |

`rho c` is the density times the sound speed of the ambient gas (about 413 Pa s/m for air at 20 degrees C); the caller
supplies it. The face state keeps the entropy, the tangential velocity and the outgoing Riemann invariant of the interior
cell and solves the impedance relation with the exact simple-wave relations, so it is not limited to small amplitudes.

What was checked (closed forms, Cartesian grids, gas at rest):

* a pulse in a tube for `z/(rho c)` = 0, 1/3, 1, 3, 19: the reflection coefficient is within 1.2e-3 of
  `(z - rho c)/(z + rho c)` (limit 1e-2);
* both quiet boundaries reflect a pulse at normal incidence by less than 1e-4 (limit 1 percent);
* a tone in a tube: the standing-wave ratio is 3 within 0.2 percent for `z = 3 rho c` and `z = rho c/3`, with the pressure
  maximum at the stiff wall and the minimum at the soft one;
* a uniform overpressure in a tube closed by two matched walls returns to the ambient pressure; with two Thompson
  boundaries it stays;
* plane pulses at 30 and 45 degrees: the three formulas of the table within 0.015 (limit 0.03).

Limits:

* The impedance is real and independent of frequency. A frequency-dependent wall is the mass-spring-damper wall of the
  next section; liners and resonators with a measured impedance curve are not built.
* Both quiet boundaries are quiet only near normal incidence: they reflect 7 percent of a wave arriving at 30 degrees and
  17 percent at 45 degrees (below 1 percent only within 11 degrees of the normal). For oblique waves put a sponge layer
  in front of the boundary (next section). There is no multi-dimensional non-reflecting condition.
* The matched wall is matched for the `rho c` it is given; a mean flow through the boundary or a different local state
  changes the match.
* No wave-equation solver and no modal export exist.

## Frequency-dependent wall: the mass-spring-damper wall (F-332, ADR 0243; library only)

A wall whose impedance depends on frequency needs memory in the time domain. `MsdWall { p, r, m, k }` (`rcfd-fv`
`compressible_bc.rs`) is the simplest such wall: per unit area a plate of mass `m` [kg/m^2] on a spring `k` [Pa/m] and a
damper `r` [Pa s/m], pushed by the gas: `m dv/dt + r v + k xi = p_face - p`, `d xi/dt = v`. For a tone its specific
impedance is

    Z(omega) = r + i (omega m - k/omega)        (time factor e^{+i omega t}),

and a plane wave at normal incidence is reflected with `(Z - rho c)/(Z + rho c)`, a complex number: below the resonance
`omega0 = sqrt(k/m)` the wall is a spring, above it a mass, at the resonance it is the damper alone. It is opt-in
(`CompressibleSolver::set_msd_wall(patch, Some(wall))`, which replaces the patch's `CompBc` while it is set); without it
the solver produces the same bits as before. It is reachable from Rust, not yet from a case file. Every check with its
limit and measured value is in [ADR 0243](adr/0243-f332-registration-mass-spring-damper-wall.md).

How it works: every face of the patch carries the displacement `xi` and the velocity `v` of the wall
(`msd_wall_state`, `set_msd_wall_state`). The face state keeps the entropy, the tangential velocity and the outgoing
Riemann invariant of the interior cell, as the impedance wall does, and has the normal velocity `v`; its pressure
drives the wall. `xi` and `v` are advanced with the same Runge-Kutta stages as the flow. With `m = 0` the wall has no
velocity of its own and the face is the impedance wall with the reference pressure `p + k xi`; with `m = 0` and `k = 0`
it is `CompBc::Impedance { p, z: r }` bit for bit.

The step: the wall equation is integrated explicitly, so a light mass or a stiff spring limits the step to
`msd_dt_limit()` = `0.8 min(2m/(r + rho c), (r + rho c)/k)`. `max_dt()` includes this limit while a wall is set, and
`step(dt)` refuses a larger step. If the limit is far below the CFL step, the mass is too light to matter at the
resolved frequencies: set `m = 0` (and for a spring too stiff to matter use `CompBc::Slip`).

What was checked (closed forms, tube with `rho c = 1`, wall with `r = 0.5`, `m = 1/pi`, `k = 16 pi`, resonance at the
wavelength 0.5, 320 to 500 cells per wavelength):

* a tone at 0.8, 1 and 1.25 times the resonance frequency: the measured complex reflection coefficient is within
  `2.9e-4` in magnitude (relative) and `2.4e-4` rad in phase of the closed form (0.797 at -55.3 degrees, 1/3 at 180
  degrees, 0.797 at +55.3 degrees); the limit was 2 percent and 0.02 rad. On half the resolution, at the resonance:
  `4.5e-4` and `1.3e-3` rad;
* the same measurement (least-squares split of the tone into the incident and the reflected wave at 400 probe cells) on
  walls with a known real reflection, the impedance wall `z = rho c/2` and the rigid wall: within `1.3e-4` and `5e-5`
  rad, so the measurement resolves the tolerance; the impedance wall alone misses the closed form of the
  mass-spring-damper wall by 58 percent and 2.2 rad away from the resonance;
* the wall without damper reflects with the magnitude 1 within `2.8e-3` at the three frequencies;
* a wall released from a displacement in a gas at rest follows the damped oscillator with the radiation damping `rho c`
  within `1.2e-6` of the initial displacement;
* a light mass and a stiff spring whose step limits are a third and a sixth of the CFL step run stably at
  `max_dt()` and reflect a pulse like their damper and like a rigid wall (differences `8e-6` and `4e-5`); without the
  limit both runs fail;
* `m = 0`, `k = 0` against `CompBc::Impedance`: 1400 steps bit-identical.

Limits:

* One degree of freedom per face and no coupling between faces: no plate bending, no membrane, no cavity, no fit of a
  measured impedance curve by several poles.
* The mesh face does not move: the wall velocity is a transpiration velocity (displacements small against the cell size
  and the wavelength). Mass crosses the face and returns when the wall returns, so the totals of the solver are not
  conserved to round-off with a wall.
* Checked at normal incidence only; gas at rest; Cartesian grids.
* Not together with a sponge layer, and not on moving meshes (`step_ale`, `replace_mesh` and `set_sponge` return an
  error while a wall is set).

## Sponge layer of the compressible solver (F-331, ADR 0237; library only)

A sponge layer is a band of cells in front of a boundary in which the solution is driven back to a reference state, so
that waves are absorbed before they reach the boundary. It is opt-in (`CompressibleSolver::set_sponge`, `rcfd-fv`
`sponge.rs`); without it the solver produces the same bits as before. It is reachable from Rust, not yet from a case
file. Every check with its limit and measured value is in
[ADR 0237](adr/0237-f331-registration-sponge-layer-of-the-compressible-solver.md).

| Form | What it does | Reflection of a plane wave at the angle theta |
|---|---|---|
| `SpongeForm::Plain` | every conserved variable relaxes to the reference: `S = -rate (U - U_ref)` | none at normal incidence; the long-wave part of an oblique wave is reflected by up to `(cos - 1)/(cos + 1)`, as by the matched wall |
| `SpongeForm::Matched { normal }` | mass, energy and the momentum along `normal` relax; the tangential momentum is not damped but forced by `-rate grad_t Q`, `dQ/dt = p - p_ref` (a perfectly matched layer of linear acoustics about a state at rest) | the reflection of the boundary behind the layer times `R0^cos(theta)` |

How to size a layer: choose the thickness `L` (40 cells in the checks; the layer has to be resolved, not to span a
wavelength), the exponent 2 and the two-way attenuation `R0` at normal incidence, and take the peak rate from
`rate_max_for_reflection(L, c, R0, 2.0)` = `3 c ln(1/R0)/(2 L)`. `SpongeLayer` holds the plane, the thickness and the
profile `rate_max (d/L)^2`; `Sponge::planar(geom, reference, &layer, matched)` builds the rates. With `R0 = 1e-6` the
matched form leaves `1e-3` of a wave at 60 degrees even in front of a rigid wall. Keep `rate_max dt` well below 2.5 (the
source is explicit and `max_dt` does not know it; the checks run at 0.10).

Which form: the plain form for a flow that leaves through the layer (it damps vortices as well, and its reference may
have a velocity) and for waves near normal incidence; the matched form for sound arriving obliquely on a medium at rest.

What was checked (Cartesian grids, gas at rest, `R` = largest difference to a run on an extended grid against the
incident peak; limit 1 percent):

* normal incidence, a pulse in a tube: `9e-7` in front of the Thompson boundary, `1.1e-6` in front of a rigid wall, in
  both forms (the rigid wall alone: 0.93);
* one isolated plane pulse above a rigid wall, matched form: `2.4e-5` at 30 degrees, `1.4e-4` at 45 degrees;
* trains of plane pulses in a rigid duct, matched form, started in the layer's own steady state: `1.2e-4`, `2.8e-4`,
  `1.2e-3` at 30, 45 and 60 degrees (without the layer the Thompson boundary reflects 0.081, 0.196, 0.372; the plain
  form at 45 degrees 0.150);
* a uniform state equal to the reference does not drift (`1e-16`); a uniform offset decays as `exp(-rate t)` within
  `7e-7`.

Limits:

* The matched form is one planar layer with a reference at rest, derived from linear acoustics. There is no treatment of
  a corner where two layers would meet and none for a mean flow through the layer.
* The matched form does not damp waves that travel along the layer. In a duct with rigid walls a transient inside the
  layer (for example a start with a wave already in the layer and the memory `Q` at zero) leaves transverse standing
  waves behind that ring for a long time: 4 to 9 percent of the wave in the first, defective set-up of the checks. Start
  with a quiet layer, or set the memory with `set_sponge_memory`.
* The memory `Q` has no decay: a pressure that stays away from the reference with a gradient along the layer drives the
  tangential momentum without bound.
* The plain form at oblique incidence needs a thickness of many wavelengths (for 1 percent of a Gaussian pulse at 60
  degrees about 175 pulse widths).
* Not on moving meshes: `step_ale` and `replace_mesh` return an error while a sponge is set.
* An isolated pulse at 60 degrees was not run (about two million cells); 60 degrees is checked with pulse trains.

## Reference: code locations, input format and command line

### Modules and entry points

| Item | Where |
|---|---|
| FW-H 1A, moving impermeable surface, medium-frame (uniform flow) evaluation | `crates/rcfd-post/src/fwh.rs` (`Surface::impermeable`, `into_medium_frame`, `signal_moving`, `valid_window_moving`, `medium_frame_observer`) |
| Metrics (SPL, OASPL, A weighting, spectrum, 1/3-octave, sphere power, JSON/CSV) | `crates/rcfd-post/src/acoustics.rs` |
| Rotor harmonics (Gutin type) | `crates/rcfd-post/src/rotor.rs` |
| Proxies (Proudman, Curle, trailing edge) | `crates/rcfd-post/src/noise_proxies.rs` |
| Surface-series reader, builder, adapter from `surfaces::Surface` | `crates/rcfd-post/src/surface_series.rs` |
| CLI | `rcfd fwh` (`crates/rcfd-cli/src/cmd_fwh.rs`) |
| Impedance and quiet boundaries of the compressible solver | `crates/rcfd-fv/src/compressible_bc.rs` (`impedance_wall`), `CompBc::Impedance` |
| Mass-spring-damper wall of the compressible solver | `crates/rcfd-fv/src/compressible_bc.rs` (`MsdWall`, `piston_face`), `CompressibleSolver::set_msd_wall`, `msd_wall_state`, `set_msd_wall_state`, `msd_dt_limit` |
| Sponge layer of the compressible solver | `crates/rcfd-fv/src/sponge.rs` (`Sponge`, `SpongeForm`, `SpongeLayer`, `rate_max_for_reflection`), `CompressibleSolver::set_sponge`, `set_sponge_memory` |

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

```text
rcfd fwh series.json --observer x,y,z [--observer ...] [--flow ux,uy,uz] [--times t0:t1:n]
         [--out signal.csv] [--spectrum bands.csv|spectrum.json] [--narrowband file.csv]
         [--fmin 20] [--fmax 20000] [--window hann|rect]
```

Output CSV: `observer,t,p_thickness,p_loading,p_total`. Exit codes: 0 success, 1 I/O, 2 usage, 3 invalid input.
The default time grid is the full valid window at the file sampling step. A `.json` spectrum target writes an
`rcfd-spectra` v1 document (one `rcfd-spectrum` report per observer), anything else writes the 1/3-octave band CSV.
The command prints the OASPL and dB(A) per observer to stderr.

## Where to go next

* [reactor-and-acoustics.md](reactor-and-acoustics.md): the older permeable-surface command and its input file.
* [milestone-fan.md](milestone-fan.md): the fan milestone this chain serves.
