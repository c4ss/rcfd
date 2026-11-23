---
type: concept
tags: [foundation, acoustics, wave-equation, retarded-time]
sources: [ref-farassat-2007-nasa-tm-214853, ref-hubbard-1991-nasa-rp-1258-vol1, ref-hubbard-1991-nasa-rp-1258-vol2]
builds_on: [acoustic-wave-equation]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Green's function and retarded time

## In short

The free-space Green's function is the sound field of a single impulsive point source: a spherical shell that leaves the source at speed $c$ and weakens as $1/r$. Any source distribution can be built from such impulses, so the field at an observer is an integral over the sources, each evaluated at the earlier time when its sound had to leave in order to arrive now: the retarded (emission) time ([[ref-farassat-2007-nasa-tm-214853]] secs. 3.1-3.2).

## Explanation

**The Green's function.** For the wave equation in unbounded three-dimensional space ([[ref-farassat-2007-nasa-tm-214853]] sec. 3.1):

$$G(\mathbf{x}, t; \mathbf{y}, \tau) = \begin{cases} \dfrac{\delta(t - \tau - r/c)}{4\pi r} & \tau \le t \\ 0 & \tau > t \end{cases}, \qquad r = |\mathbf{x} - \mathbf{y}|$$ (Farassat 2007, eq. 10)

$(\mathbf{x}, t)$ are the observer variables, $(\mathbf{y}, \tau)$ the source variables, and $\delta$ the Dirac delta function. It is nonzero only when $t - \tau = r/c$: the impulse emitted at $\tau$ arrives at distance $r$ exactly $r/c$ later. The $1/r$ factor is the spherical spreading that makes a point source lose 6 dB per doubling of distance ([[ref-hubbard-1991-nasa-rp-1258-vol2]] ch. 12, p. 55). Two derivatives recur: $\partial r/\partial x_i = \hat r_i$ and $\partial r/\partial y_i = -\hat r_i$, with $\hat{\mathbf{r}} = (\mathbf{x} - \mathbf{y})/r$ the unit radiation vector (eq. 11).

**The collapsing sphere.** Fix the observer. The source points and times that can reach it, $|\mathbf{x} - \mathbf{y}| = c(t - \tau)$, form a sphere centred on $\mathbf{x}$ whose radius shrinks to zero as $\tau \to t$: "the characteristic cone of the wave equation with the vertex at (x, t)", the backward cone only, by causality ([[ref-farassat-2007-nasa-tm-214853]] Remark 3).

**Solution for a stationary source distribution.** Convolving the sources with $G$ gives the retarded-potential solution. In Lighthill's form for the density fluctuation ([[ref-hubbard-1991-nasa-rp-1258-vol1]] ch. 4, p. 215):

$$\rho(\mathbf{x}, t) = \frac{1}{4\pi c_\infty^2} \iiint \frac{A(\mathbf{y}, \tau)}{|\mathbf{x} - \mathbf{y}|}\, d\mathbf{y}, \qquad \tau = t - \frac{|\mathbf{x} - \mathbf{y}|}{c_\infty}$$ (Lilley, eq. 14)

where $A$ is the source strength per unit volume and $\tau$ is the retarded time.

**Moving sources and the Doppler factor.** For sources on a moving surface (a rotating blade), Farassat follows each surface point $\boldsymbol{\eta}$ in a blade-fixed frame. Changing the integration variable from source time to $g = \tau - t + r/c$ brings in the Jacobian ([[ref-farassat-2007-nasa-tm-214853]] sec. 3.2):

$$\frac{\partial g}{\partial \tau} = 1 - M_r, \qquad M_r = \frac{\hat r_i v_i}{c}$$ (Farassat 2007, eq. 20)

$M_r$ is the source Mach number toward the observer and $1 - M_r$ the Doppler factor, positive for a subsonic surface. The emission time $\tau_e$ of each point solves

$$|\mathbf{x} - \mathbf{y}(\boldsymbol{\eta}, \tau)| = c\,(t - \tau)$$ (Farassat 2007, eq. 23)

For a subsonic source there is only one emission time; for rotating blades the equation is transcendental and is solved numerically, for example by stepping back along the source trajectory and refining ([[ref-farassat-2007-nasa-tm-214853]] sec. 3.2). For uniform rectilinear motion it has a closed form, with one root subsonic and two supersonic (Remark 4). The result for a surface source $Q$ is

$$4\pi p'(\mathbf{x}, t) = \int_{f=0} \left[\frac{Q}{r\,|1 - M_r|}\right]_{ret} dS$$ (Farassat 2007, eq. 29)

where $[\,]_{ret}$ means each point's integrand at its own emission time, not simply $\tau = t - r/c$ with a fixed $r$ (eq. 30). Connecting the blade edges at their emission times gives the "acoustic planform", which for supersonic tips breaks into pieces ([[ref-hubbard-1991-nasa-rp-1258-vol1]] ch. 1, p. 13).

**Far field.** At large distance the observer-space derivative can be replaced by a time derivative, $\partial/\partial x_i \approx -(\hat r_i/c)\,\partial/\partial t$ (eq. 33); Farassat makes this exact for the loading term (eq. 35), and Formulation 1A separates near-field ($1/r^2$) and far-field ($1/r$) terms ([[ref-farassat-2007-nasa-tm-214853]] secs. 3.3-3.4).

**What it adds.** The [[acoustic-wave-equation]] says how sound propagates; this page gives its explicit solution for given sources, which is what an acoustic analogy needs.

## Builds on

- [[acoustic-wave-equation]]: the operator whose impulse response $G$ is.

## Leads to

- [[acoustic-source-types]], [[lighthill-acoustic-analogy]], [[fw-h]]

## How rcfd uses it

- rcfd's FW-H finds the retarded time "by Newton per element and observer time", with 4-point Lagrange interpolation and a valid-window guard that refuses out-of-record times and non-subsonic surfaces ([[fw-h]]).
- For a uniform stream it evaluates in the medium frame with a moving observer, "exact for 1A in a medium at rest", requiring `|U| < c0` ([[fw-h]]); Farassat's formulations likewise take the observer in the frame fixed to the medium ([[ref-farassat-2007-nasa-tm-214853]] sec. 3.5).
