---
type: source
tags: [reference, foundation, turbomachinery, periodic, public-domain]
sources: []
origin: https://ntrs.nasa.gov/api/citations/19820018354/downloads/19820018354.pdf
licence: NTRS determination GOV_PUBLIC_USE_PERMITTED (record API, read 2026-11-22). Copyable; not copied to raw/ (8 MB scan; left for the lead).
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Thompkins (1982), A FORTRAN program for three-dimensional inviscid rotational flows with shock waves in axial compressor blade rows: user's manual, NASA CR-3560

**Origin:** W. T. Thompkins, Jr., "A FORTRAN Program for Calculating Three-Dimensional, Inviscid, Rotational Flows With Shock Waves in Axial Compressor Blade Rows: User's Manual", NASA Contractor Report 3560, Massachusetts Institute of Technology for NASA Lewis Research Center, grant NSG-3234, June 1982. NTRS 19820018354.
**Kind:** NASA contractor report (scanned; OCR text).
**Licence:** US Government public use permitted.
**Read:** 2026-11-22 by job p6-linear-rotating: only the section "Periodic Boundaries" (report p. 35) and the grid-description paragraph before it; equations in the OCR were garbled.

## Summary

User's manual of a time-marching Euler code for a single compressor blade row in cylindrical coordinates $(R, \theta, X)$; only its periodic-boundary treatment was read here.

## Key points

- **Grid section (before "Periodic Boundaries"):** the grid is offset (staggered) so that boundaries lie midway between grid lines, which simplifies hard-wall and periodic boundaries; blade-to-blade grid lines stay at constant $(r, z)$ because the radial and axial stretchings are independent of $\theta$.
- **"Periodic Boundaries", p. 35:** the flow is assumed periodic blade to blade; with the staggered grid, periodicity is imposed "simply by using the fact that the conditions at k=1 are those of k=NTH-1 and conditions at k=NTH are those of k=2", i.e. the ghost layer on one side of the passage takes the values one pitch away on the other side; for the non-staggered grid of fig. 9 the periodicity is maintained by averaging across the seam (equation garbled in the OCR).

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[periodic-and-cyclic-boundaries]]
