---
type: concept
tags: [foundation, discretisation, limiter, unstructured]
sources: [ref-berger-aftosmis-murman-2005-slope-limiters, ref-venkatakrishnan-1995-icase-95-3]
builds_on: [gradient-reconstruction, tvd-schemes-and-flux-limiters]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Slope limiters on unstructured meshes (Barth-Jespersen, Venkatakrishnan)

## In short

On an unstructured mesh there is no "previous" and "next" cell along a line, so 1D flux limiters do not carry over directly. Instead each cell's gradient is scaled down until the linear reconstruction, evaluated at the cell's faces, stays within the range of the neighbouring cell values. Barth and Jespersen's limiter does exactly this; Venkatakrishnan's variant smooths it to help convergence, at some cost in strict monotonicity ([[ref-venkatakrishnan-1995-icase-95-3]]; [[ref-berger-aftosmis-murman-2005-slope-limiters]]).

## Explanation

**Slope limiting.** Limiting the flux is equivalent to limiting the slope (Spekreijse), the natural form for finite volumes: reconstruct `U_i(x) = u_i + phi_i grad u_i . (x - x_i)` with a factor `0 <= phi_i <= 1`, evolve, project back to cell averages ([[ref-berger-aftosmis-murman-2005-slope-limiters]] II.B, Eqns. 4-8).

**Barth-Jespersen.** Barth and Jespersen required "that the reconstructed distribution in the control volume be bounded everywhere by the values of the neighbors" and built "a truly multi-dimensional limiter" from that monotonicity principle ([[ref-venkatakrishnan-1995-icase-95-3]] 3). Geometrically: at an extremum the gradient is set to zero; otherwise the reconstruction at each face must not pass the adjacent neighbour's value. On uniform 1D grids this is the algebraic Barth-Jespersen limiter, first proposed by van Leer and known elsewhere as monotonized central differences ([[ref-berger-aftosmis-murman-2005-slope-limiters]] III.A). It preserves linear data on stretched 1D meshes where van Leer, van Albada and min limiters activate (at stretching 1.2 they give 0.99, 0.98 and 0.90 instead of 1) ([[ref-berger-aftosmis-murman-2005-slope-limiters]] III.C).

**Convergence problems.** The Barth-Jespersen limiter "may be thought of as a generalization of the min-mod limiter and as such, leads to convergence difficulties" ([[ref-venkatakrishnan-1995-icase-95-3]] 3). On the ONERA M6 wing every limiter converged except Barth-Jespersen, "which stalls after converging roughly three orders of magnitude" ([[ref-berger-aftosmis-murman-2005-slope-limiters]] V, Fig. 12). Non-smooth limiters cause "limiter chatter" that blocks steady convergence with time marching or Newton's method ([[ref-berger-aftosmis-murman-2005-slope-limiters]] I).

**Venkatakrishnan's modification.** Venkatakrishnan analysed the convergence problem and "proposed modifications that ameliorate the situation at the expense of monotonicity"; Aftosmis et al. found they "significantly improve the convergence as well as solution accuracy" ([[ref-venkatakrishnan-1995-icase-95-3]] 3). The formula itself is not given in the sources read for this page.

**Scalar against directional limiting.** One factor per cell (scalar limiting) is "far more severe than their vector counterparts, since limiting triggered by any face of a polyhedral cell degrades the gradient in all directions"; per-component limiting is simple on cells with orthogonal faces, but on general cells limiting for one face can violate another face's bound; the best limited gradient is a linear-programming problem over the face constraints ([[ref-berger-aftosmis-murman-2005-slope-limiters]] IV.A-D).

**Cost in accuracy.** On ONERA M6 the computed drag rises with limiter dissipation: no limiter 3.24e-3, Barth-Jespersen 4.56e-3, van Leer 5.41e-3, van Albada 6.15e-3, min 8.15e-3 ([[ref-berger-aftosmis-murman-2005-slope-limiters]] V, Table 1).

## Builds on

- [[gradient-reconstruction]]: the gradient being limited.
- [[tvd-schemes-and-flux-limiters]]: the 1D limiter theory this generalises.

## Leads to

- rcfd pages: [[mrf]], [[fan-01]], [[divergence-guard]], [[doc-case-schema-linear-upwind-limiter]], [[doc-case-schema-gradient-options]]

## How rcfd uses it

- `solver.linear_upwind_limiter = "barth_jespersen"` applies a Barth-Jespersen factor per velocity component before the linear-upwind extrapolation; it made the FAN-01 sector mesh stable where the unlimited scheme diverged at iteration 7 and was forced on in 542 steady runs without a new divergence-guard trip ([[doc-case-schema-linear-upwind-limiter]]; [[divergence-guard]]); it removes the NACA 0012 trailing-edge divergence at y+ 2 ([[doc-case-schema-linear-upwind-limiter]]).
- **FAN-01 limit cycle:** the steady FAN-01 run stalls in a limiter cycle (ADRs 0110, 0111) ([[mrf]]); the wave-49 diagnosis found the Barth-Jespersen limit cycle on B3 driven by the convection deferred correction at two sites, and no automatic first-order fallback, bounded second-order scheme or hub mesh change converged it ([[fan-01]]). This is the stall behaviour the sources above describe for Barth-Jespersen ([[ref-berger-aftosmis-murman-2005-slope-limiters]] V).
- The gradient options offer `barth_jespersen` or `venkatakrishnan` as a secondary limiter with `gradient_limiter_k = 5.0`, "ADR 0016's C2 value"; "no other value was ever checked" ([[doc-case-schema-gradient-options]]; [[adr-thread-gradient-options]]).

## Open questions

- Which form of the Venkatakrishnan limiter rcfd implements, and what `gradient_limiter_k` means in it, is not recorded in the wiki; the sources read here give no formula.
