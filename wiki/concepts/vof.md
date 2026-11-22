---
type: concept
tags: [multiphase, free-surface, vof]
sources: [adr-thread-vof-multiphase-particles]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# VOF (volume of fluid)

An interface-capturing method for two immiscible fluids: one transported volume fraction (alpha) per cell marks which fluid fills it, and the mixture density and viscosity follow from alpha. Interface compression keeps the interface sharp; surface tension enters as a body force from the interface curvature (continuum surface force, CSF, or a height-function curvature).

## How rcfd uses it

- F-055 built a vertical slice of VOF for two incompressible immiscible fluids with interface compression and surface tension (ADR 0049), checked on the Zalesak slotted disk, hydrostatics, the dam break and the rising bubble ([[adr-thread-vof-multiphase-particles]]).
- The multiphase pressure solve first used plain CG because AMG broke down on the density jump; ADR 0052 found a coarse-level diagonal of exactly 0 and enabled AMG above 2000 cells ([[adr-thread-vof-multiphase-particles]]).
- The VOF host also carries mass transfer, wave generation and absorption (ADR 0052) and the mixture / Schnerr-Sauer cavitation models (ADR 0056); it has no energy equation ([[adr-thread-vof-multiphase-particles]]).
- **Hysing rising bubble.** Under refinement the default algebraic VOF converged to a bubble about 10 % too slow (ADR 0100). The main cause was the component-averaged momentum diagonal, which makes a `Symmetry` plane act mostly like a no-slip wall (ADR 0103); the other suspect was the CSF curvature with a fixed number of smoothing passes, replaced by an opt-in height-function curvature (ADR 0106) with a CSF fallback (ADR 0108) ([[adr-thread-vof-multiphase-particles]]). See [[component-diagonal]].
- **Defaults since ADR 0114:** component diagonal, full viscous stress and height-function curvature with the CSF fallback are the library defaults, with `multiphase` keys for opt-out; the user lifted the bit-identical rule for this one change. F-055 stays partial: the reworded bubble clause misses on 160 x 320 by 0.0004 in y_c(3) and 0.0001 in V_c,max ([[adr-thread-vof-multiphase-particles]]). See [[bit-identical]], [[known-miss]].

## Related

- [[adr-thread-vof-multiphase-particles]], [[lagrangian-particles]], [[roadmap-multiphase-and-particles]]
- [[doc-case-schema-multiphase]], [[doc-reference-data-wave27a-vof]]
