---
type: concept
tags: [foundation, multiphase, particles]
sources: [ref-marzouk-huckaby-2009-particle-laden-swirl, ref-jaganathan-2023-basset-history-force, ref-wikipedia-multiphase-heat, ref-schwarz-kopper-2026-particles-moving-domains]
builds_on: [viscosity, dimensionless-numbers]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Particle equation of motion (Stokes drag, response time, Stokes number, Maxey-Riley)

## In short

A small particle in a flow is pushed by the fluid's drag, pulled by gravity and lifted by buoyancy. Newton's second law for it gives an equation in which the particle velocity relaxes towards the fluid velocity with a time constant, the particle response time; comparing that time with the flow's time scale (the Stokes number) says whether the particle follows the flow or not ([[ref-marzouk-huckaby-2009-particle-laden-swirl]] sec. 3; [[ref-jaganathan-2023-basset-history-force]] sec. 1).

## Explanation

**Stokes drag.** A sphere of radius $R$ moving at speed $v$ relative to a fluid of [[viscosity]] $\mu$ at very low Reynolds number feels $F_d = 6\pi\mu R v$, opposing the motion; to 10 % accuracy this needs $Re < 1$ ([[ref-wikipedia-multiphase-heat]] "Stokes' law"). Balancing it with weight minus buoyancy gives the terminal velocity $v = \frac29(\rho_p - \rho_f)gR^2/\mu$, growing as $R^2$ (same, "Terminal velocity").

**General drag and the Lagrangian equations.** For a particle of diameter $d$ and mass $m$ at position $x$ with velocity $u$, in a fluid of velocity $U$ at the particle ([[ref-marzouk-huckaby-2009-particle-laden-swirl]] sec. 3):

$$\frac{dx}{dt} = u, \qquad m\frac{du}{dt} = -\frac{\pi d^2}{8}\rho C_D|u-U|(u-U) + mg - \rho\mathcal V g$$ (Marzouk and Huckaby, Eqs. 16, 17)

with the drag coefficient $C_D$ a function of the particle Reynolds number $Re_d = \rho|u-U|d/\mu$ (Eq. 21; see [[dimensionless-numbers]]). One sphere correlation used is $C_D = \frac{24}{Re_d}\big(1 + \frac16Re_d^{2/3}\big)$ for $Re_d \le 1000$ and $C_D = 0.424$ above, "very similar to the Schiller-Naumann expression" (Eq. 20). At low $Re_d$, $C_D \to 24/Re_d$ and the drag reduces to Stokes' law.

**Response time.** With $m = \rho_s\pi d^3/6$ the equation becomes

$$\frac{du}{dt} = -\frac{u-U}{\tau} + \left(1 - \frac{\rho}{\rho_s}\right)g, \qquad \tau = \frac{\rho_s d^2}{18\mu}\,\frac{24}{Re_d C_D}$$ (Marzouk and Huckaby, Eq. 22b)

so in the Stokes limit $\tau = \rho_s d^2/(18\mu)$: the time a particle needs to adjust to a change in the fluid velocity. A drag factor $f_D$ multiplying the Stokes drag $3\pi\mu d_p(u_f - v_p)$ is the same idea written the other way round ([[ref-schwarz-kopper-2026-particles-moving-domains]] sec. 2.2, Eq. 4).

**Stokes number.** The ratio of the particle relaxation time to a flow time scale; tracers ($S \to 0$) follow the flow exactly, inertial particles leave the fluid paths and can collect in parts of the flow (preferential concentration) ([[ref-jaganathan-2023-basset-history-force]] sec. 1).

**The Maxey-Riley terms.** For a small rigid sphere in a non-uniform flow, the Maxey-Riley-Gatignol equation adds to Stokes drag the pressure-gradient (fluid acceleration) force, added mass and the Basset-Boussinesq history force, a time integral of the past slip acceleration with a kernel $\propto 1/\sqrt t$. It assumes small particles, a dilute suspension with one-way coupling, and small particle and shear Reynolds numbers ([[ref-jaganathan-2023-basset-history-force]] sec. 1). The history force is "often neglected, not because such neglect is known to be justified, but because it is difficult to be included"; the usual argument that it is negligible for particles much heavier than the fluid holds only for point particles (same). Engineering codes often keep only drag and gravity ([[ref-marzouk-huckaby-2009-particle-laden-swirl]] Eq. 17; [[ref-schwarz-kopper-2026-particles-moving-domains]] Eq. 4).

## Builds on

- Floor: Newton's second law.
- [[viscosity]]: the fluid property in Stokes drag and in $\tau$.
- [[dimensionless-numbers]]: the particle Reynolds number and the Stokes number.

## Leads to

- [[turbulent-dispersion-random-walk]], [[two-way-coupling]], [[droplet-evaporation-and-breakup]], [[particle-wall-interaction]]
- [[lagrangian-particles]]

## How rcfd uses it

- rcfd integrates $dv/dt = f(Re)(u - v)/\tau_p + (1 - \rho_f/\rho_p)g$ with $\tau_p = \rho_p d^2/(18\mu)$; drag `stokes`, `schiller_naumann` (switch at Re 1000 UNVERIFIED) or `none` ([[lagrangian-particles]], [[doc-particles]]).
- Not modelled: added mass, pressure-gradient force, Basset history, lift, Brownian motion, rotation, particle-particle interaction ([[lagrangian-particles]]), which is the reduced form described above.
- Integrators `analytic` (exact for Stokes drag in a uniform field), `euler`, `rk2`, `rk4` ([[lagrangian-particles]]); see [[explicit-time-integration-and-runge-kutta]].
- The Schiller-Naumann constants (0.15, 0.687) are "recalled from memory" on [[doc-reference-data-wave29b-cavitation]]; the correlation of Marzouk and Huckaby Eq. 20 is a different, similar one, so it does not verify them.
