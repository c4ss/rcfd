---
type: concept
tags: [foundation, turbulence, rans]
sources: [ref-mit-ocw-2-29-turbulence, ref-davidson-turbulence-notes, ref-menter-1992-nasa-tm-103975]
builds_on: [boussinesq-eddy-viscosity, law-of-the-wall]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Mixing-length model

## In short

Prandtl's mixing-length model is the simplest eddy-viscosity closure: $\nu_t = l^2 \lvert dU/dy \rvert$, where the mixing length $l$ is prescribed algebraically, near a wall $l = \kappa y$. It needs no extra transport equation ("zero-equation model") and reproduces the logarithmic law of the wall, but the length has to be known in advance for each flow ([[ref-mit-ocw-2-29-turbulence]], [[ref-davidson-turbulence-notes]]).

## Explanation

**Derivation by dimensions.** An eddy viscosity is a velocity scale times a length scale, $\nu_t = C u^* L$. The largest eddies carry most of $k$ and interact most with the mean flow; take their size as the mixing length $L$ and their velocity as $L \lvert dU/dy \rvert$. Then ([[ref-mit-ocw-2-29-turbulence]] L25 s.15):

$$\nu_t = L^2 \left\lvert \dfrac{dU}{dy} \right\rvert, \quad -\overline{u'v'} = L^2 \left\lvert \dfrac{dU}{dy} \right\rvert \dfrac{dU}{dy}.$$

$L$ plays the role of a mean free path: the distance a fluid parcel travels before it mixes with its surroundings. In three dimensions the gradient is replaced by the strain-rate magnitude, $\nu_t = l^2 (2 s_{ij} s_{ij})^{1/2}$ ([[ref-davidson-turbulence-notes]] sec. 18.7).

**Near a wall.** An eddy cannot be larger than its distance to the wall, so Prandtl took $L = \kappa y$ ([[ref-mit-ocw-2-29-turbulence]] L25 s.15). In the log region the Reynolds shear stress is about the wall stress, $-\overline{u'v'} = u_\tau^2$; with $L = \kappa y$ this gives $dU/dy = u_\tau/(\kappa y)$, which integrates to the logarithmic law $U/u_\tau = (1/\kappa) \ln y^+ + B$ ([[ref-davidson-turbulence-notes]] eqs. 6.28-6.33). This is the link to the [[law-of-the-wall]].

**Strengths and limits.** In simple 2D shear flows mixing-length models "agree well with data", with $L$ proportional to a physical size of the flow; but "in general turbulence, more than one space and time scale" exists, and one algebraic length cannot represent them ([[ref-mit-ocw-2-29-turbulence]] L25 s.16). Menter names exactly this, "the difficulty to define an algebraic length-scale" in separated, multi-scale and 3D flows, as the reason to move from algebraic models to transport-equation models ([[ref-menter-1992-nasa-tm-103975]] p. 1). The Smagorinsky subgrid model of LES is the same formula with the length $C_S \Delta$ tied to the grid ([[ref-davidson-turbulence-notes]] sec. 18.7, [[les-and-des]]).

**What it adds to its prerequisites.** The [[boussinesq-eddy-viscosity]] hypothesis leaves $\nu_t$ open; the mixing length fills it with the simplest possible rule, using the wall distance as the length scale, the same length that gives the [[law-of-the-wall]]. Its weakness (a prescribed length) is what two-equation models remove by transporting a length-scale variable ([[two-equation-turbulence-models]]).

## Builds on

- [[boussinesq-eddy-viscosity]]: the stress-strain relation whose $\nu_t$ it supplies.
- [[law-of-the-wall]]: the log region where $L = \kappa y$ is derived and checked.

## Leads to

- [[two-equation-turbulence-models]] (transported scales instead of a prescribed length), [[les-and-des]] (Smagorinsky)

## How rcfd uses it

- `mixing_length` is a value of `turbulence.model` ([[doc-case-schema-turbulence]]); Smagorinsky, WALE and the mixing-length closure came in ADR 0026 ([[adr-thread-early-turbulence]]).
- The F-040 wall-function channel test recovers the log law "with a mixing-length interior", "not yet with k-epsilon"; its interior profile is checked within `0.03` of the mixing-length balance, and the thread lists among ADR 0707's open points "mixing-length closure not selectable from the case schema" ([[adr-thread-transient-rans]]).
- F-206 (additional closures, partial) lists algebraic zero-equation and mixing-length models among its scope ([[roadmap-turbulence]]).
