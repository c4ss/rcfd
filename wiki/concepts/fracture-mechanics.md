---
type: concept
tags: [foundation, structural, fracture]
sources: [ref-roylance-mit-3-11-mechanics-of-materials, adr-thread-structural-fracture]
builds_on: [linear-elasticity]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Fracture mechanics (energy release rate, stress intensity factor)

## In short

A crack concentrates stress at its tip, so a cracked part can fail at a load far below the strength of the uncracked material. Linear elastic fracture mechanics compares the energy a growing crack would release with the energy needed to create new surface, or equivalently compares the stress intensity factor at the tip with the material's fracture toughness ([[ref-roylance-mit-3-11-mechanics-of-materials]], "Fracture Mechanics").

## Explanation

**Griffith energy balance.** A crack grows when the strain energy released by its extension exceeds the energy of the new surfaces; this gives a critical crack length for a given stress. Irwin and Orowan replaced the surface energy by a measured critical strain energy release rate $G_c$, which includes plastic work at the tip ([[ref-roylance-mit-3-11-mechanics-of-materials]]).

**Modes.** Mode I opening, mode II in-plane shear (sliding), mode III out-of-plane shear (tearing) ([[ref-roylance-mit-3-11-mechanics-of-materials]]).

**Stress intensity factor.** Near the tip the stresses scale as $K_I/\sqrt{2\pi r}$, with $r$ the distance from the tip; all the geometry and load enter through $K_I$. For a central crack of half-length $a$ in a wide plate, $$K_I = \sigma\sqrt{\pi a}$$ and about 1.12 times that for an edge crack. Fracture occurs when $K_I$ reaches the toughness $K_{Ic}$ ([[ref-roylance-mit-3-11-mechanics-of-materials]]).

**Link between the two views.** $$G = \frac{K^2}{E}\ \text{(plane stress)}, \qquad G = \frac{K^2(1-\nu^2)}{E}\ \text{(plane strain)}$$ ([[ref-roylance-mit-3-11-mechanics-of-materials]]).

**Plastic zone.** The elastic singularity is cut off by a small plastic zone at the tip; linear fracture mechanics holds while it is small compared with the crack and the part ([[ref-roylance-mit-3-11-mechanics-of-materials]]).

**Crack growth under cycles.** $da/dN = A\,\Delta K^m$ (Paris law) links fracture to [[fatigue]] ([[ref-roylance-mit-3-11-mechanics-of-materials]], "Fatigue").

**Not covered by the sources read.** The J-integral and its domain (energy) form, interaction integrals for mixed-mode separation and crack-tip elements were not in the sources read. rcfd converts its J-integral to $K$ with the plane-strain form of the relation above (see below).

## Builds on

- [[linear-elasticity]]: the elastic field around the crack and the moduli in $G = K^2/E'$.

## Leads to

- [[fatigue]] (crack-growth life)

## How rcfd uses it

- J-integral by the domain form, stress intensity `K = sqrt(J E/(1 - nu^2))`, and the Paris crack-growth life (F-429, ADR 0547, partial: one mutation not caught) ([[adr-thread-structural-fracture]]).
- Interaction integral separating `K_I`, `K_II`, `K_III` on straight, circular, elliptical and closed fronts (F-632, ADR 0605, done) ([[adr-thread-structural-fracture]]).
