---
type: source
tags: [reference, foundation, shared-roots, nasa, cite-only]
sources: []
origin: https://www.grc.nasa.gov/www/k-12/airplane/nseqs.html
licence: none stated on the pages (NASA web pages; footer links NASA privacy statement, disclaimer and accessibility notices only)
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# NASA Glenn, *Beginner's Guide to Aeronautics* (three pages)

**Origin:** NASA Glenn Research Center, Beginner's Guide to Aeronautics. Pages read: "Navier-Stokes Equations" https://www.grc.nasa.gov/www/k-12/airplane/nseqs.html (editor and NASA Official Nancy Hall, last updated May 13, 2021); "Reynolds Number" https://www.grc.nasa.gov/www/k-12/airplane/reynolds.html (no date: the page shows an unrendered `LAST_MODIFIED` placeholder); "Mach Number" https://www.grc.nasa.gov/www/k-12/airplane/mach.html (Nancy Hall, May 13, 2021).
**Kind:** educational web pages (entry level).
**Licence:** no licence or copyright statement on the pages. NASA work, but no "public use" statement was seen, so cite only.
**Read:** 2026-11-22, by job p6-roots.

## Summary

Entry-level statements of what the Navier-Stokes equations contain and of what the Reynolds and Mach numbers compare.

## Key points by page

### "Navier-Stokes Equations"
- The 3D unsteady equations are shown as five parts: continuity (mass), x, y and z momentum, energy. Dependent variables pressure, density, temperature (through total energy) and velocity components `u, v, w`; independent variables `x, y, z, t`.
- Convection terms (left-hand side) are transport of a property by the ordered motion of the flow; the pressure appears as a gradient (e.g. `-dp/dx`); the viscous stress tensor terms, scaled by the inverse Reynolds number, are the diffusion terms; the stress tensor has nine components.
- The page calls them a "coupled system of equations", too difficult to solve analytically, hence CFD. The Euler equations are Navier-Stokes without the viscous terms and cannot model boundary layers. In CFD the stress terms are often approximated by a turbulence model.

### "Reynolds Number"
- `Re = r V L / mu = V L / nu` (`r` density, `V` velocity, `L` length scale, `mu` dynamic viscosity, `nu = mu / r`): ratio of inertial forces to viscous forces.
- Equal similarity parameters in two experiments model the relative importance of the forces correctly. Around `Re = 10 million` viscous forces are small and the Euler equations can model the flow; around `Re = 100` viscous forces must be considered.

### "Mach Number"
- `M` = object speed / speed of sound in the gas; it indicates how strongly compressibility affects the flow.
- Regimes: subsonic `M < 1` (compressibility can be ignored at the lowest speeds); transonic near 1; supersonic `1 < M < 3`, high supersonic `3 < M < 5` (aerodynamic heating); hypersonic `M > 5`.
- At low speeds, "roughly below 250 mph", air density stays essentially constant. No Mach threshold such as 0.3 is given on this page.

## Contradictions with the wiki

None.

## Pages this source informs

[[navier-stokes-equations]], [[dimensionless-numbers]], [[incompressible-flow]]
