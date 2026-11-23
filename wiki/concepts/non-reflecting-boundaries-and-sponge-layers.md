---
type: concept
tags: [foundation, acoustics, compressible, boundary-conditions, sponge-layer]
sources: [ref-hu-1995-icase-95-70, ref-hubbard-1991-nasa-rp-1258-vol2, ref-chima-1998-nasa-tm-206613]
builds_on: [characteristics-and-hyperbolic-systems, acoustic-wave-equation]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Non-reflecting boundaries and sponge layers

## In short

A computational domain has to end somewhere, but sound and other waves should leave it as if the domain went on forever. A plain boundary condition reflects part of each outgoing wave back into the solution. Non-reflecting (absorbing) boundaries reduce that reflection, either by a condition at the boundary itself (characteristic or impedance conditions) or by a layer of extra cells where the waves are damped before they reach the boundary (sponge layers, buffer zones, perfectly matched layers) ([[ref-hu-1995-icase-95-70]] sec. 1).

## Explanation

**The problem.** "In problems involving wave radiation and convection, such as in computational acoustics, the physical domains are often necessarily truncated ... Thus, at these artificial boundaries, numerical non-reflecting or absorbing boundary conditions are needed so that the out-going waves are not reflected" ([[ref-hu-1995-icase-95-70]] sec. 1). The waves to let out are those of the linearised Euler equations: acoustic waves at the speed of sound relative to the flow, and vorticity and entropy waves convected with it ([[acoustic-wave-equation]], [[ref-hu-1995-icase-95-70]] sec. 1).

**Families of methods** listed by Hu ([[ref-hu-1995-icase-95-70]] sec. 1):

- outflow conditions "based on the characteristics of the Euler equations" ([[characteristics-and-hyperbolic-systems]]): prescribe the incoming waves, let the outgoing ones pass;
- radiation conditions based on far-field asymptotic solutions;
- a buffer-zone technique "in which the mean flow is altered gradually to be supersonic in a buffer region adjacent to the artificial boundary", so that no wave can travel back;
- the perfectly matched layer (PML), carried over from Berenger's electromagnetic method.

In turbomachinery, Giles' conditions treat each circumferential Fourier mode of the boundary solution separately, the mean mode by 1-D characteristic theory; forcing an axisymmetric boundary instead "reflects outgoing waves, hindering convergence and contaminating the solution" ([[ref-chima-1998-nasa-tm-206613]] introduction).

**Absorbing layers.** In a sponge or matched layer the equations get an extra damping term. In Hu's PML each variable is split in two (for example $u = u_1 + u_2$), each with its own absorption coefficient ([[ref-hu-1995-icase-95-70]] sec. 2, eqs. 2.1-2.8):

$$\frac{\partial u_1}{\partial t} + \sigma_x u_1 = -\frac{\partial (p_1 + p_2)}{\partial x}, \quad \dots$$ (Hu 1995, eq. 2.1)

With $\sigma_x = \sigma_y = 0$ these reduce to the Euler equations, so the interior is a special case of the layer. Inside the layer the waves decay exponentially, and with $\sigma_y$ equal on both sides of an interface normal to $x$ the reflection coefficient of incident acoustic, vorticity and entropy waves is zero, "independent of the angle of incidence and frequency" (secs. 3-4). In the numerical tests the reflection falls as the layer gets thicker (sec. 5). rcfd's plain sponge instead damps the conserved variables toward a reference state, $S = -\mathrm{rate}\,(U - U_{ref})$, and has a separate matched form ([[adr-thread-compressible-acoustic-boundaries]]).

**Impedance boundaries.** A wall or boundary can also be described by its impedance: for a locally reacting duct liner, $p/v = Z/(\rho_r c_r)$, the nondimensional specific impedance (eq. 23) ([[ref-hubbard-1991-nasa-rp-1258-vol2]] ch. 13, p. 112). A boundary whose impedance equals the characteristic impedance of the medium, $\rho c$, absorbs a normally incident plane wave; a wave arriving at an angle sees a different effective impedance and is partly reflected, as rcfd's closed form for the mass-spring-damper wall shows ($\frac{Z\cos\theta - \rho c}{Z\cos\theta + \rho c}$, [[adr-thread-compressible-acoustic-boundaries]]).

**What it adds.** [[characteristics-and-hyperbolic-systems]] says which waves leave a boundary; [[acoustic-wave-equation]] gives their form. This page is about removing them cleanly: conditions that are exact only for some waves (normal incidence), and layers that trade extra cells for low reflection at all angles.

## Builds on

- [[characteristics-and-hyperbolic-systems]]: incoming and outgoing waves, characteristic outflow conditions.
- [[acoustic-wave-equation]]: linearised Euler equations, plane waves and impedance.

## Leads to

- [[adr-thread-compressible-acoustic-boundaries]], [[aeroacoustics]], [[adr-thread-acoustic-wave-solver]]

## How rcfd uses it

- ADR 0006's `NonReflecting` boundary treats only normally incident waves as non-reflecting; "oblique incidence reflects" ([[adr-thread-early-physics-models]]). Its `reflection_value` default `0.02` is held by the maintainer, although it "reflects 46 percent of a short pulse" ([[adr-thread-compressible-acoustic-boundaries]]).
- `CompBc::Impedance { p, z }` (F-243, ADR 0233) and the mass-spring-damper wall `MsdWall` (F-332, ADR 0243, oblique incidence not checked) ([[adr-thread-compressible-acoustic-boundaries]]).
- Matched and Thompson quiet boundaries are quiet only near normal incidence: "7 percent at 30 deg, 17 at 45" ([[doc-acoustics]]).
- Sponge layers (F-331, ADR 0237): plain form $S = -\mathrm{rate}\,(U - U_{ref})$ on the five conserved variables, matched form only for a reference at rest; memory decay, corners and damping along the layer (F-340, ADR 0248, clause 1 missed); sponges on moving meshes (F-341, F-407) ([[adr-thread-compressible-acoustic-boundaries]], [[dynamic-mesh]]).
- All of these are library only until the case wiring item F-330 ([[adr-thread-compressible-acoustic-boundaries]]).
- The acoustic wave solver uses the existing `sponge::SpongeLayer` ([[adr-thread-acoustic-wave-solver]]).

## Open questions

- The sources read here give no formula for the reflection coefficient of a plain (non-matched) sponge; rcfd's measured values are in its ADRs only.
