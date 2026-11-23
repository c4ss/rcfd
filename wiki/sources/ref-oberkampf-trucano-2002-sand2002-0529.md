---
type: source
tags: [reference, foundation, verification-validation, sandia, cite-only]
sources: []
origin: https://www.osti.gov/biblio/793406
licence: cover states "Approved for public release; further dissemination unlimited." (DOE-sponsored Sandia report); no copyright licence stated, so cite only
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Oberkampf and Trucano, "Verification and Validation in Computational Fluid Dynamics", SAND2002-0529

**Origin:** William L. Oberkampf and Timothy G. Trucano, *Verification and Validation in Computational Fluid Dynamics*, Sandia Report SAND2002-0529, Unlimited Release, printed March 2002, Sandia National Laboratories, OSTI 793406, doi:10.2172/793406; full text https://www.osti.gov/servlets/purl/793406 (122 pages).
**Kind:** review report.
**Licence:** "Approved for public release; further dissemination unlimited." plus the standard US Government sponsorship notice. That is a release statement, not a copy licence; cite only.
**Read:** 2026-11-22, by job p6-roots: abstract, contents, sec. 2.1 (definitions), 3.2.1, 3.2.3, 3.3.1, the algorithm-testing part of 3.4.3 (pp. 49-51), 4.1.1, 4.1.2 and 4.7.1. Text extracted with `pdftotext`; page numbers are the report's printed page numbers.

## Summary

The standard review of V&V terminology and practice for CFD: AIAA definitions, code versus calculation verification, error versus uncertainty, convergence of discretisations, benchmark and manufactured solutions, the decomposition of a validation discrepancy into experimental, modelling and numerical parts, and the recommended features of a validation metric.

## Key points by section

### Sec. 2.1 (pp. 14-15) definitions
- AIAA Guide: *Verification*: "The process of determining that a model implementation accurately represents the developer's conceptual description of the model and the solution to the model." *Validation*: "The process of determining the degree to which a model is an accurate representation of the real world from the perspective of the intended uses of the model."
- Both are ongoing processes without a defined completion point; correctness cannot be demonstrated for all conditions; "models of physics cannot be proven correct, they can only be disproved"; V&V can only assess the specific cases tested.
- Accuracy in verification is measured against benchmark solutions (analytical or highly accurate numerical); in validation against experimental data, which itself has random and bias errors.

### Sec. 3.2.1 and 3.2.3 (pp. 24-29)
- Validation depends on solution accuracy: unconverged numerical error can dominate or mask the computed-versus-measured discrepancy, and a coding error can be confused with a modelling error.
- After Roache: code verification versus verification of calculations; calculation verification needs grid convergence, which may not be attainable.
- AIAA error: "A recognizable deficiency in any phase or activity of modeling and simulations that is not due to lack of knowledge." Unacknowledged errors (blunders: programming, input, compiler) have no straightforward estimation method; acknowledged errors (finite precision, modelling approximations, discretisation) can be measured in principle.
- Eq. 2 (p. 29): the numerical error splits into the error of the exact solution of the discrete equations (zero for strongly consistent, stable schemes in simple cases) and the difference between that limit and the actual discrete solution on a given grid, time step, iteration level and computer.

### Sec. 3.3.1 (pp. 31-32) convergence of discretisations
- Truncation error is introduced by the discrete approximation; discretisation error is the difference between the discrete solution and the exact PDE solution. Strongly consistent: zero truncation error as grid spacing and time step go to zero (not guaranteed with shocks).
- Eq. 3: $\lVert u_\text{exact} - u_{h,\tau} \rVert = O(h^p, \tau^q)$; the questions are whether the discrete solution converges, what the observed $p$ and $q$ are, and what error is observed on finite grids.

### Sec. 3.4.3 (pp. 49-51) algorithm testing, benchmarks and MMS
- Highly accurate solutions: analytical solutions, benchmark ODE solutions (Blasius, Taylor-Maccoll, stagnation flow), benchmark PDE solutions (flat plate, lid-driven cavity, natural-convection cavity, cylinder, backward-facing step). No published solution should be a benchmark until computed independently, preferably with different methods. "code comparisons are a verification activity--NOT a validation activity".
- MMS: assume a solution, insert it into the PDE, collect the excess terms as a source term, add it to the PDE; boundary conditions from the assumed function (Dirichlet, Neumann or third kind). It verifies, for a narrow range of physics, the numerical method, differencing, grid transformation and coding. Salari and Knupp applied it to compressible and incompressible Navier-Stokes and tested 21 coding mistakes: MMS "correctly diagnosed every error in the set that prevented the governing equations from being solved correctly"; it cannot detect efficiency mistakes.

### Sec. 4.1.1-4.1.2 (pp. 56-60) validation, prediction, error decomposition
- "Individual computational outcomes of a model are validated; codes are not validated". AIAA *prediction*: "use of a computational model to foretell the state of a physical system under conditions for which the computational model has not been validated."
- Eq. 12-14: $u_\text{nature} - u_\text{discrete} = E_1 + E_2 + E_3 + E_4$, with $E_1 = u_\text{nature} - u_\text{exp}$ (measurement error, incl. bias), $E_2 = u_\text{exp} - u_\text{exact}$ ("modeling error", e.g. continuum, incompressibility at low speed, constant turbulent Prandtl number, turbulence models; also unmeasured inputs), $E_3$, $E_4$ the numerical parts of sec. 3.2. The authors consider bias (systematic) error the more important part of $E_1$.

### Sec. 4.7.1 (pp. 82-83) recommended features of a validation metric
- Include an estimate of numerical error, but not grouped with experimental uncertainty nor represented probabilistically: "Numerical error represents a bias error in the solution".
- Exclude no modelling assumption; include random experimental error and correlated bias errors; depend on the number of experimental replications (confidence in the mean, not just scatter); propagate input uncertainty nondeterministically. Coleman and Stern's metric (difference below the metric means "validated") is called conceptually flawed in certain respects.

## Contradictions with the wiki

None found.

## Pages this source informs

[[code-verification-and-manufactured-solutions]], [[solution-verification]], [[validation-comparison]], [[verification-vs-validation]], [[measurement-uncertainty]]
