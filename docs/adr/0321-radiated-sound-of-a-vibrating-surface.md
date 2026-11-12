# ADR 0321: Radiated sound of a vibrating surface: equivalent radiated power and the Rayleigh integral of a baffled plane

Status: **PLANNED 2026-11-12** (layout by the advanced-tier plan, [ADR 0310](0310-structural-solver-advanced-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are the
acceptance draft that the registration turns into checks.

Work package SA-11 of the structural solver, advanced tier. Size S.

## Goal

The housing's vibration is heard. From the harmonic velocity of a surface set (ADR 0302 or 0315): the equivalent radiated power
(rho c/2) sum A |v_n|^2 for any surface, as level spectra through `rcfd-post::acoustics`; and for a plane radiator in a baffle the
Rayleigh integral: pressure at field points, directivity, radiated power and radiation efficiency. No acoustic field solve and no
reaction of the air on the structure.

## Dependencies

ADR 0311; roadmap items (SA ids are this tier's packages, ADR 0310 section 7): SA-1, F-136, F-399.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. Equivalent radiated power: a rigid piston gives rho c S v^2/2
within 1e-12 and a plate mode sin sin gives rho c A v0^2/8 within 0.5 percent. Rayleigh integral of a circular piston of radius a:
on-axis pressure 2 rho c v |sin(k (sqrt(z^2 + a^2) - z)/2)| within 1 percent for z from a to 20 a at k a = 2 and 10 with faces of
a tenth of a wavelength; far-field directivity |2 J1(k a sin theta)/(k a sin theta)| within 1 percent; radiated power over the
equivalent radiated power equal to 1 - J1(2 k a)/(k a) within 1 percent at k a = 0.5, 2 and 10, and at k a = 0.1 the power rho c
k^2 (S v)^2/(4 pi) within 2 percent. Levels: 1e-12 W gives 0 dB and the third-octave sums equal the narrow-band total within
1e-10. A surface that is not plane within the registered tolerance is refused by the Rayleigh integral with its deviation; the
equivalent radiated power accepts any surface and its report says that it assumes a radiation efficiency of one.

## Reference to check against

Closed forms of the baffled circular piston: on-axis pressure, directivity and radiation resistance (Rayleigh, The Theory of
Sound, section 278 and following; Kinsler and Frey, Fundamentals of Acoustics; from memory, derivable from the Rayleigh integral).
Levels and bands are those of `rcfd-post::acoustics` (ADR 0066).

## Files

`crates/rcfd-post/src/solid_acoustic.rs`, `crates/rcfd-post/tests/sa_solid_acoustic.rs`.

## What could make it miss

A fan frame is not a baffled plane: for it only the equivalent radiated power is available, which over-predicts below coincidence.
Feeding the surface to the FW-H integral of F-136 was considered and left out: the impermeable form needs the surface pressure of
the radiated field, which is not known without an acoustic solve.
