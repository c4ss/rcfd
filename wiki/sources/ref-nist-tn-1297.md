---
type: source
tags: [reference, foundation, verification-validation, measurement, nist, cite-only]
sources: []
origin: https://www.nist.gov/pml/nist-technical-note-1297
licence: no notice on the pages read; NIST site policy (https://www.nist.gov/copyrights-disclaimers) allows copying "With the exception of material marked as copyrighted", credit requested
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# NIST Technical Note 1297 (Taylor and Kuyatt, 1994)

**Origin:** Barry N. Taylor and Chris E. Kuyatt, *Guidelines for Evaluating and Expressing the Uncertainty of NIST Measurement Results*, NIST Technical Note 1297, 1994 edition (supersedes 1993), doi:10.6028/NIST.TN.1297. HTML sections read: 2 (https://www.nist.gov/physical-measurement-laboratory/nist-tn-1297-2-classif-components-uncertainty), 5, 6 and Appendix A (law of propagation of uncertainty).
**Kind:** US Government technical note; NIST's implementation of the ISO Guide to the Expression of Uncertainty in Measurement (GUM).
**Licence:** no notice on the sections; NIST site policy as in the frontmatter. Cite only until the maintainer accepts the site policy as a copy licence.
**Read:** 2026-11-22, by job p6-roots.

## Summary

How a measurement's uncertainty is expressed: standard uncertainty as an estimated standard deviation, Type A and Type B evaluation, combining components into a combined standard uncertainty by the law of propagation of uncertainty, and an expanded uncertainty `U = k u_c` with coverage factor `k`.

## Key points by section

### Sec. 2 "Classification of Components of Uncertainty"
- 2.4: each component is represented by an estimated standard deviation, the standard uncertainty `u_i` (positive square root of the estimated variance).
- 2.5: Type A evaluation = by statistical analysis of a series of observations (`u_i = s_i`, with degrees of freedom).
- 2.6: Type B evaluation = by other means, from an assumed probability distribution based on all available information.
- 2.3: the labels "random uncertainty" and "systematic uncertainty" can mislead; a component should be described as arising from a random effect or a systematic effect, since the same component can change category depending on how it enters the measurement model.

### Sec. 5 "Combined Standard Uncertainty"
- 5.1: `u_c` is the estimated standard deviation of the result, combining the `u_i` (and covariances) by the law of propagation of uncertainty, the "root-sum-of-squares" method; equivalent documented methods (e.g. bootstrap) are accepted.
- 5.2: each recognised systematic effect is corrected for; its uncertainty is the standard uncertainty of the correction.
- 5.4: for an approximately normal distribution, `y +- u_c` is an interval of about 68 % confidence.

### Appendix A "Law of Propagation of Uncertainty"
- A-1, A-2: measurement model `Y = f(X1, ..., XN)`, estimate `y = f(x1, ..., xN)`.
- A-3: `u_c^2(y) = sum_i (df/dx_i)^2 u^2(x_i) + 2 sum_(i<j) (df/dx_i)(df/dx_j) u(x_i, x_j)`; `df/dx_i` are the sensitivity coefficients, `u(x_i, x_j)` the estimated covariance. With uncorrelated inputs the covariance sum vanishes (our reading; the page does not write that case out).
- A-4, A-5: Type A input estimate = mean of `n` observations, standard uncertainty = estimated standard deviation of the mean. A-6, A-7: Type B rectangular distribution of half-width `a`: standard uncertainty `a / sqrt(3)`.

### Sec. 6 "Expanded Uncertainty"
- 6.1: `U = k u_c(y)`; the measurand is "confidently believed" to lie in `y - U <= Y <= y + U`.
- 6.2: `k` typically 2 to 3; `k = 2` gives about 95 % for a normal distribution with well-known `u_c`, `k = 3` more than 99 %.
- 6.3-6.4: tying `k` to an exact level needs the distributions; 6.5: NIST reports `U` with `k = 2` by convention, other values only for documented requirements (e.g. a t-factor when `u_c` has few degrees of freedom).

## Contradictions with the wiki

None.

## Pages this source informs

[[measurement-uncertainty]], [[validation-comparison]], [[probability-and-statistics-basics]]
