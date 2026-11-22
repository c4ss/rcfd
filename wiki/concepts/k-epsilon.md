---
type: concept
tags: [turbulence, rans, two-equation, wall-functions]
sources: [doc-case-schema-turbulence, adr-thread-transient-rans, adr-thread-periodic-seam-diffusivity, doc-roadmap-json, ref-mit-ocw-2-29-turbulence, ref-menter-1992-nasa-tm-103975]
builds_on: [two-equation-turbulence-models, wall-functions]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# k-epsilon

The two-equation RANS model transporting the turbulent kinetic energy k and its dissipation rate epsilon, with wall functions at the wall. rcfd has the standard, RNG and realizable variants (`turbulence.model = "k_epsilon"`, `variant`); k-epsilon walls must be at rest ([[doc-case-schema-turbulence]]).

## In short

*(added by p6-turbulence)* The standard k-epsilon model (Launder and Spalding 1974) sets $\nu_t = C_\mu k^2/\varepsilon$ and transports $k$ and $\varepsilon$, with the epsilon equation built on the "equilibrium turbulent flows" idea that dissipation balances production; constants $C_\mu = 0.09$, $C_1 = 1.44$, $C_2 = 1.92$, $\sigma_k = 1.0$, $\sigma_\varepsilon = 1.3$. At high Reynolds number the wall is handled by wall functions, which fix $k$ and $\varepsilon$ in the wall cell from the log law ([[ref-mit-ocw-2-29-turbulence]] L25 s.20-23). Its known weaknesses are a "lack of sensitivity to adverse pressure-gradients", where it "predicts significantly too high shear-stress levels and thereby delays (or completely prevents) separation", and the damping functions and non-zero wall $\varepsilon$ that low-Re versions need ([[ref-menter-1992-nasa-tm-103975]] p. 2). Rewritten in k-omega variables it is the outer half of SST ([[sst-blending-functions]]).

## Builds on

*(added by p6-turbulence)*
- [[two-equation-turbulence-models]]: the k and epsilon transport equations, production, destruction, eddy diffusion.
- [[wall-functions]]: the standard wall treatment that sets the wall shear, $k$ and $\varepsilon$ in the first cell.
- Further down: [[boussinesq-eddy-viscosity]], [[energy-cascade-and-kolmogorov-scales]] (meaning of $k$ and $\varepsilon$), [[law-of-the-wall]].

## How rcfd uses it

- **Validation state (F-042 partial):** on BFS-C (window `6.16` to `6.36`) Standard `5.350` and Realizable `6.413` miss, RNG passes marginally; scalable wall functions are not wired to the settings tree ([[roadmap-turbulence]], from [[doc-roadmap-json]]).
- **Transient (F-294, F-300 partial):** k-epsilon runs on the outer iterations of `pimple` (ADR 0210), but its BDF2 time order is a known miss whose cause is not found: $p$ `0.8031` / `1.4814` / `1.4287` even with the smooth Spalding wall eddy viscosity, so "the kink is not the cause" ([[adr-thread-transient-rans]]).
- **Wall eddy viscosity law:** library option `KEpsilonModel::set_option("wall_nut_law", ..)` (Spalding); no case key ([[adr-thread-transient-rans]]).
- **Periodic seams:** the pair diffusivity applies to the momentum $\nu + \nu_t$ and to k and epsilon $\nu + \nu_{t,f} / \sigma$ (F-296) ([[adr-thread-periodic-seam-diffusivity]]).
- **The F-040 wall functions** (`WallConstants` kappa `0.41`, E `9.8`, Cmu `0.09`) are the constants the models share; the F-040 channel recovers the log law with a mixing-length interior, "not yet with k-epsilon" ([[adr-thread-transient-rans]]).

## Related

- [[k-omega-sst]], [[spalart-allmaras]], [[rcfd-turb]], [[backward-facing-step]], [[roadmap-turbulence]]

## Open questions

- The cause of the BDF2 order miss (F-300) ([[adr-thread-transient-rans]]).
