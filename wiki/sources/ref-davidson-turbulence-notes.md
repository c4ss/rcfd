---
type: source
tags: [reference, foundation, turbulence, boundary-layer, cite-only]
sources: []
origin: https://research.chalmers.se/en/publication/129778
licence: unknown (no licence statement found on the Chalmers record page or in the PDF, checked 2026-11-22)
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Davidson, Fluid mechanics, turbulent flow and turbulence modeling (Chalmers lecture notes)

**Origin:** L. Davidson, *Fluid mechanics, turbulent flow and turbulence modeling*, lecture notes for the MSc courses TME226 and MTF271, Division of Fluid Dynamics, Chalmers University of Technology. Record: https://research.chalmers.se/en/publication/129778; PDF: http://www.tfd.chalmers.se/~lada/postscript_files/solids-and-fluids_turbulent-flow_turbulence-modelling.pdf (594 pages; the version read is dated "October 28, 2026" on its title page). Printed page numbers equal PDF page numbers.
**Kind:** lecture notes (eBook).
**Licence:** unknown: cite only, never copied. Our own summary below, with section, equation and page locators.
**Read:** 2026-11-22 by job p6-turbulence: Sections 3.3 (pp. 47-51), 5.1-5.3 (pp. 77-82), 6.1-6.4 (pp. 87-97), 11.14.1-11.14.2 (pp. 154-157), 16 (pp. 180-183), 18.6-18.7 (pp. 191-192), 20-20.3 (pp. 215-219) and Appendix E.1-E.4 (pp. 347-349). Other chapters not read.

## Summary

A university course text that goes from the Navier-Stokes equations through turbulence physics to RANS, LES and hybrid models. Used here for the elementary turbulence and boundary-layer foundations (what turbulence is, the cascade and Kolmogorov scales, time averaging and the closure problem, the wall region and log law, wall functions and low-Re treatment, the Blasius solution) and for a textbook account of what SST adds and of DES/DDES.

## Key points (own summary)

### 3.3 Two-dimensional boundary layer over a flat plate (pp. 47-51) and Appendix E (pp. 347-349)
- Steady 2D incompressible boundary-layer equations (eq. 3.42) with zero streamwise pressure gradient on a flat plate; a stream function and the similarity variable $\xi = x_2 (V/(\nu x_1))^{1/2}$ turn them into the ordinary differential equation $g''' + \tfrac{1}{2} g\, g'' = 0$ (eq. 3.54), "derived (and solved numerically!) by Blasius in his PhD thesis 1907"; Table 3.1 gives $g''(0) = 0.3320573362$.
- In a boundary layer the "force" that overcomes the wall shear stress comes from the loss of momentum in the convective term, whereas channel flow is driven by a pressure drop (p. 51).
- Appendix E: $\delta_{99} = 5 (\nu x_1/V)^{1/2}$ (eq. E.1); displacement thickness $\delta^* = \int_0^\infty (1 - v_1/V)\, dx_2$ and momentum thickness $\theta = \int_0^\infty (v_1/V)(1 - v_1/V)\, dx_2$ (eq. E.2), with Blasius values $\delta^* = 1.721 (\nu x_1/V)^{1/2}$ and $\theta = 0.664 (\nu x_1/V)^{1/2}$; $C_f = \tau_w/(\tfrac{1}{2} \rho V^2)$ (eq. E.3) and $C_{f,\text{Blasius}} = 0.664/(V x_1/\nu)^{1/2}$ (p. 349).

### 5 Turbulence (pp. 77-82)
- No definition of turbulence, but characteristic features (after Pope and Tennekes & Lumley): irregularity (chaotic yet "deterministic and ... described by the Navier-Stokes equations"), increased diffusivity (which "reduces or delays thereby separation" and raises wall friction and heat transfer), large Reynolds numbers (transition in pipes at $Re_D \sim 2300$, in boundary layers at $Re_x \sim 500\,000$), always three-dimensional and unsteady, dissipative (cascade from the largest to the smallest eddies), and still a continuum (p. 77).
- The largest eddies scale with the flow geometry and take energy from the mean flow; the energy transferred per unit time from eddy to eddy "is the same for each eddy size"; dissipation $\varepsilon$ $[\mathrm{m^2/s^3}]$ is mostly at the smallest scales (p. 78).
- Kolmogorov scales from $\nu$ and $\varepsilon$ by dimensional analysis (eq. 5.5): $v_\eta = (\nu \varepsilon)^{1/4}$, $\eta = (\nu^3/\varepsilon)^{1/4}$, $\tau_\eta = (\nu/\varepsilon)^{1/2}$ (p. 79).
- Energy spectrum $E(\kappa)$, $k = \int_0^\infty E(\kappa)\, d\kappa$, $k = \tfrac{1}{2} \overline{v_i' v_i'}$ (eqs. 5.10-5.11); region I (large energy-containing eddies, production), II (inertial subrange, isotropic, spectral transfer, $P_k = \varepsilon$), III (dissipation range, Kolmogorov scales); inertial range $E(\kappa) = C_K \varepsilon^{2/3} \kappa^{-5/3}$, $C_K \sim 1.5$ (eq. 5.13); isotropic turbulence has zero shear stresses (p. 82); $\varepsilon \sim v_0^3/l_0$ for the large eddies (eq. 5.15).

### 6 Turbulent mean flow (pp. 87-97)
- Decomposition $v_i = \bar{v}_i + v_i'$, $p = \bar{p} + p'$ with a time average over a "sufficiently large" $T$; $\overline{v_i'} = 0$ (eqs. 6.1-6.4).
- Time-averaged incompressible equations (eqs. 6.9-6.10): the RANS equations, with the new term $\overline{v_i' v_j'}$, the symmetric Reynolds stress tensor, "an additional stress term due to turbulence ... and it is unknown"; the closure problem: ten unknowns (three velocities, pressure, six stresses) against four equations (p. 88).
- Boundary-layer form (eq. 6.14) and the total shear stress $\tau_{12,\text{tot}} = \mu\, dv_1/dx_2 - \rho\, \overline{v_1' v_2'}$ (eq. 6.15).
- Wall shear stress $\tau_w = \mu (dv_1/dx_2)_w$ (eq. 6.16); friction velocity $u_\tau = (\tau_w/\rho)^{1/2}$ (eq. 6.17); inner scaling $v_1^+ = v_1/u_\tau$, $x_2^+ = u_\tau x_2/\nu$ (eq. 6.23); viscous length scale $\nu/u_\tau$ (eq. 6.25).
- Wall regions (p. 91, Fig. 6.2): inner region with the viscous region $x_2^+ \le 5$ and the log region $x_2^+ \ge 30$, a buffer region between them; in the inner region the total shear stress is about the wall shear stress; viscous and turbulent stresses are about equal at $x_2^+ \sim 11$. Viscous sublayer: $v_1^+ = x_2^+$ (eq. 6.24).
- Log law (pp. 92-93): with velocity scale $u_\tau$ and length scale $\kappa x_2$, $dv_1/dx_2 = u_\tau/(\kappa x_2)$ (eq. 6.28), or via the Boussinesq form $-\overline{v_1'v_2'} = \nu_t\, dv_1/dx_2$ (eq. 6.29) with $\nu_t = \kappa u_\tau x_2$ (eq. 6.30); integration gives $v_1^+ = (1/\kappa) \ln x_2^+ + B$ (eq. 6.33), "the logarithmic law due to von Karman", usually $\kappa = 0.41$, $B = 5.2$; its upper limit grows with Reynolds number. Outer region: defect law (eq. 6.34) and log-plus-wake form with $\kappa = 0.38$, $B = 4.1$, $\Pi = 0.5$ from boundary-layer flow (eq. 6.35).
- In fully developed channel flow the pressure drop equals the wall shear stress per length (eq. 6.39); turbulent profiles are flatter, so wall shear and pressure drop are larger than in laminar flow (p. 96).
- Boundary layer vs channel (pp. 96-97): convective terms are not zero, the layer grows with $x_1$, the outer part is intermittent; the inner region ($x_2/\delta < 0.1$) is "principally the same" as in channel flow, but the log law holds only up to about $x_2/\delta \sim 0.1$.

### 11.14 Wall boundary conditions (pp. 154-157)
- Two options: a coarse mesh with the log law (wall functions) or a fine mesh with models modified for viscous effects (low-Reynolds-number models) (pp. 154-155).
- Wall functions: log law $v_1/u_\tau = (1/\kappa) \ln(E u_\tau x_2/\nu)$, $E = 9.0$, $B = (1/\kappa) \ln E$ (eqs. 11.125-11.126); $u_\tau$ solved iteratively from the wall-adjacent cell velocity (eq. 11.127); $\tau_w = \rho u_\tau^2$ is the wall force condition for the parallel velocity. In the log region production balances dissipation; with $-\overline{v_1'v_2'} = u_\tau^2$ this gives $C_\mu = u_\tau^4/k^2$, $C_\mu = 0.09$ (eqs. 11.128-11.131); then $k_P = C_\mu^{-1/2} u_\tau^2$ (eq. 11.132) and $\varepsilon_P = u_\tau^3/(\kappa x_2)$ (eq. 11.133).
- Validity $30 < x_2^+ < 400$; below, $u_\tau$ from the linear law (eq. 11.135), switching at $x_2^+ = 11$, the intersection of the two laws; between 11 and 30 a combination of both is "sometimes used" (p. 157).
- Wall functions are an approximation that "can be rather crude" where the near-wall flow matters (external flows, heat transfer); resolving the wall instead requires models modified for viscous effects (low-Re models) (p. 157).

### 16 The SST model (pp. 180-183)
- SST "includes two main novelties": k-omega in the inner boundary layer combined with k-epsilon outside, and "a limitation of the shear stress in adverse pressure gradient regions" (p. 180).
- k-epsilon "over-predicts the shear stress in adverse pressure gradient flows because of too large length scale (due to too low dissipation)" and needs near-wall damping; standard k-omega needs no damping functions but depends on the free-stream $\omega$ (p. 180).
- With the Boussinesq assumption, $-\overline{v_1'v_2'} = c_\mu^{1/2} k (P_k/\varepsilon)^{1/2}$ (eq. 16.15); experiments show $-\overline{v_1'v_2'} \sim c_\mu^{1/2} k$ in adverse-pressure-gradient boundary layers while $P_k \gg \varepsilon$, "which explains why the Boussinesq assumption over-predicts the shear stress" there; Bradshaw's assumption $-\overline{v_1'v_2'} = a_1 k$, $a_1 = c_\mu^{1/2}$ (eq. 16.14) is built into the eddy viscosity, multiplied by $F_2$ so it applies only in the boundary layer (eqs. 16.17-16.18).

### 18.6-18.7 Smagorinsky model (pp. 191-192)
- Smagorinsky SGS model $\tau_{ij} - \tfrac{1}{3} \delta_{ij} \tau_{kk} = -2 \nu_{sgs} \bar{s}_{ij}$, $\nu_{sgs} = (C_S \Delta)^2 \lvert \bar{s} \rvert$, $\Delta = (\Delta V)^{1/3}$ (eqs. 18.16-18.17); near walls a damping $1 - e^{-x_2^+/26}$ or $\Delta = \min(C_S \Delta,\, \kappa n)$; $C_S$ varies from 0.065 to 0.25 depending on the flow. The Smagorinsky model is the mixing-length model with length $C_S \Delta$ (Section 18.7).

### 20 DES (pp. 215-219)
- DES "is a mix of LES and URANS": the boundary layer is treated by (unsteady) RANS and the outer detached eddies by LES; originally for wings at very high angles of attack (p. 215).
- S-A DES: the wall distance $d$ in the SA destruction term is replaced by $\tilde{d} = \min(d,\, C_{des} \Delta)$, $\Delta = \max(\Delta \xi, \Delta \eta, \Delta \zeta)$, $C_{des} = 0.65$ (eq. 20.3); where $d > C_{des} \Delta$ the destruction grows and $\tilde{\nu}$ falls (LES mode) (pp. 215-216).
- k-omega SST DES: the $k$ dissipation term is multiplied by $F_{DES} = \max(L_t/(C_{DES} \Delta),\, 1)$, $L_t = k^{1/2}/(\beta^* \omega)$, $C_{DES} = 0.61$ (eq. 20.8); the grid decides where the switch happens (p. 218).
- DDES: when the cell size along the wall falls below the boundary-layer thickness, DES switches to LES inside the boundary layer with too coarse a mesh; DDES "protects the boundary layer from the LES mode" by $\tilde{d}_{DDES} = d - f_d \max(0,\, d - C_{DES} \Delta)$ with a shielding function $f_d$ (eqs. 20.9-20.10), and for SST $F_{DDES} = \max(L_t/(C_{DES} \Delta)\, (1 - F_S),\, 1)$ with $F_S = F_1$ or $F_2$ (eq. 20.11) (pp. 218-219).

## Contradictions with the wiki

- In its SST summary (eq. 20.5, p. 217) the notes call $d$ "the distance to the closest wall node", while [[ref-nasa-tmr]] (`spalart.html`) says that finding "the nearest wall gridpoint (or cell center)" is "incorrect" for the minimum distance. Recorded on [[wall-distance]]; external sources only, not the repo docs.

## Pages this source informs

- [[turbulence]], [[energy-cascade-and-kolmogorov-scales]], [[reynolds-averaging]], [[reynolds-stress]], [[boundary-layer]], [[blasius-solution]], [[skin-friction-coefficient]], [[friction-velocity-and-y-plus]], [[law-of-the-wall]], [[wall-functions]], [[mixing-length-model]], [[bradshaw-assumption-and-sst-limiter]], [[sst-blending-functions]], [[les-and-des]], [[k-omega-sst]], [[flow-separation-and-reattachment]]
