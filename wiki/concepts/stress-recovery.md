---
type: concept
tags: [foundation, structural, fem, post-processing]
sources: [ref-felippa-ifem, ref-roylance-mit-3-11-mechanics-of-materials, adr-thread-structural-basic-tier, adr-thread-structural-verification-elements]
builds_on: [finite-element-method-basics, a-posteriori-error-indicators]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Stress recovery (Gauss-point stresses, nodal averaging, patch recovery)

## In short

A displacement-based finite element solution gives displacements directly; stresses are derived by differentiating and are less accurate, and they jump between elements. Stress recovery is the post-processing that turns element stresses into usable values: evaluate at the Gauss points, where they are most accurate, extrapolate to the nodes and average. The difference between raw and recovered stresses also indicates the discretisation error ([[ref-felippa-ifem]] ch. 28).

## Explanation

**Accuracy.** Stresses are derived quantities: "if the accuracy level of displacements is 1% that of the stresses may be typically 10% to 20%" (Felippa, sec. 28.1). Element strains come from $\mathbf e = \mathbf B\,\mathbf u^{(e)}$ and stresses from the material law (sec. 28.2) ([[ref-felippa-ifem]]).

**Where to sample.** For distorted quadrilaterals, evaluating at the Gauss points and extrapolating to the nodes (bilinear extrapolation through a "Gauss element") usually gives better nodal values than evaluating at the nodes directly, supported by superconvergence results; for rectangles there is no difference (secs. 28.2-28.4) ([[ref-felippa-ifem]]).

**Nodal averaging.** Adjacent elements give different stresses at a shared node, because displacement-assumed elements do not make stresses continuous; the values are averaged to "nodal point stresses" (sec. 28.5) ([[ref-felippa-ifem]]).

**A caution from an example.** For a plate with a hole (Kirsch concentration 3.00), the integration-point stress was 3.26, 9 percent high; the post-processor's least-squares plane through an element's four Gauss-point values gave 3.593 at the node. Smoothed contour plots can exaggerate the peak ([[ref-roylance-mit-3-11-mechanics-of-materials]], "Finite Element Analysis").

**Recovery and error estimation.** If the recovered field is better than the raw one, their difference estimates the error of the raw field; this is the basis of recovery-type estimators ([[a-posteriori-error-indicators]]). The superconvergent patch recovery of Zienkiewicz and Zhu was not described in the sources read.

## Builds on

- [[finite-element-method-basics]]: $\mathbf B$, element stresses, Gauss points.
- [[a-posteriori-error-indicators]]: error estimates from computed solutions.

## Leads to

- [[fatigue]], [[fracture-mechanics]] (both need accurate local stress)

## How rcfd uses it

- Integration-point, element and nodal stress and strain with von Mises and principal values (F-376, ADR 0277, done), stored as `IntegrationPointField` ([[adr-thread-structural-basic-tier]]).
- Superconvergent patch recovery and a Zienkiewicz-Zhu energy-norm estimate (F-432, ADR 0478, partial with three misses, follow-up F-568) ([[adr-thread-structural-verification-elements]]).
