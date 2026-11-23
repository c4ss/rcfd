---
type: concept
tags: [numerics, simple, piso, symmetry, default]
sources: [doc-component-diagonal-default, doc-dynamic-mesh, adr-thread-piso-stability-consistent-correctors]
created: 2026-11-22
builds_on: [momentum-matrix-and-h-operator, boundary-conditions]
updated: 2026-11-22
status: draft
---

# Component diagonal (`solver.component_diagonal`)

The choice of momentum-matrix diagonal used for $\mathrm{HbyA} = H/a_P$ and the pressure equation in rcfd's segregated pressure-based kernels. `averaged` uses the mean of the velocity-component diagonals; `corrected` adds $(a_P - a_k)\, u_k$ to $H_k$ (ADR 0010), which removes the spurious sink that symmetry-type boundaries add to the tangential velocity ([[doc-component-diagonal-default]]).

## How rcfd uses it

- **The defect:** where a condition acts on one component only (`symmetry`, `shear`, `partial_slip`, `marangoni`), the converged tangential velocity carries an extra sink $(a_P - a_t)\, u_t$ that "acts like a partial no-slip wall" ([[doc-component-diagonal-default]]).
- **Defaults per kernel:** SIMPLE/SIMPLEC, PISO/PIMPLE and variable density: `averaged`. MRF zones: always corrected. VOF: own key `multiphase.component_diagonal`, `corrected` by default since ADR 0114. Coupled and density-based solvers: not affected ([[doc-component-diagonal-default]]).
- **Where it matters:** diffusion-dominated cells next to a symmetry-type plane (half channels, wall-resolved meshes touching a symmetry plane, low Re). Laminar half channel u_max 12.75 / 15.57 vs 50 with `averaged`; SST half channel bulk velocity -73 percent; on a coarse high-Re symmetry top the change is about 1e-6 ([[doc-component-diagonal-default]]).
- **Transient:** Stokes' second problem impedance +32 percent real / -51 percent imaginary with `averaged` (F-531, ADR 0455); relaxed PIMPLE can cancel or reverse the defect "by accident" (F-538, ADR 0613); one-cell-deep annuli with `symmetry` planes deviate $2.4\times 10^{-2}\,\lvert U\rvert$ after one PISO step (F-712, ADR 0650) ([[doc-component-diagonal-default]]).
- **Cost of flipping the default:** exactly one registered baseline hash moves, `packed_bed` (60, 19e53df373637262) -> (16, fb46cdfee5d0d781); 11 `rcfd-api` tests change outcome (ADR 0650) ([[doc-component-diagonal-default]]). See [[bit-identical]].
- The dynamic-mesh page lists "`Symmetry` boundaries with the averaged diagonal produce a spurious tangential force (use `zero_gradient` sides)" as a known limitation ([[doc-dynamic-mesh]]).
- **Transient evidence (F-531, F-538):** next to `Symmetry` planes the averaged diagonal gives an O(1) error in the tangential viscous flux that grows under refinement, and in the relaxed PIMPLE loop its error "can have either sign and depends on $\alpha_u$, the time step and the flow-direction spacing"; `corrected` as the transient default with `packed_bed` re-registered was "Held" by the maintainer on 2026-11-18 ([[adr-thread-piso-stability-consistent-correctors]]).

## Builds on

- [[momentum-matrix-and-h-operator]]: $a_P$, $H(U)$ and $\mathrm{HbyA} = H/a_P$ per velocity component.
- [[boundary-conditions]]: conditions that act on one velocity component only (symmetry and its kin) make the component diagonals differ.

## Related

- [[simple]]
- [[piso]]
- [[mrf]]
- [[bit-identical]]
- [[sliding-mesh]]

## Open questions

- **Decision pending (the maintainer's):** keep `averaged` (failure is silent), make `corrected` the default (re-register `packed_bed`, reword tests), or keep `averaged` with a run-time warning (not implemented). The default is unchanged as of the 2026-11-16 update ([[doc-component-diagonal-default]]).
