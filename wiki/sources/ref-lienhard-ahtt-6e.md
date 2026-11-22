---
type: source
tags: [reference, foundation, heat-transfer, textbook, cite-only]
sources: []
origin: https://ahtt.mit.edu/
licence: "copyrighted under U.S. Copyright Law"; free download and printing for personal or non-profit instructional use; copying, distributing or modifying restricted (site terms, read 2026-11-22). Cite only.
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Lienhard and Lienhard, A Heat Transfer Textbook, 6th ed. (2024)

**Origin:** John H. Lienhard IV and John H. Lienhard V, *A Heat Transfer Textbook*, 6th edition, version 6.00, 19 April 2024, Phlogiston Press. PDF: https://ahtt.mit.edu/wp-content/uploads/2024/04/AHTTv600.pdf
**Kind:** open (free-download) undergraduate heat transfer textbook.
**Licence:** copyrighted; personal or non-profit instructional use only. Cite only, no copy in `raw/`.
**Read:** 2026-11-22 by job p6-multiphase-heat, from a text extraction of the PDF (Greek letters and some symbols were lost in the extraction and were restored from the surrounding definitions): sec. 1.3, 2.1, 2.3 (contact resistance), 3.3, 4.1, 5.6, 6.3, 6.4, 6.5, 8.2-8.4, 9.2 (bubble equilibrium and surface tension), 10.1-10.3 and 10.5. Page numbers below are the book's printed pages.

## Summary

The standard engineering ladder of heat transfer: conduction (Fourier's law, the heat diffusion equation, boundary conditions, semi-infinite solutions), convection (Newton's law of cooling, the energy equation with fluid motion, the Prandtl and Nusselt numbers, laminar boundary-layer correlations), natural convection with the Boussinesq approximation and the Grashof and Rayleigh numbers, heat exchangers (effectiveness-NTU) and thermal radiation (Stefan-Boltzmann, view factors, gray bodies, Beer's law).

## Key points by section

### Sec. 1.3 "Modes of heat transfer" (pp. 11-34)
- Fourier's law, 1-D: $q = -k\,dT/dx$ (eq. 1.8); heat flux $q$ in W/m$^2$, thermal conductivity $k$ in W/m K; 3-D form $\vec q = -k\nabla T$. Heat flows from high to low temperature.
- $k$ ranges over a factor of about $10^5$ between gases and diamond at room temperature (Fig. 1.6); copper 398 W/m K.
- 1-D heat conduction equation from Fourier's law plus the First Law: $\partial^2 T/\partial x^2 = \frac{1}{\alpha}\,\partial T/\partial t$ (eq. 1.14) with thermal diffusivity $\alpha = k/(\rho c)$ in m$^2$/s, "a measure of how quickly a material can carry heat away from a hot source".
- Newton's law of cooling $q = \bar h\,(T_{body} - T_\infty)$ (eq. 1.17); $h$ is the film or heat transfer coefficient, W/m$^2$ K; Newton "never wrote such an expression". $h$ is independent of $\Delta T$ in forced convection with modest $\Delta T$; in natural convection it varies as a weak power ($\Delta T^{1/4}$ or $\Delta T^{1/3}$); in boiling about as $\Delta T^2$ (p. 20). Table 1.1: illustrative values of $h$ spanning about six orders of magnitude.
- Biot number $Bi = hL/k_b$ (body conductivity) decides whether a body can be treated as lumped; the Nusselt number $Nu = hL/k_{fluid}$ "relates only to the fluid" (p. 23, footnote 7).
- Radiation: black body, Stefan-Boltzmann $e_b = \sigma T^4$ (eq. 1.28), $\sigma = 5.670374\times10^{-8}$ W/m$^2$K$^4$; Wien $(\lambda T)_{max} = 2897.77$ $\mu$m K (eq. 1.29); net exchange of two black bodies $Q_{net} = A_1\sigma(T_1^4 - T_2^4)$ (eq. 1.31), with a view factor $F_{1-2}$ (eq. 1.32), "the fraction of energy leaving object 1 that is intercepted by object 2"; emittance $\varepsilon$, $0<\varepsilon\le1$ (eq. 1.33); transfer factor $\mathcal F_{1-2}$ (eq. 1.34), equal to $\varepsilon_1$ for a small object in a large isothermal enclosure (eq. 1.35).

### Sec. 2.1 "The heat conduction equation" (pp. 49-56)
- Fourier's law $\vec q = -k\nabla T$ (eq. 2.2); $k = k(\vec r, T)$ in general (eq. 2.3).
- Control-volume energy balance with a volumetric heat release $\dot q$ (W/m$^3$), Gauss's theorem (eq. 2.8), arbitrary region: $\nabla\cdot(k\nabla T) + \dot q = \rho c\,\partial T/\partial t$ (eq. 2.10). Limits: incompressible medium, no relative motion ("Fluid motion is added in Section 6.3"). Constant $k$: $\nabla^2 T + \dot q/k = \frac1\alpha\,\partial T/\partial t$ (eq. 2.11).

### Sec. 2.3 "Contact resistance" (pp. 75-76)
- Two pressed solid surfaces never make perfect contact; the gaps give an extra temperature drop, modelled with an interfacial conductance $h_c$ (W/m$^2$ K): $Q = A h_c\Delta T$, contact resistance $R_t = 1/(h_c A)$. Table 2.1: e.g. copper/copper 10,000-25,000, stainless/stainless 2,000-3,700 W/m$^2$ K.

### Sec. 3.3 "Heat exchanger effectiveness" (pp. 120-125)
- Effectiveness-NTU method, "first developed in full detail in 1955 by Kays and London"; works if the overall coefficient is fairly uniform.
- $\varepsilon = C_h(T_{h,in}-T_{h,out})/[C_{min}(T_{h,in}-T_{c,in})] = C_c(T_{c,out}-T_{c,in})/[C_{min}(T_{h,in}-T_{c,in})]$ (eqs. 3.16, 3.17), so $Q = \varepsilon C_{min}(T_{h,in}-T_{c,in})$. $NTU = UA/C_{min}$ (eq. 3.18), a definition originally made by Nusselt.
- Parallel flow (eq. 3.20) and counterflow (eq. 3.21), with $C_r = C_{min}/C_{max}$: $\varepsilon = [1-e^{-(1+C_r)NTU}]/(1+C_r)$ and $\varepsilon = [1-e^{-(1-C_r)NTU}]/[1-C_r e^{-(1-C_r)NTU}]$. Example 3.5: NTU 1.5, $C_r = 0.5$, parallel flow, $\varepsilon = 0.596$. Table 3.1 lists other configurations.

### Sec. 4.1 "The well-posed problem" (pp. 141-142)
- A conduction problem needs the equation, one initial condition, and two boundary conditions per coordinate: Dirichlet (first kind, $T$ given), Neumann (second kind, normal derivative or heat flux given), third kind (convection, $-k\,\partial T/\partial x|_{bndry} = h(T-T_\infty)_{bndry}$).

### Sec. 5.6 "Transient heat conduction to a semi-infinite region" (pp. 220-235)
- Sudden surface temperature change: the similarity variable $\zeta = x/\sqrt{\alpha t}$ collapses the PDE to an ODE (eq. 5.45); solution $(T-T_\infty)/(T_i-T_\infty) = \mathrm{erf}\big(x/(2\sqrt{\alpha t})\big)$ (eq. 5.50), with Table 5.3 of erf and erfc $= 1-$ erf. 99 % penetration depth $\delta_{99} = 3.64\sqrt{\alpha t}$ (eq. 5.51).
- Example 5.6: Jakob's growth of a vapour bubble in superheated liquid, $R \propto \sqrt t$ (eq. 5.58); the Plesset-Zwick / Scriven result is $\sqrt3$ times larger (eq. 5.59).

### Sec. 6.3 "The energy equation" (pp. 293-298)
- Assumptions: pressure variations too small to affect properties (liquids, gases below about 1/3 of the speed of sound), small density changes ($\nabla\cdot\vec u = 0$), constant $k$, negligible kinetic and potential energy changes, negligible viscous dissipation.
- Result: $\rho c_p\left(\partial T/\partial t + \vec u\cdot\nabla T\right) = k\nabla^2T + \dot q$ (eq. 6.37), "the same as the corresponding equation for a solid body, (2.11), except for the enthalpy transport--or convection--term". Material derivative $DT/Dt$ (eq. 6.38). Boundary-layer form $u\,\partial T/\partial x + v\,\partial T/\partial y = \alpha\,\partial^2T/\partial y^2$ (eq. 6.40).
- $h = q_w/(T_w - T_\infty) = -\frac{k}{T_w-T_\infty}\,\partial T/\partial y|_{y=0}$ (eq. 6.35). For $\nu = \alpha$ the temperature profile equals the Blasius velocity profile: $Nu_x = 0.33206\,Re_x^{1/2}$ (eq. 6.43); properties at the film temperature.

### Sec. 6.4 "The Prandtl number and the boundary layer thicknesses" (pp. 298-302)
- Dimensional analysis: $Nu_x = hx/k$, $Re_x = u_\infty x/\nu$, $Pr = \nu/\alpha = \mu c_p/k$; $Nu_x = fn(Re_x, Pr)$ (eq. 6.44).
- Kinetic theory: Pr of order one for gases; 2/3 for simple monatomic gases, 5/7 for diatomic gases with unexcited vibration; liquids of simple molecules 1-10; liquid metals $10^{-2}$ or less; very complex liquids much larger.
- $\delta > \delta_t$ when $Pr>1$ and $\delta < \delta_t$ when $Pr<1$; $\delta_t/\delta$ is a decreasing function of Pr only (eq. 6.46).

### Sec. 6.5 "Heat transfer coefficient for laminar, incompressible flow over a flat surface" (pp. 302-313)
- Integral method: $\delta_t/\delta \approx Pr^{-1/3}$ for $Pr \gtrsim 0.6$ (eq. 6.55); $Nu_x = 0.332\,Re_x^{1/2}Pr^{1/3}$, $Pr \ge 0.6$ (eq. 6.58), within 2 % of the exact solution; liquid-metal limit $Nu_x = 0.565\,Pe_x^{1/2}$ (eq. 6.62); average $\overline{Nu}_L = 0.664\,Re_L^{1/2}Pr^{1/3}$ (eq. 6.68).

### Sec. 8.2-8.4 "Natural convection" (pp. 414-441)
- Gravity term added to the boundary-layer momentum equation (eq. 8.2); with the hydrostatic pressure gradient the driving term is $(1-\rho_\infty/\rho)g$ (eq. 8.4).
- Thermal expansion coefficient $\beta = -\frac1\rho(\partial\rho/\partial T)_p \approx -\frac1\rho\,\frac{\rho_\infty-\rho}{T_\infty-T}$ (eq. 8.7); $\beta = 1/T$ for an ideal gas. The momentum equation becomes $u\,\partial u/\partial x + v\,\partial u/\partial y = -g\beta(T-T_\infty) + \nu\,\partial^2u/\partial y^2$ (eq. 8.8) for the plate orientation of Fig. 8.1a. Footnote 2 (p. 417): "The use of constant properties except for density in the buoyancy term is called the Boussinesq approximation".
- Groups: $Gr_L = g\beta\Delta T L^3/\nu^2$ (eq. 8.9), $Nu_L = fn(Gr_L, Pr)$ (eq. 8.10); Rayleigh number $Ra_L = Gr_L Pr = g\beta\Delta T L^3/(\nu\alpha)$ (eq. 8.11); $Nu = fn(Ra, Pr)$ with Pr secondary (eq. 8.12). Vertical plate: the boundary layer turns turbulent about where $Ra_L/[1+(0.492/Pr)^{9/16}]^{16/9} \approx 10^8$; Churchill-Chu laminar correlation for $Ra_L \lesssim 10^9$ (eq. 8.13a).

### Sec. 9.2 "Nucleate boiling" (pp. 478-481)
- An equilibrium vapour bubble in superheated liquid: surface tension raises the inside pressure, $R_b = 2\sigma/[p_{sat}(T_{sup}) - p_{ambient}]$ (eq. 9.1); the equilibrium is unstable. Surface tension of water (eq. 9.2a), Table 9.1 for other liquids, in mN/m.

### Sec. 10.1-10.3 Radiation (pp. 543-567)
- Gray body: emittance independent of wavelength; diffuse emission and reflection have no preferred direction (pp. 548-549). Kirchhoff's law, diffuse gray form $\varepsilon = \alpha$ (eq. 10.8c).
- View factor summation $1 = F_{1-1} + F_{1-2} + \dots + F_{1-n}$ (eq. 10.12) and reciprocity $A_1F_{1-2} = A_2F_{2-1}$ (eq. 10.15); integral definition (eq. 10.16). Radiosity $B = \rho H + \varepsilon e_b$ (eq. 10.18).

### Sec. 10.5 "Gaseous radiation" (pp. 581-594)
- Gases scatter (Rayleigh, Mie) and absorb in bands. Along a ray $di_\lambda = -\rho\kappa_\lambda i_\lambda\,dx$ (eq. 10.47) with monochromatic absorption coefficient $\kappa_\lambda$; integrated, Beer's law $i_\lambda(L) = i_\lambda(0)\,e^{-\rho\kappa_\lambda L}$ (eq. 10.48); transmittance (eq. 10.49), absorptance $1-e^{-\rho\kappa_\lambda L}$ for a non-scattering gas (eq. 10.50); $\rho\kappa_\lambda L$ is the optical depth. Emission plus absorption along a ray (eqs. 10.52, 10.53).

## Contradictions with the wiki

None. The effectiveness definition (eq. 3.17) and the parallel and counterflow formulas (eqs. 3.20, 3.21) agree with the forms that [[doc-reference-data-wave23a-mhd-heat-exchanger]] and [[doc-reference-data-lumped-radiation]] record as "citation UNVERIFIED (memory)".

## Pages this source informs

[[heat-conduction]], [[energy-equation]], [[convective-heat-transfer]], [[boussinesq-buoyancy-approximation]], [[conjugate-heat-transfer]], [[thermal-radiation]], [[heat-exchanger-effectiveness-ntu]], [[surface-tension-and-curvature]]
