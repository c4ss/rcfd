---
type: concept
tags: [foundation, turbulence, rans, sst]
sources: [ref-menter-1992-nasa-tm-103975, ref-nasa-tmr, ref-davidson-turbulence-notes]
builds_on: [boussinesq-eddy-viscosity, reynolds-stress, boundary-layer]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Bradshaw's assumption and the SST limiter

## In short

In a boundary layer the turbulent shear stress is found to be roughly proportional to the turbulent kinetic energy, $-\overline{u'v'} = a_1 k$ with $a_1$ about 0.3 (Bradshaw's assumption). A standard eddy-viscosity model instead makes the shear stress follow the local strain rate, and over-predicts it in adverse pressure gradients. SST limits the eddy viscosity, $\nu_t = \dfrac{a_1 k}{\max(a_1 \omega,\, S F_2)}$, so that the shear stress never exceeds $a_1 k$ inside boundary layers. This is the "shear-stress transport" in the name ([[ref-menter-1992-nasa-tm-103975]], [[ref-nasa-tmr]]).

## Explanation

**The observation.** In a full [[reynolds-stress-models|Reynolds-stress model]] the principal turbulent shear stress $\tau = -\overline{u'v'}$ has its own transport equation, so its history (the term $D\tau/Dt$) matters. The Johnson-King model, very successful for adverse-pressure-gradient flows, carries such a transport equation, "based on Bradshaw's assumption that the shear-stress in a boundary-layer is proportional to the turbulent kinetic energy" ([[ref-menter-1992-nasa-tm-103975]] p. 6):

$$\tau = \rho a_1 k, \quad a_1 = 0.3$$ (eq. 15)

Davidson writes it $-\overline{v_1'v_2'} = a_1 k$, $a_1 = c_\mu^{1/2}$ ([[ref-davidson-turbulence-notes]] eq. 16.14).

**What the Boussinesq relation gives instead.** A two-equation model computes $\tau = \mu_t \Omega$ with $\Omega = \partial U/\partial y$ (eq. 16), which can be rewritten $\tau = \rho \sqrt{\text{Production}_k/\text{Dissipation}_k}\, a_1 k$ (eq. 17). In adverse-pressure-gradient boundary layers production can be "significantly larger than" dissipation, so eq. 17 "leads to an overprediction of $\tau$" ([[ref-menter-1992-nasa-tm-103975]] pp. 6-7). Davidson's version: experiments show $-\overline{v_1'v_2'} \sim c_\mu^{1/2} k$ while $P_k \gg \varepsilon$, "which explains why the Boussinesq assumption over-predicts the shear stress" there ([[ref-davidson-turbulence-notes]] eq. 16.15-16.16). Too high a shear stress "delays (or completely prevents) separation", as Menter says of k-epsilon ([[ref-menter-1992-nasa-tm-103975]] p. 2; [[flow-separation-and-reattachment]]).

**The fix.** To satisfy $\tau = \rho a_1 k$ in an eddy-viscosity model, set $\nu_t = a_1 k/\Omega$ (eq. 18). Then "$\tau$ does not change more rapidly than $\rho a_1 k$", whereas in conventional two-equation models "the turbulent shear-stress responds instantaneously to changes in the shear-strain rate". Since eq. 18 alone blows up where $\Omega \to 0$, take ([[ref-menter-1992-nasa-tm-103975]] eq. 19):

$$\nu_t = \dfrac{a_1 k}{\max(a_1 \omega,\, \Omega)}$$

This picks $a_1 k/\Omega$ where $\Omega > a_1 \omega$, i.e. where production exceeds dissipation (most of an adverse-pressure-gradient layer), and the usual $k/\omega$ elsewhere. Bradshaw's assumption "does not necessarily hold" in free shear layers, so the limiter is restricted to boundary layers with $F_2$ ([[sst-blending-functions]]) (eq. 20):

$$\nu_t = \dfrac{a_1 k}{\max(a_1 \omega,\, \Omega F_2)}\,.$$

**Versions.** The 1992 TM uses the vorticity magnitude $\Omega$ and $a_1 = 0.3$, and recommends calibrating the $\omega$ production with eq. A-4, not the $(\gamma \omega/k)\, \tau_{ij}\, \partial U_i/\partial x_j$ form, because with the limiter $\nu_t \omega/k$ is no longer 1 ([[ref-menter-1992-nasa-tm-103975]] pp. 28-29). The TMR standard SST (1994) uses $a_1 = 0.31$; SST-2003 uses the strain-rate invariant $S = \sqrt{2 S_{ij} S_{ij}}$ instead of the vorticity, $\mu_t = \dfrac{\rho a_1 k}{\max(a_1 \omega,\, S F_2)}$ ([[ref-nasa-tmr]] `sst.html`).

**The production limiter (a separate device).** Not part of Bradshaw's argument, but used with SST: $P$ in the $k$ equation is replaced by $\min(P,\, 20 \beta^* \rho \omega k)$ (Menter 1993), and in SST-2003 by $\min(P,\, 10 \beta^* \rho \omega k)$ in both equations ([[ref-nasa-tmr]] `sst.html`). It caps production at a multiple of the destruction term $\beta^* \rho \omega k$.

**What it adds to its prerequisites.** [[boussinesq-eddy-viscosity]] ties the stress to the local strain; [[reynolds-stress]] shows the stress is really a transported quantity; the [[boundary-layer]] measurements give the ratio $a_1$. The limiter is the cheapest way to put a piece of stress-transport physics back into an eddy-viscosity model: one $\max(\cdot)$ in the definition of $\nu_t$, no extra equation.

## Builds on

- [[boussinesq-eddy-viscosity]]: the relation whose $\nu_t$ is limited.
- [[reynolds-stress]]: the shear component $-\overline{u'v'}$ and its transport.
- [[boundary-layer]]: where Bradshaw's ratio holds and adverse pressure gradients act.

## Leads to

- [[k-omega-sst]], [[flow-separation-and-reattachment]]

## How rcfd uses it

- rcfd's SST: $\nu_t = a_1 k/\max(a_1 \omega,\, S F_2)$, $S = \sqrt{2 S_{ij} S_{ij}}$, $a_1 = 0.31$, production limited to $10\, \beta^* \rho \omega k$, i.e. the SST-2003 form via TMR ([[doc-reference-data-sst-wall]], [[k-omega-sst]]).
- Wall-resolved backward-facing step: SST 6.554 and BSL 5.956 against the window 6.16 to 6.36 (F-043 gap) ([[roadmap-turbulence]]).
