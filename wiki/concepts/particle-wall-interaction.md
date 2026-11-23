---
type: concept
tags: [foundation, multiphase, particles, erosion]
sources: [ref-schwarz-kopper-2026-particles-moving-domains, ref-marzouk-huckaby-2009-particle-laden-swirl]
builds_on: [particle-equation-of-motion]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Particle-wall interaction (restitution, rebound, erosion)

## In short

When a tracked particle hits a wall, a model decides what happens: it bounces back, perhaps losing some speed, sticks, or leaves. The usual rebound model is the coefficient of restitution, the ratio of rebound to impact velocity, normal and tangential to the wall; the impacts can also wear the wall away (erosion) ([[ref-schwarz-kopper-2026-particles-moving-domains]] sec. 2.2, 2.4).

## Explanation

**Hard-sphere collisions.** The collision is not resolved in time but "handled in an a posteriori manner": when a particle crosses a wall during a step it is reflected about the wall normal $n$, and the momentum change at impact defines an impulse (Eq. 5). Eq. 5 alone conserves momentum but does not dissipate energy, while real collisions are inelastic ([[ref-schwarz-kopper-2026-particles-moving-domains]] sec. 2.2).

**Coefficient of restitution.** Rebound models map the impact state to the rebound state:

$$e_x = \frac{x^{ref}}{x^{imp}}$$ (Schwarz and Kopper, Eq. 6)

usually for the normal and tangential velocities, $e_n$ and $e_t$. In general $e_x$ is a function of parameters that characterise the collision physics, and empirical or learned rebound models supply it ([[ref-schwarz-kopper-2026-particles-moving-domains]] sec. 2.2). The simplest choice is a perfectly elastic collision with tangential friction neglected, as in the swirling-jet study of [[ref-marzouk-huckaby-2009-particle-laden-swirl]] (sec. 3).

**Erosion.** Each impact can remove (erosion) or add (deposition) wall material; erosion models correlate the energy dissipated at impact with an erosion rate, converted to a volume loss with the wall material's density ([[ref-schwarz-kopper-2026-particles-moving-domains]] sec. 2.4). Rebound models scatter the post-impact velocities and angles, which redistributes where later impacts fall (sec. 5.1).

## Builds on

- [[particle-equation-of-motion]]: the trajectory that reaches the wall, and the velocity that the rebound model changes.

## Leads to

- [[lagrangian-particles]] (`reflect`, `stick`, `escape`), [[adr-thread-particle-wall-and-droplets]] (erosion, rough walls, wall film)

## How rcfd uses it

- Wall interactions `reflect` (restitution $e$), `stick`, `escape`; more than `max_bounces` (1000) per step marks a particle `stuck` (ADR 0028) ([[doc-particles]], [[lagrangian-particles]]).
- Erosion uses the DNV RP O501 correlation $E = m_pKU_p^nF(\alpha)$ with the opt-in `dnv_o501_2007_steel` preset; the rough wall is a Sommerfeld-type virtual wall tilted by a random roughness angle (F-254, ADR 0194, ADR 0197) ([[doc-reference-data-f254-particle-wall]], [[adr-thread-particle-wall-and-droplets]]).
- Wall film with the Nusselt thickness, gas buoyancy and shear (F-253, F-292) ([[adr-thread-particle-wall-and-droplets]]).

## Open questions

- No open source on rough-wall (virtual wall) models or wall films was read in this job; those rcfd features rest on the sources named on [[doc-reference-data-f254-particle-wall]] and [[doc-reference-data-f253-wall-film]].
