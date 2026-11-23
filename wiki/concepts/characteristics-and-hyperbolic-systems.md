---
type: concept
tags: [foundation, compressible, hyperbolic, boundary-conditions]
sources: [ref-van-leer-1992-icase-92-43, ref-hu-1995-icase-95-70, ref-naca-1953-report-1135, ref-farassat-2007-nasa-tm-214853, ref-chima-1998-nasa-tm-206613]
builds_on: [euler-equations]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Characteristics and hyperbolic systems

## In short

In a hyperbolic system such as the Euler equations, information travels as waves with finite speeds, the eigenvalues of the flux Jacobian. Any small jump in the state can be split into these waves, each moving left or right. This tells a numerical scheme which side to take information from (upwinding) and tells a boundary condition which waves it may set and which it must let out ([[ref-van-leer-1992-icase-92-43]] sec. 2.1; [[ref-hu-1995-icase-95-70]] sec. 1).

## Explanation

**Wave decomposition.** Write the system as $U_t + F(U)_x = 0$ and define the Jacobian $A(U) = \partial F/\partial U$, so $dF = A\,dU$ ([[ref-van-leer-1992-icase-92-43]] eqs. 4-5). For the Euler equations $A$ has real eigenvalues $\lambda_k$ and a full set of eigenvectors $R_k$; a difference between two neighbouring states splits as

$$\Delta U = \sum_k \alpha_k R_k, \qquad \Delta F = \sum_k \lambda_k \alpha_k R_k$$ (van Leer 1992, eqs. 10-11)

where $\alpha_k$ is the strength of wave $k$ and $\lambda_k$ its speed. Waves with $\lambda_k > 0$ move forward and those with $\lambda_k < 0$ backward; splitting $A = A^+ + A^-$ into its positive and negative parts collects them (eqs. 7-8, 12) ([[ref-van-leer-1992-icase-92-43]] sec. 2.1). A system with this structure (real speeds, complete eigenvectors) is what "hyperbolic" means here: a disturbance propagates at finite speed along these directions rather than being felt everywhere at once.

**The Euler waves.** Linearised about a uniform mean flow, the Euler equations "support acoustic waves, which travel with the speed of sound relative to the mean flow, and vorticity and entropy waves, which travel with the mean flow" ([[ref-hu-1995-icase-95-70]] sec. 1). In one dimension with flow speed $u$ and sound speed $a$ this gives the three speeds $u - a$, $u$, $u + a$: two acoustic waves and one entropy wave (a reading of [[ref-hu-1995-icase-95-70]] sec. 1 and the three-wave sum of [[ref-van-leer-1992-icase-92-43]] eqs. 10-12). In subsonic flow ($|u| < a$) one acoustic wave runs against the flow; in supersonic flow all three run downstream.

**Characteristic lines and cones.** In steady supersonic flow, small disturbances lie on Mach lines at $\mu = \sin^{-1}(1/M)$ to the flow, and an expansion round a corner is bounded by Mach lines (Prandtl-Meyer fan) ([[ref-naca-1953-report-1135]] p. 613 symbols, p. 626 figs. 7-8). For the acoustic wave equation the set of source points that can affect an observer at $(\mathbf{x}, t)$ is a sphere shrinking at speed $c$ toward $\mathbf{x}$, "the characteristic cone of the wave equation with the vertex at (x, t)", pointing into the past by causality ([[ref-farassat-2007-nasa-tm-214853]] Remark 3).

**Upwinding.** The forward-moving part of a jump updates the node downstream of it and the backward-moving part the node upstream: this is the basis of upwind and fluctuation-splitting schemes ([[ref-van-leer-1992-icase-92-43]] sec. 2.1, eq. 12) and of the Riemann-solver fluxes in [[riemann-problem-and-godunov-method]].

**Boundaries.** At an artificial boundary only the waves that enter the domain carry information from outside; waves that leave carry the interior solution and should pass out without reflection. Outflow boundary conditions "based on the characteristics of the Euler equations" are one of the standard non-reflecting techniques ([[ref-hu-1995-icase-95-70]] sec. 1). In turbomachinery, Giles' non-reflecting conditions decompose the boundary solution circumferentially into Fourier modes and treat the zeroth (mean) mode by 1-D characteristic theory ([[ref-chima-1998-nasa-tm-206613]] introduction). Plain 1-D characteristic conditions treat waves as arriving normal to the boundary; see [[non-reflecting-boundaries-and-sponge-layers]] for what happens at oblique incidence.

**What it adds.** The [[euler-equations]] give fluxes; this page adds the wave picture hidden in them: how many waves, how fast, in which direction. That picture decides how fluxes are computed between cells and how many quantities a boundary may prescribe.

## Builds on

- [[euler-equations]]: the conservation-form system whose flux Jacobian supplies the wave speeds.

## Leads to

- [[riemann-problem-and-godunov-method]], [[non-reflecting-boundaries-and-sponge-layers]], [[shock-waves]]
- [[adr-thread-compressible-acoustic-boundaries]]

## How rcfd uses it

- The density-based Euler solver's boundaries `TotalInlet`, `PressureOutlet`, `NonReflecting`, `FarField` come from 1-D characteristic analysis (ADR 0006); "Only normally incident waves are treated as non-reflecting; oblique incidence reflects" ([[adr-thread-early-physics-models]]).
- In the case file these are the `compressible_bc` kinds `total_inlet`, `pressure_outlet`, `nonreflecting` (`reflection_value` default `0.02`) and `farfield` ([[doc-case-schema-compressible]]).
- The characteristic (LODI) treatment and the pressure-relaxation outlet $K\,(p - p_{ref})$ are cited to Thompson (1987) and Poinsot and Lele (1992); the formulas were re-derived in ADR 0006, "not checked against the papers, unverified"; the Riemann-invariant far field and total-pressure inlet are cited to Blazek, "book not accessed here, unverified" ([[doc-reference-data-compressible-bcs]]).

## Open questions

- None of the sources read here states the counting rule (how many conditions a subsonic or supersonic inflow and outflow boundary must prescribe) or the Riemann invariants of the 1D Euler equations; both remain to be sourced from an open reference.
