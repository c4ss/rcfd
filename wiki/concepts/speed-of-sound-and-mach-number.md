---
type: concept
tags: [foundation, compressible, acoustics]
sources: [ref-naca-1953-report-1135, ref-nasa-glenn-beginners-guide, ref-hubbard-1991-nasa-rp-1258-vol1]
builds_on: [ideal-gas-and-thermodynamics-basics]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Speed of sound and Mach number

## In short

The speed of sound $a$ is how fast a small pressure disturbance travels through a gas; for a perfect gas $a = \sqrt{\gamma R T}$, so it depends only on temperature. The Mach number $M = V/a$ compares the flow speed with it and says how strongly compressibility matters: density changes in a flow scale roughly with $M^2$ ([[ref-naca-1953-report-1135]] p. 616; [[ref-nasa-glenn-beginners-guide]] "Mach Number").

## Explanation

**Definition.** The speed of sound is the slope of pressure against density at constant entropy, because a sound wave is too fast for heat exchange ([[ref-naca-1953-report-1135]] p. 616):

$$a = \sqrt{\left(\frac{\partial p}{\partial \rho}\right)_s} = \sqrt{\gamma \left(\frac{\partial p}{\partial \rho}\right)_T}$$ (NACA 1135, eq. 29a)

For a thermally perfect gas

$$a = \sqrt{\gamma \frac{p}{\rho}} = \sqrt{\gamma R T}$$ (NACA 1135, eq. 29b)

and for air about $49.0\sqrt{T}$ ft/s with $T$ in degrees Rankine (eq. 29c). The same relation appears in Lilley's acoustics chapter, $c = \sqrt{\gamma R T}$ with $p = \rho R T$ ([[ref-hubbard-1991-nasa-rp-1258-vol1]] ch. 4, p. 214).

**Mach number.** $M = V/a$ (eq. 30). Dynamic pressure can be written $q = \tfrac12 \rho V^2 = \tfrac{\gamma}{2} p M^2$ (eq. 31b) ([[ref-naca-1953-report-1135]] p. 616). NASA Glenn's beginner page defines $M$ as object speed over speed of sound, an indicator of how strongly compressibility affects the flow, and names the regimes: subsonic $M < 1$, transonic near 1, supersonic $1 < M < 3$, high supersonic $3 < M < 5$, hypersonic $M > 5$ ([[ref-nasa-glenn-beginners-guide]] "Mach Number").

**Why $M$ measures compressibility.** For small disturbances about a uniform stream ([[ref-naca-1953-report-1135]] p. 616):

$$\frac{p}{p_\infty} \approx 1 - \gamma M_\infty^2 \frac{V - V_\infty}{V_\infty}, \qquad \frac{\rho}{\rho_\infty} \approx 1 - M_\infty^2 \frac{V - V_\infty}{V_\infty}$$ (NACA 1135, eqs. 41-42)

A given relative velocity change produces a density change proportional to $M^2$. At low $M$ the density hardly changes, which is the basis of [[incompressible-flow]]. The NASA page says density stays essentially constant at low speeds, "roughly below 250 mph", and gives no Mach threshold such as 0.3 ([[ref-nasa-glenn-beginners-guide]] "Mach Number").

**Stagnation ratios in terms of $M$.** With $\gamma$ fixed, every isentropic ratio is a function of $M$ alone ([[ref-naca-1953-report-1135]] p. 616):

$$\frac{T}{T_t} = \left(1 + \frac{\gamma-1}{2} M^2\right)^{-1}, \qquad \frac{p}{p_t} = \left(1 + \frac{\gamma-1}{2} M^2\right)^{-\gamma/(\gamma-1)}$$ (NACA 1135, eqs. 43-44)

$$\frac{\rho}{\rho_t} = \left(1 + \frac{\gamma-1}{2} M^2\right)^{-1/(\gamma-1)}, \qquad \frac{a}{a_t} = \left(1 + \frac{\gamma-1}{2} M^2\right)^{-1/2}$$ (NACA 1135, eqs. 45-46)

The critical state $*$ is where the local speed equals the local speed of sound, $M = 1$ (p. 614). The area of a stream tube is smallest there: $A_*/A$ (eq. 80) is a function of $M$ that equals 1 at $M = 1$ ([[ref-naca-1953-report-1135]] p. 618).

**Mach angle.** In supersonic flow small disturbances lie on Mach lines inclined at $\mu = \sin^{-1}(1/M)$ to the flow ([[ref-naca-1953-report-1135]] symbols, p. 613; Prandtl-Meyer figures, p. 626). See [[characteristics-and-hyperbolic-systems]].

**What it adds.** [[ideal-gas-and-thermodynamics-basics]] gives $p = \rho R T$ and the isentropic law; this page adds the one speed at which pressure information travels, and the single parameter $M$ that tells whether a flow can be treated as incompressible, contains shocks, or radiates sound efficiently.

## Builds on

- [[ideal-gas-and-thermodynamics-basics]]: the state equation, $\gamma$, and the isentropic law used to evaluate $(\partial p/\partial\rho)_s$.

## Leads to

- [[shock-waves]] (only for $M_1 > 1$), [[characteristics-and-hyperbolic-systems]], [[acoustic-wave-equation]], [[rotor-tone-noise]] (tip Mach number)
- [[fw-h]], [[doc-case-schema-compressible]]

## How rcfd uses it

- FW-H in rcfd requires subsonic surfaces; the uniform-stream evaluation "requires `|U| < c0`" and a valid-window guard refuses non-subsonic surfaces ([[fw-h]]).
- The FW-H chain omits the quadrupole term, "small for subsonic tip Mach numbers, not zero" ([[fw-h]], [[doc-acoustics]]).
- The density-based Euler solver (`solver.algorithm = "compressible"`) is the solver for flows where $M$ matters ([[doc-case-schema-compressible]]).
