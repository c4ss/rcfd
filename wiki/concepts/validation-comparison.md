---
type: concept
tags: [foundation, verification-validation]
sources: [ref-oberkampf-trucano-2002-sand2002-0529, ref-nparc-vv-tutorial, ref-celik-2008-jfe-discretisation-uncertainty, ref-mfix-vv-manual]
builds_on: [measurement-uncertainty, solution-verification]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Validation comparison (computation against measurement)

## In short

Validation asks how well a model represents the real world for its intended use, by comparing computed results with measurements. A fair comparison accounts for three things at once: the measurement's uncertainty, the computation's numerical error, and what is left, the modelling error. Agreement validates particular results in the range the data cover; it does not validate a code in general ([[ref-oberkampf-trucano-2002-sand2002-0529]] sec. 2.1, 4.1; [[ref-nparc-vv-tutorial]] `valassess.html`).

## Explanation

**Definition.** Validation: "The process of determining the degree to which a model is an accurate representation of the real world from the perspective of the intended uses of the model" (AIAA Guide, in [[ref-oberkampf-trucano-2002-sand2002-0529]] sec. 2.1, p. 14); "solving the right equations" ([[ref-nparc-vv-tutorial]]; [[ref-mfix-vv-manual]] sec. 1.1). Verification comes first: "Verification is a mathematics issue; not a physics issue" (Roache 2009, in [[ref-mfix-vv-manual]] sec. 1.1).

**Decomposing the discrepancy.** For a quantity `u` both computed and measured ([[ref-oberkampf-trucano-2002-sand2002-0529]] sec. 4.1.2, Eq. 12-14):

`u_nature - u_discrete = E1 + E2 + E3 + E4`

- `E1 = u_nature - u_exp`: measurement error, random and bias ([[measurement-uncertainty]]);
- `E2 = u_exp - u_exact`: modelling error, the gap between the measurement and the exact solution of the model equations, e.g. continuum, incompressibility at low speed, constant turbulent Prandtl number, turbulence model, or inputs that were not measured;
- `E3`, `E4`: numerical error of the discrete solution ([[solution-verification]]).

Only after `E1`, `E3` and `E4` are estimated can a difference be read as a statement about the model `E2`. Unconverged numerical error can dominate the discrepancy or produce apparent agreement ([[ref-oberkampf-trucano-2002-sand2002-0529]] sec. 3.2.1).

**Validation metrics.** Oberkampf and Trucano recommend that a metric include the numerical error estimate, but not grouped with the experimental uncertainty nor treated probabilistically, since "Numerical error represents a bias error in the solution"; exclude no modelling assumption; include random and correlated bias errors of the data; depend on the number of replications; and propagate input uncertainty through the model. They call Coleman and Stern's metric, under which a difference smaller than the metric means "validated", conceptually flawed in certain respects ([[ref-oberkampf-trucano-2002-sand2002-0529]] sec. 4.7.1).

**What can be claimed.** "Individual computational outcomes of a model are validated; codes are not validated" ([[ref-oberkampf-trucano-2002-sand2002-0529]] sec. 4.1.1). A model is validated only for the range of applications the experimental data cover; outside it, a computation is a *prediction* ("use of a computational model to foretell the state of a physical system under conditions for which the computational model has not been validated") ([[ref-oberkampf-trucano-2002-sand2002-0529]] sec. 4.1.1; [[ref-nparc-vv-tutorial]]). "Reasonable agreement" with experimental data alone does not justify a single-grid calculation ([[ref-celik-2008-jfe-discretisation-uncertainty]], editorial criterion 10).

**Choosing experiments.** A building-block hierarchy from unit problems (one physical feature, detailed accurate data, e.g. a turbulent flat-plate boundary layer) through benchmark and subsystem cases to complete systems, where data are sparser and boundary conditions less certain ([[ref-nparc-vv-tutorial]] `valassess.html`). Model uncertainty can be explored by repeating the case with several turbulence models (same page).

**What it adds.** [[measurement-uncertainty]] bounds the data and [[solution-verification]] bounds the numerics; validation comparison puts both next to the computed-minus-measured difference so that what remains can be attributed to the model.

## Builds on

- [[measurement-uncertainty]]: the measurement side of the comparison (`E1`).
- [[solution-verification]]: the numerical side (`E3`, `E4`).

## Leads to

- [[verification-vs-validation]] (rcfd's three kinds of checking), [[validation-data-pack]].

## How rcfd uses it

- **Measured versus computed references:** the cylinder mean-drag band of F-869 is built from eleven values, "all are computations, none is a measurement" ([[verification-vs-validation]]).
- **Not validated:** the 0.1.0 changelog says rcfd "has not been validated for engineering decisions" and "No fan prediction in this release is validated against measurement"; the design loop ranks "variants of the model, not real fans" ([[verification-vs-validation]]).
- **Numerical band added, not in quadrature:** the F-870 numerical band "is added, not combined in quadrature, to the measurement uncertainty" (ADR 0762) ([[gci-richardson]]). This matches Oberkampf and Trucano's view that numerical error is a bias and should not be grouped probabilistically with experimental uncertainty ([[ref-oberkampf-trucano-2002-sand2002-0529]] sec. 4.7.1); the ADR thread page does not say this was the reason.
- **Measured data in the repo:** see [[validation-data-pack]].
