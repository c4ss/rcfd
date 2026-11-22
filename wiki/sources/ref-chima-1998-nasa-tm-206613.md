---
type: source
tags: [reference, foundation, turbomachinery, mixing-plane, rotating]
sources: []
origin: https://ntrs.nasa.gov/api/citations/19980017287/downloads/19980017287.pdf
licence: NTRS determination GOV_PUBLIC_USE_PERMITTED (record API, read 2026-11-22), but the paper itself carries "Copyright (c) 1998 by the American Institute of Aeronautics and Astronautics, Inc. No copyright is asserted in the United States under Title 17, U.S. Code. ... All other rights are reserved by the copyright owner." Mixed statements, so cite only.
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Chima (1998), Calculation of Multistage Turbomachinery Using Steady Characteristic Boundary Conditions, NASA TM-1998-206613

**Origin:** R. V. Chima, "Calculation of Multistage Turbomachinery Using Steady Characteristic Boundary Conditions", NASA/TM-1998-206613, NASA Lewis Research Center; also AIAA paper for the 36th Aerospace Sciences Meeting, Reno, January 1998. NTRS 19980017287.
**Kind:** NASA technical memorandum / conference paper. No section numbers; locators are the paper's headings.
**Licence:** see frontmatter; cite only.
**Read:** 2026-11-22 by job p6-linear-rotating: introduction, boundary conditions, averaging techniques, results summary.

## Summary

An averaging-plane ("mixing-plane") method for steady 3-D viscous multistage turbomachinery in the SWIFT multiblock code: circumferentially averaged flow is passed between blade rows at a common grid interface, with Giles' non-reflecting characteristic boundary conditions written as perturbations about the neighbouring row's average, and two averaging techniques (mixed-out and kinetic-energy averages). Applied to the two-stage SSME fuel turbine.

## Key points

- **Introduction, methods for multistage flow:** (1) successive analysis of isolated blade rows, (2) averaging-plane methods, (3) the average-passage method, (4) full unsteady methods. Averaging-plane methods pass spanwise distributions of circumferentially averaged quantities at a common grid interface; "since averaging-plane methods often use mixed-out averages, they are commonly referred to as mixing-plane methods"; introduced by Denton and Dawes. They keep spanwise consistency but share the modelling issues of the boundary conditions, the averaging and missing (unsteady) physics. Full unsteady methods are expensive and often modify blade counts to small integer ratios.
- **Introduction, boundary conditions:** forcing the flow to be axisymmetric at the interfaces is simple and robust but reflects outgoing waves, hindering convergence and contaminating the solution; Giles' theory decomposes the boundary solution circumferentially into Fourier modes, the zeroth (mean) mode treated by 1-D characteristic theory.
- **Interface boundary condition (fig. 1):** the grids of neighbouring rows overlap by one cell; after each update the solution next to the boundary is integrated circumferentially at each spanwise station and the average stored for the neighbour's boundary condition. The interface "does not guarantee conservation between blade rows", conserving mass and energy about as well as the scheme does inside the rows; the degree depends on the averaging.
- **Averaging techniques:** any two thermodynamic and three kinematic properties may be integrated to define an average state; information is lost, so one chooses what to keep. The mixed-out average conserves mass, momentum and energy; derived by integrating the 2-D Euler equations across the pitch, where periodicity makes the integral of the circumferential fluxes zero, so the averages represent the fully mixed-out flow far downstream and may introduce mixing losses prematurely. The kinetic-energy average conserves mass and total enthalpy and represents the local state, ignoring downstream mixing loss.
- **Results:** mass-flow error under 1 percent with the kinetic-energy average (jumps at each averaging plane) and under 0.1 percent with the mixed-out average.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[frozen-rotor-and-mixing-plane]], [[periodic-and-cyclic-boundaries]], [[non-conformal-interfaces]]
