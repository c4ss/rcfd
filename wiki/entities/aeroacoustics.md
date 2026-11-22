---
type: entity
tags: [acoustics, noise, fan, compressible]
sources: [doc-acoustics, doc-reactor-and-acoustics, doc-structural-solver, adr-thread-compressible-acoustic-boundaries, adr-thread-acoustic-wave-solver]
builds_on: [fw-h, rotor-tone-noise, sound-levels-and-metrics, duct-acoustic-modes, non-reflecting-boundaries-and-sponge-layers, acoustic-wave-equation]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Aeroacoustics chain (fan noise)

**What it is:** rcfd's library chain from a CFD run to a fan-noise estimate, plus the acoustic boundaries, sponge layers, linear wave solver, duct modes and structural radiation added around it (wave 32B, ADR 0066 and later) ([[doc-acoustics]]).
**Where:** `crates/rcfd-post/src/` (`fwh.rs`, `acoustics.rs`, `rotor.rs`, `noise_proxies.rs`, `surface_series.rs`, `duct_modes.rs`, `wave_drive.rs`, `solid_acoustic.rs`), `crates/rcfd-fv/src/` (`compressible_bc.rs`, `sponge.rs`, `acoustic_wave.rs`); CLI `rcfd fwh`, `rcfd acoustics`.

## Details

| Part | Status | Source |
|---|---|---|
| FW-H 1A, moving impermeable surfaces, uniform flow | verified on closed forms; W5a/W5b missed; see [[fw-h]] | [[doc-acoustics]] |
| Permeable FW-H command `rcfd acoustics` (F-136) | medium at rest, no quadrupole | [[doc-reactor-and-acoustics]] |
| Gutin-type rotor tones | derived, not transcribed; matches FW-H within 4e-5 | [[doc-acoustics]] |
| Broadband proxies (Proudman, Curle, trailing edge) | **NOT validated**; constants uncalibrated; "not predictions" | [[doc-acoustics]] |
| Metrics (SPL, OASPL, dB(A), spectra, 1/3-octave, sphere power) | `rcfd-spectrum` v1 | [[doc-acoustics]] |
| Impedance and quiet boundaries (F-243, ADR 0233) | library only; quiet only near normal incidence | [[doc-acoustics]] |
| Mass-spring-damper wall (F-332, ADR 0243) | library only; normal incidence checked | [[doc-acoustics]] |
| Sponge layers (F-331, F-340, F-341, F-407) | library only; matched forms reference at rest | [[doc-acoustics]] |
| Linear acoustic wave solver + duct modes (F-333, ADR 0351) | library only, not coupled to a CFD run | [[doc-acoustics]] |
| Circular ducts (F-453), surface drive (F-452), returning waves (F-478) | library only | [[doc-acoustics]] |
| Radiated sound of a vibrating structure (F-418, F-551, F-579) | baffled plane and ERP only | [[doc-acoustics]] |

- **Chain limits:** subsonic surfaces, no quadrupole, no refraction or scattering, uniform mean flow only, no installation effects; "A steady RANS run gives no tonal pressure history" ([[doc-acoustics]]).
- F-243 stays partial: wave 32B "does not address the acceptance text of F-243 (impedance tube, quiet boundary)" ([[doc-acoustics]]).
- **Sources read second-hand:** Proudman alpha = 0.1 (secondary source); A-weighting constants from secondary sources, the IEC 61672-1 standard not read; 1/3-octave labels not verified against IEC 61260-1; Gutin (1936) not read ([[doc-acoustics]]).
- **Validation path stated:** a fan or airfoil with measured operating point, wall-pressure data and far-field spectra; calibrate on one data set, compare on a held-out second, report an error band ([[doc-acoustics]]).
- **ERP** "is the only figure for a fan frame or housing"; it assumes radiation efficiency one ([[doc-acoustics]]).
- The structural solver's radiation impedance of a baffled plane is in `rcfd-post` (F-579) ([[doc-structural-solver]]).
- **Compressible acoustic boundaries (P3 ADR thread):** impedance boundary, plain and matched sponge layers (with memory decay, corners, moving meshes) and a mass-spring-damper wall on `CompressibleSolver`, all library only until the wiring item F-330 ([[adr-thread-compressible-acoustic-boundaries]]).
- **Linear acoustic wave solver** (`AcousticWaveSolver`, F-333) with duct modal export in `rcfd-post`, an O-grid circular duct (F-453), and a drive from sampled CFD surfaces (F-452, F-478) ([[adr-thread-acoustic-wave-solver]]).

## Builds on

Foundation pages for each part of the chain (added by p6-compressible-acoustics, 2026-11-22):

- [[fw-h]]: the surface integral that turns sampled blade pressures into far-field sound.
- [[rotor-tone-noise]]: blade-passing frequency, its harmonics and Gutin's tone formula, which the Gutin-type check reproduces.
- [[sound-levels-and-metrics]]: SPL, OASPL, A-weighting and bands of the `rcfd-spectrum` output.
- [[duct-acoustic-modes]]: spinning and radial modes and cut-off, for the duct modal export.
- [[non-reflecting-boundaries-and-sponge-layers]]: impedance, quiet boundaries and sponges of `CompressibleSolver`.
- [[acoustic-wave-equation]]: the linear acoustic equations of `AcousticWaveSolver`.
- Further down: [[lighthill-acoustic-analogy]], [[acoustic-source-types]], [[greens-function-and-retarded-time]], [[speed-of-sound-and-mach-number]], [[euler-equations]].

## Related

- [[fw-h]]
- [[rcfd-post]]
- [[rcfd-fv]]
- [[structural-solver]]
- [[unverified]]

## Open questions

- None of the boundaries, sponges or the wave solver runs inside a CFD run or from a case file ([[doc-acoustics]]).
- No comparison with a measured fan ([[doc-acoustics]]).
