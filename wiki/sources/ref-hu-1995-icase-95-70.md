---
type: source
tags: [reference, foundation, acoustics, absorbing-boundary, pml, linearized-euler, public-domain]
sources: []
origin: https://ntrs.nasa.gov/citations/19960007445
licence: NTRS determination GOV_PUBLIC_USE_PERMITTED (record API read 2026-11-22)
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Hu (1995), On absorbing boundary conditions for linearized Euler equations by a perfectly matched layer, ICASE 95-70

**Origin:** F. Q. Hu, "On absorbing boundary conditions for linearized Euler equations by a perfectly matched layer", ICASE Report 95-70, NASA CR-198224, October 1995 (NTRS lists it as a preprint sent to a journal). NTRS 19960007445. PDF: https://ntrs.nasa.gov/api/citations/19960007445/downloads/19960007445.pdf
**Kind:** ICASE report (NASA contractor report), 27 pages.
**Licence:** US Government work, public use permitted; full copy in [raw/hu-1995-icase-95-70.pdf](../raw/hu-1995-icase-95-70.pdf).
**Read:** 2026-11-22 by job p6-compressible-acoustics: abstract, sections 1-4 and 6, section 5 (numerical examples) skimmed. Equations partly garbled in the text layer. Locators are section and equation numbers.

## Summary

Proposes a perfectly matched layer (PML) for the 2D linearized Euler equations with a uniform subsonic mean flow: split equations with absorption coefficients that, in theory, absorb outgoing acoustic, vorticity and entropy waves with zero reflection at the interface, for every angle of incidence and frequency.

## Key points (own summary)

- **Why (section 1):** in computational acoustics the domain must be truncated, so "numerical non-reflecting or absorbing boundary conditions are needed so that the out-going waves are not reflected". Techniques listed: outflow conditions "based on the characteristics of the Euler equations", radiation conditions from far-field asymptotic solutions, and a buffer-zone technique "in which the mean flow is altered gradually to be supersonic in a buffer region adjacent to the artificial boundary"; PML is Berenger's electromagnetic technique carried over.
- **Linearized Euler equations (section 2, Eqs. 1.1-1.4):** for $u, v, p, \rho$ perturbations about a uniform mean flow of Mach number $M < 1$ along $x$, nondimensionalised by the mean density and speed of sound: $\partial u/\partial t + M\, \partial u/\partial x + \partial p/\partial x = 0$, $\partial v/\partial t + M\, \partial v/\partial x + \partial p/\partial y = 0$, $\partial p/\partial t + M\, \partial p/\partial x + \partial u/\partial x + \partial v/\partial y = 0$, $\partial \rho/\partial t + M\, \partial \rho/\partial x + \partial u/\partial x + \partial v/\partial y = 0$.
- **Three wave types (sections 1-3):** these equations "support acoustic waves, which travel with the speed of sound relative to the mean flow, and vorticity and entropy waves, which travel with the mean flow" (section 2; section 3.2: vorticity and entropy waves have the form $f(x/M - t, y)$ and convect at speed $M$).
- **PML equations (section 2, Eqs. 2.1-2.8):** each variable is split into two parts (e.g. $u = u_1 + u_2$), each with its own damping term $\sigma_x u_1$ or $\sigma_y u_2$; with $\sigma_x = \sigma_y = 0$ they reduce to the Euler equations, so the interior is a special case of the layer.
- **Absorption (section 3):** inside the layer, acoustic waves keep the dispersion relation of the interior and their amplitude decreases exponentially in $x$ (or $y$) when $\sigma_x$ (or $\sigma_y$) is non-zero; vorticity and entropy waves decay exponentially as well.
- **Zero reflection (section 4):** with $\sigma_y$ equal on both sides of an interface normal to $x$, the reflection coefficient for incident acoustic, vorticity and entropy waves is zero and the transmitted wave keeps direction and amplitude, "independent of the angle of incidence and frequency".
- **Numerics (section 5, conclusions):** the numerical reflection falls as the PML thickness grows; a far-field asymptotic boundary condition gave a larger reflection in the comparison case.

## Contradictions with the wiki

None found.

## Pages this source informs

[[non-reflecting-boundaries-and-sponge-layers]], [[characteristics-and-hyperbolic-systems]], [[acoustic-wave-equation]]
