---
type: source
tags: [reference, foundation, verification-validation, nasa, cite-only]
sources: []
origin: https://www.grc.nasa.gov/www/wind/valid/tutorial/tutorial.html
licence: none stated on the pages (NASA web pages, curator John W. Slater; footer links NASA web policies)
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# NPARC Alliance CFD Verification and Validation tutorial (NASA Glenn)

**Origin:** NPARC Alliance CFD Verification and Validation web site, tutorial pages, NASA Glenn Research Center, Responsible NASA Official/Curator John W. Slater, all pages "Last updated" Wednesday 10-Feb-2021. Pages read: `tutorial.html` (index), `errors.html` ("Uncertainty and Error in CFD Simulations"), `verassess.html` ("Verification Assessment"), `valassess.html` ("Validation Assessment"), `iterconv.html` ("Examining Iterative Convergence"), `spatconv.html` ("Examining Spatial (Grid) Convergence"), all under https://www.grc.nasa.gov/www/wind/valid/tutorial/
**Kind:** web tutorial; the index says much of it follows AIAA G-077-1998, *Guide for the Verification and Validation of Computational Fluid Dynamics Simulations*, and that the material is still under development.
**Licence:** no licence statement on the pages. Cite only (as [[foundations-sources]] recorded).
**Read:** 2026-11-22, by job p6-roots.

## Summary

A practitioner's account of CFD error sources, the verification and validation assessment steps, iterative convergence checks, and the grid convergence procedure (observed order from three grids, Richardson extrapolation, Roache's GCI with factors of safety, asymptotic-range check), with a worked inlet example.

## Key points by page

### `errors.html` "Uncertainty and Error in CFD Simulations"
- AIAA G-077-1998 terms: *uncertainty* is a potential deficiency due to lack of knowledge (turbulence modelling is the main example); *error* is a recognisable deficiency not due to lack of knowledge.
- Acknowledged errors (physical approximation, round-off, iterative convergence, spatial and temporal discretisation) have procedures to identify and estimate them; unacknowledged errors (programming, usage) do not.
- Round-off: usually insignificant; check with higher precision. Iterative convergence error: scales with the solution's change at the end of the run. Discretisation error: from turning the equations into algebra on a grid; ordered errors vanish as spacing goes to zero; estimated by grid (and time-step) convergence studies. Truncation error is the difference between the PDE and its discrete form; leading even-order terms act as numerical viscosity (dissipative), odd-order as dispersive.
- Local errors are advected and diffused through the grid, so global error is more than their sum.

### `verassess.html` "Verification Assessment"
- AIAA definition: verification determines that "a model implementation accurately represents the developer's conceptual description" of the model and its solution; "solving the equations right".
- Two aspects (after Roache): verification of a code (error evaluation: bugs, wrong implementation; should not use experimental data) and verification of a calculation (error estimation: an error band on one result, usually by grid convergence).
- Process: examine programming, iterative convergence, consistency (e.g. mass conservation through a duct, total pressure not rising through an inlet), spatial and temporal convergence, comparison with analytical and benchmark numerical solutions; code-to-code comparison is stronger when the codes use different methods.

### `valassess.html` "Validation Assessment"
- AIAA definition: validation determines "the degree to which a model is an accurate representation of the real world" for its intended uses; "solving the right equations".
- A code is validated only for the range of applications covered by experimental data; applying it outside that range is *prediction*.
- Steps: iterative convergence, consistency, grid and time convergence, comparison with experimental data (whose bias and random errors must be quantified and shown as error bars), examination of model uncertainties (e.g. run several turbulence models).
- Building-block hierarchy: unit problems (e.g. turbulent flat-plate boundary layer), benchmark cases (e.g. shock/boundary-layer interaction), subsystem cases, complete system cases; data accuracy falls and boundary-condition uncertainty rises up the hierarchy.

### `iterconv.html` "Examining Iterative Convergence"
- Residuals should drop and level off, typically "reduced a certain number (usually 3-4) of orders of magnitude"; also monitor engineering quantities (lift, drag, recovery) until they stop changing; time-accurate runs need convergence within every step.

### `spatconv.html` "Examining Spatial (Grid) Convergence"
- Based on Richardson extrapolation as presented in Roache's book. Error model `E = C h^p`; the observed order is often below the theoretical one (boundary conditions, grid quality, shocks, turbulence models, non-linearities).
- Three grids at constant ratio `r`: `p = ln((f3 - f2)/(f2 - f1)) / ln(r)`. Richardson: `f_exact ~ f1 + (f1 - f2)/(r^p - 1)`, generally `p + 1` order accurate; invalid in discontinuity regions but applicable to integrated functionals.
- Fractional error `E1 = eps / (r^p - 1)`; the plain relative difference alone should not be used, since `r` near 1 makes it artificially small.
- GCI (Roache) `GCI_fine = Fs |eps| / (r^p - 1)`, as a percentage, "a measure of uncertainty of the grid convergence"; `Fs = 3.0` for two-grid comparisons, `1.25` for three or more grids.
- Asymptotic range: `GCI_23 ~ r^p GCI_12`, ratio about 1.0.
- Grid generation: nested grids via `N = 2^n m + 1`; non-integer ratios allowed by regenerating with scaled spacings; recommended `r >= 1.1` to separate discretisation error from iterative and round-off error; an effective ratio from total point counts and dimension for unstructured grids (formula shown only as an image, not read); per-direction GCIs summed when ratios differ.
- Worked example (supersonic inlet pressure recovery, grids at spacing 1, 2, 4): 0.97050, 0.96854, 0.96178; `p ~ 1.786170` (theory 2.0); `Pr(h=0) ~ 0.97130`; GCI_12 = 0.103 %, GCI_23 = 0.356 %; asymptotic ratio 1.002019 (the VERIFY program prints 0.997980); reported `Pr = 0.97130` with a 0.103 % band.

## Contradictions with the wiki

None with rcfd pages. Between references: this page recommends `r >= 1.1`; Celik et al. say it is "desirable" that `r` be greater than 1.3 ([[ref-celik-2008-jfe-discretisation-uncertainty]]). See [[solution-verification]].

## Pages this source informs

[[solution-verification]], [[code-verification-and-manufactured-solutions]], [[validation-comparison]], [[gci-richardson]], [[verification-vs-validation]]
