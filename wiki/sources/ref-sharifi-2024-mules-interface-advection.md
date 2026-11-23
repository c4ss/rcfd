---
type: source
tags: [reference, foundation, multiphase, vof, cc-by]
sources: []
origin: https://arxiv.org/abs/2410.16754
licence: CC BY 4.0 (arXiv licence link, read 2026-11-22)
read: 2026-11-22
raw: raw/sharifi-2024-mules-interface-advection.pdf
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Sharifi (2024), MULES algebraic scheme for sharp interface advection

**Origin:** M. Sharifi, "Efficient Implementation of an Accurate Algebraic Scheme for Sharp Interface Advection in Multiphase Flows", *J. Math. Techniques Comput. Math.* 3(10), 01-20 (2024); arXiv:2410.16754. Copy: [raw/sharifi-2024-mules-interface-advection.pdf](../raw/sharifi-2024-mules-interface-advection.pdf).
**Kind:** research paper (two-column layout; the body text extracted badly, so only the abstract and legible introduction sentences are used).
**Licence:** CC BY 4.0.
**Read:** 2026-11-22 by job p6-multiphase-heat: abstract and sec. 1 "Introduction" (with Figure 1, the taxonomy of two-phase methods). Sec. 2-4 could not be read reliably from the extraction.

## Summary

Tests the MULES algebraic VOF scheme with an interface compression (IC) coefficient between 0 and 2, against upwind, central, van Leer and geometric VOF, on a droplet in a 2D vortex and a stationary droplet.

## Key points by section

### Abstract
- Increasing IC improved interface tracking but caused non-physical instabilities at higher values: the interface advection error fell "from 4.8% to 3.95% as IC increased from 0 to 2", favourable "until IC surpassed 1.4, where IAE fluctuated around 4%", while the mass conservation error "rose steeply from 0% to 23.19%, driven by parasitic currents and numerical instabilities".
- Smoothing (filtering) the volume fraction before the curvature: one filtering step reduced the capillary number of the spurious flow by 73.31 %; more filtering reintroduced errors; the maximum parasitic velocity first fell by almost 75 % and then rose by 30.92 %.

### Sec. 1 "Introduction"
- Figure 1 sorts two-phase methods into interface-capturing and interface-tracking classes (front tracking is tracking; VOF, level set, phase field are capturing).
- In VOF "the interface delineation occurs implicitly through the allocation of volume fractions" to cells.
- VOF splits into geometric schemes, which reconstruct the interface from the volume fractions, and algebraic schemes, which avoid reconstruction and get the fluxes algebraically; algebraic schemes are simpler, more efficient and adapt to unstructured meshes, geometric ones need "a more cumbersome implementation process and slower execution". Algebraic VOF has two groups: THINC (hyperbolic-tangent profile within the cell) and compressive schemes, which add an artificial compression term to the advection equation acting normal to the interface.
- Deshpande et al. found parasitic currents caused by interface compression in surface-tension-dominated flows; parasitic currents arise from errors in the interface curvature or an imbalance between the surface tension and pressure gradient forces, typically for static bubbles and droplets at high density ratios. Hoang et al. found the best balance between parasitic flow and numerical diffusion at an IC coefficient of 1.

## Contradictions with the wiki

None.

## Pages this source informs

[[two-phase-flow-basics]], [[volume-fraction-transport]], [[interface-compression]], [[surface-tension-and-curvature]]
