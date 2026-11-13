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
* No installation effects. Ducted propagation and modal content only through the separate wave solver and modal
  analysis of ADR 0351, which is not coupled to the FW-H chain.
* Pressure data sampled by the CFD run must resolve the blade-passing frequency and its harmonics (the FW-H result is
  only as good as the sampled surface pressure).
* A steady RANS run gives no tonal pressure history.

An acoustic impedance boundary, quiet boundary and acoustic wave solver are not part of this wave. The impedance and
quiet boundaries were added later as library code (section "Impedance and quiet boundaries of the compressible solver");
a linear acoustic wave solver with sponge layers and the modal analysis of duct fields were added as library code in
ADR 0351 (section "Acoustic wave solver and duct modes"). None of them runs inside a CFD run; the wave solver can be driven
from a section sampled into an `rcfd-surface-series` file (ADR 0372, section "Driving the wave solver from a sampled
surface").

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
* On moving meshes only when attached to the mesh (section "The sponge on a moving mesh" below); `replace_mesh`
  returns an error while a sponge is set.
* An isolated pulse at 60 degrees was not run in ADR 0237; ADR 0248 ran it (next section).

### The shifted matched form, corners, damping along the layer (F-340, ADR 0248; library only)

`SpongeForm::MatchedShifted(MatchedOptions { normal, shift, along, second })` is the matched form with three opt-in
additions; `Plain` and `Matched` are unchanged, bit for bit. Every check with its limit and measured value is in
[ADR 0248](adr/0248-f340-registration-matched-sponge-shift-corner-and-damping-along-the-layer.md).

| Option | What it does | What it costs |
|---|---|---|
| `shift` = `a` [1/s] (the complex frequency shift) | the memories decay, `dQ/dt = p - p_ref - a Q`: under a pressure that is held the memory and the force along the layer stay bounded (`p'/a`), and a wrong start of the memory is forgotten after a few `1/a` | the attenuation exponent is multiplied by `omega^2/(a^2 + omega^2)`: below `omega ~ a` the layer does not absorb, and it no longer pulls a static pressure to the reference. Keep `a` well below the lowest frequency of interest; the checks use `a = 0.05 c/L` (0.25/s) |
| `second` (a second planar layer, normal orthogonal to the first) | two layers that overlap in a corner absorb there as well (`Sponge::corner`); the same memory `Q`, one product term in mass and energy, the force of each layer on the momentum along the other's normal | nothing in the equations; one more memory for an edge in three dimensions (built, checked by algebra only) |
| `along` = `beta` (damping of the momentum along the layer, a fraction of the layer rate) | damps waves that run along the layer | **a mismatch**: it reflects about `1.3 beta` of a pulse arriving at 60 degrees, `0.34 beta` at 45, `0.10 beta` at 30 (model); `beta = 0.05` measured 4.4 percent of a train at 60 degrees. Leave it at zero unless the ringing matters more than the oblique reflection |

`Sponge::planar_shifted(geom, reference, &layer, shift, along)` builds one layer, `Sponge::corner(geom, reference, &layer1,
&layer2, shift, along)` two. A restart, or a start with a wave already inside the layer, sets the memories with
`set_sponge_memories` (`q`, `r` and the momentum memory `m`).

What was checked with `shift = 0.25`, `along = 0` on the 40-cell layer (limit 1 percent):

* normal incidence: `9.4e-7` in front of the Thompson boundary, `1.2e-6` in front of a rigid wall;
* one isolated plane pulse above a rigid wall: `2.3e-5` at 30 degrees, `1.4e-4` at 45 degrees, **`8.1e-4` at 60 degrees** (the
  matched form of ADR 0237 in the same run: `8.0e-4`);
* trains in a rigid duct started in the layer's steady state: `3.1e-4`, `3.6e-4`, `1.3e-3` at 30, 45, 60 degrees;
* a quarter of a cylindrical pulse leaving through two layers and their corner: `5.3e-5` (no sponge: 0.058; one layer
  only: 0.029);
* a closed box with a pressure offset that varies along the layer, 100 transit times of the layer: the momentum along
  the layer stays below `3.0 delta/c` and does not grow (the form of ADR 0237 in the same run: `3.2 delta/c`, no growth
  either);
* without the options the shifted form produces the bits of `Matched`.

What it does not do:

* **It does not remove the ringing of a rigid duct.** A wave train started inside the layer with zero memory still
  leaves 3.7, 6.1 and 8.6 percent at 30, 45 and 60 degrees in the window of the ADR 0237 set-up (2.2, 3.6, 4.2 percent
  with `along = 0.05`). The ringing sits at the cut-off frequencies of the duct; a field that does not vary across the
  layer solves the equations of any matched layer, so no matched layer damps it, and a damping along the layer strong
  enough to remove it within one or two periods reflects far more than 1 percent at 45 and 60 degrees. Start with a
  quiet layer or set the memories.
* Three layers meeting in a corner of a three-dimensional box are not built.
* As before: reference at rest, explicit source, not together with a mass-spring-damper wall, Cartesian grids only
  checked; on moving meshes only attached to the mesh (next section).

### The sponge on a moving mesh (F-341, ADR 0265; library only)

By default `step_ale` refuses a sponge, as before. `CompressibleSolver::set_sponge_frame(SpongeFrame::AttachedToMesh)`
attaches it to the mesh: the rates (of both layers of a corner) and the memories belong to the cells and move with them,
and `step_ale` adds the source at the volume of each Runge-Kutta stage, so the damping is consistent with the geometric
conservation law of the ALE step. The setting is kept across `set_sponge`. Every check with its limit and measured value
is in [ADR 0265](adr/0265-f341-registration-sponge-layer-on-moving-meshes.md).

* The reference state is a lab-frame state (the gas far away, for example at rest), whatever the mesh does; the matched
  forms need it at rest in the lab.
* The memory of a cell is the time integral of `p - p_ref` along the path of the cell; it does not cross faces.
* `replace_mesh` still refuses a sponge: the solver has no map from the old cells to the new ones, and a matched layer
  restarted with zero memory under a wave rings (ADR 0237). Clear the sponge, replace the mesh, set a sponge built on the
  new mesh and, if a wave is inside the layer, its memories mapped by the caller (`set_sponge_memory`,
  `set_sponge_memories`).
* Checked: a uniform state equal to the reference stays within `3.3e-15` on the deforming set-up of ADR 0207 in all
  three forms; the tube of ADR 0237 with both grids translating at Mach 0.2 reflects `9.6e-7` (plain and matched, in front
  of the Thompson boundary and in front of a pressure release that alone reflects fully) and `9.8e-7` (shifted form); a
  uniform offset decays as `exp(-rate t)` within `3.1e-7` while the cells change their volume by 16 percent; a sponge with
  all rates zero gives the bits of `step_ale` without a sponge.
* Limits: the layer is attached, not fixed in space (a layer that should stay while cells slide or rotate through it is
  not built); the matched forms are derived for a medium at rest in the frame of the layer, so on a moving mesh they are
  matched at normal incidence only (oblique incidence and a mesh moving along the layer are not checked; a mean flow
  along a matched layer can make it unstable); their `normal` does not rotate with a rotating mesh; one-dimensional and
  two-dimensional checks only.

### A mesh moving along a matched layer, and a rotating mesh (F-407, ADR 0344; library only)

With `AttachedToMesh` the memory of a matched layer is the integral of `p - p_ref` along the path of each cell. When the
mesh moves along the layer, that memory makes the layer unstable for waves grazing along it. Linear theory gives the
growth rate `rate M / (2 (1 - M))` when the rate is small against the wave number (ADR 0344 DR2). The one-dimensional
check grew a grazing pulse to 6 times its amplitude by `t = 1.5`, where the same column at rest stays at 0.30.

`SpongeFrame::AttachedWithLabMemory` keeps the rates attached to the cells but gives the matched forms the memories of the
lab frame: each cell's memory gains `w_c . grad Q` (and likewise `r` and `m` of the shifted form), `w_c` being the
velocity of the cell centre over the step. On a mesh translating along a planar layer this is exactly the matched layer at
rest. The plain form has no memory, so both attached frames give it the same bits, and so does a cell that does not move.

* Checked: the transport term against a closed form to `1.9e-13`. A uniform reference state stays within `3.5e-15` on
  the deforming set-up of ADR 0207. The grazing column stays bounded (0.31 against 0.30 at rest). On a rotating O-grid
  (Mach 0.2 at the layer), a plain radial layer reflects `2.8e-3` of an outgoing cylindrical pulse, the same as at rest.
  The plain form reflects about `w/(8 r)` of a cylindrical pulse of width `w` at radius `r`; that comes from the geometry,
  not from the rotation. The plane-pulse results at 30 and 45 degrees are in ADR 0344 addendum 1.
* Limits: the lab memory is derived for a translation along a planar layer. With a normal component of the motion, or
  with a rotation, the attached rates move through the lab, and the matched forms on a rotating mesh are still not built.

## Acoustic wave solver and duct modes (F-333, ADR 0351; library only)

`rcfd-fv` `acoustic_wave.rs` solves the linear acoustic equations about a medium at rest, `p_t + rho0 c^2 div u = 0`,
`rho0 u_t + grad p = 0`, on any finite-volume mesh (`AcousticWaveSolver::new(geom, Medium { rho0, c }, &bcs)`). It is the
small-amplitude limit of the compressible solver about a gas at rest, with two unknowns instead of five, so it carries
sound along a duct much more cheaply. It is not coupled to a CFD run: the sound enters through a driven boundary. Every
check with its limit and measured value is in
[ADR 0351](adr/0351-f333-registration-acoustic-wave-solver-and-modal-export.md).

* Scheme: the exact Riemann flux of the linear system, values reconstructed to the faces with an unlimited least-squares
  gradient, SSP-RK3 with the step chosen by the caller. `max_dt(cfl)` gives the step for the multi-dimensional number
  `c dt sum(1/dx)`; the checks run at 0.48.
* Boundaries per patch: `WaveBc::Wall` (hard wall or symmetry), `WaveBc::Open` (characteristic, ambient outside: quiet
  at normal incidence, `(1 - cos)/(1 + cos)` at the angle `theta`), `WaveBc::Driven(f)` (a ghost state `f(x, t)`, for
  a wave entering the domain). Every patch that is not empty must be named. Cyclic patches are refused.
* Sponge: `WaveSponge::from_layer(geom, &SpongeLayer, matched)` with the profile of the compressible sponge
  (`rate_max_for_reflection`). The plain form damps `p` and `u`. The matched form damps `p` and the normal velocity
  and drives the tangential velocity from a memory `Q_t = p`, which makes it reflection-free in the equations at every
  angle. Keep `rate_max dt` well below 2.5. As with the compressible matched layer, a field that does not vary along
  the layer's normal (the ringing of a rigid duct at a cut-off frequency) is not damped, so ramp the source.

`rcfd-post` `duct_modes.rs` does the modal analysis of hard-walled ducts:

* Modes: rectangular `sqrt(eps_m eps_n) cos(m pi y/a) cos(n pi z/b)`, `k_mn^2 = (m pi/a)^2 + (n pi/b)^2`; circular
  `J_m(j'_mn r/R) e^{i m phi}` normalised, `k_mn = j'_mn/R`. In the library the radial index `n` starts at 0 for every
  `m`: `(1, 0)` is the first spinning mode, `(0, 1)` the first radial mode. Every mode is normalised to an area mean of
  `|psi|^2` of 1. `axial_wavenumber(k, k_mn)` returns `sqrt(k^2 - k_mn^2)`, or `-i sqrt(k_mn^2 - k^2)` for a cut-off
  mode. `ModeBasis::below(shape, k)` lists the modes up to `k`. The Bessel functions and the zeros of `J'_m` are
  computed (Bessel's integral, bisection), not tabulated.
* `HarmonicDft` turns time series into complex amplitudes at one frequency, exactly when the samples cover a whole
  number of periods. `decompose_section` is a weighted least-squares fit of a sampled section (use the cell areas as
  weights). `split_axial` separates one mode's station amplitudes into the waves towards `+x` and `-x` for a given
  `k_z`. `fit_axial` also fits a complex `k_z` (it is how the checks measured the solver).
* Export: `ModalExport::section` or `ModalExport::axial` (with the power of each wave, `S |a|^2 Re(k_z)/(2 rho0
  omega)`), written as JSON by `to_json_string` and read back by `from_json_str`. Errors name the field.

```json
{"format": "rcfd-duct-modes", "version": 1, "kind": "axial",
 "duct": {"shape": "circular", "radius": 0.06}, "frequency": 2361.1, "sound_speed": 340.0, "density": 1.2,
 "x_ref": 0.6, "residual": 3e-5,
 "modes": [{"m": 1, "n": 0, "k_cut": 30.69, "k_axial": [29.4, 0.0], "plus": [0.98, -0.01], "minus": [1e-4, 2e-5],
            "power_plus": 1.2e-5, "power_minus": 1.3e-13}]}
```

What was checked (a 2-D channel of height 0.12 m, 32 cells across, mode 1 at `cos theta = 0.8`, 2361 Hz):

* axial wavenumber `0.19` percent below `sqrt(k^2 - k_1^2)` (the Fourier analysis of the scheme predicted the same);
  the wave loses `0.23` percent of its amplitude per axial wavelength; the plane wave is `0.22` percent slow;
* a matched layer of 40 cells (about one wavelength) in front of a rigid end reflects `9e-5` of mode 1, and `8e-5` on a
  sheared mesh; a plain layer reflects `1e-6` of the plane wave;
* the open end reflects `0.109` of mode 1 against `1/9` from the closed form; the rigid end without a layer `1.00`;
* a cut-off mode decays `0.17` percent faster than the closed form (64 cells across);
* a synthetic field of a rectangular duct (five propagating modes in both directions and one cut-off mode) goes through
  the DFT, the section fit, the axial split and the JSON file and comes back within `4e-15`; a circular section with
  two modes outside the fitted basis comes back within `1.3e-4`.

Limits: no mean flow, linear only, no impedance wall; the solver checks are two-dimensional channels (the `(m, 0)`
modes of a rectangular duct); circular ducts are checked in the modal analysis only; one planar layer, no corner; not
reachable from a case file or the CLI.

### Circular ducts through the wave solver (F-453, ADR 0366; library only)

`rcfd_mesh::gen::circular_duct_ogrid(n, n_r, n_z, radius, core, length)` meshes a circular duct along `z` with the usual
O-grid: an `n x n` Cartesian core of half-side `core * radius` and four ring blocks of `n x n_r` cells out to the circle
(patches `wall`, `inlet`, `outlet`). The section is the inscribed polygon of `4 n` sides. Its cut-offs are those of the
circle of equal area to first order, so `k_cut,poly = j'_mn sqrt(pi / A_N)`, `A_N = (N/2) R^2 sin(2 pi/N)`. At 64 sides
that moves the axial wavenumber by `-4.5e-4` relative to the circle's closed form; at 128 sides by `-1.1e-4`.

What was checked (radius 0.06 m, each mode at `cos theta = 0.8`, a matched layer one axial wavelength thick in front of a
rigid end):

* the spinning mode `(1, 0)` at 2768 Hz on `n = 16` (110 k cells): axial wavenumber `0.26` percent below the
  polygon-corrected closed form (`0.30` percent below the circle's), and the layer reflects `3.6e-5`;
* the radial mode `(0, 1)` at 5760 Hz on `n = 32` (424 k cells): axial wavenumber `0.26` percent below the
  polygon-corrected closed form (`0.27` percent below the circle's), and the layer reflects `6.5e-5`. Both modes lose
  about `0.35` percent of their amplitude per axial wavelength;
* the Bessel zeros come from a power series in the test and agree with the library's integral form within `1.1e-14`;
* the solver commutes with the grid's rotation by 90 degrees to `1e-15`. The counter-rotating mode `(-1, 0)` still
  shows `1.2e-5` of the driven one, left over from the start-up ramp of the drive (a slower start or a longer run
  makes it smaller). The registered limit was `1e-6`, so this check is recorded as a known miss.

Limits: two modes at one frequency each; one O-grid topology; no rectangular three-dimensional duct; one planar layer;
not reachable from a case file or the CLI.

### Driving the wave solver from a sampled surface (F-452, ADR 0372; library only)

`rcfd-post` `wave_drive.rs` turns an `rcfd-surface-series` document (the pressure on a duct section, the FW-H chain's
format) into the ghost state of `WaveBc::Driven`. `SurfaceDrive::from_series(spec, medium, frame, basis, options)` takes:

* a `DuctFrame` (`origin` the zero of the section coordinates, unit `axis` into the wave domain, unit section axes `e1`,
  `e2`) and a `ModeBasis` of the duct;
* `DriveOptions { frequency, max_harmonic, ramp_time }`: the fundamental, the highest harmonic played back, the start-up
  ramp `sin^2(pi t/(2 T_r))`.

The series must cover a whole number of periods of the fundamental (otherwise it is refused: the DFT would leak the mean
and the harmonics); the harmonics `1..=max_harmonic` must lie below the Nyquist bin. Per harmonic the face amplitudes
come from a DFT on the file's clock, are fitted on the mode basis with the face areas as weights, and are played back
periodically as the wave entering the domain: each mode with its axial wavenumber and impedance `u = kz p/(rho0 omega)`
(a plane-wave impedance `p/(rho0 c)` would inject a mode at `cos theta = 0.8` 11 percent too strong), carried from the
sampling plane to the driven faces by `e^{-i kz (xi - xi_s)}`. The mean pressure is reported and not played back.
`ghost_state()` returns the `GhostState`; `state(x, t)` the unramped wave. Refused with a located error: a non-whole
number of periods, a harmonic at or above Nyquist, `rho0` or `c0` differing from the solver's, a flow or a rotating
motion, a face off the section plane, outside the duct or with a normal off the axis, more modes than faces, a bad frame.

Checked (0.12 m channel, a field of the plane wave at 1181 Hz and the plane wave and mode 1 at 2361 Hz plus a 3 Pa mean,
sampled on a wall-clustered 24-face section 0.02 m upstream of the wave mesh, through JSON text): on 64 cells across the
amplitudes measured at the inlet agree with the closed form within `8.8e-4`, `3.5e-3`, `2.3e-3` (`1.4e-2` at worst on 32
across; second order); the file-driven run equals the run driven by the closed form to `1.1e-14`; 127 and 96 samples of a
128-sample, two-period file are refused.

Limits: the sampled pressure must be the entering wave only (one section of pressure cannot separate a returning wave);
periodic signals only; a static section, medium at rest; only the channel was run (any `DuctShape` is accepted); not
reachable from a case file or the CLI.

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
| Sponge layer of the compressible solver | `crates/rcfd-fv/src/sponge.rs` (`Sponge`, `SpongeForm`, `SpongeLayer`, `rate_max_for_reflection`), `CompressibleSolver::set_sponge`, `set_sponge_memory`, `sponge_memories`, `set_sponge_memories`; the shifted form: `MatchedOptions`, `Sponge::planar_shifted`, `Sponge::corner` |
| Acoustic wave solver (linear, medium at rest) | `crates/rcfd-fv/src/acoustic_wave.rs` (`AcousticWaveSolver`, `Medium`, `WaveBc`, `WaveSponge`, `WaveSpongeForm`) |
| Duct modes, modal decomposition, axial split, `rcfd-duct-modes` export | `crates/rcfd-post/src/duct_modes.rs` (`ModeBasis`, `axial_wavenumber`, `HarmonicDft`, `decompose_section`, `split_axial`, `fit_axial`, `ModalExport`) |
| Wave solver driven by a sampled surface series | `crates/rcfd-post/src/wave_drive.rs` (`SurfaceDrive`, `DuctFrame`, `DriveOptions`, `HarmonicModes`) |

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
