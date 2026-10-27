# Fan measurement protocol (computer fans)

Status: protocol (2026-10-26, wave 46E, part G2 input). The survey of open small-fan data in [validation data](validation-data.md)
found no open source adequate as a computer-fan reference case. This page lists what a fan measured by the user must supply so that the
computer-fan tools of the [computer-fan project breakdown](fan-project-breakdown.md) can be checked against it. It is a minimum
protocol, not a standard; where a standard applies it is named, and standards not read for this page are marked so.

Scope: one axial or centrifugal fan of 40 to 140 mm, 1 000 to 5 000 rpm. Accuracy targets follow from the rule that each
measurement's standard uncertainty should be at most one third of the comparison tolerance the tools will be held to (so that a
disagreement is the model's, not the rig's). The tolerance is not set yet: this page assumes 5 % on pressure rise and flow, so about
1.7 % per measurement; the limit registered in the ADR of the first comparison replaces this assumption, and the targets scale with it.

What you produce: the fan's geometry, its fan curves, and, if you can, one system impedance curve, noise and vibration, delivered as
CSV files with a plain-text rig description (see "Data delivery" at the end). Work through the tiers below in order: geometry and the fan
curve are required, the system impedance curve is strongly recommended, noise is recommended and vibration is optional.

## What this does not cover

Thermal effects (motor heating of the air), long-term ageing and dust (F2, F3), psychoacoustic metrics beyond tonality, and
structural modes of the blades (E5) are not part of this protocol.

## What the measurements validate

| Measurement | Checks |
|---|---|
| Geometry, tip gap, frame | A1 parameter fit, A2 constraints, A5 tolerance spread (blade-to-blade variation) |
| Fan curve at several speeds, electrical power | C4 metrics, C5 sweep and operating point, C7 reduced-order tier, fan-law scaling |
| One system impedance curve | C5 operating point against a real heat sink, filter or grille |
| Sound power and narrowband spectra per operating point | D1 tones at blade-passing harmonics, D3 broadband proxy, D4 A-weighting and tonality |
| Vibration, rotor mass, residual unbalance | E1 imbalance force and balance grade, E2 later |

## Tier 0: identification and geometry (required)

- Fan make, model, nominal voltage, speed control (DC voltage or PWM), bearing type, blade count, rotation direction.
- Rotor: tip diameter, hub diameter, blade sections at no fewer than 7 radii from hub to tip (chord, stagger, camber, thickness, leading
  edge position, so sweep and lean follow). Preferred: a 3D scan (structured light or CT) with a stated accuracy of 0.05 mm or better on
  the blade surface, which is 2.5 % of a 2 mm blade thickness; supply the raw scan (STL or point cloud) as well as the sections.
- Tip gap: feeler gauges or the scan, at no fewer than 4 circumferential positions and for every blade, because the gap and its spread
  dominate small-fan losses and are an A5 tolerance input.
- Frame: inner diameter along the axis, inlet and outlet shape (a square frame uncovers part of the blade tip), strut count, shape and
  axial distance from the rotor, motor hub shape.
- Rotor mass (scale, 0.1 g) and, if available, the residual unbalance from a balancing machine.
- Option that removes most geometry error: print a rotor generated from `rcfd_mesh::fan` parameters (NACA 4-digit or circular-arc
  sections on cylinders) and fit it to an existing frame and motor. The geometry is then known by construction; measure the printed blade
  at 3 sections and report the deviation from nominal, and report the surface roughness.

## Tier 1: fan curve (required)

- Rig: a plenum (chamber) test in the style of ISO 5801 or ANSI/AMCA 210 (NOT read for this page), or the small-fan plenum of
  ISO 10302-1:2024 (catalogue summary seen, NOT read). Flow measured with a calibrated nozzle, orifice or bellmouth; fan static pressure
  as chamber pressure against ambient, with the side (inlet or outlet chamber) stated. A rig built from the open descriptions in the
  NASA and KIT sources listed in [validation data](validation-data.md) is acceptable if its own calibration is reported.
- Points: from free delivery to shut-off, at least 10 points per speed, denser near the pressure plateau (stall onset); at least 3
  speeds spanning the use range (for example 1 000, 1 500 and 2 000 rpm for a 120 mm fan).
- Speed: measure rpm at every point (optical tachometer or the tach wire counted over at least 10 s), because rpm drifts with load.
  Pressure scales with the square of speed, so a 0.25 % speed uncertainty gives 0.5 % in pressure: target 0.25 %.
- Pressure: target 1 % of reading plus 0.5 Pa. The scale is rho U_tip^2: about 190 Pa for 120 mm at 2 000 rpm (U_tip = 12.6 m/s),
  130 Pa for 40 mm at 5 000 rpm, 65 Pa for 140 mm at 1 000 rpm; the fan pressure rise is a fraction of this, and 0.5 Pa is 0.8 % of the
  smallest case. A transducer of 0 to 250 Pa range or a calibrated micromanometer is needed; record the zero before and after each speed.
- Flow: target 1.5 % of reading (from the assumed 5 % tolerance).
- Ambient: pressure (0.5 kPa), temperature (1 K) and relative humidity (5 %) at the start and end of each speed, so that density is known
  to about 0.5 %.
- Electrical input: supply voltage and current at every point (1 % each). With PWM speed control the current is pulsed: either run the
  fan from a variable DC supply without PWM, or sample a shunt fast enough to average over the PWM period (the PWM frequency is
  fan-specific; check it). The reported efficiency is then wire-to-air (motor and electronics included), as in the NASA sources.
- Torque is optional and usually impractical: with illustrative values (pressure rise 0.1 rho U_tip^2 = 19 Pa, 0.03 m^3/s, shaft
  efficiency 0.3) a 120 mm fan at 2 000 rpm needs about 1.9 W of shaft power, about 9 mN m of torque. Without torque, shaft power is not
  validated; say so in any comparison.
- Repeatability: repeat 3 points of one speed 3 times, and once after removing and remounting the fan.

## Tier 1b: one system impedance curve (strongly recommended)

Measure the pressure drop against flow of one real system part (heat sink, dust filter or grille) on the same rig, driven by an
auxiliary fan, over the flow range of the fan curve. Then measure the fan mounted on that part and record the operating point. This is
the quantity a computer-fan user cares about and the one C5 predicts.

## Tier 2: noise (recommended)

- Sound power at every curve point of at least one speed (noise changes with load, rising sharply in stall). Preferred method: the
  ISO 10302-1 plenum in a hemi-anechoic or reverberation room (NOT read for this page). ECMA-74 (free; read in part) gives a
  reproducibility standard deviation of about 1.5 dB for A-weighted sound power; that is the target.
- Microphones: class 1 sound level meter or measurement microphones, calibrated with an acoustic calibrator before and after each
  session; at least 6 fixed, documented positions on a hemisphere of radius 1 m or more.
- Background noise: at least 10 dB below the fan in every band used (adds at most 0.41 dB, from 10 log10(1 + 10^-1)); 6 dB is the floor
  (0.97 dB), reported with the correction applied.
- Spectra: record the raw signals (48 kHz or more, 24 bit, at least 30 s per point) together with a once-per-revolution tach channel on
  the same recorder, so that blade-passing tones can be order-tracked and ensemble-averaged (the synchronous-recording recommendation of
  NASA TM-2006-214450). Blade-passing frequency is B n / 60 (233 Hz for 7 blades at 2 000 rpm); resolve shaft-rate harmonics too
  (33 Hz at 2 000 rpm), so use a narrowband resolution of 2 Hz or finer. Report A-weighted and unweighted third-octave sound power and the
  ECMA-74 Annex D tone-to-noise and prominence ratios of the blade-passing tones.

## Tier 3: vibration (optional)

- Mounting stated and repeatable (soft mounts of stated stiffness, or bolted to a massive block); accelerometers on the frame in two
  radial directions and the axial direction, of a mass that is negligible against the frame (report it).
- Report velocity spectra to at least 4 times the blade-passing frequency, the 1x shaft-rate amplitude and phase against the tach, and
  the RMS velocity, at the speeds of Tier 1.
- Residual unbalance U (g mm) from a balancing machine or a trial-mass test (add a known mass at a known radius, measure the change of the
  1x response). The E1 check is the force U omega^2: 1 g mm at 2 000 rpm is 0.044 N; the NASA Quiet Space Fan's 0.762 g mm at 12 000 rpm
  is 1.2 N.
- ISO 10302-2 (structure-borne vibration of small air-moving devices) applies; title only, NOT read.

## Data delivery

One CSV per speed, SI units in the header, one row per point, plus a plain-text description of the rig, instruments (make, model,
range, calibration date, stated accuracy) and any deviation from this protocol. Example header:

```text
point,rpm,flow_m3_s,dp_static_Pa,dp_side,supply_V,supply_A,p_amb_Pa,T_amb_K,rh,lwa_dB,notes
```

Raw audio and vibration files are named by point. State the licence under which the data may be used: under CC BY 4.0 (or the user's
choice) the files can be pinned with checksums in `validation-data/manifest.json` like the other datasets; without a stated licence they
stay on the user's machine and only the comparison is recorded.

## Where to go next

- [Computer-fan project breakdown](fan-project-breakdown.md) for the parts each measurement checks.
- [Validation data](validation-data.md) for the open sources and rigs that were read, and the standards named on this page.
