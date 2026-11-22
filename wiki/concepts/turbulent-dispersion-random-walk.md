---
type: concept
tags: [foundation, multiphase, particles, turbulence]
sources: [ref-marzouk-huckaby-2009-particle-laden-swirl]
builds_on: [particle-equation-of-motion, energy-cascade-and-kolmogorov-scales, probability-and-statistics-basics]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Turbulent dispersion by discrete random walk (eddy interaction)

## In short

A RANS solution gives only the mean flow, but real turbulent eddies kick particles around and spread them out. The discrete random walk puts the eddies back statistically: each particle meets a sequence of random eddies, each with a random velocity drawn from the turbulence intensity and a lifetime from the turbulence time scale ([[ref-marzouk-huckaby-2009-particle-laden-swirl]] sec. 3).

## Explanation

**Instantaneous fluid velocity.** The fluid velocity seen by the particle is the resolved (mean) velocity interpolated to the particle plus a fluctuation ([[ref-marzouk-huckaby-2009-particle-laden-swirl]] sec. 3):

$$U = \bar U + U'$$ (Marzouk and Huckaby, Eq. 18)

and $U$ is what enters the drag of the [[particle-equation-of-motion]].

**The random walk.** "Uncorrelated eddies are generated randomly, but the particle trajectory is deterministic within the eddy": $U'$ is drawn at random when the particle enters an eddy and held fixed while it stays in it (sec. 3). The sampling uses the turbulence fields; see [[probability-and-statistics-basics]] for random variables and distributions.

**How long an eddy acts.** The particle feels one fluctuation over the interaction time, the shorter of the eddy lifetime and the time it needs to cross the eddy ([[ref-marzouk-huckaby-2009-particle-laden-swirl]] sec. 3):

$$T_{eddy} = \frac{k}{\epsilon}, \qquad T_{cross} = C_{cross}\frac{k^{3/2}}{\epsilon\,|u - \bar U - U'|}, \qquad T_{interac} = \min(T_{eddy}, T_{cross})$$ (Marzouk and Huckaby, Eqs. 19a-19c)

$T_{eddy}$ is the Lagrangian integral time scale of the turbulence; $T_{cross}$ is "the time needed by the particle to traverse the eddy", built from $k^{3/2}/\epsilon$ and the slip speed; $C_{cross} = 0.16432 = C_\mu^{3/4}$ with $C_\mu = 0.09$. The turbulence data are updated every time step because the turbulence along a trajectory is not homogeneous (sec. 3). The scales $k$ and $\epsilon$ are the energy and dissipation of [[energy-cascade-and-kolmogorov-scales]].

**What it adds.** The [[particle-equation-of-motion]] alone would see only the resolved velocity $\bar U$; the random walk adds the effect of the unresolved fluctuations "on the particle over an interaction time" (sec. 3).

## Builds on

- [[particle-equation-of-motion]]: the drag that turns the random fluid velocity into particle motion.
- [[energy-cascade-and-kolmogorov-scales]]: $k$, $\epsilon$ and the eddy time and length scales.
- [[probability-and-statistics-basics]]: sampling random velocities.

## Leads to

- [[lagrangian-particles]] (`dispersion = "drw"`)

## How rcfd uses it

- Particle dispersion is a discrete random walk; $k$ and $\epsilon$ come from k-epsilon or SST ($\epsilon = eta^*k\omega$, $eta^* = 0.09$) since F-255 (ADR 0208); other turbulence models are refused ([[lagrangian-particles]], [[doc-case-schema-particles]]).
- F-255 checks the walk against a derived mean-square displacement for an exponential velocity autocorrelation, whose long-time diffusivity $\sigma^2T$ is independent of the response time; nothing is compared with measured data ([[doc-reference-data-f255-particle-dispersion]]).
